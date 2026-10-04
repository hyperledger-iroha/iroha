//! Codec and immutable-claim tests; all checkpoint/history fields here are explicitly unproved.

use super::*;
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    sorafs::{
        pin_registry::StorageClass,
        reserve::{
            RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveDuration, ReserveLifecycleStage,
            ReservePolicyV1, ReserveProviderTermsV1, ReserveTier,
        },
    },
    transaction::FeePaymentIntent,
};
use std::{collections::BTreeMap, time::Duration};

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    )
}
fn options() -> BoundedTransactionOptions {
    BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::new(),
        deadline: Instant::now() + Duration::from_secs(300),
    }
}
fn codec_original() -> Original {
    let policy = ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: AssetDefinitionId::parse_address_literal(
            crate::genesis::profile::TAIRA_XOR_ASSET_DEFINITION_ID,
        )
        .unwrap(),
        custody_account: account(1),
        treasury_account: account(2),
        operations_authority: account(3),
        decision_authority: account(4),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: XorQuantity::try_from_micro(1_000_000_000).unwrap(),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    };
    let partition = ReserveProviderAccountV1 {
        terms: ReserveProviderTermsV1 {
            provider_id: ProviderId::new([7; 32]),
            provider_account: account(3),
            tier: ReserveTier::TierA,
            storage_class: StorageClass::Hot,
            duration: ReserveDuration::Monthly,
            capacity_gib: 1,
        },
        policy_digest: policy.digest().unwrap(),
        revision: 2,
        reserve_balance: XorQuantity::zero(),
        debt_principal: XorQuantity::zero(),
        accrued_interest: XorQuantity::zero(),
        credit_cap: XorQuantity::zero(),
        lifecycle_stage: ReserveLifecycleStage::Warning,
        days_past_due: 0,
        pending_movements: 1,
        open_appeals: 0,
        rent_charged_through_unix: 1,
        interest_accrued_at_unix: 1,
        updated_at_unix: 1,
    };
    // These are serialized claims only; no conversion to a private historical type exists.
    let history = HistoryClaim {
        transaction_hash: HashOf::from_untyped_unchecked(Hash::new(b"unproved request")),
        height: 5,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"unproved carrier")),
        block_time_ms: 1234,
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"unproved genesis",
        ))),
        provider_id: partition.terms.provider_id,
        provider_account: partition.terms.provider_account.clone(),
        movement_id: [8; 32],
        amount: XorQuantity::try_from_micro(1_000_000).unwrap(),
        requested_provider_revision: 1,
        policy_digest: policy.digest().unwrap(),
    };
    Original {
        selection: ReserveMovementDecisionSelection {
            chain_id: "codec-only-unproved-chain".into(),
            network_id: history.network_id,
            provider_id: history.provider_id,
            provider_account: history.provider_account.clone(),
            expected_provider_revision: partition.revision,
            partition_policy_digest: partition.policy_digest,
            policy_digest: policy.digest().unwrap(),
            asset_definition: policy.asset_definition.clone(),
            custody_account: policy.custody_account.clone(),
            treasury_account: policy.treasury_account.clone(),
            operations_authority: policy.operations_authority.clone(),
            decision_authority: policy.decision_authority.clone(),
        },
        history,
        policy,
        partition,
        rationale: " Exact original approval 理由 ".into(),
        checkpoint: vec![0x5a; 16 * 1024],
    }
}

#[test]
fn approval_claim_journal_roundtrip_keeps_exact_request_snapshot_rationale_and_original_terms() {
    let terms = Terms::new(now_ms().unwrap() + 600_000, &options()).unwrap();
    let term_bytes = encode(&terms, 16 * 1024).unwrap();
    let restored_terms: Terms = norito::decode_canonical_with_limits(
        &term_bytes,
        norito::DecodeLimits::new(4096, 16 * 1024, 16 * 1024, 64 * 1024, 16),
    )
    .unwrap();
    assert!(
        restored_terms == terms,
        "dispatch terms retain their own exact canonical record"
    );

    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("approval")).unwrap();
    let original = codec_original();
    publish_intent(&directory, &original).unwrap();
    let bytes = directory.read("original.nrt", MAX_ORIGINAL_BYTES).unwrap();
    let restored = read_intent(&directory).unwrap().unwrap();
    assert!(restored.history == original.history);
    assert_eq!(
        encode(&restored, MAX_ORIGINAL_BYTES).unwrap(),
        bytes.as_slice()
    );
    let mut replacement = original.clone();
    replacement.checkpoint.push(0x6b);
    assert!(publish_intent(&directory, &replacement).is_err());
    let request = restored.request(&terms, Instant::now() + Duration::from_secs(900));
    assert!(request.approve);
    assert_eq!(request.movement_id, original.history.movement_id);
    assert_eq!(request.rationale, original.rationale);
    assert_eq!(request.deadline_unix_ms, terms.signing_deadline_unix_ms);
    assert_eq!(request.policy, original.policy);
    assert_eq!(request.partition, original.partition);
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        bytes.as_slice()
    );
}

