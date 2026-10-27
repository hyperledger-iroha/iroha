//! Block drill-down: one committed block's header, transactions, results and canonical payloads.
//!
//! A block opened from the feed renders from the body the stream already decoded. A body the
//! feed released, or a height outside it, is read again from the observed validator.

use super::{
    Desktop, Message, UiResult,
    ledger::{self, BlockRow},
    theme::Palette,
    widgets::{self, Tone, icon},
};
use eframe::egui::{
    self, Align, CornerRadius, Layout, Margin, RichText, Sense, Ui,
    collapsing_header::CollapsingState,
};
use iroha_data_model::block::SignedBlock;
use mochi_core::{
    block_detail::{self, BlockDetail, EntrypointKind, InstructionRow, Outcome, TransactionRow},
    developer::ManagedNetwork,
};
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    hash::Hash,
    num::NonZeroU64,
    sync::Arc,
};

/// Characters of a payload shown inline; copying always takes the complete payload.
const SHOWN_PAYLOAD_CHARS: usize = 64 * 1024;
/// Characters of an instruction's one-line form; its structure shows everything.
const SUMMARY_CHARS: usize = 320;

/// The block opened from the Blocks view.
pub(crate) struct BlockView {
    pub(crate) height: u64,
    /// Hash of the feed row it was opened from; a block read again must match it.
    expected_hash: Option<String>,
    pub(crate) body: BlockBody,
    /// Narrows the transaction list.
    filter: String,
}

/// Where the open block's contents come from.
pub(crate) enum BlockBody {
    /// The body must be read from the validator; `requested` once the read is dispatched.
    Pending {
        requested: bool,
    },
    Ready(Box<OpenedBlock>),
    Failed(String),
}

/// A decoded block with its projection and the payloads derived from it so far.
pub(crate) struct OpenedBlock {
    block: Arc<SignedBlock>,
    detail: BlockDetail,
    payloads: Payloads,
}

impl OpenedBlock {
    fn new(block: Arc<SignedBlock>, bytes: usize) -> Self {
        let detail = BlockDetail::from_block(&block, bytes);
        Self {
            block,
            detail,
            payloads: Payloads::default(),
        }
    }
}

/// A canonical payload of the open block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Payload {
    Block,
    Wire,
    Input(usize),
    Output(usize),
}

/// Payloads computed when first shown or copied; large blocks cost nothing until then.
#[derive(Default)]
struct Payloads(HashMap<Payload, Option<String>>);

impl Payloads {
    fn get(&mut self, block: &SignedBlock, payload: Payload) -> Option<&str> {
        self.0
            .entry(payload)
            .or_insert_with(|| match payload {
                Payload::Block => block_detail::block_json(block),
                Payload::Wire => block_detail::block_wire_hex(block),
                Payload::Input(index) => block_detail::input_json(block, index),
                Payload::Output(index) => block_detail::output_json(block, index),
            })
            .as_deref()
    }
}

impl BlockView {
    /// Open `height`; a row whose body the feed still holds needs no read.
    pub(crate) fn open(feed: &VecDeque<BlockRow>, height: u64) -> Self {
        let row = feed.iter().find(|row| row.height == height);
        let body = match row.and_then(|row| row.body.clone().map(|body| (body, row.bytes))) {
            Some((block, bytes)) => BlockBody::Ready(Box::new(OpenedBlock::new(block, bytes))),
            None => BlockBody::Pending { requested: false },
        };
        Self {
            height,
            expected_hash: row.map(|row| row.hash.clone()),
            body,
            filter: String::new(),
        }
    }

    /// Whether the body still has to be read from the validator.
    pub(crate) fn wants_body(&self) -> bool {
        matches!(self.body, BlockBody::Pending { requested: false })
    }

    /// Install the outcome of the read dispatched for this view.
    pub(crate) fn receive(&mut self, result: UiResult<(Arc<SignedBlock>, usize)>) {
        self.body = match result {
            Ok((block, bytes)) => {
                let opened = OpenedBlock::new(block, bytes);
                match &self.expected_hash {
                    Some(expected) if *expected != opened.detail.hash => {
                        BlockBody::Failed(format!(
                            "The validator returned block {} for #{}, which streamed as {expected}.",
                            opened.detail.hash, self.height
                        ))
                    }
                    _ => BlockBody::Ready(Box::new(opened)),
                }
            }
            Err(error) => BlockBody::Failed(error),
        };
    }
}

/// Navigation requested while drawing the block page.
enum Navigate {
    List,
    Height(u64),
}

