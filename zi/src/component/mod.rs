pub mod border;
pub mod input;
pub mod layout;
pub mod select;
pub(crate) mod template;
pub mod text;

pub use self::layout::Layout;

use crossbeam_channel::{self, Sender};
use smallvec::SmallVec;
use std::{
    any::TypeId, cmp::Ordering, collections::hash_map::HashMap, marker::PhantomData, rc::Rc,
    sync::Arc,
};

use self::template::{ComponentId, DynamicMessage};
use crate::terminal::{Key, Rect};

pub trait Component: Sized + 'static {
    type Message: Send + 'static;
    type Properties: Clone;

    fn create(properties: Self::Properties, frame: Rect, link: ComponentLink<Self>) -> Self;

    fn view(&self) -> Layout;

    fn change(&mut self, _properties: Self::Properties) -> ShouldRender {
        ShouldRender::No
    }

    fn resize(&mut self, _frame: Rect) -> ShouldRender {
        ShouldRender::No
    }

    fn update(&mut self, _message: Self::Message) -> ShouldRender {
        ShouldRender::No
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

    fn tick(&self) -> Option<Self::Message> {
        None
    }
}

pub struct Callback<InputT, OutputT = ()>(Rc<dyn Fn(InputT) -> OutputT>);

impl<InputT, OutputT> Clone for Callback<InputT, OutputT> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<InputT, OutputT> PartialEq for Callback<InputT, OutputT> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<InputT, OutputT> Callback<InputT, OutputT> {
    pub fn emit(&self, value: InputT) -> OutputT {
        (self.0)(value)
    }
}

impl<InputT, OutputT, FnT> From<FnT> for Callback<InputT, OutputT>
where
    FnT: Fn(InputT) -> OutputT + 'static,
{
    fn from(function: FnT) -> Self {
        Self(Rc::new(function))
    }
}

#[derive(Debug)]
pub struct ComponentLink<ComponentT> {
    sender: Arc<Sender<LinkMessage>>,
    component_id: ComponentId,
    _component: PhantomData<fn() -> ComponentT>,
}

impl<ComponentT: Component> ComponentLink<ComponentT> {
    pub fn send(&self, message: ComponentT::Message) {
        self.sender
            .send(LinkMessage::Component(
                self.component_id,
                DynamicMessage(Box::new(message)),
            ))
            .expect("App receiver needs to outlive senders for inter-component messages");
    }

    pub fn callback<InputT>(
        &self,
        callback: impl Fn(InputT) -> ComponentT::Message + 'static,
    ) -> Callback<InputT> {
        let link = self.clone();
        Callback(Rc::new(move |input| link.send(callback(input))))
    }

    pub fn exit(&self) {
        self.sender
            .send(LinkMessage::Exit)
            .expect("App needs to outlive components");
    }

    pub(crate) fn new(sender: Arc<Sender<LinkMessage>>, component_id: ComponentId) -> Self {
        assert_eq!(TypeId::of::<ComponentT>(), component_id.type_id());
        Self {
            sender,
            component_id,
            _component: PhantomData,
        }
    }
}

impl<ComponentT> Clone for ComponentLink<ComponentT> {
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

impl Into<bool> for ShouldRender {
    fn into(self) -> bool {
        self == ShouldRender::Yes
    }
}

impl From<bool> for ShouldRender {
    fn from(should_render: bool) -> Self {
        if should_render {
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingTransition {
    Continue,
    Clear,
    ChangedFocus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingMatch<Message> {
    pub transition: BindingTransition,
    pub message: Option<Message>,
}

pub(crate) enum LinkMessage {
    Component(ComponentId, DynamicMessage),
    Exit,
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
