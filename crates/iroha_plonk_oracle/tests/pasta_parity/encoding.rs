//! Field and curve encodings against the `halo2curves-axiom` Pasta types.
//!
//! The vendored stack reaches Pasta through `halo2_axiom::halo2curves::pasta`
//! (`halo2curves-axiom`, which re-exports `pasta_curves` 0.5.2 and adds the
//! `SerdeObject` raw formats). For both fields and both curves this module
//! compares:
//!
//! - the `PrimeField` constants;
//! - decoding verdicts and values for canonical, non-canonical and boundary
//!   byte strings, and the halo2-axiom `SerdeFormat` encodings (`Processed`
//!   and `RawBytes`, which are canonical for Pasta);
//! - arithmetic, square roots, bit decompositions, wide reduction and seeded
//!   sampling on random and edge values;
//! - compressed point encodings, decoding verdicts on boundary and random
//!   byte strings, affine coordinates, `from_xy`, the group law, scalar
//!   multiplication, the endomorphism, batch normalisation and hash-to-curve.

use halo2_axiom::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    halo2curves::{
        Coordinates, CurveAffine, CurveExt,
        ff::{Field, FromUniformBytes, PrimeField, PrimeFieldBits, WithSmallOrderMulGroup},
        group::{Curve, Group, GroupEncoding, prime::PrimeCurveAffine},
        serde::SerdeObject,
    },
};
use iroha_pasta::{PastaAffine, PastaCurve, PastaField};
use iroha_plonk_oracle::{
    convert::{
        CurveBridge, NativeAffine, NativeBase, NativeScalar, Pallas, VendoredCurve, Vesta,
        native_affine, native_base, native_scalar,
    },
    pools::same_on_each_pool,
};
use rand_core::RngCore;

use crate::data_rng;

/// Random samples per field per pool.
const FIELD_SAMPLES: usize = 400;
/// Random points per curve per pool.
const POINT_SAMPLES: usize = 64;
/// Random 32-byte strings decoded per curve per pool.
const DECODE_FUZZ: usize = 2_000;

/// The vendored field side of a parity check.
trait VendoredField:
    PrimeField<Repr = [u8; 32]>
    + PrimeFieldBits<ReprBits = [u64; 4]>
    + FromUniformBytes<64>
    + WithSmallOrderMulGroup<3>
    + Ord
    + SerdeObject
{
}

impl<F> VendoredField for F where
    F: PrimeField<Repr = [u8; 32]>
        + PrimeFieldBits<ReprBits = [u64; 4]>
        + FromUniformBytes<64>
        + WithSmallOrderMulGroup<3>
        + Ord
        + SerdeObject
{
}

/// The native element with the same canonical encoding as `value`.
fn to_native<V: VendoredField, N: PastaField>(value: &V) -> N {
    Option::from(N::from_repr(value.to_repr())).expect("canonical encoding accepted natively")
}

/// Little-endian bytes of `value + delta` for a 256-bit little-endian value
/// (wrapping).
fn add_small(value: [u8; 32], delta: u8) -> [u8; 32] {
    let mut out = value;
    let mut carry = u16::from(delta);
    for byte in &mut out {
        let sum = u16::from(*byte) + carry;
        *byte = sum.to_le_bytes()[0];
        carry = sum >> 8;
    }
    out
}

/// The 256-bit little-endian encoding of the modulus of `F`.
fn modulus_bytes<F: PrimeField<Repr = [u8; 32]>>() -> [u8; 32] {
    add_small((-F::ONE).to_repr(), 1)
}

