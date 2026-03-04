//! Audit commands for governance workflows.

use crate::{Run, RunContext};
use eyre::Result;
use iroha::client::Client;
use iroha_crypto::Hash;
use norito::json::{Map, Value};

use super::shared::{canonicalize_hex32, compute_proposal_id, decode_hex32, print_with_summary};

#[derive(clap::Args, Debug)]
pub struct AuditDeployArgs {
    /// Namespace to audit (e.g., apps)
    #[arg(long, value_name = "NS")]
    pub namespace: String,
    /// Filter: `contract_id` substring (case-sensitive)
    #[arg(long)]
    pub contains: Option<String>,
    /// Filter: code hash hex prefix (lowercase)
    #[arg(long)]
    pub hash_prefix: Option<String>,
    /// Pagination offset
    #[arg(long)]
    pub offset: Option<u32>,
    /// Pagination limit
    #[arg(long)]
    pub limit: Option<u32>,
    /// Order: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
    #[arg(long)]
    pub order: Option<String>,
}

struct AuditNamespacePage {
    total_in_state: u64,
    offset: u64,
    limit: u64,
    instances: Vec<Value>,
}

struct AuditNamespaceReport {
    records: Vec<Value>,
    contracts_with_issues: usize,
    issue_count: usize,
}

struct InstanceAuditReport {
    record: Value,
    has_issues: bool,
    issue_count: usize,
}

impl Run for AuditDeployArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let page = self.fetch_page(&client)?;
        let report = self.audit_namespace_instances(&client, &page);
        self.print_report(context, &self.namespace, &page, report)
    }
}

impl AuditDeployArgs {
    fn fetch_page(&self, client: &Client) -> Result<AuditNamespacePage> {
        let page = client.get_gov_instances_by_ns_filtered_json(
            &self.namespace,
            self.contains.as_deref(),
            self.hash_prefix.as_deref(),
            self.offset,
            self.limit,
            self.order.as_deref(),
        )?;

        let total_in_state = page.get("total").and_then(Value::as_u64).unwrap_or(0);
        let offset = page.get("offset").and_then(Value::as_u64).unwrap_or(0);
        let limit = page.get("limit").and_then(Value::as_u64).unwrap_or(0);
        let instances = page
            .get("instances")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        Ok(AuditNamespacePage {
            total_in_state,
            offset,
            limit,
            instances,
        })
    }

    fn audit_namespace_instances(
        &self,
        client: &Client,
        page: &AuditNamespacePage,
    ) -> AuditNamespaceReport {
        let mut records = Vec::with_capacity(page.instances.len());
        let mut contracts_with_issues = 0usize;
        let mut issue_count = 0usize;

        for entry in &page.instances {
            if let Some(report) = Self::audit_entry(client, &self.namespace, entry) {
                if report.has_issues {
                    contracts_with_issues += 1;
                }
                issue_count += report.issue_count;
                records.push(report.record);
            }
        }

        AuditNamespaceReport {
            records,
            contracts_with_issues,
            issue_count,
        }
    }

    fn audit_entry(client: &Client, namespace: &str, entry: &Value) -> Option<InstanceAuditReport> {
        let obj = entry.as_object()?;
        let contract_id = obj.get("contract_id").and_then(Value::as_str)?;
        let code_hash_raw = obj.get("code_hash_hex").and_then(Value::as_str)?;

        let mut record = Map::new();
        record.insert("contract_id".into(), Value::from(contract_id));
        record.insert("code_hash_input".into(), Value::from(code_hash_raw));

        let mut manifest_map = Map::new();
        let mut proposal_map = Map::new();
        let mut code_map = Map::new();
        let mut issues: Vec<String> = Vec::new();

        let code_hash = match canonicalize_hex32(code_hash_raw) {
            Ok(hash) => hash,
            Err(err) => {
                manifest_map.insert("present".into(), Value::from(false));
                manifest_map.insert("error".into(), Value::from(err.to_string()));
                proposal_map.insert("expected_id".into(), Value::Null);
                proposal_map.insert("found".into(), Value::from(false));
                code_map.insert("present".into(), Value::from(false));
                code_map.insert("error".into(), Value::from("skipped: invalid code hash"));
                issues.push(format!(
                    "invalid_code_hash_hex: contract={contract_id} value={code_hash_raw}"
                ));
                return Some(InstanceAuditReport::from_maps(
                    record,
                    manifest_map,
                    proposal_map,
                    code_map,
                    issues,
                ));
            }
        };
        record.insert("code_hash".into(), Value::from(code_hash.clone()));

        let manifest_abi_hash =
            audit_manifest_map(client, &code_hash, &mut manifest_map, &mut issues);
        audit_code_map(client, &code_hash, &mut code_map, &mut issues);
        audit_proposal_map(
            client,
            namespace,
            contract_id,
            &code_hash,
            manifest_abi_hash.as_deref(),
            &mut proposal_map,
            &mut issues,
        );

        Some(InstanceAuditReport::from_maps(
            record,
            manifest_map,
            proposal_map,
            code_map,
            issues,
        ))
    }

