//! Mock control-plane server and CLI stub for SNNet-15D surfaces.
//!
//! Serves deterministic JSON responses backed by the seed OpenAPI/RBAC files so
//! SDK/CLI integrations can exercise the shape before the real service lands.

use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use blake3::Hasher;
use eyre::WrapErr;
use reqwest::{
    blocking::Client,
    header::{HeaderMap as ReqwestHeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;

const DEFAULT_LISTEN: &str = "127.0.0.1:18080";
const DEFAULT_OPENAPI: &str = "docs/source/soranet/control_plane_openapi.yaml";
const DEFAULT_RBAC: &str = "docs/source/soranet/control_plane_rbac.yaml";

#[derive(Clone)]
struct MockState {
    openapi_hash: String,
    rbac_hash: String,
    domains: Vec<Domain>,
    audit: Vec<AuditEvent>,
    analytics: Vec<AnalyticsPoint>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Domain {
    id: String,
    org_id: String,
    project_id: Option<String>,
    hostname: String,
    status: String,
    cache_profile: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct AuditEvent {
    id: String,
    actor: String,
    scope: String,
    action: String,
    target: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyticsPoint {
    metric: String,
    ts: String,
    value: f64,
}

fn hash_file(path: &Path) -> eyre::Result<String> {
    let data =
        std::fs::read(path).wrap_err_with(|| eyre::eyre!("failed to read {}", path.display()))?;
    let mut hasher = Hasher::new();
    hasher.update(&data);
    Ok(hasher.finalize().to_hex().to_string())
}

fn build_state(openapi: &Path, rbac: &Path) -> eyre::Result<Arc<MockState>> {
    let openapi_hash = hash_file(openapi)?;
    let rbac_hash = hash_file(rbac)?;
    let domains = vec![
        Domain {
            id: "00000000-0000-0000-0000-000000000001".to_owned(),
            org_id: "org-demo".to_owned(),
            project_id: Some("project-default".to_owned()),
            hostname: "demo.soranet.dev".to_owned(),
            status: "active".to_owned(),
            cache_profile: "core".to_owned(),
        },
        Domain {
            id: "00000000-0000-0000-0000-000000000002".to_owned(),
            org_id: "org-demo".to_owned(),
            project_id: Some("project-canary".to_owned()),
            hostname: "canary.soranet.dev".to_owned(),
            status: "pending_validation".to_owned(),
            cache_profile: "null".to_owned(),
        },
    ];
    let audit = vec![AuditEvent {
        id: "e1e98f2c-2922-4fda-bc81-7b1af865ad31".to_owned(),
        actor: "org-owner@example.com".to_owned(),
        scope: "domain:write".to_owned(),
        action: "create_domain".to_owned(),
        target: "demo.soranet.dev".to_owned(),
    }];
    let analytics = vec![AnalyticsPoint {
        metric: "requests".to_owned(),
        ts: "2026-11-20T12:00:00Z".to_owned(),
        value: 1337.0,
    }];

    Ok(Arc::new(MockState {
        openapi_hash,
        rbac_hash,
        domains,
        audit,
        analytics,
    }))
}

#[derive(Clone)]
struct Tenant {
    org: String,
    project: Option<String>,
}

fn require_tenant(headers: &HeaderMap) -> Result<Tenant, (StatusCode, Json<Value>)> {
    let org = headers
        .get("X-SN-Org")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned);
    let org = match org {
        Some(value) => value,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "missing X-SN-Org header"})),
            ));
        }
    };
    let project = headers
        .get("X-SN-Project")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned);
    Ok(Tenant { org, project })
}

fn router(state: Arc<MockState>) -> Router {
    Router::new()
        .route("/meta", get(meta))
        .route("/v1/domains", get(list_domains).post(create_domain))
        .route("/v1/domains/{domain_id}/purge", post(purge_domain))
        .route("/v1/audit/events", get(list_audit))
        .route("/v1/analytics/query", post(run_analytics))
        .with_state(state)
}

async fn meta(State(state): State<Arc<MockState>>) -> Json<Value> {
    Json(json!({
        "openapi_hash": state.openapi_hash,
        "rbac_hash": state.rbac_hash,
    }))
}

async fn list_domains(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _tenant = require_tenant(&headers)?;
    Ok(Json(json!({ "items": state.domains })))
}

