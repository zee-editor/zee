use euclid::default::Vector2D;
use ropey::Rope;
use smallvec::SmallVec;
use std::ops::{Deref, DerefMut};

use crate::{components::cursor::Cursor, syntax::OpaqueDiff, utils};

#[derive(Debug, Clone)]
pub struct Revision {
    text: Rope,
    cursor: Cursor,
    pub parent: Option<Reference>,
    pub children: SmallVec<[Reference; 1]>,
    pub redo_index: usize,
}

impl Revision {
    fn root(text: Rope) -> Self {
        let mut cursor = Cursor::new();
        cursor.move_to_start_of_buffer(&text);
        Self {
            text,
            cursor,
            parent: None,
            children: SmallVec::new(),
            redo_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub index: usize,
    diff: OpaqueDiff,
}

pub struct EditTree {
    pub revisions: Vec<Revision>,
    pub head_index: usize,
    staged: Rope,
    has_staged_changes: bool,
}

impl EditTree {
    pub fn new(mut text: Rope) -> Self {
        utils::ensure_trailing_newline_with_content(&mut text);
        Self {
            revisions: vec![Revision::root(text.clone())],
            head_index: 0,
            staged: text,
            has_staged_changes: false,
        }
    }

    pub fn next_child(&mut self) {
        let current_revision = &mut self.revisions[self.head_index];
        if current_revision.redo_index < current_revision.children.len().saturating_sub(1) {
            current_revision.redo_index += 1;
        }
    }

    pub fn previous_child(&mut self) {
        let current_revision = &mut self.revisions[self.head_index];
        if current_revision.redo_index > 0 {
            current_revision.redo_index -= 1;
        }
    }

    pub fn new_revision(&mut self, diff: OpaqueDiff, cursor: Cursor) {
        let parent_to_child_diff = diff;
        let child_to_parent_diff = parent_to_child_diff.reverse();
        // let child_to_parent_diff = diff;
        // let parent_to_child_diff = child_to_parent_diff.reverse();
        let new_revision_index = self.revisions.len();
        self.revisions.push(Revision {
            text: self.staged.clone(),
            cursor,
            parent: Some(Reference {
                index: self.head_index,
                diff: child_to_parent_diff,
            }),
            children: SmallVec::new(),
            redo_index: 0,
        });
        {
            let head = &mut self.revisions[self.head_index];
            head.children.push(Reference {
                index: new_revision_index,
                diff: parent_to_child_diff,
            });
            head.redo_index = head.children.len() - 1;
        }
        self.head_index = new_revision_index;
        self.has_staged_changes = false;
    }

    pub fn undo(&mut self) -> Option<(OpaqueDiff, Cursor)> {
        // log::debug!(
        //     "undo[start]: hi={} text='{}' staged='{}'",
        //     self.head_index,
        //     self.revisions[self.head_index].text,
        //     self.staged
        // );
        if let Some(Reference { ref diff, index }) = self.revisions[self.head_index].parent {
            let new_revision = &self.revisions[index];
            self.staged = new_revision.text.clone();
            self.has_staged_changes = false;
            self.head_index = index;
            // log::debug!(
            //     "undo[end]: hi={} text='{}' staged='{}'",
            //     self.head_index,
            //     self.revisions[self.head_index].text,
            //     self.staged
            // );
            Some((diff.clone(), new_revision.cursor.clone()))
        } else {
            // log::debug!(
            //     "undo[end]: hi={} text='{}' staged='{}'",
            //     self.head_index,
            //     self.revisions[self.head_index].text,
            //     self.staged
            // );
            None
        }
    }

    pub fn redo(&mut self) -> Option<(OpaqueDiff, Cursor)> {
        let Self {
            revisions,
            head_index,
            staged,
            has_staged_changes,
            ..
        } = self;
        let Revision {
            ref children,
            redo_index,
            ..
        } = revisions[*head_index];
        children
            .get(redo_index)
            .map(|Reference { ref diff, index }| {
                let Revision {
                    ref cursor,
                    ref text,
                    ..
                } = revisions[*index];
                *staged = text.clone();
                *has_staged_changes = false;
                *head_index = *index;
                (diff.clone(), cursor.clone())
            })
    }

    pub fn head(&self) -> &Rope {
        self.deref()
    }
}

impl Deref for EditTree {
    type Target = Rope;

    fn deref(&self) -> &Rope {
        &self.staged
    }
}

impl DerefMut for EditTree {
    fn deref_mut(&mut self) -> &mut Rope {
        self.has_staged_changes = true;
        &mut self.staged
    }
}

pub struct FormattedRevision {
    pub transform: Vector2D<isize>,
    pub current_branch: bool,
}

pub fn format_revision(
    revisions: &[Revision],
    formatted: &mut [FormattedRevision],
    index: usize,
    transform: Vector2D<isize>,
    current_branch: bool,
) -> isize {
    {
        let formatted_revision = &mut formatted[index];
        formatted_revision.transform = transform;
        formatted_revision.current_branch = current_branch;
    }

    let revision = &revisions[index];
    let mut subtree_width = 0;
    for (child_index, child) in revision.children.iter().enumerate() {
        if child_index > 0 {
            subtree_width += 5;
        }
        subtree_width += format_revision(
            revisions,
            formatted,
            child.index,
            transform + Vector2D::new(subtree_width, 2),
            current_branch && (child_index == revision.redo_index),
        );
    }
    subtree_width
}

pub fn format_tree(tree: &EditTree) -> Vec<FormattedRevision> {
    let mut formatted = Vec::with_capacity(tree.revisions.len());
    formatted.resize_with(tree.revisions.len(), || FormattedRevision {
        transform: Vector2D::zero(),
        current_branch: true,
    });
    format_revision(&tree.revisions, &mut formatted, 0, Vector2D::zero(), true);
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_with_revisions_and_no_undo() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "The flowers are...");
        tree.new_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars() - 1; // Before the newline automatically inserted
        tree.insert(position, " so...\n");
        tree.new_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars() - 1;
        tree.insert(position, "dunno.");

        assert_eq!("The flowers are... so...\ndunno.\n", &tree.to_string());
    }

