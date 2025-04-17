use std::borrow::Cow;
use std::borrow::Borrow;

///解析reqwest的Response
use headers::ContentRange;
use reqwest::Response;
use reqwest::Client;
use reqwest::header::HeaderValue;

use reqwest::header::CONTENT_RANGE;
use crate::parse_handers;

pub trait ParseResponse {
    fn is_partial_response(&self) -> bool;
    fn accept_range(&self) -> bool;
    fn content_range(&self) -> Option<&HeaderValue>;
    fn body_size(&self) -> Option<u64>;
    fn content_range(&self) -> Option<ContentRange>;
}


enum ParseResult {
    RangeAble(u64),
    UnRangeAble(u64),
    UnknownSize,
}

impl ParseResponse for Response {


    fn is_partial_response(&self) -> bool {
        self.status() == 206
    }

    fn accept_range(&self) -> bool {
        self.is_partial_response() || self.headers().get("Accept-Ranges") == Some(&HeaderValue::from_static("bytes"))
    }

    fn content_range(&self) -> Option<&HeaderValue> {
        parse_handers::parse_content_range(self.headers().get("Content-Range"))
    }

    fn body_size(&self) -> Option<u64> {
        self.content_length().or(optb)
    }

    fn get_file_name(&self) -> Option<String> {
        Cow
    }

}


