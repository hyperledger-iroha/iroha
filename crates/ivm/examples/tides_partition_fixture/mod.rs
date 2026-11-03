//! Bounded two-area prototype and an independent integer oracle.
//!
//! The compiler emits all transition arithmetic. This module's Rust oracle is
//! only an assertion after execution, never a host implementation of gameplay.

use std::{collections::BTreeMap, time::Instant};

use ivm::{CoreHost, IVM, IVMHost, ProgramMetadata, VMError, parallel::StateAccessSet};
use ivm_abi::state_value::{
    StateValueAtomV1, StateValueKindV1, StateValueNodeV1, StateValueRecordV1, StateValueSchemaV1,
    state_value_schema_hash_v1,
};
use kotodama_lang::compiler::Compiler;

/// Compiled by the integration fixture as well as this example.
pub const SOURCE: &str = include_str!("contract.ko");
/// Diagnostic per-call budget; the network must apply its own real admission.
pub const GAS_LIMIT: u64 = 20_000_000;
/// Fields in each independent logical area namespace.
pub const AREA_FIELDS: [&str; 16] = [
    "tick",
    "stock",
    "deposit",
    "cargo",
    "upkeep_spent",
    "budget",
    "worker_phase",
    "unit_hp",
    "unit_x",
    "guard_hp",
    "base_hp",
    "capture_hold",
    "owner",
    "bond",
    "capture_paid",
    "unit_present",
];

/// Compile the actual source, returning its canonical artifact.
pub fn compile_program() -> Result<Vec<u8>, String> {
    Compiler::new().compile_source(SOURCE)
}

/// Select exactly the public wrapper advertised by the artifact.
pub fn select_entrypoint(vm: &mut IVM, code: &[u8], name: &str) -> Result<(), String> {
    let metadata = ProgramMetadata::parse(code).map_err(|e| format!("metadata: {e:?}"))?;
    let interface = metadata
        .contract_interface
        .as_ref()
        .ok_or("missing interface")?;
    let entry = interface
        .entrypoints
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("missing entrypoint {name}"))?;
    let prefix = u64::try_from(metadata.prefix_len()).map_err(|e| e.to_string())?;
    vm.set_program_counter(prefix + entry.entry_pc)
        .map_err(|e| format!("entrypoint: {e:?}"))
}

/// Validate the schema and decode a small signed fixture integer.
pub fn decode_state_int(bytes: &[u8]) -> Result<i64, String> {
    let schema = StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
    };
    let schema_bytes = norito::to_bytes(&schema).map_err(|e| e.to_string())?;
    let record: StateValueRecordV1 = norito::decode_from_bytes(bytes).map_err(|e| e.to_string())?;
    if record.schema_hash != state_value_schema_hash_v1(&schema_bytes)
        || !schema.validate_atoms(&record.atoms)
    {
        return Err("invalid int state schema".into());
    }
    let [StateValueAtomV1::Pointer(envelope)] = record.atoms.as_slice() else {
        return Err("expected one int pointer".into());
    };
    ivm::numeric_tlv::decode_int_bytes(envelope)
        .map_err(|e| format!("int: {e:?}"))?
        .try_to_i64()
        .ok_or_else(|| "fixture int does not fit i64".into())
}

/// Independent scalar oracle for the limited demonstration rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Oracle {
    values: BTreeMap<String, i64>,
}

impl Default for Oracle {
    fn default() -> Self {
        Self::new()
    }
}

impl Oracle {
    /// Construct the same initial world without executing any bytecode.
    pub fn new() -> Self {
        let mut values = BTreeMap::new();
        let initial = [0, 100, 1000, 0, 0, 128, 0, 100, 0, 12, 16, 0, 0, 100, 0, 1];
        for area in ['a', 'b'] {
            for (name, value) in AREA_FIELDS.into_iter().zip(initial) {
                let value = if area == 'b' && matches!(name, "unit_hp" | "unit_present") {
                    0
                } else {
                    value
                };
                values.insert(format!("{area}_{name}"), value);
            }
        }
        values.insert("transfer_nonce".into(), 0);
        Self { values }
    }

    /// Return every canonical scalar for exhaustive VM comparison.
    pub fn snapshot(&self) -> &BTreeMap<String, i64> {
        &self.values
    }

