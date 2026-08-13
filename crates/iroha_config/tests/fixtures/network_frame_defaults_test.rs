#[test]
fn network_defaults_carry_maximal_sumeragi_v2_progress_frames() {
    const MAX_CERTIFIED_BODY_RESPONSE_BYTES: usize = 16_811_581;
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES.get(),
        17 * 1024 * 1024 + defaults::network::DEFAULT_AEAD_FRAME_OVERHEAD_BYTES
    );
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES_CONSENSUS,
        defaults::network::MAX_PLAINTEXT_FRAME_BYTES
    );
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC,
        defaults::network::MAX_PLAINTEXT_FRAME_BYTES
    );
    assert!(
        defaults::network::MAX_FRAME_BYTES.get() > MAX_CERTIFIED_BODY_RESPONSE_BYTES,
        "the encrypted frame cap must retain room for the P2P wrapper and AEAD overhead"
    );
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES_CONTROL.get(),
        2 * 1024 * 1024,
        "consensus-safety proposals and timeout certificates use the control topic"
    );
}
