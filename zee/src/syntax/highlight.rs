use zi::terminal::{Background, Style};

use zee_edit::{CharIndex, Cursor};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub cursor_focused: Style,
    pub cursor_unfocused: Style,
    pub selection_background: Background,
    pub text: Style,
    pub text_current_line: Style,
    pub code_char: Style,
    pub code_comment: Style,
    pub code_comment_doc: Style,
    pub code_constant: Style,
    pub code_function_call: Style,
    pub code_invalid: Style,
    pub code_keyword: Style,
    pub code_keyword_light: Style,
    pub code_link: Style,
    pub code_macro_call: Style,
    pub code_operator: Style,
    pub code_string: Style,
    pub code_type: Style,
    pub code_variant: Style,
}

#[inline]
pub fn text_style_at_char(
    theme: &Theme,
    cursor: &Cursor,
    char_index: CharIndex,
    focused: bool,
    line_under_cursor: bool,
    scope: &str,
    is_error: bool,
) -> Style {
    let starts = |pattern| scope.starts_with(pattern);

    let style = match () {
        _ if is_error => theme.code_invalid,
        _ if scope.is_empty() => theme.text,
        _ if starts("error") => theme.code_invalid,
        _ if starts("attribute") => theme.code_macro_call,
        _ if starts("comment.block") => theme.code_comment_doc,
        _ if starts("comment") => theme.code_comment,
        _ if starts("constructor") => theme.code_variant,
        // Constants
        _ if starts("constant.character") => theme.code_char,
        _ if starts("constant.numeric") => theme.code_function_call,
        _ if starts("constant") => theme.code_constant,
        _ if starts("string") => theme.code_string,
        // Functions
        _ if starts("function.macro") => theme.code_macro_call,
        _ if starts("function") => theme.code_function_call,
        _ if starts("keyword.control.import") => theme.code_keyword,
        _ if starts("keyword") => theme.code_keyword,
        _ if starts("operator") => theme.code_operator,
        _ if starts("property") => theme.code_function_call,
        _ if starts("punctuation.bracket") => theme.code_operator,
        _ if starts("punctuation.delimiter") => theme.code_operator,
        _ if starts("punctuation.special") => theme.code_operator,
        _ if starts("punctuation") => theme.text,
        _ if starts("special") => theme.code_operator,
        _ if starts("table.name") => theme.code_keyword_light,
        // Types
        _ if starts("type.variant") => theme.code_variant,
        _ if starts("type") => theme.code_type,
        // Text
        _ if starts("tag") => theme.code_function_call,
        _ if starts("text.title") => theme.code_keyword,
        _ if starts("text.emphasis") => theme.code_keyword_light,
        _ if starts("text.strong") => theme.code_keyword,
        _ if starts("text.literal") => theme.code_string,
        _ if starts("text.uri") => theme.code_operator,

        _ => theme.text,
    };

    if char_index == cursor.range().start || cursor.range().contains(&char_index) {
        let cursor_style = if focused {
            theme.cursor_focused
        } else {
            theme.cursor_unfocused
        };
        Style {
            background: cursor_style.background,
            foreground: cursor_style.foreground,
            bold: style.bold,
            underline: style.underline,
        }
    } else {
        let background = if cursor.selection().contains(&char_index) {
            theme.selection_background
        } else if line_under_cursor && focused {
            theme.text_current_line.background
        } else {
            theme.text.background
        };
        Style {
            background,
            foreground: style.foreground,
            bold: style.bold,
            underline: style.underline,
        }
    }
}
