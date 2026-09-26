//! Bounded offline model artifact adapters for the fixed six-lane DEEP profile.
//!
//! Public expectations and all AXT context come from the caller. Transport facts
//! are exact-compared before any child proof is decoded. One enclosing Norito
//! scope charges the artifact, carrier and every child cumulatively. This does
//! not authenticate the caller or qualify a production profile.
//! TODO: Independently qualify the protocol and authenticated production caller.

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqArtifactIdentityDescriptionV1, FastpqAxtCompactArtifactV1, FastpqCommitmentDescriptionV1,
    FastpqCompactArtifactDecodeError, FastpqCompactArtifactDecodeLimits, FastpqCompactProfileIdV1,
    FastpqOrderedCompactAirCommitmentsV1, FastpqOrdinaryCompactArtifactV1, FastpqProofKindV1,
    FastpqPublicTransferStatementV1, FastpqQuantityUnits,
};
use norito::{NoritoSerialize, core::DecodeLimits};
use sha2::{Digest as _, Sha256};

use crate::{
    Error, ProofSemantics,
    backend::{
        compact_bundle::{
            BundleLimits, VerifiedBundle, verify_axt_transfer_bundle_with_allocation,
            verify_transfer_bundle_with_allocation,
        },
        compact_public_api::AxtVerificationContext,
        compact_public_columns::{COMMITTED_COLUMN_COUNT, LAYOUT_ID},
        compact_value_domain::CompactTransferValue,
        deep_binding,
        deep_geometry::{
            CONSTRAINTS, COSET_OFFSET, FRI_ARITIES, FRI_DEGREES, FRI_LENGTHS, LDE_ROOT, LDE_ROWS,
            QUERY_CANDIDATES, QUERY_COUNT, TRACE_ROWS,
        },
        deep_proof::DeepProof,
    },
    gadgets::public_transfer_statement::PublicTransferLimits,
    proof::PublicIO,
};

#[cfg(test)]
use crate::backend::compact_v1;

/// Explicit caller ceilings at every decoded layer and across the whole request.
#[derive(Clone, Copy, Debug)]
pub(in crate::backend) struct ArtifactLimits {
    /// Bounded canonical transport, including its opaque child frame.
    pub(in crate::backend) transport: FastpqCompactArtifactDecodeLimits,
    /// Bounded conversion and validation of public transfer facts.
    pub(in crate::backend) public_statement: PublicTransferLimits,
    /// Cumulative carrier and per-segment policy.
    pub(in crate::backend) bundle: BundleLimits,
    /// Allocation charges permitted for each complete candidate child decode.
    pub(in crate::backend) max_segment_decode_allocation_charges: usize,
    /// Shared budget spanning transport, carrier and every child proof decode.
    pub(in crate::backend) total_decode: DecodeLimits,
}

/// Failure before returning an all-or-nothing candidate relation result.
#[derive(Debug, thiserror::Error)]
pub(in crate::backend) enum ArtifactError {
    /// Bounded transport or fixed diagnostic profile equality failed.
    #[error(transparent)]
    Transport(#[from] FastpqCompactArtifactDecodeError),
    /// Caller equality, public preparation or complete proof verification failed.
    #[error(transparent)]
    Verify(#[from] Error),
}

/// Complete mathematical verification and recomputed canonical content identity.
/// Private fields prevent constructing a result from an advertised description.
/// This type grants no caller authority, source finality or production qualification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend) struct VerifiedArtifact {
    bundle: VerifiedBundle,
    identity: FastpqArtifactIdentityDescriptionV1,
}

impl VerifiedArtifact {
    /// The complete bundle result whose child commitments populate this identity.
    pub(in crate::backend) fn bundle(&self) -> &VerifiedBundle {
        &self.bundle
    }
    /// Recomputed canonical digests and full authenticated ordered AIR commitments.
    pub(in crate::backend) fn identity(&self) -> &FastpqArtifactIdentityDescriptionV1 {
        &self.identity
    }
}

fn statement_digest(
    statement: &FastpqPublicTransferStatementV1,
    max_bytes: usize,
) -> crate::Result<[u8; 32]> {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let encoded = match norito::core::to_bytes_bounded(statement, max_bytes) {
        Ok(bytes) => bytes,
        Err(norito::core::BoundedEncodeError::FrameTooLarge {
            encoded_bytes,
            max_bytes,
        }) => {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_compact_identity_statement_bytes",
                actual: encoded_bytes,
                max: max_bytes,
            });
        }
        Err(norito::core::BoundedEncodeError::Serialization(error)) => {
            return Err(Error::Encode(error));
        }
        Err(error) => return Err(Error::Encode(norito::Error::Message(error.to_string()))),
    };
    Ok(Hash::new(&encoded).into())
}

