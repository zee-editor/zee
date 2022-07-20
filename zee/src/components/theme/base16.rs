use zi::Colour;

/// Represents a base16 theme.
///
/// Colours base00 to base07 are typically variations of a shade and run from
/// darkest to lightest.  These colours are used for foreground and background,
/// status bars, line highlighting and such.  Colours base08 to base0f are
/// typically individual colours used for types, operators, names and variables.
///
/// In order to create a dark theme, colours base00 to base07 should span from
/// dark to light.  For a light theme, these colours should span from light to
/// dark.
pub struct Base16Theme {
    /// The background colour.
    pub base00: Colour,

    /// The status bar's and current line's colour.
    pub base01: Colour,

    /// The highlight colour when selecting text.
    pub base02: Colour,

    /// The comment colour.
    ///
    /// This colour is not only applied to comments but will also indicate the
    /// inactive subwindows, if any.  Furthermore, this colour will be applied
    /// on the indicator telling that the current file was not modified located
    /// next to the respective subwindow's index, typically formed by a `-`.
    pub base03: Colour,

    /// The status bar's default text.
    ///
    /// Any text to be displayed in the status bar which is not coloured by
    /// applying the respective configured values from one of the fields will be
    /// displayed using this colour.
    ///
    /// Furthermore, when identifying a syntactical issue, the corresponding
    /// section will be underlined and highlighted using this colour.
    pub base04: Colour,

    /// The variables' colour.
    ///
    /// Each variable, operator and delimiter will be displayed using this
    /// colour.  Furthermore, plain text will be coloured by this field's value.
    pub base05: Colour,

    /// The cursor's colour.
    pub base06: Colour,

    /// Currently unused.
    pub base07: Colour,

    /// Keywords of the coding language.
    pub base08: Colour,

    /// The indication that a file was modified.
    ///
    /// In the status bar at the bottom, next to the current subwindow's index,
    /// there is a single character which changes to `+` whenever a file was
    /// edited.  The colour specified by this field will be applied on this `+`.
    pub base09: Colour,

    /// Classes and scope elements.
    pub base0a: Colour,

    /// The colour of concrete values like numerals and strings.
    ///
    /// This colour is not only applied on constants like numbers and strings
    /// but will also be used to indicate the file name in the status bar.
    /// Furthermore, also symbols treated as constants are coloured by this
    /// field's value.
    pub base0b: Colour,

    /// The colour of single character literals.
    pub base0c: Colour,

    /// The colour setting for functions.
    ///
    /// This colour will furthermore be used to highlight the index of the
    /// currently focused file.
    pub base0d: Colour,

    /// The directories' colour in the file selection dialogue.
    pub base0e: Colour,

    /// The colour for Rust tags and Markdown headings.
    pub base0f: Colour,
}

pub const GRUVBOX_DARK_HARD: Base16Theme = Base16Theme {
    base00: Colour::rgb(29, 32, 33),
    base01: Colour::rgb(60, 56, 54),
    base02: Colour::rgb(80, 73, 69),
    base03: Colour::rgb(102, 92, 84),
    base04: Colour::rgb(189, 174, 147),
    base05: Colour::rgb(213, 196, 161),
    base06: Colour::rgb(235, 219, 178),
    base07: Colour::rgb(251, 241, 199),
    base08: Colour::rgb(251, 73, 52),
    base09: Colour::rgb(254, 128, 25),
    base0a: Colour::rgb(250, 189, 47),
    base0b: Colour::rgb(184, 187, 38),
    base0c: Colour::rgb(142, 192, 124),
    base0d: Colour::rgb(131, 165, 152),
    base0e: Colour::rgb(211, 134, 155),
    base0f: Colour::rgb(214, 93, 14),
};

pub const GRUVBOX_DARK_PALE: Base16Theme = Base16Theme {
    base00: Colour::rgb(38, 38, 38),
    base01: Colour::rgb(58, 58, 58),
    base02: Colour::rgb(78, 78, 78),
    base03: Colour::rgb(138, 138, 138),
    base04: Colour::rgb(148, 148, 148),
    base05: Colour::rgb(218, 185, 151),
    base06: Colour::rgb(213, 196, 161),
    base07: Colour::rgb(235, 219, 178),
    base08: Colour::rgb(215, 95, 95),
    base09: Colour::rgb(255, 135, 0),
    base0a: Colour::rgb(255, 175, 0),
    base0b: Colour::rgb(175, 175, 0),
    base0c: Colour::rgb(133, 173, 133),
    base0d: Colour::rgb(131, 173, 173),
    base0e: Colour::rgb(212, 133, 173),
    base0f: Colour::rgb(214, 93, 14),
};

