//! Release-bound Kagemusha verifier registry identities.

use super::*;

const VERIFIER_OWNER_PREFIX_V4: &str = "kagemusha-v4-";
const VERIFIER_OWNER_PREFIX_V5: &str = "kagemusha-v5-";
const VERIFIER_KEY_PREFIX_V5: &str = "v5-";
const VERIFIER_IDENTITY_SCHEMA_V4: &str = "kagemusha.offline.recursive_spend.verifier_identity.v4";
const VERIFIER_IDENTITY_SCHEMA_V5: &str = "kagemusha.offline.recursive_spend.verifier_identity.v5";

#[derive(Encode)]
struct VerifierIdentity {
    schema: String,
    version: u16,
    manifest_sha256: [u8; 32],
    parity: KagemushaPastaCycleParityV1,
    circuit_id: String,
    circuit_params_sha256: [u8; 32],
    compiled_protocol_structure_sha256: [u8; 32],
    public_input_limbs: u32,
}

/// Return the release-qualified verifier-key registry identifier for one ABI-21 parity.
///
/// The manifest digest suffix keeps verifier records for overlapping retained releases distinct
/// while preserving the fixed Eq/Ep circuit identity inside each [`VerifyingKeyRecord`].
#[must_use]
pub fn kagemusha_recursive_spend_verifier_key_id_v4(
    parity: KagemushaPastaCycleParityV1,
    manifest_sha256: [u8; 32],
) -> VerifyingKeyId {
    let circuit_id = match parity {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
    };
    VerifyingKeyId::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        format!("{circuit_id}-{}", hex::encode(manifest_sha256)),
    )
}

/// Return the release-qualified verifier-key registry identifier for one V5 release parity.
///
/// V5 retains the ABI-21 circuit ids but uses a disjoint registry namespace,
/// so V4 and V5 release records cannot select the same qualified registry id
/// even when their canonical manifest digests happen to match.
#[must_use]
pub fn kagemusha_recursive_spend_verifier_key_id_v5(
    parity: KagemushaPastaCycleParityV1,
    manifest_sha256: [u8; 32],
) -> VerifyingKeyId {
    let circuit_id = match parity {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
    };
    VerifyingKeyId::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        format!(
            "{VERIFIER_KEY_PREFIX_V5}{circuit_id}-{}",
            hex::encode(manifest_sha256)
        ),
    )
}

fn public_inputs_schema_hash(
    schema: &str,
    version: u16,
    manifest_sha256: [u8; 32],
    parity: KagemushaPastaCycleParityV1,
    profile: &KagemushaPastaCycleProofProfileV4,
) -> Result<[u8; 32], KagemushaValidationError> {
    let identity = VerifierIdentity {
        schema: schema.to_owned(),
        version,
        manifest_sha256,
        parity,
        circuit_id: profile.circuit_id.clone(),
        circuit_params_sha256: profile.circuit_params_sha256()?,
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        public_input_limbs: profile.circuit_params.public_input_limbs,
    };
    Ok(Hash::new(norito::encode_canonical(&identity)?).into())
}

/// Return the exact manifest-owner identifier required by a V4 verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_verifier_owner_manifest_id_v4(
    manifest_sha256: [u8; 32],
) -> String {
    format!("{VERIFIER_OWNER_PREFIX_V4}{}", hex::encode(manifest_sha256))
}

/// Derive the manifest- and layout-bound V4 verifier public-input identity.
///
/// # Errors
///
/// Returns [`KagemushaValidationError`] when the manifest, profile, or canonical identity is invalid.
pub fn kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v4(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], KagemushaValidationError> {
    let manifest_sha256 = manifest.canonical_sha256()?;
    let profile = manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "pasta_cycle.v4.verifier_identity.profile",
        })?;
    public_inputs_schema_hash(
        VERIFIER_IDENTITY_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        manifest_sha256,
        parity,
        profile,
    )
}

/// Return the exact manifest-owner identifier required by a V5 verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_verifier_owner_manifest_id_v5(
    manifest_sha256: [u8; 32],
) -> String {
    format!("{VERIFIER_OWNER_PREFIX_V5}{}", hex::encode(manifest_sha256))
}

/// Derive the manifest- and layout-bound V5 verifier public-input identity.
///
/// # Errors
///
/// Returns [`KagemushaValidationError`] when the manifest, profile, or canonical identity is invalid.
pub fn kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v5(
    manifest: &KagemushaRecursiveSpendArtifactManifestV5,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], KagemushaValidationError> {
    let manifest_sha256 = manifest.canonical_sha256()?;
    let profile = manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "pasta_cycle.v5.verifier_identity.profile",
        })?;
    public_inputs_schema_hash(
        VERIFIER_IDENTITY_SCHEMA_V5,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V5,
        manifest_sha256,
        parity,
        profile,
    )
}

pub(super) fn verifying_key_commitment_v1(
    key: &VerifyingKeyBox,
) -> Result<[u8; 32], KagemushaReleaseVerificationError> {
    let backend = key.backend.as_str();
    let backend_len = u64::try_from(backend.len())
        .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
    let key_len = u64::try_from(key.bytes.len())
        .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
    let mut hasher = Sha256::new();
    hasher.update(b"iroha:zk:v1:vk");
    hasher.update(backend_len.to_be_bytes());
    hasher.update(backend.as_bytes());
    hasher.update(key_len.to_be_bytes());
    hasher.update(&key.bytes);
    Ok(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v5_verifier_key_ids_are_disjoint_from_v4_for_the_same_manifest_digest() {
        let manifest_sha256 = [0x5a; 32];

        for parity in [
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleParityV1::StepEp,
        ] {
            let v4 = kagemusha_recursive_spend_verifier_key_id_v4(parity, manifest_sha256);
            let v5 = kagemusha_recursive_spend_verifier_key_id_v5(parity, manifest_sha256);

            assert_ne!(v5, v4);
            assert!(v5.is_portable_registry_id());
            assert!(v5.name.starts_with(VERIFIER_KEY_PREFIX_V5));
        }
    }
}
