//use std::io::SeekFrom;
use std::{future::Future, io::SeekFrom, mem::{transmute,MaybeUninit}, os::windows::process, sync::{atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering}, Arc}, time::Duration};
use thiserror::Error;
use std::ptr::NonNull;
use headers::{self,
    HeaderMapExt,
    Range};
use reqwest::{
    self, header::{
        self, HeaderMap,HeaderValue, IF_MATCH, IF_RANGE, RANGE
    }, Client, Method, Request, Response, StatusCode, Url, Version
};

use futures::stream::StreamExt;
use tokio::{sync::SemaphorePermit, task::{AbortHandle, JoinHandle, JoinSet}};

use crate::cache::{Writer, Cacher};
use crate::tracker::Tracker;
use futures;
use parking_lot::Mutex;

struct DonwloadInfo{  
    finaly_url: String,
    url_kind: UrlKind,
    headers: HeaderMap,
    client: Client,
    response: Response,
}

struct FirstResponse{
    response: Response,
    rangeable: bool,
}

impl FirstResponse {
    async fn new(url: Url, headers: HeaderMap, client: Client) -> Self {
        Self {
            response,
        }
    }
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

// enum ResponseRange{
//     Some{response: Response, process: u64, end: u64},
//     None{process: u64, end: u64}
// }

struct ResponseRange{
    response: Option<Response>,
    process: u64,
    end: Option<u64>,
}

impl ResponseRange {
    pub fn new_none(process: u64, end: u64) -> Self {
    }

    pub fn new_some(response: Response) -> Self {
    }

    pub unsafe fn new_some_unchecked(response: Response, process: u64, end: u64) -> Self {

    }

    fn get_response_mut(&mut self) -> Option<&mut Response> {
        match self {

        }
    }

    fn into_parts(self) -> (Option<Response>, u64, u64) {
        match self {
        }
    }

}

enum EndType<'a>{
    u64(NonNull<u64>),
    Atomicu64(&'a AtomicU64),
    None 
}

type RequestBuilder = fn(&mut HeaderMap, &mut Option<Duration>, &mut Option<Version>);
type ResponseCheker = fn(&Response) -> Result<(), DownloadError>;
type ResultHander = fn(reqwest::Result<()>) -> DownloadResult<()>;

///尝试可续传链接的多次下载，非致命错误会重试
/// 遵守以下规则以保证不会发生未定义行为：
///  1.shared_process和end参数必须在调用获取锁闭包后可用
/// 遵守以下规则以保证不会发生错误的行为：
///  1.process参数必须正确对应response范围响应的起始位置
///  2.end参数必须不能超过对应response范围响应的终止位置
///  3.
#[inline]
pub(crate) async unsafe fn download_block(//不如作为单独的函数
    url: &Url,
    headers_builder: impl FnOnce(&mut HeaderMap),
    client: &Client,
    cache: &impl Cacher,
    tracker: &impl Tracker,

    process: u64,//进度
    shared_process: Option<NonNull<AtomicU64>>,//可选的，是否使用原子变量同步
    end: Option<NonNull<u64>>,//可选的，是否提前结束

    first_response: Option<Response>,//这里假设了block是对应first_response的范围
) -> DownloadResult<()>
{
    let mut writer = cache.write_at(SeekFrom::Start(process)).await;
    loop {
        download_once(url, first_response, client, &mut writer, tracker, &mut process, end).await;
    };
        let finally_result = todo!("handing result");//handing result
        let a = panic!();
        finally_result
}


#[inline]
pub(crate) async unsafe fn download_once(
    url: &Url,
    first_response: Option<Response>,
    client: &Client,
    writer: &mut impl Writer,
    tracker: &impl Tracker,

    process: &mut u64,
    end: EndType<'_>,
) -> DownloadResult<()> 
{
    let response = match first_response {
        Some(r) => r,
        None => {
            let mut req = Request::new(Method::GET, url.clone());
            let range = match end {
                EndType::u64(end) => Range::bytes(*process..*end.as_ref()).expect("msg"),
                EndType::Atomicu64(end ) =>Range::bytes(*process..*end).expect("msg"),
                EndType::None => Range::bytes(*process..).expect("msg"),
                //Some(end) => Range::bytes(*process..*end.as_ref()).expect("msg"),
                //one => Range::bytes(*process..).expect("msg"),
            };
            req.headers_mut().typed_insert(range);
            //headers_builder(req.headers_mut());
            let response = client.execute(req).await?;
            assert!(response.status() == StatusCode::from_u16(206).unwrap());
            response
        }
    };
    let reciving_guard = ();//
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();

        let _guard = true;//block_guard(block);

        let a = match end {
            EndType::u64(end) => Some(*end.as_ref()),
            EndType::Atomicu64(end) => Some(end.load(Ordering::Acquire)),
            EndType::None => None,
        };

        if let Some(end) = a{
            if *process + chunk_size as u64 > end {
                writer.write_all(&chunk[..(end - *process) as usize]).await?;
                *process += end - *process;
                tracker.record((end - *process) as u32, *process).await;
                break;
            };
        };

        writer.write_all(chunk.as_ref()).await?;
        *process += chunk_size as u64;
        tracker.record(chunk_size as u32, *process).await;
    }
    Ok(())
}

