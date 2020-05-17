use ropey::{iter::Chunks, str_utils::byte_to_char_idx, Rope, RopeSlice};
use std::ops::RangeBounds;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use super::{ByteIndex, CharIndex, LineIndex, TextStorage, TextStorageMut};

impl<'a> TextStorage<'a> for Rope {
    type Slice = RopeSlice<'a>;
    type GraphemeIterator = RopeGraphemes<'a>;

    fn len_bytes(&self) -> ByteIndex {
        self.len_bytes().into()
    }

    fn len_chars(&self) -> CharIndex {
        self.len_chars().into()
    }

    fn len_lines(&self) -> LineIndex {
        self.len_lines().into()
    }

    fn len_graphemes(&self) -> usize {
        // TODO: Should this be number of graphemes rather than width? If not,
        // change the name of the function; check usage.
        grapheme_width(&self.slice(..))
    }

    fn char_to_line(&self, char_index: CharIndex) -> LineIndex {
        self.char_to_line(char_index.into()).into()
    }

    fn line_to_char(&self, line_index: LineIndex) -> CharIndex {
        self.line_to_char(line_index.into()).into()
    }

    fn char(&self, char_index: CharIndex) -> char {
        Rope::char(self, char_index.0)
    }

    fn graphemes(&'a self) -> Self::GraphemeIterator {
        RopeGraphemes::new(&self.slice(..))
    }

    fn line(&'a self, line_index: LineIndex) -> Self::Slice {
        self.line(line_index.into())
    }

    fn slice(&'a self, range: impl RangeBounds<usize>) -> Self::Slice {
        Rope::slice(self, range)
    }

    fn prev_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        prev_grapheme_boundary(&self.slice(..), char_index)
    }

    fn next_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        next_grapheme_boundary(&self.slice(..), char_index)
    }
}

impl<'a> TextStorageMut<'a> for Rope {
    fn insert_char(&mut self, char_index: CharIndex, character: char) {
        Rope::insert_char(self, char_index.0, character);
    }

    fn remove(&mut self, range: impl RangeBounds<usize>) {
        Rope::remove(self, range);
    }
}

impl<'a> TextStorage<'a> for RopeSlice<'a> {
    type Slice = RopeSlice<'a>;
    type GraphemeIterator = RopeGraphemes<'a>;

    fn len_bytes(&self) -> ByteIndex {
        self.len_bytes().into()
    }

    fn len_chars(&self) -> CharIndex {
        self.len_chars().into()
    }

    fn len_lines(&self) -> LineIndex {
        self.len_lines().into()
    }

    fn len_graphemes(&self) -> usize {
        grapheme_width(&self)
    }

    fn char_to_line(&self, char_index: CharIndex) -> LineIndex {
        self.char_to_line(char_index.into()).into()
    }

    fn line_to_char(&self, line_index: LineIndex) -> CharIndex {
        self.line_to_char(line_index.into()).into()
    }

    fn char(&self, char_index: CharIndex) -> char {
        RopeSlice::char(self, char_index.0)
    }

    fn graphemes(&self) -> Self::GraphemeIterator {
        RopeGraphemes::new(&self.slice(..))
    }

    fn line(&self, line_index: LineIndex) -> Self::Slice {
        self.line(line_index.into())
    }

    fn slice(&self, range: impl RangeBounds<usize>) -> Self::Slice {
        RopeSlice::slice(self, range)
    }

    fn prev_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        prev_grapheme_boundary(&self, char_index)
    }

    fn next_grapheme_boundary(&self, char_index: CharIndex) -> CharIndex {
        next_grapheme_boundary(&self, char_index)
    }
}

/// Finds the previous grapheme boundary before the given char position.
fn prev_grapheme_boundary(slice: &RopeSlice, char_index: CharIndex) -> CharIndex {
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
fn next_grapheme_boundary(slice: &RopeSlice, char_index: CharIndex) -> CharIndex {
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

/// An iterator over the graphemes of a RopeSlice.
pub struct RopeGraphemes<'a> {
    text: RopeSlice<'a>,
    chunks: Chunks<'a>,
    cur_chunk: &'a str,
    cur_chunk_start: usize,
    cursor: GraphemeCursor,
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

fn grapheme_width(slice: &RopeSlice) -> usize {
    RopeGraphemes::new(slice).count()
}
