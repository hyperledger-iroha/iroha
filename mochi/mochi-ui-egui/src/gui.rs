//! Desktop views over Kagami's shared, persistent managed workspace.

use eframe::egui;
use mochi_core::{
    DashboardAccountInput, DashboardSnapshot, InstructionDraft, ManagedBlockStream,
    ManagedEventStream, StatePage, StateQueryKind, TransactionPreview,
    developer::{ContractInput, DeveloperWorkspace, ManagedNetwork, ManagedPhase},
    drafts_from_json_str, drafts_to_pretty_json, fetch_dashboard_snapshot, run_state_query,
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

/// Launch the desktop; the optional argument is a workspace directory, never a configuration file.
pub fn run(workspace: PathBuf) -> eframe::Result<()> {
    eframe::run_native(
        "Mochi",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1160.0, 800.0]),
            ..Default::default()
        },
        Box::new(move |creation| Ok(Box::new(Desktop::new(creation, workspace)))),
    )
}

type UiResult<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Dashboard,
    State,
    Activity,
    Composer,
    Contracts,
}
impl View {
    const ALL: [Self; 5] = [
        Self::Dashboard,
        Self::State,
        Self::Activity,
        Self::Composer,
        Self::Contracts,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Overview",
            Self::State => "State",
            Self::Activity => "Activity",
            Self::Composer => "Compose",
            Self::Contracts => "Contracts",
        }
    }
}

struct Selection {
    network: ManagedNetwork,
    phase: ManagedPhase,
    running: usize,
    failure: Option<String>,
}

fn load_selection(workspace: &DeveloperWorkspace, name: Option<&str>) -> UiResult<Selection> {
    let network = workspace.network(name).map_err(|e| e.to_string())?;
    let status = workspace
        .status(&network.prepared().context.name)
        .map_err(|e| e.to_string())?;
    Ok(Selection {
        network,
        phase: status.phase,
        running: status.running_peers,
        failure: status.failure,
    })
}

struct Opened {
    workspace: DeveloperWorkspace,
    names: Vec<String>,
    selected: UiResult<Option<Selection>>,
}

fn open_workspace(path: PathBuf) -> UiResult<Opened> {
    let workspace = DeveloperWorkspace::open(&path).map_err(|e| e.to_string())?;
    inspect_workspace(workspace)
}

fn inspect_workspace(workspace: DeveloperWorkspace) -> UiResult<Opened> {
    let names = workspace
        .contexts()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|c| c.name)
        .collect::<Vec<_>>();
    // An empty workspace deliberately creates neither a network nor a signing identity.
    let selected = workspace
        .selected_name()
        .map_err(|e| e.to_string())
        .and_then(|name| {
            name.map(|name| load_selection(&workspace, Some(&name)))
                .transpose()
        });
    Ok(Opened {
        workspace,
        names,
        selected,
    })
}

enum Message {
    Opened(UiResult<Opened>),
    Selected(UiResult<Selection>),
    Reset(UiResult<Vec<String>>),
    Dashboard(UiResult<DashboardSnapshot>),
    State(UiResult<StatePage>),
    Logs(UiResult<String>),
    Submitted(UiResult<String>),
    Review {
        evidence: String,
        decision: Sender<bool>,
    },
    Progress(String),
    Deployed {
        result: UiResult<String>,
        refreshed: UiResult<Opened>,
    },
}

struct Review {
    evidence: String,
    decision: Sender<bool>,
}

