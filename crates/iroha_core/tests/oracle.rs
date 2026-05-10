//! Oracle ISI integration tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    str::FromStr,
    sync::Arc,
};

use iroha_config::parameters::{
    actual::{
        Oracle as OracleConfig, OracleChangeThresholds, OracleEconomics, OracleGovernance,
        OracleTwitterBinding,
    },
    defaults,
};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute as _,
    smartcontracts::{ValidQuery, ValidSingularQuery},
    state::{State, StateTransaction, World, WorldReadOnly},
    telemetry::StateTelemetry,
};
use iroha_crypto::{Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    account::Account,
    asset::{Asset, AssetDefinition},
    block::BlockHeader,
    domain::Domain,
    isi::{
        AggregateOracleFeed, InstructionBox, OpenOracleDispute, ProposeOracleChange,
        RecordTwitterBinding, RegisterOracleFeed, ResolveOracleDispute, RevokeTwitterBinding,
        RollbackOracleChange, SubmitOracleObservation, VoteOracleChangeStage,
    },
    nexus::UniversalAccountId,
    oracle::{
        AbsoluteOutlier, AggregationRule, FeedConfig, FeedConfigVersion, FeedEventOutcome, FeedId,
        FeedSlot, KeyedHash, Observation, ObservationBody, ObservationOutcome, ObservationValue,
        OracleChangeClass, OracleChangeId, OracleChangeStage, OracleChangeStatus,
        OracleProviderKey, OutlierPolicy, RiskClass, TWITTER_FOLLOW_FEED_ID,
        TwitterBindingAttestation, TwitterBindingStatus,
    },
    permission::Permission,
    prelude::*,
    query::dsl::CompoundPredicate,
    role::{Role, RoleId},
};
use iroha_executor_data_model::permission::oracle as oracle_permission;
use iroha_primitives::numeric::Numeric;
use iroha_telemetry::metrics::Metrics;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn test_telemetry() -> StateTelemetry {
    StateTelemetry::new(Arc::new(Metrics::default()), false)
}

fn oracle_config() -> OracleConfig {
    OracleConfig {
        history_depth: nonzero!(4_usize),
        economics: OracleEconomics {
            reward_asset: defaults::oracle::reward_asset(),
            reward_pool: defaults::oracle::reward_pool(),
            reward_amount: defaults::oracle::reward_amount(),
            slash_asset: defaults::oracle::slash_asset(),
            slash_receiver: defaults::oracle::slash_receiver(),
            slash_outlier_amount: defaults::oracle::slash_outlier_amount(),
            slash_error_amount: defaults::oracle::slash_error_amount(),
            slash_no_show_amount: defaults::oracle::slash_no_show_amount(),
            dispute_bond_asset: defaults::oracle::dispute_bond_asset(),
            dispute_bond_amount: defaults::oracle::dispute_bond_amount(),
            dispute_reward_amount: defaults::oracle::dispute_reward_amount(),
            frivolous_slash_amount: defaults::oracle::frivolous_slash_amount(),
        },
        governance: OracleGovernance {
            intake_sla_blocks: defaults::oracle::intake_sla_blocks(),
            rules_sla_blocks: defaults::oracle::rules_sla_blocks(),
            cop_sla_blocks: defaults::oracle::cop_sla_blocks(),
            technical_sla_blocks: defaults::oracle::technical_sla_blocks(),
            policy_jury_sla_blocks: defaults::oracle::policy_jury_sla_blocks(),
            enact_sla_blocks: defaults::oracle::enact_sla_blocks(),
            intake_min_votes: defaults::oracle::intake_min_votes(),
            rules_min_votes: defaults::oracle::rules_min_votes(),
            cop_min_votes: OracleChangeThresholds {
                low: defaults::oracle::cop_low_votes(),
                medium: defaults::oracle::cop_medium_votes(),
                high: defaults::oracle::cop_high_votes(),
            },
            technical_min_votes: defaults::oracle::technical_min_votes(),
            policy_jury_min_votes: OracleChangeThresholds {
                low: defaults::oracle::policy_jury_low_votes(),
                medium: defaults::oracle::policy_jury_medium_votes(),
                high: defaults::oracle::policy_jury_high_votes(),
            },
        },
        twitter_binding: OracleTwitterBinding {
            feed_id: defaults::oracle::twitter_binding_feed_id(),
            pepper_id: defaults::oracle::twitter_binding_pepper_id(),
            max_ttl_ms: defaults::oracle::twitter_binding_max_ttl_ms(),
            min_ttl_ms: defaults::oracle::twitter_binding_min_ttl_ms(),
            min_update_spacing_ms: defaults::oracle::twitter_binding_min_update_spacing_ms(),
        },
    }
}

fn feed_config(feed_id: FeedId, providers: Vec<AccountId>) -> FeedConfig {
    let committee_size = u16::try_from(providers.len()).expect("provider count fits");
    FeedConfig {
        feed_id,
        feed_config_version: FeedConfigVersion(1),
        providers,
        connector_id: "http-bin".to_string(),
        connector_version: 1,
        cadence_slots: nonzero!(1_u64),
        aggregation: AggregationRule::MedianMad(300),
        outlier_policy: OutlierPolicy::Mad(350),
        min_signers: 1,
        committee_size,
        risk_class: RiskClass::Low,
        max_observers: 4,
        max_value_len: 128,
        max_error_rate_bps: 250,
        dispute_window_slots: nonzero!(4_u64),
        replay_window_slots: nonzero!(2_u64),
    }
}

fn observation(
    provider: AccountId,
    signer: &KeyPair,
    feed_id: &FeedId,
    slot: u64,
    request_hash: Hash,
    value: i128,
) -> Observation {
    let body = ObservationBody {
        feed_id: feed_id.clone(),
        feed_config_version: FeedConfigVersion(1),
        slot,
        provider_id: provider,
        connector_id: "http-bin".to_string(),
        connector_version: 1,
        request_hash,
        outcome: ObservationOutcome::Value(ObservationValue::new(value, 5)),
        timestamp_ms: Some(1_700_000_000_000),
    };
    let signature = SignatureOf::new(signer.private_key(), &body);
    Observation { body, signature }
}

fn grant_permission(world: &mut World, account: &AccountId, permission: impl Into<Permission>) {
    let permission = permission.into();
    let mut permissions = {
        let permissions = world.account_permissions_mut_for_testing().view();
        permissions.get(account).cloned().unwrap_or_default()
    };
    permissions.insert(permission);
    world
        .account_permissions_mut_for_testing()
        .insert(account.clone(), permissions);
}

fn grant_oracle_operator_permissions(world: &mut World, account: &AccountId) {
    grant_permission(world, account, oracle_permission::CanRegisterOracleFeed);
    grant_permission(world, account, oracle_permission::CanProposeOracleChange);
    grant_permission(world, account, oracle_permission::CanRollbackOracleChange);
    grant_permission(world, account, oracle_permission::CanResolveOracleDispute);
    grant_permission(world, account, oracle_permission::CanManageTwitterBindings);
    for stage in [
        OracleChangeStage::Intake,
        OracleChangeStage::RulesCommittee,
        OracleChangeStage::CopReview,
        OracleChangeStage::TechnicalAudit,
        OracleChangeStage::PolicyJury,
        OracleChangeStage::Enactment,
    ] {
        grant_permission(
            world,
            account,
            oracle_permission::CanVoteOracleChangeStage { stage },
        );
    }
}

