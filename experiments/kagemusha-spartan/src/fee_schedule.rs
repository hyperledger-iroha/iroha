//! Send's exact held FeeSchedule and integer fee arithmetic.
//!
//! The predecessor state must be authenticated and the Request must be joined
//! to the operation. This component opens the already-held schedule by digest;
//! its issuer signature was checked when installed. It verifies no receiver or
//! issuer signature and is not a complete Send relation.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

use crate::{
    certificate::{self, Cells, field_modulus, fp},
    fs::SpikeRng,
    full::Wiring,
    monetary_state::StateCells,
    request::{RequestCells, word as request_word},
    uniform::{Lin, constant, link, own, plus, scaled},
};

pub const BODY_BYTES: usize = 191;
pub const SIGNING_DOMAIN: u64 = u64::from_le_bytes(*b"kgwfsch1");
pub const OBJECT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwofee1");

#[derive(Clone, Debug)]
pub struct SignedFeeSchedule {
    pub body: [u8; BODY_BYTES],
    pub signature: [u8; 64],
}

/// These ports add fee/epoch checks only; the operation still owes every other
/// state, map, control, receipt and predecessor constraint.
pub struct FeeCells {
    pub local: usize,
    pub has_schedule: usize,
    pub held_digest: usize,
    pub fee: usize,
}

fn dummy() -> SignedFeeSchedule {
    let mut body = [0; BODY_BYTES];
    body[0] = 1;
    for offset in [2, 34, 74, 159] {
        body[offset] = 1;
    }
    body[142..158].fill(255);
    body[158] = 1;
    SignedFeeSchedule {
        body,
        signature: [0; 64],
    }
}

/// Exact native object digest, including every original signature byte.
pub fn digest(input: &SignedFeeSchedule) -> Fp {
    let mut items = vec![Fp::from(BODY_BYTES as u64)];
    items.extend(
        input
            .body
            .chunks(31)
            .map(|b| fp(&BigUint::from_bytes_le(b))),
    );
    let message = poseidon::hash_with_domain(SIGNING_DOMAIN, &items);
    let s = &input.signature;
    poseidon::hash_with_domain(
        OBJECT_DOMAIN,
        &[
            message,
            fp(&BigUint::from_bytes_be(&s[16..32])),
            fp(&BigUint::from_bytes_be(&s[..16])),
            fp(&BigUint::from_bytes_be(&s[48..])),
            fp(&BigUint::from_bytes_be(&s[32..48])),
        ],
    )
}

struct UInt {
    cell: usize,
    bytes: Vec<usize>,
    value: BigUint,
}

fn integer(value: Fp) -> BigUint {
    BigUint::from_bytes_le(value.to_repr().as_ref())
}

fn uint(c: &mut Cells, value: BigUint, n: usize) -> UInt {
    let mut raw = value.to_bytes_le();
    raw.resize(n, 0);
    let bytes: Vec<_> = raw[..n].iter().map(|b| c.byte(*b)).collect();
    let cell = c.cell(fp(&value), 255);
    c.eq(own(cell, 1), c.bytes(&bytes));
    UInt { cell, bytes, value }
}

fn decode(c: &mut Cells, raw: &[u8], indices: &[usize]) -> UInt {
    let value = BigUint::from_bytes_le(raw);
    let cell = c.cell(fp(&value), 255);
    c.eq(own(cell, 1), c.bytes(indices));
    UInt {
        cell,
        bytes: indices.to_vec(),
        value,
    }
}

fn zero(c: &mut Cells, expression: Lin<Fp>, value: Fp) -> usize {
    let z = c.bit(value == Fp::ZERO);
    let inv = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([expression.clone(), own(z, 1), vec![]]);
    c.a.rows.push([
        expression,
        own(inv, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(z, 1), -Fp::ONE)),
    ]);
    z
}

fn nonzero(c: &mut Cells, expression: Lin<Fp>, value: Fp) {
    let inv = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows
        .push([expression, own(inv, 1), constant(vec![Fp::ONE])]);
}

