//! Data availability helpers and ingest infrastructure.
//!
//! The DA program introduces Torii ingestion, replication, and enforcement logic that
//! spans multiple crates. This module collects shared building blocks that are consumed
//! by Torii surfaces, orchestrators, and validation pipelines.

#![allow(clippy::module_name_repetitions)]

pub mod commitment_store;
pub mod commitments;
pub mod confidential;
pub mod confidential_store;
pub mod pin_intents;
pub mod pin_store;
pub mod proofs;
pub mod receipts;
pub mod replay_cache;
pub mod shard_cursor;

use std::collections::BTreeSet;

pub use confidential::{ConfidentialComputeError, validate_confidential_compute_record};
use iroha_config::parameters::actual::LaneConfig;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    da::{
        commitment::{
            DaCommitmentBundle, DaCommitmentKey, DaCommitmentRecord, DaProofPolicyBundle,
            DaProofScheme, KzgCommitment,
        },
        prelude::DaProofPolicy,
    },
    nexus::LaneId,
};
pub use proofs::{DaProofVerificationError, build_da_commitment_proof, verify_da_commitment_proof};
pub use replay_cache::{
    LaneEpoch, ReplayCache, ReplayCacheConfig, ReplayFingerprint, ReplayInsertOutcome, ReplayKey,
};
pub use shard_cursor::{
    DaShardCursor, DaShardCursorError, DaShardCursorIndex, DaShardCursorJournal, LaneShardCursor,
    ShardCursorJournalError,
};
use thiserror::Error;

use crate::da::pin_intents::{PinIntentDropReason, canonicalize_bundle};

/// Errors returned when a DA commitment violates the configured lane proof policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DaProofPolicyError {
    /// Lane referenced by the commitment is not present in the configured catalog.
    #[error("lane {lane} not present in the configured lane catalog")]
    UnknownLane {
        /// Lane identifier that failed the lookup.
        lane: LaneId,
    },
    /// Commitment advertises a proof scheme that differs from the lane policy.
    #[error("lane {lane} expects proof scheme {expected} but commitment uses {observed}")]
    SchemeMismatch {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Proof scheme declared in the lane catalog.
        expected: DaProofScheme,
        /// Proof scheme attached to the commitment.
        observed: DaProofScheme,
    },
    /// A KZG lane must carry a non-zero KZG commitment.
    #[error("lane {lane} requires a non-zero KZG commitment")]
    MissingKzgCommitment {
        /// Lane identifier that failed validation.
        lane: LaneId,
    },
    /// Merkle lanes must not embed a KZG commitment.
    #[error("lane {lane} is configured for Merkle proofs and must not include a KZG commitment")]
    UnexpectedKzgCommitment {
        /// Lane identifier that failed validation.
        lane: LaneId,
    },
    /// Chunk root is zeroed for the commitment.
    #[error("lane {lane} carries a zeroed chunk_root commitment")]
    ZeroChunkRoot {
        /// Lane identifier that failed validation.
        lane: LaneId,
    },
    /// KZG commitment is zeroed.
    #[error("lane {lane} carries a zeroed KZG commitment")]
    ZeroKzgCommitment {
        /// Lane identifier that failed validation.
        lane: LaneId,
    },
}

/// Errors returned when a DA commitment bundle violates invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DaCommitmentValidationError {
    /// Underlying proof policy validation failed.
    #[error(transparent)]
    ProofPolicy(#[from] DaProofPolicyError),
    /// Confidential-compute policy validation failed.
    #[error(transparent)]
    ConfidentialCompute(#[from] ConfidentialComputeError),
    /// Commitment bundle length cannot be represented in DA location/proof metadata.
    #[error("DA commitment bundle length {len} exceeds representable maximum {max}")]
    BundleTooLarge {
        /// Number of commitments in the bundle.
        len: usize,
        /// Maximum supported commitment count for one bundle.
        max: usize,
    },
    /// Duplicate `(lane, epoch, sequence)` commitment found.
    #[error(
        "duplicate DA commitment detected for lane {key_lane}, epoch {epoch}, sequence {sequence}"
    )]
    DuplicateCommitment {
        /// Lane identifier that failed validation.
        key_lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
    },
    /// Duplicate manifest hash detected within the bundle.
    #[error("duplicate DA manifest hash detected for lane {lane}")]
    DuplicateManifest {
        /// Lane identifier that failed validation.
        lane: LaneId,
    },
    /// Manifest hash already exists in a previously committed DA record.
    #[error(
        "DA manifest hash for lane {lane}, epoch {epoch}, sequence {sequence} was already committed at lane {existing_lane}, epoch {existing_epoch}, sequence {existing_sequence}"
    )]
    CommittedManifest {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
        /// Lane identifier of the already committed record.
        existing_lane: LaneId,
        /// Epoch of the already committed record.
        existing_epoch: u64,
        /// Sequence number of the already committed record.
        existing_sequence: u64,
    },
    /// Duplicate storage ticket detected within the bundle.
    #[error(
        "duplicate DA storage ticket detected for lane {lane}, epoch {epoch}, sequence {sequence}"
    )]
    DuplicateStorageTicket {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
    },
    /// Storage ticket already exists in a previously committed DA record.
    #[error(
        "DA storage ticket for lane {lane}, epoch {epoch}, sequence {sequence} was already committed at lane {existing_lane}, epoch {existing_epoch}, sequence {existing_sequence}"
    )]
    CommittedStorageTicket {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
        /// Lane identifier of the already committed record.
        existing_lane: LaneId,
        /// Epoch of the already committed record.
        existing_epoch: u64,
        /// Sequence number of the already committed record.
        existing_sequence: u64,
    },
    /// Manifest hash must be non-zero.
    #[error("lane {lane} carries a zeroed manifest hash at epoch {epoch}, sequence {sequence}")]
    ZeroManifestHash {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence that failed validation.
        sequence: u64,
    },
    /// Storage ticket must be non-zero.
    #[error("lane {lane} carries a zeroed storage ticket at epoch {epoch}, sequence {sequence}")]
    ZeroStorageTicket {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence that failed validation.
        sequence: u64,
    },
}

