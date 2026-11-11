//! Fuzz and property tests for the collection importers: Postman, OpenAPI
//! (JSON and YAML), HTTPie exports, Rustman's native format, and the
//! registry that sniffs which one a file is.
//!
//! A user picks the file, so anything may come in: every input must end in
//! `Ok` or an `Err` with a message, never a panic or a blown stack.

mod fuzz_support;

use std::time::{Duration, Instant};

use fuzz_support::{Rng, assert_linear, cases, fuzz, long_run, size};
use rustman_core::domain::collection::{Collection, SavedRequest};
use rustman_core::domain::group::{ImportedCollection, ImportedGroup};
use rustman_core::domain::request::{
    ApiKeyLocation, AuthType, BodyType, FormField, FormFieldType, HttpMethod, KeyValue,
};
use rustman_core::secrets::strip_collection_secrets;
use rustman_formats::import::{
    ExportOptions, FormatRegistry, httpie_export, import_collection, native_export,
    native_export_all, native_import, postman, swagger,
};
use serde_json::{Map, Value, json};

// ── Generators ───────────────────────────────────────────────────────────────

fn number(rng: &mut Rng) -> Value {
    match rng.below(8) {
        0 => json!(i64::MIN),
        1 => json!(u64::MAX),
        2 => json!(1e308),
        3 => json!(-0.0),
        4 => json!(0.1),
        5 => json!(rng.below(1000)),
        6 => json!(-1),
        _ => json!(f64::MIN_POSITIVE),
    }
}

/// Any JSON value: the wrong type for whatever position it lands in.
fn junk(rng: &mut Rng, depth: usize) -> Value {
    match rng.below(if depth == 0 { 4 } else { 6 }) {
        0 => Value::Null,
        1 => Value::Bool(rng.chance(50)),
        2 => number(rng),
        3 => Value::String(rng.text(16)),
        4 => Value::Array((0..rng.below(4)).map(|_| junk(rng, depth - 1)).collect()),
        _ => Value::Object(
            (0..rng.below(4))
                .map(|_| (rng.text(6), junk(rng, depth - 1)))
                .collect(),
        ),
    }
}

/// `value`, or now and then a value of the wrong type.
fn w(value: Value, rng: &mut Rng) -> Value {
    if rng.chance(7) { junk(rng, 2) } else { value }
}

fn text(rng: &mut Rng) -> Value {
    let s = match rng.below(4) {
        0 => rng.text(20),
        1 => (*rng.pick(&[
            "",
            "{{base}}/a?b=1",
            "https://x.io/p?q=a%20b&r",
            "{\"a\": 1}",
            "[1, 2",
            "GET",
            "post",
            "TRACE",
            "json",
            "text",
            "file",
            "query",
            "header",
            "HS256",
            "true",
        ]))
        .to_owned(),
        _ => rng.text_from(
            &['a', '{', '}', '"', ':', '?', '&', '=', '%', '/', '#', 'é'],
            12,
        ),
    };
    w(Value::String(s), rng)
}

/// An object holding a random subset of `fields`, values from `make`.
fn object(fields: Vec<(&str, Value)>, rng: &mut Rng) -> Value {
    let mut map = Map::new();
    for (key, value) in fields {
        if rng.chance(80) {
            map.insert(key.to_owned(), value);
        }
    }
    if rng.chance(10) {
        map.insert(rng.text(8), junk(rng, 1));
    }
    w(Value::Object(map), rng)
}

fn list(rng: &mut Rng, max: usize, mut item: impl FnMut(&mut Rng) -> Value) -> Value {
    let items = (0..rng.below(max + 1)).map(|_| item(rng)).collect();
    w(Value::Array(items), rng)
}

fn key_values(rng: &mut Rng) -> Value {
    list(rng, 3, |rng| {
        let fields = vec![
            ("key", text(rng)),
            ("value", text(rng)),
            ("disabled", w(Value::Bool(rng.chance(30)), rng)),
            ("type", text(rng)),
            ("src", text(rng)),
            ("contentType", text(rng)),
        ];
        object(fields, rng)
    })
}

