pub mod cursor;
pub mod rope;
// pub mod string;

pub use cursor::Cursor;

use std::ops::{Add, RangeBounds, Sub};

pub trait TextStorage<'a> {
    type Slice: TextStorage<'a>;
    type GraphemeIterator: Iterator<Item = Self::Slice>;

    fn len_bytes(&self) -> ByteIndex;
    fn len_chars(&self) -> CharIndex;
    fn len_lines(&self) -> LineIndex;
    fn len_graphemes(&self) -> usize;

    fn char_to_line(&self, char_index: CharIndex) -> LineIndex;
    fn line_to_char(&self, line_index: LineIndex) -> CharIndex;

    fn char(&self, char_index: CharIndex) -> char;
    fn graphemes(&'a self) -> Self::GraphemeIterator;
    fn line(&'a self, line_index: LineIndex) -> Self::Slice;
    fn slice(&'a self, range: impl RangeBounds<usize>) -> Self::Slice;

    fn prev_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex;
    fn next_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex;
}

pub trait TextStorageMut<'a>: TextStorage<'a> {
    fn insert_char(&mut self, char_index: CharIndex, character: char);
    fn remove(&mut self, range: impl RangeBounds<usize>);
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct CharIndex(pub usize);

impl From<usize> for CharIndex {
    fn from(index: usize) -> Self {
        Self(index)
    }
}

impl From<CharIndex> for usize {
    fn from(value: CharIndex) -> Self {
        value.0
    }
}

impl CharIndex {
    fn saturating_sub(self, other: Self) -> Self {
        Self(usize::saturating_sub(self.0, other.0))
    }
}

impl Add for CharIndex {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for CharIndex {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct ByteIndex(pub usize);

impl From<usize> for ByteIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<ByteIndex> for usize {
    fn from(value: ByteIndex) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct LineIndex(pub usize);

impl From<usize> for LineIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<LineIndex> for usize {
    fn from(value: LineIndex) -> Self {
        value.0
    }
}

impl LineIndex {
    fn saturating_sub(self, other: Self) -> Self {
        Self(usize::saturating_sub(self.0, other.0))
    }
}

impl Add for LineIndex {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}
