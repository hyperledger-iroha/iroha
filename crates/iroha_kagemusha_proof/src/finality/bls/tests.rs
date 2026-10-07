//! Complete traces from the real normal-BLS signer, with independent Ark oracle.
use super::*;
use ark_bls12_381::{Bls12_381, Fq, Fq2, Fq6, Fq12, G1Affine, G2Affine, g1, g2};
use ark_ec::{
    AffineRepr, CurveGroup,
    hashing::{
        curve_maps::{swu::SWUMap, wb::WBConfig},
        map_to_curve_hasher::MapToCurve,
    },
    pairing::Pairing,
};
use ark_ff::{
    BigInt, BigInteger, Field as ArkField, PrimeField as ArkPrimeField,
    fields::field_hashers::{DefaultFieldHasher, HashToField},
};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};
use iroha_plonk_gadgets::bls12_381::{
    curve::{
        g1_program::{G1_SUBGROUP_STEPS, G1Step},
        programs::{G2_COFACTOR_STEPS, G2_SUBGROUP_STEPS, G2Step},
    },
    extension::Fp2,
    hash_to_field::W3F_SIGNING_PREFIX,
    pairing::{
        MillerG2Witness,
        final_exponent::{FINAL_EXPONENT_STEPS, FinalExponentStep},
        miller_program::{MILLER_STEPS, MillerStep},
    },
};
use sha2::Sha256;
type Iso = <g2::Config as WBConfig>::IsogenousCurve;
fn fq(a: native::Fp) -> Fq {
    Fq::from_bigint(BigInt(a)).unwrap()
}
fn fq2(a: Fp2) -> Fq2 {
    Fq2::new(fq(a[0]), fq(a[1]))
}
fn fp2(a: Fq2) -> Fp2 {
    [a.c0.into_bigint().0, a.c1.into_bigint().0]
}
fn fp12(a: &Fq12) -> Fp12 {
    [
        [fp2(a.c0.c0), fp2(a.c0.c1), fp2(a.c0.c2)],
        [fp2(a.c1.c0), fp2(a.c1.c1), fp2(a.c1.c2)],
    ]
}
fn fq12(a: &Fp12) -> Fq12 {
    Fq12::new(
        Fq6::new(fq2(a[0][0]), fq2(a[0][1]), fq2(a[0][2])),
        Fq6::new(fq2(a[1][0]), fq2(a[1][1]), fq2(a[1][2])),
    )
}
fn w1(p: G1Affine) -> G1AffineWitness {
    if p.infinity {
        G1AffineWitness {
            x: native::ZERO,
            y: native::ZERO,
            infinity: true,
        }
    } else {
        G1AffineWitness {
            x: p.x.into_bigint().0,
            y: p.y.into_bigint().0,
            infinity: false,
        }
    }
}
fn w2(p: G2Affine) -> G2AffineWitness {
    if p.infinity {
        G2AffineWitness {
            x: [native::ZERO; 2],
            y: [native::ZERO; 2],
            infinity: true,
        }
    } else {
        G2AffineWitness {
            x: fp2(p.x),
            y: fp2(p.y),
            infinity: false,
        }
    }
}
fn p1(p: G1AffineWitness) -> G1Affine {
    if p.infinity {
        G1Affine::identity()
    } else {
        G1Affine::new_unchecked(fq(p.x), fq(p.y))
    }
}
fn p2(p: G2AffineWitness) -> G2Affine {
    if p.infinity {
        G2Affine::identity()
    } else {
        G2Affine::new_unchecked(fq2(p.x), fq2(p.y))
    }
}
fn psi(p: G2Affine) -> G2Affine {
    if p.infinity {
        return p;
    }
    let mut exponent = Fq::MODULUS;
    exponent.sub_with_borrow(&1_u64.into());
    let mut third = [0; 6];
    let mut carry = 0_u128;
    for i in (0..6).rev() {
        let n = (carry << 64) + u128::from(exponent.0[i]);
        third[i] = u64::try_from(n / 3).unwrap();
        carry = n % 3;
    }
    exponent.div2();
    let xi = Fq2::new(Fq::ONE, Fq::ONE);
    G2Affine::new_unchecked(
        p.x.frobenius_map(1) * xi.pow(third).inverse().unwrap(),
        p.y.frobenius_map(1) * xi.pow(exponent).inverse().unwrap(),
    )
}
fn g1_step(step: G1Step, r: &mut [G1Affine; 3]) {
    match step {
        G1Step::Copy {
            destination,
            source,
        } => r[destination] = r[source],
        G1Step::Double {
            destination,
            source,
        } => r[destination] = (r[source] + r[source]).into_affine(),
        G1Step::Add {
            destination,
            left,
            right,
        } => r[destination] = (r[left] + r[right]).into_affine(),
        G1Step::Negate {
            destination,
            source,
        } => r[destination] = -r[source],
        G1Step::Phi {
            destination,
            source,
        } => r[destination] = g1::endomorphism(&r[source]),
        G1Step::RejectNonidentityFixedPoint => assert!(r[0] != r[1] || r[0].infinity),
    }
}
fn g2_step(step: G2Step, r: &mut [G2Affine; 6]) {
    match step {
        G2Step::Copy {
            destination,
            source,
        } => r[destination] = r[source],
        G2Step::Double {
            destination,
            source,
        } => r[destination] = (r[source] + r[source]).into_affine(),
        G2Step::Add {
            destination,
            left,
            right,
        } => r[destination] = (r[left] + r[right]).into_affine(),
        G2Step::Negate {
            destination,
            source,
        } => r[destination] = -r[source],
        G2Step::Psi {
            destination,
            source,
        } => r[destination] = psi(r[source]),
        G2Step::Psi2 {
            destination,
            source,
        } => r[destination] = psi(psi(r[source])),
    }
}
fn miller_double(point: &MillerG2Witness) -> (MillerG2Witness, [Fp2; 3]) {
    let (x, y, z) = (fq2(point.x), fq2(point.y), fq2(point.z));
    let half = Fq2::new(Fq::from(2_u64).inverse().unwrap(), Fq::ZERO);
    let xy_half = x * y * half;
    let y_squared = y.square();
    let z_squared = z.square();
    let curve_term = Fq2::new(Fq::from(4), Fq::from(4)) * (z_squared + z_squared + z_squared);
    let triple_curve_term = curve_term + curve_term + curve_term;
    let mean_term = (y_squared + triple_curve_term) * half;
    let yz_cross = (y + z).square() - y_squared - z_squared;
    let line_constant = curve_term - y_squared;
    let x_squared = x.square();
    (
        MillerG2Witness {
            x: fp2(xy_half * (y_squared - triple_curve_term)),
            y: fp2(mean_term.square()
                - (curve_term.square() + curve_term.square() + curve_term.square())),
            z: fp2(y_squared * yz_cross),
        },
        [
            fp2(line_constant),
            fp2(x_squared + x_squared + x_squared),
            fp2(-yz_cross),
        ],
    )
}
fn miller_add(point: &MillerG2Witness, addend: G2Affine) -> (MillerG2Witness, [Fp2; 3]) {
    let (x, y, z) = (fq2(point.x), fq2(point.y), fq2(point.z));
    let theta = y - addend.y * z;
    let lambda = x - addend.x * z;
    let theta_squared = theta.square();
    let lambda_squared = lambda.square();
    let lambda_cubed = lambda * lambda_squared;
    let z_theta_squared = z * theta_squared;
    let x_lambda_squared = x * lambda_squared;
    let joined_term = lambda_cubed + z_theta_squared - x_lambda_squared - x_lambda_squared;
    (
        MillerG2Witness {
            x: fp2(lambda * joined_term),
            y: fp2(theta * (x_lambda_squared - joined_term) - lambda_cubed * y),
            z: fp2(z * lambda_cubed),
        },
        [
            fp2(theta * addend.x - lambda * addend.y),
            fp2(-theta),
            fp2(lambda),
        ],
    )
}
fn isogeny(p: ark_ec::short_weierstrass::Affine<Iso>) -> G2Affine {
    fn polynomial(coefficients: &[Fq2], x: Fq2) -> Fq2 {
        coefficients
            .iter()
            .rev()
            .fold(Fq2::ZERO, |acc, c| acc * x + c)
    }
    let map = g2::Config::ISOGENY_MAP;
    G2Affine::new_unchecked(
        polynomial(map.x_map_numerator, p.x)
            * polynomial(map.x_map_denominator, p.x).inverse().unwrap(),
        p.y * polynomial(map.y_map_numerator, p.x)
            * polynomial(map.y_map_denominator, p.x).inverse().unwrap(),
    )
}
fn oracle(step: Step, c: &BlsContextWitness, state: &BlsStateWitness) -> BlsStateWitness {
    use BlsStateWitness as S;
    match (step, state) {
        (Step::Start, S::Empty) => S::Key([
            c.key_point,
            w1(G1Affine::identity()),
            w1(G1Affine::identity()),
        ]),
        (Step::G1(index), S::Key(a)) => {
            let mut r = a.map(p1);
            g1_step(G1_SUBGROUP_STEPS[index], &mut r);
            if index + 1 == G1_SUBGROUP_STEPS.len() {
                assert_eq!(r[1], r[2]);
            }
            S::Key(r.map(w1))
        }
        (Step::StartSignature, S::Key(_)) => {
            let mut r = [w2(G2Affine::identity()); 6];
            r[0] = c.signature_point;
            S::Signature(r)
        }
        (Step::G2(index), S::Signature(a)) => {
            let mut r = a.map(p2);
            g2_step(G2_SUBGROUP_STEPS[index], &mut r);
            if index + 1 == G2_SUBGROUP_STEPS.len() {
                assert_eq!(r[1], r[2]);
            }
            S::Signature(r.map(w2))
        }
        (Step::HashFields, S::Signature(_)) => {
            let mut message = W3F_SIGNING_PREFIX.to_vec();
            message.extend_from_slice(&c.message);
            let hash = <DefaultFieldHasher<Sha256> as HashToField<Fq2>>::new(&[1]);
            let values: Vec<Fq2> = hash.hash_to_field(&message, 2);
            S::Fields([fp2(values[0]), fp2(values[1])])
        }
        (Step::Swu0, S::Fields(a)) => {
            let p = SWUMap::<Iso>::new()
                .unwrap()
                .map_to_curve(fq2(a[0]))
                .unwrap();
            S::SwuFirst {
                x: fp2(p.x),
                y: fp2(p.y),
                second: a[1],
            }
        }
        (Step::Iso0, S::SwuFirst { x, y, second }) => {
            let p = ark_ec::short_weierstrass::Affine::<Iso>::new_unchecked(fq2(*x), fq2(*y));
            let mapped = isogeny(p);
            S::First {
                point: w2(mapped),
                second: *second,
            }
        }
        (Step::Swu1, S::First { point, second }) => {
            let p = SWUMap::<Iso>::new()
                .unwrap()
                .map_to_curve(fq2(*second))
                .unwrap();
            S::SwuSecond {
                first: *point,
                x: fp2(p.x),
                y: fp2(p.y),
            }
        }
        (Step::Iso1, S::SwuSecond { first, x, y }) => {
            let p = ark_ec::short_weierstrass::Affine::<Iso>::new_unchecked(fq2(*x), fq2(*y));
            S::Points([*first, w2(isogeny(p))])
        }
        (Step::StartCofactor, S::Points(a)) => {
            let mut r = [w2(G2Affine::identity()); 6];
            r[0] = w2((p2(a[0]) + p2(a[1])).into_affine());
            S::Cofactor(r)
        }
        (Step::Cofactor(index), S::Cofactor(a)) => {
            let mut r = a.map(p2);
            g2_step(G2_COFACTOR_STEPS[index], &mut r);
            if index + 1 == G2_COFACTOR_STEPS.len() {
                assert_eq!(r[4], r[0].clear_cofactor());
            }
            S::Cofactor(r.map(w2))
        }
        (Step::StartMiller, S::Cofactor(a)) => {
            let h = a[4];
            S::Miller {
                message_point: h,
                points: Box::new([h, c.signature_point].map(|p| MillerG2Witness {
                    x: p.x,
                    y: p.y,
                    z: fp2(Fq2::ONE),
                })),
                lines: Box::new([[[native::ZERO; 2]; 3]; 2]),
                accumulator: Box::new(ONE12),
            }
        }
        (
            Step::Miller(index),
            S::Miller {
                message_point,
                points,
                lines,
                accumulator,
            },
        ) => {
            let mut points = **points;
            let mut lines = **lines;
            let mut acc = fq12(accumulator);
            match MILLER_STEPS[index] {
                MillerStep::Square => acc.square_in_place(),
                MillerStep::Double { pair } => {
                    (points[pair], lines[pair]) = miller_double(&points[pair]);
                    &mut acc
                }
                MillerStep::Add { pair } => {
                    (points[pair], lines[pair]) = miller_add(
                        &points[pair],
                        [p2(*message_point), p2(c.signature_point)][pair],
                    );
                    &mut acc
                }
                MillerStep::Evaluate { pair } => {
                    let line = lines[pair].map(fq2);
                    let g1 = [p1(c.key_point), -G1Affine::generator()][pair];
                    acc.mul_by_014(
                        &line[0],
                        &(line[1] * Fq2::new(g1.x, Fq::ZERO)),
                        &(line[2] * Fq2::new(g1.y, Fq::ZERO)),
                    );
                    &mut acc
                }
                MillerStep::Conjugate => {
                    acc = Fq12::new(acc.c0, -acc.c1);
                    &mut acc
                }
            };
            S::Miller {
                message_point: *message_point,
                points: Box::new(points),
                lines: Box::new(lines),
                accumulator: Box::new(fp12(&acc)),
            }
        }
        (
            Step::StartFinal,
            S::Miller {
                accumulator,
                message_point,
                ..
            },
        ) => {
            assert_eq!(
                fq12(accumulator),
                Bls12_381::multi_miller_loop(
                    [p1(c.key_point), -G1Affine::generator()],
                    [p2(*message_point), p2(c.signature_point)],
                )
                .0,
                "fixed Miller trace matches the independent native pairing engine"
            );
            let mut r = [ZERO12; 5];
            r[0] = **accumulator;
            S::Final(Box::new(r))
        }
        (Step::Final(index), S::Final(a)) => {
            let mut r = a.each_ref().map(fq12);
            match FINAL_EXPONENT_STEPS[index] {
                FinalExponentStep::Copy {
                    destination,
                    source,
                } => r[destination] = r[source],
                FinalExponentStep::Multiply {
                    destination,
                    left,
                    right,
                } => r[destination] = r[left] * r[right],
                FinalExponentStep::Square {
                    destination,
                    source,
                } => r[destination] = r[source].square(),
                FinalExponentStep::Inverse {
                    destination,
                    source,
                } => r[destination] = r[source].inverse().unwrap(),
                FinalExponentStep::Conjugate {
                    destination,
                    source,
                } => r[destination] = Fq12::new(r[source].c0, -r[source].c1),
                FinalExponentStep::Frobenius {
                    destination,
                    source,
                    power,
                } => r[destination] = r[source].frobenius_map(power),
            }
            S::Final(Box::new(r.each_ref().map(fp12)))
        }
        (Step::Finish, S::Final(_)) => S::Done,
        _ => panic!("wrong oracle shape at {step:?}"),
    }
}
pub(super) fn context() -> BlsContextWitness {
    let mut secret = [0; 32];
    secret[0] = 17;
    let key =
        KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::BlsNormal, &secret).unwrap())
            .unwrap();
    let message = core::array::from_fn(|i| u8::try_from(i).unwrap());
    let signature = Signature::new(key.private_key(), &message);
    assert!(signature.verify(key.public_key(), &message).is_ok());
    let public_key = key.public_key().to_bytes().1.try_into().unwrap();
    let bytes = signature.payload().try_into().unwrap();
    BlsContextWitness {
        message,
        public_key,
        signature: bytes,
        key_point: w1(G1Affine::deserialize_compressed(public_key.as_slice()).unwrap()),
        signature_point: w2(G2Affine::deserialize_compressed(bytes.as_slice()).unwrap()),
    }
}
fn trace(c: &BlsContextWitness) -> Vec<BlsLeafCircuit> {
    let mut before = BlsStateWitness::Empty;
    (0..BlsLeafPlan::LENGTH)
        .map(|cursor| {
            let plan = BlsLeafPlan::at(cursor).unwrap();
            let after = oracle(plan.step(), c, &before);
            let leaf = BlsLeafCircuit::new(plan, c.clone(), before.clone(), after.clone()).unwrap();
            before = after;
            leaf
        })
        .collect()
}
fn assert_trace(trace: &[BlsLeafCircuit], valid: bool) {
    assert_eq!(trace.len(), BlsLeafPlan::LENGTH as usize);
    let first = trace.first().unwrap().endpoints();
    let last = trace.last().unwrap().endpoints();
    assert_eq!(first[4], boundary_digest_native(first[1], false));
    assert_eq!(last[5], boundary_digest_native(last[1], true));
    for pair in trace.windows(2) {
        let a = pair[0].endpoints();
        let b = pair[1].endpoints();
        assert_eq!(a[1], b[1]);
        assert_eq!(a[3], b[2]);
        assert_eq!(a[5], b[4]);
    }
    if let BlsStateWitness::Final(r) = &trace.last().unwrap().before {
        assert_eq!(r[0] == ONE12, valid);
    } else {
        panic!("fixed terminal phase");
    }
}
#[test]
fn native_complete_trace_matches_real_normal_bls() {
    let c = context();
    let good = trace(&c);
    assert_trace(&good, true);
    let mut bad_message = c.clone();
    bad_message.message[37] ^= 1;
    assert_trace(&trace(&bad_message), false);
    let mut bad_signature = c;
    let changed = (p2(bad_signature.signature_point) + G2Affine::generator()).into_affine();
    bad_signature.signature_point = w2(changed);
    let mut bytes = Vec::new();
    changed.serialize_compressed(&mut bytes).unwrap();
    bad_signature.signature = bytes.try_into().unwrap();
    assert_trace(&trace(&bad_signature), false);
    assert!(BlsLeafPlan::at(BlsLeafPlan::LENGTH).is_none());
    assert_eq!(w1(G1Affine::generator()), G1_GENERATOR);
}
fn check(leaf: &BlsLeafCircuit) -> bool {
    let started = std::time::Instant::now();
    let instances = leaf.instances().unwrap();
    let compiled = synthesize(leaf, 16, Some(&instances))
        .unwrap_or_else(|e| panic!("leaf {} {:?}: {e:?}", leaf.plan.cursor(), leaf.plan.step()));
    let used = compiled
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    let report =
        iroha_plonk::check::check(&compiled.cs, &compiled.tables, CheckMode::Strict).unwrap();
    eprintln!(
        "BLS leaf {} {:?}: {used} advice rows, {:?}, valid={}",
        leaf.plan.cursor(),
        leaf.plan.step(),
        started.elapsed(),
        report.is_satisfied()
    );
    if !report.is_satisfied() {
        for failure in report.failures().iter().take(3) {
            eprintln!("{failure}");
        }
    }
    report.is_satisfied()
}

