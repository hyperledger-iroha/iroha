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
    let mut pb = PointsBuilder::<C>::new(&mut w, e_out, e_v, e_zero, IdentityPoints::Native);
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

/// Whether a built circuit's witness holds at fixed second-phase challenges.
fn holds_any<S: Scalar>((c, i, w): &(UniformCircuit<S>, Instance<S>, Witness<S>)) -> bool {
    let mut v = w.w.clone();
    let ch: Vec<S> = (0..c.phase2_challenges() as u64)
        .map(|k| S::from(31 + 16 * k))
        .collect();
    let r = c.instantiate(&c.shape, &ch, Some(&mut v));
    let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: v });
    let (a, b, c) = r.abc(&c.shape, &z);
    a.into_iter().zip(b).zip(c).all(|((a, b), c)| a * b == c)
}

/// Review finding 7 (identity): one identity rule on both sides. A proof
/// point may be the identity (32 zero bytes, absorbed as (0, 0)) except the
/// Schnorr commitment A. An all-identity proof of the zero io (rows, rounds,
/// L and R the identity; v = z1 = 0; A = [z2] H; G the folded generator) is
/// valid natively, and the total verifier derives verdict 1 for it; with A the
/// identity both refuse.
fn identity_rule<C, D>()
where
    C: PastaCurve,
    D: PastaCurve<ScalarExt = C::Base>,
    C::Base: Scalar + PoseidonField,
    D::Base: Scalar + PoseidonField,
{
    let shape = Shape {
        label: "identity points",
        sx: 12,
        row_bits: 2,
        col_bits: 10,
        k: 3,
    };
    let gens = Gens::<C>::new(shape.cols());
    let inst = Instance {
        io: vec![F::<C>::ZERO; 8],
    };
    let mut proof = identity_proof(&shape, &gens, &inst);
    let id = C::identity().to_affine();
    let bytes = proof.to_bytes();
    assert!(
        bytes[..32].iter().all(|b| *b == 0),
        "the identity encodes as zero bytes"
    );
    assert_eq!(
        Proof::<C>::from_bytes(&shape, &bytes).map(|p| p.to_bytes()),
        Some(bytes.clone()),
        "and decodes back"
    );
    assert_eq!(
        crate::spartan::verify(&shape, &gens, &inst, &bytes),
        Ok(()),
        "native verification accepts identity points other than A"
    );
    let built = build(&shape, &gens, &inst, &proof, &[]);
    assert!(holds(&built));
    assert_eq!(
        built.1.io[4],
        C::Base::ONE,
        "the total verifier accepts the same proof"
    );
    let (c, mut i, w) = built;
    i.io[4] = C::Base::ZERO;
    assert!(!holds(&(c, i, w)), "verdict 0 cannot be claimed for it");
    proof.ipa.a = id;
    assert!(
        crate::spartan::verify(&shape, &gens, &inst, &proof.to_bytes()).is_err(),
        "native verification refuses an identity A"
    );
    let built = build(&shape, &gens, &inst, &proof, &[]);
    assert!(holds(&built));
    assert_eq!(built.1.io[4], C::Base::ZERO, "so does the total verifier");
}

