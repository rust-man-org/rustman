use super::*;
fn fixture() -> (Vec<Collection>, Vec<Group>, Vec<SavedRequest>) {
    let cols = vec![Collection {
        id: "c".into(),
        name: "Merchant API".into(),
        created_at: 0,
    }];
    let groups = vec![Group {
        id: "auth".into(),
        parent_id: "c".into(),
        parent_is_collection: true,
        name: "auth".into(),
        sort_order: 0,
    }];
    let mut login = SavedRequest::new_in_group("c".into(), Some("auth".into()), "Login".into(), 0);
    login.id = "login".into();
    login.url = "{{base_url}}/v1/login".into();
    login.body = "{\"strategy\":\"sso\",\"password\":\"never-show-this-secret\"}".into();
    let mut profile =
        SavedRequest::new_in_group("c".into(), Some("auth".into()), "Profile".into(), 1);
    profile.id = "profile".into();
    profile.url = "{{base_url}}/profile".into();
    profile.test_script = "assert(response.sso_login)".into();
    (cols, groups, vec![login, profile])
}
fn search(q: &str) -> Vec<Hit> {
    let (c, g, r) = fixture();
    find(&c, &g, &r.iter().collect::<Vec<_>>(), q)
}
#[test]
fn typo_and_abbreviation_find_endpoints() {
    for q in ["logni", "lgn", "LOGIN"] {
        assert!(search(q).iter().any(|h| h.id == "login"), "query {q}");
    }
}
#[test]
fn multiple_words_match_across_fields_but_all_are_required() {
    assert!(search("merchant login").iter().any(|h| h.id == "login"));
    assert!(
        search("sso login")
            .iter()
            .any(|h| h.id == "login" && h.kind == Kind::BodyScript)
    );
    assert!(search("login missingword").is_empty());
}
#[test]
fn folder_can_match_its_contained_endpoint_names() {
    let h = search("login")
        .into_iter()
        .find(|h| h.id == "auth")
        .unwrap();
    assert_eq!(h.kind, Kind::Folder);
    assert!(h.detail.contains("2 requests"));
}
#[test]
fn content_matches_do_not_expose_values() {
    let hits = search("sso");
    assert!(
        hits.iter()
            .any(|h| h.id == "profile" && h.kind == Kind::BodyScript)
    );
    assert!(!format!("{hits:?}").contains("never-show-this-secret"));
}
#[test]
fn blank_and_no_match_are_empty() {
    assert!(search("  ").is_empty());
    assert!(search("zzzzqqqq").is_empty());
}
#[test]
fn exact_name_beats_partial_name_and_ties_are_stable() {
    let (c, g, mut r) = fixture();
    let mut other = r[0].clone();
    other.id = "login-v2".into();
    other.name = "Login v2".into();
    r.push(other);
    let hits = find(&c, &g, &r.iter().rev().collect::<Vec<_>>(), "login");
    assert_eq!(hits[0].id, "login");
    assert_eq!(
        hits.iter().map(|h| &h.id).collect::<Vec<_>>(),
        find(&c, &g, &r.iter().collect::<Vec<_>>(), "login")
            .iter()
            .map(|h| &h.id)
            .collect::<Vec<_>>()
    );
}
#[test]
fn unicode_names_are_searchable_without_panics() {
    let (c, g, mut r) = fixture();
    r[0].name = "登录 café".into();
    assert!(
        find(&c, &g, &r.iter().collect::<Vec<_>>(), "CAFÉ")
            .iter()
            .any(|h| h.id == "login")
    );
}

#[test]
fn larger_collection_limits_rows_and_reports_query_time() {
    let (c, g, r) = fixture();
    let requests: Vec<_> = (0..500)
        .map(|i| {
            let mut req = r[0].clone();
            req.id = format!("login-{i:04}");
            req.name = format!("Login {i}");
            req
        })
        .collect();
    let refs: Vec<_> = requests.iter().collect();
    let start = std::time::Instant::now();
    let hits = find(&c, &g, &refs, "logni");
    eprintln!("500 endpoint typo search: {:?}", start.elapsed());
    assert!(hits.len() <= MAX_RESULTS);
    assert_eq!(hits.iter().filter(|h| h.kind == Kind::Endpoint).count(), 10);
    assert!(hits.iter().any(|h| h.kind == Kind::Folder));
}
