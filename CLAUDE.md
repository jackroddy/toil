# 00claude

Everything Claude has been told about this project, in one place: how to work,
how to write code, what the project is, and what is still open.

---

## How to work

Inherited from `nail-benchmarks/00claude.md`, which is Jack's and applies to
his other repos unless he says otherwise. Confirm anything that turns out to
matter.

**Be brief.** Lead with the answer. No section headers, tables, or enumerated
caveats unless asked. Offer detail rather than supplying it.

**Edit and Write, never shell.** No `sed -i`, `awk`, `perl -pi`, python
heredocs, `tee`, `cat >`, redirection, or throwaway scripts for changing files.
Bash is for running things: builds, tests, git, inspection.

**Show changes as unified diffs** when proposing edits for sign-off — `-`/`+`
with context and a `@@ file:line` marker. Never a block of the finished
function; deletions have to be visible.

**Claims need evidence.** Before naming a defect, read the call sites and
confirm a real path reaches it.

**No unprompted tests.** **No speculative API.** **`anyhow::Result` always
qualified** (never `use anyhow::Result`).

**Comments: no comment is the default.** Short notes are plain and untagged;
longer ones escalate to `// what:`, `// why:`, `// how:`, `// note:`. Inline
comments are lowercase, no trailing period. Doc comments are full sentences.
See `nail-benchmarks/00claude.md` for the whole rule.

**Names are literal and flat.** Objects are not actors — no managers, handlers,
workers, watchers.

---

## Branches, releases, formatting, changelog

Adopted from `../michi/CLAUDE.md` on 2026-09-25.

**Branches.** All working changes go on `dev`. Do not open a feature branch
unless Jack asks for one. `main` holds releases and nothing else: push to it
when cutting a release, and leave it alone the rest of the time.

