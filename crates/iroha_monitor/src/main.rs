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
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
};
use tokio::{signal, sync::mpsc, time, time::MissedTickBehavior};

use crate::{
    fetch::{PeerFetcher, PeerSnapshot, PeerUpdate, STATUS_BODY_LIMIT, spawn_stub_cluster},
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
        if permission_denied(&err) {
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
        if permission_denied(&err) {
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

fn permission_denied(err: &std::io::Error) -> bool {
    err.kind() == io::ErrorKind::PermissionDenied || err.raw_os_error() == Some(1)
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
                    InputEvent::Quit => break,
                    InputEvent::FocusNext => app.focus_next(),
                    InputEvent::FocusPrev => app.focus_prev(),
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
    events: VecDeque<String>,
    show_gas_trend: bool,
    gas_history: VecDeque<u64>,
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
        push_event(&mut events, {
            let peer_count = peers.len();
            format!("祭 network awakened with {peer_count} peers")
        });
        Self {
            refresh,
            ascii: ascii::AsciiAnimator::with_config(ascii_config),
            peers,
            focus: None,
            events,
            show_gas_trend,
            gas_history: VecDeque::with_capacity(MAX_GAS_HISTORY),
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
    }

    fn focus_next(&mut self) {
        if self.peers.is_empty() {
            return;
        }
        let current = self.focus.unwrap_or(usize::MAX);
        let next = if current + 1 >= self.peers.len() {
            0
        } else {
            current + 1
        };
        self.focus = Some(next);
    }

    fn focus_prev(&mut self) {
        if self.peers.is_empty() {
            return;
        }
        let current = self.focus.unwrap_or(0);
        let prev = if current == 0 {
            self.peers.len() - 1
        } else {
            current - 1
        };
        self.focus = Some(prev);
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

    fn latest_event(&self) -> Option<&str> {
        self.events.back().map(String::as_str)
    }
}

struct PeerSlot {
    endpoint: String,
    latest: Option<PeerSnapshot>,
    last_warning: Option<String>,
}

impl PeerSlot {
    fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            latest: None,
            last_warning: None,
        }
    }

    fn update(&mut self, snapshot: PeerSnapshot, events: &mut VecDeque<String>) {
        let name = snapshot
            .status
            .as_ref()
            .and_then(|s| s.alias.clone())
            .unwrap_or_else(|| self.endpoint.clone());

        if self.latest.is_none() {
            push_event(events, format!("{name} joined the matsuri"));
        }

        if let Some(warning) = snapshot.warnings.last() {
            push_event(events, format!("{name}: {warning}"));
            self.last_warning = Some(warning.clone());
        } else {
            self.last_warning = None;
        }

        self.latest = Some(snapshot);
    }

    fn display_name(&self) -> String {
        self.latest
            .as_ref()
            .and_then(|snap| snap.status.as_ref()?.alias.clone())
            .unwrap_or_else(|| self.endpoint.clone())
    }
}

enum InputEvent {
    Quit,
    FocusNext,
    FocusPrev,
}

fn spawn_input_listener(tx: mpsc::Sender<InputEvent>) {
    std::thread::spawn(move || {
        use std::time::Duration;

        use crossterm::event::{self, Event, KeyCode, KeyModifiers};
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(key)) => {
                        let send = |evt| tx.blocking_send(evt).ok();
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                send(InputEvent::Quit);
                                break;
                            }
                            KeyCode::Char('n') | KeyCode::Right | KeyCode::Down => {
                                send(InputEvent::FocusNext);
                            }
                            KeyCode::Char('p') | KeyCode::Left | KeyCode::Up => {
                                send(InputEvent::FocusPrev);
                            }
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                send(InputEvent::Quit);
                                break;
                            }
                            _ => {}
                        }
                    }
                    Ok(Event::Resize(_, _)) => {
                        let _ = tx.blocking_send(InputEvent::FocusNext);
                        let _ = tx.blocking_send(InputEvent::FocusPrev);
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    });
}

fn push_event(queue: &mut VecDeque<String>, msg: String) {
    const MAX_EVENTS: usize = 7;
    queue.push_back(msg);
    while queue.len() > MAX_EVENTS {
        queue.pop_front();
    }
}

