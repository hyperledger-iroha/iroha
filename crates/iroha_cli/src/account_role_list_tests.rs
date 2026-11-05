//! Exercise the parsed CLI command through the SDK's authenticated HTTP boundary.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use base64::Engine as _;
use clap::Parser as _;
use eyre::{Result, eyre};
use iroha::{
    collections::Page,
    data_model::{NetworkId, account::address::ChainDiscriminantGuard},
    http::{HttpTransport, Method, Response, TransportFuture, TransportRequest},
};
use iroha_crypto::{PublicKey, Signature};
use norito::json::{self, Value};

use super::*;
use crate::{Args, Run as _, RunContext, fallback_config};

#[derive(Debug)]
struct RoleTransport {
    path: String,
    network: NetworkId,
    account: String,
    public_key: PublicKey,
    pages: Mutex<VecDeque<Page<Value>>>,
    requests: Mutex<Vec<TransportRequest>>,
}

fn header<'a>(request: &'a TransportRequest, expected: &str) -> &'a str {
    let values = request
        .headers
        .iter()
        .filter(|(name, _)| name.as_str().eq_ignore_ascii_case(expected))
        .map(|(_, value)| value.to_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1, "exactly one {expected} header");
    values[0]
}

impl RoleTransport {
    fn respond(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        // This is the retired route's actual refusal. Keeping the test at the
        // command/HTTP boundary makes the previous signed query fail here.
        if request.method == Method::POST && request.url.path() == "/v1/query" {
            self.requests.lock().unwrap().push(request);
            return Ok(Response::builder()
                .status(400)
                .header("content-type", "application/json")
                .body(json::to_vec(&iroha_torii_shared::ErrorEnvelope::new(
                    "query_validation_failed",
                    "signed_query_shape_not_admitted: FindRolesByAccountId has no bounded signed-query source",
                ))?)?);
        }
        assert_eq!(request.method, Method::POST);
        assert_eq!(request.url.path(), self.path);
        assert!(request.url.query().is_none());
        assert_eq!(header(&request, "content-type"), "application/json");
        assert_eq!(header(&request, "accept"), "application/json");
        assert_eq!(header(&request, "x-iroha-account"), self.account);
        let timestamp = header(&request, "x-iroha-timestamp-ms").parse()?;
        let nonce = header(&request, "x-iroha-nonce");
        assert!(!nonce.is_empty());
        let signature = base64::engine::general_purpose::STANDARD
            .decode(header(&request, "x-iroha-signature"))?;
        let message = iroha::client::canonical_network_request_signature_message(
            &self.network,
            &request.method,
            &request.url,
            &request.body,
            timestamp,
            nonce,
        )?;
        Signature::try_from_bytes(&signature)?.verify(&self.public_key, &message)?;
        assert!(request.max_response_bytes > 0);
        assert!(request.timeout.is_some());
        let body: Value = json::from_slice(&request.body)?;
        assert_eq!(json::to_vec(&body)?, request.body);
        self.requests.lock().unwrap().push(request);
        let page = self
            .pages
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| eyre!("unexpected extra role page request"))?;
        Ok(Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(json::to_vec(&page)?)?)
    }
}

impl HttpTransport for RoleTransport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        self.respond(request)
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.respond(request) })
    }
}

struct Context {
    config: iroha::config::Config,
    client: Client,
    output: Option<Value>,
    i18n: iroha_i18n::Localizer,
}

impl RunContext for Context {
    fn config(&self) -> &iroha::config::Config {
        &self.config
    }
    fn transaction_metadata(&self) -> Option<&iroha_model_base::metadata::Metadata> {
        None
    }
    fn input_instructions(&self) -> bool {
        false
    }
    fn output_instructions(&self) -> bool {
        false
    }
    fn i18n(&self) -> &iroha_i18n::Localizer {
        &self.i18n
    }
    fn client_from_config(&self) -> Result<Client> {
        Ok(self.client.clone())
    }
    fn print_data<T: json::JsonSerialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        assert!(self.output.is_none(), "print one complete role-ID array");
        self.output = Some(json::to_value(value)?);
        Ok(())
    }
    fn println(&mut self, _: impl std::fmt::Display) -> Result<()> {
        panic!("role list must preserve print_data output")
    }
}

fn page(ids: &[&str], cursor: Option<&str>) -> Page<Value> {
    Page {
        items: ids.iter().map(|id| norito::json!({"id": (*id)})).collect(),
        next_cursor: cursor.map(str::to_owned),
        total: None,
    }
}

