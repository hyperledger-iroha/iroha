//! Direct adversarial witnesses and real native load observations.

use super::*;
use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
use ivm::{IVM, Memory, ProgramMetadata, encoding::wide as enc, ivm_mode};
use packet::{AFTER, AFTER_TAG, BEFORE, BEFORE_TAG, ENABLED, Event, Space};

/// Architectural register file size of the fixture register arrays.
const REGISTERS: usize = ivm::parallel::REGISTER_COUNT;
const LOW: u64 = 0x0123_4567_89ab_cdef;
const HIGH: u64 = 0xfedc_ba98_7654_3210;

#[derive(Clone)]
struct Fixture {
    phase: ReadPhase,
    row: [F; WIDTH],
    read: [F; packet::WIDTH],
    writes: [[F; packet::WIDTH]; 2],
    read_log: [F; READ_LOG_WIDTH],
    control: [F; CONTROL_WIDTH],
}

fn phase(instruction: u32, address: u64) -> ReadPhase {
    ReadPhase::from_fixed_input(
        instruction,
        address,
        7,
        [10, 11, 12],
        (100, 32, 4, Memory::STACK_START + Memory::MIN_STACK_SIZE),
        (true, false),
    )
    .unwrap()
}

fn scalar(address: u64, destination: u8) -> ReadPhase {
    phase(
        enc::encode_load(wide::memory::LOAD64, destination, 3, 0),
        address,
    )
}

fn bytes(low: u64, high: u64) -> [u8; 16] {
    (u128::from(low) | (u128::from(high) << 64)).to_le_bytes()
}

impl Fixture {
    // Integer/reference construction deliberately does not consult the AIR's
    // zero/full flags or calculated commit expression. The two values are the
    // low and high halves of the addressed sixteen-byte cell.
    fn new(phase: ReadPhase, mask: u16, values: [u64; 2]) -> Self {
        Self::with_registers(
            phase,
            mask,
            values,
            core::array::from_fn(|i| if i == 0 { 0 } else { 0x55 + i as u64 }),
            [false; REGISTERS],
        )
    }

    fn with_registers(
        phase: ReadPhase,
        mask: u16,
        values: [u64; 2],
        mut registers: [u64; REGISTERS],
        mut tags: [bool; REGISTERS],
    ) -> Self {
        let row = witness(mask, phase);
        let selected = phase.selected_bits();
        let selected_mask = 0xff_u16 << selected.start;
        let private = mask & selected_mask;
        let ok = private == 0 || (private == selected_mask && phase.stack);
        let tag = private != 0;
        let payload = bytes(values[0], values[1]);
        let read = Event {
            space: Space::Memory,
            vm: phase.vm,
            generation: 0,
            index: u32::from_le_bytes(
                u64::to_le_bytes(phase.address >> 4)[..4]
                    .try_into()
                    .expect("low-byte width"),
            ),
            write: false,
            before: payload,
            after: payload,
            before_private: mask,
            after_private: mask,
        }
        .fields(phase.clocks[0] as usize);
        let mut writes = [[F::ZERO; packet::WIDTH]; 2];
        let destination = phase.destination;
        if ok && destination != 0 {
            // Native order: the value write keeps the old tag, then the tag write.
            for (slot, write) in writes.iter_mut().enumerate() {
                let old_value = registers[destination];
                let old_tag = tags[destination];
                if slot == 0 {
                    registers[destination] = values[selected.start / 8];
                } else {
                    tags[destination] = tag;
                }
                *write = Event {
                    space: Space::Register,
                    vm: phase.vm,
                    generation: 0,
                    index: u32::try_from(destination).expect("bounded fixture value"),
                    write: true,
                    before: bytes(old_value, 0),
                    after: bytes(registers[destination], 0),
                    before_private: u16::from(old_tag),
                    after_private: u16::from(tags[destination]),
                }
                .fields(phase.clocks[slot + 1] as usize);
            }
        }
        let mut control = [F::ZERO; CONTROL_WIDTH];
        for (offset, value) in [
            (0, phase.gas_after),
            (
                2,
                if ok {
                    phase.pc.wrapping_add(4)
                } else {
                    phase.pc
                },
            ),
            (4, phase.cycles + u64::from(ok)),
        ] {
            control[offset] = F(value & u64::from(u32::MAX));
            control[offset + 1] = F(value >> 32);
        }
        control[COMMIT] = F(u64::from(ok));
        control[PRIVACY_TRAP] = F(u64::from(!ok));
        let read_log = [
            F(phase.address & u64::from(u32::MAX)),
            F(phase.address >> 32),
            F(8),
            F::ONE,
        ];
        Self {
            phase,
            row,
            read,
            writes,
            read_log,
            control,
        }
    }

