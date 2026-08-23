//! Field-neutral public contracts for the undeclared Offline Cash V2 compact-SHA candidate.
//!
//! This module is deliberately not declared by `offline_cash_v2.rs`.  It records the exact
//! fixed helper batch and a separately bounded raw-`TBSCertificate` payload contract without
//! changing any V1 wire, role, protocol, artifact, or verifier identity.  The fixed ABI preserves
//! all 184 V1 helper words, including the sixteen-word header, then continuously repacks those
//! words with two SEC1 keys into a new V2 field layout.  It is not a byte-for-byte prefix of the
//! standalone V1 27-cell layout: the first key occupies slots that are implicit zero padding in
//! the final standalone V1 cell.

use core::fmt;

use halo2_proofs::halo2curves::ff::PrimeField;

pub(super) const COMPACT_SHA_K_V2: u32 = 17;
pub(super) const COMPACT_SHA_USABLE_ROWS_V2: usize = (1 << COMPACT_SHA_K_V2) - 9;
pub(super) const COMPACT_SHA_SOURCE_HELPER_K_V1: u32 = 16;

pub(super) const COMPACT_SHA_HELPER_WORDS_V2: usize = 184;
pub(super) const COMPACT_SHA_KEY_WORDS_V2: usize = 17;
pub(super) const COMPACT_SHA_BATCH_WORDS_V2: usize =
    COMPACT_SHA_HELPER_WORDS_V2 + 2 * COMPACT_SHA_KEY_WORDS_V2;
pub(super) const COMPACT_SHA_WORDS_PER_INSTANCE_V2: usize = 7;
pub(super) const COMPACT_SHA_BATCH_INSTANCE_CELLS_V2: usize =
    COMPACT_SHA_BATCH_WORDS_V2.div_ceil(COMPACT_SHA_WORDS_PER_INSTANCE_V2);
pub(super) const COMPACT_SHA_BATCH_FINAL_ZERO_WORDS_V2: usize = COMPACT_SHA_BATCH_INSTANCE_CELLS_V2
    * COMPACT_SHA_WORDS_PER_INSTANCE_V2
    - COMPACT_SHA_BATCH_WORDS_V2;

pub(super) const COMPACT_SHA_RAW_TBS_MAX_BYTES_V2: usize = 3_767;
pub(super) const COMPACT_SHA_RAW_TBS_MAX_BLOCKS_V2: usize = 59;
pub(super) const COMPACT_SHA_RAW_TBS_WORDS_V2: usize = COMPACT_SHA_RAW_TBS_MAX_BYTES_V2.div_ceil(4);
/// Projected word count for a future aggregate raw-TBS public ABI.
///
/// No aggregate encoder, decoder, circuit, or verifier consumes this layout yet.
pub(super) const COMPACT_SHA_RAW_TBS_INSTANCE_WORDS_V2: usize =
    COMPACT_SHA_HELPER_WORDS_V2 + COMPACT_SHA_KEY_WORDS_V2 + 1 + COMPACT_SHA_RAW_TBS_WORDS_V2;
/// Projected cell count for a future aggregate raw-TBS public ABI.
pub(super) const COMPACT_SHA_RAW_TBS_INSTANCE_CELLS_V2: usize =
    COMPACT_SHA_RAW_TBS_INSTANCE_WORDS_V2.div_ceil(COMPACT_SHA_WORDS_PER_INSTANCE_V2);
/// Projected final-cell padding for a future aggregate raw-TBS public ABI.
pub(super) const COMPACT_SHA_RAW_TBS_FINAL_ZERO_WORDS_V2: usize =
    COMPACT_SHA_RAW_TBS_INSTANCE_CELLS_V2 * COMPACT_SHA_WORDS_PER_INSTANCE_V2
        - COMPACT_SHA_RAW_TBS_INSTANCE_WORDS_V2;

pub(super) const COMPACT_SHA_FIXED_MESSAGE_BYTES_V2: [usize; 9] =
    [355, 432, 494, 65, 533, 376, 65, 480, 619];
pub(super) const COMPACT_SHA_FIXED_MESSAGE_BLOCKS_V2: [usize; 9] = [6, 7, 8, 2, 9, 7, 2, 8, 10];
pub(super) const COMPACT_SHA_FIXED_MESSAGE_TOTAL_BYTES_V2: usize = 3_419;
pub(super) const COMPACT_SHA_FIXED_BLOCKS_V2: usize = 59;

