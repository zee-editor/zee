use crossbeam_channel::{select, Receiver, Sender};
use smallvec::SmallVec;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    component::{
        layout::{LaidCanvas, LaidComponent, Layout},
        template::{ComponentId, DynamicMessage, DynamicProperties, Renderable, Template},
        BindingMatch, BindingTransition, LinkMessage, ShouldRender,
    },
    error::Result,
    frontend::Frontend,
    terminal::{Canvas, Key, Position, Rect},
};

#[derive(Debug)]
enum PollState {
    Clean,
    Dirty,
    Exit,
}

struct MountedComponent {
    renderable: Box<dyn Renderable>,
    should_render: bool,
    frame: Rect,
}

impl MountedComponent {
    #[inline]
    fn change(&mut self, properties: DynamicProperties) -> bool {
        self.should_render = self.renderable.change(properties).into() || self.should_render;
        self.should_render
    }

    #[inline]
    fn resize(&mut self, frame: Rect) -> bool {
        self.should_render = self.renderable.resize(frame).into() || self.should_render;
        self.frame = frame;
        self.should_render
    }

    #[inline]
    fn update(&mut self, message: DynamicMessage) -> bool {
        self.should_render = self.renderable.update(message).into() || self.should_render;
        self.should_render
    }

    #[inline]
    fn view(&mut self) -> Layout {
        self.should_render = false;
        self.renderable.view()
    }

    #[inline]
    fn has_focus(&self) -> bool {
        self.renderable.has_focus()
    }

    #[inline]
    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<DynamicMessage> {
        self.renderable.input_binding(pressed)
    }

    #[inline]
    fn tick(&self) -> Option<DynamicMessage> {
        self.renderable.tick()
    }
}

pub struct App {
    controller: InputController,
    components: HashMap<ComponentId, MountedComponent>,
    layout_cache: HashMap<ComponentId, Layout>,
    focused_components: SmallVec<[ComponentId; 2]>,
    tickable_components: SmallVec<[(ComponentId, DynamicMessage); 2]>,
    links_receiver: Receiver<LinkMessage>,
    links_sender: Arc<Sender<LinkMessage>>,
    root: Layout,
}

impl App {
    pub fn new(root: Layout) -> Self {
        let (links_sender, links_receiver) = crossbeam_channel::unbounded();
        Self {
            controller: InputController::new(),
            components: HashMap::new(),
            layout_cache: HashMap::new(),
            focused_components: SmallVec::new(),
            tickable_components: SmallVec::new(),
            links_receiver,
            links_sender: Arc::new(links_sender),
            root,
        }
    }

    pub fn run_event_loop(&mut self, mut frontend: impl Frontend) -> Result<()> {
        let mut screen = Canvas::new(frontend.size()?);
        let mut poll_state = PollState::Dirty;
        let mut last_drawn = Instant::now() - REDRAW_LATENCY;
        loop {
            let screen_size = frontend.size()?;
            let resized = screen_size != screen.size();

            // eprintln!("({:?}, {:?})", poll_state, resized);
            match (poll_state, resized) {
                (PollState::Dirty, _) | (PollState::Clean, true) => {
                    let now = Instant::now();

                    // Draw
                    if resized {
                        screen.resize(screen_size);
                    }

                    let frame = Rect::new(Position::new(0, 0), screen.size());
                    self.draw(&mut screen, frame);
                    let drawn_time = now.elapsed();

                    // Present
                    let now = Instant::now();
                    let num_bytes_presented = frontend.present(&screen)?;

                    log::info!(
                        "Drawn in {:?} | Presented {} bytes in {:?}",
                        drawn_time,
                        num_bytes_presented,
                        now.elapsed(),
                    );
                    last_drawn = Instant::now();
                }
                (PollState::Exit, _) => {
                    return Ok(());
                }
                (PollState::Clean, false) => {}
            }

            poll_state = self.poll_events_batch(&frontend, last_drawn)?;
        }
    }

