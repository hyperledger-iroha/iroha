#[cfg(all(test, feature = "app_api", feature = "telemetry"))]
mod kaigi_response_format_tests {
    use super::*;
    use axum::http::header::CONTENT_TYPE;
    use http_body_util::BodyExt as _;

    fn kaigi_relay_metadata_test_account(seed: u8) -> AccountId {
        AccountId::new(
            checked_routing_fixture_keypair(
                seed,
                Algorithm::Ed25519,
                "derive Kaigi relay metadata test account",
            )
            .public_key()
            .clone(),
        )
    }

    fn assert_kaigi_relay_metadata_error(error: Error, expected: &str) {
        let Error::Query(iroha_data_model::ValidationFail::InternalError(message)) = error else {
            panic!("unexpected Kaigi relay metadata error: {error:?}");
        };
        assert!(message.contains(expected), "{message}");
    }

    #[test]
    fn kaigi_relay_diagnostic_count_fails_closed_at_the_hard_cap() {
        let mut count = 0usize;
        for _ in 0..KAIGI_RELAY_DIAGNOSTIC_MAX_RELAYS {
            increment_kaigi_relay_diagnostic_count(&mut count)
                .expect("relay count within the hard cap");
        }
        assert_eq!(count, KAIGI_RELAY_DIAGNOSTIC_MAX_RELAYS);
        assert!(matches!(
            increment_kaigi_relay_diagnostic_count(&mut count),
            Err(Error::Query(iroha_data_model::ValidationFail::TooComplex))
        ));
        assert_eq!(count, KAIGI_RELAY_DIAGNOSTIC_MAX_RELAYS);
    }

    #[test]
    fn kaigi_relay_metadata_decoders_accept_matching_records() {
        let relay_id = kaigi_relay_metadata_test_account(0x51);
        let registration = KaigiRelayRegistration {
            relay_id: relay_id.clone(),
            hpke_public_key: vec![0xA5; 32],
            bandwidth_class: 3,
        };
        let registration_key =
            iroha_data_model::kaigi::kaigi_relay_metadata_key(&relay_id).expect("relay key");
        let registration_value = IrohaJson::new(registration.clone());
        assert_eq!(
            decode_kaigi_relay_registration(&registration_key, &registration_value)
                .expect("matching registration metadata"),
            registration
        );

        let feedback = KaigiRelayFeedback {
            relay_id: relay_id.clone(),
            call: KaigiId::new(
                DomainId::try_new("kaigi", "universal").expect("call domain"),
                "relay-health".parse().expect("call name"),
            ),
            reported_by: relay_id.clone(),
            status: KaigiRelayHealthStatus::Healthy,
            reported_at_ms: 7,
            notes: None,
        };
        let feedback_value = IrohaJson::new(feedback.clone());
        assert_eq!(
            decode_kaigi_relay_feedback(&relay_id, &feedback_value)
                .expect("matching feedback metadata"),
            feedback
        );
    }

    #[test]
    fn kaigi_relay_metadata_rejects_registration_key_mismatch() {
        let embedded_relay = kaigi_relay_metadata_test_account(0x52);
        let keyed_relay = kaigi_relay_metadata_test_account(0x53);
        let key = iroha_data_model::kaigi::kaigi_relay_metadata_key(&keyed_relay)
            .expect("mismatched relay key");
        let value = IrohaJson::new(KaigiRelayRegistration {
            relay_id: embedded_relay,
            hpke_public_key: vec![0x5A; 32],
            bandwidth_class: 2,
        });
        let error = decode_kaigi_relay_registration(&key, &value)
            .expect_err("registration key mismatch must fail closed");
        assert_kaigi_relay_metadata_error(error, "does not match");
    }

    #[test]
    fn kaigi_relay_metadata_rejects_empty_hpke_key() {
        let relay_id = kaigi_relay_metadata_test_account(0x57);
        let key = iroha_data_model::kaigi::kaigi_relay_metadata_key(&relay_id)
            .expect("relay metadata key");
        let value = IrohaJson::new(KaigiRelayRegistration {
            relay_id,
            hpke_public_key: Vec::new(),
            bandwidth_class: 1,
        });
        let error = decode_kaigi_relay_registration(&key, &value)
            .expect_err("empty HPKE keys must fail closed");
        assert_kaigi_relay_metadata_error(error, "HPKE public key");
    }

    #[test]
    fn kaigi_relay_metadata_rejects_zero_bandwidth() {
        let relay_id = kaigi_relay_metadata_test_account(0x59);
        let key = iroha_data_model::kaigi::kaigi_relay_metadata_key(&relay_id)
            .expect("relay metadata key");
        let value = IrohaJson::new(KaigiRelayRegistration {
            relay_id,
            hpke_public_key: vec![0x5A; 32],
            bandwidth_class: 0,
        });
        let error = decode_kaigi_relay_registration(&key, &value)
            .expect_err("zero relay bandwidth must fail closed");
        assert_kaigi_relay_metadata_error(error, "non-zero bandwidth class");
    }

    #[test]
    fn kaigi_relay_metadata_rejects_malformed_feedback() {
        let relay_id = kaigi_relay_metadata_test_account(0x54);
        let malformed = IrohaJson::new("not relay feedback");
        let error = decode_kaigi_relay_feedback(&relay_id, &malformed)
            .expect_err("malformed feedback must fail closed");
        assert_kaigi_relay_metadata_error(error, "failed to decode");
    }

    #[test]
    fn kaigi_relay_metadata_rejects_feedback_relay_mismatch() {
        let expected_relay = kaigi_relay_metadata_test_account(0x55);
        let embedded_relay = kaigi_relay_metadata_test_account(0x56);
        let value = IrohaJson::new(KaigiRelayFeedback {
            relay_id: embedded_relay.clone(),
            call: KaigiId::new(
                DomainId::try_new("kaigi", "universal").expect("call domain"),
                "relay-health".parse().expect("call name"),
            ),
            reported_by: embedded_relay,
            status: KaigiRelayHealthStatus::Unavailable,
            reported_at_ms: 9,
            notes: Some("mismatched relay".to_owned()),
        });
        let error = decode_kaigi_relay_feedback(&expected_relay, &value)
            .expect_err("feedback relay mismatch must fail closed");
        assert_kaigi_relay_metadata_error(error, "does not match");
    }

    #[tokio::test]
    async fn kaigi_json_document_response_renders_json() {
        let payload = KaigiRelaySummaryListDto {
            total: 1,
            items: Vec::new(),
        };
        let response =
            respond_kaigi_json_document_with_format(&payload, crate::utils::ResponseFormat::Json);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect JSON body")
            .to_bytes();
        let decoded: norito::json::Value =
            norito::json::from_slice(&bytes).expect("decode JSON body");
        assert_eq!(decoded["total"].as_u64(), Some(1));
    }
    #[tokio::test]
    async fn kaigi_json_document_response_wraps_json_string_as_norito() {
        let payload = KaigiRelaySummaryListDto {
            total: 1,
            items: Vec::new(),
        };
        let response =
            respond_kaigi_json_document_with_format(&payload, crate::utils::ResponseFormat::Norito);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some(crate::utils::NORITO_MIME_TYPE)
        );
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect Norito body")
            .to_bytes();
        let json: String = norito::decode_from_bytes(&bytes).expect("decode Norito JSON string");
        let decoded: norito::json::Value = norito::json::from_str(&json).expect("decode JSON body");
        assert_eq!(decoded["total"].as_u64(), Some(1));
    }
}
