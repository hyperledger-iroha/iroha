//! Privately declared, fail-closed native attestation-registration candidate for Offline Cash V2.
//!
//! This private child does not alter the V1
//! helper, wire, state, artifact, or verifier contracts.  It separates two authorities:
//!
//! - a host registration authority validates private certificate/KeyMint evidence and may mint
//!   one move-only registry leaf; and
//! - an offline inclusion authority persists that leaf under a governed root and may issue a
//!   public receipt/checkpoint pair.
//!
//! Both production authority types are uninhabited.  The existing Android KeyMint semantic
//! validator and policy/root provider are private to the smart-contract implementation, so this
//! module cannot manufacture either authority without the adapter delta named below.  Parsing,
//! hashing, chain validation, registry framing, and test-only one-shot authority exercises do not
//! turn that missing authority into a production success path.

use std::collections::BTreeSet;

use iroha_data_model::offline::OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1;
use p256::PublicKey as P256PublicKey;
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use x509_parser::{
    extensions::ParsedExtension,
    prelude::{FromDer as _, X509Certificate},
    time::ASN1Time,
};
use zeroize::Zeroize;

pub(super) const NATIVE_REGISTRATION_HELPER_WORDS_V2: usize = 184;
pub(super) const NATIVE_REGISTRATION_RAW_TBS_MAX_BYTES_V2: usize = 3_767;
pub(super) const NATIVE_REGISTRATION_MIN_CHAIN_DEPTH_V2: usize = 2;
pub(super) const NATIVE_REGISTRATION_MAX_CHAIN_DEPTH_V2: usize = 4;
pub(super) const NATIVE_REGISTRATION_MAX_CERTIFICATE_BYTES_V2: usize = 16 * 1024;
pub(super) const NATIVE_REGISTRATION_MAX_ATTESTATION_EXTENSION_BYTES_V2: usize = 16 * 1024;
pub(super) const NATIVE_REGISTRATION_MAX_TRUSTED_ROOTS_V2: usize = 8;
pub(super) const NATIVE_REGISTRATION_MAX_REVOKED_CERTIFICATES_V2: usize =
    OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1;
pub(super) const NATIVE_REGISTRATION_MAX_GOVERNED_ISSUERS_V2: usize = 16;
pub(super) const NATIVE_REGISTRY_DEPTH_V2: usize = 64;
pub(super) const NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2: usize =
    2 + 8 + NATIVE_REGISTRY_DEPTH_V2 * 32 + 32;
pub(super) const NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2: usize = 595;
pub(super) const NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2: usize = 138;
pub(super) const NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2: usize =
    NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2
        + NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2
        + NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2;
pub(super) const NATIVE_REGISTRATION_OFFLINE_ENVELOPE_MAX_BYTES_V2: usize = 3_264;

pub(super) const NATIVE_REGISTRATION_SOURCE_IMPLEMENTED_V2: bool = true;
pub(super) const NATIVE_REGISTRATION_DECLARED_V2: bool = true;
pub(super) const NATIVE_REGISTRATION_KEYMINT_ADAPTER_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_ROOT_PROVIDER_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_PERSISTENCE_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_TERMINAL_ADAPTER_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_FRESHNESS_PROJECTION_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_PROJECTION_IDENTITY_BINDING_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRY_CHECKPOINT_ANTI_ROLLBACK_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_CANONICAL_DECODER_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_REVOCATION_CAP_ADAPTER_EVIDENCE_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_ARTIFACT_EVIDENCE_AVAILABLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_ACTIVATION_READY_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_RELEASE_ELIGIBLE_V2: bool = false;
pub(super) const NATIVE_REGISTRATION_PRODUCTION_AVAILABLE_V2: bool = false;

/// Exact future adapter delta.  Until every item exists, the uninhabited authority types below
/// must remain uninhabited and every production/readiness flag above must remain false.
pub(super) const NATIVE_REGISTRATION_MINIMUM_ADAPTER_DELTA_V2: &[u8] = b"make the existing bounded parse_single_der_value_v1 strict-DER primitive available in ordinary core builds without enabling release-evidence tooling; add an immutable monotonic u64 policy revision epoch beside the existing canonical OfflineDeviceAttestationPolicy hash (the current stored policy has version/hash but no revision epoch); add one pub(crate) smartcontracts/isi/offline projection wrapper that, in the same state transaction, first calls the complete validate_offline_device_attestation_registration path and then returns the exact validated registration hash and freshness identity (challenge, recent block height/hash, admission time, expiry, account, platform, device), exact leaf DER, ordered chain DER, exact Android KeyDescription extension DER, assertion SEC1 key, lowercase-hex-decoded SHA256 key id, policy revision epoch/hash, direct issuer key id projected from that governed-root-validated chain, active trusted roots, and complete revocation set; bind the registration hash plus one canonical domain-separated digest of the freshness/projection identity into the public leaf/receipt contract without disclosing its raw private fields, and re-freeze the wire/cap ledger; add a consensus-governed one-shot host-authority constructor whose consumed token is cryptographically bound to that exact projection, policy epoch/hash, registry intent, and device key id and cannot authorize a substituted or stale input; qualify the complete governed 256-revocation projection cap; add an authenticated registry-root checkpoint provider with a monotonic checkpoint sequence and atomic no-replacement persistence that rejects duplicate key ids and derived-index collisions; persist and enforce a terminal checkpoint sequence/epoch anti-rollback floor; add exact-length canonical receipt/proof/checkpoint decoders and a terminal adapter; preserve V1 unchanged; only then replace both empty authority enums and independently qualify activation/readiness/release";

/// Activation-only gaps that remain even though tests exercise the source-local DER, chain, and
/// SHA relation.  A receipt/checkpoint consistency check is not a freshness or rollback proof,
/// and the source-side 256-entry cap is not compatibility evidence for the private policy
/// projection until the full-cap adapter path is qualified.
pub(super) const NATIVE_REGISTRATION_ACTIVATION_BLOCKERS_V2: &[u8] = b"ordinary-build strict DER unavailable; full existing KeyMint validator projection unavailable; validated registration hash and private-preserving freshness/projection digest absent from leaf/receipt and wire cap not re-frozen; projection-bound governed host capability unavailable; immutable monotonic policy revision epoch unavailable; authenticated registry root provider and persistence unavailable; checkpoint sequence and terminal anti-rollback floor absent; canonical receipt/proof/checkpoint decoders absent; full governed 256-revocation projection-cap compatibility unqualified; terminal adapter/artifacts/activation/readiness/release unavailable/v2";

/// Fixed host-evidence interpretation.  The attestation digest is SHA-256 of the exact DER value
/// carried as the Android Key Attestation extension's `extnValue` contents, not of a re-encoding,
/// JSON report, CBOR envelope, or certificate suffix.
pub(super) const NATIVE_REGISTRATION_EVIDENCE_CONTRACT_V2: &[u8] = b"android-keymint/exact-complete-leaf-certificate-der/ordered-leaf-to-root-chain/exact-tbs-as-encoded/exact-1.3.6.1.4.1.11129.2.1.17-extnValue-der/sha256-no-reencoding/no-trailing-der/v2";

/// Exact public receipt schema.  No certificate, TBS, attestation extension, signature, or chain
/// bytes are present in a leaf, receipt, checkpoint, or inclusion proof.
pub(super) const NATIVE_REGISTRATION_RECEIPT_CONTRACT_V2: &[u8] = b"version-u16le/policy-epoch-u64le/policy-digest/registry-intent/device-key-id/device-sec1/device-descriptor-digest/certificate-digest/tbs-digest/issuer-key-id/issuer-key-digest/attestation-digest/chain-digest/root-certificate-digest/fixed-nine-digest/guard-bundle-digest/leaf-commitment/index-u64le=first8le(framed-SHA256-index-domain(device-key-id))/registry-root/receipt-commitment/fixed-depth64/siblings-leaf-to-root/v2";
pub(super) const NATIVE_REGISTRATION_HASH_FRAME_CONTRACT_V2: &[u8] = b"u64le-domain-byte-length || domain-bytes || repeated(u64le-field-byte-length || exact-field-bytes); SHA-256 once over complete frame; no implicit concatenation, terminator, text encoding, field sorting, or re-encoding/v2";
pub(super) const NATIVE_REGISTRATION_RAW_DIGEST_CONTRACT_V2: &[u8] = b"device-key-id=SHA256(exact-65-byte-device-sec1); certificate-digest=SHA256(exact-complete-leaf-der); tbs-digest=SHA256(exact-tbs-tlv-slice-from-leaf-der); issuer-key-digest=SHA256(exact-65-byte-direct-issuer-sec1); attestation-digest=SHA256(exact-android-key-description-extnValue-der); chain-digest=framed-SHA256(epoch,depth,leaf-through-root-exact-der)/v2";
pub(super) const NATIVE_REGISTRY_ALLOCATION_CONTRACT_V2: &[u8] = b"index=first8-little-endian(framed-SHA256(iroha:offline-cash:v2:registry:index,device-key-id)); fixed sparse depth64; no probing, remapping, replacement, or duplicate key-id leaf; occupied-index collision and replay both fail closed; siblings ordered leaf-level0 through root-level63/v2";

const HELPER_ABI_VERSION: u32 = 1;
const HELPER_WIRE_VERSION: u32 = 1;
const HELPER_K: u32 = 16;
const HELPER_DIGEST_WORDS: u32 = 8;
const HELPER_DIGEST_FIELDS: u32 = 21;
const HELPER_WORDS_PER_CELL: u32 = 7;
const HELPER_INSTANCE_CELLS: u32 = 27;

const OPERATION_WORD: usize = 5;
const ANDROID_PRESENT_WORD: usize = 6;
const FROM_LOW_WORD: usize = 8;
const FROM_HIGH_WORD: usize = 9;
const TO_LOW_WORD: usize = 10;
const TO_HIGH_WORD: usize = 11;

