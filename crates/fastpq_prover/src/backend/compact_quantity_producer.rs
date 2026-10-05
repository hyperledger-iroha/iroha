//! Sequential quantity artifact production from independent public expectations.
//!
//! Public facts, explicit work policy and private touched-tree roots are checked
//! before any physical trace is expanded. Only one segment is expanded at a time.
//! The returned canonical artifact must pass the same public offline verifier.

use iroha_allocation::{AllocationBudget, AllocationReservation};

use std::sync::{Mutex, MutexGuard, TryLockError};

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqAxtCompactArtifactV1, FastpqPublicTransferStatementV1, FastpqQuantityUnits,
    TransferSmtWitness,
};
use norito::NoritoSerialize;

use super::{
    air::q77::{ProducerLimits, VerifierLimits},
    compact_axt_batch::AxtTransferBatch,
    compact_axt_context::preflight_context,
    compact_bundle::{self, AxtBundleWire},
    compact_model_statement::with_prepared_quantity_statement,
    compact_prover_resources::check_segment_charge,
    compact_public_batch::{BatchContextLimits, preflight_prepared},
    deep_engine,
    deep_proof::MAX_FRAME_BYTES,
    deep_prover::ProducerPlan,
    deep_relation::DeepRelation,
    deep_trace_source::OwnedTraceSource,
    offline_compact::{
        ExpectedAxtContext, ExpectedStatement, ProvingError, ProvingLimits,
        QUANTITY_SHARED_FRAME_BOUND as SHARED_FRAME_BOUND, VerificationLimits,
        quantity_artifact_resources,
    },
};
use crate::{
    Error, ProofSemantics, Result,
    axt_binding::{validate_axt_public_metadata, validate_axt_public_transfer_facts},
    gadgets::{
        compact_smt_air::{PATH_LEVELS, PublicStatement, SmtWitness},
        public_transfer_statement::PreparedPublicTransfers,
    },
};

static PRODUCER: Mutex<()> = Mutex::new(());

/// Hold the original producer mutex without constructing private proving work.
#[cfg(test)]
pub(super) fn hold_producer_for_test() -> MutexGuard<'static, ()> {
    // As in `acquire`, this mutex protects no shared witness state.
    PRODUCER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[path = "compact_quantity_producer/decode_policy.rs"]
mod decode_policy;

fn acquire(mutex: &Mutex<()>) -> std::result::Result<MutexGuard<'_, ()>, ProvingError> {
    match mutex.try_lock() {
        Ok(guard) => Ok(guard),
        // The mutex protects no state: a failed request leaves no shared witness.
        Err(TryLockError::Poisoned(error)) => Ok(error.into_inner()),
        Err(TryLockError::WouldBlock) => Err(ProvingError::Busy),
    }
}

fn check(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        return Err(Error::VerifierLimitExceeded { limit, actual, max });
    }
    Ok(())
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invalid("producer byte count overflows"))
}

#[cfg(test)]
fn mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("producer work count overflows"))
}

fn invalid(details: &'static str) -> Error {
    Error::TransferInvariant {
        details: details.to_owned(),
    }
}

#[allow(
    clippy::large_types_passed_by_value,
    reason = "the 272-byte `Copy` policy is copied once per proving call; the sibling test \
              module calls this directly with owned policy values"
)]
fn check_statement(
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    proving: ProvingLimits,
    verification: VerificationLimits,
) -> Result<usize> {
    let public = verification.public_statement;
    check(
        "max_public_transfer_rows",
        statement.transitions.len(),
        public.max_rows,
    )?;
    check(
        "max_public_transfer_transcripts",
        statement.transcripts.len(),
        public.max_transcripts,
    )?;
    let mut count = 0;
    for claim in &statement.transcripts {
        count = add(count, claim.deltas.len())?;
        check("max_public_transfer_deltas", count, public.max_deltas)?;
    }
    if count == 0 {
        return Err(invalid(
            "quantity producer requires a nonempty complete bundle",
        ));
    }
    // Reject every known fixed-geometry/carrier deficit before canonical
    // statement encoding, public preparation or private-tree construction.
    quantity_artifact_resources(count, 0)?.check_proving_limits(proving, verification)?;
    decode_policy::preflight_decode_policy(count, &verification)?;
    // Canonical framing is measured before allocating an encoded statement.
    // Public preparation below separately charges keys, paths, rows and claims.
    let length = norito::core::encoded_frame_len(statement)?;
    check(
        "max_compact_producer_statement_bytes",
        length,
        public.max_public_bytes,
    )?;
    let digest: [u8; 32] = Hash::new(norito::encode_canonical(statement)?).into();
    if digest != expected.public_statement_digest {
        return Err(Error::PublicIoMismatch {
            field: "compact_public_statement_digest",
        });
    }
    Ok(count)
}

struct Artifact(FastpqAxtCompactArtifactV1);

