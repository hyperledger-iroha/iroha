use super::*;

const SIGNED_SUBJECT_FIELDS_V1: [&str; 6] = [
    "authenticated_run_schema",
    "authority_schema",
    "expected",
    "replay_namespace",
    "structural_subject",
    "validation_time_unix",
];

fn manifest_value(artifacts: &[(String, Vec<u8>)]) -> Value {
    Value::Array(
        artifacts
            .iter()
            .enumerate()
            .map(|(ordinal, (name, bytes))| {
                let mut row = Map::new();
                row.insert("name".into(), Value::from(name.clone()));
                row.insert("ordinal".into(), Value::from(ordinal as u64));
                row.insert(
                    "sha256".into(),
                    Value::from(hex::encode(Sha256::digest(bytes))),
                );
                row.insert("size".into(), Value::from(bytes.len() as u64));
                Value::Object(row)
            })
            .collect(),
    )
}

fn stage_artifacts(parent: &Path, artifacts: &[(String, Vec<u8>)]) -> Vec<PathBuf> {
    artifacts
        .iter()
        .enumerate()
        .map(|(ordinal, (_, bytes))| {
            create_artifact(
                parent,
                &format!("privacy-protocol-origin-{ordinal:02}.bin"),
                bytes,
            )
        })
        .collect()
}

fn descriptors(paths: &[PathBuf]) -> Vec<OwnedFd> {
    read_only_descriptors(&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>())
}

fn assert_service_refuses(
    subject: Value,
    manifest: Value,
    artifacts: &[(String, Vec<u8>)],
    label: &str,
) {
    let parent = temporary_parent();
    let paths = stage_artifacts(parent.path(), artifacts);
    let fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PrivacyProtocolOrigin,
        subject,
        manifest,
    );
    let service = provision(parent.path(), fixture.role);
    assign_active_run(&service, &fixture);
    assert_eq!(
        authorize(
            &service,
            &fixture.request_json(),
            descriptors(&paths),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected),
        "service accepted mutated privacy-protocol origin {label}",
    );
}

#[test]
fn privacy_protocol_origin_full_service_authorize_replay_recover_and_verify() {
    let (subject, artifacts) =
        super::super::privacy_protocol_origin::tests::service_fixture_material();
    let parent = temporary_parent();
    let paths = stage_artifacts(parent.path(), &artifacts);
    let fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PrivacyProtocolOrigin,
        subject,
        manifest_value(&artifacts),
    );
    let service = provision(parent.path(), fixture.role);
    assign_active_run(&service, &fixture);

    let authorized = authorize(
        &service,
        &fixture.request_json(),
        descriptors(&paths),
        TEST_NOW_MILLIS_V1,
    )
    .expect("authorize exact privacy-protocol origin evidence");
    assert_eq!(authorized.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&authorized), "authorized");
    let authority_envelope = sidecar_bytes(&authorized, "authority_envelope");
    let durable_receipt = sidecar_bytes(&authorized, "durable_receipt");
    assert!(!authority_envelope.is_empty());
    assert!(!durable_receipt.is_empty());

    let persisted: BTreeMap<[u8; 32], StoredAuthorizationV1> =
        load_canonical_records(&state_directory(parent.path()).join("authority-receipts-v1"))
            .expect("load persisted privacy-protocol sidecars");
    let stored = persisted
        .get(&fixture.operation_id)
        .expect("persisted privacy-protocol authorization");
    assert_eq!(stored.authority_envelope_json, authority_envelope);
    assert_eq!(stored.durable_receipt_json, durable_receipt);

    let after_authorize = service
        .provenance()
        .expect("privacy-protocol authorization provenance");
    let retried = authorize(
        &service,
        &fixture.request_json(),
        descriptors(&paths),
        TEST_NOW_MILLIS_V1 + 1,
    )
    .expect("retry exact privacy-protocol authorization");
    assert_eq!(retried.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&retried, "authority_envelope"),
        authority_envelope
    );
    assert_eq!(sidecar_bytes(&retried, "durable_receipt"), durable_receipt);
    assert_eq!(
        service.provenance().expect("retry provenance"),
        after_authorize,
        "exact retry must not create a signer commit",
    );

    let verification = verification_json(&fixture, &authorized);
    let client_uid = service
        .public_binding()
        .expect("privacy-protocol binding")
        .signer
        .client_uid;
    drop(service);
    let recovered = TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
        .expect("reopen privacy-protocol authority");
    assert_eq!(
        recovered
            .provenance()
            .expect("recovered privacy-protocol provenance"),
        after_authorize,
    );
    let verified = recovered
        .verify_json(&verification, descriptors(&paths), client_uid)
        .expect("historically verify privacy-protocol sidecars");
    assert_eq!(verified.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&verified), "valid");
    assert_eq!(
        recovered
            .provenance()
            .expect("historical verification provenance"),
        after_authorize,
        "historical verification must not create a signer commit",
    );
}

#[test]
fn privacy_protocol_origin_service_rejects_each_signed_subject_field_mutation() {
    for field in SIGNED_SUBJECT_FIELDS_V1 {
        let (mut subject, artifacts) =
            super::super::privacy_protocol_origin::tests::service_fixture_material();
        subject
            .as_object_mut()
            .expect("privacy-protocol subject")
            .insert(field.into(), Value::Null);
        assert_service_refuses(subject, manifest_value(&artifacts), &artifacts, field);
    }
}

#[test]
fn privacy_protocol_origin_service_rejects_manifest_and_artifact_mutations() {
    for case in ["name", "ordinal", "size", "sha256"] {
        let (subject, artifacts) =
            super::super::privacy_protocol_origin::tests::service_fixture_material();
        let mut manifest = manifest_value(&artifacts);
        let row = manifest
            .as_array_mut()
            .and_then(|rows| rows.first_mut())
            .and_then(Value::as_object_mut)
            .expect("first manifest row");
        match case {
            "name" => {
                row.insert("name".into(), Value::from("evidence/substituted.bin"));
            }
            "ordinal" => {
                row.insert("ordinal".into(), Value::from(1_u64));
            }
            "size" => {
                let size = row
                    .get("size")
                    .and_then(Value::as_u64)
                    .expect("artifact size");
                row.insert("size".into(), Value::from(size + 1));
            }
            "sha256" => {
                row.insert("sha256".into(), Value::from("00".repeat(32)));
            }
            _ => unreachable!("closed mutation cases"),
        }
        assert_service_refuses(subject, manifest, &artifacts, case);
    }

    let (subject, artifacts) =
        super::super::privacy_protocol_origin::tests::service_fixture_material();
    let manifest = manifest_value(&artifacts);
    let mut changed = artifacts.clone();
    changed[0].1[0] ^= 0x80;
    assert_service_refuses(subject, manifest, &changed, "artifact bytes");
}
