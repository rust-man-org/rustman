//! The git service against throwaway local repositories. No network: the
//! only "remote" state is a ref written by hand or a URL never contacted.

use git2::Repository;
use rustman_core::domain::collection::SavedRequest;
use rustman_core::domain::group::ImportedCollection;
use rustman_git::vcs::*;
use rustman_testkit::{
    COLLECTION_ID, GLOBAL_PRE_REQUEST_SCRIPT, GLOBAL_TEST_SCRIPT, TempDir, as_written,
    full_saved_request, kv, sample_collection, second_collection,
};

fn me() -> GitIdentity {
    GitIdentity {
        name: "Rustman Test".into(),
        email: "test@example.invalid".into(),
    }
}

/// A fresh, empty repository.
fn repo(label: &str) -> TempDir {
    let dir = TempDir::new(label);
    init(dir.path()).unwrap();
    dir
}

fn renamed(name: &str) -> Vec<ImportedCollection> {
    let mut c = sample_collection();
    c.collection.name = name.into();
    vec![c]
}

fn names(loaded: &LoadedCollections) -> Vec<String> {
    let mut out: Vec<String> = loaded
        .collections
        .iter()
        .map(|c| c.collection.name.clone())
        .collect();
    out.sort();
    out
}

fn file_names(loaded: &LoadedCollections) -> Vec<&str> {
    let mut out: Vec<&str> = loaded.unreadable.iter().map(|u| u.file.as_str()).collect();
    out.sort_unstable();
    out
}

/// A repo holding `sample_collection()` named `name`, committed once.
fn committed_repo(name: &str) -> (TempDir, String) {
    let dir = repo("vcs");
    write_working_tree(dir.path(), &renamed(name), "", "").unwrap();
    let id = commit(dir.path(), &me(), "first").unwrap();
    (dir, id)
}

fn secrets(r: &SavedRequest) -> [&str; 5] {
    [
        &r.bearer_token,
        &r.basic_pass,
        &r.api_key_value,
        &r.jwt_secret,
        &r.cookie_string,
    ]
}

/// `collection` as git should hold it, by the one rule for what's secret.
fn without_secrets(collection: &ImportedCollection) -> ImportedCollection {
    let mut out = collection.clone();
    rustman_core::secrets::strip_collection_secrets(&mut out);
    out
}

fn sorted(mut collections: Vec<ImportedCollection>) -> Vec<ImportedCollection> {
    collections.sort_by(|a, b| a.collection.id.cmp(&b.collection.id));
    collections
}

// ── Opening and initialising ──────────────────────────────────────────────────

#[test]
fn init_creates_a_repository_and_leaves_an_existing_one_alone() {
    let parent = TempDir::new("vcs-init");
    let path = parent.path().join("collections");
    init(&path).unwrap();
    assert!(path.join(".git").is_dir(), "the directory is created too");
    let st = status(&path).unwrap();
    assert!(st.changes.is_empty());
    assert_eq!((st.ahead, st.behind), (0, 0));
    assert_eq!(st.remote_url, None);
    assert!(full_log(&path, 10).is_empty(), "no commits yet");

    write_working_tree(&path, &[second_collection()], "", "").unwrap();
    let first = commit(&path, &me(), "first").unwrap();
    init(&path).unwrap();
    assert_eq!(full_log(&path, 10)[0].id, first, "re-init keeps history");
}

