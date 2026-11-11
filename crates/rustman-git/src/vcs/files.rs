//! Collection files: `{id}.json` per collection at the top of the working
//! tree. Rustman only replaces or deletes a file it can read back as the
//! collection the file is named after, so whatever else shares the folder
//! (a `package.json`, a file from a newer version, a merge conflict) is left
//! alone.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use git2::{Oid, Repository};
use rustman_core::domain::collection::{Collection, SavedRequest, is_valid_collection_id};
use rustman_core::domain::group::{ImportedCollection, ImportedGroup};
use serde::{Deserialize, Serialize};

use super::error::GitError;
use super::open;
use crate::atomic;

/// Holds the global scripts. It sits beside the collection files and is
/// told apart from them by name.
const GLOBAL_SCRIPT_FILE: &str = "global.json";
const GLOBAL_SCRIPT_STEM: &str = "global";

#[derive(Deserialize)]
struct CollectionFile {
    collection: Collection,
    requests: Vec<SavedRequest>,
    /// Absent in repositories written before collections had folders.
    #[serde(default)]
    groups: Vec<ImportedGroup>,
}

impl From<CollectionFile> for ImportedCollection {
    fn from(file: CollectionFile) -> Self {
        Self {
            collection: file.collection,
            requests: file.requests,
            groups: file.groups,
        }
    }
}

#[derive(Serialize)]
struct CollectionFileRef<'a> {
    collection: &'a Collection,
    requests: &'a [SavedRequest],
    groups: &'a [ImportedGroup],
}

/// Read as `String`s, written from `&str`s.
#[derive(Serialize, Deserialize)]
struct GlobalScriptFile<S> {
    pre_request_script: S,
    test_script: S,
}

/// The collections read from a working tree or a commit.
#[derive(Debug, Clone, Default)]
pub struct LoadedCollections {
    pub collections: Vec<ImportedCollection>,
    /// Files named like a collection that don't hold it. A collection whose
    /// file is listed here must be kept as it is, never treated as deleted.
    pub unreadable: Vec<UnreadableFile>,
}

/// A file Rustman can't read as its own, so never overwrites or deletes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableFile {
    pub file: String,
    pub problem: FileProblem,
}

impl UnreadableFile {
    /// The collection the file is named after; `None` for the global scripts.
    pub fn collection_id(&self) -> Option<&str> {
        self.file
            .strip_suffix(".json")
            .filter(|stem| *stem != GLOBAL_SCRIPT_STEM)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileProblem {
    /// Not JSON: cut short, holding merge conflict markers, or never Rustman's.
    Malformed(String),
    /// JSON of another tool, such as a `package.json`.
    Foreign,
    /// Rustman-shaped JSON this version can't read, e.g. from a newer one.
    Unsupported(String),
    /// A collection file holding the collection with this other id.
    OtherCollection(String),
}

impl std::fmt::Display for UnreadableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let file = &self.file;
        match &self.problem {
            FileProblem::Malformed(e) => write!(f, "{file} is not valid JSON ({e})"),
            FileProblem::Foreign => write!(f, "{file} belongs to another tool"),
            FileProblem::Unsupported(e) => {
                write!(
                    f,
                    "{file} can't be read, maybe it's from a newer Rustman ({e})"
                )
            }
            FileProblem::OtherCollection(id) => write!(f, "{file} holds collection {id}"),
        }
    }
}

/// What [`write_working_tree`] left out of the working tree.
#[derive(Debug, Default)]
pub struct Skipped {
    /// Collections whose id can't name a file, so they stay out of git.
    pub invalid_ids: Vec<String>,
    /// Files in the way of a collection or the global scripts.
    pub unreadable: Vec<UnreadableFile>,
}

enum Content<T> {
    Readable(T),
    Unreadable(FileProblem),
}

/// `bytes` as the collection `id`, the only content `{id}.json` may own.
fn as_collection(id: &str, bytes: &[u8]) -> Content<CollectionFile> {
    match serde_json::from_slice::<CollectionFile>(bytes) {
        Ok(file) if file.collection.id == id => Content::Readable(file),
        Ok(file) => Content::Unreadable(FileProblem::OtherCollection(file.collection.id)),
        Err(e) => Content::Unreadable(problem(bytes, "collection", &e)),
    }
}

