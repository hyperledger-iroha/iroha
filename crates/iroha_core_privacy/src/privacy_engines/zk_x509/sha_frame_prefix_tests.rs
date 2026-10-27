// Included by SHA tests: actual compiler rows and recomputed SHA state.

#[test]
fn every_declared_frame_prefix_matches_actual_credential_producers() {
    use crate::privacy_engines::zk_x509::{
        main_assembly::build_zk_x509_main_trace_assembly_v1,
        relation::{
            ZkX509GovernanceV1,
            release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
        },
    };
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true).unwrap();
    let trust = fixture.authoritative_state.trust_anchor();
    let crl = fixture.authoritative_state.crl_record();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .unwrap();
    for disclosures in 0..=4 {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        })
        .unwrap();
        let mut framed_calls = 0;
        for manifest in schedule.calls().iter().copied() {
            let message = &assembly.sha_witnesses[usize::from(manifest.call)].message;
            let raw_len = message.len() - sha_rfc_framed_field_offset_v1(manifest.role).unwrap();
            let framed = sha_frame_byte_v1(manifest, 0).is_some();
            assert_eq!(
                framed,
                manifest.call >= 4 && manifest.activation != ZkX509ShaCallActivationV1::Inactive
            );
            framed_calls += usize::from(framed);
            for offset in 0..manifest.maximum_message_bytes {
                if let Some(expected) = sha_frame_byte_v1(manifest, offset) {
                    let expected = match expected {
                        ShaFrameByteV1::Constant(value) => value,
                        ShaFrameByteV1::RawLengthHigh => (raw_len >> 8) as u8,
                        ShaFrameByteV1::RawLengthLow => raw_len as u8,
                    };
                    assert_eq!(
                        message.get(offset),
                        Some(&expected),
                        "call {} byte {offset}",
                        manifest.call
                    );
                }
            }
        }
        assert_eq!(framed_calls, 21 + disclosures);
    }
}

#[test]
fn frame_fixed_provider_populates_all_roles_and_keeps_payload_bytes_free() {
    for disclosures in 0..=4 {
        let shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        };
        let provider = ZkX509ShaBatchFixedProviderV1::new_v1(shape).unwrap();
        for manifest in provider.schedule.calls().iter().copied() {
            let word_schedule = &provider.calls[usize::from(manifest.call)];
            let mut seen = vec![false; manifest.maximum_message_bytes];
            for local_row in 0..manifest.maximum_local_rows {
                let word_fixed = word_schedule.fixed_row_v1(local_row).unwrap();
                if word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] != F::ONE {
                    continue;
                }
                let index = word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0 as usize;
                let global = manifest.first_logical_row + local_row;
                let fixed = provider
                    .fixed_row_v1(
                        global / ZK_X509_SHA_SEGMENT_ROWS_V1,
                        global % ZK_X509_SHA_SEGMENT_ROWS_V1,
                    )
                    .unwrap();
                for byte in 0..4 {
                    let offset = 4 * index + byte;
                    let expected = sha_frame_byte_v1(manifest, offset);
                    assert_eq!(
                        fixed[ZK_X509_SHA_FIXED_FRAME_MASK_V1 + byte],
                        F(u64::from(expected.is_some()))
                    );
                    let tuple = match expected {
                        Some(ShaFrameByteV1::Constant(value)) => {
                            (F(u64::from(value)), F::ZERO, F::ZERO)
                        }
                        Some(ShaFrameByteV1::RawLengthHigh) => (F::ZERO, F::ONE, F::ZERO),
                        Some(ShaFrameByteV1::RawLengthLow) => (F::ZERO, F::ZERO, F::ONE),
                        None => (F::ZERO, F::ZERO, F::ZERO),
                    };
                    assert_eq!(
                        (
                            fixed[ZK_X509_SHA_FIXED_FRAME_CONSTANT_V1 + byte],
                            fixed[ZK_X509_SHA_FIXED_FRAME_LENGTH_HIGH_V1 + byte],
                            fixed[ZK_X509_SHA_FIXED_FRAME_LENGTH_LOW_V1 + byte]
                        ),
                        tuple
                    );
                    if let Some(seen) = seen.get_mut(offset) {
                        *seen = true;
                    }
                }
            }
            assert!(seen.iter().all(|seen| *seen));
        }
    }
}

fn frame_input_row_v1(call: &ZkX509ShaBatchCallTraceV1, offset: usize) -> usize {
    (0..call.word.maximum_local_rows)
        .find(|row| {
            let fixed = call.fixed_row(*row).unwrap();
            fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE
                && fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1] == F((offset / 4) as u64)
        })
        .expect("production input-word row")
}
fn frame_local_residues_v1(call: &ZkX509ShaBatchCallTraceV1, row: usize) -> Vec<F> {
    evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
        &call.row(row).unwrap(),
        &call.row(row + 1).unwrap(),
        word_challenges(),
        challenges(),
        &rfc_challenges(),
        0,
    )
    .unwrap()
}

