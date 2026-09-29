#[test]
fn network_defaults_carry_maximal_sumeragi_frames() {
    // Default `sync_max_bytes` (16 MiB) plus the Sumeragi `FRAME_OVERHEAD` (64 KiB): the largest
    // sync response a peer with default local settings sends (`specs/sumeragi.md` §9.4).
    const MAX_DEFAULT_SYNC_FRAME_BYTES: usize = 16 * 1024 * 1024 + 64 * 1024;
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
        defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get() > MAX_DEFAULT_SYNC_FRAME_BYTES,
        "the block-sync frame cap must carry a maximal default Sumeragi sync response"
    );
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES_CONTROL.get(),
        2 * 1024 * 1024,
        "Sumeragi control frames (votes, certificates, timeouts, status) use the consensus-safety control topic"
    );
}
