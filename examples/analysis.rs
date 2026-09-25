//! An analysis table: numbers, and a block above them saying what they are
//! numbers of.
//!
//! The marker offsets every line here rather than eating into the first
//! column, and the padding on the last column is kept, which is what a table
//! meant to sit beside others written the same way needs.
//!
//! Two kinds of line above the header. `#` is for a reader; `#=` is for a
//! parser, and reading them back is how a later stage recovers what this run
//! was.

use toil::{Cell, Column, Marker, Schema, Style, Table, Trailing};

fn main() {
    let schema = Schema::new(vec![
        Column::new("name"),
        Column::new("tool"),
        Column::new("E"),
        Column::new("wall_s").fixed(4),
        Column::new("found"),
        Column::new("hits"),
        Column::new("sens").fixed(4),
        Column::new("params").ragged(),
    ])
    .style(
        Style::default()
            .marker(Marker::Indent)
            .trailing(Trailing::Keep),
    );

    let mut table = Table::new(schema);

    table.comment("query        200 families         38061 residues      17922777 bytes");
    table.comment("target      1000 seqs            194987 residues        220417 bytes");
    table.comment("pairs        261 rows                 3 runs");
    table.comment("");
    table.meta("run hmmer hmmer 0.2600");
    table.meta("run nail nail 0.1200 E=1000000");
    table.meta("run mmseqs mmseqs 0.4100 s=7.5");

    let runs = [
        ("hmmer", "hmmer", None, 0.26, 43, 43, 1.0, ""),
        (
            "nail",
            "nail",
            Some("1000000"),
            0.12,
            33,
            32,
            0.7442,
            "-A 10 -B 12",
        ),
        (
            "mmseqs",
            "mmseqs",
            Some("1000"),
            0.41,
            51,
            38,
            0.8837,
            "-s 7.5 --max-seqs 2000",
        ),
    ];

    for (name, tool, e, wall, found, hits, sens, params) in runs {
        table.row([
            Cell::from(name),
            Cell::from(tool),
            Cell::from(e),
            Cell::from(wall),
            Cell::from(found),
            Cell::from(hits),
            Cell::from(sens),
            Cell::from(params),
        ]);
    }

    print!("{}", table.render());
}
