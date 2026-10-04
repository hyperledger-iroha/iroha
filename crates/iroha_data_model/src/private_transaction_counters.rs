//! Bounded private transaction counters and distinct committee computation attestations.
//!
//! These signatures attest an independently computed closed projection under the native BFT
//! trust model. They are not transaction inclusion, World membership, execution proofs or ZK
//! proofs. Core must authenticate the same certified policy/account originals, consume the signed
//! request nonce and compute all counts before its installed member key signs a response.

use std::num::NonZeroU64;

use iroha_crypto::{Hash, HashOf, KeyPair, Signature, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    AccountId, DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId,
    block::{BlockHeader, consensus::SumeragiRootScope},
    smart_contract::ContractAddress,
    sumeragi_finality::{SumeragiFinalityVerifier, VerifiedFinalityPage, VerifiedSumeragiBlock},
    transaction::{Executable, TransactionEntrypoint},
};

/// Sole current request domain; signing also binds the canonical typed payload.
pub const PRIVATE_COUNTER_REQUEST_DOMAIN_V1: [u8; 32] = *b"iroha.private-counter.request.v1";
/// Sole current member-attestation domain, distinct from consensus votes.
pub const PRIVATE_COUNTER_MEMBER_DOMAIN_V1: [u8; 32] = *b"iroha.private-counter.member.v1\0";
/// Sole fixed-authority policy metadata key; no caller key or retired layout is accepted.
pub const PRIVATE_COUNTER_POLICY_METADATA_KEY_V1: &str = "boiw_private_counters_policy_v1";
/// Sole fixed-authority manifest metadata key; both current producers publish this typed layout.
pub const PRIVATE_COUNTER_MANIFEST_METADATA_KEY_V1: &str = "boiw_private_counters_manifest_v1";
/// Largest original signed request or counters response frame.
pub const MAX_PRIVATE_COUNTER_FRAME_BYTES_V1: usize = 64 * 1024;
/// Largest policy frame.
pub const MAX_PRIVATE_COUNTER_POLICY_BYTES_V1: usize = 64 * 1024;
/// Largest sealed manifest frame.
pub const MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1: usize = 1024 * 1024;
/// Largest complete run entry set.
pub const MAX_PRIVATE_COUNTER_ENTRIES_V1: usize = 1024;
/// Largest admitted absolute certified cut and original carrier height.
pub const MAX_PRIVATE_COUNTER_HEIGHT_V1: u64 = 10_000;
/// Largest closed categorical result.
pub const MAX_PRIVATE_COUNTER_GROUPS_V1: usize = 256;
/// Largest source and retained original history allowance.
pub const MAX_PRIVATE_COUNTER_SOURCE_BYTES_V1: u64 = 256 * 1024 * 1024;
/// Largest accepted signed lifetime and member signature freshness.
pub const MAX_PRIVATE_COUNTER_LIFETIME_MS_V1: u64 = 60_000;
/// Largest independently installed clock skew allowance.
pub const MAX_PRIVATE_COUNTER_CLOCK_SKEW_MS_V1: u64 = 5_000;

/// A bounded refusal; private account, metadata, transaction or parser bodies are never formatted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrivateCountersErrorV1 {
    /// Canonical frame, field or allocation bound was exceeded.
    #[error("private counters bounds")]
    Bounds,
    /// Original frame is not the sole canonical layout.
    #[error("private counters codec")]
    Codec,
    /// Native context, selected policy, manifest or claim binding differs.
    #[error("private counters context")]
    Context,
    /// A signed request or member signature is invalid.
    #[error("private counters signature")]
    Signature,
    /// This first-release request owner only admits actual single-key readers.
    #[error("private counters unsupported multisig")]
    UnsupportedMultisig,
    /// The exact native account is not an admitted reader.
    #[error("private counters unauthorized")]
    Unauthorized,
    /// Native request or attestation time is outside its finite interval.
    #[error("private counters freshness")]
    Freshness,
    /// An authenticated one-shot request nonce was already consumed.
    #[error("private counters replay")]
    Replay,
    /// The installed member cannot reconstruct the exact admitted certified cut.
    #[error("private counters unavailable")]
    Unavailable,
    /// Distinct current members do not supply the native quorum.
    #[error("private counters quorum")]
    Quorum,
}

