//! Deterministic manifest for native first-release privacy engines.
//!
//! Governance does not get to turn arbitrary non-zero digests into executable
//! consensus code. Every activatable protocol must have one compiled profile
//! whose parameter, verifier, structurally derived statement-schema,
//! engine-manifest, and limit
//! bindings exactly match the proposed activation record. A protocol whose
//! complete verifier is not compiled is rejected before it enters world state.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::privacy::{
    PrivacyAssuranceV1, PrivacyConsensusLimitsV1, PrivacyEngineIdV1, PrivacyEngineManifestDigestV1,
    PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyProofSystemIdV1,
    PrivacyProtocolActivationLimitsV1, PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1,
    PrivacyProtocolLifecycleV1, PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1,
    TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1, VERANGE_HARD_MAX_AGGREGATION_COUNT_V1,
    VeRangeActivationLimitsV1, VeRangeTransparentRangeStatementV1,
};
use iroha_schema::{FloatMode, IntMode, IntoSchema, MetaMapEntry, Metadata};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::privacy_engines::verange::{
    VERANGE_TYPE1_PROOF_VERSION_V1, VERANGE_TYPE1_SOURCE_PROFILE_V1, VERANGE_TYPE1_SUITE_V1,
    VeRangeBitLengthV1, VeRangeParametersV1,
};

const PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.digest.v1";
const PARAMETER_ID_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.parameter-id.v1";
const VERIFIER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.verifier-digest.v1";
const CANONICAL_SCHEMA_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.compiled-profile.canonical-structural-schema.v1";
const ENGINE_MANIFEST_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.compiled-profile.engine-manifest-digest.v1";
const VERANGE_PROTOCOL_LABEL_V1: &[u8] = b"verange-transparent-range-v1";
const VERANGE_PARAMETER_SET_LABEL_V1: &[u8] = b"p256-type1-bits32-bits64-v1";
const VERANGE_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:ordered-independent-type1-batch:strict-exact:v1";
const VERANGE_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:eprint-2025-528:figure-1:type-1:v1";

/// Exact compiled bindings for one native privacy protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompiledPrivacyProfileV1 {
    /// Closed protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Closed proof-system identity.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Closed native-engine identity.
    pub engine_id: PrivacyEngineIdV1,
    /// Deterministic identifier of the compiled parameter set.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the exact compiled parameters.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier relation and proof wire.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of the exact public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the complete compiled engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Exact protocol-specific limits compiled into the verifier.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
}

impl CompiledPrivacyProfileV1 {
    /// Build the only valid activation record for this compiled profile.
    #[must_use]
    pub fn activation_record(
        self,
        lifecycle: PrivacyProtocolLifecycleV1,
    ) -> PrivacyProtocolActivationRecordV1 {
        PrivacyProtocolActivationRecordV1 {
            protocol_id: self.protocol_id,
            proof_system_id: self.proof_system_id,
            engine_id: self.engine_id,
            parameter_id: self.parameter_id,
            parameter_digest: self.parameter_digest,
            verifier_digest: self.verifier_digest,
            statement_schema_digest: self.statement_schema_digest,
            engine_manifest_digest: self.engine_manifest_digest,
            lifecycle,
            limits: PrivacyConsensusLimitsV1::taira_default(),
            protocol_limits: self.protocol_limits,
            assurance: PrivacyAssuranceV1::Experimental,
        }
    }
}

/// Return the exact compiled profile for an executable native verifier.
///
/// # Errors
///
/// Returns [`CompiledPrivacyProfileErrorV1::EngineUnavailable`] for a protocol
/// whose complete end-to-end verifier is not compiled, or
/// [`CompiledPrivacyProfileErrorV1::ProfileInitializationFailed`] if fixed
/// transparent parameters cannot be derived.
pub fn compiled_privacy_profile_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    match protocol_id {
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => compiled_verange_profile_v1(),
        // TODO(privacy-native-engines): remove each fail-closed branch only
        // after its complete canonical verifier, effect derivation, KATs, and
        // adversarial tests are compiled into this manifest.
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
        | PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
        | PrivacyProtocolIdV1::IrohaZkAmsStarkV0
        | PrivacyProtocolIdV1::VegaExistingCredentialZkV0
        | PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
        | PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0
        | PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0
        | PrivacyProtocolIdV1::OrchardHalo2ActionsV1
        | PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
        | PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
        | PrivacyProtocolIdV1::PqMaspStarkV0 => {
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        }
    }
}

