use crossbeam_channel::select;
use maplit::hashmap;
use once_cell::sync::Lazy;
use smallvec::{smallvec, SmallVec};
use std::{
    cmp,
    collections::HashMap,
    io, mem,
    ops::Deref,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use ttmap::TypeMap;

use crate::{
    components::{
        prompt::Command,
        theme::{Theme, THEMES},
        BindingMatch, Bindings, Buffer, Component, ComponentId, Context, Flex, HashBindings,
        LaidComponentId, LaidComponentIds, Layout, LayoutDirection, LayoutNode, LayoutNodeFlex,
        Prompt, Splash,
    },
    error::{Error, Result},
    frontend::Frontend,
    settings::Settings,
    task::{TaskId, TaskPool},
    terminal::{Key, Position, Rect, Screen},
};

type Components<T> = HashMap<ComponentId, T>;
type Buffers = Components<Buffer>;

pub struct Editor {
    // Components
    components: TypeMap,
    prompt: Prompt,

    // Components book keeping
    layout: Layout,
    laid_components: LaidComponentIds,
    task_owners: HashMap<TaskId, ComponentId>,
    focus: Option<usize>,
    next_component_id: ComponentId,
    task_pool: TaskPool,
    current_path: PathBuf,
    controller: InputController,

    // Theme palettes and currently selected theme
    themes: &'static [(Theme, &'static str); 30],
    theme_index: usize,
    settings: Settings,
}

#[derive(Clone, Debug)]
pub enum EditorAction {
    CycleFocus,
    ClosePane,
    ChangeTheme,
    Quit,
}

static EDITOR_BINDINGS: Lazy<HashBindings<EditorAction>> = Lazy::new(|| {
    HashBindings::new(hashmap! {
        smallvec![Key::Ctrl('x'), Key::Char('o')] => EditorAction::CycleFocus,
        smallvec![Key::Ctrl('x'), Key::Ctrl('o')] => EditorAction::CycleFocus,
        smallvec![Key::Ctrl('x'), Key::Char('0')] => EditorAction::ClosePane,
        smallvec![Key::Ctrl('t')] => EditorAction::ChangeTheme,
        smallvec![Key::Ctrl('x'), Key::Ctrl('c')] => EditorAction::Quit,
    })
});

impl Editor {
    pub fn new(settings: Settings, current_path: PathBuf, task_pool: TaskPool) -> Self {
        let prompt = Prompt::new();
        Self {
            components: TypeMap::new(),
            layout: wrap_layout_with_prompt(prompt.height(), None),
            prompt,
            laid_components: LaidComponentIds::new(),
            task_owners: HashMap::with_capacity(8),
            focus: None,
            next_component_id: cmp::max(PROMPT_ID, SPLASH_ID) + 1,
            task_pool,
            current_path,
            controller: InputController::new(),

            themes: &THEMES,
            theme_index: settings.theme_index,
            settings,
        }
    }