/// Errors returned when a DA pin intent violates invariants.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DaPinIntentValidationError {
    /// Pin intent bundle version is unsupported.
    #[error("unsupported DA pin-intent bundle version {version}")]
    UnsupportedVersion {
        /// Version carried by the bundle.
        version: u16,
    },
    /// Lane referenced by the pin intent is not present in the configured catalog.
    #[error("lane {lane} not present in the configured lane catalog")]
    UnknownLane {
        /// Lane identifier that failed the lookup.
        lane: LaneId,
    },
    /// Owner account referenced by the pin intent is missing from WSV.
    #[error(
        "owner {owner} not present for lane {lane} epoch {epoch} sequence {sequence} pin intent"
    )]
    UnknownOwner {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
        /// Owner account missing from the registry.
        owner: AccountId,
    },
    /// Duplicate `(lane, epoch, sequence)` pin intent found.
    #[error("duplicate DA pin intent detected for lane {lane}, epoch {epoch}, sequence {sequence}")]
    DuplicateIntent {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
    },
    /// Duplicate storage ticket found.
    #[error(
        "duplicate DA pin-intent storage ticket detected for lane {lane}, epoch {epoch}, sequence {sequence}"
    )]
    DuplicateStorageTicket {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
    },
    /// Duplicate manifest hash found.
    #[error(
        "duplicate DA pin-intent manifest hash detected for lane {lane}, epoch {epoch}, sequence {sequence}"
    )]
    DuplicateManifest {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence number that failed validation.
        sequence: u64,
    },
    /// Manifest hash must be non-zero.
    #[error(
        "lane {lane} carries a zeroed pin-intent manifest hash at epoch {epoch}, sequence {sequence}"
    )]
    ZeroManifestHash {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence that failed validation.
        sequence: u64,
    },
    /// Storage ticket must be non-zero.
    #[error("lane {lane} carries a zeroed storage ticket at epoch {epoch}, sequence {sequence}")]
    ZeroStorageTicket {
        /// Lane identifier that failed validation.
        lane: LaneId,
        /// Epoch that failed validation.
        epoch: u64,
        /// Sequence that failed validation.
        sequence: u64,
    },
    /// Alias collision resolved to a different storage ticket.
    #[error("alias {alias} superseded: dropped {dropped_ticket:?} in favour of {kept_ticket:?}")]
    AliasSuperseded {
        /// Alias that collided.
        alias: String,
        /// Ticket rejected for this alias.
        dropped_ticket: iroha_data_model::da::types::StorageTicketId,
        /// Ticket kept for this alias.
        kept_ticket: iroha_data_model::da::types::StorageTicketId,
    },
}

/// Enforce that a commitment's proof scheme matches the configured lane policy.
///
/// Merkle lanes reject attached KZG commitments. KZG lanes require a non-zero
/// KZG commitment.
///
/// # Errors
///
/// Returns a [`DaProofPolicyError`] when the lane is unknown, the proof scheme
/// does not match the lane policy, or a KZG lane carries a zeroed commitment.
pub fn enforce_lane_proof_policy(
    record: &DaCommitmentRecord,
    lane_config: &LaneConfig,
) -> Result<(), DaProofPolicyError> {
    let Some(entry) = lane_config.entry(record.lane_id) else {
        return Err(DaProofPolicyError::UnknownLane {
            lane: record.lane_id,
        });
    };

    if entry.proof_scheme != record.proof_scheme {
        return Err(DaProofPolicyError::SchemeMismatch {
            lane: record.lane_id,
            expected: entry.proof_scheme,
            observed: record.proof_scheme,
        });
    }

    let chunk_root = record.chunk_root.as_ref();
    let chunk_root_zero_like = chunk_root[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0)
        && chunk_root[Hash::LENGTH - 1] <= 1;
    if chunk_root_zero_like {
        return Err(DaProofPolicyError::ZeroChunkRoot {
            lane: record.lane_id,
        });
    }

    match entry.proof_scheme {
        DaProofScheme::MerkleSha256 => {
            if record.kzg_commitment.is_some() {
                return Err(DaProofPolicyError::UnexpectedKzgCommitment {
                    lane: record.lane_id,
                });
            }
        }
        DaProofScheme::KzgBls12_381 => {
            let Some(kzg) = record.kzg_commitment else {
                return Err(DaProofPolicyError::MissingKzgCommitment {
                    lane: record.lane_id,
                });
            };
            if kzg.as_bytes() == &KzgCommitment::ZERO_BYTES {
                return Err(DaProofPolicyError::ZeroKzgCommitment {
                    lane: record.lane_id,
                });
            }
        }
    }

    Ok(())
}

