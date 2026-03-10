//! Animated Iroha monitor with torii ASCII art and festival metrics.

mod ascii;
mod etenraku;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
mod etenraku_trace;
mod fetch;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
mod synth;
mod theme;

use std::{
    collections::VecDeque,
    io::{self, Write},
    time::Duration,
};

use axum::http::Uri;
use clap::{Parser, ValueEnum};
use eyre::{Result, eyre};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table, Wrap},
};
use tokio::{signal, sync::mpsc, time, time::MissedTickBehavior};

use crate::{
    fetch::{
        NoticeLevel, PeerFetcher, PeerNotice, PeerSnapshot, PeerUpdate, STATUS_BODY_LIMIT,
        spawn_stub_cluster,
    },
    theme::{ThemeIntro, ThemeOptions},
};

#[derive(ValueEnum, Clone, Copy, Debug)]
enum ArtThemeArg {
    Night,
    Dawn,
    Sakura,
}

impl From<ArtThemeArg> for ascii::AsciiTheme {
    fn from(value: ArtThemeArg) -> Self {
        match value {
            ArtThemeArg::Night => ascii::AsciiTheme::Night,
            ArtThemeArg::Dawn => ascii::AsciiTheme::Dawn,
            ArtThemeArg::Sakura => ascii::AsciiTheme::Sakura,
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "iroha_monitor",
    about = "Festive Iroha monitor with ASCII torii",
    version
)]
#[allow(clippy::struct_excessive_bools)] // CLI toggles map directly to user flags; refactor would degrade UX.
struct Args {
    /// Refresh interval in milliseconds
    #[arg(short = 'i', long = "interval", default_value_t = 800)]
    refresh_ms: u64,

    /// Attach to existing peer Torii endpoints instead of local stubs
    #[arg(long = "attach", value_name = "URL", num_args = 1..)]
    attach: Vec<String>,

    /// Spawn animated local stubs instead of real peers
    #[arg(long = "spawn-lite", default_value_t = false)]
    spawn_lite: bool,

    /// Number of peers to spawn in stub mode
    #[arg(short = 'n', long = "peers", default_value_t = 4)]
    peers: usize,

    /// Skip the animated intro (useful for automated tests)
    #[arg(long = "no-theme", default_value_t = false)]
    no_theme: bool,

    /// Disable audio playback of the Etenraku theme
    #[arg(long = "no-audio", default_value_t = false)]
    no_audio: bool,

    /// External MIDI player command (optional)
    #[arg(long = "midi-player")]
    midi_player: Option<String>,

    /// MIDI file path to feed to --midi-player (defaults to built-in demo)
    #[arg(long = "midi-file")]
    midi_file: Option<String>,

    /// Render the gas history sparkline panel
    #[arg(long = "show-gas-trend", default_value_t = false)]
    show_gas_trend: bool,

    /// Speed multiplier for the ASCII animation (1 = default)
    #[arg(long = "art-speed", default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..=8))]
    art_speed: u16,

    /// Set the ASCII art palette (night, dawn, sakura)
    #[arg(long = "art-theme", value_enum, default_value_t = ArtThemeArg::Night)]
    art_theme: ArtThemeArg,

    /// Maximum frames to render when the monitor falls back to headless mode (0 = unlimited)
    #[arg(long = "headless-max-frames")]
    headless_max_frames: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let theme_playback = if args.no_theme {
        None
    } else {
        let intro = ThemeIntro::new();
        let options = ThemeOptions {
            audio: !args.no_audio,
            midi_player: args.midi_player.clone(),
            midi_file: args.midi_file.clone(),
        };
        Some(intro.play(options).await?)
    };

    let attach_endpoints = if args.spawn_lite || args.attach.is_empty() {
        None
    } else {
        Some(normalize_endpoints(&args.attach)?)
    };

    let (endpoints, stub_cluster) = if let Some(endpoints) = attach_endpoints {
        (endpoints, None)
    } else {
        let cluster = spawn_stub_cluster(args.peers.max(1)).await?;
        let urls = cluster.urls().to_vec();
        (urls, Some(cluster))
    };

    let ascii_config = ascii::AsciiConfig {
        speed: args.art_speed,
        theme: args.art_theme.into(),
    };

    let monitor_result = run_monitor(&args, endpoints, ascii_config).await;

    if let Some(mut playback) = theme_playback {
        playback.stop().await;
    }

    drop(stub_cluster);
    monitor_result
}

fn normalize_endpoints(raws: &[String]) -> Result<Vec<String>> {
    let mut normalized = Vec::with_capacity(raws.len());
    for raw in raws {
        normalized.push(normalize_endpoint(raw)?);
    }
    Ok(normalized)
}

fn normalize_endpoint(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(eyre!("Torii endpoint cannot be empty"));
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };

    let uri: Uri = candidate
        .parse()
        .map_err(|err| eyre!("invalid Torii endpoint `{trimmed}`: {err}"))?;

    match uri.scheme_str() {
        Some("http" | "https") => {}
        Some(other) => {
            return Err(eyre!(
                "unsupported scheme `{other}` for Torii endpoint `{trimmed}` (expected http or https)"
            ));
        }
        None => {
            return Err(eyre!(
                "Torii endpoint `{trimmed}` is missing a scheme; try prefixing with http://"
            ));
        }
    }

    if uri.host().is_none() {
        return Err(eyre!(
            "Torii endpoint `{trimmed}` is missing a host component"
        ));
    }

    Ok(candidate)
}

/// Frame budget for headless fallback when no explicit cap is provided.
///
/// With the default 800 ms refresh cadence this keeps the process alive for
/// just over three minutes, which is long enough for demos yet short enough to
/// avoid CI hangs when raw terminal access is unavailable.
const DEFAULT_HEADLESS_MAX_FRAMES: usize = 240;

fn headless_limit_from_args(args: &Args) -> Option<usize> {
    match args.headless_max_frames {
        Some(0) => None,
        Some(value) => usize::try_from(value).ok(),
        None => Some(DEFAULT_HEADLESS_MAX_FRAMES),
    }
}

