//! Build views: the transaction composer, the shared signed-transaction review, contract
//! deployment and package publication.

use super::{
    Desktop, Message, PublicationAction, PublicationForm, PublicationOutput, View,
    accounts::{self, LedgerSnapshot, MintRight, QuickKind},
    widgets::{self, icon},
};
use eframe::egui::{self, RichText, Ui};
use mochi_core::{
    DomainLease, FeeLimit, InstructionDraft, TransactionPreview, developer::SubmissionOutcome,
    drafts_from_json_str, drafts_to_pretty_json,
};

/// Instruction builder choices, in their segmented-control order.
const ACTIONS: [&str; 4] = ["Mint", "Burn", "Transfer", "Register account"];

/// The Contracts view's call/view form; it survives view switches but not network changes.
#[derive(Clone, Debug)]
pub(super) struct CallForm {
    pub(super) alias: String,
    pub(super) entrypoint: String,
    pub(super) arguments: String,
    /// Fee cap in the generated fee asset, as typed.
    pub(super) max_fee: String,
    /// The last result, or why it failed.
    pub(super) output: Option<Result<String, String>>,
}

impl Default for CallForm {
    fn default() -> Self {
        Self {
            alias: String::new(),
            entrypoint: String::new(),
            arguments: "{}".into(),
            // Generous on a localnet: gas at Kagami's default budget fits well inside it.
            max_fee: "100".into(),
            output: None,
        }
    }
}

/// A JSON object with one placeholder per declared parameter, e.g. `{"start": "0"}`.
pub(super) fn argument_template<'a>(
    parameters: impl Iterator<Item = (&'a str, &'a str)>,
) -> String {
    let fields = parameters
        .map(|(name, type_name)| {
            // The verified entrypoint schema takes numbers as canonical decimal strings.
            let placeholder = match type_name {
                "int" | "decimal" | "quantity" => "\"0\"",
                "bool" => "false",
                "DataSpaceId" => "0",
                "Json" => "null",
                _ => "\"\"",
            };
            format!(
                "{}: {placeholder}",
                norito::json::to_string(&name).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    if fields.is_empty() {
        "{}".into()
    } else {
        format!("{{{}}}", fields.join(", "))
    }
}

/// The single submission of a reviewed transaction and how the network settled it.
#[derive(Clone, Debug)]
pub(super) struct Submission {
    /// Original hash of the submitted transaction.
    pub(super) hash: String,
    /// `None` until the network settles it; `Err` when Mochi could not confirm the outcome.
    pub(super) outcome: Option<Result<SubmissionOutcome, String>>,
}

/// Where the reviewed transaction stands, as its review card shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ReviewStatus {
    /// Signed with quoted fee limits; nothing was submitted.
    Signed,
    /// Submitted once; the network has not settled it yet.
    Submitting,
    Applied(u64),
    Rejected(String),
    Expired,
    /// Mochi could not confirm what happened to it.
    Unconfirmed(String),
}

impl ReviewStatus {
    fn of(submission: Option<&Submission>) -> Self {
        match submission.map(|submission| &submission.outcome) {
            None => Self::Signed,
            Some(None) => Self::Submitting,
            Some(Some(Ok(SubmissionOutcome::Applied { block_height }))) => {
                Self::Applied(*block_height)
            }
            Some(Some(Ok(SubmissionOutcome::Rejected { reason }))) => {
                Self::Rejected(reason.clone())
            }
            Some(Some(Ok(SubmissionOutcome::Expired))) => Self::Expired,
            Some(Some(Err(error))) => Self::Unconfirmed(error.clone()),
        }
    }
}

/// What the review card's buttons ask for.
enum ReviewAction {
    Submit,
    Close,
    CreateToken,
    /// Send the developer's balance of this definition instead of minting it.
    SendInstead(String),
}

/// Why a mint in the reviewed transaction cannot apply: another account owns the token.
struct MintAdvice {
    definition: String,
    token: String,
    owner: String,
    /// The developer holds some of it and could send it instead.
    held: bool,
}

/// The first mint in `preview` of a token that `developer` does not own.
fn mint_advice(
    preview: &TransactionPreview,
    ledger: Option<&LedgerSnapshot>,
    developer: &str,
) -> Option<MintAdvice> {
    let ledger = ledger?;
    preview.drafts().iter().find_map(|draft| {
        let InstructionDraft::MintAsset { asset, .. } = draft else {
            return None;
        };
        let definition = asset.definition().to_string();
        (ledger.mint_right(&definition, developer) == MintRight::NotOwner).then(|| MintAdvice {
            token: ledger.asset_label(&definition),
            owner: ledger
                .owner_of(&definition)
                .map(|owner| widgets::short_middle(owner, 6))
                .unwrap_or_default(),
            held: ledger
                .holdings_of(developer)
                .iter()
                .any(|holding| holding.definition == definition),
            definition,
        })
    })
}

/// The review card's lines: each signed instruction in plain words and, when that differs,
/// its exact identifiers.
fn review_lines(
    preview: &TransactionPreview,
    ledger: Option<&LedgerSnapshot>,
    developer: &str,
) -> Vec<(String, Option<String>)> {
    let instructions = preview.instructions();
    let drafts = preview.drafts();
    // Drafts are the trailing instructions; planner-made ones such as a lease come first.
    let (lead, described) = if drafts.len() <= instructions.len() {
        (
            instructions.len() - drafts.len(),
            accounts::describe_drafts(drafts, ledger, developer),
        )
    } else {
        (instructions.len(), Vec::new())
    };
    instructions
        .iter()
        .enumerate()
        .map(|(index, signed)| {
            // Summaries carry the exact identifiers; the signed text is only a fallback.
            let exact = preview
                .summaries()
                .get(index)
                .cloned()
                .unwrap_or_else(|| signed.clone());
            let plain = if index >= lead {
                described.get(index - lead).cloned()
            } else if index == 0
                && let Some(lease) = preview.lease()
            {
                Some(lease_line(lease, ledger))
            } else {
                None
            }
            .unwrap_or_else(|| exact.clone());
            let exact = (plain != exact).then_some(exact);
            (plain, exact)
        })
        .collect()
}

/// The definition a reviewed transaction registers, which becomes the token to mint next.
pub(super) fn created_token(preview: &TransactionPreview) -> Option<String> {
    preview.drafts().iter().find_map(|draft| match draft {
        InstructionDraft::RegisterAssetDefinition { definition, .. } => {
            Some(definition.to_string())
        }
        _ => None,
    })
}

/// A token's display label, or its shortened literal before the ledger is observed.
fn token_label(definition: &str, ledger: Option<&LedgerSnapshot>) -> String {
    ledger.map_or_else(
        || widgets::short_middle(definition, 4),
        |ledger| ledger.asset_label(definition),
    )
}

/// One plain line for a domain lease, such as `Lease domain coffee.universal for 1 year · 0.5 XOR`.
fn lease_line(lease: &DomainLease, ledger: Option<&LedgerSnapshot>) -> String {
    match &lease.amount {
        Some(amount) => format!(
            "Lease domain {} for {} year{} · {amount} {}",
            lease.domain,
            lease.years,
            if lease.years == 1 { "" } else { "s" },
            token_label(&lease.asset_definition, ledger)
        ),
        None => format!("Restore domain {} · no charge", lease.domain),
    }
}

/// One plain line for a signed fee limit, such as `Network fee · up to 0.0085 XOR`.
fn fee_line(fee: &FeeLimit, ledger: Option<&LedgerSnapshot>) -> String {
    format!(
        "{} · up to {} {}",
        fee.label,
        fee.max_amount,
        token_label(&fee.asset_definition, ledger)
    )
}

/// A tinted panel for a submission outcome.
fn outcome_frame(ui: &mut Ui, fill: egui::Color32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui);
        });
}

