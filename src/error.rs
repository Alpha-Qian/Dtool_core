use thiserror::Error;
use reqwest::Response;



#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("")]
    ReqwestEorror(#[from] reqwest::Error),
    #[error("写入相关错误{:?}",.0)]
    WriteError(#[from] WriteError<T>),
    #[error("不是范围请求")]
    NoPartialResponseError,
    #[error("")]
    FileChangedError(#[from] FileChangedError),
}

///
#[derive(Debug)]
struct WriteError<T>{
    inner: T
}