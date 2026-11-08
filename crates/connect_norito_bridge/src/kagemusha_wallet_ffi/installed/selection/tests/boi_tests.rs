//! BOI signed selection only: genuine metadata signatures and genesis, no admitted graph.

use super::*;

struct BoiFixture(BaseFixture, Value);
impl BoiFixture {
    fn new() -> Self {
        Self::with_asset("839FV3NJC8NfgWQvghXU2hEFQm9a")
    }
    fn with_asset(asset: &str) -> Self {
        Self::with_asset_and_mode(asset, SumeragiConsensusMode::Permissioned)
    }
    fn with_asset_and_mode(asset: &str, mode: SumeragiConsensusMode) -> Self {
        let short_mode = match &mode {
            SumeragiConsensusMode::Permissioned => "permissioned",
            SumeragiConsensusMode::Npos => "npos",
        };
        let mut base = BaseFixture::with_asset_and_mode(asset, mode);
        // Complete public BOI v1 DATA fixture from the maintained Android runtime
        // fixture. Replace its graph selections with this genuine signed genesis and
        // genuine scheme/certificate/policy originals; no proof graph is fabricated.
        let mut app: Value = norito::json::from_slice(include_bytes!("boi_app_v1.json")).unwrap();
        let node_source_commit = app["consensus"]["nodeSourceCommit"].clone();
        // The UI fixture permits an absent APK and a trailing portal slash. The
        // Taira release composer emits these exact production service selectors.
        let services = object_mut(&mut app).get_mut("services").unwrap();
        mutate(
            services,
            "operationsPortalUrl",
            Value::String("https://boi.soramitsu.io".into()),
        );
        mutate(
            services,
            "androidApkUrl",
            Value::String("https://boi.soramitsu.io/downloads/android/digital-shekel.apk".into()),
        );
        // App UI fixtures contain placeholder account text. Admission tests need
        // genuine canonical native accounts, with distinct identities for every
        // bank and role under the explicit BOI chain discriminant.
        let Value::Array(participants) = object_mut(&mut app).get_mut("participants").unwrap()
        else {
            panic!("participants")
        };
        for (bank, participant) in participants.iter_mut().enumerate() {
            for (role, name) in [
                "settlementAccountId",
                "runtimeAccountId",
                "enrollmentAccountId",
            ]
            .into_iter()
            .enumerate()
            {
                let key =
                    KeyPair::from_seed(vec![40 + (bank * 3 + role) as u8; 32], Algorithm::Ed25519);
                let account = iroha_data_model::account::AccountId::new(key.public_key().clone());
                let address =
                    iroha_data_model::account::address::AccountAddress::from_account_id(&account)
                        .unwrap()
                        .to_i105_for_discriminant(369)
                        .unwrap();
                mutate(participant, name, Value::String(address));
            }
        }
        mutate(
            object_mut(&mut app).get_mut("digitalShekel").unwrap(),
            "assetDefinitionId",
            Value::String(asset.into()),
        );
        let digital_shekel = object_mut(&mut app).get_mut("digitalShekel").unwrap();
        mutate(
            digital_shekel,
            "owningDomain",
            Value::String("boi.universal".into()),
        );
        mutate(digital_shekel, "owningDataspace", Value::String("0".into()));
        mutate(
            digital_shekel,
            "balanceScopePolicy",
            Value::String("Global".into()),
        );
        let native = native_installation_mut(&mut base.app);
        let mut ledger = native["ledger"].clone();
        mutate(
            &mut ledger,
            "toriiUrl",
            Value::String("https://taira.sora.org".into()),
        );
        mutate(&mut app, "ledger", ledger);
        let mut consensus = native["consensus"].clone();
        // Preserve the maintained BOI v1 renderer grammar while replacing only
        // its public genesis/roster selections with genuine fixture originals. BOI
        // signs no finality-verifier pin.
        object_mut(&mut consensus).remove("finalityVerifierSha256");
        mutate(&mut consensus, "nodeSourceCommit", node_source_commit);
        mutate(&mut consensus, "mode", Value::String(short_mode.into()));
        for name in [
            "checkpointSha256",
            "checkpointHeight",
            "checkpointContextId",
        ] {
            mutate(&mut consensus, name, Value::Null);
        }
        let Value::Array(validators) = object_mut(&mut consensus).get_mut("validators").unwrap()
        else {
            panic!("validators")
        };
        for (index, row) in validators.iter_mut().enumerate() {
            mutate(
                row,
                "directToriiUrl",
                Value::String(boi_registry::BOI_VALIDATOR_BASES[index].into()),
            );
            mutate(
                row,
                "nodeFingerprint",
                Value::String(hex::encode([index as u8 + 1; 32])),
            );
        }
        mutate(&mut app, "consensus", consensus);
        base.app = app;
        mutate(
            &mut base.runtime,
            "schema",
            Value::String("boi.current-wallet-core-runtime.v1".into()),
        );
        mutate(
            &mut base.runtime,
            "enrollment_sessions",
            boi_session_selections(),
        );
        let (_, eligibility_key) = base.key.public_key().to_bytes();
        let template = iroha_data_model::kagemusha::KagemushaEligibilityPolicyTemplateV1 {
            version: 1,
            network_id: base.scheme.network_id,
            scheme_id: base.scheme.scheme_id(),
            revision: 1,
            authority: iroha_data_model::kagemusha::KagemushaEligibilityAuthorityV1::Bank {
                fi_digest: [7; 32],
            },
            public_key: eligibility_key.try_into().unwrap(),
            maximum_response_ms: 30_000,
        };
        mutate(
            &mut base.runtime,
            "eligibility_template_hex",
            Value::String(hex::encode(template.encode_canonical().unwrap())),
        );
        for (name, value) in [
            ("python_path", "/usr/bin/python3"),
            ("openssl_path", "/usr/bin/openssl"),
        ] {
            mutate(&mut base.runtime, name, Value::String(value.into()));
        }
        for name in ["python_sha256", "openssl_sha256"] {
            mutate(&mut base.runtime, name, Value::String("55".repeat(32)));
        }
        // Only Apple selects a root DER original; Android roots are SPKI pins in its policy.
        mutate(
            object_mut(&mut base.runtime).get_mut("apple").unwrap(),
            "attestation_root_der_base64",
            Value::String(STANDARD.encode(SESSION_ROOT)),
        );
        for name in ["android", "apple"] {
            mutate(
                object_mut(&mut base.runtime).get_mut(name).unwrap(),
                "verifier_configuration_path",
                Value::String(format!("srv/etc/kagemusha/wallet-e1-{name}.json")),
            );
        }
        set_boi_platforms(&mut base);
        Self(base, financial_record())
    }
    fn originals(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let runtime = norito::json::to_json_bounded(&self.0.runtime, WALLET_RUNTIME_MAX)
            .unwrap()
            .into_bytes();
        let mut app = self.0.app.clone();
        mutate(
            &mut app,
            "walletRuntime",
            norito::json!({"schema":("boi.current-wallet-runtime-pin.v1"),"currentRuntimeSha256":(hex::encode(BlobV1::of(&runtime).sha256)),"financialOriginals":(self.1.clone()), "feeSponsor":(self.0.app["feeSponsor"].clone())}),
        );
        let app = canonical(&app);
        let envelope = boi_sign(&self.0.key, &app);
        (app, envelope, runtime)
    }
    fn load(&self) -> Result<Selection> {
        let (app, envelope, runtime) = self.originals();
        self.load_originals(&app, &envelope, &runtime)
    }
    fn load_originals(&self, app: &[u8], envelope: &[u8], runtime: &[u8]) -> Result<Selection> {
        self.load_offered(app, envelope, runtime, 7)
    }
    fn load_offered(
        &self,
        app: &[u8],
        envelope: &[u8],
        runtime: &[u8],
        offered: u8,
    ) -> Result<Selection> {
        Selection::load(
            &self.trust(),
            &RuntimeOriginals {
                app_manifest: app,
                envelope,
                wallet_runtime: runtime,
                verifier_pack: if offered & 1 != 0 { TEST_PACK } else { b"" },
                producer_inventory: if offered & 2 != 0 { TEST_CATALOG } else { b"" },
                signed_genesis: &self.0.genesis,
                originals_root: if offered & 4 != 0 { TEST_ROOT } else { b"" },
                registration_source: b"",
            },
        )
    }
    fn trust(&self) -> RuntimeTrust {
        RuntimeTrust {
            authority: RuntimeAuthority::BoiTairaV1,
            key: self.0.key.public_key().clone(),
        }
    }
}

