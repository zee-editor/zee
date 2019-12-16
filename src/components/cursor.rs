use ropey::Rope;
use std::{cmp, ops::Range};

use crate::syntax::OpaqueDiff;
use crate::utils::{
    self, ensure_trailing_newline_with_content, next_grapheme_boundary, prev_grapheme_boundary,
    RopeGraphemes,
};

pub type CharIndex = usize;

#[derive(Debug)]
pub struct Cursor {
    pub range: Range<CharIndex>, // char range under cursor
    pub select: Option<CharIndex>,
    pub visual_horizontal_offset: Option<usize>,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            range: 0..1,
            visual_horizontal_offset: None,
            select: None,
        }
    }

    pub fn selection(&self) -> Range<usize> {
        match self.select {
            Some(select) if select > self.range.start => self.range.start..select,
            Some(select) if select < self.range.start => select..self.range.start,
            _ => self.range.clone(),
        }
    }

    pub fn move_up(&mut self, text: &Rope) {
        let current_line_index = text.char_to_line(self.range.start);
        if current_line_index == 0 {
            return;
        }
        self.move_vertically(text, current_line_index, current_line_index - 1);
    }

    pub fn move_down(&mut self, text: &Rope) {
        let current_line_index = text.char_to_line(self.range.start);
        if current_line_index >= text.len_lines() {
            return;
        }
        self.move_vertically(text, current_line_index, current_line_index + 1);
    }

    pub fn move_left(&mut self, text: &Rope) {
        let previous_grapheme_start = prev_grapheme_boundary(&text.slice(..), self.range.start);
        if previous_grapheme_start == 0 && self.range.start == 0 {
            return;
        }

        self.range = previous_grapheme_start..self.range.start;
        self.visual_horizontal_offset = None;
    }

    pub fn move_right_n(&mut self, text: &Rope, n: usize) {
        for _ in 0..n {
            self.move_right(text);
        }
    }

    pub fn move_right(&mut self, text: &Rope) {
        let grapheme_start = self.range.end;
        let grapheme_end = next_grapheme_boundary(&text.slice(..), self.range.end);
        if grapheme_start != grapheme_end {
            self.range = grapheme_start..grapheme_end;
        }
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_start_of_line(&mut self, text: &Rope) {
        let line_index = text.char_to_line(self.range.start);
        let char_index = text.line_to_char(line_index);
        self.range = char_index..next_grapheme_boundary(&text.slice(..), char_index);
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_end_of_line(&mut self, text: &Rope) {
        let line_index = text.char_to_line(cmp::min(
            text.len_chars().saturating_sub(1),
            self.range.start,
        ));
        let char_index = text.line_to_char(line_index);
        let line_len = text.line(line_index).len_chars();
        self.range = (char_index + line_len).saturating_sub(1)..char_index + line_len;
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_start_of_buffer(&mut self, text: &Rope) {
        self.range = 0..next_grapheme_boundary(&text.slice(..), 0);
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_end_of_buffer(&mut self, text: &Rope) {
        self.range = prev_grapheme_boundary(&text.slice(..), text.len_chars())..text.len_chars();
        self.visual_horizontal_offset = None;
    }

    pub fn insert_characters(
        &mut self,
        text: &mut Rope,
        characters: impl Iterator<Item = char>,
    ) -> OpaqueDiff {
        let mut num_bytes = 0;
        characters.enumerate().for_each(|(offset, character)| {
            text.insert_char(self.range.start + offset, character);
            num_bytes += character.len_utf8();
        });
        ensure_trailing_newline_with_content(text);
        OpaqueDiff::new(text.char_to_byte(self.range.start), 0, num_bytes)
    }

    pub fn insert_char(&mut self, text: &mut Rope, character: char) -> OpaqueDiff {
        text.insert_char(self.range.start, character);
        ensure_trailing_newline_with_content(text);
        OpaqueDiff::new(self.range.start, 0, 1)
    }

    pub fn delete(&mut self, text: &mut Rope) -> OpaqueDiff {
        if text.len_chars() == 0 {
            return OpaqueDiff::no_diff();
        }

        text.remove(self.range.start..self.range.end);
        let diff = OpaqueDiff::new(self.range.start, self.range.end - self.range.start, 0);

        let grapheme_start = self.range.start;
        let grapheme_end = next_grapheme_boundary(&text.slice(..), self.range.start);
        if grapheme_start < grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = 0..1
        }
        ensure_trailing_newline_with_content(text);
        diff
    }

    pub fn backspace(&mut self, text: &mut Rope) -> OpaqueDiff {
        if self.range.start > 0 {
            self.move_left(text);
            self.delete(text)
        } else {
            OpaqueDiff::no_diff()
        }
    }

    fn move_vertically(&mut self, text: &Rope, current_line_index: usize, new_line_index: usize) {
        if new_line_index >= text.len_lines() {
            return;
        }

        let current_line_start = text.line_to_char(current_line_index);
        let cursor_range_start = self.range.start;
        let current_visual_x = self.visual_horizontal_offset.get_or_insert_with(|| {
            utils::grapheme_width(&text.slice(current_line_start..cursor_range_start))
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

        let new_line_offset =
            text.byte_to_char(text.line_to_byte(new_line_index) + graphemes.cursor.cur_cursor());

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