/// A valid proof of the zero io whose every committed point is the
/// identity: rows, rounds, L and R the identity; v = z1 = 0; A = [z2] H; G
/// the folded generator. Native verification accepts it.
fn identity_proof<C: PastaCurve>(shape: &Shape, gens: &Gens<C>, inst: &Instance<F<C>>) -> Proof<C>
where
    C::Base: Scalar,
{
    use crate::ipa::IpaProof;
    let id = C::identity().to_affine();
    let z2 = F::<C>::from(0x5eed_u64);
    let mut proof = Proof::<C> {
        rows: vec![id; shape.rows()],
        late: Vec::new(),
        outer: vec![id; shape.outer_blocks().len()],
        v: [F::<C>::ZERO; 3],
        inner: vec![id; shape.inner_blocks().len()],
        ipa: IpaProof {
            lr: vec![(id, id); shape.col_bits],
            g_final: id,
            a: (C::from(gens.h) * z2).to_affine(),
            z1: F::<C>::ZERO,
            z2,
        },
    };
    let tr = crate::verifier::trace(shape, gens, inst, &proof, &[], &crate::spartan::Link::None);
    proof.ipa.g_final = iroha_pasta::msm::msm_public::<C>(
        &crate::ipa::s_vector(&tr.us),
        &gens.g,
        iroha_pasta::msm::MemoryBudget::DEFAULT,
    )
    .unwrap()
    .to_affine();
    proof
}
#[test]
fn identity_points_follow_one_rule_natively_and_in_the_total_verifier() {
    identity_rule::<Pallas, Vesta>();
    identity_rule::<Vesta, Pallas>();
}

/// Review finding 7 (limbs): the full-width values that cross between the
/// fields as two 128-bit shared limbs. Natively they are canonical encodings
/// (the proof decoder refuses `value + modulus`); in-circuit
/// `canonical_limbs` holds exactly for `lo + 2^128 hi` below the modulus,
/// for both scalar moduli in both circuit fields.
fn canonical_limb_cases<S: Scalar + PoseidonField>() {
    for m in [modulus::<Fp>(), modulus::<Fq>()] {
        let wide = num_bigint::BigUint::from(1u32) << 256;
        let mask = (num_bigint::BigUint::from(1u32) << 128) - 1u32;
        for (x, canonical) in [
            (num_bigint::BigUint::from(0u32), true),
            (num_bigint::BigUint::from(1u32), true),
            (&m - 1u32, true),
            (m.clone(), false),
            (&m + 1u32, false),
            (&m + &m - 1u32, false),
            (&wide - 1u32, false),
        ] {
            let mut w = Wiring::<S>::default();
            let f = w.family(0, 1);
            let shared = w.push(Array {
                name: "canonical limbs test shared values",
                family: f,
                rho: 1,
                r_count: 2,
                fixed: false,
                lookups: vec![],
                widths: vec![128],
                rows: vec![],
                values: vec![vec![big::<S>(&(&x & &mask)), big::<S>(&(&x >> 128))]],
            });
            let t = w.push(Array {
                name: "range table",
                family: f,
                rho: CH,
                r_count: 1 << CH,
                fixed: true,
                lookups: Vec::new(),
                widths: vec![CH as u16],
                rows: Vec::new(),
                values: vec![(0..1u64 << CH).map(S::from).collect()],
            });
            w.tables.push((t, crate::nonnative::RANGE_TABLE));
            canonical_limbs(&mut w, f, shared, &[(0, m.clone())]);
            let (a, p, n, t, c) = w.finish();
            let built = UniformCircuit::build_with_copies(
                "canonical limbs",
                a,
                p,
                n,
                t,
                c,
                11,
                4,
                b"canonical limbs",
            );
            assert_eq!(holds(&built), canonical, "limbs of {x} below {m}");
        }
    }
}
#[test]
fn full_width_limbs_are_canonical_natively_and_in_circuit() {
    canonical_limb_cases::<Fp>();
    canonical_limb_cases::<Fq>();
    // Native: the limbs are the canonical encodings and a scalar encoded as
    // value + modulus does not decode.
    let shape = Shape {
        label: "canonical limbs native",
        sx: 12,
        row_bits: 2,
        col_bits: 10,
        k: 3,
    };
    let gens = Gens::<Vesta>::new(shape.cols());
    let io_len = crate::census::STATEMENT_IO + 2 * (2 + shape.col_bits) - 6;
    let (inst, witness, claims) =
        crate::verifier::instance::<Vesta>(shape, &gens, 1, io_len, b"canonical limbs native");
    let proof = crate::spartan::prove_with_claims(
        &shape,
        &gens,
        &inst,
        &witness,
        &mut SpikeRng::new(b"canonical limbs native proof"),
        &claims,
    );
    let tr = crate::verifier::trace(
        &shape,
        &gens,
        &inst,
        &proof,
        &claims,
        &crate::spartan::Link::None,
    );
    let values = crate::verifier::shared_values(&tr, &proof, &claims);
    let full_base = tr
        .challenges
        .iter()
        .map(|(_, k, _)| if *k == Kind::Full { 2 } else { 1 })
        .sum::<usize>();
    let sh = shared_scalars(&tr, &proof);
    let wanted = [
        sh.io_digest,
        proof.v[0],
        proof.v[1],
        proof.v[2],
        tr.kappa_claims,
        sh.kappa_zeta,
        sh.c_z1,
        sh.z1_bf_zeta,
        proof.ipa.z2,
    ];
    for (i, v) in wanted.iter().enumerate() {
        let x = num_bigint::BigUint::from(values[full_base + 2 * i])
            + (num_bigint::BigUint::from(values[full_base + 2 * i + 1]) << 128);
        assert_eq!(
            x,
            to_big(*v),
            "full-width value {i} is its canonical encoding"
        );
        assert!(x < modulus::<F<Vesta>>());
    }
    let bytes = proof.to_bytes();
    let at = 32 * (proof.rows.len() + proof.outer.len());
    let aliased = to_big(proof.v[0]) + modulus::<F<Vesta>>();
    if aliased.bits() <= 256 {
        let mut raw = aliased.to_bytes_le();
        raw.resize(32, 0);
        let mut bad = bytes.clone();
        bad[at..at + 32].copy_from_slice(&raw);
        assert!(
            Proof::<Vesta>::from_bytes(&shape, &bad).is_none(),
            "native decoding refuses v_A + modulus"
        );
    }
    assert!(Proof::<Vesta>::from_bytes(&shape, &bytes).is_some());
}