fn read_block(
    handle: &tokio::runtime::Handle,
    network: &ManagedNetwork,
    peer: usize,
    height: NonZeroU64,
) -> UiResult<(Arc<SignedBlock>, usize)> {
    let reader = network.ledger_reader(peer).map_err(|e| e.to_string())?;
    let fetched = handle
        .block_on(mochi_core::fetch_block(&reader, height))
        .map_err(|e| e.to_string())?;
    // A generation replaced during the read must not present another chain's block.
    network.validate().map_err(|e| e.to_string())?;
    Ok((Arc::new(fetched.block), fetched.bytes))
}

impl Desktop {
    /// Show `height` in the Blocks view.
    pub(super) fn open_block(&mut self, height: u64) {
        self.block_view = Some(BlockView::open(&self.blocks_feed, height));
    }

    /// Read the open block from the observed validator when no body is at hand.
    pub(super) fn fetch_open_block(&mut self) {
        let (Some(selected), Some(handle)) = (
            self.selected.as_ref(),
            self.runtime
                .as_ref()
                .map(|runtime| runtime.handle().clone()),
        ) else {
            return;
        };
        let Some(view) = self.block_view.as_mut().filter(|view| view.wants_body()) else {
            return;
        };
        let Some(height) = NonZeroU64::new(view.height) else {
            view.body = BlockBody::Failed("Block heights start at 1.".into());
            return;
        };
        view.body = BlockBody::Pending { requested: true };
        let network = selected.network.clone();
        let peer = self.peer;
        // Background observation: keep the foreground notice, such as a just-submitted hash.
        self.spawn_task(move || Message::Block {
            height: height.get(),
            result: read_block(&handle, &network, peer, height),
        });
    }

    /// The open block's page: overview, transactions, finality, commitments and payloads.
    pub(super) fn block_page(&mut self, ui: &mut Ui) {
        let palette = widgets::palette(ui);
        let ready = self.ready();
        let latest = self.latest_height;
        let observer = self.peer + 1;
        let now = ledger::now_ms();
        let Some(view) = self.block_view.as_mut() else {
            return;
        };
        let height = view.height;
        let mut navigate = None;
        if ui
            .add(egui::Button::new(RichText::new("‹ All blocks").color(palette.muted)).frame(false))
            .clicked()
        {
            navigate = Some(Navigate::List);
        }
        let subtitle = match &view.body {
            BlockBody::Ready(opened) => format!(
                "Committed {} · {} · from validator {observer}",
                widgets::time_ago(opened.detail.created_ms, now),
                block_detail::format_utc_ms(opened.detail.created_ms)
            ),
            _ => format!("From validator {observer}"),
        };
        widgets::page_header(ui, &format!("Block #{height}"), &subtitle, |ui| {
            // Right to left: the newer block sits rightmost.
            if ui
                .add_enabled(
                    latest.is_some_and(|latest| height < latest),
                    egui::Button::new("Next ›"),
                )
                .on_hover_text(format!("Open block #{}", height + 1))
                .clicked()
            {
                navigate = Some(Navigate::Height(height + 1));
            }
            if ui
                .add_enabled(height > 1, egui::Button::new("‹ Previous"))
                .on_hover_text(format!("Open block #{}", height.saturating_sub(1)))
                .clicked()
            {
                navigate = Some(Navigate::Height(height - 1));
            }
        });
        let mut retry = false;
        match &mut view.body {
            BlockBody::Pending { .. } => {
                widgets::card(ui, |ui| {
                    if ready {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(format!(
                                    "Reading block #{height} from validator {observer}…"
                                ))
                                .color(palette.muted),
                            );
                        });
                    } else {
                        widgets::empty_state(
                            ui,
                            icon::BLOCKS,
                            "Block not loaded",
                            "Start the environment to read this block from its validator.",
                        );
                    }
                });
            }
            BlockBody::Failed(error) => {
                widgets::banner(ui, Tone::Error, error, false);
                ui.add_space(6.0);
                retry = ui
                    .add_enabled(ready, widgets::primary_button(palette, "Try again"))
                    .clicked();
            }
            BlockBody::Ready(opened) => {
                sections(ui, palette, opened, &mut view.filter, now, &mut navigate);
            }
        }
        if retry {
            view.body = BlockBody::Pending { requested: false };
        }
        match navigate {
            Some(Navigate::List) => self.block_view = None,
            Some(Navigate::Height(height)) => self.open_block(height),
            None => {}
        }
    }
}