const PROTOCOL_WORD_START: usize = 16;
const RELEASE_WORD_START: usize = 24;
const CONTEXT_WORD_START: usize = 32;
const CURRENT_HEAD_WORD_START: usize = 40;
const CURRENT_LINEAGE_WORD_START: usize = 48;
const TRANSITION_WORD_START: usize = 56;
const WALLET_WORD_START: usize = 64;
const POLICY_WORD_START: usize = 72;
const DEVICE_WORD_START: usize = 80;
const CURRENT_GUARD_WORD_START: usize = 88;
const NEXT_GUARD_WORD_START: usize = 96;
const PLATFORM_KEY_WORD_START: usize = 104;
const PLATFORM_MESSAGE_WORD_START: usize = 112;
const GUARD_USE_CLAIM_WORD_START: usize = 120;
const PLATFORM_BIND_CLAIM_WORD_START: usize = 128;
const CERTIFICATE_WORD_START: usize = 136;
const TBS_WORD_START: usize = 144;
const ISSUER_KEY_WORD_START: usize = 152;
const ATTESTATION_WORD_START: usize = 160;
const ANDROID_CLAIM_WORD_START: usize = 168;
const BUNDLE_WORD_START: usize = 176;

pub(super) const NATIVE_REGISTRATION_FIXED_MESSAGE_BYTES_V2: [usize; 9] =
    [355, 432, 494, 65, 533, 376, 65, 480, 619];
pub(super) const NATIVE_REGISTRATION_FIXED_JOB_DIGEST_OFFSETS_V2: [usize; 9] = [
    CURRENT_GUARD_WORD_START,
    NEXT_GUARD_WORD_START,
    PLATFORM_MESSAGE_WORD_START,
    PLATFORM_KEY_WORD_START,
    GUARD_USE_CLAIM_WORD_START,
    PLATFORM_BIND_CLAIM_WORD_START,
    ISSUER_KEY_WORD_START,
    ANDROID_CLAIM_WORD_START,
    BUNDLE_WORD_START,
];

const CURRENT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:current-guard";
const NEXT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:next-guard";
const PLATFORM_MESSAGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-message";
const GUARD_USE_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-use-claim";
const PLATFORM_BIND_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-bind-claim";
const ANDROID_KEY_CERT_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:android-key-cert-claim";
const GUARD_BUNDLE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-bundle";
const P256_ALGORITHM: &[u8] = b"ecdsa-p256-sha256";
const ANDROID_KEY_ORIGIN: &[u8] = b"generated-in-keymint-hardware";
const ANDROID_KEY_PURPOSE: &[u8] = b"sign";
const ANDROID_DIGEST_MODE: &[u8] = b"sha-256";
const ANDROID_USAGE_LIMIT_ONE: [u8; 4] = 1_u32.to_le_bytes();

const ANDROID_KEY_ATTESTATION_EXTENSION_OID: &str = "1.3.6.1.4.1.11129.2.1.17";
const ECDSA_WITH_SHA256_OID: &str = "1.2.840.10045.4.3.2";
const EC_PUBLIC_KEY_OID: &str = "1.2.840.10045.2.1";
const PRIME256V1_OID: &str = "1.2.840.10045.3.1.7";

const KEY_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registration:key-id";
const CHAIN_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registration:chain";
const FIXED_NINE_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registration:fixed-nine";
const LEAF_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registration:leaf";
const AUTHORIZATION_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registration:authorization";
const REGISTRY_LEAF_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registry:leaf";
const REGISTRY_NODE_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registry:node";
const REGISTRY_INDEX_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registry:index";
const RECEIPT_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registration:receipt";
const CHECKPOINT_DOMAIN: &[u8] = b"iroha:offline-cash:v2:registry:checkpoint";
const SCHEMA_VERSION: [u8; 2] = 2_u16.to_le_bytes();

const ALL_DIGEST_OFFSETS: [usize; 21] = [
    PROTOCOL_WORD_START,
    RELEASE_WORD_START,
    CONTEXT_WORD_START,
    CURRENT_HEAD_WORD_START,
    CURRENT_LINEAGE_WORD_START,
    TRANSITION_WORD_START,
    WALLET_WORD_START,
    POLICY_WORD_START,
    DEVICE_WORD_START,
    CURRENT_GUARD_WORD_START,
    NEXT_GUARD_WORD_START,
    PLATFORM_KEY_WORD_START,
    PLATFORM_MESSAGE_WORD_START,
    GUARD_USE_CLAIM_WORD_START,
    PLATFORM_BIND_CLAIM_WORD_START,
    CERTIFICATE_WORD_START,
    TBS_WORD_START,
    ISSUER_KEY_WORD_START,
    ATTESTATION_WORD_START,
    ANDROID_CLAIM_WORD_START,
    BUNDLE_WORD_START,
];

const fn protocol_hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("invalid embedded V1 helper protocol digest"),
    }
}

const fn protocol_digest_from_hex(hex: &[u8; 64]) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    let mut index = 0;
    while index < digest.len() {
        digest[index] =
            (protocol_hex_nibble(hex[2 * index]) << 4) | protocol_hex_nibble(hex[2 * index + 1]);
        index += 1;
    }
    digest
}

/// Exact V1 helper protocol identity duplicated at this private boundary because V1 exposes
/// its identity function only to its own test surface.  Candidate tests compare every entry with
/// the live V1 function, so a future V1 contract change fails the declaration gate closed.
pub(super) const fn native_registration_source_helper_protocol_digest_v1(
    parity: u32,
    role: u32,
) -> Option<[u8; 32]> {
    let digest = match (parity, role) {
        (1, 2) => protocol_digest_from_hex(
            b"a0ac04d998f0e0f258d49c246c91e0642716219f3194cdb02bcaf39a7e58056f",
        ),
        (1, 3) => protocol_digest_from_hex(
            b"bea057eb94775db5cccfa58a422cd837d823aa98756956136eafecc6a92b9b67",
        ),
        (1, 4) => protocol_digest_from_hex(
            b"dc2aae8a1bac738c7c165c6610799015c2198394bbe1d83f7eb0424ecea1aba8",
        ),
        (1, 5) => protocol_digest_from_hex(
            b"919ecb15bb10adb53676c222bf95b099c6d16a236557a7987e3708625042936f",
        ),
        (2, 2) => protocol_digest_from_hex(
            b"e07865b759cf9b3dc5b367144a73ec7e64fd404db8885d5f6a8f1cea2f163bf9",
        ),
        (2, 3) => protocol_digest_from_hex(
            b"956d9c0a4284dcf2ea263342b4f0199802a1e7de0fa257c22eda0abdff720799",
        ),
        (2, 4) => protocol_digest_from_hex(
            b"f6f0cd2e1bc1d7b493a0b2568220d1536448977dad24ea834dc6b8b232aef667",
        ),
        (2, 5) => protocol_digest_from_hex(
            b"993041665fa8e420ca8509b70cd8e460f65018fda09cd348bd7ad6ef756c6f7a",
        ),
        _ => return None,
    };
    Some(digest)
}

const _: () = assert!(NATIVE_REGISTRATION_DECLARED_V2);
const _: () = assert!(!NATIVE_REGISTRATION_KEYMINT_ADAPTER_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_ROOT_PROVIDER_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_PERSISTENCE_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_TERMINAL_ADAPTER_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_FRESHNESS_PROJECTION_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_PROJECTION_IDENTITY_BINDING_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRY_CHECKPOINT_ANTI_ROLLBACK_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_CANONICAL_DECODER_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_REVOCATION_CAP_ADAPTER_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_ARTIFACT_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_ACTIVATION_READY_V2);
const _: () = assert!(!NATIVE_REGISTRATION_RELEASE_ELIGIBLE_V2);
const _: () = assert!(!NATIVE_REGISTRATION_PRODUCTION_AVAILABLE_V2);
const _: () = assert!(NATIVE_REGISTRATION_MAX_REVOKED_CERTIFICATES_V2 == 256);
const _: () = assert!(NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2 == 2_090);
const _: () = assert!(NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2 == 595);
const _: () = assert!(NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2 == 138);
const _: () = assert!(NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2 == 2_823);
const _: () = assert!(
    NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2
        <= NATIVE_REGISTRATION_OFFLINE_ENVELOPE_MAX_BYTES_V2
);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(super) enum NativeRegistrationErrorV2 {
    #[error("native registration helper header is invalid")]
    InvalidHelperHeader,
    #[error("native registration helper protocol digest does not match its V1 parity/role")]
    InvalidHelperProtocolIdentity,
    #[error("native registration helper violates a mandatory V1 digest inequality")]
    InvalidHelperDigestEquality,
    #[error("native registration operation is invalid")]
    InvalidOperation,
    #[error("native registration requires Android attestation")]
    AndroidAttestationRequired,
    #[error("native registration sequence is invalid")]
    InvalidSequence,
    #[error("native registration contains a zero digest")]
    ZeroDigest,
    #[error("native registration policy epoch is zero")]
    ZeroPolicyEpoch,
    #[error("native registration policy epoch does not match governance")]
    PolicyEpochMismatch,
    #[error("native registration registry intent does not match governance")]
    RegistryIntentMismatch,
    #[error("native registration policy digest does not match helper ABI")]
    PolicyDigestMismatch,
    #[error(
        "native registration governance collection is empty, oversized, duplicated, or unordered"
    )]
    InvalidGovernanceCollection,
    #[error(
        "native registration certificate chain depth {actual} is outside {minimum}..={maximum}"
    )]
    InvalidChainDepth {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },
    #[error("native registration certificate {index} is empty or exceeds {maximum} bytes")]
    CertificateSize { index: usize, maximum: usize },
    #[error("native registration certificate {index} DER is invalid")]
    InvalidCertificateDer { index: usize },
    #[error("native registration certificate {index} DER has trailing bytes")]
    TrailingCertificateDer { index: usize },
    #[error("native registration certificate {index} is not canonical DER")]
    NonCanonicalCertificateDer { index: usize },
    #[error("strict canonical-DER adapter is unavailable in this build")]
    CanonicalDerAdapterUnavailable,
    #[error("native registration certificate chain does not start with the exact leaf DER")]
    LeafCertificateMismatch,
    #[error("native registration certificate chain contains a duplicate")]
    DuplicateCertificate,
    #[error("native registration certificate is revoked")]
    RevokedCertificate,
    #[error("native registration certificate time is invalid")]
    InvalidCertificateTime,
    #[error("native registration certificate algorithm is outside P-256/SHA-256 profile")]
    InvalidCertificateAlgorithm,
    #[error("native registration certificate public key is invalid")]
    InvalidCertificatePublicKey,
    #[error("native registration certificate has a duplicate or unsupported critical extension")]
    InvalidCertificateExtension,
    #[error("native registration leaf is not a digital-signature end entity")]
    InvalidLeafUsage,
    #[error("native registration issuer/root is not a certificate-signing CA")]
    InvalidCaUsage,
    #[error("native registration issuer/subject order is invalid")]
    InvalidIssuerOrder,
    #[error("native registration certificate signature is invalid")]
    InvalidCertificateSignature,
    #[error("native registration chain root is not an exact governed trust anchor")]
    UntrustedRoot,
    #[error("native registration issuer key is not governed")]
    UngovernedIssuer,
    #[error("native registration device SEC1 key is invalid or differs from leaf SPKI")]
    DeviceKeyMismatch,
    #[error("native registration key id differs from SHA-256(device SEC1 key)")]
    DeviceKeyIdMismatch,
    #[error("native registration Android attestation extension is missing or duplicated")]
    InvalidAttestationExtension,
    #[error("native registration Android attestation extension is empty or oversized")]
    AttestationExtensionSize,
    #[error(
        "native registration Android attestation extension bytes differ from the exact leaf extension"
    )]
    AttestationExtensionMismatch,
    #[error("native registration Android attestation extension is not one canonical DER value")]
    NonCanonicalAttestationExtension,
    #[error("native registration raw TBSCertificate is empty")]
    EmptyRawTbs,
    #[error("native registration raw TBSCertificate has {actual} bytes; maximum is {maximum}")]
    RawTbsTooLarge { actual: usize, maximum: usize },
    #[error("native registration raw digest at helper word {offset} does not match")]
    RawDigestMismatch { offset: usize },
    #[error("native registration fixed SHA job {job} does not match")]
    FixedJobDigestMismatch { job: usize },
    #[error("native registration fixed SHA message geometry is invalid")]
    FixedMessageGeometry,
    #[error("native registration hash framing overflowed")]
    FramingOverflow,
    #[error("native registration one-shot authority is already spent")]
    AuthoritySpent,
    #[error("native registry inclusion proof contains a zero digest")]
    ZeroInclusionDigest,
    #[error("native registry inclusion proof root does not match")]
    InclusionRootMismatch,
    #[error("native registry leaf index is not the canonical key-id-derived index")]
    NonCanonicalLeafIndex,
    #[error("native registration receipt or checkpoint commitment does not match")]
    ReceiptCommitmentMismatch,
    #[error("native registration receipt/checkpoint governance binding does not match")]
    ReceiptGovernanceMismatch,
}

