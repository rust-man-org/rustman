use std::fmt::Write as _;

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, button, column, container, row, text, text_input},
};

use crate::{
    domain::response::HttpResponse,
    message::{Message, ResponseMsg},
    services::spreadsheet::ParsedSheet,
    state::tabs::{RequestTabState, ResponsePreview},
    ui::{
        icons,
        theme::{MONO, Palette, SPACE_LG, SPACE_XS, TEXT_CAPTION, TEXT_LG, TEXT_SM},
    },
};

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let Some(resp) = tab.response.as_ref() else {
        return Space::new().into();
    };

    if resp.is_binary {
        // An oversized binary download has no bytes kept in memory at all, so
        // there is nothing to build a preview from. Say that plainly rather
        // than falling through to a preview that can never appear.
        if resp.body_stored && resp.binary_data.is_none() {
            return binary_view_with_note(
                resp,
                "This download is too large to preview in the app. \
                 The response completed successfully — only the in-app preview is skipped.",
            );
        }
        return match &tab.response_preview {
            ResponsePreview::Spreadsheet(Ok(sheet)) => spreadsheet_view(sheet),
            ResponsePreview::Spreadsheet(Err(err)) => binary_view_with_note(resp, err),
            ResponsePreview::None => binary_view(resp),
        };
    }

    // HTML has no rendered preview: show the readable-text conversion, with
    // the raw source still available via Save or Copy.
    if resp.is_html()
        && let Some(rendered) = tab.html_fallback_text.as_deref()
    {
        return simple_html_view(rendered, !resp.body_stored);
    }

    text_body(tab, resp)
}

fn text_body<'a>(tab: &'a RequestTabState, resp: &'a HttpResponse) -> Element<'a, Message> {
    let body: Element<Message> = if tab.viewer_processing {
        container(text("Parsing…").size(TEXT_SM).color(Palette::text_subtle()))
            .padding([8, 12])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        tab.response_editor
            .view()
            .map(|m| Message::Response(ResponseMsg::ViewerEdited(m)))
    };
    let mut panel = column![].height(Length::Fill);
    if let Some(bar) = paging_bar(tab, resp) {
        panel = panel.push(bar);
    }
    if tab.viewer_search.open {
        panel = panel.push(find_bar(&tab.viewer_search));
    }
    panel
        .push(container(body).width(Length::Fill).height(Length::Fill))
        .into()
}

/// Shown only for a body too large for one viewer page: which page this
/// is, how much of the body it holds, and the page arrows.
fn paging_bar<'a>(tab: &RequestTabState, resp: &HttpResponse) -> Option<Element<'a, Message>> {
    let held = tab.response_truncated_bytes?;
    let page_count = tab.viewer_page_count();
    let (start, end) = tab.viewer_page_span();
    let mut note = format!(
        "Page {} of {} · showing {} of {}",
        tab.viewer_page + 1,
        page_count,
        human_bytes(end - start),
        human_bytes(held),
    );
    let beyond = resp.body_size.saturating_sub(resp.body.len());
    if resp.body_stored && beyond > 0 {
        let _ = write!(note, " (+{} past the preview limit)", human_bytes(beyond));
    }
    let page = tab.viewer_page;
    let step = |icon: text::Text<'a>, enabled: bool, msg: ResponseMsg| {
        button(icon.size(11).color(Palette::text_muted()))
            .padding([4, 6])
            .style(crate::ui::styles::ghost_button)
            .on_press_maybe(enabled.then_some(Message::Response(msg)))
    };
    Some(
        container(
            row![
                text(note).size(TEXT_CAPTION).color(Palette::warning()),
                Space::new().width(Length::Fill),
                step(icons::arrow_left(), page > 0, ResponseMsg::ViewerPagePrev),
                step(icons::arrow_right(), page + 1 < page_count, ResponseMsg::ViewerPageNext),
            ]
            .spacing(2)
            .align_y(Alignment::Center),
        )
        .padding([2, 12])
        .width(Length::Fill)
        .into(),
    )
}

/// Widget id of the find field, for focus operations.
pub const SEARCH_INPUT_ID: &str = "response-body-search";