**Releases.** toil follows [semantic
versioning](https://semver.org/spec/v2.0.0.html). While the crate is below 1.0,
a breaking change takes the minor number. Tag every release, annotated, as
`vMAJOR.MINOR.PATCH`, on the commit that was published.

**Formatting.** `rustfmt` is the format. Run `cargo fmt` before committing.

**Changelog.** `CHANGELOG.md` follows [Keep a
Changelog](https://keepachangelog.com/en/1.1.0/). Anything that changes what
someone using the crate sees goes under `[Unreleased]` in the commit that
changes it.

---

## What this project is

`toil` is a general-purpose Rust crate for writing the padded, `#`-commented
text tables that the three symlinked projects each grew their own version of.

**The symlinks are read-only. Never write to them.** `nail/` (the aligner),
`pail/` (pipeline runner), `nail-benchmarks/` (the workspace that drives both)
are reference material and golden fixtures, nothing else. Do not port them onto
`toil`, do not add it as a dependency, do not edit them to fit. Integration is
Jack's to do, in those repos, later. Read them as much as you like.

`pail` and `libsail` are published on crates.io. Never argue about their design
from how one repo calls them.

---

## Where the crate stands

Built 2026-09-07 and passing. `#![warn(missing_docs)]` is on, so every public
item carries at least a line and a new one without it warns. `src/` is eight
files: `column.rs` (`Column`,
`Format`, `Schema`), `cell.rs` (`Cell`, `Row`, `Align`), `style.rs` (`Marker`,
`Rule`, `Stack`, `Trailing`), `render.rs` (`Widths`, `Table`, and the line
laying-out), `stream.rs` (`Stream`), `read.rs` (`Table::parse`, `ParseError`),
`reader.rs` (`Reader`, `Entry`, `Cells`: a table a line at a time, cut the way
`parse` cuts its first 256 rows), `lib.rs`.

`tests/read.rs` holds the reader to what the crate writes: every golden read
back and rendered to the same bytes, one table per combination of the style's
axes, and 5000 random tables from a fixed seed. Each of those also goes
through `Reader`, which has to give the same cells and comments.
`tests/reader.rs` reads a 1000-row `Stream` table past the sample.

`tests/golden.rs` holds seven tables the surveyed projects write today, with
their expected text embedded rather than read from those repositories. Between
them they cover both markers, the dashed and solid rules, both stackings, both
trailing modes, the ragged last column, and all three number formats.
`Rule::None` is reached by nothing. `tests/widths.rs` covers the shared floor,
which the goldens do not.

Two of the seven are the surveyed shape rather than the surveyed bytes. In both
cases the writer being copied is inconsistent with itself:

- `pid` shifts its header's first column one character right of its own data.
- `nail` leaves one more space at the end of each header line than its data
  lines carry.

Neither is a property of the format, and `toil` has no mode for either.

`examples/` has five runnable programs, one per shape: `read` (cells by label,
the `#=` lines, and appending to what was read), `analysis`, `blocks` (the shared floor, arriving a block at a time),
`stream` (header-sized widths against batch-measured ones), and `matrix`.

## The four implementations

### 1. `pail/src/table.rs`: 889 lines, the most complete

A `Sink` that writes the run summary as the pipeline runs. Columns are worked
out from the steps *before anything executes* (`Columns::of`, :188), so blocks
written at different times agree.

- `Cell { text, right }` (:443): per-cell alignment, `right` used only for the
  `|`/`||` continuation markers.
- `render(header, rows, show_header, floor)` (:471): widths are the max over
  header and cells, `widen` (:514), `write_row` (:520); last column never
  padded.
- **Width floor** (`Table::floor`): a pre-run measurement pass so blocks emitted
  one at a time still line up under a header printed once at the top.
- `Mode` (:51): `Whole` (buffer everything, render once), `Blocks { headers:
  Once | Each }`, `Ragged` (each block gets only its own columns). `Headers`
  (:36) is a separate axis, and the combinations that make no sense are
  unrepresentable.
- Columns are *composed*: fixed leading (`step`, `cmd`), then dynamic per-run
  field keys (sorted), then dynamic tags (rendered `x`/`-` for presence), then a
  fixed `METRICS` tail (:30), with `cpus` slotted second-to-last.
- Two row kinds: a step row and its command rows, and a step of one collapses
  into a single row.
- Rewrites the whole file on every flush (`Table::flush`).

```
# step     cmd   job wall(s) user(s) sys(s) cpu(%) max_rss exit status argv
# -------- ----- --- ------- ------- ------ ------ ------- ---- ------ ----
[1](setup) mkdir -   1.50    2.00    0.25   150%   2.00MiB 0    ok     /mkdir
[2](burn)  -     -   1.50    4.00    0.50   300%   2.00MiB -    -      -
        || a     1   1.50    2.00    0.25   150%   2.00MiB 0    ok     /a
```

### 2. `nail-benchmarks/benchmarks/util/src/tbl.rs`: 174 lines, the smallest

`Table { meta, headers, rows, ragged_last }` (:17) plus `render` (:43) and
`write` (:32). Every analysis in that repo goes through it. All cells are
`String`, left-aligned, widths max over header and column. `meta` is an
already-`#`-prefixed block the caller formats itself.

Distinctive: rows start with two literal spaces so cells sit under the header's
`# ` prefix, rather than the header's first column absorbing the marker (which
is what pail does). `ragged_last` leaves the final column unpadded and shrinks
its rule to the label width, for argv or a comma-joined score list.

### 3. `nail/libnail/src/output/output_tabular.rs`: 210 lines, the oldest

Schema-first rather than string-first. `Field` (:34) is an enum of extractable
columns; `Field::extract` (:55) pulls a `String` off an `Alignment`, `None`
becoming `-`. `TableFormat::new` (:91) derives column labels by regex-splitting
the CamelCase variant name into words, and `header()` (:167) stacks those words
into **multiple header rows** so `TargetStart` reads as `target` over `start`.
Precision is global: `BIT_P`/`F32_P`/`F64_P` `OnceLock`s.

Two named column sets live in the consumer (`output_stage.rs:13`, `:26`):
nail's own layout and BLAST's 12-column one, chosen by `--tbl-format`.

Known defect: `TableOutput` streams a row at a time and never learns real
widths: `update_widths`/`reset_widths` (:151, :160) have no callers, and
`output_stage.rs:97` still says `// TODO: state for column width formatting`.
So a value wider than its label pushes the row out of alignment. `nail`'s README
shows the older, properly padded output.

### 4. `nail-benchmarks/benchmarks/pid/src/parse.rs:139`, hand-rolled

Written directly with `write!` because `util::tbl` can't express it. A wide
matrix: two identifier columns, a right-aligned `%id`, then one column per run
with a **stacked header** built from splitting run names on `-`, a caller-set
`min_width` floor per column, and a single unbroken `#---...` rule across the
whole width instead of per-column dashes.

---

## What these tables have in common

- Plain text, fixed-width, space-separated, read with the eye first.
- Header lines are `#`-commented; a `#`-dashed rule sits under them.
- Data rows are indented so their first column lines up under the header label.
- Widths are the max over the column, computed after all cells exist.
- A missing value is `-`, never blank and never zero.
- One column (last, usually argv or a list) may be left unpadded.
- `meta`/preamble lines above the header carry run context.
- Numbers are pre-formatted to fixed precision by the caller (`{:.4}`, `{:.1e}`,
  `{:.2}s`, `2.00MiB`, `150%`).

## Where they differ: the axes a generic crate has to cover

- **When the data arrives.** All at once (`util::tbl`, pid) vs. streaming with
  the file rewritten as it grows (`pail`) vs. append-only with no second chance
  (`nail`'s `TableOutput`).
- **Whether widths can be known up front.** `pail` measures a floor before the
  run; `nail` cannot and settles for label widths.
- **Header shape.** One row, or several stacked rows for multi-word labels
  (`nail`, pid).
- **The `# ` marker.** Absorbed into the first column's width (`pail`, `nail`)
  or offset by two leading spaces on rows (`util::tbl`).
- **The rule.** Per-column dashes, or one unbroken line (pid).
- **Alignment.** Nearly all left; `pail` right-aligns markers, pid right-aligns
  `%id`.
- **Column set.** Static and named (`nail`), or discovered from the data
  (`pail`'s field keys and tags, pid's run columns).
- **Cell type.** Everything is already a `String` at the boundary in three of
  the four; only `nail` has a typed extractor.
- **Grouping.** `pail` has blocks and two row kinds; the rest are flat.

## Being read back

These are written for eyes, but three readers exist and constrain the format:

- `nail-benchmarks/benchmarks/util/src/manifest.rs` parses **pail's** table:
  first `#` line is the header, whitespace-split, zipped positionally with each
  row. It only works because argv is last (`:207`).
- `mgy/src/scores.rs:396` reads its own `#=` metadata lines back. `#=` is
  machine-readable metadata, plain `#` is the human header.
- `libsail::tbl` (external crate) parses **nail's**, BLAST's and HMMER's hit
  tables by fixed column index (`HitColumns`).

So: whitespace-split-and-zip has to keep working, and a ragged/space-bearing
column has to stay last.

Since 2026-09-24 `toil` reads its own tables back too; see the reader under
Decisions.

## Adjacent, probably out of scope

`nail/nail/src/stats.rs:490-615` holds an aligned `├─`/`└─` tree of `label: value`
with computed label widths. Same width-measuring instinct, different shape.
`pail/src/progress.rs` is a live terminal block, aligned but ephemeral.
`pail/src/fmt.rs` holds `secs`, `bytes`, `cpu_pct`, `dash`. Cell formatting helpers,
which a table crate may or may not want to own.

---

## Decisions

Settled 2026-09-07. Full design in
`~/.claude-personal/plans/propose-a-design-that-adaptive-clover.md`.

- **Name: `toil`**, for table output/input library. Renamed from `tabl` on
  2026-09-25, once the crate read tables as well as wrote them.
- **Per-column numeric formatting is in scope; unit formatting is not.** A
  `Column` carries precision and notation, replacing `nail`'s global
  `F32_P`/`F64_P` `OnceLock`s and the scattered `format!("{:.4}", x)`.
  `pail::fmt`'s `bytes`/`secs`/`cpu_pct` stay out. `pail::progress` uses them
  too and must not depend on a table crate to print a byte size.
- **Rows are untyped.** No generic over the caller's data type. `nail`'s
  `Field::extract` already returns `Option<String>`, and `Option<T> → Cell`
  mapping `None` to the missing placeholder makes that call site one line.
- **Streaming is first-class**, as `Stream<W>`, which takes its `Widths` at
  construction and never revises them. That is what makes `nail`'s bug
  unrepresentable.
- **`pail`'s floor generalises; its blocks do not.** A shared `Widths` value is
  the crate's organising idea. The step/command row split, collapse-when-one,
  `Metrics` and `status_word` are `pail`'s domain and stay there.
- **A reader, for everything the crate writes.** Reversed on 2026-09-24, when
  nail-benchmarks dropped its own table module and needed a parser to read
  `set.tbl`, `ledger.tbl` and michi's manifest. The parser goes here because it
  is generic to any table `toil` writes. `Table::parse` recovers the cells by
  position, and the style, labels and column widths with them, so a parsed
  table renders back to the text it came from. Two settled cases where two
  tables used to write the same bytes:
  - A comment directly above the header looked like the top line of a stacked
    label. `Table` and `Stream` now write a bare `#` between them. That is the
    only change to output, and no golden has such a comment.
  - A cell holding the placeholder reads back as missing.

  Which columns are ragged, and with no rows which marker was used, are not
  written down, so the reader searches for the answer that lays the text out
  again exactly, and tries every answer for tables of 12 columns or fewer when
  the search misses. Some tables still cannot be told apart from their bytes:
  an empty word inside a stacked label, an empty last column ruled zero wide,
  an empty cell in a ragged column, a label or a non-last cell with a space in
  it, and rarely a stacked label over ragged columns (about 1 in 2500 random
  tables). The reader then returns a table that renders the same bytes.

  Rows that do not sit under the rule even at its own widths are split on
  whitespace. That is for other writers: michi's manifest drifts a column off
  its rule in places, and nail-benchmarks reads it through `Table::parse`.
- **Zero dependencies**, and rendering has no error type: it is infallible and
  returns `String`, and only `write` returns `io::Result`. Reading has one,
  `ParseError`, and `Table::read` hands it back as `io::ErrorKind::InvalidData`.