/// Private host evidence.  It intentionally implements neither `Clone` nor `Debug`.
pub(super) struct NativeAttestationRegistrationInputV2 {
    helper_words: [u32; NATIVE_REGISTRATION_HELPER_WORDS_V2],
    device_public_key_sec1: [u8; 65],
    device_key_id: [u8; 32],
    policy_epoch: u64,
    registry_root_intent: [u8; 32],
    certificate_der: Vec<u8>,
    certificate_chain_der: Vec<Vec<u8>>,
    attestation_extension_der: Vec<u8>,
}

impl NativeAttestationRegistrationInputV2 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        helper_words: [u32; NATIVE_REGISTRATION_HELPER_WORDS_V2],
        device_public_key_sec1: [u8; 65],
        device_key_id: [u8; 32],
        policy_epoch: u64,
        registry_root_intent: [u8; 32],
        certificate_der: Vec<u8>,
        certificate_chain_der: Vec<Vec<u8>>,
        attestation_extension_der: Vec<u8>,
    ) -> Result<Self, NativeRegistrationErrorV2> {
        let input = Self {
            helper_words,
            device_public_key_sec1,
            device_key_id,
            policy_epoch,
            registry_root_intent,
            certificate_der,
            certificate_chain_der,
            attestation_extension_der,
        };
        validate_helper_header(&input.helper_words)?;
        if input.policy_epoch == 0 {
            return Err(NativeRegistrationErrorV2::ZeroPolicyEpoch);
        }
        if input.registry_root_intent == [0; 32] {
            return Err(NativeRegistrationErrorV2::RegistryIntentMismatch);
        }
        if input.device_public_key_sec1[0] != 4
            || P256PublicKey::from_sec1_bytes(&input.device_public_key_sec1).is_err()
        {
            return Err(NativeRegistrationErrorV2::DeviceKeyMismatch);
        }
        if input.device_key_id == [0; 32] {
            return Err(NativeRegistrationErrorV2::DeviceKeyIdMismatch);
        }
        if input.certificate_chain_der.len() < NATIVE_REGISTRATION_MIN_CHAIN_DEPTH_V2
            || input.certificate_chain_der.len() > NATIVE_REGISTRATION_MAX_CHAIN_DEPTH_V2
        {
            return Err(NativeRegistrationErrorV2::InvalidChainDepth {
                actual: input.certificate_chain_der.len(),
                minimum: NATIVE_REGISTRATION_MIN_CHAIN_DEPTH_V2,
                maximum: NATIVE_REGISTRATION_MAX_CHAIN_DEPTH_V2,
            });
        }
        if input.attestation_extension_der.is_empty()
            || input.attestation_extension_der.len()
                > NATIVE_REGISTRATION_MAX_ATTESTATION_EXTENSION_BYTES_V2
        {
            return Err(NativeRegistrationErrorV2::AttestationExtensionSize);
        }
        Ok(input)
    }
}

impl Drop for NativeAttestationRegistrationInputV2 {
    fn drop(&mut self) {
        self.helper_words.zeroize();
        self.device_public_key_sec1.zeroize();
        self.device_key_id.zeroize();
        self.policy_epoch.zeroize();
        self.registry_root_intent.zeroize();
        self.certificate_der.zeroize();
        for certificate in &mut self.certificate_chain_der {
            certificate.zeroize();
        }
        self.certificate_chain_der.clear();
        self.attestation_extension_der.zeroize();
    }
}

/// A block-time governance projection.  Production has deliberately no constructor; the future
/// smart-contract adapter must construct it only after the existing KeyMint policy validator has
/// accepted the corresponding registration.
#[derive(Clone)]
pub(super) struct GovernedRegistrationPolicyV2 {
    policy_epoch: u64,
    policy_digest: [u8; 32],
    registry_root_intent: [u8; 32],
    evaluation_unix_seconds: u64,
    trusted_roots_der: Vec<Vec<u8>>,
    revoked_certificate_digests: Vec<[u8; 32]>,
    governed_issuer_key_ids: Vec<[u8; 32]>,
}

/// No value of this type can be created.  Consuming it is the only production leaf-mint API.
pub(super) enum GovernedHostRegistrationAuthorityV2 {}

/// No value of this type can be created.  It is deliberately distinct from host evidence
/// authority: an offline terminal may verify inclusion but may never mint an inclusion receipt.
pub(super) enum GovernedOfflineInclusionAuthorityV2 {}

/// The sole production registration entry point.  It cannot return because its authority is an
/// empty enum.  Test-only code exercises the same internal validation/mint path with an explicit
/// one-shot test authority.
pub(super) fn mint_authenticated_registration_v2(
    _input: NativeAttestationRegistrationInputV2,
    authority: GovernedHostRegistrationAuthorityV2,
) -> Result<AuthenticatedRegistrationLeafV2, NativeRegistrationErrorV2> {
    match authority {}
}

/// The sole production receipt entry point.  Persistence/root authority is independently
/// uninhabited even if a host leaf were somehow available.
pub(super) fn issue_offline_registration_receipt_v2(
    _leaf: AuthenticatedRegistrationLeafV2,
    _proof: RegistryInclusionProofV2,
    authority: GovernedOfflineInclusionAuthorityV2,
) -> Result<OfflineRegistrationReceiptV2, NativeRegistrationErrorV2> {
    match authority {}
}

/// Public-only registry payload.  It is move-only and has no constructor outside the validated
/// host path.
pub(super) struct RegistrationRegistryLeafPayloadV2 {
    policy_epoch: u64,
    policy_digest: [u8; 32],
    registry_root_intent: [u8; 32],
    device_key_id: [u8; 32],
    device_public_key_sec1: [u8; 65],
    device_descriptor_digest: [u8; 32],
    certificate_digest: [u8; 32],
    tbs_digest: [u8; 32],
    issuer_key_id: [u8; 32],
    issuer_key_digest: [u8; 32],
    attestation_digest: [u8; 32],
    chain_digest: [u8; 32],
    root_certificate_digest: [u8; 32],
    fixed_nine_digest: [u8; 32],
    guard_bundle_digest: [u8; 32],
}

/// Move-only host-authenticated leaf.  Authentication is represented by possession of the
/// consumed authority capability plus a deterministic authorization commitment; it is not a
/// substitute for registry inclusion.
pub(super) struct AuthenticatedRegistrationLeafV2 {
    payload: RegistrationRegistryLeafPayloadV2,
    leaf_commitment: [u8; 32],
    authorization_commitment: [u8; 32],
}

impl AuthenticatedRegistrationLeafV2 {
    pub(super) const fn leaf_commitment(&self) -> [u8; 32] {
        self.leaf_commitment
    }

    pub(super) const fn authorization_commitment(&self) -> [u8; 32] {
        self.authorization_commitment
    }
}

/// Fixed-depth sparse-registry inclusion witness.  Siblings are ordered from leaf level zero to
/// root level 63; the corresponding little-endian bit of `leaf_index` selects left/right order.
pub(super) struct RegistryInclusionProofV2 {
    leaf_index: u64,
    siblings: [[u8; 32]; NATIVE_REGISTRY_DEPTH_V2],
    claimed_root: [u8; 32],
}

