use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ignore::WalkBuilder;
use std::{
    fs, iter,
    path::{Path, PathBuf},
};

use crate::{
    error::{Context as _Context, Result},
    utils::{self},
};

pub struct FilePicker {
    paths: Vec<PathBuf>,
    filtered: Vec<(usize, i64)>, // (index, score)
    matcher: Box<SkimMatcherV2>, // Boxed as it's big and we store a FilePicker in an enum variant
    prefix: PathBuf,
}

impl Clone for FilePicker {
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            filtered: self.filtered.clone(),
            matcher: Default::default(),
            prefix: self.prefix.clone(),
        }
    }
}

impl FilePicker {
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

    pub fn clear(&mut self) {
        self.reset(iter::empty(), "", "")
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
        utils::clear_path_buf(prefix);
        prefix.push(prefix_path);
        self.set_filter(filter);
    }

    pub fn selected(&self, filtered_index: usize) -> Option<&Path> {
        self.filtered
            .get(filtered_index)
            .map(|(index, _)| self.paths[*index].as_path())
    }
}

fn update_file_picker<FilesIterT>(
    file_picker: &mut FilePicker,
    path_str: String,
    files_iter: impl FnOnce(String) -> Result<FilesIterT>,
) -> Result<()>
where
    FilesIterT: Iterator<Item = PathBuf>,
{
    let prefix = Path::new(&path_str).parent().unwrap();
    if file_picker.prefix() != prefix {
        file_picker.reset(
            files_iter(path_str.clone())?.take(MAX_FILES_IN_PICKER),
            &path_str,
            &prefix,
        );
    } else {
        file_picker.set_filter(&path_str)
    }
    Ok(())
}

pub fn pick_from_directory(file_picker: &mut FilePicker, path_str: String) -> Result<()> {
    update_file_picker(file_picker, path_str, |path| {
        Ok(directory_files_iter(path)?.filter_map(|result_path| result_path.ok()))
    })
}

pub fn pick_from_repository(file_picker: &mut FilePicker, path_str: String) -> Result<()> {
    update_file_picker(file_picker, path_str, |path| {
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
