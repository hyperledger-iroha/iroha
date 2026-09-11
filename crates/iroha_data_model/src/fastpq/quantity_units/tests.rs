//! Full-domain unit arithmetic checked against the ledger bigint implementation.

use super::*;

fn quantity(mantissa: BigInt, scale: u32) -> Quantity {
    Quantity::from_canonical_numeric(Numeric::try_new(mantissa, scale).unwrap()).unwrap()
}

fn maximum_quantity() -> Quantity {
    let mut bytes = [0xff_u8; 64];
    bytes[63] = 0x7f;
    quantity(BigInt::from_twos_bytes(&bytes).unwrap(), 0)
}

fn reference_units(value: &Quantity, scale: u32) -> BigInt {
    value
        .mantissa()
        .checked_mul(&BigInt::pow10(scale - value.scale()).unwrap())
        .unwrap()
}

fn reference_bytes(value: &BigInt) -> [u8; FASTPQ_QUANTITY_UNIT_BYTES] {
    let source = value.to_twos_bytes();
    let mut bytes = [0_u8; FASTPQ_QUANTITY_UNIT_BYTES];
    bytes[..source.len()].copy_from_slice(&source);
    bytes
}

#[test]
fn normalization_covers_full_ledger_domain_at_every_admissible_scale() {
    for value in [
        Quantity::zero(),
        Quantity::one(),
        Quantity::from(u64::MAX),
        Quantity::from(u128::from(u64::MAX) + 1),
        Quantity::from(u128::MAX),
        maximum_quantity(),
        quantity(BigInt::one(), MAX_DECIMAL_SCALE),
        quantity(BigInt::from(12_345_u32), 7),
    ] {
        for scale in value.scale()..=MAX_DECIMAL_SCALE {
            let units = FastpqQuantityUnits::from_quantity(&value, scale).unwrap();
            assert_eq!(units.scale(), scale);
            assert_eq!(units.to_quantity(), Some(value.clone()));
            assert_eq!(
                units.to_le_bytes(),
                reference_bytes(&reference_units(&value, scale))
            );
            assert_eq!(
                FastpqQuantityUnits::from_limbs(*units.limbs(), scale),
                Some(units)
            );
        }
    }
    let maximum = maximum_quantity();
    assert_eq!(reference_units(&maximum, 28).bit_len(), 605);
    let units = FastpqQuantityUnits::from_quantity(&maximum, 28).unwrap();
    assert_ne!(
        units.limbs()[18],
        0,
        "normalization needs the nineteenth limb"
    );
}

#[test]
fn scale_bounds_are_checked_even_for_zero() {
    for scale in [29, 100, u32::MAX] {
        assert!(FastpqQuantityUnits::from_quantity(&Quantity::zero(), scale).is_none());
        assert!(FastpqQuantityUnits::from_quantity(&Quantity::one(), scale).is_none());
        assert!(FastpqQuantityUnits::from_limbs([0; FASTPQ_QUANTITY_UNIT_LIMBS], scale).is_none());
        assert!(crate::fastpq::normalized_numeric_to_u64(&Numeric::zero(), scale).is_none());
    }
    let fractional = quantity(BigInt::one(), 28);
    for scale in 0..28 {
        assert!(FastpqQuantityUnits::from_quantity(&fractional, scale).is_none());
    }
    assert!(crate::fastpq::normalized_numeric_to_u64(&Numeric::from(-1_i64), 0).is_none());
}

#[test]
fn limb_construction_rejects_values_outside_the_canonical_decimal_domain() {
    for scale in 0..=MAX_DECIMAL_SCALE {
        assert!(
            FastpqQuantityUnits::from_limbs([u32::MAX; FASTPQ_QUANTITY_UNIT_LIMBS], scale)
                .is_none()
        );
        let mut too_wide = [0_u32; FASTPQ_QUANTITY_UNIT_LIMBS];
        too_wide[18] = 1 << 29;
        assert!(FastpqQuantityUnits::from_limbs(too_wide, scale).is_none());
    }
    let maximum = maximum_quantity();
    let scaled = FastpqQuantityUnits::from_quantity(&maximum, 28).unwrap();
    assert!(FastpqQuantityUnits::from_limbs(*scaled.limbs(), 0).is_none());
    assert_eq!(
        FastpqQuantityUnits::from_limbs(*scaled.limbs(), 28),
        Some(scaled)
    );
    assert!(
        FastpqQuantityUnits {
            limbs: [0; FASTPQ_QUANTITY_UNIT_LIMBS],
            scale: u32::MAX
        }
        .to_quantity()
        .is_none()
    );
}

#[test]
fn bytes_and_narrowing_are_unsigned_and_little_endian() {
    for value in [
        0_u128,
        0x80,
        1 << 31,
        1 << 63,
        u128::from(u64::MAX),
        1 << 64,
        1 << 127,
    ] {
        let quantity = Quantity::from(value);
        let units = FastpqQuantityUnits::from_quantity(&quantity, 0).unwrap();
        let bytes = units.to_le_bytes();
        assert_eq!(&bytes[..16], &value.to_le_bytes());
        assert!(bytes[16..].iter().all(|byte| *byte == 0));
        assert_eq!(units.try_to_u64(), u64::try_from(value).ok());
        assert_eq!(
            crate::fastpq::normalized_numeric_to_u64(quantity.as_numeric(), 0),
            u64::try_from(value).ok()
        );
        for (limb, chunk) in units.limbs().iter().zip(bytes.chunks_exact(4)) {
            assert_eq!(limb.to_le_bytes().as_slice(), chunk);
        }
    }
    assert_eq!(
        FastpqQuantityUnits::from_quantity(&Quantity::zero(), 28)
            .unwrap()
            .try_to_u64(),
        Some(0)
    );
    assert!(
        FastpqQuantityUnits::from_quantity(&Quantity::from(u64::MAX), 1)
            .unwrap()
            .try_to_u64()
            .is_none()
    );
}

