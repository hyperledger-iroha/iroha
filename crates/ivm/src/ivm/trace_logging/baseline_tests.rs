//! Delta-register traces of the frozen baseline engine: the baseline's own 256-register
//! geometry, observed without changing anything the run produces.

use super::{
    tests::{GAS, LIMIT},
    *,
};
use crate::{ProgramMetadata, host::DefaultHost, registers::BASELINE_REGISTER_COUNT};
use ivm_abi::{
    IvmProfileId,
    baseline::{
        encoding::{
            encode_halt,
            wide::{encode_ri, encode_rr},
        },
        instruction::wide::arithmetic as a,
    },
};

/// `r200 = 5`, `r7 = 9`, `r201 = r200 + r7`, then `HALT`: two of the three destinations lie
/// outside C10's 32 registers.
const WORDS: [u32; 3] = [
    encode_ri(a::ADDI, 200, 0, 5),
    encode_ri(a::ADDI, 7, 0, 9),
    encode_rr(a::ADD, 201, 200, 7),
];

fn baseline_vm(original: &AllocationBudget, mode: TraceMode) -> IVM {
    let mut image = ivm_abi::baseline::encode_header(
        &ProgramMetadata {
            max_cycles: 1_000,
            ..ProgramMetadata::default()
        },
        0,
    );
    image.extend(WORDS.iter().flat_map(|word| word.to_le_bytes()));
    image.extend(encode_halt().to_le_bytes());
    let mut vm = IVM::try_new_with_memory_budget(GAS, original).unwrap();
    vm.load_program_for_profile(&image, IvmProfileId::BASELINE_ABI_V1)
        .unwrap();
    assert_eq!(vm.engine(), IvmEngine::Baseline);
    vm.set_trace_mode(mode);
    vm
}

/// Everything a run produces that tracing must not change.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    pc: u64,
    gas: u64,
    cycles: u64,
    registers: [u64; BASELINE_REGISTER_COUNT],
    tags: [bool; BASELINE_REGISTER_COUNT],
    register_root: [u8; 32],
    halted: bool,
}

fn outcome(vm: &IVM) -> Outcome {
    Outcome {
        pc: vm.pc,
        gas: vm.gas_remaining,
        cycles: vm.cycles,
        registers: vm.registers.baseline_snapshot(),
        tags: vm.registers.baseline_snapshot_tags(),
        register_root: vm.register_root(),
        halted: vm.halted,
    }
}

#[test]
fn baseline_delta_trace_records_the_256_register_geometry() {
    let original = AllocationBudget::new(LIMIT);
    let mut vm = baseline_vm(&original, TraceMode::DeltaRegisters);
    vm.run_with_host(&mut DefaultHost::default())
        .expect("a traced baseline run is not refused");
    let trace = vm.delta_register_trace();
    // One observation before each fetched instruction: the three words and `HALT`.
    assert_eq!(trace.len(), 4);
    let first = trace.entry(0).unwrap();
    assert_eq!(
        first.changes.len(),
        BASELINE_REGISTER_COUNT,
        "the first observation records the complete baseline image"
    );
    assert!(
        first
            .changes
            .iter()
            .enumerate()
            .all(|(index, (register, _, _))| *register == index)
    );
    let later: Vec<_> = (1..4)
        .map(|index| trace.entry(index).unwrap().changes.to_vec())
        .collect();
    assert_eq!(
        later,
        [
            vec![(200, 5, false)],
            vec![(7, 9, false)],
            vec![(201, 14, false)]
        ]
    );
    let capture = vm.try_runtime_trace_capture().expect("capture");
    assert_eq!(capture.delta_len(), 4);
    assert_eq!(
        capture.delta(3).unwrap().changes,
        [(201_usize, 14_u64, false)]
    );
    drop(capture);
    drop(vm);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn baseline_tracing_only_observes() {
    let mut outcomes = Vec::new();
    for mode in [TraceMode::Off, TraceMode::PcOnly, TraceMode::DeltaRegisters] {
        let original = AllocationBudget::new(LIMIT);
        let mut vm = baseline_vm(&original, mode);
        vm.run_with_host(&mut DefaultHost::default()).unwrap();
        assert_eq!(vm.registers.get(201), 14);
        outcomes.push(outcome(&vm));
        // A second invocation of the same image starts a new trace with a complete image.
        vm.set_trace_mode(mode);
        vm.reset().unwrap();
        vm.run_with_host(&mut DefaultHost::default()).unwrap();
        if mode == TraceMode::DeltaRegisters {
            assert_eq!(
                vm.delta_register_trace().entry(0).unwrap().changes.len(),
                BASELINE_REGISTER_COUNT
            );
        }
    }
    assert_eq!(outcomes[0], outcomes[1], "PC tracing changes nothing");
    assert_eq!(outcomes[0], outcomes[2], "delta tracing changes nothing");
}