macro_rules! counter_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($(#[$vm:meta])* $variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode,
            IntoSchema, DeriveJsonSerialize, DeriveJsonDeserialize, norito::NoritoSchema)]
        #[norito(tag = "kind", content = "payload", deny_unknown_fields)]
        $(#[$meta])*
        pub enum $name { $($(#[$vm])* $variant),+ }
    };
}

counter_enum! {
    /// The two current supported producers; no generation fallback is admitted.
    #[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterPurposeV1")]
    pub enum CounterPurposeV1 {
        /// The complete selected fifteen-case interaction plan.
        WalkthroughInteractions,
        /// The exact current compiled connected-provider plan.
        ConnectedProvider,
    }
}
counter_enum! {
    /// Reader role derived from the fixed policy's native account ACL.
    #[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterReaderV1")]
    pub enum CounterReaderV1 {
        /// Current system operator.
        Operator,
        /// Current first PSP balance account.
        Psp1,
        /// Current second PSP balance account.
        Psp2,
    }
}
counter_enum! {
    /// Categorical party, never an account or wallet identifier.
    #[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterPartyV1")]
    pub enum CounterPartyV1 {
        /// No party dimension (including the interaction projection).
        None,
        /// First participating financial institution.
        BankA,
        /// Second participating financial institution.
        BankB,
        /// Government court-order role.
        Court,
        /// First participating payment service provider.
        Psp1,
        /// Second participating payment service provider.
        Psp2,
    }
}
counter_enum! {
    /// Closed transaction class.
    #[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterTransactionRoleV1")]
    pub enum CounterTransactionRoleV1 {
        /// Business movement.
        Business,
        /// Admitted control operation.
        Control,
    }
}
counter_enum! {
    /// Closed current movement/operation categories.
    #[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterCategoryV1")]
    pub enum CounterCategoryV1 {
        /// Availability control.
        Availability,
        /// Bank-link control.
        BankLink,
        /// Independently certified batch leg.
        BatchLeg,
        /// Refused business movement.
        Blocked,
        /// Connected-provider control operation.
        Control,
        /// Court-order control.
        CourtOrder,
        /// Pending escrow.
        EscrowPending,
        /// Facility draw.
        FacilityDraw,
        /// Issuance.
        Issuance,
        /// Native mint movement.
        Mint,
        /// One transaction with multiple admitted movement kinds.
        Mixed,
        /// Policy control.
        Policy,
        /// Release movement.
        Release,
        /// Reserve movement.
        Reserve,
        /// Native seizure movement.
        Seize,
        /// Interaction seizure category.
        Seizure,
        /// Transfer movement.
        Transfer,
        /// Wallet registration.
        WalletRegistration,
    }
}
counter_enum! {
    /// Actual native terminal status.
    #[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterResultV1")]
    pub enum CounterResultV1 {
        /// Native full Network output applied.
        Applied,
        /// Native full Network output rejected.
        Rejected,
    }
}
counter_enum! {
    /// Approved categorical rejection; Core must check the exact native nominal error identity.
    #[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterRejectionV1")]
    pub enum CounterRejectionV1 {
        /// No rejection, exactly for Applied.
        None,
        /// Below the financial-institution minimum.
        BelowMinimum,
        /// Missing capability.
        CapabilityNotRegistered,
        /// Holding limit exceeded.
        HoldingLimitExceeded,
        /// Incoming payments disabled.
        IncomingDisabled,
        /// Insufficient balance.
        InsufficientBalance,
        /// Explicit native permission refusal.
        NotPermitted,
        /// Wallet inactive.
        WalletInactive,
        /// Wallet limit exceeded.
        WalletLimitExceeded,
    }
}

/// Original run-binding preimages. External SHA-256 values remain exact 32-byte arrays;
/// they must never pass through `Hash::prehashed`, which changes a bit.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "kind", content = "payload", deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterRunBindingV1")]
pub enum CounterRunBindingV1 {
    /// Original interaction metadata identity.
    Interactions {
        /// Original selected run identifier, at most 96 ASCII bytes.
        run_id: String,
        /// Original selected run namespace, at most 96 ASCII bytes.
        run_namespace: String,
        /// Original selected definition identifier, at most 96 ASCII bytes.
        definition_id: String,
        /// Exact original definition SHA-256.
        definition_hash: [u8; 32],
        /// Exact original binding SHA-256.
        bindings_hash: [u8; 32],
    },
    /// Original connected-provider metadata identity.
    Connected {
        /// Original selected definition identifier, at most 96 ASCII bytes.
        definition_id: String,
        /// Exact original logical-baseline SHA-256.
        logical_baseline_hash: [u8; 32],
        /// Exact original session SHA-256.
        session_hash: [u8; 32],
        /// Exact original provider-generation SHA-256.
        provider_generation_hash: [u8; 32],
    },
}

/// Independently authenticated current cut; no incoming bare root grants authority.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterCutV1")]
pub struct CounterCutV1 {
    /// One-based finalized successor height.
    pub height: u64,
    /// Exact authenticated header identity.
    pub block_hash: HashOf<BlockHeader>,
    /// Exact authenticated consensus header and execution decision identity.
    pub context_id: Hash,
    /// Certified pre-tail World root.
    pub world_root: Hash,
    /// Complete authenticated current epoch-context identity.
    pub epoch_context_id: [u8; 32],
}

/// Finite policy ceilings; these are allowances, never measured-work receipts.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterLimitsV1")]
pub struct CounterLimitsV1 {
    /// Complete source entry allowance, at most 1024.
    pub max_entries: u16,
    /// Closed group allowance, at most 256.
    pub max_groups: u16,
    /// Maximum finalized-carrier work.
    pub max_carrier_work: u64,
    /// Maximum total projection work.
    pub max_total_work: u64,
    /// Maximum source bytes.
    pub max_source_bytes: u64,
    /// Maximum retained native original bytes.
    pub max_retained_bytes: u64,
    /// Maximum signed request lifetime.
    pub max_time_to_live_ms: u64,
    /// Independently installed finite native clock skew.
    pub max_clock_skew_ms: u64,
    /// Maximum age of a member computation signature.
    pub max_signature_age_ms: u64,
}

/// One exact released nominal contract error and its closed categorical interpretation.
///
/// The native invocation address/code identity and source-level rejection identity are separate
/// originals. Core must check both, plus the same-cut deployed instance; an error name alone
/// establishes neither contract authority nor the approved rejection category.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterContractErrorV1")]
pub struct CounterContractErrorV1 {
    /// Exact original native invocation/deployed-instance address.
    pub contract_address: ContractAddress,
    /// Exact source-level contract identity in the native rejection.
    pub contract: String,
    /// Exact nominal error type identity in the native rejection.
    pub error_type: String,
    /// Exact original declared variant schema commitment.
    pub schema_hash: [u8; 32],
    /// Exact original declared variant name, never matched without its nominal identity.
    pub name: String,
    /// Exact nonzero enum-local variant code.
    pub code: u32,
    /// Exact native invocation and same-cut deployed code identity.
    pub code_hash: Hash,
    /// Exact external SHA-256 pin of the independently released original artifact bytes.
    pub artifact_hash: [u8; 32],
    /// Sole categorical interpretation allowed for this exact complete original identity.
    pub rejection: CounterRejectionV1,
}

/// One original executable/authority expectation authored independently before submission.
///
/// The binding is owned by the fixed committed policy, never derived from a transaction receipt
/// or its caller-provided metadata. A valid signed `Log` cannot stand in for an approved payment
/// because Core checks the actual original native executable hash and authority before counting.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::private_transaction_counters::CounterExecutableBindingV1"
)]
pub struct CounterExecutableBindingV1 {
    /// Exact compiled nonzero selected action identifier, at most 1024.
    pub action_id: u16,
    /// Exact original native transaction authority expected for this action.
    pub authority: AccountId,
    /// Native hash of the complete originally approved executable, including ordered arguments.
    pub executable_hash: HashOf<Executable>,
}

/// Fixed-authority committed policy read by Core at the original certified cut.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::PrivateCountersPolicyV1")]
pub struct PrivateCountersPolicyV1 {
    /// Sole first-release layout, one.
    pub version: u16,
    /// Actual private-child genesis identity.
    pub network_id: NetworkId,
    /// Actual immutable private-root scope.
    pub scope: SumeragiRootScope,
    /// Exact supported producer.
    pub purpose: CounterPurposeV1,
    /// Canonical original run binding.
    pub run_binding: CounterRunBindingV1,
    /// Domain-separated canonical commitment of `run_binding`.
    pub run_id: Hash,
    /// Distinct actual single-key Operator, PSP1 and PSP2 reader accounts, in that order.
    pub readers: Vec<AccountId>,
    /// Canonically sorted distinct allowed original transaction authorities.
    pub authorities: Vec<AccountId>,
    /// Native canonical commitment to the compiled exact action/semantic plan.
    pub plan_hash: Hash,
    /// Complete independently authored original executable expectations, sorted by action ID.
    pub expected_executables: Vec<CounterExecutableBindingV1>,
    /// Sorted distinct exact current released contract SHA-256 pins.
    pub contracts: Vec<[u8; 32]>,
    /// Exact released nominal error bindings, sorted by complete nominal identity, at most 128.
    pub contract_errors: Vec<CounterContractErrorV1>,
    /// Earliest admitted original transaction/cut height.
    pub first_height: u64,
    /// Latest admitted original transaction/cut height.
    pub last_height: u64,
    /// Finite original-source, output and time allowances.
    pub limits: CounterLimitsV1,
}

