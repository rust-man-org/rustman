//! Shared pieces of the fuzz tests: a seeded generator, panic capture that
//! keeps the terminal quiet, input shrinking and a linear-time check.
//!
//! Every run is deterministic. `RUSTMAN_FUZZ_CASES` scales the case counts
//! up for long runs; the defaults keep each test well under two seconds in a
//! debug build.

#![allow(dead_code)]

use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;
use std::time::{Duration, Instant};

/// xorshift64*: small, fast and good enough to drive input generators.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self((seed ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub(crate) fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next_u64() % n as u64).expect("below n")
    }

    /// A number in `lo..=hi`.
    pub(crate) fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

    pub(crate) fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    pub(crate) fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next_u64().to_le_bytes()[0]).collect()
    }

    /// A character skewed towards what breaks text parsers: ASCII
    /// punctuation, controls, and multi-byte UTF-8 of every width.
    pub(crate) fn char(&mut self) -> char {
        const TRICKY: &[char] = &[
            '\0',
            '\t',
            '\n',
            '\r',
            '\u{7f}',
            '\u{85}',
            '\u{a0}',
            'é',
            'ß',
            '\u{301}',
            '\u{200d}',
            '\u{202e}',
            '\u{2028}',
            '\u{feff}',
            'א',
            'ب',
            '€',
            '中',
            '\u{fffd}',
            '😀',
            '🇺',
            '\u{10ffff}',
            '\u{e000}',
        ];
        match self.below(10) {
            0..=4 => char::from(u8::try_from(self.range(0x20, 0x7e)).expect("ascii")),
            5..=7 => *self.pick(TRICKY),
            8 => char::from_u32(u32::try_from(self.range(0x80, 0xd7ff)).expect("bmp"))
                .expect("below the surrogates"),
            _ => char::from_u32(u32::try_from(self.range(0x1_0000, 0x10_ffff)).expect("astral"))
                .expect("above the surrogates"),
        }
    }

    pub(crate) fn text(&mut self, max_len: usize) -> String {
        let len = self.below(max_len + 1);
        (0..len).map(|_| self.char()).collect()
    }

    /// Text drawn from `alphabet`, for grammar-aware noise.
    pub(crate) fn text_from(&mut self, alphabet: &[char], max_len: usize) -> String {
        let len = self.below(max_len + 1);
        (0..len).map(|_| *self.pick(alphabet)).collect()
    }

    /// `base` with a few random character-level edits.
    pub(crate) fn mutate(&mut self, base: &str) -> String {
        let mut chars: Vec<char> = base.chars().collect();
        for _ in 0..self.range(1, 8) {
            let at = self.below(chars.len() + 1);
            match self.below(5) {
                0 if at < chars.len() => {
                    chars.remove(at);
                }
                1 if at < chars.len() => chars[at] = self.char(),
                2 => {
                    let end = (at + self.range(1, 64)).min(chars.len());
                    chars.truncate(end.max(at));
                }
                3 if !chars.is_empty() => {
                    // Duplicate a slice somewhere else: repeats structure.
                    let start = self.below(chars.len());
                    let end = (start + self.range(1, 32)).min(chars.len());
                    let slice: Vec<char> = chars[start..end].to_vec();
                    let at = at.min(chars.len());
                    chars.splice(at..at, slice);
                }
                _ => chars.insert(at.min(chars.len()), self.char()),
            }
        }
        chars.into_iter().collect()
    }
}

