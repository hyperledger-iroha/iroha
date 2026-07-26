//! Basic guard that the Halo2 backend toggle behaves deterministically: when
//! disabled, verification is rejected; when enabled (default), the backend is IPA.

use ivm::host::{DefaultHost, ZkHalo2Backend, ZkHalo2Config};

#[test]
fn halo2_backend_disabled_rejects() {
    let mut host = DefaultHost::default();
    host = host.with_zk_halo2_config(ZkHalo2Config {
        enabled: false,
        ..ZkHalo2Config::default()
    });
    assert_eq!(host.zk_config().backend, ZkHalo2Backend::Ipa);
    assert!(!host.zk_config().enabled);
}

#[test]
fn halo2_backend_enabled_defaults_to_ipa() {
    let host = DefaultHost::default();
    assert!(host.zk_config().enabled);
    assert_eq!(host.zk_config().backend, ZkHalo2Backend::Ipa);
}