async fn run_monitor(
    args: &Args,
    endpoints: Vec<String>,
    ascii_config: ascii::AsciiConfig,
) -> Result<()> {
    let refresh = Duration::from_millis(args.refresh_ms.max(200));
    let headless_limit = headless_limit_from_args(args);
    if let Err(err) = crossterm::terminal::enable_raw_mode() {
        if should_fallback_to_headless(&err) {
            eprintln!(
                "iroha_monitor: terminal raw mode unavailable ({err}); falling back to headless output. Press Ctrl+C to exit."
            );
            if let Some(limit) = headless_limit {
                eprintln!(
                    "iroha_monitor: automatic exit after {limit} headless frame(s); pass --headless-max-frames 0 to keep running"
                );
            }
            return run_monitor_headless(
                endpoints,
                refresh,
                ascii_config,
                args.show_gas_trend,
                headless_limit,
            )
            .await;
        }
        return Err(err.into());
    }

    let mut stdout = io::stdout();
    if let Err(err) = crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen) {
        let _ = crossterm::terminal::disable_raw_mode();
        if should_fallback_to_headless(&err) {
            eprintln!(
                "iroha_monitor: alternate-screen access denied ({err}); using headless fallback. Press Ctrl+C to exit."
            );
            if let Some(limit) = headless_limit {
                eprintln!(
                    "iroha_monitor: automatic exit after {limit} headless frame(s); pass --headless-max-frames 0 to keep running"
                );
            }
            return run_monitor_headless(
                endpoints,
                refresh,
                ascii_config,
                args.show_gas_trend,
                headless_limit,
            )
            .await;
        }
        return Err(err.into());
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_monitor_loop(
        endpoints,
        refresh,
        &mut terminal,
        ascii_config,
        args.show_gas_trend,
    )
    .await;

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    result
}

async fn run_monitor_headless(
    endpoints: Vec<String>,
    refresh: Duration,
    ascii_config: ascii::AsciiConfig,
    show_gas_trend: bool,
    mut max_frames: Option<usize>,
) -> Result<()> {
    if endpoints.is_empty() {
        return Err(eyre!("no endpoints configured"));
    }

    let mut fetcher = PeerFetcher::new(endpoints.clone(), refresh);
    let mut app = AppState::new(endpoints, refresh, ascii_config, show_gas_trend);
    let mut ticker = time::interval(refresh);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut printer = HeadlessPrinter::default();

    printer.render(&app)?;
    if !consume_headless_frame(&mut max_frames) {
        return finish_headless(&mut printer);
    }

    loop {
        tokio::select! {
            update = fetcher.recv() => {
                if let Some(update) = update {
                    app.update_peer(update);
                } else {
                    break;
                }
            }
            _ = ticker.tick() => {
                app.advance_animation();
                if let Err(err) = printer.render(&app) {
                    return Err(err.into());
                }
                if !consume_headless_frame(&mut max_frames) {
                    break;
                }
            }
            _ = signal::ctrl_c() => {
                break;
            }
        }
    }

    finish_headless(&mut printer)
}

fn finish_headless(printer: &mut HeadlessPrinter) -> Result<()> {
    printer.finish()?;
    Ok(())
}

fn consume_headless_frame(limit: &mut Option<usize>) -> bool {
    limit.as_mut().is_none_or(|remaining| {
        if *remaining == 0 {
            false
        } else {
            *remaining -= 1;
            *remaining > 0
        }
    })
}

fn should_fallback_to_headless(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::NotConnected
    ) || err
        .raw_os_error()
        .is_some_and(|code| matches!(code, 1 | 6 | 25))
}

#[derive(Default)]
struct HeadlessPrinter {
    last_line_len: usize,
}

impl HeadlessPrinter {
    fn render(&mut self, app: &AppState) -> io::Result<()> {
        let mut line = format_headless_line(app);
        if line.len() < self.last_line_len {
            let padding = " ".repeat(self.last_line_len - line.len());
            line.push_str(&padding);
        }
        let mut stdout = io::stdout();
        write!(stdout, "\r{line}")?;
        stdout.flush()?;
        self.last_line_len = line.len();
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        if self.last_line_len == 0 {
            return Ok(());
        }
        let mut stdout = io::stdout();
        writeln!(stdout)?;
        stdout.flush()?;
        self.last_line_len = 0;
        Ok(())
    }
}

async fn run_monitor_loop(
    endpoints: Vec<String>,
    refresh: Duration,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ascii_config: ascii::AsciiConfig,
    show_gas_trend: bool,
) -> Result<()> {
    if endpoints.is_empty() {
        return Err(eyre!("no endpoints configured"));
    }

    let mut fetcher = PeerFetcher::new(endpoints.clone(), refresh);
    let mut app = AppState::new(endpoints, refresh, ascii_config, show_gas_trend);
    let mut ticker = time::interval(refresh);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let (tx_input, mut rx_input) = mpsc::channel::<InputEvent>(16);
    spawn_input_listener(tx_input);

    loop {
        tokio::select! {
            update = fetcher.recv() => {
                if let Some(update) = update {
                    app.update_peer(update);
                } else {
                    break;
                }
            }
            Some(input) = rx_input.recv() => {
                match input {
                    InputEvent::Key(code, modifiers) => {
                        if app.handle_key(code, modifiers) {
                            break;
                        }
                        if let Err(err) = terminal.draw(|frame| render_ui(frame, &app)) {
                            return Err(err.into());
                        }
                    }
                    InputEvent::Resize => {
                        terminal.clear()?;
                        if let Err(err) = terminal.draw(|frame| render_ui(frame, &app)) {
                            return Err(err.into());
                        }
                    }
                }
            }
            _ = ticker.tick() => {
                app.advance_animation();
                if let Err(err) = terminal.draw(|frame| render_ui(frame, &app)) {
                    return Err(err.into());
                }
            }
            _ = signal::ctrl_c() => {
                break;
            }
        }
    }

    Ok(())
}

const MAX_GAS_HISTORY: usize = 180;

struct AppState {
    refresh: Duration,
    ascii: ascii::AsciiAnimator,
    peers: Vec<PeerSlot>,
    focus: Option<usize>,
    events: VecDeque<EventEntry>,
    show_gas_trend: bool,
    gas_history: VecDeque<u64>,
    sort: PeerSort,
    filter_issues_only: bool,
    search_query: String,
    search_editing: bool,
}