fn sections(
    ui: &mut Ui,
    palette: &Palette,
    opened: &mut OpenedBlock,
    filter: &mut String,
    now: u64,
    navigate: &mut Option<Navigate>,
) {
    let OpenedBlock {
        block,
        detail,
        payloads,
    } = opened;
    overview(ui, palette, detail, now, navigate);
    ui.add_space(12.0);
    transactions(ui, palette, block, detail, payloads, filter, now);
    if !detail.invocations.is_empty() {
        ui.add_space(12.0);
        invocations(ui, palette, block, detail, payloads);
    }
    ui.add_space(12.0);
    finality(ui, palette, detail);
    ui.add_space(12.0);
    commitments(ui, palette, detail);
    ui.add_space(12.0);
    canonical_payloads(ui, palette, block, detail, payloads);
}

fn overview(
    ui: &mut Ui,
    palette: &Palette,
    detail: &BlockDetail,
    now: u64,
    navigate: &mut Option<Navigate>,
) {
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::card_title(ui, icon::BLOCKS, "Overview");
            if detail.genesis {
                widgets::pill(ui, "genesis", palette.fuji, palette.fuji_soft);
            }
        });
        ui.add_space(4.0);
        let rejected = detail.rejected();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 28.0;
            let figure = |ui: &mut Ui, label: &str, value: String, color: egui::Color32| {
                widgets::stat(ui, label, |ui| {
                    ui.label(RichText::new(value).size(18.0).strong().color(color));
                });
            };
            figure(
                ui,
                "Transactions",
                detail.transactions.len().to_string(),
                palette.sky,
            );
            figure(
                ui,
                "Rejected",
                rejected.to_string(),
                if rejected > 0 {
                    palette.aka
                } else {
                    palette.text
                },
            );
            figure(
                ui,
                "Trigger runs",
                detail.invocations.len().to_string(),
                palette.text,
            );
            figure(
                ui,
                "Signatures",
                detail.signatures.len().to_string(),
                palette.text,
            );
            figure(ui, "Size", format!("{} B", detail.bytes), palette.text);
            figure(
                ui,
                "View change",
                detail.view_change_index.to_string(),
                palette.text,
            );
        });
        ui.add_space(8.0);
        egui::Grid::new("block-overview")
            .num_columns(2)
            .spacing([18.0, 8.0])
            .show(ui, |ui| {
                ui.label(widgets::eyebrow(palette, "Hash"));
                mono_value(ui, palette, &detail.hash, "block hash");
                ui.end_row();
                ui.label(widgets::eyebrow(palette, "Previous block"));
                match &detail.previous_hash {
                    Some(previous) => {
                        ui.horizontal(|ui| {
                            let previous_height = detail.height.saturating_sub(1);
                            if ui
                                .add(egui::Link::new(
                                    RichText::new(format!("#{previous_height}"))
                                        .strong()
                                        .color(palette.sky),
                                ))
                                .on_hover_text(format!("Open block #{previous_height}"))
                                .clicked()
                            {
                                *navigate = Some(Navigate::Height(previous_height));
                            }
                            mono_value(ui, palette, previous, "previous block hash");
                        });
                    }
                    None => {
                        ui.label(
                            RichText::new("None; genesis starts the chain").color(palette.muted),
                        );
                    }
                }
                ui.end_row();
                ui.label(widgets::eyebrow(palette, "Created"));
                ui.label(format!(
                    "{} · {}",
                    block_detail::format_utc_ms(detail.created_ms),
                    widgets::time_ago(detail.created_ms, now)
                ));
                ui.end_row();
                ui.label(widgets::eyebrow(palette, "Results"));
                ui.label(results_text(detail));
                ui.end_row();
            });
    });
}

fn results_text(detail: &BlockDetail) -> String {
    if !detail.has_results {
        return "Not executed: the block carries no results".to_owned();
    }
    let outputs = detail
        .transactions
        .iter()
        .filter(|row| row.output_index.is_some())
        .count()
        + detail.invocations.len();
    let fragments = detail
        .committed_fragments
        .map_or_else(String::new, |count| {
            let count = usize::try_from(count).unwrap_or(usize::MAX);
            format!(" · {} committed", plural(count, "execution fragment"))
        });
    format!("{}{fragments}", plural(outputs, "execution output"))
}

