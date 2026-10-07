//! Real signatures and filesystem ordering around explicit test runtime DATA.
//! These tests do not establish provider authority, private process custody or device attestation.

use super::*;
use crate::{
    kagemusha_wallet_v1::{Registration, enrollment_journal::permit_tests, storage},
    state::World,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_core_zk::kagemusha_wallet_enrollment_v1::issuer_worker::VerifierRuntimeSelectionV1;
use iroha_core_zk::kagemusha_wallet_enrollment_v1::{IssuerEvidenceV1, PlatformEvidenceV1};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature as AccountSignature};
use iroha_data_model::{
    Registrable as _,
    account::Account,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition},
    nexus::AxtAssetIncarnationV1,
};
use iroha_fs::PrivateDirectory;
use iroha_primitives::numeric::NumericSpec;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

const ROOT_DATA: &[u8] = b"unadmitted worker root DATA";

struct Runtime {
    config: Arc<KagemushaEnrollmentIssuer>,
    account: AccountId,
    bank_key: KeyPair,
    signer: SigningKey,
    clock: u64,
    observation: KagemushaEligibilityDecisionV1,
    authenticated: bool,
    fail_worker: bool,
    bank_account: [u8; 32],
    bank_actor: [u8; 32],
    config_change_after_bank: Option<usize>,
    clock_jump_after_bank: Option<usize>,
    config_countdown: Option<usize>,
    clock_countdown: Option<usize>,
    worker_actions: Vec<String>,
    signatures: usize,
    bank_calls: usize,
    fixture_evidence: Option<RequestV1>,
}
impl EnrollmentIssuerRuntimeV1 for Runtime {
    type Call = [u8];
    fn current_configuration(&mut self) -> Result<Arc<KagemushaEnrollmentIssuer>> {
        if let Some(left) = self.config_countdown.as_mut() {
            if *left == 0 {
                let mut changed = self.config.as_ref().clone();
                changed.revision += 1;
                changed.providers.clear();
                self.config = Arc::new(changed);
                self.config_countdown = None;
            } else {
                *left -= 1;
            }
        }
        Ok(self.config.clone())
    }
    fn require_dependencies(&mut self, _: &KagemushaEnrollmentIssuer) -> Result<()> {
        Ok(())
    }
    fn authenticate_call(&mut self, call: &[u8], expected_body: &[u8]) -> Result<AccountId> {
        // An exact-body test sentinel stands only for this unit runtime's authentication.
        if !self.authenticated || call != expected_body {
            return Err(Invalid);
        }
        Ok(self.account.clone())
    }
    fn now_ms(&mut self) -> Result<u64> {
        if let Some(left) = self.clock_countdown.as_mut() {
            if *left == 0 {
                self.clock += 100_000;
                self.clock_countdown = None;
            } else {
                *left -= 1;
            }
        }
        Ok(self.clock)
    }
    fn observe_eligibility(
        &mut self,
        provider: &KagemushaEnrollmentProvider,
        asset: &KagemushaWalletAssetScopeV1,
        original: &[u8],
        _: std::time::Duration,
    ) -> Result<Vec<u8>> {
        let policy = provider.eligibility.for_asset(asset).map_err(|_| Invalid)?;
        self.bank_calls += 1;
        let request = KagemushaEligibilityRequestV1::decode_canonical(original, &policy)
            .map_err(|_| Invalid)?;
        let body = KagemushaEligibilityResponseBodyV1 {
            version: 1,
            request_digest: request.request_digest(&policy).map_err(|_| Invalid)?,
            decision: if request.account_digest != self.bank_account
                || request.actor_digest != self.bank_actor
            {
                KagemushaEligibilityDecisionV1::NotApproved
            } else {
                self.observation
            },
            source_revision: 1,
            observed_at_ms: self.clock,
            valid_until_ms: request.expires_at_ms,
        };
        let response = KagemushaEligibilityResponseV1 {
            signature: AccountSignature::new(
                self.bank_key.private_key(),
                &body.signing_message().map_err(|_| Invalid)?,
            )
            .payload()
            .try_into()
            .map_err(|_| Invalid)?,
            body,
        };
        self.config_countdown = self.config_change_after_bank;
        self.clock_countdown = self.clock_jump_after_bank;
        response.encode_canonical().map_err(|_| Invalid)
    }
    fn worker_configuration(
        &mut self,
        provider: &KagemushaEnrollmentProvider,
        asset: &KagemushaWalletAssetScopeV1,
    ) -> Result<VerifierConfigurationV1> {
        if self.fail_worker {
            return Err(Unavailable);
        }
        VerifierConfigurationV1::from_selected(
            &provider.app,
            &provider.enrollment.for_asset(asset).map_err(|_| Invalid)?,
            ROOT_DATA,
            None,
            VerifierRuntimeSelectionV1 {
                openssl_path: "/opt/issuer/openssl",
                openssl_sha256: [44; 32],
                store_directory: "/var/lib/issuer/worker",
            },
        )
        .map_err(|_| Invalid)
    }
    fn worker_exchange(
        &mut self,
        _: &KagemushaEnrollmentProvider,
        configuration: &VerifierConfigurationV1,
        exchange: &VerifierExchangeV1,
    ) -> Result<Vec<u8>> {
        let packet: norito::json::Value = norito::json::from_slice(&exchange.frame()[4..]).unwrap();
        let action = packet["action"].as_str().unwrap();
        self.worker_actions.push(action.into());
        let mut outcome = match action {
            "journal" => "journal",
            "prepare" => "prepared",
            _ => "unavailable",
        };
        let evidence = if matches!(action, "complete" | "recover" | "inspect") {
            self.fixture_evidence.as_ref().map(|request| {
                outcome = "evidence";
                let body = &request.body;
                let PlatformEvidenceV1::Apple { key_id, attestation, key_binding_assertion } =
                    PlatformEvidenceV1::decode(&body.evidence, &body.policy).unwrap()
                else { panic!("Apple unit fixture") };
                let kind = KagemushaWalletEvidenceKindV1::AppleAppAttest;
                let originals = IssuerEvidenceV1::Apple {
                    attestation: attestation.clone(), key_binding_assertion: key_binding_assertion.clone(),
                };
                let projection = norito::json!({
                    "config_sha256": (hex::encode(configuration.digest())),
                    "challenge_digest": (hex::encode(body.challenge.challenge_digest())),
                    "key_binding": (hex::encode(kagemusha_wallet_enrollment_key_binding_v1(&body.challenge.challenge_digest(), &body.marker.payment_key))),
                    "payment_key_base64": (STANDARD.encode(body.marker.payment_key.as_sec1_bytes())),
                    "kind_tag": (kind.tag()), "time_ms": (2_000_u64),
                    "facts": (kind.required_enrollment_facts() | KAGEMUSHA_WALLET_FACT_APP_ATTEST_PRODUCTION_V1),
                    "os_patch_level": (0_u32), "vendor_patch_level": (0_u32), "boot_patch_level": (0_u32),
                    "evidence_digest": (hex::encode(originals.digest(body, kind).unwrap())),
                    "original_items_base64": (vec![STANDARD.encode(&attestation), STANDARD.encode(&key_binding_assertion)]),
                    "app_attest_key_id": (hex::encode(key_id)), "app_attest_counter": (1_u32),
                });
                norito::json::Value::String(STANDARD.encode(norito::json::to_vec(&projection).unwrap()))
            }).unwrap_or(norito::json::Value::Null)
        } else {
            norito::json::Value::Null
        };
        let incarnation = if action == "journal" {
            norito::json::Value::String(hex::encode([41; 32]))
        } else {
            packet["journal_incarnation"].clone()
        };
        let bytes = norito::json::to_vec(&norito::json!({
            "schema": ("iroha.kagemusha.wallet-e1-verifier.v1"), "version": (1_u16),
            "exchange_id": (packet["exchange_id"].clone()),
            "request_sha256": (hex::encode(Sha256::digest(&exchange.frame()[4..]))),
            "journal_incarnation": (incarnation), "config_sha256": (hex::encode(configuration.digest())),
            "outcome": (outcome), "evidence_base64": (evidence),
        })).unwrap();
        let mut frame = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
        frame.extend(bytes);
        Ok(frame)
    }
    fn sign_enrollment(
        &mut self,
        _: &KagemushaEnrollmentProvider,
        message: &[u8; 32],
    ) -> Result<Vec<u8>> {
        self.signatures += 1;
        let signature: Signature = self.signer.sign(message);
        Ok(signature.to_der().as_bytes().to_vec())
    }
}