fn boi_sign(key: &KeyPair, app: &[u8]) -> Vec<u8> {
    let mut message = b"boi:taira-app-runtime-manifest:v1\0".to_vec();
    message.extend_from_slice(app);
    let signature = Signature::try_new(key.private_key(), &message).unwrap();
    let (_, public) = key.public_key().to_bytes();
    canonical(
        &norito::json!({"schema":("boi.taira-app-runtime-manifest-signature.v1"),"algorithm":("ed25519"),"domain":("boi:taira-app-runtime-manifest:v1"),"keyId":(format!("sha256:{}",hex::encode(BlobV1::of(public).sha256))),"manifestSha256":(hex::encode(BlobV1::of(app).sha256)),"signatureBase64Url":(URL_SAFE_NO_PAD.encode(signature.payload()))}),
    )
}

fn financial_record() -> Value {
    let blob = |bytes: &[u8]| norito::json!({"bytes":(bytes.len()),"sha256":(hex::encode(BlobV1::of(bytes).sha256))});
    norito::json!({"schema":("iroha.kagemusha.wallet-financial-originals.v1"),
        "verifierPack":(blob(TEST_PACK)),"producerInventory":(blob(TEST_CATALOG)),
        "transport":(blob(TEST_TRANSPORT)),"producerCatalogDigest":(hex::encode([0x13; 32]))})
}

