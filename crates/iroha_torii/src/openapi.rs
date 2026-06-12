//! Helpers for serving Torii's OpenAPI description.
//!
//! The OpenAPI document is assembled explicitly with Norito JSON helpers (instead
//! of relying on `serde_json`) to keep the toolchain consistent with on-wire
//! serialization.

use iroha_torii_shared::uri;
use norito::json::{Map, Value};

pub(crate) const TOOL_EFFECT_EXTENSION: &str = "x-iroha-tool-effect";

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
            "HTTP surface for Torii. Public blockchain reads are available without canonical request signing when they address public state; private dataspace and account-scoped views require canonical signing and permission checks."
                .to_owned(),
        ),
    );
    info.insert("version".into(), Value::String("1.0".to_owned()));
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
            "Offline readiness and issuer endpoints. Issuer POSTs carry canonical auth in the JSON body instead of X-Iroha-* headers."
                .to_owned(),
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

    let mut multisig = Map::new();
    multisig.insert("name".into(), Value::String("Multisig".to_owned()));
    multisig.insert(
        "description".into(),
        Value::String(
            "Alias-aware multisig propose, approve, spec, and proposal lookup helpers.".to_owned(),
        ),
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

    let mut ram_lfe = Map::new();
    ram_lfe.insert("name".into(), Value::String("RamLfe".to_owned()));
    ram_lfe.insert(
        "description".into(),
        Value::String(
            "Generic RAM-LFE program-policy, execute, and receipt-verification helpers.".to_owned(),
        ),
    );

    let mut identifiers = Map::new();
    identifiers.insert("name".into(), Value::String("Identifiers".to_owned()));
    identifiers.insert(
        "description".into(),
        Value::String(
            "Identifier-policy listing, resolution, and claim-receipt helpers.".to_owned(),
        ),
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

    let mut vpn = Map::new();
    vpn.insert("name".into(), Value::String("VPN".to_owned()));
    vpn.insert(
        "description".into(),
        Value::String("Sora VPN profile, session, and receipt endpoints.".to_owned()),
    );

    let mut mcp = Map::new();
    mcp.insert("name".into(), Value::String("MCP".to_owned()));
    mcp.insert(
        "description".into(),
        Value::String("Model Context Protocol endpoint for agent tool orchestration.".to_owned()),
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

    let mut soracloud = Map::new();
    soracloud.insert("name".into(), Value::String("Soracloud".to_owned()));
    soracloud.insert(
        "description".into(),
        Value::String(
            "Soracloud service, model, private uploaded-model runtime, and receipt endpoints."
                .to_owned(),
        ),
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
        Value::Object(multisig),
        Value::Object(zk),
        Value::Object(proofs),
        Value::Object(governance),
        Value::Object(runtime),
        Value::Object(settlement),
        Value::Object(accounts),
        Value::Object(ram_lfe),
        Value::Object(identifiers),
        Value::Object(domains),
        Value::Object(assets),
        Value::Object(nfts),
        Value::Object(subscriptions),
        Value::Object(parameters),
        Value::Object(explorer),
        Value::Object(connect),
        Value::Object(vpn),
        Value::Object(mcp),
        Value::Object(push),
        Value::Object(webhooks),
        Value::Object(sorafs),
        Value::Object(soracloud),
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
    schema_ref("ErrorEnvelope")
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
    paths.insert(
        "/v1/aliases/by_account".to_owned(),
        Value::Object(alias_lookup_by_account_operation()),
    );
    paths.insert(
        "/v1/assets/aliases/resolve".to_owned(),
        Value::Object(asset_alias_resolve_operation()),
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
            "Fetch indexed DA pin intent location data.",
            "Locate a pin intent by storage ticket, alias, manifest hash, or lane/epoch/sequence and return the indexed block location payload.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/da/pin_intents/verify".to_owned(),
        Value::Object(json_post_operation(
            "DataAvailability",
            "Verify indexed DA pin intent location data.",
            "Check that a provided DA pin intent location payload matches the node's current pin-intent index.",
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
        "/v1/offline/readiness".to_owned(),
        Value::Object(json_get_operation(
            "Offline",
            "Report Offline feature readiness.",
            "Returns readiness signals for the SDK-backed recursive compact Kagemusha native bridge runtime.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/offline/v2/readiness".to_owned(),
        Value::Object(json_get_operation(
            "Offline",
            "Report Offline V2 feature readiness.",
            "Returns readiness signals for the SDK-backed recursive compact Kagemusha native bridge runtime.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    for (path, summary, description) in [
        (
            "/v1/offline/v2/keys/refill",
            "Refill Offline V2 issuer keys.",
            "POST Offline V2 issuer key-refill material. The JSON body must include account_id, timestamp_ms, nonce, and exactly one of signature_base64 or witness_base64. The canonical request signs the body after removing only the top-level proof fields. Legacy X-Iroha-* app-auth headers are rejected on this endpoint.",
        ),
        (
            "/v1/offline/v2/notes/issue",
            "Issue an Offline V2 note.",
            "POST an Offline V2 note issuance request. The JSON body must include account_id, timestamp_ms, nonce, and exactly one of signature_base64 or witness_base64. The canonical request signs the body after removing only the top-level proof fields. Legacy X-Iroha-* app-auth headers are rejected on this endpoint.",
        ),
        (
            "/v1/offline/v2/notes/redeem",
            "Redeem an Offline V2 note.",
            "POST an Offline V2 note redemption request. The JSON body must include account_id, timestamp_ms, nonce, and exactly one of signature_base64 or witness_base64. The canonical request signs the body after removing only the top-level proof fields. Legacy X-Iroha-* app-auth headers are rejected on this endpoint.",
        ),
        (
            "/v1/offline/v2/audit",
            "Submit an Offline V2 audit request.",
            "POST an Offline V2 audit request. The JSON body must include account_id, timestamp_ms, nonce, and exactly one of signature_base64 or witness_base64. The canonical request signs the body after removing only the top-level proof fields. Legacy X-Iroha-* app-auth headers are rejected on this endpoint.",
        ),
    ] {
        paths.insert(
            path.to_owned(),
            Value::Object(json_post_operation(
                "Offline",
                summary,
                description,
                "#/components/schemas/OfflineIssuerBodyAuthRequest",
                "#/components/schemas/JsonValue",
                Vec::new(),
            )),
        );
    }
    paths
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
    params
}

fn pagination_query_parameters() -> Vec<Value> {
    vec![
        integer_query_param("limit", "Optional page size limit.", Some("uint64")),
        integer_query_param(
            "offset",
            "Optional starting offset (default 0).",
            Some("uint64"),
        ),
        string_query_param(
            "count_mode",
            "Count mode: `bounded` omits exact totals; `exact` includes an exact `total`.",
        ),
    ]
}

fn app_list_query_parameters() -> Vec<Value> {
    let mut params = vec![
        string_query_param(
            "filter",
            "Optional Norito JSON filter expression encoded as a compact string (see Torii filter DSL).",
        ),
        string_query_param("sort", "Optional CSV sort spec, e.g., `id:asc`."),
    ];
    params.extend(pagination_query_parameters());
    params
}

fn explorer_pagination_query_parameters() -> Vec<Value> {
    vec![
        integer_query_param("page", "Optional page number (default 1).", Some("uint64")),
        integer_query_param(
            "per_page",
            "Optional items per page (default 10).",
            Some("uint64"),
        ),
    ]
}

fn explorer_assets_query_parameters() -> Vec<Value> {
    let mut params = explorer_pagination_query_parameters();
    params.push(string_query_param(
        "owned_by",
        "Filter assets by account owner (accepts canonical I105 account literals or on-chain aliases `name@domain.dataspace` / `name@dataspace`).",
    ));
    params.push(string_query_param(
        "definition",
        "Filter assets by asset definition selector (canonical Base58 asset definition id or on-chain alias `name#domain.dataspace` / `name#dataspace`).",
    ));
    params.push(string_query_param(
        "asset_id",
        "Filter assets by canonical Base58 asset id.",
    ));
    params
}

fn explorer_transactions_query_parameters() -> Vec<Value> {
    let mut params = explorer_pagination_query_parameters();
    params.push(string_query_param(
        "authority",
        "Filter transactions by authority account (accepts canonical I105 account literals or on-chain aliases `name@domain.dataspace` / `name@dataspace`).",
    ));
    params.push(integer_query_param(
        "block",
        "Filter transactions by block height.",
        Some("uint64"),
    ));
    params.push(string_query_param(
        "status",
        "Filter transactions by status (`committed` or `rejected`).",
    ));
    params.push(string_query_param(
        "asset_id",
        "Filter transactions by canonical Base58 asset id.",
    ));
    params
}

fn explorer_instructions_query_parameters() -> Vec<Value> {
    let mut params = explorer_pagination_query_parameters();
    params.push(string_query_param(
        "authority",
        "Filter instructions by authority account (accepts canonical I105 account literals or on-chain aliases `name@domain.dataspace` / `name@dataspace`).",
    ));
    params.push(string_query_param(
        "account",
        "Filter instructions by referenced account (transfer participants, asset-owner scoped mint/burn/asset-metadata updates, multisig accounts, and public-lane reward assets; accepts canonical I105 account literals or on-chain aliases `name@domain.dataspace` / `name@dataspace`).",
    ));
    params.push(string_query_param(
        "transaction_hash",
        "Filter instructions by transaction hash.",
    ));
    params.push(string_query_param(
        "transaction_status",
        "Filter instructions by transaction status (`committed` or `rejected`).",
    ));
    params.push(integer_query_param(
        "block",
        "Filter instructions by block height.",
        Some("uint64"),
    ));
    params.push(string_query_param(
        "kind",
        "Filter instructions by instruction kind (e.g., `transfer`, `mint`, `burn`).",
    ));
    params.push(string_query_param(
        "asset_id",
        "Filter instructions by canonical Base58 asset id.",
    ));
    params
}

fn account_assets_list_query_parameters() -> Vec<Value> {
    let mut params = pagination_query_parameters();
    params.push(string_query_param(
        "asset",
        "Filter assets by asset definition selector.",
    ));
    params.push(string_query_param(
        "scope",
        "Filter assets by balance scope (`global` or `dataspace:<id>`).",
    ));
    params
}

fn account_transactions_list_query_parameters() -> Vec<Value> {
    let mut params = pagination_query_parameters();
    params.push(string_query_param(
        "asset_id",
        "Filter transactions by canonical Base58 asset id.",
    ));
    params
}

fn contract_activity_list_query_parameters() -> Vec<Value> {
    let mut params = pagination_query_parameters();
    params.push(string_query_param(
        "authority",
        "Filter by canonical I105 authority.",
    ));
    params.push(string_query_param(
        "contract_address",
        "Filter by contract address.",
    ));
    params.push(string_query_param(
        "contract_alias",
        "Filter by deployed contract alias.",
    ));
    params.push(string_query_param(
        "contract_entrypoint",
        "Filter by contract entrypoint name.",
    ));
    params.push(integer_query_param(
        "since_timestamp_ms",
        "Filter items whose timestamp is greater than or equal to this value.",
        Some("uint64"),
    ));
    params.push(integer_query_param(
        "until_timestamp_ms",
        "Filter items whose timestamp is less than or equal to this value.",
        Some("uint64"),
    ));
    params.push(bool_query_param(
        "result_ok",
        "Filter by execution outcome.",
    ));
    params
}

fn contract_event_list_query_parameters() -> Vec<Value> {
    let mut params = pagination_query_parameters();
    params.push(string_query_param(
        "authority",
        "Filter by canonical I105 authority.",
    ));
    params.push(string_query_param(
        "contract_address",
        "Filter by contract address.",
    ));
    params.push(string_query_param(
        "contract_alias",
        "Filter by deployed contract alias.",
    ));
    params.push(string_query_param(
        "module",
        "Filter by derived module identifier.",
    ));
    params.push(string_query_param(
        "event_kind",
        "Filter by generic contract event kind.",
    ));
    params.push(string_query_param(
        "participant",
        "Filter by participant account/account-alias-like references.",
    ));
    params.push(string_query_param(
        "asset_id",
        "Filter by referenced asset identifier.",
    ));
    params.push(string_query_param(
        "provenance",
        "Filter by event provenance (`emitted` or `derived`).",
    ));
    params.push(integer_query_param(
        "since_timestamp_ms",
        "Filter items whose timestamp is greater than or equal to this value.",
        Some("uint64"),
    ));
    params.push(integer_query_param(
        "until_timestamp_ms",
        "Filter items whose timestamp is less than or equal to this value.",
        Some("uint64"),
    ));
    params.push(bool_query_param(
        "result_ok",
        "Filter by execution outcome.",
    ));
    params
}

fn contract_rollup_swaps_fills_query_parameters() -> Vec<Value> {
    let mut params = pagination_query_parameters();
    params.push(required_string_query_param(
        "authority",
        "Trader authority whose swap fills should be returned.",
    ));
    params.push(string_query_param(
        "contract_address",
        "Optional canonical contract address for the router.",
    ));
    params.push(string_query_param(
        "contract_alias",
        "Optional contract alias for the router.",
    ));
    params.push(integer_query_param(
        "scan_limit",
        "Optional scan limit for walking the router mirror history.",
        Some("uint64"),
    ));
    params
}

fn contract_rollup_swaps_candles_query_parameters() -> Vec<Value> {
    let mut params = contract_rollup_swaps_fills_query_parameters();
    params.push(integer_query_param(
        "bucket_ms",
        "Optional candle bucket width in milliseconds.",
        Some("uint64"),
    ));
    params
}

fn contract_rollup_uranai_market_history_query_parameters() -> Vec<Value> {
    let mut params = pagination_query_parameters();
    params.push(required_string_query_param(
        "market_id",
        "Uranai market id whose committed DPM price history should be replayed.",
    ));
    params.push(string_query_param(
        "contract_address",
        "Optional canonical contract address for the Uranai market contract.",
    ));
    params.push(string_query_param(
        "contract_alias",
        "Optional contract alias for the Uranai market contract.",
    ));
    params.push(integer_query_param(
        "since_timestamp_ms",
        "Return replayed points whose timestamp is greater than or equal to this value.",
        Some("uint64"),
    ));
    params.push(integer_query_param(
        "until_timestamp_ms",
        "Return replayed points whose timestamp is less than or equal to this value.",
        Some("uint64"),
    ));
    params
}

fn trader_account_rollup_query_parameters() -> Vec<Value> {
    vec![
        required_string_query_param(
            "authority",
            "Trader authority whose account summary should be returned.",
        ),
        integer_query_param(
            "scan_limit",
            "Optional scan limit for walking router history and event history.",
            Some("uint64"),
        ),
    ]
}

fn asset_holders_list_query_parameters() -> Vec<Value> {
    let mut params = pagination_query_parameters();
    params.push(string_query_param(
        "account_id",
        "Filter holders by canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
    ));
    params.push(string_query_param(
        "scope",
        "Filter holders by balance scope (`global` or `dataspace:<id>`).",
    ));
    params
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

fn iso_profile_selection_parameters() -> Vec<Value> {
    vec![
        string_header_param(
            "X-Iroha-Iso-Profile",
            "Optional ISO bridge rail profile. Overrides the `profile` query parameter.",
            false,
        ),
        string_query_param(
            "profile",
            "Optional ISO bridge rail profile used when `X-Iroha-Iso-Profile` is absent.",
        ),
    ]
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
fn system_paths() -> Map {
    let mut paths = Map::new();
    paths.insert("/health".to_owned(), Value::Object(health_operation()));
    #[cfg(feature = "telemetry")]
    {
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
    }
    paths.insert(
        uri::API_VERSION.to_owned(),
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
        uri::PEERS.to_owned(),
        Value::Object(json_get_operation(
            "System",
            "List online peers.",
            "Return the peers snapshot as a list of peer ids.",
            "#/components/schemas/PeerIdList",
            Vec::new(),
        )),
    );
    paths.insert(
        uri::CONFIGURATION.to_owned(),
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
    #[cfg(feature = "schema")]
    {
        paths.insert(
            uri::SCHEMA.to_owned(),
            Value::Object(json_get_operation(
                "System",
                "Fetch the data model schema snapshot.",
                "Return the schema payload used by the node build.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            )),
        );
    }
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
    #[cfg(feature = "profiling")]
    {
        paths.insert(
            uri::PROFILE.to_owned(),
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
                    Value::String(
                        "Return a profiling payload for the configured duration.".to_owned(),
                    ),
                );
                let mut responses = Map::new();
                responses.insert("200".to_owned(), binary_response("CPU profile payload."));
                operation.insert("responses".into(), Value::Object(responses));
                let mut methods = Map::new();
                methods.insert("get".to_owned(), Value::Object(operation));
                methods
            }),
        );
    }
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
    let mut pipeline_preflight = json_get_operation(
        "System",
        "Fetch pipeline preflight diagnostics.",
        "Return transaction admission, queue, fee, and consensus liveness budgets used by clients before submitting transactions. Defaults to application/x-norito when Accept is omitted or */*; application/json returns the same typed payload encoded as JSON.",
        "#/components/schemas/PipelinePreflightResponse",
        Vec::new(),
    );
    if let Some(Value::Object(get_op)) = pipeline_preflight.get_mut("get") {
        if let Some(Value::Object(responses)) = get_op.get_mut("responses") {
            responses.insert(
                "200".to_owned(),
                Value::Object(single_dual_format_response(
                    "#/components/schemas/PipelinePreflightResponse",
                ))
                .get("200")
                .cloned()
                .expect("200 response present"),
            );
        }
    }
    paths.insert(
        "/v1/pipeline/preflight".to_owned(),
        Value::Object(pipeline_preflight),
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
        iroha_torii_shared::uri::PIPELINE_FASTPQ_PROOFS.to_owned(),
        Value::Object(json_get_operation(
            "System",
            "Fetch FASTPQ proof sidecars.",
            "Return FASTPQ proof attachments from pipeline recovery metadata for the given height.",
            "#/components/schemas/JsonValue",
            vec![integer_path_param(
                "height",
                "Block height to inspect.",
                Some("uint64"),
            )],
        )),
    );
    #[cfg(feature = "telemetry")]
    {
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
    }
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
    paths.insert(
        "/v1/telemetry/live".to_owned(),
        Value::Object(event_stream_get_operation(
            "System",
            "Stream live telemetry updates.",
            "Stream peer and network telemetry updates via SSE.",
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
        uri::TRANSACTION.to_owned(),
        Value::Object(versioned_dual_format_post_operation(
            "Transactions",
            "Submit a versioned signed transaction.",
            "Submit a versioned SignedTransaction encoded as Norito bytes or as canonical Norito JSON. Internal TransactionEntrypoint envelopes are not accepted on this public route.",
            "#/components/schemas/VersionedSignedTransactionJson",
            "#/components/schemas/TransactionSubmissionReceipt",
            202,
        )),
    );
    paths.insert(
        uri::TRANSACTION_ENTRYPOINT.to_owned(),
        Value::Object(versioned_dual_format_post_operation(
            "Transactions",
            "Submit a versioned transaction entrypoint.",
            "Submit a versioned TransactionEntrypoint encoded as Norito bytes or as canonical Norito JSON. This route accepts sealed commitment/reveal entrypoints and other non-legacy transaction envelopes.",
            "#/components/schemas/VersionedTransactionEntrypointJson",
            "#/components/schemas/TransactionSubmissionReceipt",
            202,
        )),
    );
    paths.insert(
        uri::TRANSACTIONS_BATCH.to_owned(),
        Value::Object(binary_post_operation(
            "Transactions",
            "Submit a batch of versioned signed transactions.",
            "Submit a Norito-encoded batch of SignedTransaction payloads.",
            "#/components/schemas/JsonValue",
        )),
    );
    let mut pipeline_status = json_get_operation(
        "Transactions",
        "Fetch pipeline transaction status.",
        "Return the latest typed pipeline status for a signed transaction hash. Defaults to application/x-norito when Accept is omitted or */*; application/json returns the same typed payload encoded as JSON.",
        "#/components/schemas/PipelineTransactionStatusResponse",
        vec![
            required_string_query_param("hash", "Transaction hash (hex)."),
            string_query_param(
                "scope",
                "Read scope hint (`local`, `auto`, or `global`; defaults to `auto`).",
            ),
        ],
    );
    if let Some(Value::Object(get_op)) = pipeline_status.get_mut("get") {
        if let Some(Value::Object(responses)) = get_op.get_mut("responses") {
            responses.insert(
                "200".to_owned(),
                Value::Object(single_dual_format_response(
                    "#/components/schemas/PipelineTransactionStatusResponse",
                ))
                .get("200")
                .cloned()
                .expect("200 response present"),
            );
            responses.insert(
                "404".to_owned(),
                dual_format_response(
                    "Pipeline status not found; Torii has no cached entry for the hash yet.",
                    "#/components/schemas/ErrorEnvelope",
                ),
            );
        }
    }
    paths.insert(
        "/v1/pipeline/transactions/status".to_owned(),
        Value::Object(pipeline_status),
    );
    paths.insert(
        "/v1/transactions/history".to_owned(),
        Value::Object(json_get_operation(
            "Transactions",
            "List transaction history entries.",
            "Return the app-facing transaction history view for the active query filters.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/transactions/query".to_owned(),
        Value::Object(json_post_operation(
            "Transactions",
            "Query committed transactions.",
            "Query committed transactions with the structured QueryEnvelope filter, sort, projection, and pagination shape. This global endpoint is intended for privileged operators and developer tooling.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/transactions/visible/query".to_owned(),
        Value::Object(json_post_operation(
            "Transactions",
            "Query visible transactions.",
            "Query committed transactions visible to the authenticated viewer. Torii derives viewer scope from Authorization headers before applying the QueryEnvelope filter, sort, projection, and pagination shape.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/activity".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "List contract activity entries.",
            "Return committed contract-call activity derived from transaction metadata, with pagination and contract-specific filtering.",
            "#/components/schemas/JsonValue",
            contract_activity_list_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/contracts/events".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "List generic contract events.",
            "Return the indexed generic contract-event envelope derived from committed contract-call metadata, with pagination and contract-specific filtering.",
            "#/components/schemas/JsonValue",
            contract_event_list_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/contracts/rollups/swaps/fills".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "List stitched trader swap fills.",
            "Load the router mirror history for one trader authority, stitch it to indexed swap events, and return a paginated fills rollup.",
            "#/components/schemas/JsonValue",
            contract_rollup_swaps_fills_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/contracts/rollups/swaps/candles".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "List stitched trader swap candles.",
            "Bucket the trader swap fills rollup into OHLC candles for one authority and return the paginated time buckets.",
            "#/components/schemas/JsonValue",
            contract_rollup_swaps_candles_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/contracts/rollups/uranai/markets/history".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "List Uranai market price history.",
            "Replay committed Uranai market events in block order and return paginated DPM probability and XOR price points for one market.",
            "#/components/schemas/JsonValue",
            contract_rollup_uranai_market_history_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/contracts/rollups/trader/activity".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "List trader activity cards.",
            "Return the trader-facing activity stream derived from indexed contract events across the supported product modules.",
            "#/components/schemas/JsonValue",
            contract_event_list_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/contracts/rollups/trader/account".to_owned(),
        Value::Object(json_get_operation(
            "Contracts",
            "Fetch trader account summary.",
            "Combine the stitched swap fills rollup with supported trader activity modules into one account summary response.",
            "#/components/schemas/JsonValue",
            trader_account_rollup_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/iso20022/pacs008".to_owned(),
        Value::Object(json_post_operation(
            "ISO20022",
            "Submit ISO 20022 pacs.008 payload.",
            "Submit a pacs.008 message for ISO 20022 bridging.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            iso_profile_selection_parameters(),
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
            iso_profile_selection_parameters(),
        )),
    );
    for (path, message_type, summary) in [
        (
            "/v1/iso20022/pacs002",
            "pacs.002",
            "Submit ISO 20022 pacs.002 lifecycle payload.",
        ),
        (
            "/v1/iso20022/pacs004",
            "pacs.004",
            "Submit ISO 20022 pacs.004 lifecycle payload.",
        ),
        (
            "/v1/iso20022/camt056",
            "camt.056",
            "Submit ISO 20022 camt.056 lifecycle payload.",
        ),
        (
            "/v1/iso20022/sese023",
            "sese.023",
            "Submit ISO 20022 sese.023 settlement instruction payload.",
        ),
        (
            "/v1/iso20022/sese024",
            "sese.024",
            "Submit ISO 20022 sese.024 settlement status payload.",
        ),
        (
            "/v1/iso20022/sese025",
            "sese.025",
            "Submit ISO 20022 sese.025 settlement confirmation payload.",
        ),
        (
            "/v1/iso20022/colr012",
            "colr.012",
            "Submit ISO 20022 colr.012 collateral substitution confirmation payload.",
        ),
    ] {
        paths.insert(
            path.to_owned(),
            Value::Object(json_post_operation(
                "ISO20022",
                summary,
                &format!("Submit a {message_type} message for ISO 20022 lifecycle bridging."),
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                iso_profile_selection_parameters(),
            )),
        );
    }
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
    paths.insert(
        "/v1/iso20022/messages/{msg_id}".to_owned(),
        Value::Object(json_get_operation(
            "ISO20022",
            "Fetch ISO 20022 message record.",
            "Return the rich ISO 20022 message record by message id, including profile metadata and status history.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("msg_id", "ISO 20022 message id.")],
        )),
    );
    paths.insert(
        "/v1/iso20022/audit/messages".to_owned(),
        Value::Object(json_get_operation(
            "ISO20022",
            "Fetch ISO 20022 durable audit index.",
            "Return the deterministic tamper-evident audit manifest for durable ISO 20022 bridge records.",
            "#/components/schemas/JsonValue",
            vec![],
        )),
    );
    paths.insert(
        "/v1/iso20022/messages/{msg_id}/pacs002".to_owned(),
        Value::Object(json_get_operation(
            "ISO20022",
            "Fetch ISO 20022 pacs.002 status XML.",
            "Return the current ISO 20022 payment status report for a bridged message.",
            "#/components/schemas/XmlText",
            vec![string_path_param("msg_id", "ISO 20022 message id.")],
        )),
    );
    for (path, message_type, summary, description) in [
        (
            "/v1/iso20022/messages/{msg_id}/pacs004",
            "pacs.004",
            "Fetch ISO 20022 pacs.004 return XML.",
            "Return a payment return message from the durable ISO bridge record.",
        ),
        (
            "/v1/iso20022/messages/{msg_id}/camt029",
            "camt.029",
            "Fetch ISO 20022 camt.029 cancellation-resolution XML.",
            "Return a cancellation resolution message from the durable ISO bridge record.",
        ),
        (
            "/v1/iso20022/messages/{msg_id}/sese024",
            "sese.024",
            "Fetch ISO 20022 sese.024 settlement-status XML.",
            "Return a securities settlement status advice from the durable ISO bridge record.",
        ),
        (
            "/v1/iso20022/messages/{msg_id}/sese025",
            "sese.025",
            "Fetch ISO 20022 sese.025 settlement-confirmation XML.",
            "Return a securities settlement confirmation from the durable ISO bridge record.",
        ),
    ] {
        paths.insert(
            path.to_owned(),
            Value::Object(json_get_operation(
                "ISO20022",
                summary,
                &format!("{description} The response body is {message_type} XML."),
                "#/components/schemas/XmlText",
                vec![string_path_param("msg_id", "ISO 20022 message id.")],
            )),
        );
    }
    paths
}

fn query_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        uri::QUERY.to_owned(),
        Value::Object(versioned_dual_format_post_operation(
            "Queries",
            "Submit a signed query.",
            "Submit a SignedQuery encoded as Norito bytes or as canonical Norito JSON.",
            "#/components/schemas/VersionedSignedQueryJson",
            "#/components/schemas/JsonValue",
            200,
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
        "/v1/contracts/events/sse".to_owned(),
        Value::Object(event_stream_get_operation(
            "Streams",
            "Subscribe to contract event stream.",
            "Stream generic contract events via Server-Sent Events.",
        )),
    );
    paths.insert(
        uri::SUBSCRIPTION.to_owned(),
        Value::Object(text_get_operation(
            "Streams",
            "Connect to the event WebSocket.",
            "Upgrade to the event subscription WebSocket.",
            None,
        )),
    );
    paths.insert(
        uri::BLOCKS_STREAM.to_owned(),
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
            "Terminate a Connect session by session id. Requires `Authorization: Bearer <token_management>` from session creation.",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param("sid", "Connect session id."),
                string_header_param(
                    "Authorization",
                    "Bearer management token returned by `/v1/connect/session`.",
                    true,
                ),
            ],
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
            "Return redacted aggregate Connect relay status. Add `sid` with `Authorization: Bearer <token_management>` for per-session status.",
            "#/components/schemas/JsonValue",
            vec![
                string_query_param("sid", "Optional Connect session id for token-gated per-session status."),
                string_header_param(
                    "Authorization",
                    "Bearer management token required when `sid` is supplied.",
                    false,
                ),
            ],
        )),
    );
    paths
}

fn vpn_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/vpn/profile".to_owned(),
        Value::Object(json_get_operation(
            "VPN",
            "Fetch the public VPN profile.",
            "Return the wallet-facing Sora VPN profile advertised by this node.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/vpn/quotes".to_owned(),
        Value::Object(json_post_operation(
            "VPN",
            "Create a VPN quote.",
            "Create a signed XOR escrow quote for a Sora VPN lease. The request must include `metering_public_key_hex`; successful responses include `tx_instructions` with a native `OpenVpnLeaseEscrow` skeleton for client signing/submission.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/vpn/sessions".to_owned(),
        Value::Object(json_post_operation(
            "VPN",
            "Create a VPN session.",
            "Create a signed Sora VPN session from a quote and committed native `OpenVpnLeaseEscrow` XOR payment transaction.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/vpn/sessions/{session_id}".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "VPN",
                "Fetch a VPN session.",
                "Return the current status of a signed Sora VPN session.",
                "#/components/schemas/JsonValue",
                vec![string_path_param("session_id", "VPN session identifier.")],
            );
            let delete_op = json_delete_operation(
                "VPN",
                "Delete a VPN session.",
                "Tear down a signed Sora VPN session and return the canonical receipt.",
                "#/components/schemas/JsonValue",
                vec![string_path_param("session_id", "VPN session identifier.")],
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
        "/v1/vpn/receipts".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "VPN",
                "List VPN receipts.",
                "List canonical Sora VPN receipts for the active wallet account.",
                "#/components/schemas/JsonValue",
                Vec::new(),
            );
            let post_op = json_post_operation(
                "VPN",
                "Submit a VPN receipt.",
                "Settle an active VPN lease from relay and client usage evidence. Successful responses include `tx_instructions` with a native `SettleVpnLease` skeleton for operator signing/submission.",
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

fn mcp_paths() -> Map {
    let mut get_operation = Map::new();
    get_operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("MCP".to_owned())]),
    );
    get_operation.insert(
        "summary".into(),
        Value::String("Fetch MCP capabilities.".to_owned()),
    );
    get_operation.insert(
        "description".into(),
        Value::String(
            "Returns server capabilities and tool count for the native MCP bridge.".to_owned(),
        ),
    );
    get_operation.insert(
        "operationId".into(),
        Value::String("mcpCapabilities".to_owned()),
    );
    get_operation.insert(
        "responses".into(),
        Value::Object(single_json_response("#/components/schemas/JsonValue")),
    );

    let mut post_operation = Map::new();
    post_operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("MCP".to_owned())]),
    );
    post_operation.insert(
        "summary".into(),
        Value::String("Execute MCP JSON-RPC request.".to_owned()),
    );
    post_operation.insert(
        "description".into(),
        Value::String(
            "Accepts JSON-RPC payloads for MCP tool discovery and invocation.".to_owned(),
        ),
    );
    post_operation.insert("operationId".into(), Value::String("mcpJsonRpc".to_owned()));
    post_operation.insert(
        "requestBody".into(),
        Value::Object(json_request_body("#/components/schemas/JsonValue")),
    );
    post_operation.insert(
        "responses".into(),
        Value::Object(single_json_response("#/components/schemas/JsonValue")),
    );

    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(get_operation));
    methods.insert("post".to_owned(), Value::Object(post_operation));

    let mut paths = Map::new();
    paths.insert("/v1/mcp".to_owned(), Value::Object(methods));
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
            "Deploy a public contract.",
            "Deploy contract bytecode, derive a canonical contract address, activate it in the target dataspace (default `universal`), and return the canonical deploy-bundle receipt with one contract entry.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/call".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Call a deployed contract.",
            "Invoke a contract entrypoint by canonical contract address or by on-chain contract alias.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/call/simulate".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Simulate a deployed contract call.",
            "Execute a public contract entrypoint locally using canonical contract address or on-chain contract alias targeting and return normalized payload, gas usage, queued instructions, and diagnostics.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/view".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Execute a read-only contract view.",
            "Execute a manifest-validated read-only contract view entrypoint and return its decoded result.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/view/batch".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Execute multiple read-only contract views.",
            "Execute a batch of manifest-validated read-only contract view entrypoints and return one normalized item per request.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/aliases/resolve".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Resolve a contract alias.",
            "Resolve an on-chain contract alias to its canonical contract address and current lease status.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/contracts/aliases".to_owned(),
        Value::Object(json_post_operation(
            "Contracts",
            "Bind or clear a contract alias.",
            "Bind, update, or clear the on-chain alias for a deployed contract instance.",
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
                string_query_param(
                    "contract_address",
                    "Optional contract address used to scope logical state paths.",
                ),
                string_query_param(
                    "contract_alias",
                    "Optional contract alias used to scope logical state paths.",
                ),
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
    paths
}

fn multisig_post_operation(
    summary: &str,
    description: &str,
    request_schema_ref: &str,
    response_schema_ref: &str,
    not_found_description: &str,
) -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Multisig".to_owned())]),
    );
    operation.insert("summary".into(), Value::String(summary.to_owned()));
    operation.insert("description".into(), Value::String(description.to_owned()));
    operation.insert(
        "requestBody".into(),
        Value::Object(json_or_norito_request_body(request_schema_ref)),
    );
    let mut responses = single_json_response(response_schema_ref);
    responses.insert(
        "400".to_owned(),
        json_response(
            "Invalid multisig selector or payload.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response(not_found_description, error_schema_reference()),
    );
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn multisig_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/multisig/propose".to_owned(),
        Value::Object(multisig_post_operation(
            "Propose a multisig instruction batch.",
            "Resolve a multisig selector, wrap an instruction batch in a multisig proposal envelope, and optionally submit it.",
            "#/components/schemas/MultisigProposeRequest",
            "#/components/schemas/MultisigResponse",
            "Multisig alias not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/approve".to_owned(),
        Value::Object(multisig_post_operation(
            "Approve a multisig proposal.",
            "Resolve a multisig selector, target a proposal by `proposal_id` or `instructions_hash`, and optionally submit the approval transaction.",
            "#/components/schemas/MultisigApproveRequest",
            "#/components/schemas/MultisigResponse",
            "Multisig alias or referenced proposal not found.",
        )),
    );
    paths.insert(
        "/v1/contracts/call/multisig/propose".to_owned(),
        Value::Object(multisig_post_operation(
            "Propose a multisig contract call.",
            "Resolve a multisig selector, wrap a contract call in a multisig proposal envelope, and optionally submit it.",
            "#/components/schemas/MultisigContractCallProposeRequest",
            "#/components/schemas/MultisigContractCallResponse",
            "Multisig alias or referenced contract instance not found.",
        )),
    );
    paths.insert(
        "/v1/contracts/call/multisig/approve".to_owned(),
        Value::Object(multisig_post_operation(
            "Approve a multisig contract call proposal.",
            "Resolve a multisig selector, target a proposal by `proposal_id` or `instructions_hash`, and optionally submit the approval transaction.",
            "#/components/schemas/MultisigContractCallApproveRequest",
            "#/components/schemas/MultisigContractCallResponse",
            "Multisig alias or referenced proposal not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/cancel".to_owned(),
        Value::Object(multisig_post_operation(
            "Cancel a multisig proposal.",
            "Resolve a multisig selector, target an active proposal by `proposal_id` or `instructions_hash`, and either propose or approve the corresponding cancel action.",
            "#/components/schemas/MultisigCancelRequest",
            "#/components/schemas/MultisigCancelResponse",
            "Multisig alias or referenced proposal not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/spec".to_owned(),
        Value::Object(multisig_post_operation(
            "Fetch the active multisig spec.",
            "Resolve a multisig selector and return the current active concrete authority plus its multisig specification.",
            "#/components/schemas/MultisigSpecRequest",
            "#/components/schemas/MultisigSpecResponse",
            "Multisig alias not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/proposals/list".to_owned(),
        Value::Object(multisig_post_operation(
            "List active multisig proposals.",
            "Resolve a multisig selector and list nonterminal proposals for the active concrete multisig authority.",
            "#/components/schemas/MultisigProposalsListRequest",
            "#/components/schemas/MultisigProposalsListResponse",
            "Multisig alias not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/proposals/get".to_owned(),
        Value::Object(multisig_post_operation(
            "Fetch a multisig proposal.",
            "Resolve a multisig selector and fetch a proposal by `proposal_id` or `instructions_hash`.",
            "#/components/schemas/MultisigProposalsGetRequest",
            "#/components/schemas/MultisigProposalGetResponse",
            "Multisig alias or proposal not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/approvals/list".to_owned(),
        Value::Object(multisig_post_operation(
            "List multisig approvals.",
            "Resolve a multisig selector and list approvals recorded for the active concrete multisig authority.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            "Multisig alias not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/approvals/get".to_owned(),
        Value::Object(multisig_post_operation(
            "Fetch a multisig approval.",
            "Resolve a multisig selector and fetch a recorded approval by proposal selector.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            "Multisig alias or approval not found.",
        )),
    );
    paths.insert(
        "/v1/multisig/approvals/list_for_authority".to_owned(),
        Value::Object(multisig_post_operation(
            "List caller-authority multisig approvals.",
            "List multisig approvals visible to the authenticated caller authority using the signatory index.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            "Caller authority is not allowed to view the requested approvals.",
        )),
    );
    paths.insert(
        "/v1/multisig/approvals/get_for_authority".to_owned(),
        Value::Object(multisig_post_operation(
            "Fetch a caller-authority multisig approval.",
            "Fetch a multisig approval visible to the authenticated caller authority by proposal selector.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            "Caller authority is not allowed to view the requested approval.",
        )),
    );
    paths
}

fn controls_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/controls/asset-transfer/get".to_owned(),
        Value::Object(json_post_operation(
            "Controls",
            "Fetch asset-transfer control state.",
            "Resolve an asset-transfer control request and return the current control snapshot.",
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
        "/v1/zk/ivm/prove".to_owned(),
        Value::Object(json_post_operation(
            "ZK",
            "Prove IVM execution (job).",
            "Submit an IVM proved payload and return a job identifier for proof generation.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/zk/ivm/prove/{job_id}".to_owned(),
        Value::Object({
            let get_op = json_get_operation(
                "ZK",
                "Fetch an IVM prove job.",
                "Fetch the status of an IVM proof generation job.",
                "#/components/schemas/JsonValue",
                vec![string_path_param(
                    "job_id",
                    "Proof generation job identifier.",
                )],
            );
            let delete_op = json_delete_operation(
                "ZK",
                "Delete an IVM prove job.",
                "Delete an IVM proof generation job entry.",
                "#/components/schemas/JsonValue",
                vec![string_path_param(
                    "job_id",
                    "Proof generation job identifier.",
                )],
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
        "/v1/ministry/agenda/proposals/draft".to_owned(),
        Value::Object(json_post_operation(
            "Ministry",
            "Draft a Ministry agenda proposal submission.",
            "Build a detached-signature-ready Ministry agenda proposal transaction and return the canonical payload bytes for Connect signing.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/ministry/agenda/proposals/{proposal_id}".to_owned(),
        Value::Object(json_get_operation(
            "Ministry",
            "Fetch a submitted Ministry agenda proposal.",
            "Fetch a persisted Ministry agenda proposal submission record by proposal id.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "proposal_id",
                "Agenda proposal identifier.",
            )],
        )),
    );
    paths.insert(
        "/v1/gov/proposals/deploy-contract".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Propose contract deployment.",
            "Submit a governance proposal for contract deployment and receive draft instructions for local signing.",
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
            "Submit a zero-knowledge ballot and receive draft instructions unless the request is invalid.",
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
            "Submit a ZK ballot using the v1 envelope and receive draft instructions unless the request is invalid.",
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
            "Submit a ZK ballot proof bundle and receive draft instructions unless the request is invalid.",
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
            "Submit a non-ZK ballot and receive draft instructions unless the request is invalid.",
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
            "Finalize referendum tally and status and receive draft instructions for local signing.",
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
        "/v1/gov/stream".to_owned(),
        Value::Object(event_stream_get_operation(
            "Governance",
            "Stream governance updates.",
            "Stream governance update notifications via SSE.",
        )),
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
        "/v1/gov/contracts/{contract_address}".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch a governed contract binding.",
            "Fetch the active governance binding for a canonical contract address.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "contract_address",
                "Canonical Bech32m contract address.",
            )],
        )),
    );
    paths.insert(
        "/v1/gov/enact".to_owned(),
        Value::Object(json_post_operation(
            "Governance",
            "Enact a referendum.",
            "Enact an approved referendum and receive draft instructions for local signing.",
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
        "/v1/gov/citizens".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch citizenship registry count.",
            "Return the exact number of accounts registered in the governance citizenship registry.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/gov/citizens/{account_id}".to_owned(),
        Value::Object(json_get_operation(
            "Governance",
            "Fetch account citizenship status.",
            "Return whether an account is registered in the governance citizenship registry.",
            "#/components/schemas/JsonValue",
            vec![path_param("account_id", "Account id.")],
        )),
    );
    #[cfg(feature = "gov_vrf")]
    {
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
    }
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
    #[cfg(feature = "gov_vrf")]
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
        "/v1/node/query/projection/checkpoint".to_owned(),
        Value::Object(json_get_operation(
            "Runtime",
            "Fetch latest query projection checkpoint.",
            "Return the latest persisted query projection checkpoint descriptor.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/node/query/projection/checkpoint/plan".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("Runtime".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Plan a rebuilt query projection checkpoint.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String(
                    "Rebuilds the referenced shard archives from the live node snapshot, validates that the uploaded manifest/ticket set exactly matches the canonical non-empty live shard catalog, and returns the checkpoint descriptor without persisting it."
                        .to_owned(),
                ),
            );
            operation.insert(
                "operationId".into(),
                Value::String("nodeQueryProjectionCheckpointPlan".to_owned()),
            );
            operation.insert(
                "requestBody".into(),
                Value::Object(json_request_body("#/components/schemas/JsonValue")),
            );
            operation.insert(
                "responses".into(),
                Value::Object({
                    let mut responses = Map::new();
                    responses.insert(
                        "200".to_owned(),
                        json_response(
                            "Validated query projection checkpoint descriptor.",
                            schema_ref("#/components/schemas/JsonValue"),
                        ),
                    );
                    responses
                }),
            );
            let mut methods = Map::new();
            methods.insert("post".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/node/query/projection/checkpoint/publish".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("Runtime".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Publish a rebuilt query projection checkpoint.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String(
                    "Rebuilds the referenced shard archives from the live node snapshot, validates that the uploaded manifest/ticket set exactly matches the canonical non-empty live shard catalog, and persists the resulting checkpoint descriptor."
                        .to_owned(),
                ),
            );
            operation.insert(
                "operationId".into(),
                Value::String("nodeQueryProjectionCheckpointPublish".to_owned()),
            );
            operation.insert(
                "requestBody".into(),
                Value::Object(json_request_body("#/components/schemas/JsonValue")),
            );
            operation.insert(
                "responses".into(),
                Value::Object({
                    let mut responses = Map::new();
                    responses.insert(
                        "200".to_owned(),
                        json_response(
                            "Persisted query projection checkpoint descriptor.",
                            schema_ref("#/components/schemas/JsonValue"),
                        ),
                    );
                    responses
                }),
            );
            let mut methods = Map::new();
            methods.insert("post".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/node/query/projection/catalog/{resource}".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("Runtime".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Enumerate the live query projection shard catalog.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String(
                    "Returns the canonical ordered non-empty shard entries for the requested live projection resource. Supported resources are `accounts`, `account_assets`, `asset_holders`, `asset_definitions`, and `domains`. `asset_definition_id` narrows `asset_holders`, while `offset` and `limit` paginate the stable entry set."
                        .to_owned(),
                ),
            );
            operation.insert(
                "operationId".into(),
                Value::String("nodeQueryProjectionShardCatalog".to_owned()),
            );
            operation.insert(
                "parameters".into(),
                Value::Array(vec![
                    string_path_param(
                        "resource",
                        "Projection resource family (`accounts`, `account_assets`, `asset_holders`, `asset_definitions`, or `domains`).",
                    ),
                    string_query_param(
                        "asset_definition_id",
                        "Canonical or alias asset-definition selector used to narrow `asset_holders` entries.",
                    ),
                    integer_query_param(
                        "offset",
                        "Stable entry offset within the canonical ordered catalog.",
                        Some("uint64"),
                    ),
                    integer_query_param(
                        "limit",
                        "Maximum number of catalog entries to return.",
                        Some("uint32"),
                    ),
                ]),
            );
            operation.insert(
                "responses".into(),
                Value::Object({
                    let mut responses = Map::new();
                    responses.insert(
                        "200".to_owned(),
                        json_response(
                            "Live query projection shard catalog.",
                            schema_ref("#/components/schemas/JsonValue"),
                        ),
                    );
                    responses
                }),
            );
            let mut methods = Map::new();
            methods.insert("get".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/node/query/projection/shards/{resource}/{partition_id}".to_owned(),
        Value::Object({
            let mut operation = Map::new();
            operation.insert(
                "tags".into(),
                Value::Array(vec![Value::String("Runtime".to_owned())]),
            );
            operation.insert(
                "summary".into(),
                Value::String("Export a canonical query projection shard archive.".to_owned()),
            );
            operation.insert(
                "description".into(),
                Value::String(
                    "Builds a Norito-encoded `QueryProjectionShardArchive` for the requested resource partition directly from the node's live query snapshot. Supported resources are `accounts`, `account_assets`, `asset_holders`, `asset_definitions`, and `domains`; `asset_definition_id` is required when `resource=asset_holders`."
                        .to_owned(),
                ),
            );
            operation.insert(
                "operationId".into(),
                Value::String("nodeQueryProjectionShardExport".to_owned()),
            );
            operation.insert(
                "parameters".into(),
                Value::Array(vec![
                    string_path_param(
                        "resource",
                        "Projection resource family (`accounts`, `account_assets`, `asset_holders`, `asset_definitions`, or `domains`).",
                    ),
                    integer_path_param(
                        "partition_id",
                        "Stable projection partition identifier.",
                        Some("uint32"),
                    ),
                    string_query_param(
                        "asset_definition_id",
                        "Canonical or alias asset-definition selector required for `asset_holders`.",
                    ),
                ]),
            );
            operation.insert(
                "responses".into(),
                Value::Object({
                    let mut responses = Map::new();
                    responses.insert(
                        "200".to_owned(),
                        binary_response("Norito-encoded query projection shard archive payload."),
                    );
                    responses
                }),
            );
            let mut methods = Map::new();
            methods.insert("get".to_owned(), Value::Object(operation));
            methods
        }),
    );
    paths.insert(
        "/v1/runtime/abi/active".to_owned(),
        Value::Object(json_get_operation(
            "Runtime",
            "Fetch the active ABI version.",
            "Return the fixed runtime ABI version.",
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
            "List accounts visible to the caller. `id` filters accept canonical I105 account ids or on-chain aliases in `name@domain.dataspace` / `name@dataspace` form; responses always render canonical I105 account ids.",
            "#/components/schemas/AccountListResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/query".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Query accounts.",
            "Query accounts with JSON envelope. `id` filters accept canonical I105 account ids or on-chain aliases in `name@domain.dataspace` / `name@dataspace` form; responses always render canonical I105 account ids.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/AccountQueryResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/onboard".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Onboard an account.",
            "Register or onboard an account with an explicit canonical UAID. The request must provide `uaid` plus either `account_id` or `public_key_hex`; raw identity metadata is rejected, and optional identity evidence must be submitted only as `identity_commitment_hex`. When the UAID is not bound to the universal dataspace, Torii publishes a default manifest to bind it (requires CanPublishSpaceDirectoryManifest{dataspace=0}).",
            "#/components/schemas/AccountOnboardingRequest",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    let mut account_get = json_get_operation(
        "Accounts",
        "Fetch canonical account detail.",
        "Fetch the canonical account existence/materialization view for a specific account id or alias. This read is ingress-independent and fans out across the configured Nexus dataspaces, returning the shared canonical account payload when any authoritative dataspace reports it. Defaults to application/x-norito when Accept is omitted or */*; application/json returns the same typed payload encoded as JSON.",
        "#/components/schemas/AccountReadResponse",
        vec![string_path_param(
            "account_id",
            "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
        )],
    );
    if let Some(Value::Object(get_op)) = account_get.get_mut("get") {
        if let Some(Value::Object(responses)) = get_op.get_mut("responses") {
            responses.insert(
                "200".to_owned(),
                Value::Object(single_dual_format_response(
                    "#/components/schemas/AccountReadResponse",
                ))
                .get("200")
                .cloned()
                .expect("200 response present"),
            );
            responses.insert(
                "404".to_owned(),
                dual_format_response("Account not found.", "#/components/schemas/ErrorEnvelope"),
            );
        }
    }
    paths.insert(
        "/v1/accounts/{account_id}".to_owned(),
        Value::Object(account_get),
    );
    paths.insert(
        "/v1/accounts/faucet/puzzle".to_owned(),
        Value::Object(json_get_operation(
            "Accounts",
            "Fetch the faucet proof-of-work puzzle.",
            "Return the current decentralized faucet proof-of-work puzzle, anchored to recent committed block data; difficulty adapts to recent committed and queued faucet claim volume, the work predicate is memory-hard scrypt, and finalized VRF seed material is required in the challenge when that mode is enabled.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/faucet".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Request faucet funds.",
            "Transfer a fixed amount of testnet funds to an existing account when the configured faucet is enabled, the account has no positive balance for the configured asset, and a valid memory-hard scrypt proof-of-work solution for the returned queue-aware puzzle is supplied when required.",
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
            "Query transactions for a specific account. Results are merged across the caller-visible dataspaces only; private dataspace history the caller cannot read is silently omitted.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "account_id",
                "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
            )],
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/assets".to_owned(),
        Value::Object({
            let mut params = vec![string_path_param(
                "account_id",
                "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
            )];
            params.extend(account_assets_list_query_parameters());
            json_get_operation(
                "Accounts",
                "List account assets.",
                "List assets held by an account (supports pagination plus optional `asset` and `scope` filtering). Results are ingress-independent and merged across the caller-visible dataspaces; balances remain separated by their existing `scope` values instead of being summed across dataspaces.",
                "#/components/schemas/AccountAssetListResponse",
                params,
            )
        }),
    );
    paths.insert(
        "/v1/accounts/{account_id}/assets/query".to_owned(),
        Value::Object(json_post_operation(
            "Accounts",
            "Query account assets.",
            "Query assets held by an account. Results are ingress-independent and merged across the caller-visible dataspaces; balances remain separated by their existing `scope` values instead of being summed across dataspaces.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/AccountAssetQueryResponse",
            vec![string_path_param(
                "account_id",
                "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
            )],
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/permissions".to_owned(),
        Value::Object(json_get_operation(
            "Accounts",
            "List account permissions.",
            "List permissions granted to an account. Signed/internal reads fan out across all configured dataspaces so dataspace-scoped grants remain visible regardless of ingress; unsigned reads stay limited to caller-visible routes.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "account_id",
                "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
            )],
        )),
    );
    paths.insert(
        "/v1/accounts/{account_id}/transactions".to_owned(),
        Value::Object({
            let mut params = vec![string_path_param(
                "account_id",
                "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
            )];
            params.extend(account_transactions_list_query_parameters());
            json_get_operation(
                "Accounts",
                "List account transactions.",
                "List transactions authored by an account (supports pagination and optional asset_id filtering). Results are merged across the caller-visible dataspaces only; private dataspace history the caller cannot read is silently omitted.",
                "#/components/schemas/JsonValue",
                params,
            )
        }),
    );
    paths.insert(
        "/v1/accounts/{uaid}/portfolio".to_owned(),
        Value::Object({
            let mut params = vec![string_path_param("uaid", "User account identifier.")];
            params.push(string_query_param(
                "asset_id",
                "Filter portfolio positions by exact canonical Base58 asset id.",
            ));
            json_get_operation(
                "Accounts",
                "Fetch account portfolio.",
                "Fetch the asset portfolio for an account identifier (supports optional `asset_id` filtering).",
                "#/components/schemas/JsonValue",
                params,
            )
        }),
    );
    paths
}

fn identifier_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/identifier-policies".to_owned(),
        Value::Object(json_get_operation(
            "Identifiers",
            "List identifier policies.",
            "List globally registered hidden-function identifier policies and their public metadata.",
            "#/components/schemas/IdentifierPolicyListResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/identifiers/resolve".to_owned(),
        Value::Object(json_post_operation(
            "Identifiers",
            "Resolve an identifier.",
            "Resolve an encrypted identifier input under a hidden-function policy using a verified RAM-LFE output opening, then return the bound canonical account target when a live claim exists.",
            "#/components/schemas/IdentifierResolveRequest",
            "#/components/schemas/IdentifierResolveResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/identifiers/receipts/{receipt_hash}".to_owned(),
        Value::Object({
            let params = vec![string_path_param(
                "receipt_hash",
                "Hidden-function receipt hash used to look up the persisted claim binding.",
            )];
            json_get_operation(
                "Identifiers",
                "Look up an identifier claim by receipt hash.",
                "Resolve a persisted identifier claim by its deterministic receipt hash for audit and support tooling.",
                "#/components/schemas/IdentifierClaimLookupResponse",
                params,
            )
        }),
    );
    paths.insert(
        "/v1/accounts/{account_id}/identifiers/claim-receipt".to_owned(),
        Value::Object({
            let params = vec![string_path_param(
                "account_id",
                "Canonical target account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace` for the claim receipt.",
            )];
            json_post_operation(
                "Identifiers",
                "Issue an identifier claim receipt.",
                "Issue an attested identifier claim receipt from encrypted input and a verified RAM-LFE output opening that can be embedded into `ClaimIdentifier`.",
                "#/components/schemas/IdentifierResolveRequest",
                "#/components/schemas/IdentifierResolveResponse",
                params,
            )
        }),
    );
    paths
}

fn ram_lfe_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/ram-lfe/program-policies".to_owned(),
        Value::Object(json_get_operation(
            "RamLfe",
            "List RAM-LFE program policies.",
            "List globally registered RAM-LFE program policies and their public execution metadata.",
            "#/components/schemas/RamLfeProgramPolicyListResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/ram-lfe/programs/{program_id}/execute".to_owned(),
        Value::Object({
            let params = vec![string_path_param(
                "program_id",
                "RAM-LFE program identifier.",
            )];
            json_post_operation(
                "RamLfe",
                "Execute a RAM-LFE program.",
                "Execute a RAM-LFE program from a BFV ciphertext envelope and return the encrypted output ciphertext plus stateless execution receipt.",
                "#/components/schemas/RamLfeExecuteRequest",
                "#/components/schemas/RamLfeExecuteResponse",
                params,
            )
        }),
    );
    paths.insert(
        "/v1/ram-lfe/receipts/verify".to_owned(),
        Value::Object(json_post_operation(
            "RamLfe",
            "Verify a RAM-LFE receipt.",
            "Validate a stateless RAM-LFE execution receipt against the published on-chain program policy and optionally compare a caller-supplied output blob to the receipt output hash.",
            "#/components/schemas/RamLfeReceiptVerifyRequest",
            "#/components/schemas/RamLfeReceiptVerifyResponse",
            Vec::new(),
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
            "#/components/schemas/DomainListResponse",
            app_list_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/domains/query".to_owned(),
        Value::Object(json_post_operation(
            "Domains",
            "Query domains.",
            "Query domains with JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/DomainQueryResponse",
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
            "List asset definitions as full objects with canonical Base58 id, optional alias, and `alias_binding` lease metadata when an alias exists.",
            "#/components/schemas/AssetDefinitionListResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/assets/definitions/{asset}".to_owned(),
        Value::Object(json_get_operation(
            "Assets",
            "Fetch one asset definition.",
            "Fetch an asset definition by unprefixed Base58 id or `<name>#<domain>.<dataspace>` / `<name>#<dataspace>` alias, including `alias_binding` lease metadata when present.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "asset",
                "Asset selector (unprefixed Base58 id or alias).",
            )],
        )),
    );
    paths.insert(
        "/v1/assets/definitions/query".to_owned(),
        Value::Object(json_post_operation(
            "Assets",
            "Query asset definitions.",
            "Query asset definitions with a JSON envelope and full asset-definition objects in the response, including `alias_binding` lease metadata when present.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/AssetDefinitionQueryResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/assets/{definition_id}/holders".to_owned(),
        Value::Object({
            let mut params = vec![string_path_param(
                "definition_id",
                "Asset selector (unprefixed Base58 id or alias).",
            )];
            params.extend(asset_holders_list_query_parameters());
            json_get_operation(
                "Assets",
                "List asset holders.",
                "List holders for an asset definition (supports pagination plus optional account_id and scope filtering).",
                "#/components/schemas/AssetHolderListResponse",
                params,
            )
        }),
    );
    paths.insert(
        "/v1/assets/{definition_id}/holders/query".to_owned(),
        Value::Object(json_post_operation(
            "Assets",
            "Query asset holders.",
            "Query holders for an asset definition. `account_id` filters accept canonical I105 account ids or on-chain aliases in `name@domain.dataspace` / `name@dataspace` form. Aggregate mode supports exact PAYNET-style PKR directory queries such as grouping by `primary_alias_domain` with `distinct_count(account_id)` and `sum(quantity)`. In production aggregate mode Torii serves published DA projection shards from local cache/storage (`query_source=projection_da_cache`) and hydrates missing shards from approved SoraFS providers on demand (`query_source=projection_da_hydrated`). Incomplete projections return `projection_archive_unavailable` instead of scanning live holders. `live_debug` requires an explicit debug opt-in.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/AssetHolderQueryResponse",
            vec![string_path_param(
                "definition_id",
                "Asset selector (unprefixed Base58 id or alias).",
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
                "Asset selector (unprefixed Base58 id or alias).",
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
            "#/components/schemas/NftListResponse",
            app_list_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/nfts/query".to_owned(),
        Value::Object(json_post_operation(
            "NFTs",
            "Query NFTs.",
            "Query NFTs with JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/NftQueryResponse",
            Vec::new(),
        )),
    );
    paths
}

fn rwa_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/rwas".to_owned(),
        Value::Object(json_get_operation(
            "RWAs",
            "List RWA lots.",
            "List RWA lots visible to the caller.",
            "#/components/schemas/RwaListResponse",
            app_list_query_parameters(),
        )),
    );
    paths.insert(
        "/v1/rwas/query".to_owned(),
        Value::Object(json_post_operation(
            "RWAs",
            "Query RWA lots.",
            "Query RWA lots with JSON envelope.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/RwaQueryResponse",
            Vec::new(),
        )),
    );
    paths
}

fn subscription_paths() -> Map {
    let mut paths = Map::new();
    let plan_query_params = vec![
        string_query_param(
            "provider",
            "Filter plans by provider account id (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`).",
        ),
        integer_query_param("limit", "Optional page size limit.", Some("uint64")),
        integer_query_param(
            "offset",
            "Optional starting offset (default 0).",
            Some("uint64"),
        ),
    ];
    let plans = json_get_operation(
        "Subscriptions",
        "List subscription plans.",
        "List subscription plans by provider.",
        "#/components/schemas/JsonValue",
        plan_query_params,
    );
    paths.insert("/v1/subscriptions/plans".to_owned(), Value::Object(plans));

    let subscription_query_params = vec![
        string_query_param(
            "owned_by",
            "Filter subscriptions by subscriber account id (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`).",
        ),
        string_query_param(
            "provider",
            "Filter subscriptions by provider account id (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`).",
        ),
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
    let subs = json_get_operation(
        "Subscriptions",
        "List subscriptions.",
        "List subscriptions with optional filters.",
        "#/components/schemas/JsonValue",
        subscription_query_params,
    );
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
        Value::Object({
            let params = explorer_assets_query_parameters();
            json_get_operation(
                "Explorer",
                "List assets (explorer).",
                "List assets for explorer usage (supports pagination and optional owned_by/definition/asset_id filtering).",
                "#/components/schemas/JsonValue",
                params,
            )
        }),
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
        "/v1/explorer/rwas".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "List RWAs (explorer).",
            "List RWA lots for explorer usage.",
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
        "/v1/explorer/blocks/stream".to_owned(),
        Value::Object(event_stream_get_operation(
            "Explorer",
            "Stream blocks (explorer).",
            "Stream block updates via SSE.",
        )),
    );
    paths.insert(
        "/v1/explorer/transactions".to_owned(),
        Value::Object({
            let params = explorer_transactions_query_parameters();
            json_get_operation(
                "Explorer",
                "List transactions (explorer).",
                "List transactions for explorer usage (supports pagination and optional authority/block/status/asset_id filtering).",
                "#/components/schemas/JsonValue",
                params,
            )
        }),
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
        Value::Object({
            let params = explorer_instructions_query_parameters();
            json_get_operation(
                "Explorer",
                "List instructions (explorer).",
                "List instructions for explorer usage (supports pagination and optional account/authority/transaction/block/kind/asset_id filtering).",
                "#/components/schemas/JsonValue",
                params,
            )
        }),
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
            vec![string_path_param(
                "account_id",
                "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
            )],
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
                Value::Array(vec![string_path_param(
                    "account_id",
                    "Canonical I105 account identifier or on-chain alias `name@domain.dataspace` / `name@dataspace`.",
                )]),
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
        "/v1/explorer/asset-definitions/{definition_id}/econometrics".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch asset definition econometrics (explorer).",
            "Fetch econometrics aggregates (velocity/issuance windows) for an asset definition.",
            "#/components/schemas/JsonValue",
            vec![string_path_param(
                "definition_id",
                "Asset definition identifier.",
            )],
        )),
    );
    paths.insert(
        "/v1/explorer/asset-definitions/{definition_id}/snapshot".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch asset definition snapshot (explorer).",
            "Fetch econometrics snapshot (holders + distribution metrics) for an asset definition.",
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
            vec![string_path_param("asset_id", "Canonical Base58 asset id.")],
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
        "/v1/explorer/rwas/{rwa_id}".to_owned(),
        Value::Object(json_get_operation(
            "Explorer",
            "Fetch RWA detail (explorer).",
            "Fetch RWA detail for explorer usage.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("rwa_id", "RWA identifier.")],
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
        "/v1/sorafs/pin/register".to_owned(),
        Value::Object(json_post_operation(
            "SoraFS",
            "Register paid pin manifest.",
            "Submit a SoraFS manifest registration transaction. The request must include `content_length`; execution collects the public pin fee and records the committed fee receipt on-chain.",
            "#/components/schemas/JsonValue",
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
            "Submit a storage pin request. The manifest must already have an approved on-chain paid pin record matching the digest, chunker profile, policy, content length, chunk plan digest, and fee receipt payer.",
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
            "Request a PoR or PoTR proof stream payload. `proof_kind=pdp` is reserved for future SF-13 work and is rejected as an unsupported proof kind.",
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

fn soracloud_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/soracloud/model/upload/private/execute".to_owned(),
        Value::Object(json_post_operation(
            "Soracloud",
            "Execute a private uploaded model.",
            "Run the deterministic quantized CPU V1 private uploaded-model evaluator against an approved finalized SoraFS-backed bundle. The response contains only commitments, encrypted artifact references, and an unsigned `RecordSoracloudPrivateUploadedModelExecutionReceipt` instruction skeleton for external signing.",
            "#/components/schemas/PrivateUploadedModelExecuteRequest",
            "#/components/schemas/PrivateUploadedModelExecuteResponse",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/soracloud/model/upload/private/receipts".to_owned(),
        Value::Object(json_get_operation(
            "Soracloud",
            "List private uploaded-model receipts.",
            "List committed private uploaded-model execution receipts. `count_mode=bounded` is the default; `count_mode=exact` opts into an exact total.",
            "#/components/schemas/PrivateUploadedModelReceiptListResponse",
            vec![
                string_query_param("receipt_id", "Optional receipt hash filter."),
                string_query_param("service_name", "Optional Soracloud service name filter."),
                string_query_param("model_id", "Optional uploaded model id filter."),
                string_query_param("weight_version", "Optional model weight version filter."),
                integer_query_param("limit", "Optional page size limit.", Some("uint32")),
                string_query_param(
                    "count_mode",
                    "Count mode: `bounded` by default, or `exact` for exact totals.",
                ),
            ],
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
(`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`, `X-Iroha-Nonce`)."
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
        "/v1/sns/names".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Register a SNS name lease.",
            "Register a ledger-backed SNS name record in one of the fixed namespaces.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sns/names/{namespace}/{literal}".to_owned(),
        Value::Object(json_get_operation(
            "SNS",
            "Fetch a SNS name record.",
            "Fetch a ledger-backed SNS name record by namespace and canonical literal.",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param(
                    "namespace",
                    "SNS namespace (`account-alias`, `domain`, or `dataspace`).",
                ),
                string_path_param("literal", "Canonical SNS literal for the namespace."),
            ],
        )),
    );
    paths.insert(
        "/v1/sns/names/{namespace}/{literal}/renew".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Renew a SNS name record.",
            "Renew a ledger-backed SNS name record by namespace and canonical literal.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param(
                    "namespace",
                    "SNS namespace (`account-alias`, `domain`, or `dataspace`).",
                ),
                string_path_param("literal", "Canonical SNS literal for the namespace."),
            ],
        )),
    );
    paths.insert(
        "/v1/sns/names/{namespace}/{literal}/transfer".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Transfer a SNS name record.",
            "Transfer a ledger-backed SNS name record by namespace and canonical literal.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param(
                    "namespace",
                    "SNS namespace (`account-alias`, `domain`, or `dataspace`).",
                ),
                string_path_param("literal", "Canonical SNS literal for the namespace."),
            ],
        )),
    );
    paths.insert(
        "/v1/sns/names/{namespace}/{literal}/controllers".to_owned(),
        Value::Object(json_post_operation(
            "SNS",
            "Update SNS controllers.",
            "Update controllers for a ledger-backed SNS name record.",
            "#/components/schemas/JsonValue",
            "#/components/schemas/JsonValue",
            vec![
                string_path_param(
                    "namespace",
                    "SNS namespace (`account-alias`, `domain`, or `dataspace`).",
                ),
                string_path_param("literal", "Canonical SNS literal for the namespace."),
            ],
        )),
    );
    paths.insert(
        "/v1/sns/names/{namespace}/{literal}/freeze".to_owned(),
        Value::Object({
            let post_op = json_post_operation(
                "SNS",
                "Freeze a SNS name record.",
                "Freeze a ledger-backed SNS name record.",
                "#/components/schemas/JsonValue",
                "#/components/schemas/JsonValue",
                vec![
                    string_path_param(
                        "namespace",
                        "SNS namespace (`account-alias`, `domain`, or `dataspace`).",
                    ),
                    string_path_param("literal", "Canonical SNS literal for the namespace."),
                ],
            );
            let delete_op = json_delete_operation(
                "SNS",
                "Unfreeze a SNS name record.",
                "Unfreeze a ledger-backed SNS name record.",
                "#/components/schemas/JsonValue",
                vec![
                    string_path_param(
                        "namespace",
                        "SNS namespace (`account-alias`, `domain`, or `dataspace`).",
                    ),
                    string_path_param("literal", "Canonical SNS literal for the namespace."),
                ],
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
    fn push_device_operation(summary: &str, description: &str, accepted: &str) -> Map {
        let mut operation = Map::new();
        operation.insert(
            "tags".into(),
            Value::Array(vec![Value::String("Push".to_owned())]),
        );
        operation.insert("summary".into(), Value::String(summary.to_owned()));
        operation.insert("description".into(), Value::String(description.to_owned()));
        operation.insert(
            "requestBody".into(),
            Value::Object(json_request_body(
                "#/components/schemas/PushRegisterDeviceRequest",
            )),
        );
        let mut responses = Map::new();
        let mut accepted_response = Map::new();
        accepted_response.insert("description".into(), Value::String(accepted.to_owned()));
        responses.insert("202".to_owned(), Value::Object(accepted_response));
        responses.insert(
            "400".to_owned(),
            json_response("Invalid device request.", error_schema_reference()),
        );
        responses.insert(
            "401".to_owned(),
            json_response(
                "Canonical request signature is required or invalid.",
                error_schema_reference(),
            ),
        );
        responses.insert(
            "403".to_owned(),
            json_response(
                "Signed account does not match the device account.",
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
                "Push provider is disabled or credentials are unavailable.",
                error_schema_reference(),
            ),
        );
        operation.insert("responses".into(), Value::Object(responses));
        operation
    }

    let mut paths = Map::new();
    paths.insert(
        "/v1/notify/devices".to_owned(),
        Value::Object({
            let mut methods = Map::new();
            methods.insert(
                "post".to_owned(),
                Value::Object(push_device_operation(
                    "Register a push device.",
                    "Register a device token for push notifications. The JSON body is signed with canonical request headers; X-Iroha-Account must resolve to the same canonical account as account_id.",
                    "Device registration accepted.",
                )),
            );
            methods.insert(
                "delete".to_owned(),
                Value::Object(push_device_operation(
                    "Unregister a push device.",
                    "Unregister a device token using the same signed JSON body shape. Tokens stay out of URLs and logs.",
                    "Device unregistration accepted.",
                )),
            );
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
    paths.insert(
        "/v1/nexus/dataspaces/accounts/{literal}/summary".to_owned(),
        Value::Object(nexus_dataspaces_account_summary_operation()),
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
            "Return complete-delivery RBC status for a block height and view; delivered=true requires a non-invalid positive complete chunk summary.",
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
        "/v1/sccp/capabilities".to_owned(),
        Value::Object(sccp_capabilities_operation()),
    );
    paths.insert(
        "/v1/sccp/manifests".to_owned(),
        Value::Object(sccp_manifests_operation()),
    );
    paths.insert(
        "/v1/sccp/artifacts/message/{message_id}".to_owned(),
        Value::Object(sccp_message_artifact_operation()),
    );
    paths.insert(
        "/v1/sccp/jobs/message/{message_id}".to_owned(),
        Value::Object(sccp_message_job_operation()),
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
            "Return RBC session snapshots, including raw DELIVER and complete-delivery status.",
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
            "Submit an authenticated VRF commit payload.",
            "#/components/schemas/SumeragiVrfCommitRequest",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/vrf/reveal".to_owned(),
        Value::Object(json_post_operation(
            "Sumeragi",
            "Submit VRF reveal.",
            "Submit an authenticated VRF reveal payload.",
            "#/components/schemas/SumeragiVrfRevealRequest",
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

fn sccp_capabilities_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Bridge".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Discover SCCP proof capabilities.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the local SCCP domain id, legacy proof backends, generic message proof \
             family, supported codecs, and the per-counterparty backend labels relayers should \
             use for the current launch networks: Ethereum, BSC, Solana, TON, and Tron."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sccpCapabilities".to_owned()),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json_response("SCCP capability snapshot.", schema_ref("JsonValue")),
    );
    responses.insert("406".into(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn sccp_manifests_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Bridge".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Discover SCCP proof manifests.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the typed per-counterparty SCCP proof manifests used to derive chain-specific \
             backend labels, verifier targets, finality models, public inputs, and manifest seeds \
             for the current launch networks: Ethereum, BSC, Solana, TON, and Tron."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sccpProofManifests".to_owned()),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json_response("SCCP proof manifest collection.", schema_ref("JsonValue")),
    );
    responses.insert("406".into(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn sccp_message_artifact_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Bridge".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch a typed SCCP transparent proof artifact.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the typed transparent SCCP proof artifact for a canonical message id, \
             including the chain profile, public inputs, verifier-backend metadata, generated \
             counterparty submission package, real proof bytes, and embedded Nexus message bundle. \
             Supports JSON or Norito content negotiation via the `Accept` header."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sccpMessageArtifact".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![string_path_param(
            "message_id",
            "Canonical SCCP message id hex string.",
        )]),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json_response(
            "Typed SCCP transparent proof artifact.",
            schema_ref("JsonValue"),
        ),
    );
    responses.insert(
        "404".into(),
        json_response("SCCP message not found.", error_schema_reference()),
    );
    responses.insert("406".into(), not_acceptable_response());
    operation.insert("responses".into(), Value::Object(responses));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn sccp_message_job_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Bridge".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Fetch a normalized SCCP counterparty proof job.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns a prover-oriented SCCP job for a canonical message id, including the \
             chain-specific normalized payload projection plus the original typed Nexus message \
             bundle. Supports JSON or Norito content negotiation via the `Accept` header."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("sccpMessageJob".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![string_path_param(
            "message_id",
            "Canonical SCCP message id hex string.",
        )]),
    );
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json_response(
            "Normalized SCCP counterparty proof job.",
            schema_ref("JsonValue"),
        ),
    );
    responses.insert(
        "404".into(),
        json_response("SCCP message not found.", error_schema_reference()),
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
        Value::String("Returns the active repo agreements recorded on-chain with optional pagination and filtering.".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![
            query_param("limit", "integer", "Optional maximum number of entries to return."),
            query_param("offset", "integer", "Zero-based pagination offset (default 0)."),
            query_param("sort", "string", "Optional comma-separated sort expression (e.g., `id:asc,maturity_timestamp_ms:desc`)."),
            query_param("filter", "string", "JSON-encoded FilterExpr limiting results (fields: id, initiator, counterparty, custodian)."),
            query_param("count_mode", "string", "Count mode: `bounded` omits exact totals; `exact` includes an exact `total`."),
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
        Value::String("Structured query endpoint accepting a JSON envelope with pagination, sort, and filter fields.".to_owned()),
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

fn single_dual_format_response(schema_ref: &str) -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        norito::json!({
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": { "$ref": schema_ref }
                },
                "application/x-norito": {
                    "schema": { "$ref": schema_ref }
                }
            }
        }),
    );
    responses
}

fn dual_format_response(description: &str, schema_ref: &str) -> Value {
    norito::json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": { "$ref": schema_ref }
            },
            "application/x-norito": {
                "schema": { "$ref": schema_ref }
            }
        }
    })
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

fn json_or_norito_request_body(schema_ref: &str) -> Map {
    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert(
        "content".into(),
        norito::json!({
            "application/json": {
                "schema": { "$ref": schema_ref }
            },
            "application/x-norito": {
                "schema": {
                    "type": "string",
                    "format": "binary"
                }
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

fn versioned_dual_format_post_operation(
    tag: &str,
    summary: &str,
    description: &str,
    request_schema_ref: &str,
    response_schema_ref: &str,
    success_status: u16,
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
        Value::Object(json_or_norito_request_body(request_schema_ref)),
    );
    let mut responses = Map::new();
    responses.insert(
        success_status.to_string(),
        dual_format_response("Successful response", response_schema_ref),
    );
    responses.insert(
        "400".to_owned(),
        dual_format_response("Bad request", "#/components/schemas/ErrorEnvelope"),
    );
    responses.insert(
        "406".to_owned(),
        dual_format_response(
            "Requested content type is not acceptable; supported: application/json, application/x-norito.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    responses.insert(
        "415".to_owned(),
        dual_format_response(
            "Unsupported request content type.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    operation.insert("responses".into(), Value::Object(responses));
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
        dual_format_response(
            "Relay summaries retrieved.",
            "#/components/schemas/KaigiRelaySummaryList",
        ),
    );
    responses.insert(
        "503".to_owned(),
        dual_format_response(
            "Telemetry profile does not permit relay telemetry.",
            "#/components/schemas/ErrorEnvelope",
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
             and `release_at_ms` when an exit timer is running."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("nexusPublicLaneValidators".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![Value::Object(lane_id_parameter())]),
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
             including pending unbond timers. Supports an optional validator filter."
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
            string_query_param(
                "validator",
                "Optional validator account literal to filter stake entries (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`).",
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
             given public lane. Requires an `account` query parameter (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`) and \
             accepts optional `asset_id` and `upto_epoch` filters."
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
            string_query_param(
                "account",
                "Account literal to evaluate pending rewards for (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`).",
            ),
            string_query_param("asset_id", "Filter pending rewards by canonical Base58 asset id."),
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

fn nexus_dataspaces_account_summary_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Nexus".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Summarize dataspaces for an account literal.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Resolves the supplied canonical I105 account literal or on-chain alias \
             and returns a joined view of UAID bindings, space-directory manifests, \
             portfolio counters, and per-dataspace consensus commitments."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("nexusDataspacesAccountSummary".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(vec![string_path_param(
            "literal",
            "Account literal to resolve (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`).",
        )]),
    );
    operation.insert(
        "responses".into(),
        Value::Object(nexus_dataspaces_account_summary_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn nexus_dataspaces_account_summary_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response("Dataspace summary retrieved.", schema_ref("JsonValue")),
    );
    responses.insert(
        "400".to_owned(),
        json_response("Invalid account literal.", error_schema_reference()),
    );
    responses.insert(
        "404".to_owned(),
        json_response("Account not found.", error_schema_reference()),
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
        Value::String(
            "Relay account identifier encoded as a canonical I105 account literal or on-chain alias `name@domain.dataspace` / `name@dataspace`."
                .to_owned(),
        ),
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
        dual_format_response(
            "Relay detail retrieved.",
            "#/components/schemas/KaigiRelayDetail",
        ),
    );
    responses.insert(
        "400".to_owned(),
        dual_format_response(
            "Invalid relay identifier.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    responses.insert(
        "404".to_owned(),
        dual_format_response("Relay not found.", "#/components/schemas/ErrorEnvelope"),
    );
    responses.insert(
        "503".to_owned(),
        dual_format_response(
            "Telemetry profile does not permit relay telemetry.",
            "#/components/schemas/ErrorEnvelope",
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
        dual_format_response(
            "Relay health snapshot retrieved.",
            "#/components/schemas/KaigiRelayHealthSnapshot",
        ),
    );
    responses.insert(
        "503".to_owned(),
        dual_format_response(
            "Telemetry profile does not permit relay telemetry.",
            "#/components/schemas/ErrorEnvelope",
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
    paths.extend(vpn_paths());
    paths.extend(mcp_paths());
    paths.extend(proof_paths());
    paths.extend(contracts_paths());
    paths.extend(multisig_paths());
    paths.extend(controls_paths());
    paths.extend(zk_paths());
    paths.extend(governance_paths());
    paths.extend(runtime_paths());
    paths.extend(account_paths());
    paths.extend(ram_lfe_paths());
    paths.extend(identifier_paths());
    paths.extend(domain_paths());
    paths.extend(asset_paths());
    paths.extend(nft_paths());
    paths.extend(rwa_paths());
    paths.extend(subscription_paths());
    paths.extend(parameter_paths());
    paths.extend(space_directory_paths());
    paths.extend(explorer_paths());
    paths.extend(sorafs_paths());
    paths.extend(soracloud_paths());
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
    annotate_tool_effects(&mut paths);
    paths
}

fn annotate_tool_effects(paths: &mut Map) {
    for (path, path_item) in paths {
        let Some(path_map) = path_item.as_object_mut() else {
            continue;
        };
        for method in ["get", "post", "put", "patch", "delete", "head", "options"] {
            let Some(operation) = path_map.get_mut(method).and_then(Value::as_object_mut) else {
                continue;
            };
            let effect = operation_effect(method, path);
            operation.insert(
                TOOL_EFFECT_EXTENSION.into(),
                Value::String(effect.to_owned()),
            );
        }
    }
}

fn operation_effect(method: &str, path: &str) -> &'static str {
    if is_operator_operation(method, path) {
        return "operator";
    }
    if is_read_operation(method, path) {
        return "read";
    }
    "write"
}

fn is_operator_operation(method: &str, path: &str) -> bool {
    if method == "get" {
        return false;
    }
    path.starts_with("/v1/operator/")
        || matches!(
            path,
            uri::CONFIGURATION
                | "/v1/internal/torii/proxy"
                | "/v1/nexus/lane-lifecycle"
                | "/v1/sumeragi/evidence"
                | "/v1/sumeragi/vrf/commit"
                | "/v1/sumeragi/vrf/reveal"
                | "/v1/sumeragi/rbc/sample"
                | "/v1/gov/protected-namespaces"
        )
}

fn is_read_operation(method: &str, path: &str) -> bool {
    matches!(method, "get" | "head" | "options")
        || (method == "post"
            && matches!(
                path,
                uri::QUERY
                    | "/v1/accounts/query"
                    | "/v1/aliases/by_account"
                    | "/v1/aliases/resolve"
                    | "/v1/aliases/resolve_index"
                    | "/v1/assets/aliases/resolve"
                    | "/v1/assets/definitions/query"
                    | "/v1/assets/holders/query"
                    | "/v1/assets/query"
                    | "/v1/contracts/aliases/resolve"
                    | "/v1/contracts/view"
                    | "/v1/contracts/view/batch"
                    | "/v1/controls/asset-transfer/get"
                    | "/v1/da/commitments"
                    | "/v1/da/commitments/prove"
                    | "/v1/da/commitments/verify"
                    | "/v1/da/pin_intents"
                    | "/v1/da/pin_intents/prove"
                    | "/v1/da/pin_intents/verify"
                    | "/v1/domains/query"
                    | "/v1/multisig/approvals/get"
                    | "/v1/multisig/approvals/get_for_authority"
                    | "/v1/multisig/approvals/list"
                    | "/v1/multisig/approvals/list_for_authority"
                    | "/v1/multisig/proposals/get"
                    | "/v1/multisig/proposals/list"
                    | "/v1/multisig/spec"
                    | "/v1/nfts/query"
                    | "/v1/proofs/query"
                    | "/v1/rwas/query"
                    | "/v1/pipeline/transactions/status"
                    | "/v1/zk/roots"
                    | "/v1/zk/verify"
                    | "/v1/zk/verify-batch"
                    | "/v1/zk/vote/tally"
            ))
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
            "Routes account-alias resolution through the Nexus read proxy using the alias \
             dataspace, then returns the canonical alias spelling, the bound account \
             identifier, and (if known) the deterministic alias index. Unsigned/public \
             requests remain allowed for ordinary public alias lookups."
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
        "409".to_owned(),
        json_response(
            "Alias resolves to a non-account target, which this endpoint cannot return.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "403".to_owned(),
        json_response(
            "Alias lookup was denied by the routed dataspace and no allowed route resolved it.",
            error_schema_reference(),
        ),
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
            "Fans out the alias-index lookup across configured Nexus dataspaces because the \
             index alone does not encode a dataspace, dedupes identical bindings, and returns \
             `source = fanout` when the result comes from multi-route merging."
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

fn asset_alias_resolve_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Aliases".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Resolve an asset alias into canonical asset definition fields.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns the canonical Base58 asset definition id, human-readable fields, and `alias_binding` lease metadata for `<name>#<domain>.<dataspace>` or `<name>#<dataspace>` aliases."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("assetAliasResolve".to_owned()),
    );
    operation.insert("requestBody".into(), asset_alias_resolve_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(asset_alias_resolve_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn alias_lookup_by_account_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Aliases".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List aliases bound to an account across reachable dataspaces.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Routes the lookup through the Nexus read proxy using the target account routes, \
             merges deduplicated alias rows across reachable dataspaces, recomputes `total`, \
             and returns `source = fanout` when multiple routes contribute to the response."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("aliasLookupByAccount".to_owned()),
    );
    operation.insert("requestBody".into(), alias_lookup_by_account_request_body());
    operation.insert(
        "responses".into(),
        Value::Object(alias_lookup_by_account_responses()),
    );
    let mut methods = Map::new();
    methods.insert("post".to_owned(), Value::Object(operation));
    methods
}

fn asset_alias_resolve_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert("schema".into(), schema_ref("AssetAliasResolveRequest"));
            schema
        }),
    );

    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn asset_alias_resolve_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Asset alias successfully resolved.",
            schema_ref("AssetAliasResolveResponse"),
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
        json_response("Asset alias not found.", error_schema_reference()),
    );
    responses.insert(
        "409".to_owned(),
        json_response(
            "Asset alias is ambiguously bound to multiple definitions.",
            error_schema_reference(),
        ),
    );
    responses
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
        "403".to_owned(),
        json_response(
            "Alias-index lookup was denied by one or more routed dataspaces and no allowed route resolved it.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response("Alias index not found.", error_schema_reference()),
    );
    responses.insert(
        "409".to_owned(),
        json_response(
            "Multiple dataspaces returned conflicting alias-index bindings, or the matched alias index resolves to a non-account target.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "503".to_owned(),
        json_response("Alias runtime unavailable.", error_schema_reference()),
    );
    responses
}

fn alias_lookup_by_account_request_body() -> Value {
    let mut media = Map::new();
    media.insert(
        "application/json".into(),
        Value::Object({
            let mut schema = Map::new();
            schema.insert("schema".into(), schema_ref("AliasLookupByAccountRequest"));
            schema
        }),
    );

    let mut body = Map::new();
    body.insert("required".into(), Value::Bool(true));
    body.insert("content".into(), Value::Object(media));
    Value::Object(body)
}

fn alias_lookup_by_account_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        json_response(
            "Alias list successfully resolved.",
            schema_ref("AliasLookupByAccountResponse"),
        ),
    );
    responses.insert(
        "400".to_owned(),
        json_response(
            "Malformed request (account id or filters are invalid).",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "403".to_owned(),
        json_response(
            "Alias lookup was denied by one or more routed dataspaces and no allowed route returned aliases.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "404".to_owned(),
        json_response("Account alias list not found.", error_schema_reference()),
    );
    responses.insert(
        "409".to_owned(),
        json_response(
            "Multiple dataspaces returned conflicting alias-account roots.",
            error_schema_reference(),
        ),
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
        Value::String("Fetch a DA commitment Merkle proof.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Locates a DA commitment by manifest or lane/epoch/sequence and returns a Merkle inclusion proof for the indexed block commitment bundle."
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
            "Validates a provided DA commitment Merkle proof against the node's indexed bundle, block header commitment hash, and configured lane proof policy."
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

fn string_header_param(name: &str, description: &str, required: bool) -> Value {
    let mut param = Map::new();
    param.insert("name".into(), Value::String(name.to_owned()));
    param.insert("in".into(), Value::String("header".to_owned()));
    param.insert("required".into(), Value::Bool(required));
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

fn versioned_json_payload_schema(content_description: &str) -> Value {
    norito::json!({
        "type": "object",
        "required": ["version", "content"],
        "additionalProperties": false,
        "properties": {
            "version": {
                "oneOf": [
                    { "type": "integer", "enum": [1] },
                    { "type": "string", "enum": ["1"] }
                ],
                "description": "Versioned JSON payload version."
            },
            "content": {
                "$ref": "#/components/schemas/JsonValue",
                "description": content_description
            }
        }
    })
}

fn app_page_schema(item_schema_ref: &str) -> Value {
    let mut schema = norito::json!({
        "allOf": [
            { "$ref": "#/components/schemas/AppPageMetadata" },
            {
                "type": "object",
                "required": ["items"],
                "properties": {
                    "items": {
                        "type": "array",
                        "description": "Current result page."
                    }
                }
            }
        ]
    });
    let items = schema
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .and_then(|all_of| all_of.get_mut(1))
        .and_then(Value::as_object_mut)
        .and_then(|object| object.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut("items"))
        .and_then(Value::as_object_mut)
        .expect("app page schema items property exists");
    items.insert(
        "items".into(),
        norito::json!({
            "$ref": item_schema_ref
        }),
    );
    schema
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
        "AppPageMetadata".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["items", "has_more", "count_mode"],
            "additionalProperties": true,
            "properties": {
                "total": {
                    "type": ["integer", "null"],
                    "format": "uint64",
                    "minimum": 0,
                    "description": "Exact number of matching rows when `count_mode` is `exact`; omitted or null for bounded count mode."
                },
                "has_more": {
                    "type": "boolean",
                    "description": "Whether a later page is available from the same query."
                },
                "count_mode": {
                    "type": "string",
                    "enum": ["bounded", "exact"],
                    "description": "Actual count mode used by Torii for this response."
                },
                "indexed_height": {
                    "type": "integer",
                    "format": "uint64",
                    "minimum": 0,
                    "description": "Indexed block height for generic/projection query responses when present."
                },
                "indexed_block_hash": {
                    "type": ["string", "null"],
                    "description": "Indexed block hash for generic/projection query responses when present."
                },
                "query_source": {
                    "type": "string",
                    "description": "Backend source label for generic/projection query responses when present."
                }
            }
        }),
    );
    schemas.insert(
        "AccountListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["id"],
            "additionalProperties": true,
            "properties": {
                "id": { "type": "string" },
                "primary_alias": { "type": ["string", "null"] },
                "primary_alias_name": { "type": ["string", "null"] },
                "primary_alias_dataspace": { "type": ["string", "null"] },
                "primary_alias_domain": { "type": ["string", "null"] },
                "has_primary_alias": { "type": "boolean" }
            }
        }),
    );
    schemas.insert(
        "AccountListResponse".to_owned(),
        app_page_schema("#/components/schemas/AccountListItem"),
    );
    schemas.insert(
        "AccountQueryResponse".to_owned(),
        app_page_schema("#/components/schemas/AccountListItem"),
    );
    schemas.insert(
        "DomainListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["id"],
            "additionalProperties": true,
            "properties": {
                "id": { "type": "string" },
                "owned_by": { "type": ["string", "null"] },
                "metadata": { "$ref": "#/components/schemas/JsonValue" }
            }
        }),
    );
    schemas.insert(
        "DomainListResponse".to_owned(),
        app_page_schema("#/components/schemas/DomainListItem"),
    );
    schemas.insert(
        "DomainQueryResponse".to_owned(),
        app_page_schema("#/components/schemas/DomainListItem"),
    );
    schemas.insert(
        "AccountAssetListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["asset", "quantity"],
            "additionalProperties": true,
            "properties": {
                "account_id": { "type": "string" },
                "asset": { "type": "string" },
                "asset_id": { "type": "string" },
                "asset_name": { "type": ["string", "null"] },
                "asset_alias": { "type": ["string", "null"] },
                "scope": { "type": "string" },
                "quantity": { "type": "string" },
                "primary_alias": { "type": ["string", "null"] },
                "primary_alias_name": { "type": ["string", "null"] },
                "primary_alias_dataspace": { "type": ["string", "null"] },
                "primary_alias_domain": { "type": ["string", "null"] },
                "has_primary_alias": { "type": "boolean" }
            }
        }),
    );
    schemas.insert(
        "AccountAssetListResponse".to_owned(),
        app_page_schema("#/components/schemas/AccountAssetListItem"),
    );
    schemas.insert(
        "AccountAssetQueryResponse".to_owned(),
        app_page_schema("#/components/schemas/AccountAssetListItem"),
    );
    schemas.insert(
        "AssetDefinitionListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["id"],
            "additionalProperties": true,
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "alias": { "type": ["string", "null"] },
                "alias_binding": { "$ref": "#/components/schemas/JsonValue" },
                "metadata": { "$ref": "#/components/schemas/JsonValue" },
                "owned_by": { "type": ["string", "null"] }
            }
        }),
    );
    schemas.insert(
        "AssetDefinitionListResponse".to_owned(),
        app_page_schema("#/components/schemas/AssetDefinitionListItem"),
    );
    schemas.insert(
        "AssetDefinitionQueryResponse".to_owned(),
        app_page_schema("#/components/schemas/AssetDefinitionListItem"),
    );
    schemas.insert(
        "AssetHolderListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_id", "asset", "scope", "quantity"],
            "additionalProperties": true,
            "properties": {
                "account_id": { "type": "string" },
                "asset": { "type": "string" },
                "asset_alias": { "type": ["string", "null"] },
                "scope": { "type": "string" },
                "quantity": { "type": "string" },
                "primary_alias": { "type": ["string", "null"] },
                "primary_alias_name": { "type": ["string", "null"] },
                "primary_alias_dataspace": { "type": ["string", "null"] },
                "primary_alias_domain": { "type": ["string", "null"] },
                "has_primary_alias": { "type": "boolean" }
            }
        }),
    );
    schemas.insert(
        "AssetHolderListResponse".to_owned(),
        app_page_schema("#/components/schemas/AssetHolderListItem"),
    );
    schemas.insert(
        "AssetHolderQueryResponse".to_owned(),
        app_page_schema("#/components/schemas/AssetHolderListItem"),
    );
    schemas.insert(
        "NftListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["id"],
            "additionalProperties": true,
            "properties": {
                "id": { "type": "string" }
            }
        }),
    );
    schemas.insert(
        "NftListResponse".to_owned(),
        app_page_schema("#/components/schemas/NftListItem"),
    );
    schemas.insert(
        "NftQueryResponse".to_owned(),
        app_page_schema("#/components/schemas/NftListItem"),
    );
    schemas.insert(
        "RwaListItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["id"],
            "additionalProperties": true,
            "properties": {
                "id": { "type": "string" }
            }
        }),
    );
    schemas.insert(
        "RwaListResponse".to_owned(),
        app_page_schema("#/components/schemas/RwaListItem"),
    );
    schemas.insert(
        "RwaQueryResponse".to_owned(),
        app_page_schema("#/components/schemas/RwaListItem"),
    );
    schemas.insert(
        "Hash".to_owned(),
        norito::json!({
            "type": "string",
            "pattern": "^hash:[0-9a-fA-F]{64}#[0-9a-fA-F]{4}$",
            "description": "Norito JSON hash string with checksum."
        }),
    );
    schemas.insert(
        "ManifestDigest".to_owned(),
        norito::json!({
            "type": "array",
            "minItems": 32,
            "maxItems": 32,
            "items": { "type": "integer", "minimum": 0, "maximum": 255 },
            "description": "Canonical 32-byte SoraFS manifest digest."
        }),
    );
    schemas.insert(
        "PrivateUploadedModelQuantizedCpuModel".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["input_len", "output_len", "weights_i8", "bias_i32", "output_shift", "output_min", "output_max"],
            "additionalProperties": false,
            "properties": {
                "input_len": { "type": "integer", "format": "uint64", "minimum": 0 },
                "output_len": { "type": "integer", "format": "uint64", "minimum": 0 },
                "weights_i8": {
                    "type": "array",
                    "items": { "type": "integer", "format": "int8" }
                },
                "bias_i32": {
                    "type": "array",
                    "items": { "type": "integer", "format": "int32" }
                },
                "output_shift": { "type": "integer", "minimum": 0, "maximum": 255 },
                "output_min": { "type": "integer", "format": "int32" },
                "output_max": { "type": "integer", "format": "int32" }
            }
        }),
    );
    schemas.insert(
        "PrivateUploadedModelArtifactRef".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["schema_version", "sorafs_manifest_digest", "artifact_hash", "ciphertext_bytes", "artifact_role"],
            "additionalProperties": false,
            "properties": {
                "schema_version": { "type": "integer", "minimum": 1 },
                "sorafs_manifest_digest": { "$ref": "#/components/schemas/ManifestDigest" },
                "artifact_hash": { "$ref": "#/components/schemas/Hash" },
                "ciphertext_bytes": { "type": "integer", "format": "uint64", "minimum": 0 },
                "artifact_role": { "type": "string", "enum": ["input", "output"] }
            }
        }),
    );
    schemas.insert(
        "PrivateUploadedModelExecuteRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["service_name", "weight_version", "policy_id", "model", "plaintext_input_i32", "input_artifact", "output_artifact", "emitted_sequence"],
            "additionalProperties": false,
            "properties": {
                "service_name": { "type": "string" },
                "weight_version": { "type": "string" },
                "model_id": { "type": ["string", "null"] },
                "model_name": { "type": ["string", "null"] },
                "bundle_root": { "anyOf": [{ "$ref": "#/components/schemas/Hash" }, { "type": "null" }] },
                "policy_id": { "type": "string" },
                "decryption_request_id": {
                    "type": ["string", "null"],
                    "description": "Optional committed decryption request record id that authorizes release of the encrypted input artifact."
                },
                "model": { "$ref": "#/components/schemas/PrivateUploadedModelQuantizedCpuModel" },
                "plaintext_input_i32": {
                    "type": "array",
                    "items": { "type": "integer", "format": "int32" }
                },
                "input_artifact": { "$ref": "#/components/schemas/PrivateUploadedModelArtifactRef" },
                "output_artifact": { "$ref": "#/components/schemas/PrivateUploadedModelArtifactRef" },
                "emitted_sequence": { "type": "integer", "format": "uint64", "minimum": 0 }
            }
        }),
    );
    schemas.insert(
        "PrivateUploadedModelReceipt".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["schema_version", "receipt_id", "service_name", "model_id", "weight_version", "runtime_version", "model_manifest_digest", "model_bundle_root", "policy_id", "input_commitment", "output_commitment", "input_artifact", "output_artifact", "emitted_sequence"],
            "additionalProperties": true,
            "properties": {
                "schema_version": { "type": "integer", "minimum": 1 },
                "receipt_id": { "$ref": "#/components/schemas/Hash" },
                "service_name": { "type": "string" },
                "model_id": { "type": "string" },
                "weight_version": { "type": "string" },
                "runtime_version": { "type": "string" },
                "model_manifest_digest": { "$ref": "#/components/schemas/ManifestDigest" },
                "model_bundle_root": { "$ref": "#/components/schemas/Hash" },
                "policy_id": { "type": "string" },
                "input_commitment": { "$ref": "#/components/schemas/Hash" },
                "output_commitment": { "$ref": "#/components/schemas/Hash" },
                "input_artifact": { "$ref": "#/components/schemas/PrivateUploadedModelArtifactRef" },
                "output_artifact": { "$ref": "#/components/schemas/PrivateUploadedModelArtifactRef" },
                "emitted_sequence": { "type": "integer", "format": "uint64", "minimum": 0 }
            }
        }),
    );
    schemas.insert(
        "SoracloudTxInstruction".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["kind", "payload_hex"],
            "additionalProperties": false,
            "properties": {
                "kind": {
                    "type": "string",
                    "description": "Fully-qualified instruction type name."
                },
                "payload_hex": {
                    "type": "string",
                    "pattern": "^[0-9a-fA-F]*$",
                    "description": "Norito-encoded instruction payload for external transaction signing."
                }
            }
        }),
    );
    schemas.insert(
        "PrivateUploadedModelExecuteResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["schema_version", "status", "receipt", "tx_instructions"],
            "additionalProperties": false,
            "properties": {
                "schema_version": { "type": "integer", "minimum": 1 },
                "status": { "$ref": "#/components/schemas/JsonValue" },
                "receipt": { "$ref": "#/components/schemas/PrivateUploadedModelReceipt" },
                "tx_instructions": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/SoracloudTxInstruction" }
                }
            }
        }),
    );
    schemas.insert(
        "PrivateUploadedModelReceiptListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["schema_version", "receipts", "returned_items", "remaining_items", "has_more", "count_mode"],
            "additionalProperties": false,
            "properties": {
                "schema_version": { "type": "integer", "minimum": 1 },
                "receipts": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/PrivateUploadedModelReceipt" }
                },
                "total": { "type": ["integer", "null"], "format": "uint32", "minimum": 0 },
                "returned_items": { "type": "integer", "format": "uint32", "minimum": 0 },
                "remaining_items": { "type": "integer", "format": "uint32", "minimum": 0 },
                "has_more": { "type": "boolean" },
                "count_mode": { "type": "string", "enum": ["bounded", "exact"] },
                "continue_cursor": { "type": ["string", "null"] }
            }
        }),
    );
    schemas.insert(
        "XmlText".to_owned(),
        norito::json!({
            "description": "XML payload returned as UTF-8 text.",
            "type": "string",
            "format": "xml"
        }),
    );
    schemas.insert(
        "OfflineIssuerBodyAuthRequest".to_owned(),
        norito::json!({
            "type": "object",
            "description": "Offline issuer POST body. Top-level account_id, timestamp_ms, nonce, and exactly one proof field authenticate the request body. The canonical signed body removes only top-level signature_base64 and witness_base64, so nested fields with those names remain signed business data.",
            "required": ["account_id", "timestamp_ms", "nonce"],
            "additionalProperties": true,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Canonical Iroha account id authorizing the issuer request."
                },
                "timestamp_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Unix timestamp in milliseconds included in the canonical signing envelope."
                },
                "nonce": {
                    "type": "string",
                    "description": "Caller-chosen replay nonce included in the canonical signing envelope."
                },
                "signature_base64": {
                    "type": "string",
                    "description": "Base64 single-account signature over the body-auth canonical request."
                },
                "witness_base64": {
                    "type": "string",
                    "description": "Base64 Norito CanonicalRequestWitnessV1 for multisig accounts."
                }
            },
            "oneOf": [
                {
                    "required": ["signature_base64"],
                    "not": { "required": ["witness_base64"] }
                },
                {
                    "required": ["witness_base64"],
                    "not": { "required": ["signature_base64"] }
                }
            ]
        }),
    );
    schemas.insert(
        "PipelineTransactionStatus".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["kind"],
            "additionalProperties": false,
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["Queued", "Approved", "Committed", "Applied", "Rejected", "Expired"],
                    "description": "Stable pipeline status kind."
                },
                "block_height": {
                    "type": ["integer", "null"],
                    "format": "uint64",
                    "description": "Block height reported for the status when available."
                },
                "rejection_reason": {
                    "$ref": "#/components/schemas/JsonValue",
                    "description": "Structured rejection reason when `kind` is `Rejected`."
                }
            }
        }),
    );
    schemas.insert(
        "PipelineTransactionStatusResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["hash", "status", "scope", "resolved_from"],
            "additionalProperties": false,
            "properties": {
                "hash": {
                    "type": "string",
                    "description": "Canonical signed transaction hash (hex, lowercase)."
                },
                "status": {
                    "$ref": "#/components/schemas/PipelineTransactionStatus"
                },
                "scope": {
                    "type": "string",
                    "enum": ["local", "auto", "global"],
                    "description": "Read scope applied by Torii."
                },
                "resolved_from": {
                    "type": "string",
                    "enum": ["cache", "queue", "state"],
                    "description": "Source used by Torii to resolve the status."
                }
            }
        }),
    );
    schemas.insert(
        "PipelinePreflightResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["schema_version", "chain_height", "sumeragi", "admission", "block", "pipeline", "queue", "fees"],
            "additionalProperties": false,
            "properties": {
                "schema_version": { "type": "integer", "format": "uint64", "minimum": 1 },
                "chain_height": { "type": "integer", "format": "uint64", "minimum": 0 },
                "sumeragi": {
                    "type": "object",
                    "required": ["block_time_ms", "commit_time_ms", "stall_threshold_ms"],
                    "additionalProperties": false,
                    "properties": {
                        "block_time_ms": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "commit_time_ms": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "stall_threshold_ms": {
                            "type": "integer",
                            "format": "uint64",
                            "minimum": 1,
                            "description": "Effective Sumeragi commit-quorum timeout used before clients classify queued transactions as stalled."
                        }
                    }
                },
                "admission": {
                    "type": "object",
                    "required": ["max_signatures", "max_instructions", "max_tx_bytes", "max_decompressed_bytes", "max_metadata_depth"],
                    "additionalProperties": false,
                    "properties": {
                        "max_signatures": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "max_instructions": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "max_tx_bytes": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "max_decompressed_bytes": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "max_metadata_depth": { "type": "integer", "format": "uint64", "minimum": 0 }
                    }
                },
                "block": {
                    "type": "object",
                    "required": ["max_transactions"],
                    "additionalProperties": false,
                    "properties": {
                        "max_transactions": { "type": "integer", "format": "uint64", "minimum": 0 }
                    }
                },
                "pipeline": {
                    "type": "object",
                    "required": ["signature_batch_max", "signature_batch_max_ed25519", "signature_batch_max_secp256k1", "signature_batch_max_pqc", "signature_batch_max_bls", "overlay_max_instructions", "ivm_max_decoded_instructions"],
                    "additionalProperties": false,
                    "properties": {
                        "signature_batch_max": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "signature_batch_max_ed25519": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "signature_batch_max_secp256k1": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "signature_batch_max_pqc": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "signature_batch_max_bls": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "overlay_max_instructions": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "ivm_max_decoded_instructions": { "type": "integer", "format": "uint64", "minimum": 0 }
                    }
                },
                "queue": {
                    "type": "object",
                    "required": ["size", "queued", "inflight"],
                    "additionalProperties": false,
                    "properties": {
                        "size": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "queued": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "inflight": { "type": "integer", "format": "uint64", "minimum": 0 }
                    }
                },
                "fees": {
                    "type": "object",
                    "required": ["fee_asset_id", "fee_sink_account_id", "base_fee", "per_byte_fee", "per_instruction_fee", "per_gas_unit_fee", "sponsorship_enabled", "sponsor_max_fee", "sponsor_verified_balance_safety_floor", "fee_receipts_activation_height", "external_settlement_enabled", "burn_from_unix_timestamp_ms", "settlement_mode", "successful_claim_fee_exempt_authorities", "sponsored_contract_operation_allowlist"],
                    "additionalProperties": false,
                    "properties": {
                        "fee_asset_id": { "type": "string" },
                        "fee_sink_account_id": { "type": "string" },
                        "base_fee": { "$ref": "#/components/schemas/JsonValue" },
                        "per_byte_fee": { "$ref": "#/components/schemas/JsonValue" },
                        "per_instruction_fee": { "$ref": "#/components/schemas/JsonValue" },
                        "per_gas_unit_fee": { "$ref": "#/components/schemas/JsonValue" },
                        "sponsorship_enabled": { "type": "boolean" },
                        "sponsor_max_fee": { "$ref": "#/components/schemas/JsonValue" },
                        "sponsor_verified_balance_safety_floor": { "$ref": "#/components/schemas/JsonValue" },
                        "canonical_sponsor_account_id": { "type": ["string", "null"] },
                        "fee_receipts_activation_height": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "external_settlement_enabled": { "type": "boolean" },
                        "burn_from_unix_timestamp_ms": { "type": "integer", "format": "uint64", "minimum": 0 },
                        "settlement_mode": { "type": "string" },
                        "successful_claim_fee_exempt_authorities": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "sponsored_contract_operation_allowlist": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["entrypoints"],
                                "additionalProperties": false,
                                "properties": {
                                    "contract_alias": { "type": ["string", "null"] },
                                    "contract_address": { "type": ["string", "null"] },
                                    "entrypoints": {
                                        "type": "array",
                                        "items": { "type": "string" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }),
    );
    schemas.insert(
        "VersionedSignedTransactionJson".to_owned(),
        versioned_json_payload_schema(
            "Canonical Norito JSON representation of `SignedTransaction`.",
        ),
    );
    schemas.insert(
        "VersionedTransactionEntrypointJson".to_owned(),
        versioned_json_payload_schema(
            "Canonical Norito JSON representation of `TransactionEntrypoint`.",
        ),
    );
    schemas.insert(
        "VersionedSignedQueryJson".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["version", "content"],
            "additionalProperties": false,
            "properties": {
                "version": {
                    "type": "string",
                    "enum": ["canonical"],
                    "description": "Signed query JSON wire version."
                },
                "content": {
                    "$ref": "#/components/schemas/JsonValue",
                    "description": "Canonical SignedQuery payload."
                }
            }
        }),
    );
    schemas.insert(
        "TransactionSubmissionReceipt".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["payload", "signature"],
            "additionalProperties": false,
            "properties": {
                "payload": {
                    "type": "object",
                    "required": ["tx_hash", "entrypoint_hash", "submitted_at_ms", "submitted_at_height", "signer"],
                    "properties": {
                        "tx_hash": { "type": "string" },
                        "entrypoint_hash": { "type": "string" },
                        "signed_transaction_hash": { "type": ["string", "null"] },
                        "submitted_at_ms": { "type": "integer", "format": "uint64" },
                        "submitted_at_height": { "type": "integer", "format": "uint64" },
                        "signer": { "$ref": "#/components/schemas/JsonValue" }
                    }
                },
                "signature": { "$ref": "#/components/schemas/JsonValue" }
            }
        }),
    );
    schemas.insert(
        "AccountOnboardingRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias", "uaid"],
            "additionalProperties": false,
            "oneOf": [
                { "required": ["account_id"] },
                { "required": ["public_key_hex"] }
            ],
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Account alias literal such as `alice@universal` or `merchant@domain.dataspace`."
                },
                "account_id": {
                    "type": "string",
                    "description": "Canonical I105 account id to register. Mutually exclusive with generated `public_key_hex` onboarding material."
                },
                "public_key_hex": {
                    "type": "string",
                    "description": "Raw 32-byte Ed25519 public key bytes as 64 hex characters; Torii derives the canonical account id from this key."
                },
                "uaid": {
                    "type": "string",
                    "description": "Explicit UAID literal (`uaid:<hex>` or raw 64-hex digest with the canonical low bit set)."
                },
                "identity_commitment_hex": {
                    "type": "string",
                    "description": "Optional 64-hex digest commitment to off-chain identity evidence. Raw identity metadata is not accepted."
                },
                "permissions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional permission literals requested from the onboarding allowlist."
                }
            }
        }),
    );
    schemas.insert(
        "SumeragiVrfCommitRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["epoch", "signer", "commitment_hex", "bls_sig_hex"],
            "additionalProperties": false,
            "properties": {
                "epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Sumeragi VRF epoch number."
                },
                "signer": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Validator index in the active topology for the epoch height."
                },
                "commitment_hex": {
                    "type": "string",
                    "description": "32-byte VRF reveal commitment encoded as hex."
                },
                "bls_sig_hex": {
                    "type": "string",
                    "description": "BLS signature over the Sumeragi VRF commit preimage, encoded as non-empty hex."
                }
            }
        }),
    );
    schemas.insert(
        "SumeragiVrfRevealRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["epoch", "signer", "reveal_hex", "bls_sig_hex"],
            "additionalProperties": false,
            "properties": {
                "epoch": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Sumeragi VRF epoch number."
                },
                "signer": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Validator index in the active topology for the epoch height."
                },
                "reveal_hex": {
                    "type": "string",
                    "description": "32-byte VRF reveal value encoded as hex."
                },
                "bls_sig_hex": {
                    "type": "string",
                    "description": "BLS signature over the Sumeragi VRF reveal preimage, encoded as non-empty hex."
                }
            }
        }),
    );
    schemas.insert(
        "AccountReadResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_id", "opaque_ids"],
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Canonical account identifier rendered as the domainless I105 literal."
                },
                "label": {
                    "$ref": "#/components/schemas/JsonValue",
                    "description": "Stable account label when assigned.",
                    "nullable": true
                },
                "uaid": {
                    "$ref": "#/components/schemas/JsonValue",
                    "description": "Universal account identifier bound to this account when registered in Nexus.",
                    "nullable": true
                },
                "opaque_ids": {
                    "type": "array",
                    "description": "Opaque identifiers currently mapped to the account UAID.",
                    "items": { "$ref": "#/components/schemas/JsonValue" }
                }
            }
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
                "account_id": { "type": "string", "description": "Account selector for the device owner (canonical I105 or on-chain alias `name@domain.dataspace` / `name@dataspace`). The signed X-Iroha-Account header must resolve to the same canonical account; registrations are stored under that canonical I105 account id." },
                "platform": { "type": "string", "description": "Push platform label (FCM, APNS)." },
                "token": { "type": "string", "description": "Provider device token. Use DELETE with this body shape to unregister; do not place tokens in URLs." },
                "topics": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional topic labels stored with the device; bounded by torii.push.max_topics_per_device."
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
        "AssetAliasResolveRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias"],
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Asset alias literal (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) to resolve."
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
        "BfvParameters".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["polynomial_degree", "plaintext_modulus", "ciphertext_modulus"],
            "additionalProperties": false,
            "properties": {
                "polynomial_degree": {
                    "type": "integer",
                    "format": "uint32",
                    "description": "Ring degree used by the BFV input-encryption envelope."
                },
                "plaintext_modulus": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Plaintext modulus `t` used by the BFV input-encryption envelope."
                },
                "ciphertext_modulus": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Ciphertext modulus `q` used by the BFV input-encryption envelope."
                }
            }
        }),
    );
    schemas.insert(
        "BfvPublicKey".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["b", "a"],
            "additionalProperties": false,
            "properties": {
                "b": {
                    "type": "array",
                    "items": {"type": "integer", "format": "uint64"},
                    "description": "First BFV public-key polynomial."
                },
                "a": {
                    "type": "array",
                    "items": {"type": "integer", "format": "uint64"},
                    "description": "Second BFV public-key polynomial."
                }
            }
        }),
    );
    schemas.insert(
        "BfvIdentifierPublicParameters".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["parameters", "public_key", "max_input_bytes"],
            "additionalProperties": false,
            "properties": {
                "parameters": {
                    "$ref": "#/components/schemas/BfvParameters"
                },
                "public_key": {
                    "$ref": "#/components/schemas/BfvPublicKey"
                },
                "max_input_bytes": {
                    "type": "integer",
                    "format": "uint16",
                    "description": "Maximum number of UTF-8 bytes accepted by the BFV input envelope."
                }
            }
        }),
    );
    schemas.insert(
        "BfvRamEncryptedInputMode".to_owned(),
        norito::json!({
            "type": "string",
            "enum": ["encrypted_envelope_v1"],
            "description": "Encrypted-only RAM-FHE input envelope mode."
        }),
    );
    schemas.insert(
        "BfvRamProgramProfile".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "profile_version",
                "register_count",
                "memory_lane_count",
                "ciphertext_mul_per_step",
                "encrypted_input_mode",
                "min_ciphertext_modulus"
            ],
            "additionalProperties": false,
            "properties": {
                "profile_version": {
                    "type": "integer",
                    "format": "uint8",
                    "description": "Stable programmed RAM-FHE profile version."
                },
                "register_count": {
                    "type": "integer",
                    "format": "uint16",
                    "description": "Number of ciphertext registers in the hidden execution machine."
                },
                "memory_lane_count": {
                    "type": "integer",
                    "format": "uint16",
                    "description": "Number of ciphertext memory lanes persisted across program steps."
                },
                "ciphertext_mul_per_step": {
                    "type": "integer",
                    "format": "uint8",
                    "description": "Maximum ciphertext-ciphertext multiplications per programmed step."
                },
                "encrypted_input_mode": {
                    "$ref": "#/components/schemas/BfvRamEncryptedInputMode"
                },
                "min_ciphertext_modulus": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Minimum supported BFV ciphertext modulus for the programmed profile."
                }
            }
        }),
    );
    schemas.insert(
        "IdentifierResolutionReceiptPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "policy_id",
                "execution",
                "opening",
                "opaque_id",
                "receipt_hash",
                "uaid",
                "account_id"
            ],
            "additionalProperties": false,
            "properties": {
                "policy_id": {
                    "type": "string",
                    "description": "Identifier policy namespace used for the resolution."
                },
                "execution": {
                    "$ref": "#/components/schemas/RamLfeExecutionReceiptPayload"
                },
                "opening": {
                    "$ref": "#/components/schemas/RamLfeOutputOpening"
                },
                "opaque_id": {
                    "type": "string",
                    "description": "Derived opaque identifier literal."
                },
                "receipt_hash": {
                    "type": "string",
                    "description": "Deterministic hidden-function receipt hash for this evaluation."
                },
                "uaid": {
                    "type": "string",
                    "description": "UAID currently bound to the opaque identifier."
                },
                "account_id": {
                    "type": "string",
                    "description": "Canonical account identifier currently bound to the UAID."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeOutputOpeningPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "program_id",
                "input_ciphertext_hash",
                "output_ciphertext_hash",
                "parameter_digest",
                "evaluation_key_digest",
                "opened_output_hash",
                "opened_at_ms"
            ],
            "additionalProperties": false,
            "properties": {
                "program_id": {
                    "type": "string",
                    "description": "RAM-LFE program identifier whose encrypted output was opened."
                },
                "input_ciphertext_hash": {
                    "type": "string",
                    "description": "Hash of the encrypted input envelope from the execution receipt."
                },
                "output_ciphertext_hash": {
                    "type": "string",
                    "description": "Hash of the encrypted output envelope from the execution receipt."
                },
                "parameter_digest": {
                    "type": "string",
                    "description": "Digest of the registered BFV parameter set used for execution."
                },
                "evaluation_key_digest": {
                    "type": "string",
                    "description": "Digest of the BFV evaluation-key bundle used for execution."
                },
                "opened_output_hash": {
                    "type": "string",
                    "description": "Hash of the plaintext output bytes opened by the external authority."
                },
                "opened_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Opening timestamp in milliseconds since Unix epoch."
                },
                "expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Optional opening expiry timestamp in milliseconds since Unix epoch."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeOutputOpening".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["payload", "signature"],
            "additionalProperties": false,
            "properties": {
                "payload": {
                    "$ref": "#/components/schemas/RamLfeOutputOpeningPayload"
                },
                "signature": {
                    "type": "string",
                    "description": "Hex-encoded signature from the configured output-opening authority."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeExecutionReceiptPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "program_id",
                "program_digest",
                "backend",
                "verification_mode",
                "input_ciphertext_hash",
                "output_ciphertext_hash",
                "parameter_digest",
                "evaluation_key_digest",
                "output_hash",
                "associated_data_hash",
                "executed_at_ms"
            ],
            "additionalProperties": false,
            "properties": {
                "program_id": {
                    "type": "string",
                    "description": "RAM-LFE program identifier."
                },
                "program_digest": {
                    "type": "string",
                    "description": "Published digest of the hidden compiled program."
                },
                "backend": {
                    "type": "string",
                    "description": "RAM-LFE backend that produced the receipt."
                },
                "verification_mode": {
                    "type": "string",
                    "description": "Receipt attestation mode (`signed` or `proof`)."
                },
                "input_ciphertext_hash": {
                    "type": "string",
                    "description": "Hash of the encrypted input envelope evaluated by the program."
                },
                "output_ciphertext_hash": {
                    "type": "string",
                    "description": "Hash of the encrypted output envelope produced by the program."
                },
                "parameter_digest": {
                    "type": "string",
                    "description": "Digest of the registered BFV parameter set used for execution."
                },
                "evaluation_key_digest": {
                    "type": "string",
                    "description": "Digest of the BFV evaluation-key bundle used for execution."
                },
                "output_hash": {
                    "type": "string",
                    "description": "Hash of the encrypted output bytes; Torii execution never returns plaintext output."
                },
                "associated_data_hash": {
                    "type": "string",
                    "description": "Hash of the associated-data blob bound into execution."
                },
                "executed_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Execution timestamp in milliseconds since Unix epoch."
                },
                "expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Optional receipt expiry timestamp in milliseconds since Unix epoch."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeProofVerifierMetadata".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "proof_backend",
                "circuit_id",
                "public_inputs_schema_hash",
                "verifying_key_bytes_b64"
            ],
            "additionalProperties": false,
            "properties": {
                "proof_backend": {
                    "type": "string",
                    "description": "Proof backend expected by verifier records."
                },
                "circuit_id": {
                    "type": "string",
                    "description": "Circuit identifier bound to the verifier."
                },
                "public_inputs_schema_hash": {
                    "type": "string",
                    "description": "Hash of the public-input schema accepted by the verifier."
                },
                "verifying_key_bytes_b64": {
                    "type": "string",
                    "description": "Base64-encoded verifying key bytes; the on-chain verifier hash is derived from these bytes."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeSignedReceiptAttestation".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["kind", "signature"],
            "additionalProperties": false,
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["signed"]
                },
                "signature": {
                    "type": "string",
                    "description": "Hex-encoded resolver signature over the canonical receipt payload."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeProofReceiptAttestation".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["kind", "proof_backend", "proof_b64"],
            "additionalProperties": false,
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["proof"]
                },
                "proof_backend": {
                    "type": "string",
                    "description": "Proof backend used to produce the receipt attestation."
                },
                "proof_b64": {
                    "type": "string",
                    "description": "Base64-encoded proof bytes."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeReceiptAttestation".to_owned(),
        norito::json!({
            "oneOf": [
                {"$ref": "#/components/schemas/RamLfeSignedReceiptAttestation"},
                {"$ref": "#/components/schemas/RamLfeProofReceiptAttestation"}
            ]
        }),
    );
    schemas.insert(
        "RamLfeExecutionReceipt".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["payload", "attestation"],
            "additionalProperties": false,
            "properties": {
                "payload": {
                    "$ref": "#/components/schemas/RamLfeExecutionReceiptPayload"
                },
                "attestation": {
                    "$ref": "#/components/schemas/RamLfeReceiptAttestation"
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeProgramPolicySummary".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "program_id",
                "owner",
                "active",
                "resolver_public_key",
                "output_opening_public_key",
                "backend",
                "verification_mode"
            ],
            "additionalProperties": false,
            "properties": {
                "program_id": {
                    "type": "string",
                    "description": "RAM-LFE program identifier."
                },
                "owner": {
                    "type": "string",
                    "description": "Canonical account identifier that owns the program policy."
                },
                "active": {
                    "type": "boolean",
                    "description": "Whether the program policy is active for new executions."
                },
                "resolver_public_key": {
                    "type": "string",
                    "description": "Public key used to verify RAM-LFE execution receipts."
                },
                "output_opening_public_key": {
                    "type": "string",
                    "description": "Public key used to verify externally signed RAM-LFE output openings."
                },
                "backend": {
                    "type": "string",
                    "description": "RAM-LFE backend advertised by the program policy."
                },
                "verification_mode": {
                    "type": "string",
                    "description": "Receipt attestation mode (`signed` or `proof`)."
                },
                "input_encryption": {
                    "type": "string",
                    "description": "Optional encrypted-input scheme published for this program."
                },
                "input_encryption_public_parameters": {
                    "type": "string",
                    "description": "Optional hex-encoded Norito payload containing the public input-encryption parameters."
                },
                "input_encryption_public_parameters_decoded": {
                    "$ref": "#/components/schemas/BfvIdentifierPublicParameters"
                },
                "ram_fhe_profile": {
                    "$ref": "#/components/schemas/BfvRamProgramProfile"
                },
                "proof_verifier": {
                    "$ref": "#/components/schemas/RamLfeProofVerifierMetadata"
                },
                "note": {
                    "type": "string",
                    "description": "Optional human-readable note attached to the program policy."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeProgramPolicyListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["total", "items"],
            "additionalProperties": false,
            "properties": {
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of program policies returned."
                },
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/RamLfeProgramPolicySummary"
                    },
                    "description": "Registered RAM-LFE program policies."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeExecuteRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["encrypted_input"],
            "additionalProperties": false,
            "properties": {
                "encrypted_input": {
                    "type": "string",
                    "description": "Hex-encoded Norito BFV ciphertext envelope. Plaintext RAM-LFE inputs are rejected."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeExecuteResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "program_id",
                "opaque_hash",
                "receipt_hash",
                "output_ciphertext",
                "output_hash",
                "associated_data_hash",
                "executed_at_ms",
                "backend",
                "verification_mode",
                "receipt",
                "output_opening"
            ],
            "additionalProperties": false,
            "properties": {
                "program_id": {
                    "type": "string",
                    "description": "RAM-LFE program identifier."
                },
                "opaque_hash": {
                    "type": "string",
                    "description": "Opaque evaluator hash returned by the RAM-LFE backend."
                },
                "receipt_hash": {
                    "type": "string",
                    "description": "Deterministic receipt hash returned by the RAM-LFE backend."
                },
                "output_ciphertext": {
                    "type": "string",
                    "description": "Hex-encoded Norito BFV ciphertext envelope produced by the RAM-LFE evaluator."
                },
                "output_hash": {
                    "type": "string",
                    "description": "Hash of the encrypted output ciphertext bytes."
                },
                "associated_data_hash": {
                    "type": "string",
                    "description": "Hash of the associated-data blob bound into execution."
                },
                "executed_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Execution timestamp in milliseconds since Unix epoch."
                },
                "expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Optional receipt expiry timestamp in milliseconds since Unix epoch."
                },
                "backend": {
                    "type": "string",
                    "description": "RAM-LFE backend that produced the receipt."
                },
                "verification_mode": {
                    "type": "string",
                    "description": "Receipt attestation mode (`signed` or `proof`)."
                },
                "receipt": {
                    "$ref": "#/components/schemas/RamLfeExecutionReceipt"
                },
                "output_opening": {
                    "$ref": "#/components/schemas/RamLfeOutputOpening",
                    "description": "Signed opening over the encrypted output hash. Clients reuse this with the same encrypted input when resolving or claiming identifiers."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeReceiptVerifyRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["receipt"],
            "additionalProperties": false,
            "properties": {
                "receipt": {
                    "$ref": "#/components/schemas/RamLfeExecutionReceipt"
                },
                "output_hex": {
                    "type": "string",
                    "description": "Optional hex-encoded encrypted output ciphertext bytes to compare against `receipt.payload.output_hash`."
                }
            }
        }),
    );
    schemas.insert(
        "RamLfeReceiptVerifyResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "valid",
                "program_id",
                "backend",
                "verification_mode",
                "output_hash",
                "associated_data_hash"
            ],
            "additionalProperties": false,
            "properties": {
                "valid": {
                    "type": "boolean",
                    "description": "Whether the receipt validated against the published program policy."
                },
                "program_id": {
                    "type": "string",
                    "description": "RAM-LFE program identifier from the receipt."
                },
                "backend": {
                    "type": "string",
                    "description": "RAM-LFE backend advertised by the receipt."
                },
                "verification_mode": {
                    "type": "string",
                    "description": "Receipt attestation mode (`signed` or `proof`)."
                },
                "output_hash": {
                    "type": "string",
                    "description": "Receipt payload output hash."
                },
                "associated_data_hash": {
                    "type": "string",
                    "description": "Receipt payload associated-data hash."
                },
                "output_hash_matches": {
                    "type": "boolean",
                    "description": "Whether the supplied `output_hex` matched `output_hash`, when provided."
                },
                "error": {
                    "type": "string",
                    "description": "Validation failure reason when `valid == false`."
                }
            }
        }),
    );
    schemas.insert(
        "IdentifierPolicySummary".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["policy_id", "program_id", "owner", "active", "normalization", "resolver_public_key", "output_opening_public_key", "backend"],
            "additionalProperties": false,
            "properties": {
                "policy_id": {
                    "type": "string",
                    "description": "Identifier policy namespace literal (`<kind>#<business_rule>`)."
                },
                "program_id": {
                    "type": "string",
                    "description": "RAM-LFE program policy used by this identifier policy."
                },
                "owner": {
                    "type": "string",
                    "description": "Canonical account identifier that owns the policy."
                },
                "active": {
                    "type": "boolean",
                    "description": "Whether the policy is active for new claims and resolutions."
                },
                "normalization": {
                    "type": "string",
                    "description": "Client-side canonicalization mode required before encrypted input is produced."
                },
                "resolver_public_key": {
                    "type": "string",
                    "description": "Public key used to verify resolution receipts."
                },
                "output_opening_public_key": {
                    "type": "string",
                    "description": "Public key used to verify externally signed RAM-LFE output openings."
                },
                "backend": {
                    "type": "string",
                    "description": "Hidden-function backend advertised by the policy commitment."
                },
                "input_encryption": {
                    "type": "string",
                    "description": "Optional encrypted-input scheme published for this policy."
                },
                "input_encryption_public_parameters": {
                    "type": "string",
                    "description": "Optional hex-encoded Norito payload containing the public input-encryption parameters."
                },
                "input_encryption_public_parameters_decoded": {
                    "$ref": "#/components/schemas/BfvIdentifierPublicParameters"
                },
                "ram_fhe_profile": {
                    "$ref": "#/components/schemas/BfvRamProgramProfile"
                },
                "proof_verifier": {
                    "$ref": "#/components/schemas/RamLfeProofVerifierMetadata"
                },
                "note": {
                    "type": "string",
                    "description": "Optional human-readable note attached to the policy."
                }
            }
        }),
    );
    schemas.insert(
        "IdentifierPolicyListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["total", "items"],
            "additionalProperties": false,
            "properties": {
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Number of policies returned."
                },
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/IdentifierPolicySummary"
                    },
                    "description": "Registered identifier policies."
                }
            }
        }),
    );
    schemas.insert(
        "IdentifierResolveRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["policy_id", "encrypted_input", "output_opening"],
            "additionalProperties": false,
            "properties": {
                "policy_id": {
                    "type": "string",
                    "description": "Identifier policy namespace literal (`<kind>#<business_rule>`)."
                },
                "encrypted_input": {
                    "type": "string",
                    "description": "Hex-encoded Norito BFV ciphertext envelope. Plaintext hidden-function inputs are rejected."
                },
                "output_opening": {
                    "$ref": "#/components/schemas/RamLfeOutputOpening"
                }
            }
        }),
    );
    schemas.insert(
        "IdentifierResolveResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "payload",
                "attestation"
            ],
            "additionalProperties": false,
            "properties": {
                "payload": {
                    "$ref": "#/components/schemas/IdentifierResolutionReceiptPayload"
                },
                "attestation": {
                    "$ref": "#/components/schemas/RamLfeReceiptAttestation"
                }
            }
        }),
    );
    schemas.insert(
        "IdentifierClaimLookupResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": [
                "policy_id",
                "opaque_id",
                "receipt_hash",
                "uaid",
                "account_id",
                "verified_at_ms"
            ],
            "additionalProperties": false,
            "properties": {
                "policy_id": {
                    "type": "string",
                    "description": "Identifier policy namespace bound to the claim."
                },
                "opaque_id": {
                    "type": "string",
                    "description": "Derived opaque identifier literal."
                },
                "receipt_hash": {
                    "type": "string",
                    "description": "Deterministic hidden-function receipt hash for the original evaluation."
                },
                "uaid": {
                    "type": "string",
                    "description": "UAID currently bound to the opaque identifier."
                },
                "account_id": {
                    "type": "string",
                    "description": "Canonical account identifier currently bound to the UAID."
                },
                "verified_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Ledger timestamp in milliseconds when the identifier claim was accepted."
                },
                "expires_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Optional identifier-claim expiry timestamp in milliseconds since Unix epoch."
                }
            }
        }),
    );
    schemas.insert(
        "AliasLookupByAccountRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_id"],
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Canonical account identifier to inspect."
                },
                "dataspace": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional dataspace alias filter applied after routing."
                },
                "domain": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional alias-domain filter applied after routing."
                }
            }
        }),
    );
    schemas.insert(
        "AliasLookupByAccountItem".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias", "dataspace", "is_primary"],
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Canonical alias representation."
                },
                "dataspace": {
                    "type": "string",
                    "description": "Dataspace alias that owns the binding."
                },
                "domain": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional alias domain."
                },
                "is_primary": {
                    "type": "boolean",
                    "description": "Whether this alias is the account's primary label in that dataspace."
                }
            }
        }),
    );
    schemas.insert(
        "AliasLookupByAccountResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["account_id", "total", "items"],
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Canonical account identifier used for the lookup."
                },
                "total": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Merged count of deduplicated alias rows."
                },
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/AliasLookupByAccountItem"
                    },
                    "description": "Merged alias rows returned by reachable dataspaces."
                },
                "source": {
                    "type": "string",
                    "description": "Resolver source, including `fanout` when multiple routes contributed."
                }
            }
        }),
    );
    schemas.insert(
        "AssetAliasResolveResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias", "asset_definition_id", "asset_name"],
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Canonical alias representation."
                },
                "asset_definition_id": {
                    "type": "string",
                    "description": "Canonical asset definition id (unprefixed Base58 address)."
                },
                "asset_name": {
                    "type": "string",
                    "description": "Human-readable asset name."
                },
                "alias_binding": {
                    "$ref": "#/components/schemas/AssetAliasBinding"
                },
                "description": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional asset description."
                },
                "logo": {
                    "type": "string",
                    "nullable": true,
                    "description": "Optional SoraFS logo URI."
                },
                "source": {
                    "type": "string",
                    "description": "Resolver source."
                }
            }
        }),
    );
    schemas.insert(
        "AssetAliasBinding".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["alias", "status", "bound_at_ms"],
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Canonical alias representation."
                },
                "status": {
                    "type": "string",
                    "enum": ["permanent", "leased_active", "leased_grace", "expired_pending_cleanup"],
                    "description": "Observed lease state for the alias binding."
                },
                "lease_expiry_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Lease expiry timestamp in milliseconds since Unix epoch."
                },
                "grace_until_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "nullable": true,
                    "description": "Grace-window end timestamp in milliseconds since Unix epoch."
                },
                "bound_at_ms": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Timestamp in milliseconds when the alias binding was recorded."
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
                    "description": "Relay account identifier rendered as canonical I105 literal."
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
        app_page_schema("#/components/schemas/RepoAgreement"),
    );
    schemas.insert(
        "RepoAgreementsQueryRequest".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["pagination"],
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": ["string", "null"]
                },
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
                "select": { "$ref": "#/components/schemas/JsonValue" },
                "aggregate": { "$ref": "#/components/schemas/JsonValue" },
                "fetch_size": {
                    "type": ["integer", "null"],
                    "format": "uint64"
                },
                "count_mode": {
                    "type": ["string", "null"],
                    "enum": ["bounded", "exact", null]
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
            "required": ["lane_id", "validator", "peer_id", "stake_account", "total_stake", "self_stake", "status"],
            "additionalProperties": false,
            "properties": {
                "lane_id": {
                    "type": "integer",
                    "format": "uint64",
                    "description": "Lane serviced by the validator."
                },
                "validator": {
                    "type": "string",
                    "description": "Validator authority account literal rendered as canonical I105."
                },
                "peer_id": {
                    "type": "string",
                    "description": "Peer identity bound to the validator for consensus and routed traffic."
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
                    "description": "Validator account literal rendered as canonical I105."
                },
                "staker": {
                    "type": "string",
                    "description": "Staker account literal rendered as canonical I105."
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
    schemas.insert(
        "MultisigAccountSelector".to_owned(),
        norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "multisig_account_id": {
                    "type": "string",
                    "description": "Active concrete multisig account id."
                },
                "multisig_account_alias": {
                    "type": "string",
                    "description": "Stable multisig alias in name@dataspace or name@domain.dataspace format."
                }
            },
            "oneOf": [
                { "required": ["multisig_account_id"] },
                { "required": ["multisig_account_alias"] }
            ]
        }),
    );
    schemas.insert(
        "MultisigSpecPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["signatories", "quorum", "transaction_ttl_ms"],
            "additionalProperties": false,
            "properties": {
                "signatories": {
                    "type": "object",
                    "description": "Map of signer account ids to weight.",
                    "additionalProperties": {
                        "type": "integer",
                        "format": "uint8"
                    }
                },
                "quorum": {
                    "type": "integer",
                    "format": "uint16"
                },
                "transaction_ttl_ms": {
                    "type": "integer",
                    "format": "uint64"
                }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalPayload".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["instructions", "proposed_at_ms", "expires_at_ms", "approvals"],
            "additionalProperties": false,
            "properties": {
                "instructions": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "proposed_at_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "expires_at_ms": {
                    "type": "integer",
                    "format": "uint64"
                },
                "approvals": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "is_relayed": {
                    "anyOf": [
                        { "type": "boolean" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
    schemas.insert(
        "MultisigProposeRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id", "instructions"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_sponsor": { "type": "string" },
                        "memo": { "type": "string" },
                        "instructions": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/MultisigProposeInstructionInput" }
                        }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigProposeInstructionInput".to_owned(),
        norito::json!({
            "type": "object",
            "description": "Structured JSON InstructionBox object. For native Norito, send the entire MultisigProposeRequest body as application/x-norito; the JSON instructions field does not accept per-instruction Norito blobs."
        }),
    );
    schemas.insert(
        "MultisigApproveRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_sponsor": { "type": "string" },
                        "proposal_id": { "type": "string" },
                        "instructions_hash": { "type": "string" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["ok", "resolved_multisig_account_id"],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "resolved_multisig_account_id": { "type": "string" },
                "submitted": { "type": "boolean" },
                "proposal_id": { "type": "string" },
                "instructions_hash": { "type": "string" },
                "tx_hash_hex": { "type": "string" },
                "executed_tx_hash_hex": { "type": "string" },
                "creation_time_ms": { "type": "integer", "format": "uint64" },
                "signing_message_b64": { "type": "string" }
            }
        }),
    );
    schemas.insert(
        "MultisigContractCallProposeRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id", "entrypoint"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "contract_address": { "type": "string" },
                        "contract_alias": { "type": "string" },
                        "entrypoint": { "type": "string" },
                        "payload": { "type": "object" },
                        "gas_asset_id": { "type": "string" },
                        "fee_sponsor": { "type": "string" },
                        "gas_limit": { "type": "integer", "format": "uint64" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigContractCallApproveRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_sponsor": { "type": "string" },
                        "proposal_id": { "type": "string" },
                        "instructions_hash": { "type": "string" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigContractCallResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["ok", "resolved_multisig_account_id"],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "resolved_multisig_account_id": { "type": "string" },
                "submitted": { "type": "boolean" },
                "proposal_id": { "type": "string" },
                "instructions_hash": { "type": "string" },
                "tx_hash_hex": { "type": "string" },
                "executed_tx_hash_hex": { "type": "string" },
                "creation_time_ms": { "type": "integer", "format": "uint64" },
                "signing_message_b64": { "type": "string" }
            }
        }),
    );
    schemas.insert(
        "MultisigCancelRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "required": ["signer_account_id"],
                    "additionalProperties": false,
                    "properties": {
                        "signer_account_id": { "type": "string" },
                        "public_key_hex": { "type": "string" },
                        "signature_b64": { "type": "string" },
                        "creation_time_ms": { "type": "integer", "format": "uint64" },
                        "fee_sponsor": { "type": "string" },
                        "proposal_id": { "type": "string" },
                        "instructions_hash": { "type": "string" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigCancelResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["ok", "resolved_multisig_account_id"],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "resolved_multisig_account_id": { "type": "string" },
                "submitted": { "type": "boolean" },
                "action": { "type": "string" },
                "target_proposal_id": { "type": "string" },
                "target_instructions_hash": { "type": "string" },
                "cancel_proposal_id": { "type": "string" },
                "cancel_instructions_hash": { "type": "string" },
                "executed_tx_hash_hex": { "type": "string" },
                "creation_time_ms": { "type": "integer", "format": "uint64" },
                "signing_message_b64": { "type": "string" }
            }
        }),
    );
    schemas.insert(
        "MultisigSpecRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" }
            ]
        }),
    );
    schemas.insert(
        "MultisigSpecResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["resolved_multisig_account_id", "spec"],
            "additionalProperties": false,
            "properties": {
                "resolved_multisig_account_id": { "type": "string" },
                "spec": { "$ref": "#/components/schemas/MultisigSpecPayload" }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalsListRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" }
            ]
        }),
    );
    schemas.insert(
        "MultisigProposalEntry".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["proposal_id", "instructions_hash", "proposal"],
            "additionalProperties": false,
            "properties": {
                "proposal_id": { "type": "string" },
                "instructions_hash": { "type": "string" },
                "proposal": { "$ref": "#/components/schemas/MultisigProposalPayload" }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalsListResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["resolved_multisig_account_id", "proposals"],
            "additionalProperties": false,
            "properties": {
                "resolved_multisig_account_id": { "type": "string" },
                "proposals": {
                    "type": "array",
                    "items": { "$ref": "#/components/schemas/MultisigProposalEntry" }
                }
            }
        }),
    );
    schemas.insert(
        "MultisigProposalsGetRequest".to_owned(),
        norito::json!({
            "allOf": [
                { "$ref": "#/components/schemas/MultisigAccountSelector" },
                {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "proposal_id": { "type": "string" },
                        "instructions_hash": { "type": "string" }
                    }
                }
            ]
        }),
    );
    schemas.insert(
        "MultisigProposalGetResponse".to_owned(),
        norito::json!({
            "type": "object",
            "required": ["resolved_multisig_account_id", "proposal_id", "instructions_hash", "proposal"],
            "additionalProperties": false,
            "properties": {
                "resolved_multisig_account_id": { "type": "string" },
                "proposal_id": { "type": "string" },
                "instructions_hash": { "type": "string" },
                "proposal": { "$ref": "#/components/schemas/MultisigProposalPayload" }
            }
        }),
    );
    schemas
}