#[test]
fn arithmetic_and_comparison_require_identical_scales() {
    let left = FastpqQuantityUnits::from_quantity(&Quantity::one(), 0).unwrap();
    let right = FastpqQuantityUnits::from_quantity(&Quantity::one(), 28).unwrap();
    assert_ne!(left, right);
    assert_eq!(left.to_quantity(), right.to_quantity());
    assert!(left.checked_add(&right).is_none());
    assert!(left.checked_sub(&right).is_none());
    assert!(left.checked_cmp(&right).is_none());
    assert_eq!(left.checked_cmp(&left), Some(Ordering::Equal));
}

#[test]
fn carry_and_borrow_cross_every_mantissa_limb() {
    let one = FastpqQuantityUnits::from_quantity(&Quantity::one(), 0).unwrap();
    for boundary in 1..=15 {
        let mut limbs = [0_u32; FASTPQ_QUANTITY_UNIT_LIMBS];
        limbs[..boundary].fill(u32::MAX);
        let before = FastpqQuantityUnits::from_limbs(limbs, 0).unwrap();
        let after = before.checked_add(&one).unwrap();
        assert!(after.limbs()[..boundary].iter().all(|limb| *limb == 0));
        assert_eq!(after.limbs()[boundary], 1);
        assert_eq!(after.checked_sub(&one), Some(before));
        assert_eq!(before.checked_cmp(&after), Some(Ordering::Less));
        assert_eq!(after.checked_cmp(&before), Some(Ordering::Greater));
    }
    for scale in [0, 1, 28] {
        let maximum = FastpqQuantityUnits::from_quantity(&maximum_quantity(), scale).unwrap();
        let one = FastpqQuantityUnits::from_quantity(&Quantity::one(), scale).unwrap();
        let previous = maximum.checked_sub(&one).unwrap();
        assert_eq!(previous.checked_add(&one), Some(maximum));
        assert!(
            maximum.checked_add(&one).is_none(),
            "ledger mantissa overflow must fail"
        );
        assert!(one.checked_sub(&maximum).is_none());
    }
}

#[test]
fn arithmetic_rejects_decimal_results_that_cannot_be_represented() {
    let maximum = FastpqQuantityUnits::from_quantity(&maximum_quantity(), 1).unwrap();
    let tenth = FastpqQuantityUnits::from_quantity(&quantity(BigInt::one(), 1), 1).unwrap();
    assert!(maximum.checked_sub(&tenth).is_none());
    assert!(maximum.checked_add(&tenth).is_none());
    let zero = FastpqQuantityUnits::from_quantity(&Quantity::zero(), 1).unwrap();
    assert!(zero.checked_sub(&tenth).is_none());
    assert_eq!(tenth.checked_sub(&tenth), Some(zero));
}

#[test]
fn deterministic_full_width_arithmetic_matches_independent_bigint_reference() {
    let mut seed = 0x5fa5_0031_a77b_811d_u64;
    let mut random = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..256 {
        let mut values = Vec::new();
        for _ in 0..2 {
            let mut bytes = [0_u8; 64];
            for chunk in bytes.chunks_exact_mut(8) {
                chunk.copy_from_slice(&random().to_le_bytes());
            }
            bytes[63] &= 0x7f;
            values.push(quantity(
                BigInt::from_twos_bytes(&bytes).unwrap(),
                (random() % 29) as u32,
            ));
        }
        let scale = values.iter().map(Quantity::scale).max().unwrap();
        let left = FastpqQuantityUnits::from_quantity(&values[0], scale).unwrap();
        let right = FastpqQuantityUnits::from_quantity(&values[1], scale).unwrap();
        let ref_left = reference_units(&values[0], scale);
        let ref_right = reference_units(&values[1], scale);
        assert_eq!(left.to_le_bytes(), reference_bytes(&ref_left));
        assert_eq!(right.to_le_bytes(), reference_bytes(&ref_right));
        assert_eq!(left.checked_cmp(&right), Some(ref_left.cmp(&ref_right)));
        for (actual, reference) in [
            (left.checked_add(&right), ref_left.checked_add(&ref_right)),
            (left.checked_sub(&right), ref_left.checked_sub(&ref_right)),
        ] {
            let expected = reference
                .ok()
                .and_then(|mantissa| Numeric::try_new(mantissa, scale).ok())
                .and_then(|numeric| Quantity::from_canonical_numeric(numeric).ok());
            assert_eq!(actual.and_then(|units| units.to_quantity()), expected);
        }
    }
}

#[test]
fn wide_limb_split_preserves_low_word_and_carry() {
    for (wide, expected) in [
        (0, (0, 0)),
        (u64::from(u32::MAX), (u32::MAX, 0)),
        (1_u64 << 32, (0, 1)),
        (0x0123_4567_89ab_cdef, (0x89ab_cdef, 0x0123_4567)),
        (u64::MAX, (u32::MAX, u64::from(u32::MAX))),
    ] {
        assert_eq!(split_wide_limb(wide), expected);
    }
    let value = Quantity::from(u64::from(u32::MAX));
    let scaled = FastpqQuantityUnits::from_quantity(&value, 1).unwrap();
    assert_eq!(&scaled.limbs()[..2], &[0xffff_fff6, 9]);
    assert_eq!(scaled.to_quantity(), Some(value));
}
