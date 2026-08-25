//! Exact compiled contract for the Offline Cash V1 Halo2/IPA profile.
//!
//! These identities describe the only circuit roles and resource bounds that
//! an Offline Cash V1 release may select. The first-party backend parses
//! authenticated artifacts and verifies canonical ordinary Poseidon proofs.
//! Recursive wrappers use a fixed, reciprocal Pasta audit binding; there is no
//! public augmented-proof or delayed-history format in this clean V1 profile.
//! Production receipt authority remains fail-closed until the governed release
//! artifacts, qualification evidence, and secure-device activation gates pass.

use core::fmt;

use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
    KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4, KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
    OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1,
    OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1, OFFLINE_CASH_GUARD_BUNDLE_PAIR_BINDING_BYTES_V1,
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1,
    OFFLINE_CASH_IPA_LINEAGE_CRYPTO_BYTES_V1, OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1,
    OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1, OFFLINE_CASH_P256_V3_HALO2_K_V1,
    OFFLINE_CASH_P256_V3_PROVING_KEY_MAX_BYTES_V1, OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1,
    OFFLINE_CASH_PARAMS_BYTES_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
    OFFLINE_CASH_PAYMENT_MAX_BYTES_V1, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
    OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1, OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1,
    OFFLINE_CASH_RECURSIVE_PAIR_BINDING_PUBLIC_BYTES_V1,
    OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1 as DATA_MODEL_RECURSIVE_PAIR_BINDING_WORDS_V1,
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
const TRANSCRIPT_REVISION: &[u8] =
    b"snark-verifier/PoseidonTranscript(width3,rate2,full8,partial57,secure-mds0)/v1";
const KEY_FORMAT_REVISION: &[u8] = b"halo2-axiom/SerdeFormat::Processed";
const RECURSIVE_PAIR_BINDING_REVISION: &[u8] =
    b"offline-cash-recursive-pair-binding/state-compact100-wire+guard-bundle-canonical68-domain-sha256-join+136-u32-public/two-audit-digests/reciprocal-serial-base-point-equations+parity-lineage/native-outer+carried-terminal-decisions/v3";
const ORDINARY_PROOF_REVISION: &[u8] =
    b"poseidon-direct-instance/ordinary-proof/exact-reader/no-appended-accumulator/v1";
const PUBLIC_CONTRACT_REVISION: &[u8] =
    b"native-bridge-abi22+kagemusha-data-v4+artifact-manifest-v4";
const CREDIT_CIPHER_REVISION_V1: &[u8] =
    b"offline-cash-credit-envelope/KCE1-u16le-v1+strict-canonical-x25519+sha256-framed-domain-kdf+xchacha20poly1305/fixed116/max384/plaintext-KCO1-u16le-v1+opening32/aad-release+request+transition+credit+recipient-key-reference+recipient-x25519+ephemeral-x25519/request-signature-p256-distinct-key/v1";
const STATE_TOPOLOGY: &[u8] =
    b"final-state-wrapper/state-leaf+completed-guard-bundle/expose-state-abi229/v1";
const STATE_LEAF_TOPOLOGY: &[u8] =
    b"fixed-two-parent/send-split+receive-fold/semantic-prefix93-only/no-pair-binding-fixed-point/no-hop-cap/v1";
const FINAL_STATE_WRAPPER_CIRCUIT_REVISION_V1: &[u8] =
    b"packed-current-row-v1/k16-degree10/virtual-base(advice12,lookup-advice4,fixed1,instance2)/physical(advice8-queries8,equality8-permutation-chunks1,fixed3-queries3,instances2-direct-queries2,lookups2,selectors0,point-sets3)/commitments59+evaluations37/ordinary-poseidon3072/source-authoritative-typed-nibble-sha256/base-graph-same-parity(state-leaf+guard-bundle)/authenticated-protocol-vk-identities/exact229-primary+36-lineage/fold(state-leaf-outer,guard-bundle-outer,guard-bundle-carried)/guard-bundle-pair-full20+canonical68-domain-sha256-child-join/reciprocal-serial-base-point-equations/native-outer+carried-terminal-decisions/v4";
const GUARD_BUNDLE_WRAPPER_CIRCUIT_REVISION_V1: &[u8] =
    b"packed-current-row-v1/k16-degree10/virtual-base(advice96,lookup-advice12,fixed1,instance3)/physical(advice8-queries8,equality9-permutation-chunks2,fixed3-queries3,instances3(two-direct-queries2+one-exact36-equality),lookups2,selectors0,point-sets4)/commitments60+evaluations42/ordinary-poseidon3264/usable-rows65527/source-authoritative-typed-nibble-sha256/same-parity-fixed-children(guard-use,platform-bind,android-key-cert,guard-bundle-leaf,platform-p256,android-p256)/authenticated-role+parity+protocol+vk-identities/exact184-common-word-equality/platform+android-aux97-to-p256-statement/android-absence-duplicates-platform-p256/exact27-common+20-pair-audit+36-lineage/reciprocal-serial-base-point-equations/native-outer+carried-terminal-decisions/v4";
