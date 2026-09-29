//! Interpreter equivalence, shape admission and real-cell owner cleanup.

use super::super::*;
use super::*;
use crate::{
    BfvEvaluationKeyBundle, BfvIdentifierCiphertext, derive_identifier_key_material_from_seed,
    encrypt_identifier_from_seed, ram_lfe_bfv_parameters_v1,
};

// Fixed public test material. Runtime key generation must use secret entropy.
const SECRET: [u8; 32] =
    hex_literal::hex!("31177bbb52caec119f92536239fcfb677f9f5801ac0ce6cdf04c657ddeb938c7");
const ENCRYPTION_SEED: [u8; 32] =
    hex_literal::hex!("63190085e6ef1eafb7c576956a8d4f31779e1bd7d53346ffe2ea950a1bbf5d30");

fn fixture(program: &HiddenRamFheProgram) -> (PolicyCommitment, ClientRequest) {
    let params = ram_lfe_bfv_parameters_v1();
    let (encryption, _, relinearization_key) =
        derive_identifier_key_material_from_seed(&params, 63, &SECRET, b"trace-program").unwrap();
    let ciphertext = encrypt_identifier_from_seed(&encryption, &[2, 3], &ENCRYPTION_SEED).unwrap();
    let public = try_bfv_programmed_public_parameters_with_program(
        encryption,
        BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: None,
        },
        program,
        RamLfeVerificationMode::Signed,
        None,
    )
    .unwrap();
    let commitment = bfv_programmed_policy_commitment_with_program(
        &SECRET,
        &norito::to_bytes(&public).unwrap(),
        program,
    )
    .unwrap();
    (
        commitment,
        ClientRequest {
            normalized_input: norito::to_bytes(&ciphertext).unwrap(),
            associated_data: b"trace-program".to_vec(),
        },
    )
}

fn program(instructions: Vec<HiddenRamFheInstruction>) -> HiddenRamFheProgram {
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for instruction in instructions {
        builder.push(instruction).unwrap();
    }
    builder.finish().unwrap()
}

#[test]
fn all_eleven_instructions_match_untraced_execution_and_record_state() {
    use HiddenRamFheInstruction::*;
    let program = program(vec![
        LoadInput(0, 1),
        LoadState(1, 0),
        StoreState(1, 0),
        LoadConst(1, 5),
        Add(2, 0, 1),
        AddPlain(2, 2, 1),
        SubPlain(2, 2, 1),
        MulPlain(2, 2, 3),
        Mul(3, 0, 1),
        LoadConst(0, 0),
        SelectEqZero(0, 0, 2, 3),
        Output(0),
    ]);
    let (commitment, request) = fixture(&program);
    let ordinary =
        evaluate_commitment_with_hidden_program(&SECRET, &commitment, &request, Some(&program))
            .unwrap();
    let (traced, trace) =
        evaluate_programmed_with_trace(&SECRET, &commitment, &request, &program).unwrap();
    assert_eq!(ordinary, traced);
    assert_eq!(trace.step_count(), program.instruction_count());
    assert_eq!(trace.output_count(), 1);
    assert!(trace.snapshot(trace.step_count() + 1).is_none());
    assert!(trace.snapshot(usize::MAX).is_none());
    let initial = trace.snapshot(0).unwrap();
    assert!(initial[..4 * 128].iter().all(|&cell| cell == 0));
    let residues =
        initialization::derive_residues(&SECRET, commitment.policy_hash, &request.associated_data)
            .unwrap();
    for (index, &residue) in residues.iter().enumerate() {
        let lane = &initial[(4 + index) * 128..(5 + index) * 128];
        assert_eq!(lane[0], residue);
        assert!(lane[1..].iter().all(|&cell| cell == 0));
    }
    let after_load = trace.snapshot(1).unwrap();
    let after_store = trace.snapshot(3).unwrap();
    assert_eq!(&after_load[..128], &after_store[5 * 128..6 * 128]);
    let output: BfvIdentifierCiphertext = norito::decode_from_bytes(&traced.output).unwrap();
    let params = ram_lfe_bfv_parameters_v1();
    let (_, key, _) =
        derive_identifier_key_material_from_seed(&params, 63, &SECRET, b"trace-program").unwrap();
    assert_eq!(
        crate::decrypt(&params, &key, &output.slots[0]).unwrap()[0],
        21
    );
    let final_state = trace.snapshot(trace.step_count()).unwrap();
    assert_eq!(&final_state[..64], output.slots[0].c0.as_slice());
    assert_eq!(&final_state[64..128], output.slots[0].c1.as_slice());
    assert_eq!(format!("{trace:?}"), "[REDACTED RAM-LFE execution trace]");
    let tags: std::collections::BTreeSet<_> = trace.instructions[..trace.steps * INSTRUCTION_WIDTH]
        .chunks_exact(INSTRUCTION_WIDTH)
        .map(|row| row[0])
        .collect();
    assert_eq!(tags, (0..11).collect());
}

