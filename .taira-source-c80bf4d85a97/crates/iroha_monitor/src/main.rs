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
        StatusPayload, spawn_stub_cluster,
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

#[derive(Clone, Copy)]
struct MonitorTheme {
    background: Color,
    panel: Color,
    panel_alt: Color,
    border: Color,
    text: Color,
    muted: Color,
    title: Color,
    accent: Color,
    accent_alt: Color,
    healthy: Color,
    warning: Color,
    danger: Color,
    pending: Color,
    selected_bg: Color,
    selected_fg: Color,
}

const MONITOR_THEME: MonitorTheme = MonitorTheme {
    background: Color::Rgb(5, 9, 18),
    panel: Color::Rgb(10, 17, 30),
    panel_alt: Color::Rgb(14, 24, 40),
    border: Color::Rgb(54, 71, 92),
    text: Color::Rgb(221, 230, 239),
    muted: Color::Rgb(123, 139, 158),
    title: Color::Rgb(247, 249, 251),
    accent: Color::Rgb(94, 211, 255),
    accent_alt: Color::Rgb(255, 184, 108),
    healthy: Color::Rgb(69, 213, 155),
    warning: Color::Rgb(247, 202, 99),
    danger: Color::Rgb(255, 106, 122),
    pending: Color::Rgb(137, 150, 166),
    selected_bg: Color::Rgb(94, 211, 255),
    selected_fg: Color::Black,
};

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
        Style::default().fg(event_color(self))
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
    paint_background(frame, size);
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

fn peer_table_title(
    app: &AppState,
    visible: &[usize],
    start: usize,
    end: usize,
    capacity: usize,
) -> String {
    if visible.is_empty() {
        if app.peers.is_empty() {
            return "Peer Mesh".to_string();
        }
        return format!("Peer Mesh  no matches  {}", app.search_status());
    }

    let row_span = if capacity == 0 {
        format!("1-{}", visible.len().min(1))
    } else {
        format!("{}-{end}", start + 1)
    };
    format!(
        "Peer Mesh  rows {row_span}/{}  sort {}  filter {}  {}",
        visible.len(),
        app.sort.label(),
        if app.filter_issues_only {
            "issues"
        } else {
            "all"
        },
        app.search_status()
    )
}

fn format_headless_line(app: &AppState) -> String {
    let summary = format_summary_text(app);
    app.latest_event().map_or_else(
        || format!("[headless] {summary}"),
        |event| format!("[headless] {summary} • {event}"),
    )
}

fn paint_background(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
    frame.render_widget(
        Block::default().style(Style::default().bg(MONITOR_THEME.background)),
        area,
    );
}

fn panel_block(title: impl Into<String>, border: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(
            Style::default()
                .fg(MONITOR_THEME.text)
                .bg(MONITOR_THEME.panel),
        )
        .title(Span::styled(
            title.into(),
            Style::default()
                .fg(MONITOR_THEME.title)
                .add_modifier(Modifier::BOLD),
        ))
}

fn label_span(label: impl Into<String>) -> Span<'static> {
    Span::styled(
        label.into(),
        Style::default()
            .fg(MONITOR_THEME.muted)
            .add_modifier(Modifier::BOLD),
    )
}

fn value_span(value: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(
        value.into(),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn muted_span(value: impl Into<String>) -> Span<'static> {
    Span::styled(value.into(), Style::default().fg(MONITOR_THEME.muted))
}

fn separator_span() -> Span<'static> {
    Span::styled("  |  ", Style::default().fg(MONITOR_THEME.border))
}

fn push_metric(
    spans: &mut Vec<Span<'static>>,
    label: &'static str,
    value: impl Into<String>,
    color: Color,
) {
    if !spans.is_empty() {
        spans.push(separator_span());
    }
    spans.push(label_span(label));
    spans.push(Span::raw(" "));
    spans.push(value_span(value.into(), color));
}

