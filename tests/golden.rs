//! Output that already exists in the wild, reproduced by `toil`.
//!
//! Each case is a table one of the three surveyed projects writes today. They
//! are the acceptance criteria for the crate: between them they cover both
//! markers, the dashed and solid rules, both stackings, both trailing modes,
//! the ragged last column, and all three number formats. `Rule::None` is not
//! among them, because none of these tables leaves the rule out.
//!
//! The expected text is embedded rather than read from those repositories,
//! which are reference and not a dependency.

use toil::{Align, Cell, Column, Marker, Rule, Schema, Stack, Style, Table, Trailing};

fn row(cells: [&str; 11]) -> Vec<Cell> {
    cells.into_iter().map(Cell::from).collect()
}

/// `pail`'s run summary: the marker absorbed into the first column, a batch
/// marker right-aligned against it, and an unpadded argv on the end.
#[test]
fn pail_run_summary() {
    let mut columns: Vec<Column> = [
        "step", "cmd", "job", "wall(s)", "user(s)", "sys(s)", "cpu(%)", "max_rss", "exit", "status",
    ]
    .into_iter()
    .map(Column::new)
    .collect();
    columns.push(Column::new("argv").ragged());

    let mut table = Table::new(columns);
    table.row(row([
        "[1](setup)",
        "mkdir",
        "-",
        "1.50",
        "2.00",
        "0.25",
        "150%",
        "2.00MiB",
        "0",
        "ok",
        "/mkdir",
    ]));
    table.row(row([
        "[2](burn)",
        "-",
        "-",
        "1.50",
        "4.00",
        "0.50",
        "300%",
        "2.00MiB",
        "-",
        "-",
        "-",
    ]));

    for (name, job, argv) in [("a", "1", "/a"), ("b", "2", "/b")] {
        let mut cells = row([
            "", name, job, "1.50", "2.00", "0.25", "150%", "2.00MiB", "0", "ok", argv,
        ]);
        cells[0] = Cell::from("||").align(Align::Right);
        table.row(cells);
    }

    let expected = "\
# step     cmd   job wall(s) user(s) sys(s) cpu(%) max_rss exit status argv
# -------- ----- --- ------- ------- ------ ------ ------- ---- ------ ----
[1](setup) mkdir -   1.50    2.00    0.25   150%   2.00MiB 0    ok     /mkdir
[2](burn)  -     -   1.50    4.00    0.50   300%   2.00MiB -    -      -
        || a     1   1.50    2.00    0.25   150%   2.00MiB 0    ok     /a
        || b     2   1.50    2.00    0.25   150%   2.00MiB 0    ok     /b
";

    assert_eq!(table.render(), expected);
}

fn indented() -> Style {
    Style::default()
        .marker(Marker::Indent)
        .trailing(Trailing::Keep)
}

/// `util::tbl`'s default: the marker offsets every line, and the last column
/// keeps its padding.
#[test]
fn benchmark_analysis_table() {
    let mut table = Table::new(Schema::new(["name", "n"]).style(indented()));
    table.row(["a-long-one", "1"]);
    table.row(["b", "22"]);

    // concat! rather than a continued literal, because the
    // padding these cases are about is trailing whitespace,
    // which a `\` continuation eats
    let expected = concat!(
        "# name       n \n",
        "# ---------- --\n",
        "  a-long-one 1 \n",
        "  b          22\n",
    );

    assert_eq!(table.render(), expected);
}

/// The same, with the last column left to the rows and ruled to its label.
#[test]
fn benchmark_analysis_table_ragged() {
    let schema =
        Schema::new(vec![Column::new("name"), Column::new("doms").ragged()]).style(indented());

    let mut table = Table::new(schema);
    table.row(["a", "1.0,2.0,3.0"]);
    table.row(["b", "4.0"]);

    let expected = concat!(
        "# name doms\n",
        "# ---- ----\n",
        "  a    1.0,2.0,3.0\n",
        "  b    4.0\n",
    );

    assert_eq!(table.render(), expected);
}

