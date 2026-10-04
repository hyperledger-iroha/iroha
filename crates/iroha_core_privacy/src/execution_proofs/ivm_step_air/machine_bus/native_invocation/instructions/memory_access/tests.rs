//! Native STORE effects, arithmetic permissions, bounded custody and degree.

use super::super::{
    Instructions,
    tests::{artifact, check, native, root},
};
use super::*;
use ivm::{encoding::wide as enc, instruction::wide};
use packet::{
    AFTER, BEFORE, BEFORE_TAG, CLOCK, ENABLED, GENERATION, INDEX, KEY, SPACE, Space, WRITE,
};

const BOUNDS: [[u64; 2]; 2] = [[0x2000, 0x2020], [0x1000, 0x1008]];

// Synthetic tuples test the polynomial equations only; they never construct a
// NativeInvocation or establish instruction/source/history ownership.
struct Tuple {
    row: Witness,
    address: [F; 4],
    source: Fields,
    memory: Fields,
    initialized: Fields,
    selected: F,
    load: F,
    load_destination: F,
    destination: Fields,
}
impl Tuple {
    fn read(address: u64, before_mask: u16, destination: bool) -> Self {
        let mut tuple = Self::new(true, address, before_mask);
        tuple.selected = F::ZERO;
        tuple.load = F::ONE;
        tuple.load_destination = F(u64::from(destination));
        tuple.source.0.fill(F::ZERO);
        tuple.memory.0[WRITE] = F::ZERO;
        tuple.initialized.0[WRITE] = F::ZERO;
        for limb in 0..8 {
            tuple.memory.0[AFTER + limb] = tuple.memory.0[BEFORE + limb];
        }
        tuple.initialized.0[AFTER] = tuple.initialized.0[BEFORE];
        if destination {
            let half = usize::from(address & 8 != 0);
            for limb in 0..4 {
                tuple.destination.0[AFTER + limb] = tuple.memory.0[BEFORE + half * 4 + limb];
            }
        }
        tuple
    }

    fn new(selected: bool, address: u64, before_mask: u16) -> Self {
        let mut row = Witness([F::ZERO; WIDTH]);
        fill(&mut row.0, BOUNDS, selected, address, before_mask);
        let address = if selected { address } else { 0 };
        let mut source = Fields([F::ZERO; packet::WIDTH]);
        let mut memory = Fields([F::ZERO; packet::WIDTH]);
        let mut initialized = Fields([F::ZERO; packet::WIDTH]);
        if selected {
            for (port, space, generation, clock) in [
                (&mut memory, Space::Memory, 0, 70),
                (&mut initialized, Space::Initialization, 1, 71),
            ] {
                port.0[SPACE] = F(space as u64);
                port.0[GENERATION] = F(generation);
                port.0[INDEX] = F(address >> 4);
                port.0[KEY] = F((address >> 4) + (generation << 32) + ((space as u64) << 56));
                port.0[CLOCK] = F(clock);
                port.0[ENABLED] = F::ONE;
                port.0[WRITE] = F::ONE;
            }
            for i in 0..4 {
                source.0[BEFORE + i] = F(0x5678 + i as u64);
            }
            for i in 0..8 {
                memory.0[BEFORE + i] = F(i as u64 + 13);
                memory.0[AFTER + i] = if (i < 4) == (address & 8 == 0) {
                    source.0[BEFORE + i % 4]
                } else {
                    memory.0[BEFORE + i]
                };
            }
            initialized.0[BEFORE] = F(u64::from(before_mask));
            initialized.0[AFTER] = F(u64::from(
                before_mask | if address & 8 == 0 { 0xff } else { 0xff00 },
            ));
        }
        Self {
            row,
            address: core::array::from_fn(|i| F((address >> (16 * i)) & 0xffff)),
            source,
            memory,
            initialized,
            selected: F(u64::from(selected)),
            load: F::ZERO,
            load_destination: F::ZERO,
            destination: Fields([F::ZERO; packet::WIDTH]),
        }
    }
    fn residues(&self) -> Vec<F> {
        let mut out = Vec::new();
        append(
            &mut out,
            BOUNDS,
            64,
            &self.row.0,
            self.selected,
            self.load,
            self.load_destination,
            &self.address,
            &self.source.0,
            &self.destination.0,
            &self.memory.0,
            &self.initialized.0,
        );
        assert_eq!(out.len(), CONSTRAINTS);
        out
    }
    fn accepts(&self) -> bool {
        self.residues().iter().all(|x| *x == F::ZERO)
    }
}