/// A source implementation of the current-row machine and fixed-batch contract is present.
///
/// This does not assert that these undeclared files compile, fit k=17, synthesize, or have an
/// authenticated proving/verifying artifact.
pub(super) const COMPACT_SHA_BATCH_MACHINE_SOURCE_IMPLEMENTED_V2: bool = true;
/// The bounded field-neutral raw-TBS payload codec is implemented.
pub(super) const COMPACT_SHA_RAW_TBS_PAYLOAD_CODEC_IMPLEMENTED_V2: bool = true;
/// No complete helper+issuer+length+payload raw-TBS aggregate ABI is implemented.
pub(super) const COMPACT_SHA_RAW_TBS_AGGREGATE_ABI_IMPLEMENTED_V2: bool = false;
/// These undeclared files have no compiler evidence.
pub(super) const COMPACT_SHA_COMPILE_EVIDENCE_AVAILABLE_V2: bool = false;
pub(super) const COMPACT_SHA_BATCH_ROW_QUALIFIED_V2: bool = false;
pub(super) const COMPACT_SHA_RAW_TBS_CIRCUIT_IMPLEMENTED_V2: bool = false;
pub(super) const COMPACT_SHA_PRODUCTION_AVAILABLE_V2: bool = false;
pub(super) const COMPACT_SHA_ARTIFACT_EVIDENCE_AVAILABLE_V2: bool = false;
pub(super) const COMPACT_SHA_RECURSIVE_ADAPTER_AVAILABLE_V2: bool = false;
pub(super) const COMPACT_SHA_RELEASE_ELIGIBLE_V2: bool = false;

pub(super) const COMPACT_SHA_PUBLIC_ABI_REVISION_V2: &[u8] =
    b"offline-cash-v2-conditional-compact-sha/u32le-v1-helper-words184-continuously-repacked+sec1-17+sec1-17/pack7/direct-one-column/final-zero6/not-v1-cell-prefix/v2";
/// Future transcript target; no current adapter or verifier implements it.
pub(super) const COMPACT_SHA_TRANSCRIPT_TARGET_V2: &[u8] =
    b"Blake2bRead+Blake2bWrite/Challenge255/direct-instance/future-parent-mode-before-instances/exact-proof-length/v1";
/// Future parser/verifier target; it is not current acceptance authority.
pub(super) const COMPACT_SHA_CANONICALITY_TARGET_V2: &[u8] =
    b"pasta-field-capacity-at-least-225/canonical-field-encoding/no-reduction-alias/sec1-prefix04+terminal-zero3/exact-3232-byte-proof+32-byte-augmentation/no-trailing-bytes/future-verifier-derived-instances/v2";

/// External role selection is transcript-bound by the future parent; it is not another packed
/// word.  Keeping it outside the payload preserves the exact 218-word fixed contract and the
/// separately labeled 1,144-word raw aggregate projection below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum CompactShaPublicModeV2 {
    FixedNineJob = 1,
    RawTbs = 2,
}

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

const OPERATION_WORD: usize = 5;
const ANDROID_PRESENT_WORD: usize = 6;
const FROM_LOW_WORD: usize = 8;
const FROM_HIGH_WORD: usize = 9;
const TO_LOW_WORD: usize = 10;
const TO_HIGH_WORD: usize = 11;

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
const ANDROID_CERTIFICATE_WORD_START: usize = 136;
const ANDROID_TBS_WORD_START: usize = 144;
const ANDROID_ISSUER_KEY_WORD_START: usize = 152;
const ANDROID_ATTESTATION_WORD_START: usize = 160;
const ANDROID_CLAIM_WORD_START: usize = 168;
const BUNDLE_WORD_START: usize = 176;

const SOURCE_HELPER_ABI_VERSION: u32 = 1;
const SOURCE_HELPER_WIRE_VERSION: u32 = 1;
const SOURCE_HELPER_DIGEST_WORDS: u32 = 8;
const SOURCE_HELPER_DIGEST_FIELDS: u32 = 21;
const SOURCE_HELPER_WORDS_PER_CELL: u32 = 7;
const SOURCE_HELPER_INSTANCE_CELLS: u32 = 27;

const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("invalid embedded V1 protocol digest"),
    }
}