const P256_V3_PUBLIC_INSTANCE_ABI_REVISION_V1: &[u8] =
    b"p256-v3/one-column/direct/161-caller-bytes(sec1-uncompressed65+sha256-prehash32+p1363-low-s64)+235-verifier-derived-constant-scalars/396-cells";
const P256_V3_CIRCUIT_REVISION_V1: &[u8] =
    b"packed-affine-v3/k16/degree10/two-isolated-logical-lanes/advice16-queries16/equality16-two-permutation-chunks/fixed4-queries4/lookups4/pair-only-identical-opcode+range-tag/bind-lane0-only/table65365+usable65527+headroom162/semantic-pad64886+reserved479/points74-scalars68/ordinary-poseidon4544/private-recursive-child-not-final-wire/no-appended-generator/source-sha256-9c54b4b7a6decdd707af47d371d9b786352fb4b35c9d16662d5f5496fe1f02cd";
pub(super) const STATE_PUBLIC_INSTANCE_ABI_REVISION_V1: &[u8] =
    b"u32le-v1/header(abi,wire,k,parity,operation,parent-count,digest-words,recursive-pair-words)/digests(release,protocol,semantic,context,request,parent0,parent1,result,link,transition)/amount4/scale1/recursive-pair-binding136/primary-column33-pack7x32le-last-zero2+parity-lineage-column36/op1-send(sender-before,receiver-before,sender-after,credit)/op2-receive(balance,credit,next,send-transition)";
pub(super) const STATE_LEAF_PUBLIC_INSTANCE_ABI_REVISION_V1: &[u8] =
    b"u32le-v1/state-semantic-prefix93/header+digests+amount4+scale1/no-recursive-pair-tail/pack7x32le/one-column/last-zero5/final-state-protocol-digest-in-words16..23/state-leaf-vk-authenticated-out-of-band";
const STATE_PUBLIC_BINDING_CIRCUIT_REVISION_V1: &[u8] =
    b"state-leaf-relation/axiom-floorplanner-v1-two-pass+witnessless-measurement/semantic-prefix93-only/bit32+pack224/op-in-{1,2}/u128le-4x32-carry-zero-ends/exact-next-u64-no-overflow/nonzero-9x32byte-private-bindings+send-seed-op1+8x32limb-running-sum9rows-maxrot1/sha256-raw-byte-order/frame-u64le-lengths+u16le-head-version/balance335(sequence+lineage)+credit278+lineage361+send-seed365+send-branches101+103+receive-opening273+receive-transition341+receive-semantic430+send-context257+canonical-norito-transition441+canonical-norito-semantic421/jobs-ordered(6,6,5,6,6,2,2,5,6,7,5,8,7)-blocks/five-lanes-shared-table/send-seed+branch-openings+context+transition+semantic-op1-gated/receive-opening+transition+semantic-op2-gated/network72+asset72-exact-canonical-frame-private-witness+fixed-payload-offset-bindings/final-recursion-owned-by-state-wrapper/v11";
pub(super) const HELPER_PUBLIC_INSTANCE_ABI_REVISION_V1: &[u8] =
    b"u32le-v1/header(abi,wire,k,parity,role,operation,android-present,digest-words,from-lo,from-hi,to-lo,to-hi,digest-count,words-per-cell,cells,reserved-zero)/digests(protocol,release,context,current-head,current-lineage,transition,wallet,policy,device,current-guard,next-guard,platform-key,platform-message,guard-use-claim,platform-bind-claim,android-cert,android-tbs,android-issuer-key,android-attestation,android-claim,bundle)/pack7x32le/one-column/last-zero5/common-statement-all-helper-roles";
const HELPER_PUBLIC_BINDING_CIRCUIT_REVISION_V1: &[u8] =
    b"helper-leaf-binding/floorplanner-v1/bit32+public-byte8-range+pack224/role-specific-fixed-protocol/op-in-{1,2}/android-flag-boolean+fixed-present-or-canonical-absent-android/exact-next-u64-no-overflow/required-16-digests-nonzero/optional-five-android-digests-all-zero-or-all-nonzero/current-guard-ne-next-guard/current-head-ne-transition/sum-of-eight-u32-squared-differences-below-2^67/sha256-raw-byte-order+u64le-field-lengths/role-jobs(guard-use:0,1,2,4;platform-bind:3,5;android-key-cert:6,7;guard-bundle-leaf:8)/role-lanes(2,1,1,1)/private-platform+android-issuer-sec1-fixed65-range-checked/platform+android-intermediate-public-aux97(sec1-65+digest32)-byte-exact/p256-v3-one-shot-zeroizing-canonical-low-s-equivalence/governed-finalized-policy+release-receipt+eligibility-credential+bounded-cbor-der-keymint-x509-root-to-fixed-source/poseidon-direct-instance-ordinary-proof/private-recursive-child/v4";
