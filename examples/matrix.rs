//! One row per sequence, one column per run, which is a lot of columns.
//!
//! A run's name is longer than the numbers under it, so the label is stacked
//! over several header lines instead of setting the column's width on its own,
//! and a minimum width keeps the numbers from closing up under a short one.
//! The rule crosses the whole table rather than being broken per column, which
//! reads better when there are this many of them.

use toil::{Align, Cell, Column, Rule, Schema, Stack, Style, Table, Trailing};

/// A run's name, as the header stacks it: the search mode, then whatever the
/// run's own name is separated by.
fn run(mode: &str, name: &str) -> Column {
    let mut words = vec![mode.to_string()];
    words.extend(name.split('-').map(str::to_string));

    Column::stacked(words).min_width(6).fixed(1)
}

fn main() {
    let mut columns = vec![
        Column::new("target"),
        Column::new("family"),
        Column::new("%id").align(Align::Right),
    ];

    let runs = [
        ("prf", "blast"),
        ("prf", "hmmer"),
        ("prf", "mmseqs-s7.5-ms2000"),
        ("prf", "nail-s7.5-ms2000"),
        ("seq", "diamond-ultra-sensitive"),
        ("seq", "last"),
        ("seq", "nail-s14.0-ms2000"),
    ];
    columns.extend(runs.iter().map(|&(mode, name)| run(mode, name)));

    let schema = Schema::new(columns).style(
        Style::default()
            .rule(Rule::Solid)
            .stack(Stack::Top)
            .trailing(Trailing::Keep),
    );

    let mut table = Table::new(schema);

    let entries: [(&str, &str, &str, [Option<f64>; 7]); 5] = [
        (
            "Q9HS26_HALSA/183-491",
            "Na_H_antiporter",
            "10%",
            [
                Some(37.8),
                Some(32.2),
                Some(41.0),
                Some(33.6),
                Some(41.6),
                Some(49.6),
                Some(25.1),
            ],
        ),
        (
            "B2A353_NATTJ/185-387",
            "FlaE",
            "11%",
            [
                Some(34.5),
                Some(29.8),
                Some(33.0),
                Some(26.7),
                None,
                None,
                Some(26.0),
            ],
        ),
        (
            "HS2ST_DROME/65-319",
            "Sulfotransfer_2",
            "12%",
            [Some(24.6), None, Some(29.0), None, Some(18.5), None, None],
        ),
        (
            "SKI2_SCHPO/815-1011",
            "MTR4_beta-barrel",
            "12%",
            [None, None, Some(25.0), None, Some(27.3), None, Some(11.3)],
        ),
        (
            "PDR3_YEAST/278-627",
            "Fungal_trans",
            "7%",
            [None, None, None, None, None, None, Some(21.0)],
        ),
    ];

    for (target, family, pid, scores) in entries {
        let mut cells = vec![Cell::from(target), Cell::from(family), Cell::from(pid)];
        cells.extend(scores.into_iter().map(Cell::from));
        table.row(cells);
    }

    print!("{}", table.render());
}
