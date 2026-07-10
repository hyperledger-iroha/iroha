//! Guards the first-release P2P surface against reintroducing a non-functional relay feature.

#[test]
fn retired_relay_stub_surface_is_absent() {
    let manifest = include_str!("../Cargo.toml");
    let transport = include_str!("../src/transport.rs");
    let retired_feature = ["p2p", "turn"].join("_");
    let retired_environment_variable = ["P2P_", "TU", "RN"].concat();
    let retired_module = ["pub mod ", "tu", "rn"].concat();

    assert!(
        !manifest.contains(&retired_feature),
        "the retired relay-stub Cargo feature must not be reintroduced"
    );
    assert!(
        !transport.contains(&retired_environment_variable),
        "the retired relay-stub environment variable must not be reintroduced"
    );
    assert!(
        !transport.contains(&retired_module),
        "the retired relay-stub transport module must not be reintroduced"
    );
}