#[test]
fn maximum_program_has_exact_bounded_snapshot_and_output_capacity() {
    let program = default_bfv_programmed_hidden_program();
    let (commitment, request) = fixture(&program);
    let (response, trace) =
        evaluate_programmed_with_trace(&SECRET, &commitment, &request, &program).unwrap();
    assert_eq!(trace.step_count(), 256);
    assert_eq!(trace.output_count(), 64);
    assert_eq!(
        trace.snapshots.len() * std::mem::size_of::<u64>(),
        9_474_048
    );
    assert_eq!(trace.instructions.len(), 256 * INSTRUCTION_WIDTH);
    assert_eq!(
        response,
        evaluate_commitment_with_hidden_program(&SECRET, &commitment, &request, Some(&program),)
            .unwrap()
    );
}

#[test]
fn unused_malformed_input_and_oversized_associated_data_reject_before_execution() {
    let program = program(vec![
        HiddenRamFheInstruction::LoadInput(0, 0),
        HiddenRamFheInstruction::Output(0),
    ]);
    let (commitment, mut request) = fixture(&program);
    let mut input: BfvIdentifierCiphertext =
        norito::decode_from_bytes(&request.normalized_input).unwrap();
    input.slots.last_mut().unwrap().c0.pop();
    request.normalized_input = norito::to_bytes(&input).unwrap();
    assert!(evaluate_programmed_with_trace(&SECRET, &commitment, &request, &program).is_err());
    request
        .associated_data
        .resize(RAM_LFE_PROGRAM_ASSOCIATED_DATA_MAX_BYTES + 1, 0);
    request.normalized_input = vec![0xff];
    let error =
        evaluate_programmed_with_trace(&SECRET, &commitment, &request, &program).unwrap_err();
    assert!(error.to_string().contains("associated data"));
}

fn private_ciphertext() -> OwnedCiphertext {
    OwnedCiphertext(BfvCiphertext {
        c0: vec![41; 64],
        c1: vec![73; 64],
    })
}

#[test]
fn plaintext_scalar_lift_preserves_all_other_ciphertext_coefficients() {
    let params = ram_lfe_bfv_parameters_v1();
    let input = private_ciphertext();
    for scalar in [0, 1, 256] {
        let output = OwnedCiphertext(crate::add_plain_scalar(&params, &input, scalar).unwrap());
        assert_eq!(output.c0[0], 41 + scalar);
        assert!(output.c0[1..].iter().all(|&value| value == 41));
        assert_eq!(output.c1, input.c1);
    }
    assert!(crate::add_plain_scalar(&params, &input, 257).is_err());
}

#[test]
fn owned_ciphertexts_clear_real_cells_on_overwrite_rejection_and_unwind() {
    CLEARED.with_borrow_mut(|count| *count = Some(0));
    {
        let mut values = OwnedCiphertexts::with_capacity(1).unwrap();
        values.push(private_ciphertext()).unwrap();
        values.replace(0, private_ciphertext()).unwrap();
        assert!(values.replace(1, private_ciphertext()).is_err());
        assert!(values.push(private_ciphertext()).is_err());
    }
    assert!(
        std::panic::catch_unwind(|| {
            let _value = private_ciphertext();
            panic!("private-owner unwind control");
        })
        .is_err()
    );
    let cells = CLEARED.with_borrow_mut(|count| count.take().unwrap());
    assert_eq!(cells, 5 * 128);
}

#[test]
fn trace_clears_populated_snapshots_metadata_and_partial_error() {
    CLEARED.with_borrow_mut(|count| *count = Some(0));
    for unwind in [false, true] {
        let result = std::panic::catch_unwind(|| {
            let mut trace = RamLfeProgramExecutionTrace::new().unwrap();
            trace.instructions[0] = 31;
            let mut registers = OwnedCiphertexts::with_capacity(4).unwrap();
            let mut memory = OwnedCiphertexts::with_capacity(32).unwrap();
            for _ in 0..4 {
                registers.push(private_ciphertext()).unwrap();
            }
            for _ in 0..32 {
                memory.push(private_ciphertext()).unwrap();
            }
            memory.0.last_mut().unwrap().c1.pop();
            assert!(trace.record(None, &registers, &memory, 0).is_err());
            assert_eq!(
                trace.snapshots[0], 41,
                "record copied real preceding cells before error"
            );
            assert_eq!(
                trace.snapshots[35 * 128],
                0,
                "malformed final row was not copied"
            );
            assert!(!unwind, "trace-owner unwind control");
        });
        assert_eq!(result.is_err(), unwind);
    }
    let cells = CLEARED.with_borrow_mut(|count| count.take().unwrap());
    assert_eq!(
        cells,
        2 * (257 * SNAPSHOT_WIDTH + 256 * INSTRUCTION_WIDTH + 36 * 128 - 1)
    );
}
