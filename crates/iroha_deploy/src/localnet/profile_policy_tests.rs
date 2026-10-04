#[test]
fn sora_localnet_profiles_never_restrict_the_universal_dataspace() {
    for profile in [
        SoraProfile::Dataspace,
        SoraProfile::PrivateSbp,
        SoraProfile::PrivateCbuae,
        SoraProfile::PrivateBpng,
        SoraProfile::Nexus,
    ] {
        let (_, lanes) = localnet_lane_catalog(Some(profile), false, TairaParentCatalog::WithIs)
            .expect("Sora localnet profile should define a lane catalog");
        for lane in lanes {
            let lane = lane.as_table().expect("lane catalog entry");
            if lane.get("dataspace").and_then(toml::Value::as_str) != Some("universal") {
                continue;
            }
            assert_eq!(
                lane.get("visibility").and_then(toml::Value::as_str),
                Some("public"),
                "{profile:?} must not emit a restricted lane for the universal dataspace"
            );
        }
    }
}
