//! Credential evidence, renewal and regulatory validity on the signed byte tape.
//!
//! The 532-byte layout is the approved future-generation credential. The caller
//! owns authentication, key/point validation and identifier bindings; this helper
//! adds the self-contained validity rules from the native credential relation.
//! Issuer admission (original attestation, revocation and application policy) and
//! state/lineage joins remain separate. In particular, native record validity
//! does not require the stricter issuer-admission facts Android bit 10 or Apple
//! bit 8, and fresh evidence need not repeat enrollment's required facts.

use ff::Field;
use iroha_pasta::Fp;

use crate::{
    certificate::Cells,
    uniform::{Lin, constant, own, plus, scaled},
};

const BODY_BYTES: usize = 532;
const KIND: usize = 291;
const ENROLLMENT: usize = 292;
const FRESH: usize = 348;
const RECORD_BYTES: usize = 56;
const CONTROLS: usize = 436;
const RESPONSE: usize = 440;
const RENEWAL: usize = 488;
const LEASE: usize = 492;

fn sum_bytes(body: &[u8; BODY_BYTES], start: usize, len: usize) -> (Lin<Fp>, Fp) {
    let expression = (start..start + len).fold(vec![], |a, k| plus(&a, &own(k, 1)));
    let value = Fp::from(
        body[start..start + len]
            .iter()
            .map(|b| *b as u64)
            .sum::<u64>(),
    );
    (expression, value)
}

fn nonzero(c: &mut Cells, expression: Lin<Fp>, value: Fp) {
    let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows
        .push([expression, own(inverse, 1), constant(vec![Fp::ONE])]);
}

/// Return a constrained zero predicate, including both implications. The
/// bounded byte sums used here cannot wrap the field or cancel nonzero bytes.
fn is_zero(c: &mut Cells, expression: Lin<Fp>, value: Fp) -> usize {
    let zero = c.bit(value == Fp::ZERO);
    let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([
        expression.clone(),
        own(inverse, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(zero, 1), -Fp::ONE)),
    ]);
    c.a.rows.push([expression, own(zero, 1), vec![]]);
    zero
}

fn bits32(c: &mut Cells, body: &[u8; BODY_BYTES], start: usize) -> [usize; 32] {
    let bits = std::array::from_fn(|i| c.bit(body[start + i / 8] & (1 << (i % 8)) != 0));
    for byte in 0..4 {
        let packed = (0..8).fold(vec![], |a, bit| {
            plus(
                &a,
                &scaled(&own(bits[8 * byte + bit], 1), Fp::from(1 << bit)),
            )
        });
        c.eq(own(start + byte, 1), packed);
    }
    bits
}

fn require_bit(c: &mut Cells, gate: &Lin<Fp>, bit: usize, expected: bool) {
    let difference = if expected {
        plus(&constant(vec![Fp::ONE]), &scaled(&own(bit, 1), -Fp::ONE))
    } else {
        own(bit, 1)
    };
    c.a.rows.push([gate.clone(), difference, vec![]]);
}

/// Constrain LE64(a) <= LE64(b) as A + D = B with eight range-checked
/// difference bytes, boolean carries and final carry zero. Even A > B produces
/// a witness of the same shape; it is rejected by the final carry constraint.
fn le64(c: &mut Cells, body: &[u8; BODY_BYTES], a: usize, b: usize) {
    let av = u64::from_le_bytes(body[a..a + 8].try_into().unwrap()) as u128;
    let bv = u64::from_le_bytes(body[b..b + 8].try_into().unwrap()) as u128;
    let difference = ((bv + (1u128 << 64) - av) % (1u128 << 64)) as u64;
    let bytes = difference.to_le_bytes();
    let mut carry = false;
    let mut previous = None;
    for i in 0..8 {
        let d = c.byte(bytes[i]);
        let next = (body[a + i] as u16 + bytes[i] as u16 + u16::from(carry)) > 255;
        let next_cell = c.bit(next);
        let mut row = plus(&own(a + i, 1), &own(d, 1));
        if let Some(cell) = previous {
            row = plus(&row, &own(cell, 1));
        }
        row = plus(&row, &scaled(&own(b + i, 1), -Fp::ONE));
        row = plus(&row, &scaled(&own(next_cell, 1), -Fp::from(256)));
        c.zero(row);
        previous = Some(next_cell);
        carry = next;
    }
    c.zero(own(previous.unwrap(), 1));
}

