pub mod line_info;
pub mod status_bar;
pub mod textarea;

use std::{borrow::Cow, iter, path::PathBuf};
use zi::{
    components::text::{Text, TextAlign, TextProperties},
    prelude::*,
};

use zee_edit::{tree::EditTree, Direction};
use zee_grammar::Mode;

use self::{
    line_info::{LineInfo, Properties as LineInfoProperties},
    status_bar::{Properties as StatusBarProperties, StatusBar, Theme as StatusBarTheme},
    textarea::{Properties as TextAreaProperties, TextArea},
};
use super::edit_tree_viewer::{
    EditTreeViewer, Properties as EditTreeViewerProperties, Theme as EditTreeViewerTheme,
};
use crate::{
    editor::{
        buffer::{BufferCursor, CursorMessage, ModifiedStatus, RepositoryRc},
        ContextHandle,
    },
    syntax::{highlight::Theme as SyntaxTheme, parse::ParseTree},
    versioned::WeakHandle,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub border: Style,
    pub edit_tree_viewer: EditTreeViewerTheme,
    pub status_bar: StatusBarTheme,
    pub syntax: SyntaxTheme,
}

pub struct Properties {
    pub context: ContextHandle,
    pub theme: Cow<'static, Theme>,
    pub focused: bool,
    pub frame_id: usize,
    pub mode: &'static Mode,
    pub repo: Option<RepositoryRc>,
    pub content: WeakHandle<EditTree>,
    pub file_path: Option<PathBuf>,
    pub cursor: BufferCursor,
    pub parse_tree: Option<ParseTree>,
    pub modified_status: ModifiedStatus,
}

impl PartialEq for Properties {
    fn eq(&self, other: &Self) -> bool {
        self.cursor == other.cursor
            && self.content.version() == other.content.version()
            && self.parse_tree.as_ref().map(|tree| tree.version)
                == other.parse_tree.as_ref().map(|tree| tree.version)
            && self.modified_status == other.modified_status
            && self.focused == other.focused
            && self.frame_id == other.frame_id
            && *self.theme == *other.theme
            && self.mode == other.mode
            && self.repo == other.repo
            && self.file_path == other.file_path
    }
}

#[derive(Debug)]
pub enum Message {
    CenterCursorVisually,
    ClearSelection,
    ToggleEditTree,
}

pub struct Buffer {
    properties: Properties,
    frame: Rect,
    line_offset: usize,
    viewing_edit_tree: bool,
}

