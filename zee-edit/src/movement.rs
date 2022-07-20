use ropey::Rope;

use crate::{
    graphemes::{RopeExt, RopeGraphemes},
    Cursor,
};

/// The movement direction
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// Move the cursor horizontally in the specified direction by `count` positions
/// (grapheme clusters)
#[inline]
pub fn move_horizontally(text: &Rope, cursor: &mut Cursor, direction: Direction, count: usize) {
    let grapheme_start = match direction {
        Direction::Forward => text.next_grapheme_boundary_n(cursor.range.start, count),
        Direction::Backward => text.prev_grapheme_boundary_n(cursor.range.start, count),
    };
    cursor.range = grapheme_start..text.next_grapheme_boundary(grapheme_start);
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor vertically in the specified direction by `count` lines
#[inline]
pub fn move_vertically(
    text: &Rope,
    cursor: &mut Cursor,
    tab_width: usize,
    direction: Direction,
    count: usize,
) {
    // The maximum possible line index in the text
    let max_line_index = text.len_lines().saturating_sub(1);

    // The current line index under cursor
    let current_line_index = text.char_to_line(cursor.range.start);

    // Compute the line index where the cursor will be after moving
    let new_line_index = match direction {
        // If cursor is not on the last line and moving forward (down), compute
        // which line we'll end up on
        Direction::Forward if current_line_index < max_line_index => {
            std::cmp::min(current_line_index + count, max_line_index)
        }
        // If the cursor is on the last line and moving forward (down), move the
        // cursor to the end of the line instead.
        Direction::Forward if current_line_index == max_line_index => {
            move_to_end_of_line(text, cursor);
            return;
        }
        // If cursor is not on the first line and moving backward (up), compute
        // which line we'll end up on
        Direction::Backward if current_line_index > 0 => current_line_index.saturating_sub(count),
        // Otherwise, nothing to do
        _ => {
            return;
        }
    };

    let current_visual_x = cursor.visual_horizontal_offset.get_or_insert_with(|| {
        let current_line_start = text.line_to_char(current_line_index);
        let line_to_cursor = text.slice(current_line_start..cursor.range.start);
        crate::graphemes::width(tab_width, &line_to_cursor)
    });

    let new_line = text.line(new_line_index);
    let mut graphemes = RopeGraphemes::new(&new_line);
    let mut new_visual_x = 0;
    let mut char_offset = text.line_to_char(new_line_index);
    for grapheme in &mut graphemes {
        let width = crate::graphemes::width(tab_width, &grapheme);
        if new_visual_x + width > *current_visual_x || grapheme.slice == "\n" {
            break;
        }
        char_offset += grapheme.slice.len_chars();
        new_visual_x += width;
    }

    cursor.range = char_offset..text.next_grapheme_boundary(char_offset);
}

/// Move the cursor in the specified direction by `count` words
#[inline]
pub fn move_word(text: &Rope, cursor: &mut Cursor, direction: Direction, count: usize) {
    match direction {
        Direction::Forward => {
            for _ in 0..count {
                move_forward_word(text, cursor);
            }
        }
        Direction::Backward => {
            for _ in 0..count {
                move_backward_word(text, cursor);
            }
        }
    }
}

/// Move the cursor forward by one word
#[inline]
pub fn move_forward_word(text: &Rope, cursor: &mut Cursor) {
    let first_word_character =
        skip_while_forward(text, cursor.range.start, |c| !is_word_character(c))
            .unwrap_or_else(|| text.len_chars());
    let grapheme_start = skip_while_forward(text, first_word_character, is_word_character)
        .unwrap_or_else(|| text.len_chars());
    cursor.range = grapheme_start..text.next_grapheme_boundary(grapheme_start);
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor backward by one word
#[inline]
pub fn move_backward_word(text: &Rope, cursor: &mut Cursor) {
    let first_word_character =
        skip_while_backward(text, cursor.range.start, |c| !is_word_character(c)).unwrap_or(0);
    let grapheme_start =
        skip_while_backward(text, first_word_character, is_word_character).unwrap_or(0);
    cursor.range = grapheme_start..text.next_grapheme_boundary(grapheme_start);
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor in the specified direction by `count` paragraphs
#[inline]
pub fn move_paragraph(text: &Rope, cursor: &mut Cursor, direction: Direction, count: usize) {
    match direction {
        Direction::Forward => {
            for _ in 0..count {
                move_forward_paragraph(text, cursor);
            }
        }
        Direction::Backward => {
            for _ in 0..count {
                move_backward_paragraph(text, cursor);
            }
        }
    }
}

/// Move the cursor forward by one paragraph
#[inline]
pub fn move_forward_paragraph(text: &Rope, cursor: &mut Cursor) {
    let current_line = text.char_to_line(cursor.range.start);
    let lines = text.lines_at(current_line + 1);

    let start = lines
        .enumerate()
        .find_map(|(index, line)| {
            line.chars()
                .all(char::is_whitespace)
                .then(|| text.line_to_char(current_line + index + 1))
        })
        .unwrap_or_else(|| text.len_chars());
    cursor.range = start..text.next_grapheme_boundary(start);
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor backward by one paragraph
#[inline]
pub fn move_backward_paragraph(text: &Rope, cursor: &mut Cursor) {
    let current_line = text.char_to_line(cursor.range.start);
    let mut lines = text.lines_at(current_line.saturating_sub(1));
    lines.reverse();

    let start = lines
        .enumerate()
        .find_map(|(index, line)| {
            line.chars()
                .all(char::is_whitespace)
                .then(|| text.line_to_char(current_line.saturating_sub(index + 1)))
        })
        .unwrap_or(0);
    cursor.range = start..text.next_grapheme_boundary(start);
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor to the beginning of the current line
#[inline]
pub fn move_to_start_of_line(text: &Rope, cursor: &mut Cursor) {
    let line_start = text.line_to_char(text.char_to_line(cursor.range.start));
    cursor.range = line_start..text.next_grapheme_boundary(line_start);
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor to the end of the current line
#[inline]
pub fn move_to_end_of_line(text: &Rope, cursor: &mut Cursor) {
    let line_index = text.char_to_line(cursor.range.start);
    let line = text.line(line_index);
    let line_start = text.line_to_char(line_index);
    let line_length = line.len_chars();
    let range_end = line_start + line_length;

    let range_start = if line_length == 0 || line.char(line_length - 1) != '\n' {
        // If the current line has no newline at the end (we're at the end of the
        // buffer), create an empty range, i.e. range_start == range_end
        range_end
    } else {
        // Otherwise, select the last character in the line, guaranteed a newline
        range_end.saturating_sub(1)
    };

    cursor.range = range_start..range_end;
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor to the beginning of the text
#[inline]
pub fn move_to_start_of_buffer(text: &Rope, cursor: &mut Cursor) {
    cursor.range = 0..text.next_grapheme_boundary(0);
    cursor.visual_horizontal_offset = None;
}

/// Move the cursor to the end of the text
#[inline]
pub fn move_to_end_of_buffer(text: &Rope, cursor: &mut Cursor) {
    let length = text.len_chars();
    cursor.range = length..length;
    cursor.visual_horizontal_offset = None;
}

#[inline]
fn skip_while_forward(
    text: &Rope,
    position: usize,
    predicate: impl Fn(char) -> bool,
) -> Option<usize> {
    text.chars_at(position)
        .enumerate()
        .find_map(|(index, character)| (!predicate(character)).then(|| position + index))
}

#[inline]
fn skip_while_backward(
    text: &Rope,
    position: usize,
    predicate: impl Fn(char) -> bool,
) -> Option<usize> {
    let mut chars = text.chars_at(position);
    chars.reverse();
    chars.enumerate().find_map(|(index, character)| {
        (!predicate(character)).then(|| position.saturating_sub(index))
    })
}

#[inline]
fn is_word_character(character: char) -> bool {
    character == '_' || (!character.is_whitespace() && !character.is_ascii_punctuation())
}

#[cfg(test)]
mod tests {
    use super::{super::RopeCursorExt, *};
    use ropey::Rope;

    // Some test helpers on Cursor
    impl Cursor {
        fn move_right(&mut self, text: &Rope) {
            move_horizontally(text, self, Direction::Forward, 1)
        }

        fn move_left(&mut self, text: &Rope) {
            move_horizontally(text, self, Direction::Backward, 1)
        }
    }

    fn text_with_cursor(text: impl Into<Rope>) -> (Rope, Cursor) {
        (text.into(), Cursor::new())
    }

    #[test]
    fn move_right_on_empty_text() {
        let (text, mut cursor) = text_with_cursor("");
        cursor.move_right(&text);
        assert_eq!(cursor, Cursor::new());

        let (text, mut cursor) = text_with_cursor("\n");
        cursor.move_right(&text);
        assert_eq!(cursor, Cursor::with_range(1..1));
    }

    #[test]
    fn move_right_at_the_end() {
        let (text, mut cursor) = text_with_cursor(TEXT);
        move_to_end_of_buffer(&text, &mut cursor);
        let cursor_at_end = cursor.clone();
        cursor.move_right(&text);
        assert_eq!(cursor_at_end, cursor);
        assert_eq!(
            cursor,
            Cursor::with_range(text.len_chars()..text.len_chars())
        );
    }

    #[test]
    fn move_left_at_the_begining() {
        let text = Rope::from(TEXT);
        let mut cursor = Cursor::new();
        cursor.move_left(&text);
        assert_eq!(Cursor::with_range(0..1), cursor);
    }

    #[test]
    fn move_wide_grapheme() {
        let text = Rope::from(MULTI_CHAR_EMOJI);
        let mut cursor = Cursor::new();
        move_to_start_of_buffer(&text, &mut cursor);
        assert_eq!(0..text.len_chars(), cursor.range);
    }

    #[test]
    fn move_by_zero_positions() {
        let (text, mut cursor) = text_with_cursor("Hello\n");
        move_horizontally(&text, &mut cursor, Direction::Backward, 0);
        assert_eq!(Cursor::with_range(0..1), cursor);
        move_horizontally(&text, &mut cursor, Direction::Forward, 0);
        assert_eq!(Cursor::with_range(0..1), cursor);

        cursor.range = 1..2;
        move_horizontally(&text, &mut cursor, Direction::Backward, 0);
        assert_eq!(cursor.range, 1..2);
        move_horizontally(&text, &mut cursor, Direction::Forward, 0);
        assert_eq!(cursor.range, 1..2);
    }

    #[test]
    fn move_backward_on_empty_text() {
        let (text, mut cursor) = text_with_cursor("");
        move_horizontally(&text, &mut cursor, Direction::Backward, 1);
        assert_eq!(Cursor::new(), cursor);
    }

    #[test]
    fn move_backward_at_the_begining() {
        let (text, mut cursor) = text_with_cursor("The flowers were blooming.\n");
        move_horizontally(&text, &mut cursor, Direction::Backward, 1);
        assert_eq!(cursor, Cursor::with_range(0..1),);
        assert_eq!(text.slice_cursor(&cursor), "T");
    }

    const TEXT: &str = r#"
Basic Latin
    ! " # $ % & ' ( ) *+,-./012ABCDEFGHI` a m  t u v z { | } ~
CJK
    豈 更 車 Ⅷ
"#;
    const MULTI_CHAR_EMOJI: &str = r#"👨‍👨‍👧‍👧"#;
}
