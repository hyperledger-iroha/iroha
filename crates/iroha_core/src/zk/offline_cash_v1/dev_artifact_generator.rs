//! Developer-only generation of a complete Offline Cash V1 artifact candidate.
//!
//! This module produces transparent IPA parameters plus processed Halo2 proving
//! and verifying keys. It deliberately has no release-authority key, signature,
//! qualification-receipt, or promotion surface. The caller receives unlinked,
//! owner-private spools and must explicitly publish them as an unauthenticated
//! candidate before any separate release corridor can review and sign it.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read as _, Seek as _, SeekFrom, Write},
    panic::{AssertUnwindSafe, catch_unwind},
};

use ff::FromUniformBytes;
use halo2_base::utils::CurveAffineExt;
use halo2_proofs::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{Circuit, ProvingKey, VerifyingKey, keygen_pk, keygen_vk},
    poly::{commitment::Params as _, ipa::commitment::ParamsIPA},
};
use iroha_data_model::offline::{
    KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1,
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_P256_V3_HALO2_K_V1,
    OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
    OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1, OfflineCashRecursivePairBindingV1,
    offline_cash_artifact_set_digest_v1,
};
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    loader::native::NativeLoader, pcs::ipa::IpaAccumulator, verifier::plonk::PlonkProtocol,
};
use zeroize::Zeroizing;

use super::{
    guard_bundle_recursion::{
        OfflineCashGuardBundleChildProofV1, OfflineCashGuardBundleChildSlotV1,
        OfflineCashGuardBundleParityRecursionV1, OfflineCashGuardBundleRecursivePublicV1,
        build_guard_bundle_keygen_pair_v1, build_guard_bundle_prover_pair_v1,
        offline_cash_guard_bundle_keygen_audit_digests_v1,
    },
    helper_abi::{OfflineCashHelperOperationV1, OfflineCashHelperPublicInstancesV1},
    helper_circuit::{
        OfflineCashEpAndroidKeyCertBindingCircuitV1, OfflineCashEpGuardBundleLeafBindingCircuitV1,
        OfflineCashEpGuardUseBindingCircuitV1, OfflineCashEpPlatformBindBindingCircuitV1,
        OfflineCashEqAndroidKeyCertBindingCircuitV1, OfflineCashEqGuardBundleLeafBindingCircuitV1,
        OfflineCashEqGuardUseBindingCircuitV1, OfflineCashEqPlatformBindBindingCircuitV1,
    },
    helper_recursion::{
        offline_cash_lineage_from_ep_v1, offline_cash_lineage_from_eq_v1,
        terminal_verify_ep_outer_and_carried_v1, terminal_verify_eq_outer_and_carried_v1,
    },
    helper_relation::{
        OfflineCashHelperRelationInputV1, OfflineCashValidatedHelperRelationV1, guard_bindings_v1,
        platform_message_v1,
    },
    p256_packed_affine_ep_child_from_source_v3, p256_packed_affine_eq_child_from_source_v3,
    protocol::{
        OfflineCashHalo2CircuitRoleV1, OfflineCashHalo2ParityV1,
        offline_cash_artifact_length_bounds_v1, offline_cash_artifact_protocol_v1,
        offline_cash_halo2_profile_digest_v1, offline_cash_halo2_protocol_identity_v1,
        offline_cash_internal_child_proof_max_bytes_v1,
    },
    state_abi::{
        OfflineCashStateLeafPublicInstancesV1, OfflineCashStateOperationV1,
        OfflineCashStatePublicInstancesV1,
    },
    state_circuit::{OfflineCashEpStateLeafCircuitV1, OfflineCashEqStateLeafCircuitV1},
    state_recursion::{
        OfflineCashStateChildProofV1, OfflineCashStateChildSlotV1,
        OfflineCashStateParityRecursionV1, OfflineCashStateRecursivePublicV1,
        build_state_keygen_pair_v1, build_state_prover_pair_v1,
        offline_cash_state_keygen_audit_digests_v1,
    },
    state_relation::{
        OfflineCashStatePrivateWitnessV1, offline_cash_balance_head_v1,
        offline_cash_credit_head_v1, offline_cash_receive_opening_v1,
        offline_cash_receive_transition_digest_v1, offline_cash_state_lineage_digest_v1,
    },
    state_transition::ReceiveFoldOutputV1,
};

use crate::zk::kagemusha_recursion_adapter::{
    compile_poseidon_direct_instance_protocol_v1, create_poseidon_accumulator_fold_proof_v1,
    create_poseidon_direct_instance_proof_v1, poseidon_ipa_succinct_vk_v1,
    verify_poseidon_child_proof_native_v1,
};

const FINAL_STATE_EXPECTED_ORDINARY_PROOF_BYTES_V1: usize = 3_072;
const GUARD_BUNDLE_EXPECTED_ORDINARY_PROOF_BYTES_V1: usize = 3_264;
const P256_V3_EXPECTED_ORDINARY_PROOF_BYTES_V1: usize = 4_544;

/// Stable file name for one role in the complete candidate directory.
///
/// The exhaustive match intentionally fails compilation when the canonical role
/// inventory grows without a corresponding on-disk name.
#[must_use]
pub const fn offline_cash_artifact_file_name_v1(role: OfflineCashArtifactRoleV1) -> &'static str {
    use OfflineCashArtifactRoleV1 as Role;
    match role {
        Role::ParamsEq => "params_eq.bin",
        Role::ParamsEp => "params_ep.bin",
        Role::StatePkEq => "state_pk_eq.bin",
        Role::StateVkEq => "state_vk_eq.bin",
        Role::StatePkEp => "state_pk_ep.bin",
        Role::StateVkEp => "state_vk_ep.bin",
        Role::GuardUsePkEq => "guard_use_pk_eq.bin",
        Role::GuardUseVkEq => "guard_use_vk_eq.bin",
        Role::GuardUsePkEp => "guard_use_pk_ep.bin",
        Role::GuardUseVkEp => "guard_use_vk_ep.bin",
        Role::PlatformBindPkEq => "platform_bind_pk_eq.bin",
        Role::PlatformBindVkEq => "platform_bind_vk_eq.bin",
        Role::PlatformBindPkEp => "platform_bind_pk_ep.bin",
        Role::PlatformBindVkEp => "platform_bind_vk_ep.bin",
        Role::AndroidKeyCertPkEq => "android_key_cert_pk_eq.bin",
        Role::AndroidKeyCertVkEq => "android_key_cert_vk_eq.bin",
        Role::AndroidKeyCertPkEp => "android_key_cert_pk_ep.bin",
        Role::AndroidKeyCertVkEp => "android_key_cert_vk_ep.bin",
        Role::GuardBundlePkEq => "guard_bundle_pk_eq.bin",
        Role::GuardBundleVkEq => "guard_bundle_vk_eq.bin",
        Role::GuardBundlePkEp => "guard_bundle_pk_ep.bin",
        Role::GuardBundleVkEp => "guard_bundle_vk_ep.bin",
        Role::P256V3PkEq => "p256_v3_pk_eq.bin",
        Role::P256V3VkEq => "p256_v3_vk_eq.bin",
        Role::P256V3PkEp => "p256_v3_pk_ep.bin",
        Role::P256V3VkEp => "p256_v3_vk_ep.bin",
        Role::StateLeafPkEq => "state_leaf_pk_eq.bin",
        Role::StateLeafVkEq => "state_leaf_vk_eq.bin",
        Role::StateLeafPkEp => "state_leaf_pk_ep.bin",
        Role::StateLeafVkEp => "state_leaf_vk_ep.bin",
        Role::GuardBundleLeafPkEq => "guard_bundle_leaf_pk_eq.bin",
        Role::GuardBundleLeafVkEq => "guard_bundle_leaf_vk_eq.bin",
        Role::GuardBundleLeafPkEp => "guard_bundle_leaf_pk_ep.bin",
        Role::GuardBundleLeafVkEp => "guard_bundle_leaf_vk_ep.bin",
    }
}

/// Compiled profile digest to bind into an unsigned candidate record.
#[must_use]
pub fn offline_cash_artifact_profile_digest_v1() -> [u8; 32] {
    offline_cash_halo2_profile_digest_v1()
}

/// Compiled protocol digest for a key role, or `None` for transparent params.
#[must_use]
pub fn offline_cash_artifact_protocol_digest_v1(
    role: OfflineCashArtifactRoleV1,
) -> Option<[u8; 32]> {
    offline_cash_artifact_protocol_v1(role)
        .map(|(parity, circuit)| offline_cash_halo2_protocol_identity_v1(parity, circuit).digest())
}

