//! Format-wide decisions: the marker, the rule, and what a missing value says.

/// How the `#` marker relates to the first column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Marker {
    /// `# ` opens the header's first cell and counts toward that column's
    /// width. Data rows start at the left margin.
    #[default]
    Absorb,
    /// `# ` opens header lines and two spaces open data rows, so column widths
    /// are the content's alone.
    Indent,
}

/// The line between the header and the rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rule {
    /// A run of dashes per column, so the rule shows where the columns divide.
    #[default]
    Dashes,
    /// One unbroken run of dashes across the whole table.
    Solid,
    /// Nothing between the header and the rows.
    None,
}

/// Which end of a column a stacked label's words hang from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stack {
    /// The last word sits on the line above the rows, so a short label is
    /// adjacent to the values it names.
    #[default]
    Bottom,
    /// The first word sits on the topmost header line.
    Top,
}

/// What becomes of a line's padding once the last cell is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Trailing {
    /// Cut it off, so no line ends in whitespace.
    #[default]
    Trim,
    /// Leave it, so the last column is padded like the rest.
    Keep,
}

/// Everything about a table's shape that is not a column.
#[derive(Clone, Debug)]
pub struct Style {
    pub(crate) marker: Marker,
    pub(crate) rule: Rule,
    pub(crate) stack: Stack,
    pub(crate) missing: String,
    pub(crate) trailing: Trailing,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            marker: Marker::default(),
            rule: Rule::default(),
            stack: Stack::default(),
            missing: "-".to_string(),
            trailing: Trailing::default(),
        }
    }
}

impl Style {
    /// How the `#` marker relates to the first column.
    pub fn marker(mut self, marker: Marker) -> Style {
        self.marker = marker;
        self
    }

    /// What goes between the header and the rows.
    pub fn rule(mut self, rule: Rule) -> Style {
        self.rule = rule;
        self
    }

    /// Which end of a column a stacked label's words hang from.
    pub fn stack(mut self, stack: Stack) -> Style {
        self.stack = stack;
        self
    }

    /// What a cell with no value is written as. `-` by default.
    pub fn missing(mut self, missing: impl Into<String>) -> Style {
        self.missing = missing.into();
        self
    }

    /// What becomes of the padding at the end of a line.
    pub fn trailing(mut self, trailing: Trailing) -> Style {
        self.trailing = trailing;
        self
    }
}
