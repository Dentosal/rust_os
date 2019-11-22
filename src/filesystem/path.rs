use alloc::prelude::v1::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<'a>(&'a str);
impl<'a> Path<'a> {
    pub fn new(s: &'a str) -> Self {
        if s == "/" {
            Self("/")
        } else {
            Self(s.trim_end_matches('/'))
        }
    }

    pub fn as_str(&self) -> &str {
        self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf(self.0.to_owned())
    }

    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            None
        } else {
            match self.0.rfind('/') {
                Some(i) if i == 0 => Some(Self::new("/")),
                Some(i) => Some(Self(&self.0[..i])),
                None => None,
            }
        }
    }

    /// Everything after the last slash
    pub fn file_name(&self) -> Option<&str> {
        if self.is_root() {
            None
        } else {
            match self.0.rfind('/') {
                Some(i) => Some(&self.0[(i + 1)..]),
                None => Some(&self.0),
            }
        }
    }

    /// Iterate over components of a path, discarding absoluteness
    pub fn components(&self) -> Components {
        Components {
            path: self.clone(),
            index: if self.is_absolute() { 1 } else { 0 },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathBuf(String);

pub struct Components<'a> {
    path: Path<'a>,
    index: usize,
}
impl<'a> Iterator for Components<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.path.0.len() {
            return None;
        }

        let end_index = match self.path.0[self.index..].find('/') {
            Some(i) => self.index + i,
            None => self.path.0.len(),
        };

        assert!(end_index > self.index);

        let result = &self.path.0[self.index..end_index];
        self.index = end_index + 1;
        Some(result)
    }
}
