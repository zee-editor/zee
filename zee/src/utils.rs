use ropey::Rope;
use std::path::PathBuf;

#[derive(Copy)]
pub struct StaticRefEq<T: 'static>(&'static T);

impl<T> Clone for StaticRefEq<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T> PartialEq for StaticRefEq<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl<T> std::ops::Deref for StaticRefEq<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T> From<&'static T> for StaticRefEq<T> {
    fn from(other: &'static T) -> Self {
        Self(other)
    }
}

pub fn ensure_trailing_newline_with_content(text: &mut Rope) {
    if text.len_chars() == 0 || text.char(text.len_chars() - 1) != '\n' {
        text.insert_char(text.len_chars(), '\n');
    }
}
/// Given a path and extension, find all child files with the provided extension if the path is
/// a directory. If the path is a file or non-existent, an vector will be returned.
pub fn files_with_extension(base_path: &PathBuf, extension: &str) -> anyhow::Result<Vec<PathBuf>> {
    if base_path.is_dir() && base_path.exists() {
        Ok(std::fs::read_dir(base_path)?
            .into_iter()
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap().path())
            .filter(|r| {
                r.extension()
                    .filter(|e| e.to_str() == Some(extension))
                    .is_some()
            })
            .collect())
    } else {
        Ok(vec![])
    }
}
