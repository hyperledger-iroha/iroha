//! Admission and abuse controls for SoraFS storage pin submissions.

use std::{
    collections::HashSet,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use http::HeaderMap;

use crate::limits;

/// Policy applied to `/v1/sorafs/storage/pin` submissions.
#[derive(Clone, Debug)]
pub struct PinSubmissionPolicy {
    require_token: bool,
    tokens: Arc<HashSet<String>>,
    allow_nets: Vec<limits::IpNet>,
    rate_limiter: Option<GatewayRateLimiter>,
}

impl PinSubmissionPolicy {
    /// Build a policy from configuration.
    ///
    /// # Errors
    ///
    /// Returns [`PinPolicyError`] when configuration is invalid (e.g., missing tokens when
    /// `require_token` is enabled, or malformed CIDR entries).
    pub fn from_config(
        cfg: &iroha_config::parameters::actual::SorafsStoragePin,
    ) -> Result<Self, PinPolicyError> {
        if cfg.require_token && cfg.tokens.is_empty() {
            return Err(PinPolicyError::MissingTokens);
        }
        let mut allow_nets = Vec::with_capacity(cfg.allow_cidrs.len());
        for cidr in &cfg.allow_cidrs {
            match limits::parse_cidr(cidr) {
                Some(net) => allow_nets.push(net),
                None => return Err(PinPolicyError::InvalidCidr(cidr.clone())),
            }
        }
        let rate_limiter = GatewayRateLimiter::from_config(&cfg.rate_limit);
        Ok(Self {
            require_token: cfg.require_token,
            tokens: Arc::new(cfg.tokens.iter().cloned().collect()),
            allow_nets,
            rate_limiter,
        })
    }

    /// Enforce the policy against the incoming request.
    ///
    /// # Errors
    /// Returns an error when the caller is outside the allowed CIDR set or provides invalid credentials.
    pub fn enforce(&self, headers: &HeaderMap, remote: Option<IpAddr>) -> Result<(), PinAuthError> {
        if !self.allow_nets.is_empty()
            && remote.map_or(true, |ip| !limits::cidr_contains(&self.allow_nets, ip))
        {
            return Err(PinAuthError::IpNotAllowed);
        }

        let token = extract_token(headers);
        if self.require_token {
            let Some(token_value) = token.as_ref() else {
                return Err(PinAuthError::TokenRequired);
            };
            if !self.tokens.contains(token_value) {
                return Err(PinAuthError::InvalidToken);
            }
        } else if let Some(token_value) = token.as_ref() {
            if !self.tokens.is_empty() && !self.tokens.contains(token_value) {
                return Err(PinAuthError::InvalidToken);
            }
        }

        if let Some(limiter) = &self.rate_limiter {
            let key = rate_key(token.as_deref(), remote);
            if let Some(retry_after) = limiter.check(&key) {
                return Err(PinAuthError::RateLimited { retry_after });
            }
        }

        Ok(())
    }
}

/// Errors returned when enforcing the pin submission policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinAuthError {
    /// Token was required but missing.
    TokenRequired,
    /// Token was provided but not recognised.
    InvalidToken,
    /// Client IP not present in the allow-list.
    IpNotAllowed,
    /// Request exceeded the configured rate limit.
    RateLimited {
        /// Duration to wait before retrying.
        retry_after: Duration,
    },
}

/// Errors raised while parsing the policy configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinPolicyError {
    /// Configuration requires tokens but none were provided.
    MissingTokens,
    /// Invalid CIDR entry encountered.
    InvalidCidr(String),
}

impl std::fmt::Display for PinPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTokens => write!(
                f,
                "sorafs.storage.pin.require_token=true requires at least one token"
            ),
            Self::InvalidCidr(cidr) => {
                write!(f, "invalid sorafs.storage.pin.allow_cidrs entry: {cidr}")
            }
        }
    }
}

impl std::error::Error for PinPolicyError {}

/// Token-bucket style rate limiter with optional temporary bans.
#[derive(Debug, Clone)]
struct GatewayRateLimiter {
    max_requests: u32,
    window: Duration,
    ban: Option<Duration>,
    buckets: Arc<DashMap<String, RateEntry>>,
}

#[derive(Debug, Clone)]
struct RateEntry {
    window_start: Instant,
    count: u32,
    banned_until: Option<Instant>,
}

impl GatewayRateLimiter {
    fn from_config(cfg: &iroha_config::parameters::actual::SorafsGatewayRateLimit) -> Option<Self> {
        let max_requests = cfg.max_requests?;
        Some(Self {
            max_requests: max_requests.get(),
            window: cfg.window,
            ban: cfg.ban,
            buckets: Arc::new(DashMap::new()),
        })
    }

    /// Returns `None` when allowed, or `Some(duration)` when throttled.
    fn check(&self, key: &str) -> Option<Duration> {
        let now = Instant::now();
        let mut entry = self
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| RateEntry {
                window_start: now,
                count: 0,
                banned_until: None,
            });

        if let Some(until) = entry.banned_until {
            if until > now {
                return Some(until.saturating_duration_since(now));
            }
            entry.banned_until = None;
            entry.count = 0;
            entry.window_start = now;
        }

        if now.saturating_duration_since(entry.window_start) >= self.window {
            entry.window_start = now;
            entry.count = 0;
            entry.banned_until = None;
        }

        if entry.count >= self.max_requests {
            let retry_after = if let Some(ban) = self.ban {
                let until = now.checked_add(ban).unwrap_or_else(|| now + ban);
                entry.banned_until = Some(until);
                ban
            } else {
                self.window
                    .saturating_sub(now.saturating_duration_since(entry.window_start))
            };
            return Some(retry_after);
        }

        entry.count = entry.count.saturating_add(1);
        None
    }
}

fn rate_key(token: Option<&str>, remote: Option<IpAddr>) -> String {
    match (token, remote) {
        (Some(tok), Some(ip)) => format!("token:{tok}|ip:{ip}"),
        (Some(tok), None) => format!("token:{tok}"),
        (None, Some(ip)) => format!("ip:{ip}"),
        (None, None) => "unknown".to_owned(),
    }
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(rest) = value.strip_prefix("Bearer ") {
            let trimmed = rest.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    headers
        .get("x-sorafs-pin-token")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::rate_key;

    #[test]
    fn rate_key_scopes_shared_tokens_by_ip() {
        let key_a = rate_key(
            Some("shared-token"),
            Some("203.0.113.10".parse().expect("valid ip")),
        );
        let key_b = rate_key(
            Some("shared-token"),
            Some("203.0.113.11".parse().expect("valid ip")),
        );

        assert_ne!(key_a, key_b);
        assert_eq!(key_a, "token:shared-token|ip:203.0.113.10");
        assert_eq!(key_b, "token:shared-token|ip:203.0.113.11");
    }

    #[test]
    fn rate_key_preserves_single_dimension_fallbacks() {
        assert_eq!(rate_key(Some("token-only"), None), "token:token-only");
        assert_eq!(
            rate_key(None, Some("198.51.100.5".parse().expect("valid ip"))),
            "ip:198.51.100.5"
        );
        assert_eq!(rate_key(None, None), "unknown");
    }
}
