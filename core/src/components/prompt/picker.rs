use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ignore::WalkBuilder;
use ropey::Rope;
use std::{
    borrow::Cow,
    cmp, fmt, fs,
    path::{Path, PathBuf},
    rc::Rc,
};
use zi::{
    components::{
        input::{Cursor, Input, InputChange, InputProperties, InputStyle},
        select::{Select, SelectProperties},
        text::{Text, TextProperties},
    },
    prelude::*,
    Callback,
};

use super::{
    status::{Status, StatusProperties},
    Theme, PROMPT_MAX_HEIGHT,
};
use crate::{
    editor::ContextHandle,
    error::{Context as _Context, Result},
    task::TaskId,
    utils,
};

#[derive(Debug)]
pub struct FileListingDone {
    task_id: TaskId,
    listing: FileListing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FileSource {
    Directory,
    Repository,
}

impl FileSource {
    fn status_name(&self) -> Cow<'static, str> {
        match self {
            Self::Directory => "open",
            Self::Repository => "repo",
        }
        .into()
    }
}

#[derive(Debug)]
pub enum Message {
    FileListingDone(Result<FileListingDone>),
    OpenFile,

    // Path navigation
    AutocompletePath,
    ChangePath(InputChange),
    ChangeSelectedFile(usize),
    SelectParentDirectory,
}

#[derive(Clone)]
pub struct Properties {
    pub context: ContextHandle,
    pub theme: Cow<'static, Theme>,
    pub source: FileSource,
    pub on_open: Callback<PathBuf>,
    pub on_change_height: Callback<usize>,
}

pub struct FilePicker {
    properties: Properties,
    link: ComponentLink<Self>,
    input: Rope,
    cursor: Cursor,
    listing: Rc<FileListing>,
    selected_index: usize,
    current_task_id: Option<TaskId>,
}

impl FilePicker {
    fn list_files(&mut self, source: FileSource) {
        let link = self.link.clone();
        let input = self.input.clone();
        let mut listing = (*self.listing).clone();
        self.current_task_id = Some(self.properties.context.task_pool.spawn(move |task_id| {
            let path_str = input.to_string();
            link.send(Message::FileListingDone(match source {
                FileSource::Directory => pick_from_directory(&mut listing, path_str)
                    .map(|_| FileListingDone { task_id, listing }),
                FileSource::Repository => pick_from_repository(&mut listing, path_str)
                    .map(|_| FileListingDone { task_id, listing }),
            }))
        }))
    }

    fn height(&self) -> usize {
        1 + cmp::min(self.listing.num_filtered(), PROMPT_MAX_HEIGHT)
    }
}

impl Component for FilePicker {
    type Message = Message;
    type Properties = Properties;

    fn create(properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        let mut cursor = Cursor::new();
        let mut current_working_dir: String = properties
            .context
            .current_working_dir
            .to_string_lossy()
            .into();
        current_working_dir.push('/');
        current_working_dir.push('\n');
        let input = current_working_dir.into();
        cursor.move_to_end_of_line(&input);

        let mut picker = Self {
            properties,
            link,
            input,
            cursor,
            listing: Rc::new(FileListing::new()),
            selected_index: 0,
            current_task_id: None,
        };
        picker.list_files(picker.properties.source);
        picker.properties.on_change_height.emit(picker.height());
        picker
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        if self.properties.source != properties.source {
            self.list_files(properties.source);
        }
        let should_render = (self.properties.theme != properties.theme).into();
        self.properties = properties;
        should_render
    }

