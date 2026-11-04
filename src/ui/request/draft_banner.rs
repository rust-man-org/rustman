//! Local draft banner above the request editor (workstream 01, C9).
//!
//! One banner per draft state, following the approved mockup
//! `agent/plan/mockup/pages/rustman-requester-draft.html`: local draft,
//! main changed, field conflicts, in review, deleted endpoint. Shows the
//! persisted draft; unsaved tab edits are captured on Ctrl+S or sync.

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, text, tooltip},
};

use crate::{
    app::AppState,
    domain::draft::{DraftField, DraftState, RequestDraft, Side},
    message::{Message, draft::DraftMsg},
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_SM},
    },
};

/// "Main changed" tone. No info token exists in the palette yet (09 owns
/// it); the PUT blue is the closest neutral-informative hue.
fn info() -> Color {
    Palette::PUT
}

pub fn view(state: &AppState) -> Option<Element<'_, Message>> {
    let tab = state.tabs.active_tab();
    let (_, id) = tab.saved_as.as_ref()?;
    let draft = state.drafts.get(id)?;
    let can_write = state.auth.can_write();
    let diff_open = state.drafts.diff_open.as_deref() == Some(id.as_str());
    let unsaved = tab.modified;

    let discard = action("Discard", Some(DraftMsg::Discard(id.clone())), false, None);
    let view_diff = action(
        if diff_open { "Hide diff" } else { "View diff" },
        Some(DraftMsg::ToggleDiff(id.clone())),
        false,
        None,
    );
    let save_tip = if !can_write {
        "Needs write access to the collection repository"
    } else if !draft.can_publish() {
        "Resolve every conflict first"
    } else {
        "Not available yet: review publishing (branch + PR) is not set up"
    };

    let changed = draft.changed_fields();
    let (tone, icon, title, detail, actions, extra): (
        Color,
        Element<Message>,
        String,
        String,
        Vec<Element<Message>>,
        Option<Element<Message>>,
    ) = match draft.state() {
        DraftState::Orphaned => (
            Palette::text_subtle(),
            icons::close().size(12).color(Palette::text_subtle()).into(),
            "This endpoint was deleted".to_owned(),
            "Your draft is kept on this machine only. Restore it as a new endpoint or discard it."
                .to_owned(),
            vec![
                discard,
                action(
                    "Restore as new",
                    can_write.then(|| DraftMsg::RestoreAsNew(id.clone())),
                    true,
                    (!can_write).then_some("Needs write access to the collection repository"),
                ),
            ],
            None,
        ),
        DraftState::Conflict(fields) => (
            Palette::ERROR,
            icons::dot(Palette::ERROR),
            format!(
                "{} field{} changed on main and in your draft",
                fields.len(),
                plural(fields.len())
            ),
            "Pick a side for each field. Save for review is blocked until all are resolved."
                .to_owned(),
            vec![
                action("Discard draft", Some(DraftMsg::Discard(id.clone())), false, None),
                action("Save for review", None, true, Some("Resolve every conflict first")),
            ],
            Some(conflict_table(draft, &fields)),
        ),
        DraftState::InReview(review) => (
            Palette::text_muted(),
            icons::git_branch().size(12).color(Palette::text_muted()).into(),
            match review.pr_number {
                Some(n) => format!("In review · PR #{n}"),
                None => "In review · branch pushed".to_owned(),
            },
            format!(
                "{} → main. Saving again updates the same review.",
                review.branch
            ),
            vec![view_diff, discard],
            None,
        ),
        DraftState::UpstreamChanged => (
            info(),
            icons::dot(info()),
            "Main changed since your draft".to_owned(),
            "Their changes were merged in; your edits are kept.".to_owned(),
            vec![
                view_diff,
                discard,
                action("Save for review", None, true, Some(save_tip)),
            ],
            None,
        ),
        DraftState::Clean => (
            Palette::accent(),
            icons::dot(Palette::accent()),
            "Local draft: only you see this".to_owned(),
            format!(
                "{} field{} differ{} from main: {}. Ctrl+S saves locally.",
                changed.len(),
                plural(changed.len()),
                if changed.len() == 1 { "s" } else { "" },
                field_list(&changed)
            ),
            vec![
                view_diff,
                discard,
                action("Save for review", None, true, Some(save_tip)),
            ],
            None,
        ),
    };

    let detail = if unsaved {
        format!("{detail} Unsaved edits in this tab are not in the draft yet.")
    } else {
        detail
    };

    let mut head = row![
        container(icon).padding([4, 0]),
        column![
            text(title)
                .size(TEXT_SM)
                .color(Palette::text())
                .font(crate::ui::theme::UI_FONT_MEDIUM),
            text(detail).size(TEXT_SM).color(Palette::text_muted()),
        ]
        .spacing(2)
        .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Alignment::Start);
    for a in actions {
        head = head.push(a);
    }

    let mut body = column![head].spacing(8);
    if let Some(extra) = extra {
        body = body.push(extra);
    }
    if diff_open && !changed.is_empty() && !matches!(draft.state(), DraftState::Conflict(_)) {
        body = body.push(diff_table(draft, &changed));
    }

    Some(
        container(
            container(body)
                .padding([8, 12])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color { a: 0.10, ..tone })),
                    border: Border {
                        color: Color { a: 0.45, ..tone },
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }),
        )
        .padding([6, 12])
        .width(Length::Fill)
        .into(),
    )
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn field_list(fields: &[DraftField]) -> String {
    const SHOWN: usize = 4;
    let mut names: Vec<&str> = fields.iter().take(SHOWN).map(|f| f.label()).collect();
    let more = fields.len().saturating_sub(SHOWN);
    let more_label = format!("+{more} more");
    if more > 0 {
        names.push(&more_label);
    }
    names.join(", ")
}

