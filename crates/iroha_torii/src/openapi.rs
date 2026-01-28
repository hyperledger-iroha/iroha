//! Helpers for serving Torii's OpenAPI description.
//!
//! The OpenAPI document is assembled explicitly with Norito JSON helpers (instead
//! of relying on `serde_json`) to keep the toolchain consistent with on-wire
//! serialization.

use norito::json::{Map, Value};

fn license_section() -> Value {
    let mut license = Map::new();
    license.insert("name".into(), Value::String("Apache-2.0".to_owned()));
    license.insert(
        "url".into(),
        Value::String("https://www.apache.org/licenses/LICENSE-2.0".to_owned()),
    );
    Value::Object(license)
}

fn info_section(license: Value) -> Value {
    let mut info = Map::new();
    info.insert("title".into(), Value::String("Iroha Torii API".to_owned()));
    info.insert(
        "description".into(),
        Value::String(
            "HTTP surface for Torii. App endpoints accept optional canonical signing headers X-Iroha-Account/X-Iroha-Signature."
                .to_owned(),
        ),
    );
    info.insert("version".into(), Value::String("0.0.0-dev".to_owned()));
    info.insert("license".into(), license);
    Value::Object(info)
}

fn servers_section() -> Value {
    let mut server = Map::new();
    server.insert(
        "url".into(),
        Value::String("http://localhost:8080".to_owned()),
    );
    server.insert(
        "description".into(),
        Value::String("Example local node".to_owned()),
    );
    Value::Array(vec![Value::Object(server)])
}

fn tags_section() -> Value {
    let mut aliases = Map::new();
    aliases.insert("name".into(), Value::String("Aliases".to_owned()));
    aliases.insert(
        "description".into(),
        Value::String("Alias helper endpoints exposed under `/v1/aliases/*`.".to_owned()),
    );

    let mut time = Map::new();
    time.insert("name".into(), Value::String("Time".to_owned()));
    time.insert(
        "description".into(),
        Value::String("Network time service snapshots and diagnostics.".to_owned()),
    );

    let mut ledger = Map::new();
    ledger.insert("name".into(), Value::String("Ledger".to_owned()));
    ledger.insert(
        "description".into(),
        Value::String("Ledger helpers for block inclusion and execution proofs.".to_owned()),
    );

    let mut da = Map::new();
    da.insert("name".into(), Value::String("DataAvailability".to_owned()));
    da.insert(
        "description".into(),
        Value::String("Data availability commitments exposed under `/v1/da/*`.".to_owned()),
    );

    let mut sumeragi = Map::new();
    sumeragi.insert("name".into(), Value::String("Sumeragi".to_owned()));
    sumeragi.insert(
        "description".into(),
        Value::String("Consensus diagnostics and checkpoints under `/v1/sumeragi/*`.".to_owned()),
    );

    let mut offline = Map::new();
    offline.insert("name".into(), Value::String("Offline".to_owned()));
    offline.insert(
        "description".into(),
        Value::String(
            "Offline wallet, audit, and settlement endpoints under `/v1/offline/*`.".to_owned(),
        ),
    );

    let mut bridge = Map::new();
    bridge.insert("name".into(), Value::String("Bridge".to_owned()));
    bridge.insert(
        "description".into(),
        Value::String("Bridge finality surfaces exposed under `/v1/bridge/*`.".to_owned()),
    );

    let mut kaigi = Map::new();
    kaigi.insert("name".into(), Value::String("Kaigi".to_owned()));
    kaigi.insert(
        "description".into(),
        Value::String("Kaigi relay telemetry endpoints under `/v1/kaigi/*`.".to_owned()),
    );

    let mut nexus = Map::new();
    nexus.insert("name".into(), Value::String("Nexus".to_owned()));
    nexus.insert(
        "description".into(),
        Value::String("Nexus lane staking, lifecycle, and space directory endpoints.".to_owned()),
    );

    let mut system = Map::new();
    system.insert("name".into(), Value::String("System".to_owned()));
    system.insert(
        "description".into(),
        Value::String("Node liveness and general system helpers.".to_owned()),
    );

    let mut operator_auth = Map::new();
    operator_auth.insert("name".into(), Value::String("OperatorAuth".to_owned()));
    operator_auth.insert(
        "description".into(),
        Value::String("Operator WebAuthn bootstrap and session issuance endpoints.".to_owned()),
    );

    let mut transactions = Map::new();
    transactions.insert("name".into(), Value::String("Transactions".to_owned()));
    transactions.insert(
        "description".into(),
        Value::String("Transaction submission and ISO 20022 bridge helpers.".to_owned()),
    );

    let mut queries = Map::new();
    queries.insert("name".into(), Value::String("Queries".to_owned()));
    queries.insert(
        "description".into(),
        Value::String("Signed query submissions and query results.".to_owned()),
    );

    let mut streams = Map::new();
    streams.insert("name".into(), Value::String("Streams".to_owned()));
    streams.insert(
        "description".into(),
        Value::String("Event, block, and P2P stream endpoints (SSE/WebSocket).".to_owned()),
    );

    let mut contracts = Map::new();
    contracts.insert("name".into(), Value::String("Contracts".to_owned()));
    contracts.insert(
        "description".into(),
        Value::String("Contract deployment, instances, and execution helpers.".to_owned()),
    );

    let mut zk = Map::new();
    zk.insert("name".into(), Value::String("ZK".to_owned()));
    zk.insert(
        "description".into(),
        Value::String("Zero-knowledge proofs, VK registry, and attachments.".to_owned()),
    );

    let mut proofs = Map::new();
    proofs.insert("name".into(), Value::String("Proofs".to_owned()));
    proofs.insert(
        "description".into(),
        Value::String("Proof record access and retention helpers.".to_owned()),
    );

    let mut governance = Map::new();
    governance.insert("name".into(), Value::String("Governance".to_owned()));
    governance.insert(
        "description".into(),
        Value::String("Governance proposals, ballots, referenda, and council helpers.".to_owned()),
    );

    let mut runtime = Map::new();
    runtime.insert("name".into(), Value::String("Runtime".to_owned()));
    runtime.insert(
        "description".into(),
        Value::String("Runtime ABI, upgrades, capabilities, and metrics.".to_owned()),
    );

    let mut settlement = Map::new();
    settlement.insert("name".into(), Value::String("Settlement".to_owned()));
    settlement.insert(
        "description".into(),
        Value::String("Repo agreement queries and settlement helpers.".to_owned()),
    );

    let mut accounts = Map::new();
    accounts.insert("name".into(), Value::String("Accounts".to_owned()));
    accounts.insert(
        "description".into(),
        Value::String("Account listing and account-scoped query helpers.".to_owned()),
    );

    let mut domains = Map::new();
    domains.insert("name".into(), Value::String("Domains".to_owned()));
    domains.insert(
        "description".into(),
        Value::String("Domain listing and query helpers.".to_owned()),
    );

    let mut assets = Map::new();
    assets.insert("name".into(), Value::String("Assets".to_owned()));
    assets.insert(
        "description".into(),
        Value::String("Asset definitions, holdings, and query helpers.".to_owned()),
    );

    let mut nfts = Map::new();
    nfts.insert("name".into(), Value::String("NFTs".to_owned()));
    nfts.insert(
        "description".into(),
        Value::String("NFT listing and query helpers.".to_owned()),
    );

    let mut subscriptions = Map::new();
    subscriptions.insert("name".into(), Value::String("Subscriptions".to_owned()));
    subscriptions.insert(
        "description".into(),
        Value::String("Subscription plan and billing helpers.".to_owned()),
    );

    let mut parameters = Map::new();
    parameters.insert("name".into(), Value::String("Parameters".to_owned()));
    parameters.insert(
        "description".into(),
        Value::String("Node parameter snapshots.".to_owned()),
    );

    let mut explorer = Map::new();
    explorer.insert("name".into(), Value::String("Explorer".to_owned()));
    explorer.insert(
        "description".into(),
        Value::String("Explorer read-only endpoints and streams.".to_owned()),
    );

    let mut connect = Map::new();
    connect.insert("name".into(), Value::String("Connect".to_owned()));
    connect.insert(
        "description".into(),
        Value::String("Iroha Connect session and relay endpoints.".to_owned()),
    );

    let mut push = Map::new();
    push.insert("name".into(), Value::String("Push".to_owned()));
    push.insert(
        "description".into(),
        Value::String("Push notification device registration.".to_owned()),
    );

    let mut webhooks = Map::new();
    webhooks.insert("name".into(), Value::String("Webhooks".to_owned()));
    webhooks.insert(
        "description".into(),
        Value::String("Webhook subscription management.".to_owned()),
    );

    let mut sorafs = Map::new();
    sorafs.insert("name".into(), Value::String("SoraFS".to_owned()));
    sorafs.insert(
        "description".into(),
        Value::String("SoraFS storage, capacity, and audit endpoints.".to_owned()),
    );

    let mut sns = Map::new();
    sns.insert("name".into(), Value::String("SNS".to_owned()));
    sns.insert(
        "description".into(),
        Value::String("SNS registration and governance scaffolding.".to_owned()),
    );

    let mut soradns = Map::new();
    soradns.insert("name".into(), Value::String("SoraDNS".to_owned()));
    soradns.insert(
        "description".into(),
        Value::String("SoraDNS directory endpoints.".to_owned()),
    );

    let mut content = Map::new();
    content.insert("name".into(), Value::String("Content".to_owned()));
    content.insert(
        "description".into(),
        Value::String("Static bundle content fetch endpoints.".to_owned()),
    );

    let mut space_directory = Map::new();
    space_directory.insert("name".into(), Value::String("SpaceDirectory".to_owned()));
    space_directory.insert(
        "description".into(),
        Value::String("Space directory registration and manifest endpoints.".to_owned()),
    );

    let mut iso20022 = Map::new();
    iso20022.insert("name".into(), Value::String("ISO20022".to_owned()));
    iso20022.insert(
        "description".into(),
        Value::String("ISO 20022 bridge submissions and status queries.".to_owned()),
    );

    let mut soranet = Map::new();
    soranet.insert("name".into(), Value::String("SoraNet".to_owned()));
    soranet.insert(
        "description".into(),
        Value::String("SoraNet privacy ingestion endpoints.".to_owned()),
    );

    Value::Array(vec![
        Value::Object(aliases),
        Value::Object(time),
        Value::Object(ledger),
        Value::Object(da),
        Value::Object(sumeragi),
        Value::Object(offline),
        Value::Object(bridge),
        Value::Object(kaigi),
        Value::Object(nexus),
        Value::Object(system),
        Value::Object(operator_auth),
        Value::Object(transactions),
        Value::Object(queries),
        Value::Object(streams),
        Value::Object(contracts),
        Value::Object(zk),
        Value::Object(proofs),
        Value::Object(governance),
        Value::Object(runtime),
        Value::Object(settlement),
        Value::Object(accounts),
        Value::Object(domains),
        Value::Object(assets),
        Value::Object(nfts),
        Value::Object(subscriptions),
        Value::Object(parameters),
        Value::Object(explorer),
        Value::Object(connect),
        Value::Object(push),
        Value::Object(webhooks),
        Value::Object(sorafs),
        Value::Object(sns),
        Value::Object(soradns),
        Value::Object(content),
        Value::Object(space_directory),
        Value::Object(iso20022),
        Value::Object(soranet),
    ])
}

fn schema_ref(name: &str) -> Value {
    let mut schema = Map::new();
    schema.insert(
        "$ref".into(),
        Value::String(format!("#/components/schemas/{name}")),
    );
    Value::Object(schema)
}

fn error_schema_reference() -> Value {
    schema_ref("ErrorResponse")
}

fn not_acceptable_response() -> Value {
    json_response(
        "Requested content type is not acceptable; supported: application/json, application/x-norito.",
        error_schema_reference(),
    )
}

fn json_media_content(schema: Value) -> Value {
    let mut media = Map::new();
    media.insert("schema".into(), schema);
    let mut content = Map::new();
    content.insert("application/json".into(), Value::Object(media));
    Value::Object(content)
}

fn json_response(description: &str, schema: Value) -> Value {
    let mut body = Map::new();
    body.insert("description".into(), Value::String(description.to_owned()));
    body.insert("content".into(), json_media_content(schema));
    Value::Object(body)
}

fn json_response_with_headers(description: &str, schema: Value, headers: Map) -> Value {
    let mut body = Map::new();
    body.insert("description".into(), Value::String(description.to_owned()));
    body.insert("content".into(), json_media_content(schema));
    if !headers.is_empty() {
        body.insert("headers".into(), Value::Object(headers));
    }
    Value::Object(body)
}

fn offline_reject_headers() -> Map {
    let mut header = Map::new();
    header.insert(
        "description".into(),
        Value::String(
            "Stable offline rejection reason code (for example: certificate_expired, counter_conflict, allowance_exceeded, invoice_duplicate). Present only when the failure maps to an offline rejection."
                .to_owned(),
        ),
    );
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    schema.insert(
        "example".into(),
        Value::String("certificate_expired".to_owned()),
    );
    header.insert("schema".into(), Value::Object(schema));

    let mut headers = Map::new();
    headers.insert("x-iroha-reject-code".to_owned(), Value::Object(header));
    headers
}

fn plain_text_response(description: &str, example: Option<&str>) -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    if let Some(value) = example {
        schema.insert("example".into(), Value::String(value.to_owned()));
    }
    let mut media = Map::new();
    media.insert("schema".into(), Value::Object(schema));
    let mut content = Map::new();
    content.insert("text/plain".into(), Value::Object(media));
    let mut body = Map::new();
    body.insert("description".into(), Value::String(description.to_owned()));
    body.insert("content".into(), Value::Object(content));
    Value::Object(body)
}

fn event_stream_response(description: &str) -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    let mut media = Map::new();
    media.insert("schema".into(), Value::Object(schema));
    let mut content = Map::new();
    content.insert("text/event-stream".into(), Value::Object(media));
    let mut body = Map::new();
    body.insert("description".into(), Value::String(description.to_owned()));
    body.insert("content".into(), Value::Object(content));
    Value::Object(body)
}

fn alias_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/aliases/voprf/evaluate".to_owned(),
        Value::Object(alias_voprf_evaluate_operation()),
    );
    paths.insert(
        "/v1/aliases/resolve".to_owned(),
        Value::Object(alias_resolve_operation()),
    );
    paths.insert(
        "/v1/aliases/resolve_index".to_owned(),
        Value::Object(alias_resolve_index_operation()),
    );
    paths
}

fn time_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/time/now".to_owned(),
        Value::Object(time_now_operation()),
    );
    paths.insert(
        "/v1/time/status".to_owned(),
        Value::Object(time_status_operation()),
    );
    paths
}

fn ledger_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/ledger/headers".to_owned(),
        Value::Object(ledger_headers_operation()),
    );
    paths.insert(
        "/v1/ledger/state/{height}".to_owned(),
        Value::Object(ledger_state_root_operation()),
    );
    paths.insert(
        "/v1/ledger/state-proof/{height}".to_owned(),
        Value::Object(ledger_state_proof_operation()),
    );
    paths.insert(
        "/v1/ledger/block/{height}/proof/{entry_hash}".to_owned(),
        Value::Object(ledger_block_proof_operation()),
    );
    paths
}

