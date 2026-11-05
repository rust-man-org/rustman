//! Small toolbar controls shared by the request and response panes
//! (mockup `.p-bulk`, `.dd-mini`, `.p-more`, `.resp-icons`).

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Text, button, container, row, text},
};

use crate::{
    message::{Message, SidebarMsg},
    ui::{
        components::{atoms::tooltip::tooltip, molecules::menu},
        icons,
        theme::{Palette, UI_FONT_MEDIUM},
    },
};

/// `.p-bulk` / `.resp-action`: 24px text button, optional leading icon,
/// 11px tx-2, bg-2 on hover. `None` renders it disabled.
pub fn text_button<'a>(
    icon: Option<Text<'static>>,
    label: &str,
    msg: Option<Message>,
) -> Element<'a, Message> {
    let enabled = msg.is_some();
    let color = if enabled {
        Palette::text_muted()
    } else {
        Palette::text_subtle()
    };
    let mut content = row![].spacing(5).align_y(Alignment::Center);
    if let Some(i) = icon {
        content = content.push(i.size(12).color(color));
    }
    content = content.push(text(label.to_owned()).size(11).color(color));
    button(container(content).height(24).center_y(24))
        .on_press_maybe(msg)
        .padding([0, 8])
        .style(move |_, status| button::Style {
            background: (enabled && matches!(status, button::Status::Hovered))
                .then(|| Background::Color(Palette::surface_high())),
            text_color: color,
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// `.resp-icons button` / `.p-more`: 24px square icon button, tx-3 at
/// rest, tx-1 on bg-2 when hovered or `active`.
pub fn icon_button<'a>(
    icon: Text<'static>,
    tip: &str,
    msg: Option<Message>,
    active: bool,
) -> Element<'a, Message> {
    let enabled = msg.is_some();
    let btn = button(
        container(icon.size(13))
            .width(24)
            .height(24)
            .center_x(24)
            .center_y(24),
    )
    .on_press_maybe(msg)
    .padding(0)
    .style(move |_, status| {
        let hot = active || (enabled && matches!(status, button::Status::Hovered));
        button::Style {
            background: hot.then(|| Background::Color(Palette::surface_high())),
            text_color: if hot {
                Palette::text()
            } else if enabled {
                Palette::text_muted()
            } else {
                Color {
                    a: 0.5,
                    ..Palette::text_subtle()
                }
            },
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    });
    tooltip(btn, tip.to_owned())
}

/// `.dd-mini summary`: bd-1 framed chip with a chevron; opens `menu_key`
/// in the shared context-menu slot and shows `items` under it.
pub fn dropdown<'a>(
    label: &str,
    menu_key: &'static str,
    open: bool,
    height: f32,
    items: Vec<Element<'a, Message>>,
    width: f32,
) -> Element<'a, Message> {
    let chip = button(
        container(
            row![
                text(label.to_owned())
                    .size(11)
                    .font(UI_FONT_MEDIUM)
                    .color(Palette::text_muted()),
                icons::chevron_down().size(11).color(Palette::text_subtle()),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .height(height)
        .center_y(height),
    )
    .on_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
        menu_key.to_owned(),
    )))
    .padding([0, 9])
    .style(move |_, status| {
        let hot = open || matches!(status, button::Status::Hovered);
        button::Style {
            background: Some(Background::Color(if hot {
                Palette::surface_high()
            } else {
                Palette::background()
            })),
            text_color: Palette::text_muted(),
            border: Border {
                color: if hot {
                    Palette::border()
                } else {
                    Palette::border_subtle()
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }
    });
    anchored_menu(chip.into(), open, items, width)
}

/// Attach `items` as a popup menu under `anchor` while `open`.
pub fn anchored_menu<'a>(
    anchor: Element<'a, Message>,
    open: bool,
    items: Vec<Element<'a, Message>>,
    width: f32,
) -> Element<'a, Message> {
    if !open {
        return anchor;
    }
    crate::ui::widgets::popup::popup(
        anchor,
        menu(items, Length::Fixed(width)),
        Message::Sidebar(SidebarMsg::ContextMenuClosed),
    )
}
