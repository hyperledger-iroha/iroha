//! Send arithmetic, exact unchanged-state fields and send-chain append.
//!
//! This is a component, never a complete Send acceptance interface. The owner
//! must authenticate the predecessor/context and credential, bind a statement
//! and receipt, and compose all pending, fee, quota and time relations.

use ff::Field;
use iroha_pasta::{Fp, poseidon};

use crate::{
    certificate::{self, Cells},
    fs::SpikeRng,
    full::Wiring,
    monetary_state::StateCells,
    request::{RequestCells, word},
    uniform::{Lin, constant, link, own, plus, scaled},
};

pub const CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"kgwschn1");

/// Explicit constrained source in the shared global family. `instance` also
/// permits a source from the opened public-IO leaves of a verified predecessor.
#[derive(Clone, Copy)]
pub struct Source {
    pub array: usize,
    pub cell: usize,
    pub instance: usize,
}
impl Source {
    pub fn linear(self) -> Lin<Fp> {
        link(self.array, self.cell, vec![Some(self.instance)], Fp::ONE)
    }
    pub fn value(self, w: &Wiring<Fp>) -> Fp {
        w.arrays[self.array].values[self.cell][self.instance]
    }
}

/// Results to join to the mandatory map/time/statement owners.
pub struct SendArithmetic {
    pub local: usize,
    pub gross: usize,
    pub adjusted_burn: usize,
    pub remaining_spendable: usize,
    pub send_chain: usize,
}

fn range128(c: &mut Cells, cell: usize, value: Fp) {
    use ff::PrimeField;
    let repr = value.to_repr();
    let bytes: Vec<_> = repr[..16].iter().map(|b| c.byte(*b)).collect();
    c.eq(own(cell, 1), c.bytes(&bytes));
}