/// How the Resume picker names a saved deployment: its alias and whether it finished.
fn saved_label(deployment: &mochi_core::developer::SavedDeployment) -> String {
    let outcome = if deployment.has_receipt {
        "applied"
    } else {
        "not finished"
    };
    format!("{} · {outcome}", deployment.alias)
}

/// File names and extensions Mochi accepts as contract deployment input.
pub(crate) fn is_contract_input(path: &std::path::Path) -> bool {
    path.is_dir()
        || path.file_name().is_some_and(|name| name == "Musubi.toml")
        || path
            .extension()
            .is_some_and(|extension| extension == "ko" || extension == "to")
}

impl Desktop {
    pub(super) fn composer(&mut self, ui: &mut Ui) {
        let _address_profile = self.selected.as_ref().map(|selected| {
            iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                selected.network.address_discriminant(),
            )
        });
        let palette = widgets::palette(ui);
        widgets::page_title(
            ui,
            "Transactions",
            "Build instructions, review the exact signed transaction, then submit it once.",
        );
        widgets::card(ui, |ui| {
            widgets::card_title(ui, icon::COMPOSE, "Instruction builder");
            ui.add_space(4.0);
            let options = ACTIONS
                .iter()
                .enumerate()
                .map(|(index, label)| (index, *label))
                .collect::<Vec<_>>();
            widgets::segmented(ui, &mut self.action, &options);
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.asset)
                        .hint_text(if self.action == 3 {
                            "Account ID"
                        } else {
                            "Asset ID"
                        })
                        .desired_width(420.0),
                );
                if self.action != 3 {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.quantity)
                            .hint_text("Quantity")
                            .desired_width(110.0),
                    );
                }
            });
            if self.action == 2 {
                ui.add(
                    egui::TextEdit::singleline(&mut self.destination)
                        .hint_text("Destination account ID")
                        .desired_width(540.0),
                );
            }
            if ui
                .button(format!("{}  Add instruction", icon::NEW))
                .clicked()
            {
                self.add_instruction();
            }
        });
        ui.add_space(12.0);
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::card_title(ui, icon::LOGS, "Draft");
                let count = drafts_from_json_str(&self.drafts).map_or(0, |drafts| drafts.len());
                widgets::pill(
                    ui,
                    format!("{count} instruction{}", if count == 1 { "" } else { "s" }),
                    palette.muted,
                    palette.inset,
                );
            });
            widgets::hint(
                ui,
                "Draft JSON also supports asset definitions, roles, multisig and chain policy instructions.",
            );
            if ui
                .add(
                    egui::TextEdit::multiline(&mut self.drafts)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(12),
                )
                .changed()
            {
                self.preview = None;
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.busy && self.ready(),
                        widgets::primary_button(palette, "Review transaction"),
                    )
                    .clicked()
                {
                    self.review_drafts();
                }
                if ui.button("Clear draft").clicked() {
                    self.drafts = "[]".into();
                    self.preview = None;
                }
            });
        });
        ui.add_space(12.0);
        let ledger = self.ledger.take();
        self.transaction_preview(ui, ledger.as_ref());
        if self.ledger.is_none() {
            self.ledger = ledger;
        }
    }

    fn add_instruction(&mut self) {
        let draft = match self.action {
            0 => InstructionDraft::mint_from_input(&self.asset, &self.quantity),
            1 => InstructionDraft::burn_from_input(&self.asset, &self.quantity),
            2 => InstructionDraft::transfer_from_input(
                &self.asset,
                &self.quantity,
                &self.destination,
            ),
            _ => InstructionDraft::register_account_from_input(&self.asset),
        };
        let result = draft.and_then(|draft| {
            let mut drafts = drafts_from_json_str(&self.drafts)?;
            drafts.push(draft);
            drafts_to_pretty_json(&drafts)
        });
        match result {
            Ok(drafts) => {
                self.drafts = drafts;
                self.preview = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn review_drafts(&mut self) {
        match drafts_from_json_str(&self.drafts) {
            Ok(drafts) => self.request_preview(drafts, None),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Sign `drafts` with the fee limits the observed validator quotes, in the background.
    ///
    /// The review appears when the quote returns; `drafts_json` replaces the composer text for
    /// quick actions. Nothing is submitted until the reviewed transaction is confirmed.
    pub(super) fn request_preview(
        &mut self,
        drafts: Vec<InstructionDraft>,
        drafts_json: Option<String>,
    ) {
        let (Some(selected), Some(handle)) = (
            &self.selected,
            self.runtime
                .as_ref()
                .map(|runtime| runtime.handle().clone()),
        ) else {
            return;
        };
        let network = selected.network.clone();
        let peer = self.peer;
        self.preview = None;
        self.spawn(move || Message::Previewed {
            result: handle
                .block_on(network.preview(peer, &drafts))
                .map_err(|error| error.to_string()),
            drafts: drafts_json,
        });
    }

    /// Plan and sign a new token's domain lease, registration and first mint, in the
    /// background; the review appears once the name service and the validator have quoted it.
    pub(super) fn request_token_preview(&mut self, name: String, label: String, supply: String) {
        let (Some(selected), Some(handle)) = (
            &self.selected,
            self.runtime
                .as_ref()
                .map(|runtime| runtime.handle().clone()),
        ) else {
            return;
        };
        let network = selected.network.clone();
        let peer = self.peer;
        self.preview = None;
        self.spawn(move || Message::Previewed {
            result: handle
                .block_on(network.preview_token(peer, &name, &label, &supply))
                .map_err(|error| error.to_string()),
            drafts: None,
        });
    }

    /// The reviewed, signed transaction shared by Accounts and Transactions, with how its
    /// single submission settled.
    pub(super) fn transaction_preview(&mut self, ui: &mut Ui, ledger: Option<&LedgerSnapshot>) {
        let Some(preview) = &self.preview else {
            return;
        };
        let palette = widgets::palette(ui);
        let developer = self
            .selected
            .as_ref()
            .map(|selected| selected.network.prepared().context.account_id.clone())
            .unwrap_or_default();
        let status = ReviewStatus::of(
            self.submission
                .as_ref()
                .filter(|submission| submission.hash == preview.hash()),
        );
        let lines = review_lines(preview, ledger, &developer);
        let advice = mint_advice(preview, ledger, &developer);
        let (accent, pill, pill_fg, pill_bg) = match &status {
            ReviewStatus::Signed => (
                palette.sakura,
                "signed · not submitted".to_owned(),
                palette.kinako,
                palette.kinako_soft,
            ),
            ReviewStatus::Submitting => (
                palette.sakura,
                "submitting…".to_owned(),
                palette.sky,
                palette.sky_soft,
            ),
            ReviewStatus::Applied(height) => (
                palette.matcha,
                format!("applied · block {height}"),
                palette.matcha,
                palette.matcha_soft,
            ),
            ReviewStatus::Rejected(_) => (
                palette.aka,
                "rejected".to_owned(),
                palette.aka,
                palette.aka_soft,
            ),
            ReviewStatus::Expired => (
                palette.aka,
                "expired".to_owned(),
                palette.aka,
                palette.aka_soft,
            ),
            ReviewStatus::Unconfirmed(_) => (
                palette.kinako,
                "outcome unknown".to_owned(),
                palette.kinako,
                palette.kinako_soft,
            ),
        };
        let glyph = match &status {
            ReviewStatus::Rejected(_) => icon::ERROR,
            ReviewStatus::Expired | ReviewStatus::Unconfirmed(_) => icon::WARNING,
            _ => icon::OK,
        };
        let mut action = None;
        let card = widgets::accent_card(ui, Some(accent), |ui| {
            ui.horizontal(|ui| {
                widgets::card_title(ui, glyph, "Review transaction");
                if status == ReviewStatus::Submitting {
                    ui.spinner();
                }
                widgets::pill(ui, pill, pill_fg, pill_bg);
            });
            ui.add_space(4.0);
            widgets::copy_field(ui, "Authority", preview.authority());
            widgets::copy_field(ui, "Transaction", preview.hash());
            ui.label(widgets::eyebrow(palette, "What it does"));
            for (index, (plain, exact)) in lines.iter().enumerate() {
                ui.horizontal_top(|ui| {
                    ui.label(RichText::new(format!("{}.", index + 1)).color(palette.faint));
                    ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(RichText::new(plain).strong().color(palette.text))
                                .wrap(),
                        );
                        if let Some(exact) = exact {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(exact)
                                        .small()
                                        .monospace()
                                        .color(palette.faint),
                                )
                                .wrap(),
                            )
                            .on_hover_text("Exact identifiers in the signed instruction");
                        }
                    });
                });
            }
            if !preview.fees().is_empty() {
                ui.label(widgets::eyebrow(palette, "Signed fee limits"));
                for fee in preview.fees() {
                    ui.add(
                        egui::Label::new(RichText::new(fee_line(fee, ledger)).color(palette.text))
                            .wrap(),
                    )
                    .on_hover_text(&fee.asset_definition);
                }
            }
            ui.add_space(6.0);
            match &status {
                ReviewStatus::Signed => {
                    widgets::hint(
                        ui,
                        "Signed with the quoted fee limits. Nothing reaches the network until you submit it, once.",
                    );
                }
                ReviewStatus::Submitting => {
                    widgets::hint(ui, "Submitted once. Waiting for a block to settle it…");
                }
                ReviewStatus::Applied(height) => {
                    ui.label(
                        RichText::new(format!(
                            "{} Applied in block {height}. Balances refresh on their own.",
                            icon::OK
                        ))
                        .color(palette.matcha),
                    );
                }
                ReviewStatus::Rejected(reason) => {
                    outcome_frame(ui, palette.aka_soft, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} Rejected: nothing in this transaction applied.",
                                icon::ERROR
                            ))
                            .strong()
                            .color(palette.aka),
                        );
                        ui.add(egui::Label::new(RichText::new(reason).color(palette.text)).wrap());
                        match &advice {
                            Some(advice) => {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!(
                                            "Only {}'s owner ({}) can mint it, or an account that owner grants a mint permission. Create a token of your own to mint, or send {} from your balance.",
                                            advice.token, advice.owner, advice.token
                                        ))
                                        .color(palette.muted),
                                    )
                                    .wrap(),
                                );
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .add(widgets::tonal_button(
                                            &format!("{}  Create your own token", icon::COINS),
                                            palette.on_sakura,
                                            palette.sakura,
                                        ))
                                        .clicked()
                                    {
                                        action = Some(ReviewAction::CreateToken);
                                    }
                                    if advice.held
                                        && ui
                                            .add(widgets::tonal_button(
                                                &format!(
                                                    "{}  Send {} instead",
                                                    icon::SEND,
                                                    advice.token
                                                ),
                                                palette.text,
                                                palette.surface,
                                            ))
                                            .clicked()
                                    {
                                        action = Some(ReviewAction::SendInstead(
                                            advice.definition.clone(),
                                        ));
                                    }
                                });
                            }
                            None => {
                                widgets::hint(
                                    ui,
                                    "Change what the network refused, then review again.",
                                );
                            }
                        }
                        ui.label(
                            RichText::new("Fees for the attempt may still be charged.")
                                .small()
                                .color(palette.faint),
                        );
                    });
                }
                ReviewStatus::Expired => {
                    outcome_frame(ui, palette.kinako_soft, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "{} Expired before any block included it: nothing applied. Review it again to sign a fresh transaction.",
                                    icon::WARNING
                                ))
                                .color(palette.text),
                            )
                            .wrap(),
                        );
                    });
                }
                ReviewStatus::Unconfirmed(error) => {
                    outcome_frame(ui, palette.kinako_soft, |ui| {
                        ui.label(
                            RichText::new(format!("{} Outcome not confirmed", icon::WARNING))
                                .strong()
                                .color(palette.text),
                        );
                        ui.add(egui::Label::new(RichText::new(error).color(palette.text)).wrap());
                        widgets::hint(
                            ui,
                            "It may still apply. Look up this transaction hash before signing a replacement for it.",
                        );
                    });
                }
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                match &status {
                    ReviewStatus::Signed => {
                        if ui
                            .add_enabled(
                                !self.busy,
                                widgets::primary_button(palette, "Submit reviewed transaction"),
                            )
                            .clicked()
                        {
                            action = Some(ReviewAction::Submit);
                        }
                    }
                    ReviewStatus::Applied(_) => {
                        if ui.add(widgets::primary_button(palette, "Done")).clicked() {
                            action = Some(ReviewAction::Close);
                        }
                    }
                    ReviewStatus::Rejected(_) | ReviewStatus::Expired => {
                        if ui.button("Edit and review again").clicked() {
                            action = Some(ReviewAction::Close);
                        }
                    }
                    ReviewStatus::Submitting | ReviewStatus::Unconfirmed(_) => {}
                }
                if ui.button("Copy signed Norito hex").clicked() {
                    widgets::copy_to_clipboard(
                        ui.ctx(),
                        preview.encoded_hex(),
                        "signed Norito hex",
                    );
                }
                if matches!(status, ReviewStatus::Signed | ReviewStatus::Unconfirmed(_))
                    && ui
                        .add_enabled(!self.busy, egui::Button::new("Discard"))
                        .clicked()
                {
                    action = Some(ReviewAction::Close);
                }
            });
        });
        if std::mem::take(&mut self.reveal_review) {
            card.response.scroll_to_me(Some(egui::Align::Center));
        }
        match action {
            Some(ReviewAction::Submit) => self.submit_reviewed(),
            Some(ReviewAction::Close) => {
                self.preview = None;
                self.submission = None;
            }
            Some(ReviewAction::CreateToken) => {
                self.quick.kind = QuickKind::NewToken;
                self.preview = None;
                self.submission = None;
                self.view = View::Accounts;
            }
            Some(ReviewAction::SendInstead(definition)) => {
                if let Some(ledger) = ledger {
                    self.quick.send_instead(ledger, &developer, &definition);
                }
                self.preview = None;
                self.submission = None;
                self.view = View::Accounts;
            }
            None => {}
        }
    }

    /// Submit the reviewed preview once, then wait for the network to settle its original hash.
    pub(super) fn submit_reviewed(&mut self) {
        let (Some(selected), Some(handle), Some(preview)) = (
            &self.selected,
            self.runtime.as_ref().map(|r| r.handle().clone()),
            self.preview.clone(),
        ) else {
            return;
        };
        let network = selected.network.clone();
        let peer = self.peer;
        let hash = preview.hash().to_owned();
        self.submission = Some(Submission {
            hash: hash.clone(),
            outcome: None,
        });
        self.spawn(move || Message::Submitted {
            result: handle
                .block_on(network.submit(peer, &preview))
                .map_err(|error| error.to_string()),
            hash,
        });
    }

    pub(super) fn contracts(&mut self, ui: &mut Ui) {
        let palette = widgets::palette(ui);
        widgets::page_title(
            ui,
            "Contracts",
            "Deploy Kotodama sources (.ko), IVM bytecode (.to) or Musubi packages. Compilation, fee review and recovery use the same service as Kagami.",
        );
        let hovering = ui.ctx().input(|input| !input.raw.hovered_files.is_empty());
        widgets::accent_card(ui, hovering.then_some(palette.sakura), |ui| {
            ui.horizontal(|ui| {
                widgets::icon_badge(ui, icon::DROP, palette.sakura, palette.sakura_soft);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Deploy a smart contract")
                            .size(16.0)
                            .strong()
                            .color(palette.text),
                    );
                    widgets::hint(
                        ui,
                        "Drop a .ko, .to or Musubi.toml anywhere on this window, or type its path.",
                    );
                });
            });
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.contract_path)
                    .hint_text("Contract path")
                    .desired_width(f32::INFINITY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.contract_alias)
                    .hint_text("Alias (optional; generated in the selected dataspace)")
                    .desired_width(f32::INFINITY),
            );
            egui::CollapsingHeader::new("Package options").show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.package)
                        .hint_text("Package selector (optional)"),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.contract)
                        .hint_text("Contract target (optional)"),
                );
                ui.checkbox(&mut self.locked, "Require unchanged dependency lock");
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.auto_approve,
                    "Approve automatically when fees are at most",
                );
                ui.add_enabled(
                    self.auto_approve,
                    egui::TextEdit::singleline(&mut self.auto_approve_cap).desired_width(64.0),
                );
                ui.label(RichText::new("in one fee asset").color(palette.muted));
            });
            if ui
                .add_enabled(
                    !self.busy && self.workspace.is_some() && !self.contract_path.trim().is_empty(),
                    widgets::primary_button(palette, "Prepare deployment"),
                )
                .clicked()
            {
                self.deploy(false);
            }
            widgets::hint(
                ui,
                if self.auto_approve {
                    "Deployments within the cap dispatch without the review, and Activity records each approval. Larger or mixed-asset fees still ask first."
                } else {
                    "Mochi shows the exact deployment and quoted fees before anything is dispatched."
                },
            );
        });
        if let Some(receipt) = &self.receipt {
            ui.add_space(12.0);
            widgets::accent_card(ui, Some(palette.matcha), |ui| {
                ui.horizontal(|ui| {
                    widgets::icon_badge(ui, icon::OK, palette.matcha, palette.matcha_soft);
                    ui.label(
                        RichText::new("Verified deployment receipt")
                            .size(16.0)
                            .strong()
                            .color(palette.text),
                    );
                    if ui.button("Copy receipt").clicked() {
                        widgets::copy_to_clipboard(ui.ctx(), receipt, "receipt");
                    }
                });
                // The summary says what applied and what to do next; the JSON stays one click away.
                let (summary, json) = receipt.find("\n{").map_or((receipt.as_str(), ""), |at| {
                    (&receipt[..at], &receipt[at + 1..])
                });
                ui.add(egui::Label::new(RichText::new(summary).monospace()).wrap());
                if !json.is_empty() {
                    egui::CollapsingHeader::new("Receipt JSON")
                        .id_salt("mochi-receipt-json")
                        .show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(json).monospace()).wrap());
                        });
                }
            });
        }
        ui.add_space(12.0);
        self.contract_calls(ui);
        ui.add_space(12.0);
        widgets::card(ui, |ui| {
            widgets::card_title(ui, icon::REFRESH, "Resume a deployment");
            widgets::hint(
                ui,
                "Deploying the same input again recovers its journal on its own. Resume a saved journal when the input changed or is gone.",
            );
            if self.selected.is_some() {
                self.saved_deployment_picker(ui);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.journal_path)
                    .hint_text("Journal path")
                    .desired_width(f32::INFINITY),
            );
            if ui
                .add_enabled(
                    !self.busy && self.workspace.is_some() && !self.journal_path.trim().is_empty(),
                    egui::Button::new("Review and resume"),
                )
                .clicked()
            {
                self.deploy(true);
            }
        });
    }

    /// Call or view a deployed contract through the same service as `kagami contract`.
    fn contract_calls(&mut self, ui: &mut Ui) {
        let palette = widgets::palette(ui);
        let ready = self.ready() && !self.busy;
        let (mut view, mut call) = (false, false);
        widgets::card(ui, |ui| {
            widgets::card_title(ui, icon::CONTRACTS, "Call or view a contract");
            widgets::hint(
                ui,
                "Views read state for free. Calls change state and pay at most the fee cap; their journal resumes an uncertain outcome.",
            );
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.call.alias)
                    .hint_text("Contract alias (filled in after a deployment)")
                    .desired_width(f32::INFINITY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.call.entrypoint)
                    .hint_text("Entrypoint, e.g. hajimari")
                    .desired_width(f32::INFINITY),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.call.arguments)
                    .code_editor()
                    .desired_rows(3)
                    .hint_text("Named arguments as JSON, e.g. {\"start\": \"1\"}")
                    .desired_width(f32::INFINITY),
            );
            let filled =
                !self.call.alias.trim().is_empty() && !self.call.entrypoint.trim().is_empty();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        ready && filled,
                        egui::Button::new(format!("{}  View", icon::OK)),
                    )
                    .on_hover_text("Read-only; nothing is signed")
                    .clicked()
                {
                    view = true;
                }
                if ui
                    .add_enabled(ready && filled, widgets::primary_button(palette, "Call"))
                    .on_hover_text("Signs and submits one bounded call")
                    .clicked()
                {
                    call = true;
                }
                ui.label(RichText::new("fee cap").color(palette.muted));
                ui.add(egui::TextEdit::singleline(&mut self.call.max_fee).desired_width(72.0));
            });
            if !self.ready() {
                widgets::hint(ui, "Start the environment to call its contracts.");
            }
            if let Some(output) = &self.call.output {
                let (accent, text) = match output {
                    Ok(text) => (palette.matcha, text),
                    Err(text) => (palette.aka, text),
                };
                ui.add_space(6.0);
                widgets::accent_card(ui, Some(accent), |ui| {
                    ui.add(egui::Label::new(RichText::new(text).monospace()).wrap());
                });
            }
        });
        if view {
            self.run_contract(false);
        } else if call {
            self.run_contract(true);
        }
    }

    /// Run the call card's View, or with `mutate` its Call, in the background.
    pub(super) fn run_contract(&mut self, mutate: bool) {
        let (Some(workspace), Some(selected)) = (self.workspace.clone(), &self.selected) else {
            return;
        };
        let network = selected.network.clone();
        let form = self.call.clone();
        self.call.output = None;
        self.spawn(move || {
            let result = if mutate {
                workspace
                    .call_contract(
                        &network,
                        &form.alias,
                        &form.entrypoint,
                        &form.arguments,
                        &form.max_fee,
                    )
                    .map(|report| {
                        let mut lines = vec![format!(
                            "{} `{}` {} on {}",
                            form.alias.trim(),
                            form.entrypoint.trim(),
                            report.status,
                            report.context
                        )];
                        if let Some(receipt) = &report.receipt {
                            lines.push(format!("call hash: {}", receipt.call.hash));
                        }
                        lines.push(format!("journal: {}", report.journal.display()));
                        lines.join("\n")
                    })
            } else {
                workspace
                    .view_contract(&network, &form.alias, &form.entrypoint, &form.arguments)
                    .and_then(|view| Ok(norito::json::to_string_pretty(&view.result)?))
            };
            Message::ContractResult {
                mutated: mutate,
                result: result.map_err(|error| error.to_string()),
            }
        });
    }

    /// Offer the selected network's resumable journals; choosing one fills the journal path.
    fn saved_deployment_picker(&mut self, ui: &mut Ui) {
        if self.saved_deployments.is_none() && !self.busy {
            self.load_saved_deployments();
        }
        let palette = widgets::palette(ui);
        let mut chosen = None;
        let mut refresh = false;
        ui.horizontal(|ui| {
            match &self.saved_deployments {
                None => {
                    ui.spinner();
                    ui.label(RichText::new("Looking for saved deployments…").color(palette.muted));
                }
                Some(Err(error)) => {
                    ui.label(
                        RichText::new(format!("Saved deployments are unavailable: {error}"))
                            .color(palette.aka),
                    );
                }
                Some(Ok(saved)) if saved.is_empty() => {
                    ui.label(
                        RichText::new("No saved deployments in this environment yet.")
                            .color(palette.muted),
                    );
                }
                Some(Ok(saved)) => {
                    let current = saved.iter().find(|deployment| {
                        deployment.journal.as_os_str() == self.journal_path.trim()
                    });
                    egui::ComboBox::from_id_salt("mochi-saved-deployments")
                        .selected_text(
                            current.map_or_else(
                                || "Choose a saved deployment".to_owned(),
                                saved_label,
                            ),
                        )
                        .show_ui(ui, |ui| {
                            for deployment in saved {
                                let selected = current == Some(deployment);
                                if ui
                                    .selectable_label(selected, saved_label(deployment))
                                    .on_hover_text(deployment.journal.display().to_string())
                                    .clicked()
                                {
                                    chosen = Some(deployment.journal.display().to_string());
                                }
                            }
                        });
                }
            }
            if ui
                .add_enabled(
                    !self.busy && self.saved_deployments.is_some(),
                    egui::Button::new(icon::REFRESH),
                )
                .on_hover_text("Look for saved deployments again")
                .clicked()
            {
                refresh = true;
            }
        });
        if let Some(journal) = chosen {
            self.journal_path = journal;
        }
        if refresh {
            self.saved_deployments = None;
        }
    }

    /// Read the selected network's resumable journals in the background; nothing is signed.
    fn load_saved_deployments(&mut self) {
        let (Some(workspace), Some(selected)) = (self.workspace.clone(), &self.selected) else {
            return;
        };
        let network = selected.network.clone();
        self.spawn_task(move || {
            Message::SavedDeployments(
                workspace
                    .saved_deployments(&network)
                    .map_err(|error| error.to_string()),
            )
        });
    }

    pub(super) fn packages(&mut self, ui: &mut Ui) {
        widgets::page_title(
            ui,
            "Packages",
            "Publish Musubi packages through the generated localnet's registry, shared with Kagami.",
        );
        if let Some(action) = self
            .publication
            .show(ui, !self.busy && self.workspace.is_some())
        {
            self.publish_package(action);
        }
        if let Some(output) = &self.publication_output {
            ui.add_space(12.0);
            output.show(ui);
        }
    }
}

