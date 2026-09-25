//! Widths held apart from rows, which is what lets a table be written in
//! pieces and still line up.

use toil::{Column, Header, Schema, Stream, Table};

/// The column a value starts at, so two lines can be compared without counting
/// spaces by hand.
fn offset(line: &str, value: &str) -> Option<usize> {
    line.find(value)
}

#[test]
fn blocks_written_apart_line_up_under_one_header() {
    let schema = Schema::new(["step", "wall(s)"]);

    let rows = [
        schema.row(["[1](setup)", "1.50"]),
        schema.row(["[2](burn)", "9.25"]),
    ];
    let floor = schema.measure(&rows);

    let mut first = Table::new(schema.clone());
    first.push(rows[0].clone());

    let mut second = Table::new(schema);
    second.push(rows[1].clone());

    let text = format!(
        "{}{}",
        first.render_with(&floor, Header::Show),
        second.render_with(&floor, Header::Hide)
    );

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        4,
        "a header, a rule, and a row from each block"
    );

    // the second block's step name is shorter, so without the
    // floor its wall clock would close up a character to the
    // left of the first block's
    assert_eq!(offset(lines[2], "1.50"), offset(lines[3], "9.25"));
}

#[test]
fn a_block_wider_than_the_floor_takes_the_room_it_needs() {
    let schema = Schema::new(["step", "wall(s)"]);
    let floor = schema.widths();

    let mut table = Table::new(schema);
    table.row(["a-very-long-step-name", "1.50"]);

    let line = table.render_with(&floor, Header::Hide);
    assert!(line.starts_with("a-very-long-step-name 1.50"), "{line:?}");
}

#[test]
fn a_stream_writes_the_widths_it_was_given() {
    let schema = Schema::new([Column::new("target").min_width(20), Column::new("score")]);
    let widths = schema.widths();

    let mut out = Stream::new(schema, widths, Vec::new());
    out.row(["7tm_1", "175.8"]).unwrap();
    out.row(["a-target-whose-name-runs-past-its-column", "1.0"])
        .unwrap();

    let text = String::from_utf8(out.into_inner()).unwrap();
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(offset(lines[2], "175.8"), Some(21));

    // the long name overruns its column rather than widening
    // it, because the lines above it have already gone out and
    // cannot be widened to match
    assert!(
        lines[3].starts_with("a-target-whose-name-runs-past-its-column 1.0"),
        "{}",
        lines[3]
    );
}