fn da_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/da/ingest".to_owned(),
        Value::Object(json_post_operation(
            "DataAvailability",
            "Ingest DA payloads.",
            "Submit data availability payloads for indexing and commitment.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/da/manifests/{ticket}".to_owned(),
        Value::Object(json_get_operation(
            "DataAvailability",
            "Fetch DA manifest payload.",
            "Retrieve a previously ingested DA manifest by its ticket.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "ticket",
                "DA manifest ticket identifier.",
            )],
        )),
    );
    paths.insert(
        "/v1/da/proof_policies".to_owned(),
        Value::Object(json_get_operation(
            "DataAvailability",
            "List DA proof policies.",
            "Return the configured DA proof policy set.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/da/proof_policy_snapshot".to_owned(),
        Value::Object(json_get_operation(
            "DataAvailability",
            "Fetch the DA proof policy snapshot.",
            "Return the active DA proof policy snapshot bundle.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/da/commitments".to_owned(),
        Value::Object(da_commitments_operation()),
    );
    paths.insert(
        "/v1/da/commitments/prove".to_owned(),
        Value::Object(da_commitments_prove_operation()),
    );
    paths.insert(
        "/v1/da/commitments/verify".to_owned(),
        Value::Object(da_commitments_verify_operation()),
    );
    paths.insert(
        "/v1/da/pin_intents".to_owned(),
        Value::Object(json_post_operation(
            "DataAvailability",
            "List DA pin intents.",
            "List pin intent commitments indexed by the node.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/da/pin_intents/prove".to_owned(),
        Value::Object(json_post_operation(
            "DataAvailability",
            "Fetch a DA pin intent proof placeholder.",
            "Locate a pin intent by key and return the indexed location.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/da/pin_intents/verify".to_owned(),
        Value::Object(json_post_operation(
            "DataAvailability",
            "Verify a DA pin intent proof.",
            "Verify a pin intent proof bundle.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn offline_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/offline/allowances".to_owned(),
        Value::Object(offline_allowances_operation()),
    );
    paths.insert(
        "/v1/offline/allowances/{certificate_id_hex}".to_owned(),
        Value::Object(offline_allowance_detail_operation()),
    );
    paths.insert(
        "/v1/offline/allowances/{certificate_id_hex}/renew".to_owned(),
        Value::Object(offline_allowance_renew_operation()),
    );
    paths.insert(
        "/v1/offline/certificates".to_owned(),
        Value::Object(offline_allowances_operation()),
    );
    paths.insert(
        "/v1/offline/certificates/issue".to_owned(),
        Value::Object(offline_certificate_issue_operation()),
    );
    paths.insert(
        "/v1/offline/certificates/{certificate_id_hex}".to_owned(),
        Value::Object(offline_allowance_detail_operation()),
    );
    paths.insert(
        "/v1/offline/certificates/{certificate_id_hex}/renew".to_owned(),
        Value::Object(offline_allowance_renew_operation()),
    );
    paths.insert(
        "/v1/offline/certificates/{certificate_id_hex}/renew/issue".to_owned(),
        Value::Object(offline_certificate_renew_issue_operation()),
    );
    paths.insert(
        "/v1/offline/certificates/revoke".to_owned(),
        Value::Object(offline_certificates_revoke_operation()),
    );
    paths.insert(
        "/v1/offline/allowances/query".to_owned(),
        Value::Object(offline_allowances_query_operation()),
    );
    paths.insert(
        "/v1/offline/certificates/query".to_owned(),
        Value::Object(offline_allowances_query_operation()),
    );
    paths.insert(
        "/v1/offline/receipts".to_owned(),
        Value::Object(offline_receipts_operation()),
    );
    paths.insert(
        "/v1/offline/receipts/query".to_owned(),
        Value::Object(offline_receipts_query_operation()),
    );
    paths.insert(
        "/v1/offline/revocations".to_owned(),
        Value::Object(json_get_operation(
            "Offline",
            "List offline revocations.",
            "Return the latest offline revocation entries.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/offline/revocations/query".to_owned(),
        Value::Object(json_post_operation(
            "Offline",
            "Query offline revocations via JSON envelope.",
            "Submit a QueryEnvelope to filter offline revocations.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/offline/transfers".to_owned(),
        Value::Object(offline_transfers_operation()),
    );
    paths.insert(
        "/v1/offline/transfers/{bundle_id_hex}".to_owned(),
        Value::Object(offline_transfer_detail_operation()),
    );
    paths.insert(
        "/v1/offline/transfers/query".to_owned(),
        Value::Object(offline_transfers_query_operation()),
    );
    paths.insert(
        "/v1/offline/transfers/proof".to_owned(),
        Value::Object(json_post_operation(
            "Offline",
            "Build offline transfer proof requests.",
            "Generate FASTPQ witness payloads from a transfer payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/offline/settlements".to_owned(),
        Value::Object(offline_settlements_operation()),
    );
    paths.insert(
        "/v1/offline/settlements/{bundle_id_hex}".to_owned(),
        Value::Object(offline_transfer_detail_operation()),
    );
    paths.insert(
        "/v1/offline/settlements/query".to_owned(),
        Value::Object(offline_transfers_query_operation()),
    );
    paths.insert(
        "/v1/offline/spend-receipts".to_owned(),
        Value::Object(offline_spend_receipts_operation()),
    );
    paths.insert(
        "/v1/offline/state".to_owned(),
        Value::Object(offline_state_operation()),
    );
    paths.insert(
        "/v1/offline/bundle/proof_status".to_owned(),
        Value::Object(offline_bundle_proof_status_operation()),
    );
    paths.insert(
        "/v1/offline/rejections".to_owned(),
        Value::Object(offline_rejections_operation()),
    );
    paths.insert(
        "/v1/offline/summaries".to_owned(),
        Value::Object(offline_summaries_operation()),
    );
    paths.insert(
        "/v1/offline/summaries/query".to_owned(),
        Value::Object(offline_summaries_query_operation()),
    );
    paths
}

fn offline_allowances_operation() -> Map {
    let mut get_op = Map::new();
    get_op.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    get_op.insert(
        "summary".into(),
        Value::String("List registered offline allowances.".to_owned()),
    );
    get_op.insert(
        "description".into(),
        Value::String(
            "Returns operator-issued offline wallet certificates plus ledger-maintained \
             counter checkpoints. Supports optional filter, pagination, and sort \
             parameters encoded as query arguments."
                .to_owned(),
        ),
    );
    get_op.insert(
        "operationId".into(),
        Value::String("offlineAllowancesList".to_owned()),
    );
    let mut params = list_filter_query_parameters();
    params.extend(offline_allowance_query_parameters());
    get_op.insert("parameters".into(), Value::Array(params));
    get_op.insert(
        "responses".into(),
        Value::Object(offline_allowances_responses()),
    );

    let mut post_op = Map::new();
    post_op.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    post_op.insert(
        "summary".into(),
        Value::String("Register a new offline allowance certificate.".to_owned()),
    );
    post_op.insert(
        "description".into(),
        Value::String(
            "Accepts an operator-signed OfflineWalletCertificate and enqueues a \
             `RegisterOfflineAllowance` transaction."
                .to_owned(),
        ),
    );
    post_op.insert(
        "operationId".into(),
        Value::String("offlineAllowancesIssue".to_owned()),
    );
    post_op.insert("requestBody".into(), offline_allowance_issue_request_body());
    post_op.insert(
        "responses".into(),
        Value::Object(offline_allowance_issue_responses()),
    );

    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(get_op));
    methods.insert("post".to_owned(), Value::Object(post_op));
    methods
}

fn offline_allowances_query_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Query offline allowances with a JSON envelope.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Accepts the Norito `QueryEnvelope` structure so clients can submit complex \
             filters, selectors, and pagination hints when listing allowances."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineAllowancesQuery".to_owned()),
    );
    operation.insert("requestBody".into(), offline_query_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(offline_allowances_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_allowance_detail_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch a specific offline allowance.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the on-ledger OfflineAllowanceRecord for the provided certificate id."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineAllowanceGet".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![path_param(
            "certificate_id_hex",
            "Deterministic certificate id rendered as hex.",
        )]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_allowance_detail_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_allowance_renew_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Renew an offline allowance certificate.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Accepts a new OfflineWalletCertificate and enqueues a RegisterOfflineAllowance \
             transaction, scoped to an existing certificate id."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineAllowanceRenew".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![path_param(
            "certificate_id_hex",
            "Deterministic certificate id rendered as hex.",
        )]),
    );
    operation.insert(
        "requestBody".into(),
        offline_certificate_renew_request_body(),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_certificate_renew_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_certificate_issue_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Issue an offline wallet certificate.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Accepts an unsigned OfflineWalletCertificate draft and returns the operator-signed \
             certificate payload without registering it on-ledger."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineCertificateIssue".to_owned()),
    );
    operation.insert(
        "requestBody".into(),
        offline_certificate_issue_request_body(),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_certificate_issue_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_certificate_renew_issue_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Issue a renewed offline certificate.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Signs a refreshed OfflineWalletCertificate draft after confirming the referenced \
             allowance exists on-ledger."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineCertificateRenewIssue".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![path_param(
            "certificate_id_hex",
            "Deterministic certificate id rendered as hex.",
        )]),
    );
    operation.insert(
        "requestBody".into(),
        offline_certificate_issue_request_body(),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_certificate_issue_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_certificates_revoke_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Revoke an offline certificate verdict.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Enqueues a RegisterOfflineVerdictRevocation transaction referencing the \
             attestation verdict tied to the supplied certificate id."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineCertificateRevoke".to_owned()),
    );
    operation.insert(
        "requestBody".into(),
        offline_certificate_revoke_request_body(),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_certificate_revoke_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_settlements_operation() -> Map {
    let mut get_op = Map::new();
    get_op.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    get_op.insert(
        "summary".into(),
        Value::String("List offline settlement bundles.".to_owned()),
    );
    get_op.insert(
        "description".into(),
        Value::String("Alias of `/v1/offline/transfers` for settlement audit readers.".to_owned()),
    );
    get_op.insert(
        "operationId".into(),
        Value::String("offlineSettlementsList".to_owned()),
    );
    let mut params = list_filter_query_parameters();
    params.extend(offline_transfer_query_parameters());
    get_op.insert("parameters".into(), Value::Array(params));
    get_op.insert(
        "responses".into(),
        Value::Object(offline_transfers_responses()),
    );

    let mut post_op = Map::new();
    post_op.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    post_op.insert(
        "summary".into(),
        Value::String("Submit an offline settlement bundle.".to_owned()),
    );
    post_op.insert(
        "description".into(),
        Value::String(
            "Enqueues a `SubmitOfflineToOnlineTransfer` transaction carrying the provided \
             OfflineToOnlineTransfer bundle."
                .to_owned(),
        ),
    );
    post_op.insert(
        "operationId".into(),
        Value::String("offlineSettlementsSubmit".to_owned()),
    );
    post_op.insert(
        "requestBody".into(),
        offline_settlement_submit_request_body(),
    );
    post_op.insert(
        "responses".into(),
        Value::Object(offline_settlement_submit_responses()),
    );

    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(get_op));
    methods.insert("post".to_owned(), Value::Object(post_op));
    methods
}

fn offline_spend_receipts_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Validate offline spend receipts.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Validates receipt signatures and returns their Poseidon merkle root.".to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineSpendReceiptsSubmit".to_owned()),
    );
    operation.insert("requestBody".into(), offline_spend_receipts_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(offline_spend_receipts_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_state_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch an offline state snapshot.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns registered allowances, settlement bundles, counter summaries, \
             and verdict revocations for wallet sync."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineStateGet".to_owned()),
    );
    operation.insert("responses".into(), Value::Object(offline_state_responses()));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_bundle_proof_status_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch lightweight offline bundle proof status.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Computes the Poseidon receipts root for the stored bundle, compares it to the \
             receipts root advertised by the optional aggregate proof envelope, and returns a \
             concise status payload without streaming full receipts."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineBundleProofStatus".to_owned()),
    );
    let mut params = Vec::new();
    params.push(Value::Object({
        let mut param = Map::new();
        param.insert("name".into(), Value::String("bundle_id_hex".to_owned()));
        param.insert("in".into(), Value::String("query".to_owned()));
        param.insert("required".into(), Value::Bool(true));
        param.insert(
            "description".into(),
            Value::String("Deterministic bundle identifier (hex).".to_owned()),
        );
        let mut schema = Map::new();
        schema.insert("type".into(), Value::String("string".to_owned()));
        param.insert("schema".into(), Value::Object(schema));
        param
    }));
    params.push(address_format_query_param());
    operation.insert("parameters".into(), Value::Array(params));
    operation.insert(
        "responses".into(),
        Value::Object(offline_bundle_proof_status_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_transfers_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List pending offline-to-online transfer bundles.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns bundles with their receiver/deposit accounts, receipts, and \
             balance proofs for audit or settlement dashboards."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineTransfersList".to_owned()),
    );
    let mut params = list_filter_query_parameters();
    params.extend(offline_transfer_query_parameters());
    operation.insert("parameters".into(), Value::Array(params));
    operation.insert(
        "responses".into(),
        Value::Object(offline_transfers_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_transfer_detail_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch a specific offline transfer bundle.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the same enriched transfer payload as `/v1/offline/transfers`, scoped to a \
             single `bundle_id_hex`."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineTransferGet".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![
            path_param(
                "bundle_id_hex",
                "Deterministic bundle identifier rendered as hex.",
            ),
            address_format_query_param(),
        ]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_transfer_detail_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_transfers_query_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Query offline transfer bundles via JSON envelope.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Supports the Norito `QueryEnvelope` body to paginate and filter bundle \
             records, enabling regulator views or PSP dashboards."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineTransfersQuery".to_owned()),
    );
    operation.insert("requestBody".into(), offline_query_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(offline_transfers_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_receipts_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List flattened offline spend receipts.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns individual spend receipts extracted from offline-to-online transfer \
             bundles, suitable for audit dashboards."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineReceiptsList".to_owned()),
    );
    let mut params = list_filter_query_parameters();
    params.extend(offline_receipt_query_parameters());
    operation.insert("parameters".into(), Value::Array(params));
    operation.insert(
        "responses".into(),
        Value::Object(offline_receipts_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_receipts_query_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Query flattened offline receipts via JSON envelope.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Supports the Norito QueryEnvelope body to paginate and filter receipts \
             extracted from settlement bundles."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineReceiptsQuery".to_owned()),
    );
    operation.insert("requestBody".into(), offline_query_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(offline_receipts_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn offline_rejections_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Report offline transfer rejection counters.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns aggregated counts grouped by platform and rejection reason so \
             operators can monitor App Attest and KeyMint validation failures."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineRejections".to_owned()),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_rejections_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_summaries_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List hardware counter summaries for offline allowances.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns controller-facing summaries of the latest App Attest and Android \
             marker counters observed for each allowance certificate."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineSummariesList".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(list_filter_query_parameters()),
    );
    operation.insert(
        "responses".into(),
        Value::Object(offline_summaries_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn offline_summaries_query_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Offline".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Query offline counter summaries with a JSON envelope.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Accepts the Norito `QueryEnvelope` structure so operators can fetch counter \
             checkpoints with complex filters, selectors, and pagination hints."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("offlineSummariesQuery".to_owned()),
    );
    operation.insert("requestBody".into(), offline_query_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(offline_summaries_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn list_filter_query_parameters() -> Vec<Value> {
    let mut params = Vec::new();

    params.push(string_query_param(
        "filter",
        "Optional Norito JSON filter expression encoded as a compact string (see Torii filter DSL).",
    ));

    for (name, desc) in [
        ("limit", "Optional page size limit."),
        ("offset", "Optional starting offset (default 0)."),
    ] {
        params.push(integer_query_param(name, desc, Some("uint64")));
    }

    params.push(string_query_param(
        "sort",
        "Optional CSV sort spec, e.g., `registered_at_ms:desc`.",
    ));
    params.push(string_query_param(
        "address_format",
        "Optional rendering for account ids (`ih58` or `compressed`).",
    ));

    params
}

fn offline_allowance_query_parameters() -> Vec<Value> {
    vec![
        string_query_param(
            "controller_id",
            "Filter allowances by controller account (accepts IH58 (preferred)/sora (second-best)/public-key literals).",
        ),
        integer_query_param(
            "certificate_expires_before_ms",
            "Only include allowances whose certificate expiry is at or before this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "certificate_expires_after_ms",
            "Only include allowances whose certificate expiry is at or after this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "policy_expires_before_ms",
            "Only include allowances whose issuer policy expiry is at or before this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "policy_expires_after_ms",
            "Only include allowances whose issuer policy expiry is at or after this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "refresh_before_ms",
            "Filter to allowances whose attestation refresh deadline is at or before this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "refresh_after_ms",
            "Filter to allowances whose attestation refresh deadline is at or after this unix timestamp (ms).",
            Some("uint64"),
        ),
        string_query_param(
            "verdict_id_hex",
            "Match allowances whose cached attestation verdict id (hex) equals the provided value (case-insensitive).",
        ),
        string_query_param(
            "attestation_nonce_hex",
            "Match allowances whose attestation nonce (hex) equals the provided value (case-insensitive).",
        ),
        bool_query_param(
            "require_verdict",
            "When true, only allowances that already have attestation verdict metadata are returned.",
        ),
        bool_query_param(
            "only_missing_verdict",
            "When true, only allowances that are missing attestation verdict metadata are returned.",
        ),
        bool_query_param(
            "include_expired",
            "Include certificates/policies/refresh windows that have already expired (defaults to false).",
        ),
    ]
}

fn offline_transfer_query_parameters() -> Vec<Value> {
    vec![
        string_query_param(
            "controller_id",
            "Filter bundles by originating controller account (accepts IH58 (preferred)/sora (second-best)/public-key literals).",
        ),
        string_query_param(
            "receiver_id",
            "Filter bundles by receiver account (accepts IH58 (preferred)/sora (second-best)/public-key literals).",
        ),
        string_query_param(
            "deposit_account_id",
            "Filter bundles by deposit account (accepts IH58 (preferred)/sora (second-best)/public-key literals).",
        ),
        string_query_param(
            "certificate_id_hex",
            "Match bundles whose originating certificate id (hex) equals the provided value (case-insensitive).",
        ),
        integer_query_param(
            "certificate_expires_before_ms",
            "Only include bundles whose originating certificate expiry is at or before this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "certificate_expires_after_ms",
            "Only include bundles whose originating certificate expiry is at or after this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "policy_expires_before_ms",
            "Only include bundles whose policy expiry is at or before this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "policy_expires_after_ms",
            "Only include bundles whose policy expiry is at or after this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "refresh_before_ms",
            "Filter to bundles whose attestation refresh deadline is at or before this unix timestamp (ms).",
            Some("uint64"),
        ),
        integer_query_param(
            "refresh_after_ms",
            "Filter to bundles whose attestation refresh deadline is at or after this unix timestamp (ms).",
            Some("uint64"),
        ),
        string_query_param(
            "verdict_id_hex",
            "Match bundles whose attestation verdict id (hex) equals the provided value (case-insensitive).",
        ),
        string_query_param(
            "attestation_nonce_hex",
            "Match bundles whose attestation nonce (hex) equals the provided value (case-insensitive).",
        ),
        string_query_param(
            "platform_policy",
            "Restrict settled bundles to a specific Android attestation policy (use `play_integrity` or `hms_safety_detect`).",
        ),
        bool_query_param(
            "require_verdict",
            "When true, only bundles that already have attestation verdict metadata are returned.",
        ),
        bool_query_param(
            "only_missing_verdict",
            "When true, only bundles missing attestation verdict metadata are returned.",
        ),
    ]
}

fn offline_receipt_query_parameters() -> Vec<Value> {
    vec![
        string_query_param(
            "controller_id",
            "Filter receipts by sender/controller account (accepts IH58 (preferred)/sora (second-best)/public-key literals).",
        ),
        string_query_param(
            "receiver_id",
            "Filter receipts by receiver account (accepts IH58 (preferred)/sora (second-best)/public-key literals).",
        ),
        string_query_param(
            "bundle_id_hex",
            "Restrict receipts to a specific bundle identifier (hex, case-insensitive).",
        ),
        string_query_param(
            "certificate_id_hex",
            "Restrict receipts to a specific certificate identifier (hex, case-insensitive).",
        ),
        string_query_param("invoice_id", "Filter receipts by invoice identifier."),
        string_query_param("asset_id", "Filter receipts by asset identifier."),
    ]
}

fn string_query_param(name: &str, description: &str) -> Value {
    string_query_param_with_required(name, description, false)
}

fn required_string_query_param(name: &str, description: &str) -> Value {
    string_query_param_with_required(name, description, true)
}

fn string_query_param_with_required(name: &str, description: &str, required: bool) -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String(name.to_owned()));
    param.insert("in".into(), Value::String("query".to_owned()));
    param.insert("required".into(), Value::Bool(required));
    param.insert("description".into(), Value::String(description.to_owned()));
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    Value::Object(param)
}

fn integer_query_param(name: &str, description: &str, format: Option<&str>) -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String(name.to_owned()));
    param.insert("in".into(), Value::String("query".to_owned()));
    param.insert("required".into(), Value::Bool(false));
    param.insert("description".into(), Value::String(description.to_owned()));
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("integer".to_owned()));
    if let Some(fmt) = format {
        schema.insert("format".into(), Value::String(fmt.to_owned()));
    }
    param.insert("schema".into(), Value::Object(schema));
    Value::Object(param)
}

fn bool_query_param(name: &str, description: &str) -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String(name.to_owned()));
    param.insert("in".into(), Value::String("query".to_owned()));
    param.insert("required".into(), Value::Bool(false));
    param.insert("description".into(), Value::String(description.to_owned()));
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("boolean".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    Value::Object(param)
}

fn address_format_query_param() -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String("address_format".to_owned()));
    param.insert("in".into(), Value::String("query".to_owned()));
    param.insert("required".into(), Value::Bool(false));
    param.insert(
        "description".into(),
        Value::String("Preferred address format (`ih58` or `compressed`).".to_owned()),
    );
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    schema.insert(
        "enum".into(),
        Value::Array(vec![
            Value::String("ih58".to_owned()),
            Value::String("compressed".to_owned()),
        ]),
    );
    param.insert("schema".into(), Value::Object(schema));
    Value::Object(param)
}

fn path_param(name: &str, description: &str) -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String(name.to_owned()));
    param.insert("in".into(), Value::String("path".to_owned()));
    param.insert("required".into(), Value::Bool(true));
    param.insert("description".into(), Value::String(description.to_owned()));
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    Value::Object(param)
}

fn offline_query_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert("schema".into(), schema_ref("OfflineQueryEnvelope"));
            schema
        }),
    );
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn offline_allowances_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline allowances retrieved.",
            schema_ref("OfflineAllowanceListResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid filter or pagination parameters.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_allowance_issue_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert("schema".into(), schema_ref("OfflineAllowanceIssueRequest"));
            schema
        }),
    );
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn offline_allowance_issue_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline allowance registration enqueued.",
            schema_ref("OfflineAllowanceIssueResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid allowance payload.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_certificate_issue_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert(
                "schema".into(),
                schema_ref("OfflineCertificateIssueRequest"),
            );
            schema
        }),
    );
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn offline_certificate_issue_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline certificate issued.",
            schema_ref("OfflineCertificateIssueResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid certificate payload.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_allowance_detail_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline allowance retrieved.",
            schema_ref("OfflineAllowanceItem"),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response_with_headers(
            "Offline allowance not found.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_certificate_renew_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert(
                "schema".into(),
                schema_ref("OfflineCertificateRenewRequest"),
            );
            schema
        }),
    );
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn offline_certificate_renew_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline allowance renewal enqueued.",
            schema_ref("OfflineCertificateRenewResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid renewal payload.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_certificate_revoke_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert(
                "schema".into(),
                schema_ref("OfflineCertificateRevokeRequest"),
            );
            schema
        }),
    );
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn offline_certificate_revoke_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline verdict revocation enqueued.",
            schema_ref("OfflineCertificateRevokeResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid revocation payload.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_settlement_submit_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert(
                "schema".into(),
                schema_ref("OfflineSettlementSubmitRequest"),
            );
            schema
        }),
    );
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn offline_settlement_submit_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline settlement submission enqueued.",
            schema_ref("OfflineSettlementSubmitResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid settlement payload.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_spend_receipts_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert(
                "schema".into(),
                schema_ref("OfflineSpendReceiptsSubmitRequest"),
            );
            schema
        }),
    );
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn offline_spend_receipts_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline receipts validated.",
            schema_ref("OfflineSpendReceiptsSubmitResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid receipt payload.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_state_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline state snapshot retrieved.",
            schema_ref("OfflineStateResponse"),
        ),
    );
    responses
}

fn offline_bundle_proof_status_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline bundle proof status retrieved.",
            schema_ref("OfflineBundleProofStatusResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid bundle id.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response_with_headers(
            "Offline bundle not found.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_receipts_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline receipts retrieved.",
            schema_ref("OfflineReceiptListResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid filter or pagination parameters.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_transfers_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline transfer bundles retrieved.",
            schema_ref("OfflineTransferListResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid filter or pagination parameters.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_transfer_detail_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline transfer bundle retrieved.",
            schema_ref("OfflineTransferItem"),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response_with_headers(
            "Offline transfer bundle not found.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_rejections_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline rejection counters retrieved.",
            schema_ref("OfflineRejectionListResponse"),
        ),
    );
    responses.insert(
        "403".to_owned(),
        json_response_with_headers(
            "Telemetry profile forbids exposing metrics.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

fn offline_summaries_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Offline counter summaries retrieved.",
            schema_ref("OfflineSummaryListResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response_with_headers(
            "Invalid filter or pagination parameters.",
            error_schema_reference(),
            offline_reject_headers(),
        ),
    );
    responses
}

#[cfg(test)]
mod offline_header_tests {
    use super::*;

    #[test]
    fn offline_error_responses_advertise_reject_code_header() {
        let responses = offline_allowances_responses();
        let response = responses
            .get("400")
            .expect("offline allowances responses should define 400 response")
            .as_object()
            .expect("response should be an object");
        let headers = response
            .get("headers")
            .expect("offline error response should include headers")
            .as_object()
            .expect("headers should be an object");
        assert!(
            headers.contains_key("x-iroha-reject-code"),
            "offline error responses should advertise x-iroha-reject-code"
        );
    }
}

fn system_paths() -> Map {
    let mut paths = Map::new();
    paths.insert("/health".to_owned(), Value::Object(health_operation()));
    paths.insert(
        "/status".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch node status snapshot.",
            "Returns the node status payload.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/status/{tail}".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch a status sub-path snapshot.",
            "Returns a status sub-tree under the requested tail segment.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("tail", "Status sub-path selector.")],
        )),
    );
    paths.insert(
        "/metrics".to_owned(),
        Value::Object(text_get_operation(
            "System",
            "Fetch Prometheus metrics.",
            "Expose Prometheus metrics in text format.",
            None,
        )),
    );
    paths.insert(
        "/api_version".to_owned(),
        Value::Object(text_get_operation(
            "System",
            "Fetch the active API version.",
            "Returns the block header version string. Responds with 503 if genesis is not committed.",
            None,
        )),
    );
    paths.insert(
        "/v1/api/versions".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "List supported Torii API versions.",
            "Return the supported Torii API versions and defaults.",
            "#/components/schemas/ApiVersionInfo",
            Vec::new(),
        )),
    );
    paths.insert(
        "/peers".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "List online peers.",
            "Return the peers snapshot as a list of peer ids.",
            "#/components/schemas/PeerIdList",
            Vec::new(),
        )),
    );
    paths.insert(
        "/configuration".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "System",
                "Fetch Torii configuration snapshot.",
                "Return the live configuration snapshot.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let post_op = json_post_operation(
                "System",
                "Update Torii configuration.",
                "Submit configuration overrides for live reload.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/schema".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch the data model schema snapshot.",
            "Return the schema payload used by the node build.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/openapi".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch the OpenAPI specification.",
            "Return the Torii OpenAPI document.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/openapi.json".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch the OpenAPI specification as JSON.",
            "Return the Torii OpenAPI document.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/debug/pprof/profile".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("System".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Capture a CPU profile.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String("Return a profiling payload for the configured duration.".to_owned()),
            );
            let mut responses = Map::new();
            responses.insert("200".to_owned(), binary_response("CPU profile payload."));
            operation.insert("responses".into(), Value::Object(responses));
            let mut methods = Map::new();
            methods.insert("get".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/policy".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch node policy snapshot.",
            "Return the current node policy and limits.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/pipeline/recovery/{height}".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch pipeline recovery metadata.",
            "Return pipeline recovery metadata for the given height.",
            "#/components/schemas/JsonValue",
            vec![integer_path_param(
                "height",
                "Block height to inspect.",
                Some("uint64"),
            )],
        )),
    );
    paths.insert(
        "/v1/debug/axt/cache".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Inspect cached AXT proof state.",
            "Return the cached AXT proof snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/telemetry/peers-info".to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch peer telemetry summary.",
            "Return the latest peer telemetry snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn operator_auth_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/operator/auth/registration/options".to_owned(),
        Value::Object(operator_auth_registration_options_operation()),
    );
    paths.insert(
        "/v1/operator/auth/registration/verify".to_owned(),
        Value::Object(operator_auth_registration_verify_operation()),
    );
    paths.insert(
        "/v1/operator/auth/login/options".to_owned(),
        Value::Object(operator_auth_login_options_operation()),
    );
    paths.insert(
        "/v1/operator/auth/login/verify".to_owned(),
        Value::Object(operator_auth_login_verify_operation()),
    );
    paths
}

