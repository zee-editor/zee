use ropey::{iter::Chunks, Rope, RopeSlice};
use std::{mem, path::PathBuf};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use unicode_width::UnicodeWidthStr;

use super::smallstring::SmallString;

pub fn clear_path_buf(path: &mut PathBuf) {
    // PathBuf::clear() is a nightly only thing, is there a nicer way to
    // avoid the allocation?
    let mut old_path = PathBuf::new();
    mem::swap(path, &mut old_path);
    let mut old_path_str = old_path.into_os_string();
    old_path_str.clear();
    *path = old_path_str.into();
}

pub fn grapheme_width(slice: &RopeSlice) -> usize {
    if let Some(text) = slice.as_str() {
        if text == "\t" {
            return TAB_WIDTH;
        }
        text.chars().filter(|character| *character == '\t').count() * TAB_WIDTH
            + UnicodeWidthStr::width(text)
    } else {
        let text = SmallString::from_rope_slice(slice);
        if &text[..] == "\t" {
            return TAB_WIDTH;
        }
        text.chars().filter(|character| *character == '\t').count() * TAB_WIDTH
            + UnicodeWidthStr::width(&text[..])
    }
}

/// An iterator over the graphemes of a RopeSlice.
pub struct RopeGraphemes<'a> {
    text: RopeSlice<'a>,
    chunks: Chunks<'a>,
    cur_chunk: &'a str,
    cur_chunk_start: usize,
    pub cursor: GraphemeCursor,
}

impl<'a> RopeGraphemes<'a> {
    pub fn new<'b>(slice: &RopeSlice<'b>) -> RopeGraphemes<'b> {
        let mut chunks = slice.chunks();
        let first_chunk = chunks.next().unwrap_or("");
        RopeGraphemes {
            text: *slice,
            chunks,
            cur_chunk: first_chunk,
            cur_chunk_start: 0,
            cursor: GraphemeCursor::new(0, slice.len_bytes(), true),
        }
    }
}

impl<'a> Iterator for RopeGraphemes<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        let a = self.cursor.cur_cursor();
        let b;
        loop {
            match self
                .cursor
                .next_boundary(self.cur_chunk, self.cur_chunk_start)
            {
                Ok(None) => {
                    return None;
                }
                Ok(Some(n)) => {
                    b = n;
                    break;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    self.cur_chunk_start += self.cur_chunk.len();
                    self.cur_chunk = self.chunks.next().unwrap_or("");
                }
                error => {
                    // No so unreachable, I got `Err(PreContext)` on otherwise
                    // valid and complete utf-8.
                    eprintln!("{:?}", error);
                    unreachable!();
                }
            }
        }

        if a < self.cur_chunk_start {
            let a_char = self.text.byte_to_char(a);
            let b_char = self.text.byte_to_char(b);

            Some(self.text.slice(a_char..b_char))
        } else {
            let a2 = a - self.cur_chunk_start;
            let b2 = b - self.cur_chunk_start;
            Some((&self.cur_chunk[a2..b2]).into())
        }
    }
}

pub fn strip_trailing_whitespace(mut text: Rope) -> Rope {
    // Pretty inefficient (t)

    let mut trailing_empty_line = true;
    for line_index in (0..text.len_lines()).rev() {
        let start = text.line_to_char(line_index);
        let end = if line_index + 1 < text.len_lines() {
            text.line_to_char(line_index + 1)
        } else {
            text.len_chars()
        };
        if start == end {
            continue;
        }

        let mut cursor = end - 1;
        while cursor > start {
            cursor -= 1;
            let character = text.char(cursor);
            if character.is_whitespace() {
                text.remove(cursor..=cursor);
            } else {
                trailing_empty_line = false;
                break;
            }
        }
        if trailing_empty_line && cursor == start {
            text.remove(start..text.len_chars());
        }
    }

    if text.len_chars() > 1 && text.char(text.len_chars() - 1) != '\n' {
        text.insert_char(text.len_chars(), '\n');
    }

    text
}

pub const TAB_WIDTH: usize = 4;

pub fn ensure_trailing_newline_with_content(text: &mut Rope) {
    if text.len_chars() == 0 || text.char(text.len_chars() - 1) != '\n' {
        text.insert_char(text.len_chars(), '\n');
    }
}

#[derive(Copy)]
pub struct StaticRefEq<T: 'static>(&'static T);

impl<T> Clone for StaticRefEq<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T> PartialEq for StaticRefEq<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl<T> std::ops::Deref for StaticRefEq<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T> From<&'static T> for StaticRefEq<T> {
    fn from(other: &'static T) -> Self {
        Self(other)
    }
}
