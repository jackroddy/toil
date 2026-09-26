//! `#=` lines: a key and its words, for a parser rather than a reader.

use crate::cell::Cell;

/// One `#=` line, read as a key and the words after it.
///
/// Words are separated by spaces, and one holding the placeholder reads as
/// missing. A last word with spaces in it comes back whole through
/// [`rest`](Self::rest).
#[derive(Clone, Copy, Debug)]
pub struct MetaRow<'a> {
    key: &'a str,
    words: &'a str,
    missing: &'a str,
}

impl<'a> MetaRow<'a> {
    /// Read `line` as a `#=` line, or `None` if it is not one.
    pub(crate) fn parse(line: &'a str, missing: &'a str) -> Option<MetaRow<'a>> {
        let text = line.strip_prefix("#=")?.trim();
        let (key, words) = text.split_once(' ').unwrap_or((text, ""));
        Some(MetaRow {
            key,
            words: words.trim_start(),
            missing,
        })
    }

    /// The first word, naming what the line holds.
    pub fn key(&self) -> &'a str {
        self.key
    }

    /// How many words follow the key.
    pub fn len(&self) -> usize {
        self.words.split_whitespace().count()
    }

    /// Whether no words follow the key.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Word `i` after the key, or `None` past the end or where it holds the
    /// placeholder.
    pub fn get(&self, i: usize) -> Option<&'a str> {
        self.words
            .split_whitespace()
            .nth(i)
            .filter(|&word| word != self.missing)
    }

    /// The words from `i` to the end of the line, spaces and all, or `None`
    /// past the end or where they are only the placeholder.
    pub fn rest(&self, i: usize) -> Option<&'a str> {
        let mut rest = self.words;
        for _ in 0..i {
            let (_, after) = rest.split_once(char::is_whitespace)?;
            rest = after.trim_start();
        }
        Some(rest).filter(|&rest| !rest.is_empty() && rest != self.missing)
    }

    /// Exactly `N` words after the key, each as [`get`](Self::get) reads it,
    /// or `None` if the line holds any other number.
    pub fn exactly<const N: usize>(&self) -> Option<[Option<&'a str>; N]> {
        if self.len() != N {
            return None;
        }
        let mut words = self.words.split_whitespace();
        Some(std::array::from_fn(|_| {
            words.next().filter(|&word| word != self.missing)
        }))
    }
}

/// A `#=` line holding `key` and `words`, with no line ending.
pub(crate) fn line(
    key: &str,
    words: impl IntoIterator<Item = impl Into<Cell>>,
    missing: &str,
) -> String {
    let mut out = String::from("#=");
    if !key.is_empty() {
        out.push(' ');
        out.push_str(key);
    }
    for word in words {
        out.push(' ');
        word.into().word(missing, &mut out);
    }
    out
}
