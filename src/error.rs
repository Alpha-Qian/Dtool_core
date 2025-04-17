use thiserror::Error;
use reqwest::Response;


#[derive(Error, Debug)]
enum DownloadError {
    #[error("")]
    ReqwestEorror(#[from] reqwest::Error),
    #[error("")]
    FileChangedError(Response),//使用if-range请求头，返回状态码为200的响应，而不是206
    #[error("")]
    Other,
}