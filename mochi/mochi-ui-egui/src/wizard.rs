//! First-run onboarding for the Mochi desktop shell.

use super::*;

impl MochiApp {
    pub(super) fn render_first_run_wizard(&mut self, ctx: &egui::Context) {
        if !self.first_run_wizard.open {
            return;
        }

        egui::Window::new("Welcome to Mochi")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Pick a local sandbox").size(24.0).strong());
                ui.add_space(4.0);
                ui.small(
                    "This is the friendly local-chain cockpit. Choose a topology, point Mochi at a workspace, and launch.",
                );
                ui.add_space(12.0);

                ui.label(RichText::new("Topology").strong());
                ui.horizontal_wrapped(|ui| {
                    for preset in [ProfilePreset::SinglePeer, ProfilePreset::FourPeerBft] {
                        let selected = self.first_run_wizard.preset == preset;
                        let subtitle = match preset {
                            ProfilePreset::SinglePeer => "Fastest loop for app work and demos.",
                            ProfilePreset::FourPeerBft => "Better for consensus debugging and failure drills.",
                            _ => "Use this preset.",
                        };
                        Frame::new()
                            .fill(if selected {
                                Color32::from_rgb(45, 63, 88)
                            } else {
                                Color32::from_rgb(31, 37, 51)
                            })
                            .stroke(Stroke::new(
                                1.0,
                                if selected {
                                    Color32::from_rgb(116, 176, 141)
                                } else {
                                    Color32::from_rgb(76, 95, 126)
                                },
                            ))
                            .corner_radius(CornerRadius::same(14))
                            .inner_margin(Margin::symmetric(14, 12))
                            .show(ui, |ui| {
                                ui.set_width(240.0);
                                ui.label(RichText::new(preset.label()).strong());
                                ui.small(subtitle);
                                ui.add_space(6.0);
                                if ui.selectable_label(selected, "Use this").clicked() {
                                    self.first_run_wizard.preset = preset;
                                }
                            });
                    }
                });

                ui.add_space(12.0);
                ui.label(RichText::new("Workspace").strong());
                ui.text_edit_singleline(&mut self.first_run_wizard.workspace_input);
                ui.small("Mochi writes `.env.local` and `.mochi/generated/*` into this root.");

                ui.add_space(12.0);
                ui.checkbox(
                    &mut self.first_run_wizard.enable_nexus,
                    "Enable Nexus / multi-lane defaults",
                );
                if self.first_run_wizard.enable_nexus {
                    ui.small("This also forces DA on in the generated settings.");
                }

                ui.add_space(16.0);
                if ui
                    .add(
                        Button::new(RichText::new("Launch sandbox").strong())
                            .fill(Color32::from_rgb(116, 176, 141))
                            .min_size(egui::vec2(180.0, 36.0)),
                    )
                    .clicked()
                {
                    self.launch_first_run_wizard();
                }
            });
    }

    fn launch_first_run_wizard(&mut self) {
        let workspace = self.first_run_wizard.workspace_input.trim();
        if workspace.is_empty() {
            self.last_error = Some("Choose a workspace directory before launching.".to_owned());
            return;
        }

        self.settings_data_root_input = workspace.to_owned();
        self.settings_profile_input = self.first_run_wizard.preset.slug().to_owned();
        self.settings_nexus_enabled = self.first_run_wizard.enable_nexus;
        if self.first_run_wizard.enable_nexus {
            self.settings_sumeragi_da_enabled = true;
        }
        self.active_view = ActiveView::Dashboard;
        self.first_run_wizard.completed = true;
        self.first_run_wizard.open = false;
        self.queue_settings_apply(
            true,
            true,
            "Sandbox launched. Dashboard now shows account cards, snippets, and bootstrap files.",
        );
    }
}