pub(super) const OFFLINE_CASH_STATE_ABI_WORDS_V1: u32 = 229;
/// Non-circular public prefix exposed by `StateLeaf`. The final `State`
/// wrapper copies these words and owns the separate 136-word pair binding.
pub(super) const OFFLINE_CASH_STATE_SEMANTIC_ABI_WORDS_V1: u32 = 93;
pub(super) const OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1: u32 = 7;
pub(super) const OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1: u32 = 2;
pub(super) const OFFLINE_CASH_STATE_LEAF_INSTANCE_COLUMNS_V1: u32 = 1;
pub(super) const OFFLINE_CASH_STATE_INSTANCE_CELLS_V1: u32 = 33;
pub(super) const OFFLINE_CASH_STATE_LEAF_INSTANCE_CELLS_V1: u32 = 14;
pub(super) const OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1: u32 = 50;
pub(super) const OFFLINE_CASH_STATE_SHA_LANES_V1: u32 = 5;
pub(super) const OFFLINE_CASH_STATE_SHA_JOBS_V1: u32 = 13;
pub(super) const OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1: [u32; 13] =
    [6, 6, 5, 6, 6, 2, 2, 5, 6, 7, 5, 8, 7];
pub(super) const OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1: u32 = 71;
pub(super) const OFFLINE_CASH_HELPER_ABI_WORDS_V1: u32 = 184;
pub(super) const OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1: u32 = 7;
pub(super) const OFFLINE_CASH_HELPER_INSTANCE_COLUMNS_V1: u32 = 1;
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_COLUMNS_V1: u32 = 3;
pub(super) const OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1: u32 = 27;
pub(super) const OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1: u32 = 32;
/// Intermediate public SEC1-plus-digest column used only inside recursive
/// composition for `PlatformBind` and `AndroidKeyCert`; it is not wire data.
pub(super) const OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1: u32 = 65 + 32;
/// Canonical field-neutral reciprocal-audit binding carried only between
/// recursive circuits. It reuses the 136-word state tail and packs into twenty
/// seven-word cells when used as GuardBundle's second instance column.
pub(super) const OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1: u32 = 136;
pub(super) const OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1: u32 =
    OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1
        .div_ceil(OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1);
/// Dedicated public cells carrying one parity-local folded IPA lineage.
pub(super) const OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1: u32 = 36;
/// Private recursive-witness proof slot for the STATE relation leaf.
pub(super) const OFFLINE_CASH_STATE_LEAF_PROOF_MAX_BYTES_V1: u32 = 64 * 1024;
/// Private recursive-witness proof slot for the four-job GuardUse leaf.
pub(super) const OFFLINE_CASH_GUARD_USE_PROOF_MAX_BYTES_V1: u32 = 12 * 1024;
/// Private recursive-witness proof slot for PlatformBind.
pub(super) const OFFLINE_CASH_PLATFORM_BIND_PROOF_MAX_BYTES_V1: u32 = 8 * 1024;
/// Private recursive-witness proof slot for fixed-geometry AndroidKeyCert.
pub(super) const OFFLINE_CASH_ANDROID_KEY_CERT_PROOF_MAX_BYTES_V1: u32 = 8 * 1024;
/// Private recursive-witness proof slot for the GuardBundle SHA leaf.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_LEAF_PROOF_MAX_BYTES_V1: u32 = 8 * 1024;
/// Private recursive-witness proof slot for the recursive GuardBundle child.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_PROOF_MAX_BYTES_V1: u32 = 64 * 1024;
/// Private recursive-witness proof slot for packed-affine P-256 V3.
pub(super) const OFFLINE_CASH_P256_V3_PROOF_MAX_BYTES_V1: u32 = 4_544;

/// Return the governed proof slot for a private recursive child. Final State
/// proofs are the only role governed directly by the 3,200-byte wire cap.
pub(super) const fn offline_cash_internal_child_proof_max_bytes_v1(
    role: OfflineCashHalo2CircuitRoleV1,
) -> Option<u32> {
    match role {
        OfflineCashHalo2CircuitRoleV1::State => None,
        OfflineCashHalo2CircuitRoleV1::StateLeaf => {
            Some(OFFLINE_CASH_STATE_LEAF_PROOF_MAX_BYTES_V1)
        }
        OfflineCashHalo2CircuitRoleV1::GuardUse => Some(OFFLINE_CASH_GUARD_USE_PROOF_MAX_BYTES_V1),
        OfflineCashHalo2CircuitRoleV1::PlatformBind => {
            Some(OFFLINE_CASH_PLATFORM_BIND_PROOF_MAX_BYTES_V1)
        }
        OfflineCashHalo2CircuitRoleV1::AndroidKeyCert => {
            Some(OFFLINE_CASH_ANDROID_KEY_CERT_PROOF_MAX_BYTES_V1)
        }
        OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf => {
            Some(OFFLINE_CASH_GUARD_BUNDLE_LEAF_PROOF_MAX_BYTES_V1)
        }
        OfflineCashHalo2CircuitRoleV1::GuardBundle => {
            Some(OFFLINE_CASH_GUARD_BUNDLE_PROOF_MAX_BYTES_V1)
        }
        OfflineCashHalo2CircuitRoleV1::P256V3 => Some(OFFLINE_CASH_P256_V3_PROOF_MAX_BYTES_V1),
    }
}

