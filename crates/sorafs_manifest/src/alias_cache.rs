//! Shared alias cache policy helpers used by gateways and SDKs.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use norito::decode_from_bytes;
use thiserror::Error;

use crate::pin_registry::{
    AliasProofBundleV1, AliasProofBundleValidationError, AliasProofVerificationError,
    verify_alias_proof_bundle,
};

/// Alias cache policy describing TTL boundaries for alias proofs.
#[derive(Debug, Clone, Copy)]
pub struct AliasCachePolicy {
    positive_ttl: Duration,
    refresh_window: Duration,
    hard_expiry: Duration,
    negative_ttl: Duration,
    revocation_ttl: Duration,
    rotation_max_age: Duration,
    successor_grace: Duration,
    governance_grace: Duration,
}

impl AliasCachePolicy {
    /// Construct a new policy from the supplied durations.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        positive_ttl: Duration,
        refresh_window: Duration,
        hard_expiry: Duration,
        negative_ttl: Duration,
        revocation_ttl: Duration,
        rotation_max_age: Duration,
        successor_grace: Duration,
        governance_grace: Duration,
    ) -> Self {
        Self {
            positive_ttl,
            refresh_window,
            hard_expiry,
            negative_ttl,
            revocation_ttl,
            rotation_max_age,
            successor_grace,
            governance_grace,
        }
    }

    /// Returns the configured positive TTL.
    #[must_use]
    pub fn positive_ttl(&self) -> Duration {
        self.positive_ttl
    }

    /// Returns the configured refresh window.
    #[must_use]
    pub fn refresh_window(&self) -> Duration {
        self.refresh_window
    }

    /// Returns the configured hard expiry.
    #[must_use]
    pub fn hard_expiry(&self) -> Duration {
        self.hard_expiry
    }

    /// Returns the configured negative cache TTL.
    #[must_use]
    pub fn negative_ttl(&self) -> Duration {
        self.negative_ttl
    }

    /// Returns the configured revocation TTL.
    #[must_use]
    pub fn revocation_ttl(&self) -> Duration {
        self.revocation_ttl
    }

    /// Returns the configured rotation ceiling.
    #[must_use]
    pub fn rotation_max_age(&self) -> Duration {
        self.rotation_max_age
    }

    /// Returns the successor grace window applied after approved manifest rotations.
    #[must_use]
    pub fn successor_grace(&self) -> Duration {
        self.successor_grace
    }

    /// Returns the governance grace window applied to council-triggered rotations.
    #[must_use]
    pub fn governance_grace(&self) -> Duration {
        self.governance_grace
    }

    /// Evaluates an alias proof bundle against the cache policy.
    #[must_use]
    pub fn evaluate(&self, proof: &AliasProofBundleV1, now_secs: u64) -> AliasProofEvaluation {
        let generated = proof.generated_at_unix;
        let expires = proof.expires_at_unix;
        let age_secs = now_secs.saturating_sub(generated);

        let hard_expired = age_secs >= self.hard_expiry().as_secs() || now_secs >= expires;
        let rotation_due = age_secs >= self.rotation_max_age().as_secs();
        let refresh_threshold = self
            .positive_ttl()
            .as_secs()
            .saturating_sub(self.refresh_window().as_secs());

        let state = if hard_expired {
            AliasProofState::HardExpired
        } else if age_secs >= self.positive_ttl().as_secs() {
            AliasProofState::Expired
        } else if age_secs >= refresh_threshold {
            AliasProofState::RefreshWindow
        } else {
            AliasProofState::Fresh
        };

        let expires_in = if now_secs >= expires {
            None
        } else {
            Some(Duration::from_secs(expires - now_secs))
        };

        AliasProofEvaluation {
            state,
            rotation_due,
            age: Duration::from_secs(age_secs),
            generated_at_unix: generated,
            expires_at_unix: expires,
            expires_in,
        }
    }
}

/// Current evaluation state of an alias proof relative to the cache policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasProofState {
    /// Proof sits comfortably within the positive TTL.
    Fresh,
    /// Proof is still valid but should be refreshed synchronously.
    RefreshWindow,
    /// Proof exceeded the positive TTL and should not be served without refresh.
    Expired,
    /// Proof exceeded the hard-expiry or on-chain expiry and must be rejected.
    HardExpired,
}

impl AliasProofState {
    /// Returns true when the proof is still eligible to be served.
    #[must_use]
    pub fn is_servable(self) -> bool {
        matches!(self, Self::Fresh | Self::RefreshWindow)
    }
}

/// Alias proof evaluation output.
#[derive(Debug, Clone, Copy)]
pub struct AliasProofEvaluation {
    /// Current state relative to the cache policy.
    pub state: AliasProofState,
    /// Whether the proof age exceeds the rotation ceiling.
    pub rotation_due: bool,
    /// Proof age (`now - generated_at`).
    pub age: Duration,
    /// Original `generated_at_unix` value.
    pub generated_at_unix: u64,
    /// Original `expires_at_unix` value.
    pub expires_at_unix: u64,
    /// Remaining time before the proof expires (if any).
    pub expires_in: Option<Duration>,
}

