use euclid::default::SideOffsets2D;
use git2::Repository;
use ropey::{Rope, RopeSlice};
use size_format::SizeFormatterBinary;
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
    cursor::Cursor, theme::Theme as EditorTheme, Component, Context, Scheduler, TaskKind,
    TaskResult,
};
use crate::{
    error::Result,
    mode::{self, Mode},
    syntax::{
        highlight::{text_style_at_char, Theme as SyntaxTheme},
        parse::{NodeTrace, OpaqueDiff, ParserStatus, SyntaxCursor, SyntaxTree},
    },
    terminal::{Key, Position, Rect, Screen, Size, Style},
    utils::{
        self, next_grapheme_boundary, prev_grapheme_boundary, strip_trailing_whitespace,
        RopeGraphemes, TAB_WIDTH,
    },
};

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
    text: Rope,
    yanked_line: Option<Rope>,
    has_unsaved_changes: ModifiedStatus,
    file_path: Option<PathBuf>,
    cursor: Cursor,
    first_line: usize,
    syntax: Option<SyntaxTree>,
    repo: Option<Repository>,
}

impl Buffer {
    pub fn from_file(file_path: PathBuf) -> Result<Self> {
        let mode = mode::find_by_filename(&file_path);
        let repo = Repository::discover(&file_path).ok();
        Ok(Buffer {
            text: if file_path.exists() {
                Rope::from_reader(BufReader::new(File::open(&file_path)?))?
            } else {
                // Optimistically check we can create it
                File::open(&file_path).map(|_| ()).or_else(|error| {
                    if error.kind() == io::ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(error)
                    }
                })?;
                Rope::new()
            },
            yanked_line: None,
            has_unsaved_changes: ModifiedStatus::Unchanged,
            file_path: Some(file_path),
            cursor: Cursor::new(),
            first_line: 0,
            syntax: mode.language().map(|language| SyntaxTree::new(*language)),
            mode,
            repo,
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
                Ok(TaskKind::Buffer(BufferTask::SaveFile { text }))
            })?;
        }

        Ok(())
    }

    #[inline]
    fn ensure_cursor_in_view(&mut self, frame: &Rect) {
        let new_line = self.text.char_to_line(self.cursor.range.start);
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
        let line_under_cursor = self.text.char_to_line(self.cursor.range.start) == line_index;
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
        let mut char_index = self.text.line_to_char(line_index);

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
            let byte_index = self.text.char_to_byte(char_index);
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

            if self.cursor.range.contains(&char_index) && focused {
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

            char_index += grapheme.len_chars();
            visual_x += grapheme_width;
        }

        if line_index == self.text.len_lines() - 1
            && self.cursor.range.start == self.text.len_chars()
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
        let current_line = self.text.char_to_line(self.cursor.range.start);
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
        let line_index = self.text.char_to_line(self.cursor.range.start);
        if line_index >= frame.size.height / 2
            && self.first_line != line_index - frame.size.height / 2
        {
            self.first_line = line_index - frame.size.height / 2;
        } else if self.first_line != line_index {
            self.first_line = line_index;
        } else {
            self.first_line = 0;
        }

        // In Emacs C-L is a 3 state ting
        // else if self.text.len_lines() > frame.size.height {
        //     self.first_line = line_index - frame.size.height;
        // }
    }

    fn delete_line(&mut self) -> OpaqueDiff {
        if self.text.len_chars() == 0 {
            return OpaqueDiff::no_diff();
        }

        // Delete line
        self.has_unsaved_changes = ModifiedStatus::Changed;
        let line_index = self.text.char_to_line(self.cursor.range.start);
        let delete_range_start = self.text.line_to_char(line_index);
        let delete_range_end = self.text.line_to_char(line_index + 1);
        self.yanked_line = Some(Rope::from(
            self.text.slice(delete_range_start..delete_range_end),
        ));
        self.text.remove(delete_range_start..delete_range_end);
        let diff = OpaqueDiff::new(delete_range_start, delete_range_end - delete_range_start, 0);

        // Place cursor
        let grapheme_start = self.text.line_to_char(cmp::min(
            line_index,
            self.text.len_lines().saturating_sub(2),
        ));
        let grapheme_end = next_grapheme_boundary(&self.text.slice(..), grapheme_start);
        if grapheme_start != grapheme_end {
            self.cursor.range = grapheme_start..grapheme_end
        } else {
            self.cursor.range = 0..1
        }
        self.cursor.visual_horizontal_offset = None;

        diff
    }

    fn yank_line(&mut self) -> OpaqueDiff {
        if let Some(clipboard) = self.yanked_line.as_ref() {
            self.has_unsaved_changes = ModifiedStatus::Changed;
            let diff = OpaqueDiff::new(self.cursor.range.start, 0, clipboard.len_bytes());
            let mut cursor_start = self.cursor.range.start;
            for chunk in clipboard.chunks() {
                self.text.insert(cursor_start, chunk);
                cursor_start += chunk.chars().count();
                self.cursor.range =
                    cursor_start..next_grapheme_boundary(&self.text.slice(..), cursor_start);
            }
            return diff;
        }
        OpaqueDiff::no_diff()
    }

    fn copy_selection(&mut self) -> OpaqueDiff {
        self.yanked_line = Some(Rope::from(self.text.slice(self.cursor.selection())));
        self.cursor.select = None;
        OpaqueDiff::no_diff()
    }

    fn cut_selection(&mut self) -> OpaqueDiff {
        let selection = self.cursor.selection();
        let diff = OpaqueDiff::new(selection.start, selection.end - selection.start, 0);
        self.has_unsaved_changes = ModifiedStatus::Changed;
        self.yanked_line = Some(Rope::from(self.text.slice(selection)));
        self.text.remove(self.cursor.selection());
        self.cursor.select = None;

        let grapheme_start = cmp::min(
            self.cursor.range.start,
            prev_grapheme_boundary(&self.text.slice(..), self.text.len_chars()),
        );
        let grapheme_end = next_grapheme_boundary(&self.text.slice(..), grapheme_start);
        if grapheme_start != grapheme_end {
            self.cursor.range = grapheme_start..grapheme_end
        } else {
            self.cursor.range = 0..1
        }
        self.cursor.visual_horizontal_offset = None;
        diff
    }

    fn ensure_trailing_newline_with_content(&mut self) {
        if self.text.len_chars() == 0 || self.text.char(self.text.len_chars() - 1) != '\n' {
            self.text.insert_char(self.text.len_chars(), '\n');
        }
    }
}

