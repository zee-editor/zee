use ropey::Rope;
use smallvec::SmallVec;
use std::{
    ops::{Deref, DerefMut, Range},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tree_sitter::{
    InputEdit as TreeSitterInputEdit, Language, Node, Parser, Point as TreeSitterPoint, Tree,
    TreeCursor,
};

use super::{buffer::BufferTask, cursor::Cursor, Scheduler, TaskKind};
use crate::{
    error::{Error, Result},
    jobs::JobId as TaskId,
    smallstring::SmallString,
    terminal::{Background, Style},
};

pub struct ParserStatus {
    parser: CancelableParser,
    parsed: Option<ParsedSyntax>, // None if the parsing operation has been cancelled
}

pub struct ParsedSyntax {
    tree: Tree,
    text: Rope,
}

pub struct CancelFlag(Arc<AtomicUsize>);

const CANCEL_FLAG_UNSET: usize = 0;
const CANCEL_FLAG_SET: usize = 1;

impl CancelFlag {
    fn set(&self) {
        self.0.store(CANCEL_FLAG_SET, Ordering::SeqCst);
    }

    fn clear(&self) {
        self.0.store(CANCEL_FLAG_UNSET, Ordering::SeqCst);
    }
}

pub struct CancelableParser {
    flag: CancelFlag,
    parser: Parser,
}

impl CancelableParser {
    fn new(parser: Parser) -> Self {
        let flag = CancelFlag(Arc::new(AtomicUsize::new(CANCEL_FLAG_UNSET)));
        unsafe {
            parser.set_cancellation_flag(Some(&flag.0));
        }
        Self { flag, parser }
    }

    fn cancel_flag(&self) -> CancelFlag {
        CancelFlag(Arc::clone(&self.flag.0))
    }

    fn reset_cancel_flag(&self) {
        self.flag.clear();
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

pub struct NodeTrace<T> {
    pub path: Vec<SmallString>,
    pub nth_children: SmallVec<[u16; 32]>,
    pub trace: SmallVec<[T; 32]>,
    pub is_error: bool,
    pub byte_range: Range<usize>,
}

impl<T> NodeTrace<T> {
    pub fn new() -> Self {
        Self {
            path: Vec::new(),
            nth_children: SmallVec::new(),
            trace: SmallVec::new(),
            is_error: false,
            byte_range: 0..0,
        }
    }

    pub fn clear(&mut self) {
        self.path.clear();
        self.nth_children.clear();
        self.trace.clear();
        self.is_error = false;
        self.byte_range = 0..0;
    }
}

pub struct SyntaxCursor<'a> {
    root: Node<'a>,
    cursor: TreeCursor<'a>,
}

impl<'a> SyntaxCursor<'a> {
    #[inline]
    pub fn trace_at<T>(
        &mut self,
        trace: &mut NodeTrace<T>,
        byte_index: usize,
        map_node: impl Fn(&Node<'a>) -> T,
    ) {
        if trace.byte_range.contains(&byte_index) {
            return;
        }

        self.cursor.reset(self.root);
        trace.clear();

        trace.is_error = trace.is_error || self.cursor.node().is_error();
        trace.path.push(self.cursor.node().kind().into());
        trace.trace.push(map_node(&self.cursor.node()));
        trace.nth_children.push(0);
        while let Some(nth_child) = self.cursor.goto_first_child_for_byte(byte_index) {
            trace.is_error = trace.is_error || self.cursor.node().is_error();
            trace.path.push(self.cursor.node().kind().into());
            trace.trace.push(map_node(&self.cursor.node()));
            trace.nth_children.push(nth_child as u16);
        }
        trace.trace.reverse();
        trace.nth_children.reverse();

        let node = self.cursor.node();
        trace.byte_range = node.start_byte()..node.end_byte();
    }
}

pub struct SyntaxTree {
    language: Language,
    parsers: Vec<CancelableParser>,
    pub tree: Option<Tree>,
    current_parse_task: Option<(TaskId, CancelFlag)>,
}

impl SyntaxTree {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            parsers: vec![],
            tree: None,
            current_parse_task: None,
        }
    }

    pub fn cursor(&self) -> Option<SyntaxCursor> {
        self.tree.as_ref().map(|tree| {
            let root_node = tree.root_node();
            SyntaxCursor {
                cursor: root_node.walk(),
                root: root_node,
            }
        })
    }

    pub fn ensure_tree(
        &mut self,
        scheduler: &mut Scheduler,
        tree_fn: impl FnOnce() -> Rope,
    ) -> Result<()> {
        match (self.tree.as_ref(), self.current_parse_task.as_ref()) {
            (None, None) => self.spawn_parse_task(scheduler, tree_fn()),
            _ => Ok(()),
        }
    }

    pub fn spawn_parse_task(&mut self, scheduler: &mut Scheduler, text: Rope) -> Result<()> {
        let mut parser =
            self.parsers
                .pop()
                .map(|parser| Ok(parser))
                .unwrap_or_else(|| -> Result<_> {
                    let mut parser = Parser::new();
                    parser
                        .set_language(self.language)
                        .map_err(|error| Error::IncompatibleLanguageGrammar(error))?;
                    Ok(CancelableParser::new(parser))
                })?;

        let cancel_flag = parser.cancel_flag();
        let tree = self.tree.clone();
        let task_id = scheduler.spawn(move || {
            let maybe_tree = parser.parse_with(
                &mut |byte_index, _| {
                    let (chunk, chunk_byte_idx, _, _) = text.chunk_at_byte(byte_index);
                    assert!(byte_index >= chunk_byte_idx);
                    &chunk.as_bytes()[byte_index - chunk_byte_idx..]
                },
                tree.as_ref(),
            );
            Ok(match maybe_tree {
                Some(tree) => TaskKind::Buffer(BufferTask::ParseSyntax(ParserStatus {
                    parser,
                    parsed: Some(ParsedSyntax { tree, text }),
                })),
                None => TaskKind::Buffer(BufferTask::ParseSyntax(ParserStatus {
                    parser,
                    parsed: None,
                })),
            })
        })?;
        if let Some((_, old_cancel_flag)) = self.current_parse_task.as_ref() {
            old_cancel_flag.set();
        }
        self.current_parse_task = Some((task_id, cancel_flag));
        Ok(())
    }

    pub fn handle_parse_syntax_done(&mut self, task_id: TaskId, status: ParserStatus) {
        let ParserStatus { parser, parsed } = status;

        // Collect the parser for later use
        parser.reset_cancel_flag();
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
            assert!(tree.root_node().end_byte() <= text.len_bytes() + 1);
            self.tree = Some(tree.clone());
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub text: Style,
    pub text_current_line: Style,
    pub cursor_focused: Style,
    pub cursor_unfocused: Style,
    pub selection_background: Background,
    pub code_invalid: Style,
    pub code_constant: Style,
    pub code_keyword: Style,
    pub code_keyword_light: Style,
    pub code_string: Style,
    pub code_char: Style,
    pub code_operator: Style,
    pub code_macro_call: Style,
    pub code_function_call: Style,
    pub code_comment: Style,
    pub code_comment_doc: Style,
    pub code_link: Style,
    pub code_type: Style,
}

#[inline]
pub fn text_style_at_char(
    theme: &Theme,
    cursor: &Cursor,
    char_index: usize,
    focused: bool,
    line_under_cursor: bool,
    scope: &str,
    is_error: bool,
) -> Style {
    if cursor.range.contains(&char_index) {
        if focused {
            theme.cursor_focused
        } else {
            theme.cursor_unfocused
        }
    } else {
        let background = if cursor.selection().contains(&char_index) {
            theme.selection_background
        } else if line_under_cursor && focused {
            theme.text_current_line.background
        } else {
            theme.text.background
        };

        let style = if is_error {
            theme.code_invalid
        } else if scope.starts_with("constant") {
            theme.code_constant
        } else if scope.starts_with("string.quoted.double.dictionary.key.json")
            || scope.starts_with("support.property-name")
        {
            theme.code_keyword_light
        } else if scope.starts_with("string.quoted.double") {
            theme.code_string
        } else if scope.starts_with("string.quoted.single") {
            theme.code_char
        } else if scope.starts_with("string") {
            theme.code_string
        } else if scope.starts_with("keyword.operator") {
            theme.code_operator
        } else if scope.starts_with("storage")
            || scope.starts_with("keyword")
            || scope.starts_with("tag_name")
            || scope.ends_with("variable.self")
        {
            theme.code_keyword
        } else if scope.starts_with("variable.parameter.function")
            || scope.starts_with("identifier")
            || scope.starts_with("field_identifier")
        {
            theme.code_keyword_light
        } else if scope.starts_with("entity.name.enum")
            || scope.starts_with("support")
            || scope.starts_with("primitive_type")
        {
            theme.code_type
        } else if scope.starts_with("entity.attribute.name.punctuation") {
            theme.code_comment
        } else if scope.starts_with("entity.attribute.name")
            || scope.starts_with("entity.name.lifetime")
        {
            theme.code_macro_call
        } else if scope.starts_with("entity.name.macro.call") {
            theme.code_macro_call
        } else if scope.starts_with("entity.name.function") {
            theme.code_function_call
        } else if scope.starts_with("comment.block.line.docstr") {
            theme.code_comment_doc
        } else if scope.starts_with("comment") {
            theme.code_comment
        } else if ["<", ">", "/>", "</", "misc.other"]
            .iter()
            .any(|tag| scope.starts_with(tag))
        {
            theme.code_operator
        } else if scope.starts_with("markup.underline.link") {
            theme.code_link
        } else {
            theme.text
        };

        Style {
            background,
            foreground: style.foreground,
            bold: style.bold,
            underline: style.underline,
        }
    }
}
