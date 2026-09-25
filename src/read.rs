//! Reading a table back from the text it was written as.
//!
//! The columns are recovered by position rather than by splitting on
//! whitespace, so an empty cell, a right-aligned one and a ragged column
//! anywhere in the row all come back as they went in, as does a last column
//! with spaces in it.

use std::cmp::Reverse;
use std::fmt;
use std::path::Path;

use crate::cell::{Align, Row, Text};
use crate::column::{Column, MARKER, Schema};
use crate::render::{INDENT, Table, Widths, line, needs_separator};
use crate::style::{Marker, Rule, Stack, Style, Trailing};

/// Why some text could not be read as a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Counted from 1, or 0 for a fault in the text as a whole.
    line: usize,
    reason: &'static str,
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
    /// The style and column widths come back with it, so the result renders
    /// the text it came from, bar a [`Stream`] cell that overran its column.
    /// A cell holding the placeholder reads back as missing, and a space reads
    /// back only in the last column's cells.
    ///
    /// [`Stream`]: crate::Stream
    pub fn parse(text: &str) -> Result<Table, ParseError> {
        Table::parse_missing(text, &Style::default().missing)
    }

    /// Read a table back as [`parse`](Self::parse) does, from text written
    /// with `missing` as its [placeholder](Style::missing).
    pub fn parse_missing(text: &str, missing: &str) -> Result<Table, ParseError> {
        let raw: Vec<&str> = text.lines().collect();
        let chars: Vec<Vec<char>> = raw.iter().map(|l| l.chars().collect()).collect();

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
        let rule = match rule_at {
            Some(i) if raw[i].as_bytes().get(1) == Some(&b'-') => Rule::Solid,
            Some(_) => Rule::Dashes,
            None => Rule::None,
        };

        // the lines that could be header, bottom first: `#` lines
        // directly above the rule, or under no rule the last run of
        // them before the rows, stopping at a bare `#` or a `#=`
        // line the line directly above a rule is header whatever it
        // holds, since a header of empty labels is a bare `#` too
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
        let mut top = bottom;
        while top > 0 && is_label_line(raw[top - 1]) {
            top -= 1;
        }

        let body_from = match rule_at {
            Some(i) => i + 1,
            None => bottom + 1,
        };
        let data: Vec<usize> = (body_from..raw.len())
            .filter(|&i| !raw[i].starts_with('#'))
            .collect();

        let columns = match rule_at {
            Some(i) if rule == Rule::Dashes => runs(&chars[i]),
            _ => starts_widths(&chars, top, bottom, rule_at, &data),
        };
        if columns.is_empty() {
            return Err(ParseError {
                line: bottom + 1,
                reason: "the header names no columns",
            });
        }

        let trailing = match (top..=bottom)
            .chain(data.iter().copied())
            .any(|i| raw[i].ends_with(' '))
        {
            true => Trailing::Keep,
            false => Trailing::Trim,
        };

        let reading = Reading {
            raw: &raw,
            chars: &chars,
            top,
            bottom,
            body_from,
            rule_at,
            rule,
            columns: &columns,
            data: &data,
            marker: Marker::Absorb,
            trailing,
            missing,
        };

        // a row that does not open with two spaces was written
        // under Absorb. otherwise either marker could have written
        // the rows, since under Absorb a row whose first cell is
        // empty or right-aligned opens with spaces too, so each is
        // tried and the one that explains the text better is kept:
        // Indent on a tie when every row is indented, and Absorb,
        // the default, when there are no rows to say
        let rows = data.iter().any(|&i| !chars[i].is_empty());
        let markers: &[Marker] = match rows {
            true if !data.iter().all(|&i| opens_indented(&chars[i])) => &[Marker::Absorb],
            true => &[Marker::Indent, Marker::Absorb],
            false => &[Marker::Absorb, Marker::Indent],
        };
        let mut solved: Option<(Reading, (Score, Vec<bool>))> = None;
        for &marker in markers {
            let reading = Reading { marker, ..reading };
            let solution = reading.solve();
            if solved.as_ref().is_none_or(|(_, best)| solution.0 > best.0) {
                solved = Some((reading, solution));
            }
        }
        let (reading, (score, ragged)) = solved.expect("at least one marker is tried");

        // a stream's overrun reads right as plain, though a render
        // widens it
        let ragged = match score.0 {
            true => ragged,
            false => vec![false; columns.len()],
        };
        let built = reading.build(&ragged);

        // the search above settles nearly every table, and what
        // settles it is the table laying out again as written. when
        // it does not, every answer is tried in turn, fewest ragged
        // columns first, for tables narrow enough that trying them
        // all is cheap. text no answer reproduces, from some other
        // writer, keeps the search's
        let settled = built
            .as_ref()
            .is_some_and(|(t, first)| *first == top && reproduces(t, &raw, *first));
        let built = match settled || columns.len() > EVERY_ANSWER_UP_TO {
            true => built,
            false => every_answer(reading, markers).or(built),
        };

        let (mut table, _) = built.ok_or(ParseError {
            line: bottom + 1,
            reason: "the header does not line up with the rule",
        })?;

        // rows that do not sit under the rule even at its own
        // widths, as a stream's overrun does, came from some other
        // writer, and the most that can be done with them is to
        // split them on whitespace
        if !rows_as_ruled(&table, &raw, &data) {
            table.rows = data.iter().map(|&i| split(raw[i], &table)).collect();
        }

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

    /// The text of each `#=` line above the header, without the `#=`.
    pub fn meta_lines(&self) -> impl Iterator<Item = &str> {
        self.preamble
            .iter()
            .filter_map(|line| line.strip_prefix("#="))
            .map(|text| text.strip_prefix(' ').unwrap_or(text))
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

/// How well an answer to which columns are ragged explains the text.
//
// in order: whether the rows lay out again as written, and
// if not, how far along them the first column showing
// otherwise is; then how much of the header, from the
// bottom line up, cuts cleanly. two answers can explain the
// same text, and then the likelier is the one asking less
// out of the ordinary: fewer plain columns wider than
// anything in them, which only a minimum width explains,
// and then fewer cells with a space in them
//
// some tables write the same bytes as another: an empty
// word in a stacked label writes nothing, and nor does an
// empty cell in a ragged column or an empty last column.
// the answer kept then renders the same text, but need not
// be the table that wrote it
type Score = (bool, usize, usize, Reverse<usize>, Reverse<usize>);

/// The text being read, with what is settled about it before anything is
/// worked out.
#[derive(Clone, Copy)]
struct Reading<'a> {
    raw: &'a [&'a str],
    chars: &'a [Vec<char>],
    /// The first and last lines that could be header.
    top: usize,
    bottom: usize,
    /// The first line under the header and the rule.
    body_from: usize,
    rule_at: Option<usize>,
    rule: Rule,
    /// Each column's width as the header lays it out, before any is known to
    /// be ragged.
    columns: &'a [usize],
    /// The lines that are rows.
    data: &'a [usize],
    marker: Marker,
    trailing: Trailing,
    missing: &'a str,
}

