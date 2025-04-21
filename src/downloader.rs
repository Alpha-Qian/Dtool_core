//use std::io::SeekFrom;
use std::{future::Future, io::SeekFrom, mem::{transmuteMaybeUninit},sync::{atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering}, Arc}, time::Duration};
use thiserror::Error;
use headers::{self,
    HeaderMapExt,
    Range};
use reqwest::{
    self, header::{
        self, HeaderMap,HeaderValue, IF_MATCH, IF_RANGE, RANGE
    }, Client, Method, Request, Response, Url, Version
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



type RequestBuilder = fn(&mut HeaderMap, &mut Option<Duration>, Option<Version>);
type ResponseCheker = fn(&Response) -> Result<(), DownloadError>;
type ResultHander = fn(reqwest::Result<()>) -> DownloadResult<()>;

enum ResponseRange{
    Response(Response),//从response中解析进度和结束位置
    Range{start: &mut u64, end: Option<&u64>},//当前进度和是否提前结束
}
///尝试可续传链接的多次下载，非致命错误会重试
#[inline]
pub(crate) async fn download_block<T, C>(//不如作为单独的函数
    url: &Url,
    headers_builder: impl FnOnce(&mut HeaderMap),
    client: &Client,
    cache: &C,
    process_now: u64,//进度
    process: Option<&AtomicU64>,//是否使用原子变量同步
    end: Option<&u64>,//是否提前结束
    first_response: Option<Response>,//这里假设了block是对应first_response的范围
    tracker: &T,
) -> DownloadResult<()>
where
    T: Tracker,
    C: Cacher,
{   
    //let mut process_now = process.load(Ordering::Acquire);
    let mut writer = cache.write_at(SeekFrom::Start(process_now)).await;
    loop {
        let result: reqwest::Result<()> = 'inner: {

            let response = match first_response {
                Some(r) => r,
                None => {
                    let mut req = Request::new(Method::GET, url.clone());
                    let range = match end {
                        Some(e) => Range::bytes(process_now..*e).expect("msg"),
                        None => Range::bytes(process_now..).expect("msg"),
                    };
                    req.headers_mut().typed_insert(range);
                    headers_builder(req.headers_mut());
                    try_break!(client.execute(req).await, 'inner)
                    //parse response here
                }
            };

            //reciving_guard
            let mut stream = response.bytes_stream();
            while let Some(item) = stream.next().await{
                let chunk = try_break!(item, 'inner);
                let chunk_size = chunk.len();

                let _guard = //block_guard(block);
                if let Some(e) = end{
                    if process_now + chunk_size as u64 > *e {
                        writer.down_write(&chunk[..(*e - process_now) as usize]).await?;
                        process_now += *e - process_now;
                        if let Some(p) = process { p.store(*e, Ordering::Release) }
                        tracker.record((*e - process_now) as u32).await;
                        break;
                    };
                };
                writer.down_write(chunk.as_ref()).await?;
                process_now += chunk_size as u64;
                if let Some(p) = process { p.store(process_now, Ordering::Release) }
                tracker.record(chunk_size as u32).await;
            }
            Ok(())
        };
        match result {
            Err(e) => {},
            Ok(_) => {}
        }
        let finally_result = todo!("handing result");//handing result
        let a = panic!();
        finally_result
    }//loop end
}


pub(crate) async fn download_unrangeable(
){

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