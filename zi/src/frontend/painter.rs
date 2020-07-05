//! Module with utilities to convert a `Canvas` to a set of abstract paint operations.

use unicode_width::UnicodeWidthStr;

use super::Result;
use crate::terminal::{canvas::Textel, Canvas, Position, Size, Style};

pub trait Painter {
    const INITIAL_POSITION: Position;
    const INITIAL_STYLE: Style;

    fn create(size: Size) -> Self;

    fn paint<'a>(
        &mut self,
        target: &'a Canvas,
        paint: impl FnMut(PaintOperation<'a>) -> Result<()>,
    ) -> Result<()>;
}

pub enum PaintOperation<'a> {
    WriteContent(&'a str),
    SetStyle(&'a Style),
    MoveTo(Position),
}

pub struct IncrementalPainter {
    screen: Canvas,
    current_position: Position,
    current_style: Style,
}

impl Painter for IncrementalPainter {
    const INITIAL_POSITION: Position = Position::new(0, 0);
    const INITIAL_STYLE: Style = Style::default();

    fn create(size: Size) -> Self {
        Self {
            screen: Canvas::new(size),
            current_position: Self::INITIAL_POSITION,
            current_style: Self::INITIAL_STYLE,
        }
    }

    #[inline]
    fn paint<'a>(
        &mut self,
        target: &'a Canvas,
        mut paint: impl FnMut(PaintOperation<'a>) -> Result<()>,
    ) -> Result<()> {
        let Self {
            ref mut screen,
            ref mut current_position,
            ref mut current_style,
        } = *self;
        let size = target.size();
        let force_redraw = size != screen.size();
        if force_redraw {
            screen.resize(size);
        }

        screen
            .buffer_mut()
            .iter_mut()
            .zip(target.buffer())
            .enumerate()
            .try_for_each(|(index, (current, new))| -> Result<()> {
                if force_redraw {
                    *current = None;
                }

                if *current == *new {
                    return Ok(());
                }

                if let Some(new) = new {
                    let position = Position::new(index % size.width, index / size.width);
                    if position != *current_position {
                        // eprintln!("MoveTo({})", position);
                        paint(PaintOperation::MoveTo(position))?;
                        *current_position = position;
                    }

                    if new.style != *current_style {
                        // eprintln!("Style({:?})", new.style);
                        paint(PaintOperation::SetStyle(&new.style))?;
                        *current_style = new.style;
                    }

                    let content_width = UnicodeWidthStr::width(&new.grapheme[..]);
                    // eprintln!("Content({:?}) {}", new.grapheme, content_width);
                    paint(PaintOperation::WriteContent(&new.grapheme))?;
                    current_position.x = (index + content_width) % size.width;
                    current_position.y = (index + content_width) / size.width;
                }
                *current = new.clone();

                Ok(())
            })
    }
}

pub struct FullPainter {
    current_style: Style,
}

impl Painter for FullPainter {
    const INITIAL_POSITION: Position = Position::new(0, 0);
    const INITIAL_STYLE: Style = Style::default();

    fn create(_size: Size) -> Self {
        Self {
            current_style: Self::INITIAL_STYLE,
        }
    }

    #[inline]
    fn paint<'a>(
        &mut self,
        target: &'a Canvas,
        mut paint: impl FnMut(PaintOperation<'a>) -> Result<()>,
    ) -> Result<()> {
        let Self {
            ref mut current_style,
        } = *self;
        let size = target.size();
        target
            .buffer()
            .chunks(size.width)
            .enumerate()
            .try_for_each(|(y, line)| -> Result<()> {
                paint(PaintOperation::MoveTo(Position::new(0, y)))?;
                line.iter().try_for_each(|textel| -> Result<()> {
                    if let Some(Textel {
                        ref style,
                        ref grapheme,
                    }) = textel
                    {
                        if *style != *current_style {
                            paint(PaintOperation::SetStyle(style))?;
                            *current_style = *style;
                        }
                        paint(PaintOperation::WriteContent(grapheme))?;
                    }
                    Ok(())
                })
            })
    }
}
