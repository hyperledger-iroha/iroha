//! Canonical DA command dispatch through explicit public and account facades.

use super::*;
use iroha::http::{HttpTransport, Response, TransportFuture, TransportRequest};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct AsyncTransport(Arc<Mutex<Vec<TransportRequest>>>);

impl HttpTransport for AsyncTransport {
    fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
        panic!("DA commands must use the reusable asynchronous facade");
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            let body = match request.url.path() {
                "/v1/da/proof-policies" => json::to_vec(&DaProofPolicyBundle::new(vec![]))?,
                "/v1/da/commitments" => json::to_vec(&da::DaCommitmentListResponse {
                    policies: DaProofPolicyBundle::new(vec![]),
                    commitments: vec![],
                    next_cursor: None,
                })?,
                "/v1/da/pin-intents" => json::to_vec(&da::DaPinIntentListResponse {
                    intents: vec![],
                    next_cursor: None,
                })?,
                "/v1/da/commitments/prove" | "/v1/da/pin-intents/prove" => b"null".to_vec(),
                path => panic!("unexpected route {path}"),
            };
            self.0.lock().unwrap().push(request);
            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(body)
                .unwrap())
        })
    }
}

fn context() -> (TestContext, Arc<Mutex<Vec<TransportRequest>>>) {
    let mut context = TestContext::new(CliOutputFormat::Json);
    let requests = Arc::new(Mutex::new(Vec::new()));
    context.client_override = Some(
        iroha::client::Client::builder(context.cfg.clone())
            .http_transport(Arc::new(AsyncTransport(requests.clone())))
            .build()
            .unwrap(),
    );
    (context, requests)
}

fn commitments() -> CommitmentQueryArgs {
    CommitmentQueryArgs {
        manifest_hash: None,
        lane_id: None,
        epoch: None,
        sequence: None,
        limit: None,
        cursor_json: None,
    }
}

fn pins() -> PinIntentQueryArgs {
    PinIntentQueryArgs {
        manifest_hash: None,
        storage_ticket: None,
        alias: None,
        lane_id: None,
        epoch: None,
        sequence: None,
        limit: None,
        cursor_json: None,
    }
}

#[test]
fn public_da_commands_use_async_transport_without_account_headers() {
    let (mut context, requests) = context();
    ProofPoliciesArgs {}.run(&mut context).unwrap();
    commitments().run_list(&mut context).unwrap();
    pins().run_list(&mut context).unwrap();
    assert_eq!(context.printed.len(), 3);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    for request in requests.iter() {
        assert!(
            !request
                .headers
                .iter()
                .any(|(name, _)| name.as_str().starts_with("x-iroha-"))
        );
    }
}

#[test]
fn prove_commands_bind_account_authority_and_preserve_json_null() {
    let (mut context, requests) = context();
    let mut commitment = commitments();
    commitment.manifest_hash = Some("11".repeat(32));
    commitment.run_prove(&mut context).unwrap();
    let mut pin = pins();
    pin.storage_ticket = Some("22".repeat(32));
    pin.run_prove(&mut context).unwrap();
    assert_eq!(context.printed, ["null", "null"]);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        let headers = request
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.to_str().unwrap()))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            headers["x-iroha-account"],
            context.cfg.account.to_canonical_hex().unwrap()
        );
        assert!(headers.contains_key("x-iroha-signature"));
        assert!(headers.contains_key("x-iroha-timestamp-ms"));
        assert!(headers.contains_key("x-iroha-nonce"));
    }
}

#[test]
fn invalid_da_query_commands_fail_before_dispatch() {
    let (mut context, requests) = context();
    assert!(commitments().run_prove(&mut context).is_err());
    assert!(pins().run_prove(&mut context).is_err());
    let mut query = commitments();
    query.limit = Some(1001);
    assert!(query.run_list(&mut context).is_err());
    let mut query = pins();
    query.limit = Some(1001);
    assert!(query.run_list(&mut context).is_err());
    assert!(requests.lock().unwrap().is_empty());
    assert!(context.printed.is_empty());
}

#[test]
fn policy_command_has_one_canonical_name() {
    use clap::Subcommand as _;
    let command = Command::augment_subcommands(clap::Command::new("da"));
    command
        .clone()
        .try_get_matches_from(["da", "proof-policies"])
        .unwrap();
    assert!(
        command
            .try_get_matches_from(["da", "proof-policy-snapshot"])
            .is_err()
    );
}