impl RegistryInclusionProofV2 {
    pub(super) fn new(
        leaf_index: u64,
        siblings: [[u8; 32]; NATIVE_REGISTRY_DEPTH_V2],
        claimed_root: [u8; 32],
    ) -> Result<Self, NativeRegistrationErrorV2> {
        if claimed_root == [0; 32] || siblings.iter().any(|sibling| *sibling == [0; 32]) {
            return Err(NativeRegistrationErrorV2::ZeroInclusionDigest);
        }
        Ok(Self {
            leaf_index,
            siblings,
            claimed_root,
        })
    }

    pub(super) fn canonical_bytes(&self) -> [u8; NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2] {
        let mut output = [0_u8; NATIVE_REGISTRY_INCLUSION_PROOF_RAW_BYTES_V2];
        let mut offset = 0;
        write_canonical_bytes(&mut output, &mut offset, &SCHEMA_VERSION);
        write_canonical_bytes(&mut output, &mut offset, &self.leaf_index.to_le_bytes());
        for sibling in &self.siblings {
            write_canonical_bytes(&mut output, &mut offset, sibling);
        }
        write_canonical_bytes(&mut output, &mut offset, &self.claimed_root);
        debug_assert_eq!(offset, output.len());
        output
    }
}

/// Public governed checkpoint.  Its constructor remains behind the uninhabited inclusion/root
/// authority.  An offline terminal receives this independently of the receipt and treats it as
/// authenticated governance data, never as self-authenticating bytes from the claimant.
pub(super) struct OfflineRegistryCheckpointV2 {
    policy_epoch: u64,
    policy_digest: [u8; 32],
    registry_root_intent: [u8; 32],
    registry_root: [u8; 32],
    checkpoint_commitment: [u8; 32],
}

impl OfflineRegistryCheckpointV2 {
    pub(super) fn canonical_bytes(&self) -> [u8; NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2] {
        let mut output = [0_u8; NATIVE_REGISTRY_CHECKPOINT_RAW_BYTES_V2];
        let mut offset = 0;
        write_canonical_bytes(&mut output, &mut offset, &SCHEMA_VERSION);
        write_canonical_bytes(&mut output, &mut offset, &self.policy_epoch.to_le_bytes());
        write_canonical_bytes(&mut output, &mut offset, &self.policy_digest);
        write_canonical_bytes(&mut output, &mut offset, &self.registry_root_intent);
        write_canonical_bytes(&mut output, &mut offset, &self.registry_root);
        write_canonical_bytes(&mut output, &mut offset, &self.checkpoint_commitment);
        debug_assert_eq!(offset, output.len());
        output
    }
}

/// Public receipt.  No host-private byte string is retained.
pub(super) struct OfflineRegistrationReceiptV2 {
    payload: RegistrationRegistryLeafPayloadV2,
    leaf_commitment: [u8; 32],
    leaf_index: u64,
    registry_root: [u8; 32],
    receipt_commitment: [u8; 32],
}

/// Read-only identity projected from one durable registration receipt.
///
/// This projection deliberately excludes the registration event's `GuardBundle` digest.  A
/// later transaction may bind the registered platform key to its independently authenticated
/// current helper statement, but must not equate that current statement with the historical
/// registration event.  Possession of this value is not proof that a receipt was persisted or
/// authenticated; callers must obtain it from an independently authenticated durable receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DurableRegistrationIdentityProjectionV2 {
    policy_epoch: u64,
    policy_digest: [u8; 32],
    registry_root_intent: [u8; 32],
    device_descriptor_digest: [u8; 32],
    device_key_id: [u8; 32],
    device_public_key_sec1: [u8; 65],
    receipt_commitment: [u8; 32],
}

impl DurableRegistrationIdentityProjectionV2 {
    pub(super) const fn policy_epoch(&self) -> u64 {
        self.policy_epoch
    }

    pub(super) const fn policy_digest(&self) -> &[u8; 32] {
        &self.policy_digest
    }

    pub(super) const fn registry_root_intent(&self) -> &[u8; 32] {
        &self.registry_root_intent
    }

    pub(super) const fn device_descriptor_digest(&self) -> &[u8; 32] {
        &self.device_descriptor_digest
    }

    pub(super) const fn device_key_id(&self) -> &[u8; 32] {
        &self.device_key_id
    }

    pub(super) const fn device_public_key_sec1(&self) -> &[u8; 65] {
        &self.device_public_key_sec1
    }

    pub(super) const fn receipt_commitment(&self) -> &[u8; 32] {
        &self.receipt_commitment
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn from_test_parts(
        policy_epoch: u64,
        policy_digest: [u8; 32],
        registry_root_intent: [u8; 32],
        device_descriptor_digest: [u8; 32],
        device_key_id: [u8; 32],
        device_public_key_sec1: [u8; 65],
        receipt_commitment: [u8; 32],
    ) -> Self {
        Self {
            policy_epoch,
            policy_digest,
            registry_root_intent,
            device_descriptor_digest,
            device_key_id,
            device_public_key_sec1,
            receipt_commitment,
        }
    }
}

impl OfflineRegistrationReceiptV2 {
    /// Project only the durable registration identity needed by later transaction statements.
    pub(super) const fn durable_identity_projection(
        &self,
    ) -> DurableRegistrationIdentityProjectionV2 {
        DurableRegistrationIdentityProjectionV2 {
            policy_epoch: self.payload.policy_epoch,
            policy_digest: self.payload.policy_digest,
            registry_root_intent: self.payload.registry_root_intent,
            device_descriptor_digest: self.payload.device_descriptor_digest,
            device_key_id: self.payload.device_key_id,
            device_public_key_sec1: self.payload.device_public_key_sec1,
            receipt_commitment: self.receipt_commitment,
        }
    }

    pub(super) const fn receipt_commitment(&self) -> [u8; 32] {
        self.receipt_commitment
    }

    pub(super) const fn certificate_digest(&self) -> [u8; 32] {
        self.payload.certificate_digest
    }

    pub(super) const fn tbs_digest(&self) -> [u8; 32] {
        self.payload.tbs_digest
    }

    pub(super) const fn issuer_key_digest(&self) -> [u8; 32] {
        self.payload.issuer_key_digest
    }

    pub(super) const fn attestation_digest(&self) -> [u8; 32] {
        self.payload.attestation_digest
    }

    pub(super) const fn device_key_id(&self) -> [u8; 32] {
        self.payload.device_key_id
    }

    pub(super) const fn device_public_key_sec1(&self) -> &[u8; 65] {
        &self.payload.device_public_key_sec1
    }

