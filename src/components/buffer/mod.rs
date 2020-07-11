mod line_info;
mod status_bar;

use euclid::default::SideOffsets2D;
use git2::Repository;
use ropey::{Rope, RopeSlice};
use std::{borrow::Cow, cmp, fs::File, io::BufWriter, iter, path::PathBuf, rc::Rc};
use zee_highlight::SelectorNodeId;
use zi::{
    layout, terminal::Key, BindingMatch, BindingTransition, Canvas, Component, ComponentLink,
    Layout, Position, Rect, ShouldRender, Size, Style,
};

use self::{
    line_info::{LineInfo, Properties as LineInfoProperties},
    status_bar::{Properties as StatusBarProperties, StatusBar},
};
use super::{
    cursor::{CharIndex, Cursor},
    edit_tree_viewer::{EditTreeViewer, Theme as EditTreeViewerTheme},
};
use crate::{
    editor::Context,
    error::Result,
    mode::Mode,
    syntax::{
        highlight::{text_style_at_char, Theme as SyntaxTheme},
        parse::{NodeTrace, OpaqueDiff, ParserStatus, SyntaxCursor, SyntaxTree},
    },
    undo::EditTree,
    utils::{self, strip_trailing_whitespace, RopeGraphemes, TAB_WIDTH},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub syntax: SyntaxTheme,
    pub edit_tree_viewer: EditTreeViewerTheme,
    pub border: Style,
    pub status_base: Style,
    pub status_frame_id_focused: Style,
    pub status_frame_id_unfocused: Style,
    pub status_is_modified: Style,
    pub status_is_not_modified: Style,
    pub status_file_name: Style,
    pub status_file_size: Style,
    pub status_position_in_file: Style,
    pub status_mode: Style,
}

#[derive(Clone, Debug, PartialEq)]
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
    clipboard: Option<Rope>,
    has_unsaved_changes: ModifiedStatus,
    cursor: Cursor,
    line_offset: usize,
    syntax: Option<SyntaxTree>,
    viewing_edit_tree: bool,
    // bindings: BufferBindings,
}

impl Buffer {
    pub fn spawn_save_file(&mut self) {
        self.has_unsaved_changes = ModifiedStatus::Saving;
        if let Some(ref file_path) = self.properties.file_path {
            let text = self.text.clone();
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
                let new_text = new_text.unwrap();
                self.cursor.sync(&self.text, &new_text);
                self.text
                    .new_revision(OpaqueDiff::empty(), self.cursor.clone());
                *self.text = new_text;
                self.has_unsaved_changes = ModifiedStatus::Unchanged;
                return;
            }
            Message::ParseSyntax(parsed) => {
                let parsed = parsed.unwrap();
                if let Some(syntax) = self.syntax.as_mut() {
                    syntax.handle_parse_syntax_done(parsed);
                }
                return;
            }
            _ => {}
        };

        let mut undoing = false;
        let diff = match message {
            Message::DeleteForward => {
                let operation = self.cursor.delete(&mut self.text);
                self.clipboard = Some(operation.deleted);
                operation.diff
            }
            Message::DeleteBackward => self.cursor.backspace(&mut self.text).diff,
            Message::DeleteLine => self.delete_line(),
            Message::Yank => self.yank_line(),
            Message::CopySelection => self.copy_selection(),
            Message::CutSelection => self.cut_selection(),
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
            self.text.new_revision(diff.clone(), self.cursor.clone());
        }

