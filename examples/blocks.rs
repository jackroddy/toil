//! A table written in pieces, as the work that fills it finishes.
//!
//! A column's width is not known until its last cell has arrived, which is a
//! problem for a table you want to watch rather than wait for. The way out is
//! to settle the widths in advance from what is already known -- here the step
//! names, which exist before any of them has run -- and hand that same
//! [`Widths`] to every block.
//!
//! Run it and the blocks arrive one at a time under a header printed before
//! any of them.

use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use toil::{Cell, Column, Header, Schema, Table};

/// One step's worth of finished work.
struct Block {
    step: &'static str,
    rows: Vec<(&'static str, f64, usize)>,
}

fn main() {
    let schema = Schema::new(vec![
        Column::new("step"),
        Column::new("shard"),
        Column::new("wall(s)").fixed(2),
        Column::new("hits"),
    ]);

    let blocks = [
        Block {
            step: "[1](fetch)",
            rows: vec![("pfam", 12.41, 0)],
        },
        Block {
            step: "[2](search-every-shard)",
            rows: vec![("0", 91.30, 4182), ("1", 94.02, 3907), ("2", 30.11, 1544)],
        },
        Block {
            step: "[3](report)",
            rows: vec![("-", 0.44, 9633)],
        },
    ];

    // the step names are known before anything runs, and so are
    // the headings above the numbers, which is enough to size
    // every column but not enough to be sure: a value wider
    // than what we guessed still takes the room it needs, and
    // only its own block shifts
    let known = Schema::new(vec![Column::new("step"), Column::new("shard")]);
    let names: Vec<_> = blocks
        .iter()
        .map(|block| known.row([block.step, "-"]))
        .collect();

    let mut floor = schema.widths();
    floor.max(&known.measure(&names));

    println!("one floor, shared -- the blocks arrive as their steps finish:\n");

    print!(
        "{}",
        Table::new(schema.clone()).render_with(&floor, Header::Show)
    );
    std::io::stdout().flush().unwrap();

    for block in &blocks {
        sleep(Duration::from_millis(400));
        print!(
            "{}",
            table(&schema, block).render_with(&floor, Header::Hide)
        );
        std::io::stdout().flush().unwrap();
    }

    println!("\nthe same blocks, each measured on its own:\n");

    for block in &blocks {
        let block = table(&schema, block);
        print!("{}", block.render_with(&block.widths(), Header::Hide));
    }
}

fn table(schema: &Schema, block: &Block) -> Table {
    let mut table = Table::new(schema.clone());
    let mut step = block.step;

    for &(shard, wall, hits) in &block.rows {
        table.row([
            Cell::from(step),
            Cell::from(shard),
            Cell::from(wall),
            Cell::from(hits),
        ]);

        // the step is named once, on the first of its rows
        step = "";
    }

    table
}
