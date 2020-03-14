use euclid::default::SideOffsets2D;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ignore::WalkBuilder;
use lazy_static::lazy_static;
use maplit::hashmap;
use ropey::Rope;
use smallvec::smallvec;
use std::{
    borrow::Cow,
    cmp, fs, iter, mem,
    path::{Path, PathBuf},
};

use super::{
    cursor::{CharIndex, Cursor},
    BindingMatch, Bindings, Component, Context, HashBindings, Position, Rect, Size, TaskDone,
};
use crate::{
    error::{Error, Result},
    task::{self, TaskId},
    terminal::{Background, Foreground, Key, Screen, Style},
    utils::{self, RopeGraphemes},
};

type Scheduler<'pool> = task::Scheduler<'pool, Result<PromptTask>>;

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

pub struct PromptTask {
    file_picker: FilePicker,
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Inactive,
    PickingFileFromRepo,
    PickingFileFromDirectory,
}

impl State {
    fn is_active(&self) -> bool {
        *self != Self::Inactive
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    // File pickers
    Clear,
    PickFileFromRepo,
    PickFileFromDirectory,
    OpenFile,

    // Cursor movement
    CursorLeft,
    CursorRight,
    CursorStartOfLine,
    CursorEndOfLine,

    // Path navigation
    SelectParentDirectory,
    AutocompletePath,
    DeleteForward,
    DeleteBackward,
    InsertChar(char),

    // Selection
    SelectUp,
    SelectDown,
    SelectFirst,
    SelectLast,
}

lazy_static! {
    pub static ref HASH_BINDINGS: HashBindings<Action> = HashBindings::new(hashmap! {
        // Path navigation
        smallvec![Key::Ctrl('g')] => Action::Clear,
        smallvec![Key::Ctrl('x'), Key::Ctrl('f')] => Action::PickFileFromDirectory,
        smallvec![Key::Ctrl('x'), Key::Ctrl('v')] => Action::PickFileFromRepo,
        smallvec![Key::Char('\n')] => Action::OpenFile,

        // Cursor movement
        smallvec![Key::Ctrl('b')] => Action::CursorLeft,
        smallvec![Key::Left] => Action::CursorLeft,
        smallvec![Key::Ctrl('f')] => Action::CursorRight,
        smallvec![Key::Right] => Action::CursorRight,
        smallvec![Key::Ctrl('a')] => Action::CursorStartOfLine,
        smallvec![Key::Home] => Action::CursorStartOfLine,
        smallvec![Key::Ctrl('e')] => Action::CursorEndOfLine,
        smallvec![Key::End] => Action::CursorEndOfLine,

        // Path navigation
        smallvec![Key::Ctrl('l')] => Action::SelectParentDirectory,
        smallvec![Key::Char('\t')] => Action::AutocompletePath,
        smallvec![Key::Ctrl('d')] => Action::DeleteForward,
        smallvec![Key::Backspace] => Action::DeleteBackward,

        // Selection
        smallvec![Key::Ctrl('p')] => Action::SelectUp,
        smallvec![Key::Up] => Action::SelectUp,
        smallvec![Key::Ctrl('n')] => Action::SelectDown,
        smallvec![Key::Down] => Action::SelectDown,
        smallvec![Key::Char('p')] => Action::SelectUp,
        smallvec![Key::Alt('<')] => Action::SelectFirst,
        smallvec![Key::Alt('>')] => Action::SelectLast,
    });
}

pub struct PromptBindings;

impl Bindings<Action> for PromptBindings {
    fn matches(&self, pressed: &[Key]) -> BindingMatch<Action> {
        match pressed {
            [Key::Char(character)] if *character != '\n' && *character != '\t' => {
                BindingMatch::Full(Action::InsertChar(*character))
            }
            pressed => HASH_BINDINGS.matches(pressed),
        }
    }
}

pub struct Prompt {
    input: Rope,
    cursor: Cursor,
    command: Option<Command>,
    state: State,
    file_picker: FilePicker,
    file_picker_task: Option<TaskId>,
    bindings: PromptBindings,
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            input: Rope::new(),
            cursor: Cursor::new(),
            command: None,
            state: State::Inactive,
            file_picker: FilePicker::new(),
            file_picker_task: None,
            bindings: PromptBindings,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    pub fn poll_and_clear(&mut self) -> Option<Command> {
        let mut command = None;
        mem::swap(&mut self.command, &mut command);
        command
    }

