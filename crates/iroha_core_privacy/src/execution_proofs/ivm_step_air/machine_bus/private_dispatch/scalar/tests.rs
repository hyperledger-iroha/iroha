//! Local native private scalar captures and original-column adversaries.

use super::super::tests::{Fixture, bits, bytes, carries, contract, event};
use super::*;
use iroha_allocation::AllocationBudget;
use ivm::{
    IVM,
    encoding::wide as enc,
    execution_step_recorder::{
        DiagnosticStepOutcome, DiagnosticStepRecord, DiagnosticStepRecorder,
    },
    host::DefaultHost,
};

/// Test-only private workspace owner; packet backing has its own erasing Drop.
#[derive(Clone)]
struct ScalarFixture(Fixture);
impl Drop for ScalarFixture {
    fn drop(&mut self) {
        for field in &mut self.0.row {
            field.zeroize_v1();
        }
    }
}
impl ScalarFixture {
    fn from_record(program: &Program, record: &DiagnosticStepRecord) -> Self {
        assert_eq!(record.outcome, DiagnosticStepOutcome::Completed);
        let instruction = record.instruction.unwrap();
        assert!(is_supported(instruction));
        let pc = record.before.pc;
        let slot = usize::try_from((pc - u64::from(program.first_pc)) / 4).unwrap();
        assert_eq!(program.words[slot], instruction);
        let mut fixture = Fixture::padding();
        fixture.row[FETCH + slot] = F::ONE;
        let words = [
            pc,
            record.before.gas_remaining,
            record.after.gas_remaining,
            record.before.cycles,
            record.after.cycles,
            0,
            0,
            0,
            0,
            program.cycle_limit - 1 - record.before.cycles,
        ];
        for (i, word) in words.into_iter().enumerate() {
            bits(&mut fixture.row[WORDS + 64 * i..WORDS + 64 * (i + 1)], word);
        }
        carries(
            &mut fixture.row[CARRIES..CARRIES + 4],
            record.before.gas_remaining,
            1,
            true,
        );
        carries(
            &mut fixture.row[CARRIES + 4..CARRIES + 8],
            record.before.cycles,
            1,
            false,
        );
        carries(
            &mut fixture.row[CARRIES + 16..CARRIES + 20],
            program.cycle_limit - 1,
            record.before.cycles,
            true,
        );
        for (slot, owner, before, after, write) in [
            (PC_READ, PC_OWNER, pc, pc, false),
            (
                GAS_DEBIT,
                GAS_OWNER,
                record.before.gas_remaining,
                record.after.gas_remaining,
                true,
            ),
            (CALL_DEPTH, CALL_DEPTH_OWNER, 0, 0, false),
            (PC_WRITE, PC_OWNER, pc, record.after.pc, true),
            (
                CYCLE_WRITE,
                CYCLE_OWNER,
                record.before.cycles,
                record.after.cycles,
                true,
            ),
            (RUNNING_WRITE, RUNNING_OWNER, 1, 1, true),
        ] {
            fixture.packets.fields[slot] = event(
                Space::Owner,
                0,
                owner,
                before,
                after,
                write,
                fixture.schedule.clocks[slot],
                false,
                false,
            );
        }
        let left = wide::rs1(instruction);
        let right = wide::rs2(instruction);
        let destination = wide::rd(instruction);
        for (slot, register, enabled, write) in [
            (SCALAR_LEFT, left, true, false),
            (
                SCALAR_RIGHT,
                right,
                immediate_operand(instruction).is_none(),
                false,
            ),
            (SCALAR_DESTINATION, destination, destination != 0, true),
        ] {
            if enabled {
                fixture.packets.fields[slot] = event(
                    Space::Register,
                    0,
                    register as u32,
                    record.before.registers[register],
                    if write {
                        record.after.registers[register]
                    } else {
                        record.before.registers[register]
                    },
                    write,
                    fixture.schedule.clocks[slot],
                    record.before.tags[register],
                    if write {
                        record.after.tags[register]
                    } else {
                        record.before.tags[register]
                    },
                );
            }
        }
        let left = record.before.registers[left];
        let right = immediate_operand(instruction).unwrap_or(record.before.registers[right]);
        fixture.row[SCALAR..SCALAR + ALU].copy_from_slice(&word::witness(left, right));
        fixture.row[SCALAR + ALU..SCALAR + COMPARE].copy_from_slice(&alu::witness(
            if is_alu(instruction) {
                wide::opcode(instruction)
            } else {
                wide::arithmetic::ADD
            },
            left,
            right,
        ));
        let predicate = comparison_predicate(instruction).map_or(0, |index| {
            [
                wide::control::BEQ,
                wide::control::BNE,
                wide::control::BLT,
                wide::control::BGE,
                wide::control::BLTU,
                wide::control::BGEU,
            ][index]
        });
        fixture.row[SCALAR + COMPARE..super::super::WIDTH]
            .copy_from_slice(&branch::bank_witness(predicate, left, right));
        Self(fixture)
    }
    fn accepts(&self, program: &Program) -> bool {
        self.0.accepts(program)
    }
}