/// One exact compiled action semantic tuple; Core owns the closed numeric action map.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterSemanticV1")]
pub struct CounterSemanticV1 {
    /// Nonzero compiled action identifier, at most 1024.
    pub action_id: u16,
    /// Current case identifier, one through fifteen.
    pub case_id: u16,
    /// Original compiled step, one through 1024.
    pub step: u16,
    /// Business or control.
    pub role: CounterTransactionRoleV1,
    /// Exact admitted transaction category.
    pub category: CounterCategoryV1,
    /// Exact admitted categorical party.
    pub party: CounterPartyV1,
    /// Intended terminal status; Core compares the original actual native output.
    pub result: CounterResultV1,
    /// Intended rejection category; Core compares exact native nominal identity.
    pub rejection: CounterRejectionV1,
    /// Exact ordered instruction movement categories, at most sixteen.
    pub instruction_movements: Vec<CounterCategoryV1>,
    /// Exact ordered corresponding instruction parties.
    pub instruction_parties: Vec<CounterPartyV1>,
}

/// One original manifest entry, never returned in aggregate display groups.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterManifestEntryV1")]
pub struct CounterManifestEntryV1 {
    /// Exact original native Network entry identity.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Exact original finalized carrier height.
    pub block_height: u64,
    /// Exact original native transaction authority.
    pub authority: AccountId,
    /// Complete compiled semantic tuple.
    pub semantic: CounterSemanticV1,
}

/// Complete sealed original run set; decoding alone grants no admission authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::PrivateCountersManifestV1")]
pub struct PrivateCountersManifestV1 {
    /// Sole first-release layout, one.
    pub version: u16,
    /// Actual private-child genesis identity.
    pub network_id: NetworkId,
    /// Actual immutable private-root scope.
    pub scope: SumeragiRootScope,
    /// Exact supported producer.
    pub purpose: CounterPurposeV1,
    /// Exact original run binding commitment.
    pub run_id: Hash,
    /// Exact independently selected policy commitment.
    pub policy_hash: Hash,
    /// Entire distinct original entry set, sorted by native entry hash.
    pub entries: Vec<CounterManifestEntryV1>,
}

/// Complete dedicated payload signed by an actual native single-key reader.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::PrivateCountersRequestV1")]
pub struct PrivateCountersRequestV1 {
    /// Exact dedicated domain, never a consensus or ordinary query signature domain.
    pub domain: [u8; 32],
    /// Sole first-release layout, one.
    pub version: u16,
    /// Actual independently selected private-child network.
    pub network_id: NetworkId,
    /// Actual independently selected immutable private-root scope.
    pub scope: SumeragiRootScope,
    /// Exact signing native reader identity.
    pub authority: AccountId,
    /// Exact supported producer.
    pub purpose: CounterPurposeV1,
    /// Independently selected fixed-authority policy commitment.
    pub policy_hash: Hash,
    /// Independently selected sealed run manifest commitment.
    pub manifest_hash: Hash,
    /// Independently authenticated current cut.
    pub cut: CounterCutV1,
    /// Native creation timestamp bound by the reader signature.
    pub creation_time_ms: u64,
    /// Nonzero finite signed request lifetime.
    pub time_to_live_ms: NonZeroU64,
    /// Original fresh unpredictable challenge, also a one-shot per-node replay nonce.
    pub nonce: [u8; 32],
}

/// Sole canonical original request envelope. Core must consume replay state after authentication.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::private_transaction_counters::SignedPrivateCountersRequestV1"
)]
pub struct SignedPrivateCountersRequestV1 {
    /// Complete original signature-bound payload.
    pub payload: PrivateCountersRequestV1,
    /// Exact native single-key reader signature over the domain-separated payload hash.
    pub signature: SignatureOf<PrivateCountersRequestV1>,
}

/// Closed group identity. It contains only approved categorical enums.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterGroupKeyV1")]
pub struct CounterGroupKeyV1 {
    /// Categorical party; interactions require None after native projection.
    pub party: CounterPartyV1,
    /// Business/control class.
    pub role: CounterTransactionRoleV1,
    /// Approved native categorical movement.
    pub category: CounterCategoryV1,
    /// Actual native terminal status.
    pub result: CounterResultV1,
    /// Actual native categorical rejection.
    pub rejection: CounterRejectionV1,
}

/// Native categorical transaction count; no row, identifier, amount or hash is included.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterGroupV1")]
pub struct CounterGroupV1 {
    /// Sole categorical group identity.
    pub key: CounterGroupKeyV1,
    /// Checked positive native transaction cardinality, counted once per original entry.
    pub count: u64,
}

/// Common statement computed independently by each installed member.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::PrivateCountersClaimV1")]
pub struct PrivateCountersClaimV1 {
    /// Sole first-release layout, one.
    pub version: u16,
    /// Actual independently selected private-child network.
    pub network_id: NetworkId,
    /// Actual independently selected immutable private-root scope.
    pub scope: SumeragiRootScope,
    /// Domain-separated hash of the full ORIGINAL canonical signed request frame.
    pub request_hash: Hash,
    /// Exact authenticated native reader.
    pub authority: AccountId,
    /// Native fixed-policy reader role.
    pub reader: CounterReaderV1,
    /// Exact supported producer.
    pub purpose: CounterPurposeV1,
    /// Exact independently selected fixed-authority policy commitment.
    pub policy_hash: Hash,
    /// Exact independently selected sealed manifest commitment.
    pub manifest_hash: Hash,
    /// Exact independently authenticated native cut.
    pub cut: CounterCutV1,
    /// Native certified block time shared by all computing members.
    pub certified_block_time_ms: u64,
    /// Exact original reader challenge/replay nonce.
    pub nonce: [u8; 32],
    /// Sorted distinct closed categorical transaction counts.
    pub groups: Vec<CounterGroupV1>,
}

/// Exact per-member signed envelope; member clocks never alter the common statement.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::CounterMemberBodyV1")]
pub struct CounterMemberBodyV1 {
    /// Exact dedicated computation domain.
    pub domain: [u8; 32],
    /// Sole first-release layout, one.
    pub version: u16,
    /// Domain-separated native hash of the entire common claim.
    pub claim_hash: Hash,
    /// Exact canonical index in the independently authenticated cut committee.
    pub member_index: u16,
    /// Actual native clock observed after same-cut computation and before signing.
    pub observed_at_ms: u64,
}

/// One genuine installed member computation signature.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::private_transaction_counters::CounterMemberAttestationV1"
)]
pub struct CounterMemberAttestationV1 {
    /// Original per-member envelope.
    pub body: CounterMemberBodyV1,
    /// Raw native installed BLS signature over the exact dedicated signing preimage.
    pub signature: Signature,
}

/// Canonical original one-node response. Collection never changes its claim or member signature.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::private_transaction_counters::PrivateCountersResponseV1")]
pub struct PrivateCountersResponseV1 {
    /// Entire common computation claim.
    pub claim: PrivateCountersClaimV1,
    /// Original installed member computation envelope and signature.
    pub attestation: CounterMemberAttestationV1,
}