    fn add_component<ComponentT>(&mut self, component: ComponentT) -> ComponentId
    where
        ComponentT: Component + 'static,
    {
        let component_id = self.next_component_id;
        self.next_component_id += 1;

        self.components
            .get_or_default::<Components<ComponentT>>()
            .insert(component_id, component);
        self.focus.get_or_insert(component_id);

        let mut layout = Layout::Component(PROMPT_ID);
        mem::swap(&mut self.layout, &mut layout);
        self.layout = wrap_layout_with_prompt(
            self.prompt.height(),
            unwrap_prompt_from_layout(layout).map(|layout| {
                layout
                    .add_left(component_id, Flex::Stretched)
                    .remove_component_id(SPLASH_ID)
                    .unwrap()
            }),
        );

        component_id
    }

    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            self.prompt.log_error("[New file]".into());
        }

        match Buffer::from_file(path.to_owned()) {
            Ok(buffer) => {
                self.focus = Some(self.add_component(buffer));
            }
            Err(Error::Io(ref error)) if error.kind() == io::ErrorKind::PermissionDenied => {
                self.prompt.log_error(format!(
                    "Permission denied while opening {}",
                    path.display()
                ));
            }
            Err(Error::Io(ref error)) => {
                self.prompt
                    .log_error(format!("Could not open {} {}", path.display(), error));
            }
            error => {
                error?;
            }
        }
        Ok(())
    }

    pub fn ui_loop(&mut self, mut screen: Screen, mut frontend: impl Frontend) -> Result<()> {
        let mut average = 0.0;
        let mut n = 0;
        let mut last_drawn = Instant::now() - REDRAW_LATENCY;
        let mut frame = Rect::new(Position::new(0, 0), screen.size());
        let mut poll_state = PollState::Dirty;
        loop {
            match poll_state {
                PollState::Dirty => {
                    let now = Instant::now();
                    frame = Rect::new(Position::new(0, 0), screen.size());
                    screen.resize(frontend.size()?);
                    self.draw(&mut screen);
                    let drawn_time = now.elapsed();
                    n += 1;
                    average =
                        (average * (n as f64 - 1.0) + drawn_time.as_millis() as f64) / n as f64;

                    let now = Instant::now();
                    frontend.present(&screen)?;
                    last_drawn = Instant::now();
                    log::debug!(
                        "Drawn in {:?} | Presented in {:?} | average drawn {:.2}",
                        drawn_time,
                        now.elapsed(),
                        average
                    );
                }
                PollState::Exit => {
                    return Ok(());
                }
                _ => {}
            }

            poll_state = self.poll_events_batch(&frontend, frame, last_drawn)?;
        }
    }

    /// Poll as many events as we can respecting REDRAW_LATENCY and REDRAW_LATENCY_SUSTAINED_IO
    fn poll_events_batch(
        &mut self,
        frontend: &impl Frontend,
        frame: Rect,
        last_drawn: Instant,
    ) -> Result<PollState> {
        let mut force_redraw = false;
        let mut first_event_time: Option<Instant> = None;
        let mut dirty = false;

        while !force_redraw {
            let timeout = {
                let since_last_drawn = last_drawn.elapsed();
                if dirty && since_last_drawn >= REDRAW_LATENCY {
                    Duration::from_millis(0)
                } else if dirty {
                    REDRAW_LATENCY - since_last_drawn
                } else {
                    Duration::from_millis(60000)
                }
            };

            select! {
                recv(self.task_pool.receiver) -> task_result => {
                    let Self {
                        ref mut components,
                        ref mut prompt,
                        ref mut task_owners,
                        ref current_path,
                        ref settings,
                        ref task_pool,
                        ref themes,
                        ref focus,
                        theme_index,
                        ..
                    } = *self;
                    let task_result = task_result.map_err(anyhow::Error::from)?;
                    let component_id = task_owners.remove(&task_result.id);
                    let time = Instant::now();
                    let context = Context {
                        time,
                        focused: *focus == component_id,
                        frame,
                        frame_id: 0,  // TODO: refactor out context computation and see what to do about missing frame id
                        theme: &themes[theme_index].0,
                        path: current_path.as_path(),
                        settings,
                    };
                    if component_id == Some(PROMPT_ID) {
                        let mut scheduler = task_pool.scheduler();
                        if let Err(err) = prompt.reduce(task_result.unwrap_prompt().payload, &mut scheduler, &context) {
                            prompt.log_error(format!("{}", err));
                        }
                    } else if let Some(component) = component_id.and_then(
                        |component_id| components.get_or_default::<Buffers>().get_mut(&component_id)) {
                        let mut scheduler = task_pool.scheduler();
                        if let Err(err) = component.reduce(task_result.unwrap_buffer().payload, &mut scheduler, &context) {
                            prompt.log_error(format!("{}", err));
                        }
                    }
                    dirty = true; // notify_task_done should return whether we need to rerender
                }
                recv(frontend.events()) -> event => {
                    match event.map_err(anyhow::Error::from)? {
                        key => {
                            if self.handle_event(key, frame)? {
                                return Ok(PollState::Exit);
                            }
                            dirty = true; // handle_event should return whether we need to rerender
                        }
                    };
                    force_redraw = dirty
                        && first_event_time.get_or_insert_with(Instant::now).elapsed()
                        >= SUSTAINED_IO_REDRAW_LATENCY;
                }
                default(timeout) => {
                    force_redraw = true;
                }
            }
        }

        Ok(if dirty {
            PollState::Dirty
        } else {
            PollState::Clean
        })
    }

    #[inline]
    fn draw(&mut self, screen: &mut Screen) {
        let frame = Rect::new(Position::new(0, 0), screen.size());
        self.lay_components(frame);

        let Self {
            ref mut components,
            ref mut prompt,
            ref mut task_owners,
            ref current_path,
            ref focus,
            ref settings,
            ref task_pool,
            ref themes,
            theme_index,
            ..
        } = *self;
        let time = Instant::now();
        self.laid_components.iter().for_each(
            |&LaidComponentId {
                 id,
                 frame,
                 frame_id,
             }| {
                let context = Context {
                    time,
                    focused: false,
                    frame,
                    frame_id,
                    theme: &themes[theme_index].0,
                    path: current_path.as_path(),
                    settings,
                };

                if id == PROMPT_ID {
                    let mut scheduler = task_pool.scheduler();
                    prompt.draw(screen, &mut scheduler, &context)
                } else if id == SPLASH_ID {
                    let mut scheduler = task_pool.scheduler();
                    Splash::default().draw(screen, &mut scheduler, &context)
                } else {
                    let mut scheduler = task_pool.scheduler();
                    components
                        .get_or_default::<Buffers>()
                        .get_mut(&id)
                        .unwrap()
                        .draw(
                            screen,
                            &mut scheduler,
                            &context.set_focused(
                                focus
                                    .as_ref()
                                    .map(|focused_id| *focused_id == id && !prompt.is_active())
                                    .unwrap_or(false),
                            ),
                        );
                    for task_id in scheduler.scheduled() {
                        task_owners.insert(task_id, id);
                    }
                }
            },
        );
    }

    #[inline]
    fn handle_event(&mut self, key: Key, frame: Rect) -> Result<bool> {
        let time = Instant::now();
        let mut is_prefix_to_binding = false;
        self.controller.push(key);
        self.prompt.clear_log();

        if !self.prompt.is_active() {
            let editor_binding_match = self.controller.matches(EDITOR_BINDINGS.deref());
            is_prefix_to_binding = is_prefix_to_binding || editor_binding_match.is_prefix();
            match editor_binding_match {
                BindingMatch::Full(EditorAction::CycleFocus) => {
                    self.cycle_focus(frame, CycleFocus::Next);
                    return Ok(false);
                }
                BindingMatch::Full(EditorAction::ClosePane) => {
                    if let Some(focus) = self.focus {
                        let mut layout = Layout::Component(PROMPT_ID);
                        mem::swap(&mut self.layout, &mut layout);
                        self.layout = wrap_layout_with_prompt(
                            self.prompt.height(),
                            unwrap_prompt_from_layout(layout)
                                .and_then(|layout| layout.remove_component_id(focus)),
                        );
                        self.cycle_focus(frame, CycleFocus::Previous);
                    }
                    return Ok(false);
                }
                BindingMatch::Full(EditorAction::ChangeTheme) => {
                    self.theme_index = (self.theme_index + 1) % self.themes.len();
                    self.prompt.log_error(format!(
                        "Theme changed to {}",
                        self.themes[self.theme_index].1
                    ));
                    return Ok(false);
                }
                BindingMatch::Full(EditorAction::Quit) => {
                    log::info!("Exiting (user request)...");
                    return Ok(true);
                }
                _ => {}
            };

            if let Some(&id_with_focus) = self.focus.as_ref() {
                self.lay_components(frame);

                let Self {
                    ref mut components,
                    ref mut prompt,
                    ref mut task_owners,
                    ref mut current_path,
                    ref mut controller,
                    ref laid_components,
                    ref themes,
                    ref task_pool,
                    ref settings,
                    theme_index,
                    ..
                } = *self;
                laid_components.iter().try_for_each(
                    |&LaidComponentId {
                         id,
                         frame,
                         frame_id,
                     }|
                     -> Result<()> {
                        if id_with_focus != id {
                            return Ok(());
                        }

                        let mut scheduler = task_pool.scheduler();
                        let component =
                            components.get_or_default::<Buffers>().get_mut(&id).unwrap();
                        if let Some(path) = component.path() {
                            *current_path =
                                path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                        }
                        let binding_match = component
                            .bindings()
                            .map(|bindings| controller.matches(bindings));

                        is_prefix_to_binding = is_prefix_to_binding
                            || binding_match
                                .as_ref()
                                .map(|binding_match| binding_match.is_prefix())
                                .unwrap_or(false);

                        // log::info!("m: {:?} {}", binding_match, is_prefix_to_binding);
                        if let Some(BindingMatch::Full(action)) = binding_match {
                            if let Err(error) = component.reduce(
                                action,
                                &mut scheduler,
                                &Context {
                                    time,
                                    focused: true,
                                    frame,
                                    frame_id,
                                    theme: &themes[theme_index].0,
                                    path: current_path.as_path(),
                                    settings,
                                },
                            ) {
                                prompt.log_error(format!("{}", error));
                            }
                        }
                        for task_id in scheduler.scheduled() {
                            task_owners.insert(task_id, id);
                        }

                        Ok(())
                    },
                )?;
            }
        }

        // Update prompt
        {
            let Self {
                ref mut controller,
                ref mut current_path,
                ref mut prompt,
                ref mut task_owners,
                ref task_pool,
                ref themes,
                ref settings,
                theme_index,
                ..
            } = *self;

            let binding_match = prompt
                .bindings()
                .map(|bindings| controller.matches(bindings));

            is_prefix_to_binding = is_prefix_to_binding
                || binding_match
                    .as_ref()
                    .map(|binding_match| binding_match.is_prefix())
                    .unwrap_or(false);

            if let Some(BindingMatch::Full(action)) = binding_match {
                let mut scheduler = task_pool.scheduler();
                prompt.reduce(
                    action,
                    &mut scheduler,
                    &Context {
                        time,
                        focused: false,
                        frame,
                        frame_id: 0,
                        theme: &themes[theme_index].0,
                        path: current_path.as_path(),
                        settings,
                    },
                )?;
                for task_id in scheduler.scheduled() {
                    task_owners.insert(task_id, PROMPT_ID);
                }
            }
        }
        if let Some(Command::OpenFile(path)) = self.prompt.poll_and_clear() {
            self.open_file(path)?;
        }

        if key == Key::Ctrl('g') {
            self.prompt.log_error("Cancel".into());
            self.controller.keys.clear();
        } else if !self.controller.keys.is_empty() {
            if !is_prefix_to_binding {
                self.prompt
                    .log_error(format!("{}is undefined", self.controller));
                self.controller.keys.clear();
            } else {
                self.prompt.log_error(format!("{}", self.controller));
            }
        }

        Ok(false)
    }

    #[inline]
    fn lay_components(&mut self, frame: Rect) {
        let mut layout = Layout::Component(PROMPT_ID);
        mem::swap(&mut self.layout, &mut layout);
        self.layout =
            wrap_layout_with_prompt(self.prompt.height(), unwrap_prompt_from_layout(layout));
        self.laid_components.clear();
        self.layout
            .compute(frame, &mut 1, &mut self.laid_components);
    }

    #[inline]
    fn cycle_focus(&mut self, frame: Rect, direction: CycleFocus) {
        self.lay_components(frame);
        while let Some(index) = self
            .laid_components
            .iter()
            .position(|laid| laid.id == SPLASH_ID || laid.id == PROMPT_ID)
        {
            self.laid_components.swap_remove(index);
        }
        self.laid_components.sort_by_key(|laid| laid.frame_id);

        if self.laid_components.is_empty() {
            self.focus = None
        } else {
            let index = self
                .focus
                .map(|focus| {
                    self.laid_components
                        .iter()
                        .position(|laid| laid.id == focus)
                        .unwrap_or(0)
                })
                .unwrap_or(0);

            let next_index = match direction {
                CycleFocus::Next => index + 1,
                CycleFocus::Previous => self.laid_components.len() + index - 1,
            } % self.laid_components.len();
            self.focus = Some(self.laid_components[next_index].id);
        }
    }
}

