//! Reading a table a line at a time.

use std::io::ErrorKind;

use toil::{Cell, Column, Entry, Reader, Schema, Stream, Table};

fn schema() -> Schema {
    Schema::new([
        Column::new("query").ragged(),
        Column::new("target").min_width(16),
        Column::new("pass"),
        Column::stacked(["run", "one"]).fixed(1),
        Column::stacked(["run", "two"]).fixed(1),
        Column::new("dom").ragged(),
    ])
}

/// The cells of row `r`, as they are written.
fn row(r: usize) -> [String; 6] {
    // an overrun, and a non-ASCII cell
    let target = match r {
        700 => "a-target-far-wider-than-its-column".to_string(),
        900 => "café".to_string(),
        _ => format!("t{}", r % 97),
    };
    let pass = [".", "-", "AB"][r % 3].to_string();
    let one = match r % 5 {
        0 => "-".to_string(),
        _ => format!("{:.1}", r as f64 / 7.0),
    };
    [
        format!("q{}", r * 37 % 1000),
        target,
        pass,
        one,
        format!("{:.1}", r as f64 / 3.0),
        vec!["d"; 1 + r % 4].join(","),
    ]
}

fn written() -> Vec<u8> {
    let schema = schema();
    let widths = schema.widths();
    let mut out = Stream::new(schema, widths, Vec::new());
    out.meta("format", ["runs", "1"]).unwrap();
    out.header().unwrap();

    for r in 0..1000 {
        if r % 250 == 0 {
            out.meta("shard", [r / 250]).unwrap();
        }
        if r == 500 {
            out.comment("halfway").unwrap();
        }
        let cells = row(r);
        let mut line = out.line().unwrap();
        for (k, cell) in cells.iter().enumerate() {
            match (k, cell.as_str()) {
                (3, "-") => line.cell(None::<f64>).unwrap(),
                _ => line.cell(Cell::from(cell)).unwrap(),
            };
        }
        line.end().unwrap();
    }
    out.meta("end", [1000]).unwrap();
    out.into_inner()
}

#[test]
fn every_row_reads_back() {
    let text = written();
    let lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();

    let mut reader = Reader::new(text.as_slice()).unwrap();
    let header = reader.header();
    let format: Vec<_> = header.meta_rows().collect();
    assert_eq!(format.len(), 1);
    assert_eq!(format[0].key(), "format");
    assert_eq!(format[0].exactly(), Some([Some("runs"), Some("1")]));
    assert_eq!(header.index("run two"), Some(4));

    let mut r = 0;
    let mut comments = Vec::new();
    while let Some((number, entry)) = reader.next_entry().unwrap() {
        let at = lines[number - 1];
        match entry {
            Entry::Meta(meta) => {
                let [n] = meta.exactly().expect("one word");
                let n: usize = n.unwrap().parse().unwrap();
                assert_eq!(at, format!("#= {} {n}", meta.key()).as_bytes());
                comments.push(format!("{} {n}", meta.key()));
            }
            Entry::Comment(line) => {
                assert_eq!(line.as_bytes(), at);
                comments.push(line.to_string());
            }
            Entry::Row(cells) => {
                let want = row(r);
                assert_eq!(cells.len(), want.len(), "row {r}");
                for (k, want) in want.iter().enumerate() {
                    assert_eq!(cells.field(k), Some(want.as_bytes()), "row {r} cell {k}");
                }
                assert!(at.starts_with(want[0].as_bytes()), "row {r} line");
                r += 1;
            }
        }
    }

    assert_eq!(r, 1000);
    assert_eq!(
        comments,
        [
            "shard 0",
            "shard 1",
            "shard 2",
            "# halfway",
            "shard 3",
            "end 1000"
        ]
    );
}

#[test]
fn a_long_table_parses_as_it_streams() {
    let text = String::from_utf8(written()).unwrap();
    let back = Table::parse(&text).unwrap();
    assert_eq!(back.labels()[3], "run one");
    assert_eq!(back.rows().len(), 1000);

    for (r, got) in back.rows().iter().enumerate() {
        for (k, want) in row(r).iter().enumerate() {
            let want = Some(want.as_str()).filter(|&want| want != "-");
            assert_eq!(got.get(k), want, "row {r} cell {k}");
        }
    }

    let shards: Vec<_> = back
        .interludes()
        .iter()
        .map(|(at, line)| (*at, line.as_str()))
        .collect();
    assert_eq!(
        shards,
        [
            (0, "#= shard 0"),
            (250, "#= shard 1"),
            (500, "#= shard 2"),
            (500, "# halfway"),
            (750, "#= shard 3"),
            (1000, "#= end 1000"),
        ]
    );
}

#[test]
fn a_row_is_bytes_and_a_comment_is_text() {
    let mut text = written();
    // row 800
    let at = text.windows(5).position(|w| w == b"\nq600").unwrap();
    text[at + 2] = 0xFF;
    text.extend_from_slice(b"#= \xFF\n");
    let last = text.iter().filter(|&&b| b == b'\n').count();

    let mut reader = Reader::new(text.as_slice()).unwrap();
    let mut seen = false;
    loop {
        match reader.next_entry() {
            Ok(Some((_, Entry::Row(cells)))) => seen |= cells.field(0) == Some(b"q\xFF00"),
            Ok(Some((_, Entry::Meta(_) | Entry::Comment(_)))) => {}
            Ok(None) => panic!("the comment read as text"),
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::InvalidData);
                assert!(e.to_string().starts_with(&format!("line {last}:")), "{e}");
                break;
            }
        }
    }
    assert!(seen, "the row with a byte that is not UTF-8 was not read");
}