/// Collected original member responses. Only the native verifier admits its quorum.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::private_transaction_counters::PrivateCountersCertificateV1"
)]
pub struct PrivateCountersCertificateV1 {
    /// Entire identical canonical common computation claim.
    pub claim: PrivateCountersClaimV1,
    /// Sorted distinct original member envelopes and signatures, at most 31.
    pub attestations: Vec<CounterMemberAttestationV1>,
}

/// Independent caller selections; never populated from the incoming counters response.
#[derive(Debug, Clone, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct PrivateCountersExpectedV1 {
    /// Exact installed private-child network.
    pub network_id: NetworkId,
    /// Exact installed immutable private scope.
    pub scope: SumeragiRootScope,
    /// Exact original request authority selected by the application.
    pub authority: AccountId,
    /// Exact supported producer.
    pub purpose: CounterPurposeV1,
    /// Independently selected committed fixed-authority policy hash.
    pub policy_hash: Hash,
    /// Independently selected sealed manifest hash.
    pub manifest_hash: Hash,
    /// Original challenge retained before sending any request.
    pub nonce: [u8; 32],
}

/// Opaque admitted computation attestation; it has no public codec or constructor.
#[derive(Debug, Clone)]
pub struct VerifiedPrivateCountersV1 {
    claim: PrivateCountersClaimV1,
    cut: VerifiedSumeragiBlock,
    member_indices: Vec<u16>,
}

fn private_scope(scope: SumeragiRootScope) -> Result<(), PrivateCountersErrorV1> {
    if !matches!(scope, SumeragiRootScope::Dataspace { .. }) || scope.validate().is_err() {
        return Err(PrivateCountersErrorV1::Context);
    }
    Ok(())
}
fn sorted_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn result_rejection(result: CounterResultV1, rejection: CounterRejectionV1) -> bool {
    (result == CounterResultV1::Applied) == (rejection == CounterRejectionV1::None)
}
fn original_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 96 && value.bytes().all(|byte| byte.is_ascii_graphic())
}
fn nominal_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && !value.chars().any(char::is_control)
        && !value.contains("__kotodama_link_")
}
fn digest(domain: &[u8], wire: &[u8]) -> Hash {
    Hash::new_from_chunks(&[domain, &(wire.len() as u64).to_le_bytes(), wire])
}
fn canonical<T: norito::SerializePayload + norito::NoritoSchema>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, PrivateCountersErrorV1> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let exact = norito::canonical_frame_len(value).map_err(|_| PrivateCountersErrorV1::Codec)?;
    if exact == 0 || exact > maximum {
        return Err(PrivateCountersErrorV1::Bounds);
    }
    norito::core::to_bytes_bounded(value, exact).map_err(|error| match error {
        norito::core::BoundedEncodeError::FrameTooLarge { .. }
        | norito::core::BoundedEncodeError::AllocationFailed { .. } => {
            PrivateCountersErrorV1::Bounds
        }
        norito::core::BoundedEncodeError::Serialization(_) => PrivateCountersErrorV1::Codec,
    })
}
fn frame_decode_limits(maximum: usize) -> norito::DecodeLimits {
    // The manifest has at most 1024 entries, each with at most sixteen paired instruction
    // categories. Strings, signatures and native account-key originals use the same finite
    // sequence ceiling; the cumulative limits are independent of advertised field lengths.
    let allocation = if maximum == MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1 {
        16 * 1024 * 1024
    } else {
        4 * 1024 * 1024
    };
    norito::DecodeLimits::new(1024, maximum, 131_072, allocation, 32)
}

