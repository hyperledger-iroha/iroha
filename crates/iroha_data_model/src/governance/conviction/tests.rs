//! Exact public conviction arithmetic, update monotonicity and wire roundtrips.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};

fn policy(scale: u32) -> PlainConvictionPolicyV1 {
    let account = |seed| {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    };
    PlainConvictionPolicyV1 {
        asset_definition_id: AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("voting", "universal").unwrap(),
            "bond".parse().unwrap(),
        ),
        asset_scale: scale,
        conviction_step_blocks: 100,
        max_conviction: 6,
        approval_threshold_numerator: 1,
        approval_threshold_denominator: 2,
        minimum_turnout: 0,
        minimum_bond: Quantity::zero(),
        bond_escrow_account: account(1),
        slash_receiver_account: account(2),
    }
}

#[test]
fn units_use_frozen_scale_and_never_round_or_overflow() {
    let p = policy(2);
    assert_eq!(p.units(&"1.25".parse().unwrap()), Ok(125));
    assert_eq!(p.units(&"1.00".parse().unwrap()), Ok(100));
    assert_eq!(p.units(&Quantity::from(1_u32)), Ok(100));
    assert_eq!(
        p.units(&"0.001".parse().unwrap()),
        Err(ConvictionErrorV1::FractionalUnits)
    );
    assert_eq!(policy(0).units(&Quantity::from(u128::MAX)), Ok(u128::MAX));
    assert_eq!(
        policy(1).units(&Quantity::from(u128::MAX)),
        Err(ConvictionErrorV1::Overflow)
    );
    assert_eq!(
        policy(28).units(&"0.0000000000000000000000000001".parse().unwrap()),
        Ok(1)
    );
    assert_eq!(
        policy(28).units(&Quantity::from(1_u32)),
        Ok(10_u128.pow(28))
    );
    assert_eq!(
        policy(29).units(&Quantity::zero()),
        Err(ConvictionErrorV1::InvalidPolicy)
    );
    let too_wide: Quantity = "340282366920938463463374607431768211456".parse().unwrap();
    assert_eq!(policy(0).units(&too_wide), Err(ConvictionErrorV1::Overflow));
}

#[test]
fn exact_weight_uses_floor_sqrt_and_wide_capped_factor() {
    let p = policy(2);
    assert_eq!(p.weight(&"1.25".parse().unwrap(), 200), Ok(33));
    assert_eq!(p.weight(&"1.44".parse().unwrap(), 200), Ok(36));
    assert_eq!(p.weight(&Quantity::zero(), 200), Ok(0));
    assert_eq!(p.weight(&Quantity::from(1_u32), u64::MAX), Ok(60));
    let mut p = policy(0);
    p.conviction_step_blocks = 1;
    p.max_conviction = u64::MAX;
    assert_eq!(
        p.weight(&Quantity::from(u128::MAX), u64::MAX),
        Ok(u128::from(u64::MAX) * u128::from(u64::MAX))
    );
    for n in [0, 1, 2, 3, 4, 15, 16, 17, u128::MAX] {
        let root = integer_sqrt(n);
        assert!(root.checked_mul(root).unwrap() <= n);
        assert!(
            (root + 1)
                .checked_mul(root + 1)
                .is_none_or(|square| square > n)
        );
    }
}

#[test]
fn policy_rejects_invalid_scale_factor_and_minimum() {
    for (scale, step, maximum) in [(29, 1, 1), (0, 0, 1), (0, 1, 0)] {
        let mut p = policy(scale);
        p.conviction_step_blocks = step;
        p.max_conviction = maximum;
        assert_eq!(p.validate(), Err(ConvictionErrorV1::InvalidPolicy));
    }
    let mut p = policy(2);
    p.minimum_bond = "0.001".parse().unwrap();
    assert_eq!(p.validate(), Err(ConvictionErrorV1::FractionalUnits));
}