fn finish_artifact<V: CompactTransferValue>(
    kind: FastpqProofKindV1,
    statement_digest: [u8; 32],
    wrapper: &[u8],
    inner: &[u8],
    bundle: VerifiedBundle,
) -> crate::Result<VerifiedArtifact> {
    // Both lengths are checked by the transport before any verification work;
    // checked conversion also preserves correctness on wider future usize hosts.
    let artifact_bytes =
        u64::try_from(wrapper.len()).map_err(|_| Error::Encode(norito::Error::LengthMismatch))?;
    let segment_count = u64::try_from(bundle.segments())
        .map_err(|_| Error::Encode(norito::Error::LengthMismatch))?;
    let identity = FastpqArtifactIdentityDescriptionV1 {
        proof_kind: kind,
        profile_id: profile_id_for::<V>(),
        public_statement_digest: statement_digest,
        artifact_digest: Hash::new(wrapper).into(),
        inner_bundle_digest: Hash::new(inner).into(),
        artifact_bytes,
        commitments: FastpqCommitmentDescriptionV1::OrderedCompactAir(
            FastpqOrderedCompactAirCommitmentsV1 {
                segment_count,
                segment_air_row_roots: bundle.row_roots().to_vec(),
            },
        ),
    };
    Ok(VerifiedArtifact { bundle, identity })
}

/// Offline candidate identity only; the production qualification registry stays empty.
#[cfg(test)]
pub(in crate::backend) fn diagnostic_profile_id() -> FastpqCompactProfileIdV1 {
    profile_id_for::<u64>()
}

/// Complete fixed DEEP descriptor, independent of untrusted artifact contents.
///
/// Quantity encodings and outer relation identities retain their existing
/// meaning. This distinct nominal frame identifies the replacement proof
/// protocol, including its actual geometry and canonical child-frame identity.
#[derive(Clone, NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_model_statement::candidate_artifact::DeepQuantityArtifactProfile",
    frame = "fastpq_prover::deep_compact::QuantityArtifactProfileV1"
)]
struct DeepQuantityArtifactProfile {
    version: u16,
    catalog: &'static str,
    protocol_identity: Vec<u8>,
    trace_rows: u32,
    trace_root: u64,
    lde_rows: u32,
    lde_root: u64,
    coset_offset: u64,
    committed_columns: u32,
    public_column_layout: &'static str,
    constraints: u32,
    modulus: u64,
    extension_degree: u8,
    extension_nonresidue: u64,
    extension_schema: String,
    extension_bytes: u32,
    hash_digest_lanes: u8,
    lane_parameter_sha3_256: [u8; 32],
    fri_arities: [u32; 5],
    fri_lengths: [u32; 6],
    fri_degrees: [u32; 6],
    query_count: u32,
    query_candidates: u32,
    tape_bytes: [u32; 10],
    proof_frame_schema: String,
    proof_frame_hash: [u8; 16],
    quantity_value_schema: &'static str,
    quantity_context_schema: &'static str,
    value_hash_domain: Vec<u8>,
    relation_identities: [&'static str; 2],
}

impl DeepQuantityArtifactProfile {
    fn fixed() -> Self {
        use norito::schema::identity::NoritoSchema as _;

        Self {
            version: 1,
            catalog: fastpq_isi::FASTPQ_CATALOG_V1,
            protocol_identity: deep_binding::IDENTITY.to_vec(),
            trace_rows: TRACE_ROWS as u32,
            trace_root: fastpq_isi::FASTPQ_FINAL_V1.trace_root,
            lde_rows: LDE_ROWS as u32,
            lde_root: LDE_ROOT,
            coset_offset: COSET_OFFSET,
            committed_columns: COMMITTED_COLUMN_COUNT as u32,
            public_column_layout: LAYOUT_ID,
            constraints: CONSTRAINTS as u32,
            modulus: crate::field::GOLDILOCKS_MODULUS_V1,
            extension_degree: 4,
            extension_nonresidue: 7,
            extension_schema: crate::GoldilocksFp4V1::frame_name(),
            extension_bytes: crate::GoldilocksFp4V1::BYTES as u32,
            hash_digest_lanes: 6,
            lane_parameter_sha3_256: fastpq_isi::GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1,
            fri_arities: FRI_ARITIES.map(|value| value as u32),
            fri_lengths: FRI_LENGTHS.map(|value| value as u32),
            fri_degrees: FRI_DEGREES.map(|value| value as u32),
            query_count: QUERY_COUNT as u32,
            query_candidates: QUERY_CANDIDATES as u32,
            tape_bytes: core::array::from_fn(|round| {
                deep_binding::Round::new(round as u8 + 1)
                    .expect("fixed DEEP round")
                    .tape_bytes() as u32
            }),
            proof_frame_schema: DeepProof::frame_name(),
            proof_frame_hash: norito::schema::identity::frame_hash::<DeepProof>(),
            quantity_value_schema: "fastpq_prover::public_transfer::QuantityValueV1",
            quantity_context_schema: "fastpq_prover::compact_v1::QuantityTransferContextV1",
            value_hash_domain: b"fastpq:quantity:v1:smt:value|".to_vec(),
            relation_identities: [
                FastpqQuantityUnits::BATCH_IDENTITY,
                FastpqQuantityUnits::AXT_BATCH_IDENTITY,
            ],
        }
    }

