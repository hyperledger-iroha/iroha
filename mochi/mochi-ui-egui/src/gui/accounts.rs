//! Accounts: balances for every registered account and one-click ledger quick actions.
//!
//! Quick actions only build drafts. Each still becomes an exact signed preview that the
//! developer submits explicitly, exactly like the transaction composer. Only a definition's
//! owner can mint it, so Mint offers the developer's own tokens first and says before review
//! when another owner's token, such as the XOR fee asset, would be rejected. New token creates
//! a token the developer owns in one reviewed transaction.

use super::{
    Desktop,
    widgets::{self, icon},
};
use eframe::egui::{self, RichText, Ui};
use iroha_data_model::{account::AccountId, asset::id::AssetDefinitionId};
use mochi_core::{
    ComposeError, GeneratedAccount, InstructionDraft, StateEntry, StatePage, TokenDraft,
    asset_literal, token_domain_label,
};
use std::collections::BTreeMap;

/// What the Accounts view knows about one asset definition; presentation only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DefinitionInfo {
    /// Display name when the record carries a non-blank one.
    name: Option<String>,
    /// Owning account literal.
    owner: Option<String>,
    /// Mintability as the record names it, such as `Infinitely` or `Not`.
    mintable: Option<String>,
}

/// The display fields an asset definition record carries.
fn definition_info(entry: &StateEntry) -> DefinitionInfo {
    let value = entry
        .json
        .as_deref()
        .and_then(|json| norito::json::from_str::<norito::json::Value>(json).ok());
    let text = |field: &str| {
        let text = value.as_ref()?.get(field)?.as_str()?.trim();
        (!text.is_empty()).then(|| text.to_owned())
    };
    DefinitionInfo {
        name: text("name").map(|name| name.chars().take(32).collect()),
        owner: entry.owner.clone().filter(|owner| !owner.is_empty()),
        mintable: text("mintable"),
    }
}

/// Accounts and holdings observed from one validator of the selected generation.
#[derive(Clone, Debug)]
pub(crate) struct LedgerSnapshot {
    pub(crate) accounts: StatePage,
    pub(crate) holdings: StatePage,
    /// Asset definitions in query order, by literal; presentation only.
    definitions: Vec<(String, DefinitionInfo)>,
}

/// One balance of one account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Holding<'a> {
    /// Exact asset literal, including any dataspace scope.
    pub(crate) asset: &'a str,
    pub(crate) definition: &'a str,
    pub(crate) value: &'a str,
}

fn holding(entry: &StateEntry) -> Option<Holding<'_>> {
    Some(Holding {
        asset: &entry.title,
        definition: entry.asset_definition.as_deref()?,
        value: entry.detail.strip_prefix("Value ").unwrap_or(&entry.detail),
    })
}

/// Whether an account may mint a definition, judged from the definition record alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MintRight {
    /// The account owns the definition and may mint any amount to any account.
    Owner,
    /// Another account owns it: minting needs a permission that owner granted.
    NotOwner,
    /// The definition can no longer be minted.
    Unmintable,
    /// The definition is not observed; the network decides.
    Unknown,
}

impl LedgerSnapshot {
    pub(crate) fn new(accounts: StatePage, holdings: StatePage, definitions: &StatePage) -> Self {
        let definitions = definitions
            .entries
            .iter()
            .map(|entry| (entry.title.clone(), definition_info(entry)))
            .collect();
        Self {
            accounts,
            holdings,
            definitions,
        }
    }

    fn info(&self, definition: &str) -> Option<&DefinitionInfo> {
        self.definitions
            .iter()
            .find(|(literal, _)| literal == definition)
            .map(|(_, info)| info)
    }

    /// Display label for an asset definition: its name, else a shortened literal.
    pub(crate) fn asset_label(&self, definition: &str) -> String {
        self.info(definition)
            .and_then(|info| info.name.clone())
            .unwrap_or_else(|| widgets::short_middle(definition, 4))
    }

    /// The account that owns `definition`, when observed.
    pub(crate) fn owner_of(&self, definition: &str) -> Option<&str> {
        self.info(definition)?.owner.as_deref()
    }

    /// Whether `account` may mint `definition`.
    pub(crate) fn mint_right(&self, definition: &str, account: &str) -> MintRight {
        let Some(info) = self.info(definition) else {
            return MintRight::Unknown;
        };
        if info.mintable.as_deref() == Some("Not") {
            return MintRight::Unmintable;
        }
        match info.owner.as_deref() {
            Some(owner) if owner == account => MintRight::Owner,
            Some(_) => MintRight::NotOwner,
            None => MintRight::Unknown,
        }
    }

    /// Definition literals in query order, the ones `account` owns first.
    pub(crate) fn definitions_for(&self, account: &str) -> Vec<&str> {
        let (mut owned, others): (Vec<_>, Vec<_>) = self
            .definitions
            .iter()
            .partition(|(_, info)| info.owner.as_deref() == Some(account));
        owned.extend(others);
        owned
            .into_iter()
            .map(|(literal, _)| literal.as_str())
            .collect()
    }

    /// Balances owned by `account`, in query order.
    pub(crate) fn holdings_of(&self, account: &str) -> Vec<Holding<'_>> {
        self.holdings
            .entries
            .iter()
            .filter(|entry| entry.owner.as_deref() == Some(account))
            .filter_map(holding)
            .collect()
    }

    /// Account literals in query order.
    pub(crate) fn account_ids(&self) -> impl Iterator<Item = &str> {
        self.accounts
            .entries
            .iter()
            .map(|entry| entry.title.as_str())
    }
}

/// How the Accounts view names an account: `you` for the developer, else a shortened literal.
pub(crate) fn account_label(account: &str, developer: &str) -> String {
    if account == developer {
        "you".to_owned()
    } else {
        widgets::short_middle(account, 6)
    }
}