fn transactions(
    ui: &mut Ui,
    palette: &Palette,
    block: &SignedBlock,
    detail: &BlockDetail,
    payloads: &mut Payloads,
    filter: &mut String,
    now: u64,
) {
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::card_title(ui, icon::COMPOSE, "Transactions");
            widgets::pill(
                ui,
                detail.transactions.len().to_string(),
                palette.sky,
                palette.sky_soft,
            );
            let rejected = detail.rejected();
            if rejected > 0 {
                widgets::pill(
                    ui,
                    format!("{rejected} rejected"),
                    palette.aka,
                    palette.aka_soft,
                );
            }
            if detail.transactions.len() > 1 {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(filter)
                            .hint_text("Filter by hash, authority or instruction")
                            .desired_width(280.0),
                    );
                });
            }
        });
        if detail.transactions.is_empty() {
            widgets::hint(ui, "This block carries no transactions.");
            return;
        }
        let needle = filter.trim().to_lowercase();
        let only = detail.transactions.len() == 1;
        let mut shown = 0;
        for row in detail
            .transactions
            .iter()
            .filter(|row| row.matches(&needle))
        {
            ui.add_space(6.0);
            transaction(ui, palette, block, &detail.hash, row, payloads, now, only);
            shown += 1;
        }
        if shown == 0 {
            widgets::hint(ui, "No transaction in this block matches the filter.");
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn transaction(
    ui: &mut Ui,
    palette: &Palette,
    block: &SignedBlock,
    block_hash: &str,
    row: &TransactionRow,
    payloads: &mut Payloads,
    now: u64,
    open: bool,
) {
    let id = ui.make_persistent_id(("block-transaction", block_hash, row.index));
    egui::Frame::new()
        .fill(palette.inset)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let (_, header, _) = CollapsingState::load_with_default_open(ui.ctx(), id, open)
                .show_header(ui, |ui| {
                    let index = ui.add(
                        egui::Label::new(
                            RichText::new(format!("#{}", row.index))
                                .monospace()
                                .color(palette.muted),
                        )
                        .selectable(false)
                        .sense(Sense::click()),
                    );
                    outcome_pill(ui, palette, &row.outcome);
                    if row.kind != EntrypointKind::Transaction {
                        widgets::pill(ui, row.kind.label(), palette.fuji, palette.fuji_soft);
                    }
                    widgets::short_mono(ui, &row.hash, 8);
                    widgets::copy_button(ui, &row.hash, "transaction hash");
                    if let Some(authority) = &row.authority {
                        ui.label(RichText::new("by").color(palette.faint));
                        widgets::short_mono(ui, authority, 6);
                    }
                    let summary = ui.add(
                        egui::Label::new(RichText::new(&row.executable).color(palette.muted))
                            .selectable(false)
                            .sense(Sense::click()),
                    );
                    index.clicked() || summary.clicked()
                })
                .body(|ui| transaction_body(ui, palette, block, row, payloads, now));
            // The index and summary toggle the row like its arrow does.
            if header.inner
                && let Some(mut state) = CollapsingState::load(ui.ctx(), id)
            {
                state.toggle(ui);
                state.store(ui.ctx());
            }
        });
}

