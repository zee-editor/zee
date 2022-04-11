use ropey::{str_utils::byte_to_char_idx, Rope, RopeSlice};
use std::{
    cmp,
    ops::{Add, Range, Sub},
};
use zi::unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use super::{ensure_trailing_newline_with_content, RopeGraphemes};
use crate::syntax::OpaqueDiff;

#[derive(Clone, Debug, PartialEq)]
pub struct Cursor {
    /// Char range under cursor, aligned to extended grapheme clusters.
    range: Range<CharIndex>,
    /// The start of a selection if in select mode, ending at `range.start` or
    /// `range.end`, depending on direction. Aligned to extended grapheme
    /// clusters.
    selection: Option<CharIndex>,
    visual_horizontal_offset: Option<usize>,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            range: CharIndex(0)..CharIndex(1),
            selection: None,
            visual_horizontal_offset: None,
        }
    }

    pub fn range(&self) -> &Range<CharIndex> {
        &self.range
    }

    pub fn selection(&self) -> Range<CharIndex> {
        match self.selection {
            Some(selection) if selection > self.range.start => self.range.start..selection,
            Some(selection) if selection < self.range.start => selection..self.range.start,
            _ => self.range.clone(),
        }
    }

    pub fn column_offset(&self, text: &Rope) -> usize {
        let char_line_start = text.line_to_char(text.cursor_to_line(self));
        super::graphemes::width(&text.slice(char_line_start..self.range.start.0))
    }

    pub fn reconcile(&mut self, new_text: &Rope, diff: &OpaqueDiff) {
        let OpaqueDiff {
            char_index,
            old_char_length,
            new_char_length,
            ..
        } = *diff;

        let modified_range =
            CharIndex(char_index)..CharIndex(cmp::max(old_char_length, new_char_length));

        // The edit starts after the end of the cursor, nothing to do
        if modified_range.start >= self.range.end {
            return;
        }

        // The edit ends before the start of the cursor
        if modified_range.end <= self.range.start {
            let (start, end) = (self.range.start.0, self.range.end.0);
            if old_char_length > new_char_length {
                let length_change = old_char_length - new_char_length;
                self.range = CharIndex(start.saturating_sub(length_change))
                    ..CharIndex(end.saturating_sub(length_change));
            } else {
                let length_change = new_char_length - old_char_length;
                self.range = CharIndex(start + length_change)..CharIndex(end + length_change);
            };
        }

        // Otherwise, the change overlaps with the cursor
        let grapheme_start = prev_grapheme_boundary(
            new_text.slice(..),
            CharIndex(cmp::min(self.range.end.0, new_text.len_chars())),
        );
        let grapheme_end = next_grapheme_boundary(new_text.slice(..), grapheme_start);
        self.range = grapheme_start..grapheme_end
    }

    #[cfg(test)]
    pub fn end_of_buffer(text: &Rope) -> Self {
        Self {
            range: prev_grapheme_boundary(text.slice(..), CharIndex(text.len_chars()))
                ..CharIndex(text.len_chars()),
            visual_horizontal_offset: None,
            selection: None,
        }
    }

    pub fn begin_selection(&mut self) {
        self.selection = Some(self.range.start)
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn select_all(&mut self, text: &Rope) {
        self.move_to_start_of_buffer(text);
        self.selection = Some(CharIndex(text.len_chars()));
    }

    pub fn move_up(&mut self, text: &Rope) {
        let current_line_index = text.char_to_line(self.range.start.0);
        if current_line_index == 0 {
            return;
        }
        self.move_vertically(text, current_line_index, current_line_index - 1);
    }

    pub fn move_up_n(&mut self, text: &Rope, n: usize) {
        for _ in 0..n {
            self.move_up(text);
        }
    }

    pub fn move_down(&mut self, text: &Rope) {
        let current_line_index = text.char_to_line(self.range.start.0);
        if current_line_index >= text.len_lines() {
            return;
        }
        self.move_vertically(text, current_line_index, current_line_index + 1);
    }

    pub fn move_down_n(&mut self, text: &Rope, n: usize) {
        for _ in 0..n {
            self.move_down(text);
        }
    }

    pub fn move_left(&mut self, text: &Rope) {
        let previous_grapheme_start = prev_grapheme_boundary(text.slice(..), self.range.start);
        if previous_grapheme_start == CharIndex(0) && self.range.start == CharIndex(0) {
            return;
        }

        self.range = previous_grapheme_start..self.range.start;
        self.visual_horizontal_offset = None;
    }

    pub fn move_right(&mut self, text: &Rope) {
        let grapheme_start = self.range.end;
        let grapheme_end = next_grapheme_boundary(text.slice(..), self.range.end);
        if grapheme_start != grapheme_end {
            self.range = grapheme_start..grapheme_end;
        }
        self.visual_horizontal_offset = None;
    }

    pub fn move_right_n(&mut self, text: &Rope, n: usize) {
        for _ in 0..n {
            self.move_right(text);
        }
    }

    pub fn move_to_start_of_line(&mut self, text: &Rope) {
        let line_index = text.char_to_line(self.range.start.0);
        let char_index = CharIndex(text.line_to_char(line_index));
        self.range = char_index..next_grapheme_boundary(text.slice(..), char_index);
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_end_of_line(&mut self, text: &Rope) {
        self.range = {
            let line_index = text.char_to_line(cmp::min(
                text.len_chars().saturating_sub(1),
                self.range.start.0,
            ));
            let line_length = text.line(line_index).len_chars();
            let char_index = text.line_to_char(line_index);
            CharIndex(char_index + line_length).saturating_sub(CharIndex(1))
                ..CharIndex(char_index + line_length)
        };
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_start_of_buffer(&mut self, text: &Rope) {
        self.range = CharIndex(0)..next_grapheme_boundary(text.slice(..), CharIndex(0));
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_end_of_buffer(&mut self, text: &Rope) {
        self.range = prev_grapheme_boundary(text.slice(..), CharIndex(text.len_chars()))
            ..CharIndex(text.len_chars());
        self.visual_horizontal_offset = None;
    }

    pub fn insert_char(&mut self, text: &mut Rope, character: char) -> OpaqueDiff {
        text.insert_char(self.range.start.0, character);
        ensure_trailing_newline_with_content(text);
        OpaqueDiff::new(
            text.char_to_byte(self.range.start.0),
            0,
            character.len_utf8(),
            self.range.start.0,
            0,
            1,
        )
    }

    pub fn insert_chars(
        &mut self,
        text: &mut Rope,
        characters: impl IntoIterator<Item = char>,
    ) -> OpaqueDiff {
        let mut num_bytes = 0;
        let mut num_chars = 0;
        characters
            .into_iter()
            .enumerate()
            .for_each(|(offset, character)| {
                text.insert_char(self.range.start.0 + offset, character);
                num_bytes += character.len_utf8();
                num_chars += 1;
            });
        ensure_trailing_newline_with_content(text);
        OpaqueDiff::new(
            text.char_to_byte(self.range.start.0),
            0,
            num_bytes,
            self.range.start.0,
            0,
            num_chars,
        )
    }

    pub fn delete_forward(&mut self, text: &mut Rope) -> DeleteOperation {
        if text.len_chars() == 0 || self.range.start.0 == text.len_chars().saturating_sub(1) {
            return DeleteOperation::empty();
        }
        let diff = OpaqueDiff::new(
            text.char_to_byte(self.range.start.0),
            text.char_to_byte(self.range.end.0) - text.char_to_byte(self.range.start.0),
            0,
            self.range.start.0,
            self.range.end.0 - self.range.start.0,
            0,
        );
        text.remove(self.range.start.0..self.range.end.0);

        let grapheme_start = self.range.start;
        let grapheme_end = next_grapheme_boundary(text.slice(..), self.range.start);
        let deleted = text.slice(grapheme_start.0..grapheme_end.0).into();
        if grapheme_start < grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = CharIndex(0)..CharIndex(1)
        }
        ensure_trailing_newline_with_content(text);
        DeleteOperation { diff, deleted }
    }

    pub fn delete_backward(&mut self, text: &mut Rope) -> DeleteOperation {
        if self.range.start.0 > 0 {
            self.move_left(text);
            self.delete_forward(text)
        } else {
            DeleteOperation::empty()
        }
    }

    pub fn delete_line(&mut self, text: &mut Rope) -> DeleteOperation {
        if text.len_chars() == 0 {
            return DeleteOperation::empty();
        }

        // Delete line
        let line_index = text.char_to_line(self.range.start.0);
        let delete_range_start = text.line_to_char(line_index);
        let delete_range_end = text.line_to_char(line_index + 1);
        let deleted = text.slice(delete_range_start..delete_range_end).into();
        let diff = OpaqueDiff::new(
            text.char_to_byte(delete_range_start),
            text.char_to_byte(delete_range_end) - text.char_to_byte(delete_range_start),
            0,
            delete_range_start,
            delete_range_end - delete_range_start,
            0,
        );
        text.remove(delete_range_start..delete_range_end);

        // Update cursor position
        let grapheme_start =
            CharIndex(text.line_to_char(cmp::min(line_index, text.len_lines().saturating_sub(2))));
        let grapheme_end = next_grapheme_boundary(text.slice(..), grapheme_start);
        if grapheme_start != grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = CharIndex(0)..CharIndex(1)
        }

        DeleteOperation { diff, deleted }
    }

    pub fn delete_selection(&mut self, text: &mut Rope) -> DeleteOperation {
        if text.len_chars() == 0 {
            return DeleteOperation::empty();
        }

        // Delete selection
        let selection = self.selection();
        let deleted = text.slice(selection.start.0..selection.end.0).into();
        let diff = OpaqueDiff::new(
            text.char_to_byte(selection.start.0),
            text.char_to_byte(selection.end.0) - text.char_to_byte(selection.start.0),
            0,
            selection.start.0,
            selection.end.0 - selection.start.0,
            0,
        );
        text.remove(selection.start.0..selection.end.0);

        // Update cursor position
        let grapheme_start = cmp::min(
            self.range.start,
            prev_grapheme_boundary(text.slice(..), CharIndex(text.len_chars())),
        );
        let grapheme_end = next_grapheme_boundary(text.slice(..), grapheme_start);
        if grapheme_start != grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = CharIndex(0)..CharIndex(1)
        }
        self.clear_selection();
        self.visual_horizontal_offset = None;

        DeleteOperation { diff, deleted }
    }

    pub fn sync(&mut self, current_text: &Rope, new_text: &Rope) {
        let current_line = current_text.char_to_line(self.range.start.0);
        let current_line_offset = self.range.start.0 - current_text.line_to_char(current_line);

        let new_line = cmp::min(current_line, new_text.len_lines().saturating_sub(1));
        let new_line_offset = cmp::min(
            current_line_offset,
            new_text.line(new_line).len_chars().saturating_sub(1),
        );
        let grapheme_end = next_grapheme_boundary(
            new_text.slice(..),
            CharIndex(new_text.line_to_char(new_line) + new_line_offset),
        );
        let grapheme_start = prev_grapheme_boundary(new_text.slice(..), grapheme_end);

        self.range = if grapheme_start != grapheme_end {
            grapheme_start..grapheme_end
        } else {
            CharIndex(0)..CharIndex(1)
        };
        self.visual_horizontal_offset = None;
        self.selection = None;
    }

    fn move_vertically(&mut self, text: &Rope, current_line_index: usize, new_line_index: usize) {
        if new_line_index >= text.len_lines() {
            return;
        }

        let current_line_start = text.line_to_char(current_line_index);
        let cursor_range_start = self.range.start;
        let current_visual_x = self.visual_horizontal_offset.get_or_insert_with(|| {
            super::graphemes::width(&text.slice(current_line_start..cursor_range_start.0))
        });

        let new_line = text.line(new_line_index);
        let mut graphemes = RopeGraphemes::new(&new_line);
        let mut new_visual_x = 0;
        for grapheme in &mut graphemes {
            let width = super::graphemes::width(&grapheme);
            if new_visual_x + width > *current_visual_x {
                break;
            }
            new_visual_x += width;
        }

        let new_line_offset = CharIndex(
            text.byte_to_char(text.line_to_byte(new_line_index) + graphemes.cursor.cur_cursor()),
        );

        self.range = if new_visual_x <= *current_visual_x {
            let grapheme_start = prev_grapheme_boundary(text.slice(..), new_line_offset);
            let grapheme_end = next_grapheme_boundary(text.slice(..), grapheme_start);
            grapheme_start..grapheme_end
        } else {
            let grapheme_end = next_grapheme_boundary(text.slice(..), new_line_offset);
            let grapheme_start = prev_grapheme_boundary(text.slice(..), grapheme_end);
            grapheme_start..grapheme_end
        }
    }
}