const TEST_TRANSPORT: &[u8] = b"PUBLIC_SOFTWARE_DATA_UNADMITTED_TRANSPORT";

fn set_boi_platforms(base: &mut BaseFixture) {
    use iroha_data_model::kagemusha::KagemushaWalletAppIdentityV1;
    for name in ["android", "apple"] {
        let value = object_mut(&mut base.runtime).get_mut(name).unwrap();
        let row = object(value).unwrap();
        let mut app = KagemushaWalletAppPolicyV1::decode_canonical(
            &raw(row, "app_policy_original_base64", 1024).unwrap(),
            &base.scheme.scheme_id(),
        )
        .unwrap();
        app.identity = match app.identity {
            KagemushaWalletAppIdentityV1::Android {
                minimum_package_version,
                signer_set,
                ..
            } => KagemushaWalletAppIdentityV1::Android {
                package_name: boi_registry::BOI_ANDROID_PACKAGE.into(),
                minimum_package_version,
                signer_set,
            },
            KagemushaWalletAppIdentityV1::Apple { .. } => KagemushaWalletAppIdentityV1::Apple {
                app_id: boi_registry::BOI_IOS_APP_ID.into(),
            },
        };
        let mut policy = KagemushaWalletEnrollmentPolicyV1::decode_canonical(
            &raw(row, "enrollment_policy_original_base64", 1024).unwrap(),
            &base.scheme.scheme_id(),
        )
        .unwrap();
        policy.app_policy = app.policy_digest().unwrap();
        mutate(
            value,
            "app_policy_hex",
            Value::String(hex::encode(app.policy_digest().unwrap())),
        );
        mutate(
            value,
            "app_policy_original_base64",
            Value::String(STANDARD.encode(norito::encode_canonical(&app).unwrap())),
        );
        mutate(
            value,
            "enrollment_policy_hex",
            Value::String(hex::encode(policy.policy_digest().unwrap())),
        );
        mutate(
            value,
            "enrollment_policy_original_base64",
            Value::String(STANDARD.encode(norito::encode_canonical(&policy).unwrap())),
        );
    }
}