/// Byte strings around every decoding boundary of a 255-bit field.
fn boundary_encodings<F: PrimeField<Repr = [u8; 32]>>(rng: &mut impl RngCore) -> Vec<[u8; 32]> {
    let modulus = modulus_bytes::<F>();
    let mut out = vec![
        [0; 32],
        add_small([0; 32], 1),
        (-F::ONE).to_repr(),
        modulus,
        add_small(modulus, 1),
        add_small(modulus, 255),
        [0xff; 32],
    ];
    let mut top_clear = [0xff; 32];
    top_clear[31] = 0x7f;
    out.push(top_clear);
    let mut top_only = [0; 32];
    top_only[31] = 0x80;
    out.push(top_only);
    let mut half = [0; 32];
    half[31] = 0x40;
    out.push(half);
    // Limb boundaries 2^64k - 1 and 2^64k.
    for limb in 1..4 {
        let mut below = [0; 32];
        below[..8 * limb].fill(0xff);
        out.push(below);
        out.push(add_small(below, 1));
    }
    // Values sharing the modulus' high bytes, straddling it.
    for low in [1, 2, 4, 8, 16, 24] {
        for _ in 0..8 {
            let mut near = modulus;
            rng.fill_bytes(&mut near[..low]);
            out.push(near);
        }
    }
    for _ in 0..32 {
        let mut random = [0; 32];
        rng.fill_bytes(&mut random);
        out.push(random);
    }
    out
}

