use crossbeam_channel::{self, Sender};
use smallvec::SmallVec;
use std::{
    any::{Any, TypeId},
    hash::Hash,
    marker::PhantomData,
};

pub use super::{layout::Layout, BindingMatch, Component, ComponentLink, ShouldRender};
use crate::{
    task::{TaskId, TaskPool},
    terminal::{Key, Rect},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ComponentId {
    type_id: TypeId,
    id: u64,
}

impl ComponentId {
    pub fn new<T: 'static>(id: u64) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            id,
        }
    }
}

impl<ComponentT: Component> ComponentLink<ComponentT> {
    pub(crate) fn new(
        sender: Sender<(ComponentId, DynamicMessage)>,
        component_id: ComponentId,
    ) -> Self {
        assert_eq!(TypeId::of::<ComponentT>(), component_id.type_id);
        Self {
            sender,
            component_id,
            _component: PhantomData,
        }
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{:?}.{}", self.type_id, self.id)
    }
}

pub(crate) struct DynamicMessage(pub(crate) Box<dyn Any + Send + 'static>);
pub(crate) struct DynamicProperties(Box<dyn Any + 'static>);
pub struct DynamicTemplate(pub(crate) Box<dyn Template>);

pub(crate) trait Renderable {
    fn update(&mut self, message: DynamicMessage, pool: &TaskPool) -> ComponentUpdate;

    fn change(
        &mut self,
        properties: DynamicProperties,
        pool: &TaskPool,
    ) -> ComponentChangeProperties;

    fn view(&self, frame: Rect) -> Layout;

    fn has_focus(&self) -> bool;

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<DynamicMessage>;
}

impl<ComponentT: Component> Renderable for ComponentT {
    fn update(&mut self, message: DynamicMessage, pool: &TaskPool) -> ComponentUpdate {
        let mut scheduler = pool.scheduler::<ComponentT::Message>();
        let should_render = <Self as Component>::update(
            self,
            *message
                .0
                .downcast()
                .expect("Incorrect `Message` type when downcasting"),
            &mut scheduler,
        );
        ComponentUpdate {
            should_render,
            scheduled: scheduler.into_scheduled(),
        }
    }

    fn change(
        &mut self,
        properties: DynamicProperties,
        pool: &TaskPool,
    ) -> ComponentChangeProperties {
        let mut scheduler = pool.scheduler::<ComponentT::Message>();
        let should_render = <Self as Component>::change(
            self,
            *properties
                .0
                .downcast()
                .expect("Incorrect `Properties`` type when downcasting"),
            &mut scheduler,
        );
        ComponentChangeProperties {
            should_render,
            scheduled: scheduler.into_scheduled(),
        }
    }

    fn view(&self, frame: Rect) -> Layout {
        <Self as Component>::view(self, frame)
    }

    fn has_focus(&self) -> bool {
        <Self as Component>::has_focus(self)
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<DynamicMessage> {
        let binding_match = <Self as Component>::input_binding(self, pressed);
        BindingMatch {
            transition: binding_match.transition,
            message: binding_match
                .message
                .map(|message| DynamicMessage(Box::new(message))),
        }
    }
}

pub(crate) trait Template {
    fn key(&self) -> Option<usize>;

    fn component_type_id(&self) -> TypeId;

    fn generate_id(&self, id: u64) -> ComponentId;

    fn create(
        &self,
        id: ComponentId,
        sender: Sender<(ComponentId, DynamicMessage)>,
        pool: &TaskPool,
    ) -> ComponentCreation;

    fn dynamic_properties(&self) -> DynamicProperties;
}

pub(crate) struct ComponentCreation {
    pub(crate) component: Box<dyn Renderable>,
    pub(crate) scheduled: SmallVec<[TaskId; 2]>,
}

pub(crate) struct ComponentUpdate {
    pub(crate) should_render: ShouldRender,
    pub(crate) scheduled: SmallVec<[TaskId; 2]>,
}

pub(crate) struct ComponentChangeProperties {
    pub(crate) should_render: ShouldRender,
    pub(crate) scheduled: SmallVec<[TaskId; 2]>,
}

pub(crate) struct ComponentDef<ComponentT: Component> {
    pub key: Option<usize>,
    pub properties: ComponentT::Properties,
}

impl<ComponentT: Component> ComponentDef<ComponentT> {
    pub(crate) fn new(key: Option<usize>, properties: ComponentT::Properties) -> Self {
        Self { key, properties }
    }
}

impl<ComponentT: Component> Template for ComponentDef<ComponentT> {
    fn key(&self) -> Option<usize> {
        self.key
    }

    fn component_type_id(&self) -> TypeId {
        TypeId::of::<ComponentT>()
    }

    fn generate_id(&self, position_hash: u64) -> ComponentId {
        ComponentId::new::<ComponentT>(position_hash)
    }

    fn create(
        &self,
        component_id: ComponentId,
        sender: Sender<(ComponentId, DynamicMessage)>,
        pool: &TaskPool,
    ) -> ComponentCreation {
        let link = ComponentLink::new(sender, component_id);
        let mut scheduler = pool.scheduler::<ComponentT::Message>();
        let component = Box::new(ComponentT::create(
            self.properties.clone(),
            link,
            &mut scheduler,
        ));
        ComponentCreation {
            component,
            scheduled: scheduler.into_scheduled(),
        }
    }

    fn dynamic_properties(&self) -> DynamicProperties {
        DynamicProperties(Box::new(self.properties.clone()))
    }
}
