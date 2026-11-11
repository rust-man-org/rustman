use std::path::{Path, PathBuf};

use rustman_core::domain::environment::AppEnvironment;
use rustman_core::domain::group::ImportedCollection;
use rustman_core::domain::model::Scripts;
use rustman_core::store::{SessionStore, Workspace, WorkspaceStore};
use rustman_formats::import::import_collection;
use rustman_store::SqliteStore;
use serde::Deserialize;

use crate::args::RunOptions;

/// The collections to run and the environment to run them with.
pub(crate) struct Loaded {
    pub(crate) collections: Vec<ImportedCollection>,
    /// The app's global scripts, for a collection saved in the app.
    pub(crate) global: Scripts,
    /// The `--env` environment, or an empty one, with the `--var`s set. It
    /// lives for the whole run, so what one request's scripts set, the next
    /// one reads.
    pub(crate) env: AppEnvironment,
}

pub(crate) fn load(options: &RunOptions) -> Result<Loaded, String> {
    let file = Path::new(&options.source);
    let from_file = file.is_file();
    let saved = if from_file && options.env.is_none() {
        None
    } else {
        Some(saved_data(&data_dir(options))?)
    };
    let workspace = saved.as_ref().map(|(ws, _)| ws);
    let (collections, global) = match &saved {
        Some((ws, global)) if !from_file => {
            (vec![saved_collection(ws, &options.source)?], global.clone())
        }
        _ => (collections_in(file)?, Scripts::default()),
    };
    let mut env = match (&options.env, workspace) {
        (Some(name), Some(ws)) => saved_env(ws, name)?,
        _ => AppEnvironment {
            id: "rustman-cli".to_owned(),
            name: "rustman-cli".to_owned(),
            variables: Default::default(),
            is_active: true,
        },
    };
    env.variables.extend(options.vars.iter().cloned());
    Ok(Loaded {
        collections,
        global,
        env,
    })
}

fn data_dir(options: &RunOptions) -> PathBuf {
    options.data_dir.clone().unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustman")
    })
}

/// The app's saved workspace and global scripts. A directory without the
/// app's database is an error rather than a new, empty one.
fn saved_data(data_dir: &Path) -> Result<(Workspace, Scripts), String> {
    /// The session fields that hold the global scripts.
    #[derive(Deserialize)]
    struct Session {
        #[serde(default)]
        global_pre_request_script: String,
        #[serde(default)]
        global_test_script: String,
    }
    if !data_dir.join("rustman.db").is_file() {
        return Err(format!(
            "no saved Rustman data in {}; pass --data-dir, or a collection file",
            data_dir.display()
        ));
    }
    let store = SqliteStore::open(data_dir).map_err(|e| e.to_string())?;
    let workspace = store.load_workspace().map_err(|e| e.to_string())?;
    let global = match store.load_session::<Session>() {
        Ok(Some(session)) => Scripts {
            pre_request: session.global_pre_request_script,
            test: session.global_test_script,
        },
        Ok(None) => Scripts::default(),
        Err(e) => {
            eprintln!("rustman-cli: running without the app's global scripts: {e}");
            Scripts::default()
        }
    };
    Ok((workspace, global))
}

fn saved_collection(ws: &Workspace, name: &str) -> Result<ImportedCollection, String> {
    let collection = unique(&ws.collections, name, |c| (&c.id, &c.name), "collection")?;
    let requests: Vec<_> = ws
        .requests
        .iter()
        .filter(|r| r.collection_id == collection.id)
        .cloned()
        .collect();
    Ok(ImportedCollection::from_parts(
        collection.clone(),
        &ws.groups,
        &requests,
    ))
}

fn saved_env(ws: &Workspace, name: &str) -> Result<AppEnvironment, String> {
    unique(&ws.environments, name, |e| (&e.id, &e.name), "environment").cloned()
}

/// The one item whose id or name is `wanted`.
fn unique<'a, T>(
    items: &'a [T],
    wanted: &str,
    id_and_name: impl Fn(&T) -> (&String, &String),
    what: &str,
) -> Result<&'a T, String> {
    let mut matches = items.iter().filter(|item| {
        let (id, name) = id_and_name(item);
        id == wanted || name == wanted
    });
    match (matches.next(), matches.next()) {
        (Some(item), None) => Ok(item),
        (None, _) => Err(format!("no saved {what} is called '{wanted}'")),
        (Some(_), Some(_)) => Err(format!(
            "more than one saved {what} is called '{wanted}'; use its id"
        )),
    }
}

fn collections_in(file: &Path) -> Result<Vec<ImportedCollection>, String> {
    let text = std::fs::read_to_string(file)
        .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let outcome = import_collection(&text)?;
    for warning in &outcome.warnings {
        eprintln!("rustman-cli: {}: {warning}", file.display());
    }
    Ok(outcome.collections)
}
