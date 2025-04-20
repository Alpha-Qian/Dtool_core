//use std::io::SeekFrom;
use std::{future::Future, io::SeekFrom, mem::{transmute, transmute_copy, MaybeUninit}, ops::Deref, path::Iter, sync::{atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering}, Arc}};
use thiserror::Error;
use headers::{self,
    HeaderMapExt,
    Range};
use reqwest::{
    self, header::{
        self, HeaderMap,HeaderValue, IF_MATCH, IF_RANGE, RANGE
    }, Client, Method, Request, RequestBuilder, Response, Url
};

use futures::stream::StreamExt;
use tokio::{sync::SemaphorePermit, task::{AbortHandle, JoinHandle, JoinSet}};

use crate::cache::{Writer, Cacher};
use crate::tracker::Tracker;
use futures;

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
type DownloadResult<T> = Result<T, DownloadError>;

impl UrlKind {
    fn range_able(){
        Self::EnsureKind(EnsureUrlKind::Rangeable(_))
    }
}

async fn get_first_response(client: Client, url: &str, headers: &HeaderMap) -> DownloadResult<DonwloadInfo> {
    
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

macro_rules! try_break {
    ($expr:expr, $label:lifetime) => {
        match $expr {
            // 如果是 Ok(v)，宏展开为 v
            Ok(v) => v,
            // 如果是 Err(e)，执行带标签的 break，并将错误 e 包装后作为块的值
            // 使用 From::from 允许错误类型的自动转换，行为类似于 ? 操作符
            Err(e) => break $label Err(::std::convert::From::from(e)),
        }
    };
}

enum WithOption{
    Response(Response, Block),
    Block(Block),
}

impl WithOption {
    fn with_block(block: Block) -> Self {
        Self::Block(block)
    }
    
    fn with_response(response: Response) -> Self {
        Self::Response(response, block)
    }
}

pub async fn download_with_block<T, E>(
    client: &Client,
    block: &Block,
    tracker: T,
    hand_error: fn(&DownloadError) -> E,
    headers_builder: impl FnOnce(&mut HeaderMap),
) -> DownloadResult<()>
where 
    E: Future<Output = CheckError>,
    T: Tracker,
{
    download_block(client, block, None, tracker, hand_error, headers_builder).await
}

///尝试多次下载，非致命错误会重试
pub(crate) async fn download_block<T, E, E1, E2, C>(//不如作为单独的函数
    url: &Url,
    headers_builder: impl FnOnce(&mut HeaderMap),
    client: &Client,
    cache: &C,
    block: &Block,
    first_response: Option<Response>,//这里假设了block是对应first_response的范围
    tracker: &T,
    hand_result: E,
) -> DownloadResult<()>
where 
    E: FnMut(&DownloadError) -> E1, E1 : Future<Output = E2>,
    T: Tracker,
    C: Cacher,
{   
    let mut process = block.process.load(Ordering::Acquire);
    let mut writer = cache.write_at(SeekFrom::Start(process)).await;
    loop {
        let result: reqwest::Result<()> = 'inner: {

            let response = match first_response {
                Some(r) => r,
                None => {
                    let mut req = Request::new(Method::GET, self.url.clone());
                    req.headers_mut().typed_insert(Range::bytes(
                        process..block.end,
                    ).expect("Range header error"));
                    headers_builder(req.headers_mut());
                    try_break!(client.execute(req).await, 'inner)
                }
            };
            //reciving_guard
            let mut stream = response.bytes_stream();
            while let Some(item) = stream.next().await{
                let chunk = try_break!(item, 'inner);
                let chunk_size = chunk.len();

                let _guard = //block_guard(block);
                if process + chunk_size as u64 > block.end {
                    writer.down_write(&chunk[..(block.end - process) as usize]).await?;
                    block.process.store(block.end, Ordering::Release);
                    tracker.record((block.end - process) as u32).await;
                    break;
                };
                writer.down_write(chunk.as_ref()).await?;
                block.process.store(process, Ordering::Release);
                tracker.record(chunk_size as u32).await;
            }
            Ok(())
        };
        match result {
            Ok(_) => {
                if block.process_done() {
                    break Ok(());
                } else {
                    break Err(DownloadError::WriteError());
                }
            }
            Err(e) => {
                e.is_connect()
            }
        }
        let finally_result = todo!("handing result");//handing result
        let a = panic!();
        finally_result

        // let response = match first_response {
        //     Some(r) => r,
        //     None => {
        //         let mut req = Request::new(Method::GET, self.url.clone());
        //         self.headers_builder(req.headers_mut());
        //         client.execute(req).await?
        //     }
        // }; 
        // first_response = None;
        // let mut writer = self.cache.write_at(SeekFrom::Start(block.process.load(Ordering::Acquire))).await;
        // let result = self.download_once(&client, block, response, &tracker).await;
        // result?;
    }
}

    // ///尝试进行一次下载，可能返回错误
    // #[inline]
    // async fn download_once<T>(
    //     &self,
    //     client: &Client, 
    //     block: &Block, 
    //     response: Response, 
    //     tracker: &T,
    //     writer: &mut impl Writer,
    //     block_guard: impl fn() -> impl Future<Output = ()>,
    // ) -> DownloadResult<()>
    // where T: Tracker {
    //     //block.on_receiving();
    //     //let mut writer= self.cache.write_at(SeekFrom::Start(block.process())).await;

    //     let mut process = block.process.load(Ordering::Acquire);
    //     let mut stream = response.bytes_stream();
    //     while let Some(item) = stream.next().await{
    //         let chunk = item?;
    //         let chunk_size = chunk.len();

    //         let _guard = 
    //         if process + chunk_size as u64 > block.end {
    //             writer.down_write(&chunk[..(block.end - process) as usize]).await?;
    //             block.process.store(block.end, Ordering::Release);

    //             tracker.fetch_add(block.end - process).await;
    //             break;
    //         }
    //         writer.down_write(chunk.as_ref()).await?;
    //         block.process.store(process, Ordering::Release);
            
    //         tracker.fetch_add(chunk_size as u32).await;
    //     }
    //     Ok(())
    // }




///可续传单线程下载器
impl<C: Cacher,T: Tracker> RangeableDownloader<C, T> {
    async fn download(&self, response:Response) {
        let stream = response.bytes_stream();
        let mut writer = self.cache.write_at(SeekFrom::Start(self.process.load(Ordering::Acquire))).await;
        //let mut tracker = self.tracker_builder.build_tracker();
        while let Some(item) = stream.next().await {
            let chunk = item?.as_ref();
            writer.down_write(&chunk).await?;
            self.tracker.record(chunk.len() as u32).await;
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


pub struct Block{
    process: AtomicU64,
    end: u64,
}
struct BlockIter{
    remainnum: u8,
    process: u64,
    end: u64,
    chunk_size:u64,
}
impl Iterator for BlockIter{
    type Item = Block;
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}


impl Block {
    pub fn new(process: u64, end: u64) -> Self{
        Block{
            process: AtomicU64::new(process), 
            end,
        }
    }

    pub fn splits(&mut self, num: u8) -> BlockIter{
        

        BlockIter{
            remainnum: num,
            process: self.process.load(Ordering::Acquire),
            end: self.end,
            chunk_size: (self.end - self.process.load(Ordering::Acquire)) / num as u64,
        }
    }
    pub fn process(&self) -> u64 {
        self.process.load(Ordering::Acquire)
    }

    pub fn end(&self) -> u64 {
        self.end
    }
    pub fn remaining(&self) -> u64 {
        self.end - self.process.load(Ordering::Acquire)
    }
    pub(crate) fn process_done(&self) -> bool{
        debug_assert!(self.process.load(Ordering::Acquire) <= self.end);
        self.process.load(Ordering::Acquire) == self.end
    }

    pub unsafe fn to_static(&self) -> &'static Self {
        std::mem::transmute(self)
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



#[cfg(test)]
mod tests{
    #[test]
    fn test() {

        println!("hello world");
    }
}