#![deny(unsafe_code)]

#[path = "../privacy_native_actions.rs"]
mod privacy_native_actions;
#[path = "../privacy_wallet_bundle.rs"]
mod privacy_wallet_bundle;
#[path = "../privacy_wallet_worker.rs"]
mod privacy_wallet_worker;

use std::io::{self, BufReader, BufWriter, Read};

use privacy_wallet_worker::run_pipe_session;
use zeroize::Zeroizing;

#[cfg(all(unix, not(target_os = "haiku")))]
fn harden_process() -> Result<(), ()> {
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|_| ())?;
    #[cfg(target_os = "linux")]
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
        .map_err(|_| ())?;
    Ok(())
}

#[cfg(any(not(unix), target_os = "haiku"))]
fn harden_process() -> Result<(), ()> {
    Err(())
}

fn main() {
    if harden_process().is_err() {
        eprintln!("privacy wallet worker startup hardening failed");
        std::process::exit(63);
    }
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = BufWriter::new(io::stdout().lock());
    let mut auth_key = Zeroizing::new([0_u8; 32]);
    if input.read_exact(&mut auth_key[..]).is_err() || auth_key.iter().all(|byte| *byte == 0) {
        eprintln!("privacy wallet worker startup failed: missing authentication key");
        std::process::exit(64);
    }
    if let Err(error) = run_pipe_session(&mut input, &mut output, auth_key) {
        eprintln!("privacy wallet worker terminated: {}", error.message());
        std::process::exit(70);
    }
}
