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
// Named sizes so panels share one hierarchy instead of ad-hoc literals.
pub const TEXT_XS: f32 = 10.0; // timestamps, hashes, meta
pub const TEXT_SM: f32 = 12.0; // secondary labels, buttons, table cells
pub const TEXT_MD: f32 = 13.0; // primary body text
pub const TEXT_LG: f32 = 15.0; // section/card headers

static THEME_IDX: AtomicUsize = AtomicUsize::new(DEFAULT_THEME_IDX);

pub const RESPONSE_BODY_PADDING: iced::Padding = iced::Padding {
    top: 12.0,
    right: 12.0,
    bottom: 2.0,
    left: 12.0
};

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
}

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
        text_muted: Color::from_rgb(0.514, 0.580, 0.588),
        text_subtle: Color::from_rgb(0.345, 0.431, 0.459),
        accent: Color::from_rgb(0.149, 0.545, 0.824),
        hover: Color::from_rgb(0.055, 0.230, 0.275),
        chrome: Color::from_rgb(0.000, 0.130, 0.165),
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
        text_subtle: Color::from_rgb(0.554, 0.604, 0.680),
        accent: Color::from_rgb(0.557, 0.420, 0.965),
        hover: Color::from_rgb(0.939, 0.950, 0.969),
        chrome: Color::from_rgb(0.925, 0.940, 0.963),
        syntax_theme: Some("InspiredGitHub"),
    },
    // ── Rustman company themes (workstream 09) ──
    // Tokens from the design mockups (`agent/plan/mockup`, `:root` and
    // `[data-theme=dark]`). Appended, never inserted: sessions persist the
    // theme as an index, so earlier entries must keep their positions.
    ThemeSpec {
        name: "Rustman Light",
        dark: false,
        background: Color::from_rgb8(0xF9, 0xF9, 0xF9), // bg-1
        surface: Color::from_rgb8(0xFF, 0xFF, 0xFF),    // bg-0
        surface_high: Color::from_rgb8(0xF2, 0xF2, 0xF2), // bg-2
        surface_raised: Color::from_rgb8(0xED, 0xED, 0xED),
        row_odd: Color::from_rgb8(0xFC, 0xFC, 0xFC),
        border: Color::from_rgb8(0xE6, 0xE6, 0xE6),        // bd-2
        border_subtle: Color::from_rgb8(0xED, 0xED, 0xED), // bd-1
        text: Color::from_rgb8(0x21, 0x21, 0x21),          // tx-1
        text_muted: Color::from_rgb8(0x6B, 0x6B, 0x6B),    // tx-2
        text_subtle: Color::from_rgb8(0xA6, 0xA6, 0xA6),   // tx-3
        accent: Color::from_rgb8(0xFF, 0x6C, 0x37),
        hover: Color::from_rgb8(0xF2, 0xF2, 0xF2),
        chrome: Color::from_rgb8(0xED, 0xED, 0xED), // studio
        syntax_theme: Some("InspiredGitHub"),
    },
    ThemeSpec {
        name: "Rustman Dark",
        dark: true,
        background: Color::from_rgb8(0x1A, 0x1A, 0x1A), // bg-1
        surface: Color::from_rgb8(0x21, 0x21, 0x21),    // bg-0
        surface_high: Color::from_rgb8(0x2B, 0x2B, 0x2B), // bg-2
        surface_raised: Color::from_rgb8(0x30, 0x30, 0x30),
        row_odd: Color::from_rgb8(0x1E, 0x1E, 0x1E),
        border: Color::from_rgb8(0x3B, 0x3B, 0x3B),        // bd-2
        border_subtle: Color::from_rgb8(0x30, 0x30, 0x30), // bd-1
        text: Color::from_rgb8(0xF2, 0xF2, 0xF2),          // tx-1
        text_muted: Color::from_rgb8(0xA6, 0xA6, 0xA6),    // tx-2
        text_subtle: Color::from_rgb8(0x6B, 0x6B, 0x6B),   // tx-3
        accent: Color::from_rgb8(0xFF, 0x6C, 0x37),
        hover: Color::from_rgb8(0x2B, 0x2B, 0x2B),
        chrome: Color::from_rgb8(0x16, 0x16, 0x16), // studio
        syntax_theme: Some("base16-eighties.dark"),
    },
];