impl CounterRunBindingV1 {
    /// Validate original finite run preimages and exact external digests.
    /// # Errors
    /// Invalid original label or absent external commitment.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        let valid = match self {
            Self::Interactions {
                run_id,
                run_namespace,
                definition_id,
                definition_hash,
                bindings_hash,
            } => {
                original_label(run_id)
                    && original_label(run_namespace)
                    && original_label(definition_id)
                    && *definition_hash != [0; 32]
                    && *bindings_hash != [0; 32]
            }
            Self::Connected {
                definition_id,
                logical_baseline_hash,
                session_hash,
                provider_generation_hash,
            } => {
                original_label(definition_id)
                    && *logical_baseline_hash != [0; 32]
                    && *session_hash != [0; 32]
                    && *provider_generation_hash != [0; 32]
            }
        };
        if !valid {
            return Err(PrivateCountersErrorV1::Bounds);
        }
        Ok(())
    }
    /// Exact current producer of this binding.
    #[must_use]
    pub const fn purpose(&self) -> CounterPurposeV1 {
        match self {
            Self::Interactions { .. } => CounterPurposeV1::WalkthroughInteractions,
            Self::Connected { .. } => CounterPurposeV1::ConnectedProvider,
        }
    }
    /// Domain-separated native canonical original run commitment.
    /// # Errors
    /// Invalid or oversized original binding.
    pub fn commitment(&self) -> Result<Hash, PrivateCountersErrorV1> {
        self.validate()?;
        Ok(digest(
            b"iroha/private-counters/run/v1\0",
            &canonical(self, 1024)?,
        ))
    }
}
impl CounterCutV1 {
    /// Validate finite original cut geometry without granting finality authority.
    /// # Errors
    /// Genesis-only, excessive height or absent original epoch context.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        if !(2..=MAX_PRIVATE_COUNTER_HEIGHT_V1).contains(&self.height)
            || self.epoch_context_id == [0; 32]
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        Ok(())
    }
    /// Derive all cut bindings from an already opaque authenticated native decision.
    /// # Errors
    /// Genesis-only decision or invalid authenticated epoch context.
    pub fn from_verified(cut: &VerifiedSumeragiBlock) -> Result<Self, PrivateCountersErrorV1> {
        let selected = Self {
            height: cut.height(),
            block_hash: cut.header().hash(),
            context_id: cut.context_id(),
            world_root: cut.execution().world_state_root,
            epoch_context_id: cut
                .commitment()
                .schedule
                .current
                .context_id()
                .map_err(|_| PrivateCountersErrorV1::Context)?,
        };
        selected.validate()?;
        Ok(selected)
    }
}
impl CounterLimitsV1 {
    /// Check every finite allowance before source work or allocation.
    /// # Errors
    /// Zero, overflowing or greater-than-release bounds.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        let max_work = crate::query::parameters::MAX_FETCH_SIZE.get();
        if self.max_entries == 0
            || usize::from(self.max_entries) > MAX_PRIVATE_COUNTER_ENTRIES_V1
            || self.max_groups == 0
            || usize::from(self.max_groups) > MAX_PRIVATE_COUNTER_GROUPS_V1
            || self.max_carrier_work == 0
            || self.max_carrier_work > max_work
            || self.max_total_work == 0
            || self.max_total_work > max_work
            || self.max_source_bytes == 0
            || self.max_source_bytes > MAX_PRIVATE_COUNTER_SOURCE_BYTES_V1
            || self.max_retained_bytes == 0
            || self.max_retained_bytes > MAX_PRIVATE_COUNTER_SOURCE_BYTES_V1
            || self.max_time_to_live_ms == 0
            || self.max_time_to_live_ms > MAX_PRIVATE_COUNTER_LIFETIME_MS_V1
            || self.max_clock_skew_ms > MAX_PRIVATE_COUNTER_CLOCK_SKEW_MS_V1
            || self.max_signature_age_ms == 0
            || self.max_signature_age_ms > MAX_PRIVATE_COUNTER_LIFETIME_MS_V1
        {
            return Err(PrivateCountersErrorV1::Bounds);
        }
        Ok(())
    }
}
impl CounterContractErrorV1 {
    fn nominal_key(&self) -> (&ContractAddress, &str, &str, &[u8; 32], &str, u32) {
        (
            &self.contract_address,
            &self.contract,
            &self.error_type,
            &self.schema_hash,
            &self.name,
            self.code,
        )
    }
    /// Validate bounded exact nominal identity and independent artifact pin geometry.
    /// Core must still authenticate the original invocation, deployment and native rejection.
    /// # Errors
    /// Missing, excessive or malformed identity/pin, zero code or success interpretation.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        if !nominal_identity(&self.contract)
            || !nominal_identity(&self.error_type)
            || !nominal_identity(&self.name)
            || self.code == 0
            || self.schema_hash == [0; 32]
            || self.artifact_hash == [0; 32]
            || self.rejection == CounterRejectionV1::None
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        Ok(())
    }
}
impl CounterExecutableBindingV1 {
    /// Validate finite selected-action geometry without admitting an executable or its authority.
    /// # Errors
    /// The selected action identifier is zero or exceeds the current finite release ceiling.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        if self.action_id == 0 || usize::from(self.action_id) > MAX_PRIVATE_COUNTER_ENTRIES_V1 {
            return Err(PrivateCountersErrorV1::Context);
        }
        Ok(())
    }
    /// Commit this entire original action/authority/executable tuple in its dedicated domain.
    /// The commitment grants no authority; the fixed policy owns the pre-submit expectation.
    /// # Errors
    /// Invalid action geometry, excessive original account frame or bounded codec failure.
    pub fn commitment(&self) -> Result<Hash, PrivateCountersErrorV1> {
        self.validate()?;
        Ok(digest(
            b"iroha/private-counters/executable/v1\0",
            &canonical(self, MAX_PRIVATE_COUNTER_POLICY_BYTES_V1)?,
        ))
    }
}
impl PrivateCountersPolicyV1 {
    /// Structural policy validation only; Core must reread the fixed authority at the same cut.
    /// # Errors
    /// Wrong scope/version/run binding, duplicate identities, or finite-bound violation.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        private_scope(self.scope)?;
        self.limits.validate()?;
        if self.version != 1
            || self.purpose != self.run_binding.purpose()
            || self.run_id != self.run_binding.commitment()?
            || self.first_height < 2
            || self.last_height < self.first_height
            || self.last_height > MAX_PRIVATE_COUNTER_HEIGHT_V1
            || self.readers.len() != 3
            || self.authorities.is_empty()
            || self.authorities.len() > 128
            || !sorted_unique(&self.authorities)
            || self.expected_executables.is_empty()
            || self.expected_executables.len() > usize::from(self.limits.max_entries)
            || self
                .expected_executables
                .windows(2)
                .any(|pair| pair[0].action_id >= pair[1].action_id)
            || self.contracts.is_empty()
            || self.contracts.len() > 16
            || !sorted_unique(&self.contracts)
            || self.contracts.iter().any(|pin| *pin == [0; 32])
            || self.contract_errors.len() > 128
            || self
                .contract_errors
                .windows(2)
                .any(|pair| pair[0].nominal_key() >= pair[1].nominal_key())
            || self
                .readers
                .iter()
                .enumerate()
                .any(|(index, id)| self.readers[..index].contains(id))
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        if self.readers.iter().any(|id| id.try_signatory().is_none()) {
            return Err(PrivateCountersErrorV1::UnsupportedMultisig);
        }
        for error in &self.contract_errors {
            error.validate()?;
            if self.contracts.binary_search(&error.artifact_hash).is_err() {
                return Err(PrivateCountersErrorV1::Context);
            }
        }
        for binding in &self.expected_executables {
            binding.validate()?;
            if self.authorities.binary_search(&binding.authority).is_err() {
                return Err(PrivateCountersErrorV1::Context);
            }
        }
        Ok(())
    }
    /// Derive the exact reader role from the current fixed policy's native identity ACL.
    /// # Errors
    /// Invalid policy or account absent from that ACL.
    pub fn reader(&self, authority: &AccountId) -> Result<CounterReaderV1, PrivateCountersErrorV1> {
        self.validate()?;
        match self.readers.iter().position(|id| id == authority) {
            Some(0) => Ok(CounterReaderV1::Operator),
            Some(1) => Ok(CounterReaderV1::Psp1),
            Some(2) => Ok(CounterReaderV1::Psp2),
            _ => Err(PrivateCountersErrorV1::Unauthorized),
        }
    }
    /// Exact native policy commitment, independently selected before receiving a response.
    /// # Errors
    /// Invalid policy or noncanonical encoding.
    pub fn commitment(&self) -> Result<Hash, PrivateCountersErrorV1> {
        self.validate()?;
        Ok(digest(
            b"iroha/private-counters/policy/v1\0",
            &canonical(self, MAX_PRIVATE_COUNTER_POLICY_BYTES_V1)?,
        ))
    }
}
impl CounterSemanticV1 {
    /// Validate finite structural semantic fields; this does not admit a compiled plan.
    /// # Errors
    /// Invalid numeric action geometry, result/rejection or instruction pairing.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        if self.action_id == 0
            || self.action_id > 1024
            || !(1..=15).contains(&self.case_id)
            || self.step == 0
            || self.step > 1024
            || !result_rejection(self.result, self.rejection)
            || self.instruction_movements.len() > 16
            || self.instruction_movements.len() != self.instruction_parties.len()
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        Ok(())
    }
}

