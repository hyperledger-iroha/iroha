//! Exact compiled contract for the Offline Cash V1 Halo2/IPA profile.
//!
//! These identities describe the only circuit roles and resource bounds that
//! an Offline Cash V1 release may select.  They do not claim that a verifier
//! implementation exists: the backend remains fail-closed until the governed
//! artifacts can also be parsed and verified by the first-party circuit code.

use core::fmt;

use iroha_data_model::offline::{
    OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1,
    OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1, OFFLINE_CASH_HALO2_K_V1,
    OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1, OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1,
    OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1, OFFLINE_CASH_PARAMS_BYTES_V1,
    OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
    OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1, OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1,
    OFFLINE_CASH_SESSION_MAX_BYTES_V1, OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1,
    OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1, OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1,
    OFFLINE_CASH_WIRE_VERSION_V1, OfflineCashArtifactRoleV1,
};
use sha2::{Digest as _, Sha256};

use crate::zk::pasta_ipa_recursion::{PastaIpaInstanceQueryV1, PastaIpaProofShapeV1};

const PROFILE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:halo2-profile";
const PROTOCOL_DOMAIN: &[u8] = b"iroha:offline-cash:v1:halo2-protocol";
const HALO2_BACKEND_REVISION: &[u8] = b"halo2-axiom/0.5.1";
const SNARK_VERIFIER_REVISION: &[u8] = b"snark-verifier/bbfcc721d714bea0d44a27c8fc6c4736e73ca853";
const PCS_REVISION: &[u8] = b"transparent-pasta-ipa/no-trusted-setup";
const TRANSCRIPT_REVISION: &[u8] = b"Blake2bRead+Blake2bWrite/Challenge255";
const KEY_FORMAT_REVISION: &[u8] = b"halo2-axiom/SerdeFormat::Processed";
const HISTORY_REVISION: &[u8] =
    b"paired-pasta-ipa-history/16-canonical-scalars-then-compressed-point/v1";
const AUGMENTED_PROOF_REVISION: &[u8] =
    b"axiom-ipa-proof-prefix+history-folded-generator-suffix32/v1";
const STATE_TOPOLOGY: &[u8] = b"fixed-two-parent/send-split+receive-fold/no-hop-cap/v1";
pub(super) const STATE_PUBLIC_INSTANCE_ABI_REVISION_V1: &[u8] =
    b"u32le-v1/header(abi,wire,k,parity,operation,parent-count,digest-words,history-words)/digests(release,protocol,semantic,context,request,parent0,parent1,result,link,transition)/amount4/scale1/history136/pack7x32le/one-column/last-zero2/op1-send(sender-before,receiver-before,sender-after,credit)/op2-receive(balance,credit,next,send-transition)";
const STATE_PUBLIC_BINDING_CIRCUIT_REVISION_V1: &[u8] =
    b"state-relation/axiom-floorplanner-v1-two-pass+witnessless-measurement/bit32+pack224/op-in-{1,2}/u128le-4x32-carry-zero-ends/exact-next-u64-no-overflow/nonzero-9x32byte-private-bindings+send-seed-op1+8x32limb-running-sum9rows-maxrot1/sha256-raw-byte-order/frame-u64le-lengths+u16le-head-version/balance335(sequence+lineage)+credit278+lineage361+send-seed365+send-branches101+103+receive-opening273+receive-transition341+receive-semantic430/jobs-ordered(6,6,5,6,6,2,2,5,6,7)-blocks/five-lanes-shared-table/send-seed+branch-openings-op1-gated/receive-opening+transition+semantic-op2-gated/send-transition+semantic-deferred/recursion+guard-helper-deferred/v9";
pub(super) const HELPER_PUBLIC_INSTANCE_ABI_REVISION_V1: &[u8] =
    b"u32le-v1/header(abi,wire,k,parity,role,operation,android-present,digest-words,from-lo,from-hi,to-lo,to-hi,digest-count,words-per-cell,cells,reserved-zero)/digests(protocol,release,context,current-head,current-lineage,transition,wallet,policy,device,current-guard,next-guard,platform-key,platform-message,guard-use-claim,platform-bind-claim,android-cert,android-tbs,android-issuer-key,android-attestation,android-claim,bundle)/pack7x32le/one-column/last-zero5/common-statement-all-helper-roles";