fn transaction_body(
    ui: &mut Ui,
    palette: &Palette,
    block: &SignedBlock,
    row: &TransactionRow,
    payloads: &mut Payloads,
    now: u64,
) {
    ui.add_space(4.0);
    match &row.outcome {
        Outcome::Rejected { reason, detail } => {
            widgets::banner(ui, Tone::Error, &format!("Rejected: {reason}"), false);
            egui::CollapsingHeader::new(RichText::new("Rejection details").color(palette.muted))
                .id_salt(("rejection", row.index))
                .show(ui, |ui| {
                    widgets::code_block(ui, detail, "rejection details")
                });
        }
        Outcome::Applied {
            trigger_steps,
            transfer_receipts,
            fee_receipt,
        } => {
            let mut parts = vec![plural(*trigger_steps, "data-trigger step")];
            if *transfer_receipts > 0 {
                parts.push(plural(*transfer_receipts, "transfer receipt"));
            }
            if *fee_receipt {
                parts.push("fee charged".to_owned());
            }
            widgets::hint(ui, &format!("Applied · {}", parts.join(" · ")));
        }
        Outcome::Missing => {
            widgets::hint(ui, "The block carries no result for this input.");
        }
    }
    ui.add_space(4.0);
    egui::Grid::new(("transaction-fields", row.index))
        .num_columns(2)
        .spacing([18.0, 6.0])
        .show(ui, |ui| {
            let field = |ui: &mut Ui, label: &str, add: &mut dyn FnMut(&mut Ui)| {
                ui.label(widgets::eyebrow(palette, label));
                add(ui);
                ui.end_row();
            };
            field(ui, "Hash", &mut |ui| {
                mono_value(ui, palette, &row.hash, "transaction hash");
            });
            if let Some(inner) = &row.inner_hash {
                field(ui, "Inner transaction", &mut |ui| {
                    mono_value(ui, palette, inner, "inner transaction hash");
                });
            }
            field(ui, "Kind", &mut |ui| {
                ui.label(row.kind.label());
            });
            if let Some(authority) = &row.authority {
                field(ui, "Authority", &mut |ui| {
                    mono_value(ui, palette, authority, "authority");
                });
            }
            if let Some(created) = row.created_ms {
                field(ui, "Created", &mut |ui| {
                    ui.label(format!(
                        "{} · {}",
                        block_detail::format_utc_ms(created),
                        widgets::time_ago(created, now)
                    ));
                });
            }
            if let Some(ttl) = row.time_to_live_ms {
                field(ui, "Lifetime", &mut |ui| {
                    ui.label(lifetime(ttl));
                });
            }
            if let Some(nonce) = row.nonce {
                field(ui, "Nonce", &mut |ui| {
                    ui.label(RichText::new(nonce.to_string()).monospace());
                });
            }
            if let Some(domain) = &row.domain {
                field(ui, "Network", &mut |ui| {
                    mono_value(ui, palette, domain, "network");
                });
            }
            if !row.executable.is_empty() {
                field(ui, "Executable", &mut |ui| {
                    ui.label(&row.executable);
                });
            }
            if let Some(fee) = &row.fee_payment {
                field(ui, "Fee payment", &mut |ui| {
                    mono_value(ui, palette, fee, "fee payment");
                });
            }
            if let Some(signature) = &row.signature {
                field(ui, "Signature", &mut |ui| {
                    ui.horizontal(|ui| {
                        widgets::short_mono(ui, signature, 12);
                        widgets::copy_button(ui, signature, "signature");
                    });
                });
            }
            if row.multisig_signatures > 0 {
                field(ui, "Multisig signatures", &mut |ui| {
                    ui.label(row.multisig_signatures.to_string());
                });
            }
        });
    if !row.instructions.is_empty() {
        ui.add_space(8.0);
        ui.label(widgets::eyebrow(
            palette,
            &format!("Instructions · {}", row.instructions.len()),
        ));
        for (position, instruction) in row.instructions.iter().enumerate() {
            instruction_line(ui, palette, row.index, position, instruction);
        }
    }
    if !row.metadata.is_empty() {
        ui.add_space(8.0);
        ui.label(widgets::eyebrow(palette, "Metadata"));
        egui::Grid::new(("transaction-metadata", row.index))
            .num_columns(2)
            .spacing([18.0, 4.0])
            .show(ui, |ui| {
                for (key, value) in &row.metadata {
                    ui.label(RichText::new(key).monospace().color(palette.muted));
                    ui.add(egui::Label::new(RichText::new(value).monospace()).wrap());
                    ui.end_row();
                }
            });
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        copy_payload(
            ui,
            payloads,
            block,
            Payload::Input(row.index),
            "Copy input JSON",
        );
        if let Some(output) = row.output_index {
            copy_payload(
                ui,
                payloads,
                block,
                Payload::Output(output),
                "Copy result JSON",
            );
        }
    });
    payload_viewer(
        ui,
        palette,
        ("input-json", row.index),
        "Input JSON",
        payloads,
        block,
        Payload::Input(row.index),
    );
    if let Some(output) = row.output_index {
        payload_viewer(
            ui,
            palette,
            ("result-json", row.index),
            "Result JSON",
            payloads,
            block,
            Payload::Output(output),
        );
    }
}

fn instruction_line(
    ui: &mut Ui,
    palette: &Palette,
    transaction: usize,
    position: usize,
    instruction: &InstructionRow,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("{}.", position + 1))
                .monospace()
                .color(palette.faint),
        );
        ui.label(
            RichText::new(&instruction.name)
                .monospace()
                .strong()
                .color(palette.fuji),
        );
        if let Some(summary) = &instruction.summary {
            ui.add(
                egui::Label::new(RichText::new(clip(summary, SUMMARY_CHARS)).color(palette.text))
                    .wrap(),
            );
        }
    });
    if let Some(structure) = &instruction.detail {
        egui::CollapsingHeader::new(RichText::new("Structure").small().color(palette.muted))
            .id_salt(("instruction", transaction, position))
            .show(ui, |ui| widgets::code_block(ui, structure, "instruction"));
    }
}