impl Reading<'_> {
    fn n(&self) -> usize {
        self.columns.len()
    }

    fn style(&self) -> Style {
        Style::default()
            .marker(self.marker)
            .rule(self.rule)
            .trailing(self.trailing)
            .missing(self.missing)
    }

    fn keep(&self) -> bool {
        self.trailing == Trailing::Keep
    }

    /// The header, from the bottom line up, for as long as a line cuts cleanly
    /// against the columns, and the line it starts on. Anything above it is
    /// preamble.
    fn read_header(
        &self,
        widths: &[usize],
        ragged: &[bool],
        last: Option<Last>,
    ) -> (Vec<Vec<String>>, usize) {
        let mut header = Vec::new();
        let mut first = self.bottom + 1;
        for i in (self.top..=self.bottom).rev() {
            match words(&self.chars[i], widths, ragged, last) {
                Ok(words) => {
                    header.insert(0, words);
                    first = i;
                }
                Err(_) => break,
            }
        }
        (header, first)
    }

    /// Each column's width as the header lays it out.
    ///
    /// A solid rule spans the columns with a ragged one counted at its label's
    /// width, and the last column is whatever of it is left.
    fn header_widths(&self, ragged: &[bool]) -> Vec<usize> {
        let n = self.n();
        let mut widths = self.columns.to_vec();

        if let Some(at) = self.rule_at.filter(|_| self.rule == Rule::Solid) {
            let (header, _) = self.read_header(&widths, ragged, None);
            let label = |k: usize| {
                header
                    .iter()
                    .map(|words| words[k].chars().count())
                    .max()
                    .unwrap_or(0)
            };
            let before: usize = (0..n - 1)
                .map(|k| 1 + if ragged[k] { label(k) } else { widths[k] })
                .sum();

            // the marker is inside the span unless a ragged first
            // column is counted at its label alone
            let open = match self.marker == Marker::Absorb && ragged[0] {
                true => 0,
                false => MARKER.len(),
            };
            widths[n - 1] = self.chars[at].len().saturating_sub(open + before);
        }

        widths
    }

    /// Each column's width as the rows lay it out, where under Absorb a plain
    /// first column also holds the marker.
    ///
    /// Under no rule nothing says how wide the last column is but the rows,
    /// each of which ends it where it pleases once a ragged column has moved
    /// it along.
    fn data_widths(&self, ragged: &[bool], pieces: &[Vec<Piece>]) -> Vec<usize> {
        let n = self.n();
        let mut widths = self.header_widths(ragged);

        if self.rule == Rule::None {
            // a row's span runs from where the row opens, which for a
            // sole column under Absorb is two before its header word
            let behind = match n == 1 && self.marker == Marker::Absorb {
                true => MARKER.len(),
                false => 0,
            };
            let (header, _) = self.read_header(&widths, ragged, None);
            widths[n - 1] = header
                .iter()
                .map(|words| words[n - 1].chars().count())
                .chain(pieces.iter().map(|p| p[n - 1].span.saturating_sub(behind)))
                .max()
                .unwrap_or(0);
        }

        if self.marker == Marker::Absorb && !ragged[0] {
            widths[0] += MARKER.len();
        }
        widths
    }

    /// The last column's width as the header lays it out, which is what a
    /// plain one holds its header words to.
    fn last(&self, ragged: &[bool], pieces: &[Vec<Piece>]) -> Option<Last> {
        let mut width = self.data_widths(ragged, pieces)[self.n() - 1];
        if self.n() == 1 && self.marker == Marker::Absorb && !ragged[0] {
            width -= MARKER.len();
        }
        Some(Last {
            width,
            keep: self.keep(),
        })
    }

    /// Every row cut into pieces. The last column takes the rest of the line,
    /// so its width plays no part and none is given.
    fn cut_all(&self, ragged: &[bool]) -> Vec<Vec<Piece>> {
        let widths = self.data_widths(ragged, &[]);
        let open = match self.marker {
            Marker::Absorb => 0,
            Marker::Indent => INDENT.len(),
        };
        self.data
            .iter()
            .map(|&i| cut(&self.chars[i], open, &widths, ragged))
            .collect()
    }

    /// Whether rendering these cells lays every row out again exactly as it
    /// was written, with the widths a render would measure. A plain column is
    /// as wide as its rule, so a cell wider than that says ragged too.
    fn fits(&self, ragged: &[bool], pieces: &[Vec<Piece>]) -> bool {
        let n = self.n();
        let ruled = self.data_widths(ragged, pieces);
        let schema = with_widths(shape(n, ragged, &self.style(), &[]), &ruled, ragged);
        let rows: Vec<Row> = pieces.iter().map(|p| Row { cells: cells(p) }).collect();
        let widths: Widths = schema.measure(&rows);

        let as_ruled = (0..n).all(|k| ragged[k] || widths.as_slice()[k] == ruled[k]);
        let lead = match self.marker {
            Marker::Absorb => "",
            Marker::Indent => INDENT,
        };
        as_ruled
            && self.data.iter().zip(&rows).all(|(&i, row)| {
                let written = line(&schema, &widths, lead, &row.cells);
                written.strip_suffix('\n') == Some(self.raw[i])
            })
    }

    /// Whether a row shows the last column is not plain: a plain one is as
    /// wide as its widest cell, and under Keep every row is padded out to it.
    fn unpadded(&self, ragged: &[bool], pieces: &[Vec<Piece>]) -> bool {
        let n = self.n();
        let width = self.data_widths(ragged, pieces)[n - 1];
        pieces
            .iter()
            .any(|p| p[n - 1].span > width || (self.keep() && p[n - 1].span < width))
    }

    fn score(&self, ragged: &[bool]) -> Score {
        let n = self.n();
        let pieces = self.cut_all(ragged);
        let fit = self.fits(ragged, &pieces);
        let rows = match fit {
            true => n + 1,
            false => (0..n)
                .find(|&k| {
                    !ragged[k]
                        && match k + 1 == n {
                            true => self.unpadded(ragged, &pieces),
                            false => pieces.iter().any(|p| p[k].short),
                        }
                })
                .unwrap_or(n),
        };

        let widths = self.header_widths(ragged);
        let last = self.last(ragged, &pieces);
        let mut header = 0;
        let mut labelled = vec![0; n];
        for i in (self.top..=self.bottom).rev() {
            match words(&self.chars[i], &widths, ragged, last) {
                Ok(words) => {
                    header += n + 1;
                    for (k, word) in words.iter().enumerate() {
                        labelled[k] = labelled[k].max(word.chars().count());
                    }
                }
                Err(at) => {
                    header += at;
                    break;
                }
            }
        }

        // a rule draws a ragged column at its label's width: a
        // dashed rule each one, a solid rule in its span, which
        // ends with the last column. the labels are only known once
        // the whole header cuts
        let whole = header == (self.bottom - self.top + 1) * (n + 1);
        let ruled = !whole
            || (0..n).filter(|&k| ragged[k]).all(|k| match self.rule {
                Rule::Dashes => widths[k] == labelled[k],
                Rule::Solid => k + 1 < n || widths[k] == labelled[k],
                Rule::None => true,
            });

        // plain columns wider than their label and cells call for,
        // which only a minimum width set by hand explains
        let laid = self.data_widths(ragged, &pieces);
        let slack = (0..n)
            .filter(|&k| !ragged[k])
            .filter(|&k| {
                let marker = match k == 0 && self.marker == Marker::Absorb {
                    true => MARKER.len(),
                    false => 0,
                };
                let cells = pieces
                    .iter()
                    .map(|p| p[k].text.chars().count())
                    .max()
                    .unwrap_or(0);
                laid[k] > (labelled[k] + marker).max(cells)
            })
            .count();

        // cells with a space in them, which only the last column
        // holds
        let spaced = pieces
            .iter()
            .filter(|p| p[n - 1].text.contains(' '))
            .count();

        (fit && ruled, rows, header, Reverse(slack), Reverse(spaced))
    }

    /// The table these columns make of the text, and the line its header
    /// starts on, or `None` if no line of the header cuts against them.
    fn build(&self, ragged: &[bool]) -> Option<(Table, usize)> {
        let n = self.n();
        let pieces = self.cut_all(ragged);
        let (header, first_header) = self.read_header(
            &self.header_widths(ragged),
            ragged,
            self.last(ragged, &pieces),
        );
        if header.is_empty() {
            return None;
        }

        let (labels, stack) = labels(&header, n);
        let style = self.style().stack(stack);
        let schema = shape(n, ragged, &style, &labels);
        let schema = with_widths(schema, &self.data_widths(ragged, &pieces), ragged);

        let mut rows = Vec::new();
        let mut interludes = Vec::new();
        let mut pieces = pieces.into_iter();
        for i in self.body_from..self.raw.len() {
            match self.raw[i].starts_with('#') {
                true => interludes.push((rows.len(), self.raw[i].to_string())),
                false => {
                    let cut = pieces.next().unwrap_or_default();
                    let mut cells = cells(&cut);
                    for cell in &mut cells {
                        cell.missing = cell.text == style.missing;
                    }
                    rows.push(Row { cells });
                }
            }
        }

        // the bare `#` a render puts between a comment and the
        // header is the format's, not the preamble's, and a render
        // puts it back
        let mut preamble: Vec<String> = self.raw[..first_header]
            .iter()
            .map(|l| l.to_string())
            .collect();
        if let [.., before, last] = preamble.as_slice()
            && last == "#"
            && needs_separator(before)
        {
            preamble.pop();
        }

        let table = Table {
            schema,
            preamble,
            rows,
            interludes,
        };
        Some((table, first_header))
    }

    /// Which columns are ragged, and how well that explains the text.
    ///
    /// Start with none, and make ragged whichever column explains the text
    /// best, for as long as one explains it better. A ragged column can read
    /// as plain by chance and throw the columns after it off, so the column a
    /// row or header line first goes wrong at need not be the one to blame.
    fn solve(&self) -> (Score, Vec<bool>) {
        let n = self.n();
        let whole = (self.bottom - self.top + 1) * (n + 1);
        let perfect = (true, n + 1, whole, Reverse(0), Reverse(0));

        let mut ragged = vec![false; n];
        let mut current = self.score(&ragged);
        while current != perfect {
            // ragged columns whose words are empty on a line each move
            // the rest of it by one, and it can take several at once
            // before the line cuts any further, so sets of up to three
            // are weighed together, the smaller and then the leftmost
            // winning a tie. one column that explains everything ends
            // the search early
            let mut best: Option<(Score, Reverse<usize>, Reverse<Vec<usize>>)> = None;
            'sizes: for size in 1..=MOST_AT_ONCE {
                for set in subsets(&ragged, size) {
                    let mut r = ragged.clone();
                    for &j in &set {
                        r[j] = true;
                    }
                    let candidate = (self.score(&r), Reverse(size), Reverse(set));
                    let done = candidate.0 == perfect;
                    if best.as_ref().is_none_or(|b| candidate > *b) {
                        best = Some(candidate);
                    }
                    if done {
                        break 'sizes;
                    }
                }
            }

            match best {
                Some((score, _, Reverse(set))) if score > current => {
                    for j in set {
                        ragged[j] = true;
                    }
                    current = score;
                }
                _ => break,
            }
        }

        (current, ragged)
    }
}