    fn print_report<C: RunContext>(
        &self,
        context: &mut C,
        namespace: &str,
        page: &AuditNamespacePage,
        report: AuditNamespaceReport,
    ) -> Result<()> {
        let AuditNamespaceReport {
            records,
            contracts_with_issues,
            issue_count,
        } = report;
        let scanned = records.len() as u64;
        let ok = scanned.saturating_sub(contracts_with_issues as u64);
        let summary = format!(
            "gov audit namespace={namespace} scanned={scanned} ok={ok} with_issues={contracts_with_issues} issue_count={issue_count}"
        );

        let mut output = Map::new();
        output.insert("namespace".into(), Value::from(namespace));
        output.insert("state_total".into(), Value::from(page.total_in_state));
        output.insert("page_offset".into(), Value::from(page.offset));
        output.insert("page_limit".into(), Value::from(page.limit));
        output.insert("scanned".into(), Value::from(scanned));
        output.insert(
            "with_issues".into(),
            Value::from(contracts_with_issues as u64),
        );
        output.insert("issue_count".into(), Value::from(issue_count as u64));
        output.insert("results".into(), Value::Array(records));

        let output_value = Value::Object(output);
        print_with_summary(context, Some(summary), &output_value)
    }
}

impl InstanceAuditReport {
    fn from_maps(
        mut record: Map,
        manifest_map: Map,
        proposal_map: Map,
        code_map: Map,
        issues: Vec<String>,
    ) -> Self {
        let issue_count = issues.len();
        let has_issues = issue_count > 0;
        let issues_value = Value::Array(issues.into_iter().map(Value::from).collect());
        record.insert("manifest".into(), Value::Object(manifest_map));
        record.insert("proposal".into(), Value::Object(proposal_map));
        record.insert("code".into(), Value::Object(code_map));
        record.insert("issues".into(), issues_value);
        record.insert("has_issues".into(), Value::from(has_issues));
        InstanceAuditReport {
            record: Value::Object(record),
            has_issues,
            issue_count,
        }
    }
}

fn audit_manifest_map(
    client: &Client,
    code_hash: &str,
    manifest_map: &mut Map,
    issues: &mut Vec<String>,
) -> Option<String> {
    let manifest_value = client.get_contract_manifest_json(code_hash);
    manifest_map.insert("present".into(), Value::from(manifest_value.is_ok()));
    let mut manifest_abi_hash: Option<String> = None;

    match manifest_value {
        Ok(manifest_v) => {
            if let Some(manifest_obj) = manifest_v.get("manifest").and_then(Value::as_object) {
                if let Some(code_hash_str) = manifest_obj.get("code_hash").and_then(Value::as_str) {
                    match canonicalize_hex32(code_hash_str) {
                        Ok(manifest_hash) => {
                            let matches = manifest_hash == code_hash;
                            manifest_map.insert("code_hash".into(), Value::from(manifest_hash));
                            manifest_map.insert("code_hash_matches".into(), Value::from(matches));
                            if !matches {
                                issues.push(format!(
                                "manifest_code_hash_mismatch: expected={code_hash} got={code_hash_str}"
                            ));
                            }
                        }
                        Err(err) => {
                            manifest_map
                                .insert("code_hash_error".into(), Value::from(err.to_string()));
                            issues.push(format!("manifest_code_hash_invalid: {err}"));
                        }
                    }
                } else {
                    manifest_map.insert("code_hash".into(), Value::Null);
                }

                if let Some(abi_hash_str) = manifest_obj.get("abi_hash").and_then(Value::as_str) {
                    match canonicalize_hex32(abi_hash_str) {
                        Ok(abi_hash) => {
                            manifest_map.insert("abi_hash".into(), Value::from(abi_hash.clone()));
                            manifest_abi_hash = Some(abi_hash);
                        }
                        Err(err) => {
                            manifest_map
                                .insert("abi_hash_error".into(), Value::from(err.to_string()));
                            issues.push(format!("manifest_abi_hash_invalid: {err}"));
                        }
                    }
                } else {
                    manifest_map.insert("abi_hash".into(), Value::Null);
                    issues.push("manifest_missing_abi_hash".into());
                }
            } else {
                manifest_map.insert(
                    "error".into(),
                    Value::from("response missing manifest object"),
                );
                issues.push("manifest_structure_unexpected".into());
            }
        }
        Err(err) => {
            manifest_map.insert("error".into(), Value::from(err.to_string()));
            issues.push(format!("manifest_fetch_error: {err}"));
        }
    }

    manifest_abi_hash
}

