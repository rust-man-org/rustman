use std::sync::atomic::{AtomicUsize, Ordering};

use iced::font::Weight;
use iced::{Color, Font};

pub const MONO: Font = Font::with_name("JetBrains Mono");

pub const UI_FONT: Font = Font::with_name("Noto Sans");
pub const UI_FONT_MEDIUM: Font = Font {
    weight: Weight::Medium,
    ..UI_FONT
};

// ── Type scale ──────────────────────────────────────────────────────────────
// Nothing renders below 11 px: smaller text is unreadable at 1x.
pub const TEXT_CAPTION: f32 = 11.0; // meta: timestamps, hashes, hints, section labels
pub const TEXT_SM: f32 = 12.0; // tabs, tables, buttons, secondary labels
pub const TEXT_MD: f32 = 13.0; // body text, inputs, the editor
pub const TEXT_LG: f32 = 15.0; // panel titles
pub const TEXT_XL: f32 = 18.0; // dialog and page titles

// Explicit line heights: iced's default is 1.3× the size, which leaves
// multi-line blocks short of the design.
pub const LINE_SM: f32 = 18.0; // for TEXT_CAPTION and TEXT_SM
pub const LINE_MD: f32 = 20.0; // for TEXT_MD (the editor's line pitch)
pub const LINE_LG: f32 = 24.0; // for TEXT_LG and TEXT_XL

/// An absolute line height for `text(..).line_height(..)`.
pub fn line(px: f32) -> iced::widget::text::LineHeight {
    iced::widget::text::LineHeight::Absolute(iced::Pixels(px))
}

// ── Icon sizes ──────────────────────────────────────────────────────────────
pub const ICON_SM: f32 = 12.0; // toolbar / ghost buttons
pub const ICON_MD: f32 = 14.0; // dialog chrome
pub const ICON_LG: f32 = 16.0; // sidebar rail

// ── Corner radii ────────────────────────────────────────────────────────────
// Three radii. Nothing is pill-shaped: rounded tracks around rounded tabs
// were the "pill-in-pill" look the redesign removes.
pub const RADIUS_XS: f32 = 4.0; // chips, checkboxes, dots
pub const RADIUS_SM: f32 = 6.0; // inputs and buttons
pub const RADIUS_LG: f32 = 10.0; // dialogs, the palette, popovers

// ── Spacing scale ───────────────────────────────────────────────────────────
// A 4 px grid: 4, 8, 12, 16, 24.
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_MD: f32 = 8.0;
pub const SPACE_LG: f32 = 12.0;
pub const SPACE_XL: f32 = 16.0;
pub const SPACE_2XL: f32 = 24.0;

// ── Control heights ─────────────────────────────────────────────────────────
/// The icon rail, including its hairline.
pub const RAIL_WIDTH: f32 = 48.0;
/// Tree rows.
pub const ROW_HEIGHT: f32 = 28.0;
/// Editable table rows (params, headers, cookies).
pub const TABLE_ROW_HEIGHT: f32 = 32.0;
/// Every per-pane tab bar (request tabs, response tabs).
pub const TAB_BAR_HEIGHT: f32 = 32.0;
/// The status bar, including its top hairline.
pub const STATUS_BAR_HEIGHT: f32 = 28.0;
/// The open-request tab strip.
pub const TAB_STRIP_HEIGHT: f32 = 36.0;
/// From this window width a fresh install lays the panes out side by side.
pub const SIDE_BY_SIDE_MIN_WIDTH: f32 = 1280.0;
/// Default request pane size: its width side by side, its height stacked.
pub const REQUEST_PANE: iced::Size = iced::Size::new(520.0, 320.0);
/// Neither pane can be dragged smaller than this.
pub const MIN_PANE: f32 = 160.0;
/// The panes' area in a 1440 × 900 window, for turning an old split ratio
/// into a size.
pub const REFERENCE_PANES: iced::Size = iced::Size::new(1136.0, 748.0);
/// The code editors' line-number gutter, and the space above the first
/// line and below the last.
pub const GUTTER_WIDTH: f32 = 52.0;
pub const GUTTER_NUMBER_SIZE: f32 = TEXT_SM;
pub const GUTTER_NUMBER_PADDING: f32 = SPACE_XL;
pub const EDITOR_PADDING: f32 = SPACE_MD;
/// Line height of the code editors and viewers (13 px text).
pub const EDITOR_LINE_HEIGHT: f32 = LINE_MD;
/// Thickness of the active-tab indicator and the loading bar.
pub const TAB_INDICATOR: f32 = 2.0;
/// Buttons and fields inside a bar.
pub const CONTROL_SM: f32 = 24.0;
/// Default buttons and menu-free toolbars.
pub const CONTROL_MD: f32 = 28.0;
/// Dialog buttons and menu rows.
pub const CONTROL_LG: f32 = 32.0;
/// The URL field and the Send button: the one big control on screen.
pub const CONTROL_XL: f32 = 36.0;
/// Height of the vertical separator between groups inside a bar.
pub const INLINE_SEPARATOR: f32 = 16.0;
/// Diameter of status and unsaved-change dots.
pub const DOT: f32 = 6.0;
/// Hairline width.
pub const HAIRLINE: f32 = 1.0;

/// Hover delay before a tooltip appears — long enough not to flash while
/// the pointer travels across a toolbar, short enough to feel responsive.
pub const TOOLTIP_DELAY: std::time::Duration = std::time::Duration::from_millis(450);

/// The platform's primary shortcut modifier, for hint text ("Ctrl+S").
pub const MOD_KEY: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
};

static THEME_IDX: AtomicUsize = AtomicUsize::new(0);

