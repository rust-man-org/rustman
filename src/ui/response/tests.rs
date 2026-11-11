use iced::{
    Background, Border, Element, Length,
    widget::{column, container, row, text, text_editor},
};

use crate::{
    message::{Message, ResponseMsg},
    state::tabs::RequestTabState,
    ui::{theme::Palette, widgets::kv_table},
};

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    if let Some(err) = &tab.script_error {
        return container(
            column![
                text("Script error")
                    .size(13)
                    .color(Palette::error())
                    .font(crate::ui::theme::UI_FONT_MEDIUM),
                iced::widget::Space::new().height(6),
                text(err.clone())
                    .size(12)
                    .color(Palette::text_muted())
                    .font(crate::ui::theme::MONO),
            ]
            .align_x(iced::Alignment::Center),
        )
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    if tab.test_results.is_empty() && tab.script_logs.is_empty() {
        return kv_table::empty_state(
            "No tests yet. Write a test script in the Scripts tab — test(\"name\", condition).",
        );
    }

    let mut col = column![].spacing(0);

    if !tab.script_logs.is_empty() {
        // A real text_editor (kept read-only-in-effect — see console_editor's
        // doc comment) instead of plain `text` widgets, so the printed
        // values are actually selectable and copyable for debugging.
        let line_count = tab.console_editor.line_count().max(1);
        let editor_height = ((line_count as f32) * 20.0 + 24.0).clamp(140.0, 400.0);
        col = col.push(
            container(
                text("Console")
                    .size(11)
                    .color(Palette::text_muted())
                    .font(crate::ui::theme::UI_FONT_MEDIUM),
            )
            .padding([8, 12]),
        );
        col = col.push(
            container(
                text_editor(&tab.console_editor)
                    .on_action(|a| Message::Response(ResponseMsg::ConsoleEdited(a)))
                    .font(crate::ui::theme::MONO)
                    .size(12)
                    .height(editor_height)
                    .style(|_theme, _status| text_editor::Style {
                        background: Background::Color(Palette::surface()),
                        border: Border {
                            color: Palette::border_subtle(),
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        placeholder: Palette::text_subtle(),
                        value: Palette::text(),
                        selection: Palette::accent_soft(),
                    }),
            )
            .padding([2, 10])
            .width(Length::Fill),
        );
    }

    if tab.test_results.is_empty() {
        return crate::ui::components::scroll::vertical(crate::ui::theme::Palette::tokens(), col)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }

    let passed_count = tab.test_results.iter().filter(|r| r.passed).count();
    let total = tab.test_results.len();

    col = col.push(
        container(
            text(format!("{passed_count} / {total} passed"))
                .size(11)
                .color(if passed_count == total {
                    Palette::success()
                } else {
                    Palette::error()
                })
                .font(crate::ui::theme::UI_FONT_MEDIUM),
        )
        .padding([8, 12]),
    );

    for result in &tab.test_results {
        let (icon, color) = if result.passed {
            ("✓", Palette::success())
        } else {
            ("✗", Palette::error())
        };
        col = col.push(
            container(
                row![
                    container(
                        text(icon)
                            .size(12)
                            .color(color)
                            .font(crate::ui::theme::MONO)
                    )
                    .width(24),
                    text(result.name.clone()).size(12).color(Palette::text()),
                ]
                .align_y(iced::Alignment::Center)
                .width(Length::Fill),
            )
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(Palette::surface())),
                border: Border {
                    color: Palette::border_subtle(),
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .padding([6, 12])
            .width(Length::Fill),
        );
    }

    crate::ui::components::scroll::vertical(crate::ui::theme::Palette::tokens(), col)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
