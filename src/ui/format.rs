//! Allocation-free display strings for numbers the views show every frame.

use std::borrow::Cow;
use std::sync::LazyLock;

/// Counts below this come from a table instead of being formatted.
const TABLE: usize = 1000;

static COUNTS: LazyLock<Vec<String>> = LazyLock::new(|| (0..TABLE).map(|n| n.to_string()).collect());

/// `n` as text; borrowed for every count a tab badge realistically shows.
pub fn count(n: usize) -> Cow<'static, str> {
    match COUNTS.get(n) {
        Some(s) => Cow::Borrowed(s.as_str()),
        None => Cow::Owned(n.to_string()),
    }
}

/// Elapsed seconds with one decimal ("1.2 s"), tabled up to 100 s so the
/// loading timer formats nothing while it ticks.
pub fn seconds(ms: u128) -> Cow<'static, str> {
    static TENTHS: LazyLock<Vec<String>> = LazyLock::new(|| {
        (0..TABLE)
            .map(|d| format!("{}.{} s", d / 10, d % 10))
            .collect()
    });
    let tenths = (ms / 100) as usize;
    match TENTHS.get(tenths) {
        Some(s) => Cow::Borrowed(s.as_str()),
        None => Cow::Owned(format!("{} s", ms / 1000)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_borrowed_from_the_table() {
        assert!(matches!(count(6), Cow::Borrowed("6")));
        assert_eq!(count(123_456), "123456");
    }

    #[test]
    fn seconds_show_one_decimal_then_whole_seconds() {
        assert_eq!(seconds(0), "0.0 s");
        assert_eq!(seconds(1_249), "1.2 s");
        assert!(matches!(seconds(99_999), Cow::Borrowed("99.9 s")));
        assert_eq!(seconds(125_000), "125 s");
    }
}
