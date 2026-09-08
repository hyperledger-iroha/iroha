//! Source-bound identity shared by running validators and local release preparation.

use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::parameter::system::ConsensusHandshakeMetadata;

/// Source commit embedded in this Core build; absence is not release provenance.
#[must_use]
pub const fn source_commit() -> Option<&'static str> {
    option_env!("GIT_COMMIT_HASH")
}

/// The exact build fingerprint published by the Sumeragi adapter.
#[must_use]
pub fn build_fingerprint() -> Hash {
    let mut bytes = env!("CARGO_PKG_VERSION").as_bytes().to_vec();
    bytes.extend_from_slice(source_commit().unwrap_or("unknown").as_bytes());
    Hash::new(bytes)
}

/// Verify canonical signed genesis bytes against the explicitly selected genesis key.
/// Returns the actual block hash and its validated signed consensus metadata.
pub fn genesis_identity(
    bytes: &[u8],
    public_key: &PublicKey,
) -> eyre::Result<(Hash, ConsensusHandshakeMetadata)> {
    let block = iroha_genesis::decode_signed_genesis(bytes)?;
    if !block.header().is_genesis() || block.encode_wire()?.as_slice() != bytes {
        eyre::bail!("release genesis is not canonical framed Norito");
    }
    crate::validate_genesis_block(
        &block,
        &iroha_data_model::account::AccountId::new(public_key.clone()),
    )?;
    let metadata = iroha_genesis::signed_genesis_consensus_metadata(&block)?;
    Ok((Hash::from(block.hash()), metadata))
}
