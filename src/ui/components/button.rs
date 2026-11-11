//! Text buttons: one shape (radius 6), three variants, four heights.

use iced::widget::{Button, Text, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::ui::components::Surface;
use crate::ui::theme::{
    CONTROL_LG, CONTROL_MD, CONTROL_SM, CONTROL_XL, RADIUS_SM, SPACE_LG, SPACE_MD, SPACE_XL,
    TEXT_MD, TEXT_SM, Tokens, UI_FONT, UI_FONT_MEDIUM,
};

/// Opacity of a hovered danger fill over its surface.
pub const DANGER_HOVER_ALPHA: f32 = 0.88;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// The one main action of an area (Send, Save).
    Primary,
    /// Bordered, for actions next to a primary or on their own (Cancel, Retry).
    Secondary,
    /// Borderless until hovered, for actions inside bars.
    Ghost,
    /// The confirming action of a destructive dialog (Delete, Discard).
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// 24 px, inside tab bars and toolbars.
    Compact,
    /// 28 px, the default.
    Regular,
    /// 32 px, dialogs and cards.
    Large,
    /// 36 px, the URL row.
    Hero,
}

impl Size {
    pub const fn height(self) -> f32 {
        match self {
            Size::Compact => CONTROL_SM,
            Size::Regular => CONTROL_MD,
            Size::Large => CONTROL_LG,
            Size::Hero => CONTROL_XL,
        }
    }

    const fn padding_x(self) -> f32 {
        match self {
            Size::Compact => SPACE_MD,
            Size::Regular | Size::Large => SPACE_LG,
            Size::Hero => SPACE_XL,
        }
    }

    const fn text_size(self) -> f32 {
        match self {
            Size::Hero => TEXT_MD,
            _ => TEXT_SM,
        }
    }

    /// Width that keeps swapped labels (Send / Cancel) from resizing the
    /// row. iced buttons have no min-width, so callers apply it as a width.
    pub const fn min_width(self) -> f32 {
        match self {
            Size::Hero => 80.0,
            _ => 0.0,
        }
    }
}

/// A text button. Add `.on_press(..)`; without it the button renders disabled.
pub fn button<'a, M: 'a>(
    t: &'a Tokens,
    label: impl text::IntoFragment<'a>,
    variant: Variant,
    size: Size,
) -> Button<'a, M> {
    build(t, Surface::Canvas, None, label, None, variant, size)
}

/// [`button`] for a raised surface (menus, dialogs).
pub fn button_on<'a, M: 'a>(
    t: &'a Tokens,
    surface: Surface,
    label: impl text::IntoFragment<'a>,
    variant: Variant,
    size: Size,
) -> Button<'a, M> {
    build(t, surface, None, label, None, variant, size)
}

/// A text button with a leading icon (Retry, Import).
pub fn button_with_icon<'a, M: 'a>(
    t: &'a Tokens,
    icon: Text<'a>,
    label: impl text::IntoFragment<'a>,
    variant: Variant,
    size: Size,
) -> Button<'a, M> {
    build(t, Surface::Canvas, Some(icon), label, None, variant, size)
}

/// A text button with a trailing icon (a disclosure chevron).
pub fn button_with_trailing_icon<'a, M: 'a>(
    t: &'a Tokens,
    label: impl text::IntoFragment<'a>,
    icon: Text<'a>,
    variant: Variant,
    size: Size,
) -> Button<'a, M> {
    build(t, Surface::Canvas, None, label, Some(icon), variant, size)
}

fn build<'a, M: 'a>(
    t: &'a Tokens,
    surface: Surface,
    leading: Option<Text<'a>>,
    label: impl text::IntoFragment<'a>,
    trailing: Option<Text<'a>>,
    variant: Variant,
    size: Size,
) -> Button<'a, M> {
    let font = if size == Size::Compact { UI_FONT } else { UI_FONT_MEDIUM };
    let mut content = row![].spacing(SPACE_MD - 2.0).align_y(Alignment::Center);
    if let Some(icon) = leading {
        content = content.push(icon.size(TEXT_SM));
    }
    content = content.push(text(label).size(size.text_size()).font(font));
    if let Some(icon) = trailing {
        content = content.push(icon.size(TEXT_SM));
    }
    let label: Element<'a, M> = container(content).center_y(Length::Fill).into();
    iced::widget::button(label)
        .height(size.height())
        .padding(Padding::from([0.0, size.padding_x() + border_width(variant)]))
        .style(move |_, status| style_on(t, variant, status, surface))
}

