//! Fresh-generation provisioning for the sole native attestation custody owner.
//!
//! Retained profile validation compares the original signed policy commitment only. It never
//! takes the live daemon's history lock or treats local files as current native authority.
use super::*;
use sorafs_node::{
    provider_attestation_journal::MusubiProviderAttestationJournalPolicyV1,
    provider_attestation_native::NativeMusubiProviderAttestationCustodyV1,
};

pub(super) fn policy() -> MusubiProviderAttestationJournalPolicyV1 {
    MusubiProviderAttestationJournalPolicyV1::default()
}
impl GeneratedAuthorities {
    /// Called only while creating the original unpublished Global generation.
    pub(in crate::localnet) fn initialize_native_attestation(
        &self,
        generation: &Path,
        storage: &Path,
        genesis: HashOf<BlockHeader>,
        peer_index: usize,
    ) -> Result<()> {
        let original = self
            .providers
            .get(peer_index)
            .ok_or_else(|| eyre!("generated provider peer is absent"))?;
        let selected = policy();
        ensure!(
            storage == super::super::LocalnetPeerStoragePaths::new(generation, peer_index).sorafs
                && original.provider.attestation_policy_digest() == selected.digest()?,
            "generated native attestation storage or original policy differs"
        );
        custody::ensure_directory(storage)?;
        NativeMusubiProviderAttestationCustodyV1::initialize(
            storage,
            NetworkId::from_genesis_hash(genesis),
            original.provider_id,
            selected,
        )
        .wrap_err("initialize original generated native attestation custody")
    }
}

#[cfg(test)]
mod tests;