pub const GRUVBOX_DARK_SOFT: Base16Theme = Base16Theme {
    base00: Colour::rgb(50, 48, 47),
    base01: Colour::rgb(60, 56, 54),
    base02: Colour::rgb(80, 73, 69),
    base03: Colour::rgb(102, 92, 84),
    base04: Colour::rgb(189, 174, 147),
    base05: Colour::rgb(213, 196, 161),
    base06: Colour::rgb(235, 219, 178),
    base07: Colour::rgb(251, 241, 199),
    base08: Colour::rgb(251, 73, 52),
    base09: Colour::rgb(254, 128, 25),
    base0a: Colour::rgb(250, 189, 47),
    base0b: Colour::rgb(184, 187, 38),
    base0c: Colour::rgb(142, 192, 124),
    base0d: Colour::rgb(131, 165, 152),
    base0e: Colour::rgb(211, 134, 155),
    base0f: Colour::rgb(214, 93, 14),
};

pub const GRUVBOX_LIGHT_HARD: Base16Theme = Base16Theme {
    base00: Colour::rgb(249, 245, 215),
    base01: Colour::rgb(235, 219, 178),
    base02: Colour::rgb(213, 196, 161),
    base03: Colour::rgb(189, 174, 147),
    base04: Colour::rgb(102, 92, 84),
    base05: Colour::rgb(80, 73, 69),
    base06: Colour::rgb(60, 56, 54),
    base07: Colour::rgb(40, 40, 40),
    base08: Colour::rgb(157, 0, 6),
    base09: Colour::rgb(175, 58, 3),
    base0a: Colour::rgb(181, 118, 20),
    base0b: Colour::rgb(121, 116, 14),
    base0c: Colour::rgb(66, 123, 88),
    base0d: Colour::rgb(7, 102, 120),
    base0e: Colour::rgb(143, 63, 113),
    base0f: Colour::rgb(214, 93, 14),
};

pub const GRUVBOX_LIGHT_SOFT: Base16Theme = Base16Theme {
    base00: Colour::rgb(242, 229, 188),
    base01: Colour::rgb(235, 219, 178),
    base02: Colour::rgb(213, 196, 161),
    base03: Colour::rgb(189, 174, 147),
    base04: Colour::rgb(102, 92, 84),
    base05: Colour::rgb(80, 73, 69),
    base06: Colour::rgb(60, 56, 54),
    base07: Colour::rgb(40, 40, 40),
    base08: Colour::rgb(157, 0, 6),
    base09: Colour::rgb(175, 58, 3),
    base0a: Colour::rgb(181, 118, 20),
    base0b: Colour::rgb(121, 116, 14),
    base0c: Colour::rgb(66, 123, 88),
    base0d: Colour::rgb(7, 102, 120),
    base0e: Colour::rgb(143, 63, 113),
    base0f: Colour::rgb(214, 93, 14),
};

pub const SOLARIZED_DARK: Base16Theme = Base16Theme {
    base00: Colour::rgb(0, 43, 54),
    base01: Colour::rgb(7, 54, 66),
    base02: Colour::rgb(88, 110, 117),
    base03: Colour::rgb(101, 123, 131),
    base04: Colour::rgb(131, 148, 150),
    base05: Colour::rgb(147, 161, 161),
    base06: Colour::rgb(238, 232, 213),
    base07: Colour::rgb(253, 246, 227),
    base08: Colour::rgb(220, 50, 47),
    base09: Colour::rgb(203, 75, 22),
    base0a: Colour::rgb(181, 137, 0),
    base0b: Colour::rgb(133, 153, 0),
    base0c: Colour::rgb(42, 161, 152),
    base0d: Colour::rgb(38, 139, 210),
    base0e: Colour::rgb(108, 113, 196),
    base0f: Colour::rgb(211, 54, 130),
};

