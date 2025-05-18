use std::error::Error;
//use std::io::SeekFrom;
use std::{io::SeekFrom};
use std::ops::ControlFlow;
use std::ptr::NonNull;

use futures_util::stream::StreamExt;
use bytes::Bytes;
use crate::cache::{Writer, Cacher};
use crate::error::DownloadCoreError::{self, *};
use crate::stream::{bufstream};

type DownloadResult<T, I, W> = Result<T, DownloadCoreError<I, W>>;


type Cacher = FnOnce()
#[inline]
pub(crate) async fn download_once<S,B,E>(
    stream: &mut S,
    writer: &mut impl Writer,
    process_sync: &mut impl ProcessSender,
    end_sync: &mut impl EndReciver,
) -> DownloadResult<()>
where
    S: StreamExt<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: Error
{
    while let Some(item) = stream.next().await{
        let chunk = item.map_err(InternetEorror)?;
        if let ControlFlow::Break(_) = write_a_chunk(chunk.as_ref(), writer, process_sync, end_sync).await? { break }
    }
    Ok(())
}


#[inline]
pub(crate) async fn write_a_chunk(
    chunk: &[u8],
    writer: &mut impl Writer,
    process_sync: &mut impl ProcessSender,
    end_sync: &mut impl EndReciver,
) -> DownloadResult<ControlFlow<()>, I, W>
{
    if let (Some(end), Some(process)) = (end_sync.get_end().await, process_sync.get_process().await){
        //let process = process_sync.get_process().await;
        if process + chunk.len() as u64 > end {
            writer.write_all(&chunk[..(end - process) as usize]).await.map_err(op)?;
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
    process_sync: &mut impl ProcessSender,
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
    process_sync: &mut impl ProcessSender,
) -> DownloadResult<()> {
    let writer = jump_to_write_position(stream, cacher, process_sync, jump_to).await?;
    download_once(stream, writer, process_sync, end_sync).await?;
}


trait ProcessSender{//ProcessSender
    async fn fetch_add(&mut self, len: u32);
    async fn get_process(&self) -> Option<u64>;
    async fn get_unwarp(&self) -> u64{
        self.get_process().await.unwrap()
    }
}

trait EndReciver{//EndReciver
    async fn get_end(&self) -> Option<u64>{
        None
    }
    
    async fn get_unwarp(&self) -> u64{
        self.get_end().await.unwrap()
    }
}

#[cfg(test)]
mod tests{
    #[test]
    fn test() {
        println!("hello world");
    }
}
