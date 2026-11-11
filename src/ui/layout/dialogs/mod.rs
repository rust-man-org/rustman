//! The app's dialogs on one raised shell: title row, body, right-aligned
//! footer. Esc and a click on the scrim dismiss them.

use iced::widget::{Space, Text, column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{ClashChoice, GitMsg, ImportMsg, Message, RequestMsg, SidebarMsg};
use crate::state::sidebar::Clash;
use crate::ui::components::Surface;
use crate::ui::components::button::{Size, Variant, button_on, button_with_icon};
use crate::ui::components::icon_button::{IconSize, icon_button};
use crate::ui::components::kbd::{KeyCap, kbd};
use crate::ui::components::menu::panel_style;
use crate::ui::components::{scroll, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_LG, HAIRLINE, ICON_MD, LINE_LG, LINE_MD, MONO, RADIUS_SM, SPACE_2XL, SPACE_LG, SPACE_MD,
    SPACE_XL, SPACE_XS, TEXT_MD, TEXT_SM, TEXT_XL, Tokens, UI_FONT_MEDIUM, line,
};

mod locations;

pub(super) use locations::{folder_dialog, save_dialog};

const NARROW: f32 = 420.0;
const MEDIUM: f32 = 480.0;
const WIDE: f32 = 560.0;
const COMMAND_HEIGHT: f32 = 200.0;
const OPTION_ROW: f32 = 52.0;
/// The free height above and below a dialog, 11 to 17: the mockup's Save
/// dialog sits 176 px down a 900 px window.
const ABOVE: u16 = 11;
const BELOW: u16 = 17;

/// The shared dialog shell.
struct Dialog<'a> {
    width: f32,
    title: Element<'a, Message>,
    subtitle: Option<Element<'a, Message>>,
    body: Element<'a, Message>,
    footer: Vec<Element<'a, Message>>,
    dismiss: Message,
}

impl<'a> Dialog<'a> {
    fn view(self, t: &'a Tokens) -> Element<'a, Message> {
        let close = icon_button(t, icons::close(), IconSize::Medium).on_press(self.dismiss.clone());
        let mut head = column![
            row![
                self.title,
                Space::new().width(Length::Fill),
                tooltip::tooltip(t, close, "Close", Some("Esc".to_owned()))
            ]
            .align_y(Alignment::Center)
        ];
        if let Some(subtitle) = self.subtitle {
            head = head.push(subtitle);
        }
        let mut content = column![head, Space::new().height(SPACE_XL), self.body];
        if !self.footer.is_empty() {
            content = content.push(Space::new().height(SPACE_2XL)).push(
                row![Space::new().width(Length::Fill)]
                    .extend(self.footer)
                    .spacing(SPACE_MD)
                    .align_y(Alignment::Center),
            );
        }
        let card = container(content)
            .width(self.width)
            .padding(Padding {
                top: SPACE_XL + SPACE_XS + HAIRLINE,
                right: SPACE_2XL + HAIRLINE,
                bottom: SPACE_2XL + HAIRLINE,
                left: SPACE_2XL + HAIRLINE,
            })
            .style(move |_| panel_style(t));
        crate::ui::widgets::overlay::modal_layer(card, self.dismiss, move |card| {
            container(column![
                Space::new().height(Length::FillPortion(ABOVE)),
                row![Space::new().width(Length::Fill), card, Space::new().width(Length::Fill)],
                Space::new().height(Length::FillPortion(BELOW)),
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(t.scrim)),
                ..Default::default()
            })
            .into()
        })
    }
}

fn title<'a>(t: &'a Tokens, s: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(s).size(TEXT_XL).line_height(line(LINE_LG)).font(UI_FONT_MEDIUM).color(t.text).into()
}

fn paragraph<'a>(t: &'a Tokens, s: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(s).size(TEXT_MD).line_height(line(LINE_MD)).color(t.on_raised.text_muted).into()
}

fn label<'a>(t: &'a Tokens, s: &'static str) -> Element<'a, Message> {
    container(text(s).size(TEXT_SM).font(UI_FONT_MEDIUM).color(t.on_raised.text_muted))
        .padding(Padding {
            bottom: SPACE_XS + 2.0,
            ..Padding::ZERO
        })
        .into()
}

fn cancel<'a>(t: &'a Tokens, label: &'static str, message: Message) -> Element<'a, Message> {
    button_on(t, Surface::Raised, label, Variant::Secondary, Size::Large)
        .on_press(message)
        .into()
}

/// The primary action; disabled while `message` is `None`.
fn confirm<'a>(t: &'a Tokens, label: &'static str, message: impl Into<Option<Message>>) -> Element<'a, Message> {
    button_on(t, Surface::Raised, label, Variant::Primary, Size::Large)
        .on_press_maybe(message.into())
        .into()
}

/// The confirming action of a destructive dialog.
fn danger<'a>(t: &'a Tokens, label: &'static str, message: Message) -> Element<'a, Message> {
    button_on(t, Surface::Raised, label, Variant::Danger, Size::Large).on_press(message).into()
}

