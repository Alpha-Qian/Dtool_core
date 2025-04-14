use std::{sync::atomic::{AtomicU64, Ordering}};


pub trait Tracker {
    async fn fetch_add(&self, len: u32);
}

pub trait TrackerBuilder{
    type Output: Tracker;
    fn build_tracker(&self) -> Self::Output;
}

struct AtomicTracker{
    len: AtomicU64,
}

impl AtomicTracker {
    pub fn new() -> Self {
        Self::new_with_len(0)
    }

    pub fn new_with_len(len: u64) -> Self {
        AtomicTracker {
            len: AtomicU64::new(len),
        }
    }
}

impl Tracker for AtomicTracker {
    async fn fetch_add(&self, len: u32) {
        self.len.fetch_add(len as u64, Ordering::Release);
    }
}

pub struct NilTrackerBuilder();

impl TrackerBuilder for NilTrackerBuilder {
    type Output = NilTracker;
    fn build_tracker(&self) -> Self::Output {
        NilTracker()
    }
}

pub struct NilTracker();

impl Tracker for NilTracker {
    async fn fetch_add(&self, len: u32) {}
}



struct TracherHList<H, T>{
    head: H,
    tail: T,
}

struct Nil();

impl Tracker for Nil {
    #[inline(always)]
    async fn fetch_add(&self, len: u32) {
        // do nothing
    }
}

impl <T: Tracker, H: Tracker> Tracker for TracherHList<T, H> {
    async fn fetch_add(&self, len: u32) {
        self.head.fetch_add(len).await;
        self.tail.fetch_add(len).await;
    }
}

macro_rules! hlist {
    // 递归终止条件：无参数时返回 HNil
    () => { HNil };

    // 匹配第一个参数 `$first`，剩余参数 `$rest`
    // 递归展开剩余参数，生成嵌套的 HCons
    ($first:expr, $($rest:expr),*) => {
        HCons {
            head: $first,
            tail: hlist!($($rest),*),
        }
    };

    // 处理单个参数（无尾部逗号的情况）
    ($single:expr) => {
        HCons {
            head: $single,
            tail: HNil,
        }
    };
}

////
/*
macro_rules! impl_tuple_for_tracker {
    // 匹配元组模式，例如 (T1, T2, T3)
    ($($t:ident),*) => {
        impl<$($t: Tracker),*> Tracker for ($($t,)*) {
            async fn fetch_add(&self, len: u32) {
                // 解构元组，依次调用方法
                let ($(ref $t,)*) = self;
                $($t.fetch_add(len).await;)*
            }
        }
    };
}*/
macro_rules! impl_process_tuple {
    // 匹配元组元素，例如 (&T1, &T2)
    ($($t:ident),*) => {
        // 生命周期 'a 确保引用有效性，T 需实现 A
        impl<'a, $($t: Tracker + 'a),*> Tracker for ($(&'a $t,)*) {
            async fn fetch_add(&self, len: u32) {
                // 解构元组，直接调用不可变引用的方法
                let ($(ref $t,)*) = *self;
                $($t.fetch_add(len).await;)*
            }
        }
    };
}


impl_process_tuple!(T1);
impl_process_tuple!(T1, T2);


///为&dyn T实现Tracker trait
impl<T: Tracker + ?Sized> Tracker for &T {
    async fn fetch_add(&self, len: u32) {
        (*self).fetch_add(len).await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let a = (1,2,3);
    }
}