use ropey::Rope;
use unicode_width::UnicodeWidthStr;

use super::{
    layout::Layout, BindingMatch, BindingTransition, Component, ComponentLink, ShouldRender,
};
use crate::{
    task::Scheduler,
    terminal::{Canvas, Colour, Key, Rect, Style},
    text::{cursor, CharIndex, Cursor, TextStorage},
};

#[derive(Clone, Debug)]
pub struct InputStyle {
    pub text: Style,
    pub cursor: Style,
}

impl Default for InputStyle {
    fn default() -> Self {
        const DARK0_SOFT: Colour = Colour::rgb(50, 48, 47);
        const LIGHT2: Colour = Colour::rgb(213, 196, 161);
        const BRIGHT_BLUE: Colour = Colour::rgb(131, 165, 152);

        Self {
            text: Style::normal(DARK0_SOFT, LIGHT2),
            cursor: Style::normal(BRIGHT_BLUE, DARK0_SOFT),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InputProperties<CallbackT> {
    pub style: InputStyle,
    pub content: Rope,
    pub on_change: Option<CallbackT>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    CursorLeft,
    CursorRight,
    InsertChar(char),
    DeleteBackward,
    DeleteForward,
    StartOfLine,
    EndOfLine,
}

#[derive(Debug)]
pub struct Input<CallbackT> {
    properties: InputProperties<CallbackT>,
    content: Rope,
    cursor: Cursor,
}

impl<CallbackT> Component for Input<CallbackT>
where
    CallbackT: FnMut(String) + Clone + 'static,
{
    type Message = Message;
    type Properties = InputProperties<CallbackT>;

    fn create(
        properties: Self::Properties,
        _link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        let mut content = properties.content.clone();
        cursor::ensure_trailing_newline_with_content(&mut content);
        Self {
            properties,
            content,
            cursor: Cursor::new(),
        }
    }

    fn update(
        &mut self,
        message: Self::Message,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        match message {
            Message::CursorLeft => self.cursor.move_left(&self.content),
            Message::CursorRight => self.cursor.move_right(&self.content),
            Message::StartOfLine => self.cursor.move_to_start_of_line(&self.content),
            Message::EndOfLine => self.cursor.move_to_end_of_buffer(&self.content),
            Message::InsertChar(character) => {
                self.cursor.insert_char(&mut self.content, character);
                self.cursor.move_right(&self.content);
                if let Some(on_change) = self.properties.on_change.as_mut() {
                    (on_change)(self.content.clone().into())
                }
            }

            Message::DeleteBackward => {
                self.cursor.backspace(&mut self.content);
                if let Some(on_change) = self.properties.on_change.as_mut() {
                    (on_change)(self.content.clone().into())
                }
            }
            Message::DeleteForward => {
                self.cursor.delete(&mut self.content);
                if let Some(on_change) = self.properties.on_change.as_mut() {
                    (on_change)(self.content.clone().into())
                }
            }
        }
        ShouldRender::Yes
    }

    fn change(
        &mut self,
        properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        let mut new_content = properties.content.clone();
        cursor::ensure_trailing_newline_with_content(&mut new_content);
        self.cursor.sync(&self.content, &new_content);
        self.content = new_content;
        self.properties = properties;

        ShouldRender::Yes
    }

    fn view(&self, frame: Rect) -> Layout {
        let Self {
            ref cursor,
            properties:
                InputProperties {
                    ref content,
                    ref style,
                    ..
                },
            ..
        } = *self;

        let mut canvas = Canvas::new(frame.size);
        canvas.clear(style.text);

        let mut char_offset = 0;
        for grapheme in content.graphemes() {
            let len_chars = grapheme.len_chars();
            // TODO: don't unwrap (need to be able to create a smallstring from a rope slice)
            let grapheme = grapheme.as_str().unwrap();
            let grapheme_width = UnicodeWidthStr::width(grapheme);

            canvas.draw_str(
                char_offset,
                0,
                if cursor.range().contains(&CharIndex(char_offset)) {
                    style.cursor
                } else {
                    style.text
                },
                if grapheme_width > 0 { grapheme } else { " " },
            );
            char_offset += len_chars;
        }

        Layout::Canvas(canvas)
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let mut transition = BindingTransition::Clear;
        let message = match pressed {
            &[Key::Ctrl('b')] | &[Key::Left] => Some(Message::CursorLeft),
            &[Key::Ctrl('f')] | &[Key::Right] => Some(Message::CursorRight),
            &[Key::Ctrl('a')] | &[Key::Home] => Some(Message::StartOfLine),
            &[Key::Ctrl('e')] | &[Key::End] => Some(Message::EndOfLine),
            &[Key::Char(character)] if character != '\n' && character != '\r' => {
                Some(Message::InsertChar(character))
            }
            &[Key::Ctrl('d')] | &[Key::Delete] => Some(Message::DeleteForward),
            &[Key::Backspace] => Some(Message::DeleteBackward),
            &[Key::Ctrl('x')] => {
                transition = BindingTransition::Continue;
                None
            }
            _ => None,
        };
        BindingMatch {
            transition,
            message,
        }
    }
}

// pub fn from_rope_slice(text: &RopeSlice) -> SmallString<[u8; 4]> {
//     let mut string = SmallString::with_capacity(text.len_bytes());
//     let mut idx = 0;
//     for chunk in text.chunks() {
//         unsafe { string.insert_bytes(idx, chunk.as_bytes()) };
//         idx += chunk.len();
//     }
//     string
// }
