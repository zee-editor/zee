use ropey::Rope;
use std::ops::Range;

use crate::utils::{self, next_grapheme_boundary, prev_grapheme_boundary, RopeGraphemes};

pub type CharIndex = usize;

#[derive(Debug)]
pub struct Cursor {
    pub range: Range<CharIndex>, // char range under cursor
    pub select: Option<CharIndex>,
    pub visual_horizontal_offset: Option<usize>,
}

impl Cursor {
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
        let line_index = text.char_to_line(self.range.start);
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
