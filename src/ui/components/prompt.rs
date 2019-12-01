use std::{env, io, mem, path::PathBuf};
use termion::event::Key;

use super::{Component, Context, Scheduler};
use crate::{
    error::{Error, Result},
    ui::{Screen, Style},
};

#[derive(Clone, Debug)]
pub struct Theme {
    pub base: Style,
}

pub enum Command {
    OpenFile(PathBuf),
}

pub struct Prompt {
    command_str: String,
    command: Option<Command>,
    active: bool,
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            command_str: String::new(),
            command: None,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn poll_and_clear(&mut self) -> Option<Command> {
        let mut command = None;
        mem::swap(&mut self.command, &mut command);
        command
    }

    pub fn log_error(&mut self, message: String) {
        self.command_str = message;
    }

    pub fn clear_log(&mut self) {
        if !self.active {
            self.command_str.clear()
        }
    }
}

impl Component for Prompt {
    #[inline]
    fn draw(&mut self, screen: &mut Screen, _: &mut Scheduler, context: &Context) {
        let theme = &context.theme.prompt;
        screen.clear_region(context.frame, theme.base);
        let prefix = (if self.active { "open " } else { "" }).to_string();
        screen.draw_str(
            context.frame.origin.x,
            context.frame.origin.y,
            theme.base,
            &prefix,
        );
        screen.draw_str(
            context.frame.origin.x + prefix.len(),
            context.frame.origin.y,
            theme.base,
            &self.command_str,
        );
    }

    #[inline]
    fn handle_event(&mut self, key: Key, _: &mut Scheduler, _: &Context) -> Result<()> {
        match key {
            Key::Ctrl('g') => {
                self.active = false;
                self.command_str.clear();
            }
            Key::Alt('x') => {
                self.active = true;
                self.command_str.clear();
                self.command_str
                    .push_str(env::current_dir()?.to_str().unwrap_or(""));
                self.command_str.push_str("/");
            }
            Key::Char('\n') if self.active => {
                self.command = Some(Command::OpenFile(PathBuf::from(&self.command_str)));
                self.command_str.clear();
                self.active = false;
            }
            Key::Backspace => {
                self.command_str.pop();
            }
            Key::Char(character) if self.active => {
                self.command_str.push(character);
            }
            _ => {}
        };

        Ok(())
    }
}
