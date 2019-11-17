// Original implementation https://github.com/cessen/led

use ropey::RopeSlice;
use smallvec::SmallVec;
use std::{self, borrow::Borrow, ops::Deref, ptr, str};

#[derive(Clone, Default)]
pub struct SmallString {
    buffer: SmallVec<[u8; 8]>,
}

impl SmallString {
    /// Creates a new empty `SmallString`
    #[allow(dead_code)]
    #[inline]
    pub fn new() -> Self {
        SmallString {
            buffer: SmallVec::new(),
        }
    }

    /// Creates a new empty `SmallString` with at least `capacity` bytes
    /// of capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        SmallString {
            buffer: SmallVec::with_capacity(capacity),
        }
    }

    /// Creates a new `SmallString` with the same contents as the given `&str`.
    pub fn from_str(text: &str) -> Self {
        let mut string = SmallString::with_capacity(text.len());
        unsafe { string.insert_bytes(0, text.as_bytes()) };
        string
    }

    /// Creates a new `SmallString` with the same contents as the given `&str`.
    pub fn from_rope_slice(text: &RopeSlice) -> Self {
        let mut string = SmallString::with_capacity(text.len_bytes());
        let mut idx = 0;
        for chunk in text.chunks() {
            unsafe { string.insert_bytes(idx, chunk.as_bytes()) };
            idx += chunk.len();
        }
        string
    }

    /// Appends a `&str` to end the of the `SmallString`.
    pub fn push_str(&mut self, string: &str) {
        let len = self.len();
        unsafe {
            self.insert_bytes(len, string.as_bytes());
        }
    }

    /// Drops the text after byte index `idx`.
    ///
    /// Panics if `idx` is not a char boundary, as that would result in an
    /// invalid utf8 string.
    pub fn truncate(&mut self, idx: usize) {
        assert!(self.is_char_boundary(idx));
        debug_assert!(idx <= self.len());
        self.buffer.truncate(idx);
    }

    pub fn clear(&mut self) {
        self.truncate(0);
    }

    #[inline]
    unsafe fn insert_bytes(&mut self, idx: usize, bytes: &[u8]) {
        assert!(idx <= self.len());
        let len = self.len();
        let amt = bytes.len();
        self.buffer.reserve(amt);

        ptr::copy(
            self.buffer.as_ptr().add(idx),
            self.buffer.as_mut_ptr().add(idx + amt),
            len - idx,
        );
        ptr::copy(bytes.as_ptr(), self.buffer.as_mut_ptr().add(idx), amt);
        self.buffer.set_len(len + amt);
    }
}

impl std::cmp::PartialEq for SmallString {
    fn eq(&self, other: &Self) -> bool {
        let (s1, s2): (&str, &str) = (self, other);
        s1 == s2
    }
}

impl<'a> PartialEq<SmallString> for &'a str {
    fn eq(&self, other: &SmallString) -> bool {
        *self == (other as &str)
    }
}

impl<'a> PartialEq<&'a str> for SmallString {
    fn eq(&self, other: &&'a str) -> bool {
        (self as &str) == *other
    }
}

impl std::fmt::Display for SmallString {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        SmallString::deref(self).fmt(fm)
    }
}

impl std::fmt::Debug for SmallString {
    fn fmt(&self, fm: &mut std::fmt::Formatter) -> std::fmt::Result {
        SmallString::deref(self).fmt(fm)
    }
}

impl<'a> From<&'a str> for SmallString {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl<'a> From<RopeSlice<'a>> for SmallString {
    fn from(slice: RopeSlice) -> Self {
        Self::from_rope_slice(&slice)
    }
}

impl Into<String> for SmallString {
    fn into(self) -> String {
        unsafe { String::from_utf8_unchecked(self.buffer.to_vec()) }
    }
}

impl Deref for SmallString {
    type Target = str;

    fn deref(&self) -> &str {
        // SmallString's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl AsRef<str> for SmallString {
    fn as_ref(&self) -> &str {
        // SmallString's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}

impl Borrow<str> for SmallString {
    fn borrow(&self) -> &str {
        // SmallString's methods don't allow `buffer` to become invalid utf8,
        // so this is safe.
        unsafe { str::from_utf8_unchecked(self.buffer.as_ref()) }
    }
}