impl AppState {
    fn new(
        endpoints: Vec<String>,
        refresh: Duration,
        ascii_config: ascii::AsciiConfig,
        show_gas_trend: bool,
    ) -> Self {
        let peers: Vec<_> = endpoints.into_iter().map(PeerSlot::new).collect();
        let mut events = VecDeque::new();
        push_event(&mut events, EventSeverity::Info, {
            let peer_count = peers.len();
            format!("UPLINK ESTABLISHED. {peer_count} TARGETS ACQUIRED.")
        });
        Self {
            refresh,
            ascii: ascii::AsciiAnimator::with_config(ascii_config),
            peers,
            focus: (!events.is_empty()).then_some(0),
            events,
            show_gas_trend,
            gas_history: VecDeque::with_capacity(MAX_GAS_HISTORY),
            sort: PeerSort::Health,
            filter_issues_only: false,
            search_query: String::new(),
            search_editing: false,
        }
    }

    fn advance_animation(&mut self) {
        self.record_gas_snapshot();
        self.ascii.advance();
    }

    fn record_gas_snapshot(&mut self) {
        if !self.show_gas_trend {
            return;
        }
        let mut total = 0u64;
        for slot in &self.peers {
            if let Some(snapshot) = &slot.latest
                && let Some(g) = snapshot.metrics.gas_used
            {
                total = total.saturating_add(g);
            }
        }
        self.gas_history.push_back(total);
        while self.gas_history.len() > MAX_GAS_HISTORY {
            self.gas_history.pop_front();
        }
    }

    fn update_peer(&mut self, update: PeerUpdate) {
        if let Some(slot) = self.peers.get_mut(update.index) {
            slot.update(update.snapshot, &mut self.events);
        }
        self.normalize_focus();
    }

    fn focus_next(&mut self) {
        let visible = self.visible_peer_indices();
        if visible.is_empty() {
            return;
        }
        let current = self.selected_index();
        let pos = current
            .and_then(|index| visible.iter().position(|&candidate| candidate == index))
            .unwrap_or(0);
        self.focus = Some(visible[(pos + 1) % visible.len()]);
    }

    fn focus_prev(&mut self) {
        let visible = self.visible_peer_indices();
        if visible.is_empty() {
            return;
        }
        let current = self.selected_index();
        let pos = current
            .and_then(|index| visible.iter().position(|&candidate| candidate == index))
            .unwrap_or(0);
        let prev = if pos == 0 { visible.len() - 1 } else { pos - 1 };
        self.focus = Some(visible[prev]);
    }

    fn ascii_lines(&self, width: u16, max_lines: Option<u16>) -> Vec<String> {
        let limit = max_lines.map(usize::from);
        self.ascii.frame_with_height(width, limit)
    }

    fn show_gas_trend(&self) -> bool {
        self.show_gas_trend
    }

    fn gas_history(&self) -> impl Iterator<Item = u64> + '_ {
        self.gas_history.iter().copied()
    }

    fn visible_peer_indices(&self) -> Vec<usize> {
        let mut visible = (0..self.peers.len())
            .filter(|&index| {
                let slot = &self.peers[index];
                (!self.filter_issues_only || !matches!(peer_health(slot), PeerHealth::Healthy))
                    && self.matches_search(slot)
            })
            .collect::<Vec<_>>();
        visible.sort_by(|&left, &right| self.compare_peers(left, right));
        visible
    }

    fn matches_search(&self, slot: &PeerSlot) -> bool {
        if self.search_query.is_empty() {
            return true;
        }
        let needle = self.search_query.to_ascii_lowercase();
        slot.display_name().to_ascii_lowercase().contains(&needle)
            || slot.endpoint.to_ascii_lowercase().contains(&needle)
    }

    fn compare_peers(&self, left: usize, right: usize) -> std::cmp::Ordering {
        let left_slot = &self.peers[left];
        let right_slot = &self.peers[right];
        match self.sort {
            PeerSort::Health => peer_health_rank(right_slot)
                .cmp(&peer_health_rank(left_slot))
                .then_with(|| right_slot.latest_height().cmp(&left_slot.latest_height()))
                .then_with(|| left_slot.display_name().cmp(&right_slot.display_name())),
            PeerSort::Height => right_slot
                .latest_height()
                .cmp(&left_slot.latest_height())
                .then_with(|| peer_health_rank(right_slot).cmp(&peer_health_rank(left_slot)))
                .then_with(|| left_slot.display_name().cmp(&right_slot.display_name())),
            PeerSort::Latency => right_slot
                .latency_millis()
                .cmp(&left_slot.latency_millis())
                .then_with(|| peer_health_rank(right_slot).cmp(&peer_health_rank(left_slot)))
                .then_with(|| left_slot.display_name().cmp(&right_slot.display_name())),
            PeerSort::Name => left_slot
                .display_name()
                .cmp(&right_slot.display_name())
                .then_with(|| right_slot.latest_height().cmp(&left_slot.latest_height())),
        }
    }

    fn selected_index(&self) -> Option<usize> {
        let visible = self.visible_peer_indices();
        if visible.is_empty() {
            return None;
        }
        self.focus
            .filter(|idx| visible.iter().any(|candidate| candidate == idx))
            .or_else(|| visible.first().copied())
    }

    fn selected_peer(&self) -> Option<(usize, &PeerSlot)> {
        let idx = self.selected_index()?;
        Some((idx, &self.peers[idx]))
    }

    fn latest_event(&self) -> Option<&str> {
        self.events.back().map(|entry| entry.message.as_str())
    }

    fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.normalize_focus();
    }

    fn toggle_issue_filter(&mut self) {
        self.filter_issues_only = !self.filter_issues_only;
        self.normalize_focus();
    }

    fn begin_search(&mut self) {
        self.search_editing = true;
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_editing = false;
        self.normalize_focus();
    }

    fn handle_key(
        &mut self,
        code: crossterm::event::KeyCode,
        modifiers: crossterm::event::KeyModifiers,
    ) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};

        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c')) {
            return true;
        }

        if self.search_editing {
            match code {
                KeyCode::Esc => {
                    self.clear_search();
                }
                KeyCode::Enter => {
                    self.search_editing = false;
                    self.normalize_focus();
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.normalize_focus();
                }
                KeyCode::Char(ch)
                    if !modifiers.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) =>
                {
                    self.search_query.push(ch);
                    self.normalize_focus();
                }
                _ => {}
            }
            return false;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('n') | KeyCode::Right | KeyCode::Down => self.focus_next(),
            KeyCode::Char('p') | KeyCode::Left | KeyCode::Up => self.focus_prev(),
            KeyCode::Char('s') => self.cycle_sort(),
            KeyCode::Char('f') => self.toggle_issue_filter(),
            KeyCode::Char('/') => self.begin_search(),
            KeyCode::Char('x') => self.clear_search(),
            _ => {}
        }
        false
    }

    fn search_status(&self) -> String {
        if self.search_query.is_empty() {
            if self.search_editing {
                "search /".to_string()
            } else {
                "search off".to_string()
            }
        } else if self.search_editing {
            format!("search /{}_", self.search_query)
        } else {
            format!("search /{}", self.search_query)
        }
    }

    fn normalize_focus(&mut self) {
        self.focus = self.selected_index();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerSort {
    Health,
    Height,
    Latency,
    Name,
}

impl PeerSort {
    fn label(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Height => "height",
            Self::Latency => "latency",
            Self::Name => "name",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Health => Self::Height,
            Self::Height => Self::Latency,
            Self::Latency => Self::Name,
            Self::Name => Self::Health,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventSeverity {
    Info,
    Recovery,
    Warning,
    Critical,
}

impl EventSeverity {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Recovery => "OK",
            Self::Warning => "WARN",
            Self::Critical => "DOWN",
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Info => Style::default().fg(Color::Cyan),
            Self::Recovery => Style::default().fg(Color::Green),
            Self::Warning => Style::default().fg(Color::Yellow),
            Self::Critical => Style::default().fg(Color::LightRed),
        }
    }
}

impl From<NoticeLevel> for EventSeverity {
    fn from(value: NoticeLevel) -> Self {
        match value {
            NoticeLevel::Info => Self::Info,
            NoticeLevel::Warning => Self::Warning,
            NoticeLevel::Critical => Self::Critical,
        }
    }
}

#[derive(Clone, Debug)]
struct EventEntry {
    severity: EventSeverity,
    message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerHealth {
    Pending,
    Healthy,
    Degraded,
    Offline,
}

impl PeerHealth {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "INIT",
            Self::Healthy => "OK",
            Self::Degraded => "WARN",
            Self::Offline => "DOWN",
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Pending => Style::default().fg(Color::DarkGray),
            Self::Healthy => Style::default().fg(Color::Green),
            Self::Degraded => Style::default().fg(Color::Yellow),
            Self::Offline => Style::default().fg(Color::LightRed),
        }
    }
}

