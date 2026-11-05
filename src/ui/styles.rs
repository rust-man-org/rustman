use iced::{Background, Border, Color};

use crate::ui::theme::Palette;

/// Content surface shared by workspace organisms.
pub fn content_surface(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Palette::surface().into()),
        text_color: Some(Palette::text()),
        ..Default::default()
    }
}

// ── Text input ────────────────────────────────────────────────────────────────

/// Mockup `.kv-row input`: transparent, bg-2 on hover, bd-2 frame on a
/// bg-0 fill while focused.
pub fn kv_input(
    _theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    use iced::widget::text_input::Status;
    let accent = Palette::accent();
    let (background, border) = match status {
        Status::Focused { .. } => (Palette::surface(), Palette::border()),
        Status::Hovered => (Palette::surface_high(), Color::TRANSPARENT),
        _ => (Color::TRANSPARENT, Color::TRANSPARENT),
    };
    iced::widget::text_input::Style {
        background: Background::Color(background),
        border: Border {
            color: border,
            width: 1.0,
            radius: 4.0.into(),
        },
        icon: Palette::text_muted(),
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Color { a: 0.25, ..accent },
    }
}

/// Mockup `.auth-input`: bg-1 field in a bd-1 frame, accent when focused.
pub fn boxed_input(
    _theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    use iced::widget::text_input::Status;
    let accent = Palette::accent();
    iced::widget::text_input::Style {
        background: Background::Color(Palette::background()),
        border: Border {
            color: match status {
                Status::Focused { .. } => accent,
                Status::Hovered => Palette::border(),
                _ => Palette::border_subtle(),
            },
            width: 1.0,
            radius: 4.0.into(),
        },
        icon: Palette::text_muted(),
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Color { a: 0.25, ..accent },
    }
}

/// Mockup `.auth-select` / `.bodypick`: bg-1 select in a bd-1 frame.
pub fn boxed_pick_list(
    _theme: &iced::Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    use iced::widget::pick_list::Status;
    iced::widget::pick_list::Style {
        text_color: Palette::text(),
        placeholder_color: Palette::text_subtle(),
        handle_color: Palette::text_subtle(),
        background: Background::Color(Palette::background()),
        border: Border {
            color: match status {
                Status::Opened { .. } => Palette::accent(),
                Status::Hovered => Palette::border(),
                _ => Palette::border_subtle(),
            },
            width: 1.0,
            radius: 4.0.into(),
        },
    }
}

/// For kv-table cells (params, headers, cookies).
pub fn cell_input(
    _theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let accent = Palette::accent();
    iced::widget::text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: match status {
                iced::widget::text_input::Status::Focused { .. } => Color {
                    r: accent.r,
                    g: accent.g,
                    b: accent.b,
                    a: 0.5,
                },
                iced::widget::text_input::Status::Hovered => Palette::border_subtle(),
                _ => Color::TRANSPARENT,
            },
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: Palette::text_muted(),
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Color {
            r: accent.r,
            g: accent.g,
            b: accent.b,
            a: 0.25,
        },
    }
}

/// Borderless spreadsheet field; only keyboard/edit focus draws an outline.
pub fn table_cell_input(
    theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let mut style = cell_input(theme, status);
    style.border.radius = 0.0.into();
    if !matches!(status, iced::widget::text_input::Status::Focused { .. }) {
        style.border.color = Color::TRANSPARENT;
    }
    style
}

/// For form fields (auth inputs, sidebar inputs, URL bar).
pub fn field_input(
    _theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let accent = Palette::accent();
    iced::widget::text_input::Style {
        background: Background::Color(Palette::surface_high()),
        border: Border {
            color: match status {
                iced::widget::text_input::Status::Focused { .. } => accent,
                iced::widget::text_input::Status::Hovered => Palette::border(),
                _ => Palette::border_subtle(),
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        icon: Palette::text_muted(),
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Color {
            r: accent.r,
            g: accent.g,
            b: accent.b,
            a: 0.25,
        },
    }
}

// ── Checkbox ────────────────────────────────────────────────────────────────────

/// Accent checkbox matching the app palette (params/headers rows).
pub fn checkbox(
    _theme: &iced::Theme,
    status: iced::widget::checkbox::Status,
) -> iced::widget::checkbox::Style {
    use iced::widget::checkbox::Status;
    let (background, border_color) = match status {
        Status::Active { is_checked: true } | Status::Hovered { is_checked: true } => {
            (Background::Color(Palette::accent()), Palette::accent())
        }
        Status::Active { .. } => (
            Background::Color(Palette::surface_high()),
            Palette::border(),
        ),
        Status::Hovered { .. } => (
            Background::Color(Palette::surface_raised()),
            Palette::border(),
        ),
        Status::Disabled { is_checked: true } => (
            Background::Color(Palette::surface_raised()),
            Palette::border_subtle(),
        ),
        Status::Disabled { .. } => (
            Background::Color(Palette::surface()),
            Palette::border_subtle(),
        ),
    };
    iced::widget::checkbox::Style {
        background,
        icon_color: Color::WHITE,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 4.0.into(),
        },
        text_color: None,
    }
}

// ── Pick list ───────────────────────────────────────────────────────────────────

/// Matches `field_input` so dropdowns and text fields look like one family.
pub fn pick_list(
    _theme: &iced::Theme,
    _status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    iced::widget::pick_list::Style {
        text_color: Palette::text(),
        placeholder_color: Palette::text_subtle(),
        handle_color: Palette::text_muted(),
        background: Background::Color(Palette::surface_high()),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 8.0.into(),
        },
    }
}

// ── text_editor (scripts panel uses iced text_editor, not CodeEditor) ─────────

pub fn scripts_editor(
    _theme: &iced::Theme,
    _status: iced::widget::text_editor::Status,
) -> iced::widget::text_editor::Style {
    let accent = Palette::accent();
    iced::widget::text_editor::Style {
        background: Background::Color(Palette::surface()),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Color {
            r: accent.r,
            g: accent.g,
            b: accent.b,
            a: 0.25,
        },
    }
}

pub fn divider(_theme: &iced::Theme) -> iced::widget::rule::Style {
    iced::widget::rule::Style {
        color: Palette::border_subtle(),
        radius: 0.0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: true,
    }
}

// ── Container helpers ─────────────────────────────────────────────────────────

/// A slightly-raised container for section headers inside panels.
pub fn section_header(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface_high())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}
