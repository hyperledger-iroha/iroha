#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_p2p::tests::integration::p2p::ConsensusMessage")]
#[derive(Clone, Debug, Decode, Encode)]
struct ConsensusMessage(u32);
impl iroha_p2p::network::message::ClassifyTopic for ConsensusMessage {
    // This synthetic type has no availability or recovery variants. Positive
    // empty-class bounds fund mandatory transport geometry only for this fixture.
    fn availability_frame_maximum(
        _: &iroha_model_base::peer::PeerId,
    ) -> Result<usize, norito::core::Error> {
        Ok(1)
    }
    fn recovery_frame_maxima(
        _: &iroha_model_base::peer::PeerId,
    ) -> Result<[usize; 2], norito::core::Error> {
        Ok([1, 1])
    }
    fn inbound_topic(payload: &[u8], flags: u8) -> Result<Option<Topic>, norito::core::Error> {
        fixed_fixture_topic::<Self>(payload, flags)
    }
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        iroha_p2p::network::message::Topic::Consensus
    }
    fn progress_reconstruction(&self) -> iroha_p2p::network::message::ProgressReconstruction {
        // Tests retain this scalar request until exact delivery. Replaying the
        // same value has no stateful effect in the synthetic subscriber.
        iroha_p2p::network::message::ProgressReconstruction::Retransmit
    }
}
fn setup_logger() {
    test_logger();
}
fn default_soranet_handshake() -> ActualSoranetHandshake {
    // Keep admission inexpensive so general timing tests continue to measure
    // the behavior they name. `test_network_config` isolates replay state.
    super::low_cost_test_soranet_handshake()
}
#[test]
fn test_encryption() {
    use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};
    const TEST_KEY: [u8; 32] = [
        5, 87, 82, 183, 220, 57, 107, 49, 227, 4, 96, 231, 198, 88, 153, 11, 22, 65, 56, 45, 237,
        35, 231, 165, 122, 153, 14, 68, 13, 84, 5, 24,
    ];
    let encryptor =
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(TEST_KEY).expect("valid key length");
    let message = b"Some ciphertext";
    let aad = b"Iroha3 AAD";
    let ciphertext = encryptor
        .encrypt_easy(aad.as_ref(), message.as_ref())
        .unwrap();
    let decrypted = encryptor
        .decrypt_easy(aad.as_ref(), ciphertext.as_slice())
        .unwrap();
    assert_eq!(decrypted.as_slice(), message);
}

#[test]
fn scalar_fixture_raw_topics_match_every_declared_layout() {
    use norito::core;
    for requested in [
        0,
        core::header_flags::COMPACT_LEN,
        core::header_flags::PACKED_STRUCT | core::header_flags::COMPACT_LEN,
        core::header_flags::PACKED_STRUCT
            | core::header_flags::COMPACT_LEN
            | core::header_flags::FIELD_BITSET,
    ] {
        for chan in [0, 1, 2, 3, u8::MAX] {
            let value = MultiTopic {
                chan,
                payload: u32::MAX,
            };
            let (bytes, flags) = {
                let _flags = core::DecodeFlagsGuard::enter(requested);
                norito::codec::encode_with_header_flags(&value)
            };
            assert!(bytes.len() <= 29);
            assert_eq!(
                MultiTopic::inbound_topic(&bytes, flags).unwrap(),
                Some(value.topic())
            );
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(MultiTopic::inbound_topic(&trailing, flags).is_err());
            assert!(MultiTopic::inbound_topic(&bytes[..bytes.len() - 1], flags).is_err());
        }
        let value = ConsensusMessage(u32::MAX);
        let (bytes, flags) = {
            let _flags = core::DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&value)
        };
        assert_eq!(
            ConsensusMessage::inbound_topic(&bytes, flags).unwrap(),
            Some(Topic::Consensus)
        );
        assert!(ConsensusMessage::inbound_topic(&bytes[..bytes.len() - 1], flags).is_err());
        let text = TestMessage("bounded classification before charged string decode".to_owned());
        let (bytes, flags) = {
            let _flags = core::DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&text)
        };
        assert_eq!(
            TestMessage::inbound_topic(&bytes, flags).unwrap(),
            Some(Topic::Other)
        );
        assert!(TestMessage::inbound_topic(&[], flags).is_err());
    }
    assert!(MultiTopic::inbound_topic(&[0; 30], 0).is_err());
}