struct PeerSlot {
    endpoint: String,
    latest: Option<PeerSnapshot>,
    current_alert: Option<PeerNotice>,
}

impl PeerSlot {
    fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            latest: None,
            current_alert: None,
        }
    }

    fn update(&mut self, snapshot: PeerSnapshot, events: &mut VecDeque<EventEntry>) {
        let name = snapshot
            .status
            .as_ref()
            .and_then(|s| s.alias.clone())
            .unwrap_or_else(|| self.endpoint.clone());

        let was_online = self
            .latest
            .as_ref()
            .is_some_and(|previous| previous.status.is_some());
        let is_online = snapshot.status.is_some();

        if self.latest.is_none() && is_online {
            push_event(
                events,
                EventSeverity::Recovery,
                format!("{name}: telemetry online"),
            );
        } else if was_online && !is_online {
            push_event(
                events,
                EventSeverity::Critical,
                format!("{name}: status endpoint unavailable"),
            );
        } else if !was_online && is_online {
            push_event(
                events,
                EventSeverity::Recovery,
                format!("{name}: telemetry restored"),
            );
        }

        let next_alert = snapshot.primary_notice().cloned();
        if next_alert != self.current_alert {
            match (&self.current_alert, &next_alert) {
                (_, Some(alert)) => {
                    push_event(
                        events,
                        alert.level.into(),
                        format!("{name}: {}", alert.message),
                    );
                }
                (Some(_), None) if is_online => {
                    push_event(
                        events,
                        EventSeverity::Recovery,
                        format!("{name}: warning cleared"),
                    );
                }
                _ => {}
            }
        }

        self.current_alert = next_alert;
        self.latest = Some(snapshot);
    }

    fn display_name(&self) -> String {
        self.latest
            .as_ref()
            .and_then(|snap| snap.status.as_ref()?.alias.clone())
            .unwrap_or_else(|| self.endpoint.clone())
    }

    fn latest_height(&self) -> u64 {
        self.latest
            .as_ref()
            .and_then(|snapshot| snapshot.status.as_ref()?.blocks)
            .unwrap_or(0)
    }

    fn latency_millis(&self) -> u128 {
        self.latest
            .as_ref()
            .and_then(|snapshot| snapshot.latency)
            .map_or(u128::MAX, |latency| latency.as_millis())
    }
}

enum InputEvent {
    Key(crossterm::event::KeyCode, crossterm::event::KeyModifiers),
    Resize,
}

fn spawn_input_listener(tx: mpsc::Sender<InputEvent>) {
    std::thread::spawn(move || {
        use std::time::Duration;

        use crossterm::event::{self, Event};
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(key)) => {
                        if tx
                            .blocking_send(InputEvent::Key(key.code, key.modifiers))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Event::Resize(_, _)) => {
                        let _ = tx.blocking_send(InputEvent::Resize);
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    });
}

fn push_event(queue: &mut VecDeque<EventEntry>, severity: EventSeverity, msg: String) {
    const MAX_EVENTS: usize = 32;
    queue.push_back(EventEntry {
        severity,
        message: msg,
    });
    while queue.len() > MAX_EVENTS {
        queue.pop_front();
    }
}

