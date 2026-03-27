//! Scenario-first composer affordances.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposerScenarioCard {
    FundAccount,
    CreateTeammate,
    TestMultisig,
    TestImplicitReceive,
    PublishManifest,
}

impl ComposerScenarioCard {
    fn label(self) -> &'static str {
        match self {
            Self::FundAccount => "Fund account",
            Self::CreateTeammate => "Create teammate",
            Self::TestMultisig => "Test multisig",
            Self::TestImplicitReceive => "Test implicit receive",
            Self::PublishManifest => "Publish manifest",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::FundAccount => {
                "Prefill a bundled mint so one signer immediately has spendable sample assets."
            }
            Self::CreateTeammate => "Register a new teammate account in the default domain.",
            Self::TestMultisig => "Seed a realistic multisig proposal and policy pair.",
            Self::TestImplicitReceive => "Demo a transfer that creates an account on receipt.",
            Self::PublishManifest => "Prefill a sample Space Directory manifest registration.",
        }
    }

    fn apply(self, app: &mut MochiApp, signers: &[SigningAuthority]) {
        match self {
            Self::FundAccount => {
                app.apply_composer_template(ComposerTemplate::MintRoseToSigner, signers)
            }
            Self::CreateTeammate => {
                app.apply_composer_template(ComposerTemplate::RegisterAccountForDomain, signers)
            }
            Self::TestMultisig => {
                app.apply_composer_template(ComposerTemplate::MultisigProposeSample, signers)
            }
            Self::TestImplicitReceive => {
                app.apply_composer_template(ComposerTemplate::TransferRoseImplicitReceive, signers)
            }
            Self::PublishManifest => {
                app.apply_composer_template(ComposerTemplate::SpaceDirectoryAxtTouch, signers)
            }
        }
    }

    fn all() -> [Self; 5] {
        [
            Self::FundAccount,
            Self::CreateTeammate,
            Self::TestMultisig,
            Self::TestImplicitReceive,
            Self::PublishManifest,
        ]
    }
}

impl MochiApp {
    pub(super) fn render_composer_mode_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Composer lane").strong());
            for mode in [ComposerMode::Scenarios, ComposerMode::Advanced] {
                let selected = self.composer_mode == mode;
                if ui
                    .selectable_label(selected, mode.label())
                    .on_hover_text(match mode {
                        ComposerMode::Scenarios => {
                            "Scenario-first presets for the common Ganache-style dev loop."
                        }
                        ComposerMode::Advanced => {
                            "Raw instruction taxonomy and the full builder surface."
                        }
                    })
                    .clicked()
                {
                    self.composer_mode = mode;
                }
            }
        });
        ui.add_space(6.0);
    }

    pub(super) fn render_composer_scenario_cards(
        &mut self,
        ui: &mut egui::Ui,
        signers: &[SigningAuthority],
    ) {
        ui.label(RichText::new("Scenario quickstarts").strong());
        ui.small("Pick a story first, then tweak the generated drafts below if you need to.");
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            for scenario in ComposerScenarioCard::all() {
                let frame = Frame::new()
                    .fill(Color32::from_rgb(34, 41, 56))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(76, 95, 126)))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::symmetric(14, 12));
                frame.show(ui, |ui| {
                    ui.set_width(210.0);
                    ui.label(RichText::new(scenario.label()).strong());
                    ui.add_space(4.0);
                    ui.small(scenario.description());
                    ui.add_space(8.0);
                    if ui.button("Load scenario").clicked() {
                        scenario.apply(self, signers);
                    }
                });
            }
        });
        ui.add_space(8.0);
    }
}
