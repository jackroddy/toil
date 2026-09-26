//! Reading a table a line at a time, for text too long to hold whole.

use std::collections::VecDeque;
use std::io::BufRead;

use crate::meta::MetaRow;
use crate::read::{Masks, ParseError, Span, split};
use crate::render::Table;

/// A table read a line at a time.
///
/// Each row is split on spaces, as [`Table::parse`] splits it, the last
/// column taking the rest of the line. Only `#` lines are checked as UTF-8.
pub struct Reader<R: BufRead> {
    input: R,
    header: Table,
    masks: Masks,

    /// Lines read to settle the header and not yet handed out.
    sample: VecDeque<Vec<u8>>,

    buf: Vec<u8>,
    spans: Vec<Span>,
    line: usize,
}

/// One line of a table's body.
pub enum Entry<'a> {
    /// A row, cut into cells.
    Row(Cells<'a>),
    /// A `#=` line between the rows, read as a key and its words.
    Meta(MetaRow<'a>),
    /// Any other `#` line between the rows, `#` and all.
    Comment(&'a str),
}

/// The cells of one row, as written.
pub struct Cells<'a> {
    line: &'a [u8],
    spans: &'a [Span],
}

impl<'a> Cells<'a> {
    /// How many cells the line holds.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// Whether the line holds no cells at all.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// The text of cell `i` as written, the placeholder included, or `None`
    /// past the cells the line holds.
    pub fn field(&self, i: usize) -> Option<&'a [u8]> {
        let span = self.spans.get(i)?;
        Some(&self.line[span.start..span.end])
    }
}

impl<R: BufRead> Reader<R> {
    /// Read the lines above the rows, and enough rows to know how to cut them.
    ///
    /// # Errors
    ///
    /// Text that is not a table fails as [`std::io::ErrorKind::InvalidData`],
    /// as does a line above the rows that is not UTF-8.
    pub fn new(mut input: R) -> std::io::Result<Reader<R>> {
        let mut lead = Vec::new();
        let mut sample = VecDeque::new();

        // the first row is held back too, since it is what says which
        // marker the rows are written with
        loop {
            let mut buf = Vec::new();
            if !next_line(&mut input, &mut buf)? {
                break;
            }
            if !buf.starts_with(b"#") {
                sample.push_back(buf);
                break;
            }
            let text = String::from_utf8(buf)
                .map_err(|_| invalid(lead.len() + 1, "the line is not UTF-8"))?;
            lead.push(text);
        }

        let mut text = lead.join("\n");
        for line in &sample {
            text.push('\n');
            text.push_str(&String::from_utf8_lossy(line));
        }
        // lines() drops an empty last line, and an empty row is one
        text.push('\n');
        let mut header = Table::parse(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // the `#` lines between the rule and the first row came in
        // with the lead, and are the body's to hand out
        let before = header
            .interludes
            .iter()
            .take_while(|(at, _)| *at == 0)
            .count();
        for line in lead.drain(lead.len() - before..).rev() {
            sample.push_front(line.into_bytes());
        }
        header.rows.clear();
        header.interludes.clear();

        Ok(Reader {
            input,
            header,
            masks: Masks::default(),
            sample,
            buf: Vec::new(),
            spans: Vec::new(),
            line: lead.len(),
        })
    }

    /// The table above the rows: its preamble, columns and style, with no
    /// rows in it.
    pub fn header(&self) -> &Table {
        &self.header
    }

    /// The next line of the body with its number, counted from 1, or `None` at
    /// the end of the text.
    ///
    /// # Errors
    ///
    /// A failed read, or a `#` line that is not UTF-8, which fails as
    /// [`std::io::ErrorKind::InvalidData`].
    pub fn next_entry(&mut self) -> std::io::Result<Option<(usize, Entry<'_>)>> {
        match self.sample.pop_front() {
            Some(line) => self.buf = line,
            None => {
                if !next_line(&mut self.input, &mut self.buf)? {
                    return Ok(None);
                }
            }
        }
        self.line += 1;

        if self.buf.starts_with(b"#") {
            let text = std::str::from_utf8(&self.buf)
                .map_err(|_| invalid(self.line, "the line is not UTF-8"))?;
            let missing = self.header.schema.style.missing.as_str();
            let entry = match MetaRow::parse(text, missing) {
                Some(row) => Entry::Meta(row),
                None => Entry::Comment(text),
            };
            return Ok(Some((self.line, entry)));
        }

        let n = self.header.schema.columns.len();
        let reached = split(&self.buf, n, &mut self.spans, &mut self.masks);

        let cells = Cells {
            line: &self.buf,
            spans: &self.spans[..reached],
        };
        Ok(Some((self.line, Entry::Row(cells))))
    }
}

/// Read one line into `buf` without its line ending, or `false` at the end.
fn next_line(input: &mut impl BufRead, buf: &mut Vec<u8>) -> std::io::Result<bool> {
    buf.clear();
    if input.read_until(b'\n', buf)? == 0 {
        return Ok(false);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
    }
    Ok(true)
}

fn invalid(line: usize, reason: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, ParseError { line, reason })
}
