mod clash;

use std::fmt::Write as _;

use iced::Task;

use crate::app::AppState;
use crate::message::{AppMsg, ImportMsg, Message, RequestMsg};

pub(super) fn handle(state: &mut AppState, msg: ImportMsg) -> Task<Message> {
    match msg {
        ImportMsg::ImportCollectionFile => {
            state.import_dialog_open = false;
            pick_collection_file()
        }

        ImportMsg::OpenExportDialog(col_id) => {
            state.export_dialog_collection = Some(col_id);
            state.sidebar.export_include_secrets = false;
            state.sidebar.context_menu = None;
            Task::none()
        }

        ImportMsg::ExportIncludeSecretsToggled => {
            state.sidebar.export_include_secrets = !state.sidebar.export_include_secrets;
            Task::none()
        }

        ImportMsg::CloseExportDialog => {
            state.export_dialog_collection = None;
            Task::none()
        }

        ImportMsg::ExportCollectionJson(col_id) => export_collection(state, &col_id, Format::Rustman),
        ImportMsg::ExportCollection(col_id) => export_collection(state, &col_id, Format::Postman),

        ImportMsg::ExportAllCollections => {
            let all: Vec<crate::domain::group::ImportedCollection> = state
                .collections
                .iter()
                .map(|c| crate::app::tree::collection_from_state(state, c))
                .collect();
            if all.is_empty() {
                state.status_message = Some("Nothing to export — no collections yet".to_owned());
                return Task::none();
            }
            let json = crate::services::import::native_export_all(
                &all,
                crate::services::import::ExportOptions::default(),
            );
            let file_name = "rustman-collections.json".to_owned();
            save_json(file_name, "Export All Collections", json)
        }

        ImportMsg::SampleRequested => Task::batch([
            Task::done(Message::Request(RequestMsg::UrlChanged(crate::ui::start::SAMPLE_URL.to_owned()))),
            Task::done(Message::Request(RequestMsg::Send)),
        ]),

        ImportMsg::CollectionsImported(outcome) => {
            clash::add(state, outcome.collections, 0, outcome.warnings);
            Task::none()
        }

        ImportMsg::ClashResolved(choice) => {
            clash::resolve(state, choice);
            Task::none()
        }

        ImportMsg::ExportDone(path) => {
            state.status_message = Some(format!("Exported to {path}"));
            Task::none()
        }

        ImportMsg::ExportFailed(e) => {
            state.status_message = Some(format!("Export failed: {e}"));
            Task::none()
        }

        ImportMsg::Error(e) => {
            state.status_message = Some(format!("Import error: {e}"));
            Task::none()
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Format {
    Postman,
    Rustman,
}

/// Closes the Export dialog and saves the collection in `format`, with
/// secrets only when the dialog's switch was on.
fn export_collection(state: &mut AppState, col_id: &str, format: Format) -> Task<Message> {
    state.export_dialog_collection = None;
    let Some((name, json)) = exported(state, col_id, format) else { return Task::none() };
    match format {
        Format::Postman => save_json(format!("{name}.postman_collection.json"), "Export Postman Collection", json),
        Format::Rustman => save_json(format!("{name}.rustman.json"), "Export Rustman Collection", json),
    }
}

/// The collection's name and its export as JSON.
fn exported(state: &AppState, col_id: &str, format: Format) -> Option<(String, String)> {
    let collection = state.collections.iter().find(|c| c.id == col_id)?;
    let imported = crate::app::tree::collection_from_state(state, collection);
    let options = crate::services::import::ExportOptions {
        include_secrets: state.sidebar.export_include_secrets,
    };
    let json = match format {
        Format::Postman => crate::services::import::postman::export(&imported, options),
        Format::Rustman => crate::services::import::native_export(&imported, options),
    };
    Some((collection.name.clone(), json))
}

fn pick_collection_file() -> Task<Message> {
    Task::perform(
        async {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON | YAML", &["json", "yml", "yaml"])
                .set_title("Import a collection (Postman, OpenAPI, HTTPie or Rustman)")
                .pick_file()
                .await?;
            let limit = crate::consts::IMPORT_FILE_LIMIT;
            Some(match crate::services::files::read_capped(file.path(), limit).await {
                Ok(bytes) => crate::services::import::import_collection(&String::from_utf8_lossy(&bytes)),
                Err(e) => Err(e.describe(&file.file_name(), limit)),
            })
        },
        |outcome| match outcome {
            Some(Ok(outcome)) => Message::Import(ImportMsg::CollectionsImported(outcome)),
            Some(Err(e)) => Message::Import(ImportMsg::Error(e)),
            None => Message::App(AppMsg::Noop),
        },
    )
}

/// Warnings listed in the import summary; the rest are counted.
const WARNINGS_LISTED: usize = 3;

/// "Imported 12 requests · 2 warnings: …", naming what didn't carry over.
fn import_summary(requests: usize, warnings: &[String]) -> String {
    let mut summary = format!("Imported {requests} request{}", if requests == 1 { "" } else { "s" });
    if warnings.is_empty() {
        return summary;
    }
    let s = if warnings.len() == 1 { "" } else { "s" };
    let _ = write!(summary, " · {} warning{s}: ", warnings.len());
    summary.push_str(&warnings.iter().take(WARNINGS_LISTED).cloned().collect::<Vec<_>>().join("; "));
    if warnings.len() > WARNINGS_LISTED {
        let _ = write!(summary, "; and {} more", warnings.len() - WARNINGS_LISTED);
    }
    summary
}

/// Ask where to save `json`, write it, and report the outcome. Cancelling
/// the dialog is silent; a failed write is reported, never shown as success.
fn save_json(file_name: String, title: &'static str, json: String) -> Task<Message> {
    Task::perform(
        async move {
            let file = rfd::AsyncFileDialog::new()
                .set_file_name(&file_name)
                .add_filter("JSON", &["json"])
                .set_title(title)
                .save_file()
                .await?;
            let path = file.path().to_path_buf();
            let written = tokio::task::spawn_blocking({
                let path = path.clone();
                move || write_atomically(&path, json.as_bytes())
            })
            .await
            .map_err(std::io::Error::other)
            .and_then(|r| r);
            Some(
                written
                    .map(|()| path.display().to_string())
                    .map_err(|e| format!("{}: {e}", path.display())),
            )
        },
        |outcome| match outcome {
            Some(Ok(path)) => Message::Import(ImportMsg::ExportDone(path)),
            Some(Err(e)) => Message::Import(ImportMsg::ExportFailed(e)),
            None => Message::App(AppMsg::Noop),
        },
    )
}

/// Writes beside `path`, then renames over it, so a failed write never
/// destroys the file being replaced.
fn write_atomically(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let result = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        // Best effort: the write error is the one worth reporting.
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{import_summary, write_atomically};
    use crate::services::import::ImportOutcome;
    use crate::test_support::TempDir;

    #[test]
    fn the_import_summary_names_what_did_not_carry_over() {
        let clean: ImportOutcome = vec![crate::test_support::sample_collection()].into();
        let requests = clean.collections[0].all_requests().len();
        assert_eq!(import_summary(requests, &[]), format!("Imported {requests} requests"));
        let warnings = ["PROPFIND imported as GET".to_owned(), "Digest auth isn't supported".to_owned()];
        assert_eq!(
            import_summary(requests, &warnings),
            format!("Imported {requests} requests · 2 warnings: PROPFIND imported as GET; Digest auth isn't supported")
        );
        let many: Vec<String> = (1..=5).map(|i| format!("w{i}")).collect();
        assert!(import_summary(requests, &many).ends_with("w1; w2; w3; and 2 more"));
    }

    #[test]
    fn secrets_leave_with_an_export_only_when_the_switch_is_on() {
        let dir = TempDir::new("export-secrets");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let mut collection = crate::test_support::sample_collection();
        collection.requests[0].auth_type = crate::domain::request::AuthType::Bearer;
        collection.requests[0].bearer_token = "s3cret-token".into();
        let id = collection.collection.id.clone();
        crate::app::tree::insert_collection(&mut state, collection);
        let send = |state: &mut crate::app::AppState, msg| {
            let _ = crate::app::update::update(state, crate::message::Message::Import(msg));
        };
        send(&mut state, super::ImportMsg::OpenExportDialog(id.clone()));
        for format in [super::Format::Postman, super::Format::Rustman] {
            assert!(!super::exported(&state, &id, format).unwrap().1.contains("s3cret-token"), "{format:?} strips by default");
        }
        send(&mut state, super::ImportMsg::ExportIncludeSecretsToggled);
        for format in [super::Format::Postman, super::Format::Rustman] {
            assert!(super::exported(&state, &id, format).unwrap().1.contains("s3cret-token"), "{format:?} keeps them when asked");
        }
        send(&mut state, super::ImportMsg::OpenExportDialog(id));
        assert!(!state.sidebar.export_include_secrets, "every export starts with secrets left out");
    }

    #[test]
    fn the_sample_request_fills_in_the_url_then_sends() {
        let dir = TempDir::new("start-card");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let sample = crate::app::update::update(&mut state, crate::message::Message::Import(super::ImportMsg::SampleRequested));
        assert_eq!(sample.units(), 2);
    }

    #[test]
    fn importing_a_postman_file_reports_its_warnings() {
        let postman = r#"{"info":{"name":"Dav","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
            "item":[{"name":"Find","request":{"method":"PROPFIND","url":"https://dav.example/files"}}]}"#;
        let outcome = crate::services::import::import_collection(postman).expect("valid Postman file");
        assert!(!outcome.warnings.is_empty(), "PROPFIND has no Rustman method");
        let dir = TempDir::new("postman-warnings");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let _ = crate::app::update::update(&mut state, crate::message::Message::Import(super::ImportMsg::CollectionsImported(outcome)));
        assert!(state.status_message.as_deref().is_some_and(|m| m.contains("1 warning")));
    }

    #[test]
    fn an_export_replaces_the_file_whole() {
        let dir = TempDir::new("export");
        let path = dir.path().join("out.json");
        std::fs::write(&path, b"old").unwrap();
        write_atomically(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert!(!dir.path().join("out.json.tmp").exists());
    }

    #[test]
    fn a_failed_export_leaves_the_target_alone() {
        let dir = TempDir::new("export-fail");
        let target = dir.path().join("taken");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep"), b"data").unwrap();
        assert!(write_atomically(&target, b"new").is_err(), "a file can't replace a directory");
        assert_eq!(std::fs::read(target.join("keep")).unwrap(), b"data");
        assert!(!dir.path().join("taken.tmp").exists(), "the temp file is cleaned up");
    }
}
