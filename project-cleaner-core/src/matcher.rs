use std::sync::Arc;

use glob::Pattern;

pub struct Matcher {
    pub name: Arc<str>,
    pub to_match: Pattern,
    pub to_remove: Pattern,
}

impl Matcher {
    pub fn new(name: Arc<str>, to_match: Pattern, to_remove: Pattern) -> Self {
        Self {
            name,
            to_match,
            to_remove,
        }
    }
}
