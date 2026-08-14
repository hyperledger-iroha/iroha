use super::*;
impl Supervisor {
    pub(super) fn running_peer_aliases(&self) -> Vec<String> {
        self.peers
            .iter()
            .filter(|peer| peer.is_running())
            .map(|peer| peer.alias().to_owned())
            .collect()
    }
    pub(super) fn stop_captured_running_peers(&mut self, aliases: &[String]) -> Result<()> {
        let mut failures = Vec::new();
        for alias in aliases {
            let Some(peer) = self.peers.iter_mut().find(|peer| peer.alias() == alias) else {
                failures.push(format!("{alias}: peer disappeared from the topology"));
                continue;
            };
            if let Err(error) = peer.stop() {
                failures.push(format!("{alias}: {error}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(SupervisorError::Config(format!(
                "failed to stop captured running-peer set: {}",
                failures.join("; ")
            )))
        }
    }
    pub(super) fn restore_captured_running_peers(&mut self, aliases: &[String]) -> Result<()> {
        if aliases.is_empty() {
            return Ok(());
        }
        let irohad = self.irohad_path()?;
        let ownership_lock = Arc::clone(&self._ownership_lock);
        let mut failures = Vec::new();
        for alias in aliases {
            let Some(peer) = self.peers.iter_mut().find(|peer| peer.alias() == alias) else {
                failures.push(format!("{alias}: peer disappeared from the topology"));
                continue;
            };
            peer.refresh_state(&irohad, &ownership_lock);
            if peer.is_running() {
                continue;
            }
            if let Err(error) = peer.start(&irohad, StartReason::Manual, &ownership_lock) {
                failures.push(format!("{alias}: {error}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(SupervisorError::RunningSetRestore {
                details: failures.join("; "),
            })
        }
    }
    pub(super) fn restore_running_set_after_error(
        &mut self,
        aliases: &[String],
        primary: SupervisorError,
    ) -> SupervisorError {
        match self.restore_captured_running_peers(aliases) {
            Ok(()) => primary,
            Err(restore) => SupervisorError::OperationAndRunningSetRestore {
                primary: Box::new(primary),
                restore: Box::new(restore),
            },
        }
    }
    /// Re-render a single peer config with temporary overlays and restart that peer.
    pub fn restart_peer_with_extra_layers(
        &mut self,
        alias: &str,
        extra_layers: &[toml::Table],
    ) -> Result<()> {
        self.restart_peer_with_extra_layers_inner(alias, extra_layers, None)
    }
    #[cfg(test)]
    pub(super) fn restart_peer_with_extra_layers_with_publication_fault(
        &mut self,
        alias: &str,
        extra_layers: &[toml::Table],
        fault: PublicationFaultPoint,
    ) -> Result<()> {
        self.restart_peer_with_extra_layers_inner(alias, extra_layers, Some(fault))
    }
    fn restart_peer_with_extra_layers_inner(
        &mut self,
        alias: &str,
        extra_layers: &[toml::Table],
        publication_fault: Option<PublicationFaultPoint>,
    ) -> Result<()> {
        let index = self
            .peers
            .iter()
            .position(|peer| peer.alias() == alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        self.refresh_peer_states();
        let previously_running = self.peers[index]
            .is_running()
            .then(|| vec![alias.to_owned()])
            .unwrap_or_default();
        let generation_transaction = GenerationTransaction::begin_replacing(
            self.paths.root(),
            Some(self.genesis.generation_id.clone()),
        )?;
        let selected = verify_selected_generation(self.paths.root(), &self.genesis.generation_id)?;
        self.ensure_selected_generation_metadata(&selected)?;
        self.ensure_selected_peer_storage_paths_under_lock(&selected)?;
        let generation_id = generation_transaction.id().to_owned();
        let generation_root = generation_transaction.root().to_path_buf();
        let specs = self
            .peers
            .iter()
            .map(|peer| peer.spec.in_generation(&generation_root))
            .collect::<Result<Vec<_>>>()?;
        let genesis = self
            .genesis
            .copy_into_generation(&generation_id, &generation_root)?;
        for (peer_index, spec) in specs.iter().enumerate() {
            let layers = if peer_index == index {
                extra_layers
            } else {
                &[]
            };
            spec.write_config(
                &self.chain_id,
                &genesis,
                &specs,
                &self.peer_config_overrides,
                layers,
            )?;
        }
        genesis.validate_generation(&self.chain_id, &specs)?;
        let expected_hash = genesis.expected_hash.ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "validated generation omitted its exact genesis hash".to_owned(),
            )
        })?;
        if let Err(stop_error) = self.stop_captured_running_peers(&previously_running) {
            return Err(self.restore_running_set_after_error(&previously_running, stop_error));
        }
        let inventory = GenerationInventoryContext {
            chain_id: &self.chain_id,
            chain_discriminant: genesis.chain_discriminant,
            genesis_public_key: genesis.public_key(),
            expected_hash,
        };
        let publication = match publication_fault {
            Some(fault) => {
                generation_transaction.publish_with_fault_retaining_failure(inventory, fault)
            }
            None => generation_transaction.publish_retaining_failure(inventory),
        };
        let mut publication = match publication {
            Ok(publication) => publication,
            Err(mut failure) => {
                let primary = failure.take_error();
                let error = self.restore_running_set_after_error(&previously_running, primary);
                drop(failure);
                return Err(error);
            }
        };
        let published_id = publication.id().to_owned();
        self.adopt_generation(specs, genesis);
        let post_commit_check = (|| {
            if published_id != generation_id
                || current_generation_id(self.paths.root())?.as_deref()
                    != Some(generation_id.as_str())
            {
                return Err(SupervisorError::GenerationValidation(
                    "current-generation does not select the committed overlay generation"
                        .to_owned(),
                ));
            }
            let verified = verify_selected_generation(self.paths.root(), &generation_id)?;
            self.ensure_selected_generation_metadata(&verified)
        })();
        let primary_error =
            combine_post_commit_failures(post_commit_check.err(), publication.take_uncertainty());
        if let Some(error) = primary_error {
            let error = self.restore_running_set_after_error(&previously_running, error);
            drop(publication);
            return Err(error);
        }
        let restored = self.restore_captured_running_peers(&previously_running);
        drop(publication);
        restored
    }
    /// Wipe peer storage and regenerate the default genesis manifest.
    ///
    /// If peers were running before this call they are restarted afterwards.
    pub fn wipe_and_regenerate(&mut self) -> Result<()> {
        self.wipe_and_regenerate_inner(None, || {})
    }
    #[cfg(test)]
    pub(super) fn wipe_and_regenerate_with_publication_fault(
        &mut self,
        fault: PublicationFaultPoint,
    ) -> Result<()> {
        self.wipe_and_regenerate_inner(Some(fault), || {})
    }
    #[cfg(test)]
    pub(super) fn wipe_and_regenerate_with_publication_fault_and_failure_hook<F>(
        &mut self,
        fault: PublicationFaultPoint,
        on_precommit_failure: F,
    ) -> Result<()>
    where
        F: FnOnce(),
    {
        self.wipe_and_regenerate_inner(Some(fault), on_precommit_failure)
    }
    fn wipe_and_regenerate_inner<F>(
        &mut self,
        publication_fault: Option<PublicationFaultPoint>,
        on_precommit_failure: F,
    ) -> Result<()>
    where
        F: FnOnce(),
    {
        self.refresh_peer_states();
        let previously_running = self.running_peer_aliases();
        let mut generation_transaction = GenerationTransaction::begin_replacing(
            self.paths.root(),
            Some(self.genesis.generation_id.clone()),
        )?;
        let selected = verify_selected_generation(self.paths.root(), &self.genesis.generation_id)?;
        self.ensure_selected_generation_metadata(&selected)?;
        self.ensure_selected_peer_storage_paths_under_lock(&selected)?;
        let generation_id = generation_transaction.id().to_owned();
        let generation_root = generation_transaction.root().to_path_buf();
        let mut specs = Vec::with_capacity(self.peers.len());
        for peer in &self.peers {
            let storage_dir = generation_transaction.create_runtime_storage(peer.alias())?;
            specs.push(
                peer.spec
                    .in_fresh_generation(&generation_root, storage_dir)?,
            );
        }
        let genesis = GenesisMaterial::create(
            &mut self.binaries,
            GenesisCreateContext {
                generation_id: &generation_id,
                generation_root: &generation_root,
                chain_id: &self.chain_id,
                peers: &specs,
                config_overrides: &self.peer_config_overrides,
                consensus_mode: self.profile.consensus_mode,
                block_cadence_ms: self.profile.signed_block_cadence_ms(),
                genesis_profile: self.genesis.profile,
                vrf_seed_hex: self.genesis.vrf_seed_hex.as_deref(),
                onboarding_authority: &self.onboarding.authority,
            },
        )?;
        for spec in &specs {
            spec.write_config(
                &self.chain_id,
                &genesis,
                &specs,
                &self.peer_config_overrides,
                &[],
            )?;
        }
        genesis.validate_generation(&self.chain_id, &specs)?;
        let expected_hash = genesis.expected_hash.ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "validated generation omitted its exact genesis hash".to_owned(),
            )
        })?;
        if let Err(stop_error) = self.stop_captured_running_peers(&previously_running) {
            return Err(self.restore_running_set_after_error(&previously_running, stop_error));
        }
        let inventory = GenerationInventoryContext {
            chain_id: &self.chain_id,
            chain_discriminant: genesis.chain_discriminant,
            genesis_public_key: genesis.public_key(),
            expected_hash,
        };
        let publication = match publication_fault {
            Some(fault) => {
                generation_transaction.publish_with_fault_retaining_failure(inventory, fault)
            }
            None => generation_transaction.publish_retaining_failure(inventory),
        };
        let mut publication = match publication {
            Ok(publication) => publication,
            Err(mut failure) => {
                let primary = failure.take_error();
                on_precommit_failure();
                let error = self.restore_running_set_after_error(&previously_running, primary);
                drop(failure);
                return Err(error);
            }
        };
        let published_id = publication.id().to_owned();
        self.adopt_generation(specs, genesis);
        let post_commit_check = (|| {
            if published_id != generation_id
                || current_generation_id(self.paths.root())?.as_deref()
                    != Some(generation_id.as_str())
            {
                return Err(SupervisorError::GenerationValidation(
                    "current-generation does not select the committed generation".to_owned(),
                ));
            }
            let verified = verify_selected_generation(self.paths.root(), &generation_id)?;
            self.ensure_selected_generation_metadata(&verified)
        })();
        let primary_error =
            combine_post_commit_failures(post_commit_check.err(), publication.take_uncertainty());
        if let Some(error) = primary_error {
            let error = self.restore_running_set_after_error(&previously_running, error);
            drop(publication);
            return Err(error);
        }
        let restored = self.restore_captured_running_peers(&previously_running);
        drop(publication);
        restored
    }
    fn adopt_generation(&mut self, specs: Vec<PeerSpec>, genesis: GenesisMaterial) {
        debug_assert_eq!(self.peers.len(), specs.len());
        for (peer, spec) in self.peers.iter_mut().zip(specs) {
            peer.replace_spec(spec);
        }
        self.genesis = genesis;
        self.compatibility = None;
    }
}
