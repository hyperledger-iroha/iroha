//! Sequential quantity artifact production from independent public expectations.
//!
//! Public facts, explicit work policy and private touched-tree roots are checked
//! before any physical trace is expanded. Only one segment is expanded at a time.
//! The returned canonical artifact must pass the same public offline verifier.

use std::sync::{Mutex, MutexGuard, TryLockError};

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqAxtCompactArtifactV1, FastpqOrdinaryCompactArtifactV1, FastpqPublicTransferStatementV1,
    FastpqQuantityUnits, TransferSmtWitness,
};
use norito::NoritoSerialize;

use super::{
    compact_axt_batch::AxtTransferBatch,
    compact_axt_context::preflight_context,
    compact_bundle::{self, AxtBundleWire, BundleWire},
    compact_model_statement::with_prepared_quantity_statement,
    compact_protocol::{
        FixedAir,
        shared_openings::{preflight_prover, prove_shared},
    },
    compact_prover_resources::check_segment_charge,
    compact_public_batch::{BatchContextLimits, PublicTransferBatch, preflight_prepared},
    offline_compact::{
        ExpectedAxtContext, ExpectedStatement, ProvingError, ProvingLimits, VerificationLimits,
    },
};
use crate::{
    Error, ProofSemantics, Result,
    axt_binding::{validate_axt_public_metadata, validate_axt_public_transfer_facts},
    gadgets::{
        compact_smt_air::{
            COLUMN_COUNT, PATH_LEVELS, PHYSICAL_ROW_COUNT, PublicStatement, SmtWitness,
        },
        compact_trace_columns::smt_row_cells,
        public_transfer_statement::PreparedPublicTransfers,
    },
};

// Exact valid maximal-byte shape at the fixed 375-query geometry, not the
// larger invalid loose decode shape. The engine independently checks this cap.
const SHARED_FRAME_BOUND: usize = 4_279_877;
static PRODUCER: Mutex<()> = Mutex::new(());

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

fn mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("producer work count overflows"))
}

fn invalid(details: &'static str) -> Error {
    Error::TransferInvariant {
        details: details.to_owned(),
    }
}

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
    let bundle = verification.bundle;
    check("max_bundle_segments", count, bundle.max_segments)?;
    check("max_queries", 375, bundle.segment.max_queries)?;
    check(
        "max_bundle_queries",
        mul(count, 375)?,
        bundle.max_total_queries,
    )?;
    check(
        "max_proof_bytes",
        SHARED_FRAME_BOUND,
        bundle.segment.max_proof_bytes,
    )?;
    check(
        "max_bundle_segment_bytes",
        mul(count, SHARED_FRAME_BOUND)?,
        bundle.max_total_segment_bytes,
    )?;
    check(
        "max_compact_prover_trace_cells",
        mul(count, mul(COLUMN_COUNT, PHYSICAL_ROW_COUNT)?)?,
        proving.max_total_trace_cells,
    )?;
    check_segment_charge(0, SHARED_FRAME_BOUND, proving.max_segment_charge_bytes)?;
    decode_policy::preflight_decode_policy(count, verification)?;
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

enum Artifact {
    Ordinary(FastpqOrdinaryCompactArtifactV1),
    Axt(FastpqAxtCompactArtifactV1),
}

impl Artifact {
    fn new(
        statement: &FastpqPublicTransferStatementV1,
        axt: Option<ExpectedAxtContext<'_>>,
    ) -> Self {
        let profile_id = super::offline_compact::quantity_profile_id();
        match axt {
            None => Self::Ordinary(FastpqOrdinaryCompactArtifactV1 {
                profile_id,
                statement: statement.clone(),
                bundle_frame: Vec::new(),
            }),
            Some(axt) => Self::Axt(FastpqAxtCompactArtifactV1 {
                profile_id,
                statement: statement.clone(),
                binding: axt.binding.clone(),
                metadata: axt.metadata.clone(),
                mirrors: axt.mirrors,
                remote_spend_claims: axt.remote_spend_claims.map(<[_]>::to_vec),
                bundle_frame: Vec::new(),
            }),
        }
    }

    fn preflight(&self, count: usize, limits: VerificationLimits) -> Result<()> {
        // Each scalar/sequence field has at most ten compact prefix bytes.
        // These deliberately conservative framing allowances avoid allocating
        // dummy proof frames while retaining the exact final codec checks.
        let roots = count
            .checked_sub(1)
            .ok_or_else(|| invalid("quantity producer requires a nonempty complete bundle"))?;
        let carrier = add(
            1024,
            add(mul(roots, 64)?, mul(count, SHARED_FRAME_BOUND + 32)?)?,
        )?;
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
        let empty = match self {
            Self::Ordinary(value) => norito::core::encoded_frame_len(value)?,
            Self::Axt(value) => norito::core::encoded_frame_len(value)?,
        };
        check(
            "max_compact_producer_artifact_bytes",
            add(empty, add(carrier, 32)?)?,
            limits.transport.max_wire_bytes,
        )
    }

