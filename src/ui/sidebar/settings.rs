use iced::{
    Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, svg, text, text_input},
};

const LOGO_SVG: &[u8] = include_bytes!("../../../public/rustman-logo.svg");

#[cfg(feature = "self-update")]
use crate::{app::UpdateState, message::UpdateMsg};
use crate::{
    message::{AppMsg, LayoutMsg, Message, SettingsMsg},
    ui::theme::{Palette, TEXT_SM, THEMES, ThemeSpec},
    ui::tree_label::{tree_label, tree_label_styled},
};

pub fn view(state: &crate::app::AppState) -> Element<'_, Message> {
    let header = build_header();
    let profile = card(build_about(state.sidebar_width));
    let git_identity = card(build_git_identity(
        &state.git_user_name,
        &state.git_user_email,
    ));
    let appearance = card(build_appearance(state.theme_idx));
    let layout = card(build_layout(
        state.horizontal_layout,
        state.ui_scale,
        state.sidebar_width,
    ));
    let request_defaults = card(build_request_defaults(&state.default_timeout_text));
    let tls = card(build_tls(state.tls_options));
    let global_scripts = card(build_global_scripts(
        state.global_pre_request_editor.text().len(),
        state.global_test_editor.text().len(),
        state.sidebar_width,
    ));
    #[cfg(feature = "self-update")]
    let updates = card(build_updates(
        &state.update,
        state.sidebar_width,
        state.auto_check_updates,
    ));
    let shortcuts = card(build_shortcuts(state.sidebar_width));
    let footer = build_footer(state.creator_unlocked, state.sidebar_width);

    let mut sections = column![
        header,
        profile,
        git_identity,
        appearance,
        layout,
        request_defaults,
        tls,
        global_scripts,
    ];
    // The Updates card only exists in builds with the in-app updater; in a
    // package-manager build (`--no-default-features`) it is omitted entirely.
    #[cfg(feature = "self-update")]
    {
        sections = sections.push(updates);
    }
    sections = sections.push(shortcuts).push(footer);

    scrollable(sections.spacing(6).padding(iced::Padding {
        top: 0.0,
        right: 8.0,
        bottom: 8.0,
        left: 8.0,
    }))
    .height(Length::Fill)
    .style(crate::ui::theme::hidden_scrollbar)
    .into()
}

fn build_git_identity(name: &str, email: &str) -> Element<'static, Message> {
    column![
        section_label("GIT IDENTITY"),
        field_label("Name"),
        text_input("Your Name", name)
            .on_input(|v| Message::Settings(SettingsMsg::GitNameChanged(v)))
            .size(12)
            .padding([6, 10])
            .style(crate::ui::styles::field_input),
        Space::new().height(6),
        field_label("Email"),
        text_input("you@example.com", email)
            .on_input(|v| Message::Settings(SettingsMsg::GitEmailChanged(v)))
            .size(12)
            .padding([6, 10])
            .style(crate::ui::styles::field_input),
        Space::new().height(4),
        text("Used for git commits. Saves to the active repo's config.")
            .size(9)
            .color(Palette::text_subtle()),
    ]
    .spacing(0)
    .into()
}

fn build_request_defaults(timeout_text: &str) -> Element<'static, Message> {
    column![
        section_label("REQUEST DEFAULTS"),
        field_label("Timeout (ms)"),
        text_input("30000", timeout_text)
            .on_input(|v| Message::Settings(SettingsMsg::DefaultTimeoutChanged(v)))
            .size(12)
            .padding([6, 10])
            .width(Length::Fill)
            .style(crate::ui::styles::field_input),
        Space::new().height(4),
        text("Applied to every request. 0 falls back to 30000.")
            .size(9)
            .color(Palette::text_subtle()),
    ]
    .spacing(0)
    .into()
}

