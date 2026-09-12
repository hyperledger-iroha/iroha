//! The real Core boundary never upgrades empty storage or a hash-cache claim into finality.
use super::{
    StreamTokenHardwarePinsV1, StreamTokenIssuerError, StreamTokenStateObserverClientV1,
    hardware_finality::{
        CoreFinalityV1, FinalityFloorV1, HardwareFinalityV1, HistoricalFinalityV1,
        check_control_pins, check_observed_control, check_registered_provider,
    },
    hardware_test_support::{
        CHAIN, NETWORK, NOW_MS, PROVIDER, SignedFixture, SignedObserver, TestSignerMode, anchor,
        storage_config,
    },
};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{BlockHashes, State, World},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    block::BlockHeader,
    sorafs::capacity::ProviderId,
};
use sorafs_manifest::signer::custody::SignerCustodyAnchorV1;
use sorafs_manifest::signer::{
    stream_token_custody_control::{StreamTokenCustodyControlStateV1, StreamTokenCustodyPolicyV1},
    stream_token_evidence::{
        SignerStreamTokenObservationExpectedV1, SignerStreamTokenObservationPhaseV1,
        SignerStreamTokenStateObservationBodyV1, SignerStreamTokenStateObservationV1,
        verify_stream_token_signer_current_evidence_v1,
    },
};
use std::{num::NonZeroUsize, sync::Arc};

#[test]
fn actual_core_finality_requires_durable_certified_history_beyond_public_cache_claims() {
    let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x74; 32]));
    let anchor = SignerCustodyAnchorV1 {
        height: 1,
        block_hash: *hash.as_ref(),
        state_digest: [0x85; 32],
    };
    for cache_only in [false, true] {
        let kura = Kura::blank_kura_for_testing();
        let mut state = State::new_for_testing(
            provider_world(Some(PROVIDER)),
            kura.clone(),
            LiveQueryStore::start_test(),
        );
        if cache_only {
            state.block_hashes = BlockHashes::new(vec![hash]);
        }
        assert_eq!(state.block_hashes.view().len(), usize::from(cache_only));
        assert!(
            kura.get_durable_block_hash(NonZeroUsize::new(1).unwrap())
                .is_none()
        );
        let pins = StreamTokenHardwarePinsV1::from_config(&storage_config(1), CHAIN, NETWORK)
            .expect("valid public test pins")
            .expect("enabled test pins");
        check_registered_provider(&state.view(), &pins)
            .expect("this test reaches the durable history gate after provider admission");
        let guard = CoreFinalityV1::new(Arc::new(state), pins);
        assert!(matches!(
            guard.capture(anchor),
            Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
        ));
        let (_, _, observation) = signed_control_fixture();
        assert!(matches!(
            guard.validate(
                anchor,
                anchor,
                FinalityFloorV1 {
                    height: 1,
                    block_hash: anchor.block_hash
                },
                &[HistoricalFinalityV1::Custody(
                    observation.active_head.approved_anchor
                )],
                &observation,
            ),
            Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
        ));
    }
}

