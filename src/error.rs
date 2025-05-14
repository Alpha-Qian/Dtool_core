use std::result::Result;


#[derive(Debug)]
pub enum DownloadCoreError<I, W>
where
    I: std::error::Error + 'static,
    W: std::error::Error + 'static + std::fmt::Debug,
{
    InternetEorror(I),
    WriteError(W),
}

#[cfg(test)]
mod test{
    fn a(){
        Result::map_err(self, op)
    }
}