pub mod line_info;
pub mod status_bar;
pub mod textarea;

use git2::Repository;
use ropey::Rope;
use std::{borrow::Cow, fs::File, io::BufWriter, iter, path::PathBuf, rc::Rc};
use zi::{
    components::text::{Text, TextAlign, TextProperties},
    prelude::*,
    Callback,
};

use self::{
    line_info::{LineInfo, Properties as LineInfoProperties},
    status_bar::{Properties as StatusBarProperties, StatusBar, Theme as StatusBarTheme},
    textarea::{Properties as TextAreaProperties, TextArea},
};
use super::{
    cursor::Cursor,
    edit_tree_viewer::{
        EditTreeViewer, Properties as EditTreeViewerProperties, Theme as EditTreeViewerTheme,
    },
};
use crate::{
    editor::Context,
    error::Result,
    mode::Mode,
    syntax::{
        highlight::Theme as SyntaxTheme,
        parse::{OpaqueDiff, ParserPool, ParserStatus},
    },
    undo::EditTree,
    utils::{strip_trailing_whitespace, TAB_WIDTH},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub border: Style,
    pub edit_tree_viewer: EditTreeViewerTheme,
    pub status_bar: StatusBarTheme,
    pub syntax: SyntaxTheme,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModifiedStatus {
    Changed,
    Unchanged,
    Saving,
}

pub struct Buffer {
    properties: Properties,
    frame: Rect,
    link: ComponentLink<Self>,

    text: EditTree,
    has_unsaved_changes: ModifiedStatus,
    cursor: Cursor,
    line_offset: usize,
    parser: Option<ParserPool>,
    viewing_edit_tree: bool,
}

impl Buffer {
    pub fn spawn_save_file(&mut self) {
        self.has_unsaved_changes = ModifiedStatus::Saving;
        if let Some(ref file_path) = self.properties.file_path {
            let text = self.text.staged().clone();
            let file_path = file_path.clone();
            let link = self.link.clone();
            self.properties.context.task_pool.spawn(move |_| {
                link.send(Message::SaveFile(
                    File::create(&file_path)
                        .map(BufWriter::new)
                        .and_then(|writer| {
                            let text = strip_trailing_whitespace(text);
                            text.write_to(writer)?;
                            Ok(text)
                        })
                        .map_err(|error| error.into()),
                ))
            });
        }
    }

    #[inline]
    fn reduce(&mut self, message: Message) {
        // Stateless
        match message {
            Message::Up if !self.viewing_edit_tree => self.cursor.move_up(&self.text),
            Message::Down if !self.viewing_edit_tree => self.cursor.move_down(&self.text),
            Message::Left if !self.viewing_edit_tree => self.cursor.move_left(&self.text),
            Message::Right if !self.viewing_edit_tree => self.cursor.move_right(&self.text),
            Message::PageDown => self
                .cursor
                .move_down_n(&self.text, self.frame.size.height - 1),
            Message::PageUp => self
                .cursor
                .move_up_n(&self.text, self.frame.size.height - 1),
            Message::StartOfLine => self.cursor.move_to_start_of_line(&self.text),
            Message::EndOfLine => self.cursor.move_to_end_of_line(&self.text),
            Message::StartOfBuffer => self.cursor.move_to_start_of_buffer(&self.text),
            Message::EndOfBuffer => self.cursor.move_to_end_of_buffer(&self.text),
            Message::CenterCursorVisually => self.center_visual_cursor(),

            Message::BeginSelection => self.cursor.begin_selection(),
            Message::ClearSelection => {
                if self.viewing_edit_tree {
                    self.viewing_edit_tree = false;
                } else {
                    self.cursor.clear_selection();
                }
            }
            Message::SelectAll => self.cursor.select_all(&self.text),
            Message::SaveBuffer => self.spawn_save_file(),
            Message::Left if self.viewing_edit_tree => self.text.previous_child(),
            Message::Right if self.viewing_edit_tree => self.text.next_child(),
            Message::SaveFile(new_text) => {
                match new_text {
                    Ok(new_text) => {
                        self.cursor.sync(&self.text, &new_text);
                        self.text
                            .create_revision(OpaqueDiff::empty(), self.cursor.clone());
                        *self.text = new_text;
                        self.has_unsaved_changes = ModifiedStatus::Unchanged;
                    }
                    Err(error) => self.properties.log_message.emit(error.to_string()),
                }
                return;
            }
            Message::ParseSyntax(parsed) => {
                let parsed = parsed.unwrap();
                if let Some(parser) = self.parser.as_mut() {
                    parser.handle_parse_syntax_done(parsed);
                }
                return;
            }
            _ => {}
        };

        let mut undoing = false;
        let diff = match message {
            Message::DeleteForward => {
                let operation = self.cursor.delete(&mut self.text);
                // self.clipboard = Some(operation.deleted);
                operation.diff
            }
            Message::DeleteBackward => self.cursor.backspace(&mut self.text).diff,
            Message::DeleteLine => self.delete_line(),
            Message::Yank => self.paste_from_clipboard(),
            Message::CopySelection => self.copy_selection_to_clipboard(),
            Message::CutSelection => self.cut_selection_to_clipboard(),
            Message::InsertTab if DISABLE_TABS => {
                let diff = self
                    .cursor
                    .insert_chars(&mut self.text, iter::repeat(' ').take(TAB_WIDTH));
                self.cursor.move_right_n(&self.text, TAB_WIDTH);
                diff
            }
            Message::InsertTab if !DISABLE_TABS => {
                let diff = self
                    .cursor
                    .insert_chars(&mut self.text, iter::repeat(' ').take(TAB_WIDTH));
                self.cursor.move_right_n(&self.text, TAB_WIDTH);
                diff
            }
            Message::InsertNewLine => {
                let diff = self.cursor.insert_char(&mut self.text, '\n');
                // self.ensure_trailing_newline_with_content();
                self.cursor.move_down(&self.text);
                self.cursor.move_to_start_of_line(&self.text);
                diff
            }
            Message::ToggleEditTree => {
                self.viewing_edit_tree = !self.viewing_edit_tree;
                OpaqueDiff::empty()
            }
            Message::Undo => self
                .undo()
                .map(|diff| {
                    undoing = true;
                    diff
                })
                .unwrap_or_else(OpaqueDiff::empty),
            Message::Up if self.viewing_edit_tree => self
                .undo()
                .map(|diff| {
                    undoing = true;
                    diff
                })
                .unwrap_or_else(OpaqueDiff::empty),
            Message::Redo => self
                .redo()
                .map(|diff| {
                    undoing = true;
                    diff
                })
                .unwrap_or_else(OpaqueDiff::empty),
            Message::Down if self.viewing_edit_tree => self
                .redo()
                .map(|diff| {
                    undoing = true;
                    diff
                })
                .unwrap_or_else(OpaqueDiff::empty),
            Message::InsertChar(character) => {
                let diff = self.cursor.insert_char(&mut self.text, character);
                // self.ensure_trailing_newline_with_content();
                self.cursor.move_right(&self.text);
                diff
            }
            _ => OpaqueDiff::empty(),
        };

        if !diff.is_empty() && !undoing {
            self.has_unsaved_changes = ModifiedStatus::Changed;
            self.text.create_revision(diff.clone(), self.cursor.clone());
        }

        match self.parser.as_mut() {
            Some(parser) if !diff.is_empty() && !undoing => {
                parser.edit(&diff);
                parser.spawn(
                    &self.properties.context.task_pool,
                    self.text.staged().clone(),
                    false,
                );
            }
            _ => {}
        }
        self.ensure_cursor_in_view();
    }

    #[inline]
    fn ensure_cursor_in_view(&mut self) {
        let current_line = self.text.char_to_line(self.cursor.range().start.0);
        let num_lines = self.frame.size.height.saturating_sub(1);
        if current_line < self.line_offset {
            self.line_offset = current_line;
        } else if current_line - self.line_offset > num_lines.saturating_sub(1) {
            self.line_offset = current_line + 1 - num_lines;
        }
    }

    fn undo(&mut self) -> Option<OpaqueDiff> {
        if let Some((diff, cursor)) = self.text.undo() {
            self.cursor = cursor;
            if let Some(parser) = self.parser.as_mut() {
                parser.edit(&diff);
                parser.spawn(
                    &self.properties.context.task_pool,
                    self.text.staged().clone(),
                    true,
                );
            }
            Some(diff)
        } else {
            None
        }
    }

    fn redo(&mut self) -> Option<OpaqueDiff> {
        if let Some((diff, cursor)) = self.text.redo() {
            self.cursor = cursor;
            if let Some(parser) = self.parser.as_mut() {
                parser.edit(&diff);
                parser.spawn(
                    &self.properties.context.task_pool,
                    self.text.staged().clone(),
                    true,
                );
            }
            Some(diff)
        } else {
            None
        }
    }

    fn center_visual_cursor(&mut self) {
        let line_index = self.text.char_to_line(self.cursor.range().start.0);
        if line_index >= self.frame.size.height / 2
            && self.line_offset != line_index - self.frame.size.height / 2
        {
            self.line_offset = line_index - self.frame.size.height / 2;
        } else if self.line_offset != line_index {
            self.line_offset = line_index;
        } else {
            self.line_offset = 0;
        }
    }

    fn delete_line(&mut self) -> OpaqueDiff {
        let operation = self.cursor.delete_line(&mut self.text);
        operation.diff
    }

    fn copy_selection_to_clipboard(&mut self) -> OpaqueDiff {
        let selection = self.cursor.selection();
        self.properties
            .context
            .clipboard
            .set_contents(self.text.slice(selection.start.0..selection.end.0).into())
            .unwrap();
        self.cursor.clear_selection();
        OpaqueDiff::empty()
    }

    fn cut_selection_to_clipboard(&mut self) -> OpaqueDiff {
        let operation = self.cursor.delete_selection(&mut self.text);
        self.properties
            .context
            .clipboard
            .set_contents(operation.deleted.into())
            .unwrap();
        operation.diff
    }

    fn paste_from_clipboard(&mut self) -> OpaqueDiff {
        let clipboard_str = self.properties.context.clipboard.get_contents().unwrap();
        if !clipboard_str.is_empty() {
            self.cursor
                .insert_chars(&mut self.text, clipboard_str.chars())
        } else {
            OpaqueDiff::empty()
        }
    }
}

#[derive(Debug)]
pub enum Message {
    // Movement
    Up,
    Down,
    Left,
    Right,
    PageDown,
    PageUp,
    StartOfLine,
    EndOfLine,
    StartOfBuffer,
    EndOfBuffer,
    CenterCursorVisually,

    // Editing
    BeginSelection,
    ClearSelection,
    SelectAll,
    DeleteForward,
    DeleteBackward,
    DeleteLine,
    Yank,
    CopySelection,
    CutSelection,
    InsertTab,
    InsertNewLine,
    InsertChar(char),

    // Undo / Redo
    Undo,
    Redo,
    ToggleEditTree,

    // Buffer
    SaveBuffer,
    SaveFile(Result<Rope>),
    ParseSyntax(Result<ParserStatus>),
}

#[derive(Clone)]
pub struct Properties {
    pub context: Rc<Context>,
    pub theme: Cow<'static, Theme>,
    pub focused: bool,
    pub frame_id: usize,
    pub mode: &'static Mode,
    pub repo: Option<RepositoryRc>,
    pub content: Rope,
    pub file_path: Option<PathBuf>,
    pub log_message: Callback<String>,
}

impl PartialEq for Properties {
    fn eq(&self, other: &Self) -> bool {
        *self.theme == *other.theme
            && self.focused == other.focused
            && self.frame_id == other.frame_id
    }
}

impl Component for Buffer {
    type Properties = Properties;
    type Message = Message;

    fn create(properties: Self::Properties, frame: Rect, link: ComponentLink<Self>) -> Self {
        let link_clone = link.clone();
        let mut parser = properties
            .mode
            .language()
            .map(move |language| ParserPool::new(link_clone, *language));
        if let Some(parser) = parser.as_mut() {
            parser.ensure_tree(&properties.context.task_pool, || properties.content.clone());
        };

        Buffer {
            text: EditTree::new(properties.content.clone()),
            has_unsaved_changes: ModifiedStatus::Unchanged,
            cursor: Cursor::new(),
            line_offset: 0,
            parser,
            viewing_edit_tree: false,

            properties,
            frame,
            link,
        }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        let should_render = (self.properties.theme != properties.theme
            || self.properties.focused != properties.focused
            || self.properties.frame_id != properties.frame_id)
            .into();
        self.properties = properties;
        should_render
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        self.ensure_cursor_in_view();
        ShouldRender::Yes
    }

    fn update(&mut self, message: Message) -> ShouldRender {
        self.reduce(message);
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        // The textarea components that displays text
        let textarea = TextArea::with(TextAreaProperties {
            theme: self.properties.theme.syntax.clone(),
            focused: self.properties.focused,
            text: self.text.staged().clone(),
            cursor: self.cursor.clone(),
            mode: self.properties.mode,
            line_offset: self.line_offset,
            parse_tree: self.parser.as_ref().and_then(|parser| parser.tree.clone()),
        });

        // Vertical info bar which shows line specific diagnostics
        let line_info = LineInfo::with(LineInfoProperties {
            style: self.properties.theme.border,
            line_offset: self.line_offset,
            num_lines: self.text.len_lines(),
        });

        // The "status bar" which shows information about the file etc.
        let status_bar = StatusBar::with(StatusBarProperties {
            current_line_index: self.text.char_to_line(self.cursor.range().start.0),
            file_path: self.properties.file_path.clone(),
            focused: self.properties.focused,
            frame_id: self.properties.frame_id,
            has_unsaved_changes: self.has_unsaved_changes,
            mode: self.properties.mode.into(),
            num_lines: self.text.len_lines(),
            repository: self.properties.repo.clone(),
            size_bytes: self.text.len_bytes() as u64,
            theme: self.properties.theme.status_bar.clone(),
            // TODO: Fix visual_cursor_x to display the column (i.e. unicode
            // width). It used to be computed by draw_line.
            visual_cursor_x: self.cursor.range().start.0,
        });

        // Edit-tree viewer (aka. undo/redo tree)
        let edit_tree_viewer = if self.viewing_edit_tree {
            Some(Item::fixed(EDIT_TREE_WIDTH)(Container::row([
                Item::fixed(1)(Text::with(
                    TextProperties::new().style(self.properties.theme.border),
                )),
                Item::auto(Container::column([
                    Item::auto(EditTreeViewer::with(EditTreeViewerProperties {
                        tree: self.text.clone(),
                        theme: self.properties.theme.edit_tree_viewer.clone(),
                    })),
                    Item::fixed(1)(Text::with(
                        TextProperties::new()
                            .content("Edit Tree Viewer 🌴")
                            .style(self.properties.theme.border)
                            .align(TextAlign::Centre),
                    )),
                ])),
            ])))
        } else {
            None
        };

        Layout::column([
            Item::auto(Layout::row(
                iter::once(edit_tree_viewer)
                    .chain(iter::once(Some(Item::fixed(1)(line_info))))
                    .chain(iter::once(Some(Item::auto(textarea))))
                    .flatten(),
            )),
            Item::fixed(1)(status_bar),
        ])
    }

    fn has_focus(&self) -> bool {
        self.properties.focused
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let transition = BindingTransition::Clear;
        log::debug!("{:?}", pressed);
        let message = match pressed {
            // Cursor movement
            [Key::Ctrl('p')] | [Key::Up] => Message::Up,
            [Key::Ctrl('n')] | [Key::Down] => Message::Down,
            [Key::Ctrl('b')] | [Key::Left] => Message::Left,
            [Key::Ctrl('f')] | [Key::Right] => Message::Right,
            [Key::Ctrl('v')] | [Key::PageDown] => Message::PageDown,
            [Key::Alt('v')] | [Key::PageUp] => Message::PageUp,
            [Key::Ctrl('a')] | [Key::Home] => Message::StartOfLine,
            [Key::Ctrl('e')] | [Key::End] => Message::EndOfLine,
            [Key::Alt('<')] => Message::StartOfBuffer,
            [Key::Alt('>')] => Message::EndOfBuffer,
            [Key::Ctrl('l')] => Message::CenterCursorVisually,

            // Editing
            [Key::Null] | [Key::Ctrl(' ')] => Message::BeginSelection,
            [Key::Ctrl('g')] => Message::ClearSelection,
            [Key::Ctrl('x'), Key::Char('h')] => Message::SelectAll,
            [Key::Alt('w')] => Message::CopySelection,
            [Key::Ctrl('w')] => Message::CutSelection,
            [Key::Ctrl('y')] => Message::Yank,
            [Key::Ctrl('d')] | [Key::Delete] => Message::DeleteForward,
            [Key::Backspace] => Message::DeleteBackward,
            [Key::Ctrl('k')] => Message::DeleteLine,
            [Key::Char('\n')] => Message::InsertNewLine,
            [Key::Char('\t')] if DISABLE_TABS => Message::InsertTab,
            &[Key::Char(character)] if character != '\n' => Message::InsertChar(character),

            // Undo / Redo
            [Key::Ctrl('x'), Key::Char('u')] | [Key::Ctrl('x'), Key::Ctrl('u')] => {
                Message::ToggleEditTree
            }
            [Key::Ctrl('_')] | [Key::Ctrl('z')] | [Key::Ctrl('/')] => Message::Undo,
            [Key::Ctrl('q')] => Message::Redo,

            // Buffer
            [Key::Ctrl('x'), Key::Ctrl('s')] | [Key::Ctrl('x'), Key::Char('s')] => {
                Message::SaveBuffer
            }
            [Key::Ctrl('x')] => {
                return {
                    BindingMatch {
                        transition: BindingTransition::Continue,
                        message: None,
                    }
                }
            }
            _ => {
                return {
                    BindingMatch {
                        transition: BindingTransition::Clear,
                        message: None,
                    }
                }
            }
        };

        BindingMatch {
            transition,
            message: Some(message),
        }
    }
}

#[derive(Clone)]
pub struct RepositoryRc(Rc<Repository>);

impl RepositoryRc {
    pub fn new(repository: Repository) -> Self {
        Self(Rc::new(repository))
    }
}

impl PartialEq for RepositoryRc {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for RepositoryRc {
    type Target = Repository;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

const DISABLE_TABS: bool = false;
const EDIT_TREE_WIDTH: usize = 36;
