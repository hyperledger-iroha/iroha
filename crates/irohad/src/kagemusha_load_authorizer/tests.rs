//! Native startup, bounded outage and supervised shutdown tests. These do not assert handset
//! proof qualification; the signer remains unable to prepare from an unfinalized World.
use super::*;
use iroha_core::kagemusha_wallet_v1::{LoadAuthorizerKeyV1, LoadAuthorizerKeyringV1};
use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
use iroha_data_model::kagemusha::*;
use iroha_futures::supervisor::Supervisor;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

fn fixture<T>(name: &str) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let values: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
    ))
    .unwrap();
    let row = values["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["type"].as_str() == Some(name))
        .unwrap();
    let bytes = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
    norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
        .unwrap()
}
fn handles() -> (Arc<State>, Arc<Queue>) {
    let scheme: KagemushaWalletSchemeV1 = fixture("KagemushaWalletSchemeV1");
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            scheme.network_id,
        )),
    );
    let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "kagemusha-load-authorizer-test".parse().unwrap(),
        network,
    ));
    let (events, _) = tokio::sync::broadcast::channel(1);
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    ));
    (state, queue)
}
fn config(state: &State) -> KagemushaLoadAuthorizer {
    let mut scheme: KagemushaWalletSchemeV1 = fixture("KagemushaWalletSchemeV1");
    scheme.network_id = *state.network_id_ref().as_bytes();
    let root = SigningKey::from_slice(&[0x11; 32]).unwrap();
    let key = SigningKey::from_slice(&[0x34; 32]).unwrap();
    let body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::LoadAuthorization,
        key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap(),
        serial: 19,
    };
    let signature: Signature = root.sign(&body.signing_message());
    let certificate = KagemushaWalletSignerCertificateV1::sign(
        body,
        &scheme,
        KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
    )
    .unwrap();
    let keyring = LoadAuthorizerKeyringV1 {
        version: 1,
        keys: vec![LoadAuthorizerKeyV1 {
            scheme,
            certificate,
            secret: [0x34; 32],
        }],
    };
    KagemushaLoadAuthorizer::new(
        iroha_config::parameters::actual::KagemushaLoadAuthorizerCustody {
            keyring: zeroize::Zeroizing::new(norito::encode_canonical(&keyring).unwrap()),
            submitter: KeyPair::from_seed(vec![0x65; 32], iroha_crypto::Algorithm::Ed25519),
        },
    )
}
#[tokio::test]
async fn required_service_refuses_invalid_custody_and_limits_without_queueing() {
    let (state, queue) = handles();
    let mut malformed = config(&state);
    malformed.custody.keyring.fill(0);
    assert!(Service::new(malformed, state.clone(), queue.clone()).is_err());
    let mut capacity = config(&state);
    capacity.page_size = 0;
    assert!(Service::new(capacity, state.clone(), queue.clone()).is_err());
    assert_eq!(queue.queued_len(), 0);
}
#[tokio::test]
async fn unavailable_source_survives_restart_without_queueing_and_shutdown_joins() {
    let (state, queue) = handles();
    for _ in 0..2 {
        let mut worker = Service::new(config(&state), state.clone(), queue.clone()).unwrap();
        assert_eq!(worker.tick(), Err(TickError::SourceUnavailable));
        assert_eq!(queue.queued_len(), 0);
    }
    let worker = Service::new(config(&state), state, queue).unwrap();
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    supervisor.monitor(worker.start(shutdown.clone()));
    shutdown.send();
    assert!(supervisor.start().await.is_ok());
}
