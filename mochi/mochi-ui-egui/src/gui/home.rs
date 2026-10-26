//! Home: the Quickstart welcome when nothing is selected, otherwise a live overview of the
//! validators, the developer account, the latest blocks and quick actions.

use super::{
    Desktop, LocalnetDialog, Message, UiResult, View,
    accounts::QuickKind,
    brand::{self, Mood},
    ledger::BlockRow,
    theme,
    widgets::{self, icon},
};
use eframe::egui::{self, Align, Layout, RichText, Sense, Ui, Vec2};
use mochi_core::{
    developer::{ManagedPhase, PreparedLocalnet},
    state::BlockTip,
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// How often Home compares the validators' newest blocks.
const BLOCK_TIP_INTERVAL: Duration = Duration::from_secs(2);
/// How long one validator may take to report its newest block.
const BLOCK_TIP_TIMEOUT: Duration = Duration::from_secs(2);

/// Name Quickstart gives its new localnet: the first unused `local`, `local-2`, ….
pub(crate) fn quickstart_name(names: &[String]) -> String {
    LocalnetDialog::new(names).name
}

/// One validator's newest block (`None` before its first), or why it could not be read.
pub(crate) type TipObservation = Result<Option<BlockTip>, String>;

/// The validators' newest blocks, read for the Home cards.
#[derive(Default)]
pub(crate) struct BlockTips {
    /// The latest read, one entry per validator.
    pub(super) observed: Vec<TipObservation>,
    /// The newest height any read before `observed` saw.
    pub(super) settled: u64,
    /// A read is in flight; there is never more than one.
    pub(super) pending: bool,
    /// When the latest read started; `None` reads at once.
    pub(super) started: Option<Instant>,
}

impl BlockTips {
    fn record(&mut self, observed: UiResult<Vec<TipObservation>>) {
        // Each validator should since have reached the newest height an earlier read saw.
        self.settled = self.settled.max(newest_height(&self.observed));
        // A failed read shows nothing rather than a comparison of outdated blocks.
        self.observed = observed.unwrap_or_default();
    }
}

/// How one validator's newest block compares with the others'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TipSync {
    /// At the newest height, or reaching it while a commit spreads.
    Current,
    /// Still below the newest height an earlier read saw; counted to the newest height now.
    Behind(u64),
    /// Its block differs from another one at this height that at least as many report.
    Diverged,
    /// It did not report its newest block.
    Unanswered,
}

/// Compare each validator's newest block with the others'. A commit reaches every validator
/// within moments, so a lower height is a lag only while it stays below `settled`, the newest
/// height an earlier read saw; a validator one block short mid-commit is not flagged.
pub(crate) fn tip_sync(observed: &[TipObservation], settled: u64) -> Vec<TipSync> {
    let newest = newest_height(observed).max(settled);
    observed
        .iter()
        .map(|tip| match tip {
            Err(_) => TipSync::Unanswered,
            Ok(Some(own)) if outvoted(observed, own) => TipSync::Diverged,
            Ok(tip) => match tip.map_or(0, |tip| tip.height) {
                height if height < settled => TipSync::Behind(newest - height),
                _ => TipSync::Current,
            },
        })
        .collect()
}

fn tips(observed: &[TipObservation]) -> impl Iterator<Item = &BlockTip> {
    observed
        .iter()
        .filter_map(|tip| tip.as_ref().ok()?.as_ref())
}

fn newest_height(observed: &[TipObservation]) -> u64 {
    tips(observed).map(|tip| tip.height).max().unwrap_or(0)
}

/// Whether another block at `own`'s height is reported at least as often as `own`'s block.
fn outvoted(observed: &[TipObservation], own: &BlockTip) -> bool {
    let votes = |hash| {
        tips(observed)
            .filter(|tip| tip.height == own.height && tip.hash == hash)
            .count()
    };
    let own_votes = votes(own.hash);
    tips(observed)
        .any(|tip| tip.height == own.height && tip.hash != own.hash && votes(tip.hash) >= own_votes)
}

