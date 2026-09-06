// Lexically included by `zk_x509::stark::tests` to preserve the existing libtest paths.
use super::*;
use crate::privacy_engines::zk_x509::{
    der_air::ZkX509DerEkuV1, der_stark::ZkX509DerStarkPrivateShapeV1, io_air::ZkX509IoEndpointV1,
    p256_aggregate_adapter::p256_main_base_source_fixture_for_test_v1,
    sha_call_bus_stark::ZkX509ShaCallPublicShapeV1,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    privacy::{
        PrivacyAttributeDigestV1, PrivacyCertificateKeyDigestV1, PrivacyChallengeV1,
        PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1, PrivacyNullifierV1,
        PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyIdV1, PrivacyRootV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
        PrivacyX509ExtendedKeyUsageV1, PrivacyZkX509CertificatePolicyRecordDigestV1,
        PrivacyZkX509CrlRecordDigestV1, PrivacyZkX509TrustAnchorRecordDigestV1,
    },
};
use rand::{RngCore, SeedableRng as _, rngs::StdRng};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
};
static PROOF_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
fn proof_guard() -> std::sync::MutexGuard<'static, ()> {
    PROOF_TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("zk-X509 STARK proof mutex")
}
fn extension_v1(value: F) -> E {
    E::from_base(value)
}
const TERMINAL_TEST_HEADER_BYTES_V1: usize = 12;
const TERMINAL_TEST_RECORD_BYTES_V1: usize = 16;
const TERMINAL_TEST_VALUE_OFFSET_V1: usize = 8;
const TEST_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [0x93; 32];
fn overwrite_main_terminal_record_value_v1(
    encoded: &mut [u8],
    frame_offset: usize,
    record: usize,
    value: F,
) {
    let start = frame_offset
        + TERMINAL_TEST_HEADER_BYTES_V1
        + record * TERMINAL_TEST_RECORD_BYTES_V1
        + TERMINAL_TEST_VALUE_OFFSET_V1;
    encoded[start..start + 8].copy_from_slice(&value.0.to_be_bytes());
}
#[test]
fn main_transcript_is_release_only_and_domain_separated() {
    let public_digest = [0x71; 32];
    assert!(matches!(
        new_main_transcript_after_profile_validation_v1(&public_digest, [0_u8; 32]),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let first_release_digest = [0xa5; 32];
    let second_release_digest = [0x5a; 32];
    let first =
        new_main_transcript_after_profile_validation_v1(&public_digest, first_release_digest)
            .expect("candidate release transcript");
    let second =
        new_main_transcript_after_profile_validation_v1(&public_digest, second_release_digest)
            .expect("distinct candidate release transcript");
    let changed_public =
        new_main_transcript_after_profile_validation_v1(&[0x72; 32], first_release_digest)
            .expect("public-bound release transcript");
    let focused = new_transcript_v1(&test_stark_digest_v1(0x71)).expect("focused transcript");
    assert_ne!(first.state(), second.state());
    assert_ne!(first.state(), changed_public.state());
    assert_ne!(first.state(), focused.state());
}
#[derive(Default)]
struct MockMainTraceGroupSourceV1 {
    short_base_column: bool,
    noncanonical_aux_column: bool,
    short_fixed_row: bool,
    noncanonical_fixed_row: bool,
    short_residues: bool,
    noncanonical_residues: bool,
}
fn mock_main_column_value_v1(
    registration: RegisteredSegmentLayoutV1,
    local_column: usize,
    row: usize,
    aux: bool,
) -> F {
    F(u64::from(registration.segment.adapter.wire()) * 1_000_000
        + u64::from(registration.segment.instance) * 10_000
        + u64::try_from(local_column).expect("registered width fits u64") * 101
        + u64::try_from(row % 97).expect("small row residue")
        + u64::from(u8::from(aux)))
}
fn mock_main_residue_value_v1(
    registration: RegisteredSegmentLayoutV1,
    query_index: usize,
    x: F,
    opening: RegisteredOpenedRowsV1<'_>,
    fixed: &MainFixedOpenedRowsV1,
) -> F {
    let opened = opening
        .base_current
        .first()
        .copied()
        .unwrap_or(F::ZERO)
        .add(opening.base_next.first().copied().unwrap_or(F::ZERO))
        .add(opening.aux_current.first().copied().unwrap_or(F::ZERO))
        .add(opening.aux_next.first().copied().unwrap_or(F::ZERO))
        .add(fixed.current.first().copied().unwrap_or(F::ZERO))
        .add(fixed.next.first().copied().unwrap_or(F::ZERO));
    opened
        .add(F(u64::from(registration.segment.adapter.wire())))
        .add(F(u64::from(registration.segment.instance)))
        .add(F(u64::try_from(query_index).expect("log25 query fits u64")))
        .add(F(x.0 % 257))
}
fn mock_main_fixed_row_v1(registration: RegisteredSegmentLayoutV1, query_index: usize) -> Vec<F> {
    (0..registration.segment.fixed_width)
        .map(|column| {
            F(u64::from(registration.segment.adapter.wire()) * 1_000_000
                + u64::from(registration.segment.instance) * 10_000
                + u64::try_from(column).expect("registered width fits u64") * 101
                + u64::try_from(query_index % 97).expect("small query residue"))
        })
        .collect()
}
impl MainTraceGroupSourceV1 for MockMainTraceGroupSourceV1 {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        if local_column >= registration.segment.base_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let length = registration
            .segment
            .trace_size()
            .checked_sub(usize::from(u8::from(self.short_base_column)))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Ok(ZeroizingMainTraceColumnV1(
            (0..length)
                .map(|row| mock_main_column_value_v1(registration, local_column, row, false))
                .collect(),
        ))
    }
    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        if local_column >= registration.segment.aux_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut column = (0..registration.segment.trace_size())
            .map(|row| mock_main_column_value_v1(registration, local_column, row, true))
            .collect::<Vec<_>>();
        if self.noncanonical_aux_column {
            column[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        }
        Ok(ZeroizingMainTraceColumnV1(column))
    }
}
impl MainOpenedConstraintTestSourceV1 for MockMainTraceGroupSourceV1 {
    fn fixed_opened_rows_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
    ) -> Result<MainFixedOpenedRowsV1, ZkX509StarkErrorV1> {
        let mut fixed = MainFixedOpenedRowsV1 {
            current: mock_main_fixed_row_v1(registration, query_index),
            next: mock_main_fixed_row_v1(registration, next_query_index),
        };
        if self.short_fixed_row {
            fixed.current.pop();
        }
        if self.noncanonical_fixed_row && !fixed.next.is_empty() {
            fixed.next[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        }
        Ok(fixed)
    }
    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &MainFixedOpenedRowsV1,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let count = registration
            .segment
            .constraint_count
            .checked_sub(usize::from(u8::from(self.short_residues)))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let mut residues =
            vec![mock_main_residue_value_v1(registration, query_index, x, opening, fixed); count];
        if self.noncanonical_residues && !residues.is_empty() {
            residues[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        }
        Ok(residues)
    }
}
fn mock_main_group_providers_v1<'a>(
    sources: [&'a mut MockMainTraceGroupSourceV1; FULL_PROFILE_TRACE_GROUPS_V1],
) -> Vec<MainTraceGroupProviderV1<'a>> {
    let [log5, log8, log15, log16, log18, log19] = sources;
    vec![
        MainTraceGroupProviderV1::TestLog5(log5),
        MainTraceGroupProviderV1::TestLog8(log8),
        MainTraceGroupProviderV1::Log15(log15),
        MainTraceGroupProviderV1::TestLog16(log16),
        MainTraceGroupProviderV1::Log18(log18),
        MainTraceGroupProviderV1::Log19(log19),
    ]
}
fn mock_main_opened_group_providers_v1<'a>(
    sources: [&'a mut MockMainTraceGroupSourceV1; FULL_PROFILE_TRACE_GROUPS_V1],
) -> Vec<MainOpenedGroupProviderV1<'a>> {
    let [log5, log8, log15, log16, log18, log19] = sources;
    vec![
        MainOpenedGroupProviderV1::TestLog5(log5),
        MainOpenedGroupProviderV1::TestLog8(log8),
        MainOpenedGroupProviderV1::TestLog15(log15),
        MainOpenedGroupProviderV1::TestLog16(log16),
        MainOpenedGroupProviderV1::TestLog18(log18),
        MainOpenedGroupProviderV1::TestLog19(log19),
    ]
}
fn mock_main_opened_groups_v1(
    layout: &AggregateProofLayoutV1,
) -> Vec<aggregate::AggregateOpenedTraceGroupV1> {
    layout
        .trace_groups
        .iter()
        .enumerate()
        .map(|(group, descriptor)| {
            let row = |width: usize, offset: u64| {
                (0..width)
                    .map(|column| {
                        F(u64::try_from(group + 1).expect("small group") * 10_000
                            + offset
                            + u64::try_from(column).expect("registered width fits u64"))
                    })
                    .collect()
            };
            aggregate::AggregateOpenedTraceGroupV1 {
                base_current: row(descriptor.base_width, 100),
                base_next: row(descriptor.base_width, 200),
                aux_current: row(descriptor.aux_width, 300),
                aux_next: row(descriptor.aux_width, 400),
            }
        })
        .collect()
}
fn mock_main_alphas_v1(layout: &AggregateProofLayoutV1) -> Vec<Vec<Vec<E>>> {
    layout
        .registered_segments
        .iter()
        .enumerate()
        .map(|(registration, descriptor)| {
            vec![
                vec![
                    E::from_base(F(
                        u64::try_from(registration + 2).expect("small registration")
                    ));
                    descriptor.segment.constraint_count
                ];
                SECURITY_LANES
            ]
        })
        .collect()
}
fn mock_main_mixes_v1(layout: &AggregateProofLayoutV1) -> Vec<Vec<FriMixV1>> {
    let composition = (0..COMPOSITION_DEGREE_CHUNKS)
        .map(|index| E::from_base(F(u64::try_from(index + 31).expect("small mix"))))
        .collect::<Vec<_>>();
    layout
        .trace_groups
        .iter()
        .enumerate()
        .map(|(group, descriptor)| {
            (0..SECURITY_LANES)
                .map(|_| {
                    let coefficient =
                        E::from_base(F(u64::try_from(group + 3).expect("small group")));
                    FriMixV1 {
                        base: vec![coefficient; descriptor.base_width],
                        base_next: vec![E::ZERO; descriptor.base_width],
                        aux: vec![coefficient; descriptor.aux_width],
                        aux_next: vec![E::ZERO; descriptor.aux_width],
                        composition: composition.clone(),
                    }
                })
                .collect()
        })
        .collect()
}
fn endpoint(role: ZkX509IoSegmentRoleV1, instance: u16) -> ZkX509IoEndpointV1 {
    ZkX509IoEndpointV1 { role, instance }
}
fn channel(
    channel: u32,
    producer: ZkX509IoEndpointV1,
    consumers: Vec<ZkX509IoEndpointV1>,
    value: &[u8],
    public: bool,
) -> ZkX509IoChannelWitnessV1 {
    ZkX509IoChannelWitnessV1 {
        declaration: ZkX509IoChannelDeclarationV1 {
            channel,
            producer,
            consumers: consumers.clone(),
            byte_len: value.len() as u32,
            public_value: public.then(|| value.to_vec()),
        },
        producer_value: value.to_vec(),
        consumer_values: vec![value.to_vec(); consumers.len()],
    }
}
fn fixture_witnesses() -> Vec<ZkX509IoChannelWitnessV1> {
    vec![
        channel(
            0,
            endpoint(ZkX509IoSegmentRoleV1::StrictDer, 0),
            vec![
                endpoint(ZkX509IoSegmentRoleV1::Sha256, 0),
                endpoint(ZkX509IoSegmentRoleV1::P256, 0),
            ],
            &[0x30, 0x03, 0x02, 0x01],
            false,
        ),
        channel(
            1,
            endpoint(ZkX509IoSegmentRoleV1::Projection, 0),
            vec![endpoint(ZkX509IoSegmentRoleV1::PublicInput, 0)],
            &[0xA1, 0xB2, 0xC3, 0xD4],
            true,
        ),
    ]
}
fn fixture_statement() -> ZkX509IoStarkStatementV1 {
    ZkX509IoStarkStatementV1::new(
        fixture_witnesses()
            .into_iter()
            .map(|witness| witness.declaration)
            .collect(),
    )
    .expect("valid statement")
}
fn io_challenges_fixture_v1() -> ZkX509IoChallengesV1 {
    let mut transcript = TransparentTranscriptV1::new(
        ZK_X509_DIGEST_CONTEXT_V1,
        b"zk-x509-io-logical-active-tests-v1",
        &test_stark_digest_v1(0x63),
        &test_stark_digest_v1(0xA7),
    )
    .expect("I/O test transcript");
    derive_zk_x509_io_challenges_v1(&mut transcript).expect("I/O test challenges")
}
fn focused_io_material_fixture_v1() -> (IoTraceMaterialV1, ZkX509IoChallengesV1) {
    let statement = fixture_statement();
    let witnesses = fixture_witnesses();
    let logical_active_rows = io_active_rows_v1(statement.declarations()).expect("logical rows");
    let (layout, base_columns, fixed_columns, execution, sorted) =
        build_io_base_and_fixed_columns_v1(&statement, &witnesses)
            .expect("focused base and fixed columns");
    let challenges = io_challenges_fixture_v1();
    let aux_columns = build_io_aux_columns_v1(
        &statement,
        &witnesses,
        challenges,
        layout,
        logical_active_rows,
        &execution,
        &sorted,
    )
    .expect("focused auxiliary columns");
    (
        IoTraceMaterialV1 {
            layout,
            logical_active_rows,
            base_columns,
            aux_columns,
            fixed_columns,
        },
        challenges,
    )
}
fn main_io_topology_source_fixture_v1(
    disclosures: usize,
) -> (IrohaZkX509StarkP256StatementV1, ZkX509MainIoBaseMaterialV1) {
    let statement =
        crate::privacy_engines::zk_x509::main_io::tests::statement_with_disclosures_v1(disclosures);
    let plan = compile_zk_x509_main_io_declarations_v1(&statement).expect("MAIN I/O plan");
    let witnesses =
        topology_witnesses_v1(&plan.declarations).expect("topology-only MAIN witnesses");
    let (declarations, execution, sorted) =
        build_zk_x509_io_base_tables_v1(&witnesses).expect("topology-only MAIN base tables");
    assert_eq!(declarations, plan.declarations);
    assert_eq!(execution.len(), plan.logical_active_rows);
    assert_eq!(sorted.len(), plan.logical_active_rows);
    (
        statement,
        ZkX509MainIoBaseMaterialV1 {
            witnesses,
            declarations,
            logical_active_rows: plan.logical_active_rows,
            execution,
            sorted,
        },
    )
}
fn legacy_direct_io_fixed_row_v1(
    statement: &ZkX509IoStarkStatementV1,
    topology_execution: &[IoAccessV1],
    topology_sorted: &[IoAccessV1],
    witness_sorted: &[IoAccessV1],
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    index: usize,
) -> [F; IO_FIXED_WIDTH] {
    let mut fixed = [F::ZERO; IO_FIXED_WIDTH];
    if index < logical_active_rows {
        access_fixed_fields_v1(topology_execution[index], &mut fixed, FIX_EXEC_CHANNEL);
        access_fixed_fields_v1(topology_sorted[index], &mut fixed, FIX_SORT_CHANNEL);
        if let Some(value) =
            public_value_for_access_v1(statement.declarations(), topology_execution[index])
                .expect("legacy public lookup")
        {
            fixed[FIX_PUBLIC_SELECTOR] = F::ONE;
            fixed[FIX_PUBLIC_VALUE] = F(u64::from(value));
        }
        if index + 1 < logical_active_rows
            && witness_sorted[index].channel == witness_sorted[index + 1].channel
            && witness_sorted[index].offset == witness_sorted[index + 1].offset
        {
            fixed[FIX_SORT_SAME_ADDRESS_NEXT] = F::ONE;
        }
    }
    let [active, first, last_active, transition] =
        io_fixed_selector_fields_v1(index, logical_active_rows, layout.trace_size())
            .expect("legacy selectors");
    fixed[FIX_ACTIVE] = active;
    fixed[FIX_FIRST] = first;
    fixed[FIX_LAST_ACTIVE] = last_active;
    fixed[FIX_TRANSITION] = transition;
    fixed
}
#[test]
fn main_io_statement_only_compiler_matches_every_honest_legacy_fixed_row_and_column() {
    let (statement, source) = main_io_topology_source_fixture_v1(0);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
        .expect("MAIN I/O registration");
    let (io_statement, schedule) =
        compile_main_io_fixed_schedule_v1(registration.segment, &statement)
            .expect("statement-only fixed schedule");
    let synthetic =
        topology_witnesses_v1(io_statement.declarations()).expect("legacy topology witnesses");
    let (_, topology_execution, topology_sorted) =
        build_zk_x509_io_base_tables_v1(&synthetic).expect("legacy direct topology tables");
    assert_eq!(topology_execution.len(), source.logical_active_rows);
    assert_eq!(topology_sorted.len(), source.logical_active_rows);
    for index in 0..registration.segment.trace_size() {
        let compiled = schedule.fixed_row_v1(index).expect("compiled fixed row");
        let legacy = legacy_direct_io_fixed_row_v1(
            &io_statement,
            &topology_execution,
            &topology_sorted,
            &source.sorted,
            registration.segment,
            source.logical_active_rows,
            index,
        );
        assert_eq!(
            compiled, legacy,
            "all {IO_FIXED_WIDTH} fixed columns must match at MAIN row {index}"
        );
    }
    let post_base = projection_provider_post_base_v1(&statement);
    let prover =
        MainIoProverConstraintSourceV1::for_main_v1(&layout, &statement, &source, post_base)
            .expect("MAIN I/O prover constraint source");
    let mut streamed_columns = 0_usize;
    assert!(matches!(
        prover.stream_fixed_polynomials_v1(|column, coefficients| {
            assert_eq!(column, 0);
            assert_eq!(coefficients.len(), registration.segment.trace_size());
            streamed_columns += 1;
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        }),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert_eq!(streamed_columns, 1);
    let mut forged_source = source.clone();
    forged_source.execution[0].channel = forged_source.execution[0].channel.add(F::ONE);
    assert!(
            MainIoProverConstraintSourceV1::for_main_v1(
                &layout,
                &statement,
                &forged_source,
                post_base,
            )
            .is_err(),
            "the prover fixed source must validate witness topology against the public compiler"
        );
    let verifier = MainIoVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
        .expect("MAIN I/O verifier constraint source");
    assert_eq!(prover.fixed_schedule, schedule);
    assert_eq!(verifier.fixed_schedule, schedule);
    assert!(verifier.fixed_openings.is_empty());
}
#[test]
fn main_io_fixed_same_address_schedule_and_topology_are_independent_of_private_bytes() {
    let statement = fixture_statement();
    let witnesses = fixture_witnesses();
    let logical_active_rows = io_active_rows_v1(statement.declarations()).expect("logical rows");
    let layout = SegmentLayoutV1::for_io(logical_active_rows).expect("focused I/O layout");
    let (_, honest_fixed, honest_execution, honest_sorted) =
        build_io_base_and_fixed_columns_for_layout_v1(
            &statement,
            &witnesses,
            layout,
            logical_active_rows,
        )
        .expect("honest fixed schedule");
    let mut changed_witnesses = witnesses.clone();
    let private = changed_witnesses
        .iter_mut()
        .find(|witness| witness.declaration.public_value.is_none())
        .expect("private fixture channel");
    for (offset, byte) in private.producer_value.iter_mut().enumerate() {
        *byte = u8::try_from(0xE0 + offset).expect("small private fixture");
    }
    for consumer in &mut private.consumer_values {
        consumer.copy_from_slice(&private.producer_value);
    }
    let (_, changed_fixed, changed_execution, changed_sorted) =
        build_io_base_and_fixed_columns_for_layout_v1(
            &statement,
            &changed_witnesses,
            layout,
            logical_active_rows,
        )
        .expect("changed private bytes");
    assert_eq!(honest_fixed, changed_fixed);
    assert_ne!(honest_execution, changed_execution);
    assert_ne!(honest_sorted, changed_sorted);
    assert!(
        honest_fixed[FIX_SORT_SAME_ADDRESS_NEXT]
            .iter()
            .any(|value| *value == F::ONE)
    );
    let schedule = MainIoFixedScheduleV1::compile_v1(layout, &statement, logical_active_rows)
        .expect("statement-only schedule");
    schedule
        .validate_witness_topology_v1(&honest_execution, &honest_sorted)
        .expect("honest topology");
    schedule
        .validate_witness_topology_v1(&changed_execution, &changed_sorted)
        .expect("private values are not topology");
    let mut forged_execution = changed_execution;
    forged_execution[0].offset = forged_execution[0].offset.add(F::ONE);
    assert!(
        schedule
            .validate_witness_topology_v1(&forged_execution, &changed_sorted)
            .is_err()
    );
    let mut forged_sorted = changed_sorted;
    forged_sorted[0].is_write = F::ZERO;
    assert!(
        schedule
            .validate_witness_topology_v1(&honest_execution, &forged_sorted)
            .is_err()
    );
}
#[test]
fn full_main_io_registration_is_capacity_but_d0_through_d4_use_logical_selectors_and_endpoints() {
    let layout = SegmentLayoutV1::for_full_io().expect("full I/O layout");
    assert_eq!(layout.trace_log2, 18);
    assert_eq!(layout.trace_size(), ZK_X509_IO_FIXED_CAPACITY_ROWS_V1);
    assert_eq!(layout.active_rows, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1);
    for disclosures in 0..=4 {
        let statement =
            crate::privacy_engines::zk_x509::main_io::tests::statement_with_disclosures_v1(
                disclosures,
            );
        let plan = compile_zk_x509_main_io_declarations_v1(&statement).expect("MAIN I/O plan");
        validate_io_logical_geometry_v1(layout, plan.logical_active_rows)
            .expect("full capacity with logical prefix");
        assert!(plan.logical_active_rows < layout.trace_size());
        assert_eq!(
            io_fixed_selector_fields_v1(0, plan.logical_active_rows, layout.trace_size())
                .expect("first row"),
            [F::ONE, F::ONE, F::ZERO, F::ONE]
        );
        assert_eq!(
            io_fixed_selector_fields_v1(
                plan.logical_active_rows - 1,
                plan.logical_active_rows,
                layout.trace_size(),
            )
            .expect("last logical row"),
            [F::ONE, F::ZERO, F::ONE, F::ONE]
        );
        assert_eq!(
            io_fixed_selector_fields_v1(
                plan.logical_active_rows,
                plan.logical_active_rows,
                layout.trace_size(),
            )
            .expect("first padding row"),
            [F::ZERO, F::ZERO, F::ZERO, F::ONE]
        );
        assert_eq!(
            io_fixed_selector_fields_v1(
                layout.trace_size() - 1,
                plan.logical_active_rows,
                layout.trace_size(),
            )
            .expect("last capacity row"),
            [F::ZERO, F::ZERO, F::ZERO, F::ZERO]
        );
        let current_base = vec![F::ZERO; IO_BASE_WIDTH];
        let next_base = current_base.clone();
        let mut current_aux = vec![F::ZERO; IO_AUX_WIDTH];
        let mut next_aux = vec![F::ZERO; IO_AUX_WIDTH];
        let logical_field =
            F(u64::try_from(plan.logical_active_rows).expect("logical rows fit u64"));
        for row in [&mut current_aux, &mut next_aux] {
            row[AUX_CONT_GLOBAL_END] = logical_field;
            row[AUX_CONT_MEMORY_END] = logical_field;
            row[AUX_CONT_EXEC_START..AUX_CONT_EXEC_START + IO_LANES].fill(F::ONE);
            row[AUX_CONT_SORT_START..AUX_CONT_SORT_START + IO_LANES].fill(F::ONE);
        }
        let mut fixed = vec![F::ZERO; IO_FIXED_WIDTH];
        fixed[FIX_TRANSITION] = F::ONE;
        let residues = io_constraint_residues_v1(
            layout,
            plan.logical_active_rows,
            &current_base,
            &next_base,
            &current_aux,
            &next_aux,
            &fixed,
            io_challenges_fixture_v1(),
        )
        .expect("logical continuation residues");
        assert!(
            residues.iter().all(|residue| *residue == F::ZERO),
            "d={disclosures} logical padding row must satisfy every I/O constraint"
        );
        let capacity_residues = io_constraint_residues_v1(
            layout,
            layout.active_rows,
            &current_base,
            &next_base,
            &current_aux,
            &next_aux,
            &fixed,
            io_challenges_fixture_v1(),
        )
        .expect("capacity-sized comparison residues");
        assert!(
            capacity_residues.iter().any(|residue| *residue != F::ZERO),
            "d={disclosures} continuation endpoints must not use registration capacity"
        );
    }
}
#[test]
fn io_logical_geometry_rejects_zero_over_capacity_and_noncanonical_two_count_layouts() {
    let full = SegmentLayoutV1::for_full_io().expect("full I/O");
    assert!(validate_io_logical_geometry_v1(full, 0).is_err());
    assert!(validate_io_logical_geometry_v1(full, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1).is_err());
    assert!(io_fixed_selector_fields_v1(0, 0, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1).is_err());
    assert!(
        io_fixed_selector_fields_v1(
            0,
            ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1,
            ZK_X509_IO_FIXED_CAPACITY_ROWS_V1,
        )
        .is_err()
    );
    let logical_active_rows =
        io_active_rows_v1(fixture_statement().declarations()).expect("focused logical rows");
    let mut noncanonical = SegmentLayoutV1::for_io(logical_active_rows).expect("focused I/O");
    noncanonical.active_rows += 1;
    assert!(validate_io_logical_geometry_v1(noncanonical, logical_active_rows).is_err());
    let base = vec![F::ZERO; IO_BASE_WIDTH];
    let aux = vec![F::ZERO; IO_AUX_WIDTH];
    let fixed = vec![F::ZERO; IO_FIXED_WIDTH];
    assert!(matches!(
        io_constraint_residues_v1(
            full,
            0,
            &base,
            &base,
            &aux,
            &aux,
            &fixed,
            io_challenges_fixture_v1(),
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert!(matches!(
        io_constraint_residues_v1(
            full,
            ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1,
            &base,
            &base,
            &aux,
            &aux,
            &fixed,
            io_challenges_fixture_v1(),
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
}
#[test]
fn io_material_rejects_off_by_one_selectors_padding_forgery_and_continuation_mismatch() {
    let (material, challenges) = focused_io_material_fixture_v1();
    validate_io_trace_material_shape_v1(&material).expect("canonical material shape");
    validate_io_base_constraints_v1(&material, challenges)
        .expect("canonical focused I/O constraints");
    let logical = material.logical_active_rows;
    let mut moved_last = material.clone();
    moved_last.fixed_columns[FIX_LAST_ACTIVE][logical - 1] = F::ZERO;
    moved_last.fixed_columns[FIX_LAST_ACTIVE][logical] = F::ONE;
    assert!(matches!(
        validate_io_trace_material_shape_v1(&moved_last),
        Err(ZkX509StarkErrorV1::IoWitness)
    ));
    let mut active_padding = material.clone();
    active_padding.fixed_columns[FIX_ACTIVE][logical] = F::ONE;
    assert!(matches!(
        validate_io_trace_material_shape_v1(&active_padding),
        Err(ZkX509StarkErrorV1::IoWitness)
    ));
    let mut base_padding = material.clone();
    base_padding.base_columns[EXEC_VALUE][logical] = F::ONE;
    assert!(matches!(
        validate_io_trace_material_shape_v1(&base_padding),
        Err(ZkX509StarkErrorV1::IoWitness)
    ));
    let mut wrong_continuation = material.clone();
    wrong_continuation.aux_columns[AUX_CONT_GLOBAL_END][logical] =
        wrong_continuation.aux_columns[AUX_CONT_GLOBAL_END][logical].add(F::ONE);
    assert!(matches!(
        validate_io_trace_material_shape_v1(&wrong_continuation),
        Err(ZkX509StarkErrorV1::IoWitness)
    ));
    let mut wrong_logical_count = material;
    wrong_logical_count.logical_active_rows += 1;
    assert!(matches!(
        validate_io_trace_material_shape_v1(&wrong_logical_count),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
}
#[test]
fn phased_full_main_io_provider_enforces_base_then_token_then_aux_and_rejects_tampering() {
    let _guard = proof_guard();
    let (statement, source) = main_io_topology_source_fixture_v1(0);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
        .expect("MAIN I/O registration");
    let assert_source_rejected = |changed: &ZkX509MainIoBaseMaterialV1| {
        assert!(
            MainIoTraceGroupSourceV1::for_main_v1(&layout, &statement, changed).is_err(),
            "altered MAIN source must reject before entering the base phase"
        );
    };
    let mut changed = source.clone();
    changed.logical_active_rows -= 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.logical_active_rows += 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.logical_active_rows = 0;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.logical_active_rows = ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.declarations.swap(0, 1);
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.declarations[0].channel += 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.declarations[0].producer.instance =
        changed.declarations[0].producer.instance.wrapping_add(1);
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.declarations[0].consumers[0].instance = changed.declarations[0].consumers[0]
        .instance
        .wrapping_add(1);
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.declarations[0].byte_len += 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    let private_declaration = changed
        .declarations
        .iter_mut()
        .find(|declaration| declaration.public_value.is_none())
        .expect("MAIN has private channels");
    private_declaration.public_value = Some(vec![0_u8; private_declaration.byte_len as usize]);
    assert_source_rejected(&changed);
    changed = source.clone();
    changed
        .declarations
        .iter_mut()
        .find(|declaration| declaration.public_value.is_some())
        .expect("MAIN has public channels")
        .public_value = None;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.execution.pop();
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.execution[0].value = changed.execution[0].value.add(F::ONE);
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.sorted.pop();
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.sorted.swap(0, 1);
    assert_source_rejected(&changed);
    changed = source.clone();
    let public = changed
        .declarations
        .iter_mut()
        .find_map(|declaration| declaration.public_value.as_mut())
        .expect("MAIN has verifier-fixed public digest channels");
    public[0] ^= 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.witnesses[0].declaration.channel += 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.witnesses.pop();
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.witnesses[0].producer_value[0] ^= 1;
    assert_source_rejected(&changed);
    changed = source.clone();
    changed.witnesses[0].consumer_values.pop();
    assert_source_rejected(&changed);
    drop(changed);
    let (other_statement, _) = main_io_topology_source_fixture_v1(1);
    assert!(
        MainIoTraceGroupSourceV1::for_main_v1(&layout, &other_statement, &source).is_err(),
        "statement/source replay across disclosure counts must reject"
    );
    let isolated_io_layout =
        AggregateProofLayoutV1::for_segments(&[
            SegmentLayoutV1::for_full_io().expect("full I/O segment")
        ])
        .expect("isolated I/O aggregate");
    assert!(
        MainIoTraceGroupSourceV1::for_main_v1(&isolated_io_layout, &statement, &source).is_err(),
        "production provider requires the exact 49-registration MAIN layout"
    );
    let post_base = projection_provider_post_base_v1(&statement);
    let mut failed_bind = MainIoTraceGroupSourceV1::for_main_v1(&layout, &statement, &source)
        .expect("failed-bind adversary base phase");
    failed_bind.base_columns[EXEC_VALUE][source.logical_active_rows] = F::ONE;
    assert!(matches!(
        failed_bind.bind_challenges_v1(post_base),
        Err(ZkX509StarkErrorV1::IoWitness)
    ));
    failed_bind.base_columns[EXEC_VALUE][source.logical_active_rows] = F::ZERO;
    assert!(matches!(
        failed_bind.bind_challenges_v1(post_base),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert!(failed_bind.aux_columns.is_none());
    drop(failed_bind);
    let mut provider = MainIoTraceGroupSourceV1::for_main_v1(&layout, &statement, &source)
        .expect("MAIN I/O base phase");
    let logical = source.logical_active_rows;
    let capacity = ZK_X509_IO_FIXED_CAPACITY_ROWS_V1;
    assert_eq!(provider.registration, registration);
    assert_eq!(provider.registration.segment.active_rows, capacity);
    assert_eq!(provider.registration.segment.trace_size(), capacity);
    assert!(!provider.bind_attempted);
    assert!(provider.aux_columns.is_none());
    assert!(provider.post_base.is_none());
    provider
        .validate_base_phase_v1()
        .expect("validated pre-challenge base phase");
    assert!(provider.validate_bound_phase_v1().is_err());
    assert!(
        provider.native_aux_column_v1(registration, 0).is_err(),
        "auxiliary columns must not exist before the post-base token"
    );
    let base_column = provider
        .native_base_column_v1(registration, EXEC_VALUE)
        .expect("base column available before token");
    assert_eq!(base_column.len(), capacity);
    drop(base_column);
    assert!(
        provider
            .native_base_column_v1(registration, IO_BASE_WIDTH)
            .is_err()
    );
    let wrong_registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("projection registration");
    assert!(
        provider
            .native_base_column_v1(wrong_registration, 0)
            .is_err()
    );
    provider.base_columns[EXEC_VALUE][logical] = F::ONE;
    assert!(provider.validate_base_phase_v1().is_err());
    provider.base_columns[EXEC_VALUE][logical] = F::ZERO;
    provider.fixed_columns[FIX_LAST_ACTIVE][logical - 1] = F::ZERO;
    provider.fixed_columns[FIX_LAST_ACTIVE][logical] = F::ONE;
    assert!(provider.validate_base_phase_v1().is_err());
    provider.fixed_columns[FIX_LAST_ACTIVE][logical - 1] = F::ONE;
    provider.fixed_columns[FIX_LAST_ACTIVE][logical] = F::ZERO;
    provider
        .validate_base_phase_v1()
        .expect("restored base phase");
    provider
        .bind_challenges_v1(post_base)
        .expect("opaque post-base token binds I/O auxiliary phase");
    assert!(provider.bind_attempted);
    assert_eq!(provider.post_base, Some(post_base));
    assert!(provider.aux_columns.is_some());
    provider
        .validate_bound_phase_v1()
        .expect("validated post-token auxiliary phase");
    assert!(matches!(
        provider.bind_challenges_v1(post_base),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    let aux_column = provider
        .native_aux_column_v1(registration, AUX_CONT_GLOBAL_END)
        .expect("aux column available after token");
    assert_eq!(aux_column.len(), capacity);
    drop(aux_column);
    assert!(
        provider
            .native_aux_column_v1(registration, IO_AUX_WIDTH)
            .is_err()
    );
    assert!(
        provider
            .native_aux_column_v1(wrong_registration, 0)
            .is_err()
    );
    let aux_columns = provider.aux_columns.as_ref().expect("bound aux");
    assert!(
        provider
            .base_columns
            .iter()
            .chain(aux_columns)
            .chain(&provider.fixed_columns)
            .all(|column| column.len() == capacity)
    );
    assert_eq!(provider.fixed_columns[FIX_ACTIVE][logical - 1], F::ONE);
    assert_eq!(provider.fixed_columns[FIX_LAST_ACTIVE][logical - 1], F::ONE);
    assert_eq!(provider.fixed_columns[FIX_ACTIVE][logical], F::ZERO);
    assert_eq!(provider.fixed_columns[FIX_LAST_ACTIVE][logical], F::ZERO);
    assert_eq!(provider.fixed_columns[FIX_TRANSITION][logical], F::ONE);
    let logical_field = F(u64::try_from(logical).expect("logical rows fit u64"));
    for index in [0, logical - 1, logical, capacity - 1] {
        assert_eq!(aux_columns[AUX_CONT_GLOBAL_END][index], logical_field);
        assert_eq!(aux_columns[AUX_CONT_MEMORY_END][index], logical_field);
    }
    for lane in 0..IO_LANES {
        let final_exec = aux_columns[AUX_EXEC_AFTER + lane][logical - 1];
        let final_sort = aux_columns[AUX_SORT_AFTER + lane][logical - 1];
        assert_eq!(aux_columns[AUX_EXEC_BEFORE + lane][logical], final_exec);
        assert_eq!(aux_columns[AUX_EXEC_AFTER + lane][capacity - 1], final_exec);
        assert_eq!(aux_columns[AUX_SORT_BEFORE + lane][logical], final_sort);
        assert_eq!(aux_columns[AUX_SORT_AFTER + lane][capacity - 1], final_sort);
    }
    provider.aux_columns.as_mut().expect("bound aux")[AUX_CONT_MEMORY_END][logical] =
        logical_field.add(F::ONE);
    assert!(provider.validate_bound_phase_v1().is_err());
    provider.aux_columns.as_mut().expect("bound aux")[AUX_CONT_MEMORY_END][logical] = logical_field;
    let first_padding_product =
        provider.aux_columns.as_ref().expect("bound aux")[AUX_EXEC_BEFORE][logical];
    provider.aux_columns.as_mut().expect("bound aux")[AUX_EXEC_BEFORE][logical] =
        first_padding_product.add(F::ONE);
    assert!(matches!(
        provider.validate_bound_phase_v1(),
        Err(ZkX509StarkErrorV1::IoWitness)
    ));
    provider.zeroize_private_buffers_v1();
    assert!(provider.base_columns.is_empty());
    assert!(provider.fixed_columns.is_empty());
    assert!(provider.aux_columns.is_none());
    assert!(provider.post_base.is_none());
    assert!(provider.native_base_column_v1(registration, 0).is_err());
    assert!(provider.native_aux_column_v1(registration, 0).is_err());
    assert!(matches!(
        provider.bind_challenges_v1(post_base),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
}
#[test]
fn main_io_verifier_rejects_registration_query_next_x_width_and_noncanonical_without_mutation() {
    let (statement, _) = main_io_topology_source_fixture_v1(0);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
        .expect("MAIN I/O registration");
    let wrong_registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("wrong registration");
    let post_base = projection_provider_post_base_v1(&statement);
    let mut source = MainIoVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
        .expect("MAIN I/O verifier source");
    let original_schedule = source.fixed_schedule.clone();
    let original_challenges = source.challenges;
    let query_index = 17;
    let next_query_index = source
        .next_query_index_v1(query_index)
        .expect("canonical next query");
    let common_lde_size = source.common_lde_size_v1().expect("common size");
    let wrong_next_query_index = (next_query_index + 1) % common_lde_size;
    let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
    let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
    let base = vec![F::ZERO; registration.segment.base_width];
    let aux = vec![F::ZERO; registration.segment.aux_width];
    let opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    assert!(
        source
            .constraint_residues_v1(
                wrong_registration,
                query_index,
                next_query_index,
                x,
                opening,
            )
            .is_err()
    );
    assert!(
        source
            .constraint_residues_v1(registration, common_lde_size, next_query_index, x, opening,)
            .is_err()
    );
    assert!(
        source
            .constraint_residues_v1(
                registration,
                query_index,
                wrong_next_query_index,
                x,
                opening,
            )
            .is_err()
    );
    assert!(
        source
            .constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x.add(F::ONE),
                opening,
            )
            .is_err()
    );
    assert!(
        source
            .constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1),
                opening,
            )
            .is_err()
    );
    let short_base = vec![F::ZERO; registration.segment.base_width - 1];
    let short_opening = RegisteredOpenedRowsV1 {
        base_current: &short_base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    assert!(
        source
            .constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                short_opening,
            )
            .is_err()
    );
    let short_aux = vec![F::ZERO; registration.segment.aux_width - 1];
    let short_aux_opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &short_aux,
    };
    assert!(
        source
            .constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                short_aux_opening,
            )
            .is_err()
    );
    let mut noncanonical_base = base.clone();
    noncanonical_base[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
    let noncanonical_opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &noncanonical_base,
        aux_current: &aux,
        aux_next: &aux,
    };
    assert!(
        source
            .constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                noncanonical_opening,
            )
            .is_err()
    );
    let mut noncanonical_aux = aux.clone();
    noncanonical_aux[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
    let noncanonical_aux_opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &noncanonical_aux,
        aux_next: &aux,
    };
    assert!(
        source
            .constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                noncanonical_aux_opening,
            )
            .is_err()
    );
    assert!(
        source.fixed_openings.is_empty(),
        "every invalid request must reject before sampling"
    );
    assert_eq!(source.fixed_schedule, original_schedule);
    assert_eq!(source.challenges, original_challenges);
}
#[test]
fn main_io_closed_log18_verifier_matches_prover_residues_and_reuses_bounded_cache() {
    let (statement, source) = main_io_topology_source_fixture_v1(0);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
        .expect("MAIN I/O registration");
    let post_base = projection_provider_post_base_v1(&statement);
    let prover =
        MainIoProverConstraintSourceV1::for_main_v1(&layout, &statement, &source, post_base)
            .expect("MAIN I/O prover constraint source");
    let mut verifier =
        MainIoVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
            .expect("MAIN I/O verifier constraint source");
    let query_index = 17;
    let next_query_index = verifier
        .next_query_index_v1(query_index)
        .expect("canonical next query");
    let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
    let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
    let base = vec![F::ZERO; registration.segment.base_width];
    let aux = vec![F::ZERO; registration.segment.aux_width];
    let opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    let mut log5 = MockMainTraceGroupSourceV1::default();
    let mut log8 = MockMainTraceGroupSourceV1::default();
    let mut log15 = MockMainTraceGroupSourceV1::default();
    let mut log16 = MockMainTraceGroupSourceV1::default();
    let mut log19 = MockMainTraceGroupSourceV1::default();
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        vec![
            MainOpenedGroupProviderV1::TestLog5(&mut log5),
            MainOpenedGroupProviderV1::TestLog8(&mut log8),
            MainOpenedGroupProviderV1::TestLog15(&mut log15),
            MainOpenedGroupProviderV1::TestLog16(&mut log16),
            MainOpenedGroupProviderV1::Io(&mut verifier),
            MainOpenedGroupProviderV1::TestLog19(&mut log19),
        ],
    )
    .expect("closed MAIN provider set");
    let opened_residues = providers
        .registered_constraint_residues_v1(registration, query_index, next_query_index, x, opening)
        .expect("closed log-18 I/O verifier route");
    drop(providers);
    assert_eq!(verifier.fixed_openings.len(), 2);
    let verifier_fixed = *verifier
        .fixed_openings
        .get(&query_index)
        .expect("verifier-generated current fixed opening");
    let prover_residues = prover
        .constraint_residues_v1(registration, opening, &verifier_fixed)
        .expect("direct prover residues");
    assert_eq!(opened_residues, prover_residues);
    assert_eq!(opened_residues.len(), registration.segment.constraint_count);
    let alphas = vec![E::ONE; registration.segment.constraint_count];
    let composition = prover
        .composition_value_v1(registration, x, opening, &verifier_fixed, &alphas)
        .expect("prover quotient");
    assert_eq!(
        composition,
        accumulator_quotient_value_v1(registration.segment, x, &opened_residues, &alphas,)
            .expect("residue quotient")
    );
    let repeated = verifier
        .constraint_residues_v1(registration, query_index, next_query_index, x, opening)
        .expect("cache reuse");
    assert_eq!(repeated, opened_residues);
    assert_eq!(verifier.fixed_openings.len(), 2);
    let cap_query = 1_000;
    let cap_next = verifier
        .next_query_index_v1(cap_query)
        .expect("cap-test next query");
    verifier
        .fixed_openings
        .entry(cap_query)
        .or_insert([F::ZERO; IO_FIXED_WIDTH]);
    let mut candidate = 0_usize;
    while verifier.fixed_openings.len() < VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 {
        if candidate != cap_next {
            verifier
                .fixed_openings
                .entry(candidate)
                .or_insert([F::ZERO; IO_FIXED_WIDTH]);
        }
        candidate += 1;
    }
    assert!(!verifier.fixed_openings.contains_key(&cap_next));
    let full_cache = verifier.fixed_openings.clone();
    let cap_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(cap_query as u128));
    assert!(
        verifier
            .constraint_residues_v1(registration, cap_query, cap_next, cap_x, opening,)
            .is_err(),
        "the 117th distinct fixed opening must reject before sampling"
    );
    assert_eq!(verifier.fixed_openings, full_cache);
    assert_eq!(
        verifier.fixed_openings.len(),
        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
    );
}
fn fixture() -> &'static (ZkX509IoStarkStatementV1, Vec<u8>) {
    static FIXTURE: OnceLock<(ZkX509IoStarkStatementV1, Vec<u8>)> = OnceLock::new();
    let _guard = proof_guard();
    FIXTURE.get_or_init(|| {
        let statement = fixture_statement();
        let mut rng = StdRng::from_seed([0x5A; 32]);
        let proof = prove_zk_x509_io_segmented_stark_v1_with_rng(
            &statement,
            &fixture_witnesses(),
            &mut rng,
        )
        .expect("deterministic proof");
        (statement, proof)
    })
}
fn fixture_layout() -> SegmentLayoutV1 {
    SegmentLayoutV1::for_io(io_active_rows_v1(fixture().0.declarations()).expect("rows"))
        .expect("layout")
}
fn fixture_aggregate_layout() -> AggregateProofLayoutV1 {
    AggregateProofLayoutV1::for_segments(&[fixture_layout()]).expect("aggregate layout")
}
fn decode_fixture() -> ZkX509SegmentedStarkProofV1 {
    decode_zk_x509_segmented_stark_proof_v1(&fixture().1, &fixture_aggregate_layout())
        .expect("decode fixture")
}
fn assert_rejected(proof: &ZkX509SegmentedStarkProofV1) {
    match encode_zk_x509_segmented_stark_proof_v1(proof, &fixture_aggregate_layout()) {
        Ok(bytes) => assert!(
            verify_zk_x509_io_segmented_stark_v1(&fixture().0, &bytes).is_err(),
            "adversarial proof must reject"
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch) => {}
        Err(error) => panic!("unexpected adversarial encode failure: {error}"),
    }
}
fn projection_fixture() -> &'static (
    IrohaZkX509StarkP256StatementV1,
    ZkX509ProjectionWitnessV1,
    Vec<u8>,
) {
    static FIXTURE: OnceLock<(
        IrohaZkX509StarkP256StatementV1,
        ZkX509ProjectionWitnessV1,
        Vec<u8>,
    )> = OnceLock::new();
    let _guard = proof_guard();
    FIXTURE.get_or_init(|| {
        let (statement, witness) =
            crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let mut rng = StdRng::from_seed([0xA7; 32]);
        let proof =
            prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &witness, &mut rng)
                .expect("deterministic projection proof");
        (statement, witness, proof)
    })
}
fn projection_layout() -> SegmentLayoutV1 {
    SegmentLayoutV1::for_projection().expect("projection layout")
}
fn projection_provider_post_base_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
) -> ZkX509CredentialMainPostBaseChallengesV1 {
    let consensus_context_digest =
        ZkX509CredentialPublicBindingV1::from_consensus_context_v1(statement, [0x91; 32])
            .expect("projection fixture consensus binding")
            .consensus_context_digest;
    derive_zk_x509_credential_pre_aux_binding_v1(
        ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            consensus_context_digest,
            [0x31; 32],
            core::array::from_fn(|index| {
                test_stark_digest_v1(u8::try_from(index + 1).expect("six roots"))
            }),
        ),
        test_stark_digest_v1(0x41),
        test_stark_digest_v1(0x51),
        test_stark_digest_v1(0x61),
    )
    .expect("joint post-base challenge derivation")
    .main_post_base()
}
fn main_base_commitment_session_fixture_v1() -> ZkX509MainBaseCommitmentSessionV1 {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
        &layout,
        [0xB1; 32],
        TEST_COMPILED_PROFILE_DIGEST_V1,
    )
    .expect("canonical MAIN base session after injected profile validation")
}
fn unpinned_main_verifier_profile_fixture_v1() -> ZkX509MainVerifierProfileV1 {
    ZkX509MainVerifierProfileV1 {
        registration: validate_zk_x509_main_registration_shape_v1()
            .expect("canonical registration shape"),
        compiled_profile_digest: TEST_COMPILED_PROFILE_DIGEST_V1,
    }
}
fn der_layout() -> SegmentLayoutV1 {
    SegmentLayoutV1::for_der(ZkX509DerStarkShapeV1.active_rows()).expect("DER layout")
}
fn der_aggregate_layout() -> AggregateProofLayoutV1 {
    AggregateProofLayoutV1::for_segments(&[der_layout()]).expect("DER aggregate layout")
}
fn sha_word_layout() -> SegmentLayoutV1 {
    SegmentLayoutV1::for_sha_segment(0, ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0])
        .expect("SHA batch layout")
}
fn sha_word_aggregate_layout() -> AggregateProofLayoutV1 {
    AggregateProofLayoutV1::for_segments(&[sha_word_layout()]).expect("SHA-word aggregate layout")
}
fn projection_aggregate_layout() -> AggregateProofLayoutV1 {
    AggregateProofLayoutV1::for_segments(&[projection_layout()])
        .expect("projection aggregate layout")
}
fn accumulator_aggregate_layout() -> AggregateProofLayoutV1 {
    AggregateProofLayoutV1::for_accumulators_v1().expect("accumulator aggregate layout")
}
fn p256_aggregate_challenges_fixture() -> P256AggregateChallengesV1 {
    let mut transcript = TransparentTranscriptV1::new(
        ZK_X509_DIGEST_CONTEXT_V1,
        b"p256-aggregate-test",
        &test_stark_digest_v1(0x31),
        &test_stark_digest_v1(0x57),
    )
    .expect("P-256 aggregate transcript");
    derive_p256_aggregate_challenges_v1(&mut transcript)
        .expect("canonical P-256 aggregate challenges")
}
fn p256_terminal_fixture(role: P256EcdsaRoleV1) -> P256TerminalRegistrationV1 {
    let buses = P256BusTerminalClaimsV1 {
        value_execution: [F(11), F(12), F(13), F(14)],
        value_sorted: [F(11), F(12), F(13), F(14)],
        value_arithmetic_copy: [F(21), F(22), F(23), F(24)],
        arithmetic_value_copy: [F(21), F(22), F(23), F(24)],
        arithmetic_scalar: [F(31), F(32), F(33), F(34)],
        window_scalar: [F(41), F(42), F(43), F(44)],
        scalar_bus_arithmetic: [F(31), F(32), F(33), F(34)],
        scalar_bus_window: [F(41), F(42), F(43), F(44)],
    };
    let mut running = [F::ONE; P256_CROSS_TRACE_LANES_V1];
    let cross_sources = p256_cross_trace_terminal_roles_v1(role)
        .iter()
        .copied()
        .enumerate()
        .map(|(index, source_role)| {
            let terminal = core::array::from_fn(|lane| F((100 + index * 7 + lane) as u64));
            let claim = P256CrossTraceTerminalClaimV1 {
                role: source_role,
                start: running,
                terminal,
            };
            running = terminal;
            claim
        })
        .collect::<Vec<_>>();
    let terminals = P256TerminalRegistrationV1 {
        buses,
        cross_sources,
        sink: running,
    };
    terminals.validate(role).expect("canonical P-256 terminals");
    terminals
}
fn zero_p256_terminal_fixture(role: P256EcdsaRoleV1) -> P256TerminalRegistrationV1 {
    let zero = [F::ZERO; P256_CROSS_TRACE_LANES_V1];
    let buses = P256BusTerminalClaimsV1 {
        value_execution: zero,
        value_sorted: zero,
        value_arithmetic_copy: zero,
        arithmetic_value_copy: zero,
        arithmetic_scalar: zero,
        window_scalar: zero,
        scalar_bus_arithmetic: zero,
        scalar_bus_window: zero,
    };
    let mut running = [F::ONE; P256_CROSS_TRACE_LANES_V1];
    let cross_sources = p256_cross_trace_terminal_roles_v1(role)
        .iter()
        .copied()
        .map(|source_role| {
            let claim = P256CrossTraceTerminalClaimV1 {
                role: source_role,
                start: running,
                terminal: zero,
            };
            running = zero;
            claim
        })
        .collect::<Vec<_>>();
    P256TerminalRegistrationV1 {
        buses,
        cross_sources,
        sink: zero,
    }
}
fn p256_main_provider_post_base_fixture_v1() -> ZkX509CredentialMainPostBaseChallengesV1 {
    derive_zk_x509_credential_pre_aux_binding_v1(
        ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            [0x71; 32],
            [0x72; 32],
            core::array::from_fn(|index| {
                test_stark_digest_v1(u8::try_from(index + 0x31).expect("six roots"))
            }),
        ),
        test_stark_digest_v1(0x73),
        test_stark_digest_v1(0x74),
        test_stark_digest_v1(0x75),
    )
    .expect("canonical joint post-base challenges")
    .main_post_base()
}
fn p256_main_terminal_claims_fixture_v1() -> ZkX509P256TerminalClaimsV1 {
    fn scalar_buses(signature: usize) -> P256BusTerminalClaimsV1 {
        let mut buses = zero_p256_terminal_fixture(P256EcdsaRoleV1::CertificateOrCrl).buses;
        buses.arithmetic_scalar = core::array::from_fn(|lane| {
            F(u64::try_from(100 + signature * 16 + lane).expect("small fixture"))
        });
        buses.scalar_bus_arithmetic = buses.arithmetic_scalar;
        buses.window_scalar = core::array::from_fn(|lane| {
            F(u64::try_from(200 + signature * 16 + lane).expect("small fixture"))
        });
        buses.scalar_bus_window = buses.window_scalar;
        buses
    }
    let certificate_or_crl = core::array::from_fn(|signature| {
        let mut terminals = zero_p256_terminal_fixture(P256EcdsaRoleV1::CertificateOrCrl);
        terminals.buses = scalar_buses(signature);
        ZkX509P256CertificateTerminalClaimsV1 {
            buses: terminals.buses,
            cross_sources: terminals
                .cross_sources
                .as_slice()
                .try_into()
                .expect("four certificate cross-source claims"),
            sink: terminals.sink,
        }
    });
    let mut wallet = zero_p256_terminal_fixture(P256EcdsaRoleV1::WalletOwnership);
    wallet.buses = scalar_buses(P256_SIGNATURE_COUNT_V1 - 1);
    ZkX509P256TerminalClaimsV1 {
        certificate_or_crl,
        wallet: ZkX509P256WalletTerminalClaimsV1 {
            buses: wallet.buses,
            cross_sources: wallet
                .cross_sources
                .as_slice()
                .try_into()
                .expect("five wallet cross-source claims"),
            sink: wallet.sink,
        },
    }
}
fn main_log19_statement_fixture_v1() -> ZkX509Rfc5280StatementV1 {
    ZkX509Rfc5280StatementV1 {
        presentation_not_before_unix_seconds: 1,
        presentation_not_after_unix_seconds: 2,
        leaf_key_usage: 1,
        leaf_extended_key_usages: vec![ZkX509DerEkuV1::ClientAuthentication],
        crl_number: 1,
        disclosed_attribute_indices: Vec::new(),
    }
}
fn main_log19_terminal_claims_fixture_v1() -> ZkX509MainTerminalClaimsV1 {
    let der = ZkX509DerStarkTerminalClaimsV1 {
        input_byte: [F(3), F(5), F(7), F(11)],
        node: [F(13), F(17), F(19), F(23)],
    };
    let mut sha = ZkX509ShaSegmentTerminalClaimsV1::canonical_zero_for_test_v1();
    for segment in &mut sha.segments {
        for stream in &mut segment.rfc_stream_products {
            stream.fill(F::ONE);
        }
    }
    ZkX509MainTerminalClaimsV1 {
        der,
        rfc5280: ZkX509Rfc5280StarkTerminalClaimsV1::canonical_for_der_test_v1(der)
            .expect("canonical DER/RFC test claims"),
        sha,
        p256: p256_main_terminal_claims_fixture_v1(),
    }
}
fn main_log19_source_fixture_v1(
    layout: &AggregateProofLayoutV1,
) -> MainLog19VerifierConstraintSourceV1 {
    MainLog19VerifierConstraintSourceV1::for_main_v1(
        layout,
        &main_log19_statement_fixture_v1(),
        p256_main_provider_post_base_fixture_v1(),
        main_log19_terminal_claims_fixture_v1(),
    )
    .expect("closed mixed log19 verifier source")
}
fn main_log19_query_coordinates_fixture_v1() -> [usize; QUERY_COUNT] {
    core::array::from_fn(|index| index * P256_MAIN_LOG19_NEXT_STRIDE_V1)
}
fn main_log19_adversarial_query_coordinates_fixture_v1() -> [usize; QUERY_COUNT] {
    core::array::from_fn(|index| {
        let shift = match index {
            0 => 0,
            1 => ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
            _ => (index * 7_919 + 17) % ZK_X509_DER_STARK_TRACE_SIZE_V1,
        };
        shift * P256_MAIN_LOG19_NEXT_STRIDE_V1 + index
    })
}
fn p256_main_base_source_fixture_v1() -> P256MainBaseSourceV1 {
    p256_main_base_source_fixture_for_test_v1().expect("canonical central P-256 base source")
}