fn native(instruction: u32, inputs: &[(usize, u64, bool)]) -> (Program, ScalarFixture) {
    let artifact = contract(&[instruction], 1_000, ivm::ivm_mode::ZK);
    let mut vm = IVM::new(100);
    vm.load_prepared(&artifact).unwrap();
    for &(register, value, tag) in inputs {
        vm.set_register(register, value);
        vm.registers.set_tag(register, tag);
    }
    let budget = AllocationBudget::new(64 * std::mem::size_of::<DiagnosticStepRecord>());
    let mut recorder = DiagnosticStepRecorder::try_new(64, &budget).unwrap();
    // This is a local native diagnostic boundary, not an authenticated private
    // invocation initializer. Some alias cases overwrite the later return ABI.
    let _later_outcome =
        vm.run_with_host_diagnostic_steps(&mut DefaultHost::default(), &mut recorder);
    let program = Program::new(artifact).unwrap();
    let record = recorder
        .records()
        .first()
        .expect("actual native scalar attempt");
    assert_eq!(record.instruction, Some(instruction));
    assert_eq!(record.opcode_gas, Some(1));
    let fixture = ScalarFixture::from_record(&program, record);
    assert!(fixture.accepts(&program));
    (program, fixture)
}

#[test]
fn native_private_and_public_scalar_register_immediate_and_aliases_match() {
    let values = [0, 1, u64::MAX, 0x8000_0000_0000_0000, 0xa5a5_1234_ffff_0001];
    for opcode in [
        wide::arithmetic::ADD,
        wide::arithmetic::SUB,
        wide::arithmetic::AND,
        wide::arithmetic::OR,
        wide::arithmetic::XOR,
    ] {
        for tag in [false, true] {
            for (left, right) in values.into_iter().zip(values.into_iter().rev()) {
                for (rd, rs1, rs2) in [(4, 2, 3), (2, 2, 3), (3, 2, 3), (2, 2, 2), (0, 2, 3)] {
                    native(
                        enc::encode_rr(opcode, rd, rs1, rs2),
                        &[(2, left, tag), (3, right, tag), (4, 19, !tag)],
                    );
                }
            }
        }
        native(enc::encode_rr(opcode, 4, 0, 0), &[(4, u64::MAX, true)]);
    }
    for opcode in [
        wide::arithmetic::ADDI,
        wide::arithmetic::ANDI,
        wide::arithmetic::ORI,
        wide::arithmetic::XORI,
    ] {
        for tag in [false, true] {
            for immediate in [i8::MIN, -1, 0, 1, i8::MAX] {
                for rd in [0, 2, 4] {
                    native(
                        enc::encode_ri(opcode, rd, 2, immediate),
                        &[(2, u64::MAX, tag), (4, 77, !tag)],
                    );
                }
            }
        }
        native(enc::encode_ri(opcode, 4, 0, -1), &[(4, 17, true)]);
    }
}