///不可续传链接的多次下载
pub(crate) async fn download_unrangeable(
    url: &Url,
    client: &Client,
    cache: &impl Cacher,
    tracker: &impl Tracker,

    process: u64,
    shared_writed_process: Option<NonNull<AtomicU64>>,
    shared_downloaded_process: Option<NonNull<AtomicU64>>,
    end: Option<NonNull<u64>>,//很有可能是None

    first_response: Option<Response>,
){
    loop{
        let result: reqwest::Result<()> = 'inner: {
            let response = match first_response {
                Some(r) => r,
                None => {
                    client.get(url.clone())
                       .send()
                       .await?
                }
            }

            let mut stream = response.bytes_stream();
            while let Some(item) = stream.next().await{
                let chunk = item.unwrap();
                let chunk_size = chunk.len();
            }
            Ok(())
        };
    }
}


pub(crate) async unsafe fn download_once_unrangeable(
    url: &Url,
    first_response: Option<Response>,
    client: &Client,
    writer: &mut impl Writer,
    tracker: &impl Tracker,
    writed_tracker: &impl Tracker,

    process: &mut u64,
    end: Option<NonNull<u64>>
) -> DownloadResult<()> 
{
    let response = match first_response {
        Some(r) => r,
        None => {
            let mut req = Request::new(Method::GET, url.clone());
            //headers_builder(req.headers_mut());
            let response = client.execute(req).await?;
            assert!(response.status() == StatusCode::from_u16(206).unwrap());
            response
        }
    };
    let reciving_guard = ();//
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();

        let _guard = //block_guard(block);

        if let Some(end) = end{
            if *process + chunk_size as u64 > *end.as_ref() {
                writer.write_all(&chunk[..(*end.as_ref() - *process) as usize]).await?;
                *process += *end.as_ref() - *process;
                tracker.record((*end.as_ref() - *process) as u32, *process).await;
                break;
            };
        };

        writer.write_all(chunk.as_ref()).await?;
        *process += chunk_size as u64;
        tracker.record(chunk_size as u32, *process).await;
    }
    Ok(())
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
    pub fn get(&self) -> Self{
        match self.state.load(Ordering::Acquire) {
            0 => Self::Pending,
            1 => Self::Requesting{task: unsafe{self.abort_handle.assume_init_ref().clone()}},
            2 => Self::Receving{task: unsafe{self.abort_handle.assume_init_ref().clone()}},
            3 => Self::Done,
            _ => unreachable!("Invalid state")
        }
    }

    pub(crate) fn set(&self, state:Self){
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