    fn finish(mut self, bundle: Vec<u8>, limits: VerificationLimits) -> Result<Vec<u8>> {
        check(
            "max_compact_producer_bundle_bytes",
            bundle.len(),
            limits.transport.max_bundle_frame_bytes,
        )?;
        match &mut self {
            Self::Ordinary(value) => {
                value.bundle_frame = bundle;
                encode_artifact(value, limits)
            }
            Self::Axt(value) => {
                value.bundle_frame = bundle;
                encode_artifact(value, limits)
            }
        }
    }
}

fn encode_artifact<T: NoritoSerialize>(value: &T, limits: VerificationLimits) -> Result<Vec<u8>> {
    check(
        "max_compact_producer_artifact_bytes",
        norito::core::encoded_frame_len(value)?,
        limits.transport.max_wire_bytes,
    )?;
    Ok(norito::encode_canonical(value)?)
}

fn columns(statement: &PublicStatement, pair: &[TransferSmtWitness; 2]) -> Result<Vec<Vec<u64>>> {
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
    let witness = SmtWitness::from_inputs(statement, &siblings)
        .ok_or_else(|| {
            invalid("quantity producer private witness does not satisfy its public ports")
        })?
        .into_physical();
    let mut columns: Vec<Vec<u64>> = (0..COLUMN_COUNT)
        .map(|_| Vec::with_capacity(PHYSICAL_ROW_COUNT))
        .collect();
    for row in witness.rows() {
        for (column, value) in columns.iter_mut().zip(smt_row_cells(row)) {
            column.push(value);
        }
    }
    Ok(columns)
}

fn segments<R: FixedAir>(
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
    // Validate every complete statement before expanding even the first witness.
    for ordinal in 0..statements.len() {
        let relation = relation(ordinal)?;
        preflight_prover(&relation, verification.bundle.segment)?;
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
        let proof = prove_shared(&relation, &columns, verification.bundle.segment)?;
        drop(columns);
        let length = norito::core::encoded_frame_len(&proof)?;
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
        frames.push(norito::encode_canonical(&proof)?);
    }
    Ok(frames)
}

fn prepare_and_prove(
    prepared: &PreparedPublicTransfers<'_, FastpqQuantityUnits>,
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    axt: Option<ExpectedAxtContext<'_>>,
    proving: ProvingLimits,
    verification: VerificationLimits,
) -> Result<Vec<u8>> {
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
    if let Some(axt) = axt {
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
    artifact.preflight(count, verification)?;
    // Unlike the diagnostic materializer, this does not replace supplied roots.
    let private = prepared.build_smt_witnesses(proving.private_smt)?;
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
    let bundle = if let Some(axt) = axt {
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
    } else {
        let batch = PublicTransferBatch::new(prepared, &expected.internal(), &roots, contexts)?;
        let frames = segments(
            batch.statements(),
            private.pairs(),
            |i| batch.segment(i),
            proving,
            verification,
        )?;
        compact_bundle::encode_wire(
            &BundleWire {
                version: 1,
                intermediate_roots: roots,
                segments: frames,
            },
            count,
            verification.bundle.internal(),
        )?
    };
    drop(private);
    artifact.finish(bundle, verification)
}

pub(super) fn prove(
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    axt: Option<ExpectedAxtContext<'_>>,
    proving: ProvingLimits,
    verification: VerificationLimits,
) -> std::result::Result<Vec<u8>, ProvingError> {
    let _exclusive = acquire(&PRODUCER)?;
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    check_statement(statement, expected, proving, verification)?;
    let semantics = if axt.is_some() {
        ProofSemantics::AxtTransferClaim
    } else {
        ProofSemantics::StateTransition
    };
    let bytes = with_prepared_quantity_statement(
        statement,
        &expected.internal(),
        semantics,
        verification.public_statement,
        |prepared| prepare_and_prove(prepared, statement, expected, axt, proving, verification),
    )?;
    match axt {
        Some(context) => {
            super::offline_compact::verify_quantity_axt_artifact(
                &bytes,
                expected,
                context,
                verification,
            )?;
        }
        None => {
            super::offline_compact::verify_quantity_ordinary_artifact(
                &bytes,
                expected,
                verification,
            )?;
        }
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "compact_quantity_producer/tests.rs"]
mod tests;
