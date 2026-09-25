//! A table written a row at a time, to something that cannot be revised.

use std::fmt::Display;
use std::io::Write;

use crate::cell::{Align, Cell, Row};
use crate::column::Schema;
use crate::render::{Widths, data_open, head, line, needs_separator, space};
use crate::style::Trailing;

/// A table written as its rows arrive.
///
/// The widths are fixed when the stream is built: give it
/// [`Schema::widths`](crate::Schema::widths) when there is nothing to measure
/// yet, or [`Schema::measure`](crate::Schema::measure) over the rows already
/// known. A cell wider than its column overruns it.
pub struct Stream<W: Write> {
    schema: Schema,
    widths: Widths,
    started: bool,

    /// Whether the last line written was a comment the header would otherwise
    /// sit directly under.
    separate: bool,

    /// Where a number is formatted before it is padded.
    //
    // kept on the stream so a row of numbers allocates nothing
    scratch: String,

    out: W,
}

impl<W: Write> Stream<W> {
    /// A stream writing to `out`, with its columns fixed at `widths`.
    pub fn new(schema: impl Into<Schema>, widths: Widths, out: W) -> Stream<W> {
        Stream {
            schema: schema.into(),
            widths,
            started: false,
            separate: false,
            scratch: String::new(),
            out,
        }
    }

    /// A stream picking up a table whose header has already been written.
    ///
    /// For a block rendered somewhere other than the file it belongs to --
    /// into a buffer, on another thread -- and appended once it is whole.
    /// Nothing about the header is checked, so `widths` has to be the widths
    /// it went out with.
    pub fn continued(schema: impl Into<Schema>, widths: Widths, out: W) -> Stream<W> {
        Stream {
            started: true,
            ..Stream::new(schema, widths, out)
        }
    }

    /// Write a `#` line, for a reader.
    ///
    /// Written where it falls, so calling this before the first row is what
    /// puts it above the header.
    pub fn comment(&mut self, line: impl Display) -> std::io::Result<()> {
        let line = format!("#{}", space(line));
        self.separate = needs_separator(&line);
        writeln!(self.out, "{line}")
    }

    /// Write a `#=` line, for a parser.
    pub fn meta(&mut self, line: impl Display) -> std::io::Result<()> {
        self.separate = false;
        writeln!(self.out, "#={}", space(line))
    }

    /// Write the header now, if it has not gone out already.
    ///
    /// Writing a row does this first. Call it directly to put the header in
    /// the file before there is anything to report, so a reader following the
    /// file knows what is coming.
    pub fn header(&mut self) -> std::io::Result<()> {
        if self.started {
            return Ok(());
        }

        self.started = true;
        if self.separate {
            self.out.write_all(b"#\n")?;
        }
        self.out
            .write_all(head(&self.schema, &self.widths).as_bytes())
    }

    /// Write `cells` out against the columns and write the row.
    pub fn row(&mut self, cells: impl IntoIterator<Item = impl Into<Cell>>) -> std::io::Result<()> {
        let row = self.schema.row(cells);
        self.push(&row)
    }

    /// Write a row already built by [`Schema::row`].
    pub fn push(&mut self, row: &Row) -> std::io::Result<()> {
        self.header()?;

        let open = data_open(&self.schema);
        self.out
            .write_all(line(&self.schema, &self.widths, open, &row.cells).as_bytes())
    }

    /// Write rows that belong together, so a batch reaches the file at once.
    pub fn block(&mut self, rows: impl IntoIterator<Item = Row>) -> std::io::Result<()> {
        self.header()?;

        for row in rows {
            self.push(&row)?;
        }
        Ok(())
    }

    /// Begin a row written a cell at a time, straight into the writer.
    ///
    /// The row is finished by [`Line::end`], which is what writes the newline.
    /// This is the path for the rows a large table is made of, where the
    /// [`Cell`] per value and the `String` per line that [`row`](Self::row)
    /// builds are the cost of writing it.
    pub fn line(&mut self) -> std::io::Result<Line<'_, W>> {
        self.header()?;

        let schema = &self.schema;
        let pending = data_open(schema).len();

        Ok(Line {
            schema,
            widths: &self.widths,
            scratch: &mut self.scratch,
            out: &mut self.out,
            i: 0,
            pending,
        })
    }

    /// Give the writer back, so a buffered one can be flushed.
    pub fn into_inner(self) -> W {
        self.out
    }
}

