use maplit::hashmap;
use once_cell::sync::Lazy;
use smallvec::{smallvec, SmallVec};
use std::{cmp, iter};

use super::{
    theme::Theme as EditorTheme, BindingMatch, Bindings, Component, Context, HashBindings,
};
use crate::{
    error::Result,
    terminal::{Key, Position, Rect, Screen, Size, Style},
    undo::{self, EditTree},
};

#[derive(Clone, Debug)]
pub enum Action {
    Up,
    Down,
}

static BINDINGS: Lazy<HashBindings<Action>> = Lazy::new(|| {
    HashBindings::new(hashmap! {
        smallvec![Key::Char('p')] => Action::Up,
        smallvec![Key::Ctrl('n')] => Action::Down,
    })
});

#[derive(Clone, Debug)]
pub struct Theme {
    pub current_revision: Style,
    pub master_revision: Style,
    pub master_connector: Style,
    pub alternate_revision: Style,
    pub alternate_connector: Style,
}

pub struct EditTreeViewer;

impl EditTreeViewer {
    #[inline]
    fn draw_background(&self, screen: &mut Screen, context: &Context) {
        let Context {
            ref frame,
            frame_id,
            focused,
            ref theme,
            ..
        } = *context;
        screen.clear_region(
            Rect::new(
                Position::new(frame.origin.x, frame.origin.y),
                Size::new(frame.size.width, frame.size.height),
            ),
            theme.buffer.edit_tree_viewer.current_revision,
        );
    }

    pub fn draw(&mut self, screen: &mut Screen, tree: &EditTree, context: &Context) {
        self.draw_background(screen, context);

        let Context {
            ref frame,
            ref theme,
            ..
        } = *context;

        log::info!("Frame: {:?}", context.frame);
        // let middle_x = frame.size.width / 2;
        // let middle_y = frame.size.height / 2;

        // let mut y = middle_y + 8;
        // let mut revision_index = tree.parent_revision_index;
        // let mut revision = &tree.revisions[revision_index];
        // loop {
        //     screen.draw_str(
        //         frame.origin.x + middle_x,
        //         y,
        //         theme.buffer.syntax.text,
        //         &revision_index.to_string(),
        //     );

        //     if let Some(parent) = revision.parent.as_ref() {
        //         revision_index = parent.index;
        //         revision = &tree.revisions[revision_index];
        //         screen.draw_str(
        //             frame.origin.x + middle_x,
        //             y.saturating_sub(1),
        //             theme.buffer.syntax.text,
        //             "|",
        //         );
        //         y = y.saturating_sub(2);
        //     } else {
        //         break;
        //     }
        // }

        let formatted_tree = undo::format_tree(tree);

        let (middle_x, middle_y) = {
            let transform = formatted_tree[tree.head_index].transform;
            let middle_x = (frame.size.width / 2) as isize - transform.x;
            let middle_y = (frame.size.height / 2) as isize - transform.y;
            (middle_x, middle_y)
        };

        // let mut y = middle_y + 8;
        // let mut revision_index = tree.parent_revision_index;
        for (revision_index, formatted) in formatted_tree.iter().enumerate() {
            let (revision_style, connector_style) = if revision_index == tree.head_index {
                (
                    theme.buffer.edit_tree_viewer.current_revision,
                    theme.buffer.edit_tree_viewer.master_connector,
                )
            } else if formatted.current_branch {
                (
                    theme.buffer.edit_tree_viewer.master_revision,
                    theme.buffer.edit_tree_viewer.master_connector,
                )
            } else {
                (
                    theme.buffer.edit_tree_viewer.alternate_revision,
                    theme.buffer.edit_tree_viewer.alternate_connector,
                )
            };
            let revision = &tree.revisions[revision_index];
            let x = middle_x + formatted.transform.x;
            let y = middle_y + formatted.transform.y;
            if x >= 0 && y >= 0 && x < frame.size.width as isize && y < frame.size.height as isize {
                screen.draw_str(
                    (frame.origin.x as isize + x) as usize,
                    (frame.origin.y as isize + y) as usize,
                    revision_style,
                    &format!(
                        "{:.5}{}",
                        revision_index,
                        if revision_index == tree.head_index {
                            "*"
                        } else {
                            ""
                        },
                    ),
                );
            }

            for (child_index, child) in revision.children.iter().enumerate() {
                let formatted_child = &formatted_tree[child.index];
                let x = middle_x + formatted_child.transform.x;
                let y = middle_y + formatted_child.transform.y - 1;
                if x >= 0
                    && y >= 0
                    && x < frame.size.width as isize
                    && y < frame.size.height as isize
                {
                    screen.draw_str(
                        (frame.origin.x as isize + x) as usize,
                        (frame.origin.y as isize + y) as usize,
                        connector_style,
                        if child_index > 0 { "\\" } else { "|" },
                    );
                }
            }
            let mut pairs = revision.children.windows(2);
            while let Some(&[ref left, ref right]) = pairs.next() {
                let formatted_left = &formatted_tree[left.index];
                let formatted_right = &formatted_tree[right.index];
                assert!(formatted_left.transform.y == formatted_right.transform.y);
                let (mut start_x, mut end_x) =
                    if formatted_left.transform.x < formatted_right.transform.x {
                        (formatted_left.transform.x, formatted_right.transform.x)
                    } else {
                        (formatted_right.transform.x, formatted_left.transform.x)
                    };
                start_x = cmp::max(middle_x + start_x + 1, 0);
                end_x = cmp::min(middle_x + end_x, frame.size.width as isize);
                let y = middle_y + formatted_left.transform.y - 1;
                if end_x >= 0
                    && start_x < frame.size.width as isize
                    && y >= 0
                    && y < frame.size.height as isize
                {
                    screen.draw_str(
                        (frame.origin.x as isize + start_x) as usize,
                        (frame.origin.y as isize + y) as usize,
                        connector_style,
                        &iter::repeat('-')
                            .take((end_x - start_x) as usize)
                            .collect::<String>(),
                    );
                }
            }

            // if let Some(parent) = revision.parent.as_ref() {
            //     revision_index = parent.index;
            //     revision = &tree.revisions[revision_index];
            //     screen.draw_str(
            //         frame.origin.x + middle_x,
            //         y.saturating_sub(1),
            //         theme.buffer.syntax.text,
            //         "|",
            //     );
            //     y = y.saturating_sub(2);
            // } else {
            //     break;
            // }
        }
    }

    pub fn reduce(&mut self, action: Action, context: &Context) -> Result<()> {
        match action {
            Action::Up => Ok(()),
            Action::Down => Ok(()),
        }
    }
}