fn transaction_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/transaction".to_owned(),
        Value::Object(binary_post_operation(
            "Transactions",
            "Submit a signed transaction.",
            "Submit a SignedTransaction encoded as Norito bytes.",
            "#/components/schemas/JsonValue",
        )),
    );
    let mut pipeline_status = json_get_operation(
        "Transactions",
        "Fetch pipeline transaction status.",
        "Return the latest pipeline status for a signed transaction hash.",
        "#/components/schemas/JsonValue",
        vec![required_string_query_param(
            "hash",
            "Transaction hash (hex).",
        )],
    );
    if let Some(Value::Object(get_op)) = pipeline_status.get_mut("get") {
        if let Some(Value::Object(responses)) = get_op.get_mut("responses") {
            responses.insert(
                "404".to_owned(),
                json_response(
                    "Pipeline status not found; Torii has no cached entry for the hash yet.",
                    error_schema_reference(),
                ),
            );
        }
    }
    paths.insert(
        "/v1/pipeline/transactions/status".to_owned(),
        Value::Object(pipeline_status),
    );
    paths.insert(
        "/v1/iso20022/pacs008".to_owned(),
        Value::Object(json_post_operation(
            "ISO20022",
            "Submit ISO 20022 pacs.008 payload.",
            "Submit a pacs.008 message for ISO 20022 bridging.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/iso20022/pacs009".to_owned(),
        Value::Object(json_post_operation(
            "ISO20022",
            "Submit ISO 20022 pacs.009 payload.",
            "Submit a pacs.009 message for ISO 20022 bridging.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/iso20022/status/{msg_id}".to_owned(),
        Value::Object(json_get_operation(
            "ISO20022",
            "Fetch ISO 20022 message status.",
            "Return ISO 20022 message status by message id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("msg_id", "ISO 20022 message id.")],
        )),
    );
    paths
}

fn query_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/query".to_owned(),
        Value::Object(binary_post_operation(
            "Queries",
            "Submit a signed query.",
            "Submit a SignedQuery encoded as Norito bytes.",
            "#/components/schemas/JsonValue",
        )),
    );
    paths
}

fn stream_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/events/sse".to_owned(),
        Value::Object(event_stream_get_operation(
            "Streams",
            "Subscribe to event stream.",
            "Stream pipeline events via Server-Sent Events.",
        )),
    );
    paths.insert(
        "/events".to_owned(),
        Value::Object(text_get_operation(
            "Streams",
            "Connect to the event WebSocket.",
            "Upgrade to the event subscription WebSocket.",
            None,
        )),
    );
    paths.insert(
        "/block/stream".to_owned(),
        Value::Object(text_get_operation(
            "Streams",
            "Connect to the block stream WebSocket.",
            "Upgrade to the block stream WebSocket.",
            None,
        )),
    );
    paths.insert(
        "/p2p".to_owned(),
        Value::Object(text_get_operation(
            "Streams",
            "Connect to the P2P relay WebSocket.",
            "Upgrade to the P2P relay WebSocket.",
            None,
        )),
    );
    paths
}

fn connect_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/connect/session".to_owned(),
        Value::Object(json_post_operation(
            "Connect",
            "Open a Connect session.",
            "Create a Connect session for wallet pairing.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/connect/session/{sid}".to_owned(),
        Value::Object(json_delete_operation(
            "Connect",
            "Close a Connect session.",
            "Terminate a Connect session by session id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("sid", "Connect session id.")],
        )),
    );
    paths.insert(
        "/v1/connect/ws".to_owned(),
        Value::Object(text_get_operation(
            "Connect",
            "Connect to the Connect WebSocket.",
            "Upgrade to the Connect WebSocket stream.",
            None,
        )),
    );
    paths.insert(
        "/v1/connect/status".to_owned(),
        Value::Object(json_get_operation(
            "Connect",
            "Fetch Connect status.",
            "Return Connect relay status information.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn proof_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/proofs/{id}".to_owned(),
        Value::Object(json_get_operation(
            "Proofs",
            "Fetch a proof record.",
            "Return a proof record by id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Proof record identifier.")],
        )),
    );
    paths.insert(
        "/v1/proofs/retention".to_owned(),
        Value::Object(json_get_operation(
            "Proofs",
            "Fetch proof retention status.",
            "Return proof retention configuration and counters.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/proofs/query".to_owned(),
        Value::Object(json_post_operation(
            "Proofs",
            "Query proof records.",
            "Query proof records with a JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn contracts_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/contracts/code".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Register contract code.",
            "Submit contract bytecode for registration.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/code/{code_hash}".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "Fetch contract metadata.",
            "Fetch contract metadata by code hash.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("code_hash", "Contract code hash (hex).")],
        )),
    );
    paths.insert(
        "/v1/contracts/code-bytes/{code_hash}".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "Fetch contract code bytes.",
            "Fetch contract code bytes (base64) by code hash.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("code_hash", "Contract code hash (hex).")],
        )),
    );
    paths.insert(
        "/v1/contracts/deploy".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Deploy a contract.",
            "Deploy contract code to a namespace.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/instance".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Create a contract instance.",
            "Create a contract instance from registered code.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/instance/activate".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Activate a contract instance.",
            "Activate a previously created contract instance.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/call".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Call a contract instance.",
            "Invoke a contract instance entrypoint.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/state".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "Read smart contract state.",
            "Read smart contract state by exact path, path list, or prefix.",
            "#/components/schemas/JsonValue",
            vec![
                string_query_param("path", "Exact state key path (Name)."),
                string_query_param("paths", "Comma-separated list of state key paths (Names)."),
                string_query_param("prefix", "Prefix for state key paths (Name)."),
                bool_query_param(
                    "include_value",
                    "Include base64-encoded values (default true).",
                ),
                integer_query_param("offset", "Prefix query offset (default 0).", Some("uint64")),
                integer_query_param(
                    "limit",
                    "Prefix query limit (default 1000, max 10_000).",
                    Some("uint64"),
                ),
            ],
        )),
    );
    paths.insert(
        "/v1/contracts/instances/{ns}".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "List contract instances by namespace.",
            "Return active contract instances for a namespace.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("ns", "Contract namespace identifier.")],
        )),
    );
    paths.insert(
        "/v1/confidential/derive-keyset".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Derive a confidential keyset.",
            "Derive a confidential compute keyset for a lane.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn zk_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/zk/roots".to_owned(),
        Value::Object(json_post_operation(
            "ZK",
            "Fetch ZK roots.",
            "Fetch zero-knowledge root set information.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/verify".to_owned(),
        Value::Object(json_post_operation(
            "ZK",
            "Verify a ZK proof.",
            "Verify a zero-knowledge proof envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/submit-proof".to_owned(),
        Value::Object(json_post_operation(
            "ZK",
            "Submit a ZK proof.",
            "Submit a proof for verification and recording.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/attachments".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "ZK",
                "List ZK attachments.",
                "List stored ZK attachments.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let post_op = json_post_operation(
                "ZK",
                "Create a ZK attachment.",
                "Upload a ZK attachment.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/v1/zk/attachments/{id}".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "ZK",
                "Fetch a ZK attachment.",
                "Fetch a stored ZK attachment by id.",
                "#/components/schemas/JsonValue",
                vec![string_path_param("id", "Attachment identifier.")],
            );
            let delete_op = json_delete_operation(
                "ZK",
                "Delete a ZK attachment.",
                "Delete a ZK attachment by id.",
                "#/components/schemas/JsonValue",
                vec![string_path_param("id", "Attachment identifier.")],
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(delete_value) = delete_op.get("delete") {
                methods.insert("delete".to_owned(), delete_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/v1/zk/attachments/count".to_owned(),
        Value::Object(json_get_operation(
            "ZK",
            "Count ZK attachments.",
            "Return attachment counts.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/vote/tally".to_owned(),
        Value::Object(json_post_operation(
            "ZK",
            "Compute ZK vote tally.",
            "Compute a ZK ballot tally from the provided payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/vk/register".to_owned(),
        Value::Object(json_post_operation(
            "ZK",
            "Register a verification key.",
            "Register a verification key for a ZK backend.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/vk/update".to_owned(),
        Value::Object(json_post_operation(
            "ZK",
            "Update a verification key.",
            "Update a verification key entry.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/vk/{backend}/{name}".to_owned(),
        Value::Object(json_get_operation(
            "ZK",
            "Fetch a verification key.",
            "Fetch a verification key by backend and name.",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param("backend", "Verification backend label."),
                string_path_param("name", "Verification key name."),
            ],
        )),
    );
    paths.insert(
        "/v1/zk/vk".to_owned(),
        Value::Object(json_get_operation(
            "ZK",
            "List verification keys.",
            "List registered verification keys.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/proofs".to_owned(),
        Value::Object(json_get_operation(
            "ZK",
            "List proofs.",
            "List stored proofs.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/proofs/count".to_owned(),
        Value::Object(json_get_operation(
            "ZK",
            "Count proofs.",
            "Return total proof count.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/proof/{backend}/{hash}".to_owned(),
        Value::Object(json_get_operation(
            "ZK",
            "Fetch a proof by backend and hash.",
            "Fetch a proof record by backend and hash.",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param("backend", "Proof backend label."),
                string_path_param("hash", "Proof hash (hex)."),
            ],
        )),
    );
    paths.insert(
        "/v1/zk/proof-tags/{backend}/{hash}".to_owned(),
        Value::Object(json_get_operation(
            "ZK",
            "Fetch proof tags.",
            "Fetch proof tags by backend and hash.",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param("backend", "Proof backend label."),
                string_path_param("hash", "Proof hash (hex)."),
            ],
        )),
    );
    paths
}

fn governance_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/gov/proposals/deploy-contract".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Propose contract deployment.",
            "Submit a governance proposal for contract deployment; optionally sign and submit when authority/private_key are provided.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/proposals/{id}".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch a proposal.",
            "Fetch a governance proposal by id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Proposal identifier.")],
        )),
    );
    paths.insert(
        "/v1/gov/locks/{rid}".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch governance locks.",
            "Fetch governance lock records by referendum id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("rid", "Referendum identifier.")],
        )),
    );
    paths.insert(
        "/v1/gov/referenda/{id}".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch a referendum.",
            "Fetch a referendum by id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Referendum identifier.")],
        )),
    );
    paths.insert(
        "/v1/gov/tally/{id}".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch a tally snapshot.",
            "Fetch a tally snapshot by referendum id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Referendum identifier.")],
        )),
    );
    paths.insert(
        "/v1/gov/ballots/zk".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Submit a ZK ballot.",
            "Submit a zero-knowledge ballot; Torii submits when private_key is provided.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/ballots/zk-v1".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Submit a ZK ballot (v1).",
            "Submit a ZK ballot using the v1 envelope; Torii submits when private_key is provided.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/ballots/zk-v1/ballot-proof".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Submit a ballot proof.",
            "Submit a ZK ballot proof bundle; Torii submits when private_key is provided.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/ballots/plain".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Submit a plain ballot.",
            "Submit a non-ZK ballot; Torii submits when private_key is provided.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/finalize".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Finalize a referendum.",
            "Finalize referendum tally and status; Torii submits when authority/private_key are provided.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/protected-namespaces".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "Governance",
                "List protected namespaces.",
                "List protected namespaces for governance.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let post_op = json_post_operation(
                "Governance",
                "Update protected namespaces.",
                "Submit protected namespace updates.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/v1/gov/unlocks/stats".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch unlock statistics.",
            "Return governance unlock statistics.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/instances/{ns}".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "List contract instances by namespace.",
            "List governance contract instances for a namespace.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("ns", "Namespace identifier.")],
        )),
    );
    paths.insert(
        "/v1/gov/enact".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Enact a referendum.",
            "Enact an approved referendum; Torii submits when authority/private_key are provided.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/council/current".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch current council.",
            "Return the current governance council roster.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/council/persist".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Persist a council roster.",
            "Persist a governance council roster for an epoch.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/council/replace".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Replace a council member.",
            "Replace a council member using the next alternate.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/council/audit".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Audit council derivation.",
            "Return governance council derivation audit data.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/council/derive-vrf".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Derive council VRF inputs.",
            "Derive VRF inputs for council selection.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn runtime_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/node/capabilities".to_owned(),
        Value::Object(json_get_operation(
            "Runtime",
            "Fetch node capabilities.",
            "Return supported runtime capabilities.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/runtime/abi/active".to_owned(),
        Value::Object(json_get_operation(
            "Runtime",
            "Fetch active ABI versions.",
            "Return active ABI version list.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/runtime/abi/hash".to_owned(),
        Value::Object(json_get_operation(
            "Runtime",
            "Fetch the active ABI hash.",
            "Return the ABI hash for the active policy.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/runtime/metrics".to_owned(),
        Value::Object(json_get_operation(
            "Runtime",
            "Fetch runtime metrics.",
            "Return runtime metrics snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/runtime/upgrades".to_owned(),
        Value::Object(json_get_operation(
            "Runtime",
            "List runtime upgrades.",
            "List proposed and activated runtime upgrades.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/runtime/upgrades/propose".to_owned(),
        Value::Object(json_post_operation(
            "Runtime",
            "Propose a runtime upgrade.",
            "Submit a runtime upgrade proposal.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/runtime/upgrades/activate/{id}".to_owned(),
        Value::Object(json_post_operation(
            "Runtime",
            "Activate a runtime upgrade.",
            "Activate a runtime upgrade by id.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Upgrade identifier.")],
        )),
    );
    paths.insert(
        "/v1/runtime/upgrades/cancel/{id}".to_owned(),
        Value::Object(json_post_operation(
            "Runtime",
            "Cancel a runtime upgrade.",
            "Cancel a runtime upgrade by id.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Upgrade identifier.")],
        )),
    );
    paths
}

fn account_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/accounts".to_owned(),
        Value::Object(json_get_operation(
            "Accounts",
            "List accounts.",
            "List accounts visible to the caller.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/query".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Query accounts.",
            "Query accounts with JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/resolve".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Resolve account literals.",
            "Resolve account literals into canonical IH58 identifiers.",
            "#/components/schemas/AccountResolveRequest",
            "#/components/schemas/AccountResolveResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/onboard".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Onboard an account.",
            "Register or onboard an account; when the UAID is not bound to the global dataspace, Torii publishes a default manifest to bind it (requires CanPublishSpaceDirectoryManifest{dataspace=0}).",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/transactions/query".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Query account transactions.",
            "Query transactions for a specific account.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param("account_id", "Account identifier.")],
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/assets".to_owned(),
        Value::Object(json_get_operation(
            "Accounts",
            "List account assets.",
            "List assets held by an account.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("account_id", "Account identifier.")],
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/assets/query".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Query account assets.",
            "Query assets held by an account.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param("account_id", "Account identifier.")],
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/permissions".to_owned(),
        Value::Object(json_get_operation(
            "Accounts",
            "List account permissions.",
            "List permissions granted to an account.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("account_id", "Account identifier.")],
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/transactions".to_owned(),
        Value::Object(json_get_operation(
            "Accounts",
            "List account transactions.",
            "List transactions authored by an account.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("account_id", "Account identifier.")],
        )),
    );
    paths.insert(
        "/v1/accounts/{uaid}/portfolio".to_owned(),
        Value::Object(json_get_operation(
            "Accounts",
            "Fetch account portfolio.",
            "Fetch the asset portfolio for an account identifier.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("uaid", "User account identifier.")],
        )),
    );
    paths
}

fn domain_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/domains".to_owned(),
        Value::Object(json_get_operation(
            "Domains",
            "List domains.",
            "List registered domains.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/domains/query".to_owned(),
        Value::Object(json_post_operation(
            "Domains",
            "Query domains.",
            "Query domains with JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn asset_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/assets/definitions".to_owned(),
        Value::Object(json_get_operation(
            "Assets",
            "List asset definitions.",
            "List asset definitions.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/assets/definitions/query".to_owned(),
        Value::Object(json_post_operation(
            "Assets",
            "Query asset definitions.",
            "Query asset definitions with JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/assets/{definition_id}/holders".to_owned(),
        Value::Object(json_get_operation(
            "Assets",
            "List asset holders.",
            "List holders for an asset definition.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "definition_id",
                "Asset definition identifier.",
            )],
        )),
    );
    paths.insert(
        "/v1/assets/{definition_id}/holders/query".to_owned(),
        Value::Object(json_post_operation(
            "Assets",
            "Query asset holders.",
            "Query holders for an asset definition.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "definition_id",
                "Asset definition identifier.",
            )],
        )),
    );
    paths.insert(
        "/v1/confidential/assets/{definition_id}/transitions".to_owned(),
        Value::Object(json_get_operation(
            "Assets",
            "Fetch confidential asset transitions.",
            "Fetch confidential asset transition history for an asset definition.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "definition_id",
                "Asset definition identifier.",
            )],
        )),
    );
    paths
}

fn nft_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/nfts".to_owned(),
        Value::Object(json_get_operation(
            "NFTs",
            "List NFTs.",
            "List NFTs visible to the caller.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/nfts/query".to_owned(),
        Value::Object(json_post_operation(
            "NFTs",
            "Query NFTs.",
            "Query NFTs with JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn subscription_paths() -> Map {
    let mut paths = Map::new();
    let plan_query_params = vec![
        string_query_param("provider", "Filter plans by provider account id."),
        integer_query_param("limit", "Optional page size limit.", Some("uint64")),
        integer_query_param(
            "offset",
            "Optional starting offset (default 0).",
            Some("uint64"),
        ),
    ];
    let mut plans = json_get_operation(
        "Subscriptions",
        "List subscription plans.",
        "List subscription plans by provider.",
        "#/components/schemas/JsonValue",
        plan_query_params,
    );
    plans.extend(json_post_operation(
        "Subscriptions",
        "Create a subscription plan.",
        "Register a subscription plan on an asset definition.",
        "#/components/schemas/JsonValue",
        "#/components/schemas/JsonValue",
        Vec::new(),
    ));
    paths.insert("/v1/subscriptions/plans".to_owned(), Value::Object(plans));

    let subscription_query_params = vec![
        string_query_param("owned_by", "Filter subscriptions by subscriber account id."),
        string_query_param("provider", "Filter subscriptions by provider account id."),
        string_query_param(
            "status",
            "Filter by status (`active`, `paused`, `past_due`, `canceled`, `suspended`).",
        ),
        integer_query_param("limit", "Optional page size limit.", Some("uint64")),
        integer_query_param(
            "offset",
            "Optional starting offset (default 0).",
            Some("uint64"),
        ),
    ];
    let mut subs = json_get_operation(
        "Subscriptions",
        "List subscriptions.",
        "List subscriptions with optional filters.",
        "#/components/schemas/JsonValue",
        subscription_query_params,
    );
    subs.extend(json_post_operation(
        "Subscriptions",
        "Create a subscription.",
        "Create a subscription NFT and billing trigger.",
        "#/components/schemas/JsonValue",
        "#/components/schemas/JsonValue",
        Vec::new(),
    ));
    paths.insert("/v1/subscriptions".to_owned(), Value::Object(subs));

    let sub_param = string_path_param("subscription_id", "Subscription NFT identifier.");
    paths.insert(
        "/v1/subscriptions/{subscription_id}".to_owned(),
        Value::Object(json_get_operation(
            "Subscriptions",
            "Fetch a subscription.",
            "Fetch a subscription by NFT id.",
            "#/components/schemas/JsonValue",
            vec![sub_param.clone()],
        )),
    );
    paths.insert(
        "/v1/subscriptions/{subscription_id}/pause".to_owned(),
        Value::Object(json_post_operation(
            "Subscriptions",
            "Pause a subscription.",
            "Pause a subscription and unregister billing triggers.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![sub_param.clone()],
        )),
    );
    paths.insert(
        "/v1/subscriptions/{subscription_id}/resume".to_owned(),
        Value::Object(json_post_operation(
            "Subscriptions",
            "Resume a subscription.",
            "Resume a subscription and re-schedule billing.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![sub_param.clone()],
        )),
    );
    paths.insert(
        "/v1/subscriptions/{subscription_id}/cancel".to_owned(),
        Value::Object(json_post_operation(
            "Subscriptions",
            "Cancel a subscription.",
            "Cancel a subscription immediately or schedule cancellation at period end.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![sub_param.clone()],
        )),
    );
    paths.insert(
        "/v1/subscriptions/{subscription_id}/keep".to_owned(),
        Value::Object(json_post_operation(
            "Subscriptions",
            "Keep a subscription.",
            "Undo a scheduled period-end cancellation and keep billing active.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![sub_param.clone()],
        )),
    );
    paths.insert(
        "/v1/subscriptions/{subscription_id}/usage".to_owned(),
        Value::Object(json_post_operation(
            "Subscriptions",
            "Record subscription usage.",
            "Record usage for a subscription via a usage trigger.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![sub_param.clone()],
        )),
    );
    paths.insert(
        "/v1/subscriptions/{subscription_id}/charge-now".to_owned(),
        Value::Object(json_post_operation(
            "Subscriptions",
            "Charge a subscription now.",
            "Trigger immediate billing for a subscription.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![sub_param],
        )),
    );
    paths
}

fn parameter_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/parameters".to_owned(),
        Value::Object(json_get_operation(
            "Parameters",
            "Fetch parameter snapshots.",
            "Return system parameter snapshots.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn space_directory_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/space-directory/uaids/{uaid}".to_owned(),
        Value::Object(json_get_operation(
            "SpaceDirectory",
            "Fetch space directory bindings.",
            "Fetch bindings for a user account identifier.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("uaid", "User account identifier.")],
        )),
    );
    paths.insert(
        "/v1/space-directory/uaids/{uaid}/manifests".to_owned(),
        Value::Object(json_get_operation(
            "SpaceDirectory",
            "Fetch space directory manifests.",
            "Fetch manifests registered for a user account identifier.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("uaid", "User account identifier.")],
        )),
    );
    paths.insert(
        "/v1/space-directory/manifests".to_owned(),
        Value::Object(json_post_operation(
            "SpaceDirectory",
            "Publish a space directory manifest.",
            "Publish a space directory manifest.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/space-directory/manifests/revoke".to_owned(),
        Value::Object(json_post_operation(
            "SpaceDirectory",
            "Revoke a space directory manifest.",
            "Revoke a space directory manifest.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn explorer_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/explorer/accounts".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List accounts (explorer).",
            "List accounts for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/domains".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List domains (explorer).",
            "List domains for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/asset-definitions".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List asset definitions (explorer).",
            "List asset definitions for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/assets".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List assets (explorer).",
            "List assets for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/nfts".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List NFTs (explorer).",
            "List NFTs for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/blocks".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List blocks (explorer).",
            "List blocks for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/transactions".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List transactions (explorer).",
            "List transactions for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/transactions/stream".to_owned(),
        Value::Object(event_stream_get_operation(
            "Explorer",
            "Stream transactions (explorer).",
            "Stream transaction updates via SSE.",
        )),
    );
    paths.insert(
        "/v1/explorer/instructions".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List instructions (explorer).",
            "List instructions for explorer usage.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/instructions/stream".to_owned(),
        Value::Object(event_stream_get_operation(
            "Explorer",
            "Stream instructions (explorer).",
            "Stream instruction updates via SSE.",
        )),
    );
    paths.insert(
        "/v1/explorer/metrics".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch explorer metrics.",
            "Return explorer metrics snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/explorer/accounts/{account_id}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch account detail (explorer).",
            "Fetch account detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("account_id", "Account identifier.")],
        )),
    );
    paths.insert(
        "/v1/explorer/accounts/{account_id}/qr".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("Explorer".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Fetch account QR code.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String("Return a QR code for the account identifier.".to_owned()),
            );
            operation.insert(
                "parameters".into(),
                Value::Array(vec![string_path_param("account_id", "Account identifier.")]),
            );
            let mut responses = Map::new();
            responses.insert("200".to_owned(), binary_response("QR code payload."));
            operation.insert("responses".into(), Value::Object(responses));
            let mut methods = Map::new();
            methods.insert("get".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/explorer/domains/{domain_id}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch domain detail (explorer).",
            "Fetch domain detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("domain_id", "Domain identifier.")],
        )),
    );
    paths.insert(
        "/v1/explorer/asset-definitions/{definition_id}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch asset definition detail (explorer).",
            "Fetch asset definition detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "definition_id",
                "Asset definition identifier.",
            )],
        )),
    );
    paths.insert(
        "/v1/explorer/assets/{asset_id}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch asset detail (explorer).",
            "Fetch asset detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("asset_id", "Asset identifier.")],
        )),
    );
    paths.insert(
        "/v1/explorer/nfts/{nft_id}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch NFT detail (explorer).",
            "Fetch NFT detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("nft_id", "NFT identifier.")],
        )),
    );
    paths.insert(
        "/v1/explorer/blocks/{identifier}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch block detail (explorer).",
            "Fetch block detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "identifier",
                "Block hash or height identifier.",
            )],
        )),
    );
    paths.insert(
        "/v1/explorer/transactions/{hash}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch transaction detail (explorer).",
            "Fetch transaction detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("hash", "Transaction hash.")],
        )),
    );
    paths.insert(
        "/v1/explorer/instructions/{hash}/{index}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch instruction detail (explorer).",
            "Fetch instruction detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param("hash", "Transaction hash."),
                integer_path_param("index", "Instruction index.", Some("uint64")),
            ],
        )),
    );
    paths
}

