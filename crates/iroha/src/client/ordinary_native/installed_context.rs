//! Descriptor-backed signed-original intake, below actual Android package custody.
//! This lower layer alone does not establish a measured package or a shipping owner. The
//! closed Native Android producer retains/rechecks the package and compiled root; C/JNI
//! accepts neither offered pins nor measurements.
use super::*;
use iroha_core_zk::kagemusha_v1_recursion::{
    KagemushaMintAuthorizationFamilyV1, KagemushaRecursiveVerifierProfileV1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1,
    KagemushaOrdinaryInstalledContextCompiledBindingV1, KagemushaOrdinaryInstalledContextV1,
};

/// Signed public-original owner retaining every exact descriptor. This public Rust lower layer
/// is not the closed Android package owner: the Bridge keeps this object private behind its
/// actual package descriptors and repeated measurements before any financial startup.
/// It creates neither account custody nor a financial provider; current S/W and proofs remain strict.
pub struct KagemushaNativeOrdinaryInstalledContextV1 {
    original: HeldFile,
    runtime: HeldFile,
    sdk: HeldFile,
    profile_original: HeldFile,
    compiled_original: Vec<u8>,
    compiled: KagemushaOrdinaryInstalledContextCompiledBindingV1,
    data: KagemushaOrdinaryInstalledContextV1,
    approvals: [[u8; 64]; 3],
    authority: Arc<KagemushaNativeInstalledRuntimeAuthorityV1>,
    inventory: Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
    profile: KagemushaRecursiveVerifierProfileV1,
    package: PathBuf,
    root: PathBuf,
}
impl KagemushaNativeOrdinaryInstalledContextV1 {
    /// Admit fixed original paths under the actual Native-installed root, using only the complete
    /// producer-approved common SDK compile original and actual Native package measurement.
    /// This Rust producer boundary is not a C/JNI setter. Callers must retain measured package
    /// descriptors and recheck them before/after this constructor and every ensuing owner use.
    /// # Errors
    /// Refuses missing source-selected originals, changed custody, role/parent/package mismatches,
    /// unsigned inventories, wrong threshold releases or a substituted/unreleased profile.
    #[allow(clippy::too_many_arguments)]
    pub fn from_native_installed_originals(
        root: &Path,
        compiled_original: &[u8],
        actual_package: &str,
        actual_version: u64,
        actual_cert: [u8; 32],
        actual_dex: [u8; 32],
        actual_android_abi: &str,
        actual_library: [u8; 32],
        actual_native_abi: u32,
    ) -> Result<Self> {
        require_path(root)?;
        let compiled =
            KagemushaOrdinaryInstalledContextCompiledBindingV1::decode_original(compiled_original)
                .map_err(|e| eyre!(e))?;
        let path = root.join("ordinary-installed-context.signed.bin");
        let original = open_unselected_bounded_public(
            &path,
            KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1 + 192,
        )?;
        let signed = original.bytes(KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1 + 192)?;
        ensure!(
            signed.len() > 192,
            "ordinary installed context approvals missing"
        );
        let at = signed.len() - 192;
        let data = KagemushaOrdinaryInstalledContextV1::decode_original(&signed[..at])
            .map_err(|e| eyre!(e))?;
        let approvals: [[u8; 64]; 3] = std::array::from_fn(|i| {
            signed[at + i * 64..at + (i + 1) * 64]
                .try_into()
                .expect("bounded approval")
        });
        let runtime = open_selected_public(
            &root.join("runtime-manifest.json"),
            data.runtime_manifest_sha256,
            32 * 1024 * 1024,
        )?;
        let sdk = open_selected_public(
            &root.join("sdk-release.json"),
            data.sdk_release_sha256,
            32 * 1024 * 1024,
        )?;
        data.authenticate(
            &compiled,
            &approvals,
            &runtime.bytes(32 * 1024 * 1024)?,
            &sdk.bytes(32 * 1024 * 1024)?,
        )
        .map_err(|e| eyre!(e))?;
        let index = data
            .libraries
            .binary_search_by(|l| l.abi.as_str().cmp(actual_android_abi))
            .map_err(|_| eyre!("ordinary installed compiled Android ABI absent"))?;
        ensure!(
            data.package_name == actual_package
                && data.version_code == actual_version
                && data.certificate_sha256 == actual_cert
                && data.dex_sha256 == actual_dex
                && data.native_abi == actual_native_abi
                && data.libraries[index].sha256 == actual_library,
            "ordinary actual installed package identity rejected"
        );
        let authority = Arc::new(
            KagemushaNativeInstalledRuntimeAuthorityV1::from_independently_installed_original(
                &root.join("runtime-authority.ed25519.bin"),
                Sha256::digest(data.runtime_signer).into(),
                data.runtime_manifest_sha256,
                data.sdk_release_sha256,
                data.minimum_sequence,
            )?,
        );
        let package = root.join("ordinary-native-inventory.signed.bin");
        let public_root = root.join("public-originals");
        let inventory = Arc::new(KagemushaAdmittedOrdinaryNativeInventoryV1::intake(
            authority.clone(),
            &package,
            data.inventory_sha256,
            &public_root,
        )?);
        // Compare the complete signed context roster to the sole actual Native inventory. This
        // joins the copied namespace, not a second body decoder or a host-selected artifact list.
        let mut selected = inventory.body.originals.clone();
        selected.push(inventory.body.checkpoint.clone());
        if let Some(value) = inventory.body.integrity_policy.clone() {
            selected.push(value);
        }
        selected.sort_by(|a, b| a.path.cmp(&b.path));
        selected
            .dedup_by(|a, b| a.path == b.path && a.sha256 == b.sha256 && a.byte_len == b.byte_len);
        ensure!(
            selected.len() == data.originals.len()
                && selected
                    .iter()
                    .zip(&data.originals)
                    .all(|(a, b)| a.path == b.path
                        && a.sha256 == b.sha256
                        && a.byte_len == b.byte_len),
            "ordinary context complete public original roster changed"
        );
        let profile_bound = usize::try_from(data.recursive_profile_size)?;
        let profile_original = HeldFile::open_exact(
            root.join("recursive-verifier-profile.bin"),
            data.recursive_profile_sha256,
            data.recursive_profile_size,
            16 * 1024 * 1024,
        )?;
        let profile = KagemushaRecursiveVerifierProfileV1::from_canonical_original(
            &profile_original.bytes(profile_bound)?,
            profile_bound,
        )
        .map_err(|e| eyre!(e))?;
        ensure!(
            profile.mint_authorization_family
                == KagemushaMintAuthorizationFamilyV1::OrdinaryPreDebit113
                && profile.canonical_digest().map_err(|e| eyre!(e))? == data.native_layout_digest
                && data.native_layout_digest == inventory.release.native_profile_digest(),
            "ordinary released recursive profile original rejected"
        );
        let this = Self {
            original,
            runtime,
            sdk,
            profile_original,
            compiled_original: compiled_original.to_vec(),
            compiled,
            data,
            approvals,
            authority,
            inventory,
            profile,
            package,
            root: public_root,
        };
        this.recheck()?;
        Ok(this)
    }
    /// Revalidate exact retained descriptors, role/parent signatures, inventory and profile bindings.
    /// # Errors
    /// Refuses changed installed originals; no file reload changes the retained authority or profile.
    pub fn recheck(&self) -> Result<()> {
        ensure!(
            self.compiled.encode_original().map_err(|e| eyre!(e))? == self.compiled_original,
            "ordinary compiled original drift"
        );
        let signed = self
            .original
            .bytes(KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1 + 192)?;
        let body = self.data.encode_original().map_err(|e| eyre!(e))?;
        ensure!(
            signed.len() == body.len() + 192
                && signed[..body.len()] == body
                && self
                    .approvals
                    .iter()
                    .enumerate()
                    .all(|(i, a)| signed[body.len() + i * 64..body.len() + (i + 1) * 64] == *a),
            "ordinary context signed original changed"
        );
        self.data
            .authenticate(
                &self.compiled,
                &self.approvals,
                &self.runtime.bytes(32 * 1024 * 1024)?,
                &self.sdk.bytes(32 * 1024 * 1024)?,
            )
            .map_err(|e| eyre!(e))?;
        ensure!(
            self.profile.canonical_original().map_err(|e| eyre!(e))?
                == self
                    .profile_original
                    .bytes(usize::try_from(self.data.recursive_profile_size)?)?,
            "ordinary released profile original changed"
        );
        self.authority.recheck()?;
        self.inventory.recheck()
    }
    /// Retained exact authority for the existing genuine Native account/startup constructor.
    /// Reading it establishes no current S/W or monetary capability.
    ///
    /// # Errors
    /// Refuses changed retained originals, invalid release authentication or lost custody.
    pub fn runtime_authority(&self) -> Result<Arc<KagemushaNativeInstalledRuntimeAuthorityV1>> {
        self.recheck()?;
        Ok(self.authority.clone())
    }
    /// Exact signed packet SHA retained by this authenticated nonmonetary context.
    pub fn inventory_sha256(&self) -> [u8; 32] {
        self.data.inventory_sha256
    }
    /// Real admitted shared inventory; its complete threshold/proof-key gates stay authoritative.
    ///
    /// # Errors
    /// Refuses changed retained originals, invalid release authentication or lost custody.
    pub fn inventory(&self) -> Result<Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>> {
        self.recheck()?;
        Ok(self.inventory.clone())
    }
    /// Exact released profile, never a caller-supplied layout or zero placeholder.
    ///
    /// # Errors
    /// Refuses changed retained originals, invalid release authentication or lost custody.
    pub fn recursive_profile(&self) -> Result<KagemushaRecursiveVerifierProfileV1> {
        self.recheck()?;
        Ok(self.profile.clone())
    }
    /// Fixed original packet location retained by this producer.
    pub fn inventory_path(&self) -> &Path {
        &self.package
    }
    /// Fixed descriptor root retained by this producer.
    pub fn public_original_root(&self) -> &Path {
        &self.root
    }
}
fn open_selected_public(path: &Path, sha: [u8; 32], maximum: usize) -> Result<HeldFile> {
    let n = std::fs::symlink_metadata(path)?.len();
    HeldFile::open_exact(path.to_owned(), sha, n, u64::try_from(maximum)?)
}
fn open_unselected_bounded_public(path: &Path, maximum: usize) -> Result<HeldFile> {
    require_path(path)?;
    let file = RetainedFile::open_regular(path)?;
    let before = file.snapshot()?;
    let length = file.file().metadata()?.len();
    ensure!(
        length > 0 && length <= u64::try_from(maximum)?,
        "ordinary context bounded public original custody rejected"
    );
    let mut bytes = vec![0u8; usize::try_from(length)?];
    iroha_fs::read_exact_at(file.file(), &mut bytes, 0)?;
    ensure!(
        u64::try_from(bytes.len())? == length && before == file.snapshot()?,
        "ordinary context read custody changed"
    );
    let held = HeldFile::open_exact(
        path.to_owned(),
        Sha256::digest(&bytes).into(),
        length,
        u64::try_from(maximum)?,
    )?;
    ensure!(
        before == held.identity,
        "ordinary context original replaced before intake"
    );
    Ok(held)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Real disposable OS descriptors; public byte fixtures only, no Root/account/hardware grants.
    #[test]
    fn ordinary_context_public_original_refuses_replacement_and_symlink_custody() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("ordinary-installed-context.signed.bin");
        std::fs::write(&path, b"public").unwrap();
        let held = open_unselected_bounded_public(&path, 16).unwrap();
        assert_eq!(held.bytes(16).unwrap(), b"public");
        let replacement = root.join("replacement");
        std::fs::write(&replacement, b"public").unwrap();
        match std::fs::rename(&replacement, &path) {
            Ok(()) => assert!(held.bytes(16).is_err()),
            Err(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
                assert!(replacement.exists());
                assert_eq!(held.bytes(16).unwrap(), b"public");
            }
        }
        #[cfg(unix)]
        {
            let link = root.join("symlink");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(open_unselected_bounded_public(&link, 16).is_err());
        }
    }
    #[test]
    fn ordinary_context_public_original_bounds_recovery_before_read() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("public");
        std::fs::write(&path, b"123456789").unwrap();
        assert!(open_unselected_bounded_public(&path, 8).is_err());
        let original = open_unselected_bounded_public(&path, 16).unwrap();
        match std::fs::write(&path, b"1234") {
            Ok(()) => {
                assert!(original.bytes(16).is_err());
                // Reopening the exact truncated file cannot silently substitute the old original.
                assert!(
                    open_selected_public(&path, Sha256::digest(b"123456789").into(), 16).is_err()
                );
            }
            Err(error) => {
                // Genuine Windows non-write-sharing custody prevents this mutation.
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
                assert_eq!(original.bytes(16).unwrap(), b"123456789");
            }
        }
    }
}
