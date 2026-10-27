//! Ordinary composer controls; these DATA fixtures neither admit E1 nor approve eligibility.
use super::*;

fn request() -> Request {
    let reference = |name: &str, byte: u8| Original {
        path: PathBuf::from(name),
        sha256: hex::encode([byte; 32]),
    };
    Request {
        schema: "iroha.kagemusha.engineering-issuer-provisioning.v1".into(),
        issuer_request: reference("/private/campaign/issuer-request.json", 1),
        setup: reference("/private/campaign/live/setup/setup.json", 2),
        registered: reference("/private/campaign/live/registered.json", 3),
        peer_index: 0,
        private_parent: "/private/runtime".into(),
        output_name: "issuer".into(),
        scope_hex: hex::encode([4; 32]),
        application_scopes: scopes::Originals {
            audience: reference("/private/runtime/audience.bin", 5),
            release: reference("/private/runtime/release.bin", 6),
            service_origin: reference("/private/runtime/service-origin.bin", 7),
        },
        observation_endpoint: "https://127.0.0.1:8443/eligibility".into(),
        observation_credential: "/private/runtime/transport.key".into(),
        observation_tls_root: None,
        worker: KagemushaEnrollmentWorker {
            python_executable: "/usr/bin/python3.12".into(),
            python_sha256_hex: hex::encode([8; 32]),
            verifier_archive: "/private/runtime/wallet-e1.pyz".into(),
            verifier_sha256_hex: hex::encode([9; 32]),
            openssl_executable: "/usr/bin/openssl".into(),
            openssl_sha256_hex: hex::encode([10; 32]),
            attestation_root: None,
            store_directory: "/private/runtime/worker".into(),
            exchange_timeout_ms: 60_000,
        },
        android: Android {
            package_name: "org.example.wallet".into(),
            minimum_package_version: 7,
            signing_certificates_der: vec![],
            roots: vec![],
            patch_floor_yyyymm: 202_610,
        },
        maximum_response_ms: 5_000,
        challenge_lifetime_ms: 120_000,
    }
}
fn documents(request: &Request) -> (Value, Value, Value) {
    let handoff = norito::json!({"schema":"iroha.kagemusha.network-controller-issuer-request.v1",
        "setup_sha256":(request.setup.sha256.clone()), "registered_sha256":(request.registered.sha256.clone()),
        "scheme_id":(hex::encode([11;32])), "manifest_digest":(hex::encode([12;32])),
        "verifier_pack":{"path":"/private/campaign/catalog/engineering-verifier-pack.norito", "sha256":(hex::encode([13;32]))},
        "selection_path":"/private/campaign/issuer-selection.json", "scope":"DATA"});
    let setup = norito::json!({"schema":"iroha.kagemusha.executed-ledger-setup.v1"});
    let registered = norito::json!({"schema":"iroha.kagemusha.network-registered.v1",
        "setup_sha256":(request.setup.sha256.clone()), "scheme_id":(hex::encode([11;32])),
        "manifest_digest":(hex::encode([12;32])), "verifier_pack_sha256":(hex::encode([13;32]))});
    (handoff, setup, registered)
}
#[test]
fn issuer_composition_refuses_cross_occurrence_handoffs_and_wrong_destination() {
    let mut request = request();
    let (handoff, setup, registered) = documents(&request);
    bindings(&request, &handoff, &setup, &registered).unwrap();
    for field in [
        "setup_sha256",
        "registered_sha256",
        "scheme_id",
        "manifest_digest",
        "selection_path",
    ] {
        let mut changed = handoff.clone();
        changed
            .as_object_mut()
            .unwrap()
            .insert(field.into(), Value::String(hex::encode([99; 32])));
        assert!(
            bindings(&request, &changed, &setup, &registered).is_err(),
            "{field}"
        );
    }
    for field in [
        "setup_sha256",
        "scheme_id",
        "manifest_digest",
        "verifier_pack_sha256",
    ] {
        let mut changed = registered.clone();
        changed
            .as_object_mut()
            .unwrap()
            .insert(field.into(), Value::String(hex::encode([99; 32])));
        assert!(
            bindings(&request, &handoff, &setup, &changed).is_err(),
            "{field}"
        );
    }
    let mut changed = handoff.clone();
    changed
        .as_object_mut()
        .unwrap()
        .insert("approved".into(), Value::Bool(true));
    assert!(bindings(&request, &changed, &setup, &registered).is_err());
    request.peer_index = 4;
    assert!(bindings(&request, &handoff, &setup, &registered).is_err());
    request.peer_index = 0;
    request.worker.exchange_timeout_ms = 0;
    assert!(bindings(&request, &handoff, &setup, &registered).is_err());
    request.worker.exchange_timeout_ms = 300_001;
    assert!(bindings(&request, &handoff, &setup, &registered).is_err());
    request.worker.exchange_timeout_ms = 300_000;
    request.scope_hex = hex::encode([0; 32]);
    assert!(bindings(&request, &handoff, &setup, &registered).is_err());
}
#[test]
fn issuer_certificate_is_rooted_enrollment_only_and_foreign_root_is_refused() {
    let root = SigningKey::from_slice(&[0x11; 32]).unwrap();
    let signer = SigningKey::from_slice(&[0x31; 32]).unwrap();
    let mut scheme = KagemushaWalletSchemeV1 {
        version: 1,
        network_id: *Hash::prehashed([2; 32]).as_ref(),
        scheme_root_key: public(&root).unwrap(),
        relation_id: [3; 32],
        provider_contract: kagemusha_wallet_provider_contract_v1(),
    };
    let credential = certificate(&scheme, &signer).unwrap();
    credential
        .verify_role(&scheme, KagemushaWalletSignerRoleV1::Enrollment)
        .unwrap();
    assert!(
        credential
            .verify_role(&scheme, KagemushaWalletSignerRoleV1::Artifact)
            .is_err()
    );
    assert_eq!(credential.body.key, public(&signer).unwrap());
    scheme.scheme_root_key = public(&SigningKey::from_slice(&[0x32; 32]).unwrap()).unwrap();
    assert!(certificate(&scheme, &signer).is_err());
}
#[test]
fn issuer_platform_uses_selected_originals_and_preserves_default_off_controls() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary
        .path()
        .canonicalize()
        .unwrap()
        .join("selected.der");
    fs::write(&path, b"public selected DER DATA").unwrap();
    let selected = || Original {
        path: path.clone(),
        sha256: hash(b"public selected DER DATA"),
    };
    let mut request = request();
    request.android.signing_certificates_der.push(selected());
    request.android.roots.push(AndroidRoot {
        family: "google-ca1".into(),
        spki: selected(),
    });
    let mut inputs = Inputs(Vec::new());
    let (app, policy) = platform(&request, [21; 32], &mut inputs).unwrap();
    assert_eq!(app.scheme_id, [21; 32]);
    assert_eq!(policy.app_policy, app.policy_digest().unwrap());
    assert_eq!(
        policy.regulatory_policy,
        KagemushaWalletRegulatoryPolicyV1::default()
    );
    assert_eq!(policy.attestation_lease_lifetime_ms, 0);
    assert_eq!(policy.challenge_lifetime_ms, 120_000);
    assert_eq!(
        KagemushaWalletEnrollmentPolicyTemplateV1::decode_canonical(
            &policy.encode_canonical().unwrap(),
            &[21; 32]
        )
        .unwrap(),
        policy
    );
    inputs.revalidate().unwrap();
    request.android.roots[0].family = "huawei".into();
    assert!(platform(&request, [21; 32], &mut Inputs(Vec::new())).is_err());
    request.android.roots[0].family = "google-ca1".into();
    request.worker.attestation_root = Some(path.clone());
    assert!(platform(&request, [21; 32], &mut Inputs(Vec::new())).is_err());
    fs::write(&path, b"replaced selected input").unwrap();
    assert!(inputs.revalidate().is_err());
}