fn postman_auth(rng: &mut Rng) -> Value {
    let kind = *rng.pick(&[
        "bearer", "basic", "apikey", "jwt", "noauth", "inherit", "oauth2", "digest", "",
    ]);
    let settings = |rng: &mut Rng| {
        if rng.chance(50) {
            list(rng, 6, |rng| {
                let key = *rng.pick(&[
                    "token",
                    "username",
                    "password",
                    "key",
                    "value",
                    "in",
                    "algorithm",
                    "isSecretBase64Encoded",
                    "addTokenTo",
                    "headerPrefix",
                    "header",
                    "payload",
                    "secret",
                ]);
                let value = match rng.below(3) {
                    0 => json!("{\"sub\": \"u\"}"),
                    1 => json!(false),
                    _ => text(rng),
                };
                object(vec![("key", json!(key)), ("value", value)], rng)
            })
        } else {
            object(
                vec![
                    ("token", text(rng)),
                    ("username", text(rng)),
                    ("in", text(rng)),
                    ("payload", text(rng)),
                ],
                rng,
            )
        }
    };
    let fields = vec![
        ("type", w(json!(kind), rng)),
        ("bearer", settings(rng)),
        ("basic", settings(rng)),
        ("apikey", settings(rng)),
        ("jwt", settings(rng)),
    ];
    object(fields, rng)
}

fn postman_url(rng: &mut Rng) -> Value {
    if rng.chance(40) {
        return text(rng);
    }
    let fields = vec![
        ("raw", text(rng)),
        ("protocol", text(rng)),
        (
            "host",
            if rng.chance(50) {
                list(rng, 3, text)
            } else {
                text(rng)
            },
        ),
        ("port", text(rng)),
        (
            "path",
            list(rng, 3, |rng| {
                if rng.chance(30) {
                    object(vec![("value", text(rng))], rng)
                } else {
                    text(rng)
                }
            }),
        ),
        ("query", key_values(rng)),
    ];
    object(fields, rng)
}

fn postman_body(rng: &mut Rng) -> Value {
    let mode = *rng.pick(&[
        "raw",
        "graphql",
        "formdata",
        "urlencoded",
        "file",
        "none",
        "",
    ]);
    let fields = vec![
        ("mode", w(json!(mode), rng)),
        ("raw", text(rng)),
        (
            "options",
            object(
                vec![("raw", object(vec![("language", text(rng))], rng))],
                rng,
            ),
        ),
        ("formdata", key_values(rng)),
        ("urlencoded", key_values(rng)),
        (
            "graphql",
            object(
                vec![
                    ("query", text(rng)),
                    (
                        "variables",
                        if rng.chance(50) {
                            text(rng)
                        } else {
                            junk(rng, 2)
                        },
                    ),
                ],
                rng,
            ),
        ),
    ];
    object(fields, rng)
}

fn postman_item(rng: &mut Rng, depth: usize) -> Value {
    if depth > 0 && rng.chance(30) {
        let fields = vec![
            ("name", text(rng)),
            ("item", list(rng, 3, |rng| postman_item(rng, depth - 1))),
            ("auth", postman_auth(rng)),
        ];
        return object(fields, rng);
    }
    let request = if rng.chance(15) {
        text(rng)
    } else {
        let fields = vec![
            ("method", text(rng)),
            ("url", postman_url(rng)),
            ("header", key_values(rng)),
            ("body", postman_body(rng)),
            ("auth", postman_auth(rng)),
        ];
        object(fields, rng)
    };
    object(vec![("name", text(rng)), ("request", request)], rng)
}

fn postman_collection(rng: &mut Rng) -> Value {
    let info = object(
        vec![
            ("_postman_id", text(rng)),
            ("name", text(rng)),
            (
                "schema",
                json!("https://schema.getpostman.com/json/collection/v2.1.0/collection.json"),
            ),
        ],
        rng,
    );
    let fields = vec![
        ("info", info),
        ("item", list(rng, 4, |rng| postman_item(rng, 3))),
        ("auth", postman_auth(rng)),
    ];
    object(fields, rng)
}

fn schema(rng: &mut Rng, depth: usize) -> Value {
    let kind = *rng.pick(&[
        "object", "array", "string", "number", "integer", "boolean", "null", "",
    ]);
    let mut fields = vec![
        ("type", w(json!(kind), rng)),
        ("example", junk(rng, 1)),
        ("default", junk(rng, 1)),
    ];
    if rng.chance(20) {
        let target = *rng.pick(&[
            "#/components/schemas/A",
            "#/components/schemas/B",
            "#/definitions/Self",
            "#",
            "",
        ]);
        fields.push(("$ref", json!(target)));
    }
    if depth > 0 {
        fields.push((
            "properties",
            object(
                vec![
                    ("a", schema(rng, depth - 1)),
                    ("ü", schema(rng, depth - 1)),
                    ("", schema(rng, depth - 1)),
                ],
                rng,
            ),
        ));
        fields.push(("items", schema(rng, depth - 1)));
    }
    object(fields, rng)
}

