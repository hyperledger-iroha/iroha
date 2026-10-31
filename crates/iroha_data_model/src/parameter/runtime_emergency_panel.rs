//! Committed runtime emergency panel of the universal root (`specs/live_runtime_upgrades.md`
//! §7.1, "Emergency containment"; custom parameter `runtime_emergency_panel_v1`).
//!
//! SORA Parliament delegates emergency containment to a standing panel of primary members and
//! disjoint backup members. Containment needs `threshold` fresh endorsements from either panel;
//! it closes admission from the next block and expires at the end of its lease unless ordinary
//! governance ratifies or replaces it. Parliament sets and rotates the panel through this
//! parameter; a root without it admits no containment, and missing or malformed committed state
//! never falls back to a default.

use iroha_crypto::PublicKey;
use iroha_primitives::json::Json;

use super::{CustomParameter, CustomParameterId};

/// Why a committed `runtime_emergency_panel_v1` parameter cannot be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeEmergencyPanelErrorV1 {
    /// The custom parameter has another identity, an oversized or malformed payload, or a
    /// payload that is not the canonical JSON of its value.
    #[error("runtime emergency panel parameter: {0}")]
    CustomParameter(&'static str),
    /// A panel is empty, unsorted or repeats a member, or the panels share a member.
    #[error("the primary and backup panels must be non-empty, sorted, unique and disjoint")]
    Members,
    /// The threshold is not at least two thirds of each panel, or exceeds one of them.
    #[error("the threshold must be at least two thirds of each panel and at most its size")]
    Threshold,
    /// The lease bounds are empty or inverted.
    #[error("the lease bounds must satisfy 0 < min_lease_ms <= max_lease_ms")]
    Lease,
}

/// The standing panel that may contain runtime definitions in an emergency.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(
    name = "iroha_data_model::parameter::runtime_emergency_panel::RuntimeEmergencyPanelV1"
)]
pub struct RuntimeEmergencyPanelV1 {
    /// Primary members, sorted by key.
    pub primary: Vec<PublicKey>,
    /// Backup members, sorted by key and disjoint from the primary members.
    pub backup: Vec<PublicKey>,
    /// Endorsements required from one panel.
    pub threshold: u32,
    /// Committed time after which the panel may no longer authorize containment.
    pub term_until_ms: u64,
    /// Shortest lease a containment may take, in committed milliseconds.
    pub min_lease_ms: u64,
    /// Longest lease a containment may take, in committed milliseconds.
    pub max_lease_ms: u64,
}

impl RuntimeEmergencyPanelV1 {
    /// Largest accepted JSON payload.
    pub const MAX_PAYLOAD_BYTES: usize = 16 * 1024;

