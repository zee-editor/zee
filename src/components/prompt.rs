use euclid::default::SideOffsets2D;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ropey::Rope;
use std::{
    borrow::Cow,
    cmp, env, fs, mem,
    path::{Path, PathBuf},
};

use super::{Component, Context, Cursor, Position, Rect, Scheduler, Size};
use crate::{
    error::{Error, Result},
    terminal::{Background, Foreground, Key, Screen, Style},
    utils::{self, RopeGraphemes},
};

#[derive(Clone, Debug)]
pub struct Theme {
    pub action: Style,
    pub input: Style,
    pub cursor: Style,
    pub item_focused_background: Background,
    pub item_unfocused_background: Background,
    pub item_file_foreground: Foreground,
    pub item_directory_foreground: Foreground,
}

pub enum Command {
    OpenFile(PathBuf),
}

pub struct Prompt {
    input: Rope,
    cursor: Cursor,
    command: Option<Command>,
    active: bool,
    file_picker: FilePicker,
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            input: Rope::new(),
            cursor: Cursor::new(),
            command: None,
            active: false,
            file_picker: FilePicker::new(),
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
        self.input = Rope::from(message);
    }

    pub fn clear_log(&mut self) {
        if !self.active {
            self.input.remove(0..self.input.len_chars());
        }
    }

    pub fn height(&self) -> usize {
        if self.active {
            PROMPT_INPUT_HEIGHT + PROMPT_SELECT_HEIGHT
        } else {
            PROMPT_INPUT_HEIGHT
        }
    }

    fn read_dir(&mut self) -> Result<()> {
        let path_str: String = self.input.slice(..).into();
        self.file_picker.reset(
            fs::read_dir(
                Path::new(&path_str)
                    .parent()
                    .unwrap_or_else(|| Path::new(&path_str)),
            )?
            .map(|entry| {
                entry
                    .map(|entry| entry.path().to_path_buf())
                    .map_err(|error| Error::Io(error))
            })
            .collect::<Result<Vec<PathBuf>>>()?
            .into_iter(),
            &path_str,
        );
        Ok(())
    }
}

impl Component for Prompt {
    #[inline]
    fn draw(&mut self, screen: &mut Screen, _: &mut Scheduler, context: &Context) {
        let theme = &context.theme.prompt;

        if self.active {
            self.file_picker.draw(
                screen,
                &context.set_frame(context.frame.inner_rect(SideOffsets2D::new(
                    0,
                    0,
                    PROMPT_INPUT_HEIGHT,
                    0,
                ))),
            );
        }

        screen.clear_region(
            context.frame.inner_rect(SideOffsets2D::new(
                self.height() - PROMPT_INPUT_HEIGHT,
                0,
                0,
                0,
            )),
            theme.input,
        );

        // Draw prompt
        let prefix = (if self.active { "open" } else { "" }).to_string();
        screen.draw_str(
            context.frame.origin.x,
            context.frame.origin.y + self.height() - 1,
            theme.action,
            &prefix,
        );

        let mut char_index = 0;
        let mut screen_x = context.frame.origin.x + prefix.len() + 1;
        let screen_y = context.frame.origin.y + self.height() - 1;
        for grapheme in RopeGraphemes::new(&self.input.slice(..)) {
            let style = if self.active && self.cursor.range.contains(&char_index) {
                theme.cursor
            } else {
                theme.input
            };
            let grapheme_width = utils::grapheme_width(&grapheme);

            if grapheme_width == 0 {
                screen.draw_str(screen_x, screen_y, style, " ");
            } else {
                screen.draw_rope_slice(screen_x, screen_y, style, &grapheme);
            }

            char_index += grapheme.len_chars();
            screen_x += grapheme_width;
        }
    }

