//! Canonical SoraNet transport, anonymity, and rollout policies.

/// Transport policy applied when selecting providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportPolicy {
    /// Prefer relays that advertise SoraNet support while keeping direct transports as a fallback.
    /// Multi-source adopters now use this policy by default.
    #[default]
    SoranetPreferred,
    /// Require SoraNet transport and fail instead of falling back to direct providers.
    SoranetStrict,
    /// Enforce direct mode by restricting selection to providers that expose Torii/QUIC transports.
    /// Use this explicit downgrade when relays are unhealthy or compliance mandates direct fetches.
    DirectOnly,
}
impl TransportPolicy {
    /// Return the canonical policy label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SoranetPreferred => "soranet-first",
            Self::SoranetStrict => "soranet-strict",
            Self::DirectOnly => "direct-only",
        }
    }
    /// Parse a [`TransportPolicy`] from its exact canonical V1 label.
    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "soranet-first" => Some(Self::SoranetPreferred),
            "soranet-strict" => Some(Self::SoranetStrict),
            "direct-only" => Some(Self::DirectOnly),
            _ => None,
        }
    }
}
/// Staged anonymity policy enforced while selecting SoraNet-capable providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::enum_variant_names)]
pub enum AnonymityPolicy {
    /// Stage A (default): require that at least one SoraNet hop (guard) advertises PQ capability.
    #[default]
    GuardPq,
    /// Stage B: prefer PQ-capable relays for a majority of SoraNet hops (≥ two thirds).
    MajorityPq,
    /// Stage C: enforce PQ-only SoraNet paths, falling back to direct transports otherwise.
    StrictPq,
}
impl AnonymityPolicy {
    /// Return the canonical policy label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GuardPq => "anon-guard-pq",
            Self::MajorityPq => "anon-majority-pq",
            Self::StrictPq => "anon-strict-pq",
        }
    }
    /// Parse an [`AnonymityPolicy`] from its exact canonical V1 label.
    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "anon-guard-pq" => Some(Self::GuardPq),
            "anon-majority-pq" => Some(Self::MajorityPq),
            "anon-strict-pq" => Some(Self::StrictPq),
            _ => None,
        }
    }
    /// Returns the next less strict policy, if any.
    #[must_use]
    pub const fn fallback(self) -> Option<Self> {
        match self {
            Self::StrictPq => Some(Self::MajorityPq),
            Self::MajorityPq => Some(Self::GuardPq),
            Self::GuardPq => None,
        }
    }
}
/// Rollout phase controlling the default anonymity stage applied to SoraNet paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RolloutPhase {
    /// Canary wave — require at least one PQ-capable guard (Stage A).
    #[default]
    Canary,
    /// Ramp wave — prefer PQ-capable relays for ≥ two thirds of hops (Stage B).
    Ramp,
    /// Default GA posture — enforce PQ-only SoraNet paths (Stage C).
    Default,
}
impl RolloutPhase {
    /// Stable string label used in config/CLI bindings.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Canary => "canary",
            Self::Ramp => "ramp",
            Self::Default => "default",
        }
    }
    /// Parse a rollout phase from its exact canonical V1 label.
    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "canary" => Some(Self::Canary),
            "ramp" => Some(Self::Ramp),
            "default" => Some(Self::Default),
            _ => None,
        }
    }
    /// Map the rollout phase to the default anonymity policy.
    #[must_use]
    pub const fn default_anonymity_policy(self) -> AnonymityPolicy {
        match self {
            Self::Canary => AnonymityPolicy::GuardPq,
            Self::Ramp => AnonymityPolicy::MajorityPq,
            Self::Default => AnonymityPolicy::StrictPq,
        }
    }
}
/// Write-mode hint forwarded by SDKs to tighten PQ expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteModeHint {
    /// Default behaviour for read/replication workloads.
    #[default]
    ReadOnly,
    /// Upload workloads that require PQ-only paths end-to-end.
    UploadPqOnly,
}
impl WriteModeHint {
    /// Stable canonical V1 label used in JSON, logs, and metrics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::UploadPqOnly => "upload-pq-only",
        }
    }
    /// Parse a [`WriteModeHint`] from its exact canonical V1 label.
    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "read-only" => Some(Self::ReadOnly),
            "upload-pq-only" => Some(Self::UploadPqOnly),
            _ => None,
        }
    }
    /// Returns `true` when the hint mandates PQ-only transport.
    #[must_use]
    pub const fn enforces_pq_only(self) -> bool {
        matches!(self, Self::UploadPqOnly)
    }
    /// Apply the hint to derive effective transport/anonymity policies.
    #[must_use]
    pub const fn apply(
        self,
        transport_policy: TransportPolicy,
        anonymity_policy: AnonymityPolicy,
    ) -> (TransportPolicy, AnonymityPolicy) {
        match self {
            Self::ReadOnly => (transport_policy, anonymity_policy),
            Self::UploadPqOnly => (TransportPolicy::SoranetStrict, AnonymityPolicy::StrictPq),
        }
    }
}