fn health_color(health: PeerHealth) -> Color {
    match health {
        PeerHealth::Pending => MONITOR_THEME.pending,
        PeerHealth::Healthy => MONITOR_THEME.healthy,
        PeerHealth::Degraded => MONITOR_THEME.warning,
        PeerHealth::Offline => MONITOR_THEME.danger,
    }
}

fn event_color(severity: EventSeverity) -> Color {
    match severity {
        EventSeverity::Info => MONITOR_THEME.accent,
        EventSeverity::Recovery => MONITOR_THEME.healthy,
        EventSeverity::Warning => MONITOR_THEME.warning,
        EventSeverity::Critical => MONITOR_THEME.danger,
    }
}

fn overview_lines(app: &AppState) -> Vec<Line<'static>> {
    let stats = collect_summary(app);
    vec![
        overview_network_line(&stats),
        overview_ledger_line(&stats),
        overview_flow_line(&stats),
        overview_view_line(app),
        overview_focus_line(app),
    ]
}

fn overview_network_line(stats: &SummaryStats) -> Line<'static> {
    let mut network = Vec::new();
    push_metric(
        &mut network,
        "NETWORK",
        format!("{}/{} online", stats.online, stats.peer_count),
        MONITOR_THEME.accent,
    );
    push_metric(
        &mut network,
        "OK",
        stats.healthy.to_string(),
        MONITOR_THEME.healthy,
    );
    push_metric(
        &mut network,
        "WARN",
        stats.degraded.to_string(),
        MONITOR_THEME.warning,
    );
    push_metric(
        &mut network,
        "DOWN",
        stats.offline.to_string(),
        MONITOR_THEME.danger,
    );
    push_metric(
        &mut network,
        "REPORTED",
        stats.reported.to_string(),
        MONITOR_THEME.text,
    );
    Line::from(network)
}

fn overview_ledger_line(stats: &SummaryStats) -> Line<'static> {
    let mut ledger = Vec::new();
    push_metric(
        &mut ledger,
        "LEDGER",
        format!(
            "{} blocks / {} non-empty",
            compact_u64(stats.blocks),
            compact_u64(stats.non_empty)
        ),
        MONITOR_THEME.text,
    );
    push_metric(
        &mut ledger,
        "QUEUE",
        compact_u64(stats.queue),
        MONITOR_THEME.accent_alt,
    );
    Line::from(ledger)
}

fn overview_flow_line(stats: &SummaryStats) -> Line<'static> {
    let mut flow = Vec::new();
    push_metric(
        &mut flow,
        "FLOW",
        format!(
            "{} ok / {} rej",
            compact_u64(stats.approved),
            compact_u64(stats.rejected)
        ),
        MONITOR_THEME.text,
    );
    push_metric(
        &mut flow,
        "GAS",
        compact_u64(stats.gas),
        MONITOR_THEME.accent,
    );
    push_metric(
        &mut flow,
        "AVG LAT",
        stats
            .avg_latency_ms
            .map_or_else(|| "-".to_string(), |value| format!("{value} ms")),
        MONITOR_THEME.text,
    );
    Line::from(flow)
}

fn overview_view_line(app: &AppState) -> Line<'static> {
    let mut view = Vec::new();
    push_metric(
        &mut view,
        "VIEW",
        format!("sort {}", app.sort.label()),
        MONITOR_THEME.text,
    );
    push_metric(
        &mut view,
        "FILTER",
        if app.filter_issues_only {
            "issues".to_string()
        } else {
            "all".to_string()
        },
        if app.filter_issues_only {
            MONITOR_THEME.warning
        } else {
            MONITOR_THEME.text
        },
    );
    push_metric(
        &mut view,
        "SEARCH",
        app.search_status(),
        if app.search_editing {
            MONITOR_THEME.accent
        } else {
            MONITOR_THEME.text
        },
    );
    push_metric(
        &mut view,
        "REFRESH",
        format!("{} ms", app.refresh.as_millis()),
        MONITOR_THEME.muted,
    );
    Line::from(view)
}

