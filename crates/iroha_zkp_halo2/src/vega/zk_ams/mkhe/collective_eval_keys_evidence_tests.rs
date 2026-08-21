fn synthetic_evidence_body(tag: u8, record_index: u32) -> [u8; 5] {
    let mut body = [0_u8; 5];
    body[0] = tag;
    body[1..].copy_from_slice(&record_index.to_be_bytes());
    body
}
fn synthetic_evidence_descriptor(tag: u8, record_index: u32) -> (u64, [u8; 32]) {
    let body = synthetic_evidence_body(tag, record_index);
    (
        u64::try_from(body.len() + EVIDENCE_RECORD_DIGEST_BYTES_V1).unwrap(),
        keccak256(&body),
    )
}
fn manual_evidence_descriptor_set_digest_v1(
    header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    records: impl IntoIterator<Item = (u32, ZkAmsMkheCollectiveEvidenceRecordKindV1, u64, [u8; 32])>,
    final_count: u32,
) -> [u8; 32] {
    // Independent framing oracle: keep this literal and do not call the
    // production descriptor-set recurrence from this helper.
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.evaluated-key-evidence-descriptor-set");
    hash.update(&[
        MKHE_VERSION_V1,
        header.kind as u8,
        header.purpose as u8,
        header.ordinal,
    ]);
    hash.update(&header.galois_exponent.to_be_bytes());
    hash.update(&header.collective_key_digest);
    for (record_index, record_kind, canonical_bytes, canonical_digest) in records {
        hash.update(&record_index.to_be_bytes());
        hash.update(&[record_kind as u8]);
        hash.update(&canonical_bytes.to_be_bytes());
        hash.update(&canonical_digest);
    }
    hash.update(&final_count.to_be_bytes());
    hash.finalize()
}
fn manual_cks_compact_output_set_digest_v1(
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
    collective_key_digest: [u8; 32],
    records: impl IntoIterator<Item = (u32, [u8; 32])>,
    final_count: u32,
) -> [u8; 32] {
    // Independent framing oracle: keep this literal and do not call the
    // production compact-output recurrence from this helper.
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.evaluated-key-cks-compact-output-set");
    hash.update(&[MKHE_VERSION_V1, purpose as u8, ordinal]);
    hash.update(&exponent.to_be_bytes());
    hash.update(&collective_key_digest);
    for (digit_index, digest) in records {
        hash.update(&digit_index.to_be_bytes());
        hash.update(&digest);
    }
    hash.update(&final_count.to_be_bytes());
    hash.finalize()
}
fn synthetic_compact_output_digest(digit_index: u32) -> [u8; 32] {
    keccak256(&synthetic_evidence_body(0x53, digit_index))
}
fn evidence_capability_fixture(
    label: &[u8],
    epoch: u64,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    exponent: u32,
) -> EvidenceCapabilityFixture {
    let mut random = EvidenceTestRandom::new(label);
    let mut parties = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    for _ in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        parties.push(ZkAmsMkheActivePartySecretV1::generate(&mut random).unwrap());
    }
    parties.sort_by_key(|party| party.party().expect("generated party identifier"));
    let party_refs: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = parties
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .expect("exact release party count");
    let roster = ZkAmsMkheGovernedActiveRosterV1::new(epoch, party_refs, &mut random).unwrap();
    let wire_roster = roster.to_wire_roster().unwrap();
    let transcript_digest = keccak256(&[label, b"-transcript"].concat());
    let collective_key_digest = keccak256(&[label, b"-collective-key"].concat());
    let cks_context = ZkAmsMkheTrustedCksContextV1::from_staged_verified_digests(
        wire_roster,
        roster.key_material_digest(),
        transcript_digest,
        collective_key_digest,
        std::array::from_fn(|index| [0x10_u8.wrapping_add(index as u8); 32]),
        [0x21; 32],
        [0x22; 32],
        std::array::from_fn(|index| [0x30_u8.wrapping_add(index as u8); 32]),
        std::array::from_fn(|index| [0x40_u8.wrapping_add(index as u8); 32]),
    )
    .unwrap();
    let source_context =
        ZkAmsMkheTrustedSourceContextV1::from_staged_verified_digests(roster, &cks_context)
            .unwrap();
    let counts = evidence_set::checked_evidence_record_counts_v1(
        purpose,
        release_profile_v1().gadget_digits,
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
    )
    .unwrap();
    let source_header = ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        kind: ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        purpose,
        ordinal,
        galois_exponent: exponent,
        collective_key_digest,
    };
    let source_proof_set_digest = manual_evidence_descriptor_set_digest_v1(
        source_header,
        (0..counts.source).map(|record_index| {
            let (kind, _) =
                evidence_set::expected_source_descriptor_v1(purpose, record_index).unwrap();
            let (canonical_bytes, canonical_digest) =
                synthetic_evidence_descriptor(0x51, record_index);
            (record_index, kind, canonical_bytes, canonical_digest)
        }),
        counts.source,
    );
    let cks_header = ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        kind: ZkAmsMkheCollectiveEvidenceSetKindV1::Cks,
        ..source_header
    };
    let cks_proof_set_digest = manual_evidence_descriptor_set_digest_v1(
        cks_header,
        (0..counts.cks).map(|digit_index| {
            let (canonical_bytes, canonical_digest) =
                synthetic_evidence_descriptor(0x52, digit_index);
            (
                digit_index,
                ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit,
                canonical_bytes,
                canonical_digest,
            )
        }),
        counts.cks,
    );
    let layout = seekable_evaluated_key_layout(&release_profile_v1()).unwrap();
    let entry = ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
        ordinal,
        purpose,
        exponent,
        u64::from(ordinal) * layout.payload_bytes,
        layout.payload_bytes,
        keccak256(&[label, b"-payload"].concat()),
        source_proof_set_digest,
        cks_proof_set_digest,
    )
    .unwrap();
    EvidenceCapabilityFixture {
        roster,
        source_context,
        cks_context,
        entry,
        counts,
    }
}
fn synthetic_source_receipt(
    fixture: &EvidenceCapabilityFixture,
    record_index: u32,
) -> ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1 {
    let (kind, party_index) =
        evidence_set::expected_source_descriptor_v1(fixture.entry.purpose(), record_index).unwrap();
    let (canonical_bytes, canonical_digest) = synthetic_evidence_descriptor(0x51, record_index);
    source_stream::test_mint_verified_evidence_receipt_v1(
        &fixture.source_context,
        kind,
        fixture.entry.ordinal(),
        record_index,
        party_index,
        canonical_bytes,
        canonical_digest,
    )
}
fn synthetic_cks_receipt(
    fixture: &EvidenceCapabilityFixture,
    digit_index: u32,
) -> ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    let (canonical_bytes, canonical_digest) = synthetic_evidence_descriptor(0x52, digit_index);
    cks_stream::test_mint_verified_evidence_receipt_v1(
        &fixture.cks_context,
        fixture.entry.ordinal(),
        u8::try_from(digit_index).unwrap(),
        canonical_bytes,
        canonical_digest,
        synthetic_compact_output_digest(digit_index),
    )
}
fn mint_synthetic_evidence_capability(
    fixture: &EvidenceCapabilityFixture,
) -> ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1 {
    verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
        &fixture.source_context,
        &fixture.cks_context,
        &fixture.entry,
        (0..fixture.counts.source).map(|index| Ok(synthetic_source_receipt(fixture, index))),
        (0..fixture.counts.cks).map(|index| Ok(synthetic_cks_receipt(fixture, index))),
    )
    .unwrap()
}
fn evidence_runtime_binding(
    fixture: &EvidenceCapabilityFixture,
) -> evidence_set::EvidenceSetRuntimeBindingV1 {
    let axes = source_stream::verified_evidence_context_summary_v1(&fixture.source_context)
        .unwrap()
        .axes;
    evidence_set::EvidenceSetRuntimeBindingV1 {
        entry: fixture.entry,
        profile_digest: axes.profile_digest,
        roster_digest: axes.roster_digest,
        key_material_digest: axes.key_material_digest,
        epoch: axes.epoch,
        transcript_digest: axes.transcript_digest,
        collective_key_digest: axes.collective_key_digest,
    }
}
fn evidence_test_runtime(
    fixture: &EvidenceCapabilityFixture,
) -> ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1 {
    let profile = release_profile_v1();
    let layout = seekable_evaluated_key_layout(&profile).unwrap();
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    let mut entries = Vec::with_capacity(ZK_AMS_T256_GALOIS_KEY_COUNT_V1 + 1);
    for ordinal in 0..=ZK_AMS_T256_GALOIS_KEY_COUNT_V1 {
        if ordinal == usize::from(fixture.entry.ordinal()) {
            entries.push(fixture.entry);
            continue;
        }
        let (purpose, exponent) = if ordinal == 0 {
            (ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization, 0)
        } else {
            (
                ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
                schedule.entries[ordinal - 1].exponent,
            )
        };
        let ordinal_u8 = u8::try_from(ordinal).unwrap();
        entries.push(
            ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
                ordinal_u8,
                purpose,
                exponent,
                u64::from(ordinal_u8) * layout.payload_bytes,
                layout.payload_bytes,
                keccak256(&[b'p', ordinal_u8]),
                keccak256(&[b's', ordinal_u8]),
                keccak256(&[b'c', ordinal_u8]),
            )
            .unwrap(),
        );
    }
    let total_payload_bytes = layout.payload_bytes * u64::try_from(entries.len()).unwrap();
    let pointer = ZkAmsMkheEvaluatedKeySorafsPointerV1::new(
        [0xa1; 32],
        total_payload_bytes,
        [0xa2; 32],
        [0xa3; 32],
        [0xa4; 32],
    )
    .unwrap();
    let axes = source_stream::verified_evidence_context_summary_v1(&fixture.source_context)
        .unwrap()
        .axes;
    let manifest = ZkAmsMkheCollectiveEvaluatedKeyManifestV1::new(
        &fixture.roster.to_wire_roster().unwrap(),
        axes.transcript_digest,
        entries,
        pointer,
    )
    .unwrap();
    let binding = super::super::collective::ZkAmsMkheStreamingCollectiveEvalKeyBindingV1::test_from_verified_axes_v1(
        &fixture.roster,
        axes.transcript_digest,
        axes.collective_key_digest,
    )
    .unwrap();
    ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1::new_from_compact_cpk_v1(
        &fixture.roster,
        axes.transcript_digest,
        binding,
        &manifest,
        manifest.manifest_digest(),
    )
    .unwrap()
}
#[test]
fn release_and_tiny_evidence_counts_are_distinct_and_exact() {
    let release = release_profile_v1();
    assert_eq!(release.gadget_digits, 38);
    assert_eq!(release.moduli.len(), 38);
    assert_eq!(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, 8);
    assert_eq!(evidence_set::RELEASE_RELIN_PAIR_COUNT_V1, 36);
    assert_eq!(
        evidence_set::checked_evidence_record_counts_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            release.gadget_digits,
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
        )
        .unwrap(),
        evidence_set::EvidenceRecordCountsV1 {
            source: 21_888,
            cks: 38,
        }
    );
    assert_eq!(
        evidence_set::checked_evidence_record_counts_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
            release.gadget_digits,
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
        )
        .unwrap(),
        evidence_set::EvidenceRecordCountsV1 {
            source: 304,
            cks: 38,
        }
    );
    assert_eq!(test_profile().gadget_digits, 8);
    assert_eq!(
        evidence_set::checked_evidence_record_counts_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            8,
            8,
        )
        .unwrap(),
        evidence_set::EvidenceRecordCountsV1 {
            source: 4_608,
            cks: 8,
        }
    );
    assert_eq!(
        evidence_set::checked_evidence_record_counts_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
            8,
            8,
        )
        .unwrap(),
        evidence_set::EvidenceRecordCountsV1 { source: 64, cks: 8 }
    );
    for party in 0..8_u32 {
        assert_eq!(
            evidence_set::expected_source_descriptor_v1(
                ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
                party,
            )
            .unwrap(),
            (
                ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne,
                u8::try_from(party).unwrap(),
            )
        );
        assert_eq!(
            evidence_set::expected_source_descriptor_v1(
                ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
                8 + party,
            )
            .unwrap(),
            (
                ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo,
                u8::try_from(party).unwrap(),
            )
        );
        assert_eq!(
            evidence_set::expected_source_descriptor_v1(
                ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
                party,
            )
            .unwrap(),
            (
                ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource,
                u8::try_from(party).unwrap(),
            )
        );
    }
    assert_eq!(
        evidence_set::expected_source_descriptor_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            16,
        )
        .unwrap(),
        (ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne, 0)
    );
    assert_eq!(
        evidence_set::expected_source_descriptor_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            575,
        )
        .unwrap(),
        (ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo, 7)
    );
    assert_eq!(
        evidence_set::expected_source_descriptor_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            576,
        )
        .unwrap(),
        (ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne, 0)
    );
    assert_eq!(
        evidence_set::expected_source_descriptor_v1(
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
            8,
        )
        .unwrap(),
        (ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource, 0)
    );
}
#[test]
fn generator_and_receipt_collector_share_descriptor_set_recurrence() {
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    let fixture = evidence_capability_fixture(
        b"evidence-parity",
        17,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        1,
        schedule.entries[0].exponent,
    );
    let axes = source_stream::verified_evidence_context_summary_v1(&fixture.source_context)
        .unwrap()
        .axes;
    let mut sink = NoopEvidenceSink;
    let mut source = EvidenceHasher::new(
        fixture.entry.purpose(),
        fixture.entry.ordinal(),
        fixture.entry.galois_exponent(),
        axes.collective_key_digest,
        ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        &mut sink,
    )
    .unwrap();
    for record_index in 0..fixture.counts.source {
        let (kind, _) =
            evidence_set::expected_source_descriptor_v1(fixture.entry.purpose(), record_index)
                .unwrap();
        source
            .test_descriptor_record(
                record_index,
                kind,
                &synthetic_evidence_body(0x51, record_index),
            )
            .unwrap();
    }
    assert_eq!(
        source.finish(fixture.counts.source, &mut sink).unwrap(),
        fixture.entry.source_proof_set_digest()
    );
    let mut cks = EvidenceHasher::new(
        fixture.entry.purpose(),
        fixture.entry.ordinal(),
        fixture.entry.galois_exponent(),
        axes.collective_key_digest,
        ZkAmsMkheCollectiveEvidenceSetKindV1::Cks,
        &mut sink,
    )
    .unwrap();
    for digit_index in 0..fixture.counts.cks {
        cks.test_descriptor_record(
            digit_index,
            ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit,
            &synthetic_evidence_body(0x52, digit_index),
        )
        .unwrap();
    }
    assert_eq!(
        cks.finish(fixture.counts.cks, &mut sink).unwrap(),
        fixture.entry.cks_proof_set_digest()
    );
    let cap = mint_synthetic_evidence_capability(&fixture);
    assert!(format!("{cap:?}").contains("privately_sealed: true"));
}
#[test]
fn descriptor_set_recurrence_matches_independent_manual_framing_oracle() {
    let header = ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        kind: ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        ordinal: 1,
        galois_exponent: 3,
        collective_key_digest: [0x5a; 32],
    };
    let records: [_; 3] = std::array::from_fn(|record_index| {
        let record_index = u32::try_from(record_index).unwrap();
        let (canonical_bytes, canonical_digest) = synthetic_evidence_descriptor(0x6d, record_index);
        (
            record_index,
            ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource,
            canonical_bytes,
            canonical_digest,
        )
    });
    let mut production = evidence_set::EvidenceSetDigestV1::new(header).unwrap();
    for (record_index, kind, canonical_bytes, canonical_digest) in records {
        production
            .absorb_record(record_index, kind, canonical_bytes, canonical_digest)
            .unwrap();
    }
    let expected = production.finish(3).unwrap();
    assert_eq!(
        expected,
        manual_evidence_descriptor_set_digest_v1(header, records, 3)
    );
    let mut wrong_header = header;
    wrong_header.galois_exponent ^= 2;
    assert_ne!(
        expected,
        manual_evidence_descriptor_set_digest_v1(wrong_header, records, 3)
    );
    let mut wrong_kind = records;
    wrong_kind[1].1 = ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit;
    assert_ne!(
        expected,
        manual_evidence_descriptor_set_digest_v1(header, wrong_kind, 3)
    );
    let mut wrong_length = records;
    wrong_length[1].2 += 1;
    assert_ne!(
        expected,
        manual_evidence_descriptor_set_digest_v1(header, wrong_length, 3)
    );
    let mut wrong_index = records;
    wrong_index[1].0 = 7;
    assert_ne!(
        expected,
        manual_evidence_descriptor_set_digest_v1(header, wrong_index, 3)
    );
    let reordered = [records[1], records[0], records[2]];
    assert_ne!(
        expected,
        manual_evidence_descriptor_set_digest_v1(header, reordered, 3)
    );
    assert_ne!(
        expected,
        manual_evidence_descriptor_set_digest_v1(header, records, 2)
    );
}
#[test]
fn compact_output_recurrence_matches_independent_manual_framing_oracle() {
    let purpose = ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois;
    let ordinal = 1;
    let exponent = 3;
    let collective_key_digest = [0x6e; 32];
    let records: [_; 3] = std::array::from_fn(|digit_index| {
        let digit_index = u32::try_from(digit_index).unwrap();
        (digit_index, synthetic_compact_output_digest(digit_index))
    });
    let mut production = evidence_set::CksCompactOutputSetDigestV1::new(
        purpose,
        ordinal,
        exponent,
        collective_key_digest,
    )
    .unwrap();
    for (digit_index, digest) in records {
        production.absorb(digit_index, digest).unwrap();
    }
    let expected = production.finish(3).unwrap();
    assert_eq!(
        expected,
        manual_cks_compact_output_set_digest_v1(
            purpose,
            ordinal,
            exponent,
            collective_key_digest,
            records,
            3,
        )
    );
    for (wrong_purpose, wrong_ordinal, wrong_exponent, wrong_key) in [
        (
            ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization,
            ordinal,
            exponent,
            collective_key_digest,
        ),
        (purpose, ordinal + 1, exponent, collective_key_digest),
        (purpose, ordinal, exponent ^ 2, collective_key_digest),
        (purpose, ordinal, exponent, [0x6f; 32]),
    ] {
        assert_ne!(
            expected,
            manual_cks_compact_output_set_digest_v1(
                wrong_purpose,
                wrong_ordinal,
                wrong_exponent,
                wrong_key,
                records,
                3,
            )
        );
    }
    let mut wrong_index = records;
    wrong_index[1].0 = 7;
    assert_ne!(
        expected,
        manual_cks_compact_output_set_digest_v1(
            purpose,
            ordinal,
            exponent,
            collective_key_digest,
            wrong_index,
            3,
        )
    );
    let reordered = [records[1], records[0], records[2]];
    assert_ne!(
        expected,
        manual_cks_compact_output_set_digest_v1(
            purpose,
            ordinal,
            exponent,
            collective_key_digest,
            reordered,
            3,
        )
    );
    let mut wrong_digest = records;
    wrong_digest[1].1[0] ^= 1;
    assert_ne!(
        expected,
        manual_cks_compact_output_set_digest_v1(
            purpose,
            ordinal,
            exponent,
            collective_key_digest,
            wrong_digest,
            3,
        )
    );
    assert_ne!(
        expected,
        manual_cks_compact_output_set_digest_v1(
            purpose,
            ordinal,
            exponent,
            collective_key_digest,
            records,
            2,
        )
    );
}
#[test]
fn source_receipt_stream_rejects_missing_reorder_duplicate_extra_and_error() {
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    let fixture = evidence_capability_fixture(
        b"source-hostile",
        18,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        1,
        schedule.entries[0].exponent,
    );
    let cks = || (0..fixture.counts.cks).map(|index| Ok(synthetic_cks_receipt(&fixture, index)));
    let result = verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
        &fixture.source_context,
        &fixture.cks_context,
        &fixture.entry,
        (0..fixture.counts.source - 1).map(|index| Ok(synthetic_source_receipt(&fixture, index))),
        cks(),
    );
    assert!(matches!(result, Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)));
    for mapping in [1_u8, 2] {
        let source = (0..fixture.counts.source).map(|position| {
            let index = match (mapping, position) {
                (1, 0) => 1,
                (1, 1) => 0,
                (2, 1) => 0,
                _ => position,
            };
            Ok(synthetic_source_receipt(&fixture, index))
        });
        assert!(
            verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
                &fixture.source_context,
                &fixture.cks_context,
                &fixture.entry,
                source,
                cks(),
            )
            .is_err()
        );
    }
    let source = (0..fixture.counts.source)
        .map(|index| Ok(synthetic_source_receipt(&fixture, index)))
        .chain(std::iter::once(Ok(synthetic_source_receipt(
            &fixture,
            fixture.counts.source,
        ))));
    assert!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            source,
            cks(),
        )
        .is_err()
    );
    let source = (0..fixture.counts.source)
        .map(|index| Ok(synthetic_source_receipt(&fixture, index)))
        .chain(std::iter::once(Err(
            ZkAmsMkheErrorV1::InvalidAuthentication,
        )));
    assert!(matches!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            source,
            cks(),
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    ));
    let source = (0..fixture.counts.source).map(|index| {
        if index == 3 {
            Err(ZkAmsMkheErrorV1::InvalidAuthentication)
        } else {
            Ok(synthetic_source_receipt(&fixture, index))
        }
    });
    assert!(matches!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            source,
            cks(),
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    ));
}
#[test]
fn cks_receipt_stream_rejects_missing_reorder_duplicate_extra_and_error() {
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    let fixture = evidence_capability_fixture(
        b"cks-hostile",
        19,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        1,
        schedule.entries[0].exponent,
    );
    let source =
        || (0..fixture.counts.source).map(|index| Ok(synthetic_source_receipt(&fixture, index)));
    assert!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            source(),
            (0..fixture.counts.cks - 1).map(|index| Ok(synthetic_cks_receipt(&fixture, index))),
        )
        .is_err()
    );
    for mapping in [1_u8, 2] {
        let cks = (0..fixture.counts.cks).map(|position| {
            let index = match (mapping, position) {
                (1, 0) => 1,
                (1, 1) => 0,
                (2, 1) => 0,
                _ => position,
            };
            Ok(synthetic_cks_receipt(&fixture, index))
        });
        assert!(
            verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
                &fixture.source_context,
                &fixture.cks_context,
                &fixture.entry,
                source(),
                cks,
            )
            .is_err()
        );
    }
    let cks = (0..fixture.counts.cks)
        .map(|index| Ok(synthetic_cks_receipt(&fixture, index)))
        .chain(std::iter::once(Ok(synthetic_cks_receipt(
            &fixture,
            fixture.counts.cks,
        ))));
    assert!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            source(),
            cks,
        )
        .is_err()
    );
    let cks = (0..fixture.counts.cks)
        .map(|index| Ok(synthetic_cks_receipt(&fixture, index)))
        .chain(std::iter::once(Err(
            ZkAmsMkheErrorV1::InvalidAuthentication,
        )));
    assert!(matches!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            source(),
            cks,
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    ));
    let cks = (0..fixture.counts.cks).map(|index| {
        if index == 3 {
            Err(ZkAmsMkheErrorV1::InvalidAuthentication)
        } else {
            Ok(synthetic_cks_receipt(&fixture, index))
        }
    });
    assert!(matches!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            source(),
            cks,
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    ));
}
#[test]
fn evidence_capability_rejects_context_entry_and_private_seal_splices() {
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    let fixture = evidence_capability_fixture(
        b"evidence-splice-a",
        20,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        1,
        schedule.entries[0].exponent,
    );
    let other = evidence_capability_fixture(
        b"evidence-splice-b",
        21,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        1,
        schedule.entries[0].exponent,
    );
    assert!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &other.cks_context,
            &fixture.entry,
            std::iter::empty(),
            std::iter::empty(),
        )
        .is_err()
    );
    let mut bad_source = synthetic_source_receipt(&fixture, 0);
    source_stream::test_tamper_verified_evidence_receipt_seal_v1(&mut bad_source);
    assert!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            std::iter::once(Ok(bad_source)).chain(
                (1..fixture.counts.source)
                    .map(|index| Ok(synthetic_source_receipt(&fixture, index))),
            ),
            (0..fixture.counts.cks).map(|index| Ok(synthetic_cks_receipt(&fixture, index))),
        )
        .is_err()
    );
    let mut bad_cks = synthetic_cks_receipt(&fixture, 0);
    cks_stream::test_tamper_verified_evidence_receipt_seal_v1(&mut bad_cks);
    assert!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &fixture.entry,
            (0..fixture.counts.source).map(|index| Ok(synthetic_source_receipt(&fixture, index))),
            std::iter::once(Ok(bad_cks)).chain(
                (1..fixture.counts.cks).map(|index| Ok(synthetic_cks_receipt(&fixture, index))),
            ),
        )
        .is_err()
    );
    let bad_entry = ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
        fixture.entry.ordinal(),
        fixture.entry.purpose(),
        fixture.entry.galois_exponent(),
        fixture.entry.payload_offset(),
        fixture.entry.payload_bytes(),
        fixture.entry.payload_blake3(),
        [0x99; 32],
        fixture.entry.cks_proof_set_digest(),
    )
    .unwrap();
    assert!(
        verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
            &fixture.source_context,
            &fixture.cks_context,
            &bad_entry,
            (0..fixture.counts.source).map(|index| Ok(synthetic_source_receipt(&fixture, index))),
            (0..fixture.counts.cks).map(|index| Ok(synthetic_cks_receipt(&fixture, index))),
        )
        .is_err()
    );
    for (ordinal, exponent) in [
        (2, fixture.entry.galois_exponent()),
        (fixture.entry.ordinal(), fixture.entry.galois_exponent() ^ 2),
    ] {
        let wrong_coordinate = ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
            ordinal,
            fixture.entry.purpose(),
            exponent,
            fixture.entry.payload_offset(),
            fixture.entry.payload_bytes(),
            fixture.entry.payload_blake3(),
            fixture.entry.source_proof_set_digest(),
            fixture.entry.cks_proof_set_digest(),
        )
        .unwrap();
        assert!(
            verify_zk_ams_mkhe_evaluated_key_evidence_set_v1(
                &fixture.source_context,
                &fixture.cks_context,
                &wrong_coordinate,
                std::iter::empty(),
                std::iter::empty(),
            )
            .is_err()
        );
    }
}
#[test]
fn invalid_capability_fails_before_any_provider_operation() {
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    let fixture = evidence_capability_fixture(
        b"invalid-cap-before-provider",
        22,
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois,
        1,
        schedule.entries[0].exponent,
    );
    let mut capability = mint_synthetic_evidence_capability(&fixture);
    evidence_set::test_tamper_capability_seal_v1(&mut capability);
    let runtime = evidence_test_runtime(&fixture);
    let mut provider = TestArtifact::new(&test_profile());
    assert!(
        runtime
            .validate_seekable_key_provider(
                usize::from(fixture.entry.ordinal()),
                capability,
                &mut provider,
            )
            .is_err()
    );
    assert_eq!(provider.provider_identity_calls.get(), 0);
    assert_eq!(provider.provider_pointer_calls.get(), 0);
    assert_eq!(provider.provider_snapshot_calls, 0);
    assert_eq!(provider.provider_payload_len_calls, 0);
    assert_eq!(provider.seek_calls, 0);
    assert_eq!(provider.read_calls, 0);
    for field in 0..5 {
        let capability = mint_synthetic_evidence_capability(&fixture);
        let mut wrong_binding = evidence_runtime_binding(&fixture);
        wrong_binding.entry = ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
            fixture.entry.ordinal(),
            fixture.entry.purpose(),
            fixture.entry.galois_exponent(),
            fixture.entry.payload_offset() + if field == 0 { 1 } else { 0 },
            fixture.entry.payload_bytes() + if field == 1 { 1 } else { 0 },
            if field == 2 {
                [0x77; 32]
            } else {
                fixture.entry.payload_blake3()
            },
            if field == 3 {
                [0x78; 32]
            } else {
                fixture.entry.source_proof_set_digest()
            },
            if field == 4 {
                [0x79; 32]
            } else {
                fixture.entry.cks_proof_set_digest()
            },
        )
        .unwrap();
        assert!(consume_evidence_set_before_provider_v1(capability, wrong_binding).is_err());
    }
    for axis in 0..7 {
        let capability = mint_synthetic_evidence_capability(&fixture);
        let mut wrong_binding = evidence_runtime_binding(&fixture);
        match axis {
            0 => wrong_binding.profile_digest[0] ^= 1,
            1 => wrong_binding.roster_digest[0] ^= 1,
            2 => wrong_binding.key_material_digest[0] ^= 1,
            3 => wrong_binding.epoch += 1,
            4 => wrong_binding.transcript_digest[0] ^= 1,
            5 => wrong_binding.collective_key_digest[0] ^= 1,
            6 => {
                wrong_binding.entry = ZkAmsMkheCollectiveEvaluatedKeyEntryV1::new(
                    fixture.entry.ordinal(),
                    fixture.entry.purpose(),
                    fixture.entry.galois_exponent(),
                    fixture.entry.payload_offset(),
                    fixture.entry.payload_bytes(),
                    fixture.entry.payload_blake3(),
                    [0x81; 32],
                    fixture.entry.cks_proof_set_digest(),
                )
                .unwrap()
            }
            _ => unreachable!(),
        }
        assert!(consume_evidence_set_before_provider_v1(capability, wrong_binding).is_err());
    }
}
#[test]
fn zark_scan_rejects_tampered_or_swapped_expected_cks_outputs() {
    let profile = test_profile();
    let mut artifact = TestArtifact::new(&profile);
    let (generated, _) = published_test_key(&profile, &mut artifact).unwrap();
    let pointer = artifact.attach_pointer();
    let expected = expected_published_test_key(&profile, &generated, pointer);
    assert!(validate_seekable_evaluated_key(&profile, expected, &mut artifact.clone()).is_ok());
    let mut tampered = expected;
    tampered.cks_compact_output_set_digest[0] ^= 1;
    assert!(matches!(
        validate_seekable_evaluated_key(&profile, tampered, &mut artifact.clone()),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    ));
    let swapped_output_digest = manual_cks_compact_output_set_digest_v1(
        expected.entry.purpose(),
        expected.entry.ordinal(),
        expected.entry.galois_exponent(),
        expected.collective_key_digest,
        (0..profile.gadget_digits).map(|digit_index| {
            let source_digit = match digit_index {
                0 => 1,
                1 => 0,
                _ => digit_index,
            };
            let values: [u64; 8] = std::array::from_fn(|coefficient| {
                (source_digit as u64 + 3) * (coefficient as u64 + 7)
                    + u64::from(expected.entry.ordinal())
                    + 11
            });
            let digit = RnsPolynomial::from_unsigned(&profile, &values).unwrap();
            let mut digest =
                new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, digit.coefficients.len())
                    .unwrap();
            update_rns_digest_hasher(&mut digest, &digit.coefficients);
            (u32::try_from(digit_index).unwrap(), digest.finalize())
        }),
        u32::try_from(profile.gadget_digits).unwrap(),
    );
    let mut swapped = expected;
    swapped.cks_compact_output_set_digest = swapped_output_digest;
    assert!(matches!(
        validate_seekable_evaluated_key(&profile, swapped, &mut artifact),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    ));
}
#[test]
fn evidence_capability_is_opaque_move_only_bounded_and_facaded() {
    let child = include_str!("collective_eval_keys/evidence_set.rs");
    let production = child.split("#[cfg(test)]").next().unwrap_or(child);
    assert!(production.lines().count() <= 1_200);
    let name = "pub struct ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1";
    let position = production.find(name).expect("public opaque capability");
    let prelude = &production[position.saturating_sub(192)..position];
    assert!(!prelude.contains("derive(Clone"));
    assert!(!prelude.contains("derive(Copy"));
    let shape = production[position..]
        .split("impl core::fmt::Debug")
        .next()
        .expect("capability shape");
    for forbidden in ["Vec<", "reader:", "provider:", "payload: Vec", "receipt:"] {
        assert!(
            !shape.contains(forbidden),
            "forbidden cap owner: {forbidden}"
        );
    }
    assert!(shape.contains("payload_offset: u64"));
    assert!(shape.contains("payload_bytes: u64"));
    assert!(shape.contains("payload_blake3: [u8; 32]"));
    assert!(production.contains("It does not certify cross-set source-output algebraic equality"));
    let gateway = production
        .split("pub fn verify_zk_ams_mkhe_evaluated_key_evidence_set_v1")
        .nth(1)
        .expect("evidence-set gateway")
        .split("fn evidence_set_capability_seal_v1")
        .next()
        .expect("evidence-set gateway boundary");
    let source_context = gateway
        .find("source_stream::verified_evidence_context_summary_v1")
        .expect("source context is resealed first");
    let cks_context = gateway
        .find("cks_stream::verified_evidence_context_summary_v1")
        .expect("CKS context is resealed second");
    let entry_coordinate = gateway
        .find("validate_entry_coordinate_v1")
        .expect("entry coordinate preflight");
    assert!(source_context < cks_context && cks_context < entry_coordinate);
    let runtime = include_str!("collective_eval_keys/runtime.rs");
    let admission = runtime
        .split("pub fn validate_seekable_key_provider")
        .nth(1)
        .expect("runtime admission")
        .split("fn entry(")
        .next()
        .expect("runtime admission boundary");
    assert!(
        admission
            .find("consume_evidence_set_before_provider_v1")
            .unwrap()
            < admission.find("validate_seekable_evaluated_key").unwrap()
    );
    assert!(admission.contains("cks_compact_output_set_digest"));
    let parent = include_str!("collective_eval_keys.rs");
    assert!(parent.lines().count() <= 5_000);
    for facade in [
        include_str!("../mkhe.rs"),
        include_str!("../../zk_ams.rs"),
        include_str!("../../../vega.rs"),
    ] {
        assert!(facade.contains("ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1"));
        assert!(facade.contains("verify_zk_ams_mkhe_evaluated_key_evidence_set_v1"));
    }
}
