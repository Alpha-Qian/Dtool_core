use std::ops::Deref;
use std::sync::atomic::{AtomicU8, Ordering, AtomicU64};
use tokio::task::{JoinHandle, AbortHandle};
use std::io::{Read, Seek, SeekFrom};
use crate::cache::Cacher;
use reqwest::Response;
use crate::downloaders::downloader;


///保存单个下载块的信息 or
///表示单个线程的下载状态,无论是单线程下载还是多线程下载
///可用于多线程和单线程
pub struct Block{
    pub(crate) start: u64,
    pub(crate) process: AtomicU64,
    pub(crate) end: u64,
    pub(crate) state: AtomBlockState,
}


impl Block{

    pub fn new(start: u64, process: u64, end: u64) -> Self{
        Block{
            start,
            process: AtomicU64::new(process),
            end,
            state: AtomBlockState{state:AtomicU8::new(0)},
        }
    }

    ///获取运行guard
    pub(crate) fn get_guard(&self, downloader) -> BlockGuard<'_> {
        BlockGuard::new(&self)
    }

    pub fn get_process(&self) -> u64 {
        self.process.load(Ordering::Acquire)
    }

    pub fn get_end(&self) -> u64 {
        self.end
    }

    pub fn get_state(&self) -> BlockState {
        self.state.get()
    }
    pub(crate) fn set_state(&self, state: BlockState) {
        self.state.set(state)
    }

    pub fn is_running(&self) -> bool {
        self.state.running()
    }

    pub(crate) fn process_done(&self) -> bool{
        if self.process.load(Ordering::Acquire) == self.end.load(Ordering::Acquire){
            true
        } else {
            false
        }
    }
}


pub(crate) struct BlockGuard{
    block:&'static Block,
}

impl BlockGuard {
    fn new(block:Box<Block>) -> Self{
        let block = block.as_ref() as *const Block;
        debug_assert!(!block.is_running());
        block.set_state(BlockState::Receving);
        BlockGuard{block}
    }
}
impl Drop for BlockGuard {
    fn drop(&mut self) {
        let block = unsafe{&*self.block};
        debug_assert!(self.block.is_running());
        if self.block.process_done(){
            self.block.set_state(BlockState::Done);
        } else {
            self.block.set_state(BlockState::Pending);
        }
    }
}

impl Deref for BlockGuard {
    type Target = Block;
    fn deref(&self) -> &Self::Target {
        unsafe {
            &*self.block
        }
    }
}

enum BlockState {
    Pending,//待办
    Requesting,//正在发送get请求
    Receving,//正在接收数据
    Done,//已完成
}

struct AtomBlockState{
    pub(crate) state:AtomicU8
}

impl AtomBlockState {

    pub fn get(&self) -> BlockState{
        match self.state.load(Ordering::Acquire) {
            0 => BlockState::Pending,
            1 => BlockState::Requesting,
            2 => BlockState::Receving,
            3 => BlockState::Done,
            _ => panic!("Invalid state"),
        }
    }

    pub(crate) fn set(&self, state:BlockState){
        let val = match state {
            BlockState::Pending => 0,
            BlockState::Requesting => 1,
            BlockState::Receving => 2,
            BlockState::Done => 3,
        };
        self.state.store(val, Ordering::Release);
    }

    pub fn running(&self) -> bool {
        match self.get(){
            BlockState::Pending|BlockState::Receving => true,
            _ => false,
        }
    }
}

pub struct StaticBlock{
    start: u64,
    process: u64,
    end: u64,
}

impl From<Block> for StaticBlock{
    fn from(block:Block) -> Self{
        StaticBlock{
            start: block.start,
            process: block.process.into_inner(),
            end: block.end,
        }
    }
}

impl From<StaticBlock> for Block{
    fn from(state_block:StaticBlock) -> Self{
        Block{
            start: state_block.start,
            process: AtomicU64::new(state_block.process),
            end: state_block.end,
            state: AtomBlockState{state:AtomicU8::new(0)},
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn helloworld(){
        print!("hello world")
    }
}