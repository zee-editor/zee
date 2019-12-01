pub mod input;
pub mod screen;

pub use input::Input;
pub use screen::{Background, Colour, Foreground, Screen, Style};

pub type Rect = euclid::default::Rect<usize>;
pub type Position = euclid::default::Point2D<usize>;
pub type Size = euclid::default::Size2D<usize>;
