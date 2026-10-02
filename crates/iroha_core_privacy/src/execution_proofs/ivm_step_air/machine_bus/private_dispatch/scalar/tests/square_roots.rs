//! Native private square roots, exact original ports and coherent floor attacks.

use super::*;

fn checked(instruction: u32, inputs: &[(usize, u64, bool)]) -> (Program, ScalarFixture) {
    let (program, fixture) = native(instruction, inputs);
    let p = &fixture.0.packets.fields;
    let value = packet::half(&p[SCALAR_LEFT], BEFORE, 0);
    let root = packet::half(&p[SCALAR_DESTINATION], AFTER, 0);
    assert_eq!(ivm::gas::cost_of(instruction), Some(6));
    assert!(p[SCALAR_RIGHT].iter().all(|cell| *cell == F::ZERO));
    if has_destination(instruction) {
        assert!(root <= u64::from(u32::MAX));
        assert!(u128::from(root) * u128::from(root) <= u128::from(value));
        assert!(u128::from(root + 1) * u128::from(root + 1) > u128::from(value));
        assert_eq!(p[SCALAR_DESTINATION][AFTER_TAG], p[SCALAR_LEFT][BEFORE_TAG]);
    } else {
        assert!(p[SCALAR_DESTINATION].iter().all(|cell| *cell == F::ZERO));
    }
    assert_eq!(
        packet::half(&p[GAS_DEBIT], BEFORE, 0) - packet::half(&p[GAS_DEBIT], AFTER, 0),
        6
    );
    assert_eq!(
        packet::half(&p[CYCLE_WRITE], AFTER, 0) - packet::half(&p[CYCLE_WRITE], BEFORE, 0),
        6
    );
    assert_eq!(
        packet::half(&p[PC_WRITE], AFTER, 0),
        packet::half(&p[PC_READ], BEFORE, 0) + 4
    );
    (program, fixture)
}

#[test]
fn native_private_isqrt_boundaries_tags_aliases_zero_and_unused_operand_match() {
    let mut values = vec![0, 1, 2, 3, u64::MAX, 1 << 63];
    for root in [
        2_u64,
        3,
        255,
        256,
        65535,
        65536,
        (1 << 31) - 1,
        1 << 31,
        u64::from(u32::MAX),
    ] {
        let square = root * root;
        values.extend([square - 1, square, square + 1]);
    }
    for value in values {
        for tag in [false, true] {
            for (rd, rs) in [(4, 2), (2, 2), (0, 2), (4, 0), (0, 0)] {
                checked(
                    enc::encode_rr(wide::arithmetic::ISQRT, rd, rs, 255),
                    &[(2, value, tag), (4, 91, !tag), (255, u64::MAX, !tag)],
                );
            }
        }
    }
}

fn replace_word(bank: &mut [F], offset: usize, value: u64) {
    for limb in 0..4 {
        bank[offset + limb] = constant_limb(value, limb);
    }
    word::fill_digits(&mut bank[offset + 4..offset + 36], value);
}

fn set_destination(fixture: &mut ScalarFixture, value: u64) {
    for limb in 0..4 {
        fixture.0.packets.fields[SCALAR_DESTINATION][AFTER + limb] = constant_limb(value, limb);
    }
}

#[test]
fn private_isqrt_rejects_coherent_smaller_root_with_exact_square_plus_remainder() {
    let (program, mut forged) = checked(
        enc::encode_rr(wide::arithmetic::ISQRT, 4, 2, 255),
        &[(2, 25, true), (255, u64::MAX, false)],
    );
    // 25=4*4+9 still holds, but 9>2*4. Rebuild every affected digit,
    // product, correction, remainder, destination and carry coherently.
    let bank = &mut forged.0.row[SCALAR + SHIFT..super::super::super::WIDTH];
    replace_word(bank, division::QUOTIENT, 4);
    replace_word(bank, division::REMAINDER, 9);
    replace_word(bank, division::REMAINDER_RESULT, 8);
    let bound = multiply::correction_witness(8, 9);
    bank[division::QUOTIENT_RESULT..division::QUOTIENT_RESULT + 36].copy_from_slice(&bound[..36]);
    bank[division::QUOTIENT_BORROWS..division::QUOTIENT_BORROWS + 4].copy_from_slice(&bound[36..]);
    bank[division::SUM_CARRIES..division::SUM_CARRIES + 8].fill(F::ZERO);
    let digits = multiply::product_digits(4, 4);
    let mut product = multiply::witness(4, 4, &digits, true);
    product[multiply::SIGNED_UNSIGNED..multiply::SIGNED_SIGNED]
        .copy_from_slice(&multiply::correction_witness(25, 0));
    product[multiply::SIGNED_SIGNED..].copy_from_slice(&multiply::correction_witness(4, 0));
    forged.0.row[SCALAR + PRODUCT_DIGITS..SCALAR + MULTIPLY].copy_from_slice(&digits);
    forged.0.row[SCALAR + MULTIPLY..SCALAR + COUNT].copy_from_slice(&product);
    set_destination(&mut forged, 4);
    assert!(!forged.accepts(&program));
    forged.0.row[SCALAR + SHIFT + division::QUOTIENT_BORROWS + 3] = F::ZERO;
    assert!(
        !forged.accepts(&program),
        "false bound carry cannot erase underflow"
    );
}