impl Buffer {
    fn ensure_cursor_in_view(&mut self) -> ShouldRender {
        let content = self.properties.content.upgrade();
        let current_line = content.char_to_line(self.properties.cursor.inner().range().start);
        let num_lines = self.frame.size.height.saturating_sub(1);
        if current_line < self.line_offset {
            self.line_offset = current_line;
            ShouldRender::Yes
        } else if current_line - self.line_offset > num_lines.saturating_sub(1) {
            self.line_offset = current_line + 1 - num_lines;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn center_visual_cursor(&mut self) {
        let content = self.properties.content.upgrade();
        let line_index = content.char_to_line(self.properties.cursor.inner().range().start);
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

    fn move_up(&self) {
        if self.viewing_edit_tree {
            self.properties.cursor.undo();
        } else {
            self.properties.cursor.move_up();
        }
    }

    fn move_down(&self) {
        if self.viewing_edit_tree {
            self.properties.cursor.redo();
        } else {
            self.properties.cursor.move_down();
        }
    }

    fn move_left(&self) {
        if self.viewing_edit_tree {
            self.properties.cursor.previous_child_revision();
        } else {
            self.properties.cursor.move_left();
        }
    }

    fn move_right(&self) {
        if self.viewing_edit_tree {
            self.properties.cursor.next_child_revision();
        } else {
            self.properties.cursor.move_right();
        }
    }

    fn move_page_down(&self) {
        self.properties
            .cursor
            .move_down_n(self.frame.size.height.saturating_sub(1));
    }

    fn move_page_up(&self) {
        self.properties
            .cursor
            .move_up_n(self.frame.size.height.saturating_sub(1));
    }

    fn move_start_of_line(&self) {
        self.properties.cursor.move_start_of_line()
    }

    fn move_end_of_line(&self) {
        self.properties.cursor.move_end_of_line()
    }

    fn move_start_of_buffer(&self) {
        self.properties.cursor.move_start_of_buffer()
    }

    fn move_end_of_buffer(&self) {
        self.properties.cursor.move_end_of_buffer()
    }

    fn delete_forward(&self) {
        self.properties.cursor.delete_forward()
    }

    fn delete_backward(&self) {
        self.properties.cursor.delete_backward()
    }

    fn delete_line(&self) {
        self.properties.cursor.delete_line()
    }

    fn insert_new_line(&self) {
        self.properties.cursor.insert_new_line()
    }
}

impl Component for Buffer {
    type Properties = Properties;
    type Message = Message;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        let mut buffer = Self {
            line_offset: 0,
            viewing_edit_tree: false,
            properties,
            frame,
        };
        buffer.ensure_cursor_in_view();
        buffer
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        let changed_properties = self.properties != properties;
        self.properties = properties;
        self.ensure_cursor_in_view() | changed_properties.into()
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        let changed_frame = self.frame != frame;
        self.frame = frame;
        self.ensure_cursor_in_view() | changed_frame.into()
    }

    fn update(&mut self, message: Message) -> ShouldRender {
        match message {
            Message::CenterCursorVisually => {
                self.center_visual_cursor();
                ShouldRender::Yes
            }
            Message::ClearSelection if self.viewing_edit_tree => {
                self.viewing_edit_tree = false;
                ShouldRender::Yes
            }
            Message::ClearSelection => ShouldRender::No,
            Message::ToggleEditTree => {
                self.viewing_edit_tree = !self.viewing_edit_tree;
                ShouldRender::Yes
            }
        }
    }

    fn view(&self) -> Layout {
        let content = self.properties.content.upgrade();

        // The textarea components that displays text
        let textarea = TextArea::with(TextAreaProperties {
            theme: self.properties.theme.syntax.clone(),
            focused: self.properties.focused,
            text: content.staged().clone(),
            cursor: self.properties.cursor.inner().clone(),
            mode: self.properties.mode,
            line_offset: self.line_offset,
            parse_tree: self.properties.parse_tree.clone(),
        });

        // Vertical info bar which shows line specific diagnostics
        let line_info = LineInfo::with(LineInfoProperties {
            style: self.properties.theme.border,
            line_offset: self.line_offset,
            num_lines: content.len_lines()
                - if content.line(content.len_lines() - 1).len_chars() > 0 {
                    0
                } else {
                    1
                },
        });

        // The "status bar" which shows information about the file etc.
        let status_bar = StatusBar::with(StatusBarProperties {
            current_line_index: content.char_to_line(self.properties.cursor.inner().range().start),
            column_offset: self
                .properties
                .cursor
                .inner()
                .column_offset(self.properties.mode.indentation.tab_width(), &content),
            file_path: self.properties.file_path.clone(),
            focused: self.properties.focused,
            frame_id: self.properties.frame_id,
            modified_status: self.properties.modified_status,
            mode: self.properties.mode.into(),
            num_lines: content.len_lines(),
            repository: self.properties.repo.clone(),
            size_bytes: content.len_bytes() as u64,
            theme: self.properties.theme.status_bar.clone(),
        });

        // Edit-tree viewer (aka. undo/redo tree)
        let edit_tree_viewer = if self.viewing_edit_tree {
            Some(Item::fixed(EDIT_TREE_WIDTH)(Container::row([
                Item::fixed(1)(Text::with(
                    TextProperties::new().style(self.properties.theme.border),
                )),
                Item::auto(Container::column([
                    Item::auto(EditTreeViewer::with(EditTreeViewerProperties {
                        tree: self.properties.content.clone(),
                        theme: self.properties.theme.edit_tree_viewer.clone(),
                    })),
                    Item::fixed(1)(Text::with(
                        TextProperties::new()
                            .content("Edit Tree Viewer 🌴")
                            .style(self.properties.theme.border)
                            .align(TextAlign::Centre),
                    )),
                ])),
            ])))
        } else {
            None
        };

        Layout::column([
            Item::auto(Layout::row(
                iter::once(edit_tree_viewer)
                    .chain(iter::once(Some(Item::fixed(1)(line_info))))
                    .chain(iter::once(Some(Item::auto(textarea))))
                    .flatten(),
            )),
            Item::fixed(1)(status_bar),
        ])
    }

    fn bindings(&self, bindings: &mut Bindings<Self>) {
        use Key::*;

        bindings.set_focus(self.properties.focused);
        if !bindings.is_empty() {
            return;
        }

        // Cursor movement
        //
        // Up
        bindings
            .command("move-backward-line", Self::move_up)
            .with([Ctrl('p')])
            .with([Up]);

        // Down
        bindings
            .command("move-forward-line", Self::move_down)
            .with([Ctrl('n')])
            .with([Down]);

        // Left
        bindings
            .command("move-backward", Self::move_left)
            .with([Ctrl('b')])
            .with([Left]);

        // Right
        bindings
            .command("move-forward", Self::move_right)
            .with([Ctrl('f')])
            .with([Right]);

        // Move by word
        //
        // TODO: Add Alt + Left / Right / Up / Down alternative key bindings
        //       For this to be possible, zi should support Alt + a key, not just char
        bindings
            .command("move-backward-word", |this: &Self| {
                this.properties
                    .cursor
                    .send_cursor(CursorMessage::MoveWord(Direction::Backward, 1))
            })
            .with([Alt('b')]);
        bindings
            .command("move-forward-word", |this: &Self| {
                this.properties
                    .cursor
                    .send_cursor(CursorMessage::MoveWord(Direction::Forward, 1))
            })
            .with([Alt('f')]);

        // Move by paragraph
        bindings
            .command("move-backward-paragraph", |this: &Self| {
                this.properties
                    .cursor
                    .send_cursor(CursorMessage::MoveParagraph(Direction::Backward, 1))
            })
            .with([Alt('p')]);
        bindings
            .command("move-forward-paragraph", |this: &Self| {
                this.properties
                    .cursor
                    .send_cursor(CursorMessage::MoveParagraph(Direction::Forward, 1))
            })
            .with([Alt('n')]);

        // Page down
        bindings
            .command("move-page-down", Self::move_page_down)
            .with([Ctrl('v')])
            .with([PageDown]);

        // Page up
        bindings
            .command("move-page-up", Self::move_page_up)
            .with([Alt('v')])
            .with([PageUp]);

        // Start/end of line
        bindings
            .command("move-start-of-line", Self::move_start_of_line)
            .with([Ctrl('a')])
            .with([Home]);
        bindings
            .command("move-end-of-line", Self::move_end_of_line)
            .with([Ctrl('e')])
            .with([End]);

        // Start/end of buffer
        bindings.add(
            "move-start-of-buffer",
            [Alt('<')],
            Self::move_start_of_buffer,
        );
        bindings.add("move-end-of-buffer", [Alt('>')], Self::move_end_of_buffer);

        // Editing
        //
        // Delete forward
        bindings
            .command("delete-forward", Self::delete_forward)
            .with([Ctrl('d')])
            .with([Delete]);

        // Delete backward
        bindings.add("delete-backward", [Backspace], Self::delete_backward);

        // Delete line
        bindings.add("delete-line", [Ctrl('k')], Self::delete_line);

        // Insert new line
        bindings.add("insert-new-line", [Char('\n')], Self::insert_new_line);
        bindings.add("insert-new-line-after", [Ctrl('o')], |this: &Self| {
            this.properties.cursor.insert_char('\n', false)
        });

        // Insert tab
        bindings.add("insert-tab", [Char('\t')], |this: &Self| {
            this.properties.cursor.insert_tab()
        });

        // Insert character
        bindings.add(
            "insert-character",
            AnyCharacter,
            |this: &Self, keys: &[Key]| match keys {
                &[Char(character)] if character != '\n' => {
                    this.properties.cursor.insert_char(character, true)
                }
                _ => {}
            },
        );

        // Selections
        //
        // Begin selection
        bindings
            .command("begin-selection", |this: &Self| {
                this.properties.cursor.begin_selection();
            })
            .with([Null])
            .with([Ctrl(' ')]);

        // Select all
        bindings.add("select-all", [Ctrl('x'), Char('h')], |this: &Self| {
            this.properties.cursor.select_all();
        });
        // Copy selection to clipboard
        bindings.add("copy-selection", [Alt('w')], |this: &Self| {
            this.properties.cursor.copy_selection_to_clipboard();
        });
        // Cut selection to clipboard
        bindings.add("cut-selection", [Ctrl('w')], |this: &Self| {
            this.properties.cursor.cut_selection_to_clipboard();
        });
        // Paste from clipboard
        bindings.add("paste-clipboard", [Ctrl('y')], |this: &Self| {
            this.properties.cursor.paste_from_clipboard();
        });

        // Undo / Redo
        //
        // Undo
        bindings
            .command("undo", |this: &Self| {
                this.properties.cursor.undo();
            })
            .with([Ctrl('_')])
            .with([Ctrl('z')])
            .with([Ctrl('/')]);

        // Redo
        bindings.add("redo", [Ctrl('q')], |this: &Self| {
            this.properties.cursor.redo();
        });

        // Save buffer
        bindings
            .command("save-buffer", |this: &Self| {
                this.properties.cursor.save();
            })
            .with([Ctrl('x'), Ctrl('s')])
            .with([Ctrl('x'), Char('s')]);

        // Centre cursor visually
        bindings.add("center-cursor-visually", [Ctrl('l')], || {
            Message::CenterCursorVisually
        });

        // View edit tree
        //
        // Toggle
        bindings.add("toggle-edit-tree", [Ctrl('x'), Char('u')], || {
            Message::ToggleEditTree
        });

        // Close
        bindings.add("clear-selection", [Ctrl('g')], |this: &Self| {
            if this.viewing_edit_tree {
                Some(Message::ClearSelection)
            } else {
                this.properties.cursor.clear_selection();
                None
            }
        });
    }
}

const EDIT_TREE_WIDTH: usize = 36;