pub struct ThemeSpec {
    pub name: &'static str,
    pub dark: bool,
    pub background: Color,
    pub surface: Color,
    pub surface_high: Color,
    pub surface_raised: Color,
    pub row_odd: Color,
    pub border: Color,
    pub border_subtle: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_subtle: Color,
    pub accent: Color,
    pub hover: Color,
    pub chrome: Color,
    /// Preferred syntect syntax highlighting theme name.
    /// If `None`, auto-selects based on background brightness.
    pub syntax_theme: Option<&'static str>,
    /// HTTP method colors. Light themes need their own set: the dark-theme
    /// greens and oranges measure about 2.2:1 on white.
    pub methods: MethodColors,
    /// Response status colors, for the same reason as `methods`.
    pub status: StatusColors,
    /// The accent as text (3xx, links, JSON keys) when the theme picks one;
    /// `None` derives it from `accent` (see [`Tokens::accent_text`]).
    pub accent_text: Option<Color>,
}

/// One color per HTTP method, tuned for a theme's canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MethodColors {
    pub get: Color,
    pub post: Color,
    pub put: Color,
    pub patch: Color,
    pub delete: Color,
    /// HEAD, OPTIONS and anything unrecognised.
    pub other: Color,
}

impl MethodColors {
    /// Color for a method label. Case-insensitive; "WS" is not a method
    /// and is handled by the caller (it uses the accent).
    pub fn for_method(&self, method: &str) -> Color {
        match method.to_ascii_uppercase().as_str() {
            "GET" => self.get,
            "POST" => self.post,
            "PUT" => self.put,
            "PATCH" => self.patch,
            "DELETE" => self.delete,
            _ => self.other,
        }
    }

    #[cfg(test)]
    fn all(&self) -> [(&'static str, Color); 6] {
        [
            ("GET", self.get),
            ("POST", self.post),
            ("PUT", self.put),
            ("PATCH", self.patch),
            ("DELETE", self.delete),
            ("HEAD", self.other),
        ]
    }
}

/// Status colors. `redirect` is absent on purpose: 3xx uses the theme's
/// readable accent (see [`Tokens::accent_text`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusColors {
    pub success: Color,
    pub warning: Color,
    /// 4xx: "your request", an orange between warning and error.
    pub client_error: Color,
    /// 5xx and network failures.
    pub error: Color,
}

/// Shared by every dark theme. PUT, PATCH and DELETE are lighter than the
/// original constants so they clear 3:1 on Nord, Dracula and Solarized too.
const DARK_METHODS: MethodColors = MethodColors {
    get: Color::from_rgb(0.133, 0.773, 0.478),    // #22c57a
    post: Color::from_rgb(0.988, 0.580, 0.110),   // #fc941c
    put: Color::from_rgb(0.506, 0.549, 0.973),    // #818cf8
    patch: Color::from_rgb(0.710, 0.482, 0.961),  // #b57bf5
    delete: Color::from_rgb(0.973, 0.443, 0.443), // #f87171
    other: Color::from_rgb(0.631, 0.631, 0.667),  // #a1a1aa
};

const DARK_STATUS: StatusColors = StatusColors {
    success: Color::from_rgb(0.133, 0.773, 0.478),      // #22c57a
    warning: Color::from_rgb(0.984, 0.749, 0.184),      // #fbbf2f
    client_error: Color::from_rgb(0.976, 0.502, 0.204), // #f98034
    error: Color::from_rgb(0.973, 0.443, 0.443),        // #f87171
};

const LIGHT_METHODS: MethodColors = MethodColors {
    get: Color::from_rgb(0.082, 0.502, 0.239),    // #15803d
    post: Color::from_rgb(0.761, 0.255, 0.047),   // #c2410c
    put: Color::from_rgb(0.114, 0.306, 0.847),    // #1d4ed8
    patch: Color::from_rgb(0.494, 0.133, 0.808),  // #7e22ce
    delete: Color::from_rgb(0.725, 0.110, 0.110), // #b91c1c
    other: Color::from_rgb(0.278, 0.333, 0.412),  // #475569
};

const LIGHT_STATUS: StatusColors = StatusColors {
    success: Color::from_rgb(0.082, 0.502, 0.239),      // #15803d
    warning: Color::from_rgb(0.631, 0.384, 0.027),      // #a16207
    client_error: Color::from_rgb(0.761, 0.255, 0.047), // #c2410c
    error: Color::from_rgb(0.725, 0.110, 0.110),        // #b91c1c
};