#[test]
fn update_requires_a_real_increase_and_never_reduces_duration() {
    let old = Quantity::from(100_u32);
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &old, 200, 201),
        Err(ConvictionErrorV1::UnchangedLock)
    );
    // H=101 with the same H=201 absolute expiry used to overwrite duration 200 with 100.
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &old, 100, 201),
        Err(ConvictionErrorV1::ReducedLock)
    );
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &101_u32.into(), 100, 201),
        Err(ConvictionErrorV1::ReducedLock)
    );
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &old, 199, 300),
        Err(ConvictionErrorV1::ReducedLock)
    );
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &99_u32.into(), 200, 301),
        Err(ConvictionErrorV1::ReducedLock)
    );
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &101_u32.into(), 200, 200),
        Err(ConvictionErrorV1::ReducedLock)
    );
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &101_u32.into(), 200, 201),
        Ok(())
    );
    assert_eq!(
        validate_conviction_update_v1(&old, 200, 201, &old, 201, 202),
        Ok(())
    );
}

#[test]
fn frozen_context_has_exact_norito_and_json_roundtrips() {
    for context in [
        PlainVotingContextV1::NotApplicable,
        PlainVotingContextV1::Conviction(policy(2)),
    ] {
        let frame = norito::encode_canonical(&context).unwrap();
        let decoded: PlainVotingContextV1 = norito::decode_canonical(&frame).unwrap();
        assert_eq!(decoded, context);
        let json = norito::json::to_json(&context).unwrap();
        let decoded: PlainVotingContextV1 = norito::json::from_str(&json).unwrap();
        assert_eq!(decoded, context);
    }
    assert!(norito::json::from_str::<PlainVotingContextV1>("null").is_err());
    assert!(norito::json::from_str::<PlainVotingContextV1>("{}").is_err());
}

#[test]
fn frozen_context_and_result_reject_unrecognized_json_fields() {
    for context in [
        PlainVotingContextV1::NotApplicable,
        PlainVotingContextV1::Conviction(policy(2)),
    ] {
        let mut value = norito::json::to_value(&context).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("retired_policy".into(), true.into());
        assert!(norito::json::from_value::<PlainVotingContextV1>(value).is_err());
    }
    for result in [
        PlainVotingResultV1::Pending,
        PlainVotingResultV1::NotApplicable,
        PlainVotingResultV1::Decided(policy(0).decide([9, 1, 0]).unwrap()),
    ] {
        let mut value = norito::json::to_value(&result).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("retired_tally".into(), true.into());
        assert!(norito::json::from_value::<PlainVotingResultV1>(value).is_err());
    }
}

#[test]
fn frozen_decisions_check_threshold_turnout_and_full_width_products() {
    let mut p = policy(0);
    assert!(!p.decide([0, 0, 0]).unwrap().approved);
    assert!(p.decide([5, 5, 0]).unwrap().approved);
    assert!(!p.decide([4, 6, 0]).unwrap().approved);
    p.minimum_turnout = 11;
    assert!(!p.decide([5, 5, 0]).unwrap().approved);
    assert!(p.decide([5, 5, 1]).unwrap().approved);
    p.approval_threshold_numerator = u64::MAX;
    p.approval_threshold_denominator = u64::MAX;
    assert!(p.decide([u128::MAX, 0, 0]).unwrap().approved);
    assert!(!p.decide([u128::MAX - 1, 1, 0]).unwrap().approved);
    assert_eq!(
        p.decide([u128::MAX, 0, 1]),
        Err(ConvictionErrorV1::Overflow)
    );
    p.approval_threshold_denominator = 0;
    assert_eq!(p.validate(), Err(ConvictionErrorV1::InvalidPolicy));
    for result in [
        PlainVotingResultV1::Pending,
        PlainVotingResultV1::NotApplicable,
        PlainVotingResultV1::Decided(policy(0).decide([9, 1, 2]).unwrap()),
    ] {
        let decoded: PlainVotingResultV1 =
            norito::decode_canonical(&norito::encode_canonical(&result).unwrap()).unwrap();
        assert_eq!(decoded, result);
        let decoded: PlainVotingResultV1 =
            norito::json::from_str(&norito::json::to_json(&result).unwrap()).unwrap();
        assert_eq!(decoded, result);
    }
}
