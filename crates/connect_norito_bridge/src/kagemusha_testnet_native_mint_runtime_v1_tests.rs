//! Trusted testnet native mint pin and pre-submission ordering checks.

use std::{collections::BTreeMap, path::Path, sync::Mutex};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, block::consensus_v2::HeightContextId};

use crate::kagemusha_mobile_bootstrap_v1::{expired_test_bootstrap_v1, verified_test_bootstrap_v1};

use super::{
    KagemushaRecursiveVerifierProfileV1, KagemushaTestnetDurableObservationModeV1,
    KagemushaTestnetNativeMintInstallV1, KagemushaTestnetNativeMintReservationV1,
    KagemushaTestnetNativeMintRuntimeV1, KagemushaTestnetStateObservationScopeV1,
    authenticated_observation_scope, require_matching_verified_chain_root, require_trusted_pins,
};

fn network(byte: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])))
}

fn first_context(byte: u8) -> HeightContextId {
    HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])))
}

fn scope() -> KagemushaTestnetStateObservationScopeV1 {
    KagemushaTestnetStateObservationScopeV1::new(
        [3; 32], [4; 32], [5; 32], 2, [6; 32], [7; 32], [8; 32],
    )
    .expect("distinct native testnet pins")
}

fn unconfigured_profile() -> KagemushaRecursiveVerifierProfileV1 {
    KagemushaRecursiveVerifierProfileV1 {
        inner_state_eq: Default::default(),
        inner_state_ep: Default::default(),
        state_eq: Default::default(),
        state_ep: Default::default(),
        guard_eq: Default::default(),
        guard_ep: Default::default(),
        terminal_authorization_eq: Default::default(),
        terminal_authorization_ep: Default::default(),
        commit_wrapper_eq: Default::default(),
        commit_wrapper_ep: Default::default(),
        mint_authorization_eq: Default::default(),
        mint_authorization_ep: Default::default(),
        mint_eq: Default::default(),
        mint_ep: Default::default(),
        inner_mint_authorization_eq: Default::default(),
        inner_mint_authorization_ep: Default::default(),
        inner_mint_eq: Default::default(),
        inner_mint_ep: Default::default(),
        mint_hash_shard_eq: Default::default(),
        mint_hash_shard_ep: Default::default(),
        mint_hash_claim_eq: Default::default(),
        mint_hash_claim_ep: Default::default(),
        mint_eq_protocol_digest: [1; 32],
        mint_ep_protocol_digest: [2; 32],
        mint_hash_shard_eq_protocol_digest: [3; 32],
        mint_hash_shard_ep_protocol_digest: [4; 32],
        mint_hash_claim_eq_protocol_digest: [5; 32],
        mint_hash_claim_ep_protocol_digest: [6; 32],
        mint_genesis_authorization_id: [7; 32],
    }
}

#[test]
fn native_mint_rejects_network_and_first_context_substitution() {
    assert!(require_trusted_pins(scope(), network(3), first_context(5)).is_ok());
    assert!(require_trusted_pins(scope(), network(9), first_context(5)).is_err());
    assert!(require_trusted_pins(scope(), network(3), first_context(0)).is_err());
}

#[test]
fn recovered_finality_chain_must_use_the_native_first_context() {
    assert!(
        require_matching_verified_chain_root(
            network(3),
            first_context(5),
            network(3),
            first_context(5)
        )
        .is_ok()
    );
    assert!(
        require_matching_verified_chain_root(
            network(3),
            first_context(5),
            network(9),
            first_context(5)
        )
        .is_err()
    );
    assert!(
        require_matching_verified_chain_root(
            network(3),
            first_context(5),
            network(3),
            first_context(9)
        )
        .is_err()
    );
}

#[test]
fn native_mint_install_derives_all_pins_from_verified_bootstrap() {
    let bootstrap = verified_test_bootstrap_v1();
    assert_eq!(
        authenticated_observation_scope(&bootstrap).unwrap(),
        scope()
    );
    assert_eq!(bootstrap.network_id(), network(3));
    assert_eq!(bootstrap.first_context_id(), first_context(5));
    assert!(bootstrap.trusted_authority_policy().validate().is_ok());
}

#[test]
fn native_mint_install_still_authenticates_release_after_bootstrap() {
    let gate = crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::for_test();
    let publication = gate.exclusive().unwrap();
    let bootstrap = verified_test_bootstrap_v1();
    let anchors = BTreeMap::new();
    let inputs = KagemushaTestnetNativeMintInstallV1 {
        manifest_archive: b"not a release",
        validation_receipt_archive: b"not a receipt",
        release_attestation_archive: b"not an attestation",
        bootstrap: &bootstrap,
        profile: unconfigured_profile(),
        artifact_root: Path::new("/unused"),
        journal_path: Path::new("/unused/journal"),
        mode: KagemushaTestnetDurableObservationModeV1::Create,
        independent_anchors: &anchors,
    };
    assert!(
        KagemushaTestnetNativeMintRuntimeV1::install(&publication.permit(), inputs)
            .err()
            .is_some_and(|error| error.starts_with("invalid KAGEMUSHA release manifest:"))
    );
}