impl Artifact {
    fn new(statement: &FastpqPublicTransferStatementV1, axt: ExpectedAxtContext<'_>) -> Self {
        Self(FastpqAxtCompactArtifactV1 {
            profile_id: super::offline_compact::quantity_profile_id(),
            statement: statement.clone(),
            binding: axt.binding.clone(),
            metadata: axt.metadata.clone(),
            mirrors: axt.mirrors,
            remote_spend_claims: axt.remote_spend_claims.map(<[_]>::to_vec),
            bundle_frame: Vec::new(),
        })
    }
    fn preflight(&self, count: usize, limits: &VerificationLimits) -> Result<()> {
        let carrier = quantity_artifact_resources(count, 0)?.maximum_bundle_frame_bytes;
        check(
            "max_bundle_wire_bytes",
            carrier,
            limits.bundle.max_wire_bytes,
        )?;
        check(
            "max_compact_producer_bundle_bytes",
            carrier,
            limits.transport.max_bundle_frame_bytes,
        )?;
        check(
            "max_compact_producer_artifact_bytes",
            add(norito::core::encoded_frame_len(&self.0)?, add(carrier, 32)?)?,
            limits.transport.max_wire_bytes,
        )
    }
    fn finish(mut self, bundle: Vec<u8>, limits: &VerificationLimits) -> Result<Vec<u8>> {
        check(
            "max_compact_producer_bundle_bytes",
            bundle.len(),
            limits.transport.max_bundle_frame_bytes,
        )?;
        self.0.bundle_frame = bundle;
        encode_artifact(&self.0, limits)
    }
}

fn encode_artifact<T: NoritoSerialize>(value: &T, limits: &VerificationLimits) -> Result<Vec<u8>> {
    check(
        "max_compact_producer_artifact_bytes",
        norito::core::encoded_frame_len(value)?,
        limits.transport.max_wire_bytes,
    )?;
    Ok(norito::encode_canonical(value)?)
}

fn columns(
    statement: &PublicStatement,
    pair: &[TransferSmtWitness; 2],
) -> Result<OwnedTraceSource> {
    for (update, witness) in statement.updates.iter().zip(pair) {
        if witness.path_bits != update.path.to_le_bytes() || witness.siblings.len() != PATH_LEVELS {
            return Err(invalid(
                "quantity producer private path differs from prepared public ports",
            ));
        }
    }
    let siblings = core::array::from_fn(|update| {
        core::array::from_fn(|level| {
            let bytes = pair[update].siblings[level];
            core::array::from_fn(|limb| {
                u32::from_le_bytes(core::array::from_fn(|byte| bytes[limb * 4 + byte]))
            })
        })
    });
    let witness = SmtWitness::from_inputs_guarded(statement, &siblings)
        .map(SmtWitness::into_physical_guarded)
        .ok_or_else(|| {
            invalid("quantity producer private witness does not satisfy its public ports")
        })?;
    OwnedTraceSource::from_rows(witness.rows())
}

/// Map the facade policies to the engine's typed verifier and producer limits.
///
/// These are the only limits the engine receives for a segment; the same
/// mapping is public as `air::q77::{VerifierLimits, ProducerLimits}::for_segment`.
fn engine_limits(
    proving: &ProvingLimits,
    verification: &VerificationLimits,
) -> (VerifierLimits, ProducerLimits) {
    let verifier = VerifierLimits::for_segment(
        verification.bundle.segment,
        verification.max_segment_decode_allocation_charges,
    );
    (verifier, ProducerLimits::for_segment(proving, &verifier))
}

#[allow(
    clippy::large_types_passed_by_value,
    reason = "the 272-byte `Copy` policy is copied once per proving call; the sibling test \
              module calls this directly with owned policy values"
)]
fn segments<R: DeepRelation>(
    statements: &[PublicStatement],
    private: &[[TransferSmtWitness; 2]],
    relation: impl Fn(usize) -> Result<R>,
    proving: ProvingLimits,
    verification: VerificationLimits,
) -> Result<Vec<Vec<u8>>> {
    if statements.len() != private.len() {
        return Err(invalid(
            "quantity producer private/public pair count differs",
        ));
    }
    let (verifier_limits, producer_limits) = engine_limits(&proving, &verification);
    // Validate every complete statement before expanding even the first witness.
    for ordinal in 0..statements.len() {
        let relation = relation(ordinal)?;
        deep_engine::preflight(&relation, MAX_FRAME_BYTES, verifier_limits)?;
        ProducerPlan::new(&relation, producer_limits)?;
        check_segment_charge(
            relation.statement_bytes().len(),
            SHARED_FRAME_BOUND,
            proving.max_segment_charge_bytes,
        )?;
    }
    let mut frames = Vec::with_capacity(statements.len());
    let mut total = 0;
    for (ordinal, (statement, private)) in statements.iter().zip(private).enumerate() {
        let relation = relation(ordinal)?;
        let columns = columns(statement, private)?;
        let proof = ProducerPlan::new(&relation, producer_limits)?
            .build(columns, &mut rand::rngs::OsRng)?;
        let length = proof.len();
        check(
            "max_proof_bytes",
            length,
            verification.bundle.segment.max_proof_bytes,
        )?;
        total = add(total, length)?;
        check(
            "max_bundle_segment_bytes",
            total,
            verification.bundle.max_total_segment_bytes,
        )?;
        frames.push(proof);
    }
    Ok(frames)
}

