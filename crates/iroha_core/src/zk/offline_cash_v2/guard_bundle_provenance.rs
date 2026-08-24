//! Private k=17 GuardBundle ABI and provenance contract for Offline Cash V2.
//!
//! This source-only tranche freezes the finite V2 role namespace, exact
//! source-contract protocol identities, a 336-word field-neutral GuardBundle
//! ABI, and the one ownership path by which a registered-platform P-256 pair
//! may accompany an authenticated current helper. It does not implement any
//! helper circuit, recursive compiler, BGH19 verifier, ECC strategy, artifact
//! loader, backend, wire field, readiness decision, release, or production
//! authorization.
//!
//! The GuardBundle ABI has a 16-word header, 22 exact digest slots, and one
//! canonical 144-word/576-byte predecessor-lineage tail beginning at word 192.
//! It occupies exactly 48 cells of seven little-endian `u32` words and has no
//! padding. The current GuardBundle proof accumulator and proof bytes are
//! deliberately absent: they are post-transcript state and may reach STATE only
//! through the uninhabited verified-handoff boundary below.

use core::{convert::Infallible, fmt};

use halo2_proofs::halo2curves::ff::PrimeField;
use sha2::{Digest as _, Sha256};

#[cfg(test)]
use super::state_recursive_fold::StateRecursiveFoldParityV2;
use super::{
    OFFLINE_CASH_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2, OFFLINE_CASH_HALO2_K_V2,
    OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2, OfflineCashHalo2CircuitRoleV2,
    OfflineCashHalo2ParityV2,
    registered_platform_p256_circuit_source::UnverifiedRegisteredPlatformP256CircuitCandidatesV2,
    registered_platform_p256_statement::RegisteredPlatformP256CurrentHelperViewV2,
    state_lineage::{
        OFFLINE_CASH_GUARD_BUNDLE_LINEAGE_CHILD_ORDER_V2, OfflineCashEpParentLineageV2,
        OfflineCashEqParentLineageV2, OfflineCashGuardBundleLineageChildRoleV2,
        OfflineCashParentLineageCodecErrorV2,
    },
    state_recursive_fold::CanonicalStateAccumulatorV2,
};

const PROTOCOL_DOMAIN_V2: &[u8] = b"iroha:offline-cash:v2:halo2-source-protocol";
const HALO2_BACKEND_REVISION_V2: &[u8] = b"halo2-axiom/0.5.1";
const SNARK_VERIFIER_REVISION_V2: &[u8] =
    b"snark-verifier/bbfcc721d714bea0d44a27c8fc6c4736e73ca853";
const PCS_REVISION_V2: &[u8] = b"transparent-pasta-ipa/no-trusted-setup";
const TRANSCRIPT_REVISION_V2: &[u8] = b"Blake2bRead+Blake2bWrite/Challenge255";
const KEY_FORMAT_REVISION_V2: &[u8] = b"halo2-axiom/SerdeFormat::Processed";
const LINEAGE_REVISION_V2: &[u8] =
    b"paired-pasta-ipa-lineage/17-canonical-scalars-then-compressed-point/v2";
const INSTANCE_QUERY_REVISION_V2: &[u8] = b"direct-one-column/query-instance=false/v2";
const STATE_ABI_SOURCE_REVISION_V2: &[u8] =
    b"state-abi/237-u32le/pack7/34-cells/lineage144-at-word93/source-only/v2";
const HELPER_CHILD_ABI_TARGET_REVISION_V2: &[u8] =
    b"helper-child/common-current-statement/k17/compact-proof-under-3264/not-implemented/v2";
const GUARD_BUNDLE_ABI_SOURCE_REVISION_V2: &[u8] = b"guard-bundle-abi/336-u32le/header16+digests22+prior-lineage144-at-word192/pack7/48-cells/no-padding/current-accumulator-absent/source-only/v2";
const P256_SOURCE_REVISION_V2: &[u8] =
    b"p256-packed-affine-v3/161-byte-sec1-prehash-p1363/k17/source-only/v2";
const GUARD_BUNDLE_CHILD_ORDER_REVISION_V2: &[u8] =
    b"guard-use,platform-bind,android-key-cert-if-present,p256-signature-if-present/no-reorder/v2";

/// GuardBundle ABI version in header word zero.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ABI_VERSION_V2: u32 = 2;
/// Source-profile version in header word one. This is not an active wire type.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_SOURCE_PROFILE_VERSION_V2: u32 = 2;
/// Exact number of little-endian words in the GuardBundle ABI.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2: usize = 336;
/// Canonical words packed into one public-instance cell.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2: usize = 7;
/// Exact direct-instance cell count.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2: usize = 48;
/// Exact bytes in one field-neutral packed cell.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PACKED_CELL_BYTES_V2: usize = 28;
/// No terminal cell padding exists because 336 is divisible by seven.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_FINAL_CELL_ZERO_PADDING_WORDS_V2: usize = 0;
/// Canonical words in each digest.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORDS_V2: usize = 8;
/// Protocol plus 21 exact semantic digest slots.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_DIGEST_FIELDS_V2: usize = 22;
/// Exact words in the prior k=17 GuardBundle lineage.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORDS_V2: usize = 144;

pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PROTOCOL_WORD_START_V2: usize = 16;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_RELEASE_WORD_START_V2: usize = 24;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CONTEXT_WORD_START_V2: usize = 32;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CURRENT_HEAD_WORD_START_V2: usize = 40;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CURRENT_LINEAGE_WORD_START_V2: usize = 48;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_TRANSITION_WORD_START_V2: usize = 56;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_WALLET_WORD_START_V2: usize = 64;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_POLICY_WORD_START_V2: usize = 72;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_DEVICE_WORD_START_V2: usize = 80;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CURRENT_GUARD_WORD_START_V2: usize = 88;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_NEXT_GUARD_WORD_START_V2: usize = 96;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_KEY_WORD_START_V2: usize = 104;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_MESSAGE_WORD_START_V2: usize = 112;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_GUARD_USE_CLAIM_WORD_START_V2: usize = 120;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_BIND_CLAIM_WORD_START_V2: usize = 128;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ANDROID_CERTIFICATE_WORD_START_V2: usize = 136;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ANDROID_TBS_WORD_START_V2: usize = 144;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ANDROID_ISSUER_KEY_WORD_START_V2: usize = 152;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ANDROID_ATTESTATION_WORD_START_V2: usize = 160;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ANDROID_CLAIM_WORD_START_V2: usize = 168;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_REGISTRATION_RECEIPT_WORD_START_V2: usize = 176;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORD_START_V2: usize = 184;
/// First word of the parity-local 576-byte predecessor lineage.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORD_START_V2: usize = 192;