fn parameter(rng: &mut Rng) -> Value {
    let location = *rng.pick(&["query", "header", "path", "body", "cookie", "formData"]);
    let fields = vec![
        ("name", text(rng)),
        ("in", w(json!(location), rng)),
        ("required", w(json!(rng.chance(50)), rng)),
        ("example", junk(rng, 1)),
        ("schema", schema(rng, 2)),
    ];
    object(fields, rng)
}

fn operation(rng: &mut Rng) -> Value {
    let content_type = *rng.pick(&[
        "application/json",
        "*/*",
        "text/plain",
        "application/xml",
        "application/vnd.api+json",
    ]);
    let content = {
        let mut map = Map::new();
        map.insert(
            content_type.to_owned(),
            object(vec![("schema", schema(rng, 3))], rng),
        );
        w(Value::Object(map), rng)
    };
    let fields = vec![
        ("summary", text(rng)),
        ("operationId", text(rng)),
        ("parameters", list(rng, 4, parameter)),
        ("requestBody", object(vec![("content", content)], rng)),
    ];
    object(fields, rng)
}

fn openapi_spec(rng: &mut Rng) -> Value {
    let mut paths = Map::new();
    for _ in 0..rng.below(4) {
        let path = rng.pick(&["/pets", "/pets/{id}", "/ü/😀", "", "/a?b=1", "pets"]);
        let mut item = Map::new();
        for method in [
            "get", "post", "put", "patch", "delete", "head", "options", "trace",
        ] {
            if rng.chance(30) {
                item.insert(method.to_owned(), operation(rng));
            }
        }
        if rng.chance(20) {
            item.insert("$ref".to_owned(), json!("#/paths/~1pets"));
        }
        paths.insert((*path).to_owned(), w(Value::Object(item), rng));
    }
    let components = json!({
        "schemas": {
            "A": {"$ref": "#/components/schemas/B"},
            "B": {"type": "object", "properties": {"a": {"$ref": "#/components/schemas/A"}}},
        }
    });
    let mut fields = vec![
        ("info", object(vec![("title", text(rng))], rng)),
        ("paths", w(Value::Object(paths), rng)),
        (
            "servers",
            list(rng, 2, |rng| object(vec![("url", text(rng))], rng)),
        ),
        ("host", text(rng)),
        ("basePath", text(rng)),
        ("schemes", list(rng, 2, text)),
        ("components", components),
    ];
    fields.push(if rng.chance(50) {
        ("openapi", w(json!("3.0.3"), rng))
    } else {
        ("swagger", w(json!("2.0"), rng))
    });
    object(fields, rng)
}

fn httpie_export_file(rng: &mut Rng) -> Value {
    let fields = vec![
        ("name", text(rng)),
        (
            "requests",
            list(rng, 4, |rng| {
                let headers = object(
                    vec![
                        ("Accept", text(rng)),
                        ("!X-Off", text(rng)),
                        ("", text(rng)),
                    ],
                    rng,
                );
                let fields = vec![
                    ("name", text(rng)),
                    ("method", text(rng)),
                    ("url", text(rng)),
                    ("headers", headers),
                    ("body", text(rng)),
                    ("body_type", text(rng)),
                ];
                object(fields, rng)
            }),
        ),
    ];
    object(fields, rng)
}

/// A random edit somewhere inside `value`: a node swapped for junk, a key
/// dropped, an array item duplicated.
fn mutate_value(rng: &mut Rng, value: &mut Value) {
    match value {
        Value::Object(map) if !map.is_empty() && rng.chance(70) => {
            let key = map
                .keys()
                .nth(rng.below(map.len()))
                .cloned()
                .expect("in range");
            match rng.below(4) {
                0 => {
                    map.remove(&key);
                }
                1 => {
                    map.insert(key, junk(rng, 2));
                }
                _ => mutate_value(rng, map.get_mut(&key).expect("present")),
            }
        }
        Value::Array(items) if !items.is_empty() && rng.chance(70) => {
            let i = rng.below(items.len());
            match rng.below(4) {
                0 => {
                    items.remove(i);
                }
                1 => {
                    let copy = items[i].clone();
                    items.push(copy);
                }
                _ => mutate_value(rng, &mut items[i]),
            }
        }
        _ => *value = junk(rng, 2),
    }
}

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const FIXTURES: &[&str] = &[
    "openapi-generator-sample.json",
    "petstore-oas3.json",
    "petstore-swagger2.json",
    "postman-v2.0-users.json",
    "postman-v2.1-storefront.json",
    "storefront-oas3.json",
];

