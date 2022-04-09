use zi::{terminal::Key, Bindings, EndsWith, FlexDirection};

use super::{Editor, FileSource, Message};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KeySequenceSlice<'a> {
    keys: &'a [Key],
    prefix: bool,
}

impl<'a> KeySequenceSlice<'a> {
    pub fn new(keys: &'a [Key], prefix: bool) -> Self {
        Self { keys, prefix }
    }
}

impl<'a> std::fmt::Display for KeySequenceSlice<'a> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        for (index, key) in self.keys.iter().enumerate() {
            match key {
                Key::Char(' ') => write!(formatter, "SPC")?,
                Key::Char('\n') => write!(formatter, "RET")?,
                Key::Char('\t') => write!(formatter, "TAB")?,
                Key::Char(char) => write!(formatter, "{}", char)?,
                Key::Ctrl(char) => write!(formatter, "C-{}", char)?,
                Key::Alt(char) => write!(formatter, "A-{}", char)?,
                Key::F(number) => write!(formatter, "F{}", number)?,
                Key::Esc => write!(formatter, "ESC")?,
                key => write!(formatter, "{:?}", key)?,
            }
            if index < self.keys.len().saturating_sub(1) {
                write!(formatter, " ")?;
            } else if self.prefix {
                write!(formatter, "-")?;
            }
        }
        Ok(())
    }
}

pub(super) fn initialize(bindings: &mut Bindings<Editor>) {
    bindings.set_focus(true);
    bindings.set_notify(true);

    // Cancel
    bindings.add("cancel", EndsWith(Key::Ctrl('g')), || Message::Cancel);

    // Open a file
    bindings.add("find-file", [Key::Ctrl('x'), Key::Ctrl('f')], || {
        Message::OpenFilePicker(FileSource::Directory)
    });
    bindings.add(
        "find-file-in-repo",
        [Key::Ctrl('x'), Key::Ctrl('v')],
        || Message::OpenFilePicker(FileSource::Repository),
    );

    // Buffer management
    bindings.add("switch-buffer", [Key::Ctrl('x'), Key::Char('b')], || {
        Message::SelectBufferPicker
    });
    bindings.add("kill-buffer", [Key::Ctrl('x'), Key::Char('k')], || {
        Message::KillBufferPicker
    });

    // Window management
    //
    // Change focus
    bindings.add(
        "focus-next-window",
        [Key::Ctrl('x'), Key::Char('o')],
        || Message::FocusNextWindow,
    );
    bindings.add(
        "focus-next-window",
        [Key::Ctrl('x'), Key::Ctrl('o')],
        || Message::FocusNextWindow,
    );
    bindings.add(
        "focus-previous-window",
        [Key::Ctrl('x'), Key::Char('i')],
        || Message::FocusPreviousWindow,
    );
    bindings.add(
        "focus-previous-window",
        [Key::Ctrl('x'), Key::Ctrl('i')],
        || Message::FocusPreviousWindow,
    );
    // Make current window fullscreen
    bindings.add(
        "fullscreen-window",
        [Key::Ctrl('x'), Key::Char('1')],
        || Message::FullscreenWindow,
    );
    bindings.add(
        "fullscreen-window",
        [Key::Ctrl('x'), Key::Ctrl('1')],
        || Message::FullscreenWindow,
    );
    // Split window below (column)
    bindings.add(
        "split-window-below",
        [Key::Ctrl('x'), Key::Char('2')],
        || Message::SplitWindow(FlexDirection::Column),
    );
    bindings.add(
        "split-window-below",
        [Key::Ctrl('x'), Key::Ctrl('2')],
        || Message::SplitWindow(FlexDirection::Column),
    );
    // Split window right (row)
    bindings.add(
        "split-window-right",
        [Key::Ctrl('x'), Key::Char('3')],
        || Message::SplitWindow(FlexDirection::Row),
    );
    bindings.add(
        "split-window-right",
        [Key::Ctrl('x'), Key::Ctrl('3')],
        || Message::SplitWindow(FlexDirection::Row),
    );
    // Delete window
    bindings.add("delete-window", [Key::Ctrl('x'), Key::Char('0')], || {
        Message::DeleteWindow
    });
    bindings.add("delete-window", [Key::Ctrl('x'), Key::Ctrl('0')], || {
        Message::DeleteWindow
    });

    // Theme
    bindings.add("change-theme", [Key::Ctrl('x'), Key::Ctrl('t')], || {
        Message::ChangeTheme
    });

    // Quit
    bindings.add("quit", [Key::Ctrl('x'), Key::Ctrl('c')], || Message::Quit);
}