fn fixture() -> (
    tempfile::TempDir,
    EnrollmentIssuerV1<Runtime>,
    PreKeyDispatchV1,
) {
    let temporary = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/qualification/enrollment-issuer-tests");
    std::fs::create_dir_all(&temporary).unwrap();
    let temp = tempfile::tempdir_in(temporary).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let parent = PrivateDirectory::open(temp.path()).unwrap();
    let scope = [71; 32];
    let mut journal = EnrollmentJournalV1::initialize(
        &parent,
        "issuer",
        &enrollment_issuer_journal_scope_v1(scope).unwrap(),
    )
    .unwrap();
    let (mut dispatch, _, signer) = permit_tests::unprepared_fixture(&mut journal);
    // A different new request binds this unit's worker configuration DATA.
    dispatch.request_id = [75; 32];
    dispatch.policy.platform = KagemushaWalletEnrollmentPlatformV1::Apple {
        attestation_root_sha256: Sha256::digest(ROOT_DATA).into(),
    };
    drop(journal);
    let bank_key = KeyPair::from_seed(vec![32; 32], Algorithm::Ed25519);
    let eligibility = KagemushaEligibilityPolicyTemplateV1 {
        version: 1,
        network_id: dispatch.scheme.network_id,
        scheme_id: dispatch.scheme.scheme_id(),
        revision: 1,

        authority: KagemushaEligibilityAuthorityV1::Bank {
            fi_digest: dispatch.fi_digest,
        },
        public_key: bank_key.public_key().to_bytes().1.try_into().unwrap(),
        maximum_response_ms: 5000,
    };
    let config = Arc::new(KagemushaEnrollmentIssuer {
        revision: 1,
        scope,
        journal_dir: temp.path().join("issuer"),
        request_timeout: std::time::Duration::from_secs(5),
        max_inflight: 16,
        providers: vec![KagemushaEnrollmentProvider {
            eligibility,
            scheme: dispatch.scheme,
            app: dispatch.app.clone(),
            enrollment: KagemushaWalletEnrollmentPolicyTemplateV1 {
                version: dispatch.policy.version,
                scheme_id: dispatch.policy.scheme_id,
                app_policy: dispatch.policy.app_policy,
                platform: dispatch.policy.platform,
                regulatory_policy: dispatch.policy.regulatory_policy,
                challenge_lifetime_ms: dispatch.policy.challenge_lifetime_ms,
                attestation_lease_lifetime_ms: dispatch.policy.attestation_lease_lifetime_ms,
            },
            certificate: dispatch.enrollment_certificate,
            manifest_digest: dispatch.manifest_digest,
            release_digest: dispatch.release_digest,
            service_origin_digest: dispatch.service_origin_digest,
            observation_endpoint: "https://provider.example/eligibility".parse().unwrap(),
            observation_credential: "/private/provider-credential".into(),
            worker: iroha_config::parameters::actual::KagemushaEnrollmentWorker {
                python_executable: "/opt/issuer/python".into(),
                python_sha256: [41; 32],
                verifier_archive: "/opt/issuer/verifier.pyz".into(),
                verifier_sha256: [42; 32],
                openssl_executable: "/opt/issuer/openssl".into(),
                openssl_sha256: [43; 32],
                attestation_root: "/opt/issuer/root.pem".into(),
                store_directory: "/private/issuer/worker".into(),
                exchange_timeout: std::time::Duration::from_secs(60),
                google: None,
            },
            signer_private_key: "/private/enrollment-signer".into(),
        }],
    });
    let registration = Registration {
        scheme: dispatch.scheme,
        asset: dispatch.asset.clone(),
        reserve: dispatch.account.clone(),
        balance_scope: AssetBalanceScope::Global,
    };
    let definition = AssetDefinition::new(
        dispatch.asset.asset.clone(),
        "issuer unit DATA",
        NumericSpec::fractional(dispatch.asset.scale),
        AssetBalancePolicy::Global,
        None,
    )
    .build(&dispatch.account);
    let mut world = World::with_assets(
        [],
        [Account::new(dispatch.account.clone()).build(&dispatch.account)],
        [definition],
        [],
        [],
    );
    world.axt_asset_incarnations.insert(
        dispatch.asset.asset.clone(),
        AxtAssetIncarnationV1::try_from_bytes(dispatch.asset.asset_incarnation).unwrap(),
    );
    world.kagemusha_wallet_ledger.insert(
        storage::key(
            storage::REGISTRATION,
            dispatch.scheme.scheme_id(),
            dispatch.asset.asset_digest(),
        ),
        storage::encode(&registration).unwrap(),
    );
    let mut state = State::new_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    state.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::prehashed(dispatch.scheme.network_id)),
    );
    let runtime = Runtime {
        config,
        account: dispatch.account.clone(),
        bank_key,
        signer,
        clock: 2000,
        observation: KagemushaEligibilityDecisionV1::ApprovedUnfrozen,
        authenticated: true,
        fail_worker: false,
        bank_account: kagemusha_wallet_account_digest_v1(&dispatch.account).unwrap(),
        bank_actor: dispatch.actor_digest,
        config_change_after_bank: None,
        clock_jump_after_bank: None,
        config_countdown: None,
        clock_countdown: None,
        worker_actions: vec![],
        signatures: 0,
        bank_calls: 0,
        fixture_evidence: None,
    };
    (
        temp,
        EnrollmentIssuerV1::open(Arc::new(state), runtime).unwrap(),
        dispatch,
    )
}

