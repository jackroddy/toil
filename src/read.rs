//! Reading a table back: its cells, its labels, and the `#` lines around
//! them.
//!
//! A row is split on spaces, the last column taking the rest of the line, so
//! alignment, padding and overruns do not matter to it. A header word
//! belongs to the column it sits in, which the rule, or the header lines
//! themselves, say where each one starts.

use std::fmt;
use std::path::Path;

use crate::cell::{Align, Row, Text};
use crate::column::{Column, MARKER, Schema};
use crate::meta::MetaRow;
use crate::render::{INDENT, Table, needs_separator};
use crate::style::{Marker, Style, Trailing};

/// Why some text could not be read as a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Counted from 1, or 0 for a fault in the text as a whole.
    pub(crate) line: usize,
    pub(crate) reason: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            0 => f.write_str(self.reason),
            n => write!(f, "line {n}: {}", self.reason),
        }
    }
}

impl std::error::Error for ParseError {}

impl Table {
    /// Read a table back from text [`Table::render`] or a [`Stream`] wrote.
    ///
    /// The cells come back as written, a cell holding the placeholder as
    /// missing, along with the labels and the `#` lines, and the widths and
    /// marker that appending rows with [`Stream::continued`] needs. The rest
    /// of the style is not kept, so the result need not render the text it
    /// came from.
    ///
    /// [`Stream::continued`]: crate::Stream::continued
    ///
    /// [`Stream`]: crate::Stream
    pub fn parse(text: &str) -> Result<Table, ParseError> {
        Table::parse_missing(text, &Style::default().missing)
    }

    /// Read a table back as [`parse`](Self::parse) does, from text written
    /// with `missing` as its [placeholder](Style::missing).
    pub fn parse_missing(text: &str, missing: &str) -> Result<Table, ParseError> {
        let raw: Vec<&str> = text.lines().collect();
        let head = Head::find(&raw)?;
        let n = head.starts.len();

        let mut table = Table {
            schema: Schema::default(),
            preamble: head.preamble(&raw),
            rows: Vec::new(),
            interludes: Vec::new(),
        };
        let mut spans = Vec::new();
        let mut masks = Masks::default();
        for line in &raw[head.body_from..] {
            if line.starts_with('#') {
                table.interludes.push((table.rows.len(), line.to_string()));
                continue;
            }
            let reached = split(line.as_bytes(), n, &mut spans, &mut masks);
            let cells = (0..n)
                .map(|k| match k < reached {
                    true => {
                        let text = &line[spans[k].start..spans[k].end];
                        Text {
                            text: text.to_string(),
                            align: Align::Left,
                            missing: text == missing,
                        }
                    }
                    false => Text {
                        text: String::new(),
                        align: Align::Left,
                        missing: true,
                    },
                })
                .collect();
            table.rows.push(Row { cells });
        }

        let data: Vec<&str> = raw[head.body_from..]
            .iter()
            .filter(|l| !l.starts_with('#'))
            .copied()
            .collect();
        table.schema = head.schema(&raw, &data, missing);
        Ok(table)
    }

    /// Read the table at `path`, as [`parse`](Self::parse) reads text.
    ///
    /// Text that is not a table fails as [`std::io::ErrorKind::InvalidData`].
    pub fn read(path: impl AsRef<Path>) -> std::io::Result<Table> {
        Table::read_missing(path, &Style::default().missing)
    }

    /// Read the table at `path`, as [`parse_missing`](Self::parse_missing)
    /// reads text.
    pub fn read_missing(path: impl AsRef<Path>, missing: &str) -> std::io::Result<Table> {
        let text = std::fs::read_to_string(path)?;
        Table::parse_missing(&text, missing)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// The lines above the header, `#` and all.
    pub fn preamble(&self) -> &[String] {
        &self.preamble
    }

    /// The `#` lines between the rows, each with the number of rows above it.
    pub fn interludes(&self) -> &[(usize, String)] {
        &self.interludes
    }

    /// Each column's label, a stacked one's words joined by a space.
    pub fn labels(&self) -> &[String] {
        &self.schema.labels
    }

    /// Every `#=` line, above the header and then between the rows, in the
    /// order they were written.
    pub fn meta_rows(&self) -> impl Iterator<Item = MetaRow<'_>> {
        let missing = self.schema.style.missing.as_str();
        self.preamble
            .iter()
            .chain(self.interludes.iter().map(|(_, line)| line))
            .filter_map(move |line| MetaRow::parse(line, missing))
    }

