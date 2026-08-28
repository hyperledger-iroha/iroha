//! Secret-bearing in-memory value wrappers shared by Mochi front ends.

use std::{
    fmt,
    ops::{Deref, DerefMut},
};
use zeroize::Zeroizing;

/// A UTF-8 secret that erases its allocation before release and redacts debug output.
pub struct SecretString(Zeroizing<String>);

impl SecretString {
    /// Take ownership of a secret string.
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    /// Borrow the secret for the shortest practical operation.
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for SecretString {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl Deref for SecretString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SecretString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_and_mutation_uses_the_guarded_allocation() {
        let mut secret = SecretString::new("do-not-log".to_owned());
        assert_eq!(secret.expose(), "do-not-log");
        assert!(!format!("{secret:?}").contains("do-not-log"));
        secret.clear();
        assert!(secret.is_empty());
    }
}
