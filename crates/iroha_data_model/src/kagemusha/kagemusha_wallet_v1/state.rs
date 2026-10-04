//! Private wallet state, transition statements, proofs, commit receipts and packages
//! (§§3.1–3.6, design §3 with C1, C3 and C5).
//!
//! The private state is committed by the paired Pasta commitment of the recursive state owner;
//! this module fixes its fields, the field order of its map leaves, the public transition
//! statement and the provider receipt that certifies one committed transition. A package
//! `(statement, π, τ)` is consumed only after its receipt verifies natively under the
//! credential's payment key (§3.1); a proof alone is never a transferable credit.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1, KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_VERSION_V1, WalletResult, WalletVersionsV1,
    digest::{
        KagemushaWalletDigestRoleV1 as Role, KagemushaWalletSignerOutputV1, WalletTranscriptV1,
        kagemusha_wallet_digest_v1, kagemusha_wallet_freeze_signature_v1,
        kagemusha_wallet_preimage_v1, kagemusha_wallet_signed_object_digest_v1,
        kagemusha_wallet_verify_signature_v1,
    },
    identity::{
        KAGEMUSHA_WALLET_CONTROL_ATTESTATION_LEASE_V1, KAGEMUSHA_WALLET_CONTROL_BLACKLIST_V1,
        KAGEMUSHA_WALLET_CONTROL_QUOTAS_V1, KagemushaWalletCredentialV1,
        KagemushaWalletRegulatoryPolicyV1, KagemushaWalletSchemeV1,
    },
    invalid_v1, is_zero_v1, overflow_v1,
    policy::KagemushaWalletQuotaWindowKindV1,
    require_nonzero_v1, require_scheme_v1, require_version_v1,
};
use crate::kagemusha::KagemushaDeviceSignatureV1;

#[cfg(test)]
#[path = "state_tests.rs"]
pub(super) mod state_tests;

const DIGEST_BYTES: usize = 32;
const U64_BYTES: usize = 8;
const U128_BYTES: usize = 16;

/// Exact inline transcript bytes of one paired state commitment: `eq || ep`.
pub const KAGEMUSHA_WALLET_COMMITMENT_TRANSCRIPT_BYTES_V1: usize = 2 * DIGEST_BYTES;

const BOOTSTRAP_EFFECT_FIELDS_BYTES: usize = 2 * DIGEST_BYTES;
const LOAD_EFFECT_FIELDS_BYTES: usize = DIGEST_BYTES + 3 * U128_BYTES;
const SEND_EFFECT_FIELDS_BYTES: usize = 4 * DIGEST_BYTES + 3 * U128_BYTES + 2 * U64_BYTES;
const RECEIVE_EFFECT_FIELDS_BYTES: usize = 3 * DIGEST_BYTES + U128_BYTES;
const ARCHIVE_SENT_EFFECT_FIELDS_BYTES: usize = 2 * DIGEST_BYTES;
const UNLOAD_EFFECT_FIELDS_BYTES: usize = 2 * DIGEST_BYTES + 3 * U128_BYTES;
const REFRESH_POLICY_EFFECT_FIELDS_BYTES: usize = 1 + DIGEST_BYTES + U64_BYTES;
const RETIRING_EFFECT_FIELDS_BYTES: usize = 0;

/// Fixed field widths of every effect variant, in tag order.
const EFFECT_FIELDS_BYTES: [usize; 8] = [
    BOOTSTRAP_EFFECT_FIELDS_BYTES,
    LOAD_EFFECT_FIELDS_BYTES,
    SEND_EFFECT_FIELDS_BYTES,
    RECEIVE_EFFECT_FIELDS_BYTES,
    ARCHIVE_SENT_EFFECT_FIELDS_BYTES,
    UNLOAD_EFFECT_FIELDS_BYTES,
    REFRESH_POLICY_EFFECT_FIELDS_BYTES,
    RETIRING_EFFECT_FIELDS_BYTES,
];

const fn max_width_v1(widths: &[usize]) -> usize {
    let mut max = 0;
    let mut index = 0;
    while index < widths.len() {
        if widths[index] > max {
            max = widths[index];
        }
        index += 1;
    }
    max
}

/// Zero-filled union width of every effect transcript: the largest variant (Send).
pub const KAGEMUSHA_WALLET_EFFECT_UNION_BYTES_V1: usize = max_width_v1(&EFFECT_FIELDS_BYTES);
/// Exact effect transcript bytes: one tag byte followed by the zero-filled union.
pub const KAGEMUSHA_WALLET_EFFECT_TRANSCRIPT_BYTES_V1: usize =
    1 + KAGEMUSHA_WALLET_EFFECT_UNION_BYTES_V1;
/// Exact `statement` transcript bytes.
pub const KAGEMUSHA_WALLET_STATEMENT_TRANSCRIPT_BYTES_V1: usize = 2
    + 4 * DIGEST_BYTES
    + 1
    + 2 * U128_BYTES
    + 2 * KAGEMUSHA_WALLET_COMMITMENT_TRANSCRIPT_BYTES_V1
    + KAGEMUSHA_WALLET_EFFECT_TRANSCRIPT_BYTES_V1;
/// Exact `receipt-body` transcript bytes.
pub const KAGEMUSHA_WALLET_RECEIPT_BODY_TRANSCRIPT_BYTES_V1: usize = 2
    + 3 * DIGEST_BYTES
    + U128_BYTES
    + DIGEST_BYTES
    + 2 * KAGEMUSHA_WALLET_COMMITMENT_TRANSCRIPT_BYTES_V1
    + 3 * DIGEST_BYTES;
/// Exact `operation-id` transcript bytes: `wallet_id || u8 kind || input`.
pub const KAGEMUSHA_WALLET_OPERATION_ID_TRANSCRIPT_BYTES_V1: usize =
    DIGEST_BYTES + 1 + DIGEST_BYTES;
/// Exact `package` transcript bytes: `statement_digest || proof_digest || receipt_digest`.
pub const KAGEMUSHA_WALLET_PACKAGE_TRANSCRIPT_BYTES_V1: usize = 3 * DIGEST_BYTES;
/// Exact `unload-nullifier` transcript bytes: `scheme_id || wallet_id || LE128 ordinal`.
pub const KAGEMUSHA_WALLET_UNLOAD_NULLIFIER_TRANSCRIPT_BYTES_V1: usize =
    2 * DIGEST_BYTES + U128_BYTES;

// Map-leaf Poseidon domains (design C3): the data model fixes field order, the limb rule and
// one distinct domain per map.
// TODO(G3): the iroha_core_zk map owner hashes the leaves under these domains and publishes the
// Poseidon leaf vectors.
/// Poseidon replay domain of consumed-credit leaves (permanent map).
pub const KAGEMUSHA_WALLET_CONSUMED_CREDIT_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwccrd1");
/// Poseidon replay domain of pending-outgoing leaves.
pub const KAGEMUSHA_WALLET_PENDING_OUTGOING_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwpout1");
/// Poseidon replay domain of load-recovery leaves.
pub const KAGEMUSHA_WALLET_LOAD_RECOVERY_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwload1");
/// Poseidon replay domain of redeem-recovery leaves.
pub const KAGEMUSHA_WALLET_REDEEM_RECOVERY_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwrdm_1");
/// Poseidon replay domain of fee-claim leaves.
pub const KAGEMUSHA_WALLET_FEE_CLAIM_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwfee_1");
/// Poseidon replay domain of quota-usage leaves.
pub const KAGEMUSHA_WALLET_QUOTA_USAGE_LEAF_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwquse1");

// ---------------------------------------------------------------------------------------
// Lifecycle, commitment and private state (§3.1)
// ---------------------------------------------------------------------------------------

/// Lifecycle of one wallet incarnation; it never reverts from `Retiring` (§6.3).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletLifecycleV1"
)]
pub enum KagemushaWalletLifecycleV1 {
    /// Normal operation.
    #[codec(index = 1)]
    Active,
    /// New setup and funding closed; existing claims are preserved.
    #[codec(index = 2)]
    Retiring,
}

impl KagemushaWalletLifecycleV1 {
    /// Every lifecycle, in tag order.
    pub const ALL: [Self; 2] = [Self::Active, Self::Retiring];

    /// Transcript tag; equal to the Norito wire tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Active => 1,
            Self::Retiring => 2,
        }
    }
}

