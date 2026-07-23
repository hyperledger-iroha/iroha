//! Authenticated framing and role-safe carriers for Kagemusha ABI-20 artifacts.
//!
//! V4 packages are selector-free and intentionally reject any pre-release
//! format. Every release-sized allocation is preceded by a fixed upper-bound,
//! descriptor, and framing check. Framed bytes are first authenticated against
//! the canonical manifest; production constructors then require a separately
//! authenticated [`KagemushaAuthenticatedReleaseV4`].

use std::{
    collections::BTreeSet,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};

#[cfg(feature = "kagemusha-candidate-evidence-lab")]
use iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4;
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_HEADER_MAX_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4, KagemushaAuthenticatedReleaseV4,
    KagemushaPastaCycleArtifactKindV4, KagemushaPastaCycleArtifactV4,
    KagemushaPastaCycleFramedArtifactHeaderV4, KagemushaPastaCycleParityV1,
    KagemushaPastaCycleProofProfileV4, KagemushaPastaCycleProvingKeyHeaderV4,
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaStepCircuitParamsV4,
};
use sha2::{Digest as _, Sha256};

/// Framing magic for a streamed ABI-20 artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4: &[u8; 8] =
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4;
/// Defensive limit checked before allocating an encoded V4 header.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4: usize =
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4 as usize;
/// Fixed scratch used while authenticating a framed artifact without retaining
/// its release-sized payload.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_STREAM_SCRATCH_BYTES_V4: usize = 64 * 1024;

/// Bounded public carrier used by the core-owned KRV4 framing and parser.
pub type KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 =
    KagemushaPastaCycleFramedArtifactHeaderV4;

static NEXT_EXPORT_TEMP_ID_V4: AtomicU64 = AtomicU64::new(0);

/// Canonically decoded breakpoint header plus a borrowed processed proving key.
///
/// The proving-key bytes borrow the authenticated payload rather than being
/// copied into the Norito header. Streaming runtimes can instead parse the same
/// suffix directly from a bounded reader using the offset returned by
/// [`KagemushaAuthenticatedArtifactInspectionV4`].
#[derive(Debug, PartialEq, Eq)]
pub struct KagemushaValidatedProvingKeyPayloadV4<'a> {
    header: KagemushaPastaCycleProvingKeyHeaderV4,
    processed_proving_key: &'a [u8],
}

impl<'a> KagemushaValidatedProvingKeyPayloadV4<'a> {
    /// Return the profile-bound, keygen-derived row breakpoints.
    #[must_use]
    pub fn header(&self) -> &KagemushaPastaCycleProvingKeyHeaderV4 {
        &self.header
    }

    /// Return the raw `SerdeFormat::Processed` proving-key suffix.
    #[must_use]
    pub fn processed_proving_key(&self) -> &'a [u8] {
        self.processed_proving_key
    }
}

/// Validated layout of the breakpoint header and raw processed-key suffix
/// inside one authenticated proving-key payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaProvingKeyPayloadInspectionV4 {
    header: KagemushaPastaCycleProvingKeyHeaderV4,
    processed_proving_key_offset: u64,
    processed_proving_key_size_bytes: u64,
}

/// Infallible append-only writer for one canonical inner proving-key payload.
///
/// Halo2's processed-key serializer assumes its writer cannot fail in several
/// nested polynomial encoders. Keeping the final framed `Vec` private makes
/// every `write` append infallible; [`Self::finish`] applies the authenticated
/// size and non-empty-suffix checks after serialization. This lets callers
/// stream `ProvingKey::write` directly into its final KPKBPV4 payload without
/// first materializing and then copying a raw processed-key `Vec`.
#[must_use]
pub struct KagemushaProvingKeyPayloadWriterV4 {
    bytes: Vec<u8>,
    processed_proving_key_offset: usize,
}

impl Write for KagemushaProvingKeyPayloadWriterV4 {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl KagemushaProvingKeyPayloadWriterV4 {
    /// Finish the framed payload and return its exact canonical bytes.
    pub fn finish(self) -> Result<Vec<u8>, String> {
        if self.bytes.len() <= self.processed_proving_key_offset {
            return Err("Kagemusha V4 processed proving key is empty".to_owned());
        }
        if u64::try_from(self.bytes.len())
            .ok()
            .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
        {
            return Err("Kagemusha V4 proving-key payload exceeds its bound".to_owned());
        }
        Ok(self.bytes)
    }
}

impl KagemushaProvingKeyPayloadInspectionV4 {
    /// Return the profile-bound, keygen-derived row breakpoints.
    #[must_use]
    pub fn header(&self) -> &KagemushaPastaCycleProvingKeyHeaderV4 {
        &self.header
    }

    /// Return the processed-key offset relative to the outer payload.
    #[must_use]
    pub const fn processed_proving_key_offset(&self) -> u64 {
        self.processed_proving_key_offset
    }

