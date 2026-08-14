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
    /// Exact canonical payload size of a recursive pair carrying both parent
    /// lineages and both fixed-size accumulation transcripts.
    pub max_recursive_pair_bytes: u32,
}
/// Canonical opaque-pair measurements returned by streaming generation.
pub struct KagemushaGeneratedProofPairMeasurementV4 {
    /// Terminally verified selector-one initialization pair.
    pub initialization_pair_bytes: Vec<u8>,
    /// Exact canonical byte ceiling for a pair carrying recursive lineages and
    /// fixed accumulation transcripts.
    pub max_recursive_pair_bytes: u32,
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
const KAGEMUSHA_INITIALIZATION_RELATION_PAYER_V4: &str = "kagemusha-fixed-padding-payer";
const KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4: u128 = 1;
const KAGEMUSHA_INITIALIZATION_RELATION_LEAF_INDEX_V4: u32 = 0;
const KAGEMUSHA_INITIALIZATION_RELATION_SPEND_KEY_V4: [u8; 32] = [0x46; 32];
const KAGEMUSHA_INITIALIZATION_RELATION_RHO_V4: [u8; 32] = [0x47; 32];
const KAGEMUSHA_INITIALIZATION_RELATION_OPERATION_ID_V4: [u8; 32] = [0x48; 32];
struct KagemushaInitializationRelationV4 {
    topup: super::confidential_v2::KagemushaTopUpShieldPublicInputsV2,
    secure: super::confidential_v2::KagemushaStepSecureWitnessV3,
    output_membership: super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
}
fn kagemusha_initialization_diversifier_v4() -> [u8; 32] {
    let repr = Fp::from(4).to_repr();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(repr.as_ref());
    bytes
}
/// Build the deterministic, satisfying initialization relation shared by key
/// calibration and exact-candidate recursive qualification.
fn kagemusha_initialization_relation_v4(
    network_id: &iroha_data_model::NetworkId,
    asset_definition_id: &str,
    asset_scale: u32,
) -> Result<KagemushaInitializationRelationV4, String> {
    use super::{confidential_v2, kagemusha_v2};
    let diversifier = kagemusha_initialization_diversifier_v4();
    let empty_path = confidential_v2::compute_confidential_merkle_path_v3(&[], 0)?;
    let secure = confidential_v2::prepare_kagemusha_step_topup_witness_v3(
        network_id,
        asset_definition_id,
        KAGEMUSHA_INITIALIZATION_RELATION_PAYER_V4,
        KAGEMUSHA_INITIALIZATION_RELATION_OPERATION_ID_V4,
        KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        asset_scale,
        &KAGEMUSHA_INITIALIZATION_RELATION_SPEND_KEY_V4,
        KAGEMUSHA_INITIALIZATION_RELATION_RHO_V4,
        diversifier,
        KAGEMUSHA_INITIALIZATION_RELATION_LEAF_INDEX_V4,
        &empty_path,
    )?;
    let asset_tag = confidential_v2::derive_confidential_asset_tag_v3(asset_definition_id)?;
    let network_tag = confidential_v2::derive_confidential_network_tag_v3(network_id)?;
    let payer_tag = confidential_v2::derive_kagemusha_topup_payer_tag_v3(
        KAGEMUSHA_INITIALIZATION_RELATION_PAYER_V4,
    )?;
    let operation_tag = confidential_v2::derive_kagemusha_topup_operation_tag_v3(
        &KAGEMUSHA_INITIALIZATION_RELATION_OPERATION_ID_V4,
    )?;
    let owner_tag = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &KAGEMUSHA_INITIALIZATION_RELATION_SPEND_KEY_V4,
        diversifier,
    )?;
    let output_commitment = confidential_v2::derive_confidential_note_v3(
        asset_tag,
        KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        KAGEMUSHA_INITIALIZATION_RELATION_RHO_V4,
        owner_tag,
    )?;
    let spend_nullifier = confidential_v2::derive_confidential_nullifier_v3(
        &KAGEMUSHA_INITIALIZATION_RELATION_SPEND_KEY_V4,
        KAGEMUSHA_INITIALIZATION_RELATION_RHO_V4,
        asset_tag,
        network_tag,
    )?;
    let initial_root = confidential_v2::compute_confidential_root_v3(&[])?;
    let final_commitments = [output_commitment];
    let final_root = confidential_v2::compute_confidential_root_v3(&final_commitments)?;
    if empty_path.root != initial_root {
        return Err("Kagemusha V4 initialization empty path/root mismatch".to_owned());
    }
    let recipient_update_path = kagemusha_calibration_membership_path_v4(empty_path.clone());
    let recipient_membership_path = kagemusha_calibration_membership_path_v4(
        confidential_v2::compute_confidential_merkle_path_v3(&final_commitments, 0)?,
    );
    let dummy_leaf_index = KAGEMUSHA_INITIALIZATION_RELATION_LEAF_INDEX_V4
        .checked_add(1)
        .ok_or_else(|| "Kagemusha V4 initialization dummy index overflow".to_owned())?;
    let dummy_path = kagemusha_calibration_membership_path_v4(
        confidential_v2::compute_confidential_merkle_path_v3(
            &final_commitments,
            usize::try_from(dummy_leaf_index)
                .map_err(|_| "Kagemusha V4 initialization dummy index does not fit usize")?,
        )?,
    );
    let output_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Init,
        initial_root,
        final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: output_commitment,
            leaf_index: KAGEMUSHA_INITIALIZATION_RELATION_LEAF_INDEX_V4,
            update_path: recipient_update_path,
            membership_path: recipient_membership_path,
        }),
        change: None,
        dummy_leaf_index,
        dummy_path,
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(output_membership.clone())?;
    let topup = confidential_v2::KagemushaTopUpShieldPublicInputsV2 {
        output_commitment,
        spend_nullifier,
        initial_root,
        finalized_root: final_root,
        atomic_amount: iroha_data_model::offline::kagemusha_confidential_amount_encoding_v2(
            KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        ),
        asset_scale: {
            let mut encoded = [0_u8; 32];
            encoded[..4].copy_from_slice(&asset_scale.to_le_bytes());
            encoded
        },
        leaf_index: {
            let mut encoded = [0_u8; 32];
            encoded[..4]
                .copy_from_slice(&KAGEMUSHA_INITIALIZATION_RELATION_LEAF_INDEX_V4.to_le_bytes());
            encoded
        },
        asset_tag,
        network_tag,
        payer_tag,
        operation_tag,
    };
    Ok(KagemushaInitializationRelationV4 {
        topup,
        secure,
        output_membership,
    })
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
    use iroha_data_model::NetworkId;
    use super::kagemusha_v2;
    const ASSET_DEFINITION: &str = "kagemusha-fixed-padding#internal";
    const NETWORK_SEED: &[u8] = b"kagemusha-fixed-padding-network";
    const ASSET_SCALE: u32 = 0;
    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::new(NETWORK_SEED)
    ));
    let KagemushaInitializationRelationV4 {
        topup,
        secure,
        output_membership,
    } = kagemusha_initialization_relation_v4(&network_id, ASSET_DEFINITION, ASSET_SCALE)?;
    let asset_tag = topup.asset_tag;
    let network_tag = topup.network_tag;
    let payer_tag = topup.payer_tag;
    let operation_tag = topup.operation_tag;
    let output_commitment = topup.output_commitment;
    let spend_nullifier = topup.spend_nullifier;
    let initial_root = topup.initial_root;
    let final_root = topup.finalized_root;
    let operation_id = KAGEMUSHA_INITIALIZATION_RELATION_OPERATION_ID_V4;
    let amount = KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4;
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
        fields[index] = Fp::from_u128(amount);
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
        (kagemusha_v2::I_NETWORK_TAG, network_tag, "network tag"),
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
        vec![0_u32; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
    result_state[kagemusha_v2::S_VERSION] =
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    result_state[kagemusha_v2::S_NETWORK_TAG..kagemusha_v2::S_NETWORK_TAG + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(network_tag));
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
        .zip(amount.to_le_bytes().chunks_exact(4))
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
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5]
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
const KAGEMUSHA_CANDIDATE_STEP_TWO_KEY_SET_DOMAIN_V4: &[u8] =
    b"iroha:kagemusha:candidate-recursive-step-two-key-set:v4";