/// Paired Pasta state commitment (`Eq`, `Ep`).
///
/// Canonical field encodings are checked by the recursive verifier; the data model has no
/// Pasta dependency. The all-zero value is the Bootstrap predecessor.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Decode, Encode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletStateCommitmentV1"
)]
pub struct KagemushaWalletStateCommitmentV1 {
    /// Commitment in the `Eq` field.
    pub eq: [u8; 32],
    /// Commitment in the `Ep` field.
    pub ep: [u8; 32],
}

impl KagemushaWalletStateCommitmentV1 {
    /// The all-zero commitment, used only as the Bootstrap predecessor.
    pub const ZERO: Self = Self {
        eq: [0; 32],
        ep: [0; 32],
    };

    /// Whether both halves are zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    /// Whether both halves are nonzero.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.eq != [0; 32] && self.ep != [0; 32]
    }

    /// Append the inline transcript.
    fn write(&self, transcript: WalletTranscriptV1) -> WalletTranscriptV1 {
        transcript.digest(&self.eq).digest(&self.ep)
    }

    /// Exact inline transcript bytes `eq || ep`.
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        self.write(WalletTranscriptV1::with_capacity(
            KAGEMUSHA_WALLET_COMMITMENT_TRANSCRIPT_BYTES_V1,
        ))
        .finish()
    }
}

/// Empty-map roots of the recursive map owner, used by the Bootstrap state (§3.1).
// TODO(G3): the iroha_core_zk map owner publishes these constants with its Poseidon domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaWalletEmptyMapRootsV1 {
    /// Empty consumed-credit root.
    pub consumed_credit: [u8; 32],
    /// Empty pending-outgoing root.
    pub pending_outgoing: [u8; 32],
    /// Empty load-recovery root.
    pub load_recovery: [u8; 32],
    /// Empty redeem-recovery root.
    pub redeem_recovery: [u8; 32],
    /// Empty fee-claim root.
    pub fee_claim: [u8; 32],
    /// Empty quota-usage root.
    pub quota_usage: [u8; 32],
}

impl KagemushaWalletEmptyMapRootsV1 {
    /// Validate that every empty root is a real (nonzero) map-owner constant.
    ///
    /// # Errors
    ///
    /// Rejects an all-zero root.
    pub fn validate(&self) -> WalletResult<()> {
        for (field, root) in [
            ("empty_roots.consumed_credit", &self.consumed_credit),
            ("empty_roots.pending_outgoing", &self.pending_outgoing),
            ("empty_roots.load_recovery", &self.load_recovery),
            ("empty_roots.redeem_recovery", &self.redeem_recovery),
            ("empty_roots.fee_claim", &self.fee_claim),
            ("empty_roots.quota_usage", &self.quota_usage),
        ] {
            require_nonzero_v1(field, root)?;
        }
        Ok(())
    }
}

/// Policy part of the private state (§§3.1, 7; design C6).
///
/// An all-zero digest means "none held". `enabled_controls` stores the controls that are
/// both enabled by the held scheme policy and permitted by the credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPolicyStateV1"
)]
pub struct KagemushaWalletPolicyStateV1 {
    /// Regulatory policy of the credential; byte-identical across renewals.
    pub regulatory_policy: KagemushaWalletRegulatoryPolicyV1,
    /// Digest of the held scheme policy; zero when none is held.
    pub scheme_policy: [u8; 32],
    /// Epoch of the held scheme policy; zero when none is held.
    pub policy_epoch: u64,
    /// Active controls: scheme policy enabled and credential permitted.
    pub enabled_controls: u32,
    /// Fee schedule named by the held scheme policy; zero for no fee.
    pub fee_schedule: [u8; 32],
    /// Digest of the held blacklist; zero when none is held.
    pub blacklist: [u8; 32],
    /// Version of the held blacklist; zero when none is held.
    pub blacklist_version: u64,
    /// Gap-tree root of the held blacklist.
    pub blacklist_root: [u8; 32],
    /// Issuance time of the held blacklist in Unix milliseconds.
    pub blacklist_issued_at_ms: u64,
    /// Digest of the held quota share; zero when none is held.
    pub quota_share: [u8; 32],
    /// Identity of the held quota share; zero when none is held.
    pub quota_share_id: u64,
    /// Windows root of the held quota share.
    pub quota_windows_root: [u8; 32],
    /// Root of the persistent quota-usage map; `RefreshPolicy` never changes it.
    pub quota_usage_root: [u8; 32],
    /// Attestation lease expiry of the current credential; zero when not permitted.
    pub lease_expires_at_ms: u64,
    /// Accepted time floor in Unix milliseconds; it never decreases.
    pub accepted_time_floor_ms: u64,
    /// Digest of the committed time anchor; zero when none is committed.
    pub time_anchor: [u8; 32],
}

impl KagemushaWalletPolicyStateV1 {
    /// Bootstrap policy state: the credential's regulatory policy and lease, the map owner's
    /// empty quota-usage root, and every other field zero (design C6).
    ///
    /// # Errors
    ///
    /// Rejects an invalid credential or a zero usage root.
    pub fn bootstrap(
        credential: &KagemushaWalletCredentialV1,
        empty_quota_usage_root: &[u8; 32],
    ) -> WalletResult<Self> {
        credential.validate()?;
        let policy = Self {
            regulatory_policy: credential.body.regulatory_policy,
            scheme_policy: [0; 32],
            policy_epoch: 0,
            enabled_controls: 0,
            fee_schedule: [0; 32],
            blacklist: [0; 32],
            blacklist_version: 0,
            blacklist_root: [0; 32],
            blacklist_issued_at_ms: 0,
            quota_share: [0; 32],
            quota_share_id: 0,
            quota_windows_root: [0; 32],
            quota_usage_root: *empty_quota_usage_root,
            lease_expires_at_ms: credential.body.lease_expires_at_ms,
            accepted_time_floor_ms: 0,
            time_anchor: [0; 32],
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Whether every bit of `control` is active.
    #[must_use]
    pub const fn is_active(&self, control: u32) -> bool {
        control != 0 && self.enabled_controls & control == control
    }

    /// Whether Send needs a same-boot anchored interval: an active quota or lease control, or
    /// an enforced blacklist with a list-age rule (design C6).
    #[must_use]
    pub const fn send_requires_time_anchor(&self) -> bool {
        self.is_active(KAGEMUSHA_WALLET_CONTROL_QUOTAS_V1)
            || self.is_active(KAGEMUSHA_WALLET_CONTROL_ATTESTATION_LEASE_V1)
            || (self.is_active(KAGEMUSHA_WALLET_CONTROL_BLACKLIST_V1)
                && self.blacklist_version > 0
                && self.regulatory_policy.blacklist_max_age_ms > 0)
    }

    /// Validate the policy state's consistency rules.
    ///
    /// # Errors
    ///
    /// Rejects an invalid regulatory policy, active controls the credential does not permit,
    /// held-object fields that disagree about whether the object is held, a zero usage root,
    /// or a lease that disagrees with the regulatory policy.
    pub fn validate(&self) -> WalletResult<()> {
        self.regulatory_policy.validate()?;
        if self.enabled_controls & !self.regulatory_policy.permitted_controls != 0 {
            return Err(invalid_v1("policy.enabled_controls"));
        }
        let scheme_policy_held = self.policy_epoch != 0;
        if scheme_policy_held == is_zero_v1(&self.scheme_policy)
            || (!scheme_policy_held
                && (self.enabled_controls != 0 || !is_zero_v1(&self.fee_schedule)))
        {
            return Err(invalid_v1("policy.scheme_policy"));
        }
        let blacklist_held = self.blacklist_version != 0;
        if blacklist_held == is_zero_v1(&self.blacklist)
            || blacklist_held == is_zero_v1(&self.blacklist_root)
            || (!blacklist_held && self.blacklist_issued_at_ms != 0)
        {
            return Err(invalid_v1("policy.blacklist"));
        }
        let share_held = self.quota_share_id != 0;
        if share_held == is_zero_v1(&self.quota_share)
            || share_held == is_zero_v1(&self.quota_windows_root)
        {
            return Err(invalid_v1("policy.quota_share"));
        }
        require_nonzero_v1("policy.quota_usage_root", &self.quota_usage_root)?;
        let lease_permitted = self
            .regulatory_policy
            .permits(KAGEMUSHA_WALLET_CONTROL_ATTESTATION_LEASE_V1);
        if (self.lease_expires_at_ms != 0) != lease_permitted {
            return Err(invalid_v1("policy.lease_expires_at_ms"));
        }
        Ok(())
    }
}

/// Private state of one wallet incarnation (§3.1).
///
/// Map roots authenticate local maps whose openings and retained objects live beside the
/// state; a root is not a backup of its map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletStateV1")]
pub struct KagemushaWalletStateV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Enrolled scheme.
    pub scheme_id: [u8; 32],
    /// Enrolled asset scope digest.
    pub asset_digest: [u8; 32],
    /// Wallet incarnation identity.
    pub wallet_id: [u8; 32],
    /// Digest of the current credential.
    pub credential_digest: [u8; 32],
    /// Lifecycle.
    pub lifecycle: KagemushaWalletLifecycleV1,
    /// Spendable balance in integer asset units.
    pub balance: u128,
    /// Transition sequence; zero at Bootstrap.
    pub sequence: u128,
    /// Next payer send ordinal.
    pub next_send: u128,
    /// Next load voucher ordinal.
    pub next_load: u128,
    /// Next unload redemption ordinal.
    pub next_redeem: u128,
    /// Permanent consumed-credit map root.
    pub consumed_credit_root: [u8; 32],
    /// Pending-outgoing map root.
    pub pending_outgoing_root: [u8; 32],
    /// Load-recovery map root.
    pub load_recovery_root: [u8; 32],
    /// Redeem-recovery map root.
    pub redeem_recovery_root: [u8; 32],
    /// Fee-claim map root.
    pub fee_claim_root: [u8; 32],
    /// Policy state.
    pub policy: KagemushaWalletPolicyStateV1,
    /// Fresh state nonce hiding the commitment.
    pub state_nonce: [u8; 32],
}

impl KagemushaWalletStateV1 {
    /// Bootstrap state of a newly enrolled incarnation: zero balance, ordinals and sequence,
    /// empty maps, and the Bootstrap policy state (§3.1).
    ///
    /// # Errors
    ///
    /// Rejects an invalid credential, zero empty roots, or a zero state nonce.
    pub fn bootstrap(
        credential: &KagemushaWalletCredentialV1,
        empty_roots: &KagemushaWalletEmptyMapRootsV1,
        state_nonce: [u8; 32],
    ) -> WalletResult<Self> {
        empty_roots.validate()?;
        let policy = KagemushaWalletPolicyStateV1::bootstrap(credential, &empty_roots.quota_usage)?;
        let state = Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: credential.body.scheme_id,
            asset_digest: credential.body.asset_digest,
            wallet_id: credential.body.wallet_id,
            credential_digest: credential.credential_digest(),
            lifecycle: KagemushaWalletLifecycleV1::Active,
            balance: 0,
            sequence: 0,
            next_send: 0,
            next_load: 0,
            next_redeem: 0,
            consumed_credit_root: empty_roots.consumed_credit,
            pending_outgoing_root: empty_roots.pending_outgoing,
            load_recovery_root: empty_roots.load_recovery,
            redeem_recovery_root: empty_roots.redeem_recovery,
            fee_claim_root: empty_roots.fee_claim,
            policy,
            state_nonce,
        };
        state.validate()?;
        Ok(state)
    }

