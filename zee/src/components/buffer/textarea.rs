use euclid::default::SideOffsets2D;
use ropey::{Rope, RopeSlice};
use std::{iter, ops::Range};
use zi::{
    terminal::GraphemeCluster, Canvas, Component, ComponentLink, Layout, Position, Rect,
    ShouldRender, Size,
};

use zee_edit::{ByteIndex, Cursor, LineIndex, RopeGraphemes};
use zee_grammar::Mode;

use crate::syntax::{
    highlight::{text_style_at_char, Theme as SyntaxTheme},
    parse::ParseTree,
};

#[derive(Clone)]
pub struct Properties {
    pub theme: SyntaxTheme,
    pub focused: bool,
    pub text: Rope,
    pub cursor: Cursor,
    pub mode: &'static Mode,
    pub line_offset: usize,
    pub parse_tree: Option<ParseTree>,
}

pub struct TextArea {
    properties: Properties,
    frame: Rect,
}

impl Component for TextArea {
    type Message = ();
    type Properties = Properties;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        TextArea { properties, frame }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        self.properties = properties;
        ShouldRender::Yes
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let mut canvas = Canvas::new(self.frame.size);
        canvas.clear(self.properties.theme.text);
        self.draw_text(&mut canvas);
        canvas.into()
    }
}

impl TextArea {
    #[inline]
    fn draw_line<'a>(
        &self,
        canvas: &mut Canvas,
        frame: Rect,
        line_index: LineIndex,
        get_scope: &mut impl FnMut(ByteIndex) -> Option<&'a str>,
    ) {
        // Get references to the relevant bits of context
        let Self {
            properties:
                Properties {
                    ref theme,
                    focused,
                    ref text,
                    ref cursor,
                    ..
                },
            ..
        } = *self;

        // Highlight the currently selected line
        let line = text.line(line_index);
        let line_under_cursor = text.char_to_line(cursor.range().start) == line_index;
        if line_under_cursor && focused {
            canvas.clear_region(
                Rect::new(
                    Position::new(frame.origin.x, frame.origin.y),
                    Size::new(frame.size.width, 1),
                ),
                theme.text_current_line,
            );
        }

        let mut visual_x = frame.origin.x;
        let mut char_index = text.line_to_char(line_index);
        let line_start_byte = text.char_to_byte(char_index);

        for grapheme in RopeGraphemes::new(&line.slice(..)) {
            let is_error = false;

            let scope = get_scope(line_start_byte + grapheme.byte_start).unwrap_or("");
            let style = text_style_at_char(
                theme,
                cursor,
                char_index,
                focused,
                line_under_cursor,
                scope,
                is_error,
            );
            let grapheme_width = zee_edit::graphemes::width(&grapheme);
            let horizontal_bounds_inclusive = frame.min_x()..=frame.max_x();
            if !horizontal_bounds_inclusive.contains(&(visual_x + grapheme_width)) {
                break;
            }

            if grapheme.slice == "\t" {
                for offset in 0..grapheme_width {
                    canvas.draw_str(visual_x + offset, frame.origin.y, style, " ");
                }
            } else if grapheme_width == 0 {
                canvas.draw_str(visual_x, frame.origin.y, style, " ");
            } else {
                canvas.draw_graphemes(
                    visual_x,
                    frame.origin.y,
                    style,
                    iter::once(grapheme.chars().collect::<GraphemeCluster>()),
                );
            }

            char_index += grapheme.len_chars();
            visual_x += grapheme_width.max(1);
        }

        if line.get_char(line.len_chars().saturating_sub(1)) != Some('\n')
            && cursor.range().start == char_index
        {
            canvas.draw_str(
                visual_x,
                frame.origin.y,
                if focused {
                    theme.cursor_focused
                } else {
                    theme.cursor_unfocused
                },
                " ",
            );
        }
    }

    #[inline]
    fn text_expanse_in_view(&self, canvas: &Canvas) -> TextExpanse {
        let line_range = self.properties.line_offset
            ..(self.properties.line_offset + canvas.size().height)
                .min(self.properties.text.len_lines());

        let start_byte = self
            .properties
            .text
            .try_line_to_byte(self.properties.line_offset)
            .unwrap_or_else(|_| self.properties.text.len_bytes());
        let end_byte = self
            .properties
            .text
            .try_line_to_byte(line_range.end)
            .unwrap_or_else(|_| self.properties.text.len_bytes());

        TextExpanse {
            byte_range: start_byte..end_byte,
            line_range,
        }
    }

    #[inline]
    fn draw_text(&self, canvas: &mut Canvas) {
        let expanse = self.text_expanse_in_view(canvas);

        let parse_tree = self
            .properties
            .parse_tree
            .as_ref()
            .map(|parse_tree| &parse_tree.tree);
        match parse_tree {
            Some(tree) => {
                let query = &self
                    .properties
                    .mode
                    .grammar
                    .as_ref()
                    .unwrap()
                    .highlights
                    .as_ref()
                    .unwrap();
                let mut query_cursor = QueryCursor::new();
                query_cursor.set_byte_range(expanse.byte_range);
                let names = query.capture_names();
                log::info!("Query capture names: {:?}", names);
                let mut matches = query_cursor
                    .matches(
                        query,
                        tree.root_node(),
                        RopeProvider(self.properties.text.slice(..)),
                    )
                    .peekable();

                let mut get_scope = |byte_index: usize| {
                    loop {
                        let query_match = matches.peek()?;
                        if query_match.captures.is_empty() {
                            matches.next();
                            continue;
                        }
                        // log::debug!("{:?}", query_match.captures);
                        let capture = query_match.captures[0];
                        let capture_range = capture.node.byte_range();
                        if byte_index < capture_range.start {
                            return None;
                        } else if byte_index < capture_range.end {
                            return Some(
                                query.capture_names()[usize::try_from(capture.index).unwrap()]
                                    .as_str(),
                            );
                        } else {
                            matches.next();
                            continue;
                        }
                    }
                };

                for line_index in expanse.line_range {
                    self.draw_line(
                        canvas,
                        Rect::from_size(canvas.size()).inner_rect(SideOffsets2D::new(
                            line_index - self.properties.line_offset,
                            0,
                            0,
                            0,
                        )),
                        line_index,
                        &mut get_scope,
                    );
                }
            }
            None => {
                for line_index in expanse.line_range {
                    self.draw_line(
                        canvas,
                        Rect::from_size(canvas.size()).inner_rect(SideOffsets2D::new(
                            line_index - self.properties.line_offset,
                            0,
                            0,
                            0,
                        )),
                        line_index,
                        &mut |_| None,
                    );
                }
            }
        };
    }
}

struct TextExpanse {
    byte_range: Range<ByteIndex>,
    line_range: Range<LineIndex>,
}

use tree_sitter::{Node, QueryCursor, TextProvider};

pub struct ChunksBytes<'a> {
    chunks: ropey::iter::Chunks<'a>,
}

impl<'a> Iterator for ChunksBytes<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.next().map(str::as_bytes)
    }
}

pub struct RopeProvider<'a>(pub RopeSlice<'a>);

impl<'a> TextProvider<'a> for RopeProvider<'a> {
    type I = ChunksBytes<'a>;

    fn text(&mut self, node: Node) -> Self::I {
        let fragment = self.0.byte_slice(node.start_byte()..node.end_byte());
        ChunksBytes {
            chunks: fragment.chunks(),
        }
    }
}