struct Desktop {
    runtime: Option<tokio::runtime::Runtime>,
    sender: Sender<(u64, Message)>,
    receiver: Receiver<(u64, Message)>,
    epoch: u64,
    busy: bool,
    workspace_path: String,
    workspace: Option<DeveloperWorkspace>,
    names: Vec<String>,
    new_name: String,
    selected: Option<Selection>,
    peer: usize,
    view: View,
    error: Option<String>,
    notice: Option<String>,
    reset_intent: bool,
    last_poll: Instant,
    dashboard: Option<DashboardSnapshot>,
    query: StateQueryKind,
    page: Option<StatePage>,
    filter: String,
    activity: VecDeque<String>,
    blocks: Option<(
        ManagedBlockStream,
        tokio::sync::broadcast::Receiver<mochi_core::BlockStreamEvent>,
    )>,
    events: Option<(
        ManagedEventStream,
        tokio::sync::broadcast::Receiver<mochi_core::EventStreamEvent>,
    )>,
    logs: String,
    drafts: String,
    action: usize,
    asset: String,
    quantity: String,
    destination: String,
    preview: Option<TransactionPreview>,
    submitted_hash: Option<String>,
    contract_path: String,
    contract_alias: String,
    package: String,
    contract: String,
    locked: bool,
    journal_path: String,
    review: Option<Review>,
    receipt: Option<String>,
}

impl Desktop {
    fn new(creation: &eframe::CreationContext<'_>, path: PathBuf) -> Self {
        let mut style = (*creation.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.visuals = egui::Visuals::dark();
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(72, 87, 167);
        creation.egui_ctx.set_style(style);
        let (sender, receiver) = mpsc::channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build();
        let error = runtime
            .as_ref()
            .err()
            .map(|e| format!("Cannot start desktop task runtime: {e}"));
        let mut app = Self {
            runtime: runtime.ok(),
            sender,
            receiver,
            epoch: 0,
            busy: false,
            workspace_path: path.to_string_lossy().into_owned(),
            workspace: None,
            names: Vec::new(),
            new_name: "local".into(),
            selected: None,
            peer: 0,
            view: View::Dashboard,
            error,
            notice: None,
            reset_intent: false,
            last_poll: Instant::now(),
            dashboard: None,
            query: StateQueryKind::Accounts,
            page: None,
            filter: String::new(),
            activity: VecDeque::new(),
            blocks: None,
            events: None,
            logs: String::new(),
            drafts: "[]".into(),
            action: 0,
            asset: String::new(),
            quantity: "1".into(),
            destination: String::new(),
            preview: None,
            submitted_hash: None,
            contract_path: String::new(),
            contract_alias: String::new(),
            package: String::new(),
            contract: String::new(),
            locked: false,
            journal_path: String::new(),
            review: None,
            receipt: None,
        };
        app.open();
        app
    }

    fn spawn(&mut self, work: impl FnOnce() -> Message + Send + 'static) {
        self.busy = true;
        self.error = None;
        let sender = self.sender.clone();
        let epoch = self.epoch;
        std::thread::spawn(move || {
            let _ = sender.send((epoch, work()));
        });
    }

    fn clear_network(&mut self) {
        self.selected = None;
        self.peer = 0;
        self.blocks = None;
        self.events = None;
        self.dashboard = None;
        self.page = None;
        self.logs.clear();
        self.activity.clear();
        self.preview = None;
        self.submitted_hash = None;
        self.receipt = None;
        self.reset_intent = false;
    }

    fn open(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.clear_network();
        self.workspace = None;
        self.names.clear();
        let path = PathBuf::from(&self.workspace_path);
        self.spawn(move || Message::Opened(open_workspace(path)));
    }

    fn select(&mut self, name: String) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        self.epoch = self.epoch.wrapping_add(1);
        self.clear_network();
        self.spawn(move || {
            Message::Selected(
                workspace
                    .select(&name)
                    .map_err(|e| e.to_string())
                    .and_then(|_| load_selection(&workspace, Some(&name))),
            )
        });
    }