    /// Validate the state's self-contained rules.
    ///
    /// # Errors
    ///
    /// Rejects another version, zero identities, roots or nonce, and an invalid policy state.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("state.version", self.version)?;
        for (field, digest) in [
            ("state.scheme_id", &self.scheme_id),
            ("state.asset_digest", &self.asset_digest),
            ("state.wallet_id", &self.wallet_id),
            ("state.credential_digest", &self.credential_digest),
            ("state.consumed_credit_root", &self.consumed_credit_root),
            ("state.pending_outgoing_root", &self.pending_outgoing_root),
            ("state.load_recovery_root", &self.load_recovery_root),
            ("state.redeem_recovery_root", &self.redeem_recovery_root),
            ("state.fee_claim_root", &self.fee_claim_root),
            ("state.state_nonce", &self.state_nonce),
        ] {
            require_nonzero_v1(field, digest)?;
        }
        self.policy.validate()
    }

    /// Validate that the state belongs to `credential`.
    ///
    /// # Errors
    ///
    /// Rejects an invalid state or credential, another scheme, asset, wallet or credential,
    /// or a regulatory policy or lease that differs from the credential's.
    pub fn validate_for_credential(
        &self,
        credential: &KagemushaWalletCredentialV1,
    ) -> WalletResult<()> {
        self.validate()?;
        credential.validate()?;
        let body = &credential.body;
        require_scheme_v1("state.scheme_id", &self.scheme_id, &body.scheme_id)?;
        if self.asset_digest != body.asset_digest {
            return Err(invalid_v1("state.asset_digest"));
        }
        if self.wallet_id != body.wallet_id {
            return Err(invalid_v1("state.wallet_id"));
        }
        if self.credential_digest != credential.credential_digest() {
            return Err(invalid_v1("state.credential_digest"));
        }
        if self.policy.regulatory_policy != body.regulatory_policy {
            return Err(invalid_v1("policy.regulatory_policy"));
        }
        if self.policy.lease_expires_at_ms != body.lease_expires_at_ms {
            return Err(invalid_v1("policy.lease_expires_at_ms"));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------
// Map leaves (§3.2, design C3)
// ---------------------------------------------------------------------------------------

/// Split a 256-bit digest into its two little-endian 128-bit limbs, low half first.
///
/// This is the data-model statement of the map owner's `digest_limbs` rule (design C3); the
/// owner injects each limb into a Pasta field with `from_u128`.
#[must_use]
pub fn kagemusha_wallet_digest_limbs_v1(digest: &[u8; 32]) -> [u128; 2] {
    let mut low = [0_u8; 16];
    let mut high = [0_u8; 16];
    low.copy_from_slice(&digest[..16]);
    high.copy_from_slice(&digest[16..]);
    [u128::from_le_bytes(low), u128::from_le_bytes(high)]
}

/// Permanent consumed-credit leaf: `credit_id, payment_digest` (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletConsumedCreditLeafV1"
)]
pub struct KagemushaWalletConsumedCreditLeafV1 {
    /// Received credit identity (the map key).
    pub credit_id: [u8; 32],
    /// Digest of the full canonical Payment that credited it.
    pub payment_digest: [u8; 32],
}

impl KagemushaWalletConsumedCreditLeafV1 {
    /// Poseidon replay domain of this map.
    pub const DOMAIN: u64 = KAGEMUSHA_WALLET_CONSUMED_CREDIT_LEAF_DOMAIN_V1;

    /// Field-order 128-bit limbs (design C3).
    #[must_use]
    pub fn limbs(&self) -> [u128; 4] {
        let [credit_low, credit_high] = kagemusha_wallet_digest_limbs_v1(&self.credit_id);
        let [payment_low, payment_high] = kagemusha_wallet_digest_limbs_v1(&self.payment_digest);
        [credit_low, credit_high, payment_low, payment_high]
    }
}

/// Pending-outgoing leaf of one committed Send awaiting `ArchiveSent` (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPendingOutgoingLeafV1"
)]
pub struct KagemushaWalletPendingOutgoingLeafV1 {
    /// Credit identity (the map key).
    pub credit_id: [u8; 32],
    /// Bound receiver wallet.
    pub receiver_wallet_id: [u8; 32],
    /// Payer send ordinal consumed by the Send.
    pub send_ordinal: u128,
    /// Amount credited to the receiver.
    pub amount: u128,
    /// Fee earned at the Send commit.
    pub fee: u128,
    /// Digest of the signed Request.
    pub request_digest: [u8; 32],
}

impl KagemushaWalletPendingOutgoingLeafV1 {
    /// Poseidon replay domain of this map.
    pub const DOMAIN: u64 = KAGEMUSHA_WALLET_PENDING_OUTGOING_LEAF_DOMAIN_V1;

    /// Field-order 128-bit limbs (design C3).
    #[must_use]
    pub fn limbs(&self) -> [u128; 9] {
        let [credit_low, credit_high] = kagemusha_wallet_digest_limbs_v1(&self.credit_id);
        let [receiver_low, receiver_high] =
            kagemusha_wallet_digest_limbs_v1(&self.receiver_wallet_id);
        let [request_low, request_high] = kagemusha_wallet_digest_limbs_v1(&self.request_digest);
        [
            credit_low,
            credit_high,
            receiver_low,
            receiver_high,
            self.send_ordinal,
            self.amount,
            self.fee,
            request_low,
            request_high,
        ]
    }
}

