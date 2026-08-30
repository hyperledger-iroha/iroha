//! Bounded B0 construction and authenticated B0-to-B1 fold prerequisite.
//!
//! This child pairs the exhausted C0 and Cq snapshots, consumes the one shared 760-value batch
//! schedule, and writes B0 only in block-major order. It seals and replays the authenticated
//! layer-0 file into the verifier-literal root. A second move-only boundary absorbs that root,
//! derives exactly the verifier's 380 layer-0 alphas, authenticates paired B0 blocks, and roots B1.
//! A bounded child continues B1 through B17 and the equal terminal. No proof, ZK, RSS, receipt, or
//! release authority is produced.
use super::*;
use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::{
    ProverBatchChallengesV2, ProverBatchRowsCompleteV2, ProverFriLayer0ChallengesV2,
    ProverFriLayer0FoldCompleteV2,
};
use core::convert::Infallible;
use iroha_crypto::confidential_spool::ConfidentialSpoolChunkV1;
use std::path::Path;
#[path = "batch_fri_v2/storage_v2.rs"]
mod storage_v2;
use storage_v2::*;
#[path = "batch_fri_v2/fri_layers2_17_v2.rs"]
mod fri_layers2_17_v2;
pub(super) use fri_layers2_17_v2::*;
const BATCH_FRI0_RECORDS_V2: u64 = 512 * 380;
const BATCH_FRI0_VALUES_V2: u64 = BATCH_FRI0_RECORDS_V2 * 1_024;
const BATCH_FRI0_INPUT_READ_BYTES_V2: u64 = 2 * 3_190_784_000;
const BATCH_FRI0_WRITE_BYTES_V2: u64 = 3_190_784_000;
const BATCH_FRI0_SEAL_READ_BYTES_V2: u64 = 3_190_784_000;
const BATCH_FRI0_ROOT_READ_BYTES_V2: u64 = 3_190_784_000;
const BATCH_FRI0_TOTAL_IO_BYTES_V2: u64 = 15_953_920_000;
const BATCH_FRI0_RETAINED_FILE_BYTES_V2: u64 = 10_370_826_240;
const BATCH_FRI0_LEAF_HASHES_V2: u64 = 524_288;
const BATCH_FRI0_NODE_HASHES_V2: u64 = 524_287;
const BATCH_FRI0_MIX_HEAP_BYTES_V2: usize = 3 * 16_384;
const BATCH_FRI0_ROOT_HEAP_BYTES_V2: usize = 6_225_920 + 16_384;
const BATCH_FRI0_ROOT_STACK_BYTES_V2: usize = 20 * 32;
const BATCH_FRI1_LEAVES_V2: u64 = 262_144;
const BATCH_FRI1_VALUES_V2: u64 = 99_614_720;
const BATCH_FRI1_RECORDS_V2: u64 = 97_280;
const BATCH_FRI1_B0_READ_BYTES_V2: u64 = 3_190_784_000;
const BATCH_FRI1_WRITE_BYTES_V2: u64 = 1_595_392_000;
const BATCH_FRI1_SEAL_READ_BYTES_V2: u64 = 1_595_392_000;
const BATCH_FRI1_ROOT_READ_BYTES_V2: u64 = 1_595_392_000;
const BATCH_FRI1_TOTAL_IO_BYTES_V2: u64 = 7_976_960_000;
const BATCH_FRI1_FILE_BYTES_V2: u64 = 1_595_392_000;
const BATCH_FRI01_RETAINED_FILE_BYTES_V2: u64 = 4_786_176_000;
const BATCH_FRI1_RETAINED_TOTAL_BYTES_V2: u64 = 11_966_218_240;
const BATCH_FRI1_FOLD_HEAP_BYTES_V2: usize = 49_152;
const BATCH_FRI1_ROOT_HEAP_BYTES_V2: usize = 6_242_304;
const BATCH_FRI1_FRONTIER_BYTES_V2: usize = 608;
const BATCH_FRI1_LEAF_HASHES_V2: u64 = 262_144;
const BATCH_FRI1_NODE_HASHES_V2: u64 = 262_143;
const BATCH_FRI1_WIRE_BYTES_V2: u64 = 0;
const BATCH_FRI0_MATERIALIZED_V2: bool = false;
const BATCH_FRI0_ROOT_SEALED_V2: bool = false;
const BATCH_FRI1_MATERIALIZED_V2: bool = false;
const BATCH_FRI1_ROOT_SEALED_V2: bool = false;
const AUTHENTICATED_FRI_REPLAY_COMPLETE_V2: bool = false;
const FRI_ALL_FOLDS_COMPLETE_V2: bool = false;
const BATCH_FRI_ZERO_KNOWLEDGE_BOUND_V2: bool = false;
const BATCH_FRI_CANONICAL_PROOF_EMITTED_V2: bool = false;
const BATCH_FRI_OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const BATCH_FRI_MEASURED_RSS_WITHIN_CAP_V2: bool = false;
const BATCH_FRI_RELEASE_READY_V2: bool = false;
const BATCH_FRI_RELEASE_COMPLETE_V2: bool = false;
const _: () = {
    assert!(BATCH_FRI0_RECORDS_V2 == RELEASE_LDE_SLOTS_V2);
    assert!(BATCH_FRI0_VALUES_V2 == 199_229_440);
    assert!(BATCH_FRI0_INPUT_READ_BYTES_V2 == 2 * RELEASE_LDE_FILE_BYTES_V2);
    assert!(BATCH_FRI0_WRITE_BYTES_V2 == FRI_RELEASE_FILES_V2[0]);
    assert!(BATCH_FRI0_SEAL_READ_BYTES_V2 == FRI_RELEASE_FILES_V2[0]);
    assert!(BATCH_FRI0_ROOT_READ_BYTES_V2 == FRI_RELEASE_FILES_V2[0]);
    assert!(
        BATCH_FRI0_TOTAL_IO_BYTES_V2
            == BATCH_FRI0_INPUT_READ_BYTES_V2
                + BATCH_FRI0_WRITE_BYTES_V2
                + BATCH_FRI0_SEAL_READ_BYTES_V2
                + BATCH_FRI0_ROOT_READ_BYTES_V2
    );
    assert!(BATCH_FRI0_MIX_HEAP_BYTES_V2 == 49_152);
    assert!(BATCH_FRI0_ROOT_HEAP_BYTES_V2 == 6_242_304);
    assert!(BATCH_FRI0_ROOT_STACK_BYTES_V2 == 640);
    assert!(
        BATCH_FRI0_RETAINED_FILE_BYTES_V2
            == COMBINED_AUTHENTICATED_FILE_BYTES_V2 + FRI_RELEASE_FILES_V2[0]
    );
    assert!(BATCH_FRI0_LEAF_HASHES_V2 == 1 << 19);
    assert!(BATCH_FRI0_NODE_HASHES_V2 + 1 == BATCH_FRI0_LEAF_HASHES_V2);
    assert!(BATCH_FRI1_LEAVES_V2 == 1 << 18);
    assert!(BATCH_FRI1_VALUES_V2 == BATCH_FRI1_LEAVES_V2 * 380);
    assert!(BATCH_FRI1_RECORDS_V2 == 256 * 380);
    assert!(BATCH_FRI1_B0_READ_BYTES_V2 == FRI_RELEASE_FILES_V2[0]);
    assert!(BATCH_FRI1_WRITE_BYTES_V2 == FRI_RELEASE_FILES_V2[1]);
    assert!(BATCH_FRI1_SEAL_READ_BYTES_V2 == FRI_RELEASE_FILES_V2[1]);
    assert!(BATCH_FRI1_ROOT_READ_BYTES_V2 == FRI_RELEASE_FILES_V2[1]);
    assert!(
        BATCH_FRI1_TOTAL_IO_BYTES_V2
            == BATCH_FRI1_B0_READ_BYTES_V2
                + BATCH_FRI1_WRITE_BYTES_V2
                + BATCH_FRI1_SEAL_READ_BYTES_V2
                + BATCH_FRI1_ROOT_READ_BYTES_V2
    );
    assert!(BATCH_FRI1_FILE_BYTES_V2 == FRI_RELEASE_FILES_V2[1]);
    assert!(
        BATCH_FRI01_RETAINED_FILE_BYTES_V2 == FRI_RELEASE_FILES_V2[0] + FRI_RELEASE_FILES_V2[1]
    );
    assert!(
        BATCH_FRI1_RETAINED_TOTAL_BYTES_V2
            == COMBINED_AUTHENTICATED_FILE_BYTES_V2 + BATCH_FRI01_RETAINED_FILE_BYTES_V2
    );
    assert!(BATCH_FRI1_FOLD_HEAP_BYTES_V2 == 3 * 16_384);
    assert!(BATCH_FRI1_ROOT_HEAP_BYTES_V2 == 6_225_920 + 16_384);
    assert!(BATCH_FRI1_FRONTIER_BYTES_V2 == 19 * 32);
    assert!(BATCH_FRI1_LEAF_HASHES_V2 == BATCH_FRI1_LEAVES_V2);
    assert!(BATCH_FRI1_NODE_HASHES_V2 + 1 == BATCH_FRI1_LEAF_HASHES_V2);
    assert!(BATCH_FRI1_WIRE_BYTES_V2 == 0);
    assert!(!BATCH_FRI0_MATERIALIZED_V2);
    assert!(!BATCH_FRI0_ROOT_SEALED_V2);
    assert!(!BATCH_FRI1_MATERIALIZED_V2);
    assert!(!BATCH_FRI1_ROOT_SEALED_V2);
    assert!(!AUTHENTICATED_FRI_REPLAY_COMPLETE_V2);
    assert!(!FRI_ALL_FOLDS_COMPLETE_V2);
    assert!(!BATCH_FRI_ZERO_KNOWLEDGE_BOUND_V2);
    assert!(!BATCH_FRI_CANONICAL_PROOF_EMITTED_V2);
    assert!(!BATCH_FRI_OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!BATCH_FRI_MEASURED_RSS_WITHIN_CAP_V2);
    assert!(!BATCH_FRI_RELEASE_READY_V2);
    assert!(!BATCH_FRI_RELEASE_COMPLETE_V2);
};
pub(super) enum BatchFriLayer0AuthorityV2 {
    Production {
        exact_batch: Infallible,
        authenticated_storage: Infallible,
        layer0_root: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
pub(super) enum BatchFriLayer1AuthorityV2 {
    Production {
        authenticated_layer0_replay: Infallible,
        exact_layer0_fold: Infallible,
        authenticated_layer1_root: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
struct LiveBatchReplayPairV2 {
    c0: C0BatchReplayV2,
    cq: CqBatchReplayV2,
    challenges: ProverBatchChallengesV2,
    writer: FriLayer0WriterV2,
}
struct BatchReplayPairV2 {
    live: Option<LiveBatchReplayPairV2>,
}
struct LiveFriLayer1FoldV2 {
    replay: FriLayer0FoldReplayV2,
    challenges: ProverFriLayer0ChallengesV2,
    writer: FriLayer1WriterV2,
}
struct FriLayer1FoldV2 {
    live: Option<LiveFriLayer1FoldV2>,
}
impl BatchReplayPairV2 {
    fn write_next_v2(&mut self, block: u64, column: u16) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let c0 = live.c0.read_next_v2(block, column)?;
        let cq = live.cq.read_next_v2(block, column)?;
        let mut output = ConfidentialSpoolChunkV1::new_zeroed_v1(RELEASE_LDE_BLOCK_BYTES_V2)?;
        live.challenges.mix_next_block_v2(
            block,
            column,
            c0.bytes_v2(),
            cq.bytes_v2(),
            output.as_mut_slice_v1(),
        )?;
        live.writer.push_next_v2(block, column, output)?;
        self.live = Some(live);
        Ok(())
    }
    fn complete_v2(
        mut self,
    ) -> Result<
        (
            QPcsC0StoredV2,
            QPcsCqStoredV2,
            ProverBatchRowsCompleteV2,
            FriLayer0SealedV2,
        ),
        ProverPrerequisiteErrorV2,
    > {
        let live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let accepted_c0 = live.c0.complete_v2()?;
        let (accepted_cq, replay_permit) = live.cq.complete_v2()?;
        let transcript = live.challenges.complete_v2()?;
        let accepted_fri0 = live.writer.seal_v2(replay_permit)?;
        Ok((accepted_c0, accepted_cq, transcript, accepted_fri0))
    }
}
impl FriLayer1FoldV2 {
    fn fold_next_v2(
        &mut self,
        pair_block: u64,
        column: u16,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let pair = live.replay.read_next_pair_v2(pair_block, column)?;
        let mut output = ConfidentialSpoolChunkV1::new_zeroed_v1(RELEASE_LDE_BLOCK_BYTES_V2)?;
        live.challenges.fold_next_pair_v2(
            pair_block,
            column,
            pair.lower.bytes_v2(),
            pair.upper.bytes_v2(),
            output.as_mut_slice_v1(),
        )?;
        live.writer.push_next_v2(pair_block, column, output)?;
        self.live = Some(live);
        Ok(())
    }
    fn complete_v2(
        mut self,
    ) -> Result<
        (
            FriLayer0RootedV2,
            FriLayer1SealedV2,
            ProverFriLayer0FoldCompleteV2,
        ),
        ProverPrerequisiteErrorV2,
    > {
        let live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let (accepted_fri0, replay_complete) = live.replay.complete_v2()?;
        let transcript = live.challenges.complete_v2()?;
        let accepted_fri1 = live.writer.seal_v2(replay_complete)?;
        Ok((accepted_fri0, accepted_fri1, transcript))
    }
}
/// Non-authorizing owner of every authenticated artifact through FRI layer 0.
pub(super) struct BatchFriLayer0RootPreparedV2 {
    accepted_c0: Option<QPcsC0StoredV2>,
    masks: Option<MaskSpoolSealedV2>,
    accepted_cq: Option<QPcsCqStoredV2>,
    accepted_fri0: Option<FriLayer0RootedV2>,
    transcript: Option<ProverBatchRowsCompleteV2>,
    evaluations: ZeroizingEvaluationFrameV2,
    context: PublicSpoolContextV2,
    parameter_digest: [u8; 32],
    initial_root: [u8; 32],
    quotient_root: [u8; 32],
    layer0_root: [u8; 32],
}
/// Non-authorizing owner of every authenticated artifact through FRI layer 1.
pub(super) struct BatchFriLayer1RootPreparedV2 {
    accepted_c0: Option<QPcsC0StoredV2>,
    masks: Option<MaskSpoolSealedV2>,
    accepted_cq: Option<QPcsCqStoredV2>,
    accepted_fri0: Option<FriLayer0RootedV2>,
    accepted_fri1: Option<FriLayer1RootedV2>,
    transcript: Option<ProverFriLayer0FoldCompleteV2>,
    evaluations: ZeroizingEvaluationFrameV2,
    context: PublicSpoolContextV2,
    parameter_digest: [u8; 32],
    initial_root: [u8; 32],
    quotient_root: [u8; 32],
    layer0_root: [u8; 32],
    layer1_root: [u8; 32],
}
impl QuotientRootPreparedV2 {
    pub(super) fn prepare_batch_fri_layer0_root_v2(
        self,
        directory: &Path,
        authority: BatchFriLayer0AuthorityV2,
    ) -> Result<BatchFriLayer0RootPreparedV2, ProverPrerequisiteErrorV2> {
        match authority {
            BatchFriLayer0AuthorityV2::Production {
                exact_batch,
                authenticated_storage: _authenticated_storage,
                layer0_root: _layer0_root,
            } => match exact_batch {},
            #[cfg(test)]
            BatchFriLayer0AuthorityV2::TestOnly => {}
        }
        prepare_batch_fri_layer0_operation_v2(self, directory)
    }
}
fn prepare_batch_fri_layer0_operation_v2(
    mut prepared: QuotientRootPreparedV2,
    directory: &Path,
) -> Result<BatchFriLayer0RootPreparedV2, ProverPrerequisiteErrorV2> {
    let transcript = prepared
        .transcript
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let challenges = transcript.begin_batch_challenges_v2()?;
    let (pre_quotient_transcript, pre_layer_transcript, batch_schedule_digest) =
        challenges.context_v2()?;
    let binding = FriLayer0BindingV2::new_v2(
        prepared.parameter_digest,
        prepared.context,
        prepared.initial_root,
        prepared.quotient_root,
        pre_layer_transcript,
        batch_schedule_digest,
    )?;
    let c0 = prepared
        .accepted_c0
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
        .begin_c0_batch_replay_v2(prepared.context)?;
    let cq = prepared
        .accepted_cq
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
        .begin_cq_batch_replay_v2(
            prepared.context,
            prepared.parameter_digest,
            prepared.initial_root,
            pre_quotient_transcript,
        )?;
    let writer = FriLayer0WriterV2::create_v2(directory, binding)?;
    let mut pair = BatchReplayPairV2 {
        live: Some(LiveBatchReplayPairV2 {
            c0,
            cq,
            challenges,
            writer,
        }),
    };
    for block in 0..REPLAY_BLOCKS_PER_COLUMN_V2 {
        for column in 0..REPLAY_COLUMNS_V2 {
            pair.write_next_v2(block, column)?;
        }
    }
    let (accepted_c0, accepted_cq, transcript, accepted_fri0) = pair.complete_v2()?;
    let accepted_fri0 = accepted_fri0.root_v2()?;
    let layer0_root = accepted_fri0.root_digest_v2();
    Ok(BatchFriLayer0RootPreparedV2 {
        accepted_c0: Some(accepted_c0),
        masks: prepared.masks.take(),
        accepted_cq: Some(accepted_cq),
        accepted_fri0: Some(accepted_fri0),
        transcript: Some(transcript),
        evaluations: prepared.evaluations,
        context: prepared.context,
        parameter_digest: prepared.parameter_digest,
        initial_root: prepared.initial_root,
        quotient_root: prepared.quotient_root,
        layer0_root,
    })
}
impl BatchFriLayer0RootPreparedV2 {
    pub(super) fn prepare_batch_fri_layer1_root_v2(
        self,
        directory: &Path,
        authority: BatchFriLayer1AuthorityV2,
    ) -> Result<BatchFriLayer1RootPreparedV2, ProverPrerequisiteErrorV2> {
        match authority {
            BatchFriLayer1AuthorityV2::Production {
                authenticated_layer0_replay,
                exact_layer0_fold: _exact_layer0_fold,
                authenticated_layer1_root: _authenticated_layer1_root,
            } => match authenticated_layer0_replay {},
            #[cfg(test)]
            BatchFriLayer1AuthorityV2::TestOnly => {}
        }
        prepare_batch_fri_layer1_operation_v2(self, directory)
    }
}
fn prepare_batch_fri_layer1_operation_v2(
    mut prepared: BatchFriLayer0RootPreparedV2,
    directory: &Path,
) -> Result<BatchFriLayer1RootPreparedV2, ProverPrerequisiteErrorV2> {
    let transcript = prepared
        .transcript
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let challenges = transcript.bind_fri_layer0_root_v2(prepared.layer0_root)?;
    let (
        pre_layer_transcript,
        post_layer0_transcript,
        batch_schedule_digest,
        layer0_fold_schedule_digest,
        challenge_layer0_root,
    ) = challenges.context_v2()?;
    if challenge_layer0_root != prepared.layer0_root {
        return Err(ProverPrerequisiteErrorV2::InvalidPostRootTranscript);
    }
    let accepted_fri0 = prepared
        .accepted_fri0
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let (replay, binding) = accepted_fri0.begin_layer1_fold_replay_v2(
        pre_layer_transcript,
        post_layer0_transcript,
        batch_schedule_digest,
        layer0_fold_schedule_digest,
        challenge_layer0_root,
    )?;
    let writer = FriLayer1WriterV2::create_v2(directory, binding)?;
    let mut fold = FriLayer1FoldV2 {
        live: Some(LiveFriLayer1FoldV2 {
            replay,
            challenges,
            writer,
        }),
    };
    for pair_block in 0..256 {
        for column in 0..REPLAY_COLUMNS_V2 {
            fold.fold_next_v2(pair_block, column)?;
        }
    }
    let (accepted_fri0, accepted_fri1, transcript) = fold.complete_v2()?;
    if transcript.context_v2()
        != (
            pre_layer_transcript,
            post_layer0_transcript,
            batch_schedule_digest,
            layer0_fold_schedule_digest,
            challenge_layer0_root,
        )
    {
        return Err(ProverPrerequisiteErrorV2::InvalidPostRootTranscript);
    }
    let accepted_fri1 = accepted_fri1.root_v2()?;
    let layer1_root = accepted_fri1.root_digest_v2();
    Ok(BatchFriLayer1RootPreparedV2 {
        accepted_c0: prepared.accepted_c0.take(),
        masks: prepared.masks.take(),
        accepted_cq: prepared.accepted_cq.take(),
        accepted_fri0: Some(accepted_fri0),
        accepted_fri1: Some(accepted_fri1),
        transcript: Some(transcript),
        evaluations: prepared.evaluations,
        context: prepared.context,
        parameter_digest: prepared.parameter_digest,
        initial_root: prepared.initial_root,
        quotient_root: prepared.quotient_root,
        layer0_root: prepared.layer0_root,
        layer1_root,
    })
}
#[cfg(test)]
#[path = "batch_fri_v2_tests.rs"]
mod tests;
