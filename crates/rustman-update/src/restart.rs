use std::convert::Infallible;
use std::path::PathBuf;
use std::process::Command;

/// Relaunches the freshly installed executable and exits this process.
pub fn restart() -> Result<Infallible, String> {
    let exe = installed_exe().map_err(|e| e.to_string())?;
    if !exe.exists() {
        return Err(format!(
            "Binary not found at {path}. The update replaced the file but the current \
             process may be reading a stale inode. Try running {path} manually.",
            path = exe.display(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        // A downloaded binary is often quarantined, and Gatekeeper would
        // block the relaunch. Failing to clear it is not fatal: the spawn
        // below reports the real problem if there is one.
        if let Err(e) = Command::new("xattr")
            .args(["-dr", "com.apple.quarantine"])
            .arg(&exe)
            .output()
        {
            eprintln!("rustman: could not clear the quarantine flag: {e}");
        }
    }

    match Command::new(&exe).spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            let hint = if cfg!(target_os = "macos") {
                "The binary may need code-signing or the quarantine flag removed. \
                 Try: xattr -dr com.apple.quarantine "
            } else if cfg!(target_os = "linux") {
                "If you are running via cargo run, the binary was swapped but the \
                 old inode may still be cached. Try running the binary directly: "
            } else {
                ""
            };
            Err(format!(
                "Failed to restart from {path}: {e}. {hint}{path}",
                path = exe.display(),
            ))
        }
    }
}

/// The path of the executable now installed. After the swap, Linux's
/// `/proc/self/exe` still names the old, unlinked inode and appends
/// `" (deleted)"`; the new binary sits at the path without it.
fn installed_exe() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(if cfg!(target_os = "linux") {
        strip_deleted_suffix(exe)
    } else {
        exe
    })
}

fn strip_deleted_suffix(exe: PathBuf) -> PathBuf {
    match exe.to_str().and_then(|s| s.strip_suffix(" (deleted)")) {
        Some(stripped) => PathBuf::from(stripped),
        None => exe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_deleted_suffix_proc_self_exe_reports_after_the_swap() {
        let deleted = PathBuf::from("/opt/rustman/rustman (deleted)");
        assert_eq!(
            strip_deleted_suffix(deleted),
            PathBuf::from("/opt/rustman/rustman")
        );
    }

    #[test]
    fn leaves_a_normal_path_untouched() {
        let normal = PathBuf::from("/usr/local/bin/rustman");
        assert_eq!(strip_deleted_suffix(normal.clone()), normal);
    }
}