    fn profile_id(&self) -> FastpqCompactProfileIdV1 {
        FastpqCompactProfileIdV1(
            Sha256::digest(
                norito::encode_canonical(self).expect("bounded fixed profile description"),
            )
            .into(),
        )
    }
}

/// Full-domain offline identity only; it confers no production qualification.
pub(in crate::backend) fn quantity_diagnostic_profile_id() -> FastpqCompactProfileIdV1 {
    DeepQuantityArtifactProfile::fixed().profile_id()
}

fn profile_id_for<V: CompactTransferValue>() -> FastpqCompactProfileIdV1 {
    // The predecessor u64 fixture identity exists only in diagnostic tests.
    // Normal artifact construction and decoding share exactly one fixed profile.
    #[cfg(test)]
    if !V::QUANTITY_CONTEXT {
        return FastpqCompactProfileIdV1(Sha256::digest(compact_v1::IDENTITY).into());
    }
    quantity_diagnostic_profile_id()
}

/// Verify ordinary model bytes under the fixed candidate and caller-expected inputs.
/// No artifact field selects proof semantics or a protocol implementation.
#[cfg(test)]
pub(in crate::backend) fn verify_ordinary_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    verify_ordinary_artifact_for::<u64>(bytes, expected, None, limits)
}

/// Verify a complete ordinary QuantityValueV1 artifact under its fixed profile.
/// The caller supplies expected inputs; advertised metadata cannot select a format.
#[cfg(test)]
pub(in crate::backend) fn verify_quantity_ordinary_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    verify_ordinary_artifact_for::<FastpqQuantityUnits>(bytes, expected, None, limits)
}

/// Verify the fixed quantity route with an independently expected complete statement digest.
/// This mandatory normal-library input is checked before carrier or child verification.
pub(in crate::backend) fn verify_bound_quantity_ordinary_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    expected_statement_digest: [u8; 32],
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    verify_ordinary_artifact_for::<FastpqQuantityUnits>(
        bytes,
        expected,
        Some(expected_statement_digest),
        limits,
    )
}

fn verify_ordinary_artifact_for<V: CompactTransferValue>(
    bytes: &[u8],
    expected: &PublicIO,
    expected_statement_digest: Option<[u8; 32]>,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    norito::core::with_decode_limits_scope(limits.total_decode, || {
        let artifact = FastpqOrdinaryCompactArtifactV1::decode_canonical_with_limits(
            bytes,
            profile_id_for::<V>(),
            limits.transport,
        )?;
        let (bundle, digest) = super::with_prepared_statement_as::<V, _>(
            &artifact.statement,
            expected,
            ProofSemantics::StateTransition,
            limits.public_statement,
            |prepared| {
                let digest = statement_digest(
                    &artifact.statement,
                    limits.public_statement.max_public_bytes,
                )?;
                if expected_statement_digest.is_some_and(|expected| digest != expected) {
                    return Err(Error::PublicIoMismatch {
                        field: "compact_artifact_public_statement_digest",
                    });
                }
                let bundle = verify_transfer_bundle_with_allocation(
                    prepared,
                    expected,
                    &artifact.bundle_frame,
                    limits.bundle,
                    limits.max_segment_decode_allocation_charges,
                )?;
                Ok((bundle, digest))
            },
        )?;
        finish_artifact::<V>(
            FastpqProofKindV1::OrdinaryCompact,
            digest,
            bytes,
            &artifact.bundle_frame,
            bundle,
        )
        .map_err(ArtifactError::Verify)
    })
}

/// Verify AXT model bytes against every independently supplied caller expectation.
/// The artifact cannot substitute its own binding, mirrors, metadata or preimages.
#[cfg(test)]
pub(in crate::backend) fn verify_axt_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    verify_axt_artifact_for::<u64>(bytes, expected, None, context, limits)
}