impl AliasProofEvaluation {
    /// Returns the `Sora-Proof-Status` label associated with this evaluation.
    #[must_use]
    pub fn status_label(&self) -> &'static str {
        match (self.state, self.rotation_due) {
            (AliasProofState::Fresh, false) => "fresh",
            (AliasProofState::Fresh, true) => "fresh-rotate",
            (AliasProofState::RefreshWindow, false) => "refresh",
            (AliasProofState::RefreshWindow, true) => "refresh-rotate",
            (AliasProofState::Expired, _) => "expired",
            (AliasProofState::HardExpired, _) => "hard-expired",
        }
    }
}

/// Errors emitted while decoding or validating alias proofs.
#[derive(Debug, Error)]
pub enum AliasProofError {
    /// Failed to decode the alias proof bytes via Norito.
    #[error("decode alias proof: {0}")]
    Decode(#[from] norito::Error),
    /// Alias proof failed internal validation.
    #[error("invalid alias proof bundle: {0}")]
    Validation(#[from] AliasProofBundleValidationError),
    /// Alias proof failed cryptographic verification.
    #[error("alias proof verification failed: {0}")]
    Verification(#[from] AliasProofVerificationError),
}

/// Decodes and validates an alias proof bundle.
pub fn decode_alias_proof(bytes: &[u8]) -> Result<AliasProofBundleV1, AliasProofError> {
    let bundle: AliasProofBundleV1 = decode_from_bytes(bytes)?;
    verify_alias_proof_bundle(&bundle)?;
    Ok(bundle)
}

/// Returns the current UNIX timestamp (seconds).
#[must_use]
pub fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pin_registry::AliasBindingV1;

    fn sample_bundle(generated: u64, expires: u64) -> AliasProofBundleV1 {
        AliasProofBundleV1 {
            binding: AliasBindingV1 {
                alias: "docs/sora".into(),
                manifest_cid: vec![0x42, 0x24],
                bound_at: 1,
                expiry_epoch: 10,
            },
            registry_root: [7; 32],
            registry_height: 1,
            generated_at_unix: generated,
            expires_at_unix: expires,
            merkle_path: Vec::new(),
            council_signatures: Vec::new(),
        }
    }

    #[test]
    fn evaluates_fresh_refresh_and_expired_states() {
        let policy = AliasCachePolicy::new(
            Duration::from_mins(10),
            Duration::from_mins(2),
            Duration::from_mins(15),
            Duration::from_mins(1),
            Duration::from_mins(5),
            Duration::from_hours(6),
            Duration::from_mins(5),
            Duration::ZERO,
        );
        let now = 1_000_000;

        let fresh = sample_bundle(now - 120, now + 600);
        let eval_fresh = policy.evaluate(&fresh, now);
        assert_eq!(eval_fresh.state, AliasProofState::Fresh);
        assert!(!eval_fresh.rotation_due);

        let refresh = sample_bundle(now - 550, now + 600);
        let eval_refresh = policy.evaluate(&refresh, now);
        assert_eq!(eval_refresh.state, AliasProofState::RefreshWindow);

        let expired = sample_bundle(now - 720, now + 600);
        let eval_expired = policy.evaluate(&expired, now);
        assert_eq!(eval_expired.state, AliasProofState::Expired);

        let hard = sample_bundle(now - 1_200, now + 600);
        let eval_hard = policy.evaluate(&hard, now);
        assert_eq!(eval_hard.state, AliasProofState::HardExpired);

        let expiry = sample_bundle(now - 300, now - 1);
        let eval_expiry = policy.evaluate(&expiry, now);
        assert_eq!(eval_expiry.state, AliasProofState::HardExpired);
    }

    #[test]
    fn rotation_due_flag_triggers_when_configured() {
        let policy = AliasCachePolicy::new(
            Duration::from_mins(20),
            Duration::from_mins(1),
            Duration::from_hours(1),
            Duration::from_mins(1),
            Duration::from_mins(5),
            Duration::from_mins(5),
            Duration::from_mins(5),
            Duration::ZERO,
        );
        let now = 50_000;
        let bundle = sample_bundle(now - 350, now + 10_000);
        let eval = policy.evaluate(&bundle, now);
        assert_eq!(eval.state, AliasProofState::Fresh);
        assert!(eval.rotation_due);
    }

    #[test]
    fn exposes_grace_windows() {
        let successor = Duration::from_mins(7);
        let governance = Duration::from_secs(5);
        let policy = AliasCachePolicy::new(
            Duration::from_mins(10),
            Duration::from_mins(2),
            Duration::from_mins(15),
            Duration::from_mins(1),
            Duration::from_mins(5),
            Duration::from_hours(6),
            successor,
            governance,
        );
        assert_eq!(policy.successor_grace(), successor);
        assert_eq!(policy.governance_grace(), governance);
    }
}