fn sorafs_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/sorafs/pin/register".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Register a pin manifest.",
            "Register a pin manifest for SoraFS.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/declare".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Declare capacity.",
            "Declare SoraFS capacity availability.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/telemetry".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit capacity telemetry.",
            "Submit SoraFS capacity telemetry.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/dispute".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit a capacity dispute.",
            "Submit a capacity dispute request.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/schedule".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit a capacity schedule.",
            "Submit a capacity schedule request.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/complete".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit a capacity completion.",
            "Submit a capacity completion notice.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/uptime".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit capacity uptime.",
            "Submit capacity uptime metrics.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/por-challenge".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit PoR challenge.",
            "Submit proof-of-replication challenge.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/por-proof".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit PoR proof.",
            "Submit proof-of-replication proof.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/por-verdict".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit PoR verdict.",
            "Submit proof-of-replication verdict.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/por/status".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch PoR status.",
            "Fetch proof-of-replication status.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/por/export".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Export PoR report.",
            "Export the PoR report bundle.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/por/ingestion/{manifest_digest_hex}".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch PoR ingestion status.",
            "Fetch PoR ingestion status for a manifest digest.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "manifest_digest_hex",
                "Manifest digest (hex).",
            )],
        )),
    );
    paths.insert(
        "/v1/sorafs/por/report/{iso_week}".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch PoR report.",
            "Fetch PoR report for an ISO week.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("iso_week", "ISO week label.")],
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/por".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit PoR capacity report.",
            "Submit PoR capacity report.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/failure".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit capacity failure report.",
            "Submit capacity failure report.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/audit/repair/report".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit repair report.",
            "Submit a SoraFS repair report.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/audit/repair/slash".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit repair slash.",
            "Submit a SoraFS repair slash.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/audit/repair/claim".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Claim repair ticket.",
            "Claim a SoraFS repair ticket (requires manifest_digest_hex, idempotency key, and worker signature).",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/audit/repair/heartbeat".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Repair heartbeat.",
            "Record a SoraFS repair worker heartbeat (requires manifest_digest_hex, idempotency key, and worker signature).",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/audit/repair/complete".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Complete repair ticket.",
            "Complete a SoraFS repair ticket (requires manifest_digest_hex, idempotency key, and worker signature).",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/audit/repair/fail".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Fail repair ticket.",
            "Fail a SoraFS repair ticket (requires manifest_digest_hex, idempotency key, and worker signature).",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    let repair_status_query_params = vec![
        string_query_param(
            "status",
            "Filter by repair status (queued, verifying, in_progress, completed, failed, escalated).",
        ),
        string_query_param("provider", "Filter by provider id (hex)."),
    ];
    paths.insert(
        "/v1/sorafs/audit/repair/status".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "List repair status.",
            "List SoraFS repair status across manifests (each entry includes Norito base64 record + ordered event log).",
            "#/components/schemas/JsonValue",
            repair_status_query_params.clone(),
        )),
    );
    paths.insert(
        "/v1/sorafs/audit/repair/status/{manifest_hex}".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch repair status.",
            "Fetch repair status for a manifest (each entry includes Norito base64 record + ordered event log).",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param("manifest_hex", "Manifest hash (hex)."),
                string_query_param(
                    "status",
                    "Filter by repair status (queued, verifying, in_progress, completed, failed, escalated).",
                ),
                string_query_param("provider", "Filter by provider id (hex)."),
            ],
        )),
    );
    paths.insert(
        "/v1/sorafs/providers".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "List providers.",
            "List SoraFS providers.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/providers/advert".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit provider advert.",
            "Submit a SoraFS provider advert.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/capacity/state".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch capacity state.",
            "Fetch SoraFS capacity state.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/pin".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch pin registry.",
            "Fetch the SoraFS pin registry.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/pin/{digest_hex}".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch a pin manifest.",
            "Fetch a pin manifest by digest.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "digest_hex",
                "Pin manifest digest (hex).",
            )],
        )),
    );
    paths.insert(
        "/v1/sorafs/aliases".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "List aliases.",
            "List SoraFS aliases.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/replication".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "List replication orders.",
            "List SoraFS replication orders.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/state".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch storage state.",
            "Fetch SoraFS storage state.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/manifest/{manifest_id}".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch storage manifest.",
            "Fetch SoraFS storage manifest.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("manifest_id", "Manifest identifier.")],
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/plan/{manifest_id}".to_owned(),
        Value::Object(json_get_operation(
            "SoraFS",
            "Fetch storage plan.",
            "Fetch SoraFS storage plan.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("manifest_id", "Manifest identifier.")],
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/pin".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Pin storage content.",
            "Submit a storage pin request.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/fetch".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Fetch storage content.",
            "Submit a storage fetch request.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/token".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Request storage token.",
            "Request a storage access token.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/car/{manifest_id}".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("SoraFS".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Fetch CAR bytes.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String("Fetch CAR bytes for a manifest.".to_owned()),
            );
            operation.insert(
                "parameters".into(),
                Value::Array(vec![string_path_param(
                    "manifest_id",
                    "Manifest identifier.",
                )]),
            );
            let mut responses = Map::new();
            responses.insert("200".to_owned(), binary_response("CAR payload."));
            operation.insert("responses".into(), Value::Object(responses));
            let mut methods = Map::new();
            methods.insert("get".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("SoraFS".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Fetch storage chunk bytes.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String("Fetch a storage chunk by digest.".to_owned()),
            );
            operation.insert(
                "parameters".into(),
                Value::Array(vec![
                    string_path_param("manifest_id", "Manifest identifier."),
                    string_path_param("chunk_digest", "Chunk digest (hex)."),
                ]),
            );
            let mut responses = Map::new();
            responses.insert("200".to_owned(), binary_response("Chunk payload."));
            operation.insert("responses".into(), Value::Object(responses));
            let mut methods = Map::new();
            methods.insert("get".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/sorafs/storage/por-sample".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Request a PoR sample.",
            "Request a proof-of-replication sample.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/proof/stream".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Stream proofs.",
            "Request a proof stream payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/por-challenge".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit storage PoR challenge.",
            "Submit a storage PoR challenge.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/por-proof".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit storage PoR proof.",
            "Submit a storage PoR proof.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/storage/por-verdict".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit storage PoR verdict.",
            "Submit a storage PoR verdict.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/deal/usage".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit deal usage.",
            "Submit a SoraFS deal usage report.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sorafs/deal/settle".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Submit deal settlement.",
            "Submit a SoraFS deal settlement.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn soradns_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/soradns/directory/latest".to_owned(),
        Value::Object(json_get_operation(
            "SoraDNS",
            "Fetch latest directory snapshot.",
            "Fetch the latest SoraDNS directory snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/soradns/directory/events".to_owned(),
        Value::Object(event_stream_get_operation(
            "SoraDNS",
            "Stream directory events.",
            "Stream SoraDNS directory events via SSE.",
        )),
    );
    paths
}

fn content_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/content/{bundle}/{path}".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("Content".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Fetch content bundle bytes.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String(
                    "Fetch content bundle bytes (path captures the remaining path segments). \
Role- or sponsor-gated bundles require canonical request headers \
(`X-Iroha-Account`, `X-Iroha-Signature`)."
                        .to_owned(),
                ),
            );
            operation.insert(
                "parameters".into(),
                Value::Array(vec![
                    string_path_param("bundle", "Content bundle identifier."),
                    string_path_param("path", "Bundle-relative path."),
                ]),
            );
            let mut responses = Map::new();
            responses.insert("200".to_owned(), binary_response("Content payload."));
            operation.insert("responses".into(), Value::Object(responses));
            let mut methods = Map::new();
            methods.insert("get".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths
}

fn sns_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/sns/registrations".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Register a SNS suffix.",
            "Register a SNS suffix.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sns/registrations/{selector}".to_owned(),
        Value::Object(json_get_operation(
            "SNS",
            "Fetch a SNS registration.",
            "Fetch a SNS registration by selector.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("selector", "SNS selector value.")],
        )),
    );
    paths.insert(
        "/v1/sns/registrations/{selector}/renew".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Renew a SNS registration.",
            "Renew a SNS registration by selector.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param("selector", "SNS selector value.")],
        )),
    );
    paths.insert(
        "/v1/sns/registrations/{selector}/transfer".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Transfer a SNS registration.",
            "Transfer a SNS registration by selector.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param("selector", "SNS selector value.")],
        )),
    );
    paths.insert(
        "/v1/sns/registrations/{selector}/controllers".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Update SNS controllers.",
            "Update SNS registration controllers.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param("selector", "SNS selector value.")],
        )),
    );
    paths.insert(
        "/v1/sns/registrations/{selector}/freeze".to_owned(),
        Value::Object({
            let post_op = json_post_operation(
                "SNS",
                "Freeze a SNS registration.",
                "Freeze a SNS registration.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                vec![string_path_param("selector", "SNS selector value.")],
            );
            let delete_op = json_delete_operation(
                "SNS",
                "Unfreeze a SNS registration.",
                "Unfreeze a SNS registration.",
                "#/components/schemas/JsonValue",
                vec![string_path_param("selector", "SNS selector value.")],
            );
            let mut methods = Map::new();
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            if let Some(delete_value) = delete_op.get("delete") {
                methods.insert("delete".to_owned(), delete_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/v1/sns/policies/{suffix_id}".to_owned(),
        Value::Object(json_get_operation(
            "SNS",
            "Fetch SNS policy.",
            "Fetch SNS policy by suffix id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("suffix_id", "Suffix identifier.")],
        )),
    );
    paths.insert(
        "/v1/sns/governance/cases".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "SNS",
                "List SNS governance cases.",
                "List SNS governance cases.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let post_op = json_post_operation(
                "SNS",
                "Create a SNS governance case.",
                "Create a SNS governance case.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            methods
        }),
    );
    paths
}

fn soranet_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/soranet/privacy/event".to_owned(),
        Value::Object(json_post_operation(
            "SoraNet",
            "Submit a privacy event.",
            "Submit a SoraNet privacy event payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/soranet/privacy/share".to_owned(),
        Value::Object(json_post_operation(
            "SoraNet",
            "Submit a privacy share payload.",
            "Submit a SoraNet privacy share payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn push_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/notify/devices".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("Push".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Register a push device.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String("Register a device token for push notifications.".to_owned()),
            );
            operation.insert(
                "requestBody".into(),
                Value::Object(json_request_body(
                    "#/components/schemas/PushRegisterDeviceRequest",
                )),
            );
            let mut responses = Map::new();
            let mut accepted = Map::new();
            accepted.insert(
                "description".into(),
                Value::String("Device registration accepted.".to_owned()),
            );
            responses.insert("202".to_owned(), Value::Object(accepted));
            responses.insert(
                "400".to_owned(),
                json_response(
                    "Invalid device registration request.",
                    error_schema_reference(),
                ),
            );
            responses.insert(
                "429".to_owned(),
                json_response("Push registration rate limited.", error_schema_reference()),
            );
            responses.insert(
                "503".to_owned(),
                json_response(
                    "Push registration is unavailable.",
                    error_schema_reference(),
                ),
            );
            operation.insert("responses".into(), Value::Object(responses));
            let mut methods = Map::new();
            methods.insert("post".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths
}

fn webhook_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/webhooks".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "Webhooks",
                "List webhooks.",
                "List registered webhooks.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let post_op = json_post_operation(
                "Webhooks",
                "Create a webhook.",
                "Create a webhook subscription.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/v1/webhooks/{id}".to_owned(),
        Value::Object(json_delete_operation(
            "Webhooks",
            "Delete a webhook.",
            "Delete a webhook subscription.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Webhook identifier.")],
        )),
    );
    paths
}

fn kaigi_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/kaigi/relays".to_owned(),
        Value::Object(kaigi_relays_operation()),
    );
    paths.insert(
        "/v1/kaigi/relays/{relay_id}".to_owned(),
        Value::Object(kaigi_relay_detail_operation()),
    );
    paths.insert(
        "/v1/kaigi/relays/health".to_owned(),
        Value::Object(kaigi_relays_health_operation()),
    );
    paths.insert(
        "/v1/kaigi/relays/events".to_owned(),
        Value::Object(kaigi_relays_events_operation()),
    );
    paths
}

fn nexus_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/nexus/lifecycle".to_owned(),
        Value::Object(json_post_operation(
            "Nexus",
            "Apply a lane lifecycle plan.",
            "Apply a Nexus lane lifecycle plan.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/nexus/public_lanes/{lane_id}/validators".to_owned(),
        Value::Object(nexus_public_lane_validators_operation()),
    );
    paths.insert(
        "/v1/nexus/public_lanes/{lane_id}/stake".to_owned(),
        Value::Object(nexus_public_lane_stake_operation()),
    );
    paths.insert(
        "/v1/nexus/public_lanes/{lane_id}/rewards/pending".to_owned(),
        Value::Object(nexus_public_lane_rewards_operation()),
    );
    paths
}

fn sumeragi_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/sumeragi/evidence/count".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Count evidence entries.",
            "Return the total number of evidence entries.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/evidence".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "Sumeragi",
                "List evidence entries.",
                "List evidence entries recorded by the node.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let post_op = json_post_operation(
                "Sumeragi",
                "Submit evidence.",
                "Submit consensus evidence payloads.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let mut methods = Map::new();
            if let Some(get_value) = get_op.get("get") {
                methods.insert("get".to_owned(), get_value.clone());
            }
            if let Some(post_value) = post_op.get("post") {
                methods.insert("post".to_owned(), post_value.clone());
            }
            methods
        }),
    );
    paths.insert(
        "/v1/sumeragi/new_view/sse".to_owned(),
        Value::Object(event_stream_get_operation(
            "Sumeragi",
            "Stream new view events.",
            "Stream new view events via SSE.",
        )),
    );
    paths.insert(
        "/v1/sumeragi/new_view/json".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch new view status.",
            "Return new view status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/status".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch Sumeragi status.",
            "Return Sumeragi status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/status/sse".to_owned(),
        Value::Object(event_stream_get_operation(
            "Sumeragi",
            "Stream Sumeragi status.",
            "Stream Sumeragi status via SSE.",
        )),
    );
    paths.insert(
        "/v1/sumeragi/leader".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch current leader.",
            "Return the current Sumeragi leader snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/bls_keys".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch consensus BLS keys.",
            "Return the consensus BLS key set.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/qc".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch QC snapshots.",
            "Return quorum certificate snapshots.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/checkpoints".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch checkpoint snapshots.",
            "Return checkpoint snapshots.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/consensus-keys".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch consensus keys.",
            "Return consensus key material snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/commit-certificates".to_owned(),
        Value::Object(sumeragi_commit_qcs_operation()),
    );
    paths.insert(
        "/v1/sumeragi/rbc".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch RBC status.",
            "Return RBC status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/rbc/delivered/{height}/{view}".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch RBC delivered status.",
            "Return RBC delivered status for a block height and view.",
            "#/components/schemas/JsonValue",
            vec![
                Value::Object(block_height_parameter()),
                integer_path_param("view", "View change index.", Some("uint64")),
            ],
        )),
    );
    paths.insert(
        "/v1/sumeragi/pacemaker".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch pacemaker status.",
            "Return pacemaker status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/phases".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch phase status.",
            "Return consensus phase status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/bridge/finality/{height}".to_owned(),
        Value::Object(bridge_finality_operation()),
    );
    paths.insert(
        "/v1/bridge/finality/bundle/{height}".to_owned(),
        Value::Object(bridge_finality_bundle_operation()),
    );
    paths.insert(
        "/v1/sumeragi/validator-sets".to_owned(),
        Value::Object(sumeragi_validator_sets_operation()),
    );
    paths.insert(
        "/v1/sumeragi/validator-sets/{height}".to_owned(),
        Value::Object(sumeragi_validator_set_by_height_operation()),
    );
    paths.insert(
        "/v1/sumeragi/key-lifecycle".to_owned(),
        Value::Object(sumeragi_key_lifecycle_operation()),
    );
    paths.insert(
        "/v1/sumeragi/telemetry".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch Sumeragi telemetry.",
            "Return Sumeragi telemetry snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/params".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch Sumeragi parameters.",
            "Return Sumeragi consensus parameters.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/rbc/sessions".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "List RBC sessions.",
            "Return RBC session snapshots.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/commit_qc/{hash}".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch commit QC.",
            "Fetch commit QC by block hash.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("hash", "Block hash (hex).")],
        )),
    );
    paths.insert(
        "/v1/sumeragi/collectors".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch collector status.",
            "Return collector status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/vrf/penalties/{epoch}".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch VRF penalties.",
            "Fetch VRF penalties for an epoch.",
            "#/components/schemas/JsonValue",
            vec![integer_path_param(
                "epoch",
                "Epoch identifier.",
                Some("uint64"),
            )],
        )),
    );
    paths.insert(
        "/v1/sumeragi/vrf/epoch/{epoch}".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch VRF epoch data.",
            "Fetch VRF epoch data.",
            "#/components/schemas/JsonValue",
            vec![integer_path_param(
                "epoch",
                "Epoch identifier.",
                Some("uint64"),
            )],
        )),
    );
    paths.insert(
        "/v1/sumeragi/vrf/commit".to_owned(),
        Value::Object(json_post_operation(
            "Sumeragi",
            "Submit VRF commit.",
            "Submit VRF commit payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/vrf/reveal".to_owned(),
        Value::Object(json_post_operation(
            "Sumeragi",
            "Submit VRF reveal.",
            "Submit VRF reveal payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/rbc/sample".to_owned(),
        Value::Object(json_post_operation(
            "Sumeragi",
            "Sample RBC sessions.",
            "Request an RBC sampling payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths
}

fn sumeragi_commit_qcs_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Sumeragi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List recent commit certificates (newest first).".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the bounded history of commit certificates produced by validators. \
             Supports JSON or Norito content negotiation via the `Accept` header."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sumeragiQcs".to_owned()),
    );
    operation.insert("parameters".into(), Value::Array(history_window_params()));
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Commit certificate history (newest first).",
            schema_ref("QcList"),
        ),
    );
    responses.insert("406".to_owned(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn bridge_finality_bundle_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Bridge".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String(
            "Return a bridge commitment + justification bundle for a block height.".to_owned(),
        ),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns a commitment (block hash + authority set) and justification (signatures) \
             alongside the block header and commit certificate for the requested height."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("bridgeFinalityBundle".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![Value::Object(block_height_parameter())]),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json_response(
            "Finality bundle for the requested block height.",
            schema_ref("BridgeFinalityBundle"),
        ),
    );
    responses.insert(
        "404".into(),
        json_response(
            "Finality bundle not found for the requested height.",
            error_schema_reference(),
        ),
    );
    responses.insert("406".into(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn bridge_finality_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Bridge".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Return a self-contained finality proof for a block height.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the block header and commit certificate for the requested height, allowing \
             external chains to verify finality with the provided validator set signatures."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("bridgeFinalityProof".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![Value::Object(block_height_parameter())]),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json_response(
            "Finality proof for the requested block height.",
            schema_ref("BridgeFinalityProof"),
        ),
    );
    responses.insert(
        "404".into(),
        json_response(
            "Finality proof not found for the requested height.",
            error_schema_reference(),
        ),
    );
    responses.insert("406".into(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn sumeragi_key_lifecycle_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Sumeragi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List consensus key lifecycle records (newest first).".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the bounded history of consensus/committee key lifecycle records \
             (activation/expiry/HSM metadata) with JSON or Norito negotiation via `Accept`."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sumeragiKeyLifecycle".to_owned()),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Consensus key lifecycle history (newest first).",
            schema_ref("ConsensusKeyRecord"),
        ),
    );
    responses.insert("406".to_owned(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn sumeragi_validator_sets_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Sumeragi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List recent validator-set snapshots (newest first).".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns a bounded history of validator-set snapshots derived from commit certificates with optional `from`/`limit` query parameters and JSON/Norito negotiation via `Accept`."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sumeragiValidatorSets".to_owned()),
    );
    operation.insert("parameters".into(), Value::Array(history_window_params()));
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Validator-set history (newest first).",
            schema_ref("ValidatorSetSnapshotList"),
        ),
    );
    responses.insert("406".to_owned(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn sumeragi_validator_set_by_height_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Sumeragi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch a validator-set snapshot by block height.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the validator-set snapshot derived from the commit certificate for the specified block height."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sumeragiValidatorSetByHeight".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![Value::Object(block_height_parameter())]),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Validator-set snapshot for the requested height.",
            schema_ref("ValidatorSetSnapshot"),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response(
            "No commit certificate recorded for the requested height.",
            error_schema_reference(),
        ),
    );
    responses.insert("406".to_owned(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn repo_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/repo/agreements".to_owned(),
        Value::Object(repo_agreements_get_operation()),
    );
    paths.insert(
        "/v1/repo/agreements/query".to_owned(),
        Value::Object(repo_agreements_query_operation()),
    );
    paths
}

fn repo_agreements_get_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Settlement".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List active repo agreements".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String("Returns the active repo agreements recorded on-chain with optional pagination, filtering, and account address formatting.".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![
            query_param("limit", "integer", "Optional maximum number of entries to return."),
            query_param("offset", "integer", "Zero-based pagination offset (default 0)."),
            query_param("sort", "string", "Optional comma-separated sort expression (e.g., `id:asc,maturity_timestamp_ms:desc`)."),
            query_param("filter", "string", "JSON-encoded FilterExpr limiting results (fields: id, initiator, counterparty, custodian)."),
            query_param("address_format", "string", "Preferred address format (`ih58` or `compressed`)."),
        ]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(
            "#/components/schemas/RepoAgreementListResponse",
        )),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn repo_agreements_query_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Settlement".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Query repo agreements with JSON envelope".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String("Structured query endpoint accepting a JSON envelope with pagination, sort, filter, and address_format fields.".to_owned()),
    );
    operation.insert(
        "requestBody".into(),
        Value::Object(json_request_body(
            "#/components/schemas/RepoAgreementsQueryRequest",
        )),
    );
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(
            "#/components/schemas/RepoAgreementListResponse",
        )),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn query_param(name: &str, param_type: &str, description: &str) -> Value {
    norito::json!({
        "name": name,
        "in": "query",
        "required": false,
        "schema": { "type": param_type },
        "description": description
    })
}

