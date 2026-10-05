//! Incumbent-certified scheduling authorizations for native validator epochs.
//!
//! An authorization names one scheduling epoch, its inclusive height bounds, the signing
//! generation it selects, its threshold-beacon binding and its predecessor. The body carries no
//! certificate: the incumbent boundary quorum authenticates it separately, so its identity is
//! independent of the signer subset.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId,
    isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
};

/// Sole first-release scheduling authorization layout version.
const AUTHORIZATION_VERSION_V1: u16 = 1;
// TODO(S21): the paired-Pasta mint-finality circuit still recomputes this exact transcript,
// including its domain. Rename the domain when that authority is removed and the body reshaped.
const AUTHORIZATION_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality-epoch-authorization";

/// Exact authority of an installed threshold-beacon transcript.
///
/// Norito encodes `session_id` before `transcript_hash`. The installed enum
/// variant owns one length-delimited instance of this body.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct InstalledBeaconEpochBindingV1 {
    /// Non-zero session identifier.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub session_id: [u8; 32],
    /// Recomputed complete transcript commitment.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub transcript_hash: [u8; 32],
}

/// Exact beacon authority bound to one scheduling epoch.
///
/// JSON uses a closed `kind`/`value` envelope with `bootstrap` and `installed`
/// tags; only `bootstrap` has a null value.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum BeaconEpochBindingV1 {
    /// Generation-zero genesis before the authenticated initial ceremony is installed.
    Bootstrap,
    /// Complete binding to an authenticated installed threshold-beacon transcript.
    Installed(InstalledBeaconEpochBindingV1),
}

/// Decision certified by the incumbent boundary quorum for the next scheduling epoch.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[repr(u8)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ValidatorEpochDecisionV1 {
    /// Signed genesis establishes the initial authorization.
    Genesis = 0,
    /// Activate one complete prepared successor generation.
    Activate = 1,
    /// Retain the exact incumbent generation and installed beacon authority.
    Retain = 2,
    /// Retain the incumbent and certify cancellation of the named frozen attempt.
    RetainAndCancel = 3,
}

/// Complete scheduling authority certified by the incumbent boundary quorum.
///
/// This body excludes its certificate, so its identity is independent of signer subset and
/// cannot recursively depend on the context or certificate which authenticates it.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi::epoch::ValidatorEpochAuthorizationV1")]
pub struct ValidatorEpochAuthorizationV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Monotonic scheduling epoch, advanced even when the incumbent is retained.
    pub epoch: u64,
    /// First governed height, inclusive.
    pub first_height: u64,
    /// Last governed height, inclusive.
    pub last_height: u64,
    /// Generation whose paired-Pasta keys are authorized.
    pub authority_generation: u64,
    /// Exact immutable ordered authority commitment.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub authority_id: [u8; 32],
    /// Exact installed beacon authority, or the explicit genesis bootstrap state.
    pub beacon: BeaconEpochBindingV1,
    /// Previous authorization identity; zero only for genesis.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub previous_authorization_id: [u8; 32],
    /// Frozen attempt identity for activation or certified cancellation; otherwise zero.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub transition_id: [u8; 32],
    /// Incumbent-certified disposition of the next scheduling epoch.
    pub decision: ValidatorEpochDecisionV1,
}

impl ValidatorEpochAuthorizationV1 {
    /// Construct the initial scheduling authorization from the signed genesis authority.
    ///
    /// The network and complete key commitment come from the validated generation-zero
    /// authority. This constructs the body; callers still authenticate the signed genesis.
    ///
    /// # Errors
    /// Rejects invalid authorities, nonzero generations, and an empty height interval.
    pub fn genesis(
        authority: &KagemushaMintFinalityAuthorityGenerationV1,
        last_height: u64,
    ) -> Result<Self, ValidatorEpochAuthorizationErrorV1> {
        let authorization = Self {
            version: AUTHORIZATION_VERSION_V1,
            network_id: authority.network_id,
            epoch: 0,
            first_height: 1,
            last_height,
            authority_generation: authority.generation,
            authority_id: signing_generation_id(authority)?,
            beacon: BeaconEpochBindingV1::Bootstrap,
            previous_authorization_id: [0; 32],
            transition_id: [0; 32],
            decision: ValidatorEpochDecisionV1::Genesis,
        };
        authorization.validate_against_authority(authority)?;
        Ok(authorization)
    }

