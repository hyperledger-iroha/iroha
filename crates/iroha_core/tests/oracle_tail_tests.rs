#[test]
fn record_twitter_binding_rejects_version_ttl_and_non_provider_revoke() {
    let (provider, signer) = iroha_test_samples::gen_account_in("validators");
    let (outsider, _) = iroha_test_samples::gen_account_in("validators");
    let feed_id: FeedId = TWITTER_FOLLOW_FEED_ID.parse().expect("feed id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-ttl-negative"));

    let (state, _, _, _) = oracle_state_with_accounts(&[provider.clone(), outsider.clone()]);

    let kit = iroha_data_model::oracle::kits::twitter_follow_binding();
    let mut feed_config = kit.feed_config;
    feed_config.providers = vec![provider.clone()];
    feed_config.min_signers = 1;
    feed_config.committee_size = 1;
    feed_config.feed_id = feed_id.clone();
    let binding_hash = KeyedHash::new(
        defaults::oracle::twitter_binding_pepper_id(),
        defaults::oracle::twitter_binding_pepper_id(),
        b"user-ttl-negative",
    );
    let attestation = twitter_binding_attestation(&feed_config, &uaid, binding_hash.clone(), 5_000);

    let mut sb = state.block(header(1));
    let mut stx = sb.transaction();
    execute_boxed(
        RegisterOracleFeed {
            feed: feed_config.clone(),
        },
        &provider,
        &mut stx,
    )
    .expect("register twitter feed");

    let mut wrong_version = attestation.clone();
    wrong_version.feed_config_version = FeedConfigVersion(2);
    assert_rejects_with(
        execute_boxed(
            RecordTwitterBinding {
                attestation: wrong_version,
                feed_id: feed_id.clone(),
            },
            &provider,
            &mut stx,
        ),
        "does not match registered version",
    );

    let mut too_long = attestation.clone();
    too_long.expires_at_ms =
        too_long.observed_at_ms + defaults::oracle::twitter_binding_max_ttl_ms() + 1;
    assert_rejects_with(
        execute_boxed(
            RecordTwitterBinding {
                attestation: too_long,
                feed_id: feed_id.clone(),
            },
            &provider,
            &mut stx,
        ),
        "exceeds max",
    );

    let mut too_short = attestation.clone();
    too_short.expires_at_ms =
        too_short.observed_at_ms + defaults::oracle::twitter_binding_min_ttl_ms() - 1;
    assert_rejects_with(
        execute_boxed(
            RecordTwitterBinding {
                attestation: too_short,
                feed_id: feed_id.clone(),
            },
            &provider,
            &mut stx,
        ),
        "below min",
    );

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
        attestation,
        feed_id,
    }
    .execute(&provider, &mut stx)
    .expect("record binding");

    assert_rejects_with(
        execute_boxed(
            RevokeTwitterBinding {
                binding_hash,
                reason: "outsider revoke".to_string(),
            },
            &outsider,
            &mut stx,
        ),
        "is not part of feed",
    );
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
