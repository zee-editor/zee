use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ignore::WalkBuilder;
use ropey::Rope;
use size_format::{SizeFormatterBinary, SizeFormatterSI};
use std::{
    borrow::Cow,
    cmp, fmt, fs,
    ops::Deref,
    path::{Path, PathBuf, MAIN_SEPARATOR},
    rc::Rc,
    time::SystemTime,
};
use time_humanize::HumanTime;
use zi::{
    components::{
        select::{Select, SelectProperties},
        text::{Text, TextAlign, TextProperties},
    },
    prelude::*,
    Callback,
};

use zee_edit::{movement, Cursor, Direction};

use super::{
    status::{Status, StatusProperties},
    Theme, PROMPT_MAX_HEIGHT,
};
use crate::{
    components::input::{Input, InputChange, InputProperties, InputStyle},
    editor::ContextHandle,
    error::{Context as _Context, Result},
    task::TaskId,
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
            Self::Repository => "find",
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
    fn list_files(&mut self) {
        let task = {
            let link = self.link.clone();
            let path_str = self.input.to_string();
            let mut listing = (*self.listing).clone();
            let source = self.properties.source;
            move |task_id| {
                link.send(Message::FileListingDone(
                    match source {
                        FileSource::Directory => pick_from_directory(&mut listing, path_str),
                        FileSource::Repository => pick_from_repository(&mut listing, path_str),
                    }
                    .map(|_| FileListingDone { task_id, listing }),
                ))
            }
        };
        self.current_task_id = Some(self.properties.context.task_pool.spawn(task));
    }

    fn height(&self) -> usize {
        2 + cmp::min(self.listing.num_filtered(), PROMPT_MAX_HEIGHT)
    }
}

impl Component for FilePicker {
    type Message = Message;
    type Properties = Properties;