    /// Return the exact remaining processed-key byte length.
    #[must_use]
    pub const fn processed_proving_key_size_bytes(&self) -> u64 {
        self.processed_proving_key_size_bytes
    }
}

/// Read and validate the bounded inner proving-key prefix, leaving `reader`
/// positioned at the first raw processed-key byte.
pub fn read_kagemusha_pasta_cycle_proving_key_prefix_v4<R: Read>(
    reader: &mut R,
    payload_size_bytes: u64,
    expected_parity: KagemushaPastaCycleParityV1,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaProvingKeyPayloadInspectionV4, String> {
    if payload_size_bytes == 0
        || payload_size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
    {
        return Err("Kagemusha V4 proving-key payload length is invalid".to_owned());
    }
    let mut magic = [0_u8; KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4.len()];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("failed to read Kagemusha V4 proving-key magic: {error}"))?;
    if &magic != KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4 {
        return Err("Kagemusha V4 proving-key payload magic mismatch".to_owned());
    }
    let mut header_len_bytes = [0_u8; 4];
    reader.read_exact(&mut header_len_bytes).map_err(|error| {
        format!("failed to read Kagemusha V4 proving-key header length: {error}")
    })?;
    let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
        .map_err(|_| "Kagemusha V4 proving-key header length does not fit usize".to_owned())?;
    let maximum_header = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_HEADER_MAX_BYTES_V4)
        .map_err(|_| "Kagemusha V4 proving-key header bound does not fit usize".to_owned())?;
    let prefix_len = magic
        .len()
        .checked_add(header_len_bytes.len())
        .and_then(|len| len.checked_add(header_len))
        .ok_or_else(|| "Kagemusha V4 proving-key prefix length overflow".to_owned())?;
    let prefix_len_u64 = u64::try_from(prefix_len)
        .map_err(|_| "Kagemusha V4 proving-key prefix length does not fit u64".to_owned())?;
    if header_len == 0 || header_len > maximum_header || prefix_len_u64 >= payload_size_bytes {
        return Err("Kagemusha V4 proving-key header length is invalid".to_owned());
    }
    let mut header_bytes = vec![0_u8; header_len];
    reader
        .read_exact(&mut header_bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 proving-key header: {error}"))?;
    let decode_limits = norito::core::DecodeLimits::new(
        1024,
        maximum_header,
        1024,
        maximum_header.saturating_mul(4),
        8,
    );
    let header: KagemushaPastaCycleProvingKeyHeaderV4 =
        norito::decode_from_bytes_with_limits(&header_bytes, decode_limits)
            .map_err(|_| "Kagemusha V4 proving-key header is malformed".to_owned())?;
    header
        .validate(expected_parity, circuit_params)
        .map_err(|error| error.to_string())?;
    if norito::to_bytes(&header)
        .map_err(|error| format!("failed to re-encode Kagemusha V4 proving-key header: {error}"))?
        != header_bytes
    {
        return Err("Kagemusha V4 proving-key header is not canonical".to_owned());
    }
    Ok(KagemushaProvingKeyPayloadInspectionV4 {
        header,
        processed_proving_key_offset: prefix_len_u64,
        processed_proving_key_size_bytes: payload_size_bytes - prefix_len_u64,
    })
}

/// Begin one canonical KPKBPV4 payload and return an append-only writer
/// positioned exactly at the processed proving-key suffix.
pub fn begin_kagemusha_pasta_cycle_proving_key_payload_v4(
    header: &KagemushaPastaCycleProvingKeyHeaderV4,
    expected_parity: KagemushaPastaCycleParityV1,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaProvingKeyPayloadWriterV4, String> {
    header
        .validate(expected_parity, circuit_params)
        .map_err(|error| error.to_string())?;
    let header_bytes = norito::to_bytes(header)
        .map_err(|error| format!("failed to encode Kagemusha V4 proving-key header: {error}"))?;
    let maximum_header = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_HEADER_MAX_BYTES_V4)
        .map_err(|_| "Kagemusha V4 proving-key header bound does not fit usize".to_owned())?;
    if header_bytes.is_empty() || header_bytes.len() > maximum_header {
        return Err("Kagemusha V4 proving-key header length is invalid".to_owned());
    }
    let header_len = u32::try_from(header_bytes.len())
        .map_err(|_| "Kagemusha V4 proving-key header length does not fit u32".to_owned())?;
    let prefix_len = KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4
        .len()
        .checked_add(size_of::<u32>())
        .and_then(|len| len.checked_add(header_bytes.len()))
        .ok_or_else(|| "Kagemusha V4 proving-key prefix length overflow".to_owned())?;
    let mut bytes = Vec::with_capacity(prefix_len);
    bytes.extend_from_slice(KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4);
    bytes.extend_from_slice(&header_len.to_le_bytes());
    bytes.extend_from_slice(&header_bytes);
    Ok(KagemushaProvingKeyPayloadWriterV4 {
        bytes,
        processed_proving_key_offset: prefix_len,
    })
}

/// Encode one canonical inner proving-key payload without embedding the large
/// processed key in a Norito collection.
pub fn encode_kagemusha_pasta_cycle_proving_key_payload_v4(
    header: &KagemushaPastaCycleProvingKeyHeaderV4,
    expected_parity: KagemushaPastaCycleParityV1,
    circuit_params: &KagemushaStepCircuitParamsV4,
    processed_proving_key: &[u8],
) -> Result<Vec<u8>, String> {
    if processed_proving_key.is_empty() {
        return Err("Kagemusha V4 processed proving key is empty".to_owned());
    }
    let mut payload = begin_kagemusha_pasta_cycle_proving_key_payload_v4(
        header,
        expected_parity,
        circuit_params,
    )?;
    let total_len = payload
        .processed_proving_key_offset
        .checked_add(processed_proving_key.len())
        .ok_or_else(|| "Kagemusha V4 proving-key payload length overflow".to_owned())?;
    if u64::try_from(total_len)
        .ok()
        .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
    {
        return Err("Kagemusha V4 proving-key payload exceeds its bound".to_owned());
    }
    payload
        .bytes
        .try_reserve_exact(processed_proving_key.len())
        .map_err(|_| "failed to reserve Kagemusha V4 proving-key payload".to_owned())?;
    payload
        .write_all(processed_proving_key)
        .map_err(|error| format!("failed to write Kagemusha V4 proving-key payload: {error}"))?;
    payload.finish()
}

/// Decode and canonically validate one inner proving-key payload.
///
/// Legacy raw processed-key payloads have no magic prefix and are rejected;
/// there is intentionally no compatibility fallback.
pub fn decode_kagemusha_pasta_cycle_proving_key_payload_v4<'a>(
    bytes: &'a [u8],
    expected_parity: KagemushaPastaCycleParityV1,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaValidatedProvingKeyPayloadV4<'a>, String> {
    if bytes.is_empty()
        || u64::try_from(bytes.len())
            .ok()
            .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
    {
        return Err("Kagemusha V4 proving-key payload length is invalid".to_owned());
    }
    let magic_len = KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4.len();
    let length_end = magic_len
        .checked_add(size_of::<u32>())
        .ok_or_else(|| "Kagemusha V4 proving-key prefix length overflow".to_owned())?;
    if bytes.len() <= length_end
        || bytes.get(..magic_len) != Some(KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4)
    {
        return Err("Kagemusha V4 proving-key payload magic mismatch".to_owned());
    }
    let header_len_bytes: [u8; 4] = bytes[magic_len..length_end]
        .try_into()
        .map_err(|_| "Kagemusha V4 proving-key header length is truncated".to_owned())?;
    let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
        .map_err(|_| "Kagemusha V4 proving-key header length does not fit usize".to_owned())?;
    let maximum_header = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_HEADER_MAX_BYTES_V4)
        .map_err(|_| "Kagemusha V4 proving-key header bound does not fit usize".to_owned())?;
    let header_end = length_end
        .checked_add(header_len)
        .ok_or_else(|| "Kagemusha V4 proving-key header length overflow".to_owned())?;
    if header_len == 0 || header_len > maximum_header || header_end >= bytes.len() {
        return Err("Kagemusha V4 proving-key header length is invalid".to_owned());
    }
    let header_bytes = &bytes[length_end..header_end];
    let decode_limits = norito::core::DecodeLimits::new(
        1024,
        maximum_header,
        1024,
        maximum_header.saturating_mul(4),
        8,
    );
    let header: KagemushaPastaCycleProvingKeyHeaderV4 =
        norito::decode_from_bytes_with_limits(header_bytes, decode_limits)
            .map_err(|_| "Kagemusha V4 proving-key header is malformed".to_owned())?;
    header
        .validate(expected_parity, circuit_params)
        .map_err(|error| error.to_string())?;
    if norito::to_bytes(&header)
        .map_err(|error| format!("failed to re-encode Kagemusha V4 proving-key header: {error}"))?
        != header_bytes
    {
        return Err("Kagemusha V4 proving-key header is not canonical".to_owned());
    }
    Ok(KagemushaValidatedProvingKeyPayloadV4 {
        header,
        processed_proving_key: &bytes[header_end..],
    })
}

/// Return the canonical file name for one of the eight V4 artifact roles.
#[must_use]
pub const fn kagemusha_artifact_file_name_v4(
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> &'static str {
    match (parity, kind) {
        (KagemushaPastaCycleParityV1::StepEq, KagemushaPastaCycleArtifactKindV4::ParamsIpa) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEq, KagemushaPastaCycleArtifactKindV4::ProvingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEq, KagemushaPastaCycleArtifactKindV4::VerifyingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4
        }
        (
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ) => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
        (KagemushaPastaCycleParityV1::StepEp, KagemushaPastaCycleArtifactKindV4::ParamsIpa) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEp, KagemushaPastaCycleArtifactKindV4::ProvingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEp, KagemushaPastaCycleArtifactKindV4::VerifyingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4
        }
        (
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ) => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
    }
}

fn validate_header_v4(
    header: &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
) -> Result<(), String> {
    header.validate().map_err(|error| error.to_string())
}