    pub fn log_error(&mut self, message: String) {
        if !self.is_active() {
            self.input = Rope::from(message);
        }
    }

    pub fn clear_log(&mut self) {
        if !self.is_active() {
            self.input.remove(0..self.input.len_chars());
        }
    }

    pub fn height(&self) -> usize {
        if self.is_active() {
            PROMPT_INPUT_HEIGHT + cmp::min(self.file_picker.filtered.len(), PROMPT_SELECT_HEIGHT)
        } else {
            PROMPT_INPUT_HEIGHT
        }
    }

    fn pick_from_directory(&mut self, scheduler: &mut Scheduler) -> Result<()> {
        let path_str = self.input.to_string();
        let mut file_picker = self.file_picker.clone();
        self.file_picker_task = Some(scheduler.spawn(move || {
            pick_from_directory(&mut file_picker, path_str)?;
            Ok(PromptTask { file_picker })
        })?);
        Ok(())
    }

    fn pick_from_repository(&mut self, scheduler: &mut Scheduler) -> Result<()> {
        let path_str = self.input.to_string();
        let mut file_picker = self.file_picker.clone();
        self.file_picker_task = Some(scheduler.spawn(move || {
            pick_from_repository(&mut file_picker, path_str)?;
            Ok(PromptTask { file_picker })
        })?);
        Ok(())
    }

    #[inline]
    fn set_input_to_cwd(&mut self, context: &Context) {
        self.cursor.delete_line(&mut self.input);
        self.cursor.insert_chars(
            &mut self.input,
            context
                .path
                .parent()
                .unwrap_or(context.path)
                .to_str()
                .unwrap_or("")
                .chars(),
        );
        self.cursor.move_to_end_of_line(&self.input);
        self.cursor.insert_char(&mut self.input, '/');
        self.cursor.move_right(&self.input);
    }
}

fn repository_files_iter(path: impl AsRef<Path>) -> impl Iterator<Item = Result<PathBuf>> {
    WalkBuilder::new(path.as_ref().parent().unwrap_or_else(|| path.as_ref()))
        .build()
        .filter_map(|entry| {
            let is_dir = entry
                .as_ref()
                .map(|entry| entry.path().is_dir())
                .unwrap_or(false);
            if entry.is_ok() && !is_dir {
                Some(
                    entry
                        .map(|entry| entry.path().to_path_buf())
                        .map_err(Error::FilePicker),
                )
            } else {
                None
            }
        })
}

fn directory_files_iter(path: impl AsRef<Path>) -> Result<impl Iterator<Item = Result<PathBuf>>> {
    fs::read_dir(path.as_ref().parent().unwrap_or_else(|| path.as_ref()))
        .map(|walk| {
            walk.map(|entry| {
                entry
                    .map(|entry| entry.path().to_path_buf())
                    .map_err(Error::Io)
            })
        })
        .map_err(Error::Io)
}

impl Component for Prompt {
    type Action = Action;
    type Bindings = PromptBindings;
    type TaskPayload = Result<PromptTask>;

