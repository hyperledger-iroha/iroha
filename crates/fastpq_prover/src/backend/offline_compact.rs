//! Fixed quantity-artifact production and verification for offline callers.
//!
//! The two routes verify complete ordered bundles under the fixed six-lane compact V1
//! implementation. The caller supplies independent expected public inputs and AXT
//! context; artifact bytes cannot select another value domain or protocol.
//! Success establishes mathematical consistency with those expectations, not
//! their authority, ledger finality, replay admission or production qualification.
//! Proving uses explicit work limits and constructs one segment at a time.
//! No profile registry or production ingress is enabled by this module.
//! TODO: Complete independent protocol qualification and authenticated admission.

use iroha_data_model::{
    fastpq::{
        FastpqArtifactIdentityDescriptionV1, FastpqAxtPreProofMirrorsV1, FastpqAxtPublicMetadataV1,
        FastpqCompactArtifactDecodeError, FastpqCompactArtifactDecodeLimits,
        FastpqCompactProfileIdV1, FastpqPublicInputs, FastpqPublicTransferStatementV1,
    },
    nexus::{AxtFastpqBinding, AxtRemoteSpendClaimV1},
    privacy::GoldilocksDigest384V1,
};
use norito::core::DecodeLimits;

use super::{
    compact_bundle::BundleLimits,
    compact_model_statement::candidate_artifact::{self, ArtifactError, ArtifactLimits},
    compact_public_api::AxtVerificationContext,
};
use crate::{
    Error, VerifyLimits,
    axt_binding::{AxtProofContextMirrors, AxtPublicMetadataBytes},
    gadgets::public_transfer_statement::{PublicTransferLimits, TransferSmtBuildLimits},
    proof::PublicIO,
};

/// Independently expected public inputs, ordering and complete canonical statement identity.
///
/// Obtain these values from the surrounding application's trusted context. Copying
/// them from the artifact being checked does not authenticate that artifact.
/// Compute the statement digest from the independently authenticated complete
/// statement as `Hash::new(norito::encode_canonical(statement)?)`. Its canonical
/// frame includes every transcript header and occurrence. The caller owns the
/// provenance of that expected digest; the verifier cannot establish it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedStatement {
    /// Expected dataspace, slot, touched roots, permission and transaction commitments.
    pub inputs: FastpqPublicInputs,
    /// Independently expected ordering commitment for the complete statement.
    pub ordering_hash: [u8; 32],
    /// Expected hash of the complete canonical model public-statement frame.
    pub public_statement_digest: [u8; 32],
}

impl ExpectedStatement {
    pub(super) fn internal(self) -> PublicIO {
        PublicIO {
            dsid: self.inputs.dsid,
            slot: self.inputs.slot,
            old_root: self.inputs.old_root,
            new_root: self.inputs.new_root,
            perm_root: self.inputs.perm_root,
            tx_set_hash: self.inputs.tx_set_hash,
            ordering_hash: self.ordering_hash,
        }
    }
}

/// Complete independent AXT expectations borrowed from the surrounding caller.
///
/// `None` and `Some(&[])` remote preimages remain distinct. This type does not
/// authenticate bindings, signatures, expiry at use, permissions or source finality.
#[derive(Debug, Clone, Copy)]
pub struct ExpectedAxtContext<'a> {
    /// Expected canonical binding, not the artifact's advertised binding.
    pub binding: &'a AxtFastpqBinding,
    /// Expected exact canonical public metadata encodings.
    pub metadata: &'a FastpqAxtPublicMetadataV1,
    /// Expected pre-proof outer values that must match the public metadata.
    pub mirrors: FastpqAxtPreProofMirrorsV1,
    /// Complete independently expected remote-spend preimages when applicable.
    pub remote_spend_claims: Option<&'a [AxtRemoteSpendClaimV1]>,
}

