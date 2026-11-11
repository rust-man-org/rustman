//! Shell-word splitting for pasted commands.
//!
//! Commands get pasted from three shells, and each one quotes differently:
//!
//! * **POSIX** (bash/zsh, every browser's "Copy as cURL (bash)"): `'…'`,
//!   `"…"`, `$'…'` ANSI-C strings, `\` escapes, `\`-newline continuations.
//! * **cmd.exe** (Chromium's "Copy as cURL (cmd)"): `^` escapes special
//!   characters and `^`-newline continues a line; the program then splits
//!   its arguments MSVC-style (`"…"`, `\"`).
//! * **PowerShell**: a backtick escapes and a backtick-newline continues a
//!   line; backslashes are plain path separators.
//!
//! A pasted line can hold more than one command (`echo … | curl …`,
//! `curl … > out.json`). [`commands`] splits at unquoted control operators so
//! the caller can pick the command it cares about. `#` comments are dropped.

use std::iter::Peekable;
use std::str::Chars;

/// The quoting rules a pasted command was written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dialect {
    Posix,
    Cmd,
    PowerShell,
}

impl Dialect {
    /// Guess the dialect from line continuations, the one reliable tell: no
    /// POSIX command ends a line with `^` or a backtick. Single-line cmd
    /// commands from Chromium quote every argument as `^"…^"`, so the first
    /// few arguments give those away.
    pub(super) fn detect(input: &str) -> Self {
        if let Some(dialect) = continuation_dialect(input) {
            return dialect;
        }
        let caret_quoted = input
            .split_whitespace()
            .skip(1)
            .take(3)
            .any(|word| word.starts_with("^\""));
        if caret_quoted { Self::Cmd } else { Self::Posix }
    }
}

/// Split `input` into commands, each a list of words with quoting removed.
/// Commands are separated by unquoted `|`, `;`, `&&`, `>`, … operators.
pub(super) fn commands(input: &str) -> Vec<Vec<String>> {
    let tokens = match Dialect::detect(input) {
        Dialect::Cmd => lex_cmd(input),
        dialect => PosixLexer::new(input, dialect == Dialect::PowerShell).run(),
    };
    let mut commands = vec![Vec::new()];
    for token in tokens {
        match token {
            Token::Word(word) => commands.last_mut().expect("never empty").push(word),
            Token::Operator => commands.push(Vec::new()),
        }
    }
    commands.retain(|words| !words.is_empty());
    commands
}

/// The words of the first command in `input`.
pub(crate) fn split(input: &str) -> Vec<String> {
    commands(input).into_iter().next().unwrap_or_default()
}

enum Token {
    Word(String),
    Operator,
}

/// Whether a line starting with `c` carries on the previous command. A
/// newline ends a command, except before an option: multi-line pastes that
/// lost their `\` continuations still import whole.
fn continues_command(c: char) -> bool {
    c == '-'
}

fn is_operator(c: char) -> bool {
    matches!(c, '|' | ';' | '&' | '<' | '>')
}

/// The dialect implied by the first line that ends in a continuation
/// character outside quotes, if any line does. The scan uses POSIX quoting,
/// which is close enough for every dialect to find line ends that are not
/// inside a quoted body.
///
/// A `^` or backtick only counts after a blank, and a line of nothing but
/// backticks is a Markdown fence, not a continuation.
fn continuation_dialect(input: &str) -> Option<Dialect> {
    let mut chars = input.chars();
    let mut quote: Option<char> = None;
    let mut last = ' ';
    let mut last_after_blank = false;
    let mut prev_blank = true;
    let mut only_backticks = true;
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == '\\' && q == '"' {
                chars.next();
            }
            continue;
        }
        match c {
            '\n' => {
                let continued = last_after_blank && !only_backticks;
                match last {
                    '^' if continued => return Some(Dialect::Cmd),
                    '`' if continued => return Some(Dialect::PowerShell),
                    _ => {}
                }
                (last, prev_blank, only_backticks) = (' ', true, true);
                continue;
            }
            '\\' => match chars.next() {
                Some('\n' | '\r') => return Some(Dialect::Posix),
                _ => last = 'x',
            },
            '\'' | '"' => quote = Some(c),
            c if c.is_whitespace() => {
                prev_blank = true;
                continue;
            }
            c => {
                last_after_blank = prev_blank;
                last = c;
            }
        }
        prev_blank = false;
        only_backticks &= c == '`';
    }
    None
}