/// Theme used when no session has been saved yet (fresh install). Existing
/// users keep the index stored in their session.
pub const DEFAULT_THEME_IDX: usize = THEMES.len() - 2; // "Rustman Light"

/// Width used for interactive scrollbars, in pixels.
///
/// Wider than iced's 10 px default on purpose. iced computes the thumb length as
/// rail_length * (viewport / content) and floors it at 2 px, so a long list or a
/// big response body collapses the thumb into an unhittable sliver (a
/// 10,000-line body in a 400 px panel works out to ~2 px, against roughly 24 px
/// for a comfortable pointer target). The length is out of the host's hands, so
/// the width is made generous and the rail is kept clickable -- iced jumps to
/// wherever the rail is pressed, which is the practical way to move around a
/// document whose thumb has collapsed to a sliver.
pub const SCROLLBAR_WIDTH: f32 = 12.0;

/// A scrollbar sized for pointer and trackpad use.
///
/// Pair with `thin_scrollbar`; see `SCROLLBAR_WIDTH` for why the iced default
/// width is not enough on long content.
pub fn grabbable_scrollbar() -> iced::widget::scrollable::Scrollbar {
    iced::widget::scrollable::Scrollbar::new()
        .width(SCROLLBAR_WIDTH)
        .scroller_width(SCROLLBAR_WIDTH)
        .margin(1)
}