    pub(super) fn canonical_bytes(&self) -> [u8; NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2] {
        let mut output = [0_u8; NATIVE_REGISTRATION_RECEIPT_RAW_BYTES_V2];
        let mut offset = 0;
        write_canonical_bytes(&mut output, &mut offset, &SCHEMA_VERSION);
        write_canonical_bytes(
            &mut output,
            &mut offset,
            &self.payload.policy_epoch.to_le_bytes(),
        );
        for field in [
            &self.payload.policy_digest,
            &self.payload.registry_root_intent,
            &self.payload.device_key_id,
        ] {
            write_canonical_bytes(&mut output, &mut offset, field);
        }
        write_canonical_bytes(
            &mut output,
            &mut offset,
            &self.payload.device_public_key_sec1,
        );
        for field in [
            &self.payload.device_descriptor_digest,
            &self.payload.certificate_digest,
            &self.payload.tbs_digest,
            &self.payload.issuer_key_id,
            &self.payload.issuer_key_digest,
            &self.payload.attestation_digest,
            &self.payload.chain_digest,
            &self.payload.root_certificate_digest,
            &self.payload.fixed_nine_digest,
            &self.payload.guard_bundle_digest,
            &self.leaf_commitment,
        ] {
            write_canonical_bytes(&mut output, &mut offset, field);
        }
        write_canonical_bytes(&mut output, &mut offset, &self.leaf_index.to_le_bytes());
        write_canonical_bytes(&mut output, &mut offset, &self.registry_root);
        write_canonical_bytes(&mut output, &mut offset, &self.receipt_commitment);
        debug_assert_eq!(offset, output.len());
        output
    }
}

/// Verify a receipt against an independently authenticated governed checkpoint and exact
/// fixed-depth proof.  This function is intentionally authorization-free: verification is safe
/// offline, while checkpoint publication remains authority-gated.  It proves consistency only;
/// until a terminal adapter persists and enforces a monotonic checkpoint floor, replay of an older
/// otherwise-valid receipt/checkpoint pair is deliberately an activation blocker.
pub(super) fn verify_offline_registration_receipt_v2(
    receipt: &OfflineRegistrationReceiptV2,
    proof: &RegistryInclusionProofV2,
    checkpoint: &OfflineRegistryCheckpointV2,
) -> Result<(), NativeRegistrationErrorV2> {
    let leaf_commitment = registration_leaf_commitment(&receipt.payload)?;
    if leaf_commitment != receipt.leaf_commitment {
        return Err(NativeRegistrationErrorV2::ReceiptCommitmentMismatch);
    }
    if receipt.leaf_index != registry_leaf_index(&receipt.payload.device_key_id)? {
        return Err(NativeRegistrationErrorV2::NonCanonicalLeafIndex);
    }
    if proof.leaf_index != receipt.leaf_index || proof.claimed_root != receipt.registry_root {
        return Err(NativeRegistrationErrorV2::InclusionRootMismatch);
    }
    let root = registry_root_from_proof(receipt.leaf_commitment, proof)?;
    if root != receipt.registry_root {
        return Err(NativeRegistrationErrorV2::InclusionRootMismatch);
    }
    if checkpoint.policy_epoch != receipt.payload.policy_epoch
        || checkpoint.policy_digest != receipt.payload.policy_digest
        || checkpoint.registry_root_intent != receipt.payload.registry_root_intent
        || checkpoint.registry_root != receipt.registry_root
    {
        return Err(NativeRegistrationErrorV2::ReceiptGovernanceMismatch);
    }
    let expected_checkpoint = checkpoint_commitment(
        checkpoint.policy_epoch,
        &checkpoint.policy_digest,
        &checkpoint.registry_root_intent,
        &checkpoint.registry_root,
    )?;
    if expected_checkpoint != checkpoint.checkpoint_commitment {
        return Err(NativeRegistrationErrorV2::ReceiptCommitmentMismatch);
    }
    let expected_receipt = receipt_commitment(
        &receipt.payload,
        &receipt.leaf_commitment,
        receipt.leaf_index,
        &receipt.registry_root,
    )?;
    if expected_receipt != receipt.receipt_commitment {
        return Err(NativeRegistrationErrorV2::ReceiptCommitmentMismatch);
    }
    Ok(())
}

fn validate_and_prepare_registration(
    input: NativeAttestationRegistrationInputV2,
    policy: &GovernedRegistrationPolicyV2,
) -> Result<AuthenticatedRegistrationLeafV2, NativeRegistrationErrorV2> {
    validate_policy(policy)?;
    if input.policy_epoch != policy.policy_epoch {
        return Err(NativeRegistrationErrorV2::PolicyEpochMismatch);
    }
    if input.registry_root_intent != policy.registry_root_intent {
        return Err(NativeRegistrationErrorV2::RegistryIntentMismatch);
    }
    if digest_words(&input.helper_words, POLICY_WORD_START) != policy.policy_digest {
        return Err(NativeRegistrationErrorV2::PolicyDigestMismatch);
    }
    if input.certificate_chain_der.first() != Some(&input.certificate_der) {
        return Err(NativeRegistrationErrorV2::LeafCertificateMismatch);
    }

    let parsed_chain = parse_and_validate_chain(&input.certificate_chain_der, policy)?;
    let leaf = &parsed_chain[0];
    let issuer = &parsed_chain[1];
    let raw_tbs = leaf.tbs_certificate.as_ref();
    if raw_tbs.is_empty() {
        return Err(NativeRegistrationErrorV2::EmptyRawTbs);
    }
    if raw_tbs.len() > NATIVE_REGISTRATION_RAW_TBS_MAX_BYTES_V2 {
        return Err(NativeRegistrationErrorV2::RawTbsTooLarge {
            actual: raw_tbs.len(),
            maximum: NATIVE_REGISTRATION_RAW_TBS_MAX_BYTES_V2,
        });
    }

    let leaf_key = p256_sec1_key(leaf)?;
    if leaf_key != input.device_public_key_sec1 {
        return Err(NativeRegistrationErrorV2::DeviceKeyMismatch);
    }
    let device_key_id = sha256_bytes(&input.device_public_key_sec1);

    let issuer_key = p256_sec1_key(issuer)?;
    let issuer_key_digest = sha256_bytes(&issuer_key);
    let issuer_key_id = domain_hash(KEY_ID_DOMAIN, &[&issuer_key])?;
    if !policy.governed_issuer_key_ids.contains(&issuer_key_id) {
        return Err(NativeRegistrationErrorV2::UngovernedIssuer);
    }

    let leaf_attestation = exact_android_attestation_extension(leaf)?;
    if leaf_attestation != input.attestation_extension_der.as_slice() {
        return Err(NativeRegistrationErrorV2::AttestationExtensionMismatch);
    }
    validate_attestation_extension_der(&input.attestation_extension_der)?;

    let certificate_digest = sha256_bytes(&input.certificate_der);
    let tbs_digest = sha256_bytes(raw_tbs);
    let attestation_digest = sha256_bytes(&input.attestation_extension_der);
    let fixed_digests = verify_fixed_nine_jobs(
        &input.helper_words,
        &input.device_public_key_sec1,
        &issuer_key,
    )?;
    if device_key_id != input.device_key_id
        || device_key_id != digest_words(&input.helper_words, PLATFORM_KEY_WORD_START)
    {
        return Err(NativeRegistrationErrorV2::DeviceKeyIdMismatch);
    }
    for (offset, actual) in [
        (CERTIFICATE_WORD_START, certificate_digest),
        (TBS_WORD_START, tbs_digest),
        (ISSUER_KEY_WORD_START, issuer_key_digest),
        (ATTESTATION_WORD_START, attestation_digest),
    ] {
        if digest_words(&input.helper_words, offset) != actual {
            return Err(NativeRegistrationErrorV2::RawDigestMismatch { offset });
        }
    }
    let fixed_refs = fixed_digests
        .iter()
        .map(|digest| digest.as_slice())
        .collect::<Vec<_>>();
    let fixed_nine_digest = domain_hash(FIXED_NINE_DOMAIN, &fixed_refs)?;
    let chain_digest = certificate_chain_digest(input.policy_epoch, &input.certificate_chain_der)?;
    let root_certificate_digest = sha256_bytes(
        input
            .certificate_chain_der
            .last()
            .expect("chain depth was validated"),
    );
    let payload = RegistrationRegistryLeafPayloadV2 {
        policy_epoch: input.policy_epoch,
        policy_digest: policy.policy_digest,
        registry_root_intent: input.registry_root_intent,
        device_key_id: input.device_key_id,
        device_public_key_sec1: input.device_public_key_sec1,
        device_descriptor_digest: digest_words(&input.helper_words, DEVICE_WORD_START),
        certificate_digest,
        tbs_digest,
        issuer_key_id,
        issuer_key_digest,
        attestation_digest,
        chain_digest,
        root_certificate_digest,
        fixed_nine_digest,
        guard_bundle_digest: digest_words(&input.helper_words, BUNDLE_WORD_START),
    };
    let leaf_commitment = registration_leaf_commitment(&payload)?;
    let authorization_commitment = domain_hash(
        AUTHORIZATION_DOMAIN,
        &[
            &policy.policy_epoch.to_le_bytes(),
            &policy.policy_digest,
            &policy.registry_root_intent,
            &leaf_commitment,
        ],
    )?;
    Ok(AuthenticatedRegistrationLeafV2 {
        payload,
        leaf_commitment,
        authorization_commitment,
    })
}

fn validate_policy(policy: &GovernedRegistrationPolicyV2) -> Result<(), NativeRegistrationErrorV2> {
    if policy.policy_epoch == 0 {
        return Err(NativeRegistrationErrorV2::ZeroPolicyEpoch);
    }
    if policy.policy_digest == [0; 32] || policy.registry_root_intent == [0; 32] {
        return Err(NativeRegistrationErrorV2::ZeroDigest);
    }
    if policy.trusted_roots_der.is_empty()
        || policy.trusted_roots_der.len() > NATIVE_REGISTRATION_MAX_TRUSTED_ROOTS_V2
        || policy.revoked_certificate_digests.len()
            > NATIVE_REGISTRATION_MAX_REVOKED_CERTIFICATES_V2
        || policy.governed_issuer_key_ids.is_empty()
        || policy.governed_issuer_key_ids.len() > NATIVE_REGISTRATION_MAX_GOVERNED_ISSUERS_V2
        || !strictly_sorted(&policy.revoked_certificate_digests)
        || !strictly_sorted(&policy.governed_issuer_key_ids)
        || policy
            .revoked_certificate_digests
            .iter()
            .chain(policy.governed_issuer_key_ids.iter())
            .any(|digest| *digest == [0; 32])
    {
        return Err(NativeRegistrationErrorV2::InvalidGovernanceCollection);
    }
    let root_digests = policy
        .trusted_roots_der
        .iter()
        .map(|root| sha256_bytes(root))
        .collect::<Vec<_>>();
    if !strictly_sorted(&root_digests)
        || root_digests
            .iter()
            .any(|digest| policy.revoked_certificate_digests.contains(digest))
    {
        return Err(NativeRegistrationErrorV2::InvalidGovernanceCollection);
    }
    for (index, root_der) in policy.trusted_roots_der.iter().enumerate() {
        let root = parse_exact_certificate(index, root_der)?;
        validate_certificate_profile(&root)?;
        validate_certificate_time(&root, policy.evaluation_unix_seconds)?;
        if !certificate_is_ca(&root)? {
            return Err(NativeRegistrationErrorV2::InvalidCaUsage);
        }
        if root.issuer() == root.subject() {
            root.verify_signature(Some(root.public_key()))
                .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateSignature)?;
        }
    }
    Ok(())
}

fn parse_and_validate_chain<'a>(
    chain: &'a [Vec<u8>],
    policy: &GovernedRegistrationPolicyV2,
) -> Result<Vec<X509Certificate<'a>>, NativeRegistrationErrorV2> {
    if chain.len() < NATIVE_REGISTRATION_MIN_CHAIN_DEPTH_V2
        || chain.len() > NATIVE_REGISTRATION_MAX_CHAIN_DEPTH_V2
    {
        return Err(NativeRegistrationErrorV2::InvalidChainDepth {
            actual: chain.len(),
            minimum: NATIVE_REGISTRATION_MIN_CHAIN_DEPTH_V2,
            maximum: NATIVE_REGISTRATION_MAX_CHAIN_DEPTH_V2,
        });
    }
    let revoked = policy
        .revoked_certificate_digests
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::with_capacity(chain.len());
    for (index, certificate_der) in chain.iter().enumerate() {
        let digest = sha256_bytes(certificate_der);
        if !seen.insert(digest) {
            return Err(NativeRegistrationErrorV2::DuplicateCertificate);
        }
        if revoked.contains(&digest) {
            return Err(NativeRegistrationErrorV2::RevokedCertificate);
        }
        let certificate = parse_exact_certificate(index, certificate_der)?;
        validate_certificate_profile(&certificate)?;
        validate_certificate_time(&certificate, policy.evaluation_unix_seconds)?;
        parsed.push(certificate);
    }
    let leaf = parsed.first().expect("bounded chain is non-empty");
    if certificate_is_ca(leaf)? || !leaf_allows_digital_signature(leaf)? {
        return Err(NativeRegistrationErrorV2::InvalidLeafUsage);
    }
    for pair in parsed.windows(2) {
        let certificate = &pair[0];
        let issuer = &pair[1];
        if certificate.issuer() != issuer.subject() {
            return Err(NativeRegistrationErrorV2::InvalidIssuerOrder);
        }
        if !certificate_is_ca(issuer)? {
            return Err(NativeRegistrationErrorV2::InvalidCaUsage);
        }
        certificate
            .verify_signature(Some(issuer.public_key()))
            .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateSignature)?;
    }
    let root_der = chain.last().expect("bounded chain is non-empty");
    if !policy.trusted_roots_der.iter().any(|root| root == root_der) {
        return Err(NativeRegistrationErrorV2::UntrustedRoot);
    }
    let root = parsed.last().expect("bounded chain is non-empty");
    if !certificate_is_ca(root)? {
        return Err(NativeRegistrationErrorV2::InvalidCaUsage);
    }
    if root.issuer() == root.subject() {
        root.verify_signature(Some(root.public_key()))
            .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateSignature)?;
    }
    Ok(parsed)
}