const fn digest_from_hex(hex: &[u8; 64]) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    let mut index = 0;
    while index < digest.len() {
        digest[index] = (hex_nibble(hex[2 * index]) << 4) | hex_nibble(hex[2 * index + 1]);
        index += 1;
    }
    digest
}

/// Exact V1 protocol identity duplicated at this undeclared boundary because the V1 helper
/// validator is private.  Candidate tests compare every entry with the live V1 identity function.
pub(super) const fn compact_sha_source_helper_protocol_digest_v1(
    parity: u32,
    role: u32,
) -> Option<[u8; 32]> {
    let digest = match (parity, role) {
        (1, 2) => {
            digest_from_hex(b"a0ac04d998f0e0f258d49c246c91e0642716219f3194cdb02bcaf39a7e58056f")
        }
        (1, 3) => {
            digest_from_hex(b"bea057eb94775db5cccfa58a422cd837d823aa98756956136eafecc6a92b9b67")
        }
        (1, 4) => {
            digest_from_hex(b"dc2aae8a1bac738c7c165c6610799015c2198394bbe1d83f7eb0424ecea1aba8")
        }
        (1, 5) => {
            digest_from_hex(b"919ecb15bb10adb53676c222bf95b099c6d16a236557a7987e3708625042936f")
        }
        (2, 2) => {
            digest_from_hex(b"e07865b759cf9b3dc5b367144a73ec7e64fd404db8885d5f6a8f1cea2f163bf9")
        }
        (2, 3) => {
            digest_from_hex(b"956d9c0a4284dcf2ea263342b4f0199802a1e7de0fa257c22eda0abdff720799")
        }
        (2, 4) => {
            digest_from_hex(b"f6f0cd2e1bc1d7b493a0b2568220d1536448977dad24ea834dc6b8b232aef667")
        }
        (2, 5) => {
            digest_from_hex(b"993041665fa8e420ca8509b70cd8e460f65018fda09cd348bd7ad6ef756c6f7a")
        }
        _ => return None,
    };
    Some(digest)
}

const _: () = assert!(COMPACT_SHA_BATCH_WORDS_V2 == 218);
const _: () = assert!(COMPACT_SHA_BATCH_INSTANCE_CELLS_V2 == 32);
const _: () = assert!(COMPACT_SHA_BATCH_FINAL_ZERO_WORDS_V2 == 6);
const _: () = assert!(COMPACT_SHA_RAW_TBS_WORDS_V2 == 942);
const _: () = assert!((COMPACT_SHA_RAW_TBS_MAX_BYTES_V2 + 9).div_ceil(64) == 59);
const _: () = assert!((COMPACT_SHA_RAW_TBS_MAX_BYTES_V2 + 1 + 9).div_ceil(64) == 60);
const _: () = assert!(COMPACT_SHA_RAW_TBS_MAX_BLOCKS_V2 == 59);
const _: () = assert!(COMPACT_SHA_RAW_TBS_INSTANCE_WORDS_V2 == 1_144);
const _: () = assert!(COMPACT_SHA_RAW_TBS_INSTANCE_CELLS_V2 == 164);
const _: () = assert!(COMPACT_SHA_RAW_TBS_FINAL_ZERO_WORDS_V2 == 4);
const _: () = assert!(COMPACT_SHA_FIXED_MESSAGE_TOTAL_BYTES_V2 == 3_419);
const _: () = assert!(COMPACT_SHA_FIXED_BLOCKS_V2 == 59);
const _: () = assert!(COMPACT_SHA_BATCH_MACHINE_SOURCE_IMPLEMENTED_V2);
const _: () = assert!(COMPACT_SHA_RAW_TBS_PAYLOAD_CODEC_IMPLEMENTED_V2);
const _: () = assert!(!COMPACT_SHA_RAW_TBS_AGGREGATE_ABI_IMPLEMENTED_V2);
const _: () = assert!(!COMPACT_SHA_COMPILE_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!COMPACT_SHA_BATCH_ROW_QUALIFIED_V2);
const _: () = assert!(!COMPACT_SHA_RAW_TBS_CIRCUIT_IMPLEMENTED_V2);
const _: () = assert!(!COMPACT_SHA_PRODUCTION_AVAILABLE_V2);
const _: () = assert!(!COMPACT_SHA_ARTIFACT_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!COMPACT_SHA_RECURSIVE_ADAPTER_AVAILABLE_V2);
const _: () = assert!(!COMPACT_SHA_RELEASE_ELIGIBLE_V2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CompactShaAbiErrorV2 {
    InvalidHeader,
    InvalidProtocolIdentity,
    InvalidOperation,
    AndroidCertificateRequired,
    InvalidSequence,
    InvalidDigest,
    InvalidGuardTransition,
    InvalidSec1Key,
    InvalidMessageGeometry,
    EmptyRawTbs,
    RawTbsCapExceeded { actual: usize, maximum: usize },
    NonCanonicalRawTbsPayload,
    ArithmeticOverflow,
}

impl fmt::Display for CompactShaAbiErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => formatter.write_str("invalid compact-SHA V2 helper header"),
            Self::InvalidProtocolIdentity => {
                formatter.write_str("invalid compact-SHA V2 source helper protocol identity")
            }
            Self::InvalidOperation => formatter.write_str("invalid compact-SHA V2 operation"),
            Self::AndroidCertificateRequired => {
                formatter.write_str("fixed compact-SHA batch requires Android certificate inputs")
            }
            Self::InvalidSequence => formatter.write_str("invalid compact-SHA V2 sequence"),
            Self::InvalidDigest => formatter.write_str("invalid compact-SHA V2 digest layout"),
            Self::InvalidGuardTransition => {
                formatter.write_str("invalid compact-SHA V2 guard/head transition")
            }
            Self::InvalidSec1Key => formatter.write_str("invalid compact-SHA V2 SEC1 key"),
            Self::InvalidMessageGeometry => {
                formatter.write_str("invalid compact-SHA V2 fixed-message geometry")
            }
            Self::EmptyRawTbs => formatter.write_str("raw TBSCertificate cannot be empty"),
            Self::RawTbsCapExceeded { actual, maximum } => write!(
                formatter,
                "raw TBSCertificate length {actual} exceeds governed candidate cap {maximum}"
            ),
            Self::NonCanonicalRawTbsPayload => {
                formatter.write_str("non-canonical compact-SHA V2 raw-TBS payload")
            }
            Self::ArithmeticOverflow => formatter.write_str("compact-SHA V2 arithmetic overflow"),
        }
    }
}