fn authenticate(
    owner: &mut EnrollmentIssuerV1<Runtime>,
    dispatch: &PreKeyDispatchV1,
) -> EnrollmentIssuerSessionV1 {
    let original = dispatch.encode().unwrap();
    owner.authenticate(&original, &original).unwrap()
}

#[test]
fn scope_and_operation_domains_are_exact_and_entropy_is_nonzero() {
    assert!(enrollment_issuer_journal_scope_v1([0; 32]).is_err());
    assert_ne!(
        enrollment_issuer_journal_scope_v1([1; 32]).unwrap(),
        enrollment_issuer_journal_scope_v1([2; 32]).unwrap()
    );
    assert!(operation_digest(b"x", &[]).is_err());
    assert_ne!(
        operation_digest(b"pre-key", &[1]).unwrap(),
        operation_digest(b"deliver", &[1]).unwrap()
    );
    assert_ne!(random_nonce().unwrap(), [0; 32]);
}

#[test]
fn unsigned_calls_and_unconfigured_bank_cannot_create_sessions() {
    let (_temp, mut owner, mut dispatch) = fixture();
    let original = dispatch.encode().unwrap();
    owner.runtime.authenticated = false;
    assert!(owner.authenticate(&original, &original).is_err());
    owner.runtime.authenticated = true;
    dispatch.fi_digest = [99; 32];
    let foreign = dispatch.encode().unwrap();
    assert!(owner.authenticate(&foreign, &foreign).is_err());
    assert_eq!(owner.runtime.signatures, 0);
    assert_eq!(owner.runtime.bank_calls, 0);
}