pub const SOLARIZED_LIGHT: Base16Theme = Base16Theme {
    base00: Colour::rgb(253, 246, 227),
    base01: Colour::rgb(238, 232, 213),
    base02: Colour::rgb(147, 161, 161),
    base03: Colour::rgb(131, 148, 150),
    base04: Colour::rgb(101, 123, 131),
    base05: Colour::rgb(88, 110, 117),
    base06: Colour::rgb(7, 54, 66),
    base07: Colour::rgb(0, 43, 54),
    base08: Colour::rgb(220, 50, 47),
    base09: Colour::rgb(203, 75, 22),
    base0a: Colour::rgb(181, 137, 0),
    base0b: Colour::rgb(133, 153, 0),
    base0c: Colour::rgb(42, 161, 152),
    base0d: Colour::rgb(38, 139, 210),
    base0e: Colour::rgb(108, 113, 196),
    base0f: Colour::rgb(211, 54, 130),
};

pub const SYNTH_MIDNIGHT: Base16Theme = Base16Theme {
    base00: Colour::rgb(4, 4, 4),
    base01: Colour::rgb(20, 20, 20),
    base02: Colour::rgb(36, 36, 36),
    base03: Colour::rgb(97, 80, 122),
    base04: Colour::rgb(191, 187, 191),
    base05: Colour::rgb(223, 219, 223),
    base06: Colour::rgb(239, 235, 239),
    base07: Colour::rgb(255, 251, 255),
    base08: Colour::rgb(181, 59, 80),
    base09: Colour::rgb(228, 96, 14),
    base0a: Colour::rgb(218, 232, 77),
    base0b: Colour::rgb(6, 234, 97),
    base0c: Colour::rgb(124, 237, 233),
    base0d: Colour::rgb(3, 174, 255),
    base0e: Colour::rgb(234, 92, 226),
    base0f: Colour::rgb(157, 77, 14),
};

pub const DEFAULT_DARK: Base16Theme = Base16Theme {
    base00: Colour::rgb(24, 24, 24),
    base01: Colour::rgb(40, 40, 40),
    base02: Colour::rgb(56, 56, 56),
    base03: Colour::rgb(88, 88, 88),
    base04: Colour::rgb(184, 184, 184),
    base05: Colour::rgb(216, 216, 216),
    base06: Colour::rgb(232, 232, 232),
    base07: Colour::rgb(248, 248, 248),
    base08: Colour::rgb(171, 70, 66),
    base09: Colour::rgb(220, 150, 86),
    base0a: Colour::rgb(247, 202, 136),
    base0b: Colour::rgb(161, 181, 108),
    base0c: Colour::rgb(134, 193, 185),
    base0d: Colour::rgb(124, 175, 194),
    base0e: Colour::rgb(186, 139, 175),
    base0f: Colour::rgb(161, 105, 70),
};

pub const DEFAULT_LIGHT: Base16Theme = Base16Theme {
    base00: Colour::rgb(248, 248, 248),
    base01: Colour::rgb(232, 232, 232),
    base02: Colour::rgb(216, 216, 216),
    base03: Colour::rgb(184, 184, 184),
    base04: Colour::rgb(88, 88, 88),
    base05: Colour::rgb(56, 56, 56),
    base06: Colour::rgb(40, 40, 40),
    base07: Colour::rgb(24, 24, 24),
    base08: Colour::rgb(171, 70, 66),
    base09: Colour::rgb(220, 150, 86),
    base0a: Colour::rgb(247, 202, 136),
    base0b: Colour::rgb(161, 181, 108),
    base0c: Colour::rgb(134, 193, 185),
    base0d: Colour::rgb(124, 175, 194),
    base0e: Colour::rgb(186, 139, 175),
    base0f: Colour::rgb(161, 105, 70),
};

pub const EIGHTIES: Base16Theme = Base16Theme {
    base00: Colour::rgb(45, 45, 45),
    base01: Colour::rgb(57, 57, 57),
    base02: Colour::rgb(81, 81, 81),
    base03: Colour::rgb(116, 115, 105),
    base04: Colour::rgb(160, 159, 147),
    base05: Colour::rgb(211, 208, 200),
    base06: Colour::rgb(232, 230, 223),
    base07: Colour::rgb(242, 240, 236),
    base08: Colour::rgb(242, 119, 122),
    base09: Colour::rgb(249, 145, 87),
    base0a: Colour::rgb(255, 204, 102),
    base0b: Colour::rgb(153, 204, 153),
    base0c: Colour::rgb(102, 204, 204),
    base0d: Colour::rgb(102, 153, 204),
    base0e: Colour::rgb(204, 153, 204),
    base0f: Colour::rgb(210, 123, 83),
};