/// One plain-language line per draft, naming tokens and accounts like the Accounts view.
///
/// A token registered earlier in the same drafts is named before the ledger has observed it.
pub(crate) fn describe_drafts(
    drafts: &[InstructionDraft],
    ledger: Option<&LedgerSnapshot>,
    developer: &str,
) -> Vec<String> {
    let created = drafts
        .iter()
        .filter_map(|draft| match draft {
            InstructionDraft::RegisterAssetDefinition {
                definition, name, ..
            } => Some((definition.to_string(), name.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let token = |definition: &AssetDefinitionId| {
        let literal = definition.to_string();
        created.get(&literal).cloned().unwrap_or_else(|| {
            ledger.map_or_else(
                || widgets::short_middle(&literal, 4),
                |ledger| ledger.asset_label(&literal),
            )
        })
    };
    let who = |account: &AccountId| account_label(&account.to_string(), developer);
    drafts
        .iter()
        .map(|draft| match draft {
            InstructionDraft::MintAsset { asset, quantity } => format!(
                "Mint {quantity} {} to {}",
                token(asset.definition()),
                who(asset.account())
            ),
            InstructionDraft::BurnAsset { asset, quantity } => format!(
                "Burn {quantity} {} from {}",
                token(asset.definition()),
                who(asset.account())
            ),
            InstructionDraft::TransferAsset {
                asset,
                quantity,
                destination,
            } => format!(
                "Send {quantity} {} from {} to {}",
                token(asset.definition()),
                who(asset.account()),
                who(destination)
            ),
            InstructionDraft::RegisterAccount { account } => {
                format!("Register account {}", who(account))
            }
            InstructionDraft::RegisterAssetDefinition {
                name,
                domain: Some(domain),
                ..
            } => format!("Create token {name} in {domain}"),
            other => other.summary(),
        })
        .collect()
}

/// Plain words for a composer error in a quick-action form.
fn explain(error: &ComposeError) -> String {
    match error {
        ComposeError::InvalidQuantity { .. } => {
            "Amounts are plain decimals such as 100 or 2.5.".into()
        }
        ComposeError::InvalidAccountId { .. } => {
            "That is not an account ID on this network.".into()
        }
        ComposeError::InvalidAssetId { .. } | ComposeError::InvalidAssetDefinitionId { .. } => {
            "Pick a token from the list.".into()
        }
        ComposeError::InvalidAssetDefinitionName { .. } => {
            "Token names need a letter or digit and no #, @ or control characters.".into()
        }
        ComposeError::InvalidDomainId { .. } => {
            "Domain labels use lowercase letters, digits and dashes.".into()
        }
        other => other.to_string(),
    }
}

/// The ledger change a quick action prepares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum QuickKind {
    #[default]
    Mint,
    Transfer,
    Burn,
    NewToken,
    NewAccount,
}

/// The token New token creates.
#[derive(Clone, Debug)]
pub(crate) struct TokenForm {
    pub(crate) name: String,
    /// Domain label under `.universal`; it follows the name until edited.
    pub(crate) label: String,
    pub(crate) label_edited: bool,
    pub(crate) supply: String,
}

impl Default for TokenForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            label: String::new(),
            label_edited: false,
            supply: "1000".into(),
        }
    }
}

/// Form state for the quick actions; it never submits anything by itself.
#[derive(Clone, Debug)]
pub(crate) struct QuickActions {
    pub(crate) kind: QuickKind,
    /// Exact developer holding to send or burn, or to fund a new account from.
    pub(crate) asset: String,
    /// Asset definition to mint.
    pub(crate) definition: String,
    /// Recipient of a transfer or mint; an empty mint recipient is the developer.
    pub(crate) recipient: String,
    pub(crate) quantity: String,
    /// Optional funding for a new account, taken from `asset`.
    pub(crate) fund: String,
    pub(crate) generated: Option<GeneratedAccount>,
    pub(crate) reveal_key: bool,
    pub(crate) token: TokenForm,
}

impl Default for QuickActions {
    fn default() -> Self {
        Self {
            kind: QuickKind::Mint,
            asset: String::new(),
            definition: String::new(),
            recipient: String::new(),
            quantity: "100".into(),
            fund: "100".into(),
            generated: None,
            reveal_key: false,
            token: TokenForm::default(),
        }
    }
}

/// Width reserved beside a text field for its pick-from-list menu button.
const MENU_BUTTON_ROOM: f32 = 56.0;

impl QuickActions {
    /// Exact drafts for the selected action, signed by `developer`. Parsing belongs to the
    /// canonical composer; errors are worded for the form.
    pub(crate) fn drafts(
        &self,
        discriminant: u16,
        developer: &str,
    ) -> Result<Vec<InstructionDraft>, String> {
        if let Some(missing) = self.missing() {
            return Err(missing.into());
        }
        let asset = self.asset.trim();
        let recipient = self.recipient.trim();
        let quantity = self.quantity.trim();
        let draft = |result: Result<InstructionDraft, ComposeError>| {
            result.map_err(|error| explain(&error))
        };
        match self.kind {
            QuickKind::Mint => {
                let holder = if recipient.is_empty() {
                    developer
                } else {
                    recipient
                };
                let target = asset_literal(self.definition.trim(), holder, discriminant)
                    .map_err(|error| explain(&error))?;
                Ok(vec![draft(InstructionDraft::mint_from_input(
                    &target, quantity,
                ))?])
            }
            QuickKind::Burn => Ok(vec![draft(InstructionDraft::burn_from_input(
                asset, quantity,
            ))?]),
            QuickKind::Transfer => Ok(vec![draft(InstructionDraft::transfer_from_input(
                asset, quantity, recipient,
            ))?]),
            QuickKind::NewToken => self
                .token_draft(developer)
                .map(|token| token.drafts)
                .map_err(|error| explain(&error)),
            QuickKind::NewAccount => {
                let generated = self
                    .generated
                    .as_ref()
                    .ok_or("Generate a key pair for the new account first.")?;
                let mut drafts = vec![generated.register_draft()];
                let fund = self.fund.trim();
                if !asset.is_empty() && !fund.is_empty() {
                    drafts.push(draft(InstructionDraft::transfer_from_input(
                        asset,
                        fund,
                        &generated.account_literal(discriminant),
                    ))?);
                }
                Ok(drafts)
            }
        }
    }

    /// The token New token would create for `developer`.
    pub(crate) fn token_draft(&self, developer: &str) -> Result<TokenDraft, ComposeError> {
        let creator = AccountId::parse_encoded(developer).map_err(|error| {
            ComposeError::InvalidAccountId {
                account: developer.to_owned(),
                reason: error.to_string(),
            }
        })?;
        TokenDraft::new(
            &self.token.name,
            &self.token.label,
            &self.token.supply,
            &creator,
        )
    }