    /// Validate canonical shape without treating this body as a certificate.
    ///
    /// # Errors
    /// Rejects invalid versions, empty identities or intervals, and contradictory decisions.
    pub fn validate(&self) -> Result<(), ValidatorEpochAuthorizationErrorV1> {
        if self.version != AUTHORIZATION_VERSION_V1 {
            return Err(ValidatorEpochAuthorizationErrorV1::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.network_id.as_bytes() == &[0; 32]
            || self.authority_id == [0; 32]
            || self.first_height == 0
            || self.last_height < self.first_height
        {
            return Err(invalid("epoch_authorization"));
        }
        match self.beacon {
            BeaconEpochBindingV1::Bootstrap => {
                if self.decision != ValidatorEpochDecisionV1::Genesis {
                    return Err(invalid("epoch_authorization.beacon"));
                }
            }
            BeaconEpochBindingV1::Installed(binding) => {
                if binding.session_id == [0; 32] || binding.transcript_hash == [0; 32] {
                    return Err(invalid("epoch_authorization.beacon"));
                }
            }
        }
        match self.decision {
            ValidatorEpochDecisionV1::Genesis => {
                if self.epoch != 0
                    || self.authority_generation != 0
                    || self.first_height != 1
                    || self.previous_authorization_id != [0; 32]
                    || self.transition_id != [0; 32]
                    || self.beacon != BeaconEpochBindingV1::Bootstrap
                {
                    return Err(invalid("epoch_authorization.genesis"));
                }
            }
            decision => {
                if self.epoch == 0
                    || self.first_height == 1
                    || self.previous_authorization_id == [0; 32]
                    || matches!(self.beacon, BeaconEpochBindingV1::Bootstrap)
                    || ((decision == ValidatorEpochDecisionV1::Retain)
                        != (self.transition_id == [0; 32]))
                {
                    return Err(invalid("epoch_authorization.decision"));
                }
            }
        }
        Ok(())
    }

    /// Validate the exact generation selected by this authorization.
    ///
    /// # Errors
    /// Rejects any mismatch in network, generation number, or complete authority commitment.
    pub fn validate_against_authority(
        &self,
        authority: &KagemushaMintFinalityAuthorityGenerationV1,
    ) -> Result<(), ValidatorEpochAuthorizationErrorV1> {
        self.validate()?;
        if self.network_id != authority.network_id {
            return Err(invalid("epoch_authorization.authority"));
        }
        // The authorization's authority_generation names this record's generation.
        if self.authority_generation != authority.generation {
            return Err(invalid("epoch_authorization.authority"));
        }
        if self.authority_id != signing_generation_id(authority)? {
            return Err(invalid("epoch_authorization.authority"));
        }
        Ok(())
    }

    /// Validate one contiguous scheduling transition from an authenticated predecessor.
    ///
    /// The caller must separately verify the incumbent boundary certificate and, for activation,
    /// the complete prepared target and all-seat custody readiness. A genesis-to-retain transition
    /// binds the initial ceremony from authenticated state; later retention preserves its binding.
    ///
    /// # Errors
    /// Rejects epoch or height gaps, stale parent identity, generation relabeling, or changed
    /// incumbent authority on retention.
    pub fn validate_successor(
        &self,
        previous: &Self,
    ) -> Result<(), ValidatorEpochAuthorizationErrorV1> {
        self.validate()?;
        previous.validate()?;
        if self.network_id != previous.network_id
            || previous.epoch.checked_add(1) != Some(self.epoch)
            || previous.last_height.checked_add(1) != Some(self.first_height)
            || self.previous_authorization_id != previous.authorization_id()?
        {
            return Err(invalid("epoch_authorization.predecessor"));
        }
        match self.decision {
            ValidatorEpochDecisionV1::Genesis => {
                return Err(invalid("epoch_authorization.successor"));
            }
            ValidatorEpochDecisionV1::Activate => {
                if previous.authority_generation.checked_add(1) != Some(self.authority_generation)
                    || self.authority_id == previous.authority_id
                {
                    return Err(invalid("epoch_authorization.activation"));
                }
            }
            ValidatorEpochDecisionV1::Retain | ValidatorEpochDecisionV1::RetainAndCancel => {
                if self.authority_generation != previous.authority_generation
                    || self.authority_id != previous.authority_id
                    || (previous.beacon != BeaconEpochBindingV1::Bootstrap
                        && self.beacon != previous.beacon)
                {
                    return Err(invalid("epoch_authorization.retention"));
                }
            }
        }
        Ok(())
    }

    /// Compute the fixed-width identity certified by the incumbent boundary quorum.
    ///
    /// # Errors
    /// Returns an error unless the authorization has a canonical shape.
    pub fn authorization_id(&self) -> Result<[u8; 32], ValidatorEpochAuthorizationErrorV1> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(AUTHORIZATION_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(self.version.to_le_bytes());
        hasher.update(self.network_id.as_bytes());
        hasher.update(self.epoch.to_le_bytes());
        hasher.update(self.first_height.to_le_bytes());
        hasher.update(self.last_height.to_le_bytes());
        hasher.update(self.authority_generation.to_le_bytes());
        hasher.update(self.authority_id);
        match self.beacon {
            BeaconEpochBindingV1::Bootstrap => {
                hasher.update([0]);
                hasher.update([0; 64]);
            }
            BeaconEpochBindingV1::Installed(binding) => {
                hasher.update([1]);
                hasher.update(binding.session_id);
                hasher.update(binding.transcript_hash);
            }
        }
        hasher.update(self.previous_authorization_id);
        hasher.update(self.transition_id);
        hasher.update([self.decision as u8]);
        Ok(hasher.finalize().into())
    }
}

/// Structural failure of a scheduling authorization body or its selected signing generation.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValidatorEpochAuthorizationErrorV1 {
    /// The body used a version other than the sole first-release layout.
    #[error("unsupported validator epoch authorization version {actual}")]
    UnsupportedVersion {
        /// Encountered version.
        actual: u16,
    },
    /// A required field was zero, inconsistent, or non-canonical.
    #[error("invalid validator epoch authorization field `{field}`")]
    InvalidField {
        /// Stable field label.
        field: &'static str,
    },
    /// The signing generation selected by the authorization failed its own validation.
    #[error("invalid validator epoch signing generation: {0}")]
    InvalidSigningGeneration(String),
}

fn invalid(field: &'static str) -> ValidatorEpochAuthorizationErrorV1 {
    ValidatorEpochAuthorizationErrorV1::InvalidField { field }
}

// TODO(S21): replace with the neutral validator-generation commitment once the paired-Pasta
// mint-finality generation is removed from the authorization.
fn signing_generation_id(
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
) -> Result<[u8; 32], ValidatorEpochAuthorizationErrorV1> {
    authority.authority_id().map_err(|error| {
        ValidatorEpochAuthorizationErrorV1::InvalidSigningGeneration(error.to_string())
    })
}

#[cfg(test)]
#[path = "authorization_tests.rs"]
mod tests;