/// TLS / connection workarounds for endpoints a default client cannot reach
/// (issue #40: works in Chrome/curl, fails in Rustman with `os error 10054`).
fn build_tls(options: crate::services::http::TlsOptions) -> Element<'static, Message> {
    use crate::message::TlsOption;

    column![
        section_label("TLS & CONNECTION"),
        tls_toggle(
            "Send HTTP/1.1 only",
            "Try this first. Rustman offers HTTP/2 in the handshake; some servers              close the connection instead of declining it (Windows: os error 10054).",
            options.http1_only,
            TlsOption::Http1Only,
        ),
        Space::new().height(6),
        tls_toggle(
            "Ignore certificate errors",
            "Accepts self-signed, internal-CA and mismatched-hostname certificates.              Like curl -k — only use it for endpoints you trust.",
            options.accept_invalid_certs,
            TlsOption::AcceptInvalidCerts,
        ),
        Space::new().height(6),
        tls_toggle(
            "Force TLS 1.2",
            "Pin the handshake to TLS 1.2, for servers that fail on 1.3.",
            options.force_tls12,
            TlsOption::ForceTls12,
        ),
        Space::new().height(6),
        tls_toggle(
            "Force TLS 1.3",
            "Pin the handshake to TLS 1.3. Clears the 1.2 pin.",
            options.force_tls13,
            TlsOption::ForceTls13,
        ),
        Space::new().height(4),
        text("Applies to the next request; existing connections are dropped.")
            .size(9)
            .color(Palette::text_subtle()),
    ]
    .spacing(0)
    .into()
}

fn tls_toggle(
    label: &'static str,
    help: &'static str,
    enabled: bool,
    option: crate::message::TlsOption,
) -> Element<'static, Message> {
    column![
        iced::widget::checkbox(enabled)
            .label(label)
            .on_toggle(move |_| Message::Settings(SettingsMsg::TlsOptionToggled(option)))
            .size(15)
            .text_size(12)
            .style(crate::ui::styles::checkbox),
        text(help).size(9).color(Palette::text_subtle()),
    ]
    .spacing(2)
    .into()
}