#[test]
fn configured_route_without_bank_account_actor_approval_cannot_grant() {
    for wrong_account in [true, false] {
        let (_temp, mut owner, dispatch) = fixture();
        if wrong_account {
            owner.runtime.bank_account = [98; 32];
        } else {
            owner.runtime.bank_actor = [99; 32];
        }
        // Authentication and exact configured routing can select only a DATA attempt.
        let mut session = authenticate(&mut owner, &dispatch);
        assert_eq!(owner.runtime.signatures, 0);
        assert!(owner.pre_key_permit(&mut session).is_err());
        assert_eq!(owner.runtime.bank_calls, 1);
        assert_eq!(owner.runtime.signatures, 0);
        assert_eq!(owner.runtime.worker_actions, vec!["journal", "prepare"]);
        assert!(owner.issue_credential(&mut session).is_err());
        assert!(owner.deliver_credential(&mut session).is_err());
    }
}

#[test]
fn permit_is_prepared_bank_checked_signed_and_retained_exactly_once() {
    let (_temp, mut owner, dispatch) = fixture();
    let mut session = authenticate(&mut owner, &dispatch);
    let original = owner.pre_key_permit(&mut session).unwrap();
    let permit = KagemushaEnrollmentPermitV1::decode_canonical(
        &original,
        &dispatch.scheme,
        &dispatch.enrollment_certificate,
    )
    .unwrap();
    assert_eq!(permit.body.challenge, session.attempt.selection().challenge);
    assert_eq!(owner.runtime.worker_actions, vec!["journal", "prepare"]);
    assert_eq!(owner.runtime.signatures, 1);
    assert_eq!(owner.pre_key_permit(&mut session).unwrap(), original);
    assert_eq!(owner.runtime.signatures, 1);
    assert_eq!(owner.runtime.bank_calls, 2);
    owner.runtime.clock = 1;
    assert!(owner.pre_key_permit(&mut session).is_err());
}

