//! Canonical final-proof replay orchestration for the retained qPCS owners.

use core::convert::Infallible;
use std::path::Path;

use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::CanonicalProofSectionV2;

use super::*;

#[path = "sink_codec_v2.rs"]
mod sink_codec_v2;
pub(in super::super::super) use sink_codec_v2::BatchFriCanonicalProofSinkV2;
use sink_codec_v2::{CanonicalProofSinkWriterV2, write_canonical_prefix_v2};
#[path = "atomic_quarantine_v2.rs"]
mod atomic_quarantine_v2;
use atomic_quarantine_v2::*;
#[path = "merkle_pass_v2.rs"]
mod merkle_pass_v2;
use merkle_pass_v2::*;

const CANONICAL_PROOF_REPLAY_READ_BYTES_V2: u64 = 12_763_154_240;
const CANONICAL_PROOF_REPLAY_LEAF_HASHES_V2: u64 = 2_097_148;
const CANONICAL_PROOF_REPLAY_NODE_HASHES_V2: u64 = 2_097_128;
const CANONICAL_PROOF_PRIOR_RETAINED_BYTES_V2: u64 = 13_561_628_480;
const CANONICAL_PROOF_MAX_RETAINED_BYTES_V2: u64 = 13_590_918_880;
const CANONICAL_PROOF_SOURCE_TOTAL_IO_BYTES_V2: u64 = 44_671_067_200;
const CANONICAL_PROOF_KAT_AUTHENTICATED_IO_BYTES_V2: u64 = 44_752_739_200;
const CANONICAL_PROOF_KAT_END_TO_END_IO_BYTES_V2: u64 = 44_779_935_904;
const CANONICAL_PROOF_MAX_AUTHENTICATED_IO_BYTES_V2: u64 = 44_758_938_400;
const CANONICAL_PROOF_MAX_END_TO_END_IO_BYTES_V2: u64 = 44_788_184_192;
const CANONICAL_PROOF_KAT_WIRE_BYTES_V2: usize = 27_196_704;
const CANONICAL_PROOF_PEAK_HEAP_EXCLUDING_CALLER_SINK_V2: usize = 29_262_417;

const CANONICAL_PROOF_REPLAY_COMPLETE_V2: bool = false;
const CANONICAL_PROOF_EMITTED_V2: bool = false;
const CANONICAL_PROOF_ZERO_KNOWLEDGE_BOUND_V2: bool = false;
const CANONICAL_PROOF_RSS_ACCEPTED_V2: bool = false;
const CANONICAL_PROOF_RECEIPT_ACCEPTED_V2: bool = false;
const CANONICAL_PROOF_RELEASE_READY_V2: bool = false;

pub(in super::super::super) enum BatchFriCanonicalProofAuthorityV2 {
    Production {
        authenticated_final_replay: Infallible,
        canonical_merkle_proofs: Infallible,
        exact_sink: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

/// Move-only owner returned only after every replay and exact sink completion.
pub(in super::super::super) struct BatchFriCanonicalProofEmittedV2<Output> {
    output: Option<Output>,
    replay_binding_digest: [u8; 32],
}

pub(in super::super::super) struct BatchFriCanonicalProofPreparedV2 {
    terminal: BatchFriTerminalPreparedV2,
    purposes: Option<CanonicalProofReplayPurposesV2>,
    binding: CanonicalProofReplayBindingV2,
    writer: Option<CanonicalProofSinkWriterV2<AtomicProofQuarantineSinkV2>>,
}

impl<Output> BatchFriCanonicalProofEmittedV2<Output> {
    pub(in super::super::super) fn into_output_v2(
        mut self,
    ) -> Result<Output, ProverPrerequisiteErrorV2> {
        self.output
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)
    }

    pub(in super::super::super) const fn replay_binding_digest_v2(&self) -> [u8; 32] {
        self.replay_binding_digest
    }
}

fn section_and_root_v2(
    purposes: &CanonicalProofReplayPurposesV2,
    binding: &CanonicalProofReplayBindingV2,
    ordinal: usize,
) -> Result<(CanonicalProofSectionV2, [u8; 32]), ProverPrerequisiteErrorV2> {
    let section = purposes.plan_v2()?.section_v2(ordinal)?;
    let root = match ordinal {
        0 => binding.initial_root,
        1 => binding.quotient_root,
        2..=19 => binding.fri_roots[ordinal - 2],
        _ => return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof),
    };
    Ok((section, root))
}