#[test]
fn native_mismatched_tags_trap_and_cannot_form_a_successful_private_scalar_row() {
    for opcode in [
        wide::arithmetic::ADD,
        wide::arithmetic::SUB,
        wide::arithmetic::AND,
        wide::arithmetic::OR,
        wide::arithmetic::XOR,
        wide::arithmetic::SLT,
        wide::arithmetic::SLTU,
        wide::arithmetic::SEQ,
        wide::arithmetic::SNE,
    ] {
        for rd in [0, 4] {
            let instruction = enc::encode_rr(opcode, rd, 2, 3);
            let artifact = contract(&[instruction], 1_000, ivm::ivm_mode::ZK);
            let mut vm = IVM::new(100);
            vm.load_prepared(&artifact).unwrap();
            vm.set_register(2, 11);
            vm.set_register(3, 23);
            vm.registers.set_tag(2, true);
            let budget = AllocationBudget::new(8 * std::mem::size_of::<DiagnosticStepRecord>());
            let mut recorder = DiagnosticStepRecorder::try_new(8, &budget).unwrap();
            assert!(matches!(
                vm.run_with_host_diagnostic_steps(&mut DefaultHost::default(), &mut recorder),
                Err(ivm::VMError::PrivacyViolation)
            ));
            let record = &recorder.records()[0];
            assert!(matches!(record.outcome, DiagnosticStepOutcome::Trapped(_)));
            assert_eq!(record.after.registers, record.before.registers);
            assert_eq!(record.after.gas_remaining + 1, record.before.gas_remaining);
            let (program, mut forged) = native(instruction, &[(2, 11, false), (3, 23, false)]);
            forged.0.packets.fields[SCALAR_LEFT][BEFORE_TAG] = F::ONE;
            forged.0.packets.fields[SCALAR_LEFT][AFTER_TAG] = F::ONE;
            if rd != 0 {
                forged.0.packets.fields[SCALAR_DESTINATION][AFTER_TAG] = F::ONE;
            }
            assert!(!forged.accepts(&program));
        }
    }
}

#[test]
fn every_scalar_original_field_and_workspace_mutation_rejects_except_prior_destination() {
    for instruction in [
        enc::encode_rr(wide::arithmetic::ADD, 4, 2, 3),
        enc::encode_rr(wide::arithmetic::SLT, 4, 2, 3),
        enc::encode_rr(wide::arithmetic::SLTU, 2, 2, 3),
        enc::encode_rr(wide::arithmetic::SEQ, 3, 2, 3),
        enc::encode_rr(wide::arithmetic::SNE, 0, 2, 3),
        enc::encode_rr(wide::arithmetic::XOR, 2, 2, 3),
        enc::encode_ri(wide::arithmetic::ANDI, 4, 2, -1),
        enc::encode_ri(wide::arithmetic::ADDI, 0, 2, -1),
    ] {
        let (program, fixture) = native(
            instruction,
            &[(2, u64::MAX, true), (3, 3, true), (4, 9, false)],
        );
        for slot in 0..PORTS {
            for column in 0..packet::WIDTH {
                let mut bad = fixture.clone();
                bad.0.packets.fields[slot][column] = bad.0.packets.fields[slot][column].add(F::ONE);
                let free = slot == SCALAR_DESTINATION
                    && wide::rd(instruction) != 0
                    && wide::rd(instruction) != wide::rs1(instruction)
                    && (immediate_operand(instruction).is_some()
                        || wide::rd(instruction) != wide::rs2(instruction))
                    && ((BEFORE..BEFORE + 4).contains(&column) || column == BEFORE_TAG);
                if !free {
                    assert!(!bad.accepts(&program), "slot {slot} column {column}");
                }
            }
        }
        for index in 0..super::super::WIDTH {
            let mut bad = fixture.clone();
            bad.0.row[index] = bad.0.row[index].add(F::ONE);
            assert!(!bad.accepts(&program), "workspace {index}");
        }
    }
}