/// Commit the entire exact selected semantic plan and its original executable/authority bindings.
///
/// This helper supplies no plan authority: Core must derive the same complete selected list from
/// its installed current compiled action map. Manifest order is by original entry hash and cannot
/// replace that semantic selection. A changed role, party, result, rejection, instruction order or
/// selected action, transaction authority or complete executable changes the commitment.
/// Bindings must be independently authored before submission, never reconstructed from receipts.
/// # Errors
/// Empty, excessive, duplicate/reordered or structurally invalid semantic descriptors.
pub fn counter_plan_commitment_v1(
    semantics: &[CounterSemanticV1],
    bindings: &[CounterExecutableBindingV1],
) -> Result<Hash, PrivateCountersErrorV1> {
    if semantics.is_empty()
        || semantics.len() > MAX_PRIVATE_COUNTER_ENTRIES_V1
        || semantics
            .windows(2)
            .any(|pair| pair[0].action_id >= pair[1].action_id)
        || semantics.len() != bindings.len()
        || semantics
            .iter()
            .zip(bindings)
            .any(|(semantic, binding)| semantic.action_id != binding.action_id)
    {
        return Err(PrivateCountersErrorV1::Context);
    }
    for semantic in semantics {
        semantic.validate()?;
    }
    for binding in bindings {
        binding.validate()?;
    }
    let mut original_plan = Vec::new();
    original_plan
        .try_reserve_exact(semantics.len())
        .map_err(|_| PrivateCountersErrorV1::Bounds)?;
    for semantic in semantics {
        let mut movements = Vec::new();
        movements
            .try_reserve_exact(semantic.instruction_movements.len())
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        movements.extend_from_slice(&semantic.instruction_movements);
        let mut parties = Vec::new();
        parties
            .try_reserve_exact(semantic.instruction_parties.len())
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        parties.extend_from_slice(&semantic.instruction_parties);
        original_plan.push(CounterSemanticV1 {
            action_id: semantic.action_id,
            case_id: semantic.case_id,
            step: semantic.step,
            role: semantic.role,
            category: semantic.category,
            party: semantic.party,
            result: semantic.result,
            rejection: semantic.rejection,
            instruction_movements: movements,
            instruction_parties: parties,
        });
    }
    // Commit borrowed native account originals rather than cloning an AccountId without the
    // independently prepaid retained-owner charges required by its admission clone API.
    let mut original_executables = Vec::new();
    original_executables
        .try_reserve_exact(bindings.len())
        .map_err(|_| PrivateCountersErrorV1::Bounds)?;
    let mut original_bytes = 0_usize;
    for binding in bindings {
        original_bytes = original_bytes
            .checked_add(
                norito::canonical_frame_len(binding).map_err(|_| PrivateCountersErrorV1::Codec)?,
            )
            .filter(|bytes| *bytes <= MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1)
            .ok_or(PrivateCountersErrorV1::Bounds)?;
        original_executables.push(binding.commitment()?);
    }
    Ok(digest(
        b"iroha/private-counters/plan/v1\0",
        &canonical(
            &(original_plan, original_executables),
            MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1,
        )?,
    ))
}
impl PrivateCountersManifestV1 {
    /// Validate the complete distinct original set; admission still requires native plan/originals.
    /// # Errors
    /// Wrong scope/version, missing/duplicate/reordered entries, or malformed original binding.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        private_scope(self.scope)?;
        if self.version != 1
            || self.entries.is_empty()
            || self.entries.len() > MAX_PRIVATE_COUNTER_ENTRIES_V1
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].entrypoint_hash >= pair[1].entrypoint_hash)
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        for entry in &self.entries {
            if !(2..=MAX_PRIVATE_COUNTER_HEIGHT_V1).contains(&entry.block_height) {
                return Err(PrivateCountersErrorV1::Context);
            }
            entry.semantic.validate()?;
        }
        Ok(())
    }
    /// Compare every structural original binding with the exact selected committed policy.
    /// # Errors
    /// Foreign policy/run/root, unavailable authority, or height/entry ceiling violation.
    pub fn validate_against_policy(
        &self,
        policy: &PrivateCountersPolicyV1,
    ) -> Result<(), PrivateCountersErrorV1> {
        self.validate()?;
        policy.validate()?;
        if self.network_id != policy.network_id
            || self.scope != policy.scope
            || self.purpose != policy.purpose
            || self.run_id != policy.run_id
            || self.policy_hash != policy.commitment()?
            || self.entries.len() != policy.expected_executables.len()
            || self.entries.iter().any(|entry| {
                entry.block_height < policy.first_height
                    || entry.block_height > policy.last_height
                    || policy.authorities.binary_search(&entry.authority).is_err()
            })
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        let mut selected_actions = [false; MAX_PRIVATE_COUNTER_ENTRIES_V1 + 1];
        for entry in &self.entries {
            let action = usize::from(entry.semantic.action_id);
            if selected_actions[action] {
                return Err(PrivateCountersErrorV1::Context);
            }
            selected_actions[action] = true;
            let index = policy
                .expected_executables
                .binary_search_by_key(&entry.semantic.action_id, |binding| binding.action_id)
                .map_err(|_| PrivateCountersErrorV1::Context)?;
            if policy.expected_executables[index].authority != entry.authority {
                return Err(PrivateCountersErrorV1::Context);
            }
        }
        Ok(())
    }
    /// Domain-separated canonical complete manifest commitment.
    /// # Errors
    /// Invalid or oversized original manifest.
    pub fn commitment(&self) -> Result<Hash, PrivateCountersErrorV1> {
        self.validate()?;
        Ok(digest(
            b"iroha/private-counters/manifest/v1\0",
            &canonical(self, MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1)?,
        ))
    }
}
impl PrivateCountersRequestV1 {
    /// Verify structural request bounds without trusting its caller-selected context.
    /// # Errors
    /// Invalid domain/version/scope, absent challenge, expired interval geometry or unsupported reader.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        private_scope(self.scope)?;
        self.cut.validate()?;
        if self.domain != PRIVATE_COUNTER_REQUEST_DOMAIN_V1
            || self.version != 1
            || self.nonce == [0; 32]
            || self.creation_time_ms == 0
            || self.time_to_live_ms.get() > MAX_PRIVATE_COUNTER_LIFETIME_MS_V1
            || self
                .creation_time_ms
                .checked_add(self.time_to_live_ms.get())
                .is_none()
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        if self.authority.try_signatory().is_none() {
            return Err(PrivateCountersErrorV1::UnsupportedMultisig);
        }
        Ok(())
    }
    /// Validate exact fixed-policy context and native time before signature or history work.
    /// This does not consume the required per-node replay nonce.
    /// # Errors
    /// Foreign or unapproved context/reader, excessive TTL, future or expired native time.
    pub fn validate_at(
        &self,
        policy: &PrivateCountersPolicyV1,
        now_ms: u64,
    ) -> Result<CounterReaderV1, PrivateCountersErrorV1> {
        self.validate()?;
        policy.validate()?;
        if self.network_id != policy.network_id
            || self.scope != policy.scope
            || self.purpose != policy.purpose
            || self.policy_hash != policy.commitment()?
            || self.cut.height < policy.first_height
            || self.cut.height > policy.last_height
            || self.time_to_live_ms.get() > policy.limits.max_time_to_live_ms
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        if now_ms == 0
            || self.creation_time_ms > now_ms.saturating_add(policy.limits.max_clock_skew_ms)
            || now_ms
                > self
                    .creation_time_ms
                    .checked_add(self.time_to_live_ms.get())
                    .ok_or(PrivateCountersErrorV1::Freshness)?
                    .saturating_add(policy.limits.max_clock_skew_ms)
        {
            return Err(PrivateCountersErrorV1::Freshness);
        }
        policy.reader(&self.authority)
    }
    /// Dedicated native signing hash of the complete canonical payload.
    /// # Errors
    /// Invalid or oversized payload.
    pub fn signing_hash(&self) -> Result<HashOf<Self>, PrivateCountersErrorV1> {
        self.validate()?;
        Ok(HashOf::from_untyped_unchecked(digest(
            b"iroha/private-counters/request-signature/v1\0",
            &canonical(self, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1)?,
        )))
    }
    /// Sign the sole request layout with its exact native reader key.
    /// # Errors
    /// Key differs from the actual single-key authority or signing fails.
    pub fn try_sign(
        self,
        key: &KeyPair,
    ) -> Result<SignedPrivateCountersRequestV1, PrivateCountersErrorV1> {
        if self.authority.try_signatory() != Some(key.public_key()) {
            return Err(PrivateCountersErrorV1::Unauthorized);
        }
        let signature = SignatureOf::try_from_hash(key.private_key(), self.signing_hash()?)
            .map_err(|_| PrivateCountersErrorV1::Signature)?;
        Ok(SignedPrivateCountersRequestV1 {
            payload: self,
            signature,
        })
    }
}
impl SignedPrivateCountersRequestV1 {
    /// Structural envelope validation; decode never authenticates the reader.
    /// # Errors
    /// Invalid signature-bound payload geometry.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        self.payload.validate()
    }
    /// Authenticate the actual native single-key reader over the complete signed payload.
    /// Core must also check account originals, fixed policy/time and consume its nonce before work.
    /// # Errors
    /// Unsupported controller or invalid signature.
    pub fn verify_signature(&self) -> Result<(), PrivateCountersErrorV1> {
        self.validate()?;
        let key = self
            .payload
            .authority
            .try_signatory()
            .ok_or(PrivateCountersErrorV1::UnsupportedMultisig)?;
        self.signature
            .verify_hash(key, self.payload.signing_hash()?)
            .map_err(|_| PrivateCountersErrorV1::Signature)
    }
    /// Hash this entire original canonical envelope, including the reader signature.
    /// # Errors
    /// Invalid or oversized original request.
    pub fn original_hash(&self) -> Result<Hash, PrivateCountersErrorV1> {
        Ok(digest(
            b"iroha/private-counters/original-request/v1\0",
            &self.encode_canonical()?,
        ))
    }
}
impl PrivateCountersClaimV1 {
    /// Check the closed categorical projection and all finite structural bindings.
    /// # Errors
    /// Invalid scope/status/group geometry, repeated groups or overflow.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        private_scope(self.scope)?;
        self.cut.validate()?;
        if self.version != 1
            || self.nonce == [0; 32]
            || self.certified_block_time_ms == 0
            || self.groups.len() > MAX_PRIVATE_COUNTER_GROUPS_V1
            || self
                .groups
                .windows(2)
                .any(|pair| pair[0].key >= pair[1].key)
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        let mut total = 0_u64;
        for group in &self.groups {
            if group.count == 0
                || !result_rejection(group.key.result, group.key.rejection)
                || (self.purpose == CounterPurposeV1::WalkthroughInteractions
                    && group.key.party != CounterPartyV1::None)
            {
                return Err(PrivateCountersErrorV1::Context);
            }
            total = total
                .checked_add(group.count)
                .filter(|total| *total <= MAX_PRIVATE_COUNTER_ENTRIES_V1 as u64)
                .ok_or(PrivateCountersErrorV1::Bounds)?;
        }
        Ok(())
    }
    /// Dedicated canonical common claim commitment.
    /// # Errors
    /// Invalid or oversized categorical claim.
    pub fn commitment(&self) -> Result<Hash, PrivateCountersErrorV1> {
        self.validate()?;
        Ok(digest(
            b"iroha/private-counters/claim/v1\0",
            &canonical(self, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1)?,
        ))
    }
}
impl CounterMemberBodyV1 {
    /// Dedicated raw native signing preimage, distinct from every consensus signature preimage.
    /// # Errors
    /// Invalid domain/version/member/time or canonical encoding.
    pub fn signing_preimage(&self) -> Result<Vec<u8>, PrivateCountersErrorV1> {
        if self.domain != PRIVATE_COUNTER_MEMBER_DOMAIN_V1
            || self.version != 1
            || self.member_index >= 31
            || self.observed_at_ms == 0
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        let wire = canonical(self, 1024)?;
        let mut preimage = b"iroha/private-counters/member-signature/v1\0".to_vec();
        preimage.extend_from_slice(&(wire.len() as u64).to_le_bytes());
        preimage.extend_from_slice(&wire);
        Ok(preimage)
    }
}
impl PrivateCountersResponseV1 {
    /// Check one original node response structurally without admitting signer custody.
    /// # Errors
    /// Wrong domain, changed common statement or malformed signature geometry.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        self.claim.validate()?;
        self.attestation.body.signing_preimage()?;
        if self.attestation.body.claim_hash != self.claim.commitment()?
            || self.attestation.signature.payload().len() != 96
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        Ok(())
    }
}
impl PrivateCountersCertificateV1 {
    /// Structural response validation only; it never establishes committee custody or quorum.
    /// # Errors
    /// Empty/duplicate/reordered/oversized attestations or changed common claim.
    pub fn validate(&self) -> Result<(), PrivateCountersErrorV1> {
        self.claim.validate()?;
        let hash = self.claim.commitment()?;
        if self.attestations.is_empty()
            || self.attestations.len() > 31
            || self
                .attestations
                .windows(2)
                .any(|pair| pair[0].body.member_index >= pair[1].body.member_index)
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        for attestation in &self.attestations {
            attestation.body.signing_preimage()?;
            if attestation.body.claim_hash != hash || attestation.signature.payload().len() != 96 {
                return Err(PrivateCountersErrorV1::Context);
            }
        }
        Ok(())
    }
}

