use ropey::Rope;
use std::ops::{Deref, DerefMut};

use crate::{components::cursor::Cursor, syntax::OpaqueDiff, utils};

#[derive(Debug, Clone)]
struct Revision {
    text: Rope,
    cursor: Cursor,
    parent: Option<Reference>,
}

#[derive(Debug, Clone)]
struct Reference {
    index: usize,
    diff: OpaqueDiff,
}

pub struct UndoTree {
    revisions: Vec<Revision>,
    parent_revision_index: usize,
    head: Rope,
}

impl UndoTree {
    pub fn new(mut text: Rope) -> Self {
        utils::ensure_trailing_newline_with_content(&mut text);
        let mut cursor = Cursor::new();
        cursor.move_to_start_of_buffer(&text);
        let root = Revision {
            text: text.clone(),
            cursor,
            parent: None,
        };
        Self {
            revisions: vec![root],
            parent_revision_index: 0,
            head: text,
        }
    }

    pub fn new_revision(&mut self, diff: OpaqueDiff, cursor: Cursor) {
        self.revisions.push(Revision {
            text: self.head.clone(),
            cursor,
            parent: Some(Reference {
                index: self.parent_revision_index,
                diff,
            }),
        });
        self.parent_revision_index = self.revisions.len() - 1;
    }

    pub fn undo(&mut self) -> Option<(OpaqueDiff, Cursor)> {
        let Revision {
            ref text,
            ref parent,
            ref cursor,
        } = self.revisions[self.parent_revision_index];
        self.head = text.clone();
        if let Some(Reference { ref diff, index }) = parent {
            let diff = diff.reverse();
            self.parent_revision_index = *index;
            // eprintln!("Current revision: {:?}", self.head);
            Some((diff, cursor.clone()))
        } else {
            None
        }
    }

    pub fn head(&self) -> &Rope {
        self.deref()
    }
}

impl Deref for UndoTree {
    type Target = Rope;

    fn deref(&self) -> &Rope {
        &self.head
    }
}

impl DerefMut for UndoTree {
    fn deref_mut(&mut self) -> &mut Rope {
        &mut self.head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_with_revisions_and_no_undo() {
        let mut tree = UndoTree::new(Rope::new());
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
    fn insert_and_undo() {
        let mut tree = UndoTree::new(Rope::new());
        tree.insert(0, "The flowers are...");
        tree.new_revision(OpaqueDiff::empty(), Cursor::end_of_buffer(&tree));

        let position = tree.len_chars() - 1; // Before the newline automatically inserted
        tree.insert(position, " so...\n");
        let position = tree.len_chars() - 1;
        tree.insert(position, "dunno.");

        tree.undo();
        assert_eq!("The flowers are...\n", &tree.to_string());

        let position = tree.len_chars() - 1;
        tree.insert(position, " violet.");
        assert_eq!("The flowers are... violet.\n", &tree.to_string());
    }
}