#[test]
fn native_mint_rejects_delayed_bootstrap_before_loading_or_creating_journal() {
    let gate = crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::for_test();
    let publication = gate.exclusive().unwrap();
    let bootstrap = expired_test_bootstrap_v1();
    let anchors = BTreeMap::new();
    let storage = tempfile::tempdir().unwrap();
    let journal = storage.path().join("private-mint");
    let expected = bootstrap.require_unexpired().unwrap_err();
    let inputs = KagemushaTestnetNativeMintInstallV1 {
        manifest_archive: b"not a release",
        validation_receipt_archive: b"not a receipt",
        release_attestation_archive: b"not an attestation",
        bootstrap: &bootstrap,
        profile: unconfigured_profile(),
        artifact_root: storage.path(),
        journal_path: &journal,
        mode: KagemushaTestnetDurableObservationModeV1::Create,
        independent_anchors: &anchors,
    };
    assert_eq!(
        KagemushaTestnetNativeMintRuntimeV1::install(&publication.permit(), inputs).err(),
        Some(expected.clone()),
    );
    assert_eq!(
        super::load_and_install_kagemusha_testnet_durable_state_observation_owner_v1(
            &publication.permit(),
            b"not a release",
            b"not a receipt",
            b"not an attestation",
            &bootstrap,
            unconfigured_profile(),
            storage.path(),
            &journal,
            KagemushaTestnetDurableObservationModeV1::Create,
            &anchors,
        )
        .err(),
        Some(expected),
    );
    assert!(!journal.exists());
}

#[test]
fn native_mint_refuses_finality_without_its_private_reservation() {
    let gate = crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::for_test();
    let publication = gate.exclusive().unwrap();
    // Tests may construct private fields. A real host can obtain a token only
    // after the owner has fsynced the exact native-only reservation.
    let runtime = KagemushaTestnetNativeMintRuntimeV1 {
        trusted_network_id: network(3),
        trusted_first_context_id: first_context(5),
        reservations: Mutex::new(BTreeMap::new()),
    };
    let token = KagemushaTestnetNativeMintReservationV1 {
        operation_id: [9; 32],
        reservation_digest: [10; 32],
    };
    assert_eq!(token.operation_id(), [9; 32]);
    assert_eq!(
        runtime
            .pin_finality_chain(&publication.permit(), &token, b"[]")
            .err(),
        Some("testnet mint finality requires this runtime's persisted reservation".to_owned())
    );
    // A token from another exact reservation remains unusable even if the
    // operation identifier happens to match.
    runtime
        .reservations
        .lock()
        .unwrap()
        .insert(token.operation_id(), [11; 32]);
    assert_eq!(
        runtime
            .pin_finality_chain(&publication.permit(), &token, b"[]")
            .err(),
        Some("testnet mint finality requires this runtime's persisted reservation".to_owned())
    );
    runtime
        .reservations
        .lock()
        .unwrap()
        .insert(token.operation_id(), token.reservation_digest);
    assert!(
        runtime
            .pin_finality_chain(&publication.permit(), &token, b"[]")
            .is_err()
    );
}

#[test]
fn native_mint_inherited_process_never_waits_for_the_private_reservation_mutex() {
    use std::{sync::mpsc, thread, time::Duration};
    let gate =
        crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::inherited_for_test();
    let runtime = KagemushaTestnetNativeMintRuntimeV1 {
        trusted_network_id: network(3),
        trusted_first_context_id: first_context(5),
        reservations: Mutex::new(BTreeMap::new()),
    };
    let token = KagemushaTestnetNativeMintReservationV1 {
        operation_id: [9; 32],
        reservation_digest: [10; 32],
    };
    let locked = runtime.reservations.lock().unwrap();
    let (send, receive) = mpsc::channel();
    thread::scope(|threads| {
        let worker = threads.spawn(|| {
            send.send(
                gate.with_dispatch(|permit| runtime.pin_finality_chain(permit, &token, b"[]"))
                    .err(),
            )
            .unwrap();
        });
        let result = receive.recv_timeout(Duration::from_secs(1));
        // Release and join even on regression, so the test does not strand an owner thread.
        drop(locked);
        worker.join().unwrap();
        assert_eq!(
            result,
            Ok(Some(
                "KAGEMUSHA testnet publication belongs to another process".to_owned()
            ))
        );
    });
}