/// Exact integer comparison through bounded byte differences and borrows.
fn less(c: &mut Cells, a: &UInt, b: &UInt) -> usize {
    assert_eq!(a.bytes.len(), b.bytes.len());
    let mut borrow = 0i16;
    let mut prior = None;
    for (&a, &b) in a.bytes.iter().zip(&b.bytes) {
        let av = c.a.values[a][0].to_repr()[0] as i16;
        let bv = c.a.values[b][0].to_repr()[0] as i16;
        let difference = av - bv - borrow;
        let d = c.byte(difference.rem_euclid(256) as u8);
        let bit = c.bit(difference < 0);
        let mut row = plus(&own(a, 1), &scaled(&own(b, 1), -Fp::ONE));
        if let Some(k) = prior {
            row = plus(&row, &scaled(&own(k, 1), -Fp::ONE));
        }
        row = plus(&row, &scaled(&own(bit, 1), Fp::from(256)));
        c.eq(row, own(d, 1));
        prior = Some(bit);
        borrow = i16::from(difference < 0);
    }
    prior.unwrap()
}

fn divide(c: &mut Cells, a: &UInt) -> (UInt, UInt) {
    let q = uint(c, &a.value / 10_000u32, 16);
    let r = uint(c, &a.value % 10_000u32, 2);
    c.eq(
        own(a.cell, 1),
        plus(&scaled(&own(q.cell, 1), Fp::from(10_000)), &own(r.cell, 1)),
    );
    let limit = uint(c, BigUint::from(10_000u32), 2);
    c.eq(own(limit.cell, 1), constant(vec![Fp::from(10_000)]));
    let valid = less(c, &r, &limit);
    c.eq(own(valid, 1), constant(vec![Fp::ONE]));
    (q, r)
}

fn source(
    c: &mut Cells,
    w: &Wiring<Fp>,
    family: usize,
    local: usize,
    index: usize,
    n: usize,
) -> UInt {
    assert_eq!((w.arrays[local].family, w.arrays[local].rho), (family, 0));
    let result = uint(c, integer(w.arrays[local].values[index][0]), n);
    c.eq(
        own(result.cell, 1),
        link(local, index, vec![Some(0)], Fp::ONE),
    );
    result
}

fn field_source(c: &mut Cells, w: &Wiring<Fp>, family: usize, local: usize, index: usize) -> usize {
    assert_eq!((w.arrays[local].family, w.arrays[local].rho), (family, 0));
    let cell = c.cell(w.arrays[local].values[index][0], 255);
    c.eq(own(cell, 1), link(local, index, vec![Some(0)], Fp::ONE));
    cell
}