/// One row of a [`Stream`], written as its cells are given.
///
/// A row that stops short of the last column is filled out with missing cells
/// and a cell past it is dropped, as in [`Schema::row`](crate::Schema::row).
pub struct Line<'a, W: Write> {
    schema: &'a Schema,
    widths: &'a Widths,
    scratch: &'a mut String,
    out: &'a mut W,
    i: usize,

    /// Spaces due before the next cell and not yet written.
    //
    // held back so a row ending in padding can be trimmed
    // once the last cell is known
    pending: usize,
}

impl<'a, W: Write> Line<'a, W> {
    /// Write a cell's text.
    pub fn text(&mut self, text: &str) -> std::io::Result<&mut Self> {
        self.put(text.as_bytes(), text.chars().count(), None)?;
        Ok(self)
    }

    /// Write a cell that is already bytes, one column per byte.
    ///
    /// Padding is by length, so this is for text of one byte per character.
    pub fn bytes(&mut self, text: &[u8]) -> std::io::Result<&mut Self> {
        self.put(text, text.len(), None)?;
        Ok(self)
    }

    /// Write a number, formatted by the column it falls in.
    pub fn num(&mut self, n: f64) -> std::io::Result<&mut Self> {
        let schema: &'a Schema = self.schema;

        // the scratch comes out of the line for as long as the
        // number is in it, since writing the cell takes the line
        // itself
        let mut buf = std::mem::take(self.scratch);
        match schema.columns.get(self.i) {
            Some(column) => column.format.write(n, &mut buf),
            None => buf.clear(),
        }

        let written = self.put(buf.as_bytes(), buf.len(), None);
        *self.scratch = buf;

        written?;
        Ok(self)
    }

    /// Write a cell of any kind, as [`Stream::row`] would write it.
    pub fn cell(&mut self, cell: impl Into<Cell>) -> std::io::Result<&mut Self> {
        let schema: &'a Schema = self.schema;
        let Some(column) = schema.columns.get(self.i) else {
            return Ok(self);
        };

        let cell = cell.into();
        let mut buf = std::mem::take(self.scratch);
        let align = cell.write(column, &schema.style, &mut buf);

        let written = self.put(buf.as_bytes(), buf.chars().count(), Some(align));
        *self.scratch = buf;

        written?;
        Ok(self)
    }

    /// Write the placeholder for a cell with no value.
    pub fn missing(&mut self) -> std::io::Result<&mut Self> {
        let schema: &'a Schema = self.schema;
        let missing = schema.style.missing.as_str();

        self.put(missing.as_bytes(), missing.chars().count(), None)?;
        Ok(self)
    }

    /// Fill out the columns left, and end the line.
    pub fn end(&mut self) -> std::io::Result<()> {
        while self.i < self.schema.columns.len() {
            self.missing()?;
        }

        if self.schema.style.trailing == Trailing::Keep {
            self.spaces()?;
        }

        self.out.write_all(b"\n")
    }

    /// Write one cell's text, padded on `align`'s side or else its column's.
    fn put(&mut self, text: &[u8], width: usize, align: Option<Align>) -> std::io::Result<()> {
        if self.i >= self.schema.columns.len() {
            return Ok(());
        }

        if self.i > 0 {
            self.pending += 1;
        }

        let pad = match self.schema.is_ragged(self.i) {
            true => 0,
            false => self
                .widths
                .as_slice()
                .get(self.i)
                .copied()
                .unwrap_or(0)
                .saturating_sub(width),
        };

        let align = align.unwrap_or(self.schema.columns[self.i].align);
        if align == Align::Right {
            self.pending += pad;
        }

        // an empty cell writes nothing, so the spaces in front
        // of it stay pending and a row of them can still be
        // trimmed
        if !text.is_empty() {
            self.spaces()?;
            self.out.write_all(text)?;
        }

        if align == Align::Left {
            self.pending += pad;
        }

        self.i += 1;
        Ok(())
    }

    /// Write the pending spaces.
    fn spaces(&mut self) -> std::io::Result<()> {
        const SPACES: [u8; 32] = [b' '; 32];

        let mut owed = std::mem::take(&mut self.pending);
        while owed > 0 {
            let n = owed.min(SPACES.len());
            self.out.write_all(&SPACES[..n])?;
            owed -= n;
        }

        Ok(())
    }
}
