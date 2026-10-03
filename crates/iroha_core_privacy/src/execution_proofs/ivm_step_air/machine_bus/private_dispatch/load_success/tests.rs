//! Original successful ZK LOAD packets, ordered value/tag writes and history adversaries.

use super::super::{tests as dispatch_tests, wide};
use super::*;
use ivm::{Memory, encoding::wide as enc};
use packet::Event;

#[derive(Clone)]
struct Fixture {
    rows: Box<[[F; WIDTH]; PHASES]>,
    schedule: Schedule,
}
fn set_port(row: &mut [F; WIDTH], slot: usize, fields: [F; packet::WIDTH]) {
    row[PACKETS + slot * packet::WIDTH..PACKETS + (slot + 1) * packet::WIDTH]
        .copy_from_slice(&fields);
}
fn event(
    space: Space,
    generation: u16,
    index: u32,
    clock: u32,
    before: [u8; 16],
    after: [u8; 16],
    tags: [u16; 2],
    write: bool,
) -> [F; packet::WIDTH] {
    Event {
        space,
        vm: 7,
        generation,
        index,
        write,
        before,
        after,
        before_private: tags[0],
        after_private: tags[1],
    }
    .fields(clock as usize)
}
fn word_event(index: u32, clock: u32, before: u64, after: u64, write: bool) -> [F; packet::WIDTH] {
    event(
        Space::Owner,
        0,
        index,
        clock,
        (before as u128).to_le_bytes(),
        (after as u128).to_le_bytes(),
        [0, 0],
        write,
    )
}
fn program(base: u8, destination: u8, imm: i8) -> Program {
    Program::new(dispatch_tests::contract(
        &[
            enc::encode_load(wide::memory::LOAD64, destination, base, imm),
            enc::encode_halt(),
            enc::encode_halt(),
            enc::encode_halt(),
        ],
        1_000,
        ivm::ivm_mode::ZK,
    ))
    .unwrap()
}
struct NativeSource {
    descriptors: [u64; 10],
    gas: u64,
    cycles: u64,
}
impl Fixture {
    fn new(program: &Program, address: u64, mask: u16, active: u16, old_tag: bool) -> Self {
        Self::at(program, 0, address, mask, active, old_tag)
    }
    fn at(
        program: &Program,
        pc_slot: usize,
        address: u64,
        mask: u16,
        active: u16,
        old_tag: bool,
    ) -> Self {
        Self::at_with_source(program, pc_slot, address, mask, active, old_tag, None)
    }
    fn at_with_source(
        program: &Program,
        pc_slot: usize,
        address: u64,
        mask: u16,
        active: u16,
        old_tag: bool,
        source: Option<NativeSource>,
    ) -> Self {
        let schedule = Schedule::new(7, 64).unwrap();
        let mut dispatch = if let Some(source) = &source {
            dispatch_tests::Fixture::with_controls(
                program,
                pc_slot,
                false,
                0,
                source.gas,
                source.cycles,
            )
        } else {
            dispatch_tests::Fixture::new(program, pc_slot, false, 0)
        };
        dispatch.schedule = schedule.dispatch();
        let instruction = program.words[pc_slot];
        let base = address.wrapping_sub(i64::from(wide::imm8(instruction)) as u64);
        let destination = wide::rd(instruction);
        let old_tag = old_tag && destination != 0 && destination != wide::rs1(instruction);
        let old_value = if destination == 0 {
            0
        } else if destination == wide::rs1(instruction) {
            base
        } else {
            0x1234_5678
        };
        for (w, value) in [(6, base), (7, address)] {
            dispatch_tests::bits(
                &mut dispatch.row[super::super::WORDS + w * 64..super::super::WORDS + (w + 1) * 64],
                value,
            );
        }
        dispatch_tests::carries(
            &mut dispatch.row[super::super::CARRIES + 8..super::super::CARRIES + 12],
            base,
            i64::from(wide::imm8(instruction)) as u64,
            false,
        );
        dispatch.packets.fields[4] = dispatch_tests::event(
            Space::Register,
            0,
            wide::rs1(instruction) as u32,
            base,
            base,
            false,
            schedule.dispatch().clocks[4],
            false,
            false,
        );
        for slot in 0..super::super::PORTS {
            if dispatch.packets.fields[slot][ENABLED] == F::ONE {
                dispatch.packets.fields[slot][CLOCK] =
                    F(u64::from(schedule.dispatch().clocks[slot]));
            }
        }
        let policies = [
            Memory::STACK_START + Memory::MIN_STACK_SIZE,
            Memory::INPUT_START,
            program.code_end(),
        ];
        let descriptors = [
            Memory::STACK_START,
            policies[0],
            Memory::HEAP_START + 0x800,
            Memory::HEAP_START + 0x810,
            Memory::HEAP_START + 0x900,
            Memory::HEAP_START + 0x910,
            Memory::HEAP_START + 0x800,
            Memory::HEAP_START + 0x810,
            Memory::HEAP_START + 0x900,
            Memory::HEAP_START + 0x910,
        ];
        let descriptors = source
            .as_ref()
            .map_or(descriptors, |source| source.descriptors);
        let mut frame = frame_access::tests::Fixture::new(
            true,
            active,
            address,
            8,
            false,
            descriptors,
            u16::MAX,
        );
        for i in 0..12 {
            if frame.ports[i][ENABLED] == F::ONE {
                frame.ports[i][CLOCK] = F(u64::from(schedule.clock(21 + i)));
            }
        }
        let frame_schedule = frame_access::Schedule::new(
            7,
            8,
            false,
            core::array::from_fn(|i| schedule.clock(21 + i)),
        )
        .unwrap();
        let addr = core::array::from_fn(|i| F((address >> (16 * i)) & 0xffff));
        let mut ignored = Vec::new();
        let decision = frame_access::append_residues(
            &mut ignored,
            frame_schedule,
            &frame.row,
            frame_access::Request {
                selected: F::ONE,
                address: &addr,
            },
            frame_access::Ports {
                active: &frame.ports[0],
                descriptors: core::array::from_fn(|i| &frame.ports[1 + i]),
                initialized: &frame.ports[11],
            },
        );
        let mut rows = Box::new([[F::ZERO; WIDTH]; PHASES]);
        rows[0][..super::super::WIDTH].copy_from_slice(&dispatch.row);
        for i in 0..21 {
            set_port(&mut rows[0], i, dispatch.packets.fields[i]);
        }
        for (i, value) in policies.into_iter().enumerate() {
            set_port(
                &mut rows[1],
                i,
                word_event(
                    POLICY_INDEXES[i],
                    schedule.clock(18 + i),
                    value,
                    value,
                    false,
                ),
            );
        }
        rows[2][..frame_access::WIDTH].copy_from_slice(&frame.row);
        for i in 0..12 {
            set_port(&mut rows[2], i, frame.ports[i]);
        }
        let cell = if address < program.code_end() {
            core::array::from_fn(|i| {
                // CODE starts at the original metadata boundary, including the
                // admitted interface prefix; instruction words alone are not this image.
                program.artifact().artifact()[program.artifact().header_len()..]
                    .get((address & !15) as usize + i)
                    .copied()
                    .unwrap_or(0)
            })
        } else {
            0xfedc_ba98_7654_3210_0123_4567_89ab_cdef_u128.to_le_bytes()
        };
        let half = (address & 8) as usize;
        let value = u64::from_le_bytes(cell[half..half + 8].try_into().unwrap());
        let private = ((mask >> half) & 255) == 255
            && address >= Memory::STACK_START
            && address.wrapping_add(8) <= policies[0];
        set_port(
            &mut rows[3],
            0,
            event(
                Space::Memory,
                0,
                (address >> 4) as u32,
                schedule.clock(33),
                cell,
                cell,
                [mask; 2],
                false,
            ),
        );
        if destination != 0 {
            set_port(
                &mut rows[3],
                1,
                event(
                    Space::Register,
                    0,
                    destination as u32,
                    schedule.clock(34),
                    (old_value as u128).to_le_bytes(),
                    (value as u128).to_le_bytes(),
                    [old_tag as u16; 2],
                    true,
                ),
            );
            set_port(
                &mut rows[3],
                2,
                event(
                    Space::Register,
                    0,
                    destination as u32,
                    schedule.clock(35),
                    (value as u128).to_le_bytes(),
                    (value as u128).to_le_bytes(),
                    [old_tag as u16, private as u16],
                    true,
                ),
            );
        }
        for i in 0..3 {
            set_port(&mut rows[3], 3 + i, dispatch.packets.fields[18 + i]);
        }
        rows[3][..effect::WIDTH].copy_from_slice(&effect::witness(address, true, policies, mask));
        let mut carry = [F::ZERO; CARRY_WIDTH];
        carry[ADDRESS..ADDRESS + 4].copy_from_slice(&addr);
        carry[LOAD] = F::ONE;
        carry[DESTINATION] = F(destination as u64);
        carry[DESTINATION_ENABLED] = F(u64::from(destination != 0));
        for i in 0..3 {
            carry[COMPLETION + i * packet::WIDTH..COMPLETION + (i + 1) * packet::WIDTH]
                .copy_from_slice(&dispatch.packets.fields[18 + i]);
            carry[POLICY + i * packet::WIDTH..POLICY + (i + 1) * packet::WIDTH]
                .copy_from_slice(port(&rows[1], i));
        }
        carry[PERMITTED] = decision.permitted;
        carry[RANGE_ERROR] = decision.range_error;
        for row in rows.iter_mut() {
            row[CARRY..].copy_from_slice(&carry);
        }
        Self { rows, schedule }
    }
    fn padding() -> Self {
        let schedule = Schedule::new(7, 64).unwrap();
        let mut dispatch = dispatch_tests::Fixture::padding();
        for slot in 0..21 {
            if dispatch.packets.fields[slot][ENABLED] == F::ONE {
                dispatch.packets.fields[slot][CLOCK] =
                    F(u64::from(schedule.dispatch().clocks[slot]));
            }
        }
        let frame = frame_access::tests::Fixture::new(false, 0, 0, 8, false, [0; 10], 0);
        let mut rows = Box::new([[F::ZERO; WIDTH]; PHASES]);
        rows[0][..super::super::WIDTH].copy_from_slice(&dispatch.row);
        for i in 0..21 {
            set_port(&mut rows[0], i, dispatch.packets.fields[i]);
        }
        rows[2][..frame_access::WIDTH].copy_from_slice(&frame.row);
        for i in 0..12 {
            set_port(&mut rows[2], i, frame.ports[i]);
        }
        rows[3][..effect::WIDTH].copy_from_slice(&effect::witness(0, false, [0; 3], 0));
        for i in 0..3 {
            set_port(&mut rows[3], 3 + i, dispatch.packets.fields[18 + i]);
        }
        for phase in 0..PHASES {
            for i in 0..3 {
                rows[phase][CARRY + COMPLETION + i * packet::WIDTH
                    ..CARRY + COMPLETION + (i + 1) * packet::WIDTH]
                    .copy_from_slice(&dispatch.packets.fields[18 + i]);
            }
        }
        Self { rows, schedule }
    }
    fn borrowed(&self) -> Rows<'_> {
        Rows {
            phases: core::array::from_fn(|i| &self.rows[i]),
        }
    }
    fn residues(&self, program: &Program) -> Vec<F> {
        let mut out = Vec::new();
        append_semantics(&mut out, program, self.schedule, &self.borrowed());
        out
    }
    fn accepts(&self, program: &Program) -> bool {
        self.residues(program).iter().all(|&r| r == F::ZERO)
    }
}

