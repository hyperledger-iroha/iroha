//! Shared timeout parsing helpers for integration tests.

use std::{env, time::Duration};

/// Read a duration from an environment variable.
///
/// Bare integer values are interpreted as seconds. Values with an `ms` suffix
/// are interpreted as milliseconds.
#[must_use]
pub fn read_env_duration(var: &str, default: Duration) -> Duration {
    if let Ok(raw) = env::var(var) {
        let trimmed = raw.trim();
        if let Some(ms) = trimmed.strip_suffix("ms")
            && let Ok(value) = ms.parse::<u64>()
        {
            return Duration::from_millis(value);
        }
        if let Ok(value) = trimmed.parse::<u64>() {
            return Duration::from_secs(value);
        }
    }
    default
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvRestore {
        key: &'static str,
        value: Option<String>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            set_env_var(key, value);
            Self {
                key,
                value: previous,
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                set_env_var(self.key, value);
            } else {
                remove_env_var(self.key);
            }
        }
    }

    #[allow(unsafe_code)]
    fn set_env_var(key: &str, value: &str) {
        // Safety: this test serializes env mutation with ENV_LOCK.
        unsafe { env::set_var(key, value) };
    }

    #[allow(unsafe_code)]
    fn remove_env_var(key: &str) {
        // Safety: this test serializes env mutation with ENV_LOCK.
        unsafe { env::remove_var(key) };
    }

    #[test]
    fn read_env_duration_accepts_seconds_and_milliseconds() {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock");
        let _restore = EnvRestore::set("IROHA_TEST_TIMEOUT_PARSE_CASE", "250ms");
        assert_eq!(
            read_env_duration("IROHA_TEST_TIMEOUT_PARSE_CASE", Duration::from_secs(1)),
            Duration::from_millis(250)
        );

        let _restore = EnvRestore::set("IROHA_TEST_TIMEOUT_PARSE_CASE", "7");
        assert_eq!(
            read_env_duration("IROHA_TEST_TIMEOUT_PARSE_CASE", Duration::from_secs(1)),
            Duration::from_secs(7)
        );
    }
}