pub const THEMES: &[ThemeSpec] = &[
    ThemeSpec {
        name: "Violet",
        dark: true,
        background: Color::from_rgb(0.047, 0.051, 0.078),
        surface: Color::from_rgb(0.082, 0.090, 0.133),
        surface_high: Color::from_rgb(0.114, 0.125, 0.180),
        surface_raised: Color::from_rgb(0.149, 0.161, 0.227),
        row_odd: Color::from_rgb(0.066, 0.071, 0.106),
        border: Color::from_rgb(0.243, 0.263, 0.353),
        border_subtle: Color::from_rgb(0.145, 0.157, 0.227),
        text: Color::from_rgb(0.953, 0.957, 0.984),
        text_muted: Color::from_rgb(0.608, 0.635, 0.722),
        text_subtle: Color::from_rgb(0.408, 0.435, 0.533),
        accent: Color::from_rgb(0.541, 0.478, 1.000),
        hover: Color::from_rgb(0.102, 0.110, 0.161),
        chrome: Color::from_rgb(0.031, 0.035, 0.055),
        methods: DARK_METHODS,
        status: DARK_STATUS,
        accent_text: None,
        syntax_theme: Some("base16-eighties.dark"),
    },
    ThemeSpec {
        name: "Amber",
        dark: true,
        background: Color::from_rgb(0.071, 0.067, 0.063),
        surface: Color::from_rgb(0.114, 0.106, 0.098),
        surface_high: Color::from_rgb(0.161, 0.149, 0.137),
        surface_raised: Color::from_rgb(0.208, 0.192, 0.176),
        row_odd: Color::from_rgb(0.090, 0.084, 0.078),
        border: Color::from_rgb(0.320, 0.293, 0.261),
        border_subtle: Color::from_rgb(0.190, 0.176, 0.161),
        text: Color::from_rgb(0.973, 0.965, 0.953),
        text_muted: Color::from_rgb(0.650, 0.620, 0.580),
        text_subtle: Color::from_rgb(0.460, 0.435, 0.405),
        accent: Color::from_rgb(0.988, 0.580, 0.110),
        hover: Color::from_rgb(0.145, 0.135, 0.125),
        chrome: Color::from_rgb(0.050, 0.047, 0.043),
        methods: DARK_METHODS,
        status: DARK_STATUS,
        accent_text: None,
        syntax_theme: Some("base16-mocha.dark"),
    },
    ThemeSpec {
        name: "Midnight",
        dark: true,
        background: Color::from_rgb(0.055, 0.059, 0.072),
        surface: Color::from_rgb(0.095, 0.101, 0.123),
        surface_high: Color::from_rgb(0.140, 0.149, 0.178),
        surface_raised: Color::from_rgb(0.185, 0.196, 0.230),
        row_odd: Color::from_rgb(0.076, 0.081, 0.099),
        border: Color::from_rgb(0.280, 0.300, 0.350),
        border_subtle: Color::from_rgb(0.175, 0.186, 0.220),
        text: Color::from_rgb(0.965, 0.973, 0.988),
        text_muted: Color::from_rgb(0.620, 0.645, 0.695),
        text_subtle: Color::from_rgb(0.440, 0.470, 0.530),
        accent: Color::from_rgb(0.557, 0.420, 0.965),
        hover: Color::from_rgb(0.125, 0.133, 0.160),
        chrome: Color::from_rgb(0.040, 0.043, 0.053),
        methods: DARK_METHODS,
        status: DARK_STATUS,
        accent_text: None,
        syntax_theme: Some("base16-ocean.dark"),
    },
    ThemeSpec {
        name: "Nord",
        dark: true,
        background: Color::from_rgb(0.145, 0.161, 0.192),
        surface: Color::from_rgb(0.180, 0.204, 0.243),
        surface_high: Color::from_rgb(0.216, 0.243, 0.283),
        surface_raised: Color::from_rgb(0.263, 0.298, 0.369),
        row_odd: Color::from_rgb(0.160, 0.180, 0.216),
        border: Color::from_rgb(0.320, 0.356, 0.420),
        border_subtle: Color::from_rgb(0.230, 0.255, 0.300),
        text: Color::from_rgb(0.925, 0.937, 0.957),
        text_muted: Color::from_rgb(0.700, 0.725, 0.765),
        text_subtle: Color::from_rgb(0.500, 0.525, 0.565),
        accent: Color::from_rgb(0.533, 0.753, 0.816),
        hover: Color::from_rgb(0.200, 0.224, 0.263),
        chrome: Color::from_rgb(0.122, 0.137, 0.163),
        methods: DARK_METHODS,
        status: DARK_STATUS,
        accent_text: None,
        syntax_theme: Some("base16-ocean.dark"),
    },
    ThemeSpec {
        name: "Dracula",
        dark: true,
        background: Color::from_rgb(0.157, 0.165, 0.212),
        surface: Color::from_rgb(0.184, 0.192, 0.239),
        surface_high: Color::from_rgb(0.208, 0.220, 0.267),
        surface_raised: Color::from_rgb(0.267, 0.278, 0.353),
        row_odd: Color::from_rgb(0.145, 0.153, 0.196),
        border: Color::from_rgb(0.330, 0.345, 0.420),
        border_subtle: Color::from_rgb(0.230, 0.242, 0.300),
        text: Color::from_rgb(0.973, 0.973, 0.949),
        text_muted: Color::from_rgb(0.700, 0.700, 0.760),
        text_subtle: Color::from_rgb(0.500, 0.500, 0.560),
        accent: Color::from_rgb(1.000, 0.475, 0.776),
        hover: Color::from_rgb(0.220, 0.230, 0.290),
        chrome: Color::from_rgb(0.125, 0.133, 0.173),
        methods: DARK_METHODS,
        status: DARK_STATUS,
        accent_text: None,
        syntax_theme: Some("base16-eighties.dark"),
    },
    ThemeSpec {
        name: "Kanagawa",
        dark: true,
        background: Color::from_rgb(0.122, 0.122, 0.157),
        surface: Color::from_rgb(0.102, 0.102, 0.133),
        surface_high: Color::from_rgb(0.165, 0.165, 0.216),
        surface_raised: Color::from_rgb(0.212, 0.212, 0.275),
        row_odd: Color::from_rgb(0.086, 0.086, 0.114),
        border: Color::from_rgb(0.329, 0.329, 0.427),
        border_subtle: Color::from_rgb(0.176, 0.196, 0.255),
        text: Color::from_rgb(0.863, 0.843, 0.729),
        text_muted: Color::from_rgb(0.706, 0.667, 0.529),
        text_subtle: Color::from_rgb(0.447, 0.443, 0.412),
        accent: Color::from_rgb(0.494, 0.612, 0.847),
        hover: Color::from_rgb(0.133, 0.196, 0.286),
        chrome: Color::from_rgb(0.086, 0.086, 0.114),
        methods: DARK_METHODS,
        status: DARK_STATUS,
        accent_text: None,
        syntax_theme: Some("base16-eighties.dark"),
    },
    ThemeSpec {
        name: "Solarized",
        dark: true,
        background: Color::from_rgb(0.000, 0.169, 0.212),
        surface: Color::from_rgb(0.027, 0.212, 0.259),
        surface_high: Color::from_rgb(0.043, 0.243, 0.293),
        surface_raised: Color::from_rgb(0.070, 0.280, 0.330),
        row_odd: Color::from_rgb(0.000, 0.145, 0.184),
        border: Color::from_rgb(0.220, 0.370, 0.400),
        border_subtle: Color::from_rgb(0.100, 0.260, 0.300),
        text: Color::from_rgb(0.933, 0.910, 0.835),
        text_muted: Color::from_rgb(0.600, 0.655, 0.655),
        text_subtle: Color::from_rgb(0.420, 0.500, 0.520),
        accent: Color::from_rgb(0.149, 0.545, 0.824),
        hover: Color::from_rgb(0.055, 0.230, 0.275),
        chrome: Color::from_rgb(0.000, 0.130, 0.165),
        methods: DARK_METHODS,
        status: DARK_STATUS,
        accent_text: None,
        syntax_theme: Some("Solarized (dark)"),
    },
    ThemeSpec {
        name: "Light",
        dark: false,
        background: Color::from_rgb(0.965, 0.972, 0.984),
        surface: Color::from_rgb(0.996, 0.997, 1.000),
        surface_high: Color::from_rgb(0.925, 0.941, 0.965),
        surface_raised: Color::from_rgb(0.890, 0.914, 0.949),
        row_odd: Color::from_rgb(0.977, 0.982, 0.991),
        border: Color::from_rgb(0.737, 0.776, 0.835),
        border_subtle: Color::from_rgb(0.847, 0.875, 0.918),
        text: Color::from_rgb(0.105, 0.124, 0.158),
        text_muted: Color::from_rgb(0.355, 0.400, 0.475),
        text_subtle: Color::from_rgb(0.460, 0.510, 0.590),
        accent: Color::from_rgb(0.557, 0.420, 0.965),
        hover: Color::from_rgb(0.939, 0.950, 0.969),
        chrome: Color::from_rgb(0.925, 0.940, 0.963),
        methods: LIGHT_METHODS,
        status: LIGHT_STATUS,
        accent_text: Some(Color::from_rgb(0.427, 0.157, 0.851)), // #6d28d9
        syntax_theme: Some("InspiredGitHub"),
    },
];

