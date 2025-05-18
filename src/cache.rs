use std::io::SeekFrom;

pub trait Cacher {
    type Write : Writer;
    async fn write_at(&self, pos:SeekFrom) -> Self::Write;
}

pub trait Writer {
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()>;
}

// pub trait Reader {
//     async fn down_read(&mut self, buf: &mut [u8]) -> std::io::Result<()>;
// }

// struct StdIo<T>(T);
// struct TokioIo<T>(T);
// struct AsyncstdIo<T>(T);

// impl<T:Write> Writer for StdIo<T> {
//     async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
//         self.0.write_all(buf)
//     }
// }

// impl<T:AsyncWriteExt + std::marker::Unpin> Writer for TokioIo<T> {
//     async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
//         self.0.write_all(buf).await
//     }
// }


// impl<T:Read> Reader for StdIo<T>  {
//     async fn down_read(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
//         self.0.read_exact(buf)
//     }
// }

// impl Reader for TokioIo<tokio::fs::File> {
//     async fn down_read(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
//         self.0.read_exact(buf).await;
//         Ok(())
//     }
// }


// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::io::Cursor;
//     use tokio::runtime::Runtime; // Added for async tests

//     // --- Writer Tests ---

//     #[test]
//     fn test_stdio_writer() {
//         // Use Cursor<Vec<u8>> as a mock Write object
//         let mut buffer = Cursor::new(Vec::new());
//         let mut stdio_writer = StdIo(&mut buffer);
//         let data_to_write = b"hello stdio";

//         // down_write is async, but StdIo<T: Write> implementation is effectively sync
//         // We still need an async context to call it.
//         let rt = Runtime::new().unwrap();
//         rt.block_on(async {
//             let result = stdio_writer.write_all(data_to_write).await;
//             assert!(result.is_ok());
//         });

//         // Verify the data was written
//         assert_eq!(buffer.get_ref().as_slice(), data_to_write);
//     }

//     #[tokio::test] // Use tokio::test for async tests
//     async fn test_tokio_writer() {
//         // Use Vec<u8> as a mock AsyncWriteExt object
//         let mut buffer = Vec::new();
//         // TokioIo needs the inner type to be Unpin. Vec<u8> is Unpin.
//         let mut tokio_writer = TokioIo(&mut buffer);
//         let data_to_write = b"hello tokio";

//         let result = tokio_writer.write_all(data_to_write).await;
//         assert!(result.is_ok());

//         // Verify the data was written
//         assert_eq!(buffer.as_slice(), data_to_write);
//     }

    // // --- Reader Tests ---

    // #[test]
    // fn test_stdio_reader() {
    //     let data_to_read = b"read me stdio";
    //     let mut buffer = Cursor::new(data_to_read.to_vec());
    //     let mut stdio_reader = StdIo(&mut buffer);
    //     let mut read_buf = vec![0u8; data_to_read.len()];

    //     let rt = Runtime::new().unwrap();
    //     rt.block_on(async {
    //         let result = stdio_reader.down_read(&mut read_buf).await;
    //          // StdIo<T: Read> reads exactly the buffer size or errors
    //         assert!(result.is_ok());
    //     });

    //     assert_eq!(read_buf.as_slice(), data_to_read);
    // }

//     // TokioIo<tokio::fs::File> Reader test requires a file.
//     // We'll use a temporary file.
//     use tempfile::NamedTempFile; // Add dependency in Cargo.toml: tempfile = "3"
//     use tokio::fs::OpenOptions;

//     #[tokio::test]
//     async fn test_tokio_reader() {
//         let data_to_read = b"read me tokio file";
//         // Create a temporary file and write data to it
//         let temp_file = NamedTempFile::new().expect("Failed to create temp file");
//         {
//             let mut file = std::fs::File::create(temp_file.path()).expect("Failed to create file");
//             file.write_all(data_to_read).expect("Failed to write to temp file");
//         } // File is closed here

//         // Open the temp file with Tokio for async reading
//         let mut tokio_file = OpenOptions::new()
//             .read(true)
//             .open(temp_file.path())
//             .await
//             .expect("Failed to open temp file with tokio");

//         let mut tokio_reader = TokioIo(tokio_file);
//         let mut read_buf = vec![0u8; data_to_read.len()];

//         let result = tokio_reader.down_read(&mut read_buf).await;
//         assert!(result.is_ok());
//         assert_eq!(read_buf.as_slice(), data_to_read);

//         // The temp file is automatically deleted when temp_file goes out of scope
//     }


//     // Placeholder for Cacher tests if needed later
//     // #[test]
//     // fn test_cacher_example() {
//     //     // ... Cacher trait tests would go here ...
//     // }

//     // Original placeholder test (can be removed or kept)
//     #[test]
//     fn test() {
//         println!("test placeholder"); // Updated message
//     }
// }
