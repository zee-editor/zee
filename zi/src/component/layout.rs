use smallvec::{smallvec, SmallVec};
use std::{
    cmp,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use super::{
    template::{ComponentDef, DynamicTemplate, Template},
    Component,
};
use crate::terminal::{Canvas, Position, Rect, Size};

/// Vertical layout with child components laid out from top to bottom.
#[inline]
pub fn column(children: impl Into<Items>) -> Layout {
    container(FlexDirection::Column, children)
}

#[inline]
pub fn column_iter(children: impl IntoIterator<Item = Item>) -> Layout {
    container_iter(FlexDirection::Column, children)
}

#[inline]
pub fn column_reverse_iter(children: impl IntoIterator<Item = Item>) -> Layout {
    container_iter(FlexDirection::ColumnReverse, children)
}

#[inline]
pub fn row(children: impl Into<Items>) -> Layout {
    container(FlexDirection::Row, children)
}

#[inline]
pub fn row_iter(children: impl IntoIterator<Item = Item>) -> Layout {
    container_iter(FlexDirection::Row, children)
}

#[inline]
pub fn row_reverse_iter(children: impl IntoIterator<Item = Item>) -> Layout {
    container_iter(FlexDirection::RowReverse, children)
}

#[inline]
pub fn container(direction: FlexDirection, children: impl Into<Items>) -> Layout {
    Layout(LayoutNode::Container(Box::new(Container {
        direction,
        children: children.into().0,
    })))
}

#[inline]
pub fn container_iter(
    direction: FlexDirection,
    children: impl IntoIterator<Item = Item>,
) -> Layout {
    Layout(LayoutNode::Container(Box::new(Container {
        direction,
        children: children.into_iter().collect(),
    })))
}

#[inline]
pub fn component<ComponentT: Component>(properties: ComponentT::Properties) -> Layout {
    Layout(LayoutNode::Component(DynamicTemplate(Box::new(
        ComponentDef::<ComponentT>::new(None, properties),
    ))))
}

#[inline]
pub fn component_with_key<ComponentT: Component>(
    key: usize,
    properties: ComponentT::Properties,
) -> Layout {
    Layout(LayoutNode::Component(DynamicTemplate(Box::new(
        ComponentDef::<ComponentT>::new(Some(key.into()), properties),
    ))))
}

#[inline]
pub fn component_with_key_str<ComponentT: Component>(
    key: &str,
    properties: ComponentT::Properties,
) -> Layout {
    Layout(LayoutNode::Component(DynamicTemplate(Box::new(
        ComponentDef::<ComponentT>::new(Some(key.into()), properties),
    ))))
}

#[inline]
pub fn auto(layout: Layout) -> Item {
    Item {
        node: layout,
        flex: FlexBasis::Auto,
    }
}

#[inline]
pub fn fixed(size: usize, layout: Layout) -> Item {
    Item {
        node: layout,
        flex: FlexBasis::Fixed(size),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ComponentKey(usize);

impl From<usize> for ComponentKey {
    fn from(key: usize) -> Self {
        Self(key)
    }
}

impl From<&str> for ComponentKey {
    fn from(key: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        Self(hasher.finish() as usize)
    }
}

#[derive(Clone)]
pub struct Layout(pub(crate) LayoutNode);

#[derive(Clone)]
pub(crate) enum LayoutNode {
    Container(Box<Container>),
    Component(DynamicTemplate),
    Canvas(Canvas),
}

impl LayoutNode {
    pub(crate) fn crawl(
        &mut self,
        frame: Rect,
        position_hash: u64,
        view_fn: &mut impl FnMut(LaidComponent),
        draw_fn: &mut impl FnMut(LaidCanvas),
    ) {
        let mut hasher = DefaultHasher::new();
        hasher.write_u64(position_hash);
        match self {
            Self::Container(container) => {
                hasher.write_u64(Self::CONTAINER_HASH);
                if container.direction.is_reversed() {
                    let frames: SmallVec<[_; ARRAY_SIZE]> =
                        splits_iter(frame, container.direction, container.children.iter().rev())
                            .collect();
                    for (child, frame) in container.children.iter_mut().rev().zip(frames) {
                        // hasher.write_u64(Self::CONTAINER_ITEM_HASH);
                        child.node.0.crawl(frame, hasher.finish(), view_fn, draw_fn);
                    }
                } else {
                    let frames: SmallVec<[_; ARRAY_SIZE]> =
                        splits_iter(frame, container.direction, container.children.iter())
                            .collect();
                    for (child, frame) in container.children.iter_mut().zip(frames) {
                        // hasher.write_u64(Self::CONTAINER_ITEM_HASH);
                        child.node.0.crawl(frame, hasher.finish(), view_fn, draw_fn);
                    }
                }
            }
            Self::Component(template) => {
                template.component_type_id().hash(&mut hasher);
                template.key().map(|key| key.hash(&mut hasher));
                view_fn(LaidComponent {
                    frame,
                    position_hash: hasher.finish(),
                    template,
                });
            }
            Self::Canvas(canvas) => {
                hasher.write_u64(Self::CANVAS_HASH);
                draw_fn(LaidCanvas {
                    frame,
                    position_hash: hasher.finish(),
                    canvas,
                });
            }
        };
    }

    // Some random numbers to initialise the hash (0 & 1 would also do, but
    // hopefully this is less pathological if a simpler hash the `DefaultHasher`
    // was used).
    const CONTAINER_HASH: u64 = 0x5aa2d5349a05cde8;
    const CANVAS_HASH: u64 = 0x38c0758c1492cbf1;
}

impl From<Canvas> for Layout {
    fn from(canvas: Canvas) -> Self {
        Self(LayoutNode::Canvas(canvas))
    }
}

#[derive(Clone)]
pub struct Container {
    children: SmallVec<[Item; ARRAY_SIZE]>,
    direction: FlexDirection,
}

#[derive(Clone)]
pub struct Item {
    node: Layout,
    flex: FlexBasis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlexBasis {
    Auto,
    Fixed(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlexDirection {
    Column,
    ColumnReverse,
    Row,
    RowReverse,
}

impl FlexDirection {
    #[inline]
    pub fn is_reversed(&self) -> bool {
        match self {
            FlexDirection::Column | FlexDirection::Row => false,
            FlexDirection::ColumnReverse | FlexDirection::RowReverse => true,
        }
    }

    #[inline]
    pub(crate) fn dimension(self, size: Size) -> usize {
        match self {
            FlexDirection::Row => size.width,
            FlexDirection::RowReverse => size.width,
            FlexDirection::Column => size.height,
            FlexDirection::ColumnReverse => size.height,
        }
    }
}

pub(crate) struct LaidComponent<'a> {
    pub(crate) frame: Rect,
    pub(crate) position_hash: u64,
    pub(crate) template: &'a mut DynamicTemplate,
}

pub(crate) struct LaidCanvas<'a> {
    pub(crate) frame: Rect,
    pub(crate) position_hash: u64,
    pub(crate) canvas: &'a Canvas,
}

pub struct Items(SmallVec<[Item; ARRAY_SIZE]>);
const ARRAY_SIZE: usize = 4;

impl From<SmallVec<[Item; ARRAY_SIZE]>> for Items {
    #[inline]
    fn from(array: SmallVec<[Item; ARRAY_SIZE]>) -> Items {
        Self(array)
    }
}

impl From<[Item; 0]> for Items {
    #[inline]
    fn from(_array: [Item; 0]) -> Items {
        Self(SmallVec::new())
    }
}

impl From<[Item; 1]> for Items {
    #[inline]
    fn from(array: [Item; 1]) -> Items {
        match array {
            [x0] => Self(smallvec![x0]),
        }
    }
}

impl From<[Item; 2]> for Items {
    #[inline]
    fn from(array: [Item; 2]) -> Items {
        match array {
            [x0, x1] => Self(smallvec![x0, x1]),
        }
    }
}

impl From<[Item; 3]> for Items {
    #[inline]
    fn from(array: [Item; 3]) -> Items {
        match array {
            [x0, x1, x2] => Self(smallvec![x0, x1, x2]),
        }
    }
}

impl From<[Item; 4]> for Items {
    #[inline]
    fn from(array: [Item; 4]) -> Items {
        match array {
            [x0, x1, x2, x3] => Self(smallvec![x0, x1, x2, x3]),
        }
    }
}

#[inline]
fn splits_iter<'a>(
    frame: Rect,
    direction: FlexDirection,
    children: impl Iterator<Item = &'a Item> + Clone + 'a,
) -> impl Iterator<Item = Rect> + 'a {
    let total_size = direction.dimension(frame.size);

    // Compute how much space is available for stretched components
    let (stretched_budget, num_stretched_children, total_fixed_size) = {
        let mut stretched_budget = total_size;
        let mut num_stretched_children = 0;
        let mut total_fixed_size = 0;
        for child in children.clone() {
            match child.flex {
                FlexBasis::Auto => {
                    num_stretched_children += 1;
                }
                FlexBasis::Fixed(size) => {
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
        .map(move |child| match child.flex {
            FlexBasis::Auto => {
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
            FlexBasis::Fixed(size) => {
                let offset = total_size - remaining_size;
                let size = cmp::min(remaining_size, size);
                remaining_size -= size;
                (offset, size)
            }
        })
        .map(move |(offset, size)| match direction {
            FlexDirection::Row | FlexDirection::RowReverse => Rect::new(
                Position::new(frame.origin.x + offset, frame.origin.y),
                Size::new(size, frame.size.height),
            ),
            FlexDirection::Column | FlexDirection::ColumnReverse => Rect::new(
                Position::new(frame.origin.x, frame.origin.y + offset),
                Size::new(frame.size.width, size),
            ),
        })
}