/// Owner-private, seekable output of one generated artifact.
///
/// The backing file is unlinked. Copying re-hashes the complete payload and
/// rejects truncation, extension, or mutation before returning.
#[must_use]
pub struct OfflineCashGeneratedArtifactSpoolV1 {
    role: OfflineCashArtifactRoleV1,
    file: File,
    byte_len: u64,
    sha256: [u8; 32],
}

impl core::fmt::Debug for OfflineCashGeneratedArtifactSpoolV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashGeneratedArtifactSpoolV1")
            .field("role", &self.role)
            .field("byte_len", &self.byte_len)
            .field("sha256", &hex::encode(self.sha256))
            .finish_non_exhaustive()
    }
}

impl OfflineCashGeneratedArtifactSpoolV1 {
    /// Canonical artifact role.
    #[must_use]
    pub const fn role(&self) -> OfflineCashArtifactRoleV1 {
        self.role
    }

    /// Exact binding of the generated payload.
    #[must_use]
    pub const fn binding(&self) -> OfflineCashArtifactBindingV1 {
        OfflineCashArtifactBindingV1 {
            role: self.role,
            sha256: self.sha256,
            byte_len: self.byte_len,
        }
    }

    /// Copy the complete payload to a caller-owned staging sink.
    pub fn copy_to(&mut self, writer: &mut dyn Write) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind {:?} spool: {error}", self.role))?;
        let mut remaining = self.byte_len;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        while remaining != 0 {
            let requested = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("bounded artifact chunk fits usize");
            let read = self
                .file
                .read(&mut buffer[..requested])
                .map_err(|error| format!("failed to read {:?} spool: {error}", self.role))?;
            if read == 0 {
                return Err(format!("generated {:?} spool is truncated", self.role));
            }
            writer
                .write_all(&buffer[..read])
                .map_err(|error| format!("failed to copy {:?} spool: {error}", self.role))?;
            hasher.update(&buffer[..read]);
            remaining -= u64::try_from(read).expect("read count fits u64");
        }
        let mut trailing = [0_u8; 1];
        if self
            .file
            .read(&mut trailing)
            .map_err(|error| format!("failed to finish {:?} spool: {error}", self.role))?
            != 0
        {
            return Err(format!(
                "generated {:?} spool has trailing bytes",
                self.role
            ));
        }
        let actual: [u8; 32] = hasher.finalize().into();
        if actual != self.sha256 {
            return Err(format!("generated {:?} spool digest changed", self.role));
        }
        Ok(())
    }
}

/// Complete generated set, retained in exact [`OfflineCashArtifactRoleV1::ALL`]
/// order and not authenticated for release.
#[must_use]
pub struct OfflineCashGeneratedArtifactSetV1 {
    artifacts: Vec<OfflineCashGeneratedArtifactSpoolV1>,
    bindings: Vec<OfflineCashArtifactBindingV1>,
    artifact_set_digest: [u8; 32],
}

impl core::fmt::Debug for OfflineCashGeneratedArtifactSetV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashGeneratedArtifactSetV1")
            .field("artifact_count", &self.artifacts.len())
            .field(
                "artifact_set_digest",
                &hex::encode(self.artifact_set_digest),
            )
            .field("authenticated_release", &false)
            .finish()
    }
}

