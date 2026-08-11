#[cfg(all(test, feature = "app_api", feature = "telemetry"))]
mod kaigi_response_format_tests {
    use axum::http::header::CONTENT_TYPE;
    use http_body_util::BodyExt as _;

    use super::*;

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
