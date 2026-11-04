//! Banner molecule: a tinted, bordered notice with a mark, title, detail
//! line, trailing actions and optional content below.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Column, column, container, row},
};

use crate::ui::{
    components::{
        Tone,
        atoms::{Mark, text},
    },
    theme::Palette,
};

/// Builder:
/// `Banner::new(Tone::Danger, Mark::Dot, "Title").detail("…").action(btn).body(table)`.
pub struct Banner<'a, M> {
    tone: Tone,
    mark: Mark,
    title: String,
    detail: Option<String>,
    actions: Vec<Element<'a, M>>,
    body: Vec<Element<'a, M>>,
}

impl<'a, M: 'a> Banner<'a, M> {
    pub fn new(tone: Tone, mark: Mark, title: impl Into<String>) -> Self {
        Self {
            tone,
            mark,
            title: title.into(),
            detail: None,
            actions: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Trailing action, right-aligned, in insertion order.
    pub fn action(mut self, action: impl Into<Element<'a, M>>) -> Self {
        self.actions.push(action.into());
        self
    }

    /// Content under the heading row (tables, lists).
    pub fn body(mut self, content: impl Into<Element<'a, M>>) -> Self {
        self.body.push(content.into());
        self
    }
}

impl<'a, M: 'a> From<Banner<'a, M>> for Element<'a, M> {
    fn from(b: Banner<'a, M>) -> Self {
        let mut heading: Column<'a, M> = column![text::title(b.title)].spacing(2);
        if let Some(detail) = b.detail {
            heading = heading.push(text::muted(detail));
        }
        let mut head = row![
            container(b.mark.view(b.tone, 12.0)).padding([4, 0]),
            heading.width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Start);
        for a in b.actions {
            head = head.push(a);
        }

        let mut content = column![head].spacing(8);
        for part in b.body {
            content = content.push(part);
        }

        let tone = b.tone;
        container(
            container(content)
                .padding([8, 12])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(tone.tint(0.10))),
                    border: Border {
                        color: tone.tint(0.45),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }),
        )
        .padding([6, 12])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface())),
            ..Default::default()
        })
        .into()
    }
}