/// Encoding, decoding and arithmetic parity of one field pair.
fn field_parity<V: VendoredField, N: PastaField>(label: &str) {
    assert_eq!(V::MODULUS, N::MODULUS, "{label}: MODULUS");
    assert_eq!(V::NUM_BITS, N::NUM_BITS, "{label}: NUM_BITS");
    assert_eq!(V::CAPACITY, N::CAPACITY, "{label}: CAPACITY");
    assert_eq!(V::S, N::S, "{label}: S");
    for (name, vendored, native) in [
        ("TWO_INV", V::TWO_INV.to_repr(), N::TWO_INV.to_repr()),
        (
            "MULTIPLICATIVE_GENERATOR",
            V::MULTIPLICATIVE_GENERATOR.to_repr(),
            N::MULTIPLICATIVE_GENERATOR.to_repr(),
        ),
        (
            "ROOT_OF_UNITY",
            V::ROOT_OF_UNITY.to_repr(),
            N::ROOT_OF_UNITY.to_repr(),
        ),
        (
            "ROOT_OF_UNITY_INV",
            V::ROOT_OF_UNITY_INV.to_repr(),
            N::ROOT_OF_UNITY_INV.to_repr(),
        ),
        ("DELTA", V::DELTA.to_repr(), N::DELTA.to_repr()),
        ("ZETA", V::ZETA.to_repr(), N::ZETA.to_repr()),
    ] {
        assert_eq!(vendored, native, "{label}: {name}");
    }
    assert_eq!(
        V::char_le_bits().into_inner(),
        N::char_le_bits().into_inner(),
        "{label}: char_le_bits"
    );

    same_on_each_pool(label, |threads| {
        let mut rng = data_rng(&format!("pasta parity encoding {label}"));
        // Decoding verdicts and values, plain and through the serde formats.
        for bytes in boundary_encodings::<V>(&mut rng) {
            let vendored: Option<V> = V::from_repr(bytes).into();
            let native: Option<N> = N::from_repr(bytes).into();
            assert_eq!(
                vendored.map(|v| v.to_repr()),
                native.map(|n| n.to_repr()),
                "{label}: from_repr verdict for {} at {threads} threads",
                crate::hex(&bytes)
            );
            assert_eq!(
                V::from_raw_bytes(&bytes).map(|v| v.to_repr()),
                native.map(|n| n.to_repr()),
                "{label}: SerdeObject::from_raw_bytes"
            );
            for format in [SerdeFormat::Processed, SerdeFormat::RawBytes] {
                let read = <V as SerdePrimeField>::read(&mut bytes.as_slice(), format).ok();
                assert_eq!(
                    read.map(|v| v.to_repr()),
                    native.map(|n| n.to_repr()),
                    "{label}: SerdePrimeField::read {format:?}"
                );
            }
        }
        // Arithmetic on random and edge values.
        let mut samples = vec![V::ZERO, V::ONE, -V::ONE, V::TWO_INV, V::ZETA, V::DELTA];
        samples.extend((0..FIELD_SAMPLES).map(|_| V::random(&mut rng)));
        for (index, a) in samples.iter().enumerate() {
            let b = samples[(index * 7 + 3) % samples.len()];
            let (na, nb): (N, N) = (to_native(a), to_native(&b));
            let check = |name: &str, vendored: V, native: N| {
                assert_eq!(
                    vendored.to_repr(),
                    native.to_repr(),
                    "{label}: {name} of {} and {}",
                    crate::hex(&a.to_repr()),
                    crate::hex(&b.to_repr())
                );
            };
            check("add", *a + b, na + nb);
            check("sub", *a - b, na - nb);
            check("mul", *a * b, na * nb);
            check("square", a.square(), na.square());
            check("double", a.double(), na.double());
            check("neg", -*a, -na);
            let exponent = [
                rng.next_u64(),
                rng.next_u64(),
                rng.next_u64(),
                rng.next_u64(),
            ];
            check(
                "pow_vartime",
                a.pow_vartime(exponent),
                na.pow_vartime(exponent),
            );
            let vendored_inverse: Option<V> = a.invert().into();
            let native_inverse: Option<N> = na.invert().into();
            assert_eq!(
                vendored_inverse.map(|v| v.to_repr()),
                native_inverse.map(|n| n.to_repr()),
                "{label}: invert"
            );
            let vendored_root: Option<V> = a.sqrt().into();
            let native_root: Option<N> = na.sqrt().into();
            assert_eq!(
                vendored_root.map(|v| v.to_repr()),
                native_root.map(|n| n.to_repr()),
                "{label}: sqrt returns the same root"
            );
            let (vendored_square, vendored_ratio) = V::sqrt_ratio(a, &b);
            let (native_square, native_ratio) = N::sqrt_ratio(&na, &nb);
            assert_eq!(
                bool::from(vendored_square),
                bool::from(native_square),
                "{label}: sqrt_ratio verdict"
            );
            check("sqrt_ratio", vendored_ratio, native_ratio);
            assert_eq!(
                bool::from(a.is_odd()),
                bool::from(na.is_odd()),
                "{label}: is_odd"
            );
            assert_eq!(
                bool::from(a.is_zero()),
                bool::from(na.is_zero()),
                "{label}: is_zero"
            );
            assert_eq!(a.cmp(&b), na.cmp(&nb), "{label}: Ord");
            assert_eq!(
                a.to_le_bits().into_inner(),
                na.to_le_bits().into_inner(),
                "{label}: to_le_bits"
            );
            assert_eq!(
                a.to_raw_bytes(),
                na.to_repr().to_vec(),
                "{label}: to_raw_bytes"
            );
            for format in [SerdeFormat::Processed, SerdeFormat::RawBytes] {
                let mut written = Vec::new();
                SerdePrimeField::write(a, &mut written, format).expect("write to Vec");
                assert_eq!(written, na.to_repr().to_vec(), "{label}: write {format:?}");
            }
        }
        // Integer embeddings and wide reduction.
        for _ in 0..64 {
            let small = rng.next_u64();
            let wide = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
            assert_eq!(
                V::from(small).to_repr(),
                N::from(small).to_repr(),
                "{label}: from u64"
            );
            assert_eq!(
                V::from_u128(wide).to_repr(),
                N::from_u128(wide).to_repr(),
                "{label}: from_u128"
            );
            let mut uniform = [0; 64];
            rng.fill_bytes(&mut uniform);
            assert_eq!(
                V::from_uniform_bytes(&uniform).to_repr(),
                N::from_uniform_bytes(&uniform).to_repr(),
                "{label}: from_uniform_bytes"
            );
        }
        let mut uniform = [0xff; 64];
        assert_eq!(
            V::from_uniform_bytes(&uniform).to_repr(),
            N::from_uniform_bytes(&uniform).to_repr(),
            "{label}: from_uniform_bytes of 2^512 - 1"
        );
        uniform = [0; 64];
        assert_eq!(
            V::from_uniform_bytes(&uniform).to_repr(),
            N::from_uniform_bytes(&uniform).to_repr(),
            "{label}: from_uniform_bytes of zero"
        );
        // Seeded sampling consumes the stream identically.
        let mut vendored_stream = data_rng(&format!("pasta parity sampling {label}"));
        let mut native_stream = vendored_stream.clone();
        for _ in 0..64 {
            assert_eq!(
                V::random(&mut vendored_stream).to_repr(),
                N::random(&mut native_stream).to_repr(),
                "{label}: Field::random"
            );
        }
        assert_eq!(vendored_stream.next_u64(), native_stream.next_u64());
    });
}