    /// Which column carries `label`, as [`labels`](Self::labels) spells it.
    pub fn index(&self, label: &str) -> Option<usize> {
        self.schema.labels.iter().position(|l| l == label)
    }

    /// The text of row `row` under `label`, or `None` where there is no such
    /// row or column or the cell holds no value.
    pub fn get(&self, row: usize, label: &str) -> Option<&str> {
        self.rows.get(row)?.get(self.index(label)?)
    }

    /// The rows, in the order they were added or read.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
}

/// Where a table's header is, and where each of its columns starts.
struct Head {
    /// The first and last header lines.
    top: usize,
    bottom: usize,
    /// The first line under the header and the rule.
    body_from: usize,
    /// Where each column starts on the header lines, and under a dashed rule
    /// how many dashes it has.
    starts: Vec<usize>,
    dashes: Option<Vec<usize>>,
}

impl Head {
    /// Find the header of `raw`, the text's lines.
    fn find(raw: &[&str]) -> Result<Head, ParseError> {
        // what comes before the first row: the preamble, the
        // header, the rule, and any `#` line written between the
        // rule and the first row
        let lead = raw
            .iter()
            .position(|l| !l.starts_with('#'))
            .unwrap_or(raw.len());

        // the last rule-shaped line, since a preamble may draw one
        // of its own
        let rule_at = (0..lead).rev().find(|&i| is_rule(raw[i]));

        // the line directly above a rule is header whatever it holds,
        // since a header of empty labels is a bare `#` too; under no
        // rule it is the last run of label lines before the rows
        let bottom = match rule_at {
            Some(i) => i
                .checked_sub(1)
                .filter(|&b| raw[b].starts_with('#') && !raw[b].starts_with("#=")),
            None => (0..lead).rev().find(|&i| is_label_line(raw[i])),
        }
        .ok_or(ParseError {
            line: 0,
            reason: "no header naming the columns",
        })?;
        // a solid rule runs straight on from the `#`
        let runs = rule_at
            .filter(|&i| raw[i].as_bytes().get(1) != Some(&b'-'))
            .map(|i| dash_runs(raw[i]));

        // under a dashed rule a header line's words each start where a
        // column does, which a comment written straight above the header
        // need not
        let fits = |line: &str| match &runs {
            Some(runs) => words(line).all(|(at, _)| runs.iter().any(|&(start, _)| start == at)),
            None => true,
        };
        let mut top = bottom;
        while top > 0 && is_label_line(raw[top - 1]) && fits(raw[top - 1]) {
            top -= 1;
        }

        let (starts, dashes) = match runs {
            Some(runs) => (
                runs.iter().map(|&(start, _)| start).collect(),
                Some(runs.iter().map(|&(_, len)| len).collect()),
            ),
            None => {
                let mut starts: Vec<usize> = (top..=bottom)
                    .flat_map(|i| words(raw[i]).map(|(at, _)| at))
                    .collect();
                starts.sort_unstable();
                starts.dedup();
                (starts, None)
            }
        };
        if starts.is_empty() {
            return Err(ParseError {
                line: bottom + 1,
                reason: "the header names no columns",
            });
        }

        Ok(Head {
            top,
            bottom,
            body_from: rule_at.map_or(bottom + 1, |i| i + 1),
            starts,
            dashes,
        })
    }

    /// The lines above the header, less the bare `#` a render puts between a
    /// comment and the header, which a render puts back.
    fn preamble(&self, raw: &[&str]) -> Vec<String> {
        let mut preamble: Vec<String> = raw[..self.top].iter().map(|l| l.to_string()).collect();
        if let [.., before, last] = preamble.as_slice()
            && last == "#"
            && needs_separator(before)
        {
            preamble.pop();
        }
        preamble
    }