    #[inline]
    fn draw(&mut self, screen: &mut Canvas, frame: Rect) {
        self.focused_components.clear();
        self.tickable_components.clear();

        let Self {
            ref mut components,
            ref mut layout_cache,
            ref mut focused_components,
            ref mut tickable_components,
            ref links_sender,
            ..
        } = *self;
        let mut first = true;
        let mut pending = Vec::new();

        loop {
            let (layout, frame2, position_hash, parent_changed) = if first {
                first = false;
                (&mut self.root, frame, 0, false)
            } else if let Some((component_id, frame, position_hash)) = pending.pop() {
                let component = components
                    .get_mut(&component_id)
                    .expect("Layout is cached only for mounted components");
                let layout = layout_cache
                    .entry(component_id)
                    .or_insert_with(|| component.view());
                let changed = component.should_render.into();
                if changed {
                    *layout = component.view()
                }
                (layout, frame, position_hash, changed)
            } else {
                break;
            };

            layout.crawl(
                frame2,
                position_hash,
                &mut |LaidComponent {
                          frame,
                          position_hash,
                          template,
                      }| {
                    let component_id = template.generate_id(position_hash);
                    let mut new_component = false;
                    let component = components.entry(component_id).or_insert_with(|| {
                        new_component = true;
                        let renderable = template.create(component_id, frame, links_sender.clone());
                        MountedComponent {
                            renderable,
                            frame,
                            should_render: ShouldRender::Yes.into(),
                        }
                    });

                    if !new_component {
                        if parent_changed {
                            component.change(template.dynamic_properties());
                        }
                        if frame != component.frame {
                            component.resize(frame);
                        }
                    }

                    if component.has_focus() {
                        focused_components.push(component_id);
                    }

                    if let Some(message) = component.tick() {
                        tickable_components.push((component_id, message));
                    }

                    // eprintln!(
                    //     "should_render={} new={} parent_changed={} [{} at {}]",
                    //     component.should_render, new_component, parent_changed, component_id, frame,
                    // );

                    pending.push((component_id, frame, position_hash));
                },
                &mut |LaidCanvas { frame, canvas, .. }| {
                    screen.copy_region(&canvas, frame);
                },
            );
        }
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
                    Duration::from_millis(if self.tickable_components.is_empty() {
                        240
                    } else {
                        60
                    })
                }
            };

            select! {
                recv(self.links_receiver) -> links_message_result => {
                    let (component_id, dyn_message) = match links_message_result? {
                        LinkMessage::Component(component_id, dyn_message) => {
                            (component_id, dyn_message)
                        }
                        LinkMessage::Exit => return Ok(PollState::Exit),
                    };
                    let Self {
                        ref mut components,
                        ..
                    } = *self;
                    dirty = match components.get_mut(&component_id) {
                        Some(component) => {
                            component.update(dyn_message)
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
                recv(frontend.events()) -> event => {
                    match event? {
                        key => {
                            self.handle_event(key)?;
                            dirty = true; // handle_event should return whether we need to rerender
                        }
                    };
                    force_redraw = dirty
                        && first_event_time.get_or_insert_with(Instant::now).elapsed()
                        >= SUSTAINED_IO_REDRAW_LATENCY;
                }
                default(timeout) => {
                    for (component_id, dyn_message) in self.tickable_components.drain(..) {
                        dirty = true;
                        match self.components.get_mut(&component_id) {
                            Some(component) => {
                                component.update(dyn_message);
                            },
                            None => {
                                log::debug!(
                                    "Received message for nonexistent component (id: {}).",
                                    component_id,
                                );
                            }
                        }
                    }
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
    fn handle_event(&mut self, key: Key) -> Result<()> {
        let Self {
            ref mut components,
            ref focused_components,
            ref mut controller,
            ..
        } = *self;
        let mut clear_controller = false;
        let mut changed_focus = false;

        controller.push(key);
        for component_id in focused_components.iter() {
            let focused_component = components
                .get_mut(component_id)
                .expect("A focused component should be mounted.");
            let binding = focused_component.input_binding(&controller.keys);
            match binding.transition {
                BindingTransition::Continue => {}
                BindingTransition::Clear => {
                    clear_controller = true;
                }
                BindingTransition::ChangedFocus => {
                    changed_focus = true;
                }
            }
            if let Some(message) = binding.message {
                focused_component.update(message);
            }

            // If the focus has changed we don't notify other focused components
            // deeper in the tree.
            if changed_focus {
                controller.keys.clear();
                return Ok(());
            }
        }

        // If any component returned `BindingTransition::Clear`, we clear the controller.
        if clear_controller {
            controller.keys.clear();
        }

        Ok(())
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

const REDRAW_LATENCY: Duration = Duration::from_millis(10);
const SUSTAINED_IO_REDRAW_LATENCY: Duration = Duration::from_millis(100);

#[cfg(test)]
mod tests {
    use super::{ComponentId, DynamicMessage, LinkMessage};

    #[test]
    fn sizes() {
        eprintln!(
            "std::mem::size_of::<(ComponentId, DynamicMessage)>() == {}",
            std::mem::size_of::<(ComponentId, DynamicMessage)>()
        );
        eprintln!(
            "std::mem::size_of::<LinkMessage>() == {}",
            std::mem::size_of::<LinkMessage>()
        );
    }
}
