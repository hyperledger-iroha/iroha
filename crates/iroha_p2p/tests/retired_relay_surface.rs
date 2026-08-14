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
#[test]
fn retired_classical_noise_surface_is_unreachable() {
    let manifest = include_str!("../Cargo.toml");
    let peer = include_str!("../src/peer.rs");
    let transport = include_str!("../src/transport.rs");
    let (_, feature_and_dev_sections) = manifest
        .split_once("[features]")
        .expect("P2P manifest must declare its feature inventory");
    assert!(
        !manifest.contains("noise_handshake"),
        "the classical Noise handshake feature must stay retired"
    );
    assert!(
        !feature_and_dev_sections.contains("snow"),
        "the locked Snow package must not be selectable by any crate feature"
    );
    if manifest.contains("snow =") {
        assert!(
            manifest.contains("[target.'cfg(any())'.dependencies]\nsnow = \"0.10\""),
            "until lockfile regeneration removes it, Snow may exist only behind an always-false target"
        );
    }
    assert!(
        !peer.contains("HandshakeNoise"),
        "the application handshake must not regain a Noise variant"
    );
    assert!(
        !transport.contains("pub mod noise"),
        "the transport must not regain a classical Noise implementation"
    );
}
