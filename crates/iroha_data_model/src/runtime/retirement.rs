//! Evidence for source-code retirement, distinct from local release eligibility.
//!
//! A selected native producer must establish permanent exclusion, the last code-needed
//! height, an effective startup floor, and empty dependency/custody reachability before it
//! writes this statement. Inclusion alone establishes none of those producer obligations.
//! Descriptors, stored-value codecs, checkpoint readers and proof verifiers remain retained.

use iroha_crypto::Hash;
use norito::codec::{Decode, Encode};

use super::{checkpoint::StartupFloorV1, definition::DefinitionId};
use crate::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    sumeragi_amx::{AmxCertifiedBlockV1, AmxError, AmxWriteProofV1},
};

/// Finite complete proof bound, separate from an enclosing caller's allocation limits.
pub const MAX_RETIREMENT_PROOF_BYTES: usize = 1024 * 1024;
/// Permanent definition-history capacity; publication refuses before exhausting this table.
pub const MAX_RETIREMENT_DEFINITIONS: usize = 65_536;
/// Selected native service owning the permanent retirement state and witness namespace.
pub const SERVICE: &str = "runtime_retirement";
/// Immutable semantics of the retirement producer and activation guard.
pub const SERVICE_CLOSURE: &[u8] = b"iroha:runtime:retirement:v1:published-history-code-needed-highwater-permanent-exclusion-effective-floor-empty-live-closure-retained-readers";

/// Exact genesis-bound identity of one independently signed execution root.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::runtime::retirement::RuntimeRetirementRootV1")]
pub struct RuntimeRetirementRootV1 {
    /// Root's signed-genesis network identity.
    pub network: NetworkId,
    /// Global or exact full-width parent/data-space attachment.
    pub scope: SumeragiRootScope,
    /// Native consensus instance derived from the original signed genesis and chain.
    pub instance: [u8; 32],
}

/// A permanent source-owned statement about one immutable definition.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::runtime::retirement::RuntimeRetirementReceiptV1")]
pub struct RuntimeRetirementReceiptV1 {
    /// Original execution root; a relay cannot replace any component.
    pub root: RuntimeRetirementRootV1,
    /// Exact immutable definition whose handler code is excluded permanently.
    pub definition: DefinitionId,
    /// First height covered by the selected native history owner.
    pub history_started: u64,
    /// Latest height at which execution or a live completion could need its code.
    /// `None` means it was never published or selected within the complete retained history.
    #[norito(required)]
    pub last_required: Option<u64>,
    /// Height of permanent exclusion; later activation cannot lift this tombstone.
    pub excluded_at: u64,
    /// Effective, independently governed startup floor observed by the producer.
    pub startup_floor: StartupFloorV1,
    /// Original ordinary-write carrier. A later refresh preserves the exclusion and high water.
    pub receipt_height: u64,
}

/// Untrusted bounded transport of an exact native retirement write and its finality carrier.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::runtime::retirement::RuntimeRetirementProofV1")]
pub struct RuntimeRetirementProofV1 {
    /// Permanent native exclusion statement.
    pub receipt: RuntimeRetirementReceiptV1,
    /// Original source quorum and complete executed result preimage.
    pub block: AmxCertifiedBlockV1,
    /// Exact ordinary-write path under that authenticated result.
    pub path: AmxWriteProofV1,
}

/// Addition-owned permanent history of one immutable definition, keyed by `DefinitionId`.
/// Descriptors remain metadata after their executable code no longer advances this high water.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::runtime::retirement::RuntimeRetirementStateV1")]
pub struct RuntimeRetirementStateV1 {
    /// Initial conservative census height of this root's complete published definition set.
    pub history_started: u64,
    /// Most recent height at which the selected runtime or completion graph needed its code.
    #[norito(required)]
    pub last_required: Option<u64>,
    /// Original governed disable or deactivation retained after its selection restriction
    /// is removed. Reactivation before finalization clears this pending authorization.
    #[norito(required)]
    pub governed_excluded_at: Option<u64>,
    /// Permanent exclusion and latest authentic witness carrier, once retirement is finalized.
    #[norito(required)]
    pub receipt: Option<RuntimeRetirementReceiptV1>,
}