const HELPER_PUBLIC_BINDING_CIRCUIT_REVISION_V1: &[u8] =
    b"helper-binding/simple-floorplanner/bit32+pack224/role-specific-fixed-protocol/op-in-{1,2}/android-flag-boolean/exact-next-u64-no-overflow/required-16-digests-nonzero/optional-five-android-digests-all-zero-or-all-nonzero/current-guard-ne-next-guard/current-head-ne-transition/sum-of-eight-u32-squared-differences-below-2^67/p256-low-s+message+normalized-android-cert-host-preflight-only/p256-circuit+der-keymint+child-ipa-recursion-deferred/non-authorizing/v1";
pub(super) const OFFLINE_CASH_STATE_ABI_WORDS_V1: u32 = 229;
pub(super) const OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1: u32 = 7;
pub(super) const OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1: u32 = 1;
pub(super) const OFFLINE_CASH_STATE_INSTANCE_CELLS_V1: u32 = 33;
pub(super) const OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1: u32 = 50;
pub(super) const OFFLINE_CASH_STATE_SHA_LANES_V1: u32 = 5;
pub(super) const OFFLINE_CASH_STATE_SHA_JOBS_V1: u32 = 10;
pub(super) const OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1: [u32; 10] = [6, 6, 5, 6, 6, 2, 2, 5, 6, 7];
pub(super) const OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1: u32 = 51;
pub(super) const OFFLINE_CASH_HELPER_ABI_WORDS_V1: u32 = 184;
pub(super) const OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1: u32 = 7;
pub(super) const OFFLINE_CASH_HELPER_INSTANCE_COLUMNS_V1: u32 = 1;
pub(super) const OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1: u32 = 27;
pub(super) const OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1: u32 = 32;

const _: () = assert!(
    OFFLINE_CASH_STATE_INSTANCE_CELLS_V1
        == OFFLINE_CASH_STATE_ABI_WORDS_V1.div_ceil(OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1)
);
const _: () =
    assert!(OFFLINE_CASH_STATE_INSTANCE_CELLS_V1 <= OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1);
const _: () =
    assert!(OFFLINE_CASH_STATE_SHA_JOBS_V1 as usize == OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1.len());
const _: () = assert!(
    OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[0]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[1]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[2]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[3]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[4]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[5]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[6]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[7]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[8]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[9]
        == OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1
);
const _: () = assert!(OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1 == 51);
const _: () = assert!(
    OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1
        == OFFLINE_CASH_HELPER_ABI_WORDS_V1.div_ceil(OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1)
);
const _: () =
    assert!(OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1 <= OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1);

/// Pasta parity selected by one Offline Cash V1 circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum OfflineCashHalo2ParityV1 {
    /// Eq parity.
    Eq = 1,
    /// Ep parity.
    Ep = 2,
}

impl OfflineCashHalo2ParityV1 {
    /// Exact canonically ordered parity inventory.
    pub(crate) const ALL: [Self; 2] = [Self::Eq, Self::Ep];

    const fn curve_contract(self) -> &'static [u8] {
        match self {
            Self::Eq => b"pasta/EqAffine",
            Self::Ep => b"pasta/EpAffine",
        }
    }
}

/// Finite circuit-role inventory for Offline Cash V1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum OfflineCashHalo2CircuitRoleV1 {
    /// Recursive balance state transition.
    State = 1,
    /// Exact-next hardware guard-use relation.
    GuardUse = 2,
    /// Platform hardware binding relation.
    PlatformBind = 3,
    /// Android hardware-key certificate relation.
    AndroidKeyCert = 4,
    /// Aggregated guard bundle relation.
    GuardBundle = 5,
}

impl OfflineCashHalo2CircuitRoleV1 {
    /// Exact canonically ordered circuit inventory.
    pub(crate) const ALL: [Self; 5] = [
        Self::State,
        Self::GuardUse,
        Self::PlatformBind,
        Self::AndroidKeyCert,
        Self::GuardBundle,
    ];

    const fn relation_contract(self) -> &'static [u8] {
        match self {
            Self::State => STATE_TOPOLOGY,
            Self::GuardUse => b"exact-next-counter+operation+state+lineage+policy/v1",
            Self::PlatformBind => b"platform-key+policy+wallet+release-binding/v1",
            Self::AndroidKeyCert => b"android-hardware-key-single-use-certificate/v1",
            Self::GuardBundle => b"guard-use+platform-bind+optional-android-key-cert/v1",
        }
    }
}

