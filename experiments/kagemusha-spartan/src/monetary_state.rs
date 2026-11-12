//! Canonical future-generation Core30/Rest6 opening and state commitment.
//!
//! These self-contained constraints mirror the native state relation, with the
//! approved prior-credential root at core index 8. Operation owners separately
//! authenticate predecessor heads, credentials, map roots and changed fields.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};

use crate::{
    certificate::{self, Cells},
    fs::SpikeRng,
    full::Wiring,
    uniform::{Lin, constant, own, plus, scaled},
};

pub const CORE_FIELDS: usize = 30;
pub const REST_FIELDS: usize = 6;
pub const CORE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwcore1");
pub const REST_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrest1");

/// Constrained cells in one global array, ready for same-family source joins.
pub struct StateCells {
    pub local: usize,
    pub core: [usize; CORE_FIELDS],
    pub rest: [usize; REST_FIELDS],
    pub rest_digest: usize,
    pub commitment: usize,
}

pub fn commitment(core: &[Fp; CORE_FIELDS], rest: &[Fp; REST_FIELDS]) -> Fp {
    let mut preimage = core.to_vec();
    preimage.push(poseidon::hash_with_domain(REST_DOMAIN, rest));
    poseidon::hash_with_domain(CORE_DOMAIN, &preimage)
}

fn bound(c: &mut Cells, cell: usize, value: Fp, bytes: usize) {
    let repr = value.to_repr();
    let indices: Vec<_> = repr[..bytes].iter().map(|b| c.byte(*b)).collect();
    c.eq(own(cell, 1), c.bytes(&indices));
}

fn nonzero(c: &mut Cells, expression: Lin<Fp>, value: Fp) {
    let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows
        .push([expression, own(inverse, 1), constant(vec![Fp::ONE])]);
}

fn is_zero(c: &mut Cells, expression: Lin<Fp>, value: Fp) -> usize {
    let z = c.bit(value == Fp::ZERO);
    let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([expression.clone(), own(z, 1), vec![]]);
    c.a.rows.push([
        expression,
        own(inverse, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(z, 1), -Fp::ONE)),
    ]);
    z
}

fn mask(c: &mut Cells, cell: usize, value: Fp) -> [usize; 2] {
    let bits = [
        c.bit(value.to_repr()[0] & 1 != 0),
        c.bit(value.to_repr()[0] & 2 != 0),
    ];
    c.eq(
        own(cell, 1),
        plus(&own(bits[0], 1), &scaled(&own(bits[1], 1), Fp::from(2))),
    );
    bits
}