fn audit_code_map(client: &Client, code_hash: &str, code_map: &mut Map, issues: &mut Vec<String>) {
    match client.get_contract_code_bytes(code_hash) {
        Ok(bytes) => {
            let length = bytes.len() as u64;
            code_map.insert("present".into(), Value::from(true));
            code_map.insert("length".into(), Value::from(length));
            if bytes.is_empty() {
                code_map.insert("computed_hash".into(), Value::Null);
                code_map.insert("hash_matches".into(), Value::from(false));
                issues.push("code_bytes_empty".into());
            } else {
                let computed = Hash::new(&bytes);
                let computed_hex = hex::encode(computed.as_ref());
                let matches = computed_hex == code_hash;
                code_map.insert("computed_hash".into(), Value::from(computed_hex.clone()));
                code_map.insert("hash_matches".into(), Value::from(matches));
                if !matches {
                    issues.push(format!(
                        "code_hash_mismatch: expected={code_hash} computed={computed_hex}"
                    ));
                }
            }
        }
        Err(err) => {
            code_map.insert("present".into(), Value::from(false));
            code_map.insert("error".into(), Value::from(err.to_string()));
            issues.push(format!("code_bytes_error: {err}"));
        }
    }
}

fn audit_proposal_map(
    client: &Client,
    namespace: &str,
    contract_id: &str,
    code_hash: &str,
    manifest_abi_hash: Option<&str>,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    let Some(abi_hash_hex) = manifest_abi_hash else {
        proposal_map.insert("expected_id".into(), Value::Null);
        proposal_map.insert("found".into(), Value::from(false));
        return;
    };

    if let Some(expected_id) = resolve_proposal_id(
        namespace,
        contract_id,
        code_hash,
        abi_hash_hex,
        proposal_map,
        issues,
    ) && let Some(proposal_json) =
        fetch_proposal_json(client, &expected_id, proposal_map, issues)
    {
        process_proposal_json(
            &proposal_json,
            namespace,
            contract_id,
            code_hash,
            abi_hash_hex,
            proposal_map,
            issues,
        );
    }
}

fn resolve_proposal_id(
    namespace: &str,
    contract_id: &str,
    code_hash: &str,
    abi_hash_hex: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) -> Option<String> {
    match (decode_hex32(code_hash), decode_hex32(abi_hash_hex)) {
        (Ok(code_bytes), Ok(abi_bytes)) => {
            let proposal_id_bytes =
                compute_proposal_id(namespace, contract_id, &code_bytes, &abi_bytes);
            let proposal_id_hex = hex::encode(proposal_id_bytes);
            proposal_map.insert("expected_id".into(), Value::from(proposal_id_hex.clone()));
            Some(proposal_id_hex)
        }
        (Err(err), _) => {
            proposal_map.insert("expected_id".into(), Value::Null);
            proposal_map.insert("found".into(), Value::from(false));
            issues.push(format!("code_hash_decode_error: {err}"));
            None
        }
        (_, Err(err)) => {
            proposal_map.insert("expected_id".into(), Value::Null);
            proposal_map.insert("found".into(), Value::from(false));
            issues.push(format!("abi_hash_decode_error: {err}"));
            None
        }
    }
}

fn fetch_proposal_json(
    client: &Client,
    proposal_id_hex: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) -> Option<Value> {
    match client.get_gov_proposal_json(proposal_id_hex) {
        Ok(value) => Some(value),
        Err(err) => {
            proposal_map.insert("found".into(), Value::from(false));
            proposal_map.insert("error".into(), Value::from(err.to_string()));
            issues.push(format!("proposal_fetch_error: {err}"));
            None
        }
    }
}

