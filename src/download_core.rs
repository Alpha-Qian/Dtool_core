//use std::io::SeekFrom;
use std::{future::Future, io::SeekFrom, mem::{transmute,MaybeUninit}, os::windows::process, sync::{atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering}, Arc}, time::Duration};
use std::ops::ControlFlow;
use std::ptr::NonNull;
use headers::{self,
    HeaderMapExt,
    Range};
use reqwest::{
    self, header::{
        self, HeaderMap,HeaderValue, IF_MATCH, IF_RANGE, RANGE
    }, Client, Method, Request, Response, StatusCode, Url, Version
};

use futures::stream::{self, StreamExt};
use bytes::Bytes;

use tokio::{sync::SemaphorePermit, task::{AbortHandle, JoinHandle, JoinSet}};

use crate::cache::{Writer, Cacher};
use crate::tracker::Tracker;
use crate::error::DownloadError;

#[inline]
pub(crate) async unsafe fn download_once(

    stream: &mut (impl StreamExt<Item = Result<Bytes, reqwest::Error>> + std::marker::Unpin),
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

#[inline]
pub(crate) async fn write_once_to_end(
    chunk: &[u8],
    writer: &mut impl Writer,
    process_sync: &mut impl ProcessSync,
    end_sync: &mut impl EndSync,
) -> DownloadResult<ControlFlow<()>> 
{
    if let Some(end) = end_sync.get_end().await {
        let process = process_sync.get_process().await;
        if process + chunk_size as u64 > end {
            writer.write_all(&chunk[..(end - process) as usize]).await?;
            process_sync.fetch_add((end - process) as u32).await;
            return Ok(ControlFlow::Break(()));
        };
    };

    writer.write_all(chunk.as_ref()).await?;
    process_sync.fetch_add(chunk_size as u32).await;
    Ok(ControlFlow::Continue(()))
}

#[inline]
pub(crate) async fn jump_to_write_position(
    stream: &mut (impl StreamExt<Item = Result<Bytes, reqwest::Error>> + std::marker::Unpin),
    cacher: &mut impl Cacher,
    process_sync: &mut impl ProcessSync,
    jump_to: u64,
) -> DownloadResult<Writer>{
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();
        process_sync.fetch_add(chunk_size as u32).await;
        let process = process_sync.get_process().await;
        if process >= jump_to {
            writer = cacher.write_at(SeekFrom::Start(jump_to)).await?;
            return Ok(writer);
        }
    };
    Err(())
}
    
pub(crate) async unsafe fn download_once_unrangeable(
    stream: &mut (impl StreamExt<Item = Result<Bytes, reqwest::Error>> + std::marker::Unpin),
    writer: &mut impl Writer,

    download_process_sync: &mut impl ProcessSync,
    writed_process: &mut u64,
    end_sync: &u64,

) -> DownloadResult<()> 
{
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