/// Require an activation to equal the locally compiled executable profile.
///
/// # Errors
///
/// Returns a typed mismatch for the first differing consensus binding, or an
/// unavailable/initialization error when no executable profile exists.
pub fn validate_compiled_privacy_activation_v1(
    activation: &PrivacyProtocolActivationRecordV1,
) -> Result<(), CompiledPrivacyProfileValidationErrorV1> {
    let compiled = compiled_privacy_profile_v1(activation.protocol_id)
        .map_err(CompiledPrivacyProfileValidationErrorV1::Profile)?;
    if activation.proof_system_id != compiled.proof_system_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch);
    }
    if activation.engine_id != compiled.engine_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::EngineMismatch);
    }
    if activation.parameter_id != compiled.parameter_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch);
    }
    if activation.parameter_digest != compiled.parameter_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch);
    }
    if activation.verifier_digest != compiled.verifier_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch);
    }
    if activation.statement_schema_digest != compiled.statement_schema_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch);
    }
    if activation.engine_manifest_digest != compiled.engine_manifest_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch);
    }
    if activation.protocol_limits != compiled.protocol_limits {
        return Err(CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch);
    }
    if activation.limits != PrivacyConsensusLimitsV1::taira_default() {
        return Err(CompiledPrivacyProfileValidationErrorV1::ConsensusLimitsMismatch);
    }
    if activation.assurance != PrivacyAssuranceV1::Experimental {
        return Err(CompiledPrivacyProfileValidationErrorV1::AssuranceMismatch);
    }
    Ok(())
}

fn compiled_verange_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1>
{
    let bits_32 = VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits32).map_err(|_| {
        CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        }
    })?;
    let bits_64 = VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits64).map_err(|_| {
        CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        }
    })?;
    if bits_32.parameter_digest() != bits_64.parameter_digest() {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        });
    }

    let bits_32_descriptor = bits_32.descriptor();
    let bits_64_descriptor = bits_64.descriptor();
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_PARAMETER_SET_LABEL_V1,
            VERANGE_TYPE1_SOURCE_PROFILE_V1,
        ],
    );
    let proof_version = [VERANGE_TYPE1_PROOF_VERSION_V1];
    let bit_length_32 = bits_32_descriptor.bit_length.to_be_bytes();
    let rows_32 = bits_32_descriptor.rows.to_be_bytes();
    let columns_32 = bits_32_descriptor.columns.to_be_bytes();
    let bit_length_64 = bits_64_descriptor.bit_length.to_be_bytes();
    let rows_64 = bits_64_descriptor.rows.to_be_bytes();
    let columns_64 = bits_64_descriptor.columns.to_be_bytes();
    let max_batch = bits_64_descriptor.max_batch_commitments.to_be_bytes();
    let max_batch_proof = bits_64_descriptor.max_batch_proof_bytes.to_be_bytes();
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_TYPE1_SOURCE_PROFILE_V1,
            VERANGE_TYPE1_SUITE_V1,
            &proof_version,
            VERANGE_PROOF_WIRE_LABEL_V1,
            &bit_length_32,
            &rows_32,
            &columns_32,
            &bits_32_descriptor.generator_digest,
            &bit_length_64,
            &rows_64,
            &columns_64,
            &bits_64_descriptor.generator_digest,
            &max_batch,
            &max_batch_proof,
        ],
    );
    let statement_schema_digest =
        canonical_schema_digest_v1::<VeRangeTransparentRangeStatementV1>().map_err(|_| {
            CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
                protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            }
        })?;
    let parameter_digest = bits_32.parameter_digest();
    let global_commitment_cap = TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1.to_be_bytes();
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:iroha-verange-p256",
            b"engine:native-verange-p256",
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &global_commitment_cap,
        ],
    );
    let max_aggregation_count =
        VERANGE_HARD_MAX_AGGREGATION_COUNT_V1.min(TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1);

    Ok(CompiledPrivacyProfileV1 {
        protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        proof_system_id: PrivacyProofSystemIdV1::IrohaVeRangeP256,
        engine_id: PrivacyEngineIdV1::NativeVeRangeP256,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count,
            },
        ),
    })
}

