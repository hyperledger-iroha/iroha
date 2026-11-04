//! Types for representing securely printable secrets.
use norito::json::{self, JsonDeserialize, JsonSerialize};
use std::fmt;
/// String sensitive to printing and serialization
#[derive(Clone)]
pub struct SecretString(String);
impl SecretString {
    /// Wrap an owned string without copying it.
    #[inline]
    pub const fn new(value: String) -> Self {
        Self(value)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_is_const_callable_and_retains_owned_string_storage() {
        const fn wrap(value: String) -> SecretString {
            SecretString::new(value)
        }

        let mut value = String::with_capacity(128);
        value.push_str("constructor-secret-fixture");
        let pointer = value.as_ptr();
        let capacity = value.capacity();
        let secret = wrap(value);
        assert_eq!(secret.expose_secret().as_ptr(), pointer);
        assert_eq!(secret.0.capacity(), capacity);
        assert!(secret.expose_secret() == "constructor-secret-fixture");
    }

    #[test]
    fn constructed_secret_remains_redacted_in_debug_and_json() {
        let secret = SecretString::new("printing-secret-fixture".to_owned());
        assert_eq!(format!("{secret:?}"), "\"[REDACTED]\"");
        assert_eq!(format!("{secret:#?}"), "\"[REDACTED]\"");
        assert_eq!(
            json::to_json(&secret).expect("secret JSON"),
            "\"[REDACTED]\""
        );
    }
}