/// `hl-summary.tbl`: a run's own numbers under a block saying what they are
/// numbers of, with a bare `#` closing the block off from the header.
#[test]
fn hit_loss_summary() {
    let schema = Schema::new(vec![
        Column::new("name"),
        Column::new("tool"),
        Column::new("E"),
        Column::new("wall_s").fixed(4),
        Column::new("found"),
        Column::new("hits"),
        Column::new("sens").fixed(4),
        Column::new("hits_sd"),
        Column::new("sens_sd").fixed(4),
    ])
    .style(indented());

    let mut table = Table::new(schema);
    table.comment("query        200 families         38061 residues      17922777 bytes");
    table.comment("target      1000 seqs            194987 residues        220417 bytes");
    table.comment("pairs        261 rows                 2 runs");
    table.comment("hmmer         43 hits            0.2600 wall_s");
    table.comment("seed                             2.1600 wall_s");
    table.comment("");

    table.row([
        Cell::from("hmmer"),
        Cell::from("hmmer"),
        Cell::missing(),
        Cell::from(0.26),
        Cell::from(43),
        Cell::from(43),
        Cell::from(1.0),
        Cell::from(27),
        Cell::from(1.0),
    ]);
    table.row([
        Cell::from("nail"),
        Cell::from("nail"),
        Cell::from("1000000"),
        Cell::from(0.12),
        Cell::from(33),
        Cell::from(32),
        Cell::from(0.7442),
        Cell::from(23),
        Cell::from(0.8519),
    ]);

    let expected = concat!(
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
    );

    assert_eq!(table.render(), expected);
}

/// `mgy-cutoffs.tbl`: a wide table of per-family scores, the counts written
/// plainly beside the scores written to one place.
#[test]
fn cutoffs_table() {
    let schema = Schema::new(vec![
        Column::new("family"),
        Column::new("nail_1").fixed(1),
        Column::new("nail_n"),
        Column::new("mmseqs_1").fixed(1),
        Column::new("mmseqs_n"),
    ])
    .style(indented());

    let mut table = Table::new(schema);
    table.row([
        Cell::from("1-cysPrx_C"),
        Cell::from(23.9),
        Cell::from(12097),
        Cell::from(33.0),
        Cell::from(29851),
    ]);
    table.row([
        Cell::from("10_blade"),
        Cell::from(30.8),
        Cell::from(4845),
        Cell::from(38.0),
        Cell::from(1387),
    ]);

    let expected = concat!(
        "# family     nail_1 nail_n mmseqs_1 mmseqs_n\n",
        "# ---------- ------ ------ -------- --------\n",
        "  1-cysPrx_C 23.9   12097  33.0     29851   \n",
        "  10_blade   30.8   4845   38.0     1387    \n",
    );

    assert_eq!(table.render(), expected);
}