fn digest_fields_v1(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, domain);
    hash.update(usize_to_u64_v1(fields.len()).to_be_bytes());
    for field in fields {
        append_digest_field_v1(&mut hash, field);
    }
    hash.finalize().into()
}

fn append_digest_field_v1(hash: &mut Sha256, field: &[u8]) {
    hash.update(usize_to_u64_v1(field.len()).to_be_bytes());
    hash.update(field);
}

/// Hash the complete structural schema of `T` in a platform-independent order.
///
/// The `iroha_schema` map is keyed internally by Rust [`core::any::TypeId`],
/// whose ordering is not a wire contract. This function replaces every such
/// reference with its declared stable string identifier, rejects duplicate
/// identifiers, sorts top-level entries by that stable identifier, and
/// preserves field/variant order where order is part of the representation.
/// Consequently, adding, deleting, reordering, or retyping a statement field
/// changes the governed digest without relying on a hand-maintained schema
/// string.
///
/// # Errors
///
/// Returns a typed error if the generated schema has duplicate stable type
/// identifiers or contains a reference to a type omitted from its own map.
pub fn canonical_schema_digest_v1<T: IntoSchema>() -> Result<[u8; 32], CanonicalSchemaDigestErrorV1>
{
    let schema = T::schema();
    let mut type_names = BTreeMap::new();
    let mut stable_ids = BTreeSet::new();
    let mut entries = Vec::new();
    for (rust_type_id, entry) in schema.iter() {
        if !stable_ids.insert(entry.type_id.clone()) {
            return Err(CanonicalSchemaDigestErrorV1::DuplicateStableTypeId);
        }
        type_names.insert(*rust_type_id, entry.type_id.clone());
        entries.push(entry);
    }
    entries.sort_by(|left, right| {
        left.type_id
            .cmp(&right.type_id)
            .then_with(|| left.type_name.cmp(&right.type_name))
    });

    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, CANONICAL_SCHEMA_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, T::id().as_bytes());
    append_digest_count_v1(&mut hash, entries.len());
    for entry in entries {
        append_digest_field_v1(&mut hash, entry.type_id.as_bytes());
        append_digest_field_v1(&mut hash, entry.type_name.as_bytes());
        append_schema_metadata_v1(&mut hash, entry, &type_names)?;
    }
    Ok(hash.finalize().into())
}

