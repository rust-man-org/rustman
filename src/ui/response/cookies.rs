use iced::{
    Background, Border, Element, Length,
    widget::{column, container, row, text},
};

use crate::{
    message::Message,
    state::tabs::RequestTabState,
    ui::{theme::Palette, widgets::kv_table},
};

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let Some(resp) = &tab.response else {
        return kv_table::empty_state("No response yet.");
    };

    let cookies = set_cookies(&resp.headers);

    if cookies.is_empty() {
        return kv_table::empty_state("No Set-Cookie headers in this response.");
    }

    let mut col = column![].spacing(0);
    let header_bg = Palette::surface_high();
    col = col.push(
        container(
            row![
                container(
                    text("#")
                        .size(10)
                        .color(Palette::text_subtle())
                        .font(crate::ui::theme::MONO),
                )
                .width(32)
                .padding([3, 4]),
                container(
                    text("Name")
                        .size(10)
                        .color(Palette::text_subtle())
                        .font(crate::ui::theme::MONO),
                )
                .width(180)
                .padding([3, 4]),
                container(
                    text("Value")
                        .size(10)
                        .color(Palette::text_subtle())
                        .font(crate::ui::theme::MONO),
                )
                .width(Length::Fill)
                .padding([3, 4]),
            ]
            .align_y(iced::Alignment::Center),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(header_bg)),
            border: Border {
                color: Palette::border_subtle(),
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .padding([0, 4])
        .width(Length::Fill),
    );

    for (i, (name, value)) in cookies.iter().enumerate() {
        let bg = if i % 2 == 0 {
            Some(Background::Color(Palette::row_odd()))
        } else {
            None
        };
        let name = name.to_string();
        let value = value.to_string();

        col = col.push(
            container(
                row![
                    container(
                        text(format!("{}", i + 1))
                            .size(10)
                            .color(Palette::text_subtle())
                            .font(crate::ui::theme::MONO),
                    )
                    .width(32)
                    .padding([2, 4]),
                    container(
                        text(name)
                            .size(11)
                            .color(Palette::accent())
                            .font(crate::ui::theme::MONO),
                    )
                    .width(180)
                    .padding([2, 4]),
                    container(
                        text(value)
                            .size(11)
                            .color(Palette::text())
                            .font(crate::ui::theme::MONO),
                    )
                    .width(Length::Fill)
                    .padding([2, 4]),
                ]
                .align_y(iced::Alignment::Center)
                .width(Length::Fill),
            )
            .style(move |_| iced::widget::container::Style {
                background: bg,
                border: Border {
                    color: Palette::border_subtle(),
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .padding([1, 4])
            .width(Length::Fill),
        );
    }

    crate::ui::components::scroll::vertical(crate::ui::theme::Palette::tokens(), col)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Name and value of every cookie a response sets. Repeated `Set-Cookie`
/// headers arrive joined by newlines in one header entry.
fn set_cookies(headers: &std::collections::HashMap<String, String>) -> Vec<(&str, &str)> {
    headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
        .flat_map(|(_, v)| v.lines())
        .filter_map(|line| {
            // "name=value" comes first; `Path=…`, `HttpOnly` etc. follow `;`.
            let pair = line.split(';').next()?.trim();
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (!name.trim().is_empty()).then(|| (name.trim(), value.trim()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_set_cookie_header_is_listed() {
        let headers = std::collections::HashMap::from([(
            "set-cookie".to_owned(),
            "sid=abc; Path=/; HttpOnly\ntheme=dark\nempty=".to_owned(),
        )]);
        assert_eq!(set_cookies(&headers), [("sid", "abc"), ("theme", "dark"), ("empty", "")]);
    }
}
