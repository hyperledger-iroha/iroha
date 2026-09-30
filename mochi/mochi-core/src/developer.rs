//! Desktop actions over the same managed environments and contract service as Kagami.
//!
//! This adapter owns no process lifecycle, genesis renderer, signing journal or compiler.
//! Run blocking actions on the desktop task executor and report their typed progress on the UI.

use color_eyre::eyre::{Result, ensure};
pub use iroha_contract_deploy::{DeploymentPreflight, DeploymentProgress};
use iroha_data_model::{smart_contract::ContractAlias, transaction::FeePaymentIntent};
pub use iroha_deploy::managed::ManagedPhase;
use iroha_deploy::managed::{
    InstalledRuntime, ManagedContext, ManagedStatus, ManagedStore, PreparedLocalnet,
    default_state_root, workspace_state_root,
};
use musubi::deployment_runtime::{AliasSelection, DeploymentRuntime};
pub use musubi::deployment_runtime::{ContractInput, DeploymentRun};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// An immutable workspace choice for desktop background actions.
///
/// Selection is persisted by `iroha_deploy`, so Kagami launched from the same workspace sees
/// the same identity, endpoints and recovery journals. Opening a workspace starts no validators.
#[derive(Clone)]
pub struct DeveloperWorkspace {
    root: PathBuf,
    project: PathBuf,
    runtime: InstalledRuntime,
}

impl DeveloperWorkspace {
    /// Select a workspace using the installed bundle and OS application-state location.
    ///
    /// # Errors
    /// Missing workspace, invalid state location, or incomplete installed runtime.
    pub fn open(workspace: &Path) -> Result<Self> {
        Self::with_runtime(
            &default_state_root()?,
            workspace,
            InstalledRuntime::discover()?,
        )
    }

    /// Select a workspace below an explicit application-state root and installed runtime.
    ///
    /// # Errors
    /// Invalid workspace identity or unsafe private store custody.
    pub fn with_runtime(
        state_root: &Path,
        workspace: &Path,
        runtime: InstalledRuntime,
    ) -> Result<Self> {
        let project = workspace.canonicalize()?;
        let root = workspace_state_root(state_root, &project)?;
        let store = ManagedStore::open(&root)?;
        Ok(Self {
            root: store.root().to_path_buf(),
            project,
            runtime,
        })
    }

    /// Resolve desktop input against the immutable opened project, not the process directory.
    #[must_use]
    pub fn resolve_path(&self, path: &Path) -> PathBuf {
        self.project.join(path)
    }

    /// Public storage location; no credentials are copied into the project.
    #[must_use]
    pub fn state_root(&self) -> &Path {
        &self.root
    }

    /// Inspect the contexts shared with Kagami.
    ///
    /// # Errors
    /// A retained generation is invalid or its private storage is inaccessible.
    pub fn contexts(&self) -> Result<Vec<ManagedContext>> {
        Ok(ManagedStore::open(&self.root)?.contexts()?)
    }

