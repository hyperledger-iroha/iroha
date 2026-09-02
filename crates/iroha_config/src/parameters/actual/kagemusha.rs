/// Canonical files needed to authenticate and load one Kagemusha V1 proof release.
#[derive(Debug, Clone)]
pub struct KagemushaV1ProofReleaseFiles {
    /// Canonical Norito release manifest.
    pub manifest: PathBuf,
    /// Canonical Norito internal qualification receipt.
    pub validation_receipt: PathBuf,
    /// Canonical Norito locally trusted release-authority policy.
    pub authority_policy: PathBuf,
    /// Canonical Norito threshold-signed release attestation.
    pub attestation: PathBuf,
    /// Canonical JSON recursive circuit profile whose digest is signed by the release.
    pub recursive_profile: PathBuf,
    /// Directory containing artifacts named only by lowercase SHA-256 content address.
    pub artifact_directory: PathBuf,
}

/// Runtime-derived Kagemusha V1 reserve custody accounts and proof authority.
#[derive(Debug, Clone)]
pub struct Kagemusha {
    /// Lazily derived reserve accounts keyed by asset definition.
    ///
    /// This map is runtime state, not operator configuration and not an
    /// enablement catalog. Every asset can use the Kagemusha instructions.
    pub reserve_accounts: BTreeMap<AssetDefinitionId, AccountId>,
    /// Optional threshold-authenticated proof release loaded before replay.
    pub proof_release: Option<KagemushaV1ProofReleaseFiles>,
}
impl_default!(Kagemusha => {
    Self {
        reserve_accounts: BTreeMap::new(),
        proof_release: None,
    }
});