#[test]
fn operations_never_turn_a_plain_folder_into_a_repository() {
    let dir = TempDir::new("vcs-plain");
    std::fs::write(dir.path().join("package.json"), r#"{"name": "x"}"#).unwrap();

    assert!(matches!(
        status(dir.path()),
        Err(GitError::NotARepository(_))
    ));
    assert!(matches!(
        write_working_tree(dir.path(), &[second_collection()], "pre", ""),
        Err(GitError::NotARepository(_))
    ));
    assert!(commit(dir.path(), &me(), "x").is_err());
    assert!(branches(dir.path()).is_err());
    assert!(set_remote(dir.path(), "https://example.invalid/x.git").is_err());
    assert!(fetch(dir.path()).is_err());
    assert_eq!(remote_url(dir.path()), None);

    assert!(!dir.path().join(".git").exists());
    assert!(!dir.path().join("col-auth.json").exists());
}

#[test]
fn a_missing_repository_is_reported_and_not_recreated() {
    let parent = TempDir::new("vcs-missing");
    let gone = parent.path().join("moved-away");

    assert!(matches!(status(&gone), Err(GitError::Missing(_))));
    assert!(matches!(
        write_working_tree(&gone, &[second_collection()], "", ""),
        Err(GitError::Missing(_))
    ));
    assert!(working_diff(&gone).is_err());
    assert!(!gone.exists(), "nothing was recreated");
}

#[test]
fn probing_tells_repositories_plain_folders_and_missing_ones_apart() {
    let dir = repo("vcs-probe");
    assert_eq!(probe(dir.path()), Folder::Repository);

    let sub = dir.path().join("src");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("main.rs"), "").unwrap();
    let Folder::Plain { entries, enclosing } = probe(&sub) else {
        panic!("a subfolder is not a repository root");
    };
    assert_eq!(entries, 1);
    assert_eq!(
        enclosing.map(|p| p.canonicalize().unwrap()),
        Some(dir.path().canonicalize().unwrap())
    );

    assert_eq!(probe(&dir.path().join("nope")), Folder::Missing);
    assert!(!sub.join(".git").exists(), "probing changes nothing");
}

// ── Working tree ──────────────────────────────────────────────────────────────

#[test]
fn working_tree_round_trips_collections_and_global_scripts() {
    let dir = repo("vcs-tree");
    let collections = vec![sample_collection(), second_collection()];
    let skipped = write_working_tree(
        dir.path(),
        &collections,
        GLOBAL_PRE_REQUEST_SCRIPT,
        GLOBAL_TEST_SCRIPT,
    )
    .unwrap();
    assert!(skipped.invalid_ids.is_empty() && skipped.unreadable.is_empty());

    let loaded = load_collections_from(dir.path()).unwrap();
    assert!(loaded.unreadable.is_empty());
    let expected: Vec<_> = collections.iter().map(without_secrets).collect();
    assert_eq!(
        as_written(&sorted(loaded.collections)),
        as_written(&sorted(expected))
    );
    assert_eq!(
        read_global_script(dir.path()),
        Some((GLOBAL_PRE_REQUEST_SCRIPT.into(), GLOBAL_TEST_SCRIPT.into()))
    );

    let mut changes: Vec<(String, String)> = status(dir.path())
        .unwrap()
        .changes
        .into_iter()
        .map(|c| (c.path, c.state))
        .collect();
    changes.sort();
    let new = |p: &str| (p.to_owned(), "new".to_owned());
    assert_eq!(
        changes,
        [
            new("col-auth.json"),
            new("col-storefront.json"),
            new("global.json")
        ]
    );
}

#[test]
fn rewriting_the_tree_drops_removed_collections_and_empty_globals() {
    let dir = repo("vcs-prune");
    write_working_tree(
        dir.path(),
        &[sample_collection(), second_collection()],
        GLOBAL_PRE_REQUEST_SCRIPT,
        "",
    )
    .unwrap();
    std::fs::write(dir.path().join("README.md"), "notes").unwrap();

    write_working_tree(dir.path(), &[second_collection()], "", "").unwrap();

    assert!(!dir.path().join(format!("{COLLECTION_ID}.json")).exists());
    assert!(dir.path().join("col-auth.json").exists());
    assert!(
        !dir.path().join("global.json").exists(),
        "no scripts, no file"
    );
    assert!(
        dir.path().join("README.md").exists(),
        "non-JSON files are left alone"
    );
    assert_eq!(read_global_script(dir.path()), None);
}

#[test]
fn refreshing_never_deletes_files_rustman_did_not_write() {
    let dir = repo("vcs-foreign");
    let package = r#"{"name": "web", "version": "1.0.0"}"#;
    let tsconfig = "{\n  // comments make this JSONC\n  \"compilerOptions\": {}\n}\n";
    let mut exported = second_collection();
    exported.collection.id = "col-exported".into();
    let export = serde_json::to_string(&exported).unwrap();
    std::fs::write(dir.path().join("package.json"), package).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), tsconfig).unwrap();
    std::fs::write(dir.path().join("api-export.json"), &export).unwrap();

    for _ in 0..2 {
        write_working_tree(dir.path(), &[sample_collection()], "", "").unwrap();
    }
    write_working_tree(dir.path(), &[], "", "").unwrap();

    let read = |name: &str| std::fs::read_to_string(dir.path().join(name)).unwrap();
    assert_eq!(read("package.json"), package);
    assert_eq!(read("tsconfig.json"), tsconfig);
    assert_eq!(read("api-export.json"), export, "named after another id");
    assert!(
        !dir.path().join(format!("{COLLECTION_ID}.json")).exists(),
        "Rustman's own file is still removed with its collection"
    );
}

