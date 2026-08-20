//! Exact controller and source binding for native candidate qualification.

use super::{
    protocol::{RunAssignmentV1, TairaAuthorityRoleV1},
    service::{TairaAuthorityErrorV1, parse_digest},
};
use norito::json::{Map, Value};

const CONTROLLER_FIELDS_V1: [&str; 3] = ["closure_digest", "host_id", "installation_id"];
const SOURCE_FIELDS_V1: [&str; 4] = [
    "cargo_lock_sha256",
    "commit",
    "dpn_validator_release_commit",
    "workspace_source_manifest_sha256",
];

/// Require the canonical qualification subject to agree with the
/// administrator-pinned controller identity and carry the complete source tuple.
pub(super) fn validate_qualification_subject_v1(
    subject: &Value,
    assignment: &RunAssignmentV1,
) -> Result<(), TairaAuthorityErrorV1> {
    if assignment.role != TairaAuthorityRoleV1::Qualification {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let subject = subject.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    let controller = exact_object(
        subject
            .get("controller")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &CONTROLLER_FIELDS_V1,
    )?;
    let controller_digest = parse_digest(required_str(controller, "closure_digest")?)?;
    let host_id = required_str(controller, "host_id")?;
    let installation_id = required_str(controller, "installation_id")?;
    if !valid_trust_id(host_id)
        || !valid_trust_id(installation_id)
        || assignment.controller_digest != Some(controller_digest)
        || assignment.controller_host_id.as_deref() != Some(host_id)
        || assignment.controller_installation_id.as_deref() != Some(installation_id)
        || assignment.run_nonce.is_none()
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }

    let source = exact_object(
        subject
            .get("source")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &SOURCE_FIELDS_V1,
    )?;
    if !valid_commit(required_str(source, "commit")?)
        || !valid_commit(required_str(source, "dpn_validator_release_commit")?)
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    parse_digest(required_str(source, "cargo_lock_sha256")?)?;
    parse_digest(required_str(source, "workspace_source_manifest_sha256")?)?;
    Ok(())
}

fn exact_object<'a>(value: &'a Value, fields: &[&str]) -> Result<&'a Map, TairaAuthorityErrorV1> {
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(object)
}

