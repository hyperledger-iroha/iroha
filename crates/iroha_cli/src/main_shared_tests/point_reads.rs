//! Point reads submit exact singular queries instead of scanning whole collections.

use super::*;
use iroha::data_model::query::{QueryRequest, SignedQuery, SingularQueryBox};
use iroha_version::codec::DecodeVersioned;

#[derive(Debug)]
struct RecordingTransport {
    requests: std::sync::Mutex<Vec<iroha::http::TransportRequest>>,
    responses: std::sync::Mutex<std::collections::VecDeque<iroha::http::Response<Vec<u8>>>>,
}
impl iroha::http::HttpTransport for RecordingTransport {
    fn send_blocking(
        &self,
        request: iroha::http::TransportRequest,
    ) -> Result<iroha::http::Response<Vec<u8>>> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| eyre!("unexpected extra point-read request"))
    }
    fn send(&self, request: iroha::http::TransportRequest) -> iroha::http::TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}
struct PointReadContext {
    config: Config,
    client: Client,
    i18n: Localizer,
}
impl RunContext for PointReadContext {
    fn config(&self) -> &Config {
        &self.config
    }
    fn transaction_metadata(&self) -> Option<&Metadata> {
        None
    }
    fn input_instructions(&self) -> bool {
        false
    }
    fn output_instructions(&self) -> bool {
        false
    }
    fn i18n(&self) -> &Localizer {
        &self.i18n
    }
    fn print_data<T: JsonSerialize + ?Sized>(&mut self, _data: &T) -> Result<()> {
        Ok(())
    }
    fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
        Ok(())
    }
    fn client_from_config(&self) -> Result<Client> {
        Ok(self.client.clone())
    }
}

/// Run one CLI read against a node that reports the entity as missing and return the query sent.
fn singular_query_sent_by(argv: &[&str]) -> SingularQueryBox {
    let capabilities = iroha::http::Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(
            format!(
                "{{\"data_model_version\":{}}}",
                iroha::data_model::DATA_MODEL_VERSION
            )
            .into_bytes(),
        )
        .unwrap();
    let missing = iroha::http::Response::builder()
        .status(404)
        .header("content-type", "application/x-norito")
        .body(
            norito::to_bytes(&iroha_torii_shared::ErrorEnvelope::new(
                "query_validation_failed",
                "entity not found".to_owned(),
            ))
            .unwrap(),
        )
        .unwrap();
    let transport = std::sync::Arc::new(RecordingTransport {
        requests: std::sync::Mutex::new(Vec::new()),
        responses: std::sync::Mutex::new(vec![capabilities, missing].into()),
    });
    let config = fallback_config();
    let client = Client::builder(config.clone())
        .http_transport(transport.clone())
        .build()
        .expect("point-read client");
    let mut context = PointReadContext {
        config,
        client,
        i18n: Localizer::new(Bundle::Cli, Language::English),
    };
    let error = Args::try_parse_from(argv)
        .expect("parse point read")
        .command
        .run(&mut context)
        .expect_err("a missing entity remains an error");
    assert!(
        format!("{error:#}").contains("entity not found"),
        "{error:#}"
    );
    let requests = transport.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "a point read sends only the capability probe and one singular query"
    );
    assert_eq!(requests[1].url.path(), "/v1/query");
    let signed = SignedQuery::decode_all_versioned(&requests[1].body).unwrap();
    let QueryRequest::Singular(query) = signed.request() else {
        panic!("a point read must submit a singular query, never a collection scan");
    };
    query.clone()
}

fn wonderland() -> DomainId {
    DomainId::try_new("wonderland", "universal").unwrap()
}

#[test]
fn domain_get_and_meta_get_use_find_domain_by_id() {
    for argv in [
        vec![
            "iroha",
            "ledger",
            "domain",
            "get",
            "--id",
            "wonderland.universal",
        ],
        vec![
            "iroha",
            "ledger",
            "domain",
            "meta",
            "get",
            "--id",
            "wonderland.universal",
            "--key",
            "tier",
        ],
    ] {
        let SingularQueryBox::FindDomainById(query) = singular_query_sent_by(&argv) else {
            panic!("{argv:?} must use FindDomainById");
        };
        assert_eq!(query.domain_id(), &wonderland());
    }
}

#[test]
fn asset_definition_get_and_meta_get_use_find_asset_definition_by_id() {
    let definition =
        AssetDefinitionId::derive_from_components(wonderland(), "rose".parse().unwrap());
    let literal = definition.to_string();
    for argv in [
        vec![
            "iroha",
            "ledger",
            "asset",
            "definition",
            "get",
            "--id",
            literal.as_str(),
        ],
        vec![
            "iroha",
            "ledger",
            "asset",
            "definition",
            "meta",
            "get",
            "--id",
            literal.as_str(),
            "--key",
            "tier",
        ],
    ] {
        let SingularQueryBox::FindAssetDefinitionById(query) = singular_query_sent_by(&argv) else {
            panic!("{argv:?} must use FindAssetDefinitionById");
        };
        assert_eq!(query.asset_definition_id(), &definition);
    }
}

#[test]
fn nft_get_and_meta_get_use_find_nft_by_id() {
    let nft: NftId = "mona_lisa$wonderland.universal".parse().unwrap();
    let literal = nft.to_string();
    for argv in [
        vec!["iroha", "ledger", "nft", "get", "--id", literal.as_str()],
        vec![
            "iroha",
            "ledger",
            "nft",
            "meta",
            "get",
            "--id",
            literal.as_str(),
            "--key",
            "tier",
        ],
    ] {
        let SingularQueryBox::FindNftById(query) = singular_query_sent_by(&argv) else {
            panic!("{argv:?} must use FindNftById");
        };
        assert_eq!(query.nft_id(), &nft);
    }
}