fn queue_error_snapshot_schema() -> Value {
    norito::json!({
        "type": "object",
        "required": ["state", "queued", "capacity", "saturated"],
        "additionalProperties": false,
        "properties": {
            "state": {
                "type": "string",
                "description": "Queue state label, for example `healthy` or `saturated`."
            },
            "queued": {
                "type": "integer",
                "minimum": 0,
                "description": "Current queued transaction count."
            },
            "capacity": {
                "type": "integer",
                "minimum": 0,
                "description": "Configured queue capacity."
            },
            "saturated": {
                "type": "boolean",
                "description": "Whether the queue was saturated when the error was emitted."
            }
        }
    })
}

fn axt_error_details_schema() -> Value {
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "code": {
                "type": "string",
                "description": "Stable AXT rejection code."
            },
            "reason": {
                "type": "string",
                "description": "Human-readable AXT rejection label."
            },
            "snapshot_version": {
                "type": "integer",
                "minimum": 0,
                "description": "AXT policy snapshot version used to reject the request."
            },
            "dataspace": {
                "type": "integer",
                "minimum": 0,
                "description": "Dataspace id involved in the rejection."
            },
            "lane": {
                "type": "integer",
                "minimum": 0,
                "description": "Lane id involved in the rejection."
            },
            "next_min_handle_era": {
                "type": "integer",
                "minimum": 0,
                "description": "Minimum handle era a client should use for retry."
            },
            "next_min_sub_nonce": {
                "type": "integer",
                "minimum": 0,
                "description": "Minimum sub-nonce a client should use for retry."
            }
        }
    })
}

