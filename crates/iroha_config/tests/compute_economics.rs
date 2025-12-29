//! Tests for compute economics governance bounds and sponsor caps.

use std::{collections::BTreeMap, num::NonZeroU64, str::FromStr};

use iroha_config::parameters::{actual::ComputeEconomics, defaults};
use iroha_data_model::{
    compute::{ComputeGovernanceError, ComputePriceWeights},
    name::Name,
};

fn default_price_families() -> BTreeMap<Name, ComputePriceWeights> {
    defaults::compute::price_families()
}

fn default_economics() -> ComputeEconomics {
    ComputeEconomics {
        max_cu_per_call: defaults::compute::max_cu_per_call(),
        max_amplification_ratio: defaults::compute::max_amplification_ratio(),
        fee_split: defaults::compute::fee_split(),
        sponsor_policy: defaults::compute::sponsor_policy(),
        price_bounds: defaults::compute::price_bounds(),
        price_risk_classes: defaults::compute::price_risk_classes(),
        price_family_baseline: default_price_families(),
        price_amplifiers: defaults::compute::price_amplifiers(),
    }
}

#[test]
fn price_update_respects_bounds() {
    let mut families = default_price_families();
    let economics = default_economics();
    let family = Name::from_str("default").expect("family");

    // Within the balanced bounds (15%).
    let ok_weights = ComputePriceWeights {
        cycles_per_unit: NonZeroU64::new(1_100_000).expect("cycles"),
        egress_bytes_per_unit: NonZeroU64::new(1100).expect("egress"),
        unit_label: "cu".to_string(),
    };
    economics
        .apply_price_update(&family, ok_weights.clone(), &mut families)
        .expect("within bounds");
    assert_eq!(Some(&ok_weights), families.get(&family));

    // Exceeds the balanced bounds for cycles.
    let err = economics
        .apply_price_update(
            &family,
            ComputePriceWeights {
                cycles_per_unit: NonZeroU64::new(2_000_000).expect("cycles"),
                egress_bytes_per_unit: NonZeroU64::new(1024).expect("egress"),
                unit_label: "cu".to_string(),
            },
            &mut families,
        )
        .expect_err("delta should be rejected");
    assert!(matches!(
        err,
        ComputeGovernanceError::CyclesDeltaExceeded { .. }
    ));
}

#[test]
fn sponsor_caps_enforced() {
    let economics = default_economics();

    economics
        .validate_sponsor_allocation(5_000)
        .expect("under cap");
    let err = economics
        .validate_sponsor_allocation(20_000)
        .expect_err("over cap");
    assert!(matches!(
        err,
        ComputeGovernanceError::SponsorCapExceeded {
            requested: 20_000,
            limit: 10_000,
        }
    ));
}