#[test]
fn scalar_fetch_signed_immediate_alias_zero_and_tag_substitution_reject() {
    let instruction = enc::encode_ri(wide::arithmetic::ADDI, 2, 2, -1);
    let (program, fixture) = native(instruction, &[(2, u64::MAX, true)]);
    for replacement in [
        enc::encode_ri(wide::arithmetic::ADDI, 2, 2, 1),
        enc::encode_ri(wide::arithmetic::ADDI, 3, 2, -1),
        enc::encode_ri(wide::arithmetic::ANDI, 2, 2, -1),
        enc::encode_rr(wide::arithmetic::ADD, 2, 2, 255),
    ] {
        let replaced = Program::new(contract(&[replacement], 1_000, ivm::ivm_mode::ZK)).unwrap();
        assert!(!fixture.accepts(&replaced));
    }
    let mut bad = fixture.clone();
    bad.0.packets.fields[SCALAR_DESTINATION][BEFORE] = F::ZERO;
    assert!(!bad.accepts(&program));
    let (zero_program, mut zero) = native(enc::encode_ri(wide::arithmetic::ADDI, 4, 0, 1), &[]);
    zero.0.packets.fields[SCALAR_LEFT][BEFORE_TAG] = F::ONE;
    zero.0.packets.fields[SCALAR_LEFT][AFTER_TAG] = F::ONE;
    zero.0.packets.fields[SCALAR_DESTINATION][AFTER_TAG] = F::ONE;
    assert!(!zero.accepts(&zero_program));
    let mut bad = fixture.clone();
    bad.0.packets.fields[SCALAR_DESTINATION][AFTER_TAG] = F::ZERO;
    assert!(!bad.accepts(&program));
}

#[test]
fn composed_private_scalar_polynomials_have_degree_four() {
    use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
    let artifact = contract(
        &[
            enc::encode_rr(wide::arithmetic::XOR, 4, 2, 3),
            enc::encode_rr(wide::arithmetic::SLT, 4, 2, 3),
            enc::encode_rr(wide::arithmetic::SLTU, 4, 2, 3),
            enc::encode_rr(wide::arithmetic::SEQ, 4, 2, 3),
            enc::encode_rr(wide::arithmetic::SNE, 4, 2, 3),
            enc::encode_ri(wide::arithmetic::ADDI, 4, 2, -1),
        ],
        1_000,
        ivm::ivm_mode::ZK,
    );
    let program = Program::new(artifact).unwrap();
    let schedule = Schedule::new(7, core::array::from_fn(|i| i as u32)).unwrap();
    let measured = measured_maximum_affine_degree_v1(
        [0x3d; 32],
        [super::super::WIDTH + PORTS * packet::WIDTH, 0, 0, 0, 0],
        8,
        4,
        |row, _, _, _, _| {
            let packets = OriginalPackets::candidate(core::array::from_fn(|slot| {
                row[super::super::WIDTH + slot * packet::WIDTH
                    ..super::super::WIDTH + (slot + 1) * packet::WIDTH]
                    .try_into()
                    .unwrap()
            }));
            let mut out = Vec::new();
            super::super::append_residues(
                &mut out,
                &program,
                schedule,
                row[..super::super::WIDTH].try_into().unwrap(),
                &packets,
            );
            Ok::<_, core::convert::Infallible>(out)
        },
    );
    assert_eq!(measured, 4);
}