fn process_proposal_json(
    proposal_json: &Value,
    namespace: &str,
    contract_id: &str,
    code_hash: &str,
    abi_hash_hex: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    let found = proposal_json
        .get("found")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    proposal_map.insert("found".into(), Value::from(found));
    if !found {
        issues.push("proposal_missing".into());
        return;
    }

    let Some(proposal_obj) = proposal_json.get("proposal").and_then(Value::as_object) else {
        return;
    };

    update_status(proposal_obj, proposal_map, issues);
    if let Some(kind_obj) = proposal_obj.get("kind").and_then(Value::as_object) {
        if let Some(deploy_obj) = kind_obj.get("DeployContract").and_then(Value::as_object) {
            audit_deploy_contract(
                deploy_obj,
                namespace,
                contract_id,
                code_hash,
                abi_hash_hex,
                proposal_map,
                issues,
            );
        } else {
            issues.push("proposal_kind_not_deploy_contract".into());
        }
    }
}

fn update_status(proposal_obj: &Map, proposal_map: &mut Map, issues: &mut Vec<String>) {
    if let Some(status) = proposal_obj.get("status").and_then(Value::as_str) {
        proposal_map.insert("status".into(), Value::from(status));
        if status != "Enacted" {
            issues.push(format!("proposal_status_not_enacted: {status}"));
        }
    } else {
        issues.push("proposal_status_missing".into());
    }
}

fn audit_deploy_contract(
    deploy_obj: &Map,
    namespace: &str,
    contract_id: &str,
    code_hash: &str,
    abi_hash_hex: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    check_namespace(deploy_obj, namespace, proposal_map, issues);
    check_contract_id(deploy_obj, contract_id, proposal_map, issues);
    check_hex_field(
        deploy_obj,
        &HexFieldContext {
            source_key: "code_hash_hex",
            expected: code_hash,
            map_value_key: "code_hash_hex",
            matches_key: "code_hash_matches",
            error_key: "code_hash_error",
            mismatch_issue: "proposal_code_hash_mismatch",
            invalid_issue: "proposal_code_hash_invalid",
        },
        proposal_map,
        issues,
    );
    check_hex_field(
        deploy_obj,
        &HexFieldContext {
            source_key: "abi_hash_hex",
            expected: abi_hash_hex,
            map_value_key: "abi_hash_hex",
            matches_key: "abi_hash_matches",
            error_key: "abi_hash_error",
            mismatch_issue: "proposal_abi_hash_mismatch",
            invalid_issue: "proposal_abi_hash_invalid",
        },
        proposal_map,
        issues,
    );
}

fn check_namespace(
    deploy_obj: &Map,
    expected: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    let namespace_value = deploy_obj.get("namespace").cloned().unwrap_or(Value::Null);
    let namespace_match = namespace_value.as_str() == Some(expected);
    proposal_map.insert("namespace".into(), namespace_value);
    if !namespace_match {
        issues.push("proposal_namespace_mismatch".into());
    }
}

fn check_contract_id(
    deploy_obj: &Map,
    expected: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    let contract_value = deploy_obj
        .get("contract_id")
        .cloned()
        .unwrap_or(Value::Null);
    let contract_match = contract_value.as_str() == Some(expected);
    proposal_map.insert("contract_id".into(), contract_value);
    if !contract_match {
        issues.push("proposal_contract_mismatch".into());
    }
}

#[derive(Clone, Copy)]
struct HexFieldContext<'a> {
    source_key: &'a str,
    expected: &'a str,
    map_value_key: &'a str,
    matches_key: &'a str,
    error_key: &'a str,
    mismatch_issue: &'static str,
    invalid_issue: &'static str,
}

fn check_hex_field(
    deploy_obj: &Map,
    ctx: &HexFieldContext<'_>,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    if let Some(prop_hash) = deploy_obj.get(ctx.source_key).and_then(Value::as_str) {
        match canonicalize_hex32(prop_hash) {
            Ok(canonical) => {
                let matches = canonical == ctx.expected;
                proposal_map.insert(ctx.map_value_key.into(), Value::from(canonical.clone()));
                proposal_map.insert(ctx.matches_key.into(), Value::from(matches));
                if !matches {
                    issues.push(ctx.mismatch_issue.into());
                }
            }
            Err(err) => {
                proposal_map.insert(ctx.error_key.into(), Value::from(err.to_string()));
                issues.push(format!("{}: {err}", ctx.invalid_issue));
            }
        }
    } else {
        proposal_map.insert(ctx.map_value_key.into(), Value::Null);
        issues.push(ctx.invalid_issue.into());
    }
}
