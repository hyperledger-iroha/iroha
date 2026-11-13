//! The lightweight preview must be identical to a real proof's first phase.
use super::*;
use crate::uniform::{UniformCircuit, rp57};

fn check<C: PastaCurve>() {
    let (arrays, pi, _) = rp57::<F<C>>(2, b"phase1 replay circuit");
    let counts = pi.iter().map(|p| 1 << p).collect();
    let (c, instance, witness) = UniformCircuit::build(
        "phase1 replay",
        arrays,
        pi,
        counts,
        Vec::new(),
        11,
        4,
        b"phase1 replay witness",
    );
    let gens = Gens::<C>::new(c.shape.cols());
    let before = c.proving_memory_accounting();
    let preview = |seed: &[u8]| {
        prepare_linked_phase1(
            &c.shape,
            &gens,
            &instance,
            &witness,
            &mut SpikeRng::new(seed),
            &c,
        )
    };
    let a = preview(b"phase1 entropy A");
    assert_eq!(a, preview(b"phase1 entropy A"));
    assert_ne!(
        a,
        preview(b"phase1 entropy B"),
        "fresh hiding masks and blinds"
    );
    assert_eq!(
        before,
        c.proving_memory_accounting(),
        "preview builds no lazy matrix plans"
    );
    let link = Link::First {
        rho: F::<C>::from(7).to_repr(),
        r: F::<C>::from(11).to_repr(),
    };
    let prove = || {
        prove_linked_circuit(
            &c.shape,
            &gens,
            &instance,
            &witness,
            &mut SpikeRng::new(b"phase1 entropy A"),
            &link,
            &c,
        )
    };
    let (proof, d, _) = prove();
    assert_eq!(
        a, d,
        "preview and full prover use the same first-phase commitments"
    );
    assert_eq!(
        proof.to_bytes(),
        prove().0.to_bytes(),
        "exact proof replay is preserved"
    );
    assert_eq!(
        verify_linked_circuit(&c.shape, &gens, &instance, &proof.to_bytes(), &link, &c,).unwrap(),
        a
    );
    let mut changed = instance.clone();
    changed.io[1] += F::<C>::ONE;
    assert_ne!(
        prepare_linked_phase1(
            &c.shape,
            &gens,
            &changed,
            &witness,
            &mut SpikeRng::new(b"phase1 entropy A"),
            &c,
        ),
        a,
        "preview binds the actual public inputs"
    );
    assert!(
        verify_linked_circuit(&c.shape, &gens, &changed, &proof.to_bytes(), &link, &c,).is_err()
    );
}

#[test]
fn phase1_preview_matches_actual_proof_on_both_curves() {
    check::<iroha_pasta::Ep>();
    check::<iroha_pasta::Eq>();
}

/// A genuine three-phase relation: a first-phase value is copied into a
/// rho-dependent array; that array binds rho and R through public link cells
/// and looks up a rho nibble in a fixed table. The copy and lookup argument
/// creates second-phase rows. Two explicit product masks make the entropy
/// consumption independently countable, including the omitted fixed tail.
fn phased_fixture<S: Scalar>(table_last: u64) -> (UniformCircuit<S>, Instance<S>, Witness<S>) {
    use crate::uniform::{Array, Copies, Export, Import, LinkPhase, constant, link, own, plus};
    let one = || constant(vec![S::ONE]);
    let io = |name| Array {
        name,
        family: 0,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: Vec::new(),
        values: vec![vec![S::ZERO]],
    };
    let arrays = vec![
        Array {
            name: "phase1 copy source",
            family: 0,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![4],
            rows: vec![[own(0, 1), one(), constant(vec![S::from(7)])]],
            values: vec![vec![S::from(7)]],
        },
        Array {
            name: "rho-bound copy and lookup",
            family: 0,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![(0, vec![own(3, 1)])],
            widths: vec![128, 4, 129, 4],
            rows: vec![
                [own(0, 1), one(), link(3, 0, vec![Some(0)], S::ONE)],
                [plus(&own(0, 1), &own(1, 1)), one(), own(2, 1)],
                [own(2, 1), one(), link(4, 0, vec![Some(0)], S::ONE)],
            ],
            values: vec![
                vec![S::ZERO],
                vec![S::from(7)],
                vec![S::from(7)],
                vec![S::ZERO],
            ],
        },
        Array {
            name: "fixed nibble table",
            family: 0,
            rho: 4,
            r_count: 16,
            fixed: true,
            lookups: Vec::new(),
            widths: vec![5],
            rows: Vec::new(),
            values: vec![
                (0..16)
                    .map(|v| S::from(if v == 15 { table_last } else { v }))
                    .collect(),
            ],
        },
        io("public rho limb"),
        io("public R"),
        Array {
            name: "two explicit opening masks",
            family: 0,
            rho: 1,
            r_count: 2,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255; 3],
            rows: vec![[own(0, 2), own(1, 2), own(2, 2)]],
            values: vec![
                vec![S::from(2), S::from(3)],
                vec![S::from(5), S::from(7)],
                vec![S::from(10), S::from(21)],
            ],
        },
    ];
    let low = |word: &[u8; 32]| S::from_u128(u128::from_le_bytes(word[..16].try_into().unwrap()));
    let copies = Copies {
        product_masks: vec![(5, [0, 1, 2])],
        exports: vec![Export {
            array: 0,
            tuple: vec![own(0, 1)],
        }],
        imports: vec![Import {
            array: 1,
            tuple: vec![own(1, 1)],
            from: vec![Some((0, 0))],
        }],
        late_io: vec![(6, 1, 2, 0)],
        link: LinkPhase {
            arrays: vec![1],
            tables: vec![0],
            fill: Some(Box::new(move |rho| {
                vec![
                    (1, 0, 0, low(rho)),
                    (1, 1, 0, S::from(7)),
                    (1, 2, 0, low(rho) + S::from(7)),
                    (1, 3, 0, S::from(u64::from(rho[0] & 15))),
                ]
            })),
            io: vec![(3, 4), (4, 5)],
            io_values: Some(Box::new(move |rho, r| {
                vec![low(rho), S::from_repr(*r).unwrap()]
            })),
            binding: Some((vec![(1, 2, 0)], 255)),
        },
        ..Default::default()
    };
    UniformCircuit::build_with_copies(
        "phase1 fixed-link-lookup",
        arrays,
        vec![0],
        vec![1],
        vec![(2, 0)],
        copies,
        10,
        4,
        b"phase1 fixed-link-lookup witness",
    )
}

