//! Fail-closed boundary for Kagemusha Pasta-cycle recursion.
//!
//! Retired generic cross-field `Halo2Loader` prototypes emulated transcript
//! scalars and reached multi-gigabyte RSS; the earlier degree-20 processed key
//! shape was likewise too large. Those measurements explain the original
//! runaway-memory failure, but neither construction remains a production or
//! generation fallback.
//!
//! The supported nested compact V5 profile keeps the public ABI at 21 and the
//! release manifest at V4. It fixes both parities at degree 16, exposes one
//! 64-element commitment column, keeps the exact 138-`u32`
//! predecessor/result state boundary private, and caps each processed proving
//! key at 5 GiB. Production retains authenticated proving keys as file-backed
//! spools and verifier keys as bounded raw bytes. It parses Eq and Ep one at a
//! time, then materializes terminal verifier domains only after both proving
//! keys and populated circuits have been released.
//!
//! The production wire carries the current Eq/Fp and Ep/Fq proofs together.
//! The fixed verifier derives every transcript challenge, residual coefficient,
//! and IPA accumulator from proof bytes; none is caller-selected wire data.
//! The production build retains the native terminal Eq/Vesta and Ep/Pallas
//! decisions over authenticated parameters and verifier keys. Tests retain the
//! fixed-key Poseidon proof wires, canonical BGH19 IPA folding, and exact
//! bounded proof bytes. Both recursive fixed-VK verifier halves constrain those
//! same operations. Production availability remains false pending the
//! authenticated complete archive, independent review, and physical-device gates.

#[cfg(test)]
use iroha_data_model::offline::KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4;
use iroha_data_model::offline::{
    KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5, KAGEMUSHA_COMPACT_PROFILE_VERSION_V5,
    KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5, KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
    KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
    KagemushaAuthenticatedReleaseV4, KagemushaPastaCycleArtifactKindV4,
    KagemushaPastaCycleParityV1, KagemushaRecursiveSpendArtifactManifestV4,
    KagemushaRecursiveSpendPublicStatementV4,
};
pub use iroha_data_model::offline::{KagemushaPastaPublicLayoutV4, KagemushaStepCircuitParamsV4};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::io::Read as _;
use std::sync::Arc;

use ff::{Field as _, PrimeField};
use halo2_proofs::halo2curves::{
    CurveAffine,
    pasta::{Fp, Fq},
};
use snark_verifier::verifier::plonk::PlonkProtocol;

use super::kagemusha_accumulation::{
    KagemushaIpaAccumulationProofV4, KagemushaIpaAccumulatorWireV4,
};
use super::kagemusha_dense_msm::{KagemushaDenseMsmConfigV5, KagemushaDenseMsmJobsV5};
use super::kagemusha_sha256_v4::{
    KagemushaSha256ByteV4, KagemushaSha256ConfigV4, KagemushaSha256JobsV4,
};
use super::kagemusha_step_transition::{
    KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4, KAGEMUSHA_STEP_OPERATION_LIMBS_V4,
    KagemushaStepOperationVectorV4,
};

/// Maximum exact parent states consumed by one recursive transition.
pub const KAGEMUSHA_PASTA_PARENT_SLOTS_V1: usize = 2;

/// Exact public-column layout for the operation/protocol-identity Step wire.
pub const KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4: usize = 0;
/// First exact operation limb.
pub const KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4 + 8;
/// Parent-count cell.
pub const KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4 + KAGEMUSHA_STEP_OPERATION_LIMBS_V4;
/// First parent-state limb; each slot has the fixed state-vector stride.
pub const KAGEMUSHA_PASTA_PARENT_STATES_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4 + 1;
/// Exact stride of one parent/result state.
pub const KAGEMUSHA_PASTA_STATE_STRIDE_V4: usize =
    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2;
/// First result-state limb.
pub const KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4: usize = KAGEMUSHA_PASTA_PARENT_STATES_OFFSET_V4
    + KAGEMUSHA_PASTA_PARENT_SLOTS_V1 * KAGEMUSHA_PASTA_STATE_STRIDE_V4;
/// First manifest SHA-256 word.
pub const KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4 + KAGEMUSHA_PASTA_STATE_STRIDE_V4;
/// First Eq compiled-protocol identity word.
pub const KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4 + 8;
/// First Ep compiled-protocol identity word.
pub const KAGEMUSHA_PASTA_STEP_EP_PROTOCOL_SHA256_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4 + 8;

// Compact V5 public-header offsets. The legacy-named constants above describe
// the exact private semantic witness and intentionally remain separate: only
// these compact cells enter recursive instance commitments.
const KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5: usize = 0;
const KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5: usize = 1;
const KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5: usize = 2;
const KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5: usize = 3;
const KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5: usize = 5;
const KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5: usize = 7;
const KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5: usize = 11;
const KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5: usize = 13;
const KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5: usize = 15;
const KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5: usize = 17;
const KAGEMUSHA_COMPACT_HEADER_WITHOUT_SELECTOR_CELLS_V5: usize = 19;
const KAGEMUSHA_COMPACT_DIGEST_CHUNKS_V5: usize = 2;
const KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_DOMAIN_V5: u64 = u64::from_le_bytes(*b"kgopcm05");
const KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5: u64 = u64::from_le_bytes(*b"kgstcm05");
fn validate_kagemusha_circuit_params_v4(
    params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaPastaPublicLayoutV4, String> {
    params
        .validate()
        .map_err(|error| format!("invalid authenticated Kagemusha V4 circuit parameters: {error}"))
}

// The compact profile admits one reviewed k16 shape for artifact decoding and
// candidate generation. Keeping the generation boundary explicit prevents a
// stale or column-heavy profile from reaching `ParamsIPA::new` or Halo2
// configure/keygen even if it was decoded from a historical carrier.
const KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4: &[u32] = &[443];
const KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4: &[u32] = &[47, 0, 0];
const KAGEMUSHA_GENERATION_FIXED_COLUMNS_V4: u32 = 1;
const KAGEMUSHA_GENERATION_INSTANCE_COLUMNS_V4: u32 = 1;
const KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4: u64 = 12 * 1024 * 1024 * 1024;
const KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5: u64 = 12 * 1024 * 1024 * 1024;
const KAGEMUSHA_GENERATION_FIXED_HEADROOM_BYTES_V4: u64 = 56 * 1024 * 1024;
const KAGEMUSHA_GENERATION_QUOTIENT_HEADROOM_BYTES_V5: u64 = 72 * 1024 * 1024;
const KAGEMUSHA_GENERATION_FIELD_BYTES_V4: u64 = 32;
const KAGEMUSHA_GENERATION_AFFINE_BYTES_V4: u64 = 64;
const KAGEMUSHA_GENERATION_PARITIES_V4: u64 = 2;
const KAGEMUSHA_GENERATION_LIVE_COLUMN_COPIES_V4: u64 = 4;
const KAGEMUSHA_GENERATION_IPA_POINT_VECTORS_V4: u64 = 2;
const KAGEMUSHA_GENERATION_RAYON_THREADS_V5: usize = 1;
const RESOURCE_GUARD_AUTH_FD_ENV_V4: &str = "IROHA_RESOURCE_GUARD_AUTH_FD";
const RESOURCE_GUARD_AUTH_TOKEN_ENV_V4: &str = "IROHA_RESOURCE_GUARD_AUTH_TOKEN";
const RESOURCE_GUARD_AUTH_MAGIC_V4: &str = "IROHA_RESOURCE_GUARD_AUTH_V1";
const RESOURCE_GUARD_AUTH_TOKEN_HEX_BYTES_V4: usize = 64;
const RESOURCE_GUARD_AUTH_RECORD_MAX_BYTES_V4: usize = 128;
static KAGEMUSHA_GENERATION_GUARD_CLAIMED_V4: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// One-shot proof that the V4 generator inherited the active resource guard.
///
/// Construction consumes the guard's inherited pipe capability. The value is
/// deliberately neither cloneable nor constructible by callers, so the
/// allocation-heavy generator cannot be invoked through an ordinary library
/// call or a stale environment marker.
#[derive(Debug)]
pub struct KagemushaGenerationSupervisorPermitV4 {
    _not_copy: std::cell::Cell<()>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaGenerationPreflightV4 {
    layout: KagemushaPastaPublicLayoutV4,
    estimated_peak_bytes: u64,
}

fn checked_kagemusha_generation_product_v4(factors: &[u64], role: &str) -> Result<u64, String> {
    factors.iter().try_fold(1_u64, |product, factor| {
        product
            .checked_mul(*factor)
            .ok_or_else(|| format!("Kagemusha V4 generator {role} working-set estimate overflow"))
    })
}

fn estimate_kagemusha_generation_peak_bytes_v4(
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<u64, String> {
    validate_kagemusha_circuit_params_v4(step_eq_circuit_params)?;
    validate_kagemusha_circuit_params_v4(step_ep_circuit_params)?;
    if step_eq_circuit_params.k != step_ep_circuit_params.k {
        return Err("Kagemusha V4 generator Eq/Ep degree mismatch".to_owned());
    }

    let columns = |shape: KagemushaConfiguredVkWireShapeV4, parity: &str| {
        [
            shape.advice_columns,
            shape.base_fixed_columns,
            shape.selectors,
            shape.instance_columns,
        ]
        .into_iter()
        .try_fold(0_u64, |total, count| {
            total
                .checked_add(u64::try_from(count).map_err(|_| {
                    format!("Kagemusha V4 generator {parity} column count does not fit u64")
                })?)
                .ok_or_else(|| format!("Kagemusha V4 generator {parity} column-count overflow"))
        })
    };
    let eq_columns = columns(
        configured_kagemusha_eq_vk_wire_shape_v4(step_eq_circuit_params)?,
        "Eq",
    )?;
    let ep_columns = columns(
        configured_kagemusha_ep_vk_wire_shape_v4(step_ep_circuit_params)?,
        "Ep",
    )?;
    // Every populated Eq circuit is consumed and dropped before an Ep circuit
    // is built (and vice versa), so peak circuit storage is the larger parity,
    // not their sum.
    let staged_columns = eq_columns.max(ep_columns);
    let domain_rows = 1_u64
        .checked_shl(step_eq_circuit_params.k)
        .ok_or_else(|| "Kagemusha V4 generator domain-row estimate overflow".to_owned())?;

    // Halo2 keygen keeps several coefficient/evaluation/permutation forms of
    // each configured column live. Model four field-width copies across the
    // single staged parity inventory. Both ParamsIPA sets remain live because
    // paired recursion needs them, and each keeps two affine point vectors.
    // The consuming quotient evaluator retains product cosets and one staged
    // sigma chunk alongside the processed key, so reserve a separate checked
    // quotient/allocator allowance instead of treating the smaller keygen peak
    // as the complete lifecycle. The external process supervisor remains the
    // authoritative physical-memory ceiling; the fixed headroom also covers
    // evaluator metadata, stacks, allocator fragmentation, and the densest
    // witness-synthesis overlap that is not represented by the raw polynomial
    // counts. This model is the early allocation gate for obviously unsafe
    // authenticated profiles.
    let column_bytes = checked_kagemusha_generation_product_v4(
        &[
            domain_rows,
            staged_columns,
            KAGEMUSHA_GENERATION_FIELD_BYTES_V4,
            KAGEMUSHA_GENERATION_LIVE_COLUMN_COPIES_V4,
        ],
        "column",
    )?;
    let parameter_bytes = checked_kagemusha_generation_product_v4(
        &[
            domain_rows,
            KAGEMUSHA_GENERATION_PARITIES_V4,
            KAGEMUSHA_GENERATION_IPA_POINT_VECTORS_V4,
            KAGEMUSHA_GENERATION_AFFINE_BYTES_V4,
        ],
        "IPA-parameter",
    )?;
    column_bytes
        .checked_add(parameter_bytes)
        .and_then(|bytes| bytes.checked_add(KAGEMUSHA_GENERATION_FIXED_HEADROOM_BYTES_V4))
        .and_then(|bytes| bytes.checked_add(KAGEMUSHA_GENERATION_QUOTIENT_HEADROOM_BYTES_V5))
        .ok_or_else(|| "Kagemusha V4 generator aggregate working-set estimate overflow".to_owned())
}

fn preflight_kagemusha_generation_v4(
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaGenerationPreflightV4, String> {
    let eq_layout = validate_kagemusha_circuit_params_v4(step_eq_circuit_params)?;
    let ep_layout = validate_kagemusha_circuit_params_v4(step_ep_circuit_params)?;
    if eq_layout != ep_layout || step_eq_circuit_params.k != step_ep_circuit_params.k {
        return Err("Kagemusha V4 generator Eq/Ep profile mismatch".to_owned());
    }

    let estimated_peak_bytes = estimate_kagemusha_generation_peak_bytes_v4(
        step_eq_circuit_params,
        step_ep_circuit_params,
    )?;
    if estimated_peak_bytes > KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4 {
        return Err(format!(
            "Kagemusha V4 generator estimated working set {estimated_peak_bytes} bytes exceeds the fixed {}-byte safety ceiling",
            KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4
        ));
    }

    let is_first_release_profile = |params: &KagemushaStepCircuitParamsV4| {
        params.k == KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4
            && params.num_advice_per_phase == KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4
            && params.num_lookup_advice_per_phase == KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4
            && params.num_fixed == KAGEMUSHA_GENERATION_FIXED_COLUMNS_V4
            && params.lookup_bits == params.k - 1
            && params.num_instance_columns == KAGEMUSHA_GENERATION_INSTANCE_COLUMNS_V4
            && params.minimum_unusable_rows == KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4
    };
    if !is_first_release_profile(step_eq_circuit_params)
        || !is_first_release_profile(step_ep_circuit_params)
    {
        return Err(
            "Kagemusha V4 generator profile is wire-valid but not the reviewed first-release key-generation profile"
                .to_owned(),
        );
    }

    // Halo2's processed proving-key encoding contains every degree-n fixed and
    // permutation polynomial. Compute that canonical size from the reviewed
    // ConstraintSystem before `ParamsIPA::new` or key generation: checking the
    // resulting Vec after `ProvingKey::to_bytes` is too late to contain an OOM.
    validate_kagemusha_generation_encoding_sizes_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        step_eq_circuit_params,
        "Eq",
    )?;
    validate_kagemusha_generation_encoding_sizes_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        step_ep_circuit_params,
        "Ep",
    )?;

    Ok(KagemushaGenerationPreflightV4 {
        layout: eq_layout,
        estimated_peak_bytes,
    })
}

fn validate_kagemusha_generation_guard_token_v4(token: &str) -> Result<(), String> {
    if token.len() != RESOURCE_GUARD_AUTH_TOKEN_HEX_BYTES_V4
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("Kagemusha V4 resource-guard token is invalid".to_owned());
    }
    Ok(())
}

fn validate_kagemusha_generation_guard_record_v4(record: &[u8], token: &str) -> Result<(), String> {
    validate_kagemusha_generation_guard_token_v4(token)?;
    let expected = format!("{RESOURCE_GUARD_AUTH_MAGIC_V4}:{token}\n");
    if record != expected.as_bytes() {
        return Err("Kagemusha V4 resource-guard capability is invalid".to_owned());
    }
    Ok(())
}

/// Consume the one-shot capability installed by the guarded V4 launcher.
///
/// The inherited descriptor is accepted only when it is a pipe containing the
/// exact nonce-bound guard record and immediate EOF. It is made nonblocking
/// before the read so malformed direct invocations fail instead of hanging.
pub fn claim_kagemusha_generation_supervisor_permit_v4()
-> Result<KagemushaGenerationSupervisorPermitV4, String> {
    if KAGEMUSHA_GENERATION_GUARD_CLAIMED_V4.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return Err("Kagemusha V4 resource-guard capability was already consumed".to_owned());
    }

    #[cfg(not(unix))]
    {
        Err("Kagemusha V4 guarded generation requires Unix process supervision".to_owned())
    }

    #[cfg(unix)]
    {
        use std::{
            fs::File,
            io::{ErrorKind, Read as _},
            os::unix::fs::FileTypeExt as _,
        };

        let descriptor_text = std::env::var(RESOURCE_GUARD_AUTH_FD_ENV_V4).map_err(|_| {
            "Kagemusha V4 generation must run through scripts/run_kagemusha_v4_generation.py"
                .to_owned()
        })?;
        if descriptor_text.is_empty() || !descriptor_text.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("Kagemusha V4 resource-guard descriptor is invalid".to_owned());
        }
        let descriptor = descriptor_text
            .parse::<i32>()
            .ok()
            .filter(|descriptor| *descriptor >= 3 && descriptor.to_string() == descriptor_text)
            .ok_or_else(|| "Kagemusha V4 resource-guard descriptor is invalid".to_owned())?;
        let token = std::env::var(RESOURCE_GUARD_AUTH_TOKEN_ENV_V4)
            .map_err(|_| "Kagemusha V4 resource-guard token is missing".to_owned())?;
        validate_kagemusha_generation_guard_token_v4(&token)?;

        // Opening `/dev/fd` safely duplicates the inherited pipe without an
        // unsafe raw-descriptor ownership conversion. Reading the duplicate
        // consumes the shared one-shot record; the process-global claim above
        // prevents a second library call from reusing the inherited endpoint.
        let mut capability = File::open(format!("/dev/fd/{descriptor}"))
            .map_err(|_| "Kagemusha V4 resource-guard descriptor is unavailable".to_owned())?;
        if !capability
            .metadata()
            .map_err(|_| "Kagemusha V4 resource-guard descriptor is unavailable".to_owned())?
            .file_type()
            .is_fifo()
        {
            return Err("Kagemusha V4 resource-guard descriptor is not a pipe".to_owned());
        }
        let flags = rustix::fs::fcntl_getfl(&capability).map_err(|_| {
            "Kagemusha V4 resource-guard descriptor flags are unavailable".to_owned()
        })?;
        rustix::fs::fcntl_setfl(&capability, flags | rustix::fs::OFlags::NONBLOCK)
            .map_err(|_| "Kagemusha V4 resource-guard descriptor cannot be bounded".to_owned())?;

        let mut record = Vec::with_capacity(RESOURCE_GUARD_AUTH_RECORD_MAX_BYTES_V4);
        let mut chunk = [0_u8; 64];
        loop {
            match capability.read(&mut chunk) {
                Ok(0) => break,
                Ok(length) => {
                    if record
                        .len()
                        .checked_add(length)
                        .is_none_or(|length| length > RESOURCE_GUARD_AUTH_RECORD_MAX_BYTES_V4)
                    {
                        return Err(
                            "Kagemusha V4 resource-guard capability is oversized".to_owned()
                        );
                    }
                    record.extend_from_slice(&chunk[..length]);
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    return Err("Kagemusha V4 resource-guard capability is incomplete".to_owned());
                }
                Err(_) => {
                    return Err("Kagemusha V4 resource-guard capability is unreadable".to_owned());
                }
            }
        }
        validate_kagemusha_generation_guard_record_v4(&record, &token)?;
        Ok(KagemushaGenerationSupervisorPermitV4 {
            _not_copy: std::cell::Cell::new(()),
        })
    }
}

fn validate_kagemusha_generated_payload_size_v4(
    payload_len: usize,
    role: &str,
) -> Result<(), String> {
    let payload_bytes = u64::try_from(payload_len)
        .map_err(|_| format!("Kagemusha V4 generated {role} length does not fit u64"))?;
    if payload_bytes == 0 || payload_bytes >= KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4 {
        return Err(format!(
            "Kagemusha V4 generated {role} violates the fixed artifact-size corridor"
        ));
    }
    if role.contains("proving key") && payload_bytes > KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5 {
        return Err(format!(
            "Kagemusha V5 generated {role} exceeds the fixed {}-byte proving-key cap",
            KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5
        ));
    }
    if role.contains("parameters") && payload_bytes > KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5 {
        return Err(format!(
            "Kagemusha V5 generated {role} exceeds the fixed {}-byte ParamsIPA cap",
            KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5
        ));
    }
    Ok(())
}

/// A fail-closed writer used while streaming a processed proving key before
/// the owned key enters the consuming proof path.
///
/// The proving-key serializer is intentionally given the caller's final
/// staging sink instead of a `Vec<u8>`.  Counting here keeps the compact V5
/// role cap authoritative even when the sink itself has no size limit.
struct KagemushaBoundedProvingKeyWriterV5<'a> {
    sink: &'a mut dyn std::io::Write,
    written: u64,
}

impl<'a> KagemushaBoundedProvingKeyWriterV5<'a> {
    fn new(sink: &'a mut dyn std::io::Write) -> Self {
        Self { sink, written: 0 }
    }

    fn finish(self, role: &str) -> Result<u64, String> {
        let written = usize::try_from(self.written)
            .map_err(|_| format!("Kagemusha V5 {role} length does not fit usize"))?;
        validate_kagemusha_generated_payload_size_v4(written, role)?;
        Ok(self.written)
    }
}

impl std::io::Write for KagemushaBoundedProvingKeyWriterV5<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let requested = u64::try_from(bytes.len())
            .map_err(|_| std::io::Error::other("Kagemusha V5 PK write length overflow"))?;
        if self
            .written
            .checked_add(requested)
            .is_none_or(|total| total > KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5)
        {
            return Err(std::io::Error::other(
                "Kagemusha V5 processed proving key exceeds its fixed role cap",
            ));
        }
        let count = self.sink.write(bytes)?;
        self.written = self
            .written
            .checked_add(
                u64::try_from(count)
                    .map_err(|_| std::io::Error::other("Kagemusha V5 PK write count overflow"))?,
            )
            .ok_or_else(|| std::io::Error::other("Kagemusha V5 PK byte count overflow"))?;
        Ok(count)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}

/// Convert an authenticated data-model V4 configuration to Halo2's runtime
/// representation.  Callers must obtain `params` from a verified V4 profile;
/// bridge/local configuration inputs are never accepted here.
pub(crate) fn kagemusha_base_circuit_params_v4(
    params: &KagemushaStepCircuitParamsV4,
) -> Result<halo2_base::gates::circuit::BaseCircuitParams, String> {
    validate_kagemusha_circuit_params_v4(params)?;
    let convert = |values: &[u32], role: &str| {
        values
            .iter()
            .map(|value| {
                usize::try_from(*value)
                    .map_err(|_| format!("Kagemusha V4 {role} count does not fit usize"))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(halo2_base::gates::circuit::BaseCircuitParams {
        k: usize::try_from(params.k)
            .map_err(|_| "Kagemusha V4 degree does not fit usize".to_owned())?,
        num_advice_per_phase: convert(&params.num_advice_per_phase, "advice")?,
        num_fixed: usize::try_from(params.num_fixed)
            .map_err(|_| "Kagemusha V4 fixed-column count does not fit usize".to_owned())?,
        num_lookup_advice_per_phase: convert(&params.num_lookup_advice_per_phase, "lookup advice")?,
        lookup_bits: Some(
            usize::try_from(params.lookup_bits)
                .map_err(|_| "Kagemusha V4 lookup width does not fit usize".to_owned())?,
        ),
        num_instance_columns: usize::try_from(params.num_instance_columns)
            .map_err(|_| "Kagemusha V4 instance-column count does not fit usize".to_owned())?,
    })
}

fn kagemusha_usable_rows_v4(params: &KagemushaStepCircuitParamsV4) -> Result<usize, String> {
    validate_kagemusha_circuit_params_v4(params)?;
    let domain_rows = 1_usize
        .checked_shl(params.k)
        .ok_or_else(|| "Kagemusha V4 domain row count does not fit usize".to_owned())?;
    let minimum_unusable_rows = usize::try_from(params.minimum_unusable_rows)
        .map_err(|_| "Kagemusha V4 unusable-row count does not fit usize".to_owned())?;
    domain_rows
        .checked_sub(minimum_unusable_rows)
        .ok_or_else(|| "Kagemusha V4 unusable-row count exceeds its domain".to_owned())
}

type KagemushaBreakPointsV4 = Vec<Vec<usize>>;

#[cfg(test)]
fn kagemusha_break_points_to_wire_v4(break_points: &[Vec<usize>]) -> Result<Vec<Vec<u32>>, String> {
    break_points
        .iter()
        .enumerate()
        .map(|(phase, phase_break_points)| {
            phase_break_points
                .iter()
                .map(|row| {
                    u32::try_from(*row).map_err(|_| {
                        format!("Kagemusha V4 phase {phase} breakpoint row does not fit u32")
                    })
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
fn kagemusha_break_points_from_wire_v4(
    break_points: &[Vec<u32>],
) -> Result<KagemushaBreakPointsV4, String> {
    break_points
        .iter()
        .enumerate()
        .map(|(phase, phase_break_points)| {
            phase_break_points
                .iter()
                .map(|row| {
                    usize::try_from(*row).map_err(|_| {
                        format!("Kagemusha V4 phase {phase} breakpoint row does not fit usize")
                    })
                })
                .collect()
        })
        .collect()
}
/// Exact version of the canonical per-parity bootstrap payload.
pub const KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4: u16 = 5;

fn kagemusha_break_point_max_rows_v5(
    params: &KagemushaStepCircuitParamsV4,
) -> Result<usize, String> {
    let domain_rows = 1_usize
        .checked_shl(params.k)
        .ok_or_else(|| "Kagemusha V5 breakpoint domain size overflows usize".to_owned())?;
    domain_rows
        .checked_sub(
            usize::try_from(params.minimum_unusable_rows)
                .map_err(|_| "Kagemusha V5 unusable rows do not fit usize".to_owned())?,
        )
        .filter(|rows| *rows > 1)
        .ok_or_else(|| "Kagemusha V5 breakpoint domain has no usable rows".to_owned())
}

/// Convert Halo2's per-column row offsets into a canonical, strictly
/// increasing cumulative wire representation.
fn kagemusha_break_points_to_wire_v5(
    break_points: halo2_base::gates::flex_gate::MultiPhaseThreadBreakPoints,
    params: &KagemushaStepCircuitParamsV4,
) -> Result<Vec<Vec<u32>>, String> {
    let max_rows = kagemusha_break_point_max_rows_v5(params)?;
    if break_points.len() != params.num_advice_per_phase.len() {
        return Err("Kagemusha V5 breakpoint phase count mismatch".to_owned());
    }
    let mut wire = Vec::with_capacity(break_points.len());
    for (phase, (points, advice_columns)) in break_points
        .into_iter()
        .zip(&params.num_advice_per_phase)
        .enumerate()
    {
        let maximum_break_points = usize::try_from(*advice_columns)
            .map_err(|_| "Kagemusha V5 advice-column count does not fit usize".to_owned())?
            .saturating_sub(1);
        if points.len() > maximum_break_points {
            return Err(format!(
                "Kagemusha V5 phase {phase} has more breakpoints than advice-column boundaries"
            ));
        }
        let mut cumulative = 0_usize;
        let mut encoded = Vec::with_capacity(points.len());
        for point in points {
            if point == 0 || point >= max_rows {
                return Err(format!(
                    "Kagemusha V5 phase {phase} breakpoint is outside the usable domain"
                ));
            }
            cumulative = cumulative
                .checked_add(point)
                .ok_or_else(|| "Kagemusha V5 cumulative breakpoint overflows usize".to_owned())?;
            encoded.push(
                u32::try_from(cumulative).map_err(|_| {
                    "Kagemusha V5 cumulative breakpoint does not fit u32".to_owned()
                })?,
            );
        }
        wire.push(encoded);
    }
    // Decode once so the accepted wire is guaranteed to round-trip exactly.
    let decoded = kagemusha_break_points_from_wire_v5(&wire, params)?;
    if kagemusha_break_points_to_wire_v5_unchecked(decoded)? != wire {
        return Err("Kagemusha V5 breakpoint encoding is non-canonical".to_owned());
    }
    Ok(wire)
}

fn kagemusha_break_points_to_wire_v5_unchecked(
    break_points: halo2_base::gates::flex_gate::MultiPhaseThreadBreakPoints,
) -> Result<Vec<Vec<u32>>, String> {
    break_points
        .into_iter()
        .map(|points| {
            let mut cumulative = 0_usize;
            points
                .into_iter()
                .map(|point| {
                    cumulative = cumulative.checked_add(point).ok_or_else(|| {
                        "Kagemusha V5 cumulative breakpoint overflows usize".to_owned()
                    })?;
                    u32::try_from(cumulative).map_err(|_| {
                        "Kagemusha V5 cumulative breakpoint does not fit u32".to_owned()
                    })
                })
                .collect()
        })
        .collect()
}

fn kagemusha_break_points_from_wire_v5(
    wire: &[Vec<u32>],
    params: &KagemushaStepCircuitParamsV4,
) -> Result<halo2_base::gates::flex_gate::MultiPhaseThreadBreakPoints, String> {
    let max_rows = kagemusha_break_point_max_rows_v5(params)?;
    if wire.len() != params.num_advice_per_phase.len() {
        return Err("Kagemusha V5 breakpoint phase count mismatch".to_owned());
    }
    wire.iter()
        .zip(&params.num_advice_per_phase)
        .enumerate()
        .map(|(phase, (points, advice_columns))| {
            let maximum_break_points = usize::try_from(*advice_columns)
                .map_err(|_| "Kagemusha V5 advice-column count does not fit usize".to_owned())?
                .saturating_sub(1);
            if points.len() > maximum_break_points {
                return Err(format!(
                    "Kagemusha V5 phase {phase} has more breakpoints than advice-column boundaries"
                ));
            }
            let mut previous = 0_usize;
            points
                .iter()
                .map(|point| {
                    let point = usize::try_from(*point).map_err(|_| {
                        "Kagemusha V5 cumulative breakpoint does not fit usize".to_owned()
                    })?;
                    let delta = point.checked_sub(previous).filter(|delta| *delta > 0).ok_or_else(
                        || {
                            format!(
                                "Kagemusha V5 phase {phase} breakpoints are not strictly increasing"
                            )
                        },
                    )?;
                    if delta >= max_rows {
                        return Err(format!(
                            "Kagemusha V5 phase {phase} breakpoint segment is outside the usable domain"
                        ));
                    }
                    previous = point;
                    Ok(delta)
                })
                .collect()
        })
        .collect()
}

const KAGEMUSHA_RUNTIME_NORITO_MAX_NESTING_DEPTH_V4: usize = 32;

fn kagemusha_runtime_norito_decode_limits_v4(encoded_len: usize) -> norito::core::DecodeLimits {
    // Every variable-length member in the private bootstrap/proof-pair wire is
    // represented inside this exact input. Binding each individual and
    // cumulative budget to the bytes actually supplied prevents a malicious
    // length prefix from reserving release-sized memory before truncation is
    // discovered. Sixteen bytes of cumulative allocation per encoded byte
    // covers both nested sequence storage and each sequence's decoded
    // elements. At the absolute proof-pair ceiling this remains below 349 KiB.
    norito::core::DecodeLimits::new(
        encoded_len,
        encoded_len,
        encoded_len.saturating_mul(2),
        encoded_len.saturating_mul(16),
        KAGEMUSHA_RUNTIME_NORITO_MAX_NESTING_DEPTH_V4,
    )
}

/// One fully parseable parent slot in a canonical V4 bootstrap artifact.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStepBootstrapParentSlotV4 {
    /// Exact one-column public instances, represented as unreduced `u32`
    /// values before conversion into either Pasta scalar field.
    pub instances: Vec<Vec<u32>>,
    /// Ordinary augmented Step proof transcript.
    pub ordinary_proof_bytes: Vec<u8>,
    /// Non-identity carried accumulator used by the always-executed fold.
    pub carried_lineage: KagemushaIpaAccumulatorWireV4,
    /// Complete post-proof fold transcript, present even though the bootstrap
    /// parent's public parent count is zero.
    pub post_proof_fold: KagemushaIpaAccumulationProofV4,
}

/// Canonical, independently authenticated bootstrap artifact for one parity.
///
/// It contains both fixed parent slots and the all-bootstrap branch fold needed
/// by genesis. Any synthesis with a real parent supplies a per-step mixed/real
/// branch fold, which is parsed and verified by the recursive circuit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStepBootstrapV4 {
    /// Exact bootstrap payload version.
    pub version: u16,
    /// Step parity for which the proof and curve encodings are valid.
    pub parity: KagemushaPastaCycleParityV1,
    /// Domain-separated SHA-256 of the exact CircuitParamsV4 payload.
    pub circuit_params_sha256: [u8; 32],
    /// Authenticated value-free compiled-protocol structure identity.
    pub compiled_protocol_structure_sha256: [u8; 32],
    /// Identity of the independently reproducible bootstrap protocol values.
    pub bootstrap_compiled_protocol_sha256: [u8; 32],
    /// Canonical cumulative Halo2 virtual-region breakpoints captured from the
    /// exact keygen circuit. Runtime proof circuits decode these into the
    /// witness-only builder and never reconstruct the constraint graph.
    pub circuit_break_points: Vec<Vec<u32>>,
    /// One manifest-independent all-zero public slot. Both disabled circuit
    /// slots use this exact authenticated payload.
    pub parent_slot: KagemushaStepBootstrapParentSlotV4,
    /// Complete fold transcript for the two canonical bootstrap lineages.
    pub branch_merge_fold: KagemushaIpaAccumulationProofV4,
}

#[derive(Clone, Copy)]
enum KagemushaBootstrapParentValidationV4 {
    Strict,
    ProvisionalPreKeygen,
}

impl KagemushaBootstrapParentValidationV4 {
    fn require_break_points(self, circuit_break_points: &[Vec<u32>]) -> Result<bool, String> {
        match self {
            Self::Strict => Ok(true),
            Self::ProvisionalPreKeygen if circuit_break_points.is_empty() => Ok(false),
            Self::ProvisionalPreKeygen => Err(
                "Kagemusha V5 provisional pre-keygen parent must omit circuit breakpoints"
                    .to_owned(),
            ),
        }
    }
}

impl KagemushaStepBootstrapV4 {
    /// Validate every host-checkable bootstrap invariant against authenticated
    /// circuit parameters. Ordinary/fold equation validity is checked by the
    /// recursive circuit; shape validation never creates substitute bytes.
    pub fn validate(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        self.validate_internal(params, expected_parity, expected_structure_sha256, true)
    }

    fn validate_internal(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
        require_break_points: bool,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        let layout = validate_kagemusha_circuit_params_v4(params)?;
        let params_sha256 = params.sha256().map_err(|error| {
            format!("failed to identify authenticated Kagemusha V4 parameters: {error}")
        })?;
        if self.version != KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4
            || self.parity != expected_parity
            || self.circuit_params_sha256 != params_sha256
            || expected_structure_sha256 == [0; 32]
            || self.compiled_protocol_structure_sha256 != expected_structure_sha256
            || self.bootstrap_compiled_protocol_sha256 == [0; 32]
        {
            return Err("Kagemusha V4 bootstrap header mismatch".to_owned());
        }
        if require_break_points {
            kagemusha_break_points_from_wire_v5(&self.circuit_break_points, params)?;
        } else if !self.circuit_break_points.is_empty() {
            kagemusha_break_points_from_wire_v5(&self.circuit_break_points, params)?;
        }
        self.branch_merge_fold.validate_fixed_transcript(params.k)?;
        let instance_len = usize::try_from(layout.instance_column_limbs)
            .map_err(|_| "Kagemusha V4 bootstrap public length does not fit usize".to_owned())?;
        let accumulator_len = usize::try_from(layout.accumulator_limbs).map_err(|_| {
            "Kagemusha V4 bootstrap accumulator length does not fit usize".to_owned()
        })?;
        let maximum_proof_bytes = usize::try_from(params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 parent-proof bound does not fit usize".to_owned())?;
        let eq_accumulator_offset = usize::try_from(layout.parent_eq_accumulator_offset)
            .map_err(|_| "Kagemusha V4 Eq accumulator offset does not fit usize".to_owned())?;
        let ep_accumulator_offset = usize::try_from(layout.parent_ep_accumulator_offset)
            .map_err(|_| "Kagemusha V4 Ep accumulator offset does not fit usize".to_owned())?;
        let slot = &self.parent_slot;
        if slot.instances.len() != 1
            || slot.instances[0].len() != instance_len
            || slot.instances[0].iter().any(|limb| *limb != 0)
            || slot.ordinary_proof_bytes.len() != maximum_proof_bytes
            || slot.instances[0][eq_accumulator_offset..eq_accumulator_offset + accumulator_len]
                .iter()
                .chain(
                    &slot.instances[0]
                        [ep_accumulator_offset..ep_accumulator_offset + accumulator_len],
                )
                .any(|limb| *limb != 0)
        {
            return Err("Kagemusha V4 bootstrap parent shape mismatch".to_owned());
        }
        slot.post_proof_fold.validate_fixed_transcript(params.k)?;
        match expected_parity {
            KagemushaPastaCycleParityV1::StepEq => {
                slot.carried_lineage.to_eq(params.k)?;
            }
            KagemushaPastaCycleParityV1::StepEp => {
                slot.carried_lineage.to_ep(params.k)?;
            }
        }
        Ok(layout)
    }

    /// Require this payload's bootstrap protocol identities to match a locally
    /// reconstructed protocol under the authenticated V4 profile.
    pub(crate) fn validate_bootstrap_protocol<C>(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
        bootstrap_protocol: &PlonkProtocol<C>,
    ) -> Result<KagemushaPastaPublicLayoutV4, String>
    where
        C: CurveAffine,
        C::ScalarExt: PrimeField,
    {
        let layout = self.validate(params, expected_parity, expected_structure_sha256)?;
        let actual_structure =
            kagemusha_compiled_protocol_structure_sha256(bootstrap_protocol, expected_parity)?;
        let actual_identity =
            kagemusha_compiled_protocol_identity_sha256(bootstrap_protocol, expected_parity)?;
        if actual_structure != expected_structure_sha256
            || actual_identity != self.bootstrap_compiled_protocol_sha256
        {
            return Err("Kagemusha V4 bootstrap protocol identity mismatch".to_owned());
        }
        Ok(layout)
    }

    fn validate_provisional_bootstrap_protocol<C>(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
        bootstrap_protocol: &PlonkProtocol<C>,
    ) -> Result<KagemushaPastaPublicLayoutV4, String>
    where
        C: CurveAffine,
        C::ScalarExt: PrimeField,
    {
        let layout =
            self.validate_internal(params, expected_parity, expected_structure_sha256, false)?;
        let actual_structure =
            kagemusha_compiled_protocol_structure_sha256(bootstrap_protocol, expected_parity)?;
        let actual_identity =
            kagemusha_compiled_protocol_identity_sha256(bootstrap_protocol, expected_parity)?;
        if actual_structure != expected_structure_sha256
            || actual_identity != self.bootstrap_compiled_protocol_sha256
        {
            return Err("Kagemusha V4 bootstrap protocol identity mismatch".to_owned());
        }
        Ok(layout)
    }

    /// Decode one exact canonical bounded bootstrap payload before exposing any
    /// of its recursion witnesses.
    pub(crate) fn decode_authenticated(
        bytes: &[u8],
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
    ) -> Result<Self, String> {
        // A bootstrap contains one augmented proof plus bounded accumulator
        // metadata. It cannot legitimately be larger than the complete Eq/Ep
        // pair accepted by the same release. Do not inherit the much broader
        // generic artifact-file ceiling for this typed payload.
        let maximum = usize::try_from(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
        )
        .map_err(|_| "Kagemusha V4 bootstrap bound does not fit usize".to_owned())?;
        if bytes.is_empty() || bytes.len() > maximum {
            return Err("Kagemusha V4 bootstrap payload length is invalid".to_owned());
        }
        let decoded: Self = norito::decode_canonical_with_limits(
            bytes,
            kagemusha_runtime_norito_decode_limits_v4(bytes.len()),
        )
        .map_err(|error| {
            if matches!(&error, norito::Error::NonCanonicalEncoding) {
                "Kagemusha V4 bootstrap payload is not canonical Norito".to_owned()
            } else {
                format!("failed to decode Kagemusha V4 bootstrap: {error}")
            }
        })?;
        decoded.validate(params, expected_parity, expected_structure_sha256)?;
        Ok(decoded)
    }

    /// Encode one validated bootstrap payload for content-addressed framing.
    pub(crate) fn encode_authenticated(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
    ) -> Result<Vec<u8>, String> {
        self.validate(params, expected_parity, expected_structure_sha256)?;
        norito::encode_canonical(self)
            .map_err(|error| format!("failed to encode Kagemusha V4 bootstrap: {error}"))
    }

    /// Decode one authenticated Eq bootstrap parent for the runtime circuit.
    pub(crate) fn step_eq_parent(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_structure_sha256: [u8; 32],
        slot: usize,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EqAffine>, String>
    {
        self.step_eq_parent_internal(
            params,
            expected_structure_sha256,
            slot,
            KagemushaBootstrapParentValidationV4::Strict,
        )
    }

    fn step_eq_parent_internal(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_structure_sha256: [u8; 32],
        slot: usize,
        validation: KagemushaBootstrapParentValidationV4,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EqAffine>, String>
    {
        self.validate_internal(
            params,
            KagemushaPastaCycleParityV1::StepEq,
            expected_structure_sha256,
            validation.require_break_points(&self.circuit_break_points)?,
        )?;
        if slot >= KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            return Err("Kagemusha V4 Eq bootstrap slot is out of range".to_owned());
        }
        let parent = &self.parent_slot;
        Ok(KagemushaStepParentProofV4 {
            instances: parent
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|limb| Fp::from(u64::from(*limb)))
                        .collect()
                })
                .collect(),
            proof_bytes: parent.ordinary_proof_bytes.clone(),
            carried_lineage: parent.carried_lineage.to_eq(params.k)?,
            external_accumulation_proof: parent.post_proof_fold.clone(),
        })
    }

    /// Decode one authenticated Ep bootstrap parent for the runtime circuit.
    pub(crate) fn step_ep_parent(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_structure_sha256: [u8; 32],
        slot: usize,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EpAffine>, String>
    {
        self.step_ep_parent_internal(
            params,
            expected_structure_sha256,
            slot,
            KagemushaBootstrapParentValidationV4::Strict,
        )
    }

    fn step_ep_parent_internal(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_structure_sha256: [u8; 32],
        slot: usize,
        validation: KagemushaBootstrapParentValidationV4,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EpAffine>, String>
    {
        self.validate_internal(
            params,
            KagemushaPastaCycleParityV1::StepEp,
            expected_structure_sha256,
            validation.require_break_points(&self.circuit_break_points)?,
        )?;
        if slot >= KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            return Err("Kagemusha V4 Ep bootstrap slot is out of range".to_owned());
        }
        let parent = &self.parent_slot;
        Ok(KagemushaStepParentProofV4 {
            instances: parent
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|limb| Fq::from(u64::from(*limb)))
                        .collect()
                })
                .collect(),
            proof_bytes: parent.ordinary_proof_bytes.clone(),
            carried_lineage: parent.carried_lineage.to_ep(params.k)?,
            external_accumulation_proof: parent.post_proof_fold.clone(),
        })
    }
}

/// Validate one canonical V4 bootstrap payload for release tooling.
///
/// This public helper exposes no recursion witness. It returns only the exact
/// authenticated ordinary-proof byte count so bundle generation can bind its
/// measured profile without duplicating the private bootstrap wire schema.
pub fn validate_kagemusha_step_bootstrap_payload_v4(
    bytes: &[u8],
    params: &KagemushaStepCircuitParamsV4,
    parity: KagemushaPastaCycleParityV1,
    expected_structure_sha256: [u8; 32],
) -> Result<usize, String> {
    let bootstrap = KagemushaStepBootstrapV4::decode_authenticated(
        bytes,
        params,
        parity,
        expected_structure_sha256,
    )?;
    Ok(bootstrap.parent_slot.ordinary_proof_bytes.len())
}

/// Return the first public limb of one fixed parent-state slot.
#[must_use]
pub const fn kagemusha_pasta_parent_state_offset_v4(parent_slot: usize) -> usize {
    assert!(parent_slot < KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
    KAGEMUSHA_PASTA_PARENT_STATES_OFFSET_V4 + parent_slot * KAGEMUSHA_PASTA_STATE_STRIDE_V4
}

/// Version of the compiled parent-protocol identity bound inside both halves.
pub const KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1: u32 = 1;
/// Pinned `snark-verifier` revision whose `PlonkProtocol` layout and private
/// enum encodings define the explicit V1 structural descriptor.
pub const KAGEMUSHA_SNARK_VERIFIER_PROTOCOL_REVISION_V1: &str =
    "bbfcc721d714bea0d44a27c8fc6c4736e73ca853";
/// Domain separator for the fixed, value-free compiled-protocol descriptor.
pub const KAGEMUSHA_COMPILED_PROTOCOL_STRUCTURE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:compiled-protocol-structure:v1";
/// Domain separator for the authenticated compiled-protocol identity.
pub const KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:compiled-protocol-identity:v1";

fn protocol_parity_tag(parity: KagemushaPastaCycleParityV1) -> u32 {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => 1,
        KagemushaPastaCycleParityV1::StepEp => 2,
    }
}

fn append_len(output: &mut Vec<u8>, len: usize, label: &str) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(len)
            .map_err(|_| format!("Kagemusha {label} length does not fit u32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_index(output: &mut Vec<u8>, value: usize, label: &str) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| format!("Kagemusha {label} does not fit u32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_scalar_repr<F: PrimeField>(output: &mut Vec<u8>, scalar: F) -> Result<(), String> {
    let repr = scalar.to_repr();
    if repr.as_ref().len() != 32 {
        return Err("Kagemusha compiled protocol scalar is not 32 bytes".to_owned());
    }
    output.extend_from_slice(repr.as_ref());
    Ok(())
}

fn expression_unary_node(tag: u8, child: Result<Vec<u8>, String>) -> Result<Vec<u8>, String> {
    let child = child?;
    let mut encoded = vec![tag];
    append_len(&mut encoded, child.len(), "expression child")?;
    encoded.extend_from_slice(&child);
    Ok(encoded)
}

fn expression_binary_node(
    tag: u8,
    left: Result<Vec<u8>, String>,
    right: Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let left = left?;
    let right = right?;
    let mut encoded = vec![tag];
    append_len(&mut encoded, left.len(), "left expression child")?;
    encoded.extend_from_slice(&left);
    append_len(&mut encoded, right.len(), "right expression child")?;
    encoded.extend_from_slice(&right);
    Ok(encoded)
}

fn encode_common_polynomial_value(value: ciborium::value::Value) -> Result<Vec<u8>, String> {
    match value {
        ciborium::value::Value::Text(variant) if variant == "Identity" => Ok(vec![1, 0]),
        ciborium::value::Value::Map(mut fields) if fields.len() == 1 => {
            let (variant, rotation) = fields.pop().expect("one checked enum field");
            let ciborium::value::Value::Text(variant) = variant else {
                return Err("Kagemusha common-polynomial variant is not text".to_owned());
            };
            if variant != "Lagrange" {
                return Err(format!(
                    "unsupported Kagemusha common-polynomial variant `{variant}`"
                ));
            }
            let ciborium::value::Value::Integer(rotation) = rotation else {
                return Err("Kagemusha Lagrange rotation is not an integer".to_owned());
            };
            let rotation = i32::try_from(rotation)
                .map_err(|_| "Kagemusha Lagrange rotation does not fit i32".to_owned())?;
            let mut encoded = vec![1, 1];
            encoded.extend_from_slice(&rotation.to_le_bytes());
            Ok(encoded)
        }
        _ => Err("unsupported Kagemusha common-polynomial encoding".to_owned()),
    }
}

fn encode_linearization_value(value: ciborium::value::Value) -> Result<u8, String> {
    match value {
        ciborium::value::Value::Null => Ok(0),
        ciborium::value::Value::Text(variant) if variant == "WithoutConstant" => Ok(1),
        ciborium::value::Value::Text(variant) if variant == "MinusVanishingTimesQuotient" => Ok(2),
        _ => Err("unsupported Kagemusha linearization encoding".to_owned()),
    }
}

fn append_compressed_point<C: CurveAffine>(output: &mut Vec<u8>, point: C) -> Result<(), String> {
    let encoding = point.to_bytes();
    if encoding.as_ref().len() != 32 {
        return Err("Kagemusha compiled protocol point is not 32 bytes".to_owned());
    }
    output.extend_from_slice(encoding.as_ref());
    Ok(())
}

/// Return the exact fixed descriptor of a compiled parent protocol.
///
/// The descriptor deliberately excludes only the self-referential verifier-key
/// commitments and transcript initial state.  It includes every verifier
/// control-flow field, quotient expression, and instance-commitment key.  Its
/// digest can therefore be fixed before the final self key is known, while the
/// excluded values are witness-loaded and constrained by the identity below.
pub fn kagemusha_compiled_protocol_structure_sha256<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    if protocol.domain_as_witness.is_some() {
        return Err("native Kagemusha protocol unexpectedly has a witness domain".to_owned());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(KAGEMUSHA_COMPILED_PROTOCOL_STRUCTURE_DOMAIN_V1);
    bytes.push(0);
    bytes.extend_from_slice(&KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(KAGEMUSHA_SNARK_VERIFIER_PROTOCOL_REVISION_V1.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&protocol_parity_tag(parity).to_le_bytes());

    append_index(&mut bytes, protocol.domain.k, "domain k")?;
    append_index(&mut bytes, protocol.domain.n, "domain n")?;
    append_scalar_repr(&mut bytes, protocol.domain.n_inv)?;
    append_scalar_repr(&mut bytes, protocol.domain.r#gen)?;
    append_scalar_repr(&mut bytes, protocol.domain.gen_inv)?;

    // Only the count belongs to the fixed structure. The self-referential
    // preprocessed values are authenticated separately by the identity below.
    append_len(
        &mut bytes,
        protocol.preprocessed.len(),
        "preprocessed point count",
    )?;
    for (label, values) in [
        ("instance column count", &protocol.num_instance),
        ("witness phase count", &protocol.num_witness),
        ("challenge phase count", &protocol.num_challenge),
    ] {
        append_len(&mut bytes, values.len(), label)?;
        for value in values {
            append_index(&mut bytes, *value, label)?;
        }
    }

    for (label, queries) in [
        ("evaluation query count", &protocol.evaluations),
        ("PCS query count", &protocol.queries),
    ] {
        append_len(&mut bytes, queries.len(), label)?;
        for query in queries {
            append_index(&mut bytes, query.poly, "query polynomial index")?;
            bytes.extend_from_slice(&query.rotation.0.to_le_bytes());
        }
    }

    append_index(
        &mut bytes,
        protocol.quotient.chunk_degree,
        "quotient chunk degree",
    )?;
    // `Expression::evaluate` is the pinned verifier's own exhaustive recursive
    // visitor. It canonicalizes `DistributePowers` to the same sum/product
    // operations used during verification, while retaining every scalar,
    // polynomial, common-polynomial, challenge, unary, binary, and scale node.
    let numerator = protocol.quotient.numerator.evaluate(
        &|scalar| {
            let mut encoded = vec![0];
            append_scalar_repr(&mut encoded, scalar)?;
            Ok(encoded)
        },
        &|common_polynomial| {
            let value =
                ciborium::value::Value::serialized(&common_polynomial).map_err(|error| {
                    format!("failed to inspect Kagemusha common polynomial: {error}")
                })?;
            encode_common_polynomial_value(value)
        },
        &|query| {
            let mut encoded = vec![2];
            append_index(&mut encoded, query.poly, "expression polynomial index")?;
            encoded.extend_from_slice(&query.rotation.0.to_le_bytes());
            Ok(encoded)
        },
        &|challenge| {
            let mut encoded = vec![3];
            append_index(&mut encoded, challenge, "expression challenge index")?;
            Ok(encoded)
        },
        &|child| expression_unary_node(4, child),
        &|left, right| expression_binary_node(5, left, right),
        &|left, right| expression_binary_node(6, left, right),
        &|child, scalar| {
            let mut encoded = expression_unary_node(7, child)?;
            append_scalar_repr(&mut encoded, scalar)?;
            Ok(encoded)
        },
    )?;
    append_len(&mut bytes, numerator.len(), "quotient numerator")?;
    bytes.extend_from_slice(&numerator);

    // Presence, not the self-referential value, is part of the fixed shape.
    bytes.push(u8::from(protocol.transcript_initial_state.is_some()));
    match &protocol.instance_committing_key {
        Some(key) => {
            bytes.push(1);
            append_len(
                &mut bytes,
                key.bases.len(),
                "instance committing-key base count",
            )?;
            for base in &key.bases {
                append_compressed_point(&mut bytes, *base)?;
            }
            match key.constant {
                Some(constant) => {
                    bytes.push(1);
                    append_compressed_point(&mut bytes, constant)?;
                }
                None => bytes.push(0),
            }
        }
        None => bytes.push(0),
    }

    let linearization = ciborium::value::Value::serialized(&protocol.linearization)
        .map_err(|error| format!("failed to inspect Kagemusha linearization: {error}"))?;
    bytes.push(encode_linearization_value(linearization)?);

    append_len(
        &mut bytes,
        protocol.accumulator_indices.len(),
        "accumulator column count",
    )?;
    for column in &protocol.accumulator_indices {
        append_len(&mut bytes, column.len(), "accumulator index count")?;
        for (column, row) in column {
            append_index(&mut bytes, *column, "accumulator column index")?;
            append_index(&mut bytes, *row, "accumulator row index")?;
        }
    }
    Ok(Sha256::digest(bytes).into())
}

fn kagemusha_compiled_protocol_identity_preimage<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<Vec<u8>, String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    let structure = kagemusha_compiled_protocol_structure_sha256(protocol, parity)?;
    let transcript_initial_state = protocol
        .transcript_initial_state
        .ok_or_else(|| "Kagemusha compiled protocol has no transcript initial state".to_owned())?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1);
    bytes.push(0);
    bytes.extend_from_slice(&KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&protocol_parity_tag(parity).to_le_bytes());
    bytes.extend_from_slice(&structure);
    bytes.extend_from_slice(
        &u32::try_from(protocol.preprocessed.len())
            .map_err(|_| "Kagemusha preprocessed point count does not fit u32".to_owned())?
            .to_le_bytes(),
    );
    for point in &protocol.preprocessed {
        append_compressed_point(&mut bytes, *point)?;
    }
    bytes.extend_from_slice(transcript_initial_state.to_repr().as_ref());
    Ok(bytes)
}

/// Derive the release-authenticated identity of one exact compiled protocol.
///
/// Terminal verification computes this value from the authenticated Params/VK
/// artifacts.  Recursive circuits independently hash the same preimage from
/// witness-loaded preprocessed points and transcript state.
pub fn kagemusha_compiled_protocol_identity_sha256<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    Ok(
        Sha256::digest(kagemusha_compiled_protocol_identity_preimage(
            protocol, parity,
        )?)
        .into(),
    )
}

/// Convert a standard SHA-256 digest to the eight public big-endian words used
/// by the constrained SHA gadget.
#[must_use]
pub fn kagemusha_sha256_public_words(digest: [u8; 32]) -> [u32; 8] {
    std::array::from_fn(|index| {
        u32::from_be_bytes(
            digest[index * 4..index * 4 + 4]
                .try_into()
                .expect("SHA-256 word has four bytes"),
        )
    })
}

/// Preserve one exact 32-byte wire value as eight little-endian `u32` limbs.
///
/// This is deliberately distinct from [`kagemusha_sha256_public_words`]: the
/// constrained SHA gadget exposes big-endian digest words, while manifest and
/// state-vector bindings carry their original bytes without reinterpreting the
/// wire encoding.
#[must_use]
fn kagemusha_exact_u32_public_limbs(bytes: [u8; 32]) -> [u32; 8] {
    std::array::from_fn(|index| {
        u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("exact 32-byte value has eight four-byte limbs"),
        )
    })
}

fn kagemusha_u32_words_to_u128_chunks_v5(words: &[u32; 8]) -> [u128; 2] {
    std::array::from_fn(|chunk| {
        words[chunk * 4..chunk * 4 + 4]
            .iter()
            .enumerate()
            .fold(0_u128, |value, (index, word)| {
                value | (u128::from(*word) << (index * 32))
            })
    })
}

fn kagemusha_bytes_to_u128_chunks_v5(bytes: [u8; 32]) -> [u128; 2] {
    std::array::from_fn(|index| {
        u128::from_le_bytes(
            bytes[index * 16..index * 16 + 16]
                .try_into()
                .expect("32-byte value has two exact chunks"),
        )
    })
}

fn kagemusha_pack_u32_limbs_for_poseidon_v5(limbs: &[u32]) -> Vec<Fp> {
    // Seven limbs occupy 224 bits and therefore cannot wrap Pasta Fp. Packing
    // before the sponge cuts the permutation count without dropping a bit.
    limbs
        .chunks(7)
        .map(|chunk| {
            let radix = Fp::from(1_u64 << 32);
            let mut weight = Fp::ONE;
            let mut packed = Fp::ZERO;
            for limb in chunk {
                packed += Fp::from(u64::from(*limb)) * weight;
                weight *= radix;
            }
            packed
        })
        .collect()
}

fn kagemusha_poseidon_commitment_chunks_v5(domain: u64, limbs: &[u32]) -> [u128; 2] {
    let packed = kagemusha_pack_u32_limbs_for_poseidon_v5(limbs);
    let commitment = super::confidential_v2::confidential_poseidon_hash_v3::<Fp>(domain, &packed);
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(commitment.to_repr().as_ref());
    kagemusha_bytes_to_u128_chunks_v5(bytes)
}

/// Canonical V4 recursive-verifier compilation profile.
///
/// Querying the public instance polynomial through an IPA commitment expands
/// every public limb into a fixed-base MSM inside the recursive verifier. The
/// V4 public column contains thousands of limbs, so the split scalar/point
/// audit would otherwise serialize thousands of fixed bases. The pinned
/// verifier supports the equivalent direct Lagrange-evaluation path when
/// queried instances are disabled.
fn kagemusha_ipa_compile_config_v4(public_len: usize) -> snark_verifier::system::halo2::Config {
    snark_verifier::system::halo2::Config::ipa()
        .set_query_instance(false)
        .with_num_instance(vec![public_len])
}

/// IPA multi-open prover matching [`kagemusha_ipa_compile_config_v4`].
///
/// The pinned Halo2 `ProverIPA` implementation hard-codes queried instances.
/// Delegating the opening proof while overriding this associated constant keeps
/// Halo2's proof transcript aligned with snark-verifier's direct-instance
/// protocol without forking the cryptographic implementation.
#[derive(Debug)]
struct KagemushaDirectInstanceProverIpa<'params, C: halo2_proofs::halo2curves::CurveAffine>(
    halo2_proofs::poly::ipa::multiopen::ProverIPA<'params, C>,
);

impl<'params, C>
    halo2_proofs::poly::commitment::Prover<
        'params,
        halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
    > for KagemushaDirectInstanceProverIpa<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    const QUERY_INSTANCE: bool = false;

    fn new(params: &'params halo2_proofs::poly::ipa::commitment::ParamsIPA<C>) -> Self {
        Self(<
            halo2_proofs::poly::ipa::multiopen::ProverIPA<'params, C>
            as halo2_proofs::poly::commitment::Prover<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::new(params))
    }

    fn create_proof<'com, E, T, R, I>(
        &self,
        rng: R,
        transcript: &mut T,
        queries: I,
    ) -> std::io::Result<()>
    where
        E: halo2_proofs::transcript::EncodedChallenge<C>,
        T: halo2_proofs::transcript::TranscriptWrite<C, E>,
        R: rand_core_06::RngCore,
        I: IntoIterator<Item = halo2_proofs::poly::ProverQuery<'com, C>> + Clone,
    {
        <
            halo2_proofs::poly::ipa::multiopen::ProverIPA<'params, C>
            as halo2_proofs::poly::commitment::Prover<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::create_proof(&self.0, rng, transcript, queries)
    }
}

/// IPA multi-open verifier matching [`KagemushaDirectInstanceProverIpa`].
#[derive(Debug)]
struct KagemushaDirectInstanceVerifierIpa<'params, C: halo2_proofs::halo2curves::CurveAffine>(
    halo2_proofs::poly::ipa::multiopen::VerifierIPA<'params, C>,
);

impl<'params, C>
    halo2_proofs::poly::commitment::Verifier<
        'params,
        halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
    > for KagemushaDirectInstanceVerifierIpa<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    type Guard = halo2_proofs::poly::ipa::strategy::GuardIPA<'params, C>;
    type MSMAccumulator = halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>;

    const QUERY_INSTANCE: bool = false;

    fn new(params: &'params halo2_proofs::poly::ipa::commitment::ParamsVerifierIPA<C>) -> Self {
        Self(<
            halo2_proofs::poly::ipa::multiopen::VerifierIPA<'params, C>
            as halo2_proofs::poly::commitment::Verifier<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::new(params))
    }

    fn verify_proof<'com, E, T, I>(
        &self,
        transcript: &mut T,
        queries: I,
        msm: Self::MSMAccumulator,
    ) -> Result<Self::Guard, halo2_proofs::poly::Error>
    where
        'params: 'com,
        E: halo2_proofs::transcript::EncodedChallenge<C>,
        T: halo2_proofs::transcript::TranscriptRead<C, E>,
        I: IntoIterator<
                Item = halo2_proofs::poly::VerifierQuery<
                    'com,
                    C,
                    halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>,
                >,
            > + Clone,
    {
        <
            halo2_proofs::poly::ipa::multiopen::VerifierIPA<'params, C>
            as halo2_proofs::poly::commitment::Verifier<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::verify_proof(&self.0, transcript, queries, msm)
    }
}

/// Single-proof strategy for the direct-instance IPA verifier.
///
/// snark-verifier's otherwise-equivalent strategy is implemented specifically
/// for Halo2's queried-instance `VerifierIPA`, so the local verifier needs the
/// same final MSM decision under its own type.
#[derive(Debug)]
struct KagemushaDirectInstanceSingleStrategy<'params, C: halo2_proofs::halo2curves::CurveAffine> {
    msm: halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>,
}

impl<'params, C> KagemushaDirectInstanceSingleStrategy<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    fn from_params(params: &'params halo2_proofs::poly::ipa::commitment::ParamsIPA<C>) -> Self {
        Self {
            msm: halo2_proofs::poly::ipa::msm::MSMIPA::new(params),
        }
    }
}

impl<'params, C>
    halo2_proofs::poly::VerificationStrategy<
        'params,
        halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
        KagemushaDirectInstanceVerifierIpa<'params, C>,
    > for KagemushaDirectInstanceSingleStrategy<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    type Output = C;

    fn new(params: &'params halo2_proofs::poly::ipa::commitment::ParamsIPA<C>) -> Self {
        Self::from_params(params)
    }

    fn process(
        self,
        verify: impl FnOnce(
            halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>,
        ) -> Result<
            halo2_proofs::poly::ipa::strategy::GuardIPA<'params, C>,
            halo2_proofs::plonk::Error,
        >,
    ) -> Result<Self::Output, halo2_proofs::plonk::Error> {
        use halo2_proofs::poly::commitment::MSM as _;

        let guard = verify(self.msm)?;
        let folded_generator = guard.compute_g();
        let (msm, _) = guard.use_g(folded_generator);
        if msm.check() {
            Ok(folded_generator)
        } else {
            Err(halo2_proofs::plonk::Error::ConstraintSystemFailure)
        }
    }

    fn finalize(self) -> bool {
        unreachable!("Kagemusha single-proof strategy decides in process")
    }
}

/// Deterministic universal target used to break the remaining self-protocol
/// shape cycle during artifact generation.
///
/// `BaseConfig` fixes the complete Halo2 constraint-system structure from
/// `BaseCircuitParams`; virtual arithmetic performed during synthesis changes
/// fixed/preprocessed *values* but not the PLONK query graph.  Artifact
/// generation therefore compiles this empty bootstrap circuit first, preserves
/// that protocol structure in `without_witnesses`, generates the real Step key,
/// recompiles it, and requires the two structure digests to match exactly.
#[derive(Clone, Debug)]
pub struct KagemushaUniversalProtocolTargetV1 {
    /// Exact release `BaseConfig`, shared by bootstrap and final Step circuit.
    pub base_circuit_params: halo2_base::gates::circuit::BaseCircuitParams,
    /// Exact instance-column lengths supplied to `snark-verifier::compile`.
    pub instance_column_lengths: Vec<usize>,
}

impl KagemushaUniversalProtocolTargetV1 {
    /// Reject a target that cannot describe the one-column Kagemusha Step ABI.
    pub fn validate(&self) -> Result<(), String> {
        if self.base_circuit_params.k == 0
            || self.base_circuit_params.num_instance_columns != 1
            || self.instance_column_lengths.len() != 1
            || self.instance_column_lengths[0] == 0
        {
            return Err("Kagemusha universal protocol target shape mismatch".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct KagemushaProtocolBootstrapCircuit<F>
where
    F: halo2_base::utils::ScalarField,
{
    params: halo2_base::gates::circuit::BaseCircuitParams,
    marker: std::marker::PhantomData<F>,
}

impl<F> halo2_proofs::plonk::Circuit<F> for KagemushaProtocolBootstrapCircuit<F>
where
    F: halo2_base::utils::ScalarField,
{
    type Config = halo2_base::gates::circuit::BaseConfig<F>;
    type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
    type Params = halo2_base::gates::circuit::BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<F>,
        params: Self::Params,
    ) -> Self::Config {
        halo2_base::gates::circuit::BaseConfig::configure(meta, params)
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<F>) -> Self::Config {
        unreachable!("Kagemusha bootstrap requires circuit params")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl halo2_proofs::circuit::Layouter<F>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let builder = halo2_base::gates::circuit::builder::BaseCircuitBuilder::<F>::new(false)
            .use_params(self.params.clone());
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<F> as halo2_proofs::plonk::Circuit<
            F,
        >>::synthesize(&builder, config, layouter)
    }
}

#[cfg(test)]
fn kagemusha_bootstrap_verifying_key_v1<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    target: &KagemushaUniversalProtocolTargetV1,
) -> Result<halo2_proofs::plonk::VerifyingKey<C>, String>
where
    C: CurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField,
{
    use halo2_proofs::poly::commitment::Params as _;

    target.validate()?;
    if usize::try_from(params.k()).ok() != Some(target.base_circuit_params.k) {
        return Err("Kagemusha bootstrap Params degree does not match BaseConfig".to_owned());
    }
    let circuit = KagemushaProtocolBootstrapCircuit::<C::ScalarExt> {
        params: target.base_circuit_params.clone(),
        marker: std::marker::PhantomData,
    };
    halo2_proofs::plonk::keygen_vk(params, &circuit)
        .map_err(|error| format!("failed to generate Kagemusha bootstrap VK: {error}"))
}

fn kagemusha_bootstrap_proving_key_v1<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    target: &KagemushaUniversalProtocolTargetV1,
    circuit: &KagemushaProtocolBootstrapCircuit<C::ScalarExt>,
) -> Result<halo2_proofs::plonk::ProvingKey<C>, String>
where
    C: CurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField,
{
    use halo2_proofs::poly::commitment::Params as _;

    target.validate()?;
    let expected = &target.base_circuit_params;
    let actual = &circuit.params;
    if usize::try_from(params.k()).ok() != Some(expected.k)
        || actual.k != expected.k
        || actual.num_advice_per_phase != expected.num_advice_per_phase
        || actual.num_lookup_advice_per_phase != expected.num_lookup_advice_per_phase
        || actual.num_fixed != expected.num_fixed
        || actual.lookup_bits != expected.lookup_bits
        || actual.num_instance_columns != expected.num_instance_columns
    {
        return Err("Kagemusha bootstrap Params/circuit does not match BaseConfig".to_owned());
    }
    // The bootstrap VK and PK describe the same empty, production-shaped
    // circuit. Building them in one synthesis preserves the exact key bytes
    // while avoiding a complete first keygen pass and its retained allocator
    // pages immediately before the memory-critical bootstrap proof.
    halo2_proofs::plonk::keygen_pk2(params, circuit, false)
        .map_err(|error| format!("failed to generate Kagemusha bootstrap PK: {error}"))
}

/// Compile the deterministic bootstrap protocol whose structure is retained
/// by a self-recursive Step circuit during key generation.
///
/// The protocol values belong only to the canonical all-zero bootstrap proof;
/// they are never substituted for the final Step protocol. After the final
/// Step VK exists, callers compare structure hashes with
/// [`kagemusha_require_protocol_structure_v1`] and authenticate both protocol
/// identities independently.
#[cfg(test)]
pub fn kagemusha_bootstrap_compiled_protocol_v1<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    target: &KagemushaUniversalProtocolTargetV1,
) -> Result<PlonkProtocol<C>, String>
where
    C: CurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField,
{
    let verifying_key = kagemusha_bootstrap_verifying_key_v1(params, target)?;
    Ok(snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(target.instance_column_lengths[0]),
    ))
}

/// Require a final self protocol to converge to the deterministic bootstrap
/// structure.  A mismatch is an artifact-generation failure, never a reason to
/// alter the target at runtime.
pub fn kagemusha_require_protocol_structure_v1<C>(
    bootstrap: &PlonkProtocol<C>,
    final_protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    let expected = kagemusha_compiled_protocol_structure_sha256(bootstrap, parity)?;
    let actual = kagemusha_compiled_protocol_structure_sha256(final_protocol, parity)?;
    if actual != expected {
        return Err("Kagemusha final protocol did not converge to bootstrap structure".to_owned());
    }
    Ok(actual)
}

/// Internal semantic boundary shared by both authoritative V4 parities.
struct KagemushaSemanticBoundaryV4 {
    /// Canonical public-statement digest as eight unreduced little-endian limbs.
    pub public_statement_digest: [u32; 8],
    /// Number of consumed parent proof pairs.
    pub parent_count: u32,
    /// Complete ordered parent result states with exact zero padding.
    pub parent_states: [Vec<u32>; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Complete state resulting from the current transition.
    pub result_state: Vec<u32>,
    /// Authenticated artifact-manifest SHA-256 as eight unreduced limbs.
    pub manifest_sha256: [u32; 8],
    /// SHA-256 joins for the Eq parent's scalar and point verifier halves.
    pub parent_eq_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// SHA-256 joins for the Ep parent's scalar and point verifier halves.
    pub parent_ep_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
}

impl KagemushaSemanticBoundaryV4 {
    fn validate_with_parent_state_order(
        &self,
        proof_step_count: u32,
        require_lexicographic_parent_state_order: bool,
        require_deferred_audit_joins: bool,
    ) -> Result<(), String> {
        use super::kagemusha_v2::KagemushaRecursiveSpendStateVectorV2;
        use iroha_data_model::offline::{
            KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2,
        };

        let parent_count = usize::try_from(self.parent_count)
            .map_err(|_| "Kagemusha parent count does not fit usize".to_owned())?;
        let initializing = proof_step_count == 1;
        if proof_step_count == 0
            || self.public_statement_digest == [0; 8]
            || self.manifest_sha256 == [0; 8]
            || parent_count > KAGEMUSHA_PASTA_PARENT_SLOTS_V1
            || initializing != (parent_count == 0)
            || (!initializing && parent_count == 0)
            || self
                .parent_states
                .iter()
                .any(|state| state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2)
            || self.result_state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2
            || self.result_state.first().copied()
                != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2)
        {
            return Err("Kagemusha exact-state public-instance shape mismatch".to_owned());
        }
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            let present = slot < parent_count;
            let state = &self.parent_states[slot];
            let eq_digest = self.parent_eq_deferred_sha256[slot];
            let ep_digest = self.parent_ep_deferred_sha256[slot];
            if present {
                if state.first().copied()
                    != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2)
                    || state == &self.result_state
                    || if require_deferred_audit_joins {
                        eq_digest == [0; 8] || ep_digest == [0; 8] || eq_digest == ep_digest
                    } else {
                        eq_digest != [0; 8] || ep_digest != [0; 8]
                    }
                {
                    return Err("Kagemusha present parent slot is invalid".to_owned());
                }
            } else if state.iter().any(|limb| *limb != 0)
                || eq_digest != [0; 8]
                || ep_digest != [0; 8]
            {
                return Err("Kagemusha absent parent slot has non-zero padding".to_owned());
            }
        }
        if require_lexicographic_parent_state_order
            && parent_count == KAGEMUSHA_PASTA_PARENT_SLOTS_V1
            && self.parent_states[0] >= self.parent_states[1]
        {
            return Err("Kagemusha parent states are not in canonical order".to_owned());
        }
        let result_vector = KagemushaRecursiveSpendStateVectorV2 {
            limbs: self
                .result_state
                .clone()
                .try_into()
                .map_err(|_| "Kagemusha result state has the wrong length".to_owned())?,
        };
        if result_vector.proof_step_count() != proof_step_count
            || result_vector.peer_hop_count() > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || result_vector.manifest_sha256_limbs() != self.manifest_sha256
        {
            return Err("Kagemusha result-state counters or manifest mismatch".to_owned());
        }
        let mut maximum_parent_step = 0_u32;
        let mut maximum_parent_hop = 0_u32;
        for state in self.parent_states.iter().take(parent_count) {
            let vector = KagemushaRecursiveSpendStateVectorV2 {
                limbs: state
                    .clone()
                    .try_into()
                    .map_err(|_| "Kagemusha parent state has the wrong length".to_owned())?,
            };
            let parent_step = vector.proof_step_count();
            let parent_hop = vector.peer_hop_count();
            if parent_step == 0
                || parent_step >= proof_step_count
                || parent_hop > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
                || vector.manifest_sha256_limbs() != self.manifest_sha256
            {
                return Err("Kagemusha parent-state counters or manifest mismatch".to_owned());
            }
            maximum_parent_step = maximum_parent_step.max(parent_step);
            maximum_parent_hop = maximum_parent_hop.max(parent_hop);
        }
        if initializing {
            if result_vector.peer_hop_count() != 0 {
                return Err("Kagemusha initialization state has a peer hop".to_owned());
            }
        } else if maximum_parent_step.checked_add(1) != Some(proof_step_count)
            || !matches!(
                result_vector
                    .peer_hop_count()
                    .checked_sub(maximum_parent_hop),
                Some(0 | 1)
            )
        {
            return Err("Kagemusha parent/result step or hop relation mismatch".to_owned());
        }
        Ok(())
    }
}

/// Degree-parameterized V4 public inputs used by both concrete Step circuits.
///
/// The semantic prefix is fixed for ABI-21. Only the two IPA accumulator
/// slices are dynamic, and their exact offsets are derived from the separately
/// authenticated [`KagemushaStepCircuitParamsV4`].
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaPastaCyclePublicInputsV4 {
    /// Canonical public-statement digest as eight unreduced limbs.
    pub public_statement_digest: [u32; 8],
    /// Exact canonical operation row shared by both Step parities.
    pub operation: KagemushaStepOperationVectorV4,
    /// Number of consumed parent proof pairs.
    pub parent_count: u32,
    /// Complete ordered parent result states with exact zero padding.
    pub parent_states: [Vec<u32>; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Complete state resulting from this transition.
    pub result_state: Vec<u32>,
    /// Authenticated artifact-manifest SHA-256.
    pub manifest_sha256: [u32; 8],
    /// SHA-256 identity of the exact compiled Eq parent protocol.
    pub step_eq_compiled_protocol_sha256: [u32; 8],
    /// SHA-256 identity of the exact compiled Ep parent protocol.
    pub step_ep_compiled_protocol_sha256: [u32; 8],
    /// Complete Eq parent lineage, absent only at initialization.
    pub parent_eq_lineage_accumulator: Option<KagemushaIpaAccumulatorWireV4>,
    /// Complete Ep parent lineage, absent only at initialization.
    pub parent_ep_lineage_accumulator: Option<KagemushaIpaAccumulatorWireV4>,
    /// Eq scalar/point audit joins for the two fixed parent slots.
    pub parent_eq_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Ep scalar/point audit joins for the two fixed parent slots.
    pub parent_ep_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Explicit circuit mode. Public proof pairs accept only `1` (live); the
    /// adapter alone uses `0` for its authenticated all-zero bootstrap proof.
    pub live_selector: u32,
}

impl KagemushaPastaCyclePublicInputsV4 {
    /// Validate the semantic state boundary and its degree-specific lineage.
    pub fn validate(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        self.validate_with_deferred_audit_joins(proof_step_count, params, true)
    }

    fn validate_for_audit_derivation_prepass(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        self.validate_with_deferred_audit_joins(proof_step_count, params, false)
    }

    fn validate_with_deferred_audit_joins(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
        require_deferred_audit_joins: bool,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        let layout = validate_kagemusha_circuit_params_v4(params)?;
        KagemushaSemanticBoundaryV4 {
            public_statement_digest: self.public_statement_digest,
            parent_count: self.parent_count,
            parent_states: self.parent_states.clone(),
            result_state: self.result_state.clone(),
            manifest_sha256: self.manifest_sha256,
            parent_eq_deferred_sha256: self.parent_eq_deferred_sha256,
            parent_ep_deferred_sha256: self.parent_ep_deferred_sha256,
        }
        // ABI-21 parent slots preserve split.inputs order, which is already
        // canonical by bundle digest. State-vector lexicographic order is an
        // unrelated historical V1 wire rule and cannot be imposed here.
        .validate_with_parent_state_order(
            proof_step_count,
            false,
            require_deferred_audit_joins,
        )?;
        if self.live_selector != KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4
            || self.operation.to_fields().is_err()
            || self.step_eq_compiled_protocol_sha256 == [0; 8]
            || self.step_ep_compiled_protocol_sha256 == [0; 8]
            || self.step_eq_compiled_protocol_sha256 == self.step_ep_compiled_protocol_sha256
        {
            return Err("Kagemusha V4 operation/protocol public shape mismatch".to_owned());
        }
        let initializing = proof_step_count == 1;
        match (
            initializing,
            &self.parent_eq_lineage_accumulator,
            &self.parent_ep_lineage_accumulator,
        ) {
            (true, None, None) => {}
            (false, Some(eq), Some(ep)) => {
                eq.to_eq(params.k)?;
                ep.to_ep(params.k)?;
            }
            _ => {
                return Err("Kagemusha V4 parent-lineage accumulator presence mismatch".to_owned());
            }
        }
        Ok(layout)
    }

    fn compact_header_chunks_v5(&self, proof_step_count: u32) -> Vec<u128> {
        let mut header = Vec::with_capacity(KAGEMUSHA_COMPACT_HEADER_WITHOUT_SELECTOR_CELLS_V5);
        header.extend([
            u128::from(KAGEMUSHA_COMPACT_PROFILE_VERSION_V5),
            u128::from(self.parent_count),
            u128::from(proof_step_count),
        ]);
        header.extend(kagemusha_u32_words_to_u128_chunks_v5(
            &self.public_statement_digest,
        ));
        header.extend(kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_DOMAIN_V5,
            &self.operation.limbs,
        ));
        for (slot, state) in self.parent_states.iter().enumerate() {
            if slot < self.parent_count as usize {
                header.extend(kagemusha_poseidon_commitment_chunks_v5(
                    KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
                    state,
                ));
            } else {
                header.extend([0_u128; KAGEMUSHA_COMPACT_DIGEST_CHUNKS_V5]);
            }
        }
        header.extend(kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            &self.result_state,
        ));
        header.extend(kagemusha_u32_words_to_u128_chunks_v5(&self.manifest_sha256));
        header.extend(kagemusha_u32_words_to_u128_chunks_v5(
            &self.step_eq_compiled_protocol_sha256,
        ));
        header.extend(kagemusha_u32_words_to_u128_chunks_v5(
            &self.step_ep_compiled_protocol_sha256,
        ));
        debug_assert_eq!(
            header.len(),
            KAGEMUSHA_COMPACT_HEADER_WITHOUT_SELECTOR_CELLS_V5
        );
        header
    }

    /// Convert the compact parity-local V5 recursive boundary to one column.
    pub fn instance_column<F>(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
        parity: KagemushaPastaCycleParityV1,
    ) -> Result<Vec<F>, String>
    where
        F: PrimeField + From<u64>,
    {
        self.instance_column_with_deferred_audit_joins(proof_step_count, params, parity, true)
    }

    fn instance_column_for_audit_derivation_prepass<F>(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
        parity: KagemushaPastaCycleParityV1,
    ) -> Result<Vec<F>, String>
    where
        F: PrimeField + From<u64>,
    {
        self.instance_column_with_deferred_audit_joins(proof_step_count, params, parity, false)
    }

    fn instance_column_with_deferred_audit_joins<F>(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
        parity: KagemushaPastaCycleParityV1,
        require_deferred_audit_joins: bool,
    ) -> Result<Vec<F>, String>
    where
        F: PrimeField + From<u64>,
    {
        let layout = self.validate_with_deferred_audit_joins(
            proof_step_count,
            params,
            require_deferred_audit_joins,
        )?;
        let mut limbs = self.compact_header_chunks_v5(proof_step_count);
        let accumulator_limbs = usize::try_from(layout.accumulator_limbs)
            .map_err(|_| "Kagemusha V4 accumulator length does not fit usize".to_owned())?;
        let accumulator = match parity {
            KagemushaPastaCycleParityV1::StepEq => &self.parent_eq_lineage_accumulator,
            KagemushaPastaCycleParityV1::StepEp => &self.parent_ep_lineage_accumulator,
        };
        match accumulator {
            Some(accumulator) => limbs.extend(accumulator.instance_limbs(params.k)?),
            None => limbs.resize(limbs.len() + accumulator_limbs, 0),
        }
        for digest in self
            .parent_eq_deferred_sha256
            .iter()
            .chain(&self.parent_ep_deferred_sha256)
        {
            limbs.extend(kagemusha_u32_words_to_u128_chunks_v5(digest));
        }
        limbs.push(u128::from(self.live_selector));
        let expected = usize::try_from(layout.instance_column_limbs)
            .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
        if limbs.len() != expected {
            return Err("Kagemusha V4 instance-column length mismatch".to_owned());
        }
        Ok(limbs.into_iter().map(F::from_u128).collect())
    }

    fn private_semantic_column<F>(&self) -> Vec<F>
    where
        F: PrimeField + From<u64>,
    {
        self.public_statement_digest
            .iter()
            .chain(&self.operation.limbs)
            .chain(std::iter::once(&self.parent_count))
            .chain(self.parent_states.iter().flatten())
            .chain(&self.result_state)
            .chain(&self.manifest_sha256)
            .chain(&self.step_eq_compiled_protocol_sha256)
            .chain(&self.step_ep_compiled_protocol_sha256)
            .copied()
            .map(|limb| F::from(u64::from(limb)))
            .collect()
    }
}

/// Compact public carrier embedded once in an Eq/Ep proof pair.
///
/// Exact operations and state openings remain prover-side witnesses. The wire
/// retains only the common semantic header, the two parity-local accumulated
/// lineages, and four reciprocal audit digests needed to reconstruct each
/// 64-cell verifier instance.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaCompactPublicInputsV5 {
    common_header: Vec<u128>,
    parent_eq_lineage_accumulator: Option<KagemushaIpaAccumulatorWireV4>,
    parent_ep_lineage_accumulator: Option<KagemushaIpaAccumulatorWireV4>,
    parent_eq_deferred_chunks: [[u128; 2]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    parent_ep_deferred_chunks: [[u128; 2]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
}

impl KagemushaCompactPublicInputsV5 {
    fn from_private(inputs: &KagemushaPastaCyclePublicInputsV4, proof_step_count: u32) -> Self {
        let mut common_header = inputs.compact_header_chunks_v5(proof_step_count);
        common_header.push(u128::from(inputs.live_selector));
        Self {
            common_header,
            parent_eq_lineage_accumulator: inputs.parent_eq_lineage_accumulator.clone(),
            parent_ep_lineage_accumulator: inputs.parent_ep_lineage_accumulator.clone(),
            parent_eq_deferred_chunks: inputs
                .parent_eq_deferred_sha256
                .map(|digest| kagemusha_u32_words_to_u128_chunks_v5(&digest)),
            parent_ep_deferred_chunks: inputs
                .parent_ep_deferred_sha256
                .map(|digest| kagemusha_u32_words_to_u128_chunks_v5(&digest)),
        }
    }

    fn parent_count(&self) -> Result<u32, String> {
        self.common_header
            .get(KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5)
            .copied()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|count| *count <= KAGEMUSHA_PASTA_PARENT_SLOTS_V1 as u32)
            .ok_or_else(|| "Kagemusha V5 compact parent count is invalid".to_owned())
    }

    fn proof_step_count(&self) -> Result<u32, String> {
        self.common_header
            .get(KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5)
            .copied()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|count| *count != 0)
            .ok_or_else(|| "Kagemusha V5 compact proof-step count is invalid".to_owned())
    }

    fn validate(
        &self,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        let layout = validate_kagemusha_circuit_params_v4(params)?;
        if self.common_header.len() != 20
            || self.common_header[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5]
                != u128::from(KAGEMUSHA_COMPACT_PROFILE_VERSION_V5)
            || self.common_header[19] != u128::from(KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4)
        {
            return Err("Kagemusha V5 compact common header is invalid".to_owned());
        }
        let parent_count = self.parent_count()?;
        let proof_step_count = self.proof_step_count()?;
        if (proof_step_count == 1) != (parent_count == 0) {
            return Err("Kagemusha V5 compact initialization header is invalid".to_owned());
        }
        match (
            parent_count == 0,
            &self.parent_eq_lineage_accumulator,
            &self.parent_ep_lineage_accumulator,
        ) {
            (true, None, None) => {}
            (false, Some(eq), Some(ep)) => {
                eq.to_eq(params.k)?;
                ep.to_ep(params.k)?;
            }
            _ => {
                return Err("Kagemusha V5 compact lineage presence mismatch".to_owned());
            }
        }
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            let present = slot < parent_count as usize;
            let eq = self.parent_eq_deferred_chunks[slot];
            let ep = self.parent_ep_deferred_chunks[slot];
            if present {
                if eq == [0; 2] || ep == [0; 2] || eq == ep {
                    return Err("Kagemusha V5 compact reciprocal audit is invalid".to_owned());
                }
            } else if eq != [0; 2] || ep != [0; 2] {
                return Err("Kagemusha V5 absent audit slot is not zero".to_owned());
            }
        }
        for offset in [
            KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5,
            KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5,
            KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5,
            KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5,
            KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
            KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5,
        ] {
            if self.common_header[offset..offset + 2] == [0; 2] {
                return Err("Kagemusha V5 compact semantic commitment is zero".to_owned());
            }
        }
        if self.common_header[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
            == self.common_header[KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
                ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
        {
            return Err("Kagemusha V5 compact protocol identities collide".to_owned());
        }
        Ok(layout)
    }

    fn instance_column<F>(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        parity: KagemushaPastaCycleParityV1,
    ) -> Result<Vec<F>, String>
    where
        F: PrimeField + From<u64>,
    {
        let layout = self.validate(params)?;
        let mut cells = self.common_header[..19].to_vec();
        let accumulator = match parity {
            KagemushaPastaCycleParityV1::StepEq => &self.parent_eq_lineage_accumulator,
            KagemushaPastaCycleParityV1::StepEp => &self.parent_ep_lineage_accumulator,
        };
        match accumulator {
            Some(accumulator) => cells.extend(accumulator.instance_limbs(params.k)?),
            None => cells.resize(
                cells.len()
                    + usize::try_from(layout.accumulator_limbs)
                        .map_err(|_| "Kagemusha V5 accumulator length does not fit usize")?,
                0,
            ),
        }
        cells.extend(self.parent_eq_deferred_chunks.iter().flatten().copied());
        cells.extend(self.parent_ep_deferred_chunks.iter().flatten().copied());
        cells.push(self.common_header[19]);
        if cells.len() != usize::try_from(layout.instance_column_limbs).unwrap_or(0) {
            return Err("Kagemusha V5 compact instance length mismatch".to_owned());
        }
        Ok(cells.into_iter().map(F::from_u128).collect())
    }
}

/// Backend-native V4 Eq/Ep pair encoded inside the public opaque proof box.
///
/// This is deliberately not a data-model envelope. ABI 21 carries the
/// canonical Norito bytes of this value as an opaque proof payload, while the
/// core alone constructs, decodes, and verifies its recursion-specific fields.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaPastaCycleProofPairV4 {
    /// Exact backend-native pair layout version.
    pub(crate) version: u16,
    /// Logical recursive transition count proved by both halves.
    pub(crate) proof_step_count: u32,
    /// Compact common header and parity-local recursive tails.
    public_inputs: KagemushaCompactPublicInputsV5,
    /// Current Eq/Fp augmented proof bytes.
    pub(crate) step_eq_proof_bytes: Vec<u8>,
    /// Current Ep/Fq augmented proof bytes.
    pub(crate) step_ep_proof_bytes: Vec<u8>,
    /// BGH19 proof folding the current Eq opening with its parent lineage.
    pub(crate) step_eq_accumulation_proof: KagemushaIpaAccumulationProofV4,
    /// BGH19 proof folding the current Ep opening with its parent lineage.
    pub(crate) step_ep_accumulation_proof: KagemushaIpaAccumulationProofV4,
}

/// Exact backend-native layout version of [`KagemushaPastaCycleProofPairV4`].
pub(crate) const KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4: u16 = 5;

impl KagemushaPastaCycleProofPairV4 {
    /// Validate the complete pair against authenticated release parameters and
    /// the release's measured opaque-proof payload cap.
    pub(crate) fn validate(
        &self,
        step_eq_params: &KagemushaStepCircuitParamsV4,
        step_ep_params: &KagemushaStepCircuitParamsV4,
        max_pair_bytes: u32,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        let eq_layout = self.public_inputs.validate(step_eq_params)?;
        let ep_layout = self.public_inputs.validate(step_ep_params)?;
        let eq_proof_bytes = usize::try_from(step_eq_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof size does not fit usize".to_owned())?;
        let ep_proof_bytes = usize::try_from(step_ep_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Ep proof size does not fit usize".to_owned())?;
        if self.version != KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4
            || eq_layout != ep_layout
            || step_eq_params.k != step_ep_params.k
            || self.public_inputs.proof_step_count()? != self.proof_step_count
            || self.step_eq_proof_bytes.len() != eq_proof_bytes
            || self.step_ep_proof_bytes.len() != ep_proof_bytes
            || self.step_eq_proof_bytes == self.step_ep_proof_bytes
        {
            return Err("Kagemusha V4 Eq/Ep proof-pair shape mismatch".to_owned());
        }
        let has_parent = self.public_inputs.parent_count()? != 0;
        self.step_eq_accumulation_proof
            .validate(step_eq_params.k, has_parent)?;
        self.step_ep_accumulation_proof
            .validate(step_ep_params.k, has_parent)?;

        let maximum = usize::try_from(max_pair_bytes)
            .map_err(|_| "Kagemusha V4 pair bound does not fit usize".to_owned())?;
        let absolute_maximum = usize::try_from(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
        )
        .map_err(|_| "Kagemusha V4 absolute pair bound does not fit usize".to_owned())?;
        if maximum == 0 || maximum > absolute_maximum {
            return Err("Kagemusha V4 authenticated pair bound is invalid".to_owned());
        }
        let encoded = norito::encode_canonical(self)
            .map_err(|error| format!("failed to encode Kagemusha V4 proof pair: {error}"))?;
        if encoded.len() > maximum {
            return Err(format!(
                "Kagemusha V4 proof pair is {} bytes; authenticated maximum is {maximum}",
                encoded.len()
            ));
        }
        Ok(eq_layout)
    }

    /// Decode one opaque ABI-21 proof payload, reject non-canonical bytes, and
    /// validate it against the pinned authenticated release profile.
    pub(crate) fn decode_authenticated(
        bytes: &[u8],
        step_eq_params: &KagemushaStepCircuitParamsV4,
        step_ep_params: &KagemushaStepCircuitParamsV4,
        max_pair_bytes: u32,
    ) -> Result<Self, String> {
        let maximum = usize::try_from(max_pair_bytes)
            .map_err(|_| "Kagemusha V4 pair bound does not fit usize".to_owned())?;
        if bytes.is_empty() || bytes.len() > maximum {
            return Err("Kagemusha V4 opaque proof payload length is invalid".to_owned());
        }
        let pair: Self = norito::decode_canonical_with_limits(
            bytes,
            kagemusha_runtime_norito_decode_limits_v4(bytes.len()),
        )
        .map_err(|error| {
            if matches!(&error, norito::Error::NonCanonicalEncoding) {
                "Kagemusha V4 proof pair is not canonical Norito".to_owned()
            } else {
                format!("failed to decode Kagemusha V4 proof pair: {error}")
            }
        })?;
        pair.validate(step_eq_params, step_ep_params, max_pair_bytes)?;
        Ok(pair)
    }

    /// Encode one fully validated native pair for the public opaque proof box.
    pub(crate) fn encode_authenticated(
        &self,
        step_eq_params: &KagemushaStepCircuitParamsV4,
        step_ep_params: &KagemushaStepCircuitParamsV4,
        max_pair_bytes: u32,
    ) -> Result<Vec<u8>, String> {
        self.validate(step_eq_params, step_ep_params, max_pair_bytes)?;
        norito::encode_canonical(self)
            .map_err(|error| format!("failed to encode Kagemusha V4 proof pair: {error}"))
    }
}

/// Validate a canonical V4 proof-pair measurement without exposing its wire.
///
/// Artifact tooling uses this only after producing a real pair with the
/// authenticated keys. Runtime verification additionally performs both
/// terminal cryptographic decisions through the installed verifier.
pub fn validate_kagemusha_proof_pair_measurement_v4(
    bytes: &[u8],
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
) -> Result<usize, String> {
    KagemushaPastaCycleProofPairV4::decode_authenticated(
        bytes,
        step_eq_params,
        step_ep_params,
        max_pair_bytes,
    )?;
    Ok(bytes.len())
}

const KAGEMUSHA_POSEIDON_WIDTH: usize = 3;
const KAGEMUSHA_POSEIDON_RATE: usize = 2;
const KAGEMUSHA_POSEIDON_FULL_ROUNDS: usize = 8;
const KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS: usize = 57;
const KAGEMUSHA_POSEIDON_SECURE_MDS: usize = 0;

fn catch_kagemusha_native_verifier_panic<T>(
    label: &str,
    verify: impl FnOnce() -> T,
) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(verify))
        .map_err(|_| format!("Kagemusha V4 {label} rejected an invalid native verifier relation"))
}

/// Fully verify and terminally decide a degree-parameterized V4 Eq proof.
pub(crate) fn terminal_verify_step_eq_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    proof: &[u8],
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<(), String> {
    use halo2_proofs::poly::commitment::Params as _;

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k {
        return Err("Kagemusha V4 Eq ParamsIPA/circuit degree mismatch".to_owned());
    }
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?;
    let instances = vec![public_inputs.instance_column::<Fp>(
        proof_step_count,
        circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
    )?];
    let current = succinct_verify_step_eq_instances(
        params,
        verifying_key,
        proof,
        &instances,
        max_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current,
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    Ok(())
}

/// Fully verify and terminally decide a degree-parameterized V4 Ep proof.
pub(crate) fn terminal_verify_step_ep_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    proof: &[u8],
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<(), String> {
    use halo2_proofs::poly::commitment::Params as _;

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k {
        return Err("Kagemusha V4 Ep ParamsIPA/circuit degree mismatch".to_owned());
    }
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?;
    let instances = vec![public_inputs.instance_column::<Fq>(
        proof_step_count,
        circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
    )?];
    let current = succinct_verify_step_ep_instances(
        params,
        verifying_key,
        proof,
        &instances,
        max_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current,
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    Ok(())
}

fn succinct_verify_step_eq_instances(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    proof: &[u8],
    instances: &[Vec<Fp>],
    max_proof_bytes: usize,
) -> Result<
    snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EqAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
    String,
> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Eq, EqAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaSuccinctVerifyingKey},
        system::halo2::{compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkSuccinctVerifier},
    };

    if max_proof_bytes == 0 || proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha Eq proof length is invalid".to_owned());
    }
    type Scheme = IpaAs<EqAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let w = hash_to_curve(&[1]).to_affine();
    let u = hash_to_curve(&[2]).to_affine();
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "Eq parameter degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k())
                    .map_err(|_| "Eq parameter degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        u,
        Some(w),
    );
    let protocol = compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(instances[0].len()),
    );
    let mut cursor = std::io::Cursor::new(proof);
    {
        let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
        let parsed = catch_kagemusha_native_verifier_panic("Eq proof parse", || {
            PlonkSuccinctVerifier::<Scheme>::read_proof(&svk, &protocol, instances, &mut transcript)
        })?
        .map_err(|error| format!("failed to parse Kagemusha Eq proof: {error:?}"))?;
        let accumulators = catch_kagemusha_native_verifier_panic("Eq proof verification", || {
            PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, instances, &parsed)
        })?
        .map_err(|error| format!("Kagemusha Eq succinct verification failed: {error:?}"))?;
        let [accumulator]: [_; 1] = accumulators.try_into().map_err(|accumulators: Vec<_>| {
            format!(
                "Kagemusha Eq proof emitted {} opening accumulators instead of one",
                accumulators.len()
            )
        })?;
        if cursor.position()
            != u64::try_from(proof.len()).map_err(|_| "Eq proof length does not fit u64")?
        {
            return Err("Kagemusha Eq proof has trailing bytes".to_owned());
        }
        return Ok(accumulator);
    }
}

fn succinct_verify_step_ep_instances(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    proof: &[u8],
    instances: &[Vec<Fq>],
    max_proof_bytes: usize,
) -> Result<
    snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EpAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
    String,
> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Ep, EpAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaSuccinctVerifyingKey},
        system::halo2::{compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkSuccinctVerifier},
    };

    if max_proof_bytes == 0 || proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha Ep proof length is invalid".to_owned());
    }
    type Scheme = IpaAs<EpAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let w = hash_to_curve(&[1]).to_affine();
    let u = hash_to_curve(&[2]).to_affine();
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "Ep parameter degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k())
                    .map_err(|_| "Ep parameter degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        u,
        Some(w),
    );
    let protocol = compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(instances[0].len()),
    );
    let mut cursor = std::io::Cursor::new(proof);
    {
        let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
        let parsed = catch_kagemusha_native_verifier_panic("Ep proof parse", || {
            PlonkSuccinctVerifier::<Scheme>::read_proof(&svk, &protocol, instances, &mut transcript)
        })?
        .map_err(|error| format!("failed to parse Kagemusha Ep proof: {error:?}"))?;
        let accumulators = catch_kagemusha_native_verifier_panic("Ep proof verification", || {
            PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, instances, &parsed)
        })?
        .map_err(|error| format!("Kagemusha Ep succinct verification failed: {error:?}"))?;
        let [accumulator]: [_; 1] = accumulators.try_into().map_err(|accumulators: Vec<_>| {
            format!(
                "Kagemusha Ep proof emitted {} opening accumulators instead of one",
                accumulators.len()
            )
        })?;
        if cursor.position()
            != u64::try_from(proof.len()).map_err(|_| "Ep proof length does not fit u64")?
        {
            return Err("Kagemusha Ep proof has trailing bytes".to_owned());
        }
        return Ok(accumulator);
    }
}

/// Recompile the exact V4 self protocols from authenticated Params/VKs and
/// require the pair's public identities to match both compiled protocols.
fn terminal_validate_compiled_protocol_identities_v4(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    public_inputs: &KagemushaCompactPublicInputsV5,
    proof_step_count: u32,
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<(), String> {
    use halo2_proofs::poly::commitment::Params as _;
    use snark_verifier::system::halo2::compile;

    let eq_layout = public_inputs.validate(step_eq_circuit_params)?;
    let ep_layout = public_inputs.validate(step_ep_circuit_params)?;
    if eq_layout != ep_layout
        || step_eq_circuit_params.k != step_ep_circuit_params.k
        || step_eq_params.k() != step_eq_circuit_params.k
        || step_ep_params.k() != step_ep_circuit_params.k
    {
        return Err("Kagemusha V4 terminal parameter/layout mismatch".to_owned());
    }
    let instance_len = usize::try_from(eq_layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 terminal public length does not fit usize".to_owned())?;
    let compile_config = || kagemusha_ipa_compile_config_v4(instance_len);
    let eq_protocol = compile(step_eq_params, step_eq_verifying_key, compile_config());
    let ep_protocol = compile(step_ep_params, step_ep_verifying_key, compile_config());
    let expected_eq = kagemusha_u32_words_to_u128_chunks_v5(&kagemusha_sha256_public_words(
        kagemusha_compiled_protocol_identity_sha256(
            &eq_protocol,
            KagemushaPastaCycleParityV1::StepEq,
        )?,
    ));
    let expected_ep = kagemusha_u32_words_to_u128_chunks_v5(&kagemusha_sha256_public_words(
        kagemusha_compiled_protocol_identity_sha256(
            &ep_protocol,
            KagemushaPastaCycleParityV1::StepEp,
        )?,
    ));
    if public_inputs.common_header[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
        ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
        != expected_eq
        || public_inputs.common_header[KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
            != expected_ep
        || public_inputs.proof_step_count()? != proof_step_count
    {
        return Err(
            "Kagemusha V4 compiled-protocol identity does not match authenticated artifacts"
                .to_owned(),
        );
    }
    Ok(())
}

/// Fully verify and terminally decide both halves of one backend-native V4
/// pair under its authenticated release parameters.
pub(crate) fn terminal_verify_proof_pair_v4(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    pair: &KagemushaPastaCycleProofPairV4,
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
) -> Result<(), String> {
    terminal_verify_proof_pair_lineage_v4(
        step_eq_params,
        step_eq_verifying_key,
        step_ep_params,
        step_ep_verifying_key,
        pair,
        step_eq_circuit_params,
        step_ep_circuit_params,
        max_pair_bytes,
    )?;
    Ok(())
}

/// Verify a V4 pair, terminally decide both folds, and return the complete
/// lineages needed to construct an authenticated child operation.
pub(crate) fn terminal_verify_proof_pair_lineage_v4(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    pair: &KagemushaPastaCycleProofPairV4,
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
) -> Result<(KagemushaIpaAccumulatorWireV4, KagemushaIpaAccumulatorWireV4), String> {
    pair.validate(
        step_eq_circuit_params,
        step_ep_circuit_params,
        max_pair_bytes,
    )?;
    terminal_validate_compiled_protocol_identities_v4(
        step_eq_params,
        step_eq_verifying_key,
        step_ep_params,
        step_ep_verifying_key,
        &pair.public_inputs,
        pair.proof_step_count,
        step_eq_circuit_params,
        step_ep_circuit_params,
    )?;

    let eq_instances = vec![
        pair.public_inputs
            .instance_column::<Fp>(step_eq_circuit_params, KagemushaPastaCycleParityV1::StepEq)?,
    ];
    let eq_current = succinct_verify_step_eq_instances(
        step_eq_params,
        step_eq_verifying_key,
        &pair.step_eq_proof_bytes,
        &eq_instances,
        usize::try_from(step_eq_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?,
    )?;
    let eq_parent = pair
        .public_inputs
        .parent_eq_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_eq(step_eq_circuit_params.k))
        .transpose()?;
    let eq_lineage = super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        step_eq_params,
        step_eq_circuit_params.k,
        eq_current,
        eq_parent,
        &pair.step_eq_accumulation_proof,
    )?;

    let ep_instances = vec![
        pair.public_inputs
            .instance_column::<Fq>(step_ep_circuit_params, KagemushaPastaCycleParityV1::StepEp)?,
    ];
    let ep_current = succinct_verify_step_ep_instances(
        step_ep_params,
        step_ep_verifying_key,
        &pair.step_ep_proof_bytes,
        &ep_instances,
        usize::try_from(step_ep_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?,
    )?;
    let ep_parent = pair
        .public_inputs
        .parent_ep_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_ep(step_ep_circuit_params.k))
        .transpose()?;
    let ep_lineage = super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        step_ep_params,
        step_ep_circuit_params.k,
        ep_current,
        ep_parent,
        &pair.step_ep_accumulation_proof,
    )?;

    Ok((
        KagemushaIpaAccumulatorWireV4::from_eq(&eq_lineage, step_eq_circuit_params.k)?,
        KagemushaIpaAccumulatorWireV4::from_ep(&ep_lineage, step_ep_circuit_params.k)?,
    ))
}

/// Parsed terminal-verifier material for one authenticated V4 release.
///
/// As with the prover, fields are private and are populated only by the V4
/// framed-artifact loader after profile, digest, key, and bootstrap checks.
const KAGEMUSHA_HALO2_KEY_VERSION_V4: u8 = 0x02;
const KAGEMUSHA_HALO2_UNCOMPRESSED_SELECTORS_V4: u8 = 0;
const KAGEMUSHA_HALO2_VK_HEADER_BYTES_V4: u64 = 10;
const KAGEMUSHA_HALO2_PK_VECTOR_HEADERS_BYTES_V4: u64 = 4 * 4;
const KAGEMUSHA_HALO2_LENGTH_PREFIX_BYTES_V4: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KagemushaProcessedKeyShapeV4 {
    k: u32,
    domain_rows: u32,
    fixed_polynomials: usize,
    permutation_polynomials: usize,
    point_bytes: usize,
    scalar_bytes: usize,
}

impl KagemushaProcessedKeyShapeV4 {
    fn fixed_polynomials_u32(self, role: &str) -> Result<u32, String> {
        u32::try_from(self.fixed_polynomials)
            .map_err(|_| format!("Kagemusha V4 {role} fixed-polynomial count does not fit u32"))
    }

    fn permutation_polynomials_u32(self, role: &str) -> Result<u32, String> {
        u32::try_from(self.permutation_polynomials).map_err(|_| {
            format!("Kagemusha V4 {role} permutation-polynomial count does not fit u32")
        })
    }

    fn verifier_key_bytes(self, role: &str) -> Result<u64, String> {
        let commitments = self
            .fixed_polynomials
            .checked_add(self.permutation_polynomials)
            .and_then(|count| u64::try_from(count).ok())
            .ok_or_else(|| format!("Kagemusha V4 {role} commitment count overflows"))?;
        let point_bytes = u64::try_from(self.point_bytes)
            .map_err(|_| format!("Kagemusha V4 {role} point width does not fit u64"))?;
        commitments
            .checked_mul(point_bytes)
            .and_then(|bytes| bytes.checked_add(KAGEMUSHA_HALO2_VK_HEADER_BYTES_V4))
            .ok_or_else(|| format!("Kagemusha V4 {role} verifier-key length overflows"))
    }

    fn proving_key_bytes(self, role: &str) -> Result<u64, String> {
        let scalar_bytes = u64::try_from(self.scalar_bytes)
            .map_err(|_| format!("Kagemusha V4 {role} scalar width does not fit u64"))?;
        let polynomial_bytes = u64::from(self.domain_rows)
            .checked_mul(scalar_bytes)
            .and_then(|bytes| bytes.checked_add(KAGEMUSHA_HALO2_LENGTH_PREFIX_BYTES_V4))
            .ok_or_else(|| format!("Kagemusha V4 {role} polynomial length overflows"))?;
        let polynomial_count = self
            .fixed_polynomials
            .checked_mul(2)
            .and_then(|count| {
                self.permutation_polynomials
                    .checked_mul(2)
                    .and_then(|permutations| count.checked_add(permutations))
            })
            .and_then(|count| count.checked_add(3))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or_else(|| format!("Kagemusha V4 {role} polynomial count overflows"))?;
        self.verifier_key_bytes(role)?
            .checked_add(KAGEMUSHA_HALO2_PK_VECTOR_HEADERS_BYTES_V4)
            .and_then(|bytes| {
                polynomial_count
                    .checked_mul(polynomial_bytes)
                    .and_then(|polynomials| bytes.checked_add(polynomials))
            })
            .ok_or_else(|| format!("Kagemusha V4 {role} proving-key length overflows"))
    }
}

struct KagemushaStructuralCursorV4<'a> {
    bytes: &'a [u8],
    offset: usize,
    role: &'a str,
}

impl<'a> KagemushaStructuralCursorV4<'a> {
    fn new(bytes: &'a [u8], role: &'a str) -> Self {
        Self {
            bytes,
            offset: 0,
            role,
        }
    }

    fn read_array<const N: usize>(&mut self, field: &str) -> Result<[u8; N], String> {
        let end = self.offset.checked_add(N).ok_or_else(|| {
            format!(
                "Kagemusha V4 {} {field} structural offset overflows",
                self.role
            )
        })?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| format!("Kagemusha V4 {} {field} is truncated", self.role))?
            .try_into()
            .map_err(|_| format!("Kagemusha V4 {} {field} has invalid width", self.role))?;
        self.offset = end;
        Ok(value)
    }

    fn read_u8(&mut self, field: &str) -> Result<u8, String> {
        Ok(self.read_array::<1>(field)?[0])
    }

    fn read_u32_le(&mut self, field: &str) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.read_array(field)?))
    }

    #[cfg(test)]
    fn read_u32_be(&mut self, field: &str) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.read_array(field)?))
    }

    fn skip(&mut self, count: usize, field: &str) -> Result<(), String> {
        let end = self.offset.checked_add(count).ok_or_else(|| {
            format!(
                "Kagemusha V4 {} {field} structural offset overflows",
                self.role
            )
        })?;
        if end > self.bytes.len() {
            return Err(format!(
                "Kagemusha V4 {} {field} payload is truncated",
                self.role
            ));
        }
        self.offset = end;
        Ok(())
    }

    fn finish(self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "Kagemusha V4 {} encoding has trailing bytes",
                self.role
            ))
        }
    }
}

trait KagemushaProcessedKeyCurveV4: CurveAffine {
    fn configured_wire_shape_v4(
        circuit_params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaConfiguredVkWireShapeV4, String>;
}

impl KagemushaProcessedKeyCurveV4 for halo2_proofs::halo2curves::pasta::EqAffine {
    fn configured_wire_shape_v4(
        circuit_params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaConfiguredVkWireShapeV4, String> {
        configured_kagemusha_eq_vk_wire_shape_v4(circuit_params)
    }
}

impl KagemushaProcessedKeyCurveV4 for halo2_proofs::halo2curves::pasta::EpAffine {
    fn configured_wire_shape_v4(
        circuit_params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaConfiguredVkWireShapeV4, String> {
        configured_kagemusha_ep_vk_wire_shape_v4(circuit_params)
    }
}

fn kagemusha_processed_key_shape_v4<C>(
    circuit_params: &KagemushaStepCircuitParamsV4,
    role: &str,
) -> Result<KagemushaProcessedKeyShapeV4, String>
where
    C: KagemushaProcessedKeyCurveV4,
    C::Scalar: halo2_base::utils::ScalarField,
{
    validate_kagemusha_circuit_params_v4(circuit_params)?;
    let configured = C::configured_wire_shape_v4(circuit_params)?;

    // `keygen_vk` disables selector compression. It appends one fixed
    // polynomial per selector before serializing the fixed commitments, while
    // the permutation vectors retain one polynomial per equality-enabled
    // column. Derive both counts from the complete authenticated composite
    // circuit (Base, all SHA lanes, and the dense reciprocal MSM) rather than
    // the Base subcircuit alone or either serialized u32 count.
    let fixed_polynomials = configured
        .base_fixed_columns
        .checked_add(configured.selectors)
        .ok_or_else(|| format!("Kagemusha V4 {role} fixed-polynomial count overflows"))?;
    let permutation_polynomials = configured.permutation_columns;
    let domain_rows = 1_u32
        .checked_shl(circuit_params.k)
        .ok_or_else(|| format!("Kagemusha V4 {role} domain-row count overflows"))?;
    let point_bytes = C::default().to_bytes().as_ref().len();
    let scalar_bytes = <C::Scalar as PrimeField>::Repr::default().as_ref().len();
    if point_bytes == 0 || scalar_bytes == 0 {
        return Err(format!(
            "Kagemusha V4 {role} processed element width is zero"
        ));
    }

    let shape = KagemushaProcessedKeyShapeV4 {
        k: circuit_params.k,
        domain_rows,
        fixed_polynomials,
        permutation_polynomials,
        point_bytes,
        scalar_bytes,
    };
    shape.fixed_polynomials_u32(role)?;
    shape.permutation_polynomials_u32(role)?;
    Ok(shape)
}

fn kagemusha_params_encoded_bytes_v4<C>(expected_k: u32, role: &str) -> Result<u64, String>
where
    C: CurveAffine,
{
    let domain_rows = 1_u64
        .checked_shl(expected_k)
        .ok_or_else(|| format!("Kagemusha V4 {role} parameter row count overflows"))?;
    let point_count = domain_rows
        .checked_mul(2)
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| format!("Kagemusha V4 {role} parameter point count overflows"))?;
    let point_bytes = u64::try_from(C::default().to_bytes().as_ref().len())
        .map_err(|_| format!("Kagemusha V4 {role} parameter point width does not fit u64"))?;
    point_count
        .checked_mul(point_bytes)
        .and_then(|bytes| bytes.checked_add(KAGEMUSHA_HALO2_LENGTH_PREFIX_BYTES_V4))
        .ok_or_else(|| format!("Kagemusha V4 {role} parameter byte length overflows"))
}

/// Exact processed serialization lengths derived from one authenticated V4 profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaArtifactEncodingSizesV4 {
    /// Exact unframed `ParamsIPA` length.
    pub(crate) parameters_bytes: u64,
    /// Exact unframed processed proving-key length.
    pub(crate) proving_key_bytes: u64,
    /// Exact unframed processed verifier-key length.
    pub(crate) verifying_key_bytes: u64,
}

fn kagemusha_artifact_encoding_sizes_for_curve_v4<C>(
    circuit_params: &KagemushaStepCircuitParamsV4,
    role: &str,
) -> Result<KagemushaArtifactEncodingSizesV4, String>
where
    C: KagemushaProcessedKeyCurveV4,
    C::Scalar: halo2_base::utils::ScalarField,
{
    let shape = kagemusha_processed_key_shape_v4::<C>(circuit_params, role)?;
    Ok(KagemushaArtifactEncodingSizesV4 {
        parameters_bytes: kagemusha_params_encoded_bytes_v4::<C>(circuit_params.k, role)?,
        proving_key_bytes: shape.proving_key_bytes(role)?,
        verifying_key_bytes: shape.verifier_key_bytes(role)?,
    })
}

/// Derive exact processed role lengths without allocating a Halo2 domain or key.
pub(crate) fn kagemusha_artifact_encoding_sizes_v4(
    circuit_params: &KagemushaStepCircuitParamsV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<KagemushaArtifactEncodingSizesV4, String> {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => kagemusha_artifact_encoding_sizes_for_curve_v4::<
            halo2_proofs::halo2curves::pasta::EqAffine,
        >(circuit_params, "Eq"),
        KagemushaPastaCycleParityV1::StepEp => kagemusha_artifact_encoding_sizes_for_curve_v4::<
            halo2_proofs::halo2curves::pasta::EpAffine,
        >(circuit_params, "Ep"),
    }
}

fn validate_kagemusha_generation_encoding_sizes_v4<C>(
    circuit_params: &KagemushaStepCircuitParamsV4,
    role: &str,
) -> Result<(), String>
where
    C: KagemushaProcessedKeyCurveV4,
    C::Scalar: halo2_base::utils::ScalarField,
{
    let shape = kagemusha_processed_key_shape_v4::<C>(circuit_params, role)?;
    let lengths = [
        (
            "parameters",
            kagemusha_params_encoded_bytes_v4::<C>(circuit_params.k, role)?,
        ),
        ("verifier key", shape.verifier_key_bytes(role)?),
        ("proving key", shape.proving_key_bytes(role)?),
    ];
    for (kind, length) in lengths {
        if length == 0 || length >= KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4 {
            return Err(format!(
                "Kagemusha V4 canonical {role} {kind} length {length} bytes violates the fixed {}-byte artifact-size corridor",
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            ));
        }
    }
    Ok(())
}

fn validate_kagemusha_params_encoding_v4<C>(
    bytes: &[u8],
    expected_k: u32,
    role: &str,
) -> Result<(), String>
where
    C: CurveAffine,
{
    let mut cursor = KagemushaStructuralCursorV4::new(bytes, role);
    let encoded_k = cursor.read_u32_le("parameter degree")?;
    if encoded_k != expected_k {
        return Err(format!(
            "Kagemusha V4 {role} parameter degree {encoded_k} does not match authenticated degree {expected_k}"
        ));
    }
    let encoded_bytes = kagemusha_params_encoded_bytes_v4::<C>(expected_k, role)?;
    let payload_bytes = encoded_bytes
        .checked_sub(KAGEMUSHA_HALO2_LENGTH_PREFIX_BYTES_V4)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| format!("Kagemusha V4 {role} parameter payload length overflows"))?;
    cursor.skip(payload_bytes, "parameter points")?;
    cursor.finish()
}

fn validate_kagemusha_processed_vk_prefix_v4(
    cursor: &mut KagemushaStructuralCursorV4<'_>,
    shape: KagemushaProcessedKeyShapeV4,
) -> Result<(), String> {
    let version = cursor.read_u8("verifier-key version")?;
    if version != KAGEMUSHA_HALO2_KEY_VERSION_V4 {
        return Err(format!(
            "Kagemusha V4 {} verifier-key version {version:#04x} is unsupported",
            cursor.role
        ));
    }
    let encoded_k = cursor.read_u32_le("verifier-key degree")?;
    if encoded_k != shape.k {
        return Err(format!(
            "Kagemusha V4 {} verifier-key degree {encoded_k} does not match authenticated degree {}",
            cursor.role, shape.k
        ));
    }
    let compress_selectors = cursor.read_u8("selector-compression flag")?;
    if compress_selectors != KAGEMUSHA_HALO2_UNCOMPRESSED_SELECTORS_V4 {
        return Err(format!(
            "Kagemusha V4 {} verifier key does not use the canonical uncompressed-selector encoding",
            cursor.role
        ));
    }
    let fixed_polynomials = cursor.read_u32_le("fixed-commitment count")?;
    let expected_fixed = shape.fixed_polynomials_u32(cursor.role)?;
    if fixed_polynomials != expected_fixed {
        return Err(format!(
            "Kagemusha V4 {} fixed-commitment count {fixed_polynomials} does not match authenticated shape {expected_fixed}",
            cursor.role
        ));
    }
    let commitment_count = shape
        .fixed_polynomials
        .checked_add(shape.permutation_polynomials)
        .ok_or_else(|| format!("Kagemusha V4 {} commitment count overflows", cursor.role))?;
    let commitment_bytes = commitment_count
        .checked_mul(shape.point_bytes)
        .ok_or_else(|| {
            format!(
                "Kagemusha V4 {} commitment byte length overflows",
                cursor.role
            )
        })?;
    cursor.skip(commitment_bytes, "verifier-key commitments")
}

fn validate_kagemusha_processed_vk_encoding_v4(
    bytes: &[u8],
    shape: KagemushaProcessedKeyShapeV4,
    role: &str,
) -> Result<(), String> {
    let mut cursor = KagemushaStructuralCursorV4::new(bytes, role);
    validate_kagemusha_processed_vk_prefix_v4(&mut cursor, shape)?;
    cursor.finish()
}

#[cfg(test)]
fn validate_kagemusha_processed_polynomial_v4(
    cursor: &mut KagemushaStructuralCursorV4<'_>,
    shape: KagemushaProcessedKeyShapeV4,
    field: &str,
) -> Result<(), String> {
    let encoded_len = cursor.read_u32_be(field)?;
    if encoded_len != shape.domain_rows {
        return Err(format!(
            "Kagemusha V4 {} {field} length {encoded_len} does not match authenticated domain size {}",
            cursor.role, shape.domain_rows
        ));
    }
    let value_bytes = usize::try_from(shape.domain_rows)
        .ok()
        .and_then(|rows| rows.checked_mul(shape.scalar_bytes))
        .ok_or_else(|| format!("Kagemusha V4 {} {field} byte length overflows", cursor.role))?;
    cursor.skip(value_bytes, field)
}

#[cfg(test)]
fn validate_kagemusha_processed_polynomial_vec_v4(
    cursor: &mut KagemushaStructuralCursorV4<'_>,
    shape: KagemushaProcessedKeyShapeV4,
    expected_count: usize,
    field: &str,
) -> Result<(), String> {
    let encoded_count = cursor.read_u32_be(field)?;
    let expected_count_u32 = u32::try_from(expected_count).map_err(|_| {
        format!(
            "Kagemusha V4 {} {field} count does not fit u32",
            cursor.role
        )
    })?;
    if encoded_count != expected_count_u32 {
        return Err(format!(
            "Kagemusha V4 {} {field} count {encoded_count} does not match authenticated shape {expected_count_u32}",
            cursor.role
        ));
    }
    for _ in 0..expected_count {
        validate_kagemusha_processed_polynomial_v4(cursor, shape, field)?;
    }
    Ok(())
}

#[cfg(test)]
fn validate_kagemusha_processed_pk_encoding_v4(
    bytes: &[u8],
    shape: KagemushaProcessedKeyShapeV4,
    role: &str,
) -> Result<(), String> {
    let mut cursor = KagemushaStructuralCursorV4::new(bytes, role);
    validate_kagemusha_processed_vk_prefix_v4(&mut cursor, shape)?;
    validate_kagemusha_processed_polynomial_v4(&mut cursor, shape, "l0 polynomial")?;
    validate_kagemusha_processed_polynomial_v4(&mut cursor, shape, "l_last polynomial")?;
    validate_kagemusha_processed_polynomial_v4(&mut cursor, shape, "l_active_row polynomial")?;
    validate_kagemusha_processed_polynomial_vec_v4(
        &mut cursor,
        shape,
        shape.fixed_polynomials,
        "fixed-value polynomials",
    )?;
    validate_kagemusha_processed_polynomial_vec_v4(
        &mut cursor,
        shape,
        shape.fixed_polynomials,
        "fixed coefficient polynomials",
    )?;
    validate_kagemusha_processed_polynomial_vec_v4(
        &mut cursor,
        shape,
        shape.permutation_polynomials,
        "permutation Lagrange polynomials",
    )?;
    validate_kagemusha_processed_polynomial_vec_v4(
        &mut cursor,
        shape,
        shape.permutation_polynomials,
        "permutation coefficient polynomials",
    )?;
    cursor.finish()
}

#[derive(Clone, Copy, Debug)]
struct KagemushaConfiguredVkWireShapeV4 {
    k: u32,
    domain_size: u64,
    advice_columns: usize,
    base_fixed_columns: usize,
    selectors: usize,
    permutation_columns: usize,
    instance_columns: usize,
    curve_bytes: usize,
    scalar_bytes: usize,
}

#[derive(Clone, Copy, Debug)]
struct KagemushaVkWirePreflightV4 {
    serialized_len: u64,
    fixed_columns: usize,
    permutation_columns: usize,
}

struct KagemushaWireScannerV4<'reader> {
    reader: &'reader mut dyn std::io::Read,
    consumed: u64,
    consumed_sha256: Option<Sha256>,
    role: &'reader str,
}

impl<'reader> KagemushaWireScannerV4<'reader> {
    fn new(reader: &'reader mut dyn std::io::Read, role: &'reader str) -> Self {
        Self {
            reader,
            consumed: 0,
            consumed_sha256: Some(Sha256::new()),
            role,
        }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut bytes = [0_u8; N];
        self.reader.read_exact(&mut bytes).map_err(|error| {
            format!(
                "failed to preflight Kagemusha V4 {} at byte {}: {error}",
                self.role, self.consumed
            )
        })?;
        self.consumed =
            self.consumed
                .checked_add(u64::try_from(N).map_err(|_| {
                    format!("Kagemusha V4 {} read width does not fit u64", self.role)
                })?)
                .ok_or_else(|| format!("Kagemusha V4 {} length overflow", self.role))?;
        if let Some(hasher) = &mut self.consumed_sha256 {
            hasher.update(&bytes);
        }
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_array::<1>()?[0])
    }

    fn read_u32_le(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u32_be(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    fn skip_exact(&mut self, bytes: u64) -> Result<(), String> {
        let mut remaining = bytes;
        let mut scratch = [0_u8; 64 * 1024];
        while remaining != 0 {
            let chunk = usize::try_from(remaining.min(scratch.len() as u64))
                .map_err(|_| format!("Kagemusha V4 {} skip width does not fit usize", self.role))?;
            self.reader
                .read_exact(&mut scratch[..chunk])
                .map_err(|error| {
                    format!(
                        "failed to preflight Kagemusha V4 {} at byte {}: {error}",
                        self.role, self.consumed
                    )
                })?;
            let chunk_u64 = u64::try_from(chunk)
                .map_err(|_| format!("Kagemusha V4 {} skip width does not fit u64", self.role))?;
            self.consumed = self
                .consumed
                .checked_add(chunk_u64)
                .ok_or_else(|| format!("Kagemusha V4 {} length overflow", self.role))?;
            if let Some(hasher) = &mut self.consumed_sha256 {
                hasher.update(&scratch[..chunk]);
            }
            remaining -= chunk_u64;
        }
        Ok(())
    }

    fn finish_consumed_sha256(&mut self) -> Result<[u8; 32], String> {
        self.consumed_sha256
            .take()
            .map(|hasher| hasher.finalize().into())
            .ok_or_else(|| {
                format!(
                    "Kagemusha V4 {} consumed-byte digest was already finalized",
                    self.role
                )
            })
    }
}

fn kagemusha_wire_product_v4(
    count: usize,
    element_bytes: usize,
    role: &str,
) -> Result<u64, String> {
    count
        .checked_mul(element_bytes)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| format!("Kagemusha V4 {role} byte length overflow"))
}

fn configured_kagemusha_eq_vk_wire_shape_v4(
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaConfiguredVkWireShapeV4, String> {
    use halo2_proofs::plonk::{Circuit as _, ConstraintSystem};

    validate_kagemusha_circuit_params_v4(circuit_params)?;
    let mut cs = ConstraintSystem::<Fp>::default();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        KagemushaStepEqCircuitV4::configure_with_params(&mut cs, circuit_params.clone())
    }))
    .map_err(|_| "Kagemusha V4 Eq verifier-key configuration panicked".to_owned())?;
    let domain_size = 1_u64
        .checked_shl(circuit_params.k)
        .ok_or_else(|| "Kagemusha V4 Eq verifier-key domain size overflow".to_owned())?;
    Ok(KagemushaConfiguredVkWireShapeV4 {
        k: circuit_params.k,
        domain_size,
        advice_columns: cs.num_advice_columns(),
        base_fixed_columns: cs.num_fixed_columns(),
        selectors: cs.num_selectors(),
        permutation_columns: cs.permutation().get_columns().len(),
        instance_columns: cs.num_instance_columns(),
        curve_bytes: 32,
        scalar_bytes: 32,
    })
}

fn configured_kagemusha_ep_vk_wire_shape_v4(
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaConfiguredVkWireShapeV4, String> {
    use halo2_proofs::plonk::{Circuit as _, ConstraintSystem};

    validate_kagemusha_circuit_params_v4(circuit_params)?;
    let mut cs = ConstraintSystem::<Fq>::default();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        KagemushaStepEpCircuitV4::configure_with_params(&mut cs, circuit_params.clone())
    }))
    .map_err(|_| "Kagemusha V4 Ep verifier-key configuration panicked".to_owned())?;
    let domain_size = 1_u64
        .checked_shl(circuit_params.k)
        .ok_or_else(|| "Kagemusha V4 Ep verifier-key domain size overflow".to_owned())?;
    Ok(KagemushaConfiguredVkWireShapeV4 {
        k: circuit_params.k,
        domain_size,
        advice_columns: cs.num_advice_columns(),
        base_fixed_columns: cs.num_fixed_columns(),
        selectors: cs.num_selectors(),
        permutation_columns: cs.permutation().get_columns().len(),
        instance_columns: cs.num_instance_columns(),
        curve_bytes: 32,
        scalar_bytes: 32,
    })
}

fn preflight_kagemusha_processed_vk_v4(
    scanner: &mut KagemushaWireScannerV4<'_>,
    shape: KagemushaConfiguredVkWireShapeV4,
    exact_payload_len: Option<u64>,
) -> Result<KagemushaVkWirePreflightV4, String> {
    let version = scanner.read_u8()?;
    let k = scanner.read_u32_le()?;
    let compress_selectors = match scanner.read_u8()? {
        0 => false,
        1 => true,
        _ => {
            return Err(format!(
                "Kagemusha V4 {} selector-compression flag is not boolean",
                scanner.role
            ));
        }
    };
    let fixed_columns = usize::try_from(scanner.read_u32_le()?).map_err(|_| {
        format!(
            "Kagemusha V4 {} fixed-column count does not fit usize",
            scanner.role
        )
    })?;
    let maximum_fixed_columns = shape
        .base_fixed_columns
        .checked_add(shape.selectors)
        .ok_or_else(|| format!("Kagemusha V4 {} fixed-column bound overflow", scanner.role))?;
    let expected_uncompressed_fixed = maximum_fixed_columns;
    let fixed_count_is_valid = if compress_selectors {
        (shape.base_fixed_columns..=maximum_fixed_columns).contains(&fixed_columns)
    } else {
        fixed_columns == expected_uncompressed_fixed
    };
    if version != 0x02 || k != shape.k || !fixed_count_is_valid {
        return Err(format!(
            "Kagemusha V4 {} verifier-key prefix/profile mismatch",
            scanner.role
        ));
    }

    scanner.skip_exact(kagemusha_wire_product_v4(
        fixed_columns,
        shape.curve_bytes,
        scanner.role,
    )?)?;
    scanner.skip_exact(kagemusha_wire_product_v4(
        shape.permutation_columns,
        shape.curve_bytes,
        scanner.role,
    )?)?;
    if compress_selectors {
        let selector_bytes_per_column = shape
            .domain_size
            .checked_add(7)
            .ok_or_else(|| format!("Kagemusha V4 {} selector length overflow", scanner.role))?
            / 8;
        scanner.skip_exact(
            selector_bytes_per_column
                .checked_mul(u64::try_from(shape.selectors).map_err(|_| {
                    format!(
                        "Kagemusha V4 {} selector count does not fit u64",
                        scanner.role
                    )
                })?)
                .ok_or_else(|| {
                    format!("Kagemusha V4 {} selector payload overflow", scanner.role)
                })?,
        )?;
    }
    if exact_payload_len.is_some_and(|expected| scanner.consumed != expected) {
        return Err(format!(
            "Kagemusha V4 {} verifier-key length is not the exact configured wire length",
            scanner.role
        ));
    }
    Ok(KagemushaVkWirePreflightV4 {
        serialized_len: scanner.consumed,
        fixed_columns,
        permutation_columns: shape.permutation_columns,
    })
}

fn preflight_kagemusha_polynomial_v4(
    scanner: &mut KagemushaWireScannerV4<'_>,
    expected_len: u64,
    scalar_bytes: usize,
) -> Result<(), String> {
    if u64::from(scanner.read_u32_be()?) != expected_len {
        return Err(format!(
            "Kagemusha V4 {} polynomial length does not match its authenticated domain",
            scanner.role
        ));
    }
    let scalar_bytes = u64::try_from(scalar_bytes).map_err(|_| {
        format!(
            "Kagemusha V4 {} scalar width does not fit u64",
            scanner.role
        )
    })?;
    scanner.skip_exact(expected_len.checked_mul(scalar_bytes).ok_or_else(|| {
        format!(
            "Kagemusha V4 {} polynomial byte length overflow",
            scanner.role
        )
    })?)
}

fn preflight_kagemusha_polynomial_vec_v4(
    scanner: &mut KagemushaWireScannerV4<'_>,
    expected_count: usize,
    expected_polynomial_len: u64,
    scalar_bytes: usize,
) -> Result<(), String> {
    let actual_count = usize::try_from(scanner.read_u32_be()?).map_err(|_| {
        format!(
            "Kagemusha V4 {} polynomial count does not fit usize",
            scanner.role
        )
    })?;
    if actual_count != expected_count {
        return Err(format!(
            "Kagemusha V4 {} polynomial-vector count does not match its embedded verifier key",
            scanner.role
        ));
    }
    for _ in 0..actual_count {
        preflight_kagemusha_polynomial_v4(scanner, expected_polynomial_len, scalar_bytes)?;
    }
    Ok(())
}

fn parse_kagemusha_params_from_source_v4<C>(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    parity: KagemushaPastaCycleParityV1,
    expected_k: u32,
    role: &'static str,
) -> Result<halo2_proofs::poly::ipa::commitment::ParamsIPA<C>, String>
where
    C: CurveAffine,
{
    use halo2_proofs::poly::commitment::Params as _;

    let domain_size = 1_u64
        .checked_shl(expected_k)
        .ok_or_else(|| format!("Kagemusha V4 {role} ParamsIPA domain size overflow"))?;
    let expected_len = domain_size
        .checked_mul(2)
        .and_then(|points| points.checked_add(2))
        .and_then(|points| points.checked_mul(32))
        .and_then(|point_bytes| point_bytes.checked_add(4))
        .ok_or_else(|| format!("Kagemusha V4 {role} ParamsIPA length overflow"))?;
    super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        parity,
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
        |reader, header| {
            if header.payload_size_bytes != expected_len {
                return Err(format!(
                    "Kagemusha V4 {role} ParamsIPA payload length does not match degree {expected_k}"
                ));
            }
            let mut k_bytes = [0_u8; 4];
            reader.read_exact(&mut k_bytes).map_err(|error| {
                format!("failed to read Kagemusha V4 {role} ParamsIPA degree: {error}")
            })?;
            if u32::from_le_bytes(k_bytes) != expected_k {
                return Err(format!("Kagemusha V4 {role} ParamsIPA degree mismatch"));
            }
            let mut replay = std::io::Cursor::new(k_bytes).chain(reader);
            let params = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                halo2_proofs::poly::ipa::commitment::ParamsIPA::<C>::read(&mut replay)
            }))
            .map_err(|_| format!("Kagemusha V4 {role} ParamsIPA reader panicked"))?
            .map_err(|error| {
                format!("failed to parse Kagemusha V4 {role} parameters: {error}")
            })?;
            if params.k() != expected_k {
                return Err(format!("Kagemusha V4 {role} ParamsIPA degree mismatch"));
            }
            Ok(params)
        },
    )
}

pub(crate) fn load_kagemusha_eq_params_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    expected_k: u32,
) -> Result<
    halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    String,
> {
    parse_kagemusha_params_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
        expected_k,
        "Eq",
    )
}

pub(crate) fn load_kagemusha_ep_params_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    expected_k: u32,
) -> Result<
    halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    String,
> {
    parse_kagemusha_params_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
        expected_k,
        "Ep",
    )
}

fn read_bounded_kagemusha_bootstrap_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<Vec<u8>, String> {
    super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        parity,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        |reader, header| {
            if header.payload_size_bytes == 0
                || header.payload_size_bytes
                    > u64::from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4)
            {
                return Err("Kagemusha V4 bootstrap payload exceeds its proof-pair bound".to_owned());
            }
            let len = usize::try_from(header.payload_size_bytes)
                .map_err(|_| "Kagemusha V4 bootstrap length does not fit usize".to_owned())?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(len).map_err(|_| {
                "failed to reserve bounded Kagemusha V4 bootstrap payload".to_owned()
            })?;
            bytes.resize(len, 0);
            reader.read_exact(&mut bytes).map_err(|error| {
                format!("failed to read bounded Kagemusha V4 bootstrap payload: {error}")
            })?;
            Ok(bytes)
        },
    )
}

#[cfg(test)]
mod source_parser_preflight_tests {
    use std::io::Cursor;

    use sha2::{Digest as _, Sha256};

    use super::{
        KagemushaConfiguredVkWireShapeV4, KagemushaPkWirePreflightV4, KagemushaVkWirePreflightV4,
        KagemushaWireScannerV4, ensure_kagemusha_pk_preflight_matches_vk_v4,
        preflight_kagemusha_polynomial_v4, preflight_kagemusha_polynomial_vec_v4,
        preflight_kagemusha_processed_vk_v4,
    };

    fn shape() -> KagemushaConfiguredVkWireShapeV4 {
        KagemushaConfiguredVkWireShapeV4 {
            k: 8,
            domain_size: 256,
            advice_columns: 7,
            base_fixed_columns: 2,
            selectors: 3,
            permutation_columns: 4,
            instance_columns: 1,
            curve_bytes: 32,
            scalar_bytes: 32,
        }
    }

    fn uncompressed_vk_bytes() -> Vec<u8> {
        let shape = shape();
        let fixed_columns = shape.base_fixed_columns + shape.selectors;
        let mut bytes = vec![0x02];
        bytes.extend_from_slice(&shape.k.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(fixed_columns as u32).to_le_bytes());
        bytes.resize(
            bytes.len() + (fixed_columns + shape.permutation_columns) * shape.curve_bytes,
            0,
        );
        bytes
    }

    #[test]
    fn verifier_key_preflight_rejects_malformed_k_count_and_length() {
        let valid = uncompressed_vk_bytes();
        let mut cursor = Cursor::new(valid.as_slice());
        let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "test VK");
        let parsed =
            preflight_kagemusha_processed_vk_v4(&mut scanner, shape(), Some(valid.len() as u64))
                .expect("configured VK wire");
        assert_eq!(parsed.fixed_columns, 5);
        let expected_digest: [u8; 32] = Sha256::digest(&valid).into();
        assert_eq!(
            scanner
                .finish_consumed_sha256()
                .expect("first digest finalization"),
            expected_digest
        );

        let mut bad_k = valid.clone();
        bad_k[1..5].copy_from_slice(&31_u32.to_le_bytes());
        let mut cursor = Cursor::new(bad_k.as_slice());
        let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "bad-k VK");
        assert!(
            preflight_kagemusha_processed_vk_v4(&mut scanner, shape(), Some(bad_k.len() as u64),)
                .is_err()
        );

        let mut bad_count = valid.clone();
        bad_count[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut cursor = Cursor::new(bad_count.as_slice());
        let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "bad-count VK");
        assert!(
            preflight_kagemusha_processed_vk_v4(
                &mut scanner,
                shape(),
                Some(bad_count.len() as u64),
            )
            .is_err(),
            "attacker count must fail before any count-sized allocation"
        );

        let mut trailing = valid.clone();
        trailing.push(0);
        let mut cursor = Cursor::new(trailing.as_slice());
        let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "trailing VK");
        assert!(
            preflight_kagemusha_processed_vk_v4(
                &mut scanner,
                shape(),
                Some(trailing.len() as u64),
            )
            .is_err()
        );
    }

    #[test]
    fn proving_key_preflight_rejects_polynomial_length_and_vector_count_before_allocation() {
        let mut malicious_polynomial = Cursor::new(u32::MAX.to_be_bytes());
        let mut scanner =
            KagemushaWireScannerV4::new(&mut malicious_polynomial, "malicious polynomial");
        assert!(preflight_kagemusha_polynomial_v4(&mut scanner, 256, 32).is_err());
        assert_eq!(scanner.consumed, 4);

        let mut malicious_count = Cursor::new(u32::MAX.to_be_bytes());
        let mut scanner =
            KagemushaWireScannerV4::new(&mut malicious_count, "malicious polynomial vector");
        assert!(preflight_kagemusha_polynomial_vec_v4(&mut scanner, 4, 256, 32).is_err());
        assert_eq!(scanner.consumed, 4);
    }

    #[test]
    fn wire_scanner_stops_hashing_after_digest_finalization() {
        let bytes = [1_u8, 2, 3, 4, 5, 6, 7, 8];
        let mut cursor = Cursor::new(bytes.as_slice());
        let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "digest boundary");
        assert_eq!(
            scanner.read_array::<4>().expect("digest prefix"),
            [1, 2, 3, 4]
        );
        let expected_prefix_digest: [u8; 32] = Sha256::digest([1_u8, 2, 3, 4]).into();
        assert_eq!(
            scanner
                .finish_consumed_sha256()
                .expect("first digest finalization"),
            expected_prefix_digest
        );
        scanner.skip_exact(4).expect("unhashed suffix scan");
        assert_eq!(scanner.consumed, bytes.len() as u64);
        assert!(scanner.finish_consumed_sha256().is_err());
    }

    #[test]
    fn proving_key_preflight_binds_the_exact_standalone_verifier_key() {
        let expected = [0x51; 32];
        let preflight = KagemushaPkWirePreflightV4 {
            vk: KagemushaVkWirePreflightV4 {
                serialized_len: 35_018,
                fixed_columns: 560,
                permutation_columns: 534,
            },
            embedded_verifying_key_sha256: expected,
        };
        ensure_kagemusha_pk_preflight_matches_vk_v4(&preflight, expected, "Eq")
            .expect("exact embedded verifier identity");
        assert!(
            ensure_kagemusha_pk_preflight_matches_vk_v4(&preflight, [0x52; 32], "Eq")
                .expect_err("substituted standalone verifier must fail")
                .contains("different verifier key")
        );
    }
}

fn parse_kagemusha_params_v4<C>(
    bytes: &[u8],
    expected_k: u32,
    role: &str,
) -> Result<halo2_proofs::poly::ipa::commitment::ParamsIPA<C>, String>
where
    C: CurveAffine,
{
    use halo2_proofs::poly::commitment::Params as _;

    validate_kagemusha_params_encoding_v4::<C>(bytes, expected_k, role)?;
    let mut cursor = std::io::Cursor::new(bytes);
    let params = halo2_proofs::poly::ipa::commitment::ParamsIPA::<C>::read(&mut cursor)
        .map_err(|error| format!("failed to parse Kagemusha V4 {role} parameters: {error}"))?;
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| format!("Kagemusha V4 {role} parameter length does not fit u64"))?
        || params.k() != expected_k
    {
        return Err(format!(
            "Kagemusha V4 {role} parameters have a trailing byte or degree mismatch"
        ));
    }
    Ok(params)
}

fn parse_kagemusha_eq_vk_v4(
    bytes: &[u8],
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::VerifyingKey};

    let shape = kagemusha_processed_key_shape_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &circuit_params,
        "Eq",
    )?;
    validate_kagemusha_processed_vk_encoding_v4(bytes, shape, "Eq")?;
    let mut cursor = std::io::Cursor::new(bytes);
    #[cfg(feature = "circuit-params")]
    let key = VerifyingKey::read::<_, KagemushaStepEqCircuitV4>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to parse Kagemusha V4 Eq verifier key: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        VerifyingKey::read::<_, KagemushaStepEqCircuitV4>(&mut cursor, SerdeFormat::Processed)
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq verifier key: {error}"))?
    };
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| "Kagemusha V4 Eq verifier-key length does not fit u64".to_owned())?
    {
        return Err("Kagemusha V4 Eq verifier key has trailing bytes".to_owned());
    }
    Ok(key)
}

fn parse_kagemusha_ep_vk_v4(
    bytes: &[u8],
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::VerifyingKey};

    let shape = kagemusha_processed_key_shape_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &circuit_params,
        "Ep",
    )?;
    validate_kagemusha_processed_vk_encoding_v4(bytes, shape, "Ep")?;
    let mut cursor = std::io::Cursor::new(bytes);
    #[cfg(feature = "circuit-params")]
    let key = VerifyingKey::read::<_, KagemushaStepEpCircuitV4>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to parse Kagemusha V4 Ep verifier key: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        VerifyingKey::read::<_, KagemushaStepEpCircuitV4>(&mut cursor, SerdeFormat::Processed)
            .map_err(|error| format!("failed to parse Kagemusha V4 Ep verifier key: {error}"))?
    };
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| "Kagemusha V4 Ep verifier-key length does not fit u64".to_owned())?
    {
        return Err("Kagemusha V4 Ep verifier key has trailing bytes".to_owned());
    }
    Ok(key)
}

struct KagemushaVkDigestingReaderV4<'reader> {
    inner: &'reader mut dyn std::io::Read,
    raw: Sha256,
    domain_separated: Sha256,
}

impl<'reader> KagemushaVkDigestingReaderV4<'reader> {
    fn new(inner: &'reader mut dyn std::io::Read, payload_len: u64) -> Self {
        let backend = super::ZK_BACKEND_HALO2_IPA;
        let mut domain_separated = Sha256::new();
        domain_separated.update(b"iroha:zk:v1:vk");
        domain_separated.update((backend.len() as u64).to_be_bytes());
        domain_separated.update(backend.as_bytes());
        domain_separated.update(payload_len.to_be_bytes());
        Self {
            inner,
            raw: Sha256::new(),
            domain_separated,
        }
    }

    fn finish(self) -> ([u8; 32], [u8; 32]) {
        (
            self.raw.finalize().into(),
            self.domain_separated.finalize().into(),
        )
    }
}

impl std::io::Read for KagemushaVkDigestingReaderV4<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.raw.update(&buffer[..read]);
        self.domain_separated.update(&buffer[..read]);
        Ok(read)
    }
}

struct KagemushaSha256WriterV4(Sha256);

impl Default for KagemushaSha256WriterV4 {
    fn default() -> Self {
        Self(Sha256::new())
    }
}

impl std::io::Write for KagemushaSha256WriterV4 {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn canonical_kagemusha_vk_sha256_v4<C>(
    key: &halo2_proofs::plonk::VerifyingKey<C>,
    role: &str,
) -> Result<[u8; 32], String>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    use halo2_proofs::SerdeFormat;

    let mut writer = KagemushaSha256WriterV4::default();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        key.write(&mut writer, SerdeFormat::Processed)
    }))
    .map_err(|_| format!("Kagemusha V4 {role} verifier-key serialization panicked"))?
    .map_err(|error| format!("failed to serialize Kagemusha V4 {role} verifier key: {error}"))?;
    Ok(writer.0.finalize().into())
}

pub(crate) struct KagemushaLoadedVerifyingKeyV4<C: CurveAffine> {
    pub(crate) key: halo2_proofs::plonk::VerifyingKey<C>,
    pub(crate) processed_len: u64,
    pub(crate) processed_sha256: [u8; 32],
    pub(crate) commitment: [u8; 32],
}

fn preflight_kagemusha_vk_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    parity: KagemushaPastaCycleParityV1,
    shape: KagemushaConfiguredVkWireShapeV4,
    role: &'static str,
) -> Result<KagemushaVkWirePreflightV4, String> {
    super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        parity,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        |reader, header| {
            let mut scanner = KagemushaWireScannerV4::new(reader, role);
            preflight_kagemusha_processed_vk_v4(
                &mut scanner,
                shape,
                Some(header.payload_size_bytes),
            )
        },
    )
}

pub(crate) fn load_kagemusha_eq_verifying_key_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaLoadedVerifyingKeyV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::VerifyingKey};

    let shape = configured_kagemusha_eq_vk_wire_shape_v4(circuit_params)?;
    let preflight = preflight_kagemusha_vk_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
        shape,
        "Eq",
    )?;
    let (key, processed_sha256, commitment) =
        super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
            source,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            |reader, header| {
                if header.payload_size_bytes != preflight.serialized_len {
                    return Err("Kagemusha V4 Eq verifier-key length changed after preflight".to_owned());
                }
                let mut digesting = KagemushaVkDigestingReaderV4::new(
                    reader,
                    header.payload_size_bytes,
                );
                #[cfg(feature = "circuit-params")]
                let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    VerifyingKey::read::<_, KagemushaStepEqCircuitV4>(
                        &mut digesting,
                        SerdeFormat::Processed,
                        circuit_params.clone(),
                    )
                }))
                .map_err(|_| "Kagemusha V4 Eq verifier-key reader panicked".to_owned())?
                .map_err(|error| format!("failed to parse Kagemusha V4 Eq verifier key: {error}"))?;
                #[cfg(not(feature = "circuit-params"))]
                let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    VerifyingKey::read::<_, KagemushaStepEqCircuitV4>(
                        &mut digesting,
                        SerdeFormat::Processed,
                    )
                }))
                .map_err(|_| "Kagemusha V4 Eq verifier-key reader panicked".to_owned())?
                .map_err(|error| format!("failed to parse Kagemusha V4 Eq verifier key: {error}"))?;
                let (processed_sha256, commitment) = digesting.finish();
                Ok((key, processed_sha256, commitment))
            },
    )?;
    if key.fixed_commitments().len() != preflight.fixed_columns
        || key.fixed_commitments().len() != key.cs().num_fixed_columns()
        || key.permutation().commitments().len() != preflight.permutation_columns
        || key.permutation().commitments().len() != key.cs().permutation().get_columns().len()
        || canonical_kagemusha_vk_sha256_v4(&key, "Eq")? != processed_sha256
    {
        return Err(
            "Kagemusha V4 Eq verifier key is not canonical for its configured shape".to_owned(),
        );
    }
    Ok(KagemushaLoadedVerifyingKeyV4 {
        key,
        processed_len: preflight.serialized_len,
        processed_sha256,
        commitment,
    })
}

pub(crate) fn load_kagemusha_ep_verifying_key_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaLoadedVerifyingKeyV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::VerifyingKey};

    let shape = configured_kagemusha_ep_vk_wire_shape_v4(circuit_params)?;
    let preflight = preflight_kagemusha_vk_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
        shape,
        "Ep",
    )?;
    let (key, processed_sha256, commitment) =
        super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
            source,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            |reader, header| {
                if header.payload_size_bytes != preflight.serialized_len {
                    return Err("Kagemusha V4 Ep verifier-key length changed after preflight".to_owned());
                }
                let mut digesting = KagemushaVkDigestingReaderV4::new(
                    reader,
                    header.payload_size_bytes,
                );
                #[cfg(feature = "circuit-params")]
                let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    VerifyingKey::read::<_, KagemushaStepEpCircuitV4>(
                        &mut digesting,
                        SerdeFormat::Processed,
                        circuit_params.clone(),
                    )
                }))
                .map_err(|_| "Kagemusha V4 Ep verifier-key reader panicked".to_owned())?
                .map_err(|error| format!("failed to parse Kagemusha V4 Ep verifier key: {error}"))?;
                #[cfg(not(feature = "circuit-params"))]
                let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    VerifyingKey::read::<_, KagemushaStepEpCircuitV4>(
                        &mut digesting,
                        SerdeFormat::Processed,
                    )
                }))
                .map_err(|_| "Kagemusha V4 Ep verifier-key reader panicked".to_owned())?
                .map_err(|error| format!("failed to parse Kagemusha V4 Ep verifier key: {error}"))?;
                let (processed_sha256, commitment) = digesting.finish();
                Ok((key, processed_sha256, commitment))
            },
    )?;
    if key.fixed_commitments().len() != preflight.fixed_columns
        || key.fixed_commitments().len() != key.cs().num_fixed_columns()
        || key.permutation().commitments().len() != preflight.permutation_columns
        || key.permutation().commitments().len() != key.cs().permutation().get_columns().len()
        || canonical_kagemusha_vk_sha256_v4(&key, "Ep")? != processed_sha256
    {
        return Err(
            "Kagemusha V4 Ep verifier key is not canonical for its configured shape".to_owned(),
        );
    }
    Ok(KagemushaLoadedVerifyingKeyV4 {
        key,
        processed_len: preflight.serialized_len,
        processed_sha256,
        commitment,
    })
}

fn validate_kagemusha_profile_protocol_v4<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<C>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    parity: KagemushaPastaCycleParityV1,
    expected_structure_sha256: [u8; 32],
    bootstrap_bytes: &[u8],
) -> Result<(KagemushaStepBootstrapV4, [u8; 32], PlonkProtocol<C>), String>
where
    C: CurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField + PrimeField,
{
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public layout does not fit usize".to_owned())?;
    let final_protocol = snark_verifier::system::halo2::compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    // The authenticated release binds the bootstrap payload, including its
    // generation-time protocol identity. Re-generating the bootstrap VK here
    // used a full keygen allocation on every runtime load without adding a new
    // trust boundary: the final VK and bootstrap artifact are both already
    // covered by the signed release. Validate the final protocol's exact
    // structure and terminally verify the bootstrap equations below instead.
    let actual_structure = kagemusha_compiled_protocol_structure_sha256(&final_protocol, parity)?;
    if expected_structure_sha256 == [0; 32] || actual_structure != expected_structure_sha256 {
        return Err("Kagemusha V4 compiled protocol structure mismatch".to_owned());
    }
    let bootstrap = KagemushaStepBootstrapV4::decode_authenticated(
        bootstrap_bytes,
        circuit_params,
        parity,
        expected_structure_sha256,
    )?;
    let final_identity = kagemusha_compiled_protocol_identity_sha256(&final_protocol, parity)?;
    Ok((bootstrap, final_identity, final_protocol))
}

/// Terminally verify every Eq bootstrap equation before the payload can enter
/// a recursive witness. The ordinary selector-zero proof is generated by the
/// final Step proving key and is therefore verified by the final Step VK. The
/// authenticated bootstrap payload records the generation-time protocol
/// identity, while runtime validates the final protocol structure without
/// regenerating any key. The all-zero parent has no carried public lineage, so
/// the circuit selects `current`;
/// nevertheless both fixed-shape fold stages execute and must be valid for
/// `(current, current)`.
fn terminal_validate_kagemusha_eq_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    bootstrap: &KagemushaStepBootstrapV4,
) -> Result<(), String> {
    let instances = bootstrap
        .parent_slot
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|limb| Fp::from(u64::from(*limb)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let exact_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Eq bootstrap proof length does not fit usize".to_owned())?;
    let current = succinct_verify_step_eq_instances(
        params,
        step_verifying_key,
        &bootstrap.parent_slot.ordinary_proof_bytes,
        &instances,
        exact_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    let current_wire = KagemushaIpaAccumulatorWireV4::from_eq(&current, circuit_params.k)?;
    if bootstrap.parent_slot.carried_lineage != current_wire {
        return Err(
            "Kagemusha V4 Eq bootstrap carried lineage is not its proof lineage".to_owned(),
        );
    }
    let carried = bootstrap
        .parent_slot
        .carried_lineage
        .to_eq(circuit_params.k)?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(carried),
        &bootstrap.parent_slot.post_proof_fold,
    )?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(current),
        &bootstrap.branch_merge_fold,
    )?;
    Ok(())
}

/// Ep/Pallas analogue of [`terminal_validate_kagemusha_eq_bootstrap_v4`].
fn terminal_validate_kagemusha_ep_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    bootstrap: &KagemushaStepBootstrapV4,
) -> Result<(), String> {
    let instances = bootstrap
        .parent_slot
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|limb| Fq::from(u64::from(*limb)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let exact_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Ep bootstrap proof length does not fit usize".to_owned())?;
    let current = succinct_verify_step_ep_instances(
        params,
        step_verifying_key,
        &bootstrap.parent_slot.ordinary_proof_bytes,
        &instances,
        exact_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    let current_wire = KagemushaIpaAccumulatorWireV4::from_ep(&current, circuit_params.k)?;
    if bootstrap.parent_slot.carried_lineage != current_wire {
        return Err(
            "Kagemusha V4 Ep bootstrap carried lineage is not its proof lineage".to_owned(),
        );
    }
    let carried = bootstrap
        .parent_slot
        .carried_lineage
        .to_ep(circuit_params.k)?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(carried),
        &bootstrap.parent_slot.post_proof_fold,
    )?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(current),
        &bootstrap.branch_merge_fold,
    )?;
    Ok(())
}

trait KagemushaArtifactPayloadBytesV4 {
    fn payload_bytes(&self) -> &[u8];
}

impl KagemushaArtifactPayloadBytesV4 for &[u8] {
    fn payload_bytes(&self) -> &[u8] {
        self
    }
}

impl KagemushaArtifactPayloadBytesV4
    for super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4
{
    fn payload_bytes(&self) -> &[u8] {
        self.payload()
    }
}

fn with_kagemusha_artifact_payload_v4<P, T, L, Parse>(
    load: &mut L,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
    parse: Parse,
) -> Result<T, String>
where
    P: KagemushaArtifactPayloadBytesV4,
    L: FnMut(KagemushaPastaCycleParityV1, KagemushaPastaCycleArtifactKindV4) -> Result<P, String>,
    Parse: FnOnce(&[u8]) -> Result<T, String>,
{
    let payload = load(parity, kind)?;
    parse(payload.payload_bytes())
}

struct KagemushaPinnedQualificationSourceV4 {
    source: Arc<dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4>,
    authenticated_release: KagemushaAuthenticatedReleaseV4,
}

impl super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4
    for KagemushaPinnedQualificationSourceV4
{
    fn authenticated_release(&self) -> &KagemushaAuthenticatedReleaseV4 {
        &self.authenticated_release
    }

    fn with_framed_artifact(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
        consume: &mut dyn FnMut(
            &mut dyn super::kagemusha_artifact_source_v4::KagemushaArtifactReadSeekV4,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.source.with_framed_artifact(parity, kind, consume)
    }

    fn authenticated_inspection(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
    ) -> Result<
        Option<super::kagemusha_artifact_v4::KagemushaAuthenticatedArtifactInspectionV4>,
        String,
    > {
        self.source.authenticated_inspection(parity, kind)
    }
}

fn qualify_kagemusha_eq_artifacts_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
) -> Result<super::kagemusha_artifact_source_v4::KagemushaQualifiedParityMetadataV4, String> {
    let profile = source
        .authenticated_release()
        .manifest()
        .profiles
        .first()
        .cloned()
        .ok_or_else(|| "Kagemusha V4 authenticated release omits Eq profile".to_owned())?;
    if profile.parity != KagemushaPastaCycleParityV1::StepEq {
        return Err("Kagemusha V4 authenticated Eq profile order mismatch".to_owned());
    }
    let params = load_kagemusha_eq_params_from_source_v4(source, profile.ipa_k)?;
    let verifying_key =
        load_kagemusha_eq_verifying_key_from_source_v4(source, &profile.circuit_params)?;
    let proving_key = preflight_kagemusha_pk_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
        configured_kagemusha_eq_vk_wire_shape_v4(&profile.circuit_params)?,
        "Eq",
    )?;
    ensure_kagemusha_pk_preflight_matches_vk_v4(
        &proving_key,
        verifying_key.processed_sha256,
        "Eq",
    )?;
    let bootstrap_bytes = read_bounded_kagemusha_bootstrap_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let (bootstrap, compiled_protocol_identity_sha256, compiled_protocol) =
        validate_kagemusha_profile_protocol_v4(
            &params,
            &verifying_key.key,
            &profile.circuit_params,
            KagemushaPastaCycleParityV1::StepEq,
            profile.compiled_protocol_structure_sha256,
            &bootstrap_bytes,
        )?;
    terminal_validate_kagemusha_eq_bootstrap_v4(
        &params,
        &verifying_key.key,
        &profile.circuit_params,
        &bootstrap,
    )?;
    drop(compiled_protocol);
    super::kagemusha_artifact_source_v4::KagemushaQualifiedParityMetadataV4::new(
        KagemushaPastaCycleParityV1::StepEq,
        profile.circuit_params,
        compiled_protocol_identity_sha256,
        verifying_key.processed_len,
        verifying_key.processed_sha256,
        verifying_key.commitment,
        proving_key.embedded_verifying_key_sha256,
        proving_key.vk.fixed_columns,
        proving_key.vk.permutation_columns,
    )
}

fn qualify_kagemusha_ep_artifacts_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
) -> Result<super::kagemusha_artifact_source_v4::KagemushaQualifiedParityMetadataV4, String> {
    let profile = source
        .authenticated_release()
        .manifest()
        .profiles
        .get(1)
        .cloned()
        .ok_or_else(|| "Kagemusha V4 authenticated release omits Ep profile".to_owned())?;
    if profile.parity != KagemushaPastaCycleParityV1::StepEp {
        return Err("Kagemusha V4 authenticated Ep profile order mismatch".to_owned());
    }
    let params = load_kagemusha_ep_params_from_source_v4(source, profile.ipa_k)?;
    let verifying_key =
        load_kagemusha_ep_verifying_key_from_source_v4(source, &profile.circuit_params)?;
    let proving_key = preflight_kagemusha_pk_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
        configured_kagemusha_ep_vk_wire_shape_v4(&profile.circuit_params)?,
        "Ep",
    )?;
    ensure_kagemusha_pk_preflight_matches_vk_v4(
        &proving_key,
        verifying_key.processed_sha256,
        "Ep",
    )?;
    let bootstrap_bytes = read_bounded_kagemusha_bootstrap_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let (bootstrap, compiled_protocol_identity_sha256, compiled_protocol) =
        validate_kagemusha_profile_protocol_v4(
            &params,
            &verifying_key.key,
            &profile.circuit_params,
            KagemushaPastaCycleParityV1::StepEp,
            profile.compiled_protocol_structure_sha256,
            &bootstrap_bytes,
        )?;
    terminal_validate_kagemusha_ep_bootstrap_v4(
        &params,
        &verifying_key.key,
        &profile.circuit_params,
        &bootstrap,
    )?;
    drop(compiled_protocol);
    super::kagemusha_artifact_source_v4::KagemushaQualifiedParityMetadataV4::new(
        KagemushaPastaCycleParityV1::StepEp,
        profile.circuit_params,
        compiled_protocol_identity_sha256,
        verifying_key.processed_len,
        verifying_key.processed_sha256,
        verifying_key.commitment,
        proving_key.embedded_verifying_key_sha256,
        proving_key.vk.fixed_columns,
        proving_key.vk.permutation_columns,
    )
}

pub(super) fn qualify_kagemusha_authenticated_artifact_source_v4(
    source: Arc<dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4>,
    authenticated_release: KagemushaAuthenticatedReleaseV4,
) -> Result<super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4, String> {
    authenticated_release
        .manifest()
        .validate()
        .map_err(|error| format!("invalid authenticated Kagemusha V4 manifest: {error}"))?;
    if source.authenticated_release() != &authenticated_release {
        return Err("Kagemusha V4 artifact source release changed before qualification".to_owned());
    }
    let pinned = KagemushaPinnedQualificationSourceV4 {
        source: Arc::clone(&source),
        authenticated_release: authenticated_release.clone(),
    };
    // Each helper drops all domain-sized Eq objects before Ep is opened.
    let step_eq = qualify_kagemusha_eq_artifacts_v4(&pinned)?;
    let step_ep = qualify_kagemusha_ep_artifacts_v4(&pinned)?;
    if step_eq.compiled_protocol_identity_sha256() == step_ep.compiled_protocol_identity_sha256()
        || step_eq.verifying_key_commitment() == step_ep.verifying_key_commitment()
    {
        return Err("Kagemusha V4 Eq/Ep qualified identities are not distinct".to_owned());
    }
    super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4::new(
        source,
        authenticated_release,
        step_eq,
        step_ep,
    )
}

pub(crate) struct KagemushaPastaCycleRuntimeContextV5 {
    step_eq_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    step_ep_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_circuit_params: KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
}

pub(crate) struct KagemushaPastaCycleTerminalVerifierV4 {
    context: std::sync::Arc<KagemushaPastaCycleRuntimeContextV5>,
    step_eq_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
}

impl std::ops::Deref for KagemushaPastaCycleTerminalVerifierV4 {
    type Target = KagemushaPastaCycleRuntimeContextV5;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl KagemushaPastaCycleTerminalVerifierV4 {
    /// Parse and cross-check all verifier roles from one authenticated release.
    pub(crate) fn from_authenticated_artifacts(
        artifacts: &super::kagemusha_artifact_v4::KagemushaPastaCycleVerifierArtifactsV4,
    ) -> Result<Self, String> {
        Self::from_payload_loader(artifacts.manifest(), |parity, kind| match (parity, kind) {
            (KagemushaPastaCycleParityV1::StepEq, KagemushaPastaCycleArtifactKindV4::ParamsIpa) => {
                Ok(artifacts.step_eq_parameters())
            }
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ) => Ok(artifacts.step_eq_verifying_key()),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ) => Ok(artifacts.step_eq_bootstrap_witness()),
            (KagemushaPastaCycleParityV1::StepEp, KagemushaPastaCycleArtifactKindV4::ParamsIpa) => {
                Ok(artifacts.step_ep_parameters())
            }
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ) => Ok(artifacts.step_ep_verifying_key()),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ) => Ok(artifacts.step_ep_bootstrap_witness()),
            (_, KagemushaPastaCycleArtifactKindV4::ProvingKey) => {
                Err("Kagemusha V4 verifier loader requested a proving key".to_owned())
            }
        })
    }

    /// Parse one already role- and manifest-validated carrier at a time.
    pub(crate) fn from_validated_artifact_loader<F>(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        load: F,
    ) -> Result<Self, String>
    where
        F: FnMut(
            KagemushaPastaCycleParityV1,
            KagemushaPastaCycleArtifactKindV4,
        ) -> Result<
            super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4,
            String,
        >,
    {
        Self::from_payload_loader(manifest, load)
    }

    fn from_payload_loader<P, F>(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        mut load: F,
    ) -> Result<Self, String>
    where
        P: KagemushaArtifactPayloadBytesV4,
        F: FnMut(
            KagemushaPastaCycleParityV1,
            KagemushaPastaCycleArtifactKindV4,
        ) -> Result<P, String>,
    {
        let step_eq = manifest
            .profiles
            .first()
            .ok_or_else(|| "Kagemusha V4 Eq release profile is absent".to_owned())?
            .clone();
        let step_ep = manifest
            .profiles
            .get(1)
            .ok_or_else(|| "Kagemusha V4 Ep release profile is absent".to_owned())?
            .clone();
        if step_eq.parity != KagemushaPastaCycleParityV1::StepEq
            || step_ep.parity != KagemushaPastaCycleParityV1::StepEp
        {
            return Err("Kagemusha V4 release profile order mismatch".to_owned());
        }
        let step_eq_params = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            |bytes| {
                parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
                    bytes,
                    step_eq.ipa_k,
                    "Eq",
                )
            },
        )?;
        let step_eq_verifying_key = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            |bytes| parse_kagemusha_eq_vk_v4(bytes, step_eq.circuit_params.clone()),
        )?;
        let step_eq_bootstrap = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            |bytes| {
                Ok(validate_kagemusha_profile_protocol_v4(
                    &step_eq_params,
                    &step_eq_verifying_key,
                    &step_eq.circuit_params,
                    KagemushaPastaCycleParityV1::StepEq,
                    step_eq.compiled_protocol_structure_sha256,
                    bytes,
                )?
                .0)
            },
        )?;
        terminal_validate_kagemusha_eq_bootstrap_v4(
            &step_eq_params,
            &step_eq_verifying_key,
            &step_eq.circuit_params,
            &step_eq_bootstrap,
        )?;
        drop(step_eq_bootstrap);
        let step_ep_params = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            |bytes| {
                parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
                    bytes,
                    step_ep.ipa_k,
                    "Ep",
                )
            },
        )?;
        let step_ep_verifying_key = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            |bytes| parse_kagemusha_ep_vk_v4(bytes, step_ep.circuit_params.clone()),
        )?;
        let step_ep_bootstrap = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            |bytes| {
                Ok(validate_kagemusha_profile_protocol_v4(
                    &step_ep_params,
                    &step_ep_verifying_key,
                    &step_ep.circuit_params,
                    KagemushaPastaCycleParityV1::StepEp,
                    step_ep.compiled_protocol_structure_sha256,
                    bytes,
                )?
                .0)
            },
        )?;
        terminal_validate_kagemusha_ep_bootstrap_v4(
            &step_ep_params,
            &step_ep_verifying_key,
            &step_ep.circuit_params,
            &step_ep_bootstrap,
        )?;
        drop(step_ep_bootstrap);
        Ok(Self {
            context: std::sync::Arc::new(KagemushaPastaCycleRuntimeContextV5 {
                step_eq_params,
                step_eq_circuit_params: step_eq.circuit_params.clone(),
                step_ep_params,
                step_ep_circuit_params: step_ep.circuit_params.clone(),
                max_pair_bytes: manifest.max_proof_bytes,
            }),
            step_eq_verifying_key,
            step_ep_verifying_key,
        })
    }

    /// Decode and terminally decide one opaque ABI-21 pair only after its
    /// complete public state is matched to the caller's canonical statement.
    ///
    /// This keeps fold transcripts and accumulator wires private to the
    /// recursion adapter while giving the public facade a fail-closed binding
    /// check over every value needed by the lifecycle.
    pub(crate) fn verify_encoded_pair_binding(
        &self,
        bytes: &[u8],
        expected_statement: &KagemushaRecursiveSpendPublicStatementV4,
        expected_operation: &KagemushaStepOperationVectorV4,
        expected_statement_digest: [u32; 8],
        expected_state: &[u32],
        expected_proof_step_count: u32,
        expected_manifest_sha256: [u32; 8],
    ) -> Result<(), String> {
        let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
            bytes,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        expected_operation.validate_terminal_statement_v4(expected_statement)?;
        let expected_statement_chunks =
            kagemusha_u32_words_to_u128_chunks_v5(&expected_statement_digest);
        let expected_operation_chunks = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_DOMAIN_V5,
            &expected_operation.limbs,
        );
        let expected_state_chunks = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            expected_state,
        );
        let expected_manifest_chunks =
            kagemusha_u32_words_to_u128_chunks_v5(&expected_manifest_sha256);
        if pair.proof_step_count != expected_proof_step_count
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
                ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2]
                != expected_statement_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5
                ..KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5 + 2]
                != expected_operation_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5
                ..KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + 2]
                != expected_state_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
                != expected_manifest_chunks
        {
            return Err(
                "Kagemusha V4 proof pair does not match the canonical public statement".to_owned(),
            );
        }
        self.verify_pair(&pair)
    }

    /// Decode and terminally decide the generator's unbound live-pair
    /// calibration vector. This is used only to qualify an authenticated
    /// release; lifecycle acceptance must use `verify_encoded_pair_binding`.
    pub(crate) fn verify_encoded_pair_qualification(&self, bytes: &[u8]) -> Result<(), String> {
        let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
            bytes,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        self.verify_pair(&pair)
    }

    /// Fully verify and terminally decide one decoded backend-native V4 pair.
    pub(crate) fn verify_pair(&self, pair: &KagemushaPastaCycleProofPairV4) -> Result<(), String> {
        terminal_verify_proof_pair_v4(
            &self.step_eq_params,
            &self.step_eq_verifying_key,
            &self.step_ep_params,
            &self.step_ep_verifying_key,
            pair,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )
    }
}

/// Structural metadata extracted before parsing one authenticated proving key.
#[derive(Debug)]
struct KagemushaPkWirePreflightV4 {
    vk: KagemushaVkWirePreflightV4,
    embedded_verifying_key_sha256: [u8; 32],
}

fn ensure_kagemusha_pk_preflight_matches_vk_v4(
    preflight: &KagemushaPkWirePreflightV4,
    processed_verifying_key_sha256: [u8; 32],
    role: &str,
) -> Result<(), String> {
    if preflight.embedded_verifying_key_sha256 != processed_verifying_key_sha256 {
        return Err(format!(
            "Kagemusha V4 {role} proving key embeds a different verifier key"
        ));
    }
    Ok(())
}

fn preflight_kagemusha_pk_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    parity: KagemushaPastaCycleParityV1,
    shape: KagemushaConfiguredVkWireShapeV4,
    role: &'static str,
) -> Result<KagemushaPkWirePreflightV4, String> {
    super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        parity,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        |reader, header| {
            let mut scanner = KagemushaWireScannerV4::new(reader, role);
            let vk = preflight_kagemusha_processed_vk_v4(&mut scanner, shape, None)?;
            let embedded_verifying_key_sha256 = scanner.finish_consumed_sha256()?;
            for _ in 0..3 {
                preflight_kagemusha_polynomial_v4(
                    &mut scanner,
                    shape.domain_size,
                    shape.scalar_bytes,
                )?;
            }
            for expected_count in [
                vk.fixed_columns,
                vk.fixed_columns,
                vk.permutation_columns,
                vk.permutation_columns,
            ] {
                preflight_kagemusha_polynomial_vec_v4(
                    &mut scanner,
                    expected_count,
                    shape.domain_size,
                    shape.scalar_bytes,
                )?;
            }
            if scanner.consumed != header.payload_size_bytes {
                return Err(format!(
                    "Kagemusha V4 {role} proving key has trailing, truncated, or structurally unaccounted bytes"
                ));
            }
            Ok(KagemushaPkWirePreflightV4 {
                vk,
                embedded_verifying_key_sha256,
            })
        },
    )
}

pub(crate) struct KagemushaLoadedProvingKeyV4<C: CurveAffine> {
    pub(crate) key: halo2_proofs::plonk::ProvingKey<C>,
}

pub(crate) fn load_kagemusha_eq_proving_key_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
    expected_verifying_key_sha256: [u8; 32],
) -> Result<KagemushaLoadedProvingKeyV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let shape = configured_kagemusha_eq_vk_wire_shape_v4(circuit_params)?;
    let preflight = preflight_kagemusha_pk_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
        shape,
        "Eq",
    )?;
    let key = super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        |mut reader, _header| {
            #[cfg(feature = "circuit-params")]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEqCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                    circuit_params.clone(),
                )
            }))
            .map_err(|_| "Kagemusha V4 Eq proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEqCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                )
            }))
            .map_err(|_| "Kagemusha V4 Eq proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?;
            Ok(key)
        },
    )?;
    let embedded_vk = key.get_vk();
    if embedded_vk.fixed_commitments().len() != preflight.vk.fixed_columns
        || embedded_vk.fixed_commitments().len() != embedded_vk.cs().num_fixed_columns()
        || embedded_vk.permutation().commitments().len() != preflight.vk.permutation_columns
        || embedded_vk.permutation().commitments().len()
            != embedded_vk.cs().permutation().get_columns().len()
        || canonical_kagemusha_vk_sha256_v4(embedded_vk, "Eq proving-key embedded")?
            != expected_verifying_key_sha256
    {
        return Err(
            "Kagemusha V4 Eq proving key embeds a different verifier key or shape".to_owned(),
        );
    }
    Ok(KagemushaLoadedProvingKeyV4 { key })
}

pub(crate) fn load_kagemusha_ep_proving_key_from_source_v4(
    source: &dyn super::kagemusha_artifact_source_v4::KagemushaAuthenticatedArtifactSourceV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
    expected_verifying_key_sha256: [u8; 32],
) -> Result<KagemushaLoadedProvingKeyV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let shape = configured_kagemusha_ep_vk_wire_shape_v4(circuit_params)?;
    let preflight = preflight_kagemusha_pk_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
        shape,
        "Ep",
    )?;
    let key = super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        |mut reader, _header| {
            #[cfg(feature = "circuit-params")]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEpCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                    circuit_params.clone(),
                )
            }))
            .map_err(|_| "Kagemusha V4 Ep proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Ep proving key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEpCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                )
            }))
            .map_err(|_| "Kagemusha V4 Ep proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Ep proving key: {error}"))?;
            Ok(key)
        },
    )?;
    let embedded_vk = key.get_vk();
    if embedded_vk.fixed_commitments().len() != preflight.vk.fixed_columns
        || embedded_vk.fixed_commitments().len() != embedded_vk.cs().num_fixed_columns()
        || embedded_vk.permutation().commitments().len() != preflight.vk.permutation_columns
        || embedded_vk.permutation().commitments().len()
            != embedded_vk.cs().permutation().get_columns().len()
        || canonical_kagemusha_vk_sha256_v4(embedded_vk, "Ep proving-key embedded")?
            != expected_verifying_key_sha256
    {
        return Err(
            "Kagemusha V4 Ep proving key embeds a different verifier key or shape".to_owned(),
        );
    }
    Ok(KagemushaLoadedProvingKeyV4 { key })
}

fn load_kagemusha_eq_proving_key_from_qualified_source_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaLoadedProvingKeyV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let metadata = source.step_eq();
    let key = super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        |mut reader, _header| {
            #[cfg(feature = "circuit-params")]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEqCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                    circuit_params.clone(),
                )
            }))
            .map_err(|_| "Kagemusha V4 Eq proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEqCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                )
            }))
            .map_err(|_| "Kagemusha V4 Eq proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?;
            Ok(key)
        },
    )?;
    let embedded_vk = key.get_vk();
    if embedded_vk.fixed_commitments().len() != metadata.proving_key_fixed_columns()
        || embedded_vk.fixed_commitments().len() != embedded_vk.cs().num_fixed_columns()
        || embedded_vk.permutation().commitments().len()
            != metadata.proving_key_permutation_columns()
        || embedded_vk.permutation().commitments().len()
            != embedded_vk.cs().permutation().get_columns().len()
        || canonical_kagemusha_vk_sha256_v4(embedded_vk, "Eq proving-key embedded")?
            != metadata.proving_key_embedded_verifying_key_sha256()
    {
        return Err(
            "Kagemusha V4 Eq proving key changed after structural qualification".to_owned(),
        );
    }
    Ok(KagemushaLoadedProvingKeyV4 { key })
}

fn load_kagemusha_ep_proving_key_from_qualified_source_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaLoadedProvingKeyV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let metadata = source.step_ep();
    let key = super::kagemusha_artifact_source_v4::with_kagemusha_authenticated_artifact_payload_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        |mut reader, _header| {
            #[cfg(feature = "circuit-params")]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEpCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                    circuit_params.clone(),
                )
            }))
            .map_err(|_| "Kagemusha V4 Ep proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Ep proving key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ProvingKey::read::<_, KagemushaStepEpCircuitV4>(
                    &mut reader,
                    SerdeFormat::Processed,
                )
            }))
            .map_err(|_| "Kagemusha V4 Ep proving-key reader panicked".to_owned())?
            .map_err(|error| format!("failed to parse Kagemusha V4 Ep proving key: {error}"))?;
            Ok(key)
        },
    )?;
    let embedded_vk = key.get_vk();
    if embedded_vk.fixed_commitments().len() != metadata.proving_key_fixed_columns()
        || embedded_vk.fixed_commitments().len() != embedded_vk.cs().num_fixed_columns()
        || embedded_vk.permutation().commitments().len()
            != metadata.proving_key_permutation_columns()
        || embedded_vk.permutation().commitments().len()
            != embedded_vk.cs().permutation().get_columns().len()
        || canonical_kagemusha_vk_sha256_v4(embedded_vk, "Ep proving-key embedded")?
            != metadata.proving_key_embedded_verifying_key_sha256()
    {
        return Err(
            "Kagemusha V4 Ep proving key changed after structural qualification".to_owned(),
        );
    }
    Ok(KagemushaLoadedProvingKeyV4 { key })
}

#[cfg(test)]
fn parse_kagemusha_eq_pk_v4(
    bytes: &[u8],
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let shape = kagemusha_processed_key_shape_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &circuit_params,
        "Eq",
    )?;
    validate_kagemusha_processed_pk_encoding_v4(bytes, shape, "Eq")?;
    let mut cursor = std::io::Cursor::new(bytes);
    #[cfg(feature = "circuit-params")]
    let key = ProvingKey::read::<_, KagemushaStepEqCircuitV4>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        ProvingKey::read::<_, KagemushaStepEqCircuitV4>(&mut cursor, SerdeFormat::Processed)
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?
    };
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| "Kagemusha V4 Eq proving-key length does not fit u64".to_owned())?
    {
        return Err("Kagemusha V4 Eq proving key has trailing bytes".to_owned());
    }
    Ok(key)
}

fn kagemusha_eq_succinct_vk_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
) -> Result<
    snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    String,
> {
    use halo2_proofs::{
        halo2curves::{CurveExt as _, group::Curve as _, pasta::Eq},
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        pcs::ipa::IpaSuccinctVerifyingKey,
        util::arithmetic::{Domain, root_of_unity},
    };

    let k = usize::try_from(params.k())
        .map_err(|_| "Kagemusha V4 Eq parameter degree does not fit usize".to_owned())?;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    Ok(IpaSuccinctVerifyingKey::new(
        Domain::new(k, root_of_unity(k)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    ))
}

fn kagemusha_ep_succinct_vk_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
) -> Result<
    snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    String,
> {
    use halo2_proofs::{
        halo2curves::{CurveExt as _, group::Curve as _, pasta::Ep},
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        pcs::ipa::IpaSuccinctVerifyingKey,
        util::arithmetic::{Domain, root_of_unity},
    };

    let k = usize::try_from(params.k())
        .map_err(|_| "Kagemusha V4 Ep parameter degree does not fit usize".to_owned())?;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    Ok(IpaSuccinctVerifyingKey::new(
        Domain::new(k, root_of_unity(k)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    ))
}

/// Trust mode used while authenticating the bounded roles and the two pinned
/// proving-key spools. Candidate evidence remains structurally distinct from
/// a promoted release throughout loading.
enum KagemushaArtifactSpoolBindingV5<'a> {
    AuthenticatedRelease(&'a KagemushaAuthenticatedReleaseV4),
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    CandidateEvidenceLab {
        candidate: &'a iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
        manifest_sha256: [u8; 32],
    },
}

#[cfg(feature = "kagemusha-candidate-evidence-lab")]
fn validate_kagemusha_candidate_spool_identity_v5(
    candidate_sha256: [u8; 32],
    manifest_sha256: [u8; 32],
    expected_candidate_sha256: [u8; 32],
    expected_manifest_sha256: [u8; 32],
) -> Result<(), String> {
    if candidate_sha256 == [0; 32]
        || manifest_sha256 == [0; 32]
        || candidate_sha256 != expected_candidate_sha256
        || manifest_sha256 != expected_manifest_sha256
    {
        return Err("Kagemusha V5 candidate identity mismatch".to_owned());
    }
    Ok(())
}

impl<'a> KagemushaArtifactSpoolBindingV5<'a> {
    fn authenticated_release(release: &'a KagemushaAuthenticatedReleaseV4) -> Result<Self, String> {
        let manifest_sha256 = release
            .manifest()
            .canonical_sha256()
            .map_err(|error| error.to_string())?;
        if manifest_sha256 == [0; 32]
            || manifest_sha256 != release.manifest_sha256()
            || release.release_attestation_sha256() == [0; 32]
            || release.release_policy_sha256() == [0; 32]
        {
            return Err("Kagemusha V5 authenticated release identity mismatch".to_owned());
        }
        Ok(Self::AuthenticatedRelease(release))
    }

    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    fn candidate_evidence_lab(
        candidate: &'a iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
    ) -> Result<Self, String> {
        candidate.validate().map_err(|error| error.to_string())?;
        let candidate_sha256 = candidate.sha256().map_err(|error| error.to_string())?;
        let manifest_bytes = norito::encode_canonical(&candidate.manifest).map_err(|error| {
            format!("failed to encode Kagemusha V5 candidate manifest: {error}")
        })?;
        let manifest_sha256: [u8; 32] = Sha256::digest(manifest_bytes).into();
        validate_kagemusha_candidate_spool_identity_v5(
            candidate_sha256,
            manifest_sha256,
            expected_candidate_sha256,
            expected_manifest_sha256,
        )?;
        Ok(Self::CandidateEvidenceLab {
            candidate,
            manifest_sha256,
        })
    }

    fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV4 {
        match self {
            Self::AuthenticatedRelease(release) => release.manifest(),
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidenceLab { candidate, .. } => &candidate.manifest,
        }
    }

    fn manifest_sha256(&self) -> [u8; 32] {
        match self {
            Self::AuthenticatedRelease(release) => release.manifest_sha256(),
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidenceLab {
                manifest_sha256, ..
            } => *manifest_sha256,
        }
    }

    fn descriptor(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
    ) -> Result<&iroha_data_model::offline::KagemushaPastaCycleArtifactV4, String> {
        self.manifest()
            .profiles
            .iter()
            .find(|profile| profile.parity == parity)
            .and_then(|profile| {
                profile
                    .artifacts
                    .iter()
                    .find(|descriptor| descriptor.kind == kind)
            })
            .ok_or_else(|| "Kagemusha V5 artifact manifest role is absent".to_owned())
    }

    fn validate_header(
        &self,
        header: &iroha_data_model::offline::KagemushaPastaCycleFramedArtifactHeaderV4,
        descriptor: &iroha_data_model::offline::KagemushaPastaCycleArtifactV4,
    ) -> Result<(), String> {
        match self {
            Self::AuthenticatedRelease(_) => {
                header.validate_against_manifest(self.manifest(), descriptor)
            }
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidenceLab { .. } => {
                header.validate_against_candidate_manifest(self.manifest(), descriptor)
            }
        }
        .map_err(|error| error.to_string())
    }

    fn validate_payload(
        &self,
        payload: &super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
    ) -> Result<(), String> {
        let descriptor = self.descriptor(parity, kind)?;
        if payload.header().parity != parity || payload.header().kind != kind {
            return Err("Kagemusha V5 artifact loader returned the wrong role".to_owned());
        }
        self.validate_header(payload.header(), descriptor)?;
        if u64::try_from(payload.payload().len()) != Ok(payload.header().payload_size_bytes)
            || <[u8; 32]>::from(Sha256::digest(payload.payload()))
                != payload.header().payload_sha256
        {
            return Err("Kagemusha V5 authenticated artifact payload mismatch".to_owned());
        }
        Ok(())
    }
}

/// Authenticated, reopenable framed proving-key payload backed by a pinned
/// spool file. Only offsets and digests are retained in memory.
pub(crate) struct KagemushaProvingKeySpoolV5 {
    file: std::fs::File,
    framed_size: u64,
    framed_sha256: [u8; 32],
    payload_offset: u64,
    payload_size: u64,
    payload_sha256: [u8; 32],
}

impl KagemushaProvingKeySpoolV5 {
    fn authenticate(
        mut file: std::fs::File,
        binding: &KagemushaArtifactSpoolBindingV5<'_>,
        parity: KagemushaPastaCycleParityV1,
    ) -> Result<Self, String> {
        use std::io::{Read as _, Seek as _};

        let descriptor =
            binding.descriptor(parity, KagemushaPastaCycleArtifactKindV4::ProvingKey)?;
        file.seek(std::io::SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind Kagemusha V5 PK spool: {error}"))?;

        let mut magic = [0_u8;
            super::kagemusha_artifact_v4::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4
                .len()];
        file.read_exact(&mut magic)
            .map_err(|error| format!("failed to read Kagemusha V5 PK spool magic: {error}"))?;
        if &magic
            != super::kagemusha_artifact_v4::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4
        {
            return Err("Kagemusha V5 PK spool magic mismatch".to_owned());
        }
        let mut header_len_bytes = [0_u8; 4];
        file.read_exact(&mut header_len_bytes)
            .map_err(|error| format!("failed to read Kagemusha V5 PK header length: {error}"))?;
        let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
            .map_err(|_| "Kagemusha V5 PK header length does not fit usize".to_owned())?;
        if header_len == 0
            || header_len
                > super::kagemusha_artifact_v4::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
        {
            return Err("Kagemusha V5 PK spool header length is invalid".to_owned());
        }
        let mut header_bytes = vec![0_u8; header_len];
        file.read_exact(&mut header_bytes)
            .map_err(|error| format!("failed to read Kagemusha V5 PK spool header: {error}"))?;
        let header_decode_limits = norito::core::DecodeLimits::new(
            1024,
            super::kagemusha_artifact_v4::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4,
            4096,
            super::kagemusha_artifact_v4::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
                .saturating_mul(4),
            16,
        );
        let header: iroha_data_model::offline::KagemushaPastaCycleFramedArtifactHeaderV4 =
            norito::decode_canonical_with_limits(&header_bytes, header_decode_limits).map_err(
                |error| {
                    if matches!(&error, norito::Error::NonCanonicalEncoding) {
                        "Kagemusha V5 PK spool header is non-canonical or has the wrong role"
                            .to_owned()
                    } else {
                        "Kagemusha V5 PK spool header is malformed".to_owned()
                    }
                },
            )?;
        if header.parity != parity || header.kind != KagemushaPastaCycleArtifactKindV4::ProvingKey {
            return Err(
                "Kagemusha V5 PK spool header is non-canonical or has the wrong role".to_owned(),
            );
        }
        binding
            .validate_header(&header, descriptor)
            .map_err(|error| format!("Kagemusha V5 PK spool header is unauthenticated: {error}"))?;
        let payload_offset = u64::try_from(magic.len() + header_len_bytes.len() + header_len)
            .map_err(|_| "Kagemusha V5 PK payload offset does not fit u64".to_owned())?;
        if payload_offset.checked_add(header.payload_size_bytes) != Some(descriptor.size_bytes)
            || header.payload_size_bytes == 0
            || header.payload_size_bytes > KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5
        {
            return Err(
                "Kagemusha V5 PK spool length violates its authenticated role cap".to_owned(),
            );
        }

        let source = Self {
            file,
            framed_size: descriptor.size_bytes,
            framed_sha256: descriptor.sha256,
            payload_offset,
            payload_size: descriptor.payload_size_bytes,
            payload_sha256: descriptor.payload_sha256,
        };
        source.reauthenticate()?;
        Ok(source)
    }

    fn reauthenticate(&self) -> Result<(), String> {
        use std::io::{Read as _, Seek as _};

        let mut file = self
            .file
            .try_clone()
            .map_err(|error| format!("failed to duplicate Kagemusha V5 PK spool: {error}"))?;
        file.seek(std::io::SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind Kagemusha V5 PK spool: {error}"))?;
        let mut framed = Sha256::new();
        let mut payload = Sha256::new();
        let mut offset = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        while offset < self.framed_size {
            let remaining = self.framed_size - offset;
            let requested = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("bounded PK hash chunk fits usize");
            file.read_exact(&mut buffer[..requested]).map_err(|error| {
                format!("failed to stream-authenticate Kagemusha V5 PK spool: {error}")
            })?;
            framed.update(&buffer[..requested]);
            let chunk_start = offset;
            let chunk_end = offset + requested as u64;
            let payload_start = chunk_start.max(self.payload_offset);
            let payload_end = chunk_end.min(self.payload_offset + self.payload_size);
            if payload_start < payload_end {
                let start = usize::try_from(payload_start - chunk_start)
                    .expect("bounded PK payload chunk start fits usize");
                let end = usize::try_from(payload_end - chunk_start)
                    .expect("bounded PK payload chunk end fits usize");
                payload.update(&buffer[start..end]);
            }
            offset = chunk_end;
        }
        let mut trailing = [0_u8; 1];
        if file
            .read(&mut trailing)
            .map_err(|error| format!("failed to check Kagemusha V5 PK spool tail: {error}"))?
            != 0
            || <[u8; 32]>::from(framed.finalize()) != self.framed_sha256
            || <[u8; 32]>::from(payload.finalize()) != self.payload_sha256
        {
            return Err("Kagemusha V5 PK spool changed after authentication".to_owned());
        }
        Ok(())
    }

    fn open_payload(&self) -> Result<KagemushaProvingKeyPayloadReaderV5, String> {
        use std::io::Seek as _;

        self.reauthenticate()?;
        let mut file = self
            .file
            .try_clone()
            .map_err(|error| format!("failed to duplicate Kagemusha V5 PK spool: {error}"))?;
        file.seek(std::io::SeekFrom::Start(self.payload_offset))
            .map_err(|error| format!("failed to seek Kagemusha V5 PK payload: {error}"))?;
        Ok(KagemushaProvingKeyPayloadReaderV5 {
            file,
            start: self.payload_offset,
            length: self.payload_size,
            position: 0,
            expected_sha256: self.payload_sha256,
        })
    }
}

struct KagemushaProvingKeyPayloadReaderV5 {
    file: std::fs::File,
    start: u64,
    length: u64,
    position: u64,
    expected_sha256: [u8; 32],
}

impl KagemushaProvingKeyPayloadReaderV5 {
    fn finish(mut self) -> Result<(), String> {
        use std::io::{Read as _, Seek as _};

        if self.position != self.length {
            return Err(
                "Kagemusha V5 proving-key parser did not consume the exact payload".to_owned(),
            );
        }
        self.seek(std::io::SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind parsed Kagemusha V5 PK: {error}"))?;
        let mut hasher = Sha256::new();
        let mut remaining = self.length;
        let mut buffer = [0_u8; 64 * 1024];
        while remaining != 0 {
            let requested = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("bounded PK hash chunk fits usize");
            self.read_exact(&mut buffer[..requested])
                .map_err(|error| format!("failed to rehash parsed Kagemusha V5 PK: {error}"))?;
            hasher.update(&buffer[..requested]);
            remaining -= requested as u64;
        }
        if <[u8; 32]>::from(hasher.finalize()) != self.expected_sha256 {
            return Err("Kagemusha V5 proving-key payload changed while being parsed".to_owned());
        }
        Ok(())
    }
}

impl std::io::Read for KagemushaProvingKeyPayloadReaderV5 {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.length.saturating_sub(self.position);
        let allowed = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded PK read length fits usize");
        if allowed == 0 {
            return Ok(0);
        }
        let count = std::io::Read::read(&mut self.file, &mut buffer[..allowed])?;
        self.position = self
            .position
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("Kagemusha V5 PK read position overflow"))?;
        Ok(count)
    }
}

impl std::io::Seek for KagemushaProvingKeyPayloadReaderV5 {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        let relative = match position {
            std::io::SeekFrom::Start(offset) => i128::from(offset),
            std::io::SeekFrom::Current(delta) => i128::from(self.position) + i128::from(delta),
            std::io::SeekFrom::End(delta) => i128::from(self.length) + i128::from(delta),
        };
        if relative < 0 || relative > i128::from(self.length) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Kagemusha V5 PK seek escaped its authenticated payload",
            ));
        }
        let relative = u64::try_from(relative)
            .map_err(|_| std::io::Error::other("Kagemusha V5 PK seek overflow"))?;
        std::io::Seek::seek(
            &mut self.file,
            std::io::SeekFrom::Start(
                self.start
                    .checked_add(relative)
                    .ok_or_else(|| std::io::Error::other("Kagemusha V5 PK seek overflow"))?,
            ),
        )?;
        self.position = relative;
        Ok(relative)
    }
}

fn validate_kagemusha_processed_pk_reader_v5(
    reader: &mut KagemushaProvingKeyPayloadReaderV5,
    shape: KagemushaProcessedKeyShapeV4,
    role: &str,
) -> Result<(), String> {
    use std::io::{Read as _, Seek as _};

    let read_u8 =
        |reader: &mut KagemushaProvingKeyPayloadReaderV5, field: &str| -> Result<u8, String> {
            let mut bytes = [0_u8; 1];
            reader
                .read_exact(&mut bytes)
                .map_err(|error| format!("Kagemusha V5 {role} {field} is truncated: {error}"))?;
            Ok(bytes[0])
        };
    let read_u32_le =
        |reader: &mut KagemushaProvingKeyPayloadReaderV5, field: &str| -> Result<u32, String> {
            let mut bytes = [0_u8; 4];
            reader
                .read_exact(&mut bytes)
                .map_err(|error| format!("Kagemusha V5 {role} {field} is truncated: {error}"))?;
            Ok(u32::from_le_bytes(bytes))
        };
    let read_u32_be =
        |reader: &mut KagemushaProvingKeyPayloadReaderV5, field: &str| -> Result<u32, String> {
            let mut bytes = [0_u8; 4];
            reader
                .read_exact(&mut bytes)
                .map_err(|error| format!("Kagemusha V5 {role} {field} is truncated: {error}"))?;
            Ok(u32::from_be_bytes(bytes))
        };
    let skip = |reader: &mut KagemushaProvingKeyPayloadReaderV5,
                bytes: u64,
                field: &str|
     -> Result<(), String> {
        let offset = i64::try_from(bytes)
            .map_err(|_| format!("Kagemusha V5 {role} {field} length does not fit i64"))?;
        reader
            .seek(std::io::SeekFrom::Current(offset))
            .map_err(|error| format!("Kagemusha V5 {role} {field} is truncated: {error}"))?;
        Ok(())
    };

    reader
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind Kagemusha V5 {role} PK: {error}"))?;
    let version = read_u8(reader, "verifier-key version")?;
    let encoded_k = read_u32_le(reader, "verifier-key degree")?;
    let selectors = read_u8(reader, "selector-compression flag")?;
    let fixed_count = read_u32_le(reader, "fixed-commitment count")?;
    if version != KAGEMUSHA_HALO2_KEY_VERSION_V4
        || encoded_k != shape.k
        || selectors != KAGEMUSHA_HALO2_UNCOMPRESSED_SELECTORS_V4
        || fixed_count != shape.fixed_polynomials_u32(role)?
    {
        return Err(format!(
            "Kagemusha V5 {role} processed verifier-key prefix does not match the authenticated shape"
        ));
    }
    let commitment_count = shape
        .fixed_polynomials
        .checked_add(shape.permutation_polynomials)
        .ok_or_else(|| format!("Kagemusha V5 {role} commitment count overflows"))?;
    skip(
        reader,
        u64::try_from(commitment_count)
            .ok()
            .and_then(|count| count.checked_mul(shape.point_bytes as u64))
            .ok_or_else(|| format!("Kagemusha V5 {role} commitment length overflows"))?,
        "verifier-key commitments",
    )?;

    let validate_polynomial = |reader: &mut KagemushaProvingKeyPayloadReaderV5,
                               field: &str|
     -> Result<(), String> {
        let encoded = read_u32_be(reader, field)?;
        if encoded != shape.domain_rows {
            return Err(format!(
                "Kagemusha V5 {role} {field} length {encoded} does not match authenticated domain size {}",
                shape.domain_rows
            ));
        }
        skip(
            reader,
            u64::from(shape.domain_rows)
                .checked_mul(shape.scalar_bytes as u64)
                .ok_or_else(|| format!("Kagemusha V5 {role} {field} byte length overflows"))?,
            field,
        )
    };
    validate_polynomial(reader, "l0 polynomial")?;
    validate_polynomial(reader, "l_last polynomial")?;
    validate_polynomial(reader, "l_active_row polynomial")?;
    for (expected, field) in [
        (shape.fixed_polynomials, "fixed-value polynomials"),
        (shape.fixed_polynomials, "fixed coefficient polynomials"),
        (
            shape.permutation_polynomials,
            "permutation Lagrange polynomials",
        ),
        (
            shape.permutation_polynomials,
            "permutation coefficient polynomials",
        ),
    ] {
        let encoded = read_u32_be(reader, field)?;
        if usize::try_from(encoded) != Ok(expected) {
            return Err(format!(
                "Kagemusha V5 {role} {field} count does not match authenticated shape {expected}"
            ));
        }
        for _ in 0..expected {
            validate_polynomial(reader, field)?;
        }
    }
    let expected_length = shape.proving_key_bytes(role)?;
    if reader.position != reader.length || reader.length != expected_length {
        return Err(format!(
            "Kagemusha V5 {role} processed proving-key length does not match authenticated shape {expected_length}"
        ));
    }
    Ok(())
}

fn parse_kagemusha_eq_pk_spool_v5(
    source: &KagemushaProvingKeySpoolV5,
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    use std::io::Seek as _;

    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let shape = kagemusha_processed_key_shape_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &circuit_params,
        "Eq",
    )?;
    let mut reader = source.open_payload()?;
    validate_kagemusha_processed_pk_reader_v5(&mut reader, shape, "Eq")?;
    reader
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind Kagemusha V5 Eq PK: {error}"))?;
    #[cfg(feature = "circuit-params")]
    let key = ProvingKey::read::<_, KagemushaStepEqCircuitV4>(
        &mut reader,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to stream-parse Kagemusha V5 Eq PK: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        ProvingKey::read::<_, KagemushaStepEqCircuitV4>(&mut reader, SerdeFormat::Processed)
            .map_err(|error| format!("failed to stream-parse Kagemusha V5 Eq PK: {error}"))?
    };
    reader.finish()?;
    Ok(key)
}

fn parse_kagemusha_ep_pk_spool_v5(
    source: &KagemushaProvingKeySpoolV5,
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    use std::io::Seek as _;

    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let shape = kagemusha_processed_key_shape_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &circuit_params,
        "Ep",
    )?;
    let mut reader = source.open_payload()?;
    validate_kagemusha_processed_pk_reader_v5(&mut reader, shape, "Ep")?;
    reader
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind Kagemusha V5 Ep PK: {error}"))?;
    #[cfg(feature = "circuit-params")]
    let key = ProvingKey::read::<_, KagemushaStepEpCircuitV4>(
        &mut reader,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to stream-parse Kagemusha V5 Ep PK: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        ProvingKey::read::<_, KagemushaStepEpCircuitV4>(&mut reader, SerdeFormat::Processed)
            .map_err(|error| format!("failed to stream-parse Kagemusha V5 Ep PK: {error}"))?
    };
    reader.finish()?;
    Ok(key)
}

pub(crate) struct KagemushaPastaCycleProverV4 {
    manifest_sha256: [u8; 32],
    context: std::sync::Arc<KagemushaPastaCycleRuntimeContextV5>,
    step_eq_verifying_key_bytes: Vec<u8>,
    step_eq_proving_key_spool: KagemushaProvingKeySpoolV5,
    step_eq_bootstrap: KagemushaStepBootstrapV4,
    step_eq_compiled_protocol_sha256: [u8; 32],
    step_eq_compiled_parent_protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_verifying_key_bytes: Vec<u8>,
    step_ep_proving_key_spool: KagemushaProvingKeySpoolV5,
    step_ep_bootstrap: KagemushaStepBootstrapV4,
    step_ep_compiled_protocol_sha256: [u8; 32],
    step_ep_compiled_parent_protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
}

impl std::ops::Deref for KagemushaPastaCycleProverV4 {
    type Target = KagemushaPastaCycleRuntimeContextV5;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl KagemushaPastaCycleProverV4 {
    /// The legacy all-eight in-memory carrier cannot satisfy the compact V5
    /// active-memory contract. Callers must provide pinned proving-key spools.
    pub(crate) fn from_authenticated_artifacts(
        _artifacts: &super::kagemusha_artifact_v4::KagemushaPastaCycleProverArtifactsV4,
    ) -> Result<Self, String> {
        Err(
            "Kagemusha V5 rejects in-memory dual-PK carriers; use authenticated proving-key spools"
                .to_owned(),
        )
    }

    /// Parse six bounded roles while retaining only authenticated, reopenable
    /// file sources for the two release-sized proving keys.
    pub(crate) fn from_authenticated_artifact_spool_loader<F>(
        release: &KagemushaAuthenticatedReleaseV4,
        step_eq_proving_key_file: std::fs::File,
        step_ep_proving_key_file: std::fs::File,
        load: F,
    ) -> Result<Self, String>
    where
        F: FnMut(
            KagemushaPastaCycleParityV1,
            KagemushaPastaCycleArtifactKindV4,
        ) -> Result<
            super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4,
            String,
        >,
    {
        let binding = KagemushaArtifactSpoolBindingV5::authenticated_release(release)?;
        Self::from_artifact_spool_loader_binding(
            &binding,
            step_eq_proving_key_file,
            step_ep_proving_key_file,
            load,
        )
    }

    /// Parse candidate evidence without conferring production-release trust.
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    pub(crate) fn from_candidate_artifact_spool_loader<F>(
        candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
        step_eq_proving_key_file: std::fs::File,
        step_ep_proving_key_file: std::fs::File,
        load: F,
    ) -> Result<Self, String>
    where
        F: FnMut(
            KagemushaPastaCycleParityV1,
            KagemushaPastaCycleArtifactKindV4,
        ) -> Result<
            super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4,
            String,
        >,
    {
        let binding = KagemushaArtifactSpoolBindingV5::candidate_evidence_lab(
            candidate,
            expected_candidate_sha256,
            expected_manifest_sha256,
        )?;
        Self::from_artifact_spool_loader_binding(
            &binding,
            step_eq_proving_key_file,
            step_ep_proving_key_file,
            load,
        )
    }

    fn from_artifact_spool_loader_binding<F>(
        binding: &KagemushaArtifactSpoolBindingV5<'_>,
        step_eq_proving_key_file: std::fs::File,
        step_ep_proving_key_file: std::fs::File,
        mut load: F,
    ) -> Result<Self, String>
    where
        F: FnMut(
            KagemushaPastaCycleParityV1,
            KagemushaPastaCycleArtifactKindV4,
        ) -> Result<
            super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4,
            String,
        >,
    {
        use halo2_proofs::SerdeFormat;

        let manifest = binding.manifest();
        let manifest_sha256 = binding.manifest_sha256();
        if manifest_sha256 == [0; 32] {
            return Err("Kagemusha V4 artifact manifest digest is zero".to_owned());
        }
        let step_eq_proving_key_spool = KagemushaProvingKeySpoolV5::authenticate(
            step_eq_proving_key_file,
            binding,
            KagemushaPastaCycleParityV1::StepEq,
        )?;
        let step_ep_proving_key_spool = KagemushaProvingKeySpoolV5::authenticate(
            step_ep_proving_key_file,
            binding,
            KagemushaPastaCycleParityV1::StepEp,
        )?;
        let mut bound_load = |parity, kind| {
            let payload = load(parity, kind)?;
            binding.validate_payload(&payload, parity, kind)?;
            Ok(payload)
        };
        let step_eq = manifest
            .profiles
            .first()
            .ok_or_else(|| "Kagemusha V4 Eq release profile is absent".to_owned())?
            .clone();
        let step_ep = manifest
            .profiles
            .get(1)
            .ok_or_else(|| "Kagemusha V4 Ep release profile is absent".to_owned())?
            .clone();
        if step_eq.parity != KagemushaPastaCycleParityV1::StepEq
            || step_ep.parity != KagemushaPastaCycleParityV1::StepEp
        {
            return Err("Kagemusha V4 release profile order mismatch".to_owned());
        }
        let step_eq_params = with_kagemusha_artifact_payload_v4(
            &mut bound_load,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            |bytes| {
                parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
                    bytes,
                    step_eq.ipa_k,
                    "Eq",
                )
            },
        )?;
        let (step_eq_verifying_key_bytes, step_eq_verifying_key) =
            with_kagemusha_artifact_payload_v4(
                &mut bound_load,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
                |bytes| {
                    Ok((
                        bytes.to_vec(),
                        parse_kagemusha_eq_vk_v4(bytes, step_eq.circuit_params.clone())?,
                    ))
                },
            )?;
        let (step_eq_bootstrap, step_eq_compiled_protocol_sha256, step_eq_compiled_parent_protocol) =
            with_kagemusha_artifact_payload_v4(
                &mut bound_load,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
                |bytes| {
                    validate_kagemusha_profile_protocol_v4(
                        &step_eq_params,
                        &step_eq_verifying_key,
                        &step_eq.circuit_params,
                        KagemushaPastaCycleParityV1::StepEq,
                        step_eq.compiled_protocol_structure_sha256,
                        bytes,
                    )
                },
            )?;
        terminal_validate_kagemusha_eq_bootstrap_v4(
            &step_eq_params,
            &step_eq_verifying_key,
            &step_eq.circuit_params,
            &step_eq_bootstrap,
        )?;
        drop(step_eq_verifying_key);
        let step_eq_proving_key = parse_kagemusha_eq_pk_spool_v5(
            &step_eq_proving_key_spool,
            step_eq.circuit_params.clone(),
        )?;
        if step_eq_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            != step_eq_verifying_key_bytes
        {
            return Err("Kagemusha V5 Eq proving key embeds a different verifier key".to_owned());
        }
        drop(step_eq_proving_key);

        let step_ep_params = with_kagemusha_artifact_payload_v4(
            &mut bound_load,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            |bytes| {
                parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
                    bytes,
                    step_ep.ipa_k,
                    "Ep",
                )
            },
        )?;
        let (step_ep_verifying_key_bytes, step_ep_verifying_key) =
            with_kagemusha_artifact_payload_v4(
                &mut bound_load,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
                |bytes| {
                    Ok((
                        bytes.to_vec(),
                        parse_kagemusha_ep_vk_v4(bytes, step_ep.circuit_params.clone())?,
                    ))
                },
            )?;
        let (step_ep_bootstrap, step_ep_compiled_protocol_sha256, step_ep_compiled_parent_protocol) =
            with_kagemusha_artifact_payload_v4(
                &mut bound_load,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
                |bytes| {
                    validate_kagemusha_profile_protocol_v4(
                        &step_ep_params,
                        &step_ep_verifying_key,
                        &step_ep.circuit_params,
                        KagemushaPastaCycleParityV1::StepEp,
                        step_ep.compiled_protocol_structure_sha256,
                        bytes,
                    )
                },
            )?;
        terminal_validate_kagemusha_ep_bootstrap_v4(
            &step_ep_params,
            &step_ep_verifying_key,
            &step_ep.circuit_params,
            &step_ep_bootstrap,
        )?;
        drop(step_ep_verifying_key);
        let step_ep_proving_key = parse_kagemusha_ep_pk_spool_v5(
            &step_ep_proving_key_spool,
            step_ep.circuit_params.clone(),
        )?;
        if step_ep_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            != step_ep_verifying_key_bytes
        {
            return Err("Kagemusha V5 Ep proving key embeds a different verifier key".to_owned());
        }
        drop(step_ep_proving_key);
        let step_eq_succinct_vk = kagemusha_eq_succinct_vk_v4(&step_eq_params)?;
        let step_ep_succinct_vk = kagemusha_ep_succinct_vk_v4(&step_ep_params)?;
        Ok(Self {
            manifest_sha256,
            context: std::sync::Arc::new(KagemushaPastaCycleRuntimeContextV5 {
                step_eq_params,
                step_eq_circuit_params: step_eq.circuit_params.clone(),
                step_ep_params,
                step_ep_circuit_params: step_ep.circuit_params.clone(),
                max_pair_bytes: manifest.max_proof_bytes,
            }),
            step_eq_verifying_key_bytes,
            step_eq_proving_key_spool,
            step_eq_bootstrap,
            step_eq_compiled_protocol_sha256,
            step_eq_compiled_parent_protocol,
            step_eq_succinct_vk,
            step_ep_verifying_key_bytes,
            step_ep_proving_key_spool,
            step_ep_bootstrap,
            step_ep_compiled_protocol_sha256,
            step_ep_compiled_parent_protocol,
            step_ep_succinct_vk,
        })
    }

    pub(crate) fn step_eq_compiled_protocol_sha256(&self) -> [u8; 32] {
        self.step_eq_compiled_protocol_sha256
    }

    pub(crate) fn step_ep_compiled_protocol_sha256(&self) -> [u8; 32] {
        self.step_ep_compiled_protocol_sha256
    }

    pub(crate) fn shared_terminal_verifier_v5(
        &self,
    ) -> Result<KagemushaPastaCycleTerminalVerifierV4, String> {
        let step_eq_verifying_key = parse_kagemusha_eq_vk_v4(
            &self.step_eq_verifying_key_bytes,
            self.step_eq_circuit_params.clone(),
        )?;
        let step_ep_verifying_key = parse_kagemusha_ep_vk_v4(
            &self.step_ep_verifying_key_bytes,
            self.step_ep_circuit_params.clone(),
        )?;
        Ok(KagemushaPastaCycleTerminalVerifierV4 {
            context: std::sync::Arc::clone(&self.context),
            step_eq_verifying_key,
            step_ep_verifying_key,
        })
    }

    fn step_eq_parent_from_pair_v4(
        &self,
        pair: &KagemushaPastaCycleProofPairV4,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EqAffine>, String>
    {
        let instances = vec![pair.public_inputs.instance_column::<Fp>(
            &self.step_eq_circuit_params,
            KagemushaPastaCycleParityV1::StepEq,
        )?];
        let (carried_lineage, external_accumulation_proof) = if pair.public_inputs.parent_count()?
            == 0
        {
            (
                self.step_eq_bootstrap
                    .parent_slot
                    .carried_lineage
                    .to_eq(self.step_eq_circuit_params.k)?,
                self.step_eq_bootstrap.parent_slot.post_proof_fold.clone(),
            )
        } else {
            (
                pair.public_inputs
                    .parent_eq_lineage_accumulator
                    .as_ref()
                    .ok_or_else(|| "Kagemusha V4 Eq parent omitted its carried lineage".to_owned())?
                    .to_eq(self.step_eq_circuit_params.k)?,
                pair.step_eq_accumulation_proof.clone(),
            )
        };
        Ok(KagemushaStepParentProofV4 {
            instances,
            proof_bytes: pair.step_eq_proof_bytes.clone(),
            carried_lineage,
            external_accumulation_proof,
        })
    }

    fn step_ep_parent_from_pair_v4(
        &self,
        pair: &KagemushaPastaCycleProofPairV4,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EpAffine>, String>
    {
        let instances = vec![pair.public_inputs.instance_column::<Fq>(
            &self.step_ep_circuit_params,
            KagemushaPastaCycleParityV1::StepEp,
        )?];
        let (carried_lineage, external_accumulation_proof) = if pair.public_inputs.parent_count()?
            == 0
        {
            (
                self.step_ep_bootstrap
                    .parent_slot
                    .carried_lineage
                    .to_ep(self.step_ep_circuit_params.k)?,
                self.step_ep_bootstrap.parent_slot.post_proof_fold.clone(),
            )
        } else {
            (
                pair.public_inputs
                    .parent_ep_lineage_accumulator
                    .as_ref()
                    .ok_or_else(|| "Kagemusha V4 Ep parent omitted its carried lineage".to_owned())?
                    .to_ep(self.step_ep_circuit_params.k)?,
                pair.step_ep_accumulation_proof.clone(),
            )
        };
        Ok(KagemushaStepParentProofV4 {
            instances,
            proof_bytes: pair.step_ep_proof_bytes.clone(),
            carried_lineage,
            external_accumulation_proof,
        })
    }

    fn prepare_step_recursions_v4(
        &self,
        public_inputs: &mut KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
        parent_state_openings: &[Vec<u32>],
    ) -> Result<
        (
            KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>,
            KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>,
            KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
            KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
        ),
        String,
    > {
        if parent_pair_bytes.len() > KAGEMUSHA_PASTA_PARENT_SLOTS_V1
            || parent_pair_bytes.len() != parent_state_openings.len()
        {
            return Err("Kagemusha V4 operation consumes more than two parents".to_owned());
        }
        let manifest_words = kagemusha_exact_u32_public_limbs(self.manifest_sha256);
        let eq_protocol_words =
            kagemusha_sha256_public_words(self.step_eq_compiled_protocol_sha256);
        let ep_protocol_words =
            kagemusha_sha256_public_words(self.step_ep_compiled_protocol_sha256);
        let manifest_chunks = kagemusha_u32_words_to_u128_chunks_v5(&manifest_words);
        let eq_protocol_chunks = kagemusha_u32_words_to_u128_chunks_v5(&eq_protocol_words);
        let ep_protocol_chunks = kagemusha_u32_words_to_u128_chunks_v5(&ep_protocol_words);
        let step_eq_terminal_verifying_key = parse_kagemusha_eq_vk_v4(
            &self.step_eq_verifying_key_bytes,
            self.step_eq_circuit_params.clone(),
        )?;
        let step_ep_terminal_verifying_key = parse_kagemusha_ep_vk_v4(
            &self.step_ep_verifying_key_bytes,
            self.step_ep_circuit_params.clone(),
        )?;

        let mut parents = Vec::with_capacity(parent_pair_bytes.len());
        for bytes in parent_pair_bytes {
            let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
                bytes,
                &self.step_eq_circuit_params,
                &self.step_ep_circuit_params,
                self.max_pair_bytes,
            )?;
            if pair.public_inputs.common_header[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
                != manifest_chunks
                || pair.public_inputs.common_header
                    [KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
                        ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
                    != eq_protocol_chunks
                || pair.public_inputs.common_header
                    [KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
                        ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
                    != ep_protocol_chunks
            {
                return Err(
                    "Kagemusha V4 parent pair belongs to a different authenticated release"
                        .to_owned(),
                );
            }
            let (eq_lineage, ep_lineage) = terminal_verify_proof_pair_lineage_v4(
                &self.step_eq_params,
                &step_eq_terminal_verifying_key,
                &self.step_ep_params,
                &step_ep_terminal_verifying_key,
                &pair,
                &self.step_eq_circuit_params,
                &self.step_ep_circuit_params,
                self.max_pair_bytes,
            )?;
            parents.push((pair, eq_lineage, ep_lineage));
        }
        drop(step_eq_terminal_verifying_key);
        drop(step_ep_terminal_verifying_key);

        public_inputs.parent_count = u32::try_from(parents.len())
            .map_err(|_| "Kagemusha V4 parent count does not fit u32".to_owned())?;
        public_inputs.manifest_sha256 = manifest_words;
        public_inputs.step_eq_compiled_protocol_sha256 = eq_protocol_words;
        public_inputs.step_ep_compiled_protocol_sha256 = ep_protocol_words;
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            public_inputs.parent_states[slot] = parent_state_openings.get(slot).map_or_else(
                || {
                    vec![
                        0;
                        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2
                    ]
                },
                Clone::clone,
            );
            // The native audit-derivation prepass accepts only blank derived-join
            // slots. It derives both real digests below before either
            // proof circuit is built, so no placeholder can enter a proof.
            public_inputs.parent_eq_deferred_sha256[slot] = [0; 8];
            public_inputs.parent_ep_deferred_sha256[slot] = [0; 8];
        }

        let (parent_eq_lineage_accumulator, eq_branch_merge_fold) = match parents.as_slice() {
            [] => (None, self.step_eq_bootstrap.branch_merge_fold.clone()),
            [(_, lineage, _)] => (
                Some(lineage.clone()),
                self.step_eq_bootstrap.branch_merge_fold.clone(),
            ),
            [(_, first, _), (_, second, _)] => {
                let (fold, accumulated) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
                    &self.step_eq_params,
                    self.step_eq_circuit_params.k,
                    first.to_eq(self.step_eq_circuit_params.k)?,
                    Some(second.to_eq(self.step_eq_circuit_params.k)?),
                )?;
                (
                    Some(KagemushaIpaAccumulatorWireV4::from_eq(
                        &accumulated,
                        self.step_eq_circuit_params.k,
                    )?),
                    fold,
                )
            }
            _ => unreachable!("parent count was bounded above"),
        };
        let (parent_ep_lineage_accumulator, ep_branch_merge_fold) = match parents.as_slice() {
            [] => (None, self.step_ep_bootstrap.branch_merge_fold.clone()),
            [(_, _, lineage)] => (
                Some(lineage.clone()),
                self.step_ep_bootstrap.branch_merge_fold.clone(),
            ),
            [(_, _, first), (_, _, second)] => {
                let (fold, accumulated) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
                    &self.step_ep_params,
                    self.step_ep_circuit_params.k,
                    first.to_ep(self.step_ep_circuit_params.k)?,
                    Some(second.to_ep(self.step_ep_circuit_params.k)?),
                )?;
                (
                    Some(KagemushaIpaAccumulatorWireV4::from_ep(
                        &accumulated,
                        self.step_ep_circuit_params.k,
                    )?),
                    fold,
                )
            }
            _ => unreachable!("parent count was bounded above"),
        };
        public_inputs.parent_eq_lineage_accumulator = parent_eq_lineage_accumulator;
        public_inputs.parent_ep_lineage_accumulator = parent_ep_lineage_accumulator;

        let mut eq_parent_witnesses = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
        let mut ep_parent_witnesses = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            if let Some((pair, _, _)) = parents.get(slot) {
                eq_parent_witnesses.push(self.step_eq_parent_from_pair_v4(pair)?);
                ep_parent_witnesses.push(self.step_ep_parent_from_pair_v4(pair)?);
            } else {
                eq_parent_witnesses.push(self.step_eq_bootstrap.step_eq_parent(
                    &self.step_eq_circuit_params,
                    self.step_eq_bootstrap.compiled_protocol_structure_sha256,
                    slot,
                )?);
                ep_parent_witnesses.push(self.step_ep_bootstrap.step_ep_parent(
                    &self.step_ep_circuit_params,
                    self.step_ep_bootstrap.compiled_protocol_structure_sha256,
                    slot,
                )?);
            }
        }
        let step_eq_recursion = KagemushaStepParityRecursionV4 {
            succinct_vk: self.step_eq_succinct_vk.clone(),
            compiled_parent_protocol: self.step_eq_compiled_parent_protocol.clone(),
            fixed_structure_sha256: self.step_eq_bootstrap.compiled_protocol_structure_sha256,
            parents: eq_parent_witnesses.try_into().map_err(|parents: Vec<_>| {
                format!(
                    "Kagemusha V4 Eq recursion has {} parents instead of two",
                    parents.len()
                )
            })?,
            branch_merge_fold: eq_branch_merge_fold,
        };
        let step_ep_recursion = KagemushaStepParityRecursionV4 {
            succinct_vk: self.step_ep_succinct_vk.clone(),
            compiled_parent_protocol: self.step_ep_compiled_parent_protocol.clone(),
            fixed_structure_sha256: self.step_ep_bootstrap.compiled_protocol_structure_sha256,
            parents: ep_parent_witnesses.try_into().map_err(|parents: Vec<_>| {
                format!(
                    "Kagemusha V4 Ep recursion has {} parents instead of two",
                    parents.len()
                )
            })?,
            branch_merge_fold: ep_branch_merge_fold,
        };

        let eq_audits =
            collect_kagemusha_scalar_audits_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
                public_inputs,
                proof_step_count,
                &self.step_eq_circuit_params,
                &step_eq_recursion,
                KagemushaPastaCycleParityV1::StepEq,
            )?;
        let ep_audits =
            collect_kagemusha_scalar_audits_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
                public_inputs,
                proof_step_count,
                &self.step_ep_circuit_params,
                &step_ep_recursion,
                KagemushaPastaCycleParityV1::StepEp,
            )?;
        if public_inputs.parent_count == 0 {
            // Initialization has no public deferred join. The fixed Step
            // circuits still execute both verifier halves and constrain both
            // zero slots; only the native derivation passes are unnecessary.
            public_inputs.parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
            public_inputs.parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        } else {
            // Keep the public inputs blank until both independent derivations
            // have finished: the derivation boundary deliberately rejects a
            // caller-preselected join. Each large witness-only builder drops
            // inside its scope before the opposite parity starts.
            let eq_public_words = kagemusha_deferred_audit_public_words_v5(
                &eq_audits.audit,
                &eq_audits.stages,
                public_inputs.parent_count,
                eq_audits.inner_parent_counts,
            )?;
            let ep_public_words = kagemusha_deferred_audit_public_words_v5(
                &ep_audits.audit,
                &ep_audits.stages,
                public_inputs.parent_count,
                ep_audits.inner_parent_counts,
            )?;
            public_inputs.parent_eq_deferred_sha256 = eq_public_words;
            public_inputs.parent_ep_deferred_sha256 = ep_public_words;
        }
        let eq_layout = public_inputs.validate(proof_step_count, &self.step_eq_circuit_params)?;
        let ep_layout = public_inputs.validate(proof_step_count, &self.step_ep_circuit_params)?;
        if eq_layout != ep_layout {
            return Err("Kagemusha V4 prepared Eq/Ep public layouts differ".to_owned());
        }
        Ok((step_eq_recursion, step_ep_recursion, eq_audits, ep_audits))
    }

    /// Prepare canonical real-or-bootstrap parent slots, derive both deferred
    /// audit joins, build both concrete circuits, and return a terminally
    /// verified backend-native V4 proof pair.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_operation_v4(
        &self,
        mut public_inputs: KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
        parent_state_openings: &[Vec<u32>],
        secure: &super::confidential_v2::KagemushaStepSecureWitnessV3,
        output_membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    ) -> Result<KagemushaPastaCycleProofPairV4, String> {
        let (step_eq_recursion, step_ep_recursion, eq_output, ep_output) = self
            .prepare_step_recursions_v4(
                &mut public_inputs,
                proof_step_count,
                parent_pair_bytes,
                parent_state_openings,
            )?;
        let result_frontier = public_inputs
            .result_state
            .get(super::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX)
            .copied()
            .ok_or_else(|| "Kagemusha V4 result state omits its frontier".to_owned())?;
        if result_frontier != output_membership.dummy_leaf_index {
            return Err("Kagemusha V4 result state/frontier witness mismatch".to_owned());
        }
        let expected_parent_frontier = match output_membership.operation {
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::Init => None,
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::Split => output_membership
                .recipient
                .as_ref()
                .map(|leaf| leaf.leaf_index),
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::RedemptionChange => {
                output_membership
                    .change
                    .as_ref()
                    .map(|leaf| leaf.leaf_index)
            }
        };
        match expected_parent_frontier {
            None if public_inputs.parent_count == 0 => {}
            Some(expected) if public_inputs.parent_count > 0 => {
                for parent in public_inputs
                    .parent_states
                    .iter()
                    .take(public_inputs.parent_count as usize)
                {
                    if parent
                        .get(super::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX)
                        .copied()
                        != Some(expected)
                    {
                        return Err(
                            "Kagemusha V4 output insertion does not start at the parent frontier"
                                .to_owned(),
                        );
                    }
                }
            }
            _ => return Err("Kagemusha V4 membership/parent profile mismatch".to_owned()),
        }
        let witness = KagemushaStepWitnessV4 {
            public_inputs: &public_inputs,
            proof_step_count,
            secure,
            output_membership,
            step_eq_recursion: &step_eq_recursion,
            step_ep_recursion: &step_ep_recursion,
            step_eq_bootstrap: Some(&self.step_eq_bootstrap),
            step_ep_bootstrap: Some(&self.step_ep_bootstrap),
        };
        self.prove_step_v4(
            &witness,
            &public_inputs,
            proof_step_count,
            &eq_output,
            &ep_output,
        )
    }

    /// Prove and terminally decide one operation, then expose only canonical
    /// opaque ABI-21 bytes to the public lifecycle facade.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_operation_encoded_v4(
        &self,
        public_inputs: KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
        parent_state_openings: &[Vec<u32>],
        secure: &super::confidential_v2::KagemushaStepSecureWitnessV3,
        output_membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    ) -> Result<Vec<u8>, String> {
        let pair = self.prove_operation_v4(
            public_inputs,
            proof_step_count,
            parent_pair_bytes,
            parent_state_openings,
            secure,
            output_membership,
        )?;
        pair.encode_authenticated(
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )
    }

    /// Prove both concrete V4 halves, fold each current opening with its parent
    /// lineage, and terminally decide the resulting pair before returning it.
    fn prove_step_v4(
        &self,
        witness: &KagemushaStepWitnessV4<'_>,
        public_inputs: &KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        eq_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
        ep_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    ) -> Result<KagemushaPastaCycleProofPairV4, String> {
        let eq_layout = public_inputs.validate(proof_step_count, &self.step_eq_circuit_params)?;
        let ep_layout = public_inputs.validate(proof_step_count, &self.step_ep_circuit_params)?;
        if eq_layout != ep_layout || self.step_eq_circuit_params.k != self.step_ep_circuit_params.k
        {
            return Err("Kagemusha V4 prover Eq/Ep profile mismatch".to_owned());
        }

        let step_eq = build_kagemusha_step_eq_circuit_v5(
            witness,
            self.step_eq_circuit_params.clone(),
            &self.step_ep_circuit_params,
            ep_output,
            KagemushaStepPublicModeV4::Live,
            KagemushaCircuitBuilderStageV5::Prover(&self.step_eq_bootstrap.circuit_break_points),
        )?;
        let step_eq_proving_key = parse_kagemusha_eq_pk_spool_v5(
            &self.step_eq_proving_key_spool,
            self.step_eq_circuit_params.clone(),
        )?;
        let (step_eq_proof_bytes, step_eq_verifying_key) = prove_step_eq_v4(
            &self.step_eq_params,
            step_eq_proving_key,
            step_eq,
            &public_inputs,
            proof_step_count,
            &self.step_eq_circuit_params,
        )?;

        let eq_instances = vec![public_inputs.instance_column::<Fp>(
            proof_step_count,
            &self.step_eq_circuit_params,
            KagemushaPastaCycleParityV1::StepEq,
        )?];
        let eq_current = succinct_verify_step_eq_instances(
            &self.step_eq_params,
            &step_eq_verifying_key,
            &step_eq_proof_bytes,
            &eq_instances,
            usize::try_from(self.step_eq_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?,
        )?;
        drop(step_eq_verifying_key);
        let eq_parent = public_inputs
            .parent_eq_lineage_accumulator
            .as_ref()
            .map(|wire| wire.to_eq(self.step_eq_circuit_params.k))
            .transpose()?;
        let (step_eq_accumulation_proof, _) =
            super::kagemusha_accumulation::fold_eq_accumulators_v4(
                &self.step_eq_params,
                self.step_eq_circuit_params.k,
                eq_current,
                eq_parent,
            )?;

        // The Eq circuit and key are fully dropped before the Ep circuit is
        // populated and its spool is opened.
        let step_ep = build_kagemusha_step_ep_circuit_v5(
            witness,
            &self.step_eq_circuit_params,
            self.step_ep_circuit_params.clone(),
            eq_output,
            KagemushaStepPublicModeV4::Live,
            KagemushaCircuitBuilderStageV5::Prover(&self.step_ep_bootstrap.circuit_break_points),
        )?;
        let step_ep_proving_key = parse_kagemusha_ep_pk_spool_v5(
            &self.step_ep_proving_key_spool,
            self.step_ep_circuit_params.clone(),
        )?;
        let (step_ep_proof_bytes, step_ep_verifying_key) = prove_step_ep_v4(
            &self.step_ep_params,
            step_ep_proving_key,
            step_ep,
            &public_inputs,
            proof_step_count,
            &self.step_ep_circuit_params,
        )?;
        let ep_instances = vec![public_inputs.instance_column::<Fq>(
            proof_step_count,
            &self.step_ep_circuit_params,
            KagemushaPastaCycleParityV1::StepEp,
        )?];
        let ep_current = succinct_verify_step_ep_instances(
            &self.step_ep_params,
            &step_ep_verifying_key,
            &step_ep_proof_bytes,
            &ep_instances,
            usize::try_from(self.step_ep_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?,
        )?;
        drop(step_ep_verifying_key);
        let ep_parent = public_inputs
            .parent_ep_lineage_accumulator
            .as_ref()
            .map(|wire| wire.to_ep(self.step_ep_circuit_params.k))
            .transpose()?;
        let (step_ep_accumulation_proof, _) =
            super::kagemusha_accumulation::fold_ep_accumulators_v4(
                &self.step_ep_params,
                self.step_ep_circuit_params.k,
                ep_current,
                ep_parent,
            )?;

        let compact_public_inputs =
            KagemushaCompactPublicInputsV5::from_private(&public_inputs, proof_step_count);
        let pair = KagemushaPastaCycleProofPairV4 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
            proof_step_count,
            public_inputs: compact_public_inputs,
            step_eq_proof_bytes,
            step_ep_proof_bytes,
            step_eq_accumulation_proof,
            step_ep_accumulation_proof,
        };
        pair.validate(
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        let step_eq_terminal_verifying_key = parse_kagemusha_eq_vk_v4(
            &self.step_eq_verifying_key_bytes,
            self.step_eq_circuit_params.clone(),
        )?;
        let step_ep_terminal_verifying_key = parse_kagemusha_ep_vk_v4(
            &self.step_ep_verifying_key_bytes,
            self.step_ep_circuit_params.clone(),
        )?;
        terminal_verify_proof_pair_v4(
            &self.step_eq_params,
            &step_eq_terminal_verifying_key,
            &self.step_ep_params,
            &step_ep_terminal_verifying_key,
            &pair,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        Ok(pair)
    }
}

// Source-backed production operations serialize their complete heavy phase.
// The pinned source already serializes individual file callbacks, but parsed
// ParamsIPA/proving-key objects outlive those callbacks.  Keeping this permit
// for the full operation prevents concurrent calls from recreating the same
// two-parity memory peak that the source-backed path is designed to remove.
static KAGEMUSHA_SOURCE_RUNTIME_HEAVY_PERMIT_V4: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_kagemusha_source_runtime_heavy_v4() -> std::sync::MutexGuard<'static, ()> {
    match KAGEMUSHA_SOURCE_RUNTIME_HEAVY_PERMIT_V4.lock() {
        Ok(permit) => permit,
        // The permit serializes memory-heavy work but protects no mutable
        // protocol state. A bridge boundary deliberately catches worker
        // panics, so retaining poison here would let one rejected operation
        // permanently disable every later offline-cash operation.
        Err(poisoned) => {
            KAGEMUSHA_SOURCE_RUNTIME_HEAVY_PERMIT_V4.clear_poison();
            poisoned.into_inner()
        }
    }
}

#[derive(Default)]
struct KagemushaSourceRuntimeHeavyResidencyV4 {
    active: std::cell::Cell<Option<KagemushaPastaCycleParityV1>>,
    peak: std::cell::Cell<u8>,
    #[cfg(test)]
    events: std::cell::RefCell<Vec<(KagemushaPastaCycleParityV1, bool)>>,
}

impl KagemushaSourceRuntimeHeavyResidencyV4 {
    fn enter(
        &self,
        parity: KagemushaPastaCycleParityV1,
    ) -> Result<KagemushaSourceRuntimeHeavyResidencyGuardV4<'_>, String> {
        if self.active.get().is_some() {
            return Err(
                "Kagemusha V4 source runtime attempted concurrent Eq/Ep heavy residency".to_owned(),
            );
        }
        self.active.set(Some(parity));
        self.peak.set(self.peak.get().max(1));
        #[cfg(test)]
        self.events.borrow_mut().push((parity, true));
        Ok(KagemushaSourceRuntimeHeavyResidencyGuardV4 {
            owner: self,
            parity,
        })
    }

    fn assert_released(&self) -> Result<(), String> {
        if self.active.get().is_some() || self.peak.get() > 1 {
            return Err("Kagemusha V4 source runtime residency invariant failed".to_owned());
        }
        Ok(())
    }
}

struct KagemushaSourceRuntimeHeavyResidencyGuardV4<'a> {
    owner: &'a KagemushaSourceRuntimeHeavyResidencyV4,
    parity: KagemushaPastaCycleParityV1,
}

impl Drop for KagemushaSourceRuntimeHeavyResidencyGuardV4<'_> {
    fn drop(&mut self) {
        if self.owner.active.get() == Some(self.parity) {
            self.owner.active.set(None);
        }
        #[cfg(test)]
        self.owner.events.borrow_mut().push((self.parity, false));
    }
}

fn qualified_kagemusha_structure_sha256_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String> {
    let profile = match parity {
        KagemushaPastaCycleParityV1::StepEq => {
            source.authenticated_release().manifest().profiles.first()
        }
        KagemushaPastaCycleParityV1::StepEp => {
            source.authenticated_release().manifest().profiles.get(1)
        }
    }
    .ok_or_else(|| "Kagemusha V4 qualified release omits a parity profile".to_owned())?;
    let metadata = match parity {
        KagemushaPastaCycleParityV1::StepEq => source.step_eq(),
        KagemushaPastaCycleParityV1::StepEp => source.step_ep(),
    };
    if profile.parity != parity
        || profile.circuit_params != *metadata.circuit_params()
        || profile.ipa_k != metadata.circuit_params().k
        || profile.compiled_protocol_structure_sha256 == [0; 32]
    {
        return Err("Kagemusha V4 qualified profile metadata changed".to_owned());
    }
    Ok(profile.compiled_protocol_structure_sha256)
}

fn ensure_kagemusha_eq_verifying_material_identity_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    expected_identity: [u8; 32],
) -> Result<(), String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 Eq public length does not fit usize".to_owned())?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    if kagemusha_compiled_protocol_identity_sha256(&protocol, KagemushaPastaCycleParityV1::StepEq)?
        != expected_identity
    {
        return Err("Kagemusha V4 Eq runtime protocol identity changed".to_owned());
    }
    Ok(())
}

fn ensure_kagemusha_ep_verifying_material_identity_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    expected_identity: [u8; 32],
) -> Result<(), String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 Ep public length does not fit usize".to_owned())?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    if kagemusha_compiled_protocol_identity_sha256(&protocol, KagemushaPastaCycleParityV1::StepEp)?
        != expected_identity
    {
        return Err("Kagemusha V4 Ep runtime protocol identity changed".to_owned());
    }
    Ok(())
}

struct KagemushaSourceEqVerifierMaterialV4 {
    params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    verifying_key: halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit_params: KagemushaStepCircuitParamsV4,
}

struct KagemushaSourceEpVerifierMaterialV4 {
    params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    verifying_key: halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit_params: KagemushaStepCircuitParamsV4,
}

fn load_kagemusha_source_eq_verifier_material_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
) -> Result<KagemushaSourceEqVerifierMaterialV4, String> {
    let metadata = source.step_eq();
    let circuit_params = metadata.circuit_params().clone();
    let params = load_kagemusha_eq_params_from_source_v4(source, circuit_params.k)?;
    let loaded = load_kagemusha_eq_verifying_key_from_source_v4(source, &circuit_params)?;
    if loaded.processed_len != metadata.processed_verifying_key_len()
        || loaded.processed_sha256 != metadata.processed_verifying_key_sha256()
        || loaded.commitment != metadata.verifying_key_commitment()
    {
        return Err("Kagemusha V4 Eq qualified verifier metadata changed".to_owned());
    }
    ensure_kagemusha_eq_verifying_material_identity_v4(
        &params,
        &loaded.key,
        &circuit_params,
        metadata.compiled_protocol_identity_sha256(),
    )?;
    Ok(KagemushaSourceEqVerifierMaterialV4 {
        params,
        verifying_key: loaded.key,
        circuit_params,
    })
}

fn load_kagemusha_source_ep_verifier_material_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
) -> Result<KagemushaSourceEpVerifierMaterialV4, String> {
    let metadata = source.step_ep();
    let circuit_params = metadata.circuit_params().clone();
    let params = load_kagemusha_ep_params_from_source_v4(source, circuit_params.k)?;
    let loaded = load_kagemusha_ep_verifying_key_from_source_v4(source, &circuit_params)?;
    if loaded.processed_len != metadata.processed_verifying_key_len()
        || loaded.processed_sha256 != metadata.processed_verifying_key_sha256()
        || loaded.commitment != metadata.verifying_key_commitment()
    {
        return Err("Kagemusha V4 Ep qualified verifier metadata changed".to_owned());
    }
    ensure_kagemusha_ep_verifying_material_identity_v4(
        &params,
        &loaded.key,
        &circuit_params,
        metadata.compiled_protocol_identity_sha256(),
    )?;
    Ok(KagemushaSourceEpVerifierMaterialV4 {
        params,
        verifying_key: loaded.key,
        circuit_params,
    })
}

fn verify_kagemusha_source_eq_pair_lineage_v4(
    material: &KagemushaSourceEqVerifierMaterialV4,
    pair: &KagemushaPastaCycleProofPairV4,
) -> Result<KagemushaIpaAccumulatorWireV4, String> {
    let instances = vec![pair.public_inputs.instance_column::<Fp>(
        &material.circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
    )?];
    let current = succinct_verify_step_eq_instances(
        &material.params,
        &material.verifying_key,
        &pair.step_eq_proof_bytes,
        &instances,
        usize::try_from(material.circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?,
    )?;
    let parent = pair
        .public_inputs
        .parent_eq_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_eq(material.circuit_params.k))
        .transpose()?;
    let lineage = super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        &material.params,
        material.circuit_params.k,
        current,
        parent,
        &pair.step_eq_accumulation_proof,
    )?;
    KagemushaIpaAccumulatorWireV4::from_eq(&lineage, material.circuit_params.k)
}

fn verify_kagemusha_source_ep_pair_lineage_v4(
    material: &KagemushaSourceEpVerifierMaterialV4,
    pair: &KagemushaPastaCycleProofPairV4,
) -> Result<KagemushaIpaAccumulatorWireV4, String> {
    let instances = vec![pair.public_inputs.instance_column::<Fq>(
        &material.circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
    )?];
    let current = succinct_verify_step_ep_instances(
        &material.params,
        &material.verifying_key,
        &pair.step_ep_proof_bytes,
        &instances,
        usize::try_from(material.circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?,
    )?;
    let parent = pair
        .public_inputs
        .parent_ep_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_ep(material.circuit_params.k))
        .transpose()?;
    let lineage = super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        &material.params,
        material.circuit_params.k,
        current,
        parent,
        &pair.step_ep_accumulation_proof,
    )?;
    KagemushaIpaAccumulatorWireV4::from_ep(&lineage, material.circuit_params.k)
}

/// Source-backed terminal verifier retaining only qualified release metadata.
pub(crate) struct KagemushaPastaCycleSourceBackedVerifierV4 {
    source: Arc<super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4>,
    max_pair_bytes: u32,
}

impl KagemushaPastaCycleSourceBackedVerifierV4 {
    pub(crate) fn new(
        source: Arc<super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4>,
    ) -> Result<Self, String> {
        let max_pair_bytes = source.authenticated_release().manifest().max_proof_bytes;
        if max_pair_bytes == 0
            || max_pair_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
        {
            return Err("Kagemusha V4 qualified proof-pair bound is invalid".to_owned());
        }
        Ok(Self {
            source,
            max_pair_bytes,
        })
    }

    fn decode_pair(&self, bytes: &[u8]) -> Result<KagemushaPastaCycleProofPairV4, String> {
        KagemushaPastaCycleProofPairV4::decode_authenticated(
            bytes,
            self.source.step_eq().circuit_params(),
            self.source.step_ep().circuit_params(),
            self.max_pair_bytes,
        )
    }

    fn verify_pair(&self, pair: &KagemushaPastaCycleProofPairV4) -> Result<(), String> {
        let residency = KagemushaSourceRuntimeHeavyResidencyV4::default();
        pair.validate(
            self.source.step_eq().circuit_params(),
            self.source.step_ep().circuit_params(),
            self.max_pair_bytes,
        )?;
        let manifest_chunks =
            kagemusha_u32_words_to_u128_chunks_v5(&kagemusha_exact_u32_public_limbs(
                self.source.authenticated_release().manifest_sha256(),
            ));
        let eq_protocol_chunks =
            kagemusha_u32_words_to_u128_chunks_v5(&kagemusha_sha256_public_words(
                self.source.step_eq().compiled_protocol_identity_sha256(),
            ));
        let ep_protocol_chunks =
            kagemusha_u32_words_to_u128_chunks_v5(&kagemusha_sha256_public_words(
                self.source.step_ep().compiled_protocol_identity_sha256(),
            ));
        if pair.public_inputs.common_header[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
            != manifest_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
                ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
                != eq_protocol_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
                ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
                != ep_protocol_chunks
        {
            return Err(
                "Kagemusha V4 pair selects different qualified protocol identities".to_owned(),
            );
        }
        {
            let _eq_residency = residency.enter(KagemushaPastaCycleParityV1::StepEq)?;
            let eq = load_kagemusha_source_eq_verifier_material_v4(&self.source)?;
            verify_kagemusha_source_eq_pair_lineage_v4(&eq, pair)?;
        }
        {
            let _ep_residency = residency.enter(KagemushaPastaCycleParityV1::StepEp)?;
            let ep = load_kagemusha_source_ep_verifier_material_v4(&self.source)?;
            verify_kagemusha_source_ep_pair_lineage_v4(&ep, pair)?;
        }
        residency.assert_released()
    }

    pub(crate) fn verify_encoded_pair_binding(
        &self,
        bytes: &[u8],
        expected_statement: &KagemushaRecursiveSpendPublicStatementV4,
        expected_operation: &KagemushaStepOperationVectorV4,
        expected_statement_digest: [u32; 8],
        expected_state: &[u32],
        expected_proof_step_count: u32,
        expected_manifest_sha256: [u32; 8],
    ) -> Result<(), String> {
        let _permit = lock_kagemusha_source_runtime_heavy_v4();
        let pair = self.decode_pair(bytes)?;
        expected_operation.validate_terminal_statement_v4(expected_statement)?;
        let expected_statement_chunks =
            kagemusha_u32_words_to_u128_chunks_v5(&expected_statement_digest);
        let expected_operation_chunks = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_DOMAIN_V5,
            &expected_operation.limbs,
        );
        let expected_state_chunks = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            expected_state,
        );
        let expected_manifest_chunks =
            kagemusha_u32_words_to_u128_chunks_v5(&expected_manifest_sha256);
        if pair.proof_step_count != expected_proof_step_count
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
                ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2]
                != expected_statement_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5
                ..KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5 + 2]
                != expected_operation_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5
                ..KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + 2]
                != expected_state_chunks
            || pair.public_inputs.common_header[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
                != expected_manifest_chunks
        {
            return Err(
                "Kagemusha V4 proof pair does not match the canonical public statement".to_owned(),
            );
        }
        self.verify_pair(&pair)
    }

    pub(crate) fn verify_encoded_pair_qualification(&self, bytes: &[u8]) -> Result<(), String> {
        let _permit = lock_kagemusha_source_runtime_heavy_v4();
        let pair = self.decode_pair(bytes)?;
        self.verify_pair(&pair)
    }
}

struct KagemushaSourceEqProverMaterialV4 {
    params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    break_points: KagemushaBreakPointsV4,
    circuit_params: KagemushaStepCircuitParamsV4,
}

struct KagemushaSourceEpProverMaterialV4 {
    params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    break_points: KagemushaBreakPointsV4,
    circuit_params: KagemushaStepCircuitParamsV4,
}

struct KagemushaSourceEqRecursionMaterialV4 {
    verifier: KagemushaSourceEqVerifierMaterialV4,
    bootstrap: KagemushaStepBootstrapV4,
    compiled_parent_protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
}

struct KagemushaSourceEpRecursionMaterialV4 {
    verifier: KagemushaSourceEpVerifierMaterialV4,
    bootstrap: KagemushaStepBootstrapV4,
    compiled_parent_protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
}

fn load_kagemusha_source_eq_prover_material_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
    bootstrap_break_points: &[Vec<u32>],
) -> Result<KagemushaSourceEqProverMaterialV4, String> {
    let metadata = source.step_eq();
    let circuit_params = metadata.circuit_params().clone();
    let break_points =
        kagemusha_break_points_from_wire_v5(bootstrap_break_points, &circuit_params)?;
    let params = load_kagemusha_eq_params_from_source_v4(source, circuit_params.k)?;
    let loaded_key =
        load_kagemusha_eq_proving_key_from_qualified_source_v4(source, &circuit_params)?;
    ensure_kagemusha_eq_verifying_material_identity_v4(
        &params,
        loaded_key.key.get_vk(),
        &circuit_params,
        metadata.compiled_protocol_identity_sha256(),
    )?;
    Ok(KagemushaSourceEqProverMaterialV4 {
        params,
        proving_key: loaded_key.key,
        break_points,
        circuit_params,
    })
}

fn load_kagemusha_source_ep_prover_material_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
    bootstrap_break_points: &[Vec<u32>],
) -> Result<KagemushaSourceEpProverMaterialV4, String> {
    let metadata = source.step_ep();
    let circuit_params = metadata.circuit_params().clone();
    let break_points =
        kagemusha_break_points_from_wire_v5(bootstrap_break_points, &circuit_params)?;
    let params = load_kagemusha_ep_params_from_source_v4(source, circuit_params.k)?;
    let loaded_key =
        load_kagemusha_ep_proving_key_from_qualified_source_v4(source, &circuit_params)?;
    ensure_kagemusha_ep_verifying_material_identity_v4(
        &params,
        loaded_key.key.get_vk(),
        &circuit_params,
        metadata.compiled_protocol_identity_sha256(),
    )?;
    Ok(KagemushaSourceEpProverMaterialV4 {
        params,
        proving_key: loaded_key.key,
        break_points,
        circuit_params,
    })
}

fn load_kagemusha_source_eq_recursion_material_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
) -> Result<KagemushaSourceEqRecursionMaterialV4, String> {
    let verifier = load_kagemusha_source_eq_verifier_material_v4(source)?;
    let structure_sha256 =
        qualified_kagemusha_structure_sha256_v4(source, KagemushaPastaCycleParityV1::StepEq)?;
    let bootstrap_bytes = read_bounded_kagemusha_bootstrap_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let (bootstrap, identity, compiled_parent_protocol) = validate_kagemusha_profile_protocol_v4(
        &verifier.params,
        &verifier.verifying_key,
        &verifier.circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        structure_sha256,
        &bootstrap_bytes,
    )?;
    if identity != source.step_eq().compiled_protocol_identity_sha256() {
        return Err("Kagemusha V4 Eq qualified recursion protocol identity changed".to_owned());
    }
    terminal_validate_kagemusha_eq_bootstrap_v4(
        &verifier.params,
        &verifier.verifying_key,
        &verifier.circuit_params,
        &bootstrap,
    )?;
    let succinct_vk = kagemusha_eq_succinct_vk_v4(&verifier.params)?;
    Ok(KagemushaSourceEqRecursionMaterialV4 {
        verifier,
        bootstrap,
        compiled_parent_protocol,
        succinct_vk,
    })
}

fn load_kagemusha_source_ep_recursion_material_v4(
    source: &super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4,
) -> Result<KagemushaSourceEpRecursionMaterialV4, String> {
    let verifier = load_kagemusha_source_ep_verifier_material_v4(source)?;
    let structure_sha256 =
        qualified_kagemusha_structure_sha256_v4(source, KagemushaPastaCycleParityV1::StepEp)?;
    let bootstrap_bytes = read_bounded_kagemusha_bootstrap_from_source_v4(
        source,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let (bootstrap, identity, compiled_parent_protocol) = validate_kagemusha_profile_protocol_v4(
        &verifier.params,
        &verifier.verifying_key,
        &verifier.circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        structure_sha256,
        &bootstrap_bytes,
    )?;
    if identity != source.step_ep().compiled_protocol_identity_sha256() {
        return Err("Kagemusha V4 Ep qualified recursion protocol identity changed".to_owned());
    }
    terminal_validate_kagemusha_ep_bootstrap_v4(
        &verifier.params,
        &verifier.verifying_key,
        &verifier.circuit_params,
        &bootstrap,
    )?;
    let succinct_vk = kagemusha_ep_succinct_vk_v4(&verifier.params)?;
    Ok(KagemushaSourceEpRecursionMaterialV4 {
        verifier,
        bootstrap,
        compiled_parent_protocol,
        succinct_vk,
    })
}

fn kagemusha_source_eq_parent_from_pair_v4(
    pair: &KagemushaPastaCycleProofPairV4,
    bootstrap: &KagemushaStepBootstrapV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    let instances = vec![
        pair.public_inputs
            .instance_column::<Fp>(circuit_params, KagemushaPastaCycleParityV1::StepEq)?,
    ];
    let (carried_lineage, external_accumulation_proof) = if pair.public_inputs.parent_count()? == 0
    {
        (
            bootstrap
                .parent_slot
                .carried_lineage
                .to_eq(circuit_params.k)?,
            bootstrap.parent_slot.post_proof_fold.clone(),
        )
    } else {
        (
            pair.public_inputs
                .parent_eq_lineage_accumulator
                .as_ref()
                .ok_or_else(|| "Kagemusha V4 Eq parent omitted its carried lineage".to_owned())?
                .to_eq(circuit_params.k)?,
            pair.step_eq_accumulation_proof.clone(),
        )
    };
    Ok(KagemushaStepParentProofV4 {
        instances,
        proof_bytes: pair.step_eq_proof_bytes.clone(),
        carried_lineage,
        external_accumulation_proof,
    })
}

fn kagemusha_source_ep_parent_from_pair_v4(
    pair: &KagemushaPastaCycleProofPairV4,
    bootstrap: &KagemushaStepBootstrapV4,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    let instances = vec![
        pair.public_inputs
            .instance_column::<Fq>(circuit_params, KagemushaPastaCycleParityV1::StepEp)?,
    ];
    let (carried_lineage, external_accumulation_proof) = if pair.public_inputs.parent_count()? == 0
    {
        (
            bootstrap
                .parent_slot
                .carried_lineage
                .to_ep(circuit_params.k)?,
            bootstrap.parent_slot.post_proof_fold.clone(),
        )
    } else {
        (
            pair.public_inputs
                .parent_ep_lineage_accumulator
                .as_ref()
                .ok_or_else(|| "Kagemusha V4 Ep parent omitted its carried lineage".to_owned())?
                .to_ep(circuit_params.k)?,
            pair.step_ep_accumulation_proof.clone(),
        )
    };
    Ok(KagemushaStepParentProofV4 {
        instances,
        proof_bytes: pair.step_ep_proof_bytes.clone(),
        carried_lineage,
        external_accumulation_proof,
    })
}

struct KagemushaPreparedSourceRecursionsV4 {
    step_eq: KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep: KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_eq_bootstrap: KagemushaStepBootstrapV4,
    step_ep_bootstrap: KagemushaStepBootstrapV4,
}

/// Source-backed prover retaining no parsed parameters or proving keys.
pub(crate) struct KagemushaPastaCycleSourceBackedProverV4 {
    source: Arc<super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4>,
    manifest_sha256: [u8; 32],
    max_pair_bytes: u32,
}

impl KagemushaPastaCycleSourceBackedProverV4 {
    pub(crate) fn new(
        source: Arc<super::kagemusha_artifact_source_v4::KagemushaQualifiedArtifactSourceV4>,
    ) -> Result<Self, String> {
        let manifest_sha256 = source.authenticated_release().manifest_sha256();
        let max_pair_bytes = source.authenticated_release().manifest().max_proof_bytes;
        if max_pair_bytes == 0
            || max_pair_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
        {
            return Err("Kagemusha V4 qualified proof-pair bound is invalid".to_owned());
        }
        Ok(Self {
            source,
            manifest_sha256,
            max_pair_bytes,
        })
    }

    pub(crate) fn step_eq_compiled_protocol_sha256(&self) -> [u8; 32] {
        self.source.step_eq().compiled_protocol_identity_sha256()
    }

    pub(crate) fn step_ep_compiled_protocol_sha256(&self) -> [u8; 32] {
        self.source.step_ep().compiled_protocol_identity_sha256()
    }

    fn decode_parent_pairs_v4(
        &self,
        parent_pair_bytes: &[&[u8]],
    ) -> Result<Vec<KagemushaPastaCycleProofPairV4>, String> {
        if parent_pair_bytes.len() > KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            return Err("Kagemusha V4 operation consumes more than two parents".to_owned());
        }
        let manifest_chunks = kagemusha_u32_words_to_u128_chunks_v5(
            &kagemusha_exact_u32_public_limbs(self.manifest_sha256),
        );
        let eq_protocol_chunks = kagemusha_u32_words_to_u128_chunks_v5(
            &kagemusha_sha256_public_words(self.step_eq_compiled_protocol_sha256()),
        );
        let ep_protocol_chunks = kagemusha_u32_words_to_u128_chunks_v5(
            &kagemusha_sha256_public_words(self.step_ep_compiled_protocol_sha256()),
        );
        parent_pair_bytes
            .iter()
            .map(|bytes| {
                let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
                    bytes,
                    self.source.step_eq().circuit_params(),
                    self.source.step_ep().circuit_params(),
                    self.max_pair_bytes,
                )?;
                if pair.public_inputs.common_header[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
                    != manifest_chunks
                    || pair.public_inputs.common_header
                        [KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
                            ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
                        != eq_protocol_chunks
                    || pair.public_inputs.common_header
                        [KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
                            ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
                        != ep_protocol_chunks
                {
                    return Err(
                        "Kagemusha V4 parent pair belongs to a different authenticated release"
                            .to_owned(),
                    );
                }
                Ok(pair)
            })
            .collect()
    }

    fn prepare_source_recursions_v4(
        &self,
        public_inputs: &mut KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
        parent_state_openings: &[Vec<u32>],
        residency: &KagemushaSourceRuntimeHeavyResidencyV4,
    ) -> Result<KagemushaPreparedSourceRecursionsV4, String> {
        if parent_pair_bytes.len() != parent_state_openings.len() {
            return Err(
                "Kagemusha V4 operation requires one state opening per parent pair".to_owned(),
            );
        }
        let parents = self.decode_parent_pairs_v4(parent_pair_bytes)?;
        for (pair, opening) in parents.iter().zip(parent_state_openings) {
            if opening.len()
                != iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2
            {
                return Err("Kagemusha V4 parent state opening has invalid length".to_owned());
            }
            let opening_commitment = kagemusha_poseidon_commitment_chunks_v5(
                KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
                opening,
            );
            if pair.public_inputs.common_header[KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5
                ..KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + 2]
                != opening_commitment
            {
                return Err(
                    "Kagemusha V4 parent state opening does not match its proof pair".to_owned(),
                );
            }
        }
        public_inputs.parent_count = u32::try_from(parents.len())
            .map_err(|_| "Kagemusha V4 parent count does not fit u32".to_owned())?;
        public_inputs.manifest_sha256 = kagemusha_exact_u32_public_limbs(self.manifest_sha256);
        public_inputs.step_eq_compiled_protocol_sha256 =
            kagemusha_sha256_public_words(self.step_eq_compiled_protocol_sha256());
        public_inputs.step_ep_compiled_protocol_sha256 =
            kagemusha_sha256_public_words(self.step_ep_compiled_protocol_sha256());
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            public_inputs.parent_states[slot] = parent_state_openings.get(slot).map_or_else(
                || {
                    vec![
                        0;
                        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2
                    ]
                },
                Clone::clone,
            );
            public_inputs.parent_eq_deferred_sha256[slot] = [0; 8];
            public_inputs.parent_ep_deferred_sha256[slot] = [0; 8];
        }

        let (step_eq, step_eq_bootstrap, parent_eq_lineage_accumulator) = {
            let _eq_residency = residency.enter(KagemushaPastaCycleParityV1::StepEq)?;
            let material = load_kagemusha_source_eq_recursion_material_v4(&self.source)?;
            let lineages = parents
                .iter()
                .map(|pair| verify_kagemusha_source_eq_pair_lineage_v4(&material.verifier, pair))
                .collect::<Result<Vec<_>, _>>()?;
            let (parent_lineage, branch_merge_fold) = match lineages.as_slice() {
                [] => (None, material.bootstrap.branch_merge_fold.clone()),
                [lineage] => (
                    Some(lineage.clone()),
                    material.bootstrap.branch_merge_fold.clone(),
                ),
                [first, second] => {
                    let (fold, accumulated) =
                        super::kagemusha_accumulation::fold_eq_accumulators_v4(
                            &material.verifier.params,
                            material.verifier.circuit_params.k,
                            first.to_eq(material.verifier.circuit_params.k)?,
                            Some(second.to_eq(material.verifier.circuit_params.k)?),
                        )?;
                    (
                        Some(KagemushaIpaAccumulatorWireV4::from_eq(
                            &accumulated,
                            material.verifier.circuit_params.k,
                        )?),
                        fold,
                    )
                }
                _ => {
                    return Err("Kagemusha V4 Eq parent lineage count exceeds two".to_owned());
                }
            };
            let mut witnesses = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
            for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
                if let Some(pair) = parents.get(slot) {
                    witnesses.push(kagemusha_source_eq_parent_from_pair_v4(
                        pair,
                        &material.bootstrap,
                        &material.verifier.circuit_params,
                    )?);
                } else {
                    witnesses.push(material.bootstrap.step_eq_parent(
                        &material.verifier.circuit_params,
                        material.bootstrap.compiled_protocol_structure_sha256,
                        slot,
                    )?);
                }
            }
            let parents: [KagemushaStepParentProofV4<_>; KAGEMUSHA_PASTA_PARENT_SLOTS_V1] =
                witnesses.try_into().map_err(|witnesses: Vec<_>| {
                    format!(
                        "Kagemusha V4 Eq recursion has {} parents instead of two",
                        witnesses.len()
                    )
                })?;
            let recursion = KagemushaStepParityRecursionV4 {
                succinct_vk: material.succinct_vk.clone(),
                compiled_parent_protocol: material.compiled_parent_protocol.clone(),
                fixed_structure_sha256: material.bootstrap.compiled_protocol_structure_sha256,
                parents,
                branch_merge_fold,
            };
            (recursion, material.bootstrap.clone(), parent_lineage)
        };

        let (step_ep, step_ep_bootstrap, parent_ep_lineage_accumulator) = {
            let _ep_residency = residency.enter(KagemushaPastaCycleParityV1::StepEp)?;
            let material = load_kagemusha_source_ep_recursion_material_v4(&self.source)?;
            let lineages = parents
                .iter()
                .map(|pair| verify_kagemusha_source_ep_pair_lineage_v4(&material.verifier, pair))
                .collect::<Result<Vec<_>, _>>()?;
            let (parent_lineage, branch_merge_fold) = match lineages.as_slice() {
                [] => (None, material.bootstrap.branch_merge_fold.clone()),
                [lineage] => (
                    Some(lineage.clone()),
                    material.bootstrap.branch_merge_fold.clone(),
                ),
                [first, second] => {
                    let (fold, accumulated) =
                        super::kagemusha_accumulation::fold_ep_accumulators_v4(
                            &material.verifier.params,
                            material.verifier.circuit_params.k,
                            first.to_ep(material.verifier.circuit_params.k)?,
                            Some(second.to_ep(material.verifier.circuit_params.k)?),
                        )?;
                    (
                        Some(KagemushaIpaAccumulatorWireV4::from_ep(
                            &accumulated,
                            material.verifier.circuit_params.k,
                        )?),
                        fold,
                    )
                }
                _ => {
                    return Err("Kagemusha V4 Ep parent lineage count exceeds two".to_owned());
                }
            };
            let mut witnesses = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
            for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
                if let Some(pair) = parents.get(slot) {
                    witnesses.push(kagemusha_source_ep_parent_from_pair_v4(
                        pair,
                        &material.bootstrap,
                        &material.verifier.circuit_params,
                    )?);
                } else {
                    witnesses.push(material.bootstrap.step_ep_parent(
                        &material.verifier.circuit_params,
                        material.bootstrap.compiled_protocol_structure_sha256,
                        slot,
                    )?);
                }
            }
            let parents: [KagemushaStepParentProofV4<_>; KAGEMUSHA_PASTA_PARENT_SLOTS_V1] =
                witnesses.try_into().map_err(|witnesses: Vec<_>| {
                    format!(
                        "Kagemusha V4 Ep recursion has {} parents instead of two",
                        witnesses.len()
                    )
                })?;
            let recursion = KagemushaStepParityRecursionV4 {
                succinct_vk: material.succinct_vk.clone(),
                compiled_parent_protocol: material.compiled_parent_protocol.clone(),
                fixed_structure_sha256: material.bootstrap.compiled_protocol_structure_sha256,
                parents,
                branch_merge_fold,
            };
            (recursion, material.bootstrap.clone(), parent_lineage)
        };

        public_inputs.parent_eq_lineage_accumulator = parent_eq_lineage_accumulator;
        public_inputs.parent_ep_lineage_accumulator = parent_ep_lineage_accumulator;
        if public_inputs.parent_count == 0 {
            public_inputs.parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
            public_inputs.parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        } else {
            let eq_public_words = {
                let audits = collect_kagemusha_scalar_audits_v4::<
                    halo2_proofs::halo2curves::pasta::EqAffine,
                >(
                    public_inputs,
                    proof_step_count,
                    self.source.step_eq().circuit_params(),
                    &step_eq,
                    KagemushaPastaCycleParityV1::StepEq,
                )?;
                kagemusha_deferred_audit_public_words_v5(
                    &audits.audit,
                    &audits.stages,
                    public_inputs.parent_count,
                    audits.inner_parent_counts,
                )?
            };
            let ep_public_words = {
                let audits = collect_kagemusha_scalar_audits_v4::<
                    halo2_proofs::halo2curves::pasta::EpAffine,
                >(
                    public_inputs,
                    proof_step_count,
                    self.source.step_ep().circuit_params(),
                    &step_ep,
                    KagemushaPastaCycleParityV1::StepEp,
                )?;
                kagemusha_deferred_audit_public_words_v5(
                    &audits.audit,
                    &audits.stages,
                    public_inputs.parent_count,
                    audits.inner_parent_counts,
                )?
            };
            public_inputs.parent_eq_deferred_sha256 = eq_public_words;
            public_inputs.parent_ep_deferred_sha256 = ep_public_words;
        }
        let eq_layout =
            public_inputs.validate(proof_step_count, self.source.step_eq().circuit_params())?;
        let ep_layout =
            public_inputs.validate(proof_step_count, self.source.step_ep().circuit_params())?;
        if eq_layout != ep_layout {
            return Err("Kagemusha V4 prepared Eq/Ep public layouts differ".to_owned());
        }
        Ok(KagemushaPreparedSourceRecursionsV4 {
            step_eq,
            step_ep,
            step_eq_bootstrap,
            step_ep_bootstrap,
        })
    }

    fn validate_output_frontier_v4(
        public_inputs: &KagemushaPastaCyclePublicInputsV4,
        output_membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    ) -> Result<(), String> {
        let result_frontier = public_inputs
            .result_state
            .get(super::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX)
            .copied()
            .ok_or_else(|| "Kagemusha V4 result state omits its frontier".to_owned())?;
        if result_frontier != output_membership.dummy_leaf_index {
            return Err("Kagemusha V4 result state/frontier witness mismatch".to_owned());
        }
        let expected_parent_frontier = match output_membership.operation {
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::Init => None,
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::Split => output_membership
                .recipient
                .as_ref()
                .map(|leaf| leaf.leaf_index),
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::RedemptionChange => {
                output_membership
                    .change
                    .as_ref()
                    .map(|leaf| leaf.leaf_index)
            }
        };
        match expected_parent_frontier {
            None if public_inputs.parent_count == 0 => Ok(()),
            Some(expected) if public_inputs.parent_count > 0 => {
                for parent in public_inputs
                    .parent_states
                    .iter()
                    .take(public_inputs.parent_count as usize)
                {
                    if parent
                        .get(super::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX)
                        .copied()
                        != Some(expected)
                    {
                        return Err(
                            "Kagemusha V4 output insertion does not start at the parent frontier"
                                .to_owned(),
                        );
                    }
                }
                Ok(())
            }
            _ => Err("Kagemusha V4 membership/parent profile mismatch".to_owned()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_operation_encoded_v4(
        &self,
        mut public_inputs: KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
        parent_state_openings: &[Vec<u32>],
        secure: &super::confidential_v2::KagemushaStepSecureWitnessV3,
        output_membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    ) -> Result<Vec<u8>, String> {
        let _permit = lock_kagemusha_source_runtime_heavy_v4();
        let residency = KagemushaSourceRuntimeHeavyResidencyV4::default();
        let prepared = self.prepare_source_recursions_v4(
            &mut public_inputs,
            proof_step_count,
            parent_pair_bytes,
            parent_state_openings,
            &residency,
        )?;
        Self::validate_output_frontier_v4(&public_inputs, output_membership)?;

        let ep_reciprocal_output =
            collect_kagemusha_scalar_audits_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
                &public_inputs,
                proof_step_count,
                self.source.step_ep().circuit_params(),
                &prepared.step_ep,
                KagemushaPastaCycleParityV1::StepEp,
            )?;

        let (step_eq_proof_bytes, step_eq_accumulation_proof, eq_reciprocal_output) = {
            let _eq_residency = residency.enter(KagemushaPastaCycleParityV1::StepEq)?;
            let KagemushaSourceEqProverMaterialV4 {
                params,
                proving_key,
                break_points,
                circuit_params,
            } = load_kagemusha_source_eq_prover_material_v4(
                &self.source,
                &prepared.step_eq_bootstrap.circuit_break_points,
            )?;
            let witness = KagemushaStepWitnessV4 {
                public_inputs: &public_inputs,
                proof_step_count,
                secure,
                output_membership,
                step_eq_recursion: &prepared.step_eq,
                step_ep_recursion: &prepared.step_ep,
                step_eq_bootstrap: Some(&prepared.step_eq_bootstrap),
                step_ep_bootstrap: Some(&prepared.step_ep_bootstrap),
            };
            let (circuit, reciprocal_output) = build_kagemusha_step_eq_circuit_sequential_v4(
                &witness,
                circuit_params.clone(),
                self.source.step_ep().circuit_params(),
                KagemushaStepPublicModeV4::Live,
                Some(&break_points),
                &ep_reciprocal_output,
            )?;
            let (proof, verifying_key) = prove_step_eq_v4(
                &params,
                proving_key,
                circuit,
                &public_inputs,
                proof_step_count,
                &circuit_params,
            )?;
            let instances = vec![public_inputs.instance_column::<Fp>(
                proof_step_count,
                &circuit_params,
                KagemushaPastaCycleParityV1::StepEq,
            )?];
            let current = succinct_verify_step_eq_instances(
                &params,
                &verifying_key,
                &proof,
                &instances,
                usize::try_from(circuit_params.max_parent_proof_bytes)
                    .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?,
            )?;
            drop(verifying_key);
            let parent = public_inputs
                .parent_eq_lineage_accumulator
                .as_ref()
                .map(|wire| wire.to_eq(circuit_params.k))
                .transpose()?;
            let (fold, _) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
                &params,
                circuit_params.k,
                current.clone(),
                parent.clone(),
            )?;
            super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
                &params,
                circuit_params.k,
                current,
                parent,
                &fold,
            )?;
            (proof, fold, reciprocal_output)
        };
        drop(ep_reciprocal_output);

        let (step_ep_proof_bytes, step_ep_accumulation_proof) = {
            let _ep_residency = residency.enter(KagemushaPastaCycleParityV1::StepEp)?;
            let KagemushaSourceEpProverMaterialV4 {
                params,
                proving_key,
                break_points,
                circuit_params,
            } = load_kagemusha_source_ep_prover_material_v4(
                &self.source,
                &prepared.step_ep_bootstrap.circuit_break_points,
            )?;
            let witness = KagemushaStepWitnessV4 {
                public_inputs: &public_inputs,
                proof_step_count,
                secure,
                output_membership,
                step_eq_recursion: &prepared.step_eq,
                step_ep_recursion: &prepared.step_ep,
                step_eq_bootstrap: Some(&prepared.step_eq_bootstrap),
                step_ep_bootstrap: Some(&prepared.step_ep_bootstrap),
            };
            let (circuit, _) = build_kagemusha_step_ep_circuit_sequential_v4(
                &witness,
                self.source.step_eq().circuit_params(),
                circuit_params.clone(),
                KagemushaStepPublicModeV4::Live,
                Some(&break_points),
                &eq_reciprocal_output,
            )?;
            let (proof, verifying_key) = prove_step_ep_v4(
                &params,
                proving_key,
                circuit,
                &public_inputs,
                proof_step_count,
                &circuit_params,
            )?;
            let instances = vec![public_inputs.instance_column::<Fq>(
                proof_step_count,
                &circuit_params,
                KagemushaPastaCycleParityV1::StepEp,
            )?];
            let current = succinct_verify_step_ep_instances(
                &params,
                &verifying_key,
                &proof,
                &instances,
                usize::try_from(circuit_params.max_parent_proof_bytes)
                    .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?,
            )?;
            drop(verifying_key);
            let parent = public_inputs
                .parent_ep_lineage_accumulator
                .as_ref()
                .map(|wire| wire.to_ep(circuit_params.k))
                .transpose()?;
            let (fold, _) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
                &params,
                circuit_params.k,
                current.clone(),
                parent.clone(),
            )?;
            super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
                &params,
                circuit_params.k,
                current,
                parent,
                &fold,
            )?;
            (proof, fold)
        };
        drop(eq_reciprocal_output);

        let compact_public_inputs =
            KagemushaCompactPublicInputsV5::from_private(&public_inputs, proof_step_count);
        let pair = KagemushaPastaCycleProofPairV4 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
            proof_step_count,
            public_inputs: compact_public_inputs,
            step_eq_proof_bytes,
            step_ep_proof_bytes,
            step_eq_accumulation_proof,
            step_ep_accumulation_proof,
        };
        let encoded = pair.encode_authenticated(
            self.source.step_eq().circuit_params(),
            self.source.step_ep().circuit_params(),
            self.max_pair_bytes,
        )?;
        residency.assert_released()?;
        Ok(encoded)
    }
}

/// Circuit-side parent-proof and lineage-accumulation primitives shared by
/// the fixed StepEq and StepEp builders.
mod scalar_lineage_v1 {
    use std::{
        cell::Cell,
        io::{self, Read},
        ops::Range,
        rc::Rc,
    };

    use halo2_base::{
        AssignedValue,
        QuantumCell::{Constant, Existing},
        gates::{GateInstructions, RangeInstructions},
        utils::{BigPrimeField, CurveAffineExt},
    };
    use halo2_proofs::halo2curves::ff::Field as _;
    use snark_verifier::{
        Error,
        loader::{halo2::Halo2Loader, native::NativeLoader},
        pcs::{
            AccumulationScheme,
            ipa::{Bgh19, IpaAccumulator, IpaAs, IpaSuccinctVerifyingKey},
        },
        system::halo2::transcript::halo2::PoseidonTranscript,
        verifier::{
            SnarkVerifier,
            plonk::{PlonkProtocol, PlonkSuccinctVerifier},
        },
    };

    use super::{
        KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5, KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1,
        KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1, KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS, KAGEMUSHA_POSEIDON_RATE, KAGEMUSHA_POSEIDON_SECURE_MDS,
        KAGEMUSHA_POSEIDON_WIDTH, KagemushaPastaCycleParityV1, KagemushaSha256ByteV4,
        KagemushaSha256JobsV4, kagemusha_compiled_protocol_structure_sha256, protocol_parity_tag,
    };
    use crate::zk::{
        kagemusha_accumulation::{
            KagemushaIpaAccumulationProofV4, kagemusha_ipa_accumulation_proof_bytes_v4,
        },
        kagemusha_cycle_loader::DeferredScalarEccChip,
    };

    type DeferredLoader<'chip, C> = Rc<Halo2Loader<C, DeferredScalarEccChip<'chip, C>>>;
    type DeferredLoadedScalar<'chip, C> =
        snark_verifier::loader::halo2::Scalar<C, DeferredScalarEccChip<'chip, C>>;
    pub(super) type DeferredAccumulator<'chip, C> = IpaAccumulator<C, DeferredLoader<'chip, C>>;
    type DeferredTranscript<'chip, C, R> = PoseidonTranscript<
        C,
        DeferredLoader<'chip, C>,
        R,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;

    /// Native values transported to the reciprocal point half after the
    /// scalar half has witness-loaded and identity-bound one parent protocol.
    #[derive(Clone, Debug)]
    pub(super) struct DeferredProtocolIdentityWitness<C>
    where
        C: CurveAffineExt,
    {
        /// Exact fixed protocol-structure digest embedded by the circuit.
        pub(super) structure_sha256: [u8; 32],
        /// Protocol parity/domain tag.
        pub(super) parity: KagemushaPastaCycleParityV1,
        /// Self-referential VK commitments, in compiled-protocol order.
        pub(super) preprocessed: Vec<C>,
        /// Exact verifier-key transcript initial state.
        pub(super) transcript_initial_state: C::ScalarExt,
    }

    /// One witness-loaded compiled protocol whose dynamic values have already
    /// been constrained to the release identity public input.
    pub(super) struct LoadedParentProtocolV1<'chip, C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        pub(super) protocol: PlonkProtocol<C, DeferredLoader<'chip, C>>,
        pub(super) identity_witness: DeferredProtocolIdentityWitness<C>,
    }

    /// One parent-instance copy binding used by the fixed-shape V4 verifier.
    ///
    /// The binding itself carries no host presence flag.  Its equality is
    /// gated exclusively by the already-constrained current-Step slot bit
    /// passed to [`constrain_parent_scalar_lineage_v4`].
    pub(super) struct ParentInstanceCopyBindingV4<'a, F>
    where
        F: ff::Field,
    {
        /// Parent proof instance column.
        pub(super) column: usize,
        /// Exact source range in that parent column.
        pub(super) source: Range<usize>,
        /// Current Step cells receiving the conditional copy constraint.
        pub(super) expected: &'a [AssignedValue<F>],
    }

    /// One parent ordinary proof together with the external fold that completed
    /// that parent's lineage after its outer proof was created.
    pub(super) struct ParentScalarLineageWitnessV4<'a, C>
    where
        C: CurveAffineExt,
    {
        /// Exact parent public instances, real or authenticated bootstrap.
        pub(super) instances: &'a [Vec<C::ScalarExt>],
        /// Exact ordinary parent transcript.
        pub(super) proof_bytes: &'a [u8],
        /// Always-present, non-identity carried accumulator.
        pub(super) carried_lineage: &'a IpaAccumulator<C, NativeLoader>,
        /// Instance column containing the dynamic accumulator vector.
        pub(super) carried_lineage_instance_column: usize,
        /// Exact degree-derived carried-accumulator range.
        pub(super) carried_lineage_instance_range: Range<usize>,
        /// Parent slices rebound to the current Step public boundary.
        pub(super) instance_copy_bindings: &'a [ParentInstanceCopyBindingV4<'a, C::ScalarExt>],
        /// Always-present degree-specific post-proof BGH19 transcript.
        pub(super) external_accumulation_proof: &'a KagemushaIpaAccumulationProofV4,
    }

    /// Semantic reason an exact range of deferred curve equations is enabled.
    ///
    /// The enum, rather than a caller-provided Boolean vector, is retained in
    /// the fixed audit shape.  The scalar half derives its assigned selector
    /// from verified parent instances; the reciprocal half derives the same
    /// selector from the cross-bound parent-count witnesses and public slot
    /// bits.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum DeferredEquationGateV4 {
        /// Ordinary succinct verification for one parent slot.
        ParentCurrent { slot: usize },
        /// Parent-current plus carried-lineage BGH19 fold.
        ParentCarriedFold { slot: usize },
        /// Selection of the parent's current or folded lineage.
        ParentLineageSelect { slot: usize },
        /// Two-parent branch BGH19 fold.
        BranchFold,
        /// Selection of parent zero or the two-parent branch fold.
        BranchSelect,
    }

    impl DeferredEquationGateV4 {
        /// Stable tag committed in the recursive deferred-audit preimage.
        pub(super) fn audit_tag(self) -> u32 {
            match self {
                Self::ParentCurrent { slot: 0 } => 1,
                Self::ParentCurrent { slot: 1 } => 2,
                Self::ParentCarriedFold { slot: 0 } => 3,
                Self::ParentCarriedFold { slot: 1 } => 4,
                Self::ParentLineageSelect { slot: 0 } => 5,
                Self::ParentLineageSelect { slot: 1 } => 6,
                Self::BranchFold => 7,
                Self::BranchSelect => 8,
                Self::ParentCurrent { .. }
                | Self::ParentCarriedFold { .. }
                | Self::ParentLineageSelect { .. } => {
                    unreachable!("validated V4 parent slot is zero or one")
                }
            }
        }
    }

    /// One contiguous, fixed-shape range of deferred equations and its
    /// in-circuit scalar selector.
    #[derive(Clone, Debug)]
    pub(super) struct AssignedDeferredEquationStageV4<F>
    where
        F: ff::Field,
    {
        pub(super) range: Range<usize>,
        pub(super) gate: DeferredEquationGateV4,
        pub(super) enabled: AssignedValue<F>,
    }

    /// Field-independent compiled shape of one deferred-equation stage.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct DeferredEquationStageShapeV4 {
        pub(super) range: Range<usize>,
        pub(super) gate: DeferredEquationGateV4,
    }

    impl<F> AssignedDeferredEquationStageV4<F>
    where
        F: ff::Field,
    {
        pub(super) fn shape(&self) -> DeferredEquationStageShapeV4 {
            DeferredEquationStageShapeV4 {
                range: self.range.clone(),
                gate: self.gate,
            }
        }
    }

    /// Complete selected lineage for one parent slot.
    pub(super) struct ParentScalarLineageV4<'chip, C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        pub(super) accumulator: DeferredAccumulator<'chip, C>,
        pub(super) stages: Vec<AssignedDeferredEquationStageV4<C::ScalarExt>>,
    }

    /// Unconditionally-computed two-parent branch candidate and its fixed
    /// deferred-equation stages.
    pub(super) struct ExposedParentLineageV4<C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        pub(super) stages: Vec<AssignedDeferredEquationStageV4<C::ScalarExt>>,
    }

    /// Require the complete post-branch V4 stage order for the shared public
    /// deferred audit.
    ///
    /// Both V4 slots bind the same complete audit. Slot presence
    /// only controls public exposure of that digest.  This is required for a
    /// one-parent step: its enabled `BranchSelect` equation is created after
    /// parent zero and therefore must be covered by slot zero's non-zero join.
    pub(super) fn validate_stage_shapes_v4(
        stages: &[DeferredEquationStageShapeV4],
        equation_count: usize,
    ) -> Result<(), Error> {
        const COMPLETE: [DeferredEquationGateV4; 8] = [
            DeferredEquationGateV4::ParentCurrent { slot: 0 },
            DeferredEquationGateV4::ParentCarriedFold { slot: 0 },
            DeferredEquationGateV4::ParentLineageSelect { slot: 0 },
            DeferredEquationGateV4::ParentCurrent { slot: 1 },
            DeferredEquationGateV4::ParentCarriedFold { slot: 1 },
            DeferredEquationGateV4::ParentLineageSelect { slot: 1 },
            DeferredEquationGateV4::BranchFold,
            DeferredEquationGateV4::BranchSelect,
        ];
        if stages.len() != COMPLETE.len()
            || stages
                .iter()
                .zip(COMPLETE)
                .any(|(stage, expected)| stage.gate != expected)
        {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 deferred stages do not have the complete post-branch order"
                    .to_owned(),
            ));
        }
        let mut cursor = 0;
        for stage in stages {
            if stage.range.start != cursor
                || stage.range.start >= stage.range.end
                || stage.range.end > equation_count
            {
                return Err(Error::AssertionFailure(
                    "Kagemusha V4 deferred stages are not a contiguous audit partition".to_owned(),
                ));
            }
            cursor = stage.range.end;
        }
        if cursor != equation_count {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 deferred stages do not cover the complete post-branch audit"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn expand_stage_plan_v4<F>(
        stages: &[AssignedDeferredEquationStageV4<F>],
        equation_count: usize,
    ) -> Result<(Vec<u32>, Vec<AssignedValue<F>>), Error>
    where
        F: ff::Field,
    {
        let shapes = stages
            .iter()
            .map(AssignedDeferredEquationStageV4::shape)
            .collect::<Vec<_>>();
        validate_stage_shapes_v4(&shapes, equation_count)?;
        let mut gate_tags = Vec::with_capacity(equation_count);
        let mut selectors = Vec::with_capacity(equation_count);
        for stage in stages {
            gate_tags.extend(std::iter::repeat_n(
                stage.gate.audit_tag(),
                stage.range.len(),
            ));
            selectors.extend(std::iter::repeat_n(stage.enabled, stage.range.len()));
        }
        Ok((gate_tags, selectors))
    }

    /// A `Read` implementation whose position remains observable after the
    /// transcript borrows it, allowing every parser to reject trailing bytes.
    #[derive(Clone, Debug)]
    struct ExactReader<'a> {
        bytes: &'a [u8],
        position: Rc<Cell<usize>>,
    }

    impl<'a> ExactReader<'a> {
        fn new(bytes: &'a [u8]) -> (Self, Rc<Cell<usize>>) {
            let position = Rc::new(Cell::new(0));
            (
                Self {
                    bytes,
                    position: Rc::clone(&position),
                },
                position,
            )
        }
    }

    impl Read for ExactReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let start = self.position.get();
            let available = &self.bytes[start..];
            let len = available.len().min(output.len());
            output[..len].copy_from_slice(&available[..len]);
            self.position.set(start + len);
            Ok(len)
        }
    }

    fn transcript_error(message: impl Into<String>) -> Error {
        Error::Transcript(io::ErrorKind::InvalidData, message.into())
    }

    fn push_constant_bytes<F: BigPrimeField>(
        output: &mut Vec<KagemushaSha256ByteV4<F>>,
        bytes: &[u8],
    ) {
        output.extend(bytes.iter().copied().map(KagemushaSha256ByteV4::constant));
    }

    /// Witness-load the only self-referential protocol values and bind their
    /// exact canonical identity to the release-authenticated public words.
    ///
    /// `fixed_structure_sha256` is part of the outer circuit relation.  It is
    /// checked against the native compiled protocol before assignment and then
    /// loaded as constants.  The final VK may therefore be compiled only after
    /// key generation without ever becoming a constant of its own circuit.
    pub(super) fn load_and_constrain_parent_protocol<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        sha_jobs: &mut KagemushaSha256JobsV4<C::ScalarExt>,
        protocol: &PlonkProtocol<C>,
        parity: KagemushaPastaCycleParityV1,
        fixed_structure_sha256: [u8; 32],
        expected_words: &[AssignedValue<C::ScalarExt>],
    ) -> Result<LoadedParentProtocolV1<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if expected_words.len() != 2
            || protocol.preprocessed.is_empty()
            || protocol
                .preprocessed
                .iter()
                .any(|point| bool::from(point.is_identity()))
        {
            return Err(Error::InvalidInstances);
        }
        let actual_structure = kagemusha_compiled_protocol_structure_sha256(protocol, parity)
            .map_err(transcript_error)?;
        if actual_structure != fixed_structure_sha256 {
            return Err(transcript_error(
                "Kagemusha compiled parent protocol structure mismatch",
            ));
        }
        let transcript_initial_state = protocol.transcript_initial_state.ok_or_else(|| {
            transcript_error("Kagemusha compiled parent protocol has no transcript state")
        })?;
        let loaded = protocol.loaded_preprocessed_as_witness(loader, false);

        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let mut bytes = Vec::new();
        push_constant_bytes(&mut bytes, KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1);
        push_constant_bytes(&mut bytes, &[0]);
        push_constant_bytes(
            &mut bytes,
            &KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes(),
        );
        push_constant_bytes(&mut bytes, &protocol_parity_tag(parity).to_le_bytes());
        push_constant_bytes(&mut bytes, &fixed_structure_sha256);
        push_constant_bytes(
            &mut bytes,
            &u32::try_from(loaded.preprocessed.len())
                .map_err(|_| transcript_error("Kagemusha preprocessed count does not fit u32"))?
                .to_le_bytes(),
        );
        for point in &loaded.preprocessed {
            bytes.extend(chip.assigned_point_bytes(&mut ctx, &point.assigned())?);
        }
        let loaded_transcript_state =
            loaded.transcript_initial_state.as_ref().ok_or_else(|| {
                transcript_error("loaded Kagemusha parent protocol has no transcript state")
            })?;
        bytes.extend(chip.assigned_scalar_bytes(&mut ctx, *loaded_transcript_state.assigned()));
        let digest = sha_jobs
            .digest_constrained(ctx.main(), &bytes)
            .map_err(transcript_error)?;
        for (assigned, expected) in digest.chunks_exact(4).zip(expected_words) {
            let packed = super::pack_assigned_u32_words_v5(ctx.main(), chip.range(), assigned);
            ctx.main().constrain_equal(&packed, expected);
        }
        drop(ctx);

        Ok(LoadedParentProtocolV1 {
            protocol: loaded,
            identity_witness: DeferredProtocolIdentityWitness {
                structure_sha256: fixed_structure_sha256,
                parity,
                preprocessed: protocol.preprocessed.clone(),
                transcript_initial_state,
            },
        })
    }

    fn load_native_accumulator<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        accumulator: &IpaAccumulator<C, NativeLoader>,
    ) -> DeferredAccumulator<'chip, C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        IpaAccumulator::new(
            accumulator
                .xi
                .iter()
                .map(|challenge| loader.assign_scalar(*challenge))
                .collect(),
            loader.assign_ec_point(accumulator.u),
        )
    }

    fn assigned_instance_cells_v4<C>(
        column: &[DeferredLoadedScalar<'_, C>],
        range: Range<usize>,
        expected_len: usize,
    ) -> Result<Vec<AssignedValue<C::ScalarExt>>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if expected_len == 0 || range.len() != expected_len || range.end > column.len() {
            return Err(Error::InvalidInstances);
        }
        Ok(column[range]
            .iter()
            .map(|scalar| *scalar.assigned())
            .collect())
    }

    fn constrain_equal_when<F>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        enabled: AssignedValue<F>,
        lhs: AssignedValue<F>,
        rhs: AssignedValue<F>,
    ) where
        F: BigPrimeField,
    {
        range.gate().assert_bit(ctx, enabled);
        let difference = range.gate().sub(ctx, Existing(lhs), Existing(rhs));
        let selected = range
            .gate()
            .mul(ctx, Existing(enabled), Existing(difference));
        range.gate().assert_is_const(ctx, &selected, &F::ZERO);
    }

    fn selector_and<F>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        lhs: AssignedValue<F>,
        rhs: AssignedValue<F>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        range.gate().assert_bit(ctx, lhs);
        range.gate().assert_bit(ctx, rhs);
        let output = range.gate().mul(ctx, Existing(lhs), Existing(rhs));
        range.gate().assert_bit(ctx, output);
        output
    }

    fn derive_parent_count_and_presence<C>(
        loader: &DeferredLoader<'_, C>,
        loaded_instances: &[Vec<DeferredLoadedScalar<'_, C>>],
    ) -> Result<(AssignedValue<C::ScalarExt>, AssignedValue<C::ScalarExt>), Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        let parent_count = loaded_instances
            .first()
            .and_then(|column| column.get(KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5))
            .map(|value| *value.assigned())
            .ok_or(Error::InvalidInstances)?;
        let chip = loader.ecc_chip();
        let range = chip.range();
        let mut ctx = loader.ctx_mut();
        range.range_check(ctx.main(), parent_count, 2);
        let is_three =
            range
                .gate()
                .is_equal(ctx.main(), parent_count, Constant(C::ScalarExt::from(3)));
        range
            .gate()
            .assert_is_const(ctx.main(), &is_three, &C::ScalarExt::ZERO);
        let is_zero = range.gate().is_zero(ctx.main(), parent_count);
        let has_parent = range.gate().not(ctx.main(), is_zero);
        range.gate().assert_bit(ctx.main(), has_parent);
        Ok((parent_count, has_parent))
    }

    /// Derive the exact two public slot-presence bits from the current Step's
    /// constrained parent-count cell.
    pub(super) fn constrain_parent_slot_selectors_v4<C>(
        loader: &DeferredLoader<'_, C>,
        parent_count: AssignedValue<C::ScalarExt>,
    ) -> [AssignedValue<C::ScalarExt>; 2]
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        let chip = loader.ecc_chip();
        let range = chip.range();
        let mut ctx = loader.ctx_mut();
        range.range_check(ctx.main(), parent_count, 2);
        let is_three =
            range
                .gate()
                .is_equal(ctx.main(), parent_count, Constant(C::ScalarExt::from(3)));
        range
            .gate()
            .assert_is_const(ctx.main(), &is_three, &C::ScalarExt::ZERO);
        let is_zero = range.gate().is_zero(ctx.main(), parent_count);
        let present_zero = range.gate().not(ctx.main(), is_zero);
        let present_one =
            range
                .gate()
                .is_equal(ctx.main(), parent_count, Constant(C::ScalarExt::from(2)));
        range.gate().assert_bit(ctx.main(), present_zero);
        range.gate().assert_bit(ctx.main(), present_one);
        [present_zero, present_one]
    }

    fn select_accumulator<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        when_true: &DeferredAccumulator<'chip, C>,
        when_false: &DeferredAccumulator<'chip, C>,
        selector: AssignedValue<C::ScalarExt>,
    ) -> Result<DeferredAccumulator<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if when_true.xi.len() != when_false.xi.len() {
            return Err(Error::AssertionFailure(
                "Kagemusha accumulator selector received different round counts".to_owned(),
            ));
        }
        let selected_xi = when_true
            .xi
            .iter()
            .zip(&when_false.xi)
            .map(|(when_true, when_false)| {
                let when_true = *when_true.assigned();
                let when_false = *when_false.assigned();
                let selected = {
                    let chip = loader.ecc_chip();
                    let range = chip.range();
                    range
                        .gate()
                        .select(loader.ctx_mut().main(), when_true, when_false, selector)
                };
                loader.scalar_from_assigned(selected)
            })
            .collect();
        let when_true = when_true.u.assigned().clone();
        let when_false = when_false.u.assigned().clone();
        let selected_u = {
            let chip = loader.ecc_chip();
            chip.select_point(&mut loader.ctx_mut(), &when_true, &when_false, selector)
        };
        Ok(IpaAccumulator::new(
            selected_xi,
            loader.ec_point_from_assigned(selected_u),
        ))
    }

    fn record_stage<C>(
        loader: &DeferredLoader<'_, C>,
        start: usize,
        gate: DeferredEquationGateV4,
        enabled: AssignedValue<C::ScalarExt>,
        stages: &mut Vec<AssignedDeferredEquationStageV4<C::ScalarExt>>,
    ) -> Result<(), Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        let end = loader.ecc_chip().equation_count();
        if start >= end {
            return Err(Error::AssertionFailure(
                "Kagemusha fixed deferred-equation stage is empty".to_owned(),
            ));
        }
        stages.push(AssignedDeferredEquationStageV4 {
            range: start..end,
            gate,
            enabled,
        });
        Ok(())
    }

    fn verify_ordinary_parent<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
        instances: &[Vec<DeferredLoadedScalar<'chip, C>>],
        proof_bytes: &[u8],
        max_proof_bytes: usize,
    ) -> Result<DeferredAccumulator<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if max_proof_bytes == 0 || proof_bytes.is_empty() || proof_bytes.len() > max_proof_bytes {
            return Err(transcript_error(
                "Kagemusha parent proof violates the fixed proof slot",
            ));
        }
        let (reader, position) = ExactReader::new(proof_bytes);
        let mut transcript =
            DeferredTranscript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(loader, reader);
        let parsed = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::read_proof(
            succinct_vk,
            protocol,
            instances,
            &mut transcript,
        )?;
        let mut accumulators = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::verify(
            succinct_vk,
            protocol,
            instances,
            &parsed,
        )?;
        if position.get() != proof_bytes.len() {
            return Err(transcript_error(
                "Kagemusha parent proof has trailing bytes",
            ));
        }
        if accumulators.len() != 1 {
            return Err(Error::AssertionFailure(
                "Kagemusha fixed parent verifier did not emit one IPA accumulator".to_owned(),
            ));
        }
        Ok(accumulators.remove(0))
    }

    fn verify_fold<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        inputs: &[DeferredAccumulator<'chip, C>],
        proof_bytes: &[u8],
        expected_proof_bytes: usize,
    ) -> Result<DeferredAccumulator<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if inputs.len() < 2
            || expected_proof_bytes == 0
            || proof_bytes.len() != expected_proof_bytes
        {
            return Err(transcript_error(
                "Kagemusha BGH19 fold has the wrong input or byte count",
            ));
        }
        let (reader, position) = ExactReader::new(proof_bytes);
        let mut transcript =
            DeferredTranscript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(loader, reader);
        let parsed =
            <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::read_proof(
                succinct_vk,
                inputs,
                &mut transcript,
            )?;
        let accumulated =
            <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::verify(
                succinct_vk,
                inputs,
                &parsed,
            )?;
        if position.get() != proof_bytes.len() {
            return Err(transcript_error("Kagemusha BGH19 fold has trailing bytes"));
        }
        Ok(accumulated)
    }

    /// Verify one V4 parent slot with degree-derived accumulator and transcript
    /// lengths.  All three stages execute even when the public slot selector is
    /// zero; authenticated bootstrap material must therefore be fully parseable.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn constrain_parent_scalar_lineage_v4<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        protocol: &LoadedParentProtocolV1<'chip, C>,
        parent_slot: usize,
        slot_enabled: AssignedValue<C::ScalarExt>,
        authenticated_round_count: u32,
        max_parent_proof_bytes: usize,
        accumulator_instance_limbs: usize,
        witness: ParentScalarLineageWitnessV4<'_, C>,
    ) -> Result<ParentScalarLineageV4<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if parent_slot >= 2 || max_parent_proof_bytes == 0 || accumulator_instance_limbs == 0 {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 parent slot/configuration is invalid".to_owned(),
            ));
        }
        witness
            .external_accumulation_proof
            .validate_fixed_transcript(authenticated_round_count)
            .map_err(|error| transcript_error(error))?;
        let expected_fold_bytes =
            kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)
                .map_err(|error| transcript_error(error))?;
        {
            let chip = loader.ecc_chip();
            chip.range()
                .gate()
                .assert_bit(loader.ctx_mut().main(), slot_enabled);
        }
        let loaded_instances = witness
            .instances
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| loader.assign_scalar(*value))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let parent_live_selector = loaded_instances
            .first()
            .and_then(|column| column.last())
            .ok_or(Error::InvalidInstances)?;
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&parent_live_selector.assigned(), &slot_enabled);
        let (_parent_count, has_carried_lineage) =
            derive_parent_count_and_presence(loader, &loaded_instances)?;

        let carried_column = loaded_instances
            .get(witness.carried_lineage_instance_column)
            .ok_or(Error::InvalidInstances)?;
        let expected_carried = assigned_instance_cells_v4(
            carried_column,
            witness.carried_lineage_instance_range,
            accumulator_instance_limbs,
        )?;
        for binding in witness.instance_copy_bindings {
            let column = loaded_instances
                .get(binding.column)
                .ok_or(Error::InvalidInstances)?;
            if binding.source.len() != binding.expected.len() || binding.source.end > column.len() {
                return Err(Error::InvalidInstances);
            }
            let parent_cells = column[binding.source.clone()]
                .iter()
                .map(|scalar| *scalar.assigned())
                .collect::<Vec<_>>();
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            for (parent, expected) in parent_cells.iter().zip(binding.expected) {
                constrain_equal_when(ctx.main(), range, slot_enabled, *parent, *expected);
            }
        }

        let carried = load_native_accumulator(loader, witness.carried_lineage);
        let carried_challenges = carried
            .xi
            .iter()
            .map(|challenge| *challenge.assigned())
            .collect::<Vec<_>>();
        let carried_point = carried.u.assigned().clone();
        let assigned_carried = {
            let chip = loader.ecc_chip();
            chip.assigned_accumulator_instance_limbs_v4(
                &mut loader.ctx_mut(),
                authenticated_round_count,
                &carried_challenges,
                &carried_point,
            )?
        };
        if assigned_carried.len() != accumulator_instance_limbs {
            return Err(Error::InvalidInstances);
        }
        {
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            let zero = ctx.main().load_zero();
            for (actual, expected) in assigned_carried.iter().zip(&expected_carried) {
                let selected = range
                    .gate()
                    .select(ctx.main(), *actual, zero, has_carried_lineage);
                constrain_equal_when(ctx.main(), range, slot_enabled, selected, *expected);
            }
        }

        let mut stages = Vec::with_capacity(3);
        let current_start = loader.ecc_chip().equation_count();
        let current = verify_ordinary_parent(
            loader,
            succinct_vk,
            &protocol.protocol,
            &loaded_instances,
            witness.proof_bytes,
            max_parent_proof_bytes,
        )?;
        if usize::try_from(authenticated_round_count).ok() != Some(current.xi.len()) {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 ordinary proof emitted the wrong IPA round count".to_owned(),
            ));
        }
        record_stage(
            loader,
            current_start,
            DeferredEquationGateV4::ParentCurrent { slot: parent_slot },
            slot_enabled,
            &mut stages,
        )?;

        let fold_enabled = {
            let chip = loader.ecc_chip();
            selector_and(
                loader.ctx_mut().main(),
                chip.range(),
                slot_enabled,
                has_carried_lineage,
            )
        };
        let fold_start = loader.ecc_chip().equation_count();
        let folded = verify_fold(
            loader,
            succinct_vk,
            &[current.clone(), carried],
            &witness.external_accumulation_proof.bytes,
            expected_fold_bytes,
        )?;
        record_stage(
            loader,
            fold_start,
            DeferredEquationGateV4::ParentCarriedFold { slot: parent_slot },
            fold_enabled,
            &mut stages,
        )?;

        let select_start = loader.ecc_chip().equation_count();
        let accumulator = select_accumulator(loader, &folded, &current, has_carried_lineage)?;
        record_stage(
            loader,
            select_start,
            DeferredEquationGateV4::ParentLineageSelect { slot: parent_slot },
            slot_enabled,
            &mut stages,
        )?;
        Ok(ParentScalarLineageV4 {
            accumulator,
            stages,
        })
    }

    /// Degree-parameterized V4 branch fold and public-lineage selection.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn constrain_exposed_parent_lineage_v4<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        authenticated_round_count: u32,
        accumulator_instance_limbs: usize,
        parent_zero: &DeferredAccumulator<'chip, C>,
        parent_one: &DeferredAccumulator<'chip, C>,
        slot_present: [AssignedValue<C::ScalarExt>; 2],
        branch_merge_proof: &KagemushaIpaAccumulationProofV4,
        exposed_instance_limbs: &[AssignedValue<C::ScalarExt>],
    ) -> Result<ExposedParentLineageV4<C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        branch_merge_proof
            .validate_fixed_transcript(authenticated_round_count)
            .map_err(|error| transcript_error(error))?;
        let expected_fold_bytes =
            kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)
                .map_err(|error| transcript_error(error))?;
        if accumulator_instance_limbs == 0
            || exposed_instance_limbs.len() != accumulator_instance_limbs
        {
            return Err(Error::InvalidInstances);
        }
        {
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            range.gate().assert_bit(ctx.main(), slot_present[0]);
            range.gate().assert_bit(ctx.main(), slot_present[1]);
            let absent_zero = range.gate().not(ctx.main(), slot_present[0]);
            let invalid_second =
                range
                    .gate()
                    .mul(ctx.main(), Existing(slot_present[1]), Existing(absent_zero));
            range
                .gate()
                .assert_is_const(ctx.main(), &invalid_second, &C::ScalarExt::ZERO);
        }

        let branch_start = loader.ecc_chip().equation_count();
        let branch = verify_fold(
            loader,
            succinct_vk,
            &[parent_zero.clone(), parent_one.clone()],
            &branch_merge_proof.bytes,
            expected_fold_bytes,
        )?;
        let mut stages = Vec::with_capacity(2);
        record_stage(
            loader,
            branch_start,
            DeferredEquationGateV4::BranchFold,
            slot_present[1],
            &mut stages,
        )?;

        let select_start = loader.ecc_chip().equation_count();
        let selected = select_accumulator(loader, &branch, parent_zero, slot_present[1])?;
        record_stage(
            loader,
            select_start,
            DeferredEquationGateV4::BranchSelect,
            slot_present[0],
            &mut stages,
        )?;

        let selected_challenges = selected
            .xi
            .iter()
            .map(|challenge| *challenge.assigned())
            .collect::<Vec<_>>();
        let selected_point = selected.u.assigned().clone();
        let selected_limbs = {
            let chip = loader.ecc_chip();
            chip.assigned_accumulator_instance_limbs_v4(
                &mut loader.ctx_mut(),
                authenticated_round_count,
                &selected_challenges,
                &selected_point,
            )?
        };
        if selected_limbs.len() != accumulator_instance_limbs {
            return Err(Error::InvalidInstances);
        }
        {
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            let zero = ctx.main().load_zero();
            for (actual, expected) in selected_limbs.iter().zip(exposed_instance_limbs) {
                let exposed = range
                    .gate()
                    .select(ctx.main(), *actual, zero, slot_present[0]);
                ctx.main().constrain_equal(&exposed, expected);
            }
        }
        Ok(ExposedParentLineageV4 { stages })
    }

    /// Hash the complete selector-bound V5 audit once and expose it through
    /// both independently presence-gated public join slots.
    ///
    /// Both public slots receive the same complete post-branch preimage.  For
    /// a one-parent step slot zero is present and therefore binds every
    /// enabled equation, including `BranchSelect`; slot one remains canonical
    /// zero.  A two-parent step exposes the same complete digest in both slots.
    pub(super) fn constrain_scalar_audit_identity_v4<C>(
        loader: &DeferredLoader<'_, C>,
        sha_jobs: &mut KagemushaSha256JobsV4<C::ScalarExt>,
        range: &halo2_base::gates::RangeChip<C::ScalarExt>,
        stages: &[AssignedDeferredEquationStageV4<C::ScalarExt>],
        slot_present: [AssignedValue<C::ScalarExt>; 2],
        expected_words: [&[AssignedValue<C::ScalarExt>]; 2],
    ) -> Result<[AssignedValue<C::ScalarExt>; 8], Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if expected_words.iter().any(|words| words.len() != 2) {
            return Err(Error::InvalidInstances);
        }
        let chip = loader.ecc_chip();
        let (gate_tags, selectors) = expand_stage_plan_v4(stages, chip.equation_count())?;
        let mut ctx = loader.ctx_mut();
        for selector in slot_present.iter().copied() {
            range.gate().assert_bit(ctx.main(), selector);
        }
        let bytes = chip.assigned_equation_bytes_v5(&mut ctx, &gate_tags, &selectors)?;
        let digest = sha_jobs
            .digest_constrained(ctx.main(), &bytes)
            .map_err(transcript_error)?;
        for (present, expected_words) in slot_present.into_iter().zip(expected_words) {
            for (assigned, expected) in digest.chunks_exact(4).zip(expected_words) {
                let packed = super::pack_assigned_u32_words_v5(ctx.main(), range, assigned);
                let exposed = range
                    .gate()
                    .mul(ctx.main(), Existing(present), Existing(packed));
                ctx.main().constrain_equal(&exposed, expected);
            }
        }
        Ok(digest)
    }
}

/// One real-or-bootstrap fixed parent slot consumed by a V4 parity circuit.
#[derive(Clone)]
pub(crate) struct KagemushaStepParentProofV4<C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    /// Exact one-column parent instances.
    pub(crate) instances: Vec<Vec<C::ScalarExt>>,
    /// Ordinary augmented parent proof transcript.
    pub(crate) proof_bytes: Vec<u8>,
    /// Always-present non-identity carried accumulator.
    pub(crate) carried_lineage:
        snark_verifier::pcs::ipa::IpaAccumulator<C, snark_verifier::loader::native::NativeLoader>,
    /// Always-present post-proof fold transcript.
    pub(crate) external_accumulation_proof: KagemushaIpaAccumulationProofV4,
}

/// Complete fixed two-parent/three-fold recursive witness for one V4 parity.
pub(crate) struct KagemushaStepParityRecursionV4<C>
where
    C: halo2_base::utils::CurveAffineExt,
{
    /// Canonical IPA succinct key derived from the authenticated ParamsIPA.
    pub(crate) succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<C>,
    /// Final compiled self protocol derived from authenticated ParamsIPA/VK.
    pub(crate) compiled_parent_protocol: PlonkProtocol<C>,
    /// Authenticated value-free self-protocol structure digest.
    pub(crate) fixed_structure_sha256: [u8; 32],
    /// Exactly two real-or-bootstrap ordinary proofs and post-proof folds.
    pub(crate) parents: [KagemushaStepParentProofV4<C>; 2],
    /// Per-step branch fold.  This is distinct from the all-bootstrap genesis
    /// artifact whenever either parent slot is real.
    pub(crate) branch_merge_fold: KagemushaIpaAccumulationProofV4,
}

/// Complete concrete witness needed to build both V4 Step parities.
pub(crate) struct KagemushaStepWitnessV4<'a> {
    /// Common field-neutral public boundary.
    pub(crate) public_inputs: &'a KagemushaPastaCyclePublicInputsV4,
    /// Exact logical recursive step counter.
    pub(crate) proof_step_count: u32,
    /// All fixed Eq-only secure relation openings.
    pub(crate) secure: &'a super::confidential_v2::KagemushaStepSecureWitnessV3,
    /// Eq-only output insertion/membership witness.
    pub(crate) output_membership: &'a super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    /// Same-scalar Eq recursion witness.
    pub(crate) step_eq_recursion:
        &'a KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    /// Same-scalar Ep recursion witness.
    pub(crate) step_ep_recursion:
        &'a KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    /// Authenticated canonical Eq bootstrap payload; absence is an error.
    pub(crate) step_eq_bootstrap: Option<&'a KagemushaStepBootstrapV4>,
    /// Authenticated canonical Ep bootstrap payload; absence is an error.
    pub(crate) step_ep_bootstrap: Option<&'a KagemushaStepBootstrapV4>,
}

struct KagemushaScalarAuditOutputV4<C>
where
    C: halo2_base::utils::CurveAffineExt,
{
    identity: scalar_lineage_v1::DeferredProtocolIdentityWitness<C>,
    audit: super::kagemusha_cycle_loader::DeferredEquationWitness<C>,
    stages: Vec<scalar_lineage_v1::DeferredEquationStageShapeV4>,
    inner_parent_counts: [u32; 2],
}

/// Serialize one scalar-verifier audit exactly as both constrained halves do
/// and derive its selector-gated public SHA-256 words.
fn kagemusha_deferred_audit_public_words_v5<C>(
    witness: &super::kagemusha_cycle_loader::DeferredEquationWitness<C>,
    stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
    current_parent_count: u32,
    inner_parent_counts: [u32; 2],
) -> Result<[[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1], String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: PrimeField,
    C::ScalarExt: PrimeField,
{
    use super::kagemusha_cycle_loader::{
        KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V5, KAGEMUSHA_DEFERRED_AUDIT_VERSION_V5,
    };

    if current_parent_count > 2 || inner_parent_counts.into_iter().any(|count| count > 2) {
        return Err("Kagemusha V5 deferred-audit parent count is invalid".to_owned());
    }
    scalar_lineage_v1::validate_stage_shapes_v4(stages, witness.equations.len())
        .map_err(|error| format!("invalid Kagemusha V5 deferred-audit stage plan: {error:?}"))?;

    let slot_present = [current_parent_count >= 1, current_parent_count == 2];
    let parent_has_carried = inner_parent_counts.map(|count| count != 0);
    let mut gate_tags = vec![0_u32; witness.equations.len()];
    let mut selectors = vec![false; witness.equations.len()];
    for stage in stages {
        let enabled = match stage.gate {
            scalar_lineage_v1::DeferredEquationGateV4::ParentCurrent { slot }
            | scalar_lineage_v1::DeferredEquationGateV4::ParentLineageSelect { slot } => {
                slot_present[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::ParentCarriedFold { slot } => {
                slot_present[slot] && parent_has_carried[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::BranchFold => slot_present[1],
            scalar_lineage_v1::DeferredEquationGateV4::BranchSelect => slot_present[0],
        };
        for equation in stage.range.clone() {
            gate_tags[equation] = stage.gate.audit_tag();
            selectors[equation] = enabled;
        }
    }

    fn push_len(output: &mut Sha256, value: usize, label: &str) -> Result<(), String> {
        let value = u32::try_from(value)
            .map_err(|_| format!("Kagemusha V5 deferred-audit {label} does not fit u32"))?;
        output.update(value.to_le_bytes());
        Ok(())
    }

    fn push_field<F: PrimeField>(
        output: &mut Sha256,
        value: &F,
        label: &str,
    ) -> Result<(), String> {
        let repr = value.to_repr();
        if repr.as_ref().len() != 32 {
            return Err(format!(
                "Kagemusha V5 deferred-audit {label} is not a 32-byte Pasta scalar"
            ));
        }
        output.update(repr.as_ref());
        Ok(())
    }

    let mut digest = Sha256::new();
    digest.update(KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V5);
    digest.update([0]);
    digest.update(KAGEMUSHA_DEFERRED_AUDIT_VERSION_V5.to_le_bytes());
    push_len(&mut digest, witness.sources.len(), "source count")?;
    push_len(&mut digest, witness.equations.len(), "equation count")?;
    for source in &witness.sources {
        let coordinates: Option<snark_verifier::util::arithmetic::Coordinates<C>> =
            source.coordinates().into();
        let coordinates = coordinates
            .ok_or_else(|| "Kagemusha V5 deferred-audit source is the identity point".to_owned())?;
        push_field(&mut digest, coordinates.x(), "source x-coordinate")?;
        push_field(&mut digest, coordinates.y(), "source y-coordinate")?;
    }
    for (index, equation) in witness.equations.iter().enumerate() {
        digest.update(gate_tags[index].to_le_bytes());
        digest.update([u8::from(selectors[index])]);
        push_len(&mut digest, equation.len(), "term count")?;
        for (source_index, coefficient) in equation {
            push_len(&mut digest, *source_index, "source index")?;
            push_field(&mut digest, coefficient, "coefficient")?;
        }
    }
    let public_words = kagemusha_sha256_public_words(digest.finalize().into());
    Ok(slot_present.map(|present| if present { public_words } else { [0; 8] }))
}

/// Execute the scalar-verifier witness pass with blank derived-audit words.
/// The resulting native audit is then serialized above and installed as
/// the public join before the proving pass builds both complete circuits.
fn collect_kagemusha_scalar_audits_v4<C>(
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    params: &KagemushaStepCircuitParamsV4,
    recursion: &KagemushaStepParityRecursionV4<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<KagemushaScalarAuditOutputV4<C>, String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt:
        halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField + PrimeField + From<u64>,
{
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;

    let layout = public_inputs.validate_for_audit_derivation_prepass(proof_step_count, params)?;
    // This native audit prepass is never synthesized. Witness-only mode keeps
    // values needed for the reciprocal join without retaining selectors, copy
    // constraints, or fixed-column bookkeeping.
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(true)
        .use_params(kagemusha_base_circuit_params_v4(params)?);
    let values = public_inputs.instance_column_for_audit_derivation_prepass::<C::ScalarExt>(
        proof_step_count,
        params,
        parity,
    )?;
    let public_cells = builder.main(0).assign_witnesses(values);
    builder.assigned_instances = vec![public_cells.clone()];
    let mut sha_jobs = KagemushaSha256JobsV4::default();
    constrain_kagemusha_parity_scalar_v4(
        &mut builder,
        &mut sha_jobs,
        &public_cells,
        parity,
        params,
        &layout,
        recursion,
        false,
    )
}

fn scalar_field_parent_count_v4<F: ff::Field>(value: F) -> Result<u32, String> {
    if value == F::ZERO {
        Ok(0)
    } else if value == F::ONE {
        Ok(1)
    } else if value == F::ONE + F::ONE {
        Ok(2)
    } else {
        Err("Kagemusha parent proof exposes an invalid parent count".to_owned())
    }
}

fn parent_matches_bootstrap_v4<C>(
    parent: &KagemushaStepParentProofV4<C>,
    bootstrap: &KagemushaStepBootstrapParentSlotV4,
    expected_accumulator: &snark_verifier::pcs::ipa::IpaAccumulator<
        C,
        snark_verifier::loader::native::NativeLoader,
    >,
) -> bool
where
    C: halo2_proofs::halo2curves::CurveAffine,
    C::ScalarExt: From<u64> + PartialEq,
{
    parent.proof_bytes == bootstrap.ordinary_proof_bytes
        && parent.external_accumulation_proof == bootstrap.post_proof_fold
        && parent.carried_lineage.xi == expected_accumulator.xi
        && parent.carried_lineage.u == expected_accumulator.u
        && parent.instances.len() == bootstrap.instances.len()
        && parent
            .instances
            .iter()
            .zip(&bootstrap.instances)
            .all(|(actual, expected)| {
                actual.len() == expected.len()
                    && actual.iter().zip(expected).all(|(actual, expected)| {
                        *actual == C::ScalarExt::from(u64::from(*expected))
                    })
            })
}

fn validate_runtime_parity_v4<C>(
    recursion: &KagemushaStepParityRecursionV4<C>,
    params: &KagemushaStepCircuitParamsV4,
    layout: &KagemushaPastaPublicLayoutV4,
) -> Result<(), String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::ScalarExt: PrimeField + From<u64>,
{
    let expected_instances = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 instance length does not fit usize".to_owned())?;
    let max_proof_bytes = usize::try_from(params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 proof bound does not fit usize".to_owned())?;
    let expected_rounds = usize::try_from(params.k)
        .map_err(|_| "Kagemusha V4 IPA degree does not fit usize".to_owned())?;
    let live_offset = usize::try_from(layout.live_selector_offset)
        .map_err(|_| "Kagemusha V4 live-selector offset does not fit usize".to_owned())?;
    recursion
        .branch_merge_fold
        .validate_fixed_transcript(params.k)?;
    for parent in &recursion.parents {
        if parent.instances.len() != 1
            || parent.instances[0].len() != expected_instances
            || parent.proof_bytes.is_empty()
            || parent.proof_bytes.len() > max_proof_bytes
            || parent.carried_lineage.xi.len() != expected_rounds
            || bool::from(parent.carried_lineage.u.is_identity())
            || !matches!(
                parent.instances[0].get(live_offset),
                Some(value) if *value == C::ScalarExt::ZERO || *value == C::ScalarExt::ONE
            )
        {
            return Err("Kagemusha V4 runtime parent shape mismatch".to_owned());
        }
        parent
            .external_accumulation_proof
            .validate_fixed_transcript(params.k)?;
        scalar_field_parent_count_v4(
            parent.instances[0][KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
        )?;
    }
    Ok(())
}

fn require_kagemusha_step_bootstrap_v4<'a>(
    bootstrap: Option<&'a KagemushaStepBootstrapV4>,
    role: &str,
) -> Result<&'a KagemushaStepBootstrapV4, String> {
    bootstrap.ok_or_else(|| format!("Kagemusha V4 {role} bootstrap artifact is missing"))
}

fn validate_kagemusha_step_witness_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
    require_break_points: bool,
) -> Result<KagemushaPastaPublicLayoutV4, String> {
    let eq_layout = witness
        .public_inputs
        .validate(witness.proof_step_count, step_eq_params)?;
    let ep_layout = witness
        .public_inputs
        .validate(witness.proof_step_count, step_ep_params)?;
    if eq_layout != ep_layout || step_eq_params.k != step_ep_params.k {
        return Err("Kagemusha V4 Eq/Ep public layouts differ".to_owned());
    }
    let step_eq_bootstrap = require_kagemusha_step_bootstrap_v4(witness.step_eq_bootstrap, "Eq")?;
    let step_ep_bootstrap = require_kagemusha_step_bootstrap_v4(witness.step_ep_bootstrap, "Ep")?;
    step_eq_bootstrap.validate_internal(
        step_eq_params,
        KagemushaPastaCycleParityV1::StepEq,
        witness.step_eq_recursion.fixed_structure_sha256,
        require_break_points,
    )?;
    step_ep_bootstrap.validate_internal(
        step_ep_params,
        KagemushaPastaCycleParityV1::StepEp,
        witness.step_ep_recursion.fixed_structure_sha256,
        require_break_points,
    )?;
    validate_runtime_parity_v4(witness.step_eq_recursion, step_eq_params, &eq_layout)?;
    validate_runtime_parity_v4(witness.step_ep_recursion, step_ep_params, &ep_layout)?;

    let parent_count = usize::try_from(witness.public_inputs.parent_count)
        .map_err(|_| "Kagemusha V4 parent count does not fit usize".to_owned())?;
    let live_offset = usize::try_from(eq_layout.live_selector_offset)
        .map_err(|_| "Kagemusha V4 live-selector offset does not fit usize".to_owned())?;
    for slot in 0..parent_count {
        if witness.step_eq_recursion.parents[slot].instances[0][live_offset] != Fp::ONE
            || witness.step_ep_recursion.parents[slot].instances[0][live_offset] != Fq::ONE
        {
            return Err(format!(
                "Kagemusha V4 real parent slot {slot} is not a live proof"
            ));
        }
    }
    for slot in parent_count..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
        let expected = step_eq_bootstrap
            .parent_slot
            .carried_lineage
            .to_eq(step_eq_params.k)?;
        if !parent_matches_bootstrap_v4(
            &witness.step_eq_recursion.parents[slot],
            &step_eq_bootstrap.parent_slot,
            &expected,
        ) {
            return Err(format!(
                "Kagemusha V4 Eq absent parent slot {slot} is not authenticated bootstrap"
            ));
        }
        let expected = step_ep_bootstrap
            .parent_slot
            .carried_lineage
            .to_ep(step_ep_params.k)?;
        if !parent_matches_bootstrap_v4(
            &witness.step_ep_recursion.parents[slot],
            &step_ep_bootstrap.parent_slot,
            &expected,
        ) {
            return Err(format!(
                "Kagemusha V4 Ep absent parent slot {slot} is not authenticated bootstrap"
            ));
        }
    }
    if parent_count < KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
        if witness.step_eq_recursion.branch_merge_fold != step_eq_bootstrap.branch_merge_fold
            || witness.step_ep_recursion.branch_merge_fold != step_ep_bootstrap.branch_merge_fold
        {
            return Err(
                "Kagemusha V4 disabled branch fold is not authenticated bootstrap".to_owned(),
            );
        }
    }
    Ok(eq_layout)
}

fn constrain_kagemusha_parity_scalar_v4<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::ScalarExt>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::ScalarExt>,
    public_cells: &[halo2_base::AssignedValue<C::ScalarExt>],
    parity: KagemushaPastaCycleParityV1,
    params: &KagemushaStepCircuitParamsV4,
    layout: &KagemushaPastaPublicLayoutV4,
    recursion: &KagemushaStepParityRecursionV4<C>,
    bind_public_audits: bool,
) -> Result<KagemushaScalarAuditOutputV4<C>, String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
{
    use std::mem;

    use halo2_ecc::fields::fp::FpChip;
    use snark_verifier::loader::halo2::Halo2Loader;

    use super::kagemusha_cycle_loader::{DeferredScalarEccChip, LIMB_BITS, LIMBS};

    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
    let accumulator_limbs = usize::try_from(layout.accumulator_limbs)
        .map_err(|_| "Kagemusha V4 accumulator length does not fit usize".to_owned())?;
    if public_cells.len() != public_len
        || recursion
            .parents
            .iter()
            .any(|parent| parent.instances.len() != 1 || parent.instances[0].len() != public_len)
    {
        return Err("Kagemusha V4 fixed parent-instance shape mismatch".to_owned());
    }
    let own_protocol_offset = match parity {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5,
    };
    let carried_offset = usize::try_from(match parity {
        KagemushaPastaCycleParityV1::StepEq => layout.parent_eq_accumulator_offset,
        KagemushaPastaCycleParityV1::StepEp => layout.parent_ep_accumulator_offset,
    })
    .map_err(|_| "Kagemusha V4 carried offset does not fit usize".to_owned())?;
    let deferred_offset = usize::try_from(match parity {
        KagemushaPastaCycleParityV1::StepEq => layout.parent_eq_deferred_offset,
        KagemushaPastaCycleParityV1::StepEp => layout.parent_ep_deferred_offset,
    })
    .map_err(|_| "Kagemusha V4 deferred offset does not fit usize".to_owned())?;
    let max_parent_proof_bytes = usize::try_from(params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 proof bound does not fit usize".to_owned())?;

    let range = builder.range_chip();
    let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar_integer = FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let chip = DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer);
    let loader = Halo2Loader::new(chip, mem::take(builder.pool(0)));
    let loaded_protocol = scalar_lineage_v1::load_and_constrain_parent_protocol(
        &loader,
        sha_jobs,
        &recursion.compiled_parent_protocol,
        parity,
        recursion.fixed_structure_sha256,
        &public_cells[own_protocol_offset..own_protocol_offset + 2],
    )
    .map_err(|error| format!("failed to bind Kagemusha V4 parent protocol: {error:?}"))?;
    let parent_count = public_cells[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5];
    let slot_present = scalar_lineage_v1::constrain_parent_slot_selectors_v4(&loader, parent_count);

    let mut lineages = Vec::with_capacity(2);
    let mut inner_parent_counts = [0_u32; 2];
    for slot in 0..2 {
        let parent = &recursion.parents[slot];
        inner_parent_counts[slot] = scalar_field_parent_count_v4(
            parent.instances[0][KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
        )?;
        let bindings = [
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + 2,
                expected: &public_cells[KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5
                    + slot * 2
                    ..KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5 + (slot + 1) * 2],
            },
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2,
                expected: &public_cells[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2],
            },
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 4,
                expected: &public_cells[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 4],
            },
        ];
        let lineage = scalar_lineage_v1::constrain_parent_scalar_lineage_v4(
            &loader,
            &recursion.succinct_vk,
            &loaded_protocol,
            slot,
            slot_present[slot],
            params.k,
            max_parent_proof_bytes,
            accumulator_limbs,
            scalar_lineage_v1::ParentScalarLineageWitnessV4 {
                instances: &parent.instances,
                proof_bytes: &parent.proof_bytes,
                carried_lineage: &parent.carried_lineage,
                carried_lineage_instance_column: 0,
                carried_lineage_instance_range: carried_offset..carried_offset + accumulator_limbs,
                instance_copy_bindings: &bindings,
                external_accumulation_proof: &parent.external_accumulation_proof,
            },
        )
        .map_err(|error| {
            format!("failed to constrain Kagemusha V4 parent slot {slot}: {error:?}")
        })?;
        lineages.push(lineage);
    }

    let branch = scalar_lineage_v1::constrain_exposed_parent_lineage_v4(
        &loader,
        &recursion.succinct_vk,
        params.k,
        accumulator_limbs,
        &lineages[0].accumulator,
        &lineages[1].accumulator,
        slot_present,
        &recursion.branch_merge_fold,
        &public_cells[carried_offset..carried_offset + accumulator_limbs],
    )
    .map_err(|error| format!("failed to constrain Kagemusha V4 branch lineage: {error:?}"))?;
    let mut all_stages = lineages
        .iter()
        .flat_map(|lineage| lineage.stages.iter().cloned())
        .collect::<Vec<_>>();
    all_stages.extend(branch.stages.iter().cloned());
    let complete_audit = loader.ecc_chip().witness();
    let complete_shapes = all_stages
        .iter()
        .map(|stage| stage.shape())
        .collect::<Vec<_>>();
    if bind_public_audits {
        scalar_lineage_v1::constrain_scalar_audit_identity_v4(
            &loader,
            sha_jobs,
            &range,
            &all_stages,
            slot_present,
            [
                &public_cells[deferred_offset..deferred_offset + 2],
                &public_cells[deferred_offset + 2..deferred_offset + 4],
            ],
        )
        .map_err(|error| format!("failed to bind Kagemusha V4 complete audit: {error:?}"))?;
    }
    let identity = loaded_protocol.identity_witness.clone();
    *builder.pool(0) = loader.take_ctx();

    Ok(KagemushaScalarAuditOutputV4 {
        identity,
        audit: complete_audit,
        stages: complete_shapes,
        inner_parent_counts,
    })
}

fn constrain_equal_if_v4<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    enabled: halo2_base::AssignedValue<F>,
    lhs: halo2_base::AssignedValue<F>,
    rhs: halo2_base::AssignedValue<F>,
) where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    range.gate().assert_bit(ctx, enabled);
    let difference = range.gate().sub(ctx, Existing(lhs), Existing(rhs));
    let selected = range
        .gate()
        .mul(ctx, Existing(enabled), Existing(difference));
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn pack_assigned_u32_words_v5<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    words: &[halo2_base::AssignedValue<F>],
) -> halo2_base::AssignedValue<F>
where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Constant,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    let radix = F::from(1_u64 << 32);
    let mut weight = F::ONE;
    let mut weights = Vec::with_capacity(words.len());
    for _ in words {
        weights.push(Constant(weight));
        weight *= radix;
    }
    range
        .gate()
        .inner_product(ctx, words.iter().copied(), weights)
}

fn pack_assigned_u32_limbs_for_poseidon_v5(
    ctx: &mut halo2_base::Context<Fp>,
    range: &halo2_base::gates::RangeChip<Fp>,
    limbs: &[halo2_base::AssignedValue<Fp>],
) -> Vec<halo2_base::AssignedValue<Fp>> {
    limbs
        .chunks(7)
        .map(|chunk| pack_assigned_u32_words_v5(ctx, range, chunk))
        .collect()
}

fn constrain_exact_u32_digest_chunks_v5<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    words: &[halo2_base::AssignedValue<F>],
    expected: &[halo2_base::AssignedValue<F>],
) -> Result<(), String>
where
    F: halo2_base::utils::BigPrimeField,
{
    if words.len() != 8 || expected.len() != KAGEMUSHA_COMPACT_DIGEST_CHUNKS_V5 {
        return Err("Kagemusha V5 digest chunk shape mismatch".to_owned());
    }
    for (words, expected) in words.chunks_exact(4).zip(expected) {
        let packed = pack_assigned_u32_words_v5(ctx, range, words);
        ctx.constrain_equal(&packed, expected);
    }
    Ok(())
}

fn constrain_fp_commitment_chunks_v5(
    ctx: &mut halo2_base::Context<Fp>,
    range: &halo2_base::gates::RangeChip<Fp>,
    commitment: halo2_base::AssignedValue<Fp>,
    expected: &[halo2_base::AssignedValue<Fp>],
    enabled: halo2_base::AssignedValue<Fp>,
) -> Result<(), String> {
    use halo2_base::{
        QuantumCell::{Constant, Existing},
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    const FP_MODULUS_LOW: u128 =
        (0x2246_98fc_u128 << 96) | (0x094c_f91b_u128 << 64) | (0x992d_30ed_u128 << 32) | 1;
    const FP_MODULUS_HIGH: u128 = 0x4000_0000_0000_0000_0000_0000_0000_0000;
    if expected.len() != 2 {
        return Err("Kagemusha V5 commitment chunk shape mismatch".to_owned());
    }
    let [low, high]: [halo2_base::AssignedValue<Fp>; 2] = expected
        .try_into()
        .expect("validated commitment chunk pair");
    range.range_check(ctx, low, 128);
    range.range_check(ctx, high, 128);
    let high_less = range.is_less_than(ctx, high, Constant(Fp::from_u128(FP_MODULUS_HIGH)), 128);
    let high_equal = range
        .gate()
        .is_equal(ctx, high, Constant(Fp::from_u128(FP_MODULUS_HIGH)));
    let low_less = range.is_less_than(ctx, low, Constant(Fp::from_u128(FP_MODULUS_LOW)), 128);
    let equal_and_low = range
        .gate()
        .mul(ctx, Existing(high_equal), Existing(low_less));
    let canonical = range
        .gate()
        .add(ctx, Existing(high_less), Existing(equal_and_low));
    range.gate().assert_is_const(ctx, &canonical, &Fp::ONE);

    let two_to_128 = Fp::from_u128(1_u128 << 127) + Fp::from_u128(1_u128 << 127);
    let reconstructed = range.gate().mul_add(ctx, high, Constant(two_to_128), low);
    constrain_equal_if_v4(ctx, range, enabled, reconstructed, commitment);
    Ok(())
}

fn constrain_kagemusha_compact_eq_header_v5(
    ctx: &mut halo2_base::Context<Fp>,
    range: &halo2_base::gates::RangeChip<Fp>,
    compact: &[halo2_base::AssignedValue<Fp>],
    semantic: &[halo2_base::AssignedValue<Fp>],
) -> Result<(), String> {
    use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};

    if compact.len() != 64 || semantic.len() < KAGEMUSHA_PASTA_STEP_EP_PROTOCOL_SHA256_OFFSET_V4 + 8
    {
        return Err("Kagemusha V5 compact/private semantic shape mismatch".to_owned());
    }
    range.gate().assert_is_const(
        ctx,
        &compact[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5],
        &Fp::from(u64::from(KAGEMUSHA_COMPACT_PROFILE_VERSION_V5)),
    );
    ctx.constrain_equal(
        &compact[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
        &semantic[KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4],
    );
    constrain_exact_u32_digest_chunks_v5(
        ctx,
        range,
        &semantic[KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4
            ..KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4 + 8],
        &compact[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2],
    )?;
    constrain_exact_u32_digest_chunks_v5(
        ctx,
        range,
        &semantic[KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4
            ..KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4 + 8],
        &compact[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2],
    )?;
    constrain_exact_u32_digest_chunks_v5(
        ctx,
        range,
        &semantic[KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4
            ..KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4 + 8],
        &compact[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2],
    )?;
    constrain_exact_u32_digest_chunks_v5(
        ctx,
        range,
        &semantic[KAGEMUSHA_PASTA_STEP_EP_PROTOCOL_SHA256_OFFSET_V4
            ..KAGEMUSHA_PASTA_STEP_EP_PROTOCOL_SHA256_OFFSET_V4 + 8],
        &compact[KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2],
    )?;

    let poseidon =
        super::confidential_v2::confidential_relation_gadget::ConfidentialPoseidonChipV3::new(
            ctx, range,
        );
    let operation_limbs = &semantic[KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4
        ..KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4 + KAGEMUSHA_STEP_OPERATION_LIMBS_V4];
    let packed_operation = pack_assigned_u32_limbs_for_poseidon_v5(ctx, range, operation_limbs);
    let operation_commitment = poseidon.hash(
        ctx,
        range,
        KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_DOMAIN_V5,
        &packed_operation,
    );
    let one = ctx.load_constant(Fp::ONE);
    constrain_fp_commitment_chunks_v5(
        ctx,
        range,
        operation_commitment,
        &compact[KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5
            ..KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5 + 2],
        one,
    )?;

    let parent_count = compact[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5];
    let slot_present = constrain_two_parent_presence_bits(ctx, range, parent_count);
    let zero = ctx.load_zero();
    for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
        let start = kagemusha_pasta_parent_state_offset_v4(slot);
        let packed_state = pack_assigned_u32_limbs_for_poseidon_v5(
            ctx,
            range,
            &semantic[start..start + KAGEMUSHA_PASTA_STATE_STRIDE_V4],
        );
        let commitment = poseidon.hash(
            ctx,
            range,
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            &packed_state,
        );
        let compact_start = KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5 + slot * 2;
        constrain_fp_commitment_chunks_v5(
            ctx,
            range,
            commitment,
            &compact[compact_start..compact_start + 2],
            slot_present[slot],
        )?;
        let absent = range.gate().not(ctx, slot_present[slot]);
        for chunk in &compact[compact_start..compact_start + 2] {
            constrain_equal_if_v4(ctx, range, absent, *chunk, zero);
        }
    }
    let result_start = KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4;
    let packed_result = pack_assigned_u32_limbs_for_poseidon_v5(
        ctx,
        range,
        &semantic[result_start..result_start + KAGEMUSHA_PASTA_STATE_STRIDE_V4],
    );
    let result_commitment = poseidon.hash(
        ctx,
        range,
        KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
        &packed_result,
    );
    constrain_fp_commitment_chunks_v5(
        ctx,
        range,
        result_commitment,
        &compact[KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5
            ..KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + 2],
        one,
    )?;

    // The step count is runtime witness data. Never assert it as a constant:
    // doing so would place the keygen step into fixed columns and make every
    // later recursive step incompatible with the authenticated StepEq key.
    // Bind the compact header only to the constrained native operation field.
    let operation_step =
        KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4 + super::kagemusha_v2::I_PROOF_STEP_COUNT * 8;
    let operation_step_value =
        pack_assigned_u32_words_v5(ctx, range, &semantic[operation_step..operation_step + 8]);
    ctx.constrain_equal(
        &compact[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5],
        &operation_step_value,
    );
    range.range_check(
        ctx,
        compact[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5],
        32,
    );
    Ok(())
}

fn constrain_kagemusha_output_frontier_v4<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    bindings: &super::kagemusha_step_transition::NamedTransitionBindings<F>,
    output: &[halo2_base::AssignedValue<F>;
         super::kagemusha_v2::KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4],
    topup_leaf_index: halo2_base::AssignedValue<F>,
) where
    F: halo2_base::utils::BigPrimeField,
{
    constrain_equal_if_v4(ctx, range, bindings.is_init, topup_leaf_index, output[7]);
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_append,
        output[7],
        bindings.input_next_zero_leaf_index,
    );
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_redemption,
        output[9],
        bindings.input_next_zero_leaf_index,
    );
    ctx.constrain_equal(&output[10], &bindings.output_next_zero_leaf_index);
}

fn constrain_kagemusha_common_transition<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    sha_jobs: &mut KagemushaSha256JobsV4<F>,
    public_cells: &[halo2_base::AssignedValue<F>],
    expected_public_len: usize,
) -> Result<super::kagemusha_step_transition::NamedTransitionBindings<F>, String>
where
    F: halo2_base::utils::BigPrimeField,
{
    if public_cells.len() != expected_public_len {
        return Err("Kagemusha Step public column has the wrong length".to_owned());
    }
    let operation: &[halo2_base::AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_LIMBS_V4] =
        public_cells[KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4
            ..KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4 + KAGEMUSHA_STEP_OPERATION_LIMBS_V4]
            .try_into()
            .expect("validated fixed operation range");
    let parent_states: [&[halo2_base::AssignedValue<F>]; 2] = std::array::from_fn(|slot| {
        &public_cells[kagemusha_pasta_parent_state_offset_v4(slot)
            ..kagemusha_pasta_parent_state_offset_v4(slot) + KAGEMUSHA_PASTA_STATE_STRIDE_V4]
    });
    let result_state = &public_cells[KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4
        ..KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4 + KAGEMUSHA_PASTA_STATE_STRIDE_V4];
    let bindings = super::kagemusha_step_transition::constrain_two_input_step_transition_v4(
        ctx,
        range,
        sha_jobs,
        public_cells[KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4],
        parent_states,
        result_state,
        operation,
    )?;
    for (operation_limb, public_limb) in bindings.statement_digest_limbs.iter().zip(
        &public_cells[KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4
            ..KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4 + 8],
    ) {
        ctx.constrain_equal(operation_limb, public_limb);
    }
    for index in 0..8 {
        let operation_limb = bindings.operation.limbs
            [(super::kagemusha_v2::I_ARTIFACT_MANIFEST_SHA256 + index / 2) * 8 + index % 2];
        ctx.constrain_equal(
            &operation_limb,
            &public_cells[KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4 + index],
        );
    }
    Ok(bindings)
}

fn operation_u128_v4<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    operation: &super::kagemusha_step_transition::AssignedKagemushaStepOperationV4<F>,
    low_index: usize,
) -> halo2_base::AssignedValue<F>
where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Constant,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    range.gate().mul_add(
        ctx,
        operation.fields[low_index + 1],
        Constant(F::from_u128(1_u128 << 64)),
        operation.fields[low_index],
    )
}

fn constrain_kagemusha_eq_secure_relations_v4(
    ctx: &mut halo2_base::Context<Fp>,
    range: &halo2_base::gates::RangeChip<Fp>,
    bindings: &super::kagemusha_step_transition::NamedTransitionBindings<Fp>,
    secure: &super::confidential_v2::KagemushaStepSecureWitnessV3,
    membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
) -> Result<(), String> {
    use ff::Field as _;
    use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};

    use super::{confidential_v2, kagemusha_v2};

    const DEPTH: usize = confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2;
    let topup = confidential_v2::secure_relation_v3::assign_kagemusha_topup_shield_v3::<DEPTH>(
        ctx,
        range,
        Some(&secure.topup),
    )?;
    let transfer = confidential_v2::secure_relation_v3::assign_confidential_transfer_step_v4::<
        DEPTH,
    >(ctx, range, Some(&secure.transfer))?;
    let unshield =
        confidential_v2::secure_relation_v3::assign_confidential_unshield_change_step_v4::<DEPTH>(
            ctx,
            range,
            Some(&secure.unshield_change),
        )?;
    let output = kagemusha_v2::output_membership_v4::assign_kagemusha_output_membership_v4(
        ctx,
        range,
        Some(membership),
    )?;

    for (lhs, rhs) in [
        (output[0], bindings.is_init),
        (output[1], bindings.is_append),
        (output[2], bindings.is_redemption),
        (output[3], bindings.has_change),
        (output[4], bindings.input_root),
        (output[5], bindings.output_root),
        (output[6], bindings.recipient_commitment),
        (output[8], bindings.change_commitment),
    ] {
        ctx.constrain_equal(&lhs, &rhs);
    }
    // The secure membership relation exposes the exact output leaf positions.
    // Copy-bind those positions to the append-only public-state frontier so a
    // recursive proof cannot skip an empty leaf or seed an arbitrary frontier.
    constrain_kagemusha_output_frontier_v4(ctx, range, bindings, &output, topup[6]);

    let operation = &bindings.operation;
    let init_amount = operation_u128_v4(ctx, range, operation, kagemusha_v2::I_CURRENT_AMOUNT_LO);
    for (lhs, rhs) in [
        (topup[0], bindings.recipient_commitment),
        (
            topup[1],
            operation.fields[kagemusha_v2::I_CURRENT_NULLIFIER],
        ),
        (topup[2], bindings.input_root),
        (topup[3], bindings.output_root),
        (topup[4], init_amount),
        (topup[5], operation.fields[kagemusha_v2::I_ASSET_SCALE]),
        (topup[7], operation.fields[kagemusha_v2::I_ASSET_TAG]),
        (topup[8], operation.fields[kagemusha_v2::I_CHAIN_TAG]),
    ] {
        constrain_equal_if_v4(ctx, range, bindings.is_init, lhs, rhs);
    }
    super::kagemusha_step_transition::constrain_kagemusha_step_init_topup_tags_v4(
        ctx, range, bindings, topup[9], topup[10],
    );

    let input_amount = operation_u128_v4(ctx, range, operation, kagemusha_v2::I_INPUT_AMOUNT_LO);
    let recipient_amount =
        operation_u128_v4(ctx, range, operation, kagemusha_v2::I_RECIPIENT_AMOUNT_LO);
    let change_amount = operation_u128_v4(ctx, range, operation, kagemusha_v2::I_CHANGE_AMOUNT_LO);
    for (lhs, rhs) in [
        (transfer.input_amount, input_amount),
        (transfer.recipient_amount, recipient_amount),
        (transfer.change_amount, change_amount),
        (transfer.has_change, bindings.has_change),
    ] {
        constrain_equal_if_v4(ctx, range, bindings.is_append, lhs, rhs);
    }
    let one = ctx.load_constant(Fp::ONE);
    let append_second_input = range.gate().sub(
        ctx,
        operation.fields[kagemusha_v2::I_TRANSFER_INPUT_COUNT],
        one,
    );
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_append,
        transfer.has_second_input,
        append_second_input,
    );
    for (lhs, rhs) in transfer.public.into_iter().zip([
        bindings.input_commitments[0],
        bindings.input_commitments[1],
        bindings.input_nullifiers[0],
        bindings.input_nullifiers[1],
        bindings.recipient_commitment,
        bindings.change_commitment,
        bindings.input_root,
        operation.fields[kagemusha_v2::I_ASSET_TAG],
        operation.fields[kagemusha_v2::I_CHAIN_TAG],
    ]) {
        constrain_equal_if_v4(ctx, range, bindings.is_append, lhs, rhs);
    }

    let public_amount = operation.fields[kagemusha_v2::I_UNSHIELD_PUBLIC_AMOUNT];
    for (lhs, rhs) in [
        (unshield.input_amount, input_amount),
        (unshield.change_amount, change_amount),
    ] {
        constrain_equal_if_v4(ctx, range, bindings.is_redemption, lhs, rhs);
    }
    let zero = ctx.load_constant(Fp::ZERO);
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_redemption,
        unshield.has_second_input,
        zero,
    );
    for (lhs, rhs) in unshield.public.into_iter().zip([
        bindings.input_commitments[0],
        bindings.input_commitments[1],
        bindings.input_nullifiers[0],
        bindings.input_nullifiers[1],
        bindings.change_commitment,
        bindings.input_root,
        public_amount,
        operation.fields[kagemusha_v2::I_ASSET_TAG],
        operation.fields[kagemusha_v2::I_CHAIN_TAG],
    ]) {
        constrain_equal_if_v4(ctx, range, bindings.is_redemption, lhs, rhs);
    }
    Ok(())
}

fn constrain_kagemusha_reciprocal_output_v4<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::Base>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::Base>,
    dense_jobs: &mut KagemushaDenseMsmJobsV5<C>,
    public_cells: &[halo2_base::AssignedValue<C::Base>],
    layout: &KagemushaPastaPublicLayoutV4,
    output: &KagemushaScalarAuditOutputV4<C>,
) -> Result<(), String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField
        + halo2_base::utils::ScalarField
        + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    use std::mem;

    use halo2_ecc::fields::fp::FpChip;

    use super::kagemusha_cycle_loader::{LIMB_BITS, LIMBS};

    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 reciprocal public length does not fit usize".to_owned())?;
    if public_cells.len() != public_len {
        return Err("Kagemusha V4 reciprocal public column has the wrong length".to_owned());
    }
    let (protocol_offset, deferred_offset) = match output.identity.parity {
        KagemushaPastaCycleParityV1::StepEq => (
            KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
            usize::try_from(layout.parent_eq_deferred_offset)
                .map_err(|_| "Kagemusha V4 Eq audit offset does not fit usize".to_owned())?,
        ),
        KagemushaPastaCycleParityV1::StepEp => (
            KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5,
            usize::try_from(layout.parent_ep_deferred_offset)
                .map_err(|_| "Kagemusha V4 Ep audit offset does not fit usize".to_owned())?,
        ),
    };
    let range = builder.range_chip();
    let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let mut ctx = mem::take(builder.pool(0));
    let parent_public_parent_counts = output
        .inner_parent_counts
        .map(|count| ctx.main().load_witness(C::Base::from(u64::from(count))));
    constrain_reciprocal_point_audit_identity_v4::<C>(
        &mut ctx,
        sha_jobs,
        &base,
        &scalar,
        &output.audit,
        &output.stages,
        public_cells[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
        parent_public_parent_counts,
        [
            &public_cells[deferred_offset..deferred_offset + 2],
            &public_cells[deferred_offset + 2..deferred_offset + 4],
        ],
        KagemushaDeferredMsmV5::Dense(dense_jobs),
    )?;
    constrain_reciprocal_protocol_identity::<C>(
        &mut ctx,
        sha_jobs,
        &base,
        &scalar,
        &output.identity,
        output.identity.structure_sha256,
        &public_cells[protocol_offset..protocol_offset + 2],
    )?;
    *builder.pool(0) = ctx;
    Ok(())
}

fn kagemusha_builder_without_witnesses_v4<F>(
    builder: &halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>
where
    F: halo2_base::utils::ScalarField,
{
    builder.deep_clone().unknown(true)
}

#[derive(Clone, Debug)]
pub(crate) struct KagemushaStepCompositeConfigV4<F: halo2_base::utils::ScalarField> {
    base: halo2_base::gates::circuit::BaseConfig<F>,
    sha: KagemushaSha256ConfigV4,
    dense: KagemushaDenseMsmConfigV5,
}

#[derive(Clone)]
pub(crate) struct KagemushaStepEqCircuitV4 {
    params: KagemushaStepCircuitParamsV4,
    builder: halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp>,
    sha_jobs: KagemushaSha256JobsV4<Fp>,
    dense_jobs: KagemushaDenseMsmJobsV5<halo2_proofs::halo2curves::pasta::EpAffine>,
}

impl halo2_proofs::plonk::Circuit<Fp> for KagemushaStepEqCircuitV4 {
    type Config = KagemushaStepCompositeConfigV4<Fp>;
    type FloorPlanner = halo2_proofs::circuit::V1;
    type Params = KagemushaStepCircuitParamsV4;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            params: self.params.clone(),
            builder: kagemusha_builder_without_witnesses_v4(&self.builder),
            sha_jobs: self.sha_jobs.unknown(),
            dense_jobs: self.dense_jobs.unknown(),
        }
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fp>,
        params: Self::Params,
    ) -> Self::Config {
        let base = kagemusha_base_circuit_params_v4(&params)
            .expect("authenticated Kagemusha StepEq V4 circuit parameters");
        let usable_rows = kagemusha_usable_rows_v4(&params)
            .expect("authenticated Kagemusha StepEq V4 unusable-row bound");
        let mut base = halo2_base::gates::circuit::BaseConfig::configure(meta, base);
        base.set_usable_rows(usable_rows);
        KagemushaStepCompositeConfigV4 {
            base,
            sha: KagemushaSha256ConfigV4::configure(meta),
            dense: KagemushaDenseMsmConfigV5::configure::<halo2_proofs::halo2curves::pasta::EpAffine>(
                meta,
            ),
        }
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fp>) -> Self::Config {
        unreachable!("Kagemusha StepEq V4 requires authenticated circuit parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<Fp>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let usable_rows = kagemusha_usable_rows_v4(&self.params)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp> as halo2_proofs::plonk::Circuit<
            Fp,
        >>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "Kagemusha StepEq Base"),
        )?;
        self.sha_jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            usable_rows,
        )?;
        self.dense_jobs.synthesize(
            &config.dense,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            usable_rows,
        )
    }
}

/// Production StepEp circuit type with explicit authenticated V4 parameters.
#[derive(Clone)]
pub(crate) struct KagemushaStepEpCircuitV4 {
    params: KagemushaStepCircuitParamsV4,
    builder: halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq>,
    sha_jobs: KagemushaSha256JobsV4<Fq>,
    dense_jobs: KagemushaDenseMsmJobsV5<halo2_proofs::halo2curves::pasta::EqAffine>,
}

impl halo2_proofs::plonk::Circuit<Fq> for KagemushaStepEpCircuitV4 {
    type Config = KagemushaStepCompositeConfigV4<Fq>;
    type FloorPlanner = halo2_proofs::circuit::V1;
    type Params = KagemushaStepCircuitParamsV4;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            params: self.params.clone(),
            builder: kagemusha_builder_without_witnesses_v4(&self.builder),
            sha_jobs: self.sha_jobs.unknown(),
            dense_jobs: self.dense_jobs.unknown(),
        }
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fq>,
        params: Self::Params,
    ) -> Self::Config {
        let base = kagemusha_base_circuit_params_v4(&params)
            .expect("authenticated Kagemusha StepEp V4 circuit parameters");
        let usable_rows = kagemusha_usable_rows_v4(&params)
            .expect("authenticated Kagemusha StepEp V4 unusable-row bound");
        let mut base = halo2_base::gates::circuit::BaseConfig::configure(meta, base);
        base.set_usable_rows(usable_rows);
        KagemushaStepCompositeConfigV4 {
            base,
            sha: KagemushaSha256ConfigV4::configure(meta),
            dense: KagemushaDenseMsmConfigV5::configure::<halo2_proofs::halo2curves::pasta::EqAffine>(
                meta,
            ),
        }
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fq>) -> Self::Config {
        unreachable!("Kagemusha StepEp V4 requires authenticated circuit parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<Fq>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let usable_rows = kagemusha_usable_rows_v4(&self.params)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq> as halo2_proofs::plonk::Circuit<
            Fq,
        >>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "Kagemusha StepEp Base"),
        )?;
        self.sha_jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            usable_rows,
        )?;
        self.dense_jobs.synthesize(
            &config.dense,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            usable_rows,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KagemushaStepPublicModeV4 {
    Live,
    Bootstrap,
}

#[derive(Clone, Copy)]
enum KagemushaCircuitBuilderStageV5<'a> {
    Keygen,
    Prover(&'a [Vec<u32>]),
}

impl KagemushaCircuitBuilderStageV5<'_> {
    fn requires_break_points(self) -> bool {
        matches!(self, Self::Prover(_))
    }
}

fn kagemusha_base_builder_for_stage_v5<F>(
    circuit_params: &KagemushaStepCircuitParamsV4,
    stage: KagemushaCircuitBuilderStageV5<'_>,
) -> Result<halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>, String>
where
    F: halo2_base::utils::ScalarField,
{
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;

    let params = kagemusha_base_circuit_params_v4(circuit_params)?;
    Ok(match stage {
        KagemushaCircuitBuilderStageV5::Keygen => {
            BaseCircuitBuilder::<F>::new(false).use_params(params)
        }
        KagemushaCircuitBuilderStageV5::Prover(wire) => BaseCircuitBuilder::<F>::prover(
            params,
            kagemusha_break_points_from_wire_v5(wire, circuit_params)?,
        ),
    })
}

/// Assign the exposed V4 column and gate its complete semantic interpretation
/// behind the appended live selector.
///
/// Both modes assign and constrain the same two columns of advice values. In
/// live mode every semantic limb is copy-equivalent to its exposed limb. In
/// bootstrap mode every exposed limb (including the selector) is constrained
/// to zero, while the same fixed-shape semantic relation is populated with the
/// adapter's private calibration witness. Consequently a bootstrap proof has
/// no public spend/state meaning and cannot be replayed as a live proof.
fn assign_kagemusha_public_mode_v4<F>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    semantic_values: Vec<F>,
    layout: &KagemushaPastaPublicLayoutV4,
    mode: KagemushaStepPublicModeV4,
) -> Result<Vec<halo2_base::AssignedValue<F>>, String>
where
    F: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
    let live_offset = usize::try_from(layout.live_selector_offset)
        .map_err(|_| "Kagemusha V4 live-selector offset does not fit usize".to_owned())?;
    if semantic_values.len() != public_len || live_offset >= public_len {
        return Err("Kagemusha V4 semantic public column has the wrong length".to_owned());
    }
    let exposed_values = match mode {
        KagemushaStepPublicModeV4::Live => semantic_values.clone(),
        KagemushaStepPublicModeV4::Bootstrap => vec![F::ZERO; public_len],
    };
    let exposed = builder.main(0).assign_witnesses(exposed_values);
    let semantic = builder.main(0).assign_witnesses(semantic_values);
    builder.assigned_instances = vec![exposed.clone()];

    let range = builder.range_chip();
    let ctx = builder.main(0);
    for cell in &semantic {
        range.range_check(ctx, *cell, 128);
    }
    let live = exposed[live_offset];
    range.gate().assert_bit(ctx, live);
    range
        .gate()
        .assert_is_const(ctx, &semantic[live_offset], &F::ONE);
    range.gate().assert_is_const(
        ctx,
        &semantic[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5],
        &F::from(u64::from(KAGEMUSHA_COMPACT_PROFILE_VERSION_V5)),
    );
    range.range_check(ctx, semantic[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5], 2);
    let invalid_parent_count = range.gate().is_equal(
        ctx,
        semantic[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
        halo2_base::QuantumCell::Constant(F::from(3)),
    );
    range
        .gate()
        .assert_is_const(ctx, &invalid_parent_count, &F::ZERO);
    range.range_check(
        ctx,
        semantic[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5],
        32,
    );
    let not_live = range.gate().not(ctx, live);
    for (exposed, semantic) in exposed.iter().zip(&semantic) {
        let bootstrap_value = range
            .gate()
            .mul(ctx, Existing(not_live), Existing(*exposed));
        range
            .gate()
            .assert_is_const(ctx, &bootstrap_value, &F::ZERO);
        constrain_equal_if_v4(ctx, &range, live, *exposed, *semantic);
    }
    Ok(semantic)
}

fn validate_kagemusha_populated_builder_fit_v5<F>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    role: &str,
) -> Result<(), String>
where
    F: halo2_base::utils::ScalarField,
{
    let pinned = kagemusha_base_circuit_params_v4(circuit_params)?;
    let unusable_rows = usize::try_from(KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4)
        .map_err(|_| "Kagemusha V5 unusable-row count does not fit usize".to_owned())?;
    let required = builder.calculate_params(Some(unusable_rows));
    // `calculate_params` installs its result. Restore the authenticated shape
    // before the circuit can escape this constructor.
    builder.set_params(pinned.clone());

    let phase_fits = |needed: &[usize], available: &[usize]| {
        needed.len() <= available.len()
            && needed
                .iter()
                .zip(available)
                .all(|(needed, available)| needed <= available)
    };
    if required.k != pinned.k
        || required.lookup_bits != pinned.lookup_bits
        || !phase_fits(&required.num_advice_per_phase, &pinned.num_advice_per_phase)
        || !kagemusha_lookup_phase_columns_fit_v5(
            &required.num_lookup_advice_per_phase,
            &pinned.num_lookup_advice_per_phase,
        )
        || required.num_fixed > pinned.num_fixed
        || required.num_instance_columns > pinned.num_instance_columns
    {
        return Err(format!(
            "Kagemusha V5 {role} populated circuit does not fit the authenticated k={} [advice={:?}, lookup={:?}, fixed={}, instance={}] shape (required advice={:?}, lookup={:?}, fixed={}, instance={})",
            pinned.k,
            pinned.num_advice_per_phase,
            pinned.num_lookup_advice_per_phase,
            pinned.num_fixed,
            pinned.num_instance_columns,
            required.num_advice_per_phase,
            required.num_lookup_advice_per_phase,
            required.num_fixed,
            required.num_instance_columns,
        ));
    }
    Ok(())
}

/// Return whether the populated lookup-column counts fit the authenticated
/// per-phase shape.
///
/// `BaseCircuitBuilder::calculate_params` reports every supported phase, so
/// an unused suffix is represented as zero-width phases (for example
/// `[1, 0, 0]`). Authenticated first-release parameters omit that suffix.
/// Canonicalizing only the trailing zero-width phases preserves all real phase
/// positions: an internal zero or a later non-zero phase can never be shifted.
fn kagemusha_lookup_phase_columns_fit_v5(needed: &[usize], available: &[usize]) -> bool {
    fn trim_trailing_zero_phases(phases: &[usize]) -> &[usize] {
        let canonical_len = phases
            .iter()
            .rposition(|columns| *columns != 0)
            .map_or(0, |index| index + 1);
        &phases[..canonical_len]
    }

    let needed = trim_trailing_zero_phases(needed);
    let available = trim_trailing_zero_phases(available);
    needed.len() <= available.len()
        && needed
            .iter()
            .zip(available)
            .all(|(needed, available)| needed <= available)
}

fn validate_kagemusha_witness_builder_break_points_v5<F>(
    builder: &halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    wire: &[Vec<u32>],
    role: &str,
) -> Result<(), String>
where
    F: halo2_base::utils::ScalarField,
{
    if !builder.witness_gen_only() {
        return Err(format!(
            "Kagemusha V5 {role} live builder retained its constraint graph"
        ));
    }
    let break_points = kagemusha_break_points_from_wire_v5(wire, circuit_params)?;
    let max_rows = kagemusha_break_point_max_rows_v5(circuit_params)?;
    let statistics = builder.core().statistics();
    if statistics.total_advice_per_phase.len() != break_points.len() {
        return Err(format!(
            "Kagemusha V5 {role} witness phase count differs from authenticated breakpoints"
        ));
    }
    for (phase, (total_cells, cumulative_points)) in statistics
        .total_advice_per_phase
        .iter()
        .zip(wire)
        .enumerate()
    {
        let total_cells = *total_cells;
        let mut previous = 0_usize;
        for point in cumulative_points {
            let point = usize::try_from(*point)
                .map_err(|_| "Kagemusha V5 breakpoint does not fit usize".to_owned())?;
            if point <= previous || point >= total_cells || point - previous >= max_rows {
                return Err(format!(
                    "Kagemusha V5 {role} phase {phase} breakpoint does not cover the witness graph"
                ));
            }
            previous = point;
        }
        let final_segment = if cumulative_points.is_empty() {
            total_cells
        } else {
            total_cells - previous
        };
        if final_segment == 0 || final_segment > max_rows {
            return Err(format!(
                "Kagemusha V5 {role} phase {phase} final witness segment exceeds the usable domain"
            ));
        }
    }
    for (phase, (lookup, columns)) in builder
        .lookup_manager()
        .iter()
        .zip(&circuit_params.num_lookup_advice_per_phase)
        .enumerate()
    {
        let capacity = usize::try_from(*columns)
            .ok()
            .and_then(|columns| columns.checked_mul(max_rows))
            .ok_or_else(|| "Kagemusha V5 lookup capacity overflows usize".to_owned())?;
        if lookup.total_rows() > capacity {
            return Err(format!(
                "Kagemusha V5 {role} phase {phase} lookup witness exceeds the authenticated shape"
            ));
        }
    }
    Ok(())
}

fn ensure_kagemusha_keygen_break_points_v5<F>(
    builder: &halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    expected: &[Vec<u32>],
    role: &str,
) -> Result<(), String>
where
    F: halo2_base::utils::ScalarField,
{
    if builder.witness_gen_only()
        || kagemusha_break_points_to_wire_v5(builder.break_points(), circuit_params)? != expected
    {
        return Err(format!(
            "Kagemusha V5 {role} keygen breakpoints differ from the authenticated bootstrap"
        ));
    }
    Ok(())
}

fn format_kagemusha_consuming_keygen_error_v5(
    error: halo2_proofs::plonk::KeygenWithExtractorError<String>,
    context: &str,
) -> String {
    match error {
        halo2_proofs::plonk::KeygenWithExtractorError::Keygen(error) => {
            format!("{context}: {error}")
        }
        halo2_proofs::plonk::KeygenWithExtractorError::Extractor(error) => error,
    }
}

/// Collect the reciprocal scalar-verifier outputs without retaining either
/// populated proof circuit. Each native prepass owns and drops its temporary
/// builder before the other parity is collected.
fn collect_kagemusha_step_scalar_audits_v5(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
    require_break_points: bool,
) -> Result<
    (
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    ),
    String,
> {
    validate_kagemusha_step_witness_v4(
        witness,
        step_eq_params,
        step_ep_params,
        require_break_points,
    )?;
    let eq_output = collect_kagemusha_scalar_audits_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        witness.public_inputs,
        witness.proof_step_count,
        step_eq_params,
        witness.step_eq_recursion,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let ep_output = collect_kagemusha_scalar_audits_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        witness.public_inputs,
        witness.proof_step_count,
        step_ep_params,
        witness.step_ep_recursion,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    Ok((eq_output, ep_output))
}

fn build_kagemusha_step_eq_circuit_v5(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
    ep_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    mode: KagemushaStepPublicModeV4,
    stage: KagemushaCircuitBuilderStageV5<'_>,
) -> Result<KagemushaStepEqCircuitV4, String> {
    let layout = validate_kagemusha_step_witness_v4(
        witness,
        &step_eq_params,
        step_ep_params,
        stage.requires_break_points(),
    )?;
    let mut step_eq = kagemusha_base_builder_for_stage_v5::<Fp>(&step_eq_params, stage)?;
    let values = witness.public_inputs.instance_column::<Fp>(
        witness.proof_step_count,
        &step_eq_params,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let public = assign_kagemusha_public_mode_v4(&mut step_eq, values, &layout, mode)?;
    let range = step_eq.range_chip();
    let mut sha_jobs = KagemushaSha256JobsV4::default();
    let mut dense_jobs = KagemushaDenseMsmJobsV5::default();
    let semantic_values = witness.public_inputs.private_semantic_column::<Fp>();
    let semantic_len = semantic_values.len();
    let semantic = step_eq.main(0).assign_witnesses(semantic_values);
    let bindings = constrain_kagemusha_common_transition(
        step_eq.main(0),
        &range,
        &mut sha_jobs,
        &semantic,
        semantic_len,
    )?;
    constrain_kagemusha_compact_eq_header_v5(step_eq.main(0), &range, &public, &semantic)?;
    constrain_kagemusha_eq_secure_relations_v4(
        step_eq.main(0),
        &range,
        &bindings,
        witness.secure,
        witness.output_membership,
    )?;
    constrain_kagemusha_parity_scalar_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &mut step_eq,
        &mut sha_jobs,
        &public,
        KagemushaPastaCycleParityV1::StepEq,
        &step_eq_params,
        &layout,
        witness.step_eq_recursion,
        true,
    )?;
    constrain_kagemusha_reciprocal_output_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &mut step_eq,
        &mut sha_jobs,
        &mut dense_jobs,
        &public,
        &layout,
        ep_output,
    )?;
    match stage {
        KagemushaCircuitBuilderStageV5::Keygen => {
            validate_kagemusha_populated_builder_fit_v5(&mut step_eq, &step_eq_params, "StepEq")?;
        }
        KagemushaCircuitBuilderStageV5::Prover(break_points) => {
            validate_kagemusha_witness_builder_break_points_v5(
                &step_eq,
                &step_eq_params,
                break_points,
                "StepEq",
            )?;
        }
    }
    Ok(KagemushaStepEqCircuitV4 {
        params: step_eq_params,
        builder: step_eq,
        sha_jobs,
        dense_jobs,
    })
}

fn build_kagemusha_step_ep_circuit_v5(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: KagemushaStepCircuitParamsV4,
    eq_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    mode: KagemushaStepPublicModeV4,
    stage: KagemushaCircuitBuilderStageV5<'_>,
) -> Result<KagemushaStepEpCircuitV4, String> {
    let layout = validate_kagemusha_step_witness_v4(
        witness,
        step_eq_params,
        &step_ep_params,
        stage.requires_break_points(),
    )?;
    let mut step_ep = kagemusha_base_builder_for_stage_v5::<Fq>(&step_ep_params, stage)?;
    let values = witness.public_inputs.instance_column::<Fq>(
        witness.proof_step_count,
        &step_ep_params,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let public = assign_kagemusha_public_mode_v4(&mut step_ep, values, &layout, mode)?;
    let mut sha_jobs = KagemushaSha256JobsV4::default();
    let mut dense_jobs = KagemushaDenseMsmJobsV5::default();
    constrain_kagemusha_parity_scalar_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &mut step_ep,
        &mut sha_jobs,
        &public,
        KagemushaPastaCycleParityV1::StepEp,
        &step_ep_params,
        &layout,
        witness.step_ep_recursion,
        true,
    )?;
    constrain_kagemusha_reciprocal_output_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &mut step_ep,
        &mut sha_jobs,
        &mut dense_jobs,
        &public,
        &layout,
        eq_output,
    )?;
    match stage {
        KagemushaCircuitBuilderStageV5::Keygen => {
            validate_kagemusha_populated_builder_fit_v5(&mut step_ep, &step_ep_params, "StepEp")?;
        }
        KagemushaCircuitBuilderStageV5::Prover(break_points) => {
            validate_kagemusha_witness_builder_break_points_v5(
                &step_ep,
                &step_ep_params,
                break_points,
                "StepEp",
            )?;
        }
    }
    Ok(KagemushaStepEpCircuitV4 {
        params: step_ep_params,
        builder: step_ep,
        sha_jobs,
        dense_jobs,
    })
}

fn build_kagemusha_step_eq_circuit_sequential_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
    mode: KagemushaStepPublicModeV4,
    break_points: Option<&[Vec<usize>]>,
    reciprocal_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
) -> Result<
    (
        KagemushaStepEqCircuitV4,
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    ),
    String,
> {
    let output = collect_kagemusha_scalar_audits_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        witness.public_inputs,
        witness.proof_step_count,
        &step_eq_params,
        witness.step_eq_recursion,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let break_point_wire = break_points
        .map(|points| kagemusha_break_points_to_wire_v5(points.to_vec(), &step_eq_params))
        .transpose()?;
    let stage = match break_point_wire.as_deref() {
        Some(points) => KagemushaCircuitBuilderStageV5::Prover(points),
        None => KagemushaCircuitBuilderStageV5::Keygen,
    };
    let circuit = build_kagemusha_step_eq_circuit_v5(
        witness,
        step_eq_params,
        step_ep_params,
        reciprocal_output,
        mode,
        stage,
    )?;
    Ok((circuit, output))
}

fn build_kagemusha_step_ep_circuit_sequential_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: KagemushaStepCircuitParamsV4,
    mode: KagemushaStepPublicModeV4,
    break_points: Option<&[Vec<usize>]>,
    reciprocal_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
) -> Result<
    (
        KagemushaStepEpCircuitV4,
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    ),
    String,
> {
    let output = collect_kagemusha_scalar_audits_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        witness.public_inputs,
        witness.proof_step_count,
        &step_ep_params,
        witness.step_ep_recursion,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let break_point_wire = break_points
        .map(|points| kagemusha_break_points_to_wire_v5(points.to_vec(), &step_ep_params))
        .transpose()?;
    let stage = match break_point_wire.as_deref() {
        Some(points) => KagemushaCircuitBuilderStageV5::Prover(points),
        None => KagemushaCircuitBuilderStageV5::Keygen,
    };
    let circuit = build_kagemusha_step_ep_circuit_v5(
        witness,
        step_eq_params,
        step_ep_params,
        reciprocal_output,
        mode,
        stage,
    )?;
    Ok((circuit, output))
}

fn create_augmented_eq_proof_v4<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit: C,
    instances: &[Vec<Fp>],
) -> Result<
    (
        Vec<u8>,
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    ),
    String,
>
where
    C: halo2_proofs::plonk::Circuit<Fp>,
{
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EqAffine},
        plonk::{create_proof_consuming, verify_proof},
        poly::ipa::commitment::IPACommitmentScheme,
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    if instances.is_empty() || instances.iter().any(Vec::is_empty) {
        return Err("Kagemusha V4 Eq proof instances are empty".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let columns = instances.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let proofs_instances: [&[&[Fp]]; 1] = [columns.as_slice()];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    let verifying_key = create_proof_consuming::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceProverIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        circuit,
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Eq proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
    >(
        params,
        &verifying_key,
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Eq generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    Ok((proof, verifying_key))
}

fn create_augmented_ep_proof_v4<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit: C,
    instances: &[Vec<Fq>],
) -> Result<
    (
        Vec<u8>,
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    ),
    String,
>
where
    C: halo2_proofs::plonk::Circuit<Fq>,
{
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EpAffine},
        plonk::{create_proof_consuming, verify_proof},
        poly::ipa::commitment::IPACommitmentScheme,
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    if instances.is_empty() || instances.iter().any(Vec::is_empty) {
        return Err("Kagemusha V4 Ep proof instances are empty".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let columns = instances.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let proofs_instances: [&[&[Fq]]; 1] = [columns.as_slice()];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    let verifying_key = create_proof_consuming::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceProverIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        circuit,
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Ep proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
    >(
        params,
        &verifying_key,
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Ep generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    Ok((proof, verifying_key))
}

/// Raw, manifest-independent payloads emitted by the V4 artifact generator for
/// one Pasta parity.  The framing/export layer owns release identity and file
/// publication; this type contains only material derived by the circuit/key
/// generation process itself.
pub struct KagemushaGeneratedParityArtifactsV4 {
    /// Calibrated, inline circuit profile used to create every other payload.
    pub circuit_params: KagemushaStepCircuitParamsV4,
    /// Value-free compiled-protocol structure digest shared by bootstrap and
    /// final self protocols.
    pub compiled_protocol_structure_sha256: [u8; 32],
    /// Exact augmented Step-proof size measured during generation.
    pub step_proof_size_bytes: u32,
    /// Canonical `ParamsIPA::write` bytes.
    pub parameters: Vec<u8>,
    /// Number of processed proving-key bytes written directly to the caller's
    /// bounded staging sink.
    pub proving_key_size_bytes: u64,
    /// Processed verifier-key bytes.
    pub verifying_key: Vec<u8>,
    /// Canonical Norito bootstrap payload containing a genuine selector-zero
    /// proof under `verifying_key`.
    pub bootstrap_witness: Vec<u8>,
}

/// Owner-private, seekable spool containing one generated raw artifact.
///
/// Full release parameters and proving keys are intentionally parked here
/// between generation phases.  This keeps the generator's resident set
/// bounded to one Pasta parity without changing a single emitted byte.
#[must_use]
pub struct KagemushaGeneratedArtifactSpoolV4 {
    file: std::fs::File,
    size_bytes: u64,
    sha256: [u8; 32],
}

impl KagemushaGeneratedArtifactSpoolV4 {
    /// Exact number of raw payload bytes in this spool.
    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// SHA-256 of the exact raw payload bytes in this spool.
    #[must_use]
    pub const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    /// Copy the exact payload to `writer`, rejecting any truncated or changed
    /// backing file before returning.
    pub fn copy_to<W: std::io::Write + ?Sized>(&mut self, writer: &mut W) -> Result<(), String> {
        use std::io::{Read as _, Seek as _};

        use sha2::Digest as _;

        self.file
            .seek(std::io::SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind Kagemusha V4 artifact spool: {error}"))?;
        let mut remaining = self.size_bytes;
        let mut hasher = sha2::Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        while remaining != 0 {
            let requested = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("bounded spool chunk fits usize");
            let read = self
                .file
                .read(&mut buffer[..requested])
                .map_err(|error| format!("failed to read Kagemusha V4 artifact spool: {error}"))?;
            if read == 0 {
                return Err("Kagemusha V4 artifact spool is truncated".to_owned());
            }
            writer
                .write_all(&buffer[..read])
                .map_err(|error| format!("failed to copy Kagemusha V4 artifact spool: {error}"))?;
            hasher.update(&buffer[..read]);
            remaining -= u64::try_from(read).expect("read length fits u64");
        }
        let mut trailing = [0_u8; 1];
        if self
            .file
            .read(&mut trailing)
            .map_err(|error| format!("failed to finish Kagemusha V4 artifact spool: {error}"))?
            != 0
        {
            return Err("Kagemusha V4 artifact spool has trailing bytes".to_owned());
        }
        let actual: [u8; 32] = hasher.finalize().into();
        if actual != self.sha256 {
            return Err("Kagemusha V4 artifact spool digest changed".to_owned());
        }
        Ok(())
    }

    /// Materialize this one payload for tests.
    #[cfg(test)]
    pub fn into_bytes(mut self) -> Result<Vec<u8>, String> {
        let length = usize::try_from(self.size_bytes)
            .map_err(|_| "Kagemusha V4 artifact spool length does not fit usize".to_owned())?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| "failed to reserve Kagemusha V4 artifact payload".to_owned())?;
        self.copy_to(&mut bytes)?;
        if bytes.len() != length {
            return Err("Kagemusha V4 materialized artifact length mismatch".to_owned());
        }
        Ok(bytes)
    }
}

/// Lightweight profile metadata supplied with every streamed generator role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGeneratedParityProfileV4 {
    /// Pasta parity owning the emitted role.
    pub parity: KagemushaPastaCycleParityV1,
    /// Calibrated, inline circuit profile.
    pub circuit_params: KagemushaStepCircuitParamsV4,
    /// Value-free compiled-protocol structure digest.
    pub compiled_protocol_structure_sha256: [u8; 32],
    /// Exact augmented Step-proof size.
    pub step_proof_size_bytes: u32,
}

/// File-backed writer that deliberately presents an infallible `Write`
/// surface to Halo2's processed-key serializer. Several nested polynomial
/// serializers ignore I/O results; the first real file/size failure is saved
/// and returned by `finish` after serialization instead of being lost.
struct KagemushaInfallibleArtifactSpoolWriterV4 {
    file: std::fs::File,
    size_bytes: u64,
    sha256: sha2::Sha256,
    first_error: Option<String>,
}

impl KagemushaInfallibleArtifactSpoolWriterV4 {
    fn new(role: &str) -> Result<Self, String> {
        use sha2::Digest as _;

        Ok(Self {
            file: tempfile::tempfile().map_err(|error| {
                format!("failed to open owner-private Kagemusha V4 {role} spool: {error}")
            })?,
            size_bytes: 0,
            sha256: sha2::Sha256::new(),
            first_error: None,
        })
    }

    fn finish(mut self, role: &str) -> Result<KagemushaGeneratedArtifactSpoolV4, String> {
        use std::io::{Seek as _, Write as _};

        if let Some(error) = self.first_error.take() {
            return Err(error);
        }
        self.file
            .flush()
            .map_err(|error| format!("failed to flush Kagemusha V4 {role} spool: {error}"))?;
        let actual_len = self.file.metadata().map_err(|error| {
            format!("failed to inspect Kagemusha V4 {role} spool length: {error}")
        })?;
        if self.size_bytes == 0 || actual_len.len() != self.size_bytes {
            return Err(format!("Kagemusha V4 {role} spool length mismatch"));
        }
        self.file
            .seek(std::io::SeekFrom::Start(0))
            .map_err(|error| format!("failed to seal Kagemusha V4 {role} spool: {error}"))?;
        use sha2::Digest as _;
        Ok(KagemushaGeneratedArtifactSpoolV4 {
            file: self.file,
            size_bytes: self.size_bytes,
            sha256: self.sha256.finalize().into(),
        })
    }

    fn remember_error(&mut self, error: String) {
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }
}

impl std::io::Write for KagemushaInfallibleArtifactSpoolWriterV4 {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.first_error.is_none() {
            let next_len = self
                .size_bytes
                .checked_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
            match next_len {
                Some(next_len)
                    if next_len
                        <= iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4 =>
                {
                    if let Err(error) = self.file.write_all(bytes) {
                        self.remember_error(format!(
                            "failed to write owner-private Kagemusha V4 artifact spool: {error}"
                        ));
                    } else {
                        use sha2::Digest as _;
                        self.sha256.update(bytes);
                        self.size_bytes = next_len;
                    }
                }
                _ => self.remember_error(
                    "Kagemusha V4 generated artifact exceeds its explicit file bound".to_owned(),
                ),
            }
        }
        // Halo2's serializer assumes several nested writes cannot fail. The
        // real error is retained above and returned by `finish`.
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.first_error.is_none()
            && let Err(error) = self.file.flush()
        {
            self.remember_error(format!(
                "failed to flush owner-private Kagemusha V4 artifact spool: {error}"
            ));
        }
        Ok(())
    }
}

fn kagemusha_generated_spool_from_bytes_v4(
    role: &str,
    bytes: &[u8],
) -> Result<KagemushaGeneratedArtifactSpoolV4, String> {
    use std::io::Write as _;

    let mut writer = KagemushaInfallibleArtifactSpoolWriterV4::new(role)?;
    writer
        .write_all(bytes)
        .expect("Kagemusha V4 artifact spool writer is infallible");
    writer.finish(role)
}

/// Complete raw Eq/Ep output of one V4 generation run.
pub struct KagemushaGeneratedPastaCycleArtifactsV4 {
    /// StepEq/Vesta material.
    pub step_eq: KagemushaGeneratedParityArtifactsV4,
    /// StepEp/Pallas material.
    pub step_ep: KagemushaGeneratedParityArtifactsV4,
    /// Canonical live selector-one pair used solely to measure the opaque ABI
    /// payload.  It is terminally verified before being returned.
    pub measured_live_pair_bytes: Vec<u8>,
}

struct KagemushaGenerationCalibrationV4 {
    public_inputs: KagemushaPastaCyclePublicInputsV4,
    secure: super::confidential_v2::KagemushaStepSecureWitnessV3,
    output_membership: super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
}

fn kagemusha_calibration_exact_limbs_v4(bytes: [u8; 32]) -> [u32; 8] {
    std::array::from_fn(|index| {
        u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("32-byte calibration value has exact limbs"),
        )
    })
}

fn kagemusha_calibration_scalar_v4(bytes: [u8; 32], role: &str) -> Result<Fp, String> {
    Option::<Fp>::from(Fp::from_repr(bytes.into()))
        .ok_or_else(|| format!("Kagemusha V4 calibration {role} is not canonical Fp"))
}

fn kagemusha_calibration_put_digest_v4(
    fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
    start: usize,
    bytes: [u8; 32],
) -> Result<(), String> {
    let target = fields
        .get_mut(start..start + 4)
        .ok_or_else(|| "Kagemusha V4 calibration digest range is invalid".to_owned())?;
    for (field, chunk) in target.iter_mut().zip(bytes.chunks_exact(8)) {
        *field = Fp::from(u64::from_le_bytes(
            chunk
                .try_into()
                .expect("32-byte calibration digest has exact chunks"),
        ));
    }
    Ok(())
}

fn kagemusha_calibration_put_field_v4(
    fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
    index: usize,
    bytes: [u8; 32],
    role: &str,
) -> Result<(), String> {
    *fields
        .get_mut(index)
        .ok_or_else(|| format!("Kagemusha V4 calibration {role} index is invalid"))? =
        kagemusha_calibration_scalar_v4(bytes, role)?;
    Ok(())
}

fn kagemusha_calibration_membership_path_v4(
    path: super::confidential_v2::ConfidentialMerklePathV2,
) -> iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
    let (siblings, directions, _, root) = path.into_parts();
    iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
        siblings,
        directions,
        root,
    }
}

/// Build one deterministic, satisfying initialization relation for key
/// calibration and the measured live pair.  None of these values is an
/// authenticated release identity: the exporter supplies that layer after the
/// generated payloads and sizes are known.
fn kagemusha_generation_calibration_v4(
    step_eq_compiled_protocol_sha256: [u8; 32],
    step_ep_compiled_protocol_sha256: [u8; 32],
) -> Result<KagemushaGenerationCalibrationV4, String> {
    use halo2_proofs::halo2curves::pasta::Fp;
    use iroha_data_model::ChainId;

    use super::{confidential_v2, kagemusha_v2};

    const ASSET_DEFINITION: &str = "kagemusha-fixed-padding#internal";
    const CHAIN: &str = "kagemusha-fixed-padding-chain";
    const PAYER: &str = "kagemusha-fixed-padding-payer";
    const AMOUNT: u128 = 1;
    const ASSET_SCALE: u32 = 0;
    const LEAF_INDEX: u32 = 0;

    let chain_id = ChainId::from(CHAIN);
    let spend_key = [0x46_u8; 32];
    let rho = [0x47_u8; 32];
    let operation_id = [0x48_u8; 32];
    let diversifier = {
        let repr = Fp::from(4).to_repr();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(repr.as_ref());
        bytes
    };

    let empty_path = confidential_v2::compute_confidential_merkle_path_v3(&[], 0)?;
    let secure = confidential_v2::prepare_kagemusha_step_topup_witness_v3(
        &chain_id,
        ASSET_DEFINITION,
        PAYER,
        operation_id,
        AMOUNT,
        ASSET_SCALE,
        &spend_key,
        rho,
        diversifier,
        LEAF_INDEX,
        &empty_path,
    )?;

    let asset_tag = confidential_v2::derive_confidential_asset_tag_v3(ASSET_DEFINITION)?;
    let chain_tag = confidential_v2::derive_confidential_chain_tag_v3(CHAIN)?;
    let payer_tag = confidential_v2::derive_kagemusha_topup_payer_tag_v3(PAYER)?;
    let operation_tag = confidential_v2::derive_kagemusha_topup_operation_tag_v3(&operation_id)?;
    let owner_tag = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &spend_key,
        diversifier,
    )?;
    let output_commitment =
        confidential_v2::derive_confidential_note_v3(asset_tag, AMOUNT, rho, owner_tag)?;
    let spend_nullifier =
        confidential_v2::derive_confidential_nullifier_v3(&spend_key, rho, asset_tag, chain_tag)?;
    let initial_root = confidential_v2::compute_confidential_root_v3(&[])?;
    let final_commitments = [output_commitment];
    let final_root = confidential_v2::compute_confidential_root_v3(&final_commitments)?;
    if empty_path.root != initial_root {
        return Err("Kagemusha V4 calibration empty path/root mismatch".to_owned());
    }

    let recipient_update_path = kagemusha_calibration_membership_path_v4(empty_path.clone());
    let recipient_membership_path = kagemusha_calibration_membership_path_v4(
        confidential_v2::compute_confidential_merkle_path_v3(&final_commitments, 0)?,
    );
    let dummy_leaf_index = 1_u32;
    let dummy_path = kagemusha_calibration_membership_path_v4(
        confidential_v2::compute_confidential_merkle_path_v3(
            &final_commitments,
            usize::try_from(dummy_leaf_index)
                .map_err(|_| "Kagemusha V4 calibration dummy index does not fit usize")?,
        )?,
    );
    let output_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Init,
        initial_root,
        final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: output_commitment,
            leaf_index: LEAF_INDEX,
            update_path: recipient_update_path,
            membership_path: recipient_membership_path,
        }),
        change: None,
        dummy_leaf_index,
        dummy_path,
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(output_membership.clone())?;

    let statement_digest = [0x11_u8; 32];
    let topup_anchor_digest = [0x31_u8; 32];
    let manifest_sha256 = [0x41_u8; 32];
    let verifier_key_id_digest = [0x51_u8; 32];
    let mut fields = [Fp::ZERO; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4];
    fields[kagemusha_v2::I_LAYOUT_VERSION] = Fp::ONE;
    fields[kagemusha_v2::I_PROOF_STEP_COUNT] = Fp::ONE;
    fields[kagemusha_v2::I_ASSET_SCALE] = Fp::from(u64::from(ASSET_SCALE));
    for index in [
        kagemusha_v2::I_INPUT_SCALE,
        kagemusha_v2::I_TRANSFER_SCALE,
        kagemusha_v2::I_RECIPIENT_SCALE,
        kagemusha_v2::I_CURRENT_SCALE,
    ] {
        fields[index] = Fp::from(u64::from(ASSET_SCALE));
    }
    fields[kagemusha_v2::I_RECORD_OUTPUT_COUNT] = Fp::ONE;
    fields[kagemusha_v2::I_TRANSFER_OUTPUT_COUNT] = Fp::ONE;
    for index in [
        kagemusha_v2::I_CURRENT_AMOUNT_LO,
        kagemusha_v2::I_INPUT_AMOUNT_LO,
        kagemusha_v2::I_TRANSFER_AMOUNT_LO,
        kagemusha_v2::I_RECIPIENT_AMOUNT_LO,
    ] {
        fields[index] = Fp::from_u128(AMOUNT);
    }
    for (index, bytes, role) in [
        (kagemusha_v2::I_INITIAL_ROOT, initial_root, "initial root"),
        (kagemusha_v2::I_FINAL_ROOT, final_root, "final root"),
        (
            kagemusha_v2::I_RECORD_ROOT_BEFORE,
            initial_root,
            "record root before",
        ),
        (
            kagemusha_v2::I_RECORD_ROOT_AFTER,
            final_root,
            "record root after",
        ),
        (kagemusha_v2::I_TRANSFER_ROOT, final_root, "transfer root"),
        (
            kagemusha_v2::I_CURRENT_COMMITMENT,
            output_commitment,
            "current commitment",
        ),
        (
            kagemusha_v2::I_CURRENT_NULLIFIER,
            spend_nullifier,
            "current nullifier",
        ),
        (
            kagemusha_v2::I_RECIPIENT_COMMITMENT,
            output_commitment,
            "recipient commitment",
        ),
        (
            kagemusha_v2::I_RECIPIENT_NULLIFIER,
            spend_nullifier,
            "recipient nullifier",
        ),
        (
            kagemusha_v2::I_RECORD_OUTPUT_0,
            output_commitment,
            "record output",
        ),
        (
            kagemusha_v2::I_TRANSFER_OUTPUT_0,
            output_commitment,
            "transfer output",
        ),
        (kagemusha_v2::I_ASSET_TAG, asset_tag, "asset tag"),
        (kagemusha_v2::I_CHAIN_TAG, chain_tag, "chain tag"),
    ] {
        kagemusha_calibration_put_field_v4(&mut fields, index, bytes, role)?;
    }
    for (index, bytes) in [
        (kagemusha_v2::I_STATEMENT_DIGEST, statement_digest),
        (kagemusha_v2::I_RECIPIENT_REQUEST_DIGEST, payer_tag),
        (kagemusha_v2::I_OPERATION_ID, operation_tag),
        (kagemusha_v2::I_BRANCH_LINEAGE_ROOT, topup_anchor_digest),
        (kagemusha_v2::I_TOPUP_OPERATION_ID, operation_id),
        (kagemusha_v2::I_ARTIFACT_MANIFEST_SHA256, manifest_sha256),
        (kagemusha_v2::I_TOPUP_RECEIPT_DIGEST, topup_anchor_digest),
        (kagemusha_v2::I_TOPUP_ANCHOR_DIGEST, topup_anchor_digest),
        (
            kagemusha_v2::I_VERIFIER_KEY_ID_DIGEST,
            verifier_key_id_digest,
        ),
    ] {
        kagemusha_calibration_put_digest_v4(&mut fields, index, bytes)?;
    }
    fields[kagemusha_v2::I_TOPUP_ANCHOR_COUNT] = Fp::ONE;
    let operation = KagemushaStepOperationVectorV4::from_fields(fields);

    let mut result_state =
        vec![0_u32; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
    result_state[kagemusha_v2::S_VERSION] =
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
    result_state[kagemusha_v2::S_CHAIN_TAG..kagemusha_v2::S_CHAIN_TAG + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(chain_tag));
    result_state[kagemusha_v2::S_ASSET_TAG..kagemusha_v2::S_ASSET_TAG + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(asset_tag));
    result_state[kagemusha_v2::S_ASSET_SCALE] = ASSET_SCALE;
    result_state[kagemusha_v2::S_FINAL_ROOT..kagemusha_v2::S_FINAL_ROOT + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(final_root));
    result_state[kagemusha_v2::S_TOPUP_ANCHOR_COUNT] = 1;
    result_state[kagemusha_v2::S_TOPUP_ANCHORS..kagemusha_v2::S_TOPUP_ANCHORS + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(operation_id));
    result_state[kagemusha_v2::S_TOPUP_ANCHORS + 8..kagemusha_v2::S_TOPUP_ANCHORS + 16]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(topup_anchor_digest));
    result_state[kagemusha_v2::S_PROOF_STEP_COUNT] = 1;
    result_state[kagemusha_v2::S_CURRENT_COMMITMENT..kagemusha_v2::S_CURRENT_COMMITMENT + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(output_commitment));
    result_state[kagemusha_v2::S_CURRENT_NULLIFIER..kagemusha_v2::S_CURRENT_NULLIFIER + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(spend_nullifier));
    for (target, limb) in result_state
        [kagemusha_v2::S_CURRENT_AMOUNT..kagemusha_v2::S_CURRENT_AMOUNT + 4]
        .iter_mut()
        .zip(AMOUNT.to_le_bytes().chunks_exact(4))
    {
        *target = u32::from_le_bytes(
            limb.try_into()
                .expect("u128 calibration amount has exact limbs"),
        );
    }
    result_state[kagemusha_v2::S_CURRENT_SCALE] = ASSET_SCALE;
    result_state[kagemusha_v2::S_BRANCH_CLAIM_COUNT] = 1;
    result_state[kagemusha_v2::S_BRANCH_CLAIMS..kagemusha_v2::S_BRANCH_CLAIMS + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(topup_anchor_digest));
    result_state
        [kagemusha_v2::S_ARTIFACT_MANIFEST_SHA256..kagemusha_v2::S_ARTIFACT_MANIFEST_SHA256 + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(manifest_sha256));
    result_state[kagemusha_v2::S_VERIFIER_KEY_ID..kagemusha_v2::S_VERIFIER_KEY_ID + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(
            verifier_key_id_digest,
        ));

    let public_inputs = KagemushaPastaCyclePublicInputsV4 {
        public_statement_digest: kagemusha_calibration_exact_limbs_v4(statement_digest),
        operation,
        parent_count: 0,
        parent_states: std::array::from_fn(|_| {
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2]
        }),
        result_state,
        manifest_sha256: kagemusha_calibration_exact_limbs_v4(manifest_sha256),
        step_eq_compiled_protocol_sha256: kagemusha_sha256_public_words(
            step_eq_compiled_protocol_sha256,
        ),
        step_ep_compiled_protocol_sha256: kagemusha_sha256_public_words(
            step_ep_compiled_protocol_sha256,
        ),
        parent_eq_lineage_accumulator: None,
        parent_ep_lineage_accumulator: None,
        parent_eq_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
        parent_ep_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
        live_selector: KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
    };

    Ok(KagemushaGenerationCalibrationV4 {
        public_inputs,
        secure,
        output_membership,
    })
}

struct KagemushaEqBootstrapSeedV4 {
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    structure_sha256: [u8; 32],
    protocol_sha256: [u8; 32],
    proof: Vec<u8>,
    current: snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EqAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
}

struct KagemushaEpBootstrapSeedV4 {
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    structure_sha256: [u8; 32],
    protocol_sha256: [u8; 32],
    proof: Vec<u8>,
    current: snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EpAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
}

fn kagemusha_eq_bootstrap_seed_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaEqBootstrapSeedV4, String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 Eq bootstrap public length does not fit usize".to_owned())?;
    let target = KagemushaUniversalProtocolTargetV1 {
        base_circuit_params: kagemusha_base_circuit_params_v4(circuit_params)?,
        instance_column_lengths: vec![public_len],
    };
    let circuit = KagemushaProtocolBootstrapCircuit::<Fp> {
        params: target.base_circuit_params.clone(),
        marker: std::marker::PhantomData,
    };
    let proving_key = kagemusha_bootstrap_proving_key_v1(params, &target, &circuit)
        .map_err(|error| format!("failed to generate Kagemusha V4 Eq bootstrap PK: {error}"))?;
    let instances = vec![vec![Fp::ZERO; public_len]];
    let (proof, verifying_key) =
        create_augmented_eq_proof_v4(params, proving_key, circuit, &instances)?;
    let current =
        succinct_verify_step_eq_instances(params, &verifying_key, &proof, &instances, proof.len())?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    let structure_sha256 = kagemusha_compiled_protocol_structure_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    Ok(KagemushaEqBootstrapSeedV4 {
        protocol,
        structure_sha256,
        protocol_sha256,
        proof,
        current,
    })
}

fn kagemusha_ep_bootstrap_seed_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaEpBootstrapSeedV4, String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 Ep bootstrap public length does not fit usize".to_owned())?;
    let target = KagemushaUniversalProtocolTargetV1 {
        base_circuit_params: kagemusha_base_circuit_params_v4(circuit_params)?,
        instance_column_lengths: vec![public_len],
    };
    let circuit = KagemushaProtocolBootstrapCircuit::<Fq> {
        params: target.base_circuit_params.clone(),
        marker: std::marker::PhantomData,
    };
    let proving_key = kagemusha_bootstrap_proving_key_v1(params, &target, &circuit)
        .map_err(|error| format!("failed to generate Kagemusha V4 Ep bootstrap PK: {error}"))?;
    let instances = vec![vec![Fq::ZERO; public_len]];
    let (proof, verifying_key) =
        create_augmented_ep_proof_v4(params, proving_key, circuit, &instances)?;
    let current =
        succinct_verify_step_ep_instances(params, &verifying_key, &proof, &instances, proof.len())?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    let structure_sha256 = kagemusha_compiled_protocol_structure_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    Ok(KagemushaEpBootstrapSeedV4 {
        protocol,
        structure_sha256,
        protocol_sha256,
        proof,
        current,
    })
}

fn kagemusha_eq_seed_bootstrap_payload_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    seed: &KagemushaEqBootstrapSeedV4,
) -> Result<KagemushaStepBootstrapV4, String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    if seed.proof.len()
        != usize::try_from(circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 Eq calibrated proof size changed".to_owned());
    }
    let (post_proof_fold, _) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let (branch_merge_fold, _) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let bootstrap = KagemushaStepBootstrapV4 {
        version: KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        circuit_params_sha256: circuit_params
            .sha256()
            .map_err(|error| format!("failed to identify Kagemusha V4 Eq params: {error}"))?,
        compiled_protocol_structure_sha256: seed.structure_sha256,
        bootstrap_compiled_protocol_sha256: seed.protocol_sha256,
        circuit_break_points: Vec::new(),
        parent_slot: KagemushaStepBootstrapParentSlotV4 {
            instances: vec![vec![
                0;
                usize::try_from(layout.instance_column_limbs).map_err(
                    |_| { "Kagemusha V4 Eq bootstrap public length does not fit usize".to_owned() }
                )?
            ]],
            ordinary_proof_bytes: seed.proof.clone(),
            carried_lineage: KagemushaIpaAccumulatorWireV4::from_eq(
                &seed.current,
                circuit_params.k,
            )?,
            post_proof_fold,
        },
        branch_merge_fold,
    };
    bootstrap.validate_provisional_bootstrap_protocol(
        circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        seed.structure_sha256,
        &seed.protocol,
    )?;
    Ok(bootstrap)
}

fn kagemusha_ep_seed_bootstrap_payload_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    seed: &KagemushaEpBootstrapSeedV4,
) -> Result<KagemushaStepBootstrapV4, String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    if seed.proof.len()
        != usize::try_from(circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Ep proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 Ep calibrated proof size changed".to_owned());
    }
    let (post_proof_fold, _) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let (branch_merge_fold, _) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let bootstrap = KagemushaStepBootstrapV4 {
        version: KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        circuit_params_sha256: circuit_params
            .sha256()
            .map_err(|error| format!("failed to identify Kagemusha V4 Ep params: {error}"))?,
        compiled_protocol_structure_sha256: seed.structure_sha256,
        bootstrap_compiled_protocol_sha256: seed.protocol_sha256,
        circuit_break_points: Vec::new(),
        parent_slot: KagemushaStepBootstrapParentSlotV4 {
            instances: vec![vec![
                0;
                usize::try_from(layout.instance_column_limbs).map_err(
                    |_| { "Kagemusha V4 Ep bootstrap public length does not fit usize".to_owned() }
                )?
            ]],
            ordinary_proof_bytes: seed.proof.clone(),
            carried_lineage: KagemushaIpaAccumulatorWireV4::from_ep(
                &seed.current,
                circuit_params.k,
            )?,
            post_proof_fold,
        },
        branch_merge_fold,
    };
    bootstrap.validate_provisional_bootstrap_protocol(
        circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        seed.structure_sha256,
        &seed.protocol,
    )?;
    Ok(bootstrap)
}

fn kagemusha_eq_recursion_from_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    structure_sha256: [u8; 32],
    bootstrap: &KagemushaStepBootstrapV4,
    parent_validation: KagemushaBootstrapParentValidationV4,
) -> Result<KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    Ok(KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_eq_succinct_vk_v4(params)?,
        compiled_parent_protocol: protocol,
        fixed_structure_sha256: structure_sha256,
        parents: [
            bootstrap.step_eq_parent_internal(
                circuit_params,
                structure_sha256,
                0,
                parent_validation,
            )?,
            bootstrap.step_eq_parent_internal(
                circuit_params,
                structure_sha256,
                1,
                parent_validation,
            )?,
        ],
        branch_merge_fold: bootstrap.branch_merge_fold.clone(),
    })
}

fn kagemusha_ep_recursion_from_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    structure_sha256: [u8; 32],
    bootstrap: &KagemushaStepBootstrapV4,
    parent_validation: KagemushaBootstrapParentValidationV4,
) -> Result<KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    Ok(KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_ep_succinct_vk_v4(params)?,
        compiled_parent_protocol: protocol,
        fixed_structure_sha256: structure_sha256,
        parents: [
            bootstrap.step_ep_parent_internal(
                circuit_params,
                structure_sha256,
                0,
                parent_validation,
            )?,
            bootstrap.step_ep_parent_internal(
                circuit_params,
                structure_sha256,
                1,
                parent_validation,
            )?,
        ],
        branch_merge_fold: bootstrap.branch_merge_fold.clone(),
    })
}

fn kagemusha_eq_parameters_bytes_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::poly::commitment::Params as _;

    let mut bytes = Vec::new();
    params
        .write(&mut bytes)
        .map_err(|error| format!("failed to encode Kagemusha V4 Eq parameters: {error}"))?;
    Ok(bytes)
}

fn kagemusha_ep_parameters_bytes_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::poly::commitment::Params as _;

    let mut bytes = Vec::new();
    params
        .write(&mut bytes)
        .map_err(|error| format!("failed to encode Kagemusha V4 Ep parameters: {error}"))?;
    Ok(bytes)
}

fn emit_kagemusha_generated_parity_v5<F>(
    artifacts: KagemushaGeneratedParityArtifactsV4,
    mut proving_key: KagemushaGeneratedArtifactSpoolV4,
    parity: KagemushaPastaCycleParityV1,
    emit: &mut F,
) -> Result<(), String>
where
    F: FnMut(
        &KagemushaGeneratedParityProfileV4,
        KagemushaPastaCycleArtifactKindV4,
        &mut KagemushaGeneratedArtifactSpoolV4,
    ) -> Result<(), String>,
{
    use KagemushaPastaCycleArtifactKindV4 as Kind;

    if proving_key.size_bytes() != artifacts.proving_key_size_bytes {
        return Err("Kagemusha V5 streamed proving-key size changed".to_owned());
    }
    let profile = KagemushaGeneratedParityProfileV4 {
        parity,
        circuit_params: artifacts.circuit_params,
        compiled_protocol_structure_sha256: artifacts.compiled_protocol_structure_sha256,
        step_proof_size_bytes: artifacts.step_proof_size_bytes,
    };
    let mut parameters =
        kagemusha_generated_spool_from_bytes_v4("parameters", &artifacts.parameters)?;
    let mut verifying_key =
        kagemusha_generated_spool_from_bytes_v4("verifying key", &artifacts.verifying_key)?;
    let mut bootstrap =
        kagemusha_generated_spool_from_bytes_v4("bootstrap", &artifacts.bootstrap_witness)?;
    emit(&profile, Kind::ParamsIpa, &mut parameters)?;
    emit(&profile, Kind::ProvingKey, &mut proving_key)?;
    emit(&profile, Kind::VerifyingKey, &mut verifying_key)?;
    emit(&profile, Kind::BootstrapWitness, &mut bootstrap)
}

/// Generate and stream the complete canonical Eq/Ep artifact inventory.
pub fn generate_kagemusha_pasta_cycle_artifacts_streaming_v4<F>(
    step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: KagemushaStepCircuitParamsV4,
    emit: F,
) -> Result<Vec<u8>, String>
where
    F: FnMut(
        &KagemushaGeneratedParityProfileV4,
        KagemushaPastaCycleArtifactKindV4,
        &mut KagemushaGeneratedArtifactSpoolV4,
    ) -> Result<(), String>,
{
    generate_kagemusha_pasta_cycle_artifacts_streaming_with_progress_v4(
        step_eq_circuit_params,
        step_ep_circuit_params,
        emit,
        |_| Ok(()),
    )
}

/// Generate and stream the complete artifact inventory while reporting
/// resource-supervisor lifecycle boundaries.
///
/// Callback failures abort before the next heavyweight or publication phase.
/// The compact V5 generator owns its detailed Eq/Ep scheduling internally, so
/// the public progress surface reports the enclosing core and parity-emission
/// boundaries without weakening the one-process supervisor permit.
pub fn generate_kagemusha_pasta_cycle_artifacts_streaming_with_progress_v4<F, P>(
    step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: KagemushaStepCircuitParamsV4,
    mut emit: F,
    mut progress_callback: P,
) -> Result<Vec<u8>, String>
where
    F: FnMut(
        &KagemushaGeneratedParityProfileV4,
        KagemushaPastaCycleArtifactKindV4,
        &mut KagemushaGeneratedArtifactSpoolV4,
    ) -> Result<(), String>,
    P: FnMut(&'static str) -> Result<(), String>,
{
    let mut report = |phase| {
        progress_callback(phase).map_err(|error| {
            format!("Kagemusha V5 generator progress callback failed at {phase}: {error}")
        })
    };
    report("kagemusha-v5.generator.begin")?;
    let supervisor_permit = claim_kagemusha_generation_supervisor_permit_v4()?;
    let mut step_eq_proving_key = KagemushaInfallibleArtifactSpoolWriterV4::new("Eq proving key")?;
    let mut step_ep_proving_key = KagemushaInfallibleArtifactSpoolWriterV4::new("Ep proving key")?;
    report("kagemusha-v5.generator.core.begin")?;
    let generated = generate_kagemusha_pasta_cycle_artifacts_v4(
        step_eq_circuit_params,
        step_ep_circuit_params,
        supervisor_permit,
        &mut step_eq_proving_key,
        &mut step_ep_proving_key,
    )?;
    report("kagemusha-v5.generator.core.complete")?;
    let step_eq_proving_key = step_eq_proving_key.finish("Eq proving key")?;
    let step_ep_proving_key = step_ep_proving_key.finish("Ep proving key")?;
    let KagemushaGeneratedPastaCycleArtifactsV4 {
        step_eq,
        step_ep,
        measured_live_pair_bytes,
    } = generated;
    report("kagemusha-v5.generator.eq-artifacts.emit.begin")?;
    emit_kagemusha_generated_parity_v5(
        step_eq,
        step_eq_proving_key,
        KagemushaPastaCycleParityV1::StepEq,
        &mut emit,
    )?;
    report("kagemusha-v5.generator.eq-artifacts.emit.complete")?;
    report("kagemusha-v5.generator.ep-artifacts.emit.begin")?;
    emit_kagemusha_generated_parity_v5(
        step_ep,
        step_ep_proving_key,
        KagemushaPastaCycleParityV1::StepEp,
        &mut emit,
    )?;
    report("kagemusha-v5.generator.ep-artifacts.emit.complete")?;
    report("kagemusha-v5.generator.complete")?;
    Ok(measured_live_pair_bytes)
}

/// Generate the complete Eq/Ep V4 artifact payload set from current source.
///
/// This is deliberately a two-stage fixed-point construction. A deterministic
/// universal BaseConfig proof supplies parseable disabled-parent transcripts
/// while the final self-recursive PK/VK are generated. The final PK then
/// creates a genuine selector-zero proof over the all-zero public column; its
/// current accumulator and both independent folds become the authenticated
/// bootstrap payload. Finally a selector-one initialization is proved and
/// terminally decided to measure the public opaque pair. A checked resource
/// preflight and the reviewed first-release profile gate run before either IPA
/// parameter set is allocated. Populated keygen circuits are consumed and
/// released immediately after synthesis and authenticated-breakpoint
/// extraction, before fixed or permutation key polynomials are assembled.
/// Each final processed proving key is streamed into its supplied staging sink,
/// then moved with its witness-only circuit into the consuming proof API. The
/// function never owns a proving-key byte vector and never keeps the Eq and Ep
/// proving keys resident together.
pub fn generate_kagemusha_pasta_cycle_artifacts_v4(
    step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: KagemushaStepCircuitParamsV4,
    supervisor_permit: KagemushaGenerationSupervisorPermitV4,
    step_eq_proving_key_sink: &mut (dyn std::io::Write + Send),
    step_ep_proving_key_sink: &mut (dyn std::io::Write + Send),
) -> Result<KagemushaGeneratedPastaCycleArtifactsV4, String> {
    // Halo2 uses Rayon inside FFTs, quotient evaluation, and IPA commitments.
    // A one-process guard does not bound those per-worker scratch allocations.
    // Keep the outer lifecycle in a disposable one-worker pool so FFT and
    // quotient work remains bounded and its worker-local cache is released
    // when the attempt returns. Large MSMs alone dispatch behind process-wide
    // admission to Halo2's fixed two-worker window pool; its accumulator order
    // remains canonical while bucket storage and allocator caches stay bounded.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)
        .thread_name(|_| "kagemusha-v5-generator".to_owned())
        .build()
        .map_err(|error| format!("failed to build bounded Kagemusha worker pool: {error}"))?;
    pool.install(move || {
        generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(
            step_eq_circuit_params,
            step_ep_circuit_params,
            supervisor_permit,
            step_eq_proving_key_sink,
            step_ep_proving_key_sink,
        )
    })
}

fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(
    mut step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    mut step_ep_circuit_params: KagemushaStepCircuitParamsV4,
    _supervisor_permit: KagemushaGenerationSupervisorPermitV4,
    step_eq_proving_key_sink: &mut (dyn std::io::Write + Send),
    step_ep_proving_key_sink: &mut (dyn std::io::Write + Send),
) -> Result<KagemushaGeneratedPastaCycleArtifactsV4, String> {
    use halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::{EpAffine, EqAffine},
        plonk::{keygen_pk_consuming_with, keygen_vk_consuming_with},
        poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };

    let preflight =
        preflight_kagemusha_generation_v4(&step_eq_circuit_params, &step_ep_circuit_params)?;
    debug_assert!(preflight.estimated_peak_bytes <= KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4);
    debug_assert!(
        preflight.estimated_peak_bytes <= KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5
    );
    let public_len = usize::try_from(preflight.layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 generator public length does not fit usize".to_owned())?;

    // `ParamsIPA::new` is a transparent, public-coin derivation: the vendored
    // Halo2 implementation hashes the public domain `Halo2-Parameters` and
    // indexed messages directly to curve points (with `[1]`/`[2]` for w/u).
    // It accepts no RNG or secret seed, so reproducibility exposes no known
    // discrete-log relation or toxic setup material.
    let step_eq_params = ParamsIPA::<EqAffine>::new(step_eq_circuit_params.k);
    let step_eq_seed = kagemusha_eq_bootstrap_seed_v4(&step_eq_params, &step_eq_circuit_params)?;
    // The two universal parameter sets are not needed together while their
    // parity-local bootstrap seeds are built.  Keeping both affine Eq vectors
    // live beside bootstrap key assembly was the physical k16 peak.  Retain
    // Eq temporarily in its canonical compressed representation, release the
    // larger in-memory form, and reconstruct it only after the Ep seed exists.
    let step_eq_parameter_spool = kagemusha_eq_parameters_bytes_v4(&step_eq_params)?;
    validate_kagemusha_generated_payload_size_v4(
        step_eq_parameter_spool.len(),
        "temporary Eq parameters",
    )?;
    drop(step_eq_params);

    let step_ep_params = ParamsIPA::<EpAffine>::new(step_ep_circuit_params.k);
    let step_ep_seed = kagemusha_ep_bootstrap_seed_v4(&step_ep_params, &step_ep_circuit_params)?;
    let step_eq_params = parse_kagemusha_params_v4::<EqAffine>(
        &step_eq_parameter_spool,
        step_eq_circuit_params.k,
        "temporary generated Eq",
    )?;
    drop(step_eq_parameter_spool);
    step_eq_circuit_params.max_parent_proof_bytes = u32::try_from(step_eq_seed.proof.len())
        .map_err(|_| "Kagemusha V4 Eq proof size does not fit u32".to_owned())?;
    step_ep_circuit_params.max_parent_proof_bytes = u32::try_from(step_ep_seed.proof.len())
        .map_err(|_| "Kagemusha V4 Ep proof size does not fit u32".to_owned())?;
    validate_kagemusha_circuit_params_v4(&step_eq_circuit_params)?;
    validate_kagemusha_circuit_params_v4(&step_ep_circuit_params)?;

    let mut step_eq_seed_bootstrap = kagemusha_eq_seed_bootstrap_payload_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        &step_eq_seed,
    )?;
    let mut step_ep_seed_bootstrap = kagemusha_ep_seed_bootstrap_payload_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        &step_ep_seed,
    )?;

    let keygen_calibration = kagemusha_generation_calibration_v4(
        step_eq_seed.protocol_sha256,
        step_ep_seed.protocol_sha256,
    )?;
    let step_eq_seed_recursion = kagemusha_eq_recursion_from_bootstrap_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        step_eq_seed.protocol.clone(),
        step_eq_seed.structure_sha256,
        &step_eq_seed_bootstrap,
        KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
    )?;
    let step_ep_seed_recursion = kagemusha_ep_recursion_from_bootstrap_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        step_ep_seed.protocol.clone(),
        step_ep_seed.structure_sha256,
        &step_ep_seed_bootstrap,
        KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
    )?;
    let keygen_witness = KagemushaStepWitnessV4 {
        public_inputs: &keygen_calibration.public_inputs,
        proof_step_count: 1,
        secure: &keygen_calibration.secure,
        output_membership: &keygen_calibration.output_membership,
        step_eq_recursion: &step_eq_seed_recursion,
        step_ep_recursion: &step_ep_seed_recursion,
        step_eq_bootstrap: Some(&step_eq_seed_bootstrap),
        step_ep_bootstrap: Some(&step_ep_seed_bootstrap),
    };
    let (keygen_eq_output, keygen_ep_output) = collect_kagemusha_step_scalar_audits_v5(
        &keygen_witness,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        false,
    )?;
    let compile_config = || kagemusha_ipa_compile_config_v4(public_len);
    let step_eq_keygen_circuit = build_kagemusha_step_eq_circuit_v5(
        &keygen_witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        &keygen_ep_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let (step_eq_verifying_key, step_eq_break_points) =
        keygen_vk_consuming_with(&step_eq_params, step_eq_keygen_circuit, |circuit| {
            kagemusha_break_points_to_wire_v5(
                circuit.builder.break_points(),
                &step_eq_circuit_params,
            )
        })
        .map_err(|error| {
            format_kagemusha_consuming_keygen_error_v5(
                error,
                "failed to generate Kagemusha V4 Eq VK",
            )
        })?;
    let step_eq_verifying_key_bytes = step_eq_verifying_key.to_bytes(SerdeFormat::Processed);
    validate_kagemusha_generated_payload_size_v4(
        step_eq_verifying_key_bytes.len(),
        "Eq verifying key",
    )?;
    let step_eq_final_protocol = snark_verifier::system::halo2::compile(
        &step_eq_params,
        &step_eq_verifying_key,
        compile_config(),
    );
    let step_eq_structure_sha256 = kagemusha_require_protocol_structure_v1(
        &step_eq_seed.protocol,
        &step_eq_final_protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let step_eq_final_protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &step_eq_final_protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    drop(step_eq_verifying_key);

    let step_ep_keygen_circuit = build_kagemusha_step_ep_circuit_v5(
        &keygen_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        &keygen_eq_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let (step_ep_verifying_key, step_ep_break_points) =
        keygen_vk_consuming_with(&step_ep_params, step_ep_keygen_circuit, |circuit| {
            kagemusha_break_points_to_wire_v5(
                circuit.builder.break_points(),
                &step_ep_circuit_params,
            )
        })
        .map_err(|error| {
            format_kagemusha_consuming_keygen_error_v5(
                error,
                "failed to generate Kagemusha V4 Ep VK",
            )
        })?;
    let step_ep_verifying_key_bytes = step_ep_verifying_key.to_bytes(SerdeFormat::Processed);
    validate_kagemusha_generated_payload_size_v4(
        step_ep_verifying_key_bytes.len(),
        "Ep verifying key",
    )?;
    let step_ep_final_protocol = snark_verifier::system::halo2::compile(
        &step_ep_params,
        &step_ep_verifying_key,
        compile_config(),
    );
    let step_ep_structure_sha256 = kagemusha_require_protocol_structure_v1(
        &step_ep_seed.protocol,
        &step_ep_final_protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let step_ep_final_protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &step_ep_final_protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    drop(step_ep_verifying_key);
    drop(keygen_eq_output);
    drop(keygen_ep_output);
    drop(keygen_witness);
    drop(step_eq_seed_recursion);
    drop(step_ep_seed_recursion);
    drop(keygen_calibration);
    step_eq_seed_bootstrap.circuit_break_points = step_eq_break_points;
    step_ep_seed_bootstrap.circuit_break_points = step_ep_break_points;
    step_eq_seed_bootstrap.validate_bootstrap_protocol(
        &step_eq_circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_seed.structure_sha256,
        &step_eq_seed.protocol,
    )?;
    step_ep_seed_bootstrap.validate_bootstrap_protocol(
        &step_ep_circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_seed.structure_sha256,
        &step_ep_seed.protocol,
    )?;

    if step_eq_final_protocol_sha256 == step_ep_final_protocol_sha256 {
        return Err("Kagemusha V4 Eq/Ep final protocol identities collide".to_owned());
    }

    let final_calibration = kagemusha_generation_calibration_v4(
        step_eq_final_protocol_sha256,
        step_ep_final_protocol_sha256,
    )?;
    let step_eq_final_seed_recursion = kagemusha_eq_recursion_from_bootstrap_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        step_eq_final_protocol.clone(),
        step_eq_structure_sha256,
        &step_eq_seed_bootstrap,
        KagemushaBootstrapParentValidationV4::Strict,
    )?;
    let step_ep_final_seed_recursion = kagemusha_ep_recursion_from_bootstrap_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        step_ep_final_protocol.clone(),
        step_ep_structure_sha256,
        &step_ep_seed_bootstrap,
        KagemushaBootstrapParentValidationV4::Strict,
    )?;
    let final_bootstrap_witness = KagemushaStepWitnessV4 {
        public_inputs: &final_calibration.public_inputs,
        proof_step_count: 1,
        secure: &final_calibration.secure,
        output_membership: &final_calibration.output_membership,
        step_eq_recursion: &step_eq_final_seed_recursion,
        step_ep_recursion: &step_ep_final_seed_recursion,
        step_eq_bootstrap: Some(&step_eq_seed_bootstrap),
        step_ep_bootstrap: Some(&step_ep_seed_bootstrap),
    };
    let (final_eq_output, final_ep_output) = collect_kagemusha_step_scalar_audits_v5(
        &final_bootstrap_witness,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        true,
    )?;
    let step_eq_zero_instances = vec![vec![Fp::ZERO; public_len]];
    let step_ep_zero_instances = vec![vec![Fq::ZERO; public_len]];

    // Build the Ep bootstrap first, then release its PK. Ep is rebuilt only
    // after both authenticated bootstrap payloads exist, when its live proof
    // and final serialized PK can be emitted. At no point does an Eq PK coexist
    // with an Ep PK.
    let step_ep_bootstrap_keygen_circuit = build_kagemusha_step_ep_circuit_v5(
        &final_bootstrap_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        &final_eq_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let step_ep_bootstrap_verifying_key =
        parse_kagemusha_ep_vk_v4(&step_ep_verifying_key_bytes, step_ep_circuit_params.clone())?;
    let (step_ep_bootstrap_proving_key, ()) = keygen_pk_consuming_with(
        &step_ep_params,
        step_ep_bootstrap_verifying_key,
        step_ep_bootstrap_keygen_circuit,
        |circuit| {
            ensure_kagemusha_keygen_break_points_v5(
                &circuit.builder,
                &step_ep_circuit_params,
                &step_ep_seed_bootstrap.circuit_break_points,
                "StepEp bootstrap",
            )
        },
    )
    .map_err(|error| {
        format_kagemusha_consuming_keygen_error_v5(
            error,
            "failed to generate Kagemusha V4 Ep bootstrap PK",
        )
    })?;
    let step_ep_final_bootstrap_circuit = build_kagemusha_step_ep_circuit_v5(
        &final_bootstrap_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        &final_eq_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Prover(&step_ep_seed_bootstrap.circuit_break_points),
    )?;
    let (step_ep_bootstrap_proof, step_ep_bootstrap_verifying_key) = create_augmented_ep_proof_v4(
        &step_ep_params,
        step_ep_bootstrap_proving_key,
        step_ep_final_bootstrap_circuit,
        &step_ep_zero_instances,
    )?;
    let step_ep_bootstrap_current = succinct_verify_step_ep_instances(
        &step_ep_params,
        &step_ep_bootstrap_verifying_key,
        &step_ep_bootstrap_proof,
        &step_ep_zero_instances,
        step_ep_bootstrap_proof.len(),
    )?;
    drop(step_ep_bootstrap_verifying_key);

    let step_eq_bootstrap_keygen_circuit = build_kagemusha_step_eq_circuit_v5(
        &final_bootstrap_witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        &final_ep_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let step_eq_bootstrap_verifying_key =
        parse_kagemusha_eq_vk_v4(&step_eq_verifying_key_bytes, step_eq_circuit_params.clone())?;
    let (step_eq_proving_key, ()) = keygen_pk_consuming_with(
        &step_eq_params,
        step_eq_bootstrap_verifying_key,
        step_eq_bootstrap_keygen_circuit,
        |circuit| {
            ensure_kagemusha_keygen_break_points_v5(
                &circuit.builder,
                &step_eq_circuit_params,
                &step_eq_seed_bootstrap.circuit_break_points,
                "StepEq bootstrap",
            )
        },
    )
    .map_err(|error| {
        format_kagemusha_consuming_keygen_error_v5(error, "failed to generate Kagemusha V4 Eq PK")
    })?;
    let step_eq_final_bootstrap_circuit = build_kagemusha_step_eq_circuit_v5(
        &final_bootstrap_witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        &final_ep_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Prover(&step_eq_seed_bootstrap.circuit_break_points),
    )?;
    let (step_eq_bootstrap_proof, step_eq_bootstrap_verifying_key) = create_augmented_eq_proof_v4(
        &step_eq_params,
        step_eq_proving_key,
        step_eq_final_bootstrap_circuit,
        &step_eq_zero_instances,
    )?;
    if step_eq_bootstrap_proof.len()
        != usize::try_from(step_eq_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof size does not fit usize".to_owned())?
        || step_ep_bootstrap_proof.len()
            != usize::try_from(step_ep_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Ep proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 final/bootstrap proof size did not converge".to_owned());
    }
    let step_eq_bootstrap_current = succinct_verify_step_eq_instances(
        &step_eq_params,
        &step_eq_bootstrap_verifying_key,
        &step_eq_bootstrap_proof,
        &step_eq_zero_instances,
        step_eq_bootstrap_proof.len(),
    )?;
    drop(step_eq_bootstrap_verifying_key);
    drop(final_eq_output);
    drop(final_ep_output);
    drop(final_bootstrap_witness);
    drop(step_eq_final_seed_recursion);
    drop(step_ep_final_seed_recursion);
    drop(final_calibration);
    drop(step_eq_zero_instances);
    drop(step_ep_zero_instances);

    let mut step_eq_final_bootstrap = kagemusha_eq_seed_bootstrap_payload_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        &KagemushaEqBootstrapSeedV4 {
            protocol: step_eq_seed.protocol.clone(),
            structure_sha256: step_eq_structure_sha256,
            protocol_sha256: step_eq_seed.protocol_sha256,
            proof: step_eq_bootstrap_proof,
            current: step_eq_bootstrap_current,
        },
    )?;
    step_eq_final_bootstrap.circuit_break_points =
        step_eq_seed_bootstrap.circuit_break_points.clone();
    step_eq_final_bootstrap.validate_bootstrap_protocol(
        &step_eq_circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_structure_sha256,
        &step_eq_seed.protocol,
    )?;
    let mut step_ep_final_bootstrap = kagemusha_ep_seed_bootstrap_payload_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        &KagemushaEpBootstrapSeedV4 {
            protocol: step_ep_seed.protocol.clone(),
            structure_sha256: step_ep_structure_sha256,
            protocol_sha256: step_ep_seed.protocol_sha256,
            proof: step_ep_bootstrap_proof,
            current: step_ep_bootstrap_current,
        },
    )?;
    step_ep_final_bootstrap.circuit_break_points =
        step_ep_seed_bootstrap.circuit_break_points.clone();
    step_ep_final_bootstrap.validate_bootstrap_protocol(
        &step_ep_circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_structure_sha256,
        &step_ep_seed.protocol,
    )?;
    drop(step_eq_seed);
    drop(step_ep_seed);
    drop(step_eq_seed_bootstrap);
    drop(step_ep_seed_bootstrap);
    let step_eq_bootstrap_witness = step_eq_final_bootstrap.encode_authenticated(
        &step_eq_circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_structure_sha256,
    )?;
    validate_kagemusha_generated_payload_size_v4(
        step_eq_bootstrap_witness.len(),
        "Eq bootstrap witness",
    )?;
    let step_ep_bootstrap_witness = step_ep_final_bootstrap.encode_authenticated(
        &step_ep_circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_structure_sha256,
    )?;
    validate_kagemusha_generated_payload_size_v4(
        step_ep_bootstrap_witness.len(),
        "Ep bootstrap witness",
    )?;
    KagemushaStepBootstrapV4::decode_authenticated(
        &step_eq_bootstrap_witness,
        &step_eq_circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_structure_sha256,
    )?;
    KagemushaStepBootstrapV4::decode_authenticated(
        &step_ep_bootstrap_witness,
        &step_ep_circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_structure_sha256,
    )?;

    let live_calibration = kagemusha_generation_calibration_v4(
        step_eq_final_protocol_sha256,
        step_ep_final_protocol_sha256,
    )?;
    let step_eq_live_recursion = kagemusha_eq_recursion_from_bootstrap_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        step_eq_final_protocol.clone(),
        step_eq_structure_sha256,
        &step_eq_final_bootstrap,
        KagemushaBootstrapParentValidationV4::Strict,
    )?;
    let step_ep_live_recursion = kagemusha_ep_recursion_from_bootstrap_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        step_ep_final_protocol.clone(),
        step_ep_structure_sha256,
        &step_ep_final_bootstrap,
        KagemushaBootstrapParentValidationV4::Strict,
    )?;
    drop(step_eq_final_protocol);
    drop(step_ep_final_protocol);
    let live_witness = KagemushaStepWitnessV4 {
        public_inputs: &live_calibration.public_inputs,
        proof_step_count: 1,
        secure: &live_calibration.secure,
        output_membership: &live_calibration.output_membership,
        step_eq_recursion: &step_eq_live_recursion,
        step_ep_recursion: &step_ep_live_recursion,
        step_eq_bootstrap: Some(&step_eq_final_bootstrap),
        step_ep_bootstrap: Some(&step_ep_final_bootstrap),
    };
    let (live_eq_output, live_ep_output) = collect_kagemusha_step_scalar_audits_v5(
        &live_witness,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        true,
    )?;
    let step_eq_live_keygen_circuit = build_kagemusha_step_eq_circuit_v5(
        &live_witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        &live_ep_output,
        KagemushaStepPublicModeV4::Live,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let step_eq_live_verifying_key =
        parse_kagemusha_eq_vk_v4(&step_eq_verifying_key_bytes, step_eq_circuit_params.clone())?;
    let (step_eq_proving_key, ()) = keygen_pk_consuming_with(
        &step_eq_params,
        step_eq_live_verifying_key,
        step_eq_live_keygen_circuit,
        |circuit| {
            ensure_kagemusha_keygen_break_points_v5(
                &circuit.builder,
                &step_eq_circuit_params,
                &step_eq_final_bootstrap.circuit_break_points,
                "StepEq live",
            )
        },
    )
    .map_err(|error| {
        format_kagemusha_consuming_keygen_error_v5(error, "failed to regenerate Kagemusha V4 Eq PK")
    })?;
    let step_eq_proving_key_size_bytes = {
        let mut writer = KagemushaBoundedProvingKeyWriterV5::new(step_eq_proving_key_sink);
        step_eq_proving_key
            .write_streaming(&mut writer, SerdeFormat::Processed)
            .map_err(|error| {
                format!("failed to stream Kagemusha V5 Eq processed proving key: {error}")
            })?;
        writer.finish("Eq proving key")?
    };
    let step_eq_live_circuit = build_kagemusha_step_eq_circuit_v5(
        &live_witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        &live_ep_output,
        KagemushaStepPublicModeV4::Live,
        KagemushaCircuitBuilderStageV5::Prover(&step_eq_final_bootstrap.circuit_break_points),
    )?;
    let (step_eq_live_proof, step_eq_live_verifying_key) = prove_step_eq_v4(
        &step_eq_params,
        step_eq_proving_key,
        step_eq_live_circuit,
        &live_calibration.public_inputs,
        1,
        &step_eq_circuit_params,
    )?;
    drop(step_eq_live_verifying_key);

    // Ep was used once above to authenticate its selector-zero bootstrap. It
    // is rebuilt only after the Eq PK has been consumed so the live Ep proof
    // and published bytes are produced without dual-PK residency.
    let step_ep_live_keygen_circuit = build_kagemusha_step_ep_circuit_v5(
        &live_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        &live_eq_output,
        KagemushaStepPublicModeV4::Live,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    let step_ep_live_verifying_key =
        parse_kagemusha_ep_vk_v4(&step_ep_verifying_key_bytes, step_ep_circuit_params.clone())?;
    let (step_ep_proving_key, ()) = keygen_pk_consuming_with(
        &step_ep_params,
        step_ep_live_verifying_key,
        step_ep_live_keygen_circuit,
        |circuit| {
            ensure_kagemusha_keygen_break_points_v5(
                &circuit.builder,
                &step_ep_circuit_params,
                &step_ep_final_bootstrap.circuit_break_points,
                "StepEp live",
            )
        },
    )
    .map_err(|error| {
        format_kagemusha_consuming_keygen_error_v5(error, "failed to regenerate Kagemusha V4 Ep PK")
    })?;
    let step_ep_proving_key_size_bytes = {
        let mut writer = KagemushaBoundedProvingKeyWriterV5::new(step_ep_proving_key_sink);
        step_ep_proving_key
            .write_streaming(&mut writer, SerdeFormat::Processed)
            .map_err(|error| {
                format!("failed to stream Kagemusha V5 Ep processed proving key: {error}")
            })?;
        writer.finish("Ep proving key")?
    };
    let step_ep_live_circuit = build_kagemusha_step_ep_circuit_v5(
        &live_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        &live_eq_output,
        KagemushaStepPublicModeV4::Live,
        KagemushaCircuitBuilderStageV5::Prover(&step_ep_final_bootstrap.circuit_break_points),
    )?;
    let (step_ep_live_proof, step_ep_live_verifying_key) = prove_step_ep_v4(
        &step_ep_params,
        step_ep_proving_key,
        step_ep_live_circuit,
        &live_calibration.public_inputs,
        1,
        &step_ep_circuit_params,
    )?;
    drop(step_ep_live_verifying_key);
    drop(live_eq_output);
    drop(live_ep_output);
    drop(live_witness);
    drop(step_eq_live_recursion);
    drop(step_ep_live_recursion);
    if step_eq_live_proof.len()
        != usize::try_from(step_eq_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq live proof size does not fit usize".to_owned())?
        || step_ep_live_proof.len()
            != usize::try_from(step_ep_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Ep live proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 live proof size differs from bootstrap calibration".to_owned());
    }
    let KagemushaGenerationCalibrationV4 {
        public_inputs: measured_public_inputs,
        secure: _,
        output_membership: _,
    } = live_calibration;
    let compact_measured_public_inputs =
        KagemushaCompactPublicInputsV5::from_private(&measured_public_inputs, 1);
    let measured_pair = KagemushaPastaCycleProofPairV4 {
        version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
        proof_step_count: 1,
        public_inputs: compact_measured_public_inputs,
        step_eq_proof_bytes: step_eq_live_proof,
        step_ep_proof_bytes: step_ep_live_proof,
        step_eq_accumulation_proof: KagemushaIpaAccumulationProofV4::initialization(
            step_eq_circuit_params.k,
        )?,
        step_ep_accumulation_proof: KagemushaIpaAccumulationProofV4::initialization(
            step_ep_circuit_params.k,
        )?,
    };
    let absolute_pair_max =
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4;
    // Both release-sized proving keys and all populated circuits are gone.
    // Only now materialize the two terminal VK domains together.
    let step_eq_terminal_verifying_key =
        parse_kagemusha_eq_vk_v4(&step_eq_verifying_key_bytes, step_eq_circuit_params.clone())?;
    let step_ep_terminal_verifying_key =
        parse_kagemusha_ep_vk_v4(&step_ep_verifying_key_bytes, step_ep_circuit_params.clone())?;
    terminal_validate_kagemusha_eq_bootstrap_v4(
        &step_eq_params,
        &step_eq_terminal_verifying_key,
        &step_eq_circuit_params,
        &step_eq_final_bootstrap,
    )?;
    terminal_validate_kagemusha_ep_bootstrap_v4(
        &step_ep_params,
        &step_ep_terminal_verifying_key,
        &step_ep_circuit_params,
        &step_ep_final_bootstrap,
    )?;
    drop(step_eq_final_bootstrap);
    drop(step_ep_final_bootstrap);
    terminal_verify_proof_pair_v4(
        &step_eq_params,
        &step_eq_terminal_verifying_key,
        &step_ep_params,
        &step_ep_terminal_verifying_key,
        &measured_pair,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        absolute_pair_max,
    )?;
    drop(step_eq_terminal_verifying_key);
    drop(step_ep_terminal_verifying_key);
    let measured_live_pair_bytes = measured_pair.encode_authenticated(
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        absolute_pair_max,
    )?;
    KagemushaPastaCycleProofPairV4::decode_authenticated(
        &measured_live_pair_bytes,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        absolute_pair_max,
    )?;
    drop(measured_pair);

    let step_eq_parameters = kagemusha_eq_parameters_bytes_v4(&step_eq_params)?;
    validate_kagemusha_generated_payload_size_v4(step_eq_parameters.len(), "Eq parameters")?;
    drop(step_eq_params);
    let step_ep_parameters = kagemusha_ep_parameters_bytes_v4(&step_ep_params)?;
    validate_kagemusha_generated_payload_size_v4(step_ep_parameters.len(), "Ep parameters")?;
    drop(step_ep_params);

    {
        let parsed = parse_kagemusha_params_v4::<EqAffine>(
            &step_eq_parameters,
            step_eq_circuit_params.k,
            "generated Eq",
        )?;
        if kagemusha_eq_parameters_bytes_v4(&parsed)? != step_eq_parameters {
            return Err("Kagemusha V4 generated Eq parameter encoding is not canonical".to_owned());
        }
    }
    {
        let parsed = parse_kagemusha_params_v4::<EpAffine>(
            &step_ep_parameters,
            step_ep_circuit_params.k,
            "generated Ep",
        )?;
        if kagemusha_ep_parameters_bytes_v4(&parsed)? != step_ep_parameters {
            return Err("Kagemusha V4 generated Ep parameter encoding is not canonical".to_owned());
        }
    }
    {
        let parsed =
            parse_kagemusha_eq_vk_v4(&step_eq_verifying_key_bytes, step_eq_circuit_params.clone())?;
        if parsed.to_bytes(SerdeFormat::Processed) != step_eq_verifying_key_bytes {
            return Err("Kagemusha V4 generated Eq verifier-key round-trip mismatch".to_owned());
        }
    }
    {
        let parsed =
            parse_kagemusha_ep_vk_v4(&step_ep_verifying_key_bytes, step_ep_circuit_params.clone())?;
        if parsed.to_bytes(SerdeFormat::Processed) != step_ep_verifying_key_bytes {
            return Err("Kagemusha V4 generated Ep verifier-key round-trip mismatch".to_owned());
        }
    }
    Ok(KagemushaGeneratedPastaCycleArtifactsV4 {
        step_eq: KagemushaGeneratedParityArtifactsV4 {
            circuit_params: step_eq_circuit_params.clone(),
            compiled_protocol_structure_sha256: step_eq_structure_sha256,
            step_proof_size_bytes: step_eq_circuit_params.max_parent_proof_bytes,
            parameters: step_eq_parameters,
            proving_key_size_bytes: step_eq_proving_key_size_bytes,
            verifying_key: step_eq_verifying_key_bytes,
            bootstrap_witness: step_eq_bootstrap_witness,
        },
        step_ep: KagemushaGeneratedParityArtifactsV4 {
            circuit_params: step_ep_circuit_params.clone(),
            compiled_protocol_structure_sha256: step_ep_structure_sha256,
            step_proof_size_bytes: step_ep_circuit_params.max_parent_proof_bytes,
            parameters: step_ep_parameters,
            proving_key_size_bytes: step_ep_proving_key_size_bytes,
            verifying_key: step_ep_verifying_key_bytes,
            bootstrap_witness: step_ep_bootstrap_witness,
        },
        measured_live_pair_bytes,
    })
}

/// Produce and immediately self-verify one concrete V4 StepEq proof.
pub(crate) fn prove_step_eq_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit: KagemushaStepEqCircuitV4,
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<
    (
        Vec<u8>,
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    ),
    String,
> {
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EqAffine},
        plonk::{create_proof_consuming, verify_proof},
        poly::{commitment::Params as _, ipa::commitment::IPACommitmentScheme},
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k || circuit.params != *circuit_params {
        return Err("Kagemusha V4 StepEq proving configuration mismatch".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let column = public_inputs.instance_column::<Fp>(
        proof_step_count,
        circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let columns: [&[Fp]; 1] = [&column];
    let proofs_instances: [&[&[Fp]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    let verifying_key = create_proof_consuming::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceProverIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        circuit,
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Eq proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
    >(
        params,
        &verifying_key,
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Eq generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?;
    if proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha V4 Eq proof exceeds its authenticated bound".to_owned());
    }
    terminal_verify_step_eq_v4(
        params,
        &verifying_key,
        &proof,
        public_inputs,
        proof_step_count,
        circuit_params,
    )?;
    Ok((proof, verifying_key))
}

/// Produce and immediately self-verify one concrete V4 StepEp proof.
pub(crate) fn prove_step_ep_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit: KagemushaStepEpCircuitV4,
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<
    (
        Vec<u8>,
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    ),
    String,
> {
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EpAffine},
        plonk::{create_proof_consuming, verify_proof},
        poly::{commitment::Params as _, ipa::commitment::IPACommitmentScheme},
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k || circuit.params != *circuit_params {
        return Err("Kagemusha V4 StepEp proving configuration mismatch".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let column = public_inputs.instance_column::<Fq>(
        proof_step_count,
        circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let columns: [&[Fq]; 1] = [&column];
    let proofs_instances: [&[&[Fq]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    let verifying_key = create_proof_consuming::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceProverIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        circuit,
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Ep proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
    >(
        params,
        &verifying_key,
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Ep generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?;
    if proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha V4 Ep proof exceeds its authenticated bound".to_owned());
    }
    terminal_verify_step_ep_v4(
        params,
        &verifying_key,
        &proof,
        public_inputs,
        proof_step_count,
        circuit_params,
    )?;
    Ok((proof, verifying_key))
}

fn constrain_two_parent_presence_bits<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    parent_count: halo2_base::AssignedValue<F>,
) -> [halo2_base::AssignedValue<F>; 2]
where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Constant,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    range.range_check(ctx, parent_count, 2);
    let is_three = range
        .gate()
        .is_equal(ctx, parent_count, Constant(F::from(3)));
    range.gate().assert_is_const(ctx, &is_three, &F::ZERO);
    let is_zero = range.gate().is_zero(ctx, parent_count);
    let slot_zero = range.gate().not(ctx, is_zero);
    let slot_one = range
        .gate()
        .is_equal(ctx, parent_count, Constant(F::from(2)));
    range.gate().assert_bit(ctx, slot_zero);
    range.gate().assert_bit(ctx, slot_one);
    [slot_zero, slot_one]
}

enum KagemushaDeferredMsmV5<'a, C>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
{
    Dense(&'a mut KagemushaDenseMsmJobsV5<C>),
    #[cfg(test)]
    GenericTest,
}

/// Constrain one complete selector-bound V5 reciprocal audit once.
///
/// The complete post-branch stage plan is required for both public slots.  As
/// on the scalar side, each public exposure is multiplied by its slot-presence
/// bit; every deferred MSM, selector schedule, serialization, and hash is
/// evaluated only once.
fn constrain_reciprocal_point_audit_identity_v4<'chip, C>(
    ctx: &mut halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager<C::Base>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::Base>,
    base: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::Base>,
    scalar: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::ScalarExt>,
    witness: &super::kagemusha_cycle_loader::DeferredEquationWitness<C>,
    stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
    current_public_parent_count: halo2_base::AssignedValue<C::Base>,
    parent_public_parent_counts: [halo2_base::AssignedValue<C::Base>; 2],
    expected_words: [&[halo2_base::AssignedValue<C::Base>]; 2],
    msm: KagemushaDeferredMsmV5<'_, C>,
) -> Result<[halo2_base::AssignedValue<C::Base>; 8], String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    use super::kagemusha_cycle_loader::PastaCycleEccChip;

    if expected_words.iter().any(|words| words.len() != 2) {
        return Err("Kagemusha reciprocal V5 audit slot has the wrong shape".to_owned());
    }
    scalar_lineage_v1::validate_stage_shapes_v4(stages, witness.equations.len())
        .map_err(|error| format!("invalid Kagemusha reciprocal V4 stage plan: {error:?}"))?;

    let slot_present =
        constrain_two_parent_presence_bits(ctx.main(), base.range, current_public_parent_count);
    let parent_has_carried = parent_public_parent_counts.map(|parent_count| {
        constrain_two_parent_presence_bits(ctx.main(), base.range, parent_count)[0]
    });

    let mut gate_tags = Vec::with_capacity(witness.equations.len());
    let mut selectors = Vec::with_capacity(witness.equations.len());
    for stage in stages {
        let enabled = match stage.gate {
            scalar_lineage_v1::DeferredEquationGateV4::ParentCurrent { slot }
            | scalar_lineage_v1::DeferredEquationGateV4::ParentLineageSelect { slot } => {
                slot_present[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::ParentCarriedFold { slot } => {
                let enabled = base.range.gate().mul(
                    ctx.main(),
                    Existing(slot_present[slot]),
                    Existing(parent_has_carried[slot]),
                );
                base.range.gate().assert_bit(ctx.main(), enabled);
                enabled
            }
            scalar_lineage_v1::DeferredEquationGateV4::BranchFold => slot_present[1],
            scalar_lineage_v1::DeferredEquationGateV4::BranchSelect => slot_present[0],
        };
        gate_tags.extend(std::iter::repeat_n(
            stage.gate.audit_tag(),
            stage.range.len(),
        ));
        selectors.extend(std::iter::repeat_n(enabled, stage.range.len()));
    }

    let mut chip = PastaCycleEccChip::<C>::new(base, scalar);
    let audit = chip.assign_deferred_equations_with_selectors(ctx, witness, &selectors)?;
    let bytes = chip.assigned_equation_bytes_v5(ctx, &audit, &gate_tags, &selectors)?;
    let digest = sha_jobs.digest_constrained(ctx.main(), &bytes)?;
    match msm {
        KagemushaDeferredMsmV5::Dense(dense_jobs) => {
            chip.constrain_deferred_equation_batch_v5(
                ctx, &audit, &selectors, &digest, dense_jobs,
            )?;
        }
        #[cfg(test)]
        KagemushaDeferredMsmV5::GenericTest => {
            chip.constrain_deferred_equation_batch_generic_v5(ctx, &audit, &selectors, &digest)?;
        }
    }
    for (present, expected_words) in slot_present.into_iter().zip(expected_words) {
        for (assigned, expected) in digest.chunks_exact(4).zip(expected_words) {
            let packed = pack_assigned_u32_words_v5(ctx.main(), base.range, assigned);
            let exposed = base
                .range
                .gate()
                .mul(ctx.main(), Existing(present), Existing(packed));
            ctx.main().constrain_equal(&exposed, expected);
        }
    }
    Ok(digest)
}

/// Reconstruct the exact compiled-protocol identity in the reciprocal
/// native-point circuit and bind it to the same public release words.
///
/// The protocol points are assigned and canonicalized independently here.
/// Their equality with the values used by the scalar verifier follows from the
/// scalar/point deferred-equation SHA join; this additional digest anchors that
/// common point namespace and transcript state to the authenticated release.
fn constrain_reciprocal_protocol_identity<'chip, C>(
    ctx: &mut halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager<C::Base>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::Base>,
    base: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::Base>,
    scalar: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::ScalarExt>,
    identity: &scalar_lineage_v1::DeferredProtocolIdentityWitness<C>,
    fixed_structure_sha256: [u8; 32],
    expected_words: &[halo2_base::AssignedValue<C::Base>],
) -> Result<[halo2_base::AssignedValue<C::Base>; 8], String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    use snark_verifier::loader::halo2::{EccInstructions as _, IntegerInstructions as _};

    use super::kagemusha_cycle_loader::PastaCycleEccChip;

    if expected_words.len() != 2
        || identity.structure_sha256 != fixed_structure_sha256
        || identity.preprocessed.is_empty()
        || identity
            .preprocessed
            .iter()
            .any(|point| bool::from(point.is_identity()))
    {
        return Err("Kagemusha reciprocal protocol identity shape mismatch".to_owned());
    }
    let push_constants = |output: &mut Vec<KagemushaSha256ByteV4<C::Base>>, bytes: &[u8]| {
        output.extend(bytes.iter().copied().map(KagemushaSha256ByteV4::constant));
    };
    let mut bytes = Vec::new();
    push_constants(&mut bytes, KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1);
    push_constants(&mut bytes, &[0]);
    push_constants(
        &mut bytes,
        &KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes(),
    );
    push_constants(
        &mut bytes,
        &protocol_parity_tag(identity.parity).to_le_bytes(),
    );
    push_constants(&mut bytes, &fixed_structure_sha256);
    push_constants(
        &mut bytes,
        &u32::try_from(identity.preprocessed.len())
            .map_err(|_| "Kagemusha reciprocal protocol point count does not fit u32".to_owned())?
            .to_le_bytes(),
    );

    let chip = PastaCycleEccChip::<C>::new(base, scalar);
    for point in &identity.preprocessed {
        let point = chip.assign_point(ctx, *point);
        bytes.extend(chip.assigned_point_bytes(ctx, &point));
    }
    let transcript_initial_state = chip
        .scalar_chip()
        .assign_integer(ctx, identity.transcript_initial_state);
    bytes.extend(chip.assigned_scalar_bytes(ctx, &transcript_initial_state));
    let digest = sha_jobs.digest_constrained(ctx.main(), &bytes)?;
    for (assigned, expected) in digest.chunks_exact(4).zip(expected_words) {
        let packed = pack_assigned_u32_words_v5(ctx.main(), base.range, assigned);
        ctx.main().constrain_equal(&packed, expected);
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, mem, rc::Rc};

    use super::*;
    use halo2_proofs::arithmetic::Field;
    use iroha_data_model::offline::{
        KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4, KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
    };
    use snark_verifier::util::arithmetic::PrimeCurveAffine as _;

    fn encode_with_alternate_norito_layout<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(value).expect("encode alternate-layout Kagemusha recursion value")
    }

    #[test]
    fn source_runtime_heavy_residency_is_strictly_eq_then_ep() {
        let residency = KagemushaSourceRuntimeHeavyResidencyV4::default();
        {
            let _eq = residency
                .enter(KagemushaPastaCycleParityV1::StepEq)
                .expect("enter Eq residency");
            assert!(
                residency
                    .enter(KagemushaPastaCycleParityV1::StepEp)
                    .is_err(),
                "Ep cannot become resident while Eq is live"
            );
        }
        {
            let _ep = residency
                .enter(KagemushaPastaCycleParityV1::StepEp)
                .expect("enter Ep residency after Eq drops");
        }
        residency.assert_released().expect("all material dropped");
        assert_eq!(
            *residency.events.borrow(),
            vec![
                (KagemushaPastaCycleParityV1::StepEq, true),
                (KagemushaPastaCycleParityV1::StepEq, false),
                (KagemushaPastaCycleParityV1::StepEp, true),
                (KagemushaPastaCycleParityV1::StepEp, false),
            ]
        );
    }

    #[test]
    fn source_runtime_heavy_permit_recovers_after_worker_panic() {
        let panicked = std::panic::catch_unwind(|| {
            let _permit = lock_kagemusha_source_runtime_heavy_v4();
            panic!("source runtime permit poison fixture");
        });
        assert!(panicked.is_err(), "fixture must poison the permit once");

        let _permit = lock_kagemusha_source_runtime_heavy_v4();
        assert!(!KAGEMUSHA_SOURCE_RUNTIME_HEAVY_PERMIT_V4.is_poisoned());
    }

    fn valid_step_circuit_params_v4() -> KagemushaStepCircuitParamsV4 {
        valid_step_circuit_params_for_k_v4(16)
    }

    fn valid_step_circuit_params_for_k_v4(k: u32) -> KagemushaStepCircuitParamsV4 {
        let public_input_limbs = KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)
            .map(|layout| layout.instance_column_limbs)
            .unwrap_or(64);
        KagemushaStepCircuitParamsV4 {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4.to_vec(),
            num_lookup_advice_per_phase: KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4.to_vec(),
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs,
            minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: 8_192,
        }
    }

    fn first_release_generation_params_v4() -> KagemushaStepCircuitParamsV4 {
        let mut params = valid_step_circuit_params_v4();
        params.num_advice_per_phase = KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4.to_vec();
        params.num_lookup_advice_per_phase = KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4.to_vec();
        params
    }

    #[test]
    fn v5_lookup_shape_ignores_only_trailing_zero_phases() {
        assert!(kagemusha_lookup_phase_columns_fit_v5(&[1, 0, 0], &[1]));
        assert!(kagemusha_lookup_phase_columns_fit_v5(&[1], &[1, 0, 0]));
        assert!(kagemusha_lookup_phase_columns_fit_v5(&[0, 0, 0], &[]));

        assert!(!kagemusha_lookup_phase_columns_fit_v5(&[2, 0, 0], &[1]));
        assert!(!kagemusha_lookup_phase_columns_fit_v5(&[1, 0, 1], &[1, 1]));
        assert!(!kagemusha_lookup_phase_columns_fit_v5(&[1, 1], &[1, 0, 1]));
    }

    #[test]
    fn v5_generator_final_protocol_compile_uses_direct_instances() {
        let config = format!("{:?}", kagemusha_ipa_compile_config_v4(73));
        assert!(config.contains("query_instance: false"));
        assert!(config.contains("num_instance: [73]"));

        let source = include_str!("kagemusha_recursion_adapter.rs");
        let generator = source
            .split_once("fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(")
            .expect("artifact generator")
            .1
            .split_once("/// Produce and immediately self-verify one concrete V4 StepEq proof.")
            .expect("end artifact generator")
            .0;
        assert!(
            generator
                .contains("let compile_config = || kagemusha_ipa_compile_config_v4(public_len);")
        );
        assert_eq!(generator.matches("compile_config()").count(), 2);
        assert!(!generator.contains("Config::ipa().with_num_instance"));
    }

    #[test]
    fn v5_compact_step_count_is_witness_bound_not_fixed() {
        let source = include_str!("kagemusha_recursion_adapter.rs");
        let header = source
            .split_once("fn constrain_kagemusha_compact_eq_header_v5(")
            .expect("StepEq compact-header relation")
            .1
            .split_once("fn constrain_kagemusha_output_frontier_v4")
            .expect("end StepEq compact-header relation")
            .0;
        let signature = header
            .split_once(") -> Result<(), String>")
            .expect("StepEq compact-header signature")
            .0;

        assert!(
            !signature.contains("proof_step_count"),
            "runtime step data must not enter the circuit through a Rust constant"
        );
        assert_eq!(
            header
                .matches("KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5")
                .count(),
            2,
            "the compact step cell must appear only in the operation copy constraint and range check"
        );
        assert!(header.contains("let operation_step ="));
        assert!(header.contains("ctx.constrain_equal("));
        assert!(header.contains("range.range_check("));
        assert!(
            !header.contains("Fp::from(u64::from(proof_step_count))"),
            "keygen step one must never be assigned into fixed columns"
        );
    }

    #[test]
    fn v5_runtime_prover_retains_raw_vks_and_stages_pk_then_terminal_vks() {
        let source = include_str!("kagemusha_recursion_adapter.rs");
        let prover_fields = source
            .split_once("pub(crate) struct KagemushaPastaCycleProverV4 {")
            .expect("runtime prover fields")
            .1
            .split_once("impl std::ops::Deref for KagemushaPastaCycleProverV4")
            .expect("end runtime prover fields")
            .0;
        assert!(prover_fields.contains("step_eq_verifying_key_bytes: Vec<u8>"));
        assert!(prover_fields.contains("step_ep_verifying_key_bytes: Vec<u8>"));
        assert!(!prover_fields.contains("step_eq_verifying_key:"));
        assert!(!prover_fields.contains("step_ep_verifying_key:"));

        let prove = source
            .split_once("    fn prove_step_v4(")
            .expect("runtime staged prover")
            .1
            .split_once("/// Circuit-side parent-proof")
            .expect("end runtime staged prover")
            .0;
        let eq_pk = prove
            .find("let step_eq_proving_key =")
            .expect("Eq PK parse");
        let eq_consume = prove[eq_pk..]
            .find("let (step_eq_proof_bytes, step_eq_verifying_key) = prove_step_eq_v4")
            .map(|offset| eq_pk + offset)
            .expect("Eq consuming proof");
        let eq_vk_drop = prove[eq_consume..]
            .find("drop(step_eq_verifying_key)")
            .map(|offset| eq_consume + offset)
            .expect("returned Eq VK drop");
        let ep_circuit = prove[eq_vk_drop..]
            .find("let step_ep = build_kagemusha_step_ep_circuit_v5")
            .map(|offset| eq_vk_drop + offset)
            .expect("Ep circuit after Eq consumption");
        let ep_vk_drop = prove[ep_circuit..]
            .find("drop(step_ep_verifying_key)")
            .map(|offset| ep_circuit + offset)
            .expect("returned Ep VK drop");
        let terminal_vks = prove[ep_vk_drop..]
            .find("let step_eq_terminal_verifying_key =")
            .map(|offset| ep_vk_drop + offset)
            .expect("terminal VK parse after both PKs");
        assert!(!prove.contains("step_eq_proving_key.get_vk()"));
        assert!(!prove.contains("step_ep_proving_key.get_vk()"));
        assert!(eq_pk < eq_consume && eq_consume < eq_vk_drop);
        assert!(eq_vk_drop < ep_circuit && ep_circuit < ep_vk_drop && ep_vk_drop < terminal_vks);
    }

    #[test]
    fn source_qualification_defers_full_proving_key_parse_until_prover_use() {
        let source = include_str!("kagemusha_recursion_adapter.rs");
        for (start, end) in [
            (
                "fn qualify_kagemusha_eq_artifacts_v4(",
                "fn qualify_kagemusha_ep_artifacts_v4(",
            ),
            (
                "fn qualify_kagemusha_ep_artifacts_v4(",
                "pub(super) fn qualify_kagemusha_authenticated_artifact_source_v4(",
            ),
        ] {
            let qualification = source
                .split_once(start)
                .expect("parity qualification")
                .1
                .split_once(end)
                .expect("end parity qualification")
                .0;
            assert!(qualification.contains("preflight_kagemusha_pk_from_source_v4("));
            assert!(!qualification.contains("load_kagemusha_eq_proving_key_from_source_v4("));
            assert!(!qualification.contains("load_kagemusha_ep_proving_key_from_source_v4("));
            assert!(!qualification.contains("ProvingKey::read"));
        }

        let eq_runtime = source
            .split_once("fn load_kagemusha_source_eq_prover_material_v4(")
            .expect("Eq source prover loader")
            .1
            .split_once("fn load_kagemusha_source_ep_prover_material_v4(")
            .expect("end Eq source prover loader")
            .0;
        let ep_runtime = source
            .split_once("fn load_kagemusha_source_ep_prover_material_v4(")
            .expect("Ep source prover loader")
            .1
            .split_once("fn load_kagemusha_source_eq_recursion_material_v4(")
            .expect("end Ep source prover loader")
            .0;
        assert!(eq_runtime.contains("load_kagemusha_eq_proving_key_from_qualified_source_v4"));
        assert!(ep_runtime.contains("load_kagemusha_ep_proving_key_from_qualified_source_v4"));
    }

    #[test]
    fn v5_scalar_audit_prepass_is_witness_only() {
        let source = include_str!("kagemusha_recursion_adapter.rs");
        let prepass = source
            .split_once("fn collect_kagemusha_scalar_audits_v4<C>(")
            .expect("scalar audit prepass")
            .1
            .split_once("fn scalar_field_parent_count_v4")
            .expect("end scalar audit prepass")
            .0;
        assert!(prepass.contains("BaseCircuitBuilder::<C::ScalarExt>::new(true)"));
        assert!(!prepass.contains("BaseCircuitBuilder::<C::ScalarExt>::new(false)"));
    }

    #[test]
    fn v5_generator_never_builds_or_retains_both_parity_circuits() {
        let source = include_str!("kagemusha_recursion_adapter.rs");
        let generator = source
            .split_once("fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(")
            .expect("artifact generator")
            .1
            .split_once("/// Produce and immediately self-verify one concrete V4 StepEq proof.")
            .expect("end artifact generator")
            .0;
        assert!(!generator.contains("build_kagemusha_step_circuits_v4("));
        assert!(!generator.contains("build_kagemusha_step_circuits_with_mode_v4("));
        assert!(generator.contains("keygen_vk_consuming_with("));
        assert!(generator.contains("keygen_pk_consuming_with("));
        assert!(!generator.contains("drop(step_eq_live_keygen_circuit)"));
        assert!(!generator.contains("drop(step_ep_live_keygen_circuit)"));
        assert!(generator.contains("drop(step_eq_verifying_key)"));
        assert!(generator.contains("drop(step_ep_verifying_key)"));
        let eq_seed = generator
            .find("let step_eq_seed = kagemusha_eq_bootstrap_seed_v4")
            .expect("Eq seed generation");
        let eq_spool = generator
            .find("let step_eq_parameter_spool = kagemusha_eq_parameters_bytes_v4")
            .expect("compressed Eq parameter spool");
        let eq_drop = generator
            .find("drop(step_eq_params);")
            .expect("Eq parameters released before Ep construction");
        let ep_params = generator
            .find("let step_ep_params = ParamsIPA::<EpAffine>::new")
            .expect("Ep parameter construction");
        let ep_seed = generator
            .find("let step_ep_seed = kagemusha_ep_bootstrap_seed_v4")
            .expect("Ep seed generation");
        let eq_reparse = generator
            .find("let step_eq_params = parse_kagemusha_params_v4::<EqAffine>")
            .expect("Eq parameter reconstruction");
        assert!(
            eq_seed < eq_spool
                && eq_spool < eq_drop
                && eq_drop < ep_params
                && ep_params < ep_seed
                && ep_seed < eq_reparse
        );
        let eq_stream = generator
            .find("failed to stream Kagemusha V5 Eq processed proving key")
            .expect("Eq PK stream");
        let ep_live = generator[eq_stream..]
            .find("let step_ep_live_circuit = build_kagemusha_step_ep_circuit_v5")
            .map(|offset| eq_stream + offset)
            .expect("Ep live circuit after Eq PK consumption");
        assert!(eq_stream < ep_live);
    }

    #[test]
    fn v5_generator_uses_one_disposable_rayon_worker() {
        assert_eq!(KAGEMUSHA_GENERATION_RAYON_THREADS_V5, 1);
        let source = include_str!("kagemusha_recursion_adapter.rs");
        let wrapper = source
            .split_once("pub fn generate_kagemusha_pasta_cycle_artifacts_v4(")
            .expect("public artifact generator")
            .1
            .split_once("fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(")
            .expect("bounded generator body")
            .0;
        assert!(wrapper.contains(".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)"));
        assert!(wrapper.contains("pool.install(move ||"));
    }

    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    #[test]
    fn v5_candidate_spool_identity_rejects_wrong_bindings() {
        let candidate = [0x31; 32];
        let manifest = [0x52; 32];
        validate_kagemusha_candidate_spool_identity_v5(candidate, manifest, candidate, manifest)
            .expect("exact candidate binding");
        assert!(
            validate_kagemusha_candidate_spool_identity_v5(
                candidate, manifest, [0x32; 32], manifest,
            )
            .is_err(),
            "a different candidate digest must fail closed"
        );
        assert!(
            validate_kagemusha_candidate_spool_identity_v5(
                candidate, manifest, candidate, [0x53; 32],
            )
            .is_err(),
            "a different candidate manifest digest must fail closed"
        );
    }

    #[test]
    fn runtime_profile_validation_never_regenerates_a_bootstrap_key() {
        let source = include_str!("kagemusha_recursion_adapter.rs");
        let runtime_validation = source
            .split_once("fn validate_kagemusha_profile_protocol_v4<C>(")
            .expect("runtime profile validator")
            .1
            .split_once("fn terminal_validate_kagemusha_eq_bootstrap_v4(")
            .expect("end of runtime profile validator")
            .0;

        assert!(!runtime_validation.contains("keygen_vk"));
        assert!(!runtime_validation.contains("kagemusha_bootstrap_verifying_key_v1"));
        assert!(!runtime_validation.contains("validate_bootstrap_protocol"));
        assert!(runtime_validation.contains("kagemusha_compiled_protocol_structure_sha256"));
        assert!(runtime_validation.contains("KagemushaStepBootstrapV4::decode_authenticated"));
    }

    #[test]
    fn v4_halo2_reader_preflight_rejects_untrusted_inner_degrees_and_counts() {
        use halo2_proofs::halo2curves::pasta::{EqAffine, Fp};

        let params = valid_step_circuit_params_v4();
        let malicious_degree = u32::MAX.to_le_bytes();
        assert!(
            parse_kagemusha_params_v4::<EqAffine>(&malicious_degree, params.k, "test params")
                .expect_err("untrusted ParamsIPA degree must fail before its reader")
                .contains("does not match authenticated degree")
        );

        let mut malicious_vk = vec![KAGEMUSHA_HALO2_KEY_VERSION_V4];
        malicious_vk.extend_from_slice(&u32::MAX.to_le_bytes());
        malicious_vk.push(KAGEMUSHA_HALO2_UNCOMPRESSED_SELECTORS_V4);
        malicious_vk.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            parse_kagemusha_eq_vk_v4(&malicious_vk, params.clone())
                .expect_err("untrusted VK degree must fail before its reader")
                .contains("does not match authenticated degree")
        );
        assert!(
            parse_kagemusha_eq_pk_v4(&malicious_vk, params.clone())
                .expect_err("untrusted PK degree must fail before its reader")
                .contains("does not match authenticated degree")
        );

        malicious_vk[1..5].copy_from_slice(&params.k.to_le_bytes());
        let shape = kagemusha_processed_key_shape_v4::<EqAffine>(&params, "test VK")
            .expect("bounded authenticated key shape");
        assert!(
            validate_kagemusha_processed_vk_encoding_v4(&malicious_vk, shape, "test VK")
                .expect_err("untrusted fixed count must fail before the VK reader")
                .contains("fixed-commitment count")
        );

        let reviewed = first_release_generation_params_v4();
        let configured =
            configured_kagemusha_eq_vk_wire_shape_v4(&reviewed).expect("reviewed configured shape");
        assert_eq!(configured.advice_columns, 583);
        assert_eq!(configured.base_fixed_columns, 7);
        assert_eq!(configured.selectors, 553);
        assert_eq!(configured.permutation_columns, 534);
        assert_eq!(configured.instance_columns, 1);
        let reviewed_shape = kagemusha_processed_key_shape_v4::<EqAffine>(&reviewed, "reviewed")
            .expect("reviewed key shape");
        assert_eq!(reviewed_shape.domain_rows, 1 << 16);
        assert_eq!(reviewed_shape.fixed_polynomials, 560);
        assert_eq!(reviewed_shape.permutation_polynomials, 534);
        assert_eq!(
            reviewed_shape.fixed_polynomials + reviewed_shape.permutation_polynomials,
            1_094
        );
        assert_eq!(reviewed_shape.point_bytes, 32);
        assert_eq!(reviewed_shape.scalar_bytes, mem::size_of::<Fp>());
        assert_eq!(
            reviewed_shape
                .proving_key_bytes("Eq")
                .expect("exact compact V5 PK length"),
            4_594_903_830
        );
        assert!(
            reviewed_shape
                .proving_key_bytes("Eq")
                .expect("exact compact V5 PK length")
                <= KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5
        );
    }

    #[test]
    fn v4_proving_key_preflight_checks_every_polynomial_length_and_vector_count() {
        let shape = KagemushaProcessedKeyShapeV4 {
            k: 0,
            domain_rows: 1,
            fixed_polynomials: 1,
            permutation_polynomials: 1,
            point_bytes: 1,
            scalar_bytes: 1,
        };
        let mut vk_prefix = vec![KAGEMUSHA_HALO2_KEY_VERSION_V4];
        vk_prefix.extend_from_slice(&shape.k.to_le_bytes());
        vk_prefix.push(KAGEMUSHA_HALO2_UNCOMPRESSED_SELECTORS_V4);
        vk_prefix.extend_from_slice(&1_u32.to_le_bytes());
        vk_prefix.extend_from_slice(&[0; 2]);
        let append_polynomial = |bytes: &mut Vec<u8>| {
            bytes.extend_from_slice(&1_u32.to_be_bytes());
            bytes.push(0);
        };

        let mut malicious_polynomial = vk_prefix.clone();
        malicious_polynomial.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(
            validate_kagemusha_processed_pk_encoding_v4(&malicious_polynomial, shape, "test PK",)
                .expect_err("untrusted polynomial length must fail before the PK reader")
                .contains("l0 polynomial length")
        );

        let mut malicious_fixed_count = vk_prefix.clone();
        for _ in 0..3 {
            append_polynomial(&mut malicious_fixed_count);
        }
        malicious_fixed_count.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(
            validate_kagemusha_processed_pk_encoding_v4(&malicious_fixed_count, shape, "test PK",)
                .expect_err("untrusted fixed-vector count must fail before the PK reader")
                .contains("fixed-value polynomials count")
        );

        let mut malicious_permutation_count = vk_prefix.clone();
        for _ in 0..3 {
            append_polynomial(&mut malicious_permutation_count);
        }
        for _ in 0..2 {
            malicious_permutation_count.extend_from_slice(&1_u32.to_be_bytes());
            append_polynomial(&mut malicious_permutation_count);
        }
        malicious_permutation_count.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(
            validate_kagemusha_processed_pk_encoding_v4(
                &malicious_permutation_count,
                shape,
                "test PK",
            )
            .expect_err("untrusted permutation count must fail before the PK reader")
            .contains("permutation Lagrange polynomials count")
        );

        let mut canonical = malicious_permutation_count;
        canonical.truncate(canonical.len() - 4);
        for _ in 0..2 {
            canonical.extend_from_slice(&1_u32.to_be_bytes());
            append_polynomial(&mut canonical);
        }
        validate_kagemusha_processed_pk_encoding_v4(&canonical, shape, "test PK")
            .expect("complete bounded structural encoding");
    }

    #[test]
    fn v4_role_loader_releases_each_payload_on_success_and_error() {
        struct TrackedPayload {
            bytes: Vec<u8>,
            live: Rc<Cell<bool>>,
            drops: Rc<Cell<u32>>,
        }

        impl KagemushaArtifactPayloadBytesV4 for TrackedPayload {
            fn payload_bytes(&self) -> &[u8] {
                &self.bytes
            }
        }

        impl Drop for TrackedPayload {
            fn drop(&mut self) {
                assert!(self.live.replace(false), "payload must be live before drop");
                self.drops.set(self.drops.get() + 1);
            }
        }

        let live = Rc::new(Cell::new(false));
        let drops = Rc::new(Cell::new(0));
        let mut loads = 0_u32;
        let mut load = |_: KagemushaPastaCycleParityV1, _: KagemushaPastaCycleArtifactKindV4| {
            assert!(
                !live.replace(true),
                "the previous raw role must drop before the next load"
            );
            loads += 1;
            Ok(TrackedPayload {
                bytes: vec![u8::try_from(loads).expect("small test load count")],
                live: Rc::clone(&live),
                drops: Rc::clone(&drops),
            })
        };

        let first = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            |bytes| Ok(bytes[0]),
        )
        .expect("first parsed role");
        assert_eq!(first, 1);
        assert!(!live.get());

        let error = with_kagemusha_artifact_payload_v4(
            &mut load,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
            |_| Err::<(), _>("expected parser failure".to_owned()),
        )
        .expect_err("parser failure must propagate");
        assert_eq!(error, "expected parser failure");
        assert!(!live.get());
        assert_eq!(loads, 2);
        assert_eq!(drops.get(), 2);
    }

    fn output_frontier_binding_builder(
        profile: [u64; 3],
        input_frontier: u64,
        result_frontier: u64,
        recipient_index: u64,
        change_index: u64,
        dummy_index: u64,
        topup_leaf_index: u64,
    ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp> {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;

        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(8)
            .use_lookup_bits(7);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let zero = ctx.load_witness(Fp::ZERO);
        let [is_init, is_append, is_redemption] =
            profile.map(|value| ctx.load_witness(Fp::from(value)));
        let input_frontier = ctx.load_witness(Fp::from(input_frontier));
        let result_frontier = ctx.load_witness(Fp::from(result_frontier));
        let mut output =
            [zero; crate::zk::kagemusha_v2::KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4];
        output[7] = ctx.load_witness(Fp::from(recipient_index));
        output[9] = ctx.load_witness(Fp::from(change_index));
        output[10] = ctx.load_witness(Fp::from(dummy_index));
        let bindings = crate::zk::kagemusha_step_transition::NamedTransitionBindings {
            operation: crate::zk::kagemusha_step_transition::AssignedKagemushaStepOperationV4 {
                limbs: vec![zero; KAGEMUSHA_STEP_OPERATION_LIMBS_V4]
                    .into_boxed_slice()
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("exact Kagemusha operation limb count")),
                fields: vec![zero; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4]
                    .into_boxed_slice()
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("exact Kagemusha operation field count")),
            },
            is_init,
            is_append,
            is_redemption,
            has_change: zero,
            input_root: zero,
            output_root: zero,
            input_next_zero_leaf_index: input_frontier,
            output_next_zero_leaf_index: result_frontier,
            input_commitments: [zero; 2],
            input_nullifiers: [zero; 2],
            recipient_commitment: zero,
            change_commitment: zero,
            statement_digest_limbs: [zero; 8],
            init_payer_tag_limbs: [zero; 8],
            init_operation_tag_limbs: [zero; 8],
        };
        let topup_leaf_index = ctx.load_witness(Fp::from(topup_leaf_index));
        constrain_kagemusha_output_frontier_v4(ctx, &range, &bindings, &output, topup_leaf_index);
        builder.calculate_params(Some(9));
        builder
    }

    fn assert_frontier_binding(
        expected_satisfied: bool,
        profile: [u64; 3],
        input: u64,
        result: u64,
        recipient: u64,
        change: u64,
        dummy: u64,
        topup: u64,
    ) {
        let builder = output_frontier_binding_builder(
            profile, input, result, recipient, change, dummy, topup,
        );
        let verification =
            halo2_proofs::dev::MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("frontier binding mock prover")
                .verify();
        assert_eq!(verification.is_ok(), expected_satisfied);
    }

    #[test]
    fn v4_eq_frontier_copy_constraints_reject_every_index_substitution() {
        assert_frontier_binding(true, [1, 0, 0], 0, 8, 7, 0, 8, 7);
        assert_frontier_binding(false, [1, 0, 0], 0, 8, 7, 0, 8, 6);

        assert_frontier_binding(true, [0, 1, 0], 7, 8, 7, 0, 8, 0);
        assert_frontier_binding(false, [0, 1, 0], 7, 8, 6, 0, 8, 0);

        assert_frontier_binding(true, [0, 0, 1], 7, 8, 0, 7, 8, 0);
        assert_frontier_binding(false, [0, 0, 1], 7, 8, 0, 6, 8, 0);

        assert_frontier_binding(false, [0, 1, 0], 7, 9, 7, 0, 8, 0);
    }

    #[test]
    fn v4_params_reject_default_k12_and_stale_public_layout() {
        assert!(KagemushaStepCircuitParamsV4::default().validate().is_err());

        let valid = valid_step_circuit_params_v4();
        let layout = valid.validate().expect("valid V4 lower-bound layout");
        assert_eq!(layout.accumulator_limbs, 36);
        assert_eq!(layout.instance_column_limbs, 64);
        assert_eq!(layout.live_selector_offset, 63);

        let mut k12 = valid.clone();
        k12.k = 12;
        assert!(k12.validate().is_err());

        let mut legacy_fixed_degree_layout = valid;
        legacy_fixed_degree_layout.public_input_limbs = 4_156;
        assert!(legacy_fixed_degree_layout.validate().is_err());
    }

    #[test]
    fn v5_generation_preflight_pins_compact_k16_key_sizes_before_allocation() {
        use halo2_proofs::halo2curves::pasta::EqAffine;

        let token = "0123456789abcdef".repeat(4);
        validate_kagemusha_generation_guard_record_v4(
            format!("{RESOURCE_GUARD_AUTH_MAGIC_V4}:{token}\n").as_bytes(),
            &token,
        )
        .expect("the exact guard record is accepted");
        assert!(
            validate_kagemusha_generation_guard_record_v4(
                format!("{RESOURCE_GUARD_AUTH_MAGIC_V4}:{token}").as_bytes(),
                &token,
            )
            .is_err(),
            "a partial guard record must fail closed"
        );
        assert!(
            validate_kagemusha_generation_guard_record_v4(
                format!("{RESOURCE_GUARD_AUTH_MAGIC_V4}:{}\n", "A".repeat(64)).as_bytes(),
                &"A".repeat(64),
            )
            .is_err(),
            "the capability token must use canonical lowercase hex"
        );
        assert!(
            checked_kagemusha_generation_product_v4(&[u64::MAX, 2], "test")
                .expect_err("working-set arithmetic must fail closed")
                .contains("overflow")
        );
        let reviewed = first_release_generation_params_v4();
        let shape = kagemusha_processed_key_shape_v4::<EqAffine>(&reviewed, "Eq")
            .expect("reviewed Eq encoding shape");
        assert_eq!(
            kagemusha_params_encoded_bytes_v4::<EqAffine>(reviewed.k, "Eq")
                .expect("reviewed parameter length"),
            4_194_372
        );
        assert_eq!(
            shape.verifier_key_bytes("Eq").expect("reviewed VK length"),
            35_018
        );
        assert_eq!(
            shape.proving_key_bytes("Eq").expect("reviewed PK length"),
            4_594_903_830
        );
        let preflight = preflight_kagemusha_generation_v4(&reviewed, &reviewed)
            .expect("compact k16 profile passes before ParamsIPA allocation");
        assert_eq!(preflight.layout.instance_column_limbs, 64);
        assert_eq!(preflight.estimated_peak_bytes, 9_747_562_496);
        assert!(preflight.estimated_peak_bytes <= KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4);
        assert!(
            preflight.estimated_peak_bytes <= KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5,
            "the reviewed staged lifecycle must remain within 12 GiB"
        );

        let mut stale = reviewed;
        stale.version = 4;
        assert!(preflight_kagemusha_generation_v4(&stale, &stale).is_err());
    }

    #[test]
    fn v4_generation_preflight_rejects_degree_21_before_parameter_allocation() {
        let mut degree_21 = first_release_generation_params_v4();
        degree_21.k = 21;
        degree_21.lookup_bits = 20;
        degree_21.public_input_limbs = 64;
        assert!(degree_21.validate().is_err());
        let error = preflight_kagemusha_generation_v4(&degree_21, &degree_21)
            .expect_err("degree-21 generation must fail before ParamsIPA allocation");
        assert!(error.contains("degree") || error.contains("layout"));
    }

    #[test]
    fn v4_generation_preflight_rejects_maximum_column_profile_before_allocation() {
        let mut maximum = first_release_generation_params_v4();
        maximum.num_advice_per_phase = vec![256, 256, 256];
        maximum.num_lookup_advice_per_phase = vec![256, 256, 256];
        maximum.num_fixed = 256;
        assert!(maximum.validate().is_err());
        assert!(preflight_kagemusha_generation_v4(&maximum, &maximum).is_err());
    }

    #[test]
    fn v4_generated_payload_size_gate_rejects_empty_and_corridor_limit() {
        validate_kagemusha_generated_payload_size_v4(1, "test payload")
            .expect("non-empty bounded payload");
        assert!(validate_kagemusha_generated_payload_size_v4(0, "test payload").is_err());

        let corridor_limit = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
            .expect("artifact corridor fits usize on supported hosts");
        validate_kagemusha_generated_payload_size_v4(corridor_limit - 1, "test payload")
            .expect("largest admitted payload");
        assert!(
            validate_kagemusha_generated_payload_size_v4(corridor_limit, "test payload").is_err()
        );
    }

    fn v4_complete_stage_plan() -> Vec<scalar_lineage_v1::DeferredEquationStageShapeV4> {
        use scalar_lineage_v1::{DeferredEquationGateV4 as Gate, DeferredEquationStageShapeV4};

        [
            Gate::ParentCurrent { slot: 0 },
            Gate::ParentCarriedFold { slot: 0 },
            Gate::ParentLineageSelect { slot: 0 },
            Gate::ParentCurrent { slot: 1 },
            Gate::ParentCarriedFold { slot: 1 },
            Gate::ParentLineageSelect { slot: 1 },
            Gate::BranchFold,
            Gate::BranchSelect,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, gate)| DeferredEquationStageShapeV4 {
            range: index..index + 1,
            gate,
        })
        .collect()
    }

    #[test]
    fn v4_complete_stage_validator_rejects_omission_reorder_and_duplicate() {
        let stages = v4_complete_stage_plan();
        scalar_lineage_v1::validate_stage_shapes_v4(&stages, 8).expect("complete V4 stage plan");

        for omitted in 0..stages.len() {
            let mut candidate = stages.clone();
            candidate.remove(omitted);
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted omission {omitted}"
            );
        }

        for swapped in 0..stages.len() - 1 {
            let mut candidate = stages.clone();
            candidate.swap(swapped, swapped + 1);
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted reorder at {swapped}"
            );
        }

        for duplicated in 0..stages.len() - 1 {
            let mut candidate = stages.clone();
            candidate[duplicated + 1].gate = candidate[duplicated].gate;
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted duplicate at {duplicated}"
            );
        }
    }

    #[test]
    fn v4_every_enabled_stage_is_covered_by_a_present_complete_join() {
        use scalar_lineage_v1::DeferredEquationGateV4 as Gate;

        let stages = v4_complete_stage_plan();
        for parent_count in 0..=2 {
            let slot_present = [parent_count >= 1, parent_count == 2];
            let parent_has_carried = [true, false];
            for stage in &stages {
                let enabled = match stage.gate {
                    Gate::ParentCurrent { slot } | Gate::ParentLineageSelect { slot } => {
                        slot_present[slot]
                    }
                    Gate::ParentCarriedFold { slot } => {
                        slot_present[slot] && parent_has_carried[slot]
                    }
                    Gate::BranchFold => slot_present[1],
                    Gate::BranchSelect => slot_present[0],
                };
                if enabled {
                    assert!(
                        slot_present[0]
                            && scalar_lineage_v1::validate_stage_shapes_v4(&stages, 8).is_ok()
                            && stages.iter().any(|candidate| candidate == stage),
                        "enabled {:?} is not covered for parent count {parent_count}",
                        stage.gate
                    );
                }
            }
        }
    }

    #[test]
    fn v4_host_deferred_audit_bytes_bind_complete_one_parent_branch_select() {
        use halo2_proofs::halo2curves::{group::prime::PrimeCurveAffine as _, pasta::EqAffine};

        use crate::zk::kagemusha_cycle_loader::{
            DeferredEquationWitness, KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V5,
            KAGEMUSHA_DEFERRED_AUDIT_VERSION_V5,
        };

        let source = EqAffine::generator();
        let coefficients = [3_u64, 5, 7, 11, 13, 17, 19, 23];
        let witness = DeferredEquationWitness::<EqAffine> {
            sources: vec![source],
            equations: coefficients
                .map(|coefficient| vec![(0, Fp::from(coefficient))])
                .to_vec(),
        };
        let stages = v4_complete_stage_plan();

        let expected_bytes = |selectors: [u8; 8], coefficients: [u64; 8]| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V5);
            bytes.push(0);
            bytes.extend_from_slice(&KAGEMUSHA_DEFERRED_AUDIT_VERSION_V5.to_le_bytes());
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&8_u32.to_le_bytes());
            let coordinates: Option<snark_verifier::util::arithmetic::Coordinates<EqAffine>> =
                source.coordinates().into();
            let coordinates = coordinates.expect("generator has affine coordinates");
            bytes.extend_from_slice(coordinates.x().to_repr().as_ref());
            bytes.extend_from_slice(coordinates.y().to_repr().as_ref());
            for ((gate_tag, coefficient), selector) in [1_u32, 3, 5, 2, 4, 6, 7, 8]
                .into_iter()
                .zip(coefficients)
                .zip(selectors)
            {
                bytes.extend_from_slice(&gate_tag.to_le_bytes());
                bytes.push(selector);
                bytes.extend_from_slice(&1_u32.to_le_bytes());
                bytes.extend_from_slice(&0_u32.to_le_bytes());
                bytes.extend_from_slice(Fp::from(coefficient).to_repr().as_ref());
            }
            bytes
        };

        let one_parent = kagemusha_deferred_audit_public_words_v5(&witness, &stages, 1, [1, 0])
            .expect("serialize complete one-parent V5 audit");
        assert_eq!(
            one_parent[0],
            kagemusha_sha256_public_words(
                Sha256::digest(expected_bytes([1, 1, 1, 0, 0, 0, 0, 1], coefficients)).into()
            )
        );
        assert_ne!(one_parent[0], [0; 8]);
        assert_eq!(one_parent[1], [0; 8]);

        let mut tampered = witness.clone();
        tampered.equations[7] = vec![(0, Fp::from(29))];
        let tampered_one_parent =
            kagemusha_deferred_audit_public_words_v5(&tampered, &stages, 1, [1, 0])
                .expect("serialize BranchSelect-tampered V5 audit");
        assert_ne!(one_parent[0], tampered_one_parent[0]);
        assert_eq!(tampered_one_parent[1], [0; 8]);

        let two_parent = kagemusha_deferred_audit_public_words_v5(&witness, &stages, 2, [1, 1])
            .expect("serialize complete two-parent V5 audit");
        assert_eq!(two_parent[0], two_parent[1]);
        assert_eq!(
            two_parent[0],
            kagemusha_sha256_public_words(
                Sha256::digest(expected_bytes([1; 8], coefficients)).into()
            )
        );

        assert_eq!(
            kagemusha_deferred_audit_public_words_v5(&witness, &stages, 0, [0, 0])
                .expect("serialize absent V4 slots"),
            [[0; 8]; 2]
        );
    }

    fn v4_reciprocal_audit_builder<C>(
        witness: &crate::zk::kagemusha_cycle_loader::DeferredEquationWitness<C>,
        stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
        current_parent_count: u32,
        expected_words: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::Base>
    where
        C: halo2_base::utils::CurveAffineExt,
        C::Base: halo2_base::utils::BigPrimeField
            + halo2_base::utils::ScalarField
            + ff::WithSmallOrderMulGroup<3>,
        C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_ecc::fields::fp::FpChip;

        use crate::zk::kagemusha_cycle_loader::{LIMB_BITS, LIMBS};

        let mut builder = BaseCircuitBuilder::<C::Base>::new(false)
            .use_k(17)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
        let mut ctx = mem::take(builder.pool(0));
        let current_parent_count = ctx
            .main()
            .load_witness(C::Base::from(u64::from(current_parent_count)));
        let parent_counts = [
            ctx.main().load_witness(C::Base::ZERO),
            ctx.main().load_witness(C::Base::ZERO),
        ];
        let expected_words = expected_words.map(|words| {
            kagemusha_u32_words_to_u128_chunks_v5(&words)
                .map(|chunk| ctx.main().load_witness(C::Base::from_u128(chunk)))
        });
        let mut sha_jobs = KagemushaSha256JobsV4::default();
        constrain_reciprocal_point_audit_identity_v4::<C>(
            &mut ctx,
            &mut sha_jobs,
            &base,
            &scalar,
            witness,
            stages,
            current_parent_count,
            parent_counts,
            [&expected_words[0], &expected_words[1]],
            KagemushaDeferredMsmV5::GenericTest,
        )
        .expect("complete V4 reciprocal audit shape");
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        builder
    }

    #[test]
    fn v4_one_parent_branch_select_reciprocal_substitution_fails_for_both_parities() {
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::{
                group::prime::PrimeCurveAffine as _,
                pasta::{EpAffine, EqAffine},
            },
        };

        use crate::zk::kagemusha_cycle_loader::DeferredEquationWitness;

        fn assert_join<C>(source: C)
        where
            C: halo2_base::utils::CurveAffineExt,
            C::Base: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
            C::ScalarExt: halo2_base::utils::BigPrimeField,
        {
            let stages = v4_complete_stage_plan();
            let original = DeferredEquationWitness::<C> {
                sources: vec![source],
                equations: vec![vec![(0, C::ScalarExt::ZERO)]; 8],
            };
            let expected = kagemusha_deferred_audit_public_words_v5(&original, &stages, 1, [0, 0])
                .expect("serialize original one-parent audit");
            assert_ne!(expected[0], [0; 8]);
            assert_eq!(expected[1], [0; 8]);

            let valid = v4_reciprocal_audit_builder(&original, &stages, 1, expected);
            MockProver::run(valid.config_params.k as u32, &valid, vec![])
                .expect("valid complete reciprocal audit prover")
                .assert_satisfied();

            let mut wrong_absent_slot = expected;
            wrong_absent_slot[1] = expected[0];
            let wrong_absent_slot =
                v4_reciprocal_audit_builder(&original, &stages, 1, wrong_absent_slot);
            assert!(
                MockProver::run(
                    wrong_absent_slot.config_params.k as u32,
                    &wrong_absent_slot,
                    vec![],
                )
                .expect("non-canonical one-parent reciprocal audit prover")
                .verify()
                .is_err(),
                "a one-parent step must expose canonical zero in slot one"
            );

            let two_parent =
                kagemusha_deferred_audit_public_words_v5(&original, &stages, 2, [0, 0])
                    .expect("serialize original two-parent audit");
            assert_ne!(two_parent[0], [0; 8]);
            assert_eq!(two_parent[0], two_parent[1]);
            let valid_two_parent = v4_reciprocal_audit_builder(&original, &stages, 2, two_parent);
            MockProver::run(
                valid_two_parent.config_params.k as u32,
                &valid_two_parent,
                vec![],
            )
            .expect("valid two-parent reciprocal audit prover")
            .assert_satisfied();

            let mut wrong_second_digest = two_parent;
            wrong_second_digest[1] = [0; 8];
            let wrong_second_digest =
                v4_reciprocal_audit_builder(&original, &stages, 2, wrong_second_digest);
            assert!(
                MockProver::run(
                    wrong_second_digest.config_params.k as u32,
                    &wrong_second_digest,
                    vec![],
                )
                .expect("mismatched two-parent reciprocal audit prover")
                .verify()
                .is_err(),
                "both present parent slots must expose the same complete digest"
            );

            let mut substituted = original;
            substituted.sources.push(-source);
            substituted.equations[7] = vec![(0, C::ScalarExt::ONE), (1, C::ScalarExt::ONE)];
            let adversarial = v4_reciprocal_audit_builder(&substituted, &stages, 1, expected);
            assert!(
                MockProver::run(adversarial.config_params.k as u32, &adversarial, vec![])
                    .expect("adversarial complete reciprocal audit prover")
                    .verify()
                    .is_err(),
                "a satisfiable BranchSelect substitution must fail the scalar-audit join"
            );
        }

        assert_join(EqAffine::generator());
        assert_join(EpAffine::generator());
    }

    fn v4_accumulator(
        parity: KagemushaPastaCycleParityV1,
        k: u32,
    ) -> KagemushaIpaAccumulatorWireV4 {
        use halo2_proofs::halo2curves::{
            group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
            pasta::{EpAffine, EqAffine},
        };

        let folded_generator = match parity {
            KagemushaPastaCycleParityV1::StepEq => {
                let mut bytes = [0; 32];
                bytes.copy_from_slice(EqAffine::generator().to_bytes().as_ref());
                bytes
            }
            KagemushaPastaCycleParityV1::StepEp => {
                let mut bytes = [0; 32];
                bytes.copy_from_slice(EpAffine::generator().to_bytes().as_ref());
                bytes
            }
        };
        KagemushaIpaAccumulatorWireV4 {
            version: crate::zk::kagemusha_accumulation::KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count: k,
            round_challenges: vec![[0; 32]; usize::try_from(k).expect("test degree fits")],
            folded_generator,
        }
    }

    fn v4_fold(k: u32, tag: u8, has_parent: bool) -> KagemushaIpaAccumulationProofV4 {
        if !has_parent {
            return KagemushaIpaAccumulationProofV4::initialization(k)
                .expect("supported initialization degree");
        }
        let len = crate::zk::kagemusha_accumulation::kagemusha_ipa_accumulation_proof_bytes_v4(k)
            .expect("supported fold degree");
        KagemushaIpaAccumulationProofV4::from_fold_bytes(k, vec![tag; len])
            .expect("fixed-size fold fixture")
    }

    fn v4_public_inputs(step: u32, parent_count: u32) -> KagemushaPastaCyclePublicInputsV4 {
        assert!((1..=3).contains(&step));
        assert!(parent_count <= 2);
        let k = valid_step_circuit_params_v4().k;
        let mut parent_states = std::array::from_fn(|_| {
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2]
        });
        let mut parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        let mut parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        let eq_deferred_sha256 = std::array::from_fn(|index| 0xE410_0000 | index as u32 + 1);
        let ep_deferred_sha256 = std::array::from_fn(|index| 0xE420_0000 | index as u32 + 1);
        for slot in 0..usize::try_from(parent_count).expect("parent count fits") {
            parent_states[slot] =
                exact_state(step - parent_count + u32::try_from(slot).expect("slot fits"));
            parent_eq_deferred_sha256[slot] = eq_deferred_sha256;
            parent_ep_deferred_sha256[slot] = ep_deferred_sha256;
        }
        let has_parent = parent_count != 0;
        KagemushaPastaCyclePublicInputsV4 {
            public_statement_digest: std::array::from_fn(|index| {
                0xA410_0000 | step << 8 | index as u32 + 1
            }),
            operation: KagemushaStepOperationVectorV4::default(),
            parent_count,
            parent_states,
            result_state: exact_state(step),
            manifest_sha256: std::array::from_fn(|index| 0xA500_0000 | index as u32 + 1),
            step_eq_compiled_protocol_sha256: [0xC1C1_C1C1; 8],
            step_ep_compiled_protocol_sha256: [0xC2C2_C2C2; 8],
            parent_eq_lineage_accumulator: has_parent
                .then(|| v4_accumulator(KagemushaPastaCycleParityV1::StepEq, k)),
            parent_ep_lineage_accumulator: has_parent
                .then(|| v4_accumulator(KagemushaPastaCycleParityV1::StepEp, k)),
            parent_eq_deferred_sha256,
            parent_ep_deferred_sha256,
            live_selector: KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
        }
    }

    fn v4_pair(step: u32, parent_count: u32) -> KagemushaPastaCycleProofPairV4 {
        let params = valid_step_circuit_params_v4();
        let has_parent = parent_count != 0;
        let private_inputs = v4_public_inputs(step, parent_count);
        KagemushaPastaCycleProofPairV4 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
            proof_step_count: step,
            public_inputs: KagemushaCompactPublicInputsV5::from_private(&private_inputs, step),
            step_eq_proof_bytes: vec![0x41; params.max_parent_proof_bytes as usize],
            step_ep_proof_bytes: vec![0x42; params.max_parent_proof_bytes as usize],
            step_eq_accumulation_proof: v4_fold(params.k, 0xE1, has_parent),
            step_ep_accumulation_proof: v4_fold(params.k, 0xE2, has_parent),
        }
    }

    #[test]
    fn v4_manifest_preserves_exact_little_endian_state_limbs() {
        let params = valid_step_circuit_params_v4();
        let expected = std::array::from_fn(|index| 0xA500_0000 | index as u32 + 1);
        let mut manifest_bytes = [0_u8; 32];
        for (chunk, limb) in manifest_bytes.chunks_exact_mut(4).zip(expected) {
            chunk.copy_from_slice(&limb.to_le_bytes());
        }

        let exact = kagemusha_exact_u32_public_limbs(manifest_bytes);
        assert_eq!(exact, expected);
        assert_ne!(exact, kagemusha_sha256_public_words(manifest_bytes));

        let mut public_inputs = v4_public_inputs(1, 0);
        public_inputs.manifest_sha256 = exact;
        public_inputs
            .validate(1, &params)
            .expect("exact manifest limbs match the result-state binding");

        public_inputs.manifest_sha256 = kagemusha_sha256_public_words(manifest_bytes);
        assert!(public_inputs.validate(1, &params).is_err());
    }

    #[test]
    fn v5_eq_and_ep_public_columns_share_the_result_state_commitment() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let params = valid_step_circuit_params_v4();
        let mut public_inputs = v4_public_inputs(1, 0);
        let original_commitment = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            &public_inputs.result_state,
        );
        public_inputs.result_state[crate::zk::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX] = 37;
        let eq = public_inputs
            .instance_column::<Fp>(1, &params, KagemushaPastaCycleParityV1::StepEq)
            .expect("Eq public column");
        let ep = public_inputs
            .instance_column::<Fq>(1, &params, KagemushaPastaCycleParityV1::StepEp)
            .expect("Ep public column");
        let expected = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            &public_inputs.result_state,
        );
        assert_ne!(expected, original_commitment);
        for (index, expected) in expected.into_iter().enumerate() {
            let offset = KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + index;
            assert_eq!(eq[offset], Fp::from_u128(expected));
            assert_eq!(ep[offset], Fq::from_u128(expected));
        }
    }

    #[test]
    fn v4_public_boundary_rejects_non_live_and_bootstrap_pairs() {
        let params = valid_step_circuit_params_v4();
        let maximum =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4;
        let mut selector_two = v4_public_inputs(1, 0);
        selector_two.live_selector = 2;
        assert!(selector_two.validate(1, &params).is_err());

        let mut bootstrap = v4_public_inputs(1, 0);
        bootstrap.live_selector = KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4;
        assert!(bootstrap.validate(1, &params).is_err());

        for selector in [KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4, 2] {
            let mut pair = v4_pair(1, 0);
            pair.validate(&params, &params, maximum)
                .expect("live compact pair baseline");
            *pair
                .public_inputs
                .common_header
                .last_mut()
                .expect("compact header carries its selector") = u128::from(selector);
            assert!(pair.validate(&params, &params, maximum).is_err());
            let encoded =
                norito::encode_canonical(&pair).expect("encode adversarial V4 pair canonically");
            assert!(
                validate_kagemusha_proof_pair_measurement_v4(&encoded, &params, &params, maximum,)
                    .is_err(),
                "the public opaque-pair parser must reject selector {selector}"
            );
        }
    }

    #[test]
    fn v4_audit_derivation_prepass_accepts_only_blank_derived_join_slots() {
        let params = valid_step_circuit_params_v4();
        let mut public_inputs = v4_public_inputs(2, 1);
        public_inputs
            .validate(2, &params)
            .expect("proof inputs require authenticated deferred-audit joins");
        assert!(
            public_inputs
                .validate_for_audit_derivation_prepass(2, &params)
                .is_err(),
            "audit derivation prepass must reject a preselected join digest"
        );

        public_inputs.parent_eq_deferred_sha256[0] = [0; 8];
        public_inputs.parent_ep_deferred_sha256[0] = [0; 8];
        public_inputs
            .validate_for_audit_derivation_prepass(2, &params)
            .expect("audit derivation prepass accepts a blank derived-join parent slot");
        assert!(
            public_inputs.validate(2, &params).is_err(),
            "a live proof must require every derived parent audit join"
        );
    }

    #[test]
    fn v4_circuit_mode_rejects_selector_two_nonzero_bootstrap_and_live_all_zero() {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_proofs::dev::MockProver;

        fn builder(
            mode: KagemushaStepPublicModeV4,
        ) -> (BaseCircuitBuilder<Fp>, Vec<Fp>, u32, usize) {
            let layout =
                KagemushaPastaPublicLayoutV4::for_ipa_round_count(valid_step_circuit_params_v4().k)
                    .expect("test public layout");
            let public_len = usize::try_from(layout.instance_column_limbs)
                .expect("test public length fits usize");
            let live_offset =
                usize::try_from(layout.live_selector_offset).expect("test live offset fits usize");
            let mut semantic = vec![Fp::ZERO; public_len];
            semantic[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5] =
                Fp::from(u64::from(KAGEMUSHA_COMPACT_PROFILE_VERSION_V5));
            semantic[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5] = Fp::ONE;
            semantic[live_offset] = Fp::ONE;
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(17)
                .use_lookup_bits(8)
                .use_instance_columns(1);
            assign_kagemusha_public_mode_v4(&mut builder, semantic.clone(), &layout, mode)
                .expect("assign test V4 public mode");
            let params = builder.calculate_params(Some(8));
            (
                builder,
                semantic,
                u32::try_from(params.k).expect("small k"),
                live_offset,
            )
        }

        let (bootstrap, _, bootstrap_k, live_offset) =
            builder(KagemushaStepPublicModeV4::Bootstrap);
        let mut zero = vec![Fp::ZERO; live_offset + 1];
        MockProver::run(bootstrap_k, &bootstrap, vec![zero.clone()])
            .expect("bootstrap public-mode prover")
            .assert_satisfied();

        zero[live_offset] = Fp::from(2);
        assert!(
            MockProver::run(bootstrap_k, &bootstrap, vec![zero.clone()])
                .expect("selector-two public-mode prover")
                .verify()
                .is_err()
        );
        zero[live_offset] = Fp::ZERO;
        zero[0] = Fp::ONE;
        assert!(
            MockProver::run(bootstrap_k, &bootstrap, vec![zero])
                .expect("nonzero-bootstrap public-mode prover")
                .verify()
                .is_err()
        );

        let (live, live_instance, live_k, _) = builder(KagemushaStepPublicModeV4::Live);
        MockProver::run(live_k, &live, vec![live_instance])
            .expect("live public-mode prover")
            .assert_satisfied();
        assert!(
            MockProver::run(live_k, &live, vec![vec![Fp::ZERO; live_offset + 1]])
                .expect("live-all-zero public-mode prover")
                .verify()
                .is_err()
        );
    }

    fn v4_bootstrap() -> KagemushaStepBootstrapV4 {
        let params = valid_step_circuit_params_v4();
        let layout = params.validate().expect("valid V4 params");
        KagemushaStepBootstrapV4 {
            version: KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4,
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_params_sha256: params.sha256().expect("identify V4 params"),
            compiled_protocol_structure_sha256: [0x51; 32],
            bootstrap_compiled_protocol_sha256: [0x52; 32],
            circuit_break_points: vec![vec![1]],
            parent_slot: KagemushaStepBootstrapParentSlotV4 {
                instances: vec![vec![
                    0;
                    usize::try_from(layout.instance_column_limbs)
                        .expect("public length fits")
                ]],
                ordinary_proof_bytes: vec![0x53; params.max_parent_proof_bytes as usize],
                carried_lineage: v4_accumulator(KagemushaPastaCycleParityV1::StepEq, params.k),
                post_proof_fold: v4_fold(params.k, 0x54, true),
            },
            branch_merge_fold: v4_fold(params.k, 0x55, true),
        }
    }

    #[test]
    fn v4_bootstrap_is_canonical_manifest_independent_and_profile_bound() {
        let params = valid_step_circuit_params_v4();
        let structure = [0x51; 32];
        let bootstrap = v4_bootstrap();
        bootstrap
            .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure)
            .expect("valid manifest-independent bootstrap");
        let encoded = bootstrap
            .encode_authenticated(&params, KagemushaPastaCycleParityV1::StepEq, structure)
            .expect("encode bootstrap");
        assert_eq!(
            KagemushaStepBootstrapV4::decode_authenticated(
                &encoded,
                &params,
                KagemushaPastaCycleParityV1::StepEq,
                structure,
            )
            .expect("decode canonical bootstrap"),
            bootstrap
        );
        let alternate = encode_with_alternate_norito_layout(&bootstrap);
        assert_ne!(alternate, encoded);
        assert_eq!(
            KagemushaStepBootstrapV4::decode_authenticated(
                &alternate,
                &params,
                KagemushaPastaCycleParityV1::StepEq,
                structure,
            )
            .expect_err("alternate-layout bootstrap must be rejected"),
            "Kagemusha V4 bootstrap payload is not canonical Norito"
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let ambient_encoded = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            bootstrap
                .encode_authenticated(&params, KagemushaPastaCycleParityV1::StepEq, structure)
                .expect("encode bootstrap under alternate ambient layout")
        };
        assert_eq!(ambient_encoded, encoded);

        let mut missing_break_points = bootstrap.clone();
        missing_break_points.circuit_break_points.clear();
        assert!(
            missing_break_points
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "an authenticated runtime bootstrap must carry its keygen breakpoints"
        );
        let mut wrong_phase_count = bootstrap.clone();
        wrong_phase_count.circuit_break_points.push(vec![]);
        assert!(
            wrong_phase_count
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "breakpoints for a different phase shape must fail closed"
        );
        let mut non_increasing = bootstrap.clone();
        non_increasing.circuit_break_points = vec![vec![2, 2]];
        assert!(
            non_increasing
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "non-increasing cumulative breakpoints must fail closed"
        );
        let mut out_of_domain = bootstrap.clone();
        out_of_domain.circuit_break_points = vec![vec![
            u32::try_from(kagemusha_break_point_max_rows_v5(&params).expect("usable rows"))
                .expect("k16 rows fit u32"),
        ]];
        assert!(
            out_of_domain
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "an out-of-domain breakpoint segment must fail closed"
        );

        for mutation in [
            "version",
            "parity",
            "params_hash",
            "structure",
            "bootstrap_identity",
            "nonzero_instance",
            "short_proof",
            "long_proof",
            "parent_fold",
            "branch_fold",
        ] {
            let mut candidate = bootstrap.clone();
            match mutation {
                "version" => candidate.version ^= 1,
                "parity" => candidate.parity = KagemushaPastaCycleParityV1::StepEp,
                "params_hash" => candidate.circuit_params_sha256[0] ^= 1,
                "structure" => candidate.compiled_protocol_structure_sha256[0] ^= 1,
                "bootstrap_identity" => candidate.bootstrap_compiled_protocol_sha256 = [0; 32],
                "nonzero_instance" => candidate.parent_slot.instances[0][0] = 1,
                "short_proof" => {
                    candidate.parent_slot.ordinary_proof_bytes.pop();
                }
                "long_proof" => candidate.parent_slot.ordinary_proof_bytes.push(0),
                "parent_fold" => {
                    candidate.parent_slot.post_proof_fold.bytes.pop();
                }
                "branch_fold" => {
                    candidate.branch_merge_fold.bytes.pop();
                }
                _ => unreachable!(),
            }
            assert!(
                candidate
                    .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure)
                    .is_err(),
                "bootstrap mutation {mutation} must fail"
            );
        }

        let wrong_profile = valid_step_circuit_params_for_k_v4(21);
        assert!(
            bootstrap
                .validate(
                    &wrong_profile,
                    KagemushaPastaCycleParityV1::StepEq,
                    structure,
                )
                .is_err()
        );
    }

    #[test]
    fn v4_pair_enforces_zero_one_two_parent_shapes_and_exact_bounds() {
        let params = valid_step_circuit_params_v4();
        let maximum =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4;
        for (step, parent_count) in [(1, 0), (2, 1), (3, 2)] {
            let pair = v4_pair(step, parent_count);
            let layout = pair
                .validate(&params, &params, maximum)
                .expect("valid V4 selector shape");
            assert_eq!(
                pair.public_inputs
                    .instance_column::<Fp>(&params, KagemushaPastaCycleParityV1::StepEq)
                    .expect("V4 instance column")
                    .len(),
                usize::try_from(layout.instance_column_limbs).expect("public length fits")
            );
            let bytes = pair
                .encode_authenticated(&params, &params, maximum)
                .expect("encode bounded pair");
            assert_eq!(
                KagemushaPastaCycleProofPairV4::decode_authenticated(
                    &bytes, &params, &params, maximum,
                )
                .expect("decode canonical pair"),
                pair
            );
            let alternate = encode_with_alternate_norito_layout(&pair);
            assert_ne!(alternate, bytes);
            assert_eq!(
                KagemushaPastaCycleProofPairV4::decode_authenticated(
                    &alternate, &params, &params, maximum,
                )
                .expect_err("alternate-layout pair must be rejected"),
                "Kagemusha V4 proof pair is not canonical Norito"
            );
            assert!(
                pair.validate(
                    &params,
                    &params,
                    u32::try_from(bytes.len() - 1).expect("fixture size fits"),
                )
                .is_err(),
                "pair cap below the canonical payload must fail"
            );
        }

        let mut invalid_count = v4_pair(3, 2);
        invalid_count.public_inputs.common_header[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5] = 3;
        assert!(invalid_count.validate(&params, &params, maximum).is_err());

        let canonical = v4_pair(3, 2);
        let mut bundle_ordered_private = v4_public_inputs(3, 2);
        assert!(bundle_ordered_private.parent_states[0] < bundle_ordered_private.parent_states[1]);
        bundle_ordered_private.parent_states.swap(0, 1);
        bundle_ordered_private.parent_eq_deferred_sha256.swap(0, 1);
        bundle_ordered_private.parent_ep_deferred_sha256.swap(0, 1);
        assert!(bundle_ordered_private.parent_states[0] > bundle_ordered_private.parent_states[1]);
        bundle_ordered_private
            .validate(3, &params)
            .expect("private parent slots preserve bundle-digest order");
        let mut bundle_ordered = v4_pair(3, 2);
        bundle_ordered.public_inputs =
            KagemushaCompactPublicInputsV5::from_private(&bundle_ordered_private, 3);
        let parent_commitments = KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5;
        assert_eq!(
            &bundle_ordered.public_inputs.common_header[parent_commitments..parent_commitments + 2],
            &canonical.public_inputs.common_header[parent_commitments + 2..parent_commitments + 4],
        );
        assert_eq!(
            &bundle_ordered.public_inputs.common_header
                [parent_commitments + 2..parent_commitments + 4],
            &canonical.public_inputs.common_header[parent_commitments..parent_commitments + 2],
        );
        assert_eq!(
            bundle_ordered.public_inputs.parent_eq_deferred_chunks[0],
            canonical.public_inputs.parent_eq_deferred_chunks[1],
        );
        assert_eq!(
            bundle_ordered.public_inputs.parent_ep_deferred_chunks[0],
            canonical.public_inputs.parent_ep_deferred_chunks[1],
        );
        bundle_ordered
            .validate(&params, &params, maximum)
            .expect("V5 compact parent slots follow bundle-digest order, not state-vector order");

        let mut short = v4_pair(2, 1);
        short.step_eq_proof_bytes.pop();
        assert!(short.validate(&params, &params, maximum).is_err());
        let mut long = v4_pair(2, 1);
        long.step_ep_proof_bytes.push(0);
        assert!(long.validate(&params, &params, maximum).is_err());

        let wrong_layout = valid_step_circuit_params_for_k_v4(21);
        assert!(
            v4_pair(1, 0)
                .validate(&params, &wrong_layout, maximum)
                .is_err()
        );
        let mut missing_manifest = v4_pair(2, 1);
        missing_manifest.public_inputs.common_header[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
            .fill(0);
        assert!(
            missing_manifest
                .validate(&params, &params, maximum)
                .is_err()
        );
    }

    #[test]
    fn v4_missing_bootstrap_rejects_without_generating_padding() {
        assert!(require_kagemusha_step_bootstrap_v4(None, "Eq").is_err());
        assert!(require_kagemusha_step_bootstrap_v4(None, "Ep").is_err());
    }

    /// Keep the exact same-field Pasta recursion tuples executable.
    ///
    /// An Eq IPA proof uses `ParamsIPA<EqAffine>` and has scalar field `Fp`, so
    /// its direct Axiom circuit verifier must also be an `Fp` circuit with a
    /// `Halo2Loader<EqAffine, BaseFieldEccChip<EqAffine>>`. The reciprocal Ep
    /// tuple is `ParamsIPA<EpAffine>` / `Fq` /
    /// `Halo2Loader<EpAffine, BaseFieldEccChip<EpAffine>>`. This test is a
    /// compile-time guard against accidentally diagnosing that supported path
    /// as a Pasta trait mismatch.
    #[test]
    fn same_field_pasta_loader_type_tuples_compile() {
        use halo2_base::gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder};
        use halo2_ecc::{ecc::BaseFieldEccChip, fields::fp::FpChip};
        use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine};
        use snark_verifier::loader::halo2::Halo2Loader;

        const LIMB_BITS: usize = 86;
        const LIMBS: usize = 3;
        let seed = BaseCircuitParams {
            k: 12,
            num_advice_per_phase: vec![1],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: Some(11),
            num_instance_columns: 1,
        };

        let mut eq_outer = BaseCircuitBuilder::<Fp>::new(false).use_params(seed.clone());
        let eq_range = eq_outer.range_chip();
        let eq_base = FpChip::<Fp, Fq>::new(&eq_range, LIMB_BITS, LIMBS);
        let eq_loader = Halo2Loader::new(
            BaseFieldEccChip::<EqAffine>::new(&eq_base),
            mem::take(eq_outer.pool(0)),
        );
        fn require_eq_tuple(_: &Rc<Halo2Loader<EqAffine, BaseFieldEccChip<'_, EqAffine>>>) {}
        require_eq_tuple(&eq_loader);
        *eq_outer.pool(0) = eq_loader.take_ctx();

        let mut ep_outer = BaseCircuitBuilder::<Fq>::new(false).use_params(seed);
        let ep_range = ep_outer.range_chip();
        let ep_base = FpChip::<Fq, Fp>::new(&ep_range, LIMB_BITS, LIMBS);
        let ep_loader = Halo2Loader::new(
            BaseFieldEccChip::<EpAffine>::new(&ep_base),
            mem::take(ep_outer.pool(0)),
        );
        fn require_ep_tuple(_: &Rc<Halo2Loader<EpAffine, BaseFieldEccChip<'_, EpAffine>>>) {}
        require_ep_tuple(&ep_loader);
        *ep_outer.pool(0) = ep_loader.take_ctx();
    }

    #[test]
    fn protocol_private_enum_projection_is_explicit_and_fail_closed() {
        use ciborium::value::Value;

        assert_eq!(
            encode_common_polynomial_value(Value::Text("Identity".to_owned()))
                .expect("identity common polynomial"),
            vec![1, 0]
        );
        let mut expected_lagrange = vec![1, 1];
        expected_lagrange.extend_from_slice(&(-7_i32).to_le_bytes());
        assert_eq!(
            encode_common_polynomial_value(Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Integer((-7_i64).into()),
            )]))
            .expect("Lagrange common polynomial"),
            expected_lagrange
        );
        for malformed in [
            Value::Text("Unknown".to_owned()),
            Value::Map(Vec::new()),
            Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Text("zero".to_owned()),
            )]),
            Value::Map(vec![(
                Value::Text("Unknown".to_owned()),
                Value::Integer(0.into()),
            )]),
            Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Integer(i64::MAX.into()),
            )]),
        ] {
            assert!(encode_common_polynomial_value(malformed).is_err());
        }

        assert_eq!(encode_linearization_value(Value::Null), Ok(0));
        assert_eq!(
            encode_linearization_value(Value::Text("WithoutConstant".to_owned())),
            Ok(1)
        );
        assert_eq!(
            encode_linearization_value(Value::Text("MinusVanishingTimesQuotient".to_owned())),
            Ok(2)
        );
        assert!(
            encode_linearization_value(Value::Text("Unknown".to_owned())).is_err(),
            "an upstream enum extension requires an identity-version review"
        );
    }

    #[test]
    fn universal_protocol_bootstrap_converges_for_the_same_base_config() {
        use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};
        use halo2_proofs::{
            SerdeFormat,
            halo2curves::pasta::EqAffine,
            plonk::{keygen_pk, keygen_vk},
            poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
        };
        use snark_verifier::system::halo2::{Config, compile};

        let base_circuit_params = halo2_base::gates::circuit::BaseCircuitParams {
            k: 8,
            num_advice_per_phase: vec![2],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: Some(7),
            num_instance_columns: 1,
        };
        let target = KagemushaUniversalProtocolTargetV1 {
            base_circuit_params: base_circuit_params.clone(),
            instance_column_lengths: vec![1],
        };
        let params = ParamsIPA::<EqAffine>::new(8);
        let bootstrap_circuit = KagemushaProtocolBootstrapCircuit {
            params: base_circuit_params.clone(),
            marker: std::marker::PhantomData,
        };
        let separate_vk = kagemusha_bootstrap_verifying_key_v1(&params, &target)
            .expect("separate bootstrap VK generation");
        let separate_pk = keygen_pk(&params, separate_vk, &bootstrap_circuit)
            .expect("separate bootstrap PK generation");
        let combined_pk = kagemusha_bootstrap_proving_key_v1(&params, &target, &bootstrap_circuit)
            .expect("single-synthesis bootstrap PK generation");
        assert_eq!(
            separate_pk.to_bytes(SerdeFormat::Processed),
            combined_pk.to_bytes(SerdeFormat::Processed),
            "single-synthesis bootstrap keygen must preserve the exact processed key"
        );
        drop(combined_pk);
        drop(separate_pk);
        let bootstrap = kagemusha_bootstrap_compiled_protocol_v1(&params, &target)
            .expect("deterministic bootstrap protocol");
        assert_eq!(bootstrap.num_instance, vec![1]);
        assert!(
            bootstrap.instance_committing_key.is_none(),
            "canonical V4 compilation must evaluate public instances directly"
        );
        let bootstrap_structure = kagemusha_compiled_protocol_structure_sha256(
            &bootstrap,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("canonical bootstrap structure");
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("repeat canonical bootstrap structure"),
            "the explicit protocol descriptor must be stable"
        );
        assert_ne!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEp,
            )
            .expect("opposite-parity protocol descriptor"),
            "the same protocol bytes must remain parity-domain-separated"
        );

        let assert_structure_changes = |label: &str, protocol: &PlonkProtocol<EqAffine>| {
            assert_ne!(
                bootstrap_structure,
                kagemusha_compiled_protocol_structure_sha256(
                    protocol,
                    KagemushaPastaCycleParityV1::StepEq,
                )
                .expect("mutated protocol structure"),
                "the {label} verifier-control-flow category must affect the descriptor"
            );
        };

        let mut changed_domain = bootstrap.clone();
        changed_domain.domain.k += 1;
        assert_structure_changes("domain", &changed_domain);

        let mut changed_instance_count = bootstrap.clone();
        changed_instance_count.num_instance.push(0);
        assert_structure_changes("instance count", &changed_instance_count);
        let mut changed_witness_count = bootstrap.clone();
        changed_witness_count.num_witness.push(1);
        assert_structure_changes("witness count", &changed_witness_count);
        let mut changed_challenge_count = bootstrap.clone();
        changed_challenge_count.num_challenge.push(1);
        assert_structure_changes("challenge count", &changed_challenge_count);

        let mut changed_evaluations = bootstrap.clone();
        changed_evaluations
            .evaluations
            .first_mut()
            .expect("compiled protocol has an evaluation")
            .poly += 1;
        assert_structure_changes("evaluation", &changed_evaluations);
        let mut changed_queries = bootstrap.clone();
        changed_queries
            .queries
            .first_mut()
            .expect("compiled protocol has an opening query")
            .rotation
            .0 += 1;
        assert_structure_changes("opening query", &changed_queries);

        let mut changed_quotient = bootstrap.clone();
        changed_quotient.quotient.chunk_degree += 1;
        assert_structure_changes("quotient", &changed_quotient);

        let bootstrap_vk = kagemusha_bootstrap_verifying_key_v1(&params, &target)
            .expect("deterministic bootstrap verifying key");
        let queried_instance_protocol = compile(
            &params,
            &bootstrap_vk,
            Config::ipa().with_num_instance(vec![1]),
        );
        assert!(
            queried_instance_protocol.instance_committing_key.is_some(),
            "the upstream IPA default remains queried-instance mode"
        );
        assert_structure_changes("queried-instance presence", &queried_instance_protocol);

        // `LinearizationStrategy` is intentionally not re-exported by the
        // pinned dependency. Its derived Ciborium representation still lets
        // this regression exercise the public protocol field without copying
        // the dependency's private enum into Iroha.
        let mut changed_linearization = bootstrap.clone();
        changed_linearization.linearization =
            ciborium::value::Value::Text("WithoutConstant".to_owned())
                .deserialized()
                .expect("deserialize explicit linearization variant");
        assert_structure_changes("linearization", &changed_linearization);

        let mut changed_accumulator_indices = bootstrap.clone();
        changed_accumulator_indices
            .accumulator_indices
            .push(vec![(0, 0)]);
        assert_structure_changes("accumulator indices", &changed_accumulator_indices);

        let mut changed_transcript_presence = bootstrap.clone();
        changed_transcript_presence.transcript_initial_state = None;
        assert_structure_changes("transcript presence", &changed_transcript_presence);

        let mut changed_preprocessed_length = bootstrap.clone();
        changed_preprocessed_length.preprocessed.pop();
        assert_structure_changes("preprocessed length", &changed_preprocessed_length);

        let bootstrap_identity = kagemusha_compiled_protocol_identity_sha256(
            &bootstrap,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("bootstrap identity");
        let mut changed_preprocessed_value = bootstrap.clone();
        changed_preprocessed_value.preprocessed[0] = EqAffine::identity();
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &changed_preprocessed_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("structure with changed preprocessed value"),
            "only preprocessed point values are scrubbed from the fixed descriptor"
        );

        let mut changed_transcript_value = bootstrap.clone();
        changed_transcript_value.transcript_initial_state = changed_transcript_value
            .transcript_initial_state
            .map(|state| state + Fp::ONE);
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &changed_transcript_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("structure with changed transcript value"),
            "only the transcript-state value is scrubbed from the fixed descriptor"
        );
        assert_ne!(
            bootstrap_identity,
            kagemusha_compiled_protocol_identity_sha256(
                &changed_preprocessed_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("identity with changed preprocessed value"),
            "the complete identity must authenticate preprocessed point values"
        );
        assert_ne!(
            bootstrap_identity,
            kagemusha_compiled_protocol_identity_sha256(
                &changed_transcript_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("identity with changed transcript value"),
            "the complete identity must authenticate the transcript-state value"
        );

        let mut missing_transcript_state = bootstrap.clone();
        missing_transcript_state.transcript_initial_state = None;
        assert!(
            kagemusha_compiled_protocol_identity_sha256(
                &missing_transcript_state,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .is_err(),
            "a protocol without its authenticated transcript state must fail closed"
        );

        let mut final_builder =
            halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fp>::new(false)
                .use_params(base_circuit_params.clone());
        let range = final_builder.range_chip();
        let public = {
            let ctx = final_builder.main(0);
            let lhs = ctx.load_witness(Fp::from(17));
            let rhs = ctx.load_witness(Fp::from(25));
            range.range_check(ctx, lhs, 8);
            range.range_check(ctx, rhs, 8);
            range.gate().add(ctx, lhs, rhs)
        };
        final_builder.assigned_instances = vec![vec![public]];
        let final_vk = keygen_vk(&params, &final_builder).expect("final universal BaseConfig VK");
        let captured_break_points = final_builder.break_points();
        assert_eq!(
            kagemusha_break_points_from_wire_v4(
                &kagemusha_break_points_to_wire_v4(&captured_break_points)
                    .expect("encode captured breakpoints")
            )
            .expect("decode captured breakpoints"),
            captured_break_points,
            "captured breakpoints must round-trip through the portable header width"
        );
        let final_protocol = compile(&params, &final_vk, kagemusha_ipa_compile_config_v4(1));
        assert!(
            final_protocol.instance_committing_key.is_none(),
            "final V4 compilation must evaluate public instances directly"
        );
        kagemusha_require_protocol_structure_v1(
            &bootstrap,
            &final_protocol,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("the universal target must converge in one pass");
        assert_ne!(
            kagemusha_compiled_protocol_identity_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("bootstrap identity"),
            kagemusha_compiled_protocol_identity_sha256(
                &final_protocol,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("final identity"),
            "the static shape converges while dynamic VK values remain distinct"
        );

        let final_pk = keygen_pk(&params, final_vk.clone(), &final_builder)
            .expect("direct-instance test proving key");
        assert_eq!(
            final_builder.break_points(),
            captured_break_points,
            "PK synthesis must reproduce the VK layout"
        );
        let mut prover_builder =
            halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fp>::prover(
                base_circuit_params,
                captured_break_points,
            );
        let range = prover_builder.range_chip();
        let public = {
            let ctx = prover_builder.main(0);
            let lhs = ctx.load_witness(Fp::from(17));
            let rhs = ctx.load_witness(Fp::from(25));
            range.range_check(ctx, lhs, 8);
            range.range_check(ctx, rhs, 8);
            range.gate().add(ctx, lhs, rhs)
        };
        prover_builder.assigned_instances = vec![vec![public]];
        assert!(
            prover_builder.witness_gen_only(),
            "the proof circuit must use the witness-only prover stage"
        );
        let instances = vec![vec![Fp::from(42)]];
        let (proof, _) =
            create_augmented_eq_proof_v4(&params, final_pk, prover_builder, &instances)
                .expect("direct-instance augmented proof");
        let decide = |candidate: &[Vec<Fp>]| -> Result<(), String> {
            let current = succinct_verify_step_eq_instances(
                &params,
                &final_vk,
                &proof,
                candidate,
                proof.len(),
            )?;
            let initialization = KagemushaIpaAccumulationProofV4::initialization(8)?;
            crate::zk::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
                &params,
                8,
                current,
                None,
                &initialization,
            )
            .map(|_| ())
        };
        decide(&instances).expect("direct-instance IPA proof round-trip");
        assert!(
            decide(&[vec![Fp::from(43)]]).is_err(),
            "substituting a non-zero public instance must fail"
        );
    }

    fn exact_state(step: u32) -> Vec<u32> {
        let mut state =
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
        state[0] =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
        state[1] = step;
        for (index, limb) in state.iter_mut().enumerate().skip(2) {
            *limb = step
                .wrapping_mul(1_003)
                .wrapping_add(u32::try_from(index).expect("state-vector index fits u32"));
        }
        let offset = |field: &str| {
            crate::zk::kagemusha_v2::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V2
                .iter()
                .find_map(|(name, start, _)| (*name == field).then_some(*start))
                .expect("state fixture field exists")
        };
        state[offset("proof_step_count")] = step;
        state[offset("peer_hop_count")] = step
            .saturating_sub(1)
            .min(iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2);
        let manifest = offset("artifact_manifest_sha256");
        for (index, limb) in state[manifest..manifest + 8].iter_mut().enumerate() {
            *limb = 0xA500_0000 | u32::try_from(index + 1).expect("digest index fits u32");
        }
        state
    }

    #[test]
    fn v5_pre_keygen_parent_extraction_accepts_only_provisional_empty_breakpoints() {
        let params = valid_step_circuit_params_v4();
        let structure = [0x51; 32];
        let mut bootstrap = v4_bootstrap();
        assert!(
            bootstrap
                .step_eq_parent_internal(
                    &params,
                    structure,
                    0,
                    KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
                )
                .is_err(),
            "the provisional path must reject even valid populated keygen breakpoints"
        );
        bootstrap.circuit_break_points.clear();

        assert!(
            bootstrap.step_eq_parent(&params, structure, 0).is_err(),
            "strict parent extraction must require authenticated keygen breakpoints"
        );
        let parent = bootstrap
            .step_eq_parent_internal(
                &params,
                structure,
                0,
                KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
            )
            .expect("the pre-keygen seed may omit breakpoints before they are captured");
        assert_eq!(
            parent.instances,
            vec![vec![Fp::ZERO; parent.instances[0].len()]]
        );

        bootstrap.circuit_break_points = vec![vec![], vec![]];
        assert!(
            bootstrap
                .step_eq_parent_internal(
                    &params,
                    structure,
                    0,
                    KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
                )
                .is_err(),
            "the provisional path must still reject malformed non-empty breakpoints"
        );

        let mut ep_bootstrap = v4_bootstrap();
        ep_bootstrap.parity = KagemushaPastaCycleParityV1::StepEp;
        ep_bootstrap.circuit_break_points.clear();
        ep_bootstrap.parent_slot.carried_lineage =
            v4_accumulator(KagemushaPastaCycleParityV1::StepEp, params.k);
        assert!(
            ep_bootstrap.step_ep_parent(&params, structure, 0).is_err(),
            "strict Ep extraction must require authenticated keygen breakpoints"
        );
        ep_bootstrap
            .step_ep_parent_internal(
                &params,
                structure,
                0,
                KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
            )
            .expect("the pre-keygen Ep seed may omit breakpoints before they are captured");
    }
}