async fn create_domain(
    State(_state): State<Arc<MockState>>,
    headers: HeaderMap,
    Json(mut payload): Json<HashMap<String, Value>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant = require_tenant(&headers)?;
    let hostname = payload
        .remove("hostname")
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "placeholder.soranet.dev".to_owned());
    let domain = Domain {
        id: "00000000-0000-0000-0000-000000000099".to_owned(),
        org_id: tenant.org,
        project_id: tenant.project,
        hostname,
        status: "pending_validation".to_owned(),
        cache_profile: "core".to_owned(),
    };
    Ok(Json(json!(domain)))
}

async fn purge_domain(
    AxumPath(domain_id): AxumPath<String>,
    headers: HeaderMap,
    Json(payload): Json<PurgeRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let _tenant = require_tenant(&headers)?;
    let response = json!({
        "domain_id": domain_id,
        "kind": payload.kind,
        "entries": payload.entries,
        "request_id": payload.request_id.unwrap_or_else(|| "stub-request".to_owned()),
        "status": "accepted"
    });
    Ok((StatusCode::ACCEPTED, Json(response)))
}

#[derive(Deserialize)]
struct PurgeRequest {
    kind: String,
    entries: Vec<String>,
    request_id: Option<String>,
}

async fn list_audit(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _tenant = require_tenant(&headers)?;
    Ok(Json(json!({ "events": state.audit })))
}

async fn run_analytics(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let _tenant = require_tenant(&headers)?;
    Ok(Json(json!({ "series": state.analytics })))
}