#[test]
fn files_rustman_cannot_read_are_reported_and_never_overwritten_or_deleted() {
    let dir = repo("vcs-unreadable");
    let both = [sample_collection(), second_collection()];
    write_working_tree(dir.path(), &both, "pre", "").unwrap();
    let truncated = std::fs::read(dir.path().join("col-auth.json")).unwrap()[..40].to_vec();
    std::fs::write(dir.path().join("col-auth.json"), &truncated).unwrap();
    let newer = r#"{"collection": {"id": "col-storefront", "schema": 9}, "items": []}"#;
    std::fs::write(dir.path().join("col-storefront.json"), newer).unwrap();
    std::fs::write(dir.path().join("global.json"), "<<<<<<< HEAD").unwrap();

    let skipped = write_working_tree(dir.path(), &both, "changed", "").unwrap();
    let mut untouched: Vec<&str> = skipped.unreadable.iter().map(|u| u.file.as_str()).collect();
    untouched.sort_unstable();
    assert_eq!(
        untouched,
        ["col-auth.json", "col-storefront.json", "global.json"]
    );
    write_working_tree(dir.path(), &[], "", "").unwrap();

    let read = |name: &str| std::fs::read(dir.path().join(name)).unwrap();
    assert_eq!(read("col-auth.json"), truncated);
    assert_eq!(read("col-storefront.json"), newer.as_bytes());
    assert_eq!(read("global.json"), b"<<<<<<< HEAD");
}

#[test]
fn loading_reports_every_collection_file_it_cannot_read() {
    let dir = repo("vcs-load");
    write_working_tree(dir.path(), &[second_collection()], "pre", "").unwrap();
    std::fs::write(dir.path().join("package.json"), r#"{"name": "x"}"#).unwrap();
    std::fs::write(dir.path().join("broken.json"), "{not json").unwrap();
    std::fs::write(
        dir.path().join("newer.json"),
        r#"{"collection": {"id": "newer"}, "requests": 3}"#,
    )
    .unwrap();

    let loaded = load_collections_from(dir.path()).unwrap();
    assert_eq!(names(&loaded), ["Auth service"]);
    assert_eq!(
        file_names(&loaded),
        ["broken.json", "newer.json", "package.json"]
    );
    let problem = |file: &str| {
        let u = loaded.unreadable.iter().find(|u| u.file == file).unwrap();
        (u.collection_id(), u.problem.clone())
    };
    assert!(matches!(
        problem("broken.json"),
        (Some("broken"), FileProblem::Malformed(_))
    ));
    assert!(matches!(
        problem("newer.json"),
        (Some("newer"), FileProblem::Unsupported(_))
    ));
    assert_eq!(
        problem("package.json"),
        (Some("package"), FileProblem::Foreign)
    );
}

#[test]
fn collection_ids_cannot_point_outside_the_working_tree() {
    let parent = TempDir::new("vcs-traversal");
    let dir = parent.path().join("repo");
    init(&dir).unwrap();
    let outside = parent.path().join("escaped");
    let evil: Vec<ImportedCollection> = [
        "../escaped".to_owned(),
        outside.display().to_string(),
        "global".to_owned(),
        "nul".to_owned(),
    ]
    .into_iter()
    .map(|id| {
        let mut c = second_collection();
        c.collection.id = id;
        c
    })
    .collect();

    let skipped = write_working_tree(&dir, &evil, "", "").unwrap();
    assert_eq!(skipped.invalid_ids.len(), 4);
    assert!(!parent.path().join("escaped.json").exists());
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, [".git"]);
}

