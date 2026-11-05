use crate::{
    message::{AppMsg, Message},
    state::notifications::Toast,
    ui::{
        components::atoms::Button,
        icons,
        theme::{Palette, UI_FONT_MEDIUM},
    },
};
use iced::{
    Border, Color, Element, Length, Shadow, Vector,
    widget::{Space, column, container, row, text},
};

pub fn view(toast: &Toast) -> Element<'_, Message> {
    let mut labels = column![
        text(&toast.title)
            .size(12)
            .font(UI_FONT_MEDIUM)
            .color(Palette::text())
    ]
    .spacing(2);
    if let Some(detail) = &toast.detail {
        labels = labels.push(text(detail).size(11).color(Palette::text_muted()));
    }
    let content = if toast.detail.is_some() {
        row![
            labels.width(Length::Fill),
            Button::new("Dismiss")
                .ghost()
                .on_press(Message::App(AppMsg::ToastDismissed(toast.id.clone()))),
            icons::check_circle().size(14).color(Palette::SUCCESS)
        ]
        .spacing(8)
    } else {
        row![
            icons::check_circle().size(13).color(Palette::SUCCESS),
            labels,
            Space::new().width(Length::Fill),
            Button::new("Dismiss")
                .ghost()
                .on_press(Message::App(AppMsg::ToastDismissed(toast.id.clone())))
        ]
        .spacing(8)
    };
    container(content.align_y(iced::Alignment::Center))
        .padding([12, 14])
        .width(360)
        .style(|_| container::Style {
            background: Some(Palette::surface_raised().into()),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.18,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        })
        .into()
}
