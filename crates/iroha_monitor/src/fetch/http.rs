//! Reusable HTTP polling with bounded bodies and one deadline through body completion.

use std::{fmt, time::Duration};

use reqwest::{Client, ClientBuilder, StatusCode, redirect};
use tokio::time::Instant;

#[derive(Clone)]
pub(super) struct PollingClient {
    client: Client,
}

#[derive(Debug)]
pub(super) struct PollResponse {
    pub(super) body: Vec<u8>,
    pub(super) truncated: bool,
    /// Time until response headers, matching the monitor's existing latency metric.
    pub(super) latency: Duration,
}

#[derive(Debug)]
pub(super) struct PollError {
    kind: PollErrorKind,
    source: Option<reqwest::Error>,
}

#[derive(Debug)]
enum PollErrorKind {
    Transport,
    Status(StatusCode),
    Deadline,
}

impl PollError {
    fn transport(source: reqwest::Error) -> Self {
        Self {
            kind: PollErrorKind::Transport,
            source: Some(source),
        }
    }

    fn status(status: StatusCode) -> Self {
        Self {
            kind: PollErrorKind::Status(status),
            source: None,
        }
    }

    fn deadline() -> Self {
        Self {
            kind: PollErrorKind::Deadline,
            source: None,
        }
    }
}

impl fmt::Display for PollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(error) = &self.source {
            return error.fmt(f);
        }
        match self.kind {
            PollErrorKind::Transport => f.write_str("HTTP transport failed"),
            PollErrorKind::Status(status) => write!(f, "server returned {status}"),
            PollErrorKind::Deadline => f.write_str("request deadline elapsed"),
        }
    }
}

impl PollingClient {
    pub(super) fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Self::builder().build()?,
        })
    }

    fn builder() -> ClientBuilder {
        Client::builder()
            .redirect(redirect::Policy::limited(5))
            .referer(false)
            .http1_only()
            .http1_max_headers(100)
            .pool_max_idle_per_host(2)
            // Keep the raw-body contract even when another workspace consumer
            // enables compression features on reqwest.
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
    }

    pub(super) async fn get(
        &self,
        url: &str,
        limit: usize,
        timeout: Duration,
    ) -> Result<PollResponse, PollError> {
        tokio::time::timeout(timeout, self.get_inner(url, limit))
            .await
            .map_err(|_| PollError::deadline())?
    }

    async fn get_inner(&self, url: &str, limit: usize) -> Result<PollResponse, PollError> {
        let start = Instant::now();
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(PollError::transport)?;
        let latency = start.elapsed();
        if !response.status().is_success() {
            return Err(PollError::status(response.status()));
        }
        let declared_length = response.content_length();
        if declared_length.is_some_and(|length| length > limit as u64) {
            return Ok(PollResponse {
                body: Vec::new(),
                truncated: true,
                latency,
            });
        }
        let capacity = declared_length.map_or(0, |length| length as usize);
        let mut body = Vec::with_capacity(capacity);
        while let Some(chunk) = response.chunk().await.map_err(PollError::transport)? {
            if chunk.len() > limit - body.len() {
                return Ok(PollResponse {
                    body,
                    truncated: true,
                    latency,
                });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(PollResponse {
            body,
            truncated: false,
            latency,
        })
    }
}

#[cfg(test)]
#[path = "http/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "http/benchmark.rs"]
mod benchmark;
