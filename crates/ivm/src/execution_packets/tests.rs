//! Native ownership, exact ordinary execution and closed component boundaries.

use super::*;
use crate::{IVM, execution_memory::ExecutionMemoryLease};
use kotodama_lang::compiler::{Compiler, CompilerOptions};
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

thread_local! { static PANIC_NEXT_COMMIT: Cell<bool> = const { Cell::new(false) }; }
pub(crate) fn panic_if_requested() {
    assert!(
        !PANIC_NEXT_COMMIT.with(|flag| flag.replace(false)),
        "injected native owner unwind"
    );
}
fn contract(source: &str, max_cycles: u64) -> PreparedContract {
    let bytes = Compiler::new_with_options(CompilerOptions {
        force_zk: true,
        max_cycles,
        ..CompilerOptions::default()
    })
    .compile_source(source)
    .unwrap();
    crate::prepare_contract(std::sync::Arc::<[u8]>::from(bytes)).unwrap()
}
fn unit() -> PreparedContract {
    contract("seiyaku NativePackets { view fn main() { } }", 64)
}
fn budget() -> AllocationBudget {
    AllocationBudget::new(128 * 1024 * 1024)
}
fn parent(budget: &AllocationBudget) -> ExecutionMemoryLease {
    ExecutionMemoryLease::reserve(budget, NativeInvocation::allocation_plan().unwrap()).unwrap()
}
fn word(bytes: &[u8; 16]) -> u64 {
    u64::from_le_bytes(bytes[..8].try_into().unwrap())
}

