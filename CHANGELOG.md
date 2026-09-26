# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `Reader` cuts a row in one pass. A row of left-aligned cells, each padded
  with spaces to its width or running past it, is scanned to each next space
  as a split on whitespace would scan it; any other row takes the general
  cut, which counts characters one by one only when the row is not ASCII. On
  a table of 3.4 million rows and 17 columns, rows took 0.33 s where they
  took 0.69 s.
- `Reader` settles how to cut the rows from the first 64, where it took 256,
  so opening that table takes 0.06 s rather than 0.21 s.

## [0.2.0] - 2026-09-25

### Added

- `MetaRow`, a `#=` line read as a key and the words after it: `get` for one
  word, `exactly` for a fixed number of them, and `rest` for the text from a
  word to the end of the line. A word holding the placeholder reads as
  missing.
- `Table::meta_rows`, for every `#=` line above the header and between the
  rows, in the order they were written.
- `Entry::Meta`, for a `#=` line the `Reader` finds between the rows.

### Changed

- `Table::meta` and `Stream::meta` take a key and its words in place of one
  string: `meta("tool", [name, version])` where it was
  `meta(format!("tool {name} {version}"))`. Each word goes through
  `Into<Cell>`, and a missing or empty word writes the placeholder. The
  bytes written are the same.
- `Entry::Comment` now holds only plain `#` lines, since a `#=` line comes
  back as `Entry::Meta`.

### Removed

- `Table::meta_lines`, which `Table::meta_rows` replaces.

## [0.1.1] - 2026-09-25

### Added

- `Reader`, which reads a table a line at a time: the header first, then each
  row as byte cells borrowed from one reused buffer, and each `#` line where
  it sat among the rows. It settles how to cut every row from the first 256.

### Fixed

- `Table::parse` read a `Stream` table's ragged columns and stacked labels
  right only until a cell overran its column. After an overrun it read every
  column as plain, which shifted the header.

## [0.1.0] - 2026-09-25

### Added

- `Table`, which holds every row and lays them out once, and `Stream`, which
  writes rows as they arrive at widths fixed when it is made.
- `Schema` and `Column`: a column's label, alignment, minimum width, stacked
  header words, fixed or scientific number formatting, and whether it is left
  ragged.
- `Style`, for the comment marker, the rule under the header, how stacked
  labels sit, the missing-value placeholder, and trailing padding.
- `Widths`, so tables written at different times can line up under one header.
- `Table::parse` and `Table::read`, which turn text the crate wrote back into a
  `Table` that renders the same bytes.
- `Table::parse_missing` and `Table::read_missing`, for a table written with a
  placeholder other than `-`.
- `Table::get`, for a cell by row and label, and `Table::meta_lines`, for the
  text of the `#=` lines.
- `Table::schema`, so a table read back can be handed to `Stream::continued`
  and appended to.
- `Line::cell`, which writes any `Cell` into a stream line, integers and
  `Option`s included, and honours a cell's own alignment.

[Unreleased]: https://github.com/jackroddy/toil/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jackroddy/toil/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/jackroddy/toil/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jackroddy/toil/releases/tag/v0.1.0