/// Direct-instance policy; queried instance commitments are not permitted.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_QUERY_INSTANCE_V2: bool = false;
/// Current proof accumulators are never current GuardBundle public instances.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CURRENT_ACCUMULATOR_IN_PUBLIC_INSTANCES_V2: bool = false;
/// No GuardBundle ABI word carries the current proof accumulator.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CURRENT_ACCUMULATOR_PUBLIC_WORDS_V2: usize = 0;
/// Current proof bytes are not hashed into a GuardBundle semantic digest slot.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CURRENT_PROOF_BYTES_IN_DIGESTS_V2: bool = false;

/// The private ABI/provenance source contract is implemented.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_CONTRACT_IMPLEMENTED_V2: bool = true;
/// Exact source-contract protocol identities are frozen but are not compiled protocols.
pub(super) const OFFLINE_CASH_V2_PROTOCOL_SOURCE_IDENTITIES_FROZEN_V2: bool = true;
/// No compact k=17 GuardUse circuit source is available.
pub(super) const OFFLINE_CASH_GUARD_USE_CIRCUIT_SOURCE_AVAILABLE_V2: bool = false;
/// No compact k=17 PlatformBind circuit source is available.
pub(super) const OFFLINE_CASH_PLATFORM_BIND_CIRCUIT_SOURCE_AVAILABLE_V2: bool = false;
/// No k=17 AndroidKeyCert circuit source is available.
pub(super) const OFFLINE_CASH_ANDROID_KEY_CERT_CIRCUIT_SOURCE_AVAILABLE_V2: bool = false;
/// No GuardBundle recursive compiler is available.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_COMPILER_AVAILABLE_V2: bool = false;
/// No GuardBundle recursive circuit is implemented.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_CIRCUIT_IMPLEMENTED_V2: bool = false;
/// No in-circuit non-native Pasta ECC strategy is governed.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ECC_STRATEGY_GOVERNED_V2: bool = false;
/// No GuardBundle artifact is authenticated.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_ARTIFACTS_AUTHENTICATED_V2: bool = false;
/// No GuardBundle proof backend is available.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_BACKEND_AVAILABLE_V2: bool = false;
/// This source-only tranche adds no wire bytes.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_WIRE_DELTA_BYTES_V2: usize = 0;
/// This source-only tranche adds no proof bytes.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_PROOF_DELTA_BYTES_V2: usize = 0;
/// This source-only tranche adds no authenticated artifact bytes.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_ARTIFACT_DELTA_BYTES_V2: usize = 0;
/// This source-only tranche allocates no Halo2 rows.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_TRACE_ROW_DELTA_V2: usize = 0;
/// No V2 wire adapter carries this ABI.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_WIRE_AVAILABLE_V2: bool = false;
/// No readiness authority exists.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_READINESS_AVAILABLE_V2: bool = false;
/// No release is eligible through this contract.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_RELEASE_ELIGIBLE_V2: bool = false;
/// No production path exists.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PRODUCTION_AVAILABLE_V2: bool = false;

const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2 == 336);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2 == 48);
const _: () = assert!(
    OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2
        == OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2
            * OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2
);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_FINAL_CELL_ZERO_PADDING_WORDS_V2 == 0);
const _: () = assert!(
    16 + OFFLINE_CASH_GUARD_BUNDLE_DIGEST_FIELDS_V2 * OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORDS_V2
        == OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORD_START_V2
);
const _: () = assert!(
    OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORD_START_V2
        + OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORDS_V2
        == OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2
);
const _: () = assert!(
    OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORDS_V2 * 4
        == OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2 as usize
);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_PACKED_CELL_BYTES_V2 == 28);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_QUERY_INSTANCE_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_CURRENT_ACCUMULATOR_IN_PUBLIC_INSTANCES_V2);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_CURRENT_ACCUMULATOR_PUBLIC_WORDS_V2 == 0);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_CURRENT_PROOF_BYTES_IN_DIGESTS_V2);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_CONTRACT_IMPLEMENTED_V2);
const _: () = assert!(OFFLINE_CASH_V2_PROTOCOL_SOURCE_IDENTITIES_FROZEN_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_USE_CIRCUIT_SOURCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_PLATFORM_BIND_CIRCUIT_SOURCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_ANDROID_KEY_CERT_CIRCUIT_SOURCE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_COMPILER_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_CIRCUIT_IMPLEMENTED_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_ECC_STRATEGY_GOVERNED_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_ARTIFACTS_AUTHENTICATED_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_BACKEND_AVAILABLE_V2);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_WIRE_DELTA_BYTES_V2 == 0);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_PROOF_DELTA_BYTES_V2 == 0);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_ARTIFACT_DELTA_BYTES_V2 == 0);
const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_TRACE_ROW_DELTA_V2 == 0);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_WIRE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_READINESS_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_RELEASE_ELIGIBLE_V2);
const _: () = assert!(!OFFLINE_CASH_GUARD_BUNDLE_PRODUCTION_AVAILABLE_V2);

/// Immutable identity of one parity/role V2 source contract.
///
/// The digest is not a compiled-protocol digest and cannot authenticate a VK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashHalo2ProtocolSourceIdentityV2 {
    parity: OfflineCashHalo2ParityV2,
    role: OfflineCashHalo2CircuitRoleV2,
    digest: [u8; 32],
}

impl OfflineCashHalo2ProtocolSourceIdentityV2 {
    pub(super) const fn parity(self) -> OfflineCashHalo2ParityV2 {
        self.parity
    }

    pub(super) const fn role(self) -> OfflineCashHalo2CircuitRoleV2 {
        self.role
    }