fn execute_boxed(
    instruction: impl Into<InstructionBox>,
    authority: &AccountId,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), iroha_data_model::isi::error::InstructionExecutionError> {
    let instruction: InstructionBox = instruction.into();
    instruction.execute(authority, tx)
}

fn assert_rejects_with(
    result: Result<(), iroha_data_model::isi::error::InstructionExecutionError>,
    expected: &str,
) {
    let err = result.expect_err("operation should reject");
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains(expected),
        "expected error to contain `{expected}`, got `{rendered}`"
    );
}

fn role_with_permission(role_id: RoleId, permission: impl Into<Permission>) -> Role {
    let mut permissions = BTreeSet::new();
    permissions.insert(permission.into());
    Role {
        id: role_id,
        permissions,
        permission_epochs: BTreeMap::new(),
    }
}

fn world_with_providers(providers: &[AccountId]) -> World {
    let validator_domain_id: DomainId =
        DomainId::try_new("validators", "universal").expect("domain");
    let domain: Domain = Domain::new(validator_domain_id.clone())
        .build(providers.first().expect("at least one provider"));
    let accounts = providers
        .iter()
        .map(|id| Account::new(id.clone()).build(id));
    let mut world = World::with([domain], accounts, []);
    for provider in providers {
        grant_oracle_operator_permissions(&mut world, provider);
    }
    world
}

fn world_with_provider_role_permission(
    provider: &AccountId,
    role_id: RoleId,
    permission: impl Into<Permission>,
) -> World {
    let validator_domain_id: DomainId =
        DomainId::try_new("validators", "universal").expect("domain");
    let domain: Domain = Domain::new(validator_domain_id).build(provider);
    let account = Account::new(provider.clone()).build(provider);
    let role = role_with_permission(role_id.clone(), permission);
    let mut world = World::with_assets_and_roles(
        [domain],
        [account],
        std::iter::empty::<AssetDefinition>(),
        std::iter::empty::<Asset>(),
        std::iter::empty::<Nft>(),
        [role],
    );
    world.grant_role_for_tests(provider.clone(), role_id);
    world
}

fn header(height: u64) -> BlockHeader {
    let nonzero_height = NonZeroU64::new(height).expect("height must be non-zero");
    BlockHeader::new(nonzero_height, None, None, None, 0, 0)
}

fn oracle_state_with_accounts(
    providers: &[AccountId],
) -> (State, AssetDefinitionId, AccountId, AccountId) {
    let asset_def_id: AssetDefinitionId = defaults::oracle::reward_asset();
    let reward_pool = defaults::oracle::reward_pool();
    let slash_receiver = defaults::oracle::slash_receiver();

    let validator_domain_id: DomainId =
        DomainId::try_new("validators", "universal").expect("domain");
    let validator_domain: Domain =
        Domain::new(validator_domain_id.clone()).build(providers.first().expect("provider"));
    let sora_domain_id: DomainId = DomainId::try_new("sora", "universal").expect("domain");
    let sora_domain: Domain = Domain::new(sora_domain_id.clone()).build(&reward_pool);

    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&reward_pool);
    let mut assets = vec![
        Asset::new(
            AssetId::new(asset_def_id.clone(), reward_pool.clone()),
            Numeric::from_str("10").expect("reward pool amount"),
        ),
        Asset::new(
            AssetId::new(asset_def_id.clone(), slash_receiver.clone()),
            Numeric::from_str("5").expect("slash receiver amount"),
        ),
    ];
    let accounts: Vec<_> = providers
        .iter()
        .map(|id| {
            assets.push(Asset::new(
                AssetId::new(asset_def_id.clone(), id.clone()),
                Numeric::from_str("5").expect("provider balance"),
            ));
            Account::new(id.clone()).build(id)
        })
        .collect();

    let mut world = World::with_assets(
        [validator_domain, sora_domain],
        accounts.into_iter().chain([
            Account::new(reward_pool.clone()).build(&reward_pool),
            Account::new(slash_receiver.clone()).build(&slash_receiver),
        ]),
        [asset_def],
        assets,
        [],
    );
    for provider in providers {
        grant_oracle_operator_permissions(&mut world, provider);
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let telemetry = test_telemetry();
    let mut state = State::with_telemetry(world, kura, query, telemetry);
    state.set_oracle(oracle_config());
    (state, asset_def_id, reward_pool, slash_receiver)
}

fn aggregate_round(
    tx: &mut StateTransaction<'_, '_>,
    feed_id: &FeedId,
    providers: [(&AccountId, &KeyPair); 2],
    slot: u64,
    request: Hash,
    value: i128,
    evidence_hashes: Vec<Hash>,
) {
    for (provider, signer) in providers {
        SubmitOracleObservation {
            observation: observation(provider.clone(), signer, feed_id, slot, request, value),
        }
        .execute(provider, tx)
        .expect("submit observation");
    }
    AggregateOracleFeed {
        feed_id: feed_id.clone(),
        slot,
        request_hash: request,
        evidence_hashes,
    }
    .execute(providers[0].0, tx)
    .expect("aggregate slot");
}

fn record_twitter_binding_round(
    tx: &mut StateTransaction<'_, '_>,
    provider: &AccountId,
    signer: &KeyPair,
    feed_config: &FeedConfig,
    attestation: &TwitterBindingAttestation,
) {
    RegisterOracleFeed {
        feed: feed_config.clone(),
    }
    .execute(provider, tx)
    .expect("register feed");
    SubmitOracleObservation {
        observation: twitter_binding_observation(provider, signer, feed_config, attestation),
    }
    .execute(provider, tx)
    .expect("submit twitter binding observation");
    AggregateOracleFeed {
        feed_id: feed_config.feed_id.clone(),
        slot: attestation.slot,
        request_hash: attestation.request_hash,
        evidence_hashes: Vec::new(),
    }
    .execute(provider, tx)
    .expect("aggregate twitter binding slot");
    RecordTwitterBinding {
        attestation: attestation.clone(),
        feed_id: feed_config.feed_id.clone(),
    }
    .execute(provider, tx)
    .expect("record twitter binding");
}

