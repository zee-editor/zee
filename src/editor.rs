use crossbeam_channel::select;
use std::{
    cmp,
    collections::HashMap,
    io, mem,
    path::Path,
    time::{Duration, Instant},
};
use syntect::{
    highlighting::ThemeSet as SyntaxThemeSet,
    parsing::{SyntaxSet, SyntaxSetBuilder},
};
use termion::{self, event::Key};

use crate::{
    error::{Error, Result},
    jobs::JobPool,
    settings::Paths,
    ui::{
        components::{
            prompt::Command, theme::Theme, Buffer, Component, ComponentId, ComponentTask, Context,
            Flex, LaidComponentId, LaidComponentIds, Layout, LayoutDirection, LayoutNode,
            LayoutNodeFlex, Prompt, Splash,
        },
        input::Input,
        Position, Rect, Screen, Size,
    },
};

pub(crate) struct Editor {
    components: HashMap<ComponentId, Box<dyn Component>>,
    layout: Layout,
    laid_components: LaidComponentIds,
    next_component_id: ComponentId,
    focus: Option<usize>,
    prompt: Prompt,
    job_pool: JobPool<Result<ComponentTask>>,
    themes: [(Theme, &'static str, &'static str); 3],
    theme_index: usize,
    syntax_set: SyntaxSet,
    syntax_theme_set: SyntaxThemeSet,
}

impl Editor {
    pub fn new(settings: Paths, job_pool: JobPool<Result<ComponentTask>>) -> Self {
        let mut builder = SyntaxSetBuilder::new();
        builder
            .add_from_folder(settings.syntax_definitions, true)
            .unwrap();
        builder.add_plain_text_syntax();
        let syntax_set = builder.build();

        let mut syntax_theme_set = SyntaxThemeSet::load_defaults();
        syntax_theme_set
            .add_from_folder(settings.syntax_themes)
            .unwrap();

        Self {
            components: HashMap::with_capacity(8),
            layout: wrap_layout_with_prompt(None),
            laid_components: LaidComponentIds::new(),
            next_component_id: cmp::max(PROMPT_ID, SPLASH_ID) + 1,
            focus: None,
            prompt: Prompt::new(),
            job_pool,
            themes: [
                (Theme::gruvbox(), "gruvbox-dark-soft", "gruvbox-dark-soft"),
                (Theme::solarized(), "solarized-dark", "Solarized (dark)"),
                (Theme::gruvbox(), "gruvbox-mocha", "base16-mocha.dark"),
            ],
            theme_index: 0,
            syntax_set,
            syntax_theme_set,
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
        self.layout = wrap_layout_with_prompt(unwrap_prompt_from_layout(layout).map(|layout| {
            layout
                .add_left(component_id, Flex::Stretched)
                .remove_component_id(SPLASH_ID)
                .unwrap()
        }));

        component_id
    }

    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            self.prompt.log_error("[New file]".into());
        }
        let syntax_theme = self
            .syntax_theme_set
            .themes
            .get(self.themes[self.theme_index].2)
            .ok_or::<Error>(Error::MissingSyntectTheme(
                self.themes[self.theme_index].2.into(),
            ))?
            .clone();

