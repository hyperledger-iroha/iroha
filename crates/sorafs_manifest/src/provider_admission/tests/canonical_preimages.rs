// Admission digest preimages remain exact under every supported caller layout.
#[test]
fn admission_digest_preimages_ignore_caller_layout() {
    #[derive(NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(
        name = "sorafs_manifest::provider_admission::tests::admission_digest_preimages_ignore_caller_layout::ReviewedRevocationBody",
        frame = "sorafs_manifest::provider_admission::ProviderAdmissionRevocationV1::digest::RevocationBody<'_>"
    )]
    struct ReviewedRevocationBody<'a> {
        version: u8,
        provider_id: [u8; 32],
        envelope_digest: [u8; 32],
        revoked_at: u64,
        reason: &'a str,
        #[norito(default)]
        notes: Option<&'a str>,
    }
    fn independently_hash_frame<T: norito::NoritoSerialize>(domain: &[u8], value: &T) -> [u8; 32] {
        let frame = norito::encode_canonical(value).expect("independent canonical preimage");
        let mut preimage = domain.to_vec();
        preimage.extend_from_slice(&frame);
        *blake3::hash(&preimage).as_bytes()
    }
    let council_key = SigningKey::from_bytes(&[0xA9; 32]);
    let policy = council_policy(&[&council_key], 1);
    let envelope = signed_sample_envelope(&[&council_key]);
    let mut unsigned = envelope.clone();
    unsigned.council_signatures.clear();
    let proposal_digest = independently_hash_frame(PROPOSAL_DIGEST_DOMAIN, &envelope.proposal);
    let advert_digest = independently_hash_frame(ADVERT_BODY_DIGEST_DOMAIN, &envelope.advert_body);
    let authorization_digest =
        independently_hash_frame(ENVELOPE_AUTHORIZATION_DIGEST_DOMAIN, &unsigned);
    let envelope_digest = independently_hash_frame(ENVELOPE_DIGEST_DOMAIN, &envelope);
    let mut revocation = ProviderAdmissionRevocationV1 {
        version: PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
        provider_id: envelope.proposal.provider_id,
        envelope_digest,
        revoked_at: 10,
        reason: "endpoint compromise".to_owned(),
        council_signatures: Vec::new(),
        notes: Some("rotation".to_owned()),
    };
    // Independently spell the preserved V1 body and its exact schema identity;
    // neither the preimage nor the signature below comes from `revocation.digest()`.
    let reviewed_body = ReviewedRevocationBody {
        version: revocation.version,
        provider_id: revocation.provider_id,
        envelope_digest: revocation.envelope_digest,
        revoked_at: revocation.revoked_at,
        reason: &revocation.reason,
        notes: revocation.notes.as_deref(),
    };
    let revocation_digest = independently_hash_frame(REVOCATION_DIGEST_DOMAIN, &reviewed_body);
    {
        use norito::{NoritoSchema as _, core as ncore, json::Value};
        let expected: Vec<Value> =
            include_str!("../../../tests/fixtures/provider_admission_revocation_identity.jsonl")
                .lines()
                .map(|line| norito::json::from_str(line).expect("captured revocation identity"))
                .collect();
        assert_eq!(expected.len(), 2);
        assert_eq!(
            expected[0]["owner"].as_str(),
            Some("reviewed_revocation_body")
        );
        assert_eq!(
            expected[1]["owner"].as_str(),
            Some("production_revocation_body")
        );
        let frame = norito::encode_canonical(&reviewed_body).expect("reviewed revocation frame");
        let view = ncore::from_bytes_view(&frame).expect("validated revocation archive");
        let active_hash = norito::schema::identity::frame_hash::<ReviewedRevocationBody<'_>>();
        let nominal_name = ReviewedRevocationBody::nominal_name();
        assert_eq!(view.schema(), active_hash);
        assert_eq!(
            norito::canonical_frame_len(&reviewed_body).expect("count revocation frame"),
            frame.len()
        );
        let actual = norito::json!({
            "owner": "reviewed_revocation_body",
            "compiler_nominal_name": nominal_name,
            "compiler_nominal_hash": (hex::encode(ncore::schema_hash_for_name(&nominal_name))),
            "active_serialize_hash": (hex::encode(active_hash)),
            "advertised_schema_hash": (hex::encode(view.schema())),
            "canonical_flags": (view.flags()),
            "canonical_frame_hex": (hex::encode(&frame)),
            "canonical_bare_hex": (hex::encode(view.as_bytes())),
        });
        assert_eq!(actual, expected[0]);
        assert_eq!(
            ReviewedRevocationBody::frame_name(),
            expected[1]["compiler_nominal_name"]
                .as_str()
                .expect("production nominal")
        );
        assert_ne!(
            ReviewedRevocationBody::nominal_name(),
            ReviewedRevocationBody::frame_name()
        );
        assert_eq!(
            expected[1]["compiler_nominal_hash"],
            actual["active_serialize_hash"]
        );
        for field in [
            "active_serialize_hash",
            "advertised_schema_hash",
            "canonical_flags",
            "canonical_frame_hex",
            "canonical_bare_hex",
        ] {
            assert_eq!(
                actual[field], expected[1][field],
                "production projection {field}"
            );
        }
    }
    revocation.council_signatures =
        vec![council_signature_from_key(&council_key, &revocation_digest)];
    let expected = [
        proposal_digest,
        advert_digest,
        authorization_digest,
        envelope_digest,
        revocation_digest,
    ];
    use norito::core::header_flags::{COMPACT_LEN, PACKED_SEQ, PACKED_STRUCT};
    let layouts = crate::canonical_test_support::supported_layouts()
        .into_iter()
        .chain([
            PACKED_SEQ | PACKED_STRUCT,
            PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        ]);
    assert_eq!(layouts.clone().count(), 10);
    for flags in layouts {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            [
                compute_proposal_digest(&envelope.proposal).unwrap(),
                compute_advert_body_digest(&envelope.advert_body).unwrap(),
                compute_envelope_authorization_digest(&envelope).unwrap(),
                compute_envelope_digest(&envelope).unwrap(),
                revocation.digest().unwrap(),
            ],
            expected,
            "layout {flags:#04x}"
        );
        let record = AdmissionRecord::new(envelope.clone(), &policy)
            .expect("same independently authenticated admission under each caller layout");
        assert_eq!(record.envelope_digest(), &envelope_digest);
        record.verify_revocation(&revocation, &policy).unwrap();
        let mut tampered = revocation.clone();
        tampered.reason.push('!');
        assert_ne!(tampered.digest().unwrap(), revocation_digest);
        assert!(verify_revocation_signatures(&tampered, &policy).is_err());
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}
