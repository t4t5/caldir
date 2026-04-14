//! Calendar group names (`caldir_core::calendar::group::Group`).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{CalDirError, CalDirResult};

/// A calendar group name. Guaranteed non-empty and trimmed of surrounding
/// whitespace — construct via [`Group::parse`] or [`str::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Group(String);

impl Group {
    /// Parse a group name, trimming surrounding whitespace. Errors if the
    /// trimmed result is empty.
    pub fn parse(name: &str) -> CalDirResult<Self> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(CalDirError::Config(
                "Group name cannot be empty".to_string(),
            ));
        }
        Ok(Group(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Group {
    type Err = CalDirError;

    fn from_str(s: &str) -> CalDirResult<Self> {
        Self::parse(s)
    }
}

impl Serialize for Group {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Group {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}