fn build_global_scripts(
    pre_request_len: usize,
    test_len: usize,
    content_width: f32,
) -> Element<'static, Message> {
    let status = match (pre_request_len > 0, test_len > 0) {
        (false, false) => "Not set up yet".to_owned(),
        (true, false) => "Pre-request configured".to_owned(),
        (false, true) => "Test script configured".to_owned(),
        (true, true) => "All configured".to_owned(),
    };

    column![
        section_label("GLOBAL SCRIPTS"),
        text("Runs before every request's own pre-request/test script — for setup you'd otherwise copy-paste into each request.")
            .size(9).color(Palette::text_subtle()),
        Space::new().height(8),
        row![
            tree_label(status, 11.0, content_width - 100.0),
            Space::new().width(Length::Fill),
            button(text("Edit").size(11).color(Palette::text()))
                .on_press(Message::Settings(SettingsMsg::OpenGlobalScriptsModal))
                .style(|_t, s| {
                    let hov = matches!(s, iced::widget::button::Status::Hovered);
                    iced::widget::button::Style {
                        background: Some(Background::Color(if hov {
                            Palette::surface_raised()
                        } else {
                            Palette::surface_high()
                        })),
                        border: Border { color: Palette::border(), width: 1.0, radius: 6.0.into() },
                        text_color: Palette::text(),
                        ..Default::default()
                    }
                })
                .padding([5, 12]),
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(0)
    .into()
}

// ── Sections ──────────────────────────────────────────────────────────────────

fn build_header() -> Element<'static, Message> {
    let logo_handle = svg::Handle::from_memory(LOGO_SVG);
    container(
        row![
            svg(logo_handle).width(36).height(36),
            column![
                row![
                    text("Rustman")
                        .size(13)
                        .color(Palette::text())
                        .font(crate::ui::theme::MONO),
                    Space::new().width(5),
                    container(
                        text(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .size(9)
                            .color(Palette::text_muted())
                            .font(crate::ui::theme::MONO)
                    )
                    .style(|_| iced::widget::container::Style {
                        background: Some(Background::Color(Palette::surface_raised())),
                        border: Border {
                            color: Palette::border(),
                            width: 1.0,
                            radius: 4.0.into()
                        },
                        ..Default::default()
                    })
                    .padding([2, 5]),
                ]
                .align_y(iced::Alignment::Center),
                text("Native API Testing Client")
                    .size(9)
                    .color(Palette::text_subtle()),
            ]
            .spacing(2),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding(iced::Padding {
        top: 12.0,
        right: 12.0,
        bottom: 6.0,
        left: 12.0,
    })
    .into()
}

/// Compact project card: links to the org repo + issue tracker. Replaces
/// the old personal-profile card — no avatar download, no PII in state.
fn build_about(content_width: f32) -> Element<'static, Message> {
    let accent = Palette::accent();
    column![
        section_label("RUSTMAN"),
        full_link_row(
            "GitHub",
            "rust-man-org/rustman".to_owned(),
            "https://github.com/rust-man-org/rustman".to_owned(),
            accent,
            content_width,
        ),
        full_link_row(
            "Issues",
            "Report a bug".to_owned(),
            "https://github.com/rust-man-org/rustman/issues".to_owned(),
            accent,
            content_width,
        ),
    ]
    .spacing(6)
    .into()
}

fn build_appearance(theme_idx: usize) -> Element<'static, Message> {
    let mut cards = column![].spacing(4);
    for (i, spec) in THEMES.iter().enumerate() {
        let is_selected = i == theme_idx;
        cards = cards.push(theme_card(spec, is_selected, i));
    }

    column![section_label("APPEARANCE"), field_label("Theme"), cards,]
        .spacing(6)
        .into()
}

fn theme_card(
    spec: &'static ThemeSpec,
    is_selected: bool,
    idx: usize,
) -> Element<'static, Message> {
    let preview = row![
        swatch(spec.background),
        swatch(spec.surface),
        swatch(spec.accent)
    ]
    .spacing(3);

    let check: Element<'static, Message> = if is_selected {
        text("\u{2713}").size(TEXT_SM).color(spec.accent).into()
    } else {
        Space::new().width(0).into()
    };

    button(
        row![
            preview,
            text(spec.name).size(TEXT_SM).color(Palette::text()),
            Space::new().width(Length::Fill),
            check,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .on_press(Message::Settings(SettingsMsg::ThemeChanged(idx)))
    .style(move |_t, s| {
        let hovered = matches!(s, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: Some(Background::Color(if is_selected {
                Palette::accent_soft()
            } else if hovered {
                Palette::hover()
            } else {
                Palette::surface_high()
            })),
            border: Border {
                color: if is_selected {
                    Palette::accent()
                } else {
                    Palette::border_subtle()
                },
                width: 1.0,
                radius: 6.0.into(),
            },
            text_color: Palette::text(),
            ..Default::default()
        }
    })
    .padding([6, 10])
    .width(Length::Fill)
    .into()
}

fn swatch(color: Color) -> Element<'static, Message> {
    container(Space::new())
        .width(14)
        .height(14)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                color: Color { a: 0.3, ..color },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn build_layout(
    horizontal: bool,
    ui_scale: f64,
    content_width: f32,
) -> Element<'static, Message> {
    let options = [("Top / Bottom", false), ("Left / Right", true)];

    let mut buttons = Vec::new();
    for (label, is_horizontal) in options {
        let selected = horizontal == is_horizontal;
        buttons.push(
            container(
                button(text(label).size(TEXT_SM).color(if selected {
                    Palette::accent()
                } else {
                    Palette::text_muted()
                }))
                .on_press(Message::Settings(SettingsMsg::LayoutDirectionToggled))
                .style(move |_t, s| {
                    use iced::widget::button::Status;
                    iced::widget::button::Style {
                        background: Some(Background::Color(if selected {
                            Palette::accent_soft()
                        } else if matches!(s, Status::Hovered) {
                            Palette::hover()
                        } else {
                            Palette::surface_high()
                        })),
                        border: Border {
                            color: if selected {
                                Palette::accent()
                            } else {
                                Palette::border_subtle()
                            },
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        text_color: if selected {
                            Palette::accent()
                        } else {
                            Palette::text_muted()
                        },
                        ..Default::default()
                    }
                })
                .padding([6, 10])
                .width(Length::Fill),
            )
            .width(Length::Fill),
        );
    }
    // Side-by-side needs ~95px per button ("Left / Right" ≈ 72px text +
    // padding); below that the labels wrap inside the buttons and the row
    // grows ragged. Stack vertically instead.
    let mut col = column![section_label("LAYOUT"), field_label("Panels")].spacing(6);
    if content_width >= 230.0 {
        let mut row_el = row![].spacing(6);
        for b in buttons {
            row_el = row_el.push(b);
        }
        col = col.push(row_el);
    } else {
        for b in buttons {
            col = col.push(b);
        }
    }

    col = col.push(Space::new().height(4));
    col = col.push(field_label("UI Scale"));

    let pct = (ui_scale * 100.0).round() as i64;
    let pct_label = format!("{pct}%");
    let is_default = (ui_scale - 1.0).abs() < 0.01;

    let btn_out = button(
        text("−")
            .size(TEXT_SM)
            .color(Palette::text())
            .align_x(iced::alignment::Horizontal::Center),
    )
    .on_press(Message::Layout(LayoutMsg::ZoomOut))
    .style(|_t, s| {
        use iced::widget::button::Status;
        iced::widget::button::Style {
            background: Some(Background::Color(if matches!(s, Status::Hovered) {
                Palette::hover()
            } else {
                Palette::surface_high()
            })),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    })
    .padding([4, 12]);

    let btn_reset = button(
        text(pct_label)
            .size(TEXT_SM)
            .color(if is_default {
                Palette::text()
            } else {
                Palette::accent()
            })
            .align_x(iced::alignment::Horizontal::Center),
    )
    .on_press(Message::Layout(LayoutMsg::ZoomReset))
    .style(move |_t, s| {
        use iced::widget::button::Status;
        iced::widget::button::Style {
            background: Some(Background::Color(if !is_default {
                Palette::accent_soft()
            } else if matches!(s, Status::Hovered) {
                Palette::hover()
            } else {
                Palette::surface_high()
            })),
            border: Border {
                color: if !is_default {
                    Palette::accent()
                } else {
                    Palette::border_subtle()
                },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    })
    .padding([4, 8])
    .width(Length::Fill);

    let btn_in = button(
        text("+")
            .size(TEXT_SM)
            .color(Palette::text())
            .align_x(iced::alignment::Horizontal::Center),
    )
    .on_press(Message::Layout(LayoutMsg::ZoomIn))
    .style(|_t, s| {
        use iced::widget::button::Status;
        iced::widget::button::Style {
            background: Some(Background::Color(if matches!(s, Status::Hovered) {
                Palette::hover()
            } else {
                Palette::surface_high()
            })),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    })
    .padding([4, 12]);

    let zoom_row = row![btn_out, btn_reset, btn_in]
        .spacing(6)
        .align_y(iced::Alignment::Center);

    col = col.push(zoom_row);

    let presets = [0.8, 1.0, 1.25, 1.5];
    let mut preset_row = row![].spacing(4);
    for preset in presets {
        let is_selected = (ui_scale - preset).abs() < 0.01;
        let p_label = format!("{:.0}%", preset * 100.0);
        let btn = button(
            text(p_label)
                .size(10)
                .font(crate::ui::theme::MONO)
                .color(if is_selected {
                    Palette::accent()
                } else {
                    Palette::text_muted()
                })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .on_press(Message::Layout(LayoutMsg::SetZoom(preset)))
        .style(move |_t, s| {
            use iced::widget::button::Status;
            iced::widget::button::Style {
                background: Some(Background::Color(if is_selected {
                    Palette::accent_soft()
                } else if matches!(s, Status::Hovered) {
                    Palette::hover()
                } else {
                    Palette::surface_high()
                })),
                border: Border {
                    color: if is_selected {
                        Palette::accent()
                    } else {
                        Palette::border_subtle()
                    },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .padding([3, 4])
        .width(Length::Fill);

        preset_row = preset_row.push(btn);
    }
    col = col.push(preset_row);

    col.into()
}

#[cfg(feature = "self-update")]
fn build_updates(
    update: &UpdateState,
    content_width: f32,
    auto_check: bool,
) -> Element<'static, Message> {
    let (status, status_color) = match update {
        UpdateState::Idle if !auto_check => (
            "Automatic checks off — use the button below".to_owned(),
            Palette::text_subtle(),
        ),
        UpdateState::Idle => (
            "Checked automatically on launch".to_owned(),
            Palette::text_subtle(),
        ),
        UpdateState::Checking => ("Checking…".to_owned(), Palette::text_muted()),
        UpdateState::Available(info) => (format!("v{} available", info.version), Palette::accent()),
        UpdateState::Installing => ("Downloading update…".to_owned(), Palette::text_muted()),
        UpdateState::Ready(v) => (
            format!("v{v} installed — restart to apply"),
            Palette::SUCCESS,
        ),
        UpdateState::UpToDate => ("You're on the latest version".to_owned(), Palette::SUCCESS),
        UpdateState::Failed(e) => (format!("Check failed: {e}"), Palette::ERROR),
    };
    let busy = matches!(update, UpdateState::Checking | UpdateState::Installing);

    let mut check = button(text("Check for updates").size(11).color(Palette::text()))
        .style(|_t, status| {
            let hov = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: Some(Background::Color(if hov {
                    Palette::surface_raised()
                } else {
                    Palette::surface_high()
                })),
                border: Border {
                    color: Palette::border(),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: Palette::text(),
                ..Default::default()
            }
        })
        .padding([6, 10]);
    if !busy {
        check = check.on_press(Message::Update(UpdateMsg::Check));
    }

    // Update errors carry network/HTTP details and can be long — truncate
    // with a hover tooltip instead of wrapping the card taller.
    column![
        section_label("UPDATES"),
        iced::widget::checkbox(auto_check)
            .label("Check for updates on launch")
            .on_toggle(move |_| Message::Settings(SettingsMsg::AutoCheckUpdatesToggled))
            .size(15)
            .text_size(11)
            .style(crate::ui::styles::checkbox),
        tree_label_styled(status, 11.0, content_width - 36.0, None, status_color),
        check,
    ]
    .spacing(6)
    .into()
}

fn build_shortcuts(content_width: f32) -> Element<'static, Message> {
    let cmd = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    };
    let alt = if cfg!(target_os = "macos") {
        "⌥"
    } else {
        "Alt+"
    };
    let pairs: Vec<(String, &str)> = vec![
        (format!("{cmd}T"), "New tab"),
        (format!("{cmd}W"), "Close tab"),
        (format!("{cmd}S"), "Save"),
        (format!("{cmd}P"), "Palette"),
        (format!("{cmd}F"), "Find"),
        (format!("{cmd}H"), "Find & Replace"),
        (format!("{cmd}E"), "Export cURL"),
        (format!("{cmd}Enter"), "Send"),
        (format!("{alt}1-9"), "Switch tab"),
        (format!("{cmd}+/-"), "Zoom in/out"),
        (format!("{cmd}0"), "Reset zoom"),
        ("Esc".to_owned(), "Close dialog"),
        ("↑/↓".to_owned(), "Navigate"),
    ];

    // Two columns need ~150px per half (key chip + description); below that
    // the halves squeeze and descriptions wrap, stretching rows vertically.
    // Stack single-file instead — always fits, no truncation needed.
    let per_row = if content_width >= 330.0 { 2 } else { 1 };
    // Description budget: cell width minus the key chip (~70px worst case)
    // and spacing. Truncation here is graceful (ellipsis + hover), so the
    // constants only need to be close, not exact.
    let desc_avail = if per_row == 2 {
        (content_width - 36.0) / 2.0 - 75.0
    } else {
        content_width - 107.0
    };

    let mut grid = column![section_label("KEYBOARD SHORTCUTS")].spacing(4);
    for chunk in pairs.chunks(per_row) {
        let mut r = row![].spacing(4);
        for (key, desc) in chunk {
            r = r.push(
                container(
                    row![
                        container(
                            text(key.clone())
                                .size(10)
                                .color(Palette::text())
                                .font(crate::ui::theme::MONO),
                        )
                        .style(|_| iced::widget::container::Style {
                            background: Some(Background::Color(Palette::surface_raised())),
                            border: Border {
                                color: Palette::border(),
                                width: 1.0,
                                radius: 4.0.into()
                            },
                            ..Default::default()
                        })
                        .padding([2, 5]),
                        tree_label((*desc).to_owned(), 10.0, desc_avail),
                    ]
                    .spacing(5)
                    .align_y(iced::Alignment::Center),
                )
                .width(Length::FillPortion(1))
                .padding([1, 0]),
            );
        }
        grid = grid.push(r);
    }
    grid.into()
}

fn build_footer(creator_unlocked: bool, content_width: f32) -> Element<'static, Message> {
    let accent = Palette::accent();
    // The footer text is a button, but styled to look like plain text: three
    // taps unlock the hidden creator card (see SettingsMsg::CreatorTapped).
    let mut col = column![
        button(
            text("Rustman · purely in Rust + iced")
                .size(9)
                .color(Palette::text_subtle()),
        )
        .on_press(Message::Settings(SettingsMsg::CreatorTapped))
        .style(|_t, _s| iced::widget::button::Style {
            background: None,
            ..Default::default()
        })
        .padding(0),
    ]
    .spacing(6);
    if creator_unlocked {
        col = col.push(
            container(
                column![
                    text("Built by Animesh — open to work & freelance gigs.")
                        .size(10)
                        .color(Palette::text()),
                    full_link_row(
                        "GitHub",
                        "animeshchaudhri".to_owned(),
                        "https://github.com/animeshchaudhri".to_owned(),
                        accent,
                        content_width,
                    ),
                    full_link_row(
                        "Email",
                        "ac04@duck.com".to_owned(),
                        "mailto:ac04@duck.com".to_owned(),
                        accent,
                        content_width,
                    ),
                    full_link_row(
                        "Site",
                        "animesh.us".to_owned(),
                        "https://animesh.us".to_owned(),
                        accent,
                        content_width,
                    ),
                ]
                .spacing(4),
            )
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Palette::surface_high())),
                border: Border {
                    color: Palette::border_subtle(),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .padding(10)
            .width(Length::Fill),
        );
    }
    container(col)
        .padding(iced::Padding {
            top: 4.0,
            right: 12.0,
            bottom: 8.0,
            left: 12.0,
        })
        .into()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn card(content: impl Into<Element<'static, Message>>) -> Element<'static, Message> {
    card_ref(content)
}

/// Like `card`, but for content that borrows from `AppState` (e.g. a code
/// editor's own `view()`) instead of owning everything it displays.
fn card_ref<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let bg = Palette::surface();
    let bd = Palette::border_subtle();
    container(content)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: bd,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .padding(12)
        .width(Length::Fill)
        .into()
}

fn section_label(label: &'static str) -> Element<'static, Message> {
    container(
        text(label)
            .size(9)
            .color(Palette::text_subtle())
            .font(crate::ui::theme::MONO),
    )
    .padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 2.0,
        left: 0.0,
    })
    .into()
}

fn field_label(label: &'static str) -> Element<'static, Message> {
    text(label).size(10).color(Palette::text_subtle()).into()
}

/// A full-width row: label chip + clickable link text that truncates with a
/// hover tooltip instead of overflowing. The button is Fill so the label
/// inside clips at the row edge rather than sizing the row to its content.
fn full_link_row(
    label: &'static str,
    display: String,
    url: String,
    accent: Color,
    content_width: f32,
) -> Element<'static, Message> {
    let accent_dim = Color {
        r: accent.r * 0.62,
        g: accent.g * 0.62,
        b: accent.b * 0.63,
        a: 1.0,
    };
    let chip_bg = Palette::surface_high();
    let chip_bd = Palette::border_subtle();
    row![
        container(
            text(label)
                .size(9)
                .color(Palette::text_subtle())
                .font(crate::ui::theme::MONO)
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(chip_bg)),
            border: Border {
                color: chip_bd,
                width: 1.0,
                radius: 3.0.into()
            },
            ..Default::default()
        })
        .padding([2, 6]),
        button(tree_label_styled(
            display,
            11.0,
            content_width - 100.0,
            None,
            accent
        ))
        .on_press(Message::App(AppMsg::OpenUrl(url)))
        .style(move |_t, s| {
            let hov = matches!(s, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: if hov {
                    Some(Background::Color(accent_dim))
                } else {
                    None
                },
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 3.0.into(),
                },
                text_color: accent,
                ..Default::default()
            }
        })
        .padding([1, 4])
        .width(Length::Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}
