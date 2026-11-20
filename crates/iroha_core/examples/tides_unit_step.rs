//! Measure the movement-only IVM adapter used by the independent-root prototype.
//!
//! Includes a fresh VM/load per call. Excludes native state I/O, authorization,
//! consensus, transfer certificates, restart recovery and cryptographic proofs.

use std::{error::Error, hint::black_box, time::Instant};

use iroha_core::smartcontracts::isi::game_partition::unit_step::{
    UnitStepInput, kernel_hash, step_unit,
};
use norito::{json, json::Value};

fn kernel_hash_hex() -> String {
    hex::encode(kernel_hash())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut timings = Vec::with_capacity(100);
    let mut maximum_gas = 0;
    let mut maximum_cycles = 0;
    for x in 0..100 {
        let input = UnitStepInput { hp: 100, x };
        let start = Instant::now();
        let output = step_unit(black_box(input))?;
        timings.push(start.elapsed().as_nanos() as u64);
        if output.hp != 100 || output.x != x + 1 {
            return Err("IVM movement differs from independent oracle".into());
        }
        maximum_gas = maximum_gas.max(output.gas);
        maximum_cycles = maximum_cycles.max(output.cycles);
    }
    timings.sort_unstable();
    let report: Value = json!({
        "scope": "fixed existing-ISA movement kernel only; fresh VM/load included; no state I/O, consensus, transfer or proof throughput claim",
        "samples": (timings.len()),
        "kernel_hash": (kernel_hash_hex()),
        "p50_ns": (timings[50]),
        "p95_ns": (timings[94]),
        "maximum_gas": maximum_gas,
        "maximum_cycles": maximum_cycles,
        "all_outputs_match_oracle": true
    });
    println!("{}", json::to_json_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_preserves_the_fixed_kernel_pin() {
        assert_eq!(
            kernel_hash_hex(),
            "948a6cd6ba749249e6406528b90f982a2bcfbbbfdeee9334c4bc42dbf96aaccb"
        );
    }
}