fn validate_header_against_manifest_v4(
    header: &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<(), String> {
    header
        .validate_against_manifest(manifest, descriptor)
        .map_err(|error| error.to_string())
}

fn export_header_v4(
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload: &[u8],
) -> Result<KagemushaRecursiveSpendPastaCycleArtifactHeaderV4, String> {
    profile
        .circuit_params
        .validate()
        .map_err(|error| error.to_string())?;
    if profile.ipa_k != profile.circuit_params.k
        || profile.compiled_protocol_structure_sha256 == [0; 32]
        || profile.step_proof_size_bytes != profile.circuit_params.max_parent_proof_bytes
        || payload.is_empty()
        || u64::try_from(payload.len())
            .ok()
            .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
    {
        return Err("Kagemusha V4 export profile or payload is invalid".to_owned());
    }
    let header = KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
        manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        generation: generation.to_owned(),
        parity: profile.parity,
        circuit_id: profile.circuit_id.clone(),
        parameter_generation: profile.parameter_generation.clone(),
        ipa_k: profile.ipa_k,
        circuit_params_sha256: profile
            .circuit_params
            .sha256()
            .map_err(|error| error.to_string())?,
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        step_proof_size_bytes: profile.step_proof_size_bytes,
        kind,
        payload_size_bytes: u64::try_from(payload.len())
            .map_err(|_| "Kagemusha V4 payload length does not fit u64".to_owned())?,
        payload_sha256: Sha256::digest(payload).into(),
    };
    validate_header_v4(&header)?;
    Ok(header)
}

/// Stream one canonical KRV4 package and return its exact manifest descriptor.
///
/// The supplied profile is the measured profile being assembled; its inline
/// circuit parameters are bound by the bounded header. The caller must insert
/// the returned descriptor into that profile
/// and validate the completed manifest before release signing.
pub fn write_kagemusha_pasta_cycle_artifact_v4<W: Write>(
    writer: &mut W,
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload: &[u8],
) -> Result<KagemushaPastaCycleArtifactV4, String> {
    let header = export_header_v4(generation, profile, kind, payload)?;
    let header_bytes = norito::to_bytes(&header)
        .map_err(|error| format!("failed to encode Kagemusha V4 artifact header: {error}"))?;
    if header_bytes.is_empty()
        || header_bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
    {
        return Err("Kagemusha V4 encoded header exceeds its bound".to_owned());
    }
    let header_len = u32::try_from(header_bytes.len())
        .map_err(|_| "Kagemusha V4 header length does not fit u32".to_owned())?;
    let header_len_bytes = header_len.to_le_bytes();
    let size_bytes = u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len())
        .ok()
        .and_then(|size| size.checked_add(u64::try_from(header_len_bytes.len()).ok()?))
        .and_then(|size| size.checked_add(u64::from(header_len)))
        .and_then(|size| size.checked_add(header.payload_size_bytes))
        .ok_or_else(|| "Kagemusha V4 framed artifact size overflow".to_owned())?;
    if size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4 {
        return Err(format!(
            "Kagemusha V4 framed artifact size {size_bytes} exceeds explicit ceiling {}",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        ));
    }

    let mut framed_hasher = Sha256::new();
    for bytes in [
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.as_slice(),
        header_len_bytes.as_slice(),
        header_bytes.as_slice(),
        payload,
    ] {
        writer
            .write_all(bytes)
            .map_err(|error| format!("failed to write Kagemusha V4 artifact: {error}"))?;
        framed_hasher.update(bytes);
    }
    let descriptor = KagemushaPastaCycleArtifactV4 {
        kind,
        file_name: kagemusha_artifact_file_name_v4(profile.parity, kind).to_owned(),
        size_bytes,
        sha256: framed_hasher.finalize().into(),
        payload_size_bytes: header.payload_size_bytes,
        payload_sha256: header.payload_sha256,
    };
    descriptor.validate().map_err(|error| error.to_string())?;
    Ok(descriptor)
}

/// One atomically published KRV4 file and its exact manifest descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaExportedArtifactV4 {
    path: PathBuf,
    descriptor: KagemushaPastaCycleArtifactV4,
}

impl KagemushaExportedArtifactV4 {
    /// Final canonical file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Descriptor to insert into the corresponding V4 manifest profile.
    #[must_use]
    pub const fn descriptor(&self) -> &KagemushaPastaCycleArtifactV4 {
        &self.descriptor
    }
}

fn ensure_private_export_directory_v4(directory: &Path) -> Result<(), String> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err("Kagemusha V4 export path must be a real directory".to_owned());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(directory).map_err(|error| {
                format!("failed to create Kagemusha V4 export directory: {error}")
            })?;
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect Kagemusha V4 export directory: {error}"
            ));
        }
    }
    #[cfg(unix)]
    {
        let mode = fs::metadata(directory)
            .map_err(|error| format!("failed to inspect Kagemusha V4 directory mode: {error}"))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(
                "Kagemusha V4 export directory must not grant group/other permissions".to_owned(),
            );
        }
    }
    Ok(())
}

fn create_private_temp_v4(directory: &Path, final_name: &str) -> Result<(PathBuf, File), String> {
    for _ in 0..128 {
        let id = NEXT_EXPORT_TEMP_ID_V4.fetch_add(1, Ordering::Relaxed);
        let temp_path = directory.join(format!(".{final_name}.tmp.{}.{}", std::process::id(), id));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "failed to create private Kagemusha V4 temporary file: {error}"
                ));
            }
        }
    }
    Err("failed to reserve a unique Kagemusha V4 temporary file".to_owned())
}

/// Atomically publish one owner-only KRV4 file without overwriting any path.
///
/// The output directory is created with mode `0700` on Unix, or rejected when
/// an existing directory grants group/other permissions. The file is fully
/// written and synced at mode `0600`, then atomically linked to its canonical
/// name; an existing destination always fails closed.
pub fn export_kagemusha_pasta_cycle_artifact_v4(
    output_directory: &Path,
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload: &[u8],
) -> Result<KagemushaExportedArtifactV4, String> {
    ensure_private_export_directory_v4(output_directory)?;
    let final_name = kagemusha_artifact_file_name_v4(profile.parity, kind);
    let final_path = output_directory.join(final_name);
    if fs::symlink_metadata(&final_path).is_ok() {
        return Err(format!(
            "refusing to overwrite existing Kagemusha V4 artifact `{final_name}`"
        ));
    }
    let (temp_path, mut temp_file) = create_private_temp_v4(output_directory, final_name)?;
    let descriptor = match write_kagemusha_pasta_cycle_artifact_v4(
        &mut temp_file,
        generation,
        profile,
        kind,
        payload,
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            drop(temp_file);
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
    };
    if let Err(error) = temp_file.sync_all() {
        drop(temp_file);
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "failed to sync Kagemusha V4 temporary artifact: {error}"
        ));
    }
    drop(temp_file);
    if let Err(error) = fs::hard_link(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "failed to atomically publish Kagemusha V4 artifact without overwrite: {error}"
        ));
    }
    if let Err(error) = fs::remove_file(&temp_path) {
        let _ = fs::remove_file(&final_path);
        return Err(format!(
            "failed to remove Kagemusha V4 temporary link: {error}"
        ));
    }
    File::open(output_directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("failed to sync Kagemusha V4 export directory: {error}"))?;
    Ok(KagemushaExportedArtifactV4 {
        path: final_path,
        descriptor,
    })
}

/// Fully authenticated unframed bytes from one V4 artifact role.
///
/// Construction is private to the bounded framed reader, preventing callers
/// from labeling arbitrary bytes as release-authenticated material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaValidatedArtifactPayloadV4 {
    header: KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    payload: Vec<u8>,
}