/// Load-recovery leaf of one absorbed voucher (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletLoadLeafV1"
)]
pub struct KagemushaWalletLoadLeafV1 {
    /// Load ordinal (the map key).
    pub ordinal: u128,
    /// Digest of the absorbed voucher.
    pub voucher_digest: [u8; 32],
    /// Net offline amount added.
    pub amount: u128,
}

impl KagemushaWalletLoadLeafV1 {
    /// Poseidon replay domain of this map.
    pub const DOMAIN: u64 = KAGEMUSHA_WALLET_LOAD_RECOVERY_LEAF_DOMAIN_V1;

    /// Field-order 128-bit limbs (design C3).
    #[must_use]
    pub fn limbs(&self) -> [u128; 4] {
        let [voucher_low, voucher_high] = kagemusha_wallet_digest_limbs_v1(&self.voucher_digest);
        [self.ordinal, voucher_low, voucher_high, self.amount]
    }
}

/// Redeem-recovery leaf of one unload claim (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletRedeemLeafV1"
)]
pub struct KagemushaWalletRedeemLeafV1 {
    /// Redemption ordinal (the map key).
    pub ordinal: u128,
    /// Unload nullifier.
    pub nullifier: [u8; 32],
    /// Net offline amount subtracted.
    pub amount: u128,
    /// Online charge withheld from the payout.
    pub online_charge: u128,
}

impl KagemushaWalletRedeemLeafV1 {
    /// Poseidon replay domain of this map.
    pub const DOMAIN: u64 = KAGEMUSHA_WALLET_REDEEM_RECOVERY_LEAF_DOMAIN_V1;

    /// Field-order 128-bit limbs (design C3).
    #[must_use]
    pub fn limbs(&self) -> [u128; 5] {
        let [nullifier_low, nullifier_high] = kagemusha_wallet_digest_limbs_v1(&self.nullifier);
        [
            self.ordinal,
            nullifier_low,
            nullifier_high,
            self.amount,
            self.online_charge,
        ]
    }
}

/// Fee-claim leaf of one nonzero Send fee awaiting payout (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletFeeClaimLeafV1"
)]
pub struct KagemushaWalletFeeClaimLeafV1 {
    /// Credit identity (the map key).
    pub credit_id: [u8; 32],
    /// Fee earned at the Send commit.
    pub fee: u128,
    /// Digest of the historical fee schedule.
    pub fee_schedule_digest: [u8; 32],
}

impl KagemushaWalletFeeClaimLeafV1 {
    /// Poseidon replay domain of this map.
    pub const DOMAIN: u64 = KAGEMUSHA_WALLET_FEE_CLAIM_LEAF_DOMAIN_V1;

    /// Field-order 128-bit limbs (design C3).
    #[must_use]
    pub fn limbs(&self) -> [u128; 5] {
        let [credit_low, credit_high] = kagemusha_wallet_digest_limbs_v1(&self.credit_id);
        let [schedule_low, schedule_high] =
            kagemusha_wallet_digest_limbs_v1(&self.fee_schedule_digest);
        [
            credit_low,
            credit_high,
            self.fee,
            schedule_low,
            schedule_high,
        ]
    }
}

/// Persistent quota-usage leaf keyed by `(window_kind, window_start_ms)` (§3.2, design C6).
///
/// Its window kind is limbed as its `u8` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletQuotaUsageLeafV1"
)]
pub struct KagemushaWalletQuotaUsageLeafV1 {
    /// Window kind (key part).
    pub window_kind: KagemushaWalletQuotaWindowKindV1,
    /// Window start in Unix milliseconds (key part).
    pub window_start_ms: u64,
    /// Window end in Unix milliseconds; later shares keep it for this key.
    pub window_end_ms: u64,
    /// Gross amount consumed in this window; never replenished.
    pub used: u128,
}

impl KagemushaWalletQuotaUsageLeafV1 {
    /// Poseidon replay domain of this map.
    pub const DOMAIN: u64 = KAGEMUSHA_WALLET_QUOTA_USAGE_LEAF_DOMAIN_V1;

    /// Field-order 128-bit limbs (design C3).
    #[must_use]
    pub fn limbs(&self) -> [u128; 4] {
        [
            u128::from(self.window_kind.tag()),
            u128::from(self.window_start_ms),
            u128::from(self.window_end_ms),
            self.used,
        ]
    }
}

// ---------------------------------------------------------------------------------------
// Operations, effects and statements (§3.3, §3.6)
// ---------------------------------------------------------------------------------------

/// Kind of one proven state transition (§3.3).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletOperationKindV1"
)]
pub enum KagemushaWalletOperationKindV1 {
    /// Install the zero state of a newly enrolled incarnation.
    #[codec(index = 1)]
    Bootstrap,
    /// Absorb the next finalized load voucher.
    #[codec(index = 2)]
    Load,
    /// Irreversibly debit a payment to a bound receiver.
    #[codec(index = 3)]
    Send,
    /// Credit a received Payment once.
    #[codec(index = 4)]
    Receive,
    /// Remove a delivered pending-outgoing descriptor.
    #[codec(index = 5)]
    ArchiveSent,
    /// Create a ledger-directed redemption claim.
    #[codec(index = 6)]
    Unload,
    /// Apply one authenticated policy update.
    #[codec(index = 7)]
    RefreshPolicy,
    /// Close new setup and funding.
    #[codec(index = 8)]
    Retiring,
}

impl KagemushaWalletOperationKindV1 {
    /// Every operation kind, in tag order.
    pub const ALL: [Self; 8] = [
        Self::Bootstrap,
        Self::Load,
        Self::Send,
        Self::Receive,
        Self::ArchiveSent,
        Self::Unload,
        Self::RefreshPolicy,
        Self::Retiring,
    ];

    /// Transcript tag; equal to the Norito wire tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Bootstrap => 1,
            Self::Load => 2,
            Self::Send => 3,
            Self::Receive => 4,
            Self::ArchiveSent => 5,
            Self::Unload => 6,
            Self::RefreshPolicy => 7,
            Self::Retiring => 8,
        }
    }
}

/// Kind of the signed object applied by one `RefreshPolicy` transition (design C1).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPolicyUpdateKindV1"
)]
pub enum KagemushaWalletPolicyUpdateKindV1 {
    /// Replacement credential (lease renewal).
    #[codec(index = 1)]
    Credential,
    /// Scheme policy.
    #[codec(index = 2)]
    SchemePolicy,
    /// Blacklist.
    #[codec(index = 3)]
    Blacklist,
    /// Quota share.
    #[codec(index = 4)]
    QuotaShare,
    /// Time anchor.
    #[codec(index = 5)]
    TimeAnchor,
}

impl KagemushaWalletPolicyUpdateKindV1 {
    /// Every update kind, in tag order.
    pub const ALL: [Self; 5] = [
        Self::Credential,
        Self::SchemePolicy,
        Self::Blacklist,
        Self::QuotaShare,
        Self::TimeAnchor,
    ];

    /// Transcript tag; equal to the Norito wire tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Credential => 1,
            Self::SchemePolicy => 2,
            Self::Blacklist => 3,
            Self::QuotaShare => 4,
            Self::TimeAnchor => 5,
        }
    }
}

