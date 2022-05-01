use ropey::Rope;
use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tree_sitter::{
    InputEdit as TreeSitterInputEdit, Language, Parser, Point as TreeSitterPoint, Tree,
};

use zee_edit::OpaqueDiff;

use crate::{
    error::Result,
    task::{TaskId, TaskPool},
};

pub struct ParserStatus {
    task_id: TaskId,
    parser: CancelableParser,
    parsed: Option<ParsedSyntax>, // None if the parsing operation has been cancelled
}

impl fmt::Debug for ParserStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "ParserStatus {{ task_id: {:?}, .. }}",
            self.task_id
        )
    }
}

pub struct ParsedSyntax {
    tree: Tree,
    text: Rope,
}

#[derive(Clone)]
pub struct ParseTree {
    pub version: usize,
    pub tree: Tree,
}

impl Deref for ParseTree {
    type Target = Tree;

    fn deref(&self) -> &Self::Target {
        &self.tree
    }
}

impl DerefMut for ParseTree {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tree
    }
}

pub struct ParserPool {
    pub tree: Option<ParseTree>,
    language: Language,
    parsers: Vec<CancelableParser>,
    current_parse_task: Option<(TaskId, CancelFlag)>,
}

impl ParserPool {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            parsers: vec![],
            tree: None,
            current_parse_task: None,
        }
    }

    pub fn ensure_tree(
        &mut self,
        task_pool: &TaskPool,
        tree_fn: impl FnOnce() -> Rope,
        on_parse: impl FnOnce(Result<ParserStatus>) + Send + 'static,
    ) {
        if let (None, None) = (self.tree.as_ref(), self.current_parse_task.as_ref()) {
            self.spawn(task_pool, tree_fn(), true, on_parse);
        }
    }

    pub fn spawn(
        &mut self,
        task_pool: &TaskPool,
        text: Rope,
        fresh: bool,
        on_parse: impl FnOnce(Result<ParserStatus>) + Send + 'static,
    ) {
        let mut parser = self.parsers.pop().unwrap_or_else(|| {
            let mut parser = Parser::new();
            parser
                .set_language(self.language)
                .expect("Incompatible language grammar");
            CancelableParser::new(parser)
        });

        let cancel_flag = parser.cancel_flag().clone();
        let raw_tree = self.tree.clone().map(|tree| tree.tree);
        let task_id = task_pool.spawn(move |task_id| {
            let maybe_tree = parser.parse_with(
                &mut |byte_index, _| {
                    let (chunk, chunk_byte_idx, _, _) = text.chunk_at_byte(byte_index);
                    assert!(byte_index >= chunk_byte_idx);

                    &chunk.as_bytes()[byte_index - chunk_byte_idx..]
                },
                if fresh { None } else { raw_tree.as_ref() },
            );
            // Reset the parser for later reuse
            parser.reset();

            on_parse(match maybe_tree {
                Some(tree) => Ok(ParserStatus {
                    task_id,
                    parser,
                    parsed: Some(ParsedSyntax { tree, text }),
                }),
                None => Ok(ParserStatus {
                    task_id,
                    parser,
                    parsed: None,
                }),
            });
        });
        if let Some((_, old_cancel_flag)) = self.current_parse_task.as_ref() {
            old_cancel_flag.set();
        }
        self.current_parse_task = Some((task_id, cancel_flag));
    }

    pub fn handle_parse_syntax_done(&mut self, version: usize, status: ParserStatus) {
        let ParserStatus {
            task_id,
            parser,
            parsed,
        } = status;

        // Collect the parser for later reuse
        parser.cancel_flag().clear();
        self.parsers.push(parser);

        // If we weren't waiting for this task, return
        if self
            .current_parse_task
            .as_ref()
            .map(|(expected_task_id, _)| *expected_task_id != task_id)
            .unwrap_or(true)
        {
            return;
        }
        self.current_parse_task = None;

        // If the parser task hasn't been cancelled, store the new syntax tree
        if let Some(ParsedSyntax { tree, text }) = parsed {
            assert!(tree.root_node().end_byte() <= text.len_bytes());
            self.tree = Some(ParseTree { version, tree });
        }
    }

    pub fn edit(&mut self, diff: &OpaqueDiff) {
        match self.tree {
            Some(ref mut tree) if !diff.is_empty() => {
                tree.edit(&TreeSitterInputEdit {
                    start_byte: diff.byte_index,
                    old_end_byte: diff.byte_index + diff.old_byte_length,
                    new_end_byte: diff.byte_index + diff.new_byte_length,
                    // I don't use tree sitter's line/col tracking; I'm assuming
                    // here that passing in dummy values doesn't cause any other
                    // problem apart from incorrect line/col after editing a tree.
                    start_position: TreeSitterPoint::new(0, 0),
                    old_end_position: TreeSitterPoint::new(0, 0),
                    new_end_position: TreeSitterPoint::new(0, 0),
                });
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct CancelFlag(Arc<AtomicUsize>);

impl CancelFlag {
    fn set(&self) {
        self.0.store(CANCEL_FLAG_SET, Ordering::SeqCst);
    }

    fn clear(&self) {
        self.0.store(CANCEL_FLAG_UNSET, Ordering::SeqCst);
    }
}

struct CancelableParser {
    // `parser` should appear before `flag` as it holds a reference to the
    // cancellation flag and should be destroyed first
    parser: Parser,
    flag: CancelFlag,
}

impl CancelableParser {
    fn new(mut parser: Parser) -> Self {
        // A `tree_sitter::Parser` can hold a pointer to a cancellation flag
        // that it polls periodically. This is polled from C and it is up to the
        // caller to ensure that the pointer lives at least as long as the
        // Parser. The call here is safe as Rust guarantees that struct fields
        // are dropped in the same order as they are declared.
        //
        // SAFETY: The parser cannot be running at the time when the struct is
        // destroyed, so it can't be polling the flag. It still holds a pointer
        // to the flag and it'd technically be UB if the flag was dropped first.
        let flag = CancelFlag(Arc::new(AtomicUsize::new(CANCEL_FLAG_UNSET)));
        unsafe {
            parser.set_cancellation_flag(Some(&flag.0));
        }
        Self { flag, parser }
    }

    fn cancel_flag(&self) -> &CancelFlag {
        &self.flag
    }
}

impl Deref for CancelableParser {
    type Target = Parser;

    fn deref(&self) -> &Self::Target {
        &self.parser
    }
}

impl DerefMut for CancelableParser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parser
    }
}

const CANCEL_FLAG_UNSET: usize = 0;
const CANCEL_FLAG_SET: usize = 1;
