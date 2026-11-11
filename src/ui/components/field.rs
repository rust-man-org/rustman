//! Input styles. Fields have a hairline border that strengthens on hover
//! and focus (the caret marks focus, there is no ring); table cells have no
//! border at all.

use iced::widget::text_input::{Status, Style};
use iced::{Background, Border, Color};

use crate::ui::components::Surface;
use crate::ui::theme::{RADIUS_SM, Tokens};

/// A standalone field on the canvas (URL, search, sidebar inputs).
pub fn field(t: &Tokens, status: Status) -> Style {
    field_on(t, status, Surface::Canvas)
}

/// A standalone field on `surface` (dialog inputs sit on raised).
pub fn field_on(t: &Tokens, status: Status, surface: Surface) -> Style {
    Style {
        background: Background::Color(t.surface),
        border: Border {
            color: field_border(t, surface, !matches!(status, Status::Active | Status::Disabled)),
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        icon: t.text_muted,
        placeholder: t.text_subtle,
        value: t.text,
        selection: selection(t),
    }
}

/// A field's border: the hairline at rest, stronger while hovered or focused.
pub fn field_border(t: &Tokens, surface: Surface, engaged: bool) -> Color {
    if engaged { t.border_strong } else { surface.border(t) }
}

/// An editable table cell: no border, no fill, in every state.
pub fn cell(t: &Tokens, _status: Status) -> Style {
    Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: t.text_muted,
        placeholder: t.text_subtle,
        value: t.text,
        selection: selection(t),
    }
}

pub fn selection(t: &Tokens) -> Color {
    Color { a: 0.28, ..t.accent }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_strengthens_the_border_without_an_accent_ring() {
        let t = Tokens::for_theme(0);
        assert_eq!(field(t, Status::Active).border.color, t.border);
        assert_eq!(field(t, Status::Hovered).border.color, t.border_strong);
        let focused = field(t, Status::Focused { is_hovered: false });
        assert_eq!(focused.border.color, t.border_strong);
        assert!((focused.border.width - 1.0).abs() < f32::EPSILON);
        assert_eq!(focused.background, field(t, Status::Active).background, "no fill change");
    }

    #[test]
    fn raised_fields_use_the_raised_border_at_rest() {
        let t = Tokens::for_theme(3);
        assert_eq!(field_on(t, Status::Active, Surface::Raised).border.color, t.on_raised.border);
        assert_eq!(field_on(t, Status::Focused { is_hovered: false }, Surface::Raised).border.color, t.border_strong);
    }

    #[test]
    fn table_cells_are_borderless_in_every_state() {
        let t = Tokens::for_theme(7);
        for status in [Status::Active, Status::Hovered, Status::Focused { is_hovered: true }, Status::Disabled] {
            let s = cell(t, status);
            assert!((s.border.width).abs() < f32::EPSILON, "{status:?}");
            assert_eq!(s.background, Background::Color(Color::TRANSPARENT));
        }
    }
}