/// Trust mode attached to one parsed ABI-20 inventory.
///
/// The candidate variant is compiled only into the explicitly requested
/// non-shipping evidence harness. Keeping the mode in the carrier prevents a
/// candidate payload from being relabelled as release-authenticated material
/// while still allowing both modes to exercise the same prover and verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
enum KagemushaArtifactManifestBindingV4 {
    AuthenticatedRelease(KagemushaAuthenticatedReleaseV4),
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    CandidateEvidence {
        candidate: KagemushaRecursiveSpendCandidateV4,
        candidate_sha256: [u8; 32],
        manifest_sha256: [u8; 32],
    },
}

impl KagemushaArtifactManifestBindingV4 {
    fn authenticated_release(release: &KagemushaAuthenticatedReleaseV4) -> Self {
        Self::AuthenticatedRelease(release.clone())
    }

    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    fn candidate_evidence(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
    ) -> Result<Self, String> {
        candidate.validate().map_err(|error| error.to_string())?;
        let candidate_sha256 = candidate.sha256().map_err(|error| error.to_string())?;
        let manifest_bytes = norito::to_bytes(&candidate.manifest).map_err(|error| {
            format!("failed to encode Kagemusha V4 candidate manifest: {error}")
        })?;
        let manifest_sha256: [u8; 32] = Sha256::digest(manifest_bytes).into();
        if candidate_sha256 == [0; 32]
            || manifest_sha256 == [0; 32]
            || candidate_sha256 != expected_candidate_sha256
            || manifest_sha256 != expected_manifest_sha256
        {
            return Err("Kagemusha V4 candidate identity mismatch".to_owned());
        }
        Ok(Self::CandidateEvidence {
            candidate: candidate.clone(),
            candidate_sha256,
            manifest_sha256,
        })
    }

    fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV4 {
        match self {
            Self::AuthenticatedRelease(release) => release.manifest(),
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence { candidate, .. } => &candidate.manifest,
        }
    }

    fn manifest_sha256(&self) -> [u8; 32] {
        match self {
            Self::AuthenticatedRelease(release) => release.manifest_sha256(),
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence {
                manifest_sha256, ..
            } => *manifest_sha256,
        }
    }

    fn is_candidate_evidence_lab(&self) -> bool {
        #[cfg(feature = "kagemusha-candidate-evidence-lab")]
        if matches!(self, Self::CandidateEvidence { .. }) {
            return true;
        }
        false
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::AuthenticatedRelease(release) => {
                release
                    .manifest()
                    .validate()
                    .map_err(|error| error.to_string())?;
                if release.manifest_sha256() == [0; 32] {
                    return Err("Kagemusha V4 authenticated release digest is zero".to_owned());
                }
                Ok(())
            }
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence {
                candidate,
                candidate_sha256,
                manifest_sha256,
            } => {
                candidate.validate().map_err(|error| error.to_string())?;
                let observed_candidate_sha256 =
                    candidate.sha256().map_err(|error| error.to_string())?;
                let manifest_bytes = norito::to_bytes(&candidate.manifest).map_err(|error| {
                    format!("failed to encode Kagemusha V4 candidate manifest: {error}")
                })?;
                let observed_manifest_sha256: [u8; 32] = Sha256::digest(manifest_bytes).into();
                if *candidate_sha256 == [0; 32]
                    || *manifest_sha256 == [0; 32]
                    || observed_candidate_sha256 != *candidate_sha256
                    || observed_manifest_sha256 != *manifest_sha256
                {
                    return Err("Kagemusha V4 candidate binding changed".to_owned());
                }
                Ok(())
            }
        }
    }

    fn validate_header(
        &self,
        header: &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), String> {
        match self {
            Self::AuthenticatedRelease(release) => {
                validate_header_against_manifest_v4(header, release.manifest(), descriptor)
            }
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence { candidate, .. } => {
                candidate.validate().map_err(|error| error.to_string())?;
                header
                    .validate_against_candidate_manifest(&candidate.manifest, descriptor)
                    .map_err(|error| error.to_string())
            }
        }
    }
}

impl KagemushaValidatedArtifactPayloadV4 {
    /// Return the authenticated role header.
    #[must_use]
    pub fn header(&self) -> &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 {
        &self.header
    }

    /// Return the exact authenticated unframed payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    fn validate_payload(&self) -> Result<(), String> {
        validate_header_v4(&self.header)?;
        if u64::try_from(self.payload.len())
            .ok()
            .is_none_or(|len| len != self.header.payload_size_bytes)
            || <[u8; 32]>::from(Sha256::digest(&self.payload)) != self.header.payload_sha256
        {
            return Err("Kagemusha V4 authenticated artifact payload mismatch".to_owned());
        }
        Ok(())
    }
}

/// Locate one exact role in a validated Eq-then-Ep V4 manifest inventory.
pub fn kagemusha_artifact_descriptor_v4(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> Result<&KagemushaPastaCycleArtifactV4, String> {
    manifest.validate().map_err(|error| error.to_string())?;
    manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .and_then(|profile| {
            profile
                .artifacts
                .iter()
                .find(|descriptor| descriptor.kind == kind)
        })
        .ok_or_else(|| "Kagemusha V4 artifact manifest role is absent".to_owned())
}

/// Authenticated byte layout of one complete KRV4 artifact in a pinned reader.
///
/// This value contains only the bounded public header and offsets. Construction
/// requires streaming the complete payload through both manifest-selected
/// SHA-256 checks and rejecting trailing bytes; it never retains the payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaAuthenticatedArtifactInspectionV4 {
    header: KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    payload_offset: u64,
}

impl KagemushaAuthenticatedArtifactInspectionV4 {
    /// Return the authenticated role header.
    #[must_use]
    pub fn header(&self) -> &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 {
        &self.header
    }

    /// Return the byte offset of the exact unframed payload.
    #[must_use]
    pub const fn payload_offset(&self) -> u64 {
        self.payload_offset
    }

    /// Return the exact authenticated payload length.
    #[must_use]
    pub const fn payload_size_bytes(&self) -> u64 {
        self.header.payload_size_bytes
    }
}

struct KagemushaArtifactPrefixV4 {
    inspection: KagemushaAuthenticatedArtifactInspectionV4,
    framed_hasher: Sha256,
}