pub const MOCHA: Base16Theme = Base16Theme {
    base00: Colour::rgb(59, 50, 40),
    base01: Colour::rgb(83, 70, 54),
    base02: Colour::rgb(100, 82, 64),
    base03: Colour::rgb(126, 112, 90),
    base04: Colour::rgb(184, 175, 173),
    base05: Colour::rgb(208, 200, 198),
    base06: Colour::rgb(233, 225, 221),
    base07: Colour::rgb(245, 238, 235),
    base08: Colour::rgb(203, 96, 119),
    base09: Colour::rgb(210, 139, 113),
    base0a: Colour::rgb(244, 188, 135),
    base0b: Colour::rgb(190, 181, 91),
    base0c: Colour::rgb(123, 189, 164),
    base0d: Colour::rgb(138, 179, 181),
    base0e: Colour::rgb(168, 155, 185),
    base0f: Colour::rgb(187, 149, 132),
};

pub const OCEAN: Base16Theme = Base16Theme {
    base00: Colour::rgb(43, 48, 59),
    base01: Colour::rgb(52, 61, 70),
    base02: Colour::rgb(79, 91, 102),
    base03: Colour::rgb(101, 115, 126),
    base04: Colour::rgb(167, 173, 186),
    base05: Colour::rgb(192, 197, 206),
    base06: Colour::rgb(223, 225, 232),
    base07: Colour::rgb(239, 241, 245),
    base08: Colour::rgb(191, 97, 106),
    base09: Colour::rgb(208, 135, 112),
    base0a: Colour::rgb(235, 203, 139),
    base0b: Colour::rgb(163, 190, 140),
    base0c: Colour::rgb(150, 181, 180),
    base0d: Colour::rgb(143, 161, 179),
    base0e: Colour::rgb(180, 142, 173),
    base0f: Colour::rgb(171, 121, 103),
};

pub const CUPCAKE: Base16Theme = Base16Theme {
    base00: Colour::rgb(251, 241, 242),
    base01: Colour::rgb(242, 241, 244),
    base02: Colour::rgb(216, 213, 221),
    base03: Colour::rgb(191, 185, 198),
    base04: Colour::rgb(165, 157, 175),
    base05: Colour::rgb(139, 129, 152),
    base06: Colour::rgb(114, 103, 126),
    base07: Colour::rgb(88, 80, 98),
    base08: Colour::rgb(213, 126, 133),
    base09: Colour::rgb(235, 183, 144),
    base0a: Colour::rgb(220, 177, 108),
    base0b: Colour::rgb(163, 179, 103),
    base0c: Colour::rgb(105, 169, 167),
    base0d: Colour::rgb(114, 151, 185),
    base0e: Colour::rgb(187, 153, 180),
    base0f: Colour::rgb(186, 165, 140),
};

pub const ONEDARK: Base16Theme = Base16Theme {
    base00: Colour::rgb(40, 44, 52),
    base01: Colour::rgb(53, 59, 69),
    base02: Colour::rgb(62, 68, 81),
    base03: Colour::rgb(84, 88, 98),
    base04: Colour::rgb(86, 92, 100),
    base05: Colour::rgb(171, 178, 191),
    base06: Colour::rgb(182, 189, 202),
    base07: Colour::rgb(200, 204, 212),
    base08: Colour::rgb(224, 108, 117),
    base09: Colour::rgb(209, 154, 102),
    base0a: Colour::rgb(229, 192, 123),
    base0b: Colour::rgb(152, 195, 121),
    base0c: Colour::rgb(86, 182, 194),
    base0d: Colour::rgb(97, 175, 239),
    base0e: Colour::rgb(198, 120, 221),
    base0f: Colour::rgb(190, 80, 70),
};

pub const MATERIAL: Base16Theme = Base16Theme {
    base00: Colour::rgb(38, 50, 56),
    base01: Colour::rgb(46, 60, 67),
    base02: Colour::rgb(49, 69, 73),
    base03: Colour::rgb(84, 110, 122),
    base04: Colour::rgb(178, 204, 214),
    base05: Colour::rgb(238, 255, 255),
    base06: Colour::rgb(238, 255, 255),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(240, 113, 120),
    base09: Colour::rgb(247, 140, 108),
    base0a: Colour::rgb(255, 203, 107),
    base0b: Colour::rgb(195, 232, 141),
    base0c: Colour::rgb(137, 221, 255),
    base0d: Colour::rgb(130, 170, 255),
    base0e: Colour::rgb(199, 146, 234),
    base0f: Colour::rgb(255, 83, 112),
};

