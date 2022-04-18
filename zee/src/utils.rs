use ropey::Rope;

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
