use smallstr::SmallString;
use std::{self, cmp, iter};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{Position, Size};
use crate::terminal::Rect;

/// An extended grapheme cluster represented as a `SmallString`.
pub type GraphemeCluster = SmallString<[u8; 16]>;

/// A "text element", which consists of an extended grapheme cluster and
/// associated styling.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Textel {
    pub grapheme: GraphemeCluster,
    pub style: Style,
}

/// A lightweight abstract terminal. All components in Zi ultimately draw to a
/// `Canvas`, typically via their child components or directly in the case of
/// lower level components.
#[derive(Debug, Clone)]
pub struct Canvas {
    buffer: Vec<Option<Textel>>,
    size: Size,
    min_size: Size,
}

impl Canvas {
    /// A lightweight abstract terminal. All components in Zi ultimately draw to a
    /// `Canvas`, typically via their child components or directly in the case of
    /// lower level components.
    ///
    /// ```
    /// # use zi::{Canvas, Size};
    /// let canvas = Canvas::new(Size::new(10, 20));
    /// ```
    pub fn new(size: Size) -> Self {
        Self {
            buffer: iter::repeat(Textel::default())
                .map(Some)
                .take(size.area())
                .collect(),
            size,
            min_size: Size::zero(),
        }
    }

    #[inline]
    pub fn size(&self) -> Size {
        self.size
    }

    #[inline]
    pub fn min_size(&self) -> Size {
        self.min_size
    }

    #[inline]
    pub fn buffer(&self) -> &[Option<Textel>] {
        self.buffer.as_slice()
    }

    #[inline]
    pub fn buffer_mut(&mut self) -> &mut [Option<Textel>] {
        self.buffer.as_mut_slice()
    }

    #[inline]
    pub fn resize(&mut self, size: Size) {
        self.buffer.resize(size.area(), Default::default());
        self.size = size;
        self.min_size = size.min(self.min_size);
    }

    #[inline]
    pub fn clear_region(&mut self, region: Rect, style: Style) {
        let y_range =
            region.origin.y..cmp::min(region.origin.y + region.size.height, self.size.height);
        let x_range =
            region.origin.x..cmp::min(region.origin.x + region.size.width, self.size.width);
        for y in y_range {
            self.buffer[y * self.size.width + x_range.start..y * self.size.width + x_range.end]
                .iter_mut()
                .for_each(|textel| clear_textel(textel, style, " "));
        }
    }

    #[inline]
    pub fn clear(&mut self, style: Style) {
        self.buffer
            .iter_mut()
            .for_each(|textel| clear_textel(textel, style, " "))
    }

    #[inline]
    pub fn clear_with(&mut self, style: Style, content: &str) {
        self.buffer
            .iter_mut()
            .for_each(|textel| clear_textel(textel, style, content))
    }

    #[inline]
    pub fn draw_str(&mut self, x: usize, y: usize, style: Style, text: &str) -> usize {
        self.draw_graphemes(x, y, style, UnicodeSegmentation::graphemes(text, true))
    }

    #[inline]
    pub fn draw_graphemes(
        &mut self,
        x: usize,
        y: usize,
        style: Style,
        graphemes: impl Iterator<Item = impl Into<GraphemeCluster>>,
    ) -> usize {
        if y >= self.size.height || x >= self.size.width {
            return 0;
        }

        let initial_offset = y * self.size.width + x;
        let max_offset = cmp::min((y + 1) * self.size.width + x, self.buffer.len());
        let mut current_offset = initial_offset;

        for grapheme in graphemes {
            if current_offset >= max_offset {
                break;
            }

            let grapheme = grapheme.into();
            let grapheme_width = UnicodeWidthStr::width(grapheme.as_ref());
            if grapheme_width == 0 {
                continue;
            }

            self.buffer[current_offset] = Some(Textel { style, grapheme });

            let num_modified = cmp::min(grapheme_width, max_offset - current_offset);
            self.buffer[current_offset + 1..current_offset + num_modified]
                .iter_mut()
                .for_each(|textel| *textel = None);

            current_offset += num_modified;
        }

        // Update `min_size`
        self.min_size.width = cmp::max(self.min_size.width, current_offset);
        self.min_size.height = cmp::max(self.min_size.height, y);

        current_offset - initial_offset
    }

    #[inline]
    pub fn copy_region(&mut self, source: &Self, region: Rect) {
        let y_range = cmp::min(region.origin.y, self.size.height)
            ..cmp::min(region.origin.y + source.size.height, self.size.height);
        let x_range = cmp::min(region.origin.x, self.size.width)
            ..cmp::min(region.origin.x + source.size.width, self.size.width);

        for y in y_range {
            self.buffer[y * self.size.width + x_range.start..y * self.size.width + x_range.end]
                .iter_mut()
                .zip(
                    source.buffer[(y - region.origin.y) * source.size.width
                        ..(y - region.origin.y) * source.size.width
                            + (x_range.end - region.origin.x)]
                        .iter(),
                )
                .for_each(|(textel, other)| *textel = other.clone());
        }
    }

