use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use humantime::{format_duration, parse_duration};
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::config::{ChaosConfig, IzanamiArgs};

const HELP_TEXT: &str = "↑/↓ or Tab/Shift+Tab move  •  Enter edit/run  •  Esc/q exit";

pub fn launch(args: IzanamiArgs) -> Result<Option<(ChaosConfig, IzanamiArgs)>> {
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(args);
    let outcome = run_app(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    let final_args = app.args.clone();
    match outcome? {
        Outcome::Run(config) => Ok(Some((config, final_args))),
        Outcome::Exit => Ok(None),
    }
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<Outcome> {
    loop {
        terminal.draw(|f| app.draw(f))?;
        let evt = event::read()?;
        if let Event::Key(key) = evt {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            if let Some(outcome) = app.handle_key(key)? {
                return Ok(outcome);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    AllowNet,
    Peers,
    Faulty,
    Duration,
    PipelineTime,
    TargetBlocks,
    ProgressInterval,
    ProgressTimeout,
    Seed,
    Tps,
    MaxInflight,
    LogFilter,
    FaultMin,
    FaultMax,
    Run,
    Quit,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Field::AllowNet => "Allow Network",
            Field::Peers => "Peers",
            Field::Faulty => "Faulty Peers",
            Field::Duration => "Duration",
            Field::PipelineTime => "Pipeline Time",
            Field::TargetBlocks => "Target Blocks",
            Field::ProgressInterval => "Progress Interval",
            Field::ProgressTimeout => "Progress Timeout",
            Field::Seed => "Seed",
            Field::Tps => "Tx/s Target",
            Field::MaxInflight => "Max Inflight",
            Field::LogFilter => "Log Filter",
            Field::FaultMin => "Fault Interval Min",
            Field::FaultMax => "Fault Interval Max",
            Field::Run => "Run",
            Field::Quit => "Quit",
        }
    }

    fn all() -> &'static [Field] {
        const FIELDS: &[Field] = &[
            Field::AllowNet,
            Field::Peers,
            Field::Faulty,
            Field::Duration,
            Field::PipelineTime,
            Field::TargetBlocks,
            Field::ProgressInterval,
            Field::ProgressTimeout,
            Field::Seed,
            Field::Tps,
            Field::MaxInflight,
            Field::LogFilter,
            Field::FaultMin,
            Field::FaultMax,
            Field::Run,
            Field::Quit,
        ];
        FIELDS
    }

    fn editable(self) -> bool {
        !matches!(self, Field::Run | Field::Quit)
    }
}

struct InputState {
    field: Field,
    buffer: String,
}

struct Message {
    text: String,
    is_error: bool,
}

struct App {
    args: IzanamiArgs,
    list_state: ListState,
    input: Option<InputState>,
    message: Option<Message>,
}

impl App {
    fn new(mut args: IzanamiArgs) -> Self {
        // ensure seed default display is empty rather than zero
        if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {
            args.seed = None;
        }
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            args,
            list_state,
            input: None,
            message: None,
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(2),
                Constraint::Length(3),
            ])
            .split(f.area());

        let title = Paragraph::new("Izanami Chaosnet Configuration")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(title, chunks[0]);

        let items: Vec<ListItem> = Field::all()
            .iter()
            .map(|field| {
                let value = match field {
                    Field::Run => "(start)".to_string(),
                    Field::Quit => "(exit)".to_string(),
                    _ => self.value_for(*field),
                };
                let label = field.label();
                let line = Line::from(vec![Span::raw(format!("{label}: ")), Span::raw(value)]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Fields"))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("→ ");
        f.render_stateful_widget(list, chunks[1], &mut self.list_state);

        let info = self.message.as_ref().map_or_else(
            || Paragraph::new(HELP_TEXT.to_string()).style(Style::default().fg(Color::DarkGray)),
            |msg| {
                Paragraph::new(msg.text.clone()).style(if msg.is_error {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                })
            },
        );
        f.render_widget(info, chunks[2]);

        let editing = self.input.as_ref().map_or_else(
            || {
                Paragraph::new("Press Enter on a field to edit.")
                    .block(Block::default().borders(Borders::ALL).title("Input"))
                    .style(Style::default().fg(Color::DarkGray))
            },
            |input| {
                let label = input.field.label();
                let content = format!("Editing {label}: {}", input.buffer);
                Paragraph::new(content)
                    .block(Block::default().borders(Borders::ALL).title("Input"))
                    .wrap(Wrap { trim: false })
            },
        );
        f.render_widget(editing, chunks[3]);
    }

    #[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
    fn handle_key(&mut self, key: KeyEvent) -> Result<Option<Outcome>> {
        if let Some(mut input) = self.input.take() {
            let mut keep_current = true;
            match key.code {
                KeyCode::Esc => {
                    self.clear_message();
                    keep_current = false;
                }
                KeyCode::Enter => {
                    let buffer = input.buffer.clone();
                    match self.apply_input(input.field, &buffer) {
                        Ok(()) => {
                            self.message = Some(Message {
                                text: format!("Updated {}", input.field.label()),
                                is_error: false,
                            });
                            keep_current = false;
                        }
                        Err(err) => {
                            self.message = Some(Message {
                                text: err,
                                is_error: true,
                            });
                        }
                    }
                }
                KeyCode::Backspace => {
                    input.buffer.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.buffer.clear();
                }
                KeyCode::Char(c) => {
                    input.buffer.push(c);
                }
                KeyCode::Tab => {
                    let field = input.field;
                    let buffer = input.buffer.clone();
                    match self.apply_input(field, &buffer) {
                        Ok(()) => {
                            self.message = Some(Message {
                                text: format!("Updated {}", field.label()),
                                is_error: false,
                            });
                            keep_current = false;
                            let mut next = self.move_selection(true);
                            while !next.editable() {
                                if matches!(next, Field::Run | Field::Quit) {
                                    break;
                                }
                                next = self.move_selection(true);
                            }
                            if next.editable() {
                                self.start_edit(next);
                            }
                        }
                        Err(err) => {
                            self.message = Some(Message {
                                text: err,
                                is_error: true,
                            });
                        }
                    }
                }
                KeyCode::BackTab => {
                    let field = input.field;
                    let buffer = input.buffer.clone();
                    match self.apply_input(field, &buffer) {
                        Ok(()) => {
                            self.message = Some(Message {
                                text: format!("Updated {}", field.label()),
                                is_error: false,
                            });
                            keep_current = false;
                            let mut prev = self.move_selection(false);
                            while !prev.editable() {
                                if matches!(prev, Field::Run | Field::Quit) {
                                    break;
                                }
                                prev = self.move_selection(false);
                            }
                            if prev.editable() {
                                self.start_edit(prev);
                            }
                        }
                        Err(err) => {
                            self.message = Some(Message {
                                text: err,
                                is_error: true,
                            });
                        }
                    }
                }
                _ => {}
            }
            if keep_current && self.input.is_none() {
                self.input = Some(input);
            }
            return Ok(None);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(Some(Outcome::Exit)),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.next_field();
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                self.prev_field();
            }
            KeyCode::Char('r') => {
                if let Some(Outcome::Run(cfg)) = self.try_run_selected() {
                    return Ok(Some(Outcome::Run(cfg)));
                }
            }
            KeyCode::Enter => {
                if let Some(field) = self.selected_field() {
                    if field == Field::Run {
                        if let Some(Outcome::Run(cfg)) = self.try_run_selected() {
                            return Ok(Some(Outcome::Run(cfg)));
                        }
                    } else if field == Field::Quit {
                        return Ok(Some(Outcome::Exit));
                    } else {
                        self.start_edit(field);
                    }
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn start_edit(&mut self, field: Field) {
        if let Some(value) = self.string_for(field) {
            self.input = Some(InputState {
                field,
                buffer: value,
            });
            self.clear_message();
        }
    }

    fn apply_input(&mut self, field: Field, buffer: &str) -> Result<(), String> {
        match field {
            Field::Peers => {
                let v: usize = buffer
                    .trim()
                    .parse()
                    .map_err(|_| "Peers must be a positive integer".to_string())?;
                if v == 0 {
                    return Err("Peers must be greater than zero".into());
                }
                self.args.peers = v;
            }
            Field::Faulty => {
                let v: usize = buffer
                    .trim()
                    .parse()
                    .map_err(|_| "Faulty peers must be a non-negative integer".to_string())?;
                self.args.faulty = v;
            }
            Field::Duration => {
                let dur =
                    parse_duration(buffer.trim()).map_err(|e| format!("Invalid duration: {e}"))?;
                self.args.duration = dur;
            }
            Field::PipelineTime => {
                let trimmed = buffer.trim();
                self.args.pipeline_time = if trimmed.is_empty() {
                    None
                } else {
                    let dur = parse_duration(trimmed)
                        .map_err(|e| format!("Invalid pipeline time: {e}"))?;
                    if dur.is_zero() {
                        return Err("Pipeline time must be greater than zero".into());
                    }
                    Some(dur)
                };
            }
            Field::TargetBlocks => {
                let trimmed = buffer.trim();
                self.args.target_blocks = if trimmed.is_empty() {
                    None
                } else {
                    let value: u64 = trimmed
                        .parse()
                        .map_err(|_| "Target blocks must be a positive integer".to_string())?;
                    if value == 0 {
                        return Err("Target blocks must be greater than zero".into());
                    }
                    Some(value)
                };
            }
            Field::ProgressInterval => {
                let dur = parse_duration(buffer.trim())
                    .map_err(|e| format!("Invalid progress interval: {e}"))?;
                if dur.is_zero() {
                    return Err("Progress interval must be greater than zero".into());
                }
                self.args.progress_interval = dur;
            }
            Field::ProgressTimeout => {
                let dur = parse_duration(buffer.trim())
                    .map_err(|e| format!("Invalid progress timeout: {e}"))?;
                if dur.is_zero() {
                    return Err("Progress timeout must be greater than zero".into());
                }
                self.args.progress_timeout = dur;
            }
            Field::Seed => {
                let trimmed = buffer.trim();
                self.args.seed = if trimmed.is_empty() {
                    None
                } else {
                    Some(
                        trimmed
                            .parse()
                            .map_err(|_| "Seed must be an unsigned integer".to_string())?,
                    )
                };
            }
            Field::AllowNet => {
                let normalized = buffer.trim().to_ascii_lowercase();
                self.args.allow_net = match normalized.as_str() {
                    "1" | "true" | "yes" | "on" | "y" => true,
                    "0" | "false" | "no" | "off" | "n" => false,
                    other => {
                        return Err(format!(
                            "Allow Network must be true/false (received `{other}`)"
                        ));
                    }
                };
            }
            Field::Tps => {
                let v: f64 = buffer
                    .trim()
                    .parse()
                    .map_err(|_| "TPS must be a positive number".to_string())?;
                if v <= 0.0 {
                    return Err("TPS must be positive".into());
                }
                self.args.tps = v;
            }
            Field::MaxInflight => {
                let v: usize = buffer
                    .trim()
                    .parse()
                    .map_err(|_| "Max inflight must be a positive integer".to_string())?;
                if v == 0 {
                    return Err("Max inflight must be greater than zero".into());
                }
                self.args.max_inflight = v;
            }
            Field::LogFilter => {
                self.args.log_filter = buffer.trim().to_string();
            }
            Field::FaultMin => {
                let dur = parse_duration(buffer.trim())
                    .map_err(|e| format!("Invalid min interval: {e}"))?;
                self.args.fault_interval_min = dur;
            }
            Field::FaultMax => {
                let dur = parse_duration(buffer.trim())
                    .map_err(|e| format!("Invalid max interval: {e}"))?;
                self.args.fault_interval_max = dur;
            }
            Field::Run | Field::Quit => {}
        }
        Ok(())
    }

    fn try_run_selected(&mut self) -> Option<Outcome> {
        match ChaosConfig::try_from(self.args.clone()) {
            Ok(config) => {
                self.message = Some(Message {
                    text: "Starting Izanami...".into(),
                    is_error: false,
                });
                Some(Outcome::Run(config))
            }
            Err(e) => {
                self.message = Some(Message {
                    text: format!("Configuration error: {e}"),
                    is_error: true,
                });
                None
            }
        }
    }

    fn selected_field(&self) -> Option<Field> {
        self.list_state
            .selected()
            .and_then(|idx| Field::all().get(idx))
            .copied()
    }

    fn move_selection(&mut self, forward: bool) -> Field {
        if forward {
            self.next_field()
        } else {
            self.prev_field()
        }
    }

    fn next_field(&mut self) -> Field {
        let len = Field::all().len();
        let current = self.list_state.selected().unwrap_or(0);
        let next = (current + 1) % len;
        self.list_state.select(Some(next));
        self.selected_field().unwrap()
    }

    fn prev_field(&mut self) -> Field {
        let len = Field::all().len();
        let current = self.list_state.selected().unwrap_or(0);
        let prev = if current == 0 { len - 1 } else { current - 1 };
        self.list_state.select(Some(prev));
        self.selected_field().unwrap()
    }

    fn value_for(&self, field: Field) -> String {
        match field {
            Field::AllowNet => {
                if self.args.allow_net {
                    "enabled".to_string()
                } else {
                    "disabled".to_string()
                }
            }
            Field::Peers => self.args.peers.to_string(),
            Field::Faulty => self.args.faulty.to_string(),
            Field::Duration => format_duration(self.args.duration).to_string(),
            Field::PipelineTime => self.args.pipeline_time.map_or_else(
                || "(default)".to_string(),
                |duration| format_duration(duration).to_string(),
            ),
            Field::TargetBlocks => self
                .args
                .target_blocks
                .map_or_else(|| "(none)".to_string(), |value| value.to_string()),
            Field::ProgressInterval => format_duration(self.args.progress_interval).to_string(),
            Field::ProgressTimeout => format_duration(self.args.progress_timeout).to_string(),
            Field::Seed => self
                .args
                .seed
                .map_or_else(|| "(unset)".to_string(), |s| s.to_string()),
            Field::Tps => format!("{:.2}", self.args.tps),
            Field::MaxInflight => self.args.max_inflight.to_string(),
            Field::LogFilter => self.args.log_filter.clone(),
            Field::FaultMin => format_duration(self.args.fault_interval_min).to_string(),
            Field::FaultMax => format_duration(self.args.fault_interval_max).to_string(),
            Field::Run => "(start)".into(),
            Field::Quit => "(exit)".into(),
        }
    }

    fn string_for(&self, field: Field) -> Option<String> {
        Some(match field {
            Field::Peers => self.args.peers.to_string(),
            Field::Faulty => self.args.faulty.to_string(),
            Field::Duration => format_duration(self.args.duration).to_string(),
            Field::PipelineTime => self
                .args
                .pipeline_time
                .map(|duration| format_duration(duration).to_string())
                .unwrap_or_default(),
            Field::TargetBlocks => self
                .args
                .target_blocks
                .map(|value| value.to_string())
                .unwrap_or_default(),
            Field::ProgressInterval => format_duration(self.args.progress_interval).to_string(),
            Field::ProgressTimeout => format_duration(self.args.progress_timeout).to_string(),
            Field::Seed => self.args.seed.map(|s| s.to_string()).unwrap_or_default(),
            Field::AllowNet => self.args.allow_net.to_string(),
            Field::Tps => self.args.tps.to_string(),
            Field::MaxInflight => self.args.max_inflight.to_string(),
            Field::LogFilter => self.args.log_filter.clone(),
            Field::FaultMin => format_duration(self.args.fault_interval_min).to_string(),
            Field::FaultMax => format_duration(self.args.fault_interval_max).to_string(),
            Field::Run | Field::Quit => return None,
        })
    }

    fn clear_message(&mut self) {
        self.message = None;
    }
}

enum Outcome {
    Run(ChaosConfig),
    Exit,
}