#[test]
fn field_constants_encodings_and_arithmetic_match() {
    field_parity::<<Vesta as CurveBridge>::VScalar, NativeScalar<Vesta>>("Fp");
    field_parity::<<Pallas as CurveBridge>::VScalar, NativeScalar<Pallas>>("Fq");
    // The base fields are the same two fields seen from the other curve.
    field_parity::<<Vesta as CurveBridge>::VBase, NativeBase<Vesta>>("Fq as Vesta base");
}

/// Point byte strings around every decoding boundary of curve `B`.
fn point_encodings<B: CurveBridge>(rng: &mut impl RngCore) -> Vec<[u8; 32]> {
    let modulus = modulus_bytes::<B::VBase>();
    let mut out = vec![[0; 32]];
    let mut signed_zero = [0; 32];
    signed_zero[31] = 0x80;
    out.push(signed_zero);
    for base in [modulus, add_small(modulus, 1), add_small(modulus, 2)] {
        let mut signed = base;
        signed[31] |= 0x80;
        out.push(base);
        out.push(signed);
    }
    let mut all_ones = [0xff; 32];
    out.push(all_ones);
    all_ones[31] = 0x7f;
    out.push(all_ones);
    // Small x values: some have points, some do not.
    for x in 0..24_u8 {
        for sign in [0, 0x80] {
            let mut bytes = [0; 32];
            bytes[0] = x;
            bytes[31] = sign;
            out.push(bytes);
        }
    }
    // Encodings of real points, their negations and sign flips.
    for _ in 0..16 {
        let point = VendoredCurve::<B>::random(&mut *rng).to_affine();
        let bytes = point.to_bytes();
        let mut flipped = bytes;
        flipped[31] ^= 0x80;
        out.push(bytes);
        out.push(flipped);
    }
    for _ in 0..DECODE_FUZZ {
        let mut random = [0; 32];
        rng.fill_bytes(&mut random);
        out.push(random);
    }
    out
}