fn invocations(
    ui: &mut Ui,
    palette: &Palette,
    block: &SignedBlock,
    detail: &BlockDetail,
    payloads: &mut Payloads,
) {
    widgets::card(ui, |ui| {
        widgets::card_title(ui, icon::ACTIVITY, "Trigger runs");
        widgets::hint(
            ui,
            "Scheduled and pipeline triggers the block executed, with their recorded results.",
        );
        for row in &detail.invocations {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                outcome_pill(ui, palette, &row.outcome);
                widgets::pill(ui, row.kind, palette.kinako, palette.kinako_soft);
                ui.label(RichText::new(&row.trigger).monospace());
                copy_payload(
                    ui,
                    payloads,
                    block,
                    Payload::Output(row.output_index),
                    "Copy result JSON",
                );
            });
            if let Outcome::Rejected { reason, .. } = &row.outcome {
                ui.add(egui::Label::new(RichText::new(reason).color(palette.aka)).wrap());
            }
        }
    });
}

fn finality(ui: &mut Ui, palette: &Palette, detail: &BlockDetail) {
    widgets::card(ui, |ui| {
        widgets::card_title(ui, icon::KEY, "Signatures and finality");
        if detail.signatures.is_empty() {
            widgets::hint(
                ui,
                if detail.commit_certificate.is_some() {
                    "No header signatures are attached; the commit certificate carries the committing validators' CommitQC."
                } else {
                    "No header signatures or commit certificate are attached."
                },
            );
        } else {
            egui::Grid::new("block-signatures")
                .num_columns(2)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    for signature in &detail.signatures {
                        ui.label(widgets::eyebrow(
                            palette,
                            &format!("Validator {}", signature.validator),
                        ));
                        ui.horizontal(|ui| {
                            widgets::short_mono(ui, &signature.signature, 12);
                            widgets::copy_button(ui, &signature.signature, "signature");
                        });
                        ui.end_row();
                    }
                });
        }
        if let Some(parts) = &detail.commit_certificate {
            ui.add_space(8.0);
            ui.label(widgets::eyebrow(palette, "Commit certificate"));
            egui::Grid::new("block-certificate")
                .num_columns(2)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    for part in parts {
                        ui.label(RichText::new(part.label).color(palette.muted));
                        match &part.value {
                            Some(hex) => {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} bytes", hex.len() / 2));
                                    widgets::copy_button(ui, hex, part.label);
                                });
                            }
                            None => {
                                ui.label(RichText::new("empty").color(palette.faint));
                            }
                        }
                        ui.end_row();
                    }
                });
        }
    });
}

fn commitments(ui: &mut Ui, palette: &Palette, detail: &BlockDetail) {
    widgets::card(ui, |ui| {
        widgets::card_title(ui, icon::STATE, "Header commitments");
        widgets::hint(
            ui,
            "Hashes the header commits to; a dash means the block carries no such section.",
        );
        ui.add_space(4.0);
        egui::Grid::new("block-commitments")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                for field in &detail.commitments {
                    ui.label(widgets::eyebrow(palette, field.label));
                    match &field.value {
                        Some(value) => mono_value(ui, palette, value, field.label),
                        None => {
                            ui.label(RichText::new("—").color(palette.faint));
                        }
                    }
                    ui.end_row();
                }
            });
    });
}

fn canonical_payloads(
    ui: &mut Ui,
    palette: &Palette,
    block: &SignedBlock,
    detail: &BlockDetail,
    payloads: &mut Payloads,
) {
    widgets::card(ui, |ui| {
        widgets::card_title(ui, icon::LOGS, "Canonical payloads");
        widgets::hint(
            ui,
            "The complete block as Norito JSON and as its canonical SignedBlockWire frame.",
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            copy_payload(ui, payloads, block, Payload::Block, "Copy block JSON");
            copy_payload(
                ui,
                payloads,
                block,
                Payload::Wire,
                "Copy SignedBlockWire hex",
            );
        });
        payload_viewer(
            ui,
            palette,
            ("block-json", detail.height),
            "Block JSON",
            payloads,
            block,
            Payload::Block,
        );
    });
}

fn outcome_pill(ui: &mut Ui, palette: &Palette, outcome: &Outcome) {
    match outcome {
        Outcome::Applied { .. } => {
            widgets::pill(ui, "Applied", palette.matcha, palette.matcha_soft);
        }
        Outcome::Rejected { .. } => {
            widgets::pill(ui, "Rejected", palette.aka, palette.aka_soft);
        }
        Outcome::Missing => {
            widgets::pill(ui, "No result", palette.muted, palette.surface);
        }
    }
}

