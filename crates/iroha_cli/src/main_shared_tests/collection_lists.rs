//! Collection `list` commands send canonical collection queries and print page envelopes.

use super::*;
use iroha::collections::{Collection, ListQuery, SortKey, field};

#[derive(Debug, Default)]
struct CollectionTransport {
    requests: std::sync::Mutex<Vec<iroha::http::TransportRequest>>,
    pages: std::sync::Mutex<std::collections::VecDeque<iroha::http::Response<Vec<u8>>>>,
}
impl iroha::http::HttpTransport for CollectionTransport {
    fn send_blocking(
        &self,
        request: iroha::http::TransportRequest,
    ) -> Result<iroha::http::Response<Vec<u8>>> {
        let capability_probe = request.url.path() == "/v1/node/capabilities";
        self.requests.lock().unwrap().push(request);
        if capability_probe {
            return Ok(json_response(
                200,
                &format!(
                    "{{\"data_model_version\":{}}}",
                    iroha::data_model::DATA_MODEL_VERSION
                ),
            ));
        }
        self.pages
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| eyre!("unexpected extra collection request"))
    }
    fn send(&self, request: iroha::http::TransportRequest) -> iroha::http::TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

fn json_response(status: u16, body: &str) -> iroha::http::Response<Vec<u8>> {
    iroha::http::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.as_bytes().to_vec())
        .unwrap()
}

struct ListContext {
    config: Config,
    client: Client,
    i18n: Localizer,
    output_format: CliOutputFormat,
    json_lines: bool,
    stdout: String,
}
impl RunContext for ListContext {
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
    fn output_format(&self) -> CliOutputFormat {
        self.output_format
    }
    fn json_lines(&self) -> bool {
        self.json_lines
    }
    fn print_data<T: JsonSerialize + ?Sized>(&mut self, data: &T) -> Result<()> {
        self.stdout
            .push_str(&render_json_output(data, self.json_lines)?);
        Ok(())
    }
    fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
        self.println_data(data)
    }
    fn println_data(&mut self, data: impl std::fmt::Display) -> Result<()> {
        self.stdout.push_str(&format!("{data}\n"));
        Ok(())
    }
    fn client_from_config(&self) -> Result<Client> {
        Ok(self.client.clone())
    }
}

struct ListRun {
    result: Result<()>,
    stdout: String,
    /// Collection requests (capability probes excluded): query path and JSON body.
    requests: Vec<(String, norito::json::Value)>,
}

fn run_list_command(output: OutputSelection, pages: Vec<(u16, &str)>, argv: &[&str]) -> ListRun {
    let transport = std::sync::Arc::new(CollectionTransport {
        requests: std::sync::Mutex::new(Vec::new()),
        pages: std::sync::Mutex::new(
            pages
                .into_iter()
                .map(|(status, body)| json_response(status, body))
                .collect(),
        ),
    });
    let config = fallback_config();
    let client = Client::builder(config.clone())
        .http_transport(transport.clone())
        .build()
        .expect("collection client");
    let mut context = ListContext {
        config,
        client,
        i18n: Localizer::new(Bundle::Cli, Language::English),
        output_format: output.format,
        json_lines: output.json_lines,
        stdout: String::new(),
    };
    let result = Args::try_parse_from(argv)
        .expect("parse list command")
        .command
        .run(&mut context);
    let requests = transport
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.url.path() != "/v1/node/capabilities")
        .map(|request| {
            assert_eq!(request.method, iroha::http::Method::POST);
            (
                request.url.path().to_owned(),
                norito::json::from_slice(&request.body).expect("JSON query body"),
            )
        })
        .collect();
    let requests = requests;
    ListRun {
        result,
        stdout: context.stdout,
        requests,
    }
}

fn json() -> OutputSelection {
    OutputSelection::resolve(Some(OutputFormatArg::Json), true, false)
}

fn chain_discriminant() -> u16 {
    fallback_config().account_chain_discriminant
}

/// A request path as it appears on the wire, with non-ASCII segments percent-encoded.
fn encoded_path(path: &str) -> String {
    url::Url::parse(&format!("http://localhost{path}"))
        .unwrap()
        .path()
        .to_owned()
}

const TWO_DOMAINS: &str = r#"{"items":[{"id":"garden.universal","owned_by":"alice"},{"id":"wonderland.universal","owned_by":"bob"}],"next_cursor":"c2","total":3}"#;

