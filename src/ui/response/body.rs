use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text},
};

use crate::{
    domain::response::HttpResponse,
    message::{Message, ResponseMsg},
    services::spreadsheet::ParsedSheet,
    state::tabs::{RequestTabState, ResponsePreview},
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_LG, TEXT_SM, TEXT_XS},
        widgets::mini,
    },
};

pub fn view(tab: &RequestTabState, spinner_frame: u32) -> Element<'_, Message> {
    if tab.is_loading {
        return loading_view(spinner_frame);
    }

    let Some(resp) = tab.response.as_ref() else {
        return empty_view();
    };

    if let Some(err) = &resp.error {
        return error_view(err);
    }

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

    let page_count = tab.viewer_page_count();
    let size_note: Element<Message> = match tab.response_truncated_bytes {
        Some(held_len) => {
            let mut note = format!(
                " · page {}/{} · showing {} of {}",
                tab.viewer_page + 1,
                page_count,
                human_bytes(tab.response_editor_bytes()),
                human_bytes(held_len),
            );
            // Bodies past the HTTP inline cap are cut before the viewer ever
            // sees them — say how much never made it, instead of implying the
            // pages cover everything.
            if resp.body_stored {
                let beyond = resp.body_size.saturating_sub(resp.body.len());
                if beyond > 0 {
                    note.push_str(&format!(" (+{} past preview limit)", human_bytes(beyond)));
                }
            }
            text(note).size(TEXT_XS).color(Palette::WARNING).into()
        }
        None => Space::new().into(),
    };

    let page_nav: Element<Message> = if page_count > 1 {
        let page = tab.viewer_page;
        let prev = button(icons::arrow_left().size(11).color(Palette::text_muted()))
            .padding([4, 6])
            .style(iced::widget::button::text);
        let prev = if page > 0 {
            prev.on_press(Message::Response(ResponseMsg::ViewerPagePrev))
        } else {
            prev
        };
        let next = button(icons::arrow_right().size(11).color(Palette::text_muted()))
            .padding([4, 6])
            .style(iced::widget::button::text);
        let next = if page + 1 < page_count {
            next.on_press(Message::Response(ResponseMsg::ViewerPageNext))
        } else {
            next
        };
        row![
            prev,
            text(format!("Page {}/{}", page + 1, page_count))
                .size(TEXT_XS)
                .color(Palette::text_muted()),
            next,
        ]
        .spacing(2)
        .align_y(Alignment::Center)
        .into()
    } else {
        Space::new().into()
    };

    // Mockup `.rsp-tools`: format chip and line count on the left; wrap,
    // search and copy icons on the right. Save lives in the head's ⋯ menu.
    let format_chip = container(
        text(if tab.viewer_full_is_json {
            "JSON"
        } else {
            "Text"
        })
        .size(10)
        .font(crate::ui::theme::UI_FONT_MEDIUM)
        .color(Palette::text_muted()),
    )
    .height(24)
    .center_y(24)
    .padding([0, 9])
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Palette::background())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });
    let wrap_on = tab.response_editor.wrap_enabled();
    let toolbar = container(
        row![
            format_chip,
            text(format!("{} lines", tab.response_viewer_lines))
                .size(10)
                .color(Palette::text_subtle()),
            size_note,
            Space::new().width(Length::Fill),
            page_nav,
            mini::icon_button(
                icons::wrap_text(),
                if wrap_on {
                    "Unwrap lines"
                } else {
                    "Wrap lines"
                },
                Some(Message::Response(ResponseMsg::ViewerWrapToggled)),
                wrap_on,
            ),
            mini::icon_button(
                icons::search(),
                "Search (Ctrl+F)",
                Some(Message::Response(ResponseMsg::ViewerEdited(
                    iced_code_editor::Message::OpenSearch,
                ))),
                false,
            ),
            mini::icon_button(
                icons::copy(),
                "Copy body",
                Some(Message::Response(ResponseMsg::CopyBody)),
                false,
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(iced::Padding {
        top: 8.0,
        right: 16.0,
        bottom: 4.0,
        left: 16.0,
    })
    .width(Length::Fill);

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

    let body = container(body)
        .padding([0, 2])
        .width(Length::Fill)
        .height(Length::Fill);

    let panel = column![].spacing(0).height(Length::Fill);
    panel.push(toolbar).push(body).into()
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
                    .size(TEXT_XS)
                    .color(Palette::text_subtle()),
                Space::new().width(Length::Fill),
                open_action,
                save_action,
            ]
            .align_y(Alignment::Center),
            scrollable(text(rendered).size(TEXT_SM).color(Palette::text())).height(Length::Fill),
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

/// A keyboard-key chip: bordered, mono font, slight bottom "keycap" shadow.
fn kbd_chip(label: &str) -> Element<'static, Message> {
    container(
        text(label.to_owned())
            .size(TEXT_XS)
            .color(Palette::text_muted())
            .font(MONO),
    )
    .padding([3, 8])
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::surface_high())),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 5.0.into(),
        },
        shadow: iced::Shadow {
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.4,
            },
            offset: iced::Vector::new(0.0, 1.5),
            blur_radius: 0.0,
        },
        ..Default::default()
    })
    .into()
}