#[test]
fn load_requires_only_the_selected_initialized_half_inside_the_root_stack() {
    for address in [0x2000, 0x2008, 0x2010, 0x2018] {
        let required = if address & 8 == 0 { 0xff } else { 0xff00 };
        for destination in [false, true] {
            assert!(Tuple::read(address, required, destination).accepts());
            assert!(Tuple::read(address, u16::MAX, destination).accepts());
            for byte in 0..16 {
                if required & (1 << byte) != 0 {
                    assert!(!Tuple::read(address, required ^ (1 << byte), destination).accepts());
                }
            }
        }
    }
    for address in [0, 0x1000, 0x1ff8, 0x2020, 0x201c, 0x2001, u64::MAX - 7] {
        assert!(
            !Tuple::read(address, u16::MAX, true).accepts(),
            "{address:x}"
        );
    }
    let mut wrong_roles = Tuple::read(0x2000, 0xff, true);
    wrong_roles.selected = F::ONE;
    assert!(!wrong_roles.accepts());
    wrong_roles.selected = F::ZERO;
    wrong_roles.load_destination = F(2);
    assert!(!wrong_roles.accepts());
}

#[test]
fn load_workspace_original_reads_and_atomic_result_reject_mutation() {
    for address in [0x2000, 0x2008] {
        let mut tuple = Tuple::read(address, u16::MAX, true);
        for field in 0..WIDTH {
            let original = tuple.row.0[field];
            tuple.row.0[field] = original.add(F::ONE);
            assert!(!tuple.accepts(), "workspace {field}");
            tuple.row.0[field] = original;
        }
        for initialization in [false, true] {
            for field in 0..packet::WIDTH {
                let fields = if initialization {
                    &mut tuple.initialized
                } else {
                    &mut tuple.memory
                };
                let original = fields.0[field];
                fields.0[field] = original.add(F::ONE);
                assert!(
                    !tuple.accepts(),
                    "initialization {initialization}, field {field}"
                );
                let fields = if initialization {
                    &mut tuple.initialized
                } else {
                    &mut tuple.memory
                };
                fields.0[field] = original;
            }
        }
        for field in (AFTER..AFTER + 4).chain([packet::AFTER_TAG]) {
            tuple.destination.0[field] = tuple.destination.0[field].add(F::ONE);
            assert!(!tuple.accepts(), "destination {field}");
            tuple.destination.0[field] = tuple.destination.0[field].sub(F::ONE);
        }
    }
}