fn error_details_schema() -> Value {
    norito::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "layer": {
                "type": "string",
                "description": "Public surface layer that produced the error."
            },
            "reject_code": {
                "type": "string",
                "description": "ISO-20022-style or Torii-local rejection code when available."
            },
            "queue": {
                "$ref": "#/components/schemas/QueueErrorSnapshot"
            },
            "retry_after_seconds": {
                "type": "integer",
                "minimum": 0,
                "description": "Suggested retry delay in seconds for transient errors."
            },
            "endpoint": {
                "type": "string",
                "description": "Endpoint associated with throttling or version failures."
            },
            "field": {
                "type": "string",
                "description": "Field associated with validation or decode failures."
            },
            "expected": {
                "type": "string",
                "description": "Expected field value, status, profile, or discriminant."
            },
            "actual": {
                "type": "string",
                "description": "Actual field value, status, profile, or discriminant."
            },
            "profile": {
                "type": "string",
                "description": "Network profile involved in the error."
            },
            "chain_discriminant": {
                "type": "integer",
                "minimum": 0,
                "maximum": 65535,
                "description": "I105 chain discriminant involved in the error."
            },
            "tx_hash": {
                "type": "string",
                "description": "Signed transaction hash involved in finality/status failures."
            },
            "last_status": {
                "type": "string",
                "description": "Last observed transaction status when a finality wait failed."
            },
            "hint": {
                "type": "string",
                "description": "Actionable debugging hint for callers."
            },
            "axt": {
                "$ref": "#/components/schemas/AxtErrorDetails"
            }
        }
    })
}