fn parse_exact_certificate<'a>(
    index: usize,
    certificate_der: &'a [u8],
) -> Result<X509Certificate<'a>, NativeRegistrationErrorV2> {
    if certificate_der.is_empty()
        || certificate_der.len() > NATIVE_REGISTRATION_MAX_CERTIFICATE_BYTES_V2
    {
        return Err(NativeRegistrationErrorV2::CertificateSize {
            index,
            maximum: NATIVE_REGISTRATION_MAX_CERTIFICATE_BYTES_V2,
        });
    }
    let (remaining, certificate) = X509Certificate::from_der(certificate_der)
        .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateDer { index })?;
    if !remaining.is_empty() {
        return Err(NativeRegistrationErrorV2::TrailingCertificateDer { index });
    }
    validate_canonical_der(certificate_der).map_err(|error| match error {
        NativeRegistrationErrorV2::CanonicalDerAdapterUnavailable => error,
        _ => NativeRegistrationErrorV2::NonCanonicalCertificateDer { index },
    })?;
    if certificate.as_raw() != certificate_der {
        return Err(NativeRegistrationErrorV2::InvalidCertificateDer { index });
    }
    Ok(certificate)
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn validate_canonical_der(input: &[u8]) -> Result<(), NativeRegistrationErrorV2> {
    use crate::privacy_engines::zk_x509::der::{
        ZkX509DerLimitsV1, ZkX509DerTagV1, parse_single_der_value_v1,
    };

    parse_single_der_value_v1(input, ZkX509DerLimitsV1::profile())
        .and_then(|value| value.require_tag(ZkX509DerTagV1::SEQUENCE))
        .map(|_| ())
        .map_err(|_| NativeRegistrationErrorV2::NonCanonicalAttestationExtension)
}

#[cfg(not(any(test, feature = "privacy-release-evidence")))]
fn validate_canonical_der(_input: &[u8]) -> Result<(), NativeRegistrationErrorV2> {
    Err(NativeRegistrationErrorV2::CanonicalDerAdapterUnavailable)
}

fn validate_certificate_profile(
    certificate: &X509Certificate<'_>,
) -> Result<(), NativeRegistrationErrorV2> {
    if certificate.signature_algorithm.algorithm.to_id_string() != ECDSA_WITH_SHA256_OID
        || certificate.signature_algorithm.parameters.is_some()
        || certificate
            .tbs_certificate
            .signature
            .algorithm
            .to_id_string()
            != ECDSA_WITH_SHA256_OID
        || certificate.tbs_certificate.signature.parameters.is_some()
        || certificate.signature_algorithm != certificate.tbs_certificate.signature
    {
        return Err(NativeRegistrationErrorV2::InvalidCertificateAlgorithm);
    }
    let _ = p256_sec1_key(certificate)?;
    let mut extension_oids = BTreeSet::new();
    for extension in certificate.extensions() {
        if !extension_oids.insert(extension.oid.to_id_string()) {
            return Err(NativeRegistrationErrorV2::InvalidCertificateExtension);
        }
        if extension.critical
            && matches!(
                extension.parsed_extension(),
                ParsedExtension::UnsupportedExtension { .. }
                    | ParsedExtension::ParseError { .. }
                    | ParsedExtension::Unparsed
            )
        {
            return Err(NativeRegistrationErrorV2::InvalidCertificateExtension);
        }
    }
    Ok(())
}

fn p256_sec1_key(certificate: &X509Certificate<'_>) -> Result<[u8; 65], NativeRegistrationErrorV2> {
    let public_key = certificate.public_key();
    let curve = public_key
        .algorithm
        .parameters
        .as_ref()
        .and_then(|parameters| parameters.as_oid().ok())
        .map(|oid| oid.to_id_string());
    let bytes = public_key.subject_public_key.data.as_ref();
    if public_key.algorithm.algorithm.to_id_string() != EC_PUBLIC_KEY_OID
        || curve.as_deref() != Some(PRIME256V1_OID)
        || bytes.len() != 65
        || bytes.first() != Some(&4)
        || P256PublicKey::from_sec1_bytes(bytes).is_err()
    {
        return Err(NativeRegistrationErrorV2::InvalidCertificatePublicKey);
    }
    bytes
        .try_into()
        .map_err(|_| NativeRegistrationErrorV2::InvalidCertificatePublicKey)
}

fn validate_certificate_time(
    certificate: &X509Certificate<'_>,
    evaluation_unix_seconds: u64,
) -> Result<(), NativeRegistrationErrorV2> {
    let seconds = i64::try_from(evaluation_unix_seconds)
        .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateTime)?;
    let time = ASN1Time::from_timestamp(seconds)
        .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateTime)?;
    if certificate.validity().is_valid_at(time) {
        Ok(())
    } else {
        Err(NativeRegistrationErrorV2::InvalidCertificateTime)
    }
}

fn certificate_is_ca(certificate: &X509Certificate<'_>) -> Result<bool, NativeRegistrationErrorV2> {
    let Some(basic_constraints) = certificate
        .basic_constraints()
        .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateExtension)?
    else {
        return Ok(false);
    };
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateExtension)?
    else {
        return Ok(false);
    };
    Ok(basic_constraints.critical
        && basic_constraints.value.ca
        && key_usage.critical
        && key_usage.value.key_cert_sign())
}

fn leaf_allows_digital_signature(
    certificate: &X509Certificate<'_>,
) -> Result<bool, NativeRegistrationErrorV2> {
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| NativeRegistrationErrorV2::InvalidCertificateExtension)?
    else {
        return Ok(false);
    };
    Ok(key_usage.critical && key_usage.value.digital_signature())
}

fn exact_android_attestation_extension<'der>(
    certificate: &X509Certificate<'der>,
) -> Result<&'der [u8], NativeRegistrationErrorV2> {
    let mut extensions = certificate
        .extensions()
        .iter()
        .filter(|extension| extension.oid.to_id_string() == ANDROID_KEY_ATTESTATION_EXTENSION_OID);
    let value = extensions
        .next()
        .map(|extension| extension.value)
        .ok_or(NativeRegistrationErrorV2::InvalidAttestationExtension)?;
    if extensions.next().is_some() {
        return Err(NativeRegistrationErrorV2::InvalidAttestationExtension);
    }
    if value.is_empty() || value.len() > NATIVE_REGISTRATION_MAX_ATTESTATION_EXTENSION_BYTES_V2 {
        return Err(NativeRegistrationErrorV2::AttestationExtensionSize);
    }
    Ok(value)
}

fn validate_attestation_extension_der(
    extension_der: &[u8],
) -> Result<(), NativeRegistrationErrorV2> {
    validate_canonical_der(extension_der).map_err(|error| match error {
        NativeRegistrationErrorV2::CanonicalDerAdapterUnavailable => error,
        _ => NativeRegistrationErrorV2::NonCanonicalAttestationExtension,
    })
}

fn validate_helper_header(
    words: &[u32; NATIVE_REGISTRATION_HELPER_WORDS_V2],
) -> Result<(), NativeRegistrationErrorV2> {
    if words[0] != HELPER_ABI_VERSION
        || words[1] != HELPER_WIRE_VERSION
        || words[2] != HELPER_K
        || !matches!(words[3], 1 | 2)
        || !matches!(words[4], 2..=5)
        || words[7] != HELPER_DIGEST_WORDS
        || words[12] != HELPER_DIGEST_FIELDS
        || words[13] != HELPER_WORDS_PER_CELL
        || words[14] != HELPER_INSTANCE_CELLS
        || words[15] != 0
    {
        return Err(NativeRegistrationErrorV2::InvalidHelperHeader);
    }
    let expected_protocol =
        native_registration_source_helper_protocol_digest_v1(words[3], words[4])
            .ok_or(NativeRegistrationErrorV2::InvalidHelperHeader)?;
    if digest_words(words, PROTOCOL_WORD_START) != expected_protocol {
        return Err(NativeRegistrationErrorV2::InvalidHelperProtocolIdentity);
    }
    if !matches!(words[OPERATION_WORD], 1 | 2) {
        return Err(NativeRegistrationErrorV2::InvalidOperation);
    }
    if words[ANDROID_PRESENT_WORD] != 1 {
        return Err(NativeRegistrationErrorV2::AndroidAttestationRequired);
    }
    let from = u64::from(words[FROM_LOW_WORD]) | (u64::from(words[FROM_HIGH_WORD]) << 32);
    let to = u64::from(words[TO_LOW_WORD]) | (u64::from(words[TO_HIGH_WORD]) << 32);
    if from.checked_add(1) != Some(to) {
        return Err(NativeRegistrationErrorV2::InvalidSequence);
    }
    if ALL_DIGEST_OFFSETS
        .iter()
        .any(|offset| digest_words(words, *offset) == [0; 32])
    {
        return Err(NativeRegistrationErrorV2::ZeroDigest);
    }
    if digest_words(words, CURRENT_HEAD_WORD_START) == digest_words(words, TRANSITION_WORD_START)
        || digest_words(words, CURRENT_GUARD_WORD_START)
            == digest_words(words, NEXT_GUARD_WORD_START)
    {
        return Err(NativeRegistrationErrorV2::InvalidHelperDigestEquality);
    }
    Ok(())
}