    /// Return the selected context name, distinguishing an empty selection from invalid custody.
    ///
    /// # Errors
    /// Existing selection or retained generation metadata is invalid.
    pub fn selected_name(&self) -> Result<Option<String>> {
        match ManagedStore::open(&self.root)?.context(None) {
            Ok(context) => Ok(Some(context.name)),
            Err(iroha_deploy::managed::Error::NoSelection) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Load the selected environment for immutable, network-bound observation and composition.
    ///
    /// # Errors
    /// Missing selection, invalid generated credentials or replaced generation.
    pub fn network(&self, name: Option<&str>) -> Result<ManagedNetwork> {
        let store = ManagedStore::open(&self.root)?;
        let context = store.context(name)?;
        let prepared = store.prepared(&context.name)?;
        let config = prepared.context.load_client_config()?;
        let operator = prepared.load_operator_key_pair()?;
        let network = ManagedNetwork {
            root: self.root.clone(),
            prepared,
            config,
            operator,
        };
        network.validate()?;
        Ok(network)
    }

    /// Select an existing environment without starting it.
    ///
    /// # Errors
    /// Unknown or invalid retained context.
    pub fn select(&self, name: &str) -> Result<ManagedContext> {
        Ok(ManagedStore::open(&self.root)?.select(name)?)
    }

    /// Start a persistent localnet with generated configuration and signed readiness.
    ///
    /// # Errors
    /// Generation, custody, startup, or readiness failure.
    pub fn start(&self, name: &str) -> Result<ManagedStatus> {
        let store = ManagedStore::open(&self.root)?;
        Ok(store.up(&self.runtime.localnet_request(name, Duration::from_secs(30)))?)
    }

    /// Observe the native worker without inferring liveness from stored numeric PIDs.
    ///
    /// # Errors
    /// Unknown generation or an owner which cannot be authenticated.
    pub fn status(&self, name: &str) -> Result<ManagedStatus> {
        Ok(ManagedStore::open(&self.root)?.status(name)?)
    }

    /// Stop the owned validators while retaining keys, ledger, selection and deployment journals.
    ///
    /// # Errors
    /// Unknown generation, unsafe custody or uncertain process ownership.
    pub fn stop(&self, name: &str) -> Result<ManagedStatus> {
        Ok(ManagedStore::open(&self.root)?.down(name)?)
    }

    /// Reset an explicitly named stopped localnet after the desktop obtains reset intent.
    ///
    /// # Errors
    /// Refuses a running or orphan-owned network and unsafe generation storage.
    pub fn reset(&self, name: &str) -> Result<()> {
        Ok(ManagedStore::open(&self.root)?.reset(name)?)
    }

    /// Read a bounded private log tail for display.
    ///
    /// # Errors
    /// Unknown peer, excessive byte limit, or unsafe log custody.
    pub fn logs(&self, name: &str, peer: Option<usize>, maximum: usize) -> Result<String> {
        Ok(ManagedStore::open(&self.root)?.logs(name, peer, maximum)?)
    }

    /// Compile and deploy one input, starting the default localnet if no context was selected.
    ///
    /// `review` sees exact signed-plan fee quotes before dispatch; `progress` reports native
    /// stage evidence. Exact repeats recover the same journal and submitted transaction hashes.
    ///
    /// # Errors
    /// Preparation, authorization, budget, original-hash recovery or final readback failure.
    pub fn deploy(
        &self,
        input: &ContractInput,
        context: Option<&str>,
        alias: Option<ContractAlias>,
        review: &mut dyn FnMut(&DeploymentPreflight) -> Result<()>,
        progress: &mut dyn FnMut(DeploymentProgress),
    ) -> Result<DeploymentRun> {
        let store = ManagedStore::open(&self.root)?;
        let selected = store
            .ensure_selected(&self.runtime, context, Duration::from_secs(30))?
            .context;
        let config = selected.load_client_config()?;
        let runtime =
            DeploymentRuntime::new(config, self.root.join("deployments").join(&selected.name));
        let alias = alias.map_or_else(
            || AliasSelection::Scope {
                domain: None,
                dataspace: selected.dataspace_alias.clone(),
            },
            AliasSelection::Exact,
        );
        runtime.deploy(
            input,
            &alias,
            FeePaymentIntent::authority(Vec::new(), None),
            &mut |preflight| {
                ensure!(
                    preflight.dataspace_id.as_u64() == selected.dataspace_id,
                    "deployment resolved outside the selected dataspace"
                );
                review(preflight)
            },
            progress,
        )
    }

    /// Recover an exact retained journal using its selected environment without rebuilding.
    ///
    /// # Errors
    /// Missing selection, mismatched original authority, pending finality or failed readback.
    pub fn resume(
        &self,
        journal: &Path,
        context: Option<&str>,
        review: &mut dyn FnMut(&DeploymentPreflight) -> Result<()>,
        progress: &mut dyn FnMut(DeploymentProgress),
    ) -> Result<DeploymentRun> {
        let store = ManagedStore::open(&self.root)?;
        store.context(context)?;
        let selected = store
            .ensure_selected(&self.runtime, context, Duration::from_secs(30))?
            .context;
        let runtime = DeploymentRuntime::new(
            selected.load_client_config()?,
            self.root.join("deployments").join(&selected.name),
        );
        runtime.resume(
            journal,
            &mut |preflight| {
                ensure!(
                    preflight.dataspace_id.as_u64() == selected.dataspace_id,
                    "recovery belongs to another dataspace"
                );
                review(preflight)
            },
            progress,
        )
    }
}

/// An immutable observation and signing authority for one generated network.
///
/// This value owns no child processes. Releasing it only releases local client credentials;
/// Kagami and the desktop continue to share the independently owned native worker.
#[derive(Clone)]
pub struct ManagedNetwork {
    root: PathBuf,
    prepared: PreparedLocalnet,
    config: iroha::config::Config,
    operator: iroha_crypto::KeyPair,
}

impl ManagedNetwork {
    /// Public metadata for this exact four-validator generation.
    #[must_use]
    pub fn prepared(&self) -> &PreparedLocalnet {
        &self.prepared
    }

    /// Refuse a removed or replaced generation without adopting its authority or endpoint.
    ///
    /// # Errors
    /// Invalid custody or a different generation is currently stored under this name.
    pub fn validate(&self) -> Result<()> {
        let current = ManagedStore::open(&self.root)?.prepared(&self.prepared.context.name)?;
        ensure!(
            current == self.prepared,
            "selected network was replaced; select it again"
        );
        Ok(())
    }

    /// Chain-specific address formatting for synchronous desktop draft input and output.
    #[must_use]
    pub fn address_discriminant(&self) -> u16 {
        self.config.account_chain_discriminant
    }

    /// Generated account authority for transaction previews and authenticated state queries.
    #[must_use]
    pub fn signer(&self) -> crate::SigningAuthority {
        crate::SigningAuthority::new(
            self.prepared.context.name.clone(),
            self.config.account.clone(),
            self.config.key_pair.clone(),
        )
    }

    /// Build a peer-bound Torii observer with the generated operator authority.
    ///
    /// # Errors
    /// Invalid index, stale generation or invalid immutable endpoint.
    pub fn observer(&self, peer: usize) -> Result<crate::ToriiClient> {
        self.validate()?;
        let endpoint = &self
            .prepared
            .peers
            .get(peer)
            .ok_or_else(|| color_eyre::eyre::eyre!("unknown validator index"))?
            .torii_url;
        Ok(crate::ToriiClient::builder(endpoint)?
            .with_network_id(self.config.network_id)
            .with_operator_signing_context(crate::OperatorSigningContext::new(
                self.config.network_id,
                self.operator.clone(),
            ))
            .build()?)
    }

    /// Read canonical block/event streams as the exact generated account.
    ///
    /// # Errors
    /// Invalid index, replaced generation or an invalid SDK authority.
    pub fn ledger_reader(&self, peer: usize) -> Result<iroha::client::AccountClient> {
        self.validate()?;
        let endpoint = &self
            .prepared
            .peers
            .get(peer)
            .ok_or_else(|| color_eyre::eyre::eyre!("unknown validator index"))?
            .torii_url;
        let mut config = self.config.clone();
        config.torii_api_url = endpoint.parse()?;
        Ok(iroha::client::Client::builder(config)
            .build()?
            .account_client()?)
    }

    /// Compose one exact transaction using this generation's authority and lineage.
    ///
    /// # Errors
    /// Stale generation or drafts rejected by the canonical composer.
    pub fn preview(&self, drafts: &[crate::InstructionDraft]) -> Result<crate::TransactionPreview> {
        self.validate()?;
        let _address_profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(
            self.address_discriminant(),
        );
        Ok(crate::compose_preview_with_options(
            self.config.network_id,
            drafts,
            &self.signer(),
            &crate::TransactionComposeOptions::default().with_ttl(Duration::from_secs(300)),
        )?)
    }

    /// Submit a reviewed transaction once, then observe its original hash until the deadline.
    ///
    /// # Errors
    /// Wrong generation/authority, rejection or uncertain finality. An error never rebuilds it.
    pub async fn submit(
        &self,
        peer: usize,
        preview: &crate::TransactionPreview,
    ) -> Result<crate::SmokeCommitSnapshot> {
        ensure!(
            preview.signed_transaction().network_id() == Some(&self.config.network_id),
            "transaction belongs to another network"
        );
        ensure!(
            preview.signed_transaction().authority() == &self.config.account,
            "transaction belongs to another authority"
        );
        let client = self.observer(peer)?;
        let reader = self.ledger_reader(peer)?;
        Ok(client
            .submit_and_wait_for_commit(
                &reader,
                preview.signed_transaction(),
                crate::SmokeCommitOptions::new(Duration::from_secs(30)),
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_and_cli_share_store_identity_without_writing_the_project_or_starting_nodes() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("project");
        let installation = temporary.path().join("bin");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&installation).unwrap();
        for program in ["kagami", "iroha3d"] {
            std::fs::write(
                installation.join(format!("{program}{}", std::env::consts::EXE_SUFFIX)),
                b"unused test executable",
            )
            .unwrap();
        }
        let runtime = InstalledRuntime::from_directory(&installation).unwrap();
        let base = temporary.path().join("state");
        let desktop = DeveloperWorkspace::with_runtime(&base, &workspace, runtime).unwrap();
        let expected = workspace_state_root(&base, &workspace).unwrap();
        assert_eq!(desktop.state_root(), expected.canonicalize().unwrap());
        assert_eq!(
            desktop.resolve_path(Path::new("contracts/app.ko")),
            workspace.canonicalize().unwrap().join("contracts/app.ko")
        );
        assert_eq!(desktop.resolve_path(&installation), installation);
        assert!(desktop.contexts().unwrap().is_empty());
        assert_eq!(desktop.selected_name().unwrap(), None);
        assert!(
            ManagedStore::open(&expected)
                .unwrap()
                .contexts()
                .unwrap()
                .is_empty()
        );
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 0);
        assert!(desktop.select("missing").is_err());
        assert!(desktop.status("missing").is_err());
        assert!(desktop.stop("missing").is_err());
        assert!(desktop.reset("missing").is_err());
        assert!(desktop.logs("missing", None, 1024).is_err());
        assert!(desktop.start("../invalid").is_err());
        assert!(
            desktop
                .resume(Path::new("journal"), None, &mut |_| Ok(()), &mut |_| {})
                .is_err()
        );
        assert!(
            desktop
                .deploy(
                    &ContractInput::Source(workspace.join("hello.ko")),
                    Some("missing"),
                    None,
                    &mut |_| Ok(()),
                    &mut |_| {}
                )
                .is_err()
        );
        assert!(desktop.contexts().unwrap().is_empty());
    }
    #[test]
    fn generated_authority_and_observers_survive_desktop_close_but_reject_reset() {
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("project");
        let bin = temporary.path().join("bin");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&bin).unwrap();
        // Invalid executables intentionally fail before any worker starts. Canonical preparation
        // is real; this test does not substitute a genesis/configuration renderer or daemon.
        for program in ["kagami", "iroha3d"] {
            std::fs::write(
                bin.join(format!("{program}{}", std::env::consts::EXE_SUFFIX)),
                b"not executable",
            )
            .unwrap();
        }
        let desktop = DeveloperWorkspace::with_runtime(
            &temporary.path().join("state"),
            &project,
            InstalledRuntime::from_directory(&bin).unwrap(),
        )
        .unwrap();
        assert!(desktop.start("fixture").is_err());
        let context = desktop.select("fixture").unwrap();
        let network = desktop.network(None).unwrap();
        assert_eq!(network.prepared().peers.len(), 4);
        assert_eq!(
            network.signer().account_id().to_string(),
            context.account_id
        );
        assert_ne!(
            network.signer().account_id(),
            &*iroha_test_samples::ALICE_ID
        );
        for peer in 0..4 {
            let observer = network.observer(peer).unwrap();
            let reader = network.ledger_reader(peer).unwrap();
            assert_eq!(observer.network_id().as_ref(), Some(reader.network_id()));
            assert_eq!(observer.base_url(), reader.endpoint().as_str());
        }
        assert!(network.observer(4).is_err());
        assert!(network.ledger_reader(4).is_err());
        let draft = crate::InstructionDraft::register_account_from_input(
            &iroha_test_samples::BOB_ID.to_string(),
        )
        .unwrap();
        let preview = network.preview(&[draft]).unwrap();
        assert_eq!(
            preview.signed_transaction().authority(),
            network.signer().account_id()
        );
        assert_eq!(
            preview
                .signed_transaction()
                .network_id()
                .unwrap()
                .to_string(),
            context.network_id
        );
        let root = desktop.state_root().to_path_buf();
        drop(desktop);
        assert_eq!(
            ManagedStore::open(&root).unwrap().context(None).unwrap(),
            context
        );
        network.validate().unwrap();
        ManagedStore::open(&root).unwrap().reset("fixture").unwrap();
        assert!(network.validate().is_err());
        assert!(network.observer(0).is_err());
        assert!(network.ledger_reader(0).is_err());
        assert!(network.preview(&[]).is_err());
        assert_eq!(std::fs::read_dir(project).unwrap().count(), 0);
    }
}