/// Encoding, decoding and group-law parity of one curve.
fn curve_parity<B: CurveBridge>()
where
    B::Vendored: SerdeObject,
{
    let label = B::NAME;
    let generator = B::Vendored::generator();
    assert_eq!(
        generator.to_bytes(),
        NativeAffine::<B>::generator().to_bytes(),
        "{label}: generator"
    );
    assert_eq!(
        B::Vendored::identity().to_bytes(),
        NativeAffine::<B>::identity().to_bytes(),
        "{label}: identity encoding"
    );
    assert_eq!(
        B::Vendored::b().to_repr(),
        <B::Native as PastaCurve>::b().to_repr(),
        "{label}: b"
    );
    assert!(bool::from(B::Vendored::a().is_zero()), "{label}: a = 0");

    same_on_each_pool(label, |threads| {
        let mut rng = data_rng(&format!("pasta parity curve {label}"));
        // Decoding verdicts, plain and through the serde formats.
        for bytes in point_encodings::<B>(&mut rng) {
            let vendored: Option<B::Vendored> = B::Vendored::from_bytes(&bytes).into();
            let native: Option<NativeAffine<B>> = NativeAffine::<B>::from_bytes(&bytes).into();
            let expected = vendored.map(|p| p.to_bytes());
            assert_eq!(
                expected,
                native.map(|p| p.to_bytes()),
                "{label}: from_bytes verdict for {} at {threads} threads",
                crate::hex(&bytes)
            );
            let native_unchecked: Option<NativeAffine<B>> =
                NativeAffine::<B>::from_bytes_unchecked(&bytes).into();
            let vendored_unchecked: Option<B::Vendored> =
                B::Vendored::from_bytes_unchecked(&bytes).into();
            assert_eq!(
                vendored_unchecked.map(|p| p.to_bytes()),
                native_unchecked.map(|p| p.to_bytes()),
                "{label}: from_bytes_unchecked verdict"
            );
            assert_eq!(
                B::Vendored::from_raw_bytes(&bytes).map(|p| p.to_bytes()),
                expected,
                "{label}: SerdeObject::from_raw_bytes"
            );
            for format in [SerdeFormat::Processed, SerdeFormat::RawBytes] {
                let read = <B::Vendored as SerdeCurveAffine>::read(&mut bytes.as_slice(), format);
                assert_eq!(
                    read.ok().map(|p| p.to_bytes()),
                    expected,
                    "{label}: SerdeCurveAffine::read {format:?}"
                );
            }
            if let (Some(vendored), Some(native)) = (vendored, native) {
                let coordinates: Option<Coordinates<B::Vendored>> = vendored.coordinates().into();
                let native_coordinates: Option<(NativeBase<B>, NativeBase<B>)> =
                    native.coordinates().into();
                assert_eq!(
                    coordinates.map(|c| (c.x().to_repr(), c.y().to_repr())),
                    native_coordinates.map(|(x, y)| (x.to_repr(), y.to_repr())),
                    "{label}: coordinates"
                );
            }
        }
        // from_xy verdicts.
        for _ in 0..32 {
            let point = VendoredCurve::<B>::random(&mut rng).to_affine();
            let c: Coordinates<B::Vendored> = point.coordinates().unwrap();
            let (x, y) = (*c.x(), *c.y());
            let random_x = B::VBase::random(&mut rng);
            for (cx, cy) in [
                (x, y),
                (x, -y),
                (x, y + B::VBase::ONE),
                (random_x, y),
                (B::VBase::ZERO, B::VBase::ZERO),
                (B::VBase::ZERO, B::VBase::ONE),
            ] {
                let vendored: Option<B::Vendored> = B::Vendored::from_xy(cx, cy).into();
                let native: Option<NativeAffine<B>> =
                    NativeAffine::<B>::from_xy(native_base::<B>(&cx), native_base::<B>(&cy)).into();
                assert_eq!(
                    vendored.map(|p| p.to_bytes()),
                    native.map(|p| p.to_bytes()),
                    "{label}: from_xy verdict"
                );
            }
        }
        // Group law, scalar multiplication, the endomorphism and normalisation.
        let points: Vec<VendoredCurve<B>> = (0..POINT_SAMPLES)
            .map(|index| match index {
                0 => VendoredCurve::<B>::identity(),
                1 => VendoredCurve::<B>::generator(),
                _ => VendoredCurve::<B>::random(&mut rng),
            })
            .collect();
        let mut scalars = vec![
            B::VScalar::ZERO,
            B::VScalar::ONE,
            -B::VScalar::ONE,
            B::VScalar::ZETA,
        ];
        scalars.extend((scalars.len()..POINT_SAMPLES).map(|_| B::VScalar::random(&mut rng)));
        let affine = |p: VendoredCurve<B>| p.to_affine().to_bytes();
        let native_bytes = |p: B::Native| p.to_affine().to_bytes();
        let mut vendored_sums = Vec::new();
        let mut native_sums = Vec::new();
        for (index, p) in points.iter().enumerate() {
            let q = points[(index * 5 + 1) % points.len()];
            let s = scalars[index];
            let np = native_affine::<B>(&p.to_affine()).to_curve();
            let nq = native_affine::<B>(&q.to_affine()).to_curve();
            let ns = native_scalar::<B>(&s);
            assert_eq!(affine(*p + q), native_bytes(np + nq), "{label}: add");
            assert_eq!(affine(*p - q), native_bytes(np - nq), "{label}: sub");
            assert_eq!(
                affine(p.double()),
                native_bytes(np.double()),
                "{label}: double"
            );
            assert_eq!(affine(-*p), native_bytes(-np), "{label}: neg");
            assert_eq!(
                affine(*p + q.to_affine()),
                native_bytes(np + nq.to_affine()),
                "{label}: mixed add"
            );
            let product = affine(*p * s);
            assert_eq!(product, native_bytes(np * ns), "{label}: mul");
            assert_eq!(
                product,
                native_bytes(np.mul_vartime(&ns)),
                "{label}: mul_vartime"
            );
            assert_eq!(affine(p.endo()), native_bytes(np.endo()), "{label}: endo");
            vendored_sums.push(*p + q);
            native_sums.push(np + nq);
        }
        let mut vendored_affine = vec![B::Vendored::identity(); vendored_sums.len()];
        VendoredCurve::<B>::batch_normalize(&vendored_sums, &mut vendored_affine);
        let mut native_affine_out = vec![NativeAffine::<B>::identity(); native_sums.len()];
        B::Native::batch_normalize(&native_sums, &mut native_affine_out);
        for (vendored, native) in vendored_affine.iter().zip(&native_affine_out) {
            assert_eq!(
                vendored.to_bytes(),
                native.to_bytes(),
                "{label}: batch_normalize"
            );
        }
        // Seeded sampling consumes the stream identically.
        let mut vendored_stream = data_rng(&format!("pasta parity point sampling {label}"));
        let mut native_stream = vendored_stream.clone();
        for _ in 0..8 {
            assert_eq!(
                VendoredCurve::<B>::random(&mut vendored_stream)
                    .to_affine()
                    .to_bytes(),
                B::Native::random(&mut native_stream).to_affine().to_bytes(),
                "{label}: Group::random"
            );
        }
        assert_eq!(vendored_stream.next_u64(), native_stream.next_u64());
        // Serde writes of points.
        for point in vendored_affine.iter().take(8) {
            let native = native_affine::<B>(point);
            assert_eq!(
                point.to_raw_bytes(),
                native.to_bytes().to_vec(),
                "{label}: to_raw_bytes"
            );
            for format in [SerdeFormat::Processed, SerdeFormat::RawBytes] {
                let mut written = Vec::new();
                SerdeCurveAffine::write(point, &mut written, format).expect("write to Vec");
                assert_eq!(
                    written,
                    native.to_bytes().to_vec(),
                    "{label}: write {format:?}"
                );
            }
        }
        // Hash-to-curve under several domains and messages.
        let mut long = [0; 200];
        rng.fill_bytes(&mut long);
        for domain in ["Halo2-Parameters", "iroha-plonk-oracle", ""] {
            let hasher = VendoredCurve::<B>::hash_to_curve(domain);
            for message in [&[][..], &[0], &[1], &[2], &[0, 1, 0, 0, 0], &long[..]] {
                let native = B::Native::hash_to_curve(domain, message)
                    .expect("short domain")
                    .to_affine()
                    .to_bytes();
                assert_eq!(
                    hasher(message).to_affine().to_bytes(),
                    native,
                    "{label}: hash_to_curve({domain:?}, {} bytes)",
                    message.len()
                );
            }
        }
    });
}

#[test]
fn curve_encodings_and_group_law_match() {
    curve_parity::<Vesta>();
    curve_parity::<Pallas>();
}

#[test]
fn boundary_helpers_cover_the_modulus() {
    use halo2_axiom::halo2curves::pasta::Fp;
    assert_eq!(add_small([0xff; 32], 1), [0; 32], "wrapping carry");
    assert_eq!(add_small([0; 32], 7)[0], 7);
    let modulus = modulus_bytes::<Fp>();
    assert_eq!(add_small((-Fp::ONE).to_repr(), 1), modulus);
    assert!(bool::from(Fp::from_repr(modulus).is_none()));
    let encodings = boundary_encodings::<Fp>(&mut data_rng("boundary"));
    assert!(encodings.contains(&modulus));
    assert!(encodings.contains(&(-Fp::ONE).to_repr()));
    let points = point_encodings::<Vesta>(&mut data_rng("points"));
    assert_eq!(points[0], [0; 32]);
    assert!(points.len() > DECODE_FUZZ);
}