fn required_str<'a>(object: &'a Map, field: &str) -> Result<&'a str, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40
        && value != "0000000000000000000000000000000000000000"
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_trust_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTROLLER_DIGEST_V1: [u8; 32] = [0x11; 32];

    fn valid_assignment() -> RunAssignmentV1 {
        RunAssignmentV1 {
            role: TairaAuthorityRoleV1::Qualification,
            run_id: [0x21; 32],
            subject_sha256: [0x22; 32],
            artifact_manifest_sha256: [0x23; 32],
            controller_digest: Some(CONTROLLER_DIGEST_V1),
            controller_host_id: Some("host-01.example".to_owned()),
            controller_installation_id: Some("install_01".to_owned()),
            run_nonce: Some([0x24; 32]),
            issued_at_unix_millis: 1,
            not_before_unix_millis: 1,
            expires_at_unix_millis: 2,
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0x25; 32],
        }
    }

    fn valid_subject() -> Value {
        let mut controller = Map::new();
        controller.insert(
            "closure_digest".into(),
            Value::from("11".repeat(CONTROLLER_DIGEST_V1.len())),
        );
        controller.insert("host_id".into(), Value::from("host-01.example"));
        controller.insert("installation_id".into(), Value::from("install_01"));

        let mut source = Map::new();
        source.insert("cargo_lock_sha256".into(), Value::from("a1".repeat(32)));
        source.insert("commit".into(), Value::from("b2".repeat(20)));
        source.insert(
            "dpn_validator_release_commit".into(),
            Value::from("c3".repeat(20)),
        );
        source.insert(
            "workspace_source_manifest_sha256".into(),
            Value::from("d4".repeat(32)),
        );

        let mut subject = Map::new();
        subject.insert("controller".into(), Value::Object(controller));
        subject.insert("source".into(), Value::Object(source));
        Value::Object(subject)
    }

    fn nested_object_mut<'a>(subject: &'a mut Value, field: &str) -> &'a mut Map {
        subject
            .as_object_mut()
            .and_then(|object| object.get_mut(field))
            .and_then(Value::as_object_mut)
            .expect("fixture nested object")
    }

    fn replace_nested(subject: &mut Value, object: &str, field: &str, value: impl Into<Value>) {
        nested_object_mut(subject, object).insert(field.to_owned(), value.into());
    }

    fn assert_rejected(subject: &Value, assignment: &RunAssignmentV1, case: &str) {
        assert_eq!(
            validate_qualification_subject_v1(subject, assignment),
            Err(TairaAuthorityErrorV1::Rejected),
            "accepted invalid qualification binding: {case}"
        );
    }

    #[test]
    fn qualification_subject_accepts_complete_canonical_binding() {
        assert_eq!(
            validate_qualification_subject_v1(&valid_subject(), &valid_assignment()),
            Ok(())
        );
    }

    #[test]
    fn qualification_subject_rejects_controller_mismatches() {
        let assignment = valid_assignment();
        for (field, value) in [
            ("closure_digest", "12".repeat(32)),
            ("host_id", "host-02.example".to_owned()),
            ("installation_id", "install_02".to_owned()),
        ] {
            let mut subject = valid_subject();
            replace_nested(&mut subject, "controller", field, value);
            assert_rejected(&subject, &assignment, field);
        }

        let mut missing_nonce = assignment.clone();
        missing_nonce.run_nonce = None;
        assert_rejected(&valid_subject(), &missing_nonce, "missing run nonce");

        let mut wrong_role = assignment;
        wrong_role.role = TairaAuthorityRoleV1::NativeEvidence;
        assert_rejected(&valid_subject(), &wrong_role, "wrong role");
    }

    #[test]
    fn qualification_subject_rejects_noncanonical_or_nonexact_fields() {
        let assignment = valid_assignment();
        for (object, fields) in [
            ("controller", CONTROLLER_FIELDS_V1.as_slice()),
            ("source", SOURCE_FIELDS_V1.as_slice()),
        ] {
            for field in fields {
                let mut subject = valid_subject();
                nested_object_mut(&mut subject, object).remove(*field);
                assert_rejected(&subject, &assignment, &format!("missing {object}.{field}"));
            }

            let mut subject = valid_subject();
            nested_object_mut(&mut subject, object)
                .insert("unexpected".to_owned(), Value::from("field"));
            assert_rejected(&subject, &assignment, &format!("extra {object} field"));
        }

        for (object, field, value) in [
            ("controller", "closure_digest", "AA".repeat(32)),
            ("controller", "host_id", "Host-01.example".to_owned()),
            ("controller", "installation_id", "-install_01".to_owned()),
            ("source", "cargo_lock_sha256", "A1".repeat(32)),
            ("source", "commit", "B2".repeat(20)),
            ("source", "dpn_validator_release_commit", "0".repeat(40)),
            ("source", "workspace_source_manifest_sha256", "0".repeat(64)),
        ] {
            let mut subject = valid_subject();
            replace_nested(&mut subject, object, field, value);
            assert_rejected(
                &subject,
                &assignment,
                &format!("noncanonical {object}.{field}"),
            );
        }
    }

    #[test]
    fn qualification_binding_source_is_exact_and_precedes_native_probes() {
        let source = include_str!("qualification_binding.rs")
            .split_once("\n#[cfg(test)]\nmod tests")
            .unwrap()
            .0;
        for needle in [
            "assignment.role != TairaAuthorityRoleV1::Qualification",
            "assignment.controller_digest != Some(controller_digest)",
            "assignment.controller_host_id.as_deref() != Some(host_id)",
            "assignment.controller_installation_id.as_deref() != Some(installation_id)",
            "assignment.run_nonce.is_none()",
            "SOURCE_FIELDS_V1",
            "valid_commit(required_str(source, \"commit\")?)",
            "parse_digest(required_str(source, \"cargo_lock_sha256\")?)?",
        ] {
            assert!(source.contains(needle), "missing binding step {needle}");
        }
        let service = include_str!("service.rs");
        let authorization = service
            .split_once("let request_matches_assignment =")
            .unwrap()
            .1
            .split_once("let consumption = if let Some(existing)")
            .unwrap()
            .0;
        let binding = authorization
            .find("validate_qualification_subject_v1")
            .unwrap();
        let probes = authorization.find("run_qualification_probes").unwrap();
        assert!(binding < probes);
    }
}