/// The most columns [`Reading::solve`] makes ragged in one step.
const MOST_AT_ONCE: usize = 3;

/// The widest table [`every_answer`] tries every answer for.
const EVERY_ANSWER_UP_TO: usize = 12;

/// The table that lays out again exactly as written, with the fewest ragged
/// columns, under any of `markers`, and the line its header starts on. Of
/// several with as few, the one [`Reading::score`] ranks highest.
///
/// An answer whose header takes every line that could be header comes first,
/// and only without one does a comment line above the header fall to the
/// preamble, which is what text from before the bare `#` separator needs.
fn every_answer(reading: Reading, markers: &[Marker]) -> Option<(Table, usize)> {
    let n = reading.n();
    for whole in [true, false] {
        for size in 0..=n {
            let mut best: Option<(Score, Table, usize)> = None;
            for &marker in markers {
                let reading = Reading { marker, ..reading };
                for set in subsets(&vec![false; n], size) {
                    let mut ragged = vec![false; n];
                    for j in set {
                        ragged[j] = true;
                    }
                    let Some((table, first)) = reading.build(&ragged) else {
                        continue;
                    };
                    if (whole && first != reading.top) || !reproduces(&table, reading.raw, first) {
                        continue;
                    }
                    let score = reading.score(&ragged);
                    if best.as_ref().is_none_or(|(b, _, _)| score > *b) {
                        best = Some((score, table, first));
                    }
                }
            }
            if let Some((_, table, first)) = best {
                return Some((table, first));
            }
        }
    }
    None
}