#[test]
fn compiled_frame_rejects_equal_length_prefix_substitution_after_recomputed_sha() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 0,
    })
    .unwrap();
    // Exact independent encoders supply the honest messages; the malicious
    // variant changes only one frame byte, then rebuilds the entire SHA state.
    for (call_index, message, offsets) in [
        (
            4,
            crl_commitment_preimage_v1(&[1, 2, 3]).unwrap(),
            vec![0, 63, 64],
        ),
        (
            12,
            crl_issuer_spki_preimage_v1(&[0x31; 91]).unwrap(),
            vec![0],
        ),
    ] {
        let manifest = schedule.calls[call_index];
        let honest = ZkX509ShaCallWitnessV1 {
            role: manifest.role,
            digest: Sha256::digest(&message).into(),
            message,
        };
        let canonical = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &honest,
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            0,
        )
        .unwrap();
        for offset in offsets {
            let row = frame_input_row_v1(&canonical, offset);
            assert!(
                frame_local_residues_v1(&canonical, row)
                    .iter()
                    .all(|value| *value == F::ZERO)
            );
            let mut changed = honest.clone();
            changed.message[offset] ^= 1;
            changed.digest = Sha256::digest(&changed.message).into();
            assert_eq!(changed.message.len(), honest.message.len());
            assert_ne!(changed.digest, honest.digest);
            let adversarial = build_zk_x509_sha_batch_call_trace_v1(
                manifest,
                &changed,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                0,
            )
            .unwrap();
            let residues = frame_local_residues_v1(&adversarial, row);
            let new_start = SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1 + ZK_X509_SHA_RFC_LENGTH_BITS_V1;
            let new_end = new_start + ZK_X509_SHA_RFC_LENGTH_BITS_V1 + 8;
            assert!(
                residues[new_start..new_end]
                    .iter()
                    .any(|value| *value != F::ZERO)
            );
            assert!(
                residues[..new_start]
                    .iter()
                    .chain(&residues[new_end..])
                    .all(|value| *value == F::ZERO),
                "predecessor local equations accept the recomputed prefix at call {call_index}, offset {offset}"
            );
            // The full verifier evaluator embeds the same original row in Fp4.
            use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;
            let embed = |row: ZkX509ShaBatchRowV1| ZkX509ShaBatchRowV1 {
                base: row.base.map(E::from_base),
                aux: row.aux.map(E::from_base),
                fixed: row.fixed.map(E::from_base),
            };
            let extension = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
                &embed(adversarial.row(row).unwrap()),
                &embed(adversarial.row(row + 1).unwrap()),
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                0,
            )
            .unwrap();
            assert_eq!(
                extension,
                residues.into_iter().map(E::from_base).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn frame_length_carry_and_message_membership_are_separate_constraints() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 0,
    })
    .unwrap();
    let manifest = schedule.calls[12];
    let witness = witness_for(manifest);
    let call = build_zk_x509_sha_batch_call_trace_v1(
        manifest,
        &witness,
        word_challenges(),
        challenges(),
        &rfc_challenges(),
        0,
    )
    .unwrap();
    let current = call.row(0).unwrap();
    let mut next = call.row(1).unwrap();
    next.base[ZK_X509_SHA_LENGTH_BITS_START_V1] =
        F::ONE.sub(next.base[ZK_X509_SHA_LENGTH_BITS_START_V1]);
    let residue = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
        &current,
        &next,
        word_challenges(),
        challenges(),
        &rfc_challenges(),
        0,
    )
    .unwrap();
    assert_ne!(
        residue[SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1 + ZK_X509_SHA_RFC_LENGTH_BITS_V1],
        F::ZERO
    );
    let row = frame_input_row_v1(&call, 0);
    let mut current = call.row(row).unwrap();
    current.base[SHA_WORD_CAPACITY_MESSAGE_MASK_V1] = F::ZERO;
    let residue = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
        &current,
        &call.row(row + 1).unwrap(),
        word_challenges(),
        challenges(),
        &rfc_challenges(),
        0,
    )
    .unwrap();
    let byte_constraints =
        SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1 + 2 * ZK_X509_SHA_RFC_LENGTH_BITS_V1;
    assert_eq!(
        residue[byte_constraints],
        F::ZERO,
        "word byte still equals the expected prefix"
    );
    assert_ne!(
        residue[byte_constraints + 1],
        F::ZERO,
        "prefix cannot masquerade as SHA padding"
    );
}