    /// The columns: their labels, the widths the rule or header gives them,
    /// and what appending rows needs of the style, with `data` the rows'
    /// lines.
    fn schema(&self, raw: &[&str], data: &[&str], missing: &str) -> Schema {
        let n = self.starts.len();

        // each word goes to the last column starting at or before it
        let mut labels: Vec<Vec<String>> = vec![Vec::new(); n];
        for line in &raw[self.top..=self.bottom] {
            for (at, word) in words(line) {
                let k = self.starts.iter().rposition(|&s| s <= at).unwrap_or(0);
                labels[k].push(word.to_string());
            }
        }

        let rows: Vec<&&str> = data.iter().filter(|l| !l.is_empty()).collect();
        let marker = match !rows.is_empty() && rows.iter().all(|l| l.starts_with(INDENT)) {
            true => Marker::Indent,
            false => Marker::Absorb,
        };
        let keep = (self.top..=self.bottom)
            .map(|i| raw[i])
            .chain(data.iter().copied())
            .any(|l| l.ends_with(' '));
        let style = Style::default()
            .marker(marker)
            .trailing(match keep {
                true => Trailing::Keep,
                false => Trailing::Trim,
            })
            .missing(missing);

        // a dashed rule gives every column's width; otherwise the
        // space to the next column's start does, bar the last. under
        // Absorb the first column takes the marker's width as well
        let absorb = match marker {
            Marker::Absorb => MARKER.len(),
            Marker::Indent => 0,
        };
        let widths: Vec<usize> = (0..n)
            .map(|k| {
                let width = match (&self.dashes, k + 1 < n) {
                    (Some(dashes), _) => dashes[k],
                    (None, true) => self.starts[k + 1] - self.starts[k] - 1,
                    (None, false) => labels[k]
                        .iter()
                        .map(|w| w.chars().count())
                        .max()
                        .unwrap_or(0),
                };
                match k == 0 {
                    true => width + absorb,
                    false => width,
                }
            })
            .collect();

        let columns: Vec<Column> = labels
            .into_iter()
            .zip(widths)
            .map(|(words, width)| Column::stacked(words).min_width(width))
            .collect();
        Schema::new(columns).style(style)
    }
}

/// Each run of dashes in a dashed rule after the marker: where it starts and
/// how long it is.
fn dash_runs(rule: &str) -> Vec<(usize, usize)> {
    let rule = rule.as_bytes();
    let mut runs = Vec::new();
    let mut pos = MARKER.len();

    loop {
        let start = pos;
        while pos < rule.len() && rule[pos] == b'-' {
            pos += 1;
        }
        runs.push((start, pos - start));

        if pos >= rule.len() {
            break;
        }
        pos += 1;
        if pos >= rule.len() {
            runs.push((pos, 0));
            break;
        }
    }

    runs
}

/// The words of a header line after its marker, each with where it starts.
fn words(line: &str) -> impl Iterator<Item = (usize, &str)> {
    let text = line.get(MARKER.len()..).unwrap_or_default();
    text.split(' ')
        .scan(MARKER.len(), |at, word| {
            let start = *at;
            *at += word.len() + 1;
            Some((start, word))
        })
        .filter(|(_, word)| !word.is_empty())
}

fn is_rule(line: &str) -> bool {
    line.strip_prefix('#')
        .is_some_and(|rest| rest.contains('-') && rest.chars().all(|c| c == '-' || c == ' '))
}

fn is_label_line(line: &str) -> bool {
    line.starts_with("# ") && !line.trim_end().eq("#") && !line.starts_with("#=")
}

