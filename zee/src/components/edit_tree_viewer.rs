use std::cmp;
use zee_edit::tree::{self, EditTree};
use zi::{Canvas, Component, ComponentLink, Layout, Rect, ShouldRender, Style};

use crate::versioned::WeakHandle;

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub current_revision: Style,
    pub master_revision: Style,
    pub master_connector: Style,
    pub alternate_revision: Style,
    pub alternate_connector: Style,
}

pub struct Properties {
    pub theme: Theme,
    pub tree: WeakHandle<EditTree>,
}

pub struct EditTreeViewer {
    properties: Properties,
    frame: Rect,
}

impl Component for EditTreeViewer {
    type Properties = Properties;
    type Message = ();

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self { properties, frame }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        self.properties = properties;
        ShouldRender::Yes
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self {
            frame,
            properties:
                Properties {
                    ref tree,
                    ref theme,
                },
        } = *self;
        let tree = tree.upgrade();
        let mut canvas = Canvas::new(frame.size);
        canvas.clear(theme.current_revision);

        let formatted_tree = tree::format_tree(&tree);

        let (middle_x, middle_y) = {
            let transform = formatted_tree[tree.head_index].transform;
            let middle_x = (canvas.size().width / 2) as isize - transform.x;
            let middle_y = (canvas.size().height / 2) as isize - transform.y;
            (middle_x, middle_y)
        };

        // let mut y = middle_y + 8;
        // let mut revision_index = tree.parent_revision_index;
        for (revision_index, formatted) in formatted_tree.iter().enumerate() {
            let (revision_style, connector_style) = if revision_index == tree.head_index {
                (theme.current_revision, theme.master_connector)
            } else if formatted.current_branch {
                (theme.master_revision, theme.master_connector)
            } else {
                (theme.alternate_revision, theme.alternate_connector)
            };
            let revision = &tree.revisions[revision_index];
            let x = middle_x + formatted.transform.x;
            let y = middle_y + formatted.transform.y;
            if x >= 0
                && y >= 0
                && x < canvas.size().width as isize
                && y < canvas.size().height as isize
            {
                canvas.draw_str(
                    x as usize,
                    y as usize,
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

            let num_children = revision.children.len();
            for (child_index, child) in revision.children.iter().enumerate() {
                let formatted_child = &formatted_tree[child.index];
                let x = middle_x + formatted_child.transform.x;
                let y = middle_y + formatted_child.transform.y - 1;
                if x >= 0
                    && y >= 0
                    && x < canvas.size().width as isize
                    && y < canvas.size().height as isize
                {
                    let connector = if child_index == 0 {
                        if num_children > 1 {
                            "├"
                        } else {
                            "│"
                        }
                    } else if child_index == num_children - 1 {
                        "┐"
                    } else {
                        "┬"
                    };
                    canvas.draw_str(x as usize, y as usize, connector_style, connector);
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
                end_x = cmp::min(middle_x + end_x, canvas.size().width as isize);
                let y = middle_y + formatted_left.transform.y - 1;
                if end_x >= 0
                    && start_x < canvas.size().width as isize
                    && y >= 0
                    && y < canvas.size().height as isize
                {
                    canvas.draw_str(
                        start_x as usize,
                        y as usize,
                        connector_style,
                        &"─".repeat((end_x - start_x) as usize),
                    );
                }
            }

            // if let Some(parent) = revision.parent.as_ref() {
            //     revision_index = parent.index;
            //     revision = &tree.revisions[revision_index];
            //     canvas.draw_str(
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

        canvas.into()
    }
}
