//! Laying columns out and writing them.

use std::fmt::Display;
use std::path::Path;

use crate::cell::{Align, Cell, Row, Text};
use crate::column::{MARKER, Schema};
use crate::style::{Marker, Rule, Trailing};

/// What opens a data line, standing in for the [`MARKER`] above it.
pub(crate) const INDENT: &str = "  ";

/// How wide each column is written, which tables written at different moments
/// can share to line up under one header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Widths(pub(crate) Vec<usize>);

impl Widths {
    /// Widen until `row` fits.
    pub fn widen(&mut self, row: &Row) {
        for (width, cell) in self.0.iter_mut().zip(&row.cells) {
            *width = (*width).max(cell.width());
        }
    }

    /// Widen until `other` fits.
    pub fn max(&mut self, other: &Widths) {
        for (width, &other) in self.0.iter_mut().zip(&other.0) {
            *width = (*width).max(other);
        }
    }

    /// The widths, in column order.
    pub fn as_slice(&self) -> &[usize] {
        &self.0
    }
}

/// Whether a render carries the column names or begins at the rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Header {
    /// Write the preamble, the column names and the rule above the rows.
    #[default]
    Show,
    /// Begin at the rows, for a block following a header already written.
    Hide,
}

/// A table held whole: every row is in hand before anything is written.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub(crate) schema: Schema,
    pub(crate) preamble: Vec<String>,
    pub(crate) rows: Vec<Row>,
    /// `#` lines between the rows, each with the number of rows above it.
    pub(crate) interludes: Vec<(usize, String)>,
}

impl Table {
    /// An empty table with these columns.
    pub fn new(schema: impl Into<Schema>) -> Table {
        Table {
            schema: schema.into(),
            preamble: Vec::new(),
            rows: Vec::new(),
            interludes: Vec::new(),
        }
    }

    /// Add a `#` line above the header, for a reader.
    pub fn comment(&mut self, line: impl Display) -> &mut Table {
        self.preamble.push(format!("#{}", space(line)));
        self
    }

    /// Add a `#=` line above the header, for a parser.
    pub fn meta(&mut self, line: impl Display) -> &mut Table {
        self.preamble.push(format!("#={}", space(line)));
        self
    }

    /// Write `cells` out against the columns and add the row.
    pub fn row(&mut self, cells: impl IntoIterator<Item = impl Into<Cell>>) -> &mut Table {
        let row = self.schema.row(cells);
        self.push(row)
    }

    /// Add a row already built by [`Schema::row`].
    pub fn push(&mut self, row: Row) -> &mut Table {
        self.rows.push(row);
        self
    }

    /// Add a line between the rows, after every row added so far.
    ///
    /// `line` is written as given, so it has to open with `#` for a reader to
    /// tell it from a row: `#= shard 2` to mark where a block begins, say.
    /// This is where [`Table::parse`] puts the `#` lines it finds below the
    /// header, such as a [`Stream`](crate::Stream)'s `meta` between blocks.
    pub fn interlude(&mut self, line: impl Display) -> &mut Table {
        self.interludes.push((self.rows.len(), line.to_string()));
        self
    }

    /// The widths these rows need.
    pub fn widths(&self) -> Widths {
        self.schema.measure(&self.rows)
    }

    /// The table as text, every column padded to its own widest cell.
    pub fn render(&self) -> String {
        self.render_with(&self.widths(), Header::Show)
    }

    /// The table as text, starting from `widths` rather than from the header.
    ///
    /// The widths are a floor and not a ceiling: a column whose cells are wider
    /// than `widths` allows still gets the room it needs. A `widths` built for
    /// a different set of columns is ignored.
    pub fn render_with(&self, widths: &Widths, header: Header) -> String {
        let mut widths = match widths.0.len() == self.schema.columns.len() {
            true => widths.clone(),
            false => self.schema.widths(),
        };

        for row in &self.rows {
            widths.widen(row);
        }

        let mut out = String::new();
        if header == Header::Show {
            out.push_str(&self.preamble.join("\n"));
            if !self.preamble.is_empty() {
                out.push('\n');
            }

            // a comment is written the same way as a header line,
            // so without a bare `#` between them a reader could
            // not tell a comment directly above the header from
            // the top line of a stacked label
            if self
                .preamble
                .last()
                .is_some_and(|line| needs_separator(line))
            {
                out.push_str("#\n");
            }
            out.push_str(&head(&self.schema, &widths));
        }

        let mut interludes = self.interludes.iter().peekable();
        for (i, row) in self.rows.iter().enumerate() {
            while let Some((_, text)) = interludes.next_if(|(at, _)| *at <= i) {
                out.push_str(text);
                out.push('\n');
            }
            out.push_str(&line(
                &self.schema,
                &widths,
                data_open(&self.schema),
                &row.cells,
            ));
        }
        for (_, text) in interludes {
            out.push_str(text);
            out.push('\n');
        }

        out
    }