// ---------------------------------------------------------------- POSIX ---

/// Word splitter for POSIX shells, with a PowerShell mode that swaps the
/// escape character for a backtick and keeps backslashes literal.
struct PosixLexer<'a> {
    chars: Peekable<Chars<'a>>,
    powershell: bool,
    /// An unescaped newline was skipped since the last token.
    line_break: bool,
}

impl<'a> PosixLexer<'a> {
    fn new(input: &'a str, powershell: bool) -> Self {
        Self {
            chars: input.chars().peekable(),
            powershell,
            line_break: false,
        }
    }

    fn escape_char(&self) -> char {
        if self.powershell { '`' } else { '\\' }
    }

    fn run(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(c) = self.skip_blanks() {
            if std::mem::take(&mut self.line_break) && !continues_command(c) {
                tokens.push(Token::Operator);
            }
            if c == '#' {
                self.skip_line();
            } else if is_operator(c) {
                while self.chars.next_if(|&c| is_operator(c)).is_some() {}
                tokens.push(Token::Operator);
            } else if let Some(word) = self.word() {
                tokens.push(Token::Word(word));
            }
        }
        tokens
    }

    /// Skip whitespace and line continuations between words; returns the
    /// next significant character without consuming it.
    fn skip_blanks(&mut self) -> Option<char> {
        loop {
            let c = *self.chars.peek()?;
            if c.is_whitespace() {
                self.line_break |= c == '\n';
                self.chars.next();
            } else if c == self.escape_char() {
                // `\` + newline continues the line. `\` + space at a word
                // start is the same continuation after something already
                // flattened the paste onto one line.
                let mut ahead = self.chars.clone();
                ahead.next();
                match ahead.peek() {
                    Some(n) if n.is_whitespace() => {
                        self.chars.next();
                        if self.chars.next() == Some('\r') {
                            self.chars.next_if_eq(&'\n');
                        }
                    }
                    _ => return Some(c),
                }
            } else {
                return Some(c);
            }
        }
    }

    fn skip_line(&mut self) {
        while self.chars.next_if(|&c| c != '\n').is_some() {}
    }

