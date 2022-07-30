use std::{
    ops::{Deref, DerefMut},
    rc::{Rc, Weak},
};

pub struct Versioned<T> {
    value: Rc<T>,
    version: usize,
}

impl<T> Versioned<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Rc::new(value),
            version: 0,
        }
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn weak(&self) -> WeakHandle<T> {
        WeakHandle {
            value: Rc::downgrade(&self.value),
            version: self.version,
        }
    }
}

impl<T> Deref for Versioned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: Clone> DerefMut for Versioned<T> {
    fn deref_mut(&mut self) -> &mut T {
        assert_eq!(Rc::strong_count(&self.value), 1);
        self.version += 1;
        Rc::make_mut(&mut self.value)
    }
}

#[derive(Clone)]
pub struct WeakHandle<T> {
    value: Weak<T>,
    version: usize,
}

impl<T> WeakHandle<T> {
    pub fn upgrade(&self) -> Rc<T> {
        self.value
            .upgrade()
            .expect("Tried deref-ing an invalid weak handle")
    }

    pub fn version(&self) -> usize {
        self.version
    }
}

impl<T> PartialEq for WeakHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        // `WeakHandle`s use case is for sharing expensive-to-copy fields with
        // children components. Typically, `eq` would be used as part of testing
        // whether the `Properties` of a component have changed.
        //
        // Hence, `PartialEq` tests for referential equality -- i.e. it's the
        // same Rc<T> and the same version. Otherwise, they're deemed not equal
        // even if `<T as PartialEq>::eq` is true.
        std::ptr::eq(self.value.as_ptr(), other.value.as_ptr()) && self.version == other.version
    }
}