/// Public effect of one transition; its tag equals the operation kind (design C1).
///
/// The transcript is the tag followed by the variant's fixed fields and zero fill to
/// [`KAGEMUSHA_WALLET_EFFECT_UNION_BYTES_V1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletEffectV1")]
pub enum KagemushaWalletEffectV1 {
    /// Install the zero state from the generation-0 enrollment marker.
    #[codec(index = 1)]
    Bootstrap {
        /// Enrollment identity of the credential.
        enrollment_id: [u8; 32],
        /// `marker_digest` of the generation-0 Enrollment marker (design C3).
        enrollment_marker: [u8; 32],
    },
    /// Absorb the voucher at the predecessor's `next_load`.
    #[codec(index = 2)]
    Load {
        /// Voucher digest.
        voucher: [u8; 32],
        /// Consumed load ordinal.
        load_ordinal: u128,
        /// Net offline value added to the balance.
        amount: u128,
        /// Online charge paid on the ledger; zero without a charge quote.
        online_charge: u128,
    },
    /// Irreversibly debit `amount + fee` to the bound receiver.
    #[codec(index = 3)]
    Send {
        /// Credit identity of the Request.
        credit_id: [u8; 32],
        /// Bound receiver wallet.
        receiver_wallet_id: [u8; 32],
        /// Consumed payer send ordinal.
        send_ordinal: u128,
        /// Amount credited to the receiver; positive.
        amount: u128,
        /// Fee earned at this commit.
        fee: u128,
        /// Request digest.
        request: [u8; 32],
        /// Positional verification dependencies digest (§4.3).
        dependencies: [u8; 32],
        /// Effective accepted time lower bound `L`.
        accepted_lower_ms: u64,
        /// Effective accepted time upper bound `U`.
        accepted_upper_ms: u64,
    },
    /// Credit a received Payment once.
    #[codec(index = 4)]
    Receive {
        /// Credit identity.
        credit_id: [u8; 32],
        /// Paying wallet.
        payer_wallet_id: [u8; 32],
        /// Digest of the full canonical Payment.
        payment: [u8; 32],
        /// Amount added to the balance; positive.
        amount: u128,
    },
    /// Remove the pending-outgoing descriptor of a delivered credit.
    #[codec(index = 5)]
    ArchiveSent {
        /// Credit identity.
        credit_id: [u8; 32],
        /// Digest of the verified Credited evidence.
        credited: [u8; 32],
    },
    /// Create a ledger-directed redemption claim.
    #[codec(index = 6)]
    Unload {
        /// Unload nullifier ([`kagemusha_wallet_unload_nullifier_v1`]).
        nullifier: [u8; 32],
        /// Consumed redemption ordinal.
        redeem_ordinal: u128,
        /// Net offline value subtracted from the balance; positive.
        amount: u128,
        /// Online charge withheld from the payout; at most `amount`.
        online_charge: u128,
        /// Charge quote digest; zero exactly when `online_charge` is zero.
        charge_quote: [u8; 32],
    },
    /// Apply exactly one signed policy update.
    #[codec(index = 7)]
    RefreshPolicy {
        /// Kind of the applied update object.
        update_kind: KagemushaWalletPolicyUpdateKindV1,
        /// Object digest of the applied update.
        update: [u8; 32],
        /// Successor accepted time floor.
        accepted_time_floor_ms: u64,
    },
    /// Close new setup and funding; `next_load` is in the statement.
    #[codec(index = 8)]
    Retiring,
}

impl KagemushaWalletEffectV1 {
    /// Operation kind of this effect.
    #[must_use]
    pub const fn kind(&self) -> KagemushaWalletOperationKindV1 {
        match self {
            Self::Bootstrap { .. } => KagemushaWalletOperationKindV1::Bootstrap,
            Self::Load { .. } => KagemushaWalletOperationKindV1::Load,
            Self::Send { .. } => KagemushaWalletOperationKindV1::Send,
            Self::Receive { .. } => KagemushaWalletOperationKindV1::Receive,
            Self::ArchiveSent { .. } => KagemushaWalletOperationKindV1::ArchiveSent,
            Self::Unload { .. } => KagemushaWalletOperationKindV1::Unload,
            Self::RefreshPolicy { .. } => KagemushaWalletOperationKindV1::RefreshPolicy,
            Self::Retiring => KagemushaWalletOperationKindV1::Retiring,
        }
    }

    /// Transcript tag; equal to the Norito wire tag and the operation kind tag.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        self.kind().tag()
    }

    /// Operation-id input of this effect (§3.6): Bootstrap enrollment id, Load voucher,
    /// Send/Receive/`ArchiveSent` credit id, Unload nullifier, `RefreshPolicy` update digest, and
    /// 32 zero bytes for Retiring.
    #[must_use]
    pub const fn operation_input(&self) -> [u8; 32] {
        match self {
            Self::Bootstrap { enrollment_id, .. } => *enrollment_id,
            Self::Load { voucher, .. } => *voucher,
            Self::Send { credit_id, .. }
            | Self::Receive { credit_id, .. }
            | Self::ArchiveSent { credit_id, .. } => *credit_id,
            Self::Unload { nullifier, .. } => *nullifier,
            Self::RefreshPolicy { update, .. } => *update,
            Self::Retiring => [0; 32],
        }
    }

    /// Fixed field width of this variant before zero fill.
    const fn fields_bytes(&self) -> usize {
        match self {
            Self::Bootstrap { .. } => BOOTSTRAP_EFFECT_FIELDS_BYTES,
            Self::Load { .. } => LOAD_EFFECT_FIELDS_BYTES,
            Self::Send { .. } => SEND_EFFECT_FIELDS_BYTES,
            Self::Receive { .. } => RECEIVE_EFFECT_FIELDS_BYTES,
            Self::ArchiveSent { .. } => ARCHIVE_SENT_EFFECT_FIELDS_BYTES,
            Self::Unload { .. } => UNLOAD_EFFECT_FIELDS_BYTES,
            Self::RefreshPolicy { .. } => REFRESH_POLICY_EFFECT_FIELDS_BYTES,
            Self::Retiring => RETIRING_EFFECT_FIELDS_BYTES,
        }
    }

    /// Append this variant's fixed fields.
    fn write_fields(&self, transcript: WalletTranscriptV1) -> WalletTranscriptV1 {
        match self {
            Self::Bootstrap {
                enrollment_id,
                enrollment_marker,
            } => transcript.digest(enrollment_id).digest(enrollment_marker),
            Self::Load {
                voucher,
                load_ordinal,
                amount,
                online_charge,
            } => transcript
                .digest(voucher)
                .u128(*load_ordinal)
                .u128(*amount)
                .u128(*online_charge),
            Self::Send {
                credit_id,
                receiver_wallet_id,
                send_ordinal,
                amount,
                fee,
                request,
                dependencies,
                accepted_lower_ms,
                accepted_upper_ms,
            } => transcript
                .digest(credit_id)
                .digest(receiver_wallet_id)
                .u128(*send_ordinal)
                .u128(*amount)
                .u128(*fee)
                .digest(request)
                .digest(dependencies)
                .u64(*accepted_lower_ms)
                .u64(*accepted_upper_ms),
            Self::Receive {
                credit_id,
                payer_wallet_id,
                payment,
                amount,
            } => transcript
                .digest(credit_id)
                .digest(payer_wallet_id)
                .digest(payment)
                .u128(*amount),
            Self::ArchiveSent {
                credit_id,
                credited,
            } => transcript.digest(credit_id).digest(credited),
            Self::Unload {
                nullifier,
                redeem_ordinal,
                amount,
                online_charge,
                charge_quote,
            } => transcript
                .digest(nullifier)
                .u128(*redeem_ordinal)
                .u128(*amount)
                .u128(*online_charge)
                .digest(charge_quote),
            Self::RefreshPolicy {
                update_kind,
                update,
                accepted_time_floor_ms,
            } => transcript
                .u8(update_kind.tag())
                .digest(update)
                .u64(*accepted_time_floor_ms),
            Self::Retiring => transcript,
        }
    }

    /// Append the tag, the fixed fields and the zero fill of the union.
    fn write(&self, transcript: WalletTranscriptV1) -> WalletTranscriptV1 {
        self.write_fields(transcript.u8(self.tag()))
            .zeros(KAGEMUSHA_WALLET_EFFECT_UNION_BYTES_V1.saturating_sub(self.fields_bytes()))
    }

    /// Exact effect transcript: tag, fields, zero fill to the union width.
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        self.write(WalletTranscriptV1::with_capacity(
            KAGEMUSHA_WALLET_EFFECT_TRANSCRIPT_BYTES_V1,
        ))
        .finish()
    }

    /// Validate the effect's self-contained rules.
    ///
    /// # Errors
    ///
    /// Rejects zero identities, a zero Send, Receive or Unload amount, a Send whose gross
    /// debit overflows or whose accepted interval is inverted, and an Unload whose online
    /// charge exceeds its amount or disagrees with its charge quote.
    pub fn validate(&self) -> WalletResult<()> {
        match self {
            Self::Bootstrap {
                enrollment_id,
                enrollment_marker,
            } => {
                require_nonzero_v1("effect.enrollment_id", enrollment_id)?;
                require_nonzero_v1("effect.enrollment_marker", enrollment_marker)
            }
            Self::Load { voucher, .. } => require_nonzero_v1("effect.voucher", voucher),
            Self::Send {
                credit_id,
                receiver_wallet_id,
                amount,
                fee,
                request,
                dependencies,
                accepted_lower_ms,
                accepted_upper_ms,
                ..
            } => {
                require_nonzero_v1("effect.credit_id", credit_id)?;
                require_nonzero_v1("effect.receiver_wallet_id", receiver_wallet_id)?;
                require_nonzero_v1("effect.request", request)?;
                require_nonzero_v1("effect.dependencies", dependencies)?;
                if *amount == 0 {
                    return Err(invalid_v1("effect.amount"));
                }
                amount
                    .checked_add(*fee)
                    .ok_or_else(|| overflow_v1("effect.gross"))?;
                if accepted_lower_ms > accepted_upper_ms {
                    return Err(invalid_v1("effect.accepted_time"));
                }
                Ok(())
            }
            Self::Receive {
                credit_id,
                payer_wallet_id,
                payment,
                amount,
            } => {
                require_nonzero_v1("effect.credit_id", credit_id)?;
                require_nonzero_v1("effect.payer_wallet_id", payer_wallet_id)?;
                require_nonzero_v1("effect.payment", payment)?;
                if *amount == 0 {
                    return Err(invalid_v1("effect.amount"));
                }
                Ok(())
            }
            Self::ArchiveSent {
                credit_id,
                credited,
            } => {
                require_nonzero_v1("effect.credit_id", credit_id)?;
                require_nonzero_v1("effect.credited", credited)
            }
            Self::Unload {
                nullifier,
                amount,
                online_charge,
                charge_quote,
                ..
            } => {
                require_nonzero_v1("effect.nullifier", nullifier)?;
                if *amount == 0 {
                    return Err(invalid_v1("effect.amount"));
                }
                if online_charge > amount {
                    return Err(invalid_v1("effect.online_charge"));
                }
                if (*online_charge > 0) == is_zero_v1(charge_quote) {
                    return Err(invalid_v1("effect.charge_quote"));
                }
                Ok(())
            }
            Self::RefreshPolicy { update, .. } => require_nonzero_v1("effect.update", update),
            Self::Retiring => Ok(()),
        }
    }
}