#[test]
fn successful_load_owns_all_regions_halves_tags_aliases_and_original_controls() {
    for destination in [0, 2, 3] {
        for half in [0, 8] {
            for active in [0, 3] {
                for old_tag in [false, true] {
                    let program = program(2, destination, -8);
                    for address in [
                        half,
                        Memory::HEAP_START + 0x100 + half,
                        Memory::HEAP_START + 0x800 + half,
                        Memory::INPUT_START + half,
                        Memory::OUTPUT_START + half,
                        Memory::STACK_START + 0x100 + half,
                    ] {
                        for mask in [0, 1, 0xff, 0xff00, 0xa55a, u16::MAX] {
                            let f = Fixture::new(&program, address, mask, active, old_tag);
                            let bits = (mask >> half) & 255;
                            let expected =
                                bits == 0 || (bits == 255 && address >= Memory::STACK_START);
                            assert_eq!(
                                f.accepts(&program),
                                expected,
                                "addr={address:x} rd={destination} mask={mask:x} active={active}"
                            );
                            let originals = f.borrowed();
                            assert_eq!(
                                packet::half(originals.producer(1), BEFORE, 0)
                                    - packet::half(originals.producer(1), AFTER, 0),
                                3
                            );
                            assert_eq!(
                                packet::half(originals.producer(37), AFTER, 0)
                                    - packet::half(originals.producer(37), BEFORE, 0),
                                1
                            );
                            assert_eq!(
                                packet::half(originals.producer(36), AFTER, 0)
                                    - packet::half(originals.producer(36), BEFORE, 0),
                                4
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_load_phase_original_tuple_and_carry_rejects_arbitrary_substitution() {
    let program = program(2, 3, -8);
    let original = Fixture::new(&program, Memory::STACK_START + 0x108, 0xff00, 3, true);
    let padding = Fixture::padding();
    assert!(original.accepts(&program));
    assert!(padding.accepts(&program));
    for phase in 0..PHASES {
        let mut mixed = original.clone();
        mixed.rows[phase] = padding.rows[phase];
        assert!(!mixed.accepts(&program));
        for field in CARRY..WIDTH {
            for delta in [F::ONE, F(0x1234567)] {
                let mut bad = original.clone();
                bad.rows[phase][field] = bad.rows[phase][field].add(delta);
                assert!(
                    !bad.accepts(&program),
                    "phase={phase} carry={}",
                    field - CARRY
                );
            }
        }
        let mut reordered = original.clone();
        reordered.rows.swap(phase, (phase + 1) % PHASES);
        assert!(!reordered.accepts(&program));
    }
    for (phase, used) in [21, 3, 12, 6].into_iter().enumerate() {
        for slot in 0..used {
            for field in 0..packet::WIDTH {
                let mut bad = original.clone();
                let c = PACKETS + slot * packet::WIDTH + field;
                bad.rows[phase][c] = bad.rows[phase][c].add(F(0x1234567));
                // The overwritten old destination value is supplied only by original history.
                let history_only = phase == 3 && slot == 1 && (BEFORE..BEFORE + 4).contains(&field);
                assert_eq!(
                    bad.accepts(&program),
                    history_only,
                    "phase={phase} slot={slot} field={field}"
                );
            }
        }
    }
}

#[test]
fn permission_initialization_privacy_and_memory_policy_cannot_be_chosen() {
    let program = program(2, 3, 0);
    for address in [
        program.code_end(),
        Memory::HEAP_START + 1,
        Memory::STACK_START + Memory::MIN_STACK_SIZE,
        u64::MAX - 7,
    ] {
        assert!(
            !Fixture::new(&program, address, 0, 0, false).accepts(&program),
            "addr={address:x}"
        );
    }
    let original = Fixture::new(&program, Memory::STACK_START + 0x108, 0xff00, 3, false);
    assert!(original.accepts(&program));
    for slot in [0, 1, 11] {
        let mut bad = original.clone();
        set_port(&mut bad.rows[2], slot, [F::ZERO; packet::WIDTH]);
        assert!(!bad.accepts(&program));
    }
    for offset in [
        PERMITTED,
        RANGE_ERROR,
        DESTINATION,
        DESTINATION_ENABLED,
        POLICY + BEFORE,
        ADDRESS,
    ] {
        let mut bad = original.clone();
        for row in bad.rows.iter_mut() {
            row[CARRY + offset] = row[CARRY + offset].add(F::ONE);
        }
        assert!(!bad.accepts(&program));
    }
    // A full private stack word cannot be relabeled as a public read, and a
    // foreign generation cannot supply its initialization packet.
    for field in [GENERATION, BEFORE, AFTER] {
        let mut bad = original.clone();
        let c = PACKETS + 11 * packet::WIDTH + field;
        bad.rows[2][c] = bad.rows[2][c].add(F::ONE);
        assert!(!bad.accepts(&program));
    }
    for address in [
        Memory::HEAP_START,
        Memory::INPUT_START,
        Memory::OUTPUT_START,
    ] {
        assert!(!Fixture::new(&program, address, 0xff, 0, false).accepts(&program));
    }
    assert!(!Fixture::new(&program, Memory::STACK_START, 0x7f, 0, false).accepts(&program));
    let zero_base = self::program(0, 3, 8);
    assert!(Fixture::new(&zero_base, 8, 0, 0, false).accepts(&zero_base));
    assert!(!Fixture::new(&zero_base, 16, 0, 0, false).accepts(&zero_base));
}

#[test]
fn every_load_effect_and_padding_workspace_column_has_a_canonical_owner() {
    let program = program(2, 3, 0);
    let original = Fixture::new(&program, Memory::STACK_START + 0x108, 0xff00, 3, true);
    assert!(original.accepts(&program));
    let padding = Fixture::padding();
    assert!(padding.accepts(&program));
    for field in 0..PACKETS {
        for candidate in [&original, &padding] {
            let mut bad = candidate.clone();
            bad.rows[3][field] = bad.rows[3][field].add(F::ONE);
            assert!(!bad.accepts(&program), "effect={field}");
        }
    }
    for (phase, used) in [21, 3, 12, 6].into_iter().enumerate() {
        for field in PACKETS + used * packet::WIDTH..CARRY {
            let mut bad = original.clone();
            bad.rows[phase][field] = F::ONE;
            assert!(!bad.accepts(&program));
        }
    }
}
#[test]
fn four_phases_and_all_success_joins_preserve_degree_four_and_explicit_geometry() {
    use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
    let program = program(2, 3, 0);
    let schedule = Schedule::new(7, 0).unwrap();
    let degree = measured_maximum_affine_degree_v1(
        [0xb7; 32],
        [PHASES * WIDTH, 0, 0, 0, 0],
        4,
        4,
        |row, _, _, _, _| {
            let rows = Rows {
                phases: core::array::from_fn(|i| {
                    row[i * WIDTH..(i + 1) * WIDTH].try_into().unwrap()
                }),
            };
            let mut out = Vec::new();
            append_semantics(&mut out, &program, schedule, &rows);
            Ok::<_, core::convert::Infallible>(out)
        },
    );
    assert_eq!(degree, 4);
    assert_eq!((WIDTH, CARRY_WIDTH, PORTS, PHASES), (2411, 165, 39, 4));
    assert_eq!(PORTS * super::super::super::PHASES, 312);
    assert!(Schedule::new(7, u32::MAX - 38).is_some());
    assert!(Schedule::new(7, u32::MAX - 37).is_none());
}

#[test]
fn all_thirty_nine_original_packets_and_all_eight_stages_share_one_private_history() {
    use super::super::super::{
        E, NOTE_COPY_WIDTH_V1, ORDERED, PREVIOUS, PublicPacketBus, ROW_WIDTH, SORTED,
    };
    let program = program(2, 3, -8);
    let fixture = Fixture::new(&program, Memory::STACK_START + 0x108, 0xff00, 3, true);
    assert!(fixture.accepts(&program));
    let to_event = |p: &[F; packet::WIDTH]| Event {
        space: match p[SPACE].0 {
            1 => Space::Memory,
            2 => Space::Register,
            3 => Space::Initialization,
            4 => Space::Owner,
            _ => panic!("enabled typed event"),
        },
        vm: p[VM].0 as u8,
        generation: p[GENERATION].0 as u16,
        index: p[INDEX].0 as u32,
        write: p[WRITE] == F::ONE,
        before: core::array::from_fn(|i| ((p[BEFORE + i / 2].0 >> (8 * (i % 2))) & 255) as u8),
        after: core::array::from_fn(|i| ((p[AFTER + i / 2].0 >> (8 * (i % 2))) & 255) as u8),
        before_private: p[BEFORE_TAG].0 as u16,
        after_private: p[AFTER_TAG].0 as u16,
    };
    let original = fixture.borrowed();
    let mut events = vec![None; 64];
    let mut keys = std::collections::BTreeSet::new();
    let mut next = 0;
    for slot in 0..PORTS {
        let p = original.producer(slot);
        if p[ENABLED] != F::ONE {
            continue;
        }
        if keys.insert(p[KEY].0) {
            let source = to_event(p);
            events[next] = Some(Event {
                write: true,
                before: [0; 16],
                after: source.before,
                before_private: 0,
                after_private: source.before_private,
                ..source
            });
            next += 1;
        }
    }
    assert!(next < 64);
    for slot in 0..PORTS {
        let p = original.producer(slot);
        events.push((p[ENABLED] == F::ONE).then(|| to_event(p)));
    }
    let bus = PublicPacketBus::new(events).unwrap();
    let columns = bus.columns();
    let challenges = permutation::Challenges::testing(
        E::canonical([2, 1, 0, 0]).unwrap(),
        E::canonical([7, 0, 1, 0]).unwrap(),
    );
    let aux = permutation::columns(
        &columns[NOTE_COPY_WIDTH_V1 + ORDERED..NOTE_COPY_WIDTH_V1 + SORTED],
        &columns[NOTE_COPY_WIDTH_V1 + SORTED..NOTE_COPY_WIDTH_V1 + PREVIOUS],
        &challenges,
        bus.size(),
    )
    .unwrap();
    let count = PORTS * super::super::super::PHASES;
    let start = 64 * super::super::super::PHASES;
    let rows = (start..=start + count)
        .map(|i| core::array::from_fn::<_, ROW_WIDTH, _>(|c| columns[NOTE_COPY_WIDTH_V1 + c][i]))
        .collect::<Vec<_>>();
    let aux_rows = (start..=start + count)
        .map(|i| aux.iter().map(|column| column[i]).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let history_schedule = private_history::Schedule::new(bus.trace_log2).unwrap();
    let fixed = (start..start + count)
        .map(|i| history_schedule.fixed(i).unwrap())
        .collect::<Vec<_>>();
    let windows = core::array::from_fn(|i| super::super::HistoryRow {
        current: &rows[i],
        next: &rows[i + 1],
        aux: &aux_rows[i],
        next_aux: &aux_rows[i + 1],
        fixed: &fixed[i],
    });
    let mut residues = Vec::new();
    append_residues(
        &mut residues,
        &program,
        fixture.schedule,
        &original,
        &windows,
        &challenges,
    );
    assert!(residues.iter().all(|r| *r == F::ZERO));
    for index in [0, 7, 8, count - 1] {
        let mut changed = fixed.clone();
        changed[index][super::super::super::SLOT] =
            changed[index][super::super::super::SLOT].add(F::ONE);
        let altered = core::array::from_fn(|i| super::super::HistoryRow {
            current: &rows[i],
            next: &rows[i + 1],
            aux: &aux_rows[i],
            next_aux: &aux_rows[i + 1],
            fixed: &changed[i],
        });
        residues.clear();
        append_residues(
            &mut residues,
            &program,
            fixture.schedule,
            &original,
            &altered,
            &challenges,
        );
        assert!(
            residues.iter().any(|r| *r != F::ZERO),
            "misplaced history stage {index}"
        );
    }
    // Every complete tuple field, including otherwise free overwritten old
    // bytes, must equal the same original source in each of its eight stages.
    for slot in 0..PORTS {
        for field in 0..packet::WIDTH {
            for stage in 0..super::super::super::PHASES {
                let i = slot * super::super::super::PHASES + stage;
                let h = &windows[i];
                let mut substituted = *original.producer(slot);
                substituted[field] = substituted[field].add(F(0x1234567));
                residues.clear();
                private_history::append_residues(
                    &mut residues,
                    h.current,
                    h.next,
                    h.aux,
                    h.next_aux,
                    h.fixed,
                    &substituted,
                    &challenges,
                );
                assert!(
                    residues.iter().any(|r| *r != F::ZERO),
                    "slot={slot} field={field} stage={stage}"
                );
            }
        }
    }
    assert_eq!(
        original.producer(32)[SPACE],
        F(Space::Initialization as u64)
    );
    assert_eq!(original.producer(33)[SPACE], F(Space::Memory as u64));
    assert_eq!(original.producer(34)[SPACE], F(Space::Register as u64));
    assert_eq!(original.producer(35)[INDEX], original.producer(34)[INDEX]);
}

const NATIVE_STACK: u64 = Memory::STACK_START + Memory::MIN_STACK_SIZE - 16;
const NATIVE_CELL: u128 = 0xfedc_ba98_7654_3210_0123_4567_89ab_cdef;

fn native(program: &Program, gas: u64) -> ivm::IVM {
    let mut vm = ivm::IVM::new(gas);
    vm.load_prepared(program.artifact()).unwrap();
    vm.set_zk_trace_enabled(true);
    assert_eq!(vm.pc(), u64::from(program.first_pc));
    vm
}
fn root_setup_gas() -> u64 {
    // The actual empty-argument owner allocates one Unit result word and
    // the declared 16-byte frame owns its initialization bitmap.
    8 + 16_u64.div_ceil(8) + 1
}
fn root_events(program: &Program) -> Vec<(bool, usize, u64, bool)> {
    let tables = [(10, 0), (11, 0), (12, Memory::HEAP_START), (13, 1)];
    let mut expected = tables.map(|(r, v)| (true, r, v, false)).to_vec();
    expected.extend(tables.map(|(r, v)| (false, r, v, false)));
    for (r, v) in [(31, NATIVE_STACK + 16), (1, program.code_end())] {
        expected.extend([(true, r, v, false); 2]);
    }
    expected.extend([
        (false, 10, 0, false),
        (false, 12, Memory::HEAP_START, false),
        (false, 11, 0, false),
        (false, 13, 1, false),
    ]);
    expected
}
fn assert_native_load_events(
    vm: &ivm::IVM,
    program: &Program,
    fixture: &Fixture,
    private_half: Option<u64>,
) {
    let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
    let capture = vm.try_diagnostic_snapshot(&budget).unwrap();
    let actual = capture
        .register_events()
        .map(|e| (e.written, e.index, e.value, e.tag))
        .collect::<Vec<_>>();
    // Explain every event; no prefix/suffix filtering can hide an unexpected
    // root-call access or a changed STORE/LOAD chronology.
    let mut expected = root_events(program);
    for (register, half) in [(5, 0), (6, 8)] {
        expected.push((false, 4, NATIVE_STACK, false));
        expected.push((
            false,
            register,
            (NATIVE_CELL >> (half * 8)) as u64,
            private_half == Some(half),
        ));
    }
    let originals = fixture.borrowed();
    expected.extend([4, 34, 35].into_iter().filter_map(|slot| {
        let p = originals.producer(slot);
        (p[ENABLED] == F::ONE).then(|| {
            (
                p[WRITE] == F::ONE,
                p[INDEX].0 as usize,
                packet::half(p, if p[WRITE] == F::ONE { AFTER } else { BEFORE }, 0),
                p[if p[WRITE] == F::ONE {
                    AFTER_TAG
                } else {
                    BEFORE_TAG
                }] == F::ONE,
            )
        })
    }));
    assert_eq!(
        actual, expected,
        "complete root setup, stack initialization, base read and ordered value/tag writes"
    );
}

fn native_load_case(
    region: u64,
    half: u64,
    destination: u8,
    imm: i8,
    old_tag: bool,
    private: bool,
) {
    let words = [
        enc::encode_store(wide::memory::STORE64, 4, 5, 0),
        enc::encode_store(wide::memory::STORE64, 4, 6, 8),
        enc::encode_load(wide::memory::LOAD64, destination, 2, imm),
        enc::encode_halt(),
    ];
    let program = Program::new(dispatch_tests::contract_with_frame(
        &words,
        1_000,
        ivm::ivm_mode::ZK,
        16,
    ))
    .unwrap();
    let address = region + half;
    let private_half = private.then_some(half);
    let gas = 100;
    let before_gas = gas - root_setup_gas() - 6;
    let fixture = Fixture::at_with_source(
        &program,
        2,
        address,
        if private { 0xff << half } else { 0 },
        1,
        old_tag,
        Some(NativeSource {
            descriptors: [
                NATIVE_STACK,
                NATIVE_STACK + 16,
                0,
                0,
                Memory::HEAP_START,
                Memory::HEAP_START + 8,
                0,
                0,
                Memory::HEAP_START,
                Memory::HEAP_START + 8,
            ],
            gas: before_gas,
            cycles: 2,
        }),
    );
    assert!(fixture.accepts(&program));
    let mut vm = native(&program, gas);
    let originals = fixture.borrowed();
    let memory = originals.producer(33);
    let data = core::array::from_fn::<_, 16, _>(|i| {
        ((memory[BEFORE + i / 2].0 >> (8 * (i % 2))) & 255) as u8
    });
    if region == Memory::INPUT_START {
        vm.memory.preload_input(0, &data).unwrap();
    } else if region != 0 && region != NATIVE_STACK {
        vm.memory
            .store_u128(region, u128::from_le_bytes(data))
            .unwrap();
    }
    vm.set_register(2, address.wrapping_sub(i64::from(imm) as u64));
    vm.set_register(4, NATIVE_STACK);
    vm.set_register(5, NATIVE_CELL as u64);
    vm.set_register(6, (NATIVE_CELL >> 64) as u64);
    vm.registers.set_tag(5, private_half == Some(0));
    vm.registers.set_tag(6, private_half == Some(8));
    if destination == 3 {
        vm.set_register(3, 0x1234_5678);
        vm.registers.set_tag(3, old_tag);
    }
    vm.memory.clear_tracking();
    let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
    let mut recorder =
        ivm::execution_step_recorder::DiagnosticStepRecorder::try_new(3, &budget).unwrap();
    // A prepaid diagnostic stop occurs before the next instruction. This is
    // a completed LOAD prefix, not a successful full invocation or a HALT bypass.
    assert_eq!(
        vm.run_with_host_diagnostic_steps(&mut ivm::host::DefaultHost::default(), &mut recorder),
        Err(ivm::VMError::ExecutionDeferred(
            ivm::error::ExecutionDeferral::ActiveMemoryCapacity
        ))
    );
    assert_eq!(recorder.records().len(), 3);
    for (slot, record) in recorder.records().iter().enumerate() {
        assert_eq!(record.instruction, Some(words[slot]));
        assert_eq!(
            record.outcome,
            ivm::execution_step_recorder::DiagnosticStepOutcome::Completed
        );
        assert_eq!(record.opcode_gas, Some(3));
        assert_eq!(
            record.before.pc,
            u64::from(program.first_pc) + slot as u64 * 4
        );
        assert_eq!(record.after.pc, record.before.pc + 4);
        assert_eq!(record.after.cycles, record.before.cycles + 1);
        assert_eq!(record.after.gas_remaining, record.before.gas_remaining - 3);
    }
    let step = &recorder.records()[2];
    assert_eq!(
        (step.before.gas_remaining, step.before.cycles),
        (before_gas, 2)
    );
    for (slot, before, after) in [
        (36, step.before.pc, step.after.pc),
        (1, step.before.gas_remaining, step.after.gas_remaining),
        (37, step.before.cycles, step.after.cycles),
    ] {
        assert_eq!(packet::half(originals.producer(slot), BEFORE, 0), before);
        assert_eq!(packet::half(originals.producer(slot), AFTER, 0), after);
    }
    assert_native_load_events(&vm, &program, &fixture, private_half);
    let value = if destination == 0 {
        0
    } else {
        packet::half(originals.producer(35), AFTER, 0)
    };
    let mut values = step.before.registers;
    let mut tags = step.before.tags;
    if destination != 0 {
        values[destination as usize] = value;
        tags[destination as usize] = private;
    }
    assert_eq!(step.after.registers, values);
    assert_eq!(step.after.tags, tags);
    assert_eq!(vm.registers.get(destination as usize), value);
    assert_eq!(
        vm.registers.tag(destination as usize),
        private && destination != 0
    );
    assert!(
        vm.memory
            .try_read_log_snapshot()
            .unwrap()
            .iter()
            .any(|r| (r.addr, r.len) == (address, 8))
    );
    assert_eq!(
        vm.memory
            .try_write_log_snapshot()
            .unwrap()
            .iter()
            .map(|w| (w.address(), w.bytes().len()))
            .collect::<Vec<_>>(),
        [(NATIVE_STACK, 8), (NATIVE_STACK + 8, 8)]
    );
    assert_eq!(
        (vm.gas_remaining, vm.pc(), vm.get_cycle_count()),
        (before_gas - 3, u64::from(program.first_pc) + 12, 3)
    );
    assert!(
        vm.call_result_word_count().is_err(),
        "diagnostic stop does not complete invocation authority"
    );
}

#[test]
fn actual_native_load_matches_five_regions_both_halves_aliases_zero_and_signed_immediates() {
    // HEAP data does not overlap the actual root result table; STACK data
    // belongs to the declared root frame and is initialized by guest STOREs.
    for region in [
        0,
        Memory::HEAP_START + 0x100,
        Memory::INPUT_START,
        Memory::OUTPUT_START,
        NATIVE_STACK,
    ] {
        for half in [0, 8] {
            for destination in [0, 2, 3] {
                for imm in [-128, -8, 0, 127] {
                    for old_tag in [false, true] {
                        native_load_case(region, half, destination, imm, old_tag, false);
                    }
                }
            }
        }
    }
}

#[test]
fn actual_native_private_stack_load_preserves_value_then_tag_and_rejects_failed_success_paths() {
    for half in [0, 8] {
        for destination in [0, 2, 3] {
            native_load_case(NATIVE_STACK, half, destination, 0, false, true);
        }
    }
    let p = Program::new(dispatch_tests::contract_with_frame(
        &[
            enc::encode_load(wide::memory::LOAD64, 3, 2, 0),
            enc::encode_halt(),
        ],
        1_000,
        ivm::ivm_mode::ZK,
        16,
    ))
    .unwrap();
    let outside_code = (p.code_end() + 7) & !7;
    for (address, private_base, opcode_gas, trap) in [
        (
            Memory::HEAP_START + 0x100,
            true,
            100,
            ivm::error::VmTrapKind::PrivacyViolation,
        ),
        (
            Memory::HEAP_START + 0x101,
            false,
            100,
            ivm::error::VmTrapKind::MemoryFault,
        ),
        (
            Memory::HEAP_START + 0x100,
            false,
            2,
            ivm::error::VmTrapKind::OutOfGas,
        ),
        (
            outside_code,
            false,
            100,
            ivm::error::VmTrapKind::MemoryFault,
        ),
    ] {
        let mut vm = native(&p, root_setup_gas() + opcode_gas);
        vm.set_register(2, address);
        vm.registers.set_tag(2, private_base);
        vm.set_register(3, 99);
        let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
        let mut recorder =
            ivm::execution_step_recorder::DiagnosticStepRecorder::try_new(1, &budget).unwrap();
        assert!(
            vm.run_with_host_diagnostic_steps(
                &mut ivm::host::DefaultHost::default(),
                &mut recorder
            )
            .is_err()
        );
        assert_eq!(
            recorder.records().len(),
            1,
            "actual opcode refusal, not a root setup failure"
        );
        let step = &recorder.records()[0];
        assert_eq!(step.instruction, Some(p.words[0]));
        assert_eq!(
            step.outcome,
            ivm::execution_step_recorder::DiagnosticStepOutcome::Trapped(trap)
        );
        assert_eq!(step.before.gas_remaining, opcode_gas);
        assert_eq!(vm.registers.get(3), 99);
        assert_eq!(vm.pc(), u64::from(p.first_pc));
        assert_eq!(vm.get_cycle_count(), 0);
    }
    let mut exhausted = program(2, 3, 0);
    let candidate = Fixture::new(&exhausted, Memory::HEAP_START, 0, 0, false);
    exhausted.cycle_limit = 100;
    assert!(!candidate.accepts(&exhausted));
    let mut base_private = Fixture::new(&p, Memory::HEAP_START, 0, 0, false);
    for offset in [BEFORE_TAG, AFTER_TAG] {
        base_private.rows[0][PACKETS + 4 * packet::WIDTH + offset] = F::ONE;
    }
    assert!(!base_private.accepts(&p));
}

#[test]
fn other_running_opcodes_cannot_bypass_the_closed_load_step() {
    let program = Program::new(dispatch_tests::contract(
        &[
            enc::encode_jump(wide::control::JAL, 1, 4),
            enc::encode_offset24(wide::control::JALS, 3),
            enc::encode_store(wide::memory::STORE64, 2, 3, -8),
            enc::encode_ri(wide::control::JALR, 0, 1, 0),
        ],
        1_000,
        ivm::ivm_mode::ZK,
    ))
    .unwrap();
    assert!(Fixture::padding().accepts(&program));
    for slot in [0, 1, 2, 3] {
        let mut fixture = Fixture::padding();
        let mut dispatch = dispatch_tests::Fixture::new(&program, slot, false, 0);
        dispatch.schedule = fixture.schedule.dispatch();
        for i in 0..21 {
            if dispatch.packets.fields[i][ENABLED] == F::ONE {
                dispatch.packets.fields[i][CLOCK] = F(u64::from(dispatch.schedule.clocks[i]));
            }
        }
        assert!(
            dispatch.accepts(&program),
            "actual otherwise-valid non-LOAD dispatcher"
        );
        fixture.rows[0][..super::super::WIDTH].copy_from_slice(&dispatch.row);
        for i in 0..21 {
            set_port(&mut fixture.rows[0], i, dispatch.packets.fields[i]);
        }
        for phase in 0..PHASES {
            for i in 0..3 {
                fixture.rows[phase][CARRY + COMPLETION + i * packet::WIDTH
                    ..CARRY + COMPLETION + (i + 1) * packet::WIDTH]
                    .copy_from_slice(&dispatch.packets.fields[18 + i]);
            }
        }
        for i in 0..3 {
            set_port(&mut fixture.rows[3], 3 + i, dispatch.packets.fields[18 + i]);
        }
        let nonzero = fixture
            .residues(&program)
            .into_iter()
            .filter(|r| *r != F::ZERO)
            .collect::<Vec<_>>();
        assert_eq!(
            nonzero,
            [F::ZERO.sub(F::ONE)],
            "only the new closed-opcode guard must reject slot {slot}"
        );
    }
}