async fn serve(listen: SocketAddr, openapi: PathBuf, rbac: PathBuf) -> eyre::Result<()> {
    let state = build_state(&openapi, &rbac)?;
    let listener = TcpListener::bind(listen).await?;
    let app = router(state);
    println!("control-plane-mock listening on http://{listen}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn run_cli(endpoint: &str, cli: CliCommand) -> eyre::Result<()> {
    let headers = default_cli_headers()?;
    let client = Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .wrap_err("failed to build HTTP client")?;
    match cli {
        CliCommand::Meta => {
            let data: Value = client
                .get(format!("{endpoint}/meta"))
                .send()?
                .error_for_status()?
                .json()?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        CliCommand::Domains => {
            let data: Value = client
                .get(format!("{endpoint}/v1/domains"))
                .send()?
                .error_for_status()?
                .json()?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        CliCommand::Audit => {
            let data: Value = client
                .get(format!("{endpoint}/v1/audit/events"))
                .send()?
                .error_for_status()?
                .json()?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Mode {
    Serve,
    Cli(CliCommand),
}

#[derive(Clone, Copy)]
enum CliCommand {
    Meta,
    Domains,
    Audit,
}

fn parse_args_from<I>(raw: I) -> eyre::Result<(Mode, SocketAddr, PathBuf, PathBuf, String)>
where
    I: Iterator<Item = String>,
{
    let mut mode = Mode::Serve;
    let mut listen: SocketAddr = DEFAULT_LISTEN.parse()?;
    let mut openapi = default_openapi_path();
    let mut rbac = default_rbac_path();
    let mut endpoint = format!("http://{DEFAULT_LISTEN}");
    let mut args = raw.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "serve" => mode = Mode::Serve,
            "cli" => {
                let sub = args
                    .next()
                    .unwrap_or_else(|| "meta".to_owned())
                    .to_lowercase();
                let cmd = match sub.as_str() {
                    "meta" => CliCommand::Meta,
                    "domains" => CliCommand::Domains,
                    "audit" => CliCommand::Audit,
                    _ => CliCommand::Meta,
                };
                mode = Mode::Cli(cmd);
            }
            "--listen" => {
                let addr = args
                    .next()
                    .ok_or_else(|| eyre::eyre!("--listen requires an address"))?;
                listen = addr.parse()?;
                endpoint = format!("http://{listen}");
            }
            "--openapi" => {
                let path = args
                    .next()
                    .ok_or_else(|| eyre::eyre!("--openapi requires a path"))?;
                openapi = PathBuf::from(path);
            }
            "--rbac" => {
                let path = args
                    .next()
                    .ok_or_else(|| eyre::eyre!("--rbac requires a path"))?;
                rbac = PathBuf::from(path);
            }
            "--endpoint" => {
                endpoint = args
                    .next()
                    .ok_or_else(|| eyre::eyre!("--endpoint requires a value"))?;
            }
            _ => {}
        }
    }
    Ok((mode, listen, openapi, rbac, endpoint))
}

fn parse_args() -> eyre::Result<(Mode, SocketAddr, PathBuf, PathBuf, String)> {
    parse_args_from(env::args().skip(1))
}

fn default_cli_headers() -> eyre::Result<ReqwestHeaderMap> {
    let mut headers = ReqwestHeaderMap::new();
    headers.insert("X-SN-Org", HeaderValue::from_static("org-demo"));
    headers.insert("X-SN-Project", HeaderValue::from_static("project-default"));
    Ok(headers)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_openapi_path() -> PathBuf {
    workspace_root().join(DEFAULT_OPENAPI)
}

fn default_rbac_path() -> PathBuf {
    workspace_root().join(DEFAULT_RBAC)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> eyre::Result<()> {
    let (mode, listen, openapi, rbac, endpoint) = parse_args()?;
    match mode {
        Mode::Serve => serve(listen, openapi, rbac).await,
        Mode::Cli(cmd) => run_cli(&endpoint, cmd),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Request;
    use tower::ServiceExt as _;

    use super::*;

    fn fixture_state() -> Arc<MockState> {
        build_state(&default_openapi_path(), &default_rbac_path()).unwrap()
    }

    #[tokio::test]
    async fn hashes_openapi_and_rbac() {
        let state = fixture_state();
        assert_eq!(
            state.openapi_hash,
            "2890463a18e8523ddfb8bd8b6d7ea80afedf4bcbc0f43171fb84cf83b6fd89d6"
        );
        assert_eq!(
            state.rbac_hash,
            "41073ac29e58a3227f0e2ecd7dc941825e8e8864f9140bbaf220ec463bd08aea"
        );
    }

    #[tokio::test]
    async fn router_exposes_meta_and_domains() {
        let state = fixture_state();
        let app = router(state);

        let meta = app
            .clone()
            .oneshot(
                Request::get("/meta")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(meta.status(), StatusCode::OK);

        let domains = app
            .oneshot(
                Request::get("/v1/domains")
                    .header("X-SN-Org", "org-demo")
                    .header("X-SN-Project", "project-default")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(domains.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn domains_require_tenant_headers() {
        let state = fixture_state();
        let app = router(state);

        let resp = app
            .clone()
            .oneshot(
                Request::get("/v1/domains")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let ok_resp = app
            .oneshot(
                Request::get("/v1/domains")
                    .header("X-SN-Org", "org-demo")
                    .header("X-SN-Project", "project-default")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ok_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn purge_returns_accepted() {
        let state = fixture_state();
        let app = router(state);
        let req = Request::post("/v1/domains/00000000-0000-0000-0000-000000000001/purge")
            .header("content-type", "application/json")
            .header("X-SN-Org", "org-demo")
            .body(axum::body::Body::from(
                r#"{"kind":"path","entries":["/foo"],"request_id":"test"}"#,
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[test]
    fn parses_cli_mode_and_overrides() {
        let args = vec![
            "cli".to_owned(),
            "domains".to_owned(),
            "--endpoint".to_owned(),
            "http://localhost:9999".to_owned(),
        ];
        let (mode, listen, openapi, rbac, endpoint) = parse_args_from(args.into_iter()).unwrap();
        match mode {
            Mode::Cli(CliCommand::Domains) => {}
            _ => panic!("expected cli domains mode"),
        }
        assert_eq!(listen, DEFAULT_LISTEN.parse().unwrap());
        assert!(openapi.ends_with("control_plane_openapi.yaml"));
        assert!(rbac.ends_with("control_plane_rbac.yaml"));
        assert_eq!(endpoint, "http://localhost:9999");
    }

    #[tokio::test]
    async fn cli_fetches_meta_from_live_server() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("skipping cli_fetches_meta_from_live_server: {err}");
                return;
            }
            Err(err) => panic!("failed to bind test listener: {err}"),
        };
        let addr = listener.local_addr().unwrap();
        let state = fixture_state();
        let app = router(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        tokio::task::spawn_blocking(move || {
            run_cli(&format!("http://{addr}"), CliCommand::Meta).unwrap();
        })
        .await
        .unwrap();

        server.abort();
    }
}
