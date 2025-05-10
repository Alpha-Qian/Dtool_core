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
use bytes::Bytes;

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



#[inline]
pub(crate) async unsafe fn download_once(

    stream: &mut impl StreamExt<Item = Result<Bytes, reqwest::Error>>,
    writer: &mut impl Writer,

    process_sync: &mut impl ProcessSync,
    end_sync: &mut impl EndSync,
) -> DownloadResult<()> 
{
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();

        if let Some(end) = end_sync.get_end().await {
            let process = process_sync.get_process().await;
            if process + chunk_size as u64 > end {
                writer.write_all(&chunk[..(end - process) as usize]).await?;
                process_sync.fetch_add((end - process) as u32).await;
                break;
            };
        };

        writer.write_all(chunk.as_ref()).await?;
        process_sync.fetch_add(chunk_size as u32).await;
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
    end: &mut impl EndSync,

    first_response: Option<Response>,

){
    let mut writer = cache.write_at(SeekFrom::Start(process)).await;
    loop{
    }
}


pub(crate) async unsafe fn download_once_unrangeable(
    url: &Url,
    first_response: Option<Response>,
    client: &Client,
    writer: &mut impl Writer,

    download_process_sync: &mut impl ProcessSync,
    writed_process_sync: &mut impl ProcessSync,//?
    end_sync: &mut impl EndSync,

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
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();
    }

    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();
        if 
    }


    Ok(())
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


struct RequestSender<'a>{
    url: &'a Url,
    client: &'a Client,
    request_builder: RequestBuilder,
    response_checker: ResponseCheker,
    response: Option<Response>,
}

impl<'a> RequestSender<'a>{
    async fn send_request(&mut self) -> Result<Response, DownloadError>{
        let mut req = Request::new(Method::GET, self.url.clone());
        (self.request_builder)(&mut req.headers_mut(), &mut None, &mut None);
        let response = self.client.execute(req).await?;
        self.response_checker(&response)?;
    }

    async fn get_first_response(&client: &Client, url: &Url, request_builder: RequestBuilder, response_checker: ResponseCheker) -> Self{
        

    }

}
#[cfg(test)]
mod tests{
    #[test]
    fn test() {

        println!("hello world");
    }
}