/// Add the complete self-contained evidence, renewal and regulatory predicate.
///
/// Cells 0..532 must be the exact range-checked credential body bytes already
/// authenticated by the caller. `body` supplies witnesses only: no body value
/// becomes a coefficient, table, selector or host acceptance decision.
pub(crate) fn constrain(c: &mut Cells, body: &[u8; BODY_BYTES]) {
    assert!(c.a.values.len() >= BODY_BYTES);
    let kind: [usize; 3] = std::array::from_fn(|i| c.bit(body[KIND] as usize == i + 1));
    let one_kind = kind.iter().fold(vec![], |a, k| plus(&a, &own(*k, 1)));
    c.eq(one_kind, constant(vec![Fp::ONE]));
    let tag = kind.iter().enumerate().fold(vec![], |a, (i, k)| {
        plus(&a, &scaled(&own(*k, 1), Fp::from(i as u64 + 1)))
    });
    c.eq(own(KIND, 1), tag);
    let android = plus(&own(kind[0], 1), &own(kind[1], 1));
    let apple = own(kind[2], 1);

    for start in [ENROLLMENT, FRESH] {
        let (expression, value) = sum_bytes(body, start, 32);
        nonzero(c, expression, value);
        let facts = bits32(c, body, start + 40);
        for index in std::iter::once(9).chain(12..32) {
            c.zero(own(facts[index], 1));
        }
        for index in 6..9 {
            require_bit(c, &android, facts[index], false);
        }
        for index in 0..6 {
            require_bit(c, &apple, facts[index], false);
        }
        for byte in start + 44..start + RECORD_BYTES {
            c.a.rows.push([apple.clone(), own(byte, 1), vec![]]);
        }
        if start == ENROLLMENT {
            for index in [0, 2, 3, 5] {
                require_bit(c, &android, facts[index], true);
            }
            require_bit(c, &own(kind[0], 1), facts[1], false);
            require_bit(c, &own(kind[1], 1), facts[1], true);
            for index in [6, 7] {
                require_bit(c, &apple, facts[index], true);
            }
        }
    }

    let (sequence, value) = sum_bytes(body, RENEWAL, 4);
    let first = is_zero(c, sequence, value);
    for i in 0..RECORD_BYTES {
        c.a.rows.push([
            own(first, 1),
            plus(
                &own(ENROLLMENT + i, 1),
                &scaled(&own(FRESH + i, 1), -Fp::ONE),
            ),
            vec![],
        ]);
    }
    // Renewal zero already makes the times equal; this unconditional comparison
    // is equivalent to the native first/every-later-renewal branches.
    le64(c, body, ENROLLMENT + 32, FRESH + 32);

    let controls = bits32(c, body, CONTROLS);
    for bit in &controls[2..] {
        c.zero(own(*bit, 1));
    }
    let time_rules = plus(&own(controls[0], 1), &own(controls[1], 1));
    let time_value =
        Fp::from(u64::from(body[CONTROLS] & 1 != 0) + u64::from(body[CONTROLS] & 2 != 0));
    let no_time = is_zero(c, time_rules, time_value);
    let (response, value) = sum_bytes(body, RESPONSE, 8);
    let no_response = is_zero(c, response, value);
    c.eq(own(no_time, 1), own(no_response, 1));
    let (lease, value) = sum_bytes(body, LEASE, 8);
    let no_lease = is_zero(c, lease, value);
    c.eq(
        own(no_lease, 1),
        plus(
            &constant(vec![Fp::ONE]),
            &scaled(&own(controls[1], 1), -Fp::ONE),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        nonnative::{CHUNK, RANGE_TABLE},
        r1cs::{Circuit, Instance, Witness},
        uniform::{Array, UniformCircuit},
    };

    fn put32(body: &mut [u8; BODY_BYTES], start: usize, value: u32) {
        body[start..start + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put64(body: &mut [u8; BODY_BYTES], start: usize, value: u64) {
        body[start..start + 8].copy_from_slice(&value.to_le_bytes());
    }
    fn facts(body: &mut [u8; BODY_BYTES], start: usize, value: u32) {
        put32(body, start + 40, value);
    }
    fn first(kind: u8) -> [u8; BODY_BYTES] {
        let mut body = [0; BODY_BYTES];
        body[KIND] = kind;
        body[ENROLLMENT..ENROLLMENT + 32].fill(0x11);
        put64(&mut body, ENROLLMENT + 32, 100);
        facts(
            &mut body,
            ENROLLMENT,
            match kind {
                1 => 0b10_1101,
                2 => 0b10_1111,
                3 => (1 << 6) | (1 << 7),
                _ => unreachable!(),
            },
        );
        body.copy_within(ENROLLMENT..ENROLLMENT + RECORD_BYTES, FRESH);
        body
    }
    fn renewed() -> [u8; BODY_BYTES] {
        let mut body = first(1);
        put32(&mut body, RENEWAL, 1);
        body[FRESH] = 0x22;
        facts(&mut body, FRESH, 0);
        put64(&mut body, FRESH + 32, 101);
        body
    }
    fn build(body: &[u8; BODY_BYTES]) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>) {
        let mut c = Cells::new(0);
        for byte in body {
            c.byte(*byte);
        }
        constrain(&mut c, body);
        let range = Array {
            name: "policy byte range",
            family: 0,
            rho: CHUNK,
            r_count: 1 << CHUNK,
            fixed: true,
            lookups: vec![],
            widths: vec![CHUNK as u16],
            rows: vec![],
            values: vec![(0..1u64 << CHUNK).map(Fp::from).collect()],
        };
        UniformCircuit::build(
            "credential policy",
            vec![c.a, range],
            vec![0],
            vec![1],
            vec![(1, RANGE_TABLE)],
            14,
            4,
            b"credential policy",
        )
    }
    fn holds(b: &(UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>)) -> bool {
        let (circuit, instance, witness) = b;
        let mut w = witness.w.clone();
        let relation =
            circuit.instantiate(&circuit.shape, &[Fp::from(31), Fp::from(47)], Some(&mut w));
        let z = crate::r1cs::z_vector(&circuit.shape, instance, &Witness { w });
        let (a, b, c) = relation.abc(&circuit.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
    fn accepted(body: &[u8; BODY_BYTES]) -> bool {
        holds(&build(body))
    }

    #[test]
    fn valid_platforms_renewal_and_controls_use_the_same_relation() {
        let reference = build(&first(1));
        assert!(holds(&reference));
        let challenge = [Fp::from(31), Fp::from(47)];
        for body in [first(2), first(3), renewed()] {
            let candidate = build(&body);
            assert!(holds(&candidate));
            assert_eq!(
                reference.0.entries(&challenge),
                candidate.0.entries(&challenge),
                "platform and renewal choices are witnesses, not circuit constants"
            );
        }
        for mask in 0..=3 {
            let mut body = first(1);
            put32(&mut body, CONTROLS, mask);
            put64(&mut body, RESPONSE, u64::from(mask != 0));
            put64(&mut body, LEASE, if mask & 2 != 0 { u64::MAX } else { 0 });
            assert!(accepted(&body), "defined controls mask {mask}");
        }
        // These are record-validity rules, not stricter issuance policy. Both
        // optional facts remain allowed; fresh evidence need not repeat the
        // enrollment's required flags, and equality of renewal times is valid.
        let mut body = renewed();
        facts(&mut body, FRESH, (1 << 10) | (1 << 11));
        put32(&mut body, RENEWAL, u32::MAX);
        put64(&mut body, ENROLLMENT + 32, u64::MAX);
        put64(&mut body, FRESH + 32, u64::MAX);
        assert!(accepted(&body));
        put64(&mut body, ENROLLMENT + 32, 0);
        assert!(accepted(&body));
    }

    #[test]
    fn malformed_evidence_kinds_facts_and_patches_are_unsatisfied() {
        for kind in [0, 4, 255] {
            let mut body = first(1);
            body[KIND] = kind;
            assert!(!accepted(&body), "unknown evidence kind {kind}");
        }
        for start in [ENROLLMENT, FRESH] {
            let mut body = renewed();
            body[start..start + 32].fill(0);
            assert!(!accepted(&body), "zero evidence digest at {start}");
            for bit in [9, 12, 31, 6, 7, 8] {
                let mut body = renewed();
                body[start + 40 + bit / 8] |= 1 << (bit % 8);
                assert!(
                    !accepted(&body),
                    "invalid Android evidence bit {bit} at {start}"
                );
            }
            for bit in 0..6 {
                let mut body = first(3);
                put32(&mut body, RENEWAL, 1);
                body[start + 40] |= 1 << bit;
                assert!(
                    !accepted(&body),
                    "invalid Apple evidence bit {bit} at {start}"
                );
            }
            for patch in [44, 48, 52] {
                let mut body = first(3);
                put32(&mut body, RENEWAL, 1);
                body[start + patch] = 1;
                assert!(!accepted(&body), "Apple patch word {patch} at {start}");
            }
        }
        for (kind, required) in [
            (1, &[0, 2, 3, 5][..]),
            (2, &[0, 1, 2, 3, 5][..]),
            (3, &[6, 7][..]),
        ] {
            for bit in required {
                let mut body = first(kind);
                body[ENROLLMENT + 40] &= !(1 << bit);
                body.copy_within(ENROLLMENT..ENROLLMENT + RECORD_BYTES, FRESH);
                assert!(
                    !accepted(&body),
                    "missing enrollment bit {bit} for kind {kind}"
                );
            }
        }
        let mut body = first(1);
        body[ENROLLMENT + 40] |= 2;
        body.copy_within(ENROLLMENT..ENROLLMENT + RECORD_BYTES, FRESH);
        assert!(!accepted(&body), "TEE enrollment cannot claim StrongBox");
    }

    #[test]
    fn renewal_order_and_regulatory_iff_rules_are_unsatisfied() {
        for offset in [0, 32, 40, 55] {
            let mut body = first(1);
            body[FRESH + offset] ^= 1;
            assert!(
                !accepted(&body),
                "renewal zero requires exact record equality, byte {offset}"
            );
        }
        let mut body = renewed();
        put64(&mut body, FRESH + 32, 99);
        assert!(!accepted(&body), "fresh time cannot move backwards");
        put64(&mut body, ENROLLMENT + 32, u64::MAX);
        put64(&mut body, FRESH + 32, 0);
        assert!(!accepted(&body), "u64 comparison cannot wrap");
        for (controls, response, lease) in [
            (4, 0, 0),
            (1 << 31, 0, 0),
            (0, 1, 0),
            (1, 0, 0),
            (2, 0, 1),
            (0, 0, 1),
            (1, 1, 1),
            (2, 1, 0),
        ] {
            let mut body = first(1);
            put32(&mut body, CONTROLS, controls);
            put64(&mut body, RESPONSE, response);
            put64(&mut body, LEASE, lease);
            assert!(
                !accepted(&body),
                "invalid controls/response/lease {controls}/{response}/{lease}"
            );
        }
    }
}
