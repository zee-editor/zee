pub mod canvas;
pub mod input;

pub use canvas::{Background, Canvas, Colour, Foreground, SquarePixelGrid, Style};
pub use input::Key;

pub type Rect = euclid::default::Rect<usize>;
pub type Position = euclid::default::Point2D<usize>;
pub type Size = euclid::default::Size2D<usize>;
