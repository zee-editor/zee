use std::{
    fmt::Display,
    ops::{Add, Rem},
};
use zi::{Container, FlexDirection, Item, Layout};

pub(super) enum CycleFocus {
    Next,
    Previous,
}

pub(super) struct Window<IdT> {
    pub id: IdT,
    pub focused: bool,
    pub index: WindowIndex,
}

pub(super) struct WindowTree<IdT> {
    nodes: Vec<Node<IdT>>,
    focused_index: WindowIndex,
    num_windows: WindowIndex,
}

impl<IdT: Clone + Copy + Display> WindowTree<IdT> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            focused_index: WindowIndex(0),
            num_windows: WindowIndex(0),
        }
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.focused_index = WindowIndex(0);
        self.num_windows = WindowIndex(0);
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut IdT> {
        self.nodes.iter_mut().filter_map(|node| match node {
            Node::Window(id) => Some(id),
            _ => None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.num_windows == WindowIndex(0)
    }

    pub fn add(&mut self, id: IdT) {
        self.nodes.push(Node::Window(id));
        self.focused_index = self.num_windows; // Focus the newly added window
        self.num_windows = self.num_windows.increment();
    }

    pub fn delete_focused(&mut self) {
        let focused = self.find_focused_window();
        self.nodes.remove(focused.node_index);
        self.num_windows = self.num_windows.saturating_decrement();
        self.focused_index = self.focused_index.saturating_decrement();

        let mut node_index = 0;
        while node_index < self.nodes.len() {
            match self.nodes[node_index..] {
                [Node::ContainerStart(_), window @ Node::Window(_), Node::ContainerEnd, ..] => {
                    self.nodes
                        .splice(node_index..node_index + 3, std::iter::once(window));
                }
                [Node::ContainerStart(_), Node::ContainerEnd, ..] => {
                    self.nodes.drain(node_index..node_index + 2);
                }
                _ => {
                    node_index += 1;
                }
            };
        }
    }

    pub fn delete_all_except_focused(&mut self) {
        let focused = self.nodes.remove(self.find_focused_window().node_index);
        self.nodes.clear();
        self.nodes.push(focused);
        self.focused_index = WindowIndex(0);
        self.num_windows = WindowIndex(1);
    }

    pub fn insert_at_focused(&mut self, id: IdT, direction: FlexDirection) {
        if self.num_windows == WindowIndex(0) {
            return;
        }

        let focused = self.find_focused_window();
        self.nodes.insert(focused.node_index + 1, Node::Window(id));
        if direction != focused.direction {
            self.nodes
                .insert(focused.node_index, Node::ContainerStart(direction));
            self.nodes
                .insert(focused.node_index + 3, Node::ContainerEnd);
        }

        // TODO: Should the newly created window be focused? Emacs doesn't, but often I wish it did
        //
        // self.focused_index = self.focused_index.increment();
        self.num_windows = self.num_windows.increment();
    }

    pub fn cycle_focus(&mut self, direction: CycleFocus) {
        if self.num_windows == WindowIndex(0) {
            return;
        }

        match direction {
            CycleFocus::Next => {
                self.focused_index = self.focused_index.increment() % self.num_windows;
            }
            CycleFocus::Previous => {
                self.focused_index = (self.num_windows + self.focused_index).saturating_decrement()
                    % self.num_windows;
            }
        }
    }

    pub fn layout(&self, lay_component: &mut impl FnMut(Window<IdT>) -> Layout) -> Layout {
        let mut container_stack = Vec::new();
        let mut container = Container::empty(FlexDirection::Row);
        let mut window_index = WindowIndex(0);

        for window in self.nodes.iter() {
            match window {
                Node::Window(id) => {
                    container.push(Item::auto(lay_component(Window {
                        id: *id,
                        focused: window_index == self.focused_index,
                        index: window_index,
                    })));
                    window_index = window_index.increment();
                }
                Node::ContainerStart(direction) => {
                    container_stack.push(container);
                    container = Container::empty(*direction);
                }
                Node::ContainerEnd => {
                    container_stack
                        .last_mut()
                        .unwrap()
                        .push(Item::auto(container));
                    container = container_stack.pop().unwrap();
                }
            }
        }

        assert!(container_stack.is_empty());
        container.into()
    }

    pub fn get_focused(&self) -> Option<IdT> {
        let mut window_index = self.focused_index;
        for window in self.nodes.iter() {
            if let Node::Window(id) = window {
                if window_index == WindowIndex(0) {
                    return Some(*id);
                }
                window_index = window_index.saturating_decrement();
            }
        }
        None
    }

    pub fn set_focused(&mut self, id: IdT) {
        let mut window_index = self.focused_index;
        for window in self.nodes.iter_mut() {
            if let Node::Window(current_id) = window {
                if window_index == WindowIndex(0) {
                    *current_id = id;
                    return;
                } else {
                    window_index = window_index.saturating_decrement();
                }
            }
        }
    }

    fn find_focused_window(&self) -> NodeRef {
        self.find_window_node(self.focused_index)
    }

    fn find_window_node(&self, mut window_index: WindowIndex) -> NodeRef {
        let mut container_stack = vec![FlexDirection::Row];
        for (node_index, node) in self.nodes.iter().enumerate() {
            match node {
                Node::Window(_) => {
                    if window_index == WindowIndex(0) {
                        return NodeRef {
                            direction: container_stack.pop().unwrap(),
                            node_index,
                        };
                    }
                    window_index = window_index.saturating_decrement();
                }
                Node::ContainerStart(direction) => {
                    container_stack.push(*direction);
                }
                Node::ContainerEnd => {
                    container_stack.pop();
                }
            }
        }
        assert_eq!(container_stack.len(), 1);
        NodeRef {
            direction: FlexDirection::Row,
            node_index: 0,
        }
    }
}

struct NodeRef {
    direction: FlexDirection,
    node_index: usize,
}

#[derive(Clone, Copy, Debug)]
enum Node<IdT> {
    Window(IdT),
    ContainerStart(FlexDirection),
    ContainerEnd,
}

impl<IdT: Display> Display for Node<IdT> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Node::*;
        match self {
            Window(id) => write!(formatter, "<{}/>", id),
            ContainerStart(direction) => write!(formatter, "<Container {:?}>", direction),
            ContainerEnd => write!(formatter, "</Container>"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub(super) struct WindowIndex(usize);

impl WindowIndex {
    pub fn one_based_index(&self) -> usize {
        self.0 + 1
    }

    fn saturating_decrement(self) -> Self {
        Self(usize::saturating_sub(self.0, 1))
    }

    fn increment(self) -> Self {
        Self(self.0 + 1)
    }
}

impl Display for WindowIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Add for WindowIndex {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Rem<Self> for WindowIndex {
    type Output = Self;

    fn rem(self, modulus: Self) -> Self::Output {
        Self(self.0 % modulus.0)
    }
}