fn read_kagemusha_pasta_cycle_artifact_prefix_v4<R, V>(
    reader: &mut R,
    descriptor: &KagemushaPastaCycleArtifactV4,
    validate_binding: V,
) -> Result<KagemushaArtifactPrefixV4, String>
where
    R: Read,
    V: FnOnce(
        &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
        &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), String>,
{
    descriptor.validate().map_err(|error| error.to_string())?;
    let mut framed_hasher = Sha256::new();
    let mut magic = [0_u8; KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len()];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact magic: {error}"))?;
    framed_hasher.update(magic);
    if &magic != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4 {
        return Err("Kagemusha V4 artifact magic mismatch".to_owned());
    }

    let mut header_len_bytes = [0_u8; 4];
    reader
        .read_exact(&mut header_len_bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 header length: {error}"))?;
    framed_hasher.update(header_len_bytes);
    let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
        .map_err(|_| "Kagemusha V4 header length does not fit usize".to_owned())?;
    let prefix_len = magic
        .len()
        .checked_add(header_len_bytes.len())
        .and_then(|len| len.checked_add(header_len))
        .ok_or_else(|| "Kagemusha V4 artifact prefix length overflow".to_owned())?;
    if header_len == 0
        || header_len > KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
        || u64::try_from(prefix_len)
            .ok()
            .is_none_or(|prefix| prefix >= descriptor.size_bytes)
    {
        return Err("Kagemusha V4 artifact header length is invalid".to_owned());
    }

    let mut header_bytes = vec![0_u8; header_len];
    reader
        .read_exact(&mut header_bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact header: {error}"))?;
    framed_hasher.update(&header_bytes);
    let header_decode_limits = norito::core::DecodeLimits::new(
        1024,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4,
        4096,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4.saturating_mul(4),
        16,
    );
    let header: KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 =
        norito::decode_from_bytes_with_limits(&header_bytes, header_decode_limits)
            .map_err(|_| "Kagemusha V4 artifact header is malformed".to_owned())?;
    if norito::to_bytes(&header)
        .map_err(|error| format!("failed to re-encode Kagemusha V4 header: {error}"))?
        != header_bytes
    {
        return Err("Kagemusha V4 artifact header is not canonical".to_owned());
    }
    validate_header_v4(&header)?;
    if header.kind != descriptor.kind
        || header.payload_size_bytes != descriptor.payload_size_bytes
        || header.payload_sha256 != descriptor.payload_sha256
    {
        return Err("Kagemusha V4 artifact header descriptor mismatch".to_owned());
    }
    validate_binding(&header, descriptor)?;
    let payload_offset = u64::try_from(prefix_len)
        .map_err(|_| "Kagemusha V4 payload offset does not fit u64".to_owned())?;
    if payload_offset.checked_add(header.payload_size_bytes) != Some(descriptor.size_bytes) {
        return Err("Kagemusha V4 artifact payload length mismatch".to_owned());
    }
    Ok(KagemushaArtifactPrefixV4 {
        inspection: KagemushaAuthenticatedArtifactInspectionV4 {
            header,
            payload_offset,
        },
        framed_hasher,
    })
}

fn inspect_kagemusha_pasta_cycle_artifact_content_v4<R, V>(
    reader: &mut R,
    descriptor: &KagemushaPastaCycleArtifactV4,
    validate_binding: V,
) -> Result<KagemushaAuthenticatedArtifactInspectionV4, String>
where
    R: Read + Seek,
    V: FnOnce(
        &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
        &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), String>,
{
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind Kagemusha V4 artifact: {error}"))?;
    let KagemushaArtifactPrefixV4 {
        inspection,
        mut framed_hasher,
    } = read_kagemusha_pasta_cycle_artifact_prefix_v4(reader, descriptor, validate_binding)?;
    let mut payload_hasher = Sha256::new();
    let mut remaining = inspection.header.payload_size_bytes;
    let mut scratch = [0_u8; KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_STREAM_SCRATCH_BYTES_V4];
    while remaining != 0 {
        let chunk_len = usize::try_from(
            remaining.min(
                u64::try_from(scratch.len())
                    .map_err(|_| "Kagemusha V4 stream scratch length does not fit u64")?,
            ),
        )
        .map_err(|_| "Kagemusha V4 stream chunk length does not fit usize".to_owned())?;
        reader
            .read_exact(&mut scratch[..chunk_len])
            .map_err(|error| format!("failed to stream Kagemusha V4 artifact payload: {error}"))?;
        payload_hasher.update(&scratch[..chunk_len]);
        framed_hasher.update(&scratch[..chunk_len]);
        remaining -= u64::try_from(chunk_len)
            .map_err(|_| "Kagemusha V4 stream chunk length does not fit u64".to_owned())?;
    }
    let mut trailing = [0_u8; 1];
    if reader
        .read(&mut trailing)
        .map_err(|error| format!("failed to check Kagemusha V4 trailing bytes: {error}"))?
        != 0
        || <[u8; 32]>::from(payload_hasher.finalize()) != descriptor.payload_sha256
        || <[u8; 32]>::from(framed_hasher.finalize()) != descriptor.sha256
    {
        return Err("Kagemusha V4 artifact content digest mismatch".to_owned());
    }
    Ok(inspection)
}

/// Stream-authenticate one framed artifact from a pinned seekable reader
/// without retaining its payload.
pub fn inspect_kagemusha_pasta_cycle_artifact_v4<R: Read + Seek>(
    reader: &mut R,
    release: &KagemushaAuthenticatedReleaseV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<KagemushaAuthenticatedArtifactInspectionV4, String> {
    let binding = KagemushaArtifactManifestBindingV4::authenticated_release(release);
    binding.validate()?;
    inspect_kagemusha_pasta_cycle_artifact_content_v4(reader, descriptor, |header, descriptor| {
        binding.validate_header(header, descriptor)
    })
}

struct KagemushaPayloadHashingReaderV4<R> {
    inner: R,
    payload_hasher: Sha256,
    framed_hasher: Sha256,
    bytes_read: u64,
}

impl<R: Read> Read for KagemushaPayloadHashingReaderV4<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        let bytes = &buffer[..read];
        self.payload_hasher.update(bytes);
        self.framed_hasher.update(bytes);
        self.bytes_read = self
            .bytes_read
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX);
        Ok(read)
    }
}

fn with_kagemusha_pasta_cycle_artifact_payload_content_v4<R, T, V, F>(
    reader: &mut R,
    descriptor: &KagemushaPastaCycleArtifactV4,
    validate_binding: V,
    parse: F,
) -> Result<T, String>
where
    R: Read + Seek,
    V: Copy
        + Fn(
            &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
            &KagemushaPastaCycleArtifactV4,
        ) -> Result<(), String>,
    F: FnOnce(
        &mut dyn Read,
        &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    ) -> Result<T, String>,
{
    let authenticated =
        inspect_kagemusha_pasta_cycle_artifact_content_v4(reader, descriptor, validate_binding)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind Kagemusha V4 artifact: {error}"))?;
    let KagemushaArtifactPrefixV4 {
        inspection,
        framed_hasher,
    } = read_kagemusha_pasta_cycle_artifact_prefix_v4(reader, descriptor, validate_binding)?;
    if inspection != authenticated {
        return Err("Kagemusha V4 artifact header changed after authentication".to_owned());
    }
    let bounded = reader.take(inspection.header.payload_size_bytes);
    let mut hashing_reader = KagemushaPayloadHashingReaderV4 {
        inner: bounded,
        payload_hasher: Sha256::new(),
        framed_hasher,
        bytes_read: 0,
    };
    let parsed = parse(&mut hashing_reader, &inspection.header)?;
    let KagemushaPayloadHashingReaderV4 {
        inner: bounded,
        payload_hasher,
        framed_hasher,
        bytes_read,
    } = hashing_reader;
    if bytes_read != inspection.header.payload_size_bytes
        || bounded.limit() != 0
        || <[u8; 32]>::from(payload_hasher.finalize()) != descriptor.payload_sha256
        || <[u8; 32]>::from(framed_hasher.finalize()) != descriptor.sha256
    {
        return Err("Kagemusha V4 bounded payload was not consumed authentically".to_owned());
    }
    let reader = bounded.into_inner();
    let mut trailing = [0_u8; 1];
    if reader
        .read(&mut trailing)
        .map_err(|error| format!("failed to check Kagemusha V4 trailing bytes: {error}"))?
        != 0
    {
        return Err("Kagemusha V4 artifact acquired trailing bytes".to_owned());
    }
    Ok(parsed)
}