#[test]
fn unavailable_worker_or_denied_bank_cannot_reach_signer() {
    let (_temp, mut owner, dispatch) = fixture();
    let mut session = authenticate(&mut owner, &dispatch);
    owner.runtime.fail_worker = true;
    assert!(owner.pre_key_permit(&mut session).is_err());
    assert_eq!(owner.runtime.bank_calls, 0);
    owner.runtime.fail_worker = false;
    owner.runtime.observation = KagemushaEligibilityDecisionV1::Frozen;
    assert!(owner.pre_key_permit(&mut session).is_err());
    assert_eq!(owner.runtime.signatures, 0);
    assert!(owner.issue_credential(&mut session).is_err());
    assert!(owner.deliver_credential(&mut session).is_err());
}

#[test]
fn routing_change_or_expiry_after_bank_consumption_prevents_signer_invocation() {
    for change in [true, false] {
        let (_temp, mut owner, dispatch) = fixture();
        let mut session = authenticate(&mut owner, &dispatch);
        if change {
            owner.runtime.config_change_after_bank = Some(2);
        } else {
            owner.runtime.clock_jump_after_bank = Some(2);
        }
        assert!(owner.pre_key_permit(&mut session).is_err());
        assert_eq!(owner.runtime.signatures, 0);
    }
}

#[test]
fn e5_is_consumed_before_dispatch_and_unknown_outcome_never_reprepares() {
    let (_temp, mut owner, dispatch) = fixture();
    let mut session = authenticate(&mut owner, &dispatch);
    owner.pre_key_permit(&mut session).unwrap();
    let request = permit_tests::account_request(&dispatch, &session.attempt)
        .encode()
        .unwrap();
    assert!(matches!(
        owner.verify_evidence(&mut session, &request),
        Err(Pending)
    ));
    assert_eq!(session.attempt.phase(), Phase::Verifying);
    assert_eq!(owner.runtime.worker_actions.last().unwrap(), "complete");
    assert!(matches!(
        owner.verify_evidence(&mut session, &request),
        Err(Pending)
    ));
    assert_eq!(owner.runtime.worker_actions.last().unwrap(), "recover");
    assert_eq!(
        owner
            .runtime
            .worker_actions
            .iter()
            .filter(|s| *s == "prepare")
            .count(),
        1
    );
}

#[test]
fn freshness_failure_after_e5_selection_does_not_invoke_complete() {
    let (_temp, mut owner, dispatch) = fixture();
    let mut session = authenticate(&mut owner, &dispatch);
    owner.pre_key_permit(&mut session).unwrap();
    let request = permit_tests::account_request(&dispatch, &session.attempt)
        .encode()
        .unwrap();
    owner.runtime.config_change_after_bank = Some(2);
    assert!(owner.verify_evidence(&mut session, &request).is_err());
    assert!(!owner.runtime.worker_actions.iter().any(|s| s == "complete"));
}

#[test]
fn stale_and_foreign_sessions_do_not_authorize_new_work() {
    let (_temp, mut owner, dispatch) = fixture();
    let mut first = authenticate(&mut owner, &dispatch);
    let mut stale = authenticate(&mut owner, &dispatch);
    owner.pre_key_permit(&mut first).unwrap();
    assert!(owner.pre_key_permit(&mut stale).is_err());
    first.issuer = [0; 32];
    assert!(owner.pre_key_permit(&mut first).is_err());
}