#[test]
fn genuine_load_halves_r0_and_base_alias_keep_original_atomic_clocks() {
    let (native, budget) = native(artifact(
        &[
            enc::encode_ri(wide::arithmetic::ADDI, 4, 0, 37),
            enc::encode_store(wide::memory::STORE64, 31, 4, -16),
            enc::encode_store(wide::memory::STORE64, 31, 0, -8),
            enc::encode_ri(wide::memory::LOAD64, 5, 31, -16),
            enc::encode_ri(wide::memory::LOAD64, 6, 31, -8),
            enc::encode_ri(wide::memory::LOAD64, 0, 31, -16),
            enc::encode_ri(wide::arithmetic::ADDI, 7, 31, -16),
            enc::encode_ri(wide::memory::LOAD64, 7, 7, 0),
        ],
        16,
        false,
    ));
    let instructions = Instructions::new(&native, &root(&native), &budget).unwrap();
    for window in 0..ivm::execution_packets::INSTRUCTION_WINDOWS {
        assert!(check(&instructions, &native, window), "window {window}");
    }
    for (window, register, value) in [(3, 5, 37_u64), (4, 6, 0), (5, 0, 37), (7, 7, 37)] {
        let start = first(window);
        for offset in [6, 7] {
            let packet = &native.packets()[start + offset];
            assert!(packet.enabled());
            assert!(!packet.is_write());
            assert_eq!(packet.before(), packet.after());
        }
        let destination = &native.packets()[start + 19];
        assert_eq!(destination.enabled(), register != 0);
        if register != 0 {
            assert_eq!(destination.index(), register);
            assert_eq!(&destination.after()[..8], &value.to_le_bytes());
            assert_eq!(destination.after_private(), 0);
        }
        assert!(
            !native.packets()[start + 20].enabled(),
            "no manufactured tag event"
        );
    }
    drop(instructions);
    drop(native);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn uninitialized_or_disallowed_native_loads_publish_no_original_owner() {
    use ivm::{execution_memory::ExecutionMemoryLease, execution_packets::CaptureError};
    for (base, offset, frame, uninitialized) in [
        (31, -8, 16, true),
        (31, -24, 16, false),
        (31, -7, 16, false),
        (12, 0, 16, false),
    ] {
        let budget = AllocationBudget::new(128 * 1024 * 1024);
        let mut lease =
            ExecutionMemoryLease::reserve(&budget, NativeInvocation::allocation_plan().unwrap())
                .unwrap();
        let result = NativeInvocation::run_public_leaf_root(
            artifact(
                &[enc::encode_ri(wide::memory::LOAD64, 0, base, offset)],
                frame,
                false,
            ),
            "main",
            10_000,
            &mut lease,
            &budget,
        );
        assert!(match result {
            Err(CaptureError::Execution(ivm::VMError::MemoryAccessViolation { .. })) =>
                uninitialized,
            Err(CaptureError::Unsupported) => !uninitialized,
            _ => false,
        });
        drop(lease);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn genuine_stack_edges_repeated_halves_and_unit_result_use_original_effects() {
    let (native, budget) = native(artifact(
        &[
            enc::encode_ri(wide::arithmetic::ADDI, 4, 0, 37),
            enc::encode_store(wide::memory::STORE64, 31, 4, -128),
            enc::encode_store(wide::memory::STORE64, 31, 0, -120),
            enc::encode_store(wide::memory::STORE64, 31, 4, -120),
            enc::encode_store(wide::memory::STORE64, 31, 0, -8),
        ],
        128,
        false,
    ));
    let plan = root(&native);
    let first_pc =
        u64::from_le_bytes(native.packets()[first(0)].before()[..8].try_into().unwrap()) as usize;
    assert!(
        first_pc > 0,
        "canonical CNTR creates an absolute fetch prefix"
    );
    assert_eq!(
        first_pc + native.artifact().header_len(),
        native.artifact().code_offset()
    );
    assert_eq!(MemoryAccesses::BYTES, 216_064);
    let before = budget.reserved_bytes();
    let instructions = Instructions::new(&native, &plan, &budget).unwrap();
    assert_eq!(instructions.memory.bounds, plan.memory_bounds());
    for window in 0..ivm::execution_packets::INSTRUCTION_WINDOWS {
        assert!(check(&instructions, &native, window), "window {window}");
    }
    for window in [1, 2, 3, 4, 5] {
        assert!(native.packets()[first(window) + 6].enabled());
        assert!(native.packets()[first(window) + 7].enabled());
    }
    drop(instructions);
    assert_eq!(budget.reserved_bytes(), before);
    drop(native);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn public_stack_bounds_match_native_gas_policy_at_small_and_maximum_budgets() {
    use ivm::execution_memory::ExecutionMemoryLease;
    for gas in [10_000, 100_001, u64::MAX] {
        let budget = AllocationBudget::new(128 * 1024 * 1024);
        let mut lease =
            ExecutionMemoryLease::reserve(&budget, NativeInvocation::allocation_plan().unwrap())
                .unwrap();
        let contract = artifact(
            &[enc::encode_store(wide::memory::STORE64, 31, 0, -128)],
            128,
            false,
        );
        let native =
            NativeInvocation::run_public_leaf_root(contract, "main", gas, &mut lease, &budget)
                .unwrap();
        let plan = root(&native);
        let expected_top =
            ivm::Memory::STACK_START + ivm::IvmStackPolicy::V1.stack_limit_for_gas(gas);
        assert_eq!(plan.memory_bounds()[0], [expected_top - 128, expected_top]);
        for (clock, bound) in [(32, expected_top - 128), (33, expected_top)] {
            assert_eq!(
                u64::from_le_bytes(native.packets()[clock].after()[..8].try_into().unwrap()),
                bound
            );
        }
        let instructions = Instructions::new(&native, &plan, &budget).unwrap();
        assert!(check(&instructions, &native, 0));
        drop(instructions);
        drop(native);
        drop(lease);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn region_arithmetic_rejects_coherent_outside_crossing_misaligned_and_overflow_writes() {
    for address in [0x2000, 0x2008, 0x2010, 0x2018, 0x1000] {
        for mask in [0, 0x55aa, 0xffff] {
            assert!(Tuple::new(true, address, mask).accepts());
        }
    }
    assert!(Tuple::new(false, u64::MAX, u16::MAX).accepts());
    for address in [
        0,
        0xff8,
        0x1008,
        0x1ff8,
        0x2020,
        0x201c,
        0x2001,
        u64::MAX - 7,
    ] {
        assert!(
            !Tuple::new(true, address, 0).accepts(),
            "address {address:x}"
        );
    }
    let original = Tuple::new(true, 0x2000, 0);
    let mut out = Vec::new();
    // A public zero-sized frame and nonmatching result region grant no write,
    // even when all private tuples/workspace are otherwise coherent.
    append(
        &mut out,
        [[0x2000, 0x2000], BOUNDS[1]],
        64,
        &original.row.0,
        original.selected,
        original.load,
        original.load_destination,
        &original.address,
        &original.source.0,
        &original.destination.0,
        &original.memory.0,
        &original.initialized.0,
    );
    assert!(out.iter().any(|x| *x != F::ZERO));
}

#[test]
fn native_refused_frame_and_result_descriptors_publish_no_packet_owner() {
    use ivm::{execution_memory::ExecutionMemoryLease, execution_packets::CaptureError};
    for (base, offset, frame) in [(31, -8, 0), (31, -24, 16), (12, 8, 0), (31, -7, 16)] {
        let contract = artifact(
            &[enc::encode_store(wide::memory::STORE64, base, 0, offset)],
            frame,
            false,
        );
        let budget = AllocationBudget::new(128 * 1024 * 1024);
        let mut lease =
            ExecutionMemoryLease::reserve(&budget, NativeInvocation::allocation_plan().unwrap())
                .unwrap();
        assert!(
            matches!(
                NativeInvocation::run_public_leaf_root(
                    contract, "main", 10_000, &mut lease, &budget
                ),
                Err(CaptureError::Unsupported)
            ),
            "base {base}, offset {offset}, frame {frame}"
        );
        drop(lease);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn every_private_workspace_field_and_effect_binding_is_checked() {
    for selected in [false, true] {
        let mut tuple = Tuple::new(selected, 0x2008, 0x40);
        assert!(tuple.accepts());
        for field in 0..WIDTH {
            let original = tuple.row.0[field];
            tuple.row.0[field] = original.add(F::ONE);
            assert!(!tuple.accepts(), "selected {selected}, workspace {field}");
            tuple.row.0[field] = original;
        }
        for is_initialization in [false, true] {
            for field in 0..packet::WIDTH {
                // Overwritten prior data is authorized by the history relation.
                // Its arbitrary value is intentionally not fixed by STORE.
                if selected && !is_initialization && (BEFORE + 4..BEFORE + 8).contains(&field) {
                    continue;
                }
                let fields = if is_initialization {
                    &mut tuple.initialized
                } else {
                    &mut tuple.memory
                };
                let original = fields.0[field];
                fields.0[field] = original.add(F::ONE);
                assert!(
                    !tuple.accepts(),
                    "selected {selected}, initialization {is_initialization}, field {field}"
                );
                let fields = if is_initialization {
                    &mut tuple.initialized
                } else {
                    &mut tuple.memory
                };
                fields.0[field] = original;
            }
        }
    }
    let mut wrong_half = Tuple::new(true, 0x2000, 0);
    let upper = Tuple::new(true, 0x2008, 0);
    wrong_half.memory.0[AFTER..AFTER + 8].copy_from_slice(&upper.memory.0[AFTER..AFTER + 8]);
    wrong_half.initialized.0[AFTER] = upper.initialized.0[AFTER];
    assert!(!wrong_half.accepts());
    let mut private = Tuple::new(true, 0x2000, 0);
    private.source.0[BEFORE_TAG] = F::ONE;
    assert!(!private.accepts());
    let mut scratch = Witness([F::ONE; WIDTH]);
    scratch.clear();
    assert!(scratch.0.iter().all(|x| *x == F::ZERO));
}

#[test]
fn every_compact_slot_is_owned_once_and_all_nine_gaps_reject_events() {
    for window in 0..MAX_STEPS {
        let mut slots = [0; 32];
        for clock in instruction_clocks(window).unwrap() {
            slots[clock as usize - first(window)] += 1;
        }
        for offset in [6, 7].into_iter().chain(GAPS) {
            slots[offset] += 1;
        }
        assert_eq!(slots, [1; 32]);
        for gap in GAPS {
            for field in 0..packet::WIDTH {
                let mut fields = [F::ZERO; packet::WIDTH];
                fields[field] = F::ONE;
                let mut out = Vec::new();
                append_gap(&mut out, &fields);
                assert_eq!(out.len(), packet::WIDTH);
                assert!(
                    out.iter().any(|x| *x != F::ZERO),
                    "window {window}, gap {gap}, field {field}"
                );
            }
        }
    }
}

#[test]
fn memory_and_composed_instruction_degrees_include_all_private_fields() {
    use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
    let degree = measured_maximum_affine_degree_v1(
        [0x7a; 32],
        [WIDTH + 7 + 4 * packet::WIDTH, 0, 0, 0, 0],
        8,
        3,
        |row, _, _, _, _| {
            let start = WIDTH + 7;
            let mut out = Vec::new();
            append(
                &mut out,
                BOUNDS,
                64,
                row[..WIDTH].try_into().unwrap(),
                row[WIDTH],
                row[WIDTH + 1],
                row[WIDTH + 2],
                row[WIDTH + 3..start].try_into().unwrap(),
                row[start..start + packet::WIDTH].try_into().unwrap(),
                row[start + packet::WIDTH..start + 2 * packet::WIDTH]
                    .try_into()
                    .unwrap(),
                row[start + 2 * packet::WIDTH..start + 3 * packet::WIDTH]
                    .try_into()
                    .unwrap(),
                row[start + 3 * packet::WIDTH..].try_into().unwrap(),
            );
            Ok::<_, core::convert::Infallible>(out)
        },
    );
    assert_eq!(degree, 3);
    let mut body = vec![
        enc::encode_literal(wide::memory::LDI64, 4, 0),
        enc::encode_store(wide::memory::STORE64, 31, 4, -8),
        enc::encode_ri(wide::memory::LOAD64, 4, 31, -8),
    ];
    body.extend((u8::MIN..=u8::MAX).filter_map(|opcode| {
        let word = enc::encode_rr(opcode, 6, 4, 5);
        ivm::execution_packets::public_scalar_operands(word).map(|_| word)
    }));
    let (native, _) = native(artifact(&body, 16, true));
    let program = private_dispatch::Program::new(native.artifact().clone()).unwrap();
    let bounds = root(&native).memory_bounds();
    let dispatch_width = private_dispatch::WIDTH;
    let packet_start = dispatch_width + WIDTH;
    let dimensions = packet_start + (private_dispatch::PORTS + 2 + GAPS.len()) * packet::WIDTH;
    let degree = measured_maximum_affine_degree_v1(
        [0x7b; 32],
        [dimensions, 0, 0, 0, 0],
        8,
        4,
        |row, _, _, _, _| {
            let ports =
                private_dispatch::OriginalPackets::candidate(core::array::from_fn(|slot| {
                    row[packet_start + slot * packet::WIDTH
                        ..packet_start + (slot + 1) * packet::WIDTH]
                        .try_into()
                        .unwrap()
                }));
            let mut out = Vec::new();
            let dispatch = row[..dispatch_width].try_into().unwrap();
            private_dispatch::native_witness::append_subset_residues(
                &mut out, &program, dispatch, &ports,
            );
            let decoded = private_dispatch::append_residues(
                &mut out,
                &program,
                private_dispatch::Schedule::new(0, instruction_clocks(0).unwrap()).unwrap(),
                dispatch,
                &ports,
            );
            let effects = packet_start + private_dispatch::PORTS * packet::WIDTH;
            append(
                &mut out,
                bounds,
                64,
                row[dispatch_width..packet_start].try_into().unwrap(),
                decoded.store,
                decoded.load,
                decoded.load_destination,
                &decoded.memory_address,
                decoded.store_value,
                decoded.destination,
                row[effects..effects + packet::WIDTH].try_into().unwrap(),
                row[effects + packet::WIDTH..effects + 2 * packet::WIDTH]
                    .try_into()
                    .unwrap(),
            );
            for gap in 0..GAPS.len() {
                let at = effects + (gap + 2) * packet::WIDTH;
                append_gap(&mut out, row[at..at + packet::WIDTH].try_into().unwrap());
            }
            Ok::<_, core::convert::Infallible>(out)
        },
    );
    assert_eq!(degree, 4);
}
