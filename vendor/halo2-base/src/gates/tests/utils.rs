#![allow(clippy::type_complexity)]
use num_integer::Integer;

use crate::utils::biguint_to_fe;
use crate::utils::fe_to_biguint;
use crate::utils::BigPrimeField;
use crate::utils::ScalarField;
use crate::QuantumCell;

// Ground truth functions

//  Flex Gate Ground Truths

pub fn add_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    *inputs[0].value() + *inputs[1].value()
}

pub fn sub_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    *inputs[0].value() - *inputs[1].value()
}

pub fn sub_mul_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    *inputs[0].value() - *inputs[1].value() * *inputs[2].value()
}

pub fn neg_ground_truth<F: ScalarField>(input: QuantumCell<F>) -> F {
    -(*input.value())
}

pub fn mul_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    *inputs[0].value() * *inputs[1].value()
}

pub fn mul_add_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    *inputs[0].value() * *inputs[1].value() + *inputs[2].value()
}

pub fn mul_not_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    (F::ONE - *inputs[0].value()) * *inputs[1].value()
}

pub fn div_unsafe_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    inputs[1].value().invert().unwrap() * *inputs[0].value()
}

pub fn inner_product_ground_truth<F: ScalarField>(a: &[F], b: &[F]) -> F {
    a.iter().zip(b.iter()).fold(F::ZERO, |acc, (&a, &b)| acc + a * b)
}

pub fn inner_product_left_last_ground_truth<F: ScalarField>(a: &[F], b: &[F]) -> (F, F) {
    let product = inner_product_ground_truth(a, b);
    let last = *a.last().unwrap();
    (product, last)
}

pub fn inner_product_with_sums_ground_truth<F: ScalarField>(
    input: &(Vec<QuantumCell<F>>, Vec<QuantumCell<F>>),
) -> Vec<F> {
    let (a, b) = &input;
    let mut result = Vec::new();
    let mut sum = F::ZERO;
    // TODO: convert to fold
    for (ai, bi) in a.iter().zip(b) {
        let product = *ai.value() * *bi.value();
        sum += product;
        result.push(sum);
    }
    result
}

pub fn sum_products_with_coeff_and_var_ground_truth<F: ScalarField>(
    input: &(Vec<(F, QuantumCell<F>, QuantumCell<F>)>, QuantumCell<F>),
) -> F {
    let expected =
        input.0.iter().fold(F::ZERO, |acc, (coeff, cell1, cell2)| {
            acc + *coeff * *cell1.value() * *cell2.value()
        }) + *input.1.value();
    expected
}

pub fn and_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    *inputs[0].value() * *inputs[1].value()
}

pub fn not_ground_truth<F: ScalarField>(a: &QuantumCell<F>) -> F {
    F::ONE - *a.value()
}

pub fn select_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    (*inputs[0].value() - inputs[1].value()) * *inputs[2].value() + *inputs[1].value()
}

pub fn or_and_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    let bc_val = *inputs[1].value() * inputs[2].value();
    bc_val + inputs[0].value() - bc_val * inputs[0].value()
}

pub fn idx_to_indicator_ground_truth<F: ScalarField>(inputs: (QuantumCell<F>, usize)) -> Vec<F> {
    let (idx, size) = inputs;
    let mut indicator = vec![F::ZERO; size];
    let mut idx_value = size + 1;
    for i in 0..size as u64 {
        if F::from(i) == *idx.value() {
            idx_value = i as usize;
            break;
        }
    }
    if idx_value < size {
        indicator[idx_value] = F::ONE;
    }
    indicator
}

pub fn select_by_indicator_ground_truth<F: ScalarField>(
    inputs: &(Vec<QuantumCell<F>>, QuantumCell<F>),
) -> F {
    let mut idx_value = inputs.0.len() + 1;
    let mut indicator = vec![F::ZERO; inputs.0.len()];
    for i in 0..inputs.0.len() as u64 {
        if F::from(i) == *inputs.1.value() {
            idx_value = i as usize;
            break;
        }
    }
    if idx_value < inputs.0.len() {
        indicator[idx_value] = F::ONE;
    }
    // take cross product of indicator and inputs.0
    inputs.0.iter().zip(indicator.iter()).fold(F::ZERO, |acc, (a, b)| acc + (*a.value() * *b))
}

pub fn select_from_idx_ground_truth<F: ScalarField>(
    inputs: &(Vec<QuantumCell<F>>, QuantumCell<F>),
) -> F {
    let idx = inputs.1.value();
    // Since F does not implement From<u64>, we have to iterate and find the matching index
    for i in 0..inputs.0.len() as u64 {
        if F::from(i) == *idx {
            return *inputs.0[i as usize].value();
        }
    }
    F::ZERO
}

pub fn is_zero_ground_truth<F: ScalarField>(x: F) -> F {
    if x.is_zero().into() {
        F::ONE
    } else {
        F::ZERO
    }
}

pub fn is_equal_ground_truth<F: ScalarField>(inputs: &[QuantumCell<F>]) -> F {
    if inputs[0].value() == inputs[1].value() {
        F::ONE
    } else {
        F::ZERO
    }
}

/*
pub fn lagrange_eval_ground_truth<F: ScalarField>(inputs: &[F]) -> (F, F) {
}
*/

// Range Chip Ground Truths

pub fn is_less_than_ground_truth<F: ScalarField>(inputs: (F, F)) -> F {
    if inputs.0 < inputs.1 {
        F::ONE
    } else {
        F::ZERO
    }
}

pub fn div_mod_ground_truth<F: ScalarField + BigPrimeField>(inputs: (F, u64)) -> (F, F) {
    let a = fe_to_biguint(&inputs.0);
    let (div, rem) = a.div_mod_floor(&inputs.1.into());
    (biguint_to_fe(&div), biguint_to_fe(&rem))
}

pub fn get_last_bit_ground_truth<F: ScalarField>(input: F) -> F {
    F::from(input.get_lower_32() & 1 == 1)
}