fn single_json_response(schema_ref: &str) -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        norito::json!({
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": { "$ref": schema_ref }
                }
            }
        }),
    );
    responses
}

fn json_request_body(schema_ref: &str) -> Map {
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert(
        "content".into(),
        norito::json!({
            "application/json": {
                "schema": { "$ref": schema_ref }
            }
        }),
    );
    body
}

fn binary_request_body(description: &str) -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    schema.insert("format".into(), Value::String("binary".to_owned()));
    let mut media = Map::new();
    media.insert("schema".into(), Value::Object(schema));
    let mut content = Map::new();
    content.insert("application/x-norito".into(), Value::Object(media.clone()));
    content.insert("application/octet-stream".into(), Value::Object(media));
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("description".into(), Value::String(description.to_owned()));
    body.insert("content".into(), Value::Object(content));
    Value::Object(body)
}

fn binary_response(description: &str) -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    schema.insert("format".into(), Value::String("binary".to_owned()));
    let mut media = Map::new();
    media.insert("schema".into(), Value::Object(schema));
    let mut content = Map::new();
    content.insert("application/octet-stream".into(), Value::Object(media));
    let mut body = Map::new();
    body.insert("description".into(), Value::String(description.to_owned()));
    body.insert("content".into(), Value::Object(content));
    Value::Object(body)
}

fn json_get_operation(
    tag: &str,
    summary: &str,
    description: &str,
    schema_ref: &str,
    params: Vec<Value>,
) -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String(tag.to_owned())]),
    );
    operation.insert("summary".into(), Value::String(summary.to_owned()));
    operation.insert("description".into(), Value::String(description.to_owned()));
    if !params.is_empty() {
        operation.insert("parameters".into(), Value::Array(params));
    }
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(schema_ref)),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn json_post_operation(
    tag: &str,
    summary: &str,
    description: &str,
    request_schema_ref: &str,
    response_schema_ref: &str,
    params: Vec<Value>,
) -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String(tag.to_owned())]),
    );
    operation.insert("summary".into(), Value::String(summary.to_owned()));
    operation.insert("description".into(), Value::String(description.to_owned()));
    if !params.is_empty() {
        operation.insert("parameters".into(), Value::Array(params));
    }
    operation.insert(
        "requestBody".into(),
        Value::Object(json_request_body(request_schema_ref)),
    );
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(response_schema_ref)),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn json_delete_operation(
    tag: &str,
    summary: &str,
    description: &str,
    response_schema_ref: &str,
    params: Vec<Value>,
) -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String(tag.to_owned())]),
    );
    operation.insert("summary".into(), Value::String(summary.to_owned()));
    operation.insert("description".into(), Value::String(description.to_owned()));
    if !params.is_empty() {
        operation.insert("parameters".into(), Value::Array(params));
    }
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(response_schema_ref)),
    );
    let mut methods = Map::new();
    methods.insert("delete".to_owned(), Value::Object(operation));
    methods
}

fn text_get_operation(tag: &str, summary: &str, description: &str, example: Option<&str>) -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String(tag.to_owned())]),
    );
    operation.insert("summary".into(), Value::String(summary.to_owned()));
    operation.insert("description".into(), Value::String(description.to_owned()));
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        plain_text_response("Successful response.", example),
    );
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn event_stream_get_operation(tag: &str, summary: &str, description: &str) -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String(tag.to_owned())]),
    );
    operation.insert("summary".into(), Value::String(summary.to_owned()));
    operation.insert("description".into(), Value::String(description.to_owned()));
    let mut responses = Map::new();
    responses.insert("200".to_owned(), event_stream_response("Event stream."));
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn binary_post_operation(
    tag: &str,
    summary: &str,
    description: &str,
    response_schema_ref: &str,
) -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String(tag.to_owned())]),
    );
    operation.insert("summary".into(), Value::String(summary.to_owned()));
    operation.insert("description".into(), Value::String(description.to_owned()));
    operation.insert(
        "requestBody".into(),
        binary_request_body("Binary Norito payload."),
    );
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(response_schema_ref)),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn kaigi_relays_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Kaigi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List registered Kaigi relays with their latest health sample.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns relay identifiers, domains, bandwidth classes, HPKE fingerprints, \
             and the freshest health report (if available)."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("kaigiRelaysList".to_owned()),
    );
    operation.insert("responses".into(), Value::Object(kaigi_relays_responses()));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn kaigi_relays_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Relay summaries retrieved.",
            schema_ref("KaigiRelaySummaryList"),
        ),
    );
    responses.insert(
        "503".to_owned(),
        json_response(
            "Telemetry profile does not permit relay telemetry.",
            error_schema_reference(),
        ),
    );
    responses
}

fn nexus_public_lane_validators_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Nexus".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List validators registered for a public lane.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns validators servicing the requested public lane with lifecycle status \
             (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation epoch/height, \
             and `release_at_ms` when an exit timer is running. Account literals honour the optional \
             `address_format` hint (`ih58` or `compressed`)."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("nexusPublicLaneValidators".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![
            Value::Object(lane_id_parameter()),
            address_format_query_param(),
        ]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(nexus_public_lane_validators_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn nexus_public_lane_validators_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Public lane validators retrieved.",
            schema_ref("PublicLaneValidatorListResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response("Invalid lane id.", error_schema_reference()),
    );
    responses
}

fn nexus_public_lane_stake_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Nexus".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List bonded stake shares for a public lane.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns bonded stake per validator/staker pair for the requested public lane, \
             including pending unbond timers. Supports an optional validator filter and renders \
             account literals according to the `address_format` hint (`ih58` or `compressed`)."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("nexusPublicLaneStake".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![
            Value::Object(lane_id_parameter()),
            address_format_query_param(),
            string_query_param(
                "validator",
                "Optional validator account literal to filter stake entries (IH58 (preferred)/sora (second-best)).",
            ),
        ]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(nexus_public_lane_stake_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn nexus_public_lane_stake_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Public lane stake entries retrieved.",
            schema_ref("PublicLaneStakeListResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response(
            "Invalid lane id or validator filter.",
            error_schema_reference(),
        ),
    );
    responses
}

fn nexus_public_lane_rewards_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Nexus".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List pending rewards for a public lane account.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the unclaimed reward amount per asset for the requested account in the \
             given public lane. Requires an `account` query parameter (IH58 (preferred)/sora (second-best)) and \
             accepts an optional `upto_epoch` upper bound."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("nexusPublicLaneRewards".to_owned()),
    );
    let mut upto_epoch_param = Map::new();
    upto_epoch_param.insert("name".into(), Value::String("upto_epoch".to_owned()));
    upto_epoch_param.insert("in".into(), Value::String("query".to_owned()));
    upto_epoch_param.insert("required".into(), Value::Bool(false));
    upto_epoch_param.insert(
        "description".into(),
        Value::String("Upper bound (inclusive) for reward epochs to include.".to_owned()),
    );
    let mut upto_schema = Map::new();
    upto_schema.insert("type".into(), Value::String("integer".to_owned()));
    upto_schema.insert("format".into(), Value::String("int64".to_owned()));
    upto_epoch_param.insert("schema".into(), Value::Object(upto_schema));
    operation.insert(
        "parameters".into(),
        Value::Array(vec![
            Value::Object(lane_id_parameter()),
            address_format_query_param(),
            string_query_param(
                "account",
                "Account literal to evaluate pending rewards for (IH58 (preferred)/sora (second-best)).",
            ),
            Value::Object(upto_epoch_param),
        ]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(nexus_public_lane_rewards_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn nexus_public_lane_rewards_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Pending rewards retrieved.",
            schema_ref("PublicLanePendingRewardListResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response(
            "Invalid lane id or account literal.",
            error_schema_reference(),
        ),
    );
    responses
}

fn kaigi_relay_detail_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Kaigi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch metadata and metrics for a specific Kaigi relay.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns relay registration metadata, base64 HPKE key material, \
             latest health report context, and aggregated per-domain counters."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("kaigiRelayDetail".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![Value::Object(kaigi_relay_id_parameter())]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(kaigi_relay_detail_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn kaigi_relay_id_parameter() -> Map {
    let mut param = Map::new();
    param.insert("name".into(), Value::String("relay_id".to_owned()));
    param.insert("in".into(), Value::String("path".to_owned()));
    param.insert("required".into(), Value::Bool(true));
    param.insert(
        "description".into(),
        Value::String("Relay account identifier (e.g., `relay@kaigi`).".to_owned()),
    );
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    param
}

fn kaigi_relay_detail_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response("Relay detail retrieved.", schema_ref("KaigiRelayDetail")),
    );
    responses.insert(
        "400".to_owned(),
        json_response("Invalid relay identifier.", error_schema_reference()),
    );
    responses.insert(
        "404".to_owned(),
        json_response("Relay not found.", error_schema_reference()),
    );
    responses.insert(
        "503".to_owned(),
        json_response(
            "Telemetry profile does not permit relay telemetry.",
            error_schema_reference(),
        ),
    );
    responses
}

fn kaigi_relays_health_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Kaigi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Aggregate Kaigi relay health counters across the network.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns totals for healthy/degraded/unavailable relays along with \
             registrations, failovers, and per-domain counters."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("kaigiRelayHealth".to_owned()),
    );
    operation.insert(
        "responses".into(),
        Value::Object(kaigi_relays_health_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn kaigi_relays_health_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Relay health snapshot retrieved.",
            schema_ref("KaigiRelayHealthSnapshot"),
        ),
    );
    responses.insert(
        "503".to_owned(),
        json_response(
            "Telemetry profile does not permit relay telemetry.",
            error_schema_reference(),
        ),
    );
    responses
}

fn kaigi_relays_events_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Kaigi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Subscribe to Kaigi relay telemetry events (SSE).".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Streams relay registration and health events as Server-Sent Events. \
             Optional query parameters filter by domain, relay, or event kind."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("kaigiRelayEvents".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(kaigi_events_query_parameters()),
    );
    operation.insert(
        "responses".into(),
        Value::Object(kaigi_relays_events_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn kaigi_events_query_parameters() -> Vec<Value> {
    let mut params = Vec::new();

    for (name, description) in [
        ("domain", "Optional domain filter (case-insensitive)."),
        ("relay", "Optional relay identifier filter."),
        (
            "kind",
            "Optional comma-separated list of event kinds (`registration`, `health`).",
        ),
    ] {
        let mut param = Map::new();
        param.insert("name".into(), Value::String(name.to_owned()));
        param.insert("in".into(), Value::String("query".to_owned()));
        param.insert("required".into(), Value::Bool(false));
        param.insert("description".into(), Value::String(description.to_owned()));
        let mut schema = Map::new();
        schema.insert("type".into(), Value::String("string".to_owned()));
        param.insert("schema".into(), Value::Object(schema));
        params.push(Value::Object(param));
    }

    params
}

fn kaigi_relays_events_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        event_stream_response("Relay telemetry events stream."),
    );
    responses.insert(
        "503".to_owned(),
        json_response(
            "Telemetry profile does not permit relay telemetry.",
            error_schema_reference(),
        ),
    );
    responses
}

fn paths_section() -> Map {
    let mut paths = alias_paths();
    paths.extend(time_paths());
    paths.extend(ledger_paths());
    paths.extend(da_paths());
    paths.extend(offline_paths());
    paths.extend(system_paths());
    paths.extend(operator_auth_paths());
    paths.extend(transaction_paths());
    paths.extend(query_paths());
    paths.extend(stream_paths());
    paths.extend(connect_paths());
    paths.extend(proof_paths());
    paths.extend(contracts_paths());
    paths.extend(zk_paths());
    paths.extend(governance_paths());
    paths.extend(runtime_paths());
    paths.extend(account_paths());
    paths.extend(domain_paths());
    paths.extend(asset_paths());
    paths.extend(nft_paths());
    paths.extend(subscription_paths());
    paths.extend(parameter_paths());
    paths.extend(space_directory_paths());
    paths.extend(explorer_paths());
    paths.extend(sorafs_paths());
    paths.extend(soradns_paths());
    paths.extend(content_paths());
    paths.extend(sns_paths());
    paths.extend(soranet_paths());
    paths.extend(push_paths());
    paths.extend(webhook_paths());
    paths.extend(kaigi_paths());
    paths.extend(nexus_paths());
    paths.extend(sumeragi_paths());
    paths.extend(repo_paths());
    paths
}

fn alias_voprf_evaluate_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Aliases".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Evaluate the mock alias VOPRF helper.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Takes a blinded alias element (hex) and returns the deterministically \
             evaluated mock response used by tooling."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("aliasVoprfEvaluate".to_owned()),
    );
    operation.insert("requestBody".into(), alias_voprf_request_body());
    operation.insert("responses".into(), Value::Object(alias_voprf_responses()));
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn alias_voprf_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert("schema".into(), schema_ref("AliasVoprfEvaluateRequest"));
            schema
        }),
    );

    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn alias_voprf_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Blinded element successfully evaluated.",
            schema_ref("AliasVoprfEvaluateResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response(
            "Malformed request (invalid hex encoding).",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "500".to_owned(),
        json_response("Internal server error.", error_schema_reference()),
    );
    responses
}

fn alias_resolve_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Aliases".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Resolve an alias string into an account and index.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the canonical alias spelling, the bound account identifier, and \
             (if known) the deterministic alias index."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("aliasResolve".to_owned()),
    );
    operation.insert("requestBody".into(), alias_resolve_request_body());
    operation.insert("responses".into(), Value::Object(alias_resolve_responses()));
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn alias_resolve_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert("schema".into(), schema_ref("AliasResolveRequest"));
            schema
        }),
    );

    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn alias_resolve_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Alias successfully resolved.",
            schema_ref("AliasResolveResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response(
            "Malformed request (alias missing or invalid).",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response("Alias not found.", error_schema_reference()),
    );
    responses.insert(
        "503".to_owned(),
        json_response("Alias runtime unavailable.", error_schema_reference()),
    );
    responses
}

fn alias_resolve_index_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Aliases".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Resolve an alias entry by deterministic index.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the alias string and account identifier registered under the \
             provided index."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("aliasResolveIndex".to_owned()),
    );
    operation.insert("requestBody".into(), alias_resolve_index_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(alias_resolve_index_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn alias_resolve_index_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert("schema".into(), schema_ref("AliasResolveIndexRequest"));
            schema
        }),
    );

    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn alias_resolve_index_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Alias index successfully resolved.",
            schema_ref("AliasResolveIndexResponse"),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response("Alias index not found.", error_schema_reference()),
    );
    responses.insert(
        "503".to_owned(),
        json_response("Alias runtime unavailable.", error_schema_reference()),
    );
    responses
}

fn time_now_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Time".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch the current network time snapshot.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the best-effort network time with offset/confidence relative to UTC."
                .to_owned(),
        ),
    );
    operation.insert("operationId".into(), Value::String("timeNow".to_owned()));
    operation.insert("responses".into(), Value::Object(time_now_responses()));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn time_now_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Network time snapshot retrieved.",
            schema_ref("TimeNowResponse"),
        ),
    );
    responses
}

fn time_status_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Time".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Inspect network time diagnostics.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String("Provides peer sampling metadata, RTT histogram buckets, and running totals gathered by the network time service.".to_owned()),
    );
    operation.insert("operationId".into(), Value::String("timeStatus".to_owned()));
    operation.insert("responses".into(), Value::Object(time_status_responses()));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn time_status_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Network time diagnostics collected.",
            schema_ref("TimeStatusResponse"),
        ),
    );
    responses
}

fn history_window_params() -> Vec<Value> {
    vec![
        query_param(
            "from",
            "integer",
            "Optional starting block height (inclusive, newest-first window).",
        ),
        query_param(
            "limit",
            "integer",
            "Optional maximum number of records to return (bounded by server cap).",
        ),
    ]
}

fn ledger_headers_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Ledger".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch recent block headers (newest first).".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns a newest-first window of block headers with optional `from`/`limit` query parameters and Norito/JSON negotiation via `Accept`."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("ledgerHeaders".to_owned()),
    );
    operation.insert("parameters".into(), Value::Array(history_window_params()));
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response("#/components/schemas/BlockHeaderList")),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn ledger_state_root_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Ledger".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch execution state root for a block height.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the execution post-state root recorded for the specified block height. Supports JSON or Norito content negotiation via the `Accept` header."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("ledgerStateRoot".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![Value::Object(block_height_parameter())]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(
            "#/components/schemas/StateRootResponse",
        )),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn ledger_state_proof_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Ledger".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch execution state proof (QC) for a block height.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the execution Quorum Certificate (QC) attesting the post-state root for the specified block height, suitable for light clients."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("ledgerStateProof".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![Value::Object(block_height_parameter())]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(single_json_response(
            "#/components/schemas/StateProofResponse",
        )),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn ledger_block_proof_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Ledger".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch Merkle proofs for a block entrypoint.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the inclusion and execution Merkle proofs for the specified transaction entrypoint within a block."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("ledgerBlockProof".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![
            Value::Object(block_height_parameter()),
            Value::Object(entry_hash_parameter()),
        ]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(ledger_block_proof_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn block_height_parameter() -> Map {
    let mut param = Map::new();
    param.insert("name".into(), Value::String("height".to_owned()));
    param.insert("in".into(), Value::String("path".to_owned()));
    param.insert("required".into(), Value::Bool(true));
    param.insert(
        "description".into(),
        Value::String("Block height to inspect (1-indexed).".to_owned()),
    );
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("integer".to_owned()));
    schema.insert("format".into(), Value::String("uint64".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    param
}

fn entry_hash_parameter() -> Map {
    let mut param = Map::new();
    param.insert("name".into(), Value::String("entry_hash".to_owned()));
    param.insert("in".into(), Value::String("path".to_owned()));
    param.insert("required".into(), Value::Bool(true));
    param.insert(
        "description".into(),
        Value::String("Transaction entrypoint hash encoded as lowercase hexadecimal.".to_owned()),
    );
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    param
}

fn da_commitments_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("DataAvailability".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List DA commitments indexed by the node.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns DA commitments captured from recent blocks. Results can be filtered by manifest hash or lane/epoch/sequence and paginated."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("daCommitmentsList".to_owned()),
    );
    operation.insert("requestBody".into(), da_commitment_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(da_commitments_list_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn da_commitments_prove_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("DataAvailability".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch a DA commitment proof placeholder.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Locates a DA commitment by manifest or lane/epoch/sequence and returns its indexed location. Inclusion proofs will be added once Merkle wiring lands."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("daCommitmentsProve".to_owned()),
    );
    operation.insert("requestBody".into(), da_commitment_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(da_commitments_prove_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn da_commitments_verify_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("DataAvailability".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Stateless verification helper for DA commitments.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Validates a provided DA commitment payload against the node's in-memory index. This is a placeholder until Merkle proofs are available."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("daCommitmentsVerify".to_owned()),
    );
    operation.insert(
        "requestBody".into(),
        da_commitment_request_body_with_schema("DaCommitmentWithLocation"),
    );
    operation.insert(
        "responses".into(),
        Value::Object(da_commitments_verify_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn da_commitment_request_body() -> Value {
    da_commitment_request_body_with_schema("DaCommitmentProofRequest")
}

fn da_commitment_request_body_with_schema(schema: &str) -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema_map = Map::new();
            schema_map.insert("schema".into(), schema_ref(schema));
            schema_map
        }),
    );

    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn da_commitments_list_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Commitments retrieved.",
            schema_ref("DaCommitmentWithLocationList"),
        ),
    );
    responses.insert(
        "500".to_owned(),
        json_response("Internal server error.", error_schema_reference()),
    );
    responses
}

fn da_commitments_prove_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Commitment proof response (null when not found).",
            schema_ref("DaCommitmentProofResponse"),
        ),
    );
    responses.insert(
        "500".to_owned(),
        json_response("Internal server error.", error_schema_reference()),
    );
    responses
}

fn da_commitments_verify_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Verification result.",
            schema_ref("DaCommitmentVerifyResponse"),
        ),
    );
    responses.insert(
        "500".to_owned(),
        json_response("Internal server error.", error_schema_reference()),
    );
    responses
}

fn lane_id_parameter() -> Map {
    let mut param = Map::new();
    param.insert("name".into(), Value::String("lane_id".to_owned()));
    param.insert("in".into(), Value::String("path".to_owned()));
    param.insert("required".into(), Value::Bool(true));
    param.insert(
        "description".into(),
        Value::String("Nexus public lane identifier.".to_owned()),
    );
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("integer".to_owned()));
    schema.insert("format".into(), Value::String("uint64".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    param
}

fn string_path_param(name: &str, description: &str) -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String(name.to_owned()));
    param.insert("in".into(), Value::String("path".to_owned()));
    param.insert("required".into(), Value::Bool(true));
    param.insert("description".into(), Value::String(description.to_owned()));
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("string".to_owned()));
    param.insert("schema".into(), Value::Object(schema));
    Value::Object(param)
}

fn integer_path_param(name: &str, description: &str, format: Option<&str>) -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String(name.to_owned()));
    param.insert("in".into(), Value::String("path".to_owned()));
    param.insert("required".into(), Value::Bool(true));
    param.insert("description".into(), Value::String(description.to_owned()));
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("integer".to_owned()));
    if let Some(fmt) = format {
        schema.insert("format".into(), Value::String(fmt.to_owned()));
    }
    param.insert("schema".into(), Value::Object(schema));
    Value::Object(param)
}

fn ledger_block_proof_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Merkle proofs for the requested entrypoint.",
            schema_ref("BlockProofs"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response(
            "Invalid block height or entry hash supplied.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response("Block or entrypoint not found.", error_schema_reference()),
    );
    responses
}