/// Review finding 9: the CancelRound and AdaptiveWitness forgeries, as
/// verified proofs inside the uniform builders the complete circuits use.
/// Native verification refuses each. The scalar side (its slot weights) holds
/// and reproduces the native kappa', b_final and Schnorr scalars; the points
/// side (its gamma-Horner over the round commitments) reproduces the native
/// combined commitment. The total verifier then derives verdict 0, and verdict
/// 1 (the hard verifier's equality) has no satisfying witness; the hard
/// builder refuses outright.
fn forgeries_refused<C, D>()
where
    C: PastaCurve,
    D: PastaCurve<ScalarExt = C::Base>,
    C::Base: Scalar + PoseidonField,
    D::Base: Scalar + PoseidonField,
    F<C>: Scalar + PoseidonField,
{
    let shape = Shape {
        label: "uniform forgery refusal",
        sx: 12,
        row_bits: 2,
        col_bits: 10,
        k: 3,
    };
    let gens = Gens::<C>::new(shape.cols());
    let mut rng = SpikeRng::new(b"uniform forgery refusal");
    let (inst, wit) = crate::r1cs::sample::<F<C>>(&shape, &mut rng);
    let mut bad = wit.w.clone();
    bad[(3 * shape.rows() / 4) * shape.cols()] = F::<C>::from(2);
    let bad = Witness { w: bad };
    assert!(!crate::spartan::relation_holds::<C>(&shape, &inst, &bad));
    for cheat in [
        crate::spartan::Cheat::CancelRound,
        crate::spartan::Cheat::AdaptiveWitness,
    ] {
        let (forged, _) = crate::spartan::prove_with(&shape, &gens, &inst, &bad, &mut rng, cheat);
        assert!(
            crate::spartan::verify(&shape, &gens, &inst, &forged.to_bytes()).is_err(),
            "{cheat:?}: native verification refuses"
        );
        let tr = crate::verifier::trace(
            &shape,
            &gens,
            &inst,
            &forged,
            &[],
            &crate::spartan::Link::None,
        );
        assert!(!tr.accepts, "{cheat:?}: the native trace refuses");
        // The scalar side: holds, with the native values asserted at build.
        let side = crate::scalar::scalar_side::<C>(
            0,
            0,
            &shape,
            lean_schedule(),
            &inst,
            &forged,
            &[],
            &tr,
        );
        let (sc, si, sw) = UniformCircuit::build(
            "uniform forgery scalar side",
            side.arrays,
            vec![0],
            vec![1],
            Vec::new(),
            12,
            4,
            b"uniform forgery scalar side",
        );
        assert!(holds_any(&(sc, si, sw)), "{cheat:?}: the scalar side holds");
        // The total points side: verdict 0; verdict 1 is unsatisfiable.
        let built = build(&shape, &gens, &inst, &forged, &[]);
        assert!(holds(&built), "{cheat:?}: total verification holds");
        assert_eq!(built.1.io[4], C::Base::ZERO, "{cheat:?}: verdict 0");
        let (c, mut i, w) = built;
        i.io[4] = C::Base::ONE;
        assert!(
            !holds(&(c, i, w)),
            "{cheat:?}: acceptance cannot be claimed"
        );
        // The hard points side refuses to build a verification of it.
        let hard = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut w = Wiring::<C::Base>::default();
            let f = w.family(0, 1);
            let e_out = w.reserve();
            let e_v = w.reserve();
            let e_zero = constant_entries(&mut w, f, "zero", &[vec![C::Base::ZERO]]);
            let mut pool = Pool::new(C::Base::from(NODE_DOMAIN));
            let mut pb = PointsBuilder::<C>::new(
                &mut w,
                e_out,
                e_v,
                e_zero,
                crate::full::OPERATION_IDENTITY,
            );
            pb.verify_keyed(
                &mut pool,
                &shape,
                &gens,
                &forged,
                &[],
                &tr,
                SharedMap {
                    base: 0,
                    other: None,
                    r: None,
                },
                None,
                None,
            );
        }));
        let why = hard.expect_err("the hard verifier has no witness");
        let why = why.downcast_ref::<String>().cloned().unwrap_or_default();
        assert!(
            why.contains("the final Schnorr check"),
            "{cheat:?}: refused at the final Schnorr equality, not elsewhere: {why}"
        );
    }
}
#[test]
fn uniform_builders_refuse_cancel_round_and_adaptive_witness() {
    forgeries_refused::<Pallas, Vesta>();
    forgeries_refused::<Vesta, Pallas>();
}