#[test]
fn live_recovery_needs_fresh_bank_approval_but_expired_retry_is_passive_only() {
    let (_temp, mut owner, dispatch) = fixture();
    let mut session = authenticate(&mut owner, &dispatch);
    owner.pre_key_permit(&mut session).unwrap();
    let request = permit_tests::account_request(&dispatch, &session.attempt)
        .encode()
        .unwrap();
    assert!(matches!(
        owner.verify_evidence(&mut session, &request),
        Err(Pending)
    ));
    let count = owner.runtime.worker_actions.len();
    owner.runtime.observation = KagemushaEligibilityDecisionV1::Frozen;
    assert!(owner.verify_evidence(&mut session, &request).is_err());
    assert_eq!(owner.runtime.worker_actions.len(), count);
    owner.runtime.clock = session.attempt.selection().expires_at_ms + 1;
    assert!(matches!(
        owner.verify_evidence(&mut session, &request),
        Err(Pending)
    ));
    assert_eq!(owner.runtime.worker_actions.last().unwrap(), "inspect");
    assert_eq!(session.attempt.phase(), Phase::Verifying);
    assert_eq!(owner.runtime.signatures, 1);
}

#[test]
fn real_state_network_and_current_originals_are_required() {
    let (_temp, mut owner, dispatch) = fixture();
    Arc::get_mut(&mut owner.state).unwrap().network_id =
        iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"other network",
        )));
    let original = dispatch.encode().unwrap();
    assert!(owner.authenticate(&original, &original).is_err());
    assert!(owner.runtime.worker_actions.is_empty());
}

#[test]
fn verified_fixture_issues_and_recovers_exact_e6_after_e1_expiry_with_fresh_current_policy() {
    // Worker projection is explicitly DATA in this unit runtime. Genuine rooted signatures
    // and journal ordering are exercised; no platform attestation or serving claim is made.
    let (_temp, mut owner, dispatch) = fixture();
    let mut session = authenticate(&mut owner, &dispatch);
    owner.pre_key_permit(&mut session).unwrap();
    let request = permit_tests::account_request(&dispatch, &session.attempt);
    owner.runtime.fixture_evidence = Some(request.clone());
    owner
        .verify_evidence(&mut session, &request.encode().unwrap())
        .unwrap();
    assert_eq!(session.attempt.phase(), Phase::Evidence);
    owner.runtime.clock = session.attempt.selection().expires_at_ms + 1;
    owner.issue_credential(&mut session).unwrap();
    let original = owner.deliver_credential(&mut session).unwrap();
    let result = ResultV1::decode(&original).unwrap();
    result
        .verify_for(&dispatch.scheme, &dispatch.enrollment_certificate, &request)
        .unwrap();
    assert_eq!(owner.runtime.signatures, 2);

    let EnrollmentIssuerV1 {
        runtime,
        state,
        journal,
        ..
    } = owner;
    drop(journal);
    let mut owner = EnrollmentIssuerV1::open(state, runtime).unwrap();
    let mut session = authenticate(&mut owner, &dispatch);
    owner.runtime.observation = KagemushaEligibilityDecisionV1::Frozen;
    assert!(owner.deliver_credential(&mut session).is_err());
    owner.runtime.observation = KagemushaEligibilityDecisionV1::ApprovedUnfrozen;
    let mut rotated = owner.runtime.config.as_ref().clone();
    rotated.revision += 1;
    rotated.providers[0].eligibility.revision += 1;
    owner.runtime.config = Arc::new(rotated);
    owner.issue_credential(&mut session).unwrap();
    assert_eq!(owner.deliver_credential(&mut session).unwrap(), original);
    assert_eq!(owner.runtime.signatures, 2);
}

#[test]
fn selected_scheme_operator_can_issue_permit_without_a_bank_or_parliament_gate() {
    let (_temp, mut owner, dispatch) = fixture();
    let mut configured = owner.runtime.config.as_ref().clone();
    configured.providers[0].eligibility.authority =
        KagemushaEligibilityAuthorityV1::SchemeOperator {
            operator_digest: dispatch.fi_digest,
        };
    owner.runtime.config = Arc::new(configured);
    let mut session = authenticate(&mut owner, &dispatch);
    let permit = owner.pre_key_permit(&mut session).unwrap();
    KagemushaEnrollmentPermitV1::decode_canonical(
        &permit,
        &dispatch.scheme,
        &dispatch.enrollment_certificate,
    )
    .unwrap();
    assert_eq!(owner.runtime.signatures, 1);
    assert_eq!(owner.runtime.bank_calls, 1);
}