const _: () = assert!(
    OFFLINE_CASH_STATE_INSTANCE_CELLS_V1
        == OFFLINE_CASH_STATE_ABI_WORDS_V1.div_ceil(OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1)
);
const _: () = assert!(
    OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1 as usize
        == DATA_MODEL_RECURSIVE_PAIR_BINDING_WORDS_V1
);
const _: () = assert!(
    OFFLINE_CASH_RECURSIVE_PAIR_BINDING_PUBLIC_BYTES_V1
        == DATA_MODEL_RECURSIVE_PAIR_BINDING_WORDS_V1 * 4
);
const _: () = assert!(
    OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize
        == OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1
);
const _: () = assert!(
    OFFLINE_CASH_STATE_LEAF_INSTANCE_CELLS_V1
        == OFFLINE_CASH_STATE_SEMANTIC_ABI_WORDS_V1
            .div_ceil(OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1)
);
const _: () = assert!(
    OFFLINE_CASH_STATE_ABI_WORDS_V1 - OFFLINE_CASH_STATE_SEMANTIC_ABI_WORDS_V1
        == OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1
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
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[10]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[11]
        + OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[12]
        == OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1
);
const _: () = assert!(OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1 == 71);
const _: () = assert!(
    OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1
        == OFFLINE_CASH_HELPER_ABI_WORDS_V1.div_ceil(OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1)
);
const _: () =
    assert!(OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1 <= OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1);
const _: () = assert!(KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4 == 22);
const _: () = assert!(KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4 == 4);
const _: () = assert!(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4 == 4);

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
    /// Canonical low-S P-256 V3 statement child relation.
    P256V3 = 6,
    /// Private balance/credit state relation verified by final `State`.
    StateLeaf = 7,
    /// SHA-256 bundle relation leaf verified by recursive `GuardBundle`.
    GuardBundleLeaf = 8,
}

impl OfflineCashHalo2CircuitRoleV1 {
    /// Exact canonically ordered circuit inventory.
    pub(crate) const ALL: [Self; 8] = [
        Self::State,
        Self::GuardUse,
        Self::PlatformBind,
        Self::AndroidKeyCert,
        Self::GuardBundle,
        Self::P256V3,
        Self::StateLeaf,
        Self::GuardBundleLeaf,
    ];

    const fn relation_contract(self) -> &'static [u8] {
        match self {
            Self::State => STATE_TOPOLOGY,
            Self::GuardUse => b"exact-next-counter+operation+state+lineage+policy/v1",
            Self::PlatformBind => b"platform-key+policy+wallet+release-binding/v1",
            Self::AndroidKeyCert => b"android-hardware-key-single-use-certificate/v1",
            Self::GuardBundle => b"guard-use+platform-bind+optional-android-key-cert/v1",
            Self::P256V3 => b"canonical-low-s-p256-prehashed-statement/v3",
            Self::StateLeaf => STATE_LEAF_TOPOLOGY,
            Self::GuardBundleLeaf => b"guard-bundle-sha-relation/v1",
        }
    }
}

/// Exact number of public-instance columns compiled for one role. Recursive
/// wrapper auxiliary columns are authenticated intermediate proof inputs; the
/// typed lineages are carried alongside, rather than inside, the proof bytes.
pub(super) const fn offline_cash_instance_columns_v1(role: OfflineCashHalo2CircuitRoleV1) -> u32 {
    match role {
        OfflineCashHalo2CircuitRoleV1::State => OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1,
        OfflineCashHalo2CircuitRoleV1::PlatformBind
        | OfflineCashHalo2CircuitRoleV1::AndroidKeyCert => 2,
        OfflineCashHalo2CircuitRoleV1::GuardBundle => OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_COLUMNS_V1,
        OfflineCashHalo2CircuitRoleV1::StateLeaf
        | OfflineCashHalo2CircuitRoleV1::GuardUse
        | OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf
        | OfflineCashHalo2CircuitRoleV1::P256V3 => 1,
    }
}