macro_rules! bounded_frame {
    ($ty:ty, $maximum:expr) => {
        impl $ty {
            /// Encode only the sole structurally valid canonical bounded native frame.
            /// # Errors
            /// Invalid type geometry, codec failure or excessive original frame.
            pub fn encode_canonical(&self) -> Result<Vec<u8>, PrivateCountersErrorV1> {
                self.validate()?;
                canonical(self, $maximum)
            }
            /// Decode exact original bytes under finite native allocation/frame limits.
            /// This is structural only; it neither authenticates nor admits anything.
            /// # Errors
            /// Invalid, truncated, noncanonical, trailing or oversized original frame.
            pub fn decode_bounded_canonical(wire: &[u8]) -> Result<Self, PrivateCountersErrorV1> {
                if wire.is_empty() || wire.len() > $maximum {
                    return Err(PrivateCountersErrorV1::Bounds);
                }
                let value: Self =
                    norito::decode_canonical_with_limits(wire, frame_decode_limits($maximum))
                        .map_err(|_| PrivateCountersErrorV1::Codec)?;
                value.validate()?;
                Ok(value)
            }
        }
    };
}
bounded_frame!(PrivateCountersPolicyV1, MAX_PRIVATE_COUNTER_POLICY_BYTES_V1);
bounded_frame!(
    PrivateCountersManifestV1,
    MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1
);
bounded_frame!(
    SignedPrivateCountersRequestV1,
    MAX_PRIVATE_COUNTER_FRAME_BYTES_V1
);
bounded_frame!(
    PrivateCountersResponseV1,
    MAX_PRIVATE_COUNTER_FRAME_BYTES_V1
);
bounded_frame!(
    PrivateCountersCertificateV1,
    MAX_PRIVATE_COUNTER_FRAME_BYTES_V1
);

