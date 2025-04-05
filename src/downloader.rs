//use std::io::SeekFrom;
use std::{
    sync::{
        Arc,
        atomic::{
            AtomicU8, AtomicU16, AtomicU64, Ordering
        },

    },
    mem::{MaybeUninit,
        zeroed,
    },
    io::SeekFrom,
    ops::Deref,
    fmt::Display,
};
//use std::error::Error;
use thiserror::Error;
use headers::{self,
    HeaderMapExt,
    Range
};
use reqwest::{
    self, Client, Request, RequestBuilder, Response,
    header::{
        self, HeaderMap,HeaderValue, IF_MATCH, IF_RANGE, RANGE
    }
};
use parking_lot::{
    RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use futures::stream::StreamExt;
use tokio::task::{JoinHandle, AbortHandle, JoinSet};
use crate::{cache, tracker};

use crate::cache::{Writer, Cacher};
use crate::tracker::{Tracker, TrackerBuilder};
use futures;

type BlocksVec<T: TrackerBuilder> = Vec<Box<Block<T::Output>>>;

struct DownloadRef<C, T : TrackerBuilder>{
    url:String,
    cache: C,
    tracker_builder: T,
    //task_call_back: B,    //任务完成或可恢复错误回调
    headers: HeaderMap,
    blocks: RwLock<BlocksVec<T>>,

    connection: AtomicU16,      //已经建立的连接数

    blocks_pending: AtomicU16,
    blocks_running: AtomicU16,
    block_done: AtomicU16,
}

pub struct UrlDownloader<C, T: TrackerBuilder> {
    client: reqwest::Client,//Client自带Arc
    pub(crate) inner: Arc<DownloadRef<C, T>>,
    pub(crate) tasks: JoinSet<DownloadResult<()>>,
}

///单线程可续传下载器
struct RangeableDownloader<C, T>{
    cache: C,
    tracker: T,
    client: Client,
    process: AtomicU64,
}

///单线程不可续传下载器
struct UnRangeableDownloader<C, T>{
    cache: C,
    tracker: T,
    client: Client,
    process: AtomicU64,
}

struct DonwloadInfo{  
    finaly_url: String,
    url_kind: UrlKind,
    headers: HeaderMap,
    client: Client,
    response: Response,
}

enum UrlKind {
    EnsureKind(EnsureUrlKind),
    UnsureKind,
}

enum EnsureUrlKind {
    RangeAble,
    UnRangeAble,
}


#[derive(Debug, Error, Display)]
enum DownloadError {
    WriteError(),
    NetworkError(#[from] reqwest::Error),
    SeverFileChanged,
}

impl DownloadError {
    fn retryable(&self) -> bool {
        match self {
            DownloadError::NetworkError(_) => true,

            _ => false,
        }
    }
}

type DownloadResult<T> = Result<T, DownloadError>;

impl UrlKind {
    fn range_able(){
        Self::EnsureKind(EnsureUrlKind::Rangeable(_))
    }
}

async fn send_first_request(client: Client, url: &str, headers: &HeaderMap) -> DownloadResult<DonwloadInfo> {
    
    let response = client.get(url)
        .headers(headers.clone())
        .header(RANGE, headers::Range::bytes(0..))
        .send()
        .await?
        .error_for_status()?;
    let fanily_url = response.url();
    if response.status().as_u16() == 206 {
        ///可续传
    } else {
        ///不可续传
    }
    Ok(response)
}

impl<C: cache::Cacher, T:TrackerBuilder> UrlDownloader<C, T> {

    fn from_downloadinfo(info: DonwloadInfo, cache: C, tracker_builder: T) -> Self {
        Self {
            client: info.client,
            inner: Arc::new(DownloadRef {
                url: info.finaly_url,
                cache,
                tracker_builder,
                headers: info.headers,
                blocks: RwLock::new(vec![]),
                connection: AtomicU16::new(0),
            }),
            tasks: JoinSet::new(),
        }
    }

    pub fn blocks(&self) -> &RwLock<Vec<Box<Block<T::Output>>>>{
        &self.inner.blocks
    }
    
    pub fn join_set(&self) -> &JoinSet<DownloadResult<()>> {
        &self.tasks
    }

    async fn create_task(&mut self, block: &mut Block) {
        self.tasks.spawn(self.inner.start_block(self.client.clone(), block.get_guard()));
    }

    async fn join_all(&mut self) {
        //self.tasks.join_all().await;
        while let Some(result) = self.tasks.join_next().await {
            match result {
                Ok(_) => {},
                Err(err) if err.is_panic() => {
                    use std::panic;
                    panic::resume_unwind(err.into_panic());
                }
                Err(err) => {todo!("处理错误")}
            }
        }
    }

    fn blocks_pending(&self) -> u16 {
        self.inner.blocks_pending.load(Ordering::Acquire)
    }

    fn blocks_running(&self) -> u16 {
        self.inner.blocks_running.load(Ordering::Acquire)
    }

    fn block_done(&self) -> u16 {
        self.inner.block_done.load(Ordering::Acquire)
    }

    fn connection(&self) -> u16 {
        self.inner.connection.load(Ordering::Acquire)
    }

    ///频繁调用此方法是低效的
    fn downloaded(&self) -> u64 {
        self.inner.blocks.read().iter().map(|block| block.process() - block.start).sum()
    }
    
    ///频繁调用此方法是低效的
    fn remaining(&self) -> u64 {
        self.inner.blocks.read().iter().map(|block| block.end() - block.process()).sum()
    }

    async fn shutdown(&mut self) {
        self.tasks.shutdown().await;
    }

    ///返回任务数
    fn num_tasks(&self) -> usize{
        self.tasks.len()
    }

    ///返回已创建的连接数
    fn active_connections(&self) -> u16 {
        self.inner.connection.load(Ordering::Acquire)
    }
}


///定义需要在tokio中运行的方法
impl<C: Cacher, T: TrackerBuilder> DownloadRef<C, T> {

    async fn start_block(self: Arc<Self>, client: Client, block: RunningGuard<C, T::Output>) -> DownloadResult<()> {                      
        let response = client.get(&self.url)
            .headers(self.headers.clone())
            //.header(RANGE, format!("bytes={}-{}", block.process.load(Ordering::Acquire), block.end - 1))
            .header(RANGE, headers::Range::bytes(block.process.load(Ordering::Acquire)..block.end).unwrap())
            .send()
            .await?;
        self.download_block(response, block).await
    }

    ///获取response返回的字节流
    async fn download_block(self: Arc<Self>, response: Response, block: RunningGuard<C, T::Output>) -> DownloadResult<()>{
        //BlockGuard的生命周期必须小于Arc<Self>
        block.on_receiving();
        let mut writer= self.cache.write_at(SeekFrom::Start(block.start)).await;

        let mut process = block.process.load(Ordering::Acquire);
        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await{
            let chunk = item?;
            let chunk_size = chunk.len();

            let _guard = self.blocks.read();
            if process + chunk_size as u64 > block.end {
                writer.down_write(&chunk[..(block.end - process) as usize]).await?;
                block.process.store(block.end, Ordering::Release);
                break;
            }
            writer.down_write(chunk.as_ref()).await?;
            process += chunk_size as u64;
            block.process.store(process, Ordering::Release);
        };
        Ok(())
    }
}


///可续传单线程下载器
impl<C: Cacher,T: Tracker> RangeableDownloader<C, T> {
    async fn download(&self, response:Response) {
        //let range = format!("bytes={}-", self.process.load(Ordering::Acquire));
        let stream = response.bytes_stream();
        let mut writer = self.cache.write_at(SeekFrom::Start(self.process.load(Ordering::Acquire))).await;
        //let mut tracker = self.tracker_builder.build_tracker();
        while let Some(item) = stream.next().await {
            let chunk = item?.as_ref();
            writer.down_write(&chunk).await?;
            self.tracker.fetch_add(chunk.len() as u32).await;
        }
    }
}

impl<C, T> UnRangeableDownloader<C, T> {
    async fn download(&self, from_pos: u64) {
        self.process.store(0, Ordering::Release);
        while let Some(chunk) = response.bytes().await? {
            self.
        }
    }
}


struct RecevingGuard<C,T> {
    downloader: Arc<DownloadRef<C,T>>,
}

impl<C: Cacher, T: TrackerBuilder> RecevingGuard<C, T> {
    fn new(downloader: Arc<DownloadRef<C,T>>) -> Self {
        downloader.connection.fetch_add(1, Ordering::Release);
        Self {
            downloader
        }
    }
}

impl<C, T> Drop for RecevingGuard<C, T> {
    fn drop(&mut self) {
        self.downloader.connection.fetch_sub(1, Ordering::Release);
    }
}


enum CheckType{
    ETag(HeaderValue),
    LastMotifield(HeaderValue),
    None,
}

impl CheckType {
    fn check(&self, response: &Response) -> bool {
        match self {
            CheckType::ETag(etag) => {
                let etag_header = response.headers().get(header::ETAG).unwrap();
                etag_header == etag
            }
            CheckType::LastMotifield(last_motifield) => {
                let last_motifield_header = response.headers().get(header::LAST_MODIFIED).unwrap();
                last_motifield_header == last_motifield
            }
            CheckType::None => true,
        }
    }
}

impl CheckType {
    fn with_response(response: &Response) -> Self {
        if let Some(etag) = response.headers().get(header::ETAG) {
            CheckType::ETag(etag.clone())
        } else if let Some(last_motifield) = response.headers().get(header::LAST_MODIFIED) {
            CheckType::LastMotifield(last_motifield.clone())
        } else {
            CheckType::None
        }
    }

    fn build_if_range_requests(&self, builder: RequestBuilder) -> RequestBuilder {
        match self {
            CheckType::ETag(etag) => {
                builder.header(IF_RANGE, etag.clone())
            }
            CheckType::LastMotifield(last_motifield) => {
                builder.header(IF_RANGE, last_motifield.clone())
            }
            CheckType::None => builder,
        }
    }
}

///转换普通引用为静态引用
unsafe fn to_static<'a, T>(t: &'a T) -> &'static T {
    &*(t as *const T)
}

