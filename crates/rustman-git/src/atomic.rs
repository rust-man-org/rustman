use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Replaces `path` with `bytes` so that a reader, or a crash, sees either
/// the old content or the new, never a truncated file: the bytes go to a
/// sibling file first, which is then renamed over `path`.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = temp_path(path);
    let written = std::fs::File::create(&temp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    match written.and_then(|()| std::fs::rename(&temp, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Err(cleanup) = std::fs::remove_file(&temp)
                && cleanup.kind() != io::ErrorKind::NotFound
            {
                eprintln!("rustman: cannot remove {}: {cleanup}", temp.display());
            }
            Err(e)
        }
    }
}

/// `.name.tmp` beside `path`. A name fixed per file means a temp file left
/// by a crash is overwritten by the next write instead of piling up.
fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    path.with_file_name(format!(".{name}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_leaves_no_temp_file_behind() {
        let dir = rustman_testkit::TempDir::new("atomic");
        let path = dir.path().join("a.json");
        write(&path, b"one").unwrap();
        write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["a.json"]);
    }

    #[test]
    fn a_failed_write_keeps_the_old_content() {
        let dir = rustman_testkit::TempDir::new("atomic-fail");
        let path = dir.path().join("a.json");
        write(&path, b"old").unwrap();
        std::fs::create_dir(temp_path(&path)).unwrap();
        std::fs::write(temp_path(&path).join("blocker"), "").unwrap();
        assert!(
            write(&path, b"new").is_err(),
            "the temp path is a non-empty directory"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
    }
}
