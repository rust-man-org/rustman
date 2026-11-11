//! The collection-file importers, and picking the one a file is meant for.

use rustman_core::text::head_capped;

use super::{ImportOutcome, httpie_export, native_import, postman, swagger};

/// Bytes of a file the importers look at to recognise it.
const SNIFF_BYTES: usize = 4096;

/// How sure an importer is that a file is in its format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    No,
    Possible,
    Certain,
}

/// A collection-file format.
pub trait Importer: Send + Sync {
    /// The format as the user knows it, e.g. "Postman collection".
    fn name(&self) -> &'static str;
    /// How sure this importer is, from the start of the file.
    fn sniff(&self, head: &str) -> Confidence;
    fn import(&self, text: &str) -> Result<ImportOutcome, String>;
}

/// The importers the app knows, tried most confident first.
pub struct FormatRegistry {
    importers: Vec<Box<dyn Importer>>,
}

impl FormatRegistry {
    /// Postman, Rustman, HTTPie exports and OpenAPI/Swagger.
    pub fn standard() -> Self {
        Self {
            importers: vec![
                Box::new(Postman),
                Box::new(Native),
                Box::new(HttpieExport),
                Box::new(OpenApi),
            ],
        }
    }

    /// This registry with `importer` added, tried after the existing ones
    /// when equally confident.
    #[must_use]
    pub fn with(mut self, importer: impl Importer + 'static) -> Self {
        self.importers.push(Box::new(importer));
        self
    }

    /// The importers in the order they are tried for `text`: most confident
    /// first, ties in registration order.
    pub fn ranked(&self, text: &str) -> Vec<(&dyn Importer, Confidence)> {
        let head = head_capped(text, SNIFF_BYTES);
        let mut ranked: Vec<(&dyn Importer, Confidence)> = self
            .importers
            .iter()
            .map(|importer| (importer.as_ref(), importer.sniff(head)))
            .collect();
        ranked.sort_by_key(|&(_, confidence)| std::cmp::Reverse(confidence));
        ranked
    }

    /// `text` imported by the first importer, most confident first, that can
    /// read it. Once one is certain of the file, the unsure ones aren't
    /// tried. When none can read it, the error comes from the format the file
    /// most resembles, so a broken Postman file says what is wrong with it
    /// rather than why it isn't OpenAPI. A leading byte order mark, as
    /// Windows editors write, is not part of the document.
    pub fn import(&self, text: &str) -> Result<ImportOutcome, String> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let ranked = self.ranked(text);
        let certain = ranked
            .first()
            .is_some_and(|&(_, confidence)| confidence == Confidence::Certain);
        let candidates = ranked
            .into_iter()
            .take_while(|&(_, confidence)| !certain || confidence == Confidence::Certain);
        let mut first_error = None;
        for (importer, confidence) in candidates {
            match importer.import(text) {
                Ok(outcome) => return Ok(outcome),
                Err(e) if first_error.is_none() && confidence > Confidence::No => {
                    first_error = Some(format!("This looks like a {}, but {e}", importer.name()));
                }
                Err(_) => {}
            }
        }
        Err(first_error.unwrap_or_else(|| format!("Not a {}.", self.names())))
    }

    /// "A, B, C or D".
    fn names(&self) -> String {
        let names: Vec<&str> = self.importers.iter().map(|i| i.name()).collect();
        match names.split_last() {
            Some((last, [])) => (*last).to_owned(),
            Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
            None => "known format".to_owned(),
        }
    }
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::standard()
    }
}

struct Postman;

impl Importer for Postman {
    fn name(&self) -> &'static str {
        "Postman collection"
    }

    fn sniff(&self, head: &str) -> Confidence {
        if head.contains("_postman_id") || head.contains("getpostman.com") {
            Confidence::Certain
        } else {
            Confidence::No
        }
    }

    fn import(&self, text: &str) -> Result<ImportOutcome, String> {
        postman::import(text)
    }
}

struct Native;

impl Importer for Native {
    fn name(&self) -> &'static str {
        "Rustman collection"
    }

    fn sniff(&self, head: &str) -> Confidence {
        match (
            head.contains("\"collection\""),
            head.contains("\"created_at\""),
        ) {
            (true, true) => Confidence::Certain,
            (true, false) => Confidence::Possible,
            (false, _) => Confidence::No,
        }
    }

    fn import(&self, text: &str) -> Result<ImportOutcome, String> {
        native_import(text).map(Into::into)
    }
}

