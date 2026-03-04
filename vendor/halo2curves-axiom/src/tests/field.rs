use crate::serde::SerdeObject;
use crate::{ff::Field, ff_ext::Legendre};
use ark_std::{end_timer, start_timer};
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

pub fn random_field_tests<F: Field>(type_name: String) {
    let mut rng = XorShiftRng::from_seed([
        0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc,
        0xe5,
    ]);

    random_multiplication_tests::<F, _>(&mut rng, type_name.clone());
    random_addition_tests::<F, _>(&mut rng, type_name.clone());
    random_subtraction_tests::<F, _>(&mut rng, type_name.clone());
    random_negation_tests::<F, _>(&mut rng, type_name.clone());
    random_doubling_tests::<F, _>(&mut rng, type_name.clone());
    random_squaring_tests::<F, _>(&mut rng, type_name.clone());
    random_inversion_tests::<F, _>(&mut rng, type_name.clone());
    random_expansion_tests::<F, _>(&mut rng, type_name);

    assert_eq!(F::ZERO.is_zero().unwrap_u8(), 1);
    {
        let mut z = F::ZERO;
        z = z.neg();
        assert_eq!(z.is_zero().unwrap_u8(), 1);
    }

    assert!(bool::from(F::ZERO.invert().is_none()));

    // Multiplication by zero
    {
        let mut a = F::random(&mut rng);
        a.mul_assign(&F::ZERO);
        assert_eq!(a.is_zero().unwrap_u8(), 1);
    }

    // Addition by zero
    {
        let mut a = F::random(&mut rng);
        let copy = a;
        a.add_assign(&F::ZERO);
        assert_eq!(a, copy);
    }
}

fn random_multiplication_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    let _message = format!("multiplication {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let a = F::random(&mut rng);
        let b = F::random(&mut rng);
        let c = F::random(&mut rng);

        let mut t0 = a; // (a * b) * c
        t0.mul_assign(&b);
        t0.mul_assign(&c);

        let mut t1 = a; // (a * c) * b
        t1.mul_assign(&c);
        t1.mul_assign(&b);

        let mut t2 = b; // (b * c) * a
        t2.mul_assign(&c);
        t2.mul_assign(&a);

        assert_eq!(t0, t1);
        assert_eq!(t1, t2);
    }
    end_timer!(start);
}

fn random_addition_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    let _message = format!("addition {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let a = F::random(&mut rng);
        let b = F::random(&mut rng);
        let c = F::random(&mut rng);

        let mut t0 = a; // (a + b) + c
        t0.add_assign(&b);
        t0.add_assign(&c);

        let mut t1 = a; // (a + c) + b
        t1.add_assign(&c);
        t1.add_assign(&b);

        let mut t2 = b; // (b + c) + a
        t2.add_assign(&c);
        t2.add_assign(&a);

        assert_eq!(t0, t1);
        assert_eq!(t1, t2);
    }
    end_timer!(start);
}

fn random_subtraction_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    let _message = format!("subtraction {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let a = F::random(&mut rng);
        let b = F::random(&mut rng);

        let mut t0 = a; // (a - b)
        t0.sub_assign(&b);

        let mut t1 = b; // (b - a)
        t1.sub_assign(&a);

        let mut t2 = t0; // (a - b) + (b - a) = 0
        t2.add_assign(&t1);

        assert_eq!(t2.is_zero().unwrap_u8(), 1);
    }
    end_timer!(start);
}

fn random_negation_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    let _message = format!("negation {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let a = F::random(&mut rng);
        let mut b = a;
        b = b.neg();
        b.add_assign(&a);

        assert_eq!(b.is_zero().unwrap_u8(), 1);
    }
    end_timer!(start);
}

fn random_doubling_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    let _message = format!("doubling {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let mut a = F::random(&mut rng);
        let mut b = a;
        a.add_assign(&b);
        b = b.double();

        assert_eq!(a, b);
    }
    end_timer!(start);
}

fn random_squaring_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    let _message = format!("squaring {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let mut a = F::random(&mut rng);
        let mut b = a;
        a.mul_assign(&b);
        b = b.square();

        assert_eq!(a, b);
    }
    end_timer!(start);
}