/// Authenticate one complete KRV4 file, then expose its exact payload through
/// a bounded reader for zero-copy typed parsing.
///
/// The callback must consume the payload completely. The second pass hashes
/// the exact bytes seen by the callback and verifies the framed digest again
/// before returning its result.
pub fn with_kagemusha_pasta_cycle_artifact_payload_v4<R, T, F>(
    reader: &mut R,
    release: &KagemushaAuthenticatedReleaseV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
    parse: F,
) -> Result<T, String>
where
    R: Read + Seek,
    F: FnOnce(
        &mut dyn Read,
        &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    ) -> Result<T, String>,
{
    let binding = KagemushaArtifactManifestBindingV4::authenticated_release(release);
    binding.validate()?;
    with_kagemusha_pasta_cycle_artifact_payload_content_v4(
        reader,
        descriptor,
        |header, descriptor| binding.validate_header(header, descriptor),
        parse,
    )
}

/// Read and authenticate one complete framed V4 artifact from a pinned handle.
///
/// Header and payload lengths are checked against hard limits and the exact
/// manifest descriptor before allocation. The reader must contain exactly one
/// artifact and no trailing bytes.
pub fn read_kagemusha_pasta_cycle_artifact_v4<R: Read>(
    reader: &mut R,
    release: &KagemushaAuthenticatedReleaseV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    let binding = KagemushaArtifactManifestBindingV4::authenticated_release(release);
    read_kagemusha_pasta_cycle_artifact_with_binding_v4(reader, &binding, descriptor)
}

/// Parse one exact KRV4 artifact against a clean, canonical pre-promotion
/// candidate. This API exists only in explicitly feature-selected evidence-lab
/// builds and does not manufacture an authenticated production release.
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
pub fn read_kagemusha_pasta_cycle_candidate_artifact_v4<R: Read>(
    reader: &mut R,
    candidate: &KagemushaRecursiveSpendCandidateV4,
    expected_candidate_sha256: [u8; 32],
    expected_manifest_sha256: [u8; 32],
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    let binding = KagemushaArtifactManifestBindingV4::candidate_evidence(
        candidate,
        expected_candidate_sha256,
        expected_manifest_sha256,
    )?;
    read_kagemusha_pasta_cycle_artifact_with_binding_v4(reader, &binding, descriptor)
}

