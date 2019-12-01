use crossbeam_channel::{self, Receiver};
use std::{
    io::Read,
    thread::{self, JoinHandle},
};
use termion::{event::Key, input::TermRead};

pub struct Input {
    pub receiver: Receiver<Key>,
    handle: JoinHandle<()>,
}

impl Input {
    pub fn from_reader(reader: impl Read + Send + 'static) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(2048);
        let handle = thread::spawn(move || {
            let mut keys = reader.keys();
            while let Some(event) = keys.next() {
                match event {
                    Ok(key) => {
                        sender.send(key).unwrap();
                    }
                    error => {
                        error.unwrap();
                    }
                }
            }
        });
        Self { receiver, handle }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        // ??
    }
}