/// The body find bar: query field, match-case toggle, "3 / 41" counter,
/// previous/next and close. It searches the whole body, across pages (see
/// `services::format::BodySearch`); everything shown here was computed when
/// the query changed, so rendering does no searching. The query itself is
/// clamped to `MAX_SEARCH_QUERY_BYTES`, so the field never shapes a
/// megabyte paste.
fn find_bar(search: &crate::services::format::BodySearch) -> Element<'_, Message> {
    let has_matches = !search.matches.is_empty();

    let input = text_input("Find in body", &search.query)
        .id(SEARCH_INPUT_ID)
        .on_input(|q| Message::Response(ResponseMsg::SearchQueryChanged(q)))
        // Fallback only: `FindKeys` handles Enter itself so it can tell
        // Shift+Enter apart.
        .on_submit(Message::Response(ResponseMsg::SearchNext))
        .size(TEXT_SM)
        .padding([3, 8])
        .width(Length::Fixed(240.0))
        .style(crate::ui::styles::field_input);

    let counter: Element<Message> = match search.counter() {
        Some(label) => text(label)
            .size(TEXT_CAPTION)
            .color(if search.no_results() {
                Palette::error()
            } else {
                Palette::text_muted()
            })
            .into(),
        None => Space::new().into(),
    };

    let case_on = search.case_sensitive;
    let case_toggle = crate::ui::styles::tip(
        button(text("Aa").size(TEXT_SM).font(MONO))
            .on_press(Message::Response(ResponseMsg::SearchToggleCase))
            .padding([2, 6])
            .style(move |theme, status| {
                let mut style = crate::ui::styles::ghost_button(theme, status);
                if case_on {
                    style.background = Some(Background::Color(Palette::accent_soft()));
                    style.text_color = Palette::accent();
                }
                style
            }),
        if case_on {
            "Match case: on"
        } else {
            "Match case: off"
        },
    );

    let step_button = |icon: text::Text<'static>, msg: ResponseMsg, label: &str| {
        let btn = button(icon.size(11))
            .padding([4, 6])
            .style(crate::ui::styles::ghost_button);
        let btn = if has_matches {
            btn.on_press(Message::Response(msg))
        } else {
            btn
        };
        crate::ui::styles::tip(btn, label.to_owned())
    };

    let bar = row![
        icons::search().size(11).color(Palette::text_subtle()),
        Element::new(FindKeys {
            input: input.into()
        }),
        case_toggle,
        // Fixed width so the step buttons don't shift as the count changes
        // under the pointer.
        container(counter).width(Length::Fixed(96.0)),
        step_button(
            icons::arrow_up(),
            ResponseMsg::SearchPrev,
            "Previous match (Shift+Enter)"
        ),
        step_button(
            icons::arrow_down(),
            ResponseMsg::SearchNext,
            "Next match (Enter)"
        ),
        crate::ui::styles::tip(
            button(icons::close().size(11))
                .on_press(Message::Response(ResponseMsg::SearchClose))
                .padding([4, 6])
                .style(crate::ui::styles::ghost_button),
            "Close (Esc)",
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    container(bar)
        .padding([4, 12])
        .width(Length::Fill)
        .into()
}

/// Keys the find field owns while it is focused: Enter / F3 step forward and
/// Shift+Enter / Shift+F3 back (a bare `text_input` can't tell those apart —
/// `on_submit` carries no modifiers), Escape closes the bar, and Ctrl+F
/// re-selects the query. The keys are captured so nothing else acts on them;
/// while the field is not focused every event passes straight through.
struct FindKeys<'a> {
    input: Element<'a, Message>,
}

impl FindKeys<'_> {
    /// Whether the wrapped `text_input` has keyboard focus. Its state is
    /// checked by tag before the downcast, so a different child widget can
    /// never trigger the downcast's panic (fatal under `panic = "abort"`).
    fn input_focused(tree: &iced::advanced::widget::Tree) -> bool {
        use iced::advanced::widget::tree;
        type InputState =
            text_input::State<<iced::Renderer as iced::advanced::text::Renderer>::Paragraph>;
        tree.tag == tree::Tag::of::<InputState>()
            && tree.state.downcast_ref::<InputState>().is_focused()
    }
}

impl iced::advanced::Widget<Message, iced::Theme, iced::Renderer> for FindKeys<'_> {
    fn size(&self) -> iced::Size<Length> {
        self.input.as_widget().size()
    }

    fn size_hint(&self) -> iced::Size<Length> {
        self.input.as_widget().size_hint()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.input)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.input));
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.input
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.input
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        use iced::keyboard::{self, Key, key::Named};
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
            && Self::input_focused(&tree.children[0])
        {
            let msg = match key.as_ref() {
                Key::Named(Named::Enter | Named::F3) if modifiers.shift() => {
                    Some(ResponseMsg::SearchPrev)
                }
                Key::Named(Named::Enter | Named::F3) => Some(ResponseMsg::SearchNext),
                Key::Named(Named::Escape) => Some(ResponseMsg::SearchClose),
                Key::Character(c) if c.eq_ignore_ascii_case("f") && modifiers.command() => {
                    Some(ResponseMsg::SearchOpen)
                }
                _ => None,
            };
            if let Some(msg) = msg {
                shell.publish(Message::Response(msg));
                shell.capture_event();
                return;
            }
        }
        self.input.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.input
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.input.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
}