fn overview_focus_line(app: &AppState) -> Line<'static> {
    let mut focus = Vec::new();
    focus.push(label_span("FOCUS"));
    focus.push(Span::raw(" "));
    if let Some((_, slot)) = app.selected_peer() {
        let health = peer_health(slot);
        focus.push(value_span(slot.display_name(), MONITOR_THEME.title));
        focus.push(Span::raw("  "));
        focus.push(value_span(health.label(), health_color(health)));
        focus.push(Span::raw("  "));
        focus.push(Span::styled(
            peer_note(slot),
            Style::default().fg(MONITOR_THEME.text),
        ));
    } else if app.peers.is_empty() {
        focus.push(muted_span("no peers configured"));
    } else {
        focus.push(muted_span("no peers match the current view"));
    }
    Line::from(focus)
}

fn render_peer_table(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    let visible = app.visible_peer_indices();
    let (start, end, capacity) = visible_peer_window(app, &visible, area.height);
    let title = peer_table_title(app, &visible, start, end, capacity);

    let header = Row::new(vec![
        Cell::from("Peer"),
        Cell::from("Height"),
        Cell::from("Tx ok/rej"),
        Cell::from("Q"),
        Cell::from("Gas"),
        Cell::from("RTT"),
        Cell::from("Signal"),
    ])
    .style(
        Style::default()
            .fg(MONITOR_THEME.selected_fg)
            .bg(MONITOR_THEME.accent)
            .add_modifier(Modifier::BOLD),
    );

    let rows = visible
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .enumerate()
        .map(|(row_offset, &idx)| {
            let slot = &app.peers[idx];
            let highlight = app.focus == Some(idx);
            let (name, blocks, txs, queue, gas, latency, note, health) = peer_row_data(slot);
            let health_color = health_color(health);
            let mut row = Row::new(vec![
                Cell::from(Span::styled(
                    name,
                    Style::default()
                        .fg(if matches!(health, PeerHealth::Healthy) {
                            MONITOR_THEME.title
                        } else {
                            health_color
                        })
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(
                    blocks,
                    Style::default().fg(MONITOR_THEME.text),
                )),
                Cell::from(Span::styled(txs, Style::default().fg(MONITOR_THEME.text))),
                Cell::from(Span::styled(
                    queue,
                    Style::default().fg(MONITOR_THEME.accent_alt),
                )),
                Cell::from(Span::styled(gas, Style::default().fg(MONITOR_THEME.accent))),
                Cell::from(Span::styled(
                    latency,
                    Style::default().fg(MONITOR_THEME.text),
                )),
                Cell::from(Span::styled(
                    note,
                    Style::default()
                        .fg(health_color)
                        .add_modifier(Modifier::BOLD),
                )),
            ]);
            if highlight {
                row = row.style(
                    Style::default()
                        .fg(MONITOR_THEME.selected_fg)
                        .bg(MONITOR_THEME.selected_bg)
                        .add_modifier(Modifier::BOLD),
                );
            } else {
                let background = if row_offset % 2 == 0 {
                    MONITOR_THEME.panel
                } else {
                    MONITOR_THEME.panel_alt
                };
                row = row.style(Style::default().fg(MONITOR_THEME.text).bg(background));
            }
            row
        })
        .collect::<Vec<_>>();

    let widths = [
        Constraint::Percentage(22),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(9),
        Constraint::Percentage(24),
    ];

    let table = Table::new(rows, widths).header(header).block(
        panel_block(title, MONITOR_THEME.border).style(
            Style::default()
                .fg(MONITOR_THEME.text)
                .bg(MONITOR_THEME.panel),
        ),
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
    let block = panel_block(title, MONITOR_THEME.accent);

    if data.is_empty() {
        let help = Paragraph::new("Waiting for gas metrics...")
            .block(block)
            .style(
                Style::default()
                    .fg(MONITOR_THEME.muted)
                    .bg(MONITOR_THEME.panel),
            );
        frame.render_widget(help, area);
        return;
    }

    let spark = Sparkline::default()
        .block(block)
        .style(
            Style::default()
                .fg(MONITOR_THEME.accent)
                .bg(MONITOR_THEME.panel),
        )
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
            Style::default().fg(MONITOR_THEME.muted),
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
                    Span::styled(&msg.message, Style::default().fg(MONITOR_THEME.text)),
                ])
            })
            .collect()
    };

    let events = Paragraph::new(lines)
        .block(panel_block(
            format!("Alerts & Activity  last {}", app.events.len()),
            MONITOR_THEME.border,
        ))
        .style(
            Style::default()
                .fg(MONITOR_THEME.text)
                .bg(MONITOR_THEME.panel),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(events, area);
}

