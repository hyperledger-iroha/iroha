//! FFT, IFFT and coset transforms against the vendored `EvaluationDomain`.
//!
//! On both scalar fields and for every k checked, `iroha_pasta::fft::FftDomain`
//! must agree element for element with:
//!
//! - the forward transform of every vendored FFT backend (`best_fft`, which
//!   dispatches by architecture, and `recursive`, `parallel` and `baseline`
//!   explicitly, so the result does not depend on the host);
//! - `EvaluationDomain::lagrange_to_coeff` and every backend's inverse
//!   transform scaled by `1/n`;
//! - `EvaluationDomain::coeff_to_extended` (coset FFT by `ZETA` on the extended
//!   domain), `coeff_to_extended_part` (coset FFT by `ZETA * extended_omega^i`
//!   on the base domain) and `extended_to_coeff` (coset IFFT by `ZETA`), for
//!   quotient degrees that extend the domain by 0 to 3 bits;
//! - the domain generators `omega`, `omega^-1` and `extended_omega`.
//!
//! Inputs are random vectors plus zero, unit impulses, constants and an
//! alternating vector. k = 1..=14 by default (coset transforms at selected k
//! up to 12); k = 15 and 16 and the larger cosets are ignored release tests.

use halo2_axiom::{
    arithmetic::best_fft,
    fft::{baseline, parallel, recursive, recursive::FFTData},
    halo2curves::ff::{Field, PrimeField, WithSmallOrderMulGroup},
    poly::EvaluationDomain,
};
use iroha_pasta::fft::FftDomain;
use iroha_plonk_oracle::{
    convert::{CurveBridge, NativeScalar, Pallas, Vesta, native_scalar, native_scalars},
    pools::same_on_each_pool,
};

use crate::data_rng;

/// One vendored FFT backend.
type Backend<F> = fn(&mut [F], F, u32, &FFTData<F>, bool);

