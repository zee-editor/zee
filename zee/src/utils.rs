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
