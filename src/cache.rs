use std::io::{Read,SeekFrom, Write};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub trait Cacher {
    type Write : Writer;
    //type W: Write + Read;
    async fn with_capacity(&mut self, capacity: Option<u64>);
    async fn write_at(&self, pos:SeekFrom) -> Self::Write;
}

pub trait Writer {
    async fn down_write(&mut self, buf: &[u8]) -> std::io::Result<()>;
}

pub trait Reader {
    async fn down_read(&mut self, buf: &mut [u8]) -> std::io::Result<()>;
}

struct StdIo<T>(T);
struct TokioIo<T>(T);
struct AsyncstdIo<T>(T);

impl<T:Write> Writer for StdIo<T> {
    async fn down_write(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.0.write_all(buf)
    }
}

impl<T:AsyncWriteExt + std::marker::Unpin> Writer for TokioIo<T> {
    async fn down_write(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.0.write_all(buf).await
    }
}


impl<T:Read> Reader for StdIo<T>  {
    async fn down_read(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        self.0.read_exact(buf)
    }
}

impl Reader for TokioIo<tokio::fs::File> {
    async fn down_read(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        self.0.read_exact(buf).await;
        Ok(())
    }
}


#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn test(){
        println!("test");
    }
}
