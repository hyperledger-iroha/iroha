// Included by SHA tests: independent canonical-producer and actual-credential handoffs.

fn canonical_sha_producer_channels_v1(
    disclosed_attributes: usize,
) -> Vec<crate::privacy_engines::zk_x509::io_air::ZkX509IoChannelWitnessV1> {
    use crate::privacy_engines::zk_x509::{
        der_air::{build_zk_x509_rfc5280_trace_v1, rfc5280_io_witnesses_v1},
        relation::release_fixture::{
            build_zk_x509_release_fixture_v1, reference_statement_context_v1,
        },
        verifier_profile::rfc_statement_with_crl_number_v1,
    };
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
        .expect("maximum signed credential");
    let mut statement = rfc_statement_with_crl_number_v1(
        &fixture.statement,
        fixture.authoritative_state.crl_record().crl_number,
    );
    statement
        .disclosed_attribute_indices
        .truncate(disclosed_attributes);
    let trace = build_zk_x509_rfc5280_trace_v1(
        &fixture.witness.certificate_chain_der,
        &fixture.witness.crl_der,
        statement,
    )
    .expect("canonical RFC producer");
    rfc5280_io_witnesses_v1(&trace, 0)
        .expect("canonical producer channels")
        .into_iter()
        .filter(|channel| {
            channel.declaration.consumers
                == [ZkX509IoEndpointV1 {
                    role: ZkX509IoSegmentRoleV1::Sha256,
                    instance: 0,
                }]
        })
        .collect()
}

fn canonical_issuer_spki_channel_v1(disclosed_attributes: usize) -> u32 {
    let channels = canonical_sha_producer_channels_v1(disclosed_attributes);
    let issuer = channels.last().expect("issuer-SPKI producer");
    assert_eq!(issuer.producer_value.len(), ZK_X509_CA_SPKI_DER_BYTES_V1);
    issuer.declaration.channel
}

#[test]
fn rfc_sha_channels_match_actual_producer_for_every_disclosure_count() {
    for disclosed_attributes in 0..=4 {
        let channels = canonical_sha_producer_channels_v1(disclosed_attributes);
        let mut channels = channels.iter();
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        })
        .unwrap();
        // The producer's SHA endpoints are ordered by certificate slot, CRL TBS,
        // complete CRL, then issuer SPKI. Derive addresses from its declarations,
        // independently of the SHA adapter's arithmetic offsets.
        for call in [0, 1, 2, 3, 4, CRL_ISSUER_SPKI_CALL_V1] {
            let manifest = schedule.calls[call];
            let consumer =
                sha_rfc_consumer_channels_v1(manifest.call, manifest.role, disclosed_attributes)
                    .unwrap()
                    .unwrap();
            let message = channels.next().expect("message producer");
            assert_eq!(consumer.message_channel, message.declaration.channel);
            assert_eq!(
                consumer.message_capacity_bytes,
                message.producer_value.len()
            );
            if call == CRL_ISSUER_SPKI_CALL_V1 {
                assert_eq!(consumer.length_channel, None);
                assert_eq!(consumer.role, ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha);
                // The old address selected the CRL P-256 key, which has a
                // different endpoint, capacity and tuple address.
                assert_ne!(
                    consumer.message_channel,
                    (5 + 2 * disclosed_attributes + 22) as u32
                );
            } else {
                let length = channels.next().expect("length producer");
                assert_eq!(length.producer_value.len(), core::mem::size_of::<u64>());
                assert_eq!(consumer.length_channel, Some(length.declaration.channel));
                assert_eq!(
                    consumer.role,
                    match call {
                        0..=2 => ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
                        3 => ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
                        4 => ZkX509Rfc5280OutputRoleV1::CrlCommitment,
                        _ => unreachable!("enumerated SHA consumers"),
                    }
                );
            }
        }
        assert!(channels.next().is_none(), "every SHA endpoint is consumed");
    }
}

