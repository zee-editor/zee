use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use std::{fmt, iter};

pub struct Matcher {
    ranked: Vec<(usize, i64)>,   // (original index, score)
    matcher: Box<SkimMatcherV2>, // Boxed as it's big
}

impl fmt::Debug for Matcher {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("Matcher")
            .field("ranked", &self.ranked)
            .field("matcher", &"SkimMatcherV2(...)")
            .finish()
    }
}

impl Clone for Matcher {
    fn clone(&self) -> Self {
        Self {
            ranked: self.ranked.clone(),
            matcher: default_matcher().into(),
        }
    }
}

impl Matcher {
    pub fn new() -> Self {
        Self {
            ranked: Vec::new(),
            matcher: default_matcher().into(),
        }
    }

    pub fn num_ranked(&self) -> usize {
        self.ranked.len()
    }

    pub fn set_filter<'a>(&mut self, entries: impl Iterator<Item = &'a str>, filter: &str) {
        let filter = filter.trim();
        let Self {
            ref mut ranked,
            ref mut matcher,
        } = *self;
        ranked.clear();
        ranked.extend(entries.enumerate().filter_map(|(index, file)| {
            matcher
                .fuzzy_match(&file, filter)
                .map(|score| (index, score))
        }));
        ranked.sort_unstable_by_key(|(_, score)| -score);
    }

    pub fn clear(&mut self) {
        self.set_filter(iter::empty(), "")
    }
}

impl std::ops::Index<usize> for Matcher {
    type Output = usize;

    fn index(&self, rank: usize) -> &Self::Output {
        &self.ranked[rank].0
    }
}

fn default_matcher() -> SkimMatcherV2 {
    SkimMatcherV2::default()
    // .use_cache(false)
}