fn render_compact_ui(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let size = frame.area();
    paint_background(frame, size);
    if size.height < 8 || size.width < 42 {
        let summary = Paragraph::new(format_summary_text(app))
            .block(panel_block("Iroha Monitor", MONITOR_THEME.accent))
            .style(
                Style::default()
                    .fg(MONITOR_THEME.text)
                    .bg(MONITOR_THEME.panel),
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
    let overview = Paragraph::new(overview_lines(app))
        .block(panel_block(
            "Iroha Monitor - Live Network",
            MONITOR_THEME.accent,
        ))
        .style(
            Style::default()
                .fg(MONITOR_THEME.text)
                .bg(MONITOR_THEME.panel),
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
        .block(panel_block("Festival Signal", MONITOR_THEME.accent_alt))
        .style(
            Style::default()
                .fg(MONITOR_THEME.muted)
                .bg(MONITOR_THEME.panel),
        );
    frame.render_widget(banner, area);
}

fn render_focus_panel(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    let panel = Paragraph::new(focus_panel_lines(app))
        .block(panel_block(focus_panel_title(app), focus_panel_border(app)))
        .style(
            Style::default()
                .fg(MONITOR_THEME.text)
                .bg(MONITOR_THEME.panel),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn focus_panel_lines(app: &AppState) -> Vec<Line<'static>> {
    app.selected_peer().map_or_else(
        || empty_focus_panel_lines(app),
        |(index, slot)| selected_peer_focus_lines(app, index, slot),
    )
}

fn focus_panel_border(app: &AppState) -> Color {
    app.selected_peer()
        .map_or(MONITOR_THEME.border, |(_, slot)| {
            health_color(peer_health(slot))
        })
}

fn focus_panel_title(app: &AppState) -> String {
    app.selected_peer().map_or_else(
        || "Selected Peer".to_string(),
        |(_, slot)| format!("Selected Peer - {}", peer_health(slot).label()),
    )
}

fn empty_focus_panel_lines(app: &AppState) -> Vec<Line<'static>> {
    vec![Line::from(if app.peers.is_empty() {
        muted_span("No peers configured")
    } else {
        muted_span("No peers match the current view")
    })]
}

fn selected_peer_focus_lines(app: &AppState, index: usize, slot: &PeerSlot) -> Vec<Line<'static>> {
    let health = peer_health(slot);
    let status_color = health_color(health);
    let mut rows = vec![
        selected_peer_header_line(app, index, slot),
        endpoint_line(slot),
    ];
    push_selected_peer_snapshot_lines(&mut rows, slot);
    rows.push(Line::from(vec![
        label_span("NOTE"),
        Span::raw(" "),
        Span::styled(peer_note(slot), Style::default().fg(status_color)),
    ]));
    rows
}

fn selected_peer_header_line(app: &AppState, index: usize, slot: &PeerSlot) -> Line<'static> {
    let health = peer_health(slot);
    Line::from(vec![
        label_span("PEER"),
        Span::raw(" "),
        Span::styled(
            slot.display_name(),
            Style::default()
                .fg(MONITOR_THEME.title)
                .add_modifier(Modifier::BOLD),
        ),
        separator_span(),
        value_span(health.label(), health_color(health)),
        Span::raw(format!("  {}/{}", index + 1, app.peers.len())),
    ])
}

fn endpoint_line(slot: &PeerSlot) -> Line<'static> {
    Line::from(vec![
        label_span("ENDPOINT"),
        Span::raw(" "),
        Span::styled(
            slot.endpoint.clone(),
            Style::default().fg(MONITOR_THEME.text),
        ),
    ])
}

