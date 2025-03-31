

///etag和last_modified通常只需要一个就行
enum CheckKind{
    etag(String),
    last_modified(String),
    none,
}