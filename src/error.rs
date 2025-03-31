use thiserror::Error;
use reqwest::Response;


#[derive(Error, Debug)]
enum DownloadError {
    ReqwestEorror(#[from] reqwest::Error),
    FileChangedError(Response),//使用if-range请求头，返回状态码为200的响应，而不是206
    Other,
}