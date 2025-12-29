//! Types for representing securely printable secrets.
use std::fmt;

use derive_more::Constructor;
use norito::json::{self, JsonDeserialize, JsonSerialize};

/// String sensitive to printing and serialization
#[derive(Clone, Constructor)]
pub struct SecretString(String);

impl SecretString {
    /// Returns underlying secret string
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

const REDACTED: &str = "[REDACTED]";

impl JsonSerialize for SecretString {
    fn json_serialize(&self, out: &mut String) {
        REDACTED.json_serialize(out);
    }
}

impl JsonDeserialize for SecretString {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        Ok(Self(value))
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        REDACTED.fmt(f)
    }
}
