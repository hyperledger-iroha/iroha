use std::{collections::HashSet, error::Error, fmt};

use super::{AnalysisCategory, AnalysisFinding};
use crate::{VMError, instruction::wide, metadata::ProgramMetadata};

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
    if !code.len().is_multiple_of(4) {
        return Err(BytecodeAnalysisError::Decode(VMError::DecodeError));
    }
    let mut findings = Vec::new();

    let mut arithmetic_seen: HashSet<u8> = HashSet::new();
    let mut last_store_pc: Option<u64> = None;

    for (idx, chunk) in code.chunks_exact(4).enumerate() {
        let word = u32::from_le_bytes(chunk.try_into().unwrap());
        let pc = (idx as u64).saturating_mul(4);
        let opcode = wide::opcode(word);
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
                            pc
                        ),
                    ));
                }
            }
            wide::memory::STORE64 | wide::memory::STORE128 => {
                last_store_pc = Some(pc);
            }
            wide::system::SCALL => {
                if let Some(store_pc) = last_store_pc
                    && pc.saturating_sub(store_pc) <= 64
                {
                    findings.push(AnalysisFinding::warning(
                        AnalysisCategory::BytecodeStatic,
                        "bytecode-reentrancy-risk",
                        format!(
                            "Store at pc=0x{store_pc:x} is followed by SCALL at pc=0x{:x}; review ordering for reentrancy safety",
                            pc
                        ),
                    ));
                }
            }
            _ => {}
        }
        if let Some(store_pc) = last_store_pc
            && pc.saturating_sub(store_pc) > 256
        {
            last_store_pc = None;
        }
    }

    Ok(BytecodeAnalysis {
        metadata: parsed.metadata,
        findings,
    })
}

/// Execute the bytecode within a runtime harness when available.
///
/// The standalone `kotodama_lang` crate does not bundle an IVM runtime, so this
/// function reports that runtime fuzzing is disabled.
pub fn run_bytecode_fuzz(
    bytes: &[u8],
    iterations: usize,
) -> Result<BytecodeFuzzReport, BytecodeAnalysisError> {
    let _ = iterations;
    let _ = ProgramMetadata::parse(bytes).map_err(BytecodeAnalysisError::Metadata)?;
    let mut report = BytecodeFuzzReport::default();
    report.findings.push(AnalysisFinding::info(
        AnalysisCategory::BytecodeFuzz,
        "bytecode-fuzz-disabled",
        "bytecode runtime fuzzing is not available in kotodama_lang builds",
    ));
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

#[cfg(test)]
mod tests {
    use super::super::SimpleRng;
    use super::*;
    use crate::compiler::Compiler as KotodamaCompiler;
    use norito::json as norito_json_mod;

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
    fn fuzz_reports_disabled_runtime() {
        let code = KotodamaCompiler::new()
            .compile_source("fn main() { return; }")
            .expect("compile");
        let report = run_bytecode_fuzz(&code, 2).expect("fuzz");
        assert_eq!(report.failures, 0);
        assert_eq!(report.runs, 0);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "bytecode-fuzz-disabled"),
            "expected disabled runtime finding"
        );
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