        match Buffer::from_file(path.to_owned(), self.syntax_set.clone(), syntax_theme) {
            Ok(buffer) => {
                self.focus = Some(self.add_component(buffer));
            }
            Err(Error::Io(ref error)) if error.kind() == io::ErrorKind::PermissionDenied => {
                self.prompt.log_error(format!(
                    "Permission denied while opening {}",
                    path.display()
                ));
            }
            error => {
                error?;
            }
        }
        Ok(())
    }

    pub fn ui_loop(&mut self, mut screen: Screen) -> Result<()> {
        let input = Input::from_reader(termion::get_tty()?);
        let mut last_drawn = Instant::now() - REDRAW_LATENCY;
        let mut frame = Rect::new(Position::new(0, 0), Size::new(screen.width, screen.height));
        let mut poll_state = PollState::Dirty;
        loop {
            match poll_state {
                PollState::Dirty => {
                    frame = Rect::new(Position::new(0, 0), Size::new(screen.width, screen.height));
                    screen.resize_to_terminal()?;
                    self.draw(&mut screen);
                    screen.present()?;
                    last_drawn = Instant::now();
                }
                PollState::Exit => {
                    return Ok(());
                }
                _ => {}
            }

            poll_state = self.poll_events_batch(&input, frame, last_drawn)?;
        }
    }

    /// Poll as many events as we can respecting REDRAW_LATENCY and REDRAW_LATENCY_SUSTAINED_IO
    fn poll_events_batch(
        &mut self,
        input: &Input,
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
                recv(self.job_pool.receiver) -> response => {
                    match response.unwrap().payload {
                        Ok(payload) => self.notify_task_done(payload)?,
                        Err(err) => self.prompt.log_error(format!("{}", err)),
                    }
                    dirty = true; // notify_task_done should return whether we need to rerender
                }
                recv(input.receiver) -> event => {
                    match event.unwrap() {
                        Key::Ctrl('c') => {
                            return Ok(PollState::Exit);
                        }
                        key => {
                            self.key_press(key, frame)?;
                            dirty = true; // key_press should return whether we need to rerender
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
        let Self {
            ref layout,
            ref mut components,
            ref focus,
            ref mut prompt,
            ref themes,
            theme_index,
            ref job_pool,
            ..
        } = *self;
        let frame = Rect::new(Position::new(0, 0), Size::new(screen.width, screen.height));
        let time = Instant::now();

        self.laid_components.clear();
        layout.compute(frame, &mut 1, &mut self.laid_components);
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
                    job_pool,
                };

                if id == PROMPT_ID {
                    prompt.draw(screen, &context)
                } else if id == SPLASH_ID {
                    Splash::default().draw(screen, &context)
                } else {
                    components.get_mut(&id).unwrap().draw(
                        screen,
                        &context.set_focused(
                            focus
                                .as_ref()
                                .map(|focused_id| *focused_id == id && !prompt.is_active())
                                .unwrap_or(false),
                        ),
                    );
                }
            },
        );
    }

    #[inline]
    fn notify_task_done(&mut self, response: ComponentTask) -> Result<()> {
        self.components
            .values_mut()
            .try_for_each(|component| component.task_done(&response))
    }

    #[inline]
    fn key_press(&mut self, key: Key, frame: Rect) -> Result<()> {
        // eprintln!("key_press: {:?}", key);
        let time = Instant::now();
        self.prompt.clear_log();
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

        if let (false, Some(&id_with_focus)) = (self.prompt.is_active(), self.focus.as_ref()) {
            self.lay_components(frame);

            let Self {
                ref mut components,
                ref mut prompt,
                ref laid_components,
                ref themes,
                theme_index,
                ref job_pool,
                ..
            } = *self;
            laid_components.iter().for_each(
                |&LaidComponentId {
                     id,
                     frame,
                     frame_id,
                 }| {
                    if id_with_focus == id {
                        if let Err(error) = components.get_mut(&id).unwrap().key_press(
                            key,
                            &Context {
                                time,
                                focused: true,
                                frame,
                                frame_id,
                                theme: &themes[theme_index].0,
                                job_pool,
                            },
                        ) {
                            prompt.log_error(format!("{}", error));
                        }
                    }
                },
            )
        }

        self.prompt.key_press(
            key,
            &Context {
                time,
                focused: false,
                frame,
                frame_id: 0,
                theme: &self.themes[self.theme_index].0,
                job_pool: &self.job_pool,
            },
        )?;
        if let Some(Command::OpenFile(path)) = self.prompt.poll_and_clear() {
            self.open_file(path)?;
        }

        Ok(())
    }

    #[inline]
    fn lay_components(&mut self, frame: Rect) {
        self.laid_components.clear();
        self.layout
            .compute(frame, &mut 1, &mut self.laid_components);
    }

    #[inline]
    fn cycle_focus(&mut self, frame: Rect, direction: CycleFocus) {
        self.lay_components(frame);
        while let Some(index) = self.laid_components.iter().position(|laid| laid.id < 2) {
            self.laid_components.swap_remove(index);
        }
        self.laid_components.sort_by_key(|laid| laid.frame_id);

        let len_components = self.laid_components.len();
        if len_components == 0 {
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
                CycleFocus::Previous => len_components + index - 1,
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
fn wrap_layout_with_prompt(layout: Option<Layout>) -> Layout {
    Layout::vertical(
        LayoutNodeFlex {
            node: layout.unwrap_or_else(|| Layout::Component(SPLASH_ID)),
            flex: Flex::Stretched,
        },
        LayoutNodeFlex {
            node: Layout::Component(PROMPT_ID),
            flex: Flex::Fixed(PROMPT_HEIGHT),
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
const PROMPT_HEIGHT: usize = 1;
const SPLASH_ID: ComponentId = 1;

const REDRAW_LATENCY: Duration = Duration::from_millis(10);
const SUSTAINED_IO_REDRAW_LATENCY: Duration = Duration::from_millis(100);