fn push_selected_peer_snapshot_lines(rows: &mut Vec<Line<'static>>, slot: &PeerSlot) {
    if let Some(snapshot) = &slot.latest
        && let Some(status) = &snapshot.status
    {
        let mut ledger = Vec::new();
        push_metric(
            &mut ledger,
            "HEIGHT",
            status.blocks.map_or_else(|| "-".to_string(), compact_u64),
            MONITOR_THEME.text,
        );
        push_metric(
            &mut ledger,
            "TX",
            format!(
                "{} / {}",
                status
                    .txs_approved
                    .map_or_else(|| "-".to_string(), compact_u64),
                status
                    .txs_rejected
                    .map_or_else(|| "-".to_string(), compact_u64)
            ),
            MONITOR_THEME.text,
        );
        push_metric(
            &mut ledger,
            "QUEUE",
            status
                .queue_size
                .map_or_else(|| "-".to_string(), compact_u64),
            MONITOR_THEME.accent_alt,
        );
        rows.push(Line::from(ledger));
        rows.push(selected_peer_timing_line(snapshot, status));
        rows.push(selected_peer_resource_line(snapshot, status));
    }
}

fn selected_peer_timing_line(snapshot: &PeerSnapshot, status: &StatusPayload) -> Line<'static> {
    let mut timing = Vec::new();
    push_metric(
        &mut timing,
        "LATENCY",
        snapshot.latency.map_or_else(
            || "-".to_string(),
            |value| format!("{} ms", value.as_millis()),
        ),
        MONITOR_THEME.text,
    );
    push_metric(
        &mut timing,
        "COMMIT",
        status
            .commit_time_ms
            .map_or_else(|| "-".to_string(), |value| format!("{value} ms")),
        MONITOR_THEME.text,
    );
    push_metric(
        &mut timing,
        "VIEWS",
        status
            .view_changes
            .map_or_else(|| "-".to_string(), |value| value.to_string()),
        MONITOR_THEME.warning,
    );
    Line::from(timing)
}

fn selected_peer_resource_line(snapshot: &PeerSnapshot, status: &StatusPayload) -> Line<'static> {
    let mut resources = Vec::new();
    push_metric(
        &mut resources,
        "UPTIME",
        status
            .uptime
            .map_or_else(|| "-".to_string(), |value| format!("{value} s")),
        MONITOR_THEME.text,
    );
    push_metric(
        &mut resources,
        "GAS",
        snapshot
            .metrics
            .gas_used
            .map_or_else(|| "-".to_string(), compact_u64),
        MONITOR_THEME.accent,
    );
    push_metric(
        &mut resources,
        "FEES",
        snapshot
            .metrics
            .fee_units
            .map_or_else(|| "-".to_string(), compact_u64),
        MONITOR_THEME.text,
    );
    Line::from(resources)
}