/// Whether each of `table`'s rows, laid out at the widths its columns were
/// read at, is the line in `raw` it was read from.
fn rows_as_ruled(table: &Table, raw: &[&str], data: &[usize]) -> bool {
    let schema = &table.schema;
    let widths = Widths(schema.columns.iter().map(|c| c.min_width).collect());
    let lead = match schema.style.marker {
        Marker::Absorb => "",
        Marker::Indent => INDENT,
    };
    data.iter().zip(&table.rows).all(|(&i, row)| {
        let written = line(schema, &widths, lead, &row.cells);
        written.strip_suffix('\n') == Some(raw[i])
    })
}

/// A row split on whitespace against `table`'s columns, the last taking the
/// rest of the line, and the placeholder read as missing.
fn split(line: &str, table: &Table) -> Row {
    let n = table.schema.columns.len();
    let missing = &table.schema.style.missing;
    let mut rest = line.trim_start();
    let mut cells = Vec::with_capacity(n);

    for k in 0..n {
        let text = match k + 1 == n {
            true => std::mem::take(&mut rest).trim_end(),
            false => {
                let end = rest.find(' ').unwrap_or(rest.len());
                let (word, after) = rest.split_at(end);
                rest = after.trim_start();
                word
            }
        };
        cells.push(Text {
            text: text.to_string(),
            align: Align::Left,
            missing: text == missing,
        });
    }

    Row { cells }
}

