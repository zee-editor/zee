use git2::Repository;
use ropey::Rope;
use std::{
    fmt::Display,
    fs::File,
    io::{self, BufWriter},
    path::{Path, PathBuf},
    rc::Rc,
};
use zi::ComponentLink;

use zee_edit::{
    graphemes::strip_trailing_whitespace, movement, tree::EditTree, Cursor, Direction, OpaqueDiff,
};
use zee_grammar::Mode;

use super::{ContextHandle, Editor};
use crate::{
    config::PLAIN_TEXT_MODE,
    error::Result,
    syntax::parse::{ParseTree, ParserPool, ParserStatus},
    versioned::{Versioned, WeakHandle},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferId(usize);

impl Display for BufferId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "BufferId({})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CursorId(usize);

impl Display for CursorId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "CursorId({})", self.0)
    }
}

#[derive(Debug)]
pub struct BuffersMessage {
    buffer_id: BufferId,
    inner: BufferMessage,
}

impl BuffersMessage {
    fn new(buffer_id: BufferId, message: BufferMessage) -> Self {
        Self {
            buffer_id,
            inner: message,
        }
    }
}

pub struct Buffers {
    context: ContextHandle,
    buffers: Vec<Buffer>,
    next_buffer_id: usize,
}

impl Buffers {
    pub fn new(context: ContextHandle) -> Self {
        Self {
            context,
            buffers: Vec::new(),
            next_buffer_id: 0,
        }
    }

    pub fn add(
        &mut self,
        text: Rope,
        file_path: Option<PathBuf>,
        repo: Option<RepositoryRc>,
    ) -> BufferId {
        // Generate a new buffer id
        let buffer_id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;
        self.buffers.push(Buffer::new(
            self.context.clone(),
            buffer_id,
            text,
            file_path,
            repo,
        ));
        buffer_id
    }