fn private_test_directory_in(parent: &Path) -> tempfile::TempDir {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(parent)
            .unwrap()
    }
    #[cfg(not(unix))]
    tempfile::tempdir_in(parent).unwrap()
}

fn private_test_directory() -> tempfile::TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/qualification/issuer-provisioning-tests");
    fs::create_dir_all(&base).unwrap();
    private_test_directory_in(&base.canonicalize().unwrap())
}

#[test]
fn issuer_output_parent_is_bound_to_current_qualification_before_creation() {
    let temporary = private_test_directory();
    let mut request = request();
    request.private_parent = temporary.path().to_owned();
    let (parent, output) = selected_output_parent(&request).unwrap();
    assert_eq!(parent.path(), temporary.path());
    assert_eq!(output, temporary.path().join("issuer"));
    assert!(!output.exists());
    assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
    parent.revalidate().unwrap();
    drop(parent);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        assert_eq!(
            temporary.path().metadata().unwrap().permissions().mode() & 0o7777,
            0o700
        );
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let error = selected_output_parent(&request).err().unwrap();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            temporary.path().metadata().unwrap().permissions().mode() & 0o7777,
            0o755
        );
        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
    }
}

#[test]
fn issuer_output_refuses_external_and_traversing_parent_before_creation() {
    let temporary = private_test_directory();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let external = private_test_directory_in(&repository.join("target"));
    let external_path = external.path().canonicalize().unwrap();
    let external_owner = PrivateDirectory::open_exact(&external_path).unwrap();
    let mut request = request();
    for parent in [
        external_path,
        repository.to_owned(),
        repository.join("target/qualification-other"),
        temporary.path().join(".."),
        PathBuf::from("target/qualification"),
    ] {
        request.private_parent = parent;
        assert!(selected_output_parent(&request).is_err());
    }
    assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
    assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0);
    external_owner.revalidate().unwrap();
}