    #[inline]
    fn handle_event(&mut self, key: Key, _: &mut Scheduler, _: &Context) -> Result<()> {
        match key {
            Key::Ctrl('g') => {
                self.active = false;
                self.cursor = Cursor::new();
                self.input.remove(..);
                return Ok(());
            }
            Key::Alt('x') => {
                self.active = true;
                self.input.remove(..);
                self.input
                    .insert(0, env::current_dir()?.to_str().unwrap_or(""));
                self.input.insert_char(self.input.len_chars(), '/');
                self.input.insert_char(self.input.len_chars(), '\n');
                self.cursor.move_to_end_of_line(&self.input);
                if let Err(_) = self.read_dir() {}
                return Ok(());
            }
            Key::Char('\n') if self.active => {
                let path_str: Cow<str> = self.input.slice(..).into();
                self.command = Some(Command::OpenFile(PathBuf::from(path_str.trim())));
                self.input.remove(..);
                self.cursor = Cursor::new();
                self.active = false;
            }
            _ => {}
        }

        if self.active {
            let input_changed = match key {
                Key::Ctrl('b') | Key::Left => {
                    self.cursor.move_left(&self.input);
                    false
                }
                Key::Ctrl('f') | Key::Right => {
                    self.cursor.move_right(&self.input);
                    false
                }
                Key::Ctrl('a') | Key::Home => {
                    self.cursor.move_to_start_of_line(&self.input);
                    false
                }
                Key::Ctrl('e') | Key::End => {
                    self.cursor.move_to_end_of_line(&self.input);
                    false
                }
                Key::Ctrl('l') => {
                    let path_str: String = self.input.slice(..).into();
                    self.input = Path::new(&path_str.trim())
                        .parent()
                        .map(|parent| parent.to_string_lossy())
                        .unwrap_or_else(|| "".into())
                        .into();
                    utils::ensure_trailing_newline_with_content(&mut self.input);
                    self.cursor.move_to_end_of_line(&self.input);
                    self.cursor.insert_char(&mut self.input, '/');
                    self.cursor.move_right(&self.input);
                    true
                }
                Key::Char('\t') => {
                    if let Some(path) = self.file_picker.selected() {
                        self.input = path.to_string_lossy().into();
                        utils::ensure_trailing_newline_with_content(&mut self.input);
                        self.cursor.move_to_end_of_line(&self.input);
                        if path.is_dir() {
                            self.cursor.insert_char(&mut self.input, '/');
                            self.cursor.move_right(&self.input);
                        }
                        true
                    } else {
                        false
                    }
                }
                Key::Ctrl('n') | Key::Up => {
                    self.file_picker.move_up();
                    false
                }
                Key::Ctrl('p') | Key::Down => {
                    self.file_picker.move_down();
                    false
                }
                Key::Alt('<') => {
                    self.file_picker.move_to_top();
                    false
                }
                Key::Alt('>') => {
                    self.file_picker.move_to_bottom();
                    false
                }
                Key::Backspace => !self.cursor.backspace(&mut self.input).is_empty(),
                Key::Ctrl('d') => !self.cursor.delete(&mut self.input).is_empty(),
                Key::Char(character) if character != '\t' => {
                    let diff = self.cursor.insert_char(&mut self.input, character);
                    self.cursor.move_right(&self.input);
                    !diff.is_empty()
                }
                _ => false,
            };

            if input_changed {
                if let Err(_) = self.read_dir() {}
            }
        }

        Ok(())
    }
}

struct FilePicker {
    offset: usize,
    selected: usize,
    paths: Vec<PathBuf>,
    filtered: Vec<(usize, i64)>, // (index, score)
    matcher: SkimMatcherV2,
}

impl FilePicker {
    fn new() -> Self {
        Self {
            offset: 0,
            selected: 0,
            paths: Vec::new(),
            filtered: Vec::new(),
            matcher: Default::default(),
        }
    }

    fn reset(&mut self, paths_iter: impl Iterator<Item = PathBuf>, filter: &str) {
        let Self {
            ref mut offset,
            ref mut selected,
            ref mut paths,
            ref mut filtered,
            ref mut matcher,
        } = *self;

        *offset = 0;
        *selected = 0;
        paths.clear();
        paths.extend(paths_iter);
        filtered.clear();
        filtered.extend(paths.iter().enumerate().filter_map(|(index, file)| {
            matcher
                .fuzzy_match(&file.to_string_lossy(), filter.trim())
                .map(|score| (index, score))
        }));
        &mut filtered.sort_unstable_by_key(|(_, score)| -score);
    }

    fn move_up(&mut self) {
        self.selected = cmp::min(self.selected + 1, self.filtered.len().saturating_sub(1));
    }

    fn move_down(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_to_top(&mut self) {
        self.selected = 0;
    }

    fn move_to_bottom(&mut self) {
        self.selected = self.filtered.len().saturating_sub(1);
    }

    fn selected(&self) -> Option<&Path> {
        if self.filtered.len() > 0 {
            Some(&self.paths[self.filtered[self.selected].0])
        } else {
            None
        }
    }

    fn draw(&mut self, screen: &mut Screen, context: &Context) {
        let theme = &context.theme.prompt;
        let height = context.frame.size.height;
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected - self.offset > height.saturating_sub(1) {
            self.offset = self.selected - height + 1;
        }

        screen.clear_region(
            context.frame,
            Style::normal(theme.item_unfocused_background, theme.item_file_foreground),
        );

        for (screen_index, path) in self
            .filtered
            .iter()
            .skip(self.offset)
            .take(height)
            .map(|(path_index, _)| &self.paths[*path_index])
            .enumerate()
        {
            let background = if self.offset + screen_index == self.selected {
                screen.clear_region(
                    Rect::new(
                        Position::new(
                            context.frame.origin.x,
                            context.frame.origin.y + screen_index,
                        ),
                        Size::new(context.frame.size.width, 1),
                    ),
                    Style::normal(theme.item_focused_background, theme.item_file_foreground),
                );
                theme.item_focused_background
            } else {
                theme.item_unfocused_background
            };
            let style = if path.is_dir() {
                Style::bold(background, theme.item_directory_foreground)
            } else {
                Style::normal(background, theme.item_file_foreground)
            };
            screen.draw_str(
                context.frame.origin.x,
                context.frame.origin.y + screen_index,
                style,
                &path
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_else(|| path.to_string_lossy()),
            );
        }
    }
}

const PROMPT_INPUT_HEIGHT: usize = 1;
const PROMPT_SELECT_HEIGHT: usize = 11;
