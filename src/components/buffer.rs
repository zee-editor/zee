use euclid::default::SideOffsets2D;
use git2::Repository;
use lazy_static::lazy_static;
use maplit::hashmap;
use ropey::{Rope, RopeSlice};
use size_format::SizeFormatterBinary;
use smallvec::smallvec;
use std::{
    borrow::Cow,
    cmp,
    fs::File,
    io::{self, BufReader, BufWriter},
    iter,
    path::{Path, PathBuf},
    time::Instant,
};
use zee_highlight::SelectorNodeId;

use super::{
    cursor::{CharIndex, Cursor},
    theme::Theme as EditorTheme,
    BindingMatch, Bindings, Component, Context, HashBindings, TaskDone,
};
use crate::{
    error::Result,
    mode::{self, Mode},
    syntax::{
        highlight::{text_style_at_char, Theme as SyntaxTheme},
        parse::{NodeTrace, OpaqueDiff, ParserStatus, SyntaxCursor, SyntaxTree},
    },
    task,
    terminal::{Key, Position, Rect, Screen, Size, Style},
    undo::UndoTree,
    utils::{self, strip_trailing_whitespace, RopeGraphemes, TAB_WIDTH},
};

pub type Scheduler<'pool> = task::Scheduler<'pool, Result<BufferTask>>;

