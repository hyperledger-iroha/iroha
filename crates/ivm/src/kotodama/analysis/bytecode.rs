use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use super::{AnalysisCategory, AnalysisFinding, SimpleRng};
use crate::instruction::wide;
use crate::ivm_cache::IvmCache;
use crate::metadata::ProgramMetadata;
use crate::{IVM, VMError, kotodama::std as kstd};
use norito::json as norito_json_mod;

/// Result of analysing a Kotodama bytecode artifact.
#[derive(Debug)]
pub struct BytecodeAnalysis {
    pub metadata: ProgramMetadata,
    pub findings: Vec<AnalysisFinding>,
}

/// Result of executing the bytecode fuzz harness.
#[derive(Debug, Default)]
pub struct BytecodeFuzzReport {
    pub findings: Vec<AnalysisFinding>,
    pub runs: usize,
    pub failures: usize,
}

#[derive(Debug)]
pub enum BytecodeAnalysisError {
    Metadata(VMError),
    Decode(VMError),
    Vm(VMError),
}

impl fmt::Display for BytecodeAnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BytecodeAnalysisError::Metadata(err) => {
                write!(f, "failed to parse program metadata: {err}")
            }
            BytecodeAnalysisError::Decode(err) => write!(f, "bytecode decode failed: {err}"),
            BytecodeAnalysisError::Vm(err) => write!(f, "vm error: {err}"),
        }
    }
}

impl Error for BytecodeAnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            BytecodeAnalysisError::Metadata(err)
            | BytecodeAnalysisError::Decode(err)
            | BytecodeAnalysisError::Vm(err) => Some(err),
        }
    }
}

/// Run static bytecode inspection to surface risky patterns.
pub fn analyze_bytecode(bytes: &[u8]) -> Result<BytecodeAnalysis, BytecodeAnalysisError> {
    let parsed = ProgramMetadata::parse(bytes).map_err(BytecodeAnalysisError::Metadata)?;
    let code = &bytes[parsed.code_offset..];
    let decoded = IvmCache::decode_stream(code).map_err(BytecodeAnalysisError::Decode)?;
    let mut findings = Vec::new();

    let mut arithmetic_seen: HashSet<u8> = HashSet::new();
    let mut last_store_pc: Option<u64> = None;

    for op in decoded.iter() {
        let opcode = wide::opcode(op.inst);
        match opcode {
            wide::arithmetic::MUL
            | wide::arithmetic::MULH
            | wide::arithmetic::MULHU
            | wide::arithmetic::MULHSU
            | wide::arithmetic::DIV
            | wide::arithmetic::DIVU
            | wide::arithmetic::REM
            | wide::arithmetic::REMU => {
                if arithmetic_seen.insert(opcode) {
                    findings.push(AnalysisFinding::warning(
                        AnalysisCategory::BytecodeStatic,
                        "bytecode-arithmetic",
                        format!(
                            "Instruction `{}` at pc=0x{:x} may overflow; ensure inputs are bounded or checked",
                            opcode_name(opcode),
                            op.pc
                        ),
                    ));
                }
            }
            wide::memory::STORE64 | wide::memory::STORE128 => {
                last_store_pc = Some(op.pc);
            }
            wide::system::SCALL => {
                if let Some(store_pc) = last_store_pc
                    && op.pc.saturating_sub(store_pc) <= 64
                {
                    findings.push(AnalysisFinding::warning(
                        AnalysisCategory::BytecodeStatic,
                        "bytecode-reentrancy-risk",
                        format!(
                            "Store at pc=0x{store_pc:x} is followed by SCALL at pc=0x{:x}; review ordering for reentrancy safety",
                            op.pc
                        ),
                    ));
                }
            }
            _ => {}
        }
        if let Some(store_pc) = last_store_pc
            && op.pc.saturating_sub(store_pc) > 256
        {
            last_store_pc = None;
        }
    }

    Ok(BytecodeAnalysis {
        metadata: parsed.metadata,
        findings,
    })
}