fn render_ui(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let size = frame.area();
    if size.width < 90 || size.height < 20 {
        render_compact_ui(frame, app);
        return;
    }

    let footer_height = 1u16.min(size.height);
    let header_height = if size.height >= 30 { 8 } else { 7 }.min(size.height);
    let body_height = size.height.saturating_sub(header_height + footer_height);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(body_height),
            Constraint::Length(footer_height),
        ])
        .split(size);

    if layout[0].width >= 110 {
        let header_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(layout[0]);
        render_overview_panel(frame, header_split[0], app);
        render_banner_panel(frame, header_split[1], app);
    } else {
        render_overview_panel(frame, layout[0], app);
    }

    let body_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
        .split(layout[1]);
    render_peer_table(frame, body_split[0], app);

    let side_constraints = if app.show_gas_trend() && body_split[1].height >= 17 {
        vec![
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(5),
        ]
    } else {
        vec![Constraint::Length(7), Constraint::Min(0)]
    };
    let side_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints(side_constraints)
        .split(body_split[1]);

    render_focus_panel(frame, side_split[0], app);
    if side_split.len() >= 2 {
        render_events(frame, side_split[1], app);
    }
    if side_split.len() >= 3 {
        render_gas_trend(frame, side_split[2], app);
    }

    if footer_height > 0 {
        render_footer(frame, layout[2], app);
    }
}

fn format_summary_text(app: &AppState) -> String {
    let stats = collect_summary(app);
    format!(
        "online {}/{} • healthy {} • degraded {} • down {} • blocks {}/{} • tx {}/{} • gas {} • avg lat {} • refresh {} ms",
        stats.online,
        stats.peer_count,
        stats.healthy,
        stats.degraded,
        stats.offline,
        compact_u64(stats.blocks),
        compact_u64(stats.non_empty),
        compact_u64(stats.approved),
        compact_u64(stats.rejected),
        compact_u64(stats.gas),
        stats
            .avg_latency_ms
            .map_or_else(|| "-".to_string(), |value| format!("{value} ms")),
        app.refresh.as_millis()
    )
}

fn format_headless_line(app: &AppState) -> String {
    let summary = format_summary_text(app);
    app.latest_event().map_or_else(
        || format!("[headless] {summary}"),
        |event| format!("[headless] {summary} • {event}"),
    )
}

fn render_peer_table(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    let visible = app.visible_peer_indices();
    let (start, end, capacity) = visible_peer_window(app, &visible, area.height);
    let title = if visible.is_empty() {
        if app.peers.is_empty() {
            "Peers".to_string()
        } else {
            "Peers (no matches)".to_string()
        }
    } else if capacity == 0 {
        format!("Peers 1-{}/{}", visible.len().min(1), visible.len())
    } else {
        format!(
            "Peers {}-{}/{}  sort {}{}",
            start + 1,
            end,
            visible.len(),
            app.sort.label(),
            if app.filter_issues_only {
                "  issues only"
            } else {
                ""
            }
        )
    };

    let header = Row::new(vec![
        Cell::from("Peer"),
        Cell::from("Height"),
        Cell::from("Tx"),
        Cell::from("Queue"),
        Cell::from("Gas"),
        Cell::from("Latency"),
        Cell::from("Status"),
    ])
    .style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    );

    let rows = visible
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|&idx| {
            let slot = &app.peers[idx];
            let highlight = app.focus == Some(idx);
            let (name, blocks, txs, queue, gas, latency, note, health) = peer_row_data(slot);
            let mut row = Row::new(vec![
                Cell::from(name),
                Cell::from(blocks),
                Cell::from(txs),
                Cell::from(queue),
                Cell::from(gas),
                Cell::from(latency),
                Cell::from(note),
            ]);
            if highlight {
                row = row.style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(match health {
                            PeerHealth::Healthy => Color::Green,
                            PeerHealth::Degraded => Color::Yellow,
                            PeerHealth::Offline => Color::LightRed,
                            PeerHealth::Pending => Color::Gray,
                        })
                        .add_modifier(Modifier::BOLD),
                );
            } else {
                row = row.style(health.style());
            }
            row
        })
        .collect::<Vec<_>>();

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
    );

    frame.render_widget(table, area);
}

fn render_gas_trend(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height < 3 || area.width < 16 {
        return;
    }
    let data: Vec<u64> = app.gas_history().collect();
    let latest = data.last().copied().unwrap_or(0);
    let (min, max) = data
        .iter()
        .copied()
        .fold((u64::MAX, 0u64), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let min = if min == u64::MAX { 0 } else { min };
    let max = max.max(1);
    let title = format!(
        "Gas Trend  min {}  max {}  now {}",
        compact_u64(min),
        compact_u64(max),
        compact_u64(latest)
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

    if data.is_empty() {
        let help = Paragraph::new("Waiting for gas metrics...")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, area);
        return;
    }

    let spark = Sparkline::default()
        .block(block)
        .style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .data(&data)
        .max(max);
    frame.render_widget(spark, area);
}

fn peer_row_data(
    slot: &PeerSlot,
) -> (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    PeerHealth,
) {
    let health = peer_health(slot);
    let name = slot.display_name();
    let mut blocks = "-".to_string();
    let mut queue = "-".to_string();
    let mut txs = "-".to_string();
    let mut gas = "-".to_string();
    let mut latency = "-".to_string();
    let mut note = peer_note(slot);

    if let Some(snapshot) = &slot.latest {
        if let Some(status) = &snapshot.status {
            if let Some(b) = status.blocks {
                blocks = compact_u64(b);
            }
            let ok = status.txs_approved.unwrap_or(0);
            let rej = status.txs_rejected.unwrap_or(0);
            txs = format!("{}/{}", compact_u64(ok), compact_u64(rej));
            if let Some(q) = status.queue_size {
                queue = compact_u64(q);
            }
        }
        if let Some(g) = snapshot.metrics.gas_used {
            gas = compact_u64(g);
        }
        if let Some(lat) = snapshot.latency {
            let millis = lat.as_millis();
            latency = format!("{millis}ms");
        }
    } else {
        note = "waiting for first sample".to_string();
    }

    (
        name,
        blocks,
        txs,
        queue,
        gas,
        latency,
        format!("{} {}", health.label(), note),
        health,
    )
}

fn render_events(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    let lines: Vec<Line<'_>> = if app.events.is_empty() {
        vec![Line::from(Span::styled(
            "Waiting for peer activity...",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        app.events
            .iter()
            .rev()
            .map(|msg| {
                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", msg.severity.label()),
                        msg.severity.style().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&msg.message, Style::default().fg(Color::White)),
                ])
            })
            .collect()
    };

    let events = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    "Alerts & Activity",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(events, area);
}