struct HttpieExport;

impl Importer for HttpieExport {
    fn name(&self) -> &'static str {
        "HTTPie export"
    }

    fn sniff(&self, head: &str) -> Confidence {
        if head.contains("\"requests\"") && !head.contains("\"collection\"") {
            Confidence::Possible
        } else {
            Confidence::No
        }
    }

    fn import(&self, text: &str) -> Result<ImportOutcome, String> {
        httpie_export::import(text).map(Into::into)
    }
}

struct OpenApi;

impl Importer for OpenApi {
    fn name(&self) -> &'static str {
        "OpenAPI spec"
    }

    fn sniff(&self, head: &str) -> Confidence {
        let markers = ["\"openapi\"", "\"swagger\"", "openapi:", "swagger:"];
        if markers.iter().any(|m| head.contains(m)) {
            Confidence::Certain
        } else {
            Confidence::No
        }
    }

    fn import(&self, text: &str) -> Result<ImportOutcome, String> {
        swagger::import(text)
    }
}

#[cfg(test)]
mod tests {
    use rustman_testkit::{sample_collection, second_collection};

    use super::*;
    use crate::import::{ExportOptions, native_export_all};

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// The importers sure of `text`, i.e. tied for the highest confidence.
    fn claimants(text: &str) -> Vec<&'static str> {
        let registry = FormatRegistry::standard();
        let ranked = registry.ranked(text);
        let top = ranked.first().map_or(Confidence::No, |&(_, c)| c);
        ranked
            .into_iter()
            .filter(|&(_, c)| c == top && c > Confidence::No)
            .map(|(importer, _)| importer.name())
            .collect()
    }

    #[test]
    fn every_fixture_is_claimed_by_exactly_one_importer() {
        let native = native_export_all(
            &[sample_collection(), second_collection()],
            ExportOptions::default(),
        );
        let httpie = r#"{"name": "API", "requests": [{"name": "Ping", "url": "https://x.io"}]}"#;
        let cases = [
            (fixture("postman-v2.0-users.json"), "Postman collection"),
            (
                fixture("postman-v2.1-storefront.json"),
                "Postman collection",
            ),
            (fixture("petstore-oas3.json"), "OpenAPI spec"),
            (fixture("petstore-swagger2.json"), "OpenAPI spec"),
            (fixture("storefront-oas3.json"), "OpenAPI spec"),
            (native, "Rustman collection"),
            (httpie.to_owned(), "HTTPie export"),
        ];
        for (text, format) in cases {
            assert_eq!(claimants(&text), [format]);
            assert!(FormatRegistry::standard().import(&text).is_ok(), "{format}");
        }
    }

    #[test]
    fn an_openapi_generator_request_is_not_mistaken_for_a_spec() {
        let err = FormatRegistry::standard()
            .import(&fixture("openapi-generator-sample.json"))
            .unwrap_err();
        assert_eq!(
            err,
            "This looks like a OpenAPI spec, but not an OpenAPI or Swagger spec \
             (missing 'openapi' or 'swagger' key)"
        );
    }

    #[test]
    fn an_added_importer_is_tried_when_it_is_sure() {
        struct Lines;
        impl Importer for Lines {
            fn name(&self) -> &'static str {
                "line list"
            }
            fn sniff(&self, head: &str) -> Confidence {
                if head.starts_with("#lines") {
                    Confidence::Certain
                } else {
                    Confidence::No
                }
            }
            fn import(&self, text: &str) -> Result<ImportOutcome, String> {
                text.starts_with("#lines")
                    .then(ImportOutcome::default)
                    .ok_or_else(|| "no #lines header".to_owned())
            }
        }
        let registry = FormatRegistry::standard().with(Lines);
        assert!(registry.import("#lines\nGET https://x.io").is_ok());
        let err = registry.import("hello").unwrap_err();
        assert_eq!(
            err,
            "Not a Postman collection, Rustman collection, HTTPie export, OpenAPI spec or line list."
        );
    }
}