fn render_footer(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    if area.height == 0 {
        return;
    }
    let selected = app
        .selected_peer()
        .map(|(_, slot)| format!("selected {}", slot.display_name()));
    let mut spans = vec![
        label_span("q"),
        muted_span(" quit"),
        separator_span(),
        label_span("arrows n/p"),
        muted_span(" focus"),
        separator_span(),
        label_span("s"),
        muted_span(format!(" sort:{}", app.sort.label())),
        separator_span(),
        label_span("f"),
        muted_span(format!(
            " issues:{}",
            if app.filter_issues_only { "on" } else { "off" }
        )),
        separator_span(),
        label_span("/"),
        muted_span(" search"),
        separator_span(),
        label_span("x"),
        muted_span(" clear"),
        separator_span(),
        muted_span(app.search_status()),
    ];
    if let Some(selected) = selected {
        spans.push(separator_span());
        spans.push(value_span(selected, MONITOR_THEME.text));
        spans.push(separator_span());
        spans.push(muted_span(format!("body limit {STATUS_BODY_LIMIT} bytes")));
    }
    let footer = Paragraph::new(Line::from(spans)).style(
        Style::default()
            .fg(MONITOR_THEME.muted)
            .bg(MONITOR_THEME.background),
    );
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
            let scale_u128 = u128::from(scale);
            return if value >= scale.saturating_mul(10) {
                let rounded = (u128::from(value) + scale_u128 / 2) / scale_u128;
                format!("{rounded}{suffix}")
            } else {
                let tenths = (u128::from(value) * 10 + scale_u128 / 2) / scale_u128;
                let whole = tenths / 10;
                let frac = tenths % 10;
                format!("{whole}.{frac}{suffix}")
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

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

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
                    fee_scale: None,
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
                fee_scale: None,
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
    fn overview_lines_surface_health_and_controls() {
        let mut app = AppState::new(
            vec!["http://peer-a".to_string(), "http://peer-b".to_string()],
            Duration::from_millis(800),
            ascii::AsciiConfig::default(),
            false,
        );

        app.update_peer(PeerUpdate {
            index: 0,
            snapshot: PeerSnapshot {
                status: Some(StatusPayload {
                    alias: Some("alpha".to_string()),
                    peers: Some(2),
                    blocks: Some(42),
                    blocks_non_empty: Some(40),
                    txs_approved: Some(12),
                    queue_size: Some(3),
                    ..Default::default()
                }),
                metrics: MetricsSnapshot {
                    gas_used: Some(900),
                    fee_units: None,
                    fee_scale: None,
                },
                latency: Some(Duration::from_millis(25)),
                notices: Vec::new(),
            },
        });
        app.update_peer(PeerUpdate {
            index: 1,
            snapshot: PeerSnapshot {
                status: None,
                metrics: MetricsSnapshot::default(),
                latency: None,
                notices: vec![PeerNotice {
                    level: NoticeLevel::Critical,
                    kind: NoticeKind::StatusFetchFailed,
                    message: "status unreachable".to_string(),
                }],
            },
        });

        let lines = overview_lines(&app)
            .iter()
            .map(line_text)
            .collect::<Vec<_>>();

        assert!(
            lines[0].contains("NETWORK 1/2 online")
                && lines[0].contains("OK 1")
                && lines[0].contains("DOWN 1"),
            "unexpected network line: {}",
            lines[0]
        );
        assert!(lines[1].contains("LEDGER 42 blocks / 40 non-empty"));
        assert!(lines[2].contains("GAS 900"));
        assert!(lines[3].contains("VIEW sort health"));
        assert!(lines[4].contains("FOCUS"));
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

    #[test]
    fn peer_table_title_tracks_window_filter_and_search() {
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
        app.filter_issues_only = true;
        app.search_query = "beta".to_string();
        let visible = vec![1usize];

        let title = peer_table_title(&app, &visible, 0, 1, 4);
        assert!(title.contains("Peer Mesh  rows 1-1/1"));
        assert!(title.contains("filter issues"));
        assert!(title.contains("search /beta"));

        app.search_query = "missing".to_string();
        let title = peer_table_title(&app, &[], 0, 0, 4);
        assert!(title.contains("no matches"));
        assert!(title.contains("search /missing"));
    }
}
