//! Chaos lab UI and background scheduling.

use super::*;

impl MochiApp {
    pub(super) fn poll_chaos_updates(&mut self, supervisor_slot: &mut Option<Supervisor>) {
        loop {
            match self.chaos_rx.try_recv() {
                Ok(ChaosUpdate::Event { message }) => {
                    self.chaos_log.push(message);
                    if self.chaos_log.len() > 80 {
                        let drain = self.chaos_log.len() - 80;
                        self.chaos_log.drain(0..drain);
                    }
                }
                Ok(ChaosUpdate::Finished {
                    supervisor,
                    report,
                    error,
                }) => {
                    *supervisor_slot = Some(supervisor);
                    self.chaos_inflight = false;
                    self.chaos_cancel = None;
                    self.chaos_report = Some(report);
                    match error {
                        Some(error) => {
                            self.last_info = None;
                            self.last_error = Some(error);
                        }
                        None => {
                            self.last_error = None;
                            self.last_info = Some("Chaos scenario completed.".to_owned());
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }

    pub(super) fn schedule_pending_chaos(&mut self, supervisor_slot: &mut Option<Supervisor>) {
        if self.chaos_inflight {
            return;
        }
        let Some(request) = self.chaos_pending_request.take() else {
            return;
        };
        let Some(supervisor) = supervisor_slot.take() else {
            self.chaos_pending_request = Some(request);
            return;
        };

        let tx = self.chaos_tx.clone();
        let handle = self.runtime.handle().clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.chaos_cancel = Some(cancel.clone());
        self.chaos_inflight = true;
        self.chaos_log.clear();
        self.chaos_report = None;

        self.runtime.spawn_blocking(move || {
            let run = run_chaos_preset(supervisor, &handle, request, &cancel, |event| {
                let _ = tx.send(ChaosUpdate::Event {
                    message: event.message,
                });
            });
            let _ = tx.send(ChaosUpdate::Finished {
                supervisor: run.supervisor,
                report: run.report,
                error: run.error.map(|err| err.to_string()),
            });
        });
    }

    pub(super) fn render_chaos_view(
        &mut self,
        ui: &mut egui::Ui,
        _supervisor: &mut Supervisor,
        peer_aliases: &[String],
    ) {
        ui.label(RichText::new("Chaos Lab").strong());
        ui.small(
            "Run guided Izanami-backed drills against the current Mochi sandbox. Control returns when the scenario finishes.",
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Target peer");
            ComboBox::from_id_salt("mochi_chaos_peer")
                .selected_text(
                    self.chaos_selected_peer
                        .clone()
                        .unwrap_or_else(|| "Select peer".to_owned()),
                )
                .show_ui(ui, |ui| {
                    for alias in peer_aliases {
                        let selected = self.chaos_selected_peer.as_deref() == Some(alias.as_str());
                        if ui.selectable_label(selected, alias).clicked() {
                            self.chaos_selected_peer = Some(alias.clone());
                        }
                    }
                });
            ui.label("Duration");
            ui.add(
                egui::DragValue::new(&mut self.chaos_duration_ms)
                    .range(1_000..=60_000)
                    .speed(100.0)
                    .suffix(" ms"),
            );
        });
        ui.add_space(10.0);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            for preset in ChaosPreset::all() {
                Frame::new()
                    .fill(if self.chaos_selected_preset == preset {
                        Color32::from_rgb(45, 63, 88)
                    } else {
                        Color32::from_rgb(34, 41, 56)
                    })
                    .stroke(Stroke::new(1.0, Color32::from_rgb(76, 95, 126)))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        ui.set_width(210.0);
                        ui.label(RichText::new(preset.label()).strong());
                        ui.small(preset.description());
                        ui.add_space(8.0);
                        if ui
                            .selectable_label(self.chaos_selected_preset == preset, "Select")
                            .clicked()
                        {
                            self.chaos_selected_preset = preset;
                            self.chaos_duration_ms = preset.default_duration().as_millis() as u64;
                        }
                    });
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.chaos_inflight, Button::new("Run scenario"))
                .clicked()
            {
                let Some(peer_alias) = self.chaos_selected_peer.clone() else {
                    self.last_error =
                        Some("Select a peer before starting a chaos scenario.".to_owned());
                    return;
                };
                self.chaos_pending_request = Some(ChaosRunRequest {
                    preset: self.chaos_selected_preset,
                    peer_alias,
                    duration: Duration::from_millis(self.chaos_duration_ms.max(1_000)),
                    seed: current_unix_timestamp_ms(),
                });
            }
            if ui
                .add_enabled(
                    self.chaos_inflight,
                    Button::new("Cancel after current action"),
                )
                .clicked()
            {
                if let Some(cancel) = self.chaos_cancel.as_ref() {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        });

        if self.chaos_inflight {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Scenario in progress");
                ui.add(egui::Spinner::new());
            });
        }

        if let Some(report) = &self.chaos_report {
            ui.add_space(8.0);
            ui.label(RichText::new("Last report").strong());
            ui.small(format!(
                "{} on {} • cancelled={}",
                report.preset.label(),
                report.peer_alias,
                report.cancelled
            ));
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Run log").strong());
        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            if self.chaos_log.is_empty() {
                ui.small("No chaos events yet.");
            } else {
                for line in &self.chaos_log {
                    ui.monospace(line);
                }
            }
        });
    }
}