#[derive(Clone, Debug)]
pub struct Theme {
    pub syntax: SyntaxTheme,
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

#[derive(Clone, Debug)]
enum ModifiedStatus {
    Changed,
    Unchanged,
    Saving(Instant),
}

pub enum BufferTask {
    SaveFile { text: Rope },
    ParseSyntax(ParserStatus),
}

pub struct Buffer {
    mode: &'static Mode,
    text: UndoTree,
    clipboard: Option<Rope>,
    has_unsaved_changes: ModifiedStatus,
    file_path: Option<PathBuf>,
    cursor: Cursor,
    first_line: usize,
    syntax: Option<SyntaxTree>,
    repo: Option<Repository>,
    bindings: BufferBindings,
}

impl Buffer {
    pub fn from_file(file_path: PathBuf) -> Result<Self> {
        let mode = mode::find_by_filename(&file_path);
        let repo = Repository::discover(&file_path).ok();
        let text = if file_path.exists() {
            Rope::from_reader(BufReader::new(File::open(&file_path)?))?
        } else {
            // Optimistically check if we can create it
            File::open(&file_path).map(|_| ()).or_else(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(error)
                }
            })?;
            Rope::new()
        };
        Ok(Buffer {
            text: UndoTree::new(text),
            clipboard: None,
            has_unsaved_changes: ModifiedStatus::Unchanged,
            file_path: Some(file_path),
            cursor: Cursor::new(),
            first_line: 0,
            syntax: mode.language().map(|language| SyntaxTree::new(*language)),
            mode,
            repo,
            bindings: BufferBindings,
        })
    }

    pub fn spawn_save_file(&mut self, scheduler: &mut Scheduler, context: &Context) -> Result<()> {
        self.has_unsaved_changes = ModifiedStatus::Saving(context.time);
        if let Some(ref file_path) = self.file_path {
            let text = self.text.clone();
            let file_path = file_path.clone();
            scheduler.spawn(move || {
                let text = strip_trailing_whitespace(text);
                text.write_to(BufWriter::new(File::create(&file_path)?))?;
                Ok(BufferTask::SaveFile { text })
            })?;
        }

        Ok(())
    }

    #[inline]
    fn ensure_cursor_in_view(&mut self, frame: &Rect) {
        let new_line = self.text.char_to_line(self.cursor.range().start.0);
        if new_line < self.first_line {
            self.first_line = new_line;
        } else if new_line - self.first_line > frame.size.height - 1 {
            self.first_line = new_line - frame.size.height + 1;
        }
    }

    #[inline]
    fn draw_line(
        &self,
        screen: &mut Screen,
        context: &Context,
        line_index: usize,
        line: RopeSlice,
        mut syntax_cursor: Option<&mut SyntaxCursor>,
        mut trace: &mut NodeTrace<SelectorNodeId>,
    ) -> usize {
        // Get references to the relevant bits of context
        let Context {
            ref frame,
            focused,
            theme: EditorTheme {
                buffer: ref theme, ..
            },
            ..
        } = *context;

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
            .mode
            .highlights()
            .and_then(|highlights| highlights.matches(&trace.trace, &trace.nth_children, &content))
            .map(|scope| scope.0.as_str());

        for grapheme in RopeGraphemes::new(&line.slice(..)) {
            let byte_index = self.text.char_to_byte(char_index.0);
            match (syntax_cursor.as_mut(), self.mode.highlights()) {
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
                screen.draw_rope_slice(visual_x, frame.origin.y, style, &grapheme);
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
    fn draw_text(&mut self, screen: &mut Screen, context: &Context) -> usize {
        self.ensure_cursor_in_view(&context.frame);
        let mut syntax_cursor = self.syntax.as_ref().and_then(|syntax| syntax.cursor());
        let mut trace: NodeTrace<SelectorNodeId> = NodeTrace::new();

        let mut visual_cursor_x = 0;
        for (line_index, line) in self
            .text
            .lines_at(self.first_line)
            .take(context.frame.size.height)
            .enumerate()
        {
            visual_cursor_x = cmp::max(
                visual_cursor_x,
                self.draw_line(
                    screen,
                    &context.set_frame(
                        context
                            .frame
                            .inner_rect(SideOffsets2D::new(line_index, 0, 0, 0)),
                    ),
                    self.first_line + line_index,
                    line,
                    syntax_cursor.as_mut(),
                    &mut trace,
                ),
            );
        }

        visual_cursor_x
    }

    #[inline]
    fn draw_line_info(&self, screen: &mut Screen, context: &Context) {
        for screen_index in 0..context.frame.size.height - 1 {
            screen.draw_str(
                context.frame.origin.x,
                context.frame.origin.y + screen_index as usize,
                context.theme.buffer.border,
                if self.first_line + screen_index < self.text.len_lines() - 1 {
                    " "
                } else {
                    "~"
                },
            );
        }
    }

    #[inline]
    fn draw_status_bar(&self, screen: &mut Screen, context: &Context, visual_cursor_x: usize) {
        let Context {
            ref frame,
            frame_id,
            focused,
            theme: EditorTheme {
                buffer: ref theme, ..
            },
            ..
        } = *context;
        let line_height = frame.origin.y + frame.size.height - 1;
        screen.clear_region(
            Rect::new(
                Position::new(frame.origin.x, line_height),
                Size::new(frame.size.width, 1),
            ),
            theme.status_base,
        );

        let mut offset = frame.origin.x;
        // Buffer number
        offset += screen.draw_str(
            offset,
            line_height,
            if focused {
                theme.status_frame_id_focused
            } else {
                theme.status_frame_id_unfocused
            },
            &format!(" {} ", frame_id),
        );

        // Has unsaved changes
        offset += screen.draw_str(
            offset,
            line_height,
            match self.has_unsaved_changes {
                ModifiedStatus::Unchanged => theme.status_is_not_modified,
                _ => theme.status_is_modified,
            },
            match self.has_unsaved_changes {
                ModifiedStatus::Unchanged => " - ",
                ModifiedStatus::Changed | ModifiedStatus::Saving(..) => " ☲ ",
                // ModifiedStatus::Saving(start_time) => [" | ", " / ", " - ", " \\ "]
                //     [((context.time - start_time).as_millis() / 250) as usize % 4],
            },
        );

        // File size
        offset += screen.draw_str(
            offset,
            line_height,
            theme.status_file_size,
            &format!(
                " {} ",
                SizeFormatterBinary::new(self.text.len_bytes() as u64)
            ),
        );

        // File name if buffer is backed by a file
        offset += screen.draw_str(
            offset,
            line_height,
            theme.status_file_name,
            &self
                .file_path
                .as_ref()
                .map(
                    |path| match path.file_name().and_then(|file_name| file_name.to_str()) {
                        Some(file_name) => format!("{} ", file_name),
                        None => format!("{} ", path.display()),
                    },
                )
                .unwrap_or_else(String::new),
        );

        // Name of the current mode
        screen.draw_str(
            offset,
            line_height,
            theme.status_mode,
            &format!(" {}", self.mode.name),
        );

        // Name of the current mode
        let reference = self.repo.as_ref().map(|repo| repo.head().unwrap());

        // The current position the file right-aligned
        let current_line = self.text.char_to_line(self.cursor.range().start.0);
        let num_lines = self.text.len_lines();
        let line_status = format!(
            "{}{current_line:>4}:{current_byte:>2} {percent:>3}% ",
            match reference
                .as_ref()
                .and_then(|reference| reference.shorthand())
            {
                Some(reference) => format!("{}  ", reference),
                None => String::new(),
            },
            current_line = current_line,
            current_byte = visual_cursor_x,
            percent = if num_lines > 0 {
                100 * (current_line + 1) / num_lines
            } else {
                100
            }
        );
        screen.draw_str(
            frame.origin.x + frame.size.width.saturating_sub(line_status.len()),
            line_height,
            theme.status_position_in_file,
            &line_status,
        );
    }

    fn center_visual_cursor(&mut self, frame: &Rect) {
        let line_index = self.text.char_to_line(self.cursor.range().start.0);
        if line_index >= frame.size.height / 2
            && self.first_line != line_index - frame.size.height / 2
        {
            self.first_line = line_index - frame.size.height / 2;
        } else if self.first_line != line_index {
            self.first_line = line_index;
        } else {
            self.first_line = 0;
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

#[derive(Clone, Debug)]
pub enum Action {
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
    DeleteForward,
    DeleteBackward,
    DeleteLine,
    Yank,
    CopySelection,
    CutSelection,
    InsertTab,
    InsertNewLine,
    InsertChar(char),
    Undo,

    // Buffer
    SaveBuffer,
}

lazy_static! {
    pub static ref HASH_BINDINGS: HashBindings<Action> = HashBindings::new(hashmap! {
        // Movement
        smallvec![Key::Ctrl('p')] => Action::Up,
        smallvec![Key::Up] => Action::Up,
        smallvec![Key::Ctrl('n')] => Action::Down,
        smallvec![Key::Down] => Action::Down,
        smallvec![Key::Ctrl('b')] => Action::Left,
        smallvec![Key::Left] => Action::Left,
        smallvec![Key::Ctrl('f')] => Action::Right,
        smallvec![Key::Right] => Action::Right,
        smallvec![Key::Ctrl('v')] => Action::PageDown,
        smallvec![Key::PageDown] => Action::PageDown,
        smallvec![Key::Alt('v')] => Action::PageUp,
        smallvec![Key::PageUp] => Action::PageUp,
        smallvec![Key::Ctrl('a')] => Action::StartOfLine,
        smallvec![Key::Home] => Action::StartOfLine,
        smallvec![Key::Ctrl('e')] => Action::EndOfLine,
        smallvec![Key::End] => Action::EndOfLine,
        smallvec![Key::Alt('<')] => Action::StartOfBuffer,
        smallvec![Key::Alt('>')] => Action::EndOfBuffer,
        smallvec![Key::Ctrl('l')] => Action::CenterCursorVisually,

        // Editing
        smallvec![Key::Null] => Action::BeginSelection,
        smallvec![Key::Ctrl('g')] => Action::ClearSelection,
        smallvec![Key::Alt('w')] => Action::CopySelection,
        smallvec![Key::Ctrl('w')] => Action::CutSelection,
        smallvec![Key::Ctrl('y')] => Action::Yank,
        smallvec![Key::Ctrl('d')] => Action::DeleteForward,
        smallvec![Key::Delete] => Action::DeleteForward,
        smallvec![Key::Backspace] => Action::DeleteBackward,
        smallvec![Key::Ctrl('k')] => Action::DeleteLine,
        smallvec![Key::Char('\n')] => Action::InsertNewLine,
        smallvec![Key::Char('\t')] => Action::InsertTab,
        smallvec![Key::Ctrl('/')] => Action::Undo,
        smallvec![Key::Ctrl('z')] => Action::Undo,

        // Buffer
        smallvec![Key::Ctrl('x'), Key::Ctrl('s')] => Action::SaveBuffer,
    });
}

pub struct BufferBindings;

impl Bindings<Action> for BufferBindings {
    fn matches(&self, pressed: &[Key]) -> BindingMatch<Action> {
        match pressed {
            [Key::Char(character)]
                if *character != '\n' && (!DISABLE_TABS || *character != '\t') =>
            {
                BindingMatch::Full(Action::InsertChar(*character))
            }
            pressed => HASH_BINDINGS.matches(pressed),
        }
    }
}

impl Component for Buffer {
    type Action = Action;
    type Bindings = BufferBindings;
    type TaskPayload = Result<BufferTask>;

    fn draw(&mut self, screen: &mut Screen, scheduler: &mut Scheduler, context: &Context) {
        {
            let Self {
                ref mut syntax,
                ref text,
                ..
            } = self;
            if let Some(syntax) = syntax.as_mut() {
                syntax
                    .ensure_tree(scheduler, || text.head().clone())
                    .unwrap();
            };
        }

        screen.clear_region(context.frame, context.theme.buffer.syntax.text);
        self.draw_line_info(screen, context);
        let visual_cursor_x = self.draw_text(
            screen,
            &context.set_frame(context.frame.inner_rect(SideOffsets2D::new(0, 0, 1, 1))),
        );
        self.draw_status_bar(screen, context, visual_cursor_x);
    }

    fn handle_action(
        &mut self,
        action: Self::Action,
        scheduler: &mut Scheduler,
        context: &Context,
    ) -> Result<()> {
        // Stateless
        match action {
            Action::Up => self.cursor.move_up(&self.text),
            Action::Down => self.cursor.move_down(&self.text),
            Action::Left => self.cursor.move_left(&self.text),
            Action::Right => self.cursor.move_right(&self.text),
            Action::PageDown => self
                .cursor
                .move_down_n(&self.text, context.frame.size.height - 1),
            Action::PageUp => self
                .cursor
                .move_up_n(&self.text, context.frame.size.height - 1),
            Action::StartOfLine => self.cursor.move_to_start_of_line(&self.text),
            Action::EndOfLine => self.cursor.move_to_end_of_line(&self.text),
            Action::StartOfBuffer => self.cursor.move_to_start_of_buffer(&self.text),
            Action::EndOfBuffer => self.cursor.move_to_end_of_buffer(&self.text),
            Action::CenterCursorVisually => self.center_visual_cursor(&context.frame),

            Action::BeginSelection => self.cursor.begin_selection(),
            Action::ClearSelection => self.cursor.clear_selection(),
            Action::SaveBuffer => self.spawn_save_file(scheduler, context)?,
            _ => {}
        };

        let mut undoing = false;
        let diff = match action {
            Action::DeleteForward => {
                let operation = self.cursor.delete(&mut self.text);
                self.clipboard = Some(operation.deleted);
                operation.diff
            }
            Action::DeleteBackward => self.cursor.backspace(&mut self.text).diff,
            Action::DeleteLine => self.delete_line(),
            Action::Yank => self.yank_line(),
            Action::CopySelection => self.copy_selection(),
            Action::CutSelection => self.cut_selection(),
            Action::InsertTab if DISABLE_TABS => {
                let diff = self
                    .cursor
                    .insert_chars(&mut self.text, iter::repeat(' ').take(TAB_WIDTH));
                self.cursor.move_right_n(&self.text, TAB_WIDTH);
                diff
            }
            Action::InsertNewLine => {
                let diff = self.cursor.insert_char(&mut self.text, '\n');
                // self.ensure_trailing_newline_with_content();
                self.cursor.move_down(&self.text);
                self.cursor.move_to_start_of_line(&self.text);
                diff
            }
            Action::Undo => {
                if let Some((diff, cursor)) = self.text.undo() {
                    undoing = true;
                    self.cursor = cursor;
                    if let Some(syntax) = self.syntax.as_mut() {
                        syntax.edit(&diff);
                        syntax.spawn_parse_task(scheduler, self.text.head().clone(), true)?;
                    }
                    diff
                } else {
                    OpaqueDiff::empty()
                }
            }
            Action::InsertChar(character) => {
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
                // eprintln!(
                //     "1: self.text.len_bytes() == {}  |  end_byte == {:?}  |  diff == {:?}",
                //     self.text.len_bytes(),
                //     syntax.tree.as_ref().map(|t| t.root_node().end_byte()),
                //     diff,
                // );
                syntax.edit(&diff);
                // eprintln!(
                //     "2: end_byte == {:?}",
                //     syntax.tree.as_ref().map(|t| t.root_node().end_byte()),
                // );
                syntax.spawn_parse_task(scheduler, self.text.head().clone(), false)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn task_done(&mut self, task: TaskDone<Self::TaskPayload>) -> Result<()> {
        let task_id = task.id;
        match task.payload? {
            BufferTask::SaveFile { text: new_text } => {
                self.cursor.sync(&self.text, &new_text);
                self.text
                    .new_revision(OpaqueDiff::empty(), self.cursor.clone());
                *self.text = new_text.clone();
                self.has_unsaved_changes = ModifiedStatus::Unchanged;
            }
            BufferTask::ParseSyntax(parsed) => {
                self.syntax
                    .as_mut()
                    .map(|syntax| syntax.handle_parse_syntax_done(task_id, parsed));
            }
        }
        Ok(())
    }

    fn bindings(&self) -> Option<&Self::Bindings> {
        Some(&self.bindings)
    }

    fn path(&self) -> Option<&Path> {
        self.file_path.as_ref().map(|path| path.as_path())
    }
}

const DISABLE_TABS: bool = false;