    #[inline]
    fn draw(&mut self, screen: &mut Screen, _: &mut Scheduler, context: &Context) {
        let theme = &context.theme.prompt;

        if self.is_active() {
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

        assert!(self.height() >= PROMPT_INPUT_HEIGHT);
        screen.clear_region(
            context.frame.inner_rect(SideOffsets2D::new(
                context
                    .frame
                    .size
                    .height
                    .saturating_sub(PROMPT_INPUT_HEIGHT),
                0,
                0,
                0,
            )),
            theme.input,
        );

        // Draw prompt
        let prefix = match (&self.state, self.file_picker_task.is_some()) {
            (State::PickingFileFromRepo, true) => "repo*",
            (State::PickingFileFromRepo, false) => "repo ",
            (State::PickingFileFromDirectory, true) => "open*",
            (State::PickingFileFromDirectory, false) => "open ",
            (State::Inactive, _) => "",
        };
        let prefix_offset = if prefix.is_empty() {
            0
        } else {
            prefix.len() + 1
        };

        screen.draw_str(
            context.frame.origin.x,
            context.frame.origin.y + self.height() - 1,
            theme.action,
            &prefix,
        );

        let mut char_index = CharIndex(0);
        let mut screen_x = context.frame.origin.x + prefix_offset;
        let screen_y = context.frame.origin.y + self.height() - 1;
        for grapheme in RopeGraphemes::new(&self.input.slice(..)) {
            let style = if self.is_active() && self.cursor.range().contains(&char_index) {
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

            char_index.0 += grapheme.len_chars();
            screen_x += grapheme_width;
        }
    }

    #[inline]
    fn handle_action(
        &mut self,
        action: Action,
        scheduler: &mut Scheduler,
        context: &Context,
    ) -> Result<()> {
        match action {
            Action::Clear => {
                self.state = State::Inactive;
                self.cursor = Cursor::new();
                self.input.remove(..);
                self.file_picker.clear();
                self.file_picker_task = None;
                return Ok(());
            }
            Action::PickFileFromDirectory if !self.is_active() => {
                self.state = State::PickingFileFromDirectory;
                self.set_input_to_cwd(context);
                self.pick_from_directory(scheduler)?;
                return Ok(());
            }
            Action::PickFileFromRepo if !self.is_active() => {
                self.state = State::PickingFileFromRepo;
                self.set_input_to_cwd(context);
                self.pick_from_repository(scheduler)?;
                return Ok(());
            }
            Action::OpenFile if self.is_active() => {
                let path_str: Cow<str> = self.input.slice(..).into();
                self.command = Some(Command::OpenFile(PathBuf::from(path_str.trim())));
                self.input.remove(..);
                self.cursor = Cursor::new();
                self.state = State::Inactive;
            }
            _ => {}
        }

        if self.is_active() {
            let input_changed = match action {
                Action::CursorLeft => {
                    self.cursor.move_left(&self.input);
                    false
                }
                Action::CursorRight => {
                    self.cursor.move_right(&self.input);
                    false
                }
                Action::CursorStartOfLine => {
                    self.cursor.move_to_start_of_line(&self.input);
                    false
                }
                Action::CursorEndOfLine => {
                    self.cursor.move_to_end_of_line(&self.input);
                    false
                }
                Action::SelectParentDirectory => {
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
                Action::AutocompletePath => {
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
                Action::SelectDown => {
                    self.file_picker.move_down();
                    false
                }
                Action::SelectUp => {
                    self.file_picker.move_up();
                    false
                }
                Action::SelectFirst => {
                    self.file_picker.move_to_top();
                    false
                }
                Action::SelectLast => {
                    self.file_picker.move_to_bottom();
                    false
                }
                Action::DeleteBackward => !self.cursor.backspace(&mut self.input).is_empty(),
                Action::DeleteForward => !self.cursor.delete(&mut self.input).is_empty(),
                Action::InsertChar(character) if character != '\t' => {
                    let diff = self.cursor.insert_char(&mut self.input, character);
                    self.cursor.move_right(&self.input);
                    !diff.is_empty()
                }
                _ => false,
            };

            if input_changed {
                match self.state {
                    State::PickingFileFromDirectory => self.pick_from_directory(scheduler)?,
                    State::PickingFileFromRepo => self.pick_from_repository(scheduler)?,
                    State::Inactive => {}
                }
            }
        }

        Ok(())
    }

    fn bindings(&self) -> Option<&Self::Bindings> {
        Some(&self.bindings)
    }

    fn task_done(&mut self, task: TaskDone<Self::TaskPayload>) -> Result<()> {
        let task_id = task.id;
        match task.payload {
            Ok(PromptTask { file_picker })
                if self
                    .file_picker_task
                    .map(|expected| expected == task_id)
                    .unwrap_or(false) =>
            {
                self.file_picker_task = None;
                self.file_picker = file_picker;
            }
            _ => {}
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
    prefix: PathBuf,
}

impl Clone for FilePicker {
    fn clone(&self) -> Self {
        Self {
            offset: self.offset,
            selected: self.selected,
            paths: self.paths.clone(),
            filtered: self.filtered.clone(),
            matcher: Default::default(),
            prefix: self.prefix.clone(),
        }
    }
}

impl FilePicker {
    fn new() -> Self {
        Self {
            offset: 0,
            selected: 0,
            paths: Vec::new(),
            filtered: Vec::new(),
            matcher: Default::default(),
            prefix: PathBuf::new(),
        }
    }

    fn prefix(&self) -> &Path {
        self.prefix.as_path()
    }

    fn set_filter(&mut self, filter: &str) {
        let Self {
            ref mut offset,
            ref mut selected,
            ref mut paths,
            ref mut filtered,
            ref mut matcher,
            ..
        } = *self;
        *offset = 0;
        *selected = 0;
        filtered.clear();
        filtered.extend(paths.iter().enumerate().filter_map(|(index, file)| {
            matcher
                .fuzzy_match(&file.to_string_lossy(), filter.trim())
                .map(|score| (index, score))
        }));
        filtered.sort_unstable_by_key(|(_, score)| -score);
    }

    fn clear(&mut self) {
        self.reset(iter::empty(), "", "")
    }

    fn reset(
        &mut self,
        paths_iter: impl Iterator<Item = PathBuf>,
        filter: &str,
        prefix_path: impl AsRef<Path>,
    ) {
        let Self {
            ref mut paths,
            ref mut prefix,
            ..
        } = *self;
        paths.clear();
        paths.extend(paths_iter);
        utils::clear_path_buf(prefix);
        prefix.push(prefix_path);
        self.set_filter(filter);
    }

    fn move_up(&mut self) {
        self.selected = cmp::min(self.selected + 1, self.filtered.len().saturating_sub(1));
    }

    fn move_down(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_to_top(&mut self) {
        self.selected = self.filtered.len().saturating_sub(1);
    }

    fn move_to_bottom(&mut self) {
        self.selected = 0;
    }

    fn selected(&self) -> Option<&Path> {
        if !self.filtered.is_empty() {
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

        for (option_index, path) in self
            .filtered
            .iter()
            .skip(self.offset)
            .take(height)
            .map(|(path_index, _)| &self.paths[*path_index])
            .enumerate()
        {
            let frame_y = context.frame.origin.y + height - option_index - 1;
            let background = if self.offset + option_index == self.selected {
                screen.clear_region(
                    Rect::new(
                        Position::new(context.frame.origin.x, frame_y),
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
                frame_y,
                style,
                &path.to_string_lossy()[self
                    .prefix
                    .to_str()
                    .map(|prefix| prefix.len() + 1)
                    .unwrap_or(0)..],
            );
        }
    }
}

fn update_file_picker<FilesIterT>(
    file_picker: &mut FilePicker,
    path_str: String,
    files_iter: impl FnOnce(String) -> Result<FilesIterT>,
) -> Result<()>
where
    FilesIterT: Iterator<Item = PathBuf>,
{
    let prefix = Path::new(&path_str).parent().unwrap();
    if file_picker.prefix() != prefix {
        file_picker.reset(
            files_iter(path_str.clone())?.take(MAX_FILES_IN_PICKER),
            &path_str,
            &prefix,
        );
    } else {
        file_picker.set_filter(&path_str)
    }
    Ok(())
}

fn pick_from_directory(file_picker: &mut FilePicker, path_str: String) -> Result<()> {
    update_file_picker(file_picker, path_str, |path| {
        Ok(directory_files_iter(path)?.filter_map(|result_path| result_path.ok()))
    })
}

fn pick_from_repository(file_picker: &mut FilePicker, path_str: String) -> Result<()> {
    update_file_picker(file_picker, path_str, |path| {
        Ok(repository_files_iter(path).filter_map(|result_path| result_path.ok()))
    })
}

const PROMPT_INPUT_HEIGHT: usize = 1;
const PROMPT_SELECT_HEIGHT: usize = 15;
const MAX_FILES_IN_PICKER: usize = 65536;
