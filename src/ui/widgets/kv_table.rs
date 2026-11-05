use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, button, column, container, row, text, text_editor},
};

use crate::{
    message::Message,
    ui::{
        icons,
        theme::{Palette, TEXT_SM, TEXT_XS},
    },
};

/// Mockup `.kv` column widths: check, key, value, delete.
pub const CHECK_W: f32 = 44.0;
pub const KEY_PORTION: u16 = 11;
pub const VALUE_PORTION: u16 = 16;
pub const ACTION_W: f32 = 24.0;

/// `.kv`: 1px bd-1 frame, radius 8, rows clipped to the rounded corners.
pub fn frame<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut col = column![].spacing(0);
    let n = rows.len();
    for (i, r) in rows.into_iter().enumerate() {
        col = col.push(r);
        if i + 1 < n {
            col = col.push(hairline());
        }
    }
    container(col)
        .clip(true)
        .width(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn hairline<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::border_subtle())),
            ..Default::default()
        })
        .into()
}

/// `.kv-head`: bg-1 row, 11px semibold tx-3 column titles.
pub fn head<'a>(key: &'static str, value: &'static str) -> Element<'a, Message> {
    let title = |s: &'static str| {
        text(s)
            .size(11)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..crate::ui::theme::UI_FONT
            })
            .color(Palette::text_subtle())
    };
    container(
        row![
            Space::new().width(CHECK_W),
            container(title(key)).width(Length::FillPortion(KEY_PORTION)),
            container(title(value)).width(Length::FillPortion(VALUE_PORTION)),
            Space::new().width(ACTION_W),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([9, 12])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::background())),
        ..Default::default()
    })
    .into()
}

/// `.kv-row`: check, key, value and delete cells, 4px x 12px padding.
pub fn kv_row<'a>(
    check: Element<'a, Message>,
    key: Element<'a, Message>,
    value: Element<'a, Message>,
    action: Element<'a, Message>,
) -> Element<'a, Message> {
    container(
        row![
            container(check).width(CHECK_W).center_x(CHECK_W),
            container(key).width(Length::FillPortion(KEY_PORTION)),
            container(value).width(Length::FillPortion(VALUE_PORTION)),
            container(action).width(ACTION_W).center_x(ACTION_W),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .width(Length::Fill)
    .into()
}

/// Postman's form tables (form-data, x-www-form-urlencoded): Key, Value
/// and Description in equal columns.
pub fn head3<'a>(trailing: Element<'a, Message>) -> Element<'a, Message> {
    let title = |s: &'static str| {
        text(s)
            .size(11)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..crate::ui::theme::UI_FONT
            })
            .color(Palette::text_subtle())
    };
    container(
        row![
            Space::new().width(CHECK_W),
            container(title("Key")).width(Length::FillPortion(1)),
            container(title("Value")).width(Length::FillPortion(1)),
            container(title("Description")).width(Length::FillPortion(1)),
            container(trailing).width(ACTION_W).center_x(ACTION_W),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([9, 12])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::background())),
        ..Default::default()
    })
    .into()
}

/// A [`head3`] table row: check, key, value, description, delete.
pub fn kv_row3<'a>(
    check: Element<'a, Message>,
    key: Element<'a, Message>,
    value: Element<'a, Message>,
    description: Element<'a, Message>,
    action: Element<'a, Message>,
) -> Element<'a, Message> {
    container(
        row![
            container(check).width(CHECK_W).center_x(CHECK_W),
            container(key).width(Length::FillPortion(1)),
            container(value).width(Length::FillPortion(1)),
            container(description).width(Length::FillPortion(1)),
            container(action).width(ACTION_W).center_x(ACTION_W),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .width(Length::Fill)
    .into()
}

/// [`head3`] placeholder row: "Key / Value / Description"; a click adds one.
pub fn add_row3<'a>(msg: Message) -> Element<'a, Message> {
    let ph = |s: &'static str| {
        container(
            text(s)
                .size(12)
                .font(crate::ui::theme::MONO)
                .color(Palette::text_subtle()),
        )
        .padding([6, 7])
    };
    button(
        row![
            container(icons::plus().size(12).color(Palette::text_subtle()))
                .width(CHECK_W)
                .center_x(CHECK_W),
            container(ph("Key")).width(Length::FillPortion(1)),
            container(ph("Value")).width(Length::FillPortion(1)),
            container(ph("Description")).width(Length::FillPortion(1)),
            Space::new().width(ACTION_W),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press(msg)
    .padding([4, 12])
    .width(Length::Fill)
    .style(|_, status| button::Style {
        background: matches!(status, button::Status::Hovered)
            .then(|| Background::Color(Palette::background())),
        ..Default::default()
    })
    .into()
}

/// Trailing placeholder row: muted "Key / Value", a click adds a row.
pub fn add_row<'a>(msg: Message) -> Element<'a, Message> {
    let ph = |s: &'static str| {
        container(
            text(s)
                .size(12)
                .font(crate::ui::theme::MONO)
                .color(Palette::text_subtle()),
        )
        .padding([6, 7])
    };
    button(
        row![
            container(icons::plus().size(12).color(Palette::text_subtle()))
                .width(CHECK_W)
                .center_x(CHECK_W),
            container(ph("Key")).width(Length::FillPortion(KEY_PORTION)),
            container(ph("Value")).width(Length::FillPortion(VALUE_PORTION)),
            Space::new().width(ACTION_W),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press(msg)
    .padding([4, 12])
    .width(Length::Fill)
    .style(|_, status| button::Style {
        background: matches!(status, button::Status::Hovered)
            .then(|| Background::Color(Palette::background())),
        ..Default::default()
    })
    .into()
}

/// Small 24px row delete (`×`), tx-3 until hovered.
pub fn remove_button<'a>(msg: Message) -> Element<'a, Message> {
    button(icons::close().size(12))
        .on_press(msg)
        .padding(0)
        .width(ACTION_W)
        .height(ACTION_W)
        .style(|_, status| {
            let hot = matches!(status, button::Status::Hovered);
            button::Style {
                background: hot.then(|| Background::Color(Palette::surface_high())),
                text_color: if hot {
                    Palette::text()
                } else {
                    Palette::text_subtle()
                },
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

pub fn empty_state(message: &str) -> Element<'static, Message> {
    container(
        text(message.to_owned())
            .size(TEXT_SM)
            .color(Palette::text_subtle())
            .align_x(Alignment::Center),
    )
    .padding(24)
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
                .size(TEXT_XS)
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