#[test]
fn list_sends_the_canonical_query_and_prints_one_page_document() {
    let run = run_list_command(
        json(),
        vec![(200, TWO_DOMAINS)],
        &[
            "iroha",
            "ledger",
            "domain",
            "list",
            "--where",
            r#"owned_by = "alice" or owned_by = "bob""#,
            "--sort",
            "-id",
            "--select",
            "id,owned_by",
            "--limit",
            "2",
            "--include-total",
        ],
    );
    run.result.expect("domain list");
    let expected = ListQuery::new()
        .filter(field("owned_by").eq("alice") | field("owned_by").eq("bob"))
        .sort_by(SortKey::desc("id"))
        .select(["id", "owned_by"])
        .limit(2)
        .include_total();
    assert_eq!(
        run.requests,
        vec![("/v1/domains/query".to_owned(), expected.to_json_value())]
    );
    let page: norito::json::Value = norito::json::from_str(&run.stdout).unwrap();
    assert_eq!(
        page.pointer("/items/1/id")
            .and_then(norito::json::Value::as_str),
        Some("wonderland.universal")
    );
    assert_eq!(
        page.pointer("/next_cursor")
            .and_then(norito::json::Value::as_str),
        Some("c2")
    );
    assert_eq!(
        page.pointer("/total").and_then(norito::json::Value::as_u64),
        Some(3)
    );
}

#[test]
fn every_collection_list_uses_its_query_route() {
    let account = fallback_config().account;
    let account_literal = account.to_string();
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let definition_literal = definition.to_string();
    let empty = r#"{"items":[],"next_cursor":null}"#;
    for (argv, collection) in [
        (
            vec!["iroha", "ledger", "domain", "list"],
            Collection::Domains,
        ),
        (vec!["iroha", "account", "list"], Collection::Accounts),
        (
            vec!["iroha", "ledger", "asset", "definition", "list"],
            Collection::AssetDefinitions,
        ),
        (vec!["iroha", "ledger", "nft", "list"], Collection::Nfts),
        (vec!["iroha", "ledger", "rwa", "list"], Collection::Rwas),
        (
            vec!["iroha", "ledger", "asset", "list"],
            Collection::AccountAssets(account.clone()),
        ),
        (
            vec![
                "iroha",
                "ledger",
                "asset",
                "list",
                "--account",
                &account_literal,
            ],
            Collection::AccountAssets(account.clone()),
        ),
        (
            vec![
                "iroha",
                "ledger",
                "asset",
                "holders",
                "--definition",
                &definition_literal,
            ],
            Collection::AssetHolders(definition.clone()),
        ),
        (
            vec!["iroha", "tx", "list"],
            Collection::AccountTransactions(account.clone()),
        ),
        (
            vec!["iroha", "app", "repo", "list"],
            Collection::RepoAgreements,
        ),
    ] {
        let run = run_list_command(json(), vec![(200, empty)], &argv);
        run.result
            .unwrap_or_else(|error| panic!("{argv:?}: {error:#}"));
        assert_eq!(
            run.requests,
            vec![(
                encoded_path(&collection.query_path(chain_discriminant()).unwrap()),
                ListQuery::new().to_json_value()
            )],
            "{argv:?}"
        );
        let page: norito::json::Value = norito::json::from_str(&run.stdout).unwrap();
        assert_eq!(
            page.pointer("/items")
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(0),
            "{argv:?}"
        );
        assert!(
            page.pointer("/next_cursor")
                .is_some_and(norito::json::Value::is_null)
        );
    }
}