fn boi_session_selections() -> Value {
    Value::Array(
        boi_registry::BOI_PARTICIPANTS
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kagemusha_issuer)
            .map(|(i, bank)| {
                let key = session_key(i as u8);
                let (_, public) = key.public_key().to_bytes();
                norito::json!({"schema":("boi.wallet-enrollment-session-selection.v1"),
            "jwt_issuer":(bank.session_issuer), "jwt_audience":(bank.session_audience),
            "fi_id":(bank.alias_scope), "jwt_public_key_hex":(hex::encode(public))})
            })
            .collect(),
    )
}
/// The form of account literal Core binds into its retail token: non-ASCII, unescaped UTF-8.
const CORE_ACCOUNT_LITERAL: &str = "testuﾛ1PﾀgﾘｷﾇsﾗAﾔsVﾘbﾁLoﾛﾆvﾇﾏﾑｳcfｸﾚxtｸJsﾇZ5ｵJuSV0000";
fn boi_claims(index: usize) -> Value {
    let bank = boi_registry::BOI_PARTICIPANTS[index];
    norito::json!({"sub":("owner@example.test"), "account_id":(CORE_ACCOUNT_LITERAL),
        "dataspace_id":(bank.alias_scope),
        "roles":["RETAIL_USER"], "iat":(1000), "nbf":(1000), "exp":(2000),
        "iss":(bank.session_issuer), "aud":(bank.session_audience)})
}
fn boi_token_from_bytes(claims: &[u8], key: &KeyPair) -> Vec<u8> {
    let header = URL_SAFE_NO_PAD.encode(br#"{"typ":"JWT","alg":"EdDSA"}"#);
    let claims = URL_SAFE_NO_PAD.encode(claims);
    let message = format!("{header}.{claims}");
    let signature = Signature::try_new(key.private_key(), message.as_bytes()).unwrap();
    format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature.payload())).into_bytes()
}
fn boi_token(claims: &Value, key: &KeyPair) -> Vec<u8> {
    boi_token_from_bytes(norito::json::to_json(claims).unwrap().as_bytes(), key)
}

#[test]
fn boi_signed_native_originals_select_exact_product() {
    let fixture = BoiFixture::new();
    let maintained: Value = norito::json::from_slice(include_bytes!("boi_app_v1.json")).unwrap();
    assert_eq!(
        fixture.0.app["consensus"]["nodeSourceCommit"],
        maintained["consensus"]["nodeSourceCommit"]
    );
    let selected = fixture.load().unwrap();
    assert!(matches!(
        selected.application,
        ApplicationBinding::Boi { .. }
    ));
    assert_eq!(
        selected.boi_sessions.len(),
        boi_registry::BOI_TAIRA_ISSUING_BANKS.len()
    );
    assert!(selected.bpng_session.is_none());
    assert!(selected.fi_sessions.is_empty());
}

#[test]
fn boi_node_source_commit_requires_the_maintained_schema_shape() {
    let fixture = BoiFixture::new();
    let (app, _, runtime) = fixture.originals();
    for replacement in [
        None,
        Some(Value::Null),
        Some(Value::String(String::new())),
        Some(Value::String("1".repeat(39))),
        Some(Value::String("1".repeat(41))),
        Some(Value::String("A".repeat(40))),
        Some(Value::String("g".repeat(40))),
        Some(Value::String("é".repeat(20))),
        Some(Value::String(format!(" {}", "1".repeat(40)))),
    ] {
        let mut changed: Value = norito::json::from_slice(&app).unwrap();
        let consensus = object_mut(&mut changed).get_mut("consensus").unwrap();
        if let Some(value) = replacement {
            mutate(consensus, "nodeSourceCommit", value);
        } else {
            object_mut(consensus).remove("nodeSourceCommit");
        }
        let changed = canonical(&changed);
        assert!(
            fixture
                .load_originals(&changed, &boi_sign(&fixture.0.key, &changed), &runtime)
                .is_err()
        );
    }
    // The maintained schema requires lowercase hexadecimal shape, not a particular
    // source revision or a nonzero value. Finality still derives from actual genesis.
    for commit in ["0".repeat(40), "a".repeat(40)] {
        let mut changed: Value = norito::json::from_slice(&app).unwrap();
        mutate(
            object_mut(&mut changed).get_mut("consensus").unwrap(),
            "nodeSourceCommit",
            Value::String(commit),
        );
        let changed = canonical(&changed);
        fixture
            .load_originals(&changed, &boi_sign(&fixture.0.key, &changed), &runtime)
            .unwrap();
    }
}

#[test]
fn boi_node_source_commit_does_not_extend_cbsi_consensus() {
    let mut fixture = BaseFixture::new();
    fixture.load().unwrap();
    mutate(
        native_installation_mut(&mut fixture.app)
            .get_mut("consensus")
            .unwrap(),
        "nodeSourceCommit",
        Value::String("1".repeat(40)),
    );
    assert!(fixture.load().is_err());
}

#[test]
fn boi_offline_pin_requires_its_signed_sponsor_object() {
    let fixture = BoiFixture::new();
    let (app, _, runtime) = fixture.originals();
    for missing in [false, true] {
        let mut app = norito::json::from_slice::<Value>(&app).unwrap();
        let pin = object_mut(object_mut(&mut app).get_mut("walletRuntime").unwrap());
        if missing {
            pin.remove("feeSponsor");
        } else {
            pin.insert("feeSponsor".into(), Value::Null);
        }
        let app = canonical(&app);
        let envelope = boi_sign(&fixture.0.key, &app);
        assert!(fixture.load_originals(&app, &envelope, &runtime).is_err());
    }
}

