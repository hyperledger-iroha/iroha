//! Degree algebra, coefficient identities and malformed-graph checks.

use super::reference::{add, degree_bound, horner, mul, product, sub};
use super::*;

#[test]
fn degree_algebra_tracks_zero_constants_and_checked_full_products() {
    let zero = PolynomialDegree::ZERO;
    let constant = PolynomialDegree::constant(7);
    assert_eq!(PolynomialDegree::constant(0), zero);
    assert_eq!(constant.exclusive(), 1);
    let high = PolynomialDegree::from_exclusive(usize::MAX);
    assert_eq!(high.product(zero).unwrap(), zero);
    assert_eq!(high.product(constant).unwrap(), high);
    assert_eq!(constant.product(high).unwrap(), high);
    assert!(high.product(PolynomialDegree::from_exclusive(2)).is_err());
    assert!(PolynomialDegree::from_inclusive(usize::MAX).is_err());
    assert_eq!(PolynomialDegree::from_inclusive(6).unwrap().exclusive(), 7);
    assert_eq!(zero.sum(high), high);
    assert_eq!(
        PolynomialDegree::from_exclusive(3)
            .product(PolynomialDegree::from_exclusive(5))
            .unwrap()
            .exclusive(),
        7
    );
}

#[test]
fn symbolic_graph_keeps_equal_degree_inputs_distinct_and_rejects_bad_topology() {
    let nodes = [
        Node::Input(0),
        Node::Input(1),
        Node::Sub(0, 1),
        Node::Mul(0, 1),
        Node::Constant(0),
        Node::Mul(3, 4),
        Node::Add(2, 3),
    ];
    let degree = PolynomialDegree::from_exclusive(4);
    let values = evaluate_node_degrees(&nodes, &[degree, degree]).unwrap();
    assert_eq!(
        values.iter().map(|x| x.exclusive()).collect::<Vec<_>>(),
        [4, 4, 4, 7, 0, 0, 7]
    );
    assert!(evaluate_node_degrees(&nodes, &[degree]).is_err());
    assert!(evaluate_node_degrees(&[Node::Add(0, 0)], &[]).is_err());
    assert!(evaluate_node_degrees(&[Node::Constant(1), Node::Mul(0, 2)], &[]).is_err());
    assert!(
        evaluate_node_degrees(
            &[Node::Input(0), Node::Mul(0, 0)],
            &[PolynomialDegree::from_exclusive(usize::MAX)]
        )
        .is_err()
    );
}

#[test]
fn graph_bounds_cover_independent_dense_polynomial_products_and_cancellations() {
    for left_degree in 0..6 {
        for right_degree in 0..6 {
            let left = (0..=left_degree).map(|i| 1 + i as u64).collect::<Vec<_>>();
            let right = (0..=right_degree).map(|i| 7 + i as u64).collect::<Vec<_>>();
            let a = PolynomialDegree::from_exclusive(left.len());
            let b = PolynomialDegree::from_exclusive(right.len());
            let multiplied = product(&left, &right);
            assert_eq!(degree_bound(&multiplied), a.product(b).unwrap().exclusive());
            for point in [0, 1, 3, 17] {
                assert_eq!(
                    horner(&multiplied, point),
                    mul(horner(&left, point), horner(&right, point))
                );
            }
            let mut sum = vec![0; left.len().max(right.len())];
            for (i, &value) in left.iter().enumerate() {
                sum[i] = add(sum[i], value);
            }
            for (i, &value) in right.iter().enumerate() {
                sum[i] = sub(sum[i], value);
            }
            assert!(degree_bound(&sum) <= a.sum(b).exclusive());
            let cancellation: Vec<_> = left.iter().map(|&value| sub(value, value)).collect();
            assert_eq!(degree_bound(&cancellation), 0);
        }
    }
}

#[test]
fn conditional_quotients_require_full_divisibility_and_retain_high_coefficients() {
    for n in [1, 2, 4, 8] {
        let quotient = [2, 3, 5, 7, 11];
        let mut vanishing = vec![0; n + 1];
        vanishing[0] = sub(0, 1);
        vanishing[n] = 1;
        let numerator = product(&vanishing, &quotient);
        let bounds = AirDegreeBounds::new(
            n,
            vec![PolynomialDegree::from_exclusive(numerator.len()); SLOT_COUNT],
        )
        .unwrap();
        assert_eq!(bounds.combined_numerator(), n + quotient.len());
        assert_eq!(bounds.conditional_quotients().combined, quotient.len());
        assert!(
            bounds
                .conditional_quotients()
                .slots
                .iter()
                .all(|&x| x == quotient.len())
        );
        assert_eq!(horner(&numerator, 1), 0);
        // Interpolation modulo X^N-1 erases the full numerator, despite matching
        // every subgroup value. Its claimed degree zero is not the true bound.
        let mut alias = vec![0; n];
        for (degree, &coefficient) in numerator.iter().enumerate() {
            alias[degree % n] = add(alias[degree % n], coefficient);
        }
        assert_eq!(degree_bound(&alias), 0);
        assert_ne!(horner(&alias, 3), horner(&numerator, 3));
        let mut not_divisible = numerator.clone();
        not_divisible[0] = add(not_divisible[0], 1);
        assert_ne!(horner(&not_divisible, 1), 0);
        // The same metadata also fits this nondivisible polynomial. It confers
        // no remainder check or acceptance result.
        assert_eq!(degree_bound(&not_divisible), bounds.combined_numerator());
    }
    let low =
        AirDegreeBounds::new(8, vec![PolynomialDegree::from_exclusive(8); SLOT_COUNT]).unwrap();
    assert_eq!(low.conditional_quotients().combined, 0);
    assert!(AirDegreeBounds::new(0, vec![PolynomialDegree::ZERO; SLOT_COUNT]).is_err());
    for count in [SLOT_COUNT - 1, SLOT_COUNT + 1] {
        assert!(AirDegreeBounds::new(8, vec![PolynomialDegree::ZERO; count]).is_err());
    }
}
