//! Gateway route planning, security headers and rollback regressions.
use super::*;
use std::fs;
#[test]
fn route_plan_generates_headers_and_plan() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest_path = temp.path().join("manifest.json");
    fs::write(&manifest_path, r#"{ "root_cid": [0, 0, 0, 0, 0] }"#).expect("manifest");
    let plan_path = temp.path().join("route_plan.json");
    let headers_path = temp.path().join("route_headers.txt");
    let options = GatewayRoutePlanOptions {
        manifest_json: manifest_path.clone(),
        output_path: plan_path.clone(),
        headers_out: Some(headers_path.clone()),
        alias: Some("sora:docs".into()),
        hostname: Some("docs.sora.link".into()),
        route_label: Some("docs@canary".into()),
        proof_status: Some("ok".into()),
        release_tag: Some("v1.2.3".into()),
        cutover_window: Some("2026-03-21T15:00Z/2026-03-21T15:30Z".into()),
        rollback_manifest: None,
        rollback_headers_out: None,
        rollback_route_label: None,
        rollback_release_tag: None,
        include_csp: true,
        include_permissions: true,
        include_hsts: true,
        now: OffsetDateTime::UNIX_EPOCH,
    };
    run_gateway_route_plan(options).expect("route plan generation succeeds");
    let rendered = fs::read_to_string(&plan_path).expect("plan contents");
    let plan_json: serde_json::Value = serde_json::from_str(&rendered).expect("json plan payload");
    assert_eq!(
        plan_json["content_cid"],
        serde_json::Value::String("baaaaaaaa".into())
    );
    assert_eq!(
        plan_json["route_binding"],
        serde_json::Value::String(
            "host=docs.sora.link;cid=baaaaaaaa;generated_at=1970-01-01T00:00:00Z;label=docs@canary"
                .into()
        )
    );
    assert_eq!(
        plan_json["headers"]["Sora-Name"],
        serde_json::Value::String("sora:docs".into())
    );
    assert_eq!(
        plan_json["headers"]["Sora-Content-CID"],
        serde_json::Value::String("baaaaaaaa".into())
    );
    assert_eq!(
        plan_json["headers_path"],
        serde_json::Value::String(headers_path.display().to_string())
    );
    assert!(plan_json["rollback"].is_null());
    let template = fs::read_to_string(headers_path).expect("headers template");
    assert!(
            template.contains("Sora-Route-Binding: host=docs.sora.link;cid=baaaaaaaa;generated_at=1970-01-01T00:00:00Z;label=docs@canary"),
            "template missing route binding:\n{template}"
        );
    assert!(template.contains("Content-Security-Policy: default-src 'self'"));
}
#[test]
fn route_plan_embeds_rollback_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manifest_path = temp.path().join("manifest.json");
    let rollback_manifest = temp.path().join("rollback_manifest.json");
    fs::write(&manifest_path, r#"{ "root_cids_hex": ["00"] }"#).expect("manifest");
    fs::write(&rollback_manifest, r#"{ "root_cid_hex": "ff" }"#).expect("rollback manifest");
    let plan_path = temp.path().join("route_plan.json");
    let rollback_headers = temp.path().join("rollback_headers.txt");
    let options = GatewayRoutePlanOptions {
        manifest_json: manifest_path,
        output_path: plan_path.clone(),
        headers_out: None,
        alias: Some("sora:docs".into()),
        hostname: Some("docs.sora.link".into()),
        route_label: None,
        proof_status: None,
        release_tag: Some("v1".into()),
        cutover_window: None,
        rollback_manifest: Some(rollback_manifest.clone()),
        rollback_headers_out: Some(rollback_headers.clone()),
        rollback_route_label: Some("previous".into()),
        rollback_release_tag: Some("v0".into()),
        include_csp: true,
        include_permissions: true,
        include_hsts: true,
        now: OffsetDateTime::UNIX_EPOCH,
    };
    run_gateway_route_plan(options).expect("route plan generation succeeds");
    let rendered = fs::read_to_string(&plan_path).expect("plan contents");
    let plan_json: serde_json::Value = serde_json::from_str(&rendered).expect("json plan payload");
    let rollback = plan_json["rollback"].as_object().expect("rollback section");
    assert_eq!(
        rollback["manifest_json"],
        serde_json::Value::String(rollback_manifest.display().to_string())
    );
    assert_eq!(
        rollback["release_tag"],
        serde_json::Value::String("v0".into())
    );
    assert_eq!(
        rollback["route_binding"],
        serde_json::Value::String(
            "host=docs.sora.link;cid=b74;generated_at=1970-01-01T00:00:00Z;label=previous".into()
        )
    );
    assert_eq!(
        rollback["headers_path"],
        serde_json::Value::String(rollback_headers.display().to_string())
    );
    let template =
        fs::read_to_string(rollback_headers).expect("rollback headers template contents");
    assert!(template.contains("label=previous"));
}
