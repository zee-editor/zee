pub mod buffer;
pub mod cursor;
pub mod prompt;
pub mod splash;
pub mod theme;

pub use buffer::{Buffer, BufferTask};
pub use cursor::Cursor;
pub use prompt::{Prompt, PromptTask};
pub use splash::Splash;
pub use theme::Theme;

use smallvec::{smallvec, SmallVec};
use std::{cmp, path::Path, time::Instant};

use crate::{
    error::Result,
    task,
    terminal::{screen::Screen, Key, Position, Rect, Size},
};

pub type ComponentId = usize;
pub type FrameId = usize;
pub type LaidComponentIds = SmallVec<[LaidComponentId; 16]>;

pub enum TaskKind {
    Buffer(BufferTask),
    Prompt(PromptTask),
}

pub type Scheduler<'pool> = task::Scheduler<'pool, Result<TaskKind>>;
pub type TaskResult = task::TaskResult<Result<TaskKind>>;

#[derive(Debug, Clone)]
pub struct Context<'t> {
    pub frame: Rect,
    pub time: Instant,
    pub focused: bool,
    pub frame_id: FrameId,
    pub theme: &'t Theme,
    pub path: &'t Path,
}

impl<'t> Context<'t> {
    pub fn set_frame(&self, frame: Rect) -> Self {
        Self {
            frame,
            time: self.time,
            focused: self.focused,
            frame_id: self.frame_id,
            theme: self.theme,
            path: self.path,
        }
    }

    pub fn set_focused(&self, focused: bool) -> Self {
        Self {
            frame: self.frame,
            time: self.time,
            focused,
            frame_id: self.frame_id,
            theme: self.theme,
            path: self.path,
        }
    }
}

pub trait Component {
    fn draw(&mut self, _screen: &mut Screen, _scheduler: &mut Scheduler, _context: &Context) {}

    fn handle_event(
        &mut self,
        _key: Key,
        _scheduler: &mut Scheduler,
        _context: &Context,
    ) -> Result<()> {
        Ok(())
    }

    fn task_done(&mut self, _task: TaskResult) -> Result<()> {
        Ok(())
    }

    fn path(&self) -> Option<&Path> {
        None
    }

    // fn handle_input(&mut self, controller: &Controller, context: &Context) {}
}

#[derive(Debug, Clone, PartialEq)]
pub enum Flex {
    Fixed(usize),
    Stretched,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Layout {
    Node(Box<LayoutNode>),
    Component(ComponentId),
}

impl Layout {
    #[inline]
    pub fn horizontal(left: LayoutNodeFlex, right: LayoutNodeFlex) -> Self {
        Self::node(LayoutNode {
            direction: LayoutDirection::Horizontal,
            children: smallvec![left, right],
        })
    }

    #[inline]
    pub fn vertical(top: LayoutNodeFlex, bottom: LayoutNodeFlex) -> Self {
        Self::node(LayoutNode {
            direction: LayoutDirection::Vertical,
            children: smallvec![top, bottom],
        })
    }

    #[inline]
    pub fn add_left(self, component_id: ComponentId, flex: Flex) -> Self {
        match self {
            Self::Node(mut node) => {
                node.children.push(LayoutNodeFlex {
                    node: Layout::Component(component_id),
                    flex,
                });
                Self::Node(node)
            }
            Self::Component(other) => Layout::horizontal(
                LayoutNodeFlex {
                    node: Layout::Component(component_id),
                    flex,
                },
                LayoutNodeFlex {
                    node: Self::Component(other),
                    flex: Flex::Stretched,
                },
            ),
        }
    }

    pub fn remove_component_id(self, component_id: ComponentId) -> Option<Layout> {
        match self {
            Self::Node(node) => remove_component_id(*node, component_id),
            Self::Component(id) if id == component_id => None,
            component => Some(component),
        }
    }

    pub fn compute(
        &self,
        frame: Rect,
        frame_id: &mut usize,
        components: &mut SmallVec<[LaidComponentId; 16]>,
    ) {
        match *self {
            Self::Node(ref node) => {
                node.compute(frame, frame_id, components);
            }
            Self::Component(id) => {
                components.push(LaidComponentId {
                    id,
                    frame,
                    frame_id: *frame_id,
                });
                *frame_id += 1;
            }
        };
    }