pub struct Block<T>{
    start: u64,
    process: AtomicU64,
    end: u64,
    tracker: T,
    state: AtomBlockState,
    abort_handle: Option<AbortHandle>
}

// struct TaskState<T> {
//     state: AtomicU8,
//     task_info: MaybeUninit<TaskInfo<T>>,
// }
// ///用于记录任务信息
// struct TaskInfo<T> {
//     abort_handle: AbortHandle,
//     //tracker: T,
// }

struct RunningGuard<C, T>{
    downloader: Arc<DownloadRef<C, T>>,
    block: &'static Block,
}

impl<T: Tracker> Block<T> {
    pub fn new(start: u64, process: u64, end: u64) -> Self{
        Block{
            start,
            process: AtomicU64::new(process), 
            end,
            state: AtomBlockState::pending(),
            tracker:
            //task_info: MaybeUninit::uninit(),
            //abort_handle: MaybeUninit::uninit(),
        }
    }

    pub fn process(&self) -> u64 {
        self.process.load(Ordering::Acquire)
    }

    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn state(&self) -> BlockState {
        self.state.get()
    }

    pub fn downloaded(&self) -> u64 {
        self.process.load(Ordering::Acquire) - self.start
    }

    pub fn remaining(&self) -> u64 {
        self.end - self.process.load(Ordering::Acquire)
    }
    
