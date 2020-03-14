use ropey::{str_utils::byte_to_char_idx, Rope, RopeSlice};
use std::{cmp, ops::Range};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::{
    syntax::OpaqueDiff,
    utils::{self, ensure_trailing_newline_with_content, RopeGraphemes},
};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct CharIndex(pub usize);

impl From<usize> for CharIndex {
    fn from(index: usize) -> Self {
        Self(index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct ByteIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct LineIndex(pub usize);

#[derive(Clone, Debug)]
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
            visual_horizontal_offset: None,
            selection: None,
        }
    }

    #[cfg(test)]
    pub fn end_of_buffer(text: &Rope) -> Self {
        Self {
            range: prev_grapheme_boundary(&text.slice(..), CharIndex(text.len_chars()))
                ..CharIndex(text.len_chars()),
            visual_horizontal_offset: None,
            selection: None,
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

    pub fn begin_selection(&mut self) {
        self.selection = Some(self.range.start)
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
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
        let previous_grapheme_start = prev_grapheme_boundary(&text.slice(..), self.range.start);
        if previous_grapheme_start == CharIndex(0) && self.range.start == CharIndex(0) {
            return;
        }

        self.range = previous_grapheme_start..self.range.start;
        self.visual_horizontal_offset = None;
    }

    pub fn move_right(&mut self, text: &Rope) {
        let grapheme_start = self.range.end;
        let grapheme_end = next_grapheme_boundary(&text.slice(..), self.range.end);
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
        self.range = char_index..next_grapheme_boundary(&text.slice(..), char_index);
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
            CharIndex((char_index + line_length).saturating_sub(1))
                ..CharIndex(char_index + line_length)
        };
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_start_of_buffer(&mut self, text: &Rope) {
        self.range = CharIndex(0)..next_grapheme_boundary(&text.slice(..), CharIndex(0));
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_end_of_buffer(&mut self, text: &Rope) {
        self.range = prev_grapheme_boundary(&text.slice(..), CharIndex(text.len_chars()))
            ..CharIndex(text.len_chars());
        self.visual_horizontal_offset = None;
    }

    pub fn insert_char(&mut self, text: &mut Rope, character: char) -> OpaqueDiff {
        text.insert_char(self.range.start.0, character);
        ensure_trailing_newline_with_content(text);
        OpaqueDiff::new(text.char_to_byte(self.range.start.0), 0, 1)
    }

    pub fn insert_chars(
        &mut self,
        text: &mut Rope,
        characters: impl Iterator<Item = char>,
    ) -> OpaqueDiff {
        let mut num_bytes = 0;
        characters.enumerate().for_each(|(offset, character)| {
            text.insert_char(self.range.start.0 + offset, character);
            num_bytes += character.len_utf8();
        });
        ensure_trailing_newline_with_content(text);
        OpaqueDiff::new(text.char_to_byte(self.range.start.0), 0, num_bytes)
    }

    pub fn insert_slice(&mut self, text: &mut Rope, slice: RopeSlice) -> OpaqueDiff {
        let mut cursor_start = self.range.start;
        let diff = OpaqueDiff::new(text.char_to_byte(cursor_start.0), 0, slice.len_bytes());
        for chunk in slice.chunks() {
            text.insert(cursor_start.0, chunk);
            cursor_start.0 += chunk.chars().count();
        }
        // TODO: make sure cursor start is aligned to grapheme boundary
        self.range = cursor_start..next_grapheme_boundary(&text.slice(..), cursor_start);
        diff
    }

    pub fn delete(&mut self, text: &mut Rope) -> DeleteOperation {
        if text.len_chars() == 0 || self.range.start.0 == text.len_chars().saturating_sub(1) {
            return DeleteOperation::empty();
        }
        let diff = OpaqueDiff::new(
            text.char_to_byte(self.range.start.0),
            text.char_to_byte(self.range.end.0) - text.char_to_byte(self.range.start.0),
            0,
        );
        text.remove(self.range.start.0..self.range.end.0);

        let grapheme_start = self.range.start;
        let grapheme_end = next_grapheme_boundary(&text.slice(..), self.range.start);
        let deleted = text.slice(grapheme_start.0..grapheme_end.0).into();
        if grapheme_start < grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = CharIndex(0)..CharIndex(1)
        }
        ensure_trailing_newline_with_content(text);
        DeleteOperation { diff, deleted }
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
        );
        text.remove(delete_range_start..delete_range_end);

        // Update cursor position
        let grapheme_start =
            CharIndex(text.line_to_char(cmp::min(line_index, text.len_lines().saturating_sub(2))));
        let grapheme_end = next_grapheme_boundary(&text.slice(..), grapheme_start);
        if grapheme_start != grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = CharIndex(0)..CharIndex(1)
        }

        DeleteOperation { diff, deleted }
    }

    pub fn delete_selection(&mut self, text: &mut Rope) -> DeleteOperation {
        // Delete selection
        let selection = self.selection();
        let deleted = text.slice(selection.start.0..selection.end.0).into();
        let diff = OpaqueDiff::new(selection.start.0, selection.end.0 - selection.start.0, 0);
        text.remove(selection.start.0..selection.end.0);

        // Update cursor position
        let grapheme_start = cmp::min(
            self.range.start,
            prev_grapheme_boundary(&text.slice(..), CharIndex(text.len_chars())),
        );
        let grapheme_end = next_grapheme_boundary(&text.slice(..), grapheme_start);
        if grapheme_start != grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = CharIndex(0)..CharIndex(1)
        }
        self.clear_selection();
        self.visual_horizontal_offset = None;

        DeleteOperation { diff, deleted }
    }

    pub fn backspace(&mut self, text: &mut Rope) -> DeleteOperation {
        if self.range.start.0 > 0 {
            self.move_left(text);
            self.delete(text)
        } else {
            DeleteOperation::empty()
        }
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
            &new_text.slice(..),
            CharIndex(new_text.line_to_char(new_line) + new_line_offset),
        );
        let grapheme_start = prev_grapheme_boundary(&new_text.slice(..), grapheme_end);

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
            utils::grapheme_width(&text.slice(current_line_start..cursor_range_start.0))
        });

        let new_line = text.line(new_line_index);
        let mut graphemes = RopeGraphemes::new(&new_line);
        let mut new_visual_x = 0;
        while let Some(grapheme) = graphemes.next() {
            let width = utils::grapheme_width(&grapheme);
            if new_visual_x + width > *current_visual_x {
                break;
            }
            new_visual_x += width;
        }

        let new_line_offset = CharIndex(
            text.byte_to_char(text.line_to_byte(new_line_index) + graphemes.cursor.cur_cursor()),
        );

        self.range = if new_visual_x <= *current_visual_x {
            let grapheme_start = prev_grapheme_boundary(&text.slice(..), new_line_offset);
            let grapheme_end = next_grapheme_boundary(&text.slice(..), grapheme_start);
            grapheme_start..grapheme_end
        } else {
            let grapheme_end = next_grapheme_boundary(&text.slice(..), new_line_offset);
            let grapheme_start = prev_grapheme_boundary(&text.slice(..), grapheme_end);
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

    pub fn is_empty(&self) -> bool {
        self.deleted.len_bytes() == 0 && self.diff.is_empty()
    }
}

/// Finds the previous grapheme boundary before the given char position.
pub fn prev_grapheme_boundary(slice: &RopeSlice, char_index: CharIndex) -> CharIndex {
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
pub fn next_grapheme_boundary(slice: &RopeSlice, char_index: CharIndex) -> CharIndex {
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