fn read_kagemusha_pasta_cycle_artifact_with_binding_v4<R: Read>(
    reader: &mut R,
    binding: &KagemushaArtifactManifestBindingV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    binding.validate()?;
    descriptor.validate().map_err(|error| error.to_string())?;

    let mut framed_hasher = Sha256::new();
    let mut magic = [0_u8; KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len()];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact magic: {error}"))?;
    framed_hasher.update(magic);
    if &magic != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4 {
        return Err("Kagemusha V4 artifact magic mismatch".to_owned());
    }

    let mut header_len_bytes = [0_u8; 4];
    reader
        .read_exact(&mut header_len_bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 header length: {error}"))?;
    framed_hasher.update(header_len_bytes);
    let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
        .map_err(|_| "Kagemusha V4 header length does not fit usize".to_owned())?;
    let prefix_len = magic
        .len()
        .checked_add(header_len_bytes.len())
        .and_then(|len| len.checked_add(header_len))
        .ok_or_else(|| "Kagemusha V4 artifact prefix length overflow".to_owned())?;
    if header_len == 0
        || header_len > KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
        || u64::try_from(prefix_len)
            .ok()
            .is_none_or(|prefix| prefix >= descriptor.size_bytes)
    {
        return Err("Kagemusha V4 artifact header length is invalid".to_owned());
    }

    let mut header_bytes = vec![0_u8; header_len];
    reader
        .read_exact(&mut header_bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact header: {error}"))?;
    framed_hasher.update(&header_bytes);
    let header: KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 =
        norito::decode_from_bytes(&header_bytes)
            .map_err(|_| "Kagemusha V4 artifact header is malformed".to_owned())?;
    if norito::to_bytes(&header)
        .map_err(|error| format!("failed to re-encode Kagemusha V4 header: {error}"))?
        != header_bytes
    {
        return Err("Kagemusha V4 artifact header is not canonical".to_owned());
    }
    binding.validate_header(&header, descriptor)?;
    if u64::try_from(prefix_len)
        .ok()
        .and_then(|prefix| prefix.checked_add(header.payload_size_bytes))
        != Some(descriptor.size_bytes)
    {
        return Err("Kagemusha V4 artifact payload length mismatch".to_owned());
    }

    let payload_len = usize::try_from(header.payload_size_bytes)
        .map_err(|_| "Kagemusha V4 payload length does not fit usize".to_owned())?;
    if payload_len == 0
        || u64::try_from(payload_len)
            .ok()
            .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
    {
        return Err("Kagemusha V4 artifact payload length is invalid".to_owned());
    }
    let mut payload = vec![0_u8; payload_len];
    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact payload: {error}"))?;
    let payload_sha256: [u8; 32] = Sha256::digest(&payload).into();
    framed_hasher.update(&payload);
    let framed_sha256: [u8; 32] = framed_hasher.finalize().into();
    let mut trailing = [0_u8; 1];
    if reader
        .read(&mut trailing)
        .map_err(|error| format!("failed to check Kagemusha V4 trailing bytes: {error}"))?
        != 0
        || payload_sha256 != descriptor.payload_sha256
        || framed_sha256 != descriptor.sha256
    {
        return Err("Kagemusha V4 artifact content digest mismatch".to_owned());
    }
    Ok(KagemushaValidatedArtifactPayloadV4 { header, payload })
}

fn validate_role(
    binding: &KagemushaArtifactManifestBindingV4,
    artifact: &KagemushaValidatedArtifactPayloadV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> Result<(), String> {
    binding.validate()?;
    artifact.validate_payload()?;
    if artifact.header.parity != parity || artifact.header.kind != kind {
        return Err("Kagemusha V4 artifact carrier role mismatch".to_owned());
    }
    let descriptor = binding
        .manifest()
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .and_then(|profile| {
            profile
                .artifacts
                .iter()
                .find(|descriptor| descriptor.kind == kind)
        })
        .ok_or_else(|| "Kagemusha V4 artifact manifest role is absent".to_owned())?;
    binding.validate_header(&artifact.header, descriptor)
}

/// Exact six-role verifier material bound to one authenticated V4 release.
///
/// Bootstrap witnesses remain opaque here; the recursion adapter is the sole
/// owner of their canonical typed representation and validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaPastaCycleVerifierArtifactsV4 {
    binding: KagemushaArtifactManifestBindingV4,
    step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
    step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
    step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
    step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
    step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
}

impl KagemushaPastaCycleVerifierArtifactsV4 {
    /// Bind all six verifier roles to one authenticated release.
    pub fn new(
        release: &KagemushaAuthenticatedReleaseV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::authenticated_release(release),
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    /// Bind all six verifier roles to one clean candidate in a non-shipping
    /// evidence-lab build without relabelling it as a promoted release.
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    #[allow(clippy::too_many_arguments)]
    pub fn new_candidate_evidence_lab(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::candidate_evidence(
                candidate,
                expected_candidate_sha256,
                expected_manifest_sha256,
            )?,
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_binding(
        binding: KagemushaArtifactManifestBindingV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        binding.validate()?;
        let artifacts = [
            (
                &step_eq_parameters,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                &step_eq_verifying_key,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                &step_eq_bootstrap_witness,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
            (
                &step_ep_parameters,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                &step_ep_verifying_key,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                &step_ep_bootstrap_witness,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
        ];
        let mut digests = BTreeSet::new();
        for (artifact, parity, kind) in artifacts {
            validate_role(&binding, artifact, parity, kind)?;
            if !digests.insert(artifact.header.payload_sha256) {
                return Err("Kagemusha V4 verifier payloads are not distinct".to_owned());
            }
        }
        Ok(Self {
            binding,
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        })
    }

    /// SHA-256 of the exact authenticated manifest selecting every role.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.binding.manifest_sha256()
    }

    /// Authenticated manifest selecting every verifier role.
    #[must_use]
    pub(crate) fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV4 {
        self.binding.manifest()
    }

    #[must_use]
    pub(crate) fn is_candidate_evidence_lab(&self) -> bool {
        self.binding.is_candidate_evidence_lab()
    }

    /// Exact release-specific proof-pair cap.
    #[must_use]
    pub fn max_proof_bytes(&self) -> u32 {
        self.binding.manifest().max_proof_bytes
    }

    /// Authenticated Eq profile selecting circuit parameters and measured cap.
    #[must_use]
    pub(crate) fn step_eq_profile(&self) -> &KagemushaPastaCycleProofProfileV4 {
        &self.binding.manifest().profiles[0]
    }

    /// Authenticated Ep profile selecting circuit parameters and measured cap.
    #[must_use]
    pub(crate) fn step_ep_profile(&self) -> &KagemushaPastaCycleProofProfileV4 {
        &self.binding.manifest().profiles[1]
    }

    pub(crate) fn step_eq_parameters(&self) -> &[u8] {
        self.step_eq_parameters.payload()
    }

    pub(crate) fn step_eq_verifying_key(&self) -> &[u8] {
        self.step_eq_verifying_key.payload()
    }

    pub(crate) fn step_eq_bootstrap_witness(&self) -> &[u8] {
        self.step_eq_bootstrap_witness.payload()
    }

    pub(crate) fn step_ep_parameters(&self) -> &[u8] {
        self.step_ep_parameters.payload()
    }

    pub(crate) fn step_ep_verifying_key(&self) -> &[u8] {
        self.step_ep_verifying_key.payload()
    }

    pub(crate) fn step_ep_bootstrap_witness(&self) -> &[u8] {
        self.step_ep_bootstrap_witness.payload()
    }

    fn payload_digests(&self) -> [[u8; 32]; 6] {
        [
            self.step_eq_parameters.header.payload_sha256,
            self.step_eq_verifying_key.header.payload_sha256,
            self.step_eq_bootstrap_witness.header.payload_sha256,
            self.step_ep_parameters.header.payload_sha256,
            self.step_ep_verifying_key.header.payload_sha256,
            self.step_ep_bootstrap_witness.header.payload_sha256,
        ]
    }
}

/// Exact eight-role prover material bound to one authenticated V4 release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaPastaCycleProverArtifactsV4 {
    verifier: KagemushaPastaCycleVerifierArtifactsV4,
    step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
    step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
}

impl KagemushaPastaCycleProverArtifactsV4 {
    /// Bind the complete eight-artifact inventory to one authenticated release.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        release: &KagemushaAuthenticatedReleaseV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::authenticated_release(release),
            step_eq_parameters,
            step_eq_proving_key,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_proving_key,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    /// Bind the exact eight-role inventory to one clean candidate in an
    /// explicitly selected, non-shipping evidence-lab build.
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    #[allow(clippy::too_many_arguments)]
    pub fn new_candidate_evidence_lab(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::candidate_evidence(
                candidate,
                expected_candidate_sha256,
                expected_manifest_sha256,
            )?,
            step_eq_parameters,
            step_eq_proving_key,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_proving_key,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_binding(
        binding: KagemushaArtifactManifestBindingV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        binding.validate()?;
        validate_role(
            &binding,
            &step_eq_proving_key,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
        )?;
        validate_role(
            &binding,
            &step_ep_proving_key,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
        )?;
        let verifier = KagemushaPastaCycleVerifierArtifactsV4::new_with_binding(
            binding,
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )?;
        let mut digests: BTreeSet<_> = verifier.payload_digests().into_iter().collect();
        if !digests.insert(step_eq_proving_key.header.payload_sha256)
            || !digests.insert(step_ep_proving_key.header.payload_sha256)
        {
            return Err("Kagemusha V4 prover payloads are not distinct".to_owned());
        }
        Ok(Self {
            verifier,
            step_eq_proving_key,
            step_ep_proving_key,
        })
    }

    /// SHA-256 of the exact authenticated manifest selecting all eight roles.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.verifier.manifest_sha256()
    }

    /// Exact release-specific proof-pair cap.
    #[must_use]
    pub fn max_proof_bytes(&self) -> u32 {
        self.verifier.max_proof_bytes()
    }

    #[must_use]
    pub(crate) fn verifier(&self) -> &KagemushaPastaCycleVerifierArtifactsV4 {
        &self.verifier
    }

    pub(crate) fn step_eq_proving_key(&self) -> &[u8] {
        self.step_eq_proving_key.payload()
    }

    pub(crate) fn step_ep_proving_key(&self) -> &[u8] {
        self.step_ep_proving_key.payload()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read as _};

    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_HEADER_MAX_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
        KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4, KagemushaPastaPublicLayoutV4,
    };
    use sha2::Digest as _;

    use super::*;

    fn circuit_params() -> KagemushaStepCircuitParamsV4 {
        let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let layout =
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(k).expect("test public layout");
        KagemushaStepCircuitParamsV4 {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: vec![2],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows: 9,
            max_parent_proof_bytes: 4096,
        }
    }

    fn profile() -> KagemushaPastaCycleProofProfileV4 {
        let circuit_params = circuit_params();
        KagemushaPastaCycleProofProfileV4 {
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
            parameter_generation: "test-params-v4".to_owned(),
            ipa_k: circuit_params.k,
            compiled_protocol_structure_sha256: [0x41; 32],
            step_proof_size_bytes: circuit_params.max_parent_proof_bytes,
            circuit_params,
            artifacts: Vec::new(),
        }
    }

    fn proving_key_header() -> KagemushaPastaCycleProvingKeyHeaderV4 {
        let params = circuit_params();
        let usable_rows = (1_u32 << params.k) - params.minimum_unusable_rows;
        KagemushaPastaCycleProvingKeyHeaderV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4,
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_params_sha256: params.sha256().expect("test circuit-parameter digest"),
            break_points: vec![vec![usable_rows - 1]],
        }
    }

    #[test]
    fn proving_key_payload_round_trips_without_copying_the_key_suffix() {
        let params = circuit_params();
        let header = proving_key_header();
        let processed_key = b"canonical processed proving key fixture";
        let encoded = encode_kagemusha_pasta_cycle_proving_key_payload_v4(
            &header,
            KagemushaPastaCycleParityV1::StepEq,
            &params,
            processed_key,
        )
        .expect("encode proving-key payload");
        let mut writer = begin_kagemusha_pasta_cycle_proving_key_payload_v4(
            &header,
            KagemushaPastaCycleParityV1::StepEq,
            &params,
        )
        .expect("begin streamed proving-key payload");
        for chunk in processed_key.chunks(3) {
            writer
                .write_all(chunk)
                .expect("append processed proving-key chunk");
        }
        assert_eq!(
            writer.finish().expect("finish streamed payload"),
            encoded,
            "writer framing must remain byte-identical to the slice API"
        );
        assert!(
            begin_kagemusha_pasta_cycle_proving_key_payload_v4(
                &header,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
            )
            .expect("begin empty streamed payload")
            .finish()
            .is_err(),
            "a streamed payload without a processed-key suffix must fail"
        );
        let decoded = decode_kagemusha_pasta_cycle_proving_key_payload_v4(
            &encoded,
            KagemushaPastaCycleParityV1::StepEq,
            &params,
        )
        .expect("decode proving-key payload");
        assert_eq!(decoded.header(), &header);
        assert_eq!(decoded.processed_proving_key(), processed_key);
        let payload_start = encoded.as_ptr() as usize;
        let payload_end = payload_start + encoded.len();
        let key_start = decoded.processed_proving_key().as_ptr() as usize;
        assert!((payload_start..payload_end).contains(&key_start));

        let mut cursor = Cursor::new(encoded.as_slice());
        let inspection = read_kagemusha_pasta_cycle_proving_key_prefix_v4(
            &mut cursor,
            u64::try_from(encoded.len()).expect("small fixture"),
            KagemushaPastaCycleParityV1::StepEq,
            &params,
        )
        .expect("stream proving-key prefix");
        assert_eq!(inspection.header(), &header);
        assert_eq!(
            inspection.processed_proving_key_size_bytes(),
            u64::try_from(processed_key.len()).expect("small fixture")
        );
        let mut streamed_key = Vec::new();
        cursor
            .read_to_end(&mut streamed_key)
            .expect("read processed key suffix");
        assert_eq!(streamed_key, processed_key);
    }

    #[test]
    fn proving_key_payload_rejects_legacy_noncanonical_and_bounded_headers() {
        let params = circuit_params();
        let header = proving_key_header();
        let encoded = encode_kagemusha_pasta_cycle_proving_key_payload_v4(
            &header,
            KagemushaPastaCycleParityV1::StepEq,
            &params,
            b"pk",
        )
        .expect("encode proving-key payload");

        assert!(
            decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                b"legacy raw processed proving key",
                KagemushaPastaCycleParityV1::StepEq,
                &params,
            )
            .is_err()
        );

        let mut zero_header = encoded.clone();
        zero_header[8..12].copy_from_slice(&0_u32.to_le_bytes());
        assert!(
            decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &zero_header,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
            )
            .is_err()
        );

        let mut oversized_header = encoded.clone();
        oversized_header[8..12].copy_from_slice(
            &(KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_HEADER_MAX_BYTES_V4 + 1).to_le_bytes(),
        );
        assert!(
            decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &oversized_header,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
            )
            .is_err()
        );

        let canonical_header = norito::to_bytes(&header).expect("encode header fixture");
        let mut noncanonical = Vec::new();
        noncanonical.extend_from_slice(KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_MAGIC_V4);
        noncanonical.extend_from_slice(
            &u32::try_from(canonical_header.len() + 1)
                .expect("small header")
                .to_le_bytes(),
        );
        noncanonical.extend_from_slice(&canonical_header);
        noncanonical.push(0);
        noncanonical.extend_from_slice(b"pk");
        assert!(
            decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &noncanonical,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
            )
            .is_err()
        );

        assert!(
            decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &encoded,
                KagemushaPastaCycleParityV1::StepEp,
                &params,
            )
            .is_err()
        );
    }

    fn framed_fixture(payload: &[u8]) -> (Vec<u8>, KagemushaPastaCycleArtifactV4) {
        let mut bytes = Vec::new();
        let descriptor = write_kagemusha_pasta_cycle_artifact_v4(
            &mut bytes,
            "test-generation-v4",
            &profile(),
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
            payload,
        )
        .expect("write framed artifact fixture");
        (bytes, descriptor)
    }

    fn accept_test_binding(
        _: &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
        _: &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn streaming_inspection_and_bounded_payload_reject_tamper_and_trailing_bytes() {
        let payload = b"streamed proving-key payload fixture";
        let (bytes, descriptor) = framed_fixture(payload);
        let mut cursor = Cursor::new(bytes.clone());
        let inspection = inspect_kagemusha_pasta_cycle_artifact_content_v4(
            &mut cursor,
            &descriptor,
            accept_test_binding,
        )
        .expect("inspect framed fixture");
        assert_eq!(inspection.payload_size_bytes(), payload.len() as u64);
        assert_eq!(
            inspection.header().payload_sha256,
            descriptor.payload_sha256
        );

        let parsed = with_kagemusha_pasta_cycle_artifact_payload_content_v4(
            &mut cursor,
            &descriptor,
            accept_test_binding,
            |reader, _| {
                let mut parsed = Vec::new();
                reader
                    .read_to_end(&mut parsed)
                    .map_err(|error| error.to_string())?;
                Ok(parsed)
            },
        )
        .expect("read bounded payload");
        assert_eq!(parsed, payload);

        let partial = with_kagemusha_pasta_cycle_artifact_payload_content_v4(
            &mut cursor,
            &descriptor,
            accept_test_binding,
            |reader, _| {
                let mut byte = [0_u8; 1];
                reader
                    .read_exact(&mut byte)
                    .map_err(|error| error.to_string())?;
                Ok(byte)
            },
        );
        assert!(partial.is_err());

        let mut zero_header = bytes.clone();
        zero_header[8..12].copy_from_slice(&0_u32.to_le_bytes());
        assert!(
            inspect_kagemusha_pasta_cycle_artifact_content_v4(
                &mut Cursor::new(zero_header),
                &descriptor,
                accept_test_binding,
            )
            .is_err()
        );

        let mut oversized_header = bytes.clone();
        oversized_header[8..12].copy_from_slice(
            &(u32::try_from(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4)
                .expect("header bound fits u32")
                + 1)
            .to_le_bytes(),
        );
        assert!(
            inspect_kagemusha_pasta_cycle_artifact_content_v4(
                &mut Cursor::new(oversized_header),
                &descriptor,
                accept_test_binding,
            )
            .is_err()
        );

        let original_header_len = usize::try_from(u32::from_le_bytes(
            bytes[8..12].try_into().expect("header length fixture"),
        ))
        .expect("header length fits usize");
        let mut noncanonical_header = bytes.clone();
        noncanonical_header.insert(12 + original_header_len, 0);
        noncanonical_header[8..12].copy_from_slice(
            &u32::try_from(original_header_len + 1)
                .expect("small header fixture")
                .to_le_bytes(),
        );
        let mut noncanonical_descriptor = descriptor.clone();
        noncanonical_descriptor.size_bytes += 1;
        noncanonical_descriptor.sha256 = Sha256::digest(&noncanonical_header).into();
        assert!(
            inspect_kagemusha_pasta_cycle_artifact_content_v4(
                &mut Cursor::new(noncanonical_header),
                &noncanonical_descriptor,
                accept_test_binding,
            )
            .is_err()
        );

        let mut payload_tamper = bytes.clone();
        *payload_tamper.last_mut().expect("non-empty fixture") ^= 0x80;
        assert!(
            inspect_kagemusha_pasta_cycle_artifact_content_v4(
                &mut Cursor::new(payload_tamper),
                &descriptor,
                accept_test_binding,
            )
            .is_err()
        );

        let mut trailing = bytes.clone();
        trailing.push(0x55);
        assert!(
            inspect_kagemusha_pasta_cycle_artifact_content_v4(
                &mut Cursor::new(trailing),
                &descriptor,
                accept_test_binding,
            )
            .is_err()
        );

        let mut wrong_descriptor = descriptor.clone();
        wrong_descriptor.sha256[0] ^= 1;
        assert!(
            inspect_kagemusha_pasta_cycle_artifact_content_v4(
                &mut Cursor::new(bytes),
                &wrong_descriptor,
                accept_test_binding,
            )
            .is_err()
        );
    }
}
