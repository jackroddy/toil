//! What goes in a table: cells, and the rows they are written into.

use crate::column::Column;
use crate::style::Style;

/// Which side of a cell its padding goes on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Padding on the right, so the text sits against the left of its column.
    #[default]
    Left,
    /// Padding on the left, so the text sits against the right of its column.
    Right,
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Text(String),
    Num(f64),
    Num32(f32),
    Int(i64),
    Uint(u64),
    Missing,
}

/// One cell, before its column has formatted and aligned it.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    value: Value,
    align: Option<Align>,
}

impl Cell {
    /// A cell with no value, written as the style's placeholder.
    pub fn missing() -> Cell {
        Cell {
            value: Value::Missing,
            align: None,
        }
    }

    /// Pad this cell against its column's alignment rather than with it.
    pub fn align(mut self, align: Align) -> Cell {
        self.align = Some(align);
        self
    }

    pub(crate) fn resolve(self, column: &Column, style: &Style) -> Text {
        let missing = self.value == Value::Missing;
        let align = self.align.unwrap_or(column.align);
        let text = match self.value {
            Value::Text(text) => text,
            _ => {
                let mut text = String::new();
                self.write(column, style, &mut text);
                text
            }
        };

        Text {
            text,
            align,
            missing,
        }
    }

    /// Write the cell's text into `out`, and give back the side it is padded
    /// on.
    pub(crate) fn write(&self, column: &Column, style: &Style, out: &mut String) -> Align {
        out.clear();
        match &self.value {
            Value::Text(text) => out.push_str(text),
            Value::Num(n) => column.format.write(*n, out),
            Value::Num32(n) => column.format.write(*n, out),
            Value::Int(n) => write_int(*n, out),
            Value::Uint(n) => write_int(*n, out),
            Value::Missing => out.push_str(&style.missing),
        }

        self.align.unwrap_or(column.align)
    }

    /// Write the cell out as one word of a `#=` line, onto the end of `out`.
    pub(crate) fn word(&self, missing: &str, out: &mut String) {
        match &self.value {
            Value::Text(text) if !text.is_empty() => out.push_str(text),
            Value::Text(_) | Value::Missing => out.push_str(missing),
            Value::Num(n) => write_plain(*n, out),
            Value::Num32(n) => write_plain(*n, out),
            Value::Int(n) => write_int(*n, out),
            Value::Uint(n) => write_int(*n, out),
        }
    }
}

/// A cell that has been written out, with the padding it will be given.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Text {
    pub(crate) text: String,
    pub(crate) align: Align,
    /// Whether `text` is the placeholder for no value. Only [`Row::get`] looks.
    pub(crate) missing: bool,
}

impl Text {
    pub(crate) fn width(&self) -> usize {
        self.text.chars().count()
    }
}

/// A row of cells, already written out against a schema.
///
/// Build one with [`Schema::row`](crate::Schema::row) rather than directly, so
/// the columns can apply their formatting and alignment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub(crate) cells: Vec<Text>,
}

impl Row {
    /// The text of cell `i`, or `None` past the end of the row or where the
    /// cell holds no value.
    pub fn get(&self, i: usize) -> Option<&str> {
        self.cells
            .get(i)
            .filter(|cell| !cell.missing)
            .map(|cell| cell.text.as_str())
    }

    /// How many cells the row holds.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the row holds no cells at all.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Cell {
        Cell {
            value: Value::Text(text.to_string()),
            align: None,
        }
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Cell {
        Cell {
            value: Value::Text(text),
            align: None,
        }
    }
}

impl From<&String> for Cell {
    fn from(text: &String) -> Cell {
        Cell::from(text.as_str())
    }
}

impl<T: Into<Cell>> From<Option<T>> for Cell {
    fn from(value: Option<T>) -> Cell {
        value.map(Into::into).unwrap_or_else(Cell::missing)
    }
}

impl From<f64> for Cell {
    fn from(n: f64) -> Cell {
        Cell {
            value: Value::Num(n),
            align: None,
        }
    }
}

// kept apart from f64, since widening first would write
// 0.1f32 as 0.10000000149011612 under `Format::Plain`
impl From<f32> for Cell {
    fn from(n: f32) -> Cell {
        Cell {
            value: Value::Num32(n),
            align: None,
        }
    }
}

// note: only the floats reach `Format`. an integer has no
//       precision to apply, and routing one through f64
//       would round the large ones
macro_rules! cell_from_int {
    ($variant:ident, $wide:ty: $($t:ty),*) => {$(
        impl From<$t> for Cell {
            fn from(n: $t) -> Cell {
                Cell { value: Value::$variant(n as $wide), align: None }
            }
        }
    )*};
}

cell_from_int!(Uint, u64: u8, u16, u32, u64, usize);
cell_from_int!(Int, i64: i8, i16, i32, i64, isize);

fn write_plain(n: impl std::fmt::Display + std::fmt::LowerExp, out: &mut String) {
    let mut text = String::new();
    crate::column::Format::Plain.write(n, &mut text);
    out.push_str(&text);
}

fn write_int(n: impl std::fmt::Display, out: &mut String) {
    use std::fmt::Write;

    write!(out, "{n}").expect("a String accepts everything written to it");
}
