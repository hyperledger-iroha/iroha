//! Owner-private Pasta seed descriptors for real disposable validator processes.

use super::*;
use color_eyre::eyre::ensure;
use iroha_data_model::isi::kagemusha_v1::{
    KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityPairedPossessionProofV1,
    KagemushaMintFinalitySeatReadinessContextV1, KagemushaMintFinalityValidatorKeysV1,
};
use rand::{TryRngCore as _, rngs::OsRng};
use std::{
    io::{Seek as _, SeekFrom},
    os::{
        fd::AsRawFd as _,
        unix::{
            fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
            process::CommandExt as _,
        },
    },
};
use tempfile::TempDir;
use zeroize::Zeroizing;

/// Fixed non-secret descriptor named by the node's NPoS runtime configuration.
pub(crate) const MINT_FINALITY_SEED_FD: i32 = 199;

#[allow(
    unsafe_code,
    reason = "the disposable supervisor transfers an exact owner-private seed to a fixed native descriptor"
)]
pub(super) fn inherit_disposable_mint_finality_seed(
    command: &mut tokio::process::Command,
    source: fs::File,
) {
    unsafe {
        command.as_std_mut().pre_exec(move || {
            let source_fd = source.as_raw_fd();
            let installed = if source_fd == MINT_FINALITY_SEED_FD {
                let flags = nix::libc::fcntl(source_fd, nix::libc::F_GETFD);
                flags >= 0
                    && nix::libc::fcntl(
                        source_fd,
                        nix::libc::F_SETFD,
                        flags & !nix::libc::FD_CLOEXEC,
                    ) >= 0
            } else {
                nix::libc::dup2(source_fd, MINT_FINALITY_SEED_FD) >= 0
            };
            if !installed {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[derive(Debug)]
pub(super) struct DisposableMintFinalitySeed {
    _owner_root: Arc<TempDir>,
    path: PathBuf,
}

/// A single writable child descriptor. The daemon consumes this inode; the
/// retained source stays private so the same peer can restart and sign later
/// generations from its original seed.
#[derive(Debug)]
pub(super) struct DisposableMintFinalitySeedLease {
    file: fs::File,
    path: PathBuf,
    _owner_root: Arc<TempDir>,
}

impl DisposableMintFinalitySeedLease {
    pub(super) fn descriptor(&self) -> &fs::File {
        &self.file
    }

    fn retire(&mut self) -> Result<()> {
        let metadata = self.file.metadata()?;
        let named = fs::symlink_metadata(&self.path)?;
        ensure!(
            metadata.is_file()
                && metadata.dev() == named.dev()
                && metadata.ino() == named.ino()
                && metadata.uid() == nix::unistd::geteuid().as_raw()
                && metadata.permissions().mode() & 0o7777 == 0o600
                && metadata.nlink() == 1
                && metadata.len() <= 32,
            "disposable Pasta child descriptor changed before retirement"
        );
        if metadata.len() != 0 {
            self.file.seek(SeekFrom::Start(0))?;
            self.file.write_all(&[0; 32])?;
            self.file.sync_all()?;
            self.file.set_len(0)?;
            self.file.sync_all()?;
        }
        fs::remove_file(&self.path)?;
        Ok(())
    }
}

impl Drop for DisposableMintFinalitySeedLease {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            tracing::warn!(?error, path = ?self.path, "failed to retire disposable Pasta child descriptor");
        }
    }
}

impl DisposableMintFinalitySeed {
    fn generate() -> Result<Self> {
        let root = super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
        let mut seed = Zeroizing::new([0_u8; 32]);
        OsRng
            .try_fill_bytes(seed.as_mut())
            .map_err(|error| eyre!("generate disposable Pasta custody entropy: {error}"))?;
        while *seed == [0; 32] {
            OsRng.try_fill_bytes(seed.as_mut()).map_err(|error| {
                eyre!("regenerate nonzero disposable Pasta custody entropy: {error}")
            })?;
        }
        let path = root.path().join("mint-finality.seed");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(seed.as_ref())?;
        file.sync_all()?;
        Ok(Self {
            _owner_root: root,
            path,
        })
    }

    fn open_validated(&self) -> Result<(fs::File, Zeroizing<[u8; 32]>)> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(&self.path)?;
        let before = file.metadata()?;
        let mut result = Zeroizing::new([0; 32]);
        file.read_exact(result.as_mut())?;
        let mut extra = [0; 1];
        ensure!(
            file.read(&mut extra)? == 0
                && before.is_file()
                && before.len() == 32
                && before.nlink() == 1
                && before.permissions().mode() & 0o7777 == 0o600
                && before.uid() == nix::unistd::geteuid().as_raw()
                && file.metadata()?.dev() == before.dev()
                && file.metadata()?.ino() == before.ino()
                && file.metadata()?.len() == before.len(),
            "disposable Pasta seed descriptor is not an exact owner-private file"
        );
        file.seek(SeekFrom::Start(0))?;
        Ok((file, result))
    }

    fn seed(&self) -> Result<Zeroizing<[u8; 32]>> {
        self.open_validated().map(|(_, seed)| seed)
    }

    fn stage_descriptor(&self, run_num: usize) -> Result<DisposableMintFinalitySeedLease> {
        let seed = self.seed()?;
        let path = self
            ._owner_root
            .path()
            .join(format!("mint-finality-run-{run_num}.seed"));
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let mut lease = DisposableMintFinalitySeedLease {
            file,
            path,
            _owner_root: Arc::clone(&self._owner_root),
        };
        lease.file.write_all(seed.as_ref())?;
        lease.file.sync_all()?;
        lease.file.seek(SeekFrom::Start(0))?;
        Ok(lease)
    }
}

impl NetworkPeer {
    /// Generate one independent owner-private seed for all of this peer's signing generations.
    ///
    /// # Errors
    /// Rejects a running peer, duplicate provision, or unsafe local storage.
    pub fn provision_disposable_mint_finality_seed(&self) -> Result<()> {
        ensure!(
            !self.is_running.load(Ordering::SeqCst),
            "stop peer before provisioning a Pasta seed"
        );
        let mut stored = self
            .disposable_mint_finality_seed
            .lock()
            .map_err(|_| eyre!("disposable Pasta seed lock is poisoned"))?;
        ensure!(
            stored.is_none(),
            "disposable Pasta seed is already provisioned"
        );
        *stored = Some(Arc::new(DisposableMintFinalitySeed::generate()?));
        Ok(())
    }

    fn held_disposable_mint_finality_seed(&self) -> Result<Arc<DisposableMintFinalitySeed>> {
        self.disposable_mint_finality_seed
            .lock()
            .map_err(|_| eyre!("disposable Pasta seed lock is poisoned"))?
            .clone()
            .ok_or_else(|| eyre!("peer has no owner-private Pasta seed"))
    }

    /// Derive public generation keys from this peer's held private seed.
    ///
    /// # Errors
    /// Rejects absent custody or invalid generation derivation.
    pub fn disposable_mint_finality_keys(
        &self,
        generation: u64,
    ) -> Result<KagemushaMintFinalityValidatorKeysV1> {
        let held = self.held_disposable_mint_finality_seed()?;
        let seed = held.seed()?;
        Ok(
            iroha_core_zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &*seed, generation, self.id(),
            )?,
        )
    }

    /// Derive one exact signed-network candidate publication without exposing the seed.
    ///
    /// # Errors
    /// Rejects absent custody, malformed generation or failed paired possession.
    pub fn disposable_mint_finality_candidate(
        &self,
        network_id: NetworkId,
        generation: u64,
    ) -> Result<(
        KagemushaMintFinalityValidatorKeysV1,
        KagemushaMintFinalityPairedPossessionProofV1,
    )> {
        let held = self.held_disposable_mint_finality_seed()?;
        let seed = held.seed()?;
        let keys = iroha_core_zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
            &*seed, generation, self.id(),
        )?;
        let proof = iroha_core_zk::kagemusha_v1_recursion::prove_kagemusha_mint_finality_candidate_possession_v1(
            &*seed, network_id, generation, &keys,
        )?;
        Ok((keys, proof))
    }

    /// Prove one exact prepared seat from the same held private seed.
    ///
    /// # Errors
    /// Rejects absent custody or a public authority inconsistent with this seed.
    pub fn disposable_mint_finality_readiness(
        &self,
        authority: &KagemushaMintFinalityAuthorityGenerationV1,
        context: &KagemushaMintFinalitySeatReadinessContextV1,
    ) -> Result<KagemushaMintFinalityPairedPossessionProofV1> {
        let held = self.held_disposable_mint_finality_seed()?;
        let seed = held.seed()?;
        Ok(
            iroha_core_zk::kagemusha_v1_recursion::prove_kagemusha_mint_finality_seat_readiness_v1(
                &*seed, authority, context,
            )?,
        )
    }

    pub(super) fn disposable_mint_finality_seed_descriptor(
        &self,
        run_num: usize,
    ) -> Result<Option<DisposableMintFinalitySeedLease>> {
        self.disposable_mint_finality_seed
            .lock()
            .map_err(|_| eyre!("disposable Pasta seed lock is poisoned"))?
            .as_ref()
            .map(|held| held.stage_descriptor(run_num))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_seeds_are_independent_and_each_start_consumes_only_its_child() {
        let first = DisposableMintFinalitySeed::generate().expect("first private seed");
        let second = DisposableMintFinalitySeed::generate().expect("second private seed");
        assert!(
            *first.seed().unwrap() != *second.seed().unwrap(),
            "independent disposable peers must not share a Pasta seed"
        );
        let first_lease = first.stage_descriptor(1).expect("first child descriptor");
        let mut descriptor = first_lease.descriptor().try_clone().unwrap();
        let mut bytes = Zeroizing::new([0_u8; 32]);
        descriptor.read_exact(bytes.as_mut()).unwrap();
        assert!(
            *bytes == *first.seed().unwrap(),
            "inherited descriptor must read the held peer seed"
        );
        descriptor.seek(SeekFrom::Start(0)).unwrap();
        descriptor.write_all(&[0; 32]).unwrap();
        descriptor.set_len(0).unwrap();
        drop(descriptor);
        let child_path = first_lease.path.clone();
        drop(first_lease);
        assert!(!child_path.exists(), "consumed child must be unlinked");
        let second_lease = first.stage_descriptor(2).expect("restart child descriptor");
        assert_eq!(second_lease.descriptor().metadata().unwrap().len(), 32);
        assert_eq!(first.seed().unwrap().as_ref(), bytes.as_ref());
        let failed_child_path = second_lease.path.clone();
        drop(second_lease);
        assert!(
            !failed_child_path.exists(),
            "unconsumed child must be erased and unlinked"
        );
        assert_eq!(first.path.metadata().unwrap().len(), 32);
        fs::set_permissions(&first.path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            first.stage_descriptor(3).is_err(),
            "world-readable seed must fail closed"
        );
    }
}