pub(super) fn native_fixed_messages_v2(
    words: &[u32; NATIVE_REGISTRATION_HELPER_WORDS_V2],
    device_public_key_sec1: &[u8; 65],
    issuer_public_key_sec1: &[u8; 65],
) -> Result<[Vec<u8>; 9], NativeRegistrationErrorV2> {
    validate_helper_header(words)?;
    if device_public_key_sec1[0] != 4
        || issuer_public_key_sec1[0] != 4
        || P256PublicKey::from_sec1_bytes(device_public_key_sec1).is_err()
        || P256PublicKey::from_sec1_bytes(issuer_public_key_sec1).is_err()
    {
        return Err(NativeRegistrationErrorV2::InvalidCertificatePublicKey);
    }
    let operation = [u8::try_from(words[OPERATION_WORD])
        .map_err(|_| NativeRegistrationErrorV2::InvalidOperation)?];
    let android_present = [1_u8];
    let from =
        (u64::from(words[FROM_LOW_WORD]) | (u64::from(words[FROM_HIGH_WORD]) << 32)).to_le_bytes();
    let to = (u64::from(words[TO_LOW_WORD]) | (u64::from(words[TO_HIGH_WORD]) << 32)).to_le_bytes();
    let release = digest_words(words, RELEASE_WORD_START);
    let context = digest_words(words, CONTEXT_WORD_START);
    let current_head = digest_words(words, CURRENT_HEAD_WORD_START);
    let current_lineage = digest_words(words, CURRENT_LINEAGE_WORD_START);
    let transition = digest_words(words, TRANSITION_WORD_START);
    let wallet = digest_words(words, WALLET_WORD_START);
    let policy = digest_words(words, POLICY_WORD_START);
    let device = digest_words(words, DEVICE_WORD_START);
    let current_guard = digest_words(words, CURRENT_GUARD_WORD_START);
    let next_guard = digest_words(words, NEXT_GUARD_WORD_START);
    let platform_key_digest = digest_words(words, PLATFORM_KEY_WORD_START);
    let platform_message = digest_words(words, PLATFORM_MESSAGE_WORD_START);
    let guard_use = digest_words(words, GUARD_USE_CLAIM_WORD_START);
    let platform_bind = digest_words(words, PLATFORM_BIND_CLAIM_WORD_START);
    let certificate = digest_words(words, CERTIFICATE_WORD_START);
    let tbs = digest_words(words, TBS_WORD_START);
    let issuer = digest_words(words, ISSUER_KEY_WORD_START);
    let attestation = digest_words(words, ATTESTATION_WORD_START);
    let android_claim = digest_words(words, ANDROID_CLAIM_WORD_START);

    let messages = [
        framed_bytes(
            CURRENT_GUARD_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &wallet,
                &policy,
                &device,
                &from,
            ],
        )?,
        framed_bytes(
            NEXT_GUARD_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &to,
            ],
        )?,
        framed_bytes(
            PLATFORM_MESSAGE_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &next_guard,
                &from,
                &to,
            ],
        )?,
        device_public_key_sec1.to_vec(),
        framed_bytes(
            GUARD_USE_CLAIM_DOMAIN,
            &[
                &operation,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &next_guard,
                &from,
                &to,
                &platform_message,
            ],
        )?,
        framed_bytes(
            PLATFORM_BIND_CLAIM_DOMAIN,
            &[
                &release,
                &policy,
                &wallet,
                &device,
                &platform_key_digest,
                &platform_message,
                &current_guard,
                &next_guard,
            ],
        )?,
        issuer_public_key_sec1.to_vec(),
        framed_bytes(
            ANDROID_KEY_CERT_CLAIM_DOMAIN,
            &[
                &release,
                &policy,
                &device,
                &platform_key_digest,
                &certificate,
                &tbs,
                &issuer,
                &attestation,
                P256_ALGORITHM,
                ANDROID_KEY_ORIGIN,
                ANDROID_KEY_PURPOSE,
                ANDROID_DIGEST_MODE,
                &ANDROID_USAGE_LIMIT_ONE,
            ],
        )?,
        framed_bytes(
            GUARD_BUNDLE_DOMAIN,
            &[
                &operation,
                &android_present,
                &release,
                &context,
                &current_head,
                &current_lineage,
                &transition,
                &wallet,
                &policy,
                &device,
                &current_guard,
                &next_guard,
                &from,
                &to,
                &guard_use,
                &platform_bind,
                &android_claim,
            ],
        )?,
    ];
    if messages.each_ref().map(Vec::len) != NATIVE_REGISTRATION_FIXED_MESSAGE_BYTES_V2 {
        return Err(NativeRegistrationErrorV2::FixedMessageGeometry);
    }
    Ok(messages)
}

fn verify_fixed_nine_jobs(
    words: &[u32; NATIVE_REGISTRATION_HELPER_WORDS_V2],
    device_public_key_sec1: &[u8; 65],
    issuer_public_key_sec1: &[u8; 65],
) -> Result<[[u8; 32]; 9], NativeRegistrationErrorV2> {
    let messages = native_fixed_messages_v2(words, device_public_key_sec1, issuer_public_key_sec1)?;
    let digests = messages.each_ref().map(|message| sha256_bytes(message));
    for (job, (offset, digest)) in NATIVE_REGISTRATION_FIXED_JOB_DIGEST_OFFSETS_V2
        .iter()
        .zip(digests.iter())
        .enumerate()
    {
        if digest_words(words, *offset) != *digest {
            return Err(NativeRegistrationErrorV2::FixedJobDigestMismatch { job });
        }
    }
    Ok(digests)
}

fn registration_leaf_commitment(
    payload: &RegistrationRegistryLeafPayloadV2,
) -> Result<[u8; 32], NativeRegistrationErrorV2> {
    domain_hash(
        LEAF_DOMAIN,
        &[
            &SCHEMA_VERSION,
            &payload.policy_epoch.to_le_bytes(),
            &payload.policy_digest,
            &payload.registry_root_intent,
            &payload.device_key_id,
            &payload.device_public_key_sec1,
            &payload.device_descriptor_digest,
            &payload.certificate_digest,
            &payload.tbs_digest,
            &payload.issuer_key_id,
            &payload.issuer_key_digest,
            &payload.attestation_digest,
            &payload.chain_digest,
            &payload.root_certificate_digest,
            &payload.fixed_nine_digest,
            &payload.guard_bundle_digest,
        ],
    )
}

fn receipt_commitment(
    payload: &RegistrationRegistryLeafPayloadV2,
    leaf_commitment: &[u8; 32],
    leaf_index: u64,
    registry_root: &[u8; 32],
) -> Result<[u8; 32], NativeRegistrationErrorV2> {
    domain_hash(
        RECEIPT_DOMAIN,
        &[
            &SCHEMA_VERSION,
            &payload.policy_epoch.to_le_bytes(),
            &payload.policy_digest,
            &payload.registry_root_intent,
            &payload.device_key_id,
            &payload.device_public_key_sec1,
            &payload.device_descriptor_digest,
            &payload.certificate_digest,
            &payload.tbs_digest,
            &payload.issuer_key_id,
            &payload.issuer_key_digest,
            &payload.attestation_digest,
            &payload.chain_digest,
            &payload.root_certificate_digest,
            &payload.fixed_nine_digest,
            &payload.guard_bundle_digest,
            leaf_commitment,
            &leaf_index.to_le_bytes(),
            registry_root,
        ],
    )
}

fn checkpoint_commitment(
    policy_epoch: u64,
    policy_digest: &[u8; 32],
    registry_root_intent: &[u8; 32],
    registry_root: &[u8; 32],
) -> Result<[u8; 32], NativeRegistrationErrorV2> {
    domain_hash(
        CHECKPOINT_DOMAIN,
        &[
            &SCHEMA_VERSION,
            &policy_epoch.to_le_bytes(),
            policy_digest,
            registry_root_intent,
            registry_root,
        ],
    )
}

fn registry_root_from_proof(
    leaf_commitment: [u8; 32],
    proof: &RegistryInclusionProofV2,
) -> Result<[u8; 32], NativeRegistrationErrorV2> {
    if proof.claimed_root == [0; 32] || proof.siblings.iter().any(|sibling| *sibling == [0; 32]) {
        return Err(NativeRegistrationErrorV2::ZeroInclusionDigest);
    }
    let mut node = domain_hash(REGISTRY_LEAF_DOMAIN, &[&leaf_commitment])?;
    for (level, sibling) in proof.siblings.iter().enumerate() {
        let level_bytes = u32::try_from(level)
            .map_err(|_| NativeRegistrationErrorV2::FramingOverflow)?
            .to_le_bytes();
        let (left, right) = if proof.leaf_index & (1_u64 << level) == 0 {
            (&node, sibling)
        } else {
            (sibling, &node)
        };
        node = domain_hash(REGISTRY_NODE_DOMAIN, &[&level_bytes, left, right])?;
    }
    Ok(node)
}

fn registry_leaf_index(device_key_id: &[u8; 32]) -> Result<u64, NativeRegistrationErrorV2> {
    let digest = domain_hash(REGISTRY_INDEX_DOMAIN, &[device_key_id])?;
    Ok(u64::from_le_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 digest has at least eight bytes"),
    ))
}

fn certificate_chain_digest(
    policy_epoch: u64,
    certificate_chain_der: &[Vec<u8>],
) -> Result<[u8; 32], NativeRegistrationErrorV2> {
    let epoch = policy_epoch.to_le_bytes();
    let depth = u32::try_from(certificate_chain_der.len())
        .map_err(|_| NativeRegistrationErrorV2::FramingOverflow)?
        .to_le_bytes();
    let mut fields = Vec::with_capacity(certificate_chain_der.len() + 2);
    fields.push(epoch.as_slice());
    fields.push(depth.as_slice());
    fields.extend(certificate_chain_der.iter().map(Vec::as_slice));
    domain_hash(CHAIN_DOMAIN, &fields)
}