    /// Read one word. `None` when the input ended before any content.
    fn word(&mut self) -> Option<String> {
        let mut word = String::new();
        let mut has_content = false;
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() || (is_operator(c) && c != '&') {
                break;
            }
            self.chars.next();
            if c == self.escape_char() {
                has_content |= self.unquoted_escape(&mut word);
                continue;
            }
            has_content = true;
            match c {
                '\'' => self.single_quoted(&mut word),
                '"' => self.double_quoted(&mut word),
                '$' if !self.powershell && self.chars.next_if_eq(&'\'').is_some() => {
                    self.ansi_c_quoted(&mut word);
                }
                '$' if !self.powershell && self.chars.next_if_eq(&'"').is_some() => {
                    self.double_quoted(&mut word);
                }
                // PowerShell hands arguments to curl.exe, which unescapes
                // `\"` MSVC-style.
                '\\' if self.chars.next_if_eq(&'"').is_some() => word.push('"'),
                c => word.push(c),
            }
        }
        has_content.then_some(word)
    }

    /// An escape outside quotes: the next character literally, or nothing
    /// for a line continuation. Returns whether anything was added.
    fn unquoted_escape(&mut self, word: &mut String) -> bool {
        match self.chars.next() {
            Some('\r') => {
                self.chars.next_if_eq(&'\n');
                false
            }
            Some('\n') | None => false,
            Some(c) => {
                word.push(c);
                true
            }
        }
    }

    fn single_quoted(&mut self, word: &mut String) {
        while let Some(c) = self.chars.next() {
            match c {
                '\'' if self.powershell && self.chars.next_if_eq(&'\'').is_some() => {
                    word.push('\'');
                }
                '\'' => return,
                '\\' if self.powershell && self.chars.next_if_eq(&'"').is_some() => word.push('"'),
                c => word.push(c),
            }
        }
    }

    fn double_quoted(&mut self, word: &mut String) {
        while let Some(c) = self.chars.next() {
            match c {
                '"' if self.powershell && self.chars.next_if_eq(&'"').is_some() => word.push('"'),
                '"' => return,
                '`' if self.powershell => match self.chars.next() {
                    Some('n') => word.push('\n'),
                    Some('t') => word.push('\t'),
                    Some('r') => word.push('\r'),
                    Some('0') => word.push('\0'),
                    Some(c) => word.push(c),
                    None => {}
                },
                '\\' if self.powershell && self.chars.next_if_eq(&'"').is_some() => word.push('"'),
                // Bash keeps a backslash inside double quotes unless it
                // precedes `$`, a backtick, `"`, `\` or a newline — so
                // `"C:\tmp\x"` keeps its path separators.
                '\\' if !self.powershell => match self.chars.peek() {
                    Some('$' | '`' | '"' | '\\') => word.push(self.chars.next().expect("peeked")),
                    Some('\n') => {
                        self.chars.next();
                    }
                    _ => word.push('\\'),
                },
                c => word.push(c),
            }
        }
    }

    /// `$'…'`: decode ANSI-C escapes. Escapes like `\303\251` are bytes of
    /// one UTF-8 character, so the text is assembled as bytes.
    fn ansi_c_quoted(&mut self, word: &mut String) {
        let mut bytes = Vec::new();
        while let Some(c) = self.chars.next() {
            match c {
                '\'' => break,
                '\\' => self.ansi_c_escape(&mut bytes),
                c => push_char(&mut bytes, c),
            }
        }
        word.push_str(&String::from_utf8_lossy(&bytes));
    }

    fn ansi_c_escape(&mut self, bytes: &mut Vec<u8>) {
        let Some(c) = self.chars.next() else {
            bytes.push(b'\\');
            return;
        };
        let simple = match c {
            'n' => Some(b'\n'),
            't' => Some(b'\t'),
            'r' => Some(b'\r'),
            'a' => Some(0x07),
            'b' => Some(0x08),
            'f' => Some(0x0c),
            'v' => Some(0x0b),
            'e' | 'E' => Some(0x1b),
            '\\' | '\'' | '"' | '?' => Some(c as u8),
            _ => None,
        };
        if let Some(b) = simple {
            bytes.push(b);
            return;
        }
        match c {
            'x' => match self.digits(16, 2) {
                Some(v) => bytes.push(low_byte(v)),
                None => bytes.extend_from_slice(b"\\x"),
            },
            'u' | 'U' => {
                let max = if c == 'u' { 4 } else { 8 };
                match self.digits(16, max) {
                    Some(v) => push_char(bytes, char::from_u32(v).unwrap_or('\u{FFFD}')),
                    None => {
                        bytes.push(b'\\');
                        push_char(bytes, c);
                    }
                }
            }
            '0'..='7' => {
                let first = c.to_digit(8).expect("octal digit");
                let rest = self.digits_after(8, 2, first);
                bytes.push(low_byte(rest));
            }
            'c' => {
                if let Some(ctrl) = self.chars.next() {
                    bytes.push((ctrl as u8) & 0x1f);
                }
            }
            other => {
                bytes.push(b'\\');
                push_char(bytes, other);
            }
        }
    }

    /// Up to `max` digits in `radix`; `None` if there were none.
    fn digits(&mut self, radix: u32, max: usize) -> Option<u32> {
        let first = self.chars.next_if(|c| c.is_digit(radix))?.to_digit(radix)?;
        Some(self.digits_after(radix, max - 1, first))
    }

    fn digits_after(&mut self, radix: u32, max: usize, mut value: u32) -> u32 {
        for _ in 0..max {
            match self.chars.next_if(|c| c.is_digit(radix)) {
                Some(d) => value = value * radix + d.to_digit(radix).expect("checked"),
                None => break,
            }
        }
        value
    }
}

