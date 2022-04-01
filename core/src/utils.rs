use ropey::{iter::Chunks, Rope, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use unicode_width::UnicodeWidthStr;

pub fn grapheme_width(slice: &RopeSlice) -> usize {
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
