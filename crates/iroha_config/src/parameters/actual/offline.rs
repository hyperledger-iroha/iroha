/// Universal Kagemusha execution state and optional proof-release cache parameters.
#[derive(Debug, Clone)]
pub struct Offline {
    /// Lazily derived escrow accounts keyed by asset definition.
    ///
    /// This map is runtime state, not operator configuration and not an
    /// enablement catalog. Every asset can use the offline instructions.
    pub escrow_accounts: BTreeMap<AssetDefinitionId, AccountId>,
    /// Canonical Norito policy authenticating promoted Kagemusha releases.
    pub kagemusha_release_policy_path: Option<PathBuf>,
    /// Directory containing manifest-digest-addressed Kagemusha release artifacts.
    pub kagemusha_artifact_dir: Option<PathBuf>,
    /// Root-trusted canonical seal for a fully qualified Kagemusha catalog.
    pub kagemusha_catalog_qualification_seal_path: Option<PathBuf>,
    /// Pinned Ed25519 key of the root promotion controller.
    pub kagemusha_promotion_controller_public_key: Option<PublicKey>,
    /// Pinned identifier of the independent catalog-revalidation authority.
    pub kagemusha_catalog_revalidation_authority_key_id: Option<String>,
    /// Pinned Ed25519 key of the independent catalog-revalidation authority.
    pub kagemusha_catalog_revalidation_authority_public_key: Option<PublicKey>,
    /// Root-trusted signed promotion-reservation input path.
    pub kagemusha_promotion_reservation_path: Option<PathBuf>,
    /// Root-owned no-replace output path for this validator's qualification seal.
    pub kagemusha_validator_qualification_seal_path: Option<PathBuf>,
    /// Estimated decoded Kagemusha verifier budget under the 272 MiB safety ceiling.
    pub kagemusha_max_decoded_bytes: u64,
}
impl_default!(Offline => {
        Self {
            escrow_accounts: BTreeMap::new(),
            kagemusha_release_policy_path:
                defaults::settlement::offline::kagemusha_release_policy_path(),
            kagemusha_artifact_dir: defaults::settlement::offline::kagemusha_artifact_dir(),
            kagemusha_catalog_qualification_seal_path:
                defaults::settlement::offline::kagemusha_catalog_qualification_seal_path(),
            kagemusha_promotion_controller_public_key:
                defaults::settlement::offline::kagemusha_promotion_controller_public_key(),
            kagemusha_catalog_revalidation_authority_key_id:
                defaults::settlement::offline::kagemusha_catalog_revalidation_authority_key_id(),
            kagemusha_catalog_revalidation_authority_public_key:
                defaults::settlement::offline::kagemusha_catalog_revalidation_authority_public_key(),
            kagemusha_promotion_reservation_path:
                defaults::settlement::offline::kagemusha_promotion_reservation_path(),
            kagemusha_validator_qualification_seal_path:
                defaults::settlement::offline::kagemusha_validator_qualification_seal_path(),
            kagemusha_max_decoded_bytes: defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES,
        }
});