#[test]
fn every_original_scalar_port_joins_all_private_history_stages() {
    use super::super::super::{
        E, NOTE_COPY_WIDTH_V1, ORDERED, PublicPacketBus, ROW_WIDTH, SORTED, permutation,
        private_history,
    };
    use packet::Event;
    fn as_event(fields: &[F; packet::WIDTH]) -> Option<Event> {
        (fields[ENABLED] == F::ONE).then(|| Event {
            space: match fields[SPACE].0 {
                2 => Space::Register,
                4 => Space::Owner,
                _ => panic!("only original scalar register/control ports"),
            },
            vm: fields[VM].0 as u8,
            generation: fields[GENERATION].0 as u16,
            index: fields[INDEX].0 as u32,
            write: fields[WRITE] == F::ONE,
            before: bytes(packet::half(fields, BEFORE, 0)),
            after: bytes(packet::half(fields, AFTER, 0)),
            before_private: fields[BEFORE_TAG].0 as u16,
            after_private: fields[AFTER_TAG].0 as u16,
        })
    }
    fn accepts(fixture: &ScalarFixture, substitution: Option<(usize, bool)>) -> bool {
        let mut original = fixture.0.packets.clone();
        let mut events = original.fields.iter().map(as_event).collect::<Vec<_>>();
        if let Some((slot, missing)) = substitution {
            if missing {
                events[slot] = None;
            } else {
                let event = events[slot].as_mut().unwrap();
                let value =
                    u64::from_le_bytes(event.after[..8].try_into().unwrap()).wrapping_add(1);
                event.after = bytes(value);
                if !event.write {
                    event.before = event.after;
                }
            }
        }
        // Candidate initializers only make the test history internally valid.
        // They have no execution/State authority and are not a proving adapter.
        let mut first = std::collections::BTreeMap::new();
        for event in events.iter().flatten() {
            first
                .entry((event.space as u8, event.generation, event.index))
                .or_insert_with(|| {
                    Some(Event {
                        space: event.space,
                        vm: event.vm,
                        generation: event.generation,
                        index: event.index,
                        write: true,
                        before: [0; 16],
                        after: event.before,
                        before_private: 0,
                        after_private: event.before_private,
                    })
                });
        }
        let prefix = first.len();
        let mut all = first.into_values().collect::<Vec<_>>();
        all.extend(events);
        for (slot, fields) in original.fields.iter_mut().enumerate() {
            if fields[ENABLED] == F::ONE {
                fields[CLOCK] = F((prefix + slot) as u64);
            }
        }
        let bus = PublicPacketBus::new(all).unwrap();
        let columns = bus.columns();
        let challenges = permutation::Challenges::testing(
            E::canonical([2, 1, 0, 0]).unwrap(),
            E::canonical([7, 0, 1, 0]).unwrap(),
        );
        let aux = permutation::columns(
            &columns[NOTE_COPY_WIDTH_V1 + ORDERED..NOTE_COPY_WIDTH_V1 + SORTED],
            &columns
                [NOTE_COPY_WIDTH_V1 + SORTED..NOTE_COPY_WIDTH_V1 + super::super::super::PREVIOUS],
            &challenges,
            bus.size(),
        )
        .unwrap();
        let rows = (0..bus.size())
            .map(|i| {
                core::array::from_fn::<_, ROW_WIDTH, _>(|column| {
                    columns[NOTE_COPY_WIDTH_V1 + column][i]
                })
            })
            .collect::<Vec<_>>();
        let aux_rows = (0..bus.size())
            .map(|i| aux.iter().map(|column| column[i]).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let schedule = private_history::Schedule::new(bus.trace_log2).unwrap();
        let fixed = (0..bus.size())
            .map(|i| schedule.fixed(i).unwrap())
            .collect::<Vec<_>>();
        // Confirm each alternative history is locally consistent before testing
        // the original producer join; no malformed sorted history is the oracle.
        let mut residues = Vec::new();
        for i in 0..bus.size() {
            let next = (i + 1) % bus.size();
            let producer = rows[i][ORDERED..SORTED].try_into().unwrap();
            private_history::append_residues(
                &mut residues,
                &rows[i],
                &rows[next],
                &aux_rows[i],
                &aux_rows[next],
                &fixed[i],
                producer,
                &challenges,
            );
            assert!(residues.iter().all(|value| *value == F::ZERO));
            residues.clear();
        }
        let windows = core::array::from_fn(|index| {
            let i = prefix * super::super::super::PHASES + index;
            HistoryRow {
                current: &rows[i],
                next: &rows[i + 1],
                aux: &aux_rows[i],
                next_aux: &aux_rows[i + 1],
                fixed: &fixed[i],
            }
        });
        original.append_history_residues(&mut residues, &windows, &challenges);
        residues.iter().all(|value| *value == F::ZERO)
    }
    for opcode in [
        wide::arithmetic::ADD,
        wide::arithmetic::SLT,
        wide::arithmetic::SLTU,
        wide::arithmetic::SEQ,
        wide::arithmetic::SNE,
    ] {
        let (_, fixture) = native(
            enc::encode_rr(opcode, 4, 2, 3),
            &[(2, 17, true), (3, 23, true), (4, 19, false)],
        );
        assert!(accepts(&fixture, None));
        for slot in [SCALAR_LEFT, SCALAR_RIGHT, SCALAR_DESTINATION] {
            assert!(!accepts(&fixture, Some((slot, false))));
            assert!(!accepts(&fixture, Some((slot, true))));
        }
    }
}

#[test]
fn consecutive_native_private_scalar_records_preserve_exact_register_and_control_boundaries() {
    let body = [
        enc::encode_rr(wide::arithmetic::ADD, 4, 2, 3),
        enc::encode_rr(wide::arithmetic::SUB, 2, 4, 2),
        enc::encode_ri(wide::arithmetic::XORI, 3, 2, -1),
        enc::encode_rr(wide::arithmetic::AND, 4, 3, 4),
        enc::encode_ri(wide::arithmetic::ORI, 2, 4, i8::MIN),
        enc::encode_rr(wide::arithmetic::OR, 3, 2, 3),
        enc::encode_ri(wide::arithmetic::ANDI, 4, 3, 127),
        enc::encode_ri(wide::arithmetic::ADDI, 4, 4, -1),
        enc::encode_rr(wide::arithmetic::XOR, 0, 4, 2),
        enc::encode_rr(wide::arithmetic::SLT, 4, 2, 3),
        enc::encode_rr(wide::arithmetic::SLTU, 2, 3, 2),
        enc::encode_rr(wide::arithmetic::SEQ, 3, 4, 2),
        enc::encode_rr(wide::arithmetic::SNE, 0, 2, 3),
    ];
    let artifact = contract(&body, 1_000, ivm::ivm_mode::ZK);
    let mut vm = IVM::new(100);
    vm.load_prepared(&artifact).unwrap();
    for (register, value) in [(2, u64::MAX), (3, 2)] {
        vm.set_register(register, value);
        vm.registers.set_tag(register, true);
    }
    let budget = AllocationBudget::new(64 * std::mem::size_of::<DiagnosticStepRecord>());
    let mut recorder = DiagnosticStepRecorder::try_new(64, &budget).unwrap();
    let _later_return =
        vm.run_with_host_diagnostic_steps(&mut DefaultHost::default(), &mut recorder);
    let program = Program::new(artifact).unwrap();
    let records = &recorder.records()[..body.len()];
    for (instruction, record) in body.into_iter().zip(records) {
        assert_eq!(record.instruction, Some(instruction));
        assert!(ScalarFixture::from_record(&program, record).accepts(&program));
    }
    for pair in records.windows(2) {
        assert_eq!(pair[0].after, pair[1].before);
    }
    assert_eq!(records.last().unwrap().after.registers[0], 0);
    assert!(!records.last().unwrap().after.tags[0]);
}

#[test]
fn native_private_comparisons_cover_signed_unsigned_equality_aliases_and_r0() {
    // Include a whole-field modulus to reject equality via a reduced u64 cell.
    let pairs = [
        (0, 0),
        (0, 1),
        (1, 0),
        (1, 1),
        (u64::MAX, 0),
        (0, u64::MAX),
        (u64::MAX, u64::MAX),
        (i64::MAX as u64, i64::MIN as u64),
        (i64::MIN as u64, i64::MAX as u64),
        (i64::MIN as u64, u64::MAX),
        (0xffff_ffff_0000_0001, 0),
        (0, 0xffff_ffff_0000_0001),
        (0x1234_ffff_0000_0001, 0x1234_0000_ffff_0001),
    ];
    for opcode in [
        wide::arithmetic::SLT,
        wide::arithmetic::SLTU,
        wide::arithmetic::SEQ,
        wide::arithmetic::SNE,
    ] {
        for (left, right) in pairs {
            for tag in [false, true] {
                for (rd, rs1, rs2) in [(4, 2, 3), (2, 2, 3), (3, 2, 3), (2, 2, 2), (0, 2, 3)] {
                    native(
                        enc::encode_rr(opcode, rd, rs1, rs2),
                        &[(2, left, tag), (3, right, tag), (4, 19, !tag)],
                    );
                }
            }
        }
        for (rd, rs1, rs2) in [(4, 0, 0), (0, 0, 0), (4, 0, 3), (4, 2, 0)] {
            native(
                enc::encode_rr(opcode, rd, rs1, rs2),
                &[
                    (2, u64::MAX, false),
                    (3, i64::MIN as u64, false),
                    (4, 19, true),
                ],
            );
        }
    }
}

#[test]
fn comparison_fetch_sign_predicate_result_and_modular_alias_forgery_reject() {
    let instruction = enc::encode_rr(wide::arithmetic::SLT, 4, 2, 3);
    let (program, fixture) = native(instruction, &[(2, u64::MAX, true), (3, 0, true)]);
    for replacement in [
        enc::encode_rr(wide::arithmetic::SLTU, 4, 2, 3),
        enc::encode_rr(wide::arithmetic::SEQ, 4, 2, 3),
        enc::encode_rr(wide::arithmetic::SLT, 4, 3, 2),
        enc::encode_rr(wide::arithmetic::SLT, 4, 2, 2),
        enc::encode_rr(wide::arithmetic::SLT, 5, 2, 3),
    ] {
        let changed = Program::new(contract(&[replacement], 1_000, ivm::ivm_mode::ZK)).unwrap();
        assert!(!fixture.accepts(&changed));
    }
    let mut changed = fixture.clone();
    changed.0.row[SCALAR + 63] = F::ZERO;
    assert!(!changed.accepts(&program));
    for limb in 0..4 {
        let mut changed = fixture.clone();
        changed.0.packets.fields[SCALAR_DESTINATION][AFTER + limb] = F(2);
        assert!(!changed.accepts(&program));
    }
    let (program, mut reduced) = native(
        enc::encode_rr(wide::arithmetic::SEQ, 4, 2, 3),
        &[(2, 0xffff_ffff_0000_0001, true), (3, 0, true)],
    );
    // A complete, internally coherent comparison workspace for equal reduced
    // words still cannot replace the original canonical register operands.
    reduced.0.row[SCALAR + COMPARE..super::super::WIDTH].copy_from_slice(&branch::bank_witness(
        wide::control::BEQ,
        0,
        0,
    ));
    reduced.0.packets.fields[SCALAR_DESTINATION][AFTER] = F::ONE;
    assert!(!reduced.accepts(&program));
}

#[test]
fn comparison_workspace_remains_canonical_on_alu_and_padding_rows() {
    let (program, fixture) = native(
        enc::encode_rr(wide::arithmetic::ADD, 4, 2, 3),
        &[(2, u64::MAX, true), (3, 1, true)],
    );
    let padding = ScalarFixture(Fixture::padding());
    assert!(padding.accepts(&program));
    for original in [fixture, padding] {
        for column in SCALAR + COMPARE..super::super::WIDTH {
            let mut changed = original.clone();
            changed.0.row[column] = changed.0.row[column].add(F::ONE);
            assert!(
                !changed.accepts(&program),
                "unused comparison column {column}"
            );
        }
    }
}