/// Join original identical node claims without computing or changing any count.
/// The result remains untrusted until `verify_private_counters_v1` admits its native quorum.
/// # Errors
/// Too many originals, noncanonical frames, different common claims or duplicate members.
pub fn collect_private_counters_v1(
    original_responses: &[Vec<u8>],
) -> Result<Vec<u8>, PrivateCountersErrorV1> {
    if original_responses.is_empty() || original_responses.len() > 31 {
        return Err(PrivateCountersErrorV1::Bounds);
    }
    let mut collected: Option<PrivateCountersCertificateV1> = None;
    for original in original_responses {
        let response = PrivateCountersResponseV1::decode_bounded_canonical(original)?;
        match &mut collected {
            None => {
                collected = Some(PrivateCountersCertificateV1 {
                    claim: response.claim,
                    attestations: vec![response.attestation],
                })
            }
            Some(current) => {
                if current.claim != response.claim || current.attestations.len() >= 31 {
                    return Err(PrivateCountersErrorV1::Context);
                }
                current.attestations.push(response.attestation);
            }
        }
    }
    let mut collected = collected.ok_or(PrivateCountersErrorV1::Bounds)?;
    collected
        .attestations
        .sort_by_key(|item| item.body.member_index);
    collected.encode_canonical()
}

/// Verify a distinct native committee computation claim under independently installed finality.
///
/// `page` must already authenticate the original contiguous prefix under an independently installed
/// checkpoint. Its opaque tip selects the cut; incoming counter rosters or bare hashes cannot do so.
/// `expected` and the original policy commitment must be selected before receiving a response.
/// Core's same-cut reread/computation is certified by the honest native quorum, not by a fabricated
/// World inclusion proof. Request replay consumption remains a node admission responsibility.
/// # Errors
/// Any changed original/signature/policy/root/cut/request/time/group/member/quorum binding.
pub fn verify_private_counters_v1(
    original_request: &[u8],
    original_response: &[u8],
    original_policy: &[u8],
    page: &VerifiedFinalityPage,
    expected: &PrivateCountersExpectedV1,
    now_ms: u64,
) -> Result<VerifiedPrivateCountersV1, PrivateCountersErrorV1> {
    private_scope(expected.scope)?;
    let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
        page.checkpoint(),
        &expected.network_id,
        page.checkpoint().chain_id(),
    )
    .map_err(|_| PrivateCountersErrorV1::Context)?;
    if verifier
        .root_scope()
        .map_err(|_| PrivateCountersErrorV1::Context)?
        != expected.scope
    {
        return Err(PrivateCountersErrorV1::Context);
    }
    let cut = page.tip().clone();
    let native_cut = CounterCutV1::from_verified(&cut)?;
    if cut.commitment().schedule.current.network_id != expected.network_id {
        return Err(PrivateCountersErrorV1::Context);
    }
    let policy = PrivateCountersPolicyV1::decode_bounded_canonical(original_policy)?;
    if policy.commitment()? != expected.policy_hash {
        return Err(PrivateCountersErrorV1::Context);
    }
    let request = SignedPrivateCountersRequestV1::decode_bounded_canonical(original_request)?;
    let payload = &request.payload;
    if payload.network_id != expected.network_id
        || payload.scope != expected.scope
        || payload.authority != expected.authority
        || payload.purpose != expected.purpose
        || payload.policy_hash != expected.policy_hash
        || payload.manifest_hash != expected.manifest_hash
        || payload.nonce != expected.nonce
        || payload.cut != native_cut
    {
        return Err(PrivateCountersErrorV1::Context);
    }
    let reader = payload.validate_at(&policy, now_ms)?;
    request.verify_signature()?;
    let response = PrivateCountersCertificateV1::decode_bounded_canonical(original_response)?;
    let claim = &response.claim;
    // Hash the exact retained ORIGINAL envelope; canonical decoding forbids another spelling.
    let request_hash = digest(
        b"iroha/private-counters/original-request/v1\0",
        original_request,
    );
    if claim.network_id != expected.network_id
        || claim.scope != expected.scope
        || claim.authority != expected.authority
        || claim.reader != reader
        || claim.purpose != expected.purpose
        || claim.policy_hash != expected.policy_hash
        || claim.manifest_hash != expected.manifest_hash
        || claim.nonce != expected.nonce
        || claim.cut != native_cut
        || claim.request_hash != request_hash
        || claim.certified_block_time_ms != cut.header().creation_time_ms
        || claim.groups.len() > usize::from(policy.limits.max_groups)
        || claim
            .groups
            .iter()
            .try_fold(0_u64, |total, group| total.checked_add(group.count))
            .is_none_or(|total| total > u64::from(policy.limits.max_entries))
    {
        return Err(PrivateCountersErrorV1::Context);
    }
    let members = &cut.commitment().schedule.current.committee;
    crate::sumeragi::epoch::validate_committee(members)
        .map_err(|_| PrivateCountersErrorV1::Context)?;
    if response.attestations.len() < iroha_sumeragi::types::quorum(members.len()) {
        return Err(PrivateCountersErrorV1::Quorum);
    }
    let mut member_indices = Vec::with_capacity(response.attestations.len());
    for attestation in &response.attestations {
        let body = &attestation.body;
        let member = members
            .get(usize::from(body.member_index))
            .ok_or(PrivateCountersErrorV1::Context)?;
        if body.observed_at_ms > now_ms.saturating_add(policy.limits.max_clock_skew_ms)
            || now_ms
                > body
                    .observed_at_ms
                    .saturating_add(policy.limits.max_signature_age_ms)
            || body.observed_at_ms
                < claim
                    .certified_block_time_ms
                    .saturating_sub(policy.limits.max_clock_skew_ms)
        {
            return Err(PrivateCountersErrorV1::Freshness);
        }
        payload.validate_at(&policy, body.observed_at_ms)?;
        attestation
            .signature
            .verify(member.validator.public_key(), &body.signing_preimage()?)
            .map_err(|_| PrivateCountersErrorV1::Signature)?;
        member_indices.push(body.member_index);
    }
    Ok(VerifiedPrivateCountersV1 {
        claim: response.claim,
        cut,
        member_indices,
    })
}
impl VerifiedPrivateCountersV1 {
    /// Closed native verified aggregate projection and its separate evidence bindings.
    #[must_use]
    pub fn claim(&self) -> &PrivateCountersClaimV1 {
        &self.claim
    }
    /// Independently authenticated original native cut.
    #[must_use]
    pub fn cut(&self) -> &VerifiedSumeragiBlock {
        &self.cut
    }
    /// Exact distinct authenticated installed member indexes.
    #[must_use]
    pub fn member_indices(&self) -> &[u16] {
        &self.member_indices
    }
}

#[cfg(test)]
mod tests;