/// Filter DA pin intents using the configured lane catalog.
///
/// Returns `(kept, rejected)` where `kept` contains intents that passed validation
/// and `rejected` lists every validation failure. Callers decide whether
/// rejected local spool entries are fatal or can be dropped for their workflow.
pub fn sanitize_pin_intents(
    intents: impl IntoIterator<Item = iroha_data_model::da::pin_intent::DaPinIntent>,
    lane_config: &LaneConfig,
    account_exists: impl Fn(&AccountId) -> bool,
) -> (
    Vec<iroha_data_model::da::pin_intent::DaPinIntent>,
    Vec<DaPinIntentValidationError>,
) {
    let mut seen = BTreeSet::new();
    let mut seen_tickets = BTreeSet::new();
    let mut seen_manifests = BTreeSet::new();
    let mut kept = Vec::new();
    let mut rejected = Vec::new();

    let (canonical, drop_reasons) = canonicalize_bundle(
        iroha_data_model::da::pin_intent::DaPinIntentBundle::new(intents.into_iter().collect()),
    );
    for reason in drop_reasons {
        match reason {
            PinIntentDropReason::ZeroManifest {
                lane,
                epoch,
                sequence,
            } => rejected.push(DaPinIntentValidationError::ZeroManifestHash {
                lane: LaneId::new(lane),
                epoch,
                sequence,
            }),
            PinIntentDropReason::DuplicateIntent {
                lane,
                epoch,
                sequence,
                ..
            } => rejected.push(DaPinIntentValidationError::DuplicateIntent {
                lane: LaneId::new(lane),
                epoch,
                sequence,
            }),
            PinIntentDropReason::AliasSuperseded {
                alias,
                dropped_ticket,
                kept_ticket,
            } => rejected.push(DaPinIntentValidationError::AliasSuperseded {
                alias,
                dropped_ticket,
                kept_ticket,
            }),
        }
    }

    for intent in canonical.intents {
        let key = (intent.lane_id, intent.epoch, intent.sequence);

        if lane_config.entry(intent.lane_id).is_none() {
            rejected.push(DaPinIntentValidationError::UnknownLane {
                lane: intent.lane_id,
            });
            continue;
        }

        if intent
            .manifest_hash
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        {
            rejected.push(DaPinIntentValidationError::ZeroManifestHash {
                lane: intent.lane_id,
                epoch: intent.epoch,
                sequence: intent.sequence,
            });
            continue;
        }

        if intent.storage_ticket.as_ref().iter().all(|byte| *byte == 0) {
            rejected.push(DaPinIntentValidationError::ZeroStorageTicket {
                lane: intent.lane_id,
                epoch: intent.epoch,
                sequence: intent.sequence,
            });
            continue;
        }

        if let Some(owner) = intent.owner.as_ref() {
            if !account_exists(owner) {
                rejected.push(DaPinIntentValidationError::UnknownOwner {
                    lane: intent.lane_id,
                    epoch: intent.epoch,
                    sequence: intent.sequence,
                    owner: owner.clone(),
                });
                continue;
            }
        }

        if !seen.insert(key) {
            rejected.push(DaPinIntentValidationError::DuplicateIntent {
                lane: intent.lane_id,
                epoch: intent.epoch,
                sequence: intent.sequence,
            });
            continue;
        }

        if !seen_tickets.insert(intent.storage_ticket) {
            rejected.push(DaPinIntentValidationError::DuplicateStorageTicket {
                lane: intent.lane_id,
                epoch: intent.epoch,
                sequence: intent.sequence,
            });
            continue;
        }

        if !seen_manifests.insert(intent.manifest_hash) {
            rejected.push(DaPinIntentValidationError::DuplicateManifest {
                lane: intent.lane_id,
                epoch: intent.epoch,
                sequence: intent.sequence,
            });
            continue;
        }

        kept.push(intent);
    }

    (kept, rejected)
}

