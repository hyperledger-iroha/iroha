// Complete producer/transport window reconstruction, with real signatures and durable simulation.
#[test]
fn stream_transport_commits_body_window_and_recovers_exact_receipt_under_all_layouts() {
    let mut baseline: Option<Vec<u8>> = None;
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let harness = Harness::new();
        let body = body(40);
        let expected = harness.source.register(&body);
        let receipt = Client::sign(harness.service(), &expected, &body).unwrap();
        let decoded = SignerStreamTokenReceiptV1::decode_canonical(receipt.bytes()).unwrap();
        assert_eq!(
            decoded.request.issued_at_unix_ms,
            body.issued_at.checked_mul(1_000).unwrap()
        );
        assert_eq!(
            decoded.request.expires_at_unix_ms,
            body.ttl_epoch.checked_mul(1_000).unwrap()
        );
        assert_eq!(
            decoded.request.issued_at_unix_ms,
            expected.issued_at_unix_ms()
        );
        assert_eq!(
            decoded.request.expires_at_unix_ms,
            expected.expires_at_unix_ms()
        );
        assert_eq!(
            decoded.intent.request_digest,
            decoded.request.digest().unwrap()
        );
        assert_eq!(decoded.intent.operation_id, expected.operation_id());
        Signature::try_from_bytes(&decoded.signatures[0].signature)
            .unwrap()
            .verify(&harness.source.base.binding.public_key, &payload(&body))
            .unwrap();
        if let Some(bytes) = &baseline {
            assert_eq!(receipt.bytes(), bytes.as_slice());
        } else {
            baseline = Some(receipt.bytes().to_vec());
        }
        let before = harness.source.counts();
        let recovered = Client::recover(harness.service(), &expected, &body).unwrap();
        let after = harness.source.counts();
        assert_eq!(recovered.bytes(), receipt.bytes());
        assert_eq!(
            (after.signing, after.reserves, after.commits),
            (before.signing, before.reserves, before.commits)
        );
        assert_eq!(harness.calls(), 4);
    }
}

#[test]
fn stream_transport_rejects_either_changed_body_time_before_source_or_hardware() {
    let harness = Harness::with_custody_expiry(5_000);
    let mut body = body(41);
    body.ttl_epoch = 3;
    let expected = harness.source.register(&body);
    let before = harness.source.counts();
    for change_issue in [true, false] {
        let mut changed = body.clone();
        if change_issue {
            changed.issued_at += 1;
        } else {
            changed.ttl_epoch += 1;
        }
        let actual =
            SignerStreamTokenExpectedV1::new(&changed, &harness.source.base.binding).unwrap();
        assert_ne!(actual, expected, "changed body remains structurally valid");
        assert_eq!(
            Client::sign(harness.service(), &expected, &changed).unwrap_err(),
            Error::Refused
        );
        assert_eq!(
            Client::recover(harness.service(), &expected, &changed).unwrap_err(),
            Error::Refused
        );
        assert_eq!(harness.source.counts(), before);
        assert_eq!(harness.calls(), 0);
    }
}