/// Exact length of the primary direct-instance column for one role.
///
/// `StateLeaf` deliberately exposes only the 93-word semantic prefix. Keeping
/// this distinct from final `State` prevents the recursive-pair audit from
/// becoming part of the child proof transcript that defines that same audit.
pub(super) const fn offline_cash_primary_instance_cells_v1(
    role: OfflineCashHalo2CircuitRoleV1,
) -> u32 {
    match role {
        OfflineCashHalo2CircuitRoleV1::State => OFFLINE_CASH_STATE_INSTANCE_CELLS_V1,
        OfflineCashHalo2CircuitRoleV1::StateLeaf => OFFLINE_CASH_STATE_LEAF_INSTANCE_CELLS_V1,
        OfflineCashHalo2CircuitRoleV1::GuardUse
        | OfflineCashHalo2CircuitRoleV1::PlatformBind
        | OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
        | OfflineCashHalo2CircuitRoleV1::GuardBundle
        | OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf => OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1,
        // P-256 V3 exposes 161 byte cells followed by 235 verifier-derived
        // fixed scalar cells in one direct-instance column.
        OfflineCashHalo2CircuitRoleV1::P256V3 => 396,
    }
}

/// Exact second-column length, or zero for roles without one.
pub(super) const fn offline_cash_helper_aux_instance_cells_v1(
    role: OfflineCashHalo2CircuitRoleV1,
) -> u32 {
    match role {
        OfflineCashHalo2CircuitRoleV1::State => OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1,
        OfflineCashHalo2CircuitRoleV1::PlatformBind
        | OfflineCashHalo2CircuitRoleV1::AndroidKeyCert => {
            OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1
        }
        OfflineCashHalo2CircuitRoleV1::GuardBundle => {
            OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1
        }
        _ => 0,
    }
}

