//! Two stacked sidebar sections: expanded upper body fills spare space.
use iced::{
    Element, Length,
    widget::{Space, column, container},
};

pub fn sidebar_sections<'a, M: 'a, R: iced::advanced::Renderer + 'a>(
    upper_header: Element<'a, M, iced::Theme, R>,
    upper_body: Option<Element<'a, M, iced::Theme, R>>,
    lower: Element<'a, M, iced::Theme, R>,
) -> Element<'a, M, iced::Theme, R> {
    let mut sections = column![upper_header].spacing(0);
    if let Some(body) = upper_body {
        sections = sections
            .push(container(body).height(Length::Fill))
            .push(lower);
    } else {
        sections = sections.push(lower).push(Space::new().height(Length::Fill));
    }
    sections.height(Length::Fill).width(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{
        Size,
        advanced::{layout, widget::Tree},
    };

    #[test]
    fn lower_section_stays_at_bottom_until_upper_collapses() {
        for height in [300.0, 700.0] {
            for expanded in [false, true] {
                let header: Element<'_, (), iced::Theme, ()> = Space::new().height(24).into();
                let body = expanded.then(|| Space::new().height(Length::Fill).into());
                let lower = Space::new().height(96).into();
                let mut element = sidebar_sections(header, body, lower);
                let mut tree = Tree::new(&element);
                let node = element.as_widget_mut().layout(
                    &mut tree,
                    &(),
                    &layout::Limits::new(Size::ZERO, Size::new(300.0, height)),
                );
                let lower_index = if expanded { 2 } else { 1 };
                let bounds = node.children()[lower_index].bounds();
                assert_eq!(bounds.y, if expanded { height - 96.0 } else { 24.0 });
                assert_eq!(bounds.height, 96.0);
            }
        }
    }
}
