use super::*;
use crate::{
    full::{NODE_DOMAIN, Pool, Wiring, constant_entries},
    points::PointsBuilder,
    r1cs::{Circuit, Instance, Witness},
    uniform::{Array, UniformCircuit, constant, link, own, plus},
};
use iroha_pasta::{Ep as Pallas, Eq as Vesta};

fn sources<C: PastaCurve>(gens: &Gens<C>, k: usize) -> Vec<Claim<C>> {
    (0..SOURCE_COUNT)
        .map(|j| {
            let us = (0..k)
                .map(|i| F::<C>::from((3 + i + j * 7) as u64))
                .collect::<Vec<_>>();
            let g = msm_public::<C>(&s_vector(&us), &gens.g[..1 << k], MemoryBudget::DEFAULT)
                .unwrap()
                .to_affine();
            Claim { us, g }
        })
        .collect()
}
fn native<C: PastaCurve>()
where
    F<C>: Scalar,
{
    let gens = Gens::<C>::new(16);
    let relation = [23; 32];
    let input = sources(&gens, 4);
    let (a, ta) = prove_with_rng(
        &gens,
        relation,
        input.clone(),
        &mut SpikeRng::new(b"generator private mask A"),
    );
    let (b, _) = prove_with_rng(
        &gens,
        relation,
        input,
        &mut SpikeRng::new(b"generator private mask B"),
    );
    assert!(verify(&gens, relation, &a));
    assert!(verify(&gens, relation, &b));
    assert_ne!(
        a.output.g, b.output.g,
        "fresh mask hides predecessor fingerprint"
    );
    assert_ne!(a.output.us, b.output.us);
    assert_eq!(ta.shared.len(), ta.layout.len);
    let mut wrong_relation = relation;
    wrong_relation[0] ^= 1;
    assert!(!verify(&gens, wrong_relation, &a));
    for case in 0..6 {
        let mut bad = a.clone();
        match case {
            0 => bad.sources[0].g = (C::from(bad.sources[0].g) + C::from(gens.h)).to_affine(),
            1 => bad.sources[1].us[0] += F::<C>::ONE,
            2 => bad.masks[1] += F::<C>::ONE,
            3 => bad.c += F::<C>::ONE,
            4 => bad.output.us[0] += F::<C>::ONE,
            _ => bad.output.g = (C::from(bad.output.g) + C::from(gens.h)).to_affine(),
        };
        assert!(!verify(&gens, relation, &bad), "tamper {case}");
    }
}
#[test]
fn exact_private_generator_fold_native_both_curves() {
    native::<Pallas>();
    native::<Vesta>();
}
fn holds<S: Scalar + PoseidonField>(b: &(UniformCircuit<S>, Instance<S>, Witness<S>)) -> bool {
    let mut v = b.2.w.clone();
    let rel =
        b.0.instantiate(&b.0.shape, &[S::from(31), S::from(47)], Some(&mut v));
    let z = crate::r1cs::z_vector(&b.0.shape, &b.1, &Witness { w: v });
    let (a, b, c) = rel.abc(&b.0.shape, &z);
    a.into_iter().zip(b).zip(c).all(|((a, b), c)| a * b == c)
}
fn shared_array<S: Scalar>(w: &mut Wiring<S>, family: usize, values: &[u128]) -> usize {
    let n = values.len().next_power_of_two();
    let mut v = values.iter().map(|v| S::from_u128(*v)).collect::<Vec<_>>();
    v.resize(n, S::ZERO);
    w.push(Array {
        name: "generator test shared",
        family,
        rho: n.ilog2() as usize,
        r_count: values.len(),
        fixed: false,
        lookups: vec![],
        widths: vec![128],
        rows: vec![],
        values: vec![v],
    })
}
fn public<S: Scalar>(w: &mut Wiring<S>, family: usize, sources: Vec<(usize, usize, usize)>) {
    let mut used = 0;
    while used < sources.len() {
        let offset = 4 + used;
        let count =
            (1usize << offset.trailing_zeros()).min(1usize << (sources.len() - used).ilog2());
        let ports = &sources[used..used + count];
        let vals = ports
            .iter()
            .map(|(a, c, r)| w.arrays[*a].values[*c][*r])
            .collect();
        let io = w.push(Array {
            name: "generator public component binding",
            family,
            rho: count.ilog2() as usize,
            r_count: count,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![vals],
        });
        w.io_arrays.push((io, offset));
        w.push(Array {
            name: "generator public component links",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![],
            values: vec![],
            rows: ports
                .iter()
                .enumerate()
                .map(|(i, (a, c, r))| {
                    [
                        plus(
                            &link(*a, *c, vec![Some(*r)], S::ONE),
                            &link(io, 0, vec![Some(i)], -S::ONE),
                        ),
                        constant(vec![S::ONE]),
                        vec![],
                    ]
                })
                .collect(),
        });
        used += count;
    }
}
fn scalar<C: PastaCurve>()
where
    F<C>: Scalar + PoseidonField,
{
    let gens = Gens::<C>::new(16);
    let (fold, tr) = prove_with_rng(
        &gens,
        [23; 32],
        sources(&gens, 4),
        &mut SpikeRng::new(b"generator scalar circuit"),
    );
    let mut w = Wiring::<F<C>>::default();
    let f = w.family(0, 1);
    let shared = shared_array(&mut w, f, &tr.shared);
    let ports = append_scalar(&mut w, f, shared, 0, &fold, &tr);
    assert_eq!(ports.sources.len(), 4);
    let mut public_sources = (0..tr.shared.len())
        .map(|i| (shared, 0, i))
        .collect::<Vec<_>>();
    public_sources.extend(ports.output);
    public(&mut w, f, public_sources);
    let (a, p, n, t, c) = w.finish();
    let built = UniformCircuit::build_with_copies(
        "generator scalar",
        a,
        p,
        n,
        t,
        c,
        12,
        1024,
        b"generator scalar circuit",
    );
    assert!(holds(&built));
    // A shared gamma chosen independently of its exact mask recurrence must fail.
    let mut bad = tr.clone();
    bad.shared[tr.layout.gamma] ^= 1;
    let mut w = Wiring::<F<C>>::default();
    let f = w.family(0, 1);
    let shared = shared_array(&mut w, f, &bad.shared);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| append_scalar(
            &mut w, f, shared, 0, &fold, &bad
        )))
        .is_err()
    );
}
#[test]
fn exact_private_generator_fold_scalar_both_curves() {
    scalar::<Pallas>();
    scalar::<Vesta>();
}
fn points<C, D>()
where
    C: PastaCurve,
    D: PastaCurve<ScalarExt = C::Base>,
    C::Base: Scalar + PoseidonField,
    F<C>: Scalar,
    D::Base: Scalar + PoseidonField,
{
    let gens = Gens::<C>::new(8);
    let (fold, tr) = prove_with_rng(
        &gens,
        [23; 32],
        sources(&gens, 3),
        &mut SpikeRng::new(b"generator points circuit"),
    );
    let mut fingerprint = None;
    for case in 0..4 {
        let original = match case {
            0 => fold.sources[0].g,
            1 => (C::from(fold.sources[0].g) + C::from(gens.h)).to_affine(),
            2 => C::identity().to_affine(),
            _ => fold.sources[0].g,
        };
        let mut tr = tr.clone();
        if case == 3 {
            // Same exact point under the field join, different integer limbs:
            // without canonicality this could falsify a limb-wise inherited
            // claim comparison and burn an otherwise valid incoming payment.
            let (x, _): (C::Base, C::Base) = Option::from(fold.sources[0].g.coordinates()).unwrap();
            let alias = num_bigint::BigUint::from_bytes_le(&x.to_repr())
                + num_bigint::BigUint::from_bytes_le(&(-C::Base::ONE).to_repr())
                + 1u32;
            let mut bytes = alias.to_bytes_le();
            bytes.resize(32, 0);
            let at = tr.layout.source(0);
            tr.shared[at] = u128::from_le_bytes(bytes[..16].try_into().unwrap());
            tr.shared[at + 1] = u128::from_le_bytes(bytes[16..].try_into().unwrap());
        }
        let mut values = tr.shared.clone();
        push_point::<C>(&mut values, original);
        let mut w = Wiring::<C::Base>::default();
        let f = w.family(0, 1);
        let e_out = w.reserve();
        let e_v = w.reserve();
        let e_zero = constant_entries(&mut w, f, "zero", &[vec![C::Base::ZERO]]);
        let mut pool = Pool::new(C::Base::from(NODE_DOMAIN));
        let mut pb = PointsBuilder::<C>::new(&mut w, e_out, e_v, e_zero);
        let token = pb.generator_fold(&mut pool, &gens, &fold, &tr, 0);
        let equal = pb.generator_source_equals_inherited(
            &token,
            0,
            original,
            std::array::from_fn(|i| tr.shared.len() + i),
        );
        let shared = shared_array(&mut w, f, &values);
        let n = values.len().next_power_of_two();
        w.set_export(e_v, shared, vec![own(0, n)]);
        pool.build(&mut w, e_out, &mut SpikeRng::new(b"generator points pool"));
        let idx = pb.build(&mut w, f, shared, true);
        let verdict = equal.validity(&idx);
        assert_eq!(
            w.arrays[verdict.0].values[verdict.1][verdict.2],
            C::Base::from((case == 0 || case == 3) as u64)
        );
        let mut public_sources = vec![verdict];
        public_sources.extend((0..values.len()).map(|i| (shared, 0, i)));
        public(&mut w, f, public_sources);
        let (a, p, n, t, c) = w.finish();
        let built = UniformCircuit::build_with_copies(
            "generator points",
            a,
            p,
            n,
            t,
            c,
            13,
            1024,
            b"generator points circuit",
        );
        if case == 3 {
            assert!(
                !holds(&built),
                "x+p coordinate aliases must be unsatisfiable"
            );
            continue;
        }
        assert!(holds(&built));
        let mut h = blake3::Hasher::new();
        for a in 0..built.0.arrays.len() {
            for (m, r, k, v) in built
                .0
                .entries_of(&[C::Base::from(31), C::Base::from(47)], Some(a))
            {
                for i in [m, r, k] {
                    h.update(&(i as u64).to_le_bytes());
                }
                h.update(v.to_repr().as_ref());
            }
        }
        let hash = *h.finalize().as_bytes();
        if let Some(want) = fingerprint {
            assert_eq!(hash, want, "same VK for equality/inequality/identity");
        } else {
            fingerprint = Some(hash);
        }
        let outer = Gens::<D>::new(built.0.shape.cols());
        let start = std::time::Instant::now();
        eprintln!("private generator fold proof case {case}: shape={:?}, memory={:?}", built.0.shape, built.0.proving_memory_accounting());
        let (proof, _) = crate::spartan::prove_circuit(
            &built.0.shape,
            &outer,
            &built.1,
            &built.2,
            &mut SpikeRng::new(b"generator points outer proof"),
            &built.0,
        );
        assert!(
            crate::spartan::verify_circuit(
                &built.0.shape,
                &outer,
                &built.1,
                &proof.to_bytes(),
                &built.0
            )
            .is_ok()
        );
        eprintln!("private generator fold proof case {case} verified in {:?}", start.elapsed());
        let mut forged = built.1.clone();
        forged.io[4] = C::Base::ONE - forged.io[4];
        assert!(
            crate::spartan::verify_circuit(
                &built.0.shape,
                &outer,
                &forged,
                &proof.to_bytes(),
                &built.0
            )
            .is_err()
        );
    }
}
#[test]
fn exact_private_generator_fold_points_pallas() {
    points::<Pallas, Vesta>();
}
#[test]
fn exact_private_generator_fold_points_vesta() {
    points::<Vesta, Pallas>();
}
