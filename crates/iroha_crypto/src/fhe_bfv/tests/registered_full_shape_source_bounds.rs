//! Full registered RNS-shape boundary checks for centered BFV product sums.

use super::*;

#[test]
fn eight_limb_source_rejects_late_out_of_bound_and_cancelling_products() {
    let params = ram_lfe_bfv_parameters_v1();
    let source_chain = registered_bfv_centered_scale_round_source_chain(&params)
        .expect("registered centered source chain");
    let evaluator_chain =
        registered_bfv_rns_modulus_chain(&params).expect("registered evaluator chain");
    assert_eq!(params.degree(), 64);
    assert_eq!(source_chain.moduli.len(), 8);
    assert_eq!(evaluator_chain.moduli.len(), 8);

    let product = source_chain.product().expect("eight-limb source product");
    let bound = exact_ciphertext_modulus_negacyclic_product_sum_abs_bound(&params, 1)
        .expect("one-product centered bound");
    assert!(bound + 1 < product / 2);
    let last = params.degree() - 1;
    let at_last_coefficient = |coefficient: u128| BfvRnsPolynomial {
        residues_by_limb: source_chain
            .moduli
            .iter()
            .map(|&modulus| {
                let mut limb = vec![0_u64; params.degree()];
                limb[last] = u64::try_from(coefficient % u128::from(modulus))
                    .expect("canonical residue fits a limb");
                limb
            })
            .collect(),
    };
    let zero = at_last_coefficient(0);

    for coefficient in [bound, product - bound] {
        let boundary = at_last_coefficient(coefficient);
        let direct = source_chain
            .scale_round_add_centered_product_polynomials_exact(&params, &boundary, &zero)
            .expect("exact centered boundary is admitted");
        let target = source_chain
            .scale_round_add_centered_product_polynomials_target_limbs_exact(
                &params,
                &boundary,
                &zero,
                &evaluator_chain,
            )
            .expect("target-limb path admits the same exact centered boundary");
        assert_eq!(target, direct);
    }

    let past_positive = at_last_coefficient(bound + 1);
    let past_negative = at_last_coefficient(product - (bound + 1));
    for (label, left, right, expected_leg) in [
        ("positive", &past_positive, &zero, "left product"),
        ("negative", &past_negative, &zero, "left product"),
        ("right", &zero, &past_positive, "right product"),
        ("cancelling", &past_positive, &past_negative, "left product"),
    ] {
        for result in [
            source_chain.scale_round_add_centered_product_polynomials_exact(&params, left, right),
            source_chain.scale_round_add_centered_product_polynomials_target_limbs_exact(
                &params,
                left,
                right,
                &evaluator_chain,
            ),
        ] {
            let error = result.expect_err("out-of-bound source product must be rejected");
            assert!(
                error.to_string().contains(&format!(
                    "{expected_leg} coefficient[{last}] exceeds source-chain centered bound"
                )),
                "{label}: {error}"
            );
        }
    }
}

#[test]
fn eight_limb_operand_product_rejects_late_eighth_limb_mismatch() {
    let params = ram_lfe_bfv_parameters_v1();
    let source_chain = registered_bfv_centered_scale_round_source_chain(&params)
        .expect("registered centered source chain");
    let target_chain =
        registered_bfv_rns_modulus_chain(&params).expect("registered evaluator target chain");
    assert_eq!(params.degree(), 64);
    assert_eq!(source_chain.moduli.len(), 8);
    assert_eq!(target_chain.moduli.len(), 8);

    let last = params.degree() - 1;
    let mut left_operand = vec![0_u64; params.degree()];
    left_operand[last] = params.ciphertext_modulus - 1;
    let mut right_operand = vec![0_u64; params.degree()];
    right_operand[0] = params.ciphertext_modulus - 1;
    let left_rns = source_chain
        .decompose_centered_ciphertext_modulus_polynomial(&params, &left_operand)
        .expect("centered left operand");
    let right_rns = source_chain
        .decompose_centered_ciphertext_modulus_polynomial(&params, &right_operand)
        .expect("centered right operand");
    let product = source_chain
        .multiply_rns_polynomials_negacyclic(&params, &left_rns, &right_rns)
        .expect("source-chain product of two centered minus-one operands");
    let reconstructed = source_chain
        .reconstruct_polynomial(&params, &product)
        .expect("reconstruct operand-derived product");
    assert!(
        reconstructed[..last]
            .iter()
            .all(|&coefficient| coefficient == 0)
    );
    assert_eq!(reconstructed[last], 1);

    let direct = source_chain
        .scale_round_add_centered_product_polynomials_exact(&params, &product, &product)
        .expect("valid operand-derived products pass source admission");
    let target = source_chain
        .scale_round_add_centered_product_polynomials_target_limbs_exact(
            &params,
            &product,
            &product,
            &target_chain,
        )
        .expect("valid operand-derived products pass target-limb admission");
    assert_eq!(target, direct);

    let mut corrupted_second_product = product.clone();
    let eighth_limb = source_chain.moduli.len() - 1;
    corrupted_second_product.residues_by_limb[eighth_limb][last] += 1;
    assert!(
        corrupted_second_product.residues_by_limb[eighth_limb][last]
            < source_chain.moduli[eighth_limb]
    );
    let corrupted_coefficient = source_chain
        .reconstruct_polynomial(&params, &corrupted_second_product)
        .expect("malformed product retains canonical residue shape")[last];
    let source_product = source_chain.product().expect("source-chain product");
    let corrupted_centered_abs = corrupted_coefficient.min(source_product - corrupted_coefficient);
    let one_product_bound = exact_ciphertext_modulus_negacyclic_product_sum_abs_bound(&params, 1)
        .expect("one-product centered bound");
    assert!(corrupted_centered_abs > one_product_bound);

    for result in [
        source_chain.scale_round_add_centered_product_polynomials_exact(
            &params,
            &product,
            &corrupted_second_product,
        ),
        source_chain.scale_round_add_centered_product_polynomials_target_limbs_exact(
            &params,
            &product,
            &corrupted_second_product,
            &target_chain,
        ),
    ] {
        let error = result.expect_err("late eighth-limb mismatch must fail source admission");
        assert!(
            error.to_string().contains(&format!(
                "right product coefficient[{last}] exceeds source-chain centered bound"
            )),
            "{error}"
        );
    }
}