fn random_inversion_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    assert!(bool::from(F::ZERO.invert().is_none()));

    let _message = format!("inversion {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let mut a = F::random(&mut rng);
        let b = a.invert().unwrap(); // probablistically nonzero
        a.mul_assign(&b);

        assert_eq!(a, F::ONE);
    }
    end_timer!(start);
}

fn random_expansion_tests<F: Field, R: RngCore>(mut rng: R, type_name: String) {
    let _message = format!("expansion {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        // Compare (a + b)(c + d) and (a*c + b*c + a*d + b*d)

        let a = F::random(&mut rng);
        let b = F::random(&mut rng);
        let c = F::random(&mut rng);
        let d = F::random(&mut rng);

        let mut t0 = a;
        t0.add_assign(&b);
        let mut t1 = c;
        t1.add_assign(&d);
        t0.mul_assign(&t1);

        let mut t2 = a;
        t2.mul_assign(&c);
        let mut t3 = b;
        t3.mul_assign(&c);
        let mut t4 = a;
        t4.mul_assign(&d);
        let mut t5 = b;
        t5.mul_assign(&d);

        t2.add_assign(&t3);
        t2.add_assign(&t4);
        t2.add_assign(&t5);

        assert_eq!(t0, t2);
    }
    end_timer!(start);
}

pub fn random_conversion_tests<F: ff::PrimeField<Repr = [u8; 32]>>(type_name: String) {
    let mut rng = XorShiftRng::from_seed([
        0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc,
        0xe5,
    ]);
    let _message = format!("conversion {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let a = F::random(&mut rng);
        let bytes = a.to_repr();
        let b = F::from_repr(bytes).unwrap();
        assert_eq!(a, b);
    }
    end_timer!(start);
}

#[cfg(feature = "bits")]
pub fn random_bits_tests<F: ff::PrimeFieldBits>(type_name: String) {
    let mut rng = XorShiftRng::from_seed([
        0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc,
        0xe5,
    ]);
    let _message = format!("to_le_bits {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let a = F::random(&mut rng);
        let bytes = a.to_repr();
        let bits = a.to_le_bits();
        for idx in 0..bits.len() {
            assert_eq!(bits[idx], ((bytes.as_ref()[idx / 8] >> (idx % 8)) & 1) == 1);
        }
    }
    end_timer!(start);
}

pub fn random_serialization_test<F: Field + SerdeObject>(type_name: String) {
    let mut rng = XorShiftRng::from_seed([
        0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc,
        0xe5,
    ]);
    let _message = format!("serialization with SerdeObject {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        let a = F::random(&mut rng);
        let bytes = a.to_raw_bytes();
        let b = F::from_raw_bytes(&bytes).unwrap();
        assert_eq!(a, b);
        let mut buf = Vec::new();
        a.write_raw(&mut buf).unwrap();
        let b = F::read_raw(&mut &buf[..]).unwrap();
        assert_eq!(a, b);
    }
    end_timer!(start);
}

#[cfg(feature = "derive_serde")]
pub fn random_serde_test<F>(type_name: String)
where
    F: Field + SerdeObject + Serialize + for<'de> Deserialize<'de>,
{
    let mut rng = XorShiftRng::from_seed([
        0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc,
        0xe5,
    ]);
    let _message = format!("serialization with serde {type_name}");
    let start = start_timer!(|| _message);
    for _ in 0..1000000 {
        // byte serialization
        let a = F::random(&mut rng);
        let bytes = bincode::serialize(&a).unwrap();
        let reader = std::io::Cursor::new(bytes);
        let b: F = bincode::deserialize_from(reader).unwrap();
        assert_eq!(a, b);

        // json serialization
        let json = serde_json::to_string(&a).unwrap();
        let reader = std::io::Cursor::new(json);
        let b: F = serde_json::from_reader(reader).unwrap();
        assert_eq!(a, b);
    }
    end_timer!(start);
}

pub fn random_quadratic_residue_test<F: Field + Legendre>() {
    let mut rng = XorShiftRng::from_seed([
        0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc,
        0xe5,
    ]);
    for _ in 0..100000 {
        let elem = F::random(&mut rng);
        let is_quad_res_or_zero: bool = elem.sqrt().is_some().into();
        let is_quad_non_res: bool = elem.ct_quadratic_non_residue().into();
        assert_eq!(!is_quad_non_res, is_quad_res_or_zero)
    }
}
