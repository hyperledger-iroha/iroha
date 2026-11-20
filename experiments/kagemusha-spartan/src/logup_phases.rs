//! Checks the phase assumptions of semantic LogUp tuples before the compiler
//! rewrites the arrays and introduces its own rational-function helpers.

use super::{Array, Block, Copies, Factor, Lin, Scalar, Src, Term};
use std::collections::HashSet;

pub(super) fn check<F: Scalar>(arrays: &[Array<F>], tables: &[(usize, u32)], copies: &Copies<F>) {
    let declared = copies
        .challenge
        .cells
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        declared.len(),
        copies.challenge.cells.len(),
        "unique second-phase cells"
    );
    for &(array, cell) in &declared {
        assert!(
            arrays.get(array).is_some_and(|a| cell < a.cells()),
            "challenge output must be a declared builder column"
        );
    }
    for id in &copies.link.tables {
        assert!(
            tables.iter().any(|(_, table_id)| table_id == id),
            "unknown link-phase table"
        );
    }

    // The ordinary input digest precedes both challenges. Link IO includes
    // R, which is absorbed later. Semantic tuples may read a committed
    // link-phase witness copy, but cannot read these public slots directly.
    // IO arrays are constrained to one cell in a one-instance global family
    // by build_rows. Their full public span is therefore 2^rho.
    let io_span =
        |a: usize, start: usize| start..start.checked_add(1 << arrays[a].rho).expect("IO extent");
    let link_spans: Vec<_> = copies
        .link
        .io
        .iter()
        .map(|&(a, off)| io_span(a, off))
        .collect();
    let mut late_arrays = HashSet::new();
    for &(array, offset) in copies.io_arrays.iter().chain(&copies.link.io) {
        let span = io_span(array, offset);
        if copies.late_io.iter().any(|entry| span.contains(&entry.0))
            || link_spans
                .iter()
                .any(|other| other.start < span.end && span.start < other.end)
        {
            late_arrays.insert(array);
        }
    }
    let before_challenge = |array: usize, cell: usize| {
        cell < arrays[array].cells()
            && !declared.contains(&(array, cell))
            && !late_arrays.contains(&array)
    };
    let check_tuple = |array: usize, tuple: &[Lin<F>]| {
        for term in tuple.iter().flatten() {
            let target = cell_read(array, &term.src);
            assert!(
                target.is_none_or(|(a, k)| before_challenge(a, k)),
                "semantic LogUp tuple must be fixed before its challenges"
            );
        }
    };
    for (a, array) in arrays.iter().enumerate() {
        for (_, tuple) in &array.lookups {
            check_tuple(a, tuple);
        }
    }
    for export in &copies.exports {
        check_tuple(export.array, &export.tuple);
    }
    for import in &copies.imports {
        check_tuple(import.array, &import.tuple);
    }
    for &(a, _) in tables {
        assert!(
            arrays[a].lookups.is_empty() && !copies.imports.iter().any(|im| im.array == a),
            "semantic LogUp table cannot acquire lookup helpers"
        );
        assert!(
            (0..arrays[a].cells()).all(|k| before_challenge(a, k)),
            "semantic LogUp table must be fixed before its challenges"
        );
    }

    // A challenge row defines exactly one declared cell with unit coefficient.
    // The Horner recurrence also reads that cell at strictly later instances.
    // Thus every such value is determined by backward substitution from the
    // committed coefficients and the challenge.
    let mut defined = HashSet::new();
    for (a, [lhs, scale, rhs]) in &copies.challenge.rows {
        let a = *a;
        let n = 1usize << arrays[a].rho;
        let unit = |(term, factor): &(Term<F>, Factor)| {
            *factor == Factor::One && term.coef.len() == n && term.coef.iter().all(|&v| v == F::ONE)
        };
        assert!(
            scale.len() == 1 && unit(&scale[0]) && scale[0].0.src == Src::One,
            "challenge definition is linear"
        );
        let output_side = if rhs.is_empty() { lhs } else { rhs };
        if !rhs.is_empty() {
            assert_eq!(rhs.len(), 1, "one challenge output");
        }
        let mut outputs = output_side.iter().enumerate().filter_map(|(i, entry)| {
            if let Src::Own(k) = entry.0.src
                && unit(entry)
                && declared.contains(&(a, k))
            {
                Some((i, k))
            } else {
                None
            }
        });
        let (output_at, k) = outputs
            .next()
            .expect("challenge definition has a unit output");
        assert!(
            outputs.next().is_none() && defined.insert((a, k)),
            "unique challenge definition"
        );
        for (i, (term, _)) in lhs.iter().enumerate() {
            if rhs.is_empty() && i == output_at {
                continue;
            }
            if let Some((target, cell)) = cell_read(a, &term.src)
                && !before_challenge(target, cell)
            {
                assert!(
                    target == a
                        && cell == k
                        && matches!(&term.src, Src::Link { map, .. } if map.len() == n &&
                        map.iter().take(arrays[a].r_count).enumerate().all(|(i, j)|
                            j.is_none_or(|j| j > i && j < arrays[a].r_count))),
                    "second-phase dependency must be strictly triangular"
                );
            }
        }
    }
    assert_eq!(
        defined, declared,
        "each second-phase cell has one defining row"
    );
}

/// Admit multiplicity and inverse-helper columns only in the phases where
/// their actual commitment rows are absorbed by the verifier.
pub(super) fn check_committed_columns(
    columns: &[Block],
    present: &[usize],
    second_phase: &std::ops::Range<usize>,
) {
    let [multiplicity, helper] = columns else {
        panic!("LogUp entry must have multiplicity and helper columns");
    };
    for &i in present {
        assert!(
            multiplicity.index(i) < second_phase.start,
            "late LogUp multiplicity"
        );
        assert!(
            second_phase.contains(&helper.index(i)),
            "misplaced LogUp helper"
        );
    }
}