/// Readable-text HTML rendering. Borrows the pre-rendered fallback (computed
/// once per response, not per frame). The raw source stays one Open or Save
/// away, both of which need the complete body.
fn simple_html_view<'a>(rendered: &'a str, full_body: bool) -> Element<'a, Message> {
    let (open_action, save_action): (Element<Message>, Element<Message>) = if full_body {
        (open_in_browser_button(), save_text_button())
    } else {
        (Space::new().width(0).into(), Space::new().width(0).into())
    };

    container(
        column![
            row![
                text("HTML shown as readable text. Open or save the response for the full source.")
                    .size(TEXT_CAPTION)
                    .color(Palette::text_subtle()),
                Space::new().width(Length::Fill),
                open_action,
                save_action,
            ]
            .align_y(Alignment::Center),
            crate::ui::components::scroll::vertical(crate::ui::theme::Palette::tokens(), text(rendered).size(TEXT_SM).color(Palette::text()))
                .height(Length::Fill),
        ]
        .spacing(4)
        .height(Length::Fill),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Compact "Save" action matching the toolbar's Copy button. Writes the
/// response body — HTML source, JSON, text — to a file chosen by the user.
fn save_text_button() -> Element<'static, Message> {
    text_action_button("Save", Message::Response(ResponseMsg::SaveBody))
}

/// Renders the HTML body in the system browser via a temp file, so a page
/// gets a real engine and real CSS/scripts without one being linked into the
/// app.
fn open_in_browser_button() -> Element<'static, Message> {
    text_action_button(
        "Open in browser",
        Message::Response(ResponseMsg::OpenBodyInBrowser),
    )
}

/// Compact text action matching the toolbar's Copy button style.
fn text_action_button(label: &'static str, msg: Message) -> Element<'static, Message> {
    button(text(label).size(TEXT_SM).color(Palette::text_muted()))
        .on_press(msg)
        .style(|_t, status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: if hovered {
                    Some(Background::Color(Palette::hover()))
                } else {
                    None
                },
                text_color: Palette::text_muted(),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .padding([4, 8])
        .into()
}

/// Formats a byte count as a short human-readable size.
fn human_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn binary_view(resp: &HttpResponse) -> Element<'static, Message> {
    let is_pdf = resp.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            && value.to_ascii_lowercase().contains("pdf")
    });
    binary_view_with_note(
        resp,
        if is_pdf {
            "PDFs aren't rendered in the app — save the response and open it in your PDF viewer."
        } else {
            "This response isn't valid text, so it can't be shown in the editor. Save it to open it with the right app."
        },
    )
}

fn binary_view_with_note(resp: &HttpResponse, note: &str) -> Element<'static, Message> {
    let content_type = resp
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| "unknown type".to_owned());

    let size_str = if resp.body_size < 1024 {
        format!("{} B", resp.body_size)
    } else if resp.body_size < 1024 * 1024 {
        format!("{:.1} KB", resp.body_size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", resp.body_size as f64 / (1024.0 * 1024.0))
    };

    let mut content = column![
        text("Binary response")
            .size(TEXT_LG)
            .color(Palette::text())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().height(4),
        text(format!("{content_type} · {size_str}"))
            .size(TEXT_SM)
            .color(Palette::text_muted()),
        Space::new().height(2),
        text(note.to_owned())
            .size(TEXT_SM)
            .color(Palette::text_subtle()),
    ]
    .align_x(Alignment::Center);

    // Binary bodies (PDFs included) have no inline renderer: offer to save
    // the bytes so they can be opened externally. Only offered while the
    // full body is still in memory; oversized downloads are dropped and the
    // note above says so.
    if resp.binary_data.is_some() {
        content = content.push(Space::new().height(14)).push(
            button(text("Save response…").size(TEXT_SM).color(Palette::text()))
                .on_press(Message::Response(ResponseMsg::SaveBody))
                .style(|_t, status| {
                    let hovered = matches!(status, iced::widget::button::Status::Hovered);
                    iced::widget::button::Style {
                        background: Some(Background::Color(if hovered {
                            Palette::hover()
                        } else {
                            Palette::surface_high()
                        })),
                        text_color: Palette::text(),
                        border: Border {
                            color: Palette::border(),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    }
                })
                .padding([6, 14]),
        );
    }

    container(content)
        .padding([24, 24])
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// The first sheet as plain text in fixed-width columns: two paragraphs
/// laid out once, where a widget per cell cost a rebuild of every cell on
/// every frame.
fn spreadsheet_view(sheet: &ParsedSheet) -> Element<'_, Message> {
    fn cells(lines: &str, color: iced::Color) -> iced::widget::Text<'_> {
        text(lines).font(MONO).size(TEXT_SM).color(color).wrapping(iced_core::text::Wrapping::None)
    }
    let t = Palette::tokens();
    let toolbar = container(text(sheet.summary.as_str()).size(TEXT_CAPTION).color(t.text_subtle))
        .padding([SPACE_XS, SPACE_LG]);
    let table = column![cells(&sheet.header, t.text), cells(&sheet.body, t.text_muted)].padding([SPACE_XS, SPACE_LG]);
    column![
        toolbar,
        crate::ui::components::scroll::both(t, table)
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}