pub const MATERIAL_DARKER: Base16Theme = Base16Theme {
    base00: Colour::rgb(33, 33, 33),
    base01: Colour::rgb(48, 48, 48),
    base02: Colour::rgb(53, 53, 53),
    base03: Colour::rgb(74, 74, 74),
    base04: Colour::rgb(178, 204, 214),
    base05: Colour::rgb(238, 255, 255),
    base06: Colour::rgb(238, 255, 255),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(240, 113, 120),
    base09: Colour::rgb(247, 140, 108),
    base0a: Colour::rgb(255, 203, 107),
    base0b: Colour::rgb(195, 232, 141),
    base0c: Colour::rgb(137, 221, 255),
    base0d: Colour::rgb(130, 170, 255),
    base0e: Colour::rgb(199, 146, 234),
    base0f: Colour::rgb(255, 83, 112),
};

pub const MATERIAL_LIGHTER: Base16Theme = Base16Theme {
    base00: Colour::rgb(250, 250, 250),
    base01: Colour::rgb(231, 234, 236),
    base02: Colour::rgb(204, 234, 231),
    base03: Colour::rgb(204, 215, 218),
    base04: Colour::rgb(135, 150, 176),
    base05: Colour::rgb(128, 203, 196),
    base06: Colour::rgb(128, 203, 196),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(255, 83, 112),
    base09: Colour::rgb(247, 109, 71),
    base0a: Colour::rgb(255, 182, 44),
    base0b: Colour::rgb(145, 184, 89),
    base0c: Colour::rgb(57, 173, 181),
    base0d: Colour::rgb(97, 130, 184),
    base0e: Colour::rgb(124, 77, 255),
    base0f: Colour::rgb(229, 57, 53),
};

pub const MATERIAL_PALENIGHT: Base16Theme = Base16Theme {
    base00: Colour::rgb(41, 45, 62),
    base01: Colour::rgb(68, 66, 103),
    base02: Colour::rgb(50, 55, 77),
    base03: Colour::rgb(103, 110, 149),
    base04: Colour::rgb(135, 150, 176),
    base05: Colour::rgb(149, 157, 203),
    base06: Colour::rgb(149, 157, 203),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(240, 113, 120),
    base09: Colour::rgb(247, 140, 108),
    base0a: Colour::rgb(255, 203, 107),
    base0b: Colour::rgb(195, 232, 141),
    base0c: Colour::rgb(137, 221, 255),
    base0d: Colour::rgb(130, 170, 255),
    base0e: Colour::rgb(199, 146, 234),
    base0f: Colour::rgb(255, 83, 112),
};

pub const ATLAS: Base16Theme = Base16Theme {
    base00: Colour::rgb(0, 38, 53),
    base01: Colour::rgb(0, 56, 77),
    base02: Colour::rgb(81, 127, 141),
    base03: Colour::rgb(108, 139, 145),
    base04: Colour::rgb(134, 150, 150),
    base05: Colour::rgb(161, 161, 154),
    base06: Colour::rgb(230, 230, 220),
    base07: Colour::rgb(250, 250, 248),
    base08: Colour::rgb(255, 90, 103),
    base09: Colour::rgb(240, 142, 72),
    base0a: Colour::rgb(255, 204, 27),
    base0b: Colour::rgb(127, 192, 110),
    base0c: Colour::rgb(93, 215, 185),
    base0d: Colour::rgb(20, 116, 126),
    base0e: Colour::rgb(154, 112, 164),
    base0f: Colour::rgb(196, 48, 96),
};

pub const CIRCUS: Base16Theme = Base16Theme {
    base00: Colour::rgb(25, 25, 25),
    base01: Colour::rgb(32, 32, 32),
    base02: Colour::rgb(48, 48, 48),
    base03: Colour::rgb(95, 90, 96),
    base04: Colour::rgb(80, 80, 80),
    base05: Colour::rgb(167, 167, 167),
    base06: Colour::rgb(128, 128, 128),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(220, 101, 125),
    base09: Colour::rgb(75, 177, 167),
    base0a: Colour::rgb(195, 186, 99),
    base0b: Colour::rgb(132, 185, 124),
    base0c: Colour::rgb(75, 177, 167),
    base0d: Colour::rgb(99, 158, 228),
    base0e: Colour::rgb(184, 136, 226),
    base0f: Colour::rgb(184, 136, 226),
};