/// Whether `table` renders the lines from `first` on exactly as `raw` holds
/// them. What is above the header is the preamble's and is not compared,
/// since text from before the bare `#` separator existed would fail on that
/// alone.
fn reproduces(table: &Table, raw: &[&str], first: usize) -> bool {
    let separated = table.preamble.last().is_some_and(|l| needs_separator(l));
    let above = table.preamble.len() + usize::from(separated);
    let rendered = table.render();
    rendered
        .lines()
        .skip(above)
        .eq(raw[first..].iter().copied())
}

/// Every set of `size` columns not yet ragged, in ascending order.
fn subsets(ragged: &[bool], size: usize) -> Vec<Vec<usize>> {
    let free: Vec<usize> = (0..ragged.len()).filter(|&j| !ragged[j]).collect();
    let mut sets = Vec::new();
    let mut set = Vec::with_capacity(size);

    fn extend(free: &[usize], size: usize, set: &mut Vec<usize>, sets: &mut Vec<Vec<usize>>) {
        if set.len() == size {
            sets.push(set.clone());
            return;
        }
        for (i, &j) in free.iter().enumerate() {
            set.push(j);
            extend(&free[i + 1..], size, set, sets);
            set.pop();
        }
    }

    extend(&free, size, &mut set, &mut sets);
    sets
}

