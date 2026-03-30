//! Account-first dashboard rendering.

use super::*;

impl MochiApp {
    pub(super) fn poll_dashboard_updates(&mut self) {
        loop {
            match self.dashboard_rx.try_recv() {
                Ok(update) => {
                    self.dashboard_inflight = false;
                    match update.result {
                        Ok(snapshot) => {
                            self.dashboard_snapshot = Some(snapshot);
                            self.dashboard_error = None;
                        }
                        Err(err) => {
                            self.dashboard_error = Some(err.clone());
                            self.last_error = Some(err);
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }

    fn spawn_dashboard_refresh(&mut self, supervisor: &Supervisor, peer_rows: &[PeerRow]) {
        if self.dashboard_inflight {
            return;
        }
        let Some(row) = Self::recipe_peer(peer_rows) else {
            return;
        };
        let Some(client) = supervisor.torii_client(&row.alias) else {
            self.dashboard_error =
                Some("Torii client unavailable for dashboard refresh.".to_owned());
            return;
        };
        let signers = supervisor.signers().to_vec();
        let peer = row.alias.clone();
        let tx = self.dashboard_tx.clone();
        self.dashboard_inflight = true;
        self.runtime.spawn(async move {
            let result = fetch_dashboard_snapshot(peer.clone(), &client, &signers)
                .await
                .map_err(|err| match err.detail {
                    Some(detail) => format!("{} ({detail})", err.message),
                    None => err.message,
                });
            let _ = tx.send(DashboardUpdate { result });
        });
    }

    fn bootstrap_inputs(
        &self,
        supervisor: &Supervisor,
        peer_rows: &[PeerRow],
    ) -> Option<BootstrapInputs> {
        let peer = Self::recipe_peer(peer_rows)?;
        let signer = supervisor.signers().first();
        let account_id = signer.map(|entry| account_literal(entry.account_id()));
        let private_key = signer
            .map(|entry| ExposedPrivateKey(entry.key_pair().private_key().clone()).to_string());
        Some(BootstrapInputs {
            api_base: Self::peer_api_base(peer),
            torii_url: peer.torii.clone(),
            chain_id: self.effective_chain_id_recipe(supervisor),
            account_id,
            private_key,
        })
    }

    fn write_bootstrap_files(&mut self, supervisor: &Supervisor, peer_rows: &[PeerRow]) {
        let Some(inputs) = self.bootstrap_inputs(supervisor, peer_rows) else {
            self.last_error = Some("No peer available to render bootstrap files.".to_owned());
            return;
        };
        let root = PathBuf::from(self.effective_workspace_recipe(supervisor));
        let bundle = BootstrapBundle::render(&inputs);
        match write_bootstrap_bundle(&root, &bundle, self.bootstrap_replace_existing) {
            Ok(paths) => {
                self.last_error = None;
                self.last_info = Some(format!(
                    "Wrote {} bootstrap files under {}.",
                    paths.len(),
                    root.display()
                ));
            }
            Err(BootstrapWriteError::AlreadyExists { path }) => {
                self.last_info = None;
                self.last_error = Some(format!(
                    "{} already exists. Enable “Replace existing files” to overwrite it.",
                    path.display()
                ));
            }
            Err(err) => {
                self.last_info = None;
                self.last_error = Some(format!("Failed to write bootstrap files: {err}"));
            }
        }
    }

    pub(super) fn render_dashboard_view(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_rows: &[PeerRow],
        _peer_aliases: &[String],
        metrics: &DashboardMetrics,
    ) {
        if self.dashboard_snapshot.is_none() && !self.dashboard_inflight {
            self.spawn_dashboard_refresh(supervisor, peer_rows);
        }

        let running = supervisor.is_any_running();
        let workspace_root = self.effective_workspace_recipe(supervisor);
        let snapshot = self.dashboard_snapshot.clone();

        Frame::new()
            .fill(Color32::from_rgb(31, 37, 51))
            .stroke(Stroke::new(1.0, Color32::from_rgb(76, 95, 126)))
            .corner_radius(CornerRadius::same(18))
            .inner_margin(Margin::symmetric(18, 16))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Friendly local chain").size(24.0).strong());
                        ui.small(
                            "Prefunded accounts, bootstrap files, and the most common dev tasks should all be one click away.",
                        );
                        ui.add_space(8.0);
                        ui.small(format!(
                            "Workspace: {} • Peers running: {} • Latest height: {}",
                            workspace_root,
                            metrics.peers_label(),
                            metrics.latest_height_text()
                        ));
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                        if ui.button("Refresh").clicked() {
                            self.spawn_dashboard_refresh(supervisor, peer_rows);
                        }
                        if running {
                            if ui.button("Stop sandbox").clicked() {
                                match supervisor.stop_all() {
                                    Ok(()) => {
                                        self.last_info = Some("Stopped all peers.".to_owned());
                                        self.last_error = None;
                                    }
                                    Err(err) => self.last_error = Some(err.to_string()),
                                }
                            }
                        } else if ui.button("Launch sandbox").clicked() {
                            match supervisor.start_all() {
                                Ok(()) => {
                                    self.last_info = Some("Launched the local sandbox.".to_owned());
                                    self.last_error = None;
                                }
                                Err(err) => self.last_error = Some(err.to_string()),
                            }
                        }
                    });
                });

                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if let Some(inputs) = self.bootstrap_inputs(supervisor, peer_rows)
                        && ui.button("Copy shell env").clicked()
                    {
                        Self::copy_text(ui, inputs.render_shell_exports());
                        self.last_info = Some("Copied local app bootstrap exports.".to_owned());
                    }
                    if ui.button("Write bootstrap files").clicked() {
                        self.write_bootstrap_files(supervisor, peer_rows);
                    }
                    ui.checkbox(&mut self.bootstrap_replace_existing, "Replace existing files");
                    if ui.button("Open latest blocks").clicked() {
                        self.active_view = ActiveView::Activity;
                        self.activity_view = ActivityView::Blocks;
                    }
                    if ui.button("Fund account").clicked() {
                        self.apply_composer_template(
                            ComposerTemplate::MintRoseToSigner,
                            supervisor.signers(),
                        );
                        self.active_view = ActiveView::Composer;
                    }
                    if ui.button("Test multisig").clicked() {
                        self.apply_composer_template(
                            ComposerTemplate::MultisigProposeSample,
                            supervisor.signers(),
                        );
                        self.active_view = ActiveView::Composer;
                    }
                    if ui.button("Reset chain").clicked()
                        && self.begin_maintenance(MaintenanceTask::Reset)
                    {
                        self.maintenance_command = Some(MaintenanceCommand::Reset);
                    }
                });
            });

        ui.add_space(12.0);
        ui.columns(2, |columns| {
            columns[0].label(RichText::new("Dev accounts").strong());
            columns[0].add_space(6.0);

            if let Some(error) = &self.dashboard_error {
                columns[0].colored_label(Color32::from_rgb(200, 64, 64), error);
                columns[0].add_space(6.0);
            }

            if let Some(snapshot) = snapshot.as_ref() {
                for (index, card) in snapshot.accounts.iter().enumerate() {
                    Frame::new()
                        .fill(Color32::from_rgb(34, 41, 56))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(76, 95, 126)))
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin::symmetric(14, 12))
                        .show(&mut columns[0], |ui| {
                            ui.label(RichText::new(&card.label).strong());
                            ui.small(&card.account_id);
                            if let Some(address) = &card.i105_address {
                                ui.small(format!("i105: {address}"));
                            }
                            if !card.balances.is_empty() {
                                ui.add_space(6.0);
                                for balance in card.balances.iter().take(4) {
                                    ui.small(format!(
                                        "{} • {}",
                                        balance.definition_id, balance.value
                                    ));
                                }
                            } else {
                                ui.small("No explorer balances yet.");
                            }
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Mint sample asset").clicked() {
                                    self.composer_selected_signer = Some(index);
                                    self.apply_composer_template(
                                        ComposerTemplate::MintRoseToSigner,
                                        supervisor.signers(),
                                    );
                                    self.active_view = ActiveView::Composer;
                                }
                                if ui.button("Transfer to teammate").clicked() {
                                    self.composer_selected_signer = Some(index);
                                    self.apply_composer_template(
                                        ComposerTemplate::TransferRoseToTeammate,
                                        supervisor.signers(),
                                    );
                                    self.active_view = ActiveView::Composer;
                                }
                            });
                        });
                    columns[0].add_space(8.0);
                }
            } else {
                for signer in supervisor.signers() {
                    columns[0].group(|ui| {
                        ui.label(RichText::new(signer.label()).strong());
                        ui.small(account_literal(signer.account_id()));
                        ui.small("Waiting for explorer data…");
                    });
                    columns[0].add_space(6.0);
                }
            }

            columns[1].label(RichText::new("Recent blocks").strong());
            columns[1].add_space(6.0);
            if let Some(snapshot) = snapshot.as_ref() {
                for block in &snapshot.recent_blocks {
                    Frame::new()
                        .fill(Color32::from_rgb(34, 41, 56))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(76, 95, 126)))
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(Margin::symmetric(12, 10))
                        .show(&mut columns[1], |ui| {
                            ui.label(RichText::new(format!("Block {}", block.height)).strong());
                            ui.small(&block.created_at);
                            ui.small(format!(
                                "{} tx • {} rejected",
                                block.transactions_total, block.transactions_rejected
                            ));
                        });
                    columns[1].add_space(8.0);
                }
            } else if self.dashboard_inflight {
                columns[1].label("Loading explorer data…");
            } else {
                columns[1].label("No block summary yet.");
            }
        });
    }
}
