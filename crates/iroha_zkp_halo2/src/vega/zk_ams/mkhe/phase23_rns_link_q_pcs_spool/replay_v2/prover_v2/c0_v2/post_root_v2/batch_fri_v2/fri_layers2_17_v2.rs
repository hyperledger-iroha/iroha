//! Bounded authenticated B1-to-B17 FRI continuation and equal-terminal binding.
use core::{array, convert::Infallible};
use std::path::Path;
use iroha_confidential_spool::ConfidentialSpoolChunkV1;
use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::{
    ProverFriQueriesV2, ProverFriRoundsReadyV2,
};
use super::*;
const FRI_CONTINUATION_LAYERS_V2: usize = 17;
const FRI_CONTINUATION_READ_BYTES_V2: u64 = 3_190_802_240;
const FRI_CONTINUATION_WRITE_BYTES_V2: u64 = 1_595_410_240;
const FRI_CONTINUATION_SEAL_READ_BYTES_V2: u64 = 1_595_410_240;
const FRI_CONTINUATION_ROOT_READ_BYTES_V2: u64 = 1_595_410_240;
const FRI_CONTINUATION_TOTAL_IO_BYTES_V2: u64 = 7_977_032_960;
const FRI_CONTINUATION_RETAINED_BYTES_V2: u64 = 1_595_410_240;
const FRI_ALL_LAYER_FILE_BYTES_V2: u64 = 6_381_586_240;
const FRI_WITH_PRIOR_RETAINED_BYTES_V2: u64 = 13_561_628_480;
const FRI_CONTINUATION_RECORDS_READ_V2: u64 = 197_220;
const FRI_CONTINUATION_RECORDS_WRITTEN_V2: u64 = 99_940;
const FRI_CONTINUATION_FOLD_OUTPUTS_V2: u64 = 99_613_960;
const FRI_CONTINUATION_LEAF_HASHES_V2: u64 = 262_140;
const FRI_CONTINUATION_NODE_HASHES_V2: u64 = 262_124;
const FRI_CONTINUATION_ROOTS_V2: u64 = 17;
const FRI_CONTINUATION_ALPHAS_V2: u64 = 6_460;
const FRI_CONTINUATION_QUERIES_V2: u64 = 160;
const FRI_END_TO_END_IO_BYTES_V2: u64 = 31_907_912_960;
const FRI_END_TO_END_OUTPUT_VALUES_V2: u64 = 398_458_120;
const FRI_END_TO_END_FOLD_VALUES_V2: u64 = 199_228_680;
const FRI_END_TO_END_LEAF_HASHES_V2: u64 = 1_048_572;
const FRI_END_TO_END_NODE_HASHES_V2: u64 = 1_048_554;
const FRI_CONTINUATION_PEAK_ROOT_HEAP_BYTES_V2: usize = 6_242_304;
const FRI_CONTINUATION_PEAK_FOLD_HEAP_BYTES_V2: usize = 49_152;
const FRI_CONTINUATION_TERMINAL_BYTES_V2: usize = 12_224;
const FRI_CONTINUATION_EXPLICIT_PEAK_BYTES_V2: usize = 12_599_296;
const AUTHENTICATED_FRI_REPLAY_COMPLETE_V2: bool = false;
const FRI_ALL_FOLDS_COMPLETE_V2: bool = false;
const FRI_TERMINAL_EQUALITY_BOUND_V2: bool = false;
const FRI_QUERIES_DERIVED_V2: bool = false;
const FRI_CONTINUATION_ZERO_KNOWLEDGE_BOUND_V2: bool = false;
const FRI_CONTINUATION_PROOF_EMITTED_V2: bool = false;
const FRI_CONTINUATION_RSS_ACCEPTED_V2: bool = false;
const FRI_CONTINUATION_RECEIPT_ACCEPTED_V2: bool = false;
const FRI_CONTINUATION_RELEASE_READY_V2: bool = false;
const _: () = {
    assert!(FRI_CONTINUATION_LAYERS_V2 == 17);
    assert!(
        FRI_CONTINUATION_TOTAL_IO_BYTES_V2
            == FRI_CONTINUATION_READ_BYTES_V2
                + FRI_CONTINUATION_WRITE_BYTES_V2
                + FRI_CONTINUATION_SEAL_READ_BYTES_V2
                + FRI_CONTINUATION_ROOT_READ_BYTES_V2
    );
    assert!(FRI_CONTINUATION_WRITE_BYTES_V2 == FRI_CONTINUATION_RETAINED_BYTES_V2);
    assert!(FRI_ALL_LAYER_FILE_BYTES_V2 == REPLAY_FRI_TOTAL_FILE_BYTES_V2);
    assert!(
        FRI_WITH_PRIOR_RETAINED_BYTES_V2
            == COMBINED_AUTHENTICATED_FILE_BYTES_V2 + FRI_ALL_LAYER_FILE_BYTES_V2
    );
    assert!(FRI_CONTINUATION_RECORDS_WRITTEN_V2 == 99_940);
    assert!(FRI_CONTINUATION_RECORDS_WRITTEN_V2 * 2 - 2_660 == FRI_CONTINUATION_RECORDS_READ_V2);
    assert!(FRI_CONTINUATION_ALPHAS_V2 == 17 * 380);
    assert!(FRI_CONTINUATION_ROOTS_V2 == 17);
    assert!(FRI_CONTINUATION_QUERIES_V2 == 160);
    assert!(
        FRI_END_TO_END_IO_BYTES_V2
            == BATCH_FRI0_TOTAL_IO_BYTES_V2
                + BATCH_FRI1_TOTAL_IO_BYTES_V2
                + FRI_CONTINUATION_TOTAL_IO_BYTES_V2
    );
    assert!(
        FRI_END_TO_END_OUTPUT_VALUES_V2
            == BATCH_FRI0_VALUES_V2 + BATCH_FRI1_VALUES_V2 + FRI_CONTINUATION_FOLD_OUTPUTS_V2
    );
    assert!(
        FRI_END_TO_END_FOLD_VALUES_V2 == BATCH_FRI1_VALUES_V2 + FRI_CONTINUATION_FOLD_OUTPUTS_V2
    );
    assert!(
        FRI_END_TO_END_LEAF_HASHES_V2
            == BATCH_FRI0_LEAF_HASHES_V2
                + BATCH_FRI1_LEAF_HASHES_V2
                + FRI_CONTINUATION_LEAF_HASHES_V2
    );
    assert!(
        FRI_END_TO_END_NODE_HASHES_V2
            == BATCH_FRI0_NODE_HASHES_V2
                + BATCH_FRI1_NODE_HASHES_V2
                + FRI_CONTINUATION_NODE_HASHES_V2
    );
    assert!(FRI_CONTINUATION_PEAK_ROOT_HEAP_BYTES_V2 == 6_225_920 + 16_384);
    assert!(FRI_CONTINUATION_PEAK_FOLD_HEAP_BYTES_V2 == 3 * 16_384);
    assert!(FRI_CONTINUATION_TERMINAL_BYTES_V2 == 2 * 6_080 + 64);
    assert!(FRI_CONTINUATION_EXPLICIT_PEAK_BYTES_V2 == POST_ROOT_PEAK_EXPLICIT_HEAP_BYTES_V2);
    assert!(!AUTHENTICATED_FRI_REPLAY_COMPLETE_V2);
    assert!(!FRI_ALL_FOLDS_COMPLETE_V2);
    assert!(!FRI_TERMINAL_EQUALITY_BOUND_V2);
    assert!(!FRI_QUERIES_DERIVED_V2);
    assert!(!FRI_CONTINUATION_ZERO_KNOWLEDGE_BOUND_V2);
    assert!(!FRI_CONTINUATION_PROOF_EMITTED_V2);
    assert!(!FRI_CONTINUATION_RSS_ACCEPTED_V2);
    assert!(!FRI_CONTINUATION_RECEIPT_ACCEPTED_V2);
    assert!(!FRI_CONTINUATION_RELEASE_READY_V2);
};
pub(in super::super) enum BatchFriContinuationAuthorityV2 {
    Production {
        authenticated_layers: Infallible,
        exact_folds: Infallible,
        equal_terminal: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
/// Non-authorizing owner of every rooted FRI file and the derived query schedule.
pub(in super::super) struct BatchFriTerminalPreparedV2 {
    accepted_c0: Option<QPcsC0StoredV2>,
    masks: Option<MaskSpoolSealedV2>,
    accepted_cq: Option<QPcsCqStoredV2>,
    accepted_fri0: Option<FriLayer0RootedV2>,
    accepted_fri_layers: [Option<FriLayerRootedV2>; FRI_CONTINUATION_LAYERS_V2],
    transcript: Option<ProverFriQueriesV2>,
    terminal: ZeroizingFriTerminalV2,
    terminal_replay_complete: Option<FriLayerReplayCompleteV2>,
    evaluations: ZeroizingEvaluationFrameV2,
    context: PublicSpoolContextV2,
    parameter_digest: [u8; 32],
    initial_root: [u8; 32],
    quotient_root: [u8; 32],
    layer0_root: [u8; 32],
}
impl BatchFriLayer1RootPreparedV2 {
    pub(in super::super) fn prepare_batch_fri_terminal_v2(
        self,
        directory: &Path,
        authority: BatchFriContinuationAuthorityV2,
    ) -> Result<canonical_proof_v2::BatchFriCanonicalProofPreparedV2, ProverPrerequisiteErrorV2>
    {
        match authority {
            BatchFriContinuationAuthorityV2::Production {
                authenticated_layers,
                exact_folds: _exact_folds,
                equal_terminal: _equal_terminal,
            } => match authenticated_layers {},
            #[cfg(test)]
            BatchFriContinuationAuthorityV2::TestOnly => {}
        }
        let prepared = prepare_batch_fri_terminal_operation_v2(self, directory)?;
        canonical_proof_v2::prepare_canonical_proof_quarantine_v2(prepared, directory)
    }
}
fn fold_round_into_layer_v2(
    directory: &Path,
    ready: ProverFriRoundsReadyV2,
    source: FriLayerRootedV2,
) -> Result<(FriLayerRootedV2, FriLayerRootedV2, ProverFriRoundsReadyV2), ProverPrerequisiteErrorV2>
{
    let root = source.root;
    let mut challenges = ready.bind_next_root_v2(root)?;
    let context = challenges.context_v2()?;
    let mut replay = source.begin_fold_replay_v2(context)?;
    let mut writer = FriLayerWriterV2::create_v2(directory, context, replay.layer0_binding_v2())?;
    for pair_block in 0..challenges.pair_blocks_v2() {
        for column in 0..REPLAY_COLUMNS_V2 {
            let pair = replay.read_next_pair_v2(pair_block, column)?;
            let mut output = ConfidentialSpoolChunkV1::new_zeroed_v1(
                u64::from(challenges.values_per_half_v2()) * 16,
            )?;
            challenges.fold_next_pair_v2(
                pair_block,
                column,
                pair.positive_v2(),
                pair.negative_v2(),
                output.as_mut_slice_v1(),
            )?;
            writer.push_next_v2(pair_block, column, output)?;
        }
    }
    let (source, replay_complete) = replay.complete_v2()?;
    let complete = challenges.complete_v2()?;
    let destination = writer.seal_v2(replay_complete)?.root_v2()?;
    Ok((source, destination, complete.continue_v2()?))
}
fn fold_terminal_v2(
    ready: ProverFriRoundsReadyV2,
    source: FriLayerRootedV2,
) -> Result<
    (
        FriLayerRootedV2,
        ZeroizingFriTerminalV2,
        FriLayerReplayCompleteV2,
        ProverFriQueriesV2,
    ),
    ProverPrerequisiteErrorV2,
> {
    let mut challenges = ready.bind_next_root_v2(source.root)?;
    let context = challenges.context_v2()?;
    let mut replay = source.begin_fold_replay_v2(context)?;
    let mut terminal = ZeroizingFriTerminalV2::new_v2();
    for column in 0..REPLAY_COLUMNS_V2 {
        let mut pair = replay.read_next_pair_v2(0, column)?;
        challenges.fold_terminal_column_in_place_v2(column, pair.terminal_in_place_v2()?)?;
        terminal.scatter_v2(column, pair.positive_v2())?;
    }
    let (source, replay_complete) = replay.complete_v2()?;
    let complete = challenges.complete_v2()?;
    let queries = complete
        .bind_terminal_v2(terminal.bytes_v2())?
        .derive_queries_v2()?;
    Ok((source, terminal, replay_complete, queries))
}
fn prepare_batch_fri_terminal_operation_v2(
    mut prepared: BatchFriLayer1RootPreparedV2,
    directory: &Path,
) -> Result<BatchFriTerminalPreparedV2, ProverPrerequisiteErrorV2> {
    let transcript = prepared
        .transcript
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let mut ready = transcript.begin_fri_rounds_v2()?;
    let layer1 = prepared
        .accepted_fri1
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let mut layers: [Option<FriLayerRootedV2>; FRI_CONTINUATION_LAYERS_V2] =
        array::from_fn(|_| None);
    layers[0] = Some(FriLayerRootedV2::from_layer1_v2(layer1));
    for source_layer in 1..17_usize {
        let source = layers[source_layer - 1]
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let (source, destination, next) = fold_round_into_layer_v2(directory, ready, source)?;
        layers[source_layer - 1] = Some(source);
        layers[source_layer] = Some(destination);
        ready = next;
    }
    let source = layers[16]
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let (source, terminal, terminal_replay_complete, transcript) = fold_terminal_v2(ready, source)?;
    layers[16] = Some(source);
    Ok(BatchFriTerminalPreparedV2 {
        accepted_c0: prepared.accepted_c0.take(),
        masks: prepared.masks.take(),
        accepted_cq: prepared.accepted_cq.take(),
        accepted_fri0: prepared.accepted_fri0.take(),
        accepted_fri_layers: layers,
        transcript: Some(transcript),
        terminal,
        terminal_replay_complete: Some(terminal_replay_complete),
        evaluations: prepared.evaluations,
        context: prepared.context,
        parameter_digest: prepared.parameter_digest,
        initial_root: prepared.initial_root,
        quotient_root: prepared.quotient_root,
        layer0_root: prepared.layer0_root,
    })
}
#[path = "fri_layers2_17_v2/canonical_proof_v2.rs"]
mod canonical_proof_v2;
pub(in super::super) use canonical_proof_v2::*;
#[cfg(test)]
#[path = "fri_layers2_17_v2_tests.rs"]
mod tests;
