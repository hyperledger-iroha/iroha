// Transparency source-entry and proof-token regressions.
#[test]
fn transparency_ledger_source_entries_reject_duplicate_and_out_of_window() {
    let cycle_id = *b"cycle-src-test01";
    let duplicate = transparency_source_entry(
        "gar-1",
        120,
        ModerationLedgerEntryKindV1::GarEnforcementReceipt,
        "gar-receipt-1",
        0x50,
    );
    let err = build_transparency_ledger_entries_from_source_events(
        cycle_id,
        100,
        200,
        201,
        &[duplicate.clone(), duplicate],
    )
    .expect_err("duplicate event rejected");
    assert!(matches!(
        err,
        TransparencyLedgerIngestError::DuplicateSourceEntry { .. }
    ));
    let err = build_transparency_ledger_entries_from_source_events(
        cycle_id,
        100,
        200,
        201,
        &[transparency_source_entry(
            "future-1",
            200,
            ModerationLedgerEntryKindV1::EvidenceAccess,
            "evidence-view-1",
            0x80,
        )],
    )
    .expect_err("out-of-window event rejected");
    assert!(matches!(
        err,
        TransparencyLedgerIngestError::EntryOutsideCycle { .. }
    ));
}
#[test]
fn proof_token_issuance_from_base64_verifies_and_derives_public_record() {
    let issuance = proof_token_issuance_from_base64(
        VALID_PROOF_TOKEN_B64,
        valid_signer_key(),
        Some([0x65; 32]),
        Some([0x66; 32]),
        vec![ModerationLedgerMetadataV1 {
            key: "issuer".to_string(),
            value: "gateway-a".to_string(),
        }],
    )
    .expect("valid proof-token issuance");
    assert_eq!(issuance.version, PROOF_TOKEN_ISSUANCE_VERSION_V1);
    assert_eq!(issuance.token_id, [0x61; 16]);
    assert_eq!(issuance.issued_at_unix, 1_800_000_030);
    assert_eq!(issuance.expires_at_unix, Some(1_800_086_430));
    assert_eq!(issuance.moderation_action_code, 2);
    assert_eq!(issuance.signer_key, valid_signer_key());
    assert_ne!(issuance.token_blake3, [0; 32]);
    assert_eq!(issuance.blinded_digest, [0x64; 32]);
    assert_eq!(
        issuance.entry_ids,
        vec!["denylist/global".to_string(), "gar/policy/42".to_string()]
    );
    assert_eq!(issuance.evidence_digest, Some([0x65; 32]));
    assert_eq!(issuance.policy_digest, Some([0x66; 32]));
    assert_eq!(issuance.metadata[0].key, "issuer");
}
#[test]
fn proof_token_issuance_from_base64_rejects_bad_signature_key() {
    let mut signer_key = valid_signer_key();
    signer_key[0] ^= 0x01;
    let err = proof_token_issuance_from_base64(
        VALID_PROOF_TOKEN_B64,
        signer_key,
        Some([0x65; 32]),
        None,
        Vec::new(),
    )
    .expect_err("wrong signer key must fail");
    assert!(matches!(
        err,
        ProofTokenIssuanceIngestError::InvalidSignature { .. }
    ));
}
