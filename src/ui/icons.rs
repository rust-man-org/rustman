use iced::{
    Border, Color, Element,
    widget::{Space, Text, container, text},
};

/// The embedded icon font. The family name `lucide` is the one baked into the TTF.
pub const ICON_FONT: iced::Font = iced::Font::with_name("lucide");

/// Default on-screen size for an icon glyph.
const DEFAULT_SIZE: f32 = crate::ui::theme::ICON_LG;

fn glyph<'a>(c: char) -> Text<'a> {
    text(c).font(ICON_FONT).size(DEFAULT_SIZE)
}

// ── Sidebar rail ────────────────────────────────────────────────────────────────
pub fn collections<'a>() -> Text<'a> {
    glyph('\u{e0d7}')
} // folder
pub fn history<'a>() -> Text<'a> {
    glyph('\u{e1f5}')
} // history
pub fn environments<'a>() -> Text<'a> {
    glyph('\u{e529}')
} // layers
pub fn settings<'a>() -> Text<'a> {
    glyph('\u{e154}')
} // settings

// ── Actions ─────────────────────────────────────────────────────────────────────
pub fn close<'a>() -> Text<'a> {
    glyph('\u{e1b2}')
} // x
pub fn edit<'a>() -> Text<'a> {
    glyph('\u{e1f9}')
} // pencil
pub fn copy<'a>() -> Text<'a> {
    glyph('\u{e09e}')
} // copy
pub fn import<'a>() -> Text<'a> {
    glyph('\u{e0b2}')
} // download
pub fn export<'a>() -> Text<'a> {
    glyph('\u{e19e}')
} // upload
pub fn search<'a>() -> Text<'a> {
    glyph('\u{e151}')
} // search
pub fn plus<'a>() -> Text<'a> {
    glyph('\u{e13d}')
} // plus
pub fn folder<'a>() -> Text<'a> {
    glyph('\u{e0d7}')
} // folder
pub fn folder_plus<'a>() -> Text<'a> {
    glyph('\u{e0d9}')
} // folder-plus
pub fn send<'a>() -> Text<'a> {
    glyph('\u{e152}')
} // send
/// cURL export. Was `settings-2` (a sliders glyph), which read as a settings
/// toggle sitting next to Send; a terminal prompt says "command line".
pub fn curl<'a>() -> Text<'a> {
    glyph('\u{e20a}')
} // square-terminal
pub fn clipboard_paste<'a>() -> Text<'a> {
    glyph('\u{e3e8}')
} // clipboard-paste
pub fn eye<'a>() -> Text<'a> {
    glyph('\u{e0ba}')
}
pub fn eye_closed<'a>() -> Text<'a> {
    glyph('\u{e0bb}')
}

// ── Meta / status ───────────────────────────────────────────────────────────────
pub fn timer<'a>() -> Text<'a> {
    glyph('\u{e1e0}')
} // timer
pub fn globe<'a>() -> Text<'a> {
    glyph('\u{e0e8}')
} // globe
pub fn braces<'a>() -> Text<'a> {
    glyph('\u{e36a}')
} // braces
pub fn check<'a>() -> Text<'a> {
    glyph('\u{e06c}')
} // check
pub fn alert<'a>() -> Text<'a> {
    glyph('\u{e077}')
} // alert-circle
pub fn lock<'a>() -> Text<'a> {
    glyph('\u{e10b}')
} // lock
pub fn refresh<'a>() -> Text<'a> {
    glyph('\u{e145}')
} // refresh-cw

pub fn unplug<'a>() -> Text<'a> {
    glyph('\u{e45d}')
}
pub fn wifi_off<'a>() -> Text<'a> {
    glyph('\u{e1af}')
}
pub fn shield_alert<'a>() -> Text<'a> {
    glyph('\u{e1fe}')
}
pub fn hourglass<'a>() -> Text<'a> {
    glyph('\u{e296}')
}
pub fn alert_triangle<'a>() -> Text<'a> {
    glyph('\u{e193}')
}
pub fn ellipsis<'a>() -> Text<'a> {
    glyph('\u{e0b6}')
}
pub fn minimize<'a>() -> Text<'a> {
    glyph('\u{e11c}')
}
pub fn maximize<'a>() -> Text<'a> {
    glyph('\u{e167}')
}
pub fn restore<'a>() -> Text<'a> {
    glyph('\u{e09e}')
}
pub fn panel_left<'a>() -> Text<'a> {
    glyph('\u{e12a}')
}
pub fn columns<'a>() -> Text<'a> {
    glyph('\u{e098}')
}
pub fn rows<'a>() -> Text<'a> {
    glyph('\u{e439}')
}
#[cfg(feature = "self-update")]
pub fn update<'a>() -> Text<'a> {
    glyph('\u{e07b}')
}
pub fn chevron_up<'a>() -> Text<'a> {
    glyph('\u{e070}')
}

// ── Tree / disclosure ─────────────────────────────────────────────────────────────
pub fn chevron_down<'a>() -> Text<'a> {
    glyph('\u{e06d}')
}
pub fn chevron_right<'a>() -> Text<'a> {
    glyph('\u{e06f}')
}

// ── Direction / VCS / keys ────────────────────────────────────────────────────────
pub fn arrow_right<'a>() -> Text<'a> {
    glyph('\u{e049}')
}
pub fn arrow_left<'a>() -> Text<'a> {
    glyph('\u{e048}')
}
pub fn git_branch<'a>() -> Text<'a> {
    glyph('\u{e0e2}')
} // git-branch
pub fn arrow_up<'a>() -> Text<'a> {
    glyph('\u{e04a}')
} // arrow-up
pub fn arrow_down<'a>() -> Text<'a> {
    glyph('\u{e042}')
} // arrow-down
/// The corner-down-left glyph, for the Enter keycap.
pub const ENTER_KEY: char = '\u{e0a1}';

pub fn more<'a>() -> Text<'a> {
    glyph('\u{e0b6}')
} // ellipsis
pub fn file_plus<'a>() -> Text<'a> {
    glyph('\u{e0c9}')
}
pub fn trash<'a>() -> Text<'a> {
    glyph('\u{e18e}')
} // trash-2
pub fn file_input<'a>() -> Text<'a> {
    glyph('\u{e0c5}')
}
pub fn folder_git<'a>() -> Text<'a> {
    glyph('\u{e409}')
}
pub fn rotate_ccw<'a>() -> Text<'a> {
    glyph('\u{e148}')
}
pub fn pull<'a>() -> Text<'a> {
    glyph('\u{e455}')
} // arrow-down-to-line
pub fn push<'a>() -> Text<'a> {
    glyph('\u{e45a}')
} // arrow-up-from-line

pub fn palette<'a>() -> Text<'a> {
    glyph('\u{e1dd}')
}
pub fn network<'a>() -> Text<'a> {
    glyph('\u{e125}')
}
pub fn keyboard<'a>() -> Text<'a> {
    glyph('\u{e284}')
}
pub fn info<'a>() -> Text<'a> {
    glyph('\u{e0f9}')
}

pub fn play<'a>() -> Text<'a> {
    glyph('\u{e13c}')
}

pub fn dot<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(Space::new())
        .width(8)
        .height(8)
        .style(move |_| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: crate::ui::theme::RADIUS_XS.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
