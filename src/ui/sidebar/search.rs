//! Sidebar search popover (user-provided fuzzy search reference).
use crate::{
    app::AppState,
    message::{Message, SidebarMsg},
    services::search::Kind,
    ui::{
        components::{
            atoms::text as t,
            molecules::{
                menu_divider,
                search_result::{search_panel, search_result},
            },
        },
        icons,
        theme::{MONO, Palette},
    },
};
use iced::{
    Background, Border, Element, Length,
    widget::{column, container, rich_text, row, scrollable, span, text},
};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let hits = &state.sidebar.search_hits;
    let header = row![
        text(format!("Results for \"{}\"", state.sidebar.filter))
            .size(12)
            .color(Palette::text())
            .width(Length::Fill),
        text(format!("{} results · fuzzy", hits.len()))
            .font(MONO)
            .size(11)
            .color(Palette::text_muted())
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);
    let mut list = column![].spacing(0);
    let mut previous = None;
    for (index, hit) in hits.iter().enumerate() {
        if previous != Some(hit.kind) {
            if previous.is_some() {
                list = list.push(menu_divider());
            }
            let label = match hit.kind {
                Kind::Endpoint => "ENDPOINTS",
                Kind::Folder => "FOLDERS",
                Kind::BodyScript => "BODY & SCRIPTS",
            };
            let count = hits.iter().filter(|h| h.kind == hit.kind).count();
            list = list.push(
                container(
                    text(format!("{label}  {count}"))
                        .size(11)
                        .color(Palette::text_muted()),
                )
                .padding([7, 10]),
            );
            previous = Some(hit.kind);
        }
        let leading: Element<Message> = if hit.kind == Kind::Folder {
            container(icons::folder().size(14).color(Palette::text_muted()))
                .center_x(56)
                .padding([2, 0])
                .into()
        } else {
            let color = super::drafts::method_color(&hit.method);
            container(text(hit.method.clone()).size(10).font(MONO).color(color))
                .center_x(56)
                .padding([3, 0])
                .style(move |_| container::Style {
                    background: Some(Background::Color(iced::Color { a: 0.10, ..color })),
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        };
        let kind = match hit.kind {
            Kind::Endpoint => "endpoint",
            Kind::Folder => "folder",
            Kind::BodyScript => "content",
        };
        list = list.push(search_result(
            leading,
            highlighted(&hit.name, &state.sidebar.filter),
            hit.path.clone(),
            hit.detail.clone(),
            kind,
            index == state.sidebar.search_selected,
            Message::Sidebar(SidebarMsg::SearchPick(index)),
        ));
    }
    if hits.is_empty() {
        list = list.push(
            container(
                column![
                    t::body("No matching results"),
                    t::muted("Try an endpoint name, folder, URL or script term.")
                ]
                .spacing(8),
            )
            .padding([24, 12]),
        );
    }
    let sections = hits
        .iter()
        .map(|h| h.kind)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let height = if hits.is_empty() {
        92.0
    } else {
        (hits.len() as f32 * 80.0 + sections as f32 * 30.0).min(420.0)
    };
    let results = scrollable(list)
        .id("sidebar-search-results")
        .height(height)
        .style(crate::ui::theme::thin_scrollbar);
    let footer = row![
        text("↑ ↓  navigate    Enter  open    Esc  close")
            .size(11)
            .color(Palette::text_muted()),
        text("Folders · endpoints · body · scripts")
            .size(10)
            .color(Palette::text_muted())
            .width(Length::Fill)
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);
    search_panel(header.into(), results.into(), footer.into())
}

/// Highlight whole matching words without byte-index assumptions for Unicode.
fn highlighted<'a>(name: &str, query: &str) -> Element<'a, Message> {
    let chars: Vec<_> = name.chars().collect();
    let mut marked = vec![false; chars.len()];
    for word in query.split_whitespace().take(12) {
        let n = word.chars().count();
        if n == 0 || n > chars.len() {
            continue;
        }
        for i in 0..=chars.len() - n {
            if chars[i..i + n].iter().collect::<String>().to_lowercase() == word.to_lowercase() {
                marked[i..i + n].fill(true);
            }
        }
    }
    let mut spans: Vec<iced::widget::text::Span<'a, ()>> = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mark = marked[start];
        let mut end = start + 1;
        while end < chars.len() && marked[end] == mark {
            end += 1;
        }
        spans.push(
            span(chars[start..end].iter().collect::<String>()).color(if mark {
                Palette::accent()
            } else {
                Palette::text()
            }),
        );
        start = end;
    }
    rich_text(spans)
        .size(13)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None)
        .into()
}
