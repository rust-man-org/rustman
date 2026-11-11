//! Key-value tables (params, headers): 32 px rows, a checkbox column, the
//! key at 35 %, the value after a hairline, delete on hover, and a trailing
//! placeholder row that turns into a real row as soon as it is typed in.

use iced::widget::{Space, checkbox, column, container, hover, row, text, text_input};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding};

use crate::message::KvField;
use crate::ui::components::hover_box::hover_box;
use crate::ui::components::icon_button::IconSize;
use crate::ui::components::{field, hairline, icon_button, tooltip, vhairline};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_LG, HAIRLINE, LINE_SM, MONO, line, RADIUS_XS, SPACE_LG, SPACE_MD, TABLE_ROW_HEIGHT, TEXT_SM, Tokens,
    UI_FONT,
};

/// Percent of the width up to the value column's hairline.
const KEY_PORTION: u16 = 35;
/// Side of the checkbox and delete cells.
const EDGE_CELL: f32 = CONTROL_LG;
const CHECKBOX: f32 = 14.0;

pub struct Row<'a, M> {
    /// What the row's messages carry.
    pub id: &'a str,
    pub enabled: bool,
    pub key: &'a str,
    pub value: &'a str,
    /// Shown after the value input (a header's value suggestions).
    pub suffix: Option<Element<'a, M>>,
}

/// The id of a data row's input, shared by the table and whatever moves the
/// focus into a cell: typing in the placeholder row makes a data row there,
/// and the focus has to follow the text into it.
pub fn input_id(row_id: &str, field: KvField) -> iced::widget::Id {
    let field = match field {
        KvField::Key => "key",
        KvField::Value => "value",
    };
    iced::widget::Id::from(format!("kv-{field}-{row_id}"))
}

/// What each part of a row publishes; `append` comes from the placeholder row.
pub struct Actions<M> {
    pub toggle: fn(String) -> M,
    pub key: fn(String, String) -> M,
    pub value: fn(String, String) -> M,
    pub remove: fn(String) -> M,
    pub append: fn(KvField, String) -> M,
}

impl<M> Clone for Actions<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for Actions<M> {}

pub fn kv_table<'a, M: Clone + 'a>(
    t: &'a Tokens,
    rows: Vec<Row<'a, M>>,
    actions: Actions<M>,
    placeholder: &'a str,
    help: Option<&'a str>,
) -> Element<'a, M> {
    let empty = rows.is_empty();
    let mut table = column![];
    for r in rows {
        table = table.push(data_row(t, r, actions));
    }
    table = table.push(placeholder_row(t, actions, placeholder));
    let mut content = column![table];
    if let (true, Some(help)) = (empty, help) {
        content = content.push(container(text(help).size(TEXT_SM).line_height(line(LINE_SM)).color(t.text_muted)).padding(SPACE_LG));
    }
    content.into()
}

fn data_row<'a, M: Clone + 'a>(t: &'a Tokens, r: Row<'a, M>, a: Actions<M>) -> Element<'a, M> {
    let id = r.id;
    let key = cell(t, "Key", r.key, MONO, Some(input_id(id, KvField::Key)), move |s| (a.key)(id.to_owned(), s));
    let value = cell(t, "Value", r.value, MONO, Some(input_id(id, KvField::Value)), move |s| (a.value)(id.to_owned(), s));
    let mut value_cell = row![value].align_y(Alignment::Center);
    if let Some(suffix) = r.suffix {
        value_cell = value_cell.push(suffix);
    }
    let check = checkbox(r.enabled)
        .on_toggle(move |_| (a.toggle)(id.to_owned()))
        .size(CHECKBOX)
        .spacing(0)
        .style(move |_, status| checkbox_style(t, status));
    let check = container(check)
        .width(EDGE_CELL)
        .padding(Padding { left: SPACE_LG, ..Padding::ZERO })
        .center_y(Length::Fill);
    let base = framed(t, columns(t, check.into(), key, value_cell.into()));
    let remove = icon_button(t, icons::close().size(TEXT_SM), IconSize::Small).on_press_with(move || (a.remove)(id.to_owned()));
    let remove = tooltip(t, remove, "Remove", None);
    let tinted = hover_box(base, move |hovered| container::Style {
        background: hovered.then_some(Background::Color(t.hover)),
        ..Default::default()
    });
    hover(tinted, row![Space::new().width(Length::Fill), container(remove).center(EDGE_CELL)])
}

fn placeholder_row<'a, M: Clone + 'a>(t: &'a Tokens, a: Actions<M>, placeholder: &'a str) -> Element<'a, M> {
    framed(
        t,
        columns(
            t,
            Space::new().width(EDGE_CELL).into(),
            cell(t, placeholder, "", UI_FONT, None, move |s| (a.append)(KvField::Key, s)),
            cell(t, "Value", "", UI_FONT, None, move |s| (a.append)(KvField::Value, s)),
        ),
    )
}

/// The mockup's grid: checkbox and key take 35 %, the value (after its
/// hairline) and the delete slot take the rest.
fn columns<'a, M: 'a>(t: &'a Tokens, lead: Element<'a, M>, key: Element<'a, M>, value: Element<'a, M>) -> iced::widget::Row<'a, M> {
    row![
        row![lead, container(key).width(Length::Fill).center_y(Length::Fill)].width(Length::FillPortion(KEY_PORTION)),
        row![
            vhairline(t),
            container(value).width(Length::Fill).center_y(Length::Fill),
            Space::new().width(EDGE_CELL),
        ]
        .width(Length::FillPortion(100 - KEY_PORTION)),
    ]
}

/// A 32 px row with its bottom hairline.
fn framed<'a, M: 'a>(t: &'a Tokens, content: iced::widget::Row<'a, M>) -> Element<'a, M> {
    column![content.height(TABLE_ROW_HEIGHT - HAIRLINE).align_y(Alignment::Center), hairline(t)].into()
}

/// A borderless input; values too large to edit inline show a preview.
fn cell<'a, M: Clone + 'a>(
    t: &'a Tokens,
    placeholder: &'a str,
    value: &'a str,
    font: Font,
    id: Option<iced::widget::Id>,
    on_input: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    if !crate::ui::request::cell::is_inline(value) {
        return crate::ui::request::cell::preview(value, t);
    }
    let mut input = text_input(placeholder, value);
    if let Some(id) = id {
        input = input.id(id);
    }
    input
        .on_input(on_input)
        .font(font)
        .size(TEXT_SM)
        .padding([0.0, SPACE_MD])
        .style(move |_, status| field::cell(t, status))
        .into()
}

fn checkbox_style(t: &Tokens, status: iced::widget::checkbox::Status) -> iced::widget::checkbox::Style {
    use iced::widget::checkbox::Status;
    let checked = matches!(
        status,
        Status::Active { is_checked: true } | Status::Hovered { is_checked: true } | Status::Disabled { is_checked: true }
    );
    iced::widget::checkbox::Style {
        background: Background::Color(if checked { t.accent_fill } else { t.surface }),
        icon_color: t.on_accent,
        border: Border {
            color: if checked { t.accent_fill } else { t.border_strong },
            width: if checked { 0.0 } else { HAIRLINE },
            radius: RADIUS_XS.into(),
        },
        text_color: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::checkbox::Status;

    #[test]
    fn checked_boxes_use_the_primary_fill() {
        let t = Tokens::for_theme(0);
        let on = checkbox_style(t, Status::Active { is_checked: true });
        assert_eq!(on.background, Background::Color(t.accent_fill));
        let off = checkbox_style(t, Status::Active { is_checked: false });
        assert_eq!(off.border.color, t.border_strong);
    }
}