#[test]
fn boi_sessions_bind_issuing_banks_and_reject_cross_bank_signatures() {
    let fixture = BoiFixture::new();
    let selected = fixture.load().unwrap();
    for (index, bank) in boi_registry::BOI_PARTICIPANTS
        .iter()
        .enumerate()
        .filter(|(_, bank)| bank.kagemusha_issuer)
    {
        let token = boi_token(&boi_claims(index), &session_key(index as u8));
        for android in [false, true] {
            let root = if android {
                b"".as_slice()
            } else {
                SESSION_ROOT
            };
            let session = selected
                .enrollment_session(android, [&token, b"", root])
                .unwrap();
            assert_eq!(session.config.fi, bank.alias_scope.as_bytes());
            assert_eq!(
                session.config.service_origin,
                bank.session_issuer.as_bytes()
            );
            assert_eq!(
                session.config.release,
                BlobV1::of(&fixture.originals().0).sha256
            );
        }
        let foreign = boi_token(&boi_claims(index), &session_key(((index + 1) % 7) as u8));
        assert!(
            selected
                .enrollment_session(true, [&foreign, b"", b""])
                .is_err()
        );
        let mut claims = boi_claims(index);
        mutate(
            &mut claims,
            "aud",
            Value::String(
                boi_registry::BOI_PARTICIPANTS[(index + 1) % 7]
                    .alias_scope
                    .into(),
            ),
        );
        let bad_aud = boi_token(&claims, &session_key(index as u8));
        assert!(
            selected
                .enrollment_session(true, [&bad_aud, b"", b""])
                .is_err()
        );
    }
}

#[test]
fn boi_signed_application_refuses_localnet_foreign_asset_routes_and_participants() {
    let fixture = BoiFixture::new();
    for (section, field_name, value) in [
        ("", "environment", Value::String("localnet".into())),
        ("", "walletRuntime", Value::Null),
        (
            "digitalShekel",
            "assetAlias",
            Value::String("kina#bpng".into()),
        ),
        (
            "digitalShekel",
            "balanceScopePolicy",
            Value::String("Dataspace".into()),
        ),
        ("digitalShekel", "owningDomain", Value::Null),
        ("digitalShekel", "physicalLaneId", Value::from(3u64)),
        ("ledger", "networkPrefix", Value::from(753u64)),
        (
            "services",
            "coreApiUrl",
            Value::String("https://bpng-core.soramitsu.io".into()),
        ),
    ] {
        let mut app: Value = norito::json::from_slice(&fixture.originals().0).unwrap();
        let target = if section.is_empty() {
            &mut app
        } else {
            object_mut(&mut app).get_mut(section).unwrap()
        };
        mutate(target, field_name, value);
        let app = canonical(&app);
        let envelope = boi_sign(&fixture.0.key, &app);
        assert!(
            fixture
                .load_originals(&app, &envelope, &fixture.originals().2)
                .is_err(),
            "{section}.{field_name}"
        );
    }
}

#[test]
fn boi_signature_has_no_foreign_authority_or_key_fallback() {
    let fixture = BoiFixture::new();
    let (app, envelope, runtime) = fixture.originals();
    for authority in [
        RuntimeAuthority::BpngTairaV7,
        RuntimeAuthority::CbsiReleaseV1,
        RuntimeAuthority::ApplicationReleaseV1,
    ] {
        assert!(
            signed_app(
                &RuntimeTrust {
                    authority,
                    key: fixture.0.key.public_key().clone()
                },
                &app,
                &envelope
            )
            .is_err()
        );
    }
    let foreign = KeyPair::from_seed(vec![88; 32], Algorithm::Ed25519);
    assert!(
        fixture
            .load_originals(&app, &boi_sign(&foreign, &app), &runtime)
            .is_err()
    );
}