    pub(crate) fn set_state(&self, state: BlockState) {
        self.state.set(state)
    }

    pub fn running(&self) -> bool {
        self.state.running()
    }

    pub(crate) fn process_done(&self) -> bool{
        debug_assert!(self.process.load(Ordering::Acquire) <= self.end);
        self.process.load(Ordering::Acquire) == self.end
    }
}


impl<C, T> RunningGuard<C, T> {

    unsafe fn new(block: &Block, downloader: Arc<DownloadRef<C, T>>) -> Self {
        downloader.blocks_running.fetch_add(1, Ordering::Release);
        downloader.blocks_pending.fetch_sub(1, Ordering::Release);
        block.set_state(BlockState::Requesting);
        block.task_info.write(TaskInfo { abort_handle: (), tracker: () });
        Self {
            downloader,
            block: to_static(block),
        }
    }
 
    fn on_receiving(&self) {
        self.block.set_state(BlockState::Receving);
        self.downloader.connection.fetch_add(1, Ordering::Release);
    }
}

impl<C, T> Drop for RunningGuard<C, T> {
    fn drop(&mut self) {
        //let block = unsafe{&*self.block};
        //debug_assert!(self.block.running());
        if self.block.state() == BlockState::Receving{
            self.downloader.connection.fetch_sub(1, Ordering::Release);
        }
        self.downloader.blocks_running.fetch_sub(1, Ordering::Release);


        if self.block.process_done(){
            self.block.set_state(BlockState::Done);
            self.downloader.block_done.fetch_add(1, Ordering::Release);
        } else {
            self.block.set_state(BlockState::Pending);
            self.downloader.blocks_pending.fetch_add(1, Ordering::Release);
        }
        unsafe {
            self.block.task_info.assume_init_drop();
            self.block.task_info
        }
    }
}