fn domain_hash(domain: &[u8], fields: &[&[u8]]) -> Result<[u8; 32], NativeRegistrationErrorV2> {
    Ok(sha256_bytes(&framed_bytes(domain, fields)?))
}

fn framed_bytes(domain: &[u8], fields: &[&[u8]]) -> Result<Vec<u8>, NativeRegistrationErrorV2> {
    let length = fields.iter().try_fold(
        8_usize
            .checked_add(domain.len())
            .ok_or(NativeRegistrationErrorV2::FramingOverflow)?,
        |length, field| {
            length
                .checked_add(8)
                .and_then(|length| length.checked_add(field.len()))
                .ok_or(NativeRegistrationErrorV2::FramingOverflow)
        },
    )?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| NativeRegistrationErrorV2::FramingOverflow)?;
    output.extend_from_slice(
        &u64::try_from(domain.len())
            .map_err(|_| NativeRegistrationErrorV2::FramingOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(domain);
    for field in fields {
        output.extend_from_slice(
            &u64::try_from(field.len())
                .map_err(|_| NativeRegistrationErrorV2::FramingOverflow)?
                .to_le_bytes(),
        );
        output.extend_from_slice(field);
    }
    debug_assert_eq!(output.len(), length);
    Ok(output)
}

fn write_canonical_bytes(output: &mut [u8], offset: &mut usize, bytes: &[u8]) {
    let end = (*offset)
        .checked_add(bytes.len())
        .expect("fixed canonical registration encoding cannot overflow usize");
    output[*offset..end].copy_from_slice(bytes);
    *offset = end;
}

fn sha256_bytes(input: &[u8]) -> [u8; 32] {
    Sha256::digest(input).into()
}

fn digest_words(words: &[u32; NATIVE_REGISTRATION_HELPER_WORDS_V2], offset: usize) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (destination, word) in digest.chunks_exact_mut(4).zip(&words[offset..offset + 8]) {
        destination.copy_from_slice(&word.to_le_bytes());
    }
    digest
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

#[cfg(test)]
pub(super) fn write_digest_words_for_test_v2(
    words: &mut [u32; NATIVE_REGISTRATION_HELPER_WORDS_V2],
    offset: usize,
    digest: [u8; 32],
) {
    for (index, bytes) in digest.chunks_exact(4).enumerate() {
        words[offset + index] = u32::from_le_bytes(bytes.try_into().expect("four-byte chunk"));
    }
}

#[cfg(test)]
pub(super) fn duplicate_input_for_test_v2(
    input: &NativeAttestationRegistrationInputV2,
) -> NativeAttestationRegistrationInputV2 {
    NativeAttestationRegistrationInputV2::new(
        input.helper_words,
        input.device_public_key_sec1,
        input.device_key_id,
        input.policy_epoch,
        input.registry_root_intent,
        input.certificate_der.clone(),
        input.certificate_chain_der.clone(),
        input.attestation_extension_der.clone(),
    )
    .expect("test-only duplicate preserves a previously validated structural input")
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn governed_policy_for_test_v2(
    policy_epoch: u64,
    policy_digest: [u8; 32],
    registry_root_intent: [u8; 32],
    evaluation_unix_seconds: u64,
    mut trusted_roots_der: Vec<Vec<u8>>,
    mut revoked_certificate_digests: Vec<[u8; 32]>,
    mut governed_issuer_key_ids: Vec<[u8; 32]>,
) -> GovernedRegistrationPolicyV2 {
    trusted_roots_der.sort_by_key(|root| sha256_bytes(root));
    revoked_certificate_digests.sort_unstable();
    governed_issuer_key_ids.sort_unstable();
    GovernedRegistrationPolicyV2 {
        policy_epoch,
        policy_digest,
        registry_root_intent,
        evaluation_unix_seconds,
        trusted_roots_der,
        revoked_certificate_digests,
        governed_issuer_key_ids,
    }
}

#[cfg(test)]
pub(super) fn sha256_for_test_v2(input: &[u8]) -> [u8; 32] {
    sha256_bytes(input)
}

#[cfg(test)]
pub(super) fn issuer_key_id_for_test_v2(issuer_public_key_sec1: &[u8; 65]) -> [u8; 32] {
    domain_hash(KEY_ID_DOMAIN, &[issuer_public_key_sec1])
        .expect("fixed-size key-id framing cannot overflow")
}

#[cfg(test)]
pub(super) struct TestHostRegistrationAuthorityV2 {
    policy: GovernedRegistrationPolicyV2,
    available: bool,
}

#[cfg(test)]
impl TestHostRegistrationAuthorityV2 {
    pub(super) const fn new(policy: GovernedRegistrationPolicyV2) -> Self {
        Self {
            policy,
            available: true,
        }
    }

    pub(super) fn mint(
        &mut self,
        input: NativeAttestationRegistrationInputV2,
    ) -> Result<AuthenticatedRegistrationLeafV2, NativeRegistrationErrorV2> {
        if !self.available {
            return Err(NativeRegistrationErrorV2::AuthoritySpent);
        }
        self.available = false;
        validate_and_prepare_registration(input, &self.policy)
    }
}

#[cfg(test)]
pub(super) struct TestOfflineInclusionAuthorityV2 {
    policy_epoch: u64,
    policy_digest: [u8; 32],
    registry_root_intent: [u8; 32],
    expected_root: [u8; 32],
    available: bool,
}

#[cfg(test)]
impl TestOfflineInclusionAuthorityV2 {
    pub(super) const fn new(
        policy_epoch: u64,
        policy_digest: [u8; 32],
        registry_root_intent: [u8; 32],
        expected_root: [u8; 32],
    ) -> Self {
        Self {
            policy_epoch,
            policy_digest,
            registry_root_intent,
            expected_root,
            available: true,
        }
    }

    pub(super) fn issue(
        &mut self,
        leaf: AuthenticatedRegistrationLeafV2,
        proof: &RegistryInclusionProofV2,
    ) -> Result<OfflineRegistrationReceiptV2, NativeRegistrationErrorV2> {
        if !self.available {
            return Err(NativeRegistrationErrorV2::AuthoritySpent);
        }
        self.available = false;
        if leaf.payload.policy_epoch != self.policy_epoch
            || leaf.payload.policy_digest != self.policy_digest
            || leaf.payload.registry_root_intent != self.registry_root_intent
        {
            return Err(NativeRegistrationErrorV2::ReceiptGovernanceMismatch);
        }
        if proof.leaf_index != registry_leaf_index(&leaf.payload.device_key_id)? {
            return Err(NativeRegistrationErrorV2::NonCanonicalLeafIndex);
        }
        let root = registry_root_from_proof(leaf.leaf_commitment, proof)?;
        if root != proof.claimed_root || root != self.expected_root {
            return Err(NativeRegistrationErrorV2::InclusionRootMismatch);
        }
        let receipt_commitment = receipt_commitment(
            &leaf.payload,
            &leaf.leaf_commitment,
            proof.leaf_index,
            &root,
        )?;
        Ok(OfflineRegistrationReceiptV2 {
            payload: leaf.payload,
            leaf_commitment: leaf.leaf_commitment,
            leaf_index: proof.leaf_index,
            registry_root: root,
            receipt_commitment,
        })
    }

    pub(super) fn checkpoint(
        &self,
    ) -> Result<OfflineRegistryCheckpointV2, NativeRegistrationErrorV2> {
        let checkpoint_commitment = checkpoint_commitment(
            self.policy_epoch,
            &self.policy_digest,
            &self.registry_root_intent,
            &self.expected_root,
        )?;
        Ok(OfflineRegistryCheckpointV2 {
            policy_epoch: self.policy_epoch,
            policy_digest: self.policy_digest,
            registry_root_intent: self.registry_root_intent,
            registry_root: self.expected_root,
            checkpoint_commitment,
        })
    }
}

#[cfg(test)]
pub(super) fn registry_root_for_test_v2(
    leaf_commitment: [u8; 32],
    leaf_index: u64,
    siblings: [[u8; 32]; NATIVE_REGISTRY_DEPTH_V2],
) -> [u8; 32] {
    let placeholder = [1_u8; 32];
    let proof = RegistryInclusionProofV2 {
        leaf_index,
        siblings,
        claimed_root: placeholder,
    };
    registry_root_from_proof(leaf_commitment, &proof)
        .expect("test proof uses bounded non-zero siblings")
}

#[cfg(test)]
pub(super) fn registry_leaf_index_for_test_v2(device_key_id: &[u8; 32]) -> u64 {
    registry_leaf_index(device_key_id).expect("fixed-size registry index framing cannot overflow")
}

#[cfg(test)]
pub(super) fn mutate_receipt_commitment_for_test_v2(receipt: &mut OfflineRegistrationReceiptV2) {
    receipt.receipt_commitment[0] ^= 1;
}

#[cfg(test)]
pub(super) fn unordered_governed_issuers_for_test_v2(policy: &mut GovernedRegistrationPolicyV2) {
    policy.governed_issuer_key_ids.reverse();
}

#[cfg(test)]
pub(super) fn set_policy_evaluation_time_for_test_v2(
    policy: &mut GovernedRegistrationPolicyV2,
    evaluation_unix_seconds: u64,
) {
    policy.evaluation_unix_seconds = evaluation_unix_seconds;
}

#[cfg(test)]
pub(super) fn revoke_certificate_for_test_v2(
    policy: &mut GovernedRegistrationPolicyV2,
    certificate_digest: [u8; 32],
) {
    policy.revoked_certificate_digests.push(certificate_digest);
    policy.revoked_certificate_digests.sort_unstable();
}

#[cfg(test)]
pub(super) fn replace_revocations_for_test_v2(
    policy: &mut GovernedRegistrationPolicyV2,
    mut certificate_digests: Vec<[u8; 32]>,
) {
    certificate_digests.sort_unstable();
    policy.revoked_certificate_digests = certificate_digests;
}

#[cfg(test)]
pub(super) fn mutate_checkpoint_commitment_for_test_v2(
    checkpoint: &mut OfflineRegistryCheckpointV2,
) {
    checkpoint.checkpoint_commitment[0] ^= 1;
}