    fn lifecycle(&mut self, start: bool) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let name = self.selected.as_ref().map_or_else(
            || self.new_name.trim().to_owned(),
            |s| s.network.prepared().context.name.clone(),
        );
        self.spawn(move || {
            Message::Selected(
                (if start {
                    workspace.start(&name)
                } else {
                    workspace.stop(&name)
                })
                .map_err(|e| e.to_string())
                .and_then(|_| load_selection(&workspace, Some(&name))),
            )
        });
    }

    fn refresh_selection(&mut self) {
        let (Some(workspace), Some(selected)) = (self.workspace.clone(), self.selected.as_ref())
        else {
            return;
        };
        let name = selected.network.prepared().context.name.clone();
        self.last_poll = Instant::now();
        self.spawn(move || Message::Selected(load_selection(&workspace, Some(&name))));
    }

    fn install_selection(&mut self, selection: Selection) {
        let changed = self
            .selected
            .as_ref()
            .is_none_or(|old| old.network.prepared() != selection.network.prepared());
        if changed {
            self.clear_network();
        }
        let name = selection.network.prepared().context.name.clone();
        if !self.names.contains(&name) {
            self.names.push(name);
            self.names.sort();
        }
        self.selected = Some(selection);
        self.last_poll = Instant::now();
        self.connect_streams();
    }

    fn connect_streams(&mut self) {
        let Some(selected) = self.selected.as_ref() else {
            return;
        };
        if selected.phase != ManagedPhase::Ready {
            self.blocks = None;
            self.events = None;
            return;
        }
        if self.blocks.is_some() && self.events.is_some() {
            return;
        }
        let Some(runtime) = &self.runtime else {
            return;
        };
        match selected.network.ledger_reader(self.peer) {
            Ok(reader) => {
                let label = format!("validator {}", self.peer + 1);
                let blocks =
                    ManagedBlockStream::spawn(runtime.handle(), label.clone(), reader.clone());
                let block_rx = blocks.subscribe();
                let events = ManagedEventStream::spawn(runtime.handle(), label, reader);
                let event_rx = events.subscribe();
                self.blocks = Some((blocks, block_rx));
                self.events = Some((events, event_rx));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn poll(&mut self) {
        while let Ok((epoch, message)) = self.receiver.try_recv() {
            if epoch != self.epoch {
                continue;
            }
            match message {
                Message::Review { evidence, decision } => {
                    self.review = Some(Review { evidence, decision });
                    continue;
                }
                Message::Progress(progress) => {
                    push_activity(&mut self.activity, progress);
                    continue;
                }
                _ => self.busy = false,
            }
            match message {
                Message::Opened(result) => match result {
                    Ok(opened) => {
                        self.workspace = Some(opened.workspace);
                        self.names = opened.names;
                        match opened.selected {
                            Ok(Some(selected)) => self.install_selection(selected),
                            Ok(None) => self.clear_network(),
                            Err(error) => {
                                self.clear_network();
                                self.error = Some(error);
                            }
                        }
                    }
                    Err(error) => self.error = Some(error),
                },
                Message::Selected(result) => match result {
                    Ok(selected) => self.install_selection(selected),
                    Err(error) => {
                        self.clear_network();
                        self.error = Some(error);
                    }
                },
                Message::Reset(result) => match result {
                    Ok(names) => {
                        self.clear_network();
                        self.names = names;
                        self.notice = Some(
                            "Localnet reset. Start to create a fresh identity and ledger.".into(),
                        );
                    }
                    Err(error) => self.error = Some(error),
                },
                Message::Dashboard(result) => match result {
                    Ok(snapshot) => self.dashboard = Some(snapshot),
                    Err(error) => self.error = Some(error),
                },
                Message::State(result) => match result {
                    Ok(page) => self.page = Some(page),
                    Err(error) => self.error = Some(error),
                },
                Message::Logs(result) => match result {
                    Ok(logs) => self.logs = logs,
                    Err(error) => self.error = Some(error),
                },
                Message::Submitted(result) => match result {
                    Ok(message) => self.notice = Some(message),
                    Err(error) => self.error = Some(error),
                },
                Message::Deployed { result, refreshed } => {
                    self.review = None;
                    match refreshed {
                        Ok(opened) => {
                            self.workspace = Some(opened.workspace);
                            self.names = opened.names;
                            match opened.selected {
                                Ok(Some(selected)) => self.install_selection(selected),
                                Ok(None) => self.clear_network(),
                                Err(error) => {
                                    self.clear_network();
                                    self.error = Some(error);
                                }
                            }
                        }
                        Err(error) => self.error = Some(error),
                    }
                    match result {
                        Ok(receipt) => self.receipt = Some(receipt),
                        Err(error) => self.error = Some(error),
                    }
                }
                Message::Review { .. } | Message::Progress(_) => {}
            }
        }
        if let Some((_, receiver)) = self.blocks.as_mut() {
            drain_stream(receiver, &mut self.activity);
        }
        if let Some((_, receiver)) = self.events.as_mut() {
            drain_stream(receiver, &mut self.activity);
        }
        if !self.busy
            && self.selected.is_some()
            && self.last_poll.elapsed() >= Duration::from_secs(5)
        {
            self.refresh_selection();
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Mochi");
            ui.label("Your Iroha workspace");
            if self.busy {
                ui.spinner();
                ui.label("Working…");
            }
        });
        ui.add_enabled_ui(!self.busy, |ui| {
            ui.horizontal(|ui| {
                ui.label("Workspace");
                ui.add(egui::TextEdit::singleline(&mut self.workspace_path).desired_width(520.0));
                if ui.button("Open").clicked() {
                    self.open();
                }
            });
            ui.horizontal(|ui| {
                let selected = self
                    .selected
                    .as_ref()
                    .map(|s| s.network.prepared().context.name.clone());
                let mut chosen = selected.clone();
                egui::ComboBox::from_id_salt("context")
                    .selected_text(selected.as_deref().unwrap_or("Choose localnet"))
                    .show_ui(ui, |ui| {
                        for name in &self.names {
                            ui.selectable_value(&mut chosen, Some(name.clone()), name);
                        }
                    });
                if chosen != selected {
                    if let Some(name) = chosen {
                        self.select(name);
                    }
                }
                if self.selected.is_none() {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_name)
                            .hint_text("Localnet name")
                            .desired_width(130.0),
                    );
                }
                let phase = self.selected.as_ref().map(|s| s.phase);
                if ui
                    .add_enabled(
                        self.workspace.is_some() && phase != Some(ManagedPhase::Ready),
                        egui::Button::new("Start localnet"),
                    )
                    .clicked()
                {
                    self.lifecycle(true);
                }
                if ui
                    .add_enabled(
                        matches!(phase, Some(ManagedPhase::Ready | ManagedPhase::Starting)),
                        egui::Button::new("Stop"),
                    )
                    .clicked()
                {
                    self.lifecycle(false);
                }
                if ui
                    .add_enabled(self.selected.is_some(), egui::Button::new("Refresh"))
                    .clicked()
                {
                    self.refresh_selection();
                }
                if ui
                    .add_enabled(
                        matches!(phase, Some(ManagedPhase::Stopped | ManagedPhase::Failed)),
                        egui::Button::new("Reset…"),
                    )
                    .clicked()
                {
                    self.reset_intent = true;
                }
            });
        });
        if let Some(selected) = &self.selected {
            ui.horizontal(|ui| {
                ui.strong(format!(
                    "{:?} · {} / 4 validators",
                    selected.phase, selected.running
                ));
                ui.label(&selected.network.prepared().context.chain_id);
            });
            if let Some(failure) = &selected.failure {
                ui.colored_label(egui::Color32::LIGHT_RED, failure);
            }
        } else if self.workspace.is_some() {
            ui.label("Start creates four local validators, a funded account and your client context. No configuration files needed.");
        }
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        ui.heading("Local network");
        ui.label(
            "Kagami and Mochi share this network. It stays available when you close the desktop.",
        );
        let Some(selected) = self.selected.as_ref() else {
            return;
        };
        let context = &selected.network.prepared().context;
        public_value(ui, "Account", &context.account_id);
        public_value(ui, "Network", &context.network_id);
        public_value(ui, "Dataspace", &context.dataspace_alias);
        ui.separator();
        for (index, peer) in selected.network.prepared().peers.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("Validator {}", index + 1));
                ui.monospace(&peer.torii_url);
                if ui.small_button("Copy URL").clicked() {
                    ui.ctx().copy_text(peer.torii_url.clone());
                }
            });
        }
        if ui
            .add_enabled(
                !self.busy && selected.phase == ManagedPhase::Ready,
                egui::Button::new("Refresh balances and blocks"),
            )
            .clicked()
        {
            let network = selected.network.clone();
            let peer = self.peer;
            if let Some(handle) = self.runtime.as_ref().map(|r| r.handle().clone()) {
                self.spawn(move || {
                    Message::Dashboard((|| {
                        let client = network.observer(peer).map_err(|e| e.to_string())?;
                        let context = &network.prepared().context;
                        handle
                            .block_on(fetch_dashboard_snapshot(
                                format!("Validator {}", peer + 1),
                                &client,
                                vec![DashboardAccountInput {
                                    label: context.name.clone(),
                                    account_id: context.account_id.clone(),
                                }],
                            ))
                            .map_err(|e| format!("{e:?}"))
                    })())
                });
            }
        }
        if let Some(snapshot) = &self.dashboard {
            ui.separator();
            ui.strong("Balances");
            for account in &snapshot.accounts {
                for balance in &account.balances {
                    ui.label(format!("{}  {}", balance.value, balance.definition_id));
                }
            }
            ui.strong("Recent blocks");
            for block in &snapshot.recent_blocks {
                ui.label(format!(
                    "#{} · {} transactions · {} rejected",
                    block.height, block.transactions_total, block.transactions_rejected
                ));
            }
        }
    }

    fn state(&mut self, ui: &mut egui::Ui) {
        ui.heading("State explorer");
        ui.horizontal(|ui| {
            let before = self.query;
            egui::ComboBox::from_id_salt("state_kind")
                .selected_text(self.query.label())
                .show_ui(ui, |ui| {
                    for kind in StateQueryKind::all() {
                        ui.selectable_value(&mut self.query, kind, kind.label());
                    }
                });
            if self.query != before {
                self.page = None;
            }
            if ui
                .add_enabled(!self.busy && self.ready(), egui::Button::new("Query"))
                .clicked()
            {
                self.fetch_state(false);
            }
            if ui
                .add_enabled(
                    !self.busy && self.page.as_ref().is_some_and(|p| p.cursor.is_some()),
                    egui::Button::new("Next page"),
                )
                .clicked()
            {
                self.fetch_state(true);
            }
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filter this page"));
        });
        if let Some(page) = &self.page {
            if page.kind != self.query {
                return;
            }
            ui.label(format!(
                "{} records on this page · {} remaining",
                page.entries.len(),
                page.remaining
            ));
            let filter = self.filter.to_lowercase();
            for entry in page
                .entries
                .iter()
                .filter(|e| e.search_blob.contains(&filter))
            {
                egui::CollapsingHeader::new(&entry.title)
                    .id_salt(&entry.title)
                    .show(ui, |ui| {
                        ui.label(&entry.subtitle);
                        ui.label(&entry.detail);
                        ui.horizontal(|ui| {
                            if let Some(json) = &entry.json {
                                if ui.button("Copy JSON").clicked() {
                                    ui.ctx().copy_text(json.clone());
                                }
                            }
                            if let Some(bytes) = &entry.norito_bytes {
                                if ui.button("Copy Norito hex").clicked() {
                                    ui.ctx().copy_text(hex::encode(bytes));
                                }
                            }
                        });
                        ui.monospace(entry.json.as_deref().unwrap_or(&entry.raw));
                    });
            }
        }
    }

    fn fetch_state(&mut self, next: bool) {
        let Some(selected) = &self.selected else {
            return;
        };
        let network = selected.network.clone();
        let peer = self.peer;
        let kind = self.query;
        let cursor = if next {
            self.page.as_ref().and_then(|p| p.cursor.clone())
        } else {
            None
        };
        let Some(handle) = self.runtime.as_ref().map(|r| r.handle().clone()) else {
            return;
        };
        self.spawn(move || {
            Message::State((|| {
                let client = network.observer(peer).map_err(|e| e.to_string())?;
                handle
                    .block_on(run_state_query(
                        client,
                        &network.signer(),
                        network.address_discriminant(),
                        kind,
                        cursor,
                        std::num::NonZeroU64::new(50),
                    ))
                    .map_err(|e| e.to_string())
            })())
        });
    }

    fn activity(&mut self, ui: &mut egui::Ui) {
        ui.heading("Activity");
        ui.label("Canonical block and event streams from the selected validator.");
        for entry in self.activity.iter().rev().take(100) {
            ui.monospace(entry);
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Validator logs");
            if ui
                .add_enabled(
                    !self.busy && self.selected.is_some(),
                    egui::Button::new("Read latest logs"),
                )
                .clicked()
            {
                if let (Some(workspace), Some(selected)) =
                    (self.workspace.clone(), self.selected.as_ref())
                {
                    let name = selected.network.prepared().context.name.clone();
                    let peer = self.peer;
                    self.spawn(move || {
                        Message::Logs(
                            workspace
                                .logs(&name, Some(peer), 64 * 1024)
                                .map_err(|e| e.to_string()),
                        )
                    });
                }
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.logs)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(14)
                .interactive(false),
        );
    }

    fn composer(&mut self, ui: &mut egui::Ui) {
        let _address_profile = self.selected.as_ref().map(|selected| {
            iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                selected.network.address_discriminant(),
            )
        });
        ui.heading("Compose a transaction");
        ui.label("Build instructions, review the exact signed transaction, then submit it once.");
        ui.horizontal(|ui| {
            const ACTIONS: [&str; 4] = ["Mint", "Burn", "Transfer", "Register account"];
            egui::ComboBox::from_id_salt("instruction")
                .selected_text(ACTIONS[self.action])
                .show_ui(ui, |ui| {
                    for (index, label) in ACTIONS.iter().enumerate() {
                        ui.selectable_value(&mut self.action, index, *label);
                    }
                });
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
                        .desired_width(90.0),
                );
            }
        });
        if self.action == 2 {
            ui.add(
                egui::TextEdit::singleline(&mut self.destination)
                    .hint_text("Destination account ID")
                    .desired_width(600.0),
            );
        }
        if ui.button("Add instruction").clicked() {
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
        ui.label("Draft JSON also supports asset definitions, roles, multisig and chain policy instructions.");
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
        if ui
            .add_enabled(
                !self.busy && self.ready(),
                egui::Button::new("Review transaction"),
            )
            .clicked()
        {
            if let Some(selected) = &self.selected {
                match drafts_from_json_str(&self.drafts)
                    .map_err(|e| e.to_string())
                    .and_then(|drafts| selected.network.preview(&drafts).map_err(|e| e.to_string()))
                {
                    Ok(preview) => {
                        self.preview = Some(preview);
                        self.submitted_hash = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
        }
        if let Some(preview) = &self.preview {
            ui.separator();
            public_value(ui, "Authority", preview.authority());
            public_value(ui, "Transaction", preview.hash());
            for instruction in preview.instructions() {
                ui.label(instruction);
            }
            if ui.button("Copy signed Norito hex").clicked() {
                ui.ctx().copy_text(preview.encoded_hex().to_owned());
            }
            if ui
                .add_enabled(
                    !self.busy && self.submitted_hash.as_deref() != Some(preview.hash()),
                    egui::Button::new("Submit reviewed transaction"),
                )
                .clicked()
            {
                if let (Some(selected), Some(handle)) = (
                    &self.selected,
                    self.runtime.as_ref().map(|r| r.handle().clone()),
                ) {
                    let network = selected.network.clone();
                    let peer = self.peer;
                    let preview = preview.clone();
                    self.submitted_hash = Some(preview.hash().to_owned());
                    self.spawn(move || Message::Submitted(handle.block_on(network.submit(peer, &preview)).map(|commit| format!("Applied {} in block {}", preview.hash(), commit.block_height))
                        .map_err(|e| format!("Transaction {}: {e}. Keep this original hash when checking an uncertain outcome.", preview.hash()))));
                }
            }
        }
    }

    fn contracts(&mut self, ui: &mut egui::Ui) {
        ui.heading("Deploy a smart contract");
        ui.label("Choose a .ko source, .to artifact, or Musubi package. Compilation, fees and durable recovery use the same service as Kagami.");
        ui.add(
            egui::TextEdit::singleline(&mut self.contract_path)
                .hint_text("Contract path")
                .desired_width(700.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.contract_alias)
                .hint_text("Alias (optional; generated in the selected dataspace)")
                .desired_width(700.0),
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
        if ui
            .add_enabled(
                !self.busy && self.workspace.is_some() && !self.contract_path.trim().is_empty(),
                egui::Button::new("Prepare deployment"),
            )
            .clicked()
        {
            self.deploy(false);
        }
        ui.separator();
        ui.label("Resume an interrupted deployment using its retained journal.");
        ui.add(
            egui::TextEdit::singleline(&mut self.journal_path)
                .hint_text("Journal path")
                .desired_width(700.0),
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
        if let Some(review) = &self.review {
            ui.separator();
            ui.strong("Review the exact deployment and quoted fees");
            ui.monospace(&review.evidence);
            let mut decision = None;
            ui.horizontal(|ui| {
                if ui.button("Deploy with these fees").clicked() {
                    decision = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(false);
                }
            });
            if let Some(accepted) = decision {
                if let Some(review) = self.review.take() {
                    let _ = review.decision.send(accepted);
                }
            }
        }
        if let Some(receipt) = &self.receipt {
            ui.separator();
            ui.strong("Verified deployment receipt");
            if ui.button("Copy receipt").clicked() {
                ui.ctx().copy_text(receipt.clone());
            }
            ui.monospace(receipt);
        }
    }

    fn deploy(&mut self, resume: bool) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let context = self
            .selected
            .as_ref()
            .map(|s| s.network.prepared().context.name.clone());
        let path = workspace.resolve_path(std::path::Path::new(if resume {
            &self.journal_path
        } else {
            &self.contract_path
        }));
        let alias = if self.contract_alias.trim().is_empty() {
            None
        } else {
            Some(self.contract_alias.trim().to_owned())
        };
        let package = optional_selector(&self.package);
        let contract = optional_selector(&self.contract);
        let locked = self.locked;
        let sender = self.sender.clone();
        let epoch = self.epoch;
        self.receipt = None;
        self.spawn(move || {
            let result = (|| {
                let mut review = |preflight: &mochi_core::developer::DeploymentPreflight| {
                    let evidence = norito::json::to_string_pretty(&preflight.to_json()?)?;
                    let (decision, answer) = mpsc::channel();
                    sender
                        .send((epoch, Message::Review { evidence, decision }))
                        .map_err(|_| std::io::Error::other("desktop closed before review"))?;
                    if !answer.recv().unwrap_or(false) {
                        return Err(
                            std::io::Error::other("deployment cancelled before dispatch").into(),
                        );
                    }
                    Ok(())
                };
                let mut progress = |event| {
                    let _ = sender.send((epoch, Message::Progress(format!("{event:?}"))));
                };
                let run = if resume {
                    workspace.resume(&path, context.as_deref(), &mut review, &mut progress)
                } else {
                    let input = ContractInput::from_path(&path, package, contract, locked)
                        .map_err(|e| e.to_string())?;
                    let alias = alias
                        .map(|value| value.parse())
                        .transpose()
                        .map_err(|e| format!("Invalid contract alias: {e}"))?;
                    workspace.deploy(
                        &input,
                        context.as_deref(),
                        alias,
                        &mut review,
                        &mut progress,
                    )
                }
                .map_err(|e| e.to_string())?;
                Ok(format!(
                    "Journal: {}\n{}",
                    run.journal.display(),
                    norito::json::to_string_pretty(
                        &run.receipt.to_json().map_err(|e| e.to_string())?
                    )
                    .map_err(|e| e.to_string())?
                ))
            })();
            let refreshed = inspect_workspace(workspace);
            Message::Deployed { result, refreshed }
        });
    }

    fn ready(&self) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|s| s.phase == ManagedPhase::Ready)
    }
}

impl eframe::App for Desktop {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        egui::TopBottomPanel::top("workspace").show(context, |ui| self.top_bar(ui));
        egui::SidePanel::left("navigation")
            .resizable(false)
            .min_width(155.0)
            .show(context, |ui| {
                ui.add_space(12.0);
                for view in View::ALL {
                    ui.selectable_value(&mut self.view, view, view.label());
                }
                ui.separator();
                if let Some(selected) = &self.selected {
                    ui.label("Observe validator");
                    let before = self.peer;
                    ui.add_enabled_ui(!self.busy, |ui| {
                        for index in 0..selected.network.prepared().peers.len() {
                            ui.selectable_value(
                                &mut self.peer,
                                index,
                                format!("Validator {}", index + 1),
                            );
                        }
                    });
                    if self.peer != before {
                        self.blocks = None;
                        self.events = None;
                        self.page = None;
                        self.dashboard = None;
                        self.connect_streams();
                    }
                }
            });
        egui::CentralPanel::default().show(context, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
                ui.separator();
            }
            if let Some(notice) = &self.notice {
                ui.colored_label(egui::Color32::LIGHT_GREEN, notice);
                ui.separator();
            }
            egui::ScrollArea::vertical().show(ui, |ui| match self.view {
                View::Dashboard => self.dashboard(ui),
                View::State => self.state(ui),
                View::Activity => self.activity(ui),
                View::Composer => self.composer(ui),
                View::Contracts => self.contracts(ui),
            });
        });
        if self.reset_intent {
            egui::Window::new("Reset localnet?").collapsible(false).resizable(false).show(context, |ui| {
                ui.label("This removes the stopped localnet's keys and ledger. Its old identity cannot be restored.");
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() { self.reset_intent = false; }
                    if ui.add_enabled(!self.busy, egui::Button::new("Reset this localnet")).clicked() {
                        self.reset_intent = false;
                        if let (Some(workspace), Some(selected)) = (self.workspace.clone(), &self.selected) {
                            let name = selected.network.prepared().context.name.clone();
                            self.spawn(move || Message::Reset(workspace.reset(&name).map_err(|e| e.to_string())
                                .and_then(|_| workspace.contexts().map(|contexts| contexts.into_iter().map(|c| c.name).collect()).map_err(|e| e.to_string()))));
                        }
                    }
                });
            });
        }
        context.request_repaint_after(Duration::from_millis(100));
    }
}