fn kagemusha_candidate_step_two_key_set_sha256_v4(
    candidate_sha256: [u8; 32],
    manifest_sha256: [u8; 32],
    role_digests: [[u8; 32]; 8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_CANDIDATE_STEP_TWO_KEY_SET_DOMAIN_V4);
    hasher.update([0]);
    hasher.update(candidate_sha256);
    hasher.update(manifest_sha256);
    for digest in role_digests {
        hasher.update(digest);
    }
    hasher.finalize().into()
}
fn kagemusha_candidate_step_two_role_digests_v4(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<[[u8; 32]; 8], String> {
    let descriptor = |parity, kind| {
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
            .ok_or_else(|| "Kagemusha V4 candidate key role is absent".to_owned())
    };
    let step_eq_pk = descriptor(
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    )?;
    let step_eq_vk = descriptor(
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    let step_ep_pk = descriptor(
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    )?;
    let step_ep_vk = descriptor(
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    Ok([
        step_eq_pk.sha256,
        step_eq_pk.payload_sha256,
        step_eq_vk.sha256,
        step_eq_vk.payload_sha256,
        step_ep_pk.sha256,
        step_ep_pk.payload_sha256,
        step_ep_vk.sha256,
        step_ep_vk.payload_sha256,
    ])
}
/// Strict proof that an exact unsigned candidate completed a real step-one to
/// step-two recursion and both pairs passed a freshly loaded terminal verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaCandidateRecursiveStepTwoEvidenceV4 {
    /// Canonical candidate-record identity.
    candidate_sha256: [u8; 32],
    /// Canonical unsigned-manifest identity selected by both proofs.
    manifest_sha256: [u8; 32],
    /// Exact framed Eq proving-key bytes staged in the candidate.
    step_eq_proving_key_framed_sha256: [u8; 32],
    /// Exact Eq proving-key payload staged in the candidate.
    step_eq_proving_key_payload_sha256: [u8; 32],
    /// Exact framed Eq verifying-key bytes staged in the candidate.
    step_eq_verifying_key_framed_sha256: [u8; 32],
    /// Exact Eq verifying-key payload staged in the candidate.
    step_eq_verifying_key_payload_sha256: [u8; 32],
    /// Exact framed Ep proving-key bytes staged in the candidate.
    step_ep_proving_key_framed_sha256: [u8; 32],
    /// Exact Ep proving-key payload staged in the candidate.
    step_ep_proving_key_payload_sha256: [u8; 32],
    /// Exact framed Ep verifying-key bytes staged in the candidate.
    step_ep_verifying_key_framed_sha256: [u8; 32],
    /// Exact Ep verifying-key payload staged in the candidate.
    step_ep_verifying_key_payload_sha256: [u8; 32],
    /// Key-set identity used while proving initialization.
    initialization_key_set_sha256: [u8; 32],
    /// Key-set identity used while proving the recursive child.
    append_key_set_sha256: [u8; 32],
    /// Key-set identity freshly loaded for terminal verification.
    terminal_key_set_sha256: [u8; 32],
    /// SHA-256 of the exact canonical step-one opaque pair.
    initialization_pair_sha256: [u8; 32],
    /// Canonical semantic bundle digest containing the exact step-one pair.
    initialization_bundle_digest: [u8; 32],
    /// SHA-256 of the exact canonical step-two opaque pair.
    append_pair_sha256: [u8; 32],
    /// Parent bundle digest publicly bound by the step-two operation.
    append_bound_parent_bundle_digest: [u8; 32],
    /// Proof-step counter decoded from the step-one pair.
    initialization_proof_step_count: u32,
    /// Parent count decoded from the step-one pair.
    initialization_parent_count: u32,
    /// Proof-step counter decoded from the step-two pair.
    append_proof_step_count: u32,
    /// Parent count decoded from the step-two pair.
    append_parent_count: u32,
    /// Number of pairs accepted by the freshly loaded terminal verifier.
    terminal_verified_pair_count: u32,
}
impl KagemushaCandidateRecursiveStepTwoEvidenceV4 {
    fn role_digests(&self) -> [[u8; 32]; 8] {
        [
            self.step_eq_proving_key_framed_sha256,
            self.step_eq_proving_key_payload_sha256,
            self.step_eq_verifying_key_framed_sha256,
            self.step_eq_verifying_key_payload_sha256,
            self.step_ep_proving_key_framed_sha256,
            self.step_ep_proving_key_payload_sha256,
            self.step_ep_verifying_key_framed_sha256,
            self.step_ep_verifying_key_payload_sha256,
        ]
    }
    /// Enforce the exact two-step, one-parent, one-key-set terminal evidence
    /// contract consumed by candidate publication.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero or substituted identity, any counter other
    /// than step `1/0 parents` followed by step `2/1 parent`, a child not bound
    /// to the canonical step-one bundle, or fewer/more than two terminal
    /// decisions.
    pub fn validate(&self) -> Result<(), String> {
        let role_digests = self.role_digests();
        let expected_key_set = kagemusha_candidate_step_two_key_set_sha256_v4(
            self.candidate_sha256,
            self.manifest_sha256,
            role_digests,
        );
        if self.candidate_sha256 == [0; 32]
            || self.manifest_sha256 == [0; 32]
            || role_digests.iter().any(|digest| *digest == [0; 32])
            || self.initialization_key_set_sha256 != expected_key_set
            || self.append_key_set_sha256 != expected_key_set
            || self.terminal_key_set_sha256 != expected_key_set
            || self.initialization_pair_sha256 == [0; 32]
            || self.append_pair_sha256 == [0; 32]
            || self.initialization_pair_sha256 == self.append_pair_sha256
            || self.initialization_bundle_digest == [0; 32]
            || self.append_bound_parent_bundle_digest != self.initialization_bundle_digest
            || self.initialization_proof_step_count != 1
            || self.initialization_parent_count != 0
            || self.append_proof_step_count != 2
            || self.append_parent_count != 1
            || self.terminal_verified_pair_count != 2
        {
            return Err(
                "Kagemusha V4 candidate does not carry exact verified step-one to step-two evidence"
                    .to_owned(),
            );
        }
        Ok(())
    }
    /// Rebind this result to the exact canonical candidate and its staged
    /// proving/verifying-key descriptors.
    ///
    /// # Errors
    ///
    /// Returns an error when the candidate is invalid or any candidate,
    /// manifest, framed-key, or key-payload identity differs from the evidence.
    pub fn validate_for_candidate(
        &self,
        candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    ) -> Result<(), String> {
        self.validate()?;
        candidate.validate().map_err(|error| error.to_string())?;
        let candidate_sha256 = candidate.sha256().map_err(|error| error.to_string())?;
        let manifest_sha256: [u8; 32] = Sha256::digest(
            norito::encode_canonical(&candidate.manifest)
                .map_err(|error| format!("failed to encode Kagemusha V4 candidate: {error}"))?,
        )
        .into();
        let role_digests = kagemusha_candidate_step_two_role_digests_v4(&candidate.manifest)?;
        if self.candidate_sha256 != candidate_sha256
            || self.manifest_sha256 != manifest_sha256
            || self.role_digests() != role_digests
        {
            return Err(
                "Kagemusha V4 recursive-step evidence identifies different staged candidate bytes"
                    .to_owned(),
            );
        }
        Ok(())
    }
}
fn kagemusha_candidate_qualification_init_statement_v4(
    candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    manifest_sha256: [u8; 32],
    relation: &KagemushaInitializationRelationV4,
) -> Result<KagemushaRecursiveSpendPublicStatementV4, String> {
    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KagemushaRecursiveSpendArtifactBindingV4,
        KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendTopUpAnchorRefV2,
        KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
        kagemusha_recursive_spend_verifier_key_id_v4,
    };
    let manifest = &candidate.manifest;
    let anchor_digest = [0x31_u8; 32];
    let anchor_ref = KagemushaRecursiveSpendTopUpAnchorRefV2 {
        topup_operation_id: KAGEMUSHA_INITIALIZATION_RELATION_OPERATION_ID_V4,
        anchor_digest,
    };
    anchor_ref.validate().map_err(|error| error.to_string())?;
    let amount = KagemushaScaledAmountV2::new(
        KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        manifest.asset_scale,
    )
    .map_err(|error| error.to_string())?;
    let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: manifest.generation.clone(),
        manifest_sha256,
    };
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        network_id: manifest.network_id,
        asset: manifest.asset.clone(),
        asset_scale: manifest.asset_scale,
        final_root: relation.topup.finalized_root,
        next_zero_leaf_index: relation.output_membership.dummy_leaf_index,
        topup_anchor_refs: vec![anchor_ref],
        proof_step_count: 1,
        peer_hop_count: 0,
        current_note: KagemushaSpendableNoteDescriptorV2 {
            network_id: manifest.network_id,
            asset: manifest.asset.clone(),
            note_commitment: relation.topup.output_commitment,
            spend_nullifier: relation.topup.spend_nullifier,
            amount,
        },
        branch_claims: vec![
            KagemushaRecursiveSpendBranchClaimV2::root(anchor_digest)
                .map_err(|error| error.to_string())?,
        ],
        transition: None,
        verifier_key_id: kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            manifest_sha256,
        ),
        artifact_binding,
    };
    statement
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    Ok(statement)
}
fn kagemusha_candidate_qualification_bundle_v4(
    candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    manifest_sha256: [u8; 32],
    statement: KagemushaRecursiveSpendPublicStatementV4,
    operation: &KagemushaStepOperationVectorV4,
    pair_bytes: Vec<u8>,
) -> Result<iroha_data_model::offline::KagemushaRecursiveSpendBundleV4, String> {
    use iroha_data_model::{
        offline::{
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            KagemushaPastaCycleProofEnvelopeV4, KagemushaRecursiveSpendBundleV4,
            KagemushaRecursiveSpendProofV4, KagemushaRecursiveSpendStateBoundaryV5,
        },
        proof::ProofBox,
    };
    let manifest = &candidate.manifest;
    let [step_eq, step_ep] = manifest.profiles.as_slice() else {
        return Err("Kagemusha V4 candidate does not have exactly two profiles".to_owned());
    };
    let verifier_key_sha256 =
        |profile: &iroha_data_model::offline::KagemushaPastaCycleProofProfileV4| {
            profile
                .artifacts
                .iter()
                .find(|artifact| artifact.kind == KagemushaPastaCycleArtifactKindV4::VerifyingKey)
                .map(|artifact| artifact.payload_sha256)
                .ok_or_else(|| "Kagemusha V4 candidate verifier key is absent".to_owned())
        };
    let state =
        super::kagemusha_v2::KagemushaRecursiveSpendStateVectorV5::from_statement_v4(&statement)?;
    let proof_backend = manifest.proof_backend.parse().map_err(|_| {
        format!(
            "invalid Kagemusha V4 proof backend `{}`",
            manifest.proof_backend
        )
    })?;
    let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        proof_backend: manifest.proof_backend.clone(),
        transcript_profile: manifest.transcript_profile.clone(),
        step_eq_circuit_id: step_eq.circuit_id.clone(),
        step_ep_circuit_id: step_ep.circuit_id.clone(),
        artifact_generation: manifest.generation.clone(),
        manifest_sha256,
        step_eq_parameter_generation: step_eq.parameter_generation.clone(),
        step_ep_parameter_generation: step_ep.parameter_generation.clone(),
        step_eq_circuit_params_sha256: step_eq
            .circuit_params_sha256()
            .map_err(|error| error.to_string())?,
        step_ep_circuit_params_sha256: step_ep
            .circuit_params_sha256()
            .map_err(|error| error.to_string())?,
        step_eq_verifier_key_sha256: verifier_key_sha256(step_eq)?,
        step_ep_verifier_key_sha256: verifier_key_sha256(step_ep)?,
        state_boundary: KagemushaRecursiveSpendStateBoundaryV5::new(state.limbs.to_vec())
            .map_err(|error| error.to_string())?,
        proof: ProofBox::new(proof_backend, pair_bytes),
    };
    proof_envelope
        .validate_against_candidate_manifest(manifest)
        .map_err(|error| error.to_string())?;
    let public_statement_digest = statement.digest().map_err(|error| error.to_string())?;
    let verifier_key_id = statement.verifier_key_id.clone();
    let bundle = KagemushaRecursiveSpendBundleV4 {
        statement,
        operation: operation.into(),
        recursive_proof: KagemushaRecursiveSpendProofV4 {
            verifier_key_id,
            public_statement_digest,
            proof_envelope,
        },
    };
    bundle
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    Ok(bundle)
}
fn kagemusha_candidate_private_path_v4(
    path: &iroha_data_model::offline::KagemushaConfidentialMerklePathV2,
) -> super::confidential_v2::ConfidentialMerklePathV2 {
    super::confidential_v2::ConfidentialMerklePathV2 {
        siblings: path.siblings.clone(),
        directions: path.directions.clone(),
        witness_nodes: Vec::new(),
        root: path.root,
    }
}
struct KagemushaCandidateQualificationAppendV4 {
    initialization_bundle_digest: [u8; 32],
    bound_parent_bundle_digest: [u8; 32],
    statement: KagemushaRecursiveSpendPublicStatementV4,
    operation: KagemushaStepOperationVectorV4,
}
#[allow(clippy::too_many_lines)]
fn kagemusha_candidate_qualification_append_v4(
    candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    manifest_sha256: [u8; 32],
    relation: &KagemushaInitializationRelationV4,
    init_statement: &KagemushaRecursiveSpendPublicStatementV4,
    init_operation: &KagemushaStepOperationVectorV4,
    init_pair: &[u8],
) -> Result<KagemushaCandidateQualificationAppendV4, String> {
    use super::{confidential_v2, kagemusha_v2};
    use iroha_data_model::offline::{
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendInputBranchV2,
        KagemushaRecursiveSpendSplitIntentV4, KagemushaScaledAmountV2,
        KagemushaSpendableNoteDescriptorV2,
    };
    let init_bundle = kagemusha_candidate_qualification_bundle_v4(
        candidate,
        manifest_sha256,
        init_statement.clone(),
        init_operation,
        init_pair.to_vec(),
    )?;
    let initialization_bundle_digest = init_bundle.digest().map_err(|error| error.to_string())?;
    let input_leaf = relation
        .output_membership
        .recipient
        .as_ref()
        .ok_or_else(|| "Kagemusha V4 qualification init has no output leaf".to_owned())?;
    let input_path = confidential_v2::validate_confidential_membership_path_v3(
        input_leaf.commitment,
        usize::try_from(input_leaf.leaf_index)
            .map_err(|_| "Kagemusha V4 qualification input leaf does not fit usize")?,
        &kagemusha_candidate_private_path_v4(&input_leaf.membership_path),
    )?;
    let next_zero_leaf_index = usize::try_from(relation.output_membership.dummy_leaf_index)
        .map_err(|_| "Kagemusha V4 qualification frontier does not fit usize")?;
    let next_zero_path = confidential_v2::validate_confidential_next_zero_path_v3(
        next_zero_leaf_index,
        &kagemusha_candidate_private_path_v4(&relation.output_membership.dummy_path),
    )?;
    let recipient_spend_key = [0x61_u8; 32];
    let recipient_rho = [0x62_u8; 32];
    let recipient_diversifier = confidential_v2::derive_confidential_diversifier_v2(
        b"iroha:kagemusha:candidate-recursive-step-two-recipient:v4",
    );
    let recipient_owner_tag = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &recipient_spend_key,
        recipient_diversifier,
    )?;
    let recipient_commitment = confidential_v2::derive_confidential_note_v3(
        relation.topup.asset_tag,
        KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        recipient_rho,
        recipient_owner_tag,
    )?;
    let recipient_nullifier = confidential_v2::derive_confidential_nullifier_v3(
        &recipient_spend_key,
        recipient_rho,
        relation.topup.asset_tag,
        relation.topup.network_tag,
    )?;
    let append_paths = confidential_v2::derive_confidential_sequential_append_paths_v3(
        next_zero_leaf_index,
        &next_zero_path,
        &[recipient_commitment],
    )?;
    let [recipient_paths] = append_paths.leaves.as_slice() else {
        return Err("Kagemusha V4 qualification append did not derive one output".to_owned());
    };
    let output_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Split,
        initial_root: append_paths.initial_root,
        final_root: append_paths.final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: recipient_commitment,
            leaf_index: u32::try_from(recipient_paths.leaf_index)
                .map_err(|_| "Kagemusha V4 qualification recipient leaf does not fit u32")?,
            update_path: kagemusha_calibration_membership_path_v4(
                recipient_paths.update_path.clone(),
            ),
            membership_path: kagemusha_calibration_membership_path_v4(
                recipient_paths.membership_path.clone(),
            ),
        }),
        change: None,
        dummy_leaf_index: u32::try_from(append_paths.next_zero_leaf_index)
            .map_err(|_| "Kagemusha V4 qualification next frontier does not fit u32")?,
        dummy_path: kagemusha_calibration_membership_path_v4(append_paths.next_zero_path.clone()),
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(output_membership.clone())?;
    let amount = KagemushaScaledAmountV2::new(
        KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        candidate.manifest.asset_scale,
    )
    .map_err(|error| error.to_string())?;
    let recipient_note = KagemushaSpendableNoteDescriptorV2 {
        network_id: candidate.manifest.network_id,
        asset: candidate.manifest.asset.clone(),
        note_commitment: recipient_commitment,
        spend_nullifier: recipient_nullifier,
        amount,
    };
    let split = KagemushaRecursiveSpendSplitIntentV4 {
        network_id: candidate.manifest.network_id,
        asset: candidate.manifest.asset.clone(),
        inputs: vec![KagemushaRecursiveSpendInputBranchV2 {
            bundle_digest: initialization_bundle_digest,
            input_note: init_statement.current_note.clone(),
            branch_claims: init_statement.branch_claims.clone(),
            input_root: init_statement.final_root,
            proof_step_count: init_statement.proof_step_count,
            peer_hop_count: init_statement.peer_hop_count,
        }],
        topup_anchor_refs: init_statement.topup_anchor_refs.clone(),
        asset_scale: candidate.manifest.asset_scale,
        output_artifact_binding: init_statement.artifact_binding.clone(),
        transfer_amount: amount,
        recipient_output: recipient_note,
        change_output: None,
        recipient_request_digest: [0xA6; 32],
        operation_id: [0xA8; 32],
    };
    split
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let statement = kagemusha_v2::kagemusha_recursive_spend_append_statement_v4(
        &split,
        KagemushaRecursiveSpendBranchV2::Recipient,
        output_membership.final_root,
        output_membership.dummy_leaf_index,
    )?;
    let transfer_public = super::kagemusha_step_transition::KagemushaStepTransferPublicV4 {
        input_commitments: [init_statement.current_note.note_commitment, [0; 32]],
        input_nullifiers: [init_statement.current_note.spend_nullifier, [0; 32]],
        output_commitments: [recipient_commitment, [0; 32]],
        root: init_statement.final_root,
        asset_tag: relation.topup.asset_tag,
        network_tag: relation.topup.network_tag,
    };
    let operation = KagemushaStepOperationVectorV4::from_append_v4(
        &split,
        &statement,
        &transfer_public,
        &output_membership,
    )?;
    let input_paths = vec![input_path, next_zero_path];
    let inputs = vec![confidential_v2::ConfidentialTransferInputV2 {
        amount: KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        rho: KAGEMUSHA_INITIALIZATION_RELATION_RHO_V4,
        diversifier: kagemusha_initialization_diversifier_v4(),
        leaf_index: usize::try_from(KAGEMUSHA_INITIALIZATION_RELATION_LEAF_INDEX_V4)
            .map_err(|_| "Kagemusha V4 qualification leaf index does not fit usize")?,
    }];
    let outputs = vec![confidential_v2::ConfidentialTransferOutputV2 {
        amount: KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        rho: recipient_rho,
        owner_tag: recipient_owner_tag,
    }];
    confidential_v2::prepare_kagemusha_step_transfer_witness_v3_with_paths(
        &candidate.manifest.network_id,
        &candidate.manifest.asset.to_string(),
        &KAGEMUSHA_INITIALIZATION_RELATION_SPEND_KEY_V4,
        &input_paths,
        &inputs,
        &outputs,
        init_statement.final_root,
    )?;
    Ok(KagemushaCandidateQualificationAppendV4 {
        initialization_bundle_digest,
        bound_parent_bundle_digest: split.inputs[0].bundle_digest,
        statement,
        operation,
    })
}
/// Prove and terminally verify a genuine recursive child with the exact
/// proving and verifying keys staged in one unsigned candidate.
///
/// This is a pre-promotion qualification boundary. It authenticates candidate
/// bytes and proves the real Step relation, but it does not confer release
/// authority or bypass attestation, review, benchmark, or finality gates.
///
/// # Errors
///
/// Returns an error when the candidate or a staged role changes, a proving key
/// does not embed the exact staged verifying key, either proof fails, the
/// second proof is not the sole child of the first canonical bundle, or the
/// freshly loaded terminal verifier does not accept exactly both pairs.
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn generate_candidate_recursive_step_two_receipt_v4<F>(
    candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    expected_candidate_sha256: [u8; 32],
    expected_manifest_sha256: [u8; 32],
    memory_guard: &KagemushaGenerationMemoryGuardV4,
    step_eq_proving_key_file: std::fs::File,
    step_ep_proving_key_file: std::fs::File,
    mut load: F,
) -> Result<iroha_data_model::offline::KagemushaRecursiveSpendQualificationReceiptV4, String>
where
    F: FnMut(
        KagemushaPastaCycleParityV1,
        KagemushaPastaCycleArtifactKindV4,
    )
        -> Result<super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4, String>,
{
    use iroha_data_model::offline::{
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendInputBranchV2,
        KagemushaRecursiveSpendSplitIntentV4, KagemushaScaledAmountV2,
        KagemushaSpendableNoteDescriptorV2,
    };
    use super::{confidential_v2, kagemusha_v2};
    candidate.validate().map_err(|error| error.to_string())?;
    KagemushaQualificationMemoryContractV4::for_operator(memory_guard)
        .validate_candidate(candidate)?;
    let candidate_sha256 = candidate.sha256().map_err(|error| error.to_string())?;
    let manifest_sha256: [u8; 32] = Sha256::digest(
        norito::encode_canonical(&candidate.manifest)
            .map_err(|error| format!("failed to encode Kagemusha V4 candidate: {error}"))?,
    )
    .into();
    validate_kagemusha_candidate_spool_identity_v5(
        candidate_sha256,
        manifest_sha256,
        expected_candidate_sha256,
        expected_manifest_sha256,
    )?;
    let role_digests = kagemusha_candidate_step_two_role_digests_v4(&candidate.manifest)?;
    let key_set_sha256 = kagemusha_candidate_step_two_key_set_sha256_v4(
        candidate_sha256,
        manifest_sha256,
        role_digests,
    );
    let terminal_binding = KagemushaArtifactSpoolBindingV5::candidate_evidence_lab(
        candidate,
        expected_candidate_sha256,
        expected_manifest_sha256,
    )?;
    let relation = kagemusha_initialization_relation_v4(
        &candidate.manifest.network_id,
        &candidate.manifest.asset.to_string(),
        candidate.manifest.asset_scale,
    )?;
    let init_statement =
        kagemusha_candidate_qualification_init_statement_v4(candidate, manifest_sha256, &relation)?;
    let init_operation = KagemushaStepOperationVectorV4::from_candidate_qualification_init_v4(
        &init_statement,
        &relation.topup,
        &relation.output_membership,
        KAGEMUSHA_INITIALIZATION_RELATION_PAYER_V4,
    )?;
    // The constructor authenticates both framed proving-key spools, parses the
    // exact candidate verifier payloads, and rejects a PK whose embedded VK
    // differs byte-for-byte from that staged VK.
    let prover = KagemushaPastaCycleProverV4::from_candidate_artifact_spool_loader(
        candidate,
        expected_candidate_sha256,
        expected_manifest_sha256,
        step_eq_proving_key_file,
        step_ep_proving_key_file,
        |parity, kind| load(parity, kind),
    )?;
    let init_public_inputs = kagemusha_v2::kagemusha_public_inputs_for_statement_v4(
        &init_statement,
        init_operation.clone(),
        manifest_sha256,
        prover.step_eq_compiled_protocol_sha256(),
        prover.step_ep_compiled_protocol_sha256(),
    )?;
    let init_pair = prover.prove_operation_encoded_v4(
        init_public_inputs,
        1,
        &[],
        &[],
        &relation.secure,
        &relation.output_membership,
    )?;
    let init_bundle = kagemusha_candidate_qualification_bundle_v4(
        candidate,
        manifest_sha256,
        init_statement.clone(),
        &init_operation,
        init_pair.clone(),
    )?;
    let initialization_bundle_digest = init_bundle.digest().map_err(|error| error.to_string())?;
    drop(init_bundle);
    let input_leaf = relation
        .output_membership
        .recipient
        .as_ref()
        .ok_or_else(|| "Kagemusha V4 qualification init has no output leaf".to_owned())?;
    let input_path = confidential_v2::validate_confidential_membership_path_v3(
        input_leaf.commitment,
        usize::try_from(input_leaf.leaf_index)
            .map_err(|_| "Kagemusha V4 qualification input leaf does not fit usize")?,
        &kagemusha_candidate_private_path_v4(&input_leaf.membership_path),
    )?;
    let next_zero_leaf_index = usize::try_from(relation.output_membership.dummy_leaf_index)
        .map_err(|_| "Kagemusha V4 qualification frontier does not fit usize")?;
    let next_zero_path = confidential_v2::validate_confidential_next_zero_path_v3(
        next_zero_leaf_index,
        &kagemusha_candidate_private_path_v4(&relation.output_membership.dummy_path),
    )?;
    let recipient_spend_key = [0x61_u8; 32];
    let recipient_rho = [0x62_u8; 32];
    let recipient_diversifier = confidential_v2::derive_confidential_diversifier_v2(
        b"iroha:kagemusha:candidate-recursive-step-two-recipient:v4",
    );
    let recipient_owner_tag = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &recipient_spend_key,
        recipient_diversifier,
    )?;
    let recipient_commitment = confidential_v2::derive_confidential_note_v3(
        relation.topup.asset_tag,
        KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        recipient_rho,
        recipient_owner_tag,
    )?;
    let recipient_nullifier = confidential_v2::derive_confidential_nullifier_v3(
        &recipient_spend_key,
        recipient_rho,
        relation.topup.asset_tag,
        relation.topup.network_tag,
    )?;
    let append_paths = confidential_v2::derive_confidential_sequential_append_paths_v3(
        next_zero_leaf_index,
        &next_zero_path,
        &[recipient_commitment],
    )?;
    let [recipient_paths] = append_paths.leaves.as_slice() else {
        return Err("Kagemusha V4 qualification append did not derive one output".to_owned());
    };
    let append_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Split,
        initial_root: append_paths.initial_root,
        final_root: append_paths.final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: recipient_commitment,
            leaf_index: u32::try_from(recipient_paths.leaf_index)
                .map_err(|_| "Kagemusha V4 qualification recipient leaf does not fit u32")?,
            update_path: kagemusha_calibration_membership_path_v4(
                recipient_paths.update_path.clone(),
            ),
            membership_path: kagemusha_calibration_membership_path_v4(
                recipient_paths.membership_path.clone(),
            ),
        }),
        change: None,
        dummy_leaf_index: u32::try_from(append_paths.next_zero_leaf_index)
            .map_err(|_| "Kagemusha V4 qualification next frontier does not fit u32")?,
        dummy_path: kagemusha_calibration_membership_path_v4(append_paths.next_zero_path.clone()),
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(append_membership.clone())?;
    let amount = KagemushaScaledAmountV2::new(
        KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        candidate.manifest.asset_scale,
    )
    .map_err(|error| error.to_string())?;
    let recipient_note = KagemushaSpendableNoteDescriptorV2 {
        network_id: candidate.manifest.network_id,
        asset: candidate.manifest.asset.clone(),
        note_commitment: recipient_commitment,
        spend_nullifier: recipient_nullifier,
        amount,
    };
    let split = KagemushaRecursiveSpendSplitIntentV4 {
        network_id: candidate.manifest.network_id,
        asset: candidate.manifest.asset.clone(),
        inputs: vec![KagemushaRecursiveSpendInputBranchV2 {
            bundle_digest: initialization_bundle_digest,
            input_note: init_statement.current_note.clone(),
            branch_claims: init_statement.branch_claims.clone(),
            input_root: init_statement.final_root,
            proof_step_count: init_statement.proof_step_count,
            peer_hop_count: init_statement.peer_hop_count,
        }],
        topup_anchor_refs: init_statement.topup_anchor_refs.clone(),
        asset_scale: candidate.manifest.asset_scale,
        output_artifact_binding: init_statement.artifact_binding.clone(),
        transfer_amount: amount,
        recipient_output: recipient_note,
        change_output: None,
        recipient_request_digest: [0xA6; 32],
        operation_id: [0xA8; 32],
    };
    split
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let append_statement = kagemusha_v2::kagemusha_recursive_spend_append_statement_v4(
        &split,
        KagemushaRecursiveSpendBranchV2::Recipient,
        append_membership.final_root,
        append_membership.dummy_leaf_index,
    )?;
    let transfer_public = super::kagemusha_step_transition::KagemushaStepTransferPublicV4 {
        input_commitments: [init_statement.current_note.note_commitment, [0; 32]],
        input_nullifiers: [init_statement.current_note.spend_nullifier, [0; 32]],
        output_commitments: [recipient_commitment, [0; 32]],
        root: init_statement.final_root,
        asset_tag: relation.topup.asset_tag,
        network_tag: relation.topup.network_tag,
    };
    let append_operation = KagemushaStepOperationVectorV4::from_append_v4(
        &split,
        &append_statement,
        &transfer_public,
        &append_membership,
    )?;
    let input_paths = vec![input_path, next_zero_path];
    let inputs = vec![confidential_v2::ConfidentialTransferInputV2 {
        amount: KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        rho: KAGEMUSHA_INITIALIZATION_RELATION_RHO_V4,
        diversifier: kagemusha_initialization_diversifier_v4(),
        leaf_index: usize::try_from(KAGEMUSHA_INITIALIZATION_RELATION_LEAF_INDEX_V4)
            .map_err(|_| "Kagemusha V4 qualification leaf index does not fit usize")?,
    }];
    let outputs = vec![confidential_v2::ConfidentialTransferOutputV2 {
        amount: KAGEMUSHA_INITIALIZATION_RELATION_AMOUNT_V4,
        rho: recipient_rho,
        owner_tag: recipient_owner_tag,
    }];
    let append_secure = confidential_v2::prepare_kagemusha_step_transfer_witness_v3_with_paths(
        &candidate.manifest.network_id,
        &candidate.manifest.asset.to_string(),
        &KAGEMUSHA_INITIALIZATION_RELATION_SPEND_KEY_V4,
        &input_paths,
        &inputs,
        &outputs,
        init_statement.final_root,
    )?;
    let append_public_inputs = kagemusha_v2::kagemusha_public_inputs_for_statement_v4(
        &append_statement,
        append_operation.clone(),
        manifest_sha256,
        prover.step_eq_compiled_protocol_sha256(),
        prover.step_ep_compiled_protocol_sha256(),
    )?;
    let parent_state =
        kagemusha_v2::KagemushaRecursiveSpendStateVectorV5::from_statement_v4(&init_statement)?;
    let append_pair = prover.prove_operation_encoded_v4(
        append_public_inputs,
        2,
        &[init_pair.as_slice()],
        &[parent_state.limbs.to_vec()],
        &append_secure,
        &append_membership,
    )?;
    let (
        initialization_proof_step_count,
        initialization_parent_count,
        append_proof_step_count,
        append_parent_count,
    ) = {
        let init_decoded = KagemushaPastaCycleProofPairV4::decode_authenticated(
            &init_pair,
            &prover.step_eq_circuit_params,
            &prover.step_ep_circuit_params,
            prover.max_pair_bytes,
        )?;
        let append_decoded = KagemushaPastaCycleProofPairV4::decode_authenticated(
            &append_pair,
            &prover.step_eq_circuit_params,
            &prover.step_ep_circuit_params,
            prover.max_pair_bytes,
        )?;
        (
            init_decoded.proof_step_count,
            init_decoded.public_inputs.parent_count()?,
            append_decoded.proof_step_count,
            append_decoded.public_inputs.parent_count()?,
        )
    };
    drop(prover);
    // Reopen the bounded verifier roles after all proving-key state has been
    // dropped. Every role is rebound to the same unsigned candidate before it
    // is parsed, then both semantic statements receive a full terminal decision.
    let terminal = KagemushaPastaCycleTerminalVerifierV4::from_validated_artifact_loader(
        &candidate.manifest,
        |parity, kind| {
            let payload = load(parity, kind)?;
            terminal_binding.validate_payload(&payload, parity, kind)?;
            Ok(payload)
        },
    )?;
    let init_state =
        kagemusha_v2::KagemushaRecursiveSpendStateVectorV5::from_statement_v4(&init_statement)?;
    let append_state =
        kagemusha_v2::KagemushaRecursiveSpendStateVectorV5::from_statement_v4(&append_statement)?;
    let manifest_limbs = kagemusha_exact_u32_public_limbs(manifest_sha256);
    terminal.verify_encoded_pair_binding(
        &init_pair,
        &init_statement,
        &init_operation,
        kagemusha_exact_u32_public_limbs(
            init_statement.digest().map_err(|error| error.to_string())?,
        ),
        &init_state.limbs,
        1,
        manifest_limbs,
    )?;
    terminal.verify_encoded_pair_binding(
        &append_pair,
        &append_statement,
        &append_operation,
        kagemusha_exact_u32_public_limbs(
            append_statement
                .digest()
                .map_err(|error| error.to_string())?,
        ),
        &append_state.limbs,
        2,
        manifest_limbs,
    )?;
    let evidence = KagemushaCandidateRecursiveStepTwoEvidenceV4 {
        candidate_sha256,
        manifest_sha256,
        step_eq_proving_key_framed_sha256: role_digests[0],
        step_eq_proving_key_payload_sha256: role_digests[1],
        step_eq_verifying_key_framed_sha256: role_digests[2],
        step_eq_verifying_key_payload_sha256: role_digests[3],
        step_ep_proving_key_framed_sha256: role_digests[4],
        step_ep_proving_key_payload_sha256: role_digests[5],
        step_ep_verifying_key_framed_sha256: role_digests[6],
        step_ep_verifying_key_payload_sha256: role_digests[7],
        initialization_key_set_sha256: key_set_sha256,
        append_key_set_sha256: key_set_sha256,
        terminal_key_set_sha256: key_set_sha256,
        initialization_pair_sha256: Sha256::digest(&init_pair).into(),
        initialization_bundle_digest,
        append_pair_sha256: Sha256::digest(&append_pair).into(),
        append_bound_parent_bundle_digest: split.inputs[0].bundle_digest,
        initialization_proof_step_count,
        initialization_parent_count,
        append_proof_step_count,
        append_parent_count,
        terminal_verified_pair_count: 2,
    };
    evidence.validate_for_candidate(candidate)?;
    iroha_data_model::offline::KagemushaRecursiveSpendQualificationReceiptV4::new(
        candidate,
        init_pair,
        append_pair,
    )
    .map_err(|error| error.to_string())
}
/// Reauthenticate and terminally verify the exact proof pairs stored in a
/// candidate qualification receipt.
///
/// This verifier never proves again. It authenticates all eight framed
/// candidate roles (including each proving key's embedded verifier key),
/// reconstructs the deterministic initialization and exact one-parent append,
/// derives counters from the stored proof bytes, then obtains fresh terminal
/// decisions for both pairs.
///
/// # Errors
///
/// Returns an error for any candidate, role, receipt, proof, semantic parent,
/// counter, or terminal-verifier mismatch.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn verify_candidate_recursive_step_two_receipt_v4<F>(
    candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    expected_candidate_sha256: [u8; 32],
    expected_manifest_sha256: [u8; 32],
    receipt: &iroha_data_model::offline::KagemushaRecursiveSpendQualificationReceiptV4,
    qualification_memory_contract: &KagemushaQualificationMemoryContractV4<'_>,
    step_eq_proving_key_file: std::fs::File,
    step_ep_proving_key_file: std::fs::File,
    mut load: F,
) -> Result<KagemushaCandidateRecursiveStepTwoEvidenceV4, String>
where
    F: FnMut(
        KagemushaPastaCycleParityV1,
        KagemushaPastaCycleArtifactKindV4,
    )
        -> Result<super::kagemusha_artifact_v4::KagemushaValidatedArtifactPayloadV4, String>,
{
    use super::kagemusha_v2;
    candidate.validate().map_err(|error| error.to_string())?;
    qualification_memory_contract.validate_candidate(candidate)?;
    receipt
        .validate_against_candidate(candidate)
        .map_err(|error| error.to_string())?;
    let candidate_sha256 = candidate.sha256().map_err(|error| error.to_string())?;
    let manifest_sha256: [u8; 32] = Sha256::digest(
        norito::encode_canonical(&candidate.manifest)
            .map_err(|error| format!("failed to encode Kagemusha V4 candidate: {error}"))?,
    )
    .into();
    validate_kagemusha_candidate_spool_identity_v5(
        candidate_sha256,
        manifest_sha256,
        expected_candidate_sha256,
        expected_manifest_sha256,
    )?;
    if receipt.candidate_sha256() != candidate_sha256
        || receipt.manifest_sha256() != manifest_sha256
        || receipt.artifact_role_digests()
            != candidate
                .artifact_role_digests()
                .map_err(|e| e.to_string())?
    {
        return Err("Kagemusha V4 qualification receipt substituted candidate roles".to_owned());
    }
    let role_digests = kagemusha_candidate_step_two_role_digests_v4(&candidate.manifest)?;
    let key_set_sha256 = kagemusha_candidate_step_two_key_set_sha256_v4(
        candidate_sha256,
        manifest_sha256,
        role_digests,
    );
    let terminal_binding = KagemushaArtifactSpoolBindingV5::candidate_evidence_lab(
        candidate,
        expected_candidate_sha256,
        expected_manifest_sha256,
    )?;
    // Receipt verification never proves. Authenticate the two release-sized PK
    // roles with fixed scratch, scan their exact processed-key geometry, and
    // bind each embedded VK prefix to the separately authenticated VK. Full
    // `ProvingKey` materialization remains confined to proving paths.
    let step_eq_profile = candidate
        .manifest
        .profiles
        .first()
        .filter(|profile| profile.parity == KagemushaPastaCycleParityV1::StepEq)
        .ok_or_else(|| "Kagemusha V4 qualification Eq profile is absent".to_owned())?;
    let step_ep_profile = candidate
        .manifest
        .profiles
        .get(1)
        .filter(|profile| profile.parity == KagemushaPastaCycleParityV1::StepEp)
        .ok_or_else(|| "Kagemusha V4 qualification Ep profile is absent".to_owned())?;
    let step_eq_verifying_key = terminal_binding.descriptor(
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    let step_ep_verifying_key = terminal_binding.descriptor(
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    let step_eq_proving_key_spool = KagemushaProvingKeySpoolV5::authenticate(
        step_eq_proving_key_file,
        &terminal_binding,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    authenticate_kagemusha_receipt_pk_spool_v5(
        &step_eq_proving_key_spool,
        &step_eq_profile.circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_verifying_key.payload_size_bytes,
        step_eq_verifying_key.payload_sha256,
    )?;
    drop(step_eq_proving_key_spool);
    let step_ep_proving_key_spool = KagemushaProvingKeySpoolV5::authenticate(
        step_ep_proving_key_file,
        &terminal_binding,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    authenticate_kagemusha_receipt_pk_spool_v5(
        &step_ep_proving_key_spool,
        &step_ep_profile.circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_verifying_key.payload_size_bytes,
        step_ep_verifying_key.payload_sha256,
    )?;
    drop(step_ep_proving_key_spool);
    // Parse each of the remaining six bounded roles once and retain the
    // resulting terminal verifier for both stored proof pairs.
    let terminal = KagemushaPastaCycleTerminalVerifierV4::from_validated_artifact_loader(
        &candidate.manifest,
        |parity, kind| {
            let payload = load(parity, kind)?;
            terminal_binding.validate_payload(&payload, parity, kind)?;
            Ok(payload)
        },
    )?;
    let init_decoded = KagemushaPastaCycleProofPairV4::decode_authenticated(
        receipt.initialization_pair(),
        &terminal.step_eq_circuit_params,
        &terminal.step_ep_circuit_params,
        terminal.max_pair_bytes,
    )?;
    let append_decoded = KagemushaPastaCycleProofPairV4::decode_authenticated(
        receipt.append_pair(),
        &terminal.step_eq_circuit_params,
        &terminal.step_ep_circuit_params,
        terminal.max_pair_bytes,
    )?;
    let initialization_proof_step_count = init_decoded.proof_step_count;
    let initialization_parent_count = init_decoded.public_inputs.parent_count()?;
    let append_proof_step_count = append_decoded.proof_step_count;
    let append_parent_count = append_decoded.public_inputs.parent_count()?;
    drop(init_decoded);
    drop(append_decoded);
    let relation = kagemusha_initialization_relation_v4(
        &candidate.manifest.network_id,
        &candidate.manifest.asset.to_string(),
        candidate.manifest.asset_scale,
    )?;
    let init_statement =
        kagemusha_candidate_qualification_init_statement_v4(candidate, manifest_sha256, &relation)?;
    let init_operation = KagemushaStepOperationVectorV4::from_candidate_qualification_init_v4(
        &init_statement,
        &relation.topup,
        &relation.output_membership,
        KAGEMUSHA_INITIALIZATION_RELATION_PAYER_V4,
    )?;
    let append = kagemusha_candidate_qualification_append_v4(
        candidate,
        manifest_sha256,
        &relation,
        &init_statement,
        &init_operation,
        receipt.initialization_pair(),
    )?;
    if append.bound_parent_bundle_digest != append.initialization_bundle_digest {
        return Err(
            "Kagemusha V4 qualification append is not the exact child of initialization".to_owned(),
        );
    }
    let init_state =
        kagemusha_v2::KagemushaRecursiveSpendStateVectorV5::from_statement_v4(&init_statement)?;
    let append_state =
        kagemusha_v2::KagemushaRecursiveSpendStateVectorV5::from_statement_v4(&append.statement)?;
    let manifest_limbs = kagemusha_exact_u32_public_limbs(manifest_sha256);
    terminal.verify_encoded_pair_binding(
        receipt.initialization_pair(),
        &init_statement,
        &init_operation,
        kagemusha_exact_u32_public_limbs(
            init_statement.digest().map_err(|error| error.to_string())?,
        ),
        &init_state.limbs,
        1,
        manifest_limbs,
    )?;
    terminal.verify_encoded_pair_binding(
        receipt.append_pair(),
        &append.statement,
        &append.operation,
        kagemusha_exact_u32_public_limbs(
            append
                .statement
                .digest()
                .map_err(|error| error.to_string())?,
        ),
        &append_state.limbs,
        2,
        manifest_limbs,
    )?;
    let evidence = KagemushaCandidateRecursiveStepTwoEvidenceV4 {
        candidate_sha256,
        manifest_sha256,
        step_eq_proving_key_framed_sha256: role_digests[0],
        step_eq_proving_key_payload_sha256: role_digests[1],
        step_eq_verifying_key_framed_sha256: role_digests[2],
        step_eq_verifying_key_payload_sha256: role_digests[3],
        step_ep_proving_key_framed_sha256: role_digests[4],
        step_ep_proving_key_payload_sha256: role_digests[5],
        step_ep_verifying_key_framed_sha256: role_digests[6],
        step_ep_verifying_key_payload_sha256: role_digests[7],
        initialization_key_set_sha256: key_set_sha256,
        append_key_set_sha256: key_set_sha256,
        terminal_key_set_sha256: key_set_sha256,
        initialization_pair_sha256: Sha256::digest(receipt.initialization_pair()).into(),
        initialization_bundle_digest: append.initialization_bundle_digest,
        append_pair_sha256: Sha256::digest(receipt.append_pair()).into(),
        append_bound_parent_bundle_digest: append.bound_parent_bundle_digest,
        initialization_proof_step_count,
        initialization_parent_count,
        append_proof_step_count,
        append_parent_count,
        terminal_verified_pair_count: 2,
    };
    evidence.validate_for_candidate(candidate)?;
    Ok(evidence)
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
    let circuit = KagemushaStepEqProtocolBootstrapCircuitV5 {
        params: circuit_params.clone(),
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
    let circuit = KagemushaStepEpProtocolBootstrapCircuitV5 {
        params: circuit_params.clone(),
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
#[cfg(feature = "kagemusha-generation-memory-lab")]
struct KagemushaK17ShapeProbeScopeV5;
#[cfg(feature = "kagemusha-generation-memory-lab")]
impl KagemushaK17ShapeProbeScopeV5 {
    fn enter() -> Result<Self, String> {
        let already_active =
            KAGEMUSHA_K17_SHAPE_PROBE_ACTIVE_V5.with(|active| active.replace(true));
        if already_active {
            return Err("Kagemusha k17 shape probe cannot be nested".to_owned());
        }
        KAGEMUSHA_K17_SHAPE_PROBE_REQUIRED_V5.with(|captured| captured.borrow_mut().clear());
        Ok(Self)
    }
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
impl Drop for KagemushaK17ShapeProbeScopeV5 {
    fn drop(&mut self) {
        KAGEMUSHA_K17_SHAPE_PROBE_REQUIRED_V5.with(|captured| captured.borrow_mut().clear());
        KAGEMUSHA_K17_SHAPE_PROBE_ACTIVE_V5.with(|active| active.set(false));
    }
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_shape_probe_params_v5(
    advice_columns: u32,
    lookup_columns: u32,
) -> KagemushaStepCircuitParamsV4 {
    let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
    let public_input_limbs = KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)
        .expect("production k17 public layout")
        .instance_column_limbs;
    KagemushaStepCircuitParamsV4 {
        version: iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
        k,
        num_advice_per_phase: vec![advice_columns],
        num_lookup_advice_per_phase: vec![lookup_columns, 0, 0],
        num_fixed: 1,
        lookup_bits: k - 1,
        num_instance_columns: 1,
        public_input_limbs,
        minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
        max_parent_proof_bytes: KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4,
    }
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_dummy_parent_proof_v5<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    protocol: &PlonkProtocol<C>,
    seed: u64,
) -> Result<Vec<u8>, String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField + From<u64>,
{
    use halo2_proofs::poly::commitment::ParamsProver as _;
    let generators = params.get_g();
    if generators.is_empty() {
        return Err("Kagemusha k17 shape probe ParamsIPA has no generators".to_owned());
    }
    let mut point_index = usize::try_from(seed)
        .map_err(|_| "Kagemusha k17 dummy point seed does not fit usize".to_owned())?;
    let mut scalar_index = seed;
    let mut proof = Vec::new();
    let mut push_point = |proof: &mut Vec<u8>| {
        let point = &generators[point_index % generators.len()];
        proof.extend_from_slice(point.to_bytes().as_ref());
        point_index += 1;
    };
    let mut push_scalar = |proof: &mut Vec<u8>| {
        let scalar = C::ScalarExt::from(scalar_index.max(1));
        proof.extend_from_slice(scalar.to_repr().as_ref());
        scalar_index = scalar_index.saturating_add(1);
    };
    for _ in 0..protocol.num_witness.iter().sum::<usize>() {
        push_point(&mut proof);
    }
    for _ in 0..protocol.quotient.num_chunk() {
        push_point(&mut proof);
    }
    for _ in 0..protocol.evaluations.len() {
        push_scalar(&mut proof);
    }
    let mut rotations_by_polynomial =
        std::collections::BTreeMap::<usize, std::collections::BTreeSet<i32>>::new();
    for query in &protocol.queries {
        rotations_by_polynomial
            .entry(query.poly)
            .or_default()
            .insert(query.rotation.0);
    }
    let query_sets = rotations_by_polynomial
        .into_values()
        .map(|rotations| rotations.into_iter().collect::<Vec<_>>())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    // BGH19 multi-open: f, one evaluation per distinct polynomial rotation
    // set, the ZK commitment, k pairs of IPA round points, c, the blind, and u.
    push_point(&mut proof);
    for _ in 0..query_sets {
        push_scalar(&mut proof);
    }
    push_point(&mut proof);
    for _ in 0..protocol.domain.k {
        push_point(&mut proof);
        push_point(&mut proof);
    }
    push_scalar(&mut proof);
    push_scalar(&mut proof);
    // This final BGH19 `g` is the augmented folded-generator suffix: Halo2's
    // ordinary transcript omits it and Kagemusha appends exactly this point.
    push_point(&mut proof);
    Ok(proof)
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_dummy_accumulator_v5<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    seed: u64,
) -> Result<
    snark_verifier::pcs::ipa::IpaAccumulator<C, snark_verifier::loader::native::NativeLoader>,
    String,
>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField + From<u64>,
{
    use halo2_proofs::poly::commitment::{Params as _, ParamsProver as _};
    let round_count = usize::try_from(params.k())
        .map_err(|_| "Kagemusha k17 accumulator degree does not fit usize".to_owned())?;
    let generators = params.get_g();
    let generator_index = usize::try_from(seed)
        .map_err(|_| "Kagemusha k17 accumulator seed does not fit usize".to_owned())?
        % generators.len();
    let xi = (0..round_count)
        .map(|round| {
            C::ScalarExt::from(
                seed.saturating_add(u64::try_from(round).unwrap_or(u64::MAX))
                    .saturating_add(1),
            )
        })
        .collect();
    Ok(snark_verifier::pcs::ipa::IpaAccumulator::new(
        xi,
        generators[generator_index],
    ))
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_eq_probe_seed_v5(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    expected_proof_bytes: u32,
) -> Result<KagemushaEqBootstrapSeedV4, String> {
    use halo2_proofs::plonk::keygen_vk_custom;
    let public_len = usize::try_from(
        validate_kagemusha_circuit_params_v4(circuit_params)?.instance_column_limbs,
    )
    .map_err(|_| "Kagemusha k17 Eq public length does not fit usize".to_owned())?;
    let circuit = KagemushaStepEqProtocolBootstrapCircuitV5 {
        params: circuit_params.clone(),
    };
    let verifying_key = keygen_vk_custom(params, &circuit, false)
        .map_err(|error| format!("Kagemusha k17 Eq VK-only probe failed: {error}"))?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    drop(verifying_key);
    let proof = kagemusha_k17_dummy_parent_proof_v5(params, &protocol, 11)?;
    if proof.len() != usize::try_from(expected_proof_bytes).unwrap_or(0) {
        return Err(format!(
            "Kagemusha k17 Eq dummy transcript measured {} bytes instead of configured {expected_proof_bytes}",
            proof.len()
        ));
    }
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
        current: kagemusha_k17_dummy_accumulator_v5(params, 101)?,
    })
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_ep_probe_seed_v5(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    expected_proof_bytes: u32,
) -> Result<KagemushaEpBootstrapSeedV4, String> {
    use halo2_proofs::plonk::keygen_vk_custom;
    let public_len = usize::try_from(
        validate_kagemusha_circuit_params_v4(circuit_params)?.instance_column_limbs,
    )
    .map_err(|_| "Kagemusha k17 Ep public length does not fit usize".to_owned())?;
    let circuit = KagemushaStepEpProtocolBootstrapCircuitV5 {
        params: circuit_params.clone(),
    };
    let verifying_key = keygen_vk_custom(params, &circuit, false)
        .map_err(|error| format!("Kagemusha k17 Ep VK-only probe failed: {error}"))?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    drop(verifying_key);
    let proof = kagemusha_k17_dummy_parent_proof_v5(params, &protocol, 17)?;
    if proof.len() != usize::try_from(expected_proof_bytes).unwrap_or(0) {
        return Err(format!(
            "Kagemusha k17 Ep dummy transcript measured {} bytes instead of configured {expected_proof_bytes}",
            proof.len()
        ));
    }
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
        current: kagemusha_k17_dummy_accumulator_v5(params, 211)?,
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
        circuit_params_sha256: kagemusha_circuit_params_sha256_v4(circuit_params)?,
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
        circuit_params_sha256: kagemusha_circuit_params_sha256_v4(circuit_params)?,
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
