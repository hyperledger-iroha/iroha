//! Audit commands for governance workflows.

use crate::{Run, RunContext};
use eyre::Result;
use iroha::client::Client;
use iroha_crypto::Hash;
use norito::json::{Map, Value};

use super::shared::{
    canonicalize_hex32, compute_proposal_id, decode_hex32, print_with_summary,
    resolve_contract_address_target,
};

#[derive(clap::Args, Debug)]
pub struct AuditDeployArgs {
    #[arg(long, conflicts_with = "contract_alias")]
    pub contract_address: Option<String>,
    #[arg(long, conflicts_with = "contract_address")]
    pub contract_alias: Option<String>,
}

impl Run for AuditDeployArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let contract_address = resolve_contract_address_target(
            &client,
            self.contract_address.as_deref(),
            self.contract_alias.as_deref(),
        )?;
        let report = self.audit_contract(&client, &contract_address)?;
        let found = report
            .get("found")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let has_issues = report
            .get("has_issues")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let issue_count = report
            .get("issue_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let summary = Some(format!(
            "gov audit contract_address={} found={found} has_issues={has_issues} issue_count={issue_count}",
            contract_address
        ));
        print_with_summary(context, summary, &report)
    }
}

impl AuditDeployArgs {
    fn audit_contract(
        &self,
        client: &Client,
        contract_address: &iroha::data_model::smart_contract::ContractAddress,
    ) -> Result<Value> {
        let binding = client.get_gov_contract_json(contract_address)?;
        let found = binding.get("found").and_then(Value::as_bool).unwrap_or(false);
        let dataspace = binding.get("dataspace").and_then(Value::as_str);
        let code_hash_raw = binding.get("code_hash_hex").and_then(Value::as_str);

        let mut record = Map::new();
        record.insert(
            "contract_address".into(),
            Value::from(contract_address.to_string()),
        );
        record.insert("found".into(), Value::from(found));
        record.insert(
            "dataspace".into(),
            dataspace.map_or(Value::Null, Value::from),
        );

        let mut manifest_map = Map::new();
        let mut proposal_map = Map::new();
        let mut code_map = Map::new();
        let mut issues = Vec::new();

        let Some(code_hash_raw) = code_hash_raw else {
            manifest_map.insert("present".into(), Value::from(false));
            proposal_map.insert("expected_id".into(), Value::Null);
            proposal_map.insert("found".into(), Value::from(false));
            code_map.insert("present".into(), Value::from(false));
            if !found {
                issues.push("contract_binding_missing".into());
            } else {
                issues.push("contract_binding_missing_code_hash".into());
            }
            return Ok(finalize_record(
                record,
                manifest_map,
                proposal_map,
                code_map,
                issues,
            ));
        };

        record.insert("code_hash_input".into(), Value::from(code_hash_raw));

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
                    "invalid_code_hash_hex: contract_address={contract_address} value={code_hash_raw}"
                ));
                return Ok(finalize_record(
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
            contract_address,
            &code_hash,
            manifest_abi_hash.as_deref(),
            &mut proposal_map,
            &mut issues,
        );

        Ok(finalize_record(
            record,
            manifest_map,
            proposal_map,
            code_map,
            issues,
        ))
    }
}

fn finalize_record(
    mut record: Map,
    manifest_map: Map,
    proposal_map: Map,
    code_map: Map,
    issues: Vec<String>,
) -> Value {
    let issue_count = issues.len();
    let has_issues = issue_count > 0;
    record.insert("manifest".into(), Value::Object(manifest_map));
    record.insert("proposal".into(), Value::Object(proposal_map));
    record.insert("code".into(), Value::Object(code_map));
    record.insert(
        "issues".into(),
        Value::Array(issues.into_iter().map(Value::from).collect()),
    );
    record.insert("has_issues".into(), Value::from(has_issues));
    record.insert("issue_count".into(), Value::from(issue_count as u64));
    Value::Object(record)
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
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
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

    if let Some(expected_id) =
        resolve_proposal_id(contract_address, code_hash, abi_hash_hex, proposal_map, issues)
        && let Some(proposal_json) = fetch_proposal_json(client, &expected_id, proposal_map, issues)
    {
        process_proposal_json(
            &proposal_json,
            contract_address,
            code_hash,
            abi_hash_hex,
            proposal_map,
            issues,
        );
    }
}

fn resolve_proposal_id(
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
    code_hash: &str,
    abi_hash_hex: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) -> Option<String> {
    match (decode_hex32(code_hash), decode_hex32(abi_hash_hex)) {
        (Ok(code_bytes), Ok(abi_bytes)) => {
            let proposal_id_bytes = compute_proposal_id(contract_address, &code_bytes, &abi_bytes);
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
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
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
                contract_address,
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
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
    code_hash: &str,
    abi_hash_hex: &str,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    check_contract_address(deploy_obj, contract_address, proposal_map, issues);
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

fn check_contract_address(
    deploy_obj: &Map,
    expected: &iroha::data_model::smart_contract::ContractAddress,
    proposal_map: &mut Map,
    issues: &mut Vec<String>,
) {
    let contract_value = deploy_obj
        .get("contract_address")
        .cloned()
        .unwrap_or(Value::Null);
    let contract_match = contract_value.as_str() == Some(expected.as_ref());
    proposal_map.insert("contract_address".into(), contract_value);
    if !contract_match {
        issues.push("proposal_contract_address_mismatch".into());
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
