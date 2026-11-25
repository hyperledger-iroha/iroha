//! Genuine signed-genesis intent controls; no current admission is inferred at H1.
use super::*;
use crate::{
    query::provider_admission as native,
    smartcontracts::isi::sorafs_provider_admission::test_fixture::{
        NOW, ProviderAdmissionTestFixtureV1 as Fixture, key,
    },
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::sorafs::provider_admission::governance::{
    InitialProviderAdmissionCouncilV1, InitialProviderAdmissionV1,
};
use std::cell::Cell;

fn chain() -> (CertifiedTestChain, InitializeSorafsProviderAdmissionV1) {
    let fixture = Fixture::new();
    let initialize = InitializeSorafsProviderAdmissionV1 {
        council: InitialProviderAdmissionCouncilV1 {
            policy_id: fixture.policy.policy_id,
            trusted_signers: fixture.policy.trusted_signers.clone(),
            signature_threshold: 1,
        },
        providers: vec![InitialProviderAdmissionV1 {
            owner: AccountId::new(key(1).public_key().clone()),
            material: norito::encode_canonical(&ProviderAdmissionGenesisMaterialV1 {
                proposal: fixture.envelope.proposal.clone(),
                advert_body: fixture.envelope.advert_body.clone(),
                issued_at: NOW,
                retention_epoch: NOW + 3600,
            })
            .unwrap(),
        }],
    };
    let mut config = TestChainConfig::new(World::new(), NOW * 1000);
    config.genesis_key = key(1);
    config.genesis_instructions = vec![initialize.clone().into()];
    (CertifiedTestChain::start(config).unwrap(), initialize)
}
#[test]
fn original_signed_intent_is_available_at_h1_without_current_execution_authority() {
    let (mut chain, initialize) = chain();
    let budget = AllocationBudget::new(2 * 1024 * 1024 * 1024);
    let expected: ProviderAdmissionGenesisMaterialV1 =
        decode_frame(&initialize.providers[0].material).unwrap();
    let provider =
        iroha_data_model::sorafs::capacity::ProviderId::new(expected.proposal.provider_id);
    assert!(
        native::read_finalized_provider_admission_v1(&chain.state().view(), provider, NOW).is_err()
    );
    for height in [1, 2] {
        if height == 2 {
            chain.commit_at(NOW * 1000 + 1, Vec::new());
        }
        let called = Cell::new(false);
        with_genesis_provider_admission_originals_v1(
            &chain.state().view(),
            3,
            &budget,
            |originals| {
                called.set(true);
                assert_eq!(originals.len(), 1);
                assert_eq!(originals[0].0, initialize.providers[0].owner);
                assert_eq!(originals[0].1, expected);
                assert!(budget.reserved_bytes() > 0);
                Ok(())
            },
        )
        .unwrap();
        assert!(called.get());
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
#[test]
fn original_intent_bounds_and_pool_refusal_never_enter_the_callback() {
    let (chain, _) = chain();
    let called = Cell::new(false);
    for maximum in [0, PROVIDER_ADMISSION_GENESIS_MAX_PROVIDERS_V1 + 1] {
        assert!(
            with_genesis_provider_admission_originals_v1(
                &chain.state().view(),
                maximum,
                &AllocationBudget::new(2 * 1024 * 1024 * 1024),
                |_| {
                    called.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
    }
    assert!(
        with_genesis_provider_admission_originals_v1(
            &chain.state().view(),
            3,
            &AllocationBudget::new(0),
            |_| {
                called.set(true);
                Ok(())
            }
        )
        .is_err()
    );
    assert!(!called.get());
    let absent = CertifiedTestChain::start(TestChainConfig::new(World::new(), NOW * 1000)).unwrap();
    assert!(
        with_genesis_provider_admission_originals_v1(
            &absent.state().view(),
            3,
            &AllocationBudget::new(2 * 1024 * 1024 * 1024),
            |_| {
                called.set(true);
                Ok(())
            }
        )
        .is_err()
    );
    assert!(!called.get());
}

#[test]
fn genesis_originals_report_only_one_matching_inflight_append() {
    use crate::sumeragi::test_chain::Signers;
    let (mut chain, _) = chain();
    let state = std::sync::Arc::clone(chain.state());
    let budget = AllocationBudget::new(2 * 1024 * 1024 * 1024);
    let proposal = chain.proposal(Some(NOW * 1000 + 1), Vec::new());
    let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
    pending.prepare(Signers::Quorum).unwrap();
    pending.append_prepared().unwrap();
    {
        let captured = state.view();
        assert_eq!(state.committed_height(), 1);
        for _ in 0..2 {
            let result =
                classify_genesis_provider_admission_originals_v1(&captured, 3, &budget, |_| {
                    panic!("stale State entered callback")
                });
            assert_eq!(
                result,
                Err::<(), _>(GenesisProviderOriginalsReadErrorV1::StateBehind(2))
            );
        }
        let rejected = || GenesisProviderOriginalsReadErrorV1::Rejected(ProviderAdmissionErrorV1);
        assert_eq!(original_frontier(&captured, 0, true), Err(rejected()));
        let metered = norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(1 << 24, 1 << 24, 1 << 24, 1 << 25, 64),
            || original_frontier(&captured, 3, true),
        );
        assert_eq!(metered, Err(rejected()));
        assert!(
            with_genesis_provider_admission_originals_v1(
                &captured,
                3,
                &budget,
                |_| -> Result<(), _> { panic!("independent stale read entered callback") }
            )
            .is_err()
        );
    }
    pending.publish(Signers::Quorum).unwrap();
    assert_eq!(
        classify_genesis_provider_admission_originals_v1(
            &state.view(),
            3,
            &budget,
            |v| Ok(v.len())
        ),
        Ok(1)
    );
    assert_eq!(
        classify_genesis_provider_admission_originals_v1(
            &state.view(),
            3,
            &budget,
            |_| Err::<(), _>(ProviderAdmissionErrorV1)
        ),
        Err(GenesisProviderOriginalsReadErrorV1::Rejected(
            ProviderAdmissionErrorV1
        ))
    );
}
