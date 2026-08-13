//! Proof-carrying generation and roster-independent evaluation of compact collective keys.
//!
//! Each online key contains exactly two polynomials per balanced gadget digit.
//! Relinearization digits encrypt `g^d S^2`; Galois digits encrypt
//! `g^d sigma_k(S)`, where `S` is the exact eight-party sum secret.  Generation
//! retains the full authenticated source topology and compacts every digit with
//! the native full-roster CKS protocol.  Online evaluation therefore performs
//! exactly two ring multiplications per digit, independent of roster size.
#[cfg(test)]
use super::gadget_decompose;
use super::{
    ArtifactAuthentication, BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1,
    RnsPolynomial, WideUint, ZkAmsMkheCksContributionWireV1, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    ZkAmsMkheWireBindingV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    checked_coefficient_work, checked_ring_multiplication_work, checked_rns_polynomial_bytes,
    cks::{
        ZkAmsMkheCksProofV1, derive_cks_sparse_challenge, streaming_cks_auth_domain_v1,
        streaming_cks_binding_v1, streaming_cks_challenge_seed_v1,
        streaming_cks_key_context_digest_v1, streaming_cks_record_digest_v1,
        streaming_cks_source_digest_v1, streaming_cks_wire_statement_digest_v1,
        zk_ams_mkhe_cks_resource_evidence_v1,
    },
    collective_keys::{
        ZkAmsMkheCollectiveEvaluatedKeyEntryV1, ZkAmsMkheCollectiveEvaluatedKeyManifestV1,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1, ZkAmsMkheEvaluatedKeySorafsPointerV1,
    },
    derive_uniform_rns_from_context,
    manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
    mod_add, mod_mul, mod_sub, modulus_product,
    packing::{
        ZK_AMS_T256_GALOIS_KEY_COUNT_V1, validate_zk_ams_t256_galois_key_schedule_v1,
        zk_ams_t256_galois_key_schedule_v1,
    },
    ring_multiplication_work, signed_mod,
    wire::ZkAmsMkheGovernedRosterWireV1,
};
#[cfg(test)]
use super::{
    MaskedRelaxedRandomSourceV1, PartySet, SecretPolynomial,
    active::{
        ZkAmsMkheActiveCollectivePublicKeyStatementV1, ZkAmsMkheActiveGaloisSourceStatementV1,
        ZkAmsMkheActiveGaloisSourceWitnessV1, ZkAmsMkheActivePartySecretV1,
        ZkAmsMkheActiveRkgProofV1, ZkAmsMkheActiveRkgRoundOneStatementV1,
        ZkAmsMkheActiveRkgRoundOneWitnessV1, ZkAmsMkheActiveRkgRoundTwoStatementV1,
        ZkAmsMkheActiveRkgRoundTwoWitnessV1, prove_zk_ams_mkhe_active_galois_source_v1,
        prove_zk_ams_mkhe_active_rkg_round_one_v1, prove_zk_ams_mkhe_active_rkg_round_two_v1,
        verify_zk_ams_mkhe_active_galois_source_v1, verify_zk_ams_mkhe_active_rkg_round_one_v1,
        verify_zk_ams_mkhe_active_rkg_round_two_v1,
    },
    cks::{
        ZkAmsMkheAuthenticatedCksContributionV1, ZkAmsMkheCksSourceCiphertextV1,
        ZkAmsMkheCksStatementV1, combine_zk_ams_mkhe_cks_v1, prove_zk_ams_mkhe_cks_contribution_v1,
    },
    collective::{
        ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheCollectiveLevelOneV1,
        ZkAmsMkheCollectivePartyStateV1, ZkAmsMkheCollectivePublicKeyShareV1,
        ZkAmsMkheCollectivePublicKeyV1, aggregate_zk_ams_mkhe_collective_public_key_v1,
        validate_compact_for_key,
    },
    derive_rkg_common_a,
    wire::ZkAmsMkheRnsPolynomialWireV1,
};
use crate::vega::sponge::{Keccak256, Shake256Reader, keccak256};
#[path = "collective_eval_keys/cks_stream.rs"]
mod cks_stream;
#[path = "collective_eval_keys/evidence_set.rs"]
mod evidence_set;
#[path = "collective_eval_keys/source_stream.rs"]
mod source_stream;
#[path = "collective_eval_keys/streaming_automorphism.rs"]
mod streaming_automorphism;
pub use evidence_set::{
    ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1, verify_zk_ams_mkhe_evaluated_key_evidence_set_v1,
};
pub use source_stream::{
    ZkAmsMkheTrustedSourceContextV1, ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1,
};
pub use streaming_automorphism::{
    ZkAmsMkheStreamingCollectiveAutomorphismAccountingV1,
    automorphism_switch_zk_ams_mkhe_collective_streaming_v1,
    zk_ams_mkhe_streaming_collective_automorphism_accounting_v1,
};
const EVALUATED_KEY_TARGET_A_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-target-a";
const EVALUATED_KEY_TARGET_A_CONTEXT_BYTES_V1: usize = 176;
const EVALUATED_KEY_EVIDENCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-evidence";
const EVALUATED_KEY_LINEAGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-lineage";
const EVALUATED_KEY_RUNTIME_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-runtime";
const EVALUATED_KEY_PROVIDER_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-provider";
const EVALUATED_KEY_LIMB_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-limb";
const EVALUATED_KEY_LIMB_INDEX_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-evaluated-key-limb-index";
const SOURCE_EVIDENCE_RECORD_TAG_V1: [u8; 4] = *b"ZASE";
const CKS_EVIDENCE_RECORD_TAG_V1: [u8; 4] = *b"ZACE";
const CKS_VALIDATED_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-cks-validated-receipt";
const CKS_TRUSTED_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-cks-trusted-context";
const RNS_NATIVE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-polynomial-digest";
const RNS_WIRE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-polynomial";
const EVIDENCE_RECORD_DIGEST_BYTES_V1: usize = 32;
const SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1: usize =
    4 + 1 + 1 + 8 + 1 + 4 + 1 + 32 + 32 + 32 + 8 + 32 + 32;
const CKS_EVIDENCE_COMMON_BODY_BYTES_V1: usize = 4 + 1 + 8 + 1 + 1 + 32;
const SEEKABLE_EVALUATED_KEY_TAG_V1: [u8; 4] = *b"ZARK";
const SEEKABLE_EVALUATED_KEY_BINDING_BYTES_V1: usize = 4 + 1 + 32 + 32 + 8 + 32 + 4 + 1;
const SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1: usize =
    SEEKABLE_EVALUATED_KEY_BINDING_BYTES_V1 + 32 + 32 + 1;
const SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1: usize = 1 + 4;
const SEEKABLE_EVALUATED_KEY_READ_BYTES_V1: usize = 8 * 1024;
/// Frozen signed-digit batch used by the release decomposition schedule.
const HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1: usize = 5;
/// Largest callback chunk used by the canonical evidence stream.
///
/// Chunk boundaries are transport metadata, not part of the canonical record,
/// but are deterministic and gap-free so a sink can reject omission, reorder,
/// duplication, or cross-record splicing before committing durable bytes.
pub const ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1: usize = 64 * 1024;
/// One of the two independently hashed evidence sets backing an evaluated key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheCollectiveEvidenceSetKindV1 {
    /// Pairwise RKG or automorphism-linked source proofs.
    Source = 1,
    /// Full-roster CKS compaction proofs.
    Cks = 2,
}
/// Exact canonical record family inside an evaluated-key evidence set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheCollectiveEvidenceRecordKindV1 {
    /// First authenticated RKG round for one pair/digit/party coordinate.
    RkgRoundOne = 1,
    /// Second authenticated RKG round for one pair/digit/party coordinate.
    RkgRoundTwo = 2,
    /// One automorphism-linked source encryption.
    GaloisSource = 3,
    /// One complete eight-party CKS digit.
    CksDigit = 4,
}
impl ZkAmsMkheCollectiveEvidenceRecordKindV1 {
    fn decode(value: u8) -> Result<Self, ZkAmsMkheErrorV1> {
        match value {
            1 => Ok(Self::RkgRoundOne),
            2 => Ok(Self::RkgRoundTwo),
            3 => Ok(Self::GaloisSource),
            4 => Ok(Self::CksDigit),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}
fn seekable_evaluated_key_layout(
    profile: &BgvProfile,
) -> Result<SeekableEvaluatedKeyLayoutV1, ZkAmsMkheErrorV1> {
    profile.validate()?;
    let residue_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let native_polynomial_bytes = residue_count
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let digit_record_bytes = u64::try_from(SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .checked_add(native_polynomial_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let payload_bytes = digit_record_bytes
        .checked_mul(
            u64::try_from(profile.gadget_digits)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| {
            value.checked_add(u64::try_from(SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1).ok()?)
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if payload_bytes
        != u64::try_from(
            SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1
                .checked_add(
                    profile
                        .gadget_digits
                        .checked_mul(
                            checked_rns_polynomial_bytes(profile)?
                                .checked_add(1)
                                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                        )
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                )
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(SeekableEvaluatedKeyLayoutV1 {
        residue_count,
        native_polynomial_bytes,
        digit_record_bytes,
        payload_bytes,
    })
}
/// Conservative coefficient-limb passes for one hoisted decomposition.
///
/// The CRT reconstruction and balanced-radix walk are performed once per
/// bounded five-digit batch instead of once per digit. Each digit is then
/// materialized into one RNS polynomial immediately before its evaluated-key
/// record is streamed. This keeps both the 48.5 GB artifact and the complete
/// 38-digit decomposition out of memory.
fn hoisted_hybrid_decomposition_passes(profile: &BgvProfile) -> Result<usize, ZkAmsMkheErrorV1> {
    let per_batch = profile
        .moduli
        .len()
        .checked_add(2)
        .and_then(|value| value.checked_add(profile.gadget_digits))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let batch_count = profile
        .gadget_digits
        .checked_add(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1 - 1)
        .and_then(|value| value.checked_div(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    per_batch
        .checked_mul(batch_count)
        .and_then(|value| value.checked_add(profile.gadget_digits))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
pub(super) fn seekable_evaluated_key_accounting(
    profile: &BgvProfile,
) -> Result<ZkAmsMkheSeekableEvaluatedKeyAccountingV1, ZkAmsMkheErrorV1> {
    let layout = seekable_evaluated_key_layout(profile)?;
    let native_polynomial_allocation_bytes = layout.native_polynomial_bytes;
    let native_limb_allocation_bytes = profile
        .ring_degree
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let digit_validation_metadata_bytes = profile
        .gadget_digits
        .checked_mul(core::mem::size_of::<SeekableEvaluatedKeyDigitV1>())
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let limb_validation_metadata_bytes = profile
        .gadget_digits
        .checked_mul(profile.moduli.len())
        .and_then(|value| value.checked_mul(core::mem::size_of::<SeekableEvaluatedKeyLimbV1>()))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let validation_metadata_bytes = digit_validation_metadata_bytes
        .checked_add(limb_validation_metadata_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let output_accumulator_bytes = native_polynomial_allocation_bytes
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let ntt_limb_scratch_bytes = native_limb_allocation_bytes
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let signed_decomposition_scratch_bytes = profile
        .ring_degree
        .checked_mul(
            profile
                .gadget_digits
                .min(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1),
        )
        .and_then(|values| values.checked_mul(core::mem::size_of::<i64>()))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let crt_residue_scratch_bytes = profile
        .moduli
        .len()
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let provider_read_buffer_bytes = u64::try_from(SEEKABLE_EVALUATED_KEY_READ_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let provider_hash_state_bytes =
        u64::try_from(core::mem::size_of::<norito::streaming::Blake3Hasher>())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // Target-layout owners and controls live across the arithmetic peak: one
    // hoisted batch, two output owners, two limb owners, two borrowed limb
    // views, and the optional automorphism selector.
    let explicit_multiplication_state_bytes = u64::try_from(
        core::mem::size_of::<HoistedHybridDigitBatchV1<'static>>()
            .checked_add(
                2_usize
                    .checked_mul(core::mem::size_of::<RnsPolynomial>())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .and_then(|bytes| bytes.checked_add(2 * core::mem::size_of::<Vec<u64>>()))
            .and_then(|bytes| bytes.checked_add(2 * core::mem::size_of::<&mut [u64]>()))
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<Option<usize>>()))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let decomposition_phase_bytes = output_accumulator_bytes
        .checked_add(signed_decomposition_scratch_bytes)
        .and_then(|bytes| bytes.checked_add(ntt_limb_scratch_bytes))
        .and_then(|bytes| bytes.checked_add(crt_residue_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let provider_read_phase_bytes = output_accumulator_bytes
        .checked_add(signed_decomposition_scratch_bytes)
        .and_then(|bytes| bytes.checked_add(ntt_limb_scratch_bytes))
        .and_then(|bytes| bytes.checked_add(provider_read_buffer_bytes))
        .and_then(|bytes| bytes.checked_add(provider_hash_state_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let peak_heap_allocation_bytes = output_accumulator_bytes
        .checked_add(signed_decomposition_scratch_bytes)
        .and_then(|bytes| bytes.checked_add(ntt_limb_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let multiplication_phase_bytes = provider_read_phase_bytes
        .checked_add(explicit_multiplication_state_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let peak_managed_workspace_bytes = decomposition_phase_bytes
        .max(provider_read_phase_bytes)
        .max(multiplication_phase_bytes);
    let per_key_switch_read_bytes = layout
        .native_polynomial_bytes
        .checked_mul(
            u64::try_from(profile.gadget_digits)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let balanced_decomposition_work_units = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .and_then(|value| value.checked_mul(hoisted_hybrid_decomposition_passes(profile).ok()?))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let ring_multiplication_work_units = ring_multiplication_work(profile)?
        .checked_mul(
            u64::try_from(
                profile
                    .gadget_digits
                    .checked_mul(2)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let accumulator_addition_work_units = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .and_then(|value| value.checked_mul(profile.gadget_digits))
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let total_key_switch_work_units = balanced_decomposition_work_units
        .checked_add(ring_multiplication_work_units)
        .and_then(|value| value.checked_add(accumulator_addition_work_units))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if total_key_switch_work_units > profile.max_work_units
        || peak_managed_workspace_bytes
            > u64::try_from(profile.max_workspace_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok(ZkAmsMkheSeekableEvaluatedKeyAccountingV1 {
        canonical_payload_bytes: layout.payload_bytes,
        canonical_digit_record_bytes: layout.digit_record_bytes,
        incremental_validation_read_bytes: layout.payload_bytes,
        per_key_switch_read_bytes,
        native_polynomial_allocation_bytes,
        native_limb_allocation_bytes,
        output_accumulator_bytes,
        signed_decomposition_scratch_bytes,
        crt_residue_scratch_bytes,
        ntt_limb_scratch_bytes,
        provider_read_buffer_bytes,
        provider_hash_state_bytes,
        decomposition_phase_bytes,
        provider_read_phase_bytes,
        peak_heap_allocation_bytes,
        multiplication_phase_bytes,
        peak_managed_workspace_bytes,
        validation_metadata_bytes,
        balanced_decomposition_work_units,
        ring_multiplication_work_units,
        accumulator_addition_work_units,
        total_key_switch_work_units,
    })
}
fn seekable_provider_state<P>(
    provider: &mut P,
    expected_pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
) -> Result<SeekableProviderStateV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    let state = SeekableProviderStateV1 {
        provider_identity: provider.provider_identity(),
        snapshot_identity: provider.snapshot_identity()?,
        pointer: provider.sorafs_pointer(),
        payload_len: provider.payload_len()?,
    };
    if state.provider_identity == [0; 32]
        || state.snapshot_identity == [0; 32]
        || state.pointer != expected_pointer
        || state.payload_len != expected_pointer.payload_bytes()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(state)
}
fn ensure_seekable_provider_state<P>(
    provider: &mut P,
    expected: SeekableProviderStateV1,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    if seekable_provider_state(provider, expected.pointer)? != expected {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
fn seekable_provider_seek_exact<P>(
    provider: &mut P,
    expected: SeekableProviderStateV1,
    absolute_offset: u64,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    if absolute_offset > expected.payload_len {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    ensure_seekable_provider_state(provider, expected)?;
    provider.seek(absolute_offset)?;
    ensure_seekable_provider_state(provider, expected)
}
fn seekable_provider_read_exact<P>(
    provider: &mut P,
    expected: SeekableProviderStateV1,
    destination: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    if destination.is_empty() || destination.len() > SEEKABLE_EVALUATED_KEY_READ_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    ensure_seekable_provider_state(provider, expected)?;
    let read = provider.read(destination)?;
    if read != destination.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    ensure_seekable_provider_state(provider, expected)
}
fn seekable_header_array<const N: usize>(
    header: &[u8],
    offset: usize,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    header
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        )
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}
fn parse_seekable_evaluated_key_header(
    profile: &BgvProfile,
    expected: SeekableEvaluatedKeyExpectedV1,
    header: &[u8; SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1],
) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
    if header[..4] != SEEKABLE_EVALUATED_KEY_TAG_V1
        || header[4] != MKHE_VERSION_V1
        || seekable_header_array::<32>(header, 5)? != expected.profile_digest
        || seekable_header_array::<32>(header, 37)? != expected.roster_digest
        || u64::from_be_bytes(seekable_header_array(header, 69)?) != expected.epoch
        || seekable_header_array::<32>(header, 77)? != expected.transcript_digest
        || u32::from_be_bytes(seekable_header_array(header, 109)?)
            != u32::from(expected.entry.ordinal())
        || header[113] != 0
        || usize::from(header[178]) != profile.gadget_digits
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let a_master_seed = seekable_header_array(header, 114)?;
    let contribution_proof_digest = seekable_header_array(header, 146)?;
    if a_master_seed == [0; 32]
        || contribution_proof_digest != expected.contribution_proof_digest
        || contribution_proof_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok((a_master_seed, contribution_proof_digest))
}
fn validate_seekable_expected_layout(
    profile: &BgvProfile,
    expected: SeekableEvaluatedKeyExpectedV1,
) -> Result<SeekableEvaluatedKeyLayoutV1, ZkAmsMkheErrorV1> {
    let layout = seekable_evaluated_key_layout(profile)?;
    let ordinal = usize::from(expected.entry.ordinal());
    let expected_offset = layout
        .payload_bytes
        .checked_mul(u64::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let expected_total = layout
        .payload_bytes
        .checked_mul(
            u64::try_from(expected.artifact_key_count)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if expected.artifact_key_count == 0
        || ordinal >= expected.artifact_key_count
        || expected.entry.payload_offset() != expected_offset
        || expected.entry.payload_bytes() != layout.payload_bytes
        || expected.pointer.payload_bytes() != expected_total
        || expected
            .entry
            .payload_offset()
            .checked_add(expected.entry.payload_bytes())
            .is_none_or(|end| end > expected.pointer.payload_bytes())
        || expected.profile_digest == [0; 32]
        || expected.roster_digest == [0; 32]
        || expected.epoch == 0
        || expected.transcript_digest == [0; 32]
        || expected.collective_key_digest == [0; 32]
        || expected.contribution_proof_digest == [0; 32]
        || expected.cks_compact_output_set_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    match expected.entry.purpose() {
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization => {
            if ordinal != 0 || expected.entry.galois_exponent() != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois => {
            if ordinal == 0
                || expected.entry.galois_exponent() == 0
                || expected.entry.galois_exponent().is_multiple_of(2)
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
    }
    Ok(layout)
}
fn seekable_evaluated_key_limb_hasher(
    digit_index: usize,
    limb_index: usize,
    modulus: u64,
    absolute_offset: u64,
    canonical_bytes: u64,
) -> Result<norito::streaming::Blake3Hasher, ZkAmsMkheErrorV1> {
    let mut hasher = norito::streaming::Blake3Hasher::new();
    hasher.update(EVALUATED_KEY_LIMB_DOMAIN_V1);
    hasher.update(&[MKHE_VERSION_V1]);
    hasher.update(
        &u16::try_from(digit_index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    hasher.update(
        &u16::try_from(limb_index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    hasher.update(&modulus.to_be_bytes());
    hasher.update(&absolute_offset.to_be_bytes());
    hasher.update(&canonical_bytes.to_be_bytes());
    Ok(hasher)
}
fn seekable_evaluated_key_limb_index_digest(
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    limbs: &[SeekableEvaluatedKeyLimbV1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hasher = norito::streaming::Blake3Hasher::new();
    hasher.update(EVALUATED_KEY_LIMB_INDEX_DOMAIN_V1);
    hasher.update(&[MKHE_VERSION_V1, entry.purpose() as u8, entry.ordinal()]);
    hasher.update(&entry.galois_exponent().to_be_bytes());
    hasher.update(&entry.payload_offset().to_be_bytes());
    hasher.update(&entry.payload_bytes().to_be_bytes());
    hasher.update(&entry.payload_blake3());
    hasher.update(
        &u32::try_from(limbs.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for (index, limb) in limbs.iter().enumerate() {
        hasher.update(
            &u32::try_from(index)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        hasher.update(&limb.absolute_offset.to_be_bytes());
        hasher.update(&limb.canonical_bytes.to_be_bytes());
        hasher.update(&limb.blake3);
    }
    Ok(hasher.finalize())
}
fn validate_seekable_evaluated_key<P>(
    profile: &BgvProfile,
    expected: SeekableEvaluatedKeyExpectedV1,
    provider: &mut P,
) -> Result<SeekableEvaluatedKeyValidationV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    let layout = validate_seekable_expected_layout(profile, expected)?;
    let before = seekable_provider_state(provider, expected.pointer)?;
    seekable_provider_seek_exact(provider, before, expected.entry.payload_offset())?;
    let mut payload_hasher = norito::streaming::Blake3Hasher::new();
    let mut header = [0_u8; SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1];
    seekable_provider_read_exact(provider, before, &mut header)?;
    payload_hasher.update(&header);
    let (a_master_seed, contribution_proof_digest) =
        parse_seekable_evaluated_key_header(profile, expected, &header)?;
    let mut digits = Vec::new();
    digits
        .try_reserve_exact(profile.gadget_digits)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if digits.capacity() != profile.gadget_digits {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let limb_count = profile
        .gadget_digits
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut limbs = Vec::new();
    limbs
        .try_reserve_exact(limb_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if limbs.capacity() != limb_count {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let limb_bytes = u64::try_from(
        profile
            .ring_degree
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut absolute_offset = expected
        .entry
        .payload_offset()
        .checked_add(
            u64::try_from(SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
    let mut compact_output_digest = evidence_set::CksCompactOutputSetDigestV1::new(
        expected.entry.purpose(),
        expected.entry.ordinal(),
        expected.entry.galois_exponent(),
        expected.collective_key_digest,
    )?;
    for digit_index in 0..profile.gadget_digits {
        let expected_digit_offset = expected
            .entry
            .payload_offset()
            .checked_add(
                u64::try_from(SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .and_then(|value| {
                layout
                    .digit_record_bytes
                    .checked_mul(u64::try_from(digit_index).ok()?)
                    .and_then(|digit| value.checked_add(digit))
            })
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if absolute_offset != expected_digit_offset {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut digit_hasher = norito::streaming::Blake3Hasher::new();
        let mut native_digest =
            new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, layout.residue_count)?;
        let mut prefix = [0_u8; SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1];
        seekable_provider_read_exact(provider, before, &mut prefix)?;
        payload_hasher.update(&prefix);
        digit_hasher.update(&prefix);
        if usize::from(prefix[0]) != digit_index
            || usize::try_from(u32::from_be_bytes(
                prefix[1..5]
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            ))
            .ok()
                != Some(layout.residue_count)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        absolute_offset = absolute_offset
            .checked_add(
                u64::try_from(SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (limb_index, &modulus) in profile.moduli.iter().enumerate() {
            let expected_limb_offset = expected_digit_offset
                .checked_add(
                    u64::try_from(SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1)
                        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                )
                .and_then(|value| {
                    limb_bytes
                        .checked_mul(u64::try_from(limb_index).ok()?)
                        .and_then(|limb| value.checked_add(limb))
                })
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if absolute_offset != expected_limb_offset {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            let mut limb_hasher = seekable_evaluated_key_limb_hasher(
                digit_index,
                limb_index,
                modulus,
                absolute_offset,
                limb_bytes,
            )?;
            let mut residues = 0_usize;
            let mut remaining = usize::try_from(limb_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            while remaining != 0 {
                let take = remaining.min(buffer.len());
                seekable_provider_read_exact(provider, before, &mut buffer[..take])?;
                payload_hasher.update(&buffer[..take]);
                digit_hasher.update(&buffer[..take]);
                limb_hasher.update(&buffer[..take]);
                native_digest.update(&buffer[..take]);
                for encoded in buffer[..take].chunks_exact(core::mem::size_of::<u64>()) {
                    let residue = u64::from_be_bytes(
                        encoded
                            .try_into()
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                    );
                    if residue >= modulus {
                        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                    }
                    residues = residues
                        .checked_add(1)
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                }
                remaining -= take;
            }
            if residues != profile.ring_degree {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            limbs.push(SeekableEvaluatedKeyLimbV1 {
                absolute_offset,
                canonical_bytes: limb_bytes,
                blake3: limb_hasher.finalize(),
            });
            absolute_offset = absolute_offset
                .checked_add(limb_bytes)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        if absolute_offset
            != expected_digit_offset
                .checked_add(layout.digit_record_bytes)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        digits.push(SeekableEvaluatedKeyDigitV1 {
            absolute_offset: expected_digit_offset,
            canonical_bytes: layout.digit_record_bytes,
            blake3: digit_hasher.finalize(),
        });
        compact_output_digest.absorb(
            u32::try_from(digit_index).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            native_digest.finalize(),
        )?;
    }
    let compact_output_digest = compact_output_digest.finish(
        u32::try_from(profile.gadget_digits)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )?;
    if absolute_offset
        != expected
            .entry
            .payload_offset()
            .checked_add(expected.entry.payload_bytes())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        || payload_hasher.finalize() != expected.entry.payload_blake3()
        || compact_output_digest != expected.cks_compact_output_set_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let after = seekable_provider_state(provider, expected.pointer)?;
    if after != before {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let limb_index_digest = seekable_evaluated_key_limb_index_digest(expected.entry, &limbs)?;
    if limb_index_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(SeekableEvaluatedKeyValidationV1 {
        state: after,
        a_master_seed,
        contribution_proof_digest,
        digits,
        limbs,
        limb_index_digest,
    })
}
fn seekable_provider_binding_digest(
    runtime_context_digest: [u8; 32],
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    state: SeekableProviderStateV1,
    a_master_seed: [u8; 32],
    contribution_proof_digest: [u8; 32],
    evidence_set_capability_seal: [u8; 32],
    cks_compact_output_set_digest: [u8; 32],
    limb_index_digest: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(EVALUATED_KEY_PROVIDER_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1, entry.purpose() as u8, entry.ordinal()]);
    hash.update(&entry.galois_exponent().to_be_bytes());
    hash.update(&entry.payload_offset().to_be_bytes());
    hash.update(&entry.payload_bytes().to_be_bytes());
    hash.update(&entry.payload_blake3());
    hash.update(&entry.source_proof_set_digest());
    hash.update(&entry.cks_proof_set_digest());
    hash.update(&runtime_context_digest);
    hash.update(&state.provider_identity);
    hash.update(&state.snapshot_identity);
    hash.update(&state.pointer.payload_blake3());
    hash.update(&state.pointer.payload_bytes().to_be_bytes());
    hash.update(&state.pointer.chunk_root());
    hash.update(&state.pointer.sorafs_manifest_blake3());
    hash.update(&state.pointer.chunker_profile_digest());
    hash.update(&a_master_seed);
    hash.update(&contribution_proof_digest);
    hash.update(&evidence_set_capability_seal);
    hash.update(&cks_compact_output_set_digest);
    hash.update(&limb_index_digest);
    hash.finalize()
}
fn validated_key_provider_state(
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
) -> SeekableProviderStateV1 {
    SeekableProviderStateV1 {
        provider_identity: key.provider_identity,
        snapshot_identity: key.snapshot_identity,
        pointer: key.sorafs_pointer,
        payload_len: key.sorafs_pointer.payload_bytes(),
    }
}
fn validate_bound_seekable_provider_state<P>(
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    provider: &mut P,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    ensure_seekable_provider_state(provider, validated_key_provider_state(key))
}
#[cfg(test)]
fn read_seekable_evaluated_key_digit<P>(
    profile: &BgvProfile,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    provider: &mut P,
    digit_index: usize,
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
{
    let layout = seekable_evaluated_key_layout(profile)?;
    let metadata = *key
        .digits
        .get(digit_index)
        .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
    let expected_offset = key
        .entry
        .payload_offset()
        .checked_add(
            u64::try_from(SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .and_then(|value| {
            layout
                .digit_record_bytes
                .checked_mul(u64::try_from(digit_index).ok()?)
                .and_then(|digit| value.checked_add(digit))
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if metadata.absolute_offset != expected_offset
        || metadata.canonical_bytes != layout.digit_record_bytes
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let expected_state = validated_key_provider_state(key);
    seekable_provider_seek_exact(provider, expected_state, metadata.absolute_offset)?;
    let mut hasher = norito::streaming::Blake3Hasher::new();
    let mut prefix = [0_u8; SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1];
    seekable_provider_read_exact(provider, expected_state, &mut prefix)?;
    hasher.update(&prefix);
    if usize::from(prefix[0]) != digit_index
        || usize::try_from(u32::from_be_bytes(
            prefix[1..5]
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        ))
        .ok()
            != Some(layout.residue_count)
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut residues = Vec::new();
    residues
        .try_reserve_exact(layout.residue_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if residues.capacity() != layout.residue_count {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
    let mut remaining = usize::try_from(layout.native_polynomial_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    while remaining != 0 {
        let take = remaining.min(buffer.len());
        seekable_provider_read_exact(provider, expected_state, &mut buffer[..take])?;
        hasher.update(&buffer[..take]);
        for encoded in buffer[..take].chunks_exact(8) {
            let residue = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            let limb = residues.len() / profile.ring_degree;
            if limb >= profile.moduli.len() || residue >= profile.moduli[limb] {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            residues.push(residue);
        }
        remaining -= take;
    }
    if residues.len() != layout.residue_count || hasher.finalize() != metadata.blake3 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    RnsPolynomial::from_flat(profile, residues)
}
/// Context opened once for one independently hashed canonical evidence set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
    kind: ZkAmsMkheCollectiveEvidenceSetKindV1,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    galois_exponent: u32,
    collective_key_digest: [u8; 32],
}
impl ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
    /// Evidence family.
    #[must_use]
    pub const fn kind(self) -> ZkAmsMkheCollectiveEvidenceSetKindV1 {
        self.kind
    }
    /// Evaluated-key purpose.
    #[must_use]
    pub const fn purpose(self) -> ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
        self.purpose
    }
    /// Exact evaluated-key ordinal.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }
    /// Frozen Galois exponent, or zero for relinearization.
    #[must_use]
    pub const fn galois_exponent(self) -> u32 {
        self.galois_exponent
    }
    /// Verified aggregate CPK identity.
    #[must_use]
    pub const fn collective_key_digest(self) -> [u8; 32] {
        self.collective_key_digest
    }
}
/// Exact identity and preflighted length announced before a record stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
    set: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
    record_index: u32,
    canonical_bytes: u64,
}
impl ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
    /// Parent set identity.
    #[must_use]
    pub const fn set(self) -> ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        self.set
    }
    /// Exact record family.
    #[must_use]
    pub const fn kind(self) -> ZkAmsMkheCollectiveEvidenceRecordKindV1 {
        self.kind
    }
    /// Gap-free canonical record position.
    #[must_use]
    pub const fn record_index(self) -> u32 {
        self.record_index
    }
    /// Exact self-delimiting `ZASE` or `ZACE` bytes, including digest footer.
    #[must_use]
    pub const fn canonical_bytes(self) -> u64 {
        self.canonical_bytes
    }
}
/// Record commitment announced only after every bounded chunk was accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceRecordFooterV1 {
    header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    chunk_count: u32,
    canonical_digest: [u8; 32],
}
impl ZkAmsMkheCollectiveEvidenceRecordFooterV1 {
    /// Exact opening header.
    #[must_use]
    pub const fn header(self) -> ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
        self.header
    }
    /// Exact gap-free callback chunk count.
    #[must_use]
    pub const fn chunk_count(self) -> u32 {
        self.chunk_count
    }
    /// Keccak-256 of every record byte preceding the final digest footer.
    #[must_use]
    pub const fn canonical_digest(self) -> [u8; 32] {
        self.canonical_digest
    }
}
/// Final set commitment after its exact gap-free record count was hashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvidenceSetFooterV1 {
    header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    record_count: u32,
    canonical_digest: [u8; 32],
}
impl ZkAmsMkheCollectiveEvidenceSetFooterV1 {
    /// Exact opening header.
    #[must_use]
    pub const fn header(self) -> ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        self.header
    }
    /// Exact gap-free record count.
    #[must_use]
    pub const fn record_count(self) -> u32 {
        self.record_count
    }
    /// Canonical set digest committed by the generated key and manifest entry.
    #[must_use]
    pub const fn canonical_digest(self) -> [u8; 32] {
        self.canonical_digest
    }
}
/// Exact region reserved for one transactional `ZARK` publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1 {
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    galois_exponent: u32,
    payload_offset: u64,
    payload_bytes: u64,
    artifact_bytes: u64,
}
impl ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1 {
    /// Evaluated-key purpose bound to this transaction.
    #[must_use]
    pub const fn purpose(self) -> ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
        self.purpose
    }
    /// Exact canonical key ordinal.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }
    /// Frozen odd Galois exponent, or zero for relinearization.
    #[must_use]
    pub const fn galois_exponent(self) -> u32 {
        self.galois_exponent
    }
    /// Exact absolute entry offset in the complete artifact.
    #[must_use]
    pub const fn payload_offset(self) -> u64 {
        self.payload_offset
    }
    /// Exact canonical bytes in this entry.
    #[must_use]
    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }
    /// Exact bytes in the complete 32-entry release artifact.
    #[must_use]
    pub const fn artifact_bytes(self) -> u64 {
        self.artifact_bytes
    }
}
/// Authenticated commit request for one completely reread `ZARK` entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveEvaluatedKeyPublicationFooterV1 {
    header: ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1,
    payload_blake3: [u8; 32],
    source_proof_set_digest: [u8; 32],
    cks_proof_set_digest: [u8; 32],
}
impl ZkAmsMkheCollectiveEvaluatedKeyPublicationFooterV1 {
    /// Exact opening transaction header.
    #[must_use]
    pub const fn header(self) -> ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1 {
        self.header
    }
    /// BLAKE3 of the exact reread canonical entry bytes.
    #[must_use]
    pub const fn payload_blake3(self) -> [u8; 32] {
        self.payload_blake3
    }
    /// Digest of the exact authenticated source-evidence set.
    #[must_use]
    pub const fn source_proof_set_digest(self) -> [u8; 32] {
        self.source_proof_set_digest
    }
    /// Digest of the exact authenticated CKS-evidence set.
    #[must_use]
    pub const fn cks_proof_set_digest(self) -> [u8; 32] {
        self.cks_proof_set_digest
    }
}
/// Transactional, seekable publication target for canonical evaluated keys.
///
/// `begin_entry` selects and opens one unusable staging region. Any error
/// causes `abort_entry`; only `flush_and_finalize_entry` may freeze that region.
/// A session may then advance to another disjoint canonical entry while every
/// previously finalized region remains immutable. Implementations must reject
/// every exact reopen or partial byte-range overlap with a finalized region
/// and must not expose any staging region through a provider.
pub trait ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1 {
    /// Non-zero identity of this exact publication session.
    fn publication_identity(&self) -> [u8; 32];
    /// Exact pre-sized length of the complete release artifact.
    fn artifact_len(&mut self) -> Result<u64, ZkAmsMkheErrorV1>;
    /// Select one not-yet-finalized entry and position it at the entry start.
    ///
    /// This must set `finalized_snapshot_identity` to `None` for the selected
    /// entry until `flush_and_finalize_entry` succeeds.
    fn begin_entry(
        &mut self,
        header: ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
    /// Current exact absolute artifact cursor.
    fn position(&mut self) -> Result<u64, ZkAmsMkheErrorV1>;
    /// Seek to one checked absolute artifact offset.
    fn seek(&mut self, absolute_offset: u64) -> Result<(), ZkAmsMkheErrorV1>;
    /// Write one bounded request, returning the exact number accepted.
    fn write(&mut self, source: &[u8]) -> Result<usize, ZkAmsMkheErrorV1>;
    /// Reread one bounded request, returning the exact number supplied.
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, ZkAmsMkheErrorV1>;
    /// Atomically flush, authenticate, and freeze the selected staged entry.
    ///
    /// Success returns its non-zero immutable snapshot identity. Failure must
    /// leave no publishable entry.
    fn flush_and_finalize_entry(
        &mut self,
        footer: ZkAmsMkheCollectiveEvaluatedKeyPublicationFooterV1,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1>;
    /// Return the selected entry's immutable identity, or `None` while staging.
    fn finalized_snapshot_identity(&mut self) -> Result<Option<[u8; 32]>, ZkAmsMkheErrorV1>;
    /// Poison and discard the selected incomplete transaction.
    ///
    /// Previously finalized disjoint entries must remain frozen and usable.
    fn abort_entry(&mut self, header: ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1);
}
/// One generated canonical key's immutable publication and evidence identities.
///
/// This result owns no evaluated-key polynomial or encoded payload.
#[derive(PartialEq, Eq)]
pub(super) struct ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    galois_exponent: u32,
    collective_key_digest: [u8; 32],
    source_proof_set_digest: [u8; 32],
    cks_proof_set_digest: [u8; 32],
    payload_blake3: [u8; 32],
    payload_offset: u64,
    payload_bytes: u64,
    publication_identity: [u8; 32],
    snapshot_identity: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1")
            .field("purpose", &self.purpose)
            .field("ordinal", &self.ordinal)
            .field("galois_exponent", &self.galois_exponent)
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field(
                "source_proof_set_digest",
                &hex::encode(self.source_proof_set_digest),
            )
            .field(
                "cks_proof_set_digest",
                &hex::encode(self.cks_proof_set_digest),
            )
            .field("payload_blake3", &hex::encode(self.payload_blake3))
            .field("payload_offset", &self.payload_offset)
            .field("payload_bytes", &self.payload_bytes)
            .field(
                "publication_identity",
                &hex::encode(self.publication_identity),
            )
            .field("snapshot_identity", &hex::encode(self.snapshot_identity))
            .finish()
    }
}
impl ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
    /// Evaluated-key purpose.
    #[must_use]
    pub const fn purpose(&self) -> ZkAmsMkheCollectiveEvaluatedKeyPurposeV1 {
        self.purpose
    }
    /// Exact release ordinal: relinearization first, then frozen Galois schedule order.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    /// Frozen odd Galois exponent, or zero for relinearization.
    #[must_use]
    pub const fn galois_exponent(&self) -> u32 {
        self.galois_exponent
    }
    /// Collective public-key identity used by every source proof and CKS statement.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key_digest
    }
    /// Digest of the exact authenticated pairwise-RKG or Galois-source proof set.
    #[must_use]
    pub const fn source_proof_set_digest(&self) -> [u8; 32] {
        self.source_proof_set_digest
    }
    /// Digest of all exact ordered full-roster CKS contribution proofs.
    #[must_use]
    pub const fn cks_proof_set_digest(&self) -> [u8; 32] {
        self.cks_proof_set_digest
    }
    /// BLAKE3 identity of the exact canonical `ZARK` payload.
    #[must_use]
    pub const fn payload_blake3(&self) -> [u8; 32] {
        self.payload_blake3
    }
    /// Exact canonical offset in the complete evaluated-key artifact.
    #[must_use]
    pub const fn payload_offset(&self) -> u64 {
        self.payload_offset
    }
    /// Exact canonical payload bytes.
    #[must_use]
    pub const fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    /// Exact publication session which committed the immutable entry.
    #[must_use]
    pub const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }
    /// Immutable entry-region identity returned only after atomic finalization.
    #[must_use]
    pub const fn snapshot_identity(&self) -> [u8; 32] {
        self.snapshot_identity
    }
    /// Build the exact manifest entry after successful immutable publication.
    pub fn manifest_entry(
        &self,
    ) -> Result<ZkAmsMkheCollectiveEvaluatedKeyEntryV1, ZkAmsMkheErrorV1> {
        ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
            self.ordinal,
            self.purpose,
            self.galois_exponent,
            self.payload_offset,
            self.payload_bytes,
            self.payload_blake3,
            self.source_proof_set_digest,
            self.cks_proof_set_digest,
        )
    }
}
/// Complete public statement carried beside one authenticated source proof.
///
/// The polynomial references remain valid only for the duration of the sink
/// callback.  A durable sink serializes or otherwise persists their residues;
/// retaining only the proof is deliberately insufficient.
#[derive(Clone, Copy)]
#[cfg(test)]
pub(super) enum ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a> {
    /// One party's first-round contribution to one canonical unordered RKG pair.
    RkgRoundOne {
        /// Common collective-public-key `a`.
        public_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's verified collective-public-key `b_i`.
        party_public_b: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Deterministic pair/digit RKG polynomial.
        common_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round constant contribution.
        h0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round linear contribution.
        h1: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Canonical left pair endpoint.
        left: ZkAmsMkhePartyIdV1,
        /// Canonical right pair endpoint.
        right: ZkAmsMkhePartyIdV1,
        /// Balanced gadget digit.
        digit_index: u32,
    },
    /// One party's second-round contribution, equality-linked to round one.
    RkgRoundTwo {
        /// Common collective-public-key `a`.
        public_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's verified collective-public-key `b_i`.
        party_public_b: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Deterministic pair/digit RKG polynomial.
        common_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round constant contribution.
        h0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's first round linear contribution.
        h1: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Ordered aggregate of every first-round constant contribution.
        aggregate_h0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Ordered aggregate of every first-round linear contribution.
        aggregate_h1: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's second-round constant contribution.
        k0: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Canonical left pair endpoint.
        left: ZkAmsMkhePartyIdV1,
        /// Canonical right pair endpoint.
        right: ZkAmsMkhePartyIdV1,
        /// Balanced gadget digit.
        digit_index: u32,
    },
    /// One party's encryption of `g^d sigma_k(s_i)`.
    Galois {
        /// Common collective-public-key `a`.
        public_a: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// This party's verified collective-public-key `b_i`.
        party_public_b: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Source-encryption constant polynomial.
        source_constant: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Source-encryption linear polynomial.
        source_linear: &'a ZkAmsMkheRnsPolynomialWireV1,
        /// Exact frozen schedule position.
        schedule_index: u8,
        /// Exact odd automorphism exponent at that position.
        exponent: u32,
        /// Balanced gadget digit.
        digit_index: u32,
    },
}
#[cfg(test)]
impl core::fmt::Debug for ZkAmsMkheCollectiveSourceStatementEvidenceV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RkgRoundOne {
                left,
                right,
                digit_index,
                ..
            } => formatter
                .debug_struct("RkgRoundOne")
                .field("left", left)
                .field("right", right)
                .field("digit_index", digit_index)
                .finish_non_exhaustive(),
            Self::RkgRoundTwo {
                left,
                right,
                digit_index,
                ..
            } => formatter
                .debug_struct("RkgRoundTwo")
                .field("left", left)
                .field("right", right)
                .field("digit_index", digit_index)
                .finish_non_exhaustive(),
            Self::Galois {
                schedule_index,
                exponent,
                digit_index,
                ..
            } => formatter
                .debug_struct("Galois")
                .field("schedule_index", schedule_index)
                .field("exponent", exponent)
                .field("digit_index", digit_index)
                .finish_non_exhaustive(),
        }
    }
}
/// Replayable evidence for one source proof in the exact generation sequence.
#[cfg(test)]
pub(super) struct ZkAmsMkheCollectiveSourceProofEvidenceV1<'a> {
    ordinal: u8,
    source_record_index: u32,
    party_index: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    statement: ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a>,
    proof: &'a ZkAmsMkheActiveRkgProofV1,
}
#[cfg(test)]
impl core::fmt::Debug for ZkAmsMkheCollectiveSourceProofEvidenceV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectiveSourceProofEvidenceV1")
            .field("ordinal", &self.ordinal)
            .field("source_record_index", &self.source_record_index)
            .field("party_index", &self.party_index)
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field("statement", &self.statement)
            .field(
                "statement_digest",
                &hex::encode(self.proof.statement_digest()),
            )
            .finish()
    }
}
#[cfg(test)]
impl<'a> ZkAmsMkheCollectiveSourceProofEvidenceV1<'a> {
    /// Evaluated-key ordinal containing this proof.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    /// Gap-free canonical record position within the source evidence stream.
    #[must_use]
    pub const fn source_record_index(&self) -> u32 {
        self.source_record_index
    }
    /// Exact governed contributor position.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }
    /// Frozen release-profile identity.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    /// Exact ordered roster identity.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
    /// Exact active authentication-key-set identity.
    #[must_use]
    pub const fn key_material_digest(&self) -> [u8; 32] {
        self.key_material_digest
    }
    /// Governed key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Exact ceremony transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Verified aggregate CPK identity.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key_digest
    }
    /// Complete reconstructible public algebraic statement.
    #[must_use]
    pub const fn statement(&self) -> ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a> {
        self.statement
    }
    /// Authenticated native active proof.
    #[must_use]
    pub const fn proof(&self) -> &'a ZkAmsMkheActiveRkgProofV1 {
        self.proof
    }
    /// Digest footer over every preceding canonical record byte.
    ///
    /// The evidence-set hash includes both that body and this footer.
    pub fn canonical_digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let canonical_bytes = self.canonical_encoded_len()?;
        let body_bytes = canonical_bytes
            .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let mut writer = CanonicalDigestWriter::new(body_bytes);
        self.write_canonical_body(&mut writer, canonical_bytes)?;
        writer.finish()
    }
    /// Exact self-delimiting `ZASE` bytes, including its digest footer.
    pub fn canonical_encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        let polynomial_bytes = canonical_wire_polynomial_bytes()?;
        let proof_bytes = self.proof.evidence_encoded_len()?;
        let (metadata_bytes, polynomial_count) = match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne { .. } => (32 + 32 + 4, 5),
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo { .. } => (32 + 32 + 4, 8),
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois { .. } => (1 + 4 + 4, 4),
        };
        SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1
            .checked_add(metadata_bytes)
            .and_then(|value| {
                polynomial_bytes
                    .checked_mul(polynomial_count)
                    .and_then(|bytes| value.checked_add(bytes))
            })
            .and_then(|value| value.checked_add(8))
            .and_then(|value| value.checked_add(proof_bytes))
            .and_then(|value| value.checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }
    /// Independently replay the statement, topology, active proof, and CPK linkage.
    pub fn verify(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_evidence_collective_context(
            roster,
            self.profile_digest,
            self.roster_digest,
            self.key_material_digest,
            self.epoch,
            self.transcript_digest,
            self.collective_key_digest,
            collective_key,
            shares,
        )?;
        let party_index = usize::from(self.party_index);
        let share = shares
            .get(party_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
        let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
            share.public_a(),
            share.party_public_b(),
        )?;
        let expected_record_index = match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                left,
                right,
                digit_index,
            } => {
                if self.ordinal != 0
                    || public_a != share.public_a()
                    || party_public_b != share.party_public_b()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let statement = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                    public_key,
                    common_a,
                    h0,
                    h1,
                    left,
                    right,
                    digit_index,
                )?;
                verify_zk_ams_mkhe_active_rkg_round_one_v1(
                    roster,
                    self.transcript_digest,
                    party_index,
                    statement,
                    self.proof,
                )?;
                expected_rkg_source_record_index(
                    roster,
                    left,
                    right,
                    digit_index,
                    party_index,
                    false,
                )?
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                aggregate_h0,
                aggregate_h1,
                k0,
                left,
                right,
                digit_index,
            } => {
                if self.ordinal != 0
                    || public_a != share.public_a()
                    || party_public_b != share.party_public_b()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let round_one = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                    public_key,
                    common_a,
                    h0,
                    h1,
                    left,
                    right,
                    digit_index,
                )?;
                let statement = ZkAmsMkheActiveRkgRoundTwoStatementV1::new(
                    round_one,
                    aggregate_h0,
                    aggregate_h1,
                    k0,
                )?;
                verify_zk_ams_mkhe_active_rkg_round_two_v1(
                    roster,
                    self.transcript_digest,
                    party_index,
                    statement,
                    self.proof,
                )?;
                expected_rkg_source_record_index(
                    roster,
                    left,
                    right,
                    digit_index,
                    party_index,
                    true,
                )?
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois {
                public_a,
                party_public_b,
                source_constant,
                source_linear,
                schedule_index,
                exponent,
                digit_index,
            } => {
                if self.ordinal != schedule_index.saturating_add(1)
                    || public_a != share.public_a()
                    || party_public_b != share.party_public_b()
                {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let statement = ZkAmsMkheActiveGaloisSourceStatementV1::new(
                    public_key,
                    source_constant,
                    source_linear,
                    usize::from(schedule_index),
                    exponent,
                    usize::try_from(digit_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                )?;
                verify_zk_ams_mkhe_active_galois_source_v1(
                    roster,
                    self.transcript_digest,
                    party_index,
                    statement,
                    self.proof,
                )?;
                digit_index
                    .checked_mul(
                        u32::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    )
                    .and_then(|base| base.checked_add(u32::from(self.party_index)))
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            }
        };
        if expected_record_index != self.source_record_index {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    fn record_kind(&self) -> ZkAmsMkheCollectiveEvidenceRecordKindV1 {
        match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne { .. } => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo { .. } => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois { .. } => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource
            }
        }
    }
    fn write_canonical_body(
        &self,
        writer: &mut impl CanonicalBodyWriter,
        canonical_bytes: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        write_canonical_bytes(writer, &SOURCE_EVIDENCE_RECORD_TAG_V1)?;
        write_canonical_u8(writer, MKHE_VERSION_V1)?;
        write_canonical_u8(writer, self.record_kind() as u8)?;
        write_canonical_u64(
            writer,
            u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        write_canonical_u8(writer, self.ordinal)?;
        write_canonical_u32(writer, self.source_record_index)?;
        write_canonical_u8(writer, self.party_index)?;
        write_canonical_bytes(writer, &self.profile_digest)?;
        write_canonical_bytes(writer, &self.roster_digest)?;
        write_canonical_bytes(writer, &self.key_material_digest)?;
        write_canonical_u64(writer, self.epoch)?;
        write_canonical_bytes(writer, &self.transcript_digest)?;
        write_canonical_bytes(writer, &self.collective_key_digest)?;
        match self.statement {
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                left,
                right,
                digit_index,
            } => {
                write_canonical_bytes(writer, &left.to_bytes())?;
                write_canonical_bytes(writer, &right.to_bytes())?;
                write_canonical_u32(writer, digit_index)?;
                write_canonical_wire_polynomial(writer, public_a)?;
                write_canonical_wire_polynomial(writer, party_public_b)?;
                write_canonical_wire_polynomial(writer, common_a)?;
                write_canonical_wire_polynomial(writer, h0)?;
                write_canonical_wire_polynomial(writer, h1)?;
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                public_a,
                party_public_b,
                common_a,
                h0,
                h1,
                aggregate_h0,
                aggregate_h1,
                k0,
                left,
                right,
                digit_index,
            } => {
                write_canonical_bytes(writer, &left.to_bytes())?;
                write_canonical_bytes(writer, &right.to_bytes())?;
                write_canonical_u32(writer, digit_index)?;
                write_canonical_wire_polynomial(writer, public_a)?;
                write_canonical_wire_polynomial(writer, party_public_b)?;
                write_canonical_wire_polynomial(writer, common_a)?;
                write_canonical_wire_polynomial(writer, h0)?;
                write_canonical_wire_polynomial(writer, h1)?;
                write_canonical_wire_polynomial(writer, aggregate_h0)?;
                write_canonical_wire_polynomial(writer, aggregate_h1)?;
                write_canonical_wire_polynomial(writer, k0)?;
            }
            ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois {
                public_a,
                party_public_b,
                source_constant,
                source_linear,
                schedule_index,
                exponent,
                digit_index,
            } => {
                write_canonical_u8(writer, schedule_index)?;
                write_canonical_u32(writer, exponent)?;
                write_canonical_u32(writer, digit_index)?;
                write_canonical_wire_polynomial(writer, public_a)?;
                write_canonical_wire_polynomial(writer, party_public_b)?;
                write_canonical_wire_polynomial(writer, source_constant)?;
                write_canonical_wire_polynomial(writer, source_linear)?;
            }
        }
        let proof_bytes = self.proof.evidence_encoded_len()?;
        write_canonical_u64(
            writer,
            u64::try_from(proof_bytes).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        self.proof
            .write_evidence_chunks(|chunk| write_canonical_bytes(writer, chunk))
    }
}
/// Complete source, target-key context, proofs, and recomputed compact output
/// for one full-roster CKS digit.
#[cfg(test)]
pub(super) struct ZkAmsMkheCollectiveCksDigitEvidenceV1<'a> {
    ordinal: u8,
    digit_index: u8,
    collective_key_digest: [u8; 32],
    roster: &'a ZkAmsMkheGovernedRosterWireV1,
    source: &'a ZkAmsMkheCksSourceCiphertextV1,
    target_a: &'a ZkAmsMkheRnsPolynomialWireV1,
    public_key_a: &'a ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: [&'a ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    contributions: &'a [ZkAmsMkheAuthenticatedCksContributionV1],
    compact_constant: &'a ZkAmsMkheRnsPolynomialWireV1,
}
#[cfg(test)]
impl core::fmt::Debug for ZkAmsMkheCollectiveCksDigitEvidenceV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectiveCksDigitEvidenceV1")
            .field("ordinal", &self.ordinal)
            .field("digit_index", &self.digit_index)
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field("source_digest", &hex::encode(self.source.source_digest()))
            .field("contributions", &self.contributions.len())
            .finish_non_exhaustive()
    }
}
#[cfg(test)]
impl ZkAmsMkheCollectiveCksDigitEvidenceV1<'_> {
    /// Evaluated-key ordinal containing this digit.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    /// Exact balanced gadget digit.
    #[must_use]
    pub const fn digit_index(&self) -> u8 {
        self.digit_index
    }
    /// Verified aggregate CPK identity.
    #[must_use]
    pub const fn collective_key_digest(&self) -> [u8; 32] {
        self.collective_key_digest
    }
    /// Complete exact governed wire roster used by the CKS statement.
    #[must_use]
    pub const fn roster(&self) -> &ZkAmsMkheGovernedRosterWireV1 {
        self.roster
    }
    /// Full independently keyed source ciphertext.
    #[must_use]
    pub const fn source(&self) -> &ZkAmsMkheCksSourceCiphertextV1 {
        self.source
    }
    /// Compact target `a` polynomial.
    #[must_use]
    pub const fn target_a(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        self.target_a
    }
    /// Common verified collective-public-key `a` relation polynomial.
    #[must_use]
    pub const fn public_key_a(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        self.public_key_a
    }
    /// Exact ordered verified collective-public-key `b_i` relation polynomials.
    #[must_use]
    pub const fn party_public_b(
        &self,
    ) -> &[&ZkAmsMkheRnsPolynomialWireV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        &self.party_public_b
    }
    /// Exact ordered authenticated CKS contribution set.
    #[must_use]
    pub const fn contributions(&self) -> &[ZkAmsMkheAuthenticatedCksContributionV1] {
        self.contributions
    }
    /// Recomputed compact constant polynomial stored in the generated key.
    #[must_use]
    pub const fn compact_constant(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        self.compact_constant
    }
    /// Digest footer over every preceding canonical record byte.
    ///
    /// The evidence-set hash includes both that body and this footer.
    pub fn canonical_digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let canonical_bytes = self.canonical_encoded_len()?;
        let body_bytes = canonical_bytes
            .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let mut writer = CanonicalDigestWriter::new(body_bytes);
        self.write_canonical_body(&mut writer, canonical_bytes)?;
        writer.finish()
    }
    /// Exact self-delimiting `ZACE` bytes, including its digest footer.
    pub fn canonical_encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        if self.contributions.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidCksSet);
        }
        let polynomial_bytes = canonical_wire_polynomial_bytes()?;
        let roster_bytes = self.roster.encode()?.len();
        let present_components = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .filter(|party_index| self.source.component(*party_index).is_some())
            .count();
        let mut bytes = CKS_EVIDENCE_COMMON_BODY_BYTES_V1
            .checked_add(4)
            .and_then(|value| value.checked_add(roster_bytes))
            .and_then(|value| value.checked_add(32 + 4 + 8 + 1 + 32))
            .and_then(|value| value.checked_add(polynomial_bytes))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| {
                value.checked_add(
                    ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
                        .checked_mul(32 + 1)
                        .and_then(|metadata| {
                            present_components
                                .checked_mul(polynomial_bytes)
                                .and_then(|polynomials| metadata.checked_add(polynomials))
                        })?,
                )
            })
            .and_then(|value| value.checked_add(polynomial_bytes.checked_mul(2)?))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| {
                value.checked_add(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1.checked_mul(polynomial_bytes)?)
            })
            .and_then(|value| value.checked_add(polynomial_bytes))
            .and_then(|value| value.checked_add(1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let statement = self.statement()?;
        for contribution in self.contributions {
            let contribution_bytes = contribution.to_release_wire(statement)?.encode()?.len();
            bytes = bytes
                .checked_add(8)
                .and_then(|value| value.checked_add(contribution_bytes))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        bytes
            .checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }
    /// Independently replay all eight CKS proofs and the compact output.
    pub fn verify(
        &self,
        active_roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_evidence_collective_context(
            active_roster,
            self.roster.profile_digest(),
            self.roster.roster_digest(),
            active_roster.key_material_digest(),
            self.roster.epoch(),
            self.source.transcript_digest(),
            self.collective_key_digest,
            collective_key,
            shares,
        )?;
        if self.roster != &active_roster.to_wire_roster()?
            || self.public_key_a != shares[0].public_a()
            || self
                .party_public_b
                .iter()
                .zip(shares)
                .any(|(observed, share)| *observed != share.party_public_b())
            || self.contributions.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let statement = self.statement()?;
        let compact = combine_zk_ams_mkhe_cks_v1(statement, self.contributions)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCksSet)?;
        if compact.constant() != self.compact_constant || compact.linear() != self.target_a {
            return Err(ZkAmsMkheErrorV1::InvalidCksSet);
        }
        Ok(())
    }
    fn statement(&self) -> Result<ZkAmsMkheCksStatementV1<'_>, ZkAmsMkheErrorV1> {
        ZkAmsMkheCksStatementV1::new(
            self.roster,
            self.source,
            self.target_a,
            self.public_key_a,
            &self.party_public_b,
        )
    }
    fn write_canonical_body(
        &self,
        writer: &mut impl CanonicalBodyWriter,
        canonical_bytes: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let statement = self.statement()?;
        write_canonical_bytes(writer, &CKS_EVIDENCE_RECORD_TAG_V1)?;
        write_canonical_u8(writer, MKHE_VERSION_V1)?;
        write_canonical_u64(
            writer,
            u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        write_canonical_u8(writer, self.ordinal)?;
        write_canonical_u8(writer, self.digit_index)?;
        write_canonical_bytes(writer, &self.collective_key_digest)?;
        let roster_bytes = self.roster.encode()?;
        write_canonical_u32(
            writer,
            u32::try_from(roster_bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        write_canonical_bytes(writer, &roster_bytes)?;
        write_canonical_bytes(writer, &self.source.transcript_digest())?;
        write_canonical_u32(writer, self.source.record_index())?;
        write_canonical_u64(writer, self.source.sample_index())?;
        write_canonical_u8(writer, self.source.level())?;
        write_canonical_bytes(writer, &self.source.source_digest())?;
        write_canonical_wire_polynomial(writer, self.source.constant())?;
        write_canonical_u8(
            writer,
            u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        )?;
        for (party, component) in self.roster.parties().iter().zip(
            (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map(|party_index| self.source.component(party_index)),
        ) {
            write_canonical_bytes(writer, &party.to_bytes())?;
            write_canonical_u8(writer, u8::from(component.is_some()))?;
            if let Some(component) = component {
                write_canonical_wire_polynomial(writer, component)?;
            }
        }
        write_canonical_wire_polynomial(writer, self.target_a)?;
        write_canonical_wire_polynomial(writer, self.public_key_a)?;
        write_canonical_u8(
            writer,
            u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        )?;
        for party_public_b in self.party_public_b {
            write_canonical_wire_polynomial(writer, party_public_b)?;
        }
        write_canonical_wire_polynomial(writer, self.compact_constant)?;
        write_canonical_u8(
            writer,
            u8::try_from(self.contributions.len()).map_err(|_| ZkAmsMkheErrorV1::InvalidCksSet)?,
        )?;
        for contribution in self.contributions {
            let wire = contribution.to_release_wire(statement)?;
            let bytes = wire.encode()?;
            write_canonical_u64(
                writer,
                u64::try_from(bytes.len())
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )?;
            write_canonical_bytes(writer, &bytes)?;
        }
        Ok(())
    }
}
/// Compact move-only trusted context for streaming CKS evidence verification.
///
/// The context retains only governed identity and polynomial digests. It is
/// privately sealed after the collective key and all ordered shares have been
/// validated, allowing their bulky polynomial owners to be dropped before a
/// `ZACE` record is decoded.
pub struct ZkAmsMkheTrustedCksContextV1 {
    roster: ZkAmsMkheGovernedRosterWireV1,
    key_material_digest: [u8; 32],
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    public_key_a_native_digest: [u8; 32],
    public_key_a_wire_digest: [u8; 32],
    party_public_b_native_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    party_public_b_wire_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    verification_seal: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheTrustedCksContextV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheTrustedCksContextV1")
            .field("roster_digest", &hex::encode(self.roster.roster_digest()))
            .field(
                "key_material_digest",
                &hex::encode(self.key_material_digest),
            )
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field("share_count", &self.share_digests.len())
            .field(
                "public_key_a_native_digest",
                &hex::encode(self.public_key_a_native_digest),
            )
            .field(
                "public_key_a_wire_digest",
                &hex::encode(self.public_key_a_wire_digest),
            )
            .field(
                "party_public_b_native_digest_count",
                &self.party_public_b_native_digests.len(),
            )
            .field(
                "party_public_b_wire_digest_count",
                &self.party_public_b_wire_digests.len(),
            )
            .field("privately_sealed", &(self.verification_seal != [0; 32]))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheTrustedCksContextV1 {
    /// Validate a materialized collective key and ordered shares, then mint the
    /// compact context that remains after those bulky owners are dropped.
    #[cfg(test)]
    pub fn from_verified_collective_key_and_shares(
        active_roster: &ZkAmsMkheGovernedActiveRosterV1,
        collective_key: &ZkAmsMkheCollectivePublicKeyV1,
        shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        cks_stream::trusted_context_from_verified_key_and_shares(
            active_roster,
            collective_key,
            shares,
        )
    }
    /// Crate-private mint seam for a staged CPK verifier that already checked
    /// the exact governed key/share relation without materializing all owners.
    /// TODO: keep the release RSS gate closed until the staged finalizer has a
    /// production orchestration caller and its provider/page-cache peak is
    /// independently measured; the finalizer already calls this seam.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_staged_verified_digests(
        roster: ZkAmsMkheGovernedRosterWireV1,
        key_material_digest: [u8; 32],
        transcript_digest: [u8; 32],
        collective_key_digest: [u8; 32],
        share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        public_key_a_native_digest: [u8; 32],
        public_key_a_wire_digest: [u8; 32],
        party_public_b_native_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        party_public_b_wire_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if roster.profile_digest() != release_profile_v1().digest()?
            || key_material_digest == [0; 32]
            || transcript_digest == [0; 32]
            || collective_key_digest == [0; 32]
            || share_digests.iter().any(|digest| *digest == [0; 32])
            || public_key_a_native_digest == [0; 32]
            || public_key_a_wire_digest == [0; 32]
            || party_public_b_native_digests
                .iter()
                .any(|digest| *digest == [0; 32])
            || party_public_b_wire_digests
                .iter()
                .any(|digest| *digest == [0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let verification_seal = trusted_cks_context_seal(
            roster,
            key_material_digest,
            transcript_digest,
            collective_key_digest,
            &share_digests,
            public_key_a_native_digest,
            public_key_a_wire_digest,
            &party_public_b_native_digests,
            &party_public_b_wire_digests,
        );
        Ok(Self {
            roster,
            key_material_digest,
            transcript_digest,
            collective_key_digest,
            share_digests,
            public_key_a_native_digest,
            public_key_a_wire_digest,
            party_public_b_native_digests,
            party_public_b_wire_digests,
            verification_seal,
        })
    }
    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = trusted_cks_context_seal(
            self.roster,
            self.key_material_digest,
            self.transcript_digest,
            self.collective_key_digest,
            &self.share_digests,
            self.public_key_a_native_digest,
            self.public_key_a_wire_digest,
            &self.party_public_b_native_digests,
            &self.party_public_b_wire_digests,
        );
        if self.verification_seal == [0; 32] || self.verification_seal != expected {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}
#[allow(clippy::too_many_arguments)]
fn trusted_cks_context_seal(
    roster: ZkAmsMkheGovernedRosterWireV1,
    key_material_digest: [u8; 32],
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    share_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    public_key_a_native_digest: [u8; 32],
    public_key_a_wire_digest: [u8; 32],
    party_public_b_native_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    party_public_b_wire_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CKS_TRUSTED_CONTEXT_DOMAIN_V1);
    hash.update(&roster.profile_digest());
    hash.update(&roster.roster_digest());
    hash.update(&roster.epoch().to_be_bytes());
    hash.update(&key_material_digest);
    hash.update(&transcript_digest);
    hash.update(&collective_key_digest);
    for digest in share_digests {
        hash.update(digest);
    }
    hash.update(&public_key_a_native_digest);
    hash.update(&public_key_a_wire_digest);
    for digest in party_public_b_native_digests {
        hash.update(digest);
    }
    for digest in party_public_b_wire_digests {
        hash.update(digest);
    }
    hash.finalize()
}
/// Move-only receipt for one exactly decoded and fully verified `ZACE` record.
///
/// The receipt deliberately retains no polynomial, proof, canonical byte buffer,
/// reader, path, or provider. Its private seal can only be minted after the
/// seekable decoder has replayed all eight ordered proofs and the compact output.
pub struct ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    ordinal: u8,
    digit_index: u8,
    collective_key_digest: [u8; 32],
    source_digest: [u8; 32],
    key_context_digest: [u8; 32],
    compact_constant_digest: [u8; 32],
    contribution_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
    verification_seal: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1")
            .field("ordinal", &self.ordinal)
            .field("digit_index", &self.digit_index)
            .field("canonical_bytes", &self.canonical_bytes)
            .field("canonical_digest", &hex::encode(self.canonical_digest))
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field("source_digest", &hex::encode(self.source_digest))
            .field("key_context_digest", &hex::encode(self.key_context_digest))
            .field(
                "compact_constant_digest",
                &hex::encode(self.compact_constant_digest),
            )
            .field("verified_contributions", &self.contribution_digests.len())
            .field("privately_sealed", &(self.verification_seal != [0; 32]))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    /// Exact canonical record length accepted from durable storage.
    #[must_use]
    pub const fn canonical_bytes(&self) -> u64 {
        self.canonical_bytes
    }
    /// Verified digest footer of every preceding canonical record byte.
    #[must_use]
    pub const fn canonical_digest(&self) -> [u8; 32] {
        self.canonical_digest
    }
    /// Decode exactly one seekable `ZACE` record and mint a bounded receipt.
    ///
    /// Seeking is required because canonical relation polynomials precede the
    /// contribution proofs that reference them. Every reread is digest-checked;
    /// callers may use a file, immutable object provider adapter, or a cursor,
    /// but this API never prebuffers the complete `ZACE` record.
    pub fn decode_and_verify_canonical_exact<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        trusted_context: &ZkAmsMkheTrustedCksContextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value =
            cks_stream::decode_and_verify_cks_evidence_record_streaming(reader, trusted_context)?;
        Ok(value)
    }
}
/// Generation-driven durable sink for the exact canonical evidence byte stream.
///
/// Generation first replays each complete statement and proof. It writes the
/// exact deterministic record bytes to this sink, verifies the record digest,
/// then adds the compact `{index, kind, bytes, digest}` descriptor to the shared
/// set recurrence. A sink error fails key generation; there is no advisory
/// callback path that could silently persist a different representation.
pub trait ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1 {
    /// Open one source or CKS evidence set before its first record.
    fn begin_evidence_set(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
    /// Open one preflighted, self-delimiting canonical record.
    fn begin_evidence_record(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
    /// Persist the next exact bounded chunk at a gap-free index.
    fn write_evidence_record_chunk(
        &mut self,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
        chunk_index: u32,
        bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1>;
    /// Atomically close one record after its exact length and digest are known.
    fn finish_evidence_record(
        &mut self,
        footer: ZkAmsMkheCollectiveEvidenceRecordFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
    /// Close one exact set after its gap-free count is committed to the digest.
    fn finish_evidence_set(
        &mut self,
        footer: ZkAmsMkheCollectiveEvidenceSetFooterV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
}
#[cfg(test)]
struct CeremonyContext<'a> {
    profile: BgvProfile,
    roster: &'a ZkAmsMkheGovernedActiveRosterV1,
    wire_roster: ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    shares: [&'a ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    states: [&'a ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    authentication_secrets: [&'a ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    collective_key: ZkAmsMkheCollectivePublicKeyV1,
}
#[cfg(test)]
impl<'a> CeremonyContext<'a> {
    fn new(
        roster: &'a ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        shares: [&'a ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        states: [&'a ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        authentication_secrets: [&'a ZkAmsMkheActivePartySecretV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        if transcript_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let profile = release_profile_v1();
        profile.validate()?;
        let collective_key =
            aggregate_zk_ams_mkhe_collective_public_key_v1(roster, transcript_digest, shares)?;
        for index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let expected = roster.participants()[index].party();
            if states[index].party() != expected
                || usize::from(states[index].party_index()) != index
                || states[index].profile_digest_internal() != roster.profile_digest()
                || states[index].roster_digest_internal() != roster.roster_digest()
                || states[index].key_material_digest_internal() != roster.key_material_digest()
                || states[index].epoch() != roster.epoch()
                || states[index].transcript_digest() != transcript_digest
                || states[index].public_share_digest() != shares[index].digest()
                || authentication_secrets[index].party()? != expected
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
        Ok(Self {
            profile,
            roster,
            wire_roster: roster.to_wire_roster()?,
            transcript_digest,
            shares,
            states,
            authentication_secrets,
            collective_key,
        })
    }
}
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn validate_evidence_collective_context(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    collective_key: &ZkAmsMkheCollectivePublicKeyV1,
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    let profile = release_profile_v1();
    collective_key.validate(&profile)?;
    if profile_digest != roster.profile_digest()
        || roster_digest != roster.roster_digest()
        || key_material_digest != roster.key_material_digest()
        || epoch != roster.epoch()
        || transcript_digest == [0; 32]
        || collective_key_digest == [0; 32]
        || collective_key.profile_digest() != profile_digest
        || collective_key.roster_digest() != roster_digest
        || collective_key.epoch() != epoch
        || collective_key.transcript_digest() != transcript_digest
        || collective_key.digest() != collective_key_digest
        || collective_key.public_a_wire()? != *shares[0].public_a()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    for (party_index, share) in shares.iter().enumerate() {
        if usize::from(share.party_index()) != party_index
            || share.party() != roster.participants()[party_index].party()
            || share.digest() != collective_key.share_digests_internal()[party_index]
            || share.public_a() != shares[0].public_a()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}
fn expected_rkg_source_record_index(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    left: ZkAmsMkhePartyIdV1,
    right: ZkAmsMkhePartyIdV1,
    digit_index: u32,
    party_index: usize,
    round_two: bool,
) -> Result<u32, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let digit_index =
        usize::try_from(digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    if digit_index >= profile.gadget_digits || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let left_index = roster
        .participants()
        .iter()
        .position(|participant| participant.party() == left)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    let right_index = roster
        .participants()
        .iter()
        .position(|participant| participant.party() == right)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    if left_index > right_index {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let pair_index = (0..left_index)
        .try_fold(0_usize, |sum, index| {
            sum.checked_add(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 - index)
        })
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .checked_add(right_index - left_index)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let pair_count = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 + 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let records_per_pair = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let index = digit_index
        .checked_mul(pair_count)
        .and_then(|base| base.checked_add(pair_index))
        .and_then(|pair| pair.checked_mul(records_per_pair))
        .and_then(|base| {
            base.checked_add(if round_two {
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            } else {
                0
            })
        })
        .and_then(|base| base.checked_add(party_index))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    u32::try_from(index).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
#[cfg(test)]
trait CanonicalBodyWriter {
    fn write_body(&mut self, bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1>;
}
#[cfg(test)]
struct CanonicalDigestWriter {
    hash: Keccak256,
    expected_bytes: usize,
    written_bytes: usize,
}
#[cfg(test)]
impl CanonicalDigestWriter {
    fn new(expected_bytes: usize) -> Self {
        Self {
            hash: Keccak256::new(),
            expected_bytes,
            written_bytes: 0,
        }
    }
    fn finish(self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.written_bytes != self.expected_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(self.hash.finalize())
    }
}
#[cfg(test)]
impl CanonicalBodyWriter for CanonicalDigestWriter {
    fn write_body(&mut self, bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        self.written_bytes = self
            .written_bytes
            .checked_add(bytes.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if self.written_bytes > self.expected_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.hash.update(bytes);
        Ok(())
    }
}
#[cfg(test)]
struct CanonicalRecordFanout<'a, S> {
    sink: &'a mut S,
    header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    record_hash: Keccak256,
    buffer: Box<[u8; ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1]>,
    buffered: usize,
    body_bytes: usize,
    expected_body_bytes: usize,
    chunk_index: u32,
}
#[cfg(test)]
impl<'a, S> CanonicalRecordFanout<'a, S>
where
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    fn new(
        sink: &'a mut S,
        header: ZkAmsMkheCollectiveEvidenceRecordHeaderV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let canonical_bytes = usize::try_from(header.canonical_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected_body_bytes = canonical_bytes
            .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        sink.begin_evidence_record(header)?;
        Ok(Self {
            sink,
            header,
            record_hash: Keccak256::new(),
            buffer: Box::new([0; ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1]),
            buffered: 0,
            body_bytes: 0,
            expected_body_bytes,
            chunk_index: 0,
        })
    }
    fn flush_body_chunk(&mut self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.buffered == 0 {
            return Ok(());
        }
        let chunk = &self.buffer[..self.buffered];
        self.record_hash.update(chunk);
        self.sink
            .write_evidence_record_chunk(self.header, self.chunk_index, chunk)?;
        self.chunk_index = self
            .chunk_index
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.buffered = 0;
        Ok(())
    }
    fn finish(mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.body_bytes != self.expected_body_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.flush_body_chunk()?;
        let digest = self.record_hash.finalize();
        self.sink
            .write_evidence_record_chunk(self.header, self.chunk_index, &digest)?;
        self.chunk_index = self
            .chunk_index
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.sink
            .finish_evidence_record(ZkAmsMkheCollectiveEvidenceRecordFooterV1 {
                header: self.header,
                chunk_count: self.chunk_index,
                canonical_digest: digest,
            })?;
        Ok(digest)
    }
}
#[cfg(test)]
impl<S> CanonicalBodyWriter for CanonicalRecordFanout<'_, S>
where
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    fn write_body(&mut self, mut bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        self.body_bytes = self
            .body_bytes
            .checked_add(bytes.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if self.body_bytes > self.expected_body_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        while !bytes.is_empty() {
            let available = ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1 - self.buffered;
            let take = available.min(bytes.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&bytes[..take]);
            self.buffered += take;
            bytes = &bytes[take..];
            if self.buffered == ZK_AMS_MKHE_EVIDENCE_CHUNK_BYTES_V1 {
                self.flush_body_chunk()?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
fn write_canonical_bytes(
    writer: &mut impl CanonicalBodyWriter,
    bytes: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(bytes)
}
#[cfg(test)]
fn write_canonical_u8(
    writer: &mut impl CanonicalBodyWriter,
    value: u8,
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(&[value])
}
#[cfg(test)]
fn write_canonical_u32(
    writer: &mut impl CanonicalBodyWriter,
    value: u32,
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(&value.to_be_bytes())
}
#[cfg(test)]
fn write_canonical_u64(
    writer: &mut impl CanonicalBodyWriter,
    value: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    writer.write_body(&value.to_be_bytes())
}
fn canonical_wire_polynomial_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .and_then(|count| count.checked_mul(core::mem::size_of::<u64>()))
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
#[cfg(test)]
fn write_canonical_wire_polynomial(
    writer: &mut impl CanonicalBodyWriter,
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.encoded_len()?;
    write_canonical_u32(
        writer,
        u32::try_from(polynomial.residues().len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )?;
    const RESIDUES_PER_BATCH: usize = 1024;
    let mut bytes = [0_u8; RESIDUES_PER_BATCH * core::mem::size_of::<u64>()];
    for residues in polynomial.residues().chunks(RESIDUES_PER_BATCH) {
        for (destination, residue) in bytes.chunks_exact_mut(8).zip(residues) {
            destination.copy_from_slice(&residue.to_be_bytes());
        }
        writer.write_body(&bytes[..residues.len() * 8])?;
    }
    Ok(())
}
struct CanonicalBodyReader<'a, R> {
    reader: &'a mut R,
    hash: Keccak256,
    remaining: u64,
}
impl<'a, R> CanonicalBodyReader<'a, R>
where
    R: std::io::Read,
{
    fn new(reader: &'a mut R, prefix: &[u8], remaining: u64) -> Self {
        let mut hash = Keccak256::new();
        hash.update(prefix);
        Self {
            reader,
            hash,
            remaining,
        }
    }
    fn remaining(&self) -> u64 {
        self.remaining
    }
    fn finish(self) -> Result<(&'a mut R, [u8; 32]), ZkAmsMkheErrorV1> {
        if self.remaining != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok((self.reader, self.hash.finalize()))
    }
}
impl<R> CanonicalBodyReader<'_, R>
where
    R: std::io::Read + std::io::Seek,
{
    fn absolute_position(&mut self) -> Result<u64, ZkAmsMkheErrorV1> {
        self.reader
            .stream_position()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
}
impl<R> std::io::Read for CanonicalBodyReader<'_, R>
where
    R: std::io::Read,
{
    fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
        if destination.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        let limit = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(destination.len());
        let read = self.reader.read(&mut destination[..limit])?;
        if read != 0 {
            self.hash.update(&destination[..read]);
            self.remaining -= read as u64;
        }
        Ok(read)
    }
}
fn canonical_polynomial_residue_count() -> Result<usize, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn maximum_source_evidence_record_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1
        .checked_add(32 + 32 + 4)
        .and_then(|value| {
            canonical_wire_polynomial_bytes()
                .ok()?
                .checked_mul(8)?
                .checked_add(value)
        })
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1))
        .and_then(|value| value.checked_add(4_096))
        .and_then(|value| value.checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn maximum_cks_contribution_record_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    canonical_wire_polynomial_bytes()?
        .checked_add(super::ZK_AMS_MKHE_MAX_PROOF_BYTES_V1)
        .and_then(|value| value.checked_add(4_096))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn maximum_cks_evidence_record_bytes() -> Result<usize, ZkAmsMkheErrorV1> {
    let polynomial_bytes = canonical_wire_polynomial_bytes()?;
    let roster_bytes = 4_096;
    CKS_EVIDENCE_COMMON_BODY_BYTES_V1
        .checked_add(4 + roster_bytes)
        .and_then(|value| value.checked_add(32 + 4 + 8 + 1 + 32))
        .and_then(|value| polynomial_bytes.checked_mul(20)?.checked_add(value))
        .and_then(|value| value.checked_add(1 + ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 * 33 + 1 + 1))
        .and_then(|value| {
            maximum_cks_contribution_record_bytes()
                .ok()?
                .checked_add(8)?
                .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?
                .checked_add(value)
        })
        .and_then(|value| value.checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn read_canonical_raw_exact(
    reader: &mut impl std::io::Read,
    bytes: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    reader
        .read_exact(bytes)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}
fn read_canonical_array<const N: usize>(
    reader: &mut impl std::io::Read,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let mut bytes = [0_u8; N];
    read_canonical_raw_exact(reader, &mut bytes)?;
    Ok(bytes)
}
fn read_canonical_u8(reader: &mut impl std::io::Read) -> Result<u8, ZkAmsMkheErrorV1> {
    Ok(read_canonical_array::<1>(reader)?[0])
}
fn read_canonical_u32(reader: &mut impl std::io::Read) -> Result<u32, ZkAmsMkheErrorV1> {
    Ok(u32::from_be_bytes(read_canonical_array(reader)?))
}
fn read_canonical_u64(reader: &mut impl std::io::Read) -> Result<u64, ZkAmsMkheErrorV1> {
    Ok(u64::from_be_bytes(read_canonical_array(reader)?))
}
fn read_canonical_party(
    reader: &mut impl std::io::Read,
) -> Result<ZkAmsMkhePartyIdV1, ZkAmsMkheErrorV1> {
    ZkAmsMkhePartyIdV1::new(read_canonical_array(reader)?)
}
fn read_canonical_vec_exact(
    reader: &mut impl std::io::Read,
    length: usize,
    ceiling: usize,
) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    if length == 0 || length > ceiling {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes.resize(length, 0);
    read_canonical_raw_exact(reader, &mut bytes)?;
    Ok(bytes)
}
fn finish_canonical_body<R: std::io::Read>(
    body: CanonicalBodyReader<'_, R>,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let (reader, observed) = body.finish()?;
    let expected = read_canonical_array(reader)?;
    if observed != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(observed)
}
fn require_canonical_reader_eof(reader: &mut impl std::io::Read) -> Result<(), ZkAmsMkheErrorV1> {
    let mut trailing = [0_u8; 1];
    loop {
        match reader.read(&mut trailing) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}
include!("collective_eval_keys/zeroizing_vectors.rs");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IndexedCksPolynomialV1 {
    residues_offset: u64,
    native_digest: [u8; 32],
    wire_digest: [u8; 32],
    nonzero: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IndexedCksStatementV1 {
    roster: ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    source_record_index: u32,
    sample_index: u64,
    level: u8,
    source_digest: [u8; 32],
    key_context_digest: [u8; 32],
    source_constant: IndexedCksPolynomialV1,
    source_components: [Option<IndexedCksPolynomialV1>; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    target_a: IndexedCksPolynomialV1,
    public_key_a: IndexedCksPolynomialV1,
    party_public_b: [IndexedCksPolynomialV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    compact_constant: IndexedCksPolynomialV1,
}
fn new_rns_digest_hasher(
    domain: &[u8],
    residue_count: usize,
) -> Result<Keccak256, ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(
        &u32::try_from(residue_count)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    Ok(hash)
}
fn update_rns_digest_hasher(hash: &mut Keccak256, residues: &[u64]) {
    const RESIDUES_PER_BATCH: usize = 512;
    let mut bytes = [0_u8; RESIDUES_PER_BATCH * core::mem::size_of::<u64>()];
    for batch in residues.chunks(RESIDUES_PER_BATCH) {
        for (destination, residue) in bytes.chunks_exact_mut(8).zip(batch) {
            destination.copy_from_slice(&residue.to_be_bytes());
        }
        hash.update(&bytes[..batch.len() * 8]);
    }
}
fn index_canonical_cks_polynomial<R>(
    body: &mut CanonicalBodyReader<'_, R>,
    profile: &BgvProfile,
) -> Result<IndexedCksPolynomialV1, ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    let residue_count = canonical_polynomial_residue_count()?;
    if usize::try_from(read_canonical_u32(body)?).ok() != Some(residue_count) {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let residues_offset = body.absolute_position()?;
    let mut native_hash = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut wire_hash = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
    let mut index = 0_usize;
    let mut nonzero = false;
    while index < residue_count {
        let take_residues = (residue_count - index).min(buffer.len() / 8);
        let take_bytes = take_residues * 8;
        read_canonical_raw_exact(body, &mut buffer[..take_bytes])?;
        native_hash.update(&buffer[..take_bytes]);
        wire_hash.update(&buffer[..take_bytes]);
        for encoded in buffer[..take_bytes].chunks_exact(8) {
            let residue = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            let limb = index / profile.ring_degree;
            if limb >= profile.moduli.len() || residue >= profile.moduli[limb] {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            nonzero |= residue != 0;
            index += 1;
        }
    }
    Ok(IndexedCksPolynomialV1 {
        residues_offset,
        native_digest: native_hash.finalize(),
        wire_digest: wire_hash.finalize(),
        nonzero,
    })
}
fn read_indexed_cks_limb<R>(
    reader: &mut R,
    polynomial: IndexedCksPolynomialV1,
    profile: &BgvProfile,
    limb: usize,
) -> Result<ZeroizingU64VectorV1, ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    let offset = u64::try_from(limb)
        .ok()
        .and_then(|limb| limb.checked_mul(profile.ring_degree as u64))
        .and_then(|residue| residue.checked_mul(8))
        .and_then(|bytes| polynomial.residues_offset.checked_add(bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    reader
        .seek(std::io::SeekFrom::Start(offset))
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let modulus = *profile
        .moduli
        .get(limb)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut residues = ZeroizingU64VectorV1::with_capacity_exact(profile.ring_degree)?;
    let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
    while residues.len() < profile.ring_degree {
        let take_residues = (profile.ring_degree - residues.len()).min(buffer.len() / 8);
        let take_bytes = take_residues * 8;
        read_canonical_raw_exact(reader, &mut buffer[..take_bytes])?;
        for encoded in buffer[..take_bytes].chunks_exact(8) {
            let residue = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            if residue >= modulus {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            residues.push(residue);
        }
    }
    Ok(residues)
}
fn load_indexed_cks_accumulator<R>(
    reader: &mut R,
    polynomial: IndexedCksPolynomialV1,
    profile: &BgvProfile,
) -> Result<ZeroizingU64VectorV1, ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    let residue_count = canonical_polynomial_residue_count()?;
    let mut accumulator = ZeroizingU64VectorV1::with_capacity_exact(residue_count)?;
    let mut native_hash = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut wire_hash = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    for limb in 0..profile.moduli.len() {
        let residues = read_indexed_cks_limb(reader, polynomial, profile, limb)?;
        update_rns_digest_hasher(&mut native_hash, &residues);
        update_rns_digest_hasher(&mut wire_hash, &residues);
        accumulator.extend_from_slice(&residues);
    }
    if native_hash.finalize() != polynomial.native_digest
        || wire_hash.finalize() != polynomial.wire_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(accumulator)
}
#[cfg(test)]
struct EvidenceHasher {
    digest: evidence_set::EvidenceSetDigestV1,
    records: u32,
    header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
}
#[cfg(test)]
impl EvidenceHasher {
    fn new<S>(
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
        ordinal: u8,
        exponent: u32,
        collective_key_digest: [u8; 32],
        kind: ZkAmsMkheCollectiveEvidenceSetKindV1,
        sink: &mut S,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if collective_key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let header = ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
            kind,
            purpose,
            ordinal,
            galois_exponent: exponent,
            collective_key_digest,
        };
        sink.begin_evidence_set(header)?;
        let digest = evidence_set::EvidenceSetDigestV1::new(header)?;
        Ok(Self {
            digest,
            records: 0,
            header,
        })
    }
    fn source<S>(
        &mut self,
        evidence: &ZkAmsMkheCollectiveSourceProofEvidenceV1<'_>,
        sink: &mut S,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if self.header.kind != ZkAmsMkheCollectiveEvidenceSetKindV1::Source
            || evidence.ordinal() != self.header.ordinal
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.expect_next(evidence.source_record_index())?;
        let canonical_bytes = evidence.canonical_encoded_len()?;
        let header = ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
            set: self.header,
            kind: evidence.record_kind(),
            record_index: evidence.source_record_index(),
            canonical_bytes: u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        };
        let mut writer = CanonicalRecordFanout::new(sink, header)?;
        evidence.write_canonical_body(&mut writer, canonical_bytes)?;
        let canonical_digest = writer.finish()?;
        self.digest.absorb_record(
            header.record_index,
            header.kind,
            header.canonical_bytes,
            canonical_digest,
        )?;
        self.advance()
    }
    fn cks<S>(
        &mut self,
        evidence: &ZkAmsMkheCollectiveCksDigitEvidenceV1<'_>,
        sink: &mut S,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if self.header.kind != ZkAmsMkheCollectiveEvidenceSetKindV1::Cks
            || evidence.ordinal() != self.header.ordinal
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.expect_next(u32::from(evidence.digit_index()))?;
        let canonical_bytes = evidence.canonical_encoded_len()?;
        let header = ZkAmsMkheCollectiveEvidenceRecordHeaderV1 {
            set: self.header,
            kind: ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit,
            record_index: u32::from(evidence.digit_index()),
            canonical_bytes: u64::try_from(canonical_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        };
        let mut writer = CanonicalRecordFanout::new(sink, header)?;
        evidence.write_canonical_body(&mut writer, canonical_bytes)?;
        let canonical_digest = writer.finish()?;
        self.digest.absorb_record(
            header.record_index,
            header.kind,
            header.canonical_bytes,
            canonical_digest,
        )?;
        self.advance()
    }
    fn expect_next(&self, record_index: u32) -> Result<(), ZkAmsMkheErrorV1> {
        if record_index != self.records {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    fn advance(&mut self) -> Result<(), ZkAmsMkheErrorV1> {
        self.records = self
            .records
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }
    #[cfg(test)]
    fn test_record(
        &mut self,
        record_index: u32,
        canonical_bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let record_kind = match self.header.kind {
            ZkAmsMkheCollectiveEvidenceSetKindV1::Source => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne
            }
            ZkAmsMkheCollectiveEvidenceSetKindV1::Cks => {
                ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit
            }
        };
        self.test_descriptor_record(record_index, record_kind, canonical_bytes)
    }
    #[cfg(test)]
    fn test_descriptor_record(
        &mut self,
        record_index: u32,
        record_kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
        canonical_bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.expect_next(record_index)?;
        let canonical_bytes_len = canonical_bytes
            .len()
            .checked_add(EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.digest.absorb_record(
            record_index,
            record_kind,
            canonical_bytes_len,
            keccak256(canonical_bytes),
        )?;
        self.advance()
    }
    fn finish<S>(
        mut self,
        expected_records: u32,
        sink: &mut S,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1>
    where
        S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    {
        if expected_records == 0 || self.records != expected_records {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let digest = self.digest.finish(expected_records)?;
        sink.finish_evidence_set(ZkAmsMkheCollectiveEvidenceSetFooterV1 {
            header: self.header,
            record_count: self.records,
            canonical_digest: digest,
        })?;
        Ok(digest)
    }
}
#[cfg(test)]
fn validated_source_evidence<'a>(
    context: &CeremonyContext<'_>,
    ordinal: u8,
    source_record_index: u32,
    party_index: usize,
    statement: ZkAmsMkheCollectiveSourceStatementEvidenceV1<'a>,
    proof: &'a ZkAmsMkheActiveRkgProofV1,
) -> Result<ZkAmsMkheCollectiveSourceProofEvidenceV1<'a>, ZkAmsMkheErrorV1> {
    let evidence = ZkAmsMkheCollectiveSourceProofEvidenceV1 {
        ordinal,
        source_record_index,
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        profile_digest: context.roster.profile_digest(),
        roster_digest: context.roster.roster_digest(),
        key_material_digest: context.roster.key_material_digest(),
        epoch: context.roster.epoch(),
        transcript_digest: context.transcript_digest,
        collective_key_digest: context.collective_key.digest(),
        statement,
        proof,
    };
    evidence.verify(context.roster, &context.collective_key, context.shares)?;
    Ok(evidence)
}
fn evaluated_key_evidence_digest(
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    collective_key_digest: [u8; 32],
    source_proof_set_digest: [u8; 32],
    cks_proof_set_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if collective_key_digest == [0; 32]
        || source_proof_set_digest == [0; 32]
        || cks_proof_set_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut frame = Vec::with_capacity(160);
    frame.extend_from_slice(EVALUATED_KEY_EVIDENCE_DOMAIN_V1);
    frame.extend_from_slice(&[MKHE_VERSION_V1, purpose as u8, ordinal]);
    frame.extend_from_slice(&exponent.to_be_bytes());
    frame.extend_from_slice(&collective_key_digest);
    frame.extend_from_slice(&source_proof_set_digest);
    frame.extend_from_slice(&cks_proof_set_digest);
    Ok(keccak256(&frame))
}
#[allow(
    clippy::too_many_arguments,
    reason = "the derivation binds each governed evaluated-key context axis explicitly"
)]
fn evaluated_key_target_a_context(
    roster: &ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    master_seed: [u8; 32],
    digit_index: usize,
) -> Result<[u8; EVALUATED_KEY_TARGET_A_CONTEXT_BYTES_V1], ZkAmsMkheErrorV1> {
    if transcript_digest == [0; 32] || collective_key_digest == [0; 32] || master_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile_digest = roster.profile_digest();
    let roster_digest = roster.roster_digest();
    let epoch = roster.epoch().to_be_bytes();
    let purpose_ordinal = [purpose as u8, ordinal];
    let exponent = exponent.to_be_bytes();
    let digit = u16::try_from(digit_index)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
        .to_be_bytes();
    let mut context = [0_u8; EVALUATED_KEY_TARGET_A_CONTEXT_BYTES_V1];
    let mut cursor = 0_usize;
    for value in [
        profile_digest.as_slice(),
        roster_digest.as_slice(),
        epoch.as_slice(),
        transcript_digest.as_slice(),
        collective_key_digest.as_slice(),
        purpose_ordinal.as_slice(),
        exponent.as_slice(),
        master_seed.as_slice(),
        digit.as_slice(),
    ] {
        let end = cursor
            .checked_add(value.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        context
            .get_mut(cursor..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .copy_from_slice(value);
        cursor = end;
    }
    if cursor != context.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(context)
}
#[allow(clippy::too_many_arguments)]
fn derive_target_a(
    profile: &BgvProfile,
    roster: &ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    master_seed: [u8; 32],
    digit_index: usize,
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    if digit_index >= profile.gadget_digits {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let context = evaluated_key_target_a_context(
        roster,
        transcript_digest,
        collective_key_digest,
        purpose,
        ordinal,
        exponent,
        master_seed,
        digit_index,
    )?;
    derive_uniform_rns_from_context(profile, EVALUATED_KEY_TARGET_A_DOMAIN_V1, &context)
}
#[allow(clippy::too_many_arguments)]
fn derive_target_a_limb(
    profile: &BgvProfile,
    roster: &ZkAmsMkheGovernedRosterWireV1,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    master_seed: [u8; 32],
    digit_index: usize,
    limb_index: usize,
    output: &mut [u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    profile.validate()?;
    if digit_index >= profile.gadget_digits
        || limb_index >= profile.moduli.len()
        || output.len() != profile.ring_degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let context = evaluated_key_target_a_context(
        roster,
        transcript_digest,
        collective_key_digest,
        purpose,
        ordinal,
        exponent,
        master_seed,
        digit_index,
    )?;
    let maximum_xof_bytes = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .and_then(|value| value.checked_mul(MAX_RANDOM_REJECTION_ATTEMPTS_V1))
        .and_then(|value| value.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    super::checked_rng_bytes(profile, maximum_xof_bytes)?;
    let profile_digest = profile.digest()?;
    const FRAME_BYTES: usize = EVALUATED_KEY_TARGET_A_DOMAIN_V1.len()
        + 32
        + 4
        + EVALUATED_KEY_TARGET_A_CONTEXT_BYTES_V1
        + 2;
    let context_len = u32::try_from(context.len())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
        .to_be_bytes();
    let limb = u16::try_from(limb_index)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
        .to_be_bytes();
    let mut frame = [0_u8; FRAME_BYTES];
    let mut cursor = 0_usize;
    for value in [
        EVALUATED_KEY_TARGET_A_DOMAIN_V1,
        profile_digest.as_slice(),
        context_len.as_slice(),
        context.as_slice(),
        limb.as_slice(),
    ] {
        let end = cursor
            .checked_add(value.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        frame
            .get_mut(cursor..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .copy_from_slice(value);
        cursor = end;
    }
    if cursor != frame.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let modulus = profile.moduli[limb_index];
    let zone = u64::MAX - u64::MAX % modulus;
    let mut stream = Shake256Reader::new(&frame);
    for coefficient in output {
        let mut accepted = None;
        for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
            let mut bytes = [0_u8; 8];
            stream.read(&mut bytes);
            let candidate = u64::from_le_bytes(bytes);
            if candidate < zone {
                accepted = Some(candidate % modulus);
                break;
            }
        }
        *coefficient = accepted.ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    }
    Ok(())
}
#[cfg(test)]
fn with_cks_statement<T>(
    context: &CeremonyContext<'_>,
    source: &ZkAmsMkheCksSourceCiphertextV1,
    target_a: &ZkAmsMkheRnsPolynomialWireV1,
    operation: impl FnOnce(ZkAmsMkheCksStatementV1<'_>) -> Result<T, ZkAmsMkheErrorV1>,
) -> Result<T, ZkAmsMkheErrorV1> {
    let public_a = context.shares[0].public_a();
    if context
        .shares
        .iter()
        .any(|share| share.public_a() != public_a)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let party_public_b = std::array::from_fn(|index| context.shares[index].party_public_b());
    let statement = ZkAmsMkheCksStatementV1::new(
        &context.wire_roster,
        source,
        target_a,
        public_a,
        &party_public_b,
    )?;
    operation(statement)
}
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn compact_source_digit<R, S>(
    context: &CeremonyContext<'_>,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    master_seed: [u8; 32],
    digit_index: usize,
    source_constant: RnsPolynomial,
    source_components: [RnsPolynomial; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    cks_evidence: &mut EvidenceHasher,
    random: &mut R,
    sink: &mut S,
) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
{
    source_constant.validate(&context.profile)?;
    for component in &source_components {
        component.validate(&context.profile)?;
    }
    let record_index = usize::from(ordinal)
        .checked_mul(context.profile.gadget_digits)
        .and_then(|base| base.checked_add(digit_index))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let source = ZkAmsMkheCksSourceCiphertextV1::new(
        &context.wire_roster,
        context.transcript_digest,
        record_index,
        u64::from(record_index),
        0,
        ZkAmsMkheRnsPolynomialWireV1::new(source_constant.coefficients)?,
        context
            .wire_roster
            .parties()
            .iter()
            .copied()
            .zip(source_components)
            .map(|(party, polynomial)| {
                Ok((
                    party,
                    ZkAmsMkheRnsPolynomialWireV1::new(polynomial.coefficients)?,
                ))
            })
            .collect::<Result<Vec<_>, ZkAmsMkheErrorV1>>()?,
    )?;
    let target_a = derive_target_a(
        &context.profile,
        &context.wire_roster,
        context.transcript_digest,
        context.collective_key.digest(),
        purpose,
        ordinal,
        exponent,
        master_seed,
        digit_index,
    )?;
    let target_a_wire = ZkAmsMkheRnsPolynomialWireV1::new(target_a.coefficients)?;
    with_cks_statement(context, &source, &target_a_wire, |statement| {
        let mut contributions = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
        for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let contribution = prove_zk_ams_mkhe_cks_contribution_v1(
                statement,
                party_index,
                context.states[party_index],
                context.authentication_secrets[party_index],
                random,
            )?;
            contributions.push(contribution);
        }
        let compact = combine_zk_ams_mkhe_cks_v1(statement, &contributions)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCksSet)?;
        if compact.linear() != &target_a_wire {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let evidence = ZkAmsMkheCollectiveCksDigitEvidenceV1 {
            ordinal,
            digit_index: u8::try_from(digit_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
            collective_key_digest: context.collective_key.digest(),
            roster: &context.wire_roster,
            source: &source,
            target_a: &target_a_wire,
            public_key_a: statement.public_key_a(),
            party_public_b: *statement.party_public_b(),
            contributions: &contributions,
            compact_constant: compact.constant(),
        };
        evidence.verify(context.roster, &context.collective_key, context.shares)?;
        cks_evidence.cks(&evidence, sink)?;
        Ok(compact.constant().clone())
    })
}
#[allow(
    clippy::too_many_arguments,
    reason = "the canonical header commits each protocol field in a fixed order"
)]
fn seekable_publication_header_bytes(
    profile: &BgvProfile,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    ordinal: u8,
    master_seed: [u8; 32],
    contribution_proof_digest: [u8; 32],
) -> Result<[u8; SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1], ZkAmsMkheErrorV1> {
    if profile_digest == [0; 32]
        || roster_digest == [0; 32]
        || epoch == 0
        || transcript_digest == [0; 32]
        || master_seed == [0; 32]
        || contribution_proof_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut header = [0_u8; SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1];
    let mut cursor = 0_usize;
    for bytes in [
        SEEKABLE_EVALUATED_KEY_TAG_V1.as_slice(),
        &[MKHE_VERSION_V1],
        profile_digest.as_slice(),
        roster_digest.as_slice(),
        epoch.to_be_bytes().as_slice(),
        transcript_digest.as_slice(),
        u32::from(ordinal).to_be_bytes().as_slice(),
        &[0],
        master_seed.as_slice(),
        contribution_proof_digest.as_slice(),
        &[u8::try_from(profile.gadget_digits).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?],
    ] {
        let end = cursor
            .checked_add(bytes.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        header
            .get_mut(cursor..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .copy_from_slice(bytes);
        cursor = end;
    }
    if cursor != header.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(header)
}
#[derive(Clone, Copy)]
struct SeekablePublicationFinishContextV1<'a> {
    profile: &'a BgvProfile,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
}
struct SeekableEvaluatedKeyPublicationTransactionV1<
    'a,
    P: ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1 + ?Sized,
> {
    sink: &'a mut P,
    header: ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1,
    publication_identity: [u8; 32],
    layout: SeekableEvaluatedKeyLayoutV1,
    digit_blake3: Vec<[u8; 32]>,
    next_digit: usize,
    finished: bool,
}
impl<'a, P> SeekableEvaluatedKeyPublicationTransactionV1<'a, P>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1 + ?Sized,
{
    fn begin(
        profile: &BgvProfile,
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
        ordinal: u8,
        exponent: u32,
        sink: &'a mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let layout = seekable_evaluated_key_layout(profile)?;
        let artifact_key_count = ZK_AMS_T256_GALOIS_KEY_COUNT_V1
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let ordinal_usize = usize::from(ordinal);
        if ordinal_usize >= artifact_key_count
            || matches!(
                purpose,
                ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization
            ) != (ordinal == 0 && exponent == 0)
            || matches!(purpose, ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois)
                && (ordinal == 0 || exponent == 0 || exponent.is_multiple_of(2))
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let payload_offset = layout
            .payload_bytes
            .checked_mul(u64::from(ordinal))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let artifact_bytes = layout
            .payload_bytes
            .checked_mul(
                u64::try_from(artifact_key_count)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let header = ZkAmsMkheCollectiveEvaluatedKeyPublicationHeaderV1 {
            purpose,
            ordinal,
            galois_exponent: exponent,
            payload_offset,
            payload_bytes: layout.payload_bytes,
            artifact_bytes,
        };
        let publication_identity = sink.publication_identity();
        if publication_identity == [0; 32] || sink.artifact_len()? != artifact_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if let Err(error) = sink.begin_entry(header) {
            // A sink may have touched staging state before reporting failure.
            // Poison it unconditionally so no partial entry can later become
            // visible through a provider.
            sink.abort_entry(header);
            return Err(error);
        }
        if sink.publication_identity() != publication_identity
            || sink.artifact_len()? != artifact_bytes
            || sink.finalized_snapshot_identity()?.is_some()
            || sink.position()? != payload_offset
        {
            sink.abort_entry(header);
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut transaction = Self {
            sink,
            header,
            publication_identity,
            layout,
            digit_blake3: Vec::new(),
            next_digit: 0,
            finished: false,
        };
        transaction
            .digit_blake3
            .try_reserve_exact(profile.gadget_digits)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if transaction.digit_blake3.capacity() != profile.gadget_digits {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        transaction.write_exact(&[0; SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1])?;
        Ok(transaction)
    }
    fn checked_position(&mut self, expected: u64) -> Result<(), ZkAmsMkheErrorV1> {
        if self.sink.publication_identity() != self.publication_identity
            || self.sink.artifact_len()? != self.header.artifact_bytes
            || self.sink.finalized_snapshot_identity()?.is_some()
            || self.sink.position()? != expected
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    fn seek_exact(&mut self, absolute_offset: u64) -> Result<(), ZkAmsMkheErrorV1> {
        if absolute_offset > self.header.artifact_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let before = self.sink.position()?;
        self.checked_position(before)?;
        self.sink.seek(absolute_offset)?;
        self.checked_position(absolute_offset)
    }
    fn write_exact(&mut self, source: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        if source.is_empty() || source.len() > SEEKABLE_EVALUATED_KEY_READ_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let before = self.sink.position()?;
        self.checked_position(before)?;
        let after = before
            .checked_add(
                u64::try_from(source.len())
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if after
            > self
                .header
                .payload_offset
                .checked_add(self.header.payload_bytes)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            || self.sink.write(source)? != source.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.checked_position(after)
    }
    fn read_exact(&mut self, destination: &mut [u8]) -> Result<(), ZkAmsMkheErrorV1> {
        if destination.is_empty() || destination.len() > SEEKABLE_EVALUATED_KEY_READ_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let before = self.sink.position()?;
        self.checked_position(before)?;
        let after = before
            .checked_add(
                u64::try_from(destination.len())
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if after
            > self
                .header
                .payload_offset
                .checked_add(self.header.payload_bytes)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            || self.sink.read(destination)? != destination.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.checked_position(after)
    }
    fn write_digit(
        &mut self,
        profile: &BgvProfile,
        digit_index: usize,
        stored_b_residues: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if digit_index != self.next_digit
            || digit_index >= profile.gadget_digits
            || stored_b_residues.len() != self.layout.residue_count
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let expected_offset = self
            .header
            .payload_offset
            .checked_add(
                u64::try_from(SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .and_then(|value| {
                self.layout
                    .digit_record_bytes
                    .checked_mul(u64::try_from(digit_index).ok()?)
                    .and_then(|digit| value.checked_add(digit))
            })
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.checked_position(expected_offset)?;
        let mut hasher = norito::streaming::Blake3Hasher::new();
        let mut prefix = [0_u8; SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1];
        prefix[0] = u8::try_from(digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        prefix[1..].copy_from_slice(
            &u32::try_from(self.layout.residue_count)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        self.write_exact(&prefix)?;
        hasher.update(&prefix);
        let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
        for residues in stored_b_residues
            .chunks(SEEKABLE_EVALUATED_KEY_READ_BYTES_V1 / core::mem::size_of::<u64>())
        {
            let bytes = residues
                .len()
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            for (encoded, residue) in buffer[..bytes]
                .chunks_exact_mut(core::mem::size_of::<u64>())
                .zip(residues)
            {
                encoded.copy_from_slice(&residue.to_be_bytes());
            }
            self.write_exact(&buffer[..bytes])?;
            hasher.update(&buffer[..bytes]);
        }
        self.digit_blake3.push(hasher.finalize());
        self.next_digit = self
            .next_digit
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn finish(
        mut self,
        context: SeekablePublicationFinishContextV1<'_>,
        master_seed: [u8; 32],
        source_proof_set_digest: [u8; 32],
        cks_proof_set_digest: [u8; 32],
    ) -> Result<ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1> {
        if self.next_digit != context.profile.gadget_digits
            || self.digit_blake3.len() != context.profile.gadget_digits
        {
            return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
        }
        let contribution_proof_digest = evaluated_key_evidence_digest(
            self.header.purpose,
            self.header.ordinal,
            self.header.galois_exponent,
            context.collective_key_digest,
            source_proof_set_digest,
            cks_proof_set_digest,
        )?;
        let canonical_header = seekable_publication_header_bytes(
            context.profile,
            context.profile_digest,
            context.roster_digest,
            context.epoch,
            context.transcript_digest,
            self.header.ordinal,
            master_seed,
            contribution_proof_digest,
        )?;
        self.seek_exact(self.header.payload_offset)?;
        self.write_exact(&canonical_header)?;
        self.seek_exact(self.header.payload_offset)?;
        let mut payload_hasher = norito::streaming::Blake3Hasher::new();
        let mut reread_header = [0_u8; SEEKABLE_EVALUATED_KEY_HEADER_BYTES_V1];
        self.read_exact(&mut reread_header)?;
        if reread_header != canonical_header {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        payload_hasher.update(&reread_header);
        let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
        for digit_index in 0..context.profile.gadget_digits {
            let mut digit_hasher = norito::streaming::Blake3Hasher::new();
            let mut prefix = [0_u8; SEEKABLE_EVALUATED_KEY_DIGIT_PREFIX_BYTES_V1];
            self.read_exact(&mut prefix)?;
            payload_hasher.update(&prefix);
            digit_hasher.update(&prefix);
            if usize::from(prefix[0]) != digit_index
                || usize::try_from(u32::from_be_bytes(
                    prefix[1..]
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                ))
                .ok()
                    != Some(self.layout.residue_count)
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            let mut residue_index = 0_usize;
            let mut remaining = usize::try_from(self.layout.native_polynomial_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            while remaining != 0 {
                let take = remaining.min(buffer.len());
                self.read_exact(&mut buffer[..take])?;
                payload_hasher.update(&buffer[..take]);
                digit_hasher.update(&buffer[..take]);
                for encoded in buffer[..take].chunks_exact(core::mem::size_of::<u64>()) {
                    let residue = u64::from_be_bytes(
                        encoded
                            .try_into()
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                    );
                    let limb = residue_index / context.profile.ring_degree;
                    if limb >= context.profile.moduli.len()
                        || residue >= context.profile.moduli[limb]
                    {
                        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                    }
                    residue_index = residue_index
                        .checked_add(1)
                        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                }
                remaining -= take;
            }
            if residue_index != self.layout.residue_count
                || digit_hasher.finalize() != self.digit_blake3[digit_index]
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        let expected_end = self
            .header
            .payload_offset
            .checked_add(self.header.payload_bytes)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.checked_position(expected_end)?;
        let payload_blake3 = payload_hasher.finalize();
        if payload_blake3 == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let footer = ZkAmsMkheCollectiveEvaluatedKeyPublicationFooterV1 {
            header: self.header,
            payload_blake3,
            source_proof_set_digest,
            cks_proof_set_digest,
        };
        let snapshot_identity = self.sink.flush_and_finalize_entry(footer)?;
        if snapshot_identity == [0; 32]
            || self.sink.publication_identity() != self.publication_identity
            || self.sink.artifact_len()? != self.header.artifact_bytes
            || self.sink.finalized_snapshot_identity()? != Some(snapshot_identity)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.finished = true;
        Ok(ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1 {
            purpose: self.header.purpose,
            ordinal: self.header.ordinal,
            galois_exponent: self.header.galois_exponent,
            collective_key_digest: context.collective_key_digest,
            source_proof_set_digest,
            cks_proof_set_digest,
            payload_blake3,
            payload_offset: self.header.payload_offset,
            payload_bytes: self.header.payload_bytes,
            publication_identity: self.publication_identity,
            snapshot_identity,
        })
    }
}
impl<P> Drop for SeekableEvaluatedKeyPublicationTransactionV1<'_, P>
where
    P: ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1 + ?Sized,
{
    fn drop(&mut self) {
        if !self.finished {
            self.sink.abort_entry(self.header);
        }
    }
}
#[cfg(test)]
fn sample_nonzero_ternary<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let candidate = SecretPolynomial::sample_ternary(profile, random)?;
        if candidate
            .coefficients
            .iter()
            .any(|coefficient| *coefficient != 0)
        {
            return Ok(candidate);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}
#[cfg(test)]
fn scaled_error(
    profile: &BgvProfile,
    error: &SecretPolynomial,
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    error.as_rns(profile)?.scale_plaintext_modulus(profile)
}
#[cfg(test)]
fn add_weighted_pair_source(
    profile: &BgvProfile,
    diagonal: bool,
    source_constant: &mut RnsPolynomial,
    source_linear: &mut RnsPolynomial,
    pair_constant: &RnsPolynomial,
    pair_linear: &RnsPolynomial,
) -> Result<(), ZkAmsMkheErrorV1> {
    let weighted_constant = if diagonal {
        pair_constant.clone()
    } else {
        pair_constant.add(pair_constant, profile)?
    };
    let weighted_linear = if diagonal {
        pair_linear.clone()
    } else {
        pair_linear.add(pair_linear, profile)?
    };
    *source_constant = source_constant.add(&weighted_constant, profile)?;
    *source_linear = source_linear.add(&weighted_linear, profile)?;
    Ok(())
}
/// Generate the exact 38-digit compact collective relinearization key.
///
/// Every digit first aggregates all 36 canonical unordered pair products.
/// Diagonal terms have weight one and all 28 off-diagonal terms have weight
/// two, so the source decrypts to exactly `g^d (sum_i s_i)^2`.  The complete
/// source is then compacted by eight real proof-carrying CKS contributions.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_zk_ams_mkhe_collective_relinearization_key_v1<R, S, P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    states: [&ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    authentication_secrets: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    master_seed: [u8; 32],
    random: &mut R,
    sink: &mut S,
    publication: &mut P,
) -> Result<ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    P: ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1 + ?Sized,
{
    let context = CeremonyContext::new(
        roster,
        transcript_digest,
        shares,
        states,
        authentication_secrets,
    )?;
    if master_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let purpose = ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization;
    let ordinal = 0_u8;
    let exponent = 0_u32;
    let mut publication = SeekableEvaluatedKeyPublicationTransactionV1::begin(
        &context.profile,
        purpose,
        ordinal,
        exponent,
        publication,
    )?;
    let mut source_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        sink,
    )?;
    let mut cks_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Cks,
        sink,
    )?;
    let parties = PartySet::new(context.wire_roster.parties().to_vec())?;
    let pair_count = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 + 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for digit_index in 0..context.profile.gadget_digits {
        let mut source_constant = RnsPolynomial::zero(&context.profile);
        let mut source_linear = RnsPolynomial::zero(&context.profile);
        let mut pair_index = 0_usize;
        for left_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            for right_index in left_index..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                let left = context.wire_roster.parties()[left_index];
                let right = context.wire_roster.parties()[right_index];
                let common_a = derive_rkg_common_a(
                    &context.profile,
                    &parties,
                    transcript_digest,
                    left,
                    right,
                    digit_index,
                )?;
                let common_a_wire =
                    ZkAmsMkheRnsPolynomialWireV1::new(common_a.coefficients.clone())?;
                let mut ephemerals = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut error_zeros = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut error_ones = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut h0_values = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                let mut h1_values = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
                for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                    let ephemeral = sample_nonzero_ternary(&context.profile, random)?;
                    let error_zero = SecretPolynomial::sample_error(&context.profile, random)?;
                    let error_one = SecretPolynomial::sample_error(&context.profile, random)?;
                    let secret_rns = context.states[party_index]
                        .secret()
                        .as_rns(&context.profile)?;
                    let ephemeral_rns = ephemeral.as_rns(&context.profile)?;
                    let mut h0 = common_a
                        .mul(&ephemeral_rns, &context.profile)?
                        .negate(&context.profile)?;
                    if party_index == left_index {
                        h0 = h0.add(
                            &secret_rns.scale_gadget(digit_index, &context.profile)?,
                            &context.profile,
                        )?;
                    }
                    h0 = h0.add(
                        &scaled_error(&context.profile, &error_zero)?,
                        &context.profile,
                    )?;
                    let mut h1 = scaled_error(&context.profile, &error_one)?;
                    if party_index == right_index {
                        h1 = h1.add(
                            &common_a.mul(&secret_rns, &context.profile)?,
                            &context.profile,
                        )?;
                    }
                    let h0_wire = ZkAmsMkheRnsPolynomialWireV1::new(h0.coefficients.clone())?;
                    let h1_wire = ZkAmsMkheRnsPolynomialWireV1::new(h1.coefficients.clone())?;
                    let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
                        context.shares[party_index].public_a(),
                        context.shares[party_index].party_public_b(),
                    )?;
                    let statement = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                        public_key,
                        &common_a_wire,
                        &h0_wire,
                        &h1_wire,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    )?;
                    let witness = ZkAmsMkheActiveRkgRoundOneWitnessV1::new(
                        &context.states[party_index].secret().coefficients,
                        &context.states[party_index].public_error().coefficients,
                        &ephemeral.coefficients,
                        &error_zero.coefficients,
                        &error_one.coefficients,
                    )?;
                    let proof = prove_zk_ams_mkhe_active_rkg_round_one_v1(
                        roster,
                        transcript_digest,
                        party_index,
                        statement,
                        witness,
                        context.authentication_secrets[party_index],
                        random,
                    )?;
                    let record_index = expected_rkg_source_record_index(
                        roster,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        party_index,
                        false,
                    )?;
                    let evidence = validated_source_evidence(
                        &context,
                        ordinal,
                        record_index,
                        party_index,
                        ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundOne {
                            public_a: context.shares[party_index].public_a(),
                            party_public_b: context.shares[party_index].party_public_b(),
                            common_a: &common_a_wire,
                            h0: &h0_wire,
                            h1: &h1_wire,
                            left,
                            right,
                            digit_index: u32::try_from(digit_index)
                                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        },
                        &proof,
                    )?;
                    source_evidence.source(&evidence, sink)?;
                    ephemerals.push(ephemeral);
                    error_zeros.push(error_zero);
                    error_ones.push(error_one);
                    h0_values.push(h0);
                    h1_values.push(h1);
                }
                checked_coefficient_work(&context.profile, 2 * ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?;
                let mut aggregate_h0 = RnsPolynomial::zero(&context.profile);
                let mut aggregate_h1 = RnsPolynomial::zero(&context.profile);
                for (h0, h1) in h0_values.iter().zip(&h1_values) {
                    aggregate_h0 = aggregate_h0.add(h0, &context.profile)?;
                    aggregate_h1 = aggregate_h1.add(h1, &context.profile)?;
                }
                let aggregate_h0_wire =
                    ZkAmsMkheRnsPolynomialWireV1::new(aggregate_h0.coefficients.clone())?;
                let aggregate_h1_wire =
                    ZkAmsMkheRnsPolynomialWireV1::new(aggregate_h1.coefficients.clone())?;
                let mut pair_constant = RnsPolynomial::zero(&context.profile);
                for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                    let error_two = SecretPolynomial::sample_error(&context.profile, random)?;
                    let secret_rns = context.states[party_index]
                        .secret()
                        .as_rns(&context.profile)?;
                    let right_secret = if party_index == right_index {
                        secret_rns
                    } else {
                        RnsPolynomial::zero(&context.profile)
                    };
                    let difference = ephemerals[party_index]
                        .sub(context.states[party_index].secret())?
                        .as_rns(&context.profile)?;
                    let k0 = aggregate_h0
                        .mul(&right_secret, &context.profile)?
                        .add(
                            &aggregate_h1.mul(&difference, &context.profile)?,
                            &context.profile,
                        )?
                        .add(
                            &scaled_error(&context.profile, &error_two)?,
                            &context.profile,
                        )?;
                    let k0_wire = ZkAmsMkheRnsPolynomialWireV1::new(k0.coefficients.clone())?;
                    let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
                        context.shares[party_index].public_a(),
                        context.shares[party_index].party_public_b(),
                    )?;
                    let party_h0_wire = ZkAmsMkheRnsPolynomialWireV1::new(
                        h0_values[party_index].coefficients.clone(),
                    )?;
                    let party_h1_wire = ZkAmsMkheRnsPolynomialWireV1::new(
                        h1_values[party_index].coefficients.clone(),
                    )?;
                    let round_one_statement = ZkAmsMkheActiveRkgRoundOneStatementV1::new(
                        public_key,
                        &common_a_wire,
                        &party_h0_wire,
                        &party_h1_wire,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    )?;
                    let round_one_witness = ZkAmsMkheActiveRkgRoundOneWitnessV1::new(
                        &context.states[party_index].secret().coefficients,
                        &context.states[party_index].public_error().coefficients,
                        &ephemerals[party_index].coefficients,
                        &error_zeros[party_index].coefficients,
                        &error_ones[party_index].coefficients,
                    )?;
                    let statement = ZkAmsMkheActiveRkgRoundTwoStatementV1::new(
                        round_one_statement,
                        &aggregate_h0_wire,
                        &aggregate_h1_wire,
                        &k0_wire,
                    )?;
                    let witness = ZkAmsMkheActiveRkgRoundTwoWitnessV1::new(
                        round_one_witness,
                        &error_two.coefficients,
                    )?;
                    let proof = prove_zk_ams_mkhe_active_rkg_round_two_v1(
                        roster,
                        transcript_digest,
                        party_index,
                        statement,
                        witness,
                        context.authentication_secrets[party_index],
                        random,
                    )?;
                    let record_index = expected_rkg_source_record_index(
                        roster,
                        left,
                        right,
                        u32::try_from(digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        party_index,
                        true,
                    )?;
                    let evidence = validated_source_evidence(
                        &context,
                        ordinal,
                        record_index,
                        party_index,
                        ZkAmsMkheCollectiveSourceStatementEvidenceV1::RkgRoundTwo {
                            public_a: context.shares[party_index].public_a(),
                            party_public_b: context.shares[party_index].party_public_b(),
                            common_a: &common_a_wire,
                            h0: &party_h0_wire,
                            h1: &party_h1_wire,
                            aggregate_h0: &aggregate_h0_wire,
                            aggregate_h1: &aggregate_h1_wire,
                            k0: &k0_wire,
                            left,
                            right,
                            digit_index: u32::try_from(digit_index)
                                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                        },
                        &proof,
                    )?;
                    source_evidence.source(&evidence, sink)?;
                    pair_constant = pair_constant.add(&k0, &context.profile)?;
                }
                add_weighted_pair_source(
                    &context.profile,
                    left_index == right_index,
                    &mut source_constant,
                    &mut source_linear,
                    &pair_constant,
                    &aggregate_h1,
                )?;
                pair_index = pair_index
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            }
        }
        if pair_index != pair_count {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let source_components = std::array::from_fn(|_| source_linear.clone());
        let stored_b = compact_source_digit(
            &context,
            purpose,
            ordinal,
            exponent,
            master_seed,
            digit_index,
            source_constant,
            source_components,
            &mut cks_evidence,
            random,
            sink,
        )?;
        publication.write_digit(&context.profile, digit_index, stored_b.residues())?;
    }
    let expected_source_records = pair_count
        .checked_mul(context.profile.gadget_digits)
        .and_then(|records| records.checked_mul(2))
        .and_then(|records| records.checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1))
        .and_then(|records| u32::try_from(records).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let expected_cks_records = u32::try_from(context.profile.gadget_digits)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let source_proof_set_digest = source_evidence.finish(expected_source_records, sink)?;
    let cks_proof_set_digest = cks_evidence.finish(expected_cks_records, sink)?;
    publication.finish(
        SeekablePublicationFinishContextV1 {
            profile: &context.profile,
            profile_digest: context.wire_roster.profile_digest(),
            roster_digest: context.wire_roster.roster_digest(),
            epoch: context.wire_roster.epoch(),
            transcript_digest: context.transcript_digest,
            collective_key_digest: context.collective_key.digest(),
        },
        master_seed,
        source_proof_set_digest,
        cks_proof_set_digest,
    )
}
/// Generate one exact compact collective Galois key in frozen schedule order.
///
/// For each digit all eight parties prove an encryption of
/// `g^d sigma_k(s_i)` under their already verified collective-public-key
/// share.  The ordered aggregate is then compacted through the same real CKS
/// path as relinearization.  The caller supplies a schedule index, not a free
/// exponent, so missing, reordered, or substituted keys cannot be repaired.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_zk_ams_mkhe_collective_galois_key_v1<R, S, P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    states: [&ZkAmsMkheCollectivePartyStateV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    authentication_secrets: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    schedule_index: usize,
    master_seed: [u8; 32],
    random: &mut R,
    sink: &mut S,
    publication: &mut P,
) -> Result<ZkAmsMkheGeneratedCollectiveEvaluatedKeyV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    S: ZkAmsMkheCollectiveEvaluatedKeyEvidenceSinkV1,
    P: ZkAmsMkheCollectiveEvaluatedKeyPublicationSinkV1 + ?Sized,
{
    let context = CeremonyContext::new(
        roster,
        transcript_digest,
        shares,
        states,
        authentication_secrets,
    )?;
    let schedule = zk_ams_t256_galois_key_schedule_v1()?;
    validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
    let schedule_entry = schedule
        .entries
        .get(schedule_index)
        .ok_or(ZkAmsMkheErrorV1::MissingEvaluatedKey)?;
    if schedule_index >= ZK_AMS_T256_GALOIS_KEY_COUNT_V1 || master_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let purpose = ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois;
    let ordinal = u8::try_from(
        schedule_index
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let exponent = schedule_entry.exponent;
    let exponent_usize =
        usize::try_from(exponent).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let mut publication = SeekableEvaluatedKeyPublicationTransactionV1::begin(
        &context.profile,
        purpose,
        ordinal,
        exponent,
        publication,
    )?;
    let mut source_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        sink,
    )?;
    let mut cks_evidence = EvidenceHasher::new(
        purpose,
        ordinal,
        exponent,
        context.collective_key.digest(),
        ZkAmsMkheCollectiveEvidenceSetKindV1::Cks,
        sink,
    )?;
    for digit_index in 0..context.profile.gadget_digits {
        let mut source_constant = RnsPolynomial::zero(&context.profile);
        let mut source_components: [RnsPolynomial; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            std::array::from_fn(|_| RnsPolynomial::zero(&context.profile));
        for (party_index, source_component) in source_components.iter_mut().enumerate() {
            let ephemeral = sample_nonzero_ternary(&context.profile, random)?;
            let error_zero = SecretPolynomial::sample_error(&context.profile, random)?;
            let error_one = SecretPolynomial::sample_error(&context.profile, random)?;
            let public_a = RnsPolynomial::from_flat(
                &context.profile,
                context.shares[party_index].public_a().residues().to_vec(),
            )?;
            let public_b = RnsPolynomial::from_flat(
                &context.profile,
                context.shares[party_index]
                    .party_public_b()
                    .residues()
                    .to_vec(),
            )?;
            let ephemeral_rns = ephemeral.as_rns(&context.profile)?;
            let transformed_secret = context.states[party_index]
                .secret()
                .automorphism(exponent_usize, &context.profile)?
                .as_rns(&context.profile)?
                .scale_gadget(digit_index, &context.profile)?;
            let constant = public_b
                .mul(&ephemeral_rns, &context.profile)?
                .add(
                    &scaled_error(&context.profile, &error_zero)?,
                    &context.profile,
                )?
                .add(&transformed_secret, &context.profile)?;
            let linear = public_a.mul(&ephemeral_rns, &context.profile)?.add(
                &scaled_error(&context.profile, &error_one)?,
                &context.profile,
            )?;
            let constant_wire = ZkAmsMkheRnsPolynomialWireV1::new(constant.coefficients.clone())?;
            let linear_wire = ZkAmsMkheRnsPolynomialWireV1::new(linear.coefficients.clone())?;
            let public_key = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(
                context.shares[party_index].public_a(),
                context.shares[party_index].party_public_b(),
            )?;
            let statement = ZkAmsMkheActiveGaloisSourceStatementV1::new(
                public_key,
                &constant_wire,
                &linear_wire,
                schedule_index,
                exponent,
                digit_index,
            )?;
            let witness = ZkAmsMkheActiveGaloisSourceWitnessV1::new(
                &context.states[party_index].secret().coefficients,
                &context.states[party_index].public_error().coefficients,
                &ephemeral.coefficients,
                &error_zero.coefficients,
                &error_one.coefficients,
            )?;
            let proof = prove_zk_ams_mkhe_active_galois_source_v1(
                roster,
                transcript_digest,
                party_index,
                statement,
                witness,
                context.authentication_secrets[party_index],
                random,
            )?;
            verify_zk_ams_mkhe_active_galois_source_v1(
                roster,
                transcript_digest,
                party_index,
                statement,
                &proof,
            )?;
            let source_record_index = digit_index
                .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .and_then(|base| base.checked_add(party_index))
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let evidence = validated_source_evidence(
                &context,
                ordinal,
                source_record_index,
                party_index,
                ZkAmsMkheCollectiveSourceStatementEvidenceV1::Galois {
                    public_a: context.shares[party_index].public_a(),
                    party_public_b: context.shares[party_index].party_public_b(),
                    source_constant: &constant_wire,
                    source_linear: &linear_wire,
                    schedule_index: u8::try_from(schedule_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    exponent,
                    digit_index: u32::try_from(digit_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                },
                &proof,
            )?;
            source_evidence.source(&evidence, sink)?;
            source_constant = source_constant.add(&constant, &context.profile)?;
            *source_component = linear;
        }
        let stored_b = compact_source_digit(
            &context,
            purpose,
            ordinal,
            exponent,
            master_seed,
            digit_index,
            source_constant,
            source_components,
            &mut cks_evidence,
            random,
            sink,
        )?;
        publication.write_digit(&context.profile, digit_index, stored_b.residues())?;
    }
    let expected_source_records = context
        .profile
        .gadget_digits
        .checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .and_then(|records| u32::try_from(records).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let expected_cks_records = u32::try_from(context.profile.gadget_digits)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let source_proof_set_digest = source_evidence.finish(expected_source_records, sink)?;
    let cks_proof_set_digest = cks_evidence.finish(expected_cks_records, sink)?;
    publication.finish(
        SeekablePublicationFinishContextV1 {
            profile: &context.profile,
            profile_digest: context.wire_roster.profile_digest(),
            roster_digest: context.wire_roster.roster_digest(),
            epoch: context.wire_roster.epoch(),
            transcript_digest: context.transcript_digest,
            collective_key_digest: context.collective_key.digest(),
        },
        master_seed,
        source_proof_set_digest,
        cks_proof_set_digest,
    )
}
/// Stable, seekable view of the complete content-addressed evaluated-key artifact.
///
/// The provider identity names one open provider session. The snapshot identity
/// names the immutable content revision visible through that session and must
/// not incorporate the mutable cursor position. Both identities are checked
/// before and after validation and every digit loan. Implementations must fail
/// rather than repair a seek or exact bounded read.
pub trait ZkAmsMkheCollectiveEvaluatedKeyProviderV1 {
    /// Non-zero identity of this exact open provider session.
    fn provider_identity(&self) -> [u8; 32];
    /// Exact consensus-bound SoraFS pointer served by this provider.
    fn sorafs_pointer(&self) -> ZkAmsMkheEvaluatedKeySorafsPointerV1;
    /// Non-zero immutable content revision visible through this session.
    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1>;
    /// Exact complete artifact length, not merely the selected key length.
    fn payload_len(&mut self) -> Result<u64, ZkAmsMkheErrorV1>;
    /// Seek to one checked absolute artifact offset.
    fn seek(&mut self, absolute_offset: u64) -> Result<(), ZkAmsMkheErrorV1>;
    /// Fill one bounded request and return the exact number of bytes supplied.
    ///
    /// Returning fewer bytes than requested, including zero at EOF, is a hard
    /// failure in the canonical provider adapter.
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, ZkAmsMkheErrorV1>;
}
/// Exact portable allocation, I/O, and work accounting for one seekable key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheSeekableEvaluatedKeyAccountingV1 {
    /// Exact canonical bytes of one complete `ZARK` entry.
    pub canonical_payload_bytes: u64,
    /// Exact canonical bytes of one indexed stored-`b` digit record.
    pub canonical_digit_record_bytes: u64,
    /// Bytes read and incrementally hashed before a key can be used.
    pub incremental_validation_read_bytes: u64,
    /// Bytes reread and per-limb authenticated by one complete key switch.
    pub per_key_switch_read_bytes: u64,
    /// Exact canonical allocation size of one native limb-major polynomial.
    ///
    /// The switch path does not allocate this shape; it remains exposed to
    /// describe the canonical entry and legacy test reference.
    pub native_polynomial_allocation_bytes: u64,
    /// Exact heap capacity of one degree-N native residue limb.
    pub native_limb_allocation_bytes: u64,
    /// Exact heap capacity of the two owned output accumulators.
    pub output_accumulator_bytes: u64,
    /// Exact digit-major signed coefficient batch retained while switching one key.
    pub signed_decomposition_scratch_bytes: u64,
    /// Exact one-residue-per-limb CRT input scratch.
    pub crt_residue_scratch_bytes: u64,
    /// Exact two degree-N limb buffers used by in-place NTT accumulation.
    pub ntt_limb_scratch_bytes: u64,
    /// Exact bounded provider read buffer.
    pub provider_read_buffer_bytes: u64,
    /// Exact target-layout size of one incremental BLAKE3 state.
    pub provider_hash_state_bytes: u64,
    /// Managed live bytes while retaining one batch, both limbs, and CRT scratch.
    pub decomposition_phase_bytes: u64,
    /// Managed live bytes while decoding and authenticating one stored limb.
    pub provider_read_phase_bytes: u64,
    /// Managed live bytes at the in-place NTT multiply-accumulate peak.
    pub multiplication_phase_bytes: u64,
    /// Exact heap allocation-layout high water at that peak.
    pub peak_heap_allocation_bytes: u64,
    /// Exact maximum of all implementation liveness phases.
    ///
    /// This includes owned output accumulators, exact Vec capacities, explicit
    /// scratch objects, and fixed hash/read state, but excludes caller-owned
    /// immutable input ciphertexts and allocator metadata.
    pub peak_managed_workspace_bytes: u64,
    /// Fixed metadata retained for every digit and every authenticated limb.
    pub validation_metadata_bytes: u64,
    /// Accounted coefficient work for CRT/radix passes shared within each batch.
    pub balanced_decomposition_work_units: u64,
    /// Exact work of the frozen 76 negacyclic ring multiplications.
    pub ring_multiplication_work_units: u64,
    /// Exact coefficient-limb additions into the two output accumulators.
    pub accumulator_addition_work_units: u64,
    /// Complete arithmetic work; authenticated I/O bytes remain separate.
    pub total_key_switch_work_units: u64,
}
/// Return the frozen release accounting for the sole seekable provider path.
pub fn zk_ams_mkhe_seekable_evaluated_key_accounting_v1()
-> Result<ZkAmsMkheSeekableEvaluatedKeyAccountingV1, ZkAmsMkheErrorV1> {
    seekable_evaluated_key_accounting(&release_profile_v1())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SeekableEvaluatedKeyLayoutV1 {
    residue_count: usize,
    native_polynomial_bytes: u64,
    digit_record_bytes: u64,
    payload_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SeekableEvaluatedKeyDigitV1 {
    absolute_offset: u64,
    canonical_bytes: u64,
    blake3: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SeekableEvaluatedKeyLimbV1 {
    absolute_offset: u64,
    canonical_bytes: u64,
    blake3: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SeekableProviderStateV1 {
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    payload_len: u64,
}
#[derive(Clone, Copy)]
struct SeekableEvaluatedKeyExpectedV1 {
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    pointer: ZkAmsMkheEvaluatedKeySorafsPointerV1,
    artifact_key_count: usize,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    contribution_proof_digest: [u8; 32],
    cks_compact_output_set_digest: [u8; 32],
}
struct SeekableEvaluatedKeyValidationV1 {
    state: SeekableProviderStateV1,
    a_master_seed: [u8; 32],
    contribution_proof_digest: [u8; 32],
    digits: Vec<SeekableEvaluatedKeyDigitV1>,
    limbs: Vec<SeekableEvaluatedKeyLimbV1>,
    limb_index_digest: [u8; 32],
}
include!("collective_eval_keys/runtime.rs");
/// Exact release ring-multiplication count of one compact key switch.
pub fn zk_ams_mkhe_compact_key_switch_ring_multiplications_v1() -> Result<u64, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    u64::try_from(
        profile
            .gadget_digits
            .checked_mul(2)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
#[cfg(test)]
fn blake3_hash(input: &[u8]) -> [u8; 32] {
    norito::streaming::blake3_hash(input)
}
#[cfg(test)]
mod tests {
    include!("collective_eval_keys_tests.rs");
}