    pub(super) const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

struct FramedSha256V2(Sha256);

impl FramedSha256V2 {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(
            u64::try_from(domain.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hasher.update(domain);
        Self(hasher)
    }

    fn field(&mut self, value: &[u8]) {
        self.0
            .update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

fn parity_curve_contract_v2(parity: OfflineCashHalo2ParityV2) -> &'static [u8] {
    match parity {
        OfflineCashHalo2ParityV2::Eq => b"pasta/EqAffine",
        OfflineCashHalo2ParityV2::Ep => b"pasta/EpAffine",
    }
}

fn relation_contract_v2(role: OfflineCashHalo2CircuitRoleV2) -> &'static [u8] {
    match role {
        OfflineCashHalo2CircuitRoleV2::State => {
            b"state/two-semantic-parents+guard-bundle/six-input-lineage-fold/source-only/v2"
        }
        OfflineCashHalo2CircuitRoleV2::GuardUse => {
            b"guard-use/exact-next+operation+state+lineage+policy/compact-source-unavailable/v2"
        }
        OfflineCashHalo2CircuitRoleV2::PlatformBind => {
            b"platform-bind/platform-key+policy+wallet+release/compact-source-unavailable/v2"
        }
        OfflineCashHalo2CircuitRoleV2::AndroidKeyCert => {
            b"android-key-cert/native-registration-identity+current-helper-claim/source-unavailable/v2"
        }
        OfflineCashHalo2CircuitRoleV2::GuardBundle => GUARD_BUNDLE_CHILD_ORDER_REVISION_V2,
        OfflineCashHalo2CircuitRoleV2::P256Signature => P256_SOURCE_REVISION_V2,
    }
}

fn role_abi_contract_v2(role: OfflineCashHalo2CircuitRoleV2) -> &'static [u8] {
    match role {
        OfflineCashHalo2CircuitRoleV2::State => STATE_ABI_SOURCE_REVISION_V2,
        OfflineCashHalo2CircuitRoleV2::GuardBundle => GUARD_BUNDLE_ABI_SOURCE_REVISION_V2,
        OfflineCashHalo2CircuitRoleV2::P256Signature => P256_SOURCE_REVISION_V2,
        OfflineCashHalo2CircuitRoleV2::GuardUse
        | OfflineCashHalo2CircuitRoleV2::PlatformBind
        | OfflineCashHalo2CircuitRoleV2::AndroidKeyCert => HELPER_CHILD_ABI_TARGET_REVISION_V2,
    }
}

/// Return the exact source-contract identity for one V2 role and parity.
///
/// This deliberately does not claim compilation, keygen, or artifact identity.
#[must_use]
pub(super) fn offline_cash_halo2_protocol_source_identity_v2(
    parity: OfflineCashHalo2ParityV2,
    role: OfflineCashHalo2CircuitRoleV2,
) -> OfflineCashHalo2ProtocolSourceIdentityV2 {
    let version = OFFLINE_CASH_GUARD_BUNDLE_SOURCE_PROFILE_VERSION_V2.to_le_bytes();
    let k = OFFLINE_CASH_HALO2_K_V2.to_le_bytes();
    let domain_size = (1_u64 << OFFLINE_CASH_HALO2_K_V2).to_le_bytes();
    let parity_tag = [parity as u8];
    let role_tag = [role as u8];
    let child_cap = OFFLINE_CASH_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2.to_le_bytes();
    let lineage_bytes = OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2.to_le_bytes();
    let query_instance = [u8::from(OFFLINE_CASH_GUARD_BUNDLE_QUERY_INSTANCE_V2)];
    let mut framed = FramedSha256V2::new(PROTOCOL_DOMAIN_V2);
    for field in [
        version.as_slice(),
        k.as_slice(),
        domain_size.as_slice(),
        parity_tag.as_slice(),
        role_tag.as_slice(),
        parity_curve_contract_v2(parity),
        relation_contract_v2(role),
        role_abi_contract_v2(role),
        HALO2_BACKEND_REVISION_V2,
        SNARK_VERIFIER_REVISION_V2,
        PCS_REVISION_V2,
        TRANSCRIPT_REVISION_V2,
        KEY_FORMAT_REVISION_V2,
        LINEAGE_REVISION_V2,
        INSTANCE_QUERY_REVISION_V2,
        child_cap.as_slice(),
        lineage_bytes.as_slice(),
        query_instance.as_slice(),
    ] {
        framed.field(field);
    }
    OfflineCashHalo2ProtocolSourceIdentityV2 {
        parity,
        role,
        digest: framed.finish(),
    }
}

/// Exact current-helper operation shared by every GuardBundle child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum OfflineCashGuardBundleOperationV2 {
    SendSplit = 1,
    ReceiveFold = 2,
}

/// Full current-helper statement retained by the move-only provenance owner.
///
/// Fields are private so an ordinary sibling module cannot manufacture an
/// authenticated owner from raw parts.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct OfflineCashGuardBundleStatementV2 {
    operation: OfflineCashGuardBundleOperationV2,
    android_key_cert_present: bool,
    p256_signature_present: bool,
    from_sequence: u64,
    to_sequence: u64,
    release_id: [u8; 32],
    context_digest: [u8; 32],
    current_head: [u8; 32],
    current_lineage_digest: [u8; 32],
    transition_digest: [u8; 32],
    wallet_binding: [u8; 32],
    hardware_policy_id: [u8; 32],
    guard_device_id: [u8; 32],
    current_guard_binding: [u8; 32],
    next_guard_binding: [u8; 32],
    platform_key_digest: [u8; 32],
    platform_message_digest: [u8; 32],
    guard_use_claim_digest: [u8; 32],
    platform_bind_claim_digest: [u8; 32],
    android_certificate_digest: [u8; 32],
    android_tbs_digest: [u8; 32],
    android_issuer_key_digest: [u8; 32],
    android_attestation_digest: [u8; 32],
    android_key_cert_claim_digest: [u8; 32],
    registration_receipt_commitment: [u8; 32],
    guard_bundle_digest: [u8; 32],
}

impl RegisteredPlatformP256CurrentHelperViewV2 for OfflineCashGuardBundleStatementV2 {
    fn operation_v2(&self) -> u8 {
        self.operation as u8
    }

    fn release_id_v2(&self) -> &[u8; 32] {
        &self.release_id
    }

    fn context_digest_v2(&self) -> &[u8; 32] {
        &self.context_digest
    }

    fn current_head_v2(&self) -> &[u8; 32] {
        &self.current_head
    }

    fn current_lineage_digest_v2(&self) -> &[u8; 32] {
        &self.current_lineage_digest
    }

    fn transition_digest_v2(&self) -> &[u8; 32] {
        &self.transition_digest
    }

    fn wallet_binding_v2(&self) -> &[u8; 32] {
        &self.wallet_binding
    }

    fn hardware_policy_id_v2(&self) -> &[u8; 32] {
        &self.hardware_policy_id
    }

    fn guard_device_id_v2(&self) -> &[u8; 32] {
        &self.guard_device_id
    }

    fn current_guard_binding_v2(&self) -> &[u8; 32] {
        &self.current_guard_binding
    }

    fn next_guard_binding_v2(&self) -> &[u8; 32] {
        &self.next_guard_binding
    }

    fn from_sequence_v2(&self) -> u64 {
        self.from_sequence
    }

    fn to_sequence_v2(&self) -> u64 {
        self.to_sequence
    }

    fn platform_key_digest_v2(&self) -> &[u8; 32] {
        &self.platform_key_digest
    }

    fn platform_message_digest_v2(&self) -> &[u8; 32] {
        &self.platform_message_digest
    }
}

