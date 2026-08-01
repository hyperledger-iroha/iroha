// Handshake-state admission and Noise session-binding tests.

#[cfg(feature = "noise_handshake")]
use std::sync::Arc;

#[cfg(feature = "noise_handshake")]
use iroha_crypto::encryption::ChaCha20Poly1305;

use super::*;

fn consensus_caps(fingerprint: [u8; 32]) -> ConsensusConfigCaps {
    ConsensusConfigCaps {
        execution_policy_hash: [0xB0; 32],
        nexus_policy_digest: [0xC1; 32],
        v2_config_fingerprint: fingerprint,
        ivm_gas_schedule_hash: [0xD2; 32],
    }
}

#[test]
fn v2_peer_admission_compares_canonical_shared_config_fingerprint() {
    let expected = consensus_caps([0xA5; 32]);
    assert_eq!(
        consensus_config_mismatch(&expected, &expected),
        None,
        "identical canonical admission digests must be accepted",
    );

    let changed = consensus_caps([0x5A; 32]);
    let mismatch = consensus_config_mismatch(&expected, &changed)
        .expect("different shared v2 config hashes must be rejected");
    assert!(mismatch.contains("v2_config_fingerprint mismatch"));
    assert!(mismatch.contains(&hex_bytes(&[0xA5; 32])));
    assert!(mismatch.contains(&hex_bytes(&[0x5A; 32])));
}

#[cfg(feature = "noise_handshake")]
#[tokio::test(flavor = "current_thread")]
async fn noise_handshake_derives_shared_disambiguator() {
    let soranet = Arc::new(SoranetHandshakeConfig::defaults());
    let key_pair_a = KeyPair::random();
    let key_pair_b = KeyPair::random();
    let addr_a: SocketAddr = "127.0.0.1:10001".parse().unwrap();
    let addr_b: SocketAddr = "127.0.0.1:10002".parse().unwrap();

    let (stream_a, stream_b) = tokio::io::duplex(2048);
    let (read_a, write_a) = tokio::io::split(stream_a);
    let (read_b, write_b) = tokio::io::split(stream_b);

    let outbound = ConnectedTo {
        our_public_address: addr_a,
        expected_peer_id: iroha_data_model::prelude::PeerId::from(key_pair_b.public_key().clone()),
        key_pair: key_pair_a,
        connection: Connection::from_split(1, read_a, write_a),
        chain_id: iroha_data_model::ChainId::from("test-chain"),
        consensus_caps: None,
        confidential_caps: None,
        crypto_caps: None,
        soranet_handshake: soranet.clone(),
        local_scion_supported: true,
        trust_gossip: true,
        relay_role: RelayRole::Disabled,
    };
    let inbound = ConnectedFrom {
        our_public_address: addr_b,
        key_pair: key_pair_b,
        connection: Connection::from_split(2, read_b, write_b),
        chain_id: iroha_data_model::ChainId::from("test-chain"),
        consensus_caps: None,
        confidential_caps: None,
        crypto_caps: None,
        soranet_handshake: soranet.clone(),
        local_scion_supported: true,
        trust_gossip: true,
        relay_role: RelayRole::Disabled,
    };

    let (out_res, in_res) = tokio::join!(
        ConnectedTo::send_client_hello::<ChaCha20Poly1305>(outbound),
        ConnectedFrom::read_client_hello::<ChaCha20Poly1305>(inbound),
    );
    let outbound = out_res.expect("outbound handshake");
    let inbound = in_res.expect("inbound handshake");

    assert_eq!(
        outbound.cryptographer.disambiguator, inbound.cryptographer.disambiguator,
        "noise handshake must yield a shared disambiguator"
    );
    assert_eq!(
        outbound.cryptographer.session_binding, inbound.cryptographer.session_binding,
        "noise handshake must yield the same full identity-session binding"
    );
}
