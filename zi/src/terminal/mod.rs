//! An abstract specification of a lightweight terminal.
//!
//! All components in Zi ultimately draw to a `Canvas`. Typically this is done
//! via their child components and their descendants. At the bottom of the
//! component hierarchy, low level components would draw directly on a canvas.

pub use canvas::{Background, Canvas, Colour, Foreground, SquarePixelGrid, Style};
pub use input::Key;

/// A 2D rectangle with usize coordinates. Re-exported from
/// [euclid](https://docs.rs/euclid).
pub type Rect = euclid::default::Rect<usize>;

/// A 2D position with usize coordinates. Re-exported from
/// [euclid](https://docs.rs/euclid).
pub type Position = euclid::default::Point2D<usize>;

/// A 2D size with usize width and height. Re-exported from
/// [euclid](https://docs.rs/euclid).
pub type Size = euclid::default::Size2D<usize>;

pub(crate) mod canvas;
pub(crate) mod input;
