//! Global footer frame; slots contain caller-owned actions and status.
use crate::ui::theme::Palette;
use iced::{
    Alignment, Element, Length,
    widget::{container, row},
};

pub fn footer_bar<'a, M: 'a>(
    left: Element<'a, M>,
    center: Element<'a, M>,
    right: Element<'a, M>,
) -> Element<'a, M> {
    container(
        row![left, container(center).width(Length::Fill), right]
            .spacing(10)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .padding([0, 6])
    .height(26)
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Palette::chrome().into()),
        text_color: Some(Palette::text()),
        ..Default::default()
    })
    .into()
}