impl Component for Buffer {
    fn draw(&mut self, screen: &mut Screen, scheduler: &mut Scheduler, context: &Context) {
        {
            let Self {
                ref mut syntax,
                ref text,
                ..
            } = self;
            if let Some(syntax) = syntax.as_mut() {
                syntax.ensure_tree(scheduler, || text.clone()).unwrap();
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

    fn handle_event(
        &mut self,
        key: Key,
        scheduler: &mut Scheduler,
        context: &Context,
    ) -> Result<()> {
        // Stateless
        match key {
            Key::Ctrl('p') | Key::Up => {
                self.cursor.move_up(&self.text);
            }
            Key::Ctrl('n') | Key::Down => {
                self.cursor.move_down(&self.text);
            }
            Key::Ctrl('b') | Key::Left => {
                self.cursor.move_left(&self.text);
            }
            Key::Ctrl('f') | Key::Right => {
                self.cursor.move_right(&self.text);
            }
            Key::Ctrl('v') | Key::PageDown => {
                for _ in 0..(context.frame.size.height - 1) {
                    self.cursor.move_down(&self.text);
                }
            }
            Key::Alt('v') | Key::PageUp => {
                for _ in 0..(context.frame.size.height - 1) {
                    self.cursor.move_up(&self.text);
                }
            }
            Key::Ctrl('a') | Key::Home => {
                self.cursor.move_to_start_of_line(&self.text);
            }
            Key::Ctrl('e') | Key::End => {
                self.cursor.move_to_end_of_line(&self.text);
            }
            Key::Alt('<') => {
                self.cursor.move_to_start_of_buffer(&self.text);
            }
            Key::Alt('>') => {
                self.cursor.move_to_end_of_buffer(&self.text);
            }
            Key::Ctrl('l') => self.center_visual_cursor(&context.frame),

            Key::Null => self.cursor.select = Some(self.cursor.range.start),
            Key::Ctrl('g') => self.cursor.select = None,
            Key::Alt('s') => {
                self.spawn_save_file(scheduler, context)?;
            }
            _ => {}
        };

        let diff = match key {
            Key::Ctrl('d') | Key::Delete => self.cursor.delete(&mut self.text),
            Key::Ctrl('k') => self.delete_line(),
            Key::Ctrl('y') => self.yank_line(),
            Key::Backspace => self.cursor.backspace(&mut self.text),
            Key::Alt('w') => self.copy_selection(),
            Key::Ctrl('w') => self.cut_selection(),
            Key::Char('\t') if DISABLE_TABS => {
                let diff = self
                    .cursor
                    .insert_chars(&mut self.text, iter::repeat(' ').take(TAB_WIDTH));
                self.cursor.move_right_n(&self.text, TAB_WIDTH);
                diff
            }
            Key::Char('\n') => {
                let diff = self.cursor.insert_char(&mut self.text, '\n');
                self.ensure_trailing_newline_with_content();
                self.cursor.move_down(&self.text);
                self.cursor.move_to_start_of_line(&self.text);
                diff
            }
            Key::Char(character) => {
                let diff = self.cursor.insert_char(&mut self.text, character);
                self.ensure_trailing_newline_with_content();
                self.cursor.move_right(&self.text);
                diff
            }
            _ => OpaqueDiff::no_diff(),
        };

        if !diff.is_empty() {
            self.has_unsaved_changes = ModifiedStatus::Changed;
        }

        match self.syntax.as_mut() {
            Some(syntax) if !diff.is_empty() => {
                syntax.edit(&diff);
                let text = self.text.clone();
                syntax.spawn_parse_task(scheduler, text)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn task_done(&mut self, task_result: TaskResult) -> Result<()> {
        let task_id = task_result.id;
        let payload = task_result.payload?;
        match payload {
            TaskKind::Buffer(BufferTask::SaveFile { text: new_text }) => {
                let current_line = self.text.char_to_line(self.cursor.range.start);
                let current_line_offset =
                    self.cursor.range.start - self.text.line_to_char(current_line);

                self.cursor = {
                    let new_line = cmp::min(current_line, new_text.len_lines().saturating_sub(1));
                    let new_line_offset = cmp::min(
                        current_line_offset,
                        new_text.line(new_line).len_chars().saturating_sub(1),
                    );
                    let grapheme_end = next_grapheme_boundary(
                        &new_text.slice(..),
                        new_text.line_to_char(new_line) + new_line_offset,
                    );
                    let grapheme_start = prev_grapheme_boundary(&new_text.slice(..), grapheme_end);
                    Cursor {
                        range: if grapheme_start != grapheme_end {
                            grapheme_start..grapheme_end
                        } else {
                            0..1
                        },
                        visual_horizontal_offset: None,
                        select: None,
                    }
                };

                self.text = new_text.clone();
                self.has_unsaved_changes = ModifiedStatus::Unchanged;
            }
            TaskKind::Buffer(BufferTask::ParseSyntax(parsed)) => {
                self.syntax
                    .as_mut()
                    .map(|syntax| syntax.handle_parse_syntax_done(task_id, parsed));
            }
        }

        Ok(())
    }

    fn path(&self) -> Option<&Path> {
        self.file_path.as_ref().map(|path| path.as_path())
    }
}

const DISABLE_TABS: bool = false;