impl OfflineCashGuardBundleStatementV2 {
    fn validate(&self) -> Result<(), OfflineCashGuardBundleProvenanceErrorV2> {
        let required = [
            self.release_id,
            self.context_digest,
            self.current_head,
            self.current_lineage_digest,
            self.transition_digest,
            self.wallet_binding,
            self.hardware_policy_id,
            self.guard_device_id,
            self.current_guard_binding,
            self.next_guard_binding,
            self.platform_key_digest,
            self.platform_message_digest,
            self.guard_use_claim_digest,
            self.platform_bind_claim_digest,
            self.guard_bundle_digest,
        ];
        let android = [
            self.android_certificate_digest,
            self.android_tbs_digest,
            self.android_issuer_key_digest,
            self.android_attestation_digest,
            self.android_key_cert_claim_digest,
        ];
        if required.into_iter().any(|digest| digest == [0; 32])
            || self.from_sequence.checked_add(1) != Some(self.to_sequence)
            || self.current_guard_binding == self.next_guard_binding
            || self.current_head == self.transition_digest
            || (self.android_key_cert_present
                && android.into_iter().any(|digest| digest == [0; 32]))
            || (!self.android_key_cert_present
                && android.into_iter().any(|digest| digest != [0; 32]))
            || (self.p256_signature_present && self.registration_receipt_commitment == [0; 32])
            || (!self.p256_signature_present && self.registration_receipt_commitment != [0; 32])
        {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::InvalidCurrentHelper);
        }
        Ok(())
    }

    pub(super) const fn android_key_cert_present(&self) -> bool {
        self.android_key_cert_present
    }

    pub(super) const fn p256_signature_present(&self) -> bool {
        self.p256_signature_present
    }

    pub(super) const fn registration_receipt_commitment(&self) -> &[u8; 32] {
        &self.registration_receipt_commitment
    }

    pub(super) const fn child_plan(&self) -> [OfflineCashGuardBundleChildSlotV2; 4] {
        [
            OfflineCashGuardBundleChildSlotV2 {
                lineage_role: OfflineCashGuardBundleLineageChildRoleV2::GuardUse,
                protocol_role: OfflineCashHalo2CircuitRoleV2::GuardUse,
                presence: OfflineCashGuardBundleChildPresenceV2::Required,
            },
            OfflineCashGuardBundleChildSlotV2 {
                lineage_role: OfflineCashGuardBundleLineageChildRoleV2::PlatformBind,
                protocol_role: OfflineCashHalo2CircuitRoleV2::PlatformBind,
                presence: OfflineCashGuardBundleChildPresenceV2::Required,
            },
            OfflineCashGuardBundleChildSlotV2 {
                lineage_role: OfflineCashGuardBundleLineageChildRoleV2::AndroidKeyCert,
                protocol_role: OfflineCashHalo2CircuitRoleV2::AndroidKeyCert,
                presence: if self.android_key_cert_present {
                    OfflineCashGuardBundleChildPresenceV2::Present
                } else {
                    OfflineCashGuardBundleChildPresenceV2::CanonicallyAbsent
                },
            },
            OfflineCashGuardBundleChildSlotV2 {
                lineage_role: OfflineCashGuardBundleLineageChildRoleV2::P256Signature,
                protocol_role: OfflineCashHalo2CircuitRoleV2::P256Signature,
                presence: if self.p256_signature_present {
                    OfflineCashGuardBundleChildPresenceV2::Present
                } else {
                    OfflineCashGuardBundleChildPresenceV2::CanonicallyAbsent
                },
            },
        ]
    }
}

/// Uninhabited authority for authenticating the complete current helper.
pub(super) enum OfflineCashCurrentHelperAuthenticationAuthorityV2 {}
/// Uninhabited authority for current-helper liveness/freshness.
pub(super) enum OfflineCashCurrentHelperFreshnessAuthorityV2 {}

/// Move-only owner of one complete authenticated current helper.
pub(super) struct AuthenticatedOfflineCashCurrentHelperOwnerV2 {
    statement: OfflineCashGuardBundleStatementV2,
}

impl AuthenticatedOfflineCashCurrentHelperOwnerV2 {
    pub(super) const fn statement(&self) -> &OfflineCashGuardBundleStatementV2 {
        &self.statement
    }

    #[cfg(test)]
    pub(super) fn from_test_statement_v2(
        statement: OfflineCashGuardBundleStatementV2,
    ) -> Result<Self, OfflineCashGuardBundleProvenanceErrorV2> {
        statement.validate()?;
        Ok(Self { statement })
    }
}

/// Sole production constructor; uncallable until current-helper authentication exists.
pub(super) fn authenticate_offline_cash_current_helper_v2(
    _statement: OfflineCashGuardBundleStatementV2,
    authentication: OfflineCashCurrentHelperAuthenticationAuthorityV2,
    _freshness: OfflineCashCurrentHelperFreshnessAuthorityV2,
) -> Result<AuthenticatedOfflineCashCurrentHelperOwnerV2, OfflineCashGuardBundleProvenanceErrorV2> {
    match authentication {}
}

/// Optional child slot state. Absent children retain their fixed slot and are gated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashGuardBundleChildPresenceV2 {
    Required,
    Present,
    CanonicallyAbsent,
}

/// One slot in the exact four-child GuardBundle order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashGuardBundleChildSlotV2 {
    pub(super) lineage_role: OfflineCashGuardBundleLineageChildRoleV2,
    pub(super) protocol_role: OfflineCashHalo2CircuitRoleV2,
    pub(super) presence: OfflineCashGuardBundleChildPresenceV2,
}

/// Host-side failure while constructing the exact ABI or joining provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashGuardBundleProvenanceErrorV2 {
    InvalidCurrentHelper,
    InvalidHeader,
    InvalidPriorLineage,
    UnauthenticatedBootstrap,
    ParityMismatch,
    NonCanonicalPacking,
    MissingRegisteredP256Source,
    UnexpectedRegisteredP256Source,
    RegisteredP256ContextMismatch,
    RegisteredP256ReceiptMismatch,
    RegisteredP256RoleOrOrderMismatch,
    CurrentAccumulatorParityMismatch,
    VerificationUnavailable,
}

impl fmt::Display for OfflineCashGuardBundleProvenanceErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCurrentHelper => "offline-cash V2 GuardBundle current helper is invalid",
            Self::InvalidHeader => "offline-cash V2 GuardBundle ABI header is invalid",
            Self::InvalidPriorLineage => {
                "offline-cash V2 GuardBundle prior lineage is non-canonical"
            }
            Self::UnauthenticatedBootstrap => {
                "offline-cash V2 GuardBundle bootstrap lineage is unauthenticated"
            }
            Self::ParityMismatch => "offline-cash V2 GuardBundle parity mismatch",
            Self::NonCanonicalPacking => {
                "offline-cash V2 GuardBundle packed instances are non-canonical"
            }
            Self::MissingRegisteredP256Source => {
                "offline-cash V2 GuardBundle requires its registered P-256 source"
            }
            Self::UnexpectedRegisteredP256Source => {
                "offline-cash V2 GuardBundle received a disabled P-256 source"
            }
            Self::RegisteredP256ContextMismatch => {
                "offline-cash V2 GuardBundle and registered P-256 current helpers differ"
            }
            Self::RegisteredP256ReceiptMismatch => {
                "offline-cash V2 GuardBundle registration receipt commitment differs"
            }
            Self::RegisteredP256RoleOrOrderMismatch => {
                "offline-cash V2 GuardBundle P-256 provenance has wrong role or parity order"
            }
            Self::CurrentAccumulatorParityMismatch => {
                "offline-cash V2 GuardBundle current accumulator has wrong parity"
            }
            Self::VerificationUnavailable => {
                "offline-cash V2 GuardBundle verification is unavailable"
            }
        })
    }
}