fn health_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("System".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Check Torii liveness.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String("Returns `Healthy` when the node is accepting HTTP requests.".to_owned()),
    );
    operation.insert(
        "operationId".into(),
        Value::String("healthCheck".to_owned()),
    );
    operation.insert("responses".into(), Value::Object(health_responses()));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn operator_auth_registration_options_operation() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "WebAuthn registration options for operator enrollment.",
            schema_ref("OperatorWebAuthnOptionsResponse"),
        ),
    );
    responses.insert(
        "401".to_owned(),
        json_response(
            "Authentication required or invalid.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "403".to_owned(),
        json_response(
            "Operator authentication disabled or mTLS required.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "429".to_owned(),
        json_response(
            "Operator auth rate limited or locked out.",
            error_schema_reference(),
        ),
    );

    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("OperatorAuth".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Start operator WebAuthn registration.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Issues WebAuthn registration options. Requires operator bootstrap auth.".to_string(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("operatorAuthRegistrationOptions".to_owned()),
    );
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn operator_auth_registration_verify_operation() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Operator WebAuthn credential enrolled.",
            schema_ref("OperatorWebAuthnRegistrationResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response("Invalid WebAuthn payload.", error_schema_reference()),
    );
    responses.insert(
        "401".to_owned(),
        json_response(
            "Authentication required or invalid.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "403".to_owned(),
        json_response(
            "Operator authentication disabled or mTLS required.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "429".to_owned(),
        json_response(
            "Operator auth rate limited or locked out.",
            error_schema_reference(),
        ),
    );

    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("OperatorAuth".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Finish operator WebAuthn registration.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String("Verifies a WebAuthn attestation and stores the credential.".to_owned()),
    );
    operation.insert(
        "operationId".into(),
        Value::String("operatorAuthRegistrationVerify".to_owned()),
    );
    operation.insert(
        "requestBody".into(),
        Value::Object(json_request_body("#/components/schemas/JsonValue")),
    );
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn operator_auth_login_options_operation() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "WebAuthn authentication options for operator login.",
            schema_ref("OperatorWebAuthnOptionsResponse"),
        ),
    );
    responses.insert(
        "401".to_owned(),
        json_response(
            "Authentication required or invalid.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "403".to_owned(),
        json_response(
            "Operator authentication disabled or mTLS required.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "409".to_owned(),
        json_response(
            "No operator credentials enrolled.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "429".to_owned(),
        json_response(
            "Operator auth rate limited or locked out.",
            error_schema_reference(),
        ),
    );

    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("OperatorAuth".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Start operator WebAuthn login.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String("Issues WebAuthn authentication options.".to_owned()),
    );
    operation.insert(
        "operationId".into(),
        Value::String("operatorAuthLoginOptions".to_owned()),
    );
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn operator_auth_login_verify_operation() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Operator session token issued.",
            schema_ref("OperatorWebAuthnLoginResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response("Invalid WebAuthn payload.", error_schema_reference()),
    );
    responses.insert(
        "401".to_owned(),
        json_response(
            "Authentication required or invalid.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "403".to_owned(),
        json_response(
            "Operator authentication disabled or mTLS required.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "429".to_owned(),
        json_response(
            "Operator auth rate limited or locked out.",
            error_schema_reference(),
        ),
    );

    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("OperatorAuth".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Finish operator WebAuthn login.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String("Verifies a WebAuthn assertion and issues a session token.".to_owned()),
    );
    operation.insert(
        "operationId".into(),
        Value::String("operatorAuthLoginVerify".to_owned()),
    );
    operation.insert(
        "requestBody".into(),
        Value::Object(json_request_body("#/components/schemas/JsonValue")),
    );
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn health_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        plain_text_response("Torii node is reachable.", Some("Healthy")),
    );
    responses
}

fn openapi_schemas() -> Map {
    let mut schemas = Map::new();
    schemas.insert(
        "JsonValue".to_owned(),
        norito::json!({
            "description": "Arbitrary JSON payload.",
            "type": ["object", "array", "string", "number", "boolean", "null"],
            "additionalProperties": true
        }),
    );
    schemas.insert(
        "JsonList".to_owned(),
        norito::json!({
            "type": "array",
            "items": { "$ref": "#/components/schemas/JsonValue" }
        }),
    );
    schemas.insert(
        "ApiVersionInfo".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["default", "supported", "min_proof_version"],
            "additionalProperties": false,
            "properties": {
                "default": { "type": "string", "description": "Default API version label." },
                "supported": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "All supported API version labels."
                },
                "sunset_unix": {
                    "type": ["integer", "null"],
                    "format": "uint64",
                    "description": "Optional unix timestamp when the oldest API version sunsets."
                },
                "min_proof_version": {
                    "type": "string",
                    "description": "Minimum API version required for proof/staking/fee endpoints."
                }
            }
        }),
    );
    schemas.insert(
        "PeerIdList".to_owned(),
        norito::json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Peer identifiers (public key and address encoding)."
        }),
    );
    schemas.insert(
        "PushRegisterDeviceRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_id", "platform", "token"],
            "additionalProperties": false,
            "properties": {
                "account_id": { "type": "string", "description": "Account identifier registering the device." },
                "platform": { "type": "string", "description": "Push platform label (FCM, APNS)." },
                "token": { "type": "string", "description": "Device token." },
                "topics": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional topics to subscribe the device to."
                }
            }
        }),
    );
    schemas.insert(
        "AliasVoprfEvaluateRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["blinded_element_hex"],
            "additionalProperties": false,
            "properties": {
                "blinded_element_hex": {
                    "type": "string",
                    "description": "Hex-encoded blinded element to evaluate."
                }
            }
        }),
    );
    schemas.insert(
        "AliasVoprfEvaluateResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["evaluated_element_hex", "backend"],
            "additionalProperties": false,
            "properties": {
                "evaluated_element_hex": {
                    "type": "string",
                    "description": "Hex-encoded evaluated element."
                },
                "backend": {
                    "type": "string",
                    "enum": ["blake2b512-mock"],
                    "description": "Name of the evaluator implementation."
                }
            }
        }),
    );
    schemas.insert(
        "AliasResolveRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias"],
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Alias string to resolve."
                }
            }
        }),
    );
    schemas.insert(
        "AliasResolveResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias", "account_id"],
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Canonical alias representation."
                },
                "account_id": {
                    "type": "string",
                    "description": "Account identifier bound to the alias."
                },
                "index": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Deterministic alias index if available."
                },
                "source": {
                    "type": "string",
                    "description": "Backend source for the alias mapping."
                }
            }
        }),
    );
    schemas.insert(
        "AliasResolveIndexRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["index"],
            "additionalProperties": false,
            "properties": {
                "index": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Deterministic alias index to look up."
                }
            }
        }),
    );
    schemas.insert(
        "AliasResolveIndexResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias", "account_id", "index"],
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Canonical alias representation."
                },
                "account_id": {
                    "type": "string",
                    "description": "Account identifier bound to the alias."
                },
                "index": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Deterministic alias index."
                },
                "source": {
                    "type": "string",
                    "description": "Backend source for the alias mapping."
                }
            }
        }),
    );
    schemas.insert(
        "AccountResolveRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["literal"],
            "additionalProperties": false,
            "properties": {
                "literal": {
                    "type": "string",
                    "description": "Account literal to resolve."
                }
            }
        }),
    );
    schemas.insert(
        "AccountResolveResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_id", "domain", "source"],
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Canonical IH58 account identifier."
                },
                "domain": {
                    "type": "string",
                    "description": "Resolved domain label."
                },
                "source": {
                    "type": "string",
                    "description": "Resolution source (encoded, alias, public_key, uaid, opaque)."
                },
                "format": {
                    "type": "string",
                    "enum": ["ih58", "compressed", "canonical_hex"],
                    "description": "Address format when source is encoded."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineAllowanceItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "certificate_id_hex",
                "controller_id",
                "controller_display",
                "asset_id",
                "registered_at_ms",
                "expires_at_ms",
                "policy_expires_at_ms",
                "remaining_amount",
                "record"
            ],
            "properties": {
                "certificate_id_hex": {
                    "type": "string",
                    "description": "Deterministic certificate hash rendered as lowercase hex."
                },
                "controller_id": {
                    "type": "string",
                    "description": "Canonical controller account id."
                },
                "controller_display": {
                    "type": "string",
                    "description": "Controller rendered according to the address_format hint."
                },
                "asset_id": {
                    "type": "string",
                    "description": "Asset id bound to the allowance."
                },
                "registered_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Unix timestamp (ms) when the allowance was registered."
                },
                "expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Certificate expiry timestamp (ms)."
                },
                "policy_expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Policy expiry timestamp (ms) enforced by the issuer."
                },
                "refresh_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Next attestation refresh deadline (ms), if provided.",
                    "nullable": true
                },
                "verdict_id_hex": {
                    "type": "string",
                    "description": "Cached attestation verdict identifier rendered as hex.",
                    "nullable": true
                },
                "attestation_nonce_hex": {
                    "type": "string",
                    "description": "Nonce bound to the cached verdict rendered as hex.",
                    "nullable": true
                },
                "remaining_amount": {
                    "type": "string",
                    "description": "Outstanding allowance amount that has not been reconciled."
                },
                "deadline_kind": {
                    "type": "string",
                    "description": "Deadline currently tracked for countdown warnings (`refresh`, `policy`, or `certificate`).",
                    "nullable": true
                },
                "deadline_state": {
                    "type": "string",
                    "description": "Countdown classification (`ok`, `warning`, or `expired`).",
                    "nullable": true
                },
                "deadline_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Unix millisecond timestamp for the selected deadline.",
                    "nullable": true
                },
                "deadline_ms_remaining": {
                    "type": "integer",
                    "format": "int64",
                    "description": "Signed milliseconds remaining until the deadline (negative when already expired).",
                    "nullable": true
                },
                "record": {
                    "type": "object",
                    "description": "Full OfflineAllowanceRecord JSON payload."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineAllowanceListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["items", "total"],
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/OfflineAllowanceItem"
                    }
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total number of allowances matching the filter."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineAllowanceIssueRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["authority", "private_key", "certificate"],
            "properties": {
                "authority": {
                    "type": "string",
                    "description": "Account authorizing the registration transaction."
                },
                "private_key": {
                    "type": "object",
                    "description": "Exposed private key used to sign the transaction."
                },
                "certificate": {
                    "type": "object",
                    "description": "OfflineWalletCertificate payload to register."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineAllowanceIssueResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["certificate_id_hex"],
            "properties": {
                "certificate_id_hex": {
                    "type": "string",
                    "description": "Deterministic certificate id rendered as hex."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineWalletCertificateDraft".to_owned(),
        norito::json!({
            "type": "object",
            "description": "OfflineWalletCertificate payload without the operator signature."
        }),
    );
    schemas.insert(
        "OfflineCertificateIssueRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["certificate"],
            "properties": {
                "certificate": {
                    "$ref": "#/components/schemas/OfflineWalletCertificateDraft"
                }
            }
        }),
    );
    schemas.insert(
        "OfflineCertificateIssueResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["certificate_id_hex", "certificate"],
            "properties": {
                "certificate_id_hex": {
                    "type": "string",
                    "description": "Deterministic certificate id rendered as hex."
                },
                "certificate": {
                    "type": "object",
                    "description": "Operator-signed OfflineWalletCertificate payload."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineSettlementSubmitRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["authority", "private_key", "transfer"],
            "properties": {
                "authority": {
                    "type": "string",
                    "description": "Account authorizing the settlement transaction."
                },
                "private_key": {
                    "type": "object",
                    "description": "Exposed private key used to sign the transaction."
                },
                "transfer": {
                    "type": "object",
                    "description": "OfflineToOnlineTransfer bundle to settle."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineSettlementSubmitResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["bundle_id_hex"],
            "properties": {
                "bundle_id_hex": {
                    "type": "string",
                    "description": "Deterministic bundle identifier rendered as hex."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineSpendReceiptsSubmitRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["receipts"],
            "properties": {
                "receipts": {
                    "type": "array",
                    "description": "Ordered receipts to validate.",
                    "items": { "type": "object" }
                }
            }
        }),
    );
    schemas.insert(
        "OfflineSpendReceiptsSubmitResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["receipts_root_hex", "receipt_count", "total_amount"],
            "properties": {
                "receipts_root_hex": {
                    "type": "string",
                    "description": "Poseidon receipts root rendered as hex."
                },
                "receipt_count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of receipts processed."
                },
                "total_amount": {
                    "type": "string",
                    "description": "Sum of receipt amounts."
                },
                "asset_id": {
                    "type": "string",
                    "nullable": true,
                    "description": "Asset identifier derived from the receipts."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineBundleProofSummary".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["version", "metadata_keys"],
            "properties": {
                "version": {
                    "type": "integer",
                    "format": "uint16",
                    "description": "Aggregate proof envelope version."
                },
                "proof_sum_bytes": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Sum proof size in bytes, when present."
                },
                "proof_counter_bytes": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Counter proof size in bytes, when present."
                },
                "proof_replay_bytes": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Replay proof size in bytes, when present."
                },
                "metadata_keys": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Sorted metadata keys present in the envelope."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineBundleProofStatusResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["bundle_id_hex", "receipts_root_hex", "proof_status"],
            "properties": {
                "bundle_id_hex": {
                    "type": "string",
                    "description": "Deterministic bundle identifier rendered as hex."
                },
                "receipts_root_hex": {
                    "type": "string",
                    "description": "Poseidon receipts root computed from stored receipts."
                },
                "aggregate_proof_root_hex": {
                    "type": "string",
                    "nullable": true,
                    "description": "Receipts root advertised by the aggregate proof envelope when present."
                },
                "receipts_root_matches": {
                    "type": "boolean",
                    "nullable": true,
                    "description": "Whether the computed receipts root matches the advertised root."
                },
                "proof_status": {
                    "type": "string",
                    "description": "Proof status label (`missing`, `match`, or `mismatch`)."
                },
                "proof_summary": {
                    "allOf": [
                        { "$ref": "#/components/schemas/OfflineBundleProofSummary" }
                    ],
                    "nullable": true,
                    "description": "Optional summary of the aggregate proof payload."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineStateResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["allowances", "transfers", "summaries", "revocations", "now_ms"],
            "properties": {
                "allowances": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "Registered OfflineAllowanceRecord entries."
                },
                "transfers": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "OfflineTransferRecord entries."
                },
                "summaries": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "OfflineCounterSummary entries."
                },
                "revocations": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "OfflineVerdictRevocation entries."
                },
                "now_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Snapshot timestamp (ms)."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineCertificateRenewRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["authority", "private_key", "certificate"],
            "properties": {
                "authority": {
                    "type": "string",
                    "description": "Account authorizing the renewal transaction."
                },
                "private_key": {
                    "type": "object",
                    "description": "Exposed private key used to sign the transaction."
                },
                "certificate": {
                    "type": "object",
                    "description": "New OfflineWalletCertificate payload to register."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineCertificateRenewResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["certificate_id_hex"],
            "properties": {
                "certificate_id_hex": {
                    "type": "string",
                    "description": "Deterministic renewed certificate id rendered as hex."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineCertificateRevokeRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["authority", "private_key", "certificate_id_hex"],
            "properties": {
                "authority": {
                    "type": "string",
                    "description": "Account authorizing the revocation transaction."
                },
                "private_key": {
                    "type": "object",
                    "description": "Exposed private key used to sign the transaction."
                },
                "certificate_id_hex": {
                    "type": "string",
                    "description": "Certificate identifier to revoke (hex, case-insensitive)."
                },
                "reason": {
                    "type": "string",
                    "description": "Revocation reason slug."
                },
                "note": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional human note describing the revocation."
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata attached to the revocation record."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineCertificateRevokeResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["verdict_id_hex"],
            "properties": {
                "verdict_id_hex": {
                    "type": "string",
                    "description": "Deterministic verdict identifier rendered as hex."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineReceiptListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "bundle_id_hex",
                "tx_id_hex",
                "certificate_id_hex",
                "controller_id",
                "controller_display",
                "receiver_id",
                "receiver_display",
                "asset_id",
                "amount",
                "invoice_id",
                "counter",
                "recorded_at_ms",
                "recorded_at_height"
            ],
            "properties": {
                "bundle_id_hex": { "type": "string" },
                "tx_id_hex": { "type": "string" },
                "certificate_id_hex": { "type": "string" },
                "controller_id": { "type": "string" },
                "controller_display": { "type": "string" },
                "receiver_id": { "type": "string" },
                "receiver_display": { "type": "string" },
                "asset_id": { "type": "string" },
                "amount": {
                    "type": "string",
                    "description": "Receipt amount."
                },
                "invoice_id": { "type": "string" },
                "counter": {
                    "type": "integer",
                    "format": "uint64"
                },
                "recorded_at_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "recorded_at_height": {
                    "type": "integer",
                    "format": "uint64"
                }
            }
        }),
    );
    schemas.insert(
        "OfflineReceiptListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["items", "total"],
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/OfflineReceiptListItem"
                    }
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total number of receipts matching the filter."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineSummaryItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "certificate_id_hex",
                "controller_id",
                "controller_display",
                "summary_hash_hex",
                "apple_key_counters",
                "android_series_counters"
            ],
            "properties": {
                "certificate_id_hex": {
                    "type": "string",
                    "description": "Deterministic certificate hash rendered as lowercase hex."
                },
                "controller_id": {
                    "type": "string",
                    "description": "Controller account id bound to the certificate."
                },
                "controller_display": {
                    "type": "string",
                    "description": "Controller rendered according to the address_format hint."
                },
                "summary_hash_hex": {
                    "type": "string",
                    "description": "BLAKE2b-256 hash of the counter maps rendered as hex."
                },
                "apple_key_counters": {
                    "type": "object",
                    "description": "Per-App Attest key counter checkpoints.",
                    "additionalProperties": {
                        "type": "integer",
                        "format": "uint64"
                    }
                },
                "android_series_counters": {
                    "type": "object",
                    "description": "Per-Android marker series counter checkpoints.",
                    "additionalProperties": {
                        "type": "integer",
                        "format": "uint64"
                    }
                }
            }
        }),
    );
    schemas.insert(
        "OfflineSummaryListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["items", "total"],
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/OfflineSummaryItem"
                    }
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total number of summaries matching the filter."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineTransferItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "bundle_id_hex",
                "controller_id",
                "controller_display",
                "receiver_id",
                "receiver_display",
                "deposit_account_id",
                "deposit_account_display",
                "receipt_count",
                "total_amount",
                "status",
                "recorded_at_ms",
                "recorded_at_height",
                "claimed_delta",
                "transfer"
            ],
            "properties": {
                "bundle_id_hex": {
                    "type": "string",
                    "description": "Bundle identifier rendered as lowercase hex."
                },
                "controller_id": {
                    "type": "string",
                    "description": "Controller account id associated with the originating allowance."
                },
                "controller_display": {
                    "type": "string",
                    "description": "Controller rendered with the requested address format."
                },
                "receiver_id": {
                    "type": "string",
                    "description": "Offline receiver account id."
                },
                "receiver_display": {
                    "type": "string",
                    "description": "Receiver rendered with requested address format."
                },
                "deposit_account_id": {
                    "type": "string",
                    "description": "Online account that will receive the deposit."
                },
                "deposit_account_display": {
                    "type": "string",
                    "description": "Deposit account rendered with requested address format."
                },
                "asset_id": {
                    "type": "string",
                    "description": "Asset id inferred from receipts, if present."
                },
                "receipt_count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of receipts included in the bundle."
                },
                "total_amount": {
                    "type": "string",
                    "description": "Human-readable total amount aggregated from receipts."
                },
                "status": {
                    "type": "string",
                    "description": "Lifecycle status enforced for the bundle."
                },
                "recorded_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Unix timestamp (ms) when the bundle settled on-ledger."
                },
                "recorded_at_height": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Block height that recorded the bundle."
                },
                "archived_at_height": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Optional block height when the bundle was archived.",
                    "nullable": true
                },
                "certificate_id_hex": {
                    "type": "string",
                    "description": "Canonical sender certificate identifier (hex).",
                    "nullable": true
                },
                "certificate_expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Certificate expiry timestamp captured at settlement.",
                    "nullable": true
                },
                "policy_expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Policy expiry timestamp captured at settlement.",
                    "nullable": true
                },
                "refresh_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Attestation refresh deadline captured at settlement.",
                    "nullable": true
                },
                "verdict_id_hex": {
                    "type": "string",
                    "description": "Cached attestation verdict identifier.",
                    "nullable": true
                },
                "attestation_nonce_hex": {
                    "type": "string",
                    "description": "Cached attestation nonce captured at settlement.",
                    "nullable": true
                },
                "platform_policy": {
                    "type": "string",
                    "description": "Platform attestation policy (e.g., play_integrity).",
                    "nullable": true
                },
                "platform_token_snapshot": {
                    "$ref": "#/components/schemas/OfflinePlatformTokenSnapshot",
                    "nullable": true,
                    "description": "Snapshot of the attestation token recorded during settlement."
                },
                "verdict_snapshot": {
                    "$ref": "#/components/schemas/OfflineVerdictSnapshot",
                    "nullable": true,
                    "description": "Snapshot of certificate/verdict metadata captured at settlement."
                },
                "status_transitions": {
                    "type": "array",
                    "description": "Ordered lifecycle history for the bundle.",
                    "items": {
                        "$ref": "#/components/schemas/OfflineTransferStatusTransition"
                    }
                },
                "claimed_delta": {
                    "type": "string",
                    "description": "Claimed delta reported by the balance proof."
                },
                "transfer": {
                    "type": "object",
                    "description": "Full OfflineToOnlineTransfer payload serialized as JSON."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineVerdictSnapshot".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "certificate_id",
                "certificate_expires_at_ms",
                "policy_expires_at_ms"
            ],
            "properties": {
                "certificate_id": {
                    "type": "string",
                    "description": "Deterministic certificate identifier backing the bundle (`hash:...` literal)."
                },
                "verdict_id": {
                    "type": "string",
                    "nullable": true,
                    "description": "Cached attestation verdict identifier literal."
                },
                "attestation_nonce": {
                    "type": "string",
                    "nullable": true,
                    "description": "Cached attestation nonce literal."
                },
                "refresh_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Timestamp when the attestation must be refreshed."
                },
                "certificate_expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Certificate expiry timestamp captured at settlement."
                },
                "policy_expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Policy expiry timestamp captured at settlement."
                }
            }
        }),
    );
    schemas.insert(
        "OfflinePlatformTokenSnapshot".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["policy", "attestation_jws_b64"],
            "properties": {
                "policy": {
                    "type": "string",
                    "description": "Attestation policy slug (e.g., play_integrity)."
                },
                "attestation_jws_b64": {
                    "type": "string",
                    "description": "Base64-encoded JWS payload captured at settlement."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineTransferStatusTransition".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["status", "transitioned_at_ms"],
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Status that became active during this transition."
                },
                "transitioned_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Timestamp when the transition occurred."
                },
                "verdict_snapshot": {
                    "$ref": "#/components/schemas/OfflineVerdictSnapshot",
                    "nullable": true,
                    "description": "Optional verdict metadata snapshot captured at this transition."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineTransferListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["items", "total"],
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/OfflineTransferItem"
                    }
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total number of transfer bundles matching the filter."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineRejectionItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["platform", "reason", "count"],
            "properties": {
                "platform": {
                    "type": "string",
                    "description": "Platform label (general, apple, android)."
                },
                "reason": {
                    "type": "string",
                    "description": "Canonical rejection reason label."
                },
                "count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of occurrences recorded for this platform/reason pair."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineRejectionListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["items", "total"],
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/OfflineRejectionItem"
                    }
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Sum of all rejection counters returned in the payload."
                }
            }
        }),
    );
    schemas.insert(
        "OfflineQueryEnvelope".to_owned(),
        norito::json!({
            "type": "object",
            "description": "Superset of Torii's Norito QueryEnvelope structure.",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Optional label for the query payload."
                },
                "filter": {
                    "type": "object",
                    "description": "Filter expression object expressed in the Torii filter DSL.",
                    "additionalProperties": true
                },
                "select": {
                    "type": "object",
                    "description": "Optional projection selector.",
                    "additionalProperties": true
                },
                "sort": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "description": "Field/order pair such as {\"key\":\"registered_at_ms\",\"order\":\"desc\"}.",
                        "additionalProperties": true
                    }
                },
                "pagination": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "format": "uint64"
                        },
                        "offset": {
                            "type": "integer",
                            "format": "uint64"
                        }
                    }
                },
                "fetch_size": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Optional batch fetch size hint."
                },
                "address_format": {
                    "type": "string",
                    "description": "Optional rendering preference for account ids (`ih58` or `compressed`)."
                }
            }
        }),
    );
    schemas.insert(
        "TimeHealth".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["healthy", "min_samples_ok", "offset_ok", "confidence_ok"],
            "additionalProperties": false,
            "properties": {
                "healthy": {
                    "type": "boolean",
                    "description": "Overall health status (true when all checks pass)."
                },
                "min_samples_ok": {
                    "type": "boolean",
                    "description": "Whether the minimum sample threshold is satisfied."
                },
                "offset_ok": {
                    "type": "boolean",
                    "description": "Whether the absolute offset is within configured bounds."
                },
                "confidence_ok": {
                    "type": "boolean",
                    "description": "Whether the confidence (MAD) is within configured bounds."
                }
            }
        }),
    );
    schemas.insert(
        "TimeNowResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["now", "offset_ms", "confidence_ms", "sample_count", "peer_count", "fallback", "health"],
            "additionalProperties": false,
            "properties": {
                "now": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Network time in milliseconds since the UNIX epoch."
                },
                "offset_ms": {
                    "type": "integer",
                    "format": "int64",
                    "description": "Estimated offset from the local clock in milliseconds."
                },
                "confidence_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Confidence interval (±ms) reported by the network time service."
                },
                "sample_count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of peer samples used in the aggregation."
                },
                "peer_count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of peers with recent samples."
                },
                "fallback": {
                    "type": "boolean",
                    "description": "Whether the response falls back to the local clock."
                },
                "health": {
                    "$ref": "#/components/schemas/TimeHealth"
                }
            }
        }),
    );
    schemas.insert(
        "TimeStatusBucket".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["le", "count"],
            "additionalProperties": false,
            "properties": {
                "le": {
                    "type": "number",
                    "description": "Upper bound of the RTT bucket in milliseconds."
                },
                "count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of samples observed in the bucket."
                }
            }
        }),
    );
    schemas.insert(
        "TimeStatusSample".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["peer", "last_offset_ms", "last_rtt_ms", "count"],
            "additionalProperties": false,
            "properties": {
                "peer": {
                    "type": "string",
                    "description": "Peer identifier contributing time samples."
                },
                "last_offset_ms": {
                    "type": "integer",
                    "format": "int64",
                    "description": "Most recent offset measurement in milliseconds."
                },
                "last_rtt_ms": {
                    "type": "integer",
                    "format": "int64",
                    "description": "Most recent round-trip time measurement in milliseconds."
                },
                "count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of samples collected from the peer."
                }
            }
        }),
    );
    schemas.insert(
        "TimeStatusRtt".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["buckets", "sum_ms", "count"],
            "additionalProperties": false,
            "properties": {
                "buckets": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/TimeStatusBucket" }
                },
                "sum_ms": {
                    "type": "number",
                    "description": "Cumulative RTT sample sum in milliseconds."
                },
                "count": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total number of RTT samples captured."
                }
            }
        }),
    );
    schemas.insert(
        "TimeStatusResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["peers", "samples_used", "offset_ms", "confidence_ms", "fallback", "health", "samples", "rtt", "note"],
            "additionalProperties": false,
            "properties": {
                "peers": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of peers reporting time samples."
                },
                "samples_used": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of peer samples used in the aggregation."
                },
                "offset_ms": {
                    "type": "integer",
                    "format": "int64",
                    "description": "Estimated offset from the local clock in milliseconds."
                },
                "confidence_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Confidence interval (±ms) reported by the network time service."
                },
                "fallback": {
                    "type": "boolean",
                    "description": "Whether the service fell back to the local clock."
                },
                "health": {
                    "$ref": "#/components/schemas/TimeHealth"
                },
                "samples": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/TimeStatusSample" }
                },
                "rtt": {
                    "$ref": "#/components/schemas/TimeStatusRtt"
                },
                "note": {
                    "type": "string",
                    "description": "Human-readable status note from the network time service."
                }
            }
        }),
    );
    schemas.insert(
        "BlockSignature".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["index", "signature"],
            "additionalProperties": false,
            "properties": {
                "index": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Validator index within the certificate set."
                },
                "signature": {
                    "type": "string",
                    "description": "Signature over the block header encoded as hex."
                }
            }
        }),
    );
    schemas.insert(
        "BlockHeader".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["height", "creation_time_ms", "view_change_index"],
            "additionalProperties": false,
            "properties": {
                "height": { "type": "integer", "format": "uint64" },
                "prev_block_hash": { "anyOf": [ { "type": "string" }, { "type": "null" } ] },
                "merkle_root": { "anyOf": [ { "type": "string" }, { "type": "null" } ] },
                "result_merkle_root": { "anyOf": [ { "type": "string" }, { "type": "null" } ] },
                "da_commitments_hash": { "anyOf": [ { "type": "string" }, { "type": "null" } ] },
                "creation_time_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Unix timestamp in milliseconds when the block was created."
                },
                "view_change_index": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Consensus view index assigned to the block."
                },
                "confidential_features": {
                    "anyOf": [ { "type": "string" }, { "type": "null" } ],
                    "description": "Optional digest advertising enabled confidential features."
                }
            }
        }),
    );
    schemas.insert(
        "BlockHeaderList".to_owned(),
        norito::json!({
            "type": "array",
            "items": { "$ref": "#/components/schemas/BlockHeader" }
        }),
    );
    schemas.insert(
        "StateRootResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["height", "block_hash", "state_root", "source"],
            "additionalProperties": false,
            "properties": {
                "height": { "type": "integer", "format": "uint64" },
                "block_hash": { "type": "string", "description": "Block hash at the requested height (hex)." },
                "state_root": {
                    "type": "string",
                    "description": "Execution post-state root (hex) derived from the commit QC or result Merkle root."
                },
                "source": {
                    "type": "string",
                    "enum": ["commit_qc", "result_merkle_root"],
                    "description": "Indicates whether the state root came from the commit QC or the block result Merkle root."
                },
                "commit_qc": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/Qc" },
                        { "type": "null" }
                    ],
                    "description": "Commit QC payload when available."
                }
            }
        }),
    );
    schemas.insert(
        "StateProofResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["height", "block_hash", "state_root", "commit_qc"],
            "additionalProperties": false,
            "properties": {
                "height": { "type": "integer", "format": "uint64" },
                "block_hash": { "type": "string", "description": "Block hash at the requested height (hex)." },
                "state_root": {
                    "type": "string",
                    "description": "Execution post-state root (hex) attested by the QC."
                },
                "commit_qc": { "$ref": "#/components/schemas/Qc" }
            }
        }),
    );
    schemas.insert(
        "QcRef".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["height", "view", "epoch", "subject_block_hash", "phase"],
            "additionalProperties": false,
            "properties": {
                "height": { "type": "integer", "format": "uint64" },
                "view": { "type": "integer", "format": "uint64" },
                "epoch": { "type": "integer", "format": "uint64" },
                "subject_block_hash": { "type": "string", "description": "Hash of the certified block (hex)." },
                "phase": { "type": "string", "description": "Certified phase." }
            }
        }),
    );
    schemas.insert(
        "Qc".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "phase",
                "subject_block_hash",
                "parent_state_root",
                "post_state_root",
                "height",
                "view",
                "epoch",
                "mode_tag",
                "validator_set_hash",
                "validator_set_hash_version",
                "validator_set",
                "aggregate"
            ],
            "additionalProperties": false,
            "properties": {
                "phase": { "type": "string", "description": "Certified phase." },
                "subject_block_hash": { "type": "string", "description": "Hash of the certified block (hex)." },
                "parent_state_root": { "type": "string", "description": "Parent state root bound into the QC (hex)." },
                "post_state_root": { "type": "string", "description": "Post-state root bound into the QC (hex)." },
                "height": { "type": "integer", "format": "uint64" },
                "view": { "type": "integer", "format": "uint64" },
                "epoch": { "type": "integer", "format": "uint64" },
                "mode_tag": { "type": "string", "description": "Consensus mode tag used for domain separation." },
                "highest_qc": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/QcRef" },
                        { "type": "null" }
                    ],
                    "description": "Optional highest QC reference (advisory)."
                },
                "validator_set_hash": { "type": "string", "description": "Stable hash of the validator set." },
                "validator_set_hash_version": { "type": "integer", "format": "uint16" },
                "validator_set": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Ordered validator set used to assemble the certificate."
                },
                "aggregate": {
                    "type": "object",
                    "required": ["signers_bitmap", "bls_aggregate_signature"],
                    "additionalProperties": false,
                    "properties": {
                        "signers_bitmap": { "type": "string", "description": "Signer bitmap (hex, LSB-first)." },
                        "bls_aggregate_signature": { "type": "string", "description": "BLS aggregate signature (hex)." }
                    }
                }
            }
        }),
    );
    schemas.insert(
        "QcList".to_owned(),
        norito::json!({
            "type": "array",
            "items": { "$ref": "#/components/schemas/Qc" }
        }),
    );
    schemas.insert(
        "ValidatorSetSnapshot".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["height", "block_hash", "validator_set_hash", "validator_set_hash_version", "validator_set"],
            "additionalProperties": false,
            "properties": {
                "height": { "type": "integer", "format": "uint64" },
                "block_hash": { "type": "string", "description": "Block hash certified by the validator set (hex)." },
                "validator_set_hash": { "type": "string", "description": "Stable hash of the validator set." },
                "validator_set_hash_version": { "type": "integer", "format": "uint16" },
                "validator_set": { "type": "array", "items": { "type": "string" } }
            }
        }),
    );
    schemas.insert(
        "ValidatorSetSnapshotList".to_owned(),
        norito::json!({
            "type": "array",
            "items": { "$ref": "#/components/schemas/ValidatorSetSnapshot" }
        }),
    );
    schemas.insert(
        "BlockMerkleProof".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["leaf_index", "audit_path"],
            "additionalProperties": false,
            "properties": {
                "leaf_index": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Zero-based position of the leaf within the Merkle tree."
                },
                "audit_path": {
                    "type": "array",
                    "description": "Sibling nodes from leaf to root; null denotes an absent sibling for balanced padding.",
                    "items": {
                        "type": "string",
                        "nullable": true,
                        "description": "Sibling hash encoded as lowercase hexadecimal."
                    }
                }
            }
        }),
    );
    schemas.insert(
        "BlockReceiptProof".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["leaf", "proof"],
            "additionalProperties": false,
            "properties": {
                "leaf": {
                    "type": "string",
                    "description": "Transaction entrypoint hash encoded as lowercase hexadecimal."
                },
                "proof": { "$ref": "#/components/schemas/BlockMerkleProof" }
            }
        }),
    );
    schemas.insert(
        "BlockExecutionReceiptProof".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["leaf", "proof"],
            "additionalProperties": false,
            "properties": {
                "leaf": {
                    "type": "string",
                    "description": "Execution result hash encoded as lowercase hexadecimal."
                },
                "proof": { "$ref": "#/components/schemas/BlockMerkleProof" }
            }
        }),
    );
    schemas.insert(
        "BlockProofs".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["block_height", "entry_hash", "entry_root", "entry_proof", "result_root", "result_proof"],
            "additionalProperties": false,
            "properties": {
                "block_height": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Height of the block containing the entrypoint."
                },
                "entry_hash": {
                    "type": "string",
                    "description": "Transaction entrypoint hash encoded as lowercase hexadecimal."
                },
                "entry_root": {
                    "type": "string",
                    "description": "Merkle root that authenticates the entrypoint proof (consensus root for external transactions, extended root after time-trigger execution otherwise)."
                },
                "entry_proof": {
                    "$ref": "#/components/schemas/BlockReceiptProof"
                },
                "result_root": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ],
                    "description": "Merkle root that authenticates the execution proof when results are available; null when the block contains no execution results."
                },
                "result_proof": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/BlockExecutionReceiptProof" },
                        { "type": "null" }
                    ],
                    "description": "Execution result proof when the block carries execution results; null otherwise."
                }
            }
        }),
    );
    schemas.insert(
        "KaigiRelayStatus".to_owned(),
        norito::json!({
            "type": "string",
            "enum": ["healthy", "degraded", "unavailable"],
            "description": "Reported relay health label."
        }),
    );
    schemas.insert(
        "KaigiRelaySummary".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["relay_id", "domain", "bandwidth_class", "hpke_fingerprint_hex"],
            "additionalProperties": false,
            "properties": {
                "relay_id": {
                    "type": "string",
                    "description": "Relay account identifier (IH58 or `<alias|public_key>@domain`)."
                },
                "domain": {
                    "type": "string",
                    "description": "Domain that registered the relay."
                },
                "bandwidth_class": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 255,
                    "description": "Advertised bandwidth class (relative scale; larger values indicate higher capacity)."
                },
                "hpke_fingerprint_hex": {
                    "type": "string",
                    "description": "Hex-encoded BLAKE3 fingerprint of the relay's HPKE public key."
                },
                "status": {
                    "allOf": [
                        { "$ref": "#/components/schemas/KaigiRelayStatus" }
                    ],
                    "description": "Latest health status when available."
                },
                "reported_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Milliseconds since UNIX epoch of the latest health report."
                }
            }
        }),
    );
    schemas.insert(
        "KaigiRelaySummaryList".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["total", "items"],
            "additionalProperties": false,
            "properties": {
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total number of relay entries returned."
                },
                "items": {
                    "type": "array",
                    "description": "Relay summaries.",
                    "items": {
                        "$ref": "#/components/schemas/KaigiRelaySummary"
                    }
                }
            }
        }),
    );
    schemas.insert(
        "KaigiRelayDomainMetrics".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "domain",
                "registrations_total",
                "manifest_updates_total",
                "failovers_total",
                "health_reports_total"
            ],
            "additionalProperties": false,
            "properties": {
                "domain": {
                    "type": "string",
                    "description": "Domain label for the aggregated counters."
                },
                "registrations_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total relay registrations observed for the domain."
                },
                "manifest_updates_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total relay manifest updates observed for the domain."
                },
                "failovers_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total relay failovers observed for the domain."
                },
                "health_reports_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total relay health reports recorded for the domain."
                }
            }
        }),
    );
    schemas.insert(
        "KaigiId".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["domain_id", "call_name"],
            "additionalProperties": false,
            "properties": {
                "domain_id": {
                    "type": "string",
                    "description": "Domain that owns the Kaigi call."
                },
                "call_name": {
                    "type": "string",
                    "description": "Call name within the domain namespace."
                }
            }
        }),
    );
    schemas.insert(
        "KaigiRelayDetail".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["relay", "hpke_public_key_b64"],
            "additionalProperties": false,
            "properties": {
                "relay": {
                    "$ref": "#/components/schemas/KaigiRelaySummary"
                },
                "hpke_public_key_b64": {
                    "type": "string",
                    "description": "Base64-encoded HPKE public key advertised by the relay."
                },
                "reported_call": {
                    "allOf": [
                        { "$ref": "#/components/schemas/KaigiId" }
                    ],
                    "description": "Kaigi call associated with the latest health report, when available."
                },
                "reported_by": {
                    "type": "string",
                    "description": "Account identifier that submitted the latest health report."
                },
                "notes": {
                    "type": "string",
                    "description": "Free-form notes accompanying the latest health report."
                },
                "metrics": {
                    "allOf": [
                        { "$ref": "#/components/schemas/KaigiRelayDomainMetrics" }
                    ],
                    "description": "Aggregated counters for the relay's owning domain."
                }
            }
        }),
    );
    schemas.insert(
        "KaigiRelayHealthSnapshot".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "healthy_total",
                "degraded_total",
                "unavailable_total",
                "reports_total",
                "registrations_total",
                "failovers_total",
                "domains"
            ],
            "additionalProperties": false,
            "properties": {
                "healthy_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of relays currently reported healthy."
                },
                "degraded_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of relays currently reported degraded."
                },
                "unavailable_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of relays currently reported unavailable."
                },
                "reports_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total relay health reports observed."
                },
                "registrations_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total relay registrations observed."
                },
                "failovers_total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Total relay failovers observed."
                },
                "domains": {
                    "type": "array",
                    "description": "Per-domain relay metrics.",
                    "items": {
                        "$ref": "#/components/schemas/KaigiRelayDomainMetrics"
                    }
                }
            }
        }),
    );
    schemas.insert(
        "RepoLeg".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["asset_definition_id", "quantity", "metadata"],
            "additionalProperties": false,
            "properties": {
                "asset_definition_id": {
                    "type": "string",
                    "description": "Asset definition identifier associated with the leg."
                },
                "quantity": {
                    "type": "string",
                    "description": "Decimal representation of the leg quantity."
                },
                "metadata": {
                    "type": "object",
                    "description": "Arbitrary Norito metadata captured alongside the leg."
                }
            }
        }),
    );
    schemas.insert(
        "RepoGovernance".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["haircut_bps", "margin_frequency_secs"],
            "additionalProperties": false,
            "properties": {
                "haircut_bps": {
                    "type": "integer",
                    "format": "uint16",
                    "description": "Haircut applied to the collateral leg in basis points."
                },
                "margin_frequency_secs": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Cadence between required margin checks in seconds."
                }
            }
        }),
    );
    schemas.insert(
        "RepoAgreement".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "id",
                "initiator",
                "counterparty",
                "cash_leg",
                "collateral_leg",
                "rate_bps",
                "maturity_timestamp_ms",
                "initiated_timestamp_ms",
                "last_margin_check_timestamp_ms",
                "governance"
            ],
            "additionalProperties": false,
            "properties": {
                "id": { "type": "string" },
                "initiator": { "type": "string" },
                "counterparty": { "type": "string" },
                "custodian": {
                    "type": "string",
                    "nullable": true
                },
                "cash_leg": { "$ref": "#/components/schemas/RepoLeg" },
                "collateral_leg": { "$ref": "#/components/schemas/RepoLeg" },
                "rate_bps": {
                    "type": "integer",
                    "format": "uint16"
                },
                "maturity_timestamp_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "initiated_timestamp_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "last_margin_check_timestamp_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "governance": { "$ref": "#/components/schemas/RepoGovernance" }
            }
        }),
    );
    schemas.insert(
        "RepoAgreementListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["items", "total"],
            "additionalProperties": false,
            "properties": {
                "items": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/RepoAgreement" }
                },
                "total": {
                    "type": "integer",
                    "format": "uint64"
                }
            }
        }),
    );
    schemas.insert(
        "RepoAgreementsQueryRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["pagination"],
            "additionalProperties": false,
            "properties": {
                "pagination": {
                    "type": "object",
                    "required": ["offset"],
                    "properties": {
                        "offset": {
                            "type": "integer",
                            "format": "uint64"
                        },
                        "limit": {
                            "type": "integer",
                            "format": "uint64"
                        }
                    }
                },
                "sort": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["key", "order"],
                        "properties": {
                            "key": { "type": "string" },
                            "order": {
                                "type": "string",
                                "enum": ["asc", "desc"],
                                "default": "asc"
                            }
                        }
                    }
                },
                "filter": {
                    "type": "object",
                    "description": "Norito filter expression tree."
                },
                "fetch_size": {
                    "type": "integer",
                    "format": "uint64"
                },
                "address_format": {
                    "type": "string",
                    "enum": ["ih58", "compressed"]
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneValidatorStatus".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["type"],
            "additionalProperties": false,
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["PendingActivation", "Active", "Jailed", "Exiting", "Exited", "Slashed"],
                    "description": "Lifecycle status for the validator."
                },
                "activates_at_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Activation epoch when pending; host auto-promotes once the current epoch meets this value."
                },
                "reason": {
                    "type": "string",
                    "nullable": true,
                    "description": "Jail reason when applicable."
                },
                "releases_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Release timestamp (ms) when exiting."
                },
                "slash_id": {
                    "type": "string",
                    "nullable": true,
                    "description": "Slash identifier when slashed."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneValidatorRecord".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "validator", "stake_account", "total_stake", "self_stake", "status"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane serviced by the validator."
                },
                "validator": {
                    "type": "string",
                    "description": "Validator account literal rendered per address_format."
                },
                "stake_account": {
                    "type": "string",
                    "description": "Account backing the bonded stake."
                },
                "total_stake": {
                    "type": "string",
                    "description": "Total bonded stake for the validator."
                },
                "self_stake": {
                    "type": "string",
                    "description": "Validator-supplied stake amount."
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata (commission, endpoints, jurisdiction flags, etc.)."
                },
                "status": {
                    "$ref": "#/components/schemas/PublicLaneValidatorStatus"
                },
                "activation_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Epoch recorded when the validator first became active."
                },
                "activation_height": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Block height recorded when the validator first became active."
                },
                "last_reward_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Epoch that last produced a reward payout."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneValidatorListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "total", "items"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of validator entries returned."
                },
                "items": {
                    "type": "array",
                    "description": "Validator records for the lane.",
                    "items": { "$ref": "#/components/schemas/PublicLaneValidatorRecord" }
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneUnbonding".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["request_id", "amount", "release_at_ms"],
            "additionalProperties": false,
            "properties": {
                "request_id": {
                    "type": "string",
                    "description": "Client-supplied identifier rendered as hex."
                },
                "amount": {
                    "type": "string",
                    "description": "Amount scheduled for release."
                },
                "release_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Release timestamp (ms)."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneStakeShare".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "validator", "staker", "bonded", "metadata", "pending_unbonds"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane serviced by the stake entry."
                },
                "validator": {
                    "type": "string",
                    "description": "Validator account literal rendered per address_format."
                },
                "staker": {
                    "type": "string",
                    "description": "Staker account literal rendered per address_format."
                },
                "bonded": {
                    "type": "string",
                    "description": "Bonded amount for the validator/staker pair."
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata captured alongside the stake share."
                },
                "pending_unbonds": {
                    "type": "array",
                    "description": "Pending unbonding requests for the stake.",
                    "items": { "$ref": "#/components/schemas/PublicLaneUnbonding" }
                }
            }
        }),
    );
    schemas.insert(
        "PublicLaneStakeListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "total", "items"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of stake entries returned."
                },
                "items": {
                    "type": "array",
                    "description": "Stake shares for the lane.",
                    "items": { "$ref": "#/components/schemas/PublicLaneStakeShare" }
                }
            }
        }),
    );
    schemas.insert(
        "PublicLanePendingReward".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "lane_id",
                "account",
                "asset",
                "last_claimed_epoch",
                "pending_through_epoch",
                "amount"
            ],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "account": {
                    "type": "string",
                    "description": "Account literal for the reward recipient."
                },
                "asset": {
                    "type": "string",
                    "description": "Asset identifier for the reward payouts."
                },
                "last_claimed_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Latest epoch that was already claimed (0 when none)."
                },
                "pending_through_epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Latest epoch included in `amount`."
                },
                "amount": {
                    "type": "string",
                    "description": "Reward amount available to claim (decimal string)."
                }
            }
        }),
    );
    schemas.insert(
        "PublicLanePendingRewardListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["lane_id", "total", "items"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane identifier echoed from the request."
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of pending reward entries."
                },
                "items": {
                    "type": "array",
                    "description": "Pending rewards for the lane/account.",
                    "items": { "$ref": "#/components/schemas/PublicLanePendingReward" }
                }
            }
        }),
    );
    schemas.insert(
        "DaPagination".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "limit": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Maximum number of commitments to return."
                },
                "offset": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of commitments to skip from the start of the ordered index."
                }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentProofRequest".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "manifest_hash": {
                    "type": "string",
                    "description": "Hex-encoded manifest digest to look up."
                },
                "lane_id": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Lane identifier for the commitment."
                },
                "epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Epoch number for the commitment."
                },
                "sequence": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Monotonic sequence within the lane/epoch."
                },
                "pagination": {
                    "$ref": "#/components/schemas/DaPagination"
                }
            }
        }),
    );
    schemas.insert(
        "DaRetentionPolicy".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["hot_retention_secs", "cold_retention_secs", "required_replicas", "storage_class", "governance_tag"],
            "additionalProperties": false,
            "properties": {
                "hot_retention_secs": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Retention period in seconds for the hot tier."
                },
                "cold_retention_secs": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Retention period in seconds for the cold tier."
                },
                "required_replicas": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Replica count requested for the blob."
                },
                "storage_class": {
                    "type": "string",
                    "enum": ["Hot", "Warm", "Cold"],
                    "description": "Storage class requested for the primary replicas."
                },
                "governance_tag": {
                    "type": "string",
                    "description": "Governance tag anchoring the retention decision."
                }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentRecord".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "lane_id",
                "epoch",
                "sequence",
                "client_blob_id",
                "manifest_hash",
                "chunk_root",
                "retention_class",
                "storage_ticket",
                "acknowledgement_sig"
            ],
            "additionalProperties": false,
            "properties": {
                "lane_id": { "type": "integer", "format": "uint32" },
                "epoch": { "type": "integer", "format": "uint64" },
                "sequence": { "type": "integer", "format": "uint64" },
                "client_blob_id": {
                    "type": "string",
                    "description": "Hex-encoded blob identifier."
                },
                "manifest_hash": {
                    "type": "string",
                    "description": "Hex-encoded manifest digest (BLAKE3-256)."
                },
                "chunk_root": {
                    "type": "string",
                    "description": "Hex-encoded Merkle root over blob chunks."
                },
                "kzg_commitment": {
                    "anyOf": [ { "type": "string" }, { "type": "null" } ],
                    "description": "Optional KZG commitment in compressed hex form."
                },
                "proof_digest": {
                    "anyOf": [ { "type": "string" }, { "type": "null" } ],
                    "description": "Optional digest covering PDP/PoTR scheduling metadata."
                },
                "retention_class": {
                    "$ref": "#/components/schemas/DaRetentionPolicy"
                },
                "storage_ticket": {
                    "type": "string",
                    "description": "Hex-encoded storage ticket binding the blob to SoraFS."
                },
                "acknowledgement_sig": {
                    "type": "string",
                    "description": "Hex-encoded Torii acknowledgement signature."
                }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentLocation".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["block_height", "index_in_bundle"],
            "additionalProperties": false,
            "properties": {
                "block_height": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Height of the block that sealed the commitment."
                },
                "index_in_bundle": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Index within the block's commitment bundle."
                }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentWithLocation".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["commitment", "location"],
            "additionalProperties": false,
            "properties": {
                "commitment": { "$ref": "#/components/schemas/DaCommitmentRecord" },
                "location": { "$ref": "#/components/schemas/DaCommitmentLocation" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentWithLocationList".to_owned(),
        norito::json!({
            "type": "array",
            "items": { "$ref": "#/components/schemas/DaCommitmentWithLocation" }
        }),
    );
    schemas.insert(
        "MerkleDirection".to_owned(),
        norito::json!({
            "type": "string",
            "enum": ["Left", "Right"],
            "description": "Position of the sibling hash within the Merkle path."
        }),
    );
    schemas.insert(
        "MerklePathItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["sibling", "direction"],
            "additionalProperties": false,
            "properties": {
                "sibling": {
                    "type": "string",
                    "description": "Hex-encoded sibling hash for this tree level."
                },
                "direction": { "$ref": "#/components/schemas/MerkleDirection" }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentProof".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["commitment", "location", "bundle_hash", "bundle_len", "root", "path"],
            "additionalProperties": false,
            "properties": {
                "commitment": { "$ref": "#/components/schemas/DaCommitmentRecord" },
                "location": { "$ref": "#/components/schemas/DaCommitmentLocation" },
                "bundle_hash": {
                    "type": "string",
                    "description": "Hash of the full DA commitment bundle embedded in the block."
                },
                "bundle_len": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Total number of commitments in the bundle."
                },
                "root": {
                    "type": "string",
                    "description": "Merkle root over the bundle commitments."
                },
                "path": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/MerklePathItem" },
                    "description": "Merkle path proving inclusion of the commitment."
                }
            }
        }),
    );
    schemas.insert(
        "DaCommitmentProofResponse".to_owned(),
        norito::json!({
            "anyOf": [
                { "$ref": "#/components/schemas/DaCommitmentProof" },
                { "type": "null" }
            ]
        }),
    );
    schemas.insert(
        "DaCommitmentVerifyResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["valid"],
            "additionalProperties": false,
            "properties": {
                "valid": {
                    "type": "boolean",
                    "description": "Whether the supplied commitment matches the node's index."
                },
                "error": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional error string describing the verification failure."
                }
            }
        }),
    );
    schemas.insert(
        "OperatorWebAuthnOptionsResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["publicKey"],
            "additionalProperties": false,
            "properties": {
                "publicKey": { "$ref": "#/components/schemas/JsonValue" }
            }
        }),
    );
    schemas.insert(
        "OperatorWebAuthnRegistrationResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["status", "credential_id", "credentials_total"],
            "additionalProperties": false,
            "properties": {
                "status": { "type": "string" },
                "credential_id": { "type": "string" },
                "credentials_total": { "type": "integer", "format": "uint32" }
            }
        }),
    );
    schemas.insert(
        "OperatorWebAuthnLoginResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["status", "session_token", "expires_in_secs", "credential_id"],
            "additionalProperties": false,
            "properties": {
                "status": { "type": "string" },
                "session_token": { "type": "string" },
                "expires_in_secs": { "type": "integer", "format": "uint64" },
                "credential_id": { "type": "string" }
            }
        }),
    );
    schemas
}