#[test]
fn a_collection_file_must_hold_the_collection_it_is_named_after() {
    let dir = repo("vcs-wrong-id");
    let mut evil = second_collection();
    evil.collection.id = "../../outside".into();
    std::fs::write(
        dir.path().join("innocent.json"),
        serde_json::to_string(&evil).unwrap(),
    )
    .unwrap();
    commit(dir.path(), &me(), "planted").unwrap();
    let head = full_log(dir.path(), 1).remove(0).id;

    for loaded in [
        load_collections_from(dir.path()).unwrap(),
        read_commit_collections(dir.path(), &head).unwrap(),
    ] {
        assert!(loaded.collections.is_empty());
        assert_eq!(
            loaded.unreadable[0].problem,
            FileProblem::OtherCollection("../../outside".into())
        );
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_collection_file_is_never_followed() {
    let dir = repo("vcs-symlink");
    let target = TempDir::new("vcs-symlink-target");
    let elsewhere = target.path().join("col-auth.json");
    let original = serde_json::to_string(&second_collection()).unwrap();
    std::fs::write(&elsewhere, &original).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir.path().join("col-auth.json")).unwrap();

    assert!(
        load_collections_from(dir.path())
            .unwrap()
            .collections
            .is_empty()
    );
    write_working_tree(dir.path(), &[], "", "").unwrap();
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), original);
}

// ── Secrets ───────────────────────────────────────────────────────────────────

#[test]
fn committed_files_hold_no_secrets() {
    let (dir, head) = committed_repo("Storefront API");
    let full = full_saved_request();
    let file = std::fs::read_to_string(dir.path().join(format!("{COLLECTION_ID}.json"))).unwrap();
    for secret in secrets(&full) {
        assert!(!file.contains(secret), "{secret} reached git");
    }
    assert!(
        file.contains("\"basic_user\": \"alice\""),
        "user names are not secrets"
    );
    assert!(
        file.contains("\"bearer_token\": \"{{token}}\""),
        "references are kept"
    );

    let committed = read_commit_collections(dir.path(), &head).unwrap();
    let all = committed.collections[0].all_requests();
    let request = all.iter().find(|r| r.id == full.id).unwrap();
    assert_eq!(secrets(request), ["", "", "", "", ""]);
}

#[test]
fn blank_incoming_secrets_keep_the_local_ones() {
    let (dir, _) = committed_repo("Storefront API");
    let local = sample_collection();
    let local_requests: Vec<SavedRequest> = local.all_requests().into_iter().cloned().collect();
    let mut pulled = load_collections_from(dir.path())
        .unwrap()
        .collections
        .remove(0);

    keep_local_secrets(&mut pulled, &local_requests);

    assert_eq!(as_written(&pulled), as_written(&local));
}

#[test]
fn a_legacy_commit_with_secrets_fills_only_empty_local_fields() {
    let mut legacy = second_collection();
    legacy.requests[0].bearer_token = "old-token".into();
    legacy.requests[0].cookie_string = "old=cookie".into();
    let mut local = legacy.requests[0].clone();
    local.bearer_token = String::new();
    local.cookie_string = "newer=cookie".into();

    let mut into_empty_db = legacy.clone();
    keep_local_secrets(&mut into_empty_db, &[]);
    assert_eq!(into_empty_db.requests[0].bearer_token, "old-token");

    let mut incoming = legacy;
    keep_local_secrets(&mut incoming, std::slice::from_ref(&local));
    assert_eq!(
        incoming.requests[0].bearer_token, "old-token",
        "fills a blank field"
    );
    assert_eq!(
        incoming.requests[0].cookie_string, "newer=cookie",
        "never replaces a local value"
    );
}

#[test]
fn credential_rows_keep_local_values_while_other_row_edits_come_from_git() {
    let mut local = second_collection();
    local.requests[0].headers = vec![
        kv("h-auth", "authorization", "Bearer s3cret", true),
        kv("h-trace", "X-Trace", "on", true),
    ];
    local.requests[0].cookies = vec![kv("c-sid", "sid", "abc", true)];
    let mut pulled = local.clone();
    rustman_core::secrets::strip_collection_secrets(&mut pulled);
    assert_eq!(pulled.requests[0].headers[0].value, "", "git never sees it");
    pulled.requests[0].headers[1].value = "off".into();

    keep_local_secrets(&mut pulled, &local.requests);

    let request = &pulled.requests[0];
    assert_eq!(request.headers[0].value, "Bearer s3cret");
    assert_eq!(request.headers[1].value, "off", "a teammate's edit");
    assert_eq!(request.cookies[0].value, "abc");
}