fn port(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

/// Compose with already constrained state and Request components. Each source
/// is linked by a relation equality; host witness values grant no authority.
pub fn append_send_arithmetic(
    w: &mut Wiring<Fp>,
    family: usize,
    before: &StateCells,
    after: &StateCells,
    request: &RequestCells,
    adjusted_burn: Source,
) -> SendArithmetic {
    for array in [
        before.local,
        after.local,
        request.local,
        adjusted_burn.array,
    ] {
        assert_eq!(
            w.arrays[array].family, family,
            "shared global source family"
        );
    }
    let previous = |i| port(before.local, before.core[i]);
    let next = |i| port(after.local, after.core[i]);
    let req = |i| port(request.local, request.words[i]);
    let mut c = Cells::new(family);
    c.a.name = "Send checked arithmetic and state preservation";
    // StateCells permits Active and Retiring. Both may Send remaining value;
    // the unchanged-field equality below preserves the exact lifecycle.
    let amount = req(word::AMOUNT).value(w);
    let amount_inverse = c.cell(Option::<Fp>::from(amount.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([
        req(word::AMOUNT).linear(),
        own(amount_inverse, 1),
        constant(vec![Fp::ONE]),
    ]);
    let gross_value = amount + req(word::FEE).value(w);
    let gross = c.cell(gross_value, 128);
    range128(&mut c, gross, gross_value);
    c.eq(
        own(gross, 1),
        plus(&req(word::AMOUNT).linear(), &req(word::FEE).linear()),
    );
    let burn_value = adjusted_burn.value(w);
    let burn = c.cell(burn_value, 128);
    range128(&mut c, burn, burn_value);
    c.eq(own(burn, 1), adjusted_burn.linear());
    let remaining_value = previous(9).value(w) - gross_value - burn_value;
    let remaining = c.cell(remaining_value, 128);
    range128(&mut c, remaining, remaining_value);
    c.eq(
        previous(9).linear(),
        plus(&plus(&own(gross, 1), &own(burn, 1)), &own(remaining, 1)),
    );
    // Burned value limits affordability but is not debited a second time.
    c.eq(
        next(9).linear(),
        plus(&previous(9).linear(), &scaled(&own(gross, 1), -Fp::ONE)),
    );
    c.eq(next(10).linear(), own(burn, 1));
    for index in [11, 12] {
        c.eq(
            next(index).linear(),
            plus(&previous(index).linear(), &constant(vec![Fp::ONE])),
        );
    }
    c.eq(req(word::ORDINAL).linear(), previous(12).linear());
    for (request_indices, core_start) in
        [(word::SCHEME, 1), (word::ASSET, 3), (word::PAYER_WALLET, 5)]
    {
        for (i, r) in request_indices.into_iter().enumerate() {
            c.eq(req(r).linear(), previous(core_start + i).linear());
        }
    }
    // Maps, quota usage, time floor and nonce have separate mandatory owners.
    // Every other state field and every rest field are preserved here.
    const CHANGED: [usize; 10] = [9, 10, 11, 12, 15, 18, 20, 21, 28, 29];
    for index in 0..30 {
        if !CHANGED.contains(&index) {
            c.eq(next(index).linear(), previous(index).linear());
        }
    }
    for i in 0..6 {
        c.eq(
            port(after.local, after.rest[i]).linear(),
            port(before.local, before.rest[i]).linear(),
        );
    }
    let inputs = [
        previous(15),
        port(request.local, request.credit),
        req(word::RECEIVER_WALLET[0]),
        req(word::RECEIVER_WALLET[1]),
        previous(12),
        req(word::AMOUNT),
        req(word::FEE),
        port(request.local, request.digest),
    ];
    let values: Vec<_> = inputs.iter().map(|p| p.value(w)).collect();
    let chain = poseidon::hash_with_domain(CHAIN_DOMAIN, &values);
    let outputs = [
        c.cell(Fp::ZERO, 255),
        c.cell(chain, 255),
        c.cell(Fp::ZERO, 255),
    ];
    c.eq(next(15).linear(), own(outputs[1], 1));
    let items: Vec<_> = inputs
        .into_iter()
        .map(|p| (p.value(w), p.linear()))
        .collect();
    let local = w.push(c.a);
    certificate::hash_relation(
        w,
        family,
        local,
        CHAIN_DOMAIN,
        &items,
        outputs,
        &mut SpikeRng::new(b"exact Send chain"),
    );
    SendArithmetic {
        local,
        gross,
        adjusted_burn: burn,
        remaining_spendable: remaining,
        send_chain: outputs[1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        monetary_state, nonnative,
        r1cs::{Circuit, Instance, Witness},
        request, spartan,
        uniform::{Array, UniformCircuit},
    };
    use ff::PrimeField;
    use iroha_pasta::Eq as Vesta;

    type Built = (
        UniformCircuit<Fp>,
        Instance<Fp>,
        Witness<Fp>,
        SendArithmetic,
    );

    fn build(
        change: impl FnOnce(&mut [Fp; 30], &mut [Fp; 30], &mut [Fp; 6], &mut [Fp; 6], Fp),
    ) -> Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let rq = request::append_completed(&mut w, family, &request::reference_request());
        let rv = |i: usize| w.arrays[rq.local].values[rq.words[i]][0];
        let mut before = [Fp::ZERO; 30];
        before[0] = Fp::ONE;
        for (req, co) in [(word::SCHEME, 1), (word::ASSET, 3), (word::PAYER_WALLET, 5)] {
            for i in 0..2 {
                before[co + i] = rv(req[i]);
            }
        }
        for i in [7, 8, 17, 18, 19, 20, 21, 29] {
            before[i] = Fp::from(50 + i as u64);
        }
        before[9] = Fp::from(2000);
        before[10] = Fp::from(10);
        before[11] = Fp::from(5);
        before[12] = rv(word::ORDINAL);
        before[15] = Fp::from(91);
        before[16] = Fp::from(92);
        before[27] = rv(word::POLICY_EPOCH);
        before[28] = rv(word::ACCEPTED_TIME) - Fp::ONE;
        let mut br = [Fp::ZERO; 6];
        br[1] = rv(word::SCHEME_POLICY);
        br[2] = rv(word::FEE_SCHEDULE);
        let burn = Fp::from(30);
        let gross = rv(word::AMOUNT) + rv(word::FEE);
        let mut after = before;
        after[9] -= gross;
        after[10] = burn;
        after[11] += Fp::ONE;
        after[12] += Fp::ONE;
        after[18] += Fp::ONE;
        after[20] += Fp::ONE;
        after[28] = rv(word::ACCEPTED_TIME);
        after[29] += Fp::ONE;
        after[15] = poseidon::hash_with_domain(
            CHAIN_DOMAIN,
            &[
                before[15],
                w.arrays[rq.local].values[rq.credit][0],
                rv(word::RECEIVER_WALLET[0]),
                rv(word::RECEIVER_WALLET[1]),
                before[12],
                rv(word::AMOUNT),
                rv(word::FEE),
                w.arrays[rq.local].values[rq.digest][0],
            ],
        );
        let mut ar = br;
        change(&mut before, &mut after, &mut br, &mut ar, gross);
        let bp = monetary_state::append_state(&mut w, family, &before, &br);
        let ap = monetary_state::append_state(&mut w, family, &after, &ar);
        let burned = w.push(Array {
            name: "authenticated adjusted burn fixture",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![128],
            rows: vec![],
            values: vec![vec![burn]],
        });
        w.io_arrays.push((burned, 8));
        let component = append_send_arithmetic(&mut w, family, &bp, &ap, &rq, port(burned, 0));
        let ranges = w.push(Array {
            name: "Send range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|v| Fp::from(v as u64)).collect()],
        });
        w.tables.push((ranges, nonnative::RANGE_TABLE));
        let sources = [
            port(bp.local, bp.commitment),
            port(ap.local, ap.commitment),
            port(component.local, component.gross),
            port(component.local, component.remaining_spendable),
        ];
        let values = sources.map(|p| p.value(&w));
        let io = w.push(Array {
            name: "Send arithmetic test context",
            family,
            rho: 2,
            r_count: 4,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values.to_vec()],
        });
        w.io_arrays.push((io, 4));
        let rows = sources
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                [
                    plus(&p.linear(), &link(io, 0, vec![Some(i)], -Fp::ONE)),
                    constant(vec![Fp::ONE]),
                    vec![],
                ]
            })
            .collect();
        w.push(Array {
            name: "Send arithmetic context joins",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![],
            rows,
            values: vec![],
        });
        let (a, pi, n, t, copies) = w.finish();
        let (c, i, w) = UniformCircuit::build_with_copies(
            "Send arithmetic",
            a,
            pi,
            n,
            t,
            copies,
            12,
            16,
            b"Send arithmetic",
        );
        (c, i, w, component)
    }

    #[test]
    fn active_and_retiring_send_use_one_matrix_and_verify() {
        let active = build(|_, _, _, _, _| {});
        let retiring = build(|b, a, _, _, _| {
            b[0] = Fp::from(2);
            a[0] = Fp::from(2);
        });
        assert!(holds(&active) && holds(&retiring));
        assert_eq!(active.1.io[6], Fp::from(1006));
        assert_eq!(active.1.io[7], Fp::from(964));
        assert_eq!(
            active.0.entries(&[Fp::from(31), Fp::from(47)]),
            retiring.0.entries(&[Fp::from(31), Fp::from(47)])
        );
        let gens = Gens::<Vesta>::new(active.0.shape.cols());
        for (c, i, w, _) in [&active, &retiring] {
            let (p, _) = spartan::prove_circuit(
                &c.shape,
                &gens,
                i,
                w,
                &mut SpikeRng::new(b"Send arithmetic proof"),
                c,
            );
            assert!(spartan::verify_circuit(&c.shape, &gens, i, &p.to_bytes(), c).is_ok());
            for k in [4, 5, 8] {
                let mut wrong = i.clone();
                wrong.io[k] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(&c.shape, &gens, &wrong, &p.to_bytes(), c).is_err()
                );
            }
        }
    }

    #[test]
    fn affordability_uses_adjusted_burn_without_debiting_it_twice() {
        assert!(
            holds(&build(|b, a, _, _, gross| {
                b[9] = gross + Fp::from(30);
                a[9] = Fp::from(30);
            })),
            "exact spendable boundary"
        );
        assert!(
            !holds(&build(|b, a, _, _, gross| {
                b[9] = gross + Fp::from(29);
                a[9] = Fp::from(29);
            })),
            "one unit below adjusted affordability"
        );
        assert!(
            !holds(&build(|_, a, _, _, _| {
                a[9] -= Fp::from(30);
            })),
            "burn cannot be debited twice"
        );
        assert!(
            !holds(&build(|_, a, _, _, _| {
                a[9] += Fp::ONE;
            })),
            "gross debit cannot be reduced"
        );
        assert!(
            !holds(&build(|b, a, _, _, _| {
                a[10] = b[10];
            })),
            "stale core burn cannot replace adjusted burn"
        );
    }

    #[test]
    fn sequence_ordinal_and_chain_changes_are_exact() {
        for k in [11, 12, 15] {
            assert!(
                !holds(&build(|_, a, _, _, _| {
                    a[k] += Fp::ONE;
                })),
                "changed successor field {k}"
            );
        }
        assert!(
            !holds(&build(|b, a, _, _, _| {
                b[11] = Fp::from_u128(u128::MAX);
                a[11] = Fp::ZERO;
            })),
            "sequence cannot wrap"
        );
        assert!(
            !holds(&build(|b, _, _, _, _| {
                b[12] += Fp::ONE;
            })),
            "Request ordinal must match the predecessor"
        );
    }

    #[test]
    fn every_unrelated_core_and_rest_field_is_preserved() {
        for k in 0..30 {
            if [9, 10, 11, 12, 15, 18, 20, 21, 28, 29].contains(&k) {
                continue;
            }
            assert!(
                !holds(&build(|_, a, _, _, _| {
                    a[k] += Fp::ONE;
                })),
                "preserved core field {k}"
            );
        }
        for k in 0..6 {
            assert!(
                !holds(&build(|_, _, _, a, _| {
                    a[k] += Fp::ONE;
                })),
                "preserved rest field {k}"
            );
        }
        for k in [1, 3, 5] {
            assert!(
                !holds(&build(|b, a, _, _, _| {
                    b[k] += Fp::ONE;
                    a[k] += Fp::ONE;
                })),
                "Request identity field {k}"
            );
        }
    }

    fn holds(b: &Built) -> bool {
        let (c, i, w, _) = b;
        let mut values = w.w.clone();
        let r = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = r.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
}
