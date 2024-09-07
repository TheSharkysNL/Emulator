use std::str::pattern::{Pattern, Searcher, SearchStep};

pub struct IgnoreCaseSearcher<'a, 'b> {
    value: &'a str,
    haystack: &'b str,
    position: usize,
}

pub struct IgnoreCase<'a> {
    value: &'a str,
}

impl<'a> IgnoreCase<'a> {
    pub fn new(value: &'a str) -> Self {
        Self { 
            value 
        }
    }
}

impl<'b> Pattern for IgnoreCase<'b> {
    type Searcher<'a> = IgnoreCaseSearcher<'b, 'a>;

    fn into_searcher(self, haystack: &str) -> Self::Searcher<'_> {
        IgnoreCaseSearcher {
            value: self.value,
            haystack,
            position: 0,
        }
    }
}

unsafe impl<'a, 'b> Searcher<'b> for IgnoreCaseSearcher<'a, 'b> {
    fn haystack(&self) -> &'b str {
        self.haystack
    }

    fn next(&mut self) -> SearchStep {
        let end = self.position + self.value.len();
        if end >= self.haystack.len() {
            SearchStep::Done
        } else {
            let total_found = self.haystack.as_bytes()[self.position..].iter()
                .enumerate()
                .take_while(|(index, b)| {
                    *index < self.value.len() &&
                        b.eq_ignore_ascii_case(&self.value.as_bytes()[*index])
                }).count();

            if total_found == self.value.len() {
                SearchStep::Match(self.position, self.position + total_found)
            } else {
                SearchStep::Reject(self.position, self.position + total_found)
            }
        }
    }
}