fn cell_read(owner: usize, src: &Src) -> Option<(usize, usize)> {
    match src {
        Src::One => None,
        Src::Own(k) => Some((owner, *k)),
        Src::Link { array, cell, .. } | Src::All { array, cell } => Some((*array, *cell)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Export, Import, constant, link, own};
    use super::*;
    use ff::Field;
    use iroha_pasta::Fp;

    #[test]
    fn multiplicity_and_helper_use_their_committed_rows() {
        let block = |base| Block {
            base,
            lo: 1,
            hi: 3,
            m: 3,
        };
        let phase = 16..32;
        check_committed_columns(&[block(0), block(16)], &[0, 1, 2, 3], &phase);
        for (multiplicity, helper) in [(8, 16), (0, 8), (0, 24)] {
            assert!(
                std::panic::catch_unwind(|| {
                    check_committed_columns(
                        &[block(multiplicity), block(helper)],
                        &[0, 1, 2, 3],
                        &phase,
                    );
                })
                .is_err()
            );
        }
    }

    fn array() -> Array<Fp> {
        Array {
            name: "phase check",
            family: 0,
            rho: 1,
            r_count: 2,
            fixed: false,
            lookups: vec![],
            widths: vec![255, 255],
            rows: vec![],
            values: vec![vec![Fp::ZERO; 2]; 2],
        }
    }

    fn recurrence(step: Vec<Option<usize>>) -> Copies<Fp> {
        let mut c = Copies::default();
        c.challenge.cells.push((0, 1));
        c.challenge.rows.push((
            0,
            [
                vec![
                    (own(1, 2).remove(0), Factor::One),
                    (link(0, 1, step, Fp::ONE).remove(0), Factor::NegBeta(1)),
                ],
                vec![(constant(vec![Fp::ONE; 2]).remove(0), Factor::One)],
                vec![],
            ],
        ));
        c
    }

    #[test]
    fn horner_reads_a_determined_later_cell() {
        check(&[array()], &[], &recurrence(vec![Some(1), None]));
    }

    #[test]
    #[should_panic(expected = "strictly triangular")]
    fn cyclic_challenge_definition_is_refused() {
        check(&[array()], &[], &recurrence(vec![Some(0), None]));
    }

    #[test]
    fn challenge_definitions_bind_only_live_outputs_and_forward_reads() {
        let definition = || {
            let mut c = Copies::default();
            c.challenge.cells.push((0, 1));
            c.challenge.rows.push((
                0,
                [
                    vec![(own(0, 2).remove(0), Factor::NegBeta(1))],
                    vec![(constant(vec![Fp::ONE; 2]).remove(0), Factor::One)],
                    vec![(own(1, 2).remove(0), Factor::One)],
                ],
            ));
            c
        };
        check(&[array()], &[], &definition());
        for (case, expected) in [
            "challenge output must be a declared builder column",
            "unique challenge definition",
            "each second-phase cell has one defining row",
            "challenge definition is linear",
            "strictly triangular",
            "strictly triangular",
        ]
        .into_iter()
        .enumerate()
        {
            let mut c = definition();
            match case {
                0 => c.challenge.cells[0].1 = 2,
                1 => c.challenge.rows.push(c.challenge.rows[0].clone()),
                2 => c.challenge.rows.clear(),
                3 => c.challenge.rows[0].1[1][0].1 = Factor::NegBeta(1),
                4 => c = recurrence(vec![None, Some(0)]),
                _ => c = recurrence(vec![Some(2), None]),
            }
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                check(&[array()], &[], &c)
            }))
            .expect_err("invalid definition");
            let message = failure
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| failure.downcast_ref::<&str>().copied())
                .unwrap();
            assert!(message.contains(expected), "{case}: {message}");
        }
    }

    #[test]
    #[should_panic(expected = "semantic LogUp table cannot acquire lookup helpers")]
    fn a_table_cannot_extend_its_own_arity_with_lookup_helpers() {
        let mut a = array();
        a.lookups.push((1, vec![own(0, 2)]));
        check(&[a], &[(0, 1)], &Copies::default());
    }

    #[test]
    fn adaptive_cells_are_refused_at_each_tuple_boundary() {
        for case in 0..7 {
            let mut a = array();
            let mut extra = Vec::new();
            let mut c = recurrence(vec![Some(1), None]);
            let mut tables = vec![];
            match case {
                0 => a.lookups.push((1, vec![own(1, 2)])),
                1 => c.exports.push(Export {
                    array: 0,
                    tuple: vec![own(1, 2)],
                }),
                2 => c.imports.push(Import {
                    array: 0,
                    tuple: vec![own(1, 2)],
                    from: vec![None; 2],
                }),
                3 => tables.push((0, 1)),
                4 => {
                    c.io_arrays.push((0, 16));
                    c.late_io.push((17, 0, 1, 1));
                    a.lookups.push((1, vec![own(0, 2)]));
                }
                5 => {
                    extra.push(array());
                    c.io_arrays.push((0, 16));
                    c.link.io.push((1, 17));
                    a.lookups.push((1, vec![own(0, 2)]));
                }
                _ => {
                    extra.push(array());
                    a.lookups.push((
                        1,
                        vec![vec![Term {
                            src: Src::All { array: 1, cell: 0 },
                            coef: vec![Fp::ONE; 2],
                        }]],
                    ));
                    c.link.io.push((1, 20));
                }
            }
            let arrays: Vec<_> = std::iter::once(a).chain(extra).collect();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                check(&arrays, &tables, &c)
            }));
            assert!(result.is_err(), "tuple boundary {case}");
        }
    }
}