fn render_compact_ui(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let size = frame.area();
    if size.height < 8 || size.width < 42 {
        let summary = Paragraph::new(format_summary_text(app))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Iroha Monitor"),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(summary, size);
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(size);
    render_overview_panel(frame, layout[0], app);

    if layout[1].height >= 8 {
        let body = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(4)])
            .split(layout[1]);
        render_peer_table(frame, body[0], app);
        render_events(frame, body[1], app);
    } else {
        render_peer_table(frame, layout[1], app);
    }
}

fn render_overview_panel(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    app: &AppState,
) {
    if area.height < 3 || area.width < 20 {
        return;
    }
    let stats = collect_summary(app);
    let focus_line = app.selected_peer().map_or_else(
        || "Focus none".to_string(),
        |(_, slot)| {
            let health = peer_health(slot);
            format!(
                "Focus {}  {}  {}",
                slot.display_name(),
                health.label(),
                peer_note(slot)
            )
        },
    );
    let lines = vec![
        Line::from(format!(
            "Peers {}/{} online  Healthy {}  Degraded {}  Down {}  Reported {}",
            stats.online,
            stats.peer_count,
            stats.healthy,
            stats.degraded,
            stats.offline,
            stats.reported
        )),
        Line::from(format!(
            "Blocks {} total / {} non-empty  Tx {} ok / {} rej",
            compact_u64(stats.blocks),
            compact_u64(stats.non_empty),
            compact_u64(stats.approved),
            compact_u64(stats.rejected)
        )),
        Line::from(format!(
            "Queue {}  Gas {}  Avg latency {}  Refresh {} ms",
            compact_u64(stats.queue),
            compact_u64(stats.gas),
            stats
                .avg_latency_ms
                .map_or_else(|| "-".to_string(), |value| format!("{value} ms")),
            app.refresh.as_millis()
        )),
        Line::from(format!(
            "Sort {}  Filter {}  {}",
            app.sort.label(),
            if app.filter_issues_only {
                "issues"
            } else {
                "all"
            },
            app.search_status()
        )),
        Line::from(focus_line),
    ];
    let overview = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    "Iroha Monitor",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(overview, area);
}