    /// Advance one two-logical-second tick in an area, without host state.
    pub fn advance(&mut self, area: char) -> Result<(), String> {
        if !matches!(area, 'a' | 'b') {
            return Err("unknown area".into());
        }
        let key = |field: &str| format!("{area}_{field}");
        let mut v: Vec<i64> = AREA_FIELDS
            .iter()
            .map(|name| self.values[&key(name)])
            .collect();
        // Explicit indexed oracle uses only bounded native integers and models
        // simultaneous fire from the old HP pair before either damage write.
        if v[5] == 0 {
            return Ok(());
        }
        v[5] -= 1;
        v[0] += 1;
        v[6] = (v[6] + 1) % 4;
        if v[6] == 2 {
            let loaded = v[2].min(4);
            v[2] -= loaded;
            v[3] += loaded;
        }
        if v[6] == 0 {
            let delivered = v[3].min(1000 - v[1]);
            v[1] += delivered;
            v[3] -= delivered;
        }
        if v[1] > 0 {
            v[1] -= 1;
            v[4] += 1;
        }
        if v[15] == 1 && v[7] > 0 {
            v[8] = (v[8] + 2).min(16);
            if v[8] == 16 {
                let old_guard = v[9];
                let old_unit = v[7];
                if old_guard > 0 && old_unit > 0 {
                    v[9] = (old_guard - 4).max(0);
                    v[7] = old_unit - 1;
                }
                if old_guard == 0 && old_unit > 0 {
                    v[10] = (v[10] - 4).max(0);
                }
            }
        }
        if v[15] == 1 && v[7] > 0 && v[9] == 0 && v[10] == 0 && v[1] > 0 && v[12] == 0 {
            v[11] += 2;
            if v[11] >= 60 {
                let share = v[13] / 5;
                v[13] -= share;
                v[14] += share;
                v[12] = 1;
            }
        } else if v[12] == 0 {
            v[11] = 0;
        }
        for (name, value) in AREA_FIELDS.into_iter().zip(v) {
            self.values.insert(key(name), value);
        }
        self.check_conservation()
    }

    /// Move the one live mobile object exactly once, in one state transaction.
    pub fn transfer(&mut self) -> Result<(), String> {
        if self.values["transfer_nonce"] != 0 {
            return Err("AlreadyTransferred".into());
        }
        if self.values["a_unit_present"] != 1 || self.values["a_unit_hp"] <= 0 {
            return Err("MissingSource".into());
        }
        if self.values["b_unit_present"] != 0 {
            return Err("OccupiedDestination".into());
        }
        let hp = self.values["a_unit_hp"];
        for (key, value) in [
            ("a_unit_present", 0),
            ("b_unit_present", 1),
            ("b_unit_hp", hp),
            ("b_unit_x", 0),
            ("a_unit_hp", 0),
            ("transfer_nonce", 1),
        ] {
            self.values.insert(key.into(), value);
        }
        self.check_conservation()
    }

    /// Check source-stock/cargo/storage/upkeep and collateral accounting totals.
    pub fn check_conservation(&self) -> Result<(), String> {
        for area in ['a', 'b'] {
            let get = |name: &str| self.values[&format!("{area}_{name}")];
            if get("deposit") + get("cargo") + get("stock") + get("upkeep_spent") != 1100 {
                return Err(format!("{area}: resources not conserved"));
            }
            if get("bond") + get("capture_paid") != 100 {
                return Err(format!("{area}: collateral counters not conserved"));
            }
            if get("stock") > 1000 || get("budget") < 0 || get("tick") + get("budget") != 128 {
                return Err(format!("{area}: storage or work bound violated"));
            }
        }
        if self.values.values().any(|v| *v < 0)
            || self.values["a_unit_present"] + self.values["b_unit_present"] != 1
        {
            return Err("negative field or duplicated mobile unit".into());
        }
        Ok(())
    }
}

/// One real IVM execution measurement, excluding oracle checks.
#[derive(Clone, Copy, Debug)]
pub struct Measurement {
    /// Actual gas consumed by this call.
    pub gas: u64,
    /// Actual IVM instruction cycles for this call.
    pub cycles: u64,
    /// Wall nanoseconds including VM construction, loading, state and rollback.
    pub wall_ns: u64,
}

/// In-process IVM shim for parity/microbenchmarks, not a deployed native ledger.
pub struct Fixture {
    /// Actual compiler-produced code reused by the network integration test.
    pub code: Vec<u8>,
    host: CoreHost,
}

impl Fixture {
    /// Compile and initialize through the actual `hajimari` bytecode.
    pub fn new() -> Result<Self, String> {
        let mut this = Self {
            code: compile_program()?,
            host: CoreHost::new(),
        };
        this.execute("hajimari")?;
        Ok(this)
    }

    /// Execute a named wrapper and restore transactional host state on failure.
    pub fn execute(&mut self, name: &str) -> Result<Measurement, String> {
        let now = Instant::now();
        let saved = self.host.checkpoint().ok_or("host cannot roll back")?;
        self.host
            .begin_tx(&StateAccessSet::default())
            .map_err(|e| format!("begin: {e:?}"))?;
        let mut vm = IVM::new(GAS_LIMIT);
        vm.load_program(&self.code)
            .map_err(|e| format!("load: {e:?}"))?;
        select_entrypoint(&mut vm, &self.code, name)?;
        let result: Result<(), VMError> = vm.run_with_host(&mut self.host);
        if let Err(error) = result {
            self.host
                .restore(saved.as_ref())
                .map_err(|e| format!("restore: {e:?}"))?;
            return Err(format!("{name}: {error:?}"));
        }
        self.host
            .finish_tx()
            .map_err(|e| format!("finish: {e:?}"))?;
        Ok(Measurement {
            gas: GAS_LIMIT - vm.remaining_gas(),
            cycles: vm.get_cycle_count(),
            wall_ns: u64::try_from(now.elapsed().as_nanos()).map_err(|e| e.to_string())?,
        })
    }

