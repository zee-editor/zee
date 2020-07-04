use std::{
    any::{Any, TypeId},
    hash::{Hash, Hasher},
};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    layout::{ComponentKey, Layout},
    BindingMatch, Component, ComponentLink, LinkMessage, ShouldRender,
};
use crate::terminal::{Key, Rect};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ComponentId {
    type_id: TypeId,
    id: u64,

    // The `type_name` field is used only for debugging -- in particular
    // note that it's not a valid unique id for a type. See
    // https://doc.rust-lang.org/std/any/fn.type_name.html
    type_name: &'static str,
}

// `PartialEq` is impl'ed manually as `type_name` is only used for
// debugging and is ignored when testing for equality.
impl PartialEq for ComponentId {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self.id == other.id
    }
}

impl Eq for ComponentId {}

impl Hash for ComponentId {
    fn hash<HasherT: Hasher>(&self, hasher: &mut HasherT) {
        self.type_id.hash(hasher);
        self.id.hash(hasher);
    }
}

impl ComponentId {
    #[inline]
    pub(crate) fn new<T: 'static>(id: u64) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            id,
        }
    }

    #[inline]
    pub(crate) fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub(crate) fn type_name(&self) -> &'static str {
        self.type_name
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{} / {:x}", self.type_name, self.id >> 32)
    }
}

pub(crate) struct DynamicMessage(pub(crate) Box<dyn Any + Send + 'static>);
pub(crate) struct DynamicProperties(Box<dyn Any + 'static>);
pub(crate) struct DynamicTemplate(pub(crate) Box<dyn Template>);

impl Clone for DynamicTemplate {
    fn clone(&self) -> Self {
        self.0.clone()
    }
}

impl Template for DynamicTemplate {
    #[inline]
    fn key(&self) -> Option<ComponentKey> {
        self.0.key()
    }

    #[inline]
    fn component_type_id(&self) -> TypeId {
        self.0.component_type_id()
    }

    #[inline]
    fn generate_id(&self, id: u64) -> ComponentId {
        self.0.generate_id(id)
    }

    #[inline]
    fn create(
        &mut self,
        id: ComponentId,
        frame: Rect,
        sender: UnboundedSender<LinkMessage>,
    ) -> Box<dyn Renderable + 'static> {
        self.0.create(id, frame, sender)
    }

    #[inline]
    fn dynamic_properties(&mut self) -> DynamicProperties {
        self.0.dynamic_properties()
    }

    #[inline]
    fn clone(&self) -> DynamicTemplate {
        self.0.clone()
    }
}

pub(crate) trait Renderable {
    fn change(&mut self, properties: DynamicProperties) -> ShouldRender;

    fn resize(&mut self, frame: Rect) -> ShouldRender;

    fn update(&mut self, message: DynamicMessage) -> ShouldRender;

    fn view(&self) -> Layout;

    fn has_focus(&self) -> bool;

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<DynamicMessage>;

    fn tick(&self) -> Option<DynamicMessage>;
}

impl<ComponentT: Component> Renderable for ComponentT {
    #[inline]
    fn update(&mut self, message: DynamicMessage) -> ShouldRender {
        <Self as Component>::update(
            self,
            *message
                .0
                .downcast()
                .expect("Incorrect `Message` type when downcasting"),
        )
    }

    #[inline]
    fn change(&mut self, properties: DynamicProperties) -> ShouldRender {
        <Self as Component>::change(
            self,
            *properties
                .0
                .downcast()
                .expect("Incorrect `Properties` type when downcasting"),
        )
    }

    #[inline]
    fn resize(&mut self, frame: Rect) -> ShouldRender {
        <Self as Component>::resize(self, frame)
    }

    #[inline]
    fn view(&self) -> Layout {
        <Self as Component>::view(self)
    }

    #[inline]
    fn has_focus(&self) -> bool {
        <Self as Component>::has_focus(self)
    }

    #[inline]
    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<DynamicMessage> {
        let binding_match = <Self as Component>::input_binding(self, pressed);
        BindingMatch {
            transition: binding_match.transition,
            message: binding_match
                .message
                .map(|message| DynamicMessage(Box::new(message))),
        }
    }

    #[inline]
    fn tick(&self) -> Option<DynamicMessage> {
        <Self as Component>::tick(self).map(|message| DynamicMessage(Box::new(message)))
    }
}

pub(crate) trait Template {
    fn key(&self) -> Option<ComponentKey>;

    fn component_type_id(&self) -> TypeId;

    fn generate_id(&self, id: u64) -> ComponentId;

    fn create(
        &mut self,
        id: ComponentId,
        frame: Rect,
        sender: UnboundedSender<LinkMessage>,
    ) -> Box<dyn Renderable + 'static>;

    fn dynamic_properties(&mut self) -> DynamicProperties;

    fn clone(&self) -> DynamicTemplate;
}

pub(crate) struct ComponentDef<ComponentT: Component> {
    pub key: Option<ComponentKey>,
    pub properties: Option<ComponentT::Properties>,
}

impl<ComponentT: Component> Clone for ComponentDef<ComponentT> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            properties: self.properties.clone(),
        }
    }
}

impl<ComponentT: Component> ComponentDef<ComponentT> {
    pub(crate) fn new(key: Option<ComponentKey>, properties: ComponentT::Properties) -> Self {
        Self {
            key,
            properties: properties.into(),
        }
    }

    fn properties_unwrap(&mut self) -> ComponentT::Properties {
        let mut properties = None;
        std::mem::swap(&mut properties, &mut self.properties);
        properties.expect("Already called a method that used the `Properties` value")
    }
}

impl<ComponentT: Component> Template for ComponentDef<ComponentT> {
    #[inline]
    fn key(&self) -> Option<ComponentKey> {
        self.key
    }

    #[inline]
    fn component_type_id(&self) -> TypeId {
        TypeId::of::<ComponentT>()
    }

    #[inline]
    fn generate_id(&self, position_hash: u64) -> ComponentId {
        ComponentId::new::<ComponentT>(position_hash)
    }

    #[inline]
    fn create(
        &mut self,
        component_id: ComponentId,
        frame: Rect,
        sender: UnboundedSender<LinkMessage>,
    ) -> Box<dyn Renderable> {
        let link = ComponentLink::new(sender, component_id);
        Box::new(ComponentT::create(self.properties_unwrap(), frame, link))
    }

    #[inline]
    fn dynamic_properties(&mut self) -> DynamicProperties {
        DynamicProperties(Box::new(self.properties_unwrap()))
    }

    #[inline]
    fn clone(&self) -> DynamicTemplate {
        DynamicTemplate(Box::new(Clone::clone(self)))
    }
}