pub const CODESCHOOL: Base16Theme = Base16Theme {
    base00: Colour::rgb(35, 44, 49),
    base01: Colour::rgb(28, 54, 87),
    base02: Colour::rgb(42, 52, 58),
    base03: Colour::rgb(63, 73, 68),
    base04: Colour::rgb(132, 137, 140),
    base05: Colour::rgb(158, 167, 166),
    base06: Colour::rgb(167, 207, 163),
    base07: Colour::rgb(181, 216, 246),
    base08: Colour::rgb(42, 84, 145),
    base09: Colour::rgb(67, 130, 13),
    base0a: Colour::rgb(160, 59, 30),
    base0b: Colour::rgb(35, 121, 134),
    base0c: Colour::rgb(176, 47, 48),
    base0d: Colour::rgb(72, 77, 121),
    base0e: Colour::rgb(197, 152, 32),
    base0f: Colour::rgb(201, 131, 68),
};

pub const DECAF: Base16Theme = Base16Theme {
    base00: Colour::rgb(45, 45, 45),
    base01: Colour::rgb(57, 57, 57),
    base02: Colour::rgb(81, 81, 81),
    base03: Colour::rgb(119, 119, 119),
    base04: Colour::rgb(180, 183, 180),
    base05: Colour::rgb(204, 204, 204),
    base06: Colour::rgb(224, 224, 224),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(255, 127, 123),
    base09: Colour::rgb(255, 191, 112),
    base0a: Colour::rgb(255, 214, 124),
    base0b: Colour::rgb(190, 218, 120),
    base0c: Colour::rgb(190, 214, 255),
    base0d: Colour::rgb(144, 190, 225),
    base0e: Colour::rgb(239, 179, 247),
    base0f: Colour::rgb(255, 147, 179),
};

pub const ESPRESSO: Base16Theme = Base16Theme {
    base00: Colour::rgb(45, 45, 45),
    base01: Colour::rgb(57, 57, 57),
    base02: Colour::rgb(81, 81, 81),
    base03: Colour::rgb(119, 119, 119),
    base04: Colour::rgb(180, 183, 180),
    base05: Colour::rgb(204, 204, 204),
    base06: Colour::rgb(224, 224, 224),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(210, 82, 82),
    base09: Colour::rgb(249, 169, 89),
    base0a: Colour::rgb(255, 198, 109),
    base0b: Colour::rgb(165, 194, 97),
    base0c: Colour::rgb(190, 214, 255),
    base0d: Colour::rgb(108, 153, 187),
    base0e: Colour::rgb(209, 151, 217),
    base0f: Colour::rgb(249, 115, 148),
};

pub const HELIOS: Base16Theme = Base16Theme {
    base00: Colour::rgb(29, 32, 33),
    base01: Colour::rgb(56, 60, 62),
    base02: Colour::rgb(83, 88, 91),
    base03: Colour::rgb(111, 117, 121),
    base04: Colour::rgb(205, 205, 205),
    base05: Colour::rgb(213, 213, 213),
    base06: Colour::rgb(221, 221, 221),
    base07: Colour::rgb(229, 229, 229),
    base08: Colour::rgb(215, 38, 56),
    base09: Colour::rgb(235, 132, 19),
    base0a: Colour::rgb(241, 157, 26),
    base0b: Colour::rgb(136, 185, 45),
    base0c: Colour::rgb(27, 165, 149),
    base0d: Colour::rgb(30, 139, 172),
    base0e: Colour::rgb(190, 66, 100),
    base0f: Colour::rgb(200, 94, 13),
};

pub const ICY: Base16Theme = Base16Theme {
    base00: Colour::rgb(2, 16, 18),
    base01: Colour::rgb(3, 22, 25),
    base02: Colour::rgb(4, 31, 35),
    base03: Colour::rgb(5, 46, 52),
    base04: Colour::rgb(6, 64, 72),
    base05: Colour::rgb(9, 91, 103),
    base06: Colour::rgb(12, 124, 140),
    base07: Colour::rgb(16, 156, 176),
    base08: Colour::rgb(22, 193, 217),
    base09: Colour::rgb(179, 235, 242),
    base0a: Colour::rgb(128, 222, 234),
    base0b: Colour::rgb(77, 208, 225),
    base0c: Colour::rgb(38, 198, 218),
    base0d: Colour::rgb(0, 188, 212),
    base0e: Colour::rgb(0, 172, 193),
    base0f: Colour::rgb(0, 151, 167),
};