/// The vendored FFT backends, by name.
fn backends<F: Field>() -> [(&'static str, Backend<F>); 4] {
    [
        ("best_fft", best_fft::<F, F>),
        ("recursive", recursive::fft::<F, F>),
        ("parallel", parallel::fft::<F, F>),
        ("baseline", baseline::fft::<F, F>),
    ]
}

/// Canonical encodings of a vector.
fn reprs<F: PrimeField<Repr = [u8; 32]>>(values: &[F]) -> Vec<[u8; 32]> {
    values.iter().map(PrimeField::to_repr).collect()
}

/// Asserts equal vectors, naming the first differing index.
fn assert_values(label: &str, expected: &[[u8; 32]], actual: &[[u8; 32]]) {
    assert_eq!(expected.len(), actual.len(), "{label}: length");
    if let Some(index) = expected.iter().zip(actual).position(|(e, a)| e != a) {
        panic!("{label}: element {index} differs from the vendored transform");
    }
}

/// Named input vectors of length `n`.
fn inputs<F: Field>(label: &str, n: usize) -> Vec<(&'static str, Vec<F>)> {
    let mut rng = data_rng(label);
    let mut impulse_first = vec![F::ZERO; n];
    impulse_first[0] = F::ONE;
    let mut impulse_last = vec![F::ZERO; n];
    impulse_last[n - 1] = -F::ONE;
    vec![
        ("random", (0..n).map(|_| F::random(&mut rng)).collect()),
        ("random2", (0..n).map(|_| F::random(&mut rng)).collect()),
        ("zero", vec![F::ZERO; n]),
        ("impulse_first", impulse_first),
        ("impulse_last", impulse_last),
        ("constant", vec![F::random(&mut rng); n]),
        (
            "alternating",
            (0..n)
                .map(|i| if i % 2 == 0 { F::ONE } else { -F::ONE })
                .collect(),
        ),
    ]
}

/// Forward and inverse transforms on the base domain of size `2^k`.
fn base_transforms<B: CurveBridge>(k: u32) {
    let label = format!("{} fft k{k}", B::NAME);
    let n = 1_usize << k;
    let vendored = EvaluationDomain::<B::VScalar>::new(1, k);
    let native = FftDomain::<NativeScalar<B>>::new(k).expect("supported k");
    assert_eq!(native.n(), n);
    assert_eq!(
        native.omega(),
        native_scalar::<B>(&vendored.get_omega()),
        "{label}: omega"
    );
    assert_eq!(
        native.omega_inv(),
        native_scalar::<B>(&vendored.get_omega_inv()),
        "{label}: omega_inv"
    );
    let n_inv = B::VScalar::TWO_INV.pow_vartime([u64::from(k)]);
    assert_eq!(native.n_inv(), native_scalar::<B>(&n_inv), "{label}: n_inv");
    let cases = inputs::<B::VScalar>(&label, n);
    same_on_each_pool(&label, |threads| {
        let data = vendored.get_fft_data(n);
        let mut results = Vec::new();
        for (name, values) in &cases {
            let label = format!("{label} {name} at {threads} threads");
            // Forward.
            let mut forward = native_scalars::<B>(values);
            native.fft(&mut forward).expect("length n");
            let forward = reprs(&forward);
            for (backend, transform) in backends::<B::VScalar>() {
                let mut expected = values.clone();
                transform(&mut expected, vendored.get_omega(), k, data, false);
                assert_values(
                    &format!("{label}: fft vs {backend}"),
                    &reprs(&expected),
                    &forward,
                );
            }
            let plain = vendored.coeff_to_extended_part(
                vendored.coeff_from_vec(values.clone()),
                B::VScalar::ZETA.square(),
            );
            assert_values(
                &format!("{label}: fft vs unit coset part"),
                &reprs(&plain),
                &forward,
            );
            // Inverse.
            let mut inverse = native_scalars::<B>(values);
            native.ifft(&mut inverse).expect("length n");
            let inverse = reprs(&inverse);
            let expected = vendored.lagrange_to_coeff(vendored.lagrange_from_vec(values.clone()));
            assert_values(
                &format!("{label}: ifft vs lagrange_to_coeff"),
                &reprs(&expected),
                &inverse,
            );
            for (backend, transform) in backends::<B::VScalar>() {
                let mut expected = values.clone();
                transform(&mut expected, vendored.get_omega_inv(), k, data, true);
                for value in &mut expected {
                    *value *= n_inv;
                }
                assert_values(
                    &format!("{label}: ifft vs {backend}"),
                    &reprs(&expected),
                    &inverse,
                );
            }
            results.push((forward, inverse));
        }
        results
    });
}

#[test]
fn base_transforms_match_vendored_k1_to_k14() {
    for k in 1..=14 {
        base_transforms::<Vesta>(k);
        base_transforms::<Pallas>(k);
    }
}

#[test]
#[ignore = "k = 15 and 16 transforms on both fields; run in release"]
fn base_transforms_match_vendored_k15_k16() {
    for k in [15, 16] {
        base_transforms::<Vesta>(k);
        base_transforms::<Pallas>(k);
    }
}

/// Coset transforms for quotient factor `j` over the base size `2^k`.
fn coset_transforms<B: CurveBridge>(k: u32, j: u32) {
    let label = format!("{} coset k{k} j{j}", B::NAME);
    let n = 1_usize << k;
    let vendored = EvaluationDomain::<B::VScalar>::new(j, k);
    let extended_k = vendored.extended_k();
    let extended_n = 1_usize << extended_k;
    let native_extended = FftDomain::<NativeScalar<B>>::new(extended_k).expect("supported k");
    let native_base = FftDomain::<NativeScalar<B>>::new(k).expect("supported k");
    assert_eq!(
        native_extended.omega(),
        native_scalar::<B>(&vendored.get_extended_omega()),
        "{label}: extended omega"
    );
    let zeta = B::VScalar::ZETA;
    let native_zeta = native_scalar::<B>(&zeta);
    let cases = inputs::<B::VScalar>(&label, n);
    same_on_each_pool(&label, |threads| {
        let mut results = Vec::new();
        for (name, values) in &cases {
            let label = format!("{label} {name} at {threads} threads");
            let polynomial = vendored.coeff_from_vec(values.clone());
            // Coset FFT on the extended domain.
            let extended = vendored.coeff_to_extended(&polynomial);
            let mut padded = native_scalars::<B>(values);
            padded.resize(extended_n, NativeScalar::<B>::ZERO);
            let mut native = padded.clone();
            native_extended
                .coset_fft(&mut native, native_zeta)
                .expect("length 2^extended_k");
            let native = reprs(&native);
            assert_values(
                &format!("{label}: coeff_to_extended"),
                &reprs(&extended),
                &native,
            );
            // Coset IFFT back to coefficients.
            let mut back = native_scalars::<B>(&extended);
            native_extended
                .coset_ifft(&mut back, native_zeta)
                .expect("length 2^extended_k");
            let coefficients = vendored.extended_to_coeff(extended);
            assert_values(
                &format!("{label}: extended_to_coeff"),
                &reprs(&coefficients),
                &reprs(&back),
            );
            assert_eq!(
                back, padded,
                "{label}: the round trip restores the coefficients"
            );
            // Coset parts on the base domain.
            let mut factor = B::VScalar::ONE;
            for part in 0..extended_n / n {
                let expected = vendored.coeff_to_extended_part(polynomial.clone(), factor);
                let mut native_part = native_scalars::<B>(values);
                native_base
                    .coset_fft(&mut native_part, native_scalar::<B>(&(zeta * factor)))
                    .expect("length n");
                assert_values(
                    &format!("{label}: coeff_to_extended_part {part}"),
                    &reprs(&expected),
                    &reprs(&native_part),
                );
                factor *= vendored.get_extended_omega();
            }
            results.push(native);
        }
        results
    });
}

#[test]
fn coset_transforms_match_vendored() {
    for k in [1, 2, 3, 5, 8, 11, 12] {
        for j in [2, 3, 4, 5, 8, 9] {
            coset_transforms::<Vesta>(k, j);
            coset_transforms::<Pallas>(k, j);
        }
    }
}

#[test]
#[ignore = "coset transforms for k = 13..=16; run in release"]
fn coset_transforms_match_vendored_large() {
    for k in 13..=16 {
        for j in [2, 3, 5, 9] {
            coset_transforms::<Vesta>(k, j);
            coset_transforms::<Pallas>(k, j);
        }
    }
}

#[test]
fn native_domain_rejects_wrong_lengths() {
    let domain = FftDomain::<NativeScalar<Vesta>>::new(3).expect("k3");
    let mut short = vec![NativeScalar::<Vesta>::ONE; 7];
    assert!(domain.fft(&mut short).is_err());
    assert!(
        domain
            .coset_ifft(&mut short, NativeScalar::<Vesta>::ONE)
            .is_err()
    );
}

#[test]
fn input_vectors_have_the_intended_shapes() {
    let cases = inputs::<<Pallas as CurveBridge>::VScalar>("shapes", 4);
    assert_eq!(cases.len(), 7);
    let find = |name: &str| &cases.iter().find(|(n, _)| *n == name).expect("case").1;
    assert_eq!(
        find("impulse_first")[0],
        <Pallas as CurveBridge>::VScalar::ONE
    );
    assert_eq!(
        find("impulse_last")[3],
        -<Pallas as CurveBridge>::VScalar::ONE
    );
    assert!(find("zero").iter().all(|v| bool::from(v.is_zero())));
    assert_eq!(backends::<<Vesta as CurveBridge>::VScalar>().len(), 4);
}

#[test]
#[should_panic(expected = "element 1 differs")]
fn assert_values_names_the_first_difference() {
    assert_values("test", &[[0; 32], [1; 32]], &[[0; 32], [2; 32]]);
}
