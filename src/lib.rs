//! Padded text tables with a commented header, of the kind a program leaves
//! behind for someone to read later.
//!
//! ```text
//! # step     cmd   job wall(s) max_rss exit status argv
//! # -------- ----- --- ------- ------- ---- ------ ----
//! [1](setup) mkdir -   1.50    2.00MiB 0    ok     /mkdir -p out
//! [2](burn)  a     1   1.50    2.00MiB 0    ok     /a --jobs 4
//! ```
//!
//! Columns are padded to their widest cell, a missing value is written `-`, and the
//! header is commented so the file can be read by eye or split on whitespace.
//!
//! # Two ways to write one
//!
//! [`Table`] holds every row and lays them out once, which is what you want
//! when the data is in hand:
//!
//! ```
//! use toil::{Column, Schema, Table};
//!
//! let schema = Schema::new([
//!     Column::new("name"),
//!     Column::new("sens").fixed(4),
//! ]);
//!
//! let mut table = Table::new(schema);
//! table.comment("query 200 families");
//! table.row(["hmmer"]).row(["nail"]);
//! ```
//!
//! [`Stream`] writes rows as they arrive, to a file or a pipe that cannot be
//! revised afterwards. Its widths are settled up front and never change, so
//! there is no moment where a line has already gone out at the wrong width:
//!
//! ```
//! use toil::{Column, Schema, Stream};
//!
//! let schema = Schema::new([Column::new("target").min_width(40), Column::new("score")]);
//! let widths = schema.widths();
//!
//! let mut out = Stream::new(schema, widths, Vec::new());
//! out.row(["7tm_1", "175.8"]).unwrap();
//! ```
//!
//! # Sharing widths
//!
//! [`Widths`] is a value of its own, so tables written at different moments can
//! agree. Measure a floor from whatever is known in advance, print the header
//! once, and render each block against that floor as it finishes -- the blocks
//! line up under the one header, and drift apart only where a value turns out
//! wider than the label above it.
//!
//! ```
//! use toil::{Header, Schema, Table, Widths};
//!
//! let schema = Schema::new(["step", "wall(s)"]);
//! let floor: Widths = schema.widths();
//!
//! let mut first = Table::new(schema.clone());
//! first.row(["setup", "1.50"]);
//!
//! let mut out = first.render_with(&floor, Header::Show);
//!
//! let mut second = Table::new(schema);
//! second.row(["burn", "9.25"]);
//! out.push_str(&second.render_with(&floor, Header::Hide));
//! ```

#![warn(missing_docs)]

mod cell;
mod column;
mod meta;
mod read;
mod reader;
mod render;
mod stream;
mod style;

pub use cell::{Align, Cell, Row};
pub use column::{Column, Format, Schema};
pub use meta::MetaRow;
pub use read::ParseError;
pub use reader::{Cells, Entry, Reader};
pub use render::{Header, Table, Widths};
pub use stream::{Line, Stream};
pub use style::{Marker, Rule, Stack, Style, Trailing};
