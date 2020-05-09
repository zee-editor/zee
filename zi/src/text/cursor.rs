use std::{cmp, ops::Range};

use super::{CharIndex, TextStorage, TextStorageMut};

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
            visual_horizontal_offset: None,
            selection: None,
        }
    }

    #[cfg(test)]
    pub fn end_of_buffer<'a>(text: impl TextStorage<'a>) -> Self {
        Self {
            range: text.prev_grapheme_boundary(text.len_chars())..text.len_chars(),
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

    pub fn select_all<'a>(&mut self, text: &impl TextStorage<'a>) {
        self.move_to_start_of_buffer(text);
        self.selection = Some(text.len_chars());
    }

    // pub fn move_up(&mut self, text: impl TextStorage) {
    //     let current_line_index = text.char_to_line(self.range.start);
    //     if current_line_index.0 == 0 {
    //         return;
    //     }
    //     self.move_vertically(text, current_line_index, current_line_index - 1);
    // }

    // pub fn move_up_n(&mut self, text: &impl TextStorage, n: usize) {
    //     for _ in 0..n {
    //         self.move_up(text);
    //     }
    // }

    // pub fn move_down(&mut self, text: impl TextStorage) {
    //     let current_line_index = text.char_to_line(self.range.start);
    //     if current_line_index >= text.len_lines() {
    //         return;
    //     }
    //     self.move_vertically(text, current_line_index.0, current_line_index.0 + 1);
    // }

    // pub fn move_down_n(&mut self, text: impl TextStorage, n: usize) {
    //     for _ in 0..n {
    //         self.move_down(text);
    //     }
    // }

    pub fn move_left<'a>(&mut self, text: &impl TextStorage<'a>) {
        let previous_grapheme_start = text.prev_grapheme_boundary(self.range.start);
        if previous_grapheme_start == CharIndex(0) && self.range.start == CharIndex(0) {
            return;
        }

        self.range = previous_grapheme_start..self.range.start;
        self.visual_horizontal_offset = None;
    }

    pub fn move_right<'a>(&mut self, text: &impl TextStorage<'a>) {
        let grapheme_start = self.range.end;
        let grapheme_end = text.next_grapheme_boundary(grapheme_start);
        if grapheme_start != grapheme_end {
            self.range = grapheme_start..grapheme_end;
        }
        self.visual_horizontal_offset = None;
    }

    pub fn move_right_n<'a>(&mut self, text: &impl TextStorage<'a>, n: usize) {
        for _ in 0..n {
            self.move_right(text);
        }
    }

    pub fn move_to_start_of_line<'a>(&mut self, text: &impl TextStorage<'a>) {
        let line_index = text.char_to_line(self.range.start);
        let char_index = text.line_to_char(line_index);
        self.range = char_index..text.next_grapheme_boundary(char_index);
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_end_of_line<'a>(&mut self, text: &'a impl TextStorage<'a>) {
        self.range = {
            let line_index = text.char_to_line(
                cmp::min(
                    text.len_chars().0.saturating_sub(1).into(),
                    self.range.start.0,
                )
                .into(),
            );
            let line_length = text.line(line_index).len_chars();
            let char_index = text.line_to_char(line_index);
            (char_index + line_length).saturating_sub(1.into())..char_index + line_length
        };
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_start_of_buffer<'a>(&mut self, text: &impl TextStorage<'a>) {
        self.range = CharIndex(0)..text.next_grapheme_boundary(CharIndex(0));
        self.visual_horizontal_offset = None;
    }

    pub fn move_to_end_of_buffer<'a>(&mut self, text: &impl TextStorage<'a>) {
        self.range = text.prev_grapheme_boundary(text.len_chars())..text.len_chars();
        self.visual_horizontal_offset = None;
    }

    pub fn insert_char<'a>(&mut self, text: &mut impl TextStorageMut<'a>, character: char) {
        text.insert_char(self.range.start, character);
        ensure_trailing_newline_with_content(text);
    }

    pub fn insert_chars<'a>(
        &mut self,
        text: &mut impl TextStorageMut<'a>,
        characters: impl Iterator<Item = char>,
    ) {
        characters.enumerate().for_each(|(offset, character)| {
            text.insert_char(self.range.start + offset.into(), character);
        });
        ensure_trailing_newline_with_content(text);
    }

    //     // pub fn insert_slice<StorageT>(&mut self, text: &mut StorageT, slice: StorageT::Slice)
    //     // where
    //     //     StorageT: TextStorage,
    //     // {
    //     //     let mut cursor_start = self.range.start;
    //     //     for chunk in slice.chunks() {
    //     //         text.insert(cursor_start.0, chunk);
    //     //         cursor_start.0 += chunk.chars().count();
    //     //     }
    //     //     // TODO: make sure cursor start is aligned to grapheme boundary
    //     //     self.range = cursor_start..next_grapheme_boundary(&text.slice(..), cursor_start);
    //     // }

    pub fn delete<'a>(&mut self, text: &mut impl TextStorageMut<'a>) {
        if text.len_chars() == 0.into()
            || self.range.start == text.len_chars().saturating_sub(1.into())
        {
            return;
        }
        text.remove(self.range.start.0..self.range.end.0);

        let grapheme_start = self.range.start;
        let grapheme_end = text.next_grapheme_boundary(self.range.start);
        if grapheme_start < grapheme_end {
            self.range = grapheme_start..grapheme_end
        } else {
            self.range = CharIndex(0)..CharIndex(1)
        }
        ensure_trailing_newline_with_content(text);
    }

    //     pub fn delete_line(&mut self, text: &mut impl TextStorage) {
    //         if text.len_chars() == 0 {
    //             return;
    //         }

    //         // Delete line
    //         let line_index = text.char_to_line(self.range.start.0);
    //         let delete_range_start = text.line_to_char(line_index);
    //         let delete_range_end = text.line_to_char(line_index + 1);
    //         text.remove(delete_range_start..delete_range_end);

    //         // Update cursor position
    //         let grapheme_start =
    //             CharIndex(text.line_to_char(cmp::min(line_index, text.len_lines().saturating_sub(2))));
    //         let grapheme_end = next_grapheme_boundary(&text.slice(..), grapheme_start);
    //         if grapheme_start != grapheme_end {
    //             self.range = grapheme_start..grapheme_end
    //         } else {
    //             self.range = CharIndex(0)..CharIndex(1)
    //         }
    //     }

    //     pub fn delete_selection(&mut self, text: &mut impl TextStorage) {
    //         // Delete selection
    //         let selection = self.selection();
    //         text.remove(selection.start.0..selection.end.0);

    //         // Update cursor position
    //         let grapheme_start = cmp::min(
    //             self.range.start,
    //             prev_grapheme_boundary(&text.slice(..), CharIndex(text.len_chars())),
    //         );
    //         let grapheme_end = next_grapheme_boundary(&text.slice(..), grapheme_start);
    //         if grapheme_start != grapheme_end {
    //             self.range = grapheme_start..grapheme_end
    //         } else {
    //             self.range = CharIndex(0)..CharIndex(1)
    //         }
    //         self.clear_selection();
    //         self.visual_horizontal_offset = None;
    //     }

    pub fn backspace<'a>(&mut self, text: &mut impl TextStorageMut<'a>) {
        if self.range.start.0 > 0 {
            self.move_left(text);
            self.delete(text)
        }
    }

    pub fn sync<'a>(
        &mut self,
        current_text: &'a impl TextStorage<'a>,
        new_text: &'a impl TextStorage<'a>,
    ) {
        let current_line = current_text.char_to_line(self.range.start);
        let current_line_offset = self.range.start - current_text.line_to_char(current_line);

        let new_line = cmp::min(current_line, new_text.len_lines().saturating_sub(1.into()));
        let new_line_offset = cmp::min(
            current_line_offset,
            new_text.line(new_line).len_chars().saturating_sub(1.into()),
        );
        let grapheme_end =
            new_text.next_grapheme_boundary(new_text.line_to_char(new_line) + new_line_offset);
        let grapheme_start = new_text.prev_grapheme_boundary(grapheme_end);

        self.range = if grapheme_start != grapheme_end {
            grapheme_start..grapheme_end
        } else {
            CharIndex(0)..CharIndex(1)
        };
        self.visual_horizontal_offset = None;
        self.selection = None;
    }

    // fn move_vertically(
    //     &mut self,
    //     text: impl TextStorage,
    //     current_line_index: LineIndex,
    //     new_line_index: LineIndex,
    // ) {
    //     if new_line_index >= text.len_lines() {
    //         return;
    //     }

    //     let current_line_start = text.line_to_char(current_line_index).0;
    //     let cursor_range_start = self.range.start.0;
    //     // Should be grapheme width, not len
    //     let current_visual_x = self.visual_horizontal_offset.get_or_insert_with(|| {
    //         text.slice(current_line_start..cursor_range_start)
    //             .len_graphemes()
    //     });

    //     let new_line = text.line(new_line_index);
    //     let mut graphemes = new_line.graphemes();
    //     let mut new_visual_x = 0;
    //     while let Some(grapheme) = graphemes.next() {
    //         // Should be grapheme width, not len
    //         let width = grapheme.len_graphemes();
    //         if new_visual_x + width > *current_visual_x {
    //             break;
    //         }
    //         new_visual_x += width;
    //     }

    //     let new_line_offset = CharIndex(
    //         text.byte_to_char(text.line_to_byte(new_line_index) + graphemes.cursor.cur_cursor()),
    //     );

    //     self.range = if new_visual_x <= *current_visual_x {
    //         let grapheme_start = prev_grapheme_boundary(&text.slice(..), new_line_offset);
    //         let grapheme_end = next_grapheme_boundary(&text.slice(..), grapheme_start);
    //         grapheme_start..grapheme_end
    //     } else {
    //         let grapheme_end = next_grapheme_boundary(&text.slice(..), new_line_offset);
    //         let grapheme_start = prev_grapheme_boundary(&text.slice(..), grapheme_end);
    //         grapheme_start..grapheme_end
    //     }
    // }
}

pub(crate) fn ensure_trailing_newline_with_content<'a>(text: &mut impl TextStorageMut<'a>) {
    if text.len_chars() == 0.into() || text.char(text.len_chars() - 1.into()) != '\n' {
        text.insert_char(text.len_chars(), '\n');
    }
}
