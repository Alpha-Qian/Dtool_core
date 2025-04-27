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
use crate::error::DownloadError;
use tokio::sync::watch::channel;
use parking_lot::Mutex;

struct ResponseRange{
    response: Option<Response>,
    process: u64,
    end: Option<u64>,
}

enum EndType<'a>{
    u64(NonNull<u64>),
    Atomicu64(&'a AtomicU64),
    None,
}

type RequestBuilder = fn(&mut HeaderMap, &mut Option<Duration>, &mut Option<Version>);
type ResponseCheker = fn(&Response) -> Result<(), DownloadError>;
type ResultHander = fn(reqwest::Result<()>) -> DownloadResult<()>;
type SyncEnd = fn() -> Option<u64>;
type Track = FnMut(u32, u64) -> JoinHandle<()>;

impl Tracker for Track {
    fn record(&self, len: u32, process: u64) -> impl Future<Output = ()> {
        self(len, process)
    }
}
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
    end: EndType<'_>,//可选的，是否提前结束

    first_response: Option<Response>,//这里假设了block是对应first_response的范围
) -> DownloadResult<()>
{
    let mut writer = cache.write_at(SeekFrom::Start(process)).await;
    loop {
        //use std::ops::ControlFlow;

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

    process: impl ProcessSync,
    end: impl EndSync,
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
pub(crate) async unsafe fn download_unrangeable(
    url: &Url,
    client: &Client,
    cache: &impl Cacher,
    tracker: &impl Tracker,

    process: u64,
    end: Option<NonNull<u64>>,//很有可能是None

    first_response: Option<Response>,

){
    let mut writer = cache.write_at(SeekFrom::Start(process)).await;
    loop{
        let result = download_once_unrangeable(url, first_response, client, &mut writer, tracker, &mut process, end).await;

    }
}


pub(crate) async unsafe fn download_once_unrangeable(
    url: &Url,
    first_response: Option<Response>,
    client: &Client,
    writer: &mut impl Writer,
    tracker: &impl Tracker,
    writed_tracker: &impl Tracker,

    write_process: &mut u64,
    end: Option<NonNull<u64>>,

    // build_request: impl FnOnce(&mut HeaderMap, &mut Option<Duration>, &mut Option<Version>),
    // check_response: impl FnOnce(&Response) -> Result<CheckType, DownloadError>,
    // reciving_guard: impl FnOnce() -> impl Future<Output=Result<()>>,
    // process_guard: impl FnOnce() -> impl Future<Output=Result<()>>,
) -> DownloadResult<()> 
{
    let response = match first_response {
        Some(r) => r,
        None => {
            let mut req = Request::new(Method::GET, url.clone());
            //build request here
            //headers_builder(req.headers_mut());
            let response = client.execute(req).await?;
            assert!(response.status() == StatusCode::from_u16(206).unwrap());
            //check response here
            response
        }
    };

    let mut stream = response.bytes_stream();
    let mut download_process = 0;
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();

        let _guard = //block_guard(block);

        download_process += chunk_size as u64;
        if download_process > *write_process {
            writed_tracker.record(chunk.len() as u32, *write_process).await;
            writer.write_all(&chunk[..(download_process - *write_process) as usize]).await?;
            *write_process = download_process;
            break;
        }
        writed_tracker.record(chunk_size as u32, *write_process).await;
    }

    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();
        if 
    }


    Ok(())
}


enum CheckType{
    ETag(HeaderValue),
    LastMotifield(HeaderValue),
    None,
}




fn deafult_build_request(headers:&mut HeaderMap, duration: &mut Option<Duration>, version: &mut Option<Version>){
    *duration = Some(Duration::from_secs(30));
}
// trait download_once{
//     type reciving_guard;
//     type process_guard;
//     fn build_request(&mut self, headers:&mut HeaderMap, duration: &mut Option<Duration>, version: &mut Option<Version>){
//         *duration = Some(Duration::from_secs(30));
//     }
//     fn check_response(&self, response: &Response) -> Result<CheckType, DownloadError>{
//         match response.headers().get(header::ETAG){
//             Some(etag) => Ok(CheckType::ETag(etag.clone())),
//             None => match response.headers().get(header::LAST_MODIFIED){
//                 Some(last_modified) => Ok(CheckType::LastMotifield(last_modified.clone())),
//                 None => Ok(CheckType::None),
//             }
//         }
//     }

//     async fn reciving_guard() -> Self::reciving_guard;

//     async fn process_guard() -> Self::process_guard;
// }

// trait EndSync{
//     fn get_end(&self) -> Option<u64>;
//     fn get_end_without_lock(&self) -> Option<u64>{
//         self.get_end()
//     }
// }

trait ProcessSync{
    async fn fetch_add(&mut self, len: u32);
    async fn get_process(&self) -> u64;
}

trait EndSync{
    async fn get_end(&self) -> Option<u64>;
}

#[cfg(test)]
mod tests{
    #[test]
    fn test() {

        println!("hello world");
    }
}