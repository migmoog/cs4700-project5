use std::collections::HashSet;

pub struct Crawler {
    visited_urls: HashSet<String>,
    cookie: String
}

impl Crawler {
    pub fn new(cookie: &str) -> Self {
        Self {
            visited_urls: HashSet::new(),
            cookie: cookie.to_string(),
        }
    }
}
