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
        let text = match self.value {
            Value::Text(text) => text,
            Value::Num(n) => column.format.apply(n),
            Value::Missing => style.missing.clone(),
        };

        Text {
            text,
            align: self.align.unwrap_or(column.align),
            missing,
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

// note: only the floats reach `Format`. an integer has no precision to apply,
// and routing one through f64 would round the large ones
macro_rules! cell_from_num {
    ($($t:ty),*) => {$(
        impl From<$t> for Cell {
            fn from(n: $t) -> Cell {
                Cell { value: Value::Num(n as f64), align: None }
            }
        }
    )*};
}

macro_rules! cell_from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Cell {
            fn from(n: $t) -> Cell {
                Cell { value: Value::Text(n.to_string()), align: None }
            }
        }
    )*};
}

cell_from_num!(f32, f64);
cell_from_int!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);