fn append_schema_metadata_v1(
    hash: &mut Sha256,
    entry: &MetaMapEntry,
    type_names: &BTreeMap<core::any::TypeId, String>,
) -> Result<(), CanonicalSchemaDigestErrorV1> {
    match &entry.metadata {
        Metadata::Struct(fields) => {
            append_digest_field_v1(hash, b"struct");
            append_digest_count_v1(hash, fields.declarations.len());
            for field in &fields.declarations {
                append_digest_field_v1(hash, field.name.as_bytes());
                append_schema_type_reference_v1(hash, field.ty, type_names)?;
            }
        }
        Metadata::Tuple(fields) => {
            append_digest_field_v1(hash, b"tuple");
            append_digest_count_v1(hash, fields.types.len());
            for field_type in &fields.types {
                append_schema_type_reference_v1(hash, *field_type, type_names)?;
            }
        }
        Metadata::Enum(enum_meta) => {
            append_digest_field_v1(hash, b"enum");
            append_digest_count_v1(hash, enum_meta.variants.len());
            for variant in &enum_meta.variants {
                append_digest_field_v1(hash, variant.tag.as_bytes());
                append_digest_field_v1(hash, &[variant.discriminant]);
                match variant.ty {
                    Some(variant_type) => {
                        append_digest_field_v1(hash, b"some");
                        append_schema_type_reference_v1(hash, variant_type, type_names)?;
                    }
                    None => append_digest_field_v1(hash, b"none"),
                }
            }
        }
        Metadata::Int(mode) => {
            append_digest_field_v1(hash, b"int");
            append_digest_field_v1(
                hash,
                match mode {
                    IntMode::FixedWidth => b"fixed-width",
                    IntMode::Compact => b"compact",
                },
            );
        }
        Metadata::Float(mode) => {
            append_digest_field_v1(hash, b"float");
            append_digest_field_v1(
                hash,
                match mode {
                    FloatMode::Binary32 => b"binary32",
                    FloatMode::Binary64 => b"binary64",
                },
            );
        }
        Metadata::String => append_digest_field_v1(hash, b"string"),
        Metadata::Bool => append_digest_field_v1(hash, b"bool"),
        Metadata::FixedPoint(fixed) => {
            append_digest_field_v1(hash, b"fixed-point");
            append_schema_type_reference_v1(hash, fixed.base, type_names)?;
            append_digest_field_v1(hash, &fixed.decimal_places.to_be_bytes());
        }
        Metadata::Array(array) => {
            append_digest_field_v1(hash, b"array");
            append_schema_type_reference_v1(hash, array.ty, type_names)?;
            append_digest_field_v1(hash, &array.len.to_be_bytes());
        }
        Metadata::Vec(vector) => {
            append_digest_field_v1(hash, b"vec");
            append_schema_type_reference_v1(hash, vector.ty, type_names)?;
        }
        Metadata::Map(map) => {
            append_digest_field_v1(hash, b"map");
            append_schema_type_reference_v1(hash, map.key, type_names)?;
            append_schema_type_reference_v1(hash, map.value, type_names)?;
        }
        Metadata::Option(option_type) => {
            append_digest_field_v1(hash, b"option");
            append_schema_type_reference_v1(hash, *option_type, type_names)?;
        }
        Metadata::Result(result) => {
            append_digest_field_v1(hash, b"result");
            append_schema_type_reference_v1(hash, result.ok, type_names)?;
            append_schema_type_reference_v1(hash, result.err, type_names)?;
        }
        Metadata::Bitmap(bitmap) => {
            append_digest_field_v1(hash, b"bitmap");
            append_schema_type_reference_v1(hash, bitmap.repr, type_names)?;
            append_digest_count_v1(hash, bitmap.masks.len());
            for mask in &bitmap.masks {
                append_digest_field_v1(hash, mask.name.as_bytes());
                append_digest_field_v1(hash, &mask.mask.to_be_bytes());
            }
        }
    }
    Ok(())
}

fn append_schema_type_reference_v1(
    hash: &mut Sha256,
    rust_type_id: core::any::TypeId,
    type_names: &BTreeMap<core::any::TypeId, String>,
) -> Result<(), CanonicalSchemaDigestErrorV1> {
    let stable_id = type_names
        .get(&rust_type_id)
        .ok_or(CanonicalSchemaDigestErrorV1::MissingTypeReference)?;
    append_digest_field_v1(hash, stable_id.as_bytes());
    Ok(())
}

fn append_digest_count_v1(hash: &mut Sha256, count: usize) {
    hash.update(usize_to_u64_v1(count).to_be_bytes());
}

#[cfg(target_pointer_width = "64")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from_ne_bytes(value.to_ne_bytes())
}

#[cfg(target_pointer_width = "32")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from(u32::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(target_pointer_width = "16")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from(u16::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(not(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
)))]
compile_error!("privacy profile digest framing supports 16-, 32-, and 64-bit pointer widths");

/// Invalid structural schema emitted by an [`IntoSchema`] implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CanonicalSchemaDigestErrorV1 {
    /// Two distinct Rust types claimed the same stable schema identifier.
    #[error("privacy statement schema contains a duplicate stable type identifier")]
    DuplicateStableTypeId,
    /// Metadata referenced a type absent from the generated schema map.
    #[error("privacy statement schema contains an unresolved type reference")]
    MissingTypeReference,
}

/// Failure constructing a locally compiled privacy profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompiledPrivacyProfileErrorV1 {
    /// The protocol has no complete native verifier in this binary.
    #[error("native privacy engine for {protocol_id:?} is unavailable")]
    EngineUnavailable {
        /// Protocol whose verifier is intentionally fail-closed.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// Deterministic transparent parameter initialization failed.
    #[error("compiled privacy profile initialization failed for {protocol_id:?}")]
    ProfileInitializationFailed {
        /// Protocol whose fixed parameters failed to initialize.
        protocol_id: PrivacyProtocolIdV1,
    },
}

