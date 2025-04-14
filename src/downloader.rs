//use std::io::SeekFrom;
use std::{future::Future, io::SeekFrom, mem::{transmute, transmute_copy, MaybeUninit}, ops::Deref, sync::{atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering}, Arc}};
use thiserror::Error;
use headers::{self,
    HeaderMapExt,
    Range};
use reqwest::{
    self, header::{
        self, HeaderMap,HeaderValue, IF_MATCH, IF_RANGE, RANGE
    }, Client, Method, Request, RequestBuilder, Response, Url
};
use parking_lot::{
    RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use futures::stream::StreamExt;
use tokio::{sync::SemaphorePermit, task::{AbortHandle, JoinHandle, JoinSet}};
use crate::{cache, tracker};

use crate::cache::{Writer, Cacher};
use crate::tracker::{Tracker, TrackerBuilder};
use futures;

pub struct DownloadRef{
    url: Url,
    blocks: RwLock<Vec<Box<Block<T::Output>>>>,
    semaphore: tokio::sync::Semaphore,
    headers_builder: H,
    cache: C,
    controler: Co,
    tasks: JoinSet<DownloadResult<()>>,
}
pub struct UrlDownloader {
    client: reqwest::Client,//Client自带Arc
    pub(crate) inner: Arc<DownloadRef>,
    pub(crate) tasks: JoinSet<DownloadResult<()>>,
}

///单线程可续传下载器
struct RangeableDownloader<C, T>{
    url: Url,
    cache: C,
    tracker: T,
    client: Client,
    process: AtomicU64,
}

///单线程不可续传下载器
struct UnRangeableDownloader<C, T>{
    url: Url,
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


#[derive(Debug, Error)]
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


impl UrlDownloader 
{
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
}

enum Guard<T, N> {
    Some(T),
    Future(N),
}

impl<T, N> Guard<T, N> 
where N: Future<Output = T> {
    async fn get(self) -> T {
        match self {
            Guard::Some(some) => some,
            Guard::Future(future) => future.await,
        }
    }
}

///定义需要在tokio中运行的方法
impl DownloadRef
{
    ///只添加信号量空闲令牌
    pub fn add_pre();
    
    ///通过凭空创建信号量通行证立即分配任务
    pub fn add_tasks(self: &Arc<Self>, block: Block<T::Output>) {
        self.blocks.write().push(Box::new(block));
    }

    ///信号量有空闲时，启动下载任务
    pub async fn download_check(self: &Arc<Self>, client: Client, guard: Guard<T, N>){
        let sem = self.semaphore.acquire().await.expect("msg");
        self.tasks.spawn(self.clone().download_check(client));
        let block: &Block<_> = todo!("添加切块逻辑");
        self.download_block(client, block, Some(sem), None).await?;
    }

    ///尝试多次下载，非致命错误会重试
    pub async fn download_block<T,N>(self: Arc<Self>, client: Client, block:&'static Block<_> , guard: Guard<T, N>, first_response: Option<Response>) -> DownloadResult<()>
    where N: Future<Output = T>,
    {
        let _guard = guard.get().await;
        loop {
            // let response = match first_response {
            //     Some(r) => r,
            //     None => {
            //         let mut req = Request::new(Method::GET, self.url.clone());
            //         self.headers_builder(req.headers_mut());
            //         client.execute(req).await?
            //     }
            // };
            self.download_once(&client, block, first_response).await?
            first_response = None;
        }
    }

    ///尝试进行一次下载，可能你返回错误
    #[inline]
    async fn download_once(self: &Arc<Self>, client: &Client, block: &'static Block<_>, first_response: Option<Response>>) -> DownloadResult<()>{
        
        let response = match first_response {
            Some(r) => r,
            None => {
                let mut req = Request::new(Method::GET, self.url.clone());
                self.headers_builder(req.headers_mut());
                client.execute(req).await?
            }
        };
        
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

                block.tracker.fetch_add(block.end - process).await;
                self.controler.fetch_add(block.end - process as u32).await;
                break;
            }
            writer.down_write(chunk.as_ref()).await?;
            block.process.store(process, Ordering::Release);
            
            block.tracker.fetch_add(chunk_size as u32).await;
            self.controler.fetch_add(chunk_size as u32).await;
        }
        Ok(())
    }

}


///可续传单线程下载器
impl<C: Cacher,T: Tracker> RangeableDownloader<C, T> {
    async fn download(&self, response:Response) {
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
    downloader: Arc<DownloadRef<C, T>>,
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
    //&*(t as *const T)
    std::mem::transmute(t)
}

unsafe fn to_static_pre(t: SemaphorePermit<'_>) -> SemaphorePermit<'static> {
    std::mem::transmute(t)
}

pub struct Block<T>{
    start: u64,
    process: AtomicU64,
    end: u64,
    tracker: T,
    state: AtomBlockState,
    abort_handle: Option<AbortHandle>
}


impl<T: Tracker> Block<T> {
    pub fn new(start: u64, process: u64, end: u64) -> Self{
        Block{
            start,
            process: AtomicU64::new(process), 
            end,
            state: AtomBlockState::waiting(),
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



pub enum BlockState {
    Pending,//待办
    Requesting{task: AbortHandle},//正在发送get请求
    Receving{task: AbortHandle},//正在接收数据
    Done,//已完成
}


struct AtomBlockState{
    pub(crate) state: AtomicU8,
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

    fn set_waiting(&mut self) {
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

    fn waiting() -> Self{
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