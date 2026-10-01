//! Actual maximum-credential native boundary checks for all MAIN registrations.

use super::*;
use crate::privacy_engines::zk_x509::{
    der_stark::{
        ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1, zk_x509_der_stark_aggregate_aux_row_v1,
        zk_x509_der_stark_aggregate_base_row_v1,
    },
    main_assembly::build_zk_x509_main_trace_assembly_v1,
    p256_aggregate_adapter::P256_INPUT_SELECTION_BYTES_V1,
    p256_air::P256_ARITHMETIC_ROWS_PER_OPERATION_V1,
    p256_external_binding_air::p256_external_binding_rows_v1,
    p256_reduction_air::P256_REDUCTION_ROWS_V1,
    p256_scalar_bit_bus::P256_SCALAR_BIT_BUS_ROWS_V1,
    p256_trace::compile_p256_ecdsa_topology_v1,
    p256_window_air::{
        P256_WINDOW_BATCH_INSTANCES_V1, P256_WINDOW_ROWS_V1, P256_WINDOW_STARK_TRACE_SIZE_V1,
    },
    projection_air::ZkX509ProjectionFixedRowV1,
    relation::{
        ZkX509GovernanceV1,
        release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
    },
    rfc5280_stark::build_zk_x509_rfc5280_stark_shape_v1,
    sha_call_bus_stark::ZkX509ShaSegmentReplayV1,
};
use std::collections::BTreeSet;

/// Retain only boundary openings. The existing column owner clears both the
/// sampled witness rows and transient full columns on normal return or unwind.
struct NativeBoundaryRows {
    indices: Vec<usize>,
    base: Vec<ZeroizingMainTraceColumnV1>,
    aux: Vec<ZeroizingMainTraceColumnV1>,
    fixed: Vec<Vec<F>>,
}

impl NativeBoundaryRows {
    fn new(registration: RegisteredSegmentLayoutV1, edges: &BTreeSet<usize>) -> Self {
        let size = registration.segment.trace_size();
        let indices = edges
            .iter()
            .flat_map(|&row| [row, (row + 1) % size])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let rows = |width| {
            indices
                .iter()
                .map(|_| ZeroizingMainTraceColumnV1(vec![F::ZERO; width]))
                .collect()
        };
        Self {
            base: rows(registration.segment.base_width),
            aux: rows(registration.segment.aux_width),
            fixed: vec![vec![F::ZERO; registration.segment.fixed_width]; indices.len()],
            indices,
        }
    }

    fn replay_columns(
        &mut self,
        layout: &AggregateProofLayoutV1,
        registration: RegisteredSegmentLayoutV1,
        sources: &MainTraceReplaySourcesV1<'_, '_>,
        kind: MainTraceColumnKindV1,
    ) {
        let (start, width, rows) = match kind {
            MainTraceColumnKindV1::Base => (
                registration.base_start,
                registration.segment.base_width,
                &mut self.base,
            ),
            MainTraceColumnKindV1::Aux => (
                registration.aux_start,
                registration.segment.aux_width,
                &mut self.aux,
            ),
        };
        for local in (0..width).step_by(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            let end = width.min(local + aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1);
            let columns = sources
                .native_columns_v1(
                    layout,
                    kind,
                    registration.trace_group,
                    start + local..start + end,
                )
                .expect("bounded native source replay");
            assert_eq!(columns.len(), end - local);
            assert_eq!(
                columns.capacity(),
                end - local,
                "real source batch and final tail capacity"
            );
            assert!(columns.len() <= 8);
            for (offset, column) in columns.iter().enumerate() {
                assert_eq!(column.len(), registration.segment.trace_size());
                assert_eq!(
                    column.0.capacity(),
                    registration.segment.trace_size(),
                    "all registered native builders must meet replay admission"
                );
                for (&index, row) in self.indices.iter().zip(rows.iter_mut()) {
                    row[local + offset] = column[index];
                }
            }
        }
    }
}

