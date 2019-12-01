use super::{
    buffer::Theme as BufferTheme, prompt::Theme as PromptTheme, splash::Theme as SplashTheme,
};
use crate::terminal::{Background, Colour, Foreground, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub buffer: BufferTheme,
    pub splash: SplashTheme,
    pub prompt: PromptTheme,
}

impl Theme {
    pub fn solarized() -> Self {
        use solarized::*;

        Self {
            buffer: BufferTheme {
                text: normal(BASE03, BASE0),
                text_current_line: normal(BASE02, BASE0),
                border: normal(BASE02, BASE01),
                cursor_focused: normal(BASE2, BASE00),
                cursor_unfocused: normal(BASE01, BASE0),
                selection_background: Background(Colour::rgb(8, 23, 34)),
                status_base: normal(BASE02, BASE02),
                status_frame_id_focused: normal(BASE01, BASE2),
                status_frame_id_unfocused: normal(BASE01, BASE2),
                status_is_modified: normal(BASE02, ORANGE),
                status_is_not_modified: normal(BASE02, ORANGE),
                status_file_name: bold(BASE02, CYAN),
                status_file_size: normal(BASE02, BASE01),
                status_position_in_file: normal(BASE02, BASE01),
                status_mode: normal(BASE02, GREEN),
            },
            splash: SplashTheme {
                logo: normal(BASE03, BASE3),
                tagline: normal(BASE03, Colour::rgb(153, 246, 227)),
                credits: normal(BASE03, Colour::rgb(153, 246, 227)),
            },
            prompt: PromptTheme {
                base: normal(Colour::rgb(8, 23, 34), BASE0),
            },
        }
    }

    pub fn gruvbox() -> Self {
        use gruvbox::*;

        Self {
            buffer: BufferTheme {
                text: normal(DARK0_SOFT, LIGHT0_HARD),
                text_current_line: normal(DARK0, LIGHT0_HARD),
                border: normal(DARK0, GRAY_245),
                cursor_focused: normal(LIGHT0, DARK0),
                cursor_unfocused: normal(GRAY_245, DARK0_HARD),
                selection_background: Background(DARK0_HARD),
                status_base: normal(DARK0, DARK0),
                status_frame_id_focused: normal(BRIGHT_BLUE, DARK0_HARD),
                status_frame_id_unfocused: normal(GRAY_245, DARK0_HARD),
                status_is_modified: normal(DARK0, FADED_ORANGE),
                status_is_not_modified: normal(DARK0, GRAY_245),
                status_file_name: bold(DARK0_HARD, BRIGHT_BLUE),
                status_file_size: normal(DARK0_HARD, GRAY_245),
                status_position_in_file: normal(DARK0_HARD, GRAY_245),
                status_mode: normal(DARK0, BRIGHT_AQUA),
            },
            splash: SplashTheme {
                logo: normal(DARK0_SOFT, LIGHT2),
                tagline: normal(DARK0_SOFT, BRIGHT_BLUE),
                credits: normal(DARK0_SOFT, GRAY_245),
            },
            prompt: PromptTheme {
                base: normal(DARK1, NEUTRAL_YELLOW),
            },
        }
    }
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

#[allow(dead_code)]
pub mod solarized {
    use crate::terminal::Colour;

    // Solarized colours
    pub const BASE03: Colour = Colour::rgb(0, 43, 54);
    pub const BASE02: Colour = Colour::rgb(7, 54, 66);
    pub const BASE01: Colour = Colour::rgb(88, 110, 117);
    pub const BASE00: Colour = Colour::rgb(101, 123, 131);
    pub const BASE0: Colour = Colour::rgb(131, 148, 150);
    pub const BASE1: Colour = Colour::rgb(147, 161, 161);
    pub const BASE2: Colour = Colour::rgb(238, 232, 213);
    pub const BASE3: Colour = Colour::rgb(253, 246, 227);
    pub const YELLOW: Colour = Colour::rgb(181, 137, 0);
    pub const ORANGE: Colour = Colour::rgb(203, 75, 22);
    pub const RED: Colour = Colour::rgb(220, 50, 47);
    pub const MAGENTA: Colour = Colour::rgb(211, 54, 130);
    pub const VIOLET: Colour = Colour::rgb(108, 113, 196);
    pub const BLUE: Colour = Colour::rgb(38, 139, 210);
    pub const CYAN: Colour = Colour::rgb(42, 161, 152);
    pub const GREEN: Colour = Colour::rgb(133, 153, 0);
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
