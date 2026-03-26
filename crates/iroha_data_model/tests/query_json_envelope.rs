//! Integration tests for the JSON query envelope DSL.

#[cfg(feature = "json")]
mod json_envelope {
    use iroha_data_model::{
        account::AccountId,
        query::{
            QueryRequest,
            json::QueryEnvelopeJson,
            parameters::{FetchSize, SortOrder},
        },
    };

    const AUTHORITY: &str = "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ";
    const ALT_AUTHORITY: &str = "soraゴヂアヌメネヒョタルアキュカンコプヱガョラツゴヸナゥヘガヮザネチョヷニャヒュニョメヺェヅヤアキャヅアタタナイス";

    fn parse_authority(literal: &str) -> AccountId {
        AccountId::parse_encoded(literal)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("authority")
    }

    #[test]
    fn iterable_envelope_parses_params() {
        let json = r#"{
            "iterable": {
                "type": "FindDomains",
                "params": {
                    "limit": 5,
                    "offset": 1,
                    "fetch_size": 10,
                    "sort_by_metadata_key": "ui.order",
                    "order": "Desc"
                },
                "predicate": {
                    "equals": [
                        {"field": "authority", "value": "__AUTHORITY__"}
                    ]
                }
            }
        }"#
        .replace("__AUTHORITY__", AUTHORITY);

        let envelope: QueryEnvelopeJson = norito::json::from_str(&json).expect("parse envelope");
        let authority = parse_authority(AUTHORITY);
        let request = envelope
            .into_signed_request(authority.clone())
            .expect("build signed request");
        assert_eq!(request.authority(), &authority);

        let QueryRequest::Start(query_with_params) = request.request() else {
            panic!("expected iterable envelope");
        };
        let params = query_with_params.params();

        let pagination = params.pagination();
        assert_eq!(pagination.offset_value(), 1);
        assert_eq!(
            pagination.limit_value().map(std::num::NonZeroU64::get),
            Some(5)
        );

        let fetch: &FetchSize = params.fetch_size();
        assert_eq!(fetch.value().map(std::num::NonZeroU64::get), Some(10));

        let sorting = params.sorting();
        assert_eq!(
            sorting
                .sort_by_metadata_key()
                .map(std::string::ToString::to_string)
                .as_deref(),
            Some("ui.order")
        );
        assert!(matches!(sorting.order(), Some(SortOrder::Desc)));
    }

    #[test]
    fn iterable_order_without_key_errors() {
        let json = r#"{"iterable": {"type": "FindDomains", "params": {"order": "Asc"}}}"#;
        let envelope: QueryEnvelopeJson = norito::json::from_str(json).expect("parse envelope");
        let authority = parse_authority(ALT_AUTHORITY);
        let err = match envelope.into_signed_request(authority) {
            Ok(_) => panic!("order without sort key must error"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("sort order"));
    }

    #[test]
    fn unknown_iterable_field_is_rejected() {
        let json = r#"{
            "iterable": {
                "type": "FindDomains",
                "params": {"limit": 1},
                "bogus": true
            }
        }"#;

        let err = norito::json::from_str::<QueryEnvelopeJson>(json).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown field `bogus` inside `iterable`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn unknown_params_field_is_rejected() {
        let json = r#"{
            "iterable": {
                "type": "FindDomains",
                "params": {"limit": 1, "extra": 5}
            }
        }"#;

        let err = norito::json::from_str::<QueryEnvelopeJson>(json).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown field `extra` inside `params`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn singular_contract_manifest_roundtrip() {
        let json = r#"{
            "singular": {
                "type": "FindContractManifestByCodeHash",
                "payload": {"code_hash": "0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"}
            }
        }"#;
        let envelope: QueryEnvelopeJson = norito::json::from_str(json).expect("parse envelope");
        let authority = parse_authority(AUTHORITY);
        let request = envelope
            .into_signed_request(authority)
            .expect("build signed request");
        let QueryRequest::Singular(query_box) = request.request() else {
            panic!("expected singular envelope");
        };
        assert!(matches!(
            *query_box,
            iroha_data_model::query::SingularQueryBox::FindContractManifestByCodeHash(_)
        ));
    }
}
