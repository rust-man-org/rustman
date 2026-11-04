//! Tag atom: a tiny bordered code-font label, e.g. the `HTTP` protocol
//! marker in the request breadcrumb.

use iced::{
    Border, Element,
    widget::{container, text},
};

use crate::ui::{
    components::Tone,
    theme::{MONO, Palette},
};

pub fn tag<'a, M: 'a>(label: impl Into<String>, tone: Tone) -> Element<'a, M> {
    container(text(label.into()).size(9).font(MONO).color(tone.color()))
        .padding([1, 4])
        .style(|_| container::Style {
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 3.0.into(),
            },
            ..Default::default()
        })
        .into()
}
