//! SoraNet relay library shared by the daemon and supporting tooling.
#![allow(unexpected_cfgs)]
pub mod capability;
pub mod circuit;
pub mod compliance;
pub mod config;
pub mod congestion;
pub mod constant_rate;
pub mod directory;
pub mod dos;
pub mod error;
pub mod exit;
pub mod guard;
pub mod incentive_log;
pub mod incentives;
pub mod metrics;
pub mod popctl;
pub mod privacy;
#[cfg(feature = "runtime")]
pub mod runtime;
pub mod scheduler;
pub mod token_tool;
pub mod vpn;
pub mod vpn_adapter;
pub(crate) fn canonical_remote_ip(remote: std::net::SocketAddr) -> std::net::IpAddr {
    match remote.ip() {
        std::net::IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map_or(std::net::IpAddr::V6(address), std::net::IpAddr::V4),
        address => address,
    }
}
pub(crate) fn checked_ed25519_verifying_key_from_bytes(
    public_key: &[u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
) -> Result<ed25519_dalek::VerifyingKey, String> {
    if public_key.iter().all(|byte| *byte == 0) {
        return Err("public key material must not be all zero".to_string());
    }
    let parsed =
        iroha_crypto::ed25519_parse_public_key(public_key).map_err(|err| err.to_string())?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(parsed.as_bytes())
        .map_err(|err| err.to_string())?;
    if verifying_key.is_weak() {
        return Err("public key is small-order (weak); rejected".to_string());
    }
    Ok(verifying_key)
}
#[cfg(test)]
mod tests {
    use super::*;
    const SMALL_ORDER_ED25519_POINT: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_ED25519_IDENTITY: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    const NONCANONICAL_NON_SMALL_ORDER_ED25519_POINT: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    #[test]
    fn checked_ed25519_verifying_key_rejects_inert_weak_or_noncanonical_material() {
        let all_zero = checked_ed25519_verifying_key_from_bytes(&[0; 32])
            .expect_err("all-zero Ed25519 key must be rejected");
        assert!(
            all_zero.contains("all zero"),
            "unexpected error: {all_zero}"
        );
        let weak = checked_ed25519_verifying_key_from_bytes(&SMALL_ORDER_ED25519_POINT)
            .expect_err("small-order Ed25519 key must be rejected");
        assert!(weak.contains("small-order"), "unexpected error: {weak}");
        let noncanonical = checked_ed25519_verifying_key_from_bytes(&NONCANONICAL_ED25519_IDENTITY)
            .expect_err("noncanonical Ed25519 key must be rejected");
        assert!(
            noncanonical.contains("non-canonical"),
            "unexpected error: {noncanonical}"
        );
        let noncanonical =
            checked_ed25519_verifying_key_from_bytes(&NONCANONICAL_NON_SMALL_ORDER_ED25519_POINT)
                .expect_err("noncanonical non-small-order Ed25519 key must be rejected");
        assert!(
            noncanonical.contains("non-canonical"),
            "unexpected error: {noncanonical}"
        );
    }
}
