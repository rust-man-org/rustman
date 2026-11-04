//! Pill atom: a small rounded label with an optional leading icon, e.g.
//! the branch name next to the workspace.

use iced::{
    Alignment, Background, Border, Element,
    widget::{Text, container, row, text},
};

use crate::ui::{
    components::Tone,
    theme::{MONO, Palette, TEXT_XS},
};

/// `mono` renders the label in the code font (names, ids, branches).
pub fn pill<'a, M: 'a>(icon: Option<Text<'static>>, label: impl Into<String>, mono: bool) -> Element<'a, M> {
    let fg = Tone::Neutral.color();
    let mut label = text(label.into()).size(TEXT_XS).color(fg);
    if mono {
        label = label.font(MONO);
    }
    let mut content = row![].spacing(5).align_y(Alignment::Center);
    if let Some(icon) = icon {
        content = content.push(icon.size(10).color(fg));
    }
    container(content.push(label))
        .padding([3, 9])
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface_high())),
            border: Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