    fn update(&mut self, message: Message) -> ShouldRender {
        let initial_height = self.height();
        let input_changed = match message {
            Message::OpenFile => {
                let path_str: Cow<str> = self.input.slice(..).into();
                let path = PathBuf::from(path_str.trim());
                self.properties.on_open.emit(path);
                false
            }
            Message::SelectParentDirectory => {
                let path_str: String = self.input.slice(..).into();
                self.input = Path::new(&path_str.trim())
                    .parent()
                    .map(|parent| parent.to_string_lossy())
                    .unwrap_or_else(|| "".into())
                    .into();
                utils::ensure_trailing_newline_with_content(&mut self.input);
                self.cursor.move_to_end_of_line(&self.input);
                self.cursor.insert_char(&mut self.input, '/');
                self.cursor.move_right(&self.input);
                true
            }
            Message::AutocompletePath => {
                if let Some(path) = self.listing.selected(self.selected_index) {
                    self.input = path.to_string_lossy().into();
                    utils::ensure_trailing_newline_with_content(&mut self.input);
                    self.cursor.move_to_end_of_line(&self.input);
                    if path.is_dir() {
                        self.cursor.insert_char(&mut self.input, '/');
                        self.cursor.move_right(&self.input);
                    }
                    self.selected_index = 0;
                    true
                } else {
                    false
                }
            }
            Message::ChangePath(InputChange { content, cursor }) => {
                self.cursor = cursor;
                if let Some(content) = content {
                    self.input = content;
                    true
                } else {
                    false
                }
            }
            Message::ChangeSelectedFile(index) => {
                self.selected_index = index;
                false
            }
            Message::FileListingDone(Ok(FileListingDone { task_id, listing }))
                if self
                    .current_task_id
                    .as_ref()
                    .map(|&expected_task_id| expected_task_id == task_id)
                    .unwrap_or(false) =>
            {
                self.listing = Rc::new(listing);
                self.current_task_id = None;
                self.selected_index = 0;

                false
            }
            _ => {
                return ShouldRender::No;
            }
        };

        if input_changed {
            self.list_files(self.properties.source);
        }

        if initial_height != self.height() {
            self.properties.on_change_height.emit(self.height());
        }

        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let input = Input::with(InputProperties {
            style: InputStyle {
                content: self.properties.theme.input,
                cursor: self.properties.theme.cursor,
            },
            content: self.input.clone(),
            cursor: self.cursor.clone(),
            on_change: Some(self.link.callback(Message::ChangePath)),
            focused: true,
        });

        let listing = self.listing.clone();
        let selected_index = self.selected_index;
        let theme = self.properties.theme.clone();
        let item_at = move |index| {
            let path = listing.selected(index).unwrap();
            let background = if index == selected_index {
                theme.item_focused_background
            } else {
                theme.item_unfocused_background
            };
            let style = if path.is_dir() {
                Style::bold(background, theme.item_directory_foreground)
            } else {
                Style::normal(background, theme.item_file_foreground)
            };
            let content = &path.to_string_lossy()[listing
                .prefix()
                .to_str()
                .map(|prefix| prefix.len() + 1)
                .unwrap_or(0)..];
            Item::fixed(1)(Text::with_key(
                content,
                TextProperties::new().content(content).style(style),
            ))
        };
        Layout::column([
            Item::auto(Select::with(SelectProperties {
                background: Style::normal(
                    self.properties.theme.item_unfocused_background,
                    self.properties.theme.item_file_foreground,
                ),
                direction: FlexDirection::ColumnReverse,
                item_at: item_at.into(),
                focused: true,
                num_items: self.listing.num_filtered(),
                selected: self.selected_index,
                on_change: self.link.callback(Message::ChangeSelectedFile).into(),
                item_size: 1,
            })),
            Item::fixed(1)(Container::row([
                Item::fixed(4)(Status::with(StatusProperties {
                    action_name: self.properties.source.status_name(),
                    pending: self.current_task_id.is_some(),
                    style: self.properties.theme.action,
                })),
                Item::fixed(1)(Text::with(
                    TextProperties::new().style(self.properties.theme.input),
                )),
                Item::auto(input),
            ])),
        ])
    }

    fn bindings(&self, bindings: &mut Bindings<Self>) {
        if !bindings.is_empty() {
            return;
        }

        bindings.set_focus(true);

        bindings.add("open-file", [Key::Char('\n')], || Message::OpenFile);
        bindings.add("select-parent-directory", [Key::Ctrl('l')], || {
            Message::SelectParentDirectory
        });
        bindings.add("autocomplete-path", [Key::Char('\t')], || {
            Message::AutocompletePath
        });
    }
}

