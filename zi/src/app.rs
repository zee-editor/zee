use crossbeam_channel::{select, Receiver, Sender};
use smallvec::SmallVec;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    component::{
        layout::{LaidCanvas, LaidComponent},
        template::{ComponentId, DynamicMessage, Renderable},
        BindingTransition, Component, ComponentLink, ShouldRender,
    },
    error::{Error, Result},
    frontend::Frontend,
    task::{TaskId, TaskPool},
    terminal::{Canvas, Key, Position, Rect},
};

enum PollState {
    Clean,
    Dirty,
    Exit,
}

pub struct App {
    task_pool: TaskPool,
    controller: InputController,
    components: HashMap<ComponentId, Box<dyn Renderable>>,
    pending_tasks: HashMap<TaskId, ComponentId>,
    focused_components: SmallVec<[ComponentId; 2]>,
    links_receiver: Receiver<(ComponentId, DynamicMessage)>,
    links_sender: Sender<(ComponentId, DynamicMessage)>,
    root_id: ComponentId,
}

impl App {
    pub fn new_with_component<ComponentT: Component>(component: ComponentT) -> Result<Self> {
        // Initialise app resources and book keeping
        let task_pool = TaskPool::new()?;
        let mut components: HashMap<ComponentId, Box<dyn Renderable>> = HashMap::new();
        let pending_tasks = HashMap::new();
        let (links_sender, links_receiver) = crossbeam_channel::unbounded();

        // Mount root
        let root_id = make_root_id::<ComponentT>();
        components.insert(root_id, Box::new(component));

        Ok(Self {
            task_pool,
            controller: InputController::new(),
            components,
            pending_tasks,
            focused_components: SmallVec::new(),
            links_receiver,
            links_sender,
            root_id,
        })
    }

    pub fn new<ComponentT: Component>(properties: ComponentT::Properties) -> Result<Self> {
        // Initialise app resources and book keeping
        let task_pool = TaskPool::new()?;
        let mut components: HashMap<ComponentId, Box<dyn Renderable>> = HashMap::new();
        let mut pending_tasks = HashMap::new();
        let (links_sender, links_receiver) = crossbeam_channel::unbounded();

        // Mount root
        let root_id = make_root_id::<ComponentT>();
        let link = ComponentLink::new(links_sender.clone(), root_id);
        let mut scheduler = task_pool.scheduler::<ComponentT::Message>();
        components.insert(
            root_id,
            Box::new(ComponentT::create(properties, link, &mut scheduler)),
        );
        for task_id in scheduler.into_scheduled() {
            pending_tasks.insert(task_id, root_id);
        }

        Ok(Self {
            task_pool,
            controller: InputController::new(),
            components,
            pending_tasks,
            focused_components: SmallVec::new(),
            links_receiver,
            links_sender,
            root_id,
        })
    }

    pub fn run_event_loop(&mut self, mut frontend: impl Frontend) -> Result<()> {
        let mut screen = Canvas::new(frontend.size()?);
        let mut poll_state = PollState::Dirty;
        let mut last_drawn = Instant::now() - REDRAW_LATENCY;
        loop {
            match poll_state {
                PollState::Dirty => {
                    screen.resize(frontend.size()?);
                    let frame = Rect::new(Position::new(0, 0), screen.size());
                    self.draw(&mut screen, frame);
                    frontend.present(&screen)?;
                    last_drawn = Instant::now();
                }
                PollState::Exit => {
                    return Ok(());
                }
                PollState::Clean => {}
            }

            poll_state = self.poll_events_batch(&frontend, last_drawn)?;
        }
    }

    #[inline]
    fn root(&self) -> &Box<dyn Renderable> {
        self.components
            .get(&self.root_id)
            .expect("root component is always mounted")
    }

    #[inline]
    fn draw(&mut self, screen: &mut Canvas, frame: Rect) {
        let layout = self.root().view(frame);

        self.focused_components.clear();
        if self.root().has_focus() {
            self.focused_components.push(self.root_id);
        }

        let Self {
            ref mut components,
            ref mut focused_components,
            ref mut pending_tasks,
            ref links_sender,
            ref task_pool,
            ..
        } = *self;
        eprintln!("======");
        layout.crawl(
            frame,
            0,
            &mut |LaidComponent {
                      frame,
                      position_hash,
                      template,
                  }| {
                let component_id = template.generate_id(position_hash);
                eprintln!("cur com: [ph: {}] {}", position_hash, component_id);
                let mut new_component = false;
                let component = components.entry(component_id).or_insert_with(|| {
                    new_component = true;
                    let created = template.create(component_id, links_sender.clone(), &task_pool);
                    for task_id in created.scheduled {
                        pending_tasks.insert(task_id, component_id);
                    }
                    created.component
                });
                if !new_component {
                    let changed = component.change(template.dynamic_properties(), &task_pool);
                    for task_id in changed.scheduled {
                        pending_tasks.insert(task_id, component_id);
                    }
                }
                if component.has_focus() {
                    focused_components.push(component_id);
                }
                component.view(frame)
            },
            &mut |LaidCanvas { frame, canvas, .. }| {
                screen.copy_region(&canvas, frame);
            },
        );
    }