#[test]
fn maximum_credential_rfc_sha_handshake_matches_every_role_and_bound_segment() {
    use crate::privacy_engines::zk_x509::{
        main_assembly::build_zk_x509_main_trace_assembly_v1,
        relation::{
            ZkX509GovernanceV1,
            release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
        },
        rfc5280_stark::{ZkX509Rfc5280StarkColumnProviderV1, ZkX509ShaSegmentTerminalClaimsV1},
    };
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
        .expect("maximum signed credential");
    assert_eq!(fixture.witness.certificate_chain_der.len(), 3);
    assert_eq!(fixture.statement.disclosed_attributes.len(), 4);
    assert_eq!(fixture.crl_entry_count, 64);
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
    .expect("actual maximum source assembly");
    // Typed test roots bypass expensive commitments only in this test; every
    // downstream provider receives the same production-derived X5B1 capability.
    let pre_aux = ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
        [0x71; 32],
        assembly.verifier_profile.compiled_profile_digest,
        core::array::from_fn(|index| zk_x509_test_digest384_v1(index as u8 + 1)),
    );
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        pre_aux,
        zk_x509_test_digest384_v1(0x91),
        zk_x509_test_digest384_v1(0xA1),
        zk_x509_test_digest384_v1(0xB1),
    )
    .unwrap();
    let changed_ca = derive_zk_x509_credential_pre_aux_binding_v1(
        pre_aux,
        zk_x509_test_digest384_v1(0x91),
        zk_x509_test_digest384_v1(0xA1),
        zk_x509_test_digest384_v1(0xB2),
    )
    .unwrap();
    assert_ne!(
        binding, changed_ca,
        "compact-CA root must bind the shared capability"
    );
    assert_ne!(binding.rfc5280(), changed_ca.rfc5280());
    assert_ne!(binding.sha(), changed_ca.sha());
    assert_ne!(binding.sha_word(), changed_ca.sha_word());
    let rfc = ZkX509Rfc5280StarkColumnProviderV1::new_v1(
        &assembly.rfc_base,
        binding.main_post_base().der(),
        binding.rfc5280(),
crate::privacy_engines::zk_x509::rfc5280_stark::ZkX509ShaUnionCentersV1::identity_fixture_v1(),
)
    .unwrap();
    let roles = [
        ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
        ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
        ZkX509Rfc5280OutputRoleV1::CrlCommitment,
        ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
    ];
    let mut role_products = [[F::ONE; ZK_X509_SHA_BUS_LANES_V1]; 4];
    for (call, role_index) in [(0, 0), (1, 0), (2, 0), (3, 1), (4, 2), (12, 3)] {
        let source = build_zk_x509_sha_batch_call_base_source_v1(
            assembly.sha_schedule.calls[call],
            &assembly.sha_witnesses[call],
            4,
        )
        .unwrap();
        let trace = bind_zk_x509_sha_batch_call_base_with_initial_products_v1(
            source,
            binding,
            &ZkX509ShaSegmentProductStateV1::one_v1(),
        )
        .unwrap();
        for (product, factor) in role_products[role_index]
            .iter_mut()
            .zip(trace.rfc_terminal.combined_products())
        {
            *product = product.mul(factor);
        }
    }
    let (_, consumer_columns) =
        crate::privacy_engines::zk_x509::rfc5280_stark::zk_x509_rfc_sha_union_columns_v1();
    for ((role, products), columns) in roles.into_iter().zip(role_products).zip(consumer_columns) {
        for lane in 0..4 {
            let values = zeroize::Zeroizing::new(rfc.build_aux_column_v1(columns[lane]).unwrap());
            assert_eq!(values.len(), ZK_X509_SHA_SEGMENT_ROWS_V1);
            assert_eq!(
                products[lane],
                *values.last().unwrap(),
                "actual private RFC/SHA handoff for {role:?} lane {lane}"
            );
        }
    }
    let mut segments = Vec::new();
    let mut ca_calls = Vec::new();
    let mut boundary_rows = Vec::new();
    let mut column = zeroize::Zeroizing::new(vec![F::ZERO; ZK_X509_SHA_SEGMENT_ROWS_V1]);
    // Raw SHA word/value rows populate base column zero. Fixed-column selector
    // indices are a separate address space and must not select this probe.
    const NATIVE_WORD_COLUMN: usize = 0;
    for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
        let mut source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(
            &assembly.sha_schedule,
            &assembly.sha_witnesses,
            segment,
        )
        .unwrap();
        source
            .fill_base_column_v1(segment, NATIVE_WORD_COLUMN, &mut column)
            .unwrap();
        assert!(column.iter().any(|value| *value != F::ZERO));
        let fingerprint = |cells: &[F]| {
            let mut hash = Sha256::new();
            for cell in cells {
                hash.update(cell.0.to_be_bytes());
            }
            <[u8; 32]>::from(hash.finalize())
        };
        let base_before = fingerprint(&column);
        let active = ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment];
        let indices = [
            0,
            1,
            active - 2,
            active - 1,
            active,
            active + 1,
            ZK_X509_SHA_SEGMENT_ROWS_V1 - 1,
        ];
        let mut rows = indices.map(|row| {
            let (base, fixed) = source.base_fixed_row_v1(row).unwrap();
            ZkX509ShaBatchRowV1 {
                base,
                aux: [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
                fixed,
            }
        });
        let mut bound = source.bind_v1(binding).unwrap();
        bound
            .replay_base_column_v1(segment, NATIVE_WORD_COLUMN, &mut column)
            .unwrap();
        assert_eq!(
            fingerprint(&column),
            base_before,
            "bound source changed base cells"
        );
        let terminal = bound
            .fill_aux_column_with_air_terminals_v1(segment, 0, &mut column)
            .unwrap();
        let mut seen = [false; 7];
        let streamed = bound
            .for_each_aux_row_with_air_terminals_v1(|row, aux| {
                if let Some(index) = indices.iter().position(|&index| index == row) {
                    rows[index].aux = aux;
                    seen[index] = true;
                }
            })
            .unwrap();
        assert!(seen.into_iter().all(|seen| seen));
        assert_eq!(streamed, terminal);
        boundary_rows.push(rows);
        segments.push(terminal.segment);
        ca_calls.extend(terminal.ca_call_boundaries);
    }
    let claims =
        ZkX509ShaSegmentTerminalClaimsV1::from_sha_air_terminals_v1(ca_calls.try_into().unwrap())
            .expect("four canonical segments and all thirteen compact-CA call boundaries");
    for (segment, rows) in boundary_rows.iter().enumerate() {
        assert_actual_sha_cyclic_boundaries_v1(
            segment,
            rows,
            binding,
            segment as u8,
            &claims.ca_calls,
        );
    }
    for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
        assert_eq!(
            segments.iter().fold(F::ONE, |product, segment| product
                .mul(segment.combined_rfc_products()[lane])),
            role_products
                .iter()
                .fold(F::ONE, |product, role| product.mul(role[lane])),
            "segment replay must preserve every role product in lane {lane}",
        );
    }
}

#[test]
fn private_sha_join_points_are_after_all_rfc_events_for_every_public_shape() {
    for disclosed_attributes in 0..=4 {
        let shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        };
        let schedule = ZkX509ShaCallScheduleV1::new(shape).unwrap();
        let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(shape).unwrap();
        for (segment, expected_call) in [20_u8, 25, 9, 28].into_iter().enumerate() {
            let row = ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment] - 1;
            let (manifest, call_row) = schedule
                .logical_row(segment * ZK_X509_SHA_SEGMENT_ROWS_V1 + row)
                .unwrap();
            assert_eq!(manifest.call, expected_call);
            assert_eq!(call_row + 1, manifest.maximum_logical_rows());
            // The final call of each segment has no RFC consumer channel at
            // any row. Its before-row products are therefore the final products.
            assert!(
                sha_rfc_consumer_channels_v1(manifest.call, manifest.role, disclosed_attributes)
                    .unwrap()
                    .is_none()
            );
            let row_fixed = fixed.fixed_row_v1(segment, row).unwrap();
            assert_eq!(row_fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1], F::ONE);
            assert_eq!(row_fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1], F::ZERO);
        }
    }
}