    fn residues(&self) -> Vec<F> {
        let mut out = Vec::new();
        append_residues(
            &mut out,
            self.phase,
            &self.row,
            &self.read,
            [&self.writes[0], &self.writes[1]],
            &self.read_log,
            &self.control,
        );
        assert_eq!(out.len(), CONSTRAINTS);
        out
    }

    fn accepts(&self) -> bool {
        self.residues().into_iter().all(|x| x == F::ZERO)
    }
}

#[test]
fn exact_selected_private_bits_control_success_and_stack_only_full_private_loads() {
    for address in [Memory::HEAP_START, Memory::STACK_START] {
        for p in [scalar(address, 1), scalar(address + 8, 1)] {
            let mut masks = vec![0, u16::MAX, 0xff, 0xff00, 0x5555, 0xaaaa];
            for bit in 0..16 {
                masks.extend([1 << bit, u16::MAX ^ (1 << bit)]);
            }
            for mask in masks {
                let fixture = Fixture::new(p, mask, [LOW, HIGH]);
                assert!(fixture.accepts(), "address={address} mask={mask:#x}");
                assert_eq!(fixture.read[ENABLED], F::ONE);
                assert_eq!(fixture.read_log[3], F::ONE);
                for index in 0..WIDTH {
                    let mut wrong = fixture.clone();
                    wrong.row[index] = wrong.row[index].add(F::ONE);
                    assert!(!wrong.accepts(), "witness field={index}");
                }
            }
        }
    }
    let mut p = scalar(Memory::STACK_START, 1);
    // A stack limit before the range cannot authorize full private bytes.
    p.stack = false;
    let f = Fixture::new(p, u16::MAX, [LOW, HIGH]);
    assert!(f.accepts());
    assert_eq!(f.control[PRIVACY_TRAP], F::ONE);
}

#[test]
fn late_privacy_failure_cannot_erase_physical_read_or_commit_any_architecture() {
    // Every mask is partially private inside the addressed half.
    for (address, masks) in [
        (Memory::STACK_START, [1, 0x80, 0xfe, 0xff7f]),
        (Memory::STACK_START + 8, [0x100, 0x8000, 0xfe00, 0x7fff]),
    ] {
        for mask in masks {
            let f = Fixture::new(scalar(address, 5), mask, [LOW, HIGH]);
            assert!(f.accepts());
            assert_eq!(f.control[COMMIT], F::ZERO);
            assert_eq!(f.writes, [[F::ZERO; packet::WIDTH]; 2]);
            let mut erased = f.clone();
            erased.read = [F::ZERO; packet::WIDTH];
            assert!(!erased.accepts());
            for index in 0..packet::WIDTH {
                let mut wrong = f.clone();
                wrong.read[index] = wrong.read[index].add(F::ONE);
                assert!(!wrong.accepts(), "memory/read field {index}");
                for slot in 0..2 {
                    let mut wrong = f.clone();
                    wrong.writes[slot][index] = F::ONE;
                    assert!(!wrong.accepts(), "false write slot={slot} field={index}");
                }
            }
            for index in 0..READ_LOG_WIDTH {
                let mut wrong = f.clone();
                wrong.read_log[index] = wrong.read_log[index].add(F::ONE);
                assert!(!wrong.accepts(), "read log {index}");
            }
            for index in 0..CONTROL_WIDTH {
                let mut wrong = f.clone();
                wrong.control[index] = wrong.control[index].add(F::ONE);
                assert!(!wrong.accepts(), "gas/PC/cycle/commit field {index}");
            }
            let success = Fixture::new(f.phase, 0, [LOW, HIGH]);
            let mut forged = f.clone();
            forged.control = success.control;
            forged.writes = success.writes;
            assert!(!forged.accepts());
        }
    }
}

