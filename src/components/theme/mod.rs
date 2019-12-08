pub mod base16;

use super::{
    buffer::Theme as BufferTheme, prompt::Theme as PromptTheme, splash::Theme as SplashTheme,
    syntax::Theme as SyntaxTheme,
};
use crate::terminal::{Background, Colour, Foreground, Style};
pub use base16::Base16Theme;

#[derive(Clone, Debug)]
pub struct Theme {
    pub buffer: BufferTheme,
    pub splash: SplashTheme,
    pub prompt: PromptTheme,
}

impl Theme {
    pub fn gruvbox() -> Self {
        // For reference, in base16-gruvbox-dark-soft the colours are mapped as
        // folows:
        //
        // base00: DARK0_SOFT
        // base01: DARK1
        // base02: DARK2
        // base03: DARK3
        // base04: LIGHT3
        // base05: LIGHT2
        // base06: LIGHT1
        // base07: LIGHT0
        // base08: BRIGHT_RED
        // base09: BRIGHT_ORANGE
        // base0a: BRIGHT_YELLOW
        // base0b: BRIGHT_GREEN
        // base0c: BRIGHT_AQUA
        // base0d: BRIGHT_BLUE
        // base0e: BRIGHT_PURPLE
        // base0f: NEUTRAL_ORANGE

        use gruvbox::*;
        Self {
            buffer: BufferTheme {
                syntax: SyntaxTheme {
                    text: normal(DARK0, LIGHT1),
                    text_current_line: normal(DARK0_HARD, LIGHT1),
                    cursor_focused: normal(LIGHT0, DARK0),
                    cursor_unfocused: normal(GRAY_245, DARK0_HARD),
                    selection_background: Background(DARK0_HARD),
                    code_invalid: normal(DARK0_SOFT, BRIGHT_RED),
                    code_constant: normal(DARK0_SOFT, BRIGHT_GREEN),
                    code_keyword: bold(DARK0_SOFT, BRIGHT_PURPLE),
                    code_keyword_light: normal(DARK0_SOFT, BRIGHT_PURPLE),
                    code_string: normal(DARK0_SOFT, BRIGHT_GREEN),
                    code_char: normal(DARK0_SOFT, NEUTRAL_GREEN),
                    code_operator: normal(DARK0_SOFT, LIGHT3),
                    code_macro_call: normal(DARK0_SOFT, NEUTRAL_ORANGE),
                    code_function_call: normal(DARK0_SOFT, BRIGHT_BLUE),
                    code_comment: normal(DARK0_SOFT, DARK4),
                    code_comment_doc: normal(DARK0_SOFT, LIGHT4),
                    code_link: underline(DARK0_SOFT, LIGHT3),
                    code_type: normal(DARK0_SOFT, BRIGHT_YELLOW),
                },
                border: normal(DARK0_HARD, GRAY_245),
                status_base: normal(DARK0_SOFT, DARK0),
                status_frame_id_focused: normal(BRIGHT_BLUE, DARK0_HARD),
                status_frame_id_unfocused: normal(GRAY_245, DARK0_HARD),
                status_is_modified: normal(DARK0, BRIGHT_RED),
                status_is_not_modified: normal(DARK0, GRAY_245),
                status_file_name: normal(DARK0_SOFT, BRIGHT_BLUE),
                status_file_size: normal(DARK0_SOFT, GRAY_245),
                status_position_in_file: normal(DARK0_SOFT, GRAY_245),
                status_mode: bold(DARK0_SOFT, BRIGHT_AQUA),
            },
            splash: SplashTheme {
                logo: normal(DARK0_SOFT, LIGHT2),
                tagline: normal(DARK0_SOFT, BRIGHT_BLUE),
                credits: normal(DARK0_SOFT, GRAY_245),
            },
            prompt: PromptTheme {
                base: normal(DARK0_HARD, NEUTRAL_YELLOW),
            },
        }
    }