    /// Render the table and write it to `path`, creating the directories above
    /// it if they are not there.
    pub fn write(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.render())
    }
}

/// Whether a bare `#` has to go between `line`, the last line above the
/// header, and the header itself.
pub(crate) fn needs_separator(line: &str) -> bool {
    // a `#=` line or a bare `#` already says where it ends
    line != "#" && !line.starts_with("#=")
}

/// A comment line's text, spaced off the `#` unless it is empty.
pub(crate) fn space(line: impl Display) -> String {
    let line = line.to_string();
    match line.is_empty() {
        true => line,
        false => format!(" {line}"),
    }
}

/// The header lines and the rule under them.
pub(crate) fn head(schema: &Schema, widths: &Widths) -> String {
    let lines = schema.header_lines();
    let mut out = String::new();

    for i in 0..lines {
        let mut cells: Vec<Text> = schema
            .columns
            .iter()
            .map(|column| Text {
                text: column.word(i, lines, schema.style.stack).to_string(),
                align: Align::Left,
                missing: false,
            })
            .collect();

        let open = mark(schema, &mut cells);
        out.push_str(&line(schema, widths, open, &cells));
    }

    out.push_str(&rule(schema, widths));
    out
}

fn rule(schema: &Schema, widths: &Widths) -> String {
    match schema.style.rule {
        Rule::None => String::new(),
        Rule::Solid => format!("#{}\n", "-".repeat(span(schema, widths).saturating_sub(1))),
        Rule::Dashes => {
            let mut cells: Vec<Text> = (0..schema.columns.len())
                .map(|i| Text {
                    text: "-".repeat(rule_width(schema, widths, i)),
                    align: Align::Left,
                    missing: false,
                })
                .collect();

            // the marker takes the place of the first dashes rather
            // than sitting in front of them, so the rule stays as wide
            // as its column. a ragged first column has no width to give
            // up, and its rule is its label's width however the marker
            // is written
            if schema.style.marker == Marker::Absorb
                && !schema.is_ragged(0)
                && let Some(first) = cells.first_mut()
            {
                first
                    .text
                    .truncate(first.text.len().saturating_sub(MARKER.len()));
            }

            let open = mark(schema, &mut cells);
            line(schema, widths, open, &cells)
        }
    }
}

/// How wide a column is ruled. A ragged column has no width of its own, so its
/// rule underlines its label instead.
fn rule_width(schema: &Schema, widths: &Widths, i: usize) -> usize {
    match schema.is_ragged(i) {
        true => schema.columns[i].label_width(),
        false => widths.0.get(i).copied().unwrap_or(0),
    }
}

/// How wide the whole table is written, for a rule that crosses all of it.
fn span(schema: &Schema, widths: &Widths) -> usize {
    let columns = schema.columns.len();
    if columns == 0 {
        return 0;
    }

    let cells: usize = (0..columns).map(|i| rule_width(schema, widths, i)).sum();
    let opening = match schema.style.marker {
        Marker::Absorb => 0,
        Marker::Indent => MARKER.len(),
    };

    opening + cells + (columns - 1)
}

/// Put the marker on a line that is not data, and give back what opens it.
///
/// Under [`Marker::Absorb`] it goes inside the first cell, which is already
/// wide enough to hold it; otherwise it opens the line and shifts everything.
fn mark(schema: &Schema, cells: &mut [Text]) -> &'static str {
    match schema.style.marker {
        Marker::Absorb => {
            if let Some(first) = cells.first_mut() {
                first.text.insert_str(0, MARKER);
            }
            ""
        }
        Marker::Indent => MARKER,
    }
}

pub(crate) fn data_open(schema: &Schema) -> &'static str {
    match schema.style.marker {
        Marker::Absorb => "",
        Marker::Indent => INDENT,
    }
}

/// One line: every cell padded to its column and joined by a single space.
pub(crate) fn line(schema: &Schema, widths: &Widths, open: &str, cells: &[Text]) -> String {
    let mut out = String::from(open);

    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }

        if schema.is_ragged(i) {
            out.push_str(&cell.text);
            continue;
        }

        let pad = widths
            .0
            .get(i)
            .copied()
            .unwrap_or(0)
            .saturating_sub(cell.width());
        match cell.align {
            Align::Right => {
                out.extend(std::iter::repeat_n(' ', pad));
                out.push_str(&cell.text);
            }
            Align::Left => {
                out.push_str(&cell.text);
                out.extend(std::iter::repeat_n(' ', pad));
            }
        }
    }

    if schema.style.trailing == Trailing::Trim {
        out.truncate(out.trim_end().len());
    }

    out.push('\n');
    out
}