pub(super) fn prepare_canonical_proof_quarantine_v2(
    mut terminal: BatchFriTerminalPreparedV2,
    directory: &Path,
) -> Result<BatchFriCanonicalProofPreparedV2, ProverPrerequisiteErrorV2> {
    let transcript = terminal
        .transcript
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let plan = transcript.into_canonical_proof_plan_v2()?;
    if plan.exact_wire_bytes_v2() > 29_245_792 {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let layer0_root = terminal
        .accepted_fri0
        .as_ref()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
        .root_digest_v2();
    if layer0_root != terminal.layer0_root {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut fri_roots = [[0_u8; 32]; 18];
    fri_roots[0] = layer0_root;
    for (destination, owner) in fri_roots[1..]
        .iter_mut()
        .zip(terminal.accepted_fri_layers.iter())
    {
        *destination = owner
            .as_ref()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
            .root;
    }
    let binding = CanonicalProofReplayBindingV2 {
        parameter_digest: terminal.parameter_digest,
        context: terminal.context,
        initial_root: terminal.initial_root,
        quotient_root: terminal.quotient_root,
        fri_roots,
        terminal_digest: keccak256(terminal.terminal.bytes_v2()),
    };
    let purposes = CanonicalProofReplayPurposesV2::bind_v2(plan, binding)?;
    let exact_wire_bytes = purposes.plan_v2()?.exact_wire_bytes_v2();
    let quarantine = AtomicProofQuarantineSinkV2::create_in_v2(
        directory,
        purposes.master_binding_v2(),
        exact_wire_bytes,
    )?;
    let writer = CanonicalProofSinkWriterV2::begin_v2(quarantine, exact_wire_bytes)?;
    Ok(BatchFriCanonicalProofPreparedV2 {
        terminal,
        purposes: Some(purposes),
        binding,
        writer: Some(writer),
    })
}

fn emit_initial_sections_v2<S: BatchFriCanonicalProofSinkV2>(
    prepared: &mut BatchFriTerminalPreparedV2,
    purposes: &mut CanonicalProofReplayPurposesV2,
    binding: &CanonicalProofReplayBindingV2,
    writer: &mut CanonicalProofSinkWriterV2<S>,
) -> Result<(), ProverPrerequisiteErrorV2> {
    let (section, root) = section_and_root_v2(purposes, binding, 0)?;
    let purpose = purposes.take_next_purpose_v2(0)?;
    let c0 = prepared
        .accepted_c0
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let replay = c0.begin_canonical_proof_replay_v2(
        prepared.context,
        purposes.master_binding_v2(),
        section,
        root,
        purpose,
    )?;
    drop(emit_merkle_section_v2(
        replay,
        purposes.plan_v2()?,
        section,
        prepared.parameter_digest,
        root,
        writer,
    )?);

    let (section, root) = section_and_root_v2(purposes, binding, 1)?;
    let purpose = purposes.take_next_purpose_v2(1)?;
    let cq = prepared
        .accepted_cq
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let replay = cq.begin_canonical_proof_replay_v2(
        prepared.context,
        prepared.parameter_digest,
        prepared.initial_root,
        purposes.master_binding_v2(),
        section,
        root,
        purpose,
    )?;
    drop(emit_merkle_section_v2(
        replay,
        purposes.plan_v2()?,
        section,
        prepared.parameter_digest,
        root,
        writer,
    )?);
    Ok(())
}

fn emit_fri_sections_v2<S: BatchFriCanonicalProofSinkV2>(
    prepared: &mut BatchFriTerminalPreparedV2,
    purposes: &mut CanonicalProofReplayPurposesV2,
    binding: &CanonicalProofReplayBindingV2,
    writer: &mut CanonicalProofSinkWriterV2<S>,
) -> Result<(), ProverPrerequisiteErrorV2> {
    let (section, root) = section_and_root_v2(purposes, binding, 2)?;
    let purpose = purposes.take_next_purpose_v2(2)?;
    let fri0 = prepared
        .accepted_fri0
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let replay =
        fri0.begin_canonical_proof_replay_v2(purposes.master_binding_v2(), section, root, purpose)?;
    drop(emit_merkle_section_v2(
        replay,
        purposes.plan_v2()?,
        section,
        prepared.parameter_digest,
        root,
        writer,
    )?);

    for layer in 1..=17_usize {
        let ordinal = layer + 2;
        let (section, root) = section_and_root_v2(purposes, binding, ordinal)?;
        let purpose = purposes.take_next_purpose_v2(ordinal as u8)?;
        let owner = prepared.accepted_fri_layers[layer - 1]
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let terminal_marker = if layer == 17 {
            Some(
                prepared
                    .terminal_replay_complete
                    .take()
                    .ok_or(ProverPrerequisiteErrorV2::Poisoned)?,
            )
        } else {
            None
        };
        let (_, batch_schedule_digest, fold_schedule_digest) =
            purposes.plan_v2()?.transcript_context_v2();
        let replay = owner.begin_canonical_proof_replay_v2(
            purposes.master_binding_v2(),
            section,
            root,
            purpose,
            terminal_marker,
            batch_schedule_digest,
            fold_schedule_digest,
        )?;
        drop(emit_merkle_section_v2(
            replay,
            purposes.plan_v2()?,
            section,
            prepared.parameter_digest,
            root,
            writer,
        )?);
    }
    Ok(())
}

struct BatchFriCanonicalProofQuarantinedV2 {
    quarantine: AtomicProofQuarantineReadyV2,
    replay_binding_digest: [u8; 32],
}

fn stage_canonical_proof_operation_v2(
    mut prepared: BatchFriCanonicalProofPreparedV2,
) -> Result<BatchFriCanonicalProofQuarantinedV2, ProverPrerequisiteErrorV2> {
    let binding = prepared.binding;
    let mut purposes = prepared
        .purposes
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let mut writer = prepared
        .writer
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let masks = prepared
        .terminal
        .masks
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    write_canonical_prefix_v2(
        &mut writer,
        &binding,
        &prepared.terminal.evaluations.bytes,
        prepared.terminal.terminal.bytes_v2(),
    )?;
    emit_initial_sections_v2(&mut prepared.terminal, &mut purposes, &binding, &mut writer)?;
    emit_fri_sections_v2(&mut prepared.terminal, &mut purposes, &binding, &mut writer)?;
    if prepared.terminal.masks.is_some()
        || prepared.terminal.accepted_c0.is_some()
        || prepared.terminal.accepted_cq.is_some()
        || prepared.terminal.accepted_fri0.is_some()
        || prepared
            .terminal
            .accepted_fri_layers
            .iter()
            .any(Option::is_some)
        || prepared.terminal.terminal_replay_complete.is_some()
    {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let replay_complete = purposes.complete_v2()?;
    let replay_binding_digest = replay_complete.binding_digest_v2();
    let quarantine = writer.finish_v2()?;
    drop(masks);
    Ok(BatchFriCanonicalProofQuarantinedV2 {
        quarantine,
        replay_binding_digest,
    })
}

fn emit_canonical_proof_operation_v2<S: BatchFriCanonicalProofSinkV2>(
    prepared: BatchFriCanonicalProofPreparedV2,
    sink: S,
) -> Result<BatchFriCanonicalProofEmittedV2<S::Output>, ProverPrerequisiteErrorV2> {
    let (output, replay_binding_digest) = release_after_atomic_quarantine_v2(sink, || {
        let staged = stage_canonical_proof_operation_v2(prepared)?;
        Ok((staged.quarantine, staged.replay_binding_digest))
    })?;
    Ok(BatchFriCanonicalProofEmittedV2 {
        output: Some(output),
        replay_binding_digest,
    })
}

impl BatchFriCanonicalProofPreparedV2 {
    pub(in super::super::super) fn emit_canonical_proof_v2<S: BatchFriCanonicalProofSinkV2>(
        self,
        authority: BatchFriCanonicalProofAuthorityV2,
        sink: S,
    ) -> Result<BatchFriCanonicalProofEmittedV2<S::Output>, ProverPrerequisiteErrorV2> {
        match authority {
            BatchFriCanonicalProofAuthorityV2::Production {
                authenticated_final_replay,
                canonical_merkle_proofs: _canonical_merkle_proofs,
                exact_sink: _exact_sink,
            } => match authenticated_final_replay {},
            #[cfg(test)]
            BatchFriCanonicalProofAuthorityV2::TestOnly => {}
        }
        emit_canonical_proof_operation_v2(self, sink)
    }
}

const _: () = {
    assert!(
        CANONICAL_PROOF_REPLAY_READ_BYTES_V2
            == 2 * CQ_COLUMN_FILE_BYTES_V2 + FRI_ALL_LAYER_FILE_BYTES_V2
    );
    assert!(CANONICAL_PROOF_REPLAY_LEAF_HASHES_V2 == 2 * 524_288 + FRI_END_TO_END_LEAF_HASHES_V2);
    assert!(CANONICAL_PROOF_REPLAY_NODE_HASHES_V2 == 2 * 524_287 + FRI_END_TO_END_NODE_HASHES_V2);
    assert!(CANONICAL_PROOF_PRIOR_RETAINED_BYTES_V2 == FRI_WITH_PRIOR_RETAINED_BYTES_V2);
    assert!(
        CANONICAL_PROOF_MAX_RETAINED_BYTES_V2
            == CANONICAL_PROOF_PRIOR_RETAINED_BYTES_V2 + CANONICAL_QUARANTINE_MAX_FILE_BYTES_V2
    );
    assert!(
        CANONICAL_PROOF_SOURCE_TOTAL_IO_BYTES_V2
            == FRI_END_TO_END_IO_BYTES_V2 + CANONICAL_PROOF_REPLAY_READ_BYTES_V2
    );
    assert!(
        CANONICAL_PROOF_KAT_AUTHENTICATED_IO_BYTES_V2
            == CANONICAL_PROOF_SOURCE_TOTAL_IO_BYTES_V2 + CANONICAL_QUARANTINE_KAT_IO_BYTES_V2
    );
    assert!(
        CANONICAL_PROOF_KAT_END_TO_END_IO_BYTES_V2
            == CANONICAL_PROOF_KAT_AUTHENTICATED_IO_BYTES_V2
                + CANONICAL_PROOF_KAT_WIRE_BYTES_V2 as u64
    );
    assert!(
        CANONICAL_PROOF_MAX_AUTHENTICATED_IO_BYTES_V2
            == CANONICAL_PROOF_SOURCE_TOTAL_IO_BYTES_V2 + CANONICAL_QUARANTINE_MAX_IO_BYTES_V2
    );
    assert!(
        CANONICAL_PROOF_MAX_END_TO_END_IO_BYTES_V2
            == CANONICAL_PROOF_MAX_AUTHENTICATED_IO_BYTES_V2 + 29_245_792
    );
    assert!(
        CANONICAL_PROOF_PEAK_HEAP_EXCLUDING_CALLER_SINK_V2 == CANONICAL_QUARANTINE_HEAP_BYTES_V2
    );
    assert!(CANONICAL_PROOF_KAT_WIRE_BYTES_V2 == 27_196_704);
    assert!(!CANONICAL_PROOF_REPLAY_COMPLETE_V2);
    assert!(!CANONICAL_PROOF_EMITTED_V2);
    assert!(!CANONICAL_PROOF_ZERO_KNOWLEDGE_BOUND_V2);
    assert!(!CANONICAL_PROOF_RSS_ACCEPTED_V2);
    assert!(!CANONICAL_PROOF_RECEIPT_ACCEPTED_V2);
    assert!(!CANONICAL_PROOF_RELEASE_READY_V2);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_guards_keep_final_replay_private_sequential_and_non_authorizing() {
        let source = include_str!("canonical_proof_v2.rs");
        let common = include_str!("../../../../../canonical_proof_replay_v2.rs");
        let c0_cq = include_str!("../../../../../post_c0_replay_v2/canonical_proof_replay_v2.rs");
        let quarantine = include_str!("atomic_quarantine_v2.rs");
        assert!(source.lines().count() <= 450);
        assert!(source.contains("authenticated_final_replay: Infallible"));
        assert!(source.contains("const CANONICAL_PROOF_EMITTED_V2: bool = false;"));
        assert!(common.contains("fn read_next_column_v2("));
        assert!(!common.contains("fn snapshot_v2"));
        assert!(!common.contains("fn path_v2"));
        assert!(!common.contains("fn key_v2"));
        assert!(!c0_cq.contains("pub fn"));
        assert!(source.contains("fn stage_canonical_proof_operation_v2("));
        let stage = source
            .split("fn stage_canonical_proof_operation_v2(")
            .nth(1)
            .unwrap()
            .split("fn emit_canonical_proof_operation_v2")
            .next()
            .unwrap();
        assert!(!stage.contains("sink: S"));
        assert!(stage.contains("emit_initial_sections_v2("));
        assert!(stage.contains("emit_fri_sections_v2("));
        assert!(stage.contains("purposes.complete_v2()?"));
        assert!(stage.contains("writer.finish_v2()?"));
        assert!(source.contains("release_after_atomic_quarantine_v2(sink, ||"));
        assert!(quarantine.contains("ConfidentialSpoolWriterV1"));
        assert!(quarantine.contains("AtomicProofQuarantineReplayPermitV2"));
        assert!(quarantine.contains("let (quarantine, retained) = stage()?;"));
        assert!(!quarantine.contains("impl Clone for AtomicProofQuarantine"));
        let quarantine_production = quarantine.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "PathBuf",
            "std::fs::File",
            "pub(super) fn replay_into_sink_v2",
            "Vec<",
        ] {
            assert!(!quarantine_production.contains(forbidden));
        }
        assert!(!source.contains("Clone for BatchFriCanonicalProofEmittedV2"));
    }
}