impl<'a> ExpectedAxtContext<'a> {
    pub(super) fn internal(self) -> AxtVerificationContext<'a> {
        AxtVerificationContext {
            binding: self.binding,
            metadata: AxtPublicMetadataBytes {
                parameter: &self.metadata.parameter,
                entry_hash: &self.metadata.entry_hash,
                committed_amount: self
                    .metadata
                    .committed_amount
                    .as_ref()
                    .map(|v| v.as_slice()),
                expiry_slot: &self.metadata.expiry_slot,
                manifest_root: &self.metadata.manifest_root,
                da_commitment: &self.metadata.da_commitment,
            },
            mirrors: AxtProofContextMirrors {
                dsid: self.mirrors.dsid,
                manifest_root: self.mirrors.manifest_root,
                da_commitment: self.mirrors.da_commitment,
                committed_amount: self.mirrors.committed_amount,
                expiry_slot: self.mirrors.expiry_slot,
            },
            remote_spend_claims: self.remote_spend_claims,
        }
    }
}

/// Explicit limits for a complete ordered bundle in addition to every child cap.
///
/// No defaults or production workload profile are selected by this policy.
#[derive(Debug, Clone, Copy)]
pub struct BundleVerificationLimits {
    /// Maximum complete delta occurrences, each represented by one segment.
    pub max_segments: usize,
    /// Maximum canonical carrier bytes, including its nested raw frames.
    pub max_wire_bytes: usize,
    /// Maximum sum of canonical child proof frame lengths.
    pub max_total_segment_bytes: usize,
    /// Maximum sum of statement bytes absorbed by all segment transcripts.
    pub max_total_statement_bytes: usize,
    /// Maximum cumulative query count, checked before any child verification.
    pub max_total_queries: usize,
    /// Maximum cumulative Norito allocation charges for carrier and child decoding.
    pub max_total_decode_allocation_charges: usize,
    /// Per-segment proof and verifier-work ceilings.
    pub segment: VerifyLimits,
}

impl BundleVerificationLimits {
    pub(super) fn internal(self) -> BundleLimits {
        BundleLimits {
            max_segments: self.max_segments,
            max_wire_bytes: self.max_wire_bytes,
            max_total_segment_bytes: self.max_total_segment_bytes,
            max_total_statement_bytes: self.max_total_statement_bytes,
            max_total_queries: self.max_total_queries,
            max_total_decode_allocation_charges: self.max_total_decode_allocation_charges,
            segment: self.segment,
        }
    }
}

/// Explicit verification ceilings at every decoded layer and across the whole request.
///
/// One enclosing Norito scope spans transport, carrier and all child frames.
/// A stricter enclosing caller scope is retained. Decoder allocation charges are
/// accounting units, not a bound on process memory or public-preparation scratch.
#[derive(Debug, Clone, Copy)]
pub struct VerificationLimits {
    /// Canonical model transport limits, including the opaque carrier frame.
    pub transport: FastpqCompactArtifactDecodeLimits,
    /// Complete model public-fact preparation limits.
    pub public_statement: PublicTransferLimits,
    /// Complete ordered carrier and per-child verification limits.
    pub bundle: BundleVerificationLimits,
    /// Norito allocation charges allowed for each complete child proof decode.
    pub max_segment_decode_allocation_charges: usize,
    /// Shared transport, carrier and child decoding budget for the whole request.
    pub total_decode: DecodeLimits,
}

impl VerificationLimits {
    fn internal(self) -> ArtifactLimits {
        ArtifactLimits {
            transport: self.transport,
            public_statement: self.public_statement,
            bundle: self.bundle.internal(),
            max_segment_decode_allocation_charges: self.max_segment_decode_allocation_charges,
            total_decode: self.total_decode,
        }
    }
}

/// Explicit additional work policy for producing a complete quantity artifact.
///
/// No defaults or node admission profile are selected. At most one call to either
/// producer runs at a time in this process; a concurrent call returns `Busy`.
/// Private witnesses are bounded separately, and physical columns are constructed
/// and dropped one segment at a time. Verification limits also constrain output.
#[derive(Debug, Clone, Copy)]
pub struct ProvingLimits {
    /// Limits on private touched-tree construction and retained path material.
    pub private_smt: TransferSmtBuildLimits,
    /// Maximum total base trace cells across the ordered segments.
    pub max_total_trace_cells: usize,
    /// Maximum conservative structural working-payload charge for one segment.
    ///
    /// This is an accounting charge, not reserved memory or an RSS ceiling. It
    /// sums trace, fixed-column, commitment, FRI and bounded evaluator buffers;
    /// allocator metadata, runtime thread stacks, cold hash-DAG compilation and
    /// unrelated process memory are excluded. Public inputs, retained child
    /// frames, private SMT work and decoder charges have separate limits.
    pub max_segment_charge_bytes: usize,
}