impl<C, T> Deref for RunningGuard<C, T>{
    type Target = Block;
    fn deref(&self) -> &Self::Target {
        self.block
    }
}


pub enum BlockState {
    Pending,//待办
    Requesting{task: AbortHandle},//正在发送get请求
    Receving{task: AbortHandle},//正在接收数据
    Done,//已完成
}

///私有状态
enum InnerState{
    Pending,
    Requesting,
    Receiving,
    Done,
}

struct AtomBlockState{
    pub(crate) state:AtomicU8,
    abort_handle: MaybeUninit<AbortHandle>,
}

impl BlockState {
    fn is_running(&self) -> bool {
        match self {
            BlockState::Requesting | BlockState::Receving => true,
            _ => false,
        }
    }
}

impl AtomBlockState {

    pub fn get(&self) -> BlockState{
        match self.state.load(Ordering::Acquire) {
            0 => BlockState::Pending,
            1 => BlockState::Requesting{task: unsafe{self.abort_handle.assume_init_ref().clone()}},
            2 => BlockState::Receving{task: unsafe{self.abort_handle.assume_init_ref().clone()}},
            3 => BlockState::Done,
            _ => unreachable!("Invalid state")
        }
    }

    pub(crate) fn set(&self, state:BlockState){
        let val = match state {
            BlockState::Pending => 0,
            BlockState::Requesting{task} => {1},
            BlockState::Receving{task} => 2,
            BlockState::Done => 3,
        };
        self.state.store(val, Ordering::Release);
    }

    fn on_requesting(&self, abort_handle: AbortHandle) {
        debug_assert!(!self.get().is_running());
        self.set(BlockState::Requesting{task: abort_handle});
    }

    fn on_receiving(&self) {
        debug_assert!(self.state.load(Ordering::Acquire) == 1);
        self.state.store(2, Ordering::Release);
    }

    fn set_pending(&mut self) {
        debug_assert!(self.get().is_running());
        self.state.store(0, Ordering::Release);
        unsafe {
            self.abort_handle.assume_init_drop();
        }
    }

    fn set_done(&mut self) {
        debug_assert!(self.get().is_running());
        self.state.store(3, Ordering::Release);
        unsafe {
            self.abort_handle.assume_init_drop();
        }
    }

    pub fn running(&self) -> bool {
        match self.get(){
            BlockState::Pending | BlockState::Receving => true,
            _ => false,
        }
    }

    fn new(state:BlockState) -> Self{
        Self{
            state:AtomicU8::new(match state {
                BlockState::Pending => 0,
                BlockState::Requesting => 1,
                BlockState::Receving => 2,
                BlockState::Done => 3,
            })
        }
    }

    fn pending() -> Self{
        Self::new(BlockState::Pending)
    }

    fn done() -> Self{
        Self::new(BlockState::Done)
    }
}

trait HeaderBuilder{
    fn build_headers(header:&mut HeaderMap);
}

#[cfg(test)]
mod tests{
    #[test]
    fn test() {

        println!("hello world");
    }
}