/// `pid`'s results matrix: a label stacked from the top over each run, a
/// minimum width so the numbers under it have room, a right-aligned identity,
/// and one unbroken rule across the whole table.
#[test]
fn pid_results_matrix() {
    let schema = Schema::new(vec![
        Column::new("target"),
        Column::new("family"),
        Column::new("%id").align(Align::Right),
        Column::stacked(["prf", "blast"]).min_width(6).fixed(1),
        Column::stacked(["prf", "mmseqs", "s5.7", "ms2000"])
            .min_width(6)
            .fixed(1),
        Column::stacked(["seq", "diamond", "ultra", "sensitive"])
            .min_width(6)
            .fixed(1),
    ])
    .style(
        Style::default()
            .rule(Rule::Solid)
            .stack(Stack::Top)
            .trailing(Trailing::Keep),
    );

    let mut table = Table::new(schema);
    table.row([
        Cell::from("Q9HS26_HALSA/183-491"),
        Cell::from("Na_H_antiporter"),
        Cell::from("10%"),
        Cell::from(37.8),
        Cell::from(41.0),
        Cell::from(41.6),
    ]);
    table.row([
        Cell::from("B2A353_NATTJ/185-387"),
        Cell::from("FlaE"),
        Cell::from("11%"),
        Cell::from(34.5),
        Cell::from(33.0),
        Cell::missing(),
    ]);
    table.row([
        Cell::from("PDR3_YEAST/278-627"),
        Cell::from("Fungal_trans"),
        Cell::from("7%"),
        Cell::missing(),
        Cell::from(21.0),
        Cell::missing(),
    ]);

    let expected = concat!(
        "# target             family          %id prf    prf    seq      \n",
        "#                                        blast  mmseqs diamond  \n",
        "#                                               s5.7   ultra    \n",
        "#                                               ms2000 sensitive\n",
        "#---------------------------------------------------------------\n",
        "Q9HS26_HALSA/183-491 Na_H_antiporter 10% 37.8   41.0   41.6     \n",
        "B2A353_NATTJ/185-387 FlaE            11% 34.5   33.0   -        \n",
        "PDR3_YEAST/278-627   Fungal_trans     7% -      21.0   -        \n",
    );

    assert_eq!(table.render(), expected);
}

/// `nail`'s hit table: a label split into words and stacked from the bottom,
/// so the last word sits over the numbers, and an E-value in scientific
/// notation.
#[test]
fn nail_hit_table() {
    let schema = Schema::new(vec![
        Column::new("target"),
        Column::new("query"),
        Column::stacked(["target", "start"]),
        Column::stacked(["target", "end"]),
        Column::stacked(["query", "start"]),
        Column::stacked(["query", "end"]),
        Column::new("score").fixed(1),
        Column::stacked(["comp", "bias"]).fixed(1),
        Column::new("evalue").scientific(1),
        Column::stacked(["cell", "frac"]).fixed(3),
    ])
    .style(Style::default().trailing(Trailing::Keep));

    let mut table = Table::new(schema);

    for (target, tstart, tend, score, bias, evalue, frac) in [
        (
            "F1MV99|reviewed|Somatostatin",
            58,
            306,
            175.8,
            7.5,
            1.0e-53,
            0.066,
        ),
        (
            "O08858|reviewed|Somatostatin",
            54,
            303,
            173.0,
            7.8,
            7.7e-53,
            0.069,
        ),
        (
            "C3ZQF9|reviewed|QRFP-like",
            64,
            326,
            149.3,
            8.6,
            1.3e-45,
            0.070,
        ),
    ] {
        table.row([
            Cell::from(target),
            Cell::from("7tm_1-consensus"),
            Cell::from(tstart),
            Cell::from(tend),
            Cell::from(1),
            Cell::from(260),
            Cell::from(score),
            Cell::from(bias),
            Cell::from(evalue),
            Cell::from(frac),
        ]);
    }

    // nail leaves one more space at the end of each header
    // line than its data lines carry, an inconsistency in its
    // writer rather than a property of the format, so this is
    // its shape without that space
    let expected = concat!(
        "#                                            target target query query       comp         cell \n",
        "# target                     query           start  end    start end   score bias evalue  frac \n",
        "# -------------------------- --------------- ------ ------ ----- ----- ----- ---- ------- -----\n",
        "F1MV99|reviewed|Somatostatin 7tm_1-consensus 58     306    1     260   175.8 7.5  1.0e-53 0.066\n",
        "O08858|reviewed|Somatostatin 7tm_1-consensus 54     303    1     260   173.0 7.8  7.7e-53 0.069\n",
        "C3ZQF9|reviewed|QRFP-like    7tm_1-consensus 64     326    1     260   149.3 8.6  1.3e-45 0.070\n",
    );

    assert_eq!(table.render(), expected);
}