fn as_global_scripts(bytes: &[u8]) -> Content<GlobalScriptFile<String>> {
    match serde_json::from_slice(bytes) {
        Ok(file) => Content::Readable(file),
        Err(e) => Content::Unreadable(problem(bytes, "pre_request_script", &e)),
    }
}

/// Why `bytes` failed to deserialize. Rustman's files are JSON objects with
/// `marker` as a key.
fn problem(bytes: &[u8], marker: &str, error: &serde_json::Error) -> FileProblem {
    match serde_json::from_slice::<serde_json::Value>(bytes) {
        Err(e) => FileProblem::Malformed(e.to_string()),
        Ok(serde_json::Value::Object(map)) if map.contains_key(marker) => {
            FileProblem::Unsupported(error.to_string())
        }
        Ok(_) => FileProblem::Foreign,
    }
}

/// The collection id `path` would hold, if its name is one Rustman writes.
fn collection_stem(path: &Path) -> Option<&str> {
    let stem = path
        .file_name()?
        .to_str()?
        .strip_suffix(".json")
        .filter(|stem| *stem != GLOBAL_SCRIPT_STEM)?;
    is_valid_collection_id(stem).then_some(stem)
}

/// `{id}.json` directly in `workdir`; `None` for an id that is not a plain
/// file name, which could otherwise point anywhere on disk.
fn collection_path(workdir: &Path, id: &str) -> Option<PathBuf> {
    if id == GLOBAL_SCRIPT_STEM || !is_valid_collection_id(id) {
        return None;
    }
    let path = workdir.join(format!("{id}.json"));
    (path.parent() == Some(workdir)).then_some(path)
}

/// The collections in `dir`, which need not be a repository yet.
pub fn load_collections_from(dir: &Path) -> Result<LoadedCollections, GitError> {
    let mut out = LoadedCollections::default();
    for path in regular_files(dir)? {
        if let Some(id) = collection_stem(&path) {
            out.add(id, &std::fs::read(&path)?);
        }
    }
    Ok(out)
}

/// The collections as committed in `commit_id`.
pub fn read_commit_collections(
    data_dir: &Path,
    commit_id: &str,
) -> Result<LoadedCollections, GitError> {
    let repo = open(data_dir)?;
    let tree = repo.find_commit(Oid::from_str(commit_id)?)?.tree()?;
    let mut out = LoadedCollections::default();
    for entry in &tree {
        if let Some(id) = entry
            .name()
            .ok()
            .and_then(|n| collection_stem(Path::new(n)))
            && let Some(blob) = entry.to_object(&repo)?.as_blob()
        {
            out.add(id, blob.content());
        }
    }
    Ok(out)
}

impl LoadedCollections {
    fn add(&mut self, id: &str, bytes: &[u8]) {
        match as_collection(id, bytes) {
            Content::Readable(file) => self.collections.push(file.into()),
            Content::Unreadable(problem) => self.unreadable.push(UnreadableFile {
                file: format!("{id}.json"),
                problem,
            }),
        }
    }
}

/// The global (pre-request, test) scripts in `dir`, if saved there.
pub fn read_global_script(dir: &Path) -> Option<(String, String)> {
    let bytes = std::fs::read(dir.join(GLOBAL_SCRIPT_FILE)).ok()?;
    let Content::Readable(file) = as_global_scripts(&bytes) else {
        return None;
    };
    Some((file.pre_request_script, file.test_script))
}

/// The global (pre-request, test) scripts as committed in `commit_id`.
pub fn read_commit_global_script(data_dir: &Path, commit_id: &str) -> Option<(String, String)> {
    let repo = open(data_dir).ok()?;
    let tree = repo
        .find_commit(Oid::from_str(commit_id).ok()?)
        .ok()?
        .tree()
        .ok()?;
    let object = tree
        .get_path(Path::new(GLOBAL_SCRIPT_FILE))
        .ok()?
        .to_object(&repo)
        .ok()?;
    let Content::Readable(file) = as_global_scripts(object.as_blob()?.content()) else {
        return None;
    };
    Some((file.pre_request_script, file.test_script))
}