impl PublicationForm {
    pub(super) fn show(&mut self, ui: &mut Ui, available: bool) -> Option<PublicationAction> {
        let palette = widgets::palette(ui);
        let mut action = None;
        widgets::card(ui, |ui| {
            widgets::card_title(ui, icon::PACKAGES, "Publish a package");
            widgets::hint(
                ui,
                "Publish to the selected generated localnet. Publish starts a default localnet when none is selected.",
            );
            widgets::hint(
                ui,
                "Publication can register the package namespace and pay transaction fees.",
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.manifest)
                    .hint_text("Manifest or workspace path")
                    .desired_width(f32::INFINITY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.package)
                    .hint_text("Package selector (optional)")
                    .desired_width(f32::INFINITY),
            );
            ui.checkbox(&mut self.detach, "Return after durable seed staging");
            if ui
                .add_enabled(
                    available,
                    widgets::primary_button(palette, "Publish package"),
                )
                .clicked()
            {
                action = Some(PublicationAction::Begin);
            }
        });
        ui.add_space(12.0);
        widgets::card(ui, |ui| {
            widgets::card_title(ui, icon::REFRESH, "Resume or recover");
            widgets::hint(
                ui,
                "Use the original operation ID from publication output. Resume uses its retained package; Recover package files requires the original workspace.",
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.operation_id)
                    .hint_text("Publication operation ID")
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal_wrapped(|ui| {
                let enabled = available && !self.operation_id.trim().is_empty();
                if ui
                    .add_enabled(enabled, egui::Button::new("Resume publication"))
                    .clicked()
                {
                    action = Some(PublicationAction::Resume);
                }
                if ui
                    .add_enabled(enabled, egui::Button::new("Recover package files"))
                    .clicked()
                {
                    action = Some(PublicationAction::Recover);
                }
            });
        });
        action
    }
}

