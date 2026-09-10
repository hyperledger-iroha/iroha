//! Tests for compute economics governance bounds and sponsor caps.
use iroha_config::parameters::{actual::ComputeEconomics, defaults, user};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use iroha_data_model::compute::{
    ComputeGovernanceError, ComputePriceRiskClass, ComputePriceWeights,
};
use iroha_model_base::name::Name;
use std::{collections::BTreeMap, num::NonZeroU64, str::FromStr};
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
fn price_bounds_read_risk_class_object_keys() {
    let table = r"
[price_bounds.Low]
max_cycles_delta_bps = 500
max_egress_delta_bps = 600
[price_bounds.Balanced]
max_cycles_delta_bps = 1500
max_egress_delta_bps = 1600
[price_bounds.High]
max_cycles_delta_bps = 2500
max_egress_delta_bps = 2600
"
    .parse()
    .expect("price bounds TOML");
    let economics = ConfigReader::new()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<user::ComputeEconomics>()
        .expect("risk class keys must deserialize through ConfigReader");
    assert_eq!(economics.price_bounds.len(), 3);
    for (class, cycles, egress) in [
        (ComputePriceRiskClass::Low, 500, 600),
        (ComputePriceRiskClass::Balanced, 1500, 1600),
        (ComputePriceRiskClass::High, 2500, 2600),
    ] {
        let bounds = &economics.price_bounds[&class];
        assert_eq!(bounds.max_cycles_delta_bps.get(), cycles);
        assert_eq!(bounds.max_egress_delta_bps.get(), egress);
    }
}
#[test]
fn price_bounds_reject_noncanonical_risk_class_object_keys() {
    for key in ["Unknown", "low", "balanced", "high", "LOW", " Low", "Low "] {
        let table = format!(
            r#"
[price_bounds."{key}"]
max_cycles_delta_bps = 500
max_egress_delta_bps = 600
"#
        )
        .parse()
        .expect("price bounds TOML");
        let error = ConfigReader::new()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<user::ComputeEconomics>()
            .expect_err("noncanonical risk class keys must fail");
        assert!(format!("{error:?}").contains("price_bounds"), "key {key:?}");
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