    #[inline]
    pub fn textel(&self, x: usize, y: usize) -> &Option<Textel> {
        &self.buffer[y * self.size.width + x]
    }

    #[inline]
    pub fn textel_mut(&mut self, x: usize, y: usize) -> &mut Option<Textel> {
        &mut self.buffer[y * self.size.width + x]
    }
}

/// Specifies how content should be styled. This represents a subset of the ANSI
/// available styles which is widely supported by terminal emulators.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Style {
    pub background: Background,
    pub foreground: Foreground,
    pub bold: bool,
    pub underline: bool,
}

impl Style {
    /// Default style, white on black. This function exists as
    /// `Default::default()` is not const.
    pub const fn default() -> Self {
        Style::normal(Colour::black(), Colour::white())
    }

    #[inline]
    pub const fn normal(background: Background, foreground: Foreground) -> Self {
        Self {
            background,
            foreground,
            bold: false,
            underline: false,
        }
    }

    #[inline]
    pub const fn bold(background: Background, foreground: Foreground) -> Self {
        Self {
            background,
            foreground,
            bold: true,
            underline: false,
        }
    }

    #[inline]
    pub const fn underline(background: Background, foreground: Foreground) -> Self {
        Self {
            background,
            foreground,
            bold: false,
            underline: true,
        }
    }

    #[inline]
    pub const fn same_colour(colour: Colour) -> Self {
        Self {
            background: colour,
            foreground: colour,
            bold: false,
            underline: false,
        }
    }
}

impl Default for Style {
    #[inline]
    fn default() -> Self {
        Self::default()
    }
}

/// An RGB encoded colour, 1-byte per channel.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Colour {
    /// Creates a colour from the provided RGB values.
    #[inline]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// Returns black.
    #[inline]
    pub const fn black() -> Self {
        Self {
            red: 0,
            green: 0,
            blue: 0,
        }
    }

    /// Returns white.
    #[inline]
    pub const fn white() -> Self {
        Self {
            red: 255,
            green: 255,
            blue: 255,
        }
    }
}

/// Type alias for background colours.
pub type Background = Colour;

/// Type alias for foreground colours.
pub type Foreground = Colour;

#[inline]
fn clear_textel(textel: &mut Option<Textel>, style: Style, value: &str) {
    match *textel {
        Some(Textel {
            style: ref mut textel_style,
            ref mut grapheme,
        }) => {
            *textel_style = style;
            grapheme.clear();
            grapheme.push_str(value);
        }
        _ => {
            *textel = Some(Textel {
                style,
                grapheme: " ".into(),
            });
        }
    }
}

/// Wraps a [`Canvas`](terminal/struct.Canvas.html) and exposes a grid of square
/// "pixels". The size of the grid `(2 * height, width)` of the dimensions of the
/// wrapped canvas. This is implemented using Unicode's upper half block
/// character.
pub struct SquarePixelGrid {
    canvas: Canvas,
}

impl SquarePixelGrid {
    pub fn new(size: Size) -> Self {
        assert!(size.height % 2 == 0);
        let mut canvas = Canvas::new(Size::new(size.width, size.height / 2));
        canvas.clear_with(Default::default(), UPPER_HALF_BLOCK);
        Self { canvas }
    }

    pub fn from_available(size: Size) -> Self {
        let mut canvas = Canvas::new(Size::new(size.width, size.height));
        canvas.clear_with(Default::default(), UPPER_HALF_BLOCK);
        Self { canvas }
    }

    /// Returns the size of this square pixel grid. The grid has the same width
    /// as the wrapped canvas and twice the height.
    #[inline]
    pub fn size(&self) -> Size {
        let canvas_size = self.canvas.size();
        Size::new(canvas_size.width, canvas_size.height * 2)
    }

    #[inline]
    pub fn draw(&mut self, position: Position, colour: Colour) {
        let textel = self
            .canvas
            .textel_mut(position.x, position.y / 2)
            .as_mut()
            .expect("No textels should be uninitialised");
        if position.y % 2 == 0 {
            textel.style.foreground = colour;
        } else {
            textel.style.background = colour;
        }
    }

    #[inline]
    pub fn into_canvas(self) -> Canvas {
        self.canvas
    }
}

const UPPER_HALF_BLOCK: &str = "▀";

#[cfg(test)]
mod tests {
    use super::{GraphemeCluster, Style, Textel};

    #[test]
    fn size_of_style() {
        eprintln!(
            "std::mem::size_of::<Style>() == {}",
            std::mem::size_of::<Style>()
        );
        eprintln!(
            "std::mem::size_of::<Option<Textel>>() == {}",
            std::mem::size_of::<Option<Textel>>()
        );
        eprintln!(
            "std::mem::size_of::<GraphemeCluster>>() == {}",
            std::mem::size_of::<Option<GraphemeCluster>>()
        );
    }
}