    /// The sole canonical custom parameter identity.
    #[must_use]
    pub fn parameter_id() -> &'static CustomParameterId {
        static ID: std::sync::OnceLock<CustomParameterId> = std::sync::OnceLock::new();
        ID.get_or_init(|| {
            "runtime_emergency_panel_v1"
                .parse()
                .expect("static parameter identity")
        })
    }

    /// Check the panels, the threshold and the lease bounds.
    ///
    /// # Errors
    /// The first rule the panel violates.
    pub fn validate(&self) -> Result<(), RuntimeEmergencyPanelErrorV1> {
        let canonical = |members: &[PublicKey]| {
            !members.is_empty() && members.windows(2).all(|pair| pair[0] < pair[1])
        };
        if !canonical(&self.primary)
            || !canonical(&self.backup)
            || self
                .primary
                .iter()
                .any(|member| self.backup.binary_search(member).is_ok())
        {
            return Err(RuntimeEmergencyPanelErrorV1::Members);
        }
        let threshold = u64::from(self.threshold);
        for panel in [&self.primary, &self.backup] {
            let size = panel.len() as u64;
            if threshold == 0 || threshold > size || 3 * threshold < 2 * size {
                return Err(RuntimeEmergencyPanelErrorV1::Threshold);
            }
        }
        if self.min_lease_ms == 0 || self.min_lease_ms > self.max_lease_ms {
            return Err(RuntimeEmergencyPanelErrorV1::Lease);
        }
        Ok(())
    }

    /// Decode and validate the exact committed parameter, without substituting defaults.
    ///
    /// # Errors
    /// Another identity, more than [`Self::MAX_PAYLOAD_BYTES`] of JSON, a malformed or
    /// non-canonical payload, or a value that fails [`Self::validate`].
    pub fn from_custom_parameter(
        parameter: &CustomParameter,
    ) -> Result<Self, RuntimeEmergencyPanelErrorV1> {
        if parameter.id() != Self::parameter_id() {
            return Err(RuntimeEmergencyPanelErrorV1::CustomParameter(
                "another parameter identity",
            ));
        }
        let payload = parameter.payload();
        if payload.get().len() > Self::MAX_PAYLOAD_BYTES {
            return Err(RuntimeEmergencyPanelErrorV1::CustomParameter(
                "payload exceeds 16384 bytes",
            ));
        }
        let value: Self = norito::json::from_str(payload.get())
            .map_err(|_| RuntimeEmergencyPanelErrorV1::CustomParameter("malformed payload"))?;
        if Json::try_new(&value).ok().as_ref() != Some(payload) {
            return Err(RuntimeEmergencyPanelErrorV1::CustomParameter(
                "payload is not the canonical JSON of its value",
            ));
        }
        value.validate()?;
        Ok(value)
    }

    /// The committed custom parameter of a valid panel, with its canonical JSON payload.
    ///
    /// # Errors
    /// A panel that fails [`Self::validate`], or a payload that cannot be produced.
    pub fn to_custom_parameter(&self) -> Result<CustomParameter, RuntimeEmergencyPanelErrorV1> {
        self.validate()?;
        let payload = Json::try_new(self).map_err(|_| {
            RuntimeEmergencyPanelErrorV1::CustomParameter("payload cannot be serialized")
        })?;
        Ok(CustomParameter::new(Self::parameter_id().clone(), payload))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    fn members(seeds: std::ops::Range<u8>) -> Vec<PublicKey> {
        let mut keys: Vec<PublicKey> = seeds
            .map(|seed| {
                KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone()
            })
            .collect();
        keys.sort();
        keys
    }

    fn panel() -> RuntimeEmergencyPanelV1 {
        RuntimeEmergencyPanelV1 {
            primary: members(1..10),
            backup: members(10..19),
            threshold: 6,
            term_until_ms: 7 * 24 * 60 * 60 * 1000,
            min_lease_ms: 48 * 60 * 60 * 1000,
            max_lease_ms: 14 * 24 * 60 * 60 * 1000,
        }
    }

    #[test]
    fn a_valid_panel_round_trips_through_its_parameter() {
        let panel = panel();
        let parameter = panel.to_custom_parameter().expect("valid panel");
        assert_eq!(
            RuntimeEmergencyPanelV1::from_custom_parameter(&parameter),
            Ok(panel)
        );
    }

    #[test]
    fn panels_are_canonical_and_disjoint() {
        let mut shared = panel();
        shared.backup[0] = shared.primary[0].clone();
        shared.backup.sort();
        assert_eq!(
            shared.validate(),
            Err(RuntimeEmergencyPanelErrorV1::Members)
        );
        let mut unsorted = panel();
        unsorted.primary.swap(0, 1);
        assert_eq!(
            unsorted.validate(),
            Err(RuntimeEmergencyPanelErrorV1::Members)
        );
    }

    #[test]
    fn the_threshold_is_two_thirds_of_each_panel() {
        let mut low = panel();
        low.threshold = 5;
        assert_eq!(low.validate(), Err(RuntimeEmergencyPanelErrorV1::Threshold));
        let mut high = panel();
        high.threshold = 10;
        assert_eq!(
            high.validate(),
            Err(RuntimeEmergencyPanelErrorV1::Threshold)
        );
        let mut inverted = panel();
        inverted.min_lease_ms = inverted.max_lease_ms + 1;
        assert_eq!(
            inverted.validate(),
            Err(RuntimeEmergencyPanelErrorV1::Lease)
        );
    }
}
