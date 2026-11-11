//! Parses spreadsheet response bodies (xlsx/xls/xlsb/ods) for preview.

use std::io::Cursor;

use calamine::{Data, DataRef, Reader, Sheets, open_workbook_auto_from_rs};

/// The preview shows at most this many rows and columns; the rest is for
/// the saved file. Bounded so the preview stays one cheap paragraph.
pub const MAX_ROWS: usize = 200;
pub const MAX_COLUMNS: usize = 26;
/// Characters of a cell shown before it is cut with "…".
const CELL_CHARS: usize = 16;
/// Spaces between preview columns.
const COLUMN_GAP: usize = 2;

/// A spreadsheet's first sheet, laid out for a plain-text preview.
#[derive(Debug, Clone)]
pub struct ParsedSheet {
    /// "Sheet1 · 12 rows", noting what the preview leaves out.
    pub summary: String,
    /// The first row, then the others, in fixed-width columns.
    pub header: String,
    pub body: String,
}

/// The cells of the preview, at most [`MAX_ROWS`] × [`MAX_COLUMNS`], with
/// rows counted from the sheet's first filled one.
#[derive(Default)]
struct Grid {
    rows: Vec<Vec<String>>,
    first_row: Option<u32>,
    cut: bool,
}

impl Grid {
    /// Places a cell; false once the cells are past the last previewed row,
    /// since sheets list them row by row.
    fn put(&mut self, (row, col): (u32, u32), value: String) -> bool {
        let first = *self.first_row.get_or_insert(row);
        let (row, col) = (index(row.saturating_sub(first)), index(col));
        if row >= MAX_ROWS {
            self.cut = true;
            return false;
        }
        if col >= MAX_COLUMNS {
            self.cut = true;
            return true;
        }
        if self.rows.len() <= row {
            self.rows.resize_with(row + 1, Vec::new);
        }
        let cells = &mut self.rows[row];
        if cells.len() <= col {
            cells.resize(col + 1, String::new());
        }
        cells[col] = value;
        true
    }
}

fn index(n: u32) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// Parses the first worksheet of a spreadsheet file for preview.
///
/// xlsx and xlsb sheets are streamed cell by cell and left as soon as the
/// preview is full, so a sheet claiming a range like A1:XFD1048576 costs no
/// more than a small one. Returns `Err` with a short message if the bytes
/// aren't a recognisable spreadsheet, or it has no sheets.
pub fn parse_first_sheet(bytes: &[u8]) -> Result<ParsedSheet, String> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let name = workbook
        .sheet_names()
        .into_iter()
        .next()
        .ok_or_else(|| "Spreadsheet has no sheets".to_owned())?;
    let mut grid = Grid::default();
    let cell = |c: &DataRef<'_>| Data::from(c.clone()).to_string();
    let total_rows = match &mut workbook {
        Sheets::Xlsx(x) => {
            let mut cells = x.worksheet_cells_reader(&name).map_err(|e| e.to_string())?;
            let dimensions = cells.dimensions();
            while let Some(c) = cells.next_cell().map_err(|e| e.to_string())? {
                if !grid.put(c.get_position(), cell(c.get_value())) {
                    break;
                }
            }
            dimensions.end.0.saturating_sub(dimensions.start.0).saturating_add(1)
        }
        Sheets::Xlsb(x) => {
            let mut cells = x.worksheet_cells_reader(&name).map_err(|e| e.to_string())?;
            let dimensions = cells.dimensions();
            while let Some(c) = cells.next_cell().map_err(|e| e.to_string())? {
                if !grid.put(c.get_position(), cell(c.get_value())) {
                    break;
                }
            }
            dimensions.end.0.saturating_sub(dimensions.start.0).saturating_add(1)
        }
        Sheets::Xls(_) | Sheets::Ods(_) => {
            // These formats only parse whole; the range comes back built.
            let range = workbook.worksheet_range(&name).map_err(|e| e.to_string())?;
            let (top, left) = range.start().unwrap_or((0, 0));
            let offset = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
            for (row, col, value) in range.used_cells() {
                let position = (top.saturating_add(offset(row)), left.saturating_add(offset(col)));
                if !grid.put(position, value.to_string()) {
                    break;
                }
            }
            u32::try_from(range.height()).unwrap_or(u32::MAX)
        }
    };
    Ok(layout(&name, &grid, total_rows))
}

