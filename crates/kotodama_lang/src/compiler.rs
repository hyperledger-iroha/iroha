//! Compiler for the KOTODAMA language.
//!
//! This module implements a practical, growing compiler from Kotodama source
//! into IVM bytecode (`.to`). It performs parsing, a lightweight semantic pass,
//! IR lowering, simple register allocation, and final code generation with an
//! IVM metadata header. The compiler exposes options to control header fields
//! such as `abi_version`, `vector_length`, and `max_cycles`.
//!
//! Kotodama targets the IVM bytecode format exclusively. All helpers in this
//! module emit the canonical wide encoding introduced for the first release; no
//! alternate instruction layouts are generated.

use std::collections::{BTreeSet, HashMap};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use indexmap::IndexSet;
use iroha_crypto as _; // for Hash types in new APIs
use iroha_data_model::{
    Identifiable,
    account::AccountId,
    asset::id::{AssetDefinitionId, AssetId},
    domain::DomainId,
    isi::{
        BuiltInInstruction as _, BurnBox, ExecuteTrigger, GrantBox, InstructionBox, Log, MintBox,
        RegisterBox, RemoveKeyValueBox, RevokeBox, SetKeyValueBox, TransferBox, UnregisterBox,
    },
    name::Name,
    nft::NftId,
    query::{QueryRequest, SingularQueryBox},
    role::RoleId,
    smart_contract::manifest::{
        AccessSetHints, EntryPointKind, EntrypointParamDescriptor, TriggerCallback,
        TriggerDescriptor,
    },
    trigger::{Trigger, TriggerId},
};
use norito::json;

use super::{
    ast::{
        BinaryOp, ContractFeature, ContractMeta, FunctionKind, FunctionModifiers,
        FunctionVisibility, Program, UnaryOp,
    },
    i18n::{self, Language, Message},
    ir::{self, Instr, Terminator},
    parser, policy, regalloc,
    semantic::{self, TypedItem, TypedProgram},
};
use crate::{
    encoding, instruction,
    metadata::{
        self, CONTRACT_FEATURE_BIT_VECTOR, CONTRACT_FEATURE_BIT_ZK, EmbeddedContractDebugInfoV1,
        EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, EmbeddedFunctionBudgetReportV1,
        EmbeddedSourceLocation, EmbeddedSourceMapEntryV1, LITERAL_SECTION_MAGIC, ProgramMetadata,
    },
    pointer_abi::PointerType,
    syscalls,
};

const WIDE_IMM_MIN: i32 = -128;
const WIDE_IMM_MAX: i32 = 127;
const POINTER_STUB_LEN: usize = 24;
const CONTROL_TRANSFER_STUB_WORDS: usize = POINTER_STUB_LEN + 1;
const CONTROL_TRANSFER_SCRATCH_REG: u8 = regalloc::FP_REG as u8;
const LITERAL_SHIFT_REG: u8 = 26;
const DEFAULT_MAX_CYCLES: u64 = 1_000_000;
const GLOBAL_WILDCARD_KEY: &str = "*";
const STATE_WILDCARD_KEY: &str = "state:*";
const TRIGGER_EVENT_PUBLIC_INPUT_KEY: &str = "trigger_event_json";
const COMPILER_FINGERPRINT: &str = concat!("kotodama_lang/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
struct AccessSets {
    reads: IndexSet<String>,
    writes: IndexSet<String>,
}

impl Default for AccessSets {
    fn default() -> Self {
        Self {
            reads: IndexSet::new(),
            writes: IndexSet::new(),
        }
    }
}

#[derive(Clone)]
enum StatePathHint {
    Literal(String),
    Map { base: String },
}

impl StatePathHint {
    fn base_name(&self) -> String {
        match self {
            StatePathHint::Literal(name) => name.clone(),
            StatePathHint::Map { base } => base.clone(),
        }
    }
}

struct CompilationArtifacts {
    bytes: Vec<u8>,
    compile_report: CompileReport,
}

#[derive(Clone)]
struct FunctionDebugSeed {
    name: String,
    location: super::ast::SourceLocation,
    pc_start: u64,
    frame_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileReport {
    pub source_map: Vec<EmbeddedSourceMapEntryV1>,
    pub budget_report: Vec<EmbeddedFunctionBudgetReportV1>,
    pub access_hint_diagnostics: AccessHintDiagnostics,
}

/// Diagnostics emitted when access hints cannot be fully derived.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessHintDiagnostics {
    /// Number of state accesses that could not be resolved to literal/map hints.
    pub state_wildcards: usize,
    /// Number of ISI instructions that could not be resolved to concrete hints.
    pub isi_wildcards: usize,
}

impl AccessHintDiagnostics {
    /// Whether any access-hint fallback occurred.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state_wildcards == 0 && self.isi_wildcards == 0
    }
}

struct HintReport {
    emitted: bool,
    skipped_reasons: Vec<String>,
}

fn push_word(code: &mut Vec<u8>, word: u32) {
    code.extend_from_slice(&word.to_le_bytes());
}

fn chunk_immediate(value: i64) -> i8 {
    if value > WIDE_IMM_MAX as i64 {
        WIDE_IMM_MAX as i8
    } else if value < WIDE_IMM_MIN as i64 {
        WIDE_IMM_MIN as i8
    } else {
        value as i8
    }
}

fn emit_addi_inplace(code: &mut Vec<u8>, reg: u8, mut value: i64) {
    while value != 0 {
        let chunk = chunk_immediate(value);
        push_word(
            code,
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, reg, reg, chunk),
        );
        value -= chunk as i64;
    }
}

fn emit_addi(code: &mut Vec<u8>, rd: u8, rs1: u8, value: i64) {
    if rd != rs1 {
        push_word(
            code,
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, rd, rs1, 0),
        );
    }
    if value != 0 {
        emit_addi_inplace(code, rd, value);
    }
}

fn emit_load64(
    code: &mut Vec<u8>,
    rd: u8,
    base: u8,
    offset: i64,
    scratch: Option<u8>,
) -> Result<(), String> {
    if rd != base && ((WIDE_IMM_MIN as i64)..=(WIDE_IMM_MAX as i64)).contains(&offset) {
        push_word(code, encode_load64_rv(rd, base, offset as i16)?);
        return Ok(());
    }
    let addr_reg = if rd == base {
        scratch.ok_or_else(|| {
            format!("emit_load64 requires scratch when rd == base for offset {offset}")
        })?
    } else {
        rd
    };
    if addr_reg != base {
        push_word(code, encode_addi(addr_reg, base, 0)?);
    }
    emit_addi_inplace(code, addr_reg, offset);
    push_word(code, encode_load64_rv(rd, addr_reg, 0)?);
    Ok(())
}

fn emit_store64(
    code: &mut Vec<u8>,
    base: u8,
    rs: u8,
    offset: i64,
    scratch: u8,
) -> Result<(), String> {
    if ((WIDE_IMM_MIN as i64)..=(WIDE_IMM_MAX as i64)).contains(&offset) {
        push_word(code, encode_store64_rv(base, rs, offset as i16)?);
        return Ok(());
    }
    if scratch == base {
        return Err("emit_store64 scratch must differ from base".to_string());
    }
    push_word(code, encode_addi(scratch, base, 0)?);
    emit_addi_inplace(code, scratch, offset);
    push_word(code, encode_store64_rv(scratch, rs, 0)?);
    Ok(())
}

fn stack_slot_offset_bytes(offset: usize) -> i64 {
    8i64 + offset as i64
}

fn reserve_pointer_literal_stub(code: &mut Vec<u8>) -> usize {
    let start = code.len();
    for _ in 0..POINTER_STUB_LEN {
        push_word(code, 0);
    }
    start
}