fn command(pages: Vec<Page<Value>>, flags: &[&str]) -> (Result<()>, Context, Arc<RoleTransport>) {
    let config = fallback_config();
    let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let target = config.account.canonical_i105().unwrap();
    // I105 includes non-ASCII symbols. Compare the encoded HTTP path, while
    // keeping the CLI's account argument in its canonical human-readable form.
    let mut endpoint = config.torii_api_url.clone();
    endpoint
        .path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .extend(["v1", "accounts", target.as_str(), "roles", "query"]);
    let transport = Arc::new(RoleTransport {
        path: endpoint.path().to_owned(),
        network: config.network_id,
        account: config.account.to_canonical_hex().unwrap(),
        public_key: config.key_pair.public_key().clone(),
        pages: Mutex::new(pages.into()),
        requests: Mutex::new(Vec::new()),
    });
    let client = Client::builder(config.clone())
        .http_transport(transport.clone())
        .build()
        .unwrap();
    let mut context = Context {
        config,
        client,
        output: None,
        i18n: iroha_i18n::Localizer::new(iroha_i18n::Bundle::Cli, iroha_i18n::Language::English),
    };
    let mut args = vec!["iroha", "account", "role", "list", "--id", &target];
    args.extend_from_slice(flags);
    let result = Args::try_parse_from(args)
        .map_err(Into::into)
        .and_then(|args| args.command.run(&mut context));
    (result, context, transport)
}

fn bodies(transport: &RoleTransport) -> Vec<Value> {
    transport
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|request| json::from_slice(&request.body).unwrap())
        .collect()
}

#[test]
fn role_list_uses_signed_collection_pages_and_preserves_role_id_output() -> Result<()> {
    let (result, context, transport) = command(
        vec![
            page(&["a", "b"], Some("first")),
            page(&[], Some("second")),
            page(&["c"], None),
        ],
        &["--fetch-size", "2"],
    );
    result?;
    assert_eq!(context.output, Some(norito::json!(["a", "b", "c"])));
    assert_eq!(
        bodies(&transport),
        vec![
            ListQuery::new().limit(2).to_json_value(),
            ListQuery::new().limit(2).cursor("first").to_json_value(),
            ListQuery::new().limit(2).cursor("second").to_json_value(),
        ]
    );
    assert!(transport.pages.lock().unwrap().is_empty());
    Ok(())
}

#[test]
fn role_list_limit_and_offset_span_pages_without_reading_past_the_window() -> Result<()> {
    let (result, context, transport) = command(
        vec![
            page(&["a", "b"], Some("first")),
            page(&["c"], Some("unneeded")),
        ],
        &["--offset", "1", "--limit", "2", "--fetch-size", "2"],
    );
    result?;
    assert_eq!(context.output, Some(norito::json!(["b", "c"])));
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
    assert_eq!(
        bodies(&transport),
        vec![
            ListQuery::new().limit(2).to_json_value(),
            ListQuery::new().limit(1).cursor("first").to_json_value(),
        ]
    );
    let (result, context, transport) = command(vec![page(&["a"], None)], &["--offset", "2"]);
    result?;
    assert_eq!(context.output, Some(norito::json!([])));
    assert_eq!(
        bodies(&transport),
        vec![ListQuery::new().limit(DEFAULT_PAGE_SIZE).to_json_value()]
    );
    Ok(())
}

#[test]
fn role_list_preserves_the_complete_hashed_multisig_role_identity() -> Result<()> {
    let config = fallback_config();
    let id = format!(
        "MULTISIG_SIGNATORY/domain/{}",
        iroha_crypto::HashOf::new(&config.account)
    );
    let (result, context, _) = command(vec![page(&[&id], None)], &[]);
    result?;
    let parsed: Vec<RoleId> = json::from_value(context.output.unwrap())?;
    assert_eq!(parsed, vec![id.parse()?]);
    Ok(())
}

#[test]
fn role_list_rejects_bad_assignment_pages_without_partial_output() {
    for pages in [
        vec![page(&["a", "b", "c"], None)],
        vec![page(&["a", "a"], None)],
        vec![page(&["b", "a"], None)],
        vec![page(&["a"], Some("next")), page(&["a"], None)],
        vec![page(&[""], None)],
        vec![Page {
            items: vec![norito::json!({"other": "a"})],
            ..page(&[], None)
        }],
        vec![Page {
            total: Some(1),
            ..page(&["a"], None)
        }],
        vec![page(&[], Some(""))],
        vec![
            page(&[], Some("first")),
            page(&[], Some("second")),
            page(&[], Some("first")),
        ],
    ] {
        let (result, context, _) = command(pages, &["--fetch-size", "2"]);
        assert!(result.is_err());
        assert!(context.output.is_none());
    }
}

#[test]
fn role_list_rejects_invalid_page_and_result_bounds_before_dispatch() {
    for flags in [
        vec!["--limit", "0"],
        vec!["--fetch-size", "0"],
        vec!["--fetch-size", "4294967296"],
    ] {
        let (result, context, transport) = command(Vec::new(), &flags);
        assert!(result.is_err());
        assert!(context.output.is_none());
        assert!(transport.requests.lock().unwrap().is_empty());
    }
}
