//! Zi is a library for building modern terminal user interfaces.
//!
//! A user interface in Zi is built as a tree of stateful components. Components
//! let you split the UI into independent, reusable pieces, and think about each
//! piece in isolation.
//!
//! The `App` runtime will Zi is incremental. The runtime keeps and only calls
//! `view()` on those UI components that have changed and have to be
//! re-rendered. Lower level and independent of the components, the terminal
//! backend will incrementally redraw only those parts of the terminal that have
//! changed.
//!
//! The library is made up of four main components:
//!
//! - [`App`](struct.App.html): the runtime that will run your app
//!
//! # A Basic Example
//!
//! The following is a complete example of a Zi application which implements a
//! counter. It should provide a good sample of the different
//! [`Component`](trait.Component.html) methods and how they fit together.
//!
//! Anyone familiar with Yew, Elm or React + Redux should be familiar with all
//! the high-level concepts. Moreover, the names of some types and functions are
//! the same as in `Yew`.
//!
//! ```no_run
//! use zi::{
//!     components::{
//!         border::{Border, BorderProperties},
//!         text::{Text, TextAlign, TextProperties},
//!     },
//!     prelude::*,
//! };
//!
//! // Message type handled by the `Counter` component.
//! enum Message {
//!     Increment,
//!     Decrement,
//! }
//!
//! // The `Counter` component.
//! struct Counter {
//!     // The state of the component -- the current value of the counter.
//!     count: usize,
//!
//!     // A `ComponentLink` allows us to send messages to the component in reaction
//!     // to user input as well as to gracefully exit.
//!     link: ComponentLink<Self>,
//! }
//!
//! // Components implement the `Component` trait and are the building blocks of the
//! // UI in Zi. The trait describes stateful components and their lifecycle.
//! impl Component for Counter {
//!     // Messages are used to make components dynamic and interactive. For simple
//!     // or pure components, this will be `()`. Complex, stateful ones will
//!     // typically use an enum to declare multiple Message types. In this case, we
//!     // will emit two kinds of message (`Increment` or `Decrement`) in reaction
//!     // to user input.
//!     type Message = Message;
//!
//!     // Properties are the inputs to a Component passed in by their parent.
//!     type Properties = ();
//!
//!     // Creates ("mounts") a new `Counter` component.
//!     fn create(
//!         _properties: Self::Properties,
//!         _frame: Rect,
//!         link: ComponentLink<Self>,
//!     ) -> Self {
//!         Self { count: 0, link }
//!     }
//!
//!     // Returns the current visual layout of the component.
//!     fn view(&self) -> Layout {
//!         layout::component::<Border>(
//!             BorderProperties::new(layout::component::<Text>(
//!                 TextProperties::new()
//!                     .align(TextAlign::Centre)
//!                     .content(format!("Counter: {}", self.count)),
//!             ))
//!         )
//!     }
//!
//!     // Components handle messages in their `update` method and commonly use this
//!     // method to update their state and (optionally) re-render themselves.
//!     fn update(&mut self, message: Self::Message) -> ShouldRender {
//!         self.count = match message {
//!             Message::Increment => self.count.saturating_add(1),
//!             Message::Decrement => self.count.saturating_sub(1),
//!         };
//!         ShouldRender::Yes
//!     }
//!
//!     // Whether the component is currently focused which will caused
//!     // `input_binding` to be called on user input.
//!     fn has_focus(&self) -> bool {
//!         true
//!     }
//!
//!     // Send messages in reaction to user input.
//!     fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
//!         BindingMatch::clear(match pressed {
//!             &[Key::Char('+')]  => Some(Message::Increment),
//!             &[Key::Char('-')] => Some(Message::Decrement),
//!             &[Key::Esc] => {
//!                 self.link.exit();
//!                 None
//!             }
//!             _ => None,
//!         })
//!     }
//! }
//!
//! fn main() -> zi::Result<()> {
//!     let mut app = App::new(layout::component::<Counter>(()));
//!     app.run_event_loop(zi::frontend::default()?)
//! }
//! ```
//!
//! More examples can be found in the `examples` directory of the git
//! repository.

pub mod components;
pub mod frontend;
pub mod terminal;

pub use app::App;
pub use component::{
    layout::{
        self, auto, column, component, container, fixed, row, FlexBasis, FlexDirection, Item,
    },
    BindingMatch, BindingTransition, Callback, Component, ComponentLink, Layout, ShouldRender,
};
pub use error::{Error, Result};
pub use terminal::{Background, Canvas, Colour, Foreground, Key, Position, Rect, Size, Style};

pub mod prelude {
    //! The Zi prelude.
    pub use super::App;
    pub use super::{
        layout, BindingMatch, BindingTransition, Component, ComponentLink, Layout, ShouldRender,
    };
    pub use super::{Background, Canvas, Colour, Foreground, Key, Position, Rect, Size, Style};
}

// Crate only modules
pub(crate) mod app;
pub(crate) mod component;
pub(crate) mod error;
pub(crate) mod text;
