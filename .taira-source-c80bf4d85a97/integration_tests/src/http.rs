//! Bounded HTTP helpers for integration tests.

use std::time::Duration;

use crate::timeouts::read_env_duration;

/// Environment override for default integration-test HTTP request timeout.
pub const HTTP_REQUEST_TIMEOUT_ENV: &str = "IROHA_TEST_HTTP_REQUEST_TIMEOUT_MS";

const HTTP_REQUEST_TIMEOUT_DEFAULT: Duration = Duration::from_secs(30);

/// Default timeout applied to ad hoc reqwest clients in integration tests.
#[must_use]
pub fn request_timeout() -> Duration {
    read_env_duration(HTTP_REQUEST_TIMEOUT_ENV, HTTP_REQUEST_TIMEOUT_DEFAULT)
}

/// Build a reqwest client with the default integration-test timeout.
#[must_use]
pub fn client() -> reqwest::Client {
    client_with_timeout(request_timeout())
}

/// Build a reqwest client with an explicit timeout.
#[must_use]
pub fn client_with_timeout(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("build bounded integration-test HTTP client")
}
