//! Reading tables back: whatever `toil` writes, `Table::parse` recovers.

use toil::{
    Align, Cell, Column, Entry, Marker, Reader, Rule, Schema, Stack, Stream, Style, Table, Trailing,
};

/// Parse `text`, and hold what came back to what a render of it reads back
/// as, and to what a [`Reader`] makes of the text.
fn round_trip(text: &str) -> Table {
    let table = Table::parse(text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    let again = Table::parse(&table.render()).unwrap();
    assert_eq!(again.labels(), table.labels(), "{text}");
    assert_eq!(cells(&again), cells(&table), "{text}");
    read_alike(text, &table, text);
    table
}

/// Hold a [`Reader`] over `text` to what `Table::parse` made of it: the same
/// preamble and labels, the same cells, and the same comments where they sat.
fn read_alike(text: &str, back: &Table, context: &str) {
    let mut reader = Reader::new(text.as_bytes()).unwrap_or_else(|e| panic!("{e}\n{context}"));
    assert_eq!(reader.header().preamble(), back.preamble(), "{context}");
    assert_eq!(reader.header().labels(), back.labels(), "{context}");

    let n = back.labels().len();
    let mut rows = Vec::new();
    let mut comments = Vec::new();
    while let Some((_, entry)) = reader.next_entry().unwrap() {
        match entry {
            Entry::Row(cells) => rows.push(
                (0..n)
                    .map(|i| match cells.field(i) {
                        None => Some(String::new()),
                        Some(b"-") => None,
                        Some(text) => Some(String::from_utf8(text.to_vec()).unwrap()),
                    })
                    .collect::<Vec<_>>(),
            ),
            Entry::Meta(meta) => {
                let mut line = String::from("#=");
                for part in [Some(meta.key()), meta.rest(0)].into_iter().flatten() {
                    if !part.is_empty() {
                        line.push(' ');
                        line.push_str(part);
                    }
                }
                comments.push((rows.len(), line));
            }
            Entry::Comment(line) => comments.push((rows.len(), line.to_string())),
        }
    }

    // a `#=` line comes back as its key and words, which
    // spell the line again but for spaces at either end
    let written: Vec<(usize, String)> = back
        .interludes()
        .iter()
        .map(|(at, line)| match line.starts_with("#=") {
            true => (*at, line.trim_end().to_string()),
            false => (*at, line.clone()),
        })
        .collect();

    assert_eq!(rows, cells(back), "{context}");
    assert_eq!(comments, written, "{context}");
}

fn cells(table: &Table) -> Vec<Vec<Option<String>>> {
    table
        .rows()
        .iter()
        .map(|row| {
            (0..row.len())
                .map(|i| row.get(i).map(str::to_string))
                .collect()
        })
        .collect()
}

// ---

#[test]
fn the_goldens_read_back() {
    for text in GOLDENS {
        round_trip(text);
    }
}

#[test]
fn what_render_wrote_parses_back() {
    let schema = Schema::new(["name", "wall(s)"]).style(indented());
    let mut table = Table::new(schema);
    table.comment("query 200");
    table.row(["a-long-one", "1.50"]).row(["b", "22.00"]);

    let back = round_trip(&table.render());

    assert_eq!(back.labels(), ["name", "wall(s)"]);
    assert_eq!(back.preamble(), ["# query 200"]);
    assert_eq!(back.rows().len(), 2);
    assert_eq!(back.rows()[0].get(0), Some("a-long-one"));
    assert_eq!(
        back.rows()[1].get(back.index("wall(s)").unwrap()),
        Some("22.00")
    );
}

#[test]
fn meta_rows_read_back_what_meta_wrote() {
    let mut table = Table::new(Schema::new(["run", "shard"]));
    table.meta("tool", ["nail", "0.4.1", "a1b2c3"]);
    table.meta("tool", ["hmmer", "3.4", ""]);
    table.meta("failed", [Some("seed-3"), None]);
    table.meta("imported", ["dealt before set.tbl existed; see the ledger"]);
    table.meta("rows", [1200u64]);
    table.row(["seed-1", "0"]);
    table.interlude("#= end 1");

    let text = table.render();
    assert!(text.starts_with(concat!(
        "#= tool nail 0.4.1 a1b2c3\n",
        "#= tool hmmer 3.4 -\n",
        "#= failed seed-3 -\n",
        "#= imported dealt before set.tbl existed; see the ledger\n",
        "#= rows 1200\n",
    )));

    let back = Table::parse(&text).unwrap();
    let meta: Vec<_> = back.meta_rows().collect();
    let keys: Vec<_> = meta.iter().map(|m| m.key()).collect();
    assert_eq!(keys, ["tool", "tool", "failed", "imported", "rows", "end"]);

    let tools: Vec<_> = meta
        .iter()
        .filter(|m| m.key() == "tool")
        .map(|m| m.exactly::<3>())
        .collect();
    assert_eq!(
        tools,
        [
            Some([Some("nail"), Some("0.4.1"), Some("a1b2c3")]),
            Some([Some("hmmer"), Some("3.4"), None]),
        ]
    );
    assert_eq!(meta[0].exactly::<2>(), None);
    assert_eq!(meta[2].exactly(), Some([Some("seed-3"), None]));
    assert_eq!(
        meta[3].rest(0),
        Some("dealt before set.tbl existed; see the ledger")
    );
    assert_eq!(meta[3].get(1), Some("before"));
    assert_eq!(meta[3].rest(5), Some("the ledger"));
    assert_eq!(meta[4].get(0), Some("1200"));
    assert_eq!(meta[5].get(0), Some("1"));
}

#[test]
fn a_metadata_line_is_not_the_header() {
    let text = "#= run nail 1.5\n# query 200 families\n#\n# name n\n# ---- -\n  a    1\n";
    let back = round_trip(text);

    assert_eq!(back.labels(), ["name", "n"]);
    assert_eq!(back.rows()[0].get(0), Some("a"));
    assert_eq!(back.preamble(), ["#= run nail 1.5", "# query 200 families"]);
}

/// What an older writer left: a comment directly above the header, with no
/// bare `#` between. It does not line up with the rule, so it is preamble.
#[test]
fn a_comment_with_no_separator_is_preamble() {
    let text = "# query 200 families\n# name n\n# ---- -\n  a    1\n";
    let back = Table::parse(text).unwrap();

    assert_eq!(back.labels(), ["name", "n"]);
    assert_eq!(back.preamble(), ["# query 200 families"]);
}

#[test]
fn a_short_row_reads_its_tail_as_missing() {
    let back = Table::parse("# name tool shard\n# ---- ---- -----\n  a    nail\n").unwrap();

    assert_eq!(back.rows()[0].get(1), Some("nail"));
    assert_eq!(back.rows()[0].get(2), None);
}

#[test]
fn a_table_with_no_header_is_an_error() {
    assert!(Table::parse("  a 1\n  b 2\n").is_err());
    assert!(Table::parse("").is_err());
}

#[test]
fn the_placeholder_reads_back_as_missing() {
    let mut table = Table::new(["a", "b"]);
    table.row([Cell::missing(), Cell::from("x")]);

    let back = round_trip(&table.render());
    assert_eq!(back.rows()[0].get(0), None);
    assert_eq!(back.rows()[0].get(1), Some("x"));
}

#[test]
fn a_comment_above_the_header_is_kept_apart_from_a_stacked_label() {
    let schema = Schema::new(vec![Column::stacked(["top", "name"]), Column::new("n")]);

    let mut plain = Table::new(Schema::new(["name", "n"]));
    plain.comment("top").row(["a", "1"]);
    let mut stacked = Table::new(schema);
    stacked.row(["a", "1"]);

    let plain = round_trip(&plain.render());
    let stacked = round_trip(&stacked.render());

    assert_eq!(plain.labels(), ["name", "n"]);
    assert_eq!(plain.preamble(), ["# top"]);
    assert_eq!(stacked.labels(), ["top name", "n"]);
    assert!(stacked.preamble().is_empty());
}

#[test]
fn a_stream_reads_back_with_its_blocks_marked() {
    let schema = Schema::new(["target", "score"]).style(indented());
    let widths = schema.widths();

    let mut out = Stream::new(schema, widths, Vec::new());
    out.meta("format", [2]).unwrap();
    out.meta("shard", [1]).unwrap();
    out.header().unwrap();
    out.row(["7tm_1", "175.8"]).unwrap();
    out.meta("shard", [2]).unwrap();
    out.row(["a-name-longer-than-its-column", "1"]).unwrap();
    out.row(["x", "2"]).unwrap();
    out.meta("end", [3]).unwrap();

    let text = String::from_utf8(out.into_inner()).unwrap();
    let back = Table::parse(&text).unwrap();

    assert_eq!(back.preamble(), ["#= format 2", "#= shard 1"]);
    assert_eq!(
        back.interludes(),
        [(1, "#= shard 2".to_string()), (3, "#= end 3".to_string())]
    );
    assert_eq!(
        cells(&back),
        [
            [Some("7tm_1".into()), Some("175.8".into())],
            [
                Some("a-name-longer-than-its-column".into()),
                Some("1".into())
            ],
            [Some("x".into()), Some("2".into())],
        ]
    );
}

#[test]
fn a_last_column_full_of_spaces_reads_whole() {
    let mut columns: Vec<Column> = ["step", "exit"].into_iter().map(Column::new).collect();
    columns.push(Column::new("argv").ragged());

    let mut table = Table::new(columns);
    table.row(["[1](a)", "0", "/bin/a --flag value > /dev/null"]);
    table.row([
        Cell::from("||").align(Align::Right),
        "1".into(),
        "b c".into(),
    ]);

    let back = round_trip(&table.render());
    assert_eq!(
        back.rows()[0].get(2),
        Some("/bin/a --flag value > /dev/null")
    );
    assert_eq!(back.rows()[1].get(0), Some("||"));
}

/// Every combination of the style's axes and of where a ragged column sits,
/// over rows holding the awkward cells: empty, right-aligned, missing, wider
/// than the label, and a space in the last column.
#[test]
fn every_style_reads_back() {
    let markers = [Marker::Absorb, Marker::Indent];
    let rules = [Rule::Dashes, Rule::Solid, Rule::None];
    let stacks = [Stack::Bottom, Stack::Top];
    let trailings = [Trailing::Trim, Trailing::Keep];

    for marker in markers {
        for rule in rules {
            for stack in stacks {
                for trailing in trailings {
                    for ragged in [None, Some(0), Some(1), Some(3)] {
                        let style = Style::default()
                            .marker(marker)
                            .rule(rule)
                            .stack(stack)
                            .trailing(trailing);
                        check(style, ragged);
                    }
                }
            }
        }
    }
}

fn check(style: Style, ragged: Option<usize>) {
    let labels = [
        Column::new("name"),
        Column::stacked(["cell", "count"]),
        Column::new("score").align(Align::Right).fixed(2),
        Column::stacked(["the", "last", "one"]),
    ];
    let columns: Vec<Column> = labels
        .into_iter()
        .enumerate()
        .map(|(k, c)| match Some(k) == ragged {
            true => c.ragged(),
            false => c,
        })
        .collect();

    let mut table = Table::new(Schema::new(columns).style(style.clone()));
    table.meta("format", [1]);
    table.comment("a comment directly above the header");
    table.row([
        Cell::from("alpha"),
        Cell::from(3),
        Cell::from(1.5),
        Cell::from("x"),
    ]);
    table.interlude("#= block 2");
    table.row([
        Cell::from("a-rather-longer-name"),
        Cell::missing(),
        Cell::from(12.25),
        Cell::from("two words"),
    ]);
    table.row([
        Cell::from("b"),
        Cell::from(""),
        Cell::missing(),
        Cell::from(""),
    ]);
    table.interlude("# the end");

    let text = table.render();
    let context = format!("{style:?} ragged {ragged:?}\n{text}");
    let back = Table::parse(&text).unwrap_or_else(|e| panic!("{e}\n{context}"));
    read_alike(&text, &back, &context);

    assert_eq!(
        back.labels(),
        ["name", "cell count", "score", "the last one"],
        "{context}"
    );
    assert_eq!(
        back.preamble(),
        ["#= format 1", "# a comment directly above the header"],
        "{context}"
    );
    assert_eq!(
        back.interludes(),
        [(1, "#= block 2".to_string()), (3, "# the end".to_string())],
        "{context}"
    );

    let s = |x: &str| Some(x.to_string());
    assert_eq!(
        cells(&back),
        [
            [s("alpha"), s("3"), s("1.50"), s("x")],
            [s("a-rather-longer-name"), None, s("12.25"), s("two words")],
            // an empty cell is written as the placeholder
            [s("b"), None, None, None],
        ],
        "{context}"
    );
}

/// A small generator, so the random tables below are the same on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn word(&mut self, max: u64) -> String {
        let len = self.below(max + 1);
        (0..len)
            .map(|_| (b'a' + self.below(26) as u8) as char)
            .collect()
    }
}