fn check_phased<C: PastaCurve>() {
    let (c, instance, witness) = phased_fixture::<F<C>>(15);
    let shape = c.shape;
    assert!(c.fixed_rows(&shape) > 0);
    assert!(c.link_rows(&shape) > 0);
    assert!(c.phase2_challenges() > 0);
    assert!(c.phase1_rows(&shape) + c.link_rows(&shape) < shape.rows() - c.fixed_rows(&shape));
    let gens = Gens::<C>::new(shape.cols());
    let original = witness.w.clone();
    let before = c.proving_memory_accounting();
    let seed = b"phase1 independent schedule";
    let mut rng = SpikeRng::new(seed);
    let d = prepare_linked_phase1(&shape, &gens, &instance, &witness, &mut rng, &c);
    let mut expected_rng = SpikeRng::new(seed);
    // Two independent product masks each sample a,b; fixed rows draw no blinds.
    for _ in 0..4 + shape.rows() - c.fixed_rows(&shape) {
        let _: F<C> = expected_rng.scalar();
    }
    assert_eq!(
        rng.scalar::<F<C>>(),
        expected_rng.scalar::<F<C>>(),
        "exact prefix entropy consumption"
    );
    assert_eq!(
        witness.w, original,
        "preview must not mutate the caller witness"
    );
    assert_eq!(
        before,
        c.proving_memory_accounting(),
        "preview leaves lazy plans absent"
    );

    let fallback = |_: &[u8; 32]| panic!("the actual link circuit supplies R");
    let (secondary, rho, r) = prove_linked_circuit(
        &shape,
        &gens,
        &instance,
        &witness,
        &mut SpikeRng::new(b"phase1 secondary"),
        &Link::Second { d, rlc: &fallback },
        &c,
    );
    let fixed_r = |_: &[u8; 32]| r;
    assert_eq!(
        verify_linked_circuit(
            &shape,
            &gens,
            &instance,
            &secondary.to_bytes(),
            &Link::Second { d, rlc: &fixed_r },
            &c
        )
        .unwrap(),
        rho
    );
    let mut wrong_d = d;
    wrong_d[0] ^= 1;
    assert!(
        verify_linked_circuit(
            &shape,
            &gens,
            &instance,
            &secondary.to_bytes(),
            &Link::Second {
                d: wrong_d,
                rlc: &fixed_r
            },
            &c
        )
        .is_err(),
        "swapped predecessor digest"
    );

    let link = Link::First { rho, r };
    let (mut proof, actual_d, actual_r) = prove_linked_circuit(
        &shape,
        &gens,
        &instance,
        &witness,
        &mut SpikeRng::new(seed),
        &link,
        &c,
    );
    assert_eq!((actual_d, actual_r), (d, r));
    let bytes = proof.to_bytes();
    assert_eq!(
        prove_linked_circuit(
            &shape,
            &gens,
            &instance,
            &witness,
            &mut SpikeRng::new(seed),
            &link,
            &c,
        )
        .0
        .to_bytes(),
        bytes,
        "fixed/link/lookup proof replays exactly after all plans are cached"
    );
    assert_eq!(
        verify_linked_circuit(&shape, &gens, &instance, &bytes, &link, &c).unwrap(),
        d
    );
    let mut wrong_rho = rho;
    wrong_rho[20] ^= 1; // Above the limb used by the relation: transcript binding must reject it.
    assert!(
        verify_linked_circuit(
            &shape,
            &gens,
            &instance,
            &bytes,
            &Link::First { rho: wrong_rho, r },
            &c
        )
        .is_err(),
        "whole rho word is bound"
    );
    let wrong_r = (F::<C>::from_repr(r).unwrap() + F::<C>::ONE).to_repr();
    assert!(
        verify_linked_circuit(
            &shape,
            &gens,
            &instance,
            &bytes,
            &Link::First { rho, r: wrong_r },
            &c
        )
        .is_err(),
        "different R is bound"
    );

    let (other_key, _, _) = phased_fixture::<F<C>>(16);
    assert_eq!(other_key.layout(), c.layout());
    assert_ne!(
        fixed_for(&shape, &gens, &other_key, &instance.io),
        fixed_for(&shape, &gens, &c, &instance.io)
    );
    assert!(
        verify_linked_circuit(&shape, &gens, &instance, &bytes, &link, &other_key).is_err(),
        "a changed omitted fixed tail cannot verify the old proof"
    );
    assert_eq!(proof.late.len(), 1);
    proof.late[0] += F::<C>::ONE;
    assert!(
        verify_linked_circuit(&shape, &gens, &instance, &proof.to_bytes(), &link, &c).is_err(),
        "late values remain transcript-bound after the preview"
    );
}

#[test]
fn phase1_preview_preserves_fixed_link_and_lookup_schedule_on_both_curves() {
    check_phased::<iroha_pasta::Ep>();
    check_phased::<iroha_pasta::Eq>();
}