#[test]
fn boi_session_selection_refuses_duplicate_reordered_or_missing_rows() {
    for mode in 0..3 {
        let mut fixture = BoiFixture::new();
        let Value::Array(rows) = object_mut(&mut fixture.0.runtime)
            .get_mut("enrollment_sessions")
            .unwrap()
        else {
            panic!("rows")
        };
        match mode {
            0 => {
                // A duplicated issuing-bank row.
                let duplicate = rows[0].clone();
                rows.push(duplicate);
            }
            1 => {
                // A row naming a bank the registry does not select as an issuer.
                let foreign = boi_registry::BOI_PARTICIPANTS
                    .iter()
                    .find(|bank| !bank.kagemusha_issuer)
                    .unwrap();
                mutate(
                    &mut rows[0],
                    "fi_id",
                    Value::String(foreign.alias_scope.into()),
                );
            }
            _ => {
                rows.pop();
            }
        }
        assert!(fixture.load().is_err());
    }
}

#[test]
fn boi_signed_app_requires_exact_owner_grant_route() {
    for mode in 0..3 {
        let fixture = BoiFixture::new();
        let (bytes, _, runtime) = fixture.originals();
        let mut app: Value = norito::json::from_slice(&bytes).unwrap();
        let enrollment = object_mut(&mut app).get_mut("enrollment").unwrap();
        let routes = object_mut(object_mut(enrollment).get_mut("stageRoutes").unwrap());
        if mode == 0 {
            routes.remove("bankReadGrant");
        } else {
            let grant = routes.get_mut("bankReadGrant").unwrap();
            if mode == 1 {
                mutate(grant, "laneId", Value::from(1u64));
            } else {
                mutate(grant, "physicalDataspaceId", Value::String("1".into()));
            }
        }
        let bytes = canonical(&app);
        assert!(
            fixture
                .load_originals(&bytes, &boi_sign(&fixture.0.key, &bytes), &runtime)
                .is_err()
        );
    }
}

#[test]
fn boi_session_refuses_cross_scope_and_supplied_proof_or_root() {
    let fixture = BoiFixture::new();
    let selected = fixture.load().unwrap();
    let issuer_index = boi_registry::BOI_PARTICIPANTS
        .iter()
        .position(|bank| bank.kagemusha_issuer)
        .unwrap();
    let foreign_index = boi_registry::BOI_PARTICIPANTS
        .iter()
        .position(|bank| !bank.kagemusha_issuer)
        .unwrap();
    let good = boi_token(&boi_claims(issuer_index), &session_key(issuer_index as u8));
    assert!(selected.enrollment_session(true, [&good, b"", b""]).is_ok());
    for name in ["iss", "aud", "dataspace_id"] {
        let mut claims = boi_claims(issuer_index);
        let foreign = if name == "iss" {
            boi_registry::BOI_PARTICIPANTS[foreign_index].session_issuer
        } else {
            boi_registry::BOI_PARTICIPANTS[foreign_index].alias_scope
        };
        mutate(&mut claims, name, Value::String(foreign.into()));
        let token = boi_token(&claims, &session_key(issuer_index as u8));
        assert!(
            selected
                .enrollment_session(true, [&token, b"", b""])
                .is_err(),
            "{name}"
        );
    }
    let token = boi_token(&boi_claims(issuer_index), &session_key(issuer_index as u8));
    assert!(
        selected
            .enrollment_session(true, [&token, b"proof", b""])
            .is_err()
    );
    assert!(
        selected
            .enrollment_session(true, [&token, b"", SESSION_ROOT])
            .is_err()
    );
    assert!(
        selected
            .enrollment_session(false, [&token, b"", b"foreign root"])
            .is_err()
    );
}

#[test]
fn boi_session_accepts_the_bytes_core_signs_for_a_retail_owner() {
    // Field order and unescaped UTF-8 of Core's `RetailAuthClaims`, as serde_json writes them.
    let selected = BoiFixture::new().load().unwrap();
    let index = boi_registry::BOI_PARTICIPANTS
        .iter()
        .position(|bank| bank.kagemusha_issuer)
        .unwrap();
    let bank = boi_registry::BOI_PARTICIPANTS[index];
    let claims = format!(
        r#"{{"sub":"owner-user-id","account_id":"{CORE_ACCOUNT_LITERAL}","dataspace_id":"{}","roles":["RETAIL_USER"],"iat":1794171148,"nbf":1794171148,"exp":1794174748,"iss":"{}","aud":"{}"}}"#,
        bank.alias_scope, bank.session_issuer, bank.session_audience
    );
    let token = boi_token_from_bytes(claims.as_bytes(), &session_key(index as u8));
    for (android, root) in [(true, b"".as_slice()), (false, SESSION_ROOT)] {
        let session = selected
            .enrollment_session(android, [&token, b"", root])
            .unwrap();
        assert_eq!(session.config.actor, b"owner-user-id");
        assert_eq!(session.config.fi, bank.alias_scope.as_bytes());
        assert_eq!(session.config.session_valid_from_ms, 1_794_171_148_000);
        assert_eq!(session.config.session_expires_at_ms, 1_794_174_748_000);
    }
}

