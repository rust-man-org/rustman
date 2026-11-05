//! Right "Code snippet" panel (mockup `rustman-requester-snippet.html`,
//! `.rightbar`): the active request as code in the chosen language, with
//! line numbers and green strings. Hide from its header or the footer:
//! it closes flush, and the footer toggle brings it back. The left edge drags to
//! resize. The text is `AppState::snippet`, cached and secret-masked.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, column, container, pick_list, rich_text, row, scrollable, span, text},
};

use crate::{
    app::AppState,
    message::{LayoutMsg, Message},
    services::snippet::Lang,
    ui::{
        components::atoms::compact_icon_button,
        icons,
        theme::{MONO, Palette, UI_FONT_MEDIUM},
    },
};

/// Only called while the panel is shown: hidden, it closes flush (no edge
/// strip) and comes back from the footer toggle.
pub fn view(state: &AppState) -> Element<'_, Message> {
    let width = state
        .snippet_width
        .clamp(crate::app::snippet::MIN_W, crate::app::snippet::MAX_W);

    // `.rb-head`: title left, hide + copy right.
    let head = row![
        text("Code snippet")
            .size(13)
            .font(UI_FONT_MEDIUM)
            .color(Palette::text()),
        Space::new().width(Length::Fill),
        compact_icon_button(
            icons::panel_right_close(),
            "Hide panel",
            Message::Layout(LayoutMsg::SnippetToggled),
            24.0,
        ),
        compact_icon_button(
            icons::copy(),
            "Copy snippet",
            Message::Layout(LayoutMsg::SnippetCopied),
            24.0,
        ),
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .padding([10, 14]);

    // `.rb-lang`: 28px selector, bd-1 border (bd-2 on hover, accent while
    // open); the menu is the shared `.combo-menu` look.
    let lang = pick_list(Lang::ALL, Some(state.snippet_lang), |l| {
        Message::Layout(LayoutMsg::SnippetLangChanged(l))
    })
    .width(Length::Fill)
    .text_size(12)
    .padding([6, 10])
    .handle(iced::widget::pick_list::Handle::Static(
        iced::widget::pick_list::Icon {
            font: icons::ICON_FONT,
            code_point: '\u{e06d}', // chevron-down
            size: Some(iced::Pixels(10.0)),
            line_height: iced_core::text::LineHeight::default(),
            shaping: iced_core::text::Shaping::Basic,
        },
    ))
    .style(|_t, status| {
        use iced::widget::pick_list::Status;
        let open = matches!(status, Status::Opened { .. });
        iced::widget::pick_list::Style {
            text_color: Palette::text(),
            placeholder_color: Palette::text_subtle(),
            handle_color: Palette::text_subtle(),
            background: Background::Color(if open {
                Palette::surface()
            } else {
                Palette::background()
            }),
            border: Border {
                color: match status {
                    Status::Opened { .. } => Palette::accent(),
                    Status::Hovered => Palette::border(),
                    _ => Palette::border_subtle(),
                },
                width: 1.0,
                radius: 4.0.into(),
            },
        }
    })
    .menu_style(|_| iced::overlay::menu::Style {
        background: Background::Color(Palette::surface()),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: Palette::text(),
        selected_text_color: Palette::text(),
        selected_background: Background::Color(Palette::surface_high()),
        shadow: iced::Shadow {
            color: iced::Color {
                a: 0.16,
                ..iced::Color::BLACK
            },
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
    });

    let body: Element<'_, Message> = if state.snippet.is_empty() {
        text("Enter a URL to see the request as code.")
            .size(11)
            .color(Palette::text_subtle())
            .into()
    } else {
        code_lines(&state.snippet, state.snippet_lang.quote())
    };

    let panel = container(
        column![
            head,
            container(lang).padding([0, 14]),
            Space::new().height(10),
            scrollable(container(body).padding(iced::Padding {
                top: 2.0,
                right: 14.0,
                bottom: 16.0,
                left: 14.0,
            }))
            .height(Length::Fill)
            .style(crate::ui::theme::thin_scrollbar),
        ]
        .height(Length::Fill),
    )
    .width(width)
    .height(Length::Fill)
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface())),
        ..Default::default()
    });

    row![
        crate::ui::widgets::sidebar_splitter::right_panel_splitter(width, |w| {
            Message::Layout(LayoutMsg::SnippetWidthChanged(w))
        }),
        panel,
    ]
    .height(Length::Fill)
    .into()
}

/// Split `line` into (text, is_string) runs. `quote` is the language's
/// string delimiter; `"` strings honour `\"` escapes, shell `'` strings have
/// none. `in_quote` carries over lines (a multi-line shell body).
fn string_runs<'a>(line: &'a str, quote: char, in_quote: &mut bool) -> Vec<(&'a str, bool)> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if *in_quote && quote == '"' && c == '\\' && !escaped {
            escaped = true;
            continue;
        }
        if c == quote && !escaped {
            if *in_quote {
                // Closing quote belongs to the string.
                runs.push((&line[start..i + 1], true));
                start = i + 1;
            } else {
                if start < i {
                    runs.push((&line[start..i], false));
                }
                start = i;
            }
            *in_quote = !*in_quote;
        }
        escaped = false;
    }
    if start < line.len() {
        runs.push((&line[start..], *in_quote));
    }
    runs
}

/// `.rb-line`: right-aligned line number, then the line, with string
/// literals in the success colour.
fn code_lines(snippet: &str, quote: Option<char>) -> Element<'_, Message> {
    let mut col = column![].spacing(0);
    let mut in_quote = false;
    for (i, line) in snippet.lines().enumerate() {
        let runs = match quote {
            Some(q) => string_runs(line, q, &mut in_quote),
            None => vec![(line, false)],
        };
        let spans: Vec<_> = runs
            .into_iter()
            .filter(|(s, _)| !s.is_empty())
            .map(|(s, string)| {
                span(s).color(if string {
                    Palette::SUCCESS
                } else {
                    Palette::text()
                })
            })
            .collect();
        col = col.push(
            row![
                container(
                    text((i + 1).to_string())
                        .size(11)
                        .font(MONO)
                        .color(Palette::text_subtle())
                )
                .width(14)
                .align_right(14),
                rich_text::<(), Message, iced::Theme, iced::Renderer>(spans)
                    .size(11)
                    .font(MONO)
                    .line_height(iced::Pixels(18.0))
                    .width(Length::Fill),
            ]
            .spacing(10),
        );
    }
    col.into()
}


#[cfg(test)]
mod tests {
    use super::string_runs;

    #[test]
    fn double_quoted_strings_honour_escapes() {
        let mut q = false;
        let runs = string_runs(r#"a = "x\"y" + b"#, '"', &mut q);
        assert_eq!(
            runs,
            vec![("a = ", false), (r#""x\"y""#, true), (" + b", false)]
        );
        assert!(!q);
    }

    #[test]
    fn shell_strings_span_lines() {
        let mut q = false;
        let first = string_runs("--data-raw '{", '\'', &mut q);
        assert_eq!(first, vec![("--data-raw ", false), ("'{", true)]);
        assert!(q);
        let second = string_runs("}'", '\'', &mut q);
        assert_eq!(second, vec![("}'", true)]);
        assert!(!q);
    }
}