// ── No panics ────────────────────────────────────────────────────────────────

/// Every entry point a dropped file can reach. Errors must say something.
fn import_everything(text: &str) {
    let registry = FormatRegistry::standard();
    let ranked = registry.ranked(text);
    assert_eq!(ranked.len(), 4, "every importer is ranked");
    let check = |what: &str, result: Result<(), String>| {
        if let Err(message) = result {
            assert!(
                !message.trim().is_empty(),
                "{what} failed without a message"
            );
        }
    };
    check("import_collection", import_collection(text).map(drop));
    check("postman", postman::import(text).map(drop));
    check("openapi", swagger::import(text).map(drop));
    check("httpie export", httpie_export::import(text).map(drop));
    check("native", native_import(text).map(drop));
}

fn as_text(rng: &mut Rng, value: &Value) -> String {
    match rng.below(4) {
        0 => serde_yaml::to_string(value).expect("JSON values serialize as YAML"),
        1 => serde_json::to_string_pretty(value).expect("a JSON value serializes"),
        _ => serde_json::to_string(value).expect("a JSON value serializes"),
    }
}

#[test]
fn random_bytes_never_panic() {
    fuzz(
        0x1_0001,
        cases(3000),
        |rng| {
            let len = rng.below(300);
            String::from_utf8_lossy(&rng.bytes(len)).into_owned()
        },
        import_everything,
    );
}

#[test]
fn postman_shaped_values_never_panic() {
    fuzz(
        0x1_0002,
        cases(2000),
        |rng| {
            let value = postman_collection(rng);
            as_text(rng, &value)
        },
        import_everything,
    );
}

#[test]
fn openapi_shaped_values_never_panic() {
    fuzz(
        0x1_0003,
        cases(500),
        |rng| {
            let value = openapi_spec(rng);
            as_text(rng, &value)
        },
        import_everything,
    );
}

#[test]
fn httpie_export_shaped_values_never_panic() {
    fuzz(
        0x1_0004,
        cases(3000),
        |rng| {
            let value = httpie_export_file(rng);
            as_text(rng, &value)
        },
        import_everything,
    );
}

#[test]
fn mutated_fixtures_never_panic() {
    let fixtures: Vec<Value> = FIXTURES
        .iter()
        .map(|name| serde_json::from_str(&fixture(name)).expect("fixtures are JSON"))
        .chain([serde_json::from_str(&native_export_all(
            &[
                rustman_testkit::sample_collection(),
                rustman_testkit::second_collection(),
            ],
            WITH_SECRETS,
        ))
        .expect("native export is JSON")])
        .collect();
    fuzz(
        0x1_0005,
        cases(600),
        |rng| {
            let mut value = rng.pick(&fixtures).clone();
            for _ in 0..rng.range(1, 4) {
                mutate_value(rng, &mut value);
            }
            as_text(rng, &value)
        },
        import_everything,
    );
}

#[test]
fn mutated_fixture_text_never_panics() {
    let texts: Vec<String> = FIXTURES.iter().map(|name| fixture(name)).collect();
    fuzz(
        0x1_0006,
        cases(300),
        |rng| {
            let base = rng.pick(&texts).clone();
            rng.mutate(&base)
        },
        import_everything,
    );
}

/// YAML the JSON generators can't reach: anchors and aliases, tags,
/// multiple documents, complex keys, block scalars.
fn yaml_soup(rng: &mut Rng) -> String {
    const PIECES: &[&str] = &[
        "openapi: 3.0.0\n",
        "swagger: '2.0'\n",
        "info: {title: T}\n",
        "info: &i {title: T}\n",
        "paths:\n",
        "  /a:\n    get: {summary: s}\n",
        "  /b: *i\n",
        "  /c: &p\n    post:\n      requestBody: {content: {application/json: {schema: {type: object}}}}\n",
        "  /d: *p\n",
        "  /e: *missing\n",
        "x: &a [1, *a]\n",
        "y: &b {k: *b}\n",
        "? [complex, key]\n: value\n",
        "? {a: 1}\n: v\n",
        "1: one\n",
        "null: n\n",
        "~: tilde\n",
        "t: !!binary aGk=\n",
        "u: !custom {a: 1}\n",
        "v: !!float .nan\n",
        "w: !!int 99999999999999999999999\n",
        "z: |\n  block\n  text\n",
        "zz: >-\n  folded\n",
        "---\n",
        "...\n",
        "%YAML 1.2\n---\n",
        "<<: *i\n",
        "- item\n",
        "  - nested\n",
        "\t\ttab: x\n",
        "key: 'unterminated\n",
        "key: \"\\uD800\"\n",
        "emoji😀: 中\n",
        "&anchor_only\n",
        "*alias_only\n",
        "[",
        "{",
        "}",
        "]",
        ": :\n",
    ];
    (0..rng.range(1, 10)).map(|_| *rng.pick(PIECES)).collect()
}