fn push_char(bytes: &mut Vec<u8>, c: char) {
    let mut buf = [0; 4];
    bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
}

/// The low byte of an escape's value: bash turns `\777` into `\xff`.
fn low_byte(value: u32) -> u8 {
    value.to_le_bytes()[0]
}

// ------------------------------------------------------------------ cmd ---

/// cmd.exe's layer: strips `^` escapes and `^`-newline continuations, and
/// reports which characters were escaped (an escaped `&` is data, not an
/// operator). Inside a plain `"…"` cmd leaves carets alone.
struct CmdChars<'a> {
    chars: Peekable<Chars<'a>>,
    in_quotes: bool,
}

impl Iterator for CmdChars<'_> {
    /// A character, and whether it was `^`-escaped.
    type Item = (char, bool);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let c = self.chars.next()?;
            match c {
                '\r' => continue,
                '"' => {
                    self.in_quotes = !self.in_quotes;
                    return Some(('"', false));
                }
                '^' if !self.in_quotes => match self.chars.next()? {
                    '\r' => {
                        self.chars.next_if_eq(&'\n');
                    }
                    '\n' => {}
                    escaped => return Some((escaped, true)),
                },
                c => return Some((c, false)),
            }
        }
    }
}

/// Split a cmd.exe command. After cmd strips the carets, curl.exe splits the
/// rest MSVC-style: `"` toggles quoting and `\"` is a literal quote.
/// Chromium also doubles every backslash it emits, so `\\` reads back as one.
fn lex_cmd(input: &str) -> Vec<Token> {
    let mut chars = CmdChars {
        chars: input.chars().peekable(),
        in_quotes: false,
    }
    .peekable();
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut has_content = false;
    let mut quoted = false;

    let flush = |tokens: &mut Vec<Token>, word: &mut String, has_content: &mut bool| {
        if std::mem::take(has_content) {
            tokens.push(Token::Word(std::mem::take(word)));
        }
    };

    let mut line_break = false;
    while let Some((c, escaped)) = chars.next() {
        if !quoted && c.is_whitespace() {
            flush(&mut tokens, &mut word, &mut has_content);
            line_break |= c == '\n' && !escaped;
            continue;
        }
        if std::mem::take(&mut line_break) && !continues_command(c) {
            tokens.push(Token::Operator);
        }
        match c {
            '"' => {
                quoted = !quoted;
                has_content = true;
            }
            '\\' if matches!(chars.peek(), Some(('"' | '\\', _))) => {
                word.push(chars.next().expect("peeked").0);
                has_content = true;
            }
            c if !quoted && !escaped && is_operator(c) => {
                flush(&mut tokens, &mut word, &mut has_content);
                while chars.next_if(|&(c, esc)| !esc && is_operator(c)).is_some() {}
                tokens.push(Token::Operator);
            }
            c => {
                word.push(c);
                has_content = true;
            }
        }
    }
    flush(&mut tokens, &mut word, &mut has_content);
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(input: &str) -> Vec<String> {
        split(input)
    }

    #[test]
    fn posix_quoting() {
        assert_eq!(
            words(r#"curl 'a b' "c \"d\" \$e" f\ g 'it'\''s'"#),
            ["curl", "a b", "c \"d\" $e", "f g", "it's"]
        );
    }

    #[test]
    fn posix_keeps_backslashes_in_double_quotes() {
        assert_eq!(words(r#"curl "C:\tmp\x""#), ["curl", r"C:\tmp\x"]);
    }

    #[test]
    fn line_continuations() {
        assert_eq!(
            words("curl \\\n  -H 'a: b' \\\r\n  x"),
            ["curl", "-H", "a: b", "x"]
        );
        // The same command after something flattened it onto one line.
        assert_eq!(
            words("curl \\  -H 'a: b' \\  x"),
            ["curl", "-H", "a: b", "x"]
        );
    }

    #[test]
    fn quoted_newlines_and_carriage_returns_are_data() {
        assert_eq!(words("curl -d 'a\r\nb'"), ["curl", "-d", "a\r\nb"]);
    }

    #[test]
    fn ansi_c_strings() {
        assert_eq!(
            words(r"curl $'a\nb\x41\101\u00e9\303\251\u0021\'q'"),
            ["curl", "a\nbAAéé!'q"]
        );
    }

    #[test]
    fn operators_end_the_command() {
        let cmds = commands("echo '{}' | curl -d @- https://x.io > out.json # done");
        assert_eq!(
            cmds,
            [
                vec!["echo", "{}"],
                vec!["curl", "-d", "@-", "https://x.io"],
                vec!["out.json"]
            ]
        );
        // An unquoted `&` inside a URL stays put; bash would background the
        // command, but nobody pasting a URL means that.
        assert_eq!(
            words("curl https://x.io/?a=1&b=2"),
            ["curl", "https://x.io/?a=1&b=2"]
        );
    }

    #[test]
    fn detects_dialects() {
        assert_eq!(Dialect::detect("curl https://x.io"), Dialect::Posix);
        assert_eq!(Dialect::detect("curl ^\"https://x.io^\""), Dialect::Cmd);
        assert_eq!(
            Dialect::detect("curl ^\"https://x^\" ^\r\n  -H ^\"a: b^\""),
            Dialect::Cmd
        );
        assert_eq!(
            Dialect::detect("curl.exe `\n  https://x.io"),
            Dialect::PowerShell
        );
        // A markdown fence inside a quoted body is not a continuation.
        assert_eq!(Dialect::detect("curl -d '```\n' x"), Dialect::Posix);
    }

    #[test]
    fn chromium_cmd_copy() {
        let cmd = "curl ^\"https://x.io/a?b=1^&c=2^\" ^\r\n  -H ^\"accept: */*^\" ^\r\n  --data-raw ^\"^{^\\^\"a^\\^\":^\\^\"x^\\^\\y^\\^\"^}^\"";
        assert_eq!(
            words(cmd),
            [
                "curl",
                "https://x.io/a?b=1&c=2",
                "-H",
                "accept: */*",
                "--data-raw",
                r#"{"a":"x\y"}"#
            ]
        );
    }

    #[test]
    fn chromium_cmd_multiline_body() {
        assert_eq!(
            words("curl --data-raw ^\"a^\n\nb^\""),
            ["curl", "--data-raw", "a\nb"]
        );
    }

    #[test]
    fn hand_written_cmd() {
        let cmd = "curl -X POST \"https://x.io\" ^\n -d \"{\\\"a\\\":1}\"";
        assert_eq!(
            words(cmd),
            ["curl", "-X", "POST", "https://x.io", "-d", r#"{"a":1}"#]
        );
    }

    #[test]
    fn powershell() {
        let cmd = "curl.exe -X POST `\n  -H \"Content-Type: application/json\" `\n  -d '{\\\"a\\\":1}' `\n  C:\\tmp\\x";
        assert_eq!(
            words(cmd),
            [
                "curl.exe",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-d",
                r#"{"a":1}"#,
                r"C:\tmp\x"
            ]
        );
    }
}
