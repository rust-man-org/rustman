use iced::{Background, Border, Color};

use crate::ui::theme::Palette;

// ── Text input ────────────────────────────────────────────────────────────────

/// For kv-table cells (params, headers, cookies): see `components::field::cell`.
pub fn cell_input(
    _theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    crate::ui::components::field::cell(Palette::tokens(), status)
}

/// For form fields (auth inputs, the find bar): see `components::field`.
pub fn field_input(
    _theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    crate::ui::components::field::field(Palette::tokens(), status)
}

// ── Pick list ───────────────────────────────────────────────────────────────────

/// Matches `field_input` so dropdowns and text fields look like one family —
/// including its states: brighter border on hover, accent while open.
pub fn pick_list(
    _theme: &iced::Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    use iced::widget::pick_list::Status;
    iced::widget::pick_list::Style {
        text_color: Palette::text(),
        placeholder_color: Palette::text_subtle(),
        handle_color: Palette::text_muted(),
        background: Background::Color(Palette::surface_high()),
        border: Border {
            color: match status {
                Status::Opened { .. } => Palette::border(),
                Status::Hovered => Palette::border(),
                Status::Active => Palette::border_subtle(),
            },
            width: 1.0,
            radius: crate::ui::theme::RADIUS_SM.into(),
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

// ── Tooltips ──────────────────────────────────────────────────────────────────

/// The bubble every hover tooltip sits in: raised surface, hairline border,
/// soft shadow. Without a container style a tooltip draws bare text straight
/// over whatever is underneath it.
pub fn tooltip_bubble(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface_raised())),
        text_color: Some(Palette::text()),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: crate::ui::theme::RADIUS_SM.into(),
        },
        shadow: iced::Shadow {
            color: Color {
                a: if Palette::is_dark() { 0.45 } else { 0.18 },
                ..Color::BLACK
            },
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

/// Wraps `content` in a hover tooltip showing `label` (below the control).
///
/// Every icon-only button should go through this: an unlabeled glyph is a
/// guessing game, and the label is also where the keyboard shortcut lives
/// ("New tab (Ctrl+T)").
pub fn tip<'a, Message: 'a>(
    content: impl Into<iced::Element<'a, Message>>,
    label: impl Into<String>,
) -> iced::Element<'a, Message> {
    tip_at(content, label, iced::widget::tooltip::Position::Bottom)
}

/// [`tip`] with an explicit placement (e.g. `Right` for the sidebar rail).
pub fn tip_at<'a, Message: 'a>(
    content: impl Into<iced::Element<'a, Message>>,
    label: impl Into<String>,
    position: iced::widget::tooltip::Position,
) -> iced::Element<'a, Message> {
    let label: String = label.into();
    iced::widget::tooltip(
        content,
        iced::widget::text(label)
            .size(crate::ui::theme::TEXT_SM)
            .color(Palette::text()),
        position,
    )
    .gap(6)
    .padding(6)
    .delay(crate::ui::theme::TOOLTIP_DELAY)
    .snap_within_viewport(true)
    .style(tooltip_bubble)
    .into()
}

// ── Buttons ───────────────────────────────────────────────────────────────────

/// Borderless icon/text button: transparent at rest, `hover` fill on hover,
/// a deeper fill while pressed, dimmed when disabled.
pub fn ghost_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    iced::widget::button::Style {
        background: match status {
            Status::Hovered => Some(Background::Color(Palette::hover())),
            Status::Pressed => Some(Background::Color(Palette::surface_high())),
            Status::Active | Status::Disabled => None,
        },
        text_color: if matches!(status, Status::Disabled) {
            Palette::text_subtle()
        } else {
            Palette::text_muted()
        },
        border: Border {
            radius: crate::ui::theme::RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

