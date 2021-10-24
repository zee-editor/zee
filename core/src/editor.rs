use git2::Repository;
use ropey::Rope;
use std::{
    borrow::Cow,
    collections::hash_map::HashMap,
    fmt::Display,
    fs::File,
    io::{self, BufReader},
    iter,
    ops::{Add, Rem},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};
use zi::{
    BindingMatch, BindingTransition, Callback, Component, ComponentExt, ComponentLink, Container,
    FlexBasis, FlexDirection, Item, Key, Layout, Rect, ShouldRender,
};

use crate::{
    clipboard::Clipboard,
    components::{
        buffer::{Buffer, Properties as BufferProperties, RepositoryRc},
        prompt::{
            buffers::BufferEntry, picker::FileSource, Action as PromptAction, Prompt,
            Properties as PromptProperties, PROMPT_INACTIVE_HEIGHT,
        },
        splash::{Properties as SplashProperties, Splash},
        theme::{Theme, THEMES},
    },
    error::Result,
    mode::{self, Mode},
    settings::Settings,
    task::TaskPool,
};

#[derive(Clone, Debug)]
pub enum Message {
    ChangeTheme,
    ClosePane,
    FocusNextComponent,
    FocusPreviousComponent,
    SplitWindow(FlexDirection),
    FullscreenWindow,
    KeyPressed,
    OpenBufferSwitcher,
    ChangePromptHeight(usize),
    OpenFilePicker(FileSource),
    OpenFile(PathBuf),
    SelectBuffer(BufferId),
    Log(Option<String>),
    Cancel,
    Quit,
}

pub struct Context {
    pub args_files: Vec<PathBuf>,
    pub current_working_dir: PathBuf,
    pub settings: Settings,
    pub task_pool: TaskPool,
    pub clipboard: Arc<dyn Clipboard>,
}

pub struct Editor {
    context: Rc<Context>,
    link: ComponentLink<Self>,
    themes: &'static [(Theme, &'static str)],
    theme_index: usize,

    prompt_action: PromptAction,
    prompt_height: usize,

    buffers: HashMap<BufferId, OpenBuffer>,
    next_buffer_id: usize,
    windows: WindowTree<BufferId>,
    log_message: Callback<String>,
}

pub struct OpenBuffer {
    mode: &'static Mode,
    repo: Option<RepositoryRc>,
    content: Rope,
    file_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferId(usize);

impl Display for BufferId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "BufferId({})", self.0)
    }
}

enum CycleFocus {
    Next,
    Previous,
}

impl Editor {
    fn open_file(&mut self, file_path: PathBuf) -> Result<bool> {
        // Check if the buffer is already open
        if let Some(buffer_id) = self
            .buffers
            .iter()
            .find(|(_, buffer)| {
                buffer
                    .file_path
                    .as_ref()
                    .map(|buffer_path| *buffer_path == file_path)
                    .unwrap_or(false)
            })
            .map(|(buffer_id, _)| buffer_id)
        {
            if self.windows.is_empty() {
                self.windows.add(*buffer_id);
            } else {
                self.windows.set_focused(*buffer_id);
            }
            return Ok(false);
        }

        let mode = mode::find_by_filename(&file_path);
        let repo = Repository::discover(&file_path).ok();
        let (is_new_file, text) = if file_path.exists() {
            (
                false,
                Rope::from_reader(BufReader::new(File::open(&file_path)?))?,
            )
        } else {
            // Optimistically check if we can create it
            let is_new_file = File::open(&file_path)
                .map(|_| false)
                .or_else(|error| match error.kind() {
                    io::ErrorKind::NotFound => {
                        self.log_message("[New file]".into());
                        Ok(true)
                    }
                    io::ErrorKind::PermissionDenied => {
                        self.log_message(format!(
                            "Permission denied while opening {}",
                            file_path.display()
                        ));
                        Err(error)
                    }
                    _ => {
                        self.log_message(format!(
                            "Could not open {} ({})",
                            file_path.display(),
                            error
                        ));
                        Err(error)
                    }
                })?;
            (is_new_file, Rope::new())
        };
        // Generate a new buffer id
        let buffer_id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;

        // Store the new buffer
        self.buffers.insert(
            buffer_id,
            OpenBuffer {
                mode,
                repo: repo.map(RepositoryRc::new),
                content: text,
                file_path: Some(file_path),
            },
        );

        // Create a new window for the buffer
        if self.windows.is_empty() {
            self.windows.add(buffer_id);
        } else {
            self.windows.set_focused(buffer_id);
        }

        Ok(is_new_file)
    }

