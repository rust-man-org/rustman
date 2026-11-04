//! DataTable molecule: a compact read-only table with a caption header row.
//! Cells are any element, so a column can hold buttons.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Column, Row, column, container, rule},
};

use crate::ui::{components::atoms::text, theme::Palette};

/// Builder:
/// `DataTable::new([("Field", Length::Fixed(130.0)), ("main", Length::FillPortion(1))])
///     .row(vec![a, b])`.
/// Each row must have one cell per column.
pub struct DataTable<'a, M> {
    widths: Vec<Length>,
    rows: Column<'a, M>,
    ruled: bool,
}

impl<'a, M: 'a> DataTable<'a, M> {
    pub fn new(columns: impl IntoIterator<Item = (&'static str, Length)>) -> Self {
        Self::build(columns, false)
    }

    /// Dense editor table with straight edges and hairline cell dividers.
    pub fn ruled(columns: impl IntoIterator<Item = (&'static str, Length)>) -> Self {
        Self::build(columns, true)
    }

    fn build(columns: impl IntoIterator<Item = (&'static str, Length)>, ruled: bool) -> Self {
        let (headers, widths): (Vec<_>, Vec<_>) = columns.into_iter().unzip();
        let header = line(
            headers.into_iter().map(|h| text::caption(h)).collect(),
            &widths,
            ruled,
        );
        Self {
            rows: column![header].spacing(0),
            widths,
            ruled,
        }
    }

    pub fn row(mut self, cells: Vec<Element<'a, M>>) -> Self {
        debug_assert_eq!(cells.len(), self.widths.len(), "one cell per column");
        self.rows = self.rows.push(line(cells, &self.widths, self.ruled));
        self
    }

    pub fn row_with_hover(mut self, cells: Vec<Element<'a, M>>, enter: M, exit: M) -> Self
    where
        M: Clone,
    {
        debug_assert_eq!(cells.len(), self.widths.len(), "one cell per column");
        self.rows = self.rows.push(
            iced::widget::mouse_area(line(cells, &self.widths, self.ruled))
                .on_enter(enter)
                .on_exit(exit),
        );
        self
    }
}

fn line<'a, M: 'a>(cells: Vec<Element<'a, M>>, widths: &[Length], ruled: bool) -> Element<'a, M> {
    let mut r: Row<'a, M> = Row::new()
        .spacing(if ruled { 0 } else { 10 })
        .align_y(Alignment::Center);
    if ruled {
        r = r.height(28);
    }
    for (i, (cell, w)) in cells.into_iter().zip(widths).enumerate() {
        if ruled && i > 0 {
            r = r.push(rule::vertical(1).style(crate::ui::styles::divider));
        }
        r = r.push(
            container(cell)
                .padding(if ruled { [2, 6] } else { [0, 0] })
                .width(*w),
        );
    }
    if ruled {
        return column![r, rule::horizontal(1).style(crate::ui::styles::divider)]
            .spacing(0)
            .into();
    }
    container(r).padding([5, 10]).width(Length::Fill).into()
}

impl<'a, M: 'a> From<DataTable<'a, M>> for Element<'a, M> {
    fn from(t: DataTable<'a, M>) -> Self {
        let ruled = t.ruled;
        container(t.rows)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(move |_| container::Style {
                background: (!ruled).then(|| Background::Color(Palette::surface())),
                border: Border {
                    color: Palette::border_subtle(),
                    width: if ruled { 0.0 } else { 1.0 },
                    radius: if ruled { 0.0.into() } else { 6.0.into() },
                },
                ..Default::default()
            })
            .into()
    }
}