/// A cell as it was cut out of a line.
#[derive(Clone, Debug, Default)]
struct Piece {
    text: String,
    /// Padding came before the text, so the cell was right-aligned.
    right: bool,
    /// Whether the cell's width differs from its column's.
    //
    // it stopped short with the next cell straight after it,
    // or ran past. a ragged column does both, and a stream's
    // overrun does the second
    short: bool,
    /// How much of the line the last column took, padding and all.
    span: usize,
}

fn cells(pieces: &[Piece]) -> Vec<Text> {
    pieces
        .iter()
        .map(|p| Text {
            text: p.text.clone(),
            align: match p.right {
                true => Align::Right,
                false => Align::Left,
            },
            missing: false,
        })
        .collect()
}

/// Cut a row into one piece per column, starting at `pos`.
///
/// A ragged column's cell runs to the next space, and so does a cell wider
/// than its column, which a stream writes when a value overruns. The last
/// column takes the rest of the line, spaces and all.
fn cut(line: &[char], mut pos: usize, widths: &[usize], ragged: &[bool]) -> Vec<Piece> {
    let n = widths.len();
    let mut pieces = Vec::with_capacity(n);

    for k in 0..n {
        if pos >= line.len() {
            pieces.push(Piece::default());
            continue;
        }

        if k + 1 == n {
            let rest: String = line[pos..].iter().collect();
            let text = rest.trim().to_string();
            pieces.push(Piece {
                right: !ragged[k] && rest.starts_with(' ') && !text.is_empty(),
                text,
                short: false,
                span: line.len() - pos,
            });
            break;
        }

        let end = pos + widths[k];
        let fills = end >= line.len() || line[end] == ' ';
        let seg: String = line[pos..end.min(line.len())].iter().collect();
        let spaced = seg.trim().contains(' ');

        match !ragged[k] && fills && !spaced {
            true => {
                let text = seg.trim().to_string();
                pieces.push(Piece {
                    right: seg.starts_with(' ') && !text.is_empty(),
                    text,
                    short: false,
                    span: 0,
                });
                pos = end + 1;
            }
            // a ragged cell, or one that overran: the text up to the
            // next space
            false => {
                let len = line[pos..].iter().take_while(|&&c| c != ' ').count();
                pieces.push(Piece {
                    text: line[pos..pos + len].iter().collect(),
                    right: false,
                    short: true,
                    span: 0,
                });
                pos += len + 1;
            }
        }
    }

    pieces
}

