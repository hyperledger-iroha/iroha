//! Bounded test-only model artifact adapters for the fixed SHAKE candidate.
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
    FastpqPublicTransferStatementV1,
};
use norito::core::DecodeLimits;
use sha2::{Digest as _, Sha256};

use crate::{
    Error, ProofSemantics,
    backend::{
        compact_bundle::{
            BundleLimits, VerifiedBundle, verify_shake_axt_transfer_bundle,
            verify_shake_transfer_bundle,
        },
        compact_public_api::AxtVerificationContext,
        compact_shake_candidate,
    },
    gadgets::public_transfer_statement::PublicTransferLimits,
    proof::PublicIO,
};

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

fn finish_artifact(
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
        profile_id: diagnostic_profile_id(),
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
pub(in crate::backend) fn diagnostic_profile_id() -> FastpqCompactProfileIdV1 {
    FastpqCompactProfileIdV1(Sha256::digest(compact_shake_candidate::IDENTITY).into())
}

/// Verify ordinary model bytes under the fixed candidate and caller-expected inputs.
/// No artifact field selects proof semantics or a protocol implementation.
pub(in crate::backend) fn verify_ordinary_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    norito::core::with_decode_limits_scope(limits.total_decode, || {
        let artifact = FastpqOrdinaryCompactArtifactV1::decode_canonical_with_limits(
            bytes,
            diagnostic_profile_id(),
            limits.transport,
        )?;
        let (bundle, digest) = super::with_prepared_statement(
            &artifact.statement,
            expected,
            ProofSemantics::StateTransition,
            limits.public_statement,
            |prepared| {
                let digest = statement_digest(
                    &artifact.statement,
                    limits.public_statement.max_public_bytes,
                )?;
                let bundle = verify_shake_transfer_bundle(
                    prepared,
                    expected,
                    &artifact.bundle_frame,
                    limits.bundle,
                    limits.max_segment_decode_allocation_charges,
                )?;
                Ok((bundle, digest))
            },
        )?;
        finish_artifact(
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
pub(in crate::backend) fn verify_axt_artifact(
    bytes: &[u8],
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    limits: ArtifactLimits,
) -> Result<VerifiedArtifact, ArtifactError> {
    norito::core::with_decode_limits_scope(limits.total_decode, || {
        let artifact = FastpqAxtCompactArtifactV1::decode_canonical_with_limits(
            bytes,
            diagnostic_profile_id(),
            limits.transport,
        )?;
        validate_axt_advertisement(&artifact, context)?;
        let (bundle, digest) = super::with_prepared_statement(
            &artifact.statement,
            expected,
            ProofSemantics::AxtTransferClaim,
            limits.public_statement,
            |prepared| {
                let digest = statement_digest(
                    &artifact.statement,
                    limits.public_statement.max_public_bytes,
                )?;
                let bundle = verify_shake_axt_transfer_bundle(
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
        finish_artifact(
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
#[path = "compact_shake_artifact_tests.rs"]
mod tests;