#[test]
fn oracle_flow_persists_history_and_prunes() {
    let (provider_a, signer_a) = iroha_test_samples::gen_account_in("validators");
    let (provider_b, signer_b) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "price_xor_usd".parse().expect("feed id");

    let (mut state, _, _, _) =
        oracle_state_with_accounts(&[provider_a.clone(), provider_b.clone()]);
    let mut oracle_cfg = oracle_config();
    oracle_cfg.history_depth = nonzero!(1_usize);
    state.set_oracle(oracle_cfg);

    // First round: register feed, buffer observations, aggregate, and persist history.
    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let mut config = feed_config(
        feed_id.clone(),
        vec![provider_a.clone(), provider_b.clone()],
    );
    config.replay_window_slots = nonzero!(1_u64);
    RegisterOracleFeed {
        feed: config.clone(),
    }
    .execute(&provider_a, &mut stx)
    .expect("register feed");

    let req0 = Hash::new(b"req-0");
    aggregate_round(
        &mut stx,
        &feed_id,
        [(&provider_a, &signer_a), (&provider_b, &signer_b)],
        10,
        req0,
        1_000_000,
        vec![Hash::new(b"bundle-0")],
    );
    stx.apply();
    sb.commit().expect("commit first block");

    {
        let view = state.view();
        let history = view
            .world()
            .oracle_history()
            .get(&feed_id)
            .expect("history exists");
        assert_eq!(history.len(), 1);
        let record = history.last().expect("latest record");
        assert_eq!(record.evidence_hashes.len(), 1);
        match &record.event.outcome {
            FeedEventOutcome::Success(success) => {
                assert_eq!(success.value, ObservationValue::new(1_000_000, 5));
                assert_eq!(success.entries.len(), 2);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert!(view.world().oracle_observations().iter().next().is_none());
    }

    // Second round: new slot should prune history because depth is capped at one.
    let mut sb = state.block(header(2));
    let mut stx = sb.transaction();
    let req1 = Hash::new(b"req-1");
    aggregate_round(
        &mut stx,
        &feed_id,
        [(&provider_a, &signer_a), (&provider_b, &signer_b)],
        11,
        req1,
        2_000_000,
        Vec::new(),
    );
    stx.apply();
    sb.commit().expect("commit second block");

    let view = state.view();
    let history = view
        .world()
        .oracle_history()
        .get(&feed_id)
        .expect("history exists");
    assert_eq!(history.len(), 1, "older record pruned to depth");
    assert_eq!(history.last().unwrap().event.slot, 11);
    assert!(view.world().oracle_observations().iter().next().is_none());
}

#[test]
fn oracle_rejects_wrong_authority() {
    let (provider, signer) = iroha_test_samples::gen_account_in("validators");
    let (outsider, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "price_xor_usd".parse().expect("feed id");
    let world = world_with_providers(&[provider.clone(), outsider.clone()]);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let telemetry = test_telemetry();
    let state = State::with_telemetry(world, kura, query, telemetry);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    RegisterOracleFeed {
        feed: feed_config(feed_id.clone(), vec![provider.clone()]),
    }
    .execute(&provider, &mut stx)
    .expect("register feed");

    let req = Hash::new(b"req-unauthorized");
    let err = SubmitOracleObservation {
        observation: observation(provider.clone(), &signer, &feed_id, 7, req, 42_000),
    }
    .execute(&outsider, &mut stx)
    .expect_err("authority mismatch must reject");
    assert!(matches!(
        err,
        iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(_)
    ));
}

#[test]
fn oracle_register_requires_typed_permission_and_accepts_role_permission() {
    let (provider, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "role_permission_feed".parse().expect("feed id");
    let validator_domain_id: DomainId =
        DomainId::try_new("validators", "universal").expect("domain");
    let domain = Domain::new(validator_domain_id).build(&provider);
    let account = Account::new(provider.clone()).build(&provider);
    let mut state = State::with_telemetry(
        World::with([domain], [account], []),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        test_telemetry(),
    );
    state.set_oracle(oracle_config());

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let err = execute_boxed(
        RegisterOracleFeed {
            feed: feed_config(feed_id.clone(), vec![provider.clone()]),
        },
        &provider,
        &mut stx,
    )
    .expect_err("direct permission or role permission is required");
    assert!(matches!(
        err,
        iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(_)
    ));

    let role_id: RoleId = "oracle_registerer".parse().expect("role id");
    let role_world = world_with_provider_role_permission(
        &provider,
        role_id,
        oracle_permission::CanRegisterOracleFeed,
    );
    let mut state = State::with_telemetry(
        role_world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        test_telemetry(),
    );
    state.set_oracle(oracle_config());

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    execute_boxed(
        RegisterOracleFeed {
            feed: feed_config(feed_id.clone(), vec![provider.clone()]),
        },
        &provider,
        &mut stx,
    )
    .expect("role permission should authorize feed registration");
    stx.apply();
    sb.commit().expect("commit role-authorized registration");

    assert!(state.view().world().oracle_feeds().get(&feed_id).is_some());
}

#[test]
fn oracle_instruction_box_dispatch_covers_operator_surface() {
    let (provider_a, signer_a) = iroha_test_samples::gen_account_in("validators");
    let (provider_b, signer_b) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "dispatch_price_feed".parse().expect("feed id");
    let (mut state, _, _, _) =
        oracle_state_with_accounts(&[provider_a.clone(), provider_b.clone()]);

    let mut cfg = oracle_config();
    cfg.governance.intake_min_votes = nonzero!(1_usize);
    cfg.governance.rules_min_votes = nonzero!(1_usize);
    cfg.governance.cop_min_votes = OracleChangeThresholds {
        low: nonzero!(1_usize),
        medium: nonzero!(1_usize),
        high: nonzero!(1_usize),
    };
    cfg.governance.technical_min_votes = nonzero!(1_usize);
    cfg.governance.policy_jury_min_votes = OracleChangeThresholds {
        low: nonzero!(1_usize),
        medium: nonzero!(1_usize),
        high: nonzero!(1_usize),
    };
    state.set_oracle(cfg);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let config = feed_config(
        feed_id.clone(),
        vec![provider_a.clone(), provider_b.clone()],
    );
    execute_boxed(
        RegisterOracleFeed {
            feed: config.clone(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch register feed");

    let request = Hash::new(b"dispatch-request");
    for (provider, signer, value) in [
        (&provider_a, &signer_a, 42_000_i128),
        (&provider_b, &signer_b, 42_100_i128),
    ] {
        execute_boxed(
            SubmitOracleObservation {
                observation: observation(provider.clone(), signer, &feed_id, 12, request, value),
            },
            provider,
            &mut stx,
        )
        .expect("dispatch submit observation");
    }
    execute_boxed(
        AggregateOracleFeed {
            feed_id: feed_id.clone(),
            slot: 12,
            request_hash: request,
            evidence_hashes: vec![Hash::new(b"dispatch-bundle")],
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch aggregate feed");

    execute_boxed(
        OpenOracleDispute {
            feed_id: feed_id.clone(),
            slot: 12,
            request_hash: request,
            target: provider_b.clone(),
            bond: None,
            evidence_hashes: vec![Hash::new(b"dispatch-dispute")],
            reason: "provider challenge".to_string(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch open dispute");
    execute_boxed(
        ResolveOracleDispute {
            dispute_id: dispute_id(&feed_id, 12, request, &provider_a, &provider_b),
            outcome: iroha_data_model::oracle::OracleDisputeOutcome::Frivolous,
            notes: "insufficient evidence".to_string(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch resolve dispute");

    let change_id = OracleChangeId(Hash::new(b"dispatch-change"));
    let change_feed_id: FeedId = "dispatch_change_feed".parse().expect("feed id");
    execute_boxed(
        ProposeOracleChange {
            change_id,
            feed: feed_config(change_feed_id, vec![provider_a.clone(), provider_b.clone()]),
            class: OracleChangeClass::Low,
            payload_hash: Hash::new(b"dispatch-change-payload"),
            evidence_hashes: vec![Hash::new(b"dispatch-intake")],
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch propose change");
    execute_boxed(
        VoteOracleChangeStage {
            change_id,
            stage: OracleChangeStage::Intake,
            approve: true,
            evidence_hashes: vec![Hash::new(b"dispatch-vote")],
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch vote stage");
    execute_boxed(
        RollbackOracleChange {
            change_id,
            stage: Some(OracleChangeStage::RulesCommittee),
            reason: "operator rollback".to_string(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch rollback change");

    let twitter_kit = iroha_data_model::oracle::kits::twitter_follow_binding();
    let mut twitter_feed = twitter_kit.feed_config;
    twitter_feed.providers = vec![provider_a.clone()];
    twitter_feed.min_signers = 1;
    twitter_feed.committee_size = 1;
    twitter_feed.feed_id = TWITTER_FOLLOW_FEED_ID.parse().expect("feed id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"dispatch-uaid"));
    let binding_hash = KeyedHash::new(
        defaults::oracle::twitter_binding_pepper_id(),
        defaults::oracle::twitter_binding_pepper_id(),
        b"dispatch-user",
    );
    let attestation = twitter_binding_attestation(&twitter_feed, &uaid, binding_hash.clone(), 100);
    execute_boxed(
        RegisterOracleFeed {
            feed: twitter_feed.clone(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch register twitter feed");
    execute_boxed(
        SubmitOracleObservation {
            observation: twitter_binding_observation(
                &provider_a,
                &signer_a,
                &twitter_feed,
                &attestation,
            ),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch submit twitter observation");
    execute_boxed(
        AggregateOracleFeed {
            feed_id: twitter_feed.feed_id.clone(),
            slot: attestation.slot,
            request_hash: attestation.request_hash,
            evidence_hashes: Vec::new(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch aggregate twitter feed");
    execute_boxed(
        RecordTwitterBinding {
            attestation,
            feed_id: twitter_feed.feed_id.clone(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch record twitter binding");
    execute_boxed(
        RevokeTwitterBinding {
            binding_hash: binding_hash.clone(),
            reason: "operator cleanup".to_string(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("dispatch revoke twitter binding");

    stx.apply();
    sb.commit().expect("commit dispatch block");

    let view = state.view();
    assert!(view.world().oracle_feeds().get(&feed_id).is_some());
    assert_eq!(
        view.world()
            .oracle_history()
            .get(&feed_id)
            .expect("history")
            .len(),
        1
    );
    assert!(
        view.world()
            .twitter_bindings()
            .get(&binding_hash.digest)
            .is_none()
    );
}

#[test]
fn oracle_register_rejects_invalid_feed_configs() {
    let (provider_a, _) = iroha_test_samples::gen_account_in("validators");
    let (provider_b, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "invalid_feed_config".parse().expect("feed id");
    let world = world_with_providers(&[provider_a.clone(), provider_b.clone()]);
    let mut state = State::with_telemetry(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        test_telemetry(),
    );
    state.set_oracle(oracle_config());

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();

    let mut duplicate = feed_config(
        feed_id.clone(),
        vec![provider_a.clone(), provider_a.clone()],
    );
    duplicate.committee_size = 2;
    assert_rejects_with(
        execute_boxed(
            RegisterOracleFeed { feed: duplicate },
            &provider_a,
            &mut stx,
        ),
        "duplicate oracle provider",
    );

    let mut zero_quorum = feed_config(feed_id.clone(), vec![provider_a.clone()]);
    zero_quorum.min_signers = 0;
    assert_rejects_with(
        execute_boxed(
            RegisterOracleFeed { feed: zero_quorum },
            &provider_a,
            &mut stx,
        ),
        "at least one signer",
    );

    let mut oversized_committee = feed_config(feed_id.clone(), vec![provider_a.clone()]);
    oversized_committee.committee_size = 2;
    assert_rejects_with(
        execute_boxed(
            RegisterOracleFeed {
                feed: oversized_committee,
            },
            &provider_a,
            &mut stx,
        ),
        "committee_size 2 exceeds providers 1",
    );

    let mut min_exceeds_committee = feed_config(feed_id.clone(), vec![provider_a.clone()]);
    min_exceeds_committee.min_signers = 2;
    assert_rejects_with(
        execute_boxed(
            RegisterOracleFeed {
                feed: min_exceeds_committee,
            },
            &provider_a,
            &mut stx,
        ),
        "min_signers 2 exceeds committee_size 1",
    );

    let mut invalid_error_rate = feed_config(feed_id.clone(), vec![provider_a.clone()]);
    invalid_error_rate.max_error_rate_bps = 10_001;
    assert_rejects_with(
        execute_boxed(
            RegisterOracleFeed {
                feed: invalid_error_rate,
            },
            &provider_a,
            &mut stx,
        ),
        "max_error_rate_bps 10001 exceeds 10000",
    );

    let mut replay_exceeds_history = feed_config(feed_id, vec![provider_a]);
    replay_exceeds_history.replay_window_slots = nonzero!(5_u64);
    assert_rejects_with(
        execute_boxed(
            RegisterOracleFeed {
                feed: replay_exceeds_history,
            },
            &provider_b,
            &mut stx,
        ),
        "replay_window_slots 5 exceeds retained history depth 4",
    );
}

#[test]
fn oracle_aggregation_rejects_insufficient_quorum_replay_and_stale_windows() {
    let (provider_a, signer_a) = iroha_test_samples::gen_account_in("validators");
    let (provider_b, signer_b) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "aggregation_guards".parse().expect("feed id");
    let (state, _, _, _) = oracle_state_with_accounts(&[provider_a.clone(), provider_b.clone()]);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let mut config = feed_config(
        feed_id.clone(),
        vec![provider_a.clone(), provider_b.clone()],
    );
    config.min_signers = 2;
    config.replay_window_slots = nonzero!(1_u64);
    execute_boxed(
        RegisterOracleFeed {
            feed: config.clone(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("register feed");

    let request_1 = Hash::new(b"quorum-request-1");
    SubmitOracleObservation {
        observation: observation(provider_a.clone(), &signer_a, &feed_id, 1, request_1, 10),
    }
    .execute(&provider_a, &mut stx)
    .expect("submit first observation");
    assert_rejects_with(
        AggregateOracleFeed {
            feed_id: feed_id.clone(),
            slot: 1,
            request_hash: request_1,
            evidence_hashes: Vec::new(),
        }
        .execute(&provider_a, &mut stx),
        "insufficient oracle quorum",
    );

    let (state, _, _, _) = oracle_state_with_accounts(&[provider_a.clone(), provider_b.clone()]);
    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    execute_boxed(
        RegisterOracleFeed {
            feed: config.clone(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("register fresh feed");

    let request_2 = Hash::new(b"quorum-request-2");
    for (provider, signer, value) in [
        (&provider_a, &signer_a, 20_i128),
        (&provider_b, &signer_b, 20_i128),
    ] {
        SubmitOracleObservation {
            observation: observation(provider.clone(), signer, &feed_id, 2, request_2, value),
        }
        .execute(provider, &mut stx)
        .expect("submit quorum observation");
    }
    AggregateOracleFeed {
        feed_id: feed_id.clone(),
        slot: 2,
        request_hash: request_2,
        evidence_hashes: Vec::new(),
    }
    .execute(&provider_a, &mut stx)
    .expect("aggregate quorum window");
    assert_rejects_with(
        AggregateOracleFeed {
            feed_id: feed_id.clone(),
            slot: 2,
            request_hash: request_2,
            evidence_hashes: Vec::new(),
        }
        .execute(&provider_a, &mut stx),
        "already processed",
    );

    let request_3 = Hash::new(b"quorum-request-3");
    aggregate_round(
        &mut stx,
        &feed_id,
        [(&provider_a, &signer_a), (&provider_b, &signer_b)],
        4,
        request_3,
        30,
        Vec::new(),
    );

    let stale_request = Hash::new(b"quorum-request-stale");
    assert_rejects_with(
        SubmitOracleObservation {
            observation: observation(
                provider_a.clone(),
                &signer_a,
                &feed_id,
                1,
                stale_request,
                12,
            ),
        }
        .execute(&provider_a, &mut stx),
        "stale oracle window",
    );
}

#[test]
fn oracle_dispute_requires_history_anchor_evidence_and_target_relevance() {
    let (challenger, challenger_signer) = iroha_test_samples::gen_account_in("validators");
    let (target, target_signer) = iroha_test_samples::gen_account_in("validators");
    let (uninvolved, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "dispute_anchor_guards".parse().expect("feed id");
    let (state, _, _, _) =
        oracle_state_with_accounts(&[challenger.clone(), target.clone(), uninvolved.clone()]);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let mut config = feed_config(
        feed_id.clone(),
        vec![challenger.clone(), target.clone(), uninvolved.clone()],
    );
    config.min_signers = 2;
    config.committee_size = 3;
    execute_boxed(
        RegisterOracleFeed {
            feed: config.clone(),
        },
        &challenger,
        &mut stx,
    )
    .expect("register feed");

    let request = Hash::new(b"dispute-anchor");
    assert_rejects_with(
        execute_boxed(
            OpenOracleDispute {
                feed_id: feed_id.clone(),
                slot: 10,
                request_hash: request,
                target: target.clone(),
                bond: None,
                evidence_hashes: Vec::new(),
                reason: "missing evidence".to_string(),
            },
            &challenger,
            &mut stx,
        ),
        "evidence must not be empty",
    );
    assert_rejects_with(
        execute_boxed(
            OpenOracleDispute {
                feed_id: feed_id.clone(),
                slot: 10,
                request_hash: request,
                target: target.clone(),
                bond: None,
                evidence_hashes: vec![Hash::new(b"evidence")],
                reason: "missing history".to_string(),
            },
            &challenger,
            &mut stx,
        ),
        "no retained oracle feed event",
    );

    aggregate_round(
        &mut stx,
        &feed_id,
        [(&challenger, &challenger_signer), (&target, &target_signer)],
        10,
        request,
        42,
        vec![Hash::new(b"bundle")],
    );
    assert_rejects_with(
        execute_boxed(
            OpenOracleDispute {
                feed_id,
                slot: 10,
                request_hash: request,
                target: uninvolved,
                bond: None,
                evidence_hashes: vec![Hash::new(b"evidence")],
                reason: "uninvolved provider".to_string(),
            },
            &challenger,
            &mut stx,
        ),
        "did not contribute",
    );
}

#[test]
fn oracle_change_votes_reject_missing_permission_and_stage_jumps() {
    let (provider, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "stage_guard_feed".parse().expect("feed id");
    let validator_domain_id: DomainId =
        DomainId::try_new("validators", "universal").expect("domain");
    let domain = Domain::new(validator_domain_id).build(&provider);
    let account = Account::new(provider.clone()).build(&provider);
    let mut world = World::with([domain], [account], []);
    grant_permission(
        &mut world,
        &provider,
        oracle_permission::CanProposeOracleChange,
    );
    let mut state = State::with_telemetry(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        test_telemetry(),
    );
    state.set_oracle(oracle_config());

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let change_id = OracleChangeId(Hash::new(b"stage-missing-permission"));
    execute_boxed(
        ProposeOracleChange {
            change_id,
            feed: feed_config(feed_id.clone(), vec![provider.clone()]),
            class: OracleChangeClass::Low,
            payload_hash: Hash::new(b"payload"),
            evidence_hashes: Vec::new(),
        },
        &provider,
        &mut stx,
    )
    .expect("propose change");
    assert_rejects_with(
        execute_boxed(
            VoteOracleChangeStage {
                change_id,
                stage: OracleChangeStage::Intake,
                approve: true,
                evidence_hashes: Vec::new(),
            },
            &provider,
            &mut stx,
        ),
        "does not have `CanVoteOracleChangeStage`",
    );

    let (provider, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "stage_jump_feed".parse().expect("feed id");
    let world = world_with_providers(std::slice::from_ref(&provider));
    let mut state = State::with_telemetry(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        test_telemetry(),
    );
    let mut cfg = oracle_config();
    cfg.governance.intake_min_votes = nonzero!(1_usize);
    cfg.governance.rules_min_votes = nonzero!(1_usize);
    state.set_oracle(cfg);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let change_id = OracleChangeId(Hash::new(b"stage-jump"));
    execute_boxed(
        ProposeOracleChange {
            change_id,
            feed: feed_config(feed_id, vec![provider.clone()]),
            class: OracleChangeClass::Low,
            payload_hash: Hash::new(b"payload"),
            evidence_hashes: Vec::new(),
        },
        &provider,
        &mut stx,
    )
    .expect("propose change");
    assert_rejects_with(
        execute_boxed(
            VoteOracleChangeStage {
                change_id,
                stage: OracleChangeStage::RulesCommittee,
                approve: true,
                evidence_hashes: Vec::new(),
            },
            &provider,
            &mut stx,
        ),
        "cannot vote future oracle change stage",
    );
    execute_boxed(
        VoteOracleChangeStage {
            change_id,
            stage: OracleChangeStage::Intake,
            approve: true,
            evidence_hashes: Vec::new(),
        },
        &provider,
        &mut stx,
    )
    .expect("advance intake");
    assert_rejects_with(
        execute_boxed(
            VoteOracleChangeStage {
                change_id,
                stage: OracleChangeStage::Intake,
                approve: true,
                evidence_hashes: Vec::new(),
            },
            &provider,
            &mut stx,
        ),
        "cannot vote past oracle change stage",
    );
}

#[test]
fn oracle_queries_return_feeds_history_stats_disputes_changes_and_bindings() {
    let (provider_a, signer_a) = iroha_test_samples::gen_account_in("validators");
    let (provider_b, signer_b) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "query_surface_feed".parse().expect("feed id");
    let (state, _, _, _) = oracle_state_with_accounts(&[provider_a.clone(), provider_b.clone()]);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let config = feed_config(
        feed_id.clone(),
        vec![provider_a.clone(), provider_b.clone()],
    );
    execute_boxed(
        RegisterOracleFeed {
            feed: config.clone(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("register feed");
    let request = Hash::new(b"query-request");
    aggregate_round(
        &mut stx,
        &feed_id,
        [(&provider_a, &signer_a), (&provider_b, &signer_b)],
        10,
        request,
        50,
        vec![Hash::new(b"query-bundle")],
    );
    execute_boxed(
        OpenOracleDispute {
            feed_id: feed_id.clone(),
            slot: 10,
            request_hash: request,
            target: provider_b.clone(),
            bond: None,
            evidence_hashes: vec![Hash::new(b"query-dispute")],
            reason: "query dispute".to_string(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("open dispute");
    let dispute_id = dispute_id(&feed_id, 10, request, &provider_a, &provider_b);
    let change_id = OracleChangeId(Hash::new(b"query-change"));
    let change_feed_id: FeedId = "query_change_feed".parse().expect("feed id");
    execute_boxed(
        ProposeOracleChange {
            change_id,
            feed: feed_config(
                change_feed_id.clone(),
                vec![provider_a.clone(), provider_b.clone()],
            ),
            class: OracleChangeClass::Low,
            payload_hash: Hash::new(b"query-change-payload"),
            evidence_hashes: Vec::new(),
        },
        &provider_a,
        &mut stx,
    )
    .expect("propose change");

    let twitter_kit = iroha_data_model::oracle::kits::twitter_follow_binding();
    let mut twitter_feed = twitter_kit.feed_config;
    twitter_feed.providers = vec![provider_a.clone()];
    twitter_feed.min_signers = 1;
    twitter_feed.committee_size = 1;
    twitter_feed.feed_id = TWITTER_FOLLOW_FEED_ID.parse().expect("feed id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"query-uaid"));
    let binding_hash = KeyedHash::new(
        defaults::oracle::twitter_binding_pepper_id(),
        defaults::oracle::twitter_binding_pepper_id(),
        b"query-user",
    );
    let attestation = twitter_binding_attestation(&twitter_feed, &uaid, binding_hash.clone(), 500);
    record_twitter_binding_round(
        &mut stx,
        &provider_a,
        &signer_a,
        &twitter_feed,
        &attestation,
    );

    stx.apply();
    sb.commit().expect("commit query state");

    let view = state.view();
    assert_eq!(
        FindOracleFeedById::new(feed_id.clone())
            .execute(&view)
            .expect("find feed")
            .feed_id,
        feed_id
    );
    assert!(
        FindOracleFeeds
            .execute(CompoundPredicate::PASS, &view)
            .expect("find feeds")
            .any(|feed| feed.feed_id == feed_id)
    );
    assert_eq!(
        FindOracleHistoryByFeedId::new(feed_id.clone())
            .execute(CompoundPredicate::PASS, &view)
            .expect("find history")
            .count(),
        1
    );
    let stats_key = OracleProviderKey::new(feed_id.clone(), provider_a.clone());
    assert!(
        FindOracleProviderStatsByKey::new(stats_key.clone())
            .execute(&view)
            .expect("find provider stats")
            .inliers
            > 0
    );
    assert!(
        FindOracleProviderStatsByFeedId::new(feed_id.clone())
            .execute(CompoundPredicate::PASS, &view)
            .expect("find provider stats by feed")
            .any(|record| record.key == stats_key)
    );
    assert_eq!(
        FindOracleDisputeById::new(dispute_id)
            .execute(&view)
            .expect("find dispute")
            .id,
        dispute_id
    );
    assert!(
        FindOracleDisputes
            .execute(CompoundPredicate::PASS, &view)
            .expect("find disputes")
            .any(|dispute| dispute.id == dispute_id)
    );
    assert!(
        FindOracleDisputesByFeedId::new(feed_id)
            .execute(CompoundPredicate::PASS, &view)
            .expect("find disputes by feed")
            .any(|dispute| dispute.id == dispute_id)
    );
    assert_eq!(
        FindOracleChangeById::new(change_id)
            .execute(&view)
            .expect("find change")
            .feed
            .feed_id,
        change_feed_id
    );
    assert!(
        FindOracleChanges
            .execute(CompoundPredicate::PASS, &view)
            .expect("find changes")
            .any(|change| change.id == change_id)
    );
    assert_eq!(
        FindTwitterBindingByHash::new(binding_hash.clone())
            .execute(&view)
            .expect("find twitter binding")
            .attestation
            .uaid,
        uaid
    );
    assert!(
        FindTwitterBindingsByUaid::new(uaid)
            .execute(CompoundPredicate::PASS, &view)
            .expect("find twitter bindings by uaid")
            .any(|binding| binding.attestation.binding_hash == binding_hash)
    );
}

fn dispute_id(
    feed_id: &FeedId,
    slot: FeedSlot,
    request_hash: Hash,
    challenger: &AccountId,
    target: &AccountId,
) -> iroha_data_model::oracle::OracleDisputeId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(feed_id.as_str().as_bytes());
    hasher.update(&slot.to_le_bytes());
    hasher.update(request_hash.as_ref());
    hasher.update(challenger.to_string().as_bytes());
    hasher.update(target.to_string().as_bytes());
    let mut buf = [0_u8; 8];
    buf.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
    iroha_data_model::oracle::OracleDisputeId(u64::from_le_bytes(buf))
}

fn asset_value(view: &impl WorldReadOnly, asset_id: &AssetId) -> Numeric {
    view.asset(asset_id)
        .unwrap_or_else(|_| panic!("missing asset {asset_id}"))
        .as_ref()
        .clone()
}

#[test]
fn oracle_applies_rewards_and_penalties() {
    let (provider_a, signer_a) = iroha_test_samples::gen_account_in("validators");
    let (provider_b, signer_b) = iroha_test_samples::gen_account_in("validators");
    let (provider_c, signer_c) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "price_xor_usd".parse().expect("feed id");

    let (state, asset_def_id, reward_pool, slash_receiver) =
        oracle_state_with_accounts(&[provider_a.clone(), provider_b.clone(), provider_c.clone()]);

    let mut config = feed_config(
        feed_id.clone(),
        vec![provider_a.clone(), provider_b.clone(), provider_c.clone()],
    );
    config.outlier_policy = OutlierPolicy::Absolute(AbsoluteOutlier { max_delta: 5 });

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    RegisterOracleFeed {
        feed: config.clone(),
    }
    .execute(&provider_a, &mut stx)
    .expect("register feed");

    let req = Hash::new(b"reward-penalty");
    SubmitOracleObservation {
        observation: observation(provider_a.clone(), &signer_a, &feed_id, 42, req, 10),
    }
    .execute(&provider_a, &mut stx)
    .expect("submit observation a");
    SubmitOracleObservation {
        observation: observation(provider_b.clone(), &signer_b, &feed_id, 42, req, 12),
    }
    .execute(&provider_b, &mut stx)
    .expect("submit observation b");
    SubmitOracleObservation {
        observation: observation(provider_c.clone(), &signer_c, &feed_id, 42, req, 100),
    }
    .execute(&provider_c, &mut stx)
    .expect("submit observation c");
    AggregateOracleFeed {
        feed_id: feed_id.clone(),
        slot: 42,
        request_hash: req,
        evidence_hashes: Vec::new(),
    }
    .execute(&provider_a, &mut stx)
    .expect("aggregate slot");
    stx.apply();
    sb.commit().expect("commit block");

    let view = state.view();
    let a_id = AssetId::new(asset_def_id.clone(), provider_a.clone());
    let b_id = AssetId::new(asset_def_id.clone(), provider_b.clone());
    let c_id = AssetId::new(asset_def_id.clone(), provider_c.clone());
    let pool_id = AssetId::new(asset_def_id.clone(), reward_pool.clone());
    let slash_id = AssetId::new(asset_def_id, slash_receiver.clone());

    assert_eq!(
        asset_value(view.world(), &a_id),
        Numeric::from_str("6").expect("numeric")
    );
    assert_eq!(
        asset_value(view.world(), &b_id),
        Numeric::from_str("6").expect("numeric")
    );
    assert_eq!(
        asset_value(view.world(), &c_id),
        Numeric::from_str("4").expect("numeric")
    );
    assert_eq!(
        asset_value(view.world(), &pool_id),
        Numeric::from_str("8").expect("numeric")
    );
    assert_eq!(
        asset_value(view.world(), &slash_id),
        Numeric::from_str("6").expect("numeric")
    );
}

#[test]
fn oracle_dispute_bond_and_resolution_flow() {
    let (challenger, challenger_signer) = iroha_test_samples::gen_account_in("validators");
    let (target, target_signer) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "price_xor_usd".parse().expect("feed id");

    let (state, asset_def_id, _reward_pool, slash_receiver) =
        oracle_state_with_accounts(&[challenger.clone(), target.clone()]);

    let config = feed_config(feed_id.clone(), vec![challenger.clone(), target.clone()]);
    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();

    RegisterOracleFeed {
        feed: config.clone(),
    }
    .execute(&challenger, &mut stx)
    .expect("register feed");

    let slot = 10;
    let request = Hash::new(b"dispute-request");
    aggregate_round(
        &mut stx,
        &feed_id,
        [(&challenger, &challenger_signer), (&target, &target_signer)],
        slot,
        request,
        42_000,
        vec![Hash::new(b"dispute-bundle")],
    );
    OpenOracleDispute {
        feed_id: feed_id.clone(),
        slot,
        request_hash: request,
        target: target.clone(),
        bond: None,
        evidence_hashes: vec![Hash::new(b"dispute-evidence")],
        reason: "outlier".to_string(),
    }
    .execute(&challenger, &mut stx)
    .expect("open dispute");

    let dispute = dispute_id(&feed_id, slot, request, &challenger, &target);
    ResolveOracleDispute {
        dispute_id: dispute,
        outcome: iroha_data_model::oracle::OracleDisputeOutcome::Upheld,
        notes: String::new(),
    }
    .execute(&challenger, &mut stx)
    .expect("resolve dispute");

    stx.apply();
    sb.commit().expect("commit block");

    let view = state.view();
    let challenger_id = AssetId::new(asset_def_id.clone(), challenger.clone());
    let target_id = AssetId::new(asset_def_id.clone(), target.clone());
    let slash_id = AssetId::new(asset_def_id, slash_receiver.clone());

    assert_eq!(
        asset_value(view.world(), &challenger_id),
        Numeric::from_str("7").expect("numeric")
    );
    assert_eq!(
        asset_value(view.world(), &target_id),
        Numeric::from_str("5").expect("numeric")
    );
    assert_eq!(
        asset_value(view.world(), &slash_id),
        Numeric::from_str("5").expect("numeric")
    );

    let dispute_record = view
        .world()
        .oracle_disputes()
        .get(&dispute)
        .expect("dispute recorded");
    assert!(matches!(
        dispute_record.status,
        iroha_data_model::oracle::OracleDisputeStatus::Resolved(
            iroha_data_model::oracle::OracleDisputeOutcome::Upheld
        )
    ));
}

#[test]
fn oracle_change_pipeline_applies_feed() {
    let (provider, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "change_price_feed".parse().expect("feed id");
    let world = world_with_providers(std::slice::from_ref(&provider));
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let telemetry = test_telemetry();
    let mut state = State::with_telemetry(world, kura, query, telemetry);

    let mut cfg = oracle_config();
    cfg.governance.intake_min_votes = nonzero!(1_usize);
    cfg.governance.rules_min_votes = nonzero!(1_usize);
    cfg.governance.cop_min_votes = OracleChangeThresholds {
        low: nonzero!(1_usize),
        medium: nonzero!(1_usize),
        high: nonzero!(1_usize),
    };
    cfg.governance.technical_min_votes = nonzero!(1_usize);
    cfg.governance.policy_jury_min_votes = OracleChangeThresholds {
        low: nonzero!(1_usize),
        medium: nonzero!(1_usize),
        high: nonzero!(1_usize),
    };
    state.set_oracle(cfg);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let feed = feed_config(feed_id.clone(), vec![provider.clone()]);
    let change_id = OracleChangeId(Hash::new(b"change-ok"));

    ProposeOracleChange {
        change_id,
        feed: feed.clone(),
        class: OracleChangeClass::High,
        payload_hash: Hash::new(b"payload-1"),
        evidence_hashes: vec![Hash::new(b"evidence-intake")],
    }
    .execute(&provider, &mut stx)
    .expect("propose change");

    for stage in [
        OracleChangeStage::Intake,
        OracleChangeStage::RulesCommittee,
        OracleChangeStage::CopReview,
        OracleChangeStage::TechnicalAudit,
        OracleChangeStage::PolicyJury,
        OracleChangeStage::Enactment,
    ] {
        VoteOracleChangeStage {
            change_id,
            stage,
            approve: true,
            evidence_hashes: Vec::new(),
        }
        .execute(&provider, &mut stx)
        .expect("stage vote");
    }

    stx.apply();
    sb.commit().expect("commit block");

    let view = state.view();
    let stored = view
        .world()
        .oracle_feeds()
        .get(&feed_id)
        .cloned()
        .expect("feed enacted");
    assert_eq!(stored.feed_config_version, feed.feed_config_version);

    let change = view
        .world()
        .oracle_changes()
        .get(&change_id)
        .cloned()
        .expect("change persisted");
    assert!(matches!(change.status, OracleChangeStatus::Enacted(_)));
    let enact = change
        .stages
        .iter()
        .find(|stage| stage.stage == OracleChangeStage::Enactment)
        .expect("enact stage present");
    assert!(enact.completed_at.is_some());
}

#[test]
fn oracle_change_deadline_rolls_back() {
    let (provider, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = "change_deadline_feed".parse().expect("feed id");
    let world = world_with_providers(std::slice::from_ref(&provider));
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let telemetry = test_telemetry();
    let mut state = State::with_telemetry(world, kura, query, telemetry);

    let mut cfg = oracle_config();
    cfg.governance.intake_sla_blocks = 1;
    cfg.governance.rules_sla_blocks = 1;
    cfg.governance.cop_sla_blocks = 1;
    cfg.governance.technical_sla_blocks = 1;
    cfg.governance.policy_jury_sla_blocks = 1;
    cfg.governance.enact_sla_blocks = 1;
    state.set_oracle(cfg);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    let change_id = OracleChangeId(Hash::new(b"change-deadline"));
    ProposeOracleChange {
        change_id,
        feed: feed_config(feed_id, vec![provider.clone()]),
        class: OracleChangeClass::Low,
        payload_hash: Hash::new(b"payload-2"),
        evidence_hashes: Vec::new(),
    }
    .execute(&provider, &mut stx)
    .expect("propose change");
    stx.apply();
    sb.commit().expect("commit proposal block");

    let sb2 = state.block(header(3));
    sb2.commit().expect("commit sweep block");

    let view = state.view();
    let change = view
        .world()
        .oracle_changes()
        .get(&change_id)
        .cloned()
        .expect("change persisted");
    assert!(matches!(
        &change.status,
        OracleChangeStatus::Failed(failure)
            if matches!(failure.reason, iroha_data_model::oracle::OracleChangeStageFailure::DeadlineMissed)
    ));
}

fn twitter_binding_attestation(
    feed_config: &FeedConfig,
    uaid: &UniversalAccountId,
    binding_hash: KeyedHash,
    observed_at_ms: u64,
) -> TwitterBindingAttestation {
    let cadence = feed_config.cadence_slots.get();
    let base_slot: FeedSlot = 42;
    let slot = if base_slot.is_multiple_of(cadence) {
        base_slot
    } else {
        base_slot + (cadence - (base_slot % cadence))
    };
    TwitterBindingAttestation {
        binding_hash,
        uaid: *uaid,
        status: TwitterBindingStatus::Following,
        tweet_id: Some("tweet-1".to_string()),
        challenge_hash: None,
        expires_at_ms: observed_at_ms + 600_000,
        observed_at_ms,
        request_hash: Hash::new(b"req-twitter"),
        slot,
        feed_config_version: feed_config.feed_config_version,
    }
}

fn twitter_binding_observation(
    provider: &AccountId,
    signer: &KeyPair,
    feed: &FeedConfig,
    attestation: &TwitterBindingAttestation,
) -> Observation {
    let body = ObservationBody {
        feed_id: feed.feed_id.clone(),
        feed_config_version: feed.feed_config_version,
        slot: attestation.slot,
        provider_id: provider.clone(),
        connector_id: feed.connector_id.clone(),
        connector_version: feed.connector_version,
        request_hash: attestation.request_hash,
        outcome: ObservationOutcome::Value(attestation.observation_value()),
        timestamp_ms: Some(attestation.observed_at_ms),
    };
    let signature = SignatureOf::new(signer.private_key(), &body);
    Observation { body, signature }
}

#[test]
fn record_twitter_binding_updates_registry_and_index() {
    let (provider, signer) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = TWITTER_FOLLOW_FEED_ID.parse().expect("feed id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-123"));

    let (state, _, _, _) = oracle_state_with_accounts(std::slice::from_ref(&provider));

    let kit = iroha_data_model::oracle::kits::twitter_follow_binding();
    let mut feed_config = kit.feed_config;
    feed_config.providers = vec![provider.clone()];
    feed_config.min_signers = 1;
    feed_config.committee_size = 1;
    feed_config.feed_id = feed_id.clone();
    let binding_hash = KeyedHash::new(
        defaults::oracle::twitter_binding_pepper_id(),
        defaults::oracle::twitter_binding_pepper_id(),
        b"user-123",
    );
    let attestation = twitter_binding_attestation(&feed_config, &uaid, binding_hash.clone(), 1_000);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    RegisterOracleFeed {
        feed: feed_config.clone(),
    }
    .execute(&provider, &mut stx)
    .expect("register feed");

    SubmitOracleObservation {
        observation: twitter_binding_observation(&provider, &signer, &feed_config, &attestation),
    }
    .execute(&provider, &mut stx)
    .expect("submit twitter binding observation");
    AggregateOracleFeed {
        feed_id: feed_id.clone(),
        slot: attestation.slot,
        request_hash: attestation.request_hash,
        evidence_hashes: Vec::new(),
    }
    .execute(&provider, &mut stx)
    .expect("aggregate twitter binding slot");

    RecordTwitterBinding {
        attestation: attestation.clone(),
        feed_id: feed_id.clone(),
    }
    .execute(&provider, &mut stx)
    .expect("record twitter binding");
    stx.apply();
    sb.commit().expect("commit block");

    let view = state.view();
    let stored = view
        .world()
        .twitter_bindings()
        .get(&binding_hash.digest)
        .cloned()
        .expect("stored binding");
    assert_eq!(stored.attestation.uaid, uaid);
    assert_eq!(stored.attestation.status, TwitterBindingStatus::Following);
    let by_uaid = view
        .world()
        .twitter_bindings_by_uaid()
        .get(&uaid)
        .cloned()
        .expect("uaid index");
    assert!(by_uaid.contains(&binding_hash.digest));
}

#[test]
fn record_twitter_binding_rejects_expired_and_duplicates_and_allows_revoke() {
    let (provider, signer) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = TWITTER_FOLLOW_FEED_ID.parse().expect("feed id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-456"));

    let (state, _, _, _) = oracle_state_with_accounts(std::slice::from_ref(&provider));

    let kit = iroha_data_model::oracle::kits::twitter_follow_binding();
    let mut feed_config = kit.feed_config;
    feed_config.providers = vec![provider.clone()];
    feed_config.min_signers = 1;
    feed_config.committee_size = 1;
    feed_config.feed_id = feed_id.clone();
    let binding_hash = KeyedHash::new(
        defaults::oracle::twitter_binding_pepper_id(),
        defaults::oracle::twitter_binding_pepper_id(),
        b"user-456",
    );
    let base_attestation =
        twitter_binding_attestation(&feed_config, &uaid, binding_hash.clone(), 5_000);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    record_twitter_binding_round(
        &mut stx,
        &provider,
        &signer,
        &feed_config,
        &base_attestation,
    );
    stx.apply();
    sb.commit().expect("commit block");

    // Expired attestation rejected.
    let mut sb = state.block(header(2));
    let mut stx = sb.transaction();
    let mut expired = base_attestation.clone();
    expired.expires_at_ms = expired.observed_at_ms;
    assert!(
        RecordTwitterBinding {
            attestation: expired,
            feed_id: feed_id.clone(),
        }
        .execute(&provider, &mut stx)
        .is_err()
    );

    // Duplicate within spacing rejected.
    let mut duplicate = base_attestation.clone();
    duplicate.observed_at_ms += 1;
    assert!(
        RecordTwitterBinding {
            attestation: duplicate,
            feed_id: feed_id.clone(),
        }
        .execute(&provider, &mut stx)
        .is_err()
    );

    // Revoke removes registry entries.
    RevokeTwitterBinding {
        binding_hash: binding_hash.clone(),
        reason: "duplicate user report".to_string(),
    }
    .execute(&provider, &mut stx)
    .expect("revoke binding");
    stx.apply();
    sb.commit().expect("commit revoke block");

    let view = state.view();
    assert!(
        view.world()
            .twitter_bindings()
            .get(&binding_hash.digest)
            .is_none()
    );
}
