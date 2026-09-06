use super::Expression;
use ff::Field;

/// This describes a selector and where it is activated.
#[derive(Debug, Clone)]
pub struct SelectorDescription {
    /// The selector that this description references, by index.
    pub selector: usize,

    /// The vector of booleans defining which rows are active for this selector.
    pub activations: Vec<bool>,

    /// The maximum degree of a gate involving this selector, including the
    /// virtual selector itself. This means this will be at least 1 for any
    /// expression containing a simple selector, even if that selector is not
    /// multiplied by anything.
    pub max_degree: usize,
}

/// This describes the assigned combination of a particular selector as well as
/// the expression it should be substituted with.
#[derive(Debug, Clone)]
pub struct SelectorAssignment<F> {
    /// The selector that this structure references, by index.
    pub selector: usize,

    /// The combination this selector was assigned to
    pub combination_index: usize,

    /// The expression we wish to substitute with
    pub expression: Expression<F>,
}

/// Computes the exact deterministic selector-combination plan without materializing any field
/// polynomials. Each inner vector contains indices into `selectors`.
fn plan(selectors: &[SelectorDescription], max_degree: usize) -> Vec<Vec<usize>> {
    if selectors.is_empty() {
        return vec![];
    }

    // The length of all provided selectors must be the same.
    let n = selectors[0].activations.len();
    assert!(
        selectors
            .iter()
            .all(|selector| selector.activations.len() == n)
    );

    // Complex selectors (and unused selectors) cannot participate in a combination.
    let mut combinations = selectors
        .iter()
        .enumerate()
        .filter_map(|(index, selector)| (selector.max_degree == 0).then_some(vec![index]))
        .collect::<Vec<_>>();
    let simple = selectors
        .iter()
        .enumerate()
        .filter_map(|(index, selector)| (selector.max_degree != 0).then_some(index))
        .collect::<Vec<_>>();

    // Compute the lower-triangular exclusion matrix for simple selectors. Two selectors that are
    // active on the same row cannot share a fixed column.
    let mut exclusion_matrix = (0..simple.len())
        .map(|i| vec![false; i])
        .collect::<Vec<_>>();
    for (i, &selector_index) in simple.iter().enumerate() {
        let rows = &selectors[selector_index].activations;
        for (j, &other_index) in simple.iter().enumerate().take(i) {
            if rows
                .iter()
                .zip(selectors[other_index].activations.iter())
                .any(|(left, right)| left & right)
            {
                exclusion_matrix[i][j] = true;
            }
        }
    }

    let mut added = vec![false; simple.len()];
    for (i, &selector_index) in simple.iter().enumerate() {
        if added[i] {
            continue;
        }
        added[i] = true;
        let selector = &selectors[selector_index];
        assert!(selector.max_degree <= max_degree);
        // Omit the virtual selector's own degree; it is replaced by the combination expression.
        let mut d = selector.max_degree - 1;
        let mut combination = vec![selector_index];
        let mut combination_added = vec![i];

        'try_selectors: for (j, &candidate_index) in simple.iter().enumerate().skip(i + 1) {
            if d + combination.len() == max_degree {
                break 'try_selectors;
            }
            if added[j] {
                continue 'try_selectors;
            }
            for &member in &combination_added {
                if exclusion_matrix[j][member] {
                    continue 'try_selectors;
                }
            }

            let candidate = &selectors[candidate_index];
            let new_d = std::cmp::max(d, candidate.max_degree - 1);
            if new_d + combination.len() + 1 > max_degree {
                continue 'try_selectors;
            }

            d = new_d;
            combination.push(candidate_index);
            combination_added.push(j);
            added[j] = true;
        }
        combinations.push(combination);
    }

    combinations
}

/// Returns the exact number of fixed columns that [`process`] will materialize.
///
/// Unlike [`process`], this retains selector assignments in their bit-packed representation and
/// therefore lets callers enforce resource limits before allocating degree-sized field vectors.
pub fn combination_count(selectors: &[SelectorDescription], max_degree: usize) -> usize {
    plan(selectors, max_degree).len()
}

