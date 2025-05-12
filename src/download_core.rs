//use std::io::SeekFrom;
use std::{future::Future, io::SeekFrom, mem::{transmute,MaybeUninit}, os::windows::process, sync::{atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering}, Arc}, time::Duration};
use std::ops::ControlFlow;
use std::ptr::NonNull;

use futures::stream::StreamExt;
use bytes::Bytes;
use crate::cache::{Writer, Cacher};
use crate::error::DownloadError;
type DownloadResult<T> = Result<T, DownloadError>;

type bufstream<A: StreamExt<Item = Result<impl AsRef<[u8]>, E>> + std::marker::Unpin, E: Into<DownloadError>> = A;

#[inline]
pub(crate) async unsafe fn download_once(
    stream: &mut (impl StreamExt<Item = Result<impl AsRef<[u8]>, impl Into<DownloadError>>> + std::marker::Unpin),
    writer: &mut impl Writer,
    process_sync: &mut impl ProcessSync,
    end_sync: &mut impl EndSync,
) -> DownloadResult<()>
{
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let control_flow = write_once(chunk.as_ref(), writer, process_sync, end_sync).await?;
        if let ControlFlow::Break(_) = control_flow {
            return Ok(());
        }
    }
    Ok(())
}


#[inline]
pub(crate) async fn write_once(
    chunk: &[u8],
    writer: &mut impl Writer,
    process_sync: &mut impl ProcessSync,
    end_sync: &mut impl EndSync,
) -> DownloadResult<ControlFlow<()>>
{
    if let Some(end) = end_sync.get_end().await{ 
        let process = process_sync.get_process().await;
        if process + chunk.len() as u64 > end {
            writer.write_all(&chunk[..(end - process) as usize]).await?;
            process_sync.fetch_add((end - process) as u32).await;
            return Ok(ControlFlow::Break(()));
        };
    };

    writer.write_all(chunk.as_ref()).await?;
    process_sync.fetch_add(chunk.len() as u32).await;
    Ok(ControlFlow::Continue(()))
}

#[inline]
pub async fn jump_to_write_position(
    stream: &mut (impl StreamExt<Item = Result<Bytes, reqwest::Error>> + std::marker::Unpin),
    cacher: &mut impl Cacher,
    process_sync: &mut impl ProcessSync,
    jump_to: u64,
) -> DownloadResult<impl Writer>{
    while let Some(item) = stream.next().await{
        let chunk = item?;
        let chunk_size = chunk.len();
        process_sync.fetch_add(chunk_size as u32).await;
        let process = process_sync.get_process().await;
        if process > jump_to {
            let writer = cacher.write_at(SeekFrom::Start(jump_to)).await;
            writer.write_all(&chunk[(jump_to - process) as usize..]).await?;
            return Ok(writer);
        }
    };
    Ok(())
}

pub async fn unrangeable_download_once(
    stream: &mut (impl StreamExt<Item = Result<Bytes, reqwest::Error>> + std::marker::Unpin),
    jump_to: u64,
    cacher: &mut impl Cacher,
    process_sync: &mut impl ProcessSync,
) -> DownloadResult<()> {
    let writer = jump_to_write_position(stream, cacher, process_sync, jump_to).await?;
    download_once(stream, writer, process_sync, end_sync).await?;
}


trait ProcessSync{//ProcessSender
    async fn fetch_add(&mut self, len: u32);
    async fn get_process(&self) -> Option<u64>;
    async fn get_unwarp(&self) -> u64{
        self.get_process().unwarp()
    }
}

trait EndSync{//EndRecivee
    async fn get_end(&self) -> Option<u64>{
        None
    }
    
    async fn get_unwarp(&self) -> u64{
        self.get_end().unwarp()
    }
}

#[cfg(test)]
mod tests{
    #[test]
    fn test() {
        println!("hello world");
    }
}