#[test]
fn boi_session_requires_the_bound_account_and_no_other_claim() {
    let selected = BoiFixture::new().load().unwrap();
    let index = boi_registry::BOI_PARTICIPANTS
        .iter()
        .position(|bank| bank.kagemusha_issuer)
        .unwrap();
    let key = session_key(index as u8);
    let good = boi_token(&boi_claims(index), &key);
    assert!(selected.enrollment_session(true, [&good, b"", b""]).is_ok());
    let mut missing = boi_claims(index);
    object_mut(&mut missing).remove("account_id");
    let mut extra = boi_claims(index);
    object_mut(&mut extra).insert("device_id".into(), Value::String("unknown".into()));
    let mut cases = vec![missing, extra];
    for replacement in [
        Value::String(String::new()),
        Value::String(" padded ".into()),
        Value::String("a".repeat(513)),
        Value::Null,
        Value::Bool(true),
    ] {
        let mut claims = boi_claims(index);
        mutate(&mut claims, "account_id", replacement);
        cases.push(claims);
    }
    for (case, claims) in cases.iter().enumerate() {
        let token = boi_token(claims, &key);
        assert!(
            selected.enrollment_session(true, [&token, b"", b""]).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn boi_shared_runtime_refuses_old_session_shape_and_extra_fields() {
    for mode in 0..2 {
        let mut fixture = BoiFixture::new();
        if mode == 0 {
            object_mut(&mut fixture.0.runtime).remove("enrollment_sessions");
        } else {
            mutate(&mut fixture.0.runtime, "enrollment_session", Value::Null);
        }
        assert!(fixture.load().is_err());
    }
}

#[test]
fn boi_participant_accounts_reject_valid_foreign_discriminant_and_aliasing() {
    use iroha_data_model::account::address::AccountAddress;
    let fixture = BoiFixture::new();
    let (bytes, _, runtime) = fixture.originals();
    for index in 0..boi_registry::BOI_PARTICIPANTS.len() {
        for name in [
            "settlementAccountId",
            "runtimeAccountId",
            "enrollmentAccountId",
        ] {
            let mut app: Value = norito::json::from_slice(&bytes).unwrap();
            let Value::Array(rows) = object_mut(&mut app).get_mut("participants").unwrap() else {
                panic!("participants")
            };
            let account = AccountAddress::parse_encoded(
                text(object(&rows[index]).unwrap(), name).unwrap(),
                Some(369),
            )
            .unwrap();
            let foreign = account.to_i105_for_discriminant(753).unwrap();
            assert_eq!(
                AccountAddress::parse_encoded(&foreign, Some(753))
                    .unwrap()
                    .to_account_id()
                    .unwrap(),
                account.to_account_id().unwrap()
            );
            mutate(&mut rows[index], name, Value::String(foreign));
            let changed = canonical(&app);
            assert!(
                fixture
                    .load_originals(&changed, &boi_sign(&fixture.0.key, &changed), &runtime)
                    .is_err(),
                "bank {index} {name}"
            );
        }
    }
    let mut app: Value = norito::json::from_slice(&bytes).unwrap();
    let Value::Array(rows) = object_mut(&mut app).get_mut("participants").unwrap() else {
        panic!("participants")
    };
    let duplicate = object(&rows[0]).unwrap()["runtimeAccountId"].clone();
    mutate(&mut rows[1], "runtimeAccountId", duplicate);
    let changed = canonical(&app);
    assert!(
        fixture
            .load_originals(&changed, &boi_sign(&fixture.0.key, &changed), &runtime)
            .is_err()
    );
}

#[test]
fn boi_native_platform_identity_refuses_validly_rebound_foreign_app_policy() {
    use iroha_data_model::kagemusha::KagemushaWalletAppIdentityV1;
    for name in ["android", "apple"] {
        let mut fixture = BoiFixture::new();
        let value = object_mut(&mut fixture.0.runtime).get_mut(name).unwrap();
        let row = object(value).unwrap();
        let mut app = KagemushaWalletAppPolicyV1::decode_canonical(
            &raw(row, "app_policy_original_base64", 1024).unwrap(),
            &fixture.0.scheme.scheme_id(),
        )
        .unwrap();
        let mut policy = KagemushaWalletEnrollmentPolicyV1::decode_canonical(
            &raw(row, "enrollment_policy_original_base64", 1024).unwrap(),
            &fixture.0.scheme.scheme_id(),
        )
        .unwrap();
        match &mut app.identity {
            KagemushaWalletAppIdentityV1::Android { package_name, .. } => {
                *package_name = "org.example.foreign.wallet".into()
            }
            KagemushaWalletAppIdentityV1::Apple { app_id } => {
                *app_id = "YLWWUD25VZ.org.example.foreign.wallet".into()
            }
        }
        policy.app_policy = app.policy_digest().unwrap();
        policy.validate_for_app(&app).unwrap();
        mutate(
            value,
            "app_policy_hex",
            Value::String(hex::encode(app.policy_digest().unwrap())),
        );
        mutate(
            value,
            "app_policy_original_base64",
            Value::String(STANDARD.encode(norito::encode_canonical(&app).unwrap())),
        );
        mutate(
            value,
            "enrollment_policy_hex",
            Value::String(hex::encode(policy.policy_digest().unwrap())),
        );
        mutate(
            value,
            "enrollment_policy_original_base64",
            Value::String(STANDARD.encode(norito::encode_canonical(&policy).unwrap())),
        );
        // originals() repins runtime bytes and signs the app afresh. Refusal is
        // due to the BOI identity, not a broken signature or stale policy digest.
        assert!(fixture.load().is_err(), "{name}");
    }
}

/// Candidate check (Claude 63815590, 2026-11-06): the BOI authority admits a real composed candidate
/// runtime and the native admits its carrier through the compiled builtin producer profile, exactly as
/// a phone installation does before enrollment. It needs a candidate directory, so it is ignored:
///   BOI_OFFLINE_CANDIDATE=<runtime dir> BOI_OFFLINE_TRUST_HEX=<ed25519 hex> \
///     cargo test -p connect_norito_bridge --lib boi_candidate -- --ignored
#[test]
#[ignore = "needs BOI_OFFLINE_CANDIDATE and BOI_OFFLINE_TRUST_HEX"]
fn boi_candidate_runtime_installs_through_the_compiled_profile() {
    let dir =
        std::path::PathBuf::from(std::env::var("BOI_OFFLINE_CANDIDATE").expect("candidate dir"));
    let key = hex::decode(std::env::var("BOI_OFFLINE_TRUST_HEX").expect("trust hex")).unwrap();
    let read = |name: &str| std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let app = read("runtime-manifest.json");
    let envelope = read("runtime-manifest.sig.json");
    let runtime = read("wallet-core-runtime.json");
    let pack = read("verifier-pack.norito");
    let inventory = read("producer-inventory.norito");
    let genesis = read("certified-genesis.norito");
    let root = dir
        .join("wallet-originals")
        .to_str()
        .unwrap()
        .as_bytes()
        .to_vec();
    let trust = RuntimeTrust {
        authority: RuntimeAuthority::BoiTairaV1,
        key: PublicKey::from_bytes(Algorithm::Ed25519, &key).unwrap(),
    };
    let input = RuntimeOriginals {
        app_manifest: &app,
        envelope: &envelope,
        wallet_runtime: &runtime,
        verifier_pack: &pack,
        producer_inventory: &inventory,
        signed_genesis: &genesis,
        originals_root: &root,
        registration_source: b"",
    };
    let selected = std::sync::Arc::new(
        Selection::load(&trust, &input).expect("the BOI authority admits the candidate runtime"),
    );
    assert!(matches!(
        selected.application,
        ApplicationBinding::Boi { .. }
    ));
    assert_eq!(
        selected.boi_sessions.len(),
        boi_registry::BOI_TAIRA_ISSUING_BANKS.len()
    );
    let opening =
        iroha_core_zk::kagemusha_wallet_artifacts_v1::producer_inventory::OpeningMetricsV1::start();
    let prepared =
        super::super::super::PreparedInstallation::from_selected(input, selected, opening);
    assert!(prepared.is_ok(), "the native refuses the candidate carrier");
}