/// One pill for the whole committee: in sync, or how many validators are not.
fn sync_summary(ui: &mut Ui, sync: &[TipSync]) {
    let palette = widgets::palette(ui);
    let out = sync
        .iter()
        .filter(|sync| **sync != TipSync::Current)
        .count();
    if out == 0 {
        widgets::pill(ui, "in sync", palette.matcha, palette.matcha_soft)
            .on_hover_text("No validator has fallen behind or reports a different block");
        return;
    }
    let failed = sync
        .iter()
        .any(|sync| matches!(sync, TipSync::Diverged | TipSync::Unanswered));
    let (fg, bg) = if failed {
        (palette.aka, palette.aka_soft)
    } else {
        (palette.kinako, palette.kinako_soft)
    };
    widgets::pill(ui, format!("{out} out of sync"), fg, bg);
}

/// One validator's newest block. Being out of sync is flagged on a line of its own, so a
/// narrow card never squeezes the flag.
fn block_tip_row(ui: &mut Ui, tip: Option<&TipObservation>, sync: Option<TipSync>, newest: u64) {
    let palette = widgets::palette(ui);
    match tip {
        None => {
            ui.label(
                RichText::new("Reading latest block…")
                    .small()
                    .color(palette.faint),
            );
        }
        Some(Ok(None)) => {
            ui.label(RichText::new("No blocks yet").small().color(palette.muted));
        }
        Some(Ok(Some(tip))) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(
                    RichText::new(format!("#{}", tip.height))
                        .strong()
                        .color(palette.sky),
                );
                let hash = tip.hash.to_string();
                ui.label(
                    RichText::new(widgets::short_middle(&hash, 8))
                        .monospace()
                        .small()
                        .color(palette.muted),
                )
                .on_hover_text(&hash);
                widgets::copy_button(ui, &hash, "block hash");
            });
        }
        Some(Err(_)) => {}
    }
    let (flag, fg, bg, detail) = match (sync, tip) {
        (Some(TipSync::Behind(blocks)), _) => (
            format!(
                "{blocks} block{} behind",
                if blocks == 1 { "" } else { "s" }
            ),
            palette.kinako,
            palette.kinako_soft,
            format!("The newest block is #{newest}"),
        ),
        (Some(TipSync::Diverged), _) => (
            "hash differs".to_owned(),
            palette.aka,
            palette.aka_soft,
            "Other validators report a different block at this height".to_owned(),
        ),
        (Some(TipSync::Unanswered), Some(Err(error))) => (
            "no response".to_owned(),
            palette.aka,
            palette.aka_soft,
            super::bounded_text(error),
        ),
        _ => return,
    };
    ui.horizontal(|ui| {
        widgets::pill(ui, flag, fg, bg).on_hover_text(detail);
    });
}

/// The newest streamed blocks, one clickable row each; returns the height of a clicked row.
fn latest_blocks(ui: &mut Ui, feed: &VecDeque<BlockRow>, now: u64) -> Option<u64> {
    let palette = widgets::palette(ui);
    let mut opened = None;
    for row in feed.iter().take(5) {
        // Painted behind the row once its hover is known.
        let highlight = ui.painter().add(egui::Shape::Noop);
        // Selectable labels would take the click for text selection; the row owns it.
        let label = |ui: &mut Ui, text: RichText| {
            ui.add(egui::Label::new(text).selectable(false));
        };
        let response = ui
            .horizontal(|ui| {
                label(
                    ui,
                    RichText::new(format!("#{}", row.height))
                        .strong()
                        .color(palette.sky),
                );
                label(
                    ui,
                    RichText::new(format!(
                        "{} tx{}",
                        row.transactions,
                        if row.rejected > 0 {
                            format!(" · {} rejected", row.rejected)
                        } else {
                            String::new()
                        }
                    )),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    label(
                        ui,
                        RichText::new(widgets::time_ago(row.created_ms, now))
                            .small()
                            .color(palette.muted),
                    );
                });
            })
            .response
            .interact(Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("Open block #{}", row.height));
        if response.hovered() {
            ui.painter().set(
                highlight,
                egui::Shape::rect_filled(
                    response.rect.expand(4.0),
                    egui::CornerRadius::same(8),
                    palette.inset,
                ),
            );
        }
        if response.clicked() {
            opened = Some(row.height);
        }
    }
    opened
}

impl Desktop {
    pub(super) fn home(&mut self, ui: &mut Ui) {
        if self.selected.is_some() {
            self.overview(ui);
        } else {
            self.welcome(ui);
        }
    }

    /// Create a new four-validator localnet with a fresh name; it is selected when ready.
    pub(super) fn quickstart(&mut self) {
        if self.busy || self.workspace.is_none() {
            return;
        }
        let name = quickstart_name(&self.names);
        self.start_localnet(name);
    }