impl std::error::Error for CompactShaAbiErrorV2 {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompactShaBatchPublicAbiV2 {
    helper_words: [u32; COMPACT_SHA_HELPER_WORDS_V2],
    platform_public_key_sec1: [u8; 65],
    android_issuer_public_key_sec1: [u8; 65],
}

impl CompactShaBatchPublicAbiV2 {
    pub(super) fn new(
        helper_words: [u32; COMPACT_SHA_HELPER_WORDS_V2],
        platform_public_key_sec1: [u8; 65],
        android_issuer_public_key_sec1: [u8; 65],
    ) -> Result<Self, CompactShaAbiErrorV2> {
        if helper_words[0] != SOURCE_HELPER_ABI_VERSION
            || helper_words[1] != SOURCE_HELPER_WIRE_VERSION
            || helper_words[2] != COMPACT_SHA_SOURCE_HELPER_K_V1
            || !matches!(helper_words[3], 1 | 2)
            || !matches!(helper_words[4], 2..=5)
            || helper_words[7] != SOURCE_HELPER_DIGEST_WORDS
            || helper_words[12] != SOURCE_HELPER_DIGEST_FIELDS
            || helper_words[13] != SOURCE_HELPER_WORDS_PER_CELL
            || helper_words[14] != SOURCE_HELPER_INSTANCE_CELLS
            || helper_words[15] != 0
        {
            return Err(CompactShaAbiErrorV2::InvalidHeader);
        }
        let expected_protocol =
            compact_sha_source_helper_protocol_digest_v1(helper_words[3], helper_words[4])
                .ok_or(CompactShaAbiErrorV2::InvalidHeader)?;
        if digest_bytes(&helper_words, 16) != expected_protocol {
            return Err(CompactShaAbiErrorV2::InvalidProtocolIdentity);
        }
        if !matches!(helper_words[OPERATION_WORD], 1 | 2) {
            return Err(CompactShaAbiErrorV2::InvalidOperation);
        }
        if helper_words[ANDROID_PRESENT_WORD] != 1 {
            return Err(CompactShaAbiErrorV2::AndroidCertificateRequired);
        }
        let from = u64::from(helper_words[FROM_LOW_WORD])
            | (u64::from(helper_words[FROM_HIGH_WORD]) << 32);
        let to =
            u64::from(helper_words[TO_LOW_WORD]) | (u64::from(helper_words[TO_HIGH_WORD]) << 32);
        if from.checked_add(1) != Some(to) {
            return Err(CompactShaAbiErrorV2::InvalidSequence);
        }
        if platform_public_key_sec1[0] != 4 || android_issuer_public_key_sec1[0] != 4 {
            return Err(CompactShaAbiErrorV2::InvalidSec1Key);
        }
        for offset in [
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
            ANDROID_CERTIFICATE_WORD_START,
            ANDROID_TBS_WORD_START,
            ANDROID_ISSUER_KEY_WORD_START,
            ANDROID_ATTESTATION_WORD_START,
            ANDROID_CLAIM_WORD_START,
            BUNDLE_WORD_START,
        ] {
            if digest_bytes(&helper_words, offset) == [0; 32] {
                return Err(CompactShaAbiErrorV2::InvalidDigest);
            }
        }
        if digest_bytes(&helper_words, CURRENT_GUARD_WORD_START)
            == digest_bytes(&helper_words, NEXT_GUARD_WORD_START)
            || digest_bytes(&helper_words, CURRENT_HEAD_WORD_START)
                == digest_bytes(&helper_words, TRANSITION_WORD_START)
        {
            return Err(CompactShaAbiErrorV2::InvalidGuardTransition);
        }
        Ok(Self {
            helper_words,
            platform_public_key_sec1,
            android_issuer_public_key_sec1,
        })
    }

