//! A table written a row at a time to somewhere that cannot be revised.
//!
//! [`Stream`] takes its widths once and keeps them, because a line already on
//! its way to stdout cannot be widened to match a later one. So the widths are
//! an argument, and there are two answers to it.
//!
//! Ask the schema and you get the header's own widths, which is all that is
//! known before the first row arrives -- and which the first row is then very
//! likely to overrun. Hold a batch instead and you can measure it, at the cost
//! of keeping it in memory and of the file staying empty until you do.
//!
//! Run this to see both.

use std::io::{Write, stdout};

use toil::{Cell, Column, Row, Schema, Stream};

type Hit = (&'static str, u32, u32, f64, f64, f64);

const HITS: [Hit; 4] = [
    ("F1MV99|reviewed|Somatostatin", 58, 306, 175.8, 7.5, 1.0e-53),
    ("O08858|reviewed|Somatostatin", 54, 303, 173.0, 7.8, 7.7e-53),
    ("C3ZQF9|reviewed|QRFP-like", 64, 326, 149.3, 8.6, 1.3e-45),
    (
        "A0A0B4KGY7|unreviewed|Neuropeptide-receptor-like-31",
        51,
        316,
        145.0,
        4.3,
        2.6e-44,
    ),
];

fn schema() -> Schema {
    Schema::new(vec![
        Column::new("target"),
        Column::new("query"),
        Column::new("tstart"),
        Column::new("tend"),
        Column::new("score").fixed(1),
        Column::new("bias").fixed(1),
        Column::new("evalue").scientific(1),
    ])
}

fn cells(hit: &Hit) -> Vec<Cell> {
    let &(target, tstart, tend, score, bias, evalue) = hit;

    vec![
        Cell::from(target),
        Cell::from("7tm_1-consensus"),
        Cell::from(tstart),
        Cell::from(tend),
        Cell::from(score),
        Cell::from(bias),
        Cell::from(evalue),
    ]
}

fn main() {
    println!("widths from the header, which is all there is to go on:\n");
    from_the_header();

    println!("\nwidths measured from the batch first:\n");
    from_a_batch();
}

/// What a stream can do with nothing held back: every column is as wide as its
/// heading, and every value wider than that runs past it.
fn from_the_header() {
    let schema = schema();
    let widths = schema.widths();

    let mut out = Stream::new(schema, widths, stdout().lock());
    for hit in &HITS {
        out.row(cells(hit)).unwrap();
    }

    out.into_inner().flush().unwrap();
}

/// The same rows, measured before any of them is written. Nothing reaches the
/// file until the batch is in hand, which is the trade.
fn from_a_batch() {
    let schema = schema();
    let rows: Vec<Row> = HITS.iter().map(|hit| schema.row(cells(hit))).collect();
    let widths = schema.measure(&rows);

    let mut out = Stream::new(schema, widths, stdout().lock());
    for row in &rows {
        out.push(row).unwrap();
    }

    out.into_inner().flush().unwrap();
}