    fn create(properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        let input = {
            let mut current_working_dir =
                Rope::from(properties.context.current_working_dir.to_string_lossy());
            current_working_dir.insert_char(current_working_dir.len_chars(), MAIN_SEPARATOR);
            current_working_dir
        };
        let mut cursor = Cursor::new();
        movement::move_to_end_of_line(&input, &mut cursor);

        let mut picker = Self {
            properties,
            link,
            input,
            cursor,
            listing: Rc::new(FileListing::new()),
            selected_index: 0,
            current_task_id: None,
        };
        picker.list_files();
        picker.properties.on_change_height.emit(picker.height());
        picker
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        let should_relist_files = self.properties.source != properties.source;
        self.properties = properties;
        if should_relist_files {
            self.list_files();
        }
        ShouldRender::Yes
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
                let new_input = {
                    let path_str: Cow<str> = self.input.slice(..).into();
                    let new_input = Path::new(path_str.deref());
                    new_input
                        .parent()
                        .unwrap_or(new_input)
                        .to_str()
                        .expect("utf-8 path as it's constructed from a utf-8 str")
                        .into()
                };

                self.input = new_input;
                movement::move_to_end_of_buffer(&self.input, &mut self.cursor);

                let ends_with_separator = self
                    .input
                    .chars_at(self.input.len_chars())
                    .reversed()
                    .next()
                    .map(|character| character == MAIN_SEPARATOR)
                    .unwrap_or(false);
                if !ends_with_separator {
                    self.cursor.insert_char(&mut self.input, MAIN_SEPARATOR);
                    movement::move_horizontally(
                        &self.input,
                        &mut self.cursor,
                        Direction::Forward,
                        1,
                    );
                }
                true
            }
            Message::AutocompletePath => {
                if let Some(path) = self.listing.selected(self.selected_index) {
                    self.input = path.to_string_lossy().into();
                    movement::move_to_end_of_line(&self.input, &mut self.cursor);
                    if path.is_dir() {
                        self.cursor.insert_char(&mut self.input, MAIN_SEPARATOR);
                        movement::move_horizontally(
                            &self.input,
                            &mut self.cursor,
                            Direction::Forward,
                            1,
                        );
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
            self.list_files();
        }

        if initial_height != self.height() {
            self.properties.on_change_height.emit(self.height());
        }

        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let input = Input::with(InputProperties {
            context: self.properties.context,
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
        let now = SystemTime::now();
        let item_at = move |index| {
            let path = listing.selected(index).unwrap_or_else(|| Path::new(""));
            let background = if index == selected_index {
                theme.item_focused_background
            } else {
                theme.item_unfocused_background
            };
            let (is_dir, formatted_size, formatted_last_modified) = match path.metadata().ok() {
                Some(metadata) => (
                    metadata.is_dir(),
                    format!(" {}     ", SizeFormatterBinary::new(metadata.len())),
                    metadata
                        .modified()
                        .ok()
                        .and_then(|last_modified| now.duration_since(last_modified).ok())
                        .map(HumanTime::from)
                        .map(|last_modified| {
                            last_modified.to_text_en(
                                time_humanize::Accuracy::Rough,
                                time_humanize::Tense::Past,
                            )
                        })
                        .unwrap_or_else(String::new),
                ),
                None => (false, String::new(), String::new()),
            };
            let name = &path.to_string_lossy()[listing
                .search_dir()
                .to_str()
                .map(|prefix| prefix.len())
                .unwrap_or(0)..];

            Item::fixed(1)(Container::row([
                Text::item_with_key(
                    FlexBasis::Auto,
                    format!("{}-name", name).as_str(),
                    TextProperties::new().content(name).style(if is_dir {
                        Style::bold(background, theme.item_directory_foreground)
                    } else {
                        Style::normal(background, theme.item_file_foreground)
                    }),
                ),
                Text::item_with_key(
                    FlexBasis::Fixed(16),
                    format!("{}-size", name).as_str(),
                    TextProperties::new()
                        .content(formatted_size)
                        .style(Style::normal(background, theme.file_size))
                        .align(TextAlign::Right),
                ),
                Text::item_with_key(
                    FlexBasis::Fixed(40),
                    format!("{}-last-modified", name).as_str(),
                    TextProperties::new()
                        .content(formatted_last_modified)
                        .style(Style::normal(background, theme.mode)),
                ),
            ]))
        };

        let formatted_num_results = format!(
            "{} of {}{}",
            SizeFormatterSI::new(u64::try_from(self.listing.num_filtered()).unwrap()),
            if self.listing.paths.len() >= MAX_FILES_IN_PICKER {
                "≥"
            } else {
                ""
            },
            SizeFormatterSI::new(u64::try_from(self.listing.paths.len()).unwrap())
        );

        Layout::column([
            if self.listing.num_filtered() == 0 {
                Text::item_with(
                    FlexBasis::Fixed(1),
                    TextProperties::new()
                        .content(if self.listing.paths.is_empty() {
                            "No files"
                        } else {
                            "No matching files"
                        })
                        .style(Style::normal(
                            self.properties.theme.item_unfocused_background,
                            self.properties.theme.action.background,
                        )),
                )
            } else {
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
                }))
            },
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
                Text::item_with_key(
                    FlexBasis::Fixed(16),
                    "num-results",
                    TextProperties::new()
                        .content(formatted_num_results)
                        .style(self.properties.theme.action.invert())
                        .align(TextAlign::Right),
                ),
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

/// A list of potentially filtered paths at a given location
struct FileListing {
    paths: Vec<PathBuf>,
    filtered: Vec<(usize, i64)>, // (index, score)
    matcher: Box<SkimMatcherV2>, // Boxed as it's big and we store a FileListing in an enum variant
    search_dir: PathBuf,
}

impl fmt::Debug for FileListing {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("FileListing")
            .field("paths", &self.paths)
            .field("filtered", &self.filtered)
            .field("matcher", &"SkimMatcherV2(...)")
            .field("search_dir", &self.search_dir)
            .finish()
    }
}

impl Clone for FileListing {
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            filtered: self.filtered.clone(),
            matcher: Default::default(),
            search_dir: self.search_dir.clone(),
        }
    }
}

impl FileListing {
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            filtered: Vec::new(),
            matcher: Default::default(),
            search_dir: PathBuf::new(),
        }
    }

    pub fn search_dir(&self) -> &Path {
        self.search_dir.as_path()
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
        search_path: &str,
        search_dir: impl AsRef<Path>,
    ) {
        self.paths.clear();
        self.paths.extend(paths_iter);
        self.search_dir.clear();
        self.search_dir.push(search_dir);
        self.set_filter(search_path);
    }

    pub fn selected(&self, filtered_index: usize) -> Option<&Path> {
        self.filtered
            .get(filtered_index)
            .map(|(index, _)| self.paths[*index].as_path())
    }

    fn update<FilesIterT>(
        &mut self,
        path_str: String,
        files_iter: impl FnOnce(&Path) -> Result<FilesIterT>,
    ) -> Result<()>
    where
        FilesIterT: Iterator<Item = PathBuf>,
    {
        let search_dir = {
            let path = Path::new(&path_str);
            if path_str.ends_with(MAIN_SEPARATOR) {
                path
            } else {
                path.parent().unwrap_or(path)
            }
        };

        if self.search_dir != search_dir {
            self.reset(
                files_iter(search_dir)?.take(MAX_FILES_IN_PICKER),
                &path_str,
                &search_dir,
            );
        } else {
            self.set_filter(&path_str)
        }
        Ok(())
    }
}

fn pick_from_directory(listing: &mut FileListing, search_path: String) -> Result<()> {
    listing.update(search_path, |search_dir| {
        Ok(directory_files_iter(search_dir)?.filter_map(|result_path| result_path.ok()))
    })
}

fn pick_from_repository(listing: &mut FileListing, search_path: String) -> Result<()> {
    listing.update(search_path, |search_dir| {
        Ok(repository_files_iter(search_dir).filter_map(|result_path| result_path.ok()))
    })
}

fn directory_files_iter(search_dir: &Path) -> Result<impl Iterator<Item = Result<PathBuf>>> {
    let walk = fs::read_dir(search_dir).context("Cannot read entry while walking directory")?;
    Ok(walk.map(|entry| {
        entry
            .map(|entry| entry.path())
            .context("Cannot read entry while walking directory")
    }))
}

fn repository_files_iter(search_dir: &Path) -> impl Iterator<Item = Result<PathBuf>> {
    WalkBuilder::new(search_dir).build().filter_map(|entry| {
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