    fn log_message(&mut self, message: String) {
        self.prompt_action = PromptAction::Log { message };
        self.prompt_height = self.prompt_action.initial_height();
    }
}

impl Component for Editor {
    type Message = Message;
    type Properties = Rc<Context>;

    fn create(properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        for file_path in properties.args_files.iter().cloned() {
            link.send(Message::OpenFile(file_path));
        }
        let log_message = link.callback(|message| Message::Log(Some(message)));

        Self {
            context: properties,
            link,
            themes: &THEMES,
            theme_index: 0,
            prompt_action: PromptAction::None,
            prompt_height: PROMPT_INACTIVE_HEIGHT,
            buffers: HashMap::new(),
            next_buffer_id: 0,
            windows: WindowTree::new(),
            log_message,
        }
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        match message {
            Message::Cancel => {
                self.log_message("Cancel".into());
            }
            Message::ChangeTheme => {
                self.theme_index = (self.theme_index + 1) % self.themes.len();
                if !self.prompt_action.is_interactive() {
                    self.log_message(format!(
                        "Theme changed to {}",
                        self.themes[self.theme_index].1
                    ))
                }
            }
            Message::OpenFilePicker(source) if !self.prompt_action.is_interactive() => {
                self.prompt_action = PromptAction::OpenFile {
                    source,
                    on_open: self.link.callback(Message::OpenFile),
                    on_change_height: self.link.callback(Message::ChangePromptHeight),
                };
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::OpenFile(path) => {
                self.prompt_action = self.open_file(path).map_or_else(
                    |error| PromptAction::Log {
                        message: format!("Could not open file: {}", error.to_string()),
                    },
                    |new_file| {
                        if new_file {
                            PromptAction::Log {
                                message: "[New file]".into(),
                            }
                        } else {
                            PromptAction::None
                        }
                    },
                );
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::OpenBufferSwitcher if !self.prompt_action.is_interactive() => {
                self.prompt_action = PromptAction::SwitchBuffer {
                    entries: self
                        .buffers
                        .iter()
                        .map(|(id, buffer)| {
                            BufferEntry::new(
                                *id,
                                buffer.file_path.clone(),
                                false,
                                buffer.content.len_bytes(),
                                buffer.mode,
                            )
                        })
                        .collect(),
                    on_select: self.link.callback(Message::SelectBuffer),
                    on_change_height: self.link.callback(Message::ChangePromptHeight),
                };
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::SelectBuffer(buffer_id) => {
                self.prompt_action = PromptAction::None;
                self.prompt_height = self.prompt_action.initial_height();
                if self.windows.is_empty() {
                    self.windows.add(buffer_id);
                } else {
                    self.windows.set_focused(buffer_id);
                }
            }
            Message::ChangePromptHeight(height) => {
                self.prompt_height = height;
            }
            Message::FocusNextComponent => self.windows.cycle_focus(CycleFocus::Next),
            Message::FocusPreviousComponent => self.windows.cycle_focus(CycleFocus::Previous),
            Message::SplitWindow(direction) if !self.buffers.is_empty() => {
                if let Some(buffer_id) = self.windows.get_focused() {
                    self.windows.insert_at_focused(buffer_id, direction);
                }
            }
            Message::FullscreenWindow if !self.buffers.is_empty() => {
                self.windows.close_all_except_focused();
            }
            Message::ClosePane if !self.buffers.is_empty() => {
                self.windows.close_focused();
            }
            Message::Log(message) if !self.prompt_action.is_interactive() => {
                self.prompt_action = message
                    .map(|message| PromptAction::Log { message })
                    .unwrap_or(PromptAction::None);
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::Quit => {
                self.link.exit();
            }
            _ => {}
        }
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let buffers = if self.windows.is_empty() {
            Splash::item_with_key(
                FlexBasis::Auto,
                "splash",
                SplashProperties {
                    theme: Cow::Borrowed(&self.themes[self.theme_index].0.splash),
                },
            )
        } else {
            Item::auto(self.windows.layout(&mut |Window { id, focused, index }| {
                let buffer = self.buffers.get(&id).unwrap();
                Buffer::with_key(
                    format!("{}.{}", index.0, id.0).as_str(),
                    BufferProperties {
                        context: self.context.clone(),
                        theme: Cow::Borrowed(&self.themes[self.theme_index].0.buffer),
                        focused: focused && !self.prompt_action.is_interactive(),
                        frame_id: index.one_based_index(),
                        mode: buffer.mode,
                        repo: buffer.repo.clone(),
                        content: buffer.content.clone(),
                        file_path: buffer.file_path.clone(),
                        log_message: self.log_message.clone(),
                    },
                )
            }))
        };

        Layout::column([
            buffers,
            Prompt::item_with_key(
                FlexBasis::Fixed(if self.prompt_action.is_none() {
                    PROMPT_INACTIVE_HEIGHT
                } else {
                    self.prompt_height
                }),
                "prompt",
                PromptProperties {
                    context: self.context.clone(),
                    theme: Cow::Borrowed(&self.themes[self.theme_index].0.prompt),
                    action: self.prompt_action.clone(),
                    // on_change: link.callback(Message::PromptStateChange),
                    // on_open_file: link.callback(Message::OpenFile),
                    // message: self.prompt_message.clone(),
                },
            ),
        ])
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let transition = BindingTransition::Clear;

        let message = match pressed {
            [Key::Ctrl('g')] => Message::Cancel,

            // Open a file
            [Key::Ctrl('x'), Key::Ctrl('f')] => Message::OpenFilePicker(FileSource::Directory),
            [Key::Ctrl('x'), Key::Ctrl('v')] => Message::OpenFilePicker(FileSource::Repository),

            // Buffer management
            [Key::Ctrl('x'), Key::Char('b') | Key::Ctrl('b')] => Message::OpenBufferSwitcher,
            [Key::Ctrl('x'), Key::Char('o') | Key::Ctrl('o')] => Message::FocusNextComponent,
            [Key::Ctrl('x'), Key::Char('i') | Key::Ctrl('i') | Key::Char('O') | Key::Ctrl('O')] => {
                Message::FocusPreviousComponent
            }
            // Window management
            [Key::Ctrl('x'), Key::Char('1') | Key::Ctrl('1')] => Message::FullscreenWindow,
            [Key::Ctrl('x'), Key::Char('2') | Key::Ctrl('2')] => {
                Message::SplitWindow(FlexDirection::Column)
            }
            [Key::Ctrl('x'), Key::Char('3') | Key::Ctrl('3')] => {
                Message::SplitWindow(FlexDirection::Row)
            }
            [Key::Ctrl('x'), Key::Char('0') | Key::Ctrl('0')] => Message::ClosePane,

            // Theme
            [Key::Ctrl('t')] => Message::ChangeTheme,

            // Quit
            [Key::Ctrl('x'), Key::Ctrl('c')] => Message::Quit,
            _ => {
                if let PromptAction::Log { .. } = self.prompt_action {
                    self.link.send(Message::Log(None));
                };
                return BindingMatch {
                    transition: BindingTransition::Continue,
                    message: Some(Self::Message::KeyPressed),
                };
            }
        };
        BindingMatch {
            transition,
            message: Some(message),
        }
    }
}

pub struct Window<IdT> {
    id: IdT,
    focused: bool,
    index: WindowIndex,
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
struct WindowIndex(usize);

impl WindowIndex {
    fn saturating_decrement(self) -> Self {
        Self(usize::saturating_sub(self.0, 1))
    }

    fn increment(self) -> Self {
        Self(self.0 + 1)
    }

    fn one_based_index(&self) -> usize {
        self.0 + 1
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

struct WindowTree<IdT> {
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

    pub fn is_empty(&self) -> bool {
        self.num_windows == WindowIndex(0)
    }

    pub fn add(&mut self, id: IdT) {
        self.nodes.push(Node::Window(id));
        self.focused_index = self.num_windows; // Focus the newly added window
        self.num_windows = self.num_windows.increment();
    }

    pub fn close_focused(&mut self) {
        let focused = self.find_focused_window();
        self.nodes.remove(focused.node_index);
        self.num_windows = self.num_windows.saturating_decrement();
        self.focused_index = self.focused_index.saturating_decrement();

        let mut node_index = 0;
        while node_index < self.nodes.len() {
            match self.nodes[node_index..] {
                [Node::ContainerStart(_), window @ Node::Window(_), Node::ContainerEnd, ..] => {
                    self.nodes
                        .splice(node_index..node_index + 3, iter::once(window));
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

    pub fn close_all_except_focused(&mut self) {
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

        self.focused_index = self.focused_index.increment();
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