    pub fn remove(&mut self, id: BufferId) -> Option<Buffer> {
        self.buffers
            .iter()
            .position(|buffer| buffer.id == id)
            .map(|buffer_index| self.buffers.swap_remove(buffer_index))
    }

    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.iter().find(|buffer| buffer.id == id)
    }

    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.buffers.iter_mut().find(|buffer| buffer.id == id)
    }

    pub fn find_by_path(&self, path: impl AsRef<Path>) -> Option<BufferId> {
        self.buffers
            .iter()
            .find(|buffer| {
                buffer
                    .file_path
                    .as_ref()
                    .map(|buffer_path| *buffer_path == *path.as_ref())
                    .unwrap_or(false)
            })
            .map(|buffer| buffer.id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Buffer> {
        self.buffers.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Buffer> {
        self.buffers.iter_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    pub fn handle_message(&mut self, message: BuffersMessage) {
        match self.get_mut(message.buffer_id) {
            Some(buffer) => {
                buffer.handle_message(message.inner);
            }
            None => {
                log::warn!(
                    "Received message for unknown buffer_id={} message={:?}",
                    message.buffer_id,
                    message
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModifiedStatus {
    Changed,
    Unchanged,
    Saving,
}

pub struct Buffer {
    context: ContextHandle,
    id: BufferId,
    mode: &'static Mode,
    repo: Option<RepositoryRc>,
    content: Versioned<EditTree>,
    file_path: Option<PathBuf>,
    modified_status: ModifiedStatus,
    cursors: Vec<Cursor>,
    parser: Option<ParserPool>,
}

impl Buffer {
    fn new(
        context: ContextHandle,
        id: BufferId,
        text: Rope,
        file_path: Option<PathBuf>,
        repo: Option<RepositoryRc>,
    ) -> Self {
        let mode = file_path
            .as_ref()
            .map(|path| context.0.mode_by_filename(path))
            .unwrap_or(&PLAIN_TEXT_MODE);

        let mut parser = mode
            .language()
            .and_then(|result| result.ok())
            .map(ParserPool::new);
        if let Some(parser) = parser.as_mut() {
            let link = context.link.clone();
            parser.ensure_tree(
                &context.task_pool,
                || text.clone(),
                move |status| {
                    link.send(
                        BuffersMessage::new(id, BufferMessage::ParseSyntax { version: 0, status })
                            .into(),
                    )
                },
            );
        };

        Self {
            context,
            id,
            mode,
            repo,
            content: Versioned::new(EditTree::new(text)),
            file_path,
            modified_status: ModifiedStatus::Unchanged,
            cursors: vec![Cursor::new()],
            parser,
        }
    }

    #[inline]
    pub fn id(&self) -> BufferId {
        self.id
    }

    #[inline]
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    #[inline]
    pub fn mode(&self) -> &'static Mode {
        self.mode
    }

    #[inline]
    pub fn repository(&self) -> Option<&RepositoryRc> {
        self.repo.as_ref()
    }

    #[inline]
    pub fn edit_tree(&self) -> &EditTree {
        &self.content
    }

    #[inline]
    pub fn edit_tree_handle(&self) -> WeakHandle<EditTree> {
        self.content.weak()
    }

    #[inline]
    pub fn cursor(&self, cursor_id: CursorId) -> &Cursor {
        &self.cursors[cursor_id.0]
    }

    #[inline]
    pub fn modified_status(&self) -> ModifiedStatus {
        self.modified_status
    }

    #[inline]
    pub fn new_cursor(&mut self) -> CursorId {
        let new_cursor_id = CursorId(self.cursors.len());
        self.cursors
            .push(self.cursors.get(0).cloned().unwrap_or_else(Cursor::new));
        new_cursor_id
    }

    #[inline]
    pub fn duplicate_cursor(&mut self, cursor_id: CursorId) -> CursorId {
        let new_cursor_id = CursorId(self.cursors.len());
        self.cursors.push(self.cursors[cursor_id.0].clone());
        new_cursor_id
    }

    #[inline]
    pub fn parse_tree(&self) -> Option<&ParseTree> {
        self.parser.as_ref().and_then(|parser| parser.tree.as_ref())
    }

    #[inline]
    pub fn handle_message(&mut self, message: BufferMessage) {
        match message {
            // Start writing the buffer to disk asynchronously
            BufferMessage::SaveBufferStart => {
                self.spawn_save_file();
            }
            // Saved the buffer successfully
            BufferMessage::SaveBufferEnd(Ok(new_content)) => {
                for cursor in self.cursors.iter_mut() {
                    cursor.sync(&self.content, &new_content);
                }
                self.content
                    .create_revision(OpaqueDiff::empty(), self.cursors[0].clone());
                *self.content.staged_mut() = new_content;
                self.modified_status = ModifiedStatus::Unchanged;
            }
            // Failed to save the buffer
            BufferMessage::SaveBufferEnd(Err(error)) => {
                self.context.log(error.to_string());
            }
            // The syntax parser finished parsing the code (tree-sitter)
            BufferMessage::ParseSyntax { version, status } => {
                let parsed = status.unwrap();
                if let Some(parser) = self.parser.as_mut() {
                    parser.handle_parse_syntax_done(version, parsed);
                }
            }
            BufferMessage::CursorMessage { cursor_id, message } => {
                self.handle_cursor_message(cursor_id, message)
            }
            BufferMessage::PreviousChildRevision => self.content.previous_child(),
            BufferMessage::NextChildRevision => self.content.next_child(),
        };
    }

    #[inline]
    fn handle_cursor_message(&mut self, cursor_id: CursorId, message: CursorMessage) {
        {
            let content = &self.content;
            let cursor = &mut self.cursors[cursor_id.0];
            // Stateless
            match message {
                CursorMessage::Up(n) => movement::move_vertically(
                    content,
                    cursor,
                    self.mode.indentation.tab_width(),
                    Direction::Backward,
                    n,
                ),
                CursorMessage::Down(n) => movement::move_vertically(
                    content,
                    cursor,
                    self.mode.indentation.tab_width(),
                    Direction::Forward,
                    n,
                ),
                CursorMessage::Left => {
                    movement::move_horizontally(content, cursor, Direction::Backward, 1)
                }
                CursorMessage::Right => {
                    movement::move_horizontally(content, cursor, Direction::Forward, 1)
                }
                CursorMessage::StartOfLine => movement::move_to_start_of_line(content, cursor),
                CursorMessage::EndOfLine => movement::move_to_end_of_line(content, cursor),
                CursorMessage::StartOfBuffer => movement::move_to_start_of_buffer(content, cursor),
                CursorMessage::EndOfBuffer => movement::move_to_end_of_buffer(content, cursor),
                CursorMessage::MoveWord(direction, count) => {
                    movement::move_word(content, cursor, direction, count)
                }
                CursorMessage::MoveParagraph(direction, count) => {
                    movement::move_paragraph(content, cursor, direction, count)
                }

                CursorMessage::BeginSelection => cursor.begin_selection(),
                CursorMessage::ClearSelection => {
                    cursor.clear_selection();
                }
                CursorMessage::SelectAll => cursor.select_all(content),

                _ => {}
            }
        }

        let mut undoing = false;
        let diff = {
            match message {
                CursorMessage::DeleteForward => {
                    let operation = self.cursors[cursor_id.0].delete_forward(&mut self.content);
                    if operation.diff.is_empty() {
                        self.context.log("End of buffer");
                    }
                    operation.diff
                }
                CursorMessage::DeleteBackward => {
                    let operation = self.cursors[cursor_id.0].delete_backward(&mut self.content);
                    if operation.diff.is_empty() {
                        self.context.log("Beginning of buffer");
                    }
                    operation.diff
                }
                CursorMessage::DeleteLine => {
                    let diff = self.delete_line(cursor_id);
                    if diff.is_empty() {
                        self.context.log("End of buffer");
                    }
                    diff
                }
                CursorMessage::Yank => self.paste_from_clipboard(cursor_id),
                CursorMessage::CopySelection => self.copy_selection_to_clipboard(cursor_id),
                CursorMessage::CutSelection => self.cut_selection_to_clipboard(cursor_id),
                CursorMessage::InsertTab => {
                    let (indentation_unit, indentation_count) =
                        match &self.context.config.indentation_override {
                            Some(setting) => (
                                setting.unit.to_char(),
                                match &setting.unit {
                                    zee_grammar::config::IndentationUnit::Space => setting.width,
                                    zee_grammar::config::IndentationUnit::Tab => 1,
                                },
                            ),
                            None => (
                                self.mode.indentation.to_char(),
                                self.mode.indentation.char_count(),
                            ),
                        };
                    let diff = self.cursors[cursor_id.0].insert_chars(
                        &mut self.content,
                        std::iter::repeat(indentation_unit).take(indentation_count),
                    );
                    movement::move_horizontally(
                        &self.content,
                        &mut self.cursors[cursor_id.0],
                        Direction::Forward,
                        indentation_count,
                    );
                    diff
                }
                CursorMessage::InsertNewLine => {
                    let diff = self.cursors[cursor_id.0].insert_char(&mut self.content, '\n');
                    let cursor = &mut self.cursors[cursor_id.0];
                    movement::move_vertically(
                        &self.content,
                        cursor,
                        self.mode.indentation.tab_width(),
                        Direction::Forward,
                        1,
                    );
                    movement::move_to_start_of_line(&self.content, cursor);
                    diff
                }
                CursorMessage::InsertChar {
                    character,
                    move_forward,
                } => {
                    let diff = self.cursors[cursor_id.0].insert_char(&mut self.content, character);
                    if move_forward {
                        movement::move_horizontally(
                            &self.content,
                            &mut self.cursors[cursor_id.0],
                            Direction::Forward,
                            1,
                        );
                    }
                    diff
                }
                CursorMessage::Undo => {
                    undoing = true;
                    self.undo(cursor_id)
                }
                CursorMessage::Redo => {
                    undoing = true;
                    self.redo(cursor_id)
                }

                _ => OpaqueDiff::empty(),
            }
        };

        if !diff.is_empty() {
            self.modified_status = ModifiedStatus::Changed;
            for (id, cursor) in self.cursors.iter_mut().enumerate() {
                if id != cursor_id.0 {
                    cursor.reconcile(&self.content, &diff);
                }
            }
            if !undoing {
                self.content
                    .create_revision(diff.clone(), self.cursors[cursor_id.0].clone());
                self.update_parse_tree(&diff, false);
            }
        }
    }

    fn delete_line(&mut self, cursor_id: CursorId) -> OpaqueDiff {
        self.cursors[cursor_id.0]
            .delete_line(&mut self.content)
            .diff
    }

    fn copy_selection_to_clipboard(&mut self, cursor_id: CursorId) -> OpaqueDiff {
        let selection = self.cursors[cursor_id.0].selection();
        self.context
            .clipboard
            .set_contents(self.content.slice(selection.start..selection.end).into())
            .unwrap();
        self.cursors[cursor_id.0].clear_selection();
        OpaqueDiff::empty()
    }

    fn cut_selection_to_clipboard(&mut self, cursor_id: CursorId) -> OpaqueDiff {
        let operation = self.cursors[cursor_id.0].delete_selection(&mut self.content);
        self.context
            .clipboard
            .set_contents(operation.deleted.into())
            .unwrap();
        operation.diff
    }

    fn paste_from_clipboard(&mut self, cursor_id: CursorId) -> OpaqueDiff {
        let clipboard_str = self.context.clipboard.get_contents().unwrap();
        if !clipboard_str.is_empty() {
            self.cursors[cursor_id.0].insert_chars(&mut self.content, clipboard_str.chars())
        } else {
            OpaqueDiff::empty()
        }
    }

    fn undo(&mut self, cursor_id: CursorId) -> OpaqueDiff {
        self.content
            .undo()
            .map(|(diff, cursor)| {
                self.cursors[cursor_id.0] = cursor;
                self.update_parse_tree(&diff, true);
                diff
            })
            .unwrap_or_else(OpaqueDiff::empty)
    }

    fn redo(&mut self, cursor_id: CursorId) -> OpaqueDiff {
        self.content
            .redo()
            .map(|(diff, cursor)| {
                self.cursors[cursor_id.0] = cursor;
                self.update_parse_tree(&diff, true);
                diff
            })
            .unwrap_or_else(OpaqueDiff::empty)
    }

    fn update_parse_tree(&mut self, diff: &OpaqueDiff, fresh: bool) {
        if let Some(parser) = self.parser.as_mut() {
            let task_pool = &self.context.task_pool;
            let staged_text = self.content.staged().clone();
            let buffer_id = self.id;
            let link = self.context.link.clone();
            let version = self.content.version();
            parser.edit(diff);
            parser.spawn(task_pool, staged_text, fresh, move |status| {
                link.send(
                    BuffersMessage::new(buffer_id, BufferMessage::ParseSyntax { version, status })
                        .into(),
                )
            });
        }
    }

    fn spawn_save_file(&mut self) {
        self.modified_status = ModifiedStatus::Saving;
        if let Some(ref file_path) = self.file_path {
            let buffer_id = self.id;
            let text = self.content.staged().clone();
            let file_path = file_path.clone();
            let link = self.context.link.clone();
            self.context.task_pool.spawn(move |_| {
                let buffer_message = BufferMessage::SaveBufferEnd(
                    File::create(&file_path)
                        .map(BufWriter::new)
                        .and_then(|writer| {
                            let text = strip_trailing_whitespace(text);
                            text.write_to(writer)?;
                            Ok(text)
                        }),
                );
                link.send(BuffersMessage::new(buffer_id, buffer_message).into())
            });
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct BufferCursor {
    buffer_id: BufferId,
    cursor_id: CursorId,
    cursor: Cursor,
    link: ComponentLink<Editor>,
}

impl BufferCursor {
    pub fn new(
        buffer_id: BufferId,
        cursor_id: CursorId,
        cursor: Cursor,
        link: ComponentLink<Editor>,
    ) -> Self {
        Self {
            buffer_id,
            cursor_id,
            cursor,
            link,
        }
    }

    #[inline]
    pub fn send_message(&self, message: BufferMessage) {
        self.link.send(
            BuffersMessage {
                buffer_id: self.buffer_id,
                inner: message,
            }
            .into(),
        );
    }

    #[inline]
    pub fn send_cursor(&self, message: CursorMessage) {
        self.send_message(BufferMessage::CursorMessage {
            cursor_id: self.cursor_id,
            message,
        });
    }

    #[inline]
    pub fn save(&self) {
        self.send_message(BufferMessage::SaveBufferStart);
    }

    pub fn inner(&self) -> &Cursor {
        &self.cursor
    }

    pub fn previous_child_revision(&self) {
        self.send_message(BufferMessage::PreviousChildRevision)
    }

    pub fn next_child_revision(&self) {
        self.send_message(BufferMessage::NextChildRevision)
    }

    #[inline]
    pub fn move_up(&self) {
        self.send_cursor(CursorMessage::Up(1));
    }

    #[inline]
    pub fn move_up_n(&self, n: usize) {
        self.send_cursor(CursorMessage::Up(n));
    }

    #[inline]
    pub fn move_down(&self) {
        self.send_cursor(CursorMessage::Down(1));
    }

    #[inline]
    pub fn move_down_n(&self, n: usize) {
        self.send_cursor(CursorMessage::Down(n));
    }

    #[inline]
    pub fn move_left(&self) {
        self.send_cursor(CursorMessage::Left);
    }

    #[inline]
    pub fn move_right(&self) {
        self.send_cursor(CursorMessage::Right);
    }

    #[inline]
    pub fn move_start_of_line(&self) {
        self.send_cursor(CursorMessage::StartOfLine);
    }

    #[inline]
    pub fn move_end_of_line(&self) {
        self.send_cursor(CursorMessage::EndOfLine);
    }

    #[inline]
    pub fn move_start_of_buffer(&self) {
        self.send_cursor(CursorMessage::StartOfBuffer);
    }

    #[inline]
    pub fn move_end_of_buffer(&self) {
        self.send_cursor(CursorMessage::EndOfBuffer);
    }

    #[inline]
    pub fn begin_selection(&self) {
        self.send_cursor(CursorMessage::BeginSelection);
    }

    #[inline]
    pub fn clear_selection(&self) {
        self.send_cursor(CursorMessage::ClearSelection);
    }

    #[inline]
    pub fn select_all(&self) {
        self.send_cursor(CursorMessage::SelectAll);
    }

    #[inline]
    pub fn paste_from_clipboard(&self) {
        self.send_cursor(CursorMessage::Yank);
    }

    #[inline]
    pub fn copy_selection_to_clipboard(&self) {
        self.send_cursor(CursorMessage::CopySelection);
    }

    #[inline]
    pub fn cut_selection_to_clipboard(&self) {
        self.send_cursor(CursorMessage::CutSelection);
    }

    #[inline]
    pub fn undo(&self) {
        self.send_cursor(CursorMessage::Undo);
    }

    #[inline]
    pub fn redo(&self) {
        self.send_cursor(CursorMessage::Redo);
    }

    #[inline]
    pub fn delete_forward(&self) {
        self.send_cursor(CursorMessage::DeleteForward);
    }

    #[inline]
    pub fn delete_backward(&self) {
        self.send_cursor(CursorMessage::DeleteBackward);
    }

    #[inline]
    pub fn delete_line(&self) {
        self.send_cursor(CursorMessage::DeleteLine);
    }

    #[inline]
    pub fn insert_new_line(&self) {
        self.send_cursor(CursorMessage::InsertNewLine);
    }

    #[inline]
    pub fn insert_tab(&self) {
        self.send_cursor(CursorMessage::InsertTab);
    }

    #[inline]
    pub fn insert_char(&self, character: char, move_forward: bool) {
        self.send_cursor(CursorMessage::InsertChar {
            character,
            move_forward,
        });
    }
}

#[derive(Debug)]
pub enum BufferMessage {
    SaveBufferStart,
    SaveBufferEnd(io::Result<Rope>),
    ParseSyntax {
        version: usize,
        status: Result<ParserStatus>,
    },
    PreviousChildRevision,
    NextChildRevision,
    CursorMessage {
        cursor_id: CursorId,
        message: CursorMessage,
    },
}

#[derive(Debug)]
pub enum CursorMessage {
    // Movement
    Up(usize),
    Down(usize),
    Left,
    Right,
    StartOfLine,
    EndOfLine,
    StartOfBuffer,
    EndOfBuffer,
    MoveWord(Direction, usize),
    MoveParagraph(Direction, usize),

    // Editing
    BeginSelection,
    ClearSelection,
    SelectAll,
    Yank,
    CopySelection,
    CutSelection,

    DeleteForward,
    DeleteBackward,
    DeleteLine,
    InsertTab,
    InsertNewLine,
    InsertChar { character: char, move_forward: bool },

    // Undo / Redo
    Undo,
    Redo,
}

#[derive(Clone)]
pub struct RepositoryRc(pub Rc<Repository>);

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
