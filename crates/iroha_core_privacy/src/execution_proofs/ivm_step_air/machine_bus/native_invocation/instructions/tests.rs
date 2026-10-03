//! Genuine native control producers, canonical padding and atomic load coverage.

use super::*;
use crate::execution_proofs::ivm_step_air::residues::{Scratch, Stream};
use ivm::{PreparedContract, encoding::wide as enc, instruction::wide};

pub(super) fn artifact(body: &[u32], frame: u32, literal: bool) -> PreparedContract {
    artifact_with_literals(body, frame, if literal { &[u64::MAX] } else { &[] })
}
pub(in super::super) fn artifact_with_literals(
    body: &[u32],
    frame: u32,
    literals: &[u64],
) -> PreparedContract {
    let original = private_dispatch::tests::contract(&[], 64, ivm::ivm_mode::ZK);
    let mut interface = original.contract_interface().clone();
    interface.callables[0].frame_bytes = frame;
    let mut bytes = original.metadata().encode();
    bytes.extend(interface.encode_section());
    if !literals.is_empty() {
        use ivm_abi::metadata::{LITERAL_SECTION_MAGIC, LiteralKindV1, encode_literal_descriptor};
        // Descriptors are relative to LTLB, while trailing code alignment is
        // relative to the fixed program header and includes the CNTR prefix.
        let data_offset = 16 + literals.len() * 8;
        let data_bytes = literals.len() * 8;
        let post_pad =
            (4 - (bytes.len() - original.header_len() + data_offset + data_bytes) % 4) % 4;
        bytes.extend(LITERAL_SECTION_MAGIC);
        bytes.extend((literals.len() as u32).to_le_bytes());
        bytes.extend((post_pad as u32).to_le_bytes());
        bytes.extend((data_bytes as u32).to_le_bytes());
        for index in 0..literals.len() {
            bytes.extend(
                encode_literal_descriptor(LiteralKindV1::I64, (data_offset + index * 8) as u64)
                    .unwrap()
                    .to_le_bytes(),
            );
        }
        bytes.extend(literals.iter().flat_map(|value| value.to_le_bytes()));
        bytes.resize(bytes.len() + post_pad, 0);
    }
    bytes.extend(body.iter().flat_map(|word| word.to_le_bytes()));
    bytes.extend(crate::ivm_test_support::unit_return());
    ivm::prepare_contract(bytes.into()).unwrap()
}
pub(super) fn native(artifact: PreparedContract) -> (NativeInvocation, AllocationBudget) {
    let budget = AllocationBudget::new(128 * 1024 * 1024);
    let mut parent = ivm::execution_memory::ExecutionMemoryLease::reserve(
        &budget,
        NativeInvocation::allocation_plan().unwrap(),
    )
    .unwrap();
    let native =
        NativeInvocation::run_unit_root(artifact, "main", 10_000, &mut parent, &budget).unwrap();
    (native, budget)
}
pub(super) fn root(native: &NativeInvocation) -> root::Plan {
    root::Plan::derive(
        native.artifact(),
        native.entrypoint_index(),
        native.initial_gas(),
    )
    .unwrap()
}
pub(super) fn check(instructions: &Instructions, native: &NativeInvocation, window: usize) -> bool {
    let mut scratch = Scratch::new();
    let mut consume = |values: &[F]| {
        if values.iter().all(|value| *value == F::ZERO) {
            Ok(())
        } else {
            Err(())
        }
    };
    let mut out = Stream::new(&mut scratch, &mut consume);
    instructions.append_residues(&mut out, native, window);
    out.finish().is_ok()
}

