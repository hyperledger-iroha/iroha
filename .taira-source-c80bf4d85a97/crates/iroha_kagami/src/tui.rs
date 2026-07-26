//! Lightweight terminal UI helpers for Kagami commands.

use std::{
    fmt::Display,
    io::{self, IsTerminal as _, Write},
    sync::OnceLock,
};

use clap::ValueEnum;
use owo_colors::OwoColorize as _;

/// Mode in which the UI renders status messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiMode {
    /// Pick an appropriate mode automatically depending on the output device.
    Auto,
    /// Emit plain, non-colourised messages.
    Plain,
    /// Emit colourised, status-prefixed messages suited for interactive TTYs.
    Rich,
}

impl UiMode {
    fn resolve(self, is_tty: bool) -> UiMode {
        match self {
            UiMode::Auto => {
                if is_tty {
                    UiMode::Rich
                } else {
                    UiMode::Plain
                }
            }
            fixed => fixed,
        }
    }
}

/// Clap-facing argument for `[UiMode]`.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, Default)]
pub enum UiModeArg {
    #[default]
    Auto,
    Plain,
    Rich,
}

impl From<UiModeArg> for UiMode {
    fn from(arg: UiModeArg) -> UiMode {
        match arg {
            UiModeArg::Auto => UiMode::Auto,
            UiModeArg::Plain => UiMode::Plain,
            UiModeArg::Rich => UiMode::Rich,
        }
    }
}

/// Global UI handle.
pub struct Ui {
    mode: UiMode,
}

impl Ui {
    fn new(mode: UiMode) -> Self {
        Self { mode }
    }

    fn emit(&self, prefix: &str, message: impl Display, colour: Colour) {
        let mut stderr = io::stderr().lock();
        match self.mode {
            UiMode::Plain => {
                writeln!(stderr, "{prefix} {message}").ok();
            }
            UiMode::Rich => {
                let styled_prefix = match colour {
                    Colour::Info => prefix.bold().cyan().to_string(),
                    Colour::Success => prefix.bold().green().to_string(),
                    Colour::Warn => prefix.bold().yellow().to_string(),
                    Colour::Error => prefix.bold().red().to_string(),
                };
                writeln!(stderr, "{} {}", styled_prefix, message).ok();
            }
            UiMode::Auto => unreachable!("mode resolved at install time"),
        }
    }
}

#[derive(Clone, Copy)]
enum Colour {
    Info,
    Success,
    Warn,
    #[allow(dead_code)]
    Error,
}

static UI: OnceLock<Ui> = OnceLock::new();

/// Install the global UI configuration.
pub fn install(mode: UiMode) {
    let resolved = mode.resolve(io::stderr().is_terminal());
    let _ = UI.set(Ui::new(resolved));
}

fn ui() -> &'static Ui {
    UI.get_or_init(|| Ui::new(UiMode::Plain))
}

/// Emit an informational status line.
pub fn status(message: impl Display) {
    ui().emit("[..]", message, Colour::Info);
}

/// Emit a success status line.
pub fn success(message: impl Display) {
    ui().emit("[ok]", message, Colour::Success);
}

/// Emit a warning status line.
pub fn warn(message: impl Display) {
    ui().emit("[!!]", message, Colour::Warn);
}

/// Emit an error status line.
#[allow(dead_code)]
pub fn error(message: impl Display) {
    ui().emit("[xx]", message, Colour::Error);
}

/// Options shared across the CLI for configuring the UI.
#[derive(Debug, Clone, clap::Args)]
pub struct UiOptions {
    /// Control how Kagami formats status messages (auto detects TTY by default).
    #[clap(long = "ui-mode", value_enum, default_value_t)]
    pub mode: UiModeArg,
}

impl UiOptions {
    /// Configure the global UI per CLI options.
    pub fn install(&self) {
        install(self.mode.into());
    }
}
