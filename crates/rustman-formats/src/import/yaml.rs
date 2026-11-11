//! YAML read into JSON values, with bounds that keep hostile files cheap.

use std::cell::Cell;
use std::fmt;

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

/// Values a document may hold per byte of its text. An alias repeats the
/// whole subtree of its anchor, so without a budget a few KB of aliases can
/// expand to gigabytes. Plain YAML holds at most one value per two bytes.
const VALUES_PER_BYTE: usize = 16;

/// Values any one document may hold: under 500 MB of them.
const MAX_VALUES: usize = 1 << 22;

/// The YAML scanner does work for every open `[` or `{` on each token, which
/// makes deep flow nesting quadratic. `serde_yaml` refuses anything nested
/// deeper than this anyway, but only after scanning all of it.
const MAX_FLOW_DEPTH: usize = 128;

pub(super) fn to_json(text: &str) -> Result<Value, String> {
    if nests_deeper_than(text, MAX_FLOW_DEPTH) {
        return Err(format!("nested more than {MAX_FLOW_DEPTH} levels deep"));
    }
    let budget = Cell::new(text.len().saturating_mul(VALUES_PER_BYTE).min(MAX_VALUES));
    Budgeted(&budget)
        .deserialize(serde_yaml::Deserializer::from_str(text))
        .map_err(|e| e.to_string())
}

/// Counts brackets inside strings and comments too, so it never undercounts.
fn nests_deeper_than(text: &str, limit: usize) -> bool {
    let mut depth = 0usize;
    for byte in text.bytes() {
        match byte {
            b'[' | b'{' => {
                depth += 1;
                if depth > limit {
                    return true;
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

/// Builds a [`Value`] the way `serde_json` does, spending one unit of the
/// shared budget per value.
#[derive(Clone, Copy)]
struct Budgeted<'a>(&'a Cell<usize>);

impl Budgeted<'_> {
    fn spend<E: de::Error>(self, value: Value) -> Result<Value, E> {
        let left = self.0.get().checked_sub(1).ok_or_else(|| {
            E::custom("the document expands to too many values (are its aliases nested?)")
        })?;
        self.0.set(left);
        Ok(value)
    }
}

impl<'de> DeserializeSeed<'de> for Budgeted<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Budgeted<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any YAML value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        self.spend(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        self.spend(Value::Number(v.into()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        self.spend(Value::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        self.spend(Number::from_f64(v).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        self.spend(Value::String(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        self.spend(Value::String(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        self.spend(Value::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        self.spend(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        self.deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(self)? {
            items.push(item);
        }
        self.spend(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut fields = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            fields.insert(key, map.next_value_seed(self)?);
        }
        self.spend(Value::Object(fields))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_yaml_as_serde_json_would() {
        let text = "a: [1, -2, 3.5, .inf, true, null, x]\n200: {b: ~}\nc: &c [y]\nd: *c\n";
        let expected: Value = serde_yaml::from_str(text).unwrap();
        assert_eq!(to_json(text).unwrap(), expected);
        assert_eq!(expected["d"], json!(["y"]));
    }

    #[test]
    fn deep_flow_nesting_is_refused_before_parsing() {
        let deep = format!("a: {}1{}", "[".repeat(129), "]".repeat(129));
        assert_eq!(
            to_json(&deep).unwrap_err(),
            "nested more than 128 levels deep"
        );
        let fine = format!("a: {}1{}", "[".repeat(100), "]".repeat(100));
        assert!(to_json(&fine).is_ok());
    }

    #[test]
    fn aliases_that_expand_past_the_budget_are_refused() {
        let text = format!(
            "a: &a [{}]\nb: [{}]\n",
            vec!["x"; 400].join(","),
            vec!["*a"; 400].join(",")
        );
        assert!(to_json(&text).unwrap_err().contains("too many values"));
    }
}