/// The resolved design tokens of one theme: every color a view needs, by role.
///
/// Three surfaces and nothing in between: `canvas` is the one flat color the
/// whole window sits on (rail, sidebar, tab strip, URL row, both panes,
/// status bar), `surface` fills controls that hold a value (fields, the URL
/// bar, cards), and `raised` is for things that float (menus, tooltips,
/// dialogs, the palette). Panes are separated by `border` hairlines, never
/// by a darker band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub canvas: Color,
    pub surface: Color,
    pub raised: Color,
    /// Fill under a hovered row or ghost button.
    pub hover: Color,
    /// The 1 px hairline between panes and around fields.
    pub border: Color,
    /// A hovered field, or a separator that has to read at a glance.
    pub border_strong: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_subtle: Color,
    /// Fills and indicators: Send, the active-tab underline, focus rings.
    pub accent: Color,
    /// The accent when it is used *as text* (3xx status, links, JSON keys):
    /// the raw accent is under 4.5:1 on Light and Solarized.
    pub accent_text: Color,
    /// Selected rows and toggled ghost buttons.
    pub accent_soft: Color,
    /// Text and icons drawn on an `accent` fill.
    pub on_accent: Color,
    pub methods: MethodColors,
    pub status: StatusColors,
    /// Fill of primary buttons: the accent deepened until a white label
    /// reaches 4.5:1 (light accents keep their dark label instead).
    pub accent_fill: Color,
    /// Fill of danger buttons, under a white label: the error color made
    /// deep enough for 5.5:1, so the translucent hover still clears 4.5:1.
    pub error_fill: Color,
    /// Tinted backdrop behind an error icon.
    pub error_soft: Color,
    /// Hover tint for rows inside a `surface` card.
    pub surface_hover: Color,
    /// Colors for content on `raised` (menus, tooltips, dialogs), lifted
    /// where the canvas values fall under the contrast bar.
    pub on_raised: RaisedTokens,
    /// Dims the window behind a dialog.
    pub scrim: Color,
    /// Shadow under floating surfaces. Nothing else casts one.
    pub shadow: Color,
    /// Code colors, shared by the code viewer and the theme previews.
    pub syntax: SyntaxColors,
}

/// Colors of JSON tokens in the code viewer and editors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyntaxColors {
    pub key: Color,
    pub string: Color,
    pub number: Color,
    /// `true`, `false`, `null`.
    pub literal: Color,
    pub punctuation: Color,
}

/// The variants of the canvas colors that are used on a `raised` surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaisedTokens {
    pub border: Color,
    pub text_muted: Color,
    pub hover: Color,
    pub selected: Color,
    pub methods: MethodColors,
    pub status: StatusColors,
}

