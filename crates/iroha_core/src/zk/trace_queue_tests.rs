//! Trace-proof and trace-proving queue regressions.

use super::*;
use ivm::encoding;
use std::{num::NonZeroU64, sync::Arc};

fn assemble_zk(code: &[u8], max_cycles: u64) -> Vec<u8> {
    use ivm::ProgramMetadata;

    let meta = ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        vector_length: 0,
        max_cycles,
        abi_version: 1,
        ..ProgramMetadata::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(code);
    program
}

fn sample_zk_task() -> crate::pipeline::zk_lane::ZkTask {
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let program = assemble_zk(&halt, 4);
    let code_hash = ivm::contract_code_hash(&program);
    let trace = vec![
        ivm::zk::RegisterState {
            pc: 0,
            gpr: [0u64; 256],
            tags: [false; 256],
        },
        ivm::zk::RegisterState {
            pc: 4,
            gpr: [0u64; 256],
            tags: [false; 256],
        },
    ];
    let constraints: Vec<ivm::zk::Constraint> = Vec::new();
    let circuit = VMExecutionCircuit::new(&program, &trace, &constraints);
    assert!(circuit.verify().is_ok(), "sample trace must verify");
    crate::pipeline::zk_lane::ZkTask {
        tx_hash: None,
        code_hash: *code_hash.as_ref(),
        program: Arc::from(program),
        header: None,
        trace,
        constraints,
        mem_log: Vec::new(),
        reg_log: Vec::new(),
        step_log: Vec::new(),
        transport_capabilities: None,
        negotiated_capabilities: None,
    }
}

#[test]
fn queue_and_collect_trace_proofs() {
    reset_trace_proof_state_for_tests();
    let code_hash = [0x11; 32];
    let digest = [0xAA; 32];
    let artifact = make_trace_digest_artifact(code_hash, None, digest);
    queue_trace_proof(7, artifact.clone());
    let collected = collect_trace_proofs_for_height(7);
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].backend, TRACE_DIGEST_BACKEND);
    assert_eq!(collected[0].proof, digest.to_vec());
    assert_eq!(collected[0].code_hash, code_hash);
    assert!(collected[0].tx_hash.is_none());
    // Subsequent collection should be empty once drained.
    assert!(collect_trace_proofs_for_height(7).is_empty());
}

#[test]
fn queue_and_collect_trace_jobs() {
    reset_trace_proving_state_for_tests();
    let task = sample_zk_task();
    let digest = task.digest();
    queue_trace_for_proving(3, TraceForProving::from_task(&task, digest));
    let collected = collect_traces_for_proving(3);
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].digest, digest);
    assert_eq!(collected[0].code_hash, task.code_hash);
    assert!(Arc::ptr_eq(&collected[0].program, &task.program));
}

#[test]
fn trace_job_validation_does_not_emit_mock_proof_artifacts() {
    reset_trace_proof_state_for_tests();
    reset_trace_proving_state_for_tests();
    let mut task = sample_zk_task();
    let height = NonZeroU64::new(9).expect("non-zero");
    task.header = Some(iroha_data_model::block::BlockHeader::new(
        height, None, None, None, 0, 0,
    ));
    let digest = task.digest();
    queue_trace_for_proving(height.get(), TraceForProving::from_task(&task, digest));
    let mut entries = collect_traces_for_proving(height.get());
    assert_eq!(entries.len(), 1);
    let entry = entries.pop().expect("trace entry");
    entry.validate().expect("trace validates");
    let collected = collect_trace_proofs_for_height(height.get());
    assert!(
        collected.is_empty(),
        "validation-only trace jobs must not emit proof artifacts: {collected:?}"
    );
}

#[test]
fn trace_job_validation_rejects_tampered_trace() {
    let task = sample_zk_task();
    let digest = task.digest();
    let mut entry = TraceForProving::from_task(&task, digest);
    entry.trace[1].pc = 0;
    let err = entry
        .validate()
        .expect_err("tampered trace must not validate");
    assert!(
        err.contains("pc") || err.contains("trace") || err.contains("constraint"),
        "unexpected validation error: {err}"
    );
}
