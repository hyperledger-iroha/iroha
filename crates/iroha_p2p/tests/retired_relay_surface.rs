//! Guards the first-release P2P surface against reintroducing retired transport paths.
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
fn retired_classical_noise_surface_is_absent() {
    let manifest = include_str!("../Cargo.toml");
    let peer = include_str!("../src/peer.rs");
    let transport = include_str!("../src/transport.rs");
    assert!(
        !manifest.contains("noise_handshake"),
        "the classical Noise handshake feature must stay retired"
    );
    assert!(
        manifest.contains("[target.'cfg(any())'.dependencies]\nsnow = \"0.10\""),
        "the protected Snow lockfile pin must remain impossible to select"
    );
    assert!(
        !peer.contains("HandshakeNoise"),
        "the application handshake must not regain a Noise variant"
    );
    assert!(
        !transport.contains("pub mod noise"),
        "the transport must not regain a classical Noise implementation"
    );
}

#[test]
fn mandatory_first_release_transport_has_no_opt_out_features() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.contains("p2p_tls"),
        "mandatory TLS must not become a selectable Cargo feature"
    );
    assert!(
        !manifest.contains("p2p_bounded_queues"),
        "bounded P2P queues must not become a selectable Cargo feature"
    );
    for retired_identity_feature in ["gost", "sm"] {
        assert!(
            !manifest
                .lines()
                .any(|line| line.starts_with(&format!("{retired_identity_feature} ="))),
            "P2P node identity is fixed to BLS-normal; {retired_identity_feature} must not return"
        );
    }
    for dependency in ["rcgen", "rustls", "tokio-rustls"] {
        let dependency_prefix = format!("{dependency} = ");
        let declaration = manifest
            .lines()
            .find(|line| line.starts_with(&dependency_prefix))
            .unwrap_or_else(|| panic!("missing mandatory {dependency} dependency"));
        assert!(
            !declaration.contains("optional = true"),
            "{dependency} must remain unconditional"
        );
    }
}