pub fn thin_scrollbar(
    _theme: &iced::Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let hovered = matches!(status, iced::widget::scrollable::Status::Hovered { .. });
    let dragging = matches!(status, iced::widget::scrollable::Status::Dragged { .. });
    // Hidden at rest (content scrolls fine with the wheel without a
    // persistent bar sitting there); appears clearly on hover/drag so it's
    // still easy to find and grab when you actually want to drag it.
    let alpha = if dragging {
        0.95
    } else if hovered {
        0.75
    } else {
        0.0
    };
    let s = Palette::text_subtle();
    let scroller = Color {
        r: s.r,
        g: s.g,
        b: s.b,
        a: alpha,
    };
    // A visible track on interaction: the whole rail is a click target (iced
    // jumps to the pressed position), which is what makes a collapsed thumb
    // navigable -- but only if the user can see where to click.
    let rail_bg = Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: if hovered || dragging { 0.14 } else { 0.0 },
    };
    let rail = iced::widget::scrollable::Rail {
        background: Some(iced::Background::Color(rail_bg)),
        border: iced::Border::default(),
        scroller: iced::widget::scrollable::Scroller {
            background: iced::Background::Color(scroller),
            border: iced::Border {
                radius: 3.0.into(),
                ..Default::default()
            },
        },
    };
    iced::widget::scrollable::Style {
        container: Default::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: iced::widget::scrollable::AutoScroll {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

/// Unlike `thin_scrollbar`, this stays faintly visible at rest — used for the
/// tab strip's horizontal scroll, which (unlike content panes) is the *only*
/// way to reach overflowed tabs, so it needs to be findable without hovering.
pub fn tab_scrollbar(
    _theme: &iced::Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let hovered = matches!(status, iced::widget::scrollable::Status::Hovered { .. });
    let dragging = matches!(status, iced::widget::scrollable::Status::Dragged { .. });
    let alpha = if dragging {
        0.85
    } else if hovered {
        0.55
    } else {
        0.22
    };
    let s = Palette::text_subtle();
    let scroller = Color {
        r: s.r,
        g: s.g,
        b: s.b,
        a: alpha,
    };
    let rail = iced::widget::scrollable::Rail {
        background: None,
        border: iced::Border::default(),
        scroller: iced::widget::scrollable::Scroller {
            background: iced::Background::Color(scroller),
            border: iced::Border {
                radius: 3.0.into(),
                ..Default::default()
            },
        },
    };
    iced::widget::scrollable::Style {
        container: Default::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: iced::widget::scrollable::AutoScroll {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

/// Fully invisible in every state — the panel still scrolls with the wheel,
/// there's just never a rail/thumb drawn over the content.
pub fn hidden_scrollbar(
    _theme: &iced::Theme,
    _status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let rail = iced::widget::scrollable::Rail {
        background: None,
        border: iced::Border::default(),
        scroller: iced::widget::scrollable::Scroller {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: iced::Border::default(),
        },
    };
    iced::widget::scrollable::Style {
        container: Default::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: iced::widget::scrollable::AutoScroll {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
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
    pub fn row_even() -> Color {
        Self::surface()
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
    /// Recessed chrome (icon rail, tab strip) — deliberately a step darker
    /// than `background` so it reads as a distinct bar, not the same void.
    pub fn chrome() -> Color {
        Self::current().chrome
    }

    pub fn accent_soft() -> Color {
        let a = Self::accent();
        Self::soft(a)
    }
    pub fn success_soft() -> Color {
        Self::soft(Self::SUCCESS)
    }
    pub fn warning_soft() -> Color {
        Self::soft(Self::WARNING)
    }
    pub fn error_soft() -> Color {
        Self::soft(Self::ERROR)
    }

    fn soft(color: Color) -> Color {
        if Self::is_dark() {
            Color {
                r: color.r * 0.22,
                g: color.g * 0.22,
                b: color.b * 0.24,
                a: 1.0,
            }
        } else {
            Color {
                r: 1.0 - ((1.0 - color.r) * 0.18),
                g: 1.0 - ((1.0 - color.g) * 0.18),
                b: 1.0 - ((1.0 - color.b) * 0.18),
                a: 1.0,
            }
        }
    }

    pub const SUCCESS: Color = Color::from_rgb(0.133, 0.773, 0.478);
    pub const WARNING: Color = Color::from_rgb(0.984, 0.749, 0.184);
    pub const ERROR: Color = Color::from_rgb(0.961, 0.267, 0.341);
    pub const GET: Color = Color::from_rgb(0.133, 0.773, 0.478);
    pub const POST: Color = Color::from_rgb(0.988, 0.580, 0.110);
    pub const PUT: Color = Color::from_rgb(0.388, 0.400, 0.945);
    pub const PATCH: Color = Color::from_rgb(0.678, 0.400, 0.945);
    pub const DELETE: Color = Color::from_rgb(0.961, 0.267, 0.341);
    pub const HEAD: Color = Color::from_rgb(0.631, 0.631, 0.667);

    /// Code-editor style from Palette — makes editor bg match all other panels.
    pub fn code_editor_style() -> iced_code_editor::Style {
        let accent = Self::accent();
        let t = Self::current();
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
            current_line_highlight: Color {
                r: accent.r,
                g: accent.g,
                b: accent.b,
                a: 0.10,
            },
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
            json_key_color: accent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an sRGB color.
    fn luminance(c: Color) -> f32 {
        let ch = |v: f32| {
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
    }

    fn contrast(a: Color, b: Color) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    fn theme(name: &str) -> &'static ThemeSpec {
        THEMES.iter().find(|t| t.name == name).expect(name)
    }

    #[test]
    fn saved_theme_indices_keep_pointing_at_the_same_theme() {
        // Sessions store the index. Reordering or inserting would silently
        // switch every existing user's theme.
        let legacy = [
            "Violet", "Amber", "Midnight", "Nord", "Dracula", "Kanagawa", "Solarized", "Light",
        ];
        for (i, name) in legacy.iter().enumerate() {
            assert_eq!(THEMES[i].name, *name, "theme index {i} moved");
        }
    }

    #[test]
    fn fresh_install_defaults_to_rustman_light() {
        assert_eq!(THEMES[DEFAULT_THEME_IDX].name, "Rustman Light");
        assert!(!THEMES[DEFAULT_THEME_IDX].dark);
    }

    #[test]
    fn theme_names_are_unique() {
        for (i, a) in THEMES.iter().enumerate() {
            for b in &THEMES[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn rustman_themes_have_readable_body_text() {
        for name in ["Rustman Light", "Rustman Dark"] {
            let t = theme(name);
            for (label, bg) in [
                ("surface", t.surface),
                ("background", t.background),
                ("surface_high", t.surface_high),
            ] {
                let c = contrast(t.text, bg);
                assert!(c >= 4.5, "{name}: text on {label} contrast {c:.2} < 4.5");
            }
            let muted = contrast(t.text_muted, t.surface);
            assert!(muted >= 4.5, "{name}: muted text contrast {muted:.2} < 4.5");
        }
    }

    #[test]
    fn rustman_themes_use_the_mockup_accent() {
        let orange = Color::from_rgb8(0xFF, 0x6C, 0x37);
        assert_eq!(theme("Rustman Light").accent, orange);
        assert_eq!(theme("Rustman Dark").accent, orange);
    }
}