impl Tokens {
    /// Tokens for `THEMES[idx]` (clamped). Built once per theme; cheap to copy.
    pub fn for_theme(idx: usize) -> &'static Tokens {
        static ALL: std::sync::LazyLock<Vec<Tokens>> =
            std::sync::LazyLock::new(|| THEMES.iter().map(Tokens::from_spec).collect());
        &ALL[idx.min(ALL.len() - 1)]
    }

    fn from_spec(spec: &ThemeSpec) -> Tokens {
        let canvas = spec.surface;
        // In a light theme elevation reads through the shadow, so floating
        // things are plain white rather than a darker grey than the canvas.
        let raised = if spec.dark {
            spec.surface_raised
        } else {
            Color::WHITE
        };
        let accent_text = spec
            .accent_text
            .unwrap_or_else(|| readable(spec.accent, canvas, 4.5));
        Tokens {
            dark: spec.dark,
            canvas,
            surface: spec.surface_high,
            raised,
            hover: spec.hover,
            border: spec.border_subtle,
            border_strong: spec.border,
            text: spec.text,
            text_muted: spec.text_muted,
            text_subtle: spec.text_subtle,
            accent: spec.accent,
            accent_text,
            accent_soft: soft(spec.accent, spec.dark),
            on_accent: if contrast(Color::WHITE, spec.accent) >= 3.0 {
                Color::WHITE
            } else {
                spec.chrome
            },
            methods: spec.methods,
            status: spec.status,
            accent_fill: accent_fill(spec),
            error_fill: error_fill(spec),
            error_soft: soft(spec.status.error, spec.dark),
            surface_hover: mix(spec.surface_high, spec.text, if spec.dark { 0.05 } else { 0.035 }),
            on_raised: raised_tokens(spec, raised),
            scrim: if spec.dark {
                Color::from_rgba(0.0, 0.0, 0.0, 0.5)
            } else {
                Color::from_rgba(0.059, 0.090, 0.165, 0.28)
            },
            shadow: if spec.dark {
                Color::from_rgba(0.0, 0.0, 0.0, 0.45)
            } else {
                Color::from_rgba(0.059, 0.090, 0.165, 0.16)
            },
            syntax: syntax_colors(spec, accent_text),
        }
    }


    /// Text color for a status code: 2xx success, 3xx accent, 4xx orange,
    /// 5xx and network failure (0) red.
    pub fn status_color(&self, code: u16) -> Color {
        match code {
            200..=299 => self.status.success,
            300..=399 => self.accent_text,
            400..=499 => self.status.client_error,
            0 | 500..=599 => self.status.error,
            _ => self.text_muted,
        }
    }

    /// Text color for a method label; "WS" (a socket, not a method) uses the
    /// accent, matching the WebSocket chip.
    pub fn method_color(&self, method: &str) -> Color {
        if method.eq_ignore_ascii_case("WS") {
            self.accent_text
        } else {
            self.methods.for_method(method)
        }
    }

    /// JSON object keys in the code viewer.
    pub fn syntax_key(&self) -> Color {
        self.syntax.key
    }
}

const DARK_SYNTAX: [Color; 3] = [
    Color::from_rgb(0.647, 0.839, 0.655), // #a5d6a7
    Color::from_rgb(0.961, 0.663, 0.498), // #f5a97f
    Color::from_rgb(0.490, 0.769, 0.894), // #7dc4e4
];

const LIGHT_SYNTAX: [Color; 3] = [
    Color::from_rgb(0.039, 0.420, 0.208), // #0a6b35
    Color::from_rgb(0.706, 0.255, 0.047), // #b4410c
    Color::from_rgb(0.114, 0.306, 0.847), // #1d4ed8
];

fn syntax_colors(spec: &ThemeSpec, accent_text: Color) -> SyntaxColors {
    let [string, number, literal] = if spec.dark { DARK_SYNTAX } else { LIGHT_SYNTAX };
    SyntaxColors {
        key: accent_text,
        string,
        number,
        literal,
        punctuation: spec.text_muted,
    }
}

/// White on the fill must reach this; the hover blend loses a little.
const ERROR_FILL_CONTRAST: f32 = 5.5;
/// Below this an error red reads as brown once it is darkened.
const ERROR_FILL_MIN_SATURATION: f32 = 0.65;

/// The error color darkened in HSL, hue kept and saturation held up, so it
/// stays red rather than turning brown.
fn error_fill(spec: &ThemeSpec) -> Color {
    let (hue, saturation, mut lightness) = hsl(spec.status.error);
    let saturation = saturation.max(ERROR_FILL_MIN_SATURATION);
    while lightness > 0.0 && contrast(Color::WHITE, from_hsl(hue, saturation, lightness)) < ERROR_FILL_CONTRAST {
        lightness -= 0.01;
    }
    from_hsl(hue, saturation, lightness.max(0.0))
}

