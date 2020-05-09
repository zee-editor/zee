use std::ops::{Bound, RangeBounds};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete, Graphemes, UnicodeSegmentation};

use super::{ByteIndex, CharIndex, LineIndex, TextStorage, TextStorageMut};

impl<'a> TextStorage<'a> for String {
    type Slice = &'a str;
    type GraphemeIterator = Graphemes<'a>;

    fn len_bytes(&self) -> ByteIndex {
        String::len(self).into()
    }

    fn len_chars(&self) -> CharIndex {
        self.chars().count().into()
    }

    fn len_lines(&self) -> LineIndex {
        1.into()
    }

    fn len_graphemes(&self) -> usize {
        self.chars().count()
    }

    fn char_to_line(&self, char_index: CharIndex) -> LineIndex {
        0.into()
    }

    fn line_to_char(&self, line_index: LineIndex) -> CharIndex {
        0.into()
    }

    fn char(&self, char_index: CharIndex) -> char {
        // self.char[char_index.0]
        ' '
    }

    fn graphemes(&'a self) -> Self::GraphemeIterator {
        UnicodeSegmentation::graphemes(self.as_str(), true)
    }

    fn line(&'a self, line_index: LineIndex) -> Self::Slice {
        self.line(line_index.into())
    }

    fn slice(&'a self, range: impl RangeBounds<usize>) -> Self::Slice {
        let slice = String::as_str(self);
        match (range.start_bound(), range.end_bound()) {
            (Bound::Unbounded, Bound::Unbounded) => &slice[..],
            (Bound::Unbounded, Bound::Excluded(&end)) => &slice[..end],
            (Bound::Unbounded, Bound::Included(&end)) => &slice[..=end],

            (Bound::Included(&start), Bound::Unbounded) => &slice[start..],
            (Bound::Included(&start), Bound::Excluded(&end)) => &slice[start..end],
            (Bound::Included(&start), Bound::Included(&end)) => &slice[start..=end],

            (start, end) => panic!("Unsupported range type {:?} {:?}", start, end),
        }
    }

    fn prev_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        0.into()
        // prev_grapheme_boundary(&self.slice(..), char_index)
    }

    fn next_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        1.into()
        // next_grapheme_boundary(&self.slice(..), char_index)
    }
}

impl<'a> TextStorage<'a> for &'a str {
    type Slice = &'a str;
    type GraphemeIterator = Graphemes<'a>;

    fn len_bytes(&self) -> ByteIndex {
        str::len(self).into()
    }

    fn len_chars(&self) -> CharIndex {
        str::chars(self).count().into()
    }

    fn len_lines(&self) -> LineIndex {
        self.len_lines().into()
    }

    fn len_graphemes(&self) -> usize {
        self.chars().count()
    }

    fn char_to_line(&self, char_index: CharIndex) -> LineIndex {
        0.into()
    }

    fn line_to_char(&self, line_index: LineIndex) -> CharIndex {
        0.into()
    }

    fn char(&self, char_index: CharIndex) -> char {
        ' '
    }

    fn graphemes(&'a self) -> Self::GraphemeIterator {
        UnicodeSegmentation::graphemes(*self, true)
    }

    fn line(&'a self, line_index: LineIndex) -> Self::Slice {
        self.line(line_index.into())
    }

    fn slice(&'a self, range: impl RangeBounds<usize>) -> Self::Slice {
        let slice = self;
        match (range.start_bound(), range.end_bound()) {
            (Bound::Unbounded, Bound::Unbounded) => &slice[..],
            (Bound::Unbounded, Bound::Excluded(&end)) => &slice[..end],
            (Bound::Unbounded, Bound::Included(&end)) => &slice[..=end],

            (Bound::Included(&start), Bound::Unbounded) => &slice[start..],
            (Bound::Included(&start), Bound::Excluded(&end)) => &slice[start..end],
            (Bound::Included(&start), Bound::Included(&end)) => &slice[start..=end],

            (start, end) => panic!("Unsupported range type {:?} {:?}", start, end),
        }
    }

    fn prev_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        0.into()
        // prev_grapheme_boundary(&self.slice(..), char_index)
    }

    fn next_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        1.into()
        // next_grapheme_boundary(&self.slice(..), char_index)
    }
}