#[test]
fn issuer_output_refuses_non_child_names_and_preserves_existing_custody() {
    let temporary = private_test_directory();
    let mut request = request();
    request.private_parent = temporary.path().to_owned();
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "/absolute",
        "nested/child",
        "issuer/",
        "issuer/.",
    ] {
        request.output_name = name.into();
        assert!(selected_output_parent(&request).is_err(), "{name}");
    }
    assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
    request.output_name = "issuer".into();
    let output = temporary.path().join("issuer");
    fs::write(&output, b"partial custody").unwrap();
    assert!(selected_output_parent(&request).is_err());
    assert_eq!(fs::read(&output).unwrap(), b"partial custody");
    fs::remove_file(&output).unwrap();
    fs::create_dir(&output).unwrap();
    fs::write(output.join("retained"), b"original custody").unwrap();
    assert!(selected_output_parent(&request).is_err());
    assert_eq!(
        fs::read(output.join("retained")).unwrap(),
        b"original custody"
    );
}

#[test]
#[cfg(unix)]
fn issuer_output_refuses_aliased_parent_and_dangling_output() {
    let temporary = private_test_directory();
    let mut request = request();
    let alias = temporary.path().join("alias");
    std::os::unix::fs::symlink(temporary.path(), &alias).unwrap();
    request.private_parent = alias;
    assert!(selected_output_parent(&request).is_err());
    assert!(!temporary.path().join("issuer").exists());
    request.private_parent = temporary.path().to_owned();
    let output = temporary.path().join("issuer");
    std::os::unix::fs::symlink(temporary.path().join("missing"), &output).unwrap();
    assert!(selected_output_parent(&request).is_err());
    assert_eq!(
        fs::read_link(&output).unwrap(),
        temporary.path().join("missing")
    );
}