impl std::fmt::Display for TransportPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}
impl std::fmt::Display for AnonymityPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_labels_roundtrip_and_display() {
        for policy in [
            TransportPolicy::SoranetPreferred,
            TransportPolicy::SoranetStrict,
            TransportPolicy::DirectOnly,
        ] {
            assert_eq!(TransportPolicy::parse(policy.label()), Some(policy));
            assert_eq!(policy.to_string(), policy.label());
        }
        for policy in [
            AnonymityPolicy::GuardPq,
            AnonymityPolicy::MajorityPq,
            AnonymityPolicy::StrictPq,
        ] {
            assert_eq!(AnonymityPolicy::parse(policy.label()), Some(policy));
            assert_eq!(policy.to_string(), policy.label());
        }
        for phase in [
            RolloutPhase::Canary,
            RolloutPhase::Ramp,
            RolloutPhase::Default,
        ] {
            assert_eq!(RolloutPhase::parse(phase.label()), Some(phase));
        }
        for mode in [WriteModeHint::ReadOnly, WriteModeHint::UploadPqOnly] {
            assert_eq!(WriteModeHint::parse(mode.label()), Some(mode));
        }
    }

    #[test]
    fn policy_labels_reject_aliases_whitespace_and_case_changes() {
        for rejected in [
            "",
            "canary ",
            " Canary",
            "CANARY",
            "stage-a",
            "stage_a",
            "soranet",
            "soranet_first",
            "guard-pq",
            "guard_pq",
            "strict",
            "upload",
            "read_only",
        ] {
            assert_eq!(TransportPolicy::parse(rejected), None);
            assert_eq!(AnonymityPolicy::parse(rejected), None);
            assert_eq!(RolloutPhase::parse(rejected), None);
            assert_eq!(WriteModeHint::parse(rejected), None);
        }
    }

    #[test]
    fn rollout_defaults_and_explicit_fallback_order_are_fixed() {
        assert_eq!(
            TransportPolicy::default(),
            TransportPolicy::SoranetPreferred
        );
        assert_eq!(AnonymityPolicy::default(), AnonymityPolicy::GuardPq);
        assert_eq!(RolloutPhase::default(), RolloutPhase::Canary);
        assert_eq!(WriteModeHint::default(), WriteModeHint::ReadOnly);
        for (phase, policy) in [
            (RolloutPhase::Canary, AnonymityPolicy::GuardPq),
            (RolloutPhase::Ramp, AnonymityPolicy::MajorityPq),
            (RolloutPhase::Default, AnonymityPolicy::StrictPq),
        ] {
            assert_eq!(phase.default_anonymity_policy(), policy);
        }
        assert_eq!(
            AnonymityPolicy::StrictPq.fallback(),
            Some(AnonymityPolicy::MajorityPq)
        );
        assert_eq!(
            AnonymityPolicy::MajorityPq.fallback(),
            Some(AnonymityPolicy::GuardPq)
        );
        assert_eq!(AnonymityPolicy::GuardPq.fallback(), None);
    }

    #[test]
    fn upload_policy_requires_pq_for_every_requested_transport() {
        assert!(!WriteModeHint::ReadOnly.enforces_pq_only());
        assert!(WriteModeHint::UploadPqOnly.enforces_pq_only());
        for transport in [
            TransportPolicy::SoranetPreferred,
            TransportPolicy::SoranetStrict,
            TransportPolicy::DirectOnly,
        ] {
            for anonymity in [
                AnonymityPolicy::GuardPq,
                AnonymityPolicy::MajorityPq,
                AnonymityPolicy::StrictPq,
            ] {
                assert_eq!(
                    WriteModeHint::ReadOnly.apply(transport, anonymity),
                    (transport, anonymity)
                );
                assert_eq!(
                    WriteModeHint::UploadPqOnly.apply(transport, anonymity),
                    (TransportPolicy::SoranetStrict, AnonymityPolicy::StrictPq)
                );
            }
        }
    }
}
