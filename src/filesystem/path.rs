use alloc::prelude::v1::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Path(str);
impl Path {
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &Self {
        unsafe { &*(s.as_ref() as *const str as *const Self) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf(self.0.to_owned())
    }

    pub fn normalized(&self) -> &Path {
        if self.is_root() {
            self
        } else {
            Self::new(self.0.trim_end_matches('/'))
        }
    }

    pub fn is_root(&self) -> bool {
        &self.0 == "/"
    }

    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    pub fn parent(&self) -> Option<&Self> {
        if self.is_root() {
            None
        } else {
            match self.normalized().0.rfind('/') {
                Some(i) if i == 0 => Some(Self::new("/")),
                Some(i) => Some(Self::new(&self.0[..i])),
                None => None,
            }
        }
    }

    /// Everything after the last slash
    pub fn file_name(&self) -> Option<&str> {
        if self.is_root() {
            None
        } else {
            match self.normalized().0.rfind('/') {
                Some(i) => Some(&self.0[(i + 1)..]),
                None => Some(&self.0),
            }
        }
    }

    /// Iterate over components of a path, discarding absoluteness
    pub fn components(&self) -> Components {
        Components {
            path: self.normalized(),
            index: if self.is_absolute() { 1 } else { 0 },
        }
    }

    /// Add a component to path
    pub fn add<S: AsRef<str> + ?Sized>(&self, other: &S) -> PathBuf {
        let mut result = self.normalized().0.to_owned();
        let mut it = Path::new(other).components();

        if self.is_root() {
            if let Some(first) = it.next() {
                result.push_str(first);
            }
        }

        for c in it {
            result.push('/');
            result.push_str(c);
        }

        PathBuf(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathBuf(String);

impl PathBuf {
    pub fn new() -> Self {
        Self(String::new())
    }

    pub fn push<S: AsRef<str> + ?Sized>(&mut self, other: &S) {
        self.0 = self.add(other).0.to_owned();
    }
}

impl ::core::ops::Deref for PathBuf {
    type Target = Path;

    fn deref(&self) -> &Path {
        &Path::new(self.0.as_str())
    }
}

pub struct Components<'a> {
    path: &'a Path,
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
