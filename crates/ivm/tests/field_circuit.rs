#![cfg(feature = "ivm_zk_tests")]
use ivm::{
    field,
    halo2::{FieldCircuit, FieldOp},
};

#[test]
fn test_field_add_sub_mul_inv() {
    let add_res = field::add(5, 7);
    let add = FieldCircuit {
        op: FieldOp::Add,
        a: 5,
        b: 7,
        result: add_res,
    };
    assert!(add.verify().is_ok());
    let add_bad = FieldCircuit {
        op: FieldOp::Add,
        a: 5,
        b: 7,
        result: add_res.wrapping_add(1),
    };
    assert!(add_bad.verify().is_err());

    let sub_res = field::sub(5, 7);
    let sub = FieldCircuit {
        op: FieldOp::Sub,
        a: 5,
        b: 7,
        result: sub_res,
    };
    assert!(sub.verify().is_ok());

    let mul_res = field::mul(5, 7);
    let mul = FieldCircuit {
        op: FieldOp::Mul,
        a: 5,
        b: 7,
        result: mul_res,
    };
    assert!(mul.verify().is_ok());

    let inv_val = 5u64;
    let inv_res = field::inv(inv_val).unwrap();
    let inv = FieldCircuit {
        op: FieldOp::Inv,
        a: inv_val,
        b: 0,
        result: inv_res,
    };
    assert!(inv.verify().is_ok());
    let inv_bad = FieldCircuit {
        op: FieldOp::Inv,
        a: inv_val,
        b: 0,
        result: field::add(inv_res, 1),
    };
    assert!(inv_bad.verify().is_err());

    let inv_zero = FieldCircuit {
        op: FieldOp::Inv,
        a: 0,
        b: 0,
        result: 0,
    };
    assert!(inv_zero.verify().is_err());
}

#[test]
fn test_field_wraparound() {
    let mul_zero = FieldCircuit {
        op: FieldOp::Mul,
        a: 0,
        b: 12345,
        result: 0,
    };
    assert!(mul_zero.verify().is_ok());

    let inv_one = FieldCircuit {
        op: FieldOp::Inv,
        a: 1,
        b: 0,
        result: 1,
    };
    assert!(inv_one.verify().is_ok());
}