/// Exact `operation-id` transcript: `wallet_id || u8 kind || input`.
#[must_use]
pub fn kagemusha_wallet_operation_id_transcript_v1(
    wallet_id: &[u8; 32],
    kind: KagemushaWalletOperationKindV1,
    input: &[u8; 32],
) -> Vec<u8> {
    WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_OPERATION_ID_TRANSCRIPT_BYTES_V1)
        .digest(wallet_id)
        .u8(kind.tag())
        .digest(input)
        .finish()
}

/// Provider operation identity `H("operation-id", wallet_id || u8 kind || input)` (§3.6).
#[must_use]
pub fn kagemusha_wallet_operation_id_v1(
    wallet_id: &[u8; 32],
    kind: KagemushaWalletOperationKindV1,
    input: &[u8; 32],
) -> [u8; 32] {
    kagemusha_wallet_digest_v1(
        Role::OperationId,
        &kagemusha_wallet_operation_id_transcript_v1(wallet_id, kind, input),
    )
}

/// Exact `unload-nullifier` transcript: `scheme_id || wallet_id || LE128 redeem_ordinal`.
#[must_use]
pub fn kagemusha_wallet_unload_nullifier_transcript_v1(
    scheme_id: &[u8; 32],
    wallet_id: &[u8; 32],
    redeem_ordinal: u128,
) -> Vec<u8> {
    WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_UNLOAD_NULLIFIER_TRANSCRIPT_BYTES_V1)
        .digest(scheme_id)
        .digest(wallet_id)
        .u128(redeem_ordinal)
        .finish()
}

/// Unload nullifier `H("unload-nullifier", scheme_id || wallet_id || LE128 ordinal)` (§6.1).
#[must_use]
pub fn kagemusha_wallet_unload_nullifier_v1(
    scheme_id: &[u8; 32],
    wallet_id: &[u8; 32],
    redeem_ordinal: u128,
) -> [u8; 32] {
    kagemusha_wallet_digest_v1(
        Role::UnloadNullifier,
        &kagemusha_wallet_unload_nullifier_transcript_v1(scheme_id, wallet_id, redeem_ordinal),
    )
}

/// Public statement of one transition (§3.3).
///
/// `lifecycle`, `sequence` and `next_load` are the successor state's values. Bootstrap has
/// sequence zero and an all-zero predecessor; every later sequence is its predecessor's plus
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletStatementV1"
)]
pub struct KagemushaWalletStatementV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Scheme.
    pub scheme_id: [u8; 32],
    /// Frozen relation identity of the scheme.
    pub relation_id: [u8; 32],
    /// Digest of the credential this transition runs under.
    pub credential_digest: [u8; 32],
    /// Asset scope digest.
    pub asset_digest: [u8; 32],
    /// Successor lifecycle.
    pub lifecycle: KagemushaWalletLifecycleV1,
    /// Successor sequence.
    pub sequence: u128,
    /// Successor `next_load`.
    pub next_load: u128,
    /// Predecessor state commitment.
    pub predecessor: KagemushaWalletStateCommitmentV1,
    /// Successor state commitment.
    pub successor: KagemushaWalletStateCommitmentV1,
    /// Public effect.
    pub effect: KagemushaWalletEffectV1,
}