/// Writes `collections`, secrets blanked, and the global scripts into the
/// working tree of the existing repository at `data_dir`, and deletes the
/// files of Rustman collections no longer listed. Files Rustman can't read
/// as its own are never touched; they come back in [`Skipped`].
pub fn write_working_tree(
    data_dir: &Path,
    collections: &[ImportedCollection],
    global_pre_request_script: &str,
    global_test_script: &str,
) -> Result<Skipped, GitError> {
    let repo = open(data_dir)?;
    let workdir = workdir(&repo)?;
    let mut skipped = Skipped::default();

    let mut current = HashSet::new();
    for imported in collections {
        let id = imported.collection.id.as_str();
        let Some(path) = collection_path(workdir, id) else {
            skipped.invalid_ids.push(id.to_owned());
            continue;
        };
        current.insert(id);
        let mut stripped = imported.clone();
        rustman_core::secrets::strip_collection_secrets(&mut stripped);
        let file = CollectionFileRef {
            collection: &stripped.collection,
            requests: &stripped.requests,
            groups: &stripped.groups,
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        replace(&path, &bytes, |old| as_collection(id, old), &mut skipped)?;
    }

    for path in regular_files(workdir)? {
        if let Some(id) = collection_stem(&path)
            && !current.contains(id)
            && matches!(
                as_collection(id, &std::fs::read(&path)?),
                Content::Readable(_)
            )
        {
            std::fs::remove_file(&path)?;
        }
    }

    write_global_scripts(
        workdir,
        global_pre_request_script,
        global_test_script,
        &mut skipped,
    )?;
    Ok(skipped)
}

fn write_global_scripts(
    workdir: &Path,
    pre_request_script: &str,
    test_script: &str,
    skipped: &mut Skipped,
) -> Result<(), GitError> {
    let path = workdir.join(GLOBAL_SCRIPT_FILE);
    if !pre_request_script.is_empty() || !test_script.is_empty() {
        let file = GlobalScriptFile {
            pre_request_script,
            test_script,
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        return replace(&path, &bytes, as_global_scripts, skipped);
    }
    match std::fs::read(&path) {
        Ok(old) => match as_global_scripts(&old) {
            Content::Readable(_) => std::fs::remove_file(&path)?,
            Content::Unreadable(problem) => skipped.unreadable.push(unreadable(&path, problem)),
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Atomically writes `bytes` to `path` unless it already holds them, or
/// holds something `read` doesn't recognise as Rustman's.
fn replace<T>(
    path: &Path,
    bytes: &[u8],
    read: impl FnOnce(&[u8]) -> Content<T>,
    skipped: &mut Skipped,
) -> Result<(), GitError> {
    match std::fs::read(path) {
        Ok(old) if old == bytes => Ok(()),
        Ok(old) => match read(&old) {
            Content::Readable(_) => Ok(atomic::write(path, bytes)?),
            Content::Unreadable(problem) => {
                skipped.unreadable.push(unreadable(path, problem));
                Ok(())
            }
        },
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(atomic::write(path, bytes)?),
        Err(e) => Err(e.into()),
    }
}

fn unreadable(path: &Path, problem: FileProblem) -> UnreadableFile {
    UnreadableFile {
        file: path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        problem,
    }
}

/// The plain files directly in `dir`. Symlinks are left out: one in a
/// cloned repository could point anywhere.
fn regular_files(dir: &Path) -> Result<Vec<PathBuf>, GitError> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            out.push(entry.path());
        }
    }
    Ok(out)
}

fn workdir(repo: &Repository) -> Result<&Path, GitError> {
    repo.workdir().ok_or(GitError::Bare)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_ids_name_a_file_inside_the_working_tree() {
        let workdir = Path::new("/repo");
        assert_eq!(
            collection_path(workdir, "col-1"),
            Some(PathBuf::from("/repo/col-1.json"))
        );
        for id in ["../../x", "/etc/cron.d/x", "a/b", "..", "", "global", "con"] {
            assert_eq!(collection_path(workdir, id), None, "{id:?}");
        }
    }
}