impl std::error::Error for OfflineCashGuardBundleProvenanceErrorV2 {}

/// Exact field-neutral GuardBundle public instances for one parity.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct OfflineCashGuardBundlePublicInstancesV2 {
    parity: OfflineCashHalo2ParityV2,
    words: [u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2],
}

impl OfflineCashGuardBundlePublicInstancesV2 {
    pub(super) fn eq(
        statement: &OfflineCashGuardBundleStatementV2,
        prior_lineage: &OfflineCashEqParentLineageV2,
    ) -> Result<Self, OfflineCashGuardBundleProvenanceErrorV2> {
        if prior_lineage.is_bootstrap() {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::UnauthenticatedBootstrap);
        }
        Self::build(
            OfflineCashHalo2ParityV2::Eq,
            statement,
            prior_lineage.encode(),
        )
    }

    pub(super) fn ep(
        statement: &OfflineCashGuardBundleStatementV2,
        prior_lineage: &OfflineCashEpParentLineageV2,
    ) -> Result<Self, OfflineCashGuardBundleProvenanceErrorV2> {
        if prior_lineage.is_bootstrap() {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::UnauthenticatedBootstrap);
        }
        Self::build(
            OfflineCashHalo2ParityV2::Ep,
            statement,
            prior_lineage.encode(),
        )
    }

    fn build(
        parity: OfflineCashHalo2ParityV2,
        statement: &OfflineCashGuardBundleStatementV2,
        prior_lineage: [u8; OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2 as usize],
    ) -> Result<Self, OfflineCashGuardBundleProvenanceErrorV2> {
        statement.validate()?;
        let mut words = [0_u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2];
        words[..16].copy_from_slice(&[
            OFFLINE_CASH_GUARD_BUNDLE_ABI_VERSION_V2,
            OFFLINE_CASH_GUARD_BUNDLE_SOURCE_PROFILE_VERSION_V2,
            OFFLINE_CASH_HALO2_K_V2,
            parity as u32,
            OfflineCashHalo2CircuitRoleV2::GuardBundle as u32,
            statement.operation as u32,
            u32::from(statement.android_key_cert_present),
            u32::from(statement.p256_signature_present),
            statement.from_sequence as u32,
            (statement.from_sequence >> 32) as u32,
            statement.to_sequence as u32,
            (statement.to_sequence >> 32) as u32,
            OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORDS_V2 as u32,
            OFFLINE_CASH_GUARD_BUNDLE_DIGEST_FIELDS_V2 as u32,
            OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2 as u32,
            OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2 as u32,
        ]);
        for (offset, digest) in [
            (
                OFFLINE_CASH_GUARD_BUNDLE_PROTOCOL_WORD_START_V2,
                offline_cash_halo2_protocol_source_identity_v2(
                    parity,
                    OfflineCashHalo2CircuitRoleV2::GuardBundle,
                )
                .digest(),
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_RELEASE_WORD_START_V2,
                statement.release_id,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_CONTEXT_WORD_START_V2,
                statement.context_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_CURRENT_HEAD_WORD_START_V2,
                statement.current_head,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_CURRENT_LINEAGE_WORD_START_V2,
                statement.current_lineage_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_TRANSITION_WORD_START_V2,
                statement.transition_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_WALLET_WORD_START_V2,
                statement.wallet_binding,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_POLICY_WORD_START_V2,
                statement.hardware_policy_id,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_DEVICE_WORD_START_V2,
                statement.guard_device_id,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_CURRENT_GUARD_WORD_START_V2,
                statement.current_guard_binding,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_NEXT_GUARD_WORD_START_V2,
                statement.next_guard_binding,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_KEY_WORD_START_V2,
                statement.platform_key_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_MESSAGE_WORD_START_V2,
                statement.platform_message_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_GUARD_USE_CLAIM_WORD_START_V2,
                statement.guard_use_claim_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_BIND_CLAIM_WORD_START_V2,
                statement.platform_bind_claim_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_ANDROID_CERTIFICATE_WORD_START_V2,
                statement.android_certificate_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_ANDROID_TBS_WORD_START_V2,
                statement.android_tbs_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_ANDROID_ISSUER_KEY_WORD_START_V2,
                statement.android_issuer_key_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_ANDROID_ATTESTATION_WORD_START_V2,
                statement.android_attestation_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_ANDROID_CLAIM_WORD_START_V2,
                statement.android_key_cert_claim_digest,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_REGISTRATION_RECEIPT_WORD_START_V2,
                statement.registration_receipt_commitment,
            ),
            (
                OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORD_START_V2,
                statement.guard_bundle_digest,
            ),
        ] {
            write_digest_words_v2(&mut words, offset, digest);
        }
        for (target, chunk) in words[OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORD_START_V2..]
            .iter_mut()
            .zip(prior_lineage.chunks_exact(4))
        {
            *target = u32::from_le_bytes(chunk.try_into().expect("four-byte lineage limb"));
        }
        let instances = Self { parity, words };
        instances.validate_structure()?;
        Ok(instances)
    }

    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV2 {
        self.parity
    }

    pub(super) const fn words(&self) -> &[u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2] {
        &self.words
    }

    pub(super) fn eq_prior_lineage(
        &self,
    ) -> Result<OfflineCashEqParentLineageV2, OfflineCashGuardBundleProvenanceErrorV2> {
        if self.parity != OfflineCashHalo2ParityV2::Eq {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::ParityMismatch);
        }
        OfflineCashEqParentLineageV2::decode(&self.prior_lineage_bytes())
            .map_err(map_lineage_error_v2)
            .and_then(reject_eq_bootstrap_v2)
    }

    pub(super) fn ep_prior_lineage(
        &self,
    ) -> Result<OfflineCashEpParentLineageV2, OfflineCashGuardBundleProvenanceErrorV2> {
        if self.parity != OfflineCashHalo2ParityV2::Ep {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::ParityMismatch);
        }
        OfflineCashEpParentLineageV2::decode(&self.prior_lineage_bytes())
            .map_err(map_lineage_error_v2)
            .and_then(reject_ep_bootstrap_v2)
    }

    pub(super) fn packed_cell_bytes(
        &self,
    ) -> [[u8; OFFLINE_CASH_GUARD_BUNDLE_PACKED_CELL_BYTES_V2];
        OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2] {
        std::array::from_fn(|cell_index| {
            let mut bytes = [0_u8; OFFLINE_CASH_GUARD_BUNDLE_PACKED_CELL_BYTES_V2];
            let start = cell_index * OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2;
            for (lane, word) in self.words
                [start..start + OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2]
                .iter()
                .enumerate()
            {
                bytes[lane * 4..lane * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            bytes
        })
    }

    pub(super) fn field_instances<F: PrimeField>(
        &self,
    ) -> [F; OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2] {
        std::array::from_fn(|cell_index| {
            let start = cell_index * OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2;
            pack_words_as_field_v2::<F>(
                &self.words[start..start + OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2],
            )
        })
    }

    pub(super) fn unpack_cell_bytes(
        parity: OfflineCashHalo2ParityV2,
        cells: &[[u8; OFFLINE_CASH_GUARD_BUNDLE_PACKED_CELL_BYTES_V2]],
    ) -> Result<
        [u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2],
        OfflineCashGuardBundleProvenanceErrorV2,
    > {
        if cells.len() != OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2 {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::NonCanonicalPacking);
        }
        let mut words = [0_u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2];
        for (index, word) in words.iter_mut().enumerate() {
            let cell = &cells[index / OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2];
            let offset = index % OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2 * 4;
            *word = u32::from_le_bytes(
                cell[offset..offset + 4]
                    .try_into()
                    .expect("one packed GuardBundle word is four bytes"),
            );
        }
        let instances = Self { parity, words };
        instances.validate_structure()?;
        Ok(instances.words)
    }

    fn prior_lineage_bytes(
        &self,
    ) -> [u8; OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2 as usize] {
        let mut bytes = [0_u8; OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2 as usize];
        for (chunk, word) in bytes
            .chunks_exact_mut(4)
            .zip(&self.words[OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORD_START_V2..])
        {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    fn validate_structure(&self) -> Result<(), OfflineCashGuardBundleProvenanceErrorV2> {
        if self.words[..16]
            != [
                OFFLINE_CASH_GUARD_BUNDLE_ABI_VERSION_V2,
                OFFLINE_CASH_GUARD_BUNDLE_SOURCE_PROFILE_VERSION_V2,
                OFFLINE_CASH_HALO2_K_V2,
                self.parity as u32,
                OfflineCashHalo2CircuitRoleV2::GuardBundle as u32,
                self.words[5],
                self.words[6],
                self.words[7],
                self.words[8],
                self.words[9],
                self.words[10],
                self.words[11],
                OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORDS_V2 as u32,
                OFFLINE_CASH_GUARD_BUNDLE_DIGEST_FIELDS_V2 as u32,
                OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2 as u32,
                OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2 as u32,
            ]
            || !matches!(self.words[5], 1 | 2)
            || self.words[6] > 1
            || self.words[7] > 1
            || (u64::from(self.words[8]) | (u64::from(self.words[9]) << 32)).checked_add(1)
                != Some(u64::from(self.words[10]) | (u64::from(self.words[11]) << 32))
            || read_digest_words_v2(
                &self.words,
                OFFLINE_CASH_GUARD_BUNDLE_PROTOCOL_WORD_START_V2,
            ) != offline_cash_halo2_protocol_source_identity_v2(
                self.parity,
                OfflineCashHalo2CircuitRoleV2::GuardBundle,
            )
            .digest()
            || read_digest_words_v2(
                &self.words,
                OFFLINE_CASH_GUARD_BUNDLE_CURRENT_HEAD_WORD_START_V2,
            ) == read_digest_words_v2(
                &self.words,
                OFFLINE_CASH_GUARD_BUNDLE_TRANSITION_WORD_START_V2,
            )
            || read_digest_words_v2(
                &self.words,
                OFFLINE_CASH_GUARD_BUNDLE_CURRENT_GUARD_WORD_START_V2,
            ) == read_digest_words_v2(
                &self.words,
                OFFLINE_CASH_GUARD_BUNDLE_NEXT_GUARD_WORD_START_V2,
            )
        {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::InvalidHeader);
        }

        for offset in [
            OFFLINE_CASH_GUARD_BUNDLE_PROTOCOL_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_RELEASE_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_CONTEXT_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_CURRENT_HEAD_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_CURRENT_LINEAGE_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_TRANSITION_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_WALLET_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_POLICY_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_DEVICE_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_CURRENT_GUARD_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_NEXT_GUARD_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_KEY_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_MESSAGE_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_GUARD_USE_CLAIM_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_PLATFORM_BIND_CLAIM_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORD_START_V2,
        ] {
            if digest_words_are_zero_v2(&self.words, offset) {
                return Err(OfflineCashGuardBundleProvenanceErrorV2::InvalidHeader);
            }
        }
        for offset in [
            OFFLINE_CASH_GUARD_BUNDLE_ANDROID_CERTIFICATE_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_ANDROID_TBS_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_ANDROID_ISSUER_KEY_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_ANDROID_ATTESTATION_WORD_START_V2,
            OFFLINE_CASH_GUARD_BUNDLE_ANDROID_CLAIM_WORD_START_V2,
        ] {
            if (self.words[6] == 1) == digest_words_are_zero_v2(&self.words, offset) {
                return Err(OfflineCashGuardBundleProvenanceErrorV2::InvalidHeader);
            }
        }
        if (self.words[7] == 1)
            == digest_words_are_zero_v2(
                &self.words,
                OFFLINE_CASH_GUARD_BUNDLE_REGISTRATION_RECEIPT_WORD_START_V2,
            )
        {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::InvalidHeader);
        }
        match self.parity {
            OfflineCashHalo2ParityV2::Eq => self.eq_prior_lineage().map(|_| ()),
            OfflineCashHalo2ParityV2::Ep => self.ep_prior_lineage().map(|_| ()),
        }
    }
}