/// A button `width` wide with its label centered (Send and Cancel swap in
/// place without moving the row).
pub fn fixed<'a, M: 'a>(
    t: &'a Tokens,
    label: impl text::IntoFragment<'a>,
    variant: Variant,
    size: Size,
    width: f32,
) -> Button<'a, M> {
    let font = if size == Size::Compact { UI_FONT } else { UI_FONT_MEDIUM };
    iced::widget::button(container(text(label).size(size.text_size()).font(font)).center(Length::Fill))
        .width(width)
        .height(size.height())
        .padding(0)
        .style(move |_, status| style(t, variant, status))
}

/// iced draws a border inside the padding, CSS outside; bordered buttons
/// add the border to their padding to keep CSS label positions.
fn border_width(variant: Variant) -> f32 {
    if variant == Variant::Secondary { crate::ui::theme::HAIRLINE } else { 0.0 }
}

/// The style of a button `variant` in `status`, on the canvas.
pub fn style(t: &Tokens, variant: Variant, status: iced::widget::button::Status) -> iced::widget::button::Style {
    style_on(t, variant, status, Surface::Canvas)
}

/// The style of a button `variant` in `status` on `surface`.
pub fn style_on(
    t: &Tokens,
    variant: Variant,
    status: iced::widget::button::Status,
    surface: Surface,
) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    let disabled = status == Status::Disabled;
    let hovered = matches!(status, Status::Hovered | Status::Pressed);
    let (background, text_color, border_color) = match variant {
        Variant::Primary => (
            Some(if hovered { t.accent } else { t.accent_fill }),
            t.on_accent,
            Color::TRANSPARENT,
        ),
        Variant::Secondary => (
            Some(if status == Status::Pressed { surface.hover(t) } else { t.surface }),
            t.text,
            if hovered { surface.text_muted(t) } else { t.border_strong },
        ),
        Variant::Ghost => (
            hovered.then_some(surface.hover(t)),
            if hovered { t.text } else { surface.text_muted(t) },
            Color::TRANSPARENT,
        ),
        Variant::Danger => (
            Some(if hovered { Color { a: DANGER_HOVER_ALPHA, ..t.error_fill } } else { t.error_fill }),
            Color::WHITE,
            Color::TRANSPARENT,
        ),
    };
    let fade = |c: Color| if disabled { Color { a: c.a * 0.45, ..c } } else { c };
    iced::widget::button::Style {
        background: background.map(|c| Background::Color(fade(c))),
        text_color: if disabled && !matches!(variant, Variant::Primary | Variant::Danger) {
            t.text_subtle
        } else {
            fade(text_color)
        },
        border: Border {
            color: fade(border_color),
            width: border_width(variant),
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::button::Status;

    #[test]
    fn primary_uses_the_deepened_fill_and_ghost_has_no_resting_fill() {
        let t = Tokens::for_theme(0);
        let primary = style(t, Variant::Primary, Status::Active);
        assert_eq!(primary.background, Some(Background::Color(t.accent_fill)));
        assert_eq!(primary.text_color, t.on_accent);
        assert_eq!(primary.shadow, iced::Shadow::default(), "no glow");
        assert!(style(t, Variant::Ghost, Status::Active).background.is_none());
        assert!(style(t, Variant::Ghost, Status::Hovered).background.is_some());
    }

    #[test]
    fn raised_ghost_and_secondary_use_the_raised_tints() {
        let t = Tokens::for_theme(3);
        let ghost = style_on(t, Variant::Ghost, Status::Hovered, Surface::Raised);
        assert_eq!(ghost.background, Some(Background::Color(t.on_raised.hover)));
        let resting = style_on(t, Variant::Ghost, Status::Active, Surface::Raised);
        assert_eq!(resting.text_color, t.on_raised.text_muted);
        let pressed = style_on(t, Variant::Secondary, Status::Pressed, Surface::Raised);
        assert_eq!(pressed.background, Some(Background::Color(t.on_raised.hover)));
    }

    #[test]
    fn disabled_buttons_are_visibly_muted() {
        let t = Tokens::for_theme(0);
        let off = style(t, Variant::Secondary, Status::Disabled);
        assert_eq!(off.text_color, t.text_subtle);
    }
}