fn diff_table<'a>(draft: &RequestDraft, fields: &[DraftField]) -> Element<'a, Message> {
    let mut rows = column![table_row(
        header("Field"),
        header("main"),
        header("Your draft"),
        None
    )]
    .spacing(0);
    for f in fields {
        rows = rows.push(table_row(
            cell(f.label().to_owned(), Palette::text(), false),
            cell(f.preview(&draft.base), Palette::text_muted(), true),
            cell(f.preview(&draft.data), Palette::accent(), true),
            None,
        ));
    }
    table(rows)
}

fn conflict_table<'a>(draft: &RequestDraft, fields: &[DraftField]) -> Element<'a, Message> {
    let mut rows = column![table_row(
        header("Field"),
        header("Yours"),
        header("main"),
        Some(Space::new().width(170).into()),
    )]
    .spacing(0);
    for f in fields {
        let id = draft.request_id.clone();
        let id2 = draft.request_id.clone();
        let buttons: Element<Message> = row![
            action(
                "Keep mine",
                Some(DraftMsg::ResolveField(id, *f, Side::Mine)),
                false,
                None
            ),
            action(
                "Use main",
                Some(DraftMsg::ResolveField(id2, *f, Side::Upstream)),
                false,
                None
            ),
        ]
        .spacing(6)
        .width(170)
        .into();
        rows = rows.push(table_row(
            cell(f.label().to_owned(), Palette::text(), false),
            cell(f.preview(&draft.data), Palette::accent(), true),
            cell(f.preview(&draft.base), info(), true),
            Some(buttons),
        ));
    }
    table(rows)
}

fn table<'a>(rows: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
    container(rows)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface())),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn table_row<'a>(
    field: Element<'a, Message>,
    left: Element<'a, Message>,
    right: Element<'a, Message>,
    trailing: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut r = row![
        container(field).width(130),
        container(left).width(Length::FillPortion(1)),
        container(right).width(Length::FillPortion(1)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if let Some(t) = trailing {
        r = r.push(t);
    }
    container(r).padding([5, 10]).width(Length::Fill).into()
}

fn header<'a>(label: &'static str) -> Element<'a, Message> {
    text(label).size(TEXT_SM).color(Palette::text_subtle()).into()
}

fn cell<'a>(value: String, color: Color, mono: bool) -> Element<'a, Message> {
    let t = text(value).size(TEXT_SM).color(color);
    if mono { t.font(MONO).into() } else { t.into() }
}

/// Banner button. `msg = None` renders it disabled; `tip` explains why (or
/// what it does) on hover.
fn action<'a>(
    label: &'static str,
    msg: Option<DraftMsg>,
    primary: bool,
    tip: Option<&'static str>,
) -> Element<'a, Message> {
    let enabled = msg.is_some();
    let fg = match (primary, enabled) {
        (true, true) => Color::WHITE,
        (true, false) => Color {
            a: 0.75,
            ..Color::WHITE
        },
        (false, true) => Palette::text(),
        (false, false) => Palette::text_subtle(),
    };
    let btn = button(text(label).size(TEXT_SM).color(fg))
        .on_press_maybe(msg.map(Message::Draft))
        .padding([4, 10])
        .style(move |_t, status| {
            let hovered = matches!(status, button::Status::Hovered);
            let bg = if primary {
                let a = if enabled {
                    if hovered {
                        Palette::accent_hover()
                    } else {
                        Palette::accent()
                    }
                } else {
                    Color {
                        a: 0.45,
                        ..Palette::accent()
                    }
                };
                Some(Background::Color(a))
            } else if hovered && enabled {
                Some(Background::Color(Palette::hover()))
            } else {
                Some(Background::Color(Palette::surface()))
            };
            button::Style {
                background: bg,
                text_color: fg,
                border: Border {
                    color: if primary {
                        Color::TRANSPARENT
                    } else {
                        Palette::border()
                    },
                    width: 1.0,
                    radius: 5.0.into(),
                },
                ..Default::default()
            }
        });
    match tip {
        Some(tip) => tooltip(
            btn,
            container(text(tip).size(TEXT_SM).color(Palette::text()))
                .padding([4, 8])
                .style(|_| container::Style {
                    background: Some(Background::Color(Palette::surface_raised())),
                    border: Border {
                        color: Palette::border(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
            tooltip::Position::Bottom,
        )
        .into(),
        None => btn.into(),
    }
}