fn shared_error_schema() -> Value {
    norito::json!({
        "type": "object",
        "required": ["code", "message"],
        "additionalProperties": false,
        "properties": {
            "code": {
                "type": "string",
                "description": "Application-specific error identifier."
            },
            "message": {
                "type": "string",
                "description": "Human readable error message."
            },
            "details": {
                "anyOf": [
                    { "$ref": "#/components/schemas/ErrorDetails" },
                    { "type": "null" }
                ],
                "description": "Optional machine-readable error context such as queue pressure, retry hints, rejection codes, endpoint names, or AXT metadata."
            }
        }
    })
}

fn components_section() -> Value {
    let mut components = Map::new();
    let mut schemas = openapi_schemas();
    schemas.insert("AxtErrorDetails".to_owned(), axt_error_details_schema());
    schemas.insert(
        "QueueErrorSnapshot".to_owned(),
        queue_error_snapshot_schema(),
    );
    schemas.insert("ErrorDetails".to_owned(), error_details_schema());
    schemas.insert("ErrorEnvelope".to_owned(), shared_error_schema());
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
        assert!(paths.contains_key("/v1/aliases/by_account"));
        assert!(paths.contains_key("/v1/assets/aliases/resolve"));
        assert!(paths.contains_key("/v1/contracts/aliases"));
        assert!(paths.contains_key("/v1/contracts/aliases/resolve"));
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
        assert!(paths.contains_key("/v1/sccp/capabilities"));
        assert!(paths.contains_key("/v1/sccp/manifests"));
        assert!(paths.contains_key("/v1/sccp/artifacts/message/{message_id}"));
        assert!(paths.contains_key("/v1/sccp/jobs/message/{message_id}"));
        assert!(paths.contains_key("/v1/sumeragi/validator-sets"));
        assert!(paths.contains_key("/v1/sumeragi/validator-sets/{height}"));
        assert!(paths.contains_key("/v1/sumeragi/rbc/sessions"));
        let sumeragi_rbc_sessions_description = paths
            .get("/v1/sumeragi/rbc/sessions")
            .and_then(Value::as_object)
            .and_then(|path| path.get("get"))
            .and_then(Value::as_object)
            .and_then(|get| get.get("description"))
            .and_then(Value::as_str)
            .expect("Sumeragi RBC sessions description");
        assert!(sumeragi_rbc_sessions_description.contains("raw DELIVER"));
        assert!(sumeragi_rbc_sessions_description.contains("complete-delivery"));
        let sumeragi_rbc_delivered_description = paths
            .get("/v1/sumeragi/rbc/delivered/{height}/{view}")
            .and_then(Value::as_object)
            .and_then(|path| path.get("get"))
            .and_then(Value::as_object)
            .and_then(|get| get.get("description"))
            .and_then(Value::as_str)
            .expect("Sumeragi RBC delivered description");
        assert!(sumeragi_rbc_delivered_description.contains("complete-delivery"));
        assert!(sumeragi_rbc_delivered_description.contains("non-invalid"));
        assert!(paths.contains_key("/v1/contracts/view/batch"));
        assert!(paths.contains_key("/v1/contracts/rollups/swaps/fills"));
        assert!(paths.contains_key("/v1/contracts/rollups/swaps/candles"));
        assert!(paths.contains_key("/v1/contracts/rollups/uranai/markets/history"));
        assert!(paths.contains_key("/v1/contracts/rollups/trader/activity"));
        assert!(paths.contains_key("/v1/contracts/rollups/trader/account"));
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
        assert!(paths.contains_key("/v1/pipeline/preflight"));
        assert!(paths.contains_key(uri::TRANSACTION));
        assert!(paths.contains_key(uri::TRANSACTION_ENTRYPOINT));
        assert!(paths.contains_key(uri::TRANSACTIONS_BATCH));
        assert!(paths.contains_key(uri::QUERY));
        assert!(paths.contains_key(uri::SUBSCRIPTION));
        #[cfg(feature = "schema")]
        assert!(paths.contains_key(uri::SCHEMA));
        #[cfg(not(feature = "schema"))]
        assert!(!paths.contains_key(uri::SCHEMA));
        #[cfg(feature = "profiling")]
        assert!(paths.contains_key(uri::PROFILE));
        #[cfg(not(feature = "profiling"))]
        assert!(!paths.contains_key(uri::PROFILE));
        assert!(!paths.contains_key("/transaction"));
        assert!(!paths.contains_key("/transaction/entrypoint"));
        assert!(!paths.contains_key("/transactions/batch"));
        assert!(!paths.contains_key("/query"));
        assert!(!paths.contains_key("/events"));
        assert!(paths.contains_key("/v1/da/ingest"));
        assert!(paths.contains_key("/v1/connect/session"));
        assert!(paths.contains_key("/v1/vpn/profile"));
        assert!(paths.contains_key("/v1/vpn/quotes"));
        let vpn_quotes_post_description = paths
            .get("/v1/vpn/quotes")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .and_then(|post| post.get("description"))
            .and_then(Value::as_str)
            .expect("vpn quote create description");
        assert!(vpn_quotes_post_description.contains("metering_public_key_hex"));
        assert!(vpn_quotes_post_description.contains("OpenVpnLeaseEscrow"));
        assert!(paths.contains_key("/v1/vpn/sessions"));
        assert!(paths.contains_key("/v1/vpn/sessions/{session_id}"));
        assert!(paths.contains_key("/v1/vpn/receipts"));
        let vpn_receipts_post_description = paths
            .get("/v1/vpn/receipts")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .and_then(|post| post.get("description"))
            .and_then(Value::as_str)
            .expect("vpn receipt submit description");
        assert!(vpn_receipts_post_description.contains("tx_instructions"));
        assert!(vpn_receipts_post_description.contains("SettleVpnLease"));
        assert!(paths.contains_key("/v1/mcp"));
        assert!(paths.contains_key("/v1/zk/attachments"));
        assert!(paths.contains_key("/v1/multisig/propose"));
        assert!(paths.contains_key("/v1/multisig/approve"));
        assert!(paths.contains_key("/v1/contracts/call/multisig/propose"));
        assert!(paths.contains_key("/v1/contracts/call/multisig/approve"));
        assert!(paths.contains_key("/v1/multisig/cancel"));
        assert!(paths.contains_key("/v1/multisig/spec"));
        assert!(paths.contains_key("/v1/multisig/proposals/list"));
        assert!(paths.contains_key("/v1/multisig/proposals/get"));
        assert!(paths.contains_key("/v1/multisig/approvals/list"));
        assert!(paths.contains_key("/v1/multisig/approvals/get"));
        assert!(paths.contains_key("/v1/multisig/approvals/list_for_authority"));
        assert!(paths.contains_key("/v1/multisig/approvals/get_for_authority"));
        assert!(paths.contains_key("/v1/controls/asset-transfer/get"));
        assert!(paths.contains_key("/v1/ministry/agenda/proposals/draft"));
        assert!(paths.contains_key("/v1/ministry/agenda/proposals/{proposal_id}"));
        assert!(paths.contains_key("/v1/gov/proposals/deploy-contract"));
        assert!(paths.contains_key("/v1/gov/citizens"));
        assert!(paths.contains_key("/v1/gov/stream"));
        #[cfg(feature = "gov_vrf")]
        {
            assert!(paths.contains_key("/v1/gov/council/persist"));
            assert!(paths.contains_key("/v1/gov/council/replace"));
            assert!(paths.contains_key("/v1/gov/council/derive-vrf"));
        }
        #[cfg(not(feature = "gov_vrf"))]
        {
            assert!(!paths.contains_key("/v1/gov/council/persist"));
            assert!(!paths.contains_key("/v1/gov/council/replace"));
            assert!(!paths.contains_key("/v1/gov/council/derive-vrf"));
        }
        assert!(paths.contains_key("/v1/telemetry/live"));
        assert!(paths.contains_key("/v1/node/query/projection/checkpoint"));
        assert!(paths.contains_key("/v1/node/query/projection/checkpoint/plan"));
        assert!(paths.contains_key("/v1/node/query/projection/checkpoint/publish"));
        assert!(paths.contains_key("/v1/node/query/projection/catalog/{resource}"));
        assert!(paths.contains_key("/v1/node/query/projection/shards/{resource}/{partition_id}"));
        assert!(paths.contains_key("/v1/runtime/abi/active"));
        assert!(paths.contains_key("/v1/accounts"));
        assert!(paths.contains_key("/v1/transactions/history"));
        assert!(paths.contains_key("/v1/transactions/query"));
        assert!(paths.contains_key("/v1/transactions/visible/query"));
        assert!(paths.contains_key("/v1/contracts/activity"));
        assert!(paths.contains_key("/v1/contracts/events"));
        assert!(paths.contains_key("/v1/contracts/events/sse"));
        assert!(paths.contains_key("/v1/offline/readiness"));
        assert!(paths.contains_key("/v1/ram-lfe/program-policies"));
        assert!(paths.contains_key("/v1/ram-lfe/programs/{program_id}/execute"));
        assert!(paths.contains_key("/v1/ram-lfe/receipts/verify"));
        assert!(paths.contains_key("/v1/assets/definitions"));
        assert!(paths.contains_key("/v1/explorer/accounts"));
        assert!(paths.contains_key("/v1/sorafs/providers"));
        assert!(paths.contains_key("/v1/soradns/directory/latest"));
        assert!(paths.contains_key("/v1/content/{bundle}/{path}"));
        assert!(paths.contains_key("/v1/sns/names"));
        assert!(paths.contains_key("/v1/soranet/privacy/event"));
        assert!(paths.contains_key("/v1/webhooks"));
        assert!(paths.contains_key("/v1/notify/devices"));
        assert!(!paths.contains_key("/v1/offline/keys/refill"));
        assert!(!paths.contains_key("/v1/offline/notes/issue"));
        assert!(paths.contains_key("/v1/offline/v2/readiness"));
        assert!(paths.contains_key("/v1/offline/v2/keys/refill"));
        assert!(paths.contains_key("/v1/offline/v2/notes/issue"));
        assert!(paths.contains_key("/v1/offline/v2/notes/redeem"));
        assert!(paths.contains_key("/v1/offline/v2/audit"));
        assert!(!paths.contains_key("/v1/offline/notes/redeem"));
        assert!(!paths.contains_key("/v1/offline/audit"));
        assert!(!paths.contains_key("/v1/offline/policy"));
        assert!(!paths.contains_key("/v1/offline/revocations"));
        assert!(!paths.contains_key("/v1/offline/revocations/bundle"));
        let refill_post = paths
            .get("/v1/offline/v2/keys/refill")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("offline refill post operation");
        let refill_description = refill_post
            .get("description")
            .and_then(Value::as_str)
            .expect("offline refill description");
        assert!(refill_description.contains("signature_base64 or witness_base64"));
        assert!(refill_description.contains("X-Iroha-* app-auth headers are rejected"));
        let refill_request_schema = refill_post
            .get("requestBody")
            .and_then(Value::as_object)
            .and_then(|body| body.get("content"))
            .and_then(Value::as_object)
            .and_then(|content| content.get("application/json"))
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .expect("offline refill request schema");
        assert_eq!(
            refill_request_schema,
            "#/components/schemas/OfflineIssuerBodyAuthRequest"
        );
    }

    #[test]
    fn generated_spec_documents_offline_body_auth_schema() {
        let doc = generate_spec();
        let schema = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .and_then(|schemas| schemas.get("OfflineIssuerBodyAuthRequest"))
            .and_then(Value::as_object)
            .expect("Offline body auth schema");
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required fields");
        for field in ["account_id", "timestamp_ms", "nonce"] {
            assert!(required.contains(&Value::String(field.to_owned())));
        }
        let one_of = schema
            .get("oneOf")
            .and_then(Value::as_array)
            .expect("proof oneOf");
        assert_eq!(one_of.len(), 2);
        let description = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("schema description");
        assert!(description.contains("exactly one proof field"));
        assert!(description.contains("nested fields with those names remain signed"));
    }

    #[test]
    fn generated_spec_documents_structured_error_details() {
        let doc = generate_spec();
        let schemas = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("components schemas");
        let error_response = schemas
            .get("ErrorEnvelope")
            .and_then(Value::as_object)
            .expect("ErrorEnvelope schema");
        assert_eq!(
            error_response.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        let details = error_response
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("details"))
            .and_then(Value::as_object)
            .expect("ErrorEnvelope details property");
        let detail_refs = details
            .get("anyOf")
            .and_then(Value::as_array)
            .expect("details anyOf");
        assert!(detail_refs.iter().any(|entry| {
            entry
                .as_object()
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str)
                == Some("#/components/schemas/ErrorDetails")
        }));
        let error_details = schemas
            .get("ErrorDetails")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .expect("ErrorDetails properties");
        assert!(error_details.contains_key("reject_code"));
        assert!(error_details.contains_key("retry_after_seconds"));
        assert!(error_details.contains_key("queue"));
        assert!(error_details.contains_key("layer"));
        assert!(error_details.contains_key("endpoint"));
        assert!(error_details.contains_key("field"));
        assert!(error_details.contains_key("expected"));
        assert!(error_details.contains_key("actual"));
        assert!(error_details.contains_key("profile"));
        assert!(error_details.contains_key("chain_discriminant"));
        assert!(error_details.contains_key("tx_hash"));
        assert!(error_details.contains_key("last_status"));
        assert!(error_details.contains_key("hint"));
        assert!(error_details.contains_key("axt"));
        assert!(schemas.contains_key("QueueErrorSnapshot"));
        assert!(schemas.contains_key("AxtErrorDetails"));
    }

    #[test]
    fn generated_operations_declare_tool_effects() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        for (path, path_item) in paths {
            let path_map = path_item.as_object().expect("path item object");
            for method in ["get", "post", "put", "patch", "delete", "head", "options"] {
                let Some(operation) = path_map.get(method).and_then(Value::as_object) else {
                    continue;
                };
                let effect = operation
                    .get(TOOL_EFFECT_EXTENSION)
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| {
                        panic!("{method} {path} must declare {TOOL_EFFECT_EXTENSION}")
                    });
                assert!(
                    matches!(effect, "read" | "write" | "operator"),
                    "{method} {path} declared invalid effect {effect}"
                );
            }
        }

        let query = paths
            .get(uri::QUERY)
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("query post operation");
        assert_eq!(
            query.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
            Some("read")
        );

        let protected_namespaces = paths
            .get("/v1/gov/protected-namespaces")
            .and_then(Value::as_object)
            .and_then(|path| path.get("post"))
            .and_then(Value::as_object)
            .expect("protected namespaces post operation");
        assert_eq!(
            protected_namespaces
                .get(TOOL_EFFECT_EXTENSION)
                .and_then(Value::as_str),
            Some("operator")
        );
    }

    #[test]
    fn generated_spec_documents_vrf_signature_payloads() {
        fn post_request_schema_ref<'a>(doc: &'a Value, path: &str) -> Option<&'a str> {
            doc.get("paths")?
                .as_object()?
                .get(path)?
                .as_object()?
                .get("post")?
                .as_object()?
                .get("requestBody")?
                .as_object()?
                .get("content")?
                .as_object()?
                .get("application/json")?
                .as_object()?
                .get("schema")?
                .as_object()?
                .get("$ref")?
                .as_str()
        }

        let doc = generate_spec();
        assert_eq!(
            post_request_schema_ref(&doc, "/v1/sumeragi/vrf/commit"),
            Some("#/components/schemas/SumeragiVrfCommitRequest")
        );
        assert_eq!(
            post_request_schema_ref(&doc, "/v1/sumeragi/vrf/reveal"),
            Some("#/components/schemas/SumeragiVrfRevealRequest")
        );

        let schemas = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schemas section");
        for schema_name in ["SumeragiVrfCommitRequest", "SumeragiVrfRevealRequest"] {
            let required = schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("required"))
                .and_then(Value::as_array)
                .expect("VRF request required fields");
            assert!(
                required
                    .iter()
                    .any(|field| field.as_str() == Some("bls_sig_hex")),
                "{schema_name} should require bls_sig_hex"
            );
        }
    }

    #[test]
    fn identifier_policy_schema_exposes_ram_fhe_profile() {
        let doc = generate_spec();
        let schemas = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schemas section");
        let summary = schemas
            .get("IdentifierPolicySummary")
            .and_then(Value::as_object)
            .expect("IdentifierPolicySummary schema");
        let summary_properties = summary
            .get("properties")
            .and_then(Value::as_object)
            .expect("IdentifierPolicySummary properties");
        assert_eq!(
            summary_properties
                .get("ram_fhe_profile")
                .and_then(Value::as_object)
                .and_then(|field| field.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/BfvRamProgramProfile")
        );

        let profile = schemas
            .get("BfvRamProgramProfile")
            .and_then(Value::as_object)
            .expect("BfvRamProgramProfile schema");
        let required = profile
            .get("required")
            .and_then(Value::as_array)
            .expect("BfvRamProgramProfile required fields");
        assert!(
            required
                .iter()
                .any(|value| value.as_str() == Some("profile_version"))
        );
        assert!(
            required
                .iter()
                .any(|value| value.as_str() == Some("encrypted_input_mode"))
        );
        assert!(schemas.contains_key("BfvRamEncryptedInputMode"));
    }

    #[test]
    fn ram_lfe_execute_schema_references_receipt_payload() {
        let doc = generate_spec();
        let schemas = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schemas section");
        let execute = schemas
            .get("RamLfeExecuteResponse")
            .and_then(Value::as_object)
            .expect("RamLfeExecuteResponse schema");
        let properties = execute
            .get("properties")
            .and_then(Value::as_object)
            .expect("RamLfeExecuteResponse properties");
        assert!(!properties.contains_key("output_hex"));
        assert_eq!(
            properties
                .get("receipt")
                .and_then(Value::as_object)
                .and_then(|field| field.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/RamLfeExecutionReceipt")
        );
        assert!(schemas.contains_key("RamLfeExecutionReceiptPayload"));
    }

    #[test]
    fn account_and_asset_list_params_include_asset_related_filters() {
        fn params_for(doc: &Value, path: &str) -> Vec<String> {
            let paths = doc
                .get("paths")
                .and_then(Value::as_object)
                .expect("paths section");
            let entry = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("path entry");
            let get_op = entry.get("get").and_then(Value::as_object).expect("get op");
            get_op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|param| {
                    param
                        .as_object()
                        .and_then(|obj| obj.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        }

        let doc = generate_spec();
        let account_assets = params_for(&doc, "/v1/accounts/{account_id}/assets");
        assert!(account_assets.contains(&"limit".to_owned()));
        assert!(account_assets.contains(&"offset".to_owned()));
        assert!(account_assets.contains(&"asset".to_owned()));

        let account_transactions = params_for(&doc, "/v1/accounts/{account_id}/transactions");
        assert!(account_transactions.contains(&"limit".to_owned()));
        assert!(account_transactions.contains(&"offset".to_owned()));
        assert!(account_transactions.contains(&"asset_id".to_owned()));

        let contract_activity = params_for(&doc, "/v1/contracts/activity");
        assert!(contract_activity.contains(&"limit".to_owned()));
        assert!(contract_activity.contains(&"offset".to_owned()));
        assert!(contract_activity.contains(&"contract_alias".to_owned()));
        assert!(contract_activity.contains(&"contract_entrypoint".to_owned()));
        assert!(contract_activity.contains(&"result_ok".to_owned()));

        let contract_events = params_for(&doc, "/v1/contracts/events");
        assert!(contract_events.contains(&"limit".to_owned()));
        assert!(contract_events.contains(&"offset".to_owned()));
        assert!(contract_events.contains(&"module".to_owned()));
        assert!(contract_events.contains(&"event_kind".to_owned()));
        assert!(contract_events.contains(&"participant".to_owned()));
        assert!(contract_events.contains(&"asset_id".to_owned()));
        assert!(contract_events.contains(&"provenance".to_owned()));

        let asset_holders = params_for(&doc, "/v1/assets/{definition_id}/holders");
        assert!(asset_holders.contains(&"limit".to_owned()));
        assert!(asset_holders.contains(&"offset".to_owned()));
        assert!(asset_holders.contains(&"account_id".to_owned()));
        assert!(asset_holders.contains(&"scope".to_owned()));
    }

    #[test]
    fn portfolio_params_include_asset_id_filter() {
        fn params_for(doc: &Value, path: &str) -> Vec<String> {
            let paths = doc
                .get("paths")
                .and_then(Value::as_object)
                .expect("paths section");
            let entry = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("path entry");
            let get_op = entry.get("get").and_then(Value::as_object).expect("get op");
            get_op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|param| {
                    param
                        .as_object()
                        .and_then(|obj| obj.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        }

        let doc = generate_spec();
        let portfolio = params_for(&doc, "/v1/accounts/{uaid}/portfolio");
        assert!(portfolio.contains(&"asset_id".to_owned()));
    }

    #[test]
    fn public_lane_rewards_params_include_asset_id_filter() {
        fn params_for(doc: &Value, path: &str) -> Vec<String> {
            let paths = doc
                .get("paths")
                .and_then(Value::as_object)
                .expect("paths section");
            let entry = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("path entry");
            let get_op = entry.get("get").and_then(Value::as_object).expect("get op");
            get_op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|param| {
                    param
                        .as_object()
                        .and_then(|obj| obj.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        }

        let doc = generate_spec();
        let rewards = params_for(&doc, "/v1/nexus/public_lanes/{lane_id}/rewards/pending");
        assert!(rewards.contains(&"asset_id".to_owned()));
        assert!(rewards.contains(&"account".to_owned()));
    }

    #[test]
    fn explorer_assets_params_include_asset_id_filter() {
        fn params_for(doc: &Value, path: &str) -> Vec<String> {
            let paths = doc
                .get("paths")
                .and_then(Value::as_object)
                .expect("paths section");
            let entry = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("path entry");
            let get_op = entry.get("get").and_then(Value::as_object).expect("get op");
            get_op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|param| {
                    param
                        .as_object()
                        .and_then(|obj| obj.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        }

        let doc = generate_spec();
        let explorer_assets = params_for(&doc, "/v1/explorer/assets");
        assert!(explorer_assets.contains(&"page".to_owned()));
        assert!(explorer_assets.contains(&"per_page".to_owned()));
        assert!(explorer_assets.contains(&"asset_id".to_owned()));
        assert!(explorer_assets.contains(&"owned_by".to_owned()));
        assert!(explorer_assets.contains(&"definition".to_owned()));
    }

    #[test]
    fn explorer_transactions_params_include_asset_id_filter() {
        fn params_for(doc: &Value, path: &str) -> Vec<String> {
            let paths = doc
                .get("paths")
                .and_then(Value::as_object)
                .expect("paths section");
            let entry = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("path entry");
            let get_op = entry.get("get").and_then(Value::as_object).expect("get op");
            get_op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|param| {
                    param
                        .as_object()
                        .and_then(|obj| obj.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        }

        let doc = generate_spec();
        let explorer_transactions = params_for(&doc, "/v1/explorer/transactions");
        assert!(explorer_transactions.contains(&"page".to_owned()));
        assert!(explorer_transactions.contains(&"per_page".to_owned()));
        assert!(explorer_transactions.contains(&"asset_id".to_owned()));
        assert!(explorer_transactions.contains(&"authority".to_owned()));
        assert!(explorer_transactions.contains(&"block".to_owned()));
        assert!(explorer_transactions.contains(&"status".to_owned()));
    }

    #[test]
    fn explorer_instructions_params_include_asset_id_filter() {
        fn params_for(doc: &Value, path: &str) -> Vec<String> {
            let paths = doc
                .get("paths")
                .and_then(Value::as_object)
                .expect("paths section");
            let entry = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("path entry");
            let get_op = entry.get("get").and_then(Value::as_object).expect("get op");
            get_op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|param| {
                    param
                        .as_object()
                        .and_then(|obj| obj.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        }

        let doc = generate_spec();
        let explorer_instructions = params_for(&doc, "/v1/explorer/instructions");
        assert!(explorer_instructions.contains(&"page".to_owned()));
        assert!(explorer_instructions.contains(&"per_page".to_owned()));
        assert!(explorer_instructions.contains(&"asset_id".to_owned()));
        assert!(explorer_instructions.contains(&"authority".to_owned()));
        assert!(explorer_instructions.contains(&"account".to_owned()));
        assert!(explorer_instructions.contains(&"transaction_hash".to_owned()));
        assert!(explorer_instructions.contains(&"transaction_status".to_owned()));
        assert!(explorer_instructions.contains(&"block".to_owned()));
        assert!(explorer_instructions.contains(&"kind".to_owned()));
    }

    #[test]
    fn explorer_instructions_account_param_description_mentions_non_transfer_matches() {
        fn parameter_description(doc: &Value, path: &str, name: &str) -> String {
            let paths = doc
                .get("paths")
                .and_then(Value::as_object)
                .expect("paths section");
            let entry = paths
                .get(path)
                .and_then(Value::as_object)
                .expect("path entry");
            let get_op = entry.get("get").and_then(Value::as_object).expect("get op");
            get_op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find_map(|param| {
                    let obj = param.as_object()?;
                    let param_name = obj.get("name")?.as_str()?;
                    (param_name == name)
                        .then(|| obj.get("description")?.as_str().map(str::to_owned))
                        .flatten()
                })
                .expect("parameter description")
        }

        let doc = generate_spec();
        let description =
            parameter_description(&doc, "/v1/explorer/instructions", "account").to_lowercase();
        assert!(description.contains("mint/burn"));
        assert!(description.contains("multisig"));
        assert!(description.contains("reward"));
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
        let response_404 = responses
            .get("404")
            .and_then(Value::as_object)
            .expect("404 response");
        let error_content = response_404
            .get("content")
            .and_then(Value::as_object)
            .expect("pipeline status error content");
        assert!(error_content.contains_key("application/json"));
        assert!(error_content.contains_key("application/x-norito"));
        let response_200 = responses
            .get("200")
            .and_then(Value::as_object)
            .expect("200 response");
        let content = response_200
            .get("content")
            .and_then(Value::as_object)
            .expect("pipeline status content");
        assert!(
            content.contains_key("application/json"),
            "pipeline status should document JSON output"
        );
        assert!(
            content.contains_key("application/x-norito"),
            "pipeline status should document typed Norito output"
        );
        let params = get
            .get("parameters")
            .and_then(Value::as_array)
            .expect("pipeline status params");
        let has_scope = params.iter().any(|param| {
            param
                .as_object()
                .and_then(|obj| obj.get("name"))
                .and_then(Value::as_str)
                == Some("scope")
        });
        assert!(
            has_scope,
            "pipeline status should document scope query hint"
        );
    }

    #[test]
    fn pipeline_preflight_documents_typed_response() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        let preflight = paths
            .get("/v1/pipeline/preflight")
            .and_then(Value::as_object)
            .expect("pipeline preflight path");
        let get = preflight
            .get("get")
            .and_then(Value::as_object)
            .expect("get op");
        let responses = get
            .get("responses")
            .and_then(Value::as_object)
            .expect("responses");
        let response_200 = responses
            .get("200")
            .and_then(Value::as_object)
            .expect("200 response");
        let content = response_200
            .get("content")
            .and_then(Value::as_object)
            .expect("pipeline preflight content");
        assert!(content.contains_key("application/json"));
        assert!(content.contains_key("application/x-norito"));
        let schema_ref = content
            .get("application/json")
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str);
        assert_eq!(
            schema_ref,
            Some("#/components/schemas/PipelinePreflightResponse")
        );
        let schemas = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schemas section");
        assert!(schemas.contains_key("PipelinePreflightResponse"));
    }

    #[test]
    fn account_get_documents_canonical_dual_format_read() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        let account_get = paths
            .get("/v1/accounts/{account_id}")
            .and_then(Value::as_object)
            .expect("account get path");
        let get = account_get
            .get("get")
            .and_then(Value::as_object)
            .expect("get op");
        let responses = get
            .get("responses")
            .and_then(Value::as_object)
            .expect("responses");
        assert!(
            responses.contains_key("404"),
            "account get should document missing-account behavior"
        );
        let response_404 = responses
            .get("404")
            .and_then(Value::as_object)
            .expect("404 response");
        let error_content = response_404
            .get("content")
            .and_then(Value::as_object)
            .expect("account get error content");
        assert!(error_content.contains_key("application/json"));
        assert!(error_content.contains_key("application/x-norito"));
        let response_200 = responses
            .get("200")
            .and_then(Value::as_object)
            .expect("200 response");
        let content = response_200
            .get("content")
            .and_then(Value::as_object)
            .expect("account get content");
        assert!(content.contains_key("application/json"));
        assert!(content.contains_key("application/x-norito"));
    }

    #[test]
    fn signed_submission_routes_document_dual_error_envelopes() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");

        for path in [uri::TRANSACTION, uri::TRANSACTION_ENTRYPOINT, uri::QUERY] {
            let path_item = paths
                .get(path)
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{path} path"));
            let post = path_item
                .get("post")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{path} post operation"));
            let request_content = post
                .get("requestBody")
                .and_then(Value::as_object)
                .and_then(|body| body.get("content"))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{path} request content"));
            assert!(request_content.contains_key("application/json"));
            assert!(request_content.contains_key("application/x-norito"));
            assert!(!request_content.contains_key("application/octet-stream"));

            let responses = post
                .get("responses")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{path} responses"));
            for status in ["400", "406", "415"] {
                let content = responses
                    .get(status)
                    .and_then(Value::as_object)
                    .and_then(|response| response.get("content"))
                    .and_then(Value::as_object)
                    .unwrap_or_else(|| panic!("{path} {status} content"));
                assert!(content.contains_key("application/json"));
                assert!(content.contains_key("application/x-norito"));
                let schema_ref = content
                    .get("application/json")
                    .and_then(Value::as_object)
                    .and_then(|media| media.get("schema"))
                    .and_then(Value::as_object)
                    .and_then(|schema| schema.get("$ref"))
                    .and_then(Value::as_str);
                assert_eq!(schema_ref, Some("#/components/schemas/ErrorEnvelope"));
            }
        }
    }

    #[test]
    fn kaigi_typed_routes_document_dual_responses() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");

        for path in [
            "/v1/kaigi/relays",
            "/v1/kaigi/relays/{relay_id}",
            "/v1/kaigi/relays/health",
        ] {
            let content = paths
                .get(path)
                .and_then(Value::as_object)
                .and_then(|path_item| path_item.get("get"))
                .and_then(Value::as_object)
                .and_then(|operation| operation.get("responses"))
                .and_then(Value::as_object)
                .and_then(|responses| responses.get("200"))
                .and_then(Value::as_object)
                .and_then(|response| response.get("content"))
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{path} 200 response content"));
            assert!(content.contains_key("application/json"));
            assert!(content.contains_key("application/x-norito"));
        }
    }

    #[test]
    fn multisig_propose_documents_native_norito_request_body() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        let propose = paths
            .get("/v1/multisig/propose")
            .and_then(Value::as_object)
            .expect("multisig propose path");
        let post = propose
            .get("post")
            .and_then(Value::as_object)
            .expect("post operation");
        let request_body = post
            .get("requestBody")
            .and_then(Value::as_object)
            .expect("request body");
        let content = request_body
            .get("content")
            .and_then(Value::as_object)
            .expect("request content");
        assert!(content.contains_key("application/json"));
        assert!(content.contains_key("application/x-norito"));

        let norito_schema_type = content
            .get("application/x-norito")
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("type"))
            .and_then(Value::as_str);
        assert_eq!(norito_schema_type, Some("string"));

        let schemas = doc
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schemas");
        let instruction_schema_ref = schemas
            .get("MultisigProposeRequest")
            .and_then(Value::as_object)
            .and_then(|request| request.get("allOf"))
            .and_then(Value::as_array)
            .and_then(|all_of| all_of.get(1))
            .and_then(Value::as_object)
            .and_then(|payload| payload.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("instructions"))
            .and_then(Value::as_object)
            .and_then(|instructions| instructions.get("items"))
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str);
        assert_eq!(
            instruction_schema_ref,
            Some("#/components/schemas/MultisigProposeInstructionInput")
        );

        let instruction_input = schemas
            .get("MultisigProposeInstructionInput")
            .and_then(Value::as_object)
            .expect("instruction input schema");
        assert_eq!(
            instruction_input.get("type").and_then(Value::as_str),
            Some("object")
        );
        assert!(
            !instruction_input.contains_key("oneOf"),
            "JSON instructions must not advertise per-instruction Norito blobs"
        );
        assert!(
            instruction_input
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(
                    |description| description.contains("entire MultisigProposeRequest body")
                ),
            "instruction schema should direct native callers to whole-body Norito"
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
        assert!(!content.contains_key("application/octet-stream"));

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
                expected: "/v1/offline/readiness",
            },
            PathCase {
                label: "system",
                builder: system_paths,
                expected: "/health",
            },
            PathCase {
                label: "operator_auth",
                builder: operator_auth_paths,
                expected: "/v1/operator/auth/login/options",
            },
            PathCase {
                label: "transactions",
                builder: transaction_paths,
                expected: uri::TRANSACTION,
            },
            PathCase {
                label: "queries",
                builder: query_paths,
                expected: uri::QUERY,
            },
            PathCase {
                label: "streams",
                builder: stream_paths,
                expected: uri::SUBSCRIPTION,
            },
            PathCase {
                label: "connect",
                builder: connect_paths,
                expected: "/v1/connect/session",
            },
            PathCase {
                label: "mcp",
                builder: mcp_paths,
                expected: "/v1/mcp",
            },
            PathCase {
                label: "proofs",
                builder: proof_paths,
                expected: "/v1/proofs/retention",
            },
            PathCase {
                label: "contracts",
                builder: contracts_paths,
                expected: "/v1/contracts/call",
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
                expected: "/v1/node/query/projection/checkpoint",
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
                label: "rwas",
                builder: rwa_paths,
                expected: "/v1/rwas",
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
                label: "soracloud",
                builder: soracloud_paths,
                expected: "/v1/soracloud/model/upload/private/execute",
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
                expected: "/v1/sns/names",
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
            "AppPageMetadata",
            "AccountQueryResponse",
            "DomainQueryResponse",
            "AccountAssetQueryResponse",
            "AssetHolderQueryResponse",
            "NftQueryResponse",
            "RwaQueryResponse",
            "RepoAgreementListResponse",
            "ApiVersionInfo",
            "PeerIdList",
            "PrivateUploadedModelExecuteRequest",
            "PrivateUploadedModelExecuteResponse",
            "PrivateUploadedModelReceiptListResponse",
            "PushRegisterDeviceRequest",
        ] {
            assert!(schemas.contains_key(key), "schema missing {key}");
        }
    }

    #[test]
    fn generated_spec_documents_soracloud_private_uploaded_model_routes() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        assert!(paths.contains_key("/v1/soracloud/model/upload/private/execute"));
        assert!(paths.contains_key("/v1/soracloud/model/upload/private/receipts"));

        let schemas = doc
            .get("components")
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schema section");
        let response = schemas
            .get("PrivateUploadedModelReceiptListResponse")
            .and_then(Value::as_object)
            .expect("receipt-list schema");
        let properties = response
            .get("properties")
            .and_then(Value::as_object)
            .expect("receipt-list properties");
        for key in [
            "total",
            "returned_items",
            "remaining_items",
            "has_more",
            "count_mode",
            "continue_cursor",
        ] {
            assert!(properties.contains_key(key), "metadata field missing {key}");
        }
    }

    #[test]
    fn generated_spec_documents_app_query_page_metadata() {
        let doc = generate_spec();
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .expect("paths section");
        for (path, method, expected_ref) in [
            (
                "/v1/accounts",
                "get",
                "#/components/schemas/AccountListResponse",
            ),
            (
                "/v1/accounts/query",
                "post",
                "#/components/schemas/AccountQueryResponse",
            ),
            (
                "/v1/domains",
                "get",
                "#/components/schemas/DomainListResponse",
            ),
            (
                "/v1/domains/query",
                "post",
                "#/components/schemas/DomainQueryResponse",
            ),
            (
                "/v1/accounts/{account_id}/assets",
                "get",
                "#/components/schemas/AccountAssetListResponse",
            ),
            (
                "/v1/accounts/{account_id}/assets/query",
                "post",
                "#/components/schemas/AccountAssetQueryResponse",
            ),
            (
                "/v1/assets/definitions",
                "get",
                "#/components/schemas/AssetDefinitionListResponse",
            ),
            (
                "/v1/assets/definitions/query",
                "post",
                "#/components/schemas/AssetDefinitionQueryResponse",
            ),
            (
                "/v1/assets/{definition_id}/holders",
                "get",
                "#/components/schemas/AssetHolderListResponse",
            ),
            (
                "/v1/assets/{definition_id}/holders/query",
                "post",
                "#/components/schemas/AssetHolderQueryResponse",
            ),
            ("/v1/nfts", "get", "#/components/schemas/NftListResponse"),
            (
                "/v1/nfts/query",
                "post",
                "#/components/schemas/NftQueryResponse",
            ),
            ("/v1/rwas", "get", "#/components/schemas/RwaListResponse"),
            (
                "/v1/rwas/query",
                "post",
                "#/components/schemas/RwaQueryResponse",
            ),
            (
                "/v1/repo/agreements",
                "get",
                "#/components/schemas/RepoAgreementListResponse",
            ),
            (
                "/v1/repo/agreements/query",
                "post",
                "#/components/schemas/RepoAgreementListResponse",
            ),
        ] {
            let schema_ref = paths
                .get(path)
                .and_then(Value::as_object)
                .and_then(|path_item| path_item.get(method))
                .and_then(Value::as_object)
                .and_then(|operation| operation.get("responses"))
                .and_then(Value::as_object)
                .and_then(|responses| responses.get("200"))
                .and_then(Value::as_object)
                .and_then(|response| response.get("content"))
                .and_then(Value::as_object)
                .and_then(|content| content.get("application/json"))
                .and_then(Value::as_object)
                .and_then(|media| media.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str)
                .expect("path response schema ref");
            assert_eq!(schema_ref, expected_ref, "{method} {path}");
        }

        let schemas = doc
            .get("components")
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object)
            .expect("schema section");
        let metadata = schemas
            .get("AppPageMetadata")
            .and_then(Value::as_object)
            .expect("app page metadata schema");
        let required = metadata
            .get("required")
            .and_then(Value::as_array)
            .expect("metadata required fields");
        for field in ["items", "has_more", "count_mode"] {
            assert!(
                required.iter().any(|value| value.as_str() == Some(field)),
                "metadata required fields should include {field}"
            );
        }
        assert!(
            !required.iter().any(|value| value.as_str() == Some("total")),
            "bounded count responses must not require total"
        );
        let properties = metadata
            .get("properties")
            .and_then(Value::as_object)
            .expect("metadata properties");
        for field in [
            "total",
            "has_more",
            "count_mode",
            "indexed_height",
            "indexed_block_hash",
            "query_source",
        ] {
            assert!(
                properties.contains_key(field),
                "metadata properties should include {field}"
            );
        }

        let repo_request_properties = schemas
            .get("RepoAgreementsQueryRequest")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .expect("repo query request properties");
        for field in ["filter", "select", "aggregate", "fetch_size", "count_mode"] {
            assert!(
                repo_request_properties.contains_key(field),
                "repo query request should document {field}"
            );
        }
    }

    #[test]
    fn tags_section_includes_push_tag() {
        let tags = match tags_section() {
            Value::Array(tags) => tags,
            _ => panic!("tags section should be an array"),
        };
        let mut has_push = false;
        let mut has_soracloud = false;
        let mut has_vpn = false;
        for tag in tags {
            let Some(obj) = tag.as_object() else { continue };
            if obj.get("name").and_then(Value::as_str) == Some("Push") {
                has_push = true;
            }
            if obj.get("name").and_then(Value::as_str) == Some("Soracloud") {
                has_soracloud = true;
            }
            if obj.get("name").and_then(Value::as_str) == Some("VPN") {
                has_vpn = true;
            }
        }
        assert!(has_push, "tags should include Push");
        assert!(has_soracloud, "tags should include Soracloud");
        assert!(has_vpn, "tags should include VPN");
    }
}
