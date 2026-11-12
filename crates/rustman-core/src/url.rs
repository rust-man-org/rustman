//! Percent-encoding for URLs and query strings.

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// `s` with every byte outside RFC 3986's unreserved set percent-encoded,
/// the way curl's `--data-urlencode` does it.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 0x0f)]));
        }
    }
    out
}

/// `s` with only the characters that would break query parsing encoded, so
/// a URL rebuilt from the params table stays readable.
pub(crate) fn encode_query_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '&' => out.push_str("%26"),
            '=' => out.push_str("%3D"),
            '#' => out.push_str("%23"),
            '%' => out.push_str("%25"),
            '+' => out.push_str("%2B"),
            _ => out.push(c),
        }
    }
    out
}

/// `s` with `+` and `%XX` escapes decoded. A malformed escape, or one that
/// isn't part of a UTF-8 character, stays as written.
pub(crate) fn decode_query_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(['%', '+']) {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(after) = rest.strip_prefix('+') {
            out.push(' ');
            rest = after;
            continue;
        }
        let bytes = leading_escapes(rest);
        let (escapes, after) = rest.split_at((3 * bytes.len()).max(1));
        if bytes.is_empty() {
            out.push_str(escapes);
        } else {
            push_decoded(&mut out, &bytes, escapes);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The bytes of the `%XX` escapes at the start of `s`.
fn leading_escapes(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<3>()
        .0
        .iter()
        .map_while(|&escape| match escape {
            [b'%', hi, lo] => Some((hex_digit(hi)? << 4) | hex_digit(lo)?),
            _ => None,
        })
        .collect()
}

/// Appends `bytes`, decoded from `escapes`, keeping the escape of every byte
/// that isn't part of a UTF-8 character.
fn push_decoded(out: &mut String, bytes: &[u8], escapes: &str) {
    let mut at = 0;
    for chunk in bytes.utf8_chunks() {
        out.push_str(chunk.valid());
        at += chunk.valid().len();
        let end = at + chunk.invalid().len();
        out.push_str(&escapes[3 * at..3 * end]);
        at = end;
    }
}

/// `url` split before its `#fragment`; the second part keeps the `#`.
pub(crate) fn split_fragment(url: &str) -> (&str, &str) {
    url.split_at(url.find('#').unwrap_or(url.len()))
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_keeps_only_unreserved_characters() {
        assert_eq!(percent_encode("a-b.c_d~e"), "a-b.c_d~e");
        assert_eq!(percent_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    }

    #[test]
    fn malformed_escapes_stay_literal() {
        assert_eq!(decode_query_component("%zz%4"), "%zz%4");
        assert_eq!(decode_query_component("%ff"), "%ff");
        assert_eq!(decode_query_component("%"), "%");
    }

    #[test]
    fn each_escape_decodes_on_its_own() {
        assert_eq!(decode_query_component("a%FF+b%20c%2B"), "a%FF b c+");
        assert_eq!(decode_query_component("caf%C3%A9%C3"), "café%C3");
        assert_eq!(decode_query_component("%E2%82%AC%e9x"), "€%e9x");
        assert_eq!(decode_query_component("%41%E9%42"), "A%E9B");
    }
}