fn map_lineage_error_v2(
    _error: OfflineCashParentLineageCodecErrorV2,
) -> OfflineCashGuardBundleProvenanceErrorV2 {
    OfflineCashGuardBundleProvenanceErrorV2::InvalidPriorLineage
}

fn reject_eq_bootstrap_v2(
    lineage: OfflineCashEqParentLineageV2,
) -> Result<OfflineCashEqParentLineageV2, OfflineCashGuardBundleProvenanceErrorV2> {
    if lineage.is_bootstrap() {
        Err(OfflineCashGuardBundleProvenanceErrorV2::UnauthenticatedBootstrap)
    } else {
        Ok(lineage)
    }
}

fn reject_ep_bootstrap_v2(
    lineage: OfflineCashEpParentLineageV2,
) -> Result<OfflineCashEpParentLineageV2, OfflineCashGuardBundleProvenanceErrorV2> {
    if lineage.is_bootstrap() {
        Err(OfflineCashGuardBundleProvenanceErrorV2::UnauthenticatedBootstrap)
    } else {
        Ok(lineage)
    }
}

fn write_digest_words_v2(
    words: &mut [u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2],
    offset: usize,
    digest: [u8; 32],
) {
    for (target, chunk) in words[offset..offset + OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORDS_V2]
        .iter_mut()
        .zip(digest.chunks_exact(4))
    {
        *target = u32::from_le_bytes(chunk.try_into().expect("four-byte digest limb"));
    }
}