/// What a plain last column holds a header line to, once its width is known.
#[derive(Clone, Copy, Debug)]
struct Last {
    width: usize,
    /// The lines keep their padding, so the column is padded out to `width`.
    keep: bool,
}

/// A header line's word for each column, or the column where the line stops
/// cutting cleanly: a word that does not start where its column does, or runs
/// past it. A line that is not a header line at all fails at column 0.
fn words(
    line: &[char],
    widths: &[usize],
    ragged: &[bool],
    last: Option<Last>,
) -> Result<Vec<String>, usize> {
    // a line of empty labels, trimmed
    if line == ['#'] && !last.is_some_and(|l| l.keep) {
        return Ok(vec![String::new(); widths.len()]);
    }
    if line.len() < MARKER.len() || line[..MARKER.len()] != ['#', ' '] {
        return Err(0);
    }

    let n = widths.len();
    let mut words = Vec::with_capacity(n);
    let mut pos = MARKER.len();

    for k in 0..n {
        // the last column's word is the rest of the line, however
        // wide, and a plain one is padded out to its width where
        // the lines keep padding
        if k + 1 == n {
            // a label's words hold no spaces, in the last column as
            // anywhere
            let rest: String = line.get(pos..).unwrap_or_default().iter().collect();
            if rest.starts_with(' ') && !rest.trim().is_empty() || rest.trim().contains(' ') {
                return Err(k);
            }
            // and a ragged one is never padded
            if ragged[k] && rest.trim().len() != rest.len() {
                return Err(k);
            }
            if let Some(Last { width, keep }) = last.filter(|_| !ragged[k]) {
                let len = rest.chars().count();
                if len > width || (keep && len < width) {
                    return Err(k);
                }
            }
            words.push(rest.trim().to_string());
            return Ok(words);
        }

        if pos >= line.len() {
            // where padding is kept, every column before the last is
            // written
            if last.is_some_and(|l| l.keep) {
                return Err(k);
            }
            words.push(String::new());
            continue;
        }

        let len = line[pos..].iter().take_while(|&&c| c != ' ').count();
        if !ragged[k] && len > widths[k] {
            return Err(k);
        }
        words.push(line[pos..pos + len].iter().collect());

        pos = match ragged[k] {
            true => pos + len + 1,
            false => {
                let end = pos + widths[k];
                if line[pos + len..end.min(line.len())]
                    .iter()
                    .any(|&c| c != ' ')
                {
                    return Err(k);
                }
                if end < line.len() && line[end] != ' ' {
                    return Err(k);
                }
                // padding that is kept is written out, and so is the space
                // before the next column
                if last.is_some_and(|l| l.keep) && end >= line.len() {
                    return Err(k);
                }
                end + 1
            }
        };
    }

    Ok(words)
}