/// Random tables across every axis at once, short of the few whose bytes
/// another table writes too.
#[test]
fn random_tables_read_back() {
    let mut rng = Lcg(7);
    for case in 0..5000 {
        random_table(&mut rng, case, false);
    }
}

/// Random tables with columns set wider than their labels and cells, more
/// columns, and now and then many rows.
#[test]
fn wider_random_tables_read_back() {
    let mut rng = Lcg(8);
    for case in 0..3000 {
        random_table(&mut rng, case, true);
    }
}

/// Write a table at random, read it back, and hold the two to each other.
fn random_table(rng: &mut Lcg, case: usize, wider: bool) {
    {
        let rule = [Rule::Dashes, Rule::Solid, Rule::None][rng.below(3) as usize];
        let style = Style::default()
            .marker([Marker::Absorb, Marker::Indent][rng.below(2) as usize])
            .rule(rule)
            .stack([Stack::Bottom, Stack::Top][rng.below(2) as usize])
            .trailing([Trailing::Trim, Trailing::Keep][rng.below(2) as usize]);

        let n = 1 + rng.below(if wider { 8 } else { 5 }) as usize;
        let mut labels = Vec::new();
        let mut raggeds = Vec::new();
        let columns: Vec<Column> = (0..n)
            .map(|k| {
                let depth = 1 + rng.below(3);
                let words: Vec<String> = (0..depth)
                    // only a lone label over a dashed rule, short of
                    // the last column, may be empty: under a solid
                    // rule or none an empty label leaves no mark, an
                    // empty word in a stacked label writes nothing,
                    // and an empty last column can be ruled zero wide
                    .map(|_| match rule == Rule::Dashes && depth == 1 && k + 1 < n {
                        true => rng.word(6),
                        false => rng.word(5) + "x",
                    })
                    .collect();
                let ragged = rng.below(4) == 0;
                raggeds.push(ragged);
                labels.push(words.join(" "));
                let mut column = Column::stacked(words);
                if ragged {
                    column = column.ragged();
                }
                if rng.below(3) == 0 {
                    column = column.align(Align::Right);
                }
                if wider && rng.below(3) == 0 {
                    column = column.min_width(rng.below(9) as usize);
                }
                column
            })
            .collect();

        let mut table = Table::new(Schema::new(columns).style(style.clone()));
        let mut expected = Vec::new();
        for _ in 0..rng.below(4) {
            match rng.below(3) {
                0 => table.meta(&rng.word(8), [""; 0]),
                _ => table.comment(rng.word(8)),
            };
        }
        let rows = match wider && rng.below(10) == 0 {
            true => 65 + rng.below(136),
            false => rng.below(6),
        };
        for _ in 0..rows {
            let mut row = Vec::new();
            let mut want = Vec::new();
            for (k, &ragged) in raggeds.iter().enumerate() {
                let (cell, text) = match rng.below(6) {
                    0 => (Cell::missing(), None),
                    // a space only in the last column, and never at
                    // either end of a cell, since a reader splitting
                    // on whitespace cannot tell those from two cells
                    1 if k + 1 == n => {
                        let text = format!("{} {}", rng.word(4) + "y", rng.word(4) + "z");
                        (Cell::from(text.clone()), Some(text))
                    }
                    _ => {
                        // an empty ragged cell moves the rest of the
                        // row along by one, which a plain column can
                        // write the same way with a right-aligned cell
                        let text = match ragged {
                            true => rng.word(8) + "w",
                            false => rng.word(9),
                        };
                        // an empty cell is written as the placeholder
                        let want = Some(text.clone()).filter(|t| !t.is_empty());
                        (Cell::from(text), want)
                    }
                };
                let cell = match rng.below(8) {
                    0 => cell.align(Align::Right),
                    1 => cell.align(Align::Left),
                    _ => cell,
                };
                row.push(cell);
                want.push(text);
            }
            table.row(row);
            expected.push(want);
            if rng.below(5) == 0 {
                table.interlude(format!("#= {}", rng.word(6)));
            }
        }

        let text = table.render();
        let context = format!("case {case}: {style:?}\n{text}");
        let back = Table::parse(&text).unwrap_or_else(|e| panic!("{e}\n{context}"));
        read_alike(&text, &back, &context);

        assert_eq!(back.labels(), labels, "{context}");
        assert_eq!(cells(&back), expected, "{context}");
    }
}

