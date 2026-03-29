use std::{
    env, fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "sora-vpn-controller")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// Ignored compatibility flag used by the Electron wrapper.
    #[arg(long)]
    json: bool,
    /// Optional JSON payload consumed by `connect`.
    payload: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    InstallCheck,
    Status,
    Connect,
    Disconnect,
    Repair,
}

#[derive(Debug, Clone)]
struct State {
    installed: bool,
    active: bool,
    controller_kind: &'static str,
    interface_name: Option<String>,
    network_service: Option<String>,
    version: &'static str,
    controller_path: Option<String>,
    repair_required: bool,
    bytes_in: u64,
    bytes_out: u64,
    message: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            installed: true,
            active: false,
            controller_kind: "linux-helperd",
            interface_name: env::var("SORANET_VPN_INTERFACE").ok(),
            network_service: None,
            version: "0.1.0",
            controller_path: env::args().next(),
            repair_required: false,
            bytes_in: 0,
            bytes_out: 0,
            message: "ready".to_owned(),
        }
    }
}

fn state_path() -> PathBuf {
    env::var("SORANET_VPN_STATE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/sora-vpn-controller-state.json"))
}

fn load_state() -> State {
    let path = state_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return State::default();
    };
    parse_state(&raw).unwrap_or_default()
}

fn parse_bool(raw: &str) -> bool {
    matches!(raw.trim(), "true" | "1")
}

fn parse_state(raw: &str) -> Option<State> {
    let mut state = State::default();
    for line in raw.lines() {
        let (key, value) = line.split_once('=')?;
        match key {
            "installed" => state.installed = parse_bool(value),
            "active" => state.active = parse_bool(value),
            "interface_name" => {
                state.interface_name = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                }
            }
            "network_service" => {
                state.network_service = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                }
            }
            "repair_required" => state.repair_required = parse_bool(value),
            "bytes_in" => state.bytes_in = value.parse().ok()?,
            "bytes_out" => state.bytes_out = value.parse().ok()?,
            "message" => state.message = value.to_owned(),
            _ => {}
        }
    }
    Some(state)
}

fn persist_state(state: &State) {
    let path = state_path();
    if let Some(parent) = Path::new(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = format!(
        "installed={}\nactive={}\ninterface_name={}\nnetwork_service={}\nrepair_required={}\nbytes_in={}\nbytes_out={}\nmessage={}\n",
        state.installed,
        state.active,
        state.interface_name.clone().unwrap_or_default(),
        state.network_service.clone().unwrap_or_default(),
        state.repair_required,
        state.bytes_in,
        state.bytes_out,
        state.message,
    );
    let _ = fs::write(path, body);
}

fn print_state(state: &State) {
    let interface_name = state
        .interface_name
        .as_deref()
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_owned());
    let network_service = state
        .network_service
        .as_deref()
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_owned());
    let controller_path = state
        .controller_path
        .as_deref()
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_owned());

    println!(
        "{{\"installed\":{},\"active\":{},\"controller_kind\":\"{}\",\"interface_name\":{},\"network_service\":{},\"version\":\"{}\",\"controller_path\":{},\"repair_required\":{},\"bytes_in\":{},\"bytes_out\":{},\"message\":\"{}\"}}",
        state.installed,
        state.active,
        state.controller_kind,
        interface_name,
        network_service,
        state.version,
        controller_path,
        state.repair_required,
        state.bytes_in,
        state.bytes_out,
        state.message.replace('"', "\\\""),
    );
}

fn main() {
    let cli = Cli::parse();
    let mut state = load_state();

    match cli.command {
        Command::InstallCheck => {
            state.message = if state.repair_required {
                "repair required".to_owned()
            } else {
                "ready".to_owned()
            };
        }
        Command::Status => {}
        Command::Connect => {
            state.active = true;
            state.message = cli
                .payload
                .as_deref()
                .map(|payload| format!("connected {}", payload.len()))
                .unwrap_or_else(|| "connected".to_owned());
        }
        Command::Disconnect => {
            state.active = false;
            state.message = "idle".to_owned();
        }
        Command::Repair => {
            state.repair_required = false;
            state.message = "repaired".to_owned();
        }
    }

    persist_state(&state);
    print_state(&state);
}