/// A producer failure; no partial artifact is returned.
#[derive(Debug, thiserror::Error)]
pub enum ProvingError {
    /// Another ordinary or AXT quantity artifact is currently being produced.
    #[error("a compact quantity artifact producer is already running")]
    Busy,
    /// Public expectations, proving work, encoding or output policy failed.
    #[error(transparent)]
    Prove(#[from] Error),
    /// The completed artifact failed the caller's public verification policy.
    #[error(transparent)]
    Verify(#[from] VerificationError),
}

/// Produce a canonical ordinary quantity bundle under independent expectations.
///
/// Even one segment uses the complete bundle relation. All original quantities,
/// identities and occurrences are checked; derived private roots must equal the
/// expected roots. Returned bytes have passed the public offline verifier below.
/// Conservative output preflight requires room for every valid query set, so a
/// budget may reject even if one particular proof would be smaller. Final decode
/// checks retain any stricter enclosing Norito budget. This grants no finality.
///
/// # Errors
/// Returns `Busy` for concurrent production, or rejects inconsistent public
/// facts, exceeded work/output limits, invalid roots or final verification failure.
pub fn prove_quantity_ordinary_artifact(
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    proving: ProvingLimits,
    verification: VerificationLimits,
) -> Result<Vec<u8>, ProvingError> {
    super::compact_quantity_producer::prove(statement, expected, None, proving, verification)
}

/// Produce a canonical AXT quantity bundle with complete independent AXT context.
///
/// The complete original binding, metadata, mirrors and remote-spend claims are
/// checked before trace work and bound into every segment. Source authentication,
/// expiry-at-use and issuer authorization remain the surrounding caller's duties.
/// Returned bytes have passed the same public AXT verifier exposed below.
///
/// # Errors
/// Returns `Busy` for concurrent production, or rejects public/AXT mismatches,
/// exceeded work/output limits, invalid roots or final verification failure.
pub fn prove_quantity_axt_artifact(
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    context: ExpectedAxtContext<'_>,
    proving: ProvingLimits,
    verification: VerificationLimits,
) -> Result<Vec<u8>, ProvingError> {
    super::compact_quantity_producer::prove(
        statement,
        expected,
        Some(context),
        proving,
        verification,
    )
}

/// Failure before an all-or-nothing offline verification result can be returned.
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    /// Canonical transport, fixed profile equality or transport caps failed.
    #[error(transparent)]
    Transport(#[from] FastpqCompactArtifactDecodeError),
    /// Caller equality, public facts, cumulative policy or a child proof failed.
    #[error(transparent)]
    Verify(#[from] Error),
}

impl From<ArtifactError> for VerificationError {
    fn from(error: ArtifactError) -> Self {
        match error {
            ArtifactError::Transport(error) => Self::Transport(error),
            ArtifactError::Verify(error) => Self::Verify(error),
        }
    }
}

/// Measured verification work summed over every successfully verified child.
///
/// These counters exclude outer transport overhead and public preparation, and
/// are not a process-memory bound or a source-execution budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationWork {
    /// Sum of canonical child proof frame bytes.
    pub proof_bytes: usize,
    /// Complete child transcript initializations after bounded preflight.
    pub transcripts: usize,
    /// Distinct complete row leaf hashes across all children.
    pub row_leaves: usize,
    /// Distinct mixed and quotient leaf hashes across all children.
    pub oracle_leaves: usize,
    /// Distinct binary FRI group leaves, including terminal leaves.
    pub fri_leaves: usize,
    /// Shared internal Merkle hashes after successful reconstruction.
    pub parent_hashes: usize,
    /// Relation evaluations at transcript query indices.
    pub air_evaluations: usize,
    /// Whole-terminal polynomial degree checks.
    pub terminal_degree_checks: usize,
}

/// Complete offline verification and recomputed artifact identity.
///
/// Private fields prevent constructing a success from advertised metadata. A
/// successful value grants no source authority, finality or ingress admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedArtifact {
    inner: candidate_artifact::VerifiedArtifact,
}

impl VerifiedArtifact {
    /// Independently expected public inputs checked by every complete relation.
    pub fn expected_statement(&self) -> ExpectedStatement {
        let io = self.inner.bundle().public_io();
        ExpectedStatement {
            inputs: FastpqPublicInputs {
                dsid: io.dsid,
                slot: io.slot,
                old_root: io.old_root,
                new_root: io.new_root,
                perm_root: io.perm_root,
                tx_set_hash: io.tx_set_hash,
            },
            ordering_hash: io.ordering_hash,
            public_statement_digest: self.inner.identity().public_statement_digest,
        }
    }