#[allow(
    clippy::too_many_arguments,
    clippy::large_types_passed_by_value,
    reason = "independent statement/context/work policies and original pool/reservation are explicit; the 272-byte `Copy` policy is copied once per proving call; the sibling test \
              module calls this directly with owned policy values"
)]
fn prepare_and_prove(
    prepared: &PreparedPublicTransfers<'_, FastpqQuantityUnits>,
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    axt: ExpectedAxtContext<'_>,
    proving: ProvingLimits,
    verification: VerificationLimits,
    budget: &AllocationBudget,
    reservation: &mut AllocationReservation,
) -> Result<Vec<u8>> {
    if !reservation.belongs_to(budget) {
        return Err(Error::AllocationForeignPool);
    }
    let count = prepared.pairs().len();
    check(
        "max_transitions",
        prepared.transitions().len(),
        verification.bundle.segment.max_transitions,
    )?;
    check(
        "max_batch_bytes",
        prepared.work().public_bytes,
        verification.bundle.segment.max_batch_bytes,
    )?;
    let root_count = count
        .checked_sub(1)
        .ok_or_else(|| invalid("quantity producer requires a nonempty complete bundle"))?;
    let contexts = BatchContextLimits {
        max_segments: verification.bundle.max_segments,
        max_total_statement_bytes: verification.bundle.max_total_statement_bytes,
    };
    preflight_prepared(prepared, root_count, contexts)?;
    {
        let context = axt.internal();
        preflight_context(
            prepared,
            context.binding,
            context.metadata,
            context.remote_spend_claims,
        )?;
        validate_axt_public_transfer_facts(
            context.binding,
            context.metadata,
            prepared,
            context.remote_spend_claims,
        )?;
        validate_axt_public_metadata(context.binding, context.metadata, context.mirrors)?;
    }
    let artifact = Artifact::new(statement, axt);
    artifact.preflight(count, &verification)?;
    // Unlike the diagnostic materializer, this does not replace supplied roots.
    let tree_bytes = proving
        .private_smt
        .allocation_bytes(prepared.transitions().len(), prepared.keys().len())?;
    let mut tree_reservation = reservation.try_partition_bytes(tree_bytes)?;
    let private =
        prepared.build_smt_witnesses(proving.private_smt, budget, &mut tree_reservation)?;
    if private.pairs().len() != count {
        return Err(invalid(
            "quantity producer private/public pair count differs",
        ));
    }
    let roots: Vec<_> = private
        .pairs()
        .iter()
        .take(root_count)
        .map(|pair| pair[1].root_after)
        .collect();
    let bundle = {
        let batch = AxtTransferBatch::new(
            prepared,
            &expected.internal(),
            &roots,
            axt.internal(),
            contexts,
        )?;
        let frames = segments(
            batch.statements(),
            private.pairs(),
            |i| batch.segment(i),
            proving,
            verification,
        )?;
        compact_bundle::encode_axt_wire(
            &AxtBundleWire {
                version: 1,
                intermediate_roots: roots,
                segments: frames,
            },
            count,
            verification.bundle.internal(),
        )?
    };
    drop(private);
    artifact.finish(bundle, &verification)
}

pub(super) fn prove(
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    axt: ExpectedAxtContext<'_>,
    proving: ProvingLimits,
    verification: &VerificationLimits,
    budget: &AllocationBudget,
    reservation: &mut AllocationReservation,
) -> std::result::Result<Vec<u8>, ProvingError> {
    if !reservation.belongs_to(budget) {
        return Err(Error::AllocationForeignPool.into());
    }
    let _exclusive = acquire(&PRODUCER)?;
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    check_statement(statement, expected, proving, *verification)?;
    crate::digest384_batch::preflight_last_fields_execution(proving.digest_execution)?;
    let semantics = ProofSemantics::AxtTransferClaim;
    let bytes = with_prepared_quantity_statement(
        statement,
        &expected.internal(),
        semantics,
        verification.public_statement,
        |prepared| {
            prepare_and_prove(
                prepared,
                statement,
                expected,
                axt,
                proving,
                *verification,
                budget,
                reservation,
            )
        },
    )?;
    super::offline_compact::verify_quantity_axt_artifact(&bytes, expected, axt, *verification)?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "compact_quantity_producer/tests.rs"]
mod tests;

#[path = "compact_quantity_producer/execution_effect.rs"]
pub(super) mod execution_effect;