#[test]
fn yaml_soup_never_panics() {
    fuzz(0x1_0007, cases(4000), yaml_soup, import_everything);
}

// ── Deep and huge ────────────────────────────────────────────────────────────

/// Nesting far past any sane file. The parsers' recursion limits must turn
/// these into errors; a stack overflow would abort the app.
#[test]
fn deep_nesting_is_an_error_not_a_stack_overflow() {
    // Far past every parser's recursion limit; a long run goes to 10k (slow:
    // see `deep_nesting_is_rejected_in_linear_time`).
    let depth = size(1_000, 10_000);
    let shapes: Vec<(&str, String)> = vec![
        (
            "JSON arrays",
            format!("{}{}", "[".repeat(depth), "]".repeat(depth)),
        ),
        (
            "JSON objects",
            format!("{}1{}", "{\"a\":".repeat(depth), "}".repeat(depth)),
        ),
        (
            "Postman folders",
            format!(
                "{{\"info\":{{\"name\":\"n\",\"_postman_id\":\"x\"}},\"item\":{}{}}}",
                "[{\"name\":\"f\",\"item\":".repeat(depth),
                "[]}]".repeat(depth)
            ),
        ),
        (
            "OpenAPI schema",
            format!(
                "{{\"openapi\":\"3.0.0\",\"info\":{{}},\"paths\":{{\"/a\":{{\"post\":{{\"requestBody\":{{\"content\":{{\"application/json\":{{\"schema\":{}{}}}}}}}}}}}}}}}",
                "{\"type\":\"array\",\"items\":".repeat(depth),
                "{}".to_owned() + &"}".repeat(depth)
            ),
        ),
        (
            "YAML flow sequences",
            format!(
                "openapi: 3.0.0\nx: {}{}",
                "[".repeat(depth),
                "]".repeat(depth)
            ),
        ),
        (
            "YAML flow mappings",
            format!(
                "openapi: 3.0.0\nx: {}1{}",
                "{a: ".repeat(depth),
                "}".repeat(depth)
            ),
        ),
        (
            "YAML block sequences",
            format!("openapi: 3.0.0\nx:\n  {}x\n", "- ".repeat(depth)),
        ),
        (
            "YAML self-referencing anchor",
            "openapi: 3.0.0\ninfo: {}\npaths: &p {a: *p}\n".to_owned(),
        ),
    ];
    for (label, text) in shapes {
        let start = Instant::now();
        import_everything(&text);
        eprintln!("{label}: {:?}", start.elapsed());
    }
}

/// OpenAPI schemas nested as deep as the JSON parser allows: example
/// building recurses per level and re-parses at each array level.
#[test]
fn schemas_at_the_parser_depth_limit_import() {
    for depth in [60, 126] {
        let levels = depth / 2;
        let text = format!(
            "{{\"openapi\":\"3.0.0\",\"info\":{{}},\"paths\":{{\"/a\":{{\"post\":{{\"requestBody\":{{\"content\":{{\"application/json\":{{\"schema\":{}{}}}}}}}}}}}}}}}",
            "{\"type\":\"array\",\"items\":".repeat(levels),
            "{\"type\":\"string\"}".to_owned() + &"}".repeat(levels)
        );
        let _ = swagger::import(&text);
    }
}