#[test]
fn approval_claim_journal_rejects_malformed_bounds_roles_cas_and_changed_intent() {
    let terms = Terms::new(now_ms().unwrap() + 600_000, &options()).unwrap();
    let original = codec_original();
    original.validate().unwrap();
    for field in 0..13 {
        let mut changed = original.clone();
        match field {
            0 => changed.rationale.clear(),
            1 => changed.rationale = "x".repeat(RESERVE_MAX_REASON_BYTES_V1 + 1),
            2 => changed.selection.chain_id = "x".repeat(MAX_SELECTION_BYTES + 1),
            3 => changed.checkpoint.clear(),
            4 => changed.selection.expected_provider_revision += 1,
            5 => changed.partition.pending_movements = 0,
            6 => changed.selection.partition_policy_digest[0] ^= 1,
            7 => changed.selection.policy_digest[0] ^= 1,
            8 => changed.selection.decision_authority = account(12),
            9 => changed.history.provider_account = account(13),
            10 => {
                changed.history.network_id = NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"other codec network")),
                )
            }
            11 => changed.history.requested_provider_revision = changed.partition.revision,
            _ => changed.partition.terms.capacity_gib = 0,
        }
        assert!(changed.validate().is_err(), "malformed claim field {field}");
    }
    let options = terms.options(Instant::now() + Duration::from_secs(600));
    for field in 0..5 {
        let mut changed = original.intent();
        match field {
            0 => changed.rationale.push(' '),
            1 => changed.policy.grace_period_days += 1,
            2 => changed.partition.terms.capacity_gib += 1,
            3 => {
                changed.partition.revision += 1;
                changed.expected_provider_revision += 1;
            }
            _ => changed.partition.reserve_balance = XorQuantity::try_from_micro(1).unwrap(),
        }
        assert!(
            original
                .matches_intent(&changed)
                .and_then(|()| terms.matches(terms.requested_deadline_unix_ms, &options))
                .is_err()
        );
    }
}

#[test]
fn expired_approval_claims_keep_original_utc_fees_and_lagged_partition_digest() {
    let mut terms = Terms::new(now_ms().unwrap() + 600_000, &options()).unwrap();
    let mut original = codec_original();
    let now = now_ms().unwrap();
    // This is codec-only validation; genuine real-time expiry is a separate native fixture.
    terms.requested_deadline_unix_ms = now - 100;
    terms.signing_deadline_unix_ms = now - 200;
    original.policy.revision = 2;
    original.policy.predecessor_policy_digest = Some(original.selection.policy_digest);
    original.selection.policy_digest = original.policy.digest().unwrap();
    assert_ne!(
        original.selection.policy_digest,
        original.partition.policy_digest
    );
    original.validate().unwrap();
    let options = terms.options(Instant::now() + Duration::from_secs(900));
    original
        .matches_intent(&original.intent())
        .and_then(|()| terms.matches(terms.requested_deadline_unix_ms, &options))
        .unwrap();
    assert!(terms.signing_deadline(options.deadline).is_err());
    assert_eq!(
        original.request(&terms, options.deadline).deadline_unix_ms,
        now - 200
    );
    assert!(
        original
            .matches_intent(&original.intent())
            .and_then(|()| terms.matches(now + 600_000, &options))
            .is_err()
    );
    let mut changed = options.clone();
    changed.max_total_fees.insert(
        original.policy.asset_definition.clone(),
        iroha_primitives::numeric::Quantity::from(1_u32),
    );
    assert!(
        original
            .matches_intent(&original.intent())
            .and_then(|()| terms.matches(terms.requested_deadline_unix_ms, &changed))
            .is_err()
    );
    changed = options;
    changed.fee_payment = FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1));
    assert!(
        original
            .matches_intent(&original.intent())
            .and_then(|()| terms.matches(terms.requested_deadline_unix_ms, &changed))
            .is_err()
    );
    terms.requested_deadline_unix_ms = u64::MAX;
    assert!(terms.validate().is_err());
}