fn shared_error_schema() -> Value {
    norito::json!({
        "type": "object",
        "required": ["code", "message"],
        "additionalProperties": true,
        "properties": {
            "code": {
                "type": "string",
                "description": "Application-specific error identifier."
            },
            "message": {
                "type": "string",
                "description": "Human readable error message."
            }
        }
    })
}

fn components_section() -> Value {
    let mut components = Map::new();
    let mut schemas = openapi_schemas();
    schemas.insert("ErrorResponse".to_owned(), shared_error_schema());
    components.insert("schemas".into(), Value::Object(schemas));
    Value::Object(components)
}

/// Returns the placeholder OpenAPI document used when the live spec is unavailable.
///
/// The stub advertises a deterministic skeleton so tooling can reference a
/// well-formed document even if routing fails to expose the spec.
#[must_use]
pub fn stub_spec() -> Value {
    let mut doc = Map::new();
    doc.insert("openapi".into(), Value::String("3.1.0".to_owned()));
    doc.insert("info".into(), info_section(license_section()));
    doc.insert("servers".into(), servers_section());
    doc.insert("paths".into(), Value::Object(Map::new()));
    doc.insert("components".into(), Value::Object(Map::new()));
    doc.insert("tags".into(), tags_section());
    Value::Object(doc)
}

