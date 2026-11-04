use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, container, row, text},
};

use crate::{
    app::AppState,
    message::Message,
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_XS},
    },
};

pub(super) fn status_bar(state: &AppState) -> Element<'_, Message> {
    let env_el: Element<Message> = match state.environments.iter().find(|e| e.is_active) {
        Some(e) => row![
            icons::globe().size(11).color(Palette::accent()),
            text(e.name.as_str())
                .size(TEXT_XS)
                .color(Palette::text_muted()),
        ]
        .spacing(5)
        .align_y(Alignment::Center)
        .into(),
        None => row![
            icons::globe().size(11).color(Palette::text_subtle()),
            text("No environment")
                .size(TEXT_XS)
                .color(Palette::text_subtle()),
        ]
        .spacing(5)
        .align_y(Alignment::Center)
        .into(),
    };

    let msg = state.status_message.as_deref().unwrap_or("");

    // Local drafts (01): count plus conflicts; click toggles the filter.
    let drafts_el: Element<Message> = match state.drafts.count() {
        0 => Space::new().into(),
        n => {
            let conflicts = state.drafts.conflicted();
            let mut r = row![
                crate::ui::icons::dot(Palette::accent()),
                text(format!("{n} local draft{}", if n == 1 { "" } else { "s" }))
                    .size(TEXT_XS)
                    .color(Palette::text_muted()),
            ]
            .spacing(5)
            .align_y(Alignment::Center);
            if conflicts > 0 {
                r = r.push(
                    text(format!("· {conflicts} with conflicts"))
                        .size(TEXT_XS)
                        .color(Palette::ERROR),
                );
            }
            row![
                iced::widget::button(r)
                    .on_press(Message::Draft(crate::message::draft::DraftMsg::ToggleFilter))
                    .padding([0, 2])
                    .style(|_t, _s| iced::widget::button::Style::default()),
                separator(),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        }
    };

    container(
        row![
            if msg.is_empty() {
                text("").size(TEXT_XS)
            } else {
                text(msg).size(TEXT_XS).color(Palette::SUCCESS)
            },
            Space::new().width(Length::Fill),
            drafts_el,
            env_el,
            separator(),
            row![
                text("Ctrl+P")
                    .size(TEXT_XS)
                    .color(Palette::text_subtle())
                    .font(MONO),
                text("Command").size(TEXT_XS).color(Palette::text_subtle()),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([4, 12]),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Palette::chrome())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .height(28)
    .width(Length::Fill)
    .into()
}

fn separator() -> Element<'static, Message> {
    container(Space::new().width(1).height(12))
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::border())),
            ..Default::default()
        })
        .into()
}