fn panel<'a>(t: &'a Tokens, content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content).width(Length::Fill).style(move |_| container::Style {
        background: Some(Background::Color(t.surface)),
        border: Border {
            color: t.on_raised.border,
            width: HAIRLINE,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
}

pub(super) fn export_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let id = state.export_dialog_collection.clone().unwrap_or_default();
    let name = state.collections.iter().find(|c| c.id == id).map_or("", |c| c.name.as_str());
    let body = column![
        option(
            t,
            icons::export(),
            "Postman collection v2.1",
            "For Postman and compatible tools",
            None,
            Message::Import(ImportMsg::ExportCollection(id.clone())),
        ),
        option(
            t,
            icons::braces(),
            "Rustman JSON",
            "Lossless; imports back into Rustman",
            None,
            Message::Import(ImportMsg::ExportCollectionJson(id)),
        ),
        Space::new().height(SPACE_XL),
        crate::ui::settings::setting_row(
            t,
            "Include secrets",
            "Passwords, tokens, API keys, cookies and credential headers are left out unless this is on. {{variables}} are always kept.",
            crate::ui::settings::switch(
                t,
                state.sidebar.export_include_secrets,
                Message::Import(ImportMsg::ExportIncludeSecretsToggled),
            ),
        ),
    ];
    Dialog {
        width: MEDIUM,
        title: title(t, format!("Export “{name}”")),
        subtitle: Some(paragraph(t, "Choose a format.")),
        body: body.into(),
        footer: vec![cancel(t, "Cancel", Message::Import(ImportMsg::CloseExportDialog))],
        dismiss: Message::Import(ImportMsg::CloseExportDialog),
    }
    .view(t)
}

/// One choice in a dialog: an icon tile, a title and a line of detail.
fn option<'a>(
    t: &'a Tokens,
    icon: Text<'a>,
    title: &'static str,
    detail: &'static str,
    keys: Option<&'a str>,
    on_press: Message,
) -> Element<'a, Message> {
    let tile = container(icon.size(ICON_MD).color(t.accent_text))
        .center_x(CONTROL_LG)
        .center_y(CONTROL_LG)
        .style(move |_| container::Style {
            background: Some(Background::Color(t.surface)),
            border: Border {
                color: t.on_raised.border,
                width: HAIRLINE,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let mut content = row![
        tile,
        column![
            text(title).size(TEXT_MD).font(UI_FONT_MEDIUM).color(t.text),
            text(detail).size(TEXT_SM).color(t.on_raised.text_muted),
        ]
        .width(Length::Fill),
    ]
    .spacing(SPACE_LG)
    .align_y(Alignment::Center);
    if let Some(keys) = keys {
        content = content.push(kbd(t, KeyCap::Text(keys), Surface::Raised));
    }
    iced::widget::button(container(content).center_y(Length::Fill))
        .on_press(on_press)
        .width(Length::Fill)
        .height(OPTION_ROW)
        .padding([0.0, SPACE_MD])
        .style(move |_, status| {
            use iced::widget::button::Status;
            iced::widget::button::Style {
                background: matches!(status, Status::Hovered | Status::Pressed)
                    .then_some(Background::Color(t.on_raised.hover)),
                text_color: t.text,
                border: Border {
                    radius: RADIUS_SM.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

pub(super) fn close_confirm_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    Dialog {
        width: NARROW,
        title: title(t, "Discard unsaved changes?"),
        subtitle: None,
        body: paragraph(t, "This tab has edits that haven't been saved. Closing it loses them."),
        footer: vec![
            cancel(t, "Cancel", Message::Request(RequestMsg::CancelCloseTab)),
            danger(t, "Discard changes", Message::Request(RequestMsg::ConfirmCloseTab)),
        ],
        dismiss: Message::Request(RequestMsg::CancelCloseTab),
    }
    .view(t)
}

pub(super) fn delete_confirm_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let Some(pending) = &state.sidebar.confirm_delete else {
        return Space::new().into();
    };
    let side = Message::Sidebar;
    Dialog {
        width: NARROW,
        title: title(t, pending.question()),
        subtitle: None,
        body: paragraph(t, "This can't be undone."),
        footer: vec![
            cancel(t, "Cancel", side(SidebarMsg::DeleteCancelled)),
            danger(t, "Delete", side(SidebarMsg::DeleteConfirmed)),
        ],
        dismiss: side(SidebarMsg::DeleteCancelled),
    }
    .view(t)
}

pub(super) fn import_clash_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let Some(pending) = &state.sidebar.import_clash else {
        return Space::new().into();
    };
    let answer = |choice| Message::Import(ImportMsg::ClashResolved(choice));
    let incoming = &pending.collection.collection.name;
    let copy = button_on(t, Surface::Raised, "Import as a copy", Variant::Secondary, Size::Large).on_press(answer(ClashChoice::Copy));
    let (heading, body, mut footer): (String, String, Vec<Element<'_, Message>>) = match &pending.clash {
        Clash::Replaces { name, requests } => (
            format!("Replace '{name}'?"),
            format!(
                "'{incoming}' is a collection already here, holding {requests} request{}. Replace it, or import a copy with new ids and keep both.",
                if *requests == 1 { "" } else { "s" }
            ),
            vec![copy.into()],
        ),
        Clash::SharesIds => (
            format!("Import '{incoming}' as a copy?"),
            "Some of its folders or requests have the same ids as ones in other collections. A copy gets new ids, so nothing here is overwritten.".to_owned(),
            Vec::new(),
        ),
    };
    footer.insert(0, cancel(t, "Skip", answer(ClashChoice::Skip)));
    footer.push(match &pending.clash {
        Clash::Replaces { name, .. } => button_on(t, Surface::Raised, format!("Replace '{name}'"), Variant::Primary, Size::Large)
            .on_press(answer(ClashChoice::Replace))
            .into(),
        Clash::SharesIds => confirm(t, "Import as a copy", answer(ClashChoice::Copy)),
    });
    Dialog {
        width: WIDE,
        title: title(t, heading),
        subtitle: None,
        body: paragraph(t, body),
        footer,
        dismiss: answer(ClashChoice::Skip),
    }
    .view(t)
}

pub(super) fn restore_confirm_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let id = state.git_restore_confirm.clone().unwrap_or_default();
    let short: String = id.chars().take(7).collect();
    let heading = row![
        title(t, "Restore commit "),
        text(short).size(TEXT_XL).font(MONO).color(t.accent_text),
        title(t, "?"),
    ]
    .align_y(Alignment::Center);
    Dialog {
        width: NARROW,
        title: heading.into(),
        subtitle: None,
        body: paragraph(
            t,
            "Your collections go back to this commit. Uncommitted changes, including edits open in tabs, are \
             overwritten, and collections added since are removed.",
        ),
        footer: vec![
            cancel(t, "Cancel", Message::Git(GitMsg::CancelRestore)),
            danger(t, "Restore", Message::Git(GitMsg::RestoreCommit(id))),
        ],
        dismiss: Message::Git(GitMsg::CancelRestore),
    }
    .view(t)
}

pub(super) fn curl_modal(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let command = state.curl_modal_command.as_str();
    // A plain text view shapes everything it shows: a huge export gets a
    // capped preview, while Copy still takes the whole command.
    let head = crate::services::format::head_capped(command, crate::consts::EXPORT_PREVIEW_BYTES);
    let mut body = column![];
    if head.len() < command.len() {
        body = body.push(
            text(format!(
                "Showing {} of {}; Copy takes the full command.",
                rustman_core::text::human_size(head.len()),
                rustman_core::text::human_size(command.len()),
            ))
            .size(TEXT_SM)
            .color(t.on_raised.status.warning),
        );
        body = body.push(Space::new().height(SPACE_MD));
    }
    let shown = scroll::both(t, container(text(head).font(MONO).size(TEXT_SM).color(t.text)).padding([SPACE_MD + 2.0, SPACE_LG]))
        .width(Length::Fill)
        .height(COMMAND_HEIGHT);
    let keep = crate::ui::settings::setting_row(
        t,
        "Keep {{variables}}",
        "Share the template without your environment's values.",
        crate::ui::settings::switch(
            t,
            state.chrome.curl_keep_variables,
            Message::Request(RequestMsg::CurlKeepVariablesToggled),
        ),
    );
    body = body.push(panel(t, shown)).push(Space::new().height(SPACE_XL)).push(keep);
    let copy = button_with_icon(t, icons::copy(), "Copy", Variant::Primary, Size::Large)
        .on_press(Message::Request(RequestMsg::CopyCurlToClipboard));
    Dialog {
        width: WIDE,
        title: title(t, "Copy as cURL"),
        subtitle: None,
        body: body.into(),
        footer: vec![cancel(t, "Close", Message::Request(RequestMsg::CloseCurlModal)), copy.into()],
        dismiss: Message::Request(RequestMsg::CloseCurlModal),
    }
    .view(t)
}

pub(super) fn import_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let body: Element<'_, Message> = if state.importing {
        container(paragraph(t, "Reading the command…")).height(OPTION_ROW).center_y(OPTION_ROW).into()
    } else {
        column![
            option(
                t,
                icons::clipboard_paste(),
                "Paste from the clipboard",
                "A cURL or HTTPie command",
                Some(&IMPORT_KEY),
                Message::Request(RequestMsg::ImportFromClipboard),
            ),
            option(
                t,
                icons::curl(),
                "Choose a command file…",
                "A file holding one cURL or HTTPie command",
                None,
                Message::Request(RequestMsg::ImportFromFile),
            ),
            option(
                t,
                icons::file_input(),
                "Import a collection…",
                "Postman, OpenAPI, HTTPie or a Rustman export",
                None,
                Message::Import(ImportMsg::ImportCollectionFile),
            ),
        ]
        .into()
    };
    Dialog {
        width: MEDIUM,
        title: title(t, "Import"),
        subtitle: Some(paragraph(t, "Bring in a request from the command line, or a whole collection.")),
        body,
        footer: vec![cancel(t, "Cancel", Message::Request(RequestMsg::CloseImportDialog))],
        dismiss: Message::Request(RequestMsg::CloseImportDialog),
    }
    .view(t)
}

static IMPORT_KEY: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| crate::ui::components::tooltip::shortcut("I"));