pub struct DeleteOperation {
    pub diff: OpaqueDiff,
    pub deleted: Rope,
}

impl DeleteOperation {
    fn empty() -> Self {
        Self {
            diff: OpaqueDiff::empty(),
            deleted: Rope::new(),
        }
    }
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

trait RopeCursorExt {
    fn cursor_to_line(&self, cursor: &Cursor) -> usize;
}

impl RopeCursorExt for Rope {
    fn cursor_to_line(&self, cursor: &Cursor) -> usize {
        self.char_to_line(cursor.range.start.0)
    }
}

/// Finds the previous grapheme boundary before the given char position.
fn prev_grapheme_boundary(slice: RopeSlice, char_index: CharIndex) -> CharIndex {
    // Bounds check
    debug_assert!(char_index.0 <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_index = slice.char_to_byte(char_index.0);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_index, mut chunk_char_index, _) =
        slice.chunk_at_byte(byte_index);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_index, slice.len_bytes(), true);

    // Find the previous grapheme cluster boundary.
    loop {
        match gc.prev_boundary(chunk, chunk_byte_index) {
            Ok(None) => return CharIndex(0),
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_index);
                return CharIndex(chunk_char_index + tmp);
            }
            Err(GraphemeIncomplete::PrevChunk) => {
                let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_index - 1);
                chunk = a;
                chunk_byte_index = b;
                chunk_char_index = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

/// Finds the next grapheme boundary after the given char position.
pub fn next_grapheme_boundary(slice: RopeSlice, char_index: CharIndex) -> CharIndex {
    debug_assert!(char_index.0 <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_index = slice.char_to_byte(char_index.0);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_index, mut chunk_char_index, _) =
        slice.chunk_at_byte(byte_index);

    // Set up the grapheme cursor.
    let mut cursor = GraphemeCursor::new(byte_index, slice.len_bytes(), true);

    // Find the next grapheme cluster boundary.
    loop {
        match cursor.next_boundary(chunk, chunk_byte_index) {
            Ok(None) => return CharIndex(slice.len_chars()),
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_index);
                return CharIndex(chunk_char_index + tmp);
            }
            Err(GraphemeIncomplete::NextChunk) => {
                chunk_byte_index += chunk.len();
                let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_index);
                chunk = a;
                chunk_char_index = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                cursor.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use ropey::Rope;

    use super::*;

    fn text_with_cursor(text: impl Into<Rope>) -> (Rope, Cursor) {
        (text.into(), Cursor::new())
    }

    #[test]
    fn move_right_on_empty_text() {
        let (text, mut cursor) = text_with_cursor("\n");
        cursor.move_right(&text);
        assert_eq!(Cursor::new(), cursor);
    }

    #[test]
    fn move_right_at_the_end() {
        let (text, mut cursor) = text_with_cursor(TEXT);
        cursor.move_to_end_of_buffer(&text);
        let cursor_at_end = cursor.clone();
        cursor.move_right(&text);
        assert_eq!(cursor_at_end, cursor);
    }

    #[test]
    fn move_left_at_the_begining() {
        let text = Rope::from(TEXT);
        let mut cursor = Cursor::new();
        cursor.move_left(&text);
        assert_eq!(Cursor::new(), cursor);
    }

    #[test]
    fn move_wide_grapheme() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let mut cursor = Cursor::new();
        cursor.move_to_start_of_buffer(&text);
        assert_eq!(CharIndex(0)..CharIndex(text.len_chars()), cursor.range);
    }

