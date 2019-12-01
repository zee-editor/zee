use euclid::default::SideOffsets2D;
use ropey::{Rope, RopeSlice};
use size_format::SizeFormatterBinary;
use std::{
    borrow::Cow,
    cmp,
    ffi::OsStr,
    fs::File,
    io::{self, BufReader, BufWriter},
    ops::Range,
    path::PathBuf,
    time::Instant,
};
use tree_sitter::{
    InputEdit as TreeSitterInputEdit, Node, Parser, Point as TreeSitterPoint, Tree, TreeCursor,
};

use super::{
    theme::{gruvbox, Theme as EditorTheme},
    Component, Context, Scheduler, TaskKind, TaskResult,
};
use crate::{
    error::Result,
    jobs::{JobId as TaskId, Poll},
    mode::{self, Mode},
    terminal::{Background, Colour, Foreground, Position, Rect, Screen, Size, Style},
    utils::{
        self, next_grapheme_boundary, prev_grapheme_boundary, strip_trailing_whitespace,
        RopeGraphemes, TAB_WIDTH,
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
    SaveFile { text: Rope },
    ParseSyntax(ParsedSyntax),
}

#[derive(Debug)]
pub struct ParsedSyntax {
    tree: Tree,
    text: Rope,
}

pub struct Buffer {
    mode: &'static Mode,
    text: Rope,
    yanked_line: Option<Rope>,
    has_unsaved_changes: ModifiedStatus,
    file_path: Option<PathBuf>,
    cursor: Cursor,
    first_line: usize,
    tree: Option<Tree>,
    parse_syntax_task_id: Option<TaskId>,
    parser: Option<Parser>,
}

impl Buffer {
    pub fn from_file(file_path: PathBuf) -> Result<Self> {
        let mode = mode::find_by_filename(&file_path);
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
            tree: None,
            parse_syntax_task_id: None,
            parser: mode.language.map(|language| {
                let mut parser = Parser::new();
                parser
                    .set_language(language)
                    .expect("Compatible tree sitter grammer version");
                parser
            }),
            mode,
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

    pub fn spawn_parse_syntax(&mut self, scheduler: &mut Scheduler) -> Result<TaskId> {
        let mode = self.mode;
        let text = self.text.clone();
        scheduler.spawn(move || {
            let mut parser = mode
                .language
                .map(|language| {
                    let mut parser = Parser::new();
                    parser
                        .set_language(language)
                        .expect("Compatible tree sitter grammer version");
                    parser
                })
                .unwrap();
            // let tree = parser
            //     .parse_with(
            //         &mut |byte_index, p| {
            //             eprintln!("p: {} {}", byte_index, p);
            //             if byte_index < text.len_bytes() {
            //                 text.chunk_at_byte(byte_index).0.as_bytes()
            //             } else {
            //                 &[]
            //             }
            //         },
            //         None,
            //     )
            //     .unwrap();

            let text_str: String = text.slice(..).into();
            let tree = parser.parse(text_str.as_bytes(), None).unwrap();

            Ok(TaskKind::Buffer(BufferTask::ParseSyntax(ParsedSyntax {
                tree,
                text,
            })))
        })
    }

    pub fn handle_parse_syntax_done(&mut self, task_id: TaskId, parsed: ParsedSyntax) {
        if self
            .parse_syntax_task_id
            .map(|expected_task_id| expected_task_id != task_id)
            .unwrap_or(true)
        {
            return;
        }

        let ParsedSyntax { tree, text } = parsed;

        self.tree = Some(tree.clone());
        let root = tree.root_node();
        assert!(root.end_byte() <= text.len_bytes() + 1);

        // eprintln!("SEXP: {:?}", tree.root_node().to_sexp());
        // let mut cursor = tree.walk();
        // eprintln!(" ** Recon **");
        // print_tree(&mut cursor, &text, 0);
        // eprintln!(" ** ***** **");
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
        node: Option<Node>,
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
            let is_error = node
                .map(|node| {
                    node.is_error()
                        || ["erroneous_end_tag_name"]
                            .into_iter()
                            .find(|error| node.kind() == **error)
                            .is_some()
                })
                .unwrap_or(false);

            let foreground = {
                if is_error {
                    Foreground(gruvbox::BRIGHT_RED)
                } else {
                    match node.map(|node| node.kind()).unwrap_or("") {
                        "erroneous_end_tag_name" | "ERROR" | "MISSING" => {
                            Foreground(gruvbox::BRIGHT_RED)
                        }
                        "tag_name" | "identifier" | "field_identifier" => {
                            Foreground(gruvbox::BRIGHT_BLUE)
                        }
                        // "tag_name" => Foreground(gruvbox::NEUTRAL_ORANGE),
                        "primitive_type" => Foreground(gruvbox::BRIGHT_YELLOW),
                        "attribute_name" | "=" => Foreground(gruvbox::FADED_YELLOW),
                        "\"" | "attribute_value" => Foreground(gruvbox::NEUTRAL_AQUA),
                        "<" | ">" | "/>" | "</" => Foreground(gruvbox::GRAY_245),
                        "fn" | "struct" | "fenced_code_block" => Foreground(gruvbox::FADED_YELLOW),
                        _ => theme.text.foreground,
                    }
                }
            };

            Style {
                background: if self.cursor.selection().contains(&char_index) {
                    theme.selection_background
                } else if line_under_cursor && focused {
                    theme.text_current_line.background
                } else {
                    theme.text.background
                },
                foreground,
                bold: node
                    .map(|node| node.kind() == "identifier")
                    .unwrap_or(false),
                underline: is_error
                    || node
                        .map(|node| node.kind() == "link_destination")
                        .unwrap_or(false),
                // bold: syntax_style.font_style.contains(SyntaxFontStyle::BOLD),
                // underline: syntax_style.font_style.contains(SyntaxFontStyle::UNDERLINE),
            }
        }
    }

    #[inline]
    fn draw_line(
        &self,
        screen: &mut Screen,
        context: &Context,
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

        // let line: Cow<str> = line.slice(..cmp::min(line.len_chars(), 1000)).into();
        // let ranges: Vec<(SyntaxStyle, &str)> = highlighter.highlight(&line, &self.syntax_set);

        let mut visual_x = frame.origin.x;
        let mut char_index = self.text.line_to_char(line_index);

        // let line = Rope::from(line);

        for grapheme in RopeGraphemes::new(&line.slice(..)) {
            if self.cursor.range.contains(&char_index) && focused {
                visual_cursor_x = visual_x.saturating_sub(frame.origin.x);
            }

            let maybe_node = match self.tree {
                Some(ref tree) => {
                    let byte_index = self.text.char_to_byte(char_index);
                    tree.root_node()
                        .descendant_for_byte_range(byte_index, byte_index + 1)
                        .and_then(|mut x| {
                            let initial = x;
                            if !x.is_error() {
                                while let Some(parent) = x.parent() {
                                    if parent.is_error() {
                                        return Some(parent);
                                    }
                                    x = parent;
                                }
                            }
                            Some(initial)
                        })
                }
                _ => None,
            };

            let style = self.text_style_at_char(context, char_index, line_under_cursor, maybe_node);
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
            &format!(" {}", self.mode.name),
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
    fn draw(&mut self, screen: &mut Screen, scheduler: &mut Scheduler, context: &Context) {
        match (&self.tree, &self.parse_syntax_task_id) {
            (None, None) if self.mode.language.is_some() => {
                self.parse_syntax_task_id = Some(self.spawn_parse_syntax(scheduler).unwrap());
            }
            _ => {}
        };

        screen.clear_region(context.frame, context.theme.buffer.text);

        self.draw_line_info(screen, context);
        let visual_cursor_x = self.draw_text(
            screen,
            &context.set_frame(context.frame.inner_rect(SideOffsets2D::new(0, 0, 1, 1))),
        );
        self.draw_status_bar(screen, context, visual_cursor_x);
    }

    #[inline]
    fn handle_event(
        &mut self,
        key: Key,
        scheduler: &mut Scheduler,
        context: &Context,
    ) -> Result<()> {
        // Stateless
        match key {
            Key::Ctrl('p') | Key::Up => {
                self.cursor_up();
            }
            Key::Ctrl('n') | Key::Down => {
                self.cursor_down();
            }
            Key::Ctrl('b') | Key::Left => {
                self.cursor_left();
            }
            Key::Ctrl('f') | Key::Right => {
                self.cursor_right();
            }
            Key::Ctrl('v') | Key::PageDown => {
                for _ in 0..(context.frame.size.height - 1) {
                    self.cursor_down();
                }
            }
            Key::Alt('v') | Key::PageUp => {
                for _ in 0..(context.frame.size.height - 1) {
                    self.cursor_up();
                }
            }
            Key::Ctrl('a') | Key::Home => {
                self.cursor_start_of_line();
            }
            Key::Ctrl('e') | Key::End => {
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
            _ => {}
        };

        let mut text_changed = true;

        let cursor_end = cmp::min(self.cursor.range.end, self.text.len_chars());
        let initial_cursor =
            self.text.char_to_byte(self.cursor.range.start)..self.text.char_to_byte(cursor_end);
        let initial_point_start = TreeSitterPoint {
            row: self.text.char_to_line(self.cursor.range.start),
            column: self.text.char_to_byte(self.cursor.range.start)
                - self
                    .text
                    .line_to_byte(self.text.char_to_line(self.cursor.range.start)),
        };
        let initial_point_end = TreeSitterPoint {
            row: self.text.char_to_line(cursor_end),
            column: self.text.char_to_byte(cursor_end)
                - self.text.line_to_byte(self.text.char_to_line(cursor_end)),
        };

        match key {
            Key::Ctrl('d') | Key::Delete => {
                self.delete();
            }
            Key::Ctrl('k') => {
                self.delete_line();
            }
            Key::Ctrl('y') => {
                self.yank_line();
            }
            Key::Backspace => {
                if self.cursor.range.start > 0 {
                    self.cursor_left();
                    self.delete();
                }
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
                self.spawn_save_file(scheduler, context)?;
            }
            _ => {
                text_changed = false;
            }
        }

        let cursor_end = cmp::min(self.cursor.range.end, self.text.len_chars());
        let final_cursor =
            self.text.char_to_byte(self.cursor.range.start)..self.text.char_to_byte(cursor_end);
        let final_point_start = TreeSitterPoint {
            row: self.text.char_to_line(self.cursor.range.start),
            column: self.text.char_to_byte(self.cursor.range.start)
                - self
                    .text
                    .line_to_byte(self.text.char_to_line(self.cursor.range.start)),
        };
        let final_point_end = TreeSitterPoint {
            row: self.text.char_to_line(cursor_end),
            column: self.text.char_to_byte(cursor_end)
                - self.text.line_to_byte(self.text.char_to_line(cursor_end)),
        };

        if text_changed && self.mode.language.is_some() {
            let changes = TreeSitterInputEdit {
                start_byte: initial_cursor.start,
                old_end_byte: initial_cursor.end,
                new_end_byte: final_cursor.end,
                start_position: initial_point_start,
                old_end_position: initial_point_end,
                new_end_position: final_point_end,
            };
            // eprintln!("changes: {:?}", changes);

            match self.tree {
                Some(ref mut tree) => {
                    tree.edit(&changes);
                }
                _ => {}
            };
            self.parse_syntax_task_id = Some(self.spawn_parse_syntax(scheduler)?);
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
                self.handle_parse_syntax_done(task_id, parsed)
            }
        }

        Ok(())
    }
}

fn print_tree(cursor: &mut TreeCursor, text: &Rope, mut line_num: usize) -> usize {
    if cursor.goto_first_child() {
        line_num = print_tree(cursor, text, line_num);
    } else {
        let new_line_num = text.byte_to_line(cursor.node().start_byte());
        assert!(new_line_num >= line_num);
        if new_line_num > line_num {
            eprintln!("");
            line_num = new_line_num;
        }

        let range = text.byte_to_char(cursor.node().start_byte())
            ..text.byte_to_char(cmp::min(cursor.node().end_byte(), text.len_bytes()));
        let range_str: Cow<str> = text.slice(range).into();
        // eprint!("{} ", range_str);
        eprint!("{}[{}] ", range_str, cursor.node().kind());
    }

    if cursor.goto_next_sibling() {
        line_num = print_tree(cursor, text, line_num);
    } else {
        cursor.goto_parent();
    }

    line_num
}

const DISABLE_TABS: bool = false;
