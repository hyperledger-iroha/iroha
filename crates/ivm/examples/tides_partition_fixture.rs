//! Compile, execute and microbenchmark the bounded native Tides fixture.
//!
//! This example uses the IVM crate's in-process state shim. Real consensus,
//! deployed authorization, production CoreHost and restart evidence belong to
//! the separate four-validator integration test; this example does not claim them.

#[path = "tides_partition_fixture/mod.rs"]
mod fixture;
#[path = "tides_partition_fixture/hazard.rs"]
mod hazard;
use norito::{json, json::Value};
use std::{collections::BTreeMap, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    if let Some(path) = args.next() {
        let code = fixture::compile_program()?;
        std::fs::write(&path, &code)?;
        println!("Compiled {} bytes to {path}", code.len());
        return Ok(());
    }
    let mut vm = fixture::Fixture::new()?;
    let mut oracle = fixture::Oracle::new();
    let mut samples: BTreeMap<&str, Vec<fixture::Measurement>> = BTreeMap::new();
    for tick in 0..100 {
        for area in ['a', 'b'] {
            let category = match (area, tick <= 60) {
                ('a', true) => "mixed_approach_combat_occupation_capture_and_quiet",
                ('b', true) => "dense_peaceful_economy_step",
                ('a', false) => "dense_quiet_after_export",
                _ => "mixed_approach_combat_and_occupation_after_import",
            };
            let before = oracle.snapshot().clone();
            let observed = vm.execute(&format!("advance_{area}"))?;
            oracle.advance(area)?;
            if vm.snapshot()? != *oracle.snapshot() {
                return Err("VM/oracle divergence".into());
            }
            samples.entry(category).or_default().push(observed);
            let after = oracle.snapshot();
            if ["unit_hp", "guard_hp", "base_hp"]
                .iter()
                .any(|field| after[&format!("{area}_{field}")] < before[&format!("{area}_{field}")])
            {
                samples
                    .entry("actual_unit_or_structure_damage_step")
                    .or_default()
                    .push(observed);
            }
        }
        if tick == 60 {
            let observed = vm.execute("transfer_a_to_b")?;
            oracle.transfer()?;
            if vm.snapshot()? != *oracle.snapshot() {
                return Err("transfer divergence".into());
            }
            samples.entry("atomic_transfer").or_default().push(observed);
        }
    }
    let summary = |v: &[fixture::Measurement]| -> Value {
        let mut walls: Vec<_> = v.iter().map(|s| s.wall_ns).collect();
        walls.sort_unstable();
        json!({"samples": (v.len()), "p50_ns": (walls[walls.len()/2]),
            "p95_ns": (walls[(walls.len()*95).div_ceil(100)-1]),
            "maximum_gas": (v.iter().map(|s| s.gas).max().unwrap_or(0)),
            "maximum_cycles": (v.iter().map(|s| s.cycles).max().unwrap_or(0))})
    };
    let categories: json::Map = samples
        .iter()
        .map(|(name, values)| ((*name).to_owned(), summary(values)))
        .collect();
    println!(
        "{}",
        json::to_json_pretty(&json!({
            "scope": "actual compiled IVM arithmetic plus in-process state shim; NOT native ledger, consensus, production CoreHost, cryptographic proof or million-player capacity",
            "sample_scope": "each call is one dense two-logical-second step, never a multi-tick sparse boundary; mixed categories contain changing phases; actual damage-step category overlaps them and is a small sample, not sustained-combat throughput",
            "program_bytes": (vm.code.len()), "categories": (Value::Object(categories)),
            "all_33_state_fields_match_oracle_after_each_call": true,
            "hazard_256_entity_component": (hazard::benchmark())
        }))?
    );
    Ok(())
}
