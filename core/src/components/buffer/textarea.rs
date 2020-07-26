use euclid::default::SideOffsets2D;
use ropey::{Rope, RopeSlice};
use std::{borrow::Cow, cmp, iter};
use zee_highlight::SelectorNodeId;
use zi::{
    terminal::GraphemeCluster, Canvas, Component, ComponentLink, Layout, Position, Rect,
    ShouldRender, Size,
};

use crate::{
    components::cursor::{CharIndex, Cursor},
    mode::Mode,
    syntax::{
        highlight::{text_style_at_char, Theme as SyntaxTheme},
        parse::{NodeTrace, ParseTree, SyntaxCursor},
    },
    utils::{self, RopeGraphemes},
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
    fn draw_line(
        &self,
        canvas: &mut Canvas,
        frame: Rect,
        line_index: usize,
        line: RopeSlice,
        mut syntax_cursor: Option<&mut SyntaxCursor>,
        mut trace: &mut NodeTrace<SelectorNodeId>,
    ) -> usize {
        // Get references to the relevant bits of context
        let Self {
            properties:
                Properties {
                    ref theme,
                    focused,
                    ref text,
                    ref cursor,
                    mode,
                    ..
                },
            ..
        } = *self;

        // Highlight the currently selected line
        let line_under_cursor = text.char_to_line(cursor.range().start.0) == line_index;
        if line_under_cursor && focused {
            canvas.clear_region(
                Rect::new(
                    Position::new(frame.origin.x, frame.origin.y),
                    Size::new(frame.size.width, 1),
                ),
                theme.text_current_line,
            );
        }

        let mut visual_cursor_x = 0;
        let mut visual_x = frame.origin.x;
        let mut char_index = CharIndex(text.line_to_char(line_index));

        let mut content: Cow<str> = text
            .slice(
                text.byte_to_char(trace.byte_range.start)..text.byte_to_char(trace.byte_range.end),
            )
            .into();
        let mut scope = mode
            .highlights()
            .and_then(|highlights| highlights.matches(&trace.trace, &trace.nth_children, &content))
            .map(|scope| scope.0.as_str());

        for grapheme in RopeGraphemes::new(&line.slice(..)) {
            let byte_index = text.char_to_byte(char_index.0);
            match (syntax_cursor.as_mut(), mode.highlights()) {
                (Some(syntax_cursor), Some(highlights))
                    if !trace.byte_range.contains(&byte_index) =>
                {
                    syntax_cursor.trace_at(&mut trace, byte_index, |node| {
                        highlights.get_selector_node_id(node.kind_id())
                    });
                    content = text
                        .slice(
                            text.byte_to_char(trace.byte_range.start)
                                ..text.byte_to_char(trace.byte_range.end),
                        )
                        .into();

                    scope = highlights
                        .matches(&trace.trace, &trace.nth_children, &content)
                        .map(|scope| scope.0.as_str());
                }
                _ => {}
            };

            if cursor.range().contains(&char_index) && focused {
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
                &theme,
                &cursor,
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

            char_index.0 += grapheme.len_chars();
            visual_x += grapheme_width;
        }

        if line_index == text.len_lines() - 1 && cursor.range().start == text.len_chars().into() {
            canvas.draw_str(
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
    fn draw_text(&self, canvas: &mut Canvas) -> usize {
        let mut syntax_cursor = self
            .properties
            .parse_tree
            .as_ref()
            .map(|tree| tree.cursor());
        let mut trace: NodeTrace<SelectorNodeId> = NodeTrace::new();

        let mut visual_cursor_x = 0;
        for (line_index, line) in self
            .properties
            .text
            .lines_at(self.properties.line_offset)
            .take(canvas.size().height)
            .enumerate()
        {
            visual_cursor_x = cmp::max(
                visual_cursor_x,
                self.draw_line(
                    canvas,
                    Rect::from_size(canvas.size())
                        .inner_rect(SideOffsets2D::new(line_index, 0, 0, 0)),
                    self.properties.line_offset + line_index,
                    line,
                    syntax_cursor.as_mut(),
                    &mut trace,
                ),
            );
        }

        visual_cursor_x
    }
}
