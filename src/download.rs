use std::sync::atomic::{AtomicU64, Ordering};
use reqwest::{self, Request};
use reqwest::Url;
use reqwest::{Client,ClientBuilder};
use reqwest::{Response, RequestBuilder};
use reqwest::header::{Range, ACCEPT_RANGES};
use thiserror;
use crate::blocks::Block;
use crate::cache::Cacher;


use crate::blocks;


pub async fn send_first_request(url: String, client: Client, headers: reqwest::header::HeaderMap) -> Result<UrlKind, reqwest::Error> {
    let range = Range::bytes(0..);
    let request = client.get(&url).header(RANGE, range).headers(headers).build()?;
    let response = client.execute(request).await?;
    if response.status().is_success() {
        Ok(UrlKind::RangeAble(response))
    }
}
 

///发送带有Range的请求，会检查响应码
async fn send_range_reqwest(builder:RequestBuilder, start: u64, end: u64) -> Result<Response, reqwest::Error>{
    let range = format!("bytes={}-{}", start, end);
    let response = builder
        .header(reqwest::header::RANGE, range)
        .send()
        .await?
        .error_for_status()?;
    Ok(response)
}


enum UrlKind {
    ///可续传链接
    RangeAble(RangeAbleBuilder),
    ///不可续传链接
    UnRangeAble(UnRangeAbleBuilder),
}

enum AllKind {
    EnsureKind(UrlKind),
    ///无法确定返回链接是否可续传，但通常是可续传的。
    /// 通过在请求中添加Range头部来避免返回此变体
    UnsureKind(reqwest::Response),
}
impl UrlKind {
    fn headers(&self) -> &reqwest::header::HeaderMap {
        match self {
            UrlKind::RangeAble(builder) => builder.as_ref().headers(),
            UrlKind::UnRangeAble(builder) => builder.as_ref().headers(),
        }
    }
}


///存在问题
use std::ops::Deref;
impl Deref for UrlKind {
    type Target = Response;

    fn deref(&self) -> &Self::Target {
        match self {
            UrlKind::RangeAble(builder) => builder.as_ref(),
            UrlKind::UnRangeAble(builder) => builder.as_ref(),
        }
    }
}

///类型包装器
struct RangeAbleBuilder(Response);
struct UnRangeAbleBuilder(Response);

impl AsRef<Response> for RangeAbleBuilder {
    fn as_ref(&self) -> &Response {
        &self.0
    }
}

impl AsRef<Response> for UnRangeAbleBuilder {
    fn as_ref(&self) -> &Response {
        &self.0
    }
}


impl RangeAbleBuilder {
    pub fn download() -> SimpDownloader{}
    pub fn multi_thread_download(){}
}


impl UnRangeAbleBuilder {
    pub fn download() -> SimpDownloader{}
}

struct SimpDownloader<C> {
    url: String,
    process: AtomicU64,
    length: Option<u64>,
    cache: C,
}

///单线程可续传
struct single_thread_downloader<C> {//感觉这个实现要好些
    url: String,
    block: blocks::Block,
    writed: u64,///已写入的字节数
    cache: C,
}


fn parse_frist_response(response: Response) -> UrlKind {
    if let Some(range) = response.headers().get(Range) {
        range
    }
}


impl<C: Cacher> SimpDownloader<C> {

}

///单线程不可续传
struct SimpleDownloader<C> {
    url: String,
    block: Block,
    cache: C,
}