#[test]
fn a_changed_variable_reference_follows_git() {
    let mut incoming = second_collection();
    incoming.requests[0].bearer_token = "{{staging_token}}".into();
    let mut local = incoming.requests[0].clone();
    local.bearer_token = "{{token}}".into();
    keep_local_secrets(&mut incoming, std::slice::from_ref(&local));
    assert_eq!(incoming.requests[0].bearer_token, "{{staging_token}}");

    local.bearer_token = "real-local-token".into();
    keep_local_secrets(&mut incoming, std::slice::from_ref(&local));
    assert_eq!(incoming.requests[0].bearer_token, "real-local-token");
}

// ── History and branches ──────────────────────────────────────────────────────

#[test]
fn commits_show_up_in_the_log_newest_first() {
    let (dir, first) = committed_repo("v1");
    write_working_tree(dir.path(), &renamed("v2"), "", "").unwrap();
    let second = commit(dir.path(), &me(), "second\n\nwith a body").unwrap();

    let log = full_log(dir.path(), 10);
    let ids: Vec<&str> = log.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, [second.as_str(), first.as_str()]);
    assert_eq!(log[0].message, "second\n\nwith a body");
    assert_eq!(log[1].message, "first");
    assert!(
        log[0].timestamp > 1_700_000_000,
        "commit time is a unix timestamp"
    );
    assert_eq!(full_log(dir.path(), 1).len(), 1, "limit is honoured");

    let repo = Repository::open(dir.path()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let author = head.author();
    assert_eq!(author.name().ok(), Some("Rustman Test"));
    assert_eq!(author.email().ok(), Some("test@example.invalid"));
    assert_eq!(head.parent_id(0).unwrap().to_string(), first);
    assert!(
        status(dir.path()).unwrap().changes.is_empty(),
        "commit stages everything"
    );
}

#[test]
fn restoring_a_commit_reads_it_without_touching_the_working_tree() {
    let (dir, first) = committed_repo("v1");
    write_working_tree(dir.path(), &renamed("v2"), "pre v2", "test v2").unwrap();
    commit(dir.path(), &me(), "second").unwrap();

    let old = read_commit_collections(dir.path(), &first).unwrap();
    assert_eq!(names(&old), ["v1"]);
    assert_eq!(
        old.collections[0].all_requests().len(),
        sample_collection().all_requests().len()
    );
    assert_eq!(
        read_commit_global_script(dir.path(), &first),
        None,
        "v1 had no global.json"
    );

    let head = full_log(dir.path(), 1).remove(0).id;
    assert_eq!(
        read_commit_global_script(dir.path(), &head),
        Some(("pre v2".into(), "test v2".into()))
    );
    assert_eq!(names(&load_collections_from(dir.path()).unwrap()), ["v2"]);
    assert!(read_commit_collections(dir.path(), "not-a-commit-id").is_err());
}

#[test]
fn branches_can_be_created_and_switched() {
    let (dir, _) = committed_repo("on main");
    let main = status(dir.path()).unwrap().branch;

    create_branch(dir.path(), "feature").unwrap();
    let mut listed: Vec<(String, bool)> = branches(dir.path())
        .unwrap()
        .into_iter()
        .map(|b| (b.name, b.is_head))
        .collect();
    listed.sort();
    let mut expected = vec![("feature".to_owned(), false), (main.clone(), true)];
    expected.sort();
    assert_eq!(listed, expected);
    assert!(
        create_branch(dir.path(), "feature").is_err(),
        "no silent overwrite"
    );

    checkout_branch(dir.path(), "feature", Checkout::Safe).unwrap();
    assert_eq!(status(dir.path()).unwrap().branch, "feature");
    write_working_tree(dir.path(), &renamed("on feature"), "", "").unwrap();
    commit(dir.path(), &me(), "feature work").unwrap();

    checkout_branch(dir.path(), &main, Checkout::Safe).unwrap();
    assert_eq!(status(dir.path()).unwrap().branch, main);
    assert_eq!(
        names(&load_collections_from(dir.path()).unwrap()),
        ["on main"]
    );

    checkout_branch(dir.path(), "feature", Checkout::Safe).unwrap();
    assert_eq!(
        names(&load_collections_from(dir.path()).unwrap()),
        ["on feature"]
    );
    assert!(checkout_branch(dir.path(), "no-such-branch", Checkout::Safe).is_err());
}