    #[test]
    fn undo_at_root_has_no_effect() {
        let mut tree = EditTree::new("The flowers are violet.\n".into());
        assert_eq!("The flowers are violet.\n", &tree.to_string());
        assert_eq!(None, tree.undo());
        assert_eq!("The flowers are violet.\n", &tree.to_string());
    }

    #[test]
    fn insert_and_undo() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "The flowers are...");
        tree.new_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars() - 1; // Before the newline automatically inserted
        tree.insert(position, " so...\n");
        let position = tree.len_chars() - 1;
        tree.insert(position, "dunno.");

        assert_eq!("The flowers are... so...\ndunno.\n", &tree.to_string());
        tree.undo();
        assert_eq!("The flowers are...\n", &tree.to_string());

        let position = tree.len_chars() - 1;
        tree.insert(position, " violet.");
        assert_eq!("The flowers are... violet.\n", &tree.to_string());
    }

    #[test]
    fn undo_redo_idempotent() {
        let mut tree = EditTree::new(Rope::new());
        tree.insert(0, "The flowers are...");
        tree.new_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars() - 1; // Before the newline automatically inserted
        tree.insert(position, " so...\n");
        let position = tree.len_chars() - 1;
        tree.insert(position, "dunno.");
        tree.new_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        assert_eq!("The flowers are... so...\ndunno.\n", &tree.to_string());
        tree.undo();
        tree.undo();
        assert_eq!("The flowers are...\n", &tree.to_string());
        tree.redo();
        tree.redo();
        assert_eq!("The flowers are... so...\ndunno.\n", &tree.to_string());
        tree.undo();
        tree.undo();
        assert_eq!("The flowers are...\n", &tree.to_string());
        tree.undo();
        assert_eq!("\n", &tree.to_string());
    }

    #[test]
    fn render_undo_tree() {}
}