fn render_ui(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let size = frame.area();
    let summary_height = 3u16;
    let min_bottom_height = if app.show_gas_trend() { 8u16 } else { 4u16 };
    let available_for_ascii = size
        .height
        .saturating_sub(summary_height.saturating_add(min_bottom_height));
    let available_for_ascii_usize = usize::from(available_for_ascii);

    let mut ascii_lines = if available_for_ascii_usize == 0 {
        Vec::new()
    } else {
        app.ascii_lines(size.width, Some(available_for_ascii))
    };

    let mut used_height = ascii_lines.len();
    let help_line_raw =
        format!("Press q to leave • n/p to change focus • body limit ≈ {STATUS_BODY_LIMIT} bytes");
    let width_columns = usize::from(size.width.max(1));

    if available_for_ascii_usize > used_height {
        let remaining = available_for_ascii_usize - used_height;
        let help_line = ascii::center_line(&help_line_raw, width_columns);
        if !ascii_lines.is_empty() && remaining >= 2 {
            ascii_lines.push(String::new());
            ascii_lines.push(help_line);
            used_height += 2;
        } else if remaining >= 1 {
            ascii_lines.push(help_line);
            used_height += 1;
        }
    }

    if ascii_lines.is_empty() && available_for_ascii_usize > 0 {
        ascii_lines.push(ascii::center_line(&help_line_raw, width_columns));
        used_height = ascii_lines.len();
    }

    let ascii_height = used_height.min(available_for_ascii_usize);
    let ascii_height = ascii_height.try_into().unwrap_or(u16::MAX).min(size.height);

    let summary_section_height = summary_height.min(size.height.saturating_sub(ascii_height));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(ascii_height),
            Constraint::Length(summary_section_height),
            Constraint::Min(0),
        ])
        .split(size);

    let header_text = if ascii_lines.is_empty() {
        String::new()
    } else {
        ascii_lines.join("\n")
    };

    let header = Paragraph::new(header_text).block(
        Block::default()
            .title(Span::styled(
                "祭 Matsuri Vision",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL),
    );
    frame.render_widget(header, layout[0]);

    if summary_section_height > 0 {
        let summary = build_summary(app);
        let summary_widget = Paragraph::new(summary).block(Block::default().borders(Borders::ALL));
        frame.render_widget(summary_widget, layout[1]);
    }

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(layout[2]);

    let peer_area = bottom[0];
    let events_area = bottom[1];
    let can_render_gas = app.show_gas_trend() && peer_area.height >= 8 && peer_area.height > 5;

    if can_render_gas {
        let gas_height = 5u16.min(peer_area.height - 1);
        let table_height = peer_area.height - gas_height;
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(table_height),
                Constraint::Length(gas_height),
            ])
            .split(peer_area);
        render_peer_table(frame, split[0], app);
        render_gas_trend(frame, split[1], app);
    } else {
        render_peer_table(frame, peer_area, app);
    }

    render_events(frame, events_area, app);
}

fn build_summary(app: &AppState) -> Line<'static> {
    let text = format_summary_text(app);
    Line::from(vec![Span::styled(
        text,
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )])
}