    pub(super) const fn helper_words(&self) -> &[u32; COMPACT_SHA_HELPER_WORDS_V2] {
        &self.helper_words
    }

    pub(super) const fn mode(&self) -> CompactShaPublicModeV2 {
        CompactShaPublicModeV2::FixedNineJob
    }

    pub(super) fn words(&self) -> [u32; COMPACT_SHA_BATCH_WORDS_V2] {
        let mut words = [0_u32; COMPACT_SHA_BATCH_WORDS_V2];
        words[..COMPACT_SHA_HELPER_WORDS_V2].copy_from_slice(&self.helper_words);
        write_sec1_words(
            &mut words[COMPACT_SHA_HELPER_WORDS_V2
                ..COMPACT_SHA_HELPER_WORDS_V2 + COMPACT_SHA_KEY_WORDS_V2],
            &self.platform_public_key_sec1,
        );
        write_sec1_words(
            &mut words[COMPACT_SHA_HELPER_WORDS_V2 + COMPACT_SHA_KEY_WORDS_V2..],
            &self.android_issuer_public_key_sec1,
        );
        words
    }

    pub(super) fn field_instances<F: PrimeField>(
        &self,
    ) -> [F; COMPACT_SHA_BATCH_INSTANCE_CELLS_V2] {
        assert!(
            F::NUM_BITS > 224,
            "compact-SHA V2 seven-word packing requires at least 225 field bits"
        );
        let words = self.words();
        std::array::from_fn(|cell| {
            let start = cell * COMPACT_SHA_WORDS_PER_INSTANCE_V2;
            let end = (start + COMPACT_SHA_WORDS_PER_INSTANCE_V2).min(words.len());
            pack_words::<F>(&words[start..end])
        })
    }

