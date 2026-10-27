use ff::Field;
use rand_core::RngCore;

pub(crate) fn mul_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
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
}

pub(crate) fn add_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
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
}

pub(crate) fn sub_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
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
}

pub(crate) fn neg_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
        let a = F::random(&mut rng);
        let mut b = a;
        b = b.neg();
        b.add_assign(&a);

        assert_eq!(b.is_zero().unwrap_u8(), 1);
    }
}

pub(crate) fn double_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
        let mut a = F::random(&mut rng);
        let mut b = a;
        a.add_assign(&b);
        b = b.double();

        assert_eq!(a, b);
    }
}

pub(crate) fn square_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
        let mut a = F::random(&mut rng);
        let mut b = a;
        a.mul_assign(&b);
        b = b.square();

        assert_eq!(a, b);
    }
}

pub(crate) fn inv_test<F: Field>(mut rng: impl RngCore, n: usize) {
    assert!(bool::from(F::ZERO.invert().is_none()));

    for _ in 0..n {
        let mut a = F::random(&mut rng);
        let b = a.invert().unwrap(); // probabilistically nonzero
        a.mul_assign(&b);

        assert_eq!(a, F::ONE);
    }
}

pub(crate) fn expansion_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
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
}

pub(crate) fn zero_test<F: Field>(mut rng: impl RngCore) {
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

pub(crate) fn one_test<F: Field>(mut rng: impl RngCore) {
    assert!(bool::from(F::ONE.invert().is_some()));

    // Multiplication by one
    {
        let mut a = F::random(&mut rng);
        let copy = a;
        a.mul_assign(&F::ONE);
        assert_eq!(a, copy);
    }

    // Addition by one
    {
        let mut a = F::random(&mut rng);
        let copy = a;
        a.add_assign(&F::ONE);
        assert_eq!(a, copy + F::ONE);
    }
}

#[macro_export]
macro_rules! arith_test {
    ($field:ident) => {
        test!(arith, $field, mul_test, 1000);
        test!(arith, $field, add_test, 1000);
        test!(arith, $field, sub_test, 1000);
        test!(arith, $field, neg_test, 1000);
        test!(arith, $field, double_test, 1000);
        test!(arith, $field, square_test, 1000);
        test!(arith, $field, inv_test, 1000);
        test!(arith, $field, expansion_test, 1000);
        test!(arith, $field, one_test);
        test!(arith, $field, zero_test);
    };
}

// This test is autside the `arith_test` macro since it is only
// implemented for prime fields and quadratic extensions.
pub(crate) fn sqrt_test<F: Field>(mut rng: impl RngCore, n: usize) {
    for _ in 0..n {
        let a = F::random(&mut rng);
        let b = a.square();

        let b = b.sqrt().unwrap();
        let negb = b.neg();

        assert!(a == b || a == negb);
    }
}