    pub fn from_base16(base16: &Base16Theme) -> Self {
        let Base16Theme {
            base00: default_background,
            base01: lighter_background,
            base02: selection_background,
            base03: comments,
            base04: dark_foreground,
            base05: default_foreground,
            base06: light_foreground,
            base07: _light_background,
            base08,
            base09,
            base0a,
            base0b,
            base0c,
            base0d,
            base0e,
            base0f,
        } = *base16;

        Self {
            buffer: BufferTheme {
                syntax: SyntaxTheme {
                    text: normal(default_background, default_foreground),
                    text_current_line: normal(lighter_background, default_foreground),
                    cursor_focused: normal(light_foreground, default_background),
                    cursor_unfocused: normal(comments, default_background),
                    selection_background: Background(selection_background),
                    code_invalid: normal(default_background, base08),
                    code_constant: normal(default_background, base0b),
                    code_keyword: bold(default_background, base0e),
                    code_keyword_light: normal(default_background, base0e),
                    code_string: normal(default_background, base0b),
                    code_char: normal(default_background, base0c),
                    code_operator: normal(default_background, default_foreground),
                    code_macro_call: bold(default_background, base0f),
                    code_function_call: normal(default_background, base0d),
                    code_comment: normal(default_background, comments),
                    code_comment_doc: bold(default_background, comments),
                    code_link: underline(default_background, base09),
                    code_type: normal(default_background, base0a),
                },
                border: normal(lighter_background, dark_foreground),
                status_base: normal(lighter_background, default_background),
                status_frame_id_focused: normal(base0d, default_background),
                status_frame_id_unfocused: normal(comments, default_background),
                status_is_modified: normal(lighter_background, base09),
                status_is_not_modified: normal(lighter_background, comments),
                status_file_name: bold(lighter_background, base0d),
                status_file_size: normal(lighter_background, dark_foreground),
                status_position_in_file: normal(lighter_background, dark_foreground),
                status_mode: normal(lighter_background, base0b),
            },
            splash: SplashTheme {
                logo: normal(lighter_background, dark_foreground),
                tagline: normal(lighter_background, base0c),
                credits: normal(lighter_background, comments),
            },
            prompt: PromptTheme {
                base: normal(default_background, base0a),
            },
        }
    }
}

#[allow(dead_code)]
pub mod gruvbox {
    use crate::terminal::Colour;

    // Gruvbox colours
    pub const DARK0_HARD: Colour = Colour::rgb(29, 32, 33);
    pub const DARK0: Colour = Colour::rgb(40, 40, 40);
    pub const DARK0_SOFT: Colour = Colour::rgb(50, 48, 47);
    pub const DARK1: Colour = Colour::rgb(60, 56, 54);
    pub const DARK2: Colour = Colour::rgb(80, 73, 69);
    pub const DARK3: Colour = Colour::rgb(102, 92, 84);
    pub const DARK4: Colour = Colour::rgb(124, 111, 100);

    pub const GRAY_245: Colour = Colour::rgb(146, 131, 116);
    pub const GRAY_244: Colour = Colour::rgb(146, 131, 116);

    pub const LIGHT0_HARD: Colour = Colour::rgb(249, 245, 215);
    pub const LIGHT0: Colour = Colour::rgb(251, 241, 199);
    pub const LIGHT0_SOFT: Colour = Colour::rgb(242, 229, 188);
    pub const LIGHT1: Colour = Colour::rgb(235, 219, 178);
    pub const LIGHT2: Colour = Colour::rgb(213, 196, 161);
    pub const LIGHT3: Colour = Colour::rgb(189, 174, 147);
    pub const LIGHT4: Colour = Colour::rgb(168, 153, 132);

    pub const BRIGHT_RED: Colour = Colour::rgb(251, 73, 52);
    pub const BRIGHT_GREEN: Colour = Colour::rgb(184, 187, 38);
    pub const BRIGHT_YELLOW: Colour = Colour::rgb(250, 189, 47);
    pub const BRIGHT_BLUE: Colour = Colour::rgb(131, 165, 152);
    pub const BRIGHT_PURPLE: Colour = Colour::rgb(211, 134, 155);
    pub const BRIGHT_AQUA: Colour = Colour::rgb(142, 192, 124);
    pub const BRIGHT_ORANGE: Colour = Colour::rgb(254, 128, 25);

    pub const NEUTRAL_RED: Colour = Colour::rgb(204, 36, 29);
    pub const NEUTRAL_GREEN: Colour = Colour::rgb(152, 151, 26);
    pub const NEUTRAL_YELLOW: Colour = Colour::rgb(215, 153, 33);
    pub const NEUTRAL_BLUE: Colour = Colour::rgb(69, 133, 136);
    pub const NEUTRAL_PURPLE: Colour = Colour::rgb(177, 98, 134);
    pub const NEUTRAL_AQUA: Colour = Colour::rgb(104, 157, 106);
    pub const NEUTRAL_ORANGE: Colour = Colour::rgb(214, 93, 14);

    pub const FADED_RED: Colour = Colour::rgb(157, 0, 6);
    pub const FADED_GREEN: Colour = Colour::rgb(121, 116, 14);
    pub const FADED_YELLOW: Colour = Colour::rgb(181, 118, 20);
    pub const FADED_BLUE: Colour = Colour::rgb(7, 102, 120);
    pub const FADED_PURPLE: Colour = Colour::rgb(143, 63, 113);
    pub const FADED_AQUA: Colour = Colour::rgb(66, 123, 88);
    pub const FADED_ORANGE: Colour = Colour::rgb(175, 58, 3);
}

#[inline]
fn normal(background: Colour, foreground: Colour) -> Style {
    Style::normal(Background(background), Foreground(foreground))
}

#[inline]
fn bold(background: Colour, foreground: Colour) -> Style {
    Style::bold(Background(background), Foreground(foreground))
}

#[inline]
fn underline(background: Colour, foreground: Colour) -> Style {
    Style::underline(Background(background), Foreground(foreground))
}