/// Validate a DA pin-intent bundle before accepting an inbound block.
///
/// Unlike local spool ingestion, inbound block validation is fail-closed:
/// invalid pin intents reject the block instead of being dropped during apply.
///
/// # Errors
///
/// Returns the first [`DaPinIntentValidationError`] observed in the bundle.
pub fn validate_pin_intent_bundle(
    bundle: &iroha_data_model::da::pin_intent::DaPinIntentBundle,
    lane_config: &LaneConfig,
    account_exists: impl Fn(&AccountId) -> bool,
) -> Result<(), DaPinIntentValidationError> {
    if bundle.version != iroha_data_model::da::pin_intent::DaPinIntentBundle::VERSION_V1 {
        return Err(DaPinIntentValidationError::UnsupportedVersion {
            version: bundle.version,
        });
    }

    let (_kept, rejected) =
        sanitize_pin_intents(bundle.intents.clone(), lane_config, account_exists);
    if let Some(error) = rejected.into_iter().next() {
        return Err(error);
    }

    Ok(())
}

/// Validate commitment bundle invariants before embedding into a block.
///
/// Enforces a representable bundle length, unique `(lane, epoch, sequence)`
/// tuples, unique manifest hashes and storage tickets within the bundle,
/// non-zero manifest hashes and storage tickets, and lane proof policy
/// compatibility.
///
/// # Errors
///
/// Returns a [`DaCommitmentValidationError`] when invariants are violated.
pub fn validate_commitment_bundle(
    bundle: &DaCommitmentBundle,
    lane_config: &LaneConfig,
) -> Result<(), DaCommitmentValidationError> {
    validate_commitment_bundle_len(bundle.commitments.len())?;

    let mut seen_keys = BTreeSet::new();
    let mut seen_manifests = BTreeSet::new();
    let mut seen_tickets = BTreeSet::new();

    for record in &bundle.commitments {
        if record
            .manifest_hash
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(DaCommitmentValidationError::ZeroManifestHash {
                lane: record.lane_id,
                epoch: record.epoch,
                sequence: record.sequence,
            });
        }

        if record.storage_ticket.as_ref().iter().all(|byte| *byte == 0) {
            return Err(DaCommitmentValidationError::ZeroStorageTicket {
                lane: record.lane_id,
                epoch: record.epoch,
                sequence: record.sequence,
            });
        }

        let key = DaCommitmentKey::from_record(record);
        if !seen_keys.insert(key) {
            return Err(DaCommitmentValidationError::DuplicateCommitment {
                key_lane: record.lane_id,
                epoch: record.epoch,
                sequence: record.sequence,
            });
        }

        if !seen_manifests.insert(record.manifest_hash) {
            return Err(DaCommitmentValidationError::DuplicateManifest {
                lane: record.lane_id,
            });
        }

        if !seen_tickets.insert(record.storage_ticket) {
            return Err(DaCommitmentValidationError::DuplicateStorageTicket {
                lane: record.lane_id,
                epoch: record.epoch,
                sequence: record.sequence,
            });
        }

        enforce_lane_proof_policy(record, lane_config)?;
        validate_confidential_compute_record(lane_config, record)?;
    }

    Ok(())
}

pub(crate) const MAX_DA_BUNDLE_LOCATIONS: usize = u32::MAX as usize;

pub(crate) fn da_bundle_location_index(index: usize) -> Option<u32> {
    if index >= MAX_DA_BUNDLE_LOCATIONS {
        return None;
    }
    u32::try_from(index).ok()
}