/// A cell's place in a line, in bytes.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Split a row on spaces into `n` cells, the last taking the rest of the
/// line, and give back how many the line holds.
pub(crate) fn split(line: &[u8], n: usize, spans: &mut Vec<Span>, masks: &mut Masks) -> usize {
    spans.clear();
    let written = line.len() - line.iter().rev().take_while(|&&b| b == b' ').count();
    masks.fill(&line[..written]);
    let mut starts = Bits::new(&masks.starts);
    let mut ends = Bits::new(&masks.ends);

    for k in 0..n {
        let Some(start) = starts.next() else {
            break;
        };
        let end = match k + 1 == n {
            true => written,
            false => ends.next().unwrap_or(written),
        };
        spans.push(Span { start, end });
    }
    spans.len()
}

/// Where each run of bytes that are not spaces starts and ends in a line,
/// one bit a byte.
#[derive(Clone, Debug, Default)]
pub(crate) struct Masks {
    starts: Vec<u64>,
    ends: Vec<u64>,
}

impl Masks {
    fn fill(&mut self, line: &[u8]) {
        self.starts.clear();
        self.ends.clear();
        // whether the byte before this word is text, which
        // carries a run across the boundary
        let mut carry = 0;
        for word in line.chunks(64) {
            let mut space = spaces(word);
            if word.len() < 64 {
                space |= !0 << word.len();
            }
            let text = !space;
            let before = text << 1 | carry;
            self.starts.push(text & !before);
            self.ends.push(space & before);
            carry = text >> 63;
        }
        // a run the line ends inside ends at its end
        self.ends.push(carry);
    }
}

/// Which bytes of up to 64 are spaces, one bit a byte.
#[cfg(feature = "simd")]
fn spaces(word: &[u8]) -> u64 {
    use wide::u8x32;

    let spaces = u8x32::from(b' ');
    let mut space = 0;
    for (half, chunk) in word.chunks(32).enumerate() {
        let bytes: [u8; 32] = match chunk.try_into() {
            Ok(bytes) => bytes,
            Err(_) => {
                let mut bytes = [b' '; 32];
                bytes[..chunk.len()].copy_from_slice(chunk);
                bytes
            }
        };
        space |= u64::from(u8x32::from(bytes).simd_eq(spaces).to_bitmask()) << (half * 32);
    }
    space
}

/// Which bytes of up to 64 are spaces, one bit a byte.
//
// eight bytes at a time in a u64. a byte of x ^ SPACES is
// zero where x held a space, and the zero test leaves the
// top bit of each such byte set:
//    !(((y & LOW) + LOW) | y | LOW)
// the multiply then gathers the eight top bits into one byte
#[cfg(not(feature = "simd"))]
fn spaces(word: &[u8]) -> u64 {
    const LOW: u64 = 0x7F7F_7F7F_7F7F_7F7F;
    const SPACES: u64 = 0x2020_2020_2020_2020;
    const GATHER: u64 = 0x0102_0408_1020_4080;
    let flags = |x: u64| {
        let y = x ^ SPACES;
        (!((y & LOW).wrapping_add(LOW) | y | LOW) >> 7).wrapping_mul(GATHER) >> 56
    };

    let mut space = 0;
    let (chunks, rest) = word.as_chunks::<8>();
    let mut at = 0;
    for &chunk in chunks {
        space |= flags(u64::from_le_bytes(chunk)) << at;
        at += 8;
    }
    if !rest.is_empty() {
        let mut bytes = [b' '; 8];
        bytes[..rest.len()].copy_from_slice(rest);
        space |= flags(u64::from_le_bytes(bytes)) << at;
    }
    space
}

/// The set bits of a mask, lowest first, as byte positions.
struct Bits<'a> {
    words: &'a [u64],
    i: usize,
    word: u64,
}

impl<'a> Bits<'a> {
    fn new(words: &'a [u64]) -> Bits<'a> {
        Bits {
            words,
            i: 0,
            word: words.first().copied().unwrap_or(0),
        }
    }

    fn next(&mut self) -> Option<usize> {
        while self.word == 0 {
            self.i += 1;
            self.word = *self.words.get(self.i)?;
        }
        let at = self.word.trailing_zeros() as usize;
        self.word &= self.word - 1;
        Some(self.i * 64 + at)
    }
}
