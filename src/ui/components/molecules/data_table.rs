//! DataTable molecule: a compact read-only table with a caption header row.
//! Cells are any element, so a column can hold buttons.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Column, Row, column, container},
};

use crate::ui::{components::atoms::text, theme::Palette};

/// Builder:
/// `DataTable::new([("Field", Length::Fixed(130.0)), ("main", Length::FillPortion(1))])
///     .row(vec![a, b])`.
/// Each row must have one cell per column.
pub struct DataTable<'a, M> {
    widths: Vec<Length>,
    rows: Column<'a, M>,
}

impl<'a, M: 'a> DataTable<'a, M> {
    pub fn new(columns: impl IntoIterator<Item = (&'static str, Length)>) -> Self {
        let (headers, widths): (Vec<_>, Vec<_>) = columns.into_iter().unzip();
        let header = line(
            headers.into_iter().map(|h| text::caption(h)).collect(),
            &widths,
        );
        Self {
            rows: column![header].spacing(0),
            widths,
        }
    }

    pub fn row(mut self, cells: Vec<Element<'a, M>>) -> Self {
        debug_assert_eq!(cells.len(), self.widths.len(), "one cell per column");
        self.rows = self.rows.push(line(cells, &self.widths));
        self
    }
}

fn line<'a, M: 'a>(cells: Vec<Element<'a, M>>, widths: &[Length]) -> Element<'a, M> {
    let mut r: Row<'a, M> = Row::new().spacing(10).align_y(Alignment::Center);
    for (cell, w) in cells.into_iter().zip(widths) {
        r = r.push(container(cell).width(*w));
    }
    container(r).padding([5, 10]).width(Length::Fill).into()
}

impl<'a, M: 'a> From<DataTable<'a, M>> for Element<'a, M> {
    fn from(t: DataTable<'a, M>) -> Self {
        container(t.rows)
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
}