#[test]
fn selected_inputs_require_exact_bounded_bytes_and_private_custody() {
    let temporary = private_test_directory();
    let directory = PrivateDirectory::open_exact(temporary.path()).unwrap();
    directory
        .write_atomic("selected", b"selected DATA", PublishMode::CreateNew)
        .unwrap();
    let path = temporary.path().join("selected");
    let original = Original {
        path: path.clone(),
        sha256: hash(b"selected DATA"),
    };
    let mut inputs = Inputs(Vec::new());
    assert_eq!(inputs.read(&original, 64).unwrap(), b"selected DATA");
    inputs.retain_private(&path, 64).unwrap();
    inputs.program(&path, &original.sha256, 64).unwrap();
    // The hashing helper does not claim these bytes are executable code.
    inputs.revalidate().unwrap();
    assert!(Inputs(Vec::new()).read(&original, 3).is_err());
    assert!(Inputs(Vec::new()).retain_private(&path, 3).is_err());
    assert!(
        Inputs(Vec::new())
            .program(&path, &hex::encode([99; 32]), 64)
            .is_err()
    );
    assert!(
        Inputs(Vec::new())
            .read(
                &Original {
                    path: PathBuf::from("relative"),
                    sha256: original.sha256.clone()
                },
                64
            )
            .is_err()
    );
    fs::write(&path, b"modified DATA").unwrap();
    assert!(inputs.revalidate().is_err());
}

#[test]
fn configuration_preserves_canonical_policies_worker_and_optional_tls_selection() {
    let temporary = private_test_directory();
    let directory = PrivateDirectory::open_exact(temporary.path()).unwrap();
    directory
        .write_atomic(
            "selected.der",
            b"selected public DATA",
            PublishMode::CreateNew,
        )
        .unwrap();
    let mut request = request();
    let selected = || Original {
        path: temporary.path().join("selected.der"),
        sha256: hash(b"selected public DATA"),
    };
    request.android.signing_certificates_der.push(selected());
    request.android.roots.push(AndroidRoot {
        family: "google-ca1".into(),
        spki: selected(),
    });
    let root = SigningKey::from_slice(&[0x11; 32]).unwrap();
    let scheme = KagemushaWalletSchemeV1 {
        version: 1,
        network_id: *Hash::prehashed([2; 32]).as_ref(),
        scheme_root_key: public(&root).unwrap(),
        relation_id: [3; 32],
        provider_contract: kagemusha_wallet_provider_contract_v1(),
    };
    let scheme_id = scheme.scheme_id();
    let (app, enrollment) = platform(&request, scheme_id, &mut Inputs(Vec::new())).unwrap();
    let certificate = certificate(&scheme, &SigningKey::from_slice(&[0x31; 32]).unwrap()).unwrap();
    let observer = KeyPair::from_seed(vec![51; 32], Algorithm::Ed25519);
    let scopes = scopes::from_originals(
        b"engineering-operator".to_vec(),
        b"selected release DATA".to_vec(),
        b"https://wallet.example".to_vec(),
    )
    .unwrap();
    let eligibility = KagemushaEligibilityPolicyTemplateV1 {
        version: 1,
        network_id: scheme.network_id,
        scheme_id,
        revision: 1,
        authority: KagemushaEligibilityAuthorityV1::SchemeOperator {
            operator_digest: scopes.operator_digest,
        },
        public_key: observer.public_key().to_bytes().1.try_into().unwrap(),
        maximum_response_ms: request.maximum_response_ms,
    };
    let template = eligibility.encode_canonical().unwrap();
    let manifest = [42; 32];
    for extra_root in [false, true] {
        request.observation_tls_root = extra_root.then(selected);
        let encoded = configuration::encode(
            &request,
            temporary.path(),
            configuration::Originals {
                scheme: &scheme,
                app: &app,
                enrollment: &enrollment,
                certificate: &certificate,
                template: &template,
                manifest,
                scopes: &scopes,
            },
        )
        .unwrap();
        let layer = parse_layer(&encoded, &scheme, manifest).unwrap();
        let table = layer["torii"]["kagemusha_enrollment"]
            .as_table()
            .unwrap()
            .clone();
        let parsed = ConfigReader::new()
            .without_env()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<KagemushaEnrollmentIssuer>()
            .unwrap();
        assert_eq!(parsed.revision, 1);
        assert_eq!(parsed.request_timeout_ms, request.maximum_response_ms);
        assert_eq!(parsed.max_inflight, 1);
        assert_eq!(parsed.journal_dir, temporary.path().join("journal"));
        assert_eq!(parsed.providers.len(), 1);
        let provider = &parsed.providers[0];
        assert_eq!(provider.eligibility_template_hex, hex::encode(&template));
        assert_eq!(
            provider.release_digest_hex,
            hex::encode(scopes.release_digest)
        );
        assert_eq!(
            provider.service_origin_digest_hex,
            hex::encode(scopes.service_origin_digest)
        );
        assert_eq!(
            provider.app_hex,
            hex::encode(app.encode_canonical().unwrap())
        );
        assert_eq!(
            provider.enrollment_template_hex,
            hex::encode(enrollment.encode_canonical().unwrap())
        );
        assert_eq!(
            provider.certificate_hex,
            hex::encode(certificate.to_canonical_bytes().unwrap())
        );
        assert_eq!(provider.observation_endpoint, request.observation_endpoint);
        assert_eq!(
            provider.observation_credential,
            request.observation_credential
        );
        assert_eq!(
            provider.observation_tls_root.as_ref(),
            request.observation_tls_root.as_ref().map(|root| &root.path)
        );
        assert_eq!(
            provider.signer_private_key,
            temporary.path().join("enrollment-signer.key")
        );
        assert_eq!(
            provider.worker.python_executable,
            request.worker.python_executable
        );
        assert_eq!(
            provider.worker.python_sha256_hex,
            request.worker.python_sha256_hex
        );
        assert_eq!(
            provider.worker.verifier_archive,
            request.worker.verifier_archive
        );
        assert_eq!(
            provider.worker.verifier_sha256_hex,
            request.worker.verifier_sha256_hex
        );
        assert_eq!(
            provider.worker.openssl_executable,
            request.worker.openssl_executable
        );
        assert_eq!(
            provider.worker.openssl_sha256_hex,
            request.worker.openssl_sha256_hex
        );
        assert_eq!(
            provider.worker.store_directory,
            request.worker.store_directory
        );
        assert_eq!(
            provider.worker.exchange_timeout_ms,
            request.worker.exchange_timeout_ms
        );
        assert!(provider.worker.attestation_root.is_none());
        assert_eq!(
            KagemushaEligibilityPolicyTemplateV1::decode_canonical(
                &hex::decode(&provider.eligibility_template_hex).unwrap()
            )
            .unwrap(),
            eligibility
        );
    }
}