#[test]
fn one_template_binds_two_registered_assets_and_retains_each_attempt_configuration() {
    let (_temp, mut owner, a) = fixture();
    let mut b = a.clone();
    b.asset.asset = iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([
        0x90, 0x21, 0x31, 0x42, 0x53, 0x64, 0x45, 0x86, 0x97, 0xa8, 0xb9, 0xca, 0xdb, 0xec, 0xfd,
        0x0e,
    ])
    .unwrap();
    b.asset.asset_incarnation = [83; 32];
    b.asset.scale = 28;
    b.request_id = [84; 32];
    b.client_nonce = [85; 32];
    b.native_dispatch_nonce = [86; 32];
    let provider = owner.runtime.config.providers[0].clone();
    b.policy = provider.enrollment.for_asset(&b.asset).unwrap();
    let original = b.encode().unwrap();
    // A valid template is not registration authority.
    assert!(owner.authenticate(&original, &original).is_err());
    {
        let state = Arc::get_mut(&mut owner.state).unwrap();
        state.world.asset_definitions.insert(
            b.asset.asset.clone(),
            AssetDefinition::new(
                b.asset.asset.clone(),
                "second arbitrary issuer test DATA",
                NumericSpec::fractional(b.asset.scale),
                AssetBalancePolicy::Global,
                None,
            )
            .build(&b.account),
        );
        state.world.axt_asset_incarnations.insert(
            b.asset.asset.clone(),
            AxtAssetIncarnationV1::try_from_bytes(b.asset.asset_incarnation).unwrap(),
        );
        state.world.kagemusha_wallet_ledger.insert(
            storage::key(
                storage::REGISTRATION,
                b.scheme.scheme_id(),
                b.asset.asset_digest(),
            ),
            storage::encode(&Registration {
                scheme: b.scheme,
                asset: b.asset.clone(),
                reserve: b.account.clone(),
                balance_scope: AssetBalanceScope::Global,
            })
            .unwrap(),
        );
    }
    assert_eq!(owner.runtime.config.providers.len(), 1);
    let mut first = authenticate(&mut owner, &a);
    let permit_a = owner.pre_key_permit(&mut first).unwrap();
    let config_a = first.attempt.worker_configuration().unwrap();
    let request_a = permit_tests::account_request(&a, &first.attempt)
        .encode()
        .unwrap();
    assert!(matches!(
        owner.verify_evidence(&mut first, &request_a),
        Err(Pending)
    ));
    let mut second = authenticate(&mut owner, &b);
    owner.runtime.fail_worker = true;
    assert!(owner.pre_key_permit(&mut second).is_err());
    assert_eq!(second.attempt.phase(), Phase::Selected);
    assert_eq!(first.attempt.phase(), Phase::Verifying);
    assert_eq!(first.attempt.worker_configuration().unwrap(), config_a);
    owner.runtime.fail_worker = false;
    let permit_b = owner.pre_key_permit(&mut second).unwrap();
    let config_b = second.attempt.worker_configuration().unwrap();
    assert_ne!(config_a, config_b);
    assert_ne!(permit_a, permit_b);
    assert!(owner.verify_evidence(&mut second, &request_a).is_err());
    assert_eq!(second.attempt.phase(), Phase::Selected);
    assert!(matches!(
        owner.verify_evidence(&mut first, &request_a),
        Err(Pending)
    ));
    assert_eq!(owner.runtime.worker_actions.last().unwrap(), "recover");
    assert_eq!(first.attempt.worker_configuration().unwrap(), config_a);
    assert_eq!(second.attempt.worker_configuration().unwrap(), config_b);
    owner.runtime.clock = first.attempt.selection().expires_at_ms + 1;
    assert!(matches!(
        owner.verify_evidence(&mut first, &request_a),
        Err(Pending)
    ));
    assert_eq!(owner.runtime.worker_actions.last().unwrap(), "inspect");
    assert_eq!(
        owner
            .runtime
            .worker_actions
            .iter()
            .filter(|s| *s == "prepare")
            .count(),
        2
    );
    assert!(owner.deliver_credential(&mut second).is_err());
    assert!(owner.deliver_credential(&mut first).is_err());
}