/// Execute the bytecode within the IVM multiple times to catch obvious runtime
/// failures. The harness currently runs the program with a clean environment
/// and does not yet randomise contract inputs.
pub fn run_bytecode_fuzz(
    bytes: &[u8],
    iterations: usize,
) -> Result<BytecodeFuzzReport, BytecodeAnalysisError> {
    let parsed = ProgramMetadata::parse(bytes).map_err(BytecodeAnalysisError::Metadata)?;
    let meta = parsed.metadata;
    let mut report = BytecodeFuzzReport::default();
    let iterations = iterations.max(1);
    let mut rng = SimpleRng::new(hash_bytes(bytes));
    let gas_limit = if meta.max_cycles == 0 {
        u64::MAX
    } else {
        meta.max_cycles
    };
    for iteration in 0..iterations {
        let mut vm = IVM::new(gas_limit);
        vm.load_program(bytes).map_err(BytecodeAnalysisError::Vm)?;
        let payload = build_metadata_payload(&meta, iteration, &mut rng);
        let ptr = kstd::input_tlv_norito_bytes(&mut vm, &payload);
        vm.set_register(10, ptr);
        vm.set_register(11, payload.len() as u64);
        match vm.run() {
            Ok(()) => {}
            Err(error) => {
                report.failures += 1;
                report.findings.push(AnalysisFinding::warning(
                    AnalysisCategory::BytecodeFuzz,
                    "bytecode-runtime-failure",
                    format!("bytecode execution trapped: {error:?}"),
                ));
                break;
            }
        }
        report.runs += 1;
    }
    Ok(report)
}

fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        wide::arithmetic::MUL => "MUL",
        wide::arithmetic::MULH => "MULH",
        wide::arithmetic::MULHU => "MULHU",
        wide::arithmetic::MULHSU => "MULHSU",
        wide::arithmetic::DIV => "DIV",
        wide::arithmetic::DIVU => "DIVU",
        wide::arithmetic::REM => "REM",
        wide::arithmetic::REMU => "REMU",
        other => match other {
            wide::memory::STORE64 => "STORE64",
            wide::memory::STORE128 => "STORE128",
            wide::system::SCALL => "SCALL",
            _ => "UNKNOWN",
        },
    }
}

fn build_metadata_payload(
    meta: &ProgramMetadata,
    iteration: usize,
    rng: &mut SimpleRng,
) -> Vec<u8> {
    let seed = rng.next_u64();
    let mut object = norito_json_mod::Map::new();
    object.insert(
        "abi_version".to_owned(),
        norito_json_mod::to_value(&meta.abi_version).unwrap_or(norito_json_mod::Value::Null),
    );
    object.insert(
        "mode".to_owned(),
        norito_json_mod::to_value(&meta.mode).unwrap_or(norito_json_mod::Value::Null),
    );
    object.insert(
        "vector_length_hint".to_owned(),
        norito_json_mod::to_value(&meta.vector_length).unwrap_or(norito_json_mod::Value::Null),
    );
    object.insert(
        "max_cycles".to_owned(),
        norito_json_mod::to_value(&meta.max_cycles).unwrap_or(norito_json_mod::Value::Null),
    );
    object.insert(
        "iteration".to_owned(),
        norito_json_mod::to_value(&iteration).unwrap_or(norito_json_mod::Value::Null),
    );
    object.insert(
        "seed".to_owned(),
        norito_json_mod::to_value(&seed).unwrap_or(norito_json_mod::Value::Null),
    );
    norito_json_mod::to_vec(&norito_json_mod::Value::Object(object)).unwrap_or_default()
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = hasher.finalize();
    let mut seed = [0u8; 8];
    seed.copy_from_slice(&hash[..8]);
    u64::from_le_bytes(seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotodama::compiler::Compiler as KotodamaCompiler;

    #[test]
    fn analyze_detects_arithmetic_instruction() {
        let code = KotodamaCompiler::new()
            .compile_source("fn main() { let x = 1 * 2; }")
            .expect("compile");
        let analysis = analyze_bytecode(&code).expect("analyze");
        assert!(
            analysis
                .findings
                .iter()
                .any(|f| f.code == "bytecode-arithmetic"),
            "expected arithmetic finding, got {analysis:?}"
        );
    }

    #[test]
    fn fuzz_runs_bytecode() {
        let code = KotodamaCompiler::new()
            .compile_source("fn main() { return; }")
            .expect("compile");
        let report = run_bytecode_fuzz(&code, 2).expect("fuzz");
        assert_eq!(report.failures, 0);
        assert!(report.runs >= 1);
    }

    #[test]
    fn metadata_payload_contains_expected_fields() {
        let meta = ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0x01,
            vector_length: 8,
            max_cycles: 1024,
            abi_version: 1,
        };
        let mut rng = SimpleRng::new(42);
        let payload = build_metadata_payload(&meta, 3, &mut rng);
        let value: norito::json::Value =
            norito::json::from_slice(&payload).expect("payload should be valid json");
        let obj = value.as_object().expect("json object");
        assert_eq!(obj.get("abi_version").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(obj.get("mode").and_then(|v| v.as_u64()), Some(0x01));
        assert_eq!(obj.get("iteration").and_then(|v| v.as_u64()), Some(3));
        assert!(obj.get("seed").is_some(), "expected seed field");
    }
}