fn read_digest_words_v2(
    words: &[u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2],
    offset: usize,
) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (chunk, word) in digest
        .chunks_exact_mut(4)
        .zip(&words[offset..offset + OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORDS_V2])
    {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    digest
}

fn digest_words_are_zero_v2(
    words: &[u32; OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2],
    offset: usize,
) -> bool {
    words[offset..offset + OFFLINE_CASH_GUARD_BUNDLE_DIGEST_WORDS_V2]
        .iter()
        .all(|word| *word == 0)
}

fn pack_words_as_field_v2<F: PrimeField>(words: &[u32]) -> F {
    assert!(words.len() <= OFFLINE_CASH_GUARD_BUNDLE_WORDS_PER_INSTANCE_V2);
    let radix = F::from(1_u64 << 32);
    words.iter().rev().fold(F::ZERO, |accumulator, word| {
        accumulator * radix + F::from(u64::from(*word))
    })
}

/// Move-only optional role-6 source retained by the provenance owner.
pub(super) enum OfflineCashRegisteredP256ChildProvenanceV2 {
    CanonicallyAbsent,
    Present(UnverifiedRegisteredPlatformP256CircuitCandidatesV2),
}

/// Structurally joined current helper, optional role 6, and Eq/Ep public ABIs.
///
/// This value is explicitly unverified: it owns source provenance but has no
/// recursive proof, current accumulator, or verification authority.
pub(super) struct UnverifiedOfflineCashGuardBundleProvenanceV2 {
    current_helper: AuthenticatedOfflineCashCurrentHelperOwnerV2,
    registered_p256: OfflineCashRegisteredP256ChildProvenanceV2,
    eq_instances: OfflineCashGuardBundlePublicInstancesV2,
    ep_instances: OfflineCashGuardBundlePublicInstancesV2,
}

impl UnverifiedOfflineCashGuardBundleProvenanceV2 {
    pub(super) const fn current_helper(&self) -> &AuthenticatedOfflineCashCurrentHelperOwnerV2 {
        &self.current_helper
    }

    pub(super) const fn eq_instances(&self) -> &OfflineCashGuardBundlePublicInstancesV2 {
        &self.eq_instances
    }

    pub(super) const fn ep_instances(&self) -> &OfflineCashGuardBundlePublicInstancesV2 {
        &self.ep_instances
    }

    pub(super) const fn has_registered_p256(&self) -> bool {
        matches!(
            &self.registered_p256,
            OfflineCashRegisteredP256ChildProvenanceV2::Present(_)
        )
    }
}

/// Join the sole complete current-helper owner to optional role 6 and both prior lineages.
pub(super) fn assemble_unverified_offline_cash_guard_bundle_provenance_v2(
    current_helper: AuthenticatedOfflineCashCurrentHelperOwnerV2,
    registered_p256: OfflineCashRegisteredP256ChildProvenanceV2,
    eq_prior_lineage: &OfflineCashEqParentLineageV2,
    ep_prior_lineage: &OfflineCashEpParentLineageV2,
) -> Result<UnverifiedOfflineCashGuardBundleProvenanceV2, OfflineCashGuardBundleProvenanceErrorV2> {
    current_helper.statement.validate()?;
    match (
        &registered_p256,
        current_helper.statement.p256_signature_present,
    ) {
        (OfflineCashRegisteredP256ChildProvenanceV2::CanonicallyAbsent, true) => {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::MissingRegisteredP256Source);
        }
        (OfflineCashRegisteredP256ChildProvenanceV2::Present(_), false) => {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::UnexpectedRegisteredP256Source);
        }
        (OfflineCashRegisteredP256ChildProvenanceV2::CanonicallyAbsent, false) => {}
        (OfflineCashRegisteredP256ChildProvenanceV2::Present(candidates), true) => {
            let source_pair = candidates.source_pair();
            if !source_pair
                .authenticated_current_helper()
                .matches_current_helper_view_v2(&current_helper.statement)
            {
                return Err(OfflineCashGuardBundleProvenanceErrorV2::RegisteredP256ContextMismatch);
            }
            if source_pair
                .authenticated_current_helper()
                .durable_identity()
                .receipt_commitment()
                != current_helper.statement.registration_receipt_commitment()
            {
                return Err(OfflineCashGuardBundleProvenanceErrorV2::RegisteredP256ReceiptMismatch);
            }
            let [eq, ep] = candidates.provenance();
            if eq.parity() != OfflineCashHalo2ParityV2::Eq
                || ep.parity() != OfflineCashHalo2ParityV2::Ep
                || eq.role() != OfflineCashHalo2CircuitRoleV2::P256Signature
                || ep.role() != OfflineCashHalo2CircuitRoleV2::P256Signature
                || eq.statement_bytes() != ep.statement_bytes()
            {
                return Err(
                    OfflineCashGuardBundleProvenanceErrorV2::RegisteredP256RoleOrOrderMismatch,
                );
            }
        }
    }

    let eq_instances =
        OfflineCashGuardBundlePublicInstancesV2::eq(&current_helper.statement, eq_prior_lineage)?;
    let ep_instances =
        OfflineCashGuardBundlePublicInstancesV2::ep(&current_helper.statement, ep_prior_lineage)?;
    Ok(UnverifiedOfflineCashGuardBundleProvenanceV2 {
        current_helper,
        registered_p256,
        eq_instances,
        ep_instances,
    })
}

/// Uninhabited authority for verified GuardBundle proof accumulation.
pub(super) enum OfflineCashGuardBundleProofVerifierAuthorityV2 {}

/// Move-only handoff that could exist only after both GuardBundle proofs verify.
pub(super) struct VerifiedOfflineCashGuardBundleStateHandoffV2 {
    provenance: UnverifiedOfflineCashGuardBundleProvenanceV2,
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: CanonicalStateAccumulatorV2,
}

/// Sole production constructor; impossible while the verifier authority is uninhabited.
pub(super) fn verify_offline_cash_guard_bundle_for_state_v2(
    _provenance: UnverifiedOfflineCashGuardBundleProvenanceV2,
    _eq_current: CanonicalStateAccumulatorV2,
    _ep_current: CanonicalStateAccumulatorV2,
    authority: OfflineCashGuardBundleProofVerifierAuthorityV2,
) -> Result<VerifiedOfflineCashGuardBundleStateHandoffV2, OfflineCashGuardBundleProvenanceErrorV2> {
    match authority {}
}

