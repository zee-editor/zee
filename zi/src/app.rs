use futures::{self, stream::StreamExt};
use smallvec::SmallVec;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::{
    self,
    runtime::{Builder as RuntimeBuilder, Runtime},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};

use crate::{
    component::{
        layout::{LaidCanvas, LaidComponent, Layout},
        template::{ComponentId, DynamicMessage, DynamicProperties, Renderable, Template},
        BindingMatch, BindingTransition, LinkMessage, ShouldRender,
    },
    error::Result,
    frontend::{Event, Frontend},
    terminal::{Canvas, Key, Position, Rect, Size},
};

pub struct App {
    controller: InputController,
    components: HashMap<ComponentId, MountedComponent>,
    layout_cache: HashMap<ComponentId, Layout>,
    focused_components: SmallVec<[ComponentId; 2]>,
    tickable_components: SmallVec<[(ComponentId, DynamicMessage); 2]>,
    link_receiver: UnboundedReceiver<LinkMessage>,
    link_sender: UnboundedSender<LinkMessage>,
    root: Layout,
}

impl App {
    pub fn new(root: Layout) -> Self {
        let (link_sender, link_receiver) = mpsc::unbounded_channel();
        Self {
            controller: InputController::new(),
            components: HashMap::new(),
            layout_cache: HashMap::new(),
            focused_components: SmallVec::new(),
            tickable_components: SmallVec::new(),
            link_receiver,
            link_sender,
            root,
        }
    }

    pub fn run_event_loop(&mut self, mut frontend: impl Frontend) -> Result<()> {
        let mut screen = Canvas::new(frontend.size()?);
        let mut poll_state = PollState::Dirty(None);
        let mut last_drawn = Instant::now() - REDRAW_LATENCY;
        let mut runtime = RuntimeBuilder::new()
            .basic_scheduler()
            .enable_all()
            .build()?;

        loop {
            match poll_state {
                PollState::Dirty(new_size) => {
                    let now = Instant::now();

                    // Draw
                    if let Some(screen_size) = new_size {
                        log::debug!("Screen resized {} -> {}", screen.size(), screen_size);
                        screen.resize(screen_size);
                    }

                    let frame = Rect::new(Position::new(0, 0), screen.size());
                    self.draw(&mut screen, frame);
                    let drawn_time = now.elapsed();

                    // Present
                    let now = Instant::now();
                    let num_bytes_presented = frontend.present(&screen)?;

                    log::debug!(
                        "Frame drawn in {:?} | Presented {} bytes in {:?}",
                        drawn_time,
                        num_bytes_presented,
                        now.elapsed(),
                    );
                    last_drawn = Instant::now();
                }
                PollState::Exit => {
                    break;
                }
                _ => {}
            }

            poll_state = self.poll_events_batch(&mut runtime, &mut frontend, last_drawn)?;
        }

        Ok(())
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
            ref link_sender,
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
                        let renderable = template.create(component_id, frame, link_sender.clone());
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
        runtime: &mut Runtime,
        frontend: &mut impl Frontend,
        last_drawn: Instant,
    ) -> Result<PollState> {
        let mut force_redraw = false;
        let mut first_event_time: Option<Instant> = None;
        let mut poll_state = PollState::Clean;

        while !force_redraw && !poll_state.exit() {
            let timeout_duration = {
                let since_last_drawn = last_drawn.elapsed();
                if poll_state.dirty() && since_last_drawn >= REDRAW_LATENCY {
                    Duration::from_millis(0)
                } else if poll_state.dirty() {
                    REDRAW_LATENCY - since_last_drawn
                } else {
                    Duration::from_millis(if self.tickable_components.is_empty() {
                        240
                    } else {
                        60
                    })
                }
            };
            (runtime.block_on(async {
                tokio::select! {
                    link_message = self.link_receiver.recv() => {
                        poll_state = self.handle_link_message(
                            frontend,
                            link_message.expect("At least one sender exists."),
                        )?;
                        Ok(())
                    }
                    input_event = frontend.event_stream().next() => {
                        poll_state = self.handle_input_event(input_event.expect(
                            "At least one sender exists.",
                        )?)?;
                        force_redraw = poll_state.dirty()
                            && (first_event_time.get_or_insert_with(Instant::now).elapsed()
                                >= SUSTAINED_IO_REDRAW_LATENCY
                                || poll_state.resized());
                        Ok(())
                    }
                    _ = tokio::time::delay_for(timeout_duration) => {
                        for (component_id, dyn_message) in self.tickable_components.drain(..) {
                            poll_state = PollState::Dirty(None);
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
                        Ok(())
                    }
                }
            }) as Result<()>)?;
        }

        Ok(poll_state)
    }

    #[inline]
    fn handle_link_message(
        &mut self,
        frontend: &mut impl Frontend,
        message: LinkMessage,
    ) -> Result<PollState> {
        Ok(match message {
            LinkMessage::Component(component_id, dyn_message) => {
                if self
                    .components
                    .get_mut(&component_id)
                    .map(|component| component.update(dyn_message))
                    .unwrap_or_else(|| {
                        log::debug!(
                            "Received message for nonexistent component (id: {}).",
                            component_id,
                        );
                        false
                    })
                {
                    PollState::Dirty(None)
                } else {
                    PollState::Clean
                }
            }
            LinkMessage::Exit => PollState::Exit,
            LinkMessage::RunExclusive(process) => {
                frontend.suspend()?;
                let maybe_message = process();
                frontend.resume()?;
                // force_redraw = true;
                if let Some((component_id, dyn_message)) = maybe_message {
                    self.components
                        .get_mut(&component_id)
                        .map(|component| component.update(dyn_message))
                        .unwrap_or_else(|| {
                            log::debug!(
                                "Received message for nonexistent component (id: {}).",
                                component_id,
                            );
                            false
                        });
                }
                PollState::Dirty(None)
            }
        })
    }

    #[inline]
    fn handle_input_event(&mut self, event: Event) -> Result<PollState> {
        Ok(match event {
            Event::Key(key) => {
                self.handle_key(key)?;
                PollState::Dirty(None) // handle_event should return whether we need to rerender
            }
            Event::Resize(size) => PollState::Dirty(Some(size)),
        })
    }

    #[inline]
    fn handle_key(&mut self, key: Key) -> Result<()> {
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

#[derive(Debug, PartialEq)]
enum PollState {
    Clean,
    Dirty(Option<Size>),
    Exit,
}

impl PollState {
    fn dirty(&self) -> bool {
        match *self {
            Self::Dirty(_) => true,
            _ => false,
        }
    }

    fn resized(&self) -> bool {
        match *self {
            Self::Dirty(Some(_)) => true,
            _ => false,
        }
    }

    fn exit(&self) -> bool {
        Self::Exit == *self
    }
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