/// The columns' labels from the header's words, and which end they hang from.
fn labels(header: &[Vec<String>], n: usize) -> (Vec<Vec<String>>, Stack) {
    let lines = header.len();

    // the first and last header lines each column has a word on
    let spans: Vec<Option<(usize, usize)>> = (0..n)
        .map(|k| {
            let mut used = (0..lines).filter(|&i| !header[i][k].is_empty());
            let first = used.next()?;
            Some((first, used.next_back().unwrap_or(first)))
        })
        .collect();

    let labels = spans
        .iter()
        .enumerate()
        .map(|(k, span)| match span {
            Some((first, last)) => (*first..=*last).map(|i| header[i][k].clone()).collect(),
            None => vec![String::new()],
        })
        .collect();

    // a label short of the header's depth says which end it
    // hangs from
    let top = spans
        .iter()
        .flatten()
        .any(|&(first, last)| last - first + 1 < lines && last + 1 != lines);
    let stack = match top {
        true => Stack::Top,
        false => Stack::Bottom,
    };
    (labels, stack)
}

/// A schema of `n` columns in `style`, labelled if the labels are known yet.
fn shape(n: usize, ragged: &[bool], style: &Style, labels: &[Vec<String>]) -> Schema {
    let columns = (0..n).map(|k| {
        let label = labels
            .get(k)
            .cloned()
            .unwrap_or_else(|| vec![String::new()]);
        let column = Column::stacked(label);
        match ragged[k] {
            true => column.ragged(),
            false => column,
        }
    });
    Schema::new(columns.collect::<Vec<Column>>()).style(style.clone())
}

/// Hold each plain column at the width it was read at, so a render lays it out
/// as it was.
fn with_widths(mut schema: Schema, widths: &[usize], ragged: &[bool]) -> Schema {
    for (k, column) in schema.columns.iter_mut().enumerate() {
        if !ragged[k] {
            column.min_width = widths[k];
        }
    }
    schema
}

/// Each column's width, from the runs of a dashed rule.
///
/// Under [`Marker::Absorb`] the first run is two short of the first column,
/// the marker having taken its place, and the widths here leave it out.
fn runs(rule: &[char]) -> Vec<usize> {
    let mut runs = Vec::new();
    let mut pos = MARKER.len();

    loop {
        let start = pos;
        while pos < rule.len() && rule[pos] == '-' {
            pos += 1;
        }
        runs.push(pos - start);

        if pos >= rule.len() {
            break;
        }
        pos += 1;
        if pos >= rule.len() {
            runs.push(0);
            break;
        }
    }

    runs
}

/// Each column's width under a solid rule or none, from where the words of
/// the header start.
///
/// The line with the most words sets the columns. The last column runs to the
/// end of a solid rule, or under none to the end of the longest line; both are
/// settled properly once the ragged columns are known.
fn starts_widths(
    chars: &[Vec<char>],
    top: usize,
    bottom: usize,
    rule_at: Option<usize>,
    data: &[usize],
) -> Vec<usize> {
    let starts = |line: &[char]| -> Vec<usize> {
        (MARKER.len()..line.len())
            .filter(|&i| line[i] != ' ' && (i == MARKER.len() || line[i - 1] == ' '))
            .collect()
    };

    let reference = (top..=bottom)
        .rev()
        .max_by_key(|&i| starts(&chars[i]).len())
        .unwrap_or(bottom);
    let starts = starts(&chars[reference]);

    let end = match rule_at {
        Some(i) => chars[i].len(),
        None => (top..=bottom)
            .chain(data.iter().copied())
            .map(|i| chars[i].len())
            .max()
            .unwrap_or(0),
    };

    let mut widths: Vec<usize> = starts.windows(2).map(|w| w[1] - w[0] - 1).collect();
    if let Some(&last) = starts.last() {
        widths.push(end.saturating_sub(last));
    }
    widths
}

/// Whether a line is a rule: `#` and then nothing but dashes and spaces.
fn is_rule(line: &str) -> bool {
    line.strip_prefix('#')
        .is_some_and(|rest| rest.contains('-') && rest.chars().all(|c| c == '-' || c == ' '))
}

/// Whether a line could be part of a header: a comment with something in it,
/// and not a `#=` line.
fn is_label_line(line: &str) -> bool {
    line.starts_with("# ") && !line.trim_end().eq("#") && !line.starts_with("#=")
}

/// Whether a row opens with the two spaces [`Marker::Indent`] gives it.
fn opens_indented(line: &[char]) -> bool {
    line.is_empty() || line.starts_with(&[' ', ' '])
}
