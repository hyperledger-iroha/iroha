use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    num::NonZeroU32,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use iroha_config::{
    base::WithOrigin,
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
        defaults::kura::{
            BLOCK_SYNC_ROSTER_RETENTION, BLOCKS_IN_MEMORY, FSYNC_INTERVAL, MAX_DISK_USAGE_BYTES,
            MERGE_LEDGER_CACHE_CAPACITY, ROSTER_SIDECAR_RETENTION,
        },
    },
};
use iroha_crypto::{Algorithm, KeyPair, MerkleTree, Signature, bls_normal_pop_prove};
use iroha_data_model::{
    Level,
    block::{
        BlockExecutionContextBundle, CertifiedMergeLedgerReference, SignedBlock,
        consensus::{
            CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1,
            LaneBlockProposalV1, NativeAmxAttestationBodyV2, NativeAmxAttestationQcV2,
            NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt, SumeragiLanePayloadOwnership,
        },
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, HeightContextId,
            NativeAmxApplicationManifestLeafV1, NativeAmxApplicationManifestMemberV1,
            PROTOCOL_VERSION, PayloadEncoding, QuorumCertificate, ValidatorPower,
            finality::V2FinalityArtifact,
        },
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    isi::Log,
    merge::{
        MergeExecutionBatch, MergeLaneBinding, MergeLaneExecution, MergeLaneSignerProof,
        MergeLedgerEntry, MergeQuorumCertificate,
    },
    nexus::{LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneLifecycleParameterV1},
    peer::PeerId,
    transaction::{TransactionBuilder, TransactionResult, signed::TransactionResultInner},
    trigger::DataTriggerSequence,
};
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use nonzero_ext::nonzero;
use tempfile::TempDir as RawTempDir;
use super::*;
use crate::{
    block::BlockBuilder,
    kura::{
        CertifiedLaneBlockArtifact, CommitManifest,
        NativeAmxParticipantApplicationManifestArtifactV1,
        NativeAmxParticipantApplicationReceiptArtifact,
    },
    lane_consensus::{
        CommittedLaneBlockSession, LaneBlockVoteV1, aggregate_lane_block_votes_to_qc,
    },
    tx::AcceptedTransaction,
};
// Keep the authenticated archive payload comfortably larger than the checkpoint sidecar.
// This makes the net disk-reclamation assertion independent of small encoding-size changes.
const GC_PAYLOAD_LEN: usize = 16 * 1024;
fn test_network_id(label: &[u8]) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(label),
    ))
}