#[test]
fn value_then_tag_order_zero_register_and_control_limb_carries_are_exact() {
    for destination in 0..4 {
        for (address, loaded, full) in [
            (Memory::STACK_START, LOW, 0xff),
            (Memory::STACK_START + 8, HIGH, 0xff00),
        ] {
            let mut p = scalar(address, destination);
            p.pc = u64::MAX - 3;
            p.cycles = u64::from(u32::MAX);
            p.gas_after = (1 << 32) - 1;
            for mask in [0, full] {
                let f = Fixture::new(p, mask, [LOW, HIGH]);
                assert!(f.accepts());
                assert_eq!(&f.control[2..6], &[F::ZERO, F::ZERO, F::ZERO, F::ONE]);
                for write in &f.writes {
                    assert_eq!(write[ENABLED], F(u64::from(destination != 0)));
                }
                if destination != 0 {
                    // The tag write reads the value just written to the same register.
                    assert_eq!(packet::half(&f.writes[0], AFTER, 0), loaded);
                    assert_eq!(packet::half(&f.writes[1], BEFORE, 0), loaded);
                    let mut wrong = f.clone();
                    wrong.writes[1][BEFORE] = F(0x55);
                    assert!(!wrong.accepts());
                    wrong = f.clone();
                    wrong.writes.swap(0, 1);
                    assert!(!wrong.accepts());
                }
            }
        }
    }
    for address in [Memory::HEAP_START, Memory::HEAP_START + 8] {
        let f = Fixture::new(scalar(address, 0), 0, [LOW, HIGH]);
        assert!(f.accepts());
        assert_eq!(f.writes, [[F::ZERO; packet::WIDTH]; 2]);
        assert_eq!(f.read_log[3], F::ONE);
    }
}

#[test]
fn native_value_then_tag_order_rejects_atomic_projection_and_stale_tags() {
    for (mask, old_tag) in [(0, true), (0xff, false)] {
        let p = scalar(Memory::STACK_START, 1);
        let f = Fixture::with_registers(
            p,
            mask,
            [LOW, HIGH],
            [0x55; REGISTERS],
            [old_tag; REGISTERS],
        );
        assert!(f.accepts());
        // The value write keeps the old tag; only the following write replaces it.
        assert_eq!(f.writes[0][AFTER_TAG], F(u64::from(old_tag)));
        let mut atomic = f.clone();
        atomic.writes[0][AFTER_TAG] = F(u64::from(!old_tag));
        assert!(!atomic.accepts());
        assert_eq!(f.writes[1][AFTER_TAG], F(u64::from(!old_tag)));
        for limb in 0..8 {
            assert_eq!(f.writes[1][BEFORE + limb], f.writes[1][AFTER + limb]);
        }
        for slot in 0..2 {
            let mut omitted = f.clone();
            omitted.writes[slot] = [F::ZERO; packet::WIDTH];
            assert!(!omitted.accepts(), "omitted write {slot}");
        }
        assert_eq!(f.writes[1][BEFORE_TAG], F(u64::from(old_tag)));
        assert_eq!(packet::half(&f.writes[1], AFTER, 0), LOW);
        let mut stale = f.clone();
        stale.writes[1][BEFORE_TAG] = F(u64::from(!old_tag));
        assert!(!stale.accepts());
    }
}