#[test]
fn huge_collections_import_in_reasonable_time() {
    let n = size(5_000, 100_000);
    let items: Vec<Value> = (0..n)
        .map(|i| {
            json!({"name": format!("r{i}"), "request": {"method": "POST", "url": {"raw": format!("https://x.io/{i}?a=1"), "query": [{"key": "a", "value": "1"}]}, "header": [{"key": "A", "value": "b"}], "body": {"mode": "raw", "raw": "{\"a\":1}"}}})
        })
        .collect();
    let postman_text =
        json!({"info": {"name": "big", "_postman_id": "x"}, "item": items}).to_string();
    let mut paths = Map::new();
    for i in 0..n {
        paths.insert(
            format!("/p{i}"),
            json!({"get": {"parameters": [{"name": "q", "in": "query", "required": true, "example": i}]}}),
        );
    }
    let openapi_text =
        json!({"openapi": "3.0.0", "info": {"title": "big"}, "paths": paths}).to_string();
    let array_text = format!("[{}]", vec!["1"; n * 20].join(","));
    for (label, text, requests) in [
        ("Postman", &postman_text, Some(n)),
        ("OpenAPI", &openapi_text, Some(n)),
        ("a huge array", &array_text, None),
    ] {
        let start = Instant::now();
        let imported = import_collection(text);
        let elapsed = start.elapsed();
        eprintln!("{label}: {} bytes in {elapsed:?}", text.len());
        let count = imported.ok().map(|outcome| {
            outcome
                .collections
                .iter()
                .map(|c| c.all_requests().len())
                .sum::<usize>()
        });
        assert_eq!(count, requests, "{label}: every request, or an error");
        // One debug-build reading on a loaded machine says little about the
        // budget; long runs and release builds hold it.
        if long_run() || !cfg!(debug_assertions) {
            assert!(
                elapsed < Duration::from_secs(if n > 10_000 { 60 } else { 5 }),
                "{label}: {elapsed:?}"
            );
        }
    }
}

/// A YAML alias expands to a full copy of its anchor every time it is used,
/// and `serde_yaml` only caps the *number* of alias jumps (100 per event), not
/// their size. An anchor of `n` items aliased `n` times becomes `n * n`
/// values from roughly `5n` bytes of text.
fn alias_bomb(n: usize) -> String {
    format!(
        "openapi: 3.0.0\ninfo: {{title: t}}\npaths: {{}}\na: &a [{}]\nb: [{}]\n",
        vec!["x"; n].join(","),
        vec!["*a"; n].join(",")
    )
}

#[test]
fn yaml_aliases_do_not_expand_quadratically() {
    // Each of these expands past the value budget, so it is refused, not
    // built: the work stays proportional to the text.
    for n in [250, 500, 1000] {
        let err = swagger::import(&alias_bomb(n)).unwrap_err();
        assert!(err.contains("too many values"), "{n}: {err}");
    }

    // 4x the text: linear would be ~4x, the expansion ~16x.
    assert_linear("alias bomb", 250, alias_bomb, None, |text| {
        let _ = swagger::import(text);
    });
}

// ── Sniffing ─────────────────────────────────────────────────────────────────

#[test]
fn sniffing_any_prefix_never_panics() {
    let mut texts: Vec<String> = FIXTURES.iter().map(|name| fixture(name)).collect();
    texts.push(native_export_all(
        &[rustman_testkit::sample_collection()],
        ExportOptions::default(),
    ));
    let registry = FormatRegistry::standard();
    for text in &texts {
        for (i, _) in text.char_indices() {
            let prefix = &text[..i];
            // Sniffing reads the first 4 KB, so longer prefixes add nothing.
            if i <= 4200 {
                let _ = registry.ranked(prefix);
            }
            if i % size(4999, 97) == 0 {
                import_everything(prefix);
            }
        }
        // Byte cuts in the middle of a character, as a truncated download
        // would make them.
        for cut in (0..text.len()).step_by(size(499, 7)) {
            import_everything(&String::from_utf8_lossy(&text.as_bytes()[..cut]));
        }
    }
}

#[test]
fn the_sniff_window_can_end_inside_a_character() {
    let registry = FormatRegistry::standard();
    for wide in ["é", "€", "😀", "\u{301}"] {
        for pad in 4090..4100 {
            let text = format!(
                "{{\"info\": {{\"_postman_id\": \"x\", \"name\": \"{}{}\"}}, \"item\": []}}",
                "a".repeat(pad - 30),
                wide.repeat(8)
            );
            let ranked = registry.ranked(&text);
            assert_eq!(ranked[0].0.name(), "Postman collection");
            assert!(import_collection(&text).is_ok());
        }
    }
}

// ── Export → import round trips ──────────────────────────────────────────────

/// Round trips compare every field, credentials included.
const WITH_SECRETS: ExportOptions = ExportOptions {
    include_secrets: true,
};

