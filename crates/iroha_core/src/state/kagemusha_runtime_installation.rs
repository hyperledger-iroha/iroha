//! Exact-head runtime verifier installation through the original shared State owner.

use super::*;

impl State {
    /// Borrow an immutable snapshot of the installed process verifier.
    #[must_use]
    pub fn kagemusha_v1_runtime_verifier(
        &self,
    ) -> Arc<dyn crate::smartcontracts::isi::kagemusha::KagemushaV1RuntimeVerifier> {
        Arc::clone(&self.kagemusha_v1_runtime_verifier.read())
    }
    /// Check the process-local KAGEMUSHA verifier against finalized release authority.
    ///
    /// Daemon startup calls this before and after Kura replay, which may change the governed
    /// release lifecycle after a local verifier was installed.
    ///
    /// # Errors
    ///
    /// Returns an error if the governed registry is invalid or an authenticated
    /// local verifier differs. The built-in reject-all verifier is valid while
    /// active artifacts remain unavailable and refuses monetary operations.
    pub fn validate_kagemusha_v1_runtime_for_startup(&self) -> core::result::Result<(), String> {
        let world = self.world.view();
        let installed = self.kagemusha_v1_runtime_verifier();
        let verifier: &dyn std::any::Any = installed.as_ref();
        crate::smartcontracts::isi::kagemusha::runtime_matches_governed_registry(
            verifier,
            world.kagemusha_verifier_registry.get(),
        )
    }
    /// Capture the finalized authority before reading local verifier-release files.
    ///
    /// The matching install consumes this head after file authentication. A block
    /// committed during the read makes the old head unusable even if its registry
    /// content happens to be equal again.
    #[must_use]
    pub fn kagemusha_v1_runtime_reload_head(&self) -> KagemushaV1RuntimeReloadHead {
        let mut read_releases = StateViewReleases::new(self);
        let mut commit_releases = self.state_commit_lock.defer_notifications();
        let _state_commit_lock = commit_releases.lock();
        let world = self.world.view();
        KagemushaV1RuntimeReloadHead {
            network_id: self.network_id,
            block_hash: self
                .block_hashes
                .view_retaining(&mut read_releases.lifecycle.hashes)
                .last()
                .copied(),
            view_generation: self.state_view_generation(),
            registry: world.kagemusha_verifier_registry.get().clone(),
        }
    }
    /// Install a registry whose releases and recursive verifier artifacts were fully authenticated.
    ///
    /// This is the sole production reload from the fail-closed startup verifier.
    /// The concrete input must match the exact finalized head captured before
    /// local file I/O. A stale head or substituted release leaves State unchanged.
    pub fn install_kagemusha_v1_runtime_verifier(
        &self,
        expected: KagemushaV1RuntimeReloadHead,
        verifier: crate::smartcontracts::isi::kagemusha::AuthenticatedKagemushaV1RuntimeVerifier,
    ) -> core::result::Result<(), String> {
        self.install_kagemusha_v1_runtime_verifier_checked(expected, Arc::new(verifier))
    }

    pub(super) fn install_kagemusha_v1_runtime_verifier_checked(
        &self,
        expected: KagemushaV1RuntimeReloadHead,
        verifier: Arc<dyn crate::smartcontracts::isi::kagemusha::KagemushaV1RuntimeVerifier>,
    ) -> core::result::Result<(), String> {
        let retired_verifier;
        let mut read_releases = StateViewReleases::new(self);
        let mut verifier_releases = self.kagemusha_v1_runtime_verifier.defer_notifications();
        let mut commit_releases = self.state_commit_lock.defer_notifications();
        let _state_commit_lock = commit_releases.lock();
        let world = self.world.view();
        if self.network_id != expected.network_id
            || self
                .block_hashes
                .view_retaining(&mut read_releases.lifecycle.hashes)
                .last()
                .copied()
                != expected.block_hash
            || self.state_view_generation() != expected.view_generation
            || world.kagemusha_verifier_registry.get() != &expected.registry
        {
            return Err(
                "KAGEMUSHA verifier reload head changed during local authentication".to_owned(),
            );
        }
        let runtime: &dyn std::any::Any = verifier.as_ref();
        if world
            .kagemusha_verifier_registry
            .get()
            .active_release_id
            .is_some()
            && runtime
                .is::<crate::smartcontracts::isi::kagemusha::RejectAllKagemushaV1RuntimeVerifier>()
        {
            return Err(
                "KAGEMUSHA active verifier reload requires authenticated artifacts".to_owned(),
            );
        }
        crate::smartcontracts::isi::kagemusha::runtime_matches_governed_registry(
            runtime,
            world.kagemusha_verifier_registry.get(),
        )?;
        drop(world);
        retired_verifier = std::mem::replace(&mut *verifier_releases.write(), verifier);
        drop(_state_commit_lock);
        drop(commit_releases);
        drop(verifier_releases);
        drop(read_releases);
        drop(retired_verifier);
        Ok(())
    }
}