#[test]
fn private_isqrt_rejects_ceil_substituted_source_tag_and_carry_cells() {
    for value in [0, 2, 26, u64::MAX] {
        for tag in [false, true] {
            let (program, fixture) = checked(
                enc::encode_rr(wide::arithmetic::ISQRT, 4, 2, 255),
                &[(2, value, tag)],
            );
            let root = packet::half(&fixture.0.packets.fields[SCALAR_DESTINATION], AFTER, 0);
            let mut wrong = fixture.clone();
            set_destination(&mut wrong, root + 1);
            assert!(!wrong.accepts(&program));
            let mut wrong = fixture.clone();
            wrong.0.packets.fields[SCALAR_DESTINATION][AFTER_TAG] = F(u64::from(!tag));
            assert!(!wrong.accepts(&program));
            let gas = packet::half(&fixture.0.packets.fields[GAS_DEBIT], BEFORE, 0);
            if root < u64::from(u32::MAX) {
                // A coherent next-square bank keeps q+1, r=0, products and
                // result valid; restore the original source magnitude so the
                // exact square-plus-remainder equality must reject it.
                let mut ceil = fixture.clone();
                fill_square_root(&mut ceil.0, (root + 1) * (root + 1), gas);
                ceil.0.row[SCALAR + MULTIPLY + multiply::SIGNED_UNSIGNED
                    ..SCALAR + MULTIPLY + multiply::SIGNED_SIGNED]
                    .copy_from_slice(&multiply::correction_witness(value, 0));
                set_destination(&mut ceil, root + 1);
                assert!(!ceil.accepts(&program));
            }
            let substitute = if value == 0 { u64::MAX } else { 0 };
            let mut wrong = fixture.clone();
            fill_square_root(&mut wrong.0, substitute, gas);
            set_destination(&mut wrong, substitute.isqrt());
            assert!(
                !wrong.accepts(&program),
                "foreign source cannot own the original read"
            );
            for column in [
                SCALAR + SHIFT + division::QUOTIENT + 2,
                SCALAR + SHIFT + division::QUOTIENT_BORROWS + 3,
                SCALAR + SHIFT + division::REMAINDER_BORROWS + 3,
                SCALAR + SHIFT + division::SUM_CARRIES,
                SCALAR + SHIFT + division::SUM_CARRIES + 7,
                SCALAR + SHIFT + division::GAS_BORROWS + 3,
                SCALAR + SHIFT + division::LOCAL_TRAP,
                SCALAR + MULTIPLY + multiply::PRODUCT + 4,
            ] {
                let mut wrong = fixture.clone();
                wrong.0.row[column] = wrong.0.row[column].add(F::ONE);
                assert!(!wrong.accepts(&program), "column {column}");
            }
            assert!(Fixture::padding().accepts(&program));
        }
    }
}