    /// Read and decode all 33 durable fields for exact oracle comparison.
    pub fn snapshot(&self) -> Result<BTreeMap<String, i64>, String> {
        Oracle::new()
            .snapshot()
            .keys()
            .map(|key| {
                let bytes = self
                    .host
                    .state_bytes(key)
                    .ok_or_else(|| format!("missing {key}"))?;
                Ok((key.clone(), decode_state_int(&bytes)?))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_exposes_two_scoped_tick_wrappers_and_no_import_minter() {
        let code = compile_program().unwrap();
        let metadata = ProgramMetadata::parse(&code).unwrap();
        let interface = metadata.contract_interface.unwrap();
        for name in [
            "hajimari",
            "advance_a",
            "advance_b",
            "transfer_a_to_b",
            "reject_transfer_a_to_b",
        ] {
            assert!(
                interface.entrypoints.iter().any(|e| e.name == name),
                "{name}"
            );
        }
        assert!(
            !interface
                .entrypoints
                .iter()
                .any(|e| e.name == "import_unit")
        );
        assert_eq!(interface.states.len(), 33);
    }

    #[test]
    fn complete_scalar_state_matches_independent_oracle_at_every_tick() {
        let mut vm = Fixture::new().unwrap();
        let mut oracle = Oracle::new();
        assert_eq!(&vm.snapshot().unwrap(), oracle.snapshot());
        for _ in 0..128 {
            for area in ['a', 'b'] {
                vm.execute(&format!("advance_{area}")).unwrap();
                oracle.advance(area).unwrap();
                assert_eq!(&vm.snapshot().unwrap(), oracle.snapshot());
            }
        }
        assert_eq!(oracle.snapshot()["a_capture_paid"], 20);
        assert_eq!(oracle.snapshot()["b_capture_paid"], 0);
    }

    #[test]
    fn capture_requires_full_hold_and_preserves_eighty_percent() {
        let mut oracle = Oracle::new();
        for _ in 0..42 {
            oracle.advance('a').unwrap();
        }
        assert_eq!(oracle.snapshot()["a_capture_paid"], 0);
        oracle.advance('a').unwrap();
        assert_eq!(oracle.snapshot()["a_capture_hold"], 60);
        assert_eq!(oracle.snapshot()["a_capture_paid"], 20);
        assert_eq!(oracle.snapshot()["a_bond"], 80);
        for _ in 0..70 {
            oracle.advance('a').unwrap();
        }
        assert_eq!(oracle.snapshot()["a_capture_paid"], 20);
    }

    #[test]
    fn both_transfer_legs_and_nonce_match_the_vm() {
        let mut vm = Fixture::new().unwrap();
        let mut oracle = Oracle::new();
        for _ in 0..12 {
            vm.execute("advance_a").unwrap();
            oracle.advance('a').unwrap();
        }
        vm.execute("transfer_a_to_b").unwrap();
        oracle.transfer().unwrap();
        assert_eq!(&vm.snapshot().unwrap(), oracle.snapshot());
        for _ in 0..43 {
            vm.execute("advance_b").unwrap();
            oracle.advance('b').unwrap();
        }
        assert_eq!(&vm.snapshot().unwrap(), oracle.snapshot());
        assert_eq!(oracle.snapshot()["b_capture_paid"], 20);
        let before = vm.snapshot().unwrap();
        assert!(vm.execute("transfer_a_to_b").is_err());
        assert!(oracle.transfer().is_err());
        assert_eq!(vm.snapshot().unwrap(), before);
    }

    #[test]
    fn rejected_destination_rolls_back_source_debit() {
        let mut vm = Fixture::new().unwrap();
        let before = vm.snapshot().unwrap();
        assert!(vm.execute("reject_transfer_a_to_b").is_err());
        assert_eq!(vm.snapshot().unwrap(), before);
        vm.execute("transfer_a_to_b").unwrap();
        assert_eq!(vm.snapshot().unwrap()["transfer_nonce"], 1);
    }

    #[test]
    fn exhaustion_never_charges_or_advances_again() {
        let mut vm = Fixture::new().unwrap();
        for _ in 0..128 {
            vm.execute("advance_a").unwrap();
        }
        let before = vm.snapshot().unwrap();
        for _ in 0..4 {
            vm.execute("advance_a").unwrap();
        }
        assert_eq!(vm.snapshot().unwrap(), before);
    }

    #[test]
    fn bad_state_schema_is_rejected() {
        assert!(decode_state_int(&[]).is_err());
        assert!(decode_state_int(b"not a canonical state record").is_err());
    }

    #[test]
    fn unknown_area_does_not_change_oracle() {
        let mut oracle = Oracle::new();
        let before = oracle.clone();
        assert!(oracle.advance('x').is_err());
        assert_eq!(oracle, before);
    }
}