impl RuntimeRetirementStateV1 {
    /// Validate restored local history and its immutable exclusion fields.
    /// # Errors
    /// Missing history, a regressed use height, or a receipt for different retained history.
    pub fn validate(&self, definition: DefinitionId) -> Result<(), AmxError> {
        if self.history_started < 2
            || self
                .last_required
                .is_some_and(|height| height < self.history_started)
            || self
                .governed_excluded_at
                .is_some_and(|height| height < self.history_started)
        {
            return Err(AmxError::Record("invalid permanent runtime history"));
        }
        if let Some(receipt) = &self.receipt {
            receipt.validate()?;
            if receipt.definition != definition
                || receipt.history_started != self.history_started
                || receipt.last_required != self.last_required
                || self
                    .governed_excluded_at
                    .is_none_or(|height| height > receipt.excluded_at)
            {
                return Err(AmxError::Record(
                    "retirement receipt differs from retained history",
                ));
            }
        }
        Ok(())
    }
}

fn encoding(error: norito::core::DecodeAttemptError) -> AmxError {
    if error.kind() == norito::core::DecodeAttemptErrorKind::Invalid {
        AmxError::Encoding(error.to_string())
    } else {
        AmxError::Resource(error)
    }
}

impl RuntimeRetirementReceiptV1 {
    /// Check intrinsic identity and height rules, without granting producer authority.
    /// # Errors
    /// Invalid root, missing history, ineffective order or a floor that still needs the code.
    pub fn validate(&self) -> Result<(), AmxError> {
        self.root
            .scope
            .validate()
            .map_err(|_| AmxError::Record("invalid retirement root scope"))?;
        if self.history_started < 2
            || self.excluded_at < self.history_started
            || self.receipt_height < self.excluded_at
            || self.startup_floor.network != self.root.network
            || self.startup_floor.height == 0
            || self.startup_floor.height > self.receipt_height
            || self.last_required.is_some_and(|height| {
                height < self.history_started
                    || height >= self.excluded_at
                    || height >= self.startup_floor.height
            })
        {
            return Err(AmxError::Record(
                "retirement history, exclusion or startup floor differs",
            ));
        }
        Ok(())
    }

    /// The service-owned ordinary-write namespace of this immutable definition.
    #[must_use]
    pub fn witness_key(&self) -> Vec<u8> {
        let mut key = b"iroha:runtime:retirement:v1\0".to_vec();
        key.extend_from_slice(&self.definition.0);
        key
    }
}

impl RuntimeRetirementProofV1 {
    /// Reject an oversized offered graph before decoding or verifying its certificate.
    /// # Errors
    /// Intrinsic proof limits or the original enclosing codec/allocator refusal.
    pub fn validate_bounds(&self) -> Result<(), AmxError> {
        self.receipt.validate()?;
        if self.block.consensus_header.len() > 8192
            || self.block.commit_qc.len() > 128 * 1024
            || self.block.result_preimage.len() > MAX_RETIREMENT_PROOF_BYTES - 140 * 1024
            || self.path.siblings.len() > 256
            || norito::core::classify_decode_attempt(|| norito::canonical_frame_len(self))
                .map_err(encoding)?
                > MAX_RETIREMENT_PROOF_BYTES
        {
            return Err(AmxError::Proof(
                "retirement proof exceeds its intrinsic bound".into(),
            ));
        }
        Ok(())
    }

    /// Check inclusion against a root and result already authenticated by the caller.
    /// This method verifies no quorum, membership, producer semantics or deletion inventory.
    /// # Errors
    /// Wrong source, target, carrier height, ordinary-write path, or original codec refusal.
    pub fn verify_inclusion(
        &self,
        root: RuntimeRetirementRootV1,
        definition: DefinitionId,
        certified_height: u64,
        ordinary_writes_root: Hash,
    ) -> Result<(), AmxError> {
        self.validate_bounds()?;
        if self.receipt.root != root
            || self.receipt.definition != definition
            || self.receipt.receipt_height != certified_height
        {
            return Err(AmxError::Proof(
                "retirement proof source, definition or height differs".into(),
            ));
        }
        let value =
            norito::core::classify_decode_attempt(|| norito::encode_canonical(&self.receipt))
                .map_err(encoding)?;
        if self.path.root(&self.receipt.witness_key(), &value)? != ordinary_writes_root {
            return Err(AmxError::Proof(
                "retirement record is absent from certified ordinary writes".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