/// This function takes a vector that defines each selector as well as a closure
/// used to allocate new fixed columns, and returns the assignment of each
/// combination as well as details about each selector assignment.
///
/// This function takes
/// * `selectors`, a vector of `SelectorDescription`s that describe each
///   selector
/// * `max_degree`, the maximum allowed degree of any gate
/// * `allocate_fixed_columns`, a closure that constructs a new fixed column and
///   queries it at Rotation::cur(), returning the expression
///
/// and returns `Vec<Vec<F>>` containing the assignment of each new fixed column
/// (which each correspond to a combination) as well as a vector of
/// `SelectorAssignment` that the caller can use to perform the necessary
/// substitutions to the constraint system.
///
/// This function is completely deterministic.
pub fn process<F: Field, E>(
    selectors: Vec<SelectorDescription>,
    max_degree: usize,
    mut allocate_fixed_column: E,
) -> (Vec<Vec<F>>, Vec<SelectorAssignment<F>>)
where
    E: FnMut() -> Expression<F>,
{
    let combinations = plan(&selectors, max_degree);
    let n = selectors
        .first()
        .map_or(0, |selector| selector.activations.len());
    let mut combination_assignments = vec![];
    let mut selector_assignments = vec![];
    for combination in combinations {
        // Now, compute the selector and combination assignments.
        let mut combination_assignment = vec![F::ZERO; n];
        let combination_len = combination.len();
        let combination_index = combination_assignments.len();
        let query = allocate_fixed_column();

        let mut assigned_root = F::ONE;
        selector_assignments.extend(combination.into_iter().map(|selector_index| {
            let selector = &selectors[selector_index];
            // Compute the expression for substitution. This produces an expression of the
            // form
            //     q * Prod[i = 1..=combination_len, i != assigned_root](i - q)
            //
            // which is non-zero only on rows where `combination_assignment` is set to
            // `assigned_root`. In particular, rows set to 0 correspond to all selectors
            // being disabled.
            let mut expression = query.clone();
            let mut root = F::ONE;
            for _ in 0..combination_len {
                if root != assigned_root {
                    expression = expression * (Expression::Constant(root) - query.clone());
                }
                root += F::ONE;
            }

            // Update the combination assignment
            for (combination, selector) in combination_assignment
                .iter_mut()
                .zip(selector.activations.iter())
            {
                // This will not overwrite another selector's activations because
                // we have ensured that selectors are disjoint.
                if *selector {
                    *combination = assigned_root;
                }
            }

            assigned_root += F::ONE;

            SelectorAssignment {
                selector: selector.selector,
                combination_index,
                expression,
            }
        }));
        combination_assignments.push(combination_assignment);
    }

    (combination_assignments, selector_assignments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{plonk::FixedQuery, poly::Rotation};
    use halo2curves::pasta::Fp;
    use proptest::collection::{SizeRange, vec};
    use proptest::prelude::*;

    prop_compose! {
        fn arb_selector(assignment_size: usize, max_degree: usize)
                       (degree in 0..max_degree,
                        assignment in vec(any::<bool>(), assignment_size))
                       -> (usize, Vec<bool>) {
            (degree, assignment)
        }
    }

    prop_compose! {
        fn arb_selector_list(assignment_size: usize, max_degree: usize, num_selectors: impl Into<SizeRange>)
                            (list in vec(arb_selector(assignment_size, max_degree), num_selectors))
                            -> Vec<SelectorDescription>
        {
            list.into_iter().enumerate().map(|(i, (max_degree, activations))| {
                SelectorDescription {
                    selector: i,
                    activations,
                    max_degree,
                }
            }).collect()
        }
    }

    prop_compose! {
        fn arb_instance(max_assignment_size: usize,
                        max_degree: usize,
                        max_selectors: usize)
                       (assignment_size in 1..max_assignment_size,
                        degree in 1..max_degree,
                        num_selectors in 1..max_selectors)
                       (list in arb_selector_list(assignment_size, degree, num_selectors),
                        degree in Just(degree))
                       -> (Vec<SelectorDescription>, usize)
        {
            (list, degree)
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]
        #[test]
        fn test_selector_combination((selectors, max_degree) in arb_instance(10, 10, 15)) {
            let mut query = 0;
            let (combination_assignments, selector_assignments) =
                process::<Fp, _>(selectors.clone(), max_degree, || {
                    let tmp = Expression::Fixed(FixedQuery {
                        index: Some(query),
                        column_index: query,
                        rotation: Rotation::cur(),
                    });
                    query += 1;
                    tmp
                });
            assert_eq!(
                combination_count(&selectors, max_degree),
                combination_assignments.len(),
                "count-only planning must exactly match materialization"
            );

            {
                let mut selectors_seen = vec![];
                assert_eq!(selectors.len(), selector_assignments.len());
                for selector in &selector_assignments {
                    // Every selector should be assigned to a combination
                    assert!(selector.combination_index < combination_assignments.len());
                    assert!(!selectors_seen.contains(&selector.selector));
                    selectors_seen.push(selector.selector);
                }
            }

            // Test that, for each selector, the provided expression
            //  1. evaluates to zero on rows where the selector's activation is off
            //  2. evaluates to nonzero on rows where the selector's activation is on
            //  3. is of degree d such that d + (selector.max_degree - 1) <= max_degree
            //     OR selector.max_degree is zero
            for selector in selector_assignments {
                assert_eq!(
                    selectors[selector.selector].activations.len(),
                    combination_assignments[selector.combination_index].len()
                );
                for (&activation, &assignment) in selectors[selector.selector]
                    .activations
                    .iter()
                    .zip(combination_assignments[selector.combination_index].iter())
                {
                    let eval = selector.expression.evaluate(
                        &|c| c,
                        &|_| panic!("should not occur in returned expressions"),
                        &|query| {
                            // Should be the correct combination in the expression
                            assert_eq!(selector.combination_index, query.index.unwrap());
                            assignment
                        },
                        &|_| panic!("should not occur in returned expressions"),
                        &|_| panic!("should not occur in returned expressions"),
                        &|_| panic!("should not occur in returned expressions"),
                        &|a| -a,
                        &|a, b| a + b,
                        &|a, b| a * b,
                        &|a, f| a * f,
                    );

                    if activation {
                        assert!(!eval.is_zero_vartime());
                    } else {
                        assert!(eval.is_zero_vartime());
                    }
                }

                let expr_degree = selector.expression.degree();
                assert!(expr_degree <= max_degree);
                if selectors[selector.selector].max_degree > 0 {
                    assert!(
                        (selectors[selector.selector].max_degree - 1) + expr_degree <= max_degree
                    );
                }
            }
        }
    }
}
