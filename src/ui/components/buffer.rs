use euclid::default::SideOffsets2D;
use ropey::{Rope, RopeSlice};
use size_format::SizeFormatterBinary;
use std::{
    cmp,
    ffi::OsStr,
    fs::File,
    io::{self, BufReader, BufWriter},
    ops::Range,
    path::PathBuf,
    time::Instant,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    FontStyle as SyntaxFontStyle, Style as SyntaxStyle, Theme as SyntaxTheme,
};
use syntect::parsing::{SyntaxReference, SyntaxSet, SyntaxSetBuilder};

use super::{theme::Theme as EditorTheme, Component, ComponentTask, Context};
use crate::{
    error::{Error, Result},
    ui::{Background, Colour, Foreground, Position, Rect, Screen, Size, Style},
    utils::{
        self, next_grapheme_boundary, prev_grapheme_boundary, rope_as_str,
        strip_trailing_whitespace, RopeGraphemes, TAB_WIDTH,
    },
};
use termion::event::Key;

#[derive(Debug)]
pub struct Cursor {
    range: Range<usize>, // char range under cursor
    visual_horizontal_offset: Option<usize>,
    select: Option<usize>,
}

impl Cursor {
    fn selection(&self) -> Range<usize> {
        match self.select {
            Some(select) if select > self.range.start => self.range.start..select,
            Some(select) if select < self.range.start => select..self.range.start,
            _ => self.range.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub text: Style,
    pub text_current_line: Style,
    pub border: Style,
    pub cursor_focused: Style,
    pub cursor_unfocused: Style,
    pub selection_background: Background,
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

#[derive(Debug)]
pub enum BufferTask {
    Save { text: Rope },
}

pub struct Buffer {
    text: Rope,
    yanked_line: Option<Rope>,
    has_unsaved_changes: ModifiedStatus,
    file_path: Option<PathBuf>,
    cursor: Cursor,
    first_line: usize,
    syntax_set: SyntaxSet,
    syntax_reference: SyntaxReference,
    syntax_theme: SyntaxTheme,
}

impl Buffer {
    fn highlighter(&self) -> HighlightLines {
        // Highlighter::new(&self.syntax_theme)
        HighlightLines::new(&self.syntax_reference, &self.syntax_theme)
    }

    pub fn from_file(
        file_path: PathBuf,
        syntax_set: SyntaxSet,
        syntax_theme: SyntaxTheme,
    ) -> Result<Self> {
        let syntax_reference = file_path
            .extension()
            .and_then(OsStr::to_str)
            .and_then(|extension| syntax_set.find_syntax_by_extension(extension))
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
            .clone();

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
            cursor: Cursor {
                range: 0..1,
                visual_horizontal_offset: None,
                select: None,
            },
            first_line: 0,
            syntax_set,
            syntax_reference,
            syntax_theme,
        })
    }

    pub fn save(&mut self, context: &Context) -> Result<()> {
        self.has_unsaved_changes = ModifiedStatus::Saving(context.time);
        if let Some(ref file_path) = self.file_path {
            let text = self.text.clone();
            let file_path = file_path.clone();
            context.job_pool.spawn(move || {
                let text = strip_trailing_whitespace(text);
                text.write_to(BufWriter::new(File::create(&file_path)?))?;
                Ok(ComponentTask::Buffer(BufferTask::Save { text }))
            });
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
    fn text_style_at_char(
        &self,
        context: &Context,
        char_index: usize,
        line_under_cursor: bool,
        syntax_style: SyntaxStyle,
    ) -> Style {
        let Context {
            focused,
            theme: EditorTheme {
                buffer: ref theme, ..
            },
            ..
        } = *context;
        if self.cursor.range.contains(&char_index) {
            if focused {
                theme.cursor_focused
            } else {
                theme.cursor_unfocused
            }
        } else {
            Style {
                background: if self.cursor.selection().contains(&char_index) {
                    theme.selection_background
                } else if line_under_cursor && focused {
                    theme.text_current_line.background
                } else {
                    theme.text.background
                },
                foreground: Foreground(Colour::from(syntax_style.foreground)),
                bold: syntax_style.font_style.contains(SyntaxFontStyle::BOLD),
                underline: syntax_style.font_style.contains(SyntaxFontStyle::UNDERLINE),
            }
        }
    }

    #[inline]
    fn draw_line(
        &self,
        screen: &mut Screen,
        context: &Context,
        highlighter: &mut HighlightLines,
        line_index: usize,
        line: RopeSlice,
    ) -> usize {
        let Context {
            ref frame,
            focused,
            theme: EditorTheme {
                buffer: ref theme, ..
            },
            ..
        } = *context;

        let mut visual_cursor_x = 0;
        let line_under_cursor = self.text.char_to_line(self.cursor.range.start) == line_index;
        if line_under_cursor && focused {
            screen.clear_region(
                Rect::new(
                    Position::new(frame.origin.x, frame.origin.y),
                    Size::new(frame.size.width, 1),
                ),
                theme.text_current_line,
            );
        }

        let mut visual_x = frame.origin.x;

        let line = rope_as_str(&line);
        let ranges: Vec<(SyntaxStyle, &str)> = highlighter.highlight(&line, &self.syntax_set);

        let mut char_index = self.text.line_to_char(line_index);

        for (syntax_style, line) in ranges {
            let line = Rope::from(line);

            for grapheme in RopeGraphemes::new(&line.slice(..)) {
                if self.cursor.range.contains(&char_index) && focused {
                    visual_cursor_x = visual_x.saturating_sub(frame.origin.x);
                }

                let style =
                    self.text_style_at_char(context, char_index, line_under_cursor, syntax_style);
                let mut grapheme_width = utils::grapheme_width(&grapheme);
                let horizontal_bounds_inclusive = frame.min_x()..=frame.max_x();
                if !horizontal_bounds_inclusive.contains(&(visual_x + grapheme_width)) {
                    break;
                }

                if grapheme == "\t" {
                    grapheme_width = 4;
                    screen.draw_str(visual_x, frame.origin.y, style, "    ");
                } else if grapheme_width == 0 {
                    screen.draw_str(visual_x, frame.origin.y, style, " ");
                } else {
                    screen.draw_rope_slice(visual_x, frame.origin.y, style, &grapheme);
                }

                char_index += grapheme.len_chars();
                visual_x += grapheme_width;
            }
        }

        if line_index == self.text.len_lines() - 1
            && self.cursor.range.start == self.text.len_chars()
        {
            screen.draw_str(
                frame.origin.x,
                frame.origin.y,
                if focused {
                    theme.cursor_focused
                } else {
                    theme.cursor_unfocused
                },
                " ",
            );
        }

        visual_cursor_x
    }

    #[inline]
    fn draw_text(&mut self, screen: &mut Screen, context: &Context) -> usize {
        self.ensure_cursor_in_view(&context.frame);

        let mut highlighter = self.highlighter();
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
                    &mut highlighter,
                    self.first_line + line_index,
                    line,
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
                ModifiedStatus::Changed | ModifiedStatus::Saving(..) => " * ",
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
                .map(|path| format!("{} ", path.display()))
                .unwrap_or_else(String::new),
        );

        // Name of the current mode
        screen.draw_str(
            offset,
            line_height,
            theme.status_mode,
            &format!(" {}", self.syntax_reference.name),
        );

        // The current position the file right-aligned
        let current_line = self.text.char_to_line(self.cursor.range.start);
        let num_lines = self.text.len_lines();
        let line_status = format!(
            " {current_line}:{current_byte:>2} {percent:>3}% ",
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

    fn cursor_up(&mut self) {
        let current_line_index = self.text.char_to_line(self.cursor.range.start);
        if current_line_index == 0 {
            return;
        }
        self.move_cursor_vertically(current_line_index, current_line_index - 1);

        if current_line_index - 1 < self.first_line {
            self.first_line -= 1;
        }
    }

    fn cursor_down(&mut self) {
        let current_line_index = self.text.char_to_line(self.cursor.range.start);
        if current_line_index >= self.text.len_lines() {
            return;
        }
        self.move_cursor_vertically(current_line_index, current_line_index + 1);
    }

    fn cursor_left(&mut self) {
        let previous_grapheme_start =
            prev_grapheme_boundary(&self.text.slice(..), self.cursor.range.start);
        if previous_grapheme_start == 0 && self.cursor.range.start == 0 {
            return;
        }

        self.cursor.range = previous_grapheme_start..self.cursor.range.start;
        self.cursor.visual_horizontal_offset = None;
    }

    fn cursor_right(&mut self) {
        let grapheme_start = self.cursor.range.end;
        let grapheme_end = next_grapheme_boundary(&self.text.slice(..), self.cursor.range.end);
        if grapheme_start != grapheme_end {
            self.cursor.range = grapheme_start..grapheme_end;
        }
        self.cursor.visual_horizontal_offset = None;
    }

    fn cursor_start_of_line(&mut self) {
        let line_index = self.text.char_to_line(self.cursor.range.start);
        let char_index = self.text.line_to_char(line_index);
        self.cursor.range = char_index..next_grapheme_boundary(&self.text.slice(..), char_index);
        self.cursor.visual_horizontal_offset = None;
    }

    fn cursor_end_of_line(&mut self) {
        let line_index = self.text.char_to_line(self.cursor.range.start);
        let char_index = self.text.line_to_char(line_index);
        let line_len = self.text.line(line_index).len_chars();
        self.cursor.range = (char_index + line_len).saturating_sub(1)..char_index + line_len;
        self.cursor.visual_horizontal_offset = None;
    }

    fn cursor_start_of_buffer(&mut self) {
        self.cursor.range = 0..next_grapheme_boundary(&self.text.slice(..), 0);
        self.cursor.visual_horizontal_offset = None;
    }

    fn cursor_end_of_buffer(&mut self) {
        let len_chars = self.text.len_chars();
        self.cursor.range = prev_grapheme_boundary(&self.text.slice(..), len_chars)..len_chars;
        self.cursor.visual_horizontal_offset = None;
    }

    fn center_visual_cursor(&mut self, frame: &Rect) {
        let line_index = self.text.char_to_line(self.cursor.range.start);
        if line_index >= frame.size.height / 2 {
            let middle_vertical = line_index - frame.size.height / 2;

            if self.first_line != middle_vertical {
                self.first_line = middle_vertical;
            } else if self.first_line != line_index {
                self.first_line = line_index;
            }

            // In Emacs C-L is a 3 state ting
            // else if self.text.len_lines() > frame.size.height {
            //     self.first_line = line_index - frame.size.height;
            // }
        }
    }

    fn insert_char(&mut self, character: char) {
        self.has_unsaved_changes = ModifiedStatus::Changed;
        self.text.insert_char(self.cursor.range.start, character);
    }

    fn insert_newline(&mut self) {
        self.has_unsaved_changes = ModifiedStatus::Changed;
        self.text.insert_char(self.cursor.range.start, '\n');
    }

    fn delete(&mut self) {
        if self.text.len_chars() == 0 {
            return;
        }

        self.has_unsaved_changes = ModifiedStatus::Changed;
        self.text
            .remove(self.cursor.range.start..self.cursor.range.end);
        let grapheme_start = self.cursor.range.start;
        let grapheme_end = next_grapheme_boundary(&self.text.slice(..), self.cursor.range.start);
        if grapheme_start < grapheme_end {
            self.cursor.range = grapheme_start..grapheme_end
        } else {
            self.cursor.range = 0..1
        }
    }

    fn delete_line(&mut self) {
        if self.text.len_chars() == 0 {
            return;
        }
        self.has_unsaved_changes = ModifiedStatus::Changed;
        let line_index = self.text.char_to_line(self.cursor.range.start);
        let start = self.text.line_to_char(line_index);

        self.yanked_line = Some(Rope::from(
            self.text
                .slice(start..self.text.line_to_char(line_index + 1)),
        ));
        self.text
            .remove(start..self.text.line_to_char(line_index + 1));

        let grapheme_start = self
            .text
            .line_to_char(cmp::min(line_index, self.text.len_lines() - 2));
        let grapheme_end = next_grapheme_boundary(&self.text.slice(..), grapheme_start);
        if grapheme_start != grapheme_end {
            self.cursor.range = grapheme_start..grapheme_end
        } else {
            self.cursor.range = 0..1
        }
        self.cursor.visual_horizontal_offset = None;
    }

    fn yank_line(&mut self) {
        if let Some(clipboard) = self.yanked_line.as_ref() {
            self.has_unsaved_changes = ModifiedStatus::Changed;
            let mut cursor_start = self.cursor.range.start;
            for chunk in clipboard.chunks() {
                self.text.insert(cursor_start, chunk);
                cursor_start += chunk.chars().count();
                self.cursor.range =
                    cursor_start..next_grapheme_boundary(&self.text.slice(..), cursor_start);
            }
        }
    }

    fn copy_selection(&mut self) {
        self.yanked_line = Some(Rope::from(self.text.slice(self.cursor.selection())));
        self.cursor.select = None;
    }

    fn cut_selection(&mut self) {
        self.has_unsaved_changes = ModifiedStatus::Changed;
        self.yanked_line = Some(Rope::from(self.text.slice(self.cursor.selection())));
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
    }

    fn move_cursor_vertically(&mut self, current_line_index: usize, new_line_index: usize) {
        if new_line_index >= self.text.len_lines() {
            return;
        }

        let Self {
            ref text,
            cursor:
                Cursor {
                    range: ref mut cursor_range,
                    ref mut visual_horizontal_offset,
                    ..
                },
            ..
        } = *self;

        let current_line_start = text.line_to_char(current_line_index);
        let current_visual_x = visual_horizontal_offset.get_or_insert_with(|| {
            utils::grapheme_width(&text.slice(current_line_start..cursor_range.start))
        });

        let new_line = text.line(new_line_index);
        let mut graphemes = RopeGraphemes::new(&new_line);
        let mut new_visual_x = 0;
        while let Some(grapheme) = graphemes.next() {
            let width = utils::grapheme_width(&grapheme);
            if new_visual_x + width > *current_visual_x {
                break;
            }
            new_visual_x += width;
        }

        let new_line_offset =
            text.byte_to_char(text.line_to_byte(new_line_index) + graphemes.cursor.cur_cursor());

        if new_visual_x <= *current_visual_x {
            let grapheme_start = prev_grapheme_boundary(&text.slice(..), new_line_offset);
            let grapheme_end = next_grapheme_boundary(&text.slice(..), grapheme_start);
            *cursor_range = grapheme_start..grapheme_end;
        } else {
            let grapheme_end = next_grapheme_boundary(&text.slice(..), new_line_offset);
            let grapheme_start = prev_grapheme_boundary(&text.slice(..), grapheme_end);
            *cursor_range = grapheme_start..grapheme_end;
        }
    }

    fn ensure_trailing_newline_with_content(&mut self) {
        if self.text.len_chars() == 0 || self.text.char(self.text.len_chars() - 1) != '\n' {
            self.text.insert_char(self.text.len_chars(), '\n');
        }
    }
}

impl Component for Buffer {
    #[inline]
    fn draw(&mut self, screen: &mut Screen, context: &Context) {
        screen.clear_region(context.frame, context.theme.buffer.text);

        self.draw_line_info(screen, context);
        let visual_cursor_x = self.draw_text(
            screen,
            &context.set_frame(context.frame.inner_rect(SideOffsets2D::new(0, 0, 1, 1))),
        );
        self.draw_status_bar(screen, context, visual_cursor_x);
    }

    #[inline]
    fn key_press(&mut self, key: Key, context: &Context) -> Result<()> {
        match key {
            Key::Ctrl('p') => {
                self.cursor_up();
            }
            Key::Ctrl('n') => {
                self.cursor_down();
            }
            Key::Ctrl('b') => {
                self.cursor_left();
            }
            Key::Ctrl('f') => {
                self.cursor_right();
            }
            Key::Ctrl('v') => {
                for _ in 0..(context.frame.size.height - 1) {
                    self.cursor_down();
                }
            }
            Key::Alt('v') => {
                for _ in 0..(context.frame.size.height - 1) {
                    self.cursor_up();
                }
            }
            Key::Ctrl('a') => {
                self.cursor_start_of_line();
            }
            Key::Ctrl('e') => {
                self.cursor_end_of_line();
            }
            Key::Ctrl('l') => {
                self.center_visual_cursor(&context.frame);
            }
            Key::Alt('<') => {
                self.cursor_start_of_buffer();
            }
            Key::Alt('>') => {
                self.cursor_end_of_buffer();
            }
            Key::Ctrl('d') => {
                self.delete();
            }
            Key::Ctrl('k') => {
                self.delete_line();
            }
            Key::Ctrl('y') => {
                self.yank_line();
            }
            Key::Backspace => {
                self.cursor_left();
                self.delete();
            }
            Key::Null => {
                self.cursor.select = Some(self.cursor.range.start);
            }
            Key::Alt('w') => self.copy_selection(),
            Key::Ctrl('w') => self.cut_selection(),
            Key::Ctrl('g') => {
                self.cursor.select = None;
            }
            Key::Char('\t') if DISABLE_TABS => {
                for _ in 0..TAB_WIDTH {
                    self.insert_char(' ');
                    self.cursor_right();
                }
            }
            Key::Char('\n') => {
                self.insert_newline();
                self.ensure_trailing_newline_with_content();
                self.cursor_down();
                self.cursor_start_of_line();
            }
            Key::Char(character) => {
                self.insert_char(character);
                self.ensure_trailing_newline_with_content();
                self.cursor_right();
            }
            Key::Alt('s') => {
                self.save(context)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn task_done(&mut self, task: &ComponentTask) -> Result<()> {
        match task {
            ComponentTask::Buffer(BufferTask::Save { text: new_text }) => {
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
        }

        Ok(())
    }
}

const DISABLE_TABS: bool = false;
