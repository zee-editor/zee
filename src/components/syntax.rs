use super::{cursor::Cursor, theme::gruvbox};
use crate::terminal::{Background, Foreground, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub text: Style,
    pub text_current_line: Style,
    pub cursor_focused: Style,
    pub cursor_unfocused: Style,
    pub selection_background: Background,
}

#[inline]
pub fn text_style_at_char(
    theme: &Theme,
    cursor: &Cursor,
    char_index: usize,
    focused: bool,
    line_under_cursor: bool,
    scope: &str,
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

        let (foreground, bold, underline) = if scope.starts_with("constant") {
            (Foreground(gruvbox::BRIGHT_AQUA), false, false)
        } else if scope.starts_with("string.quoted.double.dictionary.key.json") {
            (Foreground(gruvbox::BRIGHT_BLUE), false, false)
        } else if scope.starts_with("string.quoted.double") {
            (Foreground(gruvbox::BRIGHT_ORANGE), false, false)
        } else if scope.starts_with("string.quoted.single") {
            (Foreground(gruvbox::NEUTRAL_ORANGE), false, false)
        } else if scope.starts_with("string") {
            (Foreground(gruvbox::BRIGHT_ORANGE), false, false)
        } else if scope.starts_with("keyword.operator") {
            (Foreground(gruvbox::LIGHT3), false, false)
        } else if scope.starts_with("storage")
            || scope.starts_with("keyword")
            || scope.starts_with("tag_name")
            || scope.ends_with("variable.self")
        {
            (Foreground(gruvbox::BRIGHT_BLUE), true, false)
        } else if scope.starts_with("variable.parameter.function")
            || scope.starts_with("identifier")
            || scope.starts_with("field_identifier")
        {
            (Foreground(gruvbox::BRIGHT_BLUE), false, false)
        } else if scope.starts_with("entity.name.enum")
            || scope.starts_with("support")
            || scope.starts_with("primitive_type")
        {
            (Foreground(gruvbox::BRIGHT_PURPLE), false, false)
        } else if scope.starts_with("entity.name.variable.field") {
            (theme.text.foreground, false, false)
        } else if scope.starts_with("entity.attribute.name") {
            (Foreground(gruvbox::NEUTRAL_GREEN), false, false)
        } else if scope.starts_with("entity.name.macro.call") {
            (Foreground(gruvbox::NEUTRAL_GREEN), true, false)
        } else if scope.starts_with("entity.name.function.call") {
            (Foreground(gruvbox::BRIGHT_GREEN), false, false)
        } else if scope.starts_with("entity.name") {
            (theme.text.foreground, false, false)
        } else if scope.starts_with("comment") {
            (Foreground(gruvbox::DARK4), false, false)
        } else if ["<", ">", "/>", "</", "misc.other"]
            .iter()
            .any(|tag| scope.starts_with(tag))
        {
            (Foreground(gruvbox::GRAY_245), false, false)
        } else if scope.starts_with("markup.underline.link") {
            (Foreground(gruvbox::LIGHT3), false, true)
        } else {
            (theme.text.foreground, false, false)
        };

        Style {
            background,
            foreground,
            bold,
            underline,
        }
    }
}
