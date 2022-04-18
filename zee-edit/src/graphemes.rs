use ropey::{iter::Chunks, str_utils, Rope, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use unicode_width::UnicodeWidthStr;

use super::TAB_WIDTH;

pub type CharIndex = usize;

pub fn width(slice: &RopeSlice) -> usize {
    rope_slice_as_str(slice, |text| {
        if text == "\t" {
            TAB_WIDTH
        } else {
            text.chars().filter(|character| *character == '\t').count() * TAB_WIDTH
                + UnicodeWidthStr::width(text)
        }
    })
}

pub fn rope_slice_as_str<T>(slice: &RopeSlice, closure: impl FnOnce(&str) -> T) -> T {
    if let Some(text) = slice.as_str() {
        closure(text)
    } else {
        let text = slice.chars().collect::<String>();
        closure(text.as_str())
    }
}

/// An iterator over the graphemes of a RopeSlice.
pub struct RopeGraphemes<'a> {
    text: RopeSlice<'a>,
    chunks: Chunks<'a>,
    chunk: &'a str,
    chunk_byte_start: usize,
    previous_chunk: &'a str,
    previous_chunk_byte_start: usize,
    pub cursor: GraphemeCursor,
}

impl<'a> RopeGraphemes<'a> {
    pub fn new<'b>(slice: &RopeSlice<'b>) -> RopeGraphemes<'b> {
        let mut chunks = slice.chunks();
        let chunk = chunks.next().unwrap_or("");
        RopeGraphemes {
            text: *slice,
            chunks,
            chunk,
            chunk_byte_start: 0,
            previous_chunk: "",
            previous_chunk_byte_start: 0,
            cursor: GraphemeCursor::new(0, slice.len_bytes(), true),
        }
    }
}

impl<'a> Iterator for RopeGraphemes<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        let byte_start = self.cursor.cur_cursor();
        let byte_end;
        loop {
            match self.cursor.next_boundary(self.chunk, self.chunk_byte_start) {
                Ok(None) => {
                    return None;
                }
                Ok(Some(n)) => {
                    byte_end = n;
                    break;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    self.previous_chunk = self.chunk;
                    self.previous_chunk_byte_start = self.chunk_byte_start;
                    self.chunk_byte_start += self.chunk.len();
                    self.chunk = self.chunks.next().unwrap_or("");
                }
                Err(GraphemeIncomplete::PreContext(context_length)) => {
                    assert!(context_length <= self.previous_chunk.len());
                    self.cursor
                        .provide_context(self.previous_chunk, self.previous_chunk_byte_start);
                }
                Err(error) => {
                    panic!(
                        "unexpectedly encountered `{:?}` while iterating over grapheme clusters",
                        error
                    );
                }
            }
        }

        if byte_start < self.chunk_byte_start {
            let char_start = self.text.byte_to_char(byte_start);
            let char_end = self.text.byte_to_char(byte_end);

            Some(self.text.slice(char_start..char_end))
        } else {
            let chunk_byte_start = byte_start - self.chunk_byte_start;
            let chunk_byte_end = byte_end - self.chunk_byte_start;

            Some((&self.chunk[chunk_byte_start..chunk_byte_end]).into())
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

pub trait RopeExt {
    /// Finds the previous grapheme boundary before the given char position
    fn prev_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex;

    /// Finds the next grapheme boundary after the given char position
    fn next_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex;

    /// Finds the nth previous grapheme boundary before the given char position
    fn prev_grapheme_boundary(&self, char_index: CharIndex) -> usize {
        self.prev_grapheme_boundary_n(char_index, 1)
    }

    /// Finds the nth next grapheme boundary after the given char position
    fn next_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        self.next_grapheme_boundary_n(char_index, 1)
    }
}

impl RopeExt for Rope {
    /// Finds the previous grapheme boundary before the given char position
    fn prev_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex {
        prev_grapheme_boundary_n(self.slice(..), char_index, n)
    }

    /// Finds the next grapheme boundary after the given char position
    fn next_grapheme_boundary_n(&self, char_index: CharIndex, n: usize) -> CharIndex {
        next_grapheme_boundary_n(self.slice(..), char_index, n)
    }
}

/// Finds the previous grapheme boundary before the given char position.
fn prev_grapheme_boundary_n(slice: RopeSlice, char_index: CharIndex, n: usize) -> CharIndex {
    // Bounds check
    debug_assert!(char_index <= slice.len_chars());

    // We work with bytes for this, so convert.
    let mut byte_index = slice.char_to_byte(char_index);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_index, mut chunk_char_index, _) =
        slice.chunk_at_byte(byte_index);

    // Set up the grapheme cursor.
    let mut gc = GraphemeCursor::new(byte_index, slice.len_bytes(), true);

    // Find the previous grapheme cluster boundary.
    for _ in 0..n {
        loop {
            match gc.prev_boundary(chunk, chunk_byte_index) {
                Ok(None) => return 0,
                Ok(Some(boundry_offset)) => {
                    byte_index = boundry_offset;
                    break;
                }
                Err(GraphemeIncomplete::PrevChunk) => {
                    let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_index - 1);
                    chunk = a;
                    chunk_byte_index = b;
                    chunk_char_index = c;
                }
                Err(GraphemeIncomplete::PreContext(offset)) => {
                    let ctx_chunk = slice.chunk_at_byte(offset - 1).0;
                    gc.provide_context(ctx_chunk, offset - ctx_chunk.len());
                }
                _ => unreachable!(),
            }
        }
    }

    let tmp = str_utils::byte_to_char_idx(chunk, byte_index - chunk_byte_index);
    chunk_char_index + tmp
}

/// Finds the next grapheme boundary after the given char position.
fn next_grapheme_boundary_n(slice: RopeSlice, char_index: CharIndex, n: usize) -> CharIndex {
    debug_assert!(char_index <= slice.len_chars());

    // We work with bytes for this, so convert.
    let mut byte_index = slice.char_to_byte(char_index);

    // Get the chunk with our byte index in it.
    let (mut chunk, mut chunk_byte_index, mut chunk_char_index, _) =
        slice.chunk_at_byte(byte_index);

    // Set up the grapheme cursor.
    let mut cursor = GraphemeCursor::new(byte_index, slice.len_bytes(), true);

    // Find the next grapheme cluster boundary.
    for _ in 0..n {
        loop {
            match cursor.next_boundary(chunk, chunk_byte_index) {
                Ok(None) => return slice.len_chars(),
                Ok(Some(boundry_offset)) => {
                    byte_index = boundry_offset;
                    break;
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

    let tmp = str_utils::byte_to_char_idx(chunk, byte_index - chunk_byte_index);
    chunk_char_index + tmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn prev_grapheme_1() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let grapheme_start = text.prev_grapheme_boundary(text.len_chars() - 1);
        assert_eq!(0, grapheme_start);
    }

    #[test]
    fn end_grapheme_1() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let grapheme_end = text.next_grapheme_boundary(0);
        assert_eq!(text.len_chars(), grapheme_end);
    }

    const MULTI_CHAR_EMOJI: &str = r#"👨‍👨‍👧‍👧"#;
}
