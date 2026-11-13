use super::*;
use crate::{
    fs::SpikeRng,
    full::{NODE_DOMAIN, constant_entries},
    r1cs::{Circuit, Instance, Witness},
    uniform::UniformCircuit,
};
use group::Curve;
use iroha_pasta::{Ep as Pallas, Eq as Vesta, Fp, Fq};
fn holds<F: Scalar + PoseidonField>(
    (c, i, w): &(UniformCircuit<F>, Instance<F>, Witness<F>),
) -> bool {
    let mut v = w.w.clone();
    let r = c.instantiate(&c.shape, &[F::from(31), F::from(47)], Some(&mut v));
    let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: v });
    let (a, b, c) = r.abc(&c.shape, &z);
    a.into_iter().zip(b).zip(c).all(|((a, b), c)| a * b == c)
}
fn fingerprint<F: Scalar + PoseidonField>(c: &UniformCircuit<F>) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    for a in 0..c.arrays.len() {
        for (m, r, k, v) in c.entries_of(&[F::from(31), F::from(47)], Some(a)) {
            for i in [m, r, k] {
                h.update(&(i as u64).to_le_bytes());
            }
            h.update(v.to_repr().as_ref());
        }
    }
    *h.finalize().as_bytes()
}
fn build<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    claims: &[Claim<C>],
) -> (UniformCircuit<C::Base>, Instance<C::Base>, Witness<C::Base>)
where
    C::Base: Scalar + PoseidonField,
{
    let tr = crate::verifier::trace(
        shape,
        gens,
        inst,
        proof,
        claims,
        &crate::spartan::Link::None,
    );
    let values = crate::verifier::shared_values(&tr, proof, claims);
    let mut w = Wiring::<C::Base>::default();
    let f = w.family(0, 1);
    let e_out = w.reserve();
    let e_v = w.reserve();
    let e_zero = constant_entries(&mut w, f, "zero", &[vec![C::Base::ZERO]]);
    let mut pool = Pool::new(C::Base::from(NODE_DOMAIN));
    let mut pb = PointsBuilder::<C>::new(&mut w, e_out, e_v, e_zero);
    let token = pb.verify_keyed_total(
        &mut pool,
        shape,
        gens,
        proof,
        claims,
        &tr,
        SharedMap {
            base: 0,
            other: None,
            r: None,
        },
        None,
        None,
    );
    // Native-field shared scalar binding is public in this isolated component
    // test; the complete backend supplies the cross-curve accumulator binding.
    let n = values.len().next_power_of_two();
    let mut shared: Vec<_> = values.iter().map(|v| C::Base::from_u128(*v)).collect();
    shared.resize(n, C::Base::ZERO);
    let shared_a = w.push(Array {
        name: "total points test shared values",
        family: f,
        rho: n.trailing_zeros() as usize,
        r_count: values.len(),
        fixed: false,
        lookups: vec![],
        widths: vec![128],
        rows: vec![],
        values: vec![shared],
    });
    w.set_export(e_v, shared_a, vec![own(0, n)]);
    pool.build(&mut w, e_out, &mut SpikeRng::new(b"total points pool"));
    let idx = pb.build(&mut w, f, shared_a, true);
    let (va, vc, vi) = token.validity(&idx);
    assert_eq!(
        w.arrays[va].values[vc][vi],
        C::Base::from(tr.accepts as u64)
    );
    let expected: Vec<_> = proof
        .rows
        .iter()
        .chain(&proof.outer)
        .chain(&proof.inner)
        .copied()
        .chain(proof.ipa.lr.iter().flat_map(|(l, r)| [*l, *r]))
        .chain([proof.ipa.g_final, proof.ipa.a])
        .collect();
    let wire = token.wire_points(&idx);
    assert_eq!(wire.len(), expected.len());
    let mut sources = vec![(va, vc, vi)];
    for (point, ports) in expected.iter().zip(wire) {
        let xy = coords(&C::from(*point));
        for ((a, k, r), want) in ports.into_iter().zip([xy.0, xy.1]) {
            assert_eq!(
                w.arrays[a].values[k][r], want,
                "exact Proof::to_bytes point order"
            );
            sources.push((a, k, r));
        }
    }
    sources.extend((0..values.len()).map(|i| (shared_a, 0, i)));
    // Public IO blocks must be aligned to their power-of-two extent. Split
    // the contiguous stream at dyadic boundaries instead of placing a large
    // block at offset four.
    let mut public_ports = Vec::with_capacity(sources.len());
    let mut used = 0;
    while used < sources.len() {
        let offset = 4 + used;
        let remaining = sources.len() - used;
        let capacity = 1usize << offset.trailing_zeros();
        let count = capacity.min(1usize << remaining.ilog2());
        let values = sources[used..used + count]
            .iter()
            .map(|(a, k, r)| w.arrays[*a].values[*k][*r])
            .collect();
        let io = w.push(Array {
            name: "total points verdict and exact proof points",
            family: f,
            rho: count.trailing_zeros() as usize,
            r_count: count,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values],
        });
        w.io_arrays.push((io, offset));
        public_ports.extend((0..count).map(|i| (io, i)));
        used += count;
    }
    w.push(Array {
        name: "total points public binding",
        family: f,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        values: vec![],
        rows: sources
            .iter()
            .zip(public_ports)
            .map(|((a, k, r), (io, i))| {
                [
                    plus(
                        &link(*a, *k, vec![Some(*r)], C::Base::ONE),
                        &link(io, 0, vec![Some(i)], -C::Base::ONE),
                    ),
                    constant(vec![C::Base::ONE]),
                    vec![],
                ]
            })
            .collect(),
    });
    let (a, p, n, t, c) = w.finish();
    let built =
        UniformCircuit::build_with_copies("total points", a, p, n, t, c, 13, 1024, b"total points");
    eprintln!(
        "total points sx={} rows={} cols={}",
        built.0.shape.sx,
        built.0.shape.rows(),
        built.0.shape.cols()
    );
    built
}
fn run<C, D>()
where
    C: PastaCurve,
    D: PastaCurve<ScalarExt = C::Base>,
    C::Base: Scalar + PoseidonField,
    D::Base: Scalar + PoseidonField,
{
    let shape = Shape {
        label: "total points inner",
        sx: 12,
        row_bits: 2,
        col_bits: 10,
        k: 3,
    };
    let gens = Gens::<C>::new(shape.cols());
    let io_len = crate::census::STATEMENT_IO + 2 * (2 + shape.col_bits) - 6;
    let (inst, witness, claims) =
        crate::verifier::instance::<C>(shape, &gens, 1, io_len, b"total points fixture");
    let proof = crate::spartan::prove_with_claims(
        &shape,
        &gens,
        &inst,
        &witness,
        &mut SpikeRng::new(b"total points inner proof"),
        &claims,
    );
    let mut expected = None;
    for case in 0..3 {
        let mut input = proof.clone();
        if case == 1 {
            input.ipa.z2 += F::<C>::ONE;
        }
        if case == 2 {
            input.rows.fill(C::identity().to_affine());
            input.outer.fill(C::identity().to_affine());
            input.inner.fill(C::identity().to_affine());
            for (l, r) in &mut input.ipa.lr {
                *l = C::identity().to_affine();
                *r = C::identity().to_affine();
            }
            input.ipa.g_final = C::identity().to_affine();
            input.ipa.a = C::identity().to_affine();
        }
        let b = build(&shape, &gens, &inst, &input, &claims);
        assert!(holds(&b), "total case {case}");
        assert_eq!(b.1.io[4], C::Base::from((case == 0) as u64));
        let fingerprint = fingerprint(&b.0);
        if let Some(expected) = expected {
            assert_eq!(expected, fingerprint, "same VK {case}");
        } else {
            expected = Some(fingerprint);
        }
        let g = Gens::<D>::new(b.0.shape.cols());
        let (p, _) = crate::spartan::prove_circuit(
            &b.0.shape,
            &g,
            &b.1,
            &b.2,
            &mut SpikeRng::new(b"total points outer proof"),
            &b.0,
        );
        assert!(crate::spartan::verify_circuit(&b.0.shape, &g, &b.1, &p.to_bytes(), &b.0).is_ok());
        let mut forged = b.1.clone();
        forged.io[4] = C::Base::ONE - forged.io[4];
        assert!(
            crate::spartan::verify_circuit(&b.0.shape, &g, &forged, &p.to_bytes(), &b.0).is_err()
        );
        if case == 2 {
            forged = b.1.clone();
            forged.io[5] = C::Base::ONE;
            assert!(
                crate::spartan::verify_circuit(&b.0.shape, &g, &forged, &p.to_bytes(), &b.0)
                    .is_err()
            );
        }
    }
}
#[test]
fn total_pallas_points_prove_true_false_and_identity_with_one_vk() {
    run::<Pallas, Vesta>();
}
#[test]
fn total_vesta_points_prove_true_false_and_identity_with_one_vk() {
    run::<Vesta, Pallas>();
}
