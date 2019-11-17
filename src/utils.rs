use ropey::{iter::Chunks, str_utils::byte_to_char_idx, Rope, RopeSlice};
use std::borrow::Cow;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use unicode_width::UnicodeWidthStr;

use super::smallstring::SmallString;

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

pub fn rope_as_str<'a>(slice: &'a RopeSlice) -> Cow<'a, str> {
    if let Some(text) = slice.as_str() {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(SmallString::from_rope_slice(slice).into())
    }
}

/// Finds the previous grapheme boundary before the given char position.
pub fn prev_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Find the previous grapheme cluster boundary.
    loop {
        match gc.prev_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return 0,
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return chunk_char_idx + tmp;
            }
            Err(GraphemeIncomplete::PrevChunk) => {
                let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_idx - 1);
                chunk = a;
                chunk_byte_idx = b;
                chunk_char_idx = c;
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
pub fn next_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> usize {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Find the next grapheme cluster boundary.
    loop {
        match gc.next_boundary(chunk, chunk_byte_idx) {
            Ok(None) => return slice.len_chars(),
            Ok(Some(n)) => {
                let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                return chunk_char_idx + tmp;
            }
            Err(GraphemeIncomplete::NextChunk) => {
                chunk_byte_idx += chunk.len();
                let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_idx);
                chunk = a;
                chunk_char_idx = c;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                gc.provide_context(ctx_chunk, n - ctx_chunk.len());
            }
            _ => unreachable!(),
        }
    }
}

/// Returns whether the given char position is a grapheme boundary.
pub fn is_grapheme_boundary(slice: &RopeSlice, char_idx: usize) -> bool {
    // Bounds check
    debug_assert!(char_idx <= slice.len_chars());

    // We work with bytes for this, so convert.
    let byte_idx = slice.char_to_byte(char_idx);

    // Get the chunk with our byte index in it.
    let (chunk, chunk_byte_idx, _, _) = slice.chunk_at_byte(byte_idx);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

    // Determine if the given position is a grapheme cluster boundary.
    loop {
        match gc.is_boundary(chunk, chunk_byte_idx) {
            Ok(n) => return n,
            Err(GraphemeIncomplete::PreContext(n)) => {
                let (ctx_chunk, ctx_byte_start, _, _) = slice.chunk_at_byte(n - 1);
                gc.provide_context(ctx_chunk, ctx_byte_start);
            }
            _ => unreachable!(),
        }
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
                e => {
                    // No so unreachable, I got `Err(PreContext)` on otherwise
                    // valid and complete utf-8.
                    eprintln!("{:?}", e);
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
