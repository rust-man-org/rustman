//! Compact footer action with shared hover treatment and tooltip.
use crate::ui::{components::atoms::tooltip::tooltip_above, theme::Palette};
use iced::{
    Alignment, Border, Element, Length,
    widget::{Text, button, row, text},
};

pub fn footer_action<'a, M: Clone + 'a>(
    icon: Option<Text<'static>>,
    label: impl Into<String>,
    tip: impl Into<String>,
    message: M,
) -> Element<'a, M> {
    let label = label.into();
    let mut content = row![]
        .spacing(5)
        .height(Length::Fill)
        .align_y(Alignment::Center);
    if let Some(icon) = icon {
        content = content.push(icon.size(12).color(Palette::text_muted()));
    }
    if !label.is_empty() {
        content = content.push(text(label).size(10).color(Palette::text_muted()));
    }
    tooltip_above(
        button(content)
            .height(24)
            .padding([0, 5])
            .on_press(message)
            .style(|_, status| button::Style {
                background: matches!(status, button::Status::Hovered)
                    .then(|| Palette::hover().into()),
                border: Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        tip.into(),
    )
}