fn layout(name: &str, grid: &Grid, total_rows: u32) -> ParsedSheet {
    let shown = grid.rows.len();
    let summary = if grid.cut {
        format!("{name} · first {shown} of {total_rows} rows, up to {MAX_COLUMNS} columns · save the file for the rest")
    } else {
        format!("{name} · {shown} row{}", if shown == 1 { "" } else { "s" })
    };
    let mut lines = grid.rows.iter().map(|cells| line(cells));
    let header = lines.next().unwrap_or_default();
    let body = lines.collect::<Vec<_>>().join("\n");
    ParsedSheet { summary, header, body }
}

fn line(cells: &[String]) -> String {
    let mut out = String::new();
    for cell in cells {
        let len = cell.chars().count();
        if len > CELL_CHARS {
            out.extend(cell.chars().take(CELL_CHARS - 1));
            out.push('…');
        } else {
            out.push_str(cell);
            out.extend(std::iter::repeat_n(' ', CELL_CHARS - len));
        }
        out.extend(std::iter::repeat_n(' ', COLUMN_GAP));
    }
    out.truncate(out.trim_end().len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    /// Hand-builds a minimal, spec-valid .xlsx (a zip of a few small XML
    /// parts) entirely in memory, so this test needs no external fixture
    /// file or network access.
    fn build_minimal_xlsx() -> Vec<u8> {
        let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#;

        let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;

        let workbook_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;

        let workbook = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#;

        let sheet1 = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Age</t></is></c></row>
<row r="2"><c r="A2" t="inlineStr"><is><t>Alice</t></is></c><c r="B2"><v>30</v></c></row>
</sheetData>
</worksheet>"#;

        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (name, contents) in [
            ("[Content_Types].xml", content_types),
            ("_rels/.rels", root_rels),
            ("xl/_rels/workbook.xml.rels", workbook_rels),
            ("xl/workbook.xml", workbook),
            ("xl/worksheets/sheet1.xml", sheet1),
        ] {
            zip.start_file(name, options).expect("start_file");
            zip.write_all(contents.as_bytes()).expect("write");
        }
        zip.finish().expect("finish").into_inner()
    }

    #[test]
    fn parses_first_sheet_rows_and_header() {
        let xlsx = build_minimal_xlsx();

        let sheet = parse_first_sheet(&xlsx).expect("should parse as a spreadsheet");

        assert_eq!(sheet.summary, "Sheet1 · 2 rows");
        assert_eq!(sheet.header, format!("Name{}Age", " ".repeat(CELL_CHARS - 4 + COLUMN_GAP)));
        assert_eq!(sheet.body, format!("Alice{}30", " ".repeat(CELL_CHARS - 5 + COLUMN_GAP)));
    }

    #[test]
    fn a_sheet_spanning_the_whole_grid_previews_only_its_corner() {
        let mut grid = Grid::default();
        assert!(grid.put((0, 0), "a".into()));
        assert!(grid.put((0, 16_383), "far right".into()), "a far column is skipped, not fatal");
        assert!(!grid.put((1_048_575, 16_383), "far corner".into()), "reading stops past the last row");
        assert_eq!(grid.rows, [vec!["a".to_owned()]]);
        let sheet = layout("Big", &grid, 1_048_576);
        assert!(sheet.summary.starts_with("Big · first 1 of 1048576 rows"), "{}", sheet.summary);
    }

    #[test]
    fn long_cells_are_cut_to_the_column() {
        let cut = line(&["x".repeat(CELL_CHARS + 5)]);
        assert_eq!(cut.chars().count(), CELL_CHARS);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn rejects_non_spreadsheet_bytes() {
        let result = parse_first_sheet(b"not a spreadsheet at all");
        assert!(result.is_err());
    }
}
