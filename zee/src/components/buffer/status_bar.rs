use size_format::SizeFormatterBinary;
use std::{ops::Range, path::PathBuf};
use zi::{
    unicode_width::UnicodeWidthStr, Canvas, Component, ComponentLink, Layout, Rect, ShouldRender,
    Size, Style,
};

use zee_grammar::Mode;

use crate::{
    editor::buffer::{ModifiedStatus, RepositoryRc},
    utils::StaticRefEq,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub base: Style,
    pub frame_id_focused: Style,
    pub frame_id_unfocused: Style,
    pub is_modified: Style,
    pub is_not_modified: Style,
    pub file_name: Style,
    pub file_size: Style,
    pub position_in_file: Style,
    pub mode: Style,
}

#[derive(Clone, PartialEq)]
pub struct Properties {
    pub theme: Theme,
    pub current_line_index: usize,
    pub column_offset: usize,
    pub file_path: Option<PathBuf>,
    pub focused: bool,
    pub frame_id: usize,
    pub modified_status: ModifiedStatus,
    pub mode: StaticRefEq<Mode>,
    pub num_lines: usize,
    pub repository: Option<RepositoryRc>,
    pub size_bytes: u64,
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
                    ref modified_status,
                    ref mode,
                    ref repository,
                    ref theme,
                    current_line_index,
                    focused,
                    frame_id,
                    num_lines,
                    size_bytes,
                    column_offset,
                },
            frame,
        } = *self;

        let mut canvas = StatusCanvas::new(frame.size, theme.base);
        Some(&mut canvas)
            // Buffer number
            .and_then(|canvas| {
                canvas.append_start(
                    if focused {
                        theme.frame_id_focused
                    } else {
                        theme.frame_id_unfocused
                    },
                    &format!(" {} ", frame_id),
                )
            })
            // Has unsaved changes
            .and_then(|canvas| {
                canvas.append_start(
                    match modified_status {
                        ModifiedStatus::Unchanged => theme.is_not_modified,
                        _ => theme.is_modified,
                    },
                    match modified_status {
                        ModifiedStatus::Unchanged => " - ",
                        ModifiedStatus::Changed | ModifiedStatus::Saving => " + ",
                    },
                )
            })
            // Visual indicator for current position in the file, right-aligned
            .and_then(|canvas| {
                if focused {
                    canvas.append_end(
                        theme.frame_id_focused,
                        &format!(
                            "{}",
                            PROGRESS_SYMBOLS[((PROGRESS_SYMBOLS.len() - 1) as f32
                                * (current_line_index as f32 / num_lines as f32))
                                .round() as usize],
                        ),
                    )
                } else {
                    canvas.append_end(theme.position_in_file, " ")
                }
            })
            // File size
            .and_then(|canvas| {
                canvas.append_start(
                    theme.file_size,
                    &format!(" {}", SizeFormatterBinary::new(size_bytes)),
                )
            })
            // File name if buffer is backed by a file
            .and_then(|canvas| {
                canvas.append_start(
                    theme.file_name,
                    &file_path
                        .as_ref()
                        .map(|path| {
                            match path.file_name().and_then(|file_name| file_name.to_str()) {
                                Some(file_name) => format!(" {}", file_name),
                                None => format!(" {}", path.display()),
                            }
                        })
                        .unwrap_or_else(String::new),
                )
            })
            // The current position in the file as a percentage, right-aligned
            .and_then(|canvas| {
                canvas.append_end(
                    theme.position_in_file,
                    &if current_line_index == 0 {
                        " Top ".into()
                    } else if current_line_index >= num_lines.saturating_sub(2) {
                        " End ".into()
                    } else {
                        format!(
                            " {percent:>2}% ",
                            percent = if num_lines > 0 {
                                100 * (current_line_index + 1) / num_lines
                            } else {
                                100
                            }
                        )
                    },
                )
            })
            // The row:column in the file, right-aligned
            .and_then(|canvas| {
                let line_status = format!(
                    " {one_based_line_index:>3}:{column_offset:>2} ",
                    one_based_line_index = current_line_index + 1
                );
                canvas.append_end(theme.is_not_modified, &line_status)
            })
            // Name of the current mode
            .and_then(|canvas| canvas.append_start(theme.mode, &format!("  {}", mode.name)))
            // Name of the repo right aligned
            .and_then(|canvas| {
                canvas.append_end(
                    theme.position_in_file,
                    &match repository
                        .as_ref()
                        .and_then(|repo| repo.head().ok())
                        .as_ref()
                        .and_then(|reference| reference.shorthand())
                    {
                        Some(reference) => format!("{}  ", reference),
                        None => String::new(),
                    },
                )
            });
        canvas.into()
    }
}

struct StatusCanvas {
    canvas: Canvas,
    free: Range<usize>,
}

impl StatusCanvas {
    fn new(size: Size, base: Style) -> Self {
        debug_assert!(size.height <= 1);
        let mut canvas = Canvas::new(size);
        canvas.clear(base);
        Self {
            canvas,
            free: 0..size.width,
        }
    }

    fn append_start(&mut self, style: Style, content: &str) -> Option<&mut Self> {
        (content.width() <= self.remaining_space()).then(|| {
            let written = self.canvas.draw_str(self.free.start, 0, style, content);
            self.free.start += written;
            self
        })
    }

    fn append_end(&mut self, style: Style, content: &str) -> Option<&mut Self> {
        let width = content.width();
        (width <= self.remaining_space()).then(|| {
            let written = self
                .canvas
                .draw_str(self.free.end - width, 0, style, content);
            self.free.end -= written;
            self
        })
    }

    fn remaining_space(&self) -> usize {
        self.free.end.saturating_sub(self.free.start)
    }
}

impl From<StatusCanvas> for Layout {
    fn from(status_canvas: StatusCanvas) -> Self {
        status_canvas.canvas.into()
    }
}

const PROGRESS_SYMBOLS: [char; 8] = ['▇', '▆', '▅', '▄', '▃', '▂', '▁', ' '];
