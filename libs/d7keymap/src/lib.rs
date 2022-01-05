// Features
#![feature(allocator_api)]
// No-std
#![cfg_attr(not(test), no_std)]

extern crate alloc;

use core::hash::{Hash, Hasher};
use core::str::FromStr;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::{HashMap, HashSet};

use serde::{de, Deserialize, Deserializer};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct KeySymbol(String);
impl KeySymbol {
    pub fn new(s: &str) -> Self {
        Self(s.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub type KeyCode = u16;
pub type KeyCodes = HashMap<KeyCode, KeySymbol>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combination {
    pub modifiers: HashSet<KeySymbol>,
    pub main: KeySymbol,
}
impl Combination {
    pub fn matches(&self, current: &KeySymbol, pressed: &HashSet<KeySymbol>) -> bool {
        &self.modifiers == pressed && &self.main == current
    }
}
impl FromStr for Combination {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut modifiers: Vec<_> = s.split("+").map(|m| KeySymbol(m.to_owned())).collect();
        let main = modifiers.pop().unwrap();
        Ok(Self {
            modifiers: modifiers.into_iter().collect(),
            main,
        })
    }
}
impl Hash for Combination {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.main.hash(state);
    }
}
impl<'de> Deserialize<'de> for Combination {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct KeyMap {
    /// Only these keys can be used as modifiers
    pub modifiers: HashSet<KeySymbol>,
    /// Mapping from key combinations to actions
    pub mapping: HashMap<Combination, KeyAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyAction {
    /// Product text (prefix from dead-key buffer if any and normalize)
    Text(String),
    /// Insert to dead-key buffer
    Buffer(String),
    /// Remap to another key symbol
    Remap(KeySymbol),
    /// Ignore this keypress
    Ignore,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

    #[test]
    fn test_keycodes() {
        let s = fs::read("examples/keycodes.json").unwrap();
        let data: KeyCodes = serde_json::from_slice(&s).unwrap();
        assert_eq!(data[&17], KeySymbol("LeftAlt".to_owned()));
    }

    #[test]
    fn test_keycombinations() {
        let s = fs::read("examples/keymap.json").unwrap();
        let _data: KeyMap = serde_json::from_slice(&s).unwrap();
    }
}
