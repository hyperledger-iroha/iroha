//! Canonical pure receipt checks shared by native Inrou producers and Unix deployment.

use std::collections::BTreeSet;

use eyre::{Result, eyre};

/// Require the complete field set without accepting extra or missing receipt fields.
pub(crate) fn require_exact_json_fields(
    object: &norito::json::Map,
    expected: &[&str],
    label: &str,
) -> Result<()> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
        return Err(eyre!(
            "{label} is outside its exact V1 field set (missing: {missing:?}; unexpected: {unexpected:?})"
        ));
    }
    Ok(())
}

fn require_empty_report_array(object: &norito::json::Map, field: &str, label: &str) -> Result<()> {
    if !object
        .get(field)
        .and_then(norito::json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        return Err(eyre!(
            "{label} field `{field}` must be an explicit empty array"
        ));
    }
    Ok(())
}

/// Validate explicit empty warnings/failures and the exact checks for the admitted scope.
pub(crate) fn validate_prepared_report_common_arrays(
    object: &norito::json::Map,
    service_applied: bool,
    label: &str,
) -> Result<()> {
    require_empty_report_array(object, "warnings", label)?;
    require_empty_report_array(object, "failures", label)?;
    if !service_applied {
        return require_empty_report_array(object, "checks", label);
    }
    let checks = object
        .get("checks")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("{label} field `checks` must be an array"))?;
    let public_checks = [
        (
            "inrou_authoritative_status",
            "active_adverts=4, hosted_replicas=4",
        ),
        (
            "inrou_public_routes",
            "observed distinct durable identities and guest boots for replica slots 1, 2, 3, and 4",
        ),
        (
            "inrou_public_discovery",
            "current and revision authority plus public path and CID-host bytes, headers, and hash are exact",
        ),
    ];
    let expected = match object
        .get("probe_scope")
        .and_then(norito::json::Value::as_str)
    {
        Some("candidate") => &public_checks[..2],
        Some("public") => public_checks.as_slice(),
        _ => return Err(eyre!("Inrou report omits its exact probe scope")),
    };
    if checks.len() != expected.len() {
        return Err(eyre!(
            "{label} must contain exactly the successful checks for its probe scope"
        ));
    }
    for (check, &(name, detail)) in checks.iter().zip(expected) {
        let check = check
            .as_object()
            .ok_or_else(|| eyre!("{label} check must be an object"))?;
        require_exact_json_fields(check, &["name", "http_status", "ok", "detail"], label)?;
        if check.get("name").and_then(norito::json::Value::as_str) != Some(name)
            || check
                .get("http_status")
                .and_then(norito::json::Value::as_u64)
                != Some(200)
            || check.get("ok").and_then(norito::json::Value::as_bool) != Some(true)
            || check.get("detail").and_then(norito::json::Value::as_str) != Some(detail)
        {
            return Err(eyre!(
                "{label} does not carry the exact successful Inrou check evidence"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
/// Apply the production receipt rules to a real producer's required scope.
pub(crate) fn validate_inrou_checks(
    value: &norito::json::Value,
    required_scope: &str,
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("Inrou report is not an object"))?;
    if object
        .get("probe_scope")
        .and_then(norito::json::Value::as_str)
        != Some(required_scope)
    {
        return Err(eyre!(
            "Inrou report scope does not match the required proof"
        ));
    }
    validate_prepared_report_common_arrays(object, true, "real Inrou producer report")
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::Value;

    fn report(scope: &str) -> Value {
        let mut checks = vec![
            norito::json!({
                "name": "inrou_authoritative_status",
                "http_status": 200,
                "ok": true,
                "detail": "active_adverts=4, hosted_replicas=4"
            }),
            norito::json!({
                "name": "inrou_public_routes",
                "http_status": 200,
                "ok": true,
                "detail": "observed distinct durable identities and guest boots for replica slots 1, 2, 3, and 4"
            }),
            norito::json!({
                "name": "inrou_public_discovery",
                "http_status": 200,
                "ok": true,
                "detail": "current and revision authority plus public path and CID-host bytes, headers, and hash are exact"
            }),
        ];
        if scope == "candidate" {
            checks.pop();
        }
        norito::json!({"probe_scope": scope, "warnings": [], "failures": [], "checks": checks})
    }

    #[test]
    fn exact_fields_reject_missing_and_unexpected_names() {
        let valid = norito::json!({"one": 1, "two": 2});
        let object = valid.as_object().unwrap();
        require_exact_json_fields(object, &["one", "two"], "fixture").unwrap();
        for expected in [&["one"][..], &["one", "two", "three"][..]] {
            assert!(require_exact_json_fields(object, expected, "fixture").is_err());
        }
    }

    #[test]
    fn unapplied_reports_require_all_three_explicit_empty_arrays() {
        let valid = norito::json!({"warnings": [], "failures": [], "checks": []});
        validate_prepared_report_common_arrays(valid.as_object().unwrap(), false, "fixture")
            .unwrap();
        for field in ["warnings", "failures", "checks"] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                validate_prepared_report_common_arrays(
                    missing.as_object().unwrap(),
                    false,
                    "fixture"
                )
                .is_err()
            );
            for replacement in [
                Value::Null,
                norito::json!({}),
                norito::json!(["unexpected"]),
            ] {
                let mut changed = valid.clone();
                changed
                    .as_object_mut()
                    .unwrap()
                    .insert(field.to_owned(), replacement);
                assert!(
                    validate_prepared_report_common_arrays(
                        changed.as_object().unwrap(),
                        false,
                        "fixture"
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn applied_reports_bind_every_ordered_check_field_and_reject_warning_or_failure() {
        for scope in ["candidate", "public"] {
            let valid = report(scope);
            validate_inrou_checks(&valid, scope).unwrap();
            for field in ["warnings", "failures"] {
                let mut changed = valid.clone();
                changed
                    .as_object_mut()
                    .unwrap()
                    .insert(field.to_owned(), norito::json!(["blocked"]));
                assert!(validate_inrou_checks(&changed, scope).is_err());
            }
            let checks = valid.get("checks").unwrap().as_array().unwrap();
            for index in 0..checks.len() {
                for (field, replacement) in [
                    ("name", norito::json!("other")),
                    ("http_status", norito::json!(503)),
                    ("http_status", norito::json!("200")),
                    ("ok", norito::json!(false)),
                    ("ok", norito::json!("true")),
                    ("detail", norito::json!("plausible but different")),
                    ("extra", norito::json!(true)),
                ] {
                    let mut changed = valid.clone();
                    changed
                        .as_object_mut()
                        .unwrap()
                        .get_mut("checks")
                        .unwrap()
                        .as_array_mut()
                        .unwrap()[index]
                        .as_object_mut()
                        .unwrap()
                        .insert(field.to_owned(), replacement);
                    assert!(
                        validate_inrou_checks(&changed, scope).is_err(),
                        "{scope} check {index} field {field}"
                    );
                }
                for field in ["name", "http_status", "ok", "detail"] {
                    let mut changed = valid.clone();
                    changed
                        .as_object_mut()
                        .unwrap()
                        .get_mut("checks")
                        .unwrap()
                        .as_array_mut()
                        .unwrap()[index]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                    assert!(validate_inrou_checks(&changed, scope).is_err());
                }
            }
            for mutation in 0..4 {
                let mut changed = valid.clone();
                let checks = changed
                    .as_object_mut()
                    .unwrap()
                    .get_mut("checks")
                    .unwrap()
                    .as_array_mut()
                    .unwrap();
                match mutation {
                    0 => {
                        checks.pop();
                    }
                    1 => checks.push(checks[0].clone()),
                    2 => checks.swap(0, 1),
                    _ => checks[0] = Value::Null,
                }
                assert!(validate_inrou_checks(&changed, scope).is_err());
            }
        }
    }

    #[test]
    fn candidate_checks_never_satisfy_public_scope_or_an_unknown_scope() {
        for scope in ["candidate", "public"] {
            let valid = report(scope);
            validate_inrou_checks(&valid, scope).unwrap();
            let other = if scope == "candidate" {
                "public"
            } else {
                "candidate"
            };
            assert!(validate_inrou_checks(&valid, other).is_err());
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove("probe_scope");
            assert!(validate_inrou_checks(&missing, scope).is_err());
        }
        assert!(validate_inrou_checks(&report("unrecognized"), "unrecognized").is_err());
        assert!(validate_inrou_checks(&Value::Null, "candidate").is_err());
    }
}
