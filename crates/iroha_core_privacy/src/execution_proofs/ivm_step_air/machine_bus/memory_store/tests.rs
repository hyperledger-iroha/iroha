//! Integer witness construction, direct mutation checks and actual native stores.
use super::*;
use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
use ivm::{IVM, Memory, Perm, ProgramMetadata, VMError, encoding::wide as enc, ivm_mode};
use packet::{AFTER, AFTER_TAG, BEFORE, BEFORE_TAG, ENABLED, Event, Space};

const LOW: u64 = 0x0123_4567_89ab_cdef;
const HIGH: u64 = 0xfedc_ba98_7654_3210;
const OLD: [u8; 16] = [0xa5; 16];

fn bytes(low: u64) -> [u8; 16] {
    let mut out = [0; 16];
    out[..8].copy_from_slice(&low.to_le_bytes());
    out
}
fn phase(word: u32, address: u64, outcome: MemoryOutcome) -> StorePhase {
    StorePhase::from_fixed_input(
        word,
        address,
        7,
        [10, 11, 12],
        (100, 0, 0, Memory::STACK_START + Memory::MIN_STACK_SIZE, 0),
        (true, false),
        outcome,
    )
    .unwrap()
}
fn scalar(address: u64, source: u8) -> StorePhase {
    phase(
        enc::encode_store(wide::memory::STORE64, 3, source, 0),
        address,
        MemoryOutcome::Ready,
    )
}
fn registers(p: StorePhase) -> [u64; 256] {
    let mut values = [0; 256];
    values[1] = LOW;
    values[2] = HIGH;
    values[3] = p.base_value;
    values
}
/// Both halves of one physical cell, read back without a sixteen-byte access.
fn cell(vm: &IVM, address: u64) -> [u8; 16] {
    let mut out = [0; 16];
    out[..8].copy_from_slice(&vm.memory.load_u64(address).unwrap().to_le_bytes());
    out[8..].copy_from_slice(&vm.memory.load_u64(address + 8).unwrap().to_le_bytes());
    out
}

#[derive(Clone)]
struct Fixture {
    phase: StorePhase,
    row: [F; WIDTH],
    reads: [[F; packet::WIDTH]; 2],
    memory: [F; packet::WIDTH],
    log: [F; WRITE_LOG_WIDTH],
    control: [F; CONTROL_WIDTH],
}
impl Fixture {
    // The Boolean/integer oracle is independent of the polynomial equations.
    fn new(p: StorePhase, mask: u16, values: &[u64; 256], tags: [bool; 256]) -> Self {
        Self::with_memory(p, mask, values, tags, OLD)
    }
    fn with_memory(
        p: StorePhase,
        mask: u16,
        values: &[u64; 256],
        tags: [bool; 256],
        before: [u8; 16],
    ) -> Self {
        let mut reads = [[F::ZERO; packet::WIDTH]; 2];
        for (slot, read) in reads.iter_mut().enumerate() {
            let index = p.registers[slot];
            *read = Event {
                space: Space::Register,
                vm: p.vm,
                generation: 0,
                index: u32::try_from(index).expect("bounded fixture value"),
                write: false,
                before: bytes(values[index]),
                after: bytes(values[index]),
                before_private: u16::from(tags[index]),
                after_private: u16::from(tags[index]),
            }
            .fields(p.clocks[slot] as usize);
        }
        let private = tags[p.registers[1]];
        let private_error = private && !p.stack;
        let committed = !private_error && p.outcome == MemoryOutcome::Ready;
        let row = witness(mask, committed);
        let mut memory = [F::ZERO; packet::WIDTH];
        let mut log = [F::ZERO; WRITE_LOG_WIDTH];
        if committed {
            let start = (p.address & 8) as usize;
            let payload = values[p.registers[1]].to_le_bytes();
            let mut after = before;
            after[start..start + 8].copy_from_slice(&payload);
            let selected = 0xff_u16 << start;
            let after_mask = (mask & !selected) | if private { selected } else { 0 };
            memory = Event {
                space: Space::Memory,
                vm: p.vm,
                generation: 0,
                index: u32::from_le_bytes(
                    u64::to_le_bytes(p.address >> 4)[..4]
                        .try_into()
                        .expect("low-byte width"),
                ),
                write: true,
                before,
                after,
                before_private: mask,
                after_private: after_mask,
            }
            .fields(p.clocks[2] as usize);
            log[..4].copy_from_slice(&[
                F(p.address & 0xffff_ffff),
                F(p.address >> 32),
                F(8),
                F::ONE,
            ]);
            for (i, pair) in payload.as_chunks::<2>().0.iter().enumerate() {
                log[4 + i] = F(u64::from(u16::from_le_bytes([pair[0], pair[1]])));
            }
        }
        let mut control = [F::ZERO; CONTROL_WIDTH];
        for (offset, value) in [
            (0, p.gas_after),
            (2, p.pc.wrapping_add(if committed { 4 } else { 0 })),
            (4, p.cycles + u64::from(committed)),
            (
                6,
                if committed {
                    p.output_after
                } else {
                    p.output_before
                },
            ),
        ] {
            control[offset] = F(value & 0xffff_ffff);
            control[offset + 1] = F(value >> 32);
        }
        control[COMMIT] = F(u64::from(committed));
        control[PRIVACY_TRAP] = F(u64::from(private_error));
        control[ACCESS_TRAP] = F(u64::from(
            !private_error && p.outcome == MemoryOutcome::AccessRefused,
        ));
        control[LOCAL_DEFER] = F(u64::from(
            !private_error && p.outcome == MemoryOutcome::AllocationDeferred,
        ));
        Self {
            phase: p,
            row,
            reads,
            memory,
            log,
            control,
        }
    }
    fn residues(&self) -> Vec<F> {
        let mut out = Vec::new();
        append_residues(
            &mut out,
            self.phase,
            &self.row,
            [&self.reads[0], &self.reads[1]],
            &self.memory,
            &self.log,
            &self.control,
        );
        assert_eq!(out.len(), CONSTRAINTS);
        out
    }
    fn accepts(&self) -> bool {
        self.residues().into_iter().all(|v| v == F::ZERO)
    }
}

