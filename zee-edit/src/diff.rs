use ropey::Rope;

#[derive(Clone, Debug, PartialEq)]
pub struct OpaqueDiff {
    pub byte_index: usize,
    pub old_byte_length: usize,
    pub new_byte_length: usize,
    pub char_index: usize,
    pub old_char_length: usize,
    pub new_char_length: usize,
}

impl OpaqueDiff {
    #[inline]
    pub fn new(
        byte_index: usize,
        old_byte_length: usize,
        new_byte_length: usize,
        char_index: usize,
        old_char_length: usize,
        new_char_length: usize,
    ) -> Self {
        Self {
            byte_index,
            old_byte_length,
            new_byte_length,
            char_index,
            old_char_length,
            new_char_length,
        }
    }

    #[inline]
    pub fn empty() -> Self {
        Self {
            byte_index: 0,
            old_byte_length: 0,
            new_byte_length: 0,
            char_index: 0,
            old_char_length: 0,
            new_char_length: 0,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        *self == OpaqueDiff::empty()
    }

    #[inline]
    pub fn reverse(&self) -> Self {
        Self {
            byte_index: self.byte_index,
            old_byte_length: self.new_byte_length,
            new_byte_length: self.old_byte_length,
            char_index: self.char_index,
            old_char_length: self.new_char_length,
            new_char_length: self.old_char_length,
        }
    }
}

pub struct DeleteOperation {
    pub diff: OpaqueDiff,
    pub deleted: Rope,
}

impl DeleteOperation {
    pub fn empty() -> Self {
        Self {
            diff: OpaqueDiff::empty(),
            deleted: Rope::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn mem_size_of_diffs() {
        assert_eq!(std::mem::size_of::<OpaqueDiff>(), 48);
        assert_eq!(std::mem::size_of::<Rope>(), 8);
        assert_eq!(std::mem::size_of::<DeleteOperation>(), 56);
    }
}