#[test]
fn fixed_instruction_windows_capture_actual_depth_and_keep_inactive_slots_empty() {
    let budget = budget();
    let mut parent = parent(&budget);
    let output =
        NativeInvocation::run_unit_root(unit(), "main", 10_000, &mut parent, &budget).unwrap();
    let mut active = 0;
    let mut previous = None;
    for window in 0..INSTRUCTION_WINDOWS {
        let clocks = instruction_clocks(window).unwrap();
        assert!(clocks.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(previous.is_none_or(|clock| clock < clocks[0]));
        assert!((clocks[20] as usize) < PACKET_SLOTS);
        previous = Some(clocks[20]);
        let pc = &output.packets()[clocks[0] as usize];
        let depth = &output.packets()[clocks[14] as usize];
        assert_eq!(depth.enabled(), pc.enabled());
        if pc.enabled() {
            active += 1;
            assert_eq!(depth.space(), Some(PacketSpace::Owner));
            assert_eq!(depth.index(), 36);
            assert_eq!(depth.clock(), clocks[14]);
            assert_eq!(depth.is_write(), window == MAX_STEPS);
            assert_eq!(word(depth.before()), 0);
            assert_eq!(word(depth.after()), 0);
        } else {
            assert!(
                clocks
                    .iter()
                    .all(|clock| !output.packets()[*clock as usize].enabled())
            );
        }
    }
    assert_eq!(active, output.instructions());
    assert_eq!(instruction_clocks(0).unwrap()[0], ROOT_SLOTS as u32);
    assert_eq!(
        instruction_clocks(MAX_STEPS).unwrap()[0],
        schedule::RETURN_FIRST as u32
    );
    assert!(instruction_clocks(INSTRUCTION_WINDOWS).is_none());
    assert!(instruction_clocks(usize::MAX).is_none());
    drop(output);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_compiled_root_captures_staged_gas_store_initialization_return_and_padding() {
    let contract = unit();
    let mut ordinary = IVM::new(10_000);
    ordinary.load_prepared(&contract).unwrap();
    ordinary.select_entrypoint("main").unwrap();
    ordinary.run().unwrap();
    assert_eq!(ordinary.public_call_result_word(0), Ok(0));
    let budget = budget();
    let mut parent = parent(&budget);
    let output =
        NativeInvocation::run_unit_root(contract, "main", 10_000, &mut parent, &budget).unwrap();
    assert_eq!(output.remaining_gas(), ordinary.remaining_gas());
    assert_eq!(output.cycles(), ordinary.get_cycle_count());
    assert_eq!(output.cycles(), 64);
    assert_eq!(output.instructions(), 10);
    assert_eq!(output.packets().len(), PACKET_SLOTS);
    assert_eq!(output.initial_gas(), 10_000);
    assert_eq!(
        output
            .artifact()
            .entrypoint_descriptor("main")
            .unwrap()
            .entry_pc,
        output.artifact().contract_interface().entrypoints[output.entrypoint_index()].entry_pc
    );
    // Initialization and each original write/read form one continuous history.
    // This checks native custody only, not execution-proof soundness.
    let mut state = BTreeMap::new();
    for (clock, packet) in output.packets().iter().enumerate() {
        if !packet.enabled() {
            assert_eq!(packet.space(), None);
            assert_eq!(packet.clock(), 0);
            assert_eq!(packet.before(), &[0; 16]);
            assert_eq!(packet.after(), &[0; 16]);
            continue;
        }
        assert_eq!(packet.clock(), clock as u32);
        let key = (packet.space, packet.generation(), packet.index());
        let prior = state.entry(key).or_insert(([0; 16], 0));
        assert_eq!(
            (packet.before(), packet.before_private()),
            (&prior.0, prior.1),
            "clock {clock}"
        );
        if packet.is_write() {
            *prior = (*packet.after(), packet.after_private());
        } else {
            assert_eq!(
                (packet.before(), packet.before_private()),
                (packet.after(), packet.after_private())
            );
        }
    }
    let all = output.packets();
    assert_eq!(word(all[8].before()) - word(all[8].after()), 8);
    let entry = &output.artifact().contract_interface().entrypoints[output.entrypoint_index()];
    let frame = output
        .artifact()
        .contract_interface()
        .callables
        .iter()
        .find(|call| call.entry_pc == entry.entry_pc)
        .unwrap();
    assert_eq!(
        word(all[9].before()) - word(all[9].after()),
        crate::call_gas::frame(frame.frame_bytes, 1).unwrap()
    );
    assert_eq!(
        word(all[schedule::RETURN_FIRST + 22].before())
            - word(all[schedule::RETURN_FIRST + 22].after()),
        crate::call_gas::NODE
    );
    assert_eq!(
        word(all[schedule::RETURN_FIRST + 23].before())
            - word(all[schedule::RETURN_FIRST + 23].after()),
        crate::call_gas::WORD
    );
    let first_cell = schedule::RETURN_FIRST + schedule::SCAN_OFFSET;
    assert_eq!(word(all[first_cell].before()), 255);
    assert_eq!(all[first_cell].generation(), 1);
    for offset in 0..RETURN_CELLS {
        assert!(
            !all[first_cell + 2 * offset + 1].enabled(),
            "root has no parent copyback"
        );
        if offset != 0 {
            assert!(!all[first_cell + 2 * offset].enabled());
        }
    }
    assert_eq!(word(all[schedule::RETURN_FIRST + 46].before()), 1);
    assert_eq!(word(all[schedule::RETURN_FIRST + 46].after()), 0);
    assert_eq!(
        word(all[schedule::PADDING_FIRST + 1].before()),
        output.instructions() as u64
    );
    assert_eq!(word(all[schedule::PADDING_FIRST + 1].after()), 64);
    assert_eq!(parent.remaining_bytes(), 0);
    assert_eq!(
        budget.reserved_bytes(),
        NativeInvocation::allocation_plan()
            .unwrap()
            .requested_bytes()
    );
    drop(output);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn private_selector_arguments_nonunit_and_nonzk_profiles_are_closed_before_execution() {
    let budget = budget();
    for (contract, selector) in [
        (unit(), "missing"),
        (
            contract("seiyaku S { fn hidden() { } view fn main() { } }", 64),
            "hidden",
        ),
        (
            contract("seiyaku S { view fn main(bool input) { } }", 64),
            "main",
        ),
        (
            contract("seiyaku S { view fn main() -> bool { true } }", 64),
            "main",
        ),
        (contract("seiyaku S { view fn main() { } }", 65), "main"),
    ] {
        let mut parent = parent(&budget);
        assert!(matches!(
            NativeInvocation::run_unit_root(contract, selector, 10_000, &mut parent, &budget),
            Err(CaptureError::Unsupported)
        ));
        assert_eq!(
            parent.remaining_bytes(),
            NativeInvocation::allocation_plan()
                .unwrap()
                .requested_bytes()
        );
        drop(parent);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    let bytes = Compiler::new()
        .compile_source("seiyaku S { view fn main() { } }")
        .unwrap();
    let mut parent = parent(&budget);
    assert!(matches!(
        NativeInvocation::run_unit_root(
            crate::prepare_contract(std::sync::Arc::<[u8]>::from(bytes)).unwrap(),
            "main",
            10_000,
            &mut parent,
            &budget
        ),
        Err(CaptureError::Unsupported)
    ));
}

#[test]
fn child_calls_are_local_component_refusal_and_do_not_change_ordinary_validity() {
    let contract = contract(
        "seiyaku Child { fn leaf() { } view fn main() { leaf(); } }",
        64,
    );
    let mut ordinary = IVM::new(10_000);
    ordinary.load_prepared(&contract).unwrap();
    ordinary.select_entrypoint("main").unwrap();
    ordinary.run().unwrap();
    let budget = budget();
    let mut parent = parent(&budget);
    assert!(matches!(
        NativeInvocation::run_unit_root(contract, "main", 10_000, &mut parent, &budget),
        Err(CaptureError::Unsupported)
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn short_credit_wrong_pool_and_native_faults_never_publish_partial_owners() {
    let budget = budget();
    let mut short = ExecutionMemoryLease::reserve(&budget, ExecutionMemoryPlan::default()).unwrap();
    assert!(matches!(
        NativeInvocation::run_unit_root(unit(), "main", 10_000, &mut short, &budget),
        Err(CaptureError::Reservation(InsufficientReservation { requested_bytes, remaining_bytes: 0 }))
            if requested_bytes == NativeInvocation::allocation_plan().unwrap().requested_bytes()
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    let foreign = AllocationBudget::new(128 * 1024 * 1024);
    let mut parent = parent(&foreign);
    assert!(matches!(
        NativeInvocation::run_unit_root(unit(), "main", 10_000, &mut parent, &budget),
        Err(CaptureError::PoolMismatch)
    ));
    assert_eq!(
        parent.remaining_bytes(),
        NativeInvocation::allocation_plan()
            .unwrap()
            .requested_bytes()
    );
    drop(parent);
    for (contract, gas, expected) in [
        (unit(), 0, VMError::OutOfGas),
        (
            contract("seiyaku S { view fn main() { } }", 1),
            10_000,
            VMError::ExceededMaxCycles,
        ),
    ] {
        let mut parent = super::tests::parent(&budget);
        assert!(
            matches!(NativeInvocation::run_unit_root(contract,"main",gas,&mut parent,&budget),Err(CaptureError::Execution(error)) if error == expected)
        );
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn native_unwind_discards_the_only_packet_owner_and_refunds_original_credit() {
    let contract = unit();
    let budget = budget();
    let mut parent = parent(&budget);
    PANIC_NEXT_COMMIT.with(|flag| flag.set(true));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = NativeInvocation::run_unit_root(contract, "main", 10_000, &mut parent, &budget);
        }))
        .is_err()
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn packet_clear_scrubs_the_same_fields_used_by_drop() {
    let mut packet = NativePacket::zero();
    packet.space = 4;
    packet.generation = 15;
    packet.index = 123;
    packet.clock = 456;
    packet.write = true;
    packet.before = [0xa5; 16];
    packet.after = [0x5a; 16];
    packet.before_private = u16::MAX;
    packet.after_private = 123;
    packet.clear();
    assert!(!packet.enabled());
    assert_eq!(
        (
            packet.generation(),
            packet.index(),
            packet.clock(),
            packet.is_write()
        ),
        (0, 0, 0, false)
    );
    assert_eq!(packet.before(), &[0; 16]);
    assert_eq!(packet.after(), &[0; 16]);
    assert_eq!(packet.before_private(), 0);
    assert_eq!(packet.after_private(), 0);
}
