pub mod buffer;
pub mod cursor;
pub mod edit_tree_viewer;
pub mod emojis;
pub mod prompt;
pub mod splash;
pub mod theme;

// pub trait Bindings<Action> {
//     fn matches(&self, pressed: &[Key]) -> BindingMatch<Action>;
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum BindingMatch<Action> {
//     None,
//     Prefix,
//     Full(Action),
// }

// impl<Action> BindingMatch<Action> {
//     pub fn is_prefix(&self) -> bool {
//         match self {
//             Self::Prefix => true,
//             _ => false,
//         }
//     }

//     pub fn map_action<MappedT>(self, f: impl FnOnce(Action) -> MappedT) -> BindingMatch<MappedT> {
//         match self {
//             Self::None => BindingMatch::None,
//             Self::Prefix => BindingMatch::Prefix,
//             Self::Full(action) => BindingMatch::Full(f(action)),
//         }
//     }
// }

// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct HashBindings<Action>(HashMap<SmallVec<[Key; 2]>, Action>);

// impl<Action> HashBindings<Action> {
//     pub fn new(map: HashMap<SmallVec<[Key; 2]>, Action>) -> Self {
//         Self(map)
//     }
// }

// impl<Action: Clone> Bindings<Action> for HashBindings<Action> {
//     fn matches(&self, pressed: &[Key]) -> BindingMatch<Action> {
//         for (binding, action) in self.0.iter() {
//             let is_match = binding
//                 .iter()
//                 .zip(pressed.iter())
//                 .all(|(lhs, rhs)| *lhs == *rhs);
//             if is_match {
//                 match pressed.len().cmp(&binding.len()) {
//                     Ordering::Less => {
//                         return BindingMatch::Prefix;
//                     }
//                     Ordering::Equal => {
//                         return BindingMatch::Full(action.clone());
//                     }
//                     _ => {}
//                 }
//             }
//         }
//         BindingMatch::None
//     }
// }

// #[cfg(test)]
// mod test {
//     use super::*;
//     use maplit::hashmap;
//     use smallvec::smallvec;

//     #[derive(Debug, Clone, PartialEq, Eq)]
//     enum TestAction {
//         A,
//         B,
//         C,
//         Fatality,
//     }

//     #[test]
//     fn test_empty_binding_matches() {
//         let bindings: HashBindings<TestAction> = HashBindings(HashMap::new());
//         assert_eq!(bindings.matches(&[Key::Delete]), BindingMatch::None);
//         assert_eq!(bindings.matches(&[Key::Ctrl('x')]), BindingMatch::None);
//         assert_eq!(bindings.matches(&[Key::Ctrl('a')]), BindingMatch::None);
//     }

//     #[test]
//     fn test_one_key_binding_matches() {
//         let bindings = HashBindings(hashmap! {
//             smallvec![Key::Ctrl('a')] => TestAction::A
//         });
//         assert_eq!(bindings.matches(&[Key::Delete]), BindingMatch::None);
//         assert_eq!(bindings.matches(&[Key::Ctrl('x')]), BindingMatch::None);
//         assert_eq!(
//             bindings.matches(&[Key::Ctrl('a')]),
//             BindingMatch::Full(TestAction::A)
//         );
//         assert_eq!(
//             bindings.matches(&[Key::Ctrl('a'), Key::Ctrl('a')]),
//             BindingMatch::None
//         );
//     }

//     #[test]
//     fn test_multiple_keys_binding_matches() {
//         let bindings = HashBindings(hashmap! {
//             smallvec![Key::Ctrl('a')] => TestAction::A,
//             smallvec![Key::Ctrl('b')] => TestAction::B,
//             smallvec![Key::Ctrl('x'), Key::Ctrl('a')] => TestAction::C,
//             smallvec![Key::Left, Key::Right, Key::Up, Key::Up, Key::Down] => TestAction::Fatality,
//         });
//         assert_eq!(bindings.matches(&[Key::Ctrl('z')]), BindingMatch::None);
//         assert_eq!(bindings.matches(&[Key::Ctrl('x')]), BindingMatch::Prefix);
//         assert_eq!(
//             bindings.matches(&[Key::Ctrl('a')]),
//             BindingMatch::Full(TestAction::A)
//         );
//         assert_eq!(
//             bindings.matches(&[Key::Ctrl('b')]),
//             BindingMatch::Full(TestAction::B)
//         );
//         assert_eq!(bindings.matches(&[Key::Left]), BindingMatch::Prefix);
//         assert_eq!(
//             bindings.matches(&[Key::Left, Key::Right, Key::Up]),
//             BindingMatch::Prefix
//         );
//         assert_eq!(
//             bindings.matches(&[Key::Left, Key::Right, Key::Up, Key::Up, Key::Down]),
//             BindingMatch::Full(TestAction::Fatality)
//         );
//         assert_eq!(
//             bindings.matches(&[Key::Left, Key::Right, Key::Up, Key::Up, Key::Up]),
//             BindingMatch::None
//         );
//         assert_eq!(
//             bindings.matches(&[Key::Left, Key::Right, Key::Up, Key::Up, Key::Down, Key::Up]),
//             BindingMatch::None
//         );
//     }
// }
