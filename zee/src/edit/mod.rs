pub mod cursor;
pub mod graphemes;
pub mod tree;

pub use cursor::Cursor;
pub use graphemes::{rope_slice_as_str, strip_trailing_whitespace, RopeGraphemes};
pub use tree::EditTree;

use ropey::Rope;

pub const TAB_WIDTH: usize = 4;

pub fn ensure_trailing_newline_with_content(text: &mut Rope) {
    if text.len_chars() == 0 || text.char(text.len_chars() - 1) != '\n' {
        text.insert_char(text.len_chars(), '\n');
    }
}