enum PollState {
    Clean,
    Dirty,
    Exit,
}

enum CycleFocus {
    Next,
    Previous,
}

#[inline]
fn wrap_layout_with_prompt(prompt_height: usize, layout: Option<Layout>) -> Layout {
    Layout::vertical(
        LayoutNodeFlex {
            node: layout.unwrap_or_else(|| Layout::Component(SPLASH_ID)),
            flex: Flex::Stretched,
        },
        LayoutNodeFlex {
            node: Layout::Component(PROMPT_ID),
            flex: Flex::Fixed(prompt_height),
        },
    )
}

#[inline]
fn unwrap_prompt_from_layout(layout: Layout) -> Option<Layout> {
    match layout {
        Layout::Component(PROMPT_ID) => None,
        Layout::Node(node) => match *node {
            LayoutNode {
                direction: LayoutDirection::Vertical,
                children,
            } => Some(children[0].node.clone()),
            _ => None,
        },
        _ => None,
    }
}

const PROMPT_ID: ComponentId = 0;
const SPLASH_ID: ComponentId = 1;

const REDRAW_LATENCY: Duration = Duration::from_millis(10);
const SUSTAINED_IO_REDRAW_LATENCY: Duration = Duration::from_millis(100);

struct InputController {
    keys: SmallVec<[Key; 8]>,
}

impl InputController {
    fn new() -> Self {
        Self {
            keys: SmallVec::new(),
        }
    }

    fn push(&mut self, key: Key) {
        self.keys.push(key);
        log::info!("keys: {:?}", self.keys);
    }

    fn matches<Action>(&mut self, bindings: &impl Bindings<Action>) -> BindingMatch<Action> {
        let binding_match = bindings.matches(&self.keys);
        if let BindingMatch::Full(_) = binding_match {
            self.keys.clear();
        }
        binding_match
    }
}

impl std::fmt::Display for InputController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        for key in self.keys.iter() {
            match key {
                Key::Char(char) => write!(formatter, "{} ", char)?,
                Key::Ctrl(char) => write!(formatter, "C-{} ", char)?,
                Key::Alt(char) => write!(formatter, "A-{} ", char)?,
                key => write!(formatter, "{:?} ", key)?,
            }
        }
        Ok(())
    }
}