impl KagemushaWalletStatementV1 {
    /// Exact `statement` transcript (constant length).
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        let transcript =
            WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_STATEMENT_TRANSCRIPT_BYTES_V1)
                .u16(self.version)
                .digest(&self.scheme_id)
                .digest(&self.relation_id)
                .digest(&self.credential_digest)
                .digest(&self.asset_digest)
                .u8(self.lifecycle.tag())
                .u128(self.sequence)
                .u128(self.next_load);
        let transcript = self.successor.write(self.predecessor.write(transcript));
        self.effect.write(transcript).finish()
    }

    /// Statement digest `H("statement", transcript)`.
    #[must_use]
    pub fn statement_digest(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::Statement, &self.transcript())
    }

    /// Operation identity of this transition for `wallet_id` (§3.6).
    #[must_use]
    pub fn operation_id(&self, wallet_id: &[u8; 32]) -> [u8; 32] {
        kagemusha_wallet_operation_id_v1(
            wallet_id,
            self.effect.kind(),
            &self.effect.operation_input(),
        )
    }

    /// Validate the statement's self-contained rules (§3.3, design C5).
    ///
    /// # Errors
    ///
    /// Rejects another version, zero bindings, an incomplete successor, an invalid effect, a
    /// Bootstrap that is not the unique zero-state base case, a later transition without a
    /// predecessor, a Retiring effect whose successor is not Retiring, a Load whose
    /// `next_load` is not its ordinal plus one, and a replacement-credential refresh whose
    /// update is not the statement's credential.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("statement.version", self.version)?;
        for (field, digest) in [
            ("statement.scheme_id", &self.scheme_id),
            ("statement.relation_id", &self.relation_id),
            ("statement.credential_digest", &self.credential_digest),
            ("statement.asset_digest", &self.asset_digest),
        ] {
            require_nonzero_v1(field, digest)?;
        }
        if !self.successor.is_complete() {
            return Err(invalid_v1("statement.successor"));
        }
        self.effect.validate()?;
        if let KagemushaWalletEffectV1::Bootstrap { .. } = &self.effect {
            if self.sequence != 0
                || !self.predecessor.is_zero()
                || self.lifecycle != KagemushaWalletLifecycleV1::Active
                || self.next_load != 0
            {
                return Err(invalid_v1("statement.bootstrap"));
            }
        } else {
            if self.sequence == 0 {
                return Err(invalid_v1("statement.sequence"));
            }
            if !self.predecessor.is_complete() {
                return Err(invalid_v1("statement.predecessor"));
            }
        }
        match &self.effect {
            KagemushaWalletEffectV1::Retiring
                if self.lifecycle != KagemushaWalletLifecycleV1::Retiring =>
            {
                Err(invalid_v1("statement.lifecycle"))
            }
            KagemushaWalletEffectV1::Load { load_ordinal, .. } => {
                let next = load_ordinal
                    .checked_add(1)
                    .ok_or_else(|| overflow_v1("statement.next_load"))?;
                if self.next_load == next {
                    Ok(())
                } else {
                    Err(invalid_v1("statement.next_load"))
                }
            }
            KagemushaWalletEffectV1::RefreshPolicy {
                update_kind: KagemushaWalletPolicyUpdateKindV1::Credential,
                update,
                ..
            } if *update != self.credential_digest => Err(invalid_v1("effect.update")),
            _ => Ok(()),
        }
    }

    /// Validate the statement against the scheme it claims.
    ///
    /// # Errors
    ///
    /// Rejects an invalid statement, another scheme, or another relation.
    pub fn validate_for_scheme(&self, scheme: &KagemushaWalletSchemeV1) -> WalletResult<()> {
        self.validate()?;
        require_scheme_v1("statement.scheme_id", &self.scheme_id, &scheme.scheme_id())?;
        require_scheme_v1(
            "statement.relation_id",
            &self.relation_id,
            &scheme.relation_id,
        )
    }

    /// Validate the statement against the credential it runs under.
    ///
    /// # Errors
    ///
    /// Rejects an invalid statement, another scheme, credential or asset, a Bootstrap for
    /// another enrollment, a Send to or Receive from the wallet itself, and an Unload whose
    /// nullifier does not recompute.
    pub fn validate_for_credential(
        &self,
        credential: &KagemushaWalletCredentialV1,
    ) -> WalletResult<()> {
        self.validate()?;
        credential.validate()?;
        let body = &credential.body;
        require_scheme_v1("statement.scheme_id", &self.scheme_id, &body.scheme_id)?;
        if self.credential_digest != credential.credential_digest() {
            return Err(invalid_v1("statement.credential_digest"));
        }
        if self.asset_digest != body.asset_digest {
            return Err(invalid_v1("statement.asset_digest"));
        }
        match &self.effect {
            KagemushaWalletEffectV1::Bootstrap { enrollment_id, .. }
                if *enrollment_id != body.enrollment_id =>
            {
                Err(invalid_v1("effect.enrollment_id"))
            }
            KagemushaWalletEffectV1::Send {
                receiver_wallet_id, ..
            } if *receiver_wallet_id == body.wallet_id => {
                Err(invalid_v1("effect.receiver_wallet_id"))
            }
            KagemushaWalletEffectV1::Receive {
                payer_wallet_id, ..
            } if *payer_wallet_id == body.wallet_id => Err(invalid_v1("effect.payer_wallet_id")),
            KagemushaWalletEffectV1::Unload {
                nullifier,
                redeem_ordinal,
                ..
            } if *nullifier
                != kagemusha_wallet_unload_nullifier_v1(
                    &body.scheme_id,
                    &body.wallet_id,
                    *redeem_ordinal,
                ) =>
            {
                Err(invalid_v1("effect.nullifier"))
            }
            _ => Ok(()),
        }
    }

    /// Validate `self` as the direct successor of `predecessor` (§§3.3, 6.3, design C5).
    ///
    /// # Errors
    ///
    /// Rejects an invalid statement, another scheme, relation or asset, a predecessor
    /// commitment or sequence that does not chain, a lifecycle that reverts or retires twice,
    /// a `next_load` that does not follow its effect, and a credential change outside a
    /// replacement-credential refresh.
    pub fn validate_successor_of(&self, predecessor: &Self) -> WalletResult<()> {
        self.validate()?;
        predecessor.validate()?;
        require_scheme_v1(
            "statement.scheme_id",
            &self.scheme_id,
            &predecessor.scheme_id,
        )?;
        require_scheme_v1(
            "statement.relation_id",
            &self.relation_id,
            &predecessor.relation_id,
        )?;
        if self.asset_digest != predecessor.asset_digest {
            return Err(invalid_v1("statement.asset_digest"));
        }
        if self.predecessor != predecessor.successor {
            return Err(invalid_v1("statement.predecessor"));
        }
        let sequence = predecessor
            .sequence
            .checked_add(1)
            .ok_or_else(|| overflow_v1("statement.sequence"))?;
        if self.sequence != sequence {
            return Err(invalid_v1("statement.sequence"));
        }
        let lifecycle_ok = match self.effect {
            KagemushaWalletEffectV1::Retiring => {
                predecessor.lifecycle == KagemushaWalletLifecycleV1::Active
            }
            _ => self.lifecycle == predecessor.lifecycle,
        };
        if !lifecycle_ok {
            return Err(invalid_v1("statement.lifecycle"));
        }
        let next_load_ok = match self.effect {
            KagemushaWalletEffectV1::Load { load_ordinal, .. } => {
                load_ordinal == predecessor.next_load
            }
            _ => self.next_load == predecessor.next_load,
        };
        if !next_load_ok {
            return Err(invalid_v1("statement.next_load"));
        }
        let replaces_credential = matches!(
            self.effect,
            KagemushaWalletEffectV1::RefreshPolicy {
                update_kind: KagemushaWalletPolicyUpdateKindV1::Credential,
                ..
            }
        );
        if !replaces_credential && self.credential_digest != predecessor.credential_digest {
            return Err(invalid_v1("statement.credential_digest"));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------
// Proof, receipt and package (§§3.4–3.5)
// ---------------------------------------------------------------------------------------

/// Recursive transition or `CreditStatus` proof bytes (§3.4).
///
/// The caps are decode sanity bounds; the message bounds are authoritative.
// TODO(G3): the concrete proof layout and the in-circuit proof binding are fixed by the G3
// artifact set.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletProofV1")]
pub struct KagemushaWalletProofV1 {
    /// Opaque proof bytes.
    pub bytes: Vec<u8>,
}

impl KagemushaWalletProofV1 {
    /// Proof digest `H("proof", bytes)`; consumers recompute it on every use.
    #[must_use]
    pub fn proof_digest(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::Proof, &self.bytes)
    }

    fn validate_within(&self, max: usize) -> WalletResult<()> {
        if self.bytes.is_empty() || self.bytes.len() > max {
            return Err(invalid_v1("proof.bytes"));
        }
        Ok(())
    }

    /// Validate a transition proof: `1..=`[`KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1`] bytes.
    ///
    /// # Errors
    ///
    /// Rejects an empty or oversized proof.
    pub fn validate(&self) -> WalletResult<()> {
        self.validate_within(KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1)
    }

    /// Validate a read-only `CreditStatus` proof:
    /// `1..=`[`KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1`] bytes.
    ///
    /// # Errors
    ///
    /// Rejects an empty or oversized proof.
    pub fn validate_credit_status(&self) -> WalletResult<()> {
        self.validate_within(KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1)
    }
}

/// Derived `receipt-body` fields of one commit receipt (§3.5).
///
/// Scheme, wallet and provider contract come from the credential; sequence and commitments
/// from the statement. This value is never transmitted; it is recomputed by every verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaWalletReceiptBodyV1 {
    /// Receipt version.
    pub version: u16,
    /// Credential scheme.
    pub scheme_id: [u8; 32],
    /// Credential wallet.
    pub wallet_id: [u8; 32],
    /// Credential provider contract.
    pub provider_contract: [u8; 32],
    /// Statement successor sequence.
    pub sequence: u128,
    /// Operation identity derived from the statement.
    pub operation_id: [u8; 32],
    /// Statement predecessor commitment.
    pub predecessor: KagemushaWalletStateCommitmentV1,
    /// Statement successor commitment.
    pub successor: KagemushaWalletStateCommitmentV1,
    /// Statement digest.
    pub statement_digest: [u8; 32],
    /// Proof digest.
    pub proof_digest: [u8; 32],
    /// Recovery capsule digest.
    pub capsule_digest: [u8; 32],
}