struct FileListing {
    paths: Vec<PathBuf>,
    filtered: Vec<(usize, i64)>, // (index, score)
    matcher: Box<SkimMatcherV2>, // Boxed as it's big and we store a FileListing in an enum variant
    prefix: PathBuf,
}

impl fmt::Debug for FileListing {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("FileListing")
            .field("paths", &self.paths)
            .field("filtered", &self.filtered)
            .field("matcher", &"SkimMatcherV2(...)")
            .field("prefix", &self.prefix)
            .finish()
    }
}

impl Clone for FileListing {
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            filtered: self.filtered.clone(),
            matcher: Default::default(),
            prefix: self.prefix.clone(),
        }
    }
}

impl FileListing {
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            filtered: Vec::new(),
            matcher: Default::default(),
            prefix: PathBuf::new(),
        }
    }

    pub fn prefix(&self) -> &Path {
        self.prefix.as_path()
    }

    pub fn num_filtered(&self) -> usize {
        self.filtered.len()
    }

    pub fn set_filter(&mut self, filter: &str) {
        let Self {
            ref mut paths,
            ref mut filtered,
            ref mut matcher,
            ..
        } = *self;
        filtered.clear();
        filtered.extend(paths.iter().enumerate().filter_map(|(index, file)| {
            matcher
                .fuzzy_match(&file.to_string_lossy(), filter.trim())
                .map(|score| (index, score))
        }));
        filtered.sort_unstable_by_key(|(_, score)| -score);
    }

    pub fn reset(
        &mut self,
        paths_iter: impl Iterator<Item = PathBuf>,
        filter: &str,
        prefix_path: impl AsRef<Path>,
    ) {
        let Self {
            ref mut paths,
            ref mut prefix,
            ..
        } = *self;
        paths.clear();
        paths.extend(paths_iter);
        prefix.clear();
        prefix.push(prefix_path);
        self.set_filter(filter);
    }

    pub fn selected(&self, filtered_index: usize) -> Option<&Path> {
        self.filtered
            .get(filtered_index)
            .map(|(index, _)| self.paths[*index].as_path())
    }
}

fn update_listing<FilesIterT>(
    listing: &mut FileListing,
    path_str: String,
    files_iter: impl FnOnce(String) -> Result<FilesIterT>,
) -> Result<()>
where
    FilesIterT: Iterator<Item = PathBuf>,
{
    let prefix = Path::new(&path_str).parent().unwrap();
    if listing.prefix() != prefix {
        listing.reset(
            files_iter(path_str.clone())?.take(MAX_FILES_IN_PICKER),
            &path_str,
            &prefix,
        );
    } else {
        listing.set_filter(&path_str)
    }
    Ok(())
}

fn pick_from_directory(listing: &mut FileListing, path_str: String) -> Result<()> {
    update_listing(listing, path_str, |path| {
        Ok(directory_files_iter(path)?.filter_map(|result_path| result_path.ok()))
    })
}

fn pick_from_repository(listing: &mut FileListing, path_str: String) -> Result<()> {
    update_listing(listing, path_str, |path| {
        Ok(repository_files_iter(path).filter_map(|result_path| result_path.ok()))
    })
}

fn directory_files_iter(path: impl AsRef<Path>) -> Result<impl Iterator<Item = Result<PathBuf>>> {
    Ok(
        fs::read_dir(path.as_ref().parent().unwrap_or_else(|| path.as_ref())).map(|walk| {
            walk.map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .context("Cannot read entry while walking directory")
            })
        })?,
    )
}

fn repository_files_iter(path: impl AsRef<Path>) -> impl Iterator<Item = Result<PathBuf>> {
    WalkBuilder::new(path.as_ref().parent().unwrap_or_else(|| path.as_ref()))
        .build()
        .filter_map(|entry| {
            let is_dir = entry
                .as_ref()
                .map(|entry| entry.path().is_dir())
                .unwrap_or(false);
            if entry.is_ok() && !is_dir {
                Some(
                    entry
                        .map(|entry| entry.path().to_path_buf())
                        .context("Cannot read entry while walking directory"),
                )
            } else {
                None
            }
        })
}

const MAX_FILES_IN_PICKER: usize = 16384;
