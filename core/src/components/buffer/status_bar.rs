use size_format::SizeFormatterBinary;
use std::{borrow::Cow, path::PathBuf};
use zi::{Canvas, Component, ComponentLink, Layout, Rect, ShouldRender};

use super::{ModifiedStatus, RepositoryRc, Theme};
use crate::{mode::Mode, utils::StaticRefEq};

#[derive(Clone, PartialEq)]
pub struct Properties {
    pub current_line_index: usize,
    pub file_path: Option<PathBuf>,
    pub focused: bool,
    pub frame_id: usize,
    pub has_unsaved_changes: ModifiedStatus,
    pub mode: StaticRefEq<Mode>,
    pub num_lines: usize,
    pub repository: Option<RepositoryRc>,
    pub size_bytes: u64,
    pub theme: Cow<'static, Theme>,
    pub visual_cursor_x: usize,
}

pub struct StatusBar {
    properties: Properties,
    frame: Rect,
}

impl Component for StatusBar {
    type Message = ();
    type Properties = Properties;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self { properties, frame }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        if self.properties != properties {
            self.properties = properties;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self {
            properties:
                Properties {
                    ref file_path,
                    ref has_unsaved_changes,
                    ref mode,
                    ref repository,
                    ref theme,
                    current_line_index,
                    focused,
                    frame_id,
                    num_lines,
                    size_bytes,
                    visual_cursor_x,
                },
            frame,
        } = *self;

        let mut canvas = Canvas::new(frame.size);
        canvas.clear(theme.status_base);

        let mut offset = 0;
        // Buffer number
        offset += canvas.draw_str(
            offset,
            0,
            if focused {
                theme.status_frame_id_focused
            } else {
                theme.status_frame_id_unfocused
            },
            &format!(" {} ", frame_id),
        );

        // Has unsaved changes
        offset += canvas.draw_str(
            offset,
            0,
            match has_unsaved_changes {
                ModifiedStatus::Unchanged => theme.status_is_not_modified,
                _ => theme.status_is_modified,
            },
            match has_unsaved_changes {
                ModifiedStatus::Unchanged => " - ",
                ModifiedStatus::Changed | ModifiedStatus::Saving => " ☲ ",
            },
        );

        // File size
        offset += canvas.draw_str(
            offset,
            0,
            theme.status_file_size,
            &format!(" {} ", SizeFormatterBinary::new(size_bytes)),
        );

        // File name if buffer is backed by a file
        offset += canvas.draw_str(
            offset,
            0,
            theme.status_file_name,
            &file_path
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
        canvas.draw_str(offset, 0, theme.status_mode, &format!(" {}", mode.name));

        // Name of the current mode
        let reference = repository.as_ref().map(|repo| repo.head().unwrap());

        // The current position the file right-aligned
        let line_status = format!(
            "{}{current_line:>4}:{current_byte:>2} {percent:>3}% ",
            match reference
                .as_ref()
                .and_then(|reference| reference.shorthand())
            {
                Some(reference) => format!("{}  ", reference),
                None => String::new(),
            },
            current_line = current_line_index,
            current_byte = visual_cursor_x,
            percent = if num_lines > 0 {
                100 * (current_line_index + 1) / num_lines
            } else {
                100
            }
        );
        canvas.draw_str(
            frame.size.width.saturating_sub(line_status.len()),
            0,
            theme.status_position_in_file,
            &line_status,
        );

        canvas.into()
    }
}