impl KagemushaWalletReceiptBodyV1 {
    /// Derive the receipt body of `statement` and `proof` under `credential`.
    ///
    /// # Errors
    ///
    /// Rejects an invalid statement, proof or credential, a statement for another
    /// credential, and a zero capsule digest.
    pub fn derive(
        credential: &KagemushaWalletCredentialV1,
        statement: &KagemushaWalletStatementV1,
        proof: &KagemushaWalletProofV1,
        capsule_digest: [u8; 32],
    ) -> WalletResult<Self> {
        statement.validate_for_credential(credential)?;
        proof.validate()?;
        require_nonzero_v1("receipt.capsule_digest", &capsule_digest)?;
        let body = &credential.body;
        Ok(Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: body.scheme_id,
            wallet_id: body.wallet_id,
            provider_contract: body.provider_contract,
            sequence: statement.sequence,
            operation_id: statement.operation_id(&body.wallet_id),
            predecessor: statement.predecessor,
            successor: statement.successor,
            statement_digest: statement.statement_digest(),
            proof_digest: proof.proof_digest(),
            capsule_digest,
        })
    }

    /// Exact `receipt-body` transcript.
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        let transcript =
            WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_RECEIPT_BODY_TRANSCRIPT_BYTES_V1)
                .u16(self.version)
                .digest(&self.scheme_id)
                .digest(&self.wallet_id)
                .digest(&self.provider_contract)
                .u128(self.sequence)
                .digest(&self.operation_id);
        self.successor
            .write(self.predecessor.write(transcript))
            .digest(&self.statement_digest)
            .digest(&self.proof_digest)
            .digest(&self.capsule_digest)
            .finish()
    }

    /// Signed body digest `e = H("receipt-body", transcript)`.
    #[must_use]
    pub fn body_digest(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::ReceiptBody, &self.transcript())
    }

    /// Exact ECDSA message the payment key signs.
    #[must_use]
    pub fn signing_message(&self) -> Vec<u8> {
        kagemusha_wallet_preimage_v1(Role::ReceiptBody, &self.transcript())
    }
}

/// Durable provider commit receipt signed by the credential's payment key (§§3.5, 4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletReceiptV1"
)]
pub struct KagemushaWalletReceiptV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Operation identity; recomputed from the statement by every verifier.
    pub operation_id: [u8; 32],
    /// Digest of the frozen recovery capsule body.
    pub capsule_digest: [u8; 32],
    /// Payment-key signature over `receipt-body`.
    pub signature: KagemushaDeviceSignatureV1,
}

impl KagemushaWalletReceiptV1 {
    /// Freeze a payment-key signature over the derived receipt body.
    ///
    /// # Errors
    ///
    /// Rejects what [`KagemushaWalletReceiptBodyV1::derive`] rejects and a signature that does
    /// not verify under the credential's payment key.
    pub fn sign(
        credential: &KagemushaWalletCredentialV1,
        statement: &KagemushaWalletStatementV1,
        proof: &KagemushaWalletProofV1,
        capsule_digest: [u8; 32],
        signer_output: KagemushaWalletSignerOutputV1<'_>,
    ) -> WalletResult<Self> {
        let body =
            KagemushaWalletReceiptBodyV1::derive(credential, statement, proof, capsule_digest)?;
        let signature = kagemusha_wallet_freeze_signature_v1(
            &credential.body.payment_key,
            Role::ReceiptBody,
            &body.transcript(),
            signer_output,
        )?;
        let receipt = Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            operation_id: body.operation_id,
            capsule_digest,
            signature,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Validate the receipt's self-contained fields.
    ///
    /// # Errors
    ///
    /// Rejects another version, zero digests, or a non-canonical signature encoding.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("receipt.version", self.version)?;
        require_nonzero_v1("receipt.operation_id", &self.operation_id)?;
        require_nonzero_v1("receipt.capsule_digest", &self.capsule_digest)?;
        self.signature.validate()?;
        Ok(())
    }

    /// Derive this receipt's body for `statement` and `proof` under `credential`.
    ///
    /// # Errors
    ///
    /// Rejects what [`KagemushaWalletReceiptBodyV1::derive`] rejects and an operation
    /// identity that differs from the one derived from the statement.
    pub fn body(
        &self,
        credential: &KagemushaWalletCredentialV1,
        statement: &KagemushaWalletStatementV1,
        proof: &KagemushaWalletProofV1,
    ) -> WalletResult<KagemushaWalletReceiptBodyV1> {
        self.validate()?;
        let body = KagemushaWalletReceiptBodyV1::derive(
            credential,
            statement,
            proof,
            self.capsule_digest,
        )?;
        if body.operation_id != self.operation_id {
            return Err(invalid_v1("receipt.operation_id"));
        }
        Ok(body)
    }

    /// Verify the receipt and return its digest `H("receipt", e || signature)`.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::body`] rejects and a signature that does not verify under the
    /// credential's payment key.
    pub fn verify(
        &self,
        credential: &KagemushaWalletCredentialV1,
        statement: &KagemushaWalletStatementV1,
        proof: &KagemushaWalletProofV1,
    ) -> WalletResult<[u8; 32]> {
        let body = self.body(credential, statement, proof)?;
        kagemusha_wallet_verify_signature_v1(
            &credential.body.payment_key,
            Role::ReceiptBody,
            &body.transcript(),
            &self.signature,
        )?;
        Ok(kagemusha_wallet_signed_object_digest_v1(
            Role::Receipt,
            &body.body_digest(),
            &self.signature,
        ))
    }
}

/// Package digest `H("package", statement_digest || proof_digest || receipt_digest)`.
#[must_use]
pub fn kagemusha_wallet_package_digest_v1(
    statement_digest: &[u8; 32],
    proof_digest: &[u8; 32],
    receipt_digest: &[u8; 32],
) -> [u8; 32] {
    kagemusha_wallet_digest_v1(
        Role::Package,
        &WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_PACKAGE_TRANSCRIPT_BYTES_V1)
            .digest(statement_digest)
            .digest(proof_digest)
            .digest(receipt_digest)
            .finish(),
    )
}

/// Digests of one verified package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaWalletPackageDigestsV1 {
    /// Statement digest.
    pub statement: [u8; 32],
    /// Proof digest.
    pub proof: [u8; 32],
    /// Receipt digest.
    pub receipt: [u8; 32],
    /// Package digest.
    pub package: [u8; 32],
}

/// Complete public state package `(statement, π, τ)` (§§3.1, 3.5).
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPackageV1"
)]
pub struct KagemushaWalletPackageV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Transition statement.
    pub statement: KagemushaWalletStatementV1,
    /// Recursive transition proof.
    pub proof: KagemushaWalletProofV1,
    /// Provider commit receipt.
    pub receipt: KagemushaWalletReceiptV1,
}

impl KagemushaWalletPackageV1 {
    /// Assemble a package from its parts.
    #[must_use]
    pub fn new(
        statement: KagemushaWalletStatementV1,
        proof: KagemushaWalletProofV1,
        receipt: KagemushaWalletReceiptV1,
    ) -> Self {
        Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            statement,
            proof,
            receipt,
        }
    }

    /// Validate the package's self-contained structure.
    ///
    /// # Errors
    ///
    /// Rejects another version, an invalid statement or receipt, or a proof outside its cap.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("package.version", self.version)?;
        self.statement.validate()?;
        self.proof.validate()?;
        self.receipt.validate()
    }

    /// Verify the package against the credential it runs under and return its digests.
    ///
    /// The credential's own issuer signature is verified separately by its consumer; the
    /// recursive proof is verified by the proof owner.
    ///
    /// # Errors
    ///
    /// Rejects an invalid package, a statement for another credential, scheme, asset or
    /// enrollment, a receipt whose operation identity does not recompute, and a receipt
    /// signature that does not verify under the payment key.
    pub fn verify(
        &self,
        credential: &KagemushaWalletCredentialV1,
    ) -> WalletResult<KagemushaWalletPackageDigestsV1> {
        self.validate()?;
        let receipt = self
            .receipt
            .verify(credential, &self.statement, &self.proof)?;
        let statement = self.statement.statement_digest();
        let proof = self.proof.proof_digest();
        Ok(KagemushaWalletPackageDigestsV1 {
            statement,
            proof,
            receipt,
            package: kagemusha_wallet_package_digest_v1(&statement, &proof, &receipt),
        })
    }

    /// Package digest after [`Self::verify`].
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::verify`] rejects.
    pub fn package_digest(
        &self,
        credential: &KagemushaWalletCredentialV1,
    ) -> WalletResult<[u8; 32]> {
        Ok(self.verify(credential)?.package)
    }
}

// ---------------------------------------------------------------------------------------
// Version fields (design §0 decode order)
// ---------------------------------------------------------------------------------------

impl WalletVersionsV1 for KagemushaWalletStateV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("state.version", self.version)
    }
}

impl WalletVersionsV1 for KagemushaWalletStatementV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("statement.version", self.version)
    }
}

impl WalletVersionsV1 for KagemushaWalletReceiptV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("receipt.version", self.version)
    }
}

impl WalletVersionsV1 for KagemushaWalletPackageV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("package.version", self.version)?;
        self.statement.require_versions()?;
        self.receipt.require_versions()
    }
}