impl VerifiedOfflineCashGuardBundleStateHandoffV2 {
    #[cfg(test)]
    pub(super) fn from_test_verified_parts_v2(
        provenance: UnverifiedOfflineCashGuardBundleProvenanceV2,
        eq_current: CanonicalStateAccumulatorV2,
        ep_current: CanonicalStateAccumulatorV2,
    ) -> Result<Self, OfflineCashGuardBundleProvenanceErrorV2> {
        if eq_current.parity() != StateRecursiveFoldParityV2::Eq
            || ep_current.parity() != StateRecursiveFoldParityV2::Ep
        {
            return Err(OfflineCashGuardBundleProvenanceErrorV2::CurrentAccumulatorParityMismatch);
        }
        let eq_prior = CanonicalStateAccumulatorV2::decode(
            StateRecursiveFoldParityV2::Eq,
            &provenance.eq_instances.eq_prior_lineage()?.encode(),
        )
        .map_err(|_| OfflineCashGuardBundleProvenanceErrorV2::InvalidPriorLineage)?;
        let ep_prior = CanonicalStateAccumulatorV2::decode(
            StateRecursiveFoldParityV2::Ep,
            &provenance.ep_instances.ep_prior_lineage()?.encode(),
        )
        .map_err(|_| OfflineCashGuardBundleProvenanceErrorV2::InvalidPriorLineage)?;
        Ok(Self {
            provenance,
            eq_current,
            eq_prior,
            ep_current,
            ep_prior,
        })
    }

    pub(super) fn into_state_accumulator_parts_v2(
        self,
    ) -> OfflineCashGuardBundleStateAccumulatorPartsV2 {
        OfflineCashGuardBundleStateAccumulatorPartsV2 {
            provenance_seal: OfflineCashGuardBundleStateProvenanceSealV2(self.provenance),
            eq_current: self.eq_current,
            eq_prior: self.eq_prior,
            ep_current: self.ep_current,
            ep_prior: self.ep_prior,
        }
    }
}

/// Build the narrow, canonically absent-child GuardBundle handoff used only by
/// the native relation ownership tests.
#[cfg(test)]
pub(super) fn guard_bundle_state_handoff_for_native_relation_test_v2(
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: &CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: &CanonicalStateAccumulatorV2,
) -> Result<VerifiedOfflineCashGuardBundleStateHandoffV2, OfflineCashGuardBundleProvenanceErrorV2> {
    if eq_prior.parity() != StateRecursiveFoldParityV2::Eq
        || ep_prior.parity() != StateRecursiveFoldParityV2::Ep
    {
        return Err(OfflineCashGuardBundleProvenanceErrorV2::ParityMismatch);
    }
    let eq_prior = OfflineCashEqParentLineageV2::decode(eq_prior.as_bytes())
        .map_err(|_| OfflineCashGuardBundleProvenanceErrorV2::InvalidPriorLineage)?;
    let ep_prior = OfflineCashEpParentLineageV2::decode(ep_prior.as_bytes())
        .map_err(|_| OfflineCashGuardBundleProvenanceErrorV2::InvalidPriorLineage)?;
    let current_helper = AuthenticatedOfflineCashCurrentHelperOwnerV2::from_test_statement_v2(
        OfflineCashGuardBundleStatementV2 {
            operation: OfflineCashGuardBundleOperationV2::SendSplit,
            android_key_cert_present: false,
            p256_signature_present: false,
            from_sequence: 7,
            to_sequence: 8,
            release_id: [0x11; 32],
            context_digest: [0x12; 32],
            current_head: [0x13; 32],
            current_lineage_digest: [0x14; 32],
            transition_digest: [0x15; 32],
            wallet_binding: [0x16; 32],
            hardware_policy_id: [0x17; 32],
            guard_device_id: [0x18; 32],
            current_guard_binding: [0x19; 32],
            next_guard_binding: [0x1a; 32],
            platform_key_digest: [0x1b; 32],
            platform_message_digest: [0x1c; 32],
            guard_use_claim_digest: [0x31; 32],
            platform_bind_claim_digest: [0x32; 32],
            android_certificate_digest: [0; 32],
            android_tbs_digest: [0; 32],
            android_issuer_key_digest: [0; 32],
            android_attestation_digest: [0; 32],
            android_key_cert_claim_digest: [0; 32],
            registration_receipt_commitment: [0; 32],
            guard_bundle_digest: [0x33; 32],
        },
    )?;
    let provenance = assemble_unverified_offline_cash_guard_bundle_provenance_v2(
        current_helper,
        OfflineCashRegisteredP256ChildProvenanceV2::CanonicallyAbsent,
        &eq_prior,
        &ep_prior,
    )?;
    VerifiedOfflineCashGuardBundleStateHandoffV2::from_test_verified_parts_v2(
        provenance, eq_current, ep_current,
    )
}

/// Opaque ownership seal retained by the provenance-bound STATE input pair.
pub(super) struct OfflineCashGuardBundleStateProvenanceSealV2(
    UnverifiedOfflineCashGuardBundleProvenanceV2,
);

impl OfflineCashGuardBundleStateProvenanceSealV2 {
    pub(super) const fn provenance(&self) -> &UnverifiedOfflineCashGuardBundleProvenanceV2 {
        &self.0
    }
}

/// Exact accumulator parts emitted by the one verified handoff.
pub(super) struct OfflineCashGuardBundleStateAccumulatorPartsV2 {
    provenance_seal: OfflineCashGuardBundleStateProvenanceSealV2,
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: CanonicalStateAccumulatorV2,
}

impl OfflineCashGuardBundleStateAccumulatorPartsV2 {
    pub(super) fn into_parts_v2(
        self,
    ) -> (
        OfflineCashGuardBundleStateProvenanceSealV2,
        CanonicalStateAccumulatorV2,
        CanonicalStateAccumulatorV2,
        CanonicalStateAccumulatorV2,
        CanonicalStateAccumulatorV2,
    ) {
        (
            self.provenance_seal,
            self.eq_current,
            self.eq_prior,
            self.ep_current,
            self.ep_prior,
        )
    }
}

/// Fail closed before any unverified provenance can cross the verifier boundary.
pub(super) fn fail_closed_offline_cash_guard_bundle_boundary_v2(
    _provenance: UnverifiedOfflineCashGuardBundleProvenanceV2,
) -> Result<Infallible, OfflineCashGuardBundleProvenanceErrorV2> {
    Err(OfflineCashGuardBundleProvenanceErrorV2::VerificationUnavailable)
}

const _: () = assert!(OFFLINE_CASH_GUARD_BUNDLE_LINEAGE_CHILD_ORDER_V2.len() == 4);

#[cfg(test)]
#[path = "guard_bundle_provenance_tests.rs"]
mod tests;
