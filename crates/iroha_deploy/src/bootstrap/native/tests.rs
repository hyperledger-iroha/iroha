//! Real native certificate checks over deterministic, credential-free bootstrap transport.

use super::*;
use iroha::http::{HttpTransport, Response, StatusCode, TransportFuture, TransportRequest};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
struct Transport {
    responses: BTreeMap<String, Vec<u8>>,
    requests: Mutex<Vec<TransportRequest>>,
}

impl HttpTransport for Transport {
    fn send_blocking(
        &self,
        request: TransportRequest,
    ) -> color_eyre::eyre::Result<Response<Vec<u8>>> {
        let bytes = self.responses.get(request.url.path()).cloned();
        let is_proof = request.url.path().contains("/bridge/finality/");
        self.requests.lock().unwrap().push(request);
        let mut response = Response::new(bytes.clone().unwrap_or_default());
        *response.status_mut() = if bytes.is_some() {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        };
        response.headers_mut().insert(
            "content-type",
            if is_proof {
                "application/x-norito"
            } else {
                "application/json"
            }
            .parse()
            .unwrap(),
        );
        Ok(response)
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

fn profile(fixture: &NativeFinalityFixture) -> InstalledNetworkProfile {
    InstalledNetworkProfile::new(
        "fixture".into(),
        "https://parent.example/native/",
        fixture.chain_id().into(),
        369,
    )
    .unwrap()
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn transport(tip: u64, proofs: &[SumeragiFinalityProof]) -> (PublicHttpClient, Arc<Transport>) {
    let mut responses: BTreeMap<_, _> = proofs
        .iter()
        .map(|proof| {
            (
                format!("/native/v1/bridge/finality/{}", proof.height()),
                norito::encode_canonical(proof).unwrap(),
            )
        })
        .collect();
    responses.insert("/native/status/blocks".into(), tip.to_string().into_bytes());
    let transport = Arc::new(Transport {
        responses,
        requests: Mutex::new(Vec::new()),
    });
    (
        PublicHttpClient::with_transport(transport.clone()),
        transport,
    )
}

fn capabilities(fixture: &NativeFinalityFixture) -> AccountCapabilitiesV1 {
    AccountCapabilitiesV1 {
        schema_version: 1,
        network_id: fixture.network_id(),
        network_prefix: 369,
        allowed_signing: vec!["ed25519".into()],
    }
}

fn faucet(fixture: &NativeFinalityFixture) -> AccountFaucetAdvertisement {
    AccountFaucetAdvertisement {
        schema_version: 1,
        network_id: fixture.network_id(),
        network_prefix: 369,
        authority: iroha_data_model::account::AccountId::new(
            KeyPair::from_seed(vec![45; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        ),
        asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
        amount: "25000".parse().unwrap(),
    }
}

#[test]
fn native_bootstrap_skips_ordinary_history_and_preserves_exact_transport_and_policy() {
    let mut fixture = NativeFinalityFixture::start_with_explicit_parameters("native-bootstrap");
    let mut proofs = vec![fixture.genesis_proof().clone()];
    for _ in 0..7 {
        let block = fixture.block_with_submitted_work(fixture.next_header());
        proofs.push(fixture.certify(block));
    }
    // H2–H5 are intentionally unavailable. A full-height replay cannot pass this test.
    let selected = [
        proofs[0].clone(),
        proofs[5].clone(),
        proofs[6].clone(),
        proofs[7].clone(),
    ];
    let (client, raw) = transport(8, &selected);
    let profile = profile(&fixture);
    let expected_faucet = faucet(&fixture);
    let result = discover_with(
        &profile,
        deadline(),
        &client,
        |_| Ok(capabilities(&fixture)),
        |network, _| {
            assert_eq!(network, fixture.network_id());
            Ok(expected_faucet.clone())
        },
    )
    .unwrap();
    assert_eq!(result.network_id, fixture.network_id());
    assert_eq!(result.verifier.checkpoint().height(), 8);
    assert_eq!(result.verifier.verified_tip().unwrap().height(), 8);
    assert_eq!(result.capabilities, capabilities(&fixture));
    assert_eq!(result.faucet, expected_faucet);
    let requests = raw.requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.url.path())
            .collect::<Vec<_>>(),
        [
            "/native/v1/bridge/finality/1",
            "/native/status/blocks",
            "/native/v1/bridge/finality/6",
            "/native/v1/bridge/finality/7",
            "/native/v1/bridge/finality/8",
        ]
    );
    for request in requests.iter() {
        assert_eq!(request.url.host_str(), Some("parent.example"));
        assert_eq!(request.method, iroha::http::Method::GET);
        assert!(request.body.is_empty());
        assert!(
            request
                .headers
                .iter()
                .all(|(name, _)| name.as_str() == "accept")
        );
        assert!(!request.direct_loopback);
        assert!(
            request
                .timeout
                .is_some_and(|timeout| timeout <= Duration::from_secs(30))
        );
        assert!(request.max_response_bytes <= MAX_PROOF_RESPONSE_BYTES);
    }
}

#[test]
fn native_bootstrap_authenticates_epoch_boundary_before_current_proofs() {
    let (fixture, proofs) =
        NativeFinalityFixture::short_npos_boundary_chain_with_explicit_parameters(4);
    let (client, raw) = transport(4, &proofs);
    let verified = read_parent_checkpoint(
        &profile(&fixture),
        fixture.network_id(),
        deadline(),
        &client,
    )
    .unwrap();
    assert_eq!(verified.checkpoint().height(), 4);
    assert_eq!(
        verified.checkpoint().block_hash(),
        fixture.latest().block_header.hash()
    );
    let paths: Vec<_> = raw
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|request| request.url.path().to_owned())
        .collect();
    assert_eq!(paths[2], "/native/v1/bridge/finality/3");
    assert_eq!(paths.iter().filter(|path| path.ends_with("/3")).count(), 1);
    let mut forged = proofs;
    forged[2].block_wire[0] ^= 1;
    let (client, _) = transport(4, &forged);
    assert!(
        read_parent_checkpoint(
            &profile(&fixture),
            fixture.network_id(),
            deadline(),
            &client
        )
        .is_err()
    );
}

#[test]
fn native_bootstrap_rejects_foreign_network_chain_genesis_only_and_missing_proof() {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let proofs = [fixture.genesis_proof().clone(), fixture.latest().clone()];
    let (client, _) = transport(2, &proofs);
    let foreign = NativeFinalityFixture::start("foreign-native-bootstrap");
    assert!(
        read_parent_checkpoint(
            &profile(&fixture),
            foreign.network_id(),
            deadline(),
            &client
        )
        .is_err()
    );
    let wrong_chain = InstalledNetworkProfile::new(
        "fixture".into(),
        "https://parent.example/native/",
        "foreign-chain".into(),
        369,
    )
    .unwrap();
    assert!(
        read_parent_checkpoint(&wrong_chain, fixture.network_id(), deadline(), &client).is_err()
    );
    let (client, _) = transport(1, &proofs);
    assert!(
        read_parent_checkpoint(
            &profile(&fixture),
            fixture.network_id(),
            deadline(),
            &client
        )
        .is_err()
    );
    let (client, _) = transport(2, &proofs[..1]);
    assert!(
        read_parent_checkpoint(
            &profile(&fixture),
            fixture.network_id(),
            deadline(),
            &client
        )
        .is_err()
    );
}

#[test]
fn native_bootstrap_refuses_expired_deadline_and_proof_budget_before_network_io() {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let profile = profile(&fixture);
    let (client, raw) = transport(2, &[]);
    assert!(discover_native_bootstrap(&profile, Instant::now()).is_err());
    assert!(
        read_parent_checkpoint(&profile, fixture.network_id(), Instant::now(), &client).is_err()
    );
    for mut budget in [
        ProofBudget {
            proofs: 0,
            bytes: 1,
        },
        ProofBudget {
            proofs: 1,
            bytes: 0,
        },
    ] {
        assert!(read_proof(&profile, &client, 1, deadline(), &mut budget).is_err());
    }
    assert!(raw.requests.lock().unwrap().is_empty());
}

#[test]
fn native_bootstrap_rejects_capability_and_faucet_identity_changes() {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let (client, raw) = transport(
        2,
        &[fixture.genesis_proof().clone(), fixture.latest().clone()],
    );
    let profile = profile(&fixture);
    let mut policy = capabilities(&fixture);
    policy.network_prefix = 753;
    assert!(
        discover_with(
            &profile,
            deadline(),
            &client,
            |_| Ok(policy),
            |_, _| panic!("wrong capabilities must stop discovery")
        )
        .is_err()
    );
    assert!(raw.requests.lock().unwrap().is_empty());
    let mut policy = faucet(&fixture);
    policy.network_prefix = 753;
    assert!(
        discover_with(
            &profile,
            deadline(),
            &client,
            |_| Ok(capabilities(&fixture)),
            |_, _| Ok(policy)
        )
        .is_err()
    );
}

#[test]
fn native_bootstrap_refuses_unsupported_owner_before_proofs_or_funding() {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let (client, raw) = transport(2, &[]);
    let mut policy = capabilities(&fixture);
    policy.allowed_signing = vec!["secp256k1".into()];
    assert!(
        discover_with(
            &profile(&fixture),
            deadline(),
            &client,
            |_| Ok(policy),
            |_, _| panic!("unsupported owner must stop before faucet discovery"),
        )
        .is_err()
    );
    assert!(raw.requests.lock().unwrap().is_empty());
}

/// Opt-in public discovery through the installed profile; no wallet or managed store is opened.
#[test]
#[ignore = "requires IROHA_TEST_RUNTIME_DIRECTORY and explicit read-only public Taira network access"]
fn installed_taira_native_bootstrap_read_only() {
    let runtime_directory = std::path::PathBuf::from(
        std::env::var_os("IROHA_TEST_RUNTIME_DIRECTORY")
            .expect("set IROHA_TEST_RUNTIME_DIRECTORY to the matching installed binaries"),
    );
    let runtime = crate::managed::InstalledRuntime::from_directory(&runtime_directory)
        .expect("authenticate the selected native installation");
    let profiles = runtime
        .network_profiles()
        .expect("load the independently installed network profiles");
    let profile = profiles.select("taira").expect("installed Taira profile");
    assert_eq!(profile.torii_root().as_str(), "https://taira.sora.org/");
    assert_eq!(profile.account_chain_discriminant(), 369);

    let deadline = Instant::now() + Duration::from_secs(60);
    let bootstrap = discover_native_bootstrap(profile, deadline)
        .expect("read-only native Taira discovery and certified parent verification");
    let checkpoint = bootstrap.verifier.checkpoint();
    let verified_tip = bootstrap
        .verifier
        .verified_tip()
        .expect("retain the authenticated native checkpoint tip");
    assert_eq!(checkpoint.network_id(), bootstrap.network_id);
    assert_eq!(checkpoint.chain_id(), profile.chain_id());
    assert!(checkpoint.height() >= 2);
    assert_eq!(verified_tip.height(), checkpoint.height());
    assert_eq!(bootstrap.capabilities.network_id, bootstrap.network_id);
    assert_eq!(
        bootstrap.capabilities.network_prefix,
        profile.account_chain_discriminant()
    );
    assert_eq!(bootstrap.faucet.network_id, bootstrap.network_id);
    assert_eq!(
        bootstrap.faucet.network_prefix,
        profile.account_chain_discriminant()
    );
    assert!(Instant::now() < deadline);
    eprintln!(
        "read-only Taira bootstrap: network={}, checkpoint_height={}, checkpoint_hash={}",
        bootstrap.network_id,
        checkpoint.height(),
        checkpoint.block_hash()
    );
}