fn patch_pointer_literal_stub(
    code: &mut [u8],
    start: usize,
    rd: u8,
    value: u64,
) -> Result<(), String> {
    const BASE_SHIFT: i16 = 7;
    const BASE: u64 = 1 << BASE_SHIFT;

    let mut digits: Vec<i16> = Vec::new();
    let mut n = value as i128;
    while n != 0 {
        let rem = (n % BASE as i128) as i16;
        digits.push(rem);
        n = (n - rem as i128) / BASE as i128;
    }
    if digits.is_empty() {
        digits.push(0);
    }
    digits.reverse();

    let mut words = [0u32; POINTER_STUB_LEN];
    for word in &mut words {
        *word = encode_addi(rd, rd, 0)?;
    }
    let mut idx = 0usize;

    // Ensure rd starts from zero.
    if idx < POINTER_STUB_LEN {
        words[idx] = encode_addi(rd, 0, 0)?;
        idx += 1;
    }
    // Load BASE_SHIFT into the reserved literal scratch register once.
    if idx < POINTER_STUB_LEN {
        words[idx] = encode_addi(LITERAL_SHIFT_REG, 0, 0)?;
        idx += 1;
    }
    if idx < POINTER_STUB_LEN {
        words[idx] = encode_addi(LITERAL_SHIFT_REG, 0, BASE_SHIFT)?;
        idx += 1;
    }

    let mut iter = digits.into_iter();
    if let Some(first) = iter.next() {
        if idx < POINTER_STUB_LEN {
            words[idx] = encode_addi(rd, 0, first)?;
            idx += 1;
        }
        for digit in iter {
            if idx >= POINTER_STUB_LEN {
                break;
            }
            words[idx] = encoding::wide::encode_rr(
                instruction::wide::arithmetic::SLL,
                rd,
                rd,
                LITERAL_SHIFT_REG,
            );
            idx += 1;
            if idx >= POINTER_STUB_LEN {
                break;
            }
            words[idx] = if digit != 0 {
                encode_addi(rd, rd, digit)?
            } else {
                encode_addi(rd, rd, 0)?
            };
            idx += 1;
            if idx >= POINTER_STUB_LEN {
                break;
            }
        }
    }

    for (i, word) in words.iter().enumerate() {
        let offset = start + i * 4;
        code[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    Ok(())
}

fn encode_nop() -> u32 {
    encode_addi(0, 0, 0).expect("ADDI x0, x0, 0 must always encode")
}

fn write_word(code: &mut [u8], at: usize, word: u32) {
    code[at..at + 4].copy_from_slice(&word.to_le_bytes());
}

fn reserve_control_transfer_stub(code: &mut Vec<u8>) -> usize {
    let start = code.len();
    let nop = encode_nop();
    for _ in 0..CONTROL_TRANSFER_STUB_WORDS {
        push_word(code, nop);
    }
    start
}

fn patch_jump_transfer_stub(code: &mut [u8], start: usize, target: u64) -> Result<(), String> {
    let off = (target as i64) - (start as i64);
    if (off % 4) != 0 {
        return Err(format!(
            "unaligned jump offset {off} for control transfer at {start}"
        ));
    }
    if let Ok(off32) = i32::try_from(off)
        && encode_jal(0, off32).is_ok()
    {
        write_word(code, start, encode_jal(0, off32)?);
        return Ok(());
    }

    patch_pointer_literal_stub(code, start, CONTROL_TRANSFER_SCRATCH_REG, target)?;
    let jalr = encoding::wide::encode_ri(
        instruction::wide::control::JALR,
        0,
        CONTROL_TRANSFER_SCRATCH_REG,
        0,
    );
    write_word(code, start + POINTER_STUB_LEN * 4, jalr);
    Ok(())
}

fn patch_call_transfer_stub(code: &mut [u8], start: usize, target: u64) -> Result<(), String> {
    let off = (target as i64) - (start as i64);
    if (off % 4) != 0 {
        return Err(format!(
            "unaligned call offset {off} for control transfer at {start}"
        ));
    }
    if let Ok(off32) = i32::try_from(off)
        && encode_jal(1, off32).is_ok()
    {
        write_word(code, start, encode_jal(1, off32)?);
        let skip_padding = ((CONTROL_TRANSFER_STUB_WORDS - 1) * 4) as i32;
        write_word(code, start + 4, encode_jal(0, skip_padding)?);
        return Ok(());
    }

    patch_pointer_literal_stub(code, start, 1, target)?;
    let jalr = encoding::wide::encode_ri(instruction::wide::control::JALR, 1, 1, 0);
    write_word(code, start + POINTER_STUB_LEN * 4, jalr);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DataKind {
    Account,
    AssetDef,
    NftId,
    AssetId,
    Name,
    Json,
    Domain,
    Blob,
    NoritoBytes,
    DataSpaceId,
    AxtDescriptor,
    AssetHandle,
    ProofBlob,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DataKey(DataKind, String);

type LiteralFixup = (usize, u8, DataKey);

fn emit_literal_stub(code: &mut Vec<u8>, fixups: &mut Vec<LiteralFixup>, rd: u8, key: DataKey) {
    let off = reserve_pointer_literal_stub(code);
    fixups.push((off, rd, key));
}

fn pointer_type_for_kind(kind: ir::DataRefKind) -> Option<PointerType> {
    use ir::DataRefKind::*;
    match kind {
        Account => Some(PointerType::AccountId),
        AssetDef => Some(PointerType::AssetDefinitionId),
        Name => Some(PointerType::Name),
        Json => Some(PointerType::Json),
        NftId => Some(PointerType::NftId),
        AssetId => Some(PointerType::AssetId),
        Domain => Some(PointerType::DomainId),
        Blob => Some(PointerType::Blob),
        NoritoBytes => Some(PointerType::NoritoBytes),
        DataSpaceId => Some(PointerType::DataSpaceId),
        AxtDescriptor => Some(PointerType::AxtDescriptor),
        AssetHandle => Some(PointerType::AssetHandle),
        ProofBlob => Some(PointerType::ProofBlob),
    }
}

fn data_key_for_pointer(kind: ir::DataRefKind, value: &str) -> DataKey {
    use ir::DataRefKind::*;
    match kind {
        Account => DataKey(DataKind::Account, value.to_owned()),
        AssetDef => DataKey(DataKind::AssetDef, value.to_owned()),
        Name => DataKey(DataKind::Name, value.to_owned()),
        Json => DataKey(DataKind::Json, value.to_owned()),
        NftId => DataKey(DataKind::NftId, value.to_owned()),
        AssetId => DataKey(DataKind::AssetId, value.to_owned()),
        Domain => DataKey(DataKind::Domain, value.to_owned()),
        Blob => DataKey(DataKind::Blob, value.to_owned()),
        NoritoBytes => DataKey(DataKind::NoritoBytes, value.to_owned()),
        DataSpaceId => DataKey(DataKind::DataSpaceId, value.to_owned()),
        AxtDescriptor => DataKey(DataKind::AxtDescriptor, value.to_owned()),
        AssetHandle => DataKey(DataKind::AssetHandle, value.to_owned()),
        ProofBlob => DataKey(DataKind::ProofBlob, value.to_owned()),
    }
}

fn decode_hex_or_raw_bytes(raw: &str) -> Result<Vec<u8>, String> {
    if let Some(trimmed) = raw.strip_prefix("0x") {
        if trimmed.len() % 2 == 0 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut out = Vec::with_capacity(trimmed.len() / 2);
            for chunk in trimmed.as_bytes().chunks(2) {
                let byte_str = std::str::from_utf8(chunk)
                    .map_err(|e| format!("invalid hex literal `{raw}`: {e}"))?;
                let byte = u8::from_str_radix(byte_str, 16)
                    .map_err(|e| format!("invalid hex literal `{raw}`: {e}"))?;
                out.push(byte);
            }
            return Ok(out);
        }
        return Err(format!(
            "invalid hex literal `{raw}`: expected even-length hex digits"
        ));
    }
    Ok(raw.as_bytes().to_vec())
}

fn encode_pointer_tlv_bytes(kind: ir::DataRefKind, raw: &str) -> Option<Vec<u8>> {
    use ir::DataRefKind as DRK;
    use iroha_primitives::json::Json;
    use norito::{decode_from_bytes, to_bytes};

    let (type_id, payload) = match kind {
        DRK::Account => {
            let id = iroha_data_model::account::AccountId::parse_encoded(raw)
                .ok()?
                .into_account_id();
            (PointerType::AccountId, to_bytes(&id).ok()?)
        }
        DRK::AssetDef => {
            let id: iroha_data_model::asset::AssetDefinitionId = raw.parse().ok()?;
            (PointerType::AssetDefinitionId, to_bytes(&id).ok()?)
        }
        DRK::AssetId => {
            let id: iroha_data_model::asset::AssetId = raw.parse().ok()?;
            (PointerType::AssetId, to_bytes(&id).ok()?)
        }
        DRK::NftId => {
            let id: iroha_data_model::nft::NftId = raw.parse().ok()?;
            (PointerType::NftId, to_bytes(&id).ok()?)
        }
        DRK::Name => {
            let nm: iroha_data_model::name::Name = raw.parse().ok()?;
            (PointerType::Name, to_bytes(&nm).ok()?)
        }
        DRK::Domain => {
            let id: iroha_data_model::domain::DomainId = raw.parse().ok()?;
            (PointerType::DomainId, to_bytes(&id).ok()?)
        }
        DRK::Json => {
            let json = Json::from_str_norito(raw).ok()?;
            (PointerType::Json, to_bytes(&json).ok()?)
        }
        DRK::Blob => (PointerType::Blob, decode_hex_or_raw_bytes(raw).ok()?),
        DRK::NoritoBytes => (PointerType::NoritoBytes, decode_hex_or_raw_bytes(raw).ok()?),
        DRK::DataSpaceId => {
            if let Some(raw_id) = parse_u64_literal(raw) {
                let id = iroha_data_model::nexus::DataSpaceId::new(raw_id);
                (PointerType::DataSpaceId, to_bytes(&id).ok()?)
            } else {
                let bytes = decode_hex_or_raw_bytes(raw).ok()?;
                let value: iroha_data_model::nexus::DataSpaceId = decode_from_bytes(&bytes).ok()?;
                (PointerType::DataSpaceId, to_bytes(&value).ok()?)
            }
        }
        DRK::AxtDescriptor => {
            let bytes = decode_hex_or_raw_bytes(raw).ok()?;
            let value: crate::axt::AxtDescriptor = decode_from_bytes(&bytes).ok()?;
            (PointerType::AxtDescriptor, to_bytes(&value).ok()?)
        }
        DRK::AssetHandle => {
            let bytes = decode_hex_or_raw_bytes(raw).ok()?;
            let value: crate::axt::AssetHandle = decode_from_bytes(&bytes).ok()?;
            (PointerType::AssetHandle, to_bytes(&value).ok()?)
        }
        DRK::ProofBlob => {
            let bytes = decode_hex_or_raw_bytes(raw).ok()?;
            let value: crate::axt::ProofBlob = decode_from_bytes(&bytes).ok()?;
            (PointerType::ProofBlob, to_bytes(&value).ok()?)
        }
    };

    let mut out = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    out.extend_from_slice(&(type_id as u16).to_be_bytes());
    out.push(1u8);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    out.extend_from_slice(&h);
    Some(out)
}

fn parse_u64_literal(raw: &str) -> Option<u64> {
    if let Some(hex) = raw.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else {
        raw.parse::<u64>().ok()
    }
}

// Kotodama ZK intrinsics are supported by the semantic/IR lowering:
//   - `zk_verify_transfer`, `zk_verify_unshield`, `zk_vote_verify_ballot`,
//     `zk_vote_verify_tally` lower to SCALL 0x60..0x63 with `&NoritoBytes` in r10.
//   - `execute_instruction` and `sc_execute_*` variants lower to
//     SCALL 0xA0 with `&NoritoBytes(InstructionBox)` in r10.
// See `kotodama::semantic`, `kotodama::ir`, and the sample
// `crates/kotodama_lang/src/samples/zk_vote_and_unshield.ko`.

/// Compiler entry point for translating KOTODAMA programs into IVM bytecode.
pub struct Compiler {
    lang: Language,
    opts: CompilerOptions,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Options controlling metadata emitted by the compiler.
#[derive(Clone, Debug)]
pub struct CompilerOptions {
    /// ABI version to encode in the program header. Controls syscall policy and pointer‑ABI.
    pub abi_version: u8,
    /// Force ZK mode bit in header even if program does not use ZK opcodes.
    pub force_zk: bool,
    /// Force VECTOR mode bit in header even if program does not use vector ops.
    pub force_vector: bool,
    /// Requested logical vector length (0 = max).
    pub vector_length: u8,
    /// Optional maximum cycles to encode; 0 means "use compiler default".
    pub max_cycles: u64,
    /// Max unrolled iterations for dynamic bounds lowering (feature-gated).
    pub dynamic_iter_cap: u8,
    /// Enforce the deterministic on-chain safety profile during compilation.
    pub enforce_on_chain_profile: bool,
    /// Emit additive compiler debug metadata into the artifact.
    pub emit_debug: bool,
    /// Optional logical source path embedded into compiler debug metadata.
    pub debug_source_name: Option<String>,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            abi_version: 1,
            force_zk: false,
            force_vector: false,
            vector_length: 0,
            max_cycles: DEFAULT_MAX_CYCLES,
            dynamic_iter_cap: 2,
            enforce_on_chain_profile: true,
            emit_debug: true,
            debug_source_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{
        Compiler, CompilerOptions, ContractFeature, DEFAULT_MAX_CYCLES, GLOBAL_WILDCARD_KEY,
        STATE_WILDCARD_KEY, WIDE_IMM_MAX, emit_addi, emit_load64, emit_store64,
        patch_pointer_literal_stub, pointer_type_for_kind, reserve_pointer_literal_stub,
        stack_slot_offset_bytes,
    };
    use crate::{ast::ContractMeta, ir, parser::parse, semantic::analyze};
    use crate::{encoding, instruction, metadata::ProgramMetadata, pointer_abi::PointerType};

    fn sample_account_id() -> iroha_data_model::account::AccountId {
        iroha_data_model::account::AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .expect("public key"),
        )
    }

    fn sample_account_literal() -> String {
        sample_account_id().to_string()
    }

    fn sample_account_id_alt() -> iroha_data_model::account::AccountId {
        iroha_data_model::account::AccountId::new(
            "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                .parse()
                .expect("public key"),
        )
    }

    #[test]
    fn pointer_types_cover_all_data_ref_kinds() {
        use super::ir::DataRefKind::*;
        let cases = [
            (Account, PointerType::AccountId),
            (AssetDef, PointerType::AssetDefinitionId),
            (AssetId, PointerType::AssetId),
            (NftId, PointerType::NftId),
            (Name, PointerType::Name),
            (Json, PointerType::Json),
            (Domain, PointerType::DomainId),
            (Blob, PointerType::Blob),
            (NoritoBytes, PointerType::NoritoBytes),
            (DataSpaceId, PointerType::DataSpaceId),
            (AxtDescriptor, PointerType::AxtDescriptor),
            (AssetHandle, PointerType::AssetHandle),
            (ProofBlob, PointerType::ProofBlob),
        ];
        for (kind, expected) in cases {
            let ty = pointer_type_for_kind(kind);
            assert_eq!(
                ty,
                Some(expected),
                "DataRefKind::{kind:?} should map to PointerType::{expected:?}"
            );
        }
    }

    #[test]
    fn default_max_cycles_matches_pipeline_bound() {
        let opts = CompilerOptions::default();
        assert_eq!(opts.max_cycles, DEFAULT_MAX_CYCLES);
        assert!(opts.max_cycles > 0);
    }

    #[test]
    fn meta_max_cycles_zero_uses_compiler_default() {
        let opts = CompilerOptions {
            max_cycles: 42,
            ..CompilerOptions::default()
        };
        let compiler = Compiler::new_with_options(opts);
        let src = r#"
seiyaku MyC {
  meta { max_cycles: 0; }
  hajimari() { let a = 1; }
}
"#;
        let code = compiler.compile_source(src).expect("compile");
        let parsed = ProgramMetadata::parse(&code).expect("parse meta");
        assert_eq!(parsed.metadata.max_cycles, 42);
    }

    #[test]
    fn emit_addi_zero_uses_addi_copy() {
        let mut code = Vec::new();
        emit_addi(&mut code, 5, 7, 0);
        assert_eq!(code.len(), 4);
        let word = u32::from_le_bytes(code[..4].try_into().unwrap());
        assert_eq!(
            word,
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 5, 7, 0)
        );
    }

    #[test]
    fn emit_load64_uses_addi_for_copy() {
        let mut code = Vec::new();
        let offset = WIDE_IMM_MAX as i64 + 1;
        emit_load64(&mut code, 5, 6, offset, None).expect("emit load64");
        let word = u32::from_le_bytes(code[..4].try_into().unwrap());
        assert_eq!(
            word,
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 5, 6, 0)
        );
    }

    #[test]
    fn emit_store64_uses_addi_for_copy() {
        let mut code = Vec::new();
        let offset = WIDE_IMM_MAX as i64 + 1;
        emit_store64(&mut code, 6, 5, offset, 7).expect("emit store64");
        let word = u32::from_le_bytes(code[..4].try_into().unwrap());
        assert_eq!(
            word,
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 7, 6, 0)
        );
    }

    #[test]
    fn stack_slot_offsets_do_not_truncate_large_offsets() {
        let offset = i16::MAX as usize + 2048;
        let bytes = stack_slot_offset_bytes(offset);
        assert_eq!(bytes, 8i64 + offset as i64);
        assert!(bytes > i16::MAX as i64);
    }

    #[test]
    fn pointer_literal_stub_uses_addi_for_zeroing() {
        let mut code = Vec::new();
        let start = reserve_pointer_literal_stub(&mut code);
        patch_pointer_literal_stub(&mut code, start, 5, 0).expect("patch pointer literal");
        let word = u32::from_le_bytes(code[start..start + 4].try_into().unwrap());
        assert_eq!(
            instruction::wide::opcode(word),
            instruction::wide::arithmetic::ADDI
        );
    }

    #[test]
    fn far_jump_fixup_uses_literal_stub_and_jalr() {
        let mut code = Vec::new();
        let start = super::reserve_control_transfer_stub(&mut code);
        super::patch_jump_transfer_stub(&mut code, start, 200_000).expect("patch far jump");
        let final_word_at = start + super::POINTER_STUB_LEN * 4;
        let final_word =
            u32::from_le_bytes(code[final_word_at..final_word_at + 4].try_into().unwrap());
        assert_eq!(
            final_word,
            encoding::wide::encode_ri(
                instruction::wide::control::JALR,
                0,
                super::CONTROL_TRANSFER_SCRATCH_REG,
                0,
            )
        );
    }

    #[test]
    fn near_call_fixup_skips_stub_padding_on_return() {
        let mut code = Vec::new();
        let start = super::reserve_control_transfer_stub(&mut code);
        super::patch_call_transfer_stub(&mut code, start, (start + 16) as u64)
            .expect("patch near call");

        let call_word = u32::from_le_bytes(code[start..start + 4].try_into().unwrap());
        assert_eq!(
            call_word,
            super::encode_jal(1, 16).expect("encode short call")
        );

        let skip_word = u32::from_le_bytes(code[start + 4..start + 8].try_into().unwrap());
        assert_eq!(
            skip_word,
            super::encode_jal(0, ((super::CONTROL_TRANSFER_STUB_WORDS - 1) * 4) as i32)
                .expect("encode stub skip")
        );
    }

    #[test]
    fn encode_addi_rejects_out_of_range_immediate() {
        let imm = (WIDE_IMM_MAX + 1) as i16;
        assert!(super::encode_addi(1, 1, imm).is_err());
    }

    #[test]
    fn encode_load64_rejects_out_of_range_offset() {
        let imm = (WIDE_IMM_MAX + 1) as i16;
        assert!(super::encode_load64_rv(1, 2, imm).is_err());
    }

    #[test]
    fn encode_store64_rejects_out_of_range_offset() {
        let imm = (WIDE_IMM_MAX + 1) as i16;
        assert!(super::encode_store64_rv(1, 2, imm).is_err());
    }

    #[test]
    fn encode_branch_rejects_unaligned_offsets() {
        assert!(super::encode_branch_rv(0x0, 1, 2, 2).is_err());
    }

    #[test]
    fn encode_jal_rejects_unaligned_offsets() {
        assert!(super::encode_jal(0, 2).is_err());
    }

    #[test]
    fn emit_load64_requires_scratch_when_rd_equals_base() {
        let mut code = Vec::new();
        let err = emit_load64(&mut code, 5, 5, 0, None).unwrap_err();
        assert!(err.contains("emit_load64 requires scratch"));
    }

    #[test]
    fn emit_store64_requires_distinct_scratch() {
        let mut code = Vec::new();
        let err = emit_store64(&mut code, 5, 6, 256, 5).unwrap_err();
        assert!(err.contains("emit_store64 scratch"));
    }

    #[test]
    fn decode_hex_or_raw_bytes_accepts_hex_prefix() {
        let bytes = super::decode_hex_or_raw_bytes("0x0a0b").expect("hex literal");
        assert_eq!(bytes, vec![0x0a, 0x0b]);
    }

    #[test]
    fn decode_hex_or_raw_bytes_preserves_raw_text() {
        let bytes = super::decode_hex_or_raw_bytes("raw").expect("raw literal");
        assert_eq!(bytes, b"raw".to_vec());
    }

    #[test]
    fn unary_neg_emits_neg_opcode() {
        let src = r#"
seiyaku NegTest {
  kotoage fn neg(x: int) -> int {
    return -x;
  }
}
"#;
        let compiler = Compiler::new();
        let bytes = compiler.compile_source(src).expect("compile neg");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let mut found = false;
        for chunk in bytes[parsed.code_offset..].chunks_exact(4) {
            let word = u32::from_le_bytes(<[u8; 4]>::try_from(chunk).unwrap());
            if instruction::wide::opcode(word) == instruction::wide::arithmetic::NEG {
                found = true;
                break;
            }
        }
        assert!(found, "expected NEG opcode in compiled code");
    }

    #[test]
    fn json_get_numeric_emits_numeric_syscall() {
        let src = r#"
seiyaku JsonNumericTest {
  meta { abi_version: 1; }
  fn run() {
    let ev = trigger_event();
    let _amount: Amount = json_get_numeric(ev, name("amount"));
  }
}
"#;
        let compiler = Compiler::new();
        let bytes = compiler
            .compile_source(src)
            .expect("compile json_get_numeric");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_JSON_GET_NUMERIC as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected JSON_GET_NUMERIC syscall in compiled code"
        );
    }

    #[test]
    fn json_get_asset_definition_id_emits_asset_definition_syscall() {
        let src = r#"
seiyaku JsonAssetDefinitionTest {
  meta { abi_version: 1; }
  fn run() {
    let ev = trigger_event();
    let _asset = json_get_asset_definition_id(ev, name("asset_definition_id"));
  }
}
"#;
        let compiler = Compiler::new();
        let bytes = compiler
            .compile_source(src)
            .expect("compile json_get_asset_definition_id");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected JSON_GET_ASSET_DEFINITION_ID syscall in compiled code"
        );
    }

    #[test]
    fn detect_vector_usage_includes_vector_gated_crypto_ops() {
        let ops = [
            instruction::wide::crypto::SHA256BLOCK,
            instruction::wide::crypto::AESENC,
            instruction::wide::crypto::AESDEC,
        ];
        for op in ops {
            let word = encoding::wide::encode_rr(op, 0, 0, 0);
            let code = word.to_le_bytes();
            assert!(
                super::detect_vector_usage(&code),
                "expected vector usage for opcode {op:#04x}"
            );
        }
    }

    #[test]
    fn detect_zk_usage_includes_zk_ops() {
        let ops = [
            instruction::wide::zk::ASSERT,
            instruction::wide::zk::ASSERT_EQ,
            instruction::wide::zk::FADD,
        ];
        for op in ops {
            let word = encoding::wide::encode_rr(op, 0, 0, 0);
            let code = word.to_le_bytes();
            assert!(
                super::detect_zk_usage(&code),
                "expected zk usage for opcode {op:#04x}"
            );
        }
    }

    #[test]
    fn require_compiles_without_zk_mode_and_uses_abort_syscall() {
        let src = r#"
seiyaku Test {
  kotoage fn main() {
    require(1 == 1);
  }
}
"#;
        let compiler = Compiler::new();
        let bytes = compiler.compile_source(src).expect("compile require");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        assert_eq!(
            parsed.metadata.mode & crate::metadata::mode::ZK,
            0,
            "require should not enable ZK mode"
        );

        let mut found_abort = false;
        let mut found_zk_assert = false;
        for chunk in bytes[parsed.code_offset..].chunks_exact(4) {
            let word = u32::from_le_bytes(<[u8; 4]>::try_from(chunk).unwrap());
            let op = instruction::wide::opcode(word);
            if op == instruction::wide::system::SCALL {
                let (_op, imm8) = encoding::wide::decode_sys(word);
                if imm8 == crate::syscalls::SYSCALL_ABORT as u8 {
                    found_abort = true;
                }
            }
            if op == instruction::wide::zk::ASSERT || op == instruction::wide::zk::ASSERT_EQ {
                found_zk_assert = true;
            }
        }
        assert!(found_abort, "expected ABORT syscall in compiled require");
        assert!(
            !found_zk_assert,
            "require should not emit ZK ASSERT/ASSERT_EQ opcodes"
        );
    }

    #[test]
    fn assert_compiles_without_zk_mode_and_uses_abort_syscall() {
        let src = r#"
seiyaku Test {
  kotoage fn main() {
    assert(1 == 1);
  }
}
"#;
        let compiler = Compiler::new();
        let bytes = compiler.compile_source(src).expect("compile assert");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        assert_eq!(
            parsed.metadata.mode & crate::metadata::mode::ZK,
            0,
            "assert should not enable ZK mode"
        );

        let mut found_abort = false;
        let mut found_zk_assert = false;
        for chunk in bytes[parsed.code_offset..].chunks_exact(4) {
            let word = u32::from_le_bytes(<[u8; 4]>::try_from(chunk).unwrap());
            let op = instruction::wide::opcode(word);
            if op == instruction::wide::system::SCALL {
                let (_op, imm8) = encoding::wide::decode_sys(word);
                if imm8 == crate::syscalls::SYSCALL_ABORT as u8 {
                    found_abort = true;
                }
            }
            if op == instruction::wide::zk::ASSERT || op == instruction::wide::zk::ASSERT_EQ {
                found_zk_assert = true;
            }
        }
        assert!(found_abort, "expected ABORT syscall in compiled assert");
        assert!(
            !found_zk_assert,
            "assert should not emit ZK ASSERT/ASSERT_EQ opcodes"
        );
    }

    #[test]
    fn assert_eq_compiles_without_zk_mode_and_uses_abort_syscall() {
        let src = r#"
seiyaku Test {
  kotoage fn main() {
    assert_eq(1, 1);
  }
}
"#;
        let compiler = Compiler::new();
        let bytes = compiler.compile_source(src).expect("compile assert_eq");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        assert_eq!(
            parsed.metadata.mode & crate::metadata::mode::ZK,
            0,
            "assert_eq should not enable ZK mode"
        );

        let mut found_abort = false;
        let mut found_zk_assert = false;
        for chunk in bytes[parsed.code_offset..].chunks_exact(4) {
            let word = u32::from_le_bytes(<[u8; 4]>::try_from(chunk).unwrap());
            let op = instruction::wide::opcode(word);
            if op == instruction::wide::system::SCALL {
                let (_op, imm8) = encoding::wide::decode_sys(word);
                if imm8 == crate::syscalls::SYSCALL_ABORT as u8 {
                    found_abort = true;
                }
            }
            if op == instruction::wide::zk::ASSERT || op == instruction::wide::zk::ASSERT_EQ {
                found_zk_assert = true;
            }
        }
        assert!(found_abort, "expected ABORT syscall in compiled assert_eq");
        assert!(
            !found_zk_assert,
            "assert_eq should not emit ZK ASSERT/ASSERT_EQ opcodes"
        );
    }

    #[test]
    fn validate_feature_requests_reports_unused_requested_features() {
        let mut meta = ContractMeta::default();
        meta.features.push(ContractFeature::Vector);
        let err = super::validate_feature_requests(Some(&meta), false, false)
            .expect_err("expected vector mismatch");
        assert!(err.contains("meta requests vector"));
    }

    #[test]
    fn validate_feature_requests_reports_forbidden_usage() {
        let meta = ContractMeta {
            force_zk: Some(false),
            ..Default::default()
        };
        let err = super::validate_feature_requests(Some(&meta), true, false)
            .expect_err("expected zk mismatch");
        assert!(err.contains("meta disables zk"));
    }

    #[test]
    fn meta_requests_zk_without_usage_is_error() {
        let src = r#"
seiyaku Test {
  meta { zk: true; }
  kotoage fn main() { let x = 1; }
}
"#;
        let compiler = Compiler::new();
        let err = compiler
            .compile_source(src)
            .expect_err("expected zk usage mismatch");
        assert!(err.contains("meta requests zk"));
    }

    #[test]
    fn meta_disables_zk_with_poseidon_is_error() {
        let src = r#"
seiyaku Test {
  meta { zk: false; }
  kotoage fn main() { let x = poseidon2(1, 2); }
}
"#;
        let compiler = Compiler::new();
        let err = compiler
            .compile_source(src)
            .expect_err("expected zk disabled mismatch");
        assert!(err.contains("meta disables zk"));
    }

    #[test]
    fn setvl_emits_setvl_opcode() {
        let src = r#"
seiyaku Test {
  meta { vector: true; }
  kotoage fn main() { setvl(8); }
}
"#;
        let compiler = Compiler::new();
        let bytes = compiler.compile_source(src).expect("compile setvl");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let mut found = false;
        for chunk in bytes[parsed.code_offset..].chunks_exact(4) {
            let word = u32::from_le_bytes(<[u8; 4]>::try_from(chunk).unwrap());
            if instruction::wide::opcode(word) == instruction::wide::crypto::SETVL {
                found = true;
                break;
            }
        }
        assert!(found, "expected SETVL opcode in compiled code");
    }

    #[test]
    fn setvl_requires_literal_int() {
        let src = r#"
seiyaku Test {
  kotoage fn main() { helper(1); }
  fn helper(a: int) { setvl(a); }
}
"#;
        let compiler = Compiler::new();
        let err = compiler
            .compile_source(src)
            .expect_err("expected setvl literal error");
        assert!(err.contains("setvl expects a literal int"));
    }

    #[test]
    fn manifest_access_set_hints_from_state_only_contract() {
        let src = r#"
seiyaku Test {
  state Foo: Map<Name, int>;

  kotoage fn set(pool: Name, value: int) {
    Foo[pool] = value;
  }

  kotoage fn get(pool: Name) -> int {
    return Foo[pool];
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_eq!(hints.read_keys, vec!["state:Foo".to_string()]);
        assert_eq!(hints.write_keys, vec!["state:Foo".to_string()]);
    }

    #[test]
    fn entrypoint_hints_include_map_base_for_dynamic_state_paths() {
        let src = r#"
seiyaku Test {
  state Foo: Map<int, int>;

  kotoage fn read_dyn(k: int) {
    let _x = Foo[k];
  }

  kotoage fn read_lit() {
    let _x = Foo[1];
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(hints.read_keys.contains(&"state:Foo".to_string()));
        assert!(hints.read_keys.contains(&"state:Foo/1".to_string()));
        assert!(hints.write_keys.is_empty());
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let read_dyn = entrypoints
            .iter()
            .find(|entry| entry.name == "read_dyn")
            .expect("read_dyn entrypoint");
        let read_lit = entrypoints
            .iter()
            .find(|entry| entry.name == "read_lit")
            .expect("read_lit entrypoint");
        assert_eq!(read_dyn.read_keys, vec!["state:Foo".to_string()]);
        assert!(read_dyn.write_keys.is_empty());
        assert_eq!(read_dyn.access_hints_complete, Some(true));
        assert!(read_dyn.access_hints_skipped.is_empty());
        assert!(read_lit.read_keys.contains(&"state:Foo/1".to_string()));
        assert!(read_lit.read_keys.contains(&"state:Foo".to_string()));
        assert!(read_lit.write_keys.is_empty());
        assert_eq!(read_lit.access_hints_complete, Some(true));
        assert!(read_lit.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_state_wildcard_for_dynamic_state_path() {
        let src = r#"
seiyaku Test {
  kotoage fn read(path: Name) {
    let _x = state_get(path);
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_eq!(hints.read_keys, vec![STATE_WILDCARD_KEY.to_string()]);
        assert_eq!(hints.write_keys, vec![STATE_WILDCARD_KEY.to_string()]);
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let read = entrypoints
            .iter()
            .find(|entry| entry.name == "read")
            .expect("read entrypoint");
        assert_eq!(read.read_keys, vec![STATE_WILDCARD_KEY.to_string()]);
        assert_eq!(read.write_keys, vec![STATE_WILDCARD_KEY.to_string()]);
        assert_eq!(read.access_hints_complete, Some(true));
        assert!(read.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_create_trigger_from_json() {
        let src = r#"
seiyaku Test {
  kotoage fn make() permission(Admin) {
    create_trigger(json("{\"id\":\"t1\"}"));
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        let expected = vec![
            "trigger.repetitions:t1".to_string(),
            "trigger:t1".to_string(),
        ];
        assert_eq!(hints.read_keys, expected);
        assert_eq!(hints.write_keys, expected);
    }

    #[test]
    fn manifest_access_set_hints_include_execute_instruction_literal() {
        use iroha_data_model::{
            account::AccountId,
            asset::id::{AssetDefinitionId, AssetId},
            isi::{InstructionBox, Mint},
        };

        let account = AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .expect("public key"),
        );
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_def.clone(), account.clone());
        let canonical_asset =
            AssetId::parse_literal(&asset_id.canonical_literal()).expect("parse canonical asset");
        let isi = InstructionBox::from(Mint::asset_numeric(1u32, asset_id.clone()));
        let bytes = norito::to_bytes(&isi).expect("encode InstructionBox");
        let hex_payload = format!("0x{}", hex::encode(bytes));
        let src = format!("fn main() {{ execute_instruction(norito_bytes(\"{hex_payload}\")); }}");

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(
            hints
                .read_keys
                .contains(&format!("account:{}", canonical_asset.account()))
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("domain:{}", canonical_asset.definition().domain()))
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("asset_def:{}", canonical_asset.definition()))
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("asset:{canonical_asset}"))
        );
        assert!(
            hints
                .write_keys
                .contains(&format!("asset_def:{}", canonical_asset.definition()))
        );
        assert!(
            hints
                .write_keys
                .contains(&format!("asset:{canonical_asset}"))
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_execute_instruction_details() {
        use std::str::FromStr;

        use iroha_data_model::{
            asset::id::AssetDefinitionId,
            domain::DomainId,
            isi::{Grant, InstructionBox, SetKeyValue},
            name::Name,
            nft::NftId,
            permission::Permission,
            role::RoleId,
            trigger::TriggerId,
        };
        use iroha_primitives::json::Json;

        let domain_id: DomainId = "wonderland".parse().unwrap();
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        );
        let nft_id: NftId = "n0$wonderland".parse().unwrap();
        let trigger_id: TriggerId = "wake".parse().unwrap();
        let role_id: RoleId = "auditor".parse().unwrap();
        let key = Name::from_str("meta").unwrap();
        let permission = Permission::new("CanManageDomains".to_string(), Json::new(1u32));
        let instructions = [
            InstructionBox::from(SetKeyValue::domain(
                domain_id.clone(),
                key.clone(),
                Json::new(1u32),
            )),
            InstructionBox::from(SetKeyValue::asset_definition(
                asset_def.clone(),
                key.clone(),
                Json::new(2u32),
            )),
            InstructionBox::from(SetKeyValue::nft(
                nft_id.clone(),
                key.clone(),
                Json::new(3u32),
            )),
            InstructionBox::from(SetKeyValue::trigger(
                trigger_id.clone(),
                key.clone(),
                Json::new(4u32),
            )),
            InstructionBox::from(Grant::role_permission(permission.clone(), role_id.clone())),
        ];

        let mut src = String::from("fn main() {\n");
        for isi in &instructions {
            let bytes = norito::to_bytes(isi).expect("encode InstructionBox");
            let hex_payload = format!("0x{}", hex::encode(bytes));
            src.push_str(&format!(
                "  execute_instruction(norito_bytes(\"{hex_payload}\"));\n"
            ));
        }
        src.push_str("}\n");

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");

        let domain_detail = format!("domain.detail:{domain_id}:{key}");
        assert!(hints.read_keys.contains(&format!("domain:{domain_id}")));
        assert!(hints.read_keys.contains(&domain_detail));
        assert!(hints.write_keys.contains(&domain_detail));

        let asset_def_detail = format!("asset_def.detail:{asset_def}:{key}");
        assert!(hints.read_keys.contains(&format!("asset_def:{asset_def}")));
        assert!(hints.read_keys.contains(&asset_def_detail));
        assert!(hints.write_keys.contains(&asset_def_detail));

        let nft_detail = format!("nft.detail:{nft_id}:{key}");
        assert!(hints.read_keys.contains(&format!("nft:{nft_id}")));
        assert!(hints.read_keys.contains(&nft_detail));
        assert!(hints.write_keys.contains(&nft_detail));

        let trigger_detail = format!("trigger.detail:{trigger_id}:{key}");
        assert!(hints.read_keys.contains(&format!("trigger:{trigger_id}")));
        assert!(hints.write_keys.contains(&trigger_detail));

        let perm_role = format!("perm.role:{role_id}:{}", permission.name());
        assert!(hints.read_keys.contains(&format!("role:{role_id}")));
        assert!(hints.write_keys.contains(&format!("role:{role_id}")));
        assert!(hints.write_keys.contains(&perm_role));
    }

    #[test]
    fn manifest_access_set_hints_include_execute_query_literal() {
        use iroha_data_model::{
            account::AccountId,
            asset::id::{AssetDefinitionId, AssetId},
            query::asset::FindAssetById,
            query::{QueryRequest, SingularQueryBox},
        };

        let account = AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .expect("public key"),
        );
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_def.clone(), account.clone());
        let canonical_asset =
            AssetId::parse_literal(&asset_id.canonical_literal()).expect("parse canonical asset");
        let request = QueryRequest::Singular(SingularQueryBox::FindAssetById(FindAssetById::new(
            asset_id.clone(),
        )));
        let bytes = norito::to_bytes(&request).expect("encode QueryRequest");
        let hex_payload = format!("0x{}", hex::encode(bytes));
        let src = format!("fn main() {{ execute_query(norito_bytes(\"{hex_payload}\")); }}");

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(
            hints
                .read_keys
                .contains(&format!("account:{}", canonical_asset.account()))
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("domain:{}", canonical_asset.definition().domain()))
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("asset_def:{}", canonical_asset.definition()))
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("asset:{canonical_asset}"))
        );
        assert!(hints.write_keys.is_empty());

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_transfer_domain_literal() {
        use iroha_data_model::domain::DomainId;

        let from_literal = sample_account_literal();
        let to = sample_account_id_alt();
        let to_literal = to.to_string();
        let domain: DomainId = "wonderland".parse().unwrap();
        let src = format!(
            "fn main() {{ transfer_domain(account_id(\"{from_literal}\"), domain(\"{domain}\"), account_id(\"{to_literal}\")); }}"
        );

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(hints.read_keys.contains(&format!("domain:{domain}")));
        assert!(hints.write_keys.contains(&format!("domain:{domain}")));
        assert!(hints.read_keys.contains(&format!("account:{to}")));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_wildcard_for_isi_contract() {
        let src = r#"
seiyaku Test {
  kotoage fn move(from: AccountId, to: AccountId, asset: AssetDefinitionId, amount: int) permission(Admin) {
    transfer_asset(from, to, asset, amount);
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_eq!(hints.read_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        assert_eq!(hints.write_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "move")
            .expect("move entrypoint");
        assert_eq!(main.read_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        assert_eq!(main.write_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_wildcard_for_opaque_host_calls() {
        let src = r#"
seiyaku Test {
  kotoage fn register() permission(Admin) {
    register_peer(json("{}"));
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_eq!(hints.read_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        assert_eq!(hints.write_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let register = entrypoints
            .iter()
            .find(|entry| entry.name == "register")
            .expect("register entrypoint");
        assert_eq!(register.read_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        assert_eq!(register.write_keys, vec![GLOBAL_WILDCARD_KEY.to_string()]);
        assert_eq!(register.access_hints_complete, Some(true));
        assert!(register.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_trigger_decl_sets_authority() {
        use iroha_data_model::account::{AccountId, ParsedAccountId};

        let authority_literal = sample_account_literal();
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on time pre_commit;
    authority "{authority_literal}";
  }}
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let run = entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("run entrypoint");
        assert_eq!(run.triggers.len(), 1);
        let trigger = &run.triggers[0];
        assert_eq!(trigger.id.to_string(), "wake");
        assert_eq!(
            trigger.authority,
            Some(
                AccountId::parse_encoded(authority_literal.as_str())
                    .map(ParsedAccountId::into_account_id)
                    .expect("authority literal"),
            )
        );
    }

    #[test]
    fn trigger_callback_entrypoint_is_compiled_first_even_with_private_helpers() {
        let src = r#"
seiyaku Test {
  fn update_record(request_id: Name) {
    state_set(name("LastRequestId"), pointer_to_norito(request_id));
  }

  kotoage fn run() {
    update_record(name("request-1"));
  }

  register_trigger wake {
    call run;
    on time pre_commit;
  }
}
"#;
        let compiler = Compiler::new();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let run = entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("run entrypoint");
        let parsed = ivm_abi::metadata::ProgramMetadata::parse(&bytes).expect("parse metadata");
        let embedded = parsed
            .contract_interface
            .expect("embedded contract interface");
        let run_embedded = embedded
            .entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("embedded run entrypoint");
        assert_eq!(
            run_embedded.entry_pc, 0,
            "trigger callback entrypoint must be laid out first so VM startup enters `run`"
        );
        assert_eq!(run.name, "run");
    }

    #[test]
    fn staged_mint_helper_keeps_state_map_base_literals_after_call_propagation() {
        let src = r#"
seiyaku StagedMintRequest {
  state int MintRequestNextSequence;
  state MintRequestSequenceById: Map<Name, int>;
  state MintRequestSequences: Map<int, int>;
  state MintRequestRequestIds: Map<int, Name>;
  state MintRequestFiIds: Map<int, Name>;
  state MintRequestFiAuthorities: Map<int, AccountId>;
  state MintRequestToAccounts: Map<int, AccountId>;
  state MintRequestAmounts: Map<int, int>;
  state MintRequestRequestedBy: Map<int, Json>;
  state MintRequestStates: Map<int, int>;
  state MintRequestCreatedAt: Map<int, int>;
  state MintRequestExpiresAt: Map<int, int>;
  state MintRequestFinalizedAt: Map<int, int>;
  state MintRequestCanceledAt: Map<int, int>;

  fn update_record(sequence: int,
                   request_id: Name,
                   fi_id: Name,
                   fi_multisig_account_id: AccountId,
                   to_account_id: AccountId,
                   amount_i64: int,
                   requested_by_actor_id: Json,
                   state_code: int,
                   created_at_ms: int,
                   expires_at_ms: int,
                   finalized_at_ms: int,
                   canceled_at_ms: int) {
    MintRequestSequences[sequence] = sequence;
    MintRequestRequestIds[sequence] = request_id;
    MintRequestFiIds[sequence] = fi_id;
    MintRequestFiAuthorities[sequence] = fi_multisig_account_id;
    MintRequestToAccounts[sequence] = to_account_id;
    MintRequestAmounts[sequence] = amount_i64;
    MintRequestRequestedBy[sequence] = requested_by_actor_id;
    MintRequestStates[sequence] = state_code;
    MintRequestCreatedAt[sequence] = created_at_ms;
    MintRequestExpiresAt[sequence] = expires_at_ms;
    MintRequestFinalizedAt[sequence] = finalized_at_ms;
    MintRequestCanceledAt[sequence] = canceled_at_ms;
  }

  fn run() {
    let ev = trigger_event();
    let action_key = name("action");
    let request_id_key = name("request_id");
    let fi_id_key = name("fi_id");
    let to_account_id_key = name("to_account_id");
    let amount_i64_key = name("amount_i64");
    let requested_by_actor_id_key = name("requested_by_actor_id");
    let created_at_ms_key = name("created_at_ms");
    let expires_at_ms_key = name("expires_at_ms");

    let action = json_get_name(ev, action_key);
    if (action == name("create")) {
      let request_id = json_get_name(ev, request_id_key);
      let sequence = MintRequestNextSequence + 1;
      let fi_id = json_get_name(ev, fi_id_key);
      let to_account_id = json_get_account_id(ev, to_account_id_key);
      let amount_i64 = json_get_int(ev, amount_i64_key);
      let requested_by_actor_id = json_get_json(ev, requested_by_actor_id_key);
      let created_at_ms = json_get_int(ev, created_at_ms_key);
      let expires_at_ms = json_get_int(ev, expires_at_ms_key);
      update_record(sequence,
                    request_id,
                    fi_id,
                    to_account_id,
                    to_account_id,
                    amount_i64,
                    requested_by_actor_id,
                    0,
                    created_at_ms,
                    expires_at_ms,
                    0,
                    0);
    }
  }
}
"#;

        let program = parse(src).expect("parse");
        let typed = analyze(&program).expect("analyze");
        let ir_prog =
            ir::lower_with_cap(&typed, CompilerOptions::default().dynamic_iter_cap as usize)
                .expect("lower");
        let typed_functions: Vec<_> = typed
            .items
            .iter()
            .map(|item| match item {
                crate::semantic::TypedItem::Function(func) => func,
            })
            .collect();

        let mut string_map: HashMap<(usize, ir::Temp), String> = HashMap::new();
        let mut string_literal_temps: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut dataref_kind_map: HashMap<(usize, ir::Temp), ir::DataRefKind> = HashMap::new();
        let mut int_const_map: HashMap<(usize, ir::Temp), i64> = HashMap::new();
        let mut param_temp_map: HashMap<(usize, usize), ir::Temp> = HashMap::new();

        use crate::ast::UnaryOp;
        use crate::ir::DataRefKind as DRK;
        for (func_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    if let ir::Instr::Binary { dest, .. } = instr {
                        int_const_map.remove(&(func_idx, *dest));
                    }
                    if let ir::Instr::Copy { dest, src } = instr {
                        if dest != src {
                            let dest_key = (func_idx, *dest);
                            string_map.remove(&dest_key);
                            dataref_kind_map.remove(&dest_key);
                            int_const_map.remove(&dest_key);
                            string_literal_temps.remove(&dest_key);
                            if let Some(val) = string_map.get(&(func_idx, *src)).cloned() {
                                string_map.insert(dest_key, val);
                            }
                            if let Some(kind) = dataref_kind_map.get(&(func_idx, *src)).copied() {
                                dataref_kind_map.insert(dest_key, kind);
                            }
                            if let Some(val) = int_const_map.get(&(func_idx, *src)).copied() {
                                int_const_map.insert(dest_key, val);
                            }
                            if string_literal_temps.contains(&(func_idx, *src)) {
                                string_literal_temps.insert(dest_key);
                            }
                        }
                        continue;
                    }
                    if let ir::Instr::StringConst { dest, value } = instr {
                        string_map.insert((func_idx, *dest), value.clone());
                        string_literal_temps.insert((func_idx, *dest));
                        dataref_kind_map.insert((func_idx, *dest), DRK::Blob);
                    }
                    if let ir::Instr::PointerFromString { dest, kind, src } = instr
                        && let Some(s) = string_map.get(&(func_idx, *src)).cloned()
                    {
                        string_map.insert((func_idx, *dest), s);
                        dataref_kind_map.insert((func_idx, *dest), *kind);
                    }
                    if let ir::Instr::Const { dest, value } = instr {
                        int_const_map.insert((func_idx, *dest), *value);
                    }
                    if let ir::Instr::Unary {
                        dest,
                        op: UnaryOp::Neg,
                        operand,
                    } = instr
                        && let Some(value) = int_const_map.get(&(func_idx, *operand)).copied()
                        && let Some(neg) = value.checked_neg()
                    {
                        int_const_map.insert((func_idx, *dest), neg);
                    }
                    if let ir::Instr::DataRef { dest, kind, value } = instr {
                        string_map.insert((func_idx, *dest), value.clone());
                        dataref_kind_map.insert((func_idx, *dest), *kind);
                    }
                    if let ir::Instr::PointerFromNorito { dest, kind, .. } = instr {
                        dataref_kind_map.insert((func_idx, *dest), *kind);
                    }
                    if let ir::Instr::PointerToNorito { dest, value } = instr {
                        dataref_kind_map.insert((func_idx, *dest), DRK::NoritoBytes);
                        let literal_kind = dataref_kind_map.get(&(func_idx, *value)).copied();
                        let literal_raw = string_map.get(&(func_idx, *value)).cloned();
                        if let (Some(kind), Some(raw)) = (literal_kind, literal_raw)
                            && let Some(tlv_bytes) = super::encode_pointer_tlv_bytes(kind, &raw)
                        {
                            let hex = hex::encode(tlv_bytes);
                            string_map.insert((func_idx, *dest), format!("0x{hex}"));
                        }
                    }
                    if let ir::Instr::LoadVar { dest, name } = instr
                        && let Some(param_idx) = func.params.iter().position(|p| p == name)
                    {
                        param_temp_map.entry((func_idx, param_idx)).or_insert(*dest);
                    }
                }
            }
        }

        let fn_index_by_name: HashMap<String, usize> = typed_functions
            .iter()
            .enumerate()
            .map(|(idx, func)| (func.name.clone(), idx))
            .collect();
        let mut literal_param_conflicts: HashSet<(usize, ir::Temp)> = HashSet::new();
        for (caller_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    if let Some((name, args)) = match instr {
                        ir::Instr::Call { callee, args, .. }
                        | ir::Instr::CallMulti { callee, args, .. } => {
                            Some((callee.as_str(), args.as_slice()))
                        }
                        _ => None,
                    } && let Some(&callee_idx) = fn_index_by_name.get(name)
                    {
                        let callee = &ir_prog.functions[callee_idx];
                        let count = usize::min(args.len(), callee.params.len());
                        for (i, &arg_temp) in args.iter().take(count).enumerate() {
                            let Some(&param_temp) = param_temp_map.get(&(callee_idx, i)) else {
                                continue;
                            };
                            let param_key = (callee_idx, param_temp);
                            if literal_param_conflicts.contains(&param_key) {
                                continue;
                            }
                            let arg_has_literal = string_literal_temps
                                .contains(&(caller_idx, arg_temp))
                                || dataref_kind_map.contains_key(&(caller_idx, arg_temp));
                            let Some(value) = string_map.get(&(caller_idx, arg_temp)).cloned()
                            else {
                                if string_map.contains_key(&param_key) {
                                    string_map.remove(&param_key);
                                    string_literal_temps.remove(&param_key);
                                    dataref_kind_map.remove(&param_key);
                                    literal_param_conflicts.insert(param_key);
                                }
                                continue;
                            };
                            if !arg_has_literal {
                                if string_map.contains_key(&param_key) {
                                    string_map.remove(&param_key);
                                    string_literal_temps.remove(&param_key);
                                    dataref_kind_map.remove(&param_key);
                                    literal_param_conflicts.insert(param_key);
                                }
                                continue;
                            }
                            if let Some(existing) = string_map.get(&param_key) {
                                if existing != &value {
                                    string_map.remove(&param_key);
                                    string_literal_temps.remove(&param_key);
                                    dataref_kind_map.remove(&param_key);
                                    literal_param_conflicts.insert(param_key);
                                    continue;
                                }
                            } else {
                                string_map.insert(param_key, value);
                            }
                            if string_literal_temps.contains(&(caller_idx, arg_temp)) {
                                string_literal_temps.insert(param_key);
                            }
                            if let Some(kind) =
                                dataref_kind_map.get(&(caller_idx, arg_temp)).copied()
                            {
                                dataref_kind_map.insert(param_key, kind);
                            }
                        }
                    }
                }
            }
        }

        let update_record_idx = ir_prog
            .functions
            .iter()
            .position(|func| func.name == "update_record")
            .expect("update_record index");
        let update_record = &ir_prog.functions[update_record_idx];
        let mut bases = Vec::new();
        for bb in &update_record.blocks {
            for instr in &bb.instrs {
                if let ir::Instr::PathMapKey { base, .. } = instr {
                    bases.push(
                        string_map
                            .get(&(update_record_idx, *base))
                            .cloned()
                            .expect("PathMapKey base should be a literal name"),
                    );
                }
            }
        }

        assert_eq!(
            bases,
            vec![
                "MintRequestSequences",
                "MintRequestRequestIds",
                "MintRequestFiIds",
                "MintRequestFiAuthorities",
                "MintRequestToAccounts",
                "MintRequestAmounts",
                "MintRequestRequestedBy",
                "MintRequestStates",
                "MintRequestCreatedAt",
                "MintRequestExpiresAt",
                "MintRequestFinalizedAt",
                "MintRequestCanceledAt",
            ]
        );
    }

    #[test]
    fn manifest_trigger_decl_lowers_structured_data_filter() {
        use iroha_data_model::events::{
            EventFilterBox,
            data::{
                DataEventFilter,
                prelude::{AssetEventFilter, AssetEventSet},
            },
        };

        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().expect("domain"),
            "rose".parse().expect("name"),
        );
        let asset_definition_literal = asset_definition.to_string();
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger intercept {{
    call run;
    on data asset added {{
      asset_definition "{asset_definition_literal}";
    }}
  }}
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let run = entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("run entrypoint");
        assert_eq!(run.triggers.len(), 1);
        assert_eq!(
            run.triggers[0].filter,
            EventFilterBox::Data(DataEventFilter::Asset(
                AssetEventFilter::new()
                    .for_events(AssetEventSet::Added)
                    .for_asset_definition(asset_definition),
            ))
        );
    }

    #[test]
    fn manifest_trigger_decl_lowers_structured_data_filters_for_core_families() {
        use iroha_data_model::{
            DomainId,
            account::{AccountId, ParsedAccountId},
            asset::AssetId,
            events::{
                EventFilterBox,
                data::{
                    DataEventFilter,
                    prelude::{
                        AccountEventFilter, AccountEventSet, AssetDefinitionEventFilter,
                        AssetDefinitionEventSet, AssetEventFilter, AssetEventSet,
                        ConfigurationEventFilter, ConfigurationEventSet, DomainEventFilter,
                        DomainEventSet, ExecutorEventFilter, ExecutorEventSet, NftEventFilter,
                        NftEventSet, PeerEventFilter, PeerEventSet, RoleEventFilter, RoleEventSet,
                        RwaEventFilter, RwaEventSet, TriggerEventFilter, TriggerEventSet,
                    },
                },
            },
            nft::NftId,
            peer::PeerId,
            role::RoleId,
            rwa::RwaId,
            trigger::TriggerId,
        };

        let account_literal = sample_account_literal();
        let account = AccountId::parse_encoded(account_literal.as_str())
            .map(ParsedAccountId::into_account_id)
            .expect("account");
        let peer_literal = "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D";
        let peer: PeerId = peer_literal.parse().expect("peer");
        let domain: DomainId = "wonderland".parse().expect("domain");
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().expect("domain"),
            "rose".parse().expect("name"),
        );
        let asset = AssetId::new(asset_definition.clone(), account.clone());
        let asset_literal = asset.canonical_literal();
        let nft: NftId = "n0$wonderland".parse().expect("nft");
        let rwa: RwaId = format!(
            "{}$wonderland",
            iroha_crypto::Hash::prehashed([7; iroha_crypto::Hash::LENGTH])
        )
        .parse()
        .expect("rwa");
        let trigger_id: TriggerId = "wake".parse().expect("trigger");
        let role_id: RoleId = "auditor".parse().expect("role");

        let cases = vec![
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data peer added {{
      peer "{peer_literal}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Peer(
                    PeerEventFilter::new()
                        .for_events(PeerEventSet::Added)
                        .for_peer(peer),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data domain created {{
      domain "{domain}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Domain(
                    DomainEventFilter::new()
                        .for_events(DomainEventSet::Created)
                        .for_domain(domain.clone()),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data account created {{
      account "{account_literal}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Account(
                    AccountEventFilter::new()
                        .for_events(AccountEventSet::Created)
                        .for_account(account.clone()),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data asset added {{
      asset "{asset_literal}";
      asset_definition "{asset_definition}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Asset(
                    AssetEventFilter::new()
                        .for_events(AssetEventSet::Added)
                        .for_asset(asset.clone())
                        .for_asset_definition(asset_definition.clone()),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data asset_definition created {{
      asset_definition "{asset_definition}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::AssetDefinition(
                    AssetDefinitionEventFilter::new()
                        .for_events(AssetDefinitionEventSet::Created)
                        .for_asset_definition(asset_definition.clone()),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data nft created {{
      nft "{nft}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Nft(
                    NftEventFilter::new()
                        .for_events(NftEventSet::Created)
                        .for_nft(nft),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data rwa created {{
      rwa "{rwa}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Rwa(
                    RwaEventFilter::new()
                        .for_events(RwaEventSet::Created)
                        .for_rwa(rwa),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data trigger created {{
      trigger "{trigger_id}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Trigger(
                    TriggerEventFilter::new()
                        .for_events(TriggerEventSet::Created)
                        .for_trigger(trigger_id),
                )),
            ),
            (
                format!(
                    r#"
seiyaku Test {{
  kotoage fn run() {{}}
  register_trigger wake {{
    call run;
    on data role created {{
      role "{role_id}";
    }}
  }}
}}
"#
                ),
                EventFilterBox::Data(DataEventFilter::Role(
                    RoleEventFilter::new()
                        .for_events(RoleEventSet::Created)
                        .for_role(role_id),
                )),
            ),
            (
                r#"
seiyaku Test {
  kotoage fn run() {}
  register_trigger wake {
    call run;
    on data configuration changed {}
  }
}
"#
                .to_string(),
                EventFilterBox::Data(DataEventFilter::Configuration(
                    ConfigurationEventFilter::new().for_events(ConfigurationEventSet::Changed),
                )),
            ),
            (
                r#"
seiyaku Test {
  kotoage fn run() {}
  register_trigger wake {
    call run;
    on data executor upgraded {}
  }
}
"#
                .to_string(),
                EventFilterBox::Data(DataEventFilter::Executor(
                    ExecutorEventFilter::new().for_events(ExecutorEventSet::Upgraded),
                )),
            ),
        ];

        let compiler = Compiler::new();
        for (src, expected_filter) in cases {
            let (_bytes, manifest) = compiler
                .compile_source_with_manifest(&src)
                .expect("compile manifest");
            let entrypoints = manifest.entrypoints.expect("entrypoints present");
            let run = entrypoints
                .iter()
                .find(|entry| entry.name == "run")
                .expect("run entrypoint");
            assert_eq!(run.triggers.len(), 1);
            assert_eq!(run.triggers[0].filter, expected_filter);
        }
    }

    #[test]
    fn manifest_trigger_decl_lowers_pipeline_filter() {
        use iroha_data_model::events::{
            EventFilterBox,
            pipeline::{BlockEventFilter, PipelineEventFilterBox},
        };

        let src = r#"
seiyaku Test {
  kotoage fn run() {}
  register_trigger block_wake {
    call run;
    on pipeline block;
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let run = entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("run entrypoint");
        assert_eq!(run.triggers.len(), 1);
        assert_eq!(
            run.triggers[0].filter,
            EventFilterBox::Pipeline(PipelineEventFilterBox::Block(BlockEventFilter::default(),))
        );
    }

    #[test]
    fn access_hint_diagnostics_report_isi_wildcards() {
        let src = r#"
seiyaku Test {
  kotoage fn move(from: AccountId, to: AccountId, asset: AssetDefinitionId, amount: int) permission(Admin) {
    transfer_asset(from, to, asset, amount);
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, _manifest, diag) = compiler
            .compile_source_with_manifest_and_diagnostics(src)
            .expect("compile manifest");
        assert!(diag.isi_wildcards > 0);
        assert_eq!(diag.state_wildcards, 0);
    }

    #[test]
    fn manifest_access_set_hints_include_explicit_access() {
        let account_literal = sample_account_literal();
        let account_key = format!("account:{account_literal}");
        let src = format!(
            r#"
seiyaku Test {{
  #[access(read="{account_key}", write="{account_key}")]
  kotoage fn move(from: AccountId, to: AccountId, asset: AssetDefinitionId, amount: int) permission(Admin) {{
    transfer_asset(from, to, asset, amount);
  }}
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(hints.read_keys.contains(&account_key));
        assert!(hints.write_keys.contains(&account_key));
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|e| e.name == "move")
            .expect("entrypoint present");
        assert!(main.read_keys.contains(&account_key));
        assert!(main.write_keys.contains(&account_key));
    }

    #[test]
    fn manifest_access_set_hints_include_literal_map_keys() {
        let src = r#"
seiyaku Test {
  state Foo: Map<int, int>;

  kotoage fn main() {
    Foo[1] = 2;
    let _x = Foo[1];
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(hints.read_keys.contains(&"state:Foo".to_string()));
        assert!(hints.read_keys.contains(&"state:Foo/1".to_string()));
        assert!(hints.write_keys.contains(&"state:Foo".to_string()));
        assert!(hints.write_keys.contains(&"state:Foo/1".to_string()));
    }

    #[test]
    fn manifest_access_set_hints_include_literal_numeric_map_keys() {
        let src = r#"
seiyaku Test {
  state Foo: Map<Amount, int>;

  kotoage fn main() {
    Foo[7] = 2;
    let _x = Foo[7];
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        let numeric = iroha_primitives::numeric::Numeric::new(7_u128, 0);
        let payload = norito::to_bytes(&numeric).expect("encode numeric");
        let raw = format!("0x{}", hex::encode(payload));
        let path = super::state_path_for_norito_key("Foo", &raw).expect("path");
        let expected = format!("state:{path}");
        assert!(hints.read_keys.contains(&expected));
        assert!(hints.read_keys.contains(&"state:Foo".to_string()));
        assert!(hints.write_keys.contains(&expected));
        assert!(hints.write_keys.contains(&"state:Foo".to_string()));
    }

    #[test]
    fn manifest_access_set_hints_include_literal_pointer_map_keys() {
        let src = r#"
seiyaku Test {
  state Foo: Map<Name, int>;

  kotoage fn main() {
    Foo[name("alice")] = 2;
    let _x = Foo[name("alice")];
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        let tlv = super::encode_pointer_tlv_bytes(super::ir::DataRefKind::Name, "alice")
            .expect("encode pointer tlv");
        let raw = format!("0x{}", hex::encode(tlv));
        let path = super::state_path_for_norito_key("Foo", &raw).expect("path");
        let expected = format!("state:{path}");
        assert!(hints.read_keys.contains(&expected));
        assert!(hints.read_keys.contains(&"state:Foo".to_string()));
        assert!(hints.write_keys.contains(&expected));
        assert!(hints.write_keys.contains(&"state:Foo".to_string()));
    }

    #[test]
    fn manifest_access_set_hints_include_create_trigger() {
        use std::str::FromStr;

        use iroha_data_model::{
            account::AccountId,
            events::{EventFilterBox, execute_trigger::ExecuteTriggerEventFilter},
            name::Name,
            transaction::{Executable, IvmBytecode},
            trigger::{
                Trigger, TriggerId,
                action::{Action, Repeats},
            },
        };

        let trigger_id = TriggerId::new(Name::from_str("wake").expect("trigger name"));
        let authority = AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .expect("public key"),
        );
        let filter = EventFilterBox::ExecuteTrigger(ExecuteTriggerEventFilter::new());
        let action = Action::new(
            Executable::Ivm(IvmBytecode::from_compiled(Vec::new())),
            Repeats::Indefinitely,
            authority,
            filter,
        );
        let trigger = Trigger::new(trigger_id.clone(), action);
        let json_value = norito::json::to_value(&trigger).expect("trigger json value");
        let raw_json = norito::json::to_string(&json_value).expect("trigger json");
        let escaped = raw_json.replace('\\', "\\\\").replace('"', "\\\"");
        let src = format!(
            "seiyaku Test {{ kotoage fn main() permission(Admin) {{ create_trigger(json(\"{escaped}\")); }} }}"
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        let trigger_key = format!("trigger:{trigger_id}");
        let repetitions_key = format!("trigger.repetitions:{trigger_id}");
        assert!(hints.read_keys.contains(&trigger_key));
        assert!(hints.write_keys.contains(&trigger_key));
        assert!(hints.write_keys.contains(&repetitions_key));
    }

    #[test]
    fn state_path_for_norito_key_hashes_payload() {
        let base = "Map";
        let raw = "0x6162";
        let digest: [u8; 32] = iroha_crypto::Hash::new(b"ab").into();
        let mut expected = String::from("Map/");
        use core::fmt::Write as _;
        for b in &digest {
            let _ = write!(&mut expected, "{b:02x}");
        }
        assert_eq!(
            super::state_path_for_norito_key(base, raw).as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn entry_spills_use_stack_frame() {
        // The compiler pipeline may use a few MiB of stack in debug builds; run this test on a
        // larger stack so it doesn't depend on the test harness' thread stack size.
        std::thread::Builder::new()
            .name("kotodama_entry_spills_use_stack_frame".to_owned())
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let mut src = String::from("seiyaku SpillTest {\n  fn main() -> int {\n");
                let count = 32;
                for i in 0..count {
                    let value = i + 1;
                    src.push_str(&format!("    let a{i} = {value};\n"));
                }
                src.push_str("    let sum = ");
                for i in 0..count {
                    if i > 0 {
                        src.push_str(" + ");
                    }
                    src.push_str(&format!("a{i}"));
                }
                src.push_str(";\n    return sum;\n  }\n}\n");

                let parsed = crate::parser::parse(&src).expect("parse spill test");
                let typed = crate::semantic::analyze(&parsed).expect("type spill test");
                let ir_prog = crate::ir::lower(&typed).expect("lower spill test");
                let func = ir_prog
                    .functions
                    .iter()
                    .find(|func| func.name == "main")
                    .expect("main function");
                let alloc = crate::regalloc::allocate(func);
                assert!(
                    !alloc.stack.is_empty(),
                    "expected spills to allocate stack slots"
                );
                assert!(alloc.frame_size > 0, "expected non-zero frame size");
            })
            .expect("spawn large-stack test thread")
            .join()
            .expect("test thread panicked");
    }
}

/// Convenience wrapper for encoding `rd = rs1 + rs2` using the canonical wide layout.
pub fn encode_add(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, rd, rs1, rs2)
}

/// Convenience wrapper for encoding `rd = rs1 - rs2` using the canonical wide layout.
pub fn encode_sub(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encoding::wide::encode_rr(instruction::wide::arithmetic::SUB, rd, rs1, rs2)
}

/// Encode `rd = rs1 + imm` using the canonical wide register–immediate format.
///
/// This helper is primarily used by the Kotodama code generator to materialize
/// small constants (e.g., `rd = imm` via `rs1 = x0`). Kotodama targets IVM
/// bytecode; the wide layout is the on-chain representation for the first release.
///
/// Example
/// -------
///
/// ```
/// use kotodama_lang::compiler::encode_addi;
/// let word = encode_addi(1, 1, 7).expect("addi"); // addi x1, x1, 7
/// assert_eq!(word, 0x2001_0107);
/// ```
pub fn encode_addi(rd: u8, rs1: u8, imm: i16) -> Result<u32, String> {
    if !(WIDE_IMM_MIN..=WIDE_IMM_MAX).contains(&(imm as i32)) {
        return Err(format!(
            "encode_addi immediate {imm} out of range; use emit_addi for chunked emission"
        ));
    }
    Ok(encoding::wide::encode_ri(
        instruction::wide::arithmetic::ADDI,
        rd,
        rs1,
        imm as i8,
    ))
}

/// Encode a 64-bit load (`rd <- [rs1 + imm]`) using the canonical wide layout.
#[inline]
pub fn encode_load64_rv(rd: u8, rs1: u8, imm: i16) -> Result<u32, String> {
    if !(WIDE_IMM_MIN..=WIDE_IMM_MAX).contains(&(imm as i32)) {
        return Err(format!(
            "encode_load64_rv offset {imm} out of wide range; use emit_load64"
        ));
    }
    Ok(encoding::wide::encode_load(
        instruction::wide::memory::LOAD64,
        rd,
        rs1,
        imm as i8,
    ))
}

/// Encode a 64-bit store (`[rs1 + imm] <- rs2`) using the canonical wide layout.
#[inline]
pub fn encode_store64_rv(rs1: u8, rs2: u8, imm: i16) -> Result<u32, String> {
    if !(WIDE_IMM_MIN..=WIDE_IMM_MAX).contains(&(imm as i32)) {
        return Err(format!(
            "encode_store64_rv offset {imm} out of wide range; use emit_store64"
        ));
    }
    Ok(encoding::wide::encode_store(
        instruction::wide::memory::STORE64,
        rs1,
        rs2,
        imm as i8,
    ))
}

/// Encode a branch using the canonical wide layout. `funct3` selects the branch condition.
/// Encoding for B‑type branches (BEQ/BNE/BLT/BGE/BLTU/BGEU).
pub fn encode_branch_rv(funct3: u8, rs1: u8, rs2: u8, imm: i16) -> Result<u32, String> {
    if (imm & 0x3) != 0 {
        return Err(format!(
            "encode_branch_rv requires word-aligned offset, got {imm}"
        ));
    }
    let offset_words = (imm / 4) as i32;
    if !(WIDE_IMM_MIN..=WIDE_IMM_MAX).contains(&offset_words) {
        return Err(format!(
            "encode_branch_rv offset {imm} out of wide range; use emit_branch"
        ));
    }
    let op = match funct3 {
        0x0 => instruction::wide::control::BEQ,
        0x1 => instruction::wide::control::BNE,
        0x4 => instruction::wide::control::BLT,
        0x5 => instruction::wide::control::BGE,
        0x6 => instruction::wide::control::BLTU,
        0x7 => instruction::wide::control::BGEU,
        other => {
            return Err(format!("unsupported branch funct3 {other}"));
        }
    };
    Ok(encoding::wide::encode_branch(
        op,
        rs1,
        rs2,
        offset_words as i8,
    ))
}

/// Encode a jump-and-link (`JAL`) in the canonical wide layout. Use `rd = 0` for a plain jump.
pub fn encode_jal(rd: u8, imm: i32) -> Result<u32, String> {
    if (imm % 4) != 0 {
        return Err(format!(
            "encode_jal requires word-aligned offset, got {imm}"
        ));
    }
    let offset_words = imm / 4;
    if !(-0x8000..=0x7fff).contains(&offset_words) {
        return Err(format!("encode_jal offset {imm} exceeds 16-bit word range"));
    }
    Ok(encoding::wide::encode_jump(
        instruction::wide::control::JAL,
        rd,
        offset_words as i16,
    ))
}

impl Compiler {
    /// Create a new compiler instance.
    pub fn new() -> Self {
        let lang = i18n::detect_language();
        Self {
            lang,
            opts: CompilerOptions::default(),
        }
    }

    /// Create a new compiler using a specific language.
    pub fn new_with_language(lang: Language) -> Self {
        Self {
            lang,
            opts: CompilerOptions::default(),
        }
    }

    /// Create a new compiler with custom options.
    pub fn new_with_options(opts: CompilerOptions) -> Self {
        let lang = i18n::detect_language();
        Self { lang, opts }
    }

    /// Set the ABI version that will be written into the program header.
    pub fn with_abi_version(mut self, abi_version: u8) -> Self {
        self.opts.abi_version = abi_version;
        self
    }

    /// Compile a KOTODAMA source file into IVM bytecode.
    pub fn compile_file<P: std::convert::AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<Vec<u8>, String> {
        let path_str = path.as_ref().display().to_string();
        let src = std::fs::read_to_string(&path).map_err(|e| {
            i18n::translate(self.lang, Message::ReadFile(&path_str, &e.to_string()))
        })?;
        self.compile_source(&src)
    }

    /// Compile a KOTODAMA source string into IVM bytecode.
    pub fn compile_source(&self, src: &str) -> Result<Vec<u8>, String> {
        let program =
            parser::parse(src).map_err(|e| i18n::translate(self.lang, Message::ParserError(&e)))?;
        self.compile(&program)
    }

    /// Compile a parsed [`Program`] into IVM bytecode.
    pub fn compile(&self, program: &Program) -> Result<Vec<u8>, String> {
        self.compile_program(program).map(|art| art.bytes)
    }

    fn compile_program(&self, program: &Program) -> Result<CompilationArtifacts, String> {
        let typed = semantic::analyze(program)
            .map_err(|e| i18n::translate(self.lang, Message::SemanticError(&e.message)))?;
        if self.opts.enforce_on_chain_profile
            && let Err(violations) = policy::enforce_on_chain_profile(&typed)
        {
            let message = violations
                .into_iter()
                .map(|err| err.message)
                .collect::<Vec<_>>()
                .join("\n");
            return Err(message);
        }
        // Validate features supported by the current code generator.
        validate_codegen_supported(&typed)?;
        // First release policy: support only ABI v1.
        let meta_decl = typed.contract_meta.as_ref();
        let abi_version = meta_decl
            .and_then(|m| m.abi_version)
            .unwrap_or(self.opts.abi_version);
        if abi_version != 1 {
            return Err(format!("unsupported abi_version {abi_version}; expected 1"));
        }
        let ir_prog = ir::lower_with_cap(&typed, self.opts.dynamic_iter_cap as usize)?;
        let durable_enabled = abi_version >= 1;
        // Choose the default entrypoint used when the VM starts execution at offset 0.
        // Trigger contracts must boot into their callback entrypoint, not a preceding private helper.
        let typed_functions: Vec<&semantic::TypedFunction> = typed
            .items
            .iter()
            .map(|item| match item {
                semantic::TypedItem::Function(func) => func,
            })
            .collect();
        let preferred_entry = typed
            .triggers
            .first()
            .map(|trigger| trigger.call.entrypoint.as_str())
            .or_else(|| {
                typed_functions
                    .iter()
                    .find(|func| func.name == "main")
                    .map(|func| func.name.as_str())
            })
            .or_else(|| {
                typed_functions
                    .iter()
                    .find(|func| func.modifiers.kind == FunctionKind::Hajimari)
                    .map(|func| func.name.as_str())
            })
            .or_else(|| {
                typed_functions
                    .iter()
                    .find(|func| entrypoint_kind_from_modifiers(&func.modifiers).is_some())
                    .map(|func| func.name.as_str())
            })
            .or_else(|| typed_functions.first().map(|func| func.name.as_str()))
            .ok_or_else(|| i18n::translate(self.lang, Message::NoFunctions))?;
        let entry_name = ir_prog
            .functions
            .iter()
            .find(|func| func.name == preferred_entry)
            .or_else(|| ir_prog.functions.first())
            .map(|func| func.name.clone())
            .ok_or_else(|| i18n::translate(self.lang, Message::NoFunctions))?;

        // Stage 1 pointer‑ABI: collect string constants and integer constants used by ops.
        use std::collections::{HashMap, HashSet};
        let mut string_map: HashMap<(usize, ir::Temp), String> = HashMap::new();
        let mut datarefs: Vec<(ir::DataRefKind, String)> = Vec::new();
        let mut int_const_map: HashMap<(usize, ir::Temp), i64> = HashMap::new();
        let mut param_temp_map: HashMap<(usize, usize), ir::Temp> = HashMap::new();
        let mut string_literal_temps: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut dataref_kind_map: HashMap<(usize, ir::Temp), ir::DataRefKind> = HashMap::new();
        let mut state_path_hints: HashMap<(usize, ir::Temp), StatePathHint> = HashMap::new();
        let mut norito_literal_map: HashMap<(usize, ir::Temp), String> = HashMap::new();
        let func_count = ir_prog.functions.len();
        let mut access_sets: Vec<AccessSets> = vec![AccessSets::default(); func_count];
        let mut hint_diagnostics = AccessHintDiagnostics::default();
        let mut uses_isi = false;
        use super::ir::DataRefKind as DRK;
        for (func_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    if instr_queues_isi(instr) {
                        uses_isi = true;
                    }
                    if let ir::Instr::Binary { dest, .. } = instr {
                        // Temps are mutable in loop lowerings (e.g., `i = i + 1`), so
                        // stale const facts must be dropped before codegen-time folding.
                        int_const_map.remove(&(func_idx, *dest));
                    }
                    if let ir::Instr::Copy { dest, src } = instr {
                        if dest != src {
                            let dest_key = (func_idx, *dest);
                            string_map.remove(&dest_key);
                            dataref_kind_map.remove(&dest_key);
                            state_path_hints.remove(&dest_key);
                            int_const_map.remove(&dest_key);
                            norito_literal_map.remove(&dest_key);
                            string_literal_temps.remove(&dest_key);
                            if let Some(val) = string_map.get(&(func_idx, *src)).cloned() {
                                string_map.insert(dest_key, val);
                            }
                            if let Some(kind) = dataref_kind_map.get(&(func_idx, *src)).copied() {
                                dataref_kind_map.insert(dest_key, kind);
                            }
                            if let Some(hint) = state_path_hints.get(&(func_idx, *src)).cloned() {
                                state_path_hints.insert(dest_key, hint);
                            }
                            if let Some(val) = int_const_map.get(&(func_idx, *src)).copied() {
                                int_const_map.insert(dest_key, val);
                            }
                            if let Some(val) = norito_literal_map.get(&(func_idx, *src)).cloned() {
                                norito_literal_map.insert(dest_key, val);
                            }
                            if string_literal_temps.contains(&(func_idx, *src)) {
                                string_literal_temps.insert(dest_key);
                            }
                        }
                        continue;
                    }
                    if let ir::Instr::StringConst { dest, value } = instr {
                        string_map.insert((func_idx, *dest), value.clone());
                        string_literal_temps.insert((func_idx, *dest));
                        dataref_kind_map.insert((func_idx, *dest), DRK::Blob);
                    }
                    if let ir::Instr::PointerFromString { dest, kind, src } = instr
                        && let Some(s) = string_map.get(&(func_idx, *src)).cloned()
                    {
                        string_map.insert((func_idx, *dest), s);
                        dataref_kind_map.insert((func_idx, *dest), *kind);
                    }
                    if let ir::Instr::Const { dest, value } = instr {
                        int_const_map.insert((func_idx, *dest), *value);
                    }
                    if let ir::Instr::Unary {
                        dest,
                        op: UnaryOp::Neg,
                        operand,
                    } = instr
                        && let Some(value) = int_const_map.get(&(func_idx, *operand)).copied()
                        && let Some(neg) = value.checked_neg()
                    {
                        int_const_map.insert((func_idx, *dest), neg);
                    }
                    if let ir::Instr::NumericFromInt { dest, value } = instr
                        && let Some(raw) = int_const_map.get(&(func_idx, *value)).copied()
                    {
                        let numeric = iroha_primitives::numeric::Numeric::new(raw, 0);
                        let payload = norito::to_bytes(&numeric).expect("encode numeric");
                        norito_literal_map
                            .insert((func_idx, *dest), format!("0x{}", hex::encode(payload)));
                    }
                    if let ir::Instr::DataRef { dest, kind, value } = instr {
                        // Track typed refs in string_map keyed by temp; kind is handled at use sites
                        string_map.insert((func_idx, *dest), value.clone());
                        datarefs.push((*kind, value.clone()));
                        dataref_kind_map.insert((func_idx, *dest), *kind);
                        if matches!(kind, DRK::Name) {
                            state_path_hints
                                .insert((func_idx, *dest), StatePathHint::Literal(value.clone()));
                        }
                    }
                    if let ir::Instr::PointerFromNorito { dest, kind, .. } = instr {
                        dataref_kind_map.insert((func_idx, *dest), *kind);
                    }
                    if let ir::Instr::PointerToNorito { dest, value } = instr {
                        dataref_kind_map.insert((func_idx, *dest), DRK::NoritoBytes);
                        let literal_kind = dataref_kind_map.get(&(func_idx, *value)).copied();
                        let literal_raw = string_map.get(&(func_idx, *value)).cloned();
                        if let (Some(kind), Some(raw)) = (literal_kind, literal_raw)
                            && let Some(tlv_bytes) = encode_pointer_tlv_bytes(kind, &raw)
                        {
                            let hex = hex::encode(tlv_bytes);
                            string_map.insert((func_idx, *dest), format!("0x{hex}"));
                        }
                    }
                    if let ir::Instr::LoadVar { dest, name } = instr
                        && let Some(param_idx) = func.params.iter().position(|p| p == name)
                    {
                        param_temp_map.entry((func_idx, param_idx)).or_insert(*dest);
                    }
                    if let ir::Instr::PathMapKey { dest, base, key } = instr
                        && let Some(base_hint) = state_path_hints.get(&(func_idx, *base)).cloned()
                    {
                        let map_base = base_hint.base_name();
                        if let Some(key_val) = int_const_map.get(&(func_idx, *key)).copied() {
                            let path = format!("{map_base}/{key_val}");
                            state_path_hints
                                .insert((func_idx, *dest), StatePathHint::Literal(path));
                        } else {
                            state_path_hints
                                .insert((func_idx, *dest), StatePathHint::Map { base: map_base });
                        }
                    }
                    if let ir::Instr::PathMapKeyNorito {
                        dest,
                        base,
                        key_blob,
                    } = instr
                        && let Some(base_hint) = state_path_hints.get(&(func_idx, *base)).cloned()
                    {
                        let map_base = base_hint.base_name();
                        let literal_path = string_map
                            .get(&(func_idx, *key_blob))
                            .and_then(|raw| {
                                dataref_kind_map
                                    .get(&(func_idx, *key_blob))
                                    .filter(|kind| matches!(**kind, DRK::NoritoBytes))
                                    .and_then(|_| state_path_for_norito_key(&map_base, raw))
                            })
                            .or_else(|| {
                                norito_literal_map
                                    .get(&(func_idx, *key_blob))
                                    .and_then(|raw| state_path_for_norito_key(&map_base, raw))
                            });
                        if let Some(path) = literal_path {
                            state_path_hints
                                .insert((func_idx, *dest), StatePathHint::Literal(path));
                        } else {
                            state_path_hints
                                .insert((func_idx, *dest), StatePathHint::Map { base: map_base });
                        }
                    }
                    match instr {
                        ir::Instr::StateGet { path, .. } => {
                            if let Some(key) =
                                render_state_hint(state_path_hints.get(&(func_idx, *path)))
                            {
                                insert_state_hint(&mut access_sets[func_idx].reads, key);
                            } else {
                                hint_diagnostics.state_wildcards =
                                    hint_diagnostics.state_wildcards.saturating_add(1);
                                access_sets[func_idx]
                                    .reads
                                    .insert(STATE_WILDCARD_KEY.to_string());
                                access_sets[func_idx]
                                    .writes
                                    .insert(STATE_WILDCARD_KEY.to_string());
                            }
                        }
                        ir::Instr::StateSet { path, .. } | ir::Instr::StateDel { path } => {
                            if let Some(key) =
                                render_state_hint(state_path_hints.get(&(func_idx, *path)))
                            {
                                insert_state_hint(&mut access_sets[func_idx].writes, key);
                            } else {
                                hint_diagnostics.state_wildcards =
                                    hint_diagnostics.state_wildcards.saturating_add(1);
                                access_sets[func_idx]
                                    .reads
                                    .insert(STATE_WILDCARD_KEY.to_string());
                                access_sets[func_idx]
                                    .writes
                                    .insert(STATE_WILDCARD_KEY.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Propagate string literals across call boundaries so callee parameters inherit literal metadata
        // only when every call site agrees on the same literal value.
        let mut fn_index_by_name: HashMap<&str, usize> = HashMap::new();
        for (idx, func) in ir_prog.functions.iter().enumerate() {
            fn_index_by_name.insert(&func.name, idx);
        }
        let mut explicit_hints_by_fn = vec![false; func_count];
        let explicit_hints_present = apply_explicit_access_hints(
            &typed,
            &fn_index_by_name,
            &mut access_sets,
            &mut explicit_hints_by_fn,
        );
        let mut literal_param_conflicts: HashSet<(usize, ir::Temp)> = HashSet::new();
        for (caller_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    if let Some((name, args)) = match instr {
                        ir::Instr::Call { callee, args, .. }
                        | ir::Instr::CallMulti { callee, args, .. } => {
                            Some((callee.as_str(), args.as_slice()))
                        }
                        _ => None,
                    } && let Some(&callee_idx) = fn_index_by_name.get(name)
                    {
                        let callee = &ir_prog.functions[callee_idx];
                        let count = usize::min(args.len(), callee.params.len());
                        for (i, &arg_temp) in args.iter().take(count).enumerate() {
                            let Some(&param_temp) = param_temp_map.get(&(callee_idx, i)) else {
                                continue;
                            };
                            let param_key = (callee_idx, param_temp);
                            if literal_param_conflicts.contains(&param_key) {
                                continue;
                            }
                            let arg_has_literal = string_literal_temps
                                .contains(&(caller_idx, arg_temp))
                                || dataref_kind_map.contains_key(&(caller_idx, arg_temp));
                            let Some(value) = string_map.get(&(caller_idx, arg_temp)).cloned()
                            else {
                                if string_map.contains_key(&param_key) {
                                    string_map.remove(&param_key);
                                    string_literal_temps.remove(&param_key);
                                    dataref_kind_map.remove(&param_key);
                                    literal_param_conflicts.insert(param_key);
                                }
                                continue;
                            };
                            if !arg_has_literal {
                                if string_map.contains_key(&param_key) {
                                    string_map.remove(&param_key);
                                    string_literal_temps.remove(&param_key);
                                    dataref_kind_map.remove(&param_key);
                                    literal_param_conflicts.insert(param_key);
                                }
                                continue;
                            }
                            if let Some(existing) = string_map.get(&param_key) {
                                if existing != &value {
                                    string_map.remove(&param_key);
                                    string_literal_temps.remove(&param_key);
                                    dataref_kind_map.remove(&param_key);
                                    literal_param_conflicts.insert(param_key);
                                    continue;
                                }
                            } else {
                                string_map.insert(param_key, value);
                            }
                            if string_literal_temps.contains(&(caller_idx, arg_temp)) {
                                string_literal_temps.insert(param_key);
                            }
                            if let Some(kind) =
                                dataref_kind_map.get(&(caller_idx, arg_temp)).copied()
                            {
                                dataref_kind_map.insert(param_key, kind);
                            }
                        }
                    }
                }
            }
        }

        for (func_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    if let ir::Instr::PointerFromString { kind, src, .. } = instr
                        && !string_map.contains_key(&(func_idx, *src))
                    {
                        let name = match kind {
                            ir::DataRefKind::Account => "account_id",
                            ir::DataRefKind::AssetDef => "asset_definition",
                            ir::DataRefKind::AssetId => "asset_id",
                            ir::DataRefKind::NftId => "nft_id",
                            ir::DataRefKind::Name => "name",
                            ir::DataRefKind::Json => "json",
                            ir::DataRefKind::Domain => "domain",
                            ir::DataRefKind::Blob => "blob",
                            ir::DataRefKind::NoritoBytes => "norito_bytes",
                            ir::DataRefKind::DataSpaceId => "dataspace_id",
                            ir::DataRefKind::AxtDescriptor => "axt_descriptor",
                            ir::DataRefKind::AssetHandle => "asset_handle",
                            ir::DataRefKind::ProofBlob => "proof_blob",
                        };
                        let msg = format!(
                            "{name} expects a string literal; pass a literal or Blob|bytes"
                        );
                        return Err(i18n::translate(self.lang, Message::SemanticError(&msg)));
                    }
                }
            }
        }

        if uses_isi {
            derive_isi_access_hints(
                &ir_prog,
                &string_map,
                &dataref_kind_map,
                &mut access_sets,
                &mut hint_diagnostics,
            );
        }
        let has_any_hints = access_sets
            .iter()
            .any(|set| !set.reads.is_empty() || !set.writes.is_empty());
        let include_hints = explicit_hints_present || has_any_hints;
        let mut hint_reports = Vec::with_capacity(func_count);
        for _ in 0..func_count {
            hint_reports.push(HintReport {
                emitted: include_hints,
                skipped_reasons: Vec::new(),
            });
        }

        // Data section builder and fixups.

        // Norito blobs for AccountId/AssetDefinitionId placed in data section
        let mut data_bytes: Vec<u8> = Vec::new();
        let mut data_offsets: HashMap<DataKey, u64> = HashMap::new();
        // Literal table fixups: each points to a pointer literal we will place right after metadata header
        let mut fixups: Vec<LiteralFixup> = Vec::new();

        // We now compile ALL functions and stitch them together. Track global code,
        // per-function start offsets, and fixups for inter-block control flow.
        let mut code: Vec<u8> = Vec::new();
        let mut uses_zk_global = false;
        let mut uses_vector_global = false;
        let mut call_fixups: Vec<(usize, String, String)> = Vec::new();
        let mut func_start_offsets: HashMap<String, usize> = HashMap::new();
        let mut function_debug_seeds: Vec<FunctionDebugSeed> = Vec::new();
        struct JumpFixup {
            at: usize,
            target_label: usize,
        }
        struct BranchFixup {
            jal_else_at: usize,
            else_label: usize,
            jal_then_at: usize,
            then_label: usize,
        }
        // Order: entry first, then remaining in declaration order.
        let mut ordered_funcs: Vec<(usize, &ir::Function)> = Vec::new();
        for (idx, f) in ir_prog.functions.iter().enumerate() {
            if f.name == entry_name {
                ordered_funcs.push((idx, f));
            }
        }
        for (idx, f) in ir_prog.functions.iter().enumerate() {
            if f.name != entry_name {
                ordered_funcs.push((idx, f));
            }
        }

        for (func_idx, func) in ordered_funcs {
            // Record start offset for call patching
            func_start_offsets.insert(func.name.clone(), code.len());
            let func_base = *func_start_offsets.get(&func.name).unwrap();
            let alloc = regalloc::allocate(func);
            // Treat all allocated temporaries as callee-saved: save/restore the
            // registers this function uses so caller live values survive calls.
            let mut saved_regs: Vec<u8> = alloc.regs.values().copied().map(|r| r as u8).collect();
            saved_regs.sort_unstable();
            saved_regs.dedup();
            let saved_size = saved_regs.len() * 8;
            let param_home_count = usize::min(func.params.len(), regalloc::ARG_REGS.len());
            let param_home_size = param_home_count * 8;
            let local_frame = alloc.frame_size + 8 + saved_size + param_home_size;
            function_debug_seeds.push(FunctionDebugSeed {
                name: func.name.clone(),
                location: func.location,
                pc_start: func_base as u64,
                frame_bytes: u32::try_from(local_frame).unwrap_or(u32::MAX),
            });
            let save_base = 8 + alloc.frame_size;
            let param_home_base = save_base + saved_size;
            // Determine if this function is the entry (no caller)
            let is_entry = func.name == entry_name;
            let mut uses_zk = false;
            // Scratch registers for spill shuttling and SP alias
            let scratch1: u8 = 27;
            let scratch2: u8 = 28;
            let scratchd: u8 = 29;
            let sp = regalloc::SP_REG as u8;
            let publish_tlv_word = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            );
            let publish_tlv = publish_tlv_word.to_le_bytes();
            let pointer_to_word = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_POINTER_TO_NORITO as u8,
            );
            let pointer_to_bytes = pointer_to_word.to_le_bytes();
            let pointer_from_word = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_POINTER_FROM_NORITO as u8,
            );
            let pointer_from_bytes = pointer_from_word.to_le_bytes();
            let durable_required_msg = "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.";

            // Helpers to handle spilled temporaries at use/def sites
            let src_reg = |t: &ir::Temp, scratch: u8, code: &mut Vec<u8>| -> Result<u8, String> {
                if let Some(r) = alloc.regs.get(t) {
                    Ok(*r as u8)
                } else if let Some(off) = alloc.stack.get(t) {
                    let total = stack_slot_offset_bytes(*off);
                    emit_load64(code, scratch, sp, total, Some(scratch))?;
                    Ok(scratch)
                } else {
                    Ok(0)
                }
            };
            let dst_reg = |t: &ir::Temp| -> (u8, bool, i64) {
                if let Some(r) = alloc.regs.get(t) {
                    (*r as u8, false, 0)
                } else if let Some(off) = alloc.stack.get(t) {
                    (scratchd, true, stack_slot_offset_bytes(*off))
                } else {
                    (scratchd, false, 0)
                }
            };
            let spill_back = |_: &ir::Temp,
                              from: u8,
                              spilled: bool,
                              offset: i64,
                              code: &mut Vec<u8>|
             -> Result<(), String> {
                if spilled {
                    emit_store64(code, sp, from, offset, scratch2)?;
                }
                Ok(())
            };

            let mut block_offsets: HashMap<usize, usize> = HashMap::new();
            let mut jump_fixups: Vec<JumpFixup> = Vec::new();
            let mut branch_fixups: Vec<BranchFixup> = Vec::new();

            // Tuple materialization map per function
            let mut tuple_map: std::collections::HashMap<ir::Temp, Vec<ir::Temp>> =
                Default::default();
            for bb in &func.blocks {
                block_offsets.insert(bb.label.0, code.len() - func_base);
                // Emit function prologue at entry block for non-entry functions
                if bb.label == func.entry && local_frame > 0 {
                    // Reserve space for spills + RA slot so entry functions can spill safely.
                    let sp = regalloc::SP_REG as u8;
                    emit_addi_inplace(&mut code, sp, -(local_frame as i64));
                    let scratch_base = if sp != scratch1 { scratch1 } else { scratch2 };
                    if !is_entry {
                        // Save RA (x1) at [SP+0] and callee-saved registers for non-entry calls.
                        let ra = 1u8;
                        emit_store64(&mut code, sp, ra, 0, scratch_base)?;
                        for (idx, reg) in saved_regs.iter().copied().enumerate() {
                            let offset = (save_base + idx * 8) as i64;
                            emit_store64(&mut code, sp, reg, offset, scratch_base)?;
                        }
                    }
                    // Home incoming arguments in the callee frame so later LoadVar reads
                    // remain stable after syscalls clobber the argument registers.
                    for (idx, reg) in regalloc::ARG_REGS
                        .iter()
                        .copied()
                        .enumerate()
                        .take(param_home_count)
                    {
                        let offset = (param_home_base + idx * 8) as i64;
                        emit_store64(&mut code, sp, reg as u8, offset, scratch_base)?;
                    }
                }
                for instr in &bb.instrs {
                    match instr {
                        Instr::StringConst { dest, value } => {
                            // Materialize string literals as Blob pointers via the literal table.
                            let (rd, spilled, imm) = dst_reg(dest);
                            let key = DataKey(DataKind::Blob, value.clone());
                            emit_literal_stub(&mut code, &mut fixups, rd, key);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Const { dest, value } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let imm_val = *value;
                            if ((WIDE_IMM_MIN as i64)..=(WIDE_IMM_MAX as i64)).contains(&imm_val) {
                                emit_addi(&mut code, rd, 0, imm_val);
                            } else {
                                const BASE_SHIFT: i16 = 11;
                                const BASE: i64 = 1 << BASE_SHIFT;
                                let mut digits: Vec<i16> = Vec::new();
                                let mut n = imm_val;
                                while n != 0 {
                                    let rem = n % BASE;
                                    digits.push(rem as i16);
                                    n = (n - rem) / BASE;
                                }
                                if digits.is_empty() {
                                    digits.push(0);
                                }
                                digits.reverse();
                                // Preload shift amount into the reserved literal scratch register.
                                emit_addi(&mut code, LITERAL_SHIFT_REG, 0, BASE_SHIFT as i64);
                                let mut iter = digits.into_iter();
                                if let Some(first) = iter.next() {
                                    emit_addi(&mut code, rd, 0, first as i64);
                                    for digit in iter {
                                        let shift = encoding::wide::encode_rr(
                                            instruction::wide::arithmetic::SLL,
                                            rd,
                                            rd,
                                            LITERAL_SHIFT_REG,
                                        );
                                        push_word(&mut code, shift);
                                        if digit != 0 {
                                            emit_addi(&mut code, rd, rd, digit as i64);
                                        }
                                    }
                                } else {
                                    emit_addi(&mut code, rd, 0, 0);
                                }
                            }
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::TuplePack { dest, items } => {
                            tuple_map.insert(*dest, items.clone());
                            // no code
                        }
                        Instr::TupleGet { dest, tuple, index } => {
                            // rd = item(index)
                            let (rd, spilled, imm) = dst_reg(dest);
                            let tuple_items = tuple_map.get(tuple).cloned();
                            if let Some(items) = tuple_items {
                                if let Some(src_t) = items.get(*index) {
                                    let rs = src_reg(src_t, scratch1, &mut code)?;
                                    emit_addi(&mut code, rd, rs, 0);
                                    if let Some(child_items) = tuple_map.get(src_t).cloned() {
                                        tuple_map.insert(*dest, child_items);
                                    } else {
                                        tuple_map.remove(dest);
                                    }
                                } else {
                                    // Out of bounds: move zero
                                    emit_addi(&mut code, rd, 0, 0);
                                    tuple_map.remove(dest);
                                }
                            } else {
                                // Unknown tuple: move zero
                                emit_addi(&mut code, rd, 0, 0);
                                tuple_map.remove(dest);
                            }
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Binary {
                            dest,
                            op,
                            left,
                            right,
                        } => {
                            if *op == BinaryOp::Add {
                                let left_zero =
                                    int_const_map.get(&(func_idx, *left)) == Some(&0i64);
                                let right_zero =
                                    int_const_map.get(&(func_idx, *right)) == Some(&0i64);
                                if let Some(kind) =
                                    dataref_kind_map.get(&(func_idx, *left)).copied()
                                    && let Some(lit) = string_map.get(&(func_idx, *left)).cloned()
                                    && right_zero
                                {
                                    let (rd, spilled, imm) = dst_reg(dest);
                                    let key = data_key_for_pointer(kind, &lit);
                                    emit_literal_stub(&mut code, &mut fixups, rd, key);
                                    spill_back(dest, rd, spilled, imm, &mut code)?;
                                    continue;
                                } else if let Some(kind) =
                                    dataref_kind_map.get(&(func_idx, *right)).copied()
                                    && let Some(lit) = string_map.get(&(func_idx, *right)).cloned()
                                    && left_zero
                                {
                                    let (rd, spilled, imm) = dst_reg(dest);
                                    let key = data_key_for_pointer(kind, &lit);
                                    emit_literal_stub(&mut code, &mut fixups, rd, key);
                                    spill_back(dest, rd, spilled, imm, &mut code)?;
                                    continue;
                                } else if left_zero || right_zero {
                                    let (rd, spilled, imm) = dst_reg(dest);
                                    let src = if left_zero { right } else { left };
                                    let rs = src_reg(src, scratch1, &mut code)?;
                                    push_word(&mut code, encode_addi(rd, rs, 0)?);
                                    spill_back(dest, rd, spilled, imm, &mut code)?;
                                    continue;
                                }
                            }
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(left, scratch1, &mut code)?;
                            let rs2 = src_reg(right, scratch2, &mut code)?;
                            // Pick scratch regs that don't clash with operands/dest
                            let pick_scratch = |cand: u8| -> u8 {
                                let mut s = cand;
                                while s == rd || s == rs1 || s == rs2 {
                                    s += 1;
                                }
                                s
                            };
                            match op {
                                BinaryOp::Add => {
                                    code.extend_from_slice(&encode_add(rd, rs1, rs2).to_le_bytes())
                                }
                                BinaryOp::Sub => {
                                    code.extend_from_slice(&encode_sub(rd, rs1, rs2).to_le_bytes())
                                }
                                BinaryOp::And => {
                                    let word = encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::AND,
                                        rd,
                                        rs1,
                                        rs2,
                                    );
                                    push_word(&mut code, word);
                                }
                                BinaryOp::Or => {
                                    let word = encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::OR,
                                        rd,
                                        rs1,
                                        rs2,
                                    );
                                    push_word(&mut code, word);
                                }
                                BinaryOp::Eq => {
                                    let word = encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::SEQ,
                                        rd,
                                        rs1,
                                        rs2,
                                    );
                                    push_word(&mut code, word);
                                }
                                BinaryOp::Ne => {
                                    let word = encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::SNE,
                                        rd,
                                        rs1,
                                        rs2,
                                    );
                                    push_word(&mut code, word);
                                }
                                BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                                    let (a, b, invert) = match op {
                                        BinaryOp::Lt => (rs1, rs2, false),
                                        BinaryOp::Gt => (rs2, rs1, false),
                                        BinaryOp::Le => (rs2, rs1, true),
                                        BinaryOp::Ge => (rs1, rs2, true),
                                        _ => unreachable!(),
                                    };
                                    let s1 = pick_scratch(12);
                                    let s2 = pick_scratch(13);
                                    push_word(
                                        &mut code,
                                        encoding::wide::encode_rr(
                                            instruction::wide::arithmetic::SUB,
                                            s1,
                                            a,
                                            b,
                                        ),
                                    );
                                    push_word(
                                        &mut code,
                                        encoding::wide::encode_ri(
                                            instruction::wide::arithmetic::ADDI,
                                            s2,
                                            0,
                                            63,
                                        ),
                                    );
                                    push_word(
                                        &mut code,
                                        encoding::wide::encode_rr(
                                            instruction::wide::arithmetic::SRA,
                                            rd,
                                            s1,
                                            s2,
                                        ),
                                    );
                                    push_word(
                                        &mut code,
                                        encoding::wide::encode_ri(
                                            instruction::wide::arithmetic::ANDI,
                                            rd,
                                            rd,
                                            1,
                                        ),
                                    );
                                    if invert {
                                        push_word(
                                            &mut code,
                                            encoding::wide::encode_ri(
                                                instruction::wide::arithmetic::XORI,
                                                rd,
                                                rd,
                                                1,
                                            ),
                                        );
                                    }
                                }
                                BinaryOp::Mul => push_word(
                                    &mut code,
                                    encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::MUL,
                                        rd,
                                        rs1,
                                        rs2,
                                    ),
                                ),
                                BinaryOp::Div => push_word(
                                    &mut code,
                                    encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::DIV,
                                        rd,
                                        rs1,
                                        rs2,
                                    ),
                                ),
                                BinaryOp::Mod => push_word(
                                    &mut code,
                                    encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::REM,
                                        rd,
                                        rs1,
                                        rs2,
                                    ),
                                ),
                            }
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Unary { dest, op, operand } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs = src_reg(operand, scratch1, &mut code)?;
                            match op {
                                UnaryOp::Neg => {
                                    push_word(
                                        &mut code,
                                        encoding::wide::encode_rr(
                                            instruction::wide::arithmetic::NEG,
                                            rd,
                                            rs,
                                            0,
                                        ),
                                    );
                                }
                                UnaryOp::Not => {
                                    // boolean not (0/1) via XORI with 1
                                    push_word(
                                        &mut code,
                                        encoding::wide::encode_ri(
                                            instruction::wide::arithmetic::XORI,
                                            rd,
                                            rs,
                                            1,
                                        ),
                                    );
                                }
                            }
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Abs { dest, src } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs = src_reg(src, scratch1, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::arithmetic::ABS,
                                rd,
                                rs,
                                0,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Min { dest, a, b } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(a, scratch1, &mut code)?;
                            let rs2 = src_reg(b, scratch2, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::arithmetic::MIN,
                                rd,
                                rs1,
                                rs2,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Max { dest, a, b } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(a, scratch1, &mut code)?;
                            let rs2 = src_reg(b, scratch2, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::arithmetic::MAX,
                                rd,
                                rs1,
                                rs2,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::DivCeil { dest, num, denom } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(num, scratch1, &mut code)?;
                            let rs2 = src_reg(denom, scratch2, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::arithmetic::DIV_CEIL,
                                rd,
                                rs1,
                                rs2,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Gcd { dest, a, b } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(a, scratch1, &mut code)?;
                            let rs2 = src_reg(b, scratch2, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::arithmetic::GCD,
                                rd,
                                rs1,
                                rs2,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Mean { dest, a, b } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(a, scratch1, &mut code)?;
                            let rs2 = src_reg(b, scratch2, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::arithmetic::MEAN,
                                rd,
                                rs1,
                                rs2,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Isqrt { dest, src } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs = src_reg(src, scratch1, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::arithmetic::ISQRT,
                                rd,
                                rs,
                                0,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Copy { dest, src } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            if let Some(kind) = dataref_kind_map.get(&(func_idx, *src)).copied()
                                && let Some(lit) = string_map.get(&(func_idx, *src)).cloned()
                            {
                                let key = data_key_for_pointer(kind, &lit);
                                emit_literal_stub(&mut code, &mut fixups, rd, key);
                            } else {
                                let rs = src_reg(src, scratch1, &mut code)?;
                                if rd != rs {
                                    push_word(&mut code, encode_addi(rd, rs, 0)?);
                                }
                            }
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::LoadVar { dest, name } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            let idx =
                                func.params.iter().position(|p| p == name).ok_or_else(|| {
                                    i18n::translate(self.lang, Message::UnknownParam(name))
                                })?;
                            if idx >= regalloc::ARG_REGS.len() {
                                return Err(format!(
                                    "too many return values in function: {} > {} (argument `{}` requires stack passing, which is not yet supported)",
                                    func.params.len(),
                                    regalloc::ARG_REGS.len(),
                                    name
                                ));
                            }
                            let scratch_base = if sp != scratch1 { scratch1 } else { scratch2 };
                            let offset = (param_home_base + idx * 8) as i64;
                            emit_load64(&mut code, rd, sp, offset, Some(scratch_base))?;
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Poseidon2 { dest, a, b } => {
                            uses_zk = true;
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(a, scratch1, &mut code)?;
                            let rs2 = src_reg(b, scratch2, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::crypto::POSEIDON2,
                                rd,
                                rs1,
                                rs2,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Pubkgen { dest, src } => {
                            uses_zk = true;
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs = src_reg(src, scratch1, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::crypto::PUBKGEN,
                                rd,
                                rs,
                                0,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Valcom { dest, value, blind } => {
                            uses_zk = true;
                            let (rd, spilled, imm) = dst_reg(dest);
                            let rs1 = src_reg(value, scratch1, &mut code)?;
                            let rs2 = src_reg(blind, scratch2, &mut code)?;
                            let word = encoding::wide::encode_rr(
                                instruction::wide::crypto::VALCOM,
                                rd,
                                rs1,
                                rs2,
                            );
                            push_word(&mut code, word);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::MintAsset {
                            account,
                            asset,
                            amount,
                        } => {
                            // Pointer-ABI: accept literal pointers (from string_map) or runtime pointers.
                            if int_const_map.contains_key(&(func_idx, *account))
                                || int_const_map.contains_key(&(func_idx, *asset))
                            {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "mint_asset expects (account, asset) pointers",
                                    ),
                                ));
                            }
                            // r10 = &AccountId
                            if let Some(k_acc) = string_map
                                .get(&(func_idx, *account))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, k_acc);
                            } else {
                                let r_acc = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_acc, 0)?);
                            }
                            // r11 = &AssetDefinitionId
                            if let Some(k_asset) = string_map
                                .get(&(func_idx, *asset))
                                .map(|s| DataKey(DataKind::AssetDef, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 11, k_asset);
                            } else {
                                let r_asset = src_reg(asset, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r_asset, 0)?);
                            }
                            // r12 = amount
                            let r_amt = src_reg(amount, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(12, r_amt, 0)?);

                            // Mirror TLVs for r10 and r11 into INPUT to satisfy pointer-ABI validation.
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            // Publish r10 and preserve it in x13.
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(13, 10, 0)?);
                            // Publish r11: x10 <- x11; publish; x11 <- x10.
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            // Publish r12 (amount): x10 <- x12; publish; x12 <- x10.
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            // Restore account pointer: x10 <- x13.
                            push_word(&mut code, encode_addi(10, 13, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_MINT_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::BurnAsset {
                            account,
                            asset,
                            amount,
                        } => {
                            let r_amt = src_reg(amount, scratch1, &mut code)?;
                            // r10 = &AccountId
                            if let Some(k_acc) = string_map
                                .get(&(func_idx, *account))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, k_acc);
                            } else {
                                let r_acc = src_reg(account, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_acc, 0)?);
                            }
                            // r11 = &AssetDefinitionId
                            if let Some(k_asset) = string_map
                                .get(&(func_idx, *asset))
                                .map(|s| DataKey(DataKind::AssetDef, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 11, k_asset);
                            } else {
                                let r_asset = src_reg(asset, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r_asset, 0)?);
                            }
                            push_word(&mut code, encode_addi(12, r_amt, 0)?);
                            // Mirror TLVs for r10 and r11 into INPUT to satisfy pointer‑ABI validation.
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            // Publish r10 and preserve it in x13.
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(13, 10, 0)?);
                            // Publish r11: x10 <- x11; publish; x11 <- x10.
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            // Publish r12 (amount): x10 <- x12; publish; x12 <- x10.
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            // Restore account pointer: x10 <- x13.
                            push_word(&mut code, encode_addi(10, 13, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_BURN_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::RegisterDomain { domain } => {
                            // Pointer-ABI: load DomainId TLV pointer into x10; or move from runtime pointer.
                            if let Some(dom_str) = string_map.get(&(func_idx, *domain)) {
                                let key_dom = DataKey(DataKind::Domain, dom_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_dom);
                            } else {
                                let r_dom = src_reg(domain, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_dom, 0)?);
                            }
                            // Mirror TLV into INPUT to satisfy pointer‑ABI validation in hosts.
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_REGISTER_DOMAIN as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::UnregisterDomain { domain } => {
                            if let Some(dom_str) = string_map.get(&(func_idx, *domain)) {
                                let key_dom = DataKey(DataKind::Domain, dom_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_dom);
                            } else {
                                let r_dom = src_reg(domain, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_dom, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_UNREGISTER_DOMAIN as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::UnregisterAccount { account } => {
                            if let Some(acc_str) = string_map.get(&(func_idx, *account)) {
                                let key_acc = DataKey(DataKind::Account, acc_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_acc);
                            } else {
                                let r = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_UNREGISTER_ACCOUNT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::RegisterAccount { account } => {
                            if let Some(acc_str) = string_map.get(&(func_idx, *account)) {
                                let key_acc = DataKey(DataKind::Account, acc_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_acc);
                            } else {
                                let r = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_REGISTER_ACCOUNT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::UnregisterAsset { asset } => {
                            if let Some(ad_str) = string_map.get(&(func_idx, *asset)) {
                                let key = DataKey(DataKind::AssetDef, ad_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(asset, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_UNREGISTER_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::TransferDomain { domain, to } => {
                            // Load domain into x10 and publish; keep a copy in x12
                            if let Some(dom_str) = string_map.get(&(func_idx, *domain)) {
                                let key_dom = DataKey(DataKind::Domain, dom_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_dom);
                            } else {
                                let r_dom = src_reg(domain, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_dom, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?); // x12 = x10

                            // Load 'to' AccountId into x11
                            if let Some(to_str) = string_map.get(&(func_idx, *to)) {
                                let key_to = DataKey(DataKind::Account, to_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key_to);
                            } else {
                                let r_to = src_reg(to, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r_to, 0)?);
                            }
                            // Publish 'to' TLV: x10 <- x11; publish; x11 <- x10
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            // Restore domain pointer: x10 <- x12
                            push_word(&mut code, encode_addi(10, 12, 0)?);

                            // SCALL transfer
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_TRANSFER_DOMAIN as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::RegisterPeer { json } => {
                            // r10 = &Json
                            if let Some(j) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, j.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_REGISTER_PEER as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::UnregisterPeer { json } => {
                            if let Some(j) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, j.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_UNREGISTER_PEER as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::CreateTrigger { json } => {
                            if let Some(j) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, j.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_CREATE_TRIGGER as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::RemoveTrigger { name } => {
                            if let Some(nm) = string_map.get(&(func_idx, *name)) {
                                let key = DataKey(DataKind::Name, nm.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(name, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_REMOVE_TRIGGER as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::SetTriggerEnabled { name, enabled } => {
                            if let Some(nm) = string_map.get(&(func_idx, *name)) {
                                let key = DataKey(DataKind::Name, nm.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(name, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // enabled value to r11
                            let r_en = src_reg(enabled, scratch2, &mut code)?;
                            push_word(&mut code, encode_addi(11, r_en, 0)?);
                            // Mirror name TLV
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SET_TRIGGER_ENABLED as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::CreateRole { name, json } => {
                            // r10 = &Name, r11 = &Json
                            if let Some(nm) = string_map.get(&(func_idx, *name)) {
                                let key = DataKey(DataKind::Name, nm.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(name, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(js) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, js.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key);
                            } else {
                                let r = src_reg(json, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_CREATE_ROLE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::DeleteRole { name } => {
                            if let Some(nm) = string_map.get(&(func_idx, *name)) {
                                let key = DataKey(DataKind::Name, nm.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(name, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_DELETE_ROLE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::GrantRole { account, name }
                        | Instr::RevokeRole { account, name } => {
                            // r10=&AccountId, r11=&Name
                            if let Some(a) = string_map.get(&(func_idx, *account)) {
                                let key = DataKey(DataKind::Account, a.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(nm) = string_map.get(&(func_idx, *name)) {
                                let key = DataKey(DataKind::Name, nm.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key);
                            } else {
                                let r = src_reg(name, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            let num = match instr {
                                Instr::GrantRole { .. } => syscalls::SYSCALL_GRANT_ROLE,
                                _ => syscalls::SYSCALL_REVOKE_ROLE,
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                num as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::GrantPermission { account, token }
                        | Instr::RevokePermission { account, token } => {
                            // r10 = &AccountId; r11 = &Name or &Json
                            if let Some(a) = string_map.get(&(func_idx, *account)) {
                                let key = DataKey(DataKind::Account, a.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // token pointer
                            if let Some(nm) = string_map.get(&(func_idx, *token)) {
                                // Assume Name unless starts with '{' then Json
                                let dk = if nm.starts_with("{") {
                                    DataKey(DataKind::Json, nm.clone())
                                } else {
                                    DataKey(DataKind::Name, nm.clone())
                                };
                                emit_literal_stub(&mut code, &mut fixups, 11, dk);
                            } else {
                                let r = src_reg(token, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            // Mirror both
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            let num = match instr {
                                Instr::GrantPermission { .. } => syscalls::SYSCALL_GRANT_PERMISSION,
                                _ => syscalls::SYSCALL_REVOKE_PERMISSION,
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                num as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::ZkVerify { number, payload } => {
                            // Load/move payload pointer into x10
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // Mirror into INPUT to satisfy pointer‑ABI validation
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                *number as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            uses_zk = true;
                        }
                        Instr::VendorExecuteInstruction { payload } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // Mirror into INPUT to satisfy pointer‑ABI validation
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::VendorExecuteQuery { dest, payload } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // Mirror into INPUT to satisfy pointer‑ABI validation
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::SubscriptionBill => {
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SUBSCRIPTION_BILL as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::SubscriptionRecordUsage => {
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::BuildSubmitBallotInline {
                            dest,
                            election_id,
                            ciphertext,
                            nullifier,
                            backend,
                            proof,
                            vk,
                        } => {
                            use iroha_data_model::{
                                isi::zk as DMZk,
                                proof::{ProofAttachment, ProofBox, VerifyingKeyBox},
                            };
                            let require_literal =
                                |label: &str, temp: &ir::Temp| -> Result<String, String> {
                                    if let Some(value) = string_map.get(&(func_idx, *temp)) {
                                        return Ok(value.clone());
                                    }
                                    let err = format!(
                                        "build_submit_ballot_inline requires literal {label}"
                                    );
                                    Err(i18n::translate(self.lang, Message::SemanticError(&err)))
                                };
                            let eid = require_literal("election_id", election_id)?;
                            let backend_str = require_literal("backend", backend)?;
                            let ct_literal = require_literal("ciphertext", ciphertext)?;
                            let ct_bytes = decode_hex_or_raw_bytes(&ct_literal).map_err(|e| {
                                let err =
                                    format!("build_submit_ballot_inline ciphertext literal {e}");
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?;
                            let nf_literal = require_literal("nullifier", nullifier)?;
                            let nf_bytes = decode_hex_or_raw_bytes(&nf_literal).map_err(|e| {
                                let err =
                                    format!("build_submit_ballot_inline nullifier literal {e}");
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?;
                            if nf_bytes.len() != 32 {
                                let err = "build_submit_ballot_inline nullifier must be 32 bytes"
                                    .to_string();
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::SemanticError(&err),
                                ));
                            }
                            let mut null32 = [0u8; 32];
                            null32.copy_from_slice(&nf_bytes);
                            let proof_literal = require_literal("proof", proof)?;
                            let proof_bytes =
                                decode_hex_or_raw_bytes(&proof_literal).map_err(|e| {
                                    let err =
                                        format!("build_submit_ballot_inline proof literal {e}");
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
                            let vk_literal = require_literal("vk", vk)?;
                            let vk_bytes = decode_hex_or_raw_bytes(&vk_literal).map_err(|e| {
                                let err = format!("build_submit_ballot_inline vk literal {e}");
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?;
                            let pa = ProofAttachment::new_inline(
                                backend_str.clone(),
                                ProofBox::new(backend_str.clone(), proof_bytes),
                                VerifyingKeyBox::new(backend_str, vk_bytes),
                            );
                            let sb = DMZk::SubmitBallot {
                                election_id: eid,
                                ciphertext: ct_bytes,
                                ballot_proof: pa,
                                nullifier: null32,
                            };
                            let bytes = sb.encode_as_instruction_box();
                            // Store as NoritoBytes in data and emit load into dest
                            let hex_payload = hex::encode(bytes);
                            let key = DataKey(DataKind::NoritoBytes, hex_payload);
                            let (rd, spilled, imm) = dst_reg(dest);
                            emit_literal_stub(&mut code, &mut fixups, rd, key);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::BuildUnshieldInline {
                            dest,
                            asset,
                            to,
                            amount,
                            inputs,
                            backend,
                            proof,
                            vk,
                        } => {
                            use iroha_data_model::{
                                isi::zk as DMZk,
                                prelude::*,
                                proof::{ProofAttachment, ProofBox, VerifyingKeyBox},
                            };
                            let require_literal =
                                |label: &str, temp: &ir::Temp| -> Result<String, String> {
                                    if let Some(value) = string_map.get(&(func_idx, *temp)) {
                                        return Ok(value.clone());
                                    }
                                    let err =
                                        format!("build_unshield_inline requires literal {label}");
                                    Err(i18n::translate(self.lang, Message::SemanticError(&err)))
                                };
                            let require_amount = |temp: &ir::Temp| -> Result<i64, String> {
                                if let Some(value) = int_const_map.get(&(func_idx, *temp)) {
                                    return Ok(*value);
                                }
                                let err =
                                    "build_unshield_inline requires literal amount".to_string();
                                Err(i18n::translate(self.lang, Message::SemanticError(&err)))
                            };
                            let asset_id_str = require_literal("asset", asset)?;
                            let to_str = require_literal("to", to)?;
                            let amt = require_amount(amount)?;
                            if amt < 0 {
                                let err = "build_unshield_inline requires non-negative amount"
                                    .to_string();
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::SemanticError(&err),
                                ));
                            }
                            let ad = AssetDefinitionId::parse_address_literal(&asset_id_str)
                                .map_err(|e| {
                                let err = format!(
                                    "build_unshield_inline invalid AssetDefinitionId literal `{asset_id_str}`: {e}"
                                );
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?;
                            let acct = AccountId::parse_encoded(&to_str)
                                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                                .map_err(|e| {
                                    let err = format!(
                                        "build_unshield_inline invalid AccountId literal `{to_str}`: {e}"
                                    );
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
                            // inputs: require exactly one 32-byte chunk
                            let inputs_literal = require_literal("inputs", inputs)?;
                            let in_bytes =
                                decode_hex_or_raw_bytes(&inputs_literal).map_err(|e| {
                                    let err = format!("build_unshield_inline inputs literal {e}");
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
                            if in_bytes.len() != 32 {
                                let err =
                                    "build_unshield_inline inputs must be 32 bytes".to_string();
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::SemanticError(&err),
                                ));
                            }
                            let mut one = [0u8; 32];
                            one.copy_from_slice(&in_bytes);
                            let ins = vec![one];
                            let backend_str = require_literal("backend", backend)?;
                            let proof_literal = require_literal("proof", proof)?;
                            let proof_bytes =
                                decode_hex_or_raw_bytes(&proof_literal).map_err(|e| {
                                    let err = format!("build_unshield_inline proof literal {e}");
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
                            let vk_literal = require_literal("vk", vk)?;
                            let vk_bytes = decode_hex_or_raw_bytes(&vk_literal).map_err(|e| {
                                let err = format!("build_unshield_inline vk literal {e}");
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?;
                            let pa = ProofAttachment::new_inline(
                                backend_str.clone(),
                                ProofBox::new(backend_str.clone(), proof_bytes),
                                VerifyingKeyBox::new(backend_str, vk_bytes),
                            );
                            let uz = DMZk::Unshield {
                                asset: ad,
                                to: acct,
                                public_amount: amt as u128,
                                inputs: ins,
                                proof: pa,
                                root_hint: None,
                            };
                            let bytes = uz.encode_as_instruction_box();
                            let hex_payload = hex::encode(bytes);
                            let key = DataKey(DataKind::NoritoBytes, hex_payload);
                            let (rd, spilled, imm) = dst_reg(dest);
                            emit_literal_stub(&mut code, &mut fixups, rd, key);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::RegisterAsset {
                            name,
                            symbol,
                            quantity,
                            mintable,
                        } => {
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            if let Some(name_str) = string_map.get(&(func_idx, *name)) {
                                let key_name = DataKey(DataKind::Name, name_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_name);
                            } else {
                                let r_name = src_reg(name, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_name, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let r_symbol = src_reg(symbol, scratch1, &mut code)?;
                            let r_qty = src_reg(quantity, scratch2, &mut code)?;
                            let r_mint = src_reg(mintable, scratchd, &mut code)?;
                            push_word(&mut code, encode_addi(11, r_symbol, 0)?);
                            push_word(&mut code, encode_addi(12, r_qty, 0)?);
                            push_word(&mut code, encode_addi(13, r_mint, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_REGISTER_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::CreateNewAsset {
                            name,
                            symbol,
                            quantity,
                            account,
                            mintable,
                        } => {
                            let r_name = src_reg(name, scratch1, &mut code)?;
                            let r_symbol = src_reg(symbol, scratch2, &mut code)?;
                            let r_qty = src_reg(quantity, scratchd, &mut code)?;
                            let r_mint = src_reg(mintable, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r_name, 0)?);
                            push_word(&mut code, encode_addi(11, r_symbol, 0)?);
                            push_word(&mut code, encode_addi(12, r_qty, 0)?);
                            push_word(&mut code, encode_addi(13, r_mint, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_REGISTER_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let r_acc = src_reg(account, scratch1, &mut code)?;
                            let r_name = src_reg(name, scratch2, &mut code)?;
                            let r_qty = src_reg(quantity, scratchd, &mut code)?;
                            push_word(&mut code, encode_addi(10, r_acc, 0)?);
                            push_word(&mut code, encode_addi(11, r_name, 0)?);
                            push_word(&mut code, encode_addi(12, r_qty, 0)?);
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            // Publish r10 and preserve it in x13.
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(13, 10, 0)?);
                            // Publish r11: x10 <- x11; publish; x11 <- x10.
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            // Publish r12: x10 <- x12; publish; x12 <- x10.
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            // Restore account pointer: x10 <- x13.
                            push_word(&mut code, encode_addi(10, 13, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_MINT_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::TransferAsset {
                            from,
                            to,
                            asset,
                            amount,
                        } => {
                            // Pointer-ABI: accept literal pointers (from string_map) or runtime pointers.
                            let r_amt = src_reg(amount, scratch1, &mut code)?;
                            if let Some(from_str) = string_map
                                .get(&(func_idx, *from))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, from_str);
                            } else {
                                let r_from = src_reg(from, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_from, 0)?);
                            }
                            if let Some(to_str) = string_map
                                .get(&(func_idx, *to))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 11, to_str);
                            } else {
                                let r_to = src_reg(to, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r_to, 0)?);
                            }
                            if let Some(asset_str) = string_map
                                .get(&(func_idx, *asset))
                                .map(|s| DataKey(DataKind::AssetDef, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 12, asset_str);
                            } else {
                                let r_asset = src_reg(asset, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(12, r_asset, 0)?);
                            }
                            push_word(&mut code, encode_addi(13, r_amt, 0)?);
                            // Mirror TLVs for r10, r11, r12 into INPUT
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            // r10
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Preserve the `from` account TLV pointer (x14) before it gets reused
                            push_word(&mut code, encode_addi(14, 10, 0)?);
                            // r11
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            // r12
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            // r13 (amount)
                            push_word(&mut code, encode_addi(10, 13, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(13, 10, 0)?);
                            // Restore `from` pointer into r10 before issuing the syscall
                            push_word(&mut code, encode_addi(10, 14, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_TRANSFER_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::TransferBatchBegin => {
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::TransferBatchEnd => {
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_TRANSFER_V1_BATCH_END as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::CreateNftsForAllUsers => {
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::SetExecutionDepth { value } => {
                            let r_val = src_reg(value, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r_val, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::SetVl { value } => {
                            let raw = int_const_map.get(&(func_idx, *value)).copied().ok_or_else(
                                || {
                                    let err =
                                        "setvl expects a literal int in range 0..=255".to_string();
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                },
                            )?;
                            if !(0..=u8::MAX as i64).contains(&raw) {
                                let err =
                                    format!("setvl value must be in range 0..=255, got {raw}");
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::SemanticError(&err),
                                ));
                            }
                            let word = encoding::wide::encode_rr(
                                instruction::wide::crypto::SETVL,
                                0,
                                0,
                                raw as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::SetAccountDetail {
                            account,
                            key,
                            value,
                        } => {
                            // Mixed strategy: if any argument was produced via DataRef/StringConst,
                            // emit a LOAD with a fixup into the appropriate register; otherwise move
                            // the value from its allocated register. This allows patterns like
                            // `set_account_detail(authority(), name("k"), json("{}"))` where only
                            // key/value are literals and account is provided by the host.
                            // r10 = &AccountId
                            if let Some(k_acc) = string_map
                                .get(&(func_idx, *account))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, k_acc);
                            } else {
                                let r_acc = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_acc, 0)?);
                            }
                            // r11 = &Name
                            if let Some(k_name) = string_map
                                .get(&(func_idx, *key))
                                .map(|s| DataKey(DataKind::Name, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 11, k_name);
                            } else {
                                let r_key = src_reg(key, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r_key, 0)?);
                            }
                            // r12 = &Json
                            if let Some(k_json) = string_map
                                .get(&(func_idx, *value))
                                .map(|s| DataKey(DataKind::Json, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 12, k_json);
                            } else {
                                let r_val = src_reg(value, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, r_val, 0)?);
                            }
                            // Mirror all three TLVs into INPUT to satisfy pointer‑ABI validation; preserve registers.
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            // Publish r10
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Preserve the account TLV pointer in x13 for the final syscall
                            push_word(&mut code, encode_addi(13, 10, 0)?);
                            // Publish r11: x10 <- x11; publish; x11 <- x10
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            // Publish r12: x10 <- x12; publish; x12 <- x10
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            // Restore account pointer into x10 before issuing the syscall
                            push_word(&mut code, encode_addi(10, 13, 0)?);

                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::CreateNft { nft, owner } => {
                            if let Some(k_nft) = string_map
                                .get(&(func_idx, *nft))
                                .map(|s| DataKey(DataKind::NftId, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, k_nft);
                            } else {
                                let r = src_reg(nft, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(k_owner) = string_map
                                .get(&(func_idx, *owner))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 11, k_owner);
                            } else {
                                let r = src_reg(owner, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            // Mirror TLVs into INPUT for r10 and r11
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NFT_MINT_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::SetNftData { nft, json } => {
                            // Load literals or move regs
                            let k_nft = string_map
                                .get(&(func_idx, *nft))
                                .map(|s| DataKey(DataKind::NftId, s.clone()));
                            let k_json = string_map
                                .get(&(func_idx, *json))
                                .map(|s| DataKey(DataKind::Json, s.clone()));
                            if let Some(kn) = k_nft {
                                emit_literal_stub(&mut code, &mut fixups, 10, kn);
                            } else {
                                let r = src_reg(nft, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(kj) = k_json {
                                emit_literal_stub(&mut code, &mut fixups, 11, kj);
                            } else {
                                let r = src_reg(json, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            // Mirror both into INPUT
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            // SCALL
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NFT_SET_METADATA as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::BurnNft { nft } => {
                            if let Some(kn) = string_map
                                .get(&(func_idx, *nft))
                                .map(|s| DataKey(DataKind::NftId, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, kn);
                            } else {
                                let r = src_reg(nft, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // Mirror into INPUT
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // SCALL
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NFT_BURN_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::TransferNft { from, nft, to } => {
                            if let Some(k_from) = string_map
                                .get(&(func_idx, *from))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, k_from);
                            } else {
                                let r = src_reg(from, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(k_nft) = string_map
                                .get(&(func_idx, *nft))
                                .map(|s| DataKey(DataKind::NftId, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 11, k_nft);
                            } else {
                                let r = src_reg(nft, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            if let Some(k_to) = string_map
                                .get(&(func_idx, *to))
                                .map(|s| DataKey(DataKind::Account, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 12, k_to);
                            } else {
                                let r = src_reg(to, scratchd, &mut code)?;
                                push_word(&mut code, encode_addi(12, r, 0)?);
                            }
                            // Mirror TLVs into INPUT for r10, r11, r12
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(13, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 13, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NFT_TRANSFER_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::DataRef { .. } => {
                            // No code emitted; data is accessed at use sites via fixups.
                        }
                        Instr::GetAuthority { dest } => {
                            // Request host to provide a pointer to the authority AccountId in x10
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_GET_AUTHORITY as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::CurrentTimeMs { dest } => {
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_CURRENT_TIME_MS as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::ResolveAccountAlias { dest, alias } => {
                            if let Some(alias_str) = string_map
                                .get(&(func_idx, *alias))
                                .map(|s| DataKey(DataKind::Blob, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, alias_str);
                            } else {
                                let r = src_reg(alias, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GetTriggerEvent { dest } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            emit_literal_stub(
                                &mut code,
                                &mut fixups,
                                10,
                                DataKey(DataKind::Name, TRIGGER_EVENT_PUBLIC_INPUT_KEY.to_string()),
                            );
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_GET_PUBLIC_INPUT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Call { callee, args, dest } => {
                            // Move args into conventional registers
                            for (i, a) in args.iter().enumerate() {
                                if i >= regalloc::ARG_REGS.len() {
                                    break;
                                }
                                let rd = regalloc::ARG_REGS[i] as u8;
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *a)).copied()
                                    && let Some(value) = string_map.get(&(func_idx, *a)).cloned()
                                {
                                    let key = data_key_for_pointer(kind, &value);
                                    emit_literal_stub(&mut code, &mut fixups, rd, key);
                                } else {
                                    let rs = src_reg(a, scratch1, &mut code)?;
                                    push_word(&mut code, encode_addi(rd, rs, 0)?);
                                }
                            }
                            // Reserve a fixed-size control-transfer stub so call fixups can
                            // choose between a direct JAL and a far JALR without shifting code.
                            let at = reserve_control_transfer_stub(&mut code);
                            call_fixups.push((at, callee.clone(), func.name.clone()));
                            // Move return value if needed
                            if let Some(d) = dest {
                                let (rd, spilled, imm) = dst_reg(d);
                                // Move return value in x10 into rd
                                push_word(&mut code, encode_addi(rd, 10, 0)?);
                                spill_back(d, rd, spilled, imm, &mut code)?;
                            }
                        }
                        Instr::CallMulti {
                            callee,
                            args,
                            dests,
                        } => {
                            if dests.len() > regalloc::MAX_RETURN_VALUES {
                                return Err(format!(
                                    "too many return values in call to {}: {} > {}",
                                    callee,
                                    dests.len(),
                                    regalloc::MAX_RETURN_VALUES
                                ));
                            }
                            // Move args into conventional registers
                            for (i, a) in args.iter().enumerate() {
                                if i >= regalloc::ARG_REGS.len() {
                                    break;
                                }
                                let rd = regalloc::ARG_REGS[i] as u8;
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *a)).copied()
                                    && let Some(value) = string_map.get(&(func_idx, *a)).cloned()
                                {
                                    let key = data_key_for_pointer(kind, &value);
                                    emit_literal_stub(&mut code, &mut fixups, rd, key);
                                } else {
                                    let rs = src_reg(a, scratch1, &mut code)?;
                                    push_word(&mut code, encode_addi(rd, rs, 0)?);
                                }
                            }
                            // Reserve a fixed-size control-transfer stub so call fixups can
                            // choose between a direct JAL and a far JALR without shifting code.
                            let at = reserve_control_transfer_stub(&mut code);
                            call_fixups.push((at, callee.clone(), func.name.clone()));
                            // Move return values r10.. into dest regs
                            for (i, d) in dests.iter().enumerate() {
                                let (rd, spilled, imm) = dst_reg(d);
                                let rs = (regalloc::RET_REG + i) as u8;
                                push_word(&mut code, encode_addi(rd, rs, 0)?);
                                spill_back(d, rd, spilled, imm, &mut code)?;
                            }
                        }
                        Instr::Poseidon6 { .. } => {
                            return Err("POSEIDON6 not supported".into());
                        }
                        Instr::Sm3Hash { dest, message } => {
                            if let Some(bytes) = string_map.get(&(func_idx, *message)) {
                                let key = DataKey(DataKind::Blob, bytes.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let rs = src_reg(message, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&publish.to_le_bytes());
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SM3_HASH as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Sha256Hash { dest, message } => {
                            if let Some(bytes) = string_map.get(&(func_idx, *message)) {
                                let key = DataKey(DataKind::Blob, bytes.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let rs = src_reg(message, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&publish.to_le_bytes());
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SHA256_HASH as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Sha3Hash { dest, message } => {
                            if let Some(bytes) = string_map.get(&(func_idx, *message)) {
                                let key = DataKey(DataKind::Blob, bytes.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let rs = src_reg(message, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&publish.to_le_bytes());
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SHA3_HASH as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Sm2Verify {
                            dest,
                            message,
                            signature,
                            public_key,
                            distid,
                        } => {
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            let load_blob_into_x10 =
                                |code: &mut Vec<u8>,
                                 fixups: &mut Vec<LiteralFixup>,
                                 temp: &ir::Temp|
                                 -> Result<(), String> {
                                    if let Some(bytes) = string_map.get(&(func_idx, *temp)) {
                                        let key = DataKey(DataKind::Blob, bytes.clone());
                                        emit_literal_stub(code, fixups, 10, key);
                                    } else {
                                        let rs = src_reg(temp, scratch1, code)?;
                                        push_word(code, encode_addi(10, rs, 0)?);
                                    }
                                    code.extend_from_slice(&publish.to_le_bytes());
                                    Ok(())
                                };
                            load_blob_into_x10(&mut code, &mut fixups, signature)?;
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            load_blob_into_x10(&mut code, &mut fixups, public_key)?;
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            if let Some(dist) = distid {
                                load_blob_into_x10(&mut code, &mut fixups, dist)?;
                                push_word(&mut code, encode_addi(13, 10, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(13, 0, 0)?);
                            }
                            load_blob_into_x10(&mut code, &mut fixups, message)?;
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SM2_VERIFY as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::VerifySignature {
                            dest,
                            message,
                            signature,
                            public_key,
                            scheme,
                        } => {
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            let load_blob_into_x10 =
                                |code: &mut Vec<u8>,
                                 fixups: &mut Vec<LiteralFixup>,
                                 temp: &ir::Temp|
                                 -> Result<(), String> {
                                    if let Some(bytes) = string_map.get(&(func_idx, *temp)) {
                                        let key = DataKey(DataKind::Blob, bytes.clone());
                                        emit_literal_stub(code, fixups, 10, key);
                                    } else {
                                        let rs = src_reg(temp, scratch1, code)?;
                                        push_word(code, encode_addi(10, rs, 0)?);
                                    }
                                    code.extend_from_slice(&publish.to_le_bytes());
                                    Ok(())
                                };
                            load_blob_into_x10(&mut code, &mut fixups, signature)?;
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            load_blob_into_x10(&mut code, &mut fixups, public_key)?;
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            let rs = src_reg(scheme, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(13, rs, 0)?);
                            load_blob_into_x10(&mut code, &mut fixups, message)?;
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_VERIFY_SIGNATURE as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Sm4GcmSeal {
                            dest,
                            key,
                            nonce,
                            aad,
                            plaintext,
                        } => {
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            let publish_bytes = publish.to_le_bytes();
                            let mut load_blob =
                                |temp: &ir::Temp, target: Option<u8>| -> Result<(), String> {
                                    if let Some(bytes) = string_map.get(&(func_idx, *temp)) {
                                        let key = DataKey(DataKind::Blob, bytes.clone());
                                        emit_literal_stub(&mut code, &mut fixups, 10, key);
                                    } else {
                                        let rs = src_reg(temp, scratch1, &mut code)?;
                                        push_word(&mut code, encode_addi(10, rs, 0)?);
                                    }
                                    code.extend_from_slice(&publish_bytes);
                                    if let Some(rd) = target {
                                        push_word(&mut code, encode_addi(rd, 10, 0)?);
                                    }
                                    Ok(())
                                };
                            load_blob(plaintext, Some(13))?;
                            load_blob(aad, Some(12))?;
                            load_blob(nonce, Some(11))?;
                            load_blob(key, None)?;
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SM4_GCM_SEAL as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Sm4GcmOpen {
                            dest,
                            key,
                            nonce,
                            aad,
                            ciphertext_and_tag,
                        } => {
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            let publish_bytes = publish.to_le_bytes();
                            let mut load_blob =
                                |temp: &ir::Temp, target: Option<u8>| -> Result<(), String> {
                                    if let Some(bytes) = string_map.get(&(func_idx, *temp)) {
                                        let key = DataKey(DataKind::Blob, bytes.clone());
                                        emit_literal_stub(&mut code, &mut fixups, 10, key);
                                    } else {
                                        let rs = src_reg(temp, scratch1, &mut code)?;
                                        push_word(&mut code, encode_addi(10, rs, 0)?);
                                    }
                                    code.extend_from_slice(&publish_bytes);
                                    if let Some(rd) = target {
                                        push_word(&mut code, encode_addi(rd, 10, 0)?);
                                    }
                                    Ok(())
                                };
                            load_blob(ciphertext_and_tag, Some(13))?;
                            load_blob(aad, Some(12))?;
                            load_blob(nonce, Some(11))?;
                            load_blob(key, None)?;
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SM4_GCM_OPEN as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Sm4CcmSeal {
                            dest,
                            key,
                            nonce,
                            aad,
                            plaintext,
                            tag_len,
                        } => {
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            let publish_bytes = publish.to_le_bytes();
                            let mut load_blob =
                                |temp: &ir::Temp, target: Option<u8>| -> Result<(), String> {
                                    if let Some(bytes) = string_map.get(&(func_idx, *temp)) {
                                        let key = DataKey(DataKind::Blob, bytes.clone());
                                        emit_literal_stub(&mut code, &mut fixups, 10, key);
                                    } else {
                                        let rs = src_reg(temp, scratch1, &mut code)?;
                                        push_word(&mut code, encode_addi(10, rs, 0)?);
                                    }
                                    code.extend_from_slice(&publish_bytes);
                                    if let Some(rd) = target {
                                        push_word(&mut code, encode_addi(rd, 10, 0)?);
                                    }
                                    Ok(())
                                };
                            load_blob(plaintext, Some(13))?;
                            load_blob(aad, Some(12))?;
                            load_blob(nonce, Some(11))?;
                            load_blob(key, None)?;
                            if let Some(tlen) = tag_len {
                                let rs = src_reg(tlen, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(14, rs, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(14, 0, 0)?);
                            }
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SM4_CCM_SEAL as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Sm4CcmOpen {
                            dest,
                            key,
                            nonce,
                            aad,
                            ciphertext_and_tag,
                            tag_len,
                        } => {
                            let publish = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            let publish_bytes = publish.to_le_bytes();
                            let mut load_blob =
                                |temp: &ir::Temp, target: Option<u8>| -> Result<(), String> {
                                    if let Some(bytes) = string_map.get(&(func_idx, *temp)) {
                                        let key = DataKey(DataKind::Blob, bytes.clone());
                                        emit_literal_stub(&mut code, &mut fixups, 10, key);
                                    } else {
                                        let rs = src_reg(temp, scratch1, &mut code)?;
                                        push_word(&mut code, encode_addi(10, rs, 0)?);
                                    }
                                    code.extend_from_slice(&publish_bytes);
                                    if let Some(rd) = target {
                                        push_word(&mut code, encode_addi(rd, 10, 0)?);
                                    }
                                    Ok(())
                                };
                            load_blob(ciphertext_and_tag, Some(13))?;
                            load_blob(aad, Some(12))?;
                            load_blob(nonce, Some(11))?;
                            load_blob(key, None)?;
                            if let Some(tlen) = tag_len {
                                let rs = src_reg(tlen, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(14, rs, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(14, 0, 0)?);
                            }
                            let call = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SM4_CCM_OPEN as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::AssertEq { left, right } => {
                            let rs1 = src_reg(left, scratch1, &mut code)?;
                            let rs2 = src_reg(right, scratch2, &mut code)?;
                            // Skip ABORT when the values are equal.
                            let skip_word = encode_branch_rv(0x0, rs1, rs2, 8)?;
                            push_word(&mut code, skip_word);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ABORT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::Assert { cond } => {
                            let rs = src_reg(cond, scratch1, &mut code)?;
                            // Skip ABORT when the condition is false (i.e., == 0).
                            let skip_word = encode_branch_rv(0x0, rs, 0, 8)?;
                            push_word(&mut code, skip_word);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ABORT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::AbortIf { cond } => {
                            let rs = src_reg(cond, scratch1, &mut code)?;
                            // Skip ABORT if the condition is false (i.e., == 0).
                            // Branch offsets are relative to the branch PC, so 8 bytes skips
                            // over the following single instruction.
                            let skip_word = encode_branch_rv(0x0, rs, 0, 8)?;
                            push_word(&mut code, skip_word);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ABORT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::Info { msg } => {
                            let r_msg = src_reg(msg, scratch1, &mut code)?;
                            // Move message to x10 and issue debug log syscall (RV-compat ADDI)
                            push_word(&mut code, encode_addi(10, r_msg, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_DEBUG_LOG as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::MapNew { dest } => {
                            let (rd, spilled, imm) = dst_reg(dest);
                            // Request 16 bytes plus alignment slop (8 bytes) in case the heap
                            // baseline is not aligned at 8-byte granularity.
                            emit_addi(&mut code, 10, 0, 24);
                            // SCALL ALLOC
                            let sys = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ALLOC as u8,
                            );
                            code.extend_from_slice(&sys.to_le_bytes());
                            // Align x10 to the next 8-byte boundary: x10 = (x10 + 7) & !7
                            emit_addi(&mut code, 10, 10, 7);
                            let andi = encoding::wide::encode_ri(
                                instruction::wide::arithmetic::ANDI,
                                10,
                                10,
                                -8,
                            );
                            code.extend_from_slice(&andi.to_le_bytes());
                            // Zero-initialize the single key/value pair to keep Map::new deterministic.
                            emit_addi(&mut code, scratch1, 0, 0);
                            emit_store64(&mut code, 10, scratch1, 0, scratch2)?;
                            emit_store64(&mut code, 10, scratch1, 8, scratch2)?;
                            // dest = x10
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::PointerFromString { .. } => {
                            // Marker instruction; literal data handled at use-sites via fixups/string_map.
                        }
                        Instr::PointerToNorito { dest, value } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let pointer_kind = dataref_kind_map.get(&(func_idx, *value)).copied();
                            if let Some(kind) = pointer_kind
                                && let Some(lit) = string_map.get(&(func_idx, *value)).cloned()
                            {
                                let key = data_key_for_pointer(kind, &lit);
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                if string_map.contains_key(&(func_idx, *value))
                                    && pointer_kind.is_none()
                                {
                                    return Err(i18n::translate(
                                        self.lang,
                                        Message::SemanticError(
                                            "pointer literal missing ABI metadata during pointer_to_norito lowering",
                                        ),
                                    ));
                                }
                                let rs = src_reg(value, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            code.extend_from_slice(&publish_tlv);
                            code.extend_from_slice(&pointer_to_bytes);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::PointerFromNorito { dest, blob, kind } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let type_id = pointer_type_for_kind(*kind).ok_or_else(|| {
                                i18n::translate(
                                    self.lang,
                                    Message::SemanticError(
                                        "unsupported pointer type for pointer_from_norito",
                                    ),
                                )
                            })? as u16;
                            if let Some(bytes) = string_map.get(&(func_idx, *blob)).cloned() {
                                let key = DataKey(DataKind::NoritoBytes, bytes);
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let rs = src_reg(blob, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            code.extend_from_slice(&publish_tlv);
                            emit_addi(&mut code, 11, 0, type_id as i64);
                            code.extend_from_slice(&pointer_from_bytes);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::PointerEq { dest, left, right } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            // Mirror both pointers into INPUT so TLV_EQ validates INPUT-resident TLVs.
                            let mut load_ptr = |temp: &ir::Temp,
                                                target: u8,
                                                scratch: u8,
                                                code: &mut Vec<u8>|
                             -> Result<(), String> {
                                if let Some(kind) =
                                    dataref_kind_map.get(&(func_idx, *temp)).copied()
                                    && let Some(lit) = string_map.get(&(func_idx, *temp)).cloned()
                                {
                                    let key = data_key_for_pointer(kind, &lit);
                                    emit_literal_stub(code, &mut fixups, target, key);
                                } else {
                                    let rs = src_reg(temp, scratch, code)?;
                                    push_word(code, encode_addi(target, rs, 0)?);
                                }
                                Ok(())
                            };
                            load_ptr(left, 10, scratch1, &mut code)?;
                            code.extend_from_slice(&publish_tlv);
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            load_ptr(right, 10, scratch2, &mut code)?;
                            code.extend_from_slice(&publish_tlv);

                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_TLV_EQ as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());

                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::MapGet { dest, map, key } => {
                            // Minimal map layout: [0..8) key (u64), [8..16) value (u64)
                            // Branchless compare/select via flag multiply:
                            //   flag := SEQ(LOAD64 [map + 0], key)
                            //   dest := LOAD64 [map + 8]
                            //   dest := dest * flag

                            let rmap = src_reg(map, scratch1, &mut code)?;
                            let rkey = src_reg(key, scratch2, &mut code)?;
                            let (rd, spilled, imm) = dst_reg(dest);

                            let mut flag_reg = None;
                            for cand in [scratch1, scratch2, scratchd] {
                                if cand != rmap && cand != rkey && cand != rd {
                                    flag_reg = Some(cand);
                                    break;
                                }
                            }

                            if let Some(rflag) = flag_reg {
                                emit_load64(&mut code, rflag, rmap, 0, None)?;
                                let eq = encoding::wide::encode_rr(
                                    instruction::wide::arithmetic::SEQ,
                                    rflag,
                                    rflag,
                                    rkey,
                                );
                                push_word(&mut code, eq);

                                let value_scratch = if rd == rmap {
                                    Some(if rflag != scratch1 {
                                        scratch1
                                    } else {
                                        scratch2
                                    })
                                } else {
                                    None
                                };
                                emit_load64(&mut code, rd, rmap, 8, value_scratch)?;
                                push_word(
                                    &mut code,
                                    encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::MUL,
                                        rd,
                                        rd,
                                        rflag,
                                    ),
                                );
                            } else {
                                // Fall back to using rd for the flag and reuse rkey (spilled) for value.
                                emit_load64(&mut code, rd, rmap, 0, None)?;
                                let eq = encoding::wide::encode_rr(
                                    instruction::wide::arithmetic::SEQ,
                                    rd,
                                    rd,
                                    rkey,
                                );
                                push_word(&mut code, eq);
                                emit_load64(&mut code, rkey, rmap, 8, None)?;
                                push_word(
                                    &mut code,
                                    encoding::wide::encode_rr(
                                        instruction::wide::arithmetic::MUL,
                                        rd,
                                        rkey,
                                        rd,
                                    ),
                                );
                            }
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Load64Imm { dest, base, imm } => {
                            let rbase = src_reg(base, scratch1, &mut code)?;
                            let (rd, spilled, imm_spill) = dst_reg(dest);
                            let scratch = if rd == rbase {
                                Some(if rd != scratch1 { scratch1 } else { scratch2 })
                            } else {
                                None
                            };
                            emit_load64(&mut code, rd, rbase, *imm as i64, scratch)?;
                            spill_back(dest, rd, spilled, imm_spill, &mut code)?;
                        }
                        Instr::MapSet { map, key, value } => {
                            // Minimal map layout: [0..8) key, [8..16) value
                            let rmap = src_reg(map, scratch1, &mut code)?;
                            let rkey = src_reg(key, scratch2, &mut code)?;
                            let rval = src_reg(value, scratchd, &mut code)?;
                            // Encode 64-bit store of key at offset 0
                            let scratch_base = if rmap != scratch1 { scratch1 } else { scratch2 };
                            emit_store64(&mut code, rmap, rkey, 0, scratch_base)?;
                            // Encode 64-bit store of value at offset 8
                            emit_store64(&mut code, rmap, rval, 8, scratch_base)?;
                        }
                        Instr::MapLoadPair {
                            dest_key,
                            dest_val,
                            map,
                            offset,
                        } => {
                            // Load key at offset 0, value at offset 8
                            let rmap = src_reg(map, scratch1, &mut code)?;
                            let (rd_k, spilled_k, imm_k) = dst_reg(dest_key);
                            let (rd_v, spilled_v, imm_v) = dst_reg(dest_val);
                            let base_off = *offset as i64; // in bytes

                            let key_scratch = if rd_k == rmap {
                                Some(if rmap != scratch1 { scratch1 } else { scratch2 })
                            } else {
                                None
                            };
                            emit_load64(&mut code, rd_k, rmap, base_off, key_scratch)?;
                            spill_back(dest_key, rd_k, spilled_k, imm_k, &mut code)?;

                            let val_scratch = if rd_v == rmap {
                                Some(if rmap != scratch1 { scratch1 } else { scratch2 })
                            } else {
                                None
                            };
                            emit_load64(&mut code, rd_v, rmap, base_off + 8, val_scratch)?;
                            spill_back(dest_val, rd_v, spilled_v, imm_v, &mut code)?;
                        }
                        Instr::StateGet { dest, path } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // Load path (Name) into x10; publish into INPUT; SCALL STATE_GET; move x10 to dest
                            if let Some(s) = string_map.get(&(func_idx, *path)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(path, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_STATE_GET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::StateSet { path, value } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Name path; r11=&NoritoBytes value; publish both to INPUT then SCALL
                            if let Some(s) = string_map.get(&(func_idx, *path)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(path, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // Load value into r11
                            if let Some(s) = string_map.get(&(func_idx, *value)) {
                                let key = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key);
                            } else {
                                let r = src_reg(value, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            // Publish both; preserve published path for the final syscall.
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_STATE_SET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::StateDel { path } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Name path; publish; SCALL
                            if let Some(s) = string_map.get(&(func_idx, *path)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(path, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_STATE_DEL as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::DecodeInt { dest, blob } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&NoritoBytes or &Blob; publish; SCALL; move to dest
                            if let Some(s) = string_map.get(&(func_idx, *blob)) {
                                let key = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(blob, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_DECODE_INT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::PathMapKey { dest, base, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Name base; publish; r11=key; SCALL BUILD_PATH_MAP_KEY; move to dest
                            if let Some(s) = string_map.get(&(func_idx, *base)) {
                                let key_b = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_b);
                            } else {
                                let r = src_reg(base, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            // publish base name
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // move key (int) into r11
                            let rkey = src_reg(key, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(11, rkey, 0)?);
                            // build path
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_BUILD_PATH_MAP_KEY as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            // move r10 to dest
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::EncodeInt { dest, value } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            let rv = src_reg(value, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, rv, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ENCODE_INT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::NumericFromInt { dest, value } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let rv = src_reg(value, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, rv, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NUMERIC_FROM_INT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::NumericToInt { dest, value } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            if let Some(s) = string_map.get(&(func_idx, *value)) {
                                let key = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(value, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NUMERIC_TO_INT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::NumericBinary {
                            dest,
                            op,
                            left,
                            right,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let mut load_ptr = |temp: &ir::Temp,
                                                target: u8,
                                                scratch: u8,
                                                code: &mut Vec<u8>|
                             -> Result<(), String> {
                                if let Some(kind) =
                                    dataref_kind_map.get(&(func_idx, *temp)).copied()
                                    && let Some(lit) = string_map.get(&(func_idx, *temp)).cloned()
                                {
                                    let key = data_key_for_pointer(kind, &lit);
                                    emit_literal_stub(code, &mut fixups, target, key);
                                } else {
                                    if string_map.contains_key(&(func_idx, *temp))
                                        && !dataref_kind_map.contains_key(&(func_idx, *temp))
                                    {
                                        return Err(i18n::translate(
                                            self.lang,
                                            Message::SemanticError(
                                                "numeric literal missing ABI metadata during numeric lowering",
                                            ),
                                        ));
                                    }
                                    let rs = src_reg(temp, scratch, code)?;
                                    push_word(code, encode_addi(target, rs, 0)?);
                                }
                                Ok(())
                            };
                            // Load/publish lhs
                            load_ptr(left, 10, scratch1, &mut code)?;
                            code.extend_from_slice(&publish_tlv);
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            // Load/publish rhs
                            load_ptr(right, 10, scratch1, &mut code)?;
                            code.extend_from_slice(&publish_tlv);
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            // Restore lhs into r10
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let num = match op {
                                BinaryOp::Add => syscalls::SYSCALL_NUMERIC_ADD,
                                BinaryOp::Sub => syscalls::SYSCALL_NUMERIC_SUB,
                                BinaryOp::Mul => syscalls::SYSCALL_NUMERIC_MUL,
                                BinaryOp::Div => syscalls::SYSCALL_NUMERIC_DIV,
                                BinaryOp::Mod => syscalls::SYSCALL_NUMERIC_REM,
                                _ => {
                                    return Err(i18n::translate(
                                        self.lang,
                                        Message::SemanticError(
                                            "numeric binary expects arithmetic operator",
                                        ),
                                    ));
                                }
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                num as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::NumericCompare {
                            dest,
                            op,
                            left,
                            right,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let mut load_ptr = |temp: &ir::Temp,
                                                target: u8,
                                                scratch: u8,
                                                code: &mut Vec<u8>|
                             -> Result<(), String> {
                                if let Some(kind) =
                                    dataref_kind_map.get(&(func_idx, *temp)).copied()
                                    && let Some(lit) = string_map.get(&(func_idx, *temp)).cloned()
                                {
                                    let key = data_key_for_pointer(kind, &lit);
                                    emit_literal_stub(code, &mut fixups, target, key);
                                } else {
                                    if string_map.contains_key(&(func_idx, *temp))
                                        && !dataref_kind_map.contains_key(&(func_idx, *temp))
                                    {
                                        return Err(i18n::translate(
                                            self.lang,
                                            Message::SemanticError(
                                                "numeric literal missing ABI metadata during numeric lowering",
                                            ),
                                        ));
                                    }
                                    let rs = src_reg(temp, scratch, code)?;
                                    push_word(code, encode_addi(target, rs, 0)?);
                                }
                                Ok(())
                            };
                            load_ptr(left, 10, scratch1, &mut code)?;
                            code.extend_from_slice(&publish_tlv);
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            load_ptr(right, 10, scratch1, &mut code)?;
                            code.extend_from_slice(&publish_tlv);
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let num = match op {
                                BinaryOp::Eq => syscalls::SYSCALL_NUMERIC_EQ,
                                BinaryOp::Ne => syscalls::SYSCALL_NUMERIC_NE,
                                BinaryOp::Lt => syscalls::SYSCALL_NUMERIC_LT,
                                BinaryOp::Le => syscalls::SYSCALL_NUMERIC_LE,
                                BinaryOp::Gt => syscalls::SYSCALL_NUMERIC_GT,
                                BinaryOp::Ge => syscalls::SYSCALL_NUMERIC_GE,
                                _ => {
                                    return Err(i18n::translate(
                                        self.lang,
                                        Message::SemanticError(
                                            "numeric compare expects comparison operator",
                                        ),
                                    ));
                                }
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                num as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::PathMapKeyNorito {
                            dest,
                            base,
                            key_blob,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Name base; publish; r11=&NoritoBytes blob; publish; SCALL BUILD_PATH_KEY_NORITO
                            if let Some(s) = string_map.get(&(func_idx, *base)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(base, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(s) = string_map.get(&(func_idx, *key_blob)) {
                                let kb = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, kb);
                            } else {
                                let r = src_reg(key_blob, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            // INPUT_PUBLISH_TLV always operates on r10, so preserve the published
                            // base pointer while mirroring the key blob through r10.
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_BUILD_PATH_KEY_NORITO as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonEncode { dest, json } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; SCALL JSON_ENCODE; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_ENCODE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonDecode { dest, blob } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&NoritoBytes or &Blob; publish; SCALL JSON_DECODE; move
                            if let Some(s) = string_map.get(&(func_idx, *blob)) {
                                let key = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(blob, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_DECODE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::TlvLen { dest, value } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&TLV; publish; SCALL TLV_LEN; move r10 (len) to dest
                            let r = src_reg(value, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r, 0)?);
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_TLV_LEN as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetInt { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_I64; move r10 (int) to dest
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Both args must be in INPUT for pointer-ABI validation.
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_I64 as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetNumeric { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_NUMERIC; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_NUMERIC as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetJson { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_JSON; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Both args must be in INPUT for pointer-ABI validation.
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_JSON as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetName { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_NAME; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Both args must be in INPUT for pointer-ABI validation.
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_NAME as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetAccountId { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_ACCOUNT_ID; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Both args must be in INPUT for pointer-ABI validation.
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_ACCOUNT_ID as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetAssetDefinitionId { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_ASSET_DEFINITION_ID; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetNftId { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_NFT_ID; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Both args must be in INPUT for pointer-ABI validation.
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_NFT_ID as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonGetBlobHex { dest, json, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Json; publish; r11=&Name key; SCALL JSON_GET_BLOB_HEX; move r10
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            // Both args must be in INPUT for pointer-ABI validation.
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *key)) {
                                let kb = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, kb);
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_GET_BLOB_HEX as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }

                        Instr::NameDecode { dest, blob } => {
                            // r10=&NoritoBytes; publish; SCALL NAME_DECODE; move
                            if let Some(s) = string_map.get(&(func_idx, *blob)) {
                                let key = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(blob, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NAME_DECODE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::SchemaEncode { dest, schema, json } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Name; publish; r11=&Json; publish; SCALL
                            if let Some(s) = string_map.get(&(func_idx, *schema)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(schema, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(scratch1, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *json)) {
                                let key = DataKey(DataKind::Json, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(json, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch1, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SCHEMA_ENCODE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::SchemaDecode { dest, schema, blob } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            // r10=&Name; publish; r11=&NoritoBytes or &Blob; publish; SCALL
                            if let Some(s) = string_map.get(&(func_idx, *schema)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(schema, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(scratch1, 10, 0)?);
                            if let Some(s) = string_map.get(&(func_idx, *blob)) {
                                let key = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(blob, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch1, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SCHEMA_DECODE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::SchemaInfo { dest, schema } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            if let Some(s) = string_map.get(&(func_idx, *schema)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(schema, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SCHEMA_INFO as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::VrfVerify {
                            dest,
                            input,
                            public_key,
                            proof,
                            variant,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            if let Some(s) = string_map.get(&(func_idx, *input)) {
                                let key = DataKey(DataKind::Blob, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(input, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(s) = string_map.get(&(func_idx, *public_key)) {
                                let key = DataKey(DataKind::Blob, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key);
                            } else {
                                let r = src_reg(public_key, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(s) = string_map.get(&(func_idx, *proof)) {
                                let key = DataKey(DataKind::Blob, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 12, key);
                            } else {
                                let r = src_reg(proof, scratchd, &mut code)?;
                                push_word(&mut code, encode_addi(12, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let rvar = src_reg(variant, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(13, rvar, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_VRF_VERIFY as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::VrfVerifyBatch { dest, batch } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            if let Some(s) = string_map.get(&(func_idx, *batch)) {
                                let key = DataKey(DataKind::Blob, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(batch, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_VRF_VERIFY_BATCH as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::AxtBegin { descriptor } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            if let Some(s) = string_map.get(&(func_idx, *descriptor)) {
                                let key = DataKey(DataKind::AxtDescriptor, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(descriptor, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_AXT_BEGIN as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::AxtTouch { dsid, manifest } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            if let Some(s) = string_map.get(&(func_idx, *dsid)) {
                                let key = DataKey(DataKind::DataSpaceId, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(dsid, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(m) = manifest {
                                if let Some(s) = string_map.get(&(func_idx, *m)) {
                                    let key = DataKey(DataKind::NoritoBytes, s.clone());
                                    emit_literal_stub(&mut code, &mut fixups, 11, key);
                                } else {
                                    let r = src_reg(m, scratch2, &mut code)?;
                                    push_word(&mut code, encode_addi(11, r, 0)?);
                                }
                                code.extend_from_slice(&pub_word.to_le_bytes());
                            } else {
                                push_word(&mut code, encode_addi(11, 0, 0)?);
                            }
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_AXT_TOUCH as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::VerifyDsProof { dsid, proof } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            if let Some(s) = string_map.get(&(func_idx, *dsid)) {
                                let key = DataKey(DataKind::DataSpaceId, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(dsid, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(p) = proof {
                                if let Some(s) = string_map.get(&(func_idx, *p)) {
                                    let key = DataKey(DataKind::ProofBlob, s.clone());
                                    emit_literal_stub(&mut code, &mut fixups, 11, key);
                                } else {
                                    let r = src_reg(p, scratch2, &mut code)?;
                                    push_word(&mut code, encode_addi(11, r, 0)?);
                                }
                                code.extend_from_slice(&pub_word.to_le_bytes());
                            } else {
                                push_word(&mut code, encode_addi(11, 0, 0)?);
                            }
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_VERIFY_DS_PROOF as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::UseAssetHandle {
                            handle,
                            intent,
                            proof,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            if let Some(s) = string_map.get(&(func_idx, *handle)) {
                                let key = DataKey(DataKind::AssetHandle, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(handle, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(s) = string_map.get(&(func_idx, *intent)) {
                                let key = DataKey(DataKind::NoritoBytes, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key);
                            } else {
                                let r = src_reg(intent, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(p) = proof {
                                if let Some(s) = string_map.get(&(func_idx, *p)) {
                                    let key = DataKey(DataKind::ProofBlob, s.clone());
                                    emit_literal_stub(&mut code, &mut fixups, 12, key);
                                } else {
                                    let r = src_reg(p, scratchd, &mut code)?;
                                    push_word(&mut code, encode_addi(12, r, 0)?);
                                }
                                code.extend_from_slice(&pub_word.to_le_bytes());
                            } else {
                                push_word(&mut code, encode_addi(12, 0, 0)?);
                            }
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_USE_ASSET_HANDLE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::AxtCommit => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_AXT_COMMIT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                    }
                }
                // end for instr in &bb.instrs
                let mut emit_return_value = |temp: &ir::Temp,
                                             rd: u8,
                                             scratch: u8,
                                             code: &mut Vec<u8>|
                 -> Result<(), String> {
                    if let Some(kind) = dataref_kind_map.get(&(func_idx, *temp)).copied()
                        && let Some(lit) = string_map.get(&(func_idx, *temp)).cloned()
                    {
                        let key = data_key_for_pointer(kind, &lit);
                        emit_literal_stub(code, &mut fixups, rd, key);
                    } else {
                        let rs = src_reg(temp, scratch, code)?;
                        push_word(code, encode_addi(rd, rs, 0)?);
                    }
                    Ok(())
                };
                match &bb.terminator {
                    Terminator::Return(ret) => {
                        if let Some(tmp) = ret {
                            let rd = super::regalloc::RET_REG as u8;
                            emit_return_value(tmp, rd, scratch1, &mut code)?;
                        }
                        if is_entry {
                            push_word(&mut code, encoding::wide::encode_halt());
                        } else {
                            // Epilogue: restore RA, deallocate frame, return
                            let sp = regalloc::SP_REG as u8;
                            let scratch_base = if sp != scratch1 { scratch1 } else { scratch2 };
                            for (idx, reg) in saved_regs.iter().copied().enumerate() {
                                let offset = (save_base + idx * 8) as i64;
                                emit_load64(&mut code, reg, sp, offset, Some(scratch_base))?;
                            }
                            // LD ra, [sp+0]
                            let ld = encode_load64_rv(1, sp, 0)?;
                            push_word(&mut code, ld);
                            // ADDI sp, sp, frame
                            emit_addi_inplace(&mut code, sp, local_frame as i64);
                            // JALR x0, x1, 0
                            let jalr = encoding::wide::encode_rr(
                                instruction::wide::control::JALR,
                                0,
                                1,
                                0,
                            );
                            push_word(&mut code, jalr);
                        }
                    }
                    Terminator::Return2(t0, t1) => {
                        // r10 <- first, r11 <- second, then return/halts
                        emit_return_value(t0, 10, scratch1, &mut code)?;
                        emit_return_value(t1, 11, scratch2, &mut code)?;
                        if is_entry {
                            push_word(&mut code, encoding::wide::encode_halt());
                        } else {
                            // Epilogue
                            let sp = regalloc::SP_REG as u8;
                            let scratch_base = if sp != scratch1 { scratch1 } else { scratch2 };
                            for (idx, reg) in saved_regs.iter().copied().enumerate() {
                                let offset = (save_base + idx * 8) as i64;
                                emit_load64(&mut code, reg, sp, offset, Some(scratch_base))?;
                            }
                            let ld = encode_load64_rv(1, sp, 0)?;
                            push_word(&mut code, ld);
                            emit_addi_inplace(&mut code, sp, local_frame as i64);
                            let jalr = encoding::wide::encode_rr(
                                instruction::wide::control::JALR,
                                0,
                                1,
                                0,
                            );
                            push_word(&mut code, jalr);
                        }
                    }
                    Terminator::ReturnN(vals) => {
                        if vals.len() > regalloc::MAX_RETURN_VALUES {
                            return Err(format!(
                                "too many return values in function: {} > {}",
                                vals.len(),
                                regalloc::MAX_RETURN_VALUES
                            ));
                        }
                        for (i, t) in vals.iter().enumerate() {
                            let rd = (regalloc::RET_REG + i) as u8;
                            emit_return_value(t, rd, scratch1, &mut code)?;
                        }
                        if is_entry {
                            push_word(&mut code, encoding::wide::encode_halt());
                        } else {
                            // Epilogue
                            let sp = regalloc::SP_REG as u8;
                            let scratch_base = if sp != scratch1 { scratch1 } else { scratch2 };
                            for (idx, reg) in saved_regs.iter().copied().enumerate() {
                                let offset = (save_base + idx * 8) as i64;
                                emit_load64(&mut code, reg, sp, offset, Some(scratch_base))?;
                            }
                            let ld = encode_load64_rv(1, sp, 0)?;
                            push_word(&mut code, ld);
                            emit_addi_inplace(&mut code, sp, local_frame as i64);
                            let jalr = encoding::wide::encode_rr(
                                instruction::wide::control::JALR,
                                0,
                                1,
                                0,
                            );
                            push_word(&mut code, jalr);
                        }
                    }
                    Terminator::Jump(target) => {
                        let at = reserve_control_transfer_stub(&mut code);
                        jump_fixups.push(JumpFixup {
                            at,
                            target_label: target.0,
                        });
                    }
                    Terminator::Branch {
                        cond,
                        then_bb,
                        else_bb,
                    } => {
                        let rs_cond = src_reg(cond, scratch1, &mut code)?;
                        // Branch skips the entire else-transfer stub and lands on the then-transfer stub.
                        let skip_word = encode_branch_rv(
                            0x1,
                            rs_cond,
                            0,
                            (CONTROL_TRANSFER_STUB_WORDS * 4 + 4) as i16,
                        )?;
                        push_word(&mut code, skip_word);
                        let jal_else_at = reserve_control_transfer_stub(&mut code);
                        let jal_then_at = reserve_control_transfer_stub(&mut code);
                        branch_fixups.push(BranchFixup {
                            jal_else_at,
                            else_label: else_bb.0,
                            jal_then_at,
                            then_label: then_bb.0,
                        });
                    }
                }
            }
            for fix in jump_fixups {
                let target_off = *block_offsets.get(&fix.target_label).ok_or_else(|| {
                    format!(
                        "missing block offset for label {} in {}",
                        fix.target_label, func.name
                    )
                })?;
                let target_pc = (func_base + target_off) as u64;
                patch_jump_transfer_stub(&mut code, fix.at, target_pc)?;
            }
            for fix in branch_fixups {
                let else_target = *block_offsets.get(&fix.else_label).ok_or_else(|| {
                    format!(
                        "missing else-block offset for label {} in {}",
                        fix.else_label, func.name
                    )
                })?;
                let then_target = *block_offsets.get(&fix.then_label).ok_or_else(|| {
                    format!(
                        "missing then-block offset for label {} in {}",
                        fix.then_label, func.name
                    )
                })?;
                let else_target_pc = (func_base + else_target) as u64;
                let then_target_pc = (func_base + then_target) as u64;

                patch_jump_transfer_stub(&mut code, fix.jal_else_at, else_target_pc)?;
                patch_jump_transfer_stub(&mut code, fix.jal_then_at, then_target_pc)?;
            }
            uses_zk_global |= uses_zk;
        }

        // Patch call sites now that function offsets are known.
        for (at, callee, _caller) in &call_fixups {
            let target = *func_start_offsets.get(callee).ok_or_else(|| {
                i18n::translate(self.lang, Message::SemanticError("unknown callee"))
            })? as u64;
            patch_call_transfer_stub(&mut code, *at, target)?;
        }

        uses_vector_global |= detect_vector_usage(&code);
        uses_zk_global |= detect_zk_usage(&code);

        let meta_decl = typed.contract_meta.as_ref();
        validate_feature_requests(meta_decl, uses_zk_global, uses_vector_global)?;

        // Build metadata and finalize program (with data appended).
        // Resolve mode bits: program usage OR forced by options OR contract meta
        let mut mode = 0u8;
        let meta_requests_zk = meta_decl.is_some_and(|m| {
            m.force_zk.unwrap_or(false) || m.features.contains(&ContractFeature::Zk)
        });
        if uses_zk_global || self.opts.force_zk || meta_requests_zk {
            mode |= metadata::mode::ZK;
        }
        let meta_requests_vector = meta_decl.is_some_and(|m| {
            m.force_vector.unwrap_or(false) || m.features.contains(&ContractFeature::Vector)
        });
        if uses_vector_global || self.opts.force_vector || meta_requests_vector {
            mode |= metadata::mode::VECTOR;
        }

        // Construct header using contract meta (if present) with compiler options as fallback
        let meta = ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode,
            vector_length: meta_decl
                .and_then(|m| m.vector_length)
                .unwrap_or(self.opts.vector_length),
            max_cycles: meta_decl
                .and_then(|m| m.max_cycles)
                .filter(|value| *value != 0)
                .unwrap_or(self.opts.max_cycles),
            abi_version,
        };
        // Build data section from collected keys and pointer literal table.
        use iroha_crypto::Hash as IrohaHash;
        use iroha_data_model::prelude::*;
        use iroha_primitives::json::Json;
        use norito::{decode_from_bytes, to_bytes};
        // Stable key order based on first occurrence in fixups. Also include datarefs seen even if unused
        // in emitted code to ensure TLVs are generated (useful for constructor-only samples/tests).
        let mut key_order: IndexSet<DataKey> = IndexSet::new();
        for (_, _, k) in &fixups {
            key_order.insert(k.clone());
        }
        // Extend with datarefs not already present
        for (k, v) in &datarefs {
            let dk = match k {
                DRK::Account => DataKey(DataKind::Account, v.clone()),
                DRK::AssetDef => DataKey(DataKind::AssetDef, v.clone()),
                DRK::Name => DataKey(DataKind::Name, v.clone()),
                DRK::Json => DataKey(DataKind::Json, v.clone()),
                DRK::NftId => DataKey(DataKind::NftId, v.clone()),
                DRK::AssetId => DataKey(DataKind::AssetId, v.clone()),
                DRK::Domain => DataKey(DataKind::Domain, v.clone()),
                DRK::Blob => DataKey(DataKind::Blob, v.clone()),
                DRK::NoritoBytes => DataKey(DataKind::NoritoBytes, v.clone()),
                DRK::DataSpaceId => DataKey(DataKind::DataSpaceId, v.clone()),
                DRK::AxtDescriptor => DataKey(DataKind::AxtDescriptor, v.clone()),
                DRK::AssetHandle => DataKey(DataKind::AssetHandle, v.clone()),
                DRK::ProofBlob => DataKey(DataKind::ProofBlob, v.clone()),
            };
            key_order.insert(dk);
        }
        let mut get_or_insert_data = |key: &DataKey| -> Result<u64, String> {
            if let Some(off) = data_offsets.get(key) {
                return Ok(*off);
            }
            let (type_id, mut payload) = match key {
                DataKey(DataKind::Account, s) => {
                    let id = AccountId::parse_encoded(s)
                        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                        .map_err(|e| {
                            let err = format!("invalid AccountId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?;
                    (
                        1u16,
                        to_bytes(&id).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid AccountId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::AssetDef, s) => {
                    let id = AssetDefinitionId::parse_address_literal(s).map_err(|e| {
                        let err = format!("invalid AssetDefinitionId literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        2u16,
                        to_bytes(&id).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid AssetDefinitionId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::NftId, s) => {
                    let id: iroha_data_model::nft::NftId = s.parse().map_err(|e| {
                        let err = format!("invalid NftId literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        5u16,
                        to_bytes(&id).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid NftId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::AssetId, s) => {
                    let id: iroha_data_model::asset::AssetId = s.parse().map_err(|e| {
                        let err = format!("invalid AssetId literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        7u16,
                        to_bytes(&id).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid AssetId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::Name, s) => {
                    let nm: Name = s.parse().map_err(|e| {
                        let err = format!("invalid Name literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        3u16,
                        to_bytes(&nm).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid Name literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::Json, s) => {
                    // JSON literals must be valid JSON text. (Use `norito_bytes` for opaque bytes.)
                    let value = norito::json::parse_value(s).map_err(|e| {
                        let err = format!("invalid JSON literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    let json = Json::from_norito_value_ref(&value).map_err(|e| {
                        let err = format!("invalid JSON literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        4u16,
                        to_bytes(&json).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid JSON literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::Domain, s) => {
                    let id: iroha_data_model::domain::DomainId = s.parse().map_err(|e| {
                        let err = format!("invalid DomainId literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        8u16,
                        to_bytes(&id).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid DomainId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::Blob, s) => (
                    6u16,
                    decode_hex_or_raw_bytes(s).map_err(|e| {
                        let err = format!("invalid Blob literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?,
                ),
                DataKey(DataKind::NoritoBytes, s) => {
                    let bytes = decode_hex_or_raw_bytes(s).map_err(|e| {
                        let err = format!("invalid NoritoBytes literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (9u16, bytes)
                }
                DataKey(DataKind::DataSpaceId, s) => {
                    if let Some(raw) = parse_u64_literal(s) {
                        let id = iroha_data_model::nexus::DataSpaceId::new(raw);
                        (
                            PointerType::DataSpaceId as u16,
                            to_bytes(&id).map_err(|e| e.to_string()).map_err(|e| {
                                let err = format!("invalid DataSpaceId literal `{s}`: {e}");
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?,
                        )
                    } else {
                        let bytes = decode_hex_or_raw_bytes(s).map_err(|e| {
                            let err = format!("invalid DataSpaceId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?;
                        let value: iroha_data_model::nexus::DataSpaceId = decode_from_bytes(&bytes)
                            .map_err(|e| {
                                let err = format!(
                                    "invalid DataSpaceId literal `{s}`: cannot decode ({e})"
                                );
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?;
                        (
                            PointerType::DataSpaceId as u16,
                            to_bytes(&value).map_err(|e| e.to_string()).map_err(|e| {
                                let err = format!("invalid DataSpaceId literal `{s}`: {e}");
                                i18n::translate(self.lang, Message::SemanticError(&err))
                            })?,
                        )
                    }
                }
                DataKey(DataKind::AxtDescriptor, s) => {
                    let bytes = decode_hex_or_raw_bytes(s).map_err(|e| {
                        let err = format!("invalid AxtDescriptor literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    let value: crate::axt::AxtDescriptor =
                        decode_from_bytes(&bytes).map_err(|e| {
                            let err =
                                format!("invalid AxtDescriptor literal `{s}`: cannot decode ({e})");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?;
                    (
                        PointerType::AxtDescriptor as u16,
                        to_bytes(&value).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid AxtDescriptor literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::AssetHandle, s) => {
                    let bytes = decode_hex_or_raw_bytes(s).map_err(|e| {
                        let err = format!("invalid AssetHandle literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    let value: crate::axt::AssetHandle =
                        decode_from_bytes(&bytes).map_err(|e| {
                            let err =
                                format!("invalid AssetHandle literal `{s}`: cannot decode ({e})");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?;
                    (
                        PointerType::AssetHandle as u16,
                        to_bytes(&value).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid AssetHandle literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::ProofBlob, s) => {
                    let bytes = decode_hex_or_raw_bytes(s).map_err(|e| {
                        let err = format!("invalid ProofBlob literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    let value: crate::axt::ProofBlob = decode_from_bytes(&bytes).map_err(|e| {
                        let err = format!("invalid ProofBlob literal `{s}`: cannot decode ({e})");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        PointerType::ProofBlob as u16,
                        to_bytes(&value).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid ProofBlob literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
            };
            // TLV envelope: type_id (be), version=1, len (be u32), payload, hash (32 bytes blake2b-32)
            let mut v = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
            v.extend_from_slice(&type_id.to_be_bytes());
            v.push(1u8);
            v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            v.append(&mut payload);
            let h = IrohaHash::new(&v[2 + 1 + 4..]);
            v.extend_from_slice(h.as_ref());
            let bytes = v;
            let off = data_bytes.len() as u64;
            data_bytes.extend_from_slice(&bytes);
            data_offsets.insert(key.clone(), off);
            Ok(off)
        };

        let entrypoint_descriptors = build_entrypoint_descriptors(
            &typed,
            &access_sets,
            &ir_prog.functions,
            &hint_reports,
            &func_start_offsets,
        )?;
        let access_set_hints = build_access_set_hints(&access_sets, include_hints);
        let kotoba_entries = build_kotoba_entries(&typed.kotoba_entries);
        let mut feature_bits = 0u64;
        if meta.mode & metadata::mode::ZK != 0 {
            feature_bits |= CONTRACT_FEATURE_BIT_ZK;
        }
        if meta.mode & metadata::mode::VECTOR != 0 {
            feature_bits |= CONTRACT_FEATURE_BIT_VECTOR;
        }
        let contract_interface = EmbeddedContractInterfaceV1 {
            compiler_fingerprint: COMPILER_FINGERPRINT.to_owned(),
            features_bitmap: feature_bits,
            access_set_hints: access_set_hints.clone(),
            kotoba: kotoba_entries.clone(),
            entrypoints: entrypoint_descriptors.clone(),
        };
        let compile_report = build_compile_report(
            &function_debug_seeds,
            code.len(),
            self.opts.debug_source_name.as_deref(),
            hint_diagnostics.clone(),
        );
        let debug_section = if self.opts.emit_debug {
            EmbeddedContractDebugInfoV1 {
                source_map: compile_report.source_map.clone(),
                budget_report: compile_report.budget_report.clone(),
            }
            .encode_section()
        } else {
            Vec::new()
        };

        // Compute literal table and patch LOADs. Contract artifacts are laid out as:
        //   [ header | CNTR | DBG1 | LTLB? | code ]
        let meta_bytes = meta.encode();
        let contract_section = contract_interface.encode_section();
        let header_len = meta_bytes.len() as u64;
        let need_literals = !key_order.is_empty();
        // Literal table base when present, immediately after the required CNTR/DBG1 sections.
        let lit_base = header_len + contract_section.len() as u64 + debug_section.len() as u64;
        // Literal table length and offsets
        let lit_count = key_order.len() as u64;
        let lit_size = lit_count * 8;
        let lit_header_size: u64 = if need_literals { 16 } else { 0 };
        let lit_entries_base = lit_base + lit_header_size;
        let lit_entries_base_rel = lit_entries_base
            .checked_sub(lit_base)
            .expect("literal entries base beyond literal section start");
        let data_base_rel = lit_entries_base_rel + lit_size;
        let mut lit_bytes: Vec<u8> = Vec::with_capacity(lit_size as usize);
        for k in key_order.iter() {
            let data_off = get_or_insert_data(k)?;
            let ptr = data_base_rel + data_off;
            lit_bytes.extend_from_slice(&ptr.to_le_bytes());
        }
        // Patch literal pointer stubs with absolute data addresses
        let literal_start = contract_section.len() as u64 + debug_section.len() as u64;
        for (at, rd, key) in &fixups {
            let data_off = *data_offsets
                .get(key)
                .expect("literal data offset present for pointer stub");
            let ptr = literal_start + data_base_rel + data_off;
            patch_pointer_literal_stub(&mut code, *at, *rd, ptr)?;
        }

        // Final layout assembly
        let mut out = meta_bytes;
        out.extend_from_slice(&contract_section);
        out.extend_from_slice(&debug_section);
        let mut post_pad: usize = 0;
        if need_literals {
            let total_prefix = contract_section.len()
                + debug_section.len()
                + lit_header_size as usize
                + lit_size as usize
                + data_bytes.len();
            let rem = total_prefix % 4;
            if rem != 0 {
                post_pad = 4 - rem;
            }
        }
        if need_literals {
            let data_len = data_bytes.len() as u32;
            out.extend_from_slice(&LITERAL_SECTION_MAGIC);
            out.extend_from_slice(&(lit_count as u32).to_le_bytes());
            out.extend_from_slice(&(post_pad as u32).to_le_bytes());
            out.extend_from_slice(&data_len.to_le_bytes());
            out.extend_from_slice(&lit_bytes);
            out.extend_from_slice(&data_bytes);
            if post_pad != 0 {
                out.resize(out.len() + post_pad, 0u8);
            }
        }
        let code_start = out.len();
        out.extend_from_slice(&code);
        // Optional debug: dump compiled image as hex for tests/debugging when requested.
        if cfg!(any(test, debug_assertions)) && std::env::var_os("IVM_COMPILER_DEBUG").is_some() {
            let mut pairs: Vec<_> = func_start_offsets.iter().collect();
            pairs.sort_by_key(|(_, off)| **off);
            for (name, off) in pairs {
                eprintln!(
                    "[kotodama-compile] func {} @ 0x{:x} (code+0x{:x})",
                    name,
                    code_start + *off,
                    off
                );
            }
            // Print first 64 bytes of header+lit, then first 64 bytes of code if available.
            use std::fmt::Write as _;
            let mut hex = String::new();
            // Header bytes (first 64)
            for b in out.iter().take(64) {
                let _ = write!(&mut hex, "{b:02x}");
            }
            let _ = write!(&mut hex, " | ");
            // Code bytes (first 64) start after the CNTR/literal prefix.
            for b in out.iter().skip(code_start).take(64) {
                let _ = write!(&mut hex, "{b:02x}");
            }
            eprintln!("[kotodama-compile] header+lit(first64) | code(first64): {hex}");
        }

        Ok(CompilationArtifacts {
            bytes: out,
            compile_report,
        })
    }

    /// Compile source and produce a manifest with code_hash and abi_hash.
    ///
    /// The returned `ContractManifest` includes
    /// - `code_hash`: hash of the compiled payload (literal table + code, excluding the metadata header)
    /// - `abi_hash`: hash of the allowed syscall surface for the program's `abi_version`
    pub fn compile_source_with_manifest(
        &self,
        src: &str,
    ) -> Result<
        (
            Vec<u8>,
            iroha_data_model::smart_contract::manifest::ContractManifest,
        ),
        String,
    > {
        let (bytes, manifest, _report) = self.compile_source_with_manifest_and_report(src)?;
        Ok((bytes, manifest))
    }

    /// Compile source and produce a manifest plus compiler report data.
    pub fn compile_source_with_manifest_and_report(
        &self,
        src: &str,
    ) -> Result<
        (
            Vec<u8>,
            iroha_data_model::smart_contract::manifest::ContractManifest,
            CompileReport,
        ),
        String,
    > {
        let program =
            parser::parse(src).map_err(|e| i18n::translate(self.lang, Message::ParserError(&e)))?;
        let artifacts = self.compile_program(&program)?;
        let bytes = artifacts.bytes.clone();
        let parsed = crate::metadata::ProgramMetadata::parse(&bytes)
            .map_err(|e| format!("manifest parse header: {e}"))?;
        let contract_interface = parsed.contract_interface.ok_or_else(|| {
            "manifest parse header: missing embedded contract interface".to_owned()
        })?;
        let code_hash = iroha_crypto::Hash::new(&bytes[parsed.header_len..]);
        let meta = parsed.metadata;
        // First release: emit manifests only for ABI v1
        let policy = match meta.abi_version {
            1 => crate::SyscallPolicy::AbiV1,
            v => return Err(format!("unsupported abi_version {v}; expected 1")),
        };
        let abi_hash_bytes = crate::syscalls::compute_abi_hash(policy);
        let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash_bytes)),
            compiler_fingerprint: Some(contract_interface.compiler_fingerprint),
            features_bitmap: Some(contract_interface.features_bitmap),
            access_set_hints: contract_interface.access_set_hints,
            entrypoints: Some(
                contract_interface
                    .entrypoints
                    .into_iter()
                    .map(|entrypoint| entrypoint.to_manifest_descriptor())
                    .collect(),
            ),
            kotoba: (!contract_interface.kotoba.is_empty()).then_some(contract_interface.kotoba),
            provenance: None,
        };
        Ok((bytes, manifest, artifacts.compile_report))
    }

    /// Compile source and produce a manifest plus access-hint diagnostics.
    pub fn compile_source_with_manifest_and_diagnostics(
        &self,
        src: &str,
    ) -> Result<
        (
            Vec<u8>,
            iroha_data_model::smart_contract::manifest::ContractManifest,
            AccessHintDiagnostics,
        ),
        String,
    > {
        let (bytes, manifest, report) = self.compile_source_with_manifest_and_report(src)?;
        Ok((bytes, manifest, report.access_hint_diagnostics))
    }
}

fn build_compile_report(
    function_debug_seeds: &[FunctionDebugSeed],
    code_len: usize,
    source_path: Option<&str>,
    access_hint_diagnostics: AccessHintDiagnostics,
) -> CompileReport {
    let mut entries = function_debug_seeds.to_vec();
    entries.sort_by_key(|seed| seed.pc_start);

    let mut source_map = Vec::with_capacity(entries.len());
    let mut budget_report = Vec::with_capacity(entries.len());
    for (idx, seed) in entries.iter().enumerate() {
        let pc_end = entries
            .get(idx + 1)
            .map(|next| next.pc_start)
            .unwrap_or(code_len as u64);
        let source = EmbeddedSourceLocation {
            source_path: source_path.map(ToOwned::to_owned),
            line: seed.location.line as u32,
            column: seed.location.column as u32,
        };
        let bytecode_bytes = pc_end.saturating_sub(seed.pc_start) as u32;
        let bytecode_words = bytecode_bytes / 4;
        source_map.push(EmbeddedSourceMapEntryV1 {
            function_name: seed.name.clone(),
            pc_start: seed.pc_start,
            pc_end,
            source: source.clone(),
        });
        budget_report.push(EmbeddedFunctionBudgetReportV1 {
            function_name: seed.name.clone(),
            pc_start: seed.pc_start,
            pc_end,
            bytecode_bytes,
            bytecode_words,
            frame_bytes: seed.frame_bytes,
            jump_span_words: bytecode_words,
            jump_range_risk: bytecode_words > i16::MAX as u32,
            source: Some(source),
        });
    }

    CompileReport {
        source_map,
        budget_report,
        access_hint_diagnostics,
    }
}

fn render_state_hint(hint: Option<&StatePathHint>) -> Option<String> {
    match hint? {
        StatePathHint::Literal(name) => Some(format!("state:{name}")),
        StatePathHint::Map { base } => Some(format!("state:{base}")),
    }
}

fn insert_state_hint(keys: &mut IndexSet<String>, key: String) {
    keys.insert(key.clone());
    if let Some(base) = map_base_from_state_key(&key) {
        keys.insert(base);
    }
}

fn map_base_from_state_key(key: &str) -> Option<String> {
    let rest = key.strip_prefix("state:")?;
    let (base, _) = rest.split_once('/')?;
    if base.is_empty() {
        return None;
    }
    Some(format!("state:{base}"))
}

fn state_path_for_norito_key(base: &str, raw: &str) -> Option<String> {
    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    let digest: [u8; 32] = iroha_crypto::Hash::new(&bytes).into();
    let mut out = String::with_capacity(base.len() + 1 + 64);
    out.push_str(base);
    out.push('/');
    use core::fmt::Write as _;
    for b in &digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    Some(out)
}

fn build_access_set_hints(
    access_sets: &[AccessSets],
    include_hints: bool,
) -> Option<AccessSetHints> {
    if !include_hints {
        return None;
    }
    let mut reads: BTreeSet<String> = BTreeSet::new();
    let mut writes: BTreeSet<String> = BTreeSet::new();
    for set in access_sets {
        reads.extend(set.reads.iter().cloned());
        writes.extend(set.writes.iter().cloned());
    }
    if reads.is_empty() && writes.is_empty() {
        return None;
    }
    for key in writes.iter().cloned() {
        reads.insert(key);
    }
    Some(AccessSetHints {
        read_keys: reads.into_iter().collect(),
        write_keys: writes.into_iter().collect(),
    })
}

fn build_kotoba_entries(
    entries: &[super::ast::KotobaEntry],
) -> Vec<iroha_data_model::smart_contract::manifest::KotobaTranslationEntry> {
    entries
        .iter()
        .map(
            |entry| iroha_data_model::smart_contract::manifest::KotobaTranslationEntry {
                msg_id: entry.msg_id.clone(),
                translations: entry
                    .translations
                    .iter()
                    .map(|translation| {
                        iroha_data_model::smart_contract::manifest::KotobaTranslation {
                            lang: translation.lang.clone(),
                            text: translation.text.clone(),
                        }
                    })
                    .collect(),
            },
        )
        .collect()
}

fn entrypoint_ir_symbol_name(func: &semantic::TypedFunction) -> String {
    match entrypoint_kind_from_modifiers(&func.modifiers) {
        Some(EntryPointKind::Public | EntryPointKind::View) => {
            format!("__entrypoint_impl__{}", func.name)
        }
        _ => func.name.clone(),
    }
}

fn apply_explicit_access_hints(
    typed: &TypedProgram,
    fn_index_by_name: &HashMap<&str, usize>,
    access_sets: &mut [AccessSets],
    explicit_by_fn: &mut [bool],
) -> bool {
    let mut saw_hint = false;
    for item in &typed.items {
        let TypedItem::Function(func) = item;
        if func.modifiers.access_reads.is_empty() && func.modifiers.access_writes.is_empty() {
            continue;
        }
        saw_hint = true;
        let symbol_name = entrypoint_ir_symbol_name(func);
        if let Some(&idx) = fn_index_by_name.get(symbol_name.as_str()) {
            if let Some(slot) = explicit_by_fn.get_mut(idx) {
                *slot = true;
            }
            for read in &func.modifiers.access_reads {
                access_sets[idx].reads.insert(read.clone());
            }
            for write in &func.modifiers.access_writes {
                access_sets[idx].writes.insert(write.clone());
            }
        }
    }
    saw_hint
}

fn derive_isi_access_hints(
    ir_prog: &ir::Program,
    string_map: &HashMap<(usize, ir::Temp), String>,
    dataref_kind_map: &HashMap<(usize, ir::Temp), ir::DataRefKind>,
    access_sets: &mut [AccessSets],
    hint_diagnostics: &mut AccessHintDiagnostics,
) {
    for (func_idx, func) in ir_prog.functions.iter().enumerate() {
        for bb in &func.blocks {
            for instr in &bb.instrs {
                if !instr_queues_isi(instr) {
                    continue;
                }
                record_isi_access(
                    instr,
                    func_idx,
                    string_map,
                    dataref_kind_map,
                    &mut access_sets[func_idx],
                    hint_diagnostics,
                );
            }
        }
    }
}

fn record_isi_access(
    instr: &ir::Instr,
    func_idx: usize,
    string_map: &HashMap<(usize, ir::Temp), String>,
    dataref_kind_map: &HashMap<(usize, ir::Temp), ir::DataRefKind>,
    access_set: &mut AccessSets,
    hint_diagnostics: &mut AccessHintDiagnostics,
) {
    fn apply_fallback(access_set: &mut AccessSets, hint_diagnostics: &mut AccessHintDiagnostics) {
        hint_diagnostics.isi_wildcards = hint_diagnostics.isi_wildcards.saturating_add(1);
        access_set.reads.insert(GLOBAL_WILDCARD_KEY.to_string());
        access_set.writes.insert(GLOBAL_WILDCARD_KEY.to_string());
    }
    match instr {
        ir::Instr::TransferBatchBegin | ir::Instr::TransferBatchEnd => {}
        ir::Instr::TransferAsset {
            from, to, asset, ..
        } => {
            let (Some(from), Some(to), Some(asset_def)) = (
                parse_account_temp(string_map, func_idx, *from),
                parse_account_temp(string_map, func_idx, *to),
                parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset),
            ) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            let src = AssetId::of(asset_def.clone(), from);
            let dst = AssetId::of(asset_def, to);
            add_asset_rw(access_set, &src);
            add_asset_rw(access_set, &dst);
        }
        ir::Instr::MintAsset { account, asset, .. }
        | ir::Instr::BurnAsset { account, asset, .. } => {
            let (Some(account), Some(asset_def)) = (
                parse_account_temp(string_map, func_idx, *account),
                parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset),
            ) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            let asset_id = AssetId::of(asset_def.clone(), account);
            add_asset_rw(access_set, &asset_id);
            add_asset_def_rw(access_set, &asset_def);
        }
        ir::Instr::RegisterDomain { domain } | ir::Instr::UnregisterDomain { domain } => {
            let Some(id) = parse_temp(string_map, func_idx, *domain) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_domain_rw(access_set, &id);
        }
        ir::Instr::RegisterAccount { account } | ir::Instr::UnregisterAccount { account } => {
            let Some(id) = parse_account_temp(string_map, func_idx, *account) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_account_rw(access_set, &id);
        }
        ir::Instr::UnregisterAsset { asset } => {
            let Some(id) = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_domain_r(access_set, id.domain());
            add_asset_def_rw(access_set, &id);
        }
        ir::Instr::SetAccountDetail { account, key, .. } => {
            let (Some(id), Some(key)) = (
                parse_account_temp(string_map, func_idx, *account),
                parse_temp(string_map, func_idx, *key),
            ) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_account_detail_rw(access_set, &id, &key);
        }
        ir::Instr::CreateNft { nft, .. } | ir::Instr::BurnNft { nft } => {
            let Some(id) = parse_temp(string_map, func_idx, *nft) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_nft_rw(access_set, &id);
        }
        ir::Instr::TransferNft { from, nft, to } => {
            let (Some(from), Some(to), Some(id)) = (
                parse_account_temp(string_map, func_idx, *from),
                parse_account_temp(string_map, func_idx, *to),
                parse_temp(string_map, func_idx, *nft),
            ) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_account_r(access_set, &from);
            add_account_r(access_set, &to);
            add_nft_rw(access_set, &id);
        }
        ir::Instr::RemoveTrigger { name } | ir::Instr::SetTriggerEnabled { name, .. } => {
            let Some(id) = parse_temp(string_map, func_idx, *name) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_trigger_rw(access_set, &id);
        }
        ir::Instr::CreateRole { name, .. } | ir::Instr::DeleteRole { name } => {
            let Some(id) = parse_temp(string_map, func_idx, *name) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_role_rw(access_set, &id);
        }
        ir::Instr::GrantRole { account, name } | ir::Instr::RevokeRole { account, name } => {
            let (Some(account), Some(role)) = (
                parse_account_temp(string_map, func_idx, *account),
                parse_temp(string_map, func_idx, *name),
            ) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_account_rw(access_set, &account);
            add_role_r(access_set, &role);
            add_role_binding_w(access_set, &account, &role);
        }
        ir::Instr::GrantPermission { account, token }
        | ir::Instr::RevokePermission { account, token } => {
            let Some(account) = parse_account_temp(string_map, func_idx, *account) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            let Some(perm) =
                permission_name_from_token(string_map, dataref_kind_map, func_idx, *token)
            else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_account_rw(access_set, &account);
            add_permission_account_w(access_set, &account, &perm);
        }
        // Asset-definition construction is opaque; fall back to wildcard.
        ir::Instr::RegisterAsset { .. } | ir::Instr::CreateNewAsset { .. } => {
            apply_fallback(access_set, hint_diagnostics)
        }
        ir::Instr::CreateTrigger { json } => {
            let Some(raw) = string_map.get(&(func_idx, *json)) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            let Some(id) = trigger_id_from_json(raw) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_trigger_rw(access_set, &id);
        }
        ir::Instr::VendorExecuteInstruction { payload } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            let Some(isi) = decode_instruction_box_literal(raw) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            if record_instruction_box_access(&isi, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics);
            }
        }
        ir::Instr::VendorExecuteQuery { payload, .. } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            let Some(request) = decode_query_request_literal(raw) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            if record_query_request_access(&request, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics);
            }
        }
        ir::Instr::TransferDomain { domain, to } => {
            let (Some(domain), Some(to)) = (
                parse_temp::<DomainId>(string_map, func_idx, *domain),
                parse_account_temp(string_map, func_idx, *to),
            ) else {
                return apply_fallback(access_set, hint_diagnostics);
            };
            add_domain_rw(access_set, &domain);
            add_account_r(access_set, &to);
        }
        // SetNftData lacks a metadata key in IR; fall back to wildcard.
        ir::Instr::SetNftData { .. } => apply_fallback(access_set, hint_diagnostics),
        // AXT/ZK helpers carry opaque payloads; fall back to wildcard.
        ir::Instr::RegisterPeer { .. }
        | ir::Instr::UnregisterPeer { .. }
        | ir::Instr::SubscriptionBill
        | ir::Instr::SubscriptionRecordUsage
        | ir::Instr::BuildSubmitBallotInline { .. }
        | ir::Instr::BuildUnshieldInline { .. }
        | ir::Instr::AxtBegin { .. }
        | ir::Instr::AxtTouch { .. }
        | ir::Instr::VerifyDsProof { .. }
        | ir::Instr::UseAssetHandle { .. }
        | ir::Instr::AxtCommit => apply_fallback(access_set, hint_diagnostics),
        _ => apply_fallback(access_set, hint_diagnostics),
    }
}

fn decode_instruction_box_literal(raw: &str) -> Option<InstructionBox> {
    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    let payload = match crate::pointer_abi::validate_tlv_bytes(&bytes) {
        Ok(tlv) => {
            if tlv.type_id != PointerType::NoritoBytes {
                return None;
            }
            tlv.payload.to_vec()
        }
        Err(_) => bytes,
    };
    norito::decode_from_bytes(&payload).ok()
}

fn decode_query_request_literal(raw: &str) -> Option<QueryRequest> {
    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    let payload = match crate::pointer_abi::validate_tlv_bytes(&bytes) {
        Ok(tlv) => {
            if tlv.type_id != PointerType::NoritoBytes {
                return None;
            }
            tlv.payload.to_vec()
        }
        Err(_) => bytes,
    };
    norito::decode_from_bytes(&payload).ok()
}

fn record_instruction_box_access(
    instr: &InstructionBox,
    access_set: &mut AccessSets,
) -> Option<()> {
    let any = instr.as_any();

    if any.downcast_ref::<Log>().is_some() {
        return Some(());
    }

    if let Some(tb) = any.downcast_ref::<TransferBox>() {
        match tb {
            TransferBox::Asset(t) => {
                let src = t.source.clone();
                let dst = AssetId::of(t.source.definition.clone(), t.destination.clone());
                add_asset_rw(access_set, &src);
                add_asset_rw(access_set, &dst);
            }
            TransferBox::Domain(t) => {
                add_domain_rw(access_set, &t.object);
                add_account_r(access_set, &t.source);
                add_account_r(access_set, &t.destination);
            }
            TransferBox::AssetDefinition(t) => {
                add_asset_def_rw(access_set, &t.object);
                add_account_r(access_set, &t.source);
                add_account_r(access_set, &t.destination);
            }
            TransferBox::Nft(t) => {
                add_nft_rw(access_set, &t.object);
                add_account_r(access_set, &t.source);
                add_account_r(access_set, &t.destination);
            }
        }
        return Some(());
    }

    if let Some(mb) = any.downcast_ref::<MintBox>() {
        match mb {
            MintBox::Asset(m) => {
                add_asset_rw(access_set, &m.destination);
                add_asset_def_rw(access_set, m.destination.definition());
            }
            MintBox::TriggerRepetitions(m) => {
                add_trigger_rw(access_set, &m.destination);
            }
        }
        return Some(());
    }

    if let Some(bb) = any.downcast_ref::<BurnBox>() {
        match bb {
            BurnBox::Asset(b) => {
                add_asset_rw(access_set, &b.destination);
                add_asset_def_rw(access_set, b.destination.definition());
            }
            BurnBox::TriggerRepetitions(b) => {
                add_trigger_rw(access_set, &b.destination);
            }
        }
        return Some(());
    }

    if let Some(sb) = any.downcast_ref::<SetKeyValueBox>() {
        match sb {
            SetKeyValueBox::Account(s) => {
                add_account_detail_rw(access_set, &s.object, &s.key);
            }
            SetKeyValueBox::Domain(s) => {
                add_domain_detail_rw(access_set, &s.object, &s.key);
            }
            SetKeyValueBox::AssetDefinition(s) => {
                add_asset_def_detail_rw(access_set, &s.object, &s.key);
            }
            SetKeyValueBox::Nft(s) => {
                add_nft_detail_rw(access_set, &s.object, &s.key);
            }
            SetKeyValueBox::Trigger(s) => {
                access_set.reads.insert(key_trigger(&s.object));
                access_set
                    .writes
                    .insert(key_trigger_detail(&s.object, &s.key));
            }
        }
        return Some(());
    }

    if let Some(rb) = any.downcast_ref::<RemoveKeyValueBox>() {
        match rb {
            RemoveKeyValueBox::Account(r) => {
                add_account_detail_rw(access_set, &r.object, &r.key);
            }
            RemoveKeyValueBox::Domain(r) => {
                add_domain_detail_rw(access_set, &r.object, &r.key);
            }
            RemoveKeyValueBox::AssetDefinition(r) => {
                add_asset_def_detail_rw(access_set, &r.object, &r.key);
            }
            RemoveKeyValueBox::Nft(r) => {
                add_nft_detail_rw(access_set, &r.object, &r.key);
            }
            RemoveKeyValueBox::Trigger(r) => {
                access_set.reads.insert(key_trigger(&r.object));
                access_set
                    .writes
                    .insert(key_trigger_detail(&r.object, &r.key));
            }
        }
        return Some(());
    }

    if let Some(rb) = any.downcast_ref::<RegisterBox>() {
        match rb {
            RegisterBox::Domain(r) => add_domain_rw(access_set, r.object.id()),
            RegisterBox::Account(r) => {
                if let Some(domain) = r.object.domain() {
                    add_domain_r(access_set, domain);
                }
                add_account_rw(access_set, r.object.id());
            }
            RegisterBox::AssetDefinition(r) => {
                add_domain_r(access_set, r.object.id().domain());
                add_asset_def_rw(access_set, r.object.id());
            }
            RegisterBox::Nft(r) => add_nft_rw(access_set, r.object.id()),
            RegisterBox::Peer(_) => return None,
            RegisterBox::Trigger(r) => add_trigger_rw(access_set, r.object.id()),
            RegisterBox::Role(r) => add_role_rw(access_set, r.object.id()),
        }
        return Some(());
    }

    if let Some(ub) = any.downcast_ref::<UnregisterBox>() {
        match ub {
            UnregisterBox::Domain(u) => add_domain_rw(access_set, &u.object),
            UnregisterBox::Account(u) => add_account_rw(access_set, &u.object),
            UnregisterBox::AssetDefinition(u) => add_asset_def_rw(access_set, &u.object),
            UnregisterBox::Nft(u) => add_nft_rw(access_set, &u.object),
            UnregisterBox::Peer(_) => return None,
            UnregisterBox::Trigger(u) => add_trigger_rw(access_set, &u.object),
            UnregisterBox::Role(u) => add_role_rw(access_set, &u.object),
        }
        return Some(());
    }

    if let Some(gb) = any.downcast_ref::<GrantBox>() {
        match gb {
            GrantBox::Permission(g) => {
                add_account_rw(access_set, &g.destination);
                add_permission_account_w(access_set, &g.destination, g.object.name());
            }
            GrantBox::Role(g) => {
                add_account_rw(access_set, &g.destination);
                add_role_r(access_set, &g.object);
                add_role_binding_w(access_set, &g.destination, &g.object);
            }
            GrantBox::RolePermission(g) => {
                add_role_rw(access_set, &g.destination);
                add_permission_role_w(access_set, &g.destination, g.object.name());
            }
        }
        return Some(());
    }

    if let Some(rb) = any.downcast_ref::<RevokeBox>() {
        match rb {
            RevokeBox::Permission(r) => {
                add_account_rw(access_set, &r.destination);
                add_permission_account_w(access_set, &r.destination, r.object.name());
            }
            RevokeBox::Role(r) => {
                add_account_rw(access_set, &r.destination);
                add_role_r(access_set, &r.object);
                add_role_binding_w(access_set, &r.destination, &r.object);
            }
            RevokeBox::RolePermission(r) => {
                add_role_rw(access_set, &r.destination);
                add_permission_role_w(access_set, &r.destination, r.object.name());
            }
        }
        return Some(());
    }

    if let Some(exe) = any.downcast_ref::<ExecuteTrigger>() {
        access_set.reads.insert(key_trigger(&exe.trigger));
        access_set
            .writes
            .insert(key_trigger_repetitions(&exe.trigger));
        return Some(());
    }

    None
}

fn record_query_request_access(request: &QueryRequest, access_set: &mut AccessSets) -> Option<()> {
    match request {
        QueryRequest::Singular(query) => record_singular_query_access(query, access_set),
        QueryRequest::Start(_) | QueryRequest::Continue(_) => None,
    }
}

fn record_singular_query_access(
    query: &SingularQueryBox,
    access_set: &mut AccessSets,
) -> Option<()> {
    match query {
        SingularQueryBox::FindAssetById(q) => {
            add_asset_r(access_set, q.asset_id());
            Some(())
        }
        _ => None,
    }
}

trait ParseTempLiteral: Sized {
    fn parse_temp_literal(raw: &str) -> Option<Self>;
}

impl<T: std::str::FromStr> ParseTempLiteral for T {
    fn parse_temp_literal(raw: &str) -> Option<Self> {
        raw.parse().ok()
    }
}

fn parse_temp<T: ParseTempLiteral>(
    string_map: &HashMap<(usize, ir::Temp), String>,
    func_idx: usize,
    temp: ir::Temp,
) -> Option<T> {
    T::parse_temp_literal(string_map.get(&(func_idx, temp))?)
}

fn parse_account_temp(
    string_map: &HashMap<(usize, ir::Temp), String>,
    func_idx: usize,
    temp: ir::Temp,
) -> Option<AccountId> {
    AccountId::parse_encoded(string_map.get(&(func_idx, temp))?)
        .ok()
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
}

fn permission_name_from_token(
    string_map: &HashMap<(usize, ir::Temp), String>,
    dataref_kind_map: &HashMap<(usize, ir::Temp), ir::DataRefKind>,
    func_idx: usize,
    temp: ir::Temp,
) -> Option<String> {
    let raw = string_map.get(&(func_idx, temp))?;
    match dataref_kind_map.get(&(func_idx, temp))? {
        ir::DataRefKind::Name => Some(permission_name_from_literal(raw)),
        ir::DataRefKind::Json => permission_name_from_json(raw),
        _ => None,
    }
}

fn permission_name_from_literal(raw: &str) -> String {
    raw.split_once(':')
        .map(|(name, _)| name)
        .unwrap_or(raw)
        .to_string()
}

fn permission_name_from_json(raw: &str) -> Option<String> {
    let value: norito::json::Value = norito::json::from_slice(raw.as_bytes()).ok()?;
    if let Some(name) = value.as_str() {
        return Some(permission_name_from_literal(name));
    }
    let map = value.as_object()?;
    let kind = map.get("type").and_then(norito::json::Value::as_str)?;
    Some(permission_name_from_literal(kind))
}

fn trigger_id_from_json(raw: &str) -> Option<TriggerId> {
    let value: json::Value = json::from_slice(raw.as_bytes()).ok()?;
    match value {
        json::Value::String(encoded) => {
            let bytes = STANDARD.decode(encoded.as_bytes()).ok()?;
            let trigger: Trigger = norito::decode_from_bytes(&bytes).ok()?;
            Some(trigger.id().clone())
        }
        json::Value::Object(map) => {
            let id = map.get("id")?.as_str()?;
            id.parse().ok()
        }
        _ => None,
    }
}

fn key_account(id: &AccountId) -> String {
    format!("account:{id}")
}

fn key_domain(id: &DomainId) -> String {
    format!("domain:{id}")
}

fn key_asset_def(id: &AssetDefinitionId) -> String {
    format!("asset_def:{id}")
}

fn key_asset(id: &AssetId) -> String {
    format!("asset:{id}")
}

fn key_nft(id: &NftId) -> String {
    format!("nft:{id}")
}

fn key_role(id: &RoleId) -> String {
    format!("role:{id}")
}

fn key_role_binding(account: &AccountId, role: &RoleId) -> String {
    format!("role.binding:{account}:{role}")
}

fn key_perm_account(account: &AccountId, perm: &str) -> String {
    format!("perm.account:{account}:{perm}")
}

fn key_perm_role(role: &RoleId, perm: &str) -> String {
    format!("perm.role:{role}:{perm}")
}

fn key_trigger(id: &TriggerId) -> String {
    format!("trigger:{id}")
}

fn key_trigger_repetitions(id: &TriggerId) -> String {
    format!("trigger.repetitions:{id}")
}

fn key_trigger_detail(id: &TriggerId, key: &Name) -> String {
    format!("trigger.detail:{id}:{key}")
}

fn key_account_detail(id: &AccountId, key: &Name) -> String {
    format!("account.detail:{id}:{key}")
}

fn key_domain_detail(id: &DomainId, key: &Name) -> String {
    format!("domain.detail:{id}:{key}")
}

fn key_asset_def_detail(id: &AssetDefinitionId, key: &Name) -> String {
    format!("asset_def.detail:{id}:{key}")
}

fn key_nft_detail(id: &NftId, key: &Name) -> String {
    format!("nft.detail:{id}:{key}")
}

fn add_account_r(set: &mut AccessSets, id: &AccountId) {
    set.reads.insert(key_account(id));
}

fn add_domain_r(set: &mut AccessSets, id: &DomainId) {
    set.reads.insert(key_domain(id));
}

fn add_account_rw(set: &mut AccessSets, id: &AccountId) {
    let key = key_account(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_account_detail_rw(set: &mut AccessSets, id: &AccountId, key: &Name) {
    add_account_r(set, id);
    let detail = key_account_detail(id, key);
    set.reads.insert(detail.clone());
    set.writes.insert(detail);
}

fn add_domain_rw(set: &mut AccessSets, id: &DomainId) {
    let key = key_domain(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_domain_detail_rw(set: &mut AccessSets, id: &DomainId, key: &Name) {
    add_domain_r(set, id);
    let detail = key_domain_detail(id, key);
    set.reads.insert(detail.clone());
    set.writes.insert(detail);
}

fn add_asset_def_rw(set: &mut AccessSets, id: &AssetDefinitionId) {
    let key = key_asset_def(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_asset_def_r(set: &mut AccessSets, id: &AssetDefinitionId) {
    set.reads.insert(key_asset_def(id));
}

fn add_asset_r(set: &mut AccessSets, id: &AssetId) {
    set.reads.insert(key_asset(id));
    add_account_r(set, id.account());
    add_domain_r(set, id.definition().domain());
    add_asset_def_r(set, id.definition());
}

fn add_asset_def_detail_rw(set: &mut AccessSets, id: &AssetDefinitionId, key: &Name) {
    add_asset_def_r(set, id);
    let detail = key_asset_def_detail(id, key);
    set.reads.insert(detail.clone());
    set.writes.insert(detail);
}

fn add_asset_rw(set: &mut AccessSets, id: &AssetId) {
    let key = key_asset(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
    add_account_r(set, id.account());
    add_domain_r(set, id.definition().domain());
    add_asset_def_r(set, id.definition());
}

fn add_nft_rw(set: &mut AccessSets, id: &NftId) {
    let key = key_nft(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_nft_detail_rw(set: &mut AccessSets, id: &NftId, key: &Name) {
    add_nft_rw(set, id);
    let detail = key_nft_detail(id, key);
    set.reads.insert(detail.clone());
    set.writes.insert(detail);
}

fn add_role_rw(set: &mut AccessSets, id: &RoleId) {
    let key = key_role(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_role_r(set: &mut AccessSets, id: &RoleId) {
    set.reads.insert(key_role(id));
}

fn add_role_binding_w(set: &mut AccessSets, account: &AccountId, role: &RoleId) {
    set.writes.insert(key_role_binding(account, role));
}

fn add_permission_account_w(set: &mut AccessSets, account: &AccountId, perm: &str) {
    set.writes.insert(key_perm_account(account, perm));
}

fn add_permission_role_w(set: &mut AccessSets, role: &RoleId, perm: &str) {
    set.writes.insert(key_perm_role(role, perm));
}

fn add_trigger_rw(set: &mut AccessSets, id: &TriggerId) {
    let key = key_trigger(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
    set.writes.insert(key_trigger_repetitions(id));
}

fn instr_queues_isi(instr: &ir::Instr) -> bool {
    matches!(
        instr,
        ir::Instr::RegisterAsset { .. }
            | ir::Instr::CreateNewAsset { .. }
            | ir::Instr::TransferAsset { .. }
            | ir::Instr::TransferBatchBegin
            | ir::Instr::TransferBatchEnd
            | ir::Instr::MintAsset { .. }
            | ir::Instr::BurnAsset { .. }
            | ir::Instr::SetAccountDetail { .. }
            | ir::Instr::CreateNft { .. }
            | ir::Instr::SetNftData { .. }
            | ir::Instr::BurnNft { .. }
            | ir::Instr::TransferNft { .. }
            | ir::Instr::RegisterDomain { .. }
            | ir::Instr::RegisterAccount { .. }
            | ir::Instr::UnregisterDomain { .. }
            | ir::Instr::UnregisterAsset { .. }
            | ir::Instr::UnregisterAccount { .. }
            | ir::Instr::RegisterPeer { .. }
            | ir::Instr::UnregisterPeer { .. }
            | ir::Instr::CreateTrigger { .. }
            | ir::Instr::RemoveTrigger { .. }
            | ir::Instr::SetTriggerEnabled { .. }
            | ir::Instr::GrantPermission { .. }
            | ir::Instr::RevokePermission { .. }
            | ir::Instr::CreateRole { .. }
            | ir::Instr::DeleteRole { .. }
            | ir::Instr::GrantRole { .. }
            | ir::Instr::RevokeRole { .. }
            | ir::Instr::TransferDomain { .. }
            | ir::Instr::VendorExecuteInstruction { .. }
            | ir::Instr::VendorExecuteQuery { .. }
            | ir::Instr::SubscriptionBill
            | ir::Instr::SubscriptionRecordUsage
            | ir::Instr::BuildSubmitBallotInline { .. }
            | ir::Instr::BuildUnshieldInline { .. }
            | ir::Instr::AxtBegin { .. }
            | ir::Instr::AxtTouch { .. }
            | ir::Instr::VerifyDsProof { .. }
            | ir::Instr::UseAssetHandle { .. }
            | ir::Instr::AxtCommit
    )
}

fn detect_vector_usage(code: &[u8]) -> bool {
    const VECTOR_OPS: [u8; 14] = [
        instruction::wide::crypto::VADD32,
        instruction::wide::crypto::VADD64,
        instruction::wide::crypto::VAND,
        instruction::wide::crypto::VXOR,
        instruction::wide::crypto::VOR,
        instruction::wide::crypto::VROT32,
        instruction::wide::crypto::SHA256BLOCK,
        instruction::wide::crypto::AESENC,
        instruction::wide::crypto::AESDEC,
        instruction::wide::crypto::SETVL,
        instruction::wide::crypto::PARBEGIN,
        instruction::wide::crypto::PAREND,
        instruction::wide::memory::LOAD128,
        instruction::wide::memory::STORE128,
    ];
    code.chunks_exact(4).any(|chunk| {
        let word = u32::from_le_bytes(chunk.try_into().expect("word chunk"));
        let opcode = instruction::wide::opcode(word);
        VECTOR_OPS.contains(&opcode)
    })
}

fn detect_zk_usage(code: &[u8]) -> bool {
    const ZK_OPS: [u8; 7] = [
        instruction::wide::zk::ASSERT,
        instruction::wide::zk::ASSERT_EQ,
        instruction::wide::zk::FADD,
        instruction::wide::zk::FSUB,
        instruction::wide::zk::FMUL,
        instruction::wide::zk::FINV,
        instruction::wide::zk::ASSERT_RANGE,
    ];
    code.chunks_exact(4).any(|chunk| {
        let word = u32::from_le_bytes(chunk.try_into().expect("word chunk"));
        let opcode = instruction::wide::opcode(word);
        ZK_OPS.contains(&opcode)
    })
}

fn build_entrypoint_descriptors(
    typed: &TypedProgram,
    access_sets: &[AccessSets],
    ir_functions: &[ir::Function],
    hint_reports: &[HintReport],
    func_start_offsets: &HashMap<String, usize>,
) -> Result<Vec<EmbeddedEntrypointDescriptor>, String> {
    let mut hints_by_name: HashMap<&str, (&IndexSet<String>, &IndexSet<String>)> = HashMap::new();
    let mut hintable_by_name: HashMap<&str, bool> = HashMap::new();
    let mut hint_report_by_name: HashMap<&str, &HintReport> = HashMap::new();
    for ((func, sets), report) in ir_functions
        .iter()
        .zip(access_sets.iter())
        .zip(hint_reports.iter())
    {
        hints_by_name.insert(&func.name, (&sets.reads, &sets.writes));
        hintable_by_name.insert(&func.name, report.emitted);
        hint_report_by_name.insert(&func.name, report);
    }

    let mut triggers_by_name: HashMap<String, Vec<TriggerDescriptor>> = HashMap::new();
    for trigger in &typed.triggers {
        let descriptor = TriggerDescriptor {
            id: trigger.id.clone(),
            repeats: trigger.repeats,
            filter: trigger.filter.clone(),
            authority: trigger.authority.clone(),
            metadata: trigger.metadata.clone(),
            callback: TriggerCallback {
                namespace: trigger.call.namespace.clone(),
                entrypoint: trigger.call.entrypoint.clone(),
            },
        };
        triggers_by_name
            .entry(trigger.call.entrypoint.clone())
            .or_default()
            .push(descriptor);
    }

    let build_descriptor = |func: &semantic::TypedFunction,
                            kind: EntryPointKind|
     -> Result<EmbeddedEntrypointDescriptor, String> {
        let hint_name = entrypoint_ir_symbol_name(func);
        let include_hints = hintable_by_name
            .get(hint_name.as_str())
            .copied()
            .unwrap_or(false);
        let (mut reads, mut writes): (Vec<String>, Vec<String>) = if include_hints {
            hints_by_name
                .get(hint_name.as_str())
                .map(|(r, w)| {
                    (
                        r.iter().cloned().collect::<Vec<_>>(),
                        w.iter().cloned().collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_else(|| (Vec::new(), Vec::new()))
        } else {
            (Vec::new(), Vec::new())
        };
        if include_hints && (reads.is_empty() || writes.is_empty()) {
            let (fallback_reads, fallback_writes) = crate::semantic::function_state_accesses(func);
            if reads.is_empty() && !fallback_reads.is_empty() {
                reads = fallback_reads.iter().cloned().collect();
            }
            if writes.is_empty() && !fallback_writes.is_empty() {
                writes = fallback_writes.iter().cloned().collect();
            }
        }
        let triggers = triggers_by_name
            .get(func.name.as_str())
            .cloned()
            .unwrap_or_default();
        let report = hint_report_by_name.get(hint_name.as_str()).copied();
        let entry_pc = func_start_offsets
            .get(&func.name)
            .copied()
            .ok_or_else(|| format!("missing function offset for entrypoint `{}`", func.name))?;
        Ok(EmbeddedEntrypointDescriptor {
            name: func.name.clone(),
            kind,
            params: func
                .param_types
                .iter()
                .map(|(name, ty)| EntrypointParamDescriptor {
                    name: name.clone(),
                    type_name: semantic::render_type_name(ty),
                })
                .collect(),
            return_type: func.ret_ty.as_ref().map(semantic::render_type_name),
            permission: func.modifiers.permission.clone(),
            read_keys: reads,
            write_keys: writes,
            access_hints_complete: report.map(|r| r.emitted),
            access_hints_skipped: report
                .map(|r| r.skipped_reasons.clone())
                .unwrap_or_default(),
            triggers,
            entry_pc: entry_pc as u64,
        })
    };

    let mut entrypoints: Vec<EmbeddedEntrypointDescriptor> = typed
        .items
        .iter()
        .filter_map(|item| match item {
            TypedItem::Function(func) => {
                let kind = entrypoint_kind_from_modifiers(&func.modifiers)?;
                Some(build_descriptor(func, kind))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if entrypoints.is_empty()
        && let Some(func) = typed.items.iter().find_map(|item| match item {
            TypedItem::Function(func)
                if func.modifiers.kind == FunctionKind::Free && func.name == "main" =>
            {
                Some(func)
            }
            _ => None,
        })
    {
        entrypoints.push(build_descriptor(func, EntryPointKind::Public)?);
    }

    Ok(entrypoints)
}

fn entrypoint_kind_from_modifiers(modifiers: &FunctionModifiers) -> Option<EntryPointKind> {
    match modifiers.kind {
        FunctionKind::View => Some(EntryPointKind::View),
        FunctionKind::Hajimari => Some(EntryPointKind::Hajimari),
        FunctionKind::Kaizen => Some(EntryPointKind::Kaizen),
        _ if modifiers.visibility == FunctionVisibility::Public => Some(EntryPointKind::Public),
        _ => None,
    }
}

pub mod test_helpers {
    use super::*;

    /// Trigger just the CallMulti guard in codegen with a fabricated IR function.
    /// This avoids parsing/semantic stages and focuses on the emission error path.
    pub fn try_emit_callmulti_guard_only(ret_arity: usize) -> Result<(), String> {
        // Build a minimal IR function: one param, and a single CallMulti with `ret_arity` dests.
        let arg = ir::Temp(0);
        let mut dests = Vec::new();
        for i in 0..ret_arity {
            dests.push(ir::Temp(1 + i));
        }
        let bb = ir::BasicBlock {
            label: ir::Label(0),
            instrs: vec![
                ir::Instr::LoadVar {
                    dest: arg,
                    name: "a".to_string(),
                },
                ir::Instr::CallMulti {
                    callee: "g".to_string(),
                    args: vec![arg],
                    dests: dests.clone(),
                },
            ],
            terminator: ir::Terminator::Return(None),
        };
        let func = ir::Function {
            name: "f".to_string(),
            params: vec!["a".to_string()],
            blocks: vec![bb],
            entry: ir::Label(0),
            location: crate::ast::SourceLocation { line: 1, column: 1 },
        };

        // Allocate registers once to mimic real emission environment
        let _alloc = regalloc::allocate(&func);
        // Visit instructions and hit the CallMulti guard path identical to emission.
        for bb in &func.blocks {
            for instr in &bb.instrs {
                if let ir::Instr::CallMulti { callee, dests, .. } = instr
                    && dests.len() > regalloc::MAX_RETURN_VALUES
                {
                    return Err(format!(
                        "too many return values in call to {}: {} > {}",
                        callee,
                        dests.len(),
                        regalloc::MAX_RETURN_VALUES
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_codegen_supported(tp: &semantic::TypedProgram) -> Result<(), String> {
    use semantic::{ExprKind as EK, TypedItem, TypedStatement as S};
    fn expr_ok(e: &semantic::TypedExpr) -> Result<(), String> {
        match &e.expr {
            EK::Conditional {
                cond,
                then_expr,
                else_expr,
            } => {
                expr_ok(cond)?;
                expr_ok(then_expr)?;
                expr_ok(else_expr)?;
                Ok(())
            }
            EK::Binary { left, right, .. } => {
                expr_ok(left)?;
                expr_ok(right)
            }
            EK::Unary { expr, .. } => expr_ok(expr),
            EK::NumericCast { expr } => expr_ok(expr),
            EK::Call { args, .. } => {
                for a in args {
                    expr_ok(a)?;
                }
                Ok(())
            }
            EK::Tuple(elems) => {
                for t in elems {
                    expr_ok(t)?;
                }
                Ok(())
            }
            EK::Member { object, .. } => expr_ok(object),
            EK::Index { target, index } => {
                expr_ok(target)?;
                expr_ok(index)
            }
            EK::Number(_)
            | EK::Decimal(_)
            | EK::Bool(_)
            | EK::String(_)
            | EK::Bytes(_)
            | EK::Ident(_) => Ok(()),
        }
    }
    fn block_ok(b: &semantic::TypedBlock) -> Result<(), String> {
        for s in &b.statements {
            match s {
                S::Let { value, .. } => expr_ok(value)?,
                S::Expr(e) => expr_ok(e)?,
                S::Return(Some(e)) => expr_ok(e)?,
                S::Return(None) | S::Break | S::Continue => {}
                S::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    expr_ok(cond)?;
                    block_ok(then_branch)?;
                    if let Some(b) = else_branch {
                        block_ok(b)?;
                    }
                }
                S::While { cond, body } => {
                    expr_ok(cond)?;
                    block_ok(body)?;
                }
                S::For {
                    init,
                    cond,
                    step,
                    body,
                    ..
                } => {
                    if let Some(i) = init {
                        // Walk the single init statement inline
                        match &**i {
                            S::Let { value, .. } => expr_ok(value)?,
                            S::Expr(e) => expr_ok(e)?,
                            other => {
                                return Err(format!(
                                    "unsupported init statement in for: {other:?}"
                                ));
                            }
                        }
                    }
                    if let Some(c) = cond {
                        expr_ok(c)?;
                    }
                    if let Some(st) = step {
                        match &**st {
                            S::Let { value, .. } => expr_ok(value)?,
                            S::Expr(e) => expr_ok(e)?,
                            other => {
                                return Err(format!(
                                    "unsupported step statement in for: {other:?}"
                                ));
                            }
                        }
                    }
                    block_ok(body)?;
                }
                S::ForEachMap { map, body, .. } => {
                    expr_ok(map)?;
                    block_ok(body)?;
                }
                S::MapSet { map, key, value } => {
                    expr_ok(map)?;
                    expr_ok(key)?;
                    expr_ok(value)?;
                }
            }
        }
        Ok(())
    }
    for item in &tp.items {
        let TypedItem::Function(f) = item;
        block_ok(&f.body)?;
    }
    Ok(())
}

fn validate_feature_requests(
    meta: Option<&ContractMeta>,
    uses_zk: bool,
    uses_vector: bool,
) -> Result<(), String> {
    let Some(meta) = meta else {
        return Ok(());
    };
    let mut errors = Vec::new();
    let meta_requests_zk =
        meta.force_zk == Some(true) || meta.features.contains(&ContractFeature::Zk);
    let meta_forbids_zk = meta.force_zk == Some(false);
    if meta_requests_zk && !uses_zk {
        errors.push("meta requests zk but no zk opcodes are emitted".to_string());
    }
    if meta_forbids_zk && uses_zk {
        errors.push("meta disables zk but zk opcodes are emitted".to_string());
    }
    let meta_requests_vector =
        meta.force_vector == Some(true) || meta.features.contains(&ContractFeature::Vector);
    let meta_forbids_vector = meta.force_vector == Some(false);
    if meta_requests_vector && !uses_vector {
        errors.push("meta requests vector but no vector opcodes are emitted".to_string());
    }
    if meta_forbids_vector && uses_vector {
        errors.push("meta disables vector but vector opcodes are emitted".to_string());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}