impl PublicationOutput {
    pub(super) fn show(&self, ui: &mut Ui) {
        let palette = widgets::palette(ui);
        let (fg, bg) = if self.exit_code == 0 {
            (palette.matcha, palette.matcha_soft)
        } else {
            (palette.aka, palette.aka_soft)
        };
        widgets::accent_card(ui, Some(fg), |ui| {
            ui.horizontal(|ui| {
                widgets::icon_badge(
                    ui,
                    if self.exit_code == 0 {
                        icon::OK
                    } else {
                        icon::WARNING
                    },
                    fg,
                    bg,
                );
                ui.label(
                    RichText::new(format!(
                        "Publication output · exit status {}",
                        self.exit_code
                    ))
                    .strong()
                    .color(palette.text),
                );
            });
            if !self.stdout.is_empty() {
                ui.label(widgets::eyebrow(palette, "Output"));
                if ui.button("Copy publication output").clicked() {
                    widgets::copy_to_clipboard(ui.ctx(), &self.stdout, "publication output");
                }
                ui.add(egui::Label::new(egui::RichText::new(&self.stdout).monospace()).wrap());
            }
            if !self.stderr.is_empty() {
                ui.label(widgets::eyebrow(palette, "Diagnostics"));
                if ui.button("Copy publication diagnostics").clicked() {
                    widgets::copy_to_clipboard(ui.ctx(), &self.stderr, "publication diagnostics");
                }
                ui.add(egui::Label::new(egui::RichText::new(&self.stderr).monospace()).wrap());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_deployments_are_named_by_alias_and_outcome() {
        let mut deployment = mochi_core::developer::SavedDeployment {
            alias: "Counter::universal".parse().unwrap(),
            journal: std::path::PathBuf::from("journal"),
            has_receipt: true,
        };
        assert_eq!(saved_label(&deployment), "Counter::universal · applied");
        deployment.has_receipt = false;
        assert_eq!(
            saved_label(&deployment),
            "Counter::universal · not finished"
        );
    }

    #[test]
    fn hook_argument_templates_name_every_parameter_with_a_typed_placeholder() {
        assert_eq!(argument_template(std::iter::empty()), "{}");
        assert_eq!(
            argument_template(
                [("start", "int"), ("open", "bool"), ("owner", "AccountId")].into_iter()
            ),
            r#"{"start": "0", "open": false, "owner": ""}"#
        );
        assert_eq!(
            argument_template(
                [
                    ("rate", "decimal"),
                    ("amount", "quantity"),
                    ("space", "DataSpaceId"),
                    ("extra", "Json"),
                ]
                .into_iter()
            ),
            r#"{"rate": "0", "amount": "0", "space": 0, "extra": null}"#
        );
        let template = argument_template([("start", "int")].into_iter());
        assert!(
            is_json_object(&template),
            "templates are valid JSON objects: {template}"
        );
    }

    fn is_json_object(json: &str) -> bool {
        norito::json::from_str::<norito::json::Value>(json)
            .is_ok_and(|value| value.as_object().is_some())
    }

    #[test]
    fn dropped_contract_inputs_are_recognized_by_name_and_extension() {
        let temporary = tempfile::tempdir().unwrap();
        assert!(is_contract_input(temporary.path()));
        for accepted in ["coffee.ko", "coffee.to", "Musubi.toml"] {
            assert!(
                is_contract_input(&temporary.path().join(accepted)),
                "{accepted}"
            );
        }
        for refused in ["notes.txt", "Cargo.toml", "coffee.ko.bak", "ko"] {
            assert!(
                !is_contract_input(&temporary.path().join(refused)),
                "{refused}"
            );
        }
    }

    #[test]
    fn composer_appends_exact_drafts_and_reports_invalid_input() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        desktop.action = 3;
        desktop.asset = "not an account".into();
        desktop.add_instruction();
        assert!(desktop.error.as_deref().unwrap().contains("not an account"));
        assert_eq!(desktop.drafts, "[]");
        desktop.error = None;
        desktop.action = 3;
        desktop.asset = iroha_test_samples::BOB_ID.to_string();
        desktop.add_instruction();
        assert!(desktop.error.is_none());
        assert_eq!(drafts_from_json_str(&desktop.drafts).unwrap().len(), 1);
        assert!(desktop.preview.is_none());
    }

    /// The localnet fee asset's canonical definition.
    const XOR: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
    /// The validators' wording when a non-owner mints XOR.
    const NOT_OWNER: &str = "Validation failed: Operation is not permitted: Can't mint an asset without owning its definition or an exact mint permission";

    /// A mint of 100 XOR to Alice, signed offline by a development authority.
    fn offline_mint_preview() -> TransactionPreview {
        let signer = &mochi_core::development_signing_authorities()[0];
        let draft = InstructionDraft::mint_from_input(
            &format!("{XOR}#{}", *iroha_test_samples::ALICE_ID),
            "100",
        )
        .unwrap();
        mochi_core::compose_preview_with_options(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                .parse()
                .unwrap(),
            &[draft],
            signer,
            &mochi_core::TransactionComposeOptions::default(),
        )
        .unwrap()
    }

    /// Genesis owns XOR, as on a generated localnet.
    fn xor_ledger() -> LedgerSnapshot {
        let page = |kind, entries| mochi_core::StatePage {
            kind,
            entries,
            remaining: 0,
            cursor: None,
        };
        let xor = mochi_core::StateEntry {
            title: XOR.into(),
            subtitle: String::new(),
            detail: String::new(),
            raw: String::new(),
            domain: None,
            domain_lower: None,
            owner: Some("genesis".into()),
            owner_lower: Some("genesis".into()),
            asset_definition: Some(XOR.into()),
            asset_definition_lower: Some(XOR.to_lowercase()),
            json: Some(r#"{"name": "XOR", "mintable": "Infinitely"}"#.into()),
            norito_bytes: None,
            search_blob: String::new(),
        };
        LedgerSnapshot::new(
            page(mochi_core::StateQueryKind::Accounts, Vec::new()),
            page(mochi_core::StateQueryKind::Assets, Vec::new()),
            &page(mochi_core::StateQueryKind::AssetDefinitions, vec![xor]),
        )
    }

    fn review_frame(desktop: &mut Desktop, ledger: &LedgerSnapshot) -> egui::FullOutput {
        let context = egui::Context::default();
        super::super::tests::run_headless_frame(&context, egui::RawInput::default(), |context| {
            egui::CentralPanel::default()
                .show(context, |ui| desktop.transaction_preview(ui, Some(ledger)));
        })
    }

    #[test]
    fn review_cards_name_tokens_and_show_exact_identifiers_until_submitted() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        let preview = offline_mint_preview();
        let exact = preview.summaries()[0].clone();
        assert!(exact.starts_with(&format!("Mint 100 to {XOR}#")), "{exact}");
        desktop.preview = Some(preview);
        let ledger = xor_ledger();
        let output = review_frame(&mut desktop, &ledger);
        let alice = widgets::short_middle(&iroha_test_samples::ALICE_ID.to_string(), 6);
        for text in [
            "signed · not submitted".to_owned(),
            format!("Mint 100 XOR to {alice}"),
            exact,
            "Submit reviewed transaction".to_owned(),
            "Discard".to_owned(),
        ] {
            assert!(
                super::super::tests::text_position(&output, &text).is_some(),
                "{text}"
            );
        }
        assert!(
            !desktop.reveal_review,
            "rendering consumes the scroll request"
        );
    }

    #[test]
    fn rejected_mints_show_the_reason_and_offer_a_token_of_your_own() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        desktop.view = View::Composer;
        let preview = offline_mint_preview();
        desktop.submission = Some(Submission {
            hash: preview.hash().into(),
            outcome: Some(Ok(SubmissionOutcome::Rejected {
                reason: NOT_OWNER.into(),
            })),
        });
        desktop.preview = Some(preview);
        let ledger = xor_ledger();
        let output = review_frame(&mut desktop, &ledger);
        let create = format!("{}  Create your own token", icon::COINS);
        for text in [
            "rejected",
            NOT_OWNER,
            "Edit and review again",
            create.as_str(),
        ] {
            assert!(
                super::super::tests::text_position(&output, text).is_some(),
                "{text}"
            );
        }
        assert!(
            super::super::tests::text_position(&output, "Submit reviewed transaction").is_none(),
            "a settled transaction is never submitted again"
        );
        let context = egui::Context::default();
        super::super::tests::click_label(&context, &create, |ui| {
            desktop.transaction_preview(ui, Some(&ledger));
        });
        assert!(desktop.preview.is_none() && desktop.submission.is_none());
        assert_eq!(desktop.quick.kind, QuickKind::NewToken);
        assert_eq!(desktop.view, View::Accounts);
    }

    #[test]
    fn settled_and_unconfirmed_outcomes_read_plainly() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        let preview = offline_mint_preview();
        let hash = preview.hash().to_owned();
        desktop.preview = Some(preview);
        let ledger = xor_ledger();
        for (outcome, expected) in [
            (None, vec!["submitting…"]),
            (
                Some(Ok(SubmissionOutcome::Applied { block_height: 7 })),
                vec!["applied · block 7", "Done"],
            ),
            (
                Some(Ok(SubmissionOutcome::Expired)),
                vec!["expired", "Edit and review again"],
            ),
            (
                Some(Err("Torii did not answer".to_owned())),
                vec!["outcome unknown", "Torii did not answer", "Discard"],
            ),
        ] {
            desktop.submission = Some(Submission {
                hash: hash.clone(),
                outcome,
            });
            let output = review_frame(&mut desktop, &ledger);
            for text in expected {
                assert!(
                    super::super::tests::text_position(&output, text).is_some(),
                    "{text}"
                );
            }
        }
    }

    #[test]
    fn fee_and_lease_lines_name_their_asset() {
        let ledger = xor_ledger();
        let fee = mochi_core::FeeLimit {
            label: "Network fee",
            max_amount: "0.0085".into(),
            asset_definition: XOR.into(),
        };
        assert_eq!(
            fee_line(&fee, Some(&ledger)),
            "Network fee · up to 0.0085 XOR"
        );
        assert_eq!(fee_line(&fee, None), "Network fee · up to 0.0085 6TEA…K6tw");
        let lease = DomainLease {
            domain: "coffee.universal".into(),
            years: 1,
            amount: Some("0.5".into()),
            asset_definition: XOR.into(),
        };
        assert_eq!(
            lease_line(&lease, Some(&ledger)),
            "Lease domain coffee.universal for 1 year · 0.5 XOR"
        );
        assert_eq!(
            lease_line(
                &DomainLease {
                    amount: None,
                    ..lease
                },
                Some(&ledger)
            ),
            "Restore domain coffee.universal · no charge"
        );
    }

    #[test]
    fn preview_card_is_hidden_without_a_reviewed_transaction() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        let context = egui::Context::default();
        let output = super::super::tests::run_headless_frame(
            &context,
            egui::RawInput::default(),
            |context| {
                egui::CentralPanel::default()
                    .show(context, |ui| desktop.transaction_preview(ui, None));
            },
        );
        assert!(super::super::tests::text_position(&output, "Review transaction").is_none());
        assert!(!desktop.busy);
    }
}
