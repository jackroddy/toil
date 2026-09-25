//! The row path that writes into the stream rather than into a `String`.
//!
//! What is being asserted is sameness: a line has to reach the file as the
//! bytes [`Stream::row`] would have written. Everything else about a table is
//! already covered by the goldens.

use toil::{Align, Cell, Column, Marker, Rule, Schema, Stream, Style, Trailing};

/// A cell as the two paths each take it.
#[derive(Clone, Copy)]
enum V {
    T(&'static str),
    N(f64),
    M,
}

impl From<V> for Cell {
    fn from(v: V) -> Cell {
        match v {
            V::T(text) => Cell::from(text),
            V::N(n) => Cell::from(n),
            V::M => Cell::missing(),
        }
    }
}

/// The same rows down both paths, which have to agree byte for byte.
fn both(schema: Schema, rows: &[Vec<V>]) -> (String, String) {
    let widths = schema.widths();

    let mut batch = Stream::new(schema.clone(), widths.clone(), Vec::new());
    for row in rows {
        batch.row(row.iter().copied()).unwrap();
    }

    let mut cells = Stream::new(schema, widths, Vec::new());
    for row in rows {
        let mut line = cells.line().unwrap();
        for v in row {
            match *v {
                V::T(text) => line.text(text).unwrap(),
                V::N(n) => line.num(n).unwrap(),
                V::M => line.missing().unwrap(),
            };
        }
        line.end().unwrap();
    }

    (
        String::from_utf8(batch.into_inner()).unwrap(),
        String::from_utf8(cells.into_inner()).unwrap(),
    )
}

#[test]
fn a_line_writes_what_a_row_would_have() {
    let schema = Schema::new([
        Column::new("step"),
        Column::new("job").align(Align::Right).min_width(5),
        Column::new("wall").fixed(2).min_width(8),
        Column::new("rate").scientific(1),
        Column::new("n"),
        Column::new("argv").ragged(),
    ]);

    let rows = vec![
        vec![
            V::T("[1](setup)"),
            V::N(1.0),
            V::N(1.5),
            V::N(0.000125),
            V::N(12.0),
            V::T("/mkdir -p out"),
        ],
        vec![V::T("burn"), V::M, V::N(-0.04), V::M, V::M, V::M],
        // a short row, which both paths fill out
        vec![V::T("done")],
        // an empty cell mid-row, whose padding the trim has to reach past
        vec![V::T(""), V::T(""), V::T(""), V::T(""), V::T(""), V::T("")],
    ];

    let (batch, cells) = both(schema, &rows);
    assert_eq!(batch, cells);
}

#[test]
fn a_line_writes_what_a_row_would_have_in_every_style() {
    let rows = vec![
        vec![V::T("a"), V::N(1.25), V::T("x y")],
        vec![V::M, V::M, V::M],
        vec![V::T("wider than its column"), V::N(-1.0), V::T("")],
    ];

    for marker in [Marker::Absorb, Marker::Indent] {
        for trailing in [Trailing::Trim, Trailing::Keep] {
            for rule in [Rule::Dashes, Rule::Solid, Rule::None] {
                for ragged in [true, false] {
                    let last = Column::new("note");
                    let schema = Schema::new([
                        Column::new("name").min_width(6),
                        Column::new("score").fixed(1).align(Align::Right),
                        match ragged {
                            true => last.ragged(),
                            false => last,
                        },
                    ])
                    .style(
                        Style::default()
                            .marker(marker)
                            .trailing(trailing)
                            .rule(rule),
                    );

                    let (batch, cells) = both(schema, &rows);
                    assert_eq!(batch, cells, "{marker:?} {trailing:?} {rule:?} {ragged}");
                }
            }
        }
    }
}

/// The recall table `nail-benchmarks` writes: an unpadded name column in
/// front, whose rows are adjacent and so line up without it, and an unpadded
/// domain list on the end.
#[test]
fn a_ragged_column_in_front() {
    let schema = Schema::new([
        Column::new("query").ragged(),
        Column::new("target").min_width(16),
        Column::new("pass").min_width(6),
        Column::new("nail").fixed(1).min_width(6),
        Column::new("mmseqs").fixed(1).min_width(6),
        Column::new("hmmer").fixed(1).min_width(6),
        Column::new("inc").min_width(3),
        Column::new("dom").ragged(),
    ]);

    let widths = schema.widths();
    let mut out = Stream::new(schema, widths, Vec::new());

    for (query, target, pass, nail, mmseqs, hmmer, inc, dom) in [
        (
            "2-Hacid_dh_C",
            "MGYP000522683479",
            "NNNMMH",
            Some(98.8),
            Some(94.0),
            98.7,
            1,
            "98.1",
        ),
        (
            "2-Hacid_dh_C",
            "MGYP000715666710",
            "nnnmmh",
            None,
            None,
            13.7,
            0,
            "9.3,2.8",
        ),
        (
            "2-Hacid_dh_C",
            "MGYP001025488448",
            "nnnmmh",
            Some(21.5),
            Some(30.0),
            20.9,
            1,
            "20.1",
        ),
        (
            "2-oxoacid_dh",
            "MGYP000987338150",
            "NNNMMH",
            Some(145.6),
            Some(140.0),
            145.5,
            1,
            "145.2",
        ),
    ] {
        let mut line = out.line().unwrap();
        line.text(query).unwrap().text(target).unwrap();
        line.text(pass).unwrap();

        for score in [nail, mmseqs] {
            match score {
                Some(score) => line.num(score).unwrap(),
                None => line.missing().unwrap(),
            };
        }

        line.num(hmmer).unwrap();
        line.num(inc as f64).unwrap();
        line.text(dom).unwrap();
        line.end().unwrap();
    }

    let text = String::from_utf8(out.into_inner()).unwrap();
    assert_eq!(
        text,
        "\
# query target           pass   nail   mmseqs hmmer  inc dom
# ----- ---------------- ------ ------ ------ ------ --- ---
2-Hacid_dh_C MGYP000522683479 NNNMMH 98.8   94.0   98.7   1   98.1
2-Hacid_dh_C MGYP000715666710 nnnmmh -      -      13.7   0   9.3,2.8
2-Hacid_dh_C MGYP001025488448 nnnmmh 21.5   30.0   20.9   1   20.1
2-oxoacid_dh MGYP000987338150 NNNMMH 145.6  140.0  145.5  1   145.2
"
    );
}
