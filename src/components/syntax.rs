use super::{cursor::Cursor, theme::gruvbox};
use crate::terminal::{Background, Foreground, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub text: Style,
    pub text_current_line: Style,
    pub cursor_focused: Style,
    pub cursor_unfocused: Style,
    pub selection_background: Background,
    pub code_invalid: Style,
    pub code_constant: Style,
    pub code_keyword: Style,
    pub code_keyword_light: Style,
    pub code_string: Style,
    pub code_char: Style,
    pub code_operator: Style,
    pub code_macro_call: Style,
    pub code_function_call: Style,
    pub code_comment: Style,
    pub code_comment_doc: Style,
    pub code_link: Style,
    pub code_type: Style,
}

#[inline]
pub fn text_style_at_char(
    theme: &Theme,
    cursor: &Cursor,
    char_index: usize,
    focused: bool,
    line_under_cursor: bool,
    scope: &str,
    is_error: bool,
) -> Style {
    if cursor.range.contains(&char_index) {
        if focused {
            theme.cursor_focused
        } else {
            theme.cursor_unfocused
        }
    } else {
        let background = if cursor.selection().contains(&char_index) {
            theme.selection_background
        } else if line_under_cursor && focused {
            theme.text_current_line.background
        } else {
            theme.text.background
        };

        let style = if is_error {
            theme.code_invalid
        } else if scope.starts_with("constant") {
            theme.code_constant
        } else if scope.starts_with("string.quoted.double.dictionary.key.json")
            || scope.starts_with("support.property-name")
        {
            theme.code_keyword_light
        } else if scope.starts_with("string.quoted.double") {
            theme.code_string
        } else if scope.starts_with("string.quoted.single") {
            theme.code_char
        } else if scope.starts_with("string") {
            theme.code_string
        } else if scope.starts_with("keyword.operator") {
            theme.code_operator
        } else if scope.starts_with("storage")
            || scope.starts_with("keyword")
            || scope.starts_with("tag_name")
            || scope.ends_with("variable.self")
        {
            theme.code_keyword
        } else if scope.starts_with("variable.parameter.function")
            || scope.starts_with("identifier")
            || scope.starts_with("field_identifier")
        {
            theme.code_keyword_light
        } else if scope.starts_with("entity.name.enum")
            || scope.starts_with("support")
            || scope.starts_with("primitive_type")
        {
            theme.code_type
        } else if scope.starts_with("entity.attribute.name.punctuation") {
            theme.code_comment
        } else if scope.starts_with("entity.attribute.name")
            || scope.starts_with("entity.name.lifetime")
        {
            theme.code_macro_call
        } else if scope.starts_with("entity.name.macro.call") {
            theme.code_macro_call
        } else if scope.starts_with("entity.name.function") {
            theme.code_function_call
        } else if scope.starts_with("comment.block.line.docstr") {
            theme.code_comment_doc
        } else if scope.starts_with("comment") {
            theme.code_comment
        } else if ["<", ">", "/>", "</", "misc.other"]
            .iter()
            .any(|tag| scope.starts_with(tag))
        {
            theme.code_operator
        } else if scope.starts_with("markup.underline.link") {
            theme.code_link
        } else {
            theme.text
        };

        Style {
            background,
            foreground: style.foreground,
            bold: style.bold,
            underline: style.underline,
        }
    }
}