#[test]
fn all_follows_cursors_and_jsonl_prints_one_row_per_line() {
    let run = run_list_command(
        OutputSelection::resolve(Some(OutputFormatArg::Jsonl), true, false),
        vec![
            (
                200,
                r#"{"items":[{"id":"a"},{"id":"b"}],"next_cursor":"c2"}"#,
            ),
            (200, r#"{"items":[{"id":"c"}],"next_cursor":null}"#),
        ],
        &["iroha", "account", "list", "--all", "--limit", "2"],
    );
    run.result.expect("streamed list");
    assert_eq!(
        run.stdout,
        "{\"id\":\"a\"}\n{\"id\":\"b\"}\n{\"id\":\"c\"}\n"
    );
    assert_eq!(run.requests.len(), 2);
    assert_eq!(
        run.requests[1].1,
        ListQuery::new().limit(2).cursor("c2").to_json_value(),
        "the second request continues from the first page's cursor"
    );
}

#[test]
fn all_with_json_prints_one_combined_page() {
    let run = run_list_command(
        json(),
        vec![
            (200, r#"{"items":[{"id":"a"}],"next_cursor":"c2"}"#),
            (200, r#"{"items":[{"id":"b"}],"next_cursor":null}"#),
        ],
        &["iroha", "account", "list", "--all"],
    );
    run.result.expect("combined list");
    let page: norito::json::Value = norito::json::from_str(&run.stdout).unwrap();
    assert_eq!(
        page.pointer("/items")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(
        page.pointer("/next_cursor")
            .is_some_and(norito::json::Value::is_null)
    );
}

#[test]
fn text_output_is_a_table_with_the_next_cursor() {
    let run = run_list_command(
        OutputSelection::resolve(Some(OutputFormatArg::Text), false, true),
        vec![(200, TWO_DOMAINS)],
        &["iroha", "ledger", "domain", "list", "--include-total"],
    );
    run.result.expect("text list");
    assert_eq!(
        run.stdout,
        "ID                    OWNED_BY\n\
         garden.universal      alice\n\
         wonderland.universal  bob\n\
         \n\
         total: 3\n\
         next page: --cursor c2\n"
    );
}

#[test]
fn invalid_flags_are_input_errors_before_any_request() {
    let run = run_list_command(
        json(),
        Vec::new(),
        &["iroha", "account", "list", "--filter", "id && label"],
    );
    let error = run.result.expect_err("invalid filter");
    assert!(run.requests.is_empty(), "no request for an invalid flag");
    let report = command_error_report(&error);
    assert_eq!(error_kind_for_report(&report), CliErrorKind::Input);
    let description = describe_cli_error(&report);
    assert!(
        description.message.contains("invalid `filter`"),
        "{description:?}"
    );
    assert!(description.message.contains("--filter"), "{description:?}");
}

#[test]
fn torii_query_rejections_render_code_message_and_hint_as_input_errors() {
    let run = run_list_command(
        json(),
        vec![(
            400,
            r#"{"code":"invalid_filter","message":"invalid `filter`: unknown field `colour`","details":{"field":"filter","expected":"id, owned_by, logo, metadata.*","actual":"colour","hint":"filter on one of the listed fields"}}"#,
        )],
        &[
            "iroha",
            "ledger",
            "domain",
            "list",
            "--filter",
            r#"colour = "red""#,
        ],
    );
    let error = run.result.expect_err("rejected filter");
    let report = command_error_report(&error);
    assert_eq!(error_kind_for_report(&report), CliErrorKind::Input);
    let description = describe_cli_error(&report);
    let rendered = format!("{} {:?}", description.message, description.causes);
    assert!(rendered.contains("invalid_filter"), "{rendered}");
    assert!(rendered.contains("unknown field `colour`"), "{rendered}");
    assert_eq!(
        description.hints,
        vec![
            "filter on one of the listed fields".to_owned(),
            "expected id, owned_by, logo, metadata.*; got colour".to_owned(),
        ]
    );
}

#[test]
fn rwa_and_repo_point_reads_filter_the_collection_by_id() {
    let lot = iroha::data_model::rwa::RwaId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        iroha_crypto::Hash::new(b"lot-1"),
    )
    .to_string();
    let rwa =
        format!(r#"{{"items":[{{"id":"{lot}","metadata":{{"grade":"A"}}}}],"next_cursor":null}}"#);
    let run = run_list_command(
        json(),
        vec![(200, rwa.as_str())],
        &[
            "iroha", "ledger", "rwa", "meta", "get", "--id", &lot, "--key", "grade",
        ],
    );
    run.result.expect("rwa metadata read");
    assert_eq!(run.stdout.trim(), "\"A\"");
    assert_eq!(
        run.requests,
        vec![(
            "/v1/rwas/query".to_owned(),
            ListQuery::new()
                .filter(field("id").eq(lot.as_str()))
                .limit(1)
                .to_json_value()
        )]
    );
    let run = run_list_command(
        json(),
        vec![(200, r#"{"items":[],"next_cursor":null}"#)],
        &["iroha", "app", "repo", "get", "--id", "daily_repo"],
    );
    let error = run.result.expect_err("missing agreement");
    assert!(
        error
            .to_string()
            .contains("repo_agreements `daily_repo` not found"),
        "{error:#}"
    );
}
