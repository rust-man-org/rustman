use iced::{
    Border, Color, Element,
    widget::{Space, Text, container, text},
};

/// The embedded icon font. The family name `lucide` is the one baked into the TTF.
pub const ICON_FONT: iced::Font = iced::Font::with_name("lucide");

/// Default on-screen size for an icon glyph.
const DEFAULT_SIZE: f32 = 16.0;

fn glyph(c: char) -> Text<'static> {
    text(c).font(ICON_FONT).size(DEFAULT_SIZE)
}

// ── Sidebar rail ────────────────────────────────────────────────────────────────
pub fn collections() -> Text<'static> {
    glyph('\u{e0d7}')
} // folder
pub fn history() -> Text<'static> {
    glyph('\u{e1f5}')
} // history
pub fn environments() -> Text<'static> {
    glyph('\u{e529}')
} // layers
pub fn settings() -> Text<'static> {
    glyph('\u{e154}')
} // settings

// ── Actions ─────────────────────────────────────────────────────────────────────
pub fn close() -> Text<'static> {
    glyph('\u{e1b2}')
} // x
pub fn edit() -> Text<'static> {
    glyph('\u{e1f9}')
} // pencil
pub fn copy() -> Text<'static> {
    glyph('\u{e09e}')
} // copy
pub fn import() -> Text<'static> {
    glyph('\u{e0b2}')
} // download
pub fn export() -> Text<'static> {
    glyph('\u{e19e}')
} // upload
pub fn more() -> Text<'static> {
    glyph('\u{e0b6}')
} // ellipsis
pub fn import_box() -> Text<'static> {
    glyph('\u{e427}')
} // arrow-down-square
pub fn folder() -> Text<'static> {
    glyph('\u{e0d7}')
} // folder
pub fn trash() -> Text<'static> {
    glyph('\u{e18e}')
} // trash-2
pub fn alert() -> Text<'static> {
    glyph('\u{e193}')
} // alert-triangle
pub fn folder_plus() -> Text<'static> {
    glyph('\u{e0d9}')
} // folder-plus
pub fn search() -> Text<'static> {
    glyph('\u{e151}')
} // search
pub fn plus() -> Text<'static> {
    glyph('\u{e13d}')
} // plus
pub fn send() -> Text<'static> {
    glyph('\u{e152}')
} // send
pub fn eye() -> Text<'static> {
    glyph('\u{e0ba}')
}
pub fn eye_closed() -> Text<'static> {
    glyph('\u{e0bb}')
}

// ── Meta / status ───────────────────────────────────────────────────────────────
pub fn zap() -> Text<'static> {
    glyph('\u{e1b4}')
} // zap
pub fn timer() -> Text<'static> {
    glyph('\u{e1e0}')
} // timer
pub fn globe() -> Text<'static> {
    glyph('\u{e0e8}')
} // globe
pub fn braces() -> Text<'static> {
    glyph('\u{e36a}')
} // braces

/// Saved request/response example (Lucide file-code).
pub fn example() -> Text<'static> {
    glyph('\u{e0c3}')
}

pub fn check_circle() -> Text<'static> {
    glyph('\u{e07c}')
}

pub fn panel_left_close() -> Text<'static> {
    glyph('\u{e21c}')
}
pub fn panel_left_open() -> Text<'static> {
    glyph('\u{e21d}')
}

// ── Tree / disclosure ─────────────────────────────────────────────────────────────
pub fn chevron_down() -> Text<'static> {
    glyph('\u{e06d}')
}
pub fn chevron_right() -> Text<'static> {
    glyph('\u{e06f}')
}

// ── Direction / VCS / keys ────────────────────────────────────────────────────────
pub fn arrow_right() -> Text<'static> {
    glyph('\u{e049}')
}
pub fn arrow_left() -> Text<'static> {
    glyph('\u{e048}')
}
pub fn git_branch() -> Text<'static> {
    glyph('\u{e0e2}')
} // git-branch

pub fn dot<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(Space::new())
        .width(8)
        .height(8)
        .style(move |_| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
pub fn panel_right_close() -> Text<'static> {
    glyph('\u{e432}')
}
pub fn panel_right_open() -> Text<'static> {
    glyph('\u{e434}')
}