fn optional_selector(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn public_value(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(label);
        ui.monospace(value);
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(value.to_owned());
        }
    });
}

fn push_activity(activity: &mut VecDeque<String>, value: String) {
    activity.push_back(value.chars().take(4096).collect());
    while activity.len() > 256 {
        activity.pop_front();
    }
}

fn drain_stream<T: std::fmt::Debug + Clone>(
    receiver: &mut tokio::sync::broadcast::Receiver<T>,
    activity: &mut VecDeque<String>,
) {
    for _ in 0..64 {
        match receiver.try_recv() {
            Ok(event) => push_activity(activity, format!("{event:?}")),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(count)) => push_activity(
                activity,
                format!(
                    "Stream display skipped {count} messages; inspect canonical block history for complete data."
                ),
            ),
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_package_selectors_preserve_simple_source_default() {
        assert_eq!(optional_selector("  "), None);
        assert_eq!(optional_selector(" ledger "), Some("ledger".into()));
    }

    #[test]
    fn activity_is_bounded_by_record_count_and_unicode_characters() {
        let mut activity = VecDeque::new();
        for index in 0..300 {
            push_activity(&mut activity, index.to_string());
        }
        assert_eq!(activity.len(), 256);
        assert_eq!(activity.front().unwrap(), "44");
        push_activity(&mut activity, "界".repeat(5000));
        assert_eq!(activity.back().unwrap().chars().count(), 4096);
    }

    #[test]
    fn dropped_review_cancels_without_implicit_approval() {
        let (sender, receiver) = mpsc::channel::<bool>();
        drop(sender);
        assert!(!receiver.recv().unwrap_or(false));
    }

    #[test]
    fn stream_lag_is_visible_and_draining_is_bounded() {
        let (sender, mut receiver) = tokio::sync::broadcast::channel(2);
        for index in 0..10 {
            sender.send(index).unwrap();
        }
        let mut activity = VecDeque::new();
        drain_stream(&mut receiver, &mut activity);
        assert!(activity.front().unwrap().contains("skipped"));
        assert_eq!(activity.back().unwrap(), "9");
    }
}