    /// The first empty field the selected action needs, as a prompt.
    fn missing(&self) -> Option<&'static str> {
        let empty = |text: &str| text.trim().is_empty();
        match self.kind {
            QuickKind::Mint if empty(&self.definition) => Some("Pick a token to mint."),
            QuickKind::Transfer | QuickKind::Burn if empty(&self.asset) => {
                Some("Pick one of your balances.")
            }
            QuickKind::Transfer if empty(&self.recipient) => Some("Choose who receives it."),
            QuickKind::Mint | QuickKind::Transfer | QuickKind::Burn if empty(&self.quantity) => {
                Some("Enter an amount.")
            }
            QuickKind::NewToken if empty(&self.token.name) => Some("Name your token."),
            QuickKind::NewToken if empty(&self.token.label) => Some("Choose a domain label."),
            QuickKind::NewToken if empty(&self.token.supply) => Some("Enter the first supply."),
            QuickKind::NewAccount if self.generated.is_none() => {
                Some("Generate a key pair for the new account first.")
            }
            _ => None,
        }
    }

    /// Why the selected action cannot be reviewed yet; `None` when it can.
    pub(crate) fn problem(
        &self,
        discriminant: u16,
        developer: &str,
        ledger: Option<&LedgerSnapshot>,
    ) -> Option<String> {
        if let Err(problem) = self.drafts(discriminant, developer) {
            return Some(problem);
        }
        if self.kind == QuickKind::NewToken
            && let (Some(ledger), Ok(token)) = (ledger, self.token_draft(developer))
            && ledger.info(&token.definition.to_string()).is_some()
        {
            return Some(format!(
                "{} already exists in {}: mint more from Mint.",
                self.token.name.trim(),
                token.domain
            ));
        }
        None
    }

    /// Follow the token name with its domain label until the label is edited by hand.
    fn name_changed(&mut self) {
        if !self.token.label_edited {
            self.token.label = token_domain_label(&self.token.name);
        }
    }

    /// Fill empty pickers from the ledger: the developer's first balance and first own token.
    fn seed(&mut self, ledger: Option<&LedgerSnapshot>, developer: &str) {
        let Some(ledger) = ledger else {
            return;
        };
        if self.asset.is_empty()
            && let Some(first) = ledger.holdings_of(developer).first()
        {
            self.asset = first.asset.to_owned();
        }
        if self.definition.is_empty()
            && let Some(first) = ledger.definitions_for(developer).first()
        {
            self.definition = (*first).to_owned();
        }
    }

    /// Switch to sending the developer's balance of `definition`, keeping recipient and amount.
    pub(crate) fn send_instead(
        &mut self,
        ledger: &LedgerSnapshot,
        developer: &str,
        definition: &str,
    ) {
        if let Some(holding) = ledger
            .holdings_of(developer)
            .into_iter()
            .find(|holding| holding.definition == definition)
        {
            self.asset = holding.asset.to_owned();
        }
        self.kind = QuickKind::Transfer;
    }
}

/// What the mint form tells the developer about the selected token.
enum MintGuidance {
    None,
    Owner(String),
    NotOwner {
        token: String,
        owner: String,
        held: bool,
    },
    Unmintable(String),
}

impl Desktop {
    /// Whether the developer owns a token it can still mint, as last observed.
    pub(super) fn owns_mintable_token(&self) -> bool {
        let (Some(selected), Some(ledger)) = (&self.selected, &self.ledger) else {
            return false;
        };
        let developer = &selected.network.prepared().context.account_id;
        ledger
            .definitions_for(developer)
            .iter()
            .any(|definition| ledger.mint_right(definition, developer) == MintRight::Owner)
    }