    #[inline]
    fn node(node: LayoutNode) -> Self {
        Self::Node(Box::new(node))
    }
}

pub struct LaidComponentId {
    pub id: ComponentId,
    pub frame: Rect,
    pub frame_id: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutNodeFlex {
    pub node: Layout,
    pub flex: Flex,
}

impl LayoutNodeFlex {
    pub fn remove_component_id(self, component_id: ComponentId) -> Option<LayoutNodeFlex> {
        let LayoutNodeFlex { node, flex } = self;
        node.remove_component_id(component_id)
            .map(|node| LayoutNodeFlex { node, flex })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
}

impl LayoutDirection {
    #[inline]
    fn dimension(&self, size: Size) -> usize {
        match self {
            &LayoutDirection::Horizontal => size.width,
            &LayoutDirection::Vertical => size.height,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutNode {
    pub children: SmallVec<[LayoutNodeFlex; 4]>,
    pub direction: LayoutDirection,
}

impl LayoutNode {
    pub fn compute(
        &self,
        frame: Rect,
        frame_id: &mut usize,
        components: &mut SmallVec<[LaidComponentId; 16]>,
    ) {
        for (child, frame) in
            self.children
                .iter()
                .zip(splits_iter(frame, self.direction, &self.children))
        {
            child.node.compute(frame, frame_id, components);
        }
    }
}

fn remove_component_id(node: LayoutNode, component_id: ComponentId) -> Option<Layout> {
    let LayoutNode {
        children,
        direction,
    } = node;

    let mut filtered: SmallVec<[LayoutNodeFlex; 4]> = children
        .into_iter()
        .filter_map(|child| child.remove_component_id(component_id))
        .collect();

    match filtered.len() {
        0 => None,
        1 => Some(filtered.remove(0).node),
        _ => Some(Layout::Node(Box::new(LayoutNode {
            direction,
            children: filtered,
        }))),
    }
}

#[inline]
fn splits_iter<'a>(
    frame: Rect,
    direction: LayoutDirection,
    children: &'a [LayoutNodeFlex],
) -> impl Iterator<Item = Rect> + 'a {
    let total_size = direction.dimension(frame.size);

    // Compute how much space is available for stretched components
    let (stretched_budget, num_stretched_children, total_fixed_size) = {
        let mut stretched_budget = total_size;
        let mut num_stretched_children = 0;
        let mut total_fixed_size = 0;
        for child in children.iter() {
            match child.flex {
                Flex::Stretched => {
                    num_stretched_children += 1;
                }
                Flex::Fixed(size) => {
                    stretched_budget = stretched_budget.saturating_sub(size);
                    total_fixed_size += size;
                }
            }
        }
        (stretched_budget, num_stretched_children, total_fixed_size)
    };

    // Divvy up the space equaly between stretched components.
    let stretched_size = if num_stretched_children > 0 {
        stretched_budget / num_stretched_children
    } else {
        0
    };
    let mut remainder =
        total_size.saturating_sub(num_stretched_children * stretched_size + total_fixed_size);
    let mut remaining_size = total_size;
    children
        .iter()
        .map(move |child| match child.flex {
            Flex::Stretched => {
                let offset = total_size - remaining_size;
                let size = if remainder > 0 {
                    remainder -= 1;
                    stretched_size + 1
                } else {
                    stretched_size
                };
                remaining_size -= size;
                (offset, size)
            }
            Flex::Fixed(size) => {
                let offset = total_size - remaining_size;
                let size = cmp::min(remaining_size, size);
                remaining_size -= size;
                (offset, size)
            }
        })
        .map(move |(offset, size)| match direction {
            LayoutDirection::Horizontal => Rect::new(
                Position::new(frame.origin.x + offset, frame.origin.y),
                Size::new(size, frame.size.height),
            ),
            LayoutDirection::Vertical => Rect::new(
                Position::new(frame.origin.x, frame.origin.y + offset),
                Size::new(frame.size.width, size),
            ),
        })
}