#[test]
fn strict_bls_program_boundaries_and_terminal_reject_invalid_signature() {
    let c = context();
    let good = trace(&c);
    assert!(check(&good[0]));
    assert!(check(good.last().unwrap()));
    let mut bad = c;
    bad.message[37] ^= 1;
    let bad = trace(&bad);
    assert!(
        !check(bad.last().unwrap()),
        "fully recomputed invalid signature must fail the pairing equation"
    );
    let mut forged_state = good[1].clone();
    if let BlsStateWitness::Key(registers) = &mut forged_state.after {
        registers[1] = w1(G1Affine::identity());
    } else {
        panic!("fixed key-copy phase");
    }
    assert!(
        !check(&forged_state),
        "a consistently recomputed false output commitment must fail"
    );
    let mut encoded = good[0].clone();
    encoded.context.public_key[8] ^= 1;
    assert!(
        !check(&encoded),
        "compressed bytes cannot disagree with the point"
    );
}
/// Full arithmetic qualification run; 1,084 strict k16 source circuits.
#[test]
#[ignore = "expensive complete BLS source qualification; run explicitly before qualifying these keys"]
fn strict_complete_native_bls_trace_at_k16() {
    let trace = trace(&context());
    assert_trace(&trace, true);
    for leaf in &trace {
        assert!(
            check(leaf),
            "fixed leaf {} {:?}",
            leaf.plan.cursor(),
            leaf.plan.step()
        );
    }
}
#[test]
#[ignore = "expensive representative k16 layouts for every source phase"]
fn strict_bls_phase_layouts_and_consistent_state_mutations() {
    let trace = trace(&context());
    for cursor in [
        0, 1, 2, 141, 142, 143, 144, 213, 214, 215, 216, 217, 218, 219, 220, 221, 370, 371, 372,
        373, 374, 375, 376, 377, 378, 706, 707, 708, 709, 710, 711, 712, 713, 1082, 1083,
    ] {
        let leaf = &trace[cursor];
        assert!(check(leaf), "leaf {cursor}");
        let mut public = leaf.instances().unwrap();
        public[0][0] += Fp::ONE;
        assert!(
            !check_circuit(leaf, 16, &public, CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        );
        synthesize(&leaf.without_witnesses(), 16, None).unwrap();
    }
}

#[test]
fn bounded_production_witness_matches_every_native_trace_register_without_authority() {
    for changed_message in [false, true] {
        let mut source = context();
        if changed_message {
            source.message[37] ^= 1;
        }
        let expected = trace(&source);
        let actual = prepare_bls_witness(source.message, source.public_key, source.signature)
            .expect("fixed native point encodings and bounded witness arithmetic");
        assert_eq!(actual.len(), BlsLeafPlan::LENGTH as usize);
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.plan, expected.plan);
            assert_eq!(actual.context.message, expected.context.message);
            assert_eq!(actual.context.public_key, expected.context.public_key);
            assert_eq!(actual.context.signature, expected.context.signature);
            assert_eq!(actual.context.key_point, expected.context.key_point);
            assert_eq!(
                actual.context.signature_point,
                expected.context.signature_point
            );
            assert_eq!(actual.before.tag(), expected.before.tag());
            assert_eq!(
                actual.before.words(),
                expected.before.words(),
                "before {}",
                actual.plan.cursor()
            );
            assert_eq!(actual.after.tag(), expected.after.tag());
            assert_eq!(
                actual.after.words(),
                expected.after.words(),
                "after {}",
                actual.plan.cursor()
            );
        }
        assert_eq!(
            check(actual.last().unwrap()),
            !changed_message,
            "witness preparation grants no signature acceptance; the final circuit decides"
        );
    }
    let source = context();
    for bytes in [[0; 48], {
        let mut identity = [0; 48];
        identity[0] = 0xc0;
        identity
    }] {
        assert!(matches!(
            prepare_bls_witness(source.message, bytes, source.signature),
            Err(BlsWitnessError::PublicKey)
        ));
    }
    for bytes in [[0; 96], {
        let mut identity = [0; 96];
        identity[0] = 0xc0;
        identity
    }] {
        assert!(matches!(
            prepare_bls_witness(source.message, source.public_key, bytes),
            Err(BlsWitnessError::Signature)
        ));
    }
}