    /// Recomputed canonical content identity and complete ordered AIR commitments.
    pub fn identity(&self) -> &FastpqArtifactIdentityDescriptionV1 {
        self.inner.identity()
    }

    /// Complete verified AIR row roots in original segment order.
    pub fn air_row_roots(&self) -> &[GoldilocksDigest384V1] {
        self.inner.bundle().row_roots()
    }

    /// Number of fully verified delta occurrences.
    pub fn segments(&self) -> usize {
        self.inner.bundle().segments()
    }

    /// Actual canonical carrier frame length, excluding the outer model transport.
    pub fn bundle_frame_bytes(&self) -> usize {
        self.inner.bundle().wire_bytes()
    }

    /// Total canonical statement lengths absorbed across all child transcripts.
    pub fn statement_bytes(&self) -> usize {
        self.inner.bundle().statement_bytes()
    }

    /// Measured work, returned only after every child succeeds.
    pub fn work(&self) -> VerificationWork {
        let work = self.inner.bundle().work();
        VerificationWork {
            proof_bytes: work.proof_bytes,
            transcripts: work.transcripts,
            row_leaves: work.row_leaves,
            oracle_leaves: work.oracle_leaves,
            fri_leaves: work.fri_leaves,
            parent_hashes: work.parent_hashes,
            air_evaluations: work.air_evaluations,
            terminal_degree_checks: work.terminal_degree_checks,
        }
    }
}

/// Return the exact existing fixed quantity candidate profile identifier.
///
/// This equality filter does not register or qualify a production profile. A
/// count-one quantity bundle retains its bundle schema and relation identity.
pub fn quantity_profile_id() -> FastpqCompactProfileIdV1 {
    candidate_artifact::quantity_diagnostic_profile_id()
}

/// Verify a complete ordinary quantity artifact against independent caller inputs.
///
/// The entry point fixes the quantity value domain, ordinary semantics and the six-lane
/// candidate. It does not accept a generic AIR, profile selector or private witness.
///
/// # Errors
/// Rejects canonical transport or policy failures, mismatched expected inputs,
/// mismatched complete statement digest, invalid public facts, zero/malformed
/// bundles, or any invalid child.
pub fn verify_quantity_ordinary_artifact(
    bytes: &[u8],
    expected: ExpectedStatement,
    limits: VerificationLimits,
) -> Result<VerifiedArtifact, VerificationError> {
    candidate_artifact::verify_bound_quantity_ordinary_artifact(
        bytes,
        &expected.internal(),
        expected.public_statement_digest,
        limits.internal(),
    )
    .map(|inner| VerifiedArtifact { inner })
    .map_err(Into::into)
}

/// Verify a complete AXT quantity artifact against every independent expectation.
///
/// All public metadata, binding, outer mirrors and remote preimages are supplied
/// separately from the advertised artifact; the ordinary route cannot bypass them.
///
/// # Errors
/// Rejects canonical transport or policy failures, any caller-context mismatch,
/// mismatched complete statement digest, invalid public/AXT facts, or any missing,
/// malformed or invalid child.
pub fn verify_quantity_axt_artifact(
    bytes: &[u8],
    expected: ExpectedStatement,
    context: ExpectedAxtContext<'_>,
    limits: VerificationLimits,
) -> Result<VerifiedArtifact, VerificationError> {
    candidate_artifact::verify_bound_quantity_axt_artifact(
        bytes,
        &expected.internal(),
        expected.public_statement_digest,
        context.internal(),
        limits.internal(),
    )
    .map(|inner| VerifiedArtifact { inner })
    .map_err(Into::into)
}