/// All joins and fee rules are mandatory. Presence is derived from the held
/// digest, not supplied by a caller. With no schedule, selected bytes are a
/// fixed canonical dummy and the required fee is zero; irrelevant raw bytes
/// cannot impose a policy or change the constraint matrix.
pub fn append_held(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &SignedFeeSchedule,
    state: &StateCells,
    request: &RequestCells,
) -> FeeCells {
    let mut c = Cells::new(family);
    c.a.name = "held FeeSchedule and exact fee";
    let held = field_source(&mut c, w, family, state.local, state.rest[2]);
    let held_value = c.a.values[held][0];
    let absent = zero(&mut c, own(held, 1), held_value);
    let present = c.bit(held_value != Fp::ZERO);
    c.eq(
        plus(&own(absent, 1), &own(present, 1)),
        constant(vec![Fp::ONE]),
    );
    let no_schedule = dummy();
    let original: Vec<_> = input.body.iter().chain(&input.signature).copied().collect();
    let fallback: Vec<_> = no_schedule
        .body
        .iter()
        .chain(&no_schedule.signature)
        .copied()
        .collect();
    let selected: Vec<_> = original
        .iter()
        .zip(&fallback)
        .map(|(&a, &b)| if held_value == Fp::ZERO { b } else { a })
        .collect();
    let selected_cells: Vec<_> = original
        .iter()
        .zip(&fallback)
        .zip(&selected)
        .map(|((&a, &b), &value)| {
            let raw = c.byte(a);
            let selected = c.byte(value);
            c.a.rows.push([
                own(present, 1),
                plus(&own(raw, 1), &constant(vec![-Fp::from(b as u64)])),
                plus(&own(selected, 1), &constant(vec![-Fp::from(b as u64)])),
            ]);
            selected
        })
        .collect();
    let offsets = [
        (0, 2),
        (2, 16),
        (18, 16),
        (34, 16),
        (50, 16),
        (66, 8),
        (74, 16),
        (90, 16),
        (106, 4),
        (110, 16),
        (126, 16),
        (142, 16),
        (158, 1),
        (159, 32),
    ];
    let words: Vec<_> = offsets
        .iter()
        .map(|&(at, n)| decode(&mut c, &selected[at..at + n], &selected_cells[at..at + n]))
        .collect();
    c.eq(own(words[0].cell, 1), constant(vec![Fp::ONE]));
    for (a, b) in [(1, 2), (3, 4), (6, 7)] {
        nonzero(
            &mut c,
            plus(&own(words[a].cell, 1), &own(words[b].cell, 1)),
            fp(&(&words[a].value + &words[b].value)),
        );
    }
    c.at_most(
        &words[13].bytes,
        &words[13].value,
        &(field_modulus() - 1u32),
    );
    nonzero(&mut c, own(words[13].cell, 1), fp(&words[13].value));
    let max_rate = uint(&mut c, BigUint::from(10_001u32), 4);
    c.eq(own(max_rate.cell, 1), constant(vec![Fp::from(10_001)]));
    let rate_ok = less(&mut c, &words[8], &max_rate);
    c.eq(own(rate_ok, 1), constant(vec![Fp::ONE]));
    let bad_bounds = less(&mut c, &words[11], &words[10]);
    c.zero(own(bad_bounds, 1));
    let up = c.bit(words[12].value == BigUint::from(2u32));
    c.eq(
        own(words[12].cell, 1),
        plus(&constant(vec![Fp::ONE]), &own(up, 1)),
    );

    let amount = source(
        &mut c,
        w,
        family,
        request.local,
        request.words[request_word::AMOUNT],
        16,
    );
    let requested_fee = source(
        &mut c,
        w,
        family,
        request.local,
        request.words[request_word::FEE],
        16,
    );
    let request_schedule = field_source(
        &mut c,
        w,
        family,
        request.local,
        request.words[request_word::FEE_SCHEDULE],
    );
    c.eq(own(request_schedule, 1), own(held, 1));
    let (whole_amount, remainder_amount) = divide(&mut c, &amount);
    let whole = uint(&mut c, &whole_amount.value * &words[8].value, 16);
    c.a.rows.push([
        own(whole_amount.cell, 1),
        own(words[8].cell, 1),
        own(whole.cell, 1),
    ]);
    let partial = uint(&mut c, &remainder_amount.value * &words[8].value, 4);
    c.a.rows.push([
        own(remainder_amount.cell, 1),
        own(words[8].cell, 1),
        own(partial.cell, 1),
    ]);
    let (part, rem) = divide(&mut c, &partial);
    let rem_zero = zero(&mut c, own(rem.cell, 1), fp(&rem.value));
    let increment_value =
        u32::from(words[12].value == BigUint::from(2u32) && rem.value != BigUint::from(0u32));
    let increment = c.bit(increment_value != 0);
    c.a.rows.push([
        own(up, 1),
        plus(
            &constant(vec![Fp::ONE]),
            &scaled(&own(rem_zero, 1), -Fp::ONE),
        ),
        own(increment, 1),
    ]);
    let proportional = uint(&mut c, &whole.value + &part.value + increment_value, 16);
    c.eq(
        own(proportional.cell, 1),
        plus(
            &plus(&own(whole.cell, 1), &own(part.cell, 1)),
            &own(increment, 1),
        ),
    );
    // This bounded sum is required before clamping: min/max cannot hide overflow.
    let raw = uint(&mut c, &proportional.value + &words[9].value, 16);
    c.eq(
        own(raw.cell, 1),
        plus(&own(proportional.cell, 1), &own(words[9].cell, 1)),
    );
    let below = less(&mut c, &raw, &words[10]);
    let above = less(&mut c, &words[11], &raw);
    let lower_value = if raw.value < words[10].value {
        words[10].value.clone()
    } else {
        raw.value.clone()
    };
    let lower = uint(&mut c, lower_value, 16);
    c.a.rows.push([
        own(below, 1),
        plus(
            &own(words[10].cell, 1),
            &scaled(&own(raw.cell, 1), -Fp::ONE),
        ),
        plus(&own(lower.cell, 1), &scaled(&own(raw.cell, 1), -Fp::ONE)),
    ]);
    let clamped_value = if raw.value > words[11].value {
        words[11].value.clone()
    } else {
        lower.value.clone()
    };
    let clamped = uint(&mut c, clamped_value, 16);
    c.a.rows.push([
        own(above, 1),
        plus(
            &own(words[11].cell, 1),
            &scaled(&own(lower.cell, 1), -Fp::ONE),
        ),
        plus(
            &own(clamped.cell, 1),
            &scaled(&own(lower.cell, 1), -Fp::ONE),
        ),
    ]);
    c.a.rows.push([
        own(present, 1),
        own(clamped.cell, 1),
        own(requested_fee.cell, 1),
    ]);

    for (word, core) in [(1, 1), (2, 2), (3, 3), (4, 4)] {
        let state_word = field_source(&mut c, w, family, state.local, state.core[core]);
        c.a.rows.push([
            own(present, 1),
            plus(
                &own(words[word].cell, 1),
                &scaled(&own(state_word, 1), -Fp::ONE),
            ),
            vec![],
        ]);
    }
    let requested_epoch = source(
        &mut c,
        w,
        family,
        request.local,
        request.words[request_word::POLICY_EPOCH],
        8,
    );
    let installed_epoch = source(&mut c, w, family, state.local, state.core[27], 8);
    let future = less(&mut c, &installed_epoch, &requested_epoch);
    c.zero(own(future, 1));
    let same_epoch = zero(
        &mut c,
        plus(
            &own(requested_epoch.cell, 1),
            &scaled(&own(installed_epoch.cell, 1), -Fp::ONE),
        ),
        fp(&requested_epoch.value) - fp(&installed_epoch.value),
    );
    let requested_policy = field_source(
        &mut c,
        w,
        family,
        request.local,
        request.words[request_word::SCHEME_POLICY],
    );
    let installed_policy = field_source(&mut c, w, family, state.local, state.rest[1]);
    c.a.rows.push([
        own(same_epoch, 1),
        plus(
            &own(requested_policy, 1),
            &scaled(&own(installed_policy, 1), -Fp::ONE),
        ),
        vec![],
    ]);

    let mut message_inputs = vec![(
        Fp::from(BODY_BYTES as u64),
        constant(vec![Fp::from(BODY_BYTES as u64)]),
    )];
    for (bytes, indices) in selected[..BODY_BYTES]
        .chunks(31)
        .zip(selected_cells[..BODY_BYTES].chunks(31))
    {
        let value = fp(&BigUint::from_bytes_le(bytes));
        let bridge = c.cell(value, 255);
        c.eq(own(bridge, 1), c.bytes(indices));
        message_inputs.push((value, own(bridge, 1)));
    }
    let message = poseidon::hash_with_domain(
        SIGNING_DOMAIN,
        &message_inputs.iter().map(|x| x.0).collect::<Vec<_>>(),
    );
    let message_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(message, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let mut object_inputs = vec![(message, own(message_state[1], 1))];
    for offset in [16, 0, 48, 32] {
        let at = BODY_BYTES + offset;
        let indices: Vec<_> = selected_cells[at..at + 16].iter().rev().copied().collect();
        let value = fp(&BigUint::from_bytes_be(&selected[at..at + 16]));
        let bridge = c.cell(value, 255);
        c.eq(own(bridge, 1), c.bytes(&indices));
        object_inputs.push((value, own(bridge, 1)));
    }
    let object = poseidon::hash_with_domain(
        OBJECT_DOMAIN,
        &object_inputs.iter().map(|x| x.0).collect::<Vec<_>>(),
    );
    let object_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(object, 255),
        c.cell(Fp::ZERO, 255),
    ];
    c.a.rows.push([
        own(present, 1),
        plus(&own(object_state[1], 1), &scaled(&own(held, 1), -Fp::ONE)),
        vec![],
    ]);
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"held fee schedule exact hashes");
    certificate::hash_relation(
        w,
        family,
        local,
        SIGNING_DOMAIN,
        &message_inputs,
        message_state,
        &mut rng,
    );
    certificate::hash_relation(
        w,
        family,
        local,
        OBJECT_DOMAIN,
        &object_inputs,
        object_state,
        &mut rng,
    );
    FeeCells {
        local,
        has_schedule: present,
        held_digest: held,
        fee: requested_fee.cell,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        monetary_state, nonnative,
        r1cs::{Circuit, Instance, Witness},
        request::{self, CompletedRequest},
        spartan,
        uniform::{Array, UniformCircuit},
    };
    use iroha_pasta::Eq as Vesta;

    fn hex<const N: usize>(s: &str) -> [u8; N] {
        assert_eq!(s.len(), 2 * N);
        std::array::from_fn(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
    }

    #[test]
    fn held_digest_matches_native_signed_fee_original() {
        // Canonical fixtures/kagemusha/wallet_v1_vectors.json: fee schedule.
        let input = SignedFeeSchedule {
            body: hex(concat!(
                "01009692c8c83bef03c3991f61516a22cdb560ba5a980bfd4a9c3f27339c9d2dd4456",
                "e85849d458ab1a848d48e030c816121e9b08998cc5b2aff7481f2b933875101",
                "0700000000000000e090740d52cb3d9a4969d253e0ada8b815c87385ca103787ac4c72b2b0c38204",
                "190000000300000000000000000000000000000005000000000000000000000000000000",
                "e8030000000000000000000000000000024ef64abbb5dab2425786d4e476447618c107993fa0650b02b6b6118c6ec03d05"
            )),
            signature: hex(concat!(
                "92fc8478f71e253a7740a5f28163aca615bc40e0793a115f927c14d6f405623f",
                "5c3c0022d450e976abe7c577e8f39f308f88e16213bc3f92eb78132fc7fd8bf8"
            )),
        };
        assert_eq!(
            digest(&input).to_repr(),
            hex("70eeae3bc864578883ff559ab9b67792e1aaa00a43bd0fb9a570a83790632527")
        );
    }

    fn schedule(bp: u32, fixed: u128, min: u128, max: u128, up: bool) -> SignedFeeSchedule {
        let mut input = dummy();
        input.body[34] = 2;
        input.body[106..110].copy_from_slice(&bp.to_le_bytes());
        input.body[110..126].copy_from_slice(&fixed.to_le_bytes());
        input.body[126..142].copy_from_slice(&min.to_le_bytes());
        input.body[142..158].copy_from_slice(&max.to_le_bytes());
        input.body[158] = if up { 2 } else { 1 };
        input.signature = std::array::from_fn(|i| i as u8 + 1);
        input
    }

    type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, FeeCells);
    fn build(
        input: &SignedFeeSchedule,
        amount: u128,
        fee: u128,
        held: Fp,
        patches: &[(usize, Vec<u8>)],
    ) -> Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let mut core = [Fp::ZERO; 30];
        core[0] = Fp::ONE;
        core[1] = Fp::ONE;
        core[3] = Fp::from(2);
        core[5] = Fp::from(3);
        for k in [7, 8, 17, 18, 19, 20, 21, 29] {
            core[k] = Fp::ONE;
        }
        core[27] = Fp::from(5);
        let rest = [Fp::ZERO, Fp::from(9), held, Fp::ZERO, Fp::ZERO, Fp::ZERO];
        let state = monetary_state::append_state(&mut w, family, &core, &rest);
        let mut request = CompletedRequest {
            body: [0; request::BODY_BYTES],
            signature: [1; 64],
        };
        request.body[0] = 1;
        for (at, value) in [
            (2, 1),
            (34, 2),
            (66, 3),
            (98, 4),
            (130, 5),
            (162, 6),
            (194, 1),
            (210, 7),
            (306, 5),
            (314, 9),
            (354, 1),
            (386, 1),
        ] {
            request.body[at] = value;
        }
        request.body[242..258].copy_from_slice(&amount.to_le_bytes());
        request.body[258..290].copy_from_slice(held.to_repr().as_ref());
        request.body[290..306].copy_from_slice(&fee.to_le_bytes());
        for (at, bytes) in patches {
            request.body[*at..*at + bytes.len()].copy_from_slice(bytes);
        }
        let request = request::append_completed(&mut w, family, &request);
        let ports = append_held(&mut w, family, input, &state, &request);
        let range = w.push(Array {
            name: "fee byte range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|x| Fp::from(x as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let outputs = [
            (state.local, state.commitment),
            (request.local, request.digest),
            (ports.local, ports.fee),
            (ports.local, ports.has_schedule),
            (ports.local, ports.held_digest),
        ];
        let mut values = vec![Fp::ZERO; 8];
        for (i, (a, k)) in outputs.iter().enumerate() {
            values[i] = w.arrays[*a].values[*k][0];
        }
        let io = w.push(Array {
            name: "fee public boundary",
            family,
            rho: 3,
            r_count: 5,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values],
        });
        w.io_arrays.push((io, 8));
        for (i, (a, k)) in outputs.into_iter().enumerate() {
            w.arrays[ports.local].rows.push([
                plus(
                    &link(a, k, vec![Some(0)], Fp::ONE),
                    &link(io, 0, vec![Some(i)], -Fp::ONE),
                ),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        let (a, pi, n, t, copies) = w.finish();
        let (c, i, w) = UniformCircuit::build_with_copies(
            "held exact fee",
            a,
            pi,
            n,
            t,
            copies,
            12,
            16,
            b"held exact fee",
        );
        (c, i, w, ports)
    }

    fn holds(b: &Built) -> bool {
        let (c, i, w, _) = b;
        let mut values = w.w.clone();
        let rel = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = rel.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }

    #[test]
    fn exact_rounding_clamping_extremes_and_absent_schedule() {
        for (bp, fixed, min, max, up, amount, fee) in [
            (1, 0, 0, u128::MAX, false, 10_001, 1),
            (1, 0, 0, u128::MAX, true, 10_001, 2),
            (1, 0, 0, u128::MAX, true, 10_000, 1),
            (0, 3, 7, 100, false, 100, 7),
            (10_000, 3, 0, 9, false, 100, 9),
            (0, 0, 0, u128::MAX, false, u128::MAX, 0),
            (10_000, 0, 0, u128::MAX, true, u128::MAX / 2, u128::MAX / 2),
        ] {
            let input = schedule(bp, fixed, min, max, up);
            let b = build(&input, amount, fee, digest(&input), &[]);
            assert!(holds(&b), "bp={bp}, rounding_up={up}, amount={amount}");
            assert_eq!(b.1.io[10], Fp::from_u128(fee));
        }
        let malformed = SignedFeeSchedule {
            body: [255; BODY_BYTES],
            signature: [255; 64],
        };
        let b = build(&malformed, 100, 0, Fp::ZERO, &[]);
        assert!(
            holds(&b),
            "irrelevant absent slot is selected out by held digest"
        );
        assert_eq!(b.1.io[11], Fp::ZERO);
    }

    #[test]
    fn malformed_held_bodies_and_preclamp_overflow_are_rejected() {
        let original = schedule(1, 0, 0, u128::MAX, false);
        let mut cases = Vec::new();
        for (name, at, bytes) in [
            ("version", 0, 0u16.to_le_bytes().to_vec()),
            ("rate", 106, 10_001u32.to_le_bytes().to_vec()),
            ("rounding", 158, vec![3]),
            ("zero scheme", 2, vec![0; 32]),
            ("zero asset", 34, vec![0; 32]),
            ("zero beneficiary", 74, vec![0; 32]),
            ("zero certificate", 159, vec![0; 32]),
            ("noncanonical certificate", 159, {
                let mut b = field_modulus().to_bytes_le();
                b.resize(32, 0);
                b
            }),
        ] {
            let mut input = original.clone();
            input.body[at..at + bytes.len()].copy_from_slice(&bytes);
            cases.push((name, input));
        }
        cases.push(("inverted clamp", schedule(1, 0, 10, 9, false)));
        for (name, input) in cases {
            assert!(
                !holds(&build(&input, 10_001, 1, digest(&input), &[])),
                "{name}: exact malformed object digest remains held"
            );
        }
        let overflow = schedule(10_000, u128::MAX, 0, 0, false);
        assert!(
            !holds(&build(&overflow, 1, 0, digest(&overflow), &[])),
            "zero maximum cannot conceal unclamped overflow"
        );
    }

    #[test]
    fn fee_epoch_policy_and_exact_object_joins_are_mandatory() {
        let input = schedule(1, 0, 0, u128::MAX, false);
        let held = digest(&input);
        assert!(
            !holds(&build(&input, 10_001, 2, held, &[])),
            "wrong computed fee"
        );
        assert!(
            !holds(&build(&input, 10_001, 1, held + Fp::ONE, &[])),
            "different held original"
        );
        assert!(
            !holds(&build(
                &input,
                10_001,
                1,
                held,
                &[(258, (held + Fp::ONE).to_repr().to_vec())]
            )),
            "Request schedule must equal held schedule"
        );
        assert!(
            !holds(&build(
                &input,
                10_001,
                1,
                held,
                &[(306, 6u64.to_le_bytes().to_vec())]
            )),
            "future policy epoch"
        );
        assert!(
            !holds(&build(
                &input,
                10_001,
                1,
                held,
                &[(314, Fp::from(10).to_repr().to_vec())]
            )),
            "different policy at equal epoch"
        );
        assert!(
            holds(&build(
                &input,
                10_001,
                1,
                held,
                &[
                    (306, 4u64.to_le_bytes().to_vec()),
                    (314, Fp::from(10).to_repr().to_vec())
                ]
            )),
            "older Request may name its historical policy"
        );
        for at in [2, 34] {
            let mut wrong = input.clone();
            wrong.body[at] += 1;
            assert!(
                !holds(&build(&wrong, 10_001, 1, digest(&wrong), &[])),
                "held schedule context differs"
            );
        }
        let mut changed_signature = input.clone();
        changed_signature.signature[0] ^= 1;
        assert!(
            !holds(&build(&changed_signature, 10_001, 1, held, &[])),
            "every signature byte enters held object digest"
        );
    }

    #[test]
    fn present_absent_and_rotated_schedules_share_one_proof_matrix() {
        let a = schedule(1, 0, 0, u128::MAX, false);
        let b = schedule(1, 0, 0, u128::MAX, true);
        let absent = SignedFeeSchedule {
            body: [255; BODY_BYTES],
            signature: [255; 64],
        };
        let x = build(&a, 10_001, 1, digest(&a), &[]);
        let y = build(&b, 10_001, 2, digest(&b), &[]);
        let z = build(&absent, 10_001, 0, Fp::ZERO, &[]);
        let entries = x.0.entries(&[Fp::from(31), Fp::from(47)]);
        let gens = Gens::<Vesta>::new(x.0.shape.cols());
        for candidate in [&x, &y, &z] {
            assert!(holds(candidate));
            assert_eq!(entries, candidate.0.entries(&[Fp::from(31), Fp::from(47)]));
            let (_, i, w, _) = candidate;
            let (proof, _) = spartan::prove_circuit(
                &x.0.shape,
                &gens,
                i,
                w,
                &mut SpikeRng::new(b"held fee proof"),
                &x.0,
            );
            assert!(spartan::verify_circuit(&x.0.shape, &gens, i, &proof.to_bytes(), &x.0).is_ok());
            let mut wrong = i.clone();
            wrong.io[10] += Fp::ONE;
            assert!(
                spartan::verify_circuit(&x.0.shape, &gens, &wrong, &proof.to_bytes(), &x.0)
                    .is_err()
            );
        }
    }
}
