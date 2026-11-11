//! Reading a file the user picked, or one a pasted command names, without
//! trusting its reported size: `/dev/zero` and FIFOs never end, and files
//! under `/proc` claim to be empty.

use std::path::Path;

use rustman_core::text::human_size;
use tokio::io::AsyncReadExt as _;

#[derive(Debug)]
pub enum ReadError {
    /// A directory, a device or a FIFO.
    NotAFile,
    /// At least this many bytes, past the limit.
    TooLarge(usize),
    Io(std::io::Error),
}

impl ReadError {
    /// The status line for `name` failing to read under `limit`.
    pub fn describe(&self, name: &str, limit: usize) -> String {
        match self {
            ReadError::NotAFile => format!("{name} isn't a regular file, so it can't be read."),
            ReadError::TooLarge(len) => {
                format!("{name} is {}, past the {} limit.", human_size(*len), human_size(limit))
            }
            ReadError::Io(e) => format!("Couldn't read {name}: {e}"),
        }
    }
}

/// The whole of the regular file at `path`, refusing it once it is past
/// `limit` bytes however large it claims to be.
pub async fn read_capped(path: &Path, limit: usize) -> Result<Vec<u8>, ReadError> {
    let meta = tokio::fs::metadata(path).await.map_err(ReadError::Io)?;
    if !meta.is_file() {
        return Err(ReadError::NotAFile);
    }
    let claimed = usize::try_from(meta.len()).unwrap_or(usize::MAX);
    if claimed > limit {
        return Err(ReadError::TooLarge(claimed));
    }
    let file = tokio::fs::File::open(path).await.map_err(ReadError::Io)?;
    let mut bytes = Vec::with_capacity(claimed);
    let cap = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    file.take(cap).read_to_end(&mut bytes).await.map_err(ReadError::Io)?;
    if bytes.len() > limit {
        return Err(ReadError::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn read(path: &Path, limit: usize) -> Result<Vec<u8>, ReadError> {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(read_capped(path, limit))
    }

    #[test]
    fn devices_and_directories_are_refused_unread() {
        let dir = TempDir::new("read-capped-dir");
        assert!(matches!(read(dir.path(), 16), Err(ReadError::NotAFile)));
        if cfg!(unix) {
            assert!(matches!(read(Path::new("/dev/zero"), 16), Err(ReadError::NotAFile)));
        }
    }

    #[test]
    fn a_file_past_the_limit_is_refused_and_one_within_it_is_read() {
        let dir = TempDir::new("read-capped-size");
        let path = dir.path().join("body.txt");
        std::fs::write(&path, b"0123456789").unwrap();
        assert!(matches!(read(&path, 9), Err(ReadError::TooLarge(10))));
        assert_eq!(read(&path, 10).unwrap(), b"0123456789");
    }

    #[test]
    fn a_file_that_claims_to_be_empty_is_still_bounded() {
        let status = Path::new("/proc/self/status");
        if status.is_file() {
            assert!(matches!(read(status, 1), Err(ReadError::TooLarge(2))));
        }
    }
}