#[test]
fn switching_branches_refuses_to_overwrite_uncommitted_changes() {
    let (dir, _) = committed_repo("on main");
    let main = status(dir.path()).unwrap().branch;
    create_branch(dir.path(), "feature").unwrap();
    write_working_tree(dir.path(), &renamed("saved, not committed"), "", "").unwrap();

    let Err(GitError::Dirty(paths)) = checkout_branch(dir.path(), "feature", Checkout::Safe) else {
        panic!("a dirty checkout must be refused");
    };
    assert_eq!(paths, [format!("{COLLECTION_ID}.json")]);
    assert_eq!(status(dir.path()).unwrap().branch, main);
    assert_eq!(
        names(&load_collections_from(dir.path()).unwrap()),
        ["saved, not committed"],
        "the edit is still on disk"
    );
}

#[test]
fn discarding_changes_switches_and_keeps_files_never_committed() {
    let (dir, _) = committed_repo("on main");
    create_branch(dir.path(), "feature").unwrap();
    let mut collections = renamed("discarded edit");
    collections.push(second_collection());
    write_working_tree(dir.path(), &collections, "", "").unwrap();

    checkout_branch(dir.path(), "feature", Checkout::DiscardChanges).unwrap();
    assert_eq!(status(dir.path()).unwrap().branch, "feature");
    assert_eq!(
        names(&load_collections_from(dir.path()).unwrap()),
        ["Auth service", "on main"]
    );
}

#[test]
fn current_branch_follows_head_and_never_creates_a_repository() {
    let empty = TempDir::new("vcs-head");
    assert_eq!(current_branch(empty.path()), None);
    assert!(!empty.path().join(".git").exists());

    let (dir, _) = committed_repo("on main");
    let main = status(dir.path()).unwrap().branch;
    assert_eq!(current_branch(dir.path()), Some(main));
    create_branch(dir.path(), "feature").unwrap();
    checkout_branch(dir.path(), "feature", Checkout::Safe).unwrap();
    assert_eq!(current_branch(dir.path()).as_deref(), Some("feature"));
}

#[test]
fn working_diff_shows_edits_to_committed_collections() {
    let (dir, _) = committed_repo("Before");
    assert_eq!(
        working_diff(dir.path()).unwrap(),
        "",
        "clean right after a commit"
    );

    write_working_tree(dir.path(), &renamed("After"), "", "").unwrap();
    let diff = working_diff(dir.path()).unwrap();
    assert!(diff.contains("col-storefront.json"), "{diff}");
    assert!(diff.contains("-    \"name\": \"Before\""), "{diff}");
    assert!(diff.contains("+    \"name\": \"After\""), "{diff}");
}

#[test]
fn working_diff_shows_a_collection_that_was_never_committed() {
    let (dir, _) = committed_repo("Existing");
    let mut collections = renamed("Existing");
    collections.push(second_collection());
    write_working_tree(dir.path(), &collections, "", "").unwrap();
    // Status reports the new file...
    assert!(
        status(dir.path())
            .unwrap()
            .changes
            .iter()
            .any(|c| c.path == "col-auth.json" && c.state == "new")
    );
    // ...so the diff shows it as well, with its content as added lines.
    let diff = working_diff(dir.path()).unwrap();
    assert!(diff.contains("col-auth.json"), "diff was: {diff:?}");
    assert!(
        diff.contains("+    \"name\": \"Auth service\""),
        "diff was: {diff:?}"
    );
}

#[test]
fn ahead_and_behind_count_against_the_origin_ref() {
    let (dir, first) = committed_repo("v1");
    let branch = status(dir.path()).unwrap().branch;
    {
        let repo = Repository::open(dir.path()).unwrap();
        let oid = git2::Oid::from_str(&first).unwrap();
        repo.reference(&format!("refs/remotes/origin/{branch}"), oid, true, "test")
            .unwrap();
    }
    write_working_tree(dir.path(), &renamed("v2"), "", "").unwrap();
    commit(dir.path(), &me(), "local only").unwrap();

    let st = status(dir.path()).unwrap();
    assert_eq!((st.ahead, st.behind), (1, 0));
}