/// Append the generic state relation. The shared byte-range table must exist
/// in `w`; widths are constraints, never merely prover width hints. Invalid
/// values build the same template and fail its constraints.
pub fn append_state(
    w: &mut Wiring<Fp>,
    family: usize,
    core: &[Fp; CORE_FIELDS],
    rest: &[Fp; REST_FIELDS],
) -> StateCells {
    let mut c = Cells::new(family);
    c.a.name = "canonical monetary Core30 and Rest6";
    let ci = core.map(|v| c.cell(v, 255));
    let ri = rest.map(|v| c.cell(v, 255));
    c.a.rows.push([
        plus(&own(ci[0], 1), &constant(vec![-Fp::ONE])),
        plus(&own(ci[0], 1), &constant(vec![-Fp::from(2)])),
        vec![],
    ]);
    for i in (1..7).chain(9..15) {
        bound(&mut c, ci[i], core[i], 16);
    }
    for i in 24..29 {
        bound(&mut c, ci[i], core[i], 8);
    }
    bound(&mut c, ri[4], rest[4], 8);
    for start in [1, 3, 5] {
        nonzero(
            &mut c,
            plus(&own(ci[start], 1), &own(ci[start + 1], 1)),
            core[start] + core[start + 1],
        );
    }
    for i in [7, 8, 17, 18, 19, 20, 21, 29] {
        nonzero(&mut c, own(ci[i], 1), core[i]);
    }

    let permitted = mask(&mut c, ri[0], rest[0]);
    let enabled = mask(&mut c, ci[22], core[22]);
    for i in 0..2 {
        c.a.rows
            .push([own(enabled[i], 1), own(permitted[i], 1), own(enabled[i], 1)]);
    }
    let no_time = is_zero(
        &mut c,
        plus(&own(permitted[0], 1), &own(permitted[1], 1)),
        Fp::from((rest[0].to_repr()[0] & 1) as u64 + ((rest[0].to_repr()[0] >> 1) & 1) as u64),
    );
    let no_response = is_zero(&mut c, own(ci[25], 1), core[25]);
    c.eq(own(no_time, 1), own(no_response, 1));
    let no_lease = is_zero(&mut c, own(ci[26], 1), core[26]);
    c.eq(
        own(no_lease, 1),
        plus(
            &constant(vec![Fp::ONE]),
            &scaled(&own(permitted[1], 1), -Fp::ONE),
        ),
    );

    let no_policy = is_zero(&mut c, own(ci[27], 1), core[27]);
    let no_policy_digest = is_zero(&mut c, own(ri[1], 1), rest[1]);
    c.eq(own(no_policy, 1), own(no_policy_digest, 1));
    for cell in [ci[22], ri[2]] {
        c.a.rows.push([own(no_policy, 1), own(cell, 1), vec![]]);
    }
    let no_share = is_zero(&mut c, own(ri[4], 1), rest[4]);
    for (cell, value) in [(ci[23], core[23]), (ci[24], core[24]), (ri[3], rest[3])] {
        let zero = is_zero(&mut c, own(cell, 1), value);
        c.eq(own(no_share, 1), own(zero, 1));
    }

    let rd = poseidon::hash_with_domain(REST_DOMAIN, rest);
    let head = commitment(core, rest);
    let rest_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(rd, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let head_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(head, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let rest_items: Vec<_> = rest.iter().zip(ri).map(|(v, k)| (*v, own(k, 1))).collect();
    let core_items: Vec<_> = core
        .iter()
        .zip(ci)
        .map(|(v, k)| (*v, own(k, 1)))
        .chain(std::iter::once((rd, own(rest_state[1], 1))))
        .collect();
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"canonical future state hashes");
    certificate::hash_relation(
        w,
        family,
        local,
        REST_DOMAIN,
        &rest_items,
        rest_state,
        &mut rng,
    );
    certificate::hash_relation(
        w,
        family,
        local,
        CORE_DOMAIN,
        &core_items,
        head_state,
        &mut rng,
    );
    StateCells {
        local,
        core: ci,
        rest: ri,
        rest_digest: rest_state[1],
        commitment: head_state[1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        nonnative,
        r1cs::{Circuit, Instance, Witness},
        spartan,
        uniform::{Array, UniformCircuit, link},
    };
    use iroha_pasta::Eq as Vesta;

    fn fixture(controls: bool) -> ([Fp; 30], [Fp; 6]) {
        let mut core = [Fp::ZERO; 30];
        let mut rest = [Fp::ZERO; 6];
        core[0] = Fp::ONE;
        for i in [1, 3, 5, 7, 8, 17, 18, 19, 20, 21, 29] {
            core[i] = Fp::from(i as u64 + 10);
        }
        core[9] = Fp::from(100);
        core[10] = Fp::from(20);
        if controls {
            rest[0] = Fp::from(3);
            rest[1] = Fp::from(80);
            rest[2] = Fp::from(81);
            rest[3] = Fp::from(82);
            rest[4] = Fp::from(83);
            rest[5] = Fp::from(84);
            core[22] = Fp::from(3);
            core[23] = Fp::from(90);
            core[24] = Fp::from(2000);
            core[25] = Fp::from(100);
            core[26] = Fp::from(3000);
            core[27] = Fp::from(4);
            core[28] = Fp::from(500);
        }
        (core, rest)
    }

    fn build(
        core: &[Fp; 30],
        rest: &[Fp; 6],
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, StateCells) {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let ports = append_state(&mut w, family, core, rest);
        let range = w.push(Array {
            name: "state range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|v| Fp::from(v as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let cells = [ports.commitment, ports.rest_digest];
        let io = w.push(Array {
            name: "state commitments",
            family,
            rho: 1,
            r_count: 2,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![cells.map(|k| w.arrays[ports.local].values[k][0]).to_vec()],
        });
        w.io_arrays.push((io, 4));
        for (i, k) in cells.into_iter().enumerate() {
            w.arrays[ports.local].rows.push([
                plus(&own(k, 1), &link(io, 0, vec![Some(i)], -Fp::ONE)),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        let (a, pi, n, t, copies) = w.finish();
        let (c, i, w) = UniformCircuit::build_with_copies(
            "canonical monetary state",
            a,
            pi,
            n,
            t,
            copies,
            12,
            8,
            b"canonical monetary state",
        );
        (c, i, w, ports)
    }

    fn holds(c: &UniformCircuit<Fp>, i: &Instance<Fp>, w: &Witness<Fp>) -> bool {
        let mut values = w.w.clone();
        let r = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = r.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }

    #[test]
    fn exact_state_commitment_and_prior_credential_root_are_bound() {
        let (core, rest) = fixture(false);
        let (c, i, w, p) = build(&core, &rest);
        assert!(holds(&c, &i, &w));
        assert_eq!(i.io[4], commitment(&core, &rest));
        let mut legacy: Vec<_> = core
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 8)
            .map(|(_, v)| *v)
            .collect();
        legacy.push(poseidon::hash_with_domain(REST_DOMAIN, &rest));
        assert_ne!(i.io[4], poseidon::hash_with_domain(CORE_DOMAIN, &legacy));
        for cell in p.core.into_iter().chain(p.rest) {
            let mut wrong = Witness { w: w.w.clone() };
            wrong.w[c.cell_index(p.local, cell, 0)] += Fp::ONE;
            assert!(
                !holds(&c, &i, &wrong),
                "state field must reach constrained commitment"
            );
        }
    }

    #[test]
    fn valid_control_states_prove_under_one_matrix() {
        let (a, ar) = fixture(false);
        let (b, br) = fixture(true);
        let (c, ai, aw, _) = build(&a, &ar);
        let (d, bi, bw, _) = build(&b, &br);
        assert!(holds(&c, &ai, &aw) && holds(&d, &bi, &bw));
        assert_eq!(
            c.entries(&[Fp::from(31), Fp::from(47)]),
            d.entries(&[Fp::from(31), Fp::from(47)])
        );
        let gens = Gens::<Vesta>::new(c.shape.cols());
        for (i, w) in [(&ai, &aw), (&bi, &bw)] {
            let (p, _) = spartan::prove_circuit(
                &c.shape,
                &gens,
                i,
                w,
                &mut SpikeRng::new(b"state proof"),
                &c,
            );
            assert!(spartan::verify_circuit(&c.shape, &gens, i, &p.to_bytes(), &c).is_ok());
            let mut wrong = i.clone();
            wrong.io[4] += Fp::ONE;
            assert!(spartan::verify_circuit(&c.shape, &gens, &wrong, &p.to_bytes(), &c).is_err());
        }
    }

    #[test]
    fn all_integer_widths_and_nonzero_roots_are_constraints() {
        let (core, rest) = fixture(false);
        for k in (1..7).chain(9..15) {
            let mut bad = core;
            bad[k] = Fp::from_u128(u128::MAX) + Fp::ONE;
            let (c, i, w, _) = build(&bad, &rest);
            assert!(!holds(&c, &i, &w), "u128 core {k}");
        }
        for k in 24..29 {
            let mut bad = core;
            bad[k] = Fp::from_u128(1u128 << 64);
            let (c, i, w, _) = build(&bad, &rest);
            assert!(!holds(&c, &i, &w), "u64 core {k}");
        }
        for k in [7, 8, 17, 18, 19, 20, 21, 29] {
            let mut bad = core;
            bad[k] = Fp::ZERO;
            let (c, i, w, _) = build(&bad, &rest);
            assert!(!holds(&c, &i, &w), "nonzero core {k}");
        }
        let mut bad = rest;
        bad[4] = Fp::from_u128(1u128 << 64);
        let (c, i, w, _) = build(&core, &bad);
        assert!(!holds(&c, &i, &w));
        for tag in [0, 3] {
            let mut bad = core;
            bad[0] = Fp::from(tag);
            let (c, i, w, _) = build(&bad, &rest);
            assert!(!holds(&c, &i, &w));
        }
    }

    #[test]
    fn policy_share_lease_and_control_inconsistencies_are_rejected() {
        let (core, rest) = fixture(true);
        for (k, v) in [
            (22, Fp::from(4)),
            (23, Fp::ZERO),
            (24, Fp::ZERO),
            (25, Fp::ZERO),
            (26, Fp::ZERO),
            (27, Fp::ZERO),
        ] {
            let mut bad = core;
            bad[k] = v;
            let (c, i, w, _) = build(&bad, &rest);
            assert!(!holds(&c, &i, &w), "core control {k}");
        }
        for (k, v) in [
            (0, Fp::from(4)),
            (0, Fp::ONE),
            (1, Fp::ZERO),
            (3, Fp::ZERO),
            (4, Fp::ZERO),
        ] {
            let mut bad = rest;
            bad[k] = v;
            let (c, i, w, _) = build(&core, &bad);
            assert!(!holds(&c, &i, &w), "rest control {k}");
        }
        let (c0, r0) = fixture(false);
        for k in [22, 25, 26] {
            let mut bad = c0;
            bad[k] = Fp::ONE;
            let (c, i, w, _) = build(&bad, &r0);
            assert!(!holds(&c, &i, &w));
        }
        let mut bad = r0;
        bad[2] = Fp::ONE;
        let (c, i, w, _) = build(&c0, &bad);
        assert!(!holds(&c, &i, &w));
    }
}