/// One proof's hard points side in a builder with identity rule `identity`,
/// the proof's shared values a free array: (circuit, instance, witness).
fn hard_build<C: PastaCurve>(
    shape: &Shape,
    gens: &Gens<C>,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
    identity: IdentityPoints,
) -> (UniformCircuit<C::Base>, Instance<C::Base>, Witness<C::Base>)
where
    C::Base: Scalar + PoseidonField,
{
    let tr = crate::verifier::trace(shape, gens, inst, proof, &[], &crate::spartan::Link::None);
    let values = crate::verifier::shared_values(&tr, proof, &[]);
    let mut w = Wiring::<C::Base>::default();
    let f = w.family(0, 1);
    let e_out = w.reserve();
    let e_v = w.reserve();
    let e_zero = constant_entries(&mut w, f, "zero", &[vec![C::Base::ZERO]]);
    let mut pool = Pool::new(C::Base::from(NODE_DOMAIN));
    let mut pb = PointsBuilder::<C>::new(&mut w, e_out, e_v, e_zero, identity);
    pb.verify_keyed(
        &mut pool,
        shape,
        gens,
        proof,
        &[],
        &tr,
        SharedMap {
            base: 0,
            other: None,
            r: None,
        },
        None,
        None,
    );
    let n = values.len().next_power_of_two();
    let mut shared: Vec<_> = values.iter().map(|v| C::Base::from_u128(*v)).collect();
    shared.resize(n, C::Base::ZERO);
    let shared_a = w.push(Array {
        name: "hard points test shared values",
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
    pool.build(&mut w, e_out, &mut SpikeRng::new(b"hard points pool"));
    let _ = pb.build(&mut w, f, shared_a, true);
    let (a, p, n, t, c) = w.finish();
    UniformCircuit::build_with_copies("hard points", a, p, n, t, c, 13, 4, b"hard points")
}

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

/// Review re-check R4: identity handling is each builder's explicit rule,
/// not a side effect of a generator fold or of a total slot elsewhere in the
/// circuit. A hard slot alone (no total slot, no generator fold) in a builder
/// that chose the native rule verifies the valid all-identity proof, as
/// native verification does, and still refuses an identity A. The same hard
/// slot under hard-slot strictness refuses the proof, and a strict builder
/// cannot place a total slot. The monetary backend chooses the native rule.
fn explicit_identity<C>()
where
    C: PastaCurve,
    C::Base: Scalar + PoseidonField,
{
    assert_eq!(crate::full::OPERATION_IDENTITY, IdentityPoints::Native);
    assert_eq!(crate::full::BENCHMARK_IDENTITY, IdentityPoints::Refused);
    let shape = Shape {
        label: "explicit identity rule",
        sx: 12,
        row_bits: 2,
        col_bits: 10,
        k: 3,
    };
    let gens = Gens::<C>::new(shape.cols());
    let inst = Instance {
        io: vec![F::<C>::ZERO; 8],
    };
    let mut proof = identity_proof(&shape, &gens, &inst);
    assert_eq!(
        crate::spartan::verify(&shape, &gens, &inst, &proof.to_bytes()),
        Ok(())
    );
    assert!(
        holds_any(&hard_build(
            &shape,
            &gens,
            &inst,
            &proof,
            IdentityPoints::Native
        )),
        "a native-rule hard slot verifies it without any generator fold"
    );
    let strict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        hard_build(&shape, &gens, &inst, &proof, IdentityPoints::Refused)
    }));
    let why = panic_text(strict.err().expect("hard-slot strictness refuses it"));
    assert!(why.contains("IdentityPoints::Refused"), "{why}");
    let total = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let tr = crate::verifier::trace(
            &shape,
            &gens,
            &inst,
            &proof,
            &[],
            &crate::spartan::Link::None,
        );
        let mut w = Wiring::<C::Base>::default();
        let f = w.family(0, 1);
        let e_out = w.reserve();
        let e_v = w.reserve();
        let e_zero = constant_entries(&mut w, f, "zero", &[vec![C::Base::ZERO]]);
        let mut pool = Pool::new(C::Base::from(NODE_DOMAIN));
        let mut pb = PointsBuilder::<C>::new(&mut w, e_out, e_v, e_zero, IdentityPoints::Refused);
        pb.verify_keyed_total(
            &mut pool,
            &shape,
            &gens,
            &proof,
            &[],
            &tr,
            SharedMap {
                base: 0,
                other: None,
                r: None,
            },
            None,
            None,
        );
    }));
    let why = panic_text(total.err().expect("a strict builder has no total slot"));
    assert!(
        why.contains("a total slot follows the native identity rule"),
        "{why}"
    );
    proof.ipa.a = C::identity().to_affine();
    let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        hard_build(&shape, &gens, &inst, &proof, IdentityPoints::Native)
    }));
    let why = panic_text(refused.err().expect("an identity A is refused"));
    assert!(why.contains("Schnorr A must be finite"), "{why}");
}
#[test]
fn identity_rule_is_each_builders_explicit_choice() {
    explicit_identity::<Pallas>();
    explicit_identity::<Vesta>();
}
