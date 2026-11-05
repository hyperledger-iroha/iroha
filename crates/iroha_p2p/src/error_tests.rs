//! Diagnostic text, formatting, source and conversion contracts for P2P errors.

use super::{Algorithm, Error, KeyPair, SoranetTransportDelegationError};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::NetworkId;
use iroha_model_base::peer::PeerId;
use std::{error::Error as _, fmt::Write as _, io, net::AddrParseError, sync::Arc};

fn cases() -> [(Error, String, bool); 24] {
    let expected_network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"expected")));
    let found_network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"found")));
    let expected_peer = PeerId::new(
        KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    let found_peer = PeerId::new(
        KeyPair::try_from_seed(vec![2; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    [
        (
            Error::Io(Arc::new(io::Error::other("private cause"))),
            "Failed IO operation".into(),
            true,
        ),
        (
            Error::BindListener {
                listen_addr: "[::1]:1337 (config)".into(),
                public_address: "example.test:1337".into(),
                error: Arc::new(io::Error::other("address occupied")),
            },
            "Failed to bind TCP listener for configured addresses `[::1]:1337 (config)` / `example.test:1337`: address occupied".into(),
            true,
        ),
        (
            Error::Format,
            "Message improperly formatted".into(),
            false,
        ),
        (
            Error::Field,
            "Field is not defined for a peer at this stage".into(),
            false,
        ),
        (
            Error::NoritoCodec(norito::codec::Error::InvalidMagic),
            "Norito codec error".into(),
            true,
        ),
        (
            Error::Keys(iroha_crypto::error::Error::Other("private cause".into())),
            "Failed to create keys".into(),
            true,
        ),
        (
            Error::SymmetricEncryption(iroha_crypto::encryption::Error::NotEnoughData),
            "Symmetric encryption has failed".into(),
            true,
        ),
        (
            Error::Addr("invalid".parse::<std::net::SocketAddr>().unwrap_err()),
            "Failed to parse socket address".into(),
            true,
        ),
        (
            Error::ConnectionResetByPeer,
            "Connection reset by peer in the middle of message transfer".into(),
            false,
        ),
        (
            Error::FrameTooLarge,
            "Encrypted P2P frame exceeds the runtime cap or stream `u32` wire-body limit".into(),
            false,
        ),
        (
            Error::OutboundFrameQueueFull {
                priority: "consensus",
                queued_bytes: 1234,
                max_bytes: 1200,
                queued_frames: 7,
                max_frames: 6,
            },
            "Outbound consensus frame queue full (7/6 frames, 1234/1200 bytes)".into(),
            false,
        ),
        (
            Error::MalformedPayloadFrame,
            "Authenticated decrypted frame carried a malformed inner payload; the peer must disconnect".into(),
            false,
        ),
        (
            Error::InboundTopicCapExceeded,
            "Decrypted frame exceeded the cap selected from its raw inbound topic".into(),
            false,
        ),
        (
            Error::HandshakeBadPreface,
            "Handshake preface header invalid".into(),
            false,
        ),
        (
            Error::HandshakeNetworkMismatch {
                expected: expected_network,
                found: found_network,
            },
            format!("Peer handshake belongs to another exact network (expected {expected_network}, found {found_network})"),
            false,
        ),
        (
            Error::HandshakeConsensusMismatch {
                reason: "mode=permissioned, received=public\né".into(),
            },
            "Peer consensus handshake mismatch (mode=permissioned, received=public\né)".into(),
            false,
        ),
        (
            Error::HandshakeConfidentialMismatch,
            "Peer confidential handshake mismatch (`enabled/assume_valid/backend`)".into(),
            false,
        ),
        (
            Error::HandshakeCryptoMismatch,
            "Peer crypto handshake mismatch (`sm_enabled/sm_openssl_preview`)".into(),
            false,
        ),
        (
            Error::HandshakePeerMismatch {
                expected: expected_peer.clone(),
                found: found_peer.clone(),
            },
            format!("Unexpected peer identity during handshake (expected {expected_peer}, found {found_peer})"),
            false,
        ),
        (
            Error::HandshakeNodeAlgorithmMismatch {
                found: Algorithm::Ed25519,
            },
            "Node identity handshakes require BLS-normal, but `Ed25519` was supplied locally".into(),
            false,
        ),
        (
            Error::HandshakeMessageTooLarge,
            "Handshake metadata exceeds the maximum supported length (`u16::MAX` bytes)".into(),
            false,
        ),
        (
            Error::HandshakePublicKeyMalformed(iroha_crypto::error::ParseError::Invalid(
                "private cause".into(),
            )),
            "Local peer public key is malformed during handshake".into(),
            true,
        ),
        (
            Error::HandshakeSoranet("private cause".into()),
            "`SoraNet` handshake negotiation failed.".into(),
            false,
        ),
        (
            Error::HandshakeSoranetDelegation(SoranetTransportDelegationError::UnsupportedVersion {
                expected: 1,
                found: 255,
            }),
            "`SoraNet` transport identity delegation failed: unsupported SoraNet transport delegation version 255; expected 1".into(),
            true,
        ),
    ]
}

#[test]
fn error_display_preserves_all_variant_messages_and_outer_flags() {
    for (error, expected, has_source) in cases() {
        for rendered in [
            error.to_string(),
            format!("{error:>256}"),
            format!("{error:*^256}"),
            format!("{error:.3}"),
            format!("{error:+#030.3}"),
        ] {
            assert_eq!(rendered, expected, "{error:?}");
        }
        assert_eq!(error.source().is_some(), has_source, "{error:?}");
    }
}

#[test]
fn error_display_propagates_writer_failures() {
    struct RejectWrites;
    impl std::fmt::Write for RejectWrites {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    for (error, _, _) in cases() {
        assert!(write!(&mut RejectWrites, "{error}").is_err(), "{error:?}");
    }
}

#[test]
fn error_sources_and_from_conversions_preserve_types() {
    let error = Error::from(io::Error::other("io source"));
    assert!(matches!(error, Error::Io(_)));
    assert!(error.source().unwrap().is::<Arc<io::Error>>());
    assert_eq!(error.source().unwrap().to_string(), "io source");

    let io_source = Arc::new(io::Error::other("bind source"));
    let error = Error::BindListener {
        listen_addr: "[::1]:1337".into(),
        public_address: "example.test:1337".into(),
        error: io_source.clone(),
    };
    assert!(Arc::ptr_eq(
        error
            .source()
            .unwrap()
            .downcast_ref::<Arc<io::Error>>()
            .unwrap(),
        &io_source,
    ));

    let error = Error::from(norito::codec::Error::InvalidMagic);
    assert!(matches!(error, Error::NoritoCodec(_)));
    assert!(error.source().unwrap().is::<norito::codec::Error>());

    let error = Error::from(iroha_crypto::error::Error::Other("key source".into()));
    assert!(matches!(error, Error::Keys(_)));
    assert!(error.source().unwrap().is::<iroha_crypto::error::Error>());

    let error = Error::from(iroha_crypto::encryption::Error::NotEnoughData);
    assert!(matches!(error, Error::SymmetricEncryption(_)));
    assert!(
        error
            .source()
            .unwrap()
            .is::<iroha_crypto::encryption::Error>()
    );

    let error = Error::from("invalid".parse::<std::net::SocketAddr>().unwrap_err());
    assert!(matches!(error, Error::Addr(_)));
    assert!(error.source().unwrap().is::<AddrParseError>());

    let error = Error::HandshakePublicKeyMalformed(iroha_crypto::error::ParseError::Invalid(
        "parse source".into(),
    ));
    assert!(
        error
            .source()
            .unwrap()
            .is::<iroha_crypto::error::ParseError>()
    );
    assert_eq!(error.source().unwrap().to_string(), "parse source");

    let error = Error::from(SoranetTransportDelegationError::EmptyFrame);
    assert!(matches!(error, Error::HandshakeSoranetDelegation(_)));
    assert!(
        error
            .source()
            .unwrap()
            .is::<SoranetTransportDelegationError>()
    );
}
