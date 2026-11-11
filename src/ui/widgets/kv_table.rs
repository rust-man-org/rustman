use iced::{
    Alignment, Element, Length,
    widget::{Space, button, column, container, row, text, text_editor},
};

use crate::{
    message::Message,
    ui::theme::{Palette, TEXT_CAPTION, TEXT_SM},
};

pub fn empty_state(message: &str) -> Element<'static, Message> {
    container(
        text(message.to_owned())
            .size(TEXT_SM)
            .color(Palette::text_subtle())
            .align_x(Alignment::Center),
    )
    .padding(crate::ui::theme::SPACE_2XL)
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

pub fn bulk_panel<'a>(
    content: &'a text_editor::Content,
    on_edited: fn(text_editor::Action) -> Message,
    on_toggle: Message,
) -> Element<'a, Message> {
    let toolbar = container(
        row![
            text("Key: Value per line · prefix # to disable")
                .size(TEXT_CAPTION)
                .color(Palette::text_subtle()),
            Space::new().width(Length::Fill),
            button(text("Done").size(TEXT_SM).color(Palette::accent()))
                .on_press(on_toggle)
                .style(iced::widget::button::text)
                .padding([4, 8]),
        ]
        .align_y(Alignment::Center)
        .padding([6, 10]),
    )
    .style(crate::ui::styles::section_header)
    .width(Length::Fill);

    let editor = text_editor(content)
        .on_action(on_edited)
        .height(Length::Fill)
        .font(crate::ui::theme::MONO)
        .style(crate::ui::styles::scripts_editor);

    column![toolbar, editor]
        .spacing(0)
        .height(Length::Fill)
        .into()
}