/// Non-authorizing failure from the recursive-proof shape activation gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashRecursionActivationPreflightErrorV1 {
    /// The configured IPA round count differs from the governed k=16 profile.
    InvalidRoundCount {
        /// Governed IPA round count.
        expected: u32,
        /// Configured IPA round count.
        actual: u32,
    },
    /// The shape was not accounted under the reviewed direct-instance policy.
    InvalidInstanceQuery {
        /// Instance-query policy used for the shape report.
        actual: PastaIpaInstanceQueryV1,
    },
    /// Recursive compilation requires exactly one public-instance column.
    InvalidInstanceColumnCount {
        /// Configured number of public-instance columns.
        actual: usize,
    },
    /// The configured proof cap cannot be represented by the shape report.
    InvalidProofCap,
    /// The exact configured transcript exceeds the governed parity cap.
    ProofSizeExceeded {
        /// Circuit parity whose shape was measured.
        parity: OfflineCashHalo2ParityV1,
        /// Circuit role whose shape was measured.
        circuit_role: OfflineCashHalo2CircuitRoleV1,
        /// Exact configured augmented-proof bytes.
        actual: u32,
        /// Governed maximum parity-proof bytes.
        maximum: u32,
    },
}

impl fmt::Display for OfflineCashRecursionActivationPreflightErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoundCount { expected, actual } => write!(
                formatter,
                "offline-cash recursive proof uses k={actual}, expected k={expected}"
            ),
            Self::InvalidInstanceQuery { actual } => write!(
                formatter,
                "offline-cash recursive proof uses unsupported instance-query policy {actual:?}"
            ),
            Self::InvalidInstanceColumnCount { actual } => write!(
                formatter,
                "offline-cash recursive proof uses {actual} instance columns, expected one"
            ),
            Self::InvalidProofCap => {
                formatter.write_str("offline-cash parity proof cap does not fit u32")
            }
            Self::ProofSizeExceeded {
                parity,
                circuit_role,
                actual,
                maximum,
            } => write!(
                formatter,
                "offline-cash {parity:?} {circuit_role:?} configured proof size {actual} exceeds the {maximum}-byte parity cap"
            ),
        }
    }
}

impl std::error::Error for OfflineCashRecursionActivationPreflightErrorV1 {}

/// Apply the necessary fixed-profile shape checks for recursive activation.
///
/// Passing this gate is deliberately not proof authority: it only establishes
/// k=16, the reviewed direct-instance opening policy, one instance column, and
/// the governed byte cap. The current STATE and helper binding circuits fail
/// this gate, and the production backend remains disconnected.
///
/// # Errors
///
/// Returns an error when any fixed shape property differs or when the exact
/// augmented transcript exceeds the parity-proof cap.
pub(super) fn preflight_offline_cash_recursion_activation_v1(
    parity: OfflineCashHalo2ParityV1,
    circuit_role: OfflineCashHalo2CircuitRoleV1,
    shape: &PastaIpaProofShapeV1,
) -> Result<(), OfflineCashRecursionActivationPreflightErrorV1> {
    if shape.k() != OFFLINE_CASH_HALO2_K_V1 {
        return Err(
            OfflineCashRecursionActivationPreflightErrorV1::InvalidRoundCount {
                expected: OFFLINE_CASH_HALO2_K_V1,
                actual: shape.k(),
            },
        );
    }
    if shape.instance_query() != PastaIpaInstanceQueryV1::Direct {
        return Err(
            OfflineCashRecursionActivationPreflightErrorV1::InvalidInstanceQuery {
                actual: shape.instance_query(),
            },
        );
    }
    if shape.instance_columns() != 1 {
        return Err(
            OfflineCashRecursionActivationPreflightErrorV1::InvalidInstanceColumnCount {
                actual: shape.instance_columns(),
            },
        );
    }
    let maximum = u32::try_from(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1)
        .map_err(|_| OfflineCashRecursionActivationPreflightErrorV1::InvalidProofCap)?;
    if shape.augmented_proof_bytes() > maximum {
        return Err(
            OfflineCashRecursionActivationPreflightErrorV1::ProofSizeExceeded {
                parity,
                circuit_role,
                actual: shape.augmented_proof_bytes(),
                maximum,
            },
        );
    }
    Ok(())
}

