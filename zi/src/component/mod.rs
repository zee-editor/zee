pub mod input;
pub mod layout;
pub(crate) mod template;
pub mod text;

pub use self::layout::Layout;

use crossbeam_channel::{self, Sender};
use smallvec::SmallVec;
use std::{cmp::Ordering, collections::hash_map::HashMap, marker::PhantomData};

use self::template::{ComponentId, DynamicMessage};
use crate::{
    task::Scheduler,
    terminal::{Key, Rect},
};

pub trait Component: Sized + 'static {
    type Message: Send + 'static;
    type Properties: Clone;

    fn create(
        properties: Self::Properties,
        link: ComponentLink<Self>,
        scheduler: &mut Scheduler<Self::Message>,
    ) -> Self;

    fn change(
        &mut self,
        properties: Self::Properties,
        scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender;

    fn view(&self, frame: Rect) -> Layout;

    fn update(
        &mut self,
        _message: Self::Message,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        ShouldRender::Yes
    }

    fn has_focus(&self) -> bool {
        false
    }

    fn input_binding(&self, _pressed: &[Key]) -> BindingMatch<Self::Message> {
        BindingMatch {
            transition: BindingTransition::Clear,
            message: None,
        }
    }
}

#[derive(Debug)]
pub struct ComponentLink<ComponentT: Component> {
    sender: Sender<(ComponentId, DynamicMessage)>,
    component_id: ComponentId,
    _component: PhantomData<ComponentT>,
}

impl<ComponentT: Component> ComponentLink<ComponentT> {
    pub fn send(&self, message: ComponentT::Message) {
        self.sender
            .send((self.component_id, DynamicMessage(Box::new(message))))
            .expect("receivers should outlive senders for inter-component messages");
    }
}

impl<ComponentT: Component> Clone for ComponentLink<ComponentT> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            component_id: self.component_id,
            _component: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShouldRender {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingTransition {
    Continue,
    Clear,
    ChangedFocus,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingMatch<Message> {
    pub transition: BindingTransition,
    pub message: Option<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashBindings<Action>(HashMap<SmallVec<[Key; 2]>, Action>);

impl<Message> HashBindings<Message> {
    pub fn new(map: HashMap<SmallVec<[Key; 2]>, Message>) -> Self {
        Self(map)
    }
}

impl<Message: Clone> HashBindings<Message> {
    pub fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Message> {
        for (binding, message) in self.0.iter() {
            let is_match = binding
                .iter()
                .zip(pressed.iter())
                .all(|(lhs, rhs)| *lhs == *rhs);
            if is_match {
                match pressed.len().cmp(&binding.len()) {
                    Ordering::Less => {
                        return BindingMatch {
                            transition: BindingTransition::Continue,
                            message: None,
                        };
                    }
                    Ordering::Equal => {
                        return BindingMatch {
                            transition: BindingTransition::Clear,
                            message: Some(message.clone()),
                        }
                    }
                    _ => {}
                }
            }
        }
        BindingMatch {
            transition: BindingTransition::Clear,
            message: None,
        }
    }
}
