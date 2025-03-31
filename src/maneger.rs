struct manager{
    urlinfo: Urlinfo,
    client: reqwest::Client,
    cache: Cache,
}