/// Immutable identity of one parity/role protocol contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OfflineCashHalo2ProtocolIdentityV1 {
    parity: OfflineCashHalo2ParityV1,
    circuit_role: OfflineCashHalo2CircuitRoleV1,
    digest: [u8; 32],
}

impl OfflineCashHalo2ProtocolIdentityV1 {
    /// Selected Pasta parity.
    pub(crate) const fn parity(self) -> OfflineCashHalo2ParityV1 {
        self.parity
    }

    /// Selected finite circuit role.
    pub(crate) const fn circuit_role(self) -> OfflineCashHalo2CircuitRoleV1 {
        self.circuit_role
    }

    /// SHA-256 of the complete framed protocol contract.
    pub(crate) const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

struct FramedSha256(Sha256);

impl FramedSha256 {
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

/// Return the exact protocol identity for one finite role and Pasta parity.
#[must_use]
pub(crate) fn offline_cash_halo2_protocol_identity_v1(
    parity: OfflineCashHalo2ParityV1,
    circuit_role: OfflineCashHalo2CircuitRoleV1,
) -> OfflineCashHalo2ProtocolIdentityV1 {
    let version = OFFLINE_CASH_WIRE_VERSION_V1.to_le_bytes();
    let k = OFFLINE_CASH_HALO2_K_V1.to_le_bytes();
    let domain_size = (1_u64 << OFFLINE_CASH_HALO2_K_V1).to_le_bytes();
    let parity_tag = [parity as u8];
    let role_tag = [circuit_role as u8];
    let parity_proof_cap = u64::try_from(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1)
        .expect("offline-cash proof cap fits u64")
        .to_le_bytes();
    let history_bytes = u64::try_from(OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1)
        .expect("offline-cash history size fits u64")
        .to_le_bytes();
    let state_abi_words = OFFLINE_CASH_STATE_ABI_WORDS_V1.to_le_bytes();
    let state_words_per_instance = OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1.to_le_bytes();
    let state_instance_columns = OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1.to_le_bytes();
    let state_instance_cells = OFFLINE_CASH_STATE_INSTANCE_CELLS_V1.to_le_bytes();
    let state_instance_cells_max = OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1.to_le_bytes();
    let state_sha_lanes = OFFLINE_CASH_STATE_SHA_LANES_V1.to_le_bytes();
    let state_sha_jobs = OFFLINE_CASH_STATE_SHA_JOBS_V1.to_le_bytes();
    let state_sha_total_blocks = OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1.to_le_bytes();
    let helper_abi_words = OFFLINE_CASH_HELPER_ABI_WORDS_V1.to_le_bytes();
    let helper_words_per_instance = OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1.to_le_bytes();
    let helper_instance_columns = OFFLINE_CASH_HELPER_INSTANCE_COLUMNS_V1.to_le_bytes();
    let helper_instance_cells = OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1.to_le_bytes();
    let helper_instance_cells_max = OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1.to_le_bytes();
    let mut digest = FramedSha256::new(PROTOCOL_DOMAIN);
    for field in [
        version.as_slice(),
        k.as_slice(),
        domain_size.as_slice(),
        parity_tag.as_slice(),
        role_tag.as_slice(),
        parity.curve_contract(),
        circuit_role.relation_contract(),
        HALO2_BACKEND_REVISION,
        SNARK_VERIFIER_REVISION,
        PCS_REVISION,
        TRANSCRIPT_REVISION,
        KEY_FORMAT_REVISION,
        HISTORY_REVISION,
        AUGMENTED_PROOF_REVISION,
        parity_proof_cap.as_slice(),
        history_bytes.as_slice(),
    ] {
        digest.field(field);
    }
    if circuit_role == OfflineCashHalo2CircuitRoleV1::State {
        for field in [
            STATE_PUBLIC_INSTANCE_ABI_REVISION_V1,
            STATE_PUBLIC_BINDING_CIRCUIT_REVISION_V1,
            state_abi_words.as_slice(),
            state_words_per_instance.as_slice(),
            state_instance_columns.as_slice(),
            state_instance_cells.as_slice(),
            state_instance_cells_max.as_slice(),
            state_sha_lanes.as_slice(),
            state_sha_jobs.as_slice(),
        ] {
            digest.field(field);
        }
        for blocks in OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1 {
            digest.field(&blocks.to_le_bytes());
        }
        digest.field(&state_sha_total_blocks);
    } else {
        for field in [
            HELPER_PUBLIC_INSTANCE_ABI_REVISION_V1,
            HELPER_PUBLIC_BINDING_CIRCUIT_REVISION_V1,
            helper_abi_words.as_slice(),
            helper_words_per_instance.as_slice(),
            helper_instance_columns.as_slice(),
            helper_instance_cells.as_slice(),
            helper_instance_cells_max.as_slice(),
        ] {
            digest.field(field);
        }
    }
    OfflineCashHalo2ProtocolIdentityV1 {
        parity,
        circuit_role,
        digest: digest.finish(),
    }
}

/// Return the inclusive byte-length bounds for one governed artifact role.
#[must_use]
pub(super) const fn offline_cash_artifact_length_bounds_v1(
    role: OfflineCashArtifactRoleV1,
) -> (u64, u64) {
    match role {
        OfflineCashArtifactRoleV1::ParamsEq | OfflineCashArtifactRoleV1::ParamsEp => {
            (OFFLINE_CASH_PARAMS_BYTES_V1, OFFLINE_CASH_PARAMS_BYTES_V1)
        }
        OfflineCashArtifactRoleV1::StatePkEq | OfflineCashArtifactRoleV1::StatePkEp => {
            (1, OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1)
        }
        OfflineCashArtifactRoleV1::GuardUsePkEq
        | OfflineCashArtifactRoleV1::GuardUsePkEp
        | OfflineCashArtifactRoleV1::PlatformBindPkEq
        | OfflineCashArtifactRoleV1::PlatformBindPkEp
        | OfflineCashArtifactRoleV1::AndroidKeyCertPkEq
        | OfflineCashArtifactRoleV1::AndroidKeyCertPkEp
        | OfflineCashArtifactRoleV1::GuardBundlePkEq
        | OfflineCashArtifactRoleV1::GuardBundlePkEp => {
            (1, OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1)
        }
        OfflineCashArtifactRoleV1::StateVkEq
        | OfflineCashArtifactRoleV1::StateVkEp
        | OfflineCashArtifactRoleV1::GuardUseVkEq
        | OfflineCashArtifactRoleV1::GuardUseVkEp
        | OfflineCashArtifactRoleV1::PlatformBindVkEq
        | OfflineCashArtifactRoleV1::PlatformBindVkEp
        | OfflineCashArtifactRoleV1::AndroidKeyCertVkEq
        | OfflineCashArtifactRoleV1::AndroidKeyCertVkEp
        | OfflineCashArtifactRoleV1::GuardBundleVkEq
        | OfflineCashArtifactRoleV1::GuardBundleVkEp => {
            (1, OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1)
        }
    }
}

/// Resolve a key artifact to its one exact role/parity protocol identity.
#[must_use]
pub(super) const fn offline_cash_artifact_protocol_v1(
    role: OfflineCashArtifactRoleV1,
) -> Option<(OfflineCashHalo2ParityV1, OfflineCashHalo2CircuitRoleV1)> {
    use OfflineCashArtifactRoleV1 as Artifact;
    use OfflineCashHalo2CircuitRoleV1 as Circuit;
    use OfflineCashHalo2ParityV1 as Parity;
    match role {
        Artifact::ParamsEq | Artifact::ParamsEp => None,
        Artifact::StatePkEq | Artifact::StateVkEq => Some((Parity::Eq, Circuit::State)),
        Artifact::StatePkEp | Artifact::StateVkEp => Some((Parity::Ep, Circuit::State)),
        Artifact::GuardUsePkEq | Artifact::GuardUseVkEq => Some((Parity::Eq, Circuit::GuardUse)),
        Artifact::GuardUsePkEp | Artifact::GuardUseVkEp => Some((Parity::Ep, Circuit::GuardUse)),
        Artifact::PlatformBindPkEq | Artifact::PlatformBindVkEq => {
            Some((Parity::Eq, Circuit::PlatformBind))
        }
        Artifact::PlatformBindPkEp | Artifact::PlatformBindVkEp => {
            Some((Parity::Ep, Circuit::PlatformBind))
        }
        Artifact::AndroidKeyCertPkEq | Artifact::AndroidKeyCertVkEq => {
            Some((Parity::Eq, Circuit::AndroidKeyCert))
        }
        Artifact::AndroidKeyCertPkEp | Artifact::AndroidKeyCertVkEp => {
            Some((Parity::Ep, Circuit::AndroidKeyCert))
        }
        Artifact::GuardBundlePkEq | Artifact::GuardBundleVkEq => {
            Some((Parity::Eq, Circuit::GuardBundle))
        }
        Artifact::GuardBundlePkEp | Artifact::GuardBundleVkEp => {
            Some((Parity::Ep, Circuit::GuardBundle))
        }
    }
}

/// Return the exact digest of the complete Offline Cash V1 Halo2 profile.
#[must_use]
pub(crate) fn offline_cash_halo2_profile_digest_v1() -> [u8; 32] {
    let version = OFFLINE_CASH_WIRE_VERSION_V1.to_le_bytes();
    let k = OFFLINE_CASH_HALO2_K_V1.to_le_bytes();
    let scalar_caps = [
        u64::try_from(OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1).expect("request cap fits u64"),
        u64::try_from(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1).expect("payment cap fits u64"),
        u64::try_from(OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)
            .expect("acknowledgement cap fits u64"),
        u64::try_from(OFFLINE_CASH_SESSION_MAX_BYTES_V1).expect("session cap fits u64"),
        u64::try_from(OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1).expect("text-session cap fits u64"),
        u64::try_from(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1).expect("parity-proof cap fits u64"),
        u64::try_from(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1).expect("paired-proof cap fits u64"),
        u64::try_from(OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1).expect("history size fits u64"),
        u64::try_from(OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1)
            .expect("encrypted-credit cap fits u64"),
        OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1,
        OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1,
    ];
    let mut digest = FramedSha256::new(PROFILE_DOMAIN);
    digest.field(&version);
    digest.field(&k);
    digest.field(HALO2_BACKEND_REVISION);
    digest.field(SNARK_VERIFIER_REVISION);
    digest.field(PCS_REVISION);
    digest.field(TRANSCRIPT_REVISION);
    digest.field(KEY_FORMAT_REVISION);
    digest.field(HISTORY_REVISION);
    digest.field(AUGMENTED_PROOF_REVISION);
    digest.field(STATE_TOPOLOGY);
    digest.field(STATE_PUBLIC_INSTANCE_ABI_REVISION_V1);
    digest.field(STATE_PUBLIC_BINDING_CIRCUIT_REVISION_V1);
    digest.field(HELPER_PUBLIC_INSTANCE_ABI_REVISION_V1);
    digest.field(HELPER_PUBLIC_BINDING_CIRCUIT_REVISION_V1);
    for dimension in [
        OFFLINE_CASH_STATE_ABI_WORDS_V1,
        OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1,
        OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1,
        OFFLINE_CASH_STATE_INSTANCE_CELLS_V1,
        OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1,
        OFFLINE_CASH_STATE_SHA_LANES_V1,
        OFFLINE_CASH_STATE_SHA_JOBS_V1,
    ] {
        digest.field(&dimension.to_le_bytes());
    }
    for blocks in OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1 {
        digest.field(&blocks.to_le_bytes());
    }
    digest.field(&OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1.to_le_bytes());
    for dimension in [
        OFFLINE_CASH_HELPER_ABI_WORDS_V1,
        OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1,
        OFFLINE_CASH_HELPER_INSTANCE_COLUMNS_V1,
        OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1,
        OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1,
    ] {
        digest.field(&dimension.to_le_bytes());
    }
    for cap in scalar_caps {
        digest.field(&cap.to_le_bytes());
    }
    for role in OfflineCashArtifactRoleV1::ALL {
        let role_tag = [role as u8];
        let (minimum, maximum) = offline_cash_artifact_length_bounds_v1(role);
        digest.field(&role_tag);
        digest.field(&minimum.to_le_bytes());
        digest.field(&maximum.to_le_bytes());
        if let Some((parity, circuit_role)) = offline_cash_artifact_protocol_v1(role) {
            digest.field(&offline_cash_halo2_protocol_identity_v1(parity, circuit_role).digest());
        } else {
            digest.field(&[0; 32]);
        }
    }
    digest.finish()
}