    pub(super) fn accounts(&mut self, ui: &mut Ui) {
        let _address_profile = self.selected.as_ref().map(|selected| {
            iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                selected.network.address_discriminant(),
            )
        });
        let palette = widgets::palette(ui);
        let mut refresh = false;
        widgets::page_header(
            ui,
            "Accounts",
            "Every registered account and its balances. Create a token, mint it, send or create accounts in a couple of clicks.",
            |ui| {
                refresh = ui
                    .add_enabled(
                        !self.busy && self.ready(),
                        egui::Button::new(format!("{}  Refresh accounts", icon::REFRESH)),
                    )
                    .clicked();
            },
        );
        if refresh {
            self.refresh_ledger();
        }
        let Some(selected) = &self.selected else {
            widgets::card(ui, |ui| {
                widgets::empty_state(
                    ui,
                    icon::ACCOUNTS,
                    "No environment selected",
                    "Quickstart a localnet: it comes with a funded developer account.",
                );
            });
            return;
        };
        let developer = selected.network.prepared().context.account_id.clone();
        if let Some(error) = &self.ledger_error {
            widgets::banner(
                ui,
                widgets::Tone::Warning,
                &format!("Account observation failed: {error}"),
                false,
            );
            ui.add_space(8.0);
        }
        let wide = ui.available_width() > 980.0;
        // Borrow the snapshot out of the desktop for this frame instead of cloning it.
        let ledger = self.ledger.take();
        let mut columns = |ui: &mut Ui, index: usize| match index {
            0 => self.developer_card(ui, &developer, ledger.as_ref()),
            _ => self.quick_action_card(ui, &developer, ledger.as_ref()),
        };
        if wide {
            ui.columns(2, |uis| {
                columns(&mut uis[0], 0);
                columns(&mut uis[1], 1);
            });
        } else {
            columns(ui, 0);
            ui.add_space(12.0);
            columns(ui, 1);
        }
        ui.add_space(12.0);
        self.transaction_preview(ui, ledger.as_ref());
        ui.add_space(12.0);
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::card_title(ui, icon::ACCOUNTS, "All accounts");
                if let Some(ledger) = &ledger {
                    widgets::pill(
                        ui,
                        ledger.accounts.entries.len().to_string(),
                        palette.muted,
                        palette.inset,
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.accounts_filter)
                            .hint_text("Filter accounts")
                            .desired_width(220.0),
                    );
                });
            });
            ui.add_space(6.0);
            let Some(ledger) = &ledger else {
                widgets::empty_state(
                    ui,
                    icon::ACCOUNTS,
                    "Nothing loaded yet",
                    "Accounts load automatically once validators are ready.",
                );
                return;
            };
            let filter = self.accounts_filter.trim().to_lowercase();
            let mut action = None;
            for (index, account) in ledger
                .account_ids()
                .filter(|account| filter.is_empty() || account.to_lowercase().contains(&filter))
                .enumerate()
            {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{:>2}", index + 1))
                            .monospace()
                            .color(palette.faint),
                    );
                    widgets::short_mono(ui, account, 10);
                    widgets::copy_button(ui, account, "account ID");
                    if account == developer {
                        widgets::pill(ui, "you · signer", palette.sakura, palette.sakura_soft);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(format!("{} Send", icon::SEND))
                            .on_hover_text("Send from your balance to this account")
                            .clicked()
                        {
                            action = Some((QuickKind::Transfer, account.to_owned()));
                        }
                        if ui
                            .small_button(format!("{} Mint", icon::MINT))
                            .on_hover_text("Mint one of your tokens to this account")
                            .clicked()
                        {
                            action = Some((QuickKind::Mint, account.to_owned()));
                        }
                    });
                });
                let holdings = ledger.holdings_of(account);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(28.0);
                    if holdings.is_empty() {
                        ui.label(RichText::new("No balances").small().color(palette.faint));
                    }
                    for holding in holdings {
                        widgets::pill(
                            ui,
                            format!(
                                "{} {}  {}",
                                icon::COINS,
                                holding.value,
                                ledger.asset_label(holding.definition)
                            ),
                            palette.text,
                            palette.inset,
                        )
                        .on_hover_text(holding.asset);
                    }
                });
            }
            if let Some((kind, account)) = action {
                self.quick.kind = kind;
                // An empty mint recipient is the developer, who is shown as "You".
                self.quick.recipient = if kind == QuickKind::Mint && account == developer {
                    String::new()
                } else {
                    account
                };
                self.preview = None;
                self.quick.seed(Some(ledger), &developer);
            }
            if ledger.accounts.remaining > 0 {
                widgets::hint(
                    ui,
                    &format!(
                        "{} more accounts are not shown. Use State to page through them.",
                        ledger.accounts.remaining
                    ),
                );
            }
        });
        if self.ledger.is_none() {
            self.ledger = ledger;
        }
    }

    fn developer_card(&mut self, ui: &mut Ui, developer: &str, ledger: Option<&LedgerSnapshot>) {
        let palette = widgets::palette(ui);
        widgets::accent_card(ui, Some(palette.sakura), |ui| {
            ui.horizontal(|ui| {
                widgets::card_title(ui, icon::KEY, "Developer account");
                widgets::pill(
                    ui,
                    "signs your transactions",
                    palette.sakura,
                    palette.sakura_soft,
                );
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::Label::new(RichText::new(developer).monospace().color(palette.text))
                        .wrap(),
                );
                widgets::copy_button(ui, developer, "developer account");
            });
            ui.add_space(6.0);
            ui.label(widgets::eyebrow(palette, "Balances"));
            match ledger {
                None => {
                    widgets::hint(ui, "Balances appear once the validators are ready.");
                }
                Some(ledger) => {
                    let holdings = ledger.holdings_of(developer);
                    if holdings.is_empty() {
                        widgets::hint(ui, "No balances yet.");
                    }
                    for holding in holdings {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(holding.value)
                                    .size(20.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.label(
                                RichText::new(ledger.asset_label(holding.definition))
                                    .strong()
                                    .color(palette.sakura),
                            )
                            .on_hover_text(holding.asset);
                            if ledger.mint_right(holding.definition, developer) == MintRight::Owner
                            {
                                widgets::pill(ui, "yours", palette.matcha, palette.matcha_soft)
                                    .on_hover_text("You own this token and can mint more of it");
                            }
                            widgets::copy_button(ui, holding.asset, "asset ID");
                        });
                    }
                }
            }
        });
    }

    fn quick_action_card(&mut self, ui: &mut Ui, developer: &str, ledger: Option<&LedgerSnapshot>) {
        let palette = widgets::palette(ui);
        let discriminant = self
            .selected
            .as_ref()
            .map(|selected| selected.network.address_discriminant());
        let mut review = false;
        widgets::card(ui, |ui| {
            widgets::card_title(ui, icon::MINT, "Quick actions");
            ui.add_space(4.0);
            let tabs = [
                (QuickKind::Mint, format!("{} Mint", icon::MINT)),
                (QuickKind::Transfer, format!("{} Send", icon::SEND)),
                (QuickKind::Burn, format!("{} Burn", icon::BURN)),
                (QuickKind::NewToken, format!("{} New token", icon::COINS)),
                (QuickKind::NewAccount, format!("{} New account", icon::NEW)),
            ];
            let tabs = tabs
                .iter()
                .map(|(kind, label)| (*kind, label.as_str()))
                .collect::<Vec<_>>();
            if widgets::segmented(ui, &mut self.quick.kind, &tabs) {
                self.preview = None;
            }
            ui.add_space(6.0);
            self.quick.seed(ledger, developer);
            let mut anyway = false;
            match self.quick.kind {
                QuickKind::Mint => anyway = self.mint_form(ui, developer, ledger),
                QuickKind::Transfer => {
                    ui.label(widgets::eyebrow(palette, "From your balance"));
                    self.holding_picker(ui, developer, ledger, "mochi-send-from");
                    ui.label(widgets::eyebrow(palette, "To"));
                    self.account_picker(ui, developer, ledger);
                    self.amount_field(ui, ledger, false);
                }
                QuickKind::Burn => {
                    ui.label(widgets::eyebrow(palette, "From your balance"));
                    self.holding_picker(ui, developer, ledger, "mochi-burn-from");
                    self.amount_field(ui, ledger, false);
                }
                QuickKind::NewToken => self.token_form(ui),
                QuickKind::NewAccount => {
                    ui.label(widgets::eyebrow(palette, "Fund from"));
                    self.holding_picker(ui, developer, ledger, "mochi-fund-from");
                    ui.label(widgets::eyebrow(palette, "Initial funding (optional)"));
                    ui.add(egui::TextEdit::singleline(&mut self.quick.fund).desired_width(160.0));
                    ui.add_space(4.0);
                    if ui
                        .button(format!("{}  Generate key pair", icon::KEY))
                        .on_hover_text("Creates a new Ed25519 key in memory only")
                        .clicked()
                    {
                        self.quick.generated = Some(GeneratedAccount::generate());
                        self.quick.reveal_key = false;
                        self.preview = None;
                    }
                    if let (Some(generated), Some(discriminant)) =
                        (&self.quick.generated, discriminant)
                    {
                        generated_key_card(ui, generated, discriminant, &mut self.quick.reveal_key);
                    }
                }
            }
            ui.add_space(8.0);
            let label = match self.quick.kind {
                QuickKind::Mint if anyway => "Review mint anyway",
                QuickKind::Mint => "Review mint",
                QuickKind::Transfer => "Review transfer",
                QuickKind::Burn => "Review burn",
                QuickKind::NewToken => "Review new token",
                QuickKind::NewAccount => "Review new account",
            };
            let problem = discriminant
                .and_then(|discriminant| self.quick.problem(discriminant, developer, ledger));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.busy && self.ready() && problem.is_none(),
                        widgets::primary_button(palette, label),
                    )
                    .clicked()
                {
                    review = true;
                }
                if let Some(problem) = &problem {
                    ui.label(RichText::new(problem).color(palette.muted));
                }
            });
            widgets::hint(
                ui,
                "You will see the exact signed transaction before anything is submitted.",
            );
        });
        if review {
            self.review_quick_action();
        }
    }

    /// Token, recipient and amount for a mint; returns whether the network is expected to
    /// reject it because another account owns the token.
    fn mint_form(&mut self, ui: &mut Ui, developer: &str, ledger: Option<&LedgerSnapshot>) -> bool {
        let palette = widgets::palette(ui);
        ui.label(widgets::eyebrow(palette, "Token"));
        let mut create = false;
        let definitions = ledger
            .map(|ledger| ledger.definitions_for(developer))
            .unwrap_or_default();
        let label = |definition: &str| {
            ledger.map_or_else(
                || widgets::short_middle(definition, 4),
                |ledger| ledger.asset_label(definition),
            )
        };
        let selected_text = if self.quick.definition.is_empty() {
            "Pick a token".to_owned()
        } else {
            label(&self.quick.definition)
        };
        egui::ComboBox::from_id_salt("mochi-mint-token")
            .selected_text(selected_text)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for definition in &definitions {
                    let owner = ledger.and_then(|ledger| ledger.owner_of(definition));
                    let suffix = match ledger.map(|ledger| ledger.mint_right(definition, developer))
                    {
                        Some(MintRight::Owner) => "you own it".to_owned(),
                        Some(MintRight::Unmintable) => "not mintable".to_owned(),
                        _ => owner.map_or_else(
                            || "owner unknown".to_owned(),
                            |owner| format!("owned by {}", account_label(owner, developer)),
                        ),
                    };
                    if ui
                        .selectable_label(
                            self.quick.definition == *definition,
                            format!("{} · {suffix}", label(definition)),
                        )
                        .on_hover_text(*definition)
                        .clicked()
                    {
                        self.quick.definition = (*definition).to_owned();
                    }
                }
                if !definitions.is_empty() {
                    ui.separator();
                }
                if ui
                    .button(format!("{}  Create a new token…", icon::NEW))
                    .clicked()
                {
                    create = true;
                }
            });
        if create {
            self.quick.kind = QuickKind::NewToken;
            self.preview = None;
            return false;
        }
        ui.label(widgets::eyebrow(palette, "To"));
        self.account_picker(ui, developer, ledger);
        self.amount_field(ui, ledger, true);
        let definition = self.quick.definition.trim();
        let guidance = match ledger {
            Some(ledger) if !definition.is_empty() => {
                let token = ledger.asset_label(definition);
                match ledger.mint_right(definition, developer) {
                    MintRight::Owner => MintGuidance::Owner(token),
                    MintRight::NotOwner => MintGuidance::NotOwner {
                        owner: ledger
                            .owner_of(definition)
                            .map(|owner| widgets::short_middle(owner, 6))
                            .unwrap_or_default(),
                        held: ledger
                            .holdings_of(developer)
                            .iter()
                            .any(|holding| holding.definition == definition),
                        token,
                    },
                    MintRight::Unmintable => MintGuidance::Unmintable(token),
                    MintRight::Unknown => MintGuidance::None,
                }
            }
            _ => MintGuidance::None,
        };
        ui.add_space(4.0);
        match guidance {
            MintGuidance::None => false,
            MintGuidance::Owner(token) => {
                ui.label(
                    RichText::new(format!(
                        "{} You own {token}, so you can mint any amount to any account.",
                        icon::OK
                    ))
                    .small()
                    .color(palette.matcha),
                );
                false
            }
            MintGuidance::Unmintable(token) => {
                widgets::banner(
                    ui,
                    widgets::Tone::Warning,
                    &format!("{token} can no longer be minted: its definition forbids it."),
                    false,
                );
                true
            }
            MintGuidance::NotOwner { token, owner, held } => {
                let mut send = false;
                let mut create = false;
                egui::Frame::new()
                    .fill(palette.kinako_soft)
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "{} Only {token}'s owner can mint it",
                                    icon::WARNING
                                ))
                                .strong()
                                .color(palette.text),
                            )
                            .wrap(),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "{token} belongs to {owner}. Your developer account can mint only tokens it owns, or ones whose owner granted it a mint permission, so the network will most likely reject this mint."
                                ))
                                .color(palette.muted),
                            )
                            .wrap(),
                        );
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if held
                                && ui
                                    .add(widgets::tonal_button(
                                        &format!("{}  Send {token} instead", icon::SEND),
                                        palette.text,
                                        palette.surface,
                                    ))
                                    .on_hover_text("You already hold some: send it from your balance")
                                    .clicked()
                            {
                                send = true;
                            }
                            if ui
                                .add(widgets::tonal_button(
                                    &format!("{}  Create your own token", icon::COINS),
                                    palette.on_sakura,
                                    palette.sakura,
                                ))
                                .clicked()
                            {
                                create = true;
                            }
                        });
                    });
                if send && let Some(ledger) = ledger {
                    let definition = self.quick.definition.clone();
                    self.quick.send_instead(ledger, developer, &definition);
                    self.preview = None;
                } else if create {
                    self.quick.kind = QuickKind::NewToken;
                    self.preview = None;
                }
                true
            }
        }
    }

    fn token_form(&mut self, ui: &mut Ui) {
        let palette = widgets::palette(ui);
        ui.label(widgets::eyebrow(palette, "Name"));
        if ui
            .add(
                egui::TextEdit::singleline(&mut self.quick.token.name)
                    .hint_text("e.g. Coffee")
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            self.quick.name_changed();
        }
        ui.label(widgets::eyebrow(palette, "Domain"));
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.quick.token.label)
                        .hint_text("coffee")
                        .desired_width(180.0),
                )
                .changed()
            {
                self.quick.token.label_edited = !self.quick.token.label.is_empty();
            }
            ui.label(RichText::new(".universal").monospace().color(palette.muted));
        });
        ui.label(widgets::eyebrow(palette, "First supply"));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.quick.token.supply).desired_width(160.0));
            let name = self.quick.token.name.trim();
            ui.label(
                RichText::new(if name.is_empty() {
                    "minted to you".to_owned()
                } else {
                    format!("{name} · minted to you")
                })
                .color(palette.muted),
            );
        });
        ui.add_space(4.0);
        let label = self.quick.token.label.trim();
        let domain = if label.is_empty() {
            "a domain".to_owned()
        } else {
            format!("{label}.universal")
        };
        widgets::hint(
            ui,
            &format!(
                "One transaction leases {domain} from the name service for a year (unless you already hold it), registers the token there and mints the first supply to you. The review shows the exact lease price. As its owner you can mint more any time."
            ),
        );
    }

    /// Amount input with the selected token's name as its unit.
    fn amount_field(&mut self, ui: &mut Ui, ledger: Option<&LedgerSnapshot>, mint: bool) {
        let palette = widgets::palette(ui);
        ui.label(widgets::eyebrow(palette, "Amount"));
        let definition = if mint {
            self.quick.definition.trim().to_owned()
        } else {
            self.quick
                .asset
                .split('#')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned()
        };
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.quick.quantity).desired_width(160.0));
            if let Some(ledger) = ledger
                && !definition.is_empty()
            {
                ui.label(
                    RichText::new(ledger.asset_label(&definition))
                        .strong()
                        .color(palette.muted),
                );
            }
        });
    }

    /// The developer's balances, by token name and amount held.
    fn holding_picker(
        &mut self,
        ui: &mut Ui,
        developer: &str,
        ledger: Option<&LedgerSnapshot>,
        id: &str,
    ) {
        let holdings = ledger
            .map(|ledger| ledger.holdings_of(developer))
            .unwrap_or_default();
        let label = |holding: &Holding<'_>| {
            let token = ledger.map_or_else(
                || widgets::short_middle(holding.definition, 4),
                |ledger| ledger.asset_label(holding.definition),
            );
            format!("{token} · {} held", holding.value)
        };
        let selected_text = holdings
            .iter()
            .find(|holding| holding.asset == self.quick.asset)
            .map_or_else(
                || {
                    if holdings.is_empty() {
                        "No balances yet".to_owned()
                    } else {
                        "Pick a balance".to_owned()
                    }
                },
                label,
            );
        ui.add_enabled_ui(!holdings.is_empty(), |ui| {
            egui::ComboBox::from_id_salt(id)
                .selected_text(selected_text)
                .width(ui.available_width() - 8.0)
                .show_ui(ui, |ui| {
                    for holding in &holdings {
                        if ui
                            .selectable_label(self.quick.asset == holding.asset, label(holding))
                            .on_hover_text(holding.asset)
                            .clicked()
                        {
                            self.quick.asset = holding.asset.to_owned();
                        }
                    }
                });
        });
    }

    /// Recipient field with a pick-from-list menu; an empty mint recipient means the developer.
    fn account_picker(&mut self, ui: &mut Ui, developer: &str, ledger: Option<&LedgerSnapshot>) {
        let palette = widgets::palette(ui);
        let mint = self.quick.kind == QuickKind::Mint;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.quick.recipient)
                    .hint_text(if mint {
                        "You (developer account), or paste an account ID"
                    } else {
                        "Paste an account ID or pick one"
                    })
                    .desired_width(ui.available_width() - MENU_BUTTON_ROOM),
            );
            let accounts = ledger
                .map(|ledger| ledger.account_ids().map(str::to_owned).collect::<Vec<_>>())
                .unwrap_or_default();
            ui.menu_button(icon::MENU, |ui| {
                if ui.button("You · developer account").clicked() {
                    self.quick.recipient = if mint {
                        String::new()
                    } else {
                        developer.to_owned()
                    };
                    ui.close();
                }
                if !accounts.is_empty() {
                    ui.separator();
                }
                for account in &accounts {
                    let text = if account == developer {
                        format!("{} · you", widgets::short_middle(account, 12))
                    } else {
                        widgets::short_middle(account, 12)
                    };
                    if ui.button(text).on_hover_text(account).clicked() {
                        self.quick.recipient = if mint && account == developer {
                            String::new()
                        } else {
                            account.clone()
                        };
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("Pick a registered account");
        });
        let recipient = self.quick.recipient.trim();
        let (text, color) = if recipient.is_empty() {
            if mint {
                ("→ your developer account".to_owned(), palette.muted)
            } else {
                (String::new(), palette.muted)
            }
        } else if recipient == developer {
            ("→ your developer account".to_owned(), palette.muted)
        } else if ledger.is_some_and(|ledger| ledger.account_ids().any(|id| id == recipient)) {
            ("→ a registered account".to_owned(), palette.muted)
        } else if ledger.is_some() {
            (
                "→ not registered here yet: the network refuses it unless implicit account creation is allowed".to_owned(),
                palette.kinako,
            )
        } else {
            (String::new(), palette.muted)
        };
        if !text.is_empty() {
            ui.add(egui::Label::new(RichText::new(text).small().color(color)).wrap());
        }
    }

    /// Turn the quick action into the shared reviewed preview.
    pub(super) fn review_quick_action(&mut self) {
        let Some(selected) = &self.selected else {
            return;
        };
        let discriminant = selected.network.address_discriminant();
        let developer = selected.network.prepared().context.account_id.clone();
        if self.quick.kind == QuickKind::NewToken {
            let token = &self.quick.token;
            let (name, label, supply) = (
                token.name.trim().to_owned(),
                token.label.trim().to_owned(),
                token.supply.trim().to_owned(),
            );
            self.request_token_preview(name, label, supply);
            return;
        }
        let reviewed = {
            let _address_profile =
                iroha_data_model::account::address::ChainDiscriminantGuard::enter(discriminant);
            self.quick
                .drafts(discriminant, &developer)
                .and_then(|drafts| {
                    let json =
                        mochi_core::drafts_to_pretty_json(&drafts).map_err(|e| e.to_string())?;
                    Ok((drafts, json))
                })
        };
        match reviewed {
            Ok((drafts, json)) => self.request_preview(drafts, Some(json)),
            Err(error) => self.error = Some(error),
        }
    }
}

