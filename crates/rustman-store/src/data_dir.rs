//! The app's data directory: one writer at a time, readable only by its
//! owner, and never written by a build that doesn't know its schema.

use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, ffi};

use crate::schema;

const LOCK_FILE: &str = "rustman.lock";

/// Why this process may read the app's data but not write it. Its text is
/// for a banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnly {
    /// Another Rustman holds the data directory.
    InUse,
    /// The database was last written by a newer Rustman, whose data this
    /// build could damage.
    NewerSchema,
}

impl fmt::Display for ReadOnly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InUse => {
                "Rustman is already open in another window, so changes made here aren't saved."
            }
            Self::NewerSchema => {
                "This data was saved by a newer Rustman, so changes made here aren't saved. Update Rustman to edit it."
            }
        })
    }
}

/// The app's database for this process, and the lock that makes it the only
/// writer. Keep `lock` for as long as the connection is in use.
#[derive(Debug)]
pub struct AppData {
    pub conn: Connection,
    /// `None` when this process may write.
    pub read_only: Option<ReadOnly>,
    pub lock: DataDirLock,
}

/// This process's claim on the data directory, released when dropped.
#[derive(Debug)]
pub struct DataDirLock {
    _file: Option<File>,
}

/// Opens the app's data in `data_dir`, creating it if needed: read-write
/// when no other Rustman holds the directory and the schema is one this
/// build knows, read-only otherwise.
pub fn open_app_data(data_dir: &Path) -> rusqlite::Result<AppData> {
    create_private_dir(data_dir).map_err(|e| fs_error("create", data_dir, &e))?;
    let Some(lock) = lock(data_dir).map_err(|e| fs_error("lock", data_dir, &e))? else {
        return Ok(AppData {
            conn: open_read_only(data_dir)?,
            read_only: Some(ReadOnly::InUse),
            lock: DataDirLock { _file: None },
        });
    };
    let conn = schema::open(data_dir)?;
    if schema::is_newer(&conn)? {
        drop(conn);
        return Ok(AppData {
            conn: open_read_only(data_dir)?,
            read_only: Some(ReadOnly::NewerSchema),
            lock,
        });
    }
    Ok(AppData {
        conn,
        read_only: None,
        lock,
    })
}

/// The directory's lock, or `None` when another process holds it. A file
/// system without locks can't tell, and is treated as unshared.
fn lock(data_dir: &Path) -> io::Result<Option<DataDirLock>> {
    let file = private_file(&data_dir.join(LOCK_FILE))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(DataDirLock { _file: Some(file) })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => {
            eprintln!("rustman: cannot lock {}: {e}", data_dir.display());
            Ok(Some(DataDirLock { _file: None }))
        }
    }
}

fn open_read_only(data_dir: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        data_dir.join(schema::DB_FILE),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

/// Creates `dir` readable by its owner only, when it doesn't exist yet.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    restrict(dir, 0o700)
}

/// Opens `path` for writing, creating it readable by its owner only.
fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// Limits `path` to its owner.
#[cfg(unix)]
pub(crate) fn restrict(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

/// Windows profile directories are already private to their owner.
#[cfg(not(unix))]
pub(crate) fn restrict(_: &Path, _: u32) -> io::Result<()> {
    Ok(())
}

/// A file system failure on `path` as the error opening the database
/// reports, e.g. "cannot create /x: Permission denied".
pub(crate) fn fs_error(action: &str, path: &Path, error: &io::Error) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_CANTOPEN),
        Some(format!("cannot {action} {}: {error}", path.display())),
    )
}
