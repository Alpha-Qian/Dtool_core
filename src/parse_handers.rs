///平台中立的http响应头解析
///copy from https://crates.io/crates/headers


use headers::{Header, ContentDisposition, ContentLength};



pub fn parse_content_disposition (content_disposition:&str) -> Result<(String, Option<String>), String>{
    // 示例格式：attachment; filename="example.txt"
    let parts: Vec<&str> = content_disposition.split(';').collect();
    if parts.len() < 1 {
        return Err("Invalid Content-Disposition format".to_string());
    }
    let disposition = parts[0].to_string();
    let mut filename = None;
    for part in parts.iter().skip(1) {
        let key_value: Vec<&str> = part.split('=').collect();
        if key_value.len() != 2 {
            return Err("Invalid key/value format in Content-Disposition".to_string());
        }
        let key = key_value[0].trim();
        let value = key_value[1].trim();
        if key == "filename" {
            filename = Some(value.to_string());
        }
    }
    return Ok((disposition, filename));
}

pub fn get_filename_from_url(url: &str) -> Option<&str> {
    url.rsplit_once('/').map(|(_, filename)| filename)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContentRange {
    /// First and last bytes of the range, omitted if request could not be
    /// satisfied
    pub range: Option<(u64, u64)>,

    /// Total length of the instance, can be omitted if unknown
    pub complete_length: Option<u64>,
}

///格式示例
///Content-Range: <unit> <range-start>-<range-end>/<size>
///Content-Range: <unit> <range-start>-<range-end>/*
///Content-Range: <unit> */<size>
/// Content-Range: bytes 200-1000/67589
pub fn parse_content_range(header: &str) -> Option<ContentRange> {
    let (unit,spec) = split_in_two(header, ' ')?;
    if unit != "bytes" {
        return None;
    }

    let (range, complete_length) = split_in_two(spec, '/')?;
    
    let complete_length = if complete_length == "*" {
        None
    } else {
        Some(complete_length.parse::<u64>().ok()?)
    };

    let range = if range == "*" {
        None
    } else {
        let (first_byte, last_byte) = split_in_two(range, '-')?;
        let first_byte  = first_byte.parse().ok()?;
        let last_byte = last_byte.parse::<u64>().ok()?;
        if last_byte < first_byte {
            return None;
        }
        Some((first_byte, last_byte))
    };
    Some(ContentRange { range , complete_length})
}


pub fn parse_content_length(header: &str) -> Option<u64> {
    let length = header.parse().ok()?;
    Some(length)
}

fn split_in_two(s: &str, separator: char) -> Option<(&str, &str)> {
    let mut iter = s.splitn(2, separator);
    match (iter.next(), iter.next()) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}
