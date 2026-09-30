//! Clearing owners for private replay, quotient and composition allocations.

use super::*;
use zeroize::Zeroize;

#[test]
fn main_der_source_erasure_clears_live_base_aux_and_private_geometry_cells() {
    use super::super::super::der_stark::ZkX509DerStarkPrivateShapeV1;
    let mut trace = ZkX509DerStarkTraceV1 {
        base: ZkX509DerStarkBaseV1 {
            private_shape: ZkX509DerStarkPrivateShapeV1 {
                document_lengths: vec![7, 11],
                parser_rows: 13,
                comparator_rows: 17,
            },
            rows: vec![[F(19); ZK_X509_DER_STARK_BASE_WIDTH_V1]; 3],
        },
        aux_rows: vec![[F(23); ZK_X509_DER_STARK_AUX_WIDTH_V1]; 5],
    };
    zeroize_main_der_trace_cells_v1(&mut trace);
    assert_eq!(trace.base.private_shape.document_lengths, [0, 0]);
    assert_eq!(trace.base.private_shape.parser_rows, 0);
    assert_eq!(trace.base.private_shape.comparator_rows, 0);
    assert_eq!(trace.base.rows.len(), 3);
    assert_eq!(trace.aux_rows.len(), 5);
    assert!(
        trace
            .base
            .rows
            .iter()
            .flatten()
            .all(|value| *value == F::ZERO)
    );
    assert!(
        trace
            .aux_rows
            .iter()
            .flatten()
            .all(|value| *value == F::ZERO)
    );
    zeroize_main_der_trace_v1(&mut trace);
    assert!(trace.base.private_is_zeroized_v1());
    assert!(trace.aux_rows.is_empty());
}

#[test]
fn main_private_buffer_owners_clear_every_base_and_extension_coordinate() {
    let private = E::from_coefficients([F(11), F(13), F(17), F(19)]).unwrap();
    let mut base = ZeroizingBaseColumnsV1(vec![vec![F(3); 7], vec![F(5); 11]]);
    base.zeroize();
    assert_eq!(base.iter().map(Vec::len).collect::<Vec<_>>(), [7, 11]);
    assert!(base.iter().flatten().all(|value| *value == F::ZERO));
    let mut quotient = ZeroizingExtensionColumnV1(vec![private; 9]);
    quotient.zeroize();
    assert_eq!(quotient.len(), 9);
    assert!(
        quotient
            .iter()
            .all(|value| value.coefficients() == [F::ZERO; 4])
    );
    let mut composition = RetainedCompositionMaterialV1 {
        evaluations: vec![vec![vec![private; 7], vec![private; 13]]],
        coefficient_chunks: vec![vec![vec![private; 3], vec![private; 5]]],
    };
    composition.zeroize();
    for columns in [&composition.evaluations, &composition.coefficient_chunks] {
        assert!(
            columns
                .iter()
                .flatten()
                .flatten()
                .all(|value| value.coefficients() == [F::ZERO; 4])
        );
    }
}

#[test]
fn replay_column_erasure_and_consuming_handoff_have_distinct_ownership() {
    let mut column = ZeroizingMainTraceColumnV1(vec![F(23); 31]);
    column.zeroize_private_v1();
    assert!(column.is_empty());
    // A successful handoff transfers the original allocation into the next
    // clearing owner; the emptied predecessor must not clear that live owner.
    let original = ZeroizingMainTraceColumnV1(vec![F(29); 17]);
    let address = original.as_ptr();
    let mut successor = ZeroizingBaseColumnsV1(vec![original.into_vec_v1()]);
    assert_eq!(successor[0].as_ptr(), address);
    assert_eq!(successor[0], vec![F(29); 17]);
    successor.zeroize();
    assert_eq!(successor[0], vec![F::ZERO; 17]);
}
