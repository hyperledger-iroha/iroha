//! Funded reward custody and claim ownership across repeated beneficiary rekeys.
use super::*;
use iroha_data_model::{IntoKeyValue, fee_evidence::FeeEvidencePayloadV1};
#[test]
fn recovery_preserves_reserved_and_delayed_rewards_through_repeated_rekeys() {
    crate::retail_fee_tests::fixture(1_793_451_600_000, |stx, policy| {
        let (_, binding) = super::super::tests::network_xor_claim_fixture(stx, policy);
        let old = super::super::tests::account(2);
        let middle = super::super::tests::account(3);
        let latest = super::super::tests::account(4);
        let stranger = super::super::tests::account(5);
        let period = earning_month(stx.block_unix_timestamp_ms() - 1).unwrap();
        let weights = BTreeMap::from([(old.clone(), 3)]);
        let treasury = AssetId::new(
            binding.ds_asset_id.clone(),
            binding.treasury_account_id.clone(),
        );
        let (_, treasury_balance) =
            Asset::new(treasury.clone(), quantity(200, 2).unwrap()).into_key_value();
        stx.world.assets.insert(treasury, treasury_balance);
        write(stx, service_key(&binding, period).unwrap(), &weights).unwrap();
        write(stx, pending_key(&binding, period).unwrap(), &200u64).unwrap();
        save_state(
            stx,
            &binding,
            &ValidationFeeRewardsState {
                pending_sbd_total: 200,
                ..Default::default()
            },
        )
        .unwrap();
        reserve_conversion(
            stx,
            &binding,
            &ConversionOffer {
                earning_period_start_ms: period,
                sbd_minor: 100,
                min_xor_minor: 100,
                sequence: 0,
            },
            100,
        )
        .unwrap();
        let pool = AssetId::new(
            binding.xor_asset_id.clone(),
            binding.reward_pool_account_id.clone(),
        );
        let (_, value) = Asset::new(pool.clone(), quantity(200, 9).unwrap()).into_key_value();
        stx.world.assets.insert(pool.clone(), value);
        let allocated_key = state_key(&binding, "Allocation/0").unwrap();
        let original_allocation = stx
            .world
            .smart_contract_state
            .get(&allocated_key)
            .unwrap()
            .clone();
        rekey_beneficiary(stx, &old, &middle).unwrap();
        assert_eq!(root(stx, &binding, &middle).unwrap(), old);
        assert_eq!(
            read::<u128>(stx, &claimable_key(&binding, &old).unwrap()).unwrap(),
            Some(100)
        );
        assert!(
            super::super::tests::claim_current_fee_credit(stx, &old, binding.validator_lane_id)
                .is_err()
        );
        assert!(rekey_beneficiary(stx, &old, &stranger).is_err());
        assert!(rekey_beneficiary(stx, &stranger, &middle).is_err());
        assert_eq!(
            owner(stx, &binding, &old).unwrap().unwrap().account_id,
            middle
        );
        super::super::tests::claim_current_fee_credit(stx, &middle, binding.validator_lane_id)
            .unwrap();
        assert_eq!(read_state(stx, &binding).unwrap().reserved_xor, 0);
        // A later conversion still uses the unchanged pre-recovery earning account.
        reserve_conversion(
            stx,
            &binding,
            &ConversionOffer {
                earning_period_start_ms: period,
                sbd_minor: 100,
                min_xor_minor: 100,
                sequence: 1,
            },
            100,
        )
        .unwrap();
        rekey_beneficiary(stx, &middle, &latest).unwrap();
        assert_eq!(root(stx, &binding, &latest).unwrap(), old);
        assert_eq!(owner(stx, &binding, &old).unwrap().unwrap().revision, 2);
        assert!(
            super::super::tests::claim_current_fee_credit(stx, &middle, binding.validator_lane_id)
                .is_err()
        );
        super::super::tests::claim_current_fee_credit(stx, &latest, binding.validator_lane_id)
            .unwrap();
        super::super::tests::claim_current_fee_credit(stx, &latest, binding.validator_lane_id)
            .unwrap();
        let state = read_state(stx, &binding).unwrap();
        assert_eq!(
            (
                state.pending_sbd_total,
                state.reserved_xor,
                state.next_allocation,
                state.next_claim
            ),
            (0, 0, 2, 2)
        );
        assert_eq!(
            stx.world.smart_contract_state.get(&allocated_key).unwrap(),
            &original_allocation
        );
        assert_eq!(service_weights(stx, &binding, period).unwrap(), weights);
        for account in [&middle, &latest] {
            let paid = AssetId::new(binding.xor_asset_id.clone(), account.clone());
            assert_eq!(
                minor_units(stx.world.assets.get(&paid).unwrap().as_ref(), 9).unwrap(),
                100
            );
        }
        let corpus = pending_fee_evidence_records(stx).unwrap();
        let claims = corpus
            .iter()
            .filter_map(|r| match &r.payload {
                FeeEvidencePayloadV1::RewardClaim(c) => Some(c),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(claims.len(), 2);
        assert_eq!(claims[0].beneficiary_id, old);
        assert_eq!(
            (
                claims[0].beneficiary_revision,
                claims[1].beneficiary_revision
            ),
            (1, 2)
        );
        assert!(corpus.iter().any(|r| matches!(&r.payload, FeeEvidencePayloadV1::RewardBeneficiaryRevision(revision) if revision.account_id == middle && revision.revision == 1)), "claim before same-block rekey retains its exact earlier owner source");
        assert!(corpus.iter().any(|r| matches!(&r.payload, FeeEvidencePayloadV1::RewardAllocation(a) if a.sequence == 1 && a.beneficiaries.get(&old) == Some(&old))));
    });
}