    /// Read every validator's newest block while Home shows a ready network. Like attachment
    /// progress, the read never marks the desktop busy, so the cards' controls stay enabled.
    pub(super) fn poll_block_tips(&mut self) {
        let due = self
            .block_tips
            .started
            .is_none_or(|started| started.elapsed() >= BLOCK_TIP_INTERVAL);
        if self.view != View::Home || !self.ready() || self.block_tips.pending || !due {
            return;
        }
        let (Some(selected), Some(runtime)) = (&self.selected, &self.runtime) else {
            return;
        };
        let (network, handle) = (selected.network.clone(), runtime.handle().clone());
        let (sender, epoch) = (self.sender.clone(), self.epoch);
        self.block_tips.pending = true;
        self.block_tips.started = Some(Instant::now());
        std::thread::spawn(move || {
            let tips = handle
                .block_on(network.block_tips(BLOCK_TIP_TIMEOUT))
                .map(|tips| {
                    tips.into_iter()
                        .map(|tip| tip.map_err(|error| error.to_string()))
                        .collect()
                })
                .map_err(|error| error.to_string());
            let prepared = network.prepared().clone();
            let _ = sender.send((epoch, Message::BlockTips { prepared, tips }));
        });
    }

    /// Show a read only on the generation it came from; any reply ends the read in flight.
    pub(super) fn record_block_tips(
        &mut self,
        prepared: &PreparedLocalnet,
        tips: UiResult<Vec<TipObservation>>,
    ) {
        self.block_tips.pending = false;
        let shown = self
            .selected
            .as_ref()
            .map(|selected| selected.network.prepared());
        if super::validate_retained_selection(shown, prepared).is_ok() {
            self.block_tips.record(tips);
        }
    }