/// Hue in degrees, saturation and lightness in 0..=1.
fn hsl(c: Color) -> (f32, f32, f32) {
    let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    if delta == 0.0 {
        return (0.0, 0.0, lightness);
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if max == c.r {
        60.0 * ((c.g - c.b) / delta).rem_euclid(6.0)
    } else if max == c.g {
        60.0 * ((c.b - c.r) / delta + 2.0)
    } else {
        60.0 * ((c.r - c.g) / delta + 4.0)
    };
    (hue, saturation, lightness)
}

fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> Color {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hue {
        h if h < 60.0 => (chroma, x, 0.0),
        h if h < 120.0 => (x, chroma, 0.0),
        h if h < 180.0 => (0.0, chroma, x),
        h if h < 240.0 => (0.0, x, chroma),
        h if h < 300.0 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    Color::from_rgb(r + m, g + m, b + m)
}

fn accent_fill(spec: &ThemeSpec) -> Color {
    if contrast(Color::WHITE, spec.accent) >= 3.0 {
        readable(spec.accent, Color::WHITE, 4.5)
    } else {
        spec.accent
    }
}

/// Raised surfaces are lighter than the canvas in dark themes, so muted
/// text and some method colors need lifting there, and the canvas hover
/// tint (darker than raised) can't be reused.
fn raised_tokens(spec: &ThemeSpec, raised: Color) -> RaisedTokens {
    let lift = |c: Color, min: f32| readable(c, raised, min);
    let m = spec.methods;
    let st = spec.status;
    RaisedTokens {
        border: if spec.dark { spec.border } else { spec.border_subtle },
        text_muted: lift(spec.text_muted, 4.5),
        hover: mix(raised, spec.text, if spec.dark { 0.06 } else { 0.04 }),
        selected: if spec.dark {
            mix(raised, spec.accent, 0.16)
        } else {
            soft(spec.accent, false)
        },
        methods: MethodColors {
            get: lift(m.get, 3.0),
            post: lift(m.post, 3.0),
            put: lift(m.put, 3.0),
            patch: lift(m.patch, 3.0),
            delete: lift(m.delete, 3.0),
            other: lift(m.other, 3.0),
        },
        status: StatusColors {
            success: lift(st.success, 4.5),
            warning: lift(st.warning, 4.5),
            client_error: lift(st.client_error, 4.5),
            error: lift(st.error, 4.5),
        },
    }
}

/// `a` moved toward `b` by `t` (0 = a, 1 = b).
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

/// A tinted background derived from `color`: dark themes scale it down
/// toward black, light themes wash it out toward white.
fn soft(color: Color, dark: bool) -> Color {
    if dark {
        Color::from_rgb(color.r * 0.22, color.g * 0.22, color.b * 0.24)
    } else {
        Color::from_rgb(
            1.0 - (1.0 - color.r) * 0.18,
            1.0 - (1.0 - color.g) * 0.18,
            1.0 - (1.0 - color.b) * 0.18,
        )
    }
}

/// `fg`, mixed toward white (on a dark `bg`) or black (on a light one) in
/// small steps until it reaches `min` contrast against `bg`. Colors that
/// already pass come back unchanged, so this only touches the few theme
/// accents that are too close to their own background to be read as text.
fn readable(fg: Color, bg: Color, min: f32) -> Color {
    let target = if luminance(bg) < 0.5 {
        Color::WHITE
    } else {
        Color::BLACK
    };
    let mut c = fg;
    for step in 1..=20 {
        if contrast(c, bg) >= min {
            break;
        }
        let t = step as f32 * 0.05;
        c = Color::from_rgb(
            fg.r + (target.r - fg.r) * t,
            fg.g + (target.g - fg.g) * t,
            fg.b + (target.b - fg.b) * t,
        );
    }
    c
}

pub struct Palette;

impl Palette {
    pub fn set_theme_idx(idx: usize) {
        THEME_IDX.store(idx.min(THEMES.len() - 1), Ordering::Relaxed);
    }
    pub fn theme_idx() -> usize {
        THEME_IDX.load(Ordering::Relaxed)
    }
    fn current() -> &'static ThemeSpec {
        &THEMES[Self::theme_idx()]
    }

    /// Tokens of the active theme, for style functions that are not handed
    /// any state. Views that have `AppState` should use `AppState::tokens`.
    pub fn tokens() -> &'static Tokens {
        Tokens::for_theme(Self::theme_idx())
    }

    /// The active theme as an iced theme, for everything drawn with iced's
    /// default styles (dropdown menus, scrollbars, plain text). Built once
    /// per theme; iced asks for it every frame.
    pub fn iced_theme() -> iced::Theme {
        static ICED_THEMES: std::sync::LazyLock<Vec<iced::Theme>> = std::sync::LazyLock::new(|| {
            THEMES
                .iter()
                .enumerate()
                .map(|(idx, spec)| {
                    let t = Tokens::for_theme(idx);
                    // The window clear color is the canvas, so any gap
                    // between widgets is the same flat color as the panes.
                    iced::Theme::custom(
                        spec.name,
                        iced::theme::Palette {
                            background: t.canvas,
                            text: t.text,
                            primary: t.accent,
                            success: t.status.success,
                            warning: t.status.warning,
                            danger: t.status.error,
                        },
                    )
                })
                .collect()
        });
        ICED_THEMES[Self::theme_idx()].clone()
    }

    pub fn is_dark() -> bool {
        Self::current().dark
    }
    pub fn background() -> Color {
        Self::current().background
    }
    pub fn surface() -> Color {
        Self::current().surface
    }
    pub fn surface_high() -> Color {
        Self::current().surface_high
    }
    pub fn surface_raised() -> Color {
        Self::current().surface_raised
    }
    pub fn row_odd() -> Color {
        Self::current().row_odd
    }
    pub fn border() -> Color {
        Self::current().border
    }
    pub fn border_subtle() -> Color {
        Self::current().border_subtle
    }
    pub fn text() -> Color {
        Self::current().text
    }
    pub fn text_muted() -> Color {
        Self::current().text_muted
    }
    pub fn text_subtle() -> Color {
        Self::current().text_subtle
    }
    pub fn accent() -> Color {
        Self::current().accent
    }
    /// Accent lightened for hover states on filled buttons.
    pub fn accent_hover() -> Color {
        let a = Self::accent();
        Color {
            r: (a.r + (1.0 - a.r) * 0.14).min(1.0),
            g: (a.g + (1.0 - a.g) * 0.14).min(1.0),
            b: a.b,
            a: 1.0,
        }
    }
    pub fn hover() -> Color {
        Self::current().hover
    }

    pub fn accent_soft() -> Color {
        let a = Self::accent();
        Self::soft(a)
    }
    /// Success green for the active theme (the `SUCCESS` constant is the
    /// dark-theme value and fails contrast on Light).
    pub fn success() -> Color {
        Self::tokens().status.success
    }
    pub fn warning() -> Color {
        Self::tokens().status.warning
    }
    pub fn error() -> Color {
        Self::tokens().status.error
    }

    fn soft(color: Color) -> Color {
        soft(color, Self::is_dark())
    }

    /// Code-editor style from Palette — makes editor bg match all other panels.
    pub fn code_editor_style() -> iced_code_editor::Style {
        let accent = Self::accent();
        let t = Self::current();
        let syntax = Self::tokens().syntax;
        iced_code_editor::Style {
            background: Self::surface(),
            text_color: Self::text(),
            gutter_background: Self::surface(),
            gutter_border: Self::border_subtle(),
            line_number_color: Self::text_subtle(),
            scrollbar_background: Color::TRANSPARENT,
            scroller_color: Color {
                r: Self::text_subtle().r,
                g: Self::text_subtle().g,
                b: Self::text_subtle().b,
                a: 0.5,
            },
            current_line_highlight: Color::TRANSPARENT,
            whitespace_color: Color {
                r: Self::text_subtle().r,
                g: Self::text_subtle().g,
                b: Self::text_subtle().b,
                a: 0.4,
            },
            selection_color: Color {
                r: accent.r,
                g: accent.g,
                b: accent.b,
                a: 0.28,
            },
            syntax_theme_name: t.syntax_theme,
            json_key_color: syntax.key,
            token_colors: Some(iced_code_editor::TokenColors {
                key: Some(syntax.key),
                string: Some(syntax.string),
                number: Some(syntax.number),
                literal: Some(syntax.literal),
                punctuation: Some(syntax.punctuation),
            }),
        }
    }
}