#[test]
fn store_truth_tables_preserve_half_masks_and_all_typed_effects() {
    for address in [Memory::HEAP_START, Memory::STACK_START] {
        for p in [scalar(address, 1), scalar(address + 8, 1)] {
            for outcome in [
                MemoryOutcome::Ready,
                MemoryOutcome::AccessRefused,
                MemoryOutcome::AllocationDeferred,
            ] {
                let p = StorePhase { outcome, ..p };
                for private in [false, true] {
                    let mut tags = [false; 256];
                    tags[1] = private;
                    // An unrelated private register never enters the truth table.
                    tags[2] = true;
                    for mask in [0, 1, 0xff, 0xff00, 0xaaaa, 0xffff] {
                        let f = Fixture::new(p, mask, &registers(p), tags);
                        assert!(f.accepts());
                        assert_eq!(
                            f.control[COMMIT]
                                .add(f.control[PRIVACY_TRAP])
                                .add(f.control[ACCESS_TRAP])
                                .add(f.control[LOCAL_DEFER]),
                            F::ONE
                        );
                        assert_eq!(
                            f.control[PRIVACY_TRAP],
                            F(u64::from(private && address == Memory::HEAP_START))
                        );
                        if f.control[COMMIT] == F::ZERO {
                            assert_eq!(f.memory, [F::ZERO; packet::WIDTH]);
                            assert_eq!(f.log, [F::ZERO; WRITE_LOG_WIDTH]);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_store_mask_effect_log_control_and_read_tuple_mutation_is_rejected() {
    for p in [
        scalar(Memory::STACK_START, 1),
        scalar(Memory::STACK_START + 8, 1),
    ] {
        for private in [false, true] {
            let mut tags = [false; 256];
            tags[1] = private;
            let f = Fixture::new(p, 0xa55a, &registers(p), tags);
            assert!(f.accepts());
            for i in 0..WIDTH {
                let mut m = f.clone();
                m.row[i] = m.row[i].add(F::ONE);
                assert!(!m.accepts(), "mask {i}");
            }
            for i in 0..packet::WIDTH {
                let mut m = f.clone();
                m.memory[i] = m.memory[i].add(F::ONE);
                let replaced_old_limb = (BEFORE..BEFORE + 8).contains(&i)
                    && p.selected_bits().contains(&((i - BEFORE) * 2));
                // Overwritten prior bytes are linked by typed bus history, not
                // by the store arithmetic. Never claim that missing authority.
                assert_eq!(m.accepts(), replaced_old_limb, "memory {i}");
            }
            for i in 0..WRITE_LOG_WIDTH {
                let mut m = f.clone();
                m.log[i] = m.log[i].add(F::ONE);
                assert!(!m.accepts(), "log {i}");
            }
            for i in 0..CONTROL_WIDTH {
                let mut m = f.clone();
                m.control[i] = m.control[i].add(F::ONE);
                assert!(!m.accepts(), "control {i}");
            }
            for slot in 0..2 {
                for i in 0..packet::WIDTH {
                    let mut m = f.clone();
                    m.reads[slot][i] = m.reads[slot][i].add(F::ONE);
                    assert!(!m.accepts(), "read {slot}/{i}");
                }
            }
        }
    }
}

#[test]
fn rejected_stores_cannot_erase_operand_reads_or_commit_memory_register_pc_cycles() {
    for outcome in [
        MemoryOutcome::Ready,
        MemoryOutcome::AccessRefused,
        MemoryOutcome::AllocationDeferred,
    ] {
        let mut p = scalar(Memory::HEAP_START, 1);
        p.outcome = outcome;
        let mut tags = [false; 256];
        tags[1] = true;
        let f = Fixture::new(p, 0xffff, &registers(p), tags);
        assert!(f.accepts());
        assert_eq!(f.control[PRIVACY_TRAP], F::ONE);
        assert_eq!(f.control[ACCESS_TRAP], F::ZERO);
        assert_eq!(f.control[LOCAL_DEFER], F::ZERO);
        for slot in 0..2 {
            let mut m = f.clone();
            m.reads[slot] = [F::ZERO; packet::WIDTH];
            assert!(!m.accepts());
        }
        for i in 0..packet::WIDTH {
            let mut m = f.clone();
            m.memory[i] = F::ONE;
            assert!(!m.accepts());
        }
        for i in [2, 4, 6, COMMIT, ACCESS_TRAP, LOCAL_DEFER] {
            let mut m = f.clone();
            m.control[i] = m.control[i].add(F::ONE);
            assert!(!m.accepts());
        }
        let mut m = f.clone();
        m.reads[1][packet::WRITE] = F::ONE;
        assert!(!m.accepts());
    }
}

#[test]
fn source_aliases_r0_signed_immediates_and_fixed_boundary_cannot_be_substituted() {
    for source in 0..4 {
        let p = scalar(Memory::HEAP_START, source);
        let f = Fixture::new(p, 0, &registers(p), [false; 256]);
        assert!(f.accepts());
        // r0 reads zero, and a source equal to the base register reads the base.
        if source == 0 || source == 3 {
            let mut m = f.clone();
            m.reads[1][BEFORE] = m.reads[1][BEFORE].add(F::ONE);
            m.reads[1][AFTER] = m.reads[1][BEFORE];
            assert!(!m.accepts());
        }
    }
    // Preserve the forged source's read-preservation tuple AND every derived
    // write/log byte. Only equality to the earlier same-register read may fail.
    let p = scalar(Memory::HEAP_START, 3);
    let original = Fixture::new(p, 0, &registers(p), [false; 256]);
    assert!(original.accepts());
    let mut changed = original;
    changed.reads[1][BEFORE] = changed.reads[1][BEFORE].add(F::ONE);
    changed.reads[1][AFTER] = changed.reads[1][BEFORE];
    changed.memory[AFTER] = changed.reads[1][BEFORE];
    changed.log[4] = changed.reads[1][BEFORE];
    let failed = changed
        .residues()
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (*value != F::ZERO).then_some(index))
        .collect::<Vec<_>>();
    // Two read-port blocks and the five fixed-base equations precede the one
    // nine-field base/value alias link. This is its limb0.
    let alias_start = 2 * (8 + 4 + 9 + 1 + 9) + 5;
    assert_eq!(failed, [alias_start]);
    assert!(!changed.accepts());
    for immediate in i8::MIN..=i8::MAX {
        let word = enc::encode_store(wide::memory::STORE64, 3, 1, immediate);
        let p = phase(word, Memory::HEAP_START, MemoryOutcome::Ready);
        assert_eq!(
            p.base_value
                .wrapping_add(i64::cast_unsigned(i64::from(immediate))),
            p.address
        );
        let f = Fixture::new(p, 0, &registers(p), [false; 256]);
        assert!(f.accepts());
        let mut m = f;
        m.reads[0][BEFORE] = m.reads[0][BEFORE].add(F::ONE);
        m.reads[0][AFTER] = m.reads[0][BEFORE];
        assert!(!m.accepts());
    }
    let word = enc::encode_store(wide::memory::STORE64, 3, 1, 0);
    let make = |instruction, address, gas, cycles, zk, private, clocks, cursor| {
        StorePhase::from_fixed_input(
            instruction,
            address,
            7,
            clocks,
            (
                gas,
                0,
                cycles,
                Memory::STACK_START + Memory::MIN_STACK_SIZE,
                cursor,
            ),
            (zk, private),
            MemoryOutcome::Ready,
        )
    };
    for address in [Memory::HEAP_START, Memory::HEAP_START + 8] {
        assert!(make(word, address, 3, 0, true, false, [1, 2, 3], 0).is_some());
    }
    let load = enc::encode_load(wide::memory::LOAD64, 1, 3, 0);
    for (w, a, g, c, z, p, t, o) in [
        (load, Memory::HEAP_START, 100, 0, true, false, [1, 2, 3], 0),
        (
            enc::encode_halt(),
            Memory::HEAP_START,
            100,
            0,
            true,
            false,
            [1, 2, 3],
            0,
        ),
        (
            word,
            Memory::HEAP_START + 1,
            100,
            0,
            true,
            false,
            [1, 2, 3],
            0,
        ),
        (
            word,
            Memory::HEAP_START + 4,
            100,
            0,
            true,
            false,
            [1, 2, 3],
            0,
        ),
        (word, Memory::HEAP_START, 2, 0, true, false, [1, 2, 3], 0),
        (
            word,
            Memory::HEAP_START,
            100,
            u64::MAX,
            true,
            false,
            [1, 2, 3],
            0,
        ),
        (word, Memory::HEAP_START, 100, 0, false, false, [1, 2, 3], 0),
        (word, Memory::HEAP_START, 100, 0, true, true, [1, 2, 3], 0),
        (word, Memory::HEAP_START, 100, 0, true, false, [1, 2, 2], 0),
        (word, 1 << 36, 100, 0, true, false, [1, 2, 3], 0),
        (
            word,
            Memory::OUTPUT_START,
            100,
            0,
            true,
            false,
            [1, 2, 3],
            8,
        ),
        (
            word,
            Memory::HEAP_START,
            100,
            0,
            true,
            false,
            [1, 2, 3],
            Memory::OUTPUT_SIZE + 1,
        ),
    ] {
        assert!(make(w, a, g, c, z, p, t, o).is_none());
    }
}

fn native(words: &[u32], gas: u64) -> IVM {
    let mut program = ProgramMetadata {
        mode: ivm_mode::ZK,
        max_cycles: 2,
        ..ProgramMetadata::default()
    }
    .encode();
    for word in words {
        program.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm = IVM::new(gas);
    vm.load_program(&program).unwrap();
    vm.set_zk_trace_enabled(true);
    vm
}
fn assert_native_reads(vm: &IVM, fixture: &Fixture) {
    let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
    let snapshot = vm.try_diagnostic_snapshot(&budget).unwrap();
    let actual = snapshot
        .register_events()
        .map(|e| (e.written, e.index, e.value, e.tag))
        .collect::<Vec<_>>();
    let expected = fixture
        .reads
        .iter()
        .filter(|p| p[ENABLED] == F::ONE)
        .map(|p| {
            (
                false,
                usize::try_from(p[packet::INDEX].0).expect("bounded fixture value"),
                packet::half(p, BEFORE, 0),
                p[BEFORE_TAG] == F::ONE,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "complete ordered RegEvent sequence, including r0 and aliases"
    );
}
fn assert_native_write(vm: &IVM, f: &Fixture) {
    let log = vm.memory.try_write_log_snapshot().unwrap();
    if f.control[COMMIT] == F::ONE {
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].address(), f.phase.address);
        let payload: Vec<u8> = f.log[4..]
            .iter()
            .flat_map(|v| {
                u16::from_le_bytes(v.0.to_le_bytes()[..2].try_into().expect("low-word width"))
                    .to_le_bytes()
            })
            .collect();
        assert_eq!(log[0].bytes(), &payload[..8]);
        assert_eq!(payload[8..], [0; 8]);
    } else {
        assert!(log.is_empty());
    }
    assert!(
        !vm.memory
            .try_read_log_snapshot()
            .unwrap()
            .iter()
            .any(|r| r.addr == f.phase.address && r.len == 8)
    );
}

#[test]
fn native_stores_match_ordered_alias_reads_exact_write_bytes_and_unchanged_registers() {
    for half in [0_i8, 8] {
        for source in [0_u8, 1, 3] {
            let address = Memory::HEAP_START + 0x80;
            let word = enc::encode_store(wide::memory::STORE64, 3, source, half);
            let p = phase(
                word,
                address + i64::from(half).cast_unsigned(),
                MemoryOutcome::Ready,
            );
            let values = registers(p);
            let f = Fixture::new(p, 0, &values, [false; 256]);
            assert!(f.accepts());
            let mut vm = native(&[word, enc::encode_halt()], 100);
            vm.memory.store_bytes(address, &OLD).unwrap();
            for (i, &value) in values.iter().enumerate().take(4) {
                vm.set_register(i, value);
            }
            let registers_before: [(u64, bool); 256] =
                std::array::from_fn(|i| (vm.registers.get(i), vm.registers.tag(i)));
            vm.memory.clear_tracking();
            vm.run().unwrap();
            assert_native_reads(&vm, &f);
            assert_native_write(&vm, &f);
            for (i, (value, tag)) in registers_before.into_iter().enumerate() {
                assert_eq!(vm.registers.get(i), value, "unchanged register r{i}");
                assert_eq!(vm.registers.tag(i), tag, "unchanged register tag r{i}");
            }
            let expected: Vec<u8> = f.memory[AFTER..AFTER + 8]
                .iter()
                .flat_map(|v| {
                    u16::from_le_bytes(v.0.to_le_bytes()[..2].try_into().expect("low-word width"))
                        .to_le_bytes()
                })
                .collect();
            assert_eq!(cell(&vm, address).as_slice(), expected.as_slice());
            assert_eq!(
                (vm.gas_remaining, vm.pc(), vm.get_cycle_count()),
                (97, 8, 2)
            );
        }
    }
}

#[test]
fn native_late_privacy_and_permission_failures_keep_reads_but_no_store_effects() {
    for address in [Memory::STACK_START, Memory::HEAP_START] {
        for private in [false, true] {
            let p = scalar(address, 1);
            let word = enc::encode_store(wide::memory::STORE64, 3, 1, 0);
            let mut tags = [false; 256];
            tags[1] = private;
            let values = registers(p);
            let f = Fixture::new(p, 0, &values, tags);
            assert!(f.accepts());
            let mut vm = native(&[word, enc::encode_halt()], 100);
            vm.memory.store_bytes(address, &OLD).unwrap();
            for (i, &value) in values.iter().enumerate().take(4) {
                vm.set_register(i, value);
                vm.registers.set_tag(i, tags[i]);
            }
            let registers_before: [(u64, bool); 256] =
                std::array::from_fn(|i| (vm.registers.get(i), vm.registers.tag(i)));
            vm.memory.clear_tracking();
            let result = vm.run();
            let ok = f.control[COMMIT] == F::ONE;
            assert_eq!(ok, !private || address == Memory::STACK_START);
            assert!(matches!(
                (ok, result),
                (true, Ok(())) | (false, Err(VMError::PrivacyViolation))
            ));
            assert_native_reads(&vm, &f);
            assert_native_write(&vm, &f);
            assert_eq!(
                (vm.gas_remaining, vm.pc(), vm.get_cycle_count()),
                (97, if ok { 8 } else { 0 }, if ok { 2 } else { 0 })
            );
            for (i, (value, tag)) in registers_before.into_iter().enumerate() {
                assert_eq!(vm.registers.get(i), value, "unchanged register r{i}");
                assert_eq!(vm.registers.tag(i), tag, "unchanged register tag r{i}");
            }
            if !ok {
                assert_eq!(cell(&vm, address), OLD);
            }
        }
    }
    let word = enc::encode_store(wide::memory::STORE64, 3, 1, 1);
    let p = phase(word, 0, MemoryOutcome::AccessRefused);
    assert_eq!(p.base_value, u64::MAX);
    let values = registers(p);
    let f = Fixture::new(p, 0, &values, [false; 256]);
    assert!(f.accepts());
    let mut vm = native(&[word, enc::encode_halt()], 100);
    let code_before = vm.memory.load_u64(0).unwrap();
    for (i, &value) in values.iter().enumerate().take(4) {
        vm.set_register(i, value);
    }
    vm.memory.clear_tracking();
    assert!(match (
        &(vm.run()),
        &(VMError::MemoryAccessViolation {
            addr: 0,
            perm: Perm::WRITE
        })
    ) {
        (
            Err(VMError::MemoryAccessViolation {
                addr: actual_field_0,
                perm: actual_field_1,
            }),
            VMError::MemoryAccessViolation {
                addr: expected_field_0,
                perm: expected_field_1,
            },
        ) => actual_field_0 == expected_field_0 && actual_field_1 == expected_field_1,
        _ => false,
    });
    assert_native_reads(&vm, &f);
    assert!(vm.memory.try_write_log_snapshot().unwrap().is_empty());
    assert_eq!(vm.memory.load_u64(0).unwrap(), code_before);
    assert_eq!(
        (vm.gas_remaining, vm.pc(), vm.get_cycle_count()),
        (97, 0, 0)
    );
}

#[test]
fn native_public_half_overwrite_clears_only_selected_private_bytes_and_output_rewind_is_atomic() {
    for half in [0_i8, 8] {
        let words = [
            enc::encode_store(wide::memory::STORE64, 3, 1, 0),
            enc::encode_halt(),
            enc::encode_store(wide::memory::STORE64, 3, 2, 8),
            enc::encode_halt(),
            enc::encode_store(wide::memory::STORE64, 3, 4, half),
            enc::encode_halt(),
            enc::encode_load(wide::memory::LOAD64, 5, 3, 0),
            enc::encode_halt(),
            enc::encode_load(wide::memory::LOAD64, 6, 3, 8),
            enc::encode_halt(),
        ];
        let mut vm = native(&words, 1000);
        vm.set_register(1, LOW);
        vm.set_register(2, HIGH);
        vm.set_register(3, Memory::STACK_START);
        vm.set_register(4, 7);
        vm.registers.set_tag(1, true);
        vm.registers.set_tag(2, true);
        // Two private eight-byte stores make the whole physical cell private.
        vm.run().unwrap();
        vm.set_program_counter(8).unwrap();
        vm.run().unwrap();
        let mut p = phase(
            words[4],
            Memory::STACK_START + i64::from(half).cast_unsigned(),
            MemoryOutcome::Ready,
        );
        p.pc = 16;
        p.cycles = 0;
        p.gas_after = vm.gas_remaining - 3;
        let values = core::array::from_fn(|i| vm.registers.get(i));
        let tags = core::array::from_fn(|i| vm.registers.tag(i));
        let old_bytes = cell(&vm, Memory::STACK_START);
        assert_eq!(old_bytes[..8], LOW.to_le_bytes());
        assert_eq!(old_bytes[8..], HIGH.to_le_bytes());
        let f = Fixture::with_memory(p, u16::MAX, &values, tags, old_bytes);
        assert!(f.accepts());
        let expected_mask = if half == 0 { 0xff00 } else { 0xff };
        assert_eq!(f.memory[AFTER_TAG], F(expected_mask));
        vm.set_program_counter(16).unwrap();
        vm.memory.clear_tracking();
        vm.run().unwrap();
        assert_native_reads(&vm, &f);
        assert_native_write(&vm, &f);
        vm.set_program_counter(24).unwrap();
        vm.run().unwrap();
        vm.set_program_counter(32).unwrap();
        vm.run().unwrap();
        assert_eq!(vm.registers.get(5), if half == 0 { 7 } else { LOW });
        assert_eq!(vm.registers.get(6), if half == 8 { 7 } else { HIGH });
        assert_eq!(vm.registers.tag(5), half != 0);
        assert_eq!(vm.registers.tag(6), half != 8);
    }
    let word = enc::encode_store(wide::memory::STORE64, 3, 1, 0);
    let mut vm = native(&[word, enc::encode_halt(), word, enc::encode_halt()], 100);
    vm.set_register(1, LOW);
    vm.set_register(3, Memory::OUTPUT_START + 16);
    let forward = phase(word, Memory::OUTPUT_START + 16, MemoryOutcome::Ready);
    let forward_fixture = Fixture::with_memory(
        forward,
        0,
        &core::array::from_fn(|i| vm.registers.get(i)),
        [false; 256],
        [0; 16],
    );
    assert!(forward_fixture.accepts());
    assert_eq!(forward_fixture.control[6], F(24));
    vm.memory.clear_tracking();
    vm.run().unwrap();
    assert_native_reads(&vm, &forward_fixture);
    assert_native_write(&vm, &forward_fixture);
    assert_eq!(vm.memory.output_used_len(), 24);
    assert_eq!(&vm.memory.read_output_used()[..16], &[0; 16]);
    let old = vm.memory.read_output_used().to_vec();
    vm.set_program_counter(8).unwrap();
    vm.set_register(3, Memory::OUTPUT_START + 8);
    let mut p = phase(word, Memory::OUTPUT_START + 8, MemoryOutcome::AccessRefused);
    p.output_before = 24;
    p.pc = 8;
    p.cycles = 0;
    p.gas_after = vm.gas_remaining - 3;
    let f = Fixture::new(
        p,
        0,
        &core::array::from_fn(|i| vm.registers.get(i)),
        [false; 256],
    );
    assert!(f.accepts());
    vm.memory.clear_tracking();
    assert!(match (
        &(vm.run()),
        &(VMError::MemoryAccessViolation {
            addr: u32::from_le_bytes(
                u64::to_le_bytes(Memory::OUTPUT_START + 8)[..4]
                    .try_into()
                    .expect("low-byte width")
            ),
            perm: Perm::WRITE
        })
    ) {
        (
            Err(VMError::MemoryAccessViolation {
                addr: actual_field_0,
                perm: actual_field_1,
            }),
            VMError::MemoryAccessViolation {
                addr: expected_field_0,
                perm: expected_field_1,
            },
        ) => actual_field_0 == expected_field_0 && actual_field_1 == expected_field_1,
        _ => false,
    });
    assert_native_reads(&vm, &f);
    assert_native_write(&vm, &f);
    assert_eq!(vm.memory.read_output_used(), old.as_slice());
    assert_eq!(vm.memory.output_used_len(), 24);
    assert_eq!(
        (vm.gas_remaining, vm.pc(), vm.get_cycle_count()),
        (94, 8, 0)
    );
}

#[test]
fn store_bank_degree_and_geometry_are_explicit_without_complete_memory_admission() {
    assert_eq!(WIDTH, 16);
    assert_eq!(CONSTRAINTS, 176);
    let ports = 3 * packet::WIDTH + WRITE_LOG_WIDTH + CONTROL_WIDTH;
    for p in [
        scalar(Memory::STACK_START, 1),
        scalar(Memory::HEAP_START, 1),
        scalar(Memory::HEAP_START + 8, 3),
    ] {
        // The single value tag gates the off-stack effect once; no product of
        // two operand tags remains.
        assert_eq!(
            measured_maximum_affine_degree_v1(
                [u8::from(p.stack); 32],
                [WIDTH + ports, 0, 0, 0, 0],
                4,
                2,
                |row, _, _, _, _| {
                    let base_start = WIDTH;
                    let value_start = base_start + packet::WIDTH;
                    let memory_start = value_start + packet::WIDTH;
                    let write_log_start = memory_start + packet::WIDTH;
                    let control_start = write_log_start + WRITE_LOG_WIDTH;
                    let mut out = Vec::new();
                    append_residues(
                        &mut out,
                        p,
                        row[..base_start].try_into().unwrap(),
                        [
                            row[base_start..value_start].try_into().unwrap(),
                            row[value_start..memory_start].try_into().unwrap(),
                        ],
                        row[memory_start..write_log_start].try_into().unwrap(),
                        row[write_log_start..control_start].try_into().unwrap(),
                        row[control_start..].try_into().unwrap(),
                    );
                    Ok::<_, core::convert::Infallible>(out)
                }
            ),
            2
        );
    }
    assert_eq!(WIDTH * (136 * 2 * 8 + 2 * 32), 35_840);
    assert_eq!((WIDTH + ports) * (136 * 2 * 8 + 2 * 32), 264_320);
    assert!(
        (WIDTH + ports) * (136 * 2 * 8 + 2 * 32) > 41_152,
        "packet/log/control linkage is not free profile space"
    );
    // Two different fixed memory dispositions both admit their own exact
    // conditional outcomes. This is intentionally NOT proof of permission.
    let ready = scalar(Memory::HEAP_START, 1);
    let refused = StorePhase {
        outcome: MemoryOutcome::AccessRefused,
        ..ready
    };
    let a = Fixture::new(ready, 0, &registers(ready), [false; 256]);
    let b = Fixture::new(refused, 0, &registers(refused), [false; 256]);
    assert!(a.accepts() && b.accepts());
    assert_ne!(a.control, b.control);
    let mut mixed = a;
    mixed.phase = refused;
    assert!(!mixed.accepts());
}

#[test]
fn native_early_trap_priority_and_late_alignment_keep_exact_read_prefix() {
    let word = enc::encode_store(wide::memory::STORE64, 3, 1, 0);
    let bad = Memory::HEAP_START + 1;
    let cost = 3;
    for (base_private, value_private, expected, reads) in [
        (true, true, VMError::PrivacyViolation, 0),
        (false, true, VMError::PrivacyViolation, 2),
        (
            false,
            false,
            VMError::MisalignedAccess {
                addr: u32::from_le_bytes(
                    u64::to_le_bytes(bad)[..4]
                        .try_into()
                        .expect("low-byte width"),
                ),
            },
            2,
        ),
    ] {
        for out_of_gas in [false, true] {
            let gas = if out_of_gas { cost - 1 } else { 100 };
            let mut vm = native(&[word, enc::encode_halt()], gas);
            vm.set_register(1, LOW);
            vm.set_register(2, HIGH);
            vm.set_register(3, bad);
            vm.registers.set_tag(1, value_private);
            vm.registers.set_tag(3, base_private);
            vm.memory.clear_tracking();
            let result = vm.run();
            if out_of_gas {
                assert!(matches!(result, Err(VMError::OutOfGas)));
            } else {
                assert!(match (result, &expected) {
                    (Err(VMError::PrivacyViolation), VMError::PrivacyViolation) => true,
                    (
                        Err(VMError::MisalignedAccess { addr: actual }),
                        VMError::MisalignedAccess { addr: expected },
                    ) => actual == *expected,
                    _ => false,
                });
            }
            let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
            let snapshot = vm.try_diagnostic_snapshot(&budget).unwrap();
            let actual = snapshot
                .register_events()
                .map(|e| (e.written, e.index, e.value, e.tag))
                .collect::<Vec<_>>();
            let prefix = [
                (false, 3, bad, base_private),
                (false, 1, LOW, value_private),
            ];
            assert_eq!(actual, prefix[..if out_of_gas { 0 } else { reads }]);
            assert_eq!((vm.pc(), vm.get_cycle_count()), (0, 0));
            assert_eq!(vm.gas_remaining, if out_of_gas { gas } else { gas - cost });
            assert!(vm.memory.try_write_log_snapshot().unwrap().is_empty());
            assert_eq!(vm.memory.output_used_len(), 0);
            assert_eq!(cell(&vm, Memory::HEAP_START), [0; 16]);
        }
    }
}