pub const WOODLAND: Base16Theme = Base16Theme {
    base00: Colour::rgb(35, 30, 24),
    base01: Colour::rgb(48, 43, 37),
    base02: Colour::rgb(72, 65, 58),
    base03: Colour::rgb(157, 139, 112),
    base04: Colour::rgb(180, 164, 144),
    base05: Colour::rgb(202, 188, 177),
    base06: Colour::rgb(215, 200, 188),
    base07: Colour::rgb(228, 212, 200),
    base08: Colour::rgb(211, 92, 92),
    base09: Colour::rgb(202, 127, 50),
    base0a: Colour::rgb(224, 172, 22),
    base0b: Colour::rgb(183, 186, 83),
    base0c: Colour::rgb(110, 185, 88),
    base0d: Colour::rgb(136, 164, 211),
    base0e: Colour::rgb(187, 144, 226),
    base0f: Colour::rgb(180, 147, 104),
};

pub const XCODE_DUSK: Base16Theme = Base16Theme {
    base00: Colour::rgb(40, 43, 53),
    base01: Colour::rgb(61, 64, 72),
    base02: Colour::rgb(83, 85, 93),
    base03: Colour::rgb(104, 106, 113),
    base04: Colour::rgb(126, 128, 134),
    base05: Colour::rgb(147, 149, 153),
    base06: Colour::rgb(169, 170, 174),
    base07: Colour::rgb(190, 191, 194),
    base08: Colour::rgb(178, 24, 137),
    base09: Colour::rgb(120, 109, 197),
    base0a: Colour::rgb(67, 130, 136),
    base0b: Colour::rgb(223, 0, 2),
    base0c: Colour::rgb(0, 160, 190),
    base0d: Colour::rgb(121, 14, 173),
    base0e: Colour::rgb(178, 24, 137),
    base0f: Colour::rgb(199, 124, 72),
};

pub const ZENBURN: Base16Theme = Base16Theme {
    base00: Colour::rgb(56, 56, 56),
    base01: Colour::rgb(64, 64, 64),
    base02: Colour::rgb(96, 96, 96),
    base03: Colour::rgb(111, 111, 111),
    base04: Colour::rgb(128, 128, 128),
    base05: Colour::rgb(220, 220, 204),
    base06: Colour::rgb(192, 192, 192),
    base07: Colour::rgb(255, 255, 255),
    base08: Colour::rgb(220, 163, 163),
    base09: Colour::rgb(223, 175, 143),
    base0a: Colour::rgb(224, 207, 159),
    base0b: Colour::rgb(95, 127, 95),
    base0c: Colour::rgb(147, 224, 227),
    base0d: Colour::rgb(124, 184, 187),
    base0e: Colour::rgb(220, 140, 195),
    base0f: Colour::rgb(147, 224, 227),
};

pub const VSCODE_DARK: Base16Theme = Base16Theme {
    base00: Colour::rgb(0x1E, 0x1E, 0x1E),
    base01: Colour::rgb(0x3A, 0x3D, 0x41),
    base02: Colour::rgb(0x26, 0x4F, 0x78),
    base03: Colour::rgb(0x6A, 0x99, 0x55),
    base04: Colour::rgb(0x00, 0x7A, 0xCC),
    base05: Colour::rgb(0x9C, 0xDC, 0xFE),
    base06: Colour::rgb(0xFF, 0xFF, 0xFF),

    // Currently unused.
    base07: Colour::rgb(0x00, 0x00, 0x00),

    base08: Colour::rgb(0x56, 0x9C, 0xD6),
    base09: Colour::rgb(0xD2, 0x1A, 0x25),
    base0a: Colour::rgb(0x4E, 0xC9, 0xB0),
    base0b: Colour::rgb(0xB5, 0xCE, 0xA8),
    base0c: Colour::rgb(0xCE, 0x91, 0x78),
    base0d: Colour::rgb(0xDC, 0xDC, 0xAA),
    base0e: Colour::rgb(0x4F, 0xC1, 0xFF),
    base0f: Colour::rgb(0xFF, 0xFF, 0xFF),
};
