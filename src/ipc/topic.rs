use crate::alloc::borrow::ToOwned;

use super::*;

/// While reliable and unreliable messages cannot be sent to each
/// others endpoints, topic names still use the same namespace
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Topic(String);
impl Topic {
    /// Checks that topic name is valid and if yes, returns a new Topic.
    /// These requirements may be reduced later.
    /// Currently all characters in `a-zA-Z0-9_/`, no leading or trailing `/`s
    pub fn new(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }

        if s.starts_with('/') || s.ends_with('/') {
            return None;
        }

        for c in s.chars() {
            if !(c.is_ascii_alphanumeric() || c == '_' || c == '/') {
                return None;
            }
        }

        Some(Self(s.to_owned()))
    }

    pub fn try_new(s: &str) -> Result<Self, result::Error> {
        Self::new(s).ok_or(result::Error::InvalidTopic)
    }

    pub fn string(&self) -> String {
        self.0.clone()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopicPrefix(String);
impl TopicPrefix {
    /// Mirrors `Topic::new`, but allows all prefixes`
    pub fn new(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }

        if s.starts_with('/') {
            return None;
        }

        for c in s.chars() {
            if !(c.is_ascii_alphanumeric() || c == '_' || c == '/') {
                return None;
            }
        }
        Some(Self(s.to_owned()))
    }

    pub fn try_new(s: &str) -> Result<Self, result::Error> {
        Self::new(s).ok_or(result::Error::InvalidTopic)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TopicFilter {
    /// Exact match required
    Exact(Topic),
    /// Must be a prefix of the topic name
    Prefix(TopicPrefix),
}
impl TopicFilter {
    pub fn try_new(filter: &str, exact: bool) -> Result<Self, result::Error> {
        Ok(if exact {
            Self::Exact(Topic::try_new(filter)?)
        } else {
            Self::Prefix(TopicPrefix::try_new(filter)?)
        })
    }

    fn inner(&self) -> &str {
        match self {
            Self::Exact(t) => t.0.as_str(),
            Self::Prefix(t) => t.0.as_str(),
        }
    }

    /// Does other filter match subset of this?
    pub(super) fn contains_filter(&self, other: &Self) -> bool {
        match self {
            Self::Exact(a) => match other {
                Self::Exact(b) => a.0 == b.0,
                Self::Prefix(b) => a.0.starts_with(&b.0),
            },
            Self::Prefix(a) => match other {
                Self::Exact(b) => b.0.starts_with(&a.0),
                Self::Prefix(b) => a.0.starts_with(&b.0) || b.0.starts_with(&a.0),
            },
        }
    }

    /// Does this match a topic
    pub(super) fn matches(&self, other: &Topic) -> bool {
        match self {
            Self::Exact(a) => a.0 == other.0,
            Self::Prefix(a) => other.0.starts_with(&a.0),
        }
    }
}