/// WCAG relative luminance of an sRGB color.
fn luminance(c: Color) -> f32 {
    fn lin(v: f32) -> f32 {
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two colors (1.0 – 21.0).
pub fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    /// A white label on a danger button, at rest and hovered over the
    /// raised surface, in every theme; the fill must still read as red.
    #[test]
    fn danger_fill_stays_red_and_readable_at_rest_and_on_hover() {
        use crate::ui::components::button::DANGER_HOVER_ALPHA;
        for spec in THEMES {
            let t = Tokens::from_spec(spec);
            let a = DANGER_HOVER_ALPHA;
            let hover = Color::from_rgb(
                t.error_fill.r * a + t.raised.r * (1.0 - a),
                t.error_fill.g * a + t.raised.g * (1.0 - a),
                t.error_fill.b * a + t.raised.b * (1.0 - a),
            );
            let name = spec.name;
            assert!(contrast(Color::WHITE, t.error_fill) >= 4.5, "{name}: rest");
            assert!(contrast(Color::WHITE, hover) >= 4.5, "{name}: hover {:.2}", contrast(Color::WHITE, hover));
            let (hue, saturation, _) = hsl(t.error_fill);
            assert!(!(15.0..345.0).contains(&hue) && saturation >= 0.6, "{name}: hue {hue:.0}, saturation {saturation:.2}");
        }
    }

    /// Text on a filled accent button must clear the 3:1 large-text floor in
    /// every theme — white failed it on four of the eight accents.
    #[test]
    fn on_accent_text_is_readable_in_every_theme() {
        for t in THEMES {
            let fg = if contrast(Color::WHITE, t.accent) >= 3.0 {
                Color::WHITE
            } else {
                t.chrome
            };
            let ratio = contrast(fg, t.accent);
            assert!(ratio >= 3.0, "{}: on-accent text {ratio:.2}:1", t.name);
        }
    }

    fn tokens() -> impl Iterator<Item = (&'static str, &'static Tokens)> {
        THEMES
            .iter()
            .enumerate()
            .map(|(i, spec)| (spec.name, Tokens::for_theme(i)))
    }

    /// Body text has to read on every surface it can land on: the canvas,
    /// a field or card, and a menu or dialog.
    #[test]
    fn body_text_clears_4_5_on_all_three_surfaces() {
        for (name, t) in tokens() {
            for (surface, bg) in [("canvas", t.canvas), ("surface", t.surface), ("raised", t.raised)] {
                let ratio = contrast(t.text, bg);
                assert!(ratio >= 4.5, "{name}: text on {surface} {ratio:.2}:1");
            }
            for (surface, bg) in [("canvas", t.canvas), ("surface", t.surface)] {
                let ratio = contrast(t.text_muted, bg);
                assert!(ratio >= 4.5, "{name}: text_muted on {surface} {ratio:.2}:1");
            }
            let subtle = contrast(t.text_subtle, t.canvas);
            assert!(subtle >= 3.0, "{name}: text_subtle on canvas {subtle:.2}:1");
        }
    }

    /// Method tags sit on the canvas (tree, tab strip) and inside the URL
    /// field (`surface`). The dark-theme constants measured ~2.2:1 on Light
    /// and ~2.8:1 for PUT on Nord.
    #[test]
    fn method_tags_clear_3_to_1_on_canvas_and_fields() {
        for (name, t) in tokens() {
            for (method, color) in t.methods.all() {
                for (surface, bg) in [("canvas", t.canvas), ("surface", t.surface)] {
                    let ratio = contrast(color, bg);
                    assert!(ratio >= 3.0, "{name}: {method} on {surface} {ratio:.2}:1");
                }
            }
        }
    }

    /// Status text ("200 OK", "Couldn't connect…") is read as text, so it
    /// is held to the body-text bar, not the 3:1 tag bar.
    #[test]
    fn status_colors_read_as_text_on_the_canvas() {
        for (name, t) in tokens() {
            for code in [200, 301, 404, 500, 0] {
                let ratio = contrast(t.status_color(code), t.canvas);
                assert!(ratio >= 4.5, "{name}: status {code} {ratio:.2}:1");
            }
            let warn = contrast(t.status.warning, t.canvas);
            assert!(warn >= 4.5, "{name}: warning {warn:.2}:1");
            let accent = contrast(t.accent_text, t.canvas);
            assert!(accent >= 4.5, "{name}: accent_text {accent:.2}:1");
        }
    }

    #[test]
    fn status_codes_map_to_their_class_color() {
        let t = Tokens::for_theme(0);
        assert_eq!(t.status_color(204), t.status.success);
        assert_eq!(t.status_color(302), t.accent_text);
        assert_eq!(t.status_color(422), t.status.client_error);
        assert_eq!(t.status_color(503), t.status.error);
        assert_eq!(t.status_color(0), t.status.error, "network failure reads as an error");
        assert_eq!(t.status_color(101), t.text_muted);
    }

    #[test]
    fn methods_map_case_insensitively_with_a_fallback() {
        let m = DARK_METHODS;
        assert_eq!(m.for_method("get"), m.get);
        assert_eq!(m.for_method("Delete"), m.delete);
        assert_eq!(m.for_method("OPTIONS"), m.other);
        assert_eq!(m.for_method("HEAD"), m.other);
        let t = Tokens::for_theme(0);
        assert_eq!(t.method_color("ws"), t.accent_text, "sockets use the accent");
    }

    /// `readable` leaves passing colors alone and lifts failing ones just
    /// far enough, in the direction away from the background.
    #[test]
    fn readable_only_adjusts_colors_that_fail() {
        let dark_bg = Color::from_rgb(0.1, 0.1, 0.1);
        let white_bg = Color::WHITE;
        let ok = Color::from_rgb(0.9, 0.9, 0.9);
        assert_eq!(readable(ok, dark_bg, 4.5), ok);

        let dim = Color::from_rgb(0.3, 0.3, 0.5);
        let lifted = readable(dim, dark_bg, 4.5);
        assert!(contrast(lifted, dark_bg) >= 4.5);
        assert!(lifted.r > dim.r, "moves toward white on a dark background");

        let pale = Color::from_rgb(0.7, 0.6, 1.0);
        let deepened = readable(pale, white_bg, 4.5);
        assert!(contrast(deepened, white_bg) >= 4.5);
        assert!(deepened.g < pale.g, "moves toward black on a light background");
    }

    /// Code is read as text, on the editor's canvas background.
    #[test]
    fn syntax_colors_read_on_the_editor_background() {
        for (name, t) in tokens() {
            let s = t.syntax;
            for (part, color) in [("key", s.key), ("string", s.string), ("number", s.number), ("literal", s.literal), ("punctuation", s.punctuation)] {
                let ratio = contrast(color, t.canvas);
                assert!(ratio >= 4.5, "{name}: syntax {part} {ratio:.2}:1");
            }
        }
    }

    /// Filled buttons carry 12–13 px labels, so the label needs 4.5:1.
    #[test]
    fn primary_button_labels_clear_4_5() {
        for (name, t) in tokens() {
            let ratio = contrast(t.on_accent, t.accent_fill);
            assert!(ratio >= 4.5, "{name}: on_accent on accent_fill {ratio:.2}:1");
        }
    }

    /// Menus and dialogs sit on `raised`; their muted text, method tags and
    /// status text are held to the same bars as on the canvas.
    #[test]
    fn raised_content_clears_the_same_bars() {
        for (name, t) in tokens() {
            let r = t.on_raised;
            let muted = contrast(r.text_muted, t.raised);
            assert!(muted >= 4.5, "{name}: muted on raised {muted:.2}:1");
            for (method, color) in r.methods.all() {
                let ratio = contrast(color, t.raised);
                assert!(ratio >= 3.0, "{name}: {method} on raised {ratio:.2}:1");
            }
            for color in [r.status.success, r.status.client_error, r.status.error] {
                let ratio = contrast(color, t.raised);
                assert!(ratio >= 4.5, "{name}: status on raised {ratio:.2}:1");
            }
            assert_ne!(r.hover, t.raised, "{name}: hover must be visible on raised");
        }
    }

    /// Matches the approved mockup: Violet's Send fill is #6e62cc, and the
    /// light accents keep their own color with a dark label.
    #[test]
    fn accent_fill_deepens_only_white_label_accents() {
        let hex = |c: Color| {
            let [r, g, b, _] = c.into_rgba8();
            format!("#{r:02x}{g:02x}{b:02x}")
        };
        assert_eq!(hex(Tokens::for_theme(0).accent_fill), "#6e62cc");
        for (i, spec) in THEMES.iter().enumerate() {
            let t = Tokens::for_theme(i);
            if t.on_accent != Color::WHITE {
                assert_eq!(t.accent_fill, spec.accent, "{}", spec.name);
            }
        }
    }

    /// Row tints on raised are mixed from raised itself, so text on a
    /// hovered or selected menu row still reads.
    #[test]
    fn text_reads_on_raised_row_tints() {
        for (name, t) in tokens() {
            for (tint, bg) in [("hover", t.on_raised.hover), ("selected", t.on_raised.selected)] {
                let ratio = contrast(t.text, bg);
                assert!(ratio >= 4.5, "{name}: text on raised {tint} {ratio:.2}:1");
                let muted = contrast(t.on_raised.text_muted, bg);
                assert!(muted >= 3.0, "{name}: muted on raised {tint} {muted:.2}:1");
            }
        }
    }
}
