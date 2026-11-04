//! Proves and verifies one SORA CARS RaceV1 race and prints exactly what ran.
//!
//! `cargo run --release --example race_bench -- <NeonTokyo|Harbor|Sakura> <players 1-8>
//! <throttle-percent 0-100> <seed> [lookups-per-helper 1-4]`
//!
//! Every argument is checked; anything unknown exits with status 2 instead of falling back to a
//! default. `IROHA_EXECUTION_STARK_TIMINGS=1` adds per-phase wall/CPU times on stderr.

use std::process::ExitCode;
use std::time::Instant;

use iroha_execution_stark::games::race_v1::{self, Track};
use iroha_execution_stark::params::SecurityShape;
use iroha_execution_stark::{Params, circuit_id, prove, verify};

const USAGE: &str = "usage: race_bench <NeonTokyo|Harbor|Sakura> <players 1-8> <throttle-percent 0-100> <seed> [lookups-per-helper 1-4]";

struct Args {
    track: Track,
    players: usize,
    throttle: u64,
    seed: u64,
    lookups_per_helper: Option<usize>,
}

fn parse() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        return Err(format!("expected 4 or 5 arguments, got {}", args.len()));
    }
    let track = match args[0].as_str() {
        "NeonTokyo" => Track::NeonTokyo,
        "Harbor" => Track::Harbor,
        "Sakura" => Track::Sakura,
        other => return Err(format!("unknown track {other:?}")),
    };
    let number = |index: usize, name: &str, range: std::ops::RangeInclusive<u64>| {
        let value: u64 = args[index]
            .parse()
            .map_err(|_| format!("{name} {:?} is not a number", args[index]))?;
        if range.contains(&value) {
            Ok(value)
        } else {
            Err(format!("{name} {value} is outside {range:?}"))
        }
    };
    let players = usize::try_from(number(1, "players", 1..=8)?).expect("small");
    let throttle = number(2, "throttle", 0..=100)?;
    let seed = number(3, "seed", 0..=u64::MAX)?;
    let lookups_per_helper = if args.len() == 5 {
        Some(usize::try_from(number(4, "lookups-per-helper", 1..=4)?).expect("small"))
    } else {
        None
    };
    Ok(Args {
        track,
        players,
        throttle,
        seed,
        lookups_per_helper,
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        write!(out, "{b:02x}").expect("string write");
        out
    })
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("race_bench: {error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let executable = std::env::current_exe().ok();
    let executable_hash = executable
        .as_ref()
        .and_then(|path| std::fs::read(path).ok())
        .map_or_else(
            || "unavailable".to_owned(),
            |bytes| blake3::hash(&bytes).to_hex().to_string(),
        );
    let replay = race_v1::sample_replay(
        args.track,
        args.players,
        args.seed,
        args.throttle,
        race_v1::MAX_TICKS,
    );
    let mut circuit = race_v1::circuit(args.track, args.players);
    if let Some(per) = args.lookups_per_helper {
        circuit.lookups_per_helper = per;
    }
    let public = race_v1::public_columns(&replay).expect("valid replay");
    let states = race_v1::register_states(&replay).expect("valid replay");
    let statement = race_v1::statement(&replay).expect("valid replay");
    let params = Params::standard();
    let shape = SecurityShape::of(&circuit);
    println!(
        "binary {} blake3 {executable_hash}",
        executable.map_or_else(|| "unknown".to_owned(), |p| p.display().to_string())
    );
    println!(
        "args track={:?} players={} throttle={} seed={} lookups_per_helper={}",
        args.track, args.players, args.throttle, args.seed, circuit.lookups_per_helper
    );
    println!("rayon threads {}", rayon::current_num_threads());
    println!("{circuit}");
    println!("circuit id {}", hex(&circuit_id(&circuit)));
    println!(
        "replay ticks={} removals={:?} finishers={} digest={}",
        replay.frames.len(),
        replay.dnf_events,
        statement
            .final_state
            .chunks(race_v1::REGISTERS_PER_CAR)
            .filter(|c| c[5] == 1)
            .count(),
        hex(&race_v1::replay_binding(&replay))
    );
    println!(
        "params {params:?} reported soundness budget {:.1} bits (rounds sized for 129), hash-capped {:.1}, size bound {} bytes",
        params.proven_security_bits(&shape),
        params.hash_limited_security_bits(&shape),
        iroha_execution_stark::max_proof_bytes(&circuit, &params).expect("standard parameters")
    );
    let started = Instant::now();
    let proof = match prove(&circuit, &params, &statement, &public, &states) {
        Ok(proof) => proof,
        Err(error) => {
            eprintln!("prove failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let prove_seconds = started.elapsed().as_secs_f64();
    let bytes = proof.to_bytes();
    let started = Instant::now();
    if let Err(error) = verify(&circuit, &params, &statement, &public, &proof) {
        eprintln!("verify failed: {error}");
        return ExitCode::FAILURE;
    }
    let verify_seconds = started.elapsed().as_secs_f64();
    println!(
        "prove {prove_seconds:.3} s (prove only), verify {:.1} ms, proof {} bytes, proof blake3 {}",
        verify_seconds * 1e3,
        bytes.len(),
        blake3::hash(&bytes).to_hex()
    );
    ExitCode::SUCCESS
}