fn render_banner_panel(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    app: &AppState,
) {
    if area.height < 3 || area.width < 16 {
        return;
    }
    let banner_lines = app.ascii_lines(
        area.width.saturating_sub(2),
        Some(area.height.saturating_sub(2)),
    );
    let banner = Paragraph::new(banner_lines.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    "Festival Signal",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(banner, area);
}

fn render_focus_panel(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    let lines = if let Some((index, slot)) = app.selected_peer() {
        let health = peer_health(slot);
        let name = slot.display_name();
        let endpoint = slot.endpoint.as_str();
        let mut rows = vec![
            Line::from(vec![
                Span::styled("Peer ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    name,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(health.label(), health.style().add_modifier(Modifier::BOLD)),
                Span::raw(format!("  ({}/{})", index + 1, app.peers.len())),
            ]),
            Line::from(vec![
                Span::styled("Endpoint ", Style::default().fg(Color::DarkGray)),
                Span::styled(endpoint, Style::default().fg(Color::White)),
            ]),
        ];

        if let Some(snapshot) = &slot.latest {
            if let Some(status) = &snapshot.status {
                rows.push(Line::from(format!(
                    "Height {}  Tx {} / {}  Queue {}",
                    status.blocks.map_or_else(|| "-".to_string(), compact_u64),
                    status
                        .txs_approved
                        .map_or_else(|| "-".to_string(), compact_u64),
                    status
                        .txs_rejected
                        .map_or_else(|| "-".to_string(), compact_u64),
                    status
                        .queue_size
                        .map_or_else(|| "-".to_string(), compact_u64),
                )));
                rows.push(Line::from(format!(
                    "Latency {}  Commit {}  Views {}",
                    snapshot.latency.map_or_else(
                        || "-".to_string(),
                        |value| format!("{} ms", value.as_millis())
                    ),
                    status
                        .commit_time_ms
                        .map_or_else(|| "-".to_string(), |value| format!("{value} ms")),
                    status
                        .view_changes
                        .map_or_else(|| "-".to_string(), |value| value.to_string()),
                )));
                rows.push(Line::from(format!(
                    "Uptime {}  Gas {}  Fees {}",
                    status
                        .uptime
                        .map_or_else(|| "-".to_string(), |value| format!("{value} s")),
                    snapshot
                        .metrics
                        .gas_used
                        .map_or_else(|| "-".to_string(), compact_u64),
                    snapshot
                        .metrics
                        .fee_units
                        .map_or_else(|| "-".to_string(), compact_u64),
                )));
            }
        }
        rows.push(Line::from(format!("Note {}", peer_note(slot))));
        rows
    } else {
        vec![Line::from(if app.peers.is_empty() {
            "No peers configured"
        } else {
            "No peers match the current view"
        })]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            "Selected Peer",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    let panel = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn render_footer(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height == 0 {
        return;
    }
    let help = app.selected_peer().map_or_else(
        || format!(
            "q quit | arrows n/p focus | s sort:{} | f issues:{} | / search | x clear | {}",
            app.sort.label(),
            if app.filter_issues_only { "on" } else { "off" },
            app.search_status()
        ),
        |(_, slot)| {
            format!(
                "q quit | arrows n/p focus | s sort:{} | f issues:{} | / search | x clear | selected {} | {} | body limit {} bytes",
                app.sort.label(),
                if app.filter_issues_only { "on" } else { "off" },
                slot.display_name(),
                app.search_status(),
                STATUS_BODY_LIMIT
            )
        },
    );
    let footer = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, area);
}

struct SummaryStats {
    peer_count: usize,
    online: usize,
    healthy: usize,
    degraded: usize,
    offline: usize,
    reported: u64,
    blocks: u64,
    non_empty: u64,
    approved: u64,
    rejected: u64,
    queue: u64,
    gas: u64,
    avg_latency_ms: Option<u128>,
}

fn collect_summary(app: &AppState) -> SummaryStats {
    let mut online = 0usize;
    let mut healthy = 0usize;
    let mut degraded = 0usize;
    let mut offline = 0usize;
    let mut reported = 0u64;
    let mut blocks = 0u64;
    let mut non_empty = 0u64;
    let mut approved = 0u64;
    let mut rejected = 0u64;
    let mut queue = 0u64;
    let mut gas = 0u64;
    let mut latency_total = 0u128;
    let mut latency_count = 0u128;

    for slot in &app.peers {
        match peer_health(slot) {
            PeerHealth::Healthy => healthy += 1,
            PeerHealth::Degraded => degraded += 1,
            PeerHealth::Offline => offline += 1,
            PeerHealth::Pending => {}
        }

        if let Some(snapshot) = &slot.latest {
            if snapshot.status.is_some() {
                online += 1;
            }
            if let Some(status) = &snapshot.status {
                blocks += status.blocks.unwrap_or(0);
                non_empty += status.blocks_non_empty.unwrap_or(0);
                approved += status.txs_approved.unwrap_or(0);
                rejected += status.txs_rejected.unwrap_or(0);
                queue += status.queue_size.unwrap_or(0);
                reported = reported.max(status.peers.unwrap_or(0));
            }
            gas += snapshot.metrics.gas_used.unwrap_or(0);
            if let Some(latency) = snapshot.latency {
                latency_total += latency.as_millis();
                latency_count += 1;
            }
        }
    }

    SummaryStats {
        peer_count: app.peers.len(),
        online,
        healthy,
        degraded,
        offline,
        reported,
        blocks,
        non_empty,
        approved,
        rejected,
        queue,
        gas,
        avg_latency_ms: if latency_count > 0 {
            Some(latency_total / latency_count)
        } else {
            None
        },
    }
}

fn visible_peer_window(
    app: &AppState,
    visible: &[usize],
    area_height: u16,
) -> (usize, usize, usize) {
    let total = visible.len();
    let capacity = usize::from(area_height.saturating_sub(3));
    if total == 0 || capacity == 0 {
        return (0, total.min(capacity), capacity);
    }
    let selected = app
        .selected_index()
        .and_then(|index| visible.iter().position(|&candidate| candidate == index))
        .unwrap_or(0)
        .min(total - 1);
    let max_start = total.saturating_sub(capacity);
    let start = selected.saturating_sub(capacity / 2).min(max_start);
    let end = (start + capacity).min(total);
    (start, end, capacity)
}

fn peer_health(slot: &PeerSlot) -> PeerHealth {
    let Some(snapshot) = &slot.latest else {
        return PeerHealth::Pending;
    };
    if snapshot.status.is_none() {
        return PeerHealth::Offline;
    }

    snapshot
        .primary_notice()
        .map_or(PeerHealth::Healthy, |notice| match notice.level {
            NoticeLevel::Info => PeerHealth::Healthy,
            NoticeLevel::Warning => PeerHealth::Degraded,
            NoticeLevel::Critical => PeerHealth::Offline,
        })
}

fn peer_note(slot: &PeerSlot) -> String {
    if let Some(snapshot) = &slot.latest {
        if let Some(notice) = snapshot.primary_notice() {
            return notice.message.clone();
        }
        if let Some(status) = &snapshot.status {
            if let Some(commit_time_ms) = status.commit_time_ms {
                return format!("commit {commit_time_ms} ms");
            }
            if let Some(view_changes) = status.view_changes {
                return format!("view changes {view_changes}");
            }
            if let Some(uptime) = status.uptime {
                return format!("uptime {uptime} s");
            }
        }
        return "nominal".to_string();
    }
    "awaiting data".to_string()
}

fn peer_health_rank(slot: &PeerSlot) -> u8 {
    match peer_health(slot) {
        PeerHealth::Offline => 3,
        PeerHealth::Degraded => 2,
        PeerHealth::Pending => 1,
        PeerHealth::Healthy => 0,
    }
}

fn compact_u64(value: u64) -> String {
    const UNITS: [(&str, u64); 3] = [("B", 1_000_000_000), ("M", 1_000_000), ("k", 1_000)];
    for (suffix, scale) in UNITS {
        if value >= scale {
            let major = value as f64 / scale as f64;
            return if major >= 10.0 {
                format!("{major:.0}{suffix}")
            } else {
                format!("{major:.1}{suffix}")
            };
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::backend::TestBackend;

    use super::*;
    use crate::fetch::{
        MetricsSnapshot, NoticeKind, PeerNotice, PeerSnapshot, PeerUpdate, StatusPayload,
    };

    #[test]
    fn normalize_endpoint_adds_http_scheme() {
        let endpoint = normalize_endpoint("torii.local:8080").expect("normalize endpoint");
        assert_eq!(endpoint, "http://torii.local:8080");
    }

    #[test]
    fn normalize_endpoint_rejects_invalid_scheme() {
        let err = normalize_endpoint("ssh://torii.local").unwrap_err();
        assert!(
            err.to_string().contains("unsupported scheme"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn push_event_caps_history() {
        let mut events = VecDeque::new();
        for i in 0..48 {
            push_event(&mut events, EventSeverity::Info, format!("event {i}"));
        }
        assert!(events.len() <= 32);
        assert_eq!(events.front().unwrap().message, "event 16");
        assert_eq!(events.back().unwrap().message, "event 47");
    }

    #[test]
    fn gas_history_records_and_caps() {
        let mut app = AppState::new(
            vec!["http://stub".into()],
            Duration::from_millis(200),
            ascii::AsciiConfig::default(),
            true,
        );

        for i in 0..(MAX_GAS_HISTORY + 10) {
            let snapshot = PeerSnapshot {
                status: None,
                metrics: MetricsSnapshot {
                    gas_used: Some(i as u64),
                    fee_units: None,
                },
                latency: None,
                notices: Vec::new(),
            };
            app.update_peer(PeerUpdate { index: 0, snapshot });
            app.advance_animation();
        }

        let samples: Vec<u64> = app.gas_history().collect();
        assert!(samples.len() <= MAX_GAS_HISTORY);
        assert_eq!(samples.last().copied(), Some((MAX_GAS_HISTORY + 9) as u64));
    }

    #[test]
    fn summary_and_headless_line_include_metrics_and_events() {
        let refresh = Duration::from_millis(1_000);
        let mut app = AppState::new(
            vec!["http://peer-0".to_string()],
            refresh,
            ascii::AsciiConfig::default(),
            true,
        );

        let initial_line = format_headless_line(&app);
        assert!(
            initial_line.contains("UPLINK ESTABLISHED"),
            "expected initial line to mention awakening, got `{initial_line}`"
        );

        let status = StatusPayload {
            alias: Some("peer-0".to_string()),
            peers: Some(4),
            blocks: Some(10),
            blocks_non_empty: Some(6),
            txs_approved: Some(7),
            txs_rejected: Some(1),
            queue_size: Some(2),
            ..Default::default()
        };

        let snapshot = PeerSnapshot {
            status: Some(status),
            metrics: MetricsSnapshot {
                gas_used: Some(50),
                fee_units: None,
            },
            latency: Some(Duration::from_millis(30)),
            notices: Vec::new(),
        };

        app.update_peer(PeerUpdate { index: 0, snapshot });

        let summary = format_summary_text(&app);
        assert!(
            summary.contains("online 1/1"),
            "unexpected summary `{summary}`"
        );
        assert!(
            summary.contains("blocks 10/6"),
            "unexpected summary `{summary}`"
        );
        assert!(summary.contains("tx 7/1"), "unexpected summary `{summary}`");
        assert!(summary.contains("gas 50"), "unexpected summary `{summary}`");
        assert!(
            summary.contains("avg lat 30 ms"),
            "unexpected summary `{summary}`"
        );

        let headless_line = format_headless_line(&app);
        assert!(
            headless_line.contains("telemetry online"),
            "expected join event in `{headless_line}`"
        );
    }

    #[test]
    fn peer_updates_only_emit_warning_state_changes() {
        let mut events = VecDeque::new();
        let mut slot = PeerSlot::new("http://peer-0".to_string());
        let warning_snapshot = PeerSnapshot {
            status: Some(StatusPayload {
                alias: Some("peer-0".to_string()),
                ..Default::default()
            }),
            metrics: MetricsSnapshot::default(),
            latency: None,
            notices: vec![PeerNotice {
                level: NoticeLevel::Warning,
                kind: NoticeKind::MetricsFetchFailed,
                message: "metrics timeout".to_string(),
            }],
        };

        slot.update(warning_snapshot.clone(), &mut events);
        let first_len = events.len();
        slot.update(warning_snapshot, &mut events);

        assert_eq!(
            events.len(),
            first_len,
            "repeating the same warning should not flood the event log"
        );
    }

    #[test]
    fn render_ui_handles_narrow_terminal() {
        let mut app = AppState::new(
            vec!["stub://peer/1/3".to_string()],
            Duration::from_millis(500),
            ascii::AsciiConfig::default(),
            false,
        );
        app.advance_animation();

        let backend = TestBackend::new(48, 16);
        let mut terminal = Terminal::new(backend).expect("setup test backend");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal
                .draw(|frame| render_ui(frame, &app))
                .expect("render should succeed on narrow terminals");
        }));

        assert!(
            result.is_ok(),
            "render_ui panicked on narrow terminal: {result:?}"
        );
    }

    #[test]
    fn headless_fallback_accepts_non_tty_errors() {
        let err = io::Error::from_raw_os_error(6);
        assert!(should_fallback_to_headless(&err));
    }

    #[test]
    fn visible_peers_follow_sort_filter_and_search() {
        let mut app = AppState::new(
            vec![
                "http://peer-a".to_string(),
                "http://peer-b".to_string(),
                "http://peer-c".to_string(),
            ],
            Duration::from_millis(500),
            ascii::AsciiConfig::default(),
            false,
        );

        let healthy = PeerSnapshot {
            status: Some(StatusPayload {
                alias: Some("alpha".to_string()),
                blocks: Some(12),
                ..Default::default()
            }),
            metrics: MetricsSnapshot::default(),
            latency: Some(Duration::from_millis(20)),
            notices: Vec::new(),
        };
        let degraded = PeerSnapshot {
            status: Some(StatusPayload {
                alias: Some("beta".to_string()),
                blocks: Some(20),
                ..Default::default()
            }),
            metrics: MetricsSnapshot::default(),
            latency: Some(Duration::from_millis(85)),
            notices: vec![PeerNotice {
                level: NoticeLevel::Warning,
                kind: NoticeKind::MetricsFetchFailed,
                message: "metrics timeout".to_string(),
            }],
        };
        let offline = PeerSnapshot {
            status: None,
            metrics: MetricsSnapshot::default(),
            latency: None,
            notices: vec![PeerNotice {
                level: NoticeLevel::Critical,
                kind: NoticeKind::StatusFetchFailed,
                message: "status unreachable".to_string(),
            }],
        };

        app.update_peer(PeerUpdate {
            index: 0,
            snapshot: healthy,
        });
        app.update_peer(PeerUpdate {
            index: 1,
            snapshot: degraded,
        });
        app.update_peer(PeerUpdate {
            index: 2,
            snapshot: offline,
        });

        assert_eq!(app.visible_peer_indices(), vec![2, 1, 0]);

        app.toggle_issue_filter();
        assert_eq!(app.visible_peer_indices(), vec![2, 1]);

        app.search_query = "beta".to_string();
        app.normalize_focus();
        assert_eq!(app.visible_peer_indices(), vec![1]);
        assert_eq!(app.selected_index(), Some(1));

        app.search_query = "peer-c".to_string();
        app.normalize_focus();
        assert_eq!(app.visible_peer_indices(), vec![2]);

        app.search_query.clear();
        app.sort = PeerSort::Name;
        app.normalize_focus();
        assert_eq!(app.visible_peer_indices(), vec![1, 2]);
    }
}