/// The number of cases to run: `RUSTMAN_FUZZ_CASES` when set, else
/// `default`.
pub(crate) fn cases(default: usize) -> usize {
    std::env::var("RUSTMAN_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Whether this is a long run (`RUSTMAN_FUZZ_CASES` set to a number): timing
/// tests then use full-size inputs.
pub(crate) fn long_run() -> bool {
    std::env::var("RUSTMAN_FUZZ_CASES").is_ok_and(|v| v.parse::<usize>().is_ok())
}

/// `long` in a long run, else `short`.
pub(crate) fn size(short: usize, long: usize) -> usize {
    if long_run() { long } else { short }
}

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f`, returning its panic message if it panicked. The panic hook
/// stays quiet meanwhile, so shrinking doesn't flood the output.
pub(crate) fn panic_message<R>(f: impl FnOnce() -> R) -> Option<String> {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !QUIET.with(Cell::get) {
                default(info);
            }
        }));
    });
    QUIET.with(|q| q.set(true));
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    QUIET.with(|q| q.set(false));
    let payload = result.err()?;
    Some(
        payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_owned()),
    )
}

/// A smaller input that still `fails`, by greedy removal of ever smaller
/// chunks of characters.
pub(crate) fn shrink(input: &str, fails: impl Fn(&str) -> bool) -> String {
    let mut current: Vec<char> = input.chars().collect();
    let mut chunk = (current.len() / 2).max(1);
    loop {
        let mut i = 0;
        while i + chunk <= current.len() {
            let candidate: String = current[..i].iter().chain(&current[i + chunk..]).collect();
            if fails(&candidate) {
                current = candidate.chars().collect();
            } else {
                i += chunk;
            }
        }
        if chunk == 1 {
            break;
        }
        chunk /= 2;
    }
    current.into_iter().collect()
}

/// Runs `check` on `cases` generated inputs and fails with a shrunk
/// reproducer on the first panic.
pub(crate) fn fuzz(
    seed: u64,
    cases: usize,
    mut generate: impl FnMut(&mut Rng) -> String,
    check: impl Fn(&str),
) {
    let mut rng = Rng::new(seed);
    for case in 0..cases {
        let input = generate(&mut rng);
        if let Some(message) = panic_message(|| check(&input)) {
            let small = if input.len() <= 8192 {
                shrink(&input, |s| panic_message(|| check(s)).is_some())
            } else {
                input.chars().take(200).collect()
            };
            panic!(
                "case {case} of seed {seed:#x} panicked: {message}\n  minimal input: {small:?}\n  input length: {}",
                input.len()
            );
        }
    }
}

/// Runs `check` on `cases` generated inputs, failing on the first one it
/// rejects. Unlike [`fuzz`] it doesn't shrink: for properties of generated
/// structure, deleting characters can break the precondition the check
/// relies on.
pub(crate) fn check_all(
    seed: u64,
    cases: usize,
    mut generate: impl FnMut(&mut Rng) -> String,
    check: impl Fn(&str),
) {
    let mut rng = Rng::new(seed);
    for case in 0..cases {
        let input = generate(&mut rng);
        if let Some(message) = panic_message(|| check(&input)) {
            panic!("case {case} of seed {seed:#x} failed: {message}\n  input: {input:?}");
        }
    }
}

/// The fastest of five runs each of `small` and `large`, taken in turn so a
/// burst of load lands on both sizes instead of inflating one of them.
pub(crate) fn best_of_5_alternating(
    mut small: impl FnMut(),
    mut large: impl FnMut(),
) -> (Duration, Duration) {
    fn time(f: &mut impl FnMut()) -> Duration {
        let start = Instant::now();
        f();
        start.elapsed()
    }
    let (mut t_small, mut t_large) = (Duration::MAX, Duration::MAX);
    for _ in 0..5 {
        t_small = t_small.min(time(&mut small));
        t_large = t_large.min(time(&mut large));
    }
    (t_small, t_large)
}

/// Below this the small run's time is mostly noise, so only the budget
/// counts. Growing such inputs until they measure doesn't help: past the L2
/// cache, a loaded machine makes the larger one look superlinear.
const MEASURABLE: Duration = Duration::from_millis(2);
/// Readings a superlinear verdict needs: real quadratic cost repeats, a
/// burst of load does not.
const ATTEMPTS: usize = 3;

/// Times `f` on `make(n)` and `make(4 * n)` and fails when the larger input
/// takes more than ~10x as long (linear is ~4x, quadratic ~16x) on every
/// one of [`ATTEMPTS`] readings, or more than `budget`. Returns both
/// timings for reports.
pub(crate) fn assert_linear(
    label: &str,
    n: usize,
    make: impl Fn(usize) -> String,
    budget: Option<Duration>,
    f: impl Fn(&str),
) -> (Duration, Duration) {
    let (small, large) = (make(n), make(4 * n));
    let mut attempts = 0;
    loop {
        let (t_small, t_large) = best_of_5_alternating(|| f(&small), || f(&large));
        let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-6);
        eprintln!(
            "{label}: {} B in {t_small:?}, {} B in {t_large:?} (x{ratio:.1})",
            small.len(),
            large.len()
        );
        attempts += 1;
        let superlinear = t_small > MEASURABLE && ratio > 10.0;
        if superlinear && attempts < ATTEMPTS {
            continue;
        }
        assert!(
            !superlinear,
            "{label}: 4x the input took {ratio:.1}x as long ({t_small:?} -> {t_large:?}) \
             on {ATTEMPTS} readings: superlinear"
        );
        if let Some(budget) = budget {
            assert!(
                t_large <= budget,
                "{label}: {} bytes took {t_large:?}, over the {budget:?} budget",
                large.len()
            );
        }
        return (t_small, t_large);
    }
}
