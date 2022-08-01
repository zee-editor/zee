pub mod base16;
pub use self::base16::Base16Theme;

use zi::terminal::{Colour, Style};

use super::{
    buffer::{status_bar::Theme as StatusBarTheme, Theme as BufferTheme},
    edit_tree_viewer::Theme as EditTreeViewerTheme,
    prompt::Theme as PromptTheme,
    splash::Theme as SplashTheme,
};
use crate::syntax::highlight::Theme as SyntaxTheme;

pub const THEMES: [(Theme, &str); 31] = [
    (Theme::gruvbox(), "zee-gruvbox"),
    (
        Theme::from_base16(&base16::SOLARIZED_DARK),
        "base16-solarized-dark",
    ),
    (
        Theme::from_base16(&base16::SYNTH_MIDNIGHT),
        "base16-synth-midnight",
    ),
    (Theme::from_base16(&base16::HELIOS), "base16-helios"),
    (
        Theme::from_base16(&base16::GRUVBOX_DARK_HARD),
        "base16-gruvbox-dark-hard",
    ),
    (
        Theme::from_base16(&base16::GRUVBOX_DARK_PALE),
        "base16-gruvbox-dark-pale",
    ),
    (
        Theme::from_base16(&base16::GRUVBOX_DARK_SOFT),
        "base16-gruvbox-dark-soft",
    ),
    (
        Theme::from_base16(&base16::GRUVBOX_LIGHT_HARD),
        "base16-gruvbox-light-hard",
    ),
    (
        Theme::from_base16(&base16::GRUVBOX_LIGHT_SOFT),
        "base16-gruvbox-light-soft",
    ),
    (
        Theme::from_base16(&base16::SOLARIZED_LIGHT),
        "base16-solarized-light",
    ),
    (
        Theme::from_base16(&base16::DEFAULT_DARK),
        "base16-default-dark",
    ),
    (
        Theme::from_base16(&base16::DEFAULT_LIGHT),
        "base16-default-light",
    ),
    (Theme::from_base16(&base16::EIGHTIES), "base16-eighties"),
    (Theme::from_base16(&base16::MOCHA), "base16-mocha"),
    (Theme::from_base16(&base16::OCEAN), "base16-ocean"),
    (Theme::from_base16(&base16::CUPCAKE), "base16-cupcake"),
    (Theme::from_base16(&base16::ONEDARK), "base16-onedark"),
    (Theme::from_base16(&base16::MATERIAL), "base16-material"),
    (
        Theme::from_base16(&base16::MATERIAL_DARKER),
        "base16-material-darker",
    ),
    (
        Theme::from_base16(&base16::MATERIAL_PALENIGHT),
        "base16-material-palenight",
    ),
    (
        Theme::from_base16(&base16::MATERIAL_LIGHTER),
        "base16-material-lighter",
    ),
    (Theme::from_base16(&base16::ATLAS), "base16-atlas"),
    (Theme::from_base16(&base16::CIRCUS), "base16-circus"),
    (Theme::from_base16(&base16::CODESCHOOL), "base16-codeschool"),
    (Theme::from_base16(&base16::ESPRESSO), "base16-espresso"),
    (Theme::from_base16(&base16::DECAF), "base16-decaf"),
    (Theme::from_base16(&base16::ICY), "base16-icy"),
    (Theme::from_base16(&base16::WOODLAND), "base16-woodland"),
    (Theme::from_base16(&base16::ZENBURN), "base16-zenburn"),
    (Theme::from_base16(&base16::XCODE_DUSK), "base16-xcode-dusk"),
    (
        Theme::from_base16(&base16::VSCODE_DARK),
        "base16-vscode-dark",
    ),
];

#[derive(Clone, Debug)]
pub struct Theme {
    pub buffer: BufferTheme,
    pub splash: SplashTheme,
    pub prompt: PromptTheme,
}