#[test]
fn remote_url_is_stored_and_updated_without_contacting_it() {
    let dir = repo("vcs-remote");
    assert_eq!(remote_url(dir.path()), None);
    set_remote(dir.path(), "https://example.invalid/team/api.git").unwrap();
    assert_eq!(
        remote_url(dir.path()).as_deref(),
        Some("https://example.invalid/team/api.git")
    );
    set_remote(dir.path(), "git@example.invalid:team/api.git").unwrap();
    assert_eq!(
        status(dir.path()).unwrap().remote_url.as_deref(),
        Some("git@example.invalid:team/api.git")
    );
}

/// Runs the test `name` again in a child process whose git reads `global`
/// as the user's config and `system` as the machine's, and nothing else, so
/// the identity of whoever runs the tests can't leak into it. Returns
/// whether this is that child, which then runs the test body.
fn with_only_these_git_configs(name: &str, global: &str, system: &str) -> bool {
    const CHILD: &str = "RUSTMAN_TEST_ISOLATED_GIT";
    if std::env::var_os(CHILD).is_some() {
        return true;
    }
    let configs = TempDir::new("vcs-identity-configs");
    let (global_path, system_path) = (configs.path().join("global"), configs.path().join("system"));
    std::fs::write(
        &global_path,
        global.replace("{dir}", &configs.path().display().to_string()),
    )
    .unwrap();
    std::fs::write(&system_path, system).unwrap();
    std::fs::write(
        configs.path().join("included.gitconfig"),
        "[user]\n\tname = Included Ada\n",
    )
    .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env("GIT_CONFIG_GLOBAL", &global_path)
        .env("GIT_CONFIG_SYSTEM", &system_path)
        .env_remove("GIT_CONFIG_NOSYSTEM")
        .output()
        .unwrap();
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{report}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        report.contains("1 passed"),
        "the child ran no test: {report}"
    );
    false
}

#[test]
fn the_identity_is_the_one_git_itself_would_commit_with() {
    if !with_only_these_git_configs(
        "the_identity_is_the_one_git_itself_would_commit_with",
        "[include]\n\tpath = {dir}/included.gitconfig\n",
        "[user]\n\temail = ada@machine.invalid\n",
    ) {
        return;
    }
    assert_eq!(
        default_identity(),
        Some(GitIdentity {
            name: "Included Ada".into(),
            email: "ada@machine.invalid".into()
        }),
        "a name the user's config includes, an email from the machine's"
    );

    let work = repo("vcs-identity-work");
    let elsewhere = TempDir::new("vcs-identity-include");
    let included = elsewhere.path().join("work.gitconfig");
    std::fs::write(&included, "[user]\n\temail = ada@work.invalid\n").unwrap();
    let workdir = work.path().canonicalize().unwrap();
    let local = work.path().join(".git/config");
    let mut config = std::fs::read_to_string(&local).unwrap();
    config.push_str(&format!(
        "[user]\n\tname = Work Ada\n[includeIf \"gitdir:{}/\"]\n\tpath = {}\n",
        workdir.display(),
        included.display()
    ));
    std::fs::write(&local, config).unwrap();
    assert_eq!(
        resolve_identity(work.path()),
        Some(GitIdentity {
            name: "Work Ada".into(),
            email: "ada@work.invalid".into()
        }),
        "the repository's own name and its conditionally included email"
    );

    let plain = repo("vcs-identity-plain");
    assert_eq!(
        resolve_identity(plain.path()),
        default_identity(),
        "nothing of its own: the global identity"
    );
}

#[test]
fn working_diff_is_capped() {
    let (dir, _) = committed_repo("Existing");
    std::fs::write(dir.path().join("huge.txt"), "x\n".repeat(1_300_000)).unwrap();
    let diff = working_diff(dir.path()).unwrap();
    assert!(diff.len() <= 2 * 1024 * 1024 + 64, "{} bytes", diff.len());
    assert!(diff.ends_with("… diff truncated at 2 MB\n"));
}
