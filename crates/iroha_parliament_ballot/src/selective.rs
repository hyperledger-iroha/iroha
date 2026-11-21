//! Selective verification of one anonymous Network input and its certified execution result.

use std::num::NonZeroU64;

use iroha_data_model::{
    block::execution_output::ExecutionOutputV1,
    prelude::HashOf,
    sumeragi_finality::{CertifiedTransactionInclusionV1, FinalityError, VerifiedSumeragiCommitV1},
    transaction::{TransactionEntrypoint, TransactionResult, error::TransactionRejectionReason},
};

use crate::ballot::{
    BlockBallotDecisionV1, BlockBallotStateV1, BlockInvalidReasonV1, VerificationUnavailableV1,
    decide_block_ballot_v1,
};

/// A recorded ballot could not be authenticated or reproduced.
#[derive(Debug, thiserror::Error)]
pub enum SelectiveBallotErrorV1 {
    /// The verified certificate belongs to another network or chain.
    #[error(transparent)]
    Finality(#[from] FinalityError),
    /// The evidence has the wrong kind, a substituted leaf, an invalid join or another outcome.
    #[error("invalid selective ballot evidence: {0}")]
    Evidence(&'static str),
    /// A committed input fails a consensus ballot check.
    #[error(transparent)]
    InvalidBallot(#[from] BlockInvalidReasonV1),
    /// The local verifier could not complete the ballot checks.
    #[error(transparent)]
    Unavailable(#[from] VerificationUnavailableV1),
    /// The canonical hash of an evidence leaf could not be computed.
    #[error(transparent)]
    Encoding(#[from] norito::Error),
}

/// Authenticate a ballot and its exact Network result, then re-decide the ballot with the
/// ordinary block verifier. The caller independently authenticates the global certificate
/// and the ballot state at this input, including the seal, window and earlier nullifiers.
/// A successful return describes only this occurrence. Corpus completeness requires full replay.
///
/// # Errors
/// A foreign certificate, a substituted entry or output, a different source index, an
/// invalid ballot, local verification failure, or disagreement with the certified result.
pub fn verify_selective_ballot_v1(
    certificate: &VerifiedSumeragiCommitV1,
    chain_id: &str,
    state: &impl BlockBallotStateV1,
    input: &TransactionEntrypoint,
    inclusion: &CertifiedTransactionInclusionV1,
) -> Result<BlockBallotDecisionV1, SelectiveBallotErrorV1> {
    use SelectiveBallotErrorV1::Evidence;

    certificate.verify_global_scope(state.network_id(), chain_id)?;
    let height = NonZeroU64::new(certificate.height())
        .ok_or(SelectiveBallotErrorV1::Evidence("zero certificate height"))?;
    let execution = certificate.execution();

    let TransactionEntrypoint::AnonymousBallot(carrier) = input else {
        return Err(Evidence("expected an anonymous ballot"));
    };
    let (Some(inputs), Some(outputs)) = (
        execution.transaction_input_commitment,
        execution.transaction_output_commitment,
    ) else {
        return Err(Evidence("missing certified input or output commitment"));
    };
    let row = match &inclusion.output {
        ExecutionOutputV1::Network(row) => row,
        ExecutionOutputV1::NetworkWithLifecycle(row)
            if row.trigger_lifecycle.effects().is_empty() =>
        {
            &row.execution
        }
        _ => {
            return Err(Evidence(
                "expected an ordinary ballot Network output without lifecycle effects",
            ));
        }
    };
    // Governance rows can precede the Network rows. Each proof binds its own actual leaf;
    // the authenticated Network row joins the ballot by its explicit input index.
    if row.input_index != inclusion.entry_proof.leaf_index()
        || !inclusion
            .entry_proof
            .verify(&HashOf::try_new(input)?, &inputs)
        || !inclusion
            .output_proof
            .verify(&HashOf::try_new(&inclusion.output)?, &outputs)
    {
        return Err(Evidence(
            "certified input/output membership or source index differs",
        ));
    }

    let decision = decide_block_ballot_v1(state, carrier.as_bytes(), height, row.input_index)?;
    let expected = match &decision {
        BlockBallotDecisionV1::Accepted(_) => Ok(Vec::new()),
        BlockBallotDecisionV1::Rejected(reason) => {
            Err(TransactionRejectionReason::AnonymousBallot(*reason))
        }
        BlockBallotDecisionV1::BlockInvalid(reason) => return Err(reason.clone().into()),
    };
    if row.result != TransactionResult::new(expected) || !row.completions.is_empty() {
        return Err(Evidence(
            "certified Network output differs from the ballot decision",
        ));
    }
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_data_model::{
        NetworkId,
        block::{
            BlockSignatures,
            builder::BlockBuilder,
            execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
            output_budget::ExecutionOutputLimits,
        },
        governance::anonymous_ballot::{
            BallotScopeV1, CastingClockEstablishedV1, GovernanceBlockStartEventV1,
            GovernanceBlockStartOutputV1, GovernanceOutputAnchorV1, NullifierV1,
        },
        sumeragi_finality::{SumeragiCommitCertificateV1, SumeragiCommitVerifierV1},
        testing::native_finality::NativeFinalityFixture,
    };

    use super::*;
    use crate::context::ScopedBallotContextV1;

    struct State {
        context: Arc<ScopedBallotContextV1>,
        open: bool,
    }

    impl BlockBallotStateV1 for State {
        fn network_id(&self) -> NetworkId {
            *self.context.network_id()
        }
        fn scope_context(&self, scope: &BallotScopeV1) -> Option<Arc<ScopedBallotContextV1>> {
            (scope == self.context.scope()).then(|| Arc::clone(&self.context))
        }
        fn casting_open(&self, _: &BallotScopeV1) -> bool {
            self.open
        }
        fn nullifier_accepted(&self, _: &BallotScopeV1, _: &NullifierV1) -> bool {
            false
        }
    }

    #[test]
    fn verified_ballot_joins_its_certified_output_after_the_governance_prelude() {
        let mut fixture = NativeFinalityFixture::start_at("parliament-selective", 1000);
        let (context, envelope) = crate::test_support::signed_fixture_ballot(fixture.network_id());
        let header = fixture.next_header();
        let mut builder = BlockBuilder::new(header);
        builder.push_anonymous_ballot(envelope.clone());
        let mut block = builder.build(BlockSignatures::default());
        let at = header.creation_time_ms;
        let clock = ExecutionOutputV1::GovernanceBlockStart(GovernanceBlockStartOutputV1 {
            anchor: GovernanceOutputAnchorV1::CastingClockPrelude {
                height: header.height().get(),
                timestamp_ms: at,
            },
            event: GovernanceBlockStartEventV1::CastingClockEstablished(
                CastingClockEstablishedV1 {
                    scope: *context.scope(),
                    casting_window_ms: 1000,
                    closing_ms: at + 1000,
                },
            ),
        });
        let output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index: 0,
            result: TransactionResult::new(Ok(vec![])),
            completions: vec![],
        });
        block
            .set_execution_outputs(
                vec![clock, output.clone()],
                0,
                Default::default(),
                vec![],
                Default::default(),
                Default::default(),
                &ExecutionOutputLimits {
                    max_outputs: 2,
                    max_output_bytes: 64 * 1024,
                    max_total_output_bytes: 128 * 1024,
                    max_executed_wire_bytes: 1024 * 1024,
                },
            )
            .unwrap();
        let inclusion = CertifiedTransactionInclusionV1 {
            entry_proof: block.network_input_proof(0).unwrap(),
            output,
            output_proof: block.output_proof(1).unwrap(),
        };
        let wrong_leaf = block.output_proof(0).unwrap();
        let mut reader = SumeragiCommitVerifierV1::new(&fixture.verifier()).unwrap();
        let proof = fixture.certify(block);
        let verified = fixture.verifier().verify_retained_decision(&proof).unwrap();
        let cert = reader
            .verify(&SumeragiCommitCertificateV1::from_verified(&verified).unwrap())
            .unwrap();
        let mut state = State {
            context,
            open: true,
        };
        let input = TransactionEntrypoint::AnonymousBallot(envelope);
        assert!(matches!(
            verify_selective_ballot_v1(&cert, fixture.chain_id(), &state, &input, &inclusion),
            Ok(BlockBallotDecisionV1::Accepted(_))
        ));
        assert!(
            verify_selective_ballot_v1(&cert, "foreign-chain", &state, &input, &inclusion).is_err()
        );
        let mut changed = inclusion.clone();
        changed.output_proof = wrong_leaf;
        assert!(
            verify_selective_ballot_v1(&cert, fixture.chain_id(), &state, &input, &changed)
                .is_err()
        );
        state.open = false;
        assert!(
            verify_selective_ballot_v1(&cert, fixture.chain_id(), &state, &input, &inclusion)
                .is_err()
        );
    }
}