    fn welcome(&mut self, ui: &mut Ui) {
        let palette = widgets::palette(ui);
        let mood = if self.busy {
            Mood::Excited
        } else {
            Mood::Curious
        };
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                brand::mascot(ui, self.brand.as_ref(), 168.0, mood);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("Your Iroha playground")
                            .size(30.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(
                                "Spin up four validators with a funded developer account and live endpoints. No configuration files.",
                            )
                            .size(15.0)
                            .color(palette.muted),
                        )
                        .wrap(),
                    );
                    ui.add_space(10.0);
                    ui.horizontal_wrapped(|ui| {
                        let ready = !self.busy && self.workspace.is_some();
                        if ui
                            .add_enabled(
                                ready,
                                widgets::primary_button(
                                    palette,
                                    &format!("{}  Quickstart", icon::QUICKSTART),
                                )
                                .min_size(Vec2::new(150.0, 40.0)),
                            )
                            .on_hover_text("Create and start a new localnet with an unused name")
                            .clicked()
                        {
                            self.quickstart();
                        }
                        if ui
                            .add_enabled(
                                ready,
                                egui::Button::new(format!("{}  New localnet…", icon::NEW))
                                    .min_size(Vec2::new(0.0, 40.0)),
                            )
                            .clicked()
                        {
                            self.localnet_dialog = Some(LocalnetDialog::new(&self.names));
                        }
                        if ui
                            .add_enabled(
                                ready,
                                egui::Button::new(format!("{}  Private dataspace…", icon::PRIVATE))
                                    .min_size(Vec2::new(0.0, 40.0)),
                            )
                            .clicked()
                        {
                            self.private_dialog = true;
                        }
                    });
                    if self.busy && self.foreground {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(format!(
                                    "Preparing validators… {}",
                                    widgets::elapsed(self.busy_since.elapsed())
                                ))
                                .color(palette.muted),
                            );
                        });
                    }
                });
            });
        });
        if let Some(name) = &self.attaching {
            ui.add_space(12.0);
            widgets::card(ui, |ui| {
                widgets::card_title(
                    ui,
                    icon::PRIVATE,
                    &format!("Setting up private dataspace {name}…"),
                );
                if let Some(status) = &self.attachment_progress {
                    ui.horizontal(|ui| {
                        widgets::phase_pill(ui, status.local.phase);
                        ui.label(format!(
                            "{} / 4 local validators",
                            status.local.running_peers
                        ));
                    });
                    super::attachment_details(ui, &status.attachment);
                }
            });
        }
        ui.add_space(12.0);
        let tiles = [
            (
                icon::BLOCKS,
                "Four validators",
                "A real Sumeragi network on your machine, not a simulator.",
            ),
            (
                icon::COINS,
                "Funded account",
                "A generated developer signer with XOR to send, ready to create and mint tokens.",
            ),
            (
                icon::ENDPOINTS,
                "Live endpoints",
                "Torii, metrics and MCP URLs to paste into tests.",
            ),
            (
                icon::CONTRACTS,
                "Contracts",
                "Deploy Kotodama with an exact fee review.",
            ),
        ];
        let columns = if ui.available_width() > 900.0 { 4 } else { 2 };
        widgets::card_grid(ui, &tiles, columns, |ui, (glyph, title, body)| {
            widgets::card(ui, |ui| {
                widgets::icon_badge(ui, glyph, palette.sakura, palette.sakura_soft);
                ui.label(RichText::new(*title).strong().color(palette.text));
                widgets::hint(ui, body);
            });
        });
        if !self.names.is_empty() {
            ui.add_space(16.0);
            ui.label(
                RichText::new("Your environments")
                    .size(18.0)
                    .strong()
                    .color(palette.text),
            );
            widgets::hint(
                ui,
                "Kept by this workspace and shared with Kagami. Opening one does not start it.",
            );
            ui.add_space(4.0);
            let mut chosen = None;
            ui.horizontal_wrapped(|ui| {
                for name in &self.names {
                    ui.allocate_ui_with_layout(
                        Vec2::new(220.0, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            widgets::card(ui, |ui| {
                                ui.horizontal(|ui| {
                                    widgets::icon_badge(
                                        ui,
                                        icon::BLOCKS,
                                        palette.sky,
                                        palette.sky_soft,
                                    );
                                    ui.label(RichText::new(name).strong().color(palette.text));
                                });
                                if ui
                                    .add_enabled(!self.busy, egui::Button::new("Open"))
                                    .clicked()
                                {
                                    chosen = Some(name.clone());
                                }
                            });
                        },
                    );
                }
            });
            if let Some(name) = chosen {
                self.select(name);
            }
        }
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            widgets::hint(ui, "Closing Mochi leaves your networks running, and");
            ui.code("kagami localnet up");
            widgets::hint(ui, "works on the same environments.");
        });
    }

    fn overview(&mut self, ui: &mut Ui) {
        let palette = widgets::palette(ui);
        let Some(selected) = &self.selected else {
            return;
        };
        let prepared = selected.network.prepared().clone();
        let phase = selected.phase;
        let attachment = selected.attachment.clone();
        let context = &prepared.context;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&context.name)
                    .size(26.0)
                    .strong()
                    .color(palette.text),
            );
            widgets::phase_pill(ui, phase);
            ui.label(
                RichText::new(&context.chain_id)
                    .monospace()
                    .color(palette.muted),
            );
        });
        widgets::hint(
            ui,
            "Kagami and Mochi share this network. It stays available when you close the desktop.",
        );
        ui.add_space(10.0);
        // Blocks are compared only while every validator is supervised as ready.
        let tips = (phase == ManagedPhase::Ready).then_some(&self.block_tips.observed);
        let sync = tips.map_or_else(Vec::new, |tips| tip_sync(tips, self.block_tips.settled));
        let newest = tips.map_or(0, |tips| newest_height(tips).max(self.block_tips.settled));
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Validators")
                    .size(17.0)
                    .strong()
                    .color(palette.text),
            );
            if tips.is_some_and(|tips| !tips.is_empty()) {
                sync_summary(ui, &sync);
            }
        });
        ui.add_space(2.0);
        let columns = if ui.available_width() > 1000.0 { 4 } else { 2 };
        let mut observe = None;
        let mut open = None;
        let peers = prepared.peers.iter().enumerate().collect::<Vec<_>>();
        let (busy, observed_peer) = (self.busy, self.peer);
        widgets::card_grid(ui, &peers, columns, |ui, (index, peer)| {
            let (index, observed) = (*index, *index == observed_peer);
            widgets::accent_card(ui, observed.then_some(palette.sakura), |ui| {
                ui.horizontal(|ui| {
                    let (fg, bg) = theme::phase_colors(palette, phase);
                    widgets::icon_badge(ui, icon::BLOCKS, fg, bg);
                    ui.label(
                        RichText::new(format!("Validator {}", index + 1))
                            .strong()
                            .color(palette.text),
                    );
                    if observed {
                        widgets::pill(ui, "observing", palette.sakura, palette.sakura_soft);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(&peer.torii_url)
                            .monospace()
                            .small()
                            .color(palette.muted),
                    );
                    widgets::copy_button(
                        ui,
                        &peer.torii_url,
                        &format!("validator {} URL", index + 1),
                    );
                });
                if let Some(tips) = tips {
                    block_tip_row(ui, tips.get(index), sync.get(index).copied(), newest);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if !observed
                        && ui
                            .add_enabled(!busy, egui::Button::new("Observe").small())
                            .on_hover_text("Stream blocks and events from this validator")
                            .clicked()
                    {
                        observe = Some(index);
                    }
                    if ui.small_button("Logs").clicked() {
                        open = Some((View::Logs, Some(index)));
                    }
                    if ui.small_button("Endpoints").clicked() {
                        open = Some((View::Endpoints, Some(index)));
                    }
                });
            });
        });
        if let Some(index) = observe {
            self.observe_peer(index);
        }
        if let Some((view, peer)) = open {
            self.view = view;
            if let Some(peer) = peer {
                self.logs_source = Some(peer);
                self.endpoint_peer = peer;
            }
        }
        ui.add_space(12.0);
        self.chain_pulse(ui, 32);
        ui.add_space(12.0);
        let ready = phase == ManagedPhase::Ready;
        let account = context.account_id.clone();
        let mut opened = None;
        ui.columns(2, |columns| {
            widgets::card(&mut columns[0], |ui| {
                widgets::card_title(ui, icon::KEY, "Developer account");
                ui.horizontal(|ui| {
                    widgets::short_mono(ui, &account, 12);
                    widgets::copy_button(ui, &account, "developer account");
                });
                ui.add_space(4.0);
                ui.label(widgets::eyebrow(palette, "Balances"));
                match &self.ledger {
                    Some(ledger) => {
                        let holdings = ledger.holdings_of(&account);
                        if holdings.is_empty() {
                            widgets::hint(ui, "No balances yet.");
                        }
                        for holding in holdings.iter().take(4) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(holding.value).size(18.0).strong().color(palette.text));
                                ui.label(
                                    RichText::new(ledger.asset_label(holding.definition))
                                        .strong()
                                        .color(palette.sakura),
                                )
                                .on_hover_text(holding.asset);
                            });
                        }
                    }
                    None => {
                        widgets::hint(
                            ui,
                            if ready {
                                "Loading balances…"
                            } else {
                                "Balances appear once the validators are ready."
                            },
                        );
                    }
                }
            });
            widgets::card(&mut columns[1], |ui| {
                widgets::card_title(ui, icon::BLOCKS, "Latest blocks");
                if self.blocks_feed.is_empty() {
                    widgets::hint(
                        ui,
                        "No blocks streamed yet. Idle chains create no blocks; submit a transaction to make one.",
                    );
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
                opened = latest_blocks(ui, &self.blocks_feed, now);
            });
        });
        // A latest block opens in the Blocks view's drill-down.
        if let Some(height) = opened {
            self.view = View::Blocks;
            self.open_block(height);
        }
        ui.add_space(12.0);
        ui.label(
            RichText::new("Quick actions")
                .size(17.0)
                .strong()
                .color(palette.text),
        );
        ui.add_space(2.0);
        let actions = [
            (
                icon::MINT,
                "Mint tokens",
                View::Accounts,
                Some(QuickKind::Mint),
            ),
            (
                icon::ACCOUNTS,
                "New account",
                View::Accounts,
                Some(QuickKind::NewAccount),
            ),
            (
                icon::SEND,
                "Send tokens",
                View::Accounts,
                Some(QuickKind::Transfer),
            ),
            (icon::CONTRACTS, "Deploy contract", View::Contracts, None),
            (icon::STATE, "Query state", View::State, None),
            (icon::ENDPOINTS, "Endpoints", View::Endpoints, None),
        ];
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            for (glyph, label, view, quick) in actions {
                if ui
                    .add(
                        widgets::tonal_button(
                            &format!("{glyph}  {label}"),
                            palette.text,
                            palette.surface,
                        )
                        .stroke(egui::Stroke::new(1.0, palette.border))
                        .min_size(Vec2::new(150.0, 44.0)),
                    )
                    .clicked()
                {
                    chosen = Some((view, quick));
                }
            }
        });
        if let Some((view, quick)) = chosen {
            self.view = view;
            if let Some(kind) = quick {
                // Only a token's owner mints it: without one of their own, start by creating one.
                self.quick.kind = if kind == QuickKind::Mint && !self.owns_mintable_token() {
                    QuickKind::NewToken
                } else {
                    kind
                };
                self.preview = None;
            }
        }
        if let Ok(Some(status)) = &attachment {
            ui.add_space(12.0);
            widgets::card(ui, |ui| {
                widgets::card_title(ui, icon::PRIVATE, "Parent attachment");
                super::attachment_details(ui, status);
            });
        } else if let Err(error) = &attachment {
            ui.add_space(12.0);
            widgets::banner(
                ui,
                widgets::Tone::Warning,
                &format!("Parent status unavailable: {error}"),
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::prelude::{Hash, HashOf};

    /// A validator reporting block `block` at `height`.
    fn tip(height: u64, block: u8) -> TipObservation {
        Ok(Some(BlockTip {
            height,
            hash: HashOf::from_untyped_unchecked(Hash::prehashed([block; 32])),
        }))
    }

    #[test]
    fn validators_on_one_block_are_in_sync_and_a_spreading_commit_is_no_lag() {
        use TipSync::{Behind, Current};
        let agreed = [tip(10, 1), tip(10, 1), tip(10, 1), tip(10, 1)];
        assert_eq!(tip_sync(&agreed, 0), [Current; 4]);
        // One block short of the others, yet past what the network had at the previous read.
        let spreading = [tip(10, 1), tip(10, 1), tip(10, 1), tip(9, 9)];
        assert_eq!(tip_sync(&spreading, 9), [Current; 4]);
        assert_eq!(
            tip_sync(&spreading, 10),
            [Current, Current, Current, Behind(1)]
        );
    }

    #[test]
    fn lagging_validators_count_from_the_newest_height_seen() {
        use TipSync::{Behind, Current, Unanswered};
        let lagging = [tip(12, 1), tip(12, 1), Ok(None), tip(7, 3)];
        assert_eq!(
            tip_sync(&lagging, 10),
            [Current, Current, Behind(12), Behind(5)]
        );
        // A validator that stops answering still set the height the others must reach.
        let silent = [
            Err("connection refused".into()),
            tip(9, 2),
            tip(9, 2),
            tip(9, 2),
        ];
        assert_eq!(
            tip_sync(&silent, 10),
            [Unanswered, Behind(1), Behind(1), Behind(1)]
        );
    }

    #[test]
    fn a_different_block_at_one_height_is_flagged_unless_it_is_outvoted() {
        use TipSync::{Current, Diverged};
        let minority = [tip(10, 1), tip(10, 1), tip(10, 1), tip(10, 2)];
        assert_eq!(
            tip_sync(&minority, 10),
            [Current, Current, Current, Diverged]
        );
        let split = [tip(10, 1), tip(10, 1), tip(10, 2), tip(10, 2)];
        assert_eq!(tip_sync(&split, 10), [Diverged; 4]);
        // Blocks at different heights are never compared by hash.
        assert_eq!(tip_sync(&[tip(10, 1), tip(9, 2)], 9), [Current; 2]);
    }

    #[test]
    fn reads_keep_the_previous_newest_height_and_only_the_shown_generation() {
        let mut tips = BlockTips::default();
        tips.record(Ok(vec![tip(4, 1)]));
        tips.record(Ok(vec![tip(5, 1), Err("timeout".into())]));
        assert_eq!((tips.settled, tips.observed.len()), (4, 2));
        // A failed read compares nothing but keeps the height the network had reached.
        tips.record(Err("selected network was replaced".into()));
        assert!(tips.observed.is_empty());
        tips.record(Ok(vec![tip(5, 1)]));
        assert_eq!((tips.settled, tips.observed.len()), (5, 1));

        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        desktop.block_tips.pending = true;
        let elsewhere = PreparedLocalnet {
            service_profile: Default::default(),
            context: super::super::tests::observation_context(),
            peers: Vec::new(),
        };
        desktop.record_block_tips(&elsewhere, Ok(vec![tip(4, 1)]));
        assert!(!desktop.block_tips.pending, "the reply ends the read");
        assert!(
            desktop.block_tips.observed.is_empty(),
            "no generation is shown"
        );
    }

    #[test]
    fn blocks_are_read_only_for_a_ready_network_and_forgotten_with_it() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        desktop.poll_block_tips();
        assert!(!desktop.block_tips.pending, "nothing is selected");
        assert!(desktop.receiver.try_recv().is_err());
        desktop.block_tips = BlockTips {
            observed: vec![tip(4, 1)],
            settled: 3,
            pending: true,
            started: Some(Instant::now()),
        };
        desktop.clear_network();
        let tips = &desktop.block_tips;
        assert!(tips.observed.is_empty() && tips.settled == 0);
        assert!(!tips.pending && tips.started.is_none());
    }

    #[test]
    fn cards_show_each_block_and_flag_only_validators_out_of_sync() {
        let context = egui::Context::default();
        let rows = [
            (tip(10, 1), TipSync::Current),
            (tip(8, 2), TipSync::Behind(2)),
            (tip(9, 2), TipSync::Behind(1)),
            (tip(10, 2), TipSync::Diverged),
            (Err("connection refused".into()), TipSync::Unanswered),
            (Ok(None), TipSync::Current),
        ];
        let mut draw = |context: &mut egui::Ui| {
            egui::CentralPanel::default().show(context, |ui| {
                for (tip, sync) in &rows {
                    block_tip_row(ui, Some(tip), Some(*sync), 10);
                }
                block_tip_row(ui, None, None, 10);
                sync_summary(ui, &[TipSync::Current; 4]);
                sync_summary(ui, &[TipSync::Current, TipSync::Behind(2)]);
            });
        };
        let _ = super::super::tests::render_frame(&context, Vec::new(), &mut draw);
        let output = super::super::tests::render_frame(&context, Vec::new(), &mut draw);
        for text in [
            "#10",
            "01010101…01010101",
            "#8",
            "2 blocks behind",
            "1 block behind",
            "hash differs",
            "no response",
            "No blocks yet",
            "Reading latest block…",
            "in sync",
            "1 out of sync",
        ] {
            assert!(
                super::super::tests::text_position(&output, text).is_some(),
                "{text} is shown"
            );
        }
    }

    #[test]
    fn quickstart_picks_the_first_unused_localnet_name() {
        assert_eq!(quickstart_name(&[]), "local");
        assert_eq!(
            quickstart_name(&["local".into(), "local-2".into(), "demo".into()]),
            "local-3"
        );
    }

    #[test]
    fn quickstart_without_a_workspace_or_while_busy_starts_nothing() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        desktop.quickstart();
        assert!(!desktop.busy);
        assert!(desktop.receiver.try_recv().is_err());
        desktop.busy = true;
        desktop.quickstart();
        assert!(desktop.receiver.try_recv().is_err());
    }

    #[test]
    fn welcome_offers_quickstart_without_starting_anything() {
        let mut desktop = Desktop::model(std::path::PathBuf::from("unused"));
        let context = egui::Context::default();
        let mut draw = |context: &mut egui::Ui| {
            egui::CentralPanel::default().show(context, |ui| desktop.home(ui));
        };
        let _ = super::super::tests::render_frame(&context, Vec::new(), &mut draw);
        let output = super::super::tests::render_frame(&context, Vec::new(), &mut draw);
        let label = format!("{}  Quickstart", icon::QUICKSTART);
        assert!(super::super::tests::text_position(&output, &label).is_some());
        assert!(super::super::tests::text_position(&output, "Your Iroha playground").is_some());
        super::super::tests::click_label(&context, &label, draw);
        assert!(!desktop.busy, "no workspace: Quickstart is disabled");
    }

    #[test]
    fn a_latest_block_row_opens_its_block() {
        let row = |height: u64, transactions, rejected| BlockRow {
            height,
            hash: format!("{height:064x}"),
            transactions,
            rejected,
            signatures: 4,
            created_ms: 0,
            genesis: height == 1,
            bytes: 512,
            body: None,
        };
        let feed = VecDeque::from([row(2, 3, 1), row(1, 7, 0)]);
        let context = egui::Context::default();
        let mut opened = None;
        super::super::tests::click_label(&context, "3 tx · 1 rejected", |context| {
            egui::CentralPanel::default().show(context, |ui| {
                opened = opened.or(latest_blocks(ui, &feed, 0));
            });
        });
        assert_eq!(opened, Some(2), "the clicked row's block opens");
    }
}
