use crate::{
    components::{
        prompt::Command,
        theme::{Theme, THEMES},
        Buffer, Component, ComponentId, Context, Flex, LaidComponentId, LaidComponentIds, Layout,
        LayoutDirection, LayoutNode, LayoutNodeFlex, Prompt, Splash, TaskKind,
    },
    error::{Error, Result},
    frontend::Frontend,
    task::{TaskId, TaskPool},
    terminal::{Key, Position, Rect, Screen},
};
use crossbeam_channel::select;
use std::{
    cmp,
    collections::HashMap,
    io, mem,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(crate) struct Editor {
    components: HashMap<ComponentId, Box<dyn Component>>,
    task_owners: HashMap<TaskId, ComponentId>,
    layout: Layout,
    laid_components: LaidComponentIds,
    next_component_id: ComponentId,
    focus: Option<usize>,
    prompt: Prompt,
    task_pool: TaskPool<Result<TaskKind>>,
    themes: &'static [(Theme, &'static str); 30],
    theme_index: usize,
    current_path: PathBuf,
}

impl Editor {
    pub fn new(current_path: PathBuf, task_pool: TaskPool<Result<TaskKind>>) -> Self {
        let prompt = Prompt::new();
        Self {
            components: HashMap::with_capacity(8),
            task_owners: HashMap::with_capacity(8),
            layout: wrap_layout_with_prompt(prompt.height(), None),
            laid_components: LaidComponentIds::new(),
            next_component_id: cmp::max(PROMPT_ID, SPLASH_ID) + 1,
            focus: None,
            prompt,
            task_pool,
            themes: &THEMES,
            theme_index: 0,
            current_path,
        }
    }

    pub fn add_component(&mut self, component: impl Component + 'static) -> ComponentId {
        let component_id = self.next_component_id;
        self.next_component_id += 1;

        self.components
            .insert(component_id, Box::new(component) as Box<dyn Component>);
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
                    eprintln!(
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
                    let task_result = task_result.unwrap();
                    let component_id = self.task_owners.remove(&task_result.id);
                    if let Err(err) = task_result.payload.as_ref() {
                        self.prompt.log_error(format!("{}", err));
                    }
                    if component_id == Some(PROMPT_ID) {
                        self.prompt.task_done(task_result)?;
                    } else if let Some(component) = component_id.and_then(|component_id| self.components.get_mut(&component_id)) {
                        component.task_done(task_result)?;
                    }
                    dirty = true; // notify_task_done should return whether we need to rerender
                }
                recv(frontend.events()) -> event => {
                    match event.unwrap() {
                        Key::Ctrl('c') => {
                            return Ok(PollState::Exit);
                        }
                        key => {
                            self.handle_event(key, frame)?;
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
            ref mut task_owners,
            ref mut prompt,
            ref focus,
            ref themes,
            ref task_pool,
            ref current_path,
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
                };
                let mut scheduler = task_pool.scheduler();

                if id == PROMPT_ID {
                    prompt.draw(screen, &mut scheduler, &context)
                } else if id == SPLASH_ID {
                    Splash::default().draw(screen, &mut scheduler, &context)
                } else {
                    components.get_mut(&id).unwrap().draw(
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
    fn handle_event(&mut self, key: Key, frame: Rect) -> Result<()> {
        let time = Instant::now();
        self.prompt.clear_log();
        if !self.prompt.is_active() {
            match key {
                Key::Ctrl('o') => {
                    self.cycle_focus(frame, CycleFocus::Next);
                    return Ok(());
                }
                Key::Ctrl('q') => {
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
                    return Ok(());
                }
                Key::Ctrl('t') => {
                    self.theme_index = (self.theme_index + 1) % self.themes.len();
                    self.prompt.log_error(format!(
                        "Theme changed to {}",
                        self.themes[self.theme_index].1
                    ));
                    return Ok(());
                }

                _ => {}
            };

            if let Some(&id_with_focus) = self.focus.as_ref() {
                self.lay_components(frame);

                let Self {
                    ref mut components,
                    ref mut task_owners,
                    ref mut prompt,
                    ref laid_components,
                    ref themes,
                    theme_index,
                    ref task_pool,
                    ref mut current_path,
                    ..
                } = *self;
                laid_components.iter().try_for_each(
                    |&LaidComponentId {
                         id,
                         frame,
                         frame_id,
                     }|
                     -> Result<()> {
                        if id_with_focus == id {
                            let mut scheduler = task_pool.scheduler();
                            let component = components.get_mut(&id).unwrap();
                            if let Some(path) = component.path() {
                                *current_path = path.canonicalize()?;
                            }

                            if let Err(error) = component.handle_event(
                                key,
                                &mut scheduler,
                                &Context {
                                    time,
                                    focused: true,
                                    frame,
                                    frame_id,
                                    theme: &themes[theme_index].0,
                                    path: current_path.as_path(),
                                },
                            ) {
                                prompt.log_error(format!("{}", error));
                            }
                            for task_id in scheduler.scheduled() {
                                task_owners.insert(task_id, id);
                            }
                        }
                        Ok(())
                    },
                )?;
            }
        }

        // Update prompt
        let mut scheduler = self.task_pool.scheduler();
        self.prompt.handle_event(
            key,
            &mut scheduler,
            &Context {
                time,
                focused: false,
                frame,
                frame_id: 0,
                theme: &self.themes[self.theme_index].0,
                path: self.current_path.as_path(),
            },
        )?;
        for task_id in scheduler.scheduled() {
            self.task_owners.insert(task_id, PROMPT_ID);
        }
        if let Some(Command::OpenFile(path)) = self.prompt.poll_and_clear() {
            self.open_file(path)?;
        }

        Ok(())
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

        if self.laid_components.len() == 0 {
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
