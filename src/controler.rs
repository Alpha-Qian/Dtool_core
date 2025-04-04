
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cache::Cacher;
use crate::downloader::UrlDownloader;

pub trait TaskCreate {
    
    fn divition_task(&self);
    fn cancel_task(&self);

    fn division_tasks(&self, times: u32){
        for _ in 0..times {
            self.divition_task();
        }
    }
    
    fn cancel_tasks(&self, times: u32){
        for _ in 0..times {
            self.cancel_task();
        }
    }
}

struct DeafultCreater<'a, C:Cacher> {
    downloader:&'a UrlDownloader<'a, C>,

}
enum CreateTaskError {
    AllBlockDone,
    RemainProcessTooSmall,
}

enum CancelTaskError {
    NoRunningBlock,
}

type CreateTaskResult<T> = Result<T, CreateTaskError>;

impl<C:Cacher> TaskCreate for DeafultCreater<'_,C> {

    fn division_tasks(&self, times:u32){
        let mut blocks = self.info.blocks.write();//获取写锁守护
        let mut remains: Vec<u64> = Vec::with_capacity(blocks.len());
        let mut need_divition_times: Vec<usize> = Vec::with_capacity(blocks.len());
        //
        let mut all_done = true;
        for block in blocks.iter(){
            let p = block.process.load(Ordering::Relaxed);//因为有锁保护，内存顺序无所谓
            let end = block.end;
            let remain = end - p;
            remains.push(remain);
            if (remain) > 0{
                all_done = false
            }
            need_divition_times.push(if block.is_running() {1} else {0});
        }
        if all_done {return Err(CreateTaskError::AllBlockDone);}
        
        for i in 0..times{
            let mut max_remain= 0;
            //let max_block_index = None;

            let mut index = 0;
            //循环找到剩余最多的block
            for i in 0..(remains.len()){
                let remain = remains[i];
                let num = need_divition_times[i];
                let after_divition_remain = remain / ((num + 1) as u64);
                if after_divition_remain > max_remain{
                    max_remain = after_divition_remain;
                    index = i;
                }
            }
            need_divition_times[index] += 1
        }

        //创建新的block
        let mut insert_index = 0;
        for i in 0..need_divition_times.len(){
            insert_index += 1;
            let block = blocks[i].as_ref();
            if !block.is_running(){
                if need_divition_times[i] == 1{
                    self.downloader.start_block(block);
                }
            }
            if need_divition_times[i] >= 2{
                let size = (block.process.load(Ordering::Relaxed) - block.end.load(Ordering::Relaxed)) / need_divition_times[i] as u64;
                let v = Vec::with_capacity(need_divition_times[i] - 1);
                for divition_times in 0..need_divition_times[i]{
                    let start = block.start + divition_times as u64 * size;
                    let end = start + size;
                    let new_block = Box::new(Block::new(start,start, end));
                    v.push(new_block);
                    self.downloader.start_block(new_block);
                }
                blocks.splice(insert_index..insert_index, v);
                insert_index += need_divition_times[i] - 1;
            }
        }
        Ok(())
    }
    
    fn divition_task(&self) {
        self.division_tasks(1);
    }

    fn cancel_task(&self) {
        let mut blocks = self.info.blocks.write();
        let mut max_ramain = 0;
        let mut max_block_index = None;
        for (index,block) in blocks.iter().enumerate(){
            let remain = block.end.load(Ordering::Relaxed) - block.process.load(Ordering::Relaxed);
            if remain > max_ramain{
                max_ramain = remain;
                max_block_index = Some(index);
            }
        }
        if let Some(index) = max_block_index{
            let block = blocks.remove(index);
            self.downloader.cancel_block(block);
        }
    }
}

pub trait Controler {
    async fn run(&self) -> Result<(), _>;
}

struct CreateOneByOne{
    downloader: UrlDownloader<'static, Cacher>,
}

impl CreateOneByOne {
    async fn run() {
        return Ok(());
    }
}