fn provider_world(provider: Option<[u8; 32]>) -> World {
    let owner = AccountId::new(
        KeyPair::try_from_seed(vec![0xA1; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    let mut world = World::with([], [Account::new(owner.clone()).build(&owner)], []);
    if let Some(provider) = provider {
        world
            .provider_owners
            .insert(ProviderId::new(provider), owner);
    }
    world
}

#[test]
fn current_provider_registration_is_required_independently_of_retained_custody() {
    let pins = StreamTokenHardwarePinsV1::from_config(&storage_config(1), CHAIN, NETWORK)
        .unwrap()
        .unwrap();
    let state = State::new_for_testing(
        provider_world(Some(PROVIDER)),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    check_registered_provider(&state.view(), &pins).expect("exact currently registered provider");
    for provider in [None, Some([0x99; 32])] {
        let unavailable = State::new_for_testing(
            provider_world(provider),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        assert!(matches!(
            check_registered_provider(&unavailable.view(), &pins),
            Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
        ));
    }
}

// This independently signed software fixture exercises the comparison boundary only. It does
// not manufacture a verified native snapshot, durable consensus history or hardware evidence.
fn signed_control_fixture() -> (
    StreamTokenHardwarePinsV1,
    StreamTokenCustodyControlStateV1,
    SignerStreamTokenStateObservationBodyV1,
) {
    let fixture = SignedFixture::new(1, TestSignerMode::Sign);
    let pins = fixture.pins.clone();
    let attempt = SignerStreamTokenObservationExpectedV1::current(
        pins.binding(),
        SignerStreamTokenObservationPhaseV1::BeforeAdmission,
        [0x93; 32],
        anchor(90),
        NOW_MS,
    )
    .unwrap();
    let reply = SignedObserver(fixture).observe(attempt.request()).unwrap();
    let (record, bytes) = reply.current_evidence().unwrap();
    let observation = SignerStreamTokenStateObservationV1::decode_canonical(bytes)
        .unwrap()
        .body;
    verify_stream_token_signer_current_evidence_v1(
        record,
        bytes,
        pins.binding(),
        pins.custody_trust(),
        pins.observer_trust(),
        attempt,
        NOW_MS,
    )
    .expect("independent custody and challenged observer signatures are valid");
    let trust = pins.custody_trust();
    let state = StreamTokenCustodyControlStateV1 {
        policy: StreamTokenCustodyPolicyV1 {
            binding: pins.binding().clone(),
            attester_authority: trust.authority.clone(),
            attester_public_key: trust.public_key.clone(),
            active_from_unix_ms: trust.active_from_unix_ms,
            active_until_unix_ms: trust.active_until_unix_ms,
            max_validity_ms: trust.max_validity_ms,
            max_anchor_age_ms: trust.max_anchor_age_ms,
        },
        next_sequence: observation.active_head.sequence.checked_add(1).unwrap(),
        predecessor_digest: observation.active_head.record_digest,
        active_head: Some(observation.active_head),
        signer_revoked: false,
        attester_revoked: false,
    };
    state.validate().expect("well-formed comparison fixture");
    (pins, state, observation)
}

#[test]
fn native_custody_comparison_rejects_each_substituted_active_head_coordinate() {
    let (_, state, observation) = signed_control_fixture();
    let candidate = observation.current_anchor;
    check_observed_control(&state, candidate, &observation).unwrap();
    for case in [
        "record",
        "sequence",
        "approval_height",
        "approval_hash",
        "approval_state",
        "key_generation",
        "policy_generation",
        "policy_digest",
        "unenrolled",
    ] {
        let mut changed = state.clone();
        let head = changed.active_head.as_mut().unwrap();
        match case {
            "record" => head.record_digest[0] ^= 1,
            "sequence" => head.sequence += 1,
            "approval_height" => head.approved_anchor.height += 1,
            "approval_hash" => head.approved_anchor.block_hash[0] ^= 1,
            "approval_state" => head.approved_anchor.state_digest[0] ^= 1,
            "key_generation" => head.key_revision += 1,
            "policy_generation" => head.policy_revision += 1,
            "policy_digest" => head.policy_digest[0] ^= 1,
            "unenrolled" => changed.active_head = None,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                check_observed_control(&changed, candidate, &observation),
                Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
            ),
            "{case}"
        );
    }
    let mut wrong_anchor = candidate;
    wrong_anchor.state_digest[0] ^= 1;
    assert!(matches!(
        check_observed_control(&state, wrong_anchor, &observation),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
}

#[test]
fn native_revocation_cannot_be_hidden_or_authorized_by_an_observer() {
    let (_, state, observation) = signed_control_fixture();
    for signer in [false, true] {
        for attester in [false, true] {
            if !signer && !attester {
                continue;
            }
            let mut revoked = state.clone();
            revoked.signer_revoked = signer;
            revoked.attester_revoked = attester;
            // Both a stale unrevoked observation and an accurately revoked one must fail.
            for agrees in [false, true] {
                let mut observed = observation.clone();
                observed.signer_revoked = agrees && signer;
                observed.attester_revoked = agrees && attester;
                assert!(matches!(
                    check_observed_control(&revoked, observed.current_anchor, &observed),
                    Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
                ));
            }
            let mut unconfirmed = observation.clone();
            unconfirmed.signer_revoked = signer;
            unconfirmed.attester_revoked = attester;
            assert!(matches!(
                check_observed_control(&state, unconfirmed.current_anchor, &unconfirmed),
                Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
            ));
        }
    }
}

#[test]
fn native_control_cannot_replace_independently_pinned_binding_or_attester_trust() {
    let (pins, state, _) = signed_control_fixture();
    check_control_pins(&state, &pins).unwrap();
    for case in [
        "binding",
        "attester_service",
        "attester_administrator",
        "attester_key_generation",
        "attester_policy_generation",
        "attester_policy_digest",
        "attester_key",
        "eligibility_start",
        "eligibility_end",
        "validity_limit",
        "anchor_age_limit",
    ] {
        let mut changed = state.clone();
        let policy = &mut changed.policy;
        match case {
            "binding" => policy.binding.network_id[0] ^= 1,
            "attester_service" => policy.attester_authority.service_id.push_str("-other"),
            "attester_administrator" => policy
                .attester_authority
                .administrator_id
                .push_str("-other"),
            "attester_key_generation" => policy.attester_authority.key_revision += 1,
            "attester_policy_generation" => policy.attester_authority.policy_revision += 1,
            "attester_policy_digest" => policy.attester_authority.policy_digest[0] ^= 1,
            "attester_key" => {
                policy.attester_public_key =
                    KeyPair::try_from_seed(vec![0x77; 32], Algorithm::Ed25519)
                        .unwrap()
                        .public_key()
                        .clone()
            }
            "eligibility_start" => policy.active_from_unix_ms += 1,
            "eligibility_end" => policy.active_until_unix_ms += 1,
            "validity_limit" => policy.max_validity_ms += 1,
            "anchor_age_limit" => policy.max_anchor_age_ms += 1,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                check_control_pins(&changed, &pins),
                Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
            ),
            "{case}"
        );
    }
}