fn indented() -> Style {
    Style::default()
        .marker(Marker::Indent)
        .trailing(Trailing::Keep)
}

/// The expected text of each case in `golden.rs`.
const GOLDENS: [&str; 7] = [
    concat!(
        "# step     cmd   job wall(s) user(s) sys(s) cpu(%) max_rss exit status argv\n",
        "# -------- ----- --- ------- ------- ------ ------ ------- ---- ------ ----\n",
        "[1](setup) mkdir -   1.50    2.00    0.25   150%   2.00MiB 0    ok     /mkdir\n",
        "[2](burn)  -     -   1.50    4.00    0.50   300%   2.00MiB -    -      -\n",
        "        || a     1   1.50    2.00    0.25   150%   2.00MiB 0    ok     /a\n",
        "        || b     2   1.50    2.00    0.25   150%   2.00MiB 0    ok     /b\n",
    ),
    concat!(
        "# name       n \n",
        "# ---------- --\n",
        "  a-long-one 1 \n",
        "  b          22\n",
    ),
    concat!(
        "# name doms\n",
        "# ---- ----\n",
        "  a    1.0,2.0,3.0\n",
        "  b    4.0\n",
    ),
    concat!(
        "# query        200 families         38061 residues      17922777 bytes\n",
        "# target      1000 seqs            194987 residues        220417 bytes\n",
        "# pairs        261 rows                 2 runs\n",
        "# hmmer         43 hits            0.2600 wall_s\n",
        "# seed                             2.1600 wall_s\n",
        "#\n",
        "# name  tool  E       wall_s found hits sens   hits_sd sens_sd\n",
        "# ----- ----- ------- ------ ----- ---- ------ ------- -------\n",
        "  hmmer hmmer -       0.2600 43    43   1.0000 27      1.0000 \n",
        "  nail  nail  1000000 0.1200 33    32   0.7442 23      0.8519 \n",
    ),
    concat!(
        "# family     nail_1 nail_n mmseqs_1 mmseqs_n\n",
        "# ---------- ------ ------ -------- --------\n",
        "  1-cysPrx_C 23.9   12097  33.0     29851   \n",
        "  10_blade   30.8   4845   38.0     1387    \n",
    ),
    concat!(
        "# target             family          %id prf    prf    seq      \n",
        "#                                        blast  mmseqs diamond  \n",
        "#                                               s5.7   ultra    \n",
        "#                                               ms2000 sensitive\n",
        "#---------------------------------------------------------------\n",
        "Q9HS26_HALSA/183-491 Na_H_antiporter 10% 37.8   41.0   41.6     \n",
        "B2A353_NATTJ/185-387 FlaE            11% 34.5   33.0   -        \n",
        "PDR3_YEAST/278-627   Fungal_trans     7% -      21.0   -        \n",
    ),
    concat!(
        "#                                            target target query query       comp         cell \n",
        "# target                     query           start  end    start end   score bias evalue  frac \n",
        "# -------------------------- --------------- ------ ------ ----- ----- ----- ---- ------- -----\n",
        "F1MV99|reviewed|Somatostatin 7tm_1-consensus 58     306    1     260   175.8 7.5  1.0e-53 0.066\n",
        "O08858|reviewed|Somatostatin 7tm_1-consensus 54     303    1     260   173.0 7.8  7.7e-53 0.069\n",
        "C3ZQF9|reviewed|QRFP-like    7tm_1-consensus 64     326    1     260   149.3 8.6  1.3e-45 0.070\n",
    ),
];
