# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