fn plain(rng: &mut Rng) -> String {
    match rng.below(3) {
        0 => rng.text(12),
        1 => rng.text_from(
            &['a', 'B', '-', '_', ' ', '&', '=', '%', '+', '?', '#', 'é'],
            10,
        ),
        _ => (*rng.pick(&["", "x", "{{var}}", "a b", "100%", "x+y"])).to_owned(),
    }
}

fn row(rng: &mut Rng, i: usize) -> KeyValue {
    KeyValue {
        id: format!("row-{i}"),
        key: plain(rng),
        value: plain(rng),
        enabled: rng.chance(80),
    }
}

fn random_request(rng: &mut Rng, i: usize) -> SavedRequest {
    let mut req = rustman_testkit::blank_request(&format!("req-{i}"), &plain(rng));
    req.method = rng.pick(HttpMethod::all()).clone();
    req.url = format!(
        "https://x.io/{}",
        rng.text_from(&['a', '/', '{', '}', 'é', '%', '-'], 10)
    );
    req.params = (0..rng.below(4)).map(|j| row(rng, j)).collect();
    req.headers = (0..rng.below(4)).map(|j| row(rng, j)).collect();
    req.body_type = rng
        .pick(&[
            BodyType::None,
            BodyType::Json,
            BodyType::Text,
            BodyType::FormData,
            BodyType::GraphQL,
        ])
        .clone();
    req.body = plain(rng);
    (*rng.pick(&["", "{\"a\": 1}", "{oops"])).clone_into(&mut req.graphql_variables);
    req.form_data_fields = (0..rng.below(4))
        .map(|j| {
            let file = rng.chance(30);
            FormField {
                id: format!("f-{j}"),
                key: plain(rng),
                value: if file { String::new() } else { plain(rng) },
                field_type: if file {
                    FormFieldType::File
                } else {
                    FormFieldType::Text
                },
                enabled: rng.chance(80),
                file_name: file.then(|| plain(rng)).filter(|n| !n.is_empty()),
                file_data: None,
                mime_type: rng.chance(30).then(|| "image/png".to_owned()),
            }
        })
        .collect();
    req.auth_type = rng
        .pick(&[
            AuthType::None,
            AuthType::Basic,
            AuthType::Bearer,
            AuthType::ApiKey,
            AuthType::JwtUser,
        ])
        .clone();
    req.bearer_token = plain(rng);
    req.basic_user = plain(rng);
    req.basic_pass = plain(rng);
    req.api_key_name = plain(rng);
    req.api_key_value = plain(rng);
    req.api_key_location = if rng.chance(50) {
        ApiKeyLocation::Header
    } else {
        ApiKeyLocation::Query
    };
    req.jwt_secret = plain(rng);
    req.jwt_subject = plain(rng);
    req
}

fn random_collection(rng: &mut Rng) -> ImportedCollection {
    let collection = Collection {
        id: "col".into(),
        name: plain(rng),
        created_at: 1,
    };
    let requests = (0..rng.below(4)).map(|i| random_request(rng, i)).collect();
    let groups = (0..rng.below(3))
        .map(|g| ImportedGroup {
            id: format!("g-{g}"),
            parent_id: "col".into(),
            parent_is_collection: true,
            name: plain(rng),
            sort_order: 0,
            requests: (0..rng.below(3))
                .map(|i| random_request(rng, 10 + i))
                .collect(),
            groups: Vec::new(),
        })
        .collect();
    ImportedCollection {
        collection,
        requests,
        groups,
    }
}

#[test]
fn native_export_imports_back_identically() {
    let mut rng = Rng::new(0x1_0010);
    for _ in 0..cases(500) {
        let collections: Vec<ImportedCollection> = (0..rng.range(1, 3))
            .map(|_| random_collection(&mut rng))
            .collect();
        let text = native_export_all(&collections, WITH_SECRETS);
        let back = native_import(&text).expect("a native export imports");
        assert_eq!(native_export_all(&back, WITH_SECRETS), text);
    }
}

#[test]
fn an_empty_native_export_imports_back() {
    let text = native_export_all(&[], ExportOptions::default());
    assert_eq!(native_import(&text).map(|c| c.len()), Ok(0));
}

/// What a request sends, as a comparable value.
fn sent(req: &SavedRequest) -> String {
    let resolved = req.resolve(None);
    format!(
        "{} {}\nheaders: {:?}\nauth: {:?}\nbody: {:?}",
        resolved.method,
        resolved.full_url(),
        resolved.headers,
        resolved.auth,
        resolved.body
    )
}