#[test]
fn boundary_excludes_known_earlier_failures_without_inventing_owner_permission() {
    let word = enc::encode_load(wide::memory::LOAD64, 1, 3, 0);
    let make = |instruction, address, clocks, gas, cycles, zk, private| {
        ReadPhase::from_fixed_input(
            instruction,
            address,
            1,
            clocks,
            (gas, 0, cycles, Memory::STACK_START + Memory::MIN_STACK_SIZE),
            (zk, private),
        )
    };
    for address in [Memory::HEAP_START, Memory::HEAP_START + 8] {
        assert!(make(word, address, [1, 2, 3], 3, 0, true, false).is_some());
    }
    for (w, a, c, g, t, z, p) in [
        (
            enc::encode_halt(),
            Memory::HEAP_START,
            [1, 2, 3],
            3,
            0,
            true,
            false,
        ),
        (
            enc::encode_store(wide::memory::STORE64, 3, 1, 0),
            Memory::HEAP_START,
            [1, 2, 3],
            3,
            0,
            true,
            false,
        ),
        (word, Memory::HEAP_START + 1, [1, 2, 3], 3, 0, true, false),
        (word, Memory::HEAP_START + 4, [1, 2, 3], 3, 0, true, false),
        (word, Memory::HEAP_START, [1, 2, 3], 2, 0, true, false),
        (word, Memory::HEAP_START, [1, 2, 3], 3, 0, false, false),
        (word, Memory::HEAP_START, [1, 2, 3], 3, 0, true, true),
        (word, Memory::HEAP_START, [1, 1, 3], 3, 0, true, false),
        (word, Memory::HEAP_START, [1, 3, 2], 3, 0, true, false),
        (
            word,
            Memory::HEAP_START,
            [1, 2, 3],
            3,
            u64::MAX,
            true,
            false,
        ),
        (word, u64::MAX - 7, [1, 2, 3], 3, 0, true, false),
        (word, 1 << 36, [1, 2, 3], 3, 0, true, false),
    ] {
        assert!(make(w, a, c, g, t, z, p).is_none());
    }
    // These are two coherent component histories, not authenticated memory.
    // The bank must not be mistaken for a frame/initializer/fetch proof.
    let a = Fixture::new(scalar(Memory::HEAP_START, 1), 0, [LOW, HIGH]);
    let b = Fixture::new(scalar(Memory::HEAP_START, 1), 0, [HIGH, LOW]);
    assert!(a.accepts() && b.accepts());
    assert_ne!(a.read, b.read);
    let mut mixed = a;
    mixed.read = b.read;
    assert!(!mixed.accepts());
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

fn assert_native_writes(vm: &IVM, fixture: &Fixture) {
    let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
    let snapshot = vm.try_diagnostic_snapshot(&budget).unwrap();
    let actual = snapshot
        .register_events()
        .filter(|e| e.written)
        .map(|e| (e.index, e.value, e.tag))
        .collect::<Vec<_>>();
    let expected = fixture
        .writes
        .iter()
        .filter(|p| p[ENABLED] == F::ONE)
        .map(|p| {
            (
                usize::try_from(p[packet::INDEX].0).expect("bounded fixture value"),
                packet::half(p, AFTER, 0),
                p[AFTER_TAG] == F::ONE,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "every native value/tag RegEvent in exact order"
    );
}

#[test]
fn load_ports_match_native_base_alias_halves_and_read_ranges() {
    let address = Memory::HEAP_START + 0x80;
    for half in [0, 8] {
        for destination in [1, 3] {
            let word = enc::encode_load(wide::memory::LOAD64, destination, 3, half);
            let mut vm = native(&[word, enc::encode_halt()], 100);
            vm.memory.store_u64(address, LOW).unwrap();
            vm.memory.store_u64(address + 8, HIGH).unwrap();
            vm.set_register(3, address);
            // The loaded public tag differs from this old tag. An atomic
            // value+tag projection would incorrectly erase the native intermediate.
            vm.registers.set_tag(1, true);
            let mut p = scalar(address + i64::from(half).cast_unsigned(), destination);
            p.pc = 0;
            p.cycles = 0;
            let f = Fixture::with_registers(
                p,
                0,
                [LOW, HIGH],
                core::array::from_fn(|i| vm.registers.get(i)),
                core::array::from_fn(|i| vm.registers.tag(i)),
            );
            assert!(f.accepts());
            vm.memory.clear_tracking();
            vm.run().unwrap();
            assert_eq!(
                vm.registers.get(destination as usize),
                packet::half(&f.writes[0], AFTER, 0)
            );
            assert_native_writes(&vm, &f);
            assert!(
                vm.memory
                    .try_read_log_snapshot()
                    .unwrap()
                    .iter()
                    .any(|r| (r.addr, r.len) == (address + i64::from(half).cast_unsigned(), 8))
            );
            assert_eq!(
                (vm.gas_remaining, vm.pc(), vm.get_cycle_count()),
                (97, 8, 2)
            );
        }
    }
}

#[test]
fn native_fully_private_stack_load_commits_value_and_tag() {
    for half in [0_i16, 8] {
        let load = enc::encode_load(wide::memory::LOAD64, 5, 3, half);
        let mut vm = native(
            &[
                enc::encode_store(wide::memory::STORE64, 3, 1, half),
                enc::encode_halt(),
                load,
                enc::encode_halt(),
            ],
            1000,
        );
        let address = Memory::STACK_START + i64::from(half).cast_unsigned();
        let stored = if half == 0 { LOW } else { HIGH };
        vm.set_register(1, stored);
        vm.set_register(3, Memory::STACK_START);
        vm.set_register(5, 0x55);
        vm.registers.set_tag(1, true);
        vm.run().unwrap();
        vm.set_program_counter(8).unwrap();
        let gas = vm.gas_remaining;
        let mut p = phase(load, address);
        p.pc = 8;
        p.cycles = 0;
        p.gas_after = gas - 3;
        let f = Fixture::with_registers(
            p,
            if half == 0 { 0xff } else { 0xff00 },
            if half == 0 { [LOW, 0] } else { [0, HIGH] },
            core::array::from_fn(|i| vm.registers.get(i)),
            core::array::from_fn(|i| vm.registers.tag(i)),
        );
        assert!(f.accepts());
        assert_eq!(f.control[COMMIT], F::ONE);
        assert_eq!(f.writes[0][BEFORE_TAG], F::ZERO);
        vm.memory.clear_tracking();
        vm.run().unwrap();
        assert_eq!(vm.registers.get(5), stored);
        assert_eq!(vm.registers.get(5), packet::half(&f.writes[0], AFTER, 0));
        assert!(vm.registers.tag(5));
        assert_eq!(F(u64::from(vm.registers.tag(5))), f.writes[1][AFTER_TAG]);
        assert_native_writes(&vm, &f);
        assert!(
            vm.memory
                .try_read_log_snapshot()
                .unwrap()
                .iter()
                .any(|r| (r.addr, r.len) == (address, 8))
        );
        assert_eq!(
            (vm.gas_remaining, vm.pc(), vm.get_cycle_count()),
            (gas - 3, 16, 2)
        );
    }
}

#[test]
fn load_transition_has_eighteen_auxiliary_fields_and_dynamic_degree_four() {
    assert_eq!(WIDTH, 18);
    assert_eq!(CONSTRAINTS, 159);
    let ports = 3 * packet::WIDTH + READ_LOG_WIDTH + CONTROL_WIDTH;
    for p in [
        scalar(Memory::HEAP_START, 1),
        scalar(Memory::STACK_START + 8, 0),
        scalar(Memory::STACK_START, 1),
    ] {
        assert_eq!(
            measured_maximum_affine_degree_v1(
                [0; 32],
                [WIDTH + ports, 0, 0, 0, 0],
                4,
                4,
                |row, _, _, _, _| {
                    let read_start = WIDTH;
                    let value_start = read_start + packet::WIDTH;
                    let tag_start = value_start + packet::WIDTH;
                    let read_log_start = tag_start + packet::WIDTH;
                    let control_start = read_log_start + READ_LOG_WIDTH;
                    let mut out = Vec::new();
                    append_residues(
                        &mut out,
                        p,
                        row[..WIDTH].try_into().unwrap(),
                        row[read_start..value_start].try_into().unwrap(),
                        [
                            row[value_start..tag_start].try_into().unwrap(),
                            row[tag_start..read_log_start].try_into().unwrap(),
                        ],
                        row[read_log_start..control_start].try_into().unwrap(),
                        row[control_start..].try_into().unwrap(),
                    );
                    Ok::<_, core::convert::Infallible>(out)
                }
            ),
            4
        );
    }
    // Only auxiliary arithmetic; packet/log/control copying is not free trace
    // geometry. No integrated profile or proof-size admission is claimed. The
    // bound is the 32-register scalar segment's remaining proof-byte headroom.
    let auxiliary_opening_bytes = WIDTH * (136 * 2 * 8 + 2 * 32);
    assert_eq!(auxiliary_opening_bytes, 40_320);
    assert!(auxiliary_opening_bytes <= 1_044_672);
}

#[test]
fn derived_zero_and_full_flags_require_canonical_inverses_at_every_private_count() {
    for address in [Memory::HEAP_START, Memory::STACK_START] {
        for p in [scalar(address, 1), scalar(address + 8, 1)] {
            let selected = p.selected_bits();
            for count in 0..=selected.len() {
                let mask = u16::from_le_bytes(
                    u32::to_le_bytes(((1_u32 << count) - 1) << selected.start)[..2]
                        .try_into()
                        .expect("low-byte width"),
                );
                let fixture = Fixture::new(p, mask, [LOW, HIGH]);
                assert!(fixture.accepts());
                assert_eq!(
                    fixture.control[COMMIT],
                    F(u64::from(
                        count == 0 || (count == selected.len() && p.stack)
                    ))
                );
                for index in [ZERO_INVERSE, FULL_INVERSE] {
                    let canonical = fixture.row[index];
                    for candidate in [
                        F::ZERO,
                        F::ONE,
                        canonical.add(F::ONE),
                        canonical.sub(F::ONE),
                    ] {
                        let mut changed = fixture.clone();
                        changed.row[index] = candidate;
                        assert_eq!(
                            changed.accepts(),
                            candidate == canonical,
                            "count={count} inverse={index}"
                        );
                    }
                }
            }
        }
    }
}
