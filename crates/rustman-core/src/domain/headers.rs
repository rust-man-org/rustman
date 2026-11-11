/// HTTP headers in order, duplicates kept: two rows with one name are two
/// headers on the wire. Names compare case-insensitively, as HTTP's do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    /// The first value for `name`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.position(name).map(|i| self.0[i].1.as_str())
    }

    /// Every value for `name`, in order.
    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.iter()
            .filter(move |(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }

    /// Whether any header is called `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.position(name).is_some()
    }

    /// Adds a header, keeping any existing ones with the same name.
    pub fn push(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0.push((name.into(), value.into()));
    }

    /// Sets `name` to a single value, where the first existing one was.
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let at = self.position(&name).unwrap_or(self.0.len());
        self.0.retain(|(k, _)| !k.eq_ignore_ascii_case(&name));
        self.0.insert(at.min(self.0.len()), (name, value.into()));
    }

    /// Removes the first header called `name`, returning its value.
    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.position(name).map(|i| self.0.remove(i).1)
    }

    /// The number of headers, duplicates counted.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no headers.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The headers as `(name, value)`, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.0
            .iter()
            .position(|(k, _)| k.eq_ignore_ascii_case(name))
    }
}

impl IntoIterator for Headers {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<(String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for Headers {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_case_insensitively_and_duplicates_are_kept() {
        let mut h: Headers = [("Set-Cookie", "a=1"), ("set-cookie", "b=2")]
            .into_iter()
            .collect();
        assert_eq!(h.get("SET-COOKIE"), Some("a=1"));
        assert_eq!(h.get_all("Set-Cookie").collect::<Vec<_>>(), ["a=1", "b=2"]);
        assert_eq!(h.len(), 2);
        assert_eq!(h.remove("set-cookie").as_deref(), Some("a=1"));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn insert_replaces_every_value_in_place_of_the_first() {
        let mut h: Headers = [("A", "1"), ("X", "1"), ("B", "2"), ("x", "2")]
            .into_iter()
            .collect();
        h.insert("X", "new");
        assert_eq!(
            h.iter().collect::<Vec<_>>(),
            [("A", "1"), ("X", "new"), ("B", "2")]
        );
        h.insert("C", "3");
        assert_eq!(h.iter().last(), Some(("C", "3")));
    }
}