/// Exact third-column length. Only recursive `GuardBundle` exposes its folded
/// parity-local lineage after the common and compact pair-binding columns.
pub(super) const fn offline_cash_third_instance_cells_v1(
    role: OfflineCashHalo2CircuitRoleV1,
) -> u32 {
    match role {
        OfflineCashHalo2CircuitRoleV1::GuardBundle => {
            OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1
        }
        _ => 0,
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
    /// Recursive compilation requires the role-specific public-column count.
    InvalidInstanceColumnCount {
        /// Governed number of public-instance columns for this role.
        expected: usize,
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
        /// Exact configured ordinary Poseidon-proof bytes.
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
            Self::InvalidInstanceColumnCount { expected, actual } => write!(
                formatter,
                "offline-cash recursive proof uses {actual} instance columns, expected {expected}"
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

/// Apply fixed-profile shape checks, including the public wire cap only for
/// the carried final `State` role.
///
/// Passing this gate is deliberately not proof authority: it only establishes
/// the role-specific degree, the reviewed direct-instance opening policy, and
/// the exact role-specific instance-column count. Internal child proofs are private recursive witnesses
/// with separate role-specific slots. Final `State` is compared with the
/// 3,200-byte parity wire cap; every private child is compared with its exact
/// governed recursive-witness slot.
///
/// # Errors
///
/// Returns an error when any fixed shape property differs or when the exact
/// ordinary Poseidon transcript exceeds the role-specific proof cap.
pub(super) fn preflight_offline_cash_recursion_activation_v1(
    parity: OfflineCashHalo2ParityV1,
    circuit_role: OfflineCashHalo2CircuitRoleV1,
    shape: &PastaIpaProofShapeV1,
) -> Result<(), OfflineCashRecursionActivationPreflightErrorV1> {
    let expected_k = if circuit_role == OfflineCashHalo2CircuitRoleV1::P256V3 {
        OFFLINE_CASH_P256_V3_HALO2_K_V1
    } else {
        OFFLINE_CASH_HALO2_K_V1
    };
    if shape.k() != expected_k {
        return Err(
            OfflineCashRecursionActivationPreflightErrorV1::InvalidRoundCount {
                expected: expected_k,
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
    let expected_instance_columns = usize::try_from(offline_cash_instance_columns_v1(circuit_role))
        .expect("fixed Offline Cash instance-column count fits usize");
    if shape.instance_columns() != expected_instance_columns {
        return Err(
            OfflineCashRecursionActivationPreflightErrorV1::InvalidInstanceColumnCount {
                expected: expected_instance_columns,
                actual: shape.instance_columns(),
            },
        );
    }
    let maximum = if circuit_role == OfflineCashHalo2CircuitRoleV1::State {
        u32::try_from(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1)
            .map_err(|_| OfflineCashRecursionActivationPreflightErrorV1::InvalidProofCap)?
    } else {
        offline_cash_internal_child_proof_max_bytes_v1(circuit_role)
            .ok_or(OfflineCashRecursionActivationPreflightErrorV1::InvalidProofCap)?
    };
    if shape.ordinary_proof_bytes() > maximum {
        return Err(
            OfflineCashRecursionActivationPreflightErrorV1::ProofSizeExceeded {
                parity,
                circuit_role,
                actual: shape.ordinary_proof_bytes(),
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
    let circuit_k = if circuit_role == OfflineCashHalo2CircuitRoleV1::P256V3 {
        OFFLINE_CASH_P256_V3_HALO2_K_V1
    } else {
        OFFLINE_CASH_HALO2_K_V1
    };
    let k = circuit_k.to_le_bytes();
    let domain_size = (1_u64 << circuit_k).to_le_bytes();
    let parity_tag = [parity as u8];
    let role_tag = [circuit_role as u8];
    let proof_cap = if circuit_role == OfflineCashHalo2CircuitRoleV1::State {
        u64::try_from(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1)
            .expect("offline-cash wire proof cap fits u64")
    } else {
        u64::from(
            offline_cash_internal_child_proof_max_bytes_v1(circuit_role)
                .expect("non-final roles have a private recursive proof slot"),
        )
    }
    .to_le_bytes();
    let state_abi_words = OFFLINE_CASH_STATE_ABI_WORDS_V1.to_le_bytes();
    let state_semantic_abi_words = OFFLINE_CASH_STATE_SEMANTIC_ABI_WORDS_V1.to_le_bytes();
    let state_words_per_instance = OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1.to_le_bytes();
    let role_instance_columns = offline_cash_instance_columns_v1(circuit_role).to_le_bytes();
    let state_instance_cells = OFFLINE_CASH_STATE_INSTANCE_CELLS_V1.to_le_bytes();
    let state_leaf_instance_cells = OFFLINE_CASH_STATE_LEAF_INSTANCE_CELLS_V1.to_le_bytes();
    let state_instance_cells_max = OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1.to_le_bytes();
    let state_sha_lanes = OFFLINE_CASH_STATE_SHA_LANES_V1.to_le_bytes();
    let state_sha_jobs = OFFLINE_CASH_STATE_SHA_JOBS_V1.to_le_bytes();
    let state_sha_total_blocks = OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1.to_le_bytes();
    let helper_abi_words = OFFLINE_CASH_HELPER_ABI_WORDS_V1.to_le_bytes();
    let helper_words_per_instance = OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1.to_le_bytes();
    let helper_instance_cells = OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1.to_le_bytes();
    let helper_instance_cells_max = OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1.to_le_bytes();
    let second_instance_cells =
        offline_cash_helper_aux_instance_cells_v1(circuit_role).to_le_bytes();
    let third_instance_cells = offline_cash_third_instance_cells_v1(circuit_role).to_le_bytes();
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
        RECURSIVE_PAIR_BINDING_REVISION,
        ORDINARY_PROOF_REVISION,
        proof_cap.as_slice(),
    ] {
        digest.field(field);
    }
    if matches!(
        circuit_role,
        OfflineCashHalo2CircuitRoleV1::State | OfflineCashHalo2CircuitRoleV1::StateLeaf
    ) {
        for field in [
            if circuit_role == OfflineCashHalo2CircuitRoleV1::State {
                STATE_PUBLIC_INSTANCE_ABI_REVISION_V1
            } else {
                STATE_LEAF_PUBLIC_INSTANCE_ABI_REVISION_V1
            },
            if circuit_role == OfflineCashHalo2CircuitRoleV1::State {
                FINAL_STATE_WRAPPER_CIRCUIT_REVISION_V1
            } else {
                STATE_PUBLIC_BINDING_CIRCUIT_REVISION_V1
            },
            if circuit_role == OfflineCashHalo2CircuitRoleV1::State {
                state_abi_words.as_slice()
            } else {
                state_semantic_abi_words.as_slice()
            },
            state_words_per_instance.as_slice(),
            role_instance_columns.as_slice(),
            if circuit_role == OfflineCashHalo2CircuitRoleV1::State {
                state_instance_cells.as_slice()
            } else {
                state_leaf_instance_cells.as_slice()
            },
            state_instance_cells_max.as_slice(),
            state_sha_lanes.as_slice(),
            state_sha_jobs.as_slice(),
            second_instance_cells.as_slice(),
            third_instance_cells.as_slice(),
        ] {
            digest.field(field);
        }
        for blocks in OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1 {
            digest.field(&blocks.to_le_bytes());
        }
        digest.field(&state_sha_total_blocks);
    } else if circuit_role == OfflineCashHalo2CircuitRoleV1::P256V3 {
        for field in [
            P256_V3_PUBLIC_INSTANCE_ABI_REVISION_V1,
            P256_V3_CIRCUIT_REVISION_V1,
        ] {
            digest.field(field);
        }
    } else if circuit_role == OfflineCashHalo2CircuitRoleV1::GuardBundle {
        for field in [
            HELPER_PUBLIC_INSTANCE_ABI_REVISION_V1,
            GUARD_BUNDLE_WRAPPER_CIRCUIT_REVISION_V1,
            helper_abi_words.as_slice(),
            helper_words_per_instance.as_slice(),
            role_instance_columns.as_slice(),
            helper_instance_cells.as_slice(),
            helper_instance_cells_max.as_slice(),
            second_instance_cells.as_slice(),
            third_instance_cells.as_slice(),
        ] {
            digest.field(field);
        }
    } else {
        for field in [
            HELPER_PUBLIC_INSTANCE_ABI_REVISION_V1,
            HELPER_PUBLIC_BINDING_CIRCUIT_REVISION_V1,
            helper_abi_words.as_slice(),
            helper_words_per_instance.as_slice(),
            role_instance_columns.as_slice(),
            helper_instance_cells.as_slice(),
            helper_instance_cells_max.as_slice(),
            second_instance_cells.as_slice(),
            third_instance_cells.as_slice(),
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
        OfflineCashArtifactRoleV1::StatePkEq
        | OfflineCashArtifactRoleV1::StatePkEp
        | OfflineCashArtifactRoleV1::StateLeafPkEq
        | OfflineCashArtifactRoleV1::StateLeafPkEp => {
            (1, OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1)
        }
        OfflineCashArtifactRoleV1::GuardUsePkEq
        | OfflineCashArtifactRoleV1::GuardUsePkEp
        | OfflineCashArtifactRoleV1::PlatformBindPkEq
        | OfflineCashArtifactRoleV1::PlatformBindPkEp
        | OfflineCashArtifactRoleV1::AndroidKeyCertPkEq
        | OfflineCashArtifactRoleV1::AndroidKeyCertPkEp
        | OfflineCashArtifactRoleV1::GuardBundlePkEq
        | OfflineCashArtifactRoleV1::GuardBundlePkEp
        | OfflineCashArtifactRoleV1::GuardBundleLeafPkEq
        | OfflineCashArtifactRoleV1::GuardBundleLeafPkEp => {
            (1, OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1)
        }
        OfflineCashArtifactRoleV1::P256V3PkEq | OfflineCashArtifactRoleV1::P256V3PkEp => {
            (1, OFFLINE_CASH_P256_V3_PROVING_KEY_MAX_BYTES_V1)
        }
        OfflineCashArtifactRoleV1::StateVkEq
        | OfflineCashArtifactRoleV1::StateVkEp
        | OfflineCashArtifactRoleV1::StateLeafVkEq
        | OfflineCashArtifactRoleV1::StateLeafVkEp
        | OfflineCashArtifactRoleV1::GuardUseVkEq
        | OfflineCashArtifactRoleV1::GuardUseVkEp
        | OfflineCashArtifactRoleV1::PlatformBindVkEq
        | OfflineCashArtifactRoleV1::PlatformBindVkEp
        | OfflineCashArtifactRoleV1::AndroidKeyCertVkEq
        | OfflineCashArtifactRoleV1::AndroidKeyCertVkEp
        | OfflineCashArtifactRoleV1::GuardBundleVkEq
        | OfflineCashArtifactRoleV1::GuardBundleVkEp
        | OfflineCashArtifactRoleV1::GuardBundleLeafVkEq
        | OfflineCashArtifactRoleV1::GuardBundleLeafVkEp => {
            (1, OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1)
        }
        OfflineCashArtifactRoleV1::P256V3VkEq | OfflineCashArtifactRoleV1::P256V3VkEp => {
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
        Artifact::P256V3PkEq | Artifact::P256V3VkEq => Some((Parity::Eq, Circuit::P256V3)),
        Artifact::P256V3PkEp | Artifact::P256V3VkEp => Some((Parity::Ep, Circuit::P256V3)),
        Artifact::StateLeafPkEq | Artifact::StateLeafVkEq => Some((Parity::Eq, Circuit::StateLeaf)),
        Artifact::StateLeafPkEp | Artifact::StateLeafVkEp => Some((Parity::Ep, Circuit::StateLeaf)),
        Artifact::GuardBundleLeafPkEq | Artifact::GuardBundleLeafVkEq => {
            Some((Parity::Eq, Circuit::GuardBundleLeaf))
        }
        Artifact::GuardBundleLeafPkEp | Artifact::GuardBundleLeafVkEp => {
            Some((Parity::Ep, Circuit::GuardBundleLeaf))
        }
    }
}

/// Return the exact digest of the complete Offline Cash V1 Halo2 profile.
#[must_use]
pub(crate) fn offline_cash_halo2_profile_digest_v1() -> [u8; 32] {
    offline_cash_halo2_profile_digest_for_public_contract_v1(
        KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
    )
}

fn offline_cash_halo2_profile_digest_for_public_contract_v1(
    bridge_abi_version: u32,
    data_wire_version: u16,
    artifact_manifest_schema: &str,
    artifact_manifest_version: u16,
) -> [u8; 32] {
    let version = OFFLINE_CASH_WIRE_VERSION_V1.to_le_bytes();
    let k = OFFLINE_CASH_HALO2_K_V1.to_le_bytes();
    let p256_v3_k = OFFLINE_CASH_P256_V3_HALO2_K_V1.to_le_bytes();
    let bridge_abi_version = bridge_abi_version.to_le_bytes();
    let data_wire_version = data_wire_version.to_le_bytes();
    let artifact_manifest_version = artifact_manifest_version.to_le_bytes();
    let scalar_caps = [
        u64::try_from(OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1).expect("request cap fits u64"),
        u64::try_from(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1).expect("payment cap fits u64"),
        u64::try_from(OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)
            .expect("acknowledgement cap fits u64"),
        u64::try_from(OFFLINE_CASH_SESSION_MAX_BYTES_V1).expect("session cap fits u64"),
        u64::try_from(OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1).expect("text-session cap fits u64"),
        u64::try_from(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1).expect("parity-proof cap fits u64"),
        u64::try_from(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1).expect("paired-proof cap fits u64"),
        u64::try_from(OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1)
            .expect("compact recursive-pair binding size fits u64"),
        u64::try_from(OFFLINE_CASH_GUARD_BUNDLE_PAIR_BINDING_BYTES_V1)
            .expect("GuardBundle canonical pair projection size fits u64"),
        u64::try_from(OFFLINE_CASH_RECURSIVE_PAIR_BINDING_PUBLIC_BYTES_V1)
            .expect("expanded recursive-pair binding size fits u64"),
        u64::try_from(OFFLINE_CASH_IPA_LINEAGE_CRYPTO_BYTES_V1)
            .expect("lineage crypto size fits u64"),
        u64::try_from(OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1)
            .expect("lineage encoded size fits u64"),
        u64::try_from(OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1)
            .expect("encrypted-credit cap fits u64"),
        OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1,
        OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1,
        u64::from(OFFLINE_CASH_STATE_LEAF_PROOF_MAX_BYTES_V1),
        u64::from(OFFLINE_CASH_GUARD_USE_PROOF_MAX_BYTES_V1),
        u64::from(OFFLINE_CASH_PLATFORM_BIND_PROOF_MAX_BYTES_V1),
        u64::from(OFFLINE_CASH_ANDROID_KEY_CERT_PROOF_MAX_BYTES_V1),
        u64::from(OFFLINE_CASH_GUARD_BUNDLE_LEAF_PROOF_MAX_BYTES_V1),
        u64::from(OFFLINE_CASH_GUARD_BUNDLE_PROOF_MAX_BYTES_V1),
        u64::from(OFFLINE_CASH_P256_V3_PROOF_MAX_BYTES_V1),
    ];
    let mut digest = FramedSha256::new(PROFILE_DOMAIN);
    digest.field(&version);
    digest.field(&k);
    digest.field(&p256_v3_k);
    digest.field(HALO2_BACKEND_REVISION);
    digest.field(SNARK_VERIFIER_REVISION);
    digest.field(PCS_REVISION);
    digest.field(TRANSCRIPT_REVISION);
    digest.field(KEY_FORMAT_REVISION);
    digest.field(RECURSIVE_PAIR_BINDING_REVISION);
    digest.field(ORDINARY_PROOF_REVISION);
    digest.field(PUBLIC_CONTRACT_REVISION);
    digest.field(CREDIT_CIPHER_REVISION_V1);
    digest.field(&bridge_abi_version);
    digest.field(&data_wire_version);
    digest.field(artifact_manifest_schema.as_bytes());
    digest.field(&artifact_manifest_version);
    digest.field(STATE_TOPOLOGY);
    digest.field(STATE_LEAF_TOPOLOGY);
    digest.field(STATE_PUBLIC_INSTANCE_ABI_REVISION_V1);
    digest.field(STATE_LEAF_PUBLIC_INSTANCE_ABI_REVISION_V1);
    digest.field(STATE_PUBLIC_BINDING_CIRCUIT_REVISION_V1);
    digest.field(FINAL_STATE_WRAPPER_CIRCUIT_REVISION_V1);
    digest.field(HELPER_PUBLIC_INSTANCE_ABI_REVISION_V1);
    digest.field(HELPER_PUBLIC_BINDING_CIRCUIT_REVISION_V1);
    digest.field(GUARD_BUNDLE_WRAPPER_CIRCUIT_REVISION_V1);
    digest.field(P256_V3_PUBLIC_INSTANCE_ABI_REVISION_V1);
    digest.field(P256_V3_CIRCUIT_REVISION_V1);
    for dimension in [
        OFFLINE_CASH_STATE_ABI_WORDS_V1,
        OFFLINE_CASH_STATE_SEMANTIC_ABI_WORDS_V1,
        OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1,
        OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1,
        OFFLINE_CASH_STATE_LEAF_INSTANCE_COLUMNS_V1,
        OFFLINE_CASH_STATE_INSTANCE_CELLS_V1,
        OFFLINE_CASH_STATE_LEAF_INSTANCE_CELLS_V1,
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
        OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_COLUMNS_V1,
        OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1,
        OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1,
        OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1,
        OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1,
        OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1,
        OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1,
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

#[cfg(test)]
pub(super) fn offline_cash_halo2_profile_digest_for_public_contract_test_v1(
    bridge_abi_version: u32,
    data_wire_version: u16,
    artifact_manifest_schema: &str,
    artifact_manifest_version: u16,
) -> [u8; 32] {
    offline_cash_halo2_profile_digest_for_public_contract_v1(
        bridge_abi_version,
        data_wire_version,
        artifact_manifest_schema,
        artifact_manifest_version,
    )
}