fn validate_commitment_bundle_len(len: usize) -> Result<(), DaCommitmentValidationError> {
    if len > MAX_DA_BUNDLE_LOCATIONS {
        return Err(DaCommitmentValidationError::BundleTooLarge {
            len,
            max: MAX_DA_BUNDLE_LOCATIONS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod proof_policy_tests {
    use iroha_data_model::{
        da::{
            pin_intent::{DaPinIntent, DaPinIntentBundle},
            types::StorageTicketId,
        },
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
    };

    use super::*;

    fn intent(
        lane: LaneId,
        epoch: u64,
        sequence: u64,
        manifest_bytes: [u8; 32],
        ticket_bytes: [u8; 32],
    ) -> DaPinIntent {
        DaPinIntent::new(
            lane,
            epoch,
            sequence,
            StorageTicketId::new(ticket_bytes),
            ManifestDigest::new(manifest_bytes),
        )
    }

    #[test]
    fn sanitize_pin_intents_drops_invalid_entries() {
        let lane_config = LaneConfig::default();
        let lane = lane_config.primary().lane_id;
        let valid = intent(lane, 1, 1, [1; 32], [2; 32]);
        let zero_manifest = intent(lane, 1, 2, [0; 32], [3; 32]);
        let zero_ticket = intent(lane, 1, 3, [4; 32], [0; 32]);
        let duplicate = intent(lane, 1, 1, [5; 32], [2; 32]);

        let (kept, rejected) = sanitize_pin_intents(
            vec![valid.clone(), zero_manifest, zero_ticket, duplicate.clone()],
            &lane_config,
            |_| true,
        );

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0], duplicate);
        assert_eq!(rejected.len(), 3);
        assert!(
            rejected
                .iter()
                .any(|err| matches!(err, DaPinIntentValidationError::ZeroManifestHash { .. }))
        );
        assert!(
            rejected
                .iter()
                .any(|err| matches!(err, DaPinIntentValidationError::ZeroStorageTicket { .. }))
        );
        assert!(
            rejected
                .iter()
                .any(|err| matches!(err, DaPinIntentValidationError::DuplicateIntent { .. }))
        );
    }

    #[test]
    fn sanitize_pin_intents_rejects_duplicate_sequence_with_new_ticket() {
        let lane_config = LaneConfig::default();
        let lane_id = lane_config.primary().lane_id;
        let first = intent(lane_id, 2, 4, [0x11; 32], [0x10; 32]);
        let second = intent(lane_id, 2, 4, [0x33; 32], [0x22; 32]);

        let (kept, rejected) =
            sanitize_pin_intents(vec![first.clone(), second.clone()], &lane_config, |_| true);

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].storage_ticket, second.storage_ticket);
        assert!(rejected.iter().any(|err| matches!(
            err,
            DaPinIntentValidationError::DuplicateIntent { lane, epoch, sequence }
                if *lane == lane_id && *epoch == 2 && *sequence == 4
        )));
    }

    #[test]
    fn sanitize_pin_intents_rejects_duplicate_ticket_and_manifest() {
        let lane_config = LaneConfig::default();
        let lane = lane_config.primary().lane_id;
        let first = intent(lane, 1, 1, [0x11; 32], [0x21; 32]);
        let duplicate_ticket = intent(lane, 1, 2, [0x12; 32], [0x21; 32]);
        let duplicate_manifest = intent(lane, 1, 3, [0x11; 32], [0x23; 32]);

        let (kept, rejected) = sanitize_pin_intents(
            [first.clone(), duplicate_ticket, duplicate_manifest],
            &lane_config,
            |_| true,
        );

        assert_eq!(kept, vec![first]);
        assert!(rejected.iter().any(|err| matches!(
            err,
            DaPinIntentValidationError::DuplicateStorageTicket {
                lane: seen_lane,
                epoch: 1,
                sequence: 2
            } if *seen_lane == lane
        )));
        assert!(rejected.iter().any(|err| matches!(
            err,
            DaPinIntentValidationError::DuplicateManifest {
                lane: seen_lane,
                epoch: 1,
                sequence: 3
            } if *seen_lane == lane
        )));
    }

    #[test]
    fn validate_pin_intent_bundle_rejects_unsupported_version() {
        let lane_config = LaneConfig::default();
        let lane = lane_config.primary().lane_id;
        let mut bundle = DaPinIntentBundle::new(vec![intent(lane, 1, 1, [1; 32], [2; 32])]);
        bundle.version = DaPinIntentBundle::VERSION_V1 + 1;

        let err = validate_pin_intent_bundle(&bundle, &lane_config, |_| true)
            .expect_err("unsupported pin intent bundle version must fail");
        assert!(matches!(
            err,
            DaPinIntentValidationError::UnsupportedVersion { version }
                if version == DaPinIntentBundle::VERSION_V1 + 1
        ));
    }

    #[test]
    fn validate_pin_intent_bundle_rejects_duplicate_ticket() {
        let lane_config = LaneConfig::default();
        let lane = lane_config.primary().lane_id;
        let bundle = DaPinIntentBundle::new(vec![
            intent(lane, 1, 1, [0x31; 32], [0x41; 32]),
            intent(lane, 1, 2, [0x32; 32], [0x41; 32]),
        ]);

        let err = validate_pin_intent_bundle(&bundle, &lane_config, |_| true)
            .expect_err("duplicate pin intent ticket must fail");
        assert!(matches!(
            err,
            DaPinIntentValidationError::DuplicateStorageTicket {
                lane: seen_lane,
                epoch: 1,
                sequence: 2
            } if seen_lane == lane
        ));
    }

    #[test]
    fn sanitize_pin_intents_rejects_unknown_lane() {
        let lane_config = LaneConfig::default();
        let unknown_lane = LaneId::new(99);
        let record = intent(unknown_lane, 1, 1, [9; 32], [9; 32]);

        let (kept, rejected) = sanitize_pin_intents([record], &lane_config, |_| true);

        assert!(kept.is_empty());
        assert!(matches!(
            rejected.as_slice(),
            [DaPinIntentValidationError::UnknownLane { lane }] if *lane == unknown_lane
        ));
    }

    #[test]
    fn sanitize_pin_intents_records_alias_collision() {
        let lane_config = LaneConfig::default();
        let lane = lane_config.primary().lane_id;
        let mut first = intent(lane, 1, 1, [1; 32], [1; 32]);
        first.alias = Some("alias-collision".to_string());
        let mut second = intent(lane, 1, 2, [2; 32], [2; 32]);
        second.alias = Some("alias-collision".to_string());

        let (kept, rejected) =
            sanitize_pin_intents(vec![first.clone(), second.clone()], &lane_config, |_| true);

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].storage_ticket, second.storage_ticket);
        assert!(rejected.iter().any(|err| matches!(
            err,
            DaPinIntentValidationError::AliasSuperseded {
                alias,
                dropped_ticket,
                kept_ticket
            } if alias == "alias-collision"
                && *dropped_ticket == first.storage_ticket
                && *kept_ticket == second.storage_ticket
        )));
    }

    #[test]
    fn sanitize_pin_intents_rejects_unknown_owner() {
        let lane_config = LaneConfig::default();
        let lane = lane_config.primary().lane_id;
        let mut with_owner = intent(lane, 2, 0, [0xAB; 32], [0xCD; 32]);
        with_owner.owner = Some(iroha_test_samples::ALICE_ID.clone());
        let without_owner = intent(lane, 3, 1, [0xEF; 32], [0x11; 32]);

        let (kept, rejected) = sanitize_pin_intents(
            vec![with_owner.clone(), without_owner.clone()],
            &lane_config,
            |_| false,
        );
        assert!(
            kept.iter()
                .all(|intent| intent.storage_ticket != with_owner.storage_ticket),
            "intent with unknown owner must be dropped"
        );
        assert!(
            kept.iter()
                .any(|intent| intent.storage_ticket == without_owner.storage_ticket)
        );
        assert!(
            rejected
                .iter()
                .any(|reason| matches!(reason, DaPinIntentValidationError::UnknownOwner { .. })),
            "unknown owner should surface as a validation error"
        );
    }
}