    pub(super) fn fixed_messages(&self) -> Result<[Vec<u8>; 9], CompactShaAbiErrorV2> {
        let operation = [u8::try_from(self.helper_words[OPERATION_WORD])
            .map_err(|_| CompactShaAbiErrorV2::InvalidOperation)?];
        let android_present = [1_u8];
        let from = (u64::from(self.helper_words[FROM_LOW_WORD])
            | (u64::from(self.helper_words[FROM_HIGH_WORD]) << 32))
            .to_le_bytes();
        let to = (u64::from(self.helper_words[TO_LOW_WORD])
            | (u64::from(self.helper_words[TO_HIGH_WORD]) << 32))
            .to_le_bytes();
        let release = digest_bytes(&self.helper_words, RELEASE_WORD_START);
        let context = digest_bytes(&self.helper_words, CONTEXT_WORD_START);
        let current_head = digest_bytes(&self.helper_words, CURRENT_HEAD_WORD_START);
        let current_lineage = digest_bytes(&self.helper_words, CURRENT_LINEAGE_WORD_START);
        let transition = digest_bytes(&self.helper_words, TRANSITION_WORD_START);
        let wallet = digest_bytes(&self.helper_words, WALLET_WORD_START);
        let policy = digest_bytes(&self.helper_words, POLICY_WORD_START);
        let device = digest_bytes(&self.helper_words, DEVICE_WORD_START);
        let current_guard = digest_bytes(&self.helper_words, CURRENT_GUARD_WORD_START);
        let next_guard = digest_bytes(&self.helper_words, NEXT_GUARD_WORD_START);
        let platform_key_digest = digest_bytes(&self.helper_words, PLATFORM_KEY_WORD_START);
        let platform_message = digest_bytes(&self.helper_words, PLATFORM_MESSAGE_WORD_START);
        let guard_use = digest_bytes(&self.helper_words, GUARD_USE_CLAIM_WORD_START);
        let platform_bind = digest_bytes(&self.helper_words, PLATFORM_BIND_CLAIM_WORD_START);
        let certificate = digest_bytes(&self.helper_words, ANDROID_CERTIFICATE_WORD_START);
        let tbs = digest_bytes(&self.helper_words, ANDROID_TBS_WORD_START);
        let issuer = digest_bytes(&self.helper_words, ANDROID_ISSUER_KEY_WORD_START);
        let attestation = digest_bytes(&self.helper_words, ANDROID_ATTESTATION_WORD_START);
        let android_claim = digest_bytes(&self.helper_words, ANDROID_CLAIM_WORD_START);

        let messages = [
            framed(
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
            framed(
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
            framed(
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
            self.platform_public_key_sec1.to_vec(),
            framed(
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
            framed(
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
            self.android_issuer_public_key_sec1.to_vec(),
            framed(
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
            framed(
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
        if messages.each_ref().map(Vec::len) != COMPACT_SHA_FIXED_MESSAGE_BYTES_V2 {
            return Err(CompactShaAbiErrorV2::InvalidMessageGeometry);
        }
        Ok(messages)
    }

    pub(super) fn expected_digests(&self) -> [[u8; 32]; 9] {
        [
            digest_bytes(&self.helper_words, CURRENT_GUARD_WORD_START),
            digest_bytes(&self.helper_words, NEXT_GUARD_WORD_START),
            digest_bytes(&self.helper_words, PLATFORM_MESSAGE_WORD_START),
            digest_bytes(&self.helper_words, PLATFORM_KEY_WORD_START),
            digest_bytes(&self.helper_words, GUARD_USE_CLAIM_WORD_START),
            digest_bytes(&self.helper_words, PLATFORM_BIND_CLAIM_WORD_START),
            digest_bytes(&self.helper_words, ANDROID_ISSUER_KEY_WORD_START),
            digest_bytes(&self.helper_words, ANDROID_CLAIM_WORD_START),
            digest_bytes(&self.helper_words, BUNDLE_WORD_START),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompactShaRawTbsContractV2 {
    exact_bytes: Vec<u8>,
}

impl CompactShaRawTbsContractV2 {
    pub(super) fn new(exact_bytes: Vec<u8>) -> Result<Self, CompactShaAbiErrorV2> {
        if exact_bytes.is_empty() {
            return Err(CompactShaAbiErrorV2::EmptyRawTbs);
        }
        if exact_bytes.len() > COMPACT_SHA_RAW_TBS_MAX_BYTES_V2 {
            return Err(CompactShaAbiErrorV2::RawTbsCapExceeded {
                actual: exact_bytes.len(),
                maximum: COMPACT_SHA_RAW_TBS_MAX_BYTES_V2,
            });
        }
        Ok(Self { exact_bytes })
    }

    pub(super) fn exact_bytes(&self) -> &[u8] {
        &self.exact_bytes
    }

    pub(super) const fn mode(&self) -> CompactShaPublicModeV2 {
        CompactShaPublicModeV2::RawTbs
    }

    /// Canonical byte length occupying the one raw-mode metadata word.
    pub(super) fn exact_length_word(&self) -> u32 {
        u32::try_from(self.exact_bytes.len()).expect("raw TBS cap is below u32::MAX")
    }

    /// Canonical little-endian payload words.  The unused suffix, including the high bytes of
    /// the final partial word, is exactly zero.  A future raw role may combine the continuously
    /// repacked 184 helper words, one 17-word issuer key, this length word, and this payload, but
    /// that aggregate ABI is intentionally not implemented here.
    pub(super) fn canonical_payload_words(&self) -> [u32; COMPACT_SHA_RAW_TBS_WORDS_V2] {
        let mut words = [0_u32; COMPACT_SHA_RAW_TBS_WORDS_V2];
        for (word, bytes) in words.iter_mut().zip(self.exact_bytes.chunks(4)) {
            let mut encoded = [0_u8; 4];
            encoded[..bytes.len()].copy_from_slice(bytes);
            *word = u32::from_le_bytes(encoded);
        }
        words
    }

    /// Strictly decode one canonical field-neutral payload and reject every nonzero suffix byte.
    pub(super) fn from_canonical_payload_words(
        exact_length_word: u32,
        words: [u32; COMPACT_SHA_RAW_TBS_WORDS_V2],
    ) -> Result<Self, CompactShaAbiErrorV2> {
        let exact_length = usize::try_from(exact_length_word)
            .map_err(|_| CompactShaAbiErrorV2::ArithmeticOverflow)?;
        if exact_length == 0 {
            return Err(CompactShaAbiErrorV2::EmptyRawTbs);
        }
        if exact_length > COMPACT_SHA_RAW_TBS_MAX_BYTES_V2 {
            return Err(CompactShaAbiErrorV2::RawTbsCapExceeded {
                actual: exact_length,
                maximum: COMPACT_SHA_RAW_TBS_MAX_BYTES_V2,
            });
        }
        let mut exact_bytes = Vec::with_capacity(exact_length);
        for (index, byte) in words.into_iter().flat_map(u32::to_le_bytes).enumerate() {
            if index < exact_length {
                exact_bytes.push(byte);
            } else if byte != 0 {
                return Err(CompactShaAbiErrorV2::NonCanonicalRawTbsPayload);
            }
        }
        Self::new(exact_bytes)
    }

    pub(super) const fn activation_eligible(&self) -> bool {
        false
    }
}

fn digest_bytes(words: &[u32; COMPACT_SHA_HELPER_WORDS_V2], offset: usize) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (destination, word) in digest.chunks_exact_mut(4).zip(&words[offset..offset + 8]) {
        destination.copy_from_slice(&word.to_le_bytes());
    }
    digest
}

fn write_sec1_words(destination: &mut [u32], key: &[u8; 65]) {
    debug_assert_eq!(destination.len(), COMPACT_SHA_KEY_WORDS_V2);
    for (index, word) in destination.iter_mut().enumerate() {
        let start = index * 4;
        let mut bytes = [0_u8; 4];
        let end = (start + 4).min(key.len());
        if start < end {
            bytes[..end - start].copy_from_slice(&key[start..end]);
        }
        *word = u32::from_le_bytes(bytes);
    }
}

fn pack_words<F: PrimeField>(words: &[u32]) -> F {
    debug_assert!(words.len() <= COMPACT_SHA_WORDS_PER_INSTANCE_V2);
    assert!(
        F::NUM_BITS > 224,
        "compact-SHA V2 seven-word packing requires at least 225 field bits"
    );
    let radix = F::from(1_u64 << 32);
    words.iter().rev().fold(F::ZERO, |accumulator, word| {
        accumulator * radix + F::from(u64::from(*word))
    })
}

fn framed(domain: &[u8], fields: &[&[u8]]) -> Result<Vec<u8>, CompactShaAbiErrorV2> {
    let length = fields.iter().try_fold(
        8_usize
            .checked_add(domain.len())
            .ok_or(CompactShaAbiErrorV2::ArithmeticOverflow)?,
        |length, field| {
            length
                .checked_add(8)
                .and_then(|length| length.checked_add(field.len()))
                .ok_or(CompactShaAbiErrorV2::ArithmeticOverflow)
        },
    )?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| CompactShaAbiErrorV2::ArithmeticOverflow)?;
    output.extend_from_slice(
        &u64::try_from(domain.len())
            .map_err(|_| CompactShaAbiErrorV2::ArithmeticOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(domain);
    for field in fields {
        output.extend_from_slice(
            &u64::try_from(field.len())
                .map_err(|_| CompactShaAbiErrorV2::ArithmeticOverflow)?
                .to_le_bytes(),
        );
        output.extend_from_slice(field);
    }
    debug_assert_eq!(output.len(), length);
    Ok(output)
}
