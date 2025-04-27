use std::{mem::{MaybeUninit},sync::{atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering}}};
use headers::{self,
    HeaderMapExt,
    Range};
use reqwest::{
    self, header::{
        self, HeaderMap,HeaderValue, IF_MATCH, IF_RANGE, RANGE
    }, Client, Method, Request, Response, StatusCode, Url, Version
};
use std::sync::Arc;

use futures::stream::StreamExt;
use tokio::{sync::SemaphorePermit, task::{AbortHandle, JoinHandle, JoinSet}};

use crate::cache::{Writer, Cacher};
use crate::tracker::Tracker;
use crate::download_core::download_block;
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
    client: Client,
    process: AtomicU64,
}

struct Block{

}
impl RangeableDownloader<Cacher, Tracker>{
    async fn download_block(self:Arc<Self>, block: Block){
        download_block(
    }
}
///单线程不可续传下载器
struct UnRangeableDownloader<C, T>{
    url: Url,
    cache: C,
    tracker: T,
    client: Client,
    process: AtomicU64,
}


impl CheckType {
    fn check(&self, response: &Response) -> bool {
        match self {
            Self::ETag(etag) => {
                let etag_header = response.headers().get(header::ETAG).unwrap();
                etag_header == etag
            }
            Self::LastMotifield(last_motifield) => {
                let last_motifield_header = response.headers().get(header::LAST_MODIFIED).unwrap();
                last_motifield_header == last_motifield
            }
            Self::None => true,
        }
    }
}

impl CheckType {
    fn with_response(response: &Response) -> Self {
        if let Some(etag) = response.headers().get(header::ETAG) {
            Self::ETag(etag.clone())
        } else if let Some(last_motifield) = response.headers().get(header::LAST_MODIFIED) {
            Self::LastMotifield(last_motifield.clone())
        } else {
            Self::None
        }
    }

    fn build_if_range_requests(&self, builder: RequestBuilder) -> RequestBuilder {
        match self {
            Self::ETag(etag) => {
                builder.header(IF_RANGE, etag.clone())
            }
            Self::LastMotifield(last_motifield) => {
                builder.header(IF_RANGE, last_motifield.clone())
            }
            Self::None => builder,
        }
    }
}