#[test]
fn metadata_source_factory_has_every_exact_program_phase_without_live_inputs() {
    for cursor in 0..BlsLeafPlan::LENGTH {
        let plan = BlsLeafPlan::at(cursor).unwrap();
        let source = BlsLeafCircuit::for_source(plan).unwrap();
        assert_eq!(source.plan(), plan);
        assert_eq!(source.before.tag(), plan.before_tag());
        assert_eq!(source.after.tag(), plan.after_tag());
        assert!(!source.known);
        assert_eq!(source.context.message, [0; 165]);
        assert_eq!(source.context.public_key, [0; 48]);
        assert_eq!(source.context.signature, [0; 96]);
    }
    assert!(BlsStateWitness::for_tag(12).is_err());
    assert!(BlsLeafPlan::at(BlsLeafPlan::LENGTH).is_none());
}

#[test]
#[ignore = "original k16 metadata-only layout equality across every BLS phase and varied cursors"]
fn metadata_source_factory_preserves_original_bls_layouts() {
    let trace = trace(&context());
    for cursor in [
        0, 1, 2, 141, 142, 143, 144, 213, 214, 215, 216, 217, 218, 219, 220, 221, 370, 371, 372,
        373, 374, 375, 376, 377, 378, 706, 707, 708, 709, 710, 711, 712, 713, 1082, 1083,
    ] {
        let original = synthesize(&trace[cursor].without_witnesses(), 16, None).unwrap();
        let source = BlsLeafCircuit::for_source(trace[cursor].plan()).unwrap();
        let metadata = synthesize(&source, 16, None).unwrap();
        assert_eq!(
            original.tables.fixed(),
            metadata.tables.fixed(),
            "fixed {cursor}"
        );
        assert_eq!(
            original.tables.selectors(),
            metadata.tables.selectors(),
            "selectors {cursor}"
        );
        assert_eq!(
            original.tables.permutation(),
            metadata.tables.permutation(),
            "permutation {cursor}"
        );
        assert_eq!(
            original.tables.advice_assigned(),
            metadata.tables.advice_assigned(),
            "advice {cursor}"
        );
    }
}