impl Theme {
    pub const fn gruvbox() -> Self {
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
                    cursor_focused: normal(LIGHT0, DARK0),
                    cursor_unfocused: normal(GRAY_245, DARK0_HARD),
                    selection_background: DARK0_HARD,
                    text: normal(DARK0, LIGHT1),
                    text_current_line: normal(DARK0_HARD, LIGHT1),
                    code_char: normal(DARK0_SOFT, BRIGHT_GREEN),
                    code_comment: normal(DARK0_SOFT, DARK4),
                    code_comment_doc: normal(DARK0_SOFT, LIGHT4),
                    code_constant: normal(DARK0_SOFT, BRIGHT_GREEN),
                    code_function_call: normal(DARK0_SOFT, BRIGHT_BLUE),
                    code_invalid: underline(DARK0_SOFT, BRIGHT_RED),
                    code_keyword: bold(DARK0_SOFT, BRIGHT_RED),
                    code_keyword_light: normal(DARK0_SOFT, BRIGHT_RED),
                    code_link: underline(DARK0_SOFT, LIGHT3),
                    code_macro_call: normal(DARK0_SOFT, BRIGHT_ORANGE),
                    code_operator: normal(DARK0_SOFT, GRAY_245),
                    code_string: normal(DARK0_SOFT, BRIGHT_GREEN),
                    code_type: normal(DARK0_SOFT, BRIGHT_YELLOW),
                    code_variant: normal(DARK0_SOFT, BRIGHT_PURPLE),
                },
                edit_tree_viewer: EditTreeViewerTheme {
                    current_revision: bold(DARK0, BRIGHT_RED),
                    master_revision: bold(DARK0, LIGHT1),
                    master_connector: bold(DARK0, LIGHT1),
                    alternate_revision: normal(DARK0, DARK4),
                    alternate_connector: normal(DARK0, DARK4),
                },
                border: normal(DARK0_HARD, GRAY_245),
                status_bar: StatusBarTheme {
                    base: normal(DARK0_SOFT, DARK0),
                    frame_id_focused: normal(BRIGHT_BLUE, DARK0_HARD),
                    frame_id_unfocused: normal(GRAY_245, DARK0_HARD),
                    is_modified: normal(DARK0, BRIGHT_RED),
                    is_not_modified: normal(DARK0, GRAY_245),
                    file_name: normal(DARK0_SOFT, BRIGHT_BLUE),
                    file_size: normal(DARK0_SOFT, GRAY_245),
                    position_in_file: normal(DARK0_SOFT, GRAY_245),
                    mode: bold(DARK0_SOFT, BRIGHT_AQUA),
                },
            },
            splash: SplashTheme {
                logo: normal(DARK0_SOFT, LIGHT2),
                tagline: normal(DARK0_SOFT, BRIGHT_BLUE),
                credits: normal(DARK0_SOFT, GRAY_245),
            },
            prompt: PromptTheme {
                input: normal(DARK0_HARD, NEUTRAL_YELLOW),
                action: normal(BRIGHT_BLUE, DARK0_HARD),
                cursor: normal(LIGHT0, DARK0),
                file_size: GRAY_245,
                mode: BRIGHT_AQUA,
                item_focused_background: DARK0_HARD,
                item_unfocused_background: DARK0,
                item_file_foreground: LIGHT1,
                item_directory_foreground: BRIGHT_RED,
            },
        }
    }

    pub const fn from_base16(base16: &Base16Theme) -> Self {
        let Base16Theme {
            // Default Background
            base00: default_background,
            // Lighter Background (Used for status bars)
            base01: lighter_background,
            // Selection Background
            base02: selection_background,
            // Comments, Invisibles, Line Highlighting
            base03: comments,
            // Dark Foreground (Used for status bars)
            base04: dark_foreground,
            // Default Foreground, Caret, Delimiters, Operators
            base05: default_foreground,
            // Light Foreground (Not often used)
            base06: light_foreground,
            // Light Background (Not often used)
            base07: light_background,
            // Variables, XML Tags, Markup Link Text, Markup Lists, Diff Deleted
            base08: variables,
            // Integers, Boolean, Constants, XML Attributes, Markup Link Url
            base09: constants,
            // Classes, Markup Bold, Search Text Background
            base0a: classes,
            // Strings, Inherited Class, Markup Code, Diff Inserted
            base0b: strings,
            // Support, Regular Expressions, Escape Characters, Markup Quotes
            base0c: support,
            // Functions, Methods, Attribute IDs, Headings
            base0d: functions,
            // Keywords, Storage, Selector, Markup Italic, Diff Changed
            base0e: keywords,
            // Deprecated, Opening/Closing Embedded Language Tags, e.g. <?php ?>
            base0f: embedded,
        } = *base16;

        Self {
            buffer: BufferTheme {
                syntax: SyntaxTheme {
                    cursor_focused: normal(light_foreground, default_background),
                    cursor_unfocused: normal(comments, default_background),
                    selection_background,
                    text: normal(default_background, default_foreground),
                    text_current_line: normal(lighter_background, default_foreground),
                    code_char: normal(default_background, support),
                    code_comment: normal(default_background, comments),
                    code_comment_doc: bold(default_background, comments),
                    code_constant: normal(default_background, strings),
                    code_function_call: normal(default_background, functions),
                    code_invalid: underline(default_background, variables),
                    code_keyword: normal(default_background, variables),
                    code_keyword_light: normal(default_background, variables),
                    code_link: underline(default_background, constants),
                    code_macro_call: bold(default_background, embedded),
                    code_operator: normal(default_background, default_foreground),
                    code_string: normal(default_background, strings),
                    code_type: normal(default_background, classes),
                    code_variant: normal(default_background, classes),
                },
                edit_tree_viewer: EditTreeViewerTheme {
                    current_revision: bold(default_background, embedded),
                    master_revision: bold(default_background, variables),
                    master_connector: bold(default_background, default_foreground),
                    alternate_revision: normal(default_background, default_foreground),
                    alternate_connector: normal(default_background, comments),
                },
                border: normal(lighter_background, dark_foreground),
                status_bar: StatusBarTheme {
                    base: normal(lighter_background, default_background),
                    frame_id_focused: normal(functions, default_background),
                    frame_id_unfocused: normal(comments, default_background),
                    is_modified: normal(lighter_background, constants),
                    is_not_modified: normal(lighter_background, comments),
                    file_name: bold(lighter_background, strings),
                    file_size: normal(lighter_background, dark_foreground),
                    position_in_file: normal(light_background, dark_foreground),
                    mode: normal(lighter_background, strings),
                },
            },
            splash: SplashTheme {
                logo: normal(lighter_background, dark_foreground),
                tagline: normal(lighter_background, support),
                credits: normal(lighter_background, comments),
            },
            prompt: PromptTheme {
                input: normal(default_background, classes),
                action: normal(functions, default_background),
                cursor: normal(light_foreground, default_background),
                file_size: dark_foreground,
                mode: strings,
                item_focused_background: default_background,
                item_unfocused_background: lighter_background,
                item_file_foreground: default_foreground,
                item_directory_foreground: keywords,
            },
        }
    }
}

#[allow(dead_code)]
pub mod gruvbox {
    use zi::Colour;

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
const fn normal(background: Colour, foreground: Colour) -> Style {
    Style::normal(background, foreground)
}

#[inline]
const fn bold(background: Colour, foreground: Colour) -> Style {
    Style::bold(background, foreground)
}

#[inline]
const fn underline(background: Colour, foreground: Colour) -> Style {
    Style::underline(background, foreground)
}
