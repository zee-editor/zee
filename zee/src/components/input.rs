use ropey::Rope;

use zi::{
    unicode_width::UnicodeWidthStr, AnyCharacter, Bindings, Callback, Canvas, Component,
    ComponentLink, Key, Layout, Rect, ShouldRender, Style,
};

use crate::editor::ContextHandle;
use zee_edit::{graphemes::RopeGraphemes, movement, Cursor, Direction};

#[derive(Clone, PartialEq)]
pub struct InputProperties {
    pub context: ContextHandle,
    pub style: InputStyle,
    pub content: Rope,
    pub cursor: Cursor,
    pub on_change: Option<Callback<InputChange>>,
    pub focused: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputStyle {
    pub content: Style,
    pub cursor: Style,
}

#[derive(Clone, Debug)]
pub struct InputChange {
    pub content: Option<Rope>,
    pub cursor: Cursor,
}

pub struct Input {
    properties: InputProperties,
    frame: Rect,
}

impl Component for Input {
    type Message = Message;
    type Properties = InputProperties;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self { properties, frame }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        let should_render = (self.properties != properties).into();
        self.properties = properties;
        should_render
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        let mut cursor = self.properties.cursor.clone();
        let mut content_change = None;
        let content = &self.properties.content;
        match message {
            // Movement
            Message::Move(direction, count) => {
                movement::move_horizontally(content, &mut cursor, direction, count);
            }
            Message::MoveWord(direction, count) => {
                movement::move_word(content, &mut cursor, direction, count)
            }
            Message::StartOfLine => {
                movement::move_to_start_of_line(content, &mut cursor);
            }
            Message::EndOfLine => {
                movement::move_to_end_of_line(content, &mut cursor);
            }

            // Insertion
            Message::InsertChar { character } => {
                let mut new_content = content.clone();
                cursor.insert_char(&mut new_content, character);
                movement::move_horizontally(&new_content, &mut cursor, Direction::Forward, 1);
                content_change = Some(new_content);
            }

            // Deletion
            Message::DeleteBackward => {
                let mut new_content = content.clone();
                cursor.delete_backward(&mut new_content);
                content_change = Some(new_content);
            }
            Message::DeleteForward => {
                let mut new_content = content.clone();
                cursor.delete_forward(&mut new_content);
                content_change = Some(new_content);
            }
            Message::DeleteLine => {
                let mut new_content = content.clone();
                cursor.delete_line(&mut new_content);
                content_change = Some(new_content);
            }

            // Selection
            Message::BeginSelection => {
                if cursor.is_selecting() {
                    cursor.clear_selection();
                } else {
                    cursor.begin_selection();
                }
            }
            Message::SelectAll => {
                cursor.select_all(content);
            }

            Message::Yank => {
                let clipboard_str = self.properties.context.clipboard.get_contents().unwrap();
                if !clipboard_str.is_empty() {
                    let mut new_content = content.clone();
                    cursor.insert_chars(&mut new_content, clipboard_str.chars());
                    movement::move_horizontally(
                        &new_content,
                        &mut cursor,
                        Direction::Forward,
                        clipboard_str.chars().count(),
                    );
                    content_change = Some(new_content);
                }
            }
            Message::CopySelection => {
                let selection = cursor.selection();
                self.properties
                    .context
                    .clipboard
                    .set_contents(content.slice(selection.start..selection.end).into())
                    .unwrap();
                cursor.clear_selection();
            }
            Message::CutSelection => {
                let mut new_content = content.clone();
                let operation = cursor.delete_selection(&mut new_content);
                self.properties
                    .context
                    .clipboard
                    .set_contents(operation.deleted.into())
                    .unwrap();
                content_change = Some(new_content);
            }
        }

        if let Some(on_change) = self.properties.on_change.as_ref() {
            on_change.emit(InputChange {
                cursor,
                content: content_change,
            });
        }

        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self {
            properties:
                InputProperties {
                    ref content,
                    ref cursor,
                    ref style,
                    ..
                },
            ..
        } = *self;

        let mut canvas = Canvas::new(self.frame.size);
        canvas.clear(style.content);

        let mut char_offset = 0;
        let mut visual_offset = 0;
        for grapheme in RopeGraphemes::new(&content.slice(..)) {
            let len_chars = grapheme.len_chars();
            // TODO: don't unwrap (need to be able to create a smallstring from a rope slice)
            let grapheme = grapheme.as_str().unwrap();
            let grapheme_width = UnicodeWidthStr::width(grapheme);

            canvas.draw_str(
                visual_offset,
                0,
                if cursor.selection().contains(&char_offset) {
                    style.cursor
                } else {
                    style.content
                },
                if grapheme_width > 0 { grapheme } else { " " },
            );
            visual_offset += grapheme_width;
            char_offset += len_chars;
        }

        if cursor.range().start == char_offset {
            canvas.draw_str(visual_offset, 0, style.cursor, " ");
        }

        canvas.into()
    }

    fn bindings(&self, bindings: &mut Bindings<Self>) {
        use Key::*;

        bindings.set_focus(self.properties.focused);
        if !bindings.is_empty() {
            return;
        }

        // Movement
        bindings
            .command("move-backward", || Message::Move(Direction::Backward, 1))
            .with([Ctrl('b')])
            .with([Left]);
        bindings
            .command("move-forward", || Message::Move(Direction::Forward, 1))
            .with([Ctrl('f')])
            .with([Right]);
        bindings
            .command("move-backward-word", || {
                Message::MoveWord(Direction::Backward, 1)
            })
            .with([Alt('b')]);
        bindings
            .command("move-forward-word", || {
                Message::MoveWord(Direction::Forward, 1)
            })
            .with([Alt('f')]);
        bindings
            .command("start-of-line", || Message::StartOfLine)
            .with([Ctrl('a')])
            .with([Home]);
        bindings
            .command("end-of-line", || Message::EndOfLine)
            .with([Ctrl('e')])
            .with([End]);

        // Selection
        //
        // Begin selection
        bindings
            .command("begin-selection", || Message::BeginSelection)
            .with([Null])
            .with([Ctrl(' ')]);

        // Select all
        bindings.add("select-all", [Ctrl('x'), Char('h')], || Message::SelectAll);
        // Copy selection to clipboard
        bindings.add("copy-selection", [Alt('w')], || Message::CopySelection);
        // Cut selection to clipboard
        bindings.add("cut-selection", [Ctrl('w')], || Message::CutSelection);
        // Paste from clipboard
        bindings.add("paste-clipboard", [Ctrl('y')], || Message::Yank);

        // Editing
        bindings
            .command("delete-forward", || Message::DeleteForward)
            .with([Ctrl('d')])
            .with([Delete]);
        bindings.add("delete-backward", [Backspace], || Message::DeleteBackward);
        bindings.add("delete-line", [Ctrl('k')], || Message::DeleteLine);

        bindings.add(
            "insert-character",
            AnyCharacter,
            |keys: &[Key]| match keys {
                &[Char(character)]
                    if character != '\n' && character != '\r' && character != '\t' =>
                {
                    Some(Message::InsertChar { character })
                }
                _ => None,
            },
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    // Movement
    Move(Direction, usize),
    MoveWord(Direction, usize),
    StartOfLine,
    EndOfLine,

    // Editing
    BeginSelection,
    SelectAll,
    Yank,
    CopySelection,
    CutSelection,

    DeleteForward,
    DeleteBackward,
    DeleteLine,
    InsertChar { character: char },
}
