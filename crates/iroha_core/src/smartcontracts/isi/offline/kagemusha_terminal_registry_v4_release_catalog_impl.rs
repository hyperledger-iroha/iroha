impl KagemushaReleaseCatalogV4 {
    /// Return an unconfigured, always-unready catalog.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
    /// Whether a canonical policy and artifact directory were configured.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        self.configured_policy_sha256.is_some()
    }
    /// Digest of the configured canonical Norito policy, when configured.
    #[must_use]
    pub const fn configured_policy_sha256(&self) -> Option<[u8; 32]> {
        self.configured_policy_sha256
    }
    /// Load an optional immutable verifier cache.
    ///
    /// An omitted policy/artifact pair produces the explicit empty catalog. The cache is not an
    /// offline-capability switch and is not an asset catalog; every deployment and asset retains
    /// the protocol primitives when it is empty. A partially configured pair or authentication
    /// failure is rejected only when an operator explicitly configures this cache.
    ///
    /// # Errors
    ///
    /// Returns an error when only one catalog path is configured or when the configured catalog
    /// or qualification seal cannot be authenticated.
    pub fn from_offline_config(
        config: &iroha_config::parameters::actual::Offline,
    ) -> Result<Self, String> {
        match (
            config.kagemusha_release_policy_path.as_deref(),
            config.kagemusha_artifact_dir.as_deref(),
        ) {
            (None, None) => Ok(Self::empty()),
            (Some(policy_path), Some(artifact_dir)) => {
                if let Some(seal_path) = config.kagemusha_catalog_qualification_seal_path.as_deref()
                {
                    Self::load_with_decoded_budget_and_qualification_seal(
                        policy_path,
                        artifact_dir,
                        config.kagemusha_max_decoded_bytes,
                        seal_path,
                    )
                } else {
                    Self::load_with_decoded_budget(
                        policy_path,
                        artifact_dir,
                        config.kagemusha_max_decoded_bytes,
                    )
                }
            }
            _ => Err(
                "Kagemusha V4 release policy and artifact directory must be configured together"
                    .to_owned(),
            ),
        }
    }
    /// Authenticate only the configured release policy for staged genesis signing.
    ///
    /// A Kagemusha release is bound to the genesis-derived `NetworkId`, so its artifact
    /// directory and qualification seal cannot exist while genesis is being signed. This
    /// constructor pins, canonically decodes, and hashes the configured policy while deliberately
    /// leaving the release map empty. The resulting catalog exists only to contribute the same
    /// configured-policy digest that the later fully qualified runtime catalog contributes to the
    /// signed execution-policy identity. Production staging requires a root-owned, non-writable,
    /// ACL-free policy path chain and requires the future artifact directory not to exist yet.
    ///
    /// Runtime startup must use [`Self::from_offline_config`], which still requires every
    /// explicitly configured artifact directory to authenticate as a nonempty catalog and, when
    /// configured, requires its qualification seal to authenticate too.
    ///
    /// # Errors
    ///
    /// Returns an error when only one catalog path is configured, any qualification seal is
    /// present in the staging config, or the configured canonical policy cannot be authenticated.
    pub fn from_offline_config_for_genesis_staging(
        config: &iroha_config::parameters::actual::Offline,
    ) -> Result<Self, String> {
        match (
            config.kagemusha_release_policy_path.as_deref(),
            config.kagemusha_artifact_dir.as_deref(),
        ) {
            (None, None) => {
                if config.kagemusha_catalog_qualification_seal_path.is_some() {
                    return Err(
                        "Kagemusha V4 qualification seal requires both release paths".to_owned(),
                    );
                }
                Ok(Self::empty())
            }
            (Some(policy_path), Some(artifact_dir)) => {
                if config.kagemusha_catalog_qualification_seal_path.is_some() {
                    return Err(
                        "Kagemusha V4 staged genesis config must omit the not-yet-created qualification seal"
                            .to_owned(),
                    );
                }
                Self::load_policy_only_for_genesis_staging(policy_path, artifact_dir)
            }
            _ => Err(
                "Kagemusha V4 release policy and artifact directory must be configured together"
                    .to_owned(),
            ),
        }
    }
    pub(crate) fn get(&self, manifest_sha256: &[u8; 32]) -> Option<&Arc<KagemushaCachedReleaseV4>> {
        self.releases.get(manifest_sha256)
    }
    /// Number of authenticated releases retained by this process.
    #[must_use]
    pub fn len(&self) -> usize {
        self.releases.len()
    }
    /// Whether this catalog contains no authenticated releases.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }
    /// Deterministically authenticate every manifest-digest subdirectory.
    ///
    /// Both configured paths must be canonical absolute paths. Every directory component is opened
    /// relative to its already pinned parent and symlinks are rejected at every level.
    ///
    /// All filesystem access, hashing, framing checks, Halo2 verifier parsing,
    /// and allocation-free proving-key structural validation complete before
    /// the returned immutable catalog is published. Full proving-key parsing is
    /// deferred until an actual prover operation needs that parity.
    pub fn load(policy_path: &Path, artifact_dir: &Path) -> Result<Self, String> {
        Self::load_with_decoded_budget(
            policy_path,
            artifact_dir,
            DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4,
        )
    }
    /// Authenticate a catalog under an explicit decoded-resident memory ceiling.
    pub fn load_with_decoded_budget(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
    ) -> Result<Self, String> {
        validate_kagemusha_catalog_decoded_budget_v4(max_decoded_bytes)?;
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            Self::load_descriptor_relative(policy_path, artifact_dir, max_decoded_bytes)
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (policy_path, artifact_dir, max_decoded_bytes);
            Err(
                "Kagemusha V4 descriptor-relative catalog loading is unsupported on this platform"
                    .to_owned(),
            )
        }
    }
    /// Fully authenticate a catalog and produce its root-trusted restart seal.
    ///
    /// This constructor always executes complete artifact hashing and Eq/Ep proving-key structural
    /// qualification before it emits a seal. It also requires the configured inputs and current
    /// executable to be rooted in root-owned, non-writable, symlink-free path chains.
    pub fn load_and_build_qualification_seal(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
    ) -> Result<(Self, KagemushaCatalogQualificationSealV1), String> {
        validate_kagemusha_catalog_decoded_budget_v4(max_decoded_bytes)?;
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            Self::load_and_build_qualification_seal_for_trusted_uid(
                policy_path,
                artifact_dir,
                max_decoded_bytes,
                0,
            )
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (policy_path, artifact_dir, max_decoded_bytes);
            Err("Kagemusha V4 qualification seals are unsupported on this platform".to_owned())
        }
    }
    /// Load a fully qualified catalog through a persistent root-trusted seal.
    ///
    /// Seal absence or any path, stat, build, digest, inventory, or qualified
    /// metadata mismatch fails closed. The fast path never refreshes the seal
    /// and never streams a proving-key payload.
    pub fn load_with_decoded_budget_and_qualification_seal(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        seal_path: &Path,
    ) -> Result<Self, String> {
        validate_kagemusha_catalog_decoded_budget_v4(max_decoded_bytes)?;
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            Self::load_with_qualification_seal_for_trusted_uid(
                policy_path,
                artifact_dir,
                max_decoded_bytes,
                seal_path,
                0,
            )
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (policy_path, artifact_dir, max_decoded_bytes, seal_path);
            Err("Kagemusha V4 qualification seals are unsupported on this platform".to_owned())
        }
    }
    fn load_policy_only_for_genesis_staging(
        policy_path: &Path,
        artifact_dir: &Path,
    ) -> Result<Self, String> {
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            Self::load_policy_only_for_genesis_staging_for_trusted_uid(policy_path, artifact_dir, 0)
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (policy_path, artifact_dir);
            Err(
                "Kagemusha V4 descriptor-relative policy loading is unsupported on this platform"
                    .to_owned(),
            )
        }
    }
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_policy_only_for_genesis_staging_for_trusted_uid(
        policy_path: &Path,
        artifact_dir: &Path,
        trusted_uid: u32,
    ) -> Result<Self, String> {
        let (policy_parent_path, policy_file_name) =
            absolute_file_parent_and_name(policy_path, "release policy")?;
        let policy_parent =
            CatalogDirectory::open_path(policy_parent_path, "release policy parent")?;
        policy_parent.verify_trusted_path_chain(trusted_uid, "release policy parent")?;
        let mut policy_file = policy_parent.open_file(policy_file_name, "release policy")?;
        policy_file.verify_trusted(trusted_uid)?;
        let policy_bytes =
            read_bounded_opened_file(&mut policy_file, MAX_POLICY_BYTES, "release policy")?;
        decode_trusted_policy(&policy_bytes)?;
        let policy_sha256: [u8; 32] = Sha256::digest(&policy_bytes).into();

        let (artifact_parent_path, artifact_directory_name) =
            absolute_file_parent_and_name(artifact_dir, "future artifact root")?;
        let artifact_parent =
            CatalogDirectory::open_path(artifact_parent_path, "future artifact parent")?;
        artifact_parent.verify_trusted_path_chain(trusted_uid, "future artifact parent")?;
        if artifact_parent
            .entry_names("future artifact parent")?
            .iter()
            .any(|name| name.as_str() == artifact_directory_name)
        {
            return Err(
                "Kagemusha V4 staged genesis artifact directory must not exist before exact-network release generation"
                    .to_owned(),
            );
        }
        artifact_parent.verify_path_identity()?;
        policy_file.verify_trusted(trusted_uid)?;
        policy_parent.verify_trusted_path_chain(trusted_uid, "release policy parent")?;
        Ok(Self {
            configured_policy_sha256: Some(policy_sha256),
            releases: BTreeMap::new(),
        })
    }
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_descriptor_relative(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
    ) -> Result<Self, String> {
        Self::load_descriptor_relative_with_qualification(
            policy_path,
            artifact_dir,
            max_decoded_bytes,
            None,
        )
    }
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_descriptor_relative_with_qualification(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        qualification_seal: Option<&KagemushaCatalogQualificationSealV1>,
    ) -> Result<Self, String> {
        let (policy_parent_path, policy_file_name) =
            absolute_file_parent_and_name(policy_path, "release policy")?;
        let policy_parent =
            CatalogDirectory::open_path(policy_parent_path, "release policy parent")?;
        let mut policy_file = policy_parent.open_file(policy_file_name, "release policy")?;
        let policy_bytes =
            read_bounded_opened_file(&mut policy_file, MAX_POLICY_BYTES, "release policy")?;
        let policy = decode_trusted_policy(&policy_bytes)?;
        let policy_sha256: [u8; 32] = Sha256::digest(&policy_bytes).into();
        if qualification_seal.is_some_and(|seal| seal.configured_policy_sha256 != policy_sha256) {
            return Err(
                "Kagemusha V4 qualification seal configured-policy digest mismatch".to_owned(),
            );
        }
        let artifact_root = CatalogDirectory::open_path(artifact_dir, "artifact root")?;
        let directory_names = artifact_root.entry_names("artifact root")?;
        ensure_catalog_release_count(directory_names.len())?;
        if let Some(seal) = qualification_seal {
            let sealed_names = seal
                .releases
                .iter()
                .map(|release| hex::encode(release.manifest_sha256))
                .collect::<Vec<_>>();
            if sealed_names != directory_names {
                return Err("Kagemusha V4 qualification seal release inventory mismatch".to_owned());
            }
        }
        let mut releases = BTreeMap::new();
        let mut aggregate_catalog_bytes = 0_u64;
        let mut aggregate_decoded_bytes = 0_u64;
        for directory_name in &directory_names {
            let file_name = directory_name.as_str();
            let manifest_sha256 = parse_manifest_directory_name(file_name)?;
            let directory = artifact_root
                .open_directory(directory_name, &format!("release directory `{file_name}`"))?;
            let remaining_catalog_bytes = MAX_CATALOG_AGGREGATE_BYTES_V4
                .checked_sub(aggregate_catalog_bytes)
                .ok_or_else(|| {
                    "Kagemusha V4 catalog aggregate byte accounting overflowed".to_owned()
                })?;
            let remaining_decoded_bytes = max_decoded_bytes
                .checked_sub(aggregate_decoded_bytes)
                .ok_or_else(|| {
                    "Kagemusha V4 decoded catalog memory accounting overflowed".to_owned()
                })?;
            let sealed_release = qualification_seal.and_then(|seal| {
                seal.releases
                    .iter()
                    .find(|release| release.manifest_sha256 == manifest_sha256)
            });
            if qualification_seal.is_some() && sealed_release.is_none() {
                return Err("Kagemusha V4 qualification seal omits a catalog release".to_owned());
            }
            let (release, release_bytes, release_decoded_bytes) = load_release_directory(
                &directory,
                manifest_sha256,
                &policy,
                policy_sha256,
                remaining_catalog_bytes,
                remaining_decoded_bytes,
                sealed_release,
            )?;
            aggregate_catalog_bytes =
                add_catalog_release_bytes(aggregate_catalog_bytes, release_bytes)?;
            aggregate_decoded_bytes = aggregate_decoded_bytes
                .checked_add(release_decoded_bytes)
                .ok_or_else(|| {
                    "Kagemusha V4 decoded catalog memory accounting overflowed".to_owned()
                })?;
            artifact_root.verify_directory_entry(directory_name, &directory)?;
            if releases
                .insert(manifest_sha256, Arc::new(release))
                .is_some()
            {
                return Err("Kagemusha V4 artifact catalog repeats a manifest digest".to_owned());
            }
        }
        if artifact_root.entry_names("artifact root")? != directory_names {
            return Err("Kagemusha V4 artifact inventory changed while it was loaded".to_owned());
        }
        artifact_root.verify_path_identity()?;
        policy_file.verify_unchanged()?;
        policy_parent.verify_path_identity()?;
        if releases.is_empty() {
            return Err(
                "configured Kagemusha V4 artifact directory contains no releases".to_owned(),
            );
        }
        Ok(Self {
            configured_policy_sha256: Some(policy_sha256),
            releases,
        })
    }
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_and_build_qualification_seal_for_trusted_uid(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        trusted_uid: u32,
    ) -> Result<(Self, KagemushaCatalogQualificationSealV1), String> {
        let effective_uid = rustix::process::geteuid().as_raw();
        if effective_uid != trusted_uid {
            return Err(format!(
                "Kagemusha V4 qualification seal creation requires effective uid {trusted_uid}, found {effective_uid}"
            ));
        }
        let catalog = Self::load_descriptor_relative(policy_path, artifact_dir, max_decoded_bytes)?;
        let seal = build_kagemusha_catalog_qualification_seal_v1(
            policy_path,
            artifact_dir,
            &catalog,
            trusted_uid,
        )?;
        verify_kagemusha_catalog_sealed_paths_v1(&seal.paths, trusted_uid)?;
        Ok((catalog, seal))
    }
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_with_qualification_seal_for_trusted_uid(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        seal_path: &Path,
        trusted_uid: u32,
    ) -> Result<Self, String> {
        let seal =
            read_root_trusted_kagemusha_catalog_qualification_seal_v1(seal_path, trusted_uid)?;
        seal.validate_for_configured_runtime(policy_path, artifact_dir)?;
        verify_kagemusha_catalog_sealed_paths_v1(&seal.paths, trusted_uid)?;
        let catalog = Self::load_descriptor_relative_with_qualification(
            policy_path,
            artifact_dir,
            max_decoded_bytes,
            Some(&seal),
        )?;
        verify_kagemusha_catalog_sealed_paths_v1(&seal.paths, trusted_uid)?;
        Ok(catalog)
    }
    /// Build the exact governed activation payload for one authenticated release.
    ///
    /// This is the only production constructor for the consensus payload. It projects both inline
    /// verifier records from the immutable, qualified pinned startup source, so an operator cannot
    /// substitute release fields, key bytes, commitments, schemas, activation heights, or policy
    /// identity. Consensus still enforces that `verifier_version` is the next atomic Eq/Ep version
    /// when the resulting instruction is executed.
    pub fn build_activation(
        &self,
        manifest_sha256: [u8; 32],
        verifier_version: u32,
    ) -> Result<KagemushaRecursiveSpendReleaseActivationV4, String> {
        if verifier_version == 0 {
            return Err("Kagemusha V4 verifier version must be nonzero".to_owned());
        }
        let configured_policy_sha256 = self.configured_policy_sha256.ok_or_else(|| {
            "Kagemusha V4 activation requires a configured release policy".to_owned()
        })?;
        let cached = self.get(&manifest_sha256).ok_or_else(|| {
            "Kagemusha V4 activation release is absent from the authenticated catalog".to_owned()
        })?;
        let release = cached.resolved.release();
        let binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: release.manifest().generation.clone(),
            manifest_sha256,
        };
        let step_eq_verifier_record = cached.activation_record(
            &binding,
            KagemushaPastaCycleParityV1::StepEq,
            verifier_version,
        )?;
        let step_ep_verifier_record = cached.activation_record(
            &binding,
            KagemushaPastaCycleParityV1::StepEp,
            verifier_version,
        )?;
        let activation = KagemushaRecursiveSpendReleaseActivationV4 {
            release_record: cached.release_record.clone(),
            configured_policy_sha256,
            step_eq_verifier_key_id:
                iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                    KagemushaPastaCycleParityV1::StepEq,
                    manifest_sha256,
                ),
            step_eq_verifier_record,
            step_ep_verifier_key_id:
                iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                    KagemushaPastaCycleParityV1::StepEp,
                    manifest_sha256,
                ),
            step_ep_verifier_record,
        };
        activation
            .validate_structure()
            .map_err(|error| format!("constructed Kagemusha V4 activation is invalid: {error}"))?;
        Ok(activation)
    }
    pub(crate) fn resolve_binding(
        &self,
        binding: &KagemushaRecursiveSpendArtifactBindingV4,
    ) -> Result<&Arc<KagemushaCachedReleaseV4>, String> {
        binding
            .validate()
            .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
        let cached = self.get(&binding.manifest_sha256).ok_or_else(|| {
            "Kagemusha V4 release is not present in the immutable startup catalog".to_owned()
        })?;
        if cached.resolved.release().manifest().generation != binding.generation {
            return Err("Kagemusha V4 release generation and digest disagree".to_owned());
        }
        Ok(cached)
    }
    pub(crate) fn resolve_activation_records(
        &self,
        step_eq_record: &VerifyingKeyRecord,
        step_ep_record: &VerifyingKeyRecord,
    ) -> Result<&Arc<KagemushaCachedReleaseV4>, String> {
        let manifest_sha256 = activation_manifest_sha256(step_eq_record, step_ep_record)?;
        let cached = self.get(&manifest_sha256).ok_or_else(|| {
            "active Kagemusha V4 release is absent from the immutable startup catalog".to_owned()
        })?;
        cached.validate_verifier_records(step_eq_record, step_ep_record)?;
        Ok(cached)
    }
}