fn format_summary_text(app: &AppState) -> String {
    let mut online = 0u64;
    let mut blocks = 0u64;
    let mut approved = 0u64;
    let mut rejected = 0u64;
    let mut gas = 0u64;
    let mut non_empty = 0u64;
    let mut reported = 0u64;

    for slot in &app.peers {
        if let Some(snapshot) = &slot.latest {
            online += 1;
            if let Some(status) = &snapshot.status {
                if let Some(b) = status.blocks {
                    blocks = blocks.max(b);
                }
                if let Some(b) = status.blocks_non_empty {
                    non_empty = non_empty.max(b);
                }
                if let Some(ok) = status.txs_approved {
                    approved = approved.max(ok);
                }
                if let Some(rej) = status.txs_rejected {
                    rejected = rejected.max(rej);
                }
                if let Some(p) = status.peers {
                    reported = reported.max(p);
                }
            }
            if let Some(g_used) = snapshot.metrics.gas_used {
                gas += g_used;
            }
        }
    }

    let peer_count = app.peers.len();
    let refresh_ms = app.refresh.as_millis();
    format!(
        "online {online}/{peer_count} (report {reported}) • blocks {blocks}/{non_empty} • tx {approved}/{rejected} • gas {gas} • refresh {refresh_ms} ms"
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
    let header = Row::new(vec![
        Cell::from("Peer"),
        Cell::from("Blocks"),
        Cell::from("Tx ok/rej"),
        Cell::from("Queue"),
        Cell::from("Gas"),
        Cell::from("Latency"),
        Cell::from("Mood"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows = app
        .peers
        .iter()
        .enumerate()
        .map(|(idx, slot)| {
            let highlight = app.focus == Some(idx);
            let (name, blocks, txs, queue, gas, latency, note) = peer_row_data(slot);
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
                row = row.style(Style::default().bg(Color::DarkGray));
            }
            row
        })
        .collect::<Vec<_>>();

    let widths = [
        Constraint::Length(18),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default().borders(Borders::ALL).title(Span::styled(
            "Torii peers",
            Style::default().fg(Color::LightCyan),
        )),
    );

    frame.render_widget(table, area);
}

fn render_gas_trend(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
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
    let title = format!("Gas trend (min {min} • max {max} • latest {latest})");
    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        title,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));

    if data.is_empty() {
        let help = Paragraph::new("Awaiting gas samples...").block(block);
        frame.render_widget(help, area);
        return;
    }

    let spark = Sparkline::default()
        .block(block)
        .style(Style::default().fg(Color::LightYellow))
        .data(&data)
        .max(max);
    frame.render_widget(spark, area);
}

fn peer_row_data(slot: &PeerSlot) -> (String, String, String, String, String, String, String) {
    let name = slot.display_name();
    let mut blocks = "-".to_string();
    let mut queue = "-".to_string();
    let mut txs = "-".to_string();
    let mut gas = "-".to_string();
    let mut latency = "-".to_string();
    let mut note = slot
        .last_warning
        .clone()
        .unwrap_or_else(|| "煌く".to_string());

    if let Some(snapshot) = &slot.latest {
        if let Some(status) = &snapshot.status {
            if let Some(b) = status.blocks {
                blocks = b.to_string();
            }
            let ok = status.txs_approved.unwrap_or(0);
            let rej = status.txs_rejected.unwrap_or(0);
            txs = format!("{ok}/{rej}");
            if let Some(q) = status.queue_size {
                queue = q.to_string();
            }
            if snapshot.warnings.is_empty() {
                if let Some(ms) = status.commit_time_ms {
                    note = format!("{ms} ms");
                } else if let Some(vc) = status.view_changes {
                    note = format!("view {vc}");
                } else if let Some(up) = status.uptime {
                    note = format!("{up} s");
                } else {
                    note = "祭り良好".to_string();
                }
            }
        }
        if let Some(g) = snapshot.metrics.gas_used {
            gas = g.to_string();
        }
        if let Some(lat) = snapshot.latency {
            let millis = lat.as_millis();
            latency = format!("{millis}ms");
        }
    } else {
        note = "待機中".to_string();
    }

    (name, blocks, txs, queue, gas, latency, note)
}

fn render_events(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &AppState) {
    let lines: Vec<Line<'_>> = if app.events.is_empty() {
        vec![Line::from("(quiet shrine)")]
    } else {
        app.events
            .iter()
            .rev()
            .map(|msg| {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(Color::LightMagenta),
                ))
            })
            .collect()
    };

    let events =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(Span::styled(
            "Festival whispers",
            Style::default().fg(Color::LightMagenta),
        )));
    frame.render_widget(events, area);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::backend::TestBackend;

    use super::*;
    use crate::fetch::{MetricsSnapshot, PeerSnapshot, PeerUpdate, StatusPayload};

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
        for i in 0..16 {
            push_event(&mut events, format!("event {i}"));
        }
        assert!(events.len() <= 7);
        assert_eq!(events.front().unwrap(), "event 9");
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
                    ..Default::default()
                },
                latency: None,
                warnings: Vec::new(),
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
            initial_line.contains("network awakened"),
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
                ..Default::default()
            },
            latency: Some(Duration::from_millis(30)),
            warnings: Vec::new(),
        };

        app.update_peer(PeerUpdate { index: 0, snapshot });

        let summary = format_summary_text(&app);
        assert_eq!(
            summary,
            "online 1/1 (report 4) • blocks 10/6 • tx 7/1 • gas 50 • refresh 1000 ms"
        );

        let headless_line = format_headless_line(&app);
        assert!(
            headless_line.contains("joined the matsuri"),
            "expected join event in `{headless_line}`"
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
}