    #[test]
    fn prev_grapheme_1() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let grapheme_start =
            prev_grapheme_boundary(text.slice(..), CharIndex(text.len_chars() - 1)).0;
        assert_eq!(0, grapheme_start);
    }

    #[test]
    fn end_grapheme_1() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let grapheme_end = next_grapheme_boundary(text.slice(..), CharIndex(0)).0;
        assert_eq!(text.len_chars(), grapheme_end);
    }

    #[test]
    fn sync_with_empty() {
        let current_text = Rope::from("Buy a milk goat\nAt the market\n");
        let new_text = Rope::from("");
        let mut cursor = Cursor::new();
        cursor.move_right_n(&current_text, 4);
        cursor.sync(&current_text, &new_text);
        assert_eq!(Cursor::new(), cursor);
    }

    // Delete forward
    #[test]
    fn delete_forward_at_the_end() {
        let (mut text, mut cursor) = text_with_cursor(TEXT);
        let expected = text.clone();
        cursor.move_to_end_of_buffer(&text);
        cursor.delete_forward(&mut text);
        assert_eq!(expected, text);
    }

    #[test]
    fn delete_forward_empty_text() {
        let (mut text, mut cursor) = text_with_cursor("");
        cursor.delete_forward(&mut text);
        assert_eq!(cursor, Cursor::new());
    }

    #[test]
    fn delete_forward_at_the_begining() {
        let (mut text, mut cursor) = text_with_cursor("// Hello world!\n");
        let expected = Rope::from("Hello world!\n");
        cursor.delete_forward(&mut text);
        cursor.delete_forward(&mut text);
        cursor.delete_forward(&mut text);
        assert_eq!(expected, text);
    }

    // Delete backward
    #[test]
    fn delete_backward_at_the_end() {
        let (mut text, mut cursor) = text_with_cursor("// Hello world!\n");
        let expected = Rope::from("// Hello world\n");
        cursor.move_to_end_of_buffer(&text);
        cursor.delete_backward(&mut text);
        assert_eq!(expected, text);
    }

    #[test]
    fn delete_backward_empty_text() {
        let (mut text, mut cursor) = text_with_cursor("");
        cursor.delete_backward(&mut text);
        assert_eq!(cursor, Cursor::new());
    }

    #[test]
    fn delete_backward_at_the_begining() {
        let (mut text, mut cursor) = text_with_cursor("// Hello world!\n");
        let expected = text.clone();
        cursor.delete_backward(&mut text);
        assert_eq!(expected, text);
    }

    const TEXT: &str = r#"
Basic Latin
    ! " # $ % & ' ( ) *+,-./012ABCDEFGHI` a m  t u v z { | } ~
CJK
    豈 更 車 Ⅷ
"#;
    const MULTI_CHAR_EMOJI: &str = r#"👨‍👨‍👧‍👧"#;
}
