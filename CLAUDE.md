# 00claude

Everything Claude has been told about this project, in one place: how to work,
how to write code, what the project is, and what is still open.

Open work and facts about the code live in foam, not here: `foam ready` for
what to do, `foam memories` for what a session would otherwise rediscover.

---

## How to work

These are Jack's and apply to his other repos unless he says otherwise.
Confirm anything that turns out to matter.

**Rules go in this file.** Nothing Jack teaches in this project goes in the
global CLAUDE.md. The harness memory stays empty: a fact goes to `foam
remember`, work to `foam create`.

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

**Performance claims come with the measurements.** After performance work,
report in chat before any talk of committing: the call timed, the data (rows,
columns, bytes, row shape), the build, and the baseline, which is the
published previous version measured in the same run. Interleave repeated runs
of old and new and give the spread, so a small gain can be told from noise;
say plainly when a difference is within it. Say which change is responsible
for which gain, and when a figure is extrapolated rather than timed.

**No unprompted tests.** **No speculative API.** **`anyhow::Result` always
qualified** (never `use anyhow::Result`).

**Comments: no comment is the default.** Short notes are plain and untagged;
longer ones escalate to `// what:`, `// why:`, `// how:`, `// note:`. Inline
comments are lowercase, no trailing period. Doc comments are full sentences.
The `rust-comments` skill has the whole rule.

**Names are literal and flat.** Objects are not actors — no managers, handlers,
workers, watchers.

**Judge by every table the format allows.** When weighing an optimization or a
feature, count left- and right-aligned columns, empty cells, non-ASCII text,
both markers, all three rules, ragged columns, small tables and huge ones. Do
not dismiss a case because the one current caller does not use it. A benchmark
from one caller is a good test case, not the target.

**A Decisions entry below may be a Claude session's, not Jack's.** Before
building on one, especially one that drives a lot of complexity, confirm he
chose it. When a feature starts needing searches, backstops and special
cases, stop and ask what the caller needs before adding another patch.

**When Jack says he will write the commit message,** stage what he asks for
and wait for his text instead of drafting another one.

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

**Breaking changes are fine below 1.0.** When breaking the API gives the better
design, break it and bump the minor number. Do not bend a design to stay
additive, and do not hold a break back for sign-off. That covers files as well
as the API: when a change means a table an earlier toil wrote no longer reads
the same, do not add a fallback to keep it reading. Name each break under
Changed or Removed in the changelog, so callers know what to update.

**Formatting.** `rustfmt` is the format. Run `cargo fmt` before committing.

**Changelog.** `CHANGELOG.md` follows [Keep a
Changelog](https://keepachangelog.com/en/1.1.0/). Anything that changes what
someone using the crate sees goes under `[Unreleased]` in the commit that
changes it.

---

## What this project is

`toil` is a general-purpose Rust crate for writing the padded, `#`-commented
text tables that `nail`, `pail` and `nail-benchmarks` each grew their own
writer for, and for reading them back.

**The sibling checkouts are read-only from here.** `../nail`,
`../nail-benchmarks` and `../michi` are reference material and golden
fixtures. Read them as much as you like; never edit them. `nail-benchmarks`
and `michi` depend on `toil`, and moving them to a new version is Jack's to
do, in those repos.

`pail` and `libsail` are published on crates.io. Never argue about their design
from how one repo calls them.

---

## Where the crate stands

`#![warn(missing_docs)]` is on, so every public item carries at least a line
and a new one without it warns. `src/` is nine files: `column.rs` (`Column`,
`Format`, `Schema`), `cell.rs` (`Cell`, `Row`, `Align`), `style.rs` (`Marker`,
`Rule`, `Stack`, `Trailing`), `render.rs` (`Widths`, `Table`, and the line
laying-out), `stream.rs` (`Stream`), `read.rs` (`Table::parse`, `ParseError`),
`reader.rs` (`Reader`, `Entry`, `Cells`: a table a line at a time, split the
way `parse` splits it), `meta.rs` (`MetaRow`, a `#=` line read as a key and
words), `lib.rs`.

`tests/read.rs` holds the reader to what the crate writes: the cells, labels
and `#` lines of every golden, one table per combination of the style's
axes, 5000 random tables from a fixed seed, and 3000 more with `min_width`
columns, more columns and now and then many rows. Each of those also goes
through `Reader`, which has to give the same cells and comments.
`tests/reader.rs` reads a 1000-row `Stream` table. `tests/line.rs` holds
`Stream::line` byte for byte to `Stream::row`.

`tests/golden.rs` holds seven tables the surveyed projects wrote before
`toil`, with their expected text embedded rather than read from those
repositories. Between them they cover both markers, the dashed and solid
rules, both stackings, both trailing modes, the ragged last column, and all
three number formats. `Rule::None` is reached by nothing. `tests/widths.rs`
covers the shared floor, which the goldens do not.

Two of the seven are the surveyed shape rather than the surveyed bytes. In both
cases the writer being copied is inconsistent with itself:

- `pid` shifts its header's first column one character right of its own data.
- `nail` leaves one more space at the end of each header line than its data
  lines carry.

Neither is a property of the format, and `toil` has no mode for either.

`examples/` has five runnable programs, one per shape: `read` (cells by label,
the `#=` lines, and appending to what was read), `analysis`, `blocks` (the
shared floor, arriving a block at a time), `stream` (header-sized widths
against batch-measured ones), and `matrix`.

---

## Decisions

Settled 2026-09-07 unless dated otherwise. Foam issue `toil-c4bdb0` is the
review of which of these are Jack's.

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
  is generic to any table `toil` writes.

  Reading is for the data: the cells, the labels, the `#` lines, and the
  widths, marker and padding that appending with `Stream::continued` needs.
  **It is not for rebuilding the
  writer's style**, and a parsed table need not render the text it came
  from. Jack never wanted that; an earlier session wrote it in here as a
  goal, and the search it needed was removed on 2026-09-26.

  A row is split on spaces, the last column taking the rest of the line, so
  alignment, raggedness, overruns and the marker do not matter to it, and a
  row that drifts off its rule reads the same. A header word belongs to the
  column it sits in: a dashed rule gives each column's start, and with a
  solid rule or none the header words themselves do. The writer keeps both
  exact: an empty cell is written as the placeholder, a ragged column's
  header words are padded to its label, and a bare `#` goes between a
  comment and the header below it. What a split cannot read is a space in a
  cell before the last column, which the format does not allow.
- **Dependencies are allowed.** The crate started with none. Since 2026-09-25
  Jack allows them where they earn their place, starting with `wide` for
  portable SIMD in the reader. Still prefer the standard library when it
  does the job about as well.
- **Rendering has no error type**: it is infallible and
  returns `String`, and only `write` returns `io::Result`. Reading has one,
  `ParseError`, and `Table::read` hands it back as `io::ErrorKind::InvalidData`.