fn all_requests(collection: &ImportedCollection) -> Vec<&SavedRequest> {
    let mut out: Vec<&SavedRequest> = collection
        .groups
        .iter()
        .flat_map(|g| g.requests.iter())
        .collect();
    out.extend(&collection.requests);
    out
}

#[test]
fn postman_export_imports_back_as_the_same_requests() {
    let mut rng = Rng::new(0x1_0011);
    for _ in 0..cases(800) {
        let collection = random_collection(&mut rng);
        let text = postman::export(&collection, WITH_SECRETS);
        let back = postman::import(&text).expect("a Postman export imports");
        let back = &back.collections[0];
        let before: Vec<String> = all_requests(&collection).into_iter().map(sent).collect();
        let after: Vec<String> = all_requests(back).into_iter().map(sent).collect();
        assert_eq!(before.len(), after.len(), "{text}");
        for (b, a) in before.iter().zip(&after) {
            assert_eq!(b, a, "\n{text}");
        }
    }
}

/// Each flow-collection level costs the YAML scanner work on every later
/// token, so deep `{"a":` nesting would take quadratic time there. The
/// OpenAPI importer, which the registry tries for every file whatever it
/// sniffs as, refuses JSON nested past `serde_json`'s limit of 128 as JSON
/// rather than retrying it as YAML.
#[test]
fn deep_nesting_is_rejected_in_linear_time() {
    let nested = |depth: usize| format!("{}1{}", "{\"a\":".repeat(depth), "}".repeat(depth));
    let err = swagger::import(&nested(5_000)).unwrap_err();
    assert!(
        err.contains("Not valid JSON") && err.contains("recursion limit"),
        "{err}"
    );
    assert!(import_collection(&nested(5_000)).is_err());

    assert_linear("nesting depth", 8_000, nested, None, |text| {
        let _ = import_collection(text);
    });
}

/// Every credential of every request in `collection` set to `value`: the
/// auth fields, the cookie rows and an `authorization` header row.
fn set_credentials(collection: &mut ImportedCollection, value: &str) {
    fn set(request: &mut SavedRequest, value: &str) {
        for field in [
            &mut request.bearer_token,
            &mut request.basic_pass,
            &mut request.api_key_value,
            &mut request.jwt_secret,
            &mut request.cookie_string,
        ] {
            value.clone_into(field);
        }
        for row in &mut request.cookies {
            value.clone_into(&mut row.value);
        }
        request.headers.push(KeyValue {
            id: "credential".into(),
            key: "authorization".into(),
            value: value.to_owned(),
            enabled: true,
        });
    }
    fn walk(group: &mut ImportedGroup, value: &str) {
        group.requests.iter_mut().for_each(|r| set(r, value));
        group.groups.iter_mut().for_each(|g| walk(g, value));
    }
    collection.requests.iter_mut().for_each(|r| set(r, value));
    collection.groups.iter_mut().for_each(|g| walk(g, value));
}

#[test]
fn exports_leave_credentials_out_unless_asked() {
    type Export = fn(&ImportedCollection, ExportOptions) -> String;
    let exports: [(&str, Export); 2] = [("native", native_export), ("Postman", postman::export)];
    for (format, export) in exports {
        let mut secret = rustman_testkit::sample_collection();
        set_credentials(&mut secret, "s3cr3t-marker");
        let shared = export(&secret, ExportOptions::default());
        assert!(!shared.contains("s3cr3t-marker"), "{format}: {shared}");
        assert!(
            shared.contains("authorization"),
            "{format}: header names stay"
        );
        assert!(
            export(&secret, WITH_SECRETS).contains("s3cr3t-marker"),
            "{format}"
        );

        let mut reference = rustman_testkit::sample_collection();
        set_credentials(&mut reference, "{{vault_token}}");
        assert_eq!(
            export(&reference, ExportOptions::default()),
            export(&reference, WITH_SECRETS),
            "{format}: a reference is not a secret"
        );
    }
}

#[test]
fn a_shared_native_export_imports_as_the_collection_without_its_secrets() {
    let mut rng = Rng::new(0x1_0012);
    for _ in 0..cases(300) {
        let collection = random_collection(&mut rng);
        let shared = native_import(&native_export_all(
            std::slice::from_ref(&collection),
            ExportOptions::default(),
        ))
        .expect("a native export imports");
        let mut expected = collection;
        strip_collection_secrets(&mut expected);
        assert_eq!(
            native_export_all(&shared, WITH_SECRETS),
            native_export_all(&[expected], WITH_SECRETS)
        );
    }
}