/// Include both sides of each logical endpoint plus the final physical padding
/// edge and cyclic edge. A full native domain still gets its actual final edge.
fn add_native_boundary(edges: &mut BTreeSet<usize>, end: usize, size: usize) {
    assert!(end > 0 && end <= size);
    edges.insert(end - 1);
    if end < size {
        edges.insert(end);
    }
}

fn p256_native_boundaries(
    registration: P256MainRegistrationV1,
    fixed: &P256MainVerifierFixedSourceV1,
    edges: &mut BTreeSet<usize>,
) {
    let size = registration.shape_v1().unwrap().trace_size;
    match registration.adapter_v1() {
        P256MainAdapterV1::ValueBus => {
            // The first factor's verifier-owned active selector is a prefix.
            // Binary search its endpoint without guessing a witness length.
            let mut low = 0;
            let mut high = size;
            assert_eq!(fixed.fixed_row_v1(registration, 0).unwrap()[0], F::ONE);
            while low < high {
                let middle = low + (high - low) / 2;
                if fixed.fixed_row_v1(registration, middle).unwrap()[0] == F::ONE {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            assert_eq!(
                fixed.fixed_row_v1(registration, low - 1).unwrap()[0],
                F::ONE
            );
            if low < size {
                assert_eq!(fixed.fixed_row_v1(registration, low).unwrap()[0], F::ZERO);
            }
            add_native_boundary(edges, low, size);
        }
        P256MainAdapterV1::Arithmetic => {
            let topology = compile_p256_ecdsa_topology_v1(registration.role_v1()).unwrap();
            add_native_boundary(
                edges,
                topology.linked_operations.len() * P256_ARITHMETIC_ROWS_PER_OPERATION_V1,
                size,
            );
        }
        P256MainAdapterV1::WindowBatch => {
            // Every 512-row block has its own 272-row logical trace and padding.
            for window in 0..P256_WINDOW_BATCH_INSTANCES_V1 {
                let start = window * P256_WINDOW_STARK_TRACE_SIZE_V1;
                edges.insert(start);
                add_native_boundary(edges, start + P256_WINDOW_ROWS_V1, size);
                add_native_boundary(edges, start + P256_WINDOW_STARK_TRACE_SIZE_V1, size);
            }
        }
        P256MainAdapterV1::Reduction | P256MainAdapterV1::WalletLowS => {
            add_native_boundary(edges, P256_REDUCTION_ROWS_V1, size);
        }
        P256MainAdapterV1::BindingSink => {
            add_native_boundary(
                edges,
                p256_external_binding_rows_v1(registration.role_v1()),
                size,
            );
            add_native_boundary(edges, P256_INPUT_SELECTION_BYTES_V1, size);
        }
        P256MainAdapterV1::ScalarBitBus => {
            add_native_boundary(edges, P256_SCALAR_BIT_BUS_ROWS_V1, size);
        }
    }
}

#[test]
#[ignore = "replays the actual maximum credential through all 49 native registrations"]
#[allow(clippy::too_many_lines)]
fn maximum_credential_all_49_native_registration_boundaries_have_zero_residues() {
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
        .expect("maximum signed credential");
    let trust_anchor = fixture.authoritative_state.trust_anchor();
    let crl = fixture.authoritative_state.crl_record();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .expect("actual maximum MAIN assembly");
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    assert_eq!(layout.registered_segments.len(), 49);
    let digest = |seed| PrivacyOuterDigestV1::from_bytes([seed; 48]);
    let pre_aux = ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
        [0x81; 32],
        assembly.verifier_profile.compiled_profile_digest,
        core::array::from_fn(|index| digest(index as u8 + 1)),
    );
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        pre_aux,
        digest(0x91),
        digest(0xA1),
        digest(0xB1),
    )
    .unwrap();
    let sha = core::array::from_fn(|segment| {
        ZkX509ShaBatchSegmentBaseSourceV1::new_v1(
            &assembly.sha_schedule,
            &assembly.sha_witnesses,
            segment,
        )
        .unwrap()
    });
    let p256 = P256MainBaseSourceV1::new_v1(&assembly).unwrap();
    let mut source = MainLog19BoundTraceGroupSourceV1::bind_from_phase_v1(
        &layout, &assembly, sha, p256, binding,
    )
    .unwrap();
    let mut projection = MainProjectionTraceGroupSourceV1::for_main_v1(
        &layout,
        &fixture.statement,
        &assembly.projection_trace,
    )
    .unwrap();
    projection
        .bind_challenges_v1(binding.main_post_base())
        .unwrap();
    let mut io =
        MainIoTraceGroupSourceV1::for_main_v1(&layout, &fixture.statement, &assembly.io).unwrap();
    io.bind_challenges_v1(binding.main_post_base()).unwrap();
    let p256_fixed = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let p256_challenges = p256_aggregate_challenges_from_post_base_v1(source.post_base).unwrap();
    let p256_terminals = main_p256_terminal_registrations_v1(&source.claims.p256).unwrap();
    let projection_fixed =
        compile_zk_x509_projection_stark_fixed_rows_v1(&fixture.statement).unwrap();
    let rfc_counts = build_zk_x509_rfc5280_stark_shape_v1(&assembly.rfc_trace)
        .unwrap()
        .family_counts();
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut registration_ids = BTreeSet::new();
    let mut checked_edges = 0;
    for registration in layout.registered_segments.iter().copied() {
        let size = registration.segment.trace_size();
        let mut edges = BTreeSet::from([0, size - 2, size - 1]);
        let p256_registration = match registration.segment.adapter {
            SegmentAdapterIdV1::Projection => {
                let end = assembly
                    .projection_trace
                    .fixed
                    .rows
                    .iter()
                    .position(|row| matches!(row, ZkX509ProjectionFixedRowV1::Padding))
                    .unwrap();
                add_native_boundary(&mut edges, end, size);
                None
            }
            SegmentAdapterIdV1::ByteMemory => {
                add_native_boundary(&mut edges, assembly.io.logical_active_rows, size);
                None
            }
            SegmentAdapterIdV1::StrictDer => {
                add_native_boundary(&mut edges, source.der_fixed.active_rows(), size);
                add_native_boundary(&mut edges, source.der.base.private_shape.parser_rows, size);
                add_native_boundary(&mut edges, ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1, size);
                add_native_boundary(
                    &mut edges,
                    ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1
                        + source.der.base.private_shape.comparator_rows,
                    size,
                );
                None
            }
            SegmentAdapterIdV1::Rfc5280 => {
                let mut end = 0;
                for count in rfc_counts {
                    end += count;
                    add_native_boundary(&mut edges, end, size);
                }
                assert_eq!(end, size);
                None
            }
            SegmentAdapterIdV1::Sha256CallBus => {
                let replay =
                    ZkX509ShaSegmentReplayV1::new(usize::from(registration.segment.instance))
                        .unwrap();
                add_native_boundary(&mut edges, replay.active_rows(), size);
                None
            }
            _ => {
                let p256 = p256_main_registration_from_main_layout_v1(registration).unwrap();
                p256_native_boundaries(p256, &p256_fixed, &mut edges);
                Some(p256)
            }
        };
        assert!(registration_ids.insert((registration.trace_group, registration.base_start)));
        let mut rows = NativeBoundaryRows::new(registration, &edges);
        if registration.segment.adapter == SegmentAdapterIdV1::Sha256CallBus {
            let segment = usize::from(registration.segment.instance);
            let base = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(
                &assembly.sha_schedule,
                &assembly.sha_witnesses,
                segment,
            )
            .unwrap();
            let mut base_seen = vec![0; rows.indices.len()];
            base.for_each_base_fixed_row_v1(|index, base, fixed| {
                if let Ok(slot) = rows.indices.binary_search(&index) {
                    rows.base[slot].copy_from_slice(&base);
                    rows.fixed[slot].copy_from_slice(&fixed);
                    assert_eq!(
                        fixed,
                        source.sha_fixed.fixed_row_v1(segment, index).unwrap()
                    );
                    base_seen[slot] += 1;
                }
            })
            .unwrap();
            let mut aux_seen = vec![0; rows.indices.len()];
            let terminals = source.sha_aux[segment]
                .for_each_aux_row_with_air_terminals_v1(|index, aux| {
                    if let Ok(slot) = rows.indices.binary_search(&index) {
                        rows.aux[slot].copy_from_slice(&aux);
                        aux_seen[slot] += 1;
                    }
                })
                .unwrap();
            assert_eq!(terminals.segment, source.claims.sha.segments[segment]);
            assert!(base_seen.iter().all(|count| *count == 1));
            assert!(aux_seen.iter().all(|count| *count == 1));
        } else if let Some(p256) = p256_registration {
            let mut base_seen = vec![0; rows.indices.len()];
            let mut aux_seen = vec![0; rows.indices.len()];
            source
                .p256
                .visit_native_boundary_rows_for_test_v1(
                    p256,
                    &rows.indices,
                    |index, base| {
                        let slot = rows.indices.binary_search(&index).unwrap();
                        assert_eq!(base.len(), registration.segment.base_width);
                        rows.base[slot].copy_from_slice(base);
                        base_seen[slot] += 1;
                    },
                    |index, aux| {
                        let slot = rows.indices.binary_search(&index).unwrap();
                        assert_eq!(aux.len(), registration.segment.aux_width);
                        rows.aux[slot].copy_from_slice(aux);
                        aux_seen[slot] += 1;
                    },
                )
                .unwrap();
            assert!(base_seen.iter().all(|count| *count == 1));
            assert!(aux_seen.iter().all(|count| *count == 1));
            for (slot, &index) in rows.indices.iter().enumerate() {
                rows.fixed[slot] = p256_fixed.fixed_row_v1(p256, index).unwrap();
            }
        } else {
            let sources = MainTraceReplaySourcesV1::Bound {
                log19: &source,
                projection: &projection,
                io: &io,
            };
            // RFC/DER expose the exact scalar base rows, avoiding hundreds of
            // full-column replays when only native boundary openings are needed.
            if registration.segment.adapter == SegmentAdapterIdV1::StrictDer {
                for (slot, &index) in rows.indices.iter().enumerate() {
                    rows.base[slot].copy_from_slice(
                        &zk_x509_der_stark_aggregate_base_row_v1(&source.der.base, index).unwrap(),
                    );
                    rows.aux[slot].copy_from_slice(
                        &zk_x509_der_stark_aggregate_aux_row_v1(&source.der, index).unwrap(),
                    );
                }
            } else {
                if registration.segment.adapter == SegmentAdapterIdV1::Rfc5280 {
                    for (slot, &index) in rows.indices.iter().enumerate() {
                        rows.base[slot].copy_from_slice(&source.rfc.base_row_v1(index).unwrap());
                    }
                } else {
                    rows.replay_columns(
                        &layout,
                        registration,
                        &sources,
                        MainTraceColumnKindV1::Base,
                    );
                }
                rows.replay_columns(&layout, registration, &sources, MainTraceColumnKindV1::Aux);
            }
            for (slot, &index) in rows.indices.iter().enumerate() {
                rows.fixed[slot] = match registration.segment.adapter {
                    SegmentAdapterIdV1::StrictDer => {
                        source.der_fixed.fixed_row(index).unwrap().to_vec()
                    }
                    SegmentAdapterIdV1::Rfc5280 => source.rfc.fixed_row_v1(index).unwrap().to_vec(),
                    SegmentAdapterIdV1::Projection => projection_fixed[index].to_vec(),
                    SegmentAdapterIdV1::ByteMemory => io
                        .fixed_columns
                        .iter()
                        .map(|column| column[index])
                        .collect(),
                    _ => p256_fixed
                        .fixed_row_v1(p256_registration.unwrap(), index)
                        .unwrap(),
                };
            }
        }
        let log19_constraints =
            MainLog19ProverConstraintSourceV1::for_main_v1(&layout, &source).unwrap();
        for &edge in &edges {
            let current = rows.indices.binary_search(&edge).unwrap();
            let next = rows.indices.binary_search(&((edge + 1) % size)).unwrap();
            let opening = RegisteredOpenedRowsV1 {
                base_current: &rows.base[current],
                base_next: &rows.base[next],
                aux_current: &rows.aux[current],
                aux_next: &rows.aux[next],
            };
            let evaluate = |opening: RegisteredOpenedRowsV1<'_>| match registration.segment.adapter
            {
                SegmentAdapterIdV1::Projection => projection_constraint_residues_v1(
                    opening.base_current,
                    opening.base_next,
                    opening.aux_current,
                    opening.aux_next,
                    &rows.fixed[current],
                    source.post_base.projection(),
                )
                .unwrap(),
                SegmentAdapterIdV1::ByteMemory => io_constraint_residues_v1(
                    registration.segment,
                    assembly.io.logical_active_rows,
                    opening.base_current,
                    opening.base_next,
                    opening.aux_current,
                    opening.aux_next,
                    &rows.fixed[current],
                    source.post_base.io(),
                )
                .unwrap(),
                _ if registration.trace_group == 5 => log19_constraints
                    .constraint_residues_v1(
                        registration,
                        opening,
                        &rows.fixed[current],
                        &rows.fixed[next],
                    )
                    .unwrap(),
                _ => p256_opened_residues_v1(
                    registration,
                    opening,
                    &rows.fixed[current],
                    p256_challenges,
                    &p256_terminals[p256_registration.unwrap().signature_v1()],
                )
                .unwrap(),
            };
            let residues = evaluate(opening);
            assert_eq!(residues.len(), registration.segment.constraint_count);
            let nonzero = residues
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (*value != F::ZERO).then_some(index))
                .collect::<Vec<_>>();
            if !nonzero.is_empty() {
                failures.push((
                    registration.segment.adapter,
                    registration.segment.instance,
                    edge,
                    nonzero,
                ));
            }
            if edge == 0 {
                // A live-row mutation must be observed by this registration's
                // real dispatcher; an empty or disconnected evaluator cannot pass.
                let mut changed = ZeroizingMainTraceColumnV1(rows.base[current].to_vec());
                let mut detected = false;
                for column in 0..changed.len() {
                    changed[column] = changed[column].add(F::ONE);
                    let mutated = evaluate(RegisteredOpenedRowsV1 {
                        base_current: &changed,
                        ..opening
                    });
                    assert_eq!(mutated.len(), registration.segment.constraint_count);
                    changed[column] = rows.base[current][column];
                    if mutated.iter().any(|value| *value != F::ZERO) {
                        detected = true;
                        break;
                    }
                }
                assert!(
                    detected,
                    "unobserved live-row mutation for {:?}/{}",
                    registration.segment.adapter, registration.segment.instance
                );
            }
            checked_edges += 1;
        }
        checked += 1;
        eprintln!(
            "native boundary registration {checked}/49: {:?}/{}; {} edges",
            registration.segment.adapter,
            registration.segment.instance,
            edges.len()
        );
    }
    assert_eq!(registration_ids.len(), 49);
    assert_eq!(checked, 49, "every exact-profile registration must execute");
    assert!(
        failures.is_empty(),
        "actual maximum-source native boundary failures: {failures:?}"
    );
    eprintln!("all 49 maximum-source native registrations passed {checked_edges} boundary edges");
}