/// Snapshot the configured proof policies for all lanes.
#[must_use]
pub fn proof_policies(lane_config: &LaneConfig) -> Vec<DaProofPolicy> {
    lane_config
        .entries()
        .iter()
        .map(|entry| DaProofPolicy {
            lane_id: entry.lane_id,
            dataspace_id: entry.dataspace_id,
            alias: entry.alias.clone(),
            proof_scheme: entry.proof_scheme,
        })
        .collect()
}

/// Snapshot the configured proof policies as a versioned bundle.
#[must_use]
pub fn proof_policy_bundle(lane_config: &LaneConfig) -> DaProofPolicyBundle {
    DaProofPolicyBundle::new(proof_policies(lane_config))
}

/// Compute the hash for the current proof policy bundle.
#[must_use]
pub fn proof_policy_bundle_hash(lane_config: &LaneConfig) -> HashOf<DaProofPolicyBundle> {
    let bundle = proof_policy_bundle(lane_config);
    HashOf::new(&bundle)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::RetentionClass,
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneStorageProfile},
    };
    use norito::to_bytes;

    use super::*;

    fn lane_config_with(
        lanes: Vec<ModelLaneConfig>,
    ) -> iroha_config::parameters::actual::LaneConfig {
        let max_lane = lanes
            .iter()
            .map(|lane| lane.id.as_u32())
            .max()
            .unwrap_or_default();
        let lane_count =
            NonZeroU32::new(max_lane.saturating_add(1)).expect("lane count is non-zero");
        let catalog = LaneCatalog::new(lane_count, lanes).expect("lane catalog");
        iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog)
    }

    fn merkle_record(lane: u32) -> DaCommitmentRecord {
        DaCommitmentRecord::new(
            LaneId::new(lane),
            1,
            1,
            BlobDigest::new([0x11; 32]),
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0x22; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0x33; 32]),
            None,
            Some(Hash::prehashed([0x44; 32])),
            RetentionClass::default(),
            StorageTicketId::new([0x55; 32]),
            Signature::from_bytes(&[0x66; 64]),
        )
    }

    fn kzg_record(lane: u32, commitment: Option<KzgCommitment>) -> DaCommitmentRecord {
        DaCommitmentRecord::new(
            LaneId::new(lane),
            1,
            1,
            BlobDigest::new([0xAA; 32]),
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xBB; 32]),
            DaProofScheme::KzgBls12_381,
            Hash::prehashed([0xCC; 32]),
            commitment,
            Some(Hash::prehashed([0xDD; 32])),
            RetentionClass::default(),
            StorageTicketId::new([0xEE; 32]),
            Signature::from_bytes(&[0xFF; 64]),
        )
    }

    #[test]
    fn proof_policies_reflect_lane_config() {
        let lane_a = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "merkle-lane".to_string(),
            dataspace_id: DataSpaceId::new(7),
            proof_scheme: DaProofScheme::MerkleSha256,
            ..ModelLaneConfig::default()
        };

        let lane_b = ModelLaneConfig {
            id: LaneId::new(2),
            alias: "kzg-lane".to_string(),
            dataspace_id: DataSpaceId::new(9),
            proof_scheme: DaProofScheme::KzgBls12_381,
            ..ModelLaneConfig::default()
        };

        let lane_config = lane_config_with(vec![lane_a.clone(), lane_b.clone()]);
        let policies = proof_policies(&lane_config);

        assert_eq!(policies.len(), 2);
        assert_eq!(policies[0].lane_id, lane_a.id);
        assert_eq!(policies[0].dataspace_id, lane_a.dataspace_id);
        assert_eq!(policies[0].alias, lane_a.alias);
        assert_eq!(policies[0].proof_scheme, lane_a.proof_scheme);

        assert_eq!(policies[1].lane_id, lane_b.id);
        assert_eq!(policies[1].dataspace_id, lane_b.dataspace_id);
        assert_eq!(policies[1].alias, lane_b.alias);
        assert_eq!(policies[1].proof_scheme, lane_b.proof_scheme);
    }

    #[test]
    fn proof_policy_bundle_hash_matches_precomputed() {
        let config = lane_config_with(vec![ModelLaneConfig::default()]);
        let bundle = proof_policy_bundle(&config);
        let hash = proof_policy_bundle_hash(&config);

        let expected_hash = HashOf::new(&bundle);
        assert_eq!(hash, expected_hash);
        assert_eq!(bundle.version, DaProofPolicyBundle::VERSION_V1);

        let encoded = to_bytes(&bundle.policies).expect("encode policies");
        assert_eq!(bundle.policy_hash, Hash::new(encoded));
    }

    #[test]
    fn allows_merkle_lane_without_kzg_commitment() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let record = merkle_record(0);
        assert!(enforce_lane_proof_policy(&record, &lane_config).is_ok());
    }

    #[test]
    fn rejects_unknown_lane() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let record = merkle_record(2);
        let err = enforce_lane_proof_policy(&record, &lane_config).expect_err("should fail");
        assert!(matches!(err, DaProofPolicyError::UnknownLane { .. }));
    }

    #[test]
    fn rejects_scheme_mismatch() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let record = kzg_record(0, Some(KzgCommitment::new([1; 48])));
        let err = enforce_lane_proof_policy(&record, &lane_config).expect_err("should fail");
        assert!(matches!(err, DaProofPolicyError::SchemeMismatch { .. }));
    }

    #[test]
    fn rejects_kzg_lane_without_commitment() {
        let lanes = vec![ModelLaneConfig {
            id: LaneId::SINGLE,
            proof_scheme: DaProofScheme::KzgBls12_381,
            ..ModelLaneConfig::default()
        }];
        let lane_config = lane_config_with(lanes);
        let record = kzg_record(0, None);
        let err = enforce_lane_proof_policy(&record, &lane_config).expect_err("should fail");
        assert!(matches!(
            err,
            DaProofPolicyError::MissingKzgCommitment { .. }
        ));
    }

    #[test]
    fn rejects_zero_kzg_commitment() {
        let lanes = vec![ModelLaneConfig {
            id: LaneId::SINGLE,
            proof_scheme: DaProofScheme::KzgBls12_381,
            ..ModelLaneConfig::default()
        }];
        let lane_config = lane_config_with(lanes);
        let record = kzg_record(0, Some(KzgCommitment::zero()));
        let err = enforce_lane_proof_policy(&record, &lane_config).expect_err("should fail");
        assert!(matches!(err, DaProofPolicyError::ZeroKzgCommitment { .. }));
    }

    #[test]
    fn rejects_kzg_commitment_on_merkle_lane() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let record = DaCommitmentRecord {
            kzg_commitment: Some(KzgCommitment::new([9; 48])),
            ..merkle_record(0)
        };
        let err = enforce_lane_proof_policy(&record, &lane_config).expect_err("should fail");
        assert!(matches!(
            err,
            DaProofPolicyError::UnexpectedKzgCommitment { .. }
        ));
    }

    #[test]
    fn rejects_zero_chunk_root() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let record = DaCommitmentRecord {
            chunk_root: Hash::prehashed([0u8; 32]),
            ..merkle_record(0)
        };
        let err = enforce_lane_proof_policy(&record, &lane_config).expect_err("should fail");
        assert!(matches!(err, DaProofPolicyError::ZeroChunkRoot { .. }));
    }

    #[test]
    fn validate_commitment_bundle_rejects_duplicate_key() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let record = merkle_record(0);
        let duplicate = DaCommitmentRecord {
            acknowledgement_sig: Signature::from_bytes(&[0x99; 64]),
            ..record.clone()
        };
        let bundle = DaCommitmentBundle::new(vec![record, duplicate]);

        let err =
            validate_commitment_bundle(&bundle, &lane_config).expect_err("duplicate must fail");
        assert!(matches!(
            err,
            DaCommitmentValidationError::DuplicateCommitment { .. }
        ));
    }

    #[test]
    fn validate_commitment_bundle_rejects_duplicate_sequence_with_new_ticket() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let first = merkle_record(0);
        let mut second = first.clone();
        second.storage_ticket = StorageTicketId::new([0x99; 32]);
        second.manifest_hash =
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0x77; 32]);
        let bundle = DaCommitmentBundle::new(vec![first, second]);

        let err =
            validate_commitment_bundle(&bundle, &lane_config).expect_err("duplicate must fail");
        assert!(matches!(
            err,
            DaCommitmentValidationError::DuplicateCommitment { .. }
        ));
    }

    #[test]
    fn validate_commitment_bundle_rejects_duplicate_manifest() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let first = merkle_record(0);
        let mut second = merkle_record(0);
        second.sequence = 9;
        second.storage_ticket = StorageTicketId::new([0x99; 32]);
        let bundle = DaCommitmentBundle::new(vec![first.clone(), second.clone()]);

        let err =
            validate_commitment_bundle(&bundle, &lane_config).expect_err("duplicate must fail");
        assert!(matches!(
            err,
            DaCommitmentValidationError::DuplicateManifest { .. }
        ));
    }

    #[test]
    fn validate_commitment_bundle_rejects_duplicate_storage_ticket() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let first = merkle_record(0);
        let mut second = merkle_record(0);
        second.sequence = 9;
        second.manifest_hash =
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0x77; 32]);
        let bundle = DaCommitmentBundle::new(vec![first.clone(), second.clone()]);

        let err =
            validate_commitment_bundle(&bundle, &lane_config).expect_err("duplicate must fail");
        assert!(matches!(
            err,
            DaCommitmentValidationError::DuplicateStorageTicket { .. }
        ));
    }

    #[test]
    fn validate_commitment_bundle_rejects_zero_manifest_hash() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let mut record = merkle_record(0);
        record.manifest_hash = iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0; 32]);
        let bundle = DaCommitmentBundle::new(vec![record]);

        let err =
            validate_commitment_bundle(&bundle, &lane_config).expect_err("zero manifest must fail");
        assert!(matches!(
            err,
            DaCommitmentValidationError::ZeroManifestHash { .. }
        ));
    }

    #[test]
    fn validate_commitment_bundle_rejects_zero_storage_ticket() {
        let lane_config = lane_config_with(vec![ModelLaneConfig::default()]);
        let mut record = merkle_record(0);
        record.storage_ticket = StorageTicketId::new([0; 32]);
        let bundle = DaCommitmentBundle::new(vec![record]);

        let err =
            validate_commitment_bundle(&bundle, &lane_config).expect_err("zero ticket must fail");
        assert!(matches!(
            err,
            DaCommitmentValidationError::ZeroStorageTicket { .. }
        ));
    }

    #[test]
    fn validate_commitment_bundle_rejects_confidential_lane_without_policy() {
        let mut metadata = BTreeMap::new();
        metadata.insert("confidential_compute".to_string(), "true".to_string());
        metadata.insert(
            "confidential_mechanism".to_string(),
            "encryption".to_string(),
        );
        let lane_config = lane_config_with(vec![ModelLaneConfig {
            storage: LaneStorageProfile::SplitReplica,
            metadata,
            ..ModelLaneConfig::default()
        }]);
        let bundle = DaCommitmentBundle::new(vec![merkle_record(0)]);

        let err = validate_commitment_bundle(&bundle, &lane_config)
            .expect_err("confidential lane without key policy must fail");
        assert!(matches!(
            err,
            DaCommitmentValidationError::ConfidentialCompute(
                ConfidentialComputeError::MissingPolicy
            )
        ));
    }

    #[test]
    fn validate_commitment_bundle_len_rejects_unrepresentable_location_space() {
        assert!(validate_commitment_bundle_len(0).is_ok());
        assert!(validate_commitment_bundle_len(MAX_DA_BUNDLE_LOCATIONS).is_ok());

        if let Some(len) = MAX_DA_BUNDLE_LOCATIONS.checked_add(1) {
            let err = validate_commitment_bundle_len(len)
                .expect_err("unrepresentable DA bundle length must fail");
            assert_eq!(
                err,
                DaCommitmentValidationError::BundleTooLarge {
                    len,
                    max: MAX_DA_BUNDLE_LOCATIONS
                }
            );
        }
    }

    #[test]
    fn proof_policy_bundle_exposes_hash() {
        let lanes = vec![
            ModelLaneConfig {
                id: LaneId::new(1),
                alias: "lane-a".to_string(),
                ..ModelLaneConfig::default()
            },
            ModelLaneConfig {
                id: LaneId::new(2),
                alias: "lane-b".to_string(),
                proof_scheme: DaProofScheme::KzgBls12_381,
                ..ModelLaneConfig::default()
            },
        ];

        let config = lane_config_with(lanes);
        let bundle = proof_policy_bundle(&config);

        assert_eq!(bundle.policies.len(), 2);
        assert_ne!(bundle.policy_hash, Hash::prehashed([0; 32]));
        assert_eq!(bundle.version, DaProofPolicyBundle::VERSION_V1);
    }
}