#[test]
fn all_original_windows_join_with_real_depth_reads_and_zero_inactive_packets() {
    let (native, budget) = native(artifact(
        &[
            enc::encode_ri(wide::arithmetic::ADDI, 4, 0, -1),
            enc::encode_ri(wide::arithmetic::ADDI, 4, 4, 1),
            enc::encode_ri(wide::arithmetic::ADDI, 0, 4, -128),
        ],
        0,
        false,
    ));
    let before = budget.reserved_bytes();
    let instructions = Instructions::new(&native, &root(&native), &budget).unwrap();
    assert_eq!(budget.reserved_bytes() - before, Instructions::BYTES);
    assert_eq!(INSTRUCTION_WINDOWS, 65);
    for window in 0..INSTRUCTION_WINDOWS {
        assert!(check(&instructions, &native, window), "window {window}");
        let clocks = instruction_clocks(window).unwrap();
        let pc = &native.packets()[clocks[0] as usize];
        let depth = &native.packets()[clocks[14] as usize];
        assert_eq!(depth.enabled(), pc.enabled());
        if depth.enabled() {
            assert_eq!(
                depth.space(),
                Some(ivm::execution_packets::PacketSpace::Owner)
            );
            assert_eq!(depth.index(), 36);
            assert_eq!(depth.clock(), clocks[14]);
            assert_eq!(depth.is_write(), window == INSTRUCTION_WINDOWS - 1);
            assert_eq!(depth.before(), &[0; 16]);
            assert_eq!(depth.after(), &[0; 16]);
        } else {
            assert!(
                clocks
                    .iter()
                    .all(|clock| !native.packets()[*clock as usize].enabled())
            );
        }
    }
    assert!(instruction_clocks(INSTRUCTION_WINDOWS).is_none());
    assert!(instruction_clocks(usize::MAX).is_none());
    drop(instructions);
    assert_eq!(budget.reserved_bytes(), before);
    drop(native);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn source_header_output_and_padding_mutations_fail_instruction_equations() {
    let (native, budget) = native(artifact(
        &[enc::encode_ri(wide::arithmetic::ADDI, 4, 0, -1)],
        0,
        false,
    ));
    let instructions = Instructions::new(&native, &root(&native), &budget).unwrap();
    for window in [0, 1, INSTRUCTION_WINDOWS - 2, INSTRUCTION_WINDOWS - 1] {
        let original = original(&native, window);
        let schedule =
            private_dispatch::Schedule::new(0, instruction_clocks(window).unwrap()).unwrap();
        for port in 0..private_dispatch::PORTS {
            for field in 0..packet::WIDTH {
                let fields = original.producer(port).unwrap();
                if fields[packet::WRITE] == F::ONE
                    && ((packet::BEFORE..packet::BEFORE + 8).contains(&field)
                        || field == packet::BEFORE_TAG)
                {
                    // Prior destination contents are authenticated by the same
                    // original history, not by every individual opcode bank.
                    // The complete-history adversary covers these fields too.
                    continue;
                }
                let mut values = core::array::from_fn(|index| *original.producer(index).unwrap());
                values[port][field] = values[port][field].add(F::ONE);
                let changed = private_dispatch::OriginalPackets::candidate(values);
                let mut residues = Vec::new();
                private_dispatch::append_residues(
                    &mut residues,
                    &instructions.program,
                    schedule,
                    &instructions.rows.as_slice()[window].0,
                    &changed,
                );
                assert!(
                    residues.iter().any(|value| *value != F::ZERO),
                    "window {window}, port {port}, field {field}"
                );
            }
        }
    }
    // The retired inactive running write is explicitly rejected even though
    // its zero-to-zero value transition would pass sorted history alone.
    let window = INSTRUCTION_WINDOWS - 2;
    let clocks = instruction_clocks(window).unwrap();
    let mut values = [[F::ZERO; packet::WIDTH]; private_dispatch::PORTS];
    values[20] = packet::Event {
        space: packet::Space::Owner,
        vm: 0,
        generation: 0,
        index: 35,
        write: true,
        before: [0; 16],
        after: [0; 16],
        before_private: 0,
        after_private: 0,
    }
    .fields(clocks[20] as usize);
    let changed = private_dispatch::OriginalPackets::candidate(values);
    let mut residues = Vec::new();
    private_dispatch::append_residues(
        &mut residues,
        &instructions.program,
        private_dispatch::Schedule::new(0, clocks).unwrap(),
        &instructions.rows.as_slice()[window].0,
        &changed,
    );
    assert!(residues.iter().any(|value| *value != F::ZERO));
}

#[test]
fn native_load_and_literal_paths_join_the_original_atomic_destination() {
    for (body, frame, literal) in [
        (
            vec![
                enc::encode_ri(wide::arithmetic::ADDI, 5, 31, -16),
                enc::encode_store(wide::memory::STORE64, 5, 0, 0),
                enc::encode_ri(wide::memory::LOAD64, 6, 5, 0),
            ],
            16,
            false,
        ),
        (vec![enc::encode_ri(wide::memory::LDI64, 6, 0, 0)], 0, true),
    ] {
        let (native, budget) = native(artifact(&body, frame, literal));
        let before = budget.reserved_bytes();
        let instructions = Instructions::new(&native, &root(&native), &budget).unwrap();
        assert_eq!(budget.reserved_bytes(), before + Instructions::BYTES);
        for window in 0..INSTRUCTION_WINDOWS {
            assert!(check(&instructions, &native, window), "window {window}");
        }
        drop(instructions);
        assert_eq!(budget.reserved_bytes(), before);
        let destination_clock = instruction_clocks(body.len() - 1).unwrap()[17];
        let destination = &native.packets()[destination_clock as usize];
        assert!(destination.enabled());
        assert_eq!(destination.index(), 6);
        assert!(
            !native.packets()[destination_clock as usize + 1].enabled(),
            "no fabricated tag commit"
        );
        let source = super::super::Source::new(native, &budget).unwrap();
        drop(source);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn exact_workspace_refusal_preserves_original_native_owner_and_final_refund() {
    let (native, original) = native(artifact(&[], 0, false));
    let reserved = original.reserved_bytes();
    let short = AllocationBudget::new(Instructions::BYTES - 1);
    assert!(matches!(
        Instructions::new(&native, &root(&native), &short),
        Err(Error::Allocation(_))
    ));
    assert_eq!(short.reserved_bytes(), 0);
    assert_eq!(original.reserved_bytes(), reserved);
    let exact = AllocationBudget::new(Instructions::BYTES);
    let instructions = Instructions::new(&native, &root(&native), &exact).unwrap();
    assert_eq!(exact.reserved_bytes(), Instructions::BYTES);
    drop(instructions);
    assert_eq!(exact.reserved_bytes(), 0);
    let mut scratch = Witness([F::ONE; private_dispatch::WIDTH]);
    scratch.clear();
    assert!(scratch.0.iter().all(|value| *value == F::ZERO));
    drop(native);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn maximum_native_step_profile_builds_and_evaluates_with_bounded_stack() {
    let body = [enc::encode_ri(wide::arithmetic::ADDI, 4, 4, 1); 60];
    let (native, budget) = native(artifact(&body, 0, false));
    assert_eq!(native.instructions(), ivm::execution_packets::MAX_STEPS);
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let before = budget.reserved_bytes();
            let instructions = Instructions::new(&native, &root(&native), &budget).unwrap();
            assert_eq!(budget.reserved_bytes() - before, Instructions::BYTES);
            for window in 0..INSTRUCTION_WINDOWS {
                assert!(check(&instructions, &native, window), "window {window}");
            }
            drop(instructions);
            drop(native);
            assert_eq!(budget.reserved_bytes(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