/// Verify a complete AXT QuantityValueV1 artifact with independent caller context.
/// All binding, mirrors and remote preimages remain mandatory under this route.
#[cfg(test)]
pub(in crate::backend) fn verify_quantity_axt_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    verify_axt_artifact_for::<FastpqQuantityUnits>(bytes, expected, None, context, limits)
}

/// Verify the fixed AXT quantity route with a mandatory independent statement digest.
/// All AXT context and complete statement identity remain caller expectations.
pub(in crate::backend) fn verify_bound_quantity_axt_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    expected_statement_digest: [u8; 32],
    context: AxtVerificationContext<'_>,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    verify_axt_artifact_for::<FastpqQuantityUnits>(
        bytes,
        expected,
        Some(expected_statement_digest),
        context,
        limits,
    )
}

fn verify_axt_artifact_for<V: CompactTransferValue>(
    bytes: &[u8],
    expected: &PublicIO,
    expected_statement_digest: Option<[u8; 32]>,
    context: AxtVerificationContext<'_>,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    norito::core::with_decode_limits_scope(limits.total_decode, || {
        let artifact = FastpqAxtCompactArtifactV1::decode_canonical_with_limits(
            bytes,
            profile_id_for::<V>(),
            limits.transport,
        )?;
        validate_axt_advertisement(&artifact, context)?;
        let (bundle, digest) = super::with_prepared_statement_as::<V, _>(
            &artifact.statement,
            expected,
            ProofSemantics::AxtTransferClaim,
            limits.public_statement,
            |prepared| {
                let digest = statement_digest(
                    &artifact.statement,
                    limits.public_statement.max_public_bytes,
                )?;
                if expected_statement_digest.is_some_and(|expected| digest != expected) {
                    return Err(Error::PublicIoMismatch {
                        field: "compact_artifact_public_statement_digest",
                    });
                }
                let bundle = verify_axt_transfer_bundle_with_allocation(
                    prepared,
                    expected,
                    context,
                    &artifact.bundle_frame,
                    limits.bundle,
                    limits.max_segment_decode_allocation_charges,
                )?;
                Ok((bundle, digest))
            },
        )?;
        finish_artifact::<V>(
            FastpqProofKindV1::AxtCompact,
            digest,
            bytes,
            &artifact.bundle_frame,
            bundle,
        )
        .map_err(ArtifactError::Verify)
    })
}

fn validate_axt_advertisement(
    artifact: &FastpqAxtCompactArtifactV1,
    context: AxtVerificationContext<'_>,
) -> crate::Result<()> {
    let advertised = &artifact.metadata;
    let expected = context.metadata;
    let mirrors = artifact.mirrors;
    for (field, matches) in [
        (
            "compact_artifact_binding",
            artifact.binding == *context.binding,
        ),
        (
            "compact_artifact_parameter",
            advertised.parameter == expected.parameter,
        ),
        (
            "compact_artifact_entry_hash",
            advertised.entry_hash.as_slice() == expected.entry_hash,
        ),
        (
            "compact_artifact_amount_bytes",
            advertised.committed_amount.as_ref().map(|b| b.as_slice()) == expected.committed_amount,
        ),
        (
            "compact_artifact_expiry_bytes",
            advertised.expiry_slot.as_slice() == expected.expiry_slot,
        ),
        (
            "compact_artifact_manifest_bytes",
            advertised.manifest_root.as_slice() == expected.manifest_root,
        ),
        (
            "compact_artifact_da_bytes",
            advertised.da_commitment.as_slice() == expected.da_commitment,
        ),
        (
            "compact_artifact_mirror_dsid",
            mirrors.dsid == context.mirrors.dsid,
        ),
        (
            "compact_artifact_mirror_manifest",
            mirrors.manifest_root == context.mirrors.manifest_root,
        ),
        (
            "compact_artifact_mirror_da",
            mirrors.da_commitment == context.mirrors.da_commitment,
        ),
        (
            "compact_artifact_mirror_amount",
            mirrors.committed_amount == context.mirrors.committed_amount,
        ),
        (
            "compact_artifact_mirror_expiry",
            mirrors.expiry_slot == context.mirrors.expiry_slot,
        ),
        (
            "compact_artifact_remote_claims",
            artifact.remote_spend_claims.as_deref() == context.remote_spend_claims,
        ),
    ] {
        if !matches {
            return Err(Error::PublicIoMismatch { field });
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "compact_artifact_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "compact_quantity_artifact_tests.rs"]
mod quantity_tests;