    /// Poll as many events as we can respecting REDRAW_LATENCY and REDRAW_LATENCY_SUSTAINED_IO
    #[inline]
    fn poll_events_batch(
        &mut self,
        frontend: &impl Frontend,
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
                recv(self.links_receiver) -> links_message_result => {
                    let (component_id, dyn_message) = links_message_result.map_err(|err| Error::TaskPool(Box::new(err)))?;
                    let Self {
                        ref mut components,
                        ref mut pending_tasks,
                        ref task_pool,
                        ..
                    } = *self;
                    dirty = match components.get_mut(&component_id) {
                        Some(component) => {
                            let update = component.update(dyn_message, task_pool);
                            for task_id in update.scheduled {
                                pending_tasks.insert(task_id, component_id);
                            }
                            update.should_render == ShouldRender::Yes
                        },
                        None => {
                            log::debug!(
                                "Received message for nonexistent component (id: {}).",
                                component_id,
                            );
                            false
                        }
                    }
                }
                recv(self.task_pool.receiver) -> task_result => {
                    let Self {
                        ref mut components,
                        ref mut pending_tasks,
                        ref task_pool,
                        ..
                    } = *self;
                    let task_result = task_result.map_err(|err| Error::TaskPool(Box::new(err)))?;
                    let component_id = pending_tasks
                        .remove(&task_result.id)
                        .expect("tasks are always associated with a component");
                    dirty = match components.get_mut(&component_id) {
                        Some(component) => {
                            let update = component.update(task_result.payload, task_pool);
                            for task_id in update.scheduled {
                                pending_tasks.insert(task_id, component_id);
                            }
                            update.should_render == ShouldRender::Yes
                        },
                        None => {
                            log::debug!(
                                "Task done (id: {:?}) for nonexistent component (id: {}).",
                                task_result.id,
                                component_id,
                            );
                            false
                        }
                    }
                }
                recv(frontend.events()) -> event => {
                    match event.map_err(|error| Error::TaskPool(Box::new(error)))? {
                        key => {
                            if self.handle_event(key)? {
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
    fn handle_event(&mut self, key: Key) -> Result<bool> {
        eprintln!("key: {:?}", key);

        let Self {
            ref mut components,
            ref mut pending_tasks,
            ref focused_components,
            ref task_pool,
            ref mut controller,
            ..
        } = *self;
        let mut clear_controller = false;
        let mut changed_focus = false;

        controller.push(key);
        for component_id in focused_components.iter() {
            let focused_component = components
                .get_mut(component_id)
                .expect("tasks are always associated with a component");
            let binding = focused_component.input_binding(&controller.keys);
            match binding.transition {
                BindingTransition::Continue => {}
                BindingTransition::Clear => {
                    clear_controller = true;
                }
                BindingTransition::ChangedFocus => {
                    changed_focus = true;
                }
                BindingTransition::Exit => return Ok(true),
            }
            if let Some(message) = binding.message {
                let update = focused_component.update(message, &task_pool);
                for task_id in update.scheduled {
                    pending_tasks.insert(task_id, *component_id);
                }
            }

            // If the focus has changed we don't notify other focused components
            // deeper in the tree.
            if changed_focus {
                controller.keys.clear();
                return Ok(false);
            }
        }

        // If any component returned `BindingTransition::Clear`, we clear the controller.
        if clear_controller {
            controller.keys.clear();
        }

        Ok(false)
    }
}

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

    // fn matches<Action>(&mut self, bindings: &impl Bindings<Action>) -> BindingMatch<Action> {
    //     let binding_match = bindings.matches(&self.keys);
    //     if let BindingMatch::Full(_) = binding_match {
    //         self.keys.clear();
    //     }
    //     binding_match
    // }
}

impl std::fmt::Display for InputController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        for key in self.keys.iter() {
            match key {
                Key::Char(' ') => write!(formatter, "SPC ")?,
                Key::Char('\n') => write!(formatter, "RET ")?,
                Key::Char('\t') => write!(formatter, "TAB ")?,
                Key::Char(char) => write!(formatter, "{} ", char)?,
                Key::Ctrl(char) => write!(formatter, "C-{} ", char)?,
                Key::Alt(char) => write!(formatter, "A-{} ", char)?,
                Key::F(number) => write!(formatter, "F{} ", number)?,
                Key::Esc => write!(formatter, "ESC ")?,
                key => write!(formatter, "{:?} ", key)?,
            }
        }
        Ok(())
    }
}

fn make_root_id<ComponentT: Component>() -> ComponentId {
    ComponentId::new::<ComponentT>(0)
}

const REDRAW_LATENCY: Duration = Duration::from_millis(10);
const SUSTAINED_IO_REDRAW_LATENCY: Duration = Duration::from_millis(100);