/// Failure matching governance material to compiled consensus code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompiledPrivacyProfileValidationErrorV1 {
    /// No executable compiled profile could be obtained.
    #[error(transparent)]
    Profile(CompiledPrivacyProfileErrorV1),
    /// Proof-system identity differs.
    #[error("privacy activation proof system differs from compiled profile")]
    ProofSystemMismatch,
    /// Engine identity differs.
    #[error("privacy activation engine differs from compiled profile")]
    EngineMismatch,
    /// Parameter-set identity differs.
    #[error("privacy activation parameter id differs from compiled profile")]
    ParameterIdMismatch,
    /// Parameter digest differs.
    #[error("privacy activation parameter digest differs from compiled profile")]
    ParameterDigestMismatch,
    /// Verifier digest differs.
    #[error("privacy activation verifier digest differs from compiled profile")]
    VerifierDigestMismatch,
    /// Public-statement schema digest differs.
    #[error("privacy activation statement schema differs from compiled profile")]
    StatementSchemaDigestMismatch,
    /// Engine manifest digest differs.
    #[error("privacy activation engine manifest differs from compiled profile")]
    EngineManifestDigestMismatch,
    /// Protocol-specific limits differ.
    #[error("privacy activation protocol limits differ from compiled profile")]
    ProtocolLimitsMismatch,
    /// Chain-wide Taira limits differ.
    #[error("privacy activation consensus limits differ from compiled Taira profile")]
    ConsensusLimitsMismatch,
    /// The first-release assurance tag differs.
    #[error("privacy activation assurance differs from compiled testnet profile")]
    AssuranceMismatch,
}

#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{AnonymousPgcActivationLimitsV1, PrivacyProposedLifecycleV1};
    use iroha_schema::{Declaration, MetaMap, NamedFieldsMeta, TypeId};

    use super::*;

    struct SchemaOrderAb;

    impl TypeId for SchemaOrderAb {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderAb {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    struct SchemaOrderBa;

    impl TypeId for SchemaOrderBa {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderBa {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                ],
            }));
        }
    }

    struct SchemaRetyped;

    impl TypeId for SchemaRetyped {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaRetyped {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    fn verange_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("fixed VeRange parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    #[test]
    fn only_complete_engines_have_compiled_profiles() {
        let available = PrivacyProtocolIdV1::ALL
            .into_iter()
            .filter(|protocol_id| compiled_privacy_profile_v1(*protocol_id).is_ok())
            .collect::<Vec<_>>();
        assert_eq!(
            available,
            vec![PrivacyProtocolIdV1::VeRangeTransparentRangeV1]
        );
    }

    #[test]
    fn structural_schema_digest_detects_reordering_and_retyping() {
        let original = canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema");
        let reordered = canonical_schema_digest_v1::<SchemaOrderBa>().expect("schema");
        let retyped = canonical_schema_digest_v1::<SchemaRetyped>().expect("schema");
        assert_ne!(original, reordered);
        assert_ne!(original, retyped);
        assert_ne!(reordered, retyped);
        assert_eq!(
            original,
            canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema")
        );
    }

    #[test]
    fn verange_profile_is_deterministic_and_uses_effective_global_cap() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                VeRangeActivationLimitsV1 {
                    max_aggregation_count: 8,
                }
            )
        );
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
    }

    #[test]
    fn every_compiled_binding_and_limit_is_immutable() {
        let valid = verange_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");

        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 9] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| record.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeGoldilocksStarkFri,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    let PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                        ref mut limits,
                    ) = record.protocol_limits
                    else {
                        unreachable!("fixture is VeRange");
                    };
                    limits.max_aggregation_count -= 1;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ConsensusLimitsMismatch,
                |record| record.limits.max_proof_bytes_per_action -= 1,
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn unavailable_protocol_fails_before_governance_state_mutation() {
        let mut activation = verange_activation();
        activation.protocol_id = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
        activation.proof_system_id = activation.protocol_id.expected_proof_system();
        activation.engine_id = activation.protocol_id.expected_engine();
        activation.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 8,
            },
        );
        assert_eq!(
            validate_compiled_privacy_activation_v1(&activation),
            Err(CompiledPrivacyProfileValidationErrorV1::Profile(
                CompiledPrivacyProfileErrorV1::EngineUnavailable {
                    protocol_id: PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                }
            ))
        );
    }
}