        match self.syntax.as_mut() {
            Some(syntax) if !diff.is_empty() && !undoing => {
                syntax.edit(&diff);
                syntax.spawn_parse_task(
                    &self.properties.context.task_pool,
                    self.text.head().clone(),
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
            if let Some(syntax) = self.syntax.as_mut() {
                syntax.edit(&diff);
                syntax.spawn_parse_task(
                    &self.properties.context.task_pool,
                    self.text.head().clone(),
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
            if let Some(syntax) = self.syntax.as_mut() {
                syntax.edit(&diff);
                syntax.spawn_parse_task(
                    &self.properties.context.task_pool,
                    self.text.head().clone(),
                    true,
                );
            }
            Some(diff)
        } else {
            None
        }
    }

    #[inline]
    fn draw_line(
        &self,
        screen: &mut Canvas,
        frame: Rect,
        line_index: usize,
        line: RopeSlice,
        mut syntax_cursor: Option<&mut SyntaxCursor>,
        mut trace: &mut NodeTrace<SelectorNodeId>,
    ) -> usize {
        // Get references to the relevant bits of context
        let Self {
            properties: Properties {
                ref theme, focused, ..
            },
            ..
        } = *self;

        // Highlight the currently selected line
        let line_under_cursor = self.text.char_to_line(self.cursor.range().start.0) == line_index;
        if line_under_cursor && focused {
            screen.clear_region(
                Rect::new(
                    Position::new(frame.origin.x, frame.origin.y),
                    Size::new(frame.size.width, 1),
                ),
                theme.syntax.text_current_line,
            );
        }

        let mut visual_cursor_x = 0;
        let mut visual_x = frame.origin.x;
        let mut char_index = CharIndex(self.text.line_to_char(line_index));

        let mut content: Cow<str> = self
            .text
            .slice(
                self.text.byte_to_char(trace.byte_range.start)
                    ..self.text.byte_to_char(trace.byte_range.end),
            )
            .into();
        let mut scope = self
            .properties
            .mode
            .highlights()
            .and_then(|highlights| highlights.matches(&trace.trace, &trace.nth_children, &content))
            .map(|scope| scope.0.as_str());

        for grapheme in RopeGraphemes::new(&line.slice(..)) {
            let byte_index = self.text.char_to_byte(char_index.0);
            match (syntax_cursor.as_mut(), self.properties.mode.highlights()) {
                (Some(syntax_cursor), Some(highlights))
                    if !trace.byte_range.contains(&byte_index) =>
                {
                    syntax_cursor.trace_at(&mut trace, byte_index, |node| {
                        highlights.get_selector_node_id(node.kind_id())
                    });
                    content = self
                        .text
                        .slice(
                            self.text.byte_to_char(trace.byte_range.start)
                                ..self.text.byte_to_char(trace.byte_range.end),
                        )
                        .into();

                    scope = highlights
                        .matches(&trace.trace, &trace.nth_children, &content)
                        .map(|scope| scope.0.as_str());
                }
                _ => {}
            };

            if self.cursor.range().contains(&char_index) && focused {
                // eprintln!(
                //     "Symbol under cursor [{}] -- {:?} {:?} {:?} {}",
                //     scope.unwrap_or(""),
                //     trace.path,
                //     trace.trace,
                //     trace.nth_children,
                //     content,
                // );
                visual_cursor_x = visual_x.saturating_sub(frame.origin.x);
            }

            let style = text_style_at_char(
                &theme.syntax,
                &self.cursor,
                char_index,
                focused,
                line_under_cursor,
                scope.unwrap_or(""),
                trace.is_error,
            );
            let grapheme_width = utils::grapheme_width(&grapheme);
            let horizontal_bounds_inclusive = frame.min_x()..=frame.max_x();
            if !horizontal_bounds_inclusive.contains(&(visual_x + grapheme_width)) {
                break;
            }

            if grapheme == "\t" {
                for offset in 0..grapheme_width {
                    screen.draw_str(visual_x + offset, frame.origin.y, style, " ");
                }
            } else if grapheme_width == 0 {
                screen.draw_str(visual_x, frame.origin.y, style, " ");
            } else {
                screen.draw_str(visual_x, frame.origin.y, style, &grapheme.to_string());
                // screen.draw_rope_slice(visual_x, frame.origin.y, style, &grapheme);
            }

            char_index.0 += grapheme.len_chars();
            visual_x += grapheme_width;
        }

        if line_index == self.text.len_lines() - 1
            && self.cursor.range().start == self.text.len_chars().into()
        {
            screen.draw_str(
                frame.origin.x,
                frame.origin.y,
                if focused {
                    theme.syntax.cursor_focused
                } else {
                    theme.syntax.cursor_unfocused
                },
                " ",
            );
        }

        visual_cursor_x
    }

    #[inline]
    fn draw_text(&self, screen: &mut Canvas) -> usize {
        let mut syntax_cursor = self.syntax.as_ref().and_then(|syntax| syntax.cursor());
        let mut trace: NodeTrace<SelectorNodeId> = NodeTrace::new();

        let mut visual_cursor_x = 0;
        for (line_index, line) in self
            .text
            .lines_at(self.line_offset)
            .take(screen.size().height)
            .enumerate()
        {
            visual_cursor_x = cmp::max(
                visual_cursor_x,
                self.draw_line(
                    screen,
                    Rect::from_size(screen.size())
                        .inner_rect(SideOffsets2D::new(line_index, 0, 0, 0)),
                    self.line_offset + line_index,
                    line,
                    syntax_cursor.as_mut(),
                    &mut trace,
                ),
            );
        }

        visual_cursor_x
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
        self.clipboard = Some(operation.deleted);
        operation.diff
    }

    fn yank_line(&mut self) -> OpaqueDiff {
        match self.clipboard.as_ref() {
            Some(clipboard) => self
                .cursor
                .insert_slice(&mut self.text, clipboard.slice(..)),
            None => OpaqueDiff::empty(),
        }
    }

    fn copy_selection(&mut self) -> OpaqueDiff {
        let selection = self.cursor.selection();
        self.clipboard = Some(self.text.slice(selection.start.0..selection.end.0).into());
        self.cursor.clear_selection();
        OpaqueDiff::empty()
    }

    fn cut_selection(&mut self) -> OpaqueDiff {
        let operation = self.cursor.delete_selection(&mut self.text);
        self.clipboard = Some(operation.deleted);
        operation.diff
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
}

impl PartialEq for Properties {
    fn eq(&self, other: &Self) -> bool {
        *self.theme == *other.theme
            && self.focused == other.focused
            && self.frame_id == other.frame_id
    }
}

impl Component for Buffer {
    type Message = Message;
    type Properties = Properties;

    fn create(properties: Self::Properties, frame: Rect, link: ComponentLink<Self>) -> Self {
        let link2 = link.clone();
        let mut syntax = properties
            .mode
            .language()
            .map(move |language| SyntaxTree::new(link2.clone(), *language));
        if let Some(syntax) = syntax.as_mut() {
            syntax.ensure_tree(&properties.context.task_pool, || properties.content.clone());
        };

        Buffer {
            text: EditTree::new(properties.content.clone()),
            clipboard: None,
            has_unsaved_changes: ModifiedStatus::Unchanged,
            cursor: Cursor::new(),
            line_offset: 0,
            syntax,
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
        let mut text_canvas = Canvas::new(
            self.frame
                .inner_rect(SideOffsets2D::new(
                    0,
                    0,
                    1,
                    if self.viewing_edit_tree { 32 } else { 1 },
                ))
                .size,
        );
        text_canvas.clear(self.properties.theme.syntax.text);
        let visual_cursor_x = self.draw_text(&mut text_canvas);

        let mut edit_tree_viewer_canvas = None;
        if self.viewing_edit_tree {
            let mut canvas = Canvas::new(
                self.frame
                    .inner_rect(SideOffsets2D::new(
                        0,
                        self.frame.size.width.saturating_sub(31),
                        1,
                        0,
                    ))
                    .size,
            );
            EditTreeViewer.draw(
                &mut canvas,
                &self.text,
                &self.properties.theme.edit_tree_viewer,
            );
            edit_tree_viewer_canvas = Some(layout::fixed(32, canvas.into()));
        }
        layout::column([
            layout::auto(layout::row_iter(
                iter::once(edit_tree_viewer_canvas)
                    .chain(iter::once(Some(layout::fixed(
                        1,
                        layout::component::<LineInfo>(LineInfoProperties {
                            style: self.properties.theme.border,
                            line_offset: self.line_offset,
                            num_lines: self.text.len_lines(),
                        }),
                    ))))
                    .chain(iter::once(Some(layout::auto(text_canvas.into()))))
                    .filter_map(|x| x),
            )),
            //     )
            //     edit_tree_viewer_canvas
            //         .map(|edit_tree_viewer| {
            //             layout::row([
            //                 layout::fixed(1, line_info()),
            //                 layout::auto(text_canvas.into()),
            //             ])
            //         })
            //         .unwrap_or_else(|| {
            //             layout::row([
            //                 layout::fixed(1, line_info()),
            //                 layout::auto(text_canvas.into()),
            //             ])
            //         }),
            // ),
            layout::fixed(
                1,
                layout::component::<StatusBar>(StatusBarProperties {
                    current_line_index: self.text.char_to_line(self.cursor.range().start.0),
                    file_path: self.properties.file_path.clone(),
                    focused: self.properties.focused,
                    frame_id: self.properties.frame_id,
                    has_unsaved_changes: self.has_unsaved_changes.clone(),
                    mode: self.properties.mode.into(),
                    num_lines: self.text.len_lines(),
                    repository: self.properties.repo.clone(),
                    size_bytes: self.text.len_bytes() as u64,
                    theme: self.properties.theme.clone(),
                    visual_cursor_x,
                }),
            ),
        ])
    }

    fn has_focus(&self) -> bool {
        self.properties.focused
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let transition = BindingTransition::Clear;
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
            [Key::Null] => Message::BeginSelection,
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
            [Key::Ctrl('/')] | [Key::Ctrl('z')] => Message::Undo,
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
