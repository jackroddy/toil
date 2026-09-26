//! Reading a table back: its cells by label, its `#=` lines, and enough of
//! its shape to go on writing it.
//!
//! The text here is a string, and [`Table::read`] does the same from a file.
//! A table too long to hold whole goes through a [`Reader`] instead, a line
//! at a time.

use std::io::{Write, stdout};

use toil::{Cell, Column, Entry, Reader, Rule, Schema, Stream, Style, Table};

fn main() -> std::io::Result<()> {
    let schema = Schema::new([
        Column::new("city"),
        Column::new("high").fixed(1),
        Column::new("low").fixed(1),
        Column::new("rain"),
    ])
    .style(Style::default().rule(Rule::None));

    let mut table = Table::new(schema);
    table.meta("units celsius mm");
    table.meta("day 2026-09-25");
    for (city, high, low, rain) in [
        ("Lisbon", 21.4, 13.0, Some(2)),
        ("Oslo", 11.9, 4.2, Some(14)),
        ("Tucson", 34.1, 21.7, None),
    ] {
        table.row([
            Cell::from(city),
            Cell::from(high),
            Cell::from(low),
            Cell::from(rain),
        ]);
    }

    let text = table.render();
    let back = Table::parse(&text).expect("toil wrote this text");

    let mut out = stdout().lock();
    write!(out, "{text}")?;

    let mut more = Stream::continued(back.schema().clone(), back.widths(), &mut out);
    more.row([
        Cell::from("Quito"),
        Cell::from(24.8),
        Cell::from(12.3),
        Cell::from(Some(0)),
    ])?;

    writeln!(out)?;
    for line in back.meta_lines() {
        writeln!(out, "meta: {line}")?;
    }
    for row in 0..back.rows().len() {
        let city = back.get(row, "city").unwrap_or("?");
        let rain = back.get(row, "rain").unwrap_or("no reading");
        writeln!(out, "{city}: {rain}")?;
    }

    writeln!(out)?;
    let mut reader = Reader::new(text.as_bytes())?;
    let high = reader.header().index("high").expect("the table has a high");
    while let Some((number, entry)) = reader.next_entry()? {
        if let Entry::Row(cells) = entry {
            let text = cells.field(high).unwrap_or_default();
            writeln!(out, "line {number}: high {}", String::from_utf8_lossy(text))?;
        }
    }

    out.flush()
}
