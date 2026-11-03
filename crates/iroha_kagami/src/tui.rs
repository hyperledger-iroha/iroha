//! Lightweight terminal UI helpers for Kagami commands.
use clap::ValueEnum;
use std::{
    fmt::Display,
    io::{self, IsTerminal as _, Write},
    sync::OnceLock,
};
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
    fn emit(&self, message: impl Display, colour: Colour) {
        let mut stderr = io::stderr().lock();
        self.write_line(&mut stderr, message, colour).ok();
    }
    fn write_line(
        &self,
        output: &mut impl Write,
        message: impl Display,
        colour: Colour,
    ) -> io::Result<()> {
        // A single static prefix avoids both an intermediate String and extra writes to
        // unbuffered stderr for each ANSI escape in the rich form.
        let prefix = match (self.mode, colour) {
            (UiMode::Plain, Colour::Info) => "[..]",
            (UiMode::Plain, Colour::Success) => "[ok]",
            (UiMode::Plain, Colour::Warn) => "[!!]",
            (UiMode::Rich, Colour::Info) => "\x1b[36m\x1b[1m[..]\x1b[0m\x1b[39m",
            (UiMode::Rich, Colour::Success) => "\x1b[32m\x1b[1m[ok]\x1b[0m\x1b[39m",
            (UiMode::Rich, Colour::Warn) => "\x1b[33m\x1b[1m[!!]\x1b[0m\x1b[39m",
            (UiMode::Auto, _) => unreachable!("mode resolved at install time"),
        };
        writeln!(output, "{prefix} {message}")
    }
}
#[derive(Clone, Copy)]
enum Colour {
    Info,
    Success,
    Warn,
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
    ui().emit(message, Colour::Info);
}
/// Emit a success status line.
pub fn success(message: impl Display) {
    ui().emit(message, Colour::Success);
}
/// Emit a warning status line.
pub fn warn(message: impl Display) {
    ui().emit(message, Colour::Warn);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_modes_preserve_terminal_policy() {
        assert_eq!(UiMode::Auto.resolve(true), UiMode::Rich);
        assert_eq!(UiMode::Auto.resolve(false), UiMode::Plain);
        assert_eq!(UiMode::Plain.resolve(true), UiMode::Plain);
        assert_eq!(UiMode::Rich.resolve(false), UiMode::Rich);
    }

    #[test]
    fn status_lines_preserve_plain_and_rich_bytes() {
        for (colour, prefix, escape) in [
            (Colour::Info, "[..]", "\x1b[36m"),
            (Colour::Success, "[ok]", "\x1b[32m"),
            (Colour::Warn, "[!!]", "\x1b[33m"),
        ] {
            let mut output = Vec::new();
            Ui::new(UiMode::Plain)
                .write_line(&mut output, format_args!("進捗 {}", 7), colour)
                .unwrap();
            assert_eq!(output, format!("{prefix} 進捗 7\n").as_bytes());
            output.clear();
            Ui::new(UiMode::Rich)
                .write_line(&mut output, format_args!("進捗 {}", 7), colour)
                .unwrap();
            assert_eq!(
                output,
                format!("{escape}\x1b[1m{prefix}\x1b[0m\x1b[39m 進捗 7\n").as_bytes()
            );
        }
    }

    #[test]
    fn rich_status_does_not_add_writer_calls() {
        struct CountWrites(usize);
        impl io::Write for CountWrites {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0 += 1;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        for colour in [Colour::Info, Colour::Success, Colour::Warn] {
            let mut plain = CountWrites(0);
            let mut rich = CountWrites(0);
            Ui::new(UiMode::Plain)
                .write_line(&mut plain, "message", colour)
                .unwrap();
            Ui::new(UiMode::Rich)
                .write_line(&mut rich, "message", colour)
                .unwrap();
            assert_eq!(rich.0, plain.0);
        }
    }
}