#[test]
fn private_isqrt_binds_six_gas_six_cycles_and_native_last_attempt_limit() {
    let instruction = enc::encode_rr(wide::arithmetic::ISQRT, 4, 2, 255);
    let inputs = [(2, u64::MAX, true), (255, 17, false)];
    let (program, fixture) = checked(instruction, &inputs);
    let p = &fixture.0.packets.fields;
    let gas = packet::half(&p[GAS_DEBIT], BEFORE, 0);
    let cycles = packet::half(&p[CYCLE_WRITE], BEFORE, 0);
    for cost in [0, 1, 5, 7, 12] {
        let mut wrong = fixture.clone();
        let after = gas - cost;
        bits(&mut wrong.0.row[WORDS + 128..WORDS + 192], after);
        carries(&mut wrong.0.row[CARRIES..CARRIES + 4], gas, cost, true);
        for limb in 0..4 {
            wrong.0.packets.fields[GAS_DEBIT][AFTER + limb] = constant_limb(after, limb);
        }
        assert!(!wrong.accepts(&program));
        let mut wrong = fixture.clone();
        let after = cycles + cost;
        bits(&mut wrong.0.row[WORDS + 256..WORDS + 320], after);
        carries(
            &mut wrong.0.row[CARRIES + 4..CARRIES + 8],
            cycles,
            cost,
            false,
        );
        for limb in 0..4 {
            wrong.0.packets.fields[CYCLE_WRITE][AFTER + limb] = constant_limb(after, limb);
        }
        assert!(!wrong.accepts(&program));
    }
    for gas in 0..6 {
        let (_, recorder, outcome) =
            shifts::capture(&[instruction], &inputs, root_setup_gas() + gas, 32);
        assert!(matches!(outcome, Err(ivm::VMError::OutOfGas)));
        assert_eq!(recorder.records().len(), 1);
        let record = &recorder.records()[0];
        assert_eq!(record.opcode_gas, Some(6));
        assert_eq!(record.before.gas_remaining, gas);
        assert_eq!(record.before, record.after);
        assert!(matches!(record.outcome, DiagnosticStepOutcome::Trapped(_)));
    }
    for total_gas in [root_setup_gas() + 6, 0xffff, 0x10000, u64::MAX] {
        let (program, recorder, _) = shifts::capture(&[instruction], &inputs, total_gas, 32);
        let record = &recorder.records()[0];
        assert_eq!(record.outcome, DiagnosticStepOutcome::Completed);
        assert_eq!(record.before.gas_remaining, total_gas - root_setup_gas());
        assert_eq!(record.after.gas_remaining, total_gas - root_setup_gas() - 6);
        assert_eq!(record.after.cycles - record.before.cycles, 6);
        assert!(ScalarFixture::from_record(&program, record).accepts(&program));
    }
    for limit in 1..=6 {
        let (program, recorder, outcome) = shifts::capture(&[instruction], &inputs, 100, limit);
        assert!(matches!(outcome, Err(ivm::VMError::ExceededMaxCycles)));
        assert_eq!(recorder.records().len(), 1);
        let record = &recorder.records()[0];
        assert_eq!(record.outcome, DiagnosticStepOutcome::Completed);
        assert_eq!(record.before.cycles, 0);
        assert_eq!(record.after.cycles, 6);
        assert!(ScalarFixture::from_record(&program, record).accepts(&program));
    }
}

#[test]
fn private_isqrt_mixed_original_records_preserve_continuity_and_canonical_padding() {
    let instructions = [
        enc::encode_rr(wide::arithmetic::ISQRT, 4, 2, 255),
        enc::encode_rr(wide::arithmetic::ADD, 4, 4, 3),
        enc::encode_rr(wide::arithmetic::ISQRT, 4, 4, 255),
        enc::encode_rr(wide::system::GETGAS, 5, 0, 0),
        enc::encode_rr(wide::arithmetic::ISQRT, 5, 5, 255),
    ];
    let (program, recorder, _) = shifts::capture(
        &instructions,
        &[(2, u64::MAX, true), (3, 17, true), (255, 99, false)],
        100,
        64,
    );
    assert!(recorder.records().len() >= instructions.len());
    for (instruction, record) in instructions.into_iter().zip(recorder.records()) {
        assert_eq!(record.instruction, Some(instruction));
        assert!(ScalarFixture::from_record(&program, record).accepts(&program));
    }
    for pair in recorder.records()[..instructions.len()].windows(2) {
        assert_eq!(pair[0].after, pair[1].before);
    }
    let padding = Fixture::padding();
    assert!(padding.accepts(&program));
    for column in SCALAR + SHIFT..super::super::super::WIDTH {
        let mut wrong = ScalarFixture(padding.clone());
        wrong.0.row[column] = wrong.0.row[column].add(F::ONE);
        assert!(!wrong.accepts(&program), "padding column {column}");
    }
}
