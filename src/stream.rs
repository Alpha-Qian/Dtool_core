use futures_util::StreamExt; // 确保导入了正确的 StreamExt
use std::marker::Unpin;

// (在这里或您的 crate 中定义 DownloadError)
// pub struct DownloadError;
// impl From<std::io::Error> for DownloadError { /* ... */ }


// 1. 定义一个辅助 trait 来描述对 StreamExt::Item 的约束
trait ValidStreamItem {
    type Bytes: AsRef<[u8]>;
    type Error: Into<DownloadError>;
}

// 2. 为符合条件的 Result<B, E> 实现这个辅助 trait
impl<B, E> ValidStreamItem for Result<B, E>
where
    B: AsRef<[u8]>,
    E: Into<DownloadError>,
{
    type Bytes = B;
    type Error = E;
}

// 3. 定义类型别名 bufstream，它只对 A 泛型
//    使用 where 子句和辅助 trait 来约束 A::Item
pub type bufstream<A>
where
    A: StreamExt + Unpin,
    A::Item: ValidStreamItem, // 关键：A的Item类型必须满足ValidStreamItem的约束
= A;

pub(crate) macro_rules! impl_stream{
    () => {
        impl (StreamExt<Item = Result<impl AsRef<[u8]>, impl Into<DownloadError>>> + Unpin)
    };
}
