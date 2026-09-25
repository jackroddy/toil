//! Columns, and the schema that fixes a table's shape.

use std::fmt::Write;

use crate::cell::{Align, Cell, Row};
use crate::render::Widths;
use crate::style::{Marker, Stack, Style};

/// How a numeric cell becomes text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// However the number writes itself, which for a float is as many places
    /// as it takes to read back as the same value.
    #[default]
    Plain,
    /// Fixed point to this many places.
    Fixed(usize),
    /// Scientific notation to this many places.
    Scientific(usize),
}

impl Format {
    pub(crate) fn apply(self, n: f64) -> String {
        let mut out = String::new();
        self.write(n, &mut out);
        out
    }

    /// Write `n` out against this format, into a buffer the caller keeps.
    pub(crate) fn write(self, n: f64, out: &mut String) {
        out.clear();

        let written = match self {
            Format::Plain => write!(out, "{n}"),
            Format::Fixed(places) => write!(out, "{n:.places$}"),
            Format::Scientific(places) => write!(out, "{n:.places$e}"),
        };

        // writing to a String fails only if the allocator does, which aborts
        written.expect("a String accepts everything written to it");
    }
}

/// One column: what it is called, and how its cells are written.
#[derive(Clone, Debug)]
pub struct Column {
    /// The label's words, one per header line.
    pub(crate) label: Vec<String>,
    pub(crate) align: Align,
    pub(crate) min_width: usize,
    pub(crate) format: Format,
    pub(crate) ragged: bool,
}

impl Column {
    /// A column headed by a single word.
    pub fn new(label: impl Into<String>) -> Column {
        Column {
            label: vec![label.into()],
            align: Align::default(),
            min_width: 0,
            format: Format::default(),
            ragged: false,
        }
    }

    /// A column whose label is stacked over several header lines, so that a
    /// long name does not set the column's width on its own.
    pub fn stacked(words: impl IntoIterator<Item = impl Into<String>>) -> Column {
        Column {
            label: words.into_iter().map(Into::into).collect(),
            ..Column::new("")
        }
    }

    /// Which side this column's cells are padded on.
    pub fn align(mut self, align: Align) -> Column {
        self.align = align;
        self
    }

    /// A width this column will not shrink below, whatever is in it.
    pub fn min_width(mut self, width: usize) -> Column {
        self.min_width = width;
        self
    }

    /// Write this column's numbers in fixed point, to `places` places.
    pub fn fixed(mut self, places: usize) -> Column {
        self.format = Format::Fixed(places);
        self
    }

    /// Write this column's numbers in scientific notation, to `places` places.
    pub fn scientific(mut self, places: usize) -> Column {
        self.format = Format::Scientific(places);
        self
    }

    /// Never pad this column, and rule it to the width of its label.
    ///
    /// For a column whose width belongs to the row rather than to the table --
    /// a command line, a comma-joined list, a name the rows around it repeat.
    /// Everything after an unpadded column shifts with it, so those columns
    /// line up only across rows whose ragged cells are the same width. A cell
    /// carrying a space has to be in the last column, since a reader splitting
    /// on whitespace cannot tell it from two cells.
    pub fn ragged(mut self) -> Column {
        self.ragged = true;
        self
    }

    /// The widest of the label's words, which is the floor the header sets.
    pub(crate) fn label_width(&self) -> usize {
        self.label
            .iter()
            .map(|word| word.chars().count())
            .max()
            .unwrap_or(0)
    }

    /// The label's word on header line `line`, of `lines` in total.
    pub(crate) fn word(&self, line: usize, lines: usize, stack: Stack) -> &str {
        let index = match stack {
            Stack::Top => Some(line),
            Stack::Bottom => line.checked_sub(lines - self.label.len()),
        };

        index
            .and_then(|i| self.label.get(i))
            .map(String::as_str)
            .unwrap_or("")
    }
}

impl From<&str> for Column {
    fn from(label: &str) -> Column {
        Column::new(label)
    }
}

impl From<String> for Column {
    fn from(label: String) -> Column {
        Column::new(label)
    }
}

impl From<&String> for Column {
    fn from(label: &String) -> Column {
        Column::new(label.as_str())
    }
}

/// The columns a table carries, and the style they are written in.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub(crate) columns: Vec<Column>,
    pub(crate) style: Style,
}

impl Schema {
    /// These columns, in the default style.
    pub fn new(columns: impl IntoIterator<Item = impl Into<Column>>) -> Schema {
        Schema {
            columns: columns.into_iter().map(Into::into).collect(),
            style: Style::default(),
        }
    }

    /// The marker, rule, stacking and padding these columns are written with.
    pub fn style(mut self, style: Style) -> Schema {
        self.style = style;
        self
    }

    /// Write `cells` out against these columns.
    ///
    /// Fills a short row out with missing cells and cuts a long one, so every
    /// row is as wide as the schema.
    pub fn row(&self, cells: impl IntoIterator<Item = impl Into<Cell>>) -> Row {
        let mut cells = cells.into_iter().map(Into::into);

        Row {
            cells: self
                .columns
                .iter()
                .map(|column| {
                    cells
                        .next()
                        .unwrap_or_else(Cell::missing)
                        .resolve(column, &self.style)
                })
                .collect(),
        }
    }

    /// The widths the header and the columns' own minimums call for, before
    /// any row has been seen.
    ///
    /// This is what a [`Stream`](crate::Stream) takes when there is nothing to
    /// measure yet, and the floor that blocks share when they are written one
    /// at a time.
    pub fn widths(&self) -> Widths {
        let lines = self.header_lines();

        Widths(
            self.columns
                .iter()
                .enumerate()
                .map(|(i, column)| {
                    let mut width = column.min_width.max(column.label_width());

                    // under Absorb the "# " sits inside the first column's
                    // header cell, so the column has to be that much wider to
                    // hold its own label
                    if i == 0 && lines > 0 && self.style.marker == Marker::Absorb {
                        width = width.max(column.label_width() + MARKER.len());
                    }

                    width
                })
                .collect(),
        )
    }

    /// [`widths`](Self::widths), widened until every one of `rows` fits.
    pub fn measure<'a>(&self, rows: impl IntoIterator<Item = &'a Row>) -> Widths {
        let mut widths = self.widths();
        for row in rows {
            widths.widen(row);
        }
        widths
    }

    /// How many lines the header takes, which is the longest label's word count.
    pub(crate) fn header_lines(&self) -> usize {
        self.columns
            .iter()
            .map(|column| column.label.len())
            .max()
            .unwrap_or(0)
    }

    /// Whether column `i` is written unpadded.
    pub(crate) fn is_ragged(&self, i: usize) -> bool {
        self.columns.get(i).is_some_and(|column| column.ragged)
    }
}

/// What opens a line that is not data.
pub(crate) const MARKER: &str = "# ";

impl<T: Into<Column>, const N: usize> From<[T; N]> for Schema {
    fn from(columns: [T; N]) -> Schema {
        Schema::new(columns)
    }
}

impl<T: Into<Column>> From<Vec<T>> for Schema {
    fn from(columns: Vec<T>) -> Schema {
        Schema::new(columns)
    }
}