fn empty_view() -> Element<'static, Message> {
    let modifier = if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    };
    let content = column![
        text("Send a request")
            .size(TEXT_LG)
            .color(Palette::text())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().height(8),
        row![
            text("Press").size(TEXT_SM).color(Palette::text_subtle()),
            kbd_chip(modifier),
            kbd_chip("Enter"),
            text("or click Send to run the request")
                .size(TEXT_SM)
                .color(Palette::text_subtle()),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn error_view(err: &str) -> Element<'static, Message> {
    let content = column![
        text("Request failed")
            .size(TEXT_LG)
            .color(Palette::ERROR)
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().height(4),
        text(err.to_owned())
            .size(TEXT_SM)
            .color(Palette::text_muted()),
    ]
    .max_width(480)
    .align_x(Alignment::Center);

    container(content)
        .padding([24, 24])
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn binary_view(resp: &HttpResponse) -> Element<'static, Message> {
    let is_pdf = resp.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type") && value.to_ascii_lowercase().contains("pdf")
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

/// A simple scrollable grid preview of a parsed spreadsheet's first sheet.
fn spreadsheet_view(sheet: &ParsedSheet) -> Element<'static, Message> {
    let mut rows_col = column![].spacing(0);
    for (row_idx, cells) in sheet.rows.iter().enumerate() {
        let is_header = row_idx == 0;
        let mut row_widget = row![].spacing(0);
        for cell in cells {
            row_widget = row_widget.push(
                container(text(cell.clone()).size(TEXT_SM).color(if is_header {
                    Palette::text()
                } else {
                    Palette::text_muted()
                }))
                .padding([6, 10])
                .width(Length::Fixed(140.0))
                .style(move |_| container::Style {
                    background: if is_header {
                        Some(Background::Color(Palette::surface_high()))
                    } else {
                        None
                    },
                    border: Border {
                        color: Palette::border(),
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }),
            );
        }
        rows_col = rows_col.push(row_widget);
    }

    let toolbar = container(
        text(format!(
            "Sheet: {} · {} row(s)",
            sheet.sheet_name,
            sheet.rows.len()
        ))
        .size(TEXT_XS)
        .color(Palette::text_subtle()),
    )
    .padding([6, 12]);

    column![
        toolbar,
        scrollable(rows_col)
            .direction(scrollable::Direction::Both {
                vertical: scrollable::Scrollbar::default(),
                horizontal: scrollable::Scrollbar::default(),
            })
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

fn loading_view<'a>(spinner_frame: u32) -> Element<'a, Message> {
    let art = spinner_frame_art(spinner_frame);

    container(
        column![
            text(art)
                .size(11)
                .font(MONO)
                .line_height(iced::widget::text::LineHeight::Relative(1.0))
                .color(Palette::accent()),
            Space::new().height(14),
            text("sending request…")
                .size(13)
                .color(Palette::text_muted()),
        ]
        .spacing(0)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

/// Number of pre-rendered spinner frames that are cycled while a request runs.
///
/// `donut` is a little software raytracer: its `theta`/`phi` loops run ~52,000
/// iterations with a `sin_cos` each. It used to be recomputed from scratch on the
/// UI thread for every frame at 30fps, competing for CPU with the very request it
/// was reporting on. The animation depends only on the frame counter, so the
/// frames are rendered once and reused.
const SPINNER_FRAMES: usize = 60;

/// Returns the pre-rendered art for `spinner_frame`, building the set on first
/// use.
fn spinner_frame_art(spinner_frame: u32) -> &'static str {
    use std::sync::OnceLock;
    static FRAMES: OnceLock<Vec<String>> = OnceLock::new();

    let frames = FRAMES.get_or_init(|| {
        (0..SPINNER_FRAMES)
            .map(|i| {
                let f = i as f32;
                donut(f * 0.04, f * 0.023)
            })
            .collect()
    });

    // `SPINNER_FRAMES` is non-zero, so the modulo is always in range.
    frames[(spinner_frame as usize) % SPINNER_FRAMES].as_str()
}

fn donut(a: f32, b: f32) -> String {
    const W: usize = 72;
    const H: usize = 22;
    const K2: f32 = 5.0;
    const R1: f32 = 1.0;
    const R2: f32 = 2.0;
    let kx = W as f32 * 0.27;
    let ky = kx * 0.47;
    let chars = b".,-~:;=!*#$@";

    let mut out = vec![b' '; W * H];
    let mut zbuf = vec![0.0f32; W * H];
    let (sa, ca) = a.sin_cos();
    let (sb, cb) = b.sin_cos();

    let mut theta = 0.0f32;
    while theta < std::f32::consts::TAU {
        let (st, ct) = theta.sin_cos();
        let mut phi = 0.0f32;
        while phi < std::f32::consts::TAU {
            let (sp, cp) = phi.sin_cos();
            let cx = R2 + R1 * ct;
            let cy = R1 * st;
            let x = cx * (cb * cp + sa * sb * sp) - cy * ca * sb;
            let y = cx * (sb * cp - sa * cb * sp) + cy * ca * cb;
            let z = K2 + ca * cx * sp + cy * sa;
            let ooz = 1.0 / z;
            let xp = (W as f32 / 2.0 + kx * ooz * x) as isize;
            let yp = (H as f32 / 2.0 - ky * ooz * y) as isize;
            let lum = cp * ct * sb - ca * ct * sp - sa * st + cb * (ca * st - ct * sa * sp);
            if xp >= 0 && xp < W as isize && yp >= 0 && yp < H as isize {
                let o = xp as usize + yp as usize * W;
                if ooz > zbuf[o] {
                    zbuf[o] = ooz;
                    let li = (lum * 8.0) as isize;
                    out[o] = chars[li.clamp(0, chars.len() as isize - 1) as usize];
                }
            }
            phi += 0.015;
        }
        theta += 0.05;
    }

    let mut s = String::with_capacity(W * H + H);
    for row in 0..H {
        s.push_str(std::str::from_utf8(&out[row * W..row * W + W]).unwrap());
        if row + 1 < H {
            s.push('\n');
        }
    }
    s
}