/// Wrapping monospace value with its copy control.
fn mono_value(ui: &mut Ui, palette: &Palette, value: &str, what: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new(value).monospace().color(palette.text)).wrap());
        widgets::copy_button(ui, value, what);
    });
}

fn copy_payload(
    ui: &mut Ui,
    payloads: &mut Payloads,
    block: &SignedBlock,
    payload: Payload,
    label: &str,
) {
    if ui.small_button(label).clicked() {
        let what = label.trim_start_matches("Copy ");
        match payloads.get(block, payload) {
            Some(text) => widgets::copy_to_clipboard(ui.ctx(), text, what),
            None => widgets::notify(ui.ctx(), format!("No {what} for this payload")),
        }
    }
}

/// Collapsed viewer that derives its payload only when opened.
fn payload_viewer(
    ui: &mut Ui,
    palette: &Palette,
    salt: impl Hash + Copy + std::fmt::Debug,
    title: &str,
    payloads: &mut Payloads,
    block: &SignedBlock,
    payload: Payload,
) {
    egui::CollapsingHeader::new(RichText::new(title).color(palette.muted))
        .id_salt(salt)
        .show(ui, |ui| {
            let Some(text) = payloads.get(block, payload) else {
                widgets::hint(ui, "This payload has no JSON form.");
                return;
            };
            let shown = clip(text, SHOWN_PAYLOAD_CHARS);
            if let Cow::Owned(_) = shown {
                widgets::hint(
                    ui,
                    &format!(
                        "Showing the first {} KiB of {} KiB; copying takes all of it.",
                        SHOWN_PAYLOAD_CHARS / 1024,
                        text.len() / 1024
                    ),
                );
            }
            egui::Frame::new()
                .fill(palette.inset)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .id_salt((salt, "scroll"))
                        .max_height(360.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(shown).monospace().size(12.0))
                                    .wrap(),
                            );
                        });
                });
        });
}

/// The first `limit` characters of `text`, ending with an ellipsis when cut.
fn clip(text: &str, limit: usize) -> Cow<'_, str> {
    match text.char_indices().nth(limit) {
        Some((end, _)) => format!("{}…", &text[..end]).into(),
        None => text.into(),
    }
}