fn generated_key_card(
    ui: &mut Ui,
    generated: &GeneratedAccount,
    discriminant: u16,
    reveal: &mut bool,
) {
    let palette = widgets::palette(ui);
    egui::Frame::new()
        .fill(palette.kinako_soft)
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let account = generated.account_literal(discriminant);
            widgets::copy_field(ui, "New account", &account);
            widgets::copy_field(ui, "Public key", &generated.public_key());
            ui.horizontal_wrapped(|ui| {
                ui.label(widgets::eyebrow(palette, "Private key"));
                let private = generated.exposed_private_key();
                if *reveal {
                    ui.add(egui::Label::new(RichText::new(&private).monospace()).wrap());
                } else {
                    ui.label(RichText::new("•".repeat(24)).monospace().color(palette.muted));
                }
                if ui.small_button(if *reveal { "Hide" } else { "Reveal" }).clicked() {
                    *reveal = !*reveal;
                }
                widgets::copy_button(ui, &private, "private key");
            });
            ui.label(
                RichText::new(format!(
                    "{} Localnet test key. Mochi keeps it in memory only and forgets it when you switch environments.",
                    icon::WARNING
                ))
                .small()
                .color(palette.kinako),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::account::address::ChainDiscriminantGuard;

    const DISCRIMINANT: u16 = 0x02F1;

    fn entry(
        title: &str,
        owner: Option<&str>,
        definition: Option<&str>,
        detail: &str,
    ) -> StateEntry {
        StateEntry {
            title: title.into(),
            subtitle: String::new(),
            detail: detail.into(),
            raw: String::new(),
            domain: None,
            domain_lower: None,
            owner: owner.map(Into::into),
            owner_lower: owner.map(str::to_lowercase),
            asset_definition: definition.map(Into::into),
            asset_definition_lower: definition.map(str::to_lowercase),
            json: None,
            norito_bytes: None,
            search_blob: String::new(),
        }
    }

    fn page(kind: mochi_core::StateQueryKind, entries: Vec<StateEntry>) -> StatePage {
        StatePage {
            kind,
            entries,
            remaining: 0,
            cursor: None,
        }
    }

    fn definition(title: &str, owner: Option<&str>, json: &str) -> StateEntry {
        StateEntry {
            json: Some(json.into()),
            ..entry(title, owner, Some(title), "")
        }
    }

    #[test]
    fn holdings_group_by_exact_owner_and_keep_scoped_literals() {
        let ledger = LedgerSnapshot::new(
            page(
                mochi_core::StateQueryKind::Accounts,
                vec![
                    entry("alice", Some("signatory-a"), None, ""),
                    entry("bob", None, None, ""),
                ],
            ),
            page(
                mochi_core::StateQueryKind::Assets,
                vec![
                    entry("rose#alice", Some("alice"), Some("rose"), "Value 10"),
                    entry(
                        "tea#alice#dataspace:3",
                        Some("alice"),
                        Some("tea"),
                        "Value 2.5",
                    ),
                    entry("rose#bob", Some("bob"), Some("rose"), "Value 1"),
                    entry("orphan", Some("carol"), None, "Value 4"),
                ],
            ),
            &page(
                mochi_core::StateQueryKind::AssetDefinitions,
                vec![
                    definition("rose", None, r#"{"id": "rose", "name": "Rose"}"#),
                    definition("tea", None, r#"{"id": "tea", "name": "  "}"#),
                    definition("6pEP9RjNoZ7beWkT3pLfKoM1dyfi", None, "not json"),
                ],
            ),
        );
        assert_eq!(ledger.asset_label("rose"), "Rose");
        assert_eq!(ledger.asset_label("tea"), "tea", "blank names fall back");
        assert_eq!(
            ledger.asset_label("6pEP9RjNoZ7beWkT3pLfKoM1dyfi"),
            "6pEP…dyfi"
        );
        assert_eq!(ledger.account_ids().collect::<Vec<_>>(), ["alice", "bob"]);
        assert_eq!(
            ledger.holdings_of("alice"),
            [
                Holding {
                    asset: "rose#alice",
                    definition: "rose",
                    value: "10"
                },
                Holding {
                    asset: "tea#alice#dataspace:3",
                    definition: "tea",
                    value: "2.5"
                },
            ]
        );
        assert_eq!(ledger.holdings_of("ALICE"), []);
    }

    /// The localnet shape: genesis owns XOR, the developer owns Coffee and Spent is closed.
    fn owned_ledger() -> LedgerSnapshot {
        LedgerSnapshot::new(
            page(
                mochi_core::StateQueryKind::Accounts,
                vec![
                    entry("dev", None, None, ""),
                    entry("genesis", None, None, ""),
                ],
            ),
            page(
                mochi_core::StateQueryKind::Assets,
                vec![entry("xor#dev", Some("dev"), Some("xor"), "Value 10")],
            ),
            &page(
                mochi_core::StateQueryKind::AssetDefinitions,
                vec![
                    definition(
                        "xor",
                        Some("genesis"),
                        r#"{"name": "XOR", "mintable": "Infinitely"}"#,
                    ),
                    definition(
                        "coffee",
                        Some("dev"),
                        r#"{"name": "Coffee", "mintable": "Infinitely"}"#,
                    ),
                    definition(
                        "spent",
                        Some("dev"),
                        r#"{"name": "Spent", "mintable": "Not"}"#,
                    ),
                    definition("unowned", None, r#"{"name": "Loose"}"#),
                ],
            ),
        )
    }

    #[test]
    fn mint_rights_follow_the_definition_owner_and_own_tokens_come_first() {
        let ledger = owned_ledger();
        assert_eq!(ledger.mint_right("coffee", "dev"), MintRight::Owner);
        assert_eq!(ledger.mint_right("xor", "dev"), MintRight::NotOwner);
        assert_eq!(ledger.mint_right("xor", "genesis"), MintRight::Owner);
        assert_eq!(ledger.mint_right("spent", "dev"), MintRight::Unmintable);
        assert_eq!(ledger.mint_right("unowned", "dev"), MintRight::Unknown);
        assert_eq!(ledger.mint_right("absent", "dev"), MintRight::Unknown);
        assert_eq!(ledger.owner_of("xor"), Some("genesis"));
        assert_eq!(
            ledger.definitions_for("dev"),
            ["coffee", "spent", "xor", "unowned"]
        );

        let mut quick = QuickActions::default();
        quick.seed(Some(&ledger), "dev");
        assert_eq!(quick.definition, "coffee", "Mint starts on a token you own");
        assert_eq!(quick.asset, "xor#dev", "Send starts on your first balance");
        quick.recipient = "genesis".into();
        quick.send_instead(&ledger, "dev", "xor");
        assert_eq!(quick.kind, QuickKind::Transfer);
        assert_eq!(
            (quick.asset.as_str(), quick.recipient.as_str()),
            ("xor#dev", "genesis")
        );
    }

    #[test]
    fn quick_actions_compose_exact_drafts_or_explain_what_is_missing() {
        let developer = GeneratedAccount::generate();
        let recipient = GeneratedAccount::generate();
        let developer_literal = developer.account_literal(DISCRIMINANT);
        // Canonical Base58 asset definition used by the shared composer fixtures.
        let definition = "6pEP9RjNoZ7beWkT3pLfKoM1dyfi";
        let asset = asset_literal(definition, &developer_literal, DISCRIMINANT).unwrap();
        let _guard = ChainDiscriminantGuard::enter(DISCRIMINANT);
        let mut quick = QuickActions {
            definition: String::new(),
            asset: asset.clone(),
            quantity: "5".into(),
            ..QuickActions::default()
        };
        assert_eq!(
            quick.drafts(DISCRIMINANT, &developer_literal).unwrap_err(),
            "Pick a token to mint."
        );
        quick.definition = definition.into();
        let drafts = quick.drafts(DISCRIMINANT, &developer_literal).unwrap();
        assert!(
            matches!(&drafts[..], [InstructionDraft::MintAsset { asset: minted, .. }] if minted.to_string() == asset),
            "an empty recipient mints to the developer"
        );

        quick.recipient = recipient.account_literal(DISCRIMINANT);
        let drafts = quick.drafts(DISCRIMINANT, &developer_literal).unwrap();
        assert!(matches!(
            &drafts[..],
            [InstructionDraft::MintAsset { asset: minted, .. }] if minted.account() == recipient.account_id()
        ));
        assert_eq!(
            describe_drafts(&drafts, None, &developer_literal)[0],
            format!(
                "Mint 5 6pEP…dyfi to {}",
                widgets::short_middle(&recipient.account_literal(DISCRIMINANT), 6)
            )
        );

        quick.kind = QuickKind::Transfer;
        assert!(matches!(
            &quick.drafts(DISCRIMINANT, &developer_literal).unwrap()[..],
            [InstructionDraft::TransferAsset { destination, .. }] if destination == recipient.account_id()
        ));
        quick.recipient.clear();
        assert_eq!(
            quick.drafts(DISCRIMINANT, &developer_literal).unwrap_err(),
            "Choose who receives it."
        );
        quick.kind = QuickKind::Burn;
        assert!(matches!(
            &quick.drafts(DISCRIMINANT, &developer_literal).unwrap()[..],
            [InstructionDraft::BurnAsset { .. }]
        ));
        quick.quantity = "not a number".into();
        assert_eq!(
            quick.drafts(DISCRIMINANT, &developer_literal).unwrap_err(),
            "Amounts are plain decimals such as 100 or 2.5."
        );
        quick.quantity = " ".into();
        assert_eq!(
            quick
                .problem(DISCRIMINANT, &developer_literal, None)
                .as_deref(),
            Some("Enter an amount.")
        );

        quick.kind = QuickKind::NewAccount;
        assert!(
            quick
                .drafts(DISCRIMINANT, &developer_literal)
                .unwrap_err()
                .contains("Generate a key pair")
        );
        quick.generated = Some(recipient.clone());
        let drafts = quick.drafts(DISCRIMINANT, &developer_literal).unwrap();
        assert!(matches!(
            &drafts[..],
            [
                InstructionDraft::RegisterAccount { account },
                InstructionDraft::TransferAsset { destination, .. },
            ] if account == recipient.account_id() && destination == recipient.account_id()
        ));
        quick.fund.clear();
        assert_eq!(
            quick
                .drafts(DISCRIMINANT, &developer_literal)
                .unwrap()
                .len(),
            1
        );
    }

    fn card_frame(desktop: &mut Desktop, ledger: &LedgerSnapshot) -> egui::FullOutput {
        let context = egui::Context::default();
        super::super::tests::run_headless_frame(&context, egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                desktop.quick_action_card(ui, "dev", Some(ledger))
            });
        })
    }

    #[test]
    fn mint_warns_before_review_when_another_account_owns_the_token() {
        let ledger = owned_ledger();
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        let output = card_frame(&mut desktop, &ledger);
        assert_eq!(
            desktop.quick.definition, "coffee",
            "your own token comes first"
        );
        for text in [
            format!(
                "{} You own Coffee, so you can mint any amount to any account.",
                icon::OK
            ),
            "Review mint".to_owned(),
            "→ your developer account".to_owned(),
        ] {
            assert!(
                super::super::tests::text_position(&output, &text).is_some(),
                "{text}"
            );
        }

        desktop.quick.definition = "xor".into();
        desktop.quick.recipient = "genesis".into();
        let output = card_frame(&mut desktop, &ledger);
        let send = format!("{}  Send XOR instead", icon::SEND);
        for text in [
            format!("{} Only XOR's owner can mint it", icon::WARNING),
            "Review mint anyway".to_owned(),
            format!("{}  Create your own token", icon::COINS),
            send.clone(),
            "→ a registered account".to_owned(),
        ] {
            assert!(
                super::super::tests::text_position(&output, &text).is_some(),
                "{text}"
            );
        }
        let context = egui::Context::default();
        super::super::tests::click_label(&context, &send, |ui| {
            desktop.quick_action_card(ui, "dev", Some(&ledger));
        });
        assert_eq!(desktop.quick.kind, QuickKind::Transfer);
        assert_eq!(desktop.quick.asset, "xor#dev");
        assert_eq!(
            desktop.quick.recipient, "genesis",
            "the recipient carries over"
        );
    }

    #[test]
    fn new_tokens_follow_their_name_and_refuse_a_token_that_already_exists() {
        let developer = GeneratedAccount::generate();
        let developer_literal = developer.account_literal(DISCRIMINANT);
        let _guard = ChainDiscriminantGuard::enter(DISCRIMINANT);
        let mut quick = QuickActions {
            kind: QuickKind::NewToken,
            ..QuickActions::default()
        };
        assert_eq!(
            quick
                .problem(DISCRIMINANT, &developer_literal, None)
                .as_deref(),
            Some("Name your token.")
        );
        quick.token.name = "Coffee Beans".into();
        quick.name_changed();
        assert_eq!(
            quick.token.label, "coffee-beans",
            "the label follows the name"
        );
        quick.token.label = "beans".into();
        quick.token.label_edited = true;
        quick.token.name = "Coffee".into();
        quick.name_changed();
        assert_eq!(quick.token.label, "beans", "an edited label stays");
        assert_eq!(quick.problem(DISCRIMINANT, &developer_literal, None), None);

        let token = quick.token_draft(&developer_literal).unwrap();
        let lines = describe_drafts(&token.drafts, None, &developer_literal);
        assert_eq!(
            lines,
            [
                "Create token Coffee in beans.universal".to_owned(),
                "Mint 1000 Coffee to you".to_owned(),
            ],
            "the token is named before the ledger observes it"
        );

        quick.token.label = "Beans Shop".into();
        assert_eq!(
            quick
                .problem(DISCRIMINANT, &developer_literal, None)
                .as_deref(),
            Some("Domain labels use lowercase letters, digits and dashes.")
        );
        quick.token.label = "beans".into();
        let existing = token.definition.to_string();
        let ledger = LedgerSnapshot::new(
            page(mochi_core::StateQueryKind::Accounts, Vec::new()),
            page(mochi_core::StateQueryKind::Assets, Vec::new()),
            &page(
                mochi_core::StateQueryKind::AssetDefinitions,
                vec![definition(
                    &existing,
                    Some(&developer_literal),
                    r#"{"name": "Coffee"}"#,
                )],
            ),
        );
        assert_eq!(
            quick
                .problem(DISCRIMINANT, &developer_literal, Some(&ledger))
                .as_deref(),
            Some("Coffee already exists in beans.universal: mint more from Mint.")
        );
    }
}
