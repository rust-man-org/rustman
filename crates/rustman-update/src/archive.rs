use std::path::Path;

use crate::error::UpdateError;

/// The executable inside a release archive.
#[cfg(not(target_os = "windows"))]
const BIN_NAME: &str = "rustman";
#[cfg(target_os = "windows")]
const BIN_NAME: &str = "rustman.exe";

/// Unpacks the binary from a downloaded archive and swaps it in for the
/// running executable. Blocking: decompression and file I/O.
pub(crate) fn replace_running_binary(archive: &[u8]) -> Result<(), UpdateError> {
    let current = std::env::current_exe()?;
    let tmp = current
        .parent()
        .ok_or(UpdateError::NoExeDir)?
        .join(".rustman-update.tmp");
    extract_binary(archive, &tmp)?;
    verify_binary(&tmp)?;
    self_replace::self_replace(&tmp)?;
    // The update is in place; a leftover temp file is no reason to fail it.
    if let Err(e) = std::fs::remove_file(&tmp) {
        eprintln!("rustman: could not remove {}: {e}", tmp.display());
    }
    Ok(())
}

/// Refuses anything that doesn't start like an ELF, Mach-O or PE binary.
fn verify_binary(path: &Path) -> Result<(), UpdateError> {
    let bytes = std::fs::read(path)?;
    let magic: [u8; 4] = bytes
        .get(..4)
        .and_then(|head| head.try_into().ok())
        .ok_or(UpdateError::TooSmall)?;
    match magic {
        [0x7f, b'E', b'L', b'F']
        | [0xcf, 0xfa, 0xed, 0xfe]
        | [0xca, 0xfe, 0xba, 0xbe]
        | [0xbe, 0xba, 0xfe, 0xca]
        | [b'M', b'Z', _, _] => Ok(()),
        other => Err(UpdateError::NotABinary(other)),
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_binary(archive: &[u8], out: &Path) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;

    let mut tarball = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tarball.entries()? {
        let mut entry = entry?;
        // The Linux tarball opens with a `rustman/` directory entry, whose
        // file name is also `rustman`.
        let is_binary = entry.header().entry_type().is_file()
            && entry
                .path()
                .is_ok_and(|p| p.file_name() == Some(std::ffi::OsStr::new(BIN_NAME)));
        if is_binary {
            let mut file = std::fs::File::create(out)?;
            std::io::copy(&mut entry, &mut file)?;
            std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755))?;
            return Ok(());
        }
    }
    Err(UpdateError::NotInArchive(BIN_NAME))
}

#[cfg(target_os = "windows")]
fn extract_binary(archive: &[u8], out: &Path) -> Result<(), UpdateError> {
    let archive_error = |e: zip::result::ZipError| UpdateError::Archive(e.to_string());
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).map_err(archive_error)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(archive_error)?;
        if entry.name().rsplit(['/', '\\']).next() == Some(BIN_NAME) {
            let mut file = std::fs::File::create(out)?;
            std::io::copy(&mut entry, &mut file)?;
            return Ok(());
        }
    }
    Err(UpdateError::NotInArchive(BIN_NAME))
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use std::io::Write;

    use super::*;

    fn tarball(name: &str, content: &[u8]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, content).unwrap();
        let tar = builder.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&tar).unwrap();
        gz.finish().unwrap()
    }

    /// A file path of the test's own under the temp dir, removed on drop so a
    /// failing test leaves nothing behind.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            Self(std::env::temp_dir().join(format!("rustman-update-{name}-{}", std::process::id())))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn the_binary_is_found_anywhere_in_the_tarball() {
        let out = Scratch::new("found");
        let archive = tarball("rustman/rustman", b"\x7fELF and the rest");
        extract_binary(&archive, &out.0).unwrap();
        verify_binary(&out.0).unwrap();
        assert_eq!(std::fs::read(&out.0).unwrap(), b"\x7fELF and the rest");
    }

    /// Laid out the way release.yml packs Linux builds: the `rustman/`
    /// directory entry first, then the binary inside it.
    #[test]
    fn the_binary_is_found_after_its_directory_entry() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut dir = tar::Header::new_gnu();
        dir.set_entry_type(tar::EntryType::Directory);
        dir.set_size(0);
        dir.set_mode(0o755);
        builder
            .append_data(&mut dir, "rustman/", std::io::empty())
            .unwrap();
        let binary = b"\x7fELF and the rest";
        let mut file = tar::Header::new_gnu();
        file.set_size(binary.len() as u64);
        file.set_mode(0o755);
        builder
            .append_data(&mut file, "rustman/rustman", &binary[..])
            .unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&builder.into_inner().unwrap()).unwrap();

        let out = Scratch::new("after-dir");
        extract_binary(&gz.finish().unwrap(), &out.0).unwrap();
        assert_eq!(std::fs::read(&out.0).unwrap(), binary);
    }

    #[test]
    fn an_archive_without_the_binary_is_an_error() {
        let out = Scratch::new("missing");
        let err = extract_binary(&tarball("rustman/README", b"hi"), &out.0).unwrap_err();
        assert_eq!(err.to_string(), "'rustman' not found in archive");
    }

    #[test]
    fn non_binaries_are_refused() {
        let out = Scratch::new("refused");
        std::fs::write(&out.0, b"<html>").unwrap();
        assert!(matches!(
            verify_binary(&out.0),
            Err(UpdateError::NotABinary(_))
        ));
        std::fs::write(&out.0, b"MZ").unwrap();
        assert!(matches!(verify_binary(&out.0), Err(UpdateError::TooSmall)));
    }
}