impl OfflineCashGeneratedArtifactSetV1 {
    fn from_map(
        mut artifacts: BTreeMap<OfflineCashArtifactRoleV1, OfflineCashGeneratedArtifactSpoolV1>,
    ) -> Result<Self, String> {
        if artifacts.len() != OfflineCashArtifactRoleV1::ALL.len() {
            return Err(format!(
                "offline-cash generator produced {} roles, expected {}",
                artifacts.len(),
                OfflineCashArtifactRoleV1::ALL.len()
            ));
        }
        let artifacts = OfflineCashArtifactRoleV1::ALL
            .into_iter()
            .map(|role| {
                artifacts
                    .remove(&role)
                    .ok_or_else(|| format!("offline-cash generator omitted {role:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !artifacts.is_empty()
            && !artifacts
                .iter()
                .all(|value| value.binding().sha256 != [0; 32])
        {
            return Err("offline-cash generator emitted a zero artifact digest".to_owned());
        }
        let bindings = artifacts
            .iter()
            .map(|value| value.binding())
            .collect::<Vec<_>>();
        let aggregate_bytes = bindings
            .iter()
            .try_fold(0_u64, |total, binding| total.checked_add(binding.byte_len));
        if aggregate_bytes.is_none_or(|total| total > OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1) {
            return Err("offline-cash generated artifact set exceeds its aggregate cap".to_owned());
        }
        let artifact_set_digest = offline_cash_artifact_set_digest_v1(&bindings)
            .map_err(|error| format!("generated artifact inventory is invalid: {error:?}"))?;
        if !artifacts.is_empty() && artifact_set_digest == [0; 32] {
            return Err("offline-cash generated artifact-set digest is zero".to_owned());
        }
        Ok(Self {
            artifacts,
            bindings,
            artifact_set_digest,
        })
    }

    /// Exact `ALL`-ordered artifact bindings.
    #[must_use]
    pub fn bindings(&self) -> &[OfflineCashArtifactBindingV1] {
        &self.bindings
    }

    /// Domain-separated digest of the complete ordered artifact set.
    #[must_use]
    pub const fn artifact_set_digest(&self) -> [u8; 32] {
        self.artifact_set_digest
    }

    /// Consume the set and emit every spool in exact `ALL` order.
    pub fn emit_all<F>(self, mut emit: F) -> Result<(), String>
    where
        F: FnMut(&mut OfflineCashGeneratedArtifactSpoolV1) -> Result<(), String>,
    {
        for (artifact, expected) in self
            .artifacts
            .into_iter()
            .zip(OfflineCashArtifactRoleV1::ALL)
        {
            let mut artifact = artifact;
            if artifact.role != expected {
                return Err("offline-cash generated artifact order changed".to_owned());
            }
            emit(&mut artifact)?;
        }
        Ok(())
    }
}

struct BoundedArtifactWriterV1 {
    role: OfflineCashArtifactRoleV1,
    file: File,
    minimum: u64,
    maximum: u64,
    written: u64,
    sha256: Sha256,
    first_error: Option<String>,
}

impl BoundedArtifactWriterV1 {
    fn new(role: OfflineCashArtifactRoleV1) -> Result<Self, String> {
        let (minimum, maximum) = offline_cash_artifact_length_bounds_v1(role);
        if minimum == 0 || minimum > maximum {
            return Err(format!("invalid generated {role:?} length policy"));
        }
        Ok(Self {
            role,
            file: tempfile::tempfile()
                .map_err(|error| format!("failed to open owner-private {role:?} spool: {error}"))?,
            minimum,
            maximum,
            written: 0,
            sha256: Sha256::new(),
            first_error: None,
        })
    }

    fn finish(mut self) -> Result<OfflineCashGeneratedArtifactSpoolV1, String> {
        if let Some(error) = self.first_error.take() {
            return Err(error);
        }
        self.file
            .flush()
            .map_err(|error| format!("failed to flush {:?} spool: {error}", self.role))?;
        let metadata = self
            .file
            .metadata()
            .map_err(|error| format!("failed to inspect {:?} spool: {error}", self.role))?;
        if !metadata.is_file()
            || self.written < self.minimum
            || self.written > self.maximum
            || metadata.len() != self.written
        {
            return Err(format!("generated {:?} spool length is invalid", self.role));
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to seal {:?} spool: {error}", self.role))?;
        let sha256: [u8; 32] = self.sha256.finalize().into();
        if sha256 == [0; 32] {
            return Err(format!("generated {:?} spool digest is zero", self.role));
        }
        Ok(OfflineCashGeneratedArtifactSpoolV1 {
            role: self.role,
            file: self.file,
            byte_len: self.written,
            sha256,
        })
    }

    fn remember(&mut self, message: String) {
        if self.first_error.is_none() {
            self.first_error = Some(message);
        }
    }
}

impl Write for BoundedArtifactWriterV1 {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.first_error.is_none() {
            match self
                .written
                .checked_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
            {
                Some(next) if next <= self.maximum => {
                    if let Err(error) = self.file.write_all(bytes) {
                        self.remember(format!(
                            "failed to write owner-private {:?} spool: {error}",
                            self.role
                        ));
                    } else {
                        self.sha256.update(bytes);
                        self.written = next;
                    }
                }
                _ => self.remember(format!(
                    "generated {:?} exceeds its {}-byte cap",
                    self.role, self.maximum
                )),
            }
        }
        // Halo2 processed-key serialization assumes several nested writes are
        // infallible. Preserve the first real error and return it from finish.
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.first_error.is_none()
            && let Err(error) = self.file.flush()
        {
            self.remember(format!(
                "failed to flush owner-private {:?} spool: {error}",
                self.role
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
fn spool_bytes(
    role: OfflineCashArtifactRoleV1,
    bytes: &[u8],
) -> Result<OfflineCashGeneratedArtifactSpoolV1, String> {
    let mut writer = BoundedArtifactWriterV1::new(role)?;
    writer
        .write_all(bytes)
        .expect("bounded artifact writer retains errors until finish");
    writer.finish()
}

fn insert_artifact_v1(
    artifacts: &mut BTreeMap<OfflineCashArtifactRoleV1, OfflineCashGeneratedArtifactSpoolV1>,
    artifact: OfflineCashGeneratedArtifactSpoolV1,
) -> Result<(), String> {
    let role = artifact.role();
    if artifacts.insert(role, artifact).is_some() {
        return Err(format!(
            "offline-cash generator attempted to replace the {role:?} artifact"
        ));
    }
    Ok(())
}

fn spool_params_v1<C>(
    role: OfflineCashArtifactRoleV1,
    params: &ParamsIPA<C>,
) -> Result<OfflineCashGeneratedArtifactSpoolV1, String>
where
    C: CurveAffineExt,
{
    let mut writer = BoundedArtifactWriterV1::new(role)?;
    let encoded = catch_unwind(AssertUnwindSafe(|| params.write(&mut writer)))
        .map_err(|_| format!("transparent {role:?} parameter serialization panicked"))?;
    encoded.map_err(|error| format!("failed to serialize transparent {role:?} params: {error}"))?;
    writer.finish()
}

fn spool_proving_key_v1<C>(
    role: OfflineCashArtifactRoleV1,
    key: &ProvingKey<C>,
) -> Result<OfflineCashGeneratedArtifactSpoolV1, String>
where
    C: CurveAffineExt + SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
{
    let mut writer = BoundedArtifactWriterV1::new(role)?;
    let encoded = catch_unwind(AssertUnwindSafe(|| {
        key.write_streaming(&mut writer, SerdeFormat::Processed)
    }))
    .map_err(|_| format!("processed {role:?} proving-key serialization panicked"))?;
    encoded.map_err(|error| format!("failed to serialize processed {role:?} key: {error}"))?;
    writer.finish()
}

fn spool_verifying_key_v1<C>(
    role: OfflineCashArtifactRoleV1,
    key: &VerifyingKey<C>,
) -> Result<OfflineCashGeneratedArtifactSpoolV1, String>
where
    C: CurveAffineExt + SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
{
    let mut writer = BoundedArtifactWriterV1::new(role)?;
    let encoded = catch_unwind(AssertUnwindSafe(|| {
        key.write(&mut writer, SerdeFormat::Processed)
    }))
    .map_err(|_| format!("processed {role:?} verifier-key serialization panicked"))?;
    encoded.map_err(|error| format!("failed to serialize processed {role:?} key: {error}"))?;
    writer.finish()
}

fn processed_verifying_key_sha256_v1<C>(key: &VerifyingKey<C>) -> Result<[u8; 32], String>
where
    C: CurveAffineExt + SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
{
    struct DigestWriter(Sha256);
    impl Write for DigestWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = DigestWriter(Sha256::new());
    let encoded = catch_unwind(AssertUnwindSafe(|| {
        key.write(&mut writer, SerdeFormat::Processed)
    }))
    .map_err(|_| "processed verifier-key digest serialization panicked".to_owned())?;
    encoded.map_err(|error| format!("failed to hash processed verifier key: {error}"))?;
    Ok(writer.0.finalize().into())
}

fn validate_key_roles_v1(
    parity: OfflineCashHalo2ParityV1,
    circuit_role: OfflineCashHalo2CircuitRoleV1,
    proving_role: OfflineCashArtifactRoleV1,
    verifying_role: OfflineCashArtifactRoleV1,
) -> Result<(), String> {
    if offline_cash_artifact_protocol_v1(proving_role) != Some((parity, circuit_role))
        || offline_cash_artifact_protocol_v1(verifying_role) != Some((parity, circuit_role))
    {
        return Err("offline-cash generated key role/protocol mapping is inconsistent".to_owned());
    }
    Ok(())
}

fn generate_processed_key_pair_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    keygen_circuit: &ConcreteCircuit,
    parity: OfflineCashHalo2ParityV1,
    circuit_role: OfflineCashHalo2CircuitRoleV1,
    proving_role: OfflineCashArtifactRoleV1,
    verifying_role: OfflineCashArtifactRoleV1,
    artifacts: &mut BTreeMap<OfflineCashArtifactRoleV1, OfflineCashGeneratedArtifactSpoolV1>,
) -> Result<(ProvingKey<C>, [u8; 32]), String>
where
    C: CurveAffineExt + SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::ScalarExt>,
{
    validate_key_roles_v1(parity, circuit_role, proving_role, verifying_role)?;
    if params.k() != OFFLINE_CASH_HALO2_K_V1 {
        return Err("offline-cash key generation received a non-common-k16 domain".to_owned());
    }
    let verifying_key = catch_unwind(AssertUnwindSafe(|| keygen_vk(params, keygen_circuit)))
        .map_err(|_| format!("{parity:?} {circuit_role:?} verifier key generation panicked"))?
        .map_err(|error| {
            format!("failed to generate {parity:?} {circuit_role:?} verifier key: {error}")
        })?;
    let proving_key = catch_unwind(AssertUnwindSafe(|| {
        keygen_pk(params, verifying_key, keygen_circuit)
    }))
    .map_err(|_| format!("{parity:?} {circuit_role:?} proving key generation panicked"))?
    .map_err(|error| {
        format!("failed to generate {parity:?} {circuit_role:?} proving key: {error}")
    })?;
    let proving_artifact = spool_proving_key_v1(proving_role, &proving_key)?;
    let verifying_artifact = spool_verifying_key_v1(verifying_role, proving_key.get_vk())?;
    let verifying_sha256 = verifying_artifact.binding().sha256;
    insert_artifact_v1(artifacts, proving_artifact)?;
    insert_artifact_v1(artifacts, verifying_artifact)?;
    Ok((proving_key, verifying_sha256))
}

struct GeneratedChildProofV1<C>
where
    C: CurveAffineExt,
{
    protocol: PlonkProtocol<C>,
    instances: Vec<Vec<C::ScalarExt>>,
    proof: Zeroizing<Vec<u8>>,
    accumulator: IpaAccumulator<C, NativeLoader>,
}

impl<C> GeneratedChildProofV1<C>
where
    C: CurveAffineExt,
{
    fn guard_bundle_child_v1(
        &self,
        slot: OfflineCashGuardBundleChildSlotV1,
    ) -> Result<OfflineCashGuardBundleChildProofV1<C>, String> {
        OfflineCashGuardBundleChildProofV1::new(
            slot,
            self.protocol.clone(),
            self.instances.clone(),
            self.proof.as_slice().to_vec(),
        )
    }

    fn state_child_v1(
        &self,
        slot: OfflineCashStateChildSlotV1,
    ) -> Result<OfflineCashStateChildProofV1<C>, String> {
        OfflineCashStateChildProofV1::new(
            slot,
            self.protocol.clone(),
            self.instances.clone(),
            self.proof.as_slice().to_vec(),
        )
    }
}

fn prove_direct_child_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    proving_key: ProvingKey<C>,
    expected_verifying_sha256: [u8; 32],
    circuit_role: OfflineCashHalo2CircuitRoleV1,
    circuit: ConcreteCircuit,
    instances: Vec<Vec<C::ScalarExt>>,
) -> Result<GeneratedChildProofV1<C>, String>
where
    C: CurveAffineExt + SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::ScalarExt>,
{
    let maximum = offline_cash_internal_child_proof_max_bytes_v1(circuit_role)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("{circuit_role:?} has no governed internal proof slot"))?;
    let protocol = compile_poseidon_direct_instance_protocol_v1(
        params,
        proving_key.get_vk(),
        &instances.iter().map(Vec::len).collect::<Vec<_>>(),
    )?;
    let (proof, verifying_key) = catch_unwind(AssertUnwindSafe(|| {
        create_poseidon_direct_instance_proof_v1(params, proving_key, circuit, &instances)
    }))
    .map_err(|_| format!("{circuit_role:?} direct proof generation panicked"))??;
    let exact_expected = match circuit_role {
        OfflineCashHalo2CircuitRoleV1::P256V3 => {
            if maximum != P256_V3_EXPECTED_ORDINARY_PROOF_BYTES_V1 {
                return Err(format!(
                    "governed P256V3 proof slot mismatch: expected {}, actual {maximum}",
                    P256_V3_EXPECTED_ORDINARY_PROOF_BYTES_V1
                ));
            }
            Some(P256_V3_EXPECTED_ORDINARY_PROOF_BYTES_V1)
        }
        OfflineCashHalo2CircuitRoleV1::GuardBundle => {
            Some(GUARD_BUNDLE_EXPECTED_ORDINARY_PROOF_BYTES_V1)
        }
        _ => None,
    };
    if proof.is_empty() || proof.len() > maximum {
        return Err(format!(
            "generated {circuit_role:?} proof length {} is outside its 1..={maximum} bound",
            proof.len()
        ));
    }
    if let Some(expected) = exact_expected
        && proof.len() != expected
    {
        return Err(format!(
            "generated {circuit_role:?} proof length mismatch: expected {expected}, actual {}",
            proof.len()
        ));
    }
    if processed_verifying_key_sha256_v1(&verifying_key)? != expected_verifying_sha256 {
        return Err(format!(
            "generated {circuit_role:?} proof returned a different verifier identity"
        ));
    }
    let accumulator =
        verify_poseidon_child_proof_native_v1(params, &verifying_key, &instances, &proof, maximum)?;
    Ok(GeneratedChildProofV1 {
        protocol,
        instances,
        proof: Zeroizing::new(proof),
        accumulator,
    })
}

#[allow(clippy::too_many_arguments)]
fn generate_direct_child_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    parity: OfflineCashHalo2ParityV1,
    circuit_role: OfflineCashHalo2CircuitRoleV1,
    proving_role: OfflineCashArtifactRoleV1,
    verifying_role: OfflineCashArtifactRoleV1,
    circuit: ConcreteCircuit,
    instances: Vec<Vec<C::ScalarExt>>,
    artifacts: &mut BTreeMap<OfflineCashArtifactRoleV1, OfflineCashGeneratedArtifactSpoolV1>,
) -> Result<GeneratedChildProofV1<C>, String>
where
    C: CurveAffineExt + SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::ScalarExt>,
{
    let keygen_circuit = circuit.without_witnesses();
    let (proving_key, verifying_sha256) = generate_processed_key_pair_v1(
        params,
        &keygen_circuit,
        parity,
        circuit_role,
        proving_role,
        verifying_role,
        artifacts,
    )?;
    prove_direct_child_v1(
        params,
        proving_key,
        verifying_sha256,
        circuit_role,
        circuit,
        instances,
    )
}

fn create_final_state_proof_v1<C, ConcreteCircuit>(
    params: &ParamsIPA<C>,
    proving_key: ProvingKey<C>,
    expected_verifying_sha256: [u8; 32],
    circuit: ConcreteCircuit,
    instances: &[Vec<C::ScalarExt>],
) -> Result<(Zeroizing<Vec<u8>>, VerifyingKey<C>), String>
where
    C: CurveAffineExt + SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::ScalarExt>,
{
    let target = OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1 / 2;
    if target != FINAL_STATE_EXPECTED_ORDINARY_PROOF_BYTES_V1
        || OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 != 3_200
    {
        return Err("Offline Cash final-State proof target/cap policy is invalid".to_owned());
    }
    let expected_instances = instances.iter().map(Vec::len).collect::<Vec<_>>();
    let protocol = compile_poseidon_direct_instance_protocol_v1(
        params,
        proving_key.get_vk(),
        &expected_instances,
    )?;
    if protocol.domain.k != OFFLINE_CASH_HALO2_K_V1 as usize
        || protocol.num_instance != expected_instances
    {
        return Err("final-State compiled protocol shape is not the governed k16 shape".to_owned());
    }
    let (proof, verifying_key) = catch_unwind(AssertUnwindSafe(|| {
        create_poseidon_direct_instance_proof_v1(params, proving_key, circuit, instances)
    }))
    .map_err(|_| "final-State direct proof generation panicked".to_owned())??;
    if proof.len() != FINAL_STATE_EXPECTED_ORDINARY_PROOF_BYTES_V1 {
        return Err(format!(
            "generated final-State proof length mismatch: expected {}, actual {}",
            FINAL_STATE_EXPECTED_ORDINARY_PROOF_BYTES_V1,
            proof.len()
        ));
    }
    if proof.len() > target || proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 {
        return Err(format!(
            "generated final-State proof length {} exceeds target {target} or hard cap {}",
            proof.len(),
            OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        ));
    }
    if processed_verifying_key_sha256_v1(&verifying_key)? != expected_verifying_sha256 {
        return Err(
            "generated final-State proof returned a different verifier identity".to_owned(),
        );
    }
    Ok((Zeroizing::new(proof), verifying_key))
}

#[derive(Clone, Copy)]
struct DeveloperReceiveFixtureV1 {
    output: ReceiveFoldOutputV1,
    before_amount: u128,
    after_amount: u128,
    before_opening: [u8; 32],
    after_opening: [u8; 32],
    credit_opening: [u8; 32],
    wallet_binding: [u8; 32],
    guard_device_id: [u8; 32],
    hardware_policy_id: [u8; 32],
    guard_sequence: u64,
    lineage_digest: [u8; 32],
    next_lineage_digest: [u8; 32],
    recipient_key_reference: [u8; 32],
}

impl DeveloperReceiveFixtureV1 {
    fn new() -> Result<Self, String> {
        let context_digest = [0x12; 32];
        let request_digest = [0x13; 32];
        let amount = 9_001;
        let before_amount = 10_000;
        let after_amount = 19_001;
        let before_opening = [0x51; 32];
        let credit_opening = [0x53; 32];
        let wallet_binding = [0x54; 32];
        let guard_device_id = [0x55; 32];
        let hardware_policy_id = [0x56; 32];
        let recipient_key_reference = [0x57; 32];
        let guard_sequence = 9;
        let lineage_digest = [0x58; 32];
        let balance_parent = offline_cash_balance_head_v1(
            &context_digest,
            &wallet_binding,
            &guard_device_id,
            &hardware_policy_id,
            guard_sequence,
            &lineage_digest,
            before_amount,
            &before_opening,
        );
        let credit_parent = offline_cash_credit_head_v1(
            &context_digest,
            &request_digest,
            &balance_parent,
            &recipient_key_reference,
            amount,
            &credit_opening,
        );
        let send_transition_digest = [0x17; 32];
        let after_opening = *offline_cash_receive_opening_v1(
            &context_digest,
            &before_opening,
            &credit_opening,
            &request_digest,
            &send_transition_digest,
            amount,
        );
        let next_lineage_digest = offline_cash_state_lineage_digest_v1(
            OfflineCashStateOperationV1::ReceiveFold,
            &context_digest,
            &balance_parent,
            &lineage_digest,
            guard_sequence,
            guard_sequence + 1,
            &request_digest,
            &credit_parent,
            &send_transition_digest,
            amount,
        );
        let next_head = offline_cash_balance_head_v1(
            &context_digest,
            &wallet_binding,
            &guard_device_id,
            &hardware_policy_id,
            guard_sequence + 1,
            &next_lineage_digest,
            after_amount,
            &after_opening,
        );
        let receive_transition_digest = offline_cash_receive_transition_digest_v1(
            &context_digest,
            &balance_parent,
            &credit_parent,
            &request_digest,
            &send_transition_digest,
            amount,
            after_amount,
            &next_head,
        );
        let output = ReceiveFoldOutputV1 {
            release_id: [0x11; 32],
            context_digest,
            request_digest,
            payment_digest: [0x99; 32],
            amount,
            scale: 4,
            balance_parent,
            credit_parent,
            next_head,
            send_transition_digest,
            receive_transition_digest,
        };
        let fixture = Self {
            output,
            before_amount,
            after_amount,
            before_opening,
            after_opening,
            credit_opening,
            wallet_binding,
            guard_device_id,
            hardware_policy_id,
            guard_sequence,
            lineage_digest,
            next_lineage_digest,
            recipient_key_reference,
        };
        // Exercise both independent relation constructors before expensive keygen.
        fixture.state_leaf_v1(OfflineCashHalo2ParityV1::Eq)?;
        fixture.state_leaf_v1(OfflineCashHalo2ParityV1::Ep)?;
        fixture.private_witness_v1()?;
        Ok(fixture)
    }

    fn state_leaf_v1(
        &self,
        parity: OfflineCashHalo2ParityV1,
    ) -> Result<OfflineCashStateLeafPublicInstancesV1, String> {
        OfflineCashStateLeafPublicInstancesV1::receive_fold(&self.output, parity)
            .map_err(|error| format!("failed to build developer StateLeaf public input: {error}"))
    }

    fn private_witness_v1(&self) -> Result<OfflineCashStatePrivateWitnessV1, String> {
        OfflineCashStatePrivateWitnessV1::receive_fold(
            self.before_amount,
            self.after_amount,
            self.before_opening,
            self.after_opening,
            self.credit_opening,
            self.wallet_binding,
            self.guard_device_id,
            self.hardware_policy_id,
            self.guard_sequence,
            self.lineage_digest,
            self.next_lineage_digest,
            self.recipient_key_reference,
        )
        .map_err(|error| format!("failed to build developer StateLeaf witness: {error}"))
    }

    const fn helper_input_v1(&self) -> OfflineCashHelperRelationInputV1 {
        OfflineCashHelperRelationInputV1 {
            operation: OfflineCashHelperOperationV1::ReceiveFold,
            release_id: self.output.release_id,
            context_digest: self.output.context_digest,
            current_head: self.output.balance_parent,
            current_lineage_digest: self.lineage_digest,
            transition_digest: self.output.receive_transition_digest,
            wallet_binding: self.wallet_binding,
            hardware_policy_id: self.hardware_policy_id,
            guard_device_id: self.guard_device_id,
            from_sequence: self.guard_sequence,
            to_sequence: self.guard_sequence + 1,
        }
    }
}

fn developer_device_public_key_v1(key: &SigningKey) -> Result<KagemushaDevicePublicKeyV2, String> {
    KagemushaDevicePublicKeyV2::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .map_err(|error| format!("failed to normalize developer platform key: {error}"))
}

fn developer_device_signature_v1(
    key: &SigningKey,
    message: &[u8],
) -> Result<KagemushaDeviceSignatureV2, String> {
    let signature: P256Signature = key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_ref())
        .map_err(|error| format!("failed to normalize developer platform signature: {error}"))
}

fn developer_helper_relation_v1(
    fixture: &DeveloperReceiveFixtureV1,
) -> Result<OfflineCashValidatedHelperRelationV1, String> {
    let input = fixture.helper_input_v1();
    let (current, next) = guard_bindings_v1(&input);
    let message = platform_message_v1(&input, &current, &next)
        .map_err(|error| format!("failed to build developer platform message: {error}"))?;
    // This fixed calibration witness is private, short-lived, and never emitted.
    // It is not release-authority material and cannot authenticate a candidate.
    let key = SigningKey::from_bytes((&[7_u8; 32]).into())
        .map_err(|error| format!("failed to build developer platform witness key: {error}"))?;
    let public_key = developer_device_public_key_v1(&key)?;
    let signature = developer_device_signature_v1(&key, &message)?;
    drop(key);
    OfflineCashValidatedHelperRelationV1::new(input, public_key, signature, None)
        .map_err(|error| format!("failed to validate developer helper relation: {error}"))
}

fn build_guard_bundle_recursion_v1<C>(
    params: &ParamsIPA<C>,
    guard_use: &GeneratedChildProofV1<C>,
    platform_bind: &GeneratedChildProofV1<C>,
    android_key_cert: &GeneratedChildProofV1<C>,
    guard_bundle_leaf: &GeneratedChildProofV1<C>,
    p256: &GeneratedChildProofV1<C>,
) -> Result<
    (
        OfflineCashGuardBundleParityRecursionV1<C>,
        IpaAccumulator<C, NativeLoader>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::ScalarExt: FromUniformBytes<64>,
{
    // Android absence retains fixed recursion geometry. The gated Android P-256
    // slot therefore carries the exact platform proof instead of inventing an
    // unbound all-zero signature statement.
    let fold_inputs = [
        guard_use.accumulator.clone(),
        platform_bind.accumulator.clone(),
        android_key_cert.accumulator.clone(),
        guard_bundle_leaf.accumulator.clone(),
        p256.accumulator.clone(),
        p256.accumulator.clone(),
    ];
    let (fold_proof, folded) = catch_unwind(AssertUnwindSafe(|| {
        create_poseidon_accumulator_fold_proof_v1(params, &fold_inputs)
    }))
    .map_err(|_| "GuardBundle accumulator-fold generation panicked".to_owned())??;
    let recursion = OfflineCashGuardBundleParityRecursionV1::new(
        poseidon_ipa_succinct_vk_v1(params)?,
        [
            guard_use.guard_bundle_child_v1(OfflineCashGuardBundleChildSlotV1::GuardUse)?,
            platform_bind.guard_bundle_child_v1(OfflineCashGuardBundleChildSlotV1::PlatformBind)?,
            android_key_cert
                .guard_bundle_child_v1(OfflineCashGuardBundleChildSlotV1::AndroidKeyCert)?,
            guard_bundle_leaf
                .guard_bundle_child_v1(OfflineCashGuardBundleChildSlotV1::GuardBundleLeaf)?,
            p256.guard_bundle_child_v1(OfflineCashGuardBundleChildSlotV1::PlatformP256)?,
            p256.guard_bundle_child_v1(OfflineCashGuardBundleChildSlotV1::AndroidP256)?,
        ],
        fold_proof,
    )?;
    Ok((recursion, folded))
}

fn build_state_recursion_v1<C>(
    params: &ParamsIPA<C>,
    parity: OfflineCashHalo2ParityV1,
    state_leaf: &GeneratedChildProofV1<C>,
    guard_bundle: &GeneratedChildProofV1<C>,
    guard_bundle_common: OfflineCashHelperPublicInstancesV1,
    guard_bundle_pair_binding: OfflineCashRecursivePairBindingV1,
    guard_bundle_carried: IpaAccumulator<C, NativeLoader>,
) -> Result<
    (
        OfflineCashStateParityRecursionV1<C>,
        IpaAccumulator<C, NativeLoader>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::ScalarExt: FromUniformBytes<64>,
    <C::ScalarExt as ff::PrimeField>::Repr: AsRef<[u8]>,
    C::Repr: AsRef<[u8]>,
{
    let fold_inputs = [
        state_leaf.accumulator.clone(),
        guard_bundle.accumulator.clone(),
        guard_bundle_carried.clone(),
    ];
    let (fold_proof, folded) = catch_unwind(AssertUnwindSafe(|| {
        create_poseidon_accumulator_fold_proof_v1(params, &fold_inputs)
    }))
    .map_err(|_| "final-State accumulator-fold generation panicked".to_owned())??;
    let recursion = OfflineCashStateParityRecursionV1::new(
        parity,
        poseidon_ipa_succinct_vk_v1(params)?,
        [
            state_leaf.state_child_v1(OfflineCashStateChildSlotV1::StateLeaf)?,
            guard_bundle.state_child_v1(OfflineCashStateChildSlotV1::GuardBundle)?,
        ],
        guard_bundle_common,
        guard_bundle_pair_binding,
        guard_bundle_carried,
        fold_proof,
    )?;
    Ok((recursion, folded))
}

/// Generate all 34 transparent parameters and processed key artifacts.
///
/// This developer facade does not create release-authority keys or signatures,
/// does not accept authority material, and does not emit an authenticated
/// release. Its output still requires the independent qualification/evidence
/// corridor and threshold release attestation.
pub fn generate_offline_cash_artifacts_v1() -> Result<OfflineCashGeneratedArtifactSetV1, String> {
    generate_complete_artifact_map_v1().and_then(OfflineCashGeneratedArtifactSetV1::from_map)
}

fn generate_complete_artifact_map_v1()
-> Result<BTreeMap<OfflineCashArtifactRoleV1, OfflineCashGeneratedArtifactSpoolV1>, String> {
    // The cryptographic generation body follows below. Keeping the publication-
    // independent map private prevents partial role sets from escaping as a
    // successful candidate.
    generate_cryptographic_artifacts_v1()
}

fn generate_cryptographic_artifacts_v1()
-> Result<BTreeMap<OfflineCashArtifactRoleV1, OfflineCashGeneratedArtifactSpoolV1>, String> {
    use OfflineCashArtifactRoleV1 as Artifact;
    use OfflineCashHalo2CircuitRoleV1 as CircuitRole;
    use OfflineCashHalo2ParityV1 as Parity;

    if OFFLINE_CASH_HALO2_K_V1 != 16 || OFFLINE_CASH_P256_V3_HALO2_K_V1 != OFFLINE_CASH_HALO2_K_V1 {
        return Err(
            "Offline Cash V1 artifact generation requires the governed common k16 domain"
                .to_owned(),
        );
    }

    let eq_params = catch_unwind(AssertUnwindSafe(|| {
        ParamsIPA::<EqAffine>::new(OFFLINE_CASH_HALO2_K_V1)
    }))
    .map_err(|_| "Eq transparent parameter generation panicked".to_owned())?;
    let ep_params = catch_unwind(AssertUnwindSafe(|| {
        ParamsIPA::<EpAffine>::new(OFFLINE_CASH_HALO2_K_V1)
    }))
    .map_err(|_| "Ep transparent parameter generation panicked".to_owned())?;
    let mut artifacts = BTreeMap::new();
    insert_artifact_v1(
        &mut artifacts,
        spool_params_v1(Artifact::ParamsEq, &eq_params)?,
    )?;
    insert_artifact_v1(
        &mut artifacts,
        spool_params_v1(Artifact::ParamsEp, &ep_params)?,
    )?;

    let fixture = DeveloperReceiveFixtureV1::new()?;
    let relation = developer_helper_relation_v1(&fixture)?;

    let eq_state_leaf_public = fixture.state_leaf_v1(Parity::Eq)?;
    let eq_state_leaf_circuit = OfflineCashEqStateLeafCircuitV1::new(
        eq_state_leaf_public.clone(),
        fixture.private_witness_v1()?,
    )
    .map_err(|error| format!("failed to construct Eq StateLeaf circuit: {error}"))?;
    let eq_state_leaf = generate_direct_child_v1(
        &eq_params,
        Parity::Eq,
        CircuitRole::StateLeaf,
        Artifact::StateLeafPkEq,
        Artifact::StateLeafVkEq,
        eq_state_leaf_circuit,
        vec![eq_state_leaf_public.field_instances::<Fp>().to_vec()],
        &mut artifacts,
    )?;

    let ep_state_leaf_public = fixture.state_leaf_v1(Parity::Ep)?;
    let ep_state_leaf_circuit = OfflineCashEpStateLeafCircuitV1::new(
        ep_state_leaf_public.clone(),
        fixture.private_witness_v1()?,
    )
    .map_err(|error| format!("failed to construct Ep StateLeaf circuit: {error}"))?;
    let ep_state_leaf = generate_direct_child_v1(
        &ep_params,
        Parity::Ep,
        CircuitRole::StateLeaf,
        Artifact::StateLeafPkEp,
        Artifact::StateLeafVkEp,
        ep_state_leaf_circuit,
        vec![ep_state_leaf_public.field_instances::<Fq>().to_vec()],
        &mut artifacts,
    )?;

    let eq_guard_use_circuit = OfflineCashEqGuardUseBindingCircuitV1::new(&relation)
        .map_err(|error| format!("failed to construct Eq GuardUse circuit: {error}"))?;
    let eq_guard_use_instances = eq_guard_use_circuit.public_instance_columns();
    let eq_guard_use = generate_direct_child_v1(
        &eq_params,
        Parity::Eq,
        CircuitRole::GuardUse,
        Artifact::GuardUsePkEq,
        Artifact::GuardUseVkEq,
        eq_guard_use_circuit,
        eq_guard_use_instances,
        &mut artifacts,
    )?;

    let ep_guard_use_circuit = OfflineCashEpGuardUseBindingCircuitV1::new(&relation)
        .map_err(|error| format!("failed to construct Ep GuardUse circuit: {error}"))?;
    let ep_guard_use_instances = ep_guard_use_circuit.public_instance_columns();
    let ep_guard_use = generate_direct_child_v1(
        &ep_params,
        Parity::Ep,
        CircuitRole::GuardUse,
        Artifact::GuardUsePkEp,
        Artifact::GuardUseVkEp,
        ep_guard_use_circuit,
        ep_guard_use_instances,
        &mut artifacts,
    )?;

    let eq_platform_bind_circuit = OfflineCashEqPlatformBindBindingCircuitV1::new(&relation)
        .map_err(|error| format!("failed to construct Eq PlatformBind circuit: {error}"))?;
    let eq_platform_bind_instances = eq_platform_bind_circuit.public_instance_columns();
    let eq_platform_bind = generate_direct_child_v1(
        &eq_params,
        Parity::Eq,
        CircuitRole::PlatformBind,
        Artifact::PlatformBindPkEq,
        Artifact::PlatformBindVkEq,
        eq_platform_bind_circuit,
        eq_platform_bind_instances,
        &mut artifacts,
    )?;

    let ep_platform_bind_circuit = OfflineCashEpPlatformBindBindingCircuitV1::new(&relation)
        .map_err(|error| format!("failed to construct Ep PlatformBind circuit: {error}"))?;
    let ep_platform_bind_instances = ep_platform_bind_circuit.public_instance_columns();
    let ep_platform_bind = generate_direct_child_v1(
        &ep_params,
        Parity::Ep,
        CircuitRole::PlatformBind,
        Artifact::PlatformBindPkEp,
        Artifact::PlatformBindVkEp,
        ep_platform_bind_circuit,
        ep_platform_bind_instances,
        &mut artifacts,
    )?;

    let eq_android_key_cert_circuit =
        OfflineCashEqAndroidKeyCertBindingCircuitV1::new(&relation)
            .map_err(|error| format!("failed to construct Eq AndroidKeyCert circuit: {error}"))?;
    let eq_android_key_cert_instances = eq_android_key_cert_circuit.public_instance_columns();
    let eq_android_key_cert = generate_direct_child_v1(
        &eq_params,
        Parity::Eq,
        CircuitRole::AndroidKeyCert,
        Artifact::AndroidKeyCertPkEq,
        Artifact::AndroidKeyCertVkEq,
        eq_android_key_cert_circuit,
        eq_android_key_cert_instances,
        &mut artifacts,
    )?;

    let ep_android_key_cert_circuit =
        OfflineCashEpAndroidKeyCertBindingCircuitV1::new(&relation)
            .map_err(|error| format!("failed to construct Ep AndroidKeyCert circuit: {error}"))?;
    let ep_android_key_cert_instances = ep_android_key_cert_circuit.public_instance_columns();
    let ep_android_key_cert = generate_direct_child_v1(
        &ep_params,
        Parity::Ep,
        CircuitRole::AndroidKeyCert,
        Artifact::AndroidKeyCertPkEp,
        Artifact::AndroidKeyCertVkEp,
        ep_android_key_cert_circuit,
        ep_android_key_cert_instances,
        &mut artifacts,
    )?;

    let eq_guard_bundle_leaf_circuit = OfflineCashEqGuardBundleLeafBindingCircuitV1::new(&relation)
        .map_err(|error| format!("failed to construct Eq GuardBundleLeaf circuit: {error}"))?;
    let eq_guard_bundle_leaf_instances = eq_guard_bundle_leaf_circuit.public_instance_columns();
    let eq_guard_bundle_leaf = generate_direct_child_v1(
        &eq_params,
        Parity::Eq,
        CircuitRole::GuardBundleLeaf,
        Artifact::GuardBundleLeafPkEq,
        Artifact::GuardBundleLeafVkEq,
        eq_guard_bundle_leaf_circuit,
        eq_guard_bundle_leaf_instances,
        &mut artifacts,
    )?;

    let ep_guard_bundle_leaf_circuit = OfflineCashEpGuardBundleLeafBindingCircuitV1::new(&relation)
        .map_err(|error| format!("failed to construct Ep GuardBundleLeaf circuit: {error}"))?;
    let ep_guard_bundle_leaf_instances = ep_guard_bundle_leaf_circuit.public_instance_columns();
    let ep_guard_bundle_leaf = generate_direct_child_v1(
        &ep_params,
        Parity::Ep,
        CircuitRole::GuardBundleLeaf,
        Artifact::GuardBundleLeafPkEp,
        Artifact::GuardBundleLeafVkEp,
        ep_guard_bundle_leaf_circuit,
        ep_guard_bundle_leaf_instances,
        &mut artifacts,
    )?;

    let eq_p256_circuit =
        p256_packed_affine_eq_child_from_source_v3(relation.platform_p256_child_statement_v3())
            .map_err(|error| format!("failed to construct Eq P256V3 circuit: {error}"))?;
    let eq_p256_instances = vec![
        eq_p256_circuit
            .instances()
            .map_err(|error| format!("failed to derive Eq P256V3 instances: {error}"))?,
    ];
    let eq_p256 = generate_direct_child_v1(
        &eq_params,
        Parity::Eq,
        CircuitRole::P256V3,
        Artifact::P256V3PkEq,
        Artifact::P256V3VkEq,
        eq_p256_circuit,
        eq_p256_instances,
        &mut artifacts,
    )?;

    let ep_p256_circuit =
        p256_packed_affine_ep_child_from_source_v3(relation.platform_p256_child_statement_v3())
            .map_err(|error| format!("failed to construct Ep P256V3 circuit: {error}"))?;
    let ep_p256_instances = vec![
        ep_p256_circuit
            .instances()
            .map_err(|error| format!("failed to derive Ep P256V3 instances: {error}"))?,
    ];
    let ep_p256 = generate_direct_child_v1(
        &ep_params,
        Parity::Ep,
        CircuitRole::P256V3,
        Artifact::P256V3PkEp,
        Artifact::P256V3VkEp,
        ep_p256_circuit,
        ep_p256_instances,
        &mut artifacts,
    )?;

    let (eq_guard_recursion, eq_guard_carried) = build_guard_bundle_recursion_v1(
        &eq_params,
        &eq_guard_use,
        &eq_platform_bind,
        &eq_android_key_cert,
        &eq_guard_bundle_leaf,
        &eq_p256,
    )?;
    let (ep_guard_recursion, ep_guard_carried) = build_guard_bundle_recursion_v1(
        &ep_params,
        &ep_guard_use,
        &ep_platform_bind,
        &ep_android_key_cert,
        &ep_guard_bundle_leaf,
        &ep_p256,
    )?;
    let eq_guard_common = relation
        .public_instances(Parity::Eq, CircuitRole::GuardBundle)
        .map_err(|error| format!("failed to build Eq GuardBundle public input: {error}"))?;
    let ep_guard_common = relation
        .public_instances(Parity::Ep, CircuitRole::GuardBundle)
        .map_err(|error| format!("failed to build Ep GuardBundle public input: {error}"))?;
    let eq_guard_lineage = offline_cash_lineage_from_eq_v1(&eq_guard_carried)
        .map_err(|error| format!("failed to encode Eq GuardBundle lineage: {error:?}"))?;
    let ep_guard_lineage = offline_cash_lineage_from_ep_v1(&ep_guard_carried)
        .map_err(|error| format!("failed to encode Ep GuardBundle lineage: {error:?}"))?;

    let provisional_guard_binding =
        OfflineCashRecursivePairBindingV1::new_guard_bundle([1; 32], [2; 32]).map_err(|error| {
            format!("failed to construct provisional GuardBundle binding: {error}")
        })?;
    let provisional_eq_guard_public = OfflineCashGuardBundleRecursivePublicV1::new(
        eq_guard_common.clone(),
        provisional_guard_binding,
        eq_guard_lineage,
    )
    .map_err(|error| format!("failed to build provisional Eq GuardBundle public input: {error}"))?;
    let provisional_ep_guard_public = OfflineCashGuardBundleRecursivePublicV1::new(
        ep_guard_common.clone(),
        provisional_guard_binding,
        ep_guard_lineage,
    )
    .map_err(|error| format!("failed to build provisional Ep GuardBundle public input: {error}"))?;
    let guard_audits = offline_cash_guard_bundle_keygen_audit_digests_v1(
        &provisional_eq_guard_public,
        &provisional_ep_guard_public,
        &eq_guard_recursion,
        &ep_guard_recursion,
    )?;
    let guard_binding =
        OfflineCashRecursivePairBindingV1::new_guard_bundle(guard_audits.0, guard_audits.1)
            .map_err(|error| format!("failed to construct final GuardBundle binding: {error}"))?;
    let eq_guard_public = OfflineCashGuardBundleRecursivePublicV1::new(
        eq_guard_common.clone(),
        guard_binding,
        eq_guard_lineage,
    )
    .map_err(|error| format!("failed to build Eq GuardBundle public input: {error}"))?;
    let ep_guard_public = OfflineCashGuardBundleRecursivePublicV1::new(
        ep_guard_common.clone(),
        guard_binding,
        ep_guard_lineage,
    )
    .map_err(|error| format!("failed to build Ep GuardBundle public input: {error}"))?;
    if offline_cash_guard_bundle_keygen_audit_digests_v1(
        &eq_guard_public,
        &ep_guard_public,
        &eq_guard_recursion,
        &ep_guard_recursion,
    )? != guard_audits
    {
        return Err("GuardBundle audit binding was not stable after final substitution".to_owned());
    }

    let (eq_guard_keygen, ep_guard_keygen) = build_guard_bundle_keygen_pair_v1(
        &eq_guard_public,
        &ep_guard_public,
        &eq_guard_recursion,
        &ep_guard_recursion,
    )?;
    let eq_guard_break_points = eq_guard_keygen.break_points();
    let ep_guard_break_points = ep_guard_keygen.break_points();
    let (eq_guard_proving_key, eq_guard_vk_sha256) = generate_processed_key_pair_v1(
        &eq_params,
        &eq_guard_keygen,
        Parity::Eq,
        CircuitRole::GuardBundle,
        Artifact::GuardBundlePkEq,
        Artifact::GuardBundleVkEq,
        &mut artifacts,
    )?;
    let (ep_guard_proving_key, ep_guard_vk_sha256) = generate_processed_key_pair_v1(
        &ep_params,
        &ep_guard_keygen,
        Parity::Ep,
        CircuitRole::GuardBundle,
        Artifact::GuardBundlePkEp,
        Artifact::GuardBundleVkEp,
        &mut artifacts,
    )?;
    let (eq_guard_prover, ep_guard_prover) = build_guard_bundle_prover_pair_v1(
        &eq_guard_public,
        &ep_guard_public,
        &eq_guard_recursion,
        &ep_guard_recursion,
        &eq_guard_break_points,
        &ep_guard_break_points,
    )?;
    let eq_guard_instances = eq_guard_public
        .instance_columns::<Fp>()
        .map_err(|error| format!("failed to pack Eq GuardBundle instances: {error}"))?;
    let ep_guard_instances = ep_guard_public
        .instance_columns::<Fq>()
        .map_err(|error| format!("failed to pack Ep GuardBundle instances: {error}"))?;
    let eq_guard_proof = prove_direct_child_v1(
        &eq_params,
        eq_guard_proving_key,
        eq_guard_vk_sha256,
        CircuitRole::GuardBundle,
        eq_guard_prover,
        eq_guard_instances,
    )?;
    let ep_guard_proof = prove_direct_child_v1(
        &ep_params,
        ep_guard_proving_key,
        ep_guard_vk_sha256,
        CircuitRole::GuardBundle,
        ep_guard_prover,
        ep_guard_instances,
    )?;

    let (eq_state_recursion, eq_state_carried) = build_state_recursion_v1(
        &eq_params,
        Parity::Eq,
        &eq_state_leaf,
        &eq_guard_proof,
        eq_guard_common,
        guard_binding,
        eq_guard_carried,
    )?;
    let (ep_state_recursion, ep_state_carried) = build_state_recursion_v1(
        &ep_params,
        Parity::Ep,
        &ep_state_leaf,
        &ep_guard_proof,
        ep_guard_common,
        guard_binding,
        ep_guard_carried,
    )?;
    let eq_state_lineage = offline_cash_lineage_from_eq_v1(&eq_state_carried)
        .map_err(|error| format!("failed to encode Eq final-State lineage: {error:?}"))?;
    let ep_state_lineage = offline_cash_lineage_from_ep_v1(&ep_state_carried)
        .map_err(|error| format!("failed to encode Ep final-State lineage: {error:?}"))?;

    let provisional_state_binding =
        OfflineCashRecursivePairBindingV1::new_state([1; 32], [2; 32], &guard_binding)
            .map_err(|error| format!("failed to construct provisional State binding: {error}"))?;
    let provisional_eq_state = OfflineCashStatePublicInstancesV1::from_leaf(
        eq_state_leaf_public.clone(),
        &provisional_state_binding,
    )
    .map_err(|error| format!("failed to build provisional Eq State public input: {error}"))?;
    let provisional_ep_state = OfflineCashStatePublicInstancesV1::from_leaf(
        ep_state_leaf_public.clone(),
        &provisional_state_binding,
    )
    .map_err(|error| format!("failed to build provisional Ep State public input: {error}"))?;
    let provisional_eq_state_public =
        OfflineCashStateRecursivePublicV1::new(provisional_eq_state, eq_state_lineage)
            .map_err(|error| format!("failed to build provisional Eq recursive State: {error}"))?;
    let provisional_ep_state_public =
        OfflineCashStateRecursivePublicV1::new(provisional_ep_state, ep_state_lineage)
            .map_err(|error| format!("failed to build provisional Ep recursive State: {error}"))?;
    let state_audits = offline_cash_state_keygen_audit_digests_v1(
        &provisional_eq_state_public,
        &provisional_ep_state_public,
        &eq_state_recursion,
        &ep_state_recursion,
    )?;
    let state_binding = OfflineCashRecursivePairBindingV1::new_state(
        state_audits.0,
        state_audits.1,
        &guard_binding,
    )
    .map_err(|error| format!("failed to construct final State binding: {error}"))?;
    let eq_state =
        OfflineCashStatePublicInstancesV1::from_leaf(eq_state_leaf_public, &state_binding)
            .map_err(|error| format!("failed to build Eq State public input: {error}"))?;
    let ep_state =
        OfflineCashStatePublicInstancesV1::from_leaf(ep_state_leaf_public, &state_binding)
            .map_err(|error| format!("failed to build Ep State public input: {error}"))?;
    let eq_state_public = OfflineCashStateRecursivePublicV1::new(eq_state, eq_state_lineage)
        .map_err(|error| format!("failed to build Eq recursive State: {error}"))?;
    let ep_state_public = OfflineCashStateRecursivePublicV1::new(ep_state, ep_state_lineage)
        .map_err(|error| format!("failed to build Ep recursive State: {error}"))?;
    if offline_cash_state_keygen_audit_digests_v1(
        &eq_state_public,
        &ep_state_public,
        &eq_state_recursion,
        &ep_state_recursion,
    )? != state_audits
    {
        return Err("final-State audit binding was not stable after final substitution".to_owned());
    }

    let (eq_state_keygen, ep_state_keygen) = build_state_keygen_pair_v1(
        &eq_state_public,
        &ep_state_public,
        &eq_state_recursion,
        &ep_state_recursion,
    )?;
    let eq_state_break_points = eq_state_keygen.break_points();
    let ep_state_break_points = ep_state_keygen.break_points();
    let (eq_state_proving_key, eq_state_vk_sha256) = generate_processed_key_pair_v1(
        &eq_params,
        &eq_state_keygen,
        Parity::Eq,
        CircuitRole::State,
        Artifact::StatePkEq,
        Artifact::StateVkEq,
        &mut artifacts,
    )?;
    let (ep_state_proving_key, ep_state_vk_sha256) = generate_processed_key_pair_v1(
        &ep_params,
        &ep_state_keygen,
        Parity::Ep,
        CircuitRole::State,
        Artifact::StatePkEp,
        Artifact::StateVkEp,
        &mut artifacts,
    )?;
    let (eq_state_prover, ep_state_prover) = build_state_prover_pair_v1(
        &eq_state_public,
        &ep_state_public,
        &eq_state_recursion,
        &ep_state_recursion,
        &eq_state_break_points,
        &ep_state_break_points,
    )?;
    let eq_state_instances = eq_state_public
        .instance_columns::<Fp>()
        .map_err(|error| format!("failed to pack Eq final-State instances: {error}"))?;
    let ep_state_instances = ep_state_public
        .instance_columns::<Fq>()
        .map_err(|error| format!("failed to pack Ep final-State instances: {error}"))?;
    let (eq_state_proof, eq_state_verifying_key) = create_final_state_proof_v1(
        &eq_params,
        eq_state_proving_key,
        eq_state_vk_sha256,
        eq_state_prover,
        &eq_state_instances,
    )?;
    let (ep_state_proof, ep_state_verifying_key) = create_final_state_proof_v1(
        &ep_params,
        ep_state_proving_key,
        ep_state_vk_sha256,
        ep_state_prover,
        &ep_state_instances,
    )?;
    terminal_verify_eq_outer_and_carried_v1(
        &eq_params,
        &eq_state_verifying_key,
        &eq_state_instances,
        &eq_state_proof,
        OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
        &eq_state_lineage,
    )
    .map_err(|error| {
        format!("Eq final-State outer/carried terminal qualification failed: {error:?}")
    })?;
    terminal_verify_ep_outer_and_carried_v1(
        &ep_params,
        &ep_state_verifying_key,
        &ep_state_instances,
        &ep_state_proof,
        OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
        &ep_state_lineage,
    )
    .map_err(|error| {
        format!("Ep final-State outer/carried terminal qualification failed: {error:?}")
    })?;

    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_canonical_role_has_one_unique_stable_file_name() {
        let names = OfflineCashArtifactRoleV1::ALL
            .into_iter()
            .map(offline_cash_artifact_file_name_v1)
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 34);
        assert_eq!(
            names.iter().copied().collect::<BTreeSet<_>>().len(),
            names.len()
        );
        assert!(names.iter().all(|name| {
            !name.is_empty()
                && name.ends_with(".bin")
                && !name.contains('/')
                && !name.contains("..")
        }));
    }

    #[test]
    fn bounded_spool_rejects_overflow_and_round_trips_bytes() {
        let role = OfflineCashArtifactRoleV1::GuardUseVkEq;
        let payload = b"developer-only-processed-key-test";
        let mut spool = spool_bytes(role, payload).expect("bounded test spool");
        assert_eq!(spool.role(), role);
        assert_eq!(spool.binding().byte_len, payload.len() as u64);
        assert_eq!(spool.binding().sha256, Sha256::digest(payload).into());
        let mut copied = Vec::new();
        spool.copy_to(&mut copied).expect("copy complete spool");
        assert_eq!(copied, payload);

        let mut writer = BoundedArtifactWriterV1::new(role).expect("bounded writer");
        writer.maximum = 2;
        writer
            .write_all(b"three")
            .expect("infallible serialization surface");
        assert!(writer.finish().is_err());
    }

    #[test]
    fn protocol_digest_is_absent_only_for_parameter_roles() {
        for role in OfflineCashArtifactRoleV1::ALL {
            let expected_none = matches!(
                role,
                OfflineCashArtifactRoleV1::ParamsEq | OfflineCashArtifactRoleV1::ParamsEp
            );
            assert_eq!(
                offline_cash_artifact_protocol_digest_v1(role).is_none(),
                expected_none
            );
        }
        assert_ne!(offline_cash_artifact_profile_digest_v1(), [0; 32]);
    }

    #[test]
    fn developer_fixture_joins_state_and_helper_without_a_fixed_point() {
        let fixture = DeveloperReceiveFixtureV1::new().expect("closed developer fixture");
        let state = fixture
            .state_leaf_v1(OfflineCashHalo2ParityV1::Eq)
            .expect("Eq StateLeaf")
            .relation_public()
            .expect("State relation public");
        let helper = fixture.helper_input_v1();
        assert_eq!(state.operation, OfflineCashStateOperationV1::ReceiveFold);
        assert_eq!(helper.operation, OfflineCashHelperOperationV1::ReceiveFold);
        assert_eq!(state.release_id, helper.release_id);
        assert_eq!(state.context_digest, helper.context_digest);
        assert_eq!(state.parent_0, helper.current_head);
        assert_eq!(state.transition_digest, helper.transition_digest);
        assert_ne!(helper.current_head, helper.transition_digest);
        assert_eq!(
            helper.from_sequence.checked_add(1),
            Some(helper.to_sequence)
        );
        assert_eq!(OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1 / 2, 3_072);
        assert_eq!(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1, 3_200);
    }
}