#[test]
fn selection_is_published_only_after_originals_and_never_overwrites() {
    for alter_input in [false, true] {
        let temporary = private_test_directory();
        let parent = PrivateDirectory::open_exact(temporary.path()).unwrap();
        let output = parent.create_child("issuer").unwrap();
        let output_path = temporary.path().join("issuer");
        let mut request = request();
        request.issuer_request.path = temporary.path().join("issuer-request.json");
        let mut inputs = Inputs(Vec::new());
        let configuration = b"configuration DATA; this publication test starts no service";
        assert!(publish_selection(&request, &output_path, configuration, &mut inputs).is_err());
        assert!(!temporary.path().join("issuer-selection.json").exists());
        parent
            .write_atomic("selected", b"input DATA", PublishMode::CreateNew)
            .unwrap();
        inputs
            .read(
                &Original {
                    path: temporary.path().join("selected"),
                    sha256: hash(b"input DATA"),
                },
                64,
            )
            .unwrap();
        output
            .write_atomic("issuer.toml", configuration, PublishMode::CreateNew)
            .unwrap();
        if alter_input {
            fs::write(temporary.path().join("selected"), b"changed DATA").unwrap();
            assert!(publish_selection(&request, &output_path, configuration, &mut inputs).is_err());
            assert!(!temporary.path().join("issuer-selection.json").exists());
            continue;
        }
        assert!(
            publish_selection(
                &request,
                &output_path,
                b"different DATA",
                &mut Inputs(Vec::new())
            )
            .is_err()
        );
        assert!(!temporary.path().join("issuer-selection.json").exists());
        publish_selection(&request, &output_path, configuration, &mut inputs).unwrap();
        let original = parent.read("issuer-selection.json", INPUT_MAX).unwrap();
        let selected: Value = norito::json::from_slice(&original).unwrap();
        exact_keys(
            &selected,
            &[
                "schema",
                "setup_sha256",
                "registered_sha256",
                "peer_index",
                "configuration",
            ],
        )
        .unwrap();
        assert_eq!(
            selected["schema"].as_str(),
            Some("iroha.kagemusha.network-controller-issuer-selection.v1")
        );
        assert_eq!(
            selected["setup_sha256"].as_str(),
            Some(request.setup.sha256.as_str())
        );
        assert_eq!(
            selected["registered_sha256"].as_str(),
            Some(request.registered.sha256.as_str())
        );
        assert_eq!(
            selected["peer_index"].as_u64(),
            Some(u64::from(request.peer_index))
        );
        assert_eq!(
            selected["configuration"]["path"].as_str(),
            output_path.join("issuer.toml").to_str()
        );
        assert_eq!(
            selected["configuration"]["sha256"].as_str(),
            Some(hash(configuration).as_str())
        );
        assert!(publish_selection(&request, &output_path, configuration, &mut inputs).is_err());
        assert_eq!(
            &*parent.read("issuer-selection.json", INPUT_MAX).unwrap(),
            &*original
        );
    }
}

