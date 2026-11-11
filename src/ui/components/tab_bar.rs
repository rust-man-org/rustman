//! The 32 px underline tab bar used by each pane: tabs on the left, an
//! optional trailing area (status, actions, a body-type menu) on the right,
//! a hairline along the bottom that the active tab's 2 px accent covers.

use iced::widget::{Space, button, container, row, stack, text};
use iced::{Alignment, Background, Color, Element, Length, Padding};

use crate::ui::components::{hairline, progress, status_dot};
use crate::ui::theme::{
    SPACE_LG, SPACE_MD, SPACE_XL, TAB_BAR_HEIGHT, TAB_INDICATOR, TEXT_CAPTION, TEXT_SM, Tokens,
};

pub struct Tab<'a, M> {
    label: &'a str,
    count: Option<usize>,
    dot: Option<Color>,
    active: bool,
    on_press: Option<M>,
}

impl<'a, M> Tab<'a, M> {
    pub fn new(label: &'a str, on_press: M) -> Self {
        Tab {
            label,
            count: None,
            dot: None,
            active: false,
            on_press: Some(on_press),
        }
    }

    /// A caption in tab position that can't be selected ("Response").
    pub fn caption(label: &'a str) -> Self {
        Tab {
            label,
            count: None,
            dot: None,
            active: false,
            on_press: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// A muted count after the label; zero shows nothing.
    pub fn count(mut self, n: usize) -> Self {
        self.count = (n > 0).then_some(n);
        self
    }

    /// A status dot before the label (passed / failed tests).
    pub fn dot(mut self, color: Option<Color>) -> Self {
        self.dot = color;
        self
    }
}

pub struct TabBar<'a, M> {
    tokens: &'a Tokens,
    tabs: Vec<Tab<'a, M>>,
    trailing: Option<Element<'a, M>>,
    progress: Option<f32>,
}

impl<'a, M: Clone + 'a> TabBar<'a, M> {
    pub fn new(tokens: &'a Tokens) -> Self {
        TabBar {
            tokens,
            tabs: Vec::new(),
            trailing: None,
            progress: None,
        }
    }

    pub fn tab(mut self, tab: Tab<'a, M>) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn trailing(mut self, content: impl Into<Element<'a, M>>) -> Self {
        self.trailing = Some(content.into());
        self
    }

    /// Show the indeterminate loading bar along the bottom at `phase`.
    pub fn progress(mut self, phase: Option<f32>) -> Self {
        self.progress = phase;
        self
    }

    fn view(self) -> Element<'a, M> {
        let t = self.tokens;
        let mut items = row![]
            .spacing(SPACE_XL)
            .align_y(Alignment::Center)
            .height(TAB_BAR_HEIGHT)
            .padding(Padding {
                left: SPACE_LG,
                right: SPACE_MD,
                ..Padding::ZERO
            });
        for tab in self.tabs {
            items = items.push(tab_view(t, tab));
        }
        items = items.push(Space::new().width(Length::Fill));
        if let Some(trailing) = self.trailing {
            items = items.push(trailing);
        }
        let bottom = |content: Element<'a, M>| {
            container(content)
                .height(TAB_BAR_HEIGHT)
                .width(Length::Fill)
                .align_y(Alignment::End)
        };
        let mut layers = stack![bottom(hairline(t)), items];
        if let Some(phase) = self.progress {
            layers = layers.push(bottom(progress::indeterminate(t, phase).into()));
        }
        layers.width(Length::Fill).into()
    }
}

impl<'a, M: Clone + 'a> From<TabBar<'a, M>> for Element<'a, M> {
    fn from(bar: TabBar<'a, M>) -> Self {
        bar.view()
    }
}

fn tab_view<'a, M: Clone + 'a>(t: &'a Tokens, tab: Tab<'a, M>) -> Element<'a, M> {
    let mut label = row![].spacing(SPACE_MD - 2.0).align_y(Alignment::Center);
    if let Some(color) = tab.dot {
        label = label.push(status_dot::dot(color));
    }
    label = label.push(text(tab.label).size(TEXT_SM));
    if let Some(n) = tab.count {
        label = label.push(text(crate::ui::format::count(n)).size(TEXT_CAPTION).color(t.text_muted));
    }
    let active = tab.active;
    let indicator = container(Space::new().height(TAB_INDICATOR).width(Length::Fill)).style(move |_| {
        container::Style {
            background: active.then_some(Background::Color(t.accent)),
            ..Default::default()
        }
    });
    let content = stack![
        container(label).height(Length::Fill).center_y(Length::Fill),
        container(indicator).height(Length::Fill).align_y(Alignment::End),
    ];
    button(content)
        .height(TAB_BAR_HEIGHT)
        .padding(0)
        .on_press_maybe(tab.on_press)
        .style(move |_, status| style(t, status, active))
        .into()
}

fn style(t: &Tokens, status: iced::widget::button::Status, active: bool) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    iced::widget::button::Style {
        background: None,
        text_color: match status {
            _ if active => t.text,
            Status::Hovered | Status::Pressed => t.text,
            _ => t.text_muted,
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::button::Status;

    #[test]
    fn active_and_hovered_tabs_read_as_text_others_muted() {
        let t = Tokens::for_theme(0);
        assert_eq!(style(t, Status::Active, true).text_color, t.text);
        assert_eq!(style(t, Status::Hovered, false).text_color, t.text);
        assert_eq!(style(t, Status::Active, false).text_color, t.text_muted);
        assert_eq!(style(t, Status::Disabled, false).text_color, t.text_muted, "captions stay muted");
    }

    #[test]
    fn zero_counts_are_hidden() {
        let tab: Tab<()> = Tab::new("Headers", ()).count(0);
        assert!(tab.count.is_none());
        assert_eq!(Tab::new("Headers", ()).count(6).count, Some(6));
    }
}
