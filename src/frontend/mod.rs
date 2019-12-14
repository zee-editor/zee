pub mod termion;

pub use self::termion::Termion;

use crate::{
    error::Result,
    terminal::{Key, Screen, Size},
};
use crossbeam_channel::Receiver;

pub trait Frontend {
    fn size(&self) -> Result<Size>;

    fn present(&mut self, screen: &Screen) -> Result<()>;

    fn events(&self) -> &Receiver<Key>;
}