/// `1 output` or `2 outputs`.
fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// `300 s` or `1500 ms` for a signed lifetime.
fn lifetime(ms: u64) -> String {
    if ms >= 1_000 && ms.is_multiple_of(1_000) {
        format!("{} s", ms / 1_000)
    } else {
        format!("{ms} ms")
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{click_label, run_headless_frame, text_position};
    use super::*;
    use iroha_data_model::transaction::{
        DataTriggerSequence,
        error::{TransactionLimitError, TransactionRejectionReason},
    };
    use mochi_core::{BlockSummary, block_detail::sample_block};
    use std::path::PathBuf;

    fn applied() -> Arc<SignedBlock> {
        Arc::new(sample_block(Ok(DataTriggerSequence::default()), Vec::new()))
    }

    /// A desktop whose feed streamed `block` together with its body.
    fn streamed(block: &Arc<SignedBlock>) -> Desktop {
        let mut desktop = Desktop::model(PathBuf::from("unused"));
        let detail = BlockDetail::from_block(block, 512);
        let summary = BlockSummary {
            height: detail.height,
            hash_hex: detail.hash.clone(),
            transaction_count: detail.transactions.len(),
            rejected_transaction_count: detail.rejected(),
            time_trigger_count: detail.invocations.len(),
            signature_count: detail.signatures.len(),
            view_change_index: detail.view_change_index,
            creation_time_ms: detail.created_ms,
            is_genesis: detail.genesis,
        };
        ledger::record_block(&mut desktop.blocks_feed, &summary, Some(block.clone()), 512);
        desktop.latest_height = Some(detail.height);
        desktop
    }

    /// The Blocks view on a window tall enough to paint the whole block page.
    fn tall_frame(context: &egui::Context, desktop: &mut Desktop) -> egui::FullOutput {
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 4000.0),
            )),
            ..Default::default()
        };
        let mut draw = |context: &mut Ui| {
            egui::CentralPanel::default().show(context, |ui| desktop.blocks(ui));
        };
        // The first frame measures; the second paints the settled layout.
        let _ = run_headless_frame(context, input(), &mut draw);
        run_headless_frame(context, input(), &mut draw)
    }

    #[test]
    fn a_streamed_block_opens_from_its_row_without_a_read() {
        let block = applied();
        let mut desktop = streamed(&block);
        let context = egui::Context::default();
        click_label(&context, "Details ›", |context| {
            egui::CentralPanel::default().show(context, |ui| desktop.blocks(ui));
        });
        let view = desktop
            .block_view
            .as_ref()
            .expect("the row opened its block");
        assert_eq!(view.height, 1);
        assert!(!view.wants_body(), "the feed's body needs no read");
        assert!(!desktop.busy);
        let output = tall_frame(&context, &mut desktop);
        for text in [
            "Block #1",
            "Applied",
            "iroha.log",
            "LOG(INFO): hello block",
            "Signatures and finality",
            "Header commitments",
            "Canonical payloads",
        ] {
            assert!(text_position(&output, text).is_some(), "missing {text:?}");
        }
        assert!(
            text_position(&output, &BlockDetail::from_block(&block, 0).hash).is_some(),
            "the full block hash is shown"
        );

        click_label(&context, "‹ All blocks", |context| {
            egui::CentralPanel::default().show(context, |ui| desktop.blocks(ui));
        });
        assert!(desktop.block_view.is_none(), "back returns to the list");
    }

    #[test]
    fn a_rejected_transaction_shows_its_reason() {
        let rejection = TransactionRejectionReason::LimitCheck(TransactionLimitError {
            reason: "too many instructions".to_owned(),
        });
        let mut desktop = streamed(&Arc::new(sample_block(Err(rejection), Vec::new())));
        desktop.open_block(1);
        let output = tall_frame(&egui::Context::default(), &mut desktop);
        assert!(text_position(&output, "Rejected").is_some());
        assert!(
            text_position(
                &output,
                "Rejected: Failed to validate transaction limits: too many instructions"
            )
            .is_some()
        );
    }

    #[test]
    fn a_block_outside_the_feed_waits_for_its_read() {
        let mut desktop = Desktop::model(PathBuf::from("unused"));
        desktop.open_block(1);
        assert!(desktop.block_view.as_ref().unwrap().wants_body());
        // Without a ready environment nothing is read.
        desktop.poll();
        assert!(!desktop.busy);
        assert!(desktop.block_view.as_ref().unwrap().wants_body());
        let pending = tall_frame(&egui::Context::default(), &mut desktop);
        assert!(text_position(&pending, "Block not loaded").is_some());

        let block = applied();
        let epoch = desktop.epoch;
        let completed = |height, bytes| Message::Block {
            height,
            result: Ok((block.clone(), bytes)),
        };
        desktop.sender.send((epoch, completed(2, 7))).unwrap();
        desktop.poll();
        assert!(
            desktop.block_view.as_ref().unwrap().wants_body(),
            "a read for a block since left is dropped"
        );
        desktop.sender.send((epoch, completed(1, 9))).unwrap();
        desktop.poll();
        assert!(matches!(
            &desktop.block_view.as_ref().unwrap().body,
            BlockBody::Ready(opened) if opened.detail.bytes == 9
        ));
    }

    #[test]
    fn a_read_must_return_the_streamed_block() {
        let mut view = BlockView {
            height: 1,
            expected_hash: Some("feedface".to_owned()),
            body: BlockBody::Pending { requested: true },
            filter: String::new(),
        };
        view.receive(Ok((applied(), 9)));
        assert!(matches!(
            &view.body,
            BlockBody::Failed(message) if message.ends_with("streamed as feedface.")
        ));
        view.receive(Err("validator unavailable".to_owned()));
        assert!(matches!(
            &view.body,
            BlockBody::Failed(message) if message == "validator unavailable"
        ));
    }

    #[test]
    fn navigation_glyphs_exist_in_the_bundled_fonts() {
        let context = egui::Context::default();
        let _ = run_headless_frame(&context, egui::RawInput::default(), |_| {});
        assert!(
            context.fonts_mut(|fonts| fonts.has_glyphs(&egui::FontId::proportional(14.0), "‹›…·—"))
        );
    }

    #[test]
    fn payloads_and_lifetimes_render_compactly() {
        assert_eq!(clip("abcdef", 3), "abc…");
        assert_eq!(clip("ｱｲｳ", 3), "ｱｲｳ");
        assert_eq!(lifetime(300_000), "300 s");
        assert_eq!(lifetime(1_500), "1500 ms");
        let block = applied();
        let mut payloads = Payloads::default();
        let json = payloads
            .get(&block, Payload::Block)
            .expect("block JSON")
            .to_owned();
        assert_eq!(payloads.get(&block, Payload::Block), Some(json.as_str()));
        assert_eq!(payloads.get(&block, Payload::Input(3)), None);
    }
}
