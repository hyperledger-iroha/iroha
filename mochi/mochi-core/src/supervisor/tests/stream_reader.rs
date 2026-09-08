//! Generation-owned stream authority tests.
use super::*;
#[test]
fn stream_reader_binds_owned_genesis_instead_of_vault_or_operator() {
    if !ports_available("stream_reader_binds_owned_genesis") {
        return;
    }
    let _env = env_lock().lock().expect("environment lock");
    let temp = tempfile::tempdir().expect("temporary sandbox");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("validated supervisor");
    let reader = supervisor.stream_reader("peer0").expect("stream account");
    assert_eq!(
        reader.authority(),
        &AccountId::new(supervisor.genesis.public_key().clone())
    );
    assert_eq!(*reader.network_id(), supervisor.network_id().unwrap());
    assert_eq!(
        reader.endpoint().as_str().trim_end_matches('/'),
        supervisor.peers[0].spec.torii_base_http()
    );
    assert_ne!(reader.authority(), supervisor.signers[0].account_id());
    let second_peer = supervisor
        .stream_reader("peer1")
        .expect("second peer account");
    assert_eq!(second_peer.authority(), reader.authority());
    assert_eq!(second_peer.network_id(), reader.network_id());
    assert_ne!(second_peer.endpoint(), reader.endpoint());
}

#[test]
fn stream_reader_rejects_stale_generation_and_unknown_peer() {
    if !ports_available("stream_reader_rejects_stale_generation") {
        return;
    }
    let _env = env_lock().lock().expect("environment lock");
    let temp = tempfile::tempdir().expect("temporary sandbox");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("validated supervisor");
    assert!(matches!(
        supervisor.stream_reader("missing"),
        Err(SupervisorError::PeerUnknown { .. })
    ));
    supervisor.genesis.generation_id.push_str("-stale");
    assert!(matches!(
        supervisor.stream_reader("peer0"),
        Err(SupervisorError::GenerationValidation(_))
    ));
}