/// Returns the OpenAPI specification for the full Torii surface.
#[must_use]
pub fn generate_spec() -> Value {
    let mut doc = Map::new();
    doc.insert("openapi".into(), Value::String("3.1.0".to_owned()));
    doc.insert("info".into(), info_section(license_section()));
    doc.insert("servers".into(), servers_section());
    doc.insert("tags".into(), tags_section());
    doc.insert("paths".into(), Value::Object(paths_section()));
    doc.insert("components".into(), components_section());
    Value::Object(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_spec_has_minimum_fields() {
        let doc = stub_spec();
        assert_eq!(
            doc.get("openapi").and_then(norito::json::Value::as_str),
            Some("3.1.0")
        );
        let info = doc
            .get("info")
            .and_then(|value| value.as_object())
            .expect("stub spec must expose info section");
        assert!(info.contains_key("title"));
        assert!(info.contains_key("version"));
    }

    #[test]
    fn generated_spec_includes_documented_paths() {
        let doc = generate_spec();
        if std::env::var("PRINT_TORII_SPEC").is_ok() {
            if let Ok(json) = norito::json::to_string_pretty(&doc) {
                println!("{json}");
            }
        }
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        assert!(paths.contains_key("/v1/aliases/voprf/evaluate"));
        assert!(paths.contains_key("/v1/aliases/resolve"));
        assert!(paths.contains_key("/v1/aliases/resolve_index"));
        assert!(paths.contains_key("/v1/time/now"));
        assert!(paths.contains_key("/v1/time/status"));
        assert!(paths.contains_key("/v1/ledger/headers"));
        assert!(paths.contains_key("/v1/ledger/state/{height}"));
        assert!(paths.contains_key("/v1/ledger/state-proof/{height}"));
        assert!(paths.contains_key("/v1/ledger/block/{height}/proof/{entry_hash}"));
        assert!(paths.contains_key("/v1/da/commitments"));
        assert!(paths.contains_key("/v1/da/commitments/prove"));
        assert!(paths.contains_key("/v1/da/commitments/verify"));
        assert!(paths.contains_key("/v1/sumeragi/commit-certificates"));
        assert!(paths.contains_key("/v1/bridge/finality/{height}"));
        assert!(paths.contains_key("/v1/bridge/finality/bundle/{height}"));
        assert!(paths.contains_key("/v1/sumeragi/validator-sets"));
        assert!(paths.contains_key("/v1/sumeragi/validator-sets/{height}"));
        assert!(paths.contains_key("/health"));
        assert!(paths.contains_key("/v1/operator/auth/login/verify"));
        assert!(paths.contains_key("/v1/kaigi/relays"));
        assert!(paths.contains_key("/v1/kaigi/relays/{relay_id}"));
        assert!(paths.contains_key("/v1/kaigi/relays/health"));
        assert!(paths.contains_key("/v1/kaigi/relays/events"));
        assert!(paths.contains_key("/v1/nexus/public_lanes/{lane_id}/validators"));
        assert!(paths.contains_key("/v1/nexus/public_lanes/{lane_id}/stake"));
        assert!(paths.contains_key("/v1/repo/agreements"));
        assert!(paths.contains_key("/v1/repo/agreements/query"));
        assert!(paths.contains_key("/transaction"));
        assert!(paths.contains_key("/query"));
        assert!(paths.contains_key("/events"));
        assert!(paths.contains_key("/v1/da/ingest"));
        assert!(paths.contains_key("/v1/connect/session"));
        assert!(paths.contains_key("/v1/zk/attachments"));
        assert!(paths.contains_key("/v1/gov/proposals/deploy-contract"));
        assert!(paths.contains_key("/v1/runtime/abi/active"));
        assert!(paths.contains_key("/v1/accounts"));
        assert!(paths.contains_key("/v1/accounts/resolve"));
        assert!(paths.contains_key("/v1/assets/definitions"));
        assert!(paths.contains_key("/v1/explorer/accounts"));
        assert!(paths.contains_key("/v1/sorafs/providers"));
        assert!(paths.contains_key("/v1/soradns/directory/latest"));
        assert!(paths.contains_key("/v1/content/{bundle}/{path}"));
        assert!(paths.contains_key("/v1/sns/registrations"));
        assert!(paths.contains_key("/v1/soranet/privacy/event"));
        assert!(paths.contains_key("/v1/webhooks"));
        assert!(paths.contains_key("/v1/notify/devices"));
    }

    #[test]
    fn pipeline_status_documents_not_found_response() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        let status = paths
            .get("/v1/pipeline/transactions/status")
            .and_then(Value::as_object)
            .expect("pipeline status path");
        let get = status
            .get("get")
            .and_then(Value::as_object)
            .expect("get op");
        let responses = get
            .get("responses")
            .and_then(Value::as_object)
            .expect("responses");
        assert!(
            responses.contains_key("404"),
            "pipeline status should document 404 for missing cache entries"
        );
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn helper_builders_emit_expected_shapes() {
        let body = binary_request_body("binary payload");
        let body_obj = body.as_object().expect("binary request body object");
        assert_eq!(
            body_obj.get("description").and_then(Value::as_str),
            Some("binary payload")
        );
        let content = body_obj
            .get("content")
            .and_then(Value::as_object)
            .expect("binary request body content");
        assert!(content.contains_key("application/x-norito"));
        assert!(content.contains_key("application/octet-stream"));

        let response = binary_response("binary response");
        let response_obj = response.as_object().expect("binary response object");
        assert_eq!(
            response_obj.get("description").and_then(Value::as_str),
            Some("binary response")
        );
        let response_content = response_obj
            .get("content")
            .and_then(Value::as_object)
            .expect("binary response content");
        assert!(response_content.contains_key("application/octet-stream"));

        let param = string_path_param("item", "Item id.");
        let param_obj = param.as_object().expect("string param object");
        assert_eq!(param_obj.get("name").and_then(Value::as_str), Some("item"));
        let int_param = integer_path_param("height", "Height.", Some("uint64"));
        let int_param_obj = int_param.as_object().expect("integer param object");
        let schema = int_param_obj
            .get("schema")
            .and_then(Value::as_object)
            .expect("integer schema");
        assert_eq!(schema.get("format").and_then(Value::as_str), Some("uint64"));

        let get_op = json_get_operation(
            "System",
            "Summary",
            "Description",
            "#/components/schemas/JsonValue",
            vec![string_path_param("id", "Identifier.")],
        );
        assert!(get_op.contains_key("get"));

        let post_op = json_post_operation(
            "System",
            "Summary",
            "Description",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        );
        let post_operation = post_op
            .get("post")
            .and_then(Value::as_object)
            .expect("post operation");
        let post_body = post_operation
            .get("requestBody")
            .and_then(Value::as_object)
            .expect("post request body");
        let post_content = post_body
            .get("content")
            .and_then(Value::as_object)
            .expect("post request body content");
        assert!(post_content.contains_key("application/json"));

        let delete_op = json_delete_operation(
            "System",
            "Summary",
            "Description",
            "#/components/schemas/JsonValue",
            Vec::new(),
        );
        assert!(delete_op.contains_key("delete"));

        let text_op = text_get_operation("System", "Summary", "Description", Some("example"));
        let text_get = text_op
            .get("get")
            .and_then(Value::as_object)
            .expect("text get operation");
        let text_responses = text_get
            .get("responses")
            .and_then(Value::as_object)
            .expect("text responses");
        let text_ok = text_responses
            .get("200")
            .and_then(Value::as_object)
            .expect("text ok response");
        let text_content = text_ok
            .get("content")
            .and_then(Value::as_object)
            .expect("text response content");
        assert!(text_content.contains_key("text/plain"));

        let sse_op = event_stream_get_operation("Streams", "Summary", "Description");
        let sse_get = sse_op
            .get("get")
            .and_then(Value::as_object)
            .expect("sse get operation");
        let sse_responses = sse_get
            .get("responses")
            .and_then(Value::as_object)
            .expect("sse responses");
        let sse_ok = sse_responses
            .get("200")
            .and_then(Value::as_object)
            .expect("sse ok response");
        let sse_content = sse_ok
            .get("content")
            .and_then(Value::as_object)
            .expect("sse response content");
        assert!(sse_content.contains_key("text/event-stream"));

        let binary_post = binary_post_operation(
            "Transactions",
            "Submit",
            "Description",
            "#/components/schemas/JsonValue",
        );
        let binary_operation = binary_post
            .get("post")
            .and_then(Value::as_object)
            .expect("binary post operation");
        let binary_body = binary_operation
            .get("requestBody")
            .and_then(Value::as_object)
            .expect("binary request body");
        let binary_content = binary_body
            .get("content")
            .and_then(Value::as_object)
            .expect("binary request content");
        assert!(binary_content.contains_key("application/x-norito"));
    }

    #[test]
    fn path_group_builders_expose_expected_routes() {
        struct PathCase {
            label: &'static str,
            builder: fn() -> Map,
            expected: &'static str,
        }

        let cases = [
            PathCase {
                label: "aliases",
                builder: alias_paths,
                expected: "/v1/aliases/resolve",
            },
            PathCase {
                label: "time",
                builder: time_paths,
                expected: "/v1/time/now",
            },
            PathCase {
                label: "ledger",
                builder: ledger_paths,
                expected: "/v1/ledger/headers",
            },
            PathCase {
                label: "da",
                builder: da_paths,
                expected: "/v1/da/ingest",
            },
            PathCase {
                label: "offline",
                builder: offline_paths,
                expected: "/v1/offline/revocations",
            },
            PathCase {
                label: "system",
                builder: system_paths,
                expected: "/status",
            },
            PathCase {
                label: "operator_auth",
                builder: operator_auth_paths,
                expected: "/v1/operator/auth/login/options",
            },
            PathCase {
                label: "transactions",
                builder: transaction_paths,
                expected: "/transaction",
            },
            PathCase {
                label: "queries",
                builder: query_paths,
                expected: "/query",
            },
            PathCase {
                label: "streams",
                builder: stream_paths,
                expected: "/events",
            },
            PathCase {
                label: "connect",
                builder: connect_paths,
                expected: "/v1/connect/session",
            },
            PathCase {
                label: "proofs",
                builder: proof_paths,
                expected: "/v1/proofs/retention",
            },
            PathCase {
                label: "contracts",
                builder: contracts_paths,
                expected: "/v1/contracts/deploy",
            },
            PathCase {
                label: "zk",
                builder: zk_paths,
                expected: "/v1/zk/roots",
            },
            PathCase {
                label: "governance",
                builder: governance_paths,
                expected: "/v1/gov/proposals/deploy-contract",
            },
            PathCase {
                label: "runtime",
                builder: runtime_paths,
                expected: "/v1/runtime/abi/active",
            },
            PathCase {
                label: "accounts",
                builder: account_paths,
                expected: "/v1/accounts",
            },
            PathCase {
                label: "domains",
                builder: domain_paths,
                expected: "/v1/domains",
            },
            PathCase {
                label: "assets",
                builder: asset_paths,
                expected: "/v1/assets/definitions",
            },
            PathCase {
                label: "nfts",
                builder: nft_paths,
                expected: "/v1/nfts",
            },
            PathCase {
                label: "parameters",
                builder: parameter_paths,
                expected: "/v1/parameters",
            },
            PathCase {
                label: "space_directory",
                builder: space_directory_paths,
                expected: "/v1/space-directory/uaids/{uaid}",
            },
            PathCase {
                label: "explorer",
                builder: explorer_paths,
                expected: "/v1/explorer/accounts",
            },
            PathCase {
                label: "sorafs",
                builder: sorafs_paths,
                expected: "/v1/sorafs/providers",
            },
            PathCase {
                label: "soradns",
                builder: soradns_paths,
                expected: "/v1/soradns/directory/latest",
            },
            PathCase {
                label: "content",
                builder: content_paths,
                expected: "/v1/content/{bundle}/{path}",
            },
            PathCase {
                label: "sns",
                builder: sns_paths,
                expected: "/v1/sns/registrations",
            },
            PathCase {
                label: "soranet",
                builder: soranet_paths,
                expected: "/v1/soranet/privacy/event",
            },
            PathCase {
                label: "push",
                builder: push_paths,
                expected: "/v1/notify/devices",
            },
            PathCase {
                label: "webhooks",
                builder: webhook_paths,
                expected: "/v1/webhooks",
            },
            PathCase {
                label: "kaigi",
                builder: kaigi_paths,
                expected: "/v1/kaigi/relays",
            },
            PathCase {
                label: "nexus",
                builder: nexus_paths,
                expected: "/v1/nexus/lifecycle",
            },
            PathCase {
                label: "sumeragi",
                builder: sumeragi_paths,
                expected: "/v1/sumeragi/status",
            },
            PathCase {
                label: "repo",
                builder: repo_paths,
                expected: "/v1/repo/agreements",
            },
        ];

        for case in cases {
            let paths = (case.builder)();
            assert!(
                paths.contains_key(case.expected),
                "{} paths should include {}",
                case.label,
                case.expected
            );
        }
    }

    #[test]
    fn openapi_schemas_include_system_keys() {
        let schemas = openapi_schemas();
        for key in [
            "JsonValue",
            "JsonList",
            "ApiVersionInfo",
            "PeerIdList",
            "PushRegisterDeviceRequest",
        ] {
            assert!(schemas.contains_key(key), "schema missing {key}");
        }
    }

    #[test]
    fn tags_section_includes_push_tag() {
        let tags = match tags_section() {
            Value::Array(tags) => tags,
            _ => panic!("tags section should be an array"),
        };
        let mut has_push = false;
        for tag in tags {
            let Some(obj) = tag.as_object() else { continue };
            if obj.get("name").and_then(Value::as_str) == Some("Push") {
                has_push = true;
                break;
            }
        }
        assert!(has_push, "tags should include Push");
    }
}