#[test]
fn provisioning_request_has_closed_fields_and_explicit_worker_selection() {
    let original = norito::json!({"path":"/private/selected", "sha256": (hex::encode([3;32]))});
    let worker = norito::json!({
        "python_executable":"/private/python", "python_sha256_hex": (hex::encode([4;32])),
        "verifier_archive":"/private/verifier.pyz", "verifier_sha256_hex":(hex::encode([5;32])),
        "openssl_executable":"/private/openssl", "openssl_sha256_hex":(hex::encode([6;32])),
        "attestation_root":null, "store_directory":"/private/store", "exchange_timeout_ms":1000});
    let value = norito::json!({
        "schema":"iroha.kagemusha.engineering-issuer-provisioning.v1",
        "issuer_request":(original.clone()), "setup":(original.clone()), "registered":(original.clone()),
        "peer_index":0, "private_parent":"/private/runtime", "output_name":"issuer",
        "scope_hex":(hex::encode([7;32])),
        "application_scopes":{"audience":(original.clone()), "release":(original.clone()), "service_origin":(original.clone())},
        "observation_endpoint":"https://127.0.0.1:8443/eligibility",
        "observation_credential":"/private/transport", "observation_tls_root":null,
        "worker":worker, "android": {"package_name":"org.example.wallet", "minimum_package_version":1,
            "signing_certificates_der":[(original.clone())], "roots":[{"family":"google-ca1", "spki":original}],
            "patch_floor_yyyymm":202610}, "maximum_response_ms":5000, "challenge_lifetime_ms":120000});
    let decode = |value: &Value| decode_request(&norito::json::to_vec(value).unwrap());
    assert!(decode(&value).is_ok());
    for case in 0..9 {
        let mut changed = value.clone();
        match case {
            0 => {
                changed
                    .as_object_mut()
                    .unwrap()
                    .insert("approved".into(), Value::Bool(true));
            }
            1 => {
                changed
                    .get_mut("worker")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("unverified".into(), Value::Bool(true));
            }
            2 => {
                changed
                    .get_mut("worker")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove("python_sha256_hex");
            }
            3 => {
                changed
                    .get_mut("issuer_request")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("body".into(), Value::String("unexpected".into()));
            }
            4 => {
                changed
                    .get_mut("android")
                    .unwrap()
                    .get_mut("roots")
                    .unwrap()
                    .get_mut(0)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("trusted".into(), Value::Bool(true));
            }
            5..=7 => {
                let retired = [
                    "operator_digest_hex",
                    "release_digest_hex",
                    "service_origin_digest_hex",
                ];
                changed.as_object_mut().unwrap().insert(
                    retired[case - 5].into(),
                    Value::String(hex::encode([9; 32])),
                );
            }
            _ => {
                changed
                    .get_mut("application_scopes")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("approved".into(), Value::Bool(true));
            }
        }
        assert!(decode(&changed).is_err(), "case {case}");
    }
}
