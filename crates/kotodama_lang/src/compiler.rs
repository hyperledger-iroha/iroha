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

use std::collections::{BTreeSet, HashMap, HashSet};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use indexmap::IndexSet;
use iroha_crypto as _; // for Hash types in new APIs
use iroha_data_model::{
    Identifiable,
    account::AccountId,
    asset::id::{AssetDefinitionId, AssetId},
    domain::DomainId,
    escrow::EscrowId,
    isi::{
        BurnBox, ExecuteTrigger, GrantBox, InstructionBox, Log, MintBox, RegisterBox,
        RemoveKeyValueBox, RevokeBox, SetKeyValueBox, TransferBox, UnregisterBox,
    },
    name::Name,
    nft::NftId,
    query::{QueryRequest, SingularQueryBox},
    role::RoleId,
    smart_contract::manifest::{
        AccessSetHints, DynamicAccessHint, EntryPointKind, EntrypointParamDescriptor,
        StateDescriptor, TriggerCallback, TriggerDescriptor,
    },
    trigger::{Trigger, TriggerId},
};
use norito::json;

use super::{
    ast::{
        BinaryOp, Block, ContractFeature, ContractMeta, Expr, FunctionKind, FunctionModifiers,
        FunctionVisibility, Item, Program, Statement, UnaryOp,
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
        EmbeddedSourceLocation, EmbeddedSourceMapEntryV1, EmbeddedStateDescriptor,
        EmbeddedStateFieldDescriptor, EmbeddedStateType, LITERAL_SECTION_MAGIC, ProgramMetadata,
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
const HINT_SKIP_DYNAMIC_STATE_PATH: &str = "dynamic state path is not compiler-resolved";
const HINT_SKIP_CONTRACT_CALL_TARGET: &str = "contract call target is not compiler-resolved";
const HINT_SKIP_OPAQUE_ISI: &str = "opaque ISI access is not compiler-resolved";
const HINT_SKIP_LITERAL_TRIGGER_SPEC_DECODE: &str =
    "literal create_trigger spec could not be decoded for access metadata";
const ACCOUNT_WILDCARD_KEY: &str = "account:*";
const ASSET_WILDCARD_KEY: &str = "asset:*";
const ASSET_DEF_WILDCARD_KEY: &str = "asset_def:*";
const ZK_ASSET_WILDCARD_KEY: &str = "zk_asset:*";
const NFT_COARSE_KEY: &str = "nft";
const AUTHORITY_ACCOUNT_KEY: &str = "account:$authority";
const AUTHORITY_PLACEHOLDER: &str = "$authority";
const TRIGGER_EVENT_PUBLIC_INPUT_KEY: &str = "trigger_event_json";
const COMPILER_FINGERPRINT: &str = concat!("kotodama_lang/", env!("CARGO_PKG_VERSION"));
const FIRST_RELEASE_PRELUDE: &str = r#"
fn require_authority(expected: AccountId) {
  assert(authority() == expected, "authority mismatch");
}

fn require_owner(owner: AccountId) {
  require_authority(owner);
}

fn bps_fee(amount: int, bps: int) -> int {
  assert(amount >= 0, "amount negative");
  assert(bps >= 0, "bps negative");
  return amount * bps / 10000;
}

fn checked_add_amount(left: int, right: int) -> int {
  assert(left >= 0, "left amount negative");
  assert(right >= 0, "right amount negative");
  let result = left + right;
  assert(result >= left, "amount overflow");
  return result;
}

fn checked_sub_amount(left: int, right: int) -> int {
  assert(left >= 0, "left amount negative");
  assert(right >= 0, "right amount negative");
  assert(left >= right, "amount underflow");
  return left - right;
}

fn verify_signed_json(payload: bytes, signature: bytes, public_key: bytes, scheme: int) -> Json {
  assert(verify_signature(payload, signature, public_key, scheme), "signature");
  return decode_json(payload);
}

fn require_json_int(payload: Json, key: Name) -> int {
  return payload.get_int(key);
}
"#;

#[derive(Clone, PartialEq, Eq)]
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

impl AccessSets {
    fn union_with(&mut self, other: &Self) {
        self.reads.extend(other.reads.iter().cloned());
        self.writes.extend(other.writes.iter().cloned());
    }
}

#[derive(Clone, PartialEq, Eq)]
enum StatePathHint {
    Literal(String),
    Map { base: String },
}

#[derive(Clone)]
enum AccountAccessHint {
    Literal(AccountId),
    Authority,
}

#[derive(Clone, PartialEq, Eq)]
struct LiteralPointerFact {
    raw: String,
    kind: ir::DataRefKind,
    is_string_literal: bool,
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
    /// Number of literal trigger specs that could not yield trigger access hints.
    pub literal_trigger_spec_decode_failures: usize,
}

impl AccessHintDiagnostics {
    /// Whether any access-hint fallback occurred.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state_wildcards == 0
            && self.isi_wildcards == 0
            && self.literal_trigger_spec_decode_failures == 0
    }
}

struct HintReport {
    emitted: bool,
    complete: bool,
    skipped_reasons: Vec<String>,
}

fn push_word(code: &mut Vec<u8>, word: u32) {
    code.extend_from_slice(&word.to_le_bytes());
}

fn push_syscall(code: &mut Vec<u8>, number: u32) {
    let word = if let Ok(imm8) = u8::try_from(number) {
        encoding::wide::encode_sys(instruction::wide::system::SCALL, imm8)
    } else {
        encoding::wide::encode_syscallx(number)
    };
    push_word(code, word);
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

fn patch_jump_transfer_stub(
    code: &mut [u8],
    start: usize,
    target: u64,
    pc_bias: u64,
) -> Result<(), String> {
    let runtime_start = start as u64 + pc_bias;
    let runtime_target = target + pc_bias;
    let off = (runtime_target as i64) - (runtime_start as i64);
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

    patch_pointer_literal_stub(code, start, CONTROL_TRANSFER_SCRATCH_REG, runtime_target)?;
    let jalr = encoding::wide::encode_ri(
        instruction::wide::control::JALR,
        0,
        CONTROL_TRANSFER_SCRATCH_REG,
        0,
    );
    write_word(code, start + POINTER_STUB_LEN * 4, jalr);
    Ok(())
}

fn patch_call_transfer_stub(
    code: &mut [u8],
    start: usize,
    target: u64,
    pc_bias: u64,
) -> Result<(), String> {
    let runtime_start = start as u64 + pc_bias;
    let runtime_target = target + pc_bias;
    let off = (runtime_target as i64) - (runtime_start as i64);
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

    patch_pointer_literal_stub(code, start, 1, runtime_target)?;
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
    SoracloudRequest,
    SoracloudResponse,
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
        SoracloudRequest => Some(PointerType::SoracloudRequest),
        SoracloudResponse => Some(PointerType::SoracloudResponse),
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
        SoracloudRequest => DataKey(DataKind::SoracloudRequest, value.to_owned()),
        SoracloudResponse => DataKey(DataKind::SoracloudResponse, value.to_owned()),
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

fn decode_fixed32_chunks(
    raw: &str,
    label: &str,
    allow_empty: bool,
) -> Result<Vec<[u8; 32]>, String> {
    let bytes = decode_hex_or_raw_bytes(raw).map_err(|err| format!("{label} literal {err}"))?;
    if bytes.is_empty() {
        return if allow_empty {
            Ok(Vec::new())
        } else {
            Err(format!("{label} must contain one or more 32-byte chunks"))
        };
    }
    if bytes.len() % 32 != 0 {
        return Err(format!("{label} must be a multiple of 32 bytes"));
    }
    Ok(bytes
        .chunks_exact(32)
        .map(|chunk| {
            let mut out = [0u8; 32];
            out.copy_from_slice(chunk);
            out
        })
        .collect())
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
            let id = iroha_data_model::domain::DomainId::parse_fully_qualified(raw).ok()?;
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
        DRK::SoracloudRequest => {
            let bytes = decode_hex_or_raw_bytes(raw).ok()?;
            let value: iroha_data_model::soracloud::SoracloudHostRequestEnvelopeV1 =
                decode_from_bytes(&bytes).ok()?;
            value.validate().ok()?;
            (PointerType::SoracloudRequest, to_bytes(&value).ok()?)
        }
        DRK::SoracloudResponse => {
            let bytes = decode_hex_or_raw_bytes(raw).ok()?;
            let value: iroha_data_model::soracloud::SoracloudHostResponseEnvelopeV1 =
                decode_from_bytes(&bytes).ok()?;
            value.validate().ok()?;
            (PointerType::SoracloudResponse, to_bytes(&value).ok()?)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompilerMode {
    Production,
    Test,
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
    /// Requested logical vector length; 0 selects the runtime default.
    pub vector_length: u8,
    /// Optional maximum cycles to encode; 0 means "use compiler default".
    pub max_cycles: u64,
    /// Fixed first-release dynamic iteration limit; non-default values are rejected.
    pub dynamic_iter_cap: u8,
    /// Enforce the deterministic on-chain safety profile during compilation.
    pub enforce_on_chain_profile: bool,
    /// Emit additive compiler debug metadata into the artifact.
    pub emit_debug: bool,
    /// Optional logical source path embedded into compiler debug metadata.
    pub debug_source_name: Option<String>,
    /// Controls whether test-only syntax is stripped before compilation.
    pub mode: CompilerMode,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            abi_version: 1,
            force_zk: false,
            force_vector: false,
            vector_length: 0,
            max_cycles: DEFAULT_MAX_CYCLES,
            dynamic_iter_cap: semantic::DYNAMIC_ITERATION_LIMIT as u8,
            enforce_on_chain_profile: true,
            emit_debug: true,
            debug_source_name: None,
            mode: CompilerMode::Production,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use iroha_data_model::{DomainId, asset::id::AssetDefinitionId};

    use super::{
        AUTHORITY_ACCOUNT_KEY, Compiler, CompilerMode, CompilerOptions, ContractFeature,
        DEFAULT_MAX_CYCLES, GLOBAL_WILDCARD_KEY, HINT_SKIP_CONTRACT_CALL_TARGET,
        HINT_SKIP_LITERAL_TRIGGER_SPEC_DECODE, NFT_COARSE_KEY, WIDE_IMM_MAX, emit_addi,
        emit_load64, emit_store64, patch_pointer_literal_stub, pointer_type_for_kind,
        reserve_pointer_literal_stub, retain_taira_supported_access_key, stack_slot_offset_bytes,
    };
    use crate::{ast::ContractMeta, ir, parser::parse, semantic::analyze};
    use crate::{encoding, instruction, metadata::ProgramMetadata, pointer_abi::PointerType};

    fn test_mode_compiler() -> Compiler {
        Compiler::new_with_options(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        })
    }

    fn assert_taira_supported_access_keys(keys: &[String]) {
        assert!(
            keys.iter()
                .all(|key| retain_taira_supported_access_key(key)),
            "manifest persisted unsupported Taira access key in {keys:?}"
        );
    }

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

    fn kotodama_escrow_hex(name: &str) -> String {
        let name: iroha_data_model::name::Name = name.parse().expect("valid escrow name");
        let id = iroha_data_model::escrow::EscrowId::from_kotodama_name(&name);
        hex::encode(id.as_hash().as_ref())
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
            (SoracloudRequest, PointerType::SoracloudRequest),
            (SoracloudResponse, PointerType::SoracloudResponse),
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
    fn loop_phi_lowering_is_deterministic() {
        let compiler = Compiler::new();
        let src = r#"
seiyaku StableLoop {
  kotoage fn main() -> int {
    let alpha = 1;
    let beta = 2;
    let gamma = 3;
    let delta = 4;
    let epsilon = 5;
    let cursor = 0;
    while cursor < 4 {
      if alpha < beta {
        gamma = gamma + delta;
      } else {
        delta = delta + gamma;
      }
      alpha = alpha + 1;
      beta = beta + 2;
      epsilon = epsilon + alpha;
      cursor = cursor + 1;
    }
    return alpha + beta + gamma + delta + epsilon + cursor;
  }
}
"#;
        let first = compiler.compile_source(src).expect("first compile");
        for _ in 0..8 {
            let next = compiler.compile_source(src).expect("repeat compile");
            assert_eq!(next, first);
        }
    }

    #[test]
    fn compiler_options_reject_vector_length_above_abi_max() {
        let opts = CompilerOptions {
            vector_length: ivm_abi::metadata::VECTOR_LENGTH_MAX + 1,
            ..CompilerOptions::default()
        };
        let compiler = Compiler::new_with_options(opts);
        let src = r#"
seiyaku MyC {
  hajimari() { let a = 1; }
}
"#;
        let err = compiler.compile_source(src).unwrap_err();
        assert!(
            err.contains("unsupported vector_length"),
            "unexpected error: {err}"
        );
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
        super::patch_jump_transfer_stub(&mut code, start, 200_000, 0).expect("patch far jump");
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
        super::patch_call_transfer_stub(&mut code, start, (start + 16) as u64, 0)
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
    fn far_call_fixup_includes_runtime_prefix_bias_in_literal_target() {
        let mut code = Vec::new();
        let start = super::reserve_control_transfer_stub(&mut code);
        let target = 200_000u64;
        let bias = 8_192u64;
        super::patch_call_transfer_stub(&mut code, start, target, bias).expect("patch far call");

        let mut expected = vec![0u8; super::POINTER_STUB_LEN * 4];
        super::patch_pointer_literal_stub(&mut expected, 0, 1, target + bias)
            .expect("patch expected pointer literal");
        assert_eq!(
            &code[start..start + super::POINTER_STUB_LEN * 4],
            expected.as_slice()
        );

        let final_word_at = start + super::POINTER_STUB_LEN * 4;
        let final_word =
            u32::from_le_bytes(code[final_word_at..final_word_at + 4].try_into().unwrap());
        assert_eq!(
            final_word,
            encoding::wide::encode_ri(instruction::wide::control::JALR, 1, 1, 0)
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
        let compiler = test_mode_compiler();
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
    fn get_numeric_emits_numeric_syscall() {
        let src = r#"
seiyaku JsonNumericTest {
  meta { abi_version: 1; }
  fn run() {
    let ev = trigger_event();
    let _amount: Amount = ev.get_numeric(name("amount"));
  }
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler.compile_source(src).expect("compile get_numeric");
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
    fn get_asset_definition_id_emits_asset_definition_syscall() {
        let src = r#"
seiyaku JsonAssetDefinitionTest {
  meta { abi_version: 1; }
  fn run() {
    let ev = trigger_event();
    let _asset = ev.get_asset_definition_id(name("asset_definition_id"));
  }
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile get_asset_definition_id");
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
    fn native_escrow_builtins_emit_escrow_syscalls() {
        let src = r#"
fn main() {
  let evidence = norito_bytes("00");
  escrow_open_offer(name("aitai_offer"), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 10, evidence);
  escrow_accept(name("aitai_offer"));
  escrow_mark_payment_sent(name("aitai_offer"));
  escrow_release(name("aitai_offer"));
  escrow_cancel(name("aitai_offer"));
  escrow_open_dispute(name("aitai_offer"), evidence);
  escrow_resolve_dispute(name("aitai_offer"), 6, 4, evidence);
  call escrow_open_offer(name("aitai_offer_call"), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 11, evidence);
  call escrow_accept(name("aitai_offer_call"));
  call escrow_mark_payment_sent(name("aitai_offer_call"));
  call escrow_release(name("aitai_offer_call"));
  call escrow_cancel(name("aitai_offer_call"));
  call escrow_open_dispute(name("aitai_offer_call"), evidence);
  call escrow_resolve_dispute(name("aitai_offer_call"), 7, 4, evidence);
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile native escrow builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_ESCROW_OPEN_OFFER,
                "ESCROW_OPEN_OFFER",
            ),
            (ivm_abi::syscalls::SYSCALL_ESCROW_ACCEPT, "ESCROW_ACCEPT"),
            (
                ivm_abi::syscalls::SYSCALL_ESCROW_MARK_PAYMENT_SENT,
                "ESCROW_MARK_PAYMENT_SENT",
            ),
            (ivm_abi::syscalls::SYSCALL_ESCROW_RELEASE, "ESCROW_RELEASE"),
            (ivm_abi::syscalls::SYSCALL_ESCROW_CANCEL, "ESCROW_CANCEL"),
            (
                ivm_abi::syscalls::SYSCALL_ESCROW_OPEN_DISPUTE,
                "ESCROW_OPEN_DISPUTE",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ESCROW_RESOLVE_DISPUTE,
                "ESCROW_RESOLVE_DISPUTE",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("escrow syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn native_anonymous_escrow_builtins_emit_escrow_syscalls() {
        let src = r#"
fn main() {
  let request = norito_bytes("00");
  let evidence = norito_bytes("01");
  anonymous_escrow_open_offer(request);
  anonymous_escrow_accept(name("shielded_offer"));
  anonymous_escrow_mark_payment_sent(name("shielded_offer"));
  anonymous_escrow_release(request);
  anonymous_escrow_cancel(request);
  anonymous_escrow_open_dispute(name("shielded_offer"), evidence);
  anonymous_escrow_resolve_dispute(request);
  call anonymous_escrow_open_offer(request);
  call anonymous_escrow_accept(name("shielded_offer_call"));
  call anonymous_escrow_mark_payment_sent(name("shielded_offer_call"));
  call anonymous_escrow_release(request);
  call anonymous_escrow_cancel(request);
  call anonymous_escrow_open_dispute(name("shielded_offer_call"), evidence);
  call anonymous_escrow_resolve_dispute(request);
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile native anonymous escrow builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER,
                "ANONYMOUS_ESCROW_OPEN_OFFER",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ANONYMOUS_ESCROW_ACCEPT,
                "ANONYMOUS_ESCROW_ACCEPT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT,
                "ANONYMOUS_ESCROW_MARK_PAYMENT_SENT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ANONYMOUS_ESCROW_RELEASE,
                "ANONYMOUS_ESCROW_RELEASE",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ANONYMOUS_ESCROW_CANCEL,
                "ANONYMOUS_ESCROW_CANCEL",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE,
                "ANONYMOUS_ESCROW_OPEN_DISPUTE",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE,
                "ANONYMOUS_ESCROW_RESOLVE_DISPUTE",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("anonymous escrow syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn native_escrow_builtins_report_literal_access_hints() {
        let asset_def = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"
fn main() {{
  let evidence = norito_bytes("00");
  escrow_open_offer(name("aitai_offer"), asset_definition("{asset_def}"), 10, evidence);
  escrow_accept(name("aitai_offer"));
  escrow_mark_payment_sent(name("aitai_offer"));
  escrow_release(name("aitai_offer"));
  escrow_cancel(name("aitai_offer"));
  escrow_open_dispute(name("aitai_offer"), evidence);
  escrow_resolve_dispute(name("aitai_offer"), 6, 4, evidence);
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile native escrow access hints");
        let hints = manifest
            .access_set_hints
            .expect("expected native escrow access hints");
        let escrow_hash = kotodama_escrow_hex("aitai_offer");
        for key in [
            format!("escrow_id:{escrow_hash}"),
            format!("asset_escrow:{escrow_hash}"),
            format!("asset_def:{asset_def}"),
            format!("asset:{asset_def}:$authority"),
        ] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn named_anonymous_escrow_builtins_report_literal_access_hints() {
        let src = r#"
fn main() {
  let evidence = norito_bytes("01");
  anonymous_escrow_accept(name("shielded_offer"));
  anonymous_escrow_mark_payment_sent(name("shielded_offer"));
  anonymous_escrow_open_dispute(name("shielded_offer"), evidence);
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile anonymous escrow access hints");
        let hints = manifest
            .access_set_hints
            .expect("expected anonymous escrow access hints");
        let escrow_hash = kotodama_escrow_hex("shielded_offer");
        for key in [
            format!("escrow_id:{escrow_hash}"),
            format!("anonymous_asset_escrow:{escrow_hash}"),
        ] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn literal_anonymous_escrow_request_reports_access_hints() {
        use iroha_data_model::{
            asset::AssetDefinitionId,
            isi::escrow::OpenAnonymousAssetEscrow,
            proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        };

        let asset_def: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("asset definition");
        let escrow_name: iroha_data_model::name::Name =
            "shielded_offer".parse().expect("escrow name");
        let escrow_id = iroha_data_model::escrow::EscrowId::from_kotodama_name(&escrow_name);
        let backend = "halo2/ipa/poly-open".to_string();
        let proof = ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![1, 2, 3]),
            VerifyingKeyId::new(backend, "escrow_vk"),
        );
        let request = OpenAnonymousAssetEscrow::new(
            escrow_id,
            asset_def.clone(),
            vec![[0x11; 32]],
            [0x22; 32],
            proof,
            None,
        );
        let hex_payload = format!(
            "0x{}",
            hex::encode(norito::to_bytes(&request).expect("encode anonymous escrow request"))
        );
        let src = format!(
            r#"
fn main() {{
  anonymous_escrow_open_offer(norito_bytes("{hex_payload}"));
}}
"#
        );

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile literal anonymous escrow request");
        let hints = manifest
            .access_set_hints
            .expect("expected anonymous request access hints");
        let escrow_hash = kotodama_escrow_hex("shielded_offer");
        for key in [
            format!("escrow_id:{escrow_hash}"),
            format!("anonymous_asset_escrow:{escrow_hash}"),
            format!("zk_asset:{asset_def}"),
        ] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }
        let asset_def_key = format!("asset_def:{asset_def}");
        assert!(
            hints.read_keys.contains(&asset_def_key),
            "missing read key {asset_def_key}"
        );
        assert!(
            !hints.write_keys.contains(&asset_def_key),
            "anonymous escrow open should not write asset definition key {asset_def_key}"
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
    fn escrow_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  escrow_open_offer(name("deal"), name("rose"), 10);
}
"#,
                "escrow_open_offer expects (Name, AssetDefinitionId, numeric[, Blob|bytes evidence_hashes])",
            ),
            (
                r#"
fn main() {
  call escrow_accept(1);
}
"#,
                "escrow_accept expects (Name)",
            ),
            (
                r#"
fn main() {
  escrow_open_dispute(name("deal"), 1);
}
"#,
                "escrow_open_dispute expects (Name[, Blob|bytes evidence_hashes])",
            ),
            (
                r#"
fn main() {
  call escrow_resolve_dispute(name("deal"), name("buyer"), 4);
}
"#,
                "escrow_resolve_dispute expects (Name, numeric, numeric[, Blob|bytes evidence_hashes])",
            ),
            (
                r#"
fn main() {
  anonymous_escrow_open_offer(name("deal"));
}
"#,
                "anonymous_escrow_open_offer expects (Blob|bytes) Norito request payload",
            ),
            (
                r#"
fn main() {
  call anonymous_escrow_accept(1);
}
"#,
                "anonymous_escrow_accept expects (Name)",
            ),
            (
                r#"
fn main() {
  anonymous_escrow_open_dispute(name("deal"), 1);
}
"#,
                "anonymous_escrow_open_dispute expects (Name[, Blob|bytes evidence_hashes])",
            ),
        ] {
            let parsed = parse(src).expect("parse invalid escrow source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject escrow args");
            assert!(
                err.message.contains(expected),
                "expected error containing {expected:?}, got {}",
                err.message
            );
        }
    }

    #[test]
    fn soracloud_runtime_builtins_emit_soracloud_syscalls() {
        let src = r#"
fn main(request: SoracloudRequest) {
  let _read_state = soracloud_read_committed_state(request);
  let _mutation = soracloud_emit_state_mutation(request);
  let _mailbox = soracloud_emit_mailbox_message(request);
  let _journal = soracloud_append_journal(request);
  let _checkpoint = soracloud_publish_checkpoint(request);
  let _secret = soracloud_read_secret(request);
  let _credential = soracloud_read_credential(request);
  let _fetch = soracloud_egress_fetch(request);
  let _config = soracloud_read_config(request);
  let _secret_envelope = soracloud_read_secret_envelope(request);
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile Soracloud runtime builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE,
                "SORACLOUD_READ_COMMITTED_STATE",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_EMIT_STATE_MUTATION,
                "SORACLOUD_EMIT_STATE_MUTATION",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE,
                "SORACLOUD_EMIT_MAILBOX_MESSAGE",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_APPEND_JOURNAL,
                "SORACLOUD_APPEND_JOURNAL",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT,
                "SORACLOUD_PUBLISH_CHECKPOINT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_READ_SECRET,
                "SORACLOUD_READ_SECRET",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_READ_CREDENTIAL,
                "SORACLOUD_READ_CREDENTIAL",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_EGRESS_FETCH,
                "SORACLOUD_EGRESS_FETCH",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_READ_CONFIG,
                "SORACLOUD_READ_CONFIG",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE,
                "SORACLOUD_READ_SECRET_ENVELOPE",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("Soracloud syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn soracloud_runtime_builtins_reject_norito_bytes_request_argument() {
        let parsed = parse(
            r#"
fn main() {
  let request = norito_bytes("00");
  let _response = soracloud_read_config(request);
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject request type");
        assert!(
            err.message
                .contains("soracloud_read_config expects (SoracloudRequest)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn account_multisig_admin_builtins_emit_account_admin_syscalls() {
        let account = sample_account_literal();
        let src = format!(
            r#"
fn main() {{
  let account = account_id("{account}");
  let signatory = json("\"ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774\"");
  add_signatory(account, signatory);
  remove_signatory(account, signatory);
  set_account_quorum(account, 2);
  call add_signatory(account, signatory);
  call remove_signatory(account, signatory);
  call set_account_quorum(account, 3);
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile account multisig admin builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_ADD_SIGNATORY, "ADD_SIGNATORY"),
            (
                ivm_abi::syscalls::SYSCALL_REMOVE_SIGNATORY,
                "REMOVE_SIGNATORY",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SET_ACCOUNT_QUORUM,
                "SET_ACCOUNT_QUORUM",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("account admin syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let hints = manifest
            .access_set_hints
            .expect("expected account multisig access hints");
        assert!(hints.read_keys.contains(&format!("account:{account}")));
        assert!(hints.write_keys.contains(&format!("account:{account}")));
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
    }

    #[test]
    fn account_multisig_admin_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main(account: AccountId) {
  add_signatory(account, name("not_json"));
}
"#,
                "add_signatory expects (AccountId, Json)",
            ),
            (
                r#"
fn main(account: AccountId) {
  call set_account_quorum(account, json("{}"));
}
"#,
                "set_account_quorum expects (AccountId, numeric)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject account admin args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn account_balance_query_builtin_emits_balance_syscall_and_exact_reads() {
        let account = sample_account_literal();
        let asset_definition = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"
view fn read() -> Balance {{
  let account = account_id("{account}");
  let asset = asset_definition("{asset_definition}");
  return get_account_balance(account, asset);
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile account balance query builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_GET_ACCOUNT_BALANCE)
                .expect("account balance syscall id fits in u8"),
        )
        .to_le_bytes();

        assert!(
            code.windows(needle.len()).any(|window| window == needle),
            "expected GET_ACCOUNT_BALANCE syscall in compiled code"
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let read = entrypoints
            .iter()
            .find(|entry| entry.name == "read")
            .expect("read entrypoint");
        assert_eq!(read.access_hints_complete, Some(true));
        assert!(read.access_hints_skipped.is_empty());
        assert!(read.write_keys.is_empty());
        assert!(read.read_keys.contains(&format!("account:{account}")));
        assert!(
            read.read_keys
                .contains(&format!("asset:{asset_definition}#{account}"))
        );
        assert!(
            read.read_keys
                .contains(&format!("asset_def:{asset_definition}"))
        );
    }

    #[test]
    fn account_balance_query_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main(account: AccountId) {
  let _balance = get_account_balance(account, name("not_asset"));
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject account balance args");
        assert!(
            err.message
                .contains("get_account_balance expects (AccountId, AssetDefinitionId)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn set_account_detail_builtin_emits_syscall_and_exact_access() {
        let account = sample_account_literal();
        let src = format!(
            r#"
fn main() {{
  set_account_detail(account_id("{account}"), name("status"), json("{{}}"));
  call set_account_detail(account_id("{account}"), name("mirror"), json("{{}}"));
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile set_account_detail builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SET_ACCOUNT_DETAIL,
                "SET_ACCOUNT_DETAIL",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("account-detail syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let hints = manifest
            .access_set_hints
            .expect("expected account detail access hints");
        assert!(hints.read_keys.contains(&format!("account:{account}")));
        for key in ["status", "mirror"] {
            let detail = format!("account.detail:{account}:{key}");
            assert!(hints.read_keys.contains(&detail));
            assert!(hints.write_keys.contains(&detail));
        }
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
    }

    #[test]
    fn set_account_detail_builtin_rejects_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main(account: AccountId) {
  set_account_detail(account, 1, json("{}"));
}
"#,
                "set_account_detail expects (AccountId, Name, Json)",
            ),
            (
                r#"
fn main(account: AccountId) {
  call set_account_detail(account, name("status"), name("not_json"));
}
"#,
                "set_account_detail expects (AccountId, Name, Json)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject account detail args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn native_asset_operation_builtins_emit_syscalls_and_exact_access() {
        let from = sample_account_id();
        let to = sample_account_id_alt();
        let from_literal = from.to_string();
        let to_literal = to.to_string();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let asset_definition =
            iroha_data_model::asset::id::AssetDefinitionId::parse_address_literal(asset_literal)
                .expect("asset definition literal");
        let from_asset =
            iroha_data_model::asset::id::AssetId::of(asset_definition.clone(), from.clone());
        let to_asset =
            iroha_data_model::asset::id::AssetId::of(asset_definition.clone(), to.clone());
        let src = format!(
            r#"
fn main() {{
  transfer_asset(account_id("{from_literal}"), account_id("{to_literal}"), asset_definition("{asset_literal}"), 1);
  mint_asset(account_id("{to_literal}"), asset_definition("{asset_literal}"), 2);
  burn_asset(account_id("{from_literal}"), asset_definition("{asset_literal}"), 1);
  call transfer_asset(account_id("{to_literal}"), account_id("{from_literal}"), asset_definition("{asset_literal}"), 1);
  call mint_asset(account_id("{from_literal}"), asset_definition("{asset_literal}"), 2);
  call burn_asset(account_id("{to_literal}"), asset_definition("{asset_literal}"), 1);
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile native asset operation builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_TRANSFER_ASSET, "TRANSFER_ASSET"),
            (ivm_abi::syscalls::SYSCALL_MINT_ASSET, "MINT_ASSET"),
            (ivm_abi::syscalls::SYSCALL_BURN_ASSET, "BURN_ASSET"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("asset operation syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let hints = manifest
            .access_set_hints
            .expect("expected asset operation access hints");
        assert!(
            hints
                .read_keys
                .contains(&format!("asset_def:{asset_definition}"))
        );
        for key in [format!("account:{from}"), format!("account:{to}")] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
        }
        for key in [format!("asset:{from_asset}"), format!("asset:{to_asset}")] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
    }

    #[test]
    fn native_asset_operation_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main(account: AccountId) {
  transfer_asset(account, account, name("not_asset"), 1);
}
"#,
                "transfer_asset expects (AccountId, AccountId, AssetDefinitionId, numeric)",
            ),
            (
                r#"
fn main(account: AccountId, asset: AssetDefinitionId) {
  call mint_asset(account, asset, json("{}"));
}
"#,
                "mint_asset expects (AccountId, AssetDefinitionId, numeric)",
            ),
            (
                r#"
fn main(account: AccountId, asset: AssetDefinitionId) {
  call burn_asset(account, name("not_asset"), 1);
}
"#,
                "burn_asset expects (AccountId, AssetDefinitionId, numeric)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject asset operation args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn nft_asset_operation_builtins_emit_syscalls_and_exact_access() {
        let owner = sample_account_id();
        let recipient = sample_account_id_alt();
        let owner_literal = owner.to_string();
        let recipient_literal = recipient.to_string();
        let nft = "n0$wonderland.universal";
        let nft_alt = "n1$wonderland.universal";
        let src = format!(
            r#"
fn main() {{
  nft_mint_asset(nft_id("{nft}"), account_id("{owner_literal}"));
  nft_set_metadata(nft_id("{nft}"), name("issued"), json("{{\"meta\":1}}"));
  nft_transfer_asset(account_id("{owner_literal}"), nft_id("{nft}"), account_id("{recipient_literal}"));
  nft_burn_asset(nft_id("{nft}"));
  call nft_mint_asset(nft_id("{nft_alt}"), account_id("{recipient_literal}"));
  call nft_set_metadata(nft_id("{nft_alt}"), name("mirror"), json("{{\"meta\":2}}"));
  call nft_transfer_asset(account_id("{recipient_literal}"), nft_id("{nft_alt}"), account_id("{owner_literal}"));
  call nft_burn_asset(nft_id("{nft_alt}"));
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile NFT asset operation builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_NFT_MINT_ASSET, "NFT_MINT_ASSET"),
            (
                ivm_abi::syscalls::SYSCALL_NFT_SET_METADATA,
                "NFT_SET_METADATA",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NFT_TRANSFER_ASSET,
                "NFT_TRANSFER_ASSET",
            ),
            (ivm_abi::syscalls::SYSCALL_NFT_BURN_ASSET, "NFT_BURN_ASSET"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("NFT operation syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let hints = manifest
            .access_set_hints
            .expect("expected NFT operation access hints");
        assert!(hints.read_keys.contains(&NFT_COARSE_KEY.to_string()));
        assert!(hints.write_keys.contains(&NFT_COARSE_KEY.to_string()));
        for key in [
            format!("account:{owner}"),
            format!("account:{recipient}"),
            format!("nft:{nft}"),
            format!("nft:{nft_alt}"),
        ] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
        }
        for key in [format!("nft:{nft}"), format!("nft:{nft_alt}")] {
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }
        for detail in [
            format!("nft.detail:{nft}:issued"),
            format!("nft.detail:{nft_alt}:mirror"),
        ] {
            assert!(
                hints.read_keys.contains(&detail),
                "missing read detail {detail}"
            );
            assert!(
                hints.write_keys.contains(&detail),
                "missing write detail {detail}"
            );
        }
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
    }

    #[test]
    fn nft_asset_operation_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main(account: AccountId) {
  nft_mint_asset(name("bad"), account);
}
"#,
                "nft_mint_asset expects (NftId, AccountId)",
            ),
            (
                r#"
fn main(nft: NftId) {
  call nft_set_metadata(nft, 1, json("{}"));
}
"#,
                "nft_set_metadata expects (NftId, Name, Json)",
            ),
            (
                r#"
fn main() {
  nft_burn_asset(name("bad"));
}
"#,
                "nft_burn_asset expects (NftId)",
            ),
            (
                r#"
fn main(account: AccountId, nft: NftId) {
  call nft_transfer_asset(account, nft, name("bad"));
}
"#,
                "nft_transfer_asset expects (AccountId, NftId, AccountId)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject NFT operation args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn lifecycle_builtins_emit_syscalls_and_exact_access() {
        let owner = sample_account_id();
        let recipient = sample_account_id_alt();
        let owner_literal = owner.to_string();
        let recipient_literal = recipient.to_string();
        let domain = "wonderland.universal";
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let asset_definition =
            iroha_data_model::asset::id::AssetDefinitionId::parse_address_literal(asset_literal)
                .expect("asset definition literal");
        let owner_asset =
            iroha_data_model::asset::id::AssetId::of(asset_definition.clone(), owner.clone());
        let src = format!(
            r#"
fn main() {{
  let domain_id = domain("{domain}");
  let owner = account_id("{owner_literal}");
  let recipient = account_id("{recipient_literal}");
  let asset = asset_definition("{asset_literal}");
  register_domain(domain_id);
  unregister_domain(domain_id);
  transfer_domain(owner, domain_id, recipient);
  register_account(owner);
  unregister_account(recipient);
  register_asset(asset, "ROSE", 0, 1);
  create_new_asset(asset, "ROSE", 7, owner, 1);
  unregister_asset(asset);
  call register_domain(domain_id);
  call unregister_domain(domain_id);
  call transfer_domain(owner, domain_id, recipient);
  call register_account(recipient);
  call unregister_account(owner);
  call register_asset(asset, "ROSE", 0, 1);
  call create_new_asset(asset, "ROSE", 3, owner, 1);
  call unregister_asset(asset);
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile lifecycle builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_REGISTER_DOMAIN,
                "REGISTER_DOMAIN",
            ),
            (
                ivm_abi::syscalls::SYSCALL_UNREGISTER_DOMAIN,
                "UNREGISTER_DOMAIN",
            ),
            (
                ivm_abi::syscalls::SYSCALL_TRANSFER_DOMAIN,
                "TRANSFER_DOMAIN",
            ),
            (
                ivm_abi::syscalls::SYSCALL_REGISTER_ACCOUNT,
                "REGISTER_ACCOUNT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_UNREGISTER_ACCOUNT,
                "UNREGISTER_ACCOUNT",
            ),
            (ivm_abi::syscalls::SYSCALL_REGISTER_ASSET, "REGISTER_ASSET"),
            (
                ivm_abi::syscalls::SYSCALL_UNREGISTER_ASSET,
                "UNREGISTER_ASSET",
            ),
            (ivm_abi::syscalls::SYSCALL_MINT_ASSET, "MINT_ASSET"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("lifecycle syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let hints = manifest
            .access_set_hints
            .expect("expected lifecycle access hints");
        for key in [
            format!("domain:{domain}"),
            format!("account:{owner}"),
            format!("account:{recipient}"),
            format!("asset_def:{asset_definition}"),
            format!("asset:{owner_asset}"),
        ] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
        }
        for key in [
            format!("domain:{domain}"),
            format!("account:{owner}"),
            format!("account:{recipient}"),
            format!("asset_def:{asset_definition}"),
            format!("asset:{owner_asset}"),
        ] {
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
    }

    #[test]
    fn lifecycle_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  call register_domain(name("bad"));
}
"#,
                "register_domain expects (DomainId)",
            ),
            (
                r#"
fn main() {
  register_account(name("bad"));
}
"#,
                "register_account expects (AccountId)",
            ),
            (
                r#"
fn main() {
  unregister_asset(name("bad"));
}
"#,
                "unregister_asset expects (AssetDefinitionId)",
            ),
            (
                r#"
fn main(asset: AssetDefinitionId) {
  register_asset(name("bad"), "ROSE", 1, 0);
}
"#,
                "register_asset expects (AssetDefinitionId, string, int, int)",
            ),
            (
                r#"
fn main(asset: AssetDefinitionId, account: AccountId) {
  call create_new_asset(asset, json("{}"), 1, account, 0);
}
"#,
                "create_new_asset expects (AssetDefinitionId, string, int, AccountId, int)",
            ),
            (
                r#"
fn main(account: AccountId) {
  transfer_domain(account, json("{}"), account);
}
"#,
                "transfer_domain expects (AccountId, DomainId, AccountId)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed)
                .expect_err("semantic analysis should reject lifecycle operation args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn peer_trigger_management_builtins_emit_syscalls() {
        let src = r#"
fn main() {
  let trigger = name("wake");
  register_peer(json("{}"));
  unregister_peer(json("{}"));
  create_trigger(json("{}"));
  register_trigger(json("{}"));
  remove_trigger(trigger);
  unregister_trigger(trigger);
  set_trigger_enabled(trigger, 1);
  call register_peer(json("{}"));
  call unregister_peer(json("{}"));
  call create_trigger(json("{}"));
  call register_trigger(json("{}"));
  call remove_trigger(trigger);
  call unregister_trigger(trigger);
  call set_trigger_enabled(trigger, 0);
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile peer/trigger management builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_REGISTER_PEER, "REGISTER_PEER"),
            (
                ivm_abi::syscalls::SYSCALL_UNREGISTER_PEER,
                "UNREGISTER_PEER",
            ),
            (ivm_abi::syscalls::SYSCALL_CREATE_TRIGGER, "CREATE_TRIGGER"),
            (ivm_abi::syscalls::SYSCALL_REMOVE_TRIGGER, "REMOVE_TRIGGER"),
            (
                ivm_abi::syscalls::SYSCALL_SET_TRIGGER_ENABLED,
                "SET_TRIGGER_ENABLED",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("peer/trigger syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn peer_trigger_management_builtins_report_exact_trigger_access() {
        let src = r#"
fn main() {
  let trigger = name("wake");
  remove_trigger(trigger);
  unregister_trigger(trigger);
  set_trigger_enabled(trigger, 1);
  call remove_trigger(trigger);
  call unregister_trigger(trigger);
  call set_trigger_enabled(trigger, 0);
}
"#;
        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile trigger access builtins");
        let hints = manifest
            .access_set_hints
            .expect("expected trigger access hints");
        let trigger_key = "trigger:wake".to_string();
        assert!(hints.read_keys.contains(&trigger_key));
        assert!(hints.write_keys.contains(&trigger_key));
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
    }

    #[test]
    fn role_permission_management_builtins_emit_syscalls_and_exact_access() {
        let account = sample_account_id();
        let account_literal = account.to_string();
        let src = format!(
            r#"
fn main() {{
  let account = account_id("{account_literal}");
  let role = name("auditor");
  let perm = name("read_blocks");
  create_role(role, json("{{}}"));
  grant_role(account, role);
  revoke_role(account, role);
  grant_permission(account, perm);
  revoke_permission(account, perm);
  delete_role(role);
  call create_role(role, json("{{}}"));
  call grant_role(account, role);
  call revoke_role(account, role);
  call grant_permission(account, perm);
  call revoke_permission(account, perm);
  call delete_role(role);
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile role/permission management builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_CREATE_ROLE, "CREATE_ROLE"),
            (ivm_abi::syscalls::SYSCALL_DELETE_ROLE, "DELETE_ROLE"),
            (ivm_abi::syscalls::SYSCALL_GRANT_ROLE, "GRANT_ROLE"),
            (ivm_abi::syscalls::SYSCALL_REVOKE_ROLE, "REVOKE_ROLE"),
            (
                ivm_abi::syscalls::SYSCALL_GRANT_PERMISSION,
                "GRANT_PERMISSION",
            ),
            (
                ivm_abi::syscalls::SYSCALL_REVOKE_PERMISSION,
                "REVOKE_PERMISSION",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("role/permission syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let hints = manifest
            .access_set_hints
            .expect("expected role/permission access hints");
        for key in [format!("account:{account}"), "role:auditor".to_string()] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
        }
        for key in [
            format!("account:{account}"),
            "role:auditor".to_string(),
            format!("role.binding:{account}:auditor"),
            format!("perm.account:{account}:read_blocks"),
        ] {
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
    }

    #[test]
    fn role_permission_peer_trigger_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  call register_peer(name("bad"));
}
"#,
                "register_peer expects (Json)",
            ),
            (
                r#"
fn main() {
  create_trigger(name("bad"));
}
"#,
                "create_trigger expects (Json)",
            ),
            (
                r#"
fn main() {
  call unregister_trigger(json("{}"));
}
"#,
                "unregister_trigger expects (Name)",
            ),
            (
                r#"
fn main() {
  set_trigger_enabled(name("wake"), json("{}"));
}
"#,
                "set_trigger_enabled expects (Name, int)",
            ),
            (
                r#"
fn main() {
  create_role(name("auditor"), name("read_blocks"));
}
"#,
                "create_role expects (Name, Json)",
            ),
            (
                r#"
fn main() {
  call delete_role(json("{}"));
}
"#,
                "delete_role expects (Name)",
            ),
            (
                r#"
fn main(account: AccountId) {
  grant_role(account, json("{}"));
}
"#,
                "grant/revoke_role expects (AccountId, Name)",
            ),
            (
                r#"
fn main(account: AccountId) {
  call revoke_permission(account, 1);
}
"#,
                "grant/revoke_permission expects (AccountId, Name|Json)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed)
                .expect_err("semantic analysis should reject management helper args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn public_input_builtin_emits_public_input_syscall_and_complete_access() {
        let src = r#"
view fn read_input() -> bytes {
  return get_public_input(name("proof_payload"));
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile get_public_input builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (
                ivm_abi::syscalls::SYSCALL_GET_PUBLIC_INPUT,
                "GET_PUBLIC_INPUT",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("public-input syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let read_input = entrypoints
            .iter()
            .find(|entry| entry.name == "read_input")
            .expect("read_input entrypoint");
        assert_ne!(read_input.access_hints_complete, Some(false));
        assert!(read_input.access_hints_skipped.is_empty());
        assert!(read_input.read_keys.is_empty());
        assert!(read_input.write_keys.is_empty());
    }

    #[test]
    fn public_input_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let _payload = get_public_input(1);
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject public input key type");
        assert!(
            err.message.contains("get_public_input expects (Name)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn debug_builtins_emit_debug_syscalls_and_complete_access() {
        let src = r#"
view fn inspect() -> int {
  debug_print(42);
  debug_log(json!{ status: "ok" });
  debug_log(blob("hello"));
  debug_log(norito_bytes("00"));
  return 1;
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile debug builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_DEBUG_PRINT, "DEBUG_PRINT"),
            (ivm_abi::syscalls::SYSCALL_DEBUG_LOG, "DEBUG_LOG"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("debug syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let publish_needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
        )
        .to_le_bytes();
        assert!(
            !code
                .windows(publish_needle.len())
                .any(|window| window == publish_needle),
            "debug helpers must not publish host input TLVs"
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let inspect = entrypoints
            .iter()
            .find(|entry| entry.name == "inspect")
            .expect("inspect entrypoint");
        assert_ne!(inspect.access_hints_complete, Some(false));
        assert!(inspect.access_hints_skipped.is_empty());
        assert!(inspect.read_keys.is_empty());
        assert!(inspect.write_keys.is_empty());
    }

    #[test]
    fn debug_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  debug_print(name("not_int"));
}
"#,
                "debug_print expects (int value)",
            ),
            (
                r#"
fn main() {
  debug_log(name("not_payload"));
}
"#,
                "debug_log expects (Json|Blob|bytes payload)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject invalid args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn assertion_logging_builtins_emit_abort_and_log_syscalls() {
        let src = r#"
view fn inspect() -> int {
  info("ready");
  info(7);
  assert(true);
  require(true);
  assert_eq(1, 1);
  return 1;
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile assertion/logging builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_DEBUG_LOG, "DEBUG_LOG"),
            (ivm_abi::syscalls::SYSCALL_ABORT, "ABORT"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("assertion/logging syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let inspect = entrypoints
            .iter()
            .find(|entry| entry.name == "inspect")
            .expect("inspect entrypoint");
        assert_ne!(inspect.access_hints_complete, Some(false));
        assert!(inspect.access_hints_skipped.is_empty());
        assert!(inspect.read_keys.is_empty());
        assert!(inspect.write_keys.is_empty());
    }

    #[test]
    fn assertion_logging_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  assert(1);
}
"#,
                "assert expects (bool) or (bool, string|int)",
            ),
            (
                r#"
fn main() {
  require(true, false);
}
"#,
                "require expects (bool) or (bool, string|int)",
            ),
            (
                r#"
fn main() {
  info(name("not_scalar"));
}
"#,
                "info expects (string|int)",
            ),
            (
                r#"
fn main() {
  assert_eq(true, 1);
}
"#,
                "assert_eq expects two int args",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject invalid args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn privacy_output_builtins_emit_runtime_syscalls() {
        let src = r#"
fn main() {
  let secret = get_private_input(0);
  use_nullifier(secret);
  commit_output();
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile privacy/output builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_GET_PRIVATE_INPUT,
                "GET_PRIVATE_INPUT",
            ),
            (ivm_abi::syscalls::SYSCALL_USE_NULLIFIER, "USE_NULLIFIER"),
            (ivm_abi::syscalls::SYSCALL_COMMIT_OUTPUT, "COMMIT_OUTPUT"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("privacy/output syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn privacy_output_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let _secret = get_private_input(name("not_index"));
}
"#,
                "get_private_input expects (int index)",
            ),
            (
                r#"
fn main() {
  use_nullifier(name("not_nullifier"));
}
"#,
                "use_nullifier expects (int nullifier)",
            ),
            (
                r#"
fn main() {
  commit_output(1);
}
"#,
                "commit_output expects no arguments",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject invalid args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn smart_contract_lifecycle_builtins_emit_lifecycle_syscalls() {
        let src = r#"
fn main() {
  let request = norito_bytes("00");
  deactivate_contract_instance(request);
  remove_smart_contract_bytes(request);
  register_smart_contract_code(request);
  register_smart_contract_bytes(request);
  activate_contract_instance(request);
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile smart-contract lifecycle builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE,
                "DEACTIVATE_CONTRACT_INSTANCE",
            ),
            (
                ivm_abi::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES,
                "REMOVE_SMART_CONTRACT_BYTES",
            ),
            (
                ivm_abi::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_CODE,
                "REGISTER_SMART_CONTRACT_CODE",
            ),
            (
                ivm_abi::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
                "REGISTER_SMART_CONTRACT_BYTES",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE,
                "ACTIVATE_CONTRACT_INSTANCE",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("lifecycle syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn smart_contract_lifecycle_builtins_reject_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  register_smart_contract_code(name("not_request"));
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject lifecycle request type");
        assert!(
            err.message.contains(
                "register_smart_contract_code expects (Blob|bytes) pointer to NoritoBytes lifecycle request"
            ),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn fastpq_batch_apply_builtin_emits_batch_apply_syscall_and_exact_access() {
        let from = sample_account_id();
        let to = sample_account_id_alt();
        let asset_definition: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("asset definition");
        let batch = iroha_data_model::isi::transfer::TransferAssetBatch::new(vec![
            iroha_data_model::isi::transfer::TransferAssetBatchEntry::new(
                from.clone(),
                to.clone(),
                asset_definition.clone(),
                7_u64,
            ),
            iroha_data_model::isi::transfer::TransferAssetBatchEntry::new(
                to.clone(),
                from.clone(),
                asset_definition.clone(),
                3_u64,
            ),
        ]);
        let batch_hex = hex::encode(norito::to_bytes(&batch).expect("batch request"));
        let src = format!(
            r#"
kotoage fn apply_batch() permission(Admin) {{
  let batch = norito_bytes("0x{batch_hex}");
  transfer_v1_batch_apply(batch);
}}
"#
        );
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile transfer_v1_batch_apply builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (
                ivm_abi::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY,
                "TRANSFER_V1_BATCH_APPLY",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("FASTPQ batch apply syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let apply_batch = entrypoints
            .iter()
            .find(|entry| entry.name == "apply_batch")
            .expect("apply_batch entrypoint");
        assert_eq!(apply_batch.access_hints_complete, Some(true));
        assert!(apply_batch.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&apply_batch.read_keys);
        assert_taira_supported_access_keys(&apply_batch.write_keys);
        for account in [&from, &to] {
            let key = super::key_asset(&iroha_data_model::asset::AssetId::of(
                asset_definition.clone(),
                account.clone(),
            ));
            assert!(
                apply_batch.read_keys.iter().any(|actual| actual == &key),
                "missing transfer batch read key {key}; got {:?}",
                apply_batch.read_keys
            );
            assert!(
                apply_batch.write_keys.iter().any(|actual| actual == &key),
                "missing transfer batch write key {key}; got {:?}",
                apply_batch.write_keys
            );
        }
    }

    #[test]
    fn fastpq_batch_apply_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  transfer_v1_batch_apply(name("not_batch"));
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject batch payload type");
        assert!(
            err.message
                .contains("transfer_v1_batch_apply expects (Blob|bytes) Norito TransferAssetBatch"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn fastpq_batch_boundary_builtins_emit_boundary_syscalls_and_complete_access() {
        let src = r#"
kotoage fn batch() permission(Admin) {
  transfer_v1_batch_begin();
  transfer_v1_batch_end();
  call transfer_v1_batch_begin();
  call transfer_v1_batch_end();
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile transfer V1 batch boundary builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN,
                "TRANSFER_V1_BATCH_BEGIN",
            ),
            (
                ivm_abi::syscalls::SYSCALL_TRANSFER_V1_BATCH_END,
                "TRANSFER_V1_BATCH_END",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("FASTPQ batch boundary syscall id fits in u8"),
            )
            .to_le_bytes();
            let count = code
                .windows(needle.len())
                .filter(|window| *window == needle)
                .count();
            assert!(
                count == 2,
                "expected direct and call-sugar {label} syscalls in compiled code, got {count}"
            );
        }

        let publish_tlv = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
        )
        .to_le_bytes();
        assert!(
            !code
                .windows(publish_tlv.len())
                .any(|window| window == publish_tlv),
            "batch boundary helpers must not publish input TLVs"
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let batch = entrypoints
            .iter()
            .find(|entry| entry.name == "batch")
            .expect("batch entrypoint");
        assert_ne!(batch.access_hints_complete, Some(false));
        assert!(batch.access_hints_skipped.is_empty());
        assert!(batch.read_keys.is_empty());
        assert!(batch.write_keys.is_empty());
    }

    #[test]
    fn fastpq_batch_boundary_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  transfer_v1_batch_begin(1);
}
"#,
                "transfer_v1_batch_begin expects ()",
            ),
            (
                r#"
fn main() {
  transfer_v1_batch_end(1);
}
"#,
                "transfer_v1_batch_end expects ()",
            ),
            (
                r#"
fn main() {
  call transfer_v1_batch_begin(1);
}
"#,
                "transfer_v1_batch_begin expects ()",
            ),
            (
                r#"
fn main() {
  call transfer_v1_batch_end(1);
}
"#,
                "transfer_v1_batch_end expects ()",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject boundary args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn transfer_batch_builtin_lowers_entries_between_boundaries() {
        let src = r#"
fn main() {
  let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
  transfer_batch((authority(), authority(), asset, 5));
  call transfer_batch((authority(), authority(), asset, 7));
}
"#;
        let parsed = parse(src).expect("parse transfer_batch source");
        let typed = analyze(&parsed).expect("analyze transfer_batch source");
        let ir = ir::lower(&typed).expect("lower transfer_batch source");

        let mut begins = 0;
        let mut transfers = 0;
        let mut ends = 0;
        for instr in ir
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter())
            .flat_map(|block| block.instrs.iter())
        {
            match instr {
                ir::Instr::TransferBatchBegin => begins += 1,
                ir::Instr::TransferAsset { .. } => transfers += 1,
                ir::Instr::TransferBatchEnd => ends += 1,
                _ => {}
            }
        }

        assert_eq!(begins, 2, "expected direct and call-sugar batch begin");
        assert_eq!(transfers, 2, "expected one transfer per batch entry");
        assert_eq!(ends, 2, "expected direct and call-sugar batch end");
    }

    #[test]
    fn transfer_batch_builtin_rejects_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  transfer_batch();
}
"#,
                "transfer_batch expects at least one entry",
            ),
            (
                r#"
fn main() {
  call transfer_batch(authority());
}
"#,
                "transfer_batch expects (AccountId, AccountId, AssetDefinitionId, int) tuple entries",
            ),
        ] {
            let parsed = parse(src).expect("parse invalid transfer_batch source");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject transfer_batch args");
            assert!(
                err.message.contains(expected),
                "expected error containing {expected:?}, got {}",
                err.message
            );
        }
    }

    #[test]
    fn axt_builtins_emit_axt_syscalls_and_incomplete_access() {
        let src = r#"
kotoage fn run() permission(Admin) {
  let ds = dataspace_id("7");
  let desc = axt_descriptor(norito_bytes("0x00"));
  let handle = asset_handle(norito_bytes("0x01"));
  let proof = proof_blob(norito_bytes("0x02"));
  axt_begin(desc);
  axt_touch(ds, norito_bytes("manifest"));
  axt_touch(ds);
  verify_ds_proof(ds, proof);
  verify_ds_proof(ds);
  use_asset_handle(handle, norito_bytes("intent"), proof);
  use_asset_handle(handle, norito_bytes("intent"));
  axt_commit();
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile AXT builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_AXT_BEGIN, "AXT_BEGIN"),
            (ivm_abi::syscalls::SYSCALL_AXT_TOUCH, "AXT_TOUCH"),
            (
                ivm_abi::syscalls::SYSCALL_VERIFY_DS_PROOF,
                "VERIFY_DS_PROOF",
            ),
            (
                ivm_abi::syscalls::SYSCALL_USE_ASSET_HANDLE,
                "USE_ASSET_HANDLE",
            ),
            (ivm_abi::syscalls::SYSCALL_AXT_COMMIT, "AXT_COMMIT"),
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("AXT syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let run = entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("run entrypoint");
        assert_eq!(run.access_hints_complete, Some(false));
        assert!(!run.access_hints_skipped.is_empty());
    }

    #[test]
    fn axt_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  axt_begin(norito_bytes("00"));
}
"#,
                "axt_begin expects (AxtDescriptor)",
            ),
            (
                r#"
fn main() {
  axt_touch(dataspace_id("7"), 1);
}
"#,
                "axt_touch expects (DataSpaceId[, Blob|bytes manifest])",
            ),
            (
                r#"
fn main() {
  verify_ds_proof(dataspace_id("7"), norito_bytes("00"));
}
"#,
                "verify_ds_proof expects (DataSpaceId[, ProofBlob])",
            ),
            (
                r#"
fn main() {
  use_asset_handle(asset_handle(norito_bytes("00")), 1);
}
"#,
                "use_asset_handle expects (AssetHandle, Blob|bytes intent[, ProofBlob])",
            ),
            (
                r#"
fn main() {
  axt_commit(1);
}
"#,
                "axt_commit expects no arguments",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject AXT args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn verify_proof_builtin_emits_verify_proof_syscall_and_complete_access() {
        let src = r#"
view fn check() -> int {
  let envelope = norito_bytes("00");
  if verify_proof(envelope) {
    return 1;
  }
  return 0;
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile verify_proof builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_VERIFY_PROOF, "VERIFY_PROOF"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("verify_proof syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let check = entrypoints
            .iter()
            .find(|entry| entry.name == "check")
            .expect("check entrypoint");
        assert_ne!(check.access_hints_complete, Some(false));
        assert!(check.access_hints_skipped.is_empty());
        assert!(check.read_keys.is_empty());
        assert!(check.write_keys.is_empty());
    }

    #[test]
    fn verify_proof_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let _ok = verify_proof(name("not_envelope"));
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject proof payload type");
        assert!(
            err.message.contains(
                "verify_proof expects (Blob|bytes) pointer to NoritoBytes OpenVerifyEnvelope"
            ),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn prove_execution_builtin_emits_prove_execution_syscall_and_complete_access() {
        let src = r#"
view fn proof() -> bytes {
  return prove_execution();
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile prove_execution builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_PROVE_EXECUTION)
                .expect("prove_execution syscall id fits in u8"),
        )
        .to_le_bytes();

        assert!(
            code.windows(needle.len()).any(|window| window == needle),
            "expected PROVE_EXECUTION syscall in compiled code"
        );
        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let proof = entrypoints
            .iter()
            .find(|entry| entry.name == "proof")
            .expect("proof entrypoint");
        assert_ne!(proof.access_hints_complete, Some(false));
        assert!(proof.access_hints_skipped.is_empty());
        assert!(proof.read_keys.is_empty());
        assert!(proof.write_keys.is_empty());
    }

    #[test]
    fn prove_execution_builtin_rejects_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let _proof = prove_execution(1);
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject proof arguments");
        assert!(
            err.message.contains("prove_execution expects no arguments"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn grow_heap_builtin_emits_grow_heap_syscall_and_complete_access() {
        let src = r#"
view fn grow() -> int {
  return grow_heap(4096);
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile grow_heap builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_GROW_HEAP)
                .expect("grow_heap syscall id fits in u8"),
        )
        .to_le_bytes();

        assert!(
            code.windows(needle.len()).any(|window| window == needle),
            "expected GROW_HEAP syscall in compiled code"
        );
        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let grow = entrypoints
            .iter()
            .find(|entry| entry.name == "grow")
            .expect("grow entrypoint");
        assert_ne!(grow.access_hints_complete, Some(false));
        assert!(grow.access_hints_skipped.is_empty());
        assert!(grow.read_keys.is_empty());
        assert!(grow.write_keys.is_empty());
    }

    #[test]
    fn grow_heap_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let _limit = grow_heap(name("not_bytes"));
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject byte count type");
        assert!(
            err.message.contains("grow_heap expects (int bytes)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn raw_memory_merkle_builtins_emit_alloc_and_merkle_syscalls() {
        let src = r#"
view fn merkle() -> int {
  let out = alloc(2048);
  let root = alloc(32);
  let path_len = get_merkle_path(out, out, root);
  let compact_len = get_merkle_compact(out, out, 16, root);
  let register_len = get_register_merkle_compact(10, out, 8, root);
  return path_len + compact_len + register_len;
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile raw memory Merkle builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];
        for syscall in [
            ivm_abi::syscalls::SYSCALL_ALLOC,
            ivm_abi::syscalls::SYSCALL_GET_MERKLE_PATH,
            ivm_abi::syscalls::SYSCALL_GET_MERKLE_COMPACT,
            ivm_abi::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT,
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("raw memory syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected syscall 0x{syscall:x} in compiled code"
            );
        }
        let input_publish = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV)
                .expect("input publish syscall id fits in u8"),
        )
        .to_le_bytes();
        assert!(
            !code
                .windows(input_publish.len())
                .any(|window| window == input_publish),
            "raw memory Merkle helpers should not publish pointer TLVs"
        );
        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let merkle = entrypoints
            .iter()
            .find(|entry| entry.name == "merkle")
            .expect("merkle entrypoint");
        assert_ne!(merkle.access_hints_complete, Some(false));
        assert!(merkle.access_hints_skipped.is_empty());
        assert!(merkle.read_keys.is_empty());
        assert!(merkle.write_keys.is_empty());
    }

    #[test]
    fn raw_memory_merkle_builtins_reject_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let _path = get_merkle_path(name("address"), 1);
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject Merkle args");
        assert!(
            err.message.contains("get_merkle_path expects"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn direct_codec_numeric_builtins_emit_direct_syscalls_and_complete_access() {
        let src = r#"
fn direct_helpers() -> int {
  let payload = json!{
    amount: 7,
    count: 3,
    nested: { ok: true },
    label: "ExampleName",
    owner: "alice@wonderland",
    asset: "rose#wonderland",
    nft: "n0$wonderland",
    blob: "0102"
  };
  let amount: Amount = json_get_numeric_direct(payload, name("amount"));
  let sum: Amount = numeric_add_direct(amount, amount);
  let diff: Amount = numeric_sub_direct(sum, amount);
  let product: Amount = numeric_mul_direct(diff, amount);
  let quotient: Amount = numeric_div_direct(product, amount);
  let remainder: Amount = numeric_rem_direct(product, amount);
  let negated: Amount = numeric_neg_direct(remainder);
  let same = numeric_eq_direct(sum, sum);
  let different = numeric_ne_direct(sum, diff);
  let lower = numeric_lt_direct(diff, sum);
  let lower_or_equal = numeric_le_direct(diff, sum);
  let greater = numeric_gt_direct(sum, diff);
  let greater_or_equal = numeric_ge_direct(sum, diff);
  let nested = json_get_json_direct(payload, name("nested"));
  let label = json_get_name_direct(payload, name("label"));
  let owner = json_get_account_id_direct(payload, name("owner"));
  let asset = json_get_asset_definition_id_direct(payload, name("asset"));
  let nft = json_get_nft_id_direct(payload, name("nft"));
  let blob = json_get_blob_hex_direct(payload, name("blob"));
  let with_count = json_set_int_direct(payload, name("count"), json_get_int_direct(payload, name("count")));
  let with_owner = json_set_account_id_direct(with_count, name("owner"), owner);
  let path = build_path_key_norito_direct(label, blob);
  let schema = schema_info_direct(path);
  let encoded = encode_schema_direct(name("example.schema"), with_owner);
  let decoded = decode_schema_direct(name("example.schema"), encoded);
  if same && different && lower && lower_or_equal && greater && greater_or_equal {
    return numeric_to_int_direct(negated);
  }
  return json_get_int_direct(decoded, name("count"));
}

kotoage fn run() permission(Admin) {
  info(direct_helpers());
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile direct helper builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];
        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_I64_DIRECT,
                "JSON_GET_I64_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_JSON_DIRECT,
                "JSON_GET_JSON_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_NAME_DIRECT,
                "JSON_GET_NAME_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT,
                "JSON_GET_ACCOUNT_ID_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_NFT_ID_DIRECT,
                "JSON_GET_NFT_ID_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_BLOB_HEX_DIRECT,
                "JSON_GET_BLOB_HEX_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_NUMERIC_DIRECT,
                "JSON_GET_NUMERIC_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT,
                "JSON_GET_ASSET_DEFINITION_ID_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_SET_I64_DIRECT,
                "JSON_SET_I64_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT,
                "JSON_SET_ACCOUNT_ID_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT,
                "BUILD_PATH_KEY_NORITO_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SCHEMA_INFO_DIRECT,
                "SCHEMA_INFO_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SCHEMA_ENCODE_DIRECT,
                "SCHEMA_ENCODE_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SCHEMA_DECODE_DIRECT,
                "SCHEMA_DECODE_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_TO_INT_DIRECT,
                "NUMERIC_TO_INT_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_ADD_DIRECT,
                "NUMERIC_ADD_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_SUB_DIRECT,
                "NUMERIC_SUB_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_MUL_DIRECT,
                "NUMERIC_MUL_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_DIV_DIRECT,
                "NUMERIC_DIV_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_REM_DIRECT,
                "NUMERIC_REM_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_NEG_DIRECT,
                "NUMERIC_NEG_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_EQ_DIRECT,
                "NUMERIC_EQ_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_NE_DIRECT,
                "NUMERIC_NE_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_LT_DIRECT,
                "NUMERIC_LT_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_LE_DIRECT,
                "NUMERIC_LE_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_GT_DIRECT,
                "NUMERIC_GT_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_GE_DIRECT,
                "NUMERIC_GE_DIRECT",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("direct helper syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
        let input_publish = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV)
                .expect("input publish syscall id fits in u8"),
        )
        .to_le_bytes();
        assert!(
            !code
                .windows(input_publish.len())
                .any(|window| window == input_publish),
            "direct helper builtins should not publish pointer TLVs"
        );
        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let run = entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("run entrypoint");
        assert_ne!(run.access_hints_complete, Some(false));
        assert!(run.access_hints_skipped.is_empty());
        assert!(run.read_keys.is_empty());
        assert!(run.write_keys.is_empty());
    }

    #[test]
    fn direct_codec_numeric_builtins_reject_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let bad = numeric_add_direct(1, 1);
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject raw int direct numerics");
        assert!(
            err.message.contains("numeric_add_direct expects"),
            "unexpected semantic error: {}",
            err.message
        );

        let parsed = parse(
            r#"
fn main() {
  let bad = json_get_int_direct(name("payload"), name("count"));
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject direct JSON arg types");
        assert!(
            err.message
                .contains("json_get_int_direct expects (Json, Name)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn schema_builtins_emit_regular_schema_syscalls() {
        let src = r#"
fn schema_helpers() {
  let schema = name("example.schema");
  let encoded = encode_schema(schema, json!{ count: 3 });
  let _decoded = decode_schema(schema, encoded);
  let _info = schema_info(schema);
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile regular schema helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_SCHEMA_ENCODE, "SCHEMA_ENCODE"),
            (ivm_abi::syscalls::SYSCALL_SCHEMA_DECODE, "SCHEMA_DECODE"),
            (ivm_abi::syscalls::SYSCALL_SCHEMA_INFO, "SCHEMA_INFO"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("schema syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn schema_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let _bad = encode_schema(name("example.schema"), 1);
}
"#,
                "encode_schema expects (Name, Json)",
            ),
            (
                r#"
fn main() {
  let _bad = decode_schema(1, norito_bytes("00"));
}
"#,
                "decode_schema expects (Name, Blob|bytes)",
            ),
            (
                r#"
fn main() {
  let _bad = schema_info(1);
}
"#,
                "schema_info expects (Name)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject schema args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn vrf_builtins_emit_vrf_syscalls() {
        let src = r#"
fn verify(payload: Blob) {
  let _proof = vrf_verify(payload, payload, payload, 1);
  let _batch = vrf_verify_batch(payload);
}

fn main() {
  verify(blob("0x010203"));
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler.compile_source(src).expect("compile VRF helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_VRF_VERIFY, "VRF_VERIFY"),
            (
                ivm_abi::syscalls::SYSCALL_VRF_VERIFY_BATCH,
                "VRF_VERIFY_BATCH",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("VRF syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn vrf_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let payload = blob("0x010203");
  let _bad = vrf_verify(payload, payload, payload, name("variant"));
}
"#,
                "vrf_verify expects (Blob|bytes, Blob|bytes, Blob|bytes, int variant)",
            ),
            (
                r#"
fn main() {
  let _bad = vrf_verify_batch(1);
}
"#,
                "vrf_verify_batch expects (Blob|bytes)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject VRF args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn zk_verify_builtins_emit_verify_syscalls() {
        let src = r#"
fn verify(payload: Blob) {
  zk_verify_transfer(payload);
  zk_verify_unshield(payload);
  zk_verify_batch(payload);
  zk_vote_verify_ballot(payload);
  zk_vote_verify_tally(payload);
}

fn verify_namespaced(payload: Blob) {
  zk::verify_transfer(payload);
  zk::verify_unshield(payload);
  zk::verify_batch(payload);
  zk::vote::verify_ballot(payload);
  zk::vote::verify_tally(payload);
}

fn main() {
  let payload = blob("0x010203");
  verify(payload);
  verify_namespaced(payload);
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile ZK verify helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
                "ZK_VERIFY_TRANSFER",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
                "ZK_VERIFY_UNSHIELD",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ZK_VERIFY_BATCH,
                "ZK_VERIFY_BATCH",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
                "ZK_VOTE_VERIFY_BALLOT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
                "ZK_VOTE_VERIFY_TALLY",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("ZK syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn zk_verify_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  zk_verify_transfer(1);
}
"#,
                "zk_verify_transfer expects (Blob|bytes) where the argument is a pointer to NoritoBytes TLV in INPUT",
            ),
            (
                r#"
fn main() {
  zk::vote::verify_tally(1);
}
"#,
                "zk_vote_verify_tally expects (Blob|bytes) where the argument is a pointer to NoritoBytes TLV in INPUT",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject ZK verify args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn inline_zk_builder_builtins_lower_to_ir() {
        let account = sample_account_literal();
        let input = "00".repeat(32);
        let outputs = format!("{}{}", "11".repeat(32), "22".repeat(32));
        let src = format!(
            r#"
fn main() {{
  let _ballot = build_submit_ballot_inline(
    "election",
    blob("00"),
    blob("0000000000000000000000000000000000000000000000000000000000000000"),
    "halo2",
    blob("proof"),
    blob("vk")
  );
  let _unshield = build_unshield_inline(
    asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
    account_id("{account}"),
    5,
    blob("{input}"),
    "halo2",
    blob("proof"),
    blob("vk")
  );
  let _unshield_with_outputs = build_unshield_inline(
    asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
    account_id("{account}"),
    6,
    blob("{input}"),
    blob("{outputs}"),
    "halo2",
    blob("proof"),
    blob("vk")
  );
}}
"#
        );
        let parsed = parse(&src).expect("parse inline builder source");
        let typed = analyze(&parsed).expect("analyze inline builder source");
        let ir = ir::lower(&typed).expect("lower inline builder source");

        let mut saw_submit = false;
        let mut saw_unshield_without_outputs = false;
        let mut saw_unshield_with_outputs = false;
        for instr in ir
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter())
            .flat_map(|block| block.instrs.iter())
        {
            match instr {
                ir::Instr::BuildSubmitBallotInline { .. } => saw_submit = true,
                ir::Instr::BuildUnshieldInline { outputs, .. } => {
                    if outputs.is_some() {
                        saw_unshield_with_outputs = true;
                    } else {
                        saw_unshield_without_outputs = true;
                    }
                }
                _ => {}
            }
        }

        assert!(saw_submit, "expected BuildSubmitBallotInline IR");
        assert!(
            saw_unshield_without_outputs,
            "expected legacy BuildUnshieldInline IR"
        );
        assert!(
            saw_unshield_with_outputs,
            "expected BuildUnshieldInline IR with private change outputs"
        );
    }

    #[test]
    fn unshield_inline_literal_encodes_input_and_output_chunks() {
        let asset = ir::Temp(0);
        let to = ir::Temp(1);
        let amount = ir::Temp(2);
        let inputs = ir::Temp(3);
        let outputs = ir::Temp(4);
        let backend = ir::Temp(5);
        let proof = ir::Temp(6);
        let vk = ir::Temp(7);
        let func_idx = 0;
        let mut string_map = HashMap::new();
        string_map.insert(
            (func_idx, asset),
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM".to_string(),
        );
        string_map.insert((func_idx, to), sample_account_literal());
        string_map.insert(
            (func_idx, inputs),
            format!("0x{}{}", "11".repeat(32), "12".repeat(32)),
        );
        string_map.insert(
            (func_idx, outputs),
            format!("0x{}{}", "21".repeat(32), "22".repeat(32)),
        );
        string_map.insert((func_idx, backend), "halo2/ipa".to_string());
        string_map.insert((func_idx, proof), "0xab".to_string());
        string_map.insert((func_idx, vk), "vk_unshield_outputs".to_string());
        let mut int_const_map = HashMap::new();
        int_const_map.insert((func_idx, amount), 7);

        let raw = super::unshield_inline_instruction_literal(
            &string_map,
            &int_const_map,
            func_idx,
            asset,
            to,
            amount,
            inputs,
            Some(outputs),
            backend,
            proof,
            vk,
        )
        .expect("fold unshield inline literal");
        let payload = super::decode_norito_literal_payload(&raw).expect("literal payload");
        let boxed: iroha_data_model::isi::InstructionBox =
            norito::decode_from_bytes(&payload).expect("decode InstructionBox");
        let unshield = boxed
            .as_any()
            .downcast_ref::<iroha_data_model::isi::zk::Unshield>()
            .expect("Unshield instruction");
        assert_eq!(unshield.public_amount(), &7u128);
        assert_eq!(unshield.inputs().as_slice(), &[[0x11u8; 32], [0x12u8; 32]]);
        assert_eq!(unshield.outputs().as_slice(), &[[0x21u8; 32], [0x22u8; 32]]);
    }

    #[test]
    fn inline_zk_builder_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let _bytes = build_submit_ballot_inline("election", 1, blob("00"), "halo2", blob("proof"), blob("vk"));
}
"#,
                "build_submit_ballot_inline expects (string election_id, Blob|bytes ciphertext, Blob|bytes nullifier32, string backend, Blob|bytes proof, Blob|bytes vk)",
            ),
            (
                r#"
fn main() {
  let _bytes = build_unshield_inline(name("asset"), authority(), 1, blob("00"), "halo2", blob("proof"), blob("vk"));
}
"#,
                "build_unshield_inline expects (AssetDefinitionId, AccountId, int amount, Blob|bytes inputs32, [Blob|bytes outputs32,] string backend, Blob|bytes proof, Blob|bytes vk)",
            ),
            (
                r#"
fn main() {
  let _bytes = build_unshield_inline(asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), authority(), 1, blob("00"), "halo2", 1, blob("vk"));
}
"#,
                "build_unshield_inline expects (AssetDefinitionId, AccountId, int amount, Blob|bytes inputs32, [Blob|bytes outputs32,] string backend, Blob|bytes proof, Blob|bytes vk)",
            ),
        ] {
            let parsed = parse(src).expect("parse invalid inline builder source");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject inline builder args");
            assert!(
                err.message.contains(expected),
                "expected error containing {expected:?}, got {}",
                err.message
            );
        }
    }

    #[test]
    fn vendor_bridge_and_subscription_builtins_emit_syscalls() {
        let src = r#"
kotoage fn run() permission(Admin) {
  let payload = norito_bytes("0x0102");
  execute_instruction(payload);
  sc_execute_submit_ballot(payload);
  sc_execute_unshield(payload);
  let result = execute_query(payload);
  info(tlv_len(result));
  subscription_bill();
  subscription_record_usage();
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile vendor bridge and subscription helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        let syscall_needle = |syscall: u32| {
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("test syscall id fits in u8"),
            )
            .to_le_bytes()
        };

        let execute_instruction =
            syscall_needle(ivm_abi::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION);
        let execute_instruction_count = code
            .windows(execute_instruction.len())
            .filter(|window| *window == execute_instruction)
            .count();
        assert_eq!(
            execute_instruction_count, 3,
            "execute_instruction and sc_execute aliases should lower to SMARTCONTRACT_EXECUTE_INSTRUCTION"
        );

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY,
                "SMARTCONTRACT_EXECUTE_QUERY",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SUBSCRIPTION_BILL,
                "SUBSCRIPTION_BILL",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE,
                "SUBSCRIPTION_RECORD_USAGE",
            ),
        ] {
            let needle = syscall_needle(syscall);
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }
    }

    #[test]
    fn vendor_bridge_and_subscription_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  execute_instruction(1);
}
"#,
                "execute_instruction expects (Blob|bytes) where the argument is a pointer to NoritoBytes TLV in INPUT",
            ),
            (
                r#"
fn main() {
  let _bad = execute_query(1);
}
"#,
                "execute_query expects (Blob|bytes) where the argument is a pointer to NoritoBytes TLV in INPUT",
            ),
            (
                r#"
fn main() {
  subscription_record_usage(1);
}
"#,
                "subscription_record_usage expects no arguments",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed)
                .expect_err("semantic analysis should reject bridge/subscription args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn call_contract_builtin_emits_call_contract_syscall() {
        let src = r#"
seiyaku Relay {
  kotoage fn run(target: bytes, payload: Json) -> bytes permission(Admin) {
    return call_contract(target, "settle", payload);
  }
}
"#;
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(src)
            .expect("compile call_contract helper");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_CALL_CONTRACT as u8,
        )
        .to_le_bytes();
        assert!(
            code.windows(needle.len()).any(|window| window == needle),
            "expected CALL_CONTRACT syscall in compiled code"
        );
    }

    #[test]
    fn call_contract_builtin_rejects_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let _bad = call_contract(json_object(), "settle", json!{ amount: 1 });
}
"#,
                "call_contract expects (String|Blob, String|Blob, Json)",
            ),
            (
                r#"
fn main() {
  let _bad = call_contract("target", "settle", name("not_json"));
}
"#,
                "call_contract expects (String|Blob, String|Blob, Json)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject call_contract");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn hash_builtins_emit_hash_syscalls_and_complete_access() {
        let src = r#"
view fn digest() -> int {
  let payload = blob("0x010203");
  let sm3 = sm3_hash(payload);
  let sm3_namespaced = sm::hash(payload);
  let sm3_explicit_namespaced = sm::sm3_hash(payload);
  let sha256 = sha256_hash(payload);
  let sha3 = sha3_hash(payload);
  let blake2b = blake2b256_hash(payload);
  let keccak = keccak256_hash(payload);
  let iroha = iroha_hash(payload);
  let _a = tlv_len(sm3);
  let _b = tlv_len(sm3_namespaced);
  let _c = tlv_len(sm3_explicit_namespaced);
  let _d = tlv_len(sha256);
  let _e = tlv_len(sha3);
  let _f = tlv_len(blake2b);
  let _g = tlv_len(keccak);
  return tlv_len(iroha);
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile hash helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_SM3_HASH, "SM3_HASH"),
            (ivm_abi::syscalls::SYSCALL_SHA256_HASH, "SHA256_HASH"),
            (ivm_abi::syscalls::SYSCALL_SHA3_HASH, "SHA3_HASH"),
            (
                ivm_abi::syscalls::SYSCALL_BLAKE2B256_HASH,
                "BLAKE2B256_HASH",
            ),
            (ivm_abi::syscalls::SYSCALL_KECCAK256_HASH, "KECCAK256_HASH"),
            (ivm_abi::syscalls::SYSCALL_IROHA_HASH, "IROHA_HASH"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("hash syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let digest = entrypoints
            .iter()
            .find(|entry| entry.name == "digest")
            .expect("digest entrypoint");
        assert_ne!(digest.access_hints_complete, Some(false));
        assert!(digest.access_hints_skipped.is_empty());
        assert!(digest.read_keys.is_empty());
        assert!(digest.write_keys.is_empty());
    }

    #[test]
    fn hash_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let _bad = sha256_hash(1);
}
"#,
                "sha256_hash expects (Blob|bytes) argument pointing to INPUT TLV",
            ),
            (
                r#"
fn main() {
  let _bad = sm::hash(1);
}
"#,
                "sm3_hash expects (Blob|bytes) argument pointing to INPUT TLV",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject hash args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn crypto_builtins_emit_signature_and_sm4_syscalls_and_complete_access() {
        let src = r#"
view fn crypt() -> int {
  let payload = blob("0x010203");
  let sm2 = sm2_verify(payload, payload, payload);
  let sm2_distid = sm::verify_with_distid(payload, payload, payload, payload);
  let generic = verify_signature(payload, payload, payload, 0);
  let gcm = sm::gcm_seal(payload, payload, payload, payload);
  let opened_gcm = sm::open_gcm(payload, payload, payload, gcm);
  let ccm = sm::seal_ccm(payload, payload, payload, payload, 12);
  let opened_ccm = sm::open_ccm(payload, payload, payload, ccm);
  if !sm2 {
    return 0;
  }
  if !sm2_distid {
    return 0;
  }
  if !generic {
    return 0;
  }
  return tlv_len(opened_gcm) + tlv_len(opened_ccm);
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile crypto helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_SM2_VERIFY, "SM2_VERIFY"),
            (
                ivm_abi::syscalls::SYSCALL_VERIFY_SIGNATURE,
                "VERIFY_SIGNATURE",
            ),
            (ivm_abi::syscalls::SYSCALL_SM4_GCM_SEAL, "SM4_GCM_SEAL"),
            (ivm_abi::syscalls::SYSCALL_SM4_GCM_OPEN, "SM4_GCM_OPEN"),
            (ivm_abi::syscalls::SYSCALL_SM4_CCM_SEAL, "SM4_CCM_SEAL"),
            (ivm_abi::syscalls::SYSCALL_SM4_CCM_OPEN, "SM4_CCM_OPEN"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("crypto syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let crypt = entrypoints
            .iter()
            .find(|entry| entry.name == "crypt")
            .expect("crypt entrypoint");
        assert_ne!(crypt.access_hints_complete, Some(false));
        assert!(crypt.access_hints_skipped.is_empty());
        assert!(crypt.read_keys.is_empty());
        assert!(crypt.write_keys.is_empty());
    }

    #[test]
    fn crypto_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let payload = blob("0x010203");
  let _bad = sm2_verify(payload, payload);
}
"#,
                "sm2_verify expects (Blob, Blob, Blob) or (Blob, Blob, Blob, Blob) where arguments reference INPUT TLVs",
            ),
            (
                r#"
fn main() {
  let payload = blob("0x010203");
  let _bad = sm2_verify(payload, payload, payload, 1);
}
"#,
                "sm2_verify optional distid must be provided as Blob|bytes pointer",
            ),
            (
                r#"
fn main() {
  let payload = blob("0x010203");
  let _bad = verify_signature(payload, payload, payload, name("scheme"));
}
"#,
                "verify_signature expects scheme code as int",
            ),
            (
                r#"
fn main() {
  let payload = blob("0x010203");
  let _bad = sm4_gcm_seal(payload, payload, payload, 1);
}
"#,
                "sm4_gcm_seal expects (Blob|bytes, Blob|bytes, Blob|bytes, Blob|bytes)",
            ),
            (
                r#"
fn main() {
  let payload = blob("0x010203");
  let _bad = sm4_ccm_seal(payload, payload, payload, payload, name("tag"));
}
"#,
                "sm4_ccm_seal optional tag length must be int",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject crypto args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn codec_builtins_emit_encode_decode_syscalls_and_complete_access() {
        let src = r#"
view fn roundtrip() -> int {
  let int_bytes = encode_int(7);
  let decoded = decode_int(int_bytes);
  let json_bytes = encode_json(json!{ value: 7 });
  let decoded_json = decode_json(json_bytes);
  let encoded_again = encode_json(decoded_json);
  return decoded + tlv_len(encoded_again);
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile codec helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_ENCODE_INT, "ENCODE_INT"),
            (ivm_abi::syscalls::SYSCALL_DECODE_INT, "DECODE_INT"),
            (ivm_abi::syscalls::SYSCALL_JSON_ENCODE, "JSON_ENCODE"),
            (ivm_abi::syscalls::SYSCALL_JSON_DECODE, "JSON_DECODE"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("codec syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let roundtrip = entrypoints
            .iter()
            .find(|entry| entry.name == "roundtrip")
            .expect("roundtrip entrypoint");
        assert_ne!(roundtrip.access_hints_complete, Some(false));
        assert!(roundtrip.access_hints_skipped.is_empty());
        assert!(roundtrip.read_keys.is_empty());
        assert!(roundtrip.write_keys.is_empty());
    }

    #[test]
    fn codec_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let _bad = encode_int(name("not_int"));
}
"#,
                "encode_int expects (int)",
            ),
            (
                r#"
fn main() {
  let _bad = decode_int(1);
}
"#,
                "decode_int expects (Blob|bytes)",
            ),
            (
                r#"
fn main() {
  let _bad = encode_json(name("not_json"));
}
"#,
                "encode_json expects (Json)",
            ),
            (
                r#"
fn main() {
  let _bad = decode_json(1);
}
"#,
                "decode_json expects (Blob|bytes)",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err = analyze(&parsed).expect_err("semantic analysis should reject codec args");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn math_vector_scalar_builtins_emit_opcodes_and_complete_access() {
        let src = r#"
view fn scalar_math() -> int {
  setvl(8);
  let a = abs(-5);
  let b = min(1, 2);
  let c = max(3, 4);
  let d = div_ceil(5, 2);
  let e = gcd(21, 6);
  let f = mean(2, 8);
  let g = isqrt(16);
  let h = poseidon2(1, 2);
  let i = pubkgen(3);
  let j = valcom(4, 5);
  return a + b + c + d + e + f + g + h + i + j;
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile math/vector/scalar helpers");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let opcodes: Vec<_> = bytes[parsed.code_offset..]
            .chunks_exact(4)
            .map(|chunk| {
                let word = u32::from_le_bytes(<[u8; 4]>::try_from(chunk).unwrap());
                instruction::wide::opcode(word)
            })
            .collect();

        for (opcode, label) in [
            (instruction::wide::arithmetic::ABS, "ABS"),
            (instruction::wide::arithmetic::MIN, "MIN"),
            (instruction::wide::arithmetic::MAX, "MAX"),
            (instruction::wide::arithmetic::DIV_CEIL, "DIV_CEIL"),
            (instruction::wide::arithmetic::GCD, "GCD"),
            (instruction::wide::arithmetic::MEAN, "MEAN"),
            (instruction::wide::arithmetic::ISQRT, "ISQRT"),
            (instruction::wide::crypto::POSEIDON2, "POSEIDON2"),
            (instruction::wide::crypto::PUBKGEN, "PUBKGEN"),
            (instruction::wide::crypto::VALCOM, "VALCOM"),
            (instruction::wide::crypto::SETVL, "SETVL"),
        ] {
            assert!(
                opcodes.contains(&opcode),
                "expected {label} opcode in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let scalar_math = entrypoints
            .iter()
            .find(|entry| entry.name == "scalar_math")
            .expect("scalar_math entrypoint");
        assert_ne!(scalar_math.access_hints_complete, Some(false));
        assert!(scalar_math.access_hints_skipped.is_empty());
        assert!(scalar_math.read_keys.is_empty());
        assert!(scalar_math.write_keys.is_empty());
    }

    #[test]
    fn math_vector_scalar_builtins_reject_invalid_arguments() {
        for (src, expected) in [
            (
                r#"
fn main() {
  let _bad = isqrt(name("not_int"));
}
"#,
                "isqrt expects (int)",
            ),
            (
                r#"
fn main() {
  let _bad = min(1, name("not_int"));
}
"#,
                "min expects (int, int)",
            ),
            (
                r#"
fn main() {
  let _bad = poseidon2(1, name("not_int"));
}
"#,
                "poseidon2 expects two int args",
            ),
            (
                r#"
fn main() {
  let _bad = poseidon6(1, 2, 3, 4, 5, name("not_int"));
}
"#,
                "poseidon6 expects six int args",
            ),
            (
                r#"
fn main() {
  let _bad = pubkgen(name("not_int"));
}
"#,
                "pubkgen expects one int arg",
            ),
            (
                r#"
fn main() {
  let _bad = valcom(1, name("not_int"));
}
"#,
                "valcom expects two int args",
            ),
            (
                r#"
fn main() {
  setvl(name("not_int"));
}
"#,
                "setvl expects one int arg",
            ),
        ] {
            let parsed = parse(src).expect("parse source");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject helper arguments");
            assert!(
                err.message.contains(expected),
                "expected `{expected}`, got `{}`",
                err.message
            );
        }
    }

    #[test]
    fn numeric_neg_builtin_emits_regular_numeric_neg_syscall_and_complete_access() {
        let src = r#"
view fn negate() -> Amount {
  let value: Amount = 7;
  return numeric_neg(value);
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile numeric_neg builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_NUMERIC_NEG, "NUMERIC_NEG"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("numeric_neg syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let direct_neg = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_NUMERIC_NEG_DIRECT)
                .expect("direct numeric neg syscall id fits in u8"),
        )
        .to_le_bytes();
        assert!(
            !code
                .windows(direct_neg.len())
                .any(|window| window == direct_neg),
            "numeric_neg must use the regular published-TLV syscall"
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let negate = entrypoints
            .iter()
            .find(|entry| entry.name == "negate")
            .expect("negate entrypoint");
        assert_ne!(negate.access_hints_complete, Some(false));
        assert!(negate.access_hints_skipped.is_empty());
        assert!(negate.read_keys.is_empty());
        assert!(negate.write_keys.is_empty());
    }

    #[test]
    fn numeric_neg_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let bad = numeric_neg(1);
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject raw int numeric_neg");
        assert!(
            err.message
                .contains("numeric_neg expects (Amount|Balance|fixed_u128)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn numeric_to_int_builtin_emits_regular_numeric_to_int_syscall_and_complete_access() {
        let src = r#"
view fn convert() -> int {
  let value: Amount = 7;
  return numeric_to_int(value);
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile numeric_to_int builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_NUMERIC_TO_INT, "NUMERIC_TO_INT"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("numeric_to_int syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let direct_to_int = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_NUMERIC_TO_INT_DIRECT)
                .expect("direct numeric_to_int syscall id fits in u8"),
        )
        .to_le_bytes();
        assert!(
            !code
                .windows(direct_to_int.len())
                .any(|window| window == direct_to_int),
            "numeric_to_int must use the regular published-TLV syscall"
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let convert = entrypoints
            .iter()
            .find(|entry| entry.name == "convert")
            .expect("convert entrypoint");
        assert_ne!(convert.access_hints_complete, Some(false));
        assert!(convert.access_hints_skipped.is_empty());
        assert!(convert.read_keys.is_empty());
        assert!(convert.write_keys.is_empty());
    }

    #[test]
    fn numeric_to_int_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let bad = numeric_to_int(1);
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject raw int numeric_to_int");
        assert!(
            err.message
                .contains("numeric_to_int expects (Amount|Balance|fixed_u128)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn numeric_binary_builtins_emit_regular_numeric_syscalls_and_complete_access() {
        let src = r#"
view fn compute() -> Amount {
  let left: Amount = 7;
  let right: Amount = 3;
  return numeric_add(left, numeric_rem(left, right));
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile numeric binary builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_NUMERIC_ADD, "NUMERIC_ADD"),
            (ivm_abi::syscalls::SYSCALL_NUMERIC_REM, "NUMERIC_REM"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("numeric binary syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_ADD_DIRECT,
                "NUMERIC_ADD_DIRECT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_NUMERIC_REM_DIRECT,
                "NUMERIC_REM_DIRECT",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("direct numeric binary syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                !code.windows(needle.len()).any(|window| window == needle),
                "regular numeric helpers must not emit {label}"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let compute = entrypoints
            .iter()
            .find(|entry| entry.name == "compute")
            .expect("compute entrypoint");
        assert_ne!(compute.access_hints_complete, Some(false));
        assert!(compute.access_hints_skipped.is_empty());
        assert!(compute.read_keys.is_empty());
        assert!(compute.write_keys.is_empty());
    }

    #[test]
    fn numeric_compare_builtins_emit_regular_numeric_syscalls_and_complete_access() {
        let src = r#"
view fn compare() -> int {
  let left: Amount = 7;
  let right: Amount = 3;
  if numeric_ge(left, right) {
    return 1;
  }
  return 0;
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile numeric comparison builtins");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_NUMERIC_GE, "NUMERIC_GE"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("numeric comparison syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let direct_ge = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_abi::syscalls::SYSCALL_NUMERIC_GE_DIRECT)
                .expect("direct numeric_ge syscall id fits in u8"),
        )
        .to_le_bytes();
        assert!(
            !code
                .windows(direct_ge.len())
                .any(|window| window == direct_ge),
            "regular numeric comparison helper must not emit NUMERIC_GE_DIRECT"
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let compare = entrypoints
            .iter()
            .find(|entry| entry.name == "compare")
            .expect("compare entrypoint");
        assert_ne!(compare.access_hints_complete, Some(false));
        assert!(compare.access_hints_skipped.is_empty());
        assert!(compare.read_keys.is_empty());
        assert!(compare.write_keys.is_empty());
    }

    #[test]
    fn numeric_binary_builtins_reject_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let value: Amount = 7;
  let bad = numeric_add(1, value);
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject raw int numeric_add");
        assert!(
            err.message.contains(
                "numeric_add expects (Amount|Balance|fixed_u128, Amount|Balance|fixed_u128)"
            ),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn name_decode_builtin_emits_name_decode_syscall_and_complete_access() {
        let src = r#"
view fn decode() -> Name {
  return name_decode(norito_bytes("70726f6265"));
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile name_decode builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_NAME_DECODE, "NAME_DECODE"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("name_decode syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let decode = entrypoints
            .iter()
            .find(|entry| entry.name == "decode")
            .expect("decode entrypoint");
        assert_ne!(decode.access_hints_complete, Some(false));
        assert!(decode.access_hints_skipped.is_empty());
        assert!(decode.read_keys.is_empty());
        assert!(decode.write_keys.is_empty());
    }

    #[test]
    fn name_decode_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let bad = name_decode(1);
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject raw int name_decode");
        assert!(
            err.message.contains("name_decode expects (Blob|bytes)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn tlv_eq_builtin_emits_tlv_eq_syscall_and_complete_access() {
        let src = r#"
view fn compare() -> int {
  let left = name("probe");
  let right = name_decode(norito_bytes("70726f6265"));
  if tlv_eq(left, right) {
    return 1;
  }
  return 0;
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile tlv_eq builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_TLV_EQ, "TLV_EQ"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("tlv_eq syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let compare = entrypoints
            .iter()
            .find(|entry| entry.name == "compare")
            .expect("compare entrypoint");
        assert_ne!(compare.access_hints_complete, Some(false));
        assert!(compare.access_hints_skipped.is_empty());
        assert!(compare.read_keys.is_empty());
        assert!(compare.write_keys.is_empty());
    }

    #[test]
    fn tlv_eq_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let bad = tlv_eq(1, name("probe"));
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject raw int tlv_eq");
        assert!(
            err.message
                .contains("tlv_eq expects (pointer-ABI, pointer-ABI)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn tlv_len_builtin_emits_tlv_len_syscall_and_complete_access() {
        let src = r#"
view fn length() -> int {
  return tlv_len(name("probe"));
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile tlv_len builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (ivm_abi::syscalls::SYSCALL_TLV_LEN, "TLV_LEN"),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("tlv_len syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let length = entrypoints
            .iter()
            .find(|entry| entry.name == "length")
            .expect("length entrypoint");
        assert_ne!(length.access_hints_complete, Some(false));
        assert!(length.access_hints_skipped.is_empty());
        assert!(length.read_keys.is_empty());
        assert!(length.write_keys.is_empty());
    }

    #[test]
    fn tlv_len_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let bad = tlv_len(1);
}
"#,
        )
        .expect("parse source");
        let err = analyze(&parsed).expect_err("semantic analysis should reject raw int tlv_len");
        assert!(
            err.message
                .contains("tlv_len expects a pointer-ABI type, Json, or Blob|bytes argument"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn pointer_to_norito_builtin_emits_pointer_to_norito_syscall_and_complete_access() {
        let src = r#"
view fn encode() -> bytes {
  return pointer_to_norito(name("probe"));
}
"#;
        let compiler = test_mode_compiler();
        let (bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile pointer_to_norito builtin");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                "INPUT_PUBLISH_TLV",
            ),
            (
                ivm_abi::syscalls::SYSCALL_POINTER_TO_NORITO,
                "POINTER_TO_NORITO",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("pointer_to_norito syscall id fits in u8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall in compiled code"
            );
        }

        let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
        let encode = entrypoints
            .iter()
            .find(|entry| entry.name == "encode")
            .expect("encode entrypoint");
        assert_ne!(encode.access_hints_complete, Some(false));
        assert!(encode.access_hints_skipped.is_empty());
        assert!(encode.read_keys.is_empty());
        assert!(encode.write_keys.is_empty());
    }

    #[test]
    fn pointer_to_norito_builtin_rejects_invalid_arguments() {
        let parsed = parse(
            r#"
fn main() {
  let bad = pointer_to_norito(json!{ ok: true });
}
"#,
        )
        .expect("parse source");
        let err =
            analyze(&parsed).expect_err("semantic analysis should reject JSON pointer_to_norito");
        assert!(
            err.message
                .contains("pointer_to_norito expects a pointer-ABI type or Blob|bytes argument"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn account_id_alias_literal_emits_resolve_account_alias_syscall() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(r#"fn main() { let _acct = account_id("merchant@paynet"); }"#)
            .expect("compile alias shorthand");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected RESOLVE_ACCOUNT_ALIAS syscall for alias shorthand"
        );
    }

    #[test]
    fn account_id_domain_qualified_alias_literal_emits_resolve_account_alias_syscall() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(r#"fn main() { let _acct = account_id("merchant@bank.paynet"); }"#)
            .expect("compile domain-qualified alias shorthand");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected RESOLVE_ACCOUNT_ALIAS syscall for domain-qualified alias shorthand"
        );
    }

    #[test]
    fn resolve_account_alias_builtin_emits_syscall() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(
                r#"fn main() { let _acct = resolve_account_alias("merchant@paynet"); }"#,
            )
            .expect("compile builtin alias resolution");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected RESOLVE_ACCOUNT_ALIAS syscall for builtin alias resolution"
        );
    }

    #[test]
    fn resolve_account_alias_builtin_rejects_invalid_arguments() {
        let parsed = parse(r#"fn main() { let _acct = resolve_account_alias(json_object()); }"#)
            .expect("parse invalid alias resolution");
        let err = analyze(&parsed).expect_err("semantic analysis should reject alias arg type");
        assert!(
            err.message
                .contains("resolve_account_alias expects (String|Blob)"),
            "unexpected semantic error: {}",
            err.message
        );
    }

    #[test]
    fn runtime_sysvar_builtins_emit_syscalls() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(
                r#"
fn main() {
  let _authority = authority();
  let _sysvar_authority = sysvar_authority();
  let _now = current_time_ms();
  let _height = block_height();
  let _block_time = block_time_ms();
  let _chain = chain_id();
  let _contract = contract_address();
  let _entry = entrypoint();
}
"#,
            )
            .expect("compile runtime sysvars");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (ivm_abi::syscalls::SYSCALL_GET_AUTHORITY, "GET_AUTHORITY"),
            (
                ivm_abi::syscalls::SYSCALL_SYSVAR_AUTHORITY,
                "SYSVAR_AUTHORITY",
            ),
            (
                ivm_abi::syscalls::SYSCALL_CURRENT_TIME_MS,
                "CURRENT_TIME_MS",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT,
                "SYSVAR_BLOCK_HEIGHT",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS,
                "SYSVAR_BLOCK_TIME_MS",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SYSVAR_CHAIN_ID,
                "SYSVAR_CHAIN_ID",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS,
                "SYSVAR_CONTRACT_ADDRESS",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SYSVAR_ENTRYPOINT,
                "SYSVAR_ENTRYPOINT",
            ),
        ] {
            let word = if let Ok(imm8) = u8::try_from(syscall) {
                encoding::wide::encode_sys(instruction::wide::system::SCALL, imm8)
            } else {
                encoding::wide::encode_syscallx(syscall)
            };
            let needle = word.to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall"
            );
        }
    }

    #[test]
    fn runtime_sysvar_builtins_reject_invalid_arguments() {
        for (source, expected) in [
            (
                r#"fn main() { let _authority = authority(1); }"#,
                "authority expects no arguments",
            ),
            (
                r#"fn main() { let _height = block_height(1); }"#,
                "block_height expects no arguments",
            ),
            (
                r#"fn main() { let _chain = chain_id(1); }"#,
                "chain_id expects no arguments",
            ),
            (
                r#"fn main() { let _caller = sysvar_authority(1); }"#,
                "sysvar_authority expects no arguments",
            ),
        ] {
            let parsed = parse(source).expect("parse invalid runtime sysvar call");
            let err = analyze(&parsed).expect_err("semantic analysis should reject sysvar arity");
            assert!(
                err.message.contains(expected),
                "unexpected semantic error: {}",
                err.message
            );
        }
    }

    #[test]
    fn legacy_host_runtime_control_builtins_emit_syscalls() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(
                r#"
fn main() {
  create_nfts_for_all_users();
  set_execution_depth(3);
}
"#,
            )
            .expect("compile legacy host runtime controls");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let code = &bytes[parsed.code_offset..];

        for (syscall, label) in [
            (
                ivm_abi::syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS,
                "CREATE_NFTS_FOR_ALL_USERS",
            ),
            (
                ivm_abi::syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH,
                "SET_SMARTCONTRACT_EXECUTION_DEPTH",
            ),
        ] {
            let needle = encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(syscall).expect("legacy runtime control syscall fits imm8"),
            )
            .to_le_bytes();
            assert!(
                code.windows(needle.len()).any(|window| window == needle),
                "expected {label} syscall"
            );
        }
    }

    #[test]
    fn legacy_host_runtime_control_builtins_reject_invalid_arguments() {
        for (source, expected) in [
            (
                r#"fn main() { create_nfts_for_all_users(1); }"#,
                "create_nfts_for_all_users expects no arguments",
            ),
            (
                r#"fn main() { set_execution_depth("deep"); }"#,
                "set_execution_depth expects one int arg",
            ),
        ] {
            let parsed = parse(source).expect("parse invalid legacy runtime control call");
            let err =
                analyze(&parsed).expect_err("semantic analysis should reject runtime control args");
            assert!(
                err.message.contains(expected),
                "unexpected semantic error: {}",
                err.message
            );
        }
    }

    #[test]
    fn resolve_account_alias_invalid_literal_emits_syscall() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(r#"fn main() { let _acct = resolve_account_alias("merchant@"); }"#)
            .expect("compile malformed builtin alias resolution");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected RESOLVE_ACCOUNT_ALIAS syscall for malformed builtin alias literals"
        );
    }

    #[test]
    fn resolve_account_alias_domain_qualified_builtin_emits_syscall() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(
                r#"fn main() { let _acct = resolve_account_alias("merchant@bank.paynet"); }"#,
            )
            .expect("compile domain-qualified builtin alias resolution");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected RESOLVE_ACCOUNT_ALIAS syscall for domain-qualified builtin"
        );
    }

    #[test]
    fn resolve_account_alias_invalid_domain_qualified_literal_emits_syscall() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(r#"fn main() { let _acct = resolve_account_alias("merchant@bank."); }"#)
            .expect("compile malformed domain-qualified builtin alias resolution");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let needle = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(needle.len())
                .any(|window| window == needle),
            "expected RESOLVE_ACCOUNT_ALIAS syscall for malformed domain-qualified builtin alias literals"
        );
    }

    #[test]
    fn account_id_canonical_literal_stays_static_without_alias_resolution() {
        let canonical = iroha_data_model::account::AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .expect("public key"),
        )
        .to_string();
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(&format!(
                r#"fn main() {{ let _acct = account_id("{canonical}"); }}"#
            ))
            .expect("compile canonical account literal");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let resolve = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            !bytes[parsed.code_offset..]
                .windows(resolve.len())
                .any(|window| window == resolve),
            "canonical AccountId literals must not emit alias resolution syscalls"
        );

        let static_tlv =
            super::encode_pointer_tlv_bytes(super::ir::DataRefKind::Account, &canonical)
                .expect("encode static AccountId tlv");
        assert!(
            bytes
                .windows(static_tlv.len())
                .any(|window| window == static_tlv),
            "canonical AccountId literals should be embedded as static TLVs"
        );
    }

    #[test]
    fn account_id_invalid_alias_shaped_literal_compiles_for_runtime_resolution() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(r#"fn main() { let _acct = account_id("merchant@"); }"#)
            .expect("compile invalid alias-shaped literal");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let resolve = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(resolve.len())
                .any(|window| window == resolve),
            "alias-shaped literals should defer validation to runtime resolution"
        );
    }

    #[test]
    fn account_id_invalid_domain_qualified_alias_shaped_literal_compiles_for_runtime_resolution() {
        let compiler = test_mode_compiler();
        let bytes = compiler
            .compile_source(r#"fn main() { let _acct = account_id("merchant@bank."); }"#)
            .expect("compile invalid domain-qualified alias-shaped literal");
        let parsed = ProgramMetadata::parse(&bytes).expect("parse metadata");
        let resolve = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            ivm_abi::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8,
        )
        .to_le_bytes();
        assert!(
            bytes[parsed.code_offset..]
                .windows(resolve.len())
                .any(|window| window == resolve),
            "invalid domain-qualified alias-shaped literals should defer validation to runtime resolution"
        );
    }

    #[test]
    fn account_id_invalid_non_alias_literal_fails_compile_time_encoding() {
        let compiler = test_mode_compiler();
        let err = compiler
            .compile_source(r#"fn main() { let _acct = account_id("merchant"); }"#)
            .expect_err("invalid non-alias account literal should fail compile-time encoding");
        assert!(
            err.contains("invalid AccountId literal"),
            "expected AccountId literal error, got: {err}"
        );
        assert!(
            err.contains("merchant"),
            "expected failing literal in error, got: {err}"
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
    fn zero_arg_public_entrypoint_retains_scalar_state_hints() {
        let src = r#"
seiyaku Test {
  state int counter;

  kotoage fn run() {
    let current = counter;
    if current > 0 {
      info("tick");
    }
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
        assert_eq!(hints.read_keys, vec!["state:counter".to_string()]);
        assert!(hints.write_keys.is_empty());

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let run = entrypoints
            .iter()
            .find(|entry| entry.name == "run")
            .expect("run entrypoint");
        assert_eq!(run.read_keys, vec!["state:counter".to_string()]);
        assert!(run.write_keys.is_empty());
        assert_eq!(run.access_hints_complete, Some(true));
        assert!(run.access_hints_skipped.is_empty());
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
    fn manifest_includes_dynamic_state_iteration_hints() {
        let src = r#"
seiyaku Test {
  state Foo: Map<int, int>;

  kotoage fn sum(n: int) -> int {
    let acc = 0;
    for (k, v) in Foo.take(n) {
      acc = acc + v;
    }
    return acc;
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
        assert_eq!(hints.dynamic_reads.len(), 1);
        let dynamic = &hints.dynamic_reads[0];
        assert_eq!(dynamic.base_key, "state:Foo");
        assert_eq!(dynamic.key_type, "int");
        assert_eq!(dynamic.bound_kind, "take");
        assert_eq!(
            dynamic.max_keys,
            crate::semantic::DYNAMIC_ITERATION_LIMIT as u32
        );
    }

    #[test]
    fn manifest_access_set_hints_omit_state_wildcard_for_dynamic_state_path() {
        let src = r#"
seiyaku Test {
  kotoage fn read(path: Name) {
    let _x = state_get(path);
  }
}
"#;
        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let read = entrypoints
            .iter()
            .find(|entry| entry.name == "read")
            .expect("read entrypoint");
        assert_taira_supported_access_keys(&read.read_keys);
        assert_taira_supported_access_keys(&read.write_keys);
        assert_eq!(read.access_hints_complete, Some(false));
        assert!(!read.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_state_wildcard_for_call_contract() {
        let src = r#"
seiyaku Test {
  kotoage fn relay(target: bytes, payload: Json) -> bytes permission(Admin) {
    return call_contract(target, "settle", payload);
  }
}
"#;
        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let relay = entrypoints
            .iter()
            .find(|entry| entry.name == "relay")
            .expect("relay entrypoint");
        assert_taira_supported_access_keys(&relay.read_keys);
        assert_taira_supported_access_keys(&relay.write_keys);
        assert_eq!(relay.access_hints_complete, Some(false));
        assert!(!relay.access_hints_skipped.is_empty());
    }

    #[test]
    fn compile_json_object_builders() {
        let src = r#"
seiyaku Test {
  kotoage fn build(owner: AccountId) -> Json {
    let payload = json_object();
    let payload = json_set_int(payload, name("bucket_id"), 1);
    return json_set_account_id(payload, name("owner"), owner);
  }
}
"#;
        let compiler = Compiler::new();
        compiler
            .compile_source_with_manifest(src)
            .expect("compile json object builders");
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
            DomainId::try_new("wonderland", "universal").unwrap(),
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
        assert!(
            canonical_asset.definition().try_domain().is_none(),
            "canonical execute_instruction payload should decode to an opaque asset definition id",
        );
        assert!(
            !hints.read_keys.iter().any(|key| key.starts_with("domain:")),
            "opaque canonical asset ids should not synthesize a domain access hint",
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
    fn manifest_access_set_hints_include_execute_instruction_escrow_literal() {
        use iroha_data_model::{
            asset::AssetDefinitionId,
            isi::{InstructionBox, escrow::OpenAssetEscrow},
        };
        use iroha_primitives::numeric::Numeric;

        let escrow_hash = kotodama_escrow_hex("aitai_offer");
        let escrow_name: iroha_data_model::name::Name = "aitai_offer".parse().expect("escrow name");
        let escrow_id = iroha_data_model::escrow::EscrowId::from_kotodama_name(&escrow_name);
        let asset_def: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("asset definition");
        let isi = InstructionBox::from(OpenAssetEscrow::new(
            escrow_id,
            asset_def.clone(),
            Numeric::from(10_u64),
        ));
        let bytes = norito::to_bytes(&isi).expect("encode InstructionBox");
        let hex_payload = format!("0x{}", hex::encode(bytes));
        let src = format!(
            r#"
fn main() {{
  execute_instruction(norito_bytes("{hex_payload}"));
}}
"#
        );

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile escrow execute_instruction literal");
        let hints = manifest
            .access_set_hints
            .expect("expected escrow execute_instruction access hints");
        for key in [
            format!("escrow_id:{escrow_hash}"),
            format!("asset_escrow:{escrow_hash}"),
            format!("asset_def:{asset_def}"),
            format!("asset:{asset_def}:$authority"),
        ] {
            assert!(hints.read_keys.contains(&key), "missing read key {key}");
            assert!(hints.write_keys.contains(&key), "missing write key {key}");
        }

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

        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let nft_id: NftId = "n0$wonderland.universal".parse().unwrap();
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
    fn manifest_access_set_hints_include_nft_set_metadata_literal() {
        let src = r#"
fn main() {
  nft_set_metadata(nft_id("n0$wonderland.universal"), name("dpn_metadata"), json("{\"meta\":1}"));
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        let nft_key = "nft:n0$wonderland.universal".to_string();
        let nft_detail = "nft.detail:n0$wonderland.universal:dpn_metadata".to_string();
        assert!(hints.read_keys.contains(&NFT_COARSE_KEY.to_string()));
        assert!(hints.write_keys.contains(&NFT_COARSE_KEY.to_string()));
        assert!(hints.read_keys.contains(&nft_key));
        assert!(hints.write_keys.contains(&nft_key));
        assert!(hints.read_keys.contains(&nft_detail));
        assert!(hints.write_keys.contains(&nft_detail));
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_coarse_key_for_dynamic_nft_set_metadata() {
        let src = r#"
seiyaku Test {
  kotoage fn set_metadata(nft: NftId, metadata: Json) permission(NftAuthority) {
    nft_set_metadata(nft, name("dpn_metadata"), metadata);
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
        assert!(hints.read_keys.contains(&NFT_COARSE_KEY.to_string()));
        assert!(hints.write_keys.contains(&NFT_COARSE_KEY.to_string()));
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let entry = entrypoints
            .iter()
            .find(|entry| entry.name == "set_metadata")
            .expect("set_metadata entrypoint");
        assert_eq!(entry.access_hints_complete, Some(true));
        assert!(entry.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_coarse_key_for_dynamic_nft_mint() {
        let src = r#"
seiyaku Test {
  kotoage fn mint(nft: NftId, owner: AccountId) permission(NftAuthority) {
    nft_mint_asset(nft, owner);
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
        assert!(hints.read_keys.contains(&NFT_COARSE_KEY.to_string()));
        assert!(hints.write_keys.contains(&NFT_COARSE_KEY.to_string()));
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let entry = entrypoints
            .iter()
            .find(|entry| entry.name == "mint")
            .expect("mint entrypoint");
        assert_eq!(entry.access_hints_complete, Some(true));
        assert!(entry.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_asset_registration_literals() {
        use iroha_data_model::asset::id::{AssetDefinitionId, AssetId};

        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let asset_def = AssetDefinitionId::parse_address_literal(asset_literal).unwrap();
        let account = sample_account_id();
        let account_literal = account.to_string();
        let asset_id = AssetId::of(asset_def.clone(), account.clone());
        let src = format!(
            r#"
fn main() {{
  register_asset(asset_definition("{asset_literal}"), "ROSE", 0, 1);
  create_new_asset(asset_definition("{asset_literal}"), "ROSE", 1, account_id("{account_literal}"), 1);
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
        assert!(hints.read_keys.contains(&format!("asset_def:{asset_def}")));
        assert!(hints.write_keys.contains(&format!("asset_def:{asset_def}")));
        assert!(hints.read_keys.contains(&format!("asset:{asset_id}")));
        assert!(hints.write_keys.contains(&format!("asset:{asset_id}")));
        assert!(hints.read_keys.contains(&format!("account:{account}")));
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_authority_placeholders() {
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"
fn main() {{
  register_asset(asset_definition("{asset_literal}"), "ROSE", 0, 1);
  create_role(name("minter"), json("{{\"perms\":[\"mint_asset:{asset_literal}\"]}}"));
  grant_role(authority(), name("minter"));
  mint_asset(authority(), asset_definition("{asset_literal}"), 1);
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
        assert!(hints.read_keys.contains(&AUTHORITY_ACCOUNT_KEY.to_owned()));
        assert!(hints.write_keys.contains(&AUTHORITY_ACCOUNT_KEY.to_owned()));
        assert!(
            hints
                .write_keys
                .contains(&"role.binding:$authority:minter".to_owned())
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("asset:{asset_literal}:$authority"))
        );
        assert!(
            hints
                .write_keys
                .contains(&format!("asset:{asset_literal}:$authority"))
        );
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_sysvar_authority_placeholders() {
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let source = format!(
            r#"
fn main() {{
  let caller = sysvar_authority();
  let asset = asset_definition("{asset_literal}");
  transfer_asset(caller, caller, asset, 1);
  set_account_detail(caller, name("status"), json("{{}}"));
}}
"#
        );

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&source)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        let authority_asset = format!("asset:{asset_literal}:$authority");
        let authority_detail = "account.detail:$authority:status".to_owned();

        assert!(hints.read_keys.contains(&AUTHORITY_ACCOUNT_KEY.to_owned()));
        assert!(hints.read_keys.contains(&authority_asset));
        assert!(hints.write_keys.contains(&authority_asset));
        assert!(hints.read_keys.contains(&authority_detail));
        assert!(hints.write_keys.contains(&authority_detail));
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
        assert!(main.read_keys.contains(&AUTHORITY_ACCOUNT_KEY.to_owned()));
        assert!(main.read_keys.contains(&authority_asset));
        assert!(main.write_keys.contains(&authority_asset));
        assert!(main.read_keys.contains(&authority_detail));
        assert!(main.write_keys.contains(&authority_detail));
    }

    #[test]
    fn manifest_access_set_hints_include_execute_query_literal() {
        use iroha_data_model::{
            account::AccountId,
            asset::id::{AssetDefinitionId, AssetId},
            query::asset::{FindAssetById, FindAssetDefinitionById},
            query::{QueryRequest, SingularQueryBox},
        };

        let account = AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .expect("public key"),
        );
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
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
                .contains(&format!("asset_def:{}", canonical_asset.definition()))
        );
        assert!(
            hints
                .read_keys
                .contains(&format!("asset:{canonical_asset}"))
        );
        assert!(
            canonical_asset.definition().try_domain().is_none(),
            "canonical execute_query payload should decode to an opaque asset definition id",
        );
        assert!(
            !hints.read_keys.iter().any(|key| key.starts_with("domain:")),
            "opaque canonical asset ids should not synthesize a domain access hint",
        );
        assert!(hints.write_keys.is_empty());

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());

        let definition_request = QueryRequest::Singular(SingularQueryBox::FindAssetDefinitionById(
            FindAssetDefinitionById::new(asset_def.clone()),
        ));
        let definition_bytes = norito::to_bytes(&definition_request).expect("encode QueryRequest");
        let definition_hex_payload = format!("0x{}", hex::encode(definition_bytes));
        let definition_src =
            format!("fn main() {{ execute_query(norito_bytes(\"{definition_hex_payload}\")); }}");

        let (_bytes, definition_manifest) = compiler
            .compile_source_with_manifest(&definition_src)
            .expect("compile asset definition query manifest");
        let definition_hints = definition_manifest
            .access_set_hints
            .expect("expected asset definition access_set_hints");
        assert_eq!(
            definition_hints.read_keys,
            vec![format!("asset_def:{asset_def}")]
        );
        assert!(definition_hints.write_keys.is_empty());
        assert!(
            !definition_hints
                .read_keys
                .iter()
                .any(|key| key.starts_with("domain:")),
            "opaque canonical asset definition query should not synthesize a domain access hint",
        );
    }

    #[test]
    fn manifest_access_set_hints_include_inline_zk_vendor_payloads() {
        use iroha_data_model::{
            account::{AccountId, ParsedAccountId},
            asset::id::{AssetDefinitionId, AssetId},
        };

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(include_str!("samples/zk_vote_and_unshield.ko"))
            .expect("compile sample manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        let asset_def = AssetDefinitionId::parse_address_literal("6pEP9RjNoZ7beWkT3pLfKoM1dyfi")
            .expect("sample asset definition");
        let account =
            AccountId::parse_encoded("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
                .map(ParsedAccountId::into_account_id)
                .expect("sample account");
        let asset = AssetId::of(asset_def.clone(), account.clone());

        assert!(
            hints
                .write_keys
                .contains(&"zk:election:election-1:ciphertexts".to_string())
        );
        assert!(
            hints
                .write_keys
                .contains(&"zk:election:election-1:nullifiers".to_string())
        );
        assert!(hints.write_keys.contains(&format!("zk_asset:{asset_def}")));
        assert!(hints.write_keys.contains(&format!("asset:{asset}")));
        assert!(
            hints
                .write_keys
                .contains(&format!("asset_def.detail:{asset_def}:zk.unshield.last"))
        );
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let demo = entrypoints
            .iter()
            .find(|entry| entry.name == "demo")
            .expect("demo entrypoint");
        assert_eq!(demo.access_hints_complete, Some(true));
        assert!(demo.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_transfer_domain_literal() {
        use iroha_data_model::domain::DomainId;

        let from_literal = sample_account_literal();
        let to = sample_account_id_alt();
        let to_literal = to.to_string();
        let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
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
    fn manifest_access_set_hints_omit_global_wildcard_for_alias_shorthand_account_id() {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), account_id("merchant@paynet"), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_invalid_alias_shorthand_account_id_transfer()
     {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), account_id("merchant@"), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_domain_qualified_alias_shorthand_account_id()
     {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), account_id("merchant@bank.paynet"), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_invalid_domain_qualified_alias_shorthand_account_id_transfer()
     {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), account_id("merchant@bank."), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_resolve_account_alias_builtin_transfer() {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), resolve_account_alias("merchant@paynet"), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_invalid_resolve_account_alias_builtin_transfer()
     {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), resolve_account_alias("merchant@"), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_domain_qualified_resolve_account_alias_builtin_transfer()
     {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), resolve_account_alias("merchant@bank.paynet"), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_invalid_domain_qualified_resolve_account_alias_builtin_transfer()
     {
        let from_literal = sample_account_literal();
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"fn main() {{ transfer_asset(account_id("{from_literal}"), resolve_account_alias("merchant@bank."), asset_definition("{asset_literal}"), 1); }}"#
        );

        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_coarse_asset_keys_for_dynamic_asset_contract() {
        let src = r#"
seiyaku Test {
  kotoage fn move(from: AccountId, to: AccountId, asset: AssetDefinitionId, amount: int) permission(Admin) {
    transfer_asset(from, to, asset, amount);
  }
}
"#;
        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "move")
            .expect("move entrypoint");
        assert_taira_supported_access_keys(&main.read_keys);
        assert_taira_supported_access_keys(&main.write_keys);
        assert_eq!(main.access_hints_complete, Some(true));
        assert!(main.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_omit_global_wildcard_for_opaque_host_calls() {
        let src = r#"
seiyaku Test {
  kotoage fn register() permission(Admin) {
    register_peer(json("{}"));
  }
}
"#;
        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        assert!(
            manifest.access_set_hints.is_none(),
            "opaque host calls should not persist wildcard-only access hints"
        );
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let register = entrypoints
            .iter()
            .find(|entry| entry.name == "register")
            .expect("register entrypoint");
        assert_taira_supported_access_keys(&register.read_keys);
        assert_taira_supported_access_keys(&register.write_keys);
        assert_eq!(register.access_hints_complete, Some(false));
        assert!(!register.access_hints_skipped.is_empty());
    }

    #[test]
    fn manifest_access_set_hints_include_static_smart_contract_lifecycle_helpers() {
        let code_hash = iroha_crypto::Hash::new(b"kotodama lifecycle access hints");
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            7,
            &sample_account_id(),
            0,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: Some("test".to_owned()),
            features_bitmap: Some(0),
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            provenance: None,
        };
        let register_code_hex = hex::encode(
            norito::to_bytes(
                &iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode { manifest },
            )
            .expect("register manifest request"),
        );
        let register_bytes_hex = hex::encode(
            norito::to_bytes(
                &iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes {
                    code_hash,
                    code: vec![0, 1, 2, 3],
                },
            )
            .expect("register bytes request"),
        );
        let activate_hex = hex::encode(
            norito::to_bytes(
                &iroha_data_model::isi::smart_contract_code::ActivateContractInstance {
                    contract_address: contract_address.clone(),
                    code_hash,
                },
            )
            .expect("activate request"),
        );
        let remove_hex = hex::encode(
            norito::to_bytes(
                &iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes {
                    code_hash,
                    reason: Some("test cleanup".to_owned()),
                },
            )
            .expect("remove request"),
        );
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn lifecycle() permission(Admin) {{
    register_smart_contract_code(norito_bytes("0x{register_code_hex}"));
    register_smart_contract_bytes(norito_bytes("0x{register_bytes_hex}"));
    activate_contract_instance(norito_bytes("0x{activate_hex}"));
    remove_smart_contract_bytes(norito_bytes("0x{remove_hex}"));
  }}
}}
"#
        );

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("static smart contract lifecycle helpers should have complete access hints");
        let hints = manifest
            .access_set_hints
            .expect("expected contract lifecycle access hints");
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);
        assert!(!hints.read_keys.iter().any(|key| key == GLOBAL_WILDCARD_KEY));
        assert!(
            !hints
                .write_keys
                .iter()
                .any(|key| key == GLOBAL_WILDCARD_KEY)
        );

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let lifecycle = entrypoints
            .iter()
            .find(|entry| entry.name == "lifecycle")
            .expect("lifecycle entrypoint");
        assert_eq!(lifecycle.access_hints_complete, Some(true));
        assert!(lifecycle.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&lifecycle.read_keys);
        assert_taira_supported_access_keys(&lifecycle.write_keys);

        for key in [
            super::key_contract_code(&code_hash),
            super::key_contract_manifest(&code_hash),
            super::key_contract_instance(&contract_address),
            super::key_contract_instance_code_hash(&code_hash),
        ] {
            assert!(
                lifecycle.read_keys.iter().any(|actual| actual == &key),
                "missing lifecycle read key {key}; got {:?}",
                lifecycle.read_keys
            );
        }
        for key in [
            super::key_contract_code(&code_hash),
            super::key_contract_manifest(&code_hash),
            super::key_contract_instance(&contract_address),
            super::key_contract_instance_code_hash(&code_hash),
        ] {
            assert!(
                lifecycle.write_keys.iter().any(|actual| actual == &key),
                "missing lifecycle write key {key}; got {:?}",
                lifecycle.write_keys
            );
        }
    }

    #[test]
    fn manifest_access_set_hints_include_literal_nullifier_helper() {
        let src = r#"
seiyaku Test {
  kotoage fn consume() permission(Admin) {
    use_nullifier(42);
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("literal nullifier should have complete access hints");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let consume = entrypoints
            .iter()
            .find(|entry| entry.name == "consume")
            .expect("consume entrypoint");
        assert_eq!(consume.access_hints_complete, Some(true));
        assert!(consume.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&consume.read_keys);
        assert_taira_supported_access_keys(&consume.write_keys);
        let nullifier_key = super::key_nullifier(42);
        assert_eq!(consume.read_keys, [nullifier_key.clone()]);
        assert_eq!(consume.write_keys, [nullifier_key]);
    }

    #[test]
    fn manifest_access_set_hints_include_static_peer_helpers() {
        let public_key = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774";
        let peer = iroha_data_model::peer::PeerId::from(
            public_key
                .parse::<iroha_crypto::PublicKey>()
                .expect("public key"),
        );
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn peers() permission(Admin) {{
    register_peer(json("{{\"public_key\":\"{public_key}\",\"pop\":[]}}"));
    unregister_peer(json("{{\"public_key\":\"{public_key}\"}}"));
  }}
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("static peer helpers should have complete access hints");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let peers = entrypoints
            .iter()
            .find(|entry| entry.name == "peers")
            .expect("peers entrypoint");
        let peer_key = format!("peer:{peer}");
        assert_eq!(peers.access_hints_complete, Some(true));
        assert!(peers.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&peers.read_keys);
        assert_taira_supported_access_keys(&peers.write_keys);
        assert_eq!(peers.read_keys, std::slice::from_ref(&peer_key));
        assert_eq!(peers.write_keys, [peer_key]);
    }

    #[test]
    fn manifest_access_set_hints_include_subscription_helpers() {
        let src = r#"
seiyaku Test {
  kotoage fn subscription() permission(Admin) {
    subscription_bill();
    subscription_record_usage();
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("subscription helpers should have complete access hints");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let subscription = entrypoints
            .iter()
            .find(|entry| entry.name == "subscription")
            .expect("subscription entrypoint");
        let keys = [
            "subscription:trigger_context:bill".to_owned(),
            "subscription:trigger_context:usage".to_owned(),
        ];
        assert_eq!(subscription.access_hints_complete, Some(true));
        assert!(subscription.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&subscription.read_keys);
        assert_taira_supported_access_keys(&subscription.write_keys);
        assert_eq!(subscription.read_keys, keys);
        assert_eq!(subscription.write_keys, keys);
    }

    #[test]
    fn manifest_access_set_hints_include_static_axt_helpers() {
        let descriptor = crate::axt::AxtDescriptor {
            dsids: vec![iroha_data_model::nexus::DataSpaceId::new(7)],
            touches: vec![crate::axt::AxtTouchSpec {
                dsid: iroha_data_model::nexus::DataSpaceId::new(7),
                read: vec!["balance".to_owned()],
                write: vec!["lock".to_owned()],
            }],
        };
        let manifest = crate::axt::TouchManifest {
            read: vec!["orders".to_owned()],
            write: vec!["settlements".to_owned()],
        };
        let handle = crate::axt::AssetHandle {
            scope: vec!["transfer".to_owned()],
            subject: crate::axt::HandleSubject {
                account: "alice@wonderland".to_owned(),
                origin_dsid: Some(iroha_data_model::nexus::DataSpaceId::new(11)),
            },
            budget: crate::axt::HandleBudget {
                remaining: 100,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 2,
            group_binding: crate::axt::GroupBinding {
                composability_group_id: b"group".to_vec(),
                epoch_id: 3,
            },
            target_lane: iroha_data_model::nexus::LaneId::SINGLE,
            axt_binding: vec![4; 32],
            manifest_view_root: vec![5; 32],
            expiry_slot: 99,
            max_clock_skew_ms: Some(500),
        };
        let intent = crate::axt::RemoteSpendIntent {
            asset_dsid: iroha_data_model::nexus::DataSpaceId::new(13),
            op: crate::axt::SpendOp {
                kind: "transfer".to_owned(),
                from: "alice@wonderland".to_owned(),
                to: "bob@wonderland".to_owned(),
                amount: "10".to_owned(),
            },
        };
        let descriptor_hex = hex::encode(norito::to_bytes(&descriptor).expect("descriptor"));
        let manifest_hex = hex::encode(norito::to_bytes(&manifest).expect("manifest"));
        let handle_hex = hex::encode(norito::to_bytes(&handle).expect("handle"));
        let intent_hex = hex::encode(norito::to_bytes(&intent).expect("intent"));
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn axt() permission(Admin) {{
    axt_begin(axt_descriptor("0x{descriptor_hex}"));
    axt_touch(dataspace_id("7"), norito_bytes("0x{manifest_hex}"));
    verify_ds_proof(dataspace_id("7"));
    use_asset_handle(asset_handle("0x{handle_hex}"), norito_bytes("0x{intent_hex}"));
    axt_commit();
  }}
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("static AXT helpers should have complete access hints");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let axt = entrypoints
            .iter()
            .find(|entry| entry.name == "axt")
            .expect("axt entrypoint");
        assert_eq!(axt.access_hints_complete, Some(true));
        assert!(axt.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&axt.read_keys);
        assert_taira_supported_access_keys(&axt.write_keys);
        assert!(!axt.read_keys.iter().any(|key| key == GLOBAL_WILDCARD_KEY));
        assert!(!axt.write_keys.iter().any(|key| key == GLOBAL_WILDCARD_KEY));
        for key in [
            "axt:dataspace:7",
            "axt:dataspace:7:balance",
            "axt:dataspace:7:lock",
            "axt:dataspace:7:orders",
            "axt:dataspace:7:settlements",
            "axt:dataspace:7:proof",
            "axt:dataspace:11",
            "axt:dataspace:13",
        ] {
            assert!(
                axt.read_keys.iter().any(|actual| actual == key),
                "missing AXT read key {key}; got {:?}",
                axt.read_keys
            );
        }
        for key in [
            "axt:dataspace:7",
            "axt:dataspace:7:lock",
            "axt:dataspace:7:settlements",
            "axt:dataspace:11",
            "axt:dataspace:13",
        ] {
            assert!(
                axt.write_keys.iter().any(|actual| actual == key),
                "missing AXT write key {key}; got {:?}",
                axt.write_keys
            );
        }
    }

    #[test]
    fn manifest_access_set_hints_include_static_soracloud_and_vrf_helpers() {
        let request = iroha_data_model::soracloud::SoracloudHostRequestEnvelopeV1 {
            schema_version: iroha_data_model::soracloud::SORACLOUD_HOST_REQUEST_VERSION_V1,
            operation: iroha_data_model::soracloud::SoracloudHostOperationV1::ReadConfig,
            payload: iroha_data_model::soracloud::SoracloudHostRequestPayloadV1::ReadConfig(
                iroha_data_model::soracloud::SoracloudReadConfigRequestV1 {
                    config_name: "runtime".to_owned(),
                },
            ),
        };
        let request_hex = hex::encode(norito::to_bytes(&request).expect("request"));
        let vrf_hex = hex::encode(norito::to_bytes(&(42_u64, true)).expect("vrf request"));
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn read() -> bytes permission(Admin) {{
    let request = soracloud_request("0x{request_hex}");
    let _config = soracloud_read_config(request);
    let seed = vrf_epoch_seed(norito_bytes("0x{vrf_hex}"));
    return seed;
  }}
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("static Soracloud and VRF helpers should have complete access hints");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let read = entrypoints
            .iter()
            .find(|entry| entry.name == "read")
            .expect("read entrypoint");
        assert_eq!(read.access_hints_complete, Some(true));
        assert!(read.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&read.read_keys);
        assert_taira_supported_access_keys(&read.write_keys);
        assert_eq!(read.write_keys, Vec::<String>::new());
        for key in [
            "soracloud:config:runtime",
            "vrf:epoch_seed:42",
            "vrf:epoch_seed:latest",
        ] {
            assert!(
                read.read_keys.iter().any(|actual| actual == key),
                "missing read key {key}; got {:?}",
                read.read_keys
            );
        }
    }

    #[test]
    fn manifest_access_set_hints_include_static_soracloud_operation_keys() {
        use iroha_data_model::soracloud::{
            SORACLOUD_HOST_REQUEST_VERSION_V1, SoraStateEncryptionV1, SoraStateMutationOperationV1,
            SoracloudAppendJournalRequestV1, SoracloudEgressFetchRequestV1,
            SoracloudEmitMailboxMessageRequestV1, SoracloudEmitStateMutationRequestV1,
            SoracloudHostOperationV1 as Op, SoracloudHostRequestEnvelopeV1,
            SoracloudHostRequestPayloadV1 as Payload, SoracloudPublishCheckpointRequestV1,
            SoracloudReadCommittedStateRequestV1, SoracloudReadConfigRequestV1,
            SoracloudReadCredentialRequestV1, SoracloudReadSecretEnvelopeRequestV1,
            SoracloudReadSecretRequestV1,
        };

        let name = |value: &str| -> iroha_data_model::name::Name { value.parse().expect("name") };
        let request_hex = |operation: Op, payload: Payload| -> String {
            let request = SoracloudHostRequestEnvelopeV1 {
                schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
                operation,
                payload,
            };
            hex::encode(norito::to_bytes(&request).expect("request"))
        };
        let read_state = request_hex(
            Op::ReadCommittedState,
            Payload::ReadCommittedState(SoracloudReadCommittedStateRequestV1 {
                binding_name: name("wallet"),
                state_key: "/accounts/alice".to_owned(),
            }),
        );
        let write_state = request_hex(
            Op::EmitStateMutation,
            Payload::EmitStateMutation(SoracloudEmitStateMutationRequestV1 {
                binding_name: name("wallet"),
                state_key: "/accounts/alice".to_owned(),
                operation: SoraStateMutationOperationV1::Upsert,
                encryption: SoraStateEncryptionV1::Plaintext,
                payload_bytes: Some(3),
                payload: Some(vec![1, 2, 3]),
                payload_commitment: None,
            }),
        );
        let mailbox = request_hex(
            Op::EmitMailboxMessage,
            Payload::EmitMailboxMessage(SoracloudEmitMailboxMessageRequestV1 {
                to_service: name("settlement"),
                to_handler: name("on_message"),
                payload_bytes: vec![4, 5],
                available_after_sequence: 7,
                expires_at_sequence: Some(11),
            }),
        );
        let journal = request_hex(
            Op::AppendJournal,
            Payload::AppendJournal(SoracloudAppendJournalRequestV1 {
                artifact_path: "/journals/run-7.json".to_owned(),
                payload_bytes: vec![6],
            }),
        );
        let checkpoint = request_hex(
            Op::PublishCheckpoint,
            Payload::PublishCheckpoint(SoracloudPublishCheckpointRequestV1 {
                artifact_path: "/checkpoints/run-7.bin".to_owned(),
                payload_bytes: vec![7],
            }),
        );
        let config = request_hex(
            Op::ReadConfig,
            Payload::ReadConfig(SoracloudReadConfigRequestV1 {
                config_name: "runtime".to_owned(),
            }),
        );
        let envelope = request_hex(
            Op::ReadSecretEnvelope,
            Payload::ReadSecretEnvelope(SoracloudReadSecretEnvelopeRequestV1 {
                secret_name: "api-key".to_owned(),
            }),
        );
        let secret = request_hex(
            Op::ReadSecret,
            Payload::ReadSecret(SoracloudReadSecretRequestV1 {
                secret_name: "node-token".to_owned(),
            }),
        );
        let credential = request_hex(
            Op::ReadCredential,
            Payload::ReadCredential(SoracloudReadCredentialRequestV1 {
                credential_name: "mtls-client".to_owned(),
            }),
        );
        let egress = request_hex(
            Op::EgressFetch,
            Payload::EgressFetch(SoracloudEgressFetchRequestV1 {
                url: "https://oracle.example/data".to_owned(),
                max_bytes: 4096,
                expected_hash: None,
            }),
        );
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn soracloud() permission(Admin) {{
    let read_state = soracloud_request("0x{read_state}");
    let write_state = soracloud_request("0x{write_state}");
    let mailbox = soracloud_request("0x{mailbox}");
    let journal = soracloud_request("0x{journal}");
    let checkpoint = soracloud_request("0x{checkpoint}");
    let config = soracloud_request("0x{config}");
    let envelope = soracloud_request("0x{envelope}");
    let secret = soracloud_request("0x{secret}");
    let credential = soracloud_request("0x{credential}");
    let egress = soracloud_request("0x{egress}");
    let _read_state = soracloud_read_committed_state(read_state);
    let _write_state = soracloud_emit_state_mutation(write_state);
    let _mailbox = soracloud_emit_mailbox_message(mailbox);
    let _journal = soracloud_append_journal(journal);
    let _checkpoint = soracloud_publish_checkpoint(checkpoint);
    let _config = soracloud_read_config(config);
    let _envelope = soracloud_read_secret_envelope(envelope);
    let _secret = soracloud_read_secret(secret);
    let _credential = soracloud_read_credential(credential);
    let _egress = soracloud_egress_fetch(egress);
  }}
}}
"#
        );
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("static Soracloud helpers should have complete access hints");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let soracloud = entrypoints
            .iter()
            .find(|entry| entry.name == "soracloud")
            .expect("soracloud entrypoint");
        assert_eq!(soracloud.access_hints_complete, Some(true));
        assert!(soracloud.access_hints_skipped.is_empty());
        assert_taira_supported_access_keys(&soracloud.read_keys);
        assert_taira_supported_access_keys(&soracloud.write_keys);
        for key in [
            "soracloud:state:wallet:accounts/alice",
            "soracloud:config:runtime",
            "soracloud:secret_envelope:api-key",
            "soracloud:node_secret:node-token",
            "soracloud:node_credential:mtls-client",
            "soracloud:egress:https://oracle.example/data",
        ] {
            assert!(
                soracloud.read_keys.iter().any(|actual| actual == key),
                "missing Soracloud read key {key}; got {:?}",
                soracloud.read_keys
            );
        }
        for key in [
            "soracloud:state:wallet:accounts/alice",
            "soracloud:mailbox:settlement:on_message",
            "soracloud:journal:journals/run-7.json",
            "soracloud:checkpoint:checkpoints/run-7.bin",
        ] {
            assert!(
                soracloud.write_keys.iter().any(|actual| actual == key),
                "missing Soracloud write key {key}; got {:?}",
                soracloud.write_keys
            );
        }
    }

    #[test]
    fn manifest_access_set_hints_reject_adversarial_static_host_payloads() {
        let request = iroha_data_model::soracloud::SoracloudHostRequestEnvelopeV1 {
            schema_version: iroha_data_model::soracloud::SORACLOUD_HOST_REQUEST_VERSION_V1,
            operation: iroha_data_model::soracloud::SoracloudHostOperationV1::ReadConfig,
            payload: iroha_data_model::soracloud::SoracloudHostRequestPayloadV1::ReadConfig(
                iroha_data_model::soracloud::SoracloudReadConfigRequestV1 {
                    config_name: "runtime".to_owned(),
                },
            ),
        };
        let request_hex = hex::encode(norito::to_bytes(&request).expect("request"));
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn bad_peer() permission(Admin) {{
    register_peer(json("{{}}"));
  }}

  kotoage fn bad_axt() permission(Admin) {{
    let descriptor = axt_descriptor(norito_bytes("0x00"));
    axt_begin(descriptor);
  }}

  kotoage fn bad_soracloud() permission(Admin) {{
    let request = soracloud_request("0x{request_hex}");
    let _secret = soracloud_read_secret(request);
  }}
}}
"#
        );
        let compiler = test_mode_compiler();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("test mode should report incomplete host access hints");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        for name in ["bad_peer", "bad_axt", "bad_soracloud"] {
            let entry = entrypoints
                .iter()
                .find(|entry| entry.name == name)
                .expect("entrypoint");
            assert_eq!(entry.access_hints_complete, Some(false), "{name}");
            assert!(!entry.access_hints_skipped.is_empty(), "{name}");
        }

        let err = Compiler::new()
            .compile_source_with_manifest(&src)
            .expect_err("production should reject adversarial host payloads");
        assert!(err.contains("E_ACCESS_INCOMPLETE"));
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
    fn manifest_trigger_decl_preserves_namespaced_callback() {
        let src = r#"
seiyaku Test {
  kotoage fn arm() {}
  register_trigger wake {
    call callee::run;
    on time pre_commit;
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let arm = entrypoints
            .iter()
            .find(|entry| entry.name == "arm")
            .expect("arm entrypoint");
        assert_eq!(arm.triggers.len(), 1);
        let callback = &arm.triggers[0].callback;
        assert_eq!(callback.namespace.as_deref(), Some("callee"));
        assert_eq!(callback.entrypoint, "run");
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
    fn main_entrypoint_is_compiled_first_before_hajimari() {
        let src = r#"
seiyaku Hello {
  state int Counter;

  hajimari() {
    Counter = 1;
  }

  kotoage fn main() permission(Admin) {
    write_detail();
  }

  kotoage fn write_detail() permission(Admin) {
    Counter = Counter + 1;
  }
}
"#;
        let (bytes, manifest) = Compiler::new()
            .compile_source_with_manifest(src)
            .expect("compile manifest");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        assert_eq!(entrypoints.len(), 3);
        let main = entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entrypoint");
        assert_eq!(main.name, "main");

        let parsed = ivm_abi::metadata::ProgramMetadata::parse(&bytes).expect("parse metadata");
        let embedded = parsed
            .contract_interface
            .expect("embedded contract interface");
        let main_embedded = embedded
            .entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("embedded main entrypoint");
        assert_eq!(
            main_embedded.entry_pc, 0,
            "public `main` must be laid out first so raw VM startup enters `main` before `hajimari`"
        );
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

    let action = ev.get_name(action_key);
    if (action == name("create")) {
      let request_id = ev.get_name(request_id_key);
      let sequence = MintRequestNextSequence + 1;
      let fi_id = ev.get_name(fi_id_key);
      let to_account_id = ev.get_account_id(to_account_id_key);
      let amount_i64 = ev.get_int(amount_i64_key);
      let requested_by_actor_id = ev.get_json(requested_by_actor_id_key);
      let created_at_ms = ev.get_int(created_at_ms_key);
      let expires_at_ms = ev.get_int(expires_at_ms_key);
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
        let mut seen_copy_dests: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut multi_copy_dests: HashSet<(usize, ir::Temp)> = HashSet::new();
        for (func_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    if let ir::Instr::Copy { dest, .. } = instr {
                        let key = (func_idx, *dest);
                        if !seen_copy_dests.insert(key) {
                            multi_copy_dests.insert(key);
                        }
                    }
                }
            }
        }

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
                            if !multi_copy_dests.contains(&dest_key) {
                                if let Some(val) = string_map.get(&(func_idx, *src)).cloned() {
                                    string_map.insert(dest_key, val);
                                }
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *src)).copied()
                                {
                                    dataref_kind_map.insert(dest_key, kind);
                                }
                                if let Some(val) = int_const_map.get(&(func_idx, *src)).copied() {
                                    int_const_map.insert(dest_key, val);
                                }
                                if string_literal_temps.contains(&(func_idx, *src)) {
                                    string_literal_temps.insert(dest_key);
                                }
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
                    if let ir::Instr::ActorAccount { dest, .. } = instr {
                        dataref_kind_map.insert((func_idx, *dest), DRK::Account);
                    }
                    if let ir::Instr::ActorPublicKey { dest, .. }
                    | ir::Instr::ActorSign { dest, .. } = instr
                    {
                        dataref_kind_map.insert((func_idx, *dest), DRK::Blob);
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
            DomainId::try_new("wonderland", "universal").expect("domain"),
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
        let domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
        let asset_definition = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        );
        let asset = AssetId::new(asset_definition.clone(), account.clone());
        let asset_literal = asset.canonical_literal();
        let nft: NftId = "n0$wonderland.universal".parse().expect("nft");
        let rwa: RwaId = format!(
            "{}$wonderland.universal",
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
            pipeline::{BlockEventFilter, BlockStatus, PipelineEventFilterBox},
        };

        let src = r#"
seiyaku Test {
  kotoage fn run() {}
  register_trigger block_wake {
    call run;
    on pipeline block approved;
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
            EventFilterBox::Pipeline(PipelineEventFilterBox::Block(
                BlockEventFilter::new().for_status(BlockStatus::Approved),
            ))
        );
    }

    #[test]
    fn access_hint_diagnostics_report_isi_wildcards() {
        let src = r#"
seiyaku Test {
  kotoage fn register() permission(Admin) {
    register_peer(json("{}"));
  }
}
"#;
        let compiler = test_mode_compiler();
        let (_bytes, _manifest, diag) = compiler
            .compile_source_with_manifest_and_diagnostics(src)
            .expect("compile manifest");
        assert!(diag.isi_wildcards > 0);
        assert_eq!(diag.state_wildcards, 0);
    }

    #[test]
    fn access_hint_diagnostics_report_literal_trigger_spec_decode_failures() {
        let src = r#"
seiyaku Test {
  kotoage fn register() permission(Admin) {
    create_trigger(json("{\"name\":\"t1\"}"));
  }
}
"#;
        let compiler = test_mode_compiler();
        let (_bytes, manifest, diag) = compiler
            .compile_source_with_manifest_and_diagnostics(src)
            .expect("compile manifest");
        assert_eq!(diag.literal_trigger_spec_decode_failures, 1);
        assert_eq!(diag.isi_wildcards, 1);
        assert_eq!(diag.state_wildcards, 0);
        assert!(!diag.is_empty());

        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let register = entrypoints
            .iter()
            .find(|entry| entry.name == "register")
            .expect("register entrypoint");
        assert_eq!(register.access_hints_complete, Some(false));
        assert_eq!(
            register.access_hints_skipped,
            vec![HINT_SKIP_LITERAL_TRIGGER_SPEC_DECODE.to_string()]
        );
    }

    #[test]
    fn production_rejects_incomplete_access_hints() {
        let src = r#"
seiyaku Test {
  kotoage fn register() permission(Admin) {
    register_peer(json("{}"));
  }
}
"#;
        let compiler = Compiler::new();
        let err = compiler
            .compile_source_with_manifest(src)
            .expect_err("production should reject incomplete access metadata");
        assert!(err.contains("E_ACCESS_INCOMPLETE"));
    }

    #[test]
    fn production_rejects_literal_trigger_spec_decode_failures_with_hint() {
        let src = r#"
seiyaku Test {
  kotoage fn register() permission(Admin) {
    create_trigger(json("{\"name\":\"t1\"}"));
  }
}
"#;
        let compiler = Compiler::new();
        let err = compiler
            .compile_source_with_manifest(src)
            .expect_err("production should reject undecodable literal trigger specs");
        assert!(err.contains("E_ACCESS_INCOMPLETE"));
        assert!(err.contains(HINT_SKIP_LITERAL_TRIGGER_SPEC_DECODE));
    }

    #[test]
    fn production_accepts_call_contract_access_fallback() {
        let src = r#"
seiyaku Test {
  kotoage fn relay(target: bytes, payload: Json) -> bytes permission(Admin) {
    return call_contract(target, "settle", payload);
  }
}
"#;
        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(src)
            .expect("call_contract fallback should be deployable in production");
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let relay = entrypoints
            .iter()
            .find(|entry| entry.name == "relay")
            .expect("relay entrypoint");
        assert_taira_supported_access_keys(&relay.read_keys);
        assert_taira_supported_access_keys(&relay.write_keys);
        assert_eq!(relay.access_hints_complete, Some(false));
        assert_eq!(
            relay.access_hints_skipped,
            vec![HINT_SKIP_CONTRACT_CALL_TARGET.to_string()]
        );
    }

    #[test]
    fn production_accepts_dynamic_asset_definition_transfer_with_stripped_coarse_hints() {
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
            .expect("dynamic asset transfers should use coarse asset access hints");
        assert!(
            manifest.access_set_hints.is_none(),
            "production should omit coarse wildcard-only access hints"
        );
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let move_entry = entrypoints
            .iter()
            .find(|entry| entry.name == "move")
            .expect("move entrypoint");
        assert_taira_supported_access_keys(&move_entry.read_keys);
        assert_taira_supported_access_keys(&move_entry.write_keys);
        assert_eq!(move_entry.access_hints_complete, Some(true));
        assert!(move_entry.access_hints_skipped.is_empty());
    }

    #[test]
    fn production_accepts_fixed_asset_dynamic_account_transfer_hints() {
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"
seiyaku Test {{
  kotoage fn move(from: AccountId, to: AccountId, amount: int) permission(Admin) {{
    transfer_asset(from, to, asset_definition("{asset_literal}"), amount);
  }}
}}
"#
        );

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("fixed asset dynamic accounts should have bounded access hints");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(
            !hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()),
            "fixed asset transfers should not require global read wildcards"
        );
        assert!(
            !hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()),
            "fixed asset transfers should not require global write wildcards"
        );
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);
        assert!(
            hints
                .read_keys
                .contains(&format!("asset_def:{asset_literal}"))
        );
        assert!(
            hints
                .write_keys
                .contains(&format!("asset_def:{asset_literal}"))
        );
        let entrypoints = manifest.entrypoints.expect("entrypoints present");
        let move_entry = entrypoints
            .iter()
            .find(|entry| entry.name == "move")
            .expect("move entrypoint");
        assert_eq!(move_entry.access_hints_complete, Some(true));
        assert!(move_entry.access_hints_skipped.is_empty());
    }

    #[test]
    fn production_propagates_asset_definition_helper_return_into_access_hints() {
        let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
        let src = format!(
            r#"
seiyaku Test {{
  fn settlement_asset() -> AssetDefinitionId {{
    let asset = asset_definition("{asset_literal}");
    return asset;
  }}

  kotoage fn move(from: AccountId, to: AccountId, amount: int) permission(Admin) {{
    let asset = settlement_asset();
    transfer_asset(from, to, asset, amount);
  }}
}}
"#
        );

        let compiler = Compiler::new();
        let (_bytes, manifest) = compiler
            .compile_source_with_manifest(&src)
            .expect("literal asset helper return should feed access hints");
        let hints = manifest
            .access_set_hints
            .expect("expected access_set_hints");
        assert!(!hints.read_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert!(!hints.write_keys.contains(&GLOBAL_WILDCARD_KEY.to_string()));
        assert_taira_supported_access_keys(&hints.read_keys);
        assert_taira_supported_access_keys(&hints.write_keys);
        assert!(
            hints
                .read_keys
                .contains(&format!("asset_def:{asset_literal}"))
        );
        assert!(
            hints
                .write_keys
                .contains(&format!("asset_def:{asset_literal}"))
        );
    }

    #[test]
    fn explicit_global_wildcards_are_rejected() {
        let src = r#"
seiyaku Test {
  #[access(read="*", write="*")]
  kotoage fn move(from: AccountId, to: AccountId, asset: AssetDefinitionId, amount: int) permission(Admin) {
    transfer_asset(from, to, asset, amount);
  }
}
"#;
        let compiler = Compiler::new();
        let err = compiler
            .compile_source_with_manifest(src)
            .expect_err("manual access hints should be rejected");
        assert!(err.contains("access metadata is generated by the compiler"));
    }

    #[test]
    fn manifest_access_set_hints_rejects_explicit_access() {
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
        let err = compiler
            .compile_source_with_manifest(&src)
            .expect_err("manual access hints should be rejected");
        assert!(err.contains("access metadata is generated by the compiler"));
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
    fn manifest_access_set_hints_reject_numeric_map_keys() {
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
        let err = compiler
            .compile_source_with_manifest(src)
            .expect_err("numeric map keys are not supported for durable state");
        assert!(err.contains("state Map key type `Amount` is not supported"));
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

#[cfg(test)]
mod test_mode_tests {
    use super::*;

    #[test]
    fn production_mode_strips_test_functions_from_debug_report() {
        let src = r#"
        fn helper() {}

        #[test]
        fn smoke() {}
        "#;

        let production = Compiler::new_with_options(CompilerOptions::default());
        let (_code, manifest, report) = production
            .compile_source_with_manifest_and_report(src)
            .expect("compile in production mode");
        assert!(
            report
                .source_map
                .iter()
                .all(|entry| entry.function_name != "smoke")
        );
        assert!(
            manifest
                .entrypoints
                .as_ref()
                .is_none_or(|entrypoints| entrypoints.iter().all(|entry| entry.name != "smoke"))
        );

        let test_mode = Compiler::new_with_options(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        });
        let (_code, _manifest, report) = test_mode
            .compile_source_with_manifest_and_report(src)
            .expect("compile in test mode");
        assert!(
            report
                .source_map
                .iter()
                .any(|entry| entry.function_name == "smoke")
        );
    }

    #[test]
    fn production_prelude_scan_ignores_stripped_tests() {
        let test_only_call_src = r#"
        seiyaku Demo {
            kotoage fn run() permission(Admin) {
                info("run");
            }

            #[test]
            fn smoke() {
                let expected: AccountId = authority();
                require_authority(expected);
            }
        }
        "#;
        let shadowed_helper_src = r#"
        seiyaku Demo {
            kotoage fn run(owner: AccountId) permission(Admin) {
                require_owner(owner);
            }

            #[test]
            fn require_authority() {}
        }
        "#;

        let production = Compiler::new_with_options(CompilerOptions::default());
        let (_code, _manifest, report) = production
            .compile_source_with_manifest_and_report(test_only_call_src)
            .expect("compile production with test-only prelude call");
        assert!(
            report
                .source_map
                .iter()
                .all(|entry| entry.function_name != "require_authority")
        );

        let (_code, _manifest, report) = production
            .compile_source_with_manifest_and_report(shadowed_helper_src)
            .expect("compile production with test shadowing prelude helper");
        assert!(
            report
                .source_map
                .iter()
                .any(|entry| entry.function_name == "require_authority")
        );
        assert!(
            report
                .source_map
                .iter()
                .any(|entry| entry.function_name == "require_owner")
        );
    }

    #[test]
    fn test_mode_helpers_emit_private_scallx_syscalls() {
        let src = r#"
        seiyaku Demo {
            kotoage fn run() {}

            #[test]
            fn smoke() {
                let _acct = actor_account("issuer");
                let _pk = actor_public_key("issuer");
                let _sig = actor_sign("issuer", b"message");
                invoke_entrypoint_as("issuer", "run", json("{}"));
                expect_reject_as("issuer", "run", json("{}"));
            }
        }
        "#;

        let compiler = Compiler::new_with_options(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        });
        let code = compiler.compile_source(src).expect("compile test helpers");
        let metadata = ProgramMetadata::parse(&code).expect("parse metadata");
        let code_region = &code[metadata.code_offset..];

        for syscall in [
            syscalls::SYSCALL_KOTO_TEST_ACTOR_ACCOUNT,
            syscalls::SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY,
            syscalls::SYSCALL_KOTO_TEST_ACTOR_SIGN,
            syscalls::SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS,
            syscalls::SYSCALL_KOTO_TEST_EXPECT_REJECT_AS,
        ] {
            let needle = encoding::wide::encode_syscallx(syscall).to_le_bytes();
            assert!(
                code_region
                    .windows(needle.len())
                    .any(|window| window == needle),
                "expected private Kotodama test syscall {syscall:#x} to use SCALLX"
            );
        }
    }

    #[test]
    fn first_release_prelude_helpers_are_available_without_imports() {
        let src = r#"
        seiyaku Demo {
            kotoage fn fee_quote() -> int {
                return checked_sub_amount(checked_add_amount(bps_fee(10000, 25), 10), 5);
            }
        }
        "#;

        let (_code, manifest) = Compiler::new()
            .compile_source_with_manifest(src)
            .expect("compile prelude helper use");
        assert!(
            manifest
                .entrypoints
                .as_ref()
                .is_some_and(|entrypoints| entrypoints
                    .iter()
                    .any(|entry| entry.name == "fee_quote"))
        );
    }

    #[test]
    fn manifest_state_descriptors_use_canonical_type_names() {
        let src = r#"
        seiyaku Demo {
            state int Counter;
            state Prices: Map<Name, int>;

            view fn get_counter() -> int {
                return Counter;
            }
        }
        "#;

        let (_code, manifest) = Compiler::new()
            .compile_source_with_manifest(src)
            .expect("compile state schema");
        let states = manifest.states.expect("state schema");
        assert!(
            states
                .iter()
                .any(|state| state.name == "Counter" && state.type_name == "int")
        );
        assert!(
            states
                .iter()
                .any(|state| state.name == "Prices" && state.type_name == "map<Name, int>")
        );
    }
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
        let mode_program = match self.opts.mode {
            CompilerMode::Production => program.stripped_for_production(),
            CompilerMode::Test => program.clone(),
        };
        let compiled_program = program_with_first_release_prelude(&mode_program)?;
        let typed = semantic::analyze(&compiled_program)
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
        if self.opts.dynamic_iter_cap != semantic::DYNAMIC_ITERATION_LIMIT as u8 {
            return Err(format!(
                "unsupported dynamic_iter_cap {}; first-release Kotodama uses fixed dynamic iteration limit {}",
                self.opts.dynamic_iter_cap,
                semantic::DYNAMIC_ITERATION_LIMIT
            ));
        }
        let ir_prog = ir::lower_with_cap_and_test_mode(
            &typed,
            self.opts.dynamic_iter_cap as usize,
            self.opts.mode == CompilerMode::Test,
        )?;
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
        let mut instruction_literal_access_map: HashMap<(usize, ir::Temp), AccessSets> =
            HashMap::new();
        let mut seen_copy_dests: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut multi_copy_dests: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut authority_account_temps: HashSet<(usize, ir::Temp)> = HashSet::new();
        for (func_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    if let ir::Instr::Copy { dest, .. } = instr {
                        let key = (func_idx, *dest);
                        if !seen_copy_dests.insert(key) {
                            multi_copy_dests.insert(key);
                        }
                    }
                }
            }
        }
        let func_count = ir_prog.functions.len();
        let mut access_sets: Vec<AccessSets> = vec![AccessSets::default(); func_count];
        let mut hint_skips: Vec<IndexSet<String>> = vec![IndexSet::new(); func_count];
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
                        authority_account_temps.remove(&(func_idx, *dest));
                        instruction_literal_access_map.remove(&(func_idx, *dest));
                    }
                    if let ir::Instr::Copy { dest, src } = instr {
                        if dest != src {
                            let dest_key = (func_idx, *dest);
                            string_map.remove(&dest_key);
                            dataref_kind_map.remove(&dest_key);
                            state_path_hints.remove(&dest_key);
                            int_const_map.remove(&dest_key);
                            norito_literal_map.remove(&dest_key);
                            instruction_literal_access_map.remove(&dest_key);
                            string_literal_temps.remove(&dest_key);
                            authority_account_temps.remove(&dest_key);
                            if !multi_copy_dests.contains(&dest_key) {
                                if let Some(val) = string_map.get(&(func_idx, *src)).cloned() {
                                    string_map.insert(dest_key, val);
                                }
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *src)).copied()
                                {
                                    dataref_kind_map.insert(dest_key, kind);
                                }
                                if let Some(hint) = state_path_hints.get(&(func_idx, *src)).cloned()
                                {
                                    state_path_hints.insert(dest_key, hint);
                                }
                                if let Some(val) = int_const_map.get(&(func_idx, *src)).copied() {
                                    int_const_map.insert(dest_key, val);
                                }
                                if let Some(val) =
                                    norito_literal_map.get(&(func_idx, *src)).cloned()
                                {
                                    norito_literal_map.insert(dest_key, val);
                                }
                                if let Some(access) = instruction_literal_access_map
                                    .get(&(func_idx, *src))
                                    .cloned()
                                {
                                    instruction_literal_access_map.insert(dest_key, access);
                                }
                                if string_literal_temps.contains(&(func_idx, *src)) {
                                    string_literal_temps.insert(dest_key);
                                }
                                if authority_account_temps.contains(&(func_idx, *src)) {
                                    authority_account_temps.insert(dest_key);
                                }
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
                    if let ir::Instr::BuildSubmitBallotInline {
                        dest,
                        election_id,
                        ciphertext,
                        nullifier,
                        backend,
                        proof,
                        vk,
                    } = instr
                        && let Some(raw) = submit_ballot_inline_instruction_literal(
                            &string_map,
                            func_idx,
                            *election_id,
                            *ciphertext,
                            *nullifier,
                            *backend,
                            *proof,
                            *vk,
                        )
                    {
                        if let Some(access) = access_for_instruction_literal(&raw) {
                            instruction_literal_access_map.insert((func_idx, *dest), access);
                        }
                        string_map.insert((func_idx, *dest), raw);
                        dataref_kind_map.insert((func_idx, *dest), DRK::NoritoBytes);
                    }
                    if let ir::Instr::BuildUnshieldInline {
                        dest,
                        asset,
                        to,
                        amount,
                        inputs,
                        outputs,
                        backend,
                        proof,
                        vk,
                    } = instr
                        && let Some(raw) = unshield_inline_instruction_literal(
                            &string_map,
                            &int_const_map,
                            func_idx,
                            *asset,
                            *to,
                            *amount,
                            *inputs,
                            *outputs,
                            *backend,
                            *proof,
                            *vk,
                        )
                    {
                        if let Some(access) = access_for_instruction_literal(&raw) {
                            instruction_literal_access_map.insert((func_idx, *dest), access);
                        }
                        string_map.insert((func_idx, *dest), raw);
                        dataref_kind_map.insert((func_idx, *dest), DRK::NoritoBytes);
                    }
                    if let ir::Instr::ActorAccount { dest, .. } = instr {
                        dataref_kind_map.insert((func_idx, *dest), DRK::Account);
                    }
                    if let ir::Instr::GetAuthority { dest } | ir::Instr::SysvarAuthority { dest } =
                        instr
                    {
                        dataref_kind_map.insert((func_idx, *dest), DRK::Account);
                        authority_account_temps.insert((func_idx, *dest));
                    }
                    if let ir::Instr::ActorPublicKey { dest, .. }
                    | ir::Instr::ActorSign { dest, .. } = instr
                    {
                        dataref_kind_map.insert((func_idx, *dest), DRK::Blob);
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
                }
            }
        }

        propagate_function_return_literal_facts(
            &ir_prog,
            &mut string_map,
            &mut dataref_kind_map,
            &mut string_literal_temps,
            &multi_copy_dests,
        );

        // Propagate string literals across call boundaries so callee parameters inherit literal metadata
        // only when every call site agrees on the same literal value.
        let mut fn_index_by_name: HashMap<&str, usize> = HashMap::new();
        for (idx, func) in ir_prog.functions.iter().enumerate() {
            fn_index_by_name.insert(&func.name, idx);
        }
        let mut literal_param_conflicts: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut authority_param_conflicts: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut instruction_param_unknowns: HashSet<(usize, ir::Temp)> = HashSet::new();
        let mut state_path_param_unknowns: HashSet<(usize, ir::Temp)> = HashSet::new();
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
                            if !state_path_param_unknowns.contains(&param_key) {
                                if let Some(hint) =
                                    state_path_hints.get(&(caller_idx, arg_temp)).cloned()
                                {
                                    match state_path_hints.get(&param_key) {
                                        Some(existing) if existing != &hint => {
                                            state_path_hints.remove(&param_key);
                                            state_path_param_unknowns.insert(param_key);
                                        }
                                        Some(_) => {}
                                        None => {
                                            state_path_hints.insert(param_key, hint);
                                        }
                                    }
                                } else if state_path_hints.remove(&param_key).is_some() {
                                    state_path_param_unknowns.insert(param_key);
                                }
                            }
                            if !instruction_param_unknowns.contains(&param_key) {
                                if let Some(access) = instruction_literal_access_map
                                    .get(&(caller_idx, arg_temp))
                                    .cloned()
                                {
                                    instruction_literal_access_map
                                        .entry(param_key)
                                        .or_default()
                                        .union_with(&access);
                                } else if instruction_literal_access_map
                                    .remove(&param_key)
                                    .is_some()
                                {
                                    instruction_param_unknowns.insert(param_key);
                                }
                            }
                            let arg_has_authority =
                                authority_account_temps.contains(&(caller_idx, arg_temp));
                            if !authority_param_conflicts.contains(&param_key) {
                                if arg_has_authority {
                                    if string_map.contains_key(&param_key) {
                                        string_map.remove(&param_key);
                                        string_literal_temps.remove(&param_key);
                                        dataref_kind_map.remove(&param_key);
                                        authority_account_temps.remove(&param_key);
                                        authority_param_conflicts.insert(param_key);
                                    } else {
                                        authority_account_temps.insert(param_key);
                                        dataref_kind_map.insert(param_key, DRK::Account);
                                    }
                                } else if authority_account_temps.remove(&param_key) {
                                    authority_param_conflicts.insert(param_key);
                                }
                            }
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

        propagate_function_return_literal_facts(
            &ir_prog,
            &mut string_map,
            &mut dataref_kind_map,
            &mut string_literal_temps,
            &multi_copy_dests,
        );

        for (func_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
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
                }
            }
        }

        derive_state_access_hints(
            &ir_prog,
            &state_path_hints,
            &mut access_sets,
            &mut hint_diagnostics,
            &mut hint_skips,
        );

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
                            ir::DataRefKind::SoracloudRequest => "soracloud_request",
                            ir::DataRefKind::SoracloudResponse => "soracloud_response",
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
                &int_const_map,
                &authority_account_temps,
                &dataref_kind_map,
                &instruction_literal_access_map,
                &mut access_sets,
                &mut hint_diagnostics,
                &mut hint_skips,
            );
        }
        let has_any_hints = access_sets
            .iter()
            .any(|set| !set.reads.is_empty() || !set.writes.is_empty());
        let include_hints = has_any_hints;
        let mut hint_reports = Vec::with_capacity(func_count);
        for skips in hint_skips.iter().take(func_count) {
            let skipped_reasons = skips.iter().cloned().collect::<Vec<_>>();
            hint_reports.push(HintReport {
                emitted: include_hints,
                complete: include_hints && skipped_reasons.is_empty(),
                skipped_reasons,
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
        let mut entrypoint_wrapper_offsets: HashMap<String, usize> = HashMap::new();
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
                                    "too many function parameters: {} > {} (argument `{}` exceeds the ABI v1 register argument limit)",
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
                        Instr::AddSignatory { account, signatory }
                        | Instr::RemoveSignatory { account, signatory } => {
                            if let Some(acc_str) = string_map.get(&(func_idx, *account)) {
                                let key_acc = DataKey(DataKind::Account, acc_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_acc);
                            } else {
                                let r = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(json) = string_map.get(&(func_idx, *signatory)) {
                                let key_json = DataKey(DataKind::Json, json.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key_json);
                            } else {
                                let r = src_reg(signatory, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            let syscall = match instr {
                                Instr::AddSignatory { .. } => syscalls::SYSCALL_ADD_SIGNATORY,
                                _ => syscalls::SYSCALL_REMOVE_SIGNATORY,
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscall as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::SetAccountQuorum { account, quorum } => {
                            if let Some(acc_str) = string_map.get(&(func_idx, *account)) {
                                let key_acc = DataKey(DataKind::Account, acc_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_acc);
                            } else {
                                let r = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let r_quorum = src_reg(quorum, scratch2, &mut code)?;
                            push_word(&mut code, encode_addi(11, r_quorum, 0)?);
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_SET_ACCOUNT_QUORUM as u8,
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
                        Instr::ProveExecution { dest } => {
                            push_syscall(&mut code, syscalls::SYSCALL_PROVE_EXECUTION);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Alloc { dest, bytes } => {
                            let r = src_reg(bytes, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r, 0)?);
                            push_syscall(&mut code, syscalls::SYSCALL_ALLOC);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GrowHeap { dest, bytes } => {
                            let r = src_reg(bytes, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r, 0)?);
                            push_syscall(&mut code, syscalls::SYSCALL_GROW_HEAP);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GetMerklePath {
                            dest,
                            address,
                            output,
                            root_output,
                        } => {
                            let address_reg = src_reg(address, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, address_reg, 0)?);
                            let output_reg = src_reg(output, scratch2, &mut code)?;
                            push_word(&mut code, encode_addi(11, output_reg, 0)?);
                            if let Some(root_output) = root_output {
                                let root_reg = src_reg(root_output, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, root_reg, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(12, 0, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_GET_MERKLE_PATH);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GetMerkleCompact {
                            dest,
                            address,
                            output,
                            max_depth,
                            root_output,
                        } => {
                            let address_reg = src_reg(address, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, address_reg, 0)?);
                            let output_reg = src_reg(output, scratch2, &mut code)?;
                            push_word(&mut code, encode_addi(11, output_reg, 0)?);
                            if let Some(max_depth) = max_depth {
                                let depth_reg = src_reg(max_depth, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, depth_reg, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(12, 0, 0)?);
                            }
                            if let Some(root_output) = root_output {
                                let root_reg = src_reg(root_output, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(13, root_reg, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(13, 0, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_GET_MERKLE_COMPACT);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GetRegisterMerkleCompact {
                            dest,
                            register_index,
                            output,
                            max_depth,
                            root_output,
                        } => {
                            let index_reg = src_reg(register_index, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, index_reg, 0)?);
                            let output_reg = src_reg(output, scratch2, &mut code)?;
                            push_word(&mut code, encode_addi(11, output_reg, 0)?);
                            if let Some(max_depth) = max_depth {
                                let depth_reg = src_reg(max_depth, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, depth_reg, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(12, 0, 0)?);
                            }
                            if let Some(root_output) = root_output {
                                let root_reg = src_reg(root_output, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(13, root_reg, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(13, 0, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::VerifyProof { dest, payload } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, syscalls::SYSCALL_VERIFY_PROOF);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
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
                        Instr::QueryExecuteNorito { dest, payload } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, syscalls::SYSCALL_QUERY_EXECUTE_NORITO);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::QueryGet { dest, key, syscall } => {
                            let r = src_reg(key, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r, 0)?);
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, *syscall);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GetAccountBalance {
                            dest,
                            account,
                            asset,
                        } => {
                            if let Some(account_str) = string_map.get(&(func_idx, *account)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Account, account_str.clone()),
                                );
                            } else {
                                let r = src_reg(account, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(asset_str) = string_map.get(&(func_idx, *asset)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    11,
                                    DataKey(DataKind::AssetDef, asset_str.clone()),
                                );
                            } else {
                                let r = src_reg(asset, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            push_syscall(&mut code, syscalls::SYSCALL_GET_ACCOUNT_BALANCE);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GetPublicInput { dest, key } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            if dataref_kind_map.get(&(func_idx, *key))
                                == Some(&ir::DataRefKind::Name)
                                && let Some(raw_key) = string_map.get(&(func_idx, *key))
                            {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Name, raw_key.clone()),
                                );
                            } else {
                                let r = src_reg(key, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, syscalls::SYSCALL_GET_PUBLIC_INPUT);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::GetPrivateInput { dest, index } => {
                            let r = src_reg(index, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r, 0)?);
                            push_syscall(&mut code, syscalls::SYSCALL_GET_PRIVATE_INPUT);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::UseNullifier { nullifier } => {
                            let r = src_reg(nullifier, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r, 0)?);
                            push_syscall(&mut code, syscalls::SYSCALL_USE_NULLIFIER);
                        }
                        Instr::CommitOutput => {
                            push_syscall(&mut code, syscalls::SYSCALL_COMMIT_OUTPUT);
                        }
                        Instr::SmartContractLifecycle { payload, syscall } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, *syscall);
                        }
                        Instr::TransferBatchApply { payload } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY);
                        }
                        Instr::ZkRootsGet { dest, payload }
                        | Instr::ZkVoteGetTally { dest, payload }
                        | Instr::VrfEpochSeed { dest, payload } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *payload)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let syscall = match instr {
                                Instr::ZkRootsGet { .. } => syscalls::SYSCALL_ZK_ROOTS_GET,
                                Instr::ZkVoteGetTally { .. } => syscalls::SYSCALL_ZK_VOTE_GET_TALLY,
                                Instr::VrfEpochSeed { .. } => syscalls::SYSCALL_VRF_EPOCH_SEED,
                                _ => unreachable!(),
                            };
                            push_syscall(&mut code, syscall);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::SoracloudHostCall {
                            dest,
                            request,
                            syscall,
                        } => {
                            let r = src_reg(request, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r, 0)?);
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, *syscall);
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
                                proof::{ProofAttachment, ProofBox, VerifyingKeyId},
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
                            let vk_ref = require_literal("vk_ref", vk)?;
                            let pa = ProofAttachment::new_ref(
                                backend_str.clone(),
                                ProofBox::new(backend_str.clone(), proof_bytes),
                                VerifyingKeyId::new(backend_str, vk_ref),
                            );
                            let sb = DMZk::SubmitBallot {
                                election_id: eid,
                                ciphertext: ct_bytes,
                                ballot_proof: pa,
                                nullifier: null32,
                            };
                            let bytes =
                                norito::to_bytes(&InstructionBox::from(sb)).map_err(|e| {
                                    let err = format!(
                                        "build_submit_ballot_inline encode InstructionBox: {e}"
                                    );
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
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
                            outputs,
                            backend,
                            proof,
                            vk,
                        } => {
                            use iroha_data_model::{
                                isi::zk as DMZk,
                                prelude::*,
                                proof::{ProofAttachment, ProofBox, VerifyingKeyId},
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
                            let inputs_literal = require_literal("inputs", inputs)?;
                            let ins = decode_fixed32_chunks(&inputs_literal, "inputs", false)
                                .map_err(|e| {
                                    let err = format!("build_unshield_inline {e}");
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
                            let outs = if let Some(outputs) = outputs {
                                let outputs_literal = require_literal("outputs", outputs)?;
                                decode_fixed32_chunks(&outputs_literal, "outputs", true).map_err(
                                    |e| {
                                        let err = format!("build_unshield_inline {e}");
                                        i18n::translate(self.lang, Message::SemanticError(&err))
                                    },
                                )?
                            } else {
                                Vec::new()
                            };
                            let backend_str = require_literal("backend", backend)?;
                            let proof_literal = require_literal("proof", proof)?;
                            let proof_bytes =
                                decode_hex_or_raw_bytes(&proof_literal).map_err(|e| {
                                    let err = format!("build_unshield_inline proof literal {e}");
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
                            let vk_ref = require_literal("vk_ref", vk)?;
                            let pa = ProofAttachment::new_ref(
                                backend_str.clone(),
                                ProofBox::new(backend_str.clone(), proof_bytes),
                                VerifyingKeyId::new(backend_str, vk_ref),
                            );
                            let uz = DMZk::Unshield {
                                asset: ad,
                                to: acct,
                                public_amount: amt as u128,
                                inputs: ins,
                                outputs: outs,
                                proof: pa,
                                root_hint: None,
                            };
                            let bytes =
                                norito::to_bytes(&InstructionBox::from(uz)).map_err(|e| {
                                    let err =
                                        format!("build_unshield_inline encode InstructionBox: {e}");
                                    i18n::translate(self.lang, Message::SemanticError(&err))
                                })?;
                            let hex_payload = hex::encode(bytes);
                            let key = DataKey(DataKind::NoritoBytes, hex_payload);
                            let (rd, spilled, imm) = dst_reg(dest);
                            emit_literal_stub(&mut code, &mut fixups, rd, key);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::RegisterAsset {
                            asset,
                            symbol,
                            quantity,
                            mintable,
                        } => {
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            if let Some(asset_str) = string_map.get(&(func_idx, *asset)) {
                                let key_asset = DataKey(DataKind::AssetDef, asset_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_asset);
                            } else {
                                let r_asset = src_reg(asset, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_asset, 0)?);
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
                            asset,
                            symbol,
                            quantity,
                            account,
                            mintable,
                        } => {
                            let r_symbol = src_reg(symbol, scratch2, &mut code)?;
                            let r_qty = src_reg(quantity, scratchd, &mut code)?;
                            let r_mint = src_reg(mintable, scratch1, &mut code)?;
                            if let Some(asset_str) = string_map.get(&(func_idx, *asset)) {
                                let key_asset = DataKey(DataKind::AssetDef, asset_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key_asset);
                            } else {
                                let r_asset = src_reg(asset, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_asset, 0)?);
                            }
                            push_word(&mut code, encode_addi(11, r_symbol, 0)?);
                            push_word(&mut code, encode_addi(12, r_qty, 0)?);
                            push_word(&mut code, encode_addi(13, r_mint, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_REGISTER_ASSET as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let r_acc = src_reg(account, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r_acc, 0)?);
                            if let Some(asset_str) = string_map.get(&(func_idx, *asset)) {
                                let key_asset = DataKey(DataKind::AssetDef, asset_str.clone());
                                emit_literal_stub(&mut code, &mut fixups, 11, key_asset);
                            } else {
                                let r_asset = src_reg(asset, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r_asset, 0)?);
                            }
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
                        Instr::EscrowOpenOffer {
                            escrow,
                            asset,
                            amount,
                            evidence_hashes,
                        } => {
                            let r_amount = src_reg(amount, scratch1, &mut code)?;
                            if let Some(escrow_str) = string_map
                                .get(&(func_idx, *escrow))
                                .map(|s| DataKey(DataKind::Name, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, escrow_str);
                            } else {
                                let r_escrow = src_reg(escrow, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_escrow, 0)?);
                            }
                            if let Some(asset_str) = string_map
                                .get(&(func_idx, *asset))
                                .map(|s| DataKey(DataKind::AssetDef, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 11, asset_str);
                            } else {
                                let r_asset = src_reg(asset, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r_asset, 0)?);
                            }
                            push_word(&mut code, encode_addi(12, r_amount, 0)?);
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(14, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            if let Some(evidence_hashes) = evidence_hashes {
                                let r_evidence = src_reg(evidence_hashes, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_evidence, 0)?);
                                code.extend_from_slice(&pub_word.to_le_bytes());
                                push_word(&mut code, encode_addi(13, 10, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(13, 0, 0)?);
                            }
                            push_word(&mut code, encode_addi(10, 14, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ESCROW_OPEN_OFFER as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::EscrowAccept { escrow }
                        | Instr::EscrowMarkPaymentSent { escrow }
                        | Instr::EscrowRelease { escrow }
                        | Instr::EscrowCancel { escrow } => {
                            if let Some(escrow_str) = string_map
                                .get(&(func_idx, *escrow))
                                .map(|s| DataKey(DataKind::Name, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, escrow_str);
                            } else {
                                let r_escrow = src_reg(escrow, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_escrow, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let syscall = match instr {
                                Instr::EscrowAccept { .. } => syscalls::SYSCALL_ESCROW_ACCEPT,
                                Instr::EscrowMarkPaymentSent { .. } => {
                                    syscalls::SYSCALL_ESCROW_MARK_PAYMENT_SENT
                                }
                                Instr::EscrowRelease { .. } => syscalls::SYSCALL_ESCROW_RELEASE,
                                Instr::EscrowCancel { .. } => syscalls::SYSCALL_ESCROW_CANCEL,
                                _ => unreachable!(),
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscall as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::EscrowOpenDispute {
                            escrow,
                            evidence_hashes,
                        } => {
                            if let Some(escrow_str) = string_map
                                .get(&(func_idx, *escrow))
                                .map(|s| DataKey(DataKind::Name, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, escrow_str);
                            } else {
                                let r_escrow = src_reg(escrow, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_escrow, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(evidence_hashes) = evidence_hashes {
                                push_word(&mut code, encode_addi(14, 10, 0)?);
                                let r_evidence = src_reg(evidence_hashes, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_evidence, 0)?);
                                code.extend_from_slice(&pub_word.to_le_bytes());
                                push_word(&mut code, encode_addi(11, 10, 0)?);
                                push_word(&mut code, encode_addi(10, 14, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(11, 0, 0)?);
                            }
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ESCROW_OPEN_DISPUTE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::EscrowResolveDispute {
                            escrow,
                            buyer_amount,
                            seller_amount,
                            evidence_hashes,
                        } => {
                            let r_buyer = src_reg(buyer_amount, scratch1, &mut code)?;
                            let r_seller = src_reg(seller_amount, scratch2, &mut code)?;
                            push_word(&mut code, encode_addi(11, r_buyer, 0)?);
                            push_word(&mut code, encode_addi(12, r_seller, 0)?);
                            if let Some(escrow_str) = string_map
                                .get(&(func_idx, *escrow))
                                .map(|s| DataKey(DataKind::Name, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, escrow_str);
                            } else {
                                let r_escrow = src_reg(escrow, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_escrow, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(14, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            if let Some(evidence_hashes) = evidence_hashes {
                                let r_evidence = src_reg(evidence_hashes, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_evidence, 0)?);
                                code.extend_from_slice(&pub_word.to_le_bytes());
                                push_word(&mut code, encode_addi(13, 10, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(13, 0, 0)?);
                            }
                            push_word(&mut code, encode_addi(10, 14, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ESCROW_RESOLVE_DISPUTE as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::AnonymousEscrowOpenOffer { request }
                        | Instr::AnonymousEscrowRelease { request }
                        | Instr::AnonymousEscrowCancel { request }
                        | Instr::AnonymousEscrowResolveDispute { request } => {
                            if let Some(pstr) = string_map.get(&(func_idx, *request)) {
                                let key = DataKey(DataKind::NoritoBytes, pstr.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r_request = src_reg(request, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_request, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let syscall = match instr {
                                Instr::AnonymousEscrowOpenOffer { .. } => {
                                    syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER
                                }
                                Instr::AnonymousEscrowRelease { .. } => {
                                    syscalls::SYSCALL_ANONYMOUS_ESCROW_RELEASE
                                }
                                Instr::AnonymousEscrowCancel { .. } => {
                                    syscalls::SYSCALL_ANONYMOUS_ESCROW_CANCEL
                                }
                                Instr::AnonymousEscrowResolveDispute { .. } => {
                                    syscalls::SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE
                                }
                                _ => unreachable!(),
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscall as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::AnonymousEscrowAccept { escrow }
                        | Instr::AnonymousEscrowMarkPaymentSent { escrow } => {
                            if let Some(escrow_str) = string_map
                                .get(&(func_idx, *escrow))
                                .map(|s| DataKey(DataKind::Name, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, escrow_str);
                            } else {
                                let r_escrow = src_reg(escrow, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_escrow, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let syscall = match instr {
                                Instr::AnonymousEscrowAccept { .. } => {
                                    syscalls::SYSCALL_ANONYMOUS_ESCROW_ACCEPT
                                }
                                Instr::AnonymousEscrowMarkPaymentSent { .. } => {
                                    syscalls::SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT
                                }
                                _ => unreachable!(),
                            };
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscall as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::AnonymousEscrowOpenDispute {
                            escrow,
                            evidence_hashes,
                        } => {
                            if let Some(escrow_str) = string_map
                                .get(&(func_idx, *escrow))
                                .map(|s| DataKey(DataKind::Name, s.clone()))
                            {
                                emit_literal_stub(&mut code, &mut fixups, 10, escrow_str);
                            } else {
                                let r_escrow = src_reg(escrow, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_escrow, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            if let Some(evidence_hashes) = evidence_hashes {
                                push_word(&mut code, encode_addi(14, 10, 0)?);
                                let r_evidence = src_reg(evidence_hashes, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_evidence, 0)?);
                                code.extend_from_slice(&pub_word.to_le_bytes());
                                push_word(&mut code, encode_addi(11, 10, 0)?);
                                push_word(&mut code, encode_addi(10, 14, 0)?);
                            } else {
                                push_word(&mut code, encode_addi(11, 0, 0)?);
                            }
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE as u8,
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
                        Instr::SetNftData { nft, key, json } => {
                            // Load literals or move regs
                            let k_nft = string_map
                                .get(&(func_idx, *nft))
                                .map(|s| DataKey(DataKind::NftId, s.clone()));
                            let k_key = string_map
                                .get(&(func_idx, *key))
                                .map(|s| DataKey(DataKind::Name, s.clone()));
                            let k_json = string_map
                                .get(&(func_idx, *json))
                                .map(|s| DataKey(DataKind::Json, s.clone()));
                            if let Some(kn) = k_nft {
                                emit_literal_stub(&mut code, &mut fixups, 10, kn);
                            } else {
                                let r = src_reg(nft, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            if let Some(kk) = k_key {
                                emit_literal_stub(&mut code, &mut fixups, 11, kk);
                            } else {
                                let r = src_reg(key, scratch2, &mut code)?;
                                push_word(&mut code, encode_addi(11, r, 0)?);
                            }
                            if let Some(kj) = k_json {
                                emit_literal_stub(&mut code, &mut fixups, 12, kj);
                            } else {
                                let r = src_reg(json, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, r, 0)?);
                            }
                            // Mirror all pointer arguments into INPUT.
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes()); // r10
                            push_word(&mut code, encode_addi(scratch1, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 11, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 12, 0)?);
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch1, 0)?);
                            push_word(&mut code, encode_addi(11, scratch2, 0)?);
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
                        Instr::SysvarAuthority { dest } => {
                            push_syscall(&mut code, syscalls::SYSCALL_SYSVAR_AUTHORITY);
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
                        Instr::BlockHeight { dest } => {
                            push_syscall(&mut code, syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::BlockTimeMs { dest } => {
                            push_syscall(&mut code, syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::ChainId { dest } => {
                            push_syscall(&mut code, syscalls::SYSCALL_SYSVAR_CHAIN_ID);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::ContractAddress { dest } => {
                            push_syscall(&mut code, syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Entrypoint { dest } => {
                            push_syscall(&mut code, syscalls::SYSCALL_SYSVAR_ENTRYPOINT);
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
                        Instr::CallContract {
                            dest,
                            contract,
                            entrypoint,
                            payload,
                        } => {
                            let contract_reg = src_reg(contract, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, contract_reg, 0)?);
                            let publish_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&publish_word.to_le_bytes());
                            push_word(&mut code, encode_addi(13, 10, 0)?);

                            let entrypoint_reg = src_reg(entrypoint, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, entrypoint_reg, 0)?);
                            code.extend_from_slice(&publish_word.to_le_bytes());
                            push_word(&mut code, encode_addi(14, 10, 0)?);

                            let payload_reg = src_reg(payload, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, payload_reg, 0)?);
                            code.extend_from_slice(&publish_word.to_le_bytes());
                            push_word(&mut code, encode_addi(12, 10, 0)?);
                            push_word(&mut code, encode_addi(10, 13, 0)?);
                            push_word(&mut code, encode_addi(11, 14, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_CALL_CONTRACT as u8,
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
                        Instr::InvokeEntrypointAs {
                            dest,
                            actor,
                            entrypoint,
                            payload,
                            returns_pointer,
                        } => {
                            if let Some(actor_raw) = string_map.get(&(func_idx, *actor)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Blob, actor_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(actor, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            if let Some(entrypoint_raw) = string_map.get(&(func_idx, *entrypoint)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    11,
                                    DataKey(DataKind::Blob, entrypoint_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(entrypoint, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(11, rs, 0)?);
                            }
                            if let Some(payload_raw) = string_map.get(&(func_idx, *payload)) {
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *payload)) {
                                    emit_literal_stub(
                                        &mut code,
                                        &mut fixups,
                                        12,
                                        data_key_for_pointer(*kind, payload_raw),
                                    );
                                } else {
                                    let rs_payload = src_reg(payload, scratch1, &mut code)?;
                                    push_word(&mut code, encode_addi(12, rs_payload, 0)?);
                                }
                            } else {
                                let rs_payload = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, rs_payload, 0)?);
                            }
                            push_word(
                                &mut code,
                                encode_addi(13, 0, if *returns_pointer { 1 } else { 0 })?,
                            );
                            push_word(&mut code, encode_addi(14, 0, 1)?);
                            push_syscall(
                                &mut code,
                                syscalls::SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS,
                            );
                            if let Some(dest) = dest {
                                let (rd, spilled, imm) = dst_reg(dest);
                                push_word(&mut code, encode_addi(rd, 10, 0)?);
                                spill_back(dest, rd, spilled, imm, &mut code)?;
                            }
                        }
                        Instr::InvokeEntrypointAsMulti {
                            dests,
                            actor,
                            entrypoint,
                            payload,
                            return_pointer_mask,
                        } => {
                            if dests.len() > regalloc::MAX_RETURN_VALUES {
                                return Err(format!(
                                    "too many return values in invoke_entrypoint_as: {} > {}",
                                    dests.len(),
                                    regalloc::MAX_RETURN_VALUES
                                ));
                            }
                            if let Some(actor_raw) = string_map.get(&(func_idx, *actor)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Blob, actor_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(actor, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            if let Some(entrypoint_raw) = string_map.get(&(func_idx, *entrypoint)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    11,
                                    DataKey(DataKind::Blob, entrypoint_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(entrypoint, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(11, rs, 0)?);
                            }
                            if let Some(payload_raw) = string_map.get(&(func_idx, *payload)) {
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *payload)) {
                                    emit_literal_stub(
                                        &mut code,
                                        &mut fixups,
                                        12,
                                        data_key_for_pointer(*kind, payload_raw),
                                    );
                                } else {
                                    let rs_payload = src_reg(payload, scratch1, &mut code)?;
                                    push_word(&mut code, encode_addi(12, rs_payload, 0)?);
                                }
                            } else {
                                let rs_payload = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, rs_payload, 0)?);
                            }
                            let return_pointer_mask = i16::try_from(*return_pointer_mask)
                                .map_err(|_| "return pointer mask does not fit ADDI immediate")?;
                            let return_count = i16::try_from(dests.len())
                                .map_err(|_| "return count does not fit ADDI immediate")?;
                            push_word(&mut code, encode_addi(13, 0, return_pointer_mask)?);
                            push_word(&mut code, encode_addi(14, 0, return_count)?);
                            push_syscall(
                                &mut code,
                                syscalls::SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS,
                            );
                            for (idx, dest) in dests.iter().enumerate() {
                                let source_reg = (regalloc::RET_REG + idx) as u8;
                                let (rd, spilled, imm) = dst_reg(dest);
                                push_word(&mut code, encode_addi(rd, source_reg, 0)?);
                                spill_back(dest, rd, spilled, imm, &mut code)?;
                            }
                        }
                        Instr::ExpectRejectAs {
                            actor,
                            entrypoint,
                            payload,
                        } => {
                            if let Some(actor_raw) = string_map.get(&(func_idx, *actor)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Blob, actor_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(actor, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            if let Some(entrypoint_raw) = string_map.get(&(func_idx, *entrypoint)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    11,
                                    DataKey(DataKind::Blob, entrypoint_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(entrypoint, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(11, rs, 0)?);
                            }
                            if let Some(payload_raw) = string_map.get(&(func_idx, *payload)) {
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *payload)) {
                                    emit_literal_stub(
                                        &mut code,
                                        &mut fixups,
                                        12,
                                        data_key_for_pointer(*kind, payload_raw),
                                    );
                                } else {
                                    let rs_payload = src_reg(payload, scratch1, &mut code)?;
                                    push_word(&mut code, encode_addi(12, rs_payload, 0)?);
                                }
                            } else {
                                let rs_payload = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(12, rs_payload, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_KOTO_TEST_EXPECT_REJECT_AS);
                        }
                        Instr::ActorAccount { dest, actor } => {
                            if let Some(actor_raw) = string_map.get(&(func_idx, *actor)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Blob, actor_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(actor, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_KOTO_TEST_ACTOR_ACCOUNT);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::ActorPublicKey { dest, actor } => {
                            if let Some(actor_raw) = string_map.get(&(func_idx, *actor)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Blob, actor_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(actor, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::ActorSign {
                            dest,
                            actor,
                            message,
                        } => {
                            if let Some(actor_raw) = string_map.get(&(func_idx, *actor)) {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    DataKey(DataKind::Blob, actor_raw.clone()),
                                );
                            } else {
                                let rs = src_reg(actor, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, rs, 0)?);
                            }
                            if let Some(message_raw) = string_map.get(&(func_idx, *message)) {
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *message)) {
                                    emit_literal_stub(
                                        &mut code,
                                        &mut fixups,
                                        11,
                                        data_key_for_pointer(*kind, message_raw),
                                    );
                                } else {
                                    let rs_message = src_reg(message, scratch1, &mut code)?;
                                    push_word(&mut code, encode_addi(11, rs_message, 0)?);
                                }
                            } else {
                                let rs_message = src_reg(message, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(11, rs_message, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_KOTO_TEST_ACTOR_SIGN);
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
                        Instr::Blake2b256Hash { dest, message } => {
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
                                syscalls::SYSCALL_BLAKE2B256_HASH as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::Keccak256Hash { dest, message } => {
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
                                syscalls::SYSCALL_KECCAK256_HASH as u8,
                            );
                            code.extend_from_slice(&call.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::IrohaHash { dest, message } => {
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
                                syscalls::SYSCALL_IROHA_HASH as u8,
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
                            // Skip ABORT when the condition is true (i.e., != 0).
                            let skip_word = encode_branch_rv(0x1, rs, 0, 8)?;
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
                            // Move message to r10 and issue debug log syscall.
                            push_word(&mut code, encode_addi(10, r_msg, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_DEBUG_LOG as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                        }
                        Instr::DebugPrint { value } => {
                            let r_value = src_reg(value, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(10, r_value, 0)?);
                            push_syscall(&mut code, syscalls::SYSCALL_DEBUG_PRINT);
                        }
                        Instr::DebugLog { payload } => {
                            if let Some(kind) = dataref_kind_map.get(&(func_idx, *payload))
                                && let Some(raw) = string_map.get(&(func_idx, *payload))
                            {
                                emit_literal_stub(
                                    &mut code,
                                    &mut fixups,
                                    10,
                                    data_key_for_pointer(*kind, raw),
                                );
                            } else {
                                let r_payload = src_reg(payload, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r_payload, 0)?);
                            }
                            push_syscall(&mut code, syscalls::SYSCALL_DEBUG_LOG);
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
                        Instr::StateKeys {
                            dest,
                            prefix,
                            offset,
                            limit,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            if let Some(s) = string_map.get(&(func_idx, *prefix)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(prefix, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            let offset_reg = src_reg(offset, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(11, offset_reg, 0)?);
                            let limit_reg = src_reg(limit, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(12, limit_reg, 0)?);
                            push_syscall(&mut code, syscalls::SYSCALL_STATE_KEYS);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::StateHas { dest, path } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
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
                            push_syscall(&mut code, syscalls::SYSCALL_STATE_HAS);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::StateLen { dest, path } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
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
                            push_syscall(&mut code, syscalls::SYSCALL_STATE_LEN);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::StateCount { dest, prefix } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            if let Some(s) = string_map.get(&(func_idx, *prefix)) {
                                let key = DataKey(DataKind::Name, s.clone());
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                let r = src_reg(prefix, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_syscall(&mut code, syscalls::SYSCALL_STATE_COUNT);
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
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
                        Instr::NumericNeg { dest, value } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            if let Some(kind) = dataref_kind_map.get(&(func_idx, *value)).copied()
                                && let Some(lit) = string_map.get(&(func_idx, *value)).cloned()
                            {
                                let key = data_key_for_pointer(kind, &lit);
                                emit_literal_stub(&mut code, &mut fixups, 10, key);
                            } else {
                                if string_map.contains_key(&(func_idx, *value))
                                    && !dataref_kind_map.contains_key(&(func_idx, *value))
                                {
                                    return Err(i18n::translate(
                                        self.lang,
                                        Message::SemanticError(
                                            "numeric literal missing ABI metadata during numeric lowering",
                                        ),
                                    ));
                                }
                                let r = src_reg(value, scratch1, &mut code)?;
                                push_word(&mut code, encode_addi(10, r, 0)?);
                            }
                            code.extend_from_slice(&publish_tlv);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_NUMERIC_NEG as u8,
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
                        Instr::DirectHelperSyscall {
                            dest,
                            syscall,
                            args,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(durable_required_msg),
                                ));
                            }
                            for (idx, arg) in args.iter().enumerate() {
                                let target = 10u8.checked_add(idx as u8).ok_or_else(|| {
                                    i18n::translate(
                                        self.lang,
                                        Message::SemanticError(
                                            "direct helper syscall has too many arguments",
                                        ),
                                    )
                                })?;
                                if let Some(kind) = dataref_kind_map.get(&(func_idx, *arg)).copied()
                                    && let Some(lit) = string_map.get(&(func_idx, *arg)).cloned()
                                {
                                    let key = data_key_for_pointer(kind, &lit);
                                    emit_literal_stub(&mut code, &mut fixups, target, key);
                                } else {
                                    let scratch = if target == scratch1 {
                                        scratch2
                                    } else {
                                        scratch1
                                    };
                                    let r = src_reg(arg, scratch, &mut code)?;
                                    push_word(&mut code, encode_addi(target, r, 0)?);
                                }
                            }
                            push_syscall(&mut code, *syscall);
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
                        Instr::JsonObject { dest } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_OBJECT as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonSetInt {
                            dest,
                            json,
                            key,
                            value,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
                                ));
                            }
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
                            let value_reg = src_reg(value, scratch1, &mut code)?;
                            push_word(&mut code, encode_addi(12, value_reg, 0)?);
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_SET_I64 as u8,
                            );
                            code.extend_from_slice(&word.to_le_bytes());
                            let (rd, spilled, imm) = dst_reg(dest);
                            push_word(&mut code, encode_addi(rd, 10, 0)?);
                            spill_back(dest, rd, spilled, imm, &mut code)?;
                        }
                        Instr::JsonSetAccountId {
                            dest,
                            json,
                            key,
                            value,
                        } => {
                            if !durable_enabled {
                                return Err(i18n::translate(
                                    self.lang,
                                    Message::UnsupportedBinaryOp(
                                        "durable state requires ABI v1. Add `meta { abi_version: 1; }` or compile with `--abi 1`.",
                                    ),
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
                                                "pointer literal missing ABI metadata during json_set_account_id lowering",
                                            ),
                                        ));
                                    }
                                    let rs = src_reg(temp, scratch, code)?;
                                    push_word(code, encode_addi(target, rs, 0)?);
                                }
                                Ok(())
                            };
                            load_ptr(json, 10, scratch1, &mut code)?;
                            let pub_word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
                            );
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(scratch2, 10, 0)?);
                            load_ptr(key, 10, scratch1, &mut code)?;
                            code.extend_from_slice(&pub_word.to_le_bytes());
                            push_word(&mut code, encode_addi(11, 10, 0)?);
                            push_word(&mut code, encode_addi(10, scratch2, 0)?);
                            load_ptr(value, 12, scratch1, &mut code)?;
                            let word = encoding::wide::encode_sys(
                                instruction::wide::system::SCALL,
                                syscalls::SYSCALL_JSON_SET_ACCOUNT_ID as u8,
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
                patch_jump_transfer_stub(&mut code, fix.at, target_pc, 0)?;
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

                patch_jump_transfer_stub(&mut code, fix.jal_else_at, else_target_pc, 0)?;
                patch_jump_transfer_stub(&mut code, fix.jal_then_at, then_target_pc, 0)?;
            }
            uses_zk_global |= uses_zk;
        }

        for func in &ir_prog.functions {
            if func.name == entry_name {
                continue;
            }
            let wrapper_start = code.len();
            entrypoint_wrapper_offsets.insert(func.name.clone(), wrapper_start);
            let call_at = reserve_control_transfer_stub(&mut code);
            call_fixups.push((
                call_at,
                func.name.clone(),
                format!("__entrypoint_wrapper:{}", func.name),
            ));
            push_word(&mut code, encoding::wide::encode_halt());
        }

        // Patch call sites now that function offsets are known.
        for (at, callee, _caller) in &call_fixups {
            let target = *func_start_offsets.get(callee).ok_or_else(|| {
                i18n::translate(self.lang, Message::SemanticError("unknown callee"))
            })? as u64;
            patch_call_transfer_stub(&mut code, *at, target, 0)?;
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
        let vector_length = meta_decl
            .and_then(|m| m.vector_length)
            .unwrap_or(self.opts.vector_length);
        if vector_length > ivm_abi::metadata::VECTOR_LENGTH_MAX {
            return Err(format!(
                "unsupported vector_length {vector_length}; expected 0..={}",
                ivm_abi::metadata::VECTOR_LENGTH_MAX
            ));
        }
        let meta = ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode,
            vector_length,
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
                DRK::SoracloudRequest => DataKey(DataKind::SoracloudRequest, v.clone()),
                DRK::SoracloudResponse => DataKey(DataKind::SoracloudResponse, v.clone()),
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
                    let id = iroha_data_model::domain::DomainId::parse_fully_qualified(s).map_err(
                        |e| {
                            let err = format!("invalid DomainId literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        },
                    )?;
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
                DataKey(DataKind::SoracloudRequest, s) => {
                    let bytes = decode_hex_or_raw_bytes(s).map_err(|e| {
                        let err = format!("invalid SoracloudRequest literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    let value: iroha_data_model::soracloud::SoracloudHostRequestEnvelopeV1 =
                        decode_from_bytes(&bytes).map_err(|e| {
                            let err = format!(
                                "invalid SoracloudRequest literal `{s}`: cannot decode ({e})"
                            );
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?;
                    value.validate().map_err(|e| {
                        let err = format!("invalid SoracloudRequest literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        PointerType::SoracloudRequest as u16,
                        to_bytes(&value).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid SoracloudRequest literal `{s}`: {e}");
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?,
                    )
                }
                DataKey(DataKind::SoracloudResponse, s) => {
                    let bytes = decode_hex_or_raw_bytes(s).map_err(|e| {
                        let err = format!("invalid SoracloudResponse literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    let value: iroha_data_model::soracloud::SoracloudHostResponseEnvelopeV1 =
                        decode_from_bytes(&bytes).map_err(|e| {
                            let err = format!(
                                "invalid SoracloudResponse literal `{s}`: cannot decode ({e})"
                            );
                            i18n::translate(self.lang, Message::SemanticError(&err))
                        })?;
                    value.validate().map_err(|e| {
                        let err = format!("invalid SoracloudResponse literal `{s}`: {e}");
                        i18n::translate(self.lang, Message::SemanticError(&err))
                    })?;
                    (
                        PointerType::SoracloudResponse as u16,
                        to_bytes(&value).map_err(|e| e.to_string()).map_err(|e| {
                            let err = format!("invalid SoracloudResponse literal `{s}`: {e}");
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

        let mut entrypoint_start_offsets = func_start_offsets.clone();
        entrypoint_start_offsets.extend(entrypoint_wrapper_offsets);
        let entrypoint_descriptors = build_entrypoint_descriptors(
            &typed,
            &access_sets,
            &ir_prog.functions,
            &hint_reports,
            &entrypoint_start_offsets,
        )?;
        if self.opts.mode == CompilerMode::Production
            && let Some(entrypoint) = entrypoint_descriptors.iter().find(|entrypoint| {
                entrypoint.access_hints_complete == Some(false)
                    && !production_allows_incomplete_access_hints(&entrypoint.access_hints_skipped)
            })
        {
            let reasons = if entrypoint.access_hints_skipped.is_empty() {
                "no reason recorded".to_owned()
            } else {
                entrypoint.access_hints_skipped.join("; ")
            };
            return Err(format!(
                "E_ACCESS_INCOMPLETE: entrypoint `{}` has incomplete compiler-derived access metadata: {reasons}",
                entrypoint.name
            ));
        }
        let state_descriptors = build_state_descriptors(&typed)?;
        let access_set_hints = build_access_set_hints(
            &typed,
            &access_sets,
            include_hints,
            self.opts.dynamic_iter_cap,
        );
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
            states: state_descriptors,
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
        // Sora Nexus contracts still have legitimate dynamic ledger operations
        // whose exact account/asset keys are only known from the call payload.
        // Keep emitting compiler-owned fallback hints for those paths and let
        // the scheduler's dynamic prepass/conservative fallback serialize them.
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
        let runtime_code_prefix = contract_section.len() as u64
            + debug_section.len() as u64
            + lit_header_size
            + lit_size
            + data_bytes.len() as u64
            + post_pad as u64;
        // Relative near calls are invariant under the runtime prefix, but far
        // JALR stubs need the final code-region prefix added to their absolute
        // targets so they do not jump into the CNTR/DBG1/LTLB prefix bytes.
        for (at, callee, _caller) in &call_fixups {
            let target = *func_start_offsets.get(callee).ok_or_else(|| {
                i18n::translate(self.lang, Message::SemanticError("unknown callee"))
            })? as u64;
            patch_call_transfer_stub(&mut code, *at, target, runtime_code_prefix)?;
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

    /// Compile an already parsed program and produce a manifest with code_hash and abi_hash.
    pub fn compile_program_with_manifest(
        &self,
        program: &Program,
    ) -> Result<
        (
            Vec<u8>,
            iroha_data_model::smart_contract::manifest::ContractManifest,
        ),
        String,
    > {
        let (bytes, manifest, _report) = self.compile_program_with_manifest_and_report(program)?;
        Ok((bytes, manifest))
    }

    /// Compile an already parsed program and produce a manifest plus compiler report data.
    pub fn compile_program_with_manifest_and_report(
        &self,
        program: &Program,
    ) -> Result<
        (
            Vec<u8>,
            iroha_data_model::smart_contract::manifest::ContractManifest,
            CompileReport,
        ),
        String,
    > {
        let artifacts = self.compile_program(program)?;
        self.manifest_from_artifacts(artifacts)
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
        self.manifest_from_artifacts(artifacts)
    }

    fn manifest_from_artifacts(
        &self,
        artifacts: CompilationArtifacts,
    ) -> Result<
        (
            Vec<u8>,
            iroha_data_model::smart_contract::manifest::ContractManifest,
            CompileReport,
        ),
        String,
    > {
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
            states: Some(manifest_state_descriptors(&contract_interface.states)),
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

fn program_with_first_release_prelude(program: &Program) -> Result<Program, String> {
    let mut defined = HashSet::new();
    let mut called = HashSet::new();
    collect_program_function_names(program, &mut defined, &mut called);

    let mut needed = HashSet::new();
    for name in [
        "require_authority",
        "require_owner",
        "bps_fee",
        "checked_add_amount",
        "checked_sub_amount",
        "verify_signed_json",
        "require_json_int",
    ] {
        if called.contains(name) && !defined.contains(name) {
            needed.insert(name.to_owned());
        }
    }
    if needed.contains("require_owner") && !defined.contains("require_authority") {
        needed.insert("require_authority".to_owned());
    }

    if needed.is_empty() {
        return Ok(program.clone());
    }

    let prelude = parser::parse(FIRST_RELEASE_PRELUDE)
        .map_err(|err| format!("prelude parse error: {err}"))?;
    let mut items = Vec::new();
    for item in prelude.items {
        if let Item::Function(func) = &item
            && needed.contains(&func.name)
        {
            items.push(item);
        }
    }
    items.extend(program.items.clone());

    Ok(Program {
        items,
        contract_meta: program.contract_meta.clone(),
        test_target: program.test_target.clone(),
        fixtures: program.fixtures.clone(),
    })
}

fn collect_program_function_names(
    program: &Program,
    defined: &mut HashSet<String>,
    called: &mut HashSet<String>,
) {
    for item in &program.items {
        match item {
            Item::Function(func) => {
                defined.insert(func.name.clone());
                collect_block_calls(&func.body, called);
            }
            Item::Const(const_decl) => collect_expr_calls(&const_decl.value, called),
            Item::Struct(_) | Item::State(_) | Item::Trigger(_) | Item::Kotoba(_) => {}
        }
    }
}

fn collect_block_calls(block: &Block, called: &mut HashSet<String>) {
    for stmt in &block.statements {
        collect_statement_calls(stmt, called);
    }
}

fn collect_statement_calls(stmt: &Statement, called: &mut HashSet<String>) {
    match stmt {
        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
            collect_expr_calls(value, called);
        }
        Statement::AssignExpr { target, value, .. } => {
            collect_expr_calls(target, called);
            collect_expr_calls(value, called);
        }
        Statement::Expr(expr) | Statement::Return(Some(expr)) => collect_expr_calls(expr, called),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_calls(cond, called);
            collect_block_calls(then_branch, called);
            if let Some(branch) = else_branch {
                collect_block_calls(branch, called);
            }
        }
        Statement::While { cond, body } => {
            collect_expr_calls(cond, called);
            collect_block_calls(body, called);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                collect_statement_calls(init, called);
            }
            if let Some(cond) = cond {
                collect_expr_calls(cond, called);
            }
            if let Some(step) = step {
                collect_statement_calls(step, called);
            }
            collect_block_calls(body, called);
        }
        Statement::ForEachMap { map, body, .. } => {
            collect_expr_calls(map, called);
            collect_block_calls(body, called);
        }
    }
}

fn collect_expr_calls(expr: &Expr, called: &mut HashSet<String>) {
    match expr {
        Expr::Binary { left, right, .. } => {
            collect_expr_calls(left, called);
            collect_expr_calls(right, called);
        }
        Expr::Unary { expr, .. } => collect_expr_calls(expr, called),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_expr_calls(cond, called);
            collect_expr_calls(then_expr, called);
            collect_expr_calls(else_expr, called);
        }
        Expr::Call { name, args } => {
            called.insert(name.clone());
            for arg in args {
                collect_expr_calls(arg, called);
            }
        }
        Expr::Member { object, .. } => collect_expr_calls(object, called),
        Expr::Index { target, index } => {
            collect_expr_calls(target, called);
            collect_expr_calls(index, called);
        }
        Expr::Tuple(items) => {
            for item in items {
                collect_expr_calls(item, called);
            }
        }
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Decimal(_)
        | Expr::String(_)
        | Expr::Bytes(_)
        | Expr::Ident(_) => {}
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

fn retain_taira_supported_access_key(key: &str) -> bool {
    key != GLOBAL_WILDCARD_KEY && !key.ends_with(":*")
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
    typed: &TypedProgram,
    access_sets: &[AccessSets],
    include_hints: bool,
    dynamic_iter_cap: u8,
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
    // Public Taira currently rejects wildcard access keys in contract artifacts.
    // Keep literal keys and dynamic hint metadata, but drop coarse wildcard keys
    // such as `*`, `state:*`, and `asset:*` from the persisted artifact.
    reads.retain(|key| retain_taira_supported_access_key(key));
    writes.retain(|key| retain_taira_supported_access_key(key));
    if reads.is_empty() && writes.is_empty() {
        return None;
    }
    let (dynamic_reads, dynamic_writes) =
        collect_dynamic_access_hints(typed, u32::from(dynamic_iter_cap));
    Some(AccessSetHints {
        read_keys: reads.into_iter().collect(),
        write_keys: writes.into_iter().collect(),
        dynamic_reads,
        dynamic_writes,
    })
}

fn collect_dynamic_access_hints(
    typed: &TypedProgram,
    dynamic_iter_cap: u32,
) -> (Vec<DynamicAccessHint>, Vec<DynamicAccessHint>) {
    let mut reads = BTreeSet::new();
    for item in &typed.items {
        let TypedItem::Function(func) = item;
        collect_dynamic_access_hints_from_block(&func.body, dynamic_iter_cap, &mut reads);
    }
    (
        reads
            .into_iter()
            .map(|(base_key, key_type, bound_kind)| DynamicAccessHint {
                base_key,
                key_type,
                bound_kind,
                max_keys: dynamic_iter_cap,
            })
            .collect(),
        Vec::new(),
    )
}

fn collect_dynamic_access_hints_from_block(
    block: &semantic::TypedBlock,
    dynamic_iter_cap: u32,
    reads: &mut BTreeSet<(String, String, String)>,
) {
    for stmt in &block.statements {
        collect_dynamic_access_hints_from_statement(stmt, dynamic_iter_cap, reads);
    }
}

fn collect_dynamic_access_hints_from_statement(
    stmt: &semantic::TypedStatement,
    dynamic_iter_cap: u32,
    reads: &mut BTreeSet<(String, String, String)>,
) {
    match stmt {
        semantic::TypedStatement::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_dynamic_access_hints_from_block(then_branch, dynamic_iter_cap, reads);
            if let Some(block) = else_branch {
                collect_dynamic_access_hints_from_block(block, dynamic_iter_cap, reads);
            }
        }
        semantic::TypedStatement::While { body, .. }
        | semantic::TypedStatement::For { body, .. } => {
            collect_dynamic_access_hints_from_block(body, dynamic_iter_cap, reads);
        }
        semantic::TypedStatement::ForEachMap { map, body, .. } => {
            #[cfg(feature = "kotodama_dynamic_bounds")]
            if let semantic::TypedStatement::ForEachMap {
                dyn_count,
                dyn_start,
                ..
            } = stmt
                && dyn_count.is_some()
                && let Some(base) = semantic::typed_state_handle_name(map)
                && let semantic::Type::Map(key_ty, _) = semantic::resolve_struct_type(&map.ty)
            {
                let bound_kind = if dyn_start.is_some() { "range" } else { "take" };
                reads.insert((
                    format!("state:{base}"),
                    semantic::render_type_name(key_ty.as_ref()),
                    bound_kind.to_string(),
                ));
            }
            collect_dynamic_access_hints_from_block(body, dynamic_iter_cap, reads);
        }
        semantic::TypedStatement::Let { .. }
        | semantic::TypedStatement::Expr(_)
        | semantic::TypedStatement::Return(_)
        | semantic::TypedStatement::Break
        | semantic::TypedStatement::Continue
        | semantic::TypedStatement::MapSet { .. } => {}
    }
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

fn build_state_descriptors(typed: &TypedProgram) -> Result<Vec<EmbeddedStateDescriptor>, String> {
    typed
        .states
        .iter()
        .map(|state| {
            Ok(EmbeddedStateDescriptor {
                name: state.name.clone(),
                ty: build_state_type_descriptor(&state.ty)?,
            })
        })
        .collect()
}

fn manifest_state_descriptors(states: &[EmbeddedStateDescriptor]) -> Vec<StateDescriptor> {
    states
        .iter()
        .map(|state| StateDescriptor {
            name: state.name.clone(),
            type_name: manifest_state_type_name(&state.ty),
        })
        .collect()
}

fn manifest_state_type_name(ty: &EmbeddedStateType) -> String {
    match ty {
        EmbeddedStateType::Int => "int".to_string(),
        EmbeddedStateType::FixedU128 => "FixedU128".to_string(),
        EmbeddedStateType::Amount => "Amount".to_string(),
        EmbeddedStateType::Balance => "Balance".to_string(),
        EmbeddedStateType::Bool => "bool".to_string(),
        EmbeddedStateType::String => "string".to_string(),
        EmbeddedStateType::Blob => "Blob".to_string(),
        EmbeddedStateType::Bytes => "bytes".to_string(),
        EmbeddedStateType::DataSpaceId => "DataSpaceId".to_string(),
        EmbeddedStateType::AccountId => "AccountId".to_string(),
        EmbeddedStateType::AssetDefinitionId => "AssetDefinitionId".to_string(),
        EmbeddedStateType::AssetId => "AssetId".to_string(),
        EmbeddedStateType::NftId => "NftId".to_string(),
        EmbeddedStateType::DomainId => "DomainId".to_string(),
        EmbeddedStateType::Name => "Name".to_string(),
        EmbeddedStateType::Json => "Json".to_string(),
        EmbeddedStateType::Tuple(items) => {
            let items = items
                .iter()
                .map(manifest_state_type_name)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({items})")
        }
        EmbeddedStateType::Struct { name, fields } => {
            let fields = fields
                .iter()
                .map(|field| format!("{}: {}", field.name, manifest_state_type_name(&field.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}{{{fields}}}")
        }
        EmbeddedStateType::Map { key, value } => {
            format!(
                "map<{}, {}>",
                manifest_state_type_name(key),
                manifest_state_type_name(value)
            )
        }
    }
}

fn build_state_type_descriptor(ty: &semantic::Type) -> Result<EmbeddedStateType, String> {
    use semantic::Type;

    Ok(match semantic::resolve_struct_type(ty) {
        Type::Int => EmbeddedStateType::Int,
        Type::FixedU128 => EmbeddedStateType::FixedU128,
        Type::Amount => EmbeddedStateType::Amount,
        Type::Balance => EmbeddedStateType::Balance,
        Type::Bool => EmbeddedStateType::Bool,
        Type::String => EmbeddedStateType::String,
        Type::Blob => EmbeddedStateType::Blob,
        Type::Bytes => EmbeddedStateType::Bytes,
        Type::DataSpaceId => EmbeddedStateType::DataSpaceId,
        Type::AccountId => EmbeddedStateType::AccountId,
        Type::AssetDefinitionId => EmbeddedStateType::AssetDefinitionId,
        Type::AssetId => EmbeddedStateType::AssetId,
        Type::NftId => EmbeddedStateType::NftId,
        Type::DomainId => EmbeddedStateType::DomainId,
        Type::Name => EmbeddedStateType::Name,
        Type::Json => EmbeddedStateType::Json,
        Type::Tuple(items) => EmbeddedStateType::Tuple(
            items
                .iter()
                .map(build_state_type_descriptor)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Type::Struct { name, fields } => EmbeddedStateType::Struct {
            name,
            fields: fields
                .iter()
                .map(|(field_name, field_ty)| {
                    Ok(EmbeddedStateFieldDescriptor {
                        name: field_name.clone(),
                        ty: build_state_type_descriptor(field_ty)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        },
        Type::Map(key, value) => EmbeddedStateType::Map {
            key: Box::new(build_state_type_descriptor(&key)?),
            value: Box::new(build_state_type_descriptor(&value)?),
        },
        Type::Opaque(name) => {
            return Err(format!(
                "state type `{name}` was not resolved before CNTR schema emission"
            ));
        }
        Type::Unit
        | Type::AxtDescriptor
        | Type::AssetHandle
        | Type::ProofBlob
        | Type::SoracloudRequest
        | Type::SoracloudResponse => {
            return Err("state type is not supported in embedded state schemas".to_string());
        }
    })
}

fn entrypoint_ir_symbol_name(func: &semantic::TypedFunction) -> String {
    let needs_wrapper = (matches!(func.modifiers.kind, super::ast::FunctionKind::View)
        || func.modifiers.visibility == super::ast::FunctionVisibility::Public)
        && !func.param_types.is_empty();
    if needs_wrapper {
        format!("__entrypoint_impl__{}", func.name)
    } else {
        func.name.clone()
    }
}

#[allow(clippy::too_many_arguments)]
fn derive_isi_access_hints(
    ir_prog: &ir::Program,
    string_map: &HashMap<(usize, ir::Temp), String>,
    int_const_map: &HashMap<(usize, ir::Temp), i64>,
    authority_account_temps: &HashSet<(usize, ir::Temp)>,
    dataref_kind_map: &HashMap<(usize, ir::Temp), ir::DataRefKind>,
    instruction_literal_access_map: &HashMap<(usize, ir::Temp), AccessSets>,
    access_sets: &mut [AccessSets],
    hint_diagnostics: &mut AccessHintDiagnostics,
    hint_skips: &mut [IndexSet<String>],
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
                    int_const_map,
                    authority_account_temps,
                    dataref_kind_map,
                    instruction_literal_access_map,
                    &mut access_sets[func_idx],
                    hint_diagnostics,
                    &mut hint_skips[func_idx],
                );
            }
        }
    }
}

fn record_hint_skip(skips: &mut IndexSet<String>, reason: &str) {
    skips.insert(reason.to_owned());
}

fn production_allows_incomplete_access_hints(skipped_reasons: &[String]) -> bool {
    !skipped_reasons.is_empty()
        && skipped_reasons
            .iter()
            .all(|reason| reason == HINT_SKIP_CONTRACT_CALL_TARGET)
}

fn derive_state_access_hints(
    ir_prog: &ir::Program,
    state_path_hints: &HashMap<(usize, ir::Temp), StatePathHint>,
    access_sets: &mut [AccessSets],
    hint_diagnostics: &mut AccessHintDiagnostics,
    hint_skips: &mut [IndexSet<String>],
) {
    for (func_idx, func) in ir_prog.functions.iter().enumerate() {
        for bb in &func.blocks {
            for instr in &bb.instrs {
                match instr {
                    ir::Instr::StateGet { path, .. }
                    | ir::Instr::StateHas { path, .. }
                    | ir::Instr::StateLen { path, .. } => {
                        if let Some(key) =
                            render_state_hint(state_path_hints.get(&(func_idx, *path)))
                        {
                            insert_state_hint(&mut access_sets[func_idx].reads, key);
                        } else {
                            hint_diagnostics.state_wildcards =
                                hint_diagnostics.state_wildcards.saturating_add(1);
                            record_hint_skip(
                                &mut hint_skips[func_idx],
                                HINT_SKIP_DYNAMIC_STATE_PATH,
                            );
                            access_sets[func_idx]
                                .reads
                                .insert(STATE_WILDCARD_KEY.to_string());
                            access_sets[func_idx]
                                .writes
                                .insert(STATE_WILDCARD_KEY.to_string());
                        }
                    }
                    ir::Instr::StateKeys { prefix, .. } | ir::Instr::StateCount { prefix, .. } => {
                        if let Some(key) =
                            render_state_hint(state_path_hints.get(&(func_idx, *prefix)))
                        {
                            insert_state_hint(&mut access_sets[func_idx].reads, key);
                        } else {
                            hint_diagnostics.state_wildcards =
                                hint_diagnostics.state_wildcards.saturating_add(1);
                            record_hint_skip(
                                &mut hint_skips[func_idx],
                                HINT_SKIP_DYNAMIC_STATE_PATH,
                            );
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
                            record_hint_skip(
                                &mut hint_skips[func_idx],
                                HINT_SKIP_DYNAMIC_STATE_PATH,
                            );
                            access_sets[func_idx]
                                .reads
                                .insert(STATE_WILDCARD_KEY.to_string());
                            access_sets[func_idx]
                                .writes
                                .insert(STATE_WILDCARD_KEY.to_string());
                        }
                    }
                    ir::Instr::CallContract { .. } => {
                        hint_diagnostics.state_wildcards =
                            hint_diagnostics.state_wildcards.saturating_add(1);
                        record_hint_skip(&mut hint_skips[func_idx], HINT_SKIP_CONTRACT_CALL_TARGET);
                        access_sets[func_idx]
                            .reads
                            .insert(STATE_WILDCARD_KEY.to_string());
                        access_sets[func_idx]
                            .writes
                            .insert(STATE_WILDCARD_KEY.to_string());
                    }
                    _ => {}
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_isi_access(
    instr: &ir::Instr,
    func_idx: usize,
    string_map: &HashMap<(usize, ir::Temp), String>,
    int_const_map: &HashMap<(usize, ir::Temp), i64>,
    authority_account_temps: &HashSet<(usize, ir::Temp)>,
    dataref_kind_map: &HashMap<(usize, ir::Temp), ir::DataRefKind>,
    instruction_literal_access_map: &HashMap<(usize, ir::Temp), AccessSets>,
    access_set: &mut AccessSets,
    hint_diagnostics: &mut AccessHintDiagnostics,
    hint_skips: &mut IndexSet<String>,
) {
    let mut apply_fallback = |access_set: &mut AccessSets,
                              hint_diagnostics: &mut AccessHintDiagnostics,
                              reason: &str| {
        record_hint_skip(hint_skips, reason);
        hint_diagnostics.isi_wildcards = hint_diagnostics.isi_wildcards.saturating_add(1);
        if reason == HINT_SKIP_LITERAL_TRIGGER_SPEC_DECODE {
            hint_diagnostics.literal_trigger_spec_decode_failures = hint_diagnostics
                .literal_trigger_spec_decode_failures
                .saturating_add(1);
        }
        access_set.reads.insert(GLOBAL_WILDCARD_KEY.to_string());
        access_set.writes.insert(GLOBAL_WILDCARD_KEY.to_string());
    };
    match instr {
        ir::Instr::TransferBatchBegin | ir::Instr::TransferBatchEnd => {}
        ir::Instr::TransferBatchApply { payload } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_transfer_asset_batch_access(raw, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::CallContract { .. } => {}
        ir::Instr::EscrowOpenOffer { escrow, asset, .. } => {
            let Some(escrow_id) = escrow_id_from_name_temp(string_map, func_idx, *escrow) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let asset_definition = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset);
            record_asset_escrow_open_access(access_set, &escrow_id, asset_definition.as_ref());
        }
        ir::Instr::EscrowAccept { escrow }
        | ir::Instr::EscrowMarkPaymentSent { escrow }
        | ir::Instr::EscrowOpenDispute { escrow, .. } => {
            let Some(escrow_id) = escrow_id_from_name_temp(string_map, func_idx, *escrow) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            record_asset_escrow_lifecycle_access(access_set, &escrow_id);
        }
        ir::Instr::EscrowRelease { escrow }
        | ir::Instr::EscrowCancel { escrow }
        | ir::Instr::EscrowResolveDispute { escrow, .. } => {
            let Some(escrow_id) = escrow_id_from_name_temp(string_map, func_idx, *escrow) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            record_asset_escrow_close_access(access_set, &escrow_id);
        }
        ir::Instr::AnonymousEscrowOpenOffer { request } => {
            let Some(raw) = string_map.get(&(func_idx, *request)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_anonymous_escrow_request_access(
                raw,
                AnonymousEscrowRequestKind::OpenOffer,
                access_set,
            )
            .is_none()
            {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::AnonymousEscrowRelease { request } => {
            let Some(raw) = string_map.get(&(func_idx, *request)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_anonymous_escrow_request_access(
                raw,
                AnonymousEscrowRequestKind::Release,
                access_set,
            )
            .is_none()
            {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::AnonymousEscrowCancel { request } => {
            let Some(raw) = string_map.get(&(func_idx, *request)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_anonymous_escrow_request_access(
                raw,
                AnonymousEscrowRequestKind::Cancel,
                access_set,
            )
            .is_none()
            {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::AnonymousEscrowResolveDispute { request } => {
            let Some(raw) = string_map.get(&(func_idx, *request)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_anonymous_escrow_request_access(
                raw,
                AnonymousEscrowRequestKind::ResolveDispute,
                access_set,
            )
            .is_none()
            {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::AnonymousEscrowAccept { escrow }
        | ir::Instr::AnonymousEscrowMarkPaymentSent { escrow }
        | ir::Instr::AnonymousEscrowOpenDispute { escrow, .. } => {
            let Some(escrow_id) = escrow_id_from_name_temp(string_map, func_idx, *escrow) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            record_anonymous_asset_escrow_lifecycle_access(access_set, &escrow_id);
        }
        ir::Instr::TransferAsset {
            from, to, asset, ..
        } => {
            let from =
                account_access_hint_for_temp(string_map, authority_account_temps, func_idx, *from);
            let to =
                account_access_hint_for_temp(string_map, authority_account_temps, func_idx, *to);
            if let Some(asset_def) = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset) {
                add_asset_rw_for_optional_account_hint(access_set, &asset_def, from.as_ref());
                add_asset_rw_for_optional_account_hint(access_set, &asset_def, to.as_ref());
            } else {
                add_dynamic_asset_definition_rw_for_optional_account_hint(
                    access_set,
                    from.as_ref(),
                );
                add_dynamic_asset_definition_rw_for_optional_account_hint(access_set, to.as_ref());
            }
        }
        ir::Instr::MintAsset { account, asset, .. }
        | ir::Instr::BurnAsset { account, asset, .. } => {
            let account = account_access_hint_for_temp(
                string_map,
                authority_account_temps,
                func_idx,
                *account,
            );
            if let Some(asset_def) = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset) {
                add_asset_rw_for_optional_account_hint(access_set, &asset_def, account.as_ref());
                add_asset_def_rw(access_set, &asset_def);
            } else {
                add_dynamic_asset_definition_rw_for_optional_account_hint(
                    access_set,
                    account.as_ref(),
                );
            }
        }
        ir::Instr::RegisterDomain { domain } | ir::Instr::UnregisterDomain { domain } => {
            let Some(id) = parse_domain_temp(string_map, func_idx, *domain) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_domain_rw(access_set, &id);
        }
        ir::Instr::RegisterAccount { account } | ir::Instr::UnregisterAccount { account } => {
            let Some(id) = account_access_hint_for_temp(
                string_map,
                authority_account_temps,
                func_idx,
                *account,
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_account_hint_rw(access_set, &id);
        }
        ir::Instr::AddSignatory { account, .. }
        | ir::Instr::RemoveSignatory { account, .. }
        | ir::Instr::SetAccountQuorum { account, .. } => {
            let Some(id) = account_access_hint_for_temp(
                string_map,
                authority_account_temps,
                func_idx,
                *account,
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_account_hint_rw(access_set, &id);
        }
        ir::Instr::UnregisterAsset { asset } => {
            if let Some(id) = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset) {
                add_asset_def_domain_r_if_projected(access_set, &id);
                add_asset_def_rw(access_set, &id);
            } else {
                add_dynamic_asset_definition_rw(access_set);
            }
        }
        ir::Instr::SetAccountDetail { account, key, .. } => {
            let (Some(id), Some(key)) = (
                account_access_hint_for_temp(
                    string_map,
                    authority_account_temps,
                    func_idx,
                    *account,
                ),
                parse_temp(string_map, func_idx, *key),
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_account_detail_hint_rw(access_set, &id, &key);
        }
        ir::Instr::CreateNft { nft, owner } => {
            if let Some(owner) =
                account_access_hint_for_temp(string_map, authority_account_temps, func_idx, *owner)
            {
                add_account_hint_r(access_set, &owner);
            }
            let Some(id) = parse_temp(string_map, func_idx, *nft) else {
                add_nft_coarse_rw(access_set);
                return;
            };
            add_nft_rw(access_set, &id);
        }
        ir::Instr::BurnNft { nft } => {
            let Some(id) = parse_temp(string_map, func_idx, *nft) else {
                add_nft_coarse_rw(access_set);
                return;
            };
            add_nft_rw(access_set, &id);
        }
        ir::Instr::TransferNft { from, nft, to } => {
            let (Some(from), Some(to), Some(id)) = (
                account_access_hint_for_temp(string_map, authority_account_temps, func_idx, *from),
                account_access_hint_for_temp(string_map, authority_account_temps, func_idx, *to),
                parse_temp(string_map, func_idx, *nft),
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_account_hint_r(access_set, &from);
            add_account_hint_r(access_set, &to);
            add_nft_rw(access_set, &id);
        }
        ir::Instr::RemoveTrigger { name } | ir::Instr::SetTriggerEnabled { name, .. } => {
            let Some(id) = parse_temp(string_map, func_idx, *name) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_trigger_rw(access_set, &id);
        }
        ir::Instr::CreateRole { name, .. } | ir::Instr::DeleteRole { name } => {
            let Some(id) = parse_temp(string_map, func_idx, *name) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_role_rw(access_set, &id);
        }
        ir::Instr::GrantRole { account, name } | ir::Instr::RevokeRole { account, name } => {
            let (Some(account), Some(role)) = (
                account_access_hint_for_temp(
                    string_map,
                    authority_account_temps,
                    func_idx,
                    *account,
                ),
                parse_temp(string_map, func_idx, *name),
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_account_hint_rw(access_set, &account);
            add_role_r(access_set, &role);
            add_role_binding_hint_w(access_set, &account, &role);
        }
        ir::Instr::GrantPermission { account, token }
        | ir::Instr::RevokePermission { account, token } => {
            let Some(account) = account_access_hint_for_temp(
                string_map,
                authority_account_temps,
                func_idx,
                *account,
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let Some(perm) =
                permission_name_from_token(string_map, dataref_kind_map, func_idx, *token)
            else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_account_hint_rw(access_set, &account);
            add_permission_account_hint_w(access_set, &account, &perm);
        }
        ir::Instr::RegisterAsset { asset, .. } => {
            if let Some(id) = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset) {
                add_asset_def_domain_r_if_projected(access_set, &id);
                add_asset_def_rw(access_set, &id);
            } else {
                add_dynamic_asset_definition_rw(access_set);
            }
        }
        ir::Instr::CreateNewAsset { asset, account, .. } => {
            let account = account_access_hint_for_temp(
                string_map,
                authority_account_temps,
                func_idx,
                *account,
            );
            if let Some(asset_def) = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset) {
                add_asset_def_domain_r_if_projected(access_set, &asset_def);
                add_asset_def_rw(access_set, &asset_def);
                add_asset_rw_for_optional_account_hint(access_set, &asset_def, account.as_ref());
            } else {
                add_dynamic_asset_definition_rw_for_optional_account_hint(
                    access_set,
                    account.as_ref(),
                );
            }
        }
        ir::Instr::CreateTrigger { json } => {
            let Some(raw) = string_map.get(&(func_idx, *json)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let Some(id) = trigger_id_from_json(raw) else {
                return apply_fallback(
                    access_set,
                    hint_diagnostics,
                    HINT_SKIP_LITERAL_TRIGGER_SPEC_DECODE,
                );
            };
            add_trigger_rw(access_set, &id);
        }
        ir::Instr::VendorExecuteInstruction { payload } => {
            if let Some(access) = instruction_literal_access_map.get(&(func_idx, *payload)) {
                access_set.union_with(access);
                return;
            }
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let Some(isi) = decode_instruction_box_literal(raw) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_instruction_box_access(&isi, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::VendorExecuteQuery { payload, .. }
        | ir::Instr::QueryExecuteNorito { payload, .. } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let Some(request) = decode_query_request_literal(raw) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_query_request_access(&request, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::QueryGet { key, syscall, .. } => {
            if record_typed_query_get_access(
                *key,
                *syscall,
                string_map,
                authority_account_temps,
                func_idx,
                access_set,
            )
            .is_none()
            {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::GetAccountBalance { account, asset, .. } => {
            let account = account_access_hint_for_temp(
                string_map,
                authority_account_temps,
                func_idx,
                *account,
            );
            let asset = parse_temp::<AssetDefinitionId>(string_map, func_idx, *asset);
            match (account, asset) {
                (Some(account), Some(asset)) => {
                    add_asset_r_for_account_hint(access_set, &asset, &account);
                }
                _ => apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI),
            }
        }
        ir::Instr::GetPublicInput { .. }
        | ir::Instr::GetPrivateInput { .. }
        | ir::Instr::DebugPrint { .. }
        | ir::Instr::DebugLog { .. }
        | ir::Instr::CommitOutput => {}
        ir::Instr::UseNullifier { nullifier } => {
            let Some(raw) = int_const_map.get(&(func_idx, *nullifier)).copied() else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let Ok(value) = u64::try_from(raw) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_nullifier_rw(access_set, value);
        }
        ir::Instr::SmartContractLifecycle { payload, syscall } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_smart_contract_lifecycle_access(raw, *syscall, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::ZkRootsGet { payload, .. } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_zk_roots_get_access(raw, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::ZkVoteGetTally { payload, .. } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_zk_vote_get_tally_access(raw, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::VrfEpochSeed { payload, .. } => {
            let Some(raw) = string_map.get(&(func_idx, *payload)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_vrf_epoch_seed_access(raw, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::BuildSubmitBallotInline { .. } | ir::Instr::BuildUnshieldInline { .. } => {}
        ir::Instr::TransferDomain { domain, to } => {
            let (Some(domain), Some(to)) = (
                parse_domain_temp(string_map, func_idx, *domain),
                account_access_hint_for_temp(string_map, authority_account_temps, func_idx, *to),
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_domain_rw(access_set, &domain);
            add_account_hint_r(access_set, &to);
        }
        ir::Instr::SetNftData { nft, key, .. } => {
            let Some(id) = parse_temp(string_map, func_idx, *nft) else {
                add_nft_coarse_rw(access_set);
                return;
            };
            let Some(key) = parse_temp(string_map, func_idx, *key) else {
                add_nft_rw(access_set, &id);
                return;
            };
            add_nft_detail_rw(access_set, &id, &key);
        }
        ir::Instr::RegisterPeer { json } | ir::Instr::UnregisterPeer { json } => {
            let Some(raw) = string_map.get(&(func_idx, *json)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let Some(peer) = peer_id_from_json_literal(raw) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_peer_rw(access_set, &peer);
        }
        ir::Instr::SubscriptionBill => add_subscription_context_rw(access_set, "bill"),
        ir::Instr::SubscriptionRecordUsage => add_subscription_context_rw(access_set, "usage"),
        ir::Instr::AxtBegin { descriptor } => {
            let Some(raw) = string_map.get(&(func_idx, *descriptor)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let Some(descriptor) = decode_axt_descriptor_literal(raw) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_axt_descriptor_access(access_set, &descriptor);
        }
        ir::Instr::AxtTouch { dsid, manifest } => {
            let Some(dsid) = parse_dataspace_temp(string_map, func_idx, *dsid) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if let Some(manifest) = manifest {
                let Some(raw) = string_map.get(&(func_idx, *manifest)) else {
                    return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
                };
                let Some(manifest) = decode_axt_touch_manifest_literal(raw) else {
                    return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
                };
                add_axt_touch_manifest_access(access_set, dsid, &manifest);
            } else {
                add_axt_dataspace_rw(access_set, dsid);
            }
        }
        ir::Instr::VerifyDsProof { dsid, .. } => {
            let Some(dsid) = parse_dataspace_temp(string_map, func_idx, *dsid) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_axt_dataspace_r(access_set, dsid);
            access_set
                .reads
                .insert(format!("axt:dataspace:{}:proof", dsid.as_u64()));
        }
        ir::Instr::UseAssetHandle { handle, intent, .. } => {
            let (Some(handle_raw), Some(intent_raw)) = (
                string_map.get(&(func_idx, *handle)),
                string_map.get(&(func_idx, *intent)),
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            let (Some(handle), Some(intent)) = (
                decode_asset_handle_literal(handle_raw),
                decode_remote_spend_intent_literal(intent_raw),
            ) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            add_asset_handle_access(access_set, &handle, &intent);
        }
        ir::Instr::AxtCommit => {}
        ir::Instr::SoracloudHostCall {
            request, syscall, ..
        } => {
            let Some(raw) = string_map.get(&(func_idx, *request)) else {
                return apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            };
            if record_soracloud_request_access(raw, *syscall, access_set).is_none() {
                apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI);
            }
        }
        ir::Instr::InvokeEntrypointAs { .. }
        | ir::Instr::InvokeEntrypointAsMulti { .. }
        | ir::Instr::ExpectRejectAs { .. }
        | ir::Instr::ActorAccount { .. }
        | ir::Instr::ActorPublicKey { .. }
        | ir::Instr::ActorSign { .. } => {
            apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI)
        }
        _ => apply_fallback(access_set, hint_diagnostics, HINT_SKIP_OPAQUE_ISI),
    }
}

fn decode_norito_literal_payload(raw: &str) -> Option<Vec<u8>> {
    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    match crate::pointer_abi::validate_tlv_bytes(&bytes) {
        Ok(tlv) => {
            if tlv.type_id != PointerType::NoritoBytes {
                return None;
            }
            Some(tlv.payload.to_vec())
        }
        Err(_) => Some(bytes),
    }
}

fn decode_instruction_box_literal(raw: &str) -> Option<InstructionBox> {
    use iroha_data_model::isi::zk as DMZk;

    let payload = decode_norito_literal_payload(raw)?;
    if let Ok(instr) = norito::decode_from_bytes::<InstructionBox>(&payload) {
        return Some(instr);
    }
    if let Ok(instr) = norito::decode_from_bytes::<DMZk::CreateElection>(&payload) {
        return Some(InstructionBox::from(instr));
    }
    if let Ok(instr) = norito::decode_from_bytes::<DMZk::SubmitBallot>(&payload) {
        return Some(InstructionBox::from(instr));
    }
    if let Ok(instr) = norito::decode_from_bytes::<DMZk::FinalizeElection>(&payload) {
        return Some(InstructionBox::from(instr));
    }
    if let Ok(instr) = norito::decode_from_bytes::<DMZk::Unshield>(&payload) {
        return Some(InstructionBox::from(instr));
    }
    None
}

fn access_for_instruction_literal(raw: &str) -> Option<AccessSets> {
    let instr = decode_instruction_box_literal(raw)?;
    let mut access = AccessSets::default();
    record_instruction_box_access(&instr, &mut access)?;
    Some(access)
}

enum AnonymousEscrowRequestKind {
    OpenOffer,
    Release,
    Cancel,
    ResolveDispute,
}

fn record_anonymous_escrow_request_access(
    raw: &str,
    kind: AnonymousEscrowRequestKind,
    access_set: &mut AccessSets,
) -> Option<()> {
    use iroha_data_model::isi::escrow as DMEscrow;

    let payload = decode_norito_literal_payload(raw)?;
    match kind {
        AnonymousEscrowRequestKind::OpenOffer => {
            let request: DMEscrow::OpenAnonymousAssetEscrow =
                norito::decode_from_bytes(&payload).ok()?;
            record_anonymous_asset_escrow_open_access(
                access_set,
                &request.escrow_id,
                &request.asset_definition,
            );
        }
        AnonymousEscrowRequestKind::Release => {
            let request: DMEscrow::ReleaseAnonymousAssetEscrow =
                norito::decode_from_bytes(&payload).ok()?;
            record_anonymous_asset_escrow_close_access(access_set, &request.escrow_id);
        }
        AnonymousEscrowRequestKind::Cancel => {
            let request: DMEscrow::CancelAnonymousAssetEscrow =
                norito::decode_from_bytes(&payload).ok()?;
            record_anonymous_asset_escrow_close_access(access_set, &request.escrow_id);
        }
        AnonymousEscrowRequestKind::ResolveDispute => {
            let request: DMEscrow::ResolveAnonymousEscrowDispute =
                norito::decode_from_bytes(&payload).ok()?;
            record_anonymous_asset_escrow_close_access(access_set, &request.escrow_id);
        }
    }
    Some(())
}

fn decode_query_request_literal(raw: &str) -> Option<QueryRequest> {
    let payload = decode_norito_literal_payload(raw)?;
    norito::decode_from_bytes(&payload).ok()
}

fn record_zk_roots_get_access(raw: &str, access_set: &mut AccessSets) -> Option<()> {
    let payload = decode_norito_literal_payload(raw)?;
    let (payload, flags) = decode_norito_archive_payload(&payload)?;
    let mut fields = payload;
    let asset_field = take_norito_len_prefixed(&mut fields, flags)?;
    let max_field = take_norito_len_prefixed(&mut fields, flags)?;
    if !fields.is_empty() || decode_norito_u32_bare(max_field).is_none() {
        return None;
    }
    let asset_id = decode_norito_string_bare(asset_field, flags)?;
    let asset = asset_id.parse().ok()?;
    add_zk_asset_r(access_set, &asset);
    Some(())
}

fn record_zk_vote_get_tally_access(raw: &str, access_set: &mut AccessSets) -> Option<()> {
    let payload = decode_norito_literal_payload(raw)?;
    let (payload, flags) = decode_norito_archive_payload(&payload)?;
    let mut fields = payload;
    let election_id_field = take_norito_len_prefixed(&mut fields, flags)?;
    if !fields.is_empty() {
        return None;
    }
    let election_id = decode_norito_string_bare(election_id_field, flags)?;
    add_zk_election_tally_r(access_set, &election_id);
    Some(())
}

fn record_vrf_epoch_seed_access(raw: &str, access_set: &mut AccessSets) -> Option<()> {
    let payload = decode_norito_literal_payload(raw)?;
    let (payload, flags) = decode_norito_archive_payload(&payload)?;
    let mut fields = payload;
    let epoch_field = take_norito_len_prefixed(&mut fields, flags)?;
    let fallback_field = take_norito_len_prefixed(&mut fields, flags)?;
    if !fields.is_empty() {
        return None;
    }
    let epoch = decode_norito_u64_bare(epoch_field)?;
    let fallback_to_latest = decode_norito_bool_bare(fallback_field)?;
    access_set.reads.insert(format!("vrf:epoch_seed:{epoch}"));
    if fallback_to_latest {
        access_set.reads.insert("vrf:epoch_seed:latest".to_owned());
    }
    Some(())
}

fn record_smart_contract_lifecycle_access(
    raw: &str,
    syscall: u32,
    access_set: &mut AccessSets,
) -> Option<()> {
    use iroha_data_model::isi::smart_contract_code as DMScode;

    let payload = decode_norito_literal_payload(raw)?;
    match syscall {
        syscalls::SYSCALL_REGISTER_SMART_CONTRACT_CODE => {
            let request: DMScode::RegisterSmartContractCode =
                norito::decode_from_bytes(&payload).ok()?;
            let code_hash = request.manifest.code_hash.as_ref()?;
            add_contract_code_r(access_set, code_hash);
            add_contract_manifest_rw(access_set, code_hash);
        }
        syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES => {
            let request: DMScode::RegisterSmartContractBytes =
                norito::decode_from_bytes(&payload).ok()?;
            add_contract_code_rw(access_set, &request.code_hash);
        }
        syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE => {
            let request: DMScode::ActivateContractInstance =
                norito::decode_from_bytes(&payload).ok()?;
            add_contract_code_r(access_set, &request.code_hash);
            add_contract_manifest_r(access_set, &request.code_hash);
            add_contract_instance_rw(access_set, &request.contract_address);
            add_contract_instance_code_hash_rw(access_set, &request.code_hash);
        }
        syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES => {
            let request: DMScode::RemoveSmartContractBytes =
                norito::decode_from_bytes(&payload).ok()?;
            add_contract_code_rw(access_set, &request.code_hash);
            add_contract_manifest_r(access_set, &request.code_hash);
            add_contract_instance_code_hash_r(access_set, &request.code_hash);
        }
        _ => return None,
    }
    Some(())
}

fn record_transfer_asset_batch_access(raw: &str, access_set: &mut AccessSets) -> Option<()> {
    let payload = decode_norito_literal_payload(raw)?;
    let batch: iroha_data_model::isi::transfer::TransferAssetBatch =
        norito::decode_from_bytes(&payload).ok()?;
    record_transfer_asset_batch_entries_access(&batch, access_set)
}

fn record_transfer_asset_batch_entries_access(
    batch: &iroha_data_model::isi::transfer::TransferAssetBatch,
    access_set: &mut AccessSets,
) -> Option<()> {
    if batch.entries().is_empty() {
        return None;
    }
    for entry in batch.entries() {
        let source = AssetId::of(entry.asset_definition().clone(), entry.from().clone());
        let destination = AssetId::of(entry.asset_definition().clone(), entry.to().clone());
        add_asset_rw(access_set, &source);
        add_asset_rw(access_set, &destination);
    }
    Some(())
}

fn decode_axt_descriptor_literal(raw: &str) -> Option<crate::axt::AxtDescriptor> {
    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    norito::decode_from_bytes(&bytes).ok()
}

fn decode_axt_touch_manifest_literal(raw: &str) -> Option<crate::axt::TouchManifest> {
    let payload = decode_norito_literal_payload(raw)?;
    norito::decode_from_bytes(&payload).ok()
}

fn decode_asset_handle_literal(raw: &str) -> Option<crate::axt::AssetHandle> {
    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    norito::decode_from_bytes(&bytes).ok()
}

fn decode_remote_spend_intent_literal(raw: &str) -> Option<crate::axt::RemoteSpendIntent> {
    let payload = decode_norito_literal_payload(raw)?;
    norito::decode_from_bytes(&payload).ok()
}

fn parse_dataspace_temp(
    string_map: &HashMap<(usize, ir::Temp), String>,
    func_idx: usize,
    temp: ir::Temp,
) -> Option<iroha_data_model::nexus::DataSpaceId> {
    let raw = string_map.get(&(func_idx, temp))?;
    if let Some(raw_id) = parse_u64_literal(raw) {
        return Some(iroha_data_model::nexus::DataSpaceId::new(raw_id));
    }
    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    norito::decode_from_bytes(&bytes).ok()
}

fn public_key_from_json_value(value: &json::Value) -> Option<iroha_crypto::PublicKey> {
    if let Some(key_str) = value.as_str() {
        return key_str.parse().ok();
    }
    let map = value.as_object()?;
    let value = map
        .get("public_key")
        .or_else(|| map.get("publicKey"))
        .or_else(|| map.get("key"))?;
    public_key_from_json_value(value)
}

fn peer_id_from_json_value(value: &json::Value) -> Option<iroha_data_model::peer::PeerId> {
    if let Some(peer_str) = value.as_str() {
        if let Ok(peer_id) = peer_str.parse::<iroha_data_model::peer::PeerId>() {
            return Some(peer_id);
        }
        if let Ok(peer) = peer_str.parse::<iroha_data_model::peer::Peer>() {
            return Some(peer.id().clone());
        }
        return None;
    }
    let map = value.as_object()?;
    if let Some(value) = map
        .get("peer")
        .or_else(|| map.get("peer_id"))
        .or_else(|| map.get("peerId"))
    {
        return peer_id_from_json_value(value);
    }
    let key = map
        .get("public_key")
        .or_else(|| map.get("publicKey"))
        .or_else(|| map.get("key"))?;
    public_key_from_json_value(key).map(iroha_data_model::peer::PeerId::from)
}

fn peer_id_from_json_literal(raw: &str) -> Option<iroha_data_model::peer::PeerId> {
    let value: json::Value = json::from_slice(raw.as_bytes()).ok()?;
    peer_id_from_json_value(&value)
}

fn record_soracloud_request_access(
    raw: &str,
    syscall: u32,
    access_set: &mut AccessSets,
) -> Option<()> {
    use iroha_data_model::soracloud::{
        SoracloudHostOperationV1 as Op, SoracloudHostRequestEnvelopeV1,
        SoracloudHostRequestPayloadV1 as Payload,
    };

    let bytes = decode_hex_or_raw_bytes(raw).ok()?;
    let request: SoracloudHostRequestEnvelopeV1 = norito::decode_from_bytes(&bytes).ok()?;
    request.validate().ok()?;
    let expected = soracloud_operation_for_syscall(syscall)?;
    if request.operation != expected {
        return None;
    }
    match (&request.operation, &request.payload) {
        (Op::ReadCommittedState, Payload::ReadCommittedState(payload)) => {
            add_soracloud_state_r(access_set, &payload.binding_name, &payload.state_key);
        }
        (Op::EmitStateMutation, Payload::EmitStateMutation(payload)) => {
            add_soracloud_state_rw(access_set, &payload.binding_name, &payload.state_key);
        }
        (Op::EmitMailboxMessage, Payload::EmitMailboxMessage(payload)) => {
            access_set.writes.insert(format!(
                "soracloud:mailbox:{}:{}",
                payload.to_service, payload.to_handler
            ));
        }
        (Op::AppendJournal, Payload::AppendJournal(payload)) => {
            access_set.writes.insert(format!(
                "soracloud:journal:{}",
                soracloud_host_path_key_segment(&payload.artifact_path)
            ));
        }
        (Op::PublishCheckpoint, Payload::PublishCheckpoint(payload)) => {
            access_set.writes.insert(format!(
                "soracloud:checkpoint:{}",
                soracloud_host_path_key_segment(&payload.artifact_path)
            ));
        }
        (Op::ReadConfig, Payload::ReadConfig(payload)) => {
            access_set
                .reads
                .insert(format!("soracloud:config:{}", payload.config_name));
        }
        (Op::ReadSecretEnvelope, Payload::ReadSecretEnvelope(payload)) => {
            access_set
                .reads
                .insert(format!("soracloud:secret_envelope:{}", payload.secret_name));
        }
        (Op::ReadSecret, Payload::ReadSecret(payload)) => {
            access_set
                .reads
                .insert(format!("soracloud:node_secret:{}", payload.secret_name));
        }
        (Op::ReadCredential, Payload::ReadCredential(payload)) => {
            access_set.reads.insert(format!(
                "soracloud:node_credential:{}",
                payload.credential_name
            ));
        }
        (Op::EgressFetch, Payload::EgressFetch(payload)) => {
            access_set
                .reads
                .insert(format!("soracloud:egress:{}", payload.url));
        }
        _ => return None,
    }
    Some(())
}

fn soracloud_host_path_key_segment(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}

fn soracloud_operation_for_syscall(
    syscall: u32,
) -> Option<iroha_data_model::soracloud::SoracloudHostOperationV1> {
    use iroha_data_model::soracloud::SoracloudHostOperationV1 as Op;
    match syscall {
        syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE => Some(Op::ReadCommittedState),
        syscalls::SYSCALL_SORACLOUD_EMIT_STATE_MUTATION => Some(Op::EmitStateMutation),
        syscalls::SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE => Some(Op::EmitMailboxMessage),
        syscalls::SYSCALL_SORACLOUD_APPEND_JOURNAL => Some(Op::AppendJournal),
        syscalls::SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT => Some(Op::PublishCheckpoint),
        syscalls::SYSCALL_SORACLOUD_READ_SECRET => Some(Op::ReadSecret),
        syscalls::SYSCALL_SORACLOUD_READ_CREDENTIAL => Some(Op::ReadCredential),
        syscalls::SYSCALL_SORACLOUD_EGRESS_FETCH => Some(Op::EgressFetch),
        syscalls::SYSCALL_SORACLOUD_READ_CONFIG => Some(Op::ReadConfig),
        syscalls::SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE => Some(Op::ReadSecretEnvelope),
        _ => None,
    }
}

fn decode_norito_archive_payload(bytes: &[u8]) -> Option<(&[u8], u8)> {
    if bytes.len() < norito::core::Header::SIZE || &bytes[0..4] != b"NRT0" || bytes[22] != 0 {
        return None;
    }
    let len = u64::from_le_bytes(bytes[23..31].try_into().ok()?);
    let payload = &bytes[norito::core::Header::SIZE..];
    if payload.len() != usize::try_from(len).ok()? {
        return None;
    }
    let flags = bytes[39];
    norito::core::validate_header_flags(flags).ok()?;
    Some((payload, flags))
}

fn take_norito_len_prefixed<'a>(bytes: &mut &'a [u8], flags: u8) -> Option<&'a [u8]> {
    let (len, header_len) = norito::core::read_len_from_slice_with_flags(bytes, flags).ok()?;
    let end = header_len.checked_add(len)?;
    if bytes.len() < end {
        return None;
    }
    let field = &bytes[header_len..end];
    *bytes = &bytes[end..];
    Some(field)
}

fn decode_norito_string_bare(bytes: &[u8], flags: u8) -> Option<String> {
    let mut bytes = bytes;
    let raw = take_norito_len_prefixed(&mut bytes, flags)?;
    if !bytes.is_empty() {
        return None;
    }
    std::str::from_utf8(raw).ok().map(ToOwned::to_owned)
}

fn decode_norito_u32_bare(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 4 {
        return None;
    }
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn decode_norito_u64_bare(bytes: &[u8]) -> Option<u64> {
    if bytes.len() != 8 {
        return None;
    }
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

fn decode_norito_bool_bare(bytes: &[u8]) -> Option<bool> {
    if bytes.len() != 1 {
        return None;
    }
    Some(bytes[0] != 0)
}

#[allow(clippy::too_many_arguments)]
fn submit_ballot_inline_instruction_literal(
    string_map: &HashMap<(usize, ir::Temp), String>,
    func_idx: usize,
    election_id: ir::Temp,
    ciphertext: ir::Temp,
    nullifier: ir::Temp,
    backend: ir::Temp,
    proof: ir::Temp,
    vk: ir::Temp,
) -> Option<String> {
    use iroha_data_model::{
        isi::zk as DMZk,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    };

    let literal = |temp| string_map.get(&(func_idx, temp)).cloned();
    let eid = literal(election_id)?;
    let backend_str = literal(backend)?;
    let ct_bytes = decode_hex_or_raw_bytes(&literal(ciphertext)?).ok()?;
    let nf_bytes = decode_hex_or_raw_bytes(&literal(nullifier)?).ok()?;
    let null32: [u8; 32] = nf_bytes.try_into().ok()?;
    let proof_bytes = decode_hex_or_raw_bytes(&literal(proof)?).ok()?;
    let vk_ref = literal(vk)?;
    let ballot_proof = ProofAttachment::new_ref(
        backend_str.clone(),
        ProofBox::new(backend_str.clone(), proof_bytes),
        VerifyingKeyId::new(backend_str, vk_ref),
    );
    let submit = DMZk::SubmitBallot {
        election_id: eid,
        ciphertext: ct_bytes,
        ballot_proof,
        nullifier: null32,
    };
    let boxed = InstructionBox::from(submit);
    let bytes = norito::to_bytes(&boxed).ok()?;
    Some(format!("0x{}", hex::encode(bytes)))
}

#[allow(clippy::too_many_arguments)]
fn unshield_inline_instruction_literal(
    string_map: &HashMap<(usize, ir::Temp), String>,
    int_const_map: &HashMap<(usize, ir::Temp), i64>,
    func_idx: usize,
    asset: ir::Temp,
    to: ir::Temp,
    amount: ir::Temp,
    inputs: ir::Temp,
    outputs: Option<ir::Temp>,
    backend: ir::Temp,
    proof: ir::Temp,
    vk: ir::Temp,
) -> Option<String> {
    use iroha_data_model::{
        isi::zk as DMZk,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    };

    let literal = |temp| string_map.get(&(func_idx, temp)).cloned();
    let asset_id = AssetDefinitionId::parse_address_literal(&literal(asset)?).ok()?;
    let account = AccountId::parse_encoded(&literal(to)?)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .ok()?;
    let public_amount = u128::try_from(*int_const_map.get(&(func_idx, amount))?).ok()?;
    let inputs = decode_fixed32_chunks(&literal(inputs)?, "inputs", false).ok()?;
    let outputs = if let Some(outputs) = outputs {
        decode_fixed32_chunks(&literal(outputs)?, "outputs", true).ok()?
    } else {
        Vec::new()
    };
    let backend_str = literal(backend)?;
    let proof_bytes = decode_hex_or_raw_bytes(&literal(proof)?).ok()?;
    let vk_ref = literal(vk)?;
    let proof = ProofAttachment::new_ref(
        backend_str.clone(),
        ProofBox::new(backend_str.clone(), proof_bytes),
        VerifyingKeyId::new(backend_str, vk_ref),
    );
    let unshield = DMZk::Unshield {
        asset: asset_id,
        to: account,
        public_amount,
        inputs,
        outputs,
        proof,
        root_hint: None,
    };
    let boxed = InstructionBox::from(unshield);
    let bytes = norito::to_bytes(&boxed).ok()?;
    Some(format!("0x{}", hex::encode(bytes)))
}

fn record_instruction_box_access(
    instr: &InstructionBox,
    access_set: &mut AccessSets,
) -> Option<()> {
    let any = instr.as_any();

    if any.downcast_ref::<Log>().is_some() {
        return Some(());
    }

    if let Some(instr) = any.downcast_ref::<iroha_data_model::isi::zk::CreateElection>() {
        add_zk_election_w(access_set, instr.election_id());
        return Some(());
    }
    if let Some(instr) = any.downcast_ref::<iroha_data_model::isi::zk::SubmitBallot>() {
        add_zk_election_submit_w(access_set, instr.election_id());
        return Some(());
    }
    if let Some(instr) = any.downcast_ref::<iroha_data_model::isi::zk::FinalizeElection>() {
        add_zk_election_tally_w(access_set, instr.election_id());
        return Some(());
    }
    if let Some(instr) = any.downcast_ref::<iroha_data_model::isi::zk::Unshield>() {
        let asset = AssetId::of(instr.asset().clone(), instr.to().clone());
        add_asset_rw(access_set, &asset);
        add_zk_asset_rw(access_set, instr.asset());
        let Ok(key) = "zk.unshield.last".parse::<Name>() else {
            return None;
        };
        add_asset_def_detail_rw(access_set, instr.asset(), &key);
        return Some(());
    }
    if let Some(instr) = any.downcast_ref::<iroha_data_model::isi::transfer::TransferAssetBatch>() {
        return record_transfer_asset_batch_entries_access(instr, access_set);
    }

    {
        use iroha_data_model::isi::escrow as DMEscrow;

        if let Some(instr) = any.downcast_ref::<DMEscrow::OpenAssetEscrow>() {
            record_asset_escrow_open_access(
                access_set,
                &instr.escrow_id,
                Some(&instr.asset_definition),
            );
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::AcceptAssetEscrow>() {
            record_asset_escrow_lifecycle_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::MarkEscrowPaymentSent>() {
            record_asset_escrow_lifecycle_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::ReleaseAssetEscrow>() {
            record_asset_escrow_close_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::CancelAssetEscrow>() {
            record_asset_escrow_close_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::OpenEscrowDispute>() {
            record_asset_escrow_lifecycle_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::ResolveEscrowDispute>() {
            record_asset_escrow_close_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::OpenAnonymousAssetEscrow>() {
            record_anonymous_asset_escrow_open_access(
                access_set,
                &instr.escrow_id,
                &instr.asset_definition,
            );
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::AcceptAnonymousAssetEscrow>() {
            record_anonymous_asset_escrow_lifecycle_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::MarkAnonymousEscrowPaymentSent>() {
            record_anonymous_asset_escrow_lifecycle_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::ReleaseAnonymousAssetEscrow>() {
            record_anonymous_asset_escrow_close_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::CancelAnonymousAssetEscrow>() {
            record_anonymous_asset_escrow_close_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::OpenAnonymousEscrowDispute>() {
            record_anonymous_asset_escrow_lifecycle_access(access_set, &instr.escrow_id);
            return Some(());
        }
        if let Some(instr) = any.downcast_ref::<DMEscrow::ResolveAnonymousEscrowDispute>() {
            record_anonymous_asset_escrow_close_access(access_set, &instr.escrow_id);
            return Some(());
        }
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
                add_account_rw(access_set, r.object.id());
            }
            RegisterBox::AssetDefinition(r) => {
                add_asset_def_domain_r_if_projected(access_set, r.object.id());
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
        SingularQueryBox::FindAssetDefinitionById(q) => {
            add_asset_def_r(access_set, q.asset_definition_id());
            Some(())
        }
        _ => None,
    }
}

fn record_typed_query_get_access(
    key: ir::Temp,
    syscall: u32,
    string_map: &HashMap<(usize, ir::Temp), String>,
    authority_account_temps: &HashSet<(usize, ir::Temp)>,
    func_idx: usize,
    access_set: &mut AccessSets,
) -> Option<()> {
    match syscall {
        syscalls::SYSCALL_QUERY_GET_ACCOUNT => {
            let account =
                account_access_hint_for_temp(string_map, authority_account_temps, func_idx, key)?;
            add_account_hint_r(access_set, &account);
            Some(())
        }
        syscalls::SYSCALL_QUERY_GET_ASSET => {
            let id = parse_temp::<AssetId>(string_map, func_idx, key)?;
            add_asset_r(access_set, &id);
            Some(())
        }
        syscalls::SYSCALL_QUERY_GET_ASSET_DEFINITION => {
            let id = parse_temp::<AssetDefinitionId>(string_map, func_idx, key)?;
            add_asset_def_r(access_set, &id);
            Some(())
        }
        syscalls::SYSCALL_QUERY_GET_DOMAIN => {
            let id = parse_domain_temp(string_map, func_idx, key)?;
            add_domain_r(access_set, &id);
            Some(())
        }
        syscalls::SYSCALL_QUERY_GET_NFT => {
            let id = parse_temp::<NftId>(string_map, func_idx, key)?;
            add_nft_r(access_set, &id);
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

fn escrow_id_from_name_temp(
    string_map: &HashMap<(usize, ir::Temp), String>,
    func_idx: usize,
    temp: ir::Temp,
) -> Option<EscrowId> {
    parse_temp::<Name>(string_map, func_idx, temp).map(|name| EscrowId::from_kotodama_name(&name))
}

fn parse_domain_temp(
    string_map: &HashMap<(usize, ir::Temp), String>,
    func_idx: usize,
    temp: ir::Temp,
) -> Option<iroha_data_model::domain::DomainId> {
    iroha_data_model::domain::DomainId::parse_fully_qualified(string_map.get(&(func_idx, temp))?)
        .ok()
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

fn collect_function_return_literal_facts(
    ir_prog: &ir::Program,
    string_map: &HashMap<(usize, ir::Temp), String>,
    dataref_kind_map: &HashMap<(usize, ir::Temp), ir::DataRefKind>,
    string_literal_temps: &HashSet<(usize, ir::Temp)>,
) -> HashMap<String, LiteralPointerFact> {
    let mut out = HashMap::new();
    for (func_idx, func) in ir_prog.functions.iter().enumerate() {
        let mut reachable = HashSet::new();
        let mut stack = vec![func.entry];
        while let Some(label) = stack.pop() {
            if !reachable.insert(label) {
                continue;
            }
            let Some(bb) = func.blocks.iter().find(|bb| bb.label == label) else {
                continue;
            };
            match &bb.terminator {
                ir::Terminator::Jump(next) => stack.push(*next),
                ir::Terminator::Branch {
                    then_bb, else_bb, ..
                } => {
                    stack.push(*then_bb);
                    stack.push(*else_bb);
                }
                ir::Terminator::Return(_)
                | ir::Terminator::Return2(_, _)
                | ir::Terminator::ReturnN(_) => {}
            }
        }
        let mut fact: Option<LiteralPointerFact> = None;
        let mut saw_return = false;
        let mut incompatible = false;
        for bb in &func.blocks {
            if !reachable.contains(&bb.label) {
                continue;
            }
            match &bb.terminator {
                ir::Terminator::Return(Some(temp)) => {
                    saw_return = true;
                    let key = (func_idx, *temp);
                    let Some(raw) = string_map.get(&key).cloned() else {
                        incompatible = true;
                        break;
                    };
                    let Some(kind) = dataref_kind_map.get(&key).copied() else {
                        incompatible = true;
                        break;
                    };
                    let candidate = LiteralPointerFact {
                        raw,
                        kind,
                        is_string_literal: string_literal_temps.contains(&key),
                    };
                    match &fact {
                        Some(existing) if existing != &candidate => {
                            incompatible = true;
                            break;
                        }
                        Some(_) => {}
                        None => fact = Some(candidate),
                    }
                }
                ir::Terminator::Return(None)
                | ir::Terminator::Return2(_, _)
                | ir::Terminator::ReturnN(_) => {
                    incompatible = true;
                    break;
                }
                ir::Terminator::Jump(_) | ir::Terminator::Branch { .. } => {}
            }
        }
        if saw_return
            && !incompatible
            && let Some(fact) = fact
        {
            out.insert(func.name.clone(), fact);
        }
    }
    out
}

fn propagate_function_return_literal_facts(
    ir_prog: &ir::Program,
    string_map: &mut HashMap<(usize, ir::Temp), String>,
    dataref_kind_map: &mut HashMap<(usize, ir::Temp), ir::DataRefKind>,
    string_literal_temps: &mut HashSet<(usize, ir::Temp)>,
    multi_copy_dests: &HashSet<(usize, ir::Temp)>,
) {
    loop {
        let facts = collect_function_return_literal_facts(
            ir_prog,
            string_map,
            dataref_kind_map,
            string_literal_temps,
        );
        if facts.is_empty() {
            return;
        }
        let mut changed = false;
        for (func_idx, func) in ir_prog.functions.iter().enumerate() {
            for bb in &func.blocks {
                for instr in &bb.instrs {
                    let Some((callee, dest)) = (match instr {
                        ir::Instr::Call {
                            callee,
                            dest: Some(dest),
                            ..
                        } => Some((callee.as_str(), *dest)),
                        _ => None,
                    }) else {
                        continue;
                    };
                    let Some(fact) = facts.get(callee) else {
                        continue;
                    };
                    let dest_key = (func_idx, dest);
                    if string_map.get(&dest_key) != Some(&fact.raw) {
                        string_map.insert(dest_key, fact.raw.clone());
                        changed = true;
                    }
                    if dataref_kind_map.get(&dest_key).copied() != Some(fact.kind) {
                        dataref_kind_map.insert(dest_key, fact.kind);
                        changed = true;
                    }
                    if fact.is_string_literal {
                        changed |= string_literal_temps.insert(dest_key);
                    } else {
                        changed |= string_literal_temps.remove(&dest_key);
                    }
                }
            }
        }
        changed |= propagate_literal_copy_facts(
            ir_prog,
            string_map,
            dataref_kind_map,
            string_literal_temps,
            multi_copy_dests,
        );
        if !changed {
            return;
        }
    }
}

fn propagate_literal_copy_facts(
    ir_prog: &ir::Program,
    string_map: &mut HashMap<(usize, ir::Temp), String>,
    dataref_kind_map: &mut HashMap<(usize, ir::Temp), ir::DataRefKind>,
    string_literal_temps: &mut HashSet<(usize, ir::Temp)>,
    multi_copy_dests: &HashSet<(usize, ir::Temp)>,
) -> bool {
    let mut changed = false;
    for (func_idx, func) in ir_prog.functions.iter().enumerate() {
        for bb in &func.blocks {
            for instr in &bb.instrs {
                let ir::Instr::Copy { dest, src } = instr else {
                    continue;
                };
                if dest == src {
                    continue;
                }
                let dest_key = (func_idx, *dest);
                if multi_copy_dests.contains(&dest_key) {
                    continue;
                }
                let src_key = (func_idx, *src);
                let Some(raw) = string_map.get(&src_key).cloned() else {
                    continue;
                };
                let Some(kind) = dataref_kind_map.get(&src_key).copied() else {
                    continue;
                };
                if string_map.get(&dest_key) != Some(&raw) {
                    string_map.insert(dest_key, raw);
                    changed = true;
                }
                if dataref_kind_map.get(&dest_key).copied() != Some(kind) {
                    dataref_kind_map.insert(dest_key, kind);
                    changed = true;
                }
                if string_literal_temps.contains(&src_key) {
                    changed |= string_literal_temps.insert(dest_key);
                } else {
                    changed |= string_literal_temps.remove(&dest_key);
                }
            }
        }
    }
    changed
}

fn account_access_hint_for_temp(
    string_map: &HashMap<(usize, ir::Temp), String>,
    authority_account_temps: &HashSet<(usize, ir::Temp)>,
    func_idx: usize,
    temp: ir::Temp,
) -> Option<AccountAccessHint> {
    if authority_account_temps.contains(&(func_idx, temp)) {
        return Some(AccountAccessHint::Authority);
    }
    parse_account_temp(string_map, func_idx, temp).map(AccountAccessHint::Literal)
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

fn key_account_hint(account: &AccountAccessHint) -> String {
    match account {
        AccountAccessHint::Literal(id) => key_account(id),
        AccountAccessHint::Authority => AUTHORITY_ACCOUNT_KEY.to_owned(),
    }
}

fn key_domain(id: &DomainId) -> String {
    format!("domain:{id}")
}

fn key_asset_def(id: &AssetDefinitionId) -> String {
    format!("asset_def:{id}")
}

fn key_escrow_id(id: &EscrowId) -> String {
    format!("escrow_id:{}", hex::encode(id.as_hash().as_ref()))
}

fn key_asset_escrow(id: &EscrowId) -> String {
    format!("asset_escrow:{}", hex::encode(id.as_hash().as_ref()))
}

fn key_anonymous_asset_escrow(id: &EscrowId) -> String {
    format!(
        "anonymous_asset_escrow:{}",
        hex::encode(id.as_hash().as_ref())
    )
}

fn key_asset(id: &AssetId) -> String {
    format!("asset:{id}")
}

fn key_asset_for_account_hint(
    definition: &AssetDefinitionId,
    account: &AccountAccessHint,
) -> String {
    match account {
        AccountAccessHint::Literal(account) => {
            key_asset(&AssetId::of(definition.clone(), account.clone()))
        }
        AccountAccessHint::Authority => format!("asset:{definition}:{AUTHORITY_PLACEHOLDER}"),
    }
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

fn key_role_binding_hint(account: &AccountAccessHint, role: &RoleId) -> String {
    match account {
        AccountAccessHint::Literal(account) => key_role_binding(account, role),
        AccountAccessHint::Authority => format!("role.binding:{AUTHORITY_PLACEHOLDER}:{role}"),
    }
}

fn key_perm_account(account: &AccountId, perm: &str) -> String {
    format!("perm.account:{account}:{perm}")
}

fn key_perm_account_hint(account: &AccountAccessHint, perm: &str) -> String {
    match account {
        AccountAccessHint::Literal(account) => key_perm_account(account, perm),
        AccountAccessHint::Authority => format!("perm.account:{AUTHORITY_PLACEHOLDER}:{perm}"),
    }
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

fn key_zk_asset(id: &AssetDefinitionId) -> String {
    format!("zk_asset:{id}")
}

fn key_peer(id: &iroha_data_model::peer::PeerId) -> String {
    format!("peer:{id}")
}

fn key_contract_manifest(code_hash: &iroha_crypto::Hash) -> String {
    format!("contract.manifest:{code_hash}")
}

fn key_contract_code(code_hash: &iroha_crypto::Hash) -> String {
    format!("contract.code:{code_hash}")
}

fn key_contract_instance(address: &iroha_data_model::smart_contract::ContractAddress) -> String {
    format!("contract.instance:{address}")
}

fn key_contract_instance_code_hash(code_hash: &iroha_crypto::Hash) -> String {
    format!("contract.instance.code_hash:{code_hash}")
}

fn key_nullifier(value: u64) -> String {
    format!("nullifier:{value}")
}

fn key_nft_detail(id: &NftId, key: &Name) -> String {
    format!("nft.detail:{id}:{key}")
}

fn add_account_r(set: &mut AccessSets, id: &AccountId) {
    set.reads.insert(ACCOUNT_WILDCARD_KEY.to_string());
    set.reads.insert(key_account(id));
}

fn add_account_hint_r(set: &mut AccessSets, account: &AccountAccessHint) {
    set.reads.insert(key_account_hint(account));
}

fn add_domain_r(set: &mut AccessSets, id: &DomainId) {
    set.reads.insert(key_domain(id));
}

fn add_account_rw(set: &mut AccessSets, id: &AccountId) {
    set.reads.insert(ACCOUNT_WILDCARD_KEY.to_string());
    set.writes.insert(ACCOUNT_WILDCARD_KEY.to_string());
    let key = key_account(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_account_hint_rw(set: &mut AccessSets, account: &AccountAccessHint) {
    let key = key_account_hint(account);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_account_detail_rw(set: &mut AccessSets, id: &AccountId, key: &Name) {
    add_account_r(set, id);
    let detail = key_account_detail(id, key);
    set.reads.insert(detail.clone());
    set.writes.insert(detail);
}

fn add_account_detail_hint_rw(set: &mut AccessSets, account: &AccountAccessHint, key: &Name) {
    add_account_hint_r(set, account);
    let detail = match account {
        AccountAccessHint::Literal(id) => key_account_detail(id, key),
        AccountAccessHint::Authority => format!("account.detail:{AUTHORITY_PLACEHOLDER}:{key}"),
    };
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
    set.reads.insert(ASSET_DEF_WILDCARD_KEY.to_string());
    set.writes.insert(ASSET_DEF_WILDCARD_KEY.to_string());
    let key = key_asset_def(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_asset_def_r(set: &mut AccessSets, id: &AssetDefinitionId) {
    set.reads.insert(ASSET_DEF_WILDCARD_KEY.to_string());
    set.reads.insert(key_asset_def(id));
}

fn add_asset_def_domain_r_if_projected(set: &mut AccessSets, id: &AssetDefinitionId) {
    if let Some(domain) = id.try_domain() {
        add_domain_r(set, domain);
    }
}

fn add_asset_r(set: &mut AccessSets, id: &AssetId) {
    set.reads.insert(key_asset(id));
    add_account_r(set, id.account());
    add_asset_def_domain_r_if_projected(set, id.definition());
    add_asset_def_r(set, id.definition());
}

fn add_asset_r_for_account_hint(
    set: &mut AccessSets,
    definition: &AssetDefinitionId,
    account: &AccountAccessHint,
) {
    set.reads
        .insert(key_asset_for_account_hint(definition, account));
    add_account_hint_r(set, account);
    add_asset_def_domain_r_if_projected(set, definition);
    add_asset_def_r(set, definition);
}

fn add_asset_def_detail_rw(set: &mut AccessSets, id: &AssetDefinitionId, key: &Name) {
    add_asset_def_r(set, id);
    let detail = key_asset_def_detail(id, key);
    set.reads.insert(detail.clone());
    set.writes.insert(detail);
}

fn add_zk_asset_rw(set: &mut AccessSets, id: &AssetDefinitionId) {
    let key = key_zk_asset(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_zk_asset_r(set: &mut AccessSets, id: &AssetDefinitionId) {
    set.reads.insert(key_zk_asset(id));
}

fn add_dynamic_zk_asset_rw(set: &mut AccessSets) {
    set.reads.insert(ZK_ASSET_WILDCARD_KEY.to_string());
    set.writes.insert(ZK_ASSET_WILDCARD_KEY.to_string());
}

fn add_escrow_id_rw(set: &mut AccessSets, id: &EscrowId) {
    let key = key_escrow_id(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_asset_escrow_rw(set: &mut AccessSets, id: &EscrowId) {
    add_escrow_id_rw(set, id);
    let key = key_asset_escrow(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_anonymous_asset_escrow_rw(set: &mut AccessSets, id: &EscrowId) {
    add_escrow_id_rw(set, id);
    let key = key_anonymous_asset_escrow(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_peer_rw(set: &mut AccessSets, id: &iroha_data_model::peer::PeerId) {
    let key = key_peer(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_contract_manifest_r(set: &mut AccessSets, code_hash: &iroha_crypto::Hash) {
    set.reads.insert(key_contract_manifest(code_hash));
}

fn add_contract_manifest_rw(set: &mut AccessSets, code_hash: &iroha_crypto::Hash) {
    let key = key_contract_manifest(code_hash);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_contract_code_r(set: &mut AccessSets, code_hash: &iroha_crypto::Hash) {
    set.reads.insert(key_contract_code(code_hash));
}

fn add_contract_code_rw(set: &mut AccessSets, code_hash: &iroha_crypto::Hash) {
    let key = key_contract_code(code_hash);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_contract_instance_rw(
    set: &mut AccessSets,
    address: &iroha_data_model::smart_contract::ContractAddress,
) {
    let key = key_contract_instance(address);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_contract_instance_code_hash_r(set: &mut AccessSets, code_hash: &iroha_crypto::Hash) {
    set.reads.insert(key_contract_instance_code_hash(code_hash));
}

fn add_contract_instance_code_hash_rw(set: &mut AccessSets, code_hash: &iroha_crypto::Hash) {
    let key = key_contract_instance_code_hash(code_hash);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_nullifier_rw(set: &mut AccessSets, value: u64) {
    let key = key_nullifier(value);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_zk_election_w(set: &mut AccessSets, election_id: &str) {
    set.writes.insert(format!("zk:election:{election_id}"));
}

fn add_zk_election_submit_w(set: &mut AccessSets, election_id: &str) {
    set.writes
        .insert(format!("zk:election:{election_id}:ciphertexts"));
    set.writes
        .insert(format!("zk:election:{election_id}:nullifiers"));
}

fn add_zk_election_tally_w(set: &mut AccessSets, election_id: &str) {
    set.writes
        .insert(format!("zk:election:{election_id}:tally"));
}

fn add_zk_election_tally_r(set: &mut AccessSets, election_id: &str) {
    set.reads.insert(format!("zk:election:{election_id}:tally"));
}

fn add_asset_rw(set: &mut AccessSets, id: &AssetId) {
    set.reads.insert(ASSET_WILDCARD_KEY.to_string());
    set.writes.insert(ASSET_WILDCARD_KEY.to_string());
    let key = key_asset(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
    add_account_r(set, id.account());
    add_asset_def_domain_r_if_projected(set, id.definition());
    add_asset_def_r(set, id.definition());
}

fn add_asset_rw_for_account_hint(
    set: &mut AccessSets,
    definition: &AssetDefinitionId,
    account: &AccountAccessHint,
) {
    set.reads.insert(ASSET_WILDCARD_KEY.to_string());
    set.writes.insert(ASSET_WILDCARD_KEY.to_string());
    let key = key_asset_for_account_hint(definition, account);
    set.reads.insert(key.clone());
    set.writes.insert(key);
    add_account_hint_r(set, account);
    add_asset_def_domain_r_if_projected(set, definition);
    add_asset_def_r(set, definition);
}

fn add_dynamic_asset_account_rw(set: &mut AccessSets, definition: &AssetDefinitionId) {
    set.reads.insert(ASSET_WILDCARD_KEY.to_string());
    set.writes.insert(ASSET_WILDCARD_KEY.to_string());
    set.reads.insert(ACCOUNT_WILDCARD_KEY.to_string());
    add_asset_def_domain_r_if_projected(set, definition);
    add_asset_def_rw(set, definition);
}

fn add_dynamic_asset_definition_rw(set: &mut AccessSets) {
    set.reads.insert(ASSET_WILDCARD_KEY.to_string());
    set.writes.insert(ASSET_WILDCARD_KEY.to_string());
    set.reads.insert(ASSET_DEF_WILDCARD_KEY.to_string());
    set.writes.insert(ASSET_DEF_WILDCARD_KEY.to_string());
}

fn add_dynamic_asset_definition_rw_for_optional_account_hint(
    set: &mut AccessSets,
    account: Option<&AccountAccessHint>,
) {
    add_dynamic_asset_definition_rw(set);
    if let Some(account) = account {
        add_account_hint_r(set, account);
    } else {
        set.reads.insert(ACCOUNT_WILDCARD_KEY.to_string());
    }
}

fn add_asset_rw_for_optional_account_hint(
    set: &mut AccessSets,
    definition: &AssetDefinitionId,
    account: Option<&AccountAccessHint>,
) {
    if let Some(account) = account {
        add_asset_rw_for_account_hint(set, definition, account);
    } else {
        add_dynamic_asset_account_rw(set, definition);
    }
}

fn add_nft_rw(set: &mut AccessSets, id: &NftId) {
    add_nft_coarse_rw(set);
    let key = key_nft(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_nft_r(set: &mut AccessSets, id: &NftId) {
    set.reads.insert(NFT_COARSE_KEY.to_string());
    set.reads.insert(key_nft(id));
}

fn add_nft_coarse_rw(set: &mut AccessSets) {
    set.reads.insert(NFT_COARSE_KEY.to_string());
    set.writes.insert(NFT_COARSE_KEY.to_string());
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

fn add_role_binding_hint_w(set: &mut AccessSets, account: &AccountAccessHint, role: &RoleId) {
    set.writes.insert(key_role_binding_hint(account, role));
}

fn add_permission_account_w(set: &mut AccessSets, account: &AccountId, perm: &str) {
    set.writes.insert(key_perm_account(account, perm));
}

fn add_permission_account_hint_w(set: &mut AccessSets, account: &AccountAccessHint, perm: &str) {
    set.writes.insert(key_perm_account_hint(account, perm));
}

fn add_permission_role_w(set: &mut AccessSets, role: &RoleId, perm: &str) {
    set.writes.insert(key_perm_role(role, perm));
}

fn add_subscription_context_rw(set: &mut AccessSets, kind: &str) {
    let key = format!("subscription:trigger_context:{kind}");
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_soracloud_state_r(set: &mut AccessSets, binding: &Name, state_key: &str) {
    set.reads.insert(format!(
        "soracloud:state:{binding}:{}",
        soracloud_host_path_key_segment(state_key)
    ));
}

fn add_soracloud_state_rw(set: &mut AccessSets, binding: &Name, state_key: &str) {
    let key = format!(
        "soracloud:state:{binding}:{}",
        soracloud_host_path_key_segment(state_key)
    );
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn axt_dataspace_key(dsid: iroha_data_model::nexus::DataSpaceId) -> String {
    format!("axt:dataspace:{}", dsid.as_u64())
}

fn add_axt_dataspace_r(set: &mut AccessSets, dsid: iroha_data_model::nexus::DataSpaceId) {
    set.reads.insert(axt_dataspace_key(dsid));
}

fn add_axt_dataspace_rw(set: &mut AccessSets, dsid: iroha_data_model::nexus::DataSpaceId) {
    let key = axt_dataspace_key(dsid);
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_axt_touch_key_r(
    set: &mut AccessSets,
    dsid: iroha_data_model::nexus::DataSpaceId,
    key: &str,
) {
    add_axt_dataspace_r(set, dsid);
    set.reads
        .insert(format!("axt:dataspace:{}:{key}", dsid.as_u64()));
}

fn add_axt_touch_key_rw(
    set: &mut AccessSets,
    dsid: iroha_data_model::nexus::DataSpaceId,
    key: &str,
) {
    add_axt_dataspace_rw(set, dsid);
    let key = format!("axt:dataspace:{}:{key}", dsid.as_u64());
    set.reads.insert(key.clone());
    set.writes.insert(key);
}

fn add_axt_touch_manifest_access(
    set: &mut AccessSets,
    dsid: iroha_data_model::nexus::DataSpaceId,
    manifest: &crate::axt::TouchManifest,
) {
    add_axt_dataspace_rw(set, dsid);
    for key in &manifest.read {
        add_axt_touch_key_r(set, dsid, key);
    }
    for key in &manifest.write {
        add_axt_touch_key_rw(set, dsid, key);
    }
}

fn add_axt_descriptor_access(set: &mut AccessSets, descriptor: &crate::axt::AxtDescriptor) {
    for dsid in &descriptor.dsids {
        add_axt_dataspace_rw(set, *dsid);
    }
    for touch in &descriptor.touches {
        add_axt_dataspace_rw(set, touch.dsid);
        for key in &touch.read {
            add_axt_touch_key_r(set, touch.dsid, key);
        }
        for key in &touch.write {
            add_axt_touch_key_rw(set, touch.dsid, key);
        }
    }
}

fn add_asset_handle_access(
    set: &mut AccessSets,
    handle: &crate::axt::AssetHandle,
    intent: &crate::axt::RemoteSpendIntent,
) {
    let encoded = norito::to_bytes(handle).expect("AssetHandle encoding is infallible");
    let digest: [u8; 32] = iroha_crypto::Hash::new(&encoded).into();
    let handle_key = format!("axt:asset_handle:{}", hex::encode(digest));
    set.reads.insert(handle_key.clone());
    set.writes.insert(handle_key);
    add_axt_dataspace_rw(set, intent.asset_dsid);
    if let Some(origin) = handle.subject.origin_dsid {
        add_axt_dataspace_rw(set, origin);
    }
}

fn add_trigger_rw(set: &mut AccessSets, id: &TriggerId) {
    let key = key_trigger(id);
    set.reads.insert(key.clone());
    set.writes.insert(key);
    set.writes.insert(key_trigger_repetitions(id));
}

fn record_asset_escrow_open_access(
    set: &mut AccessSets,
    escrow_id: &EscrowId,
    asset_definition: Option<&AssetDefinitionId>,
) {
    add_asset_escrow_rw(set, escrow_id);
    set.reads.insert(ACCOUNT_WILDCARD_KEY.to_string());
    set.writes.insert(ACCOUNT_WILDCARD_KEY.to_string());
    if let Some(asset_definition) = asset_definition {
        add_asset_def_domain_r_if_projected(set, asset_definition);
        add_asset_rw_for_account_hint(set, asset_definition, &AccountAccessHint::Authority);
        add_dynamic_asset_account_rw(set, asset_definition);
    } else {
        add_dynamic_asset_definition_rw_for_optional_account_hint(
            set,
            Some(&AccountAccessHint::Authority),
        );
    }
}

fn record_asset_escrow_lifecycle_access(set: &mut AccessSets, escrow_id: &EscrowId) {
    add_asset_escrow_rw(set, escrow_id);
}

fn record_asset_escrow_close_access(set: &mut AccessSets, escrow_id: &EscrowId) {
    add_asset_escrow_rw(set, escrow_id);
    add_dynamic_asset_definition_rw(set);
}

fn record_anonymous_asset_escrow_open_access(
    set: &mut AccessSets,
    escrow_id: &EscrowId,
    asset_definition: &AssetDefinitionId,
) {
    add_anonymous_asset_escrow_rw(set, escrow_id);
    add_asset_def_domain_r_if_projected(set, asset_definition);
    add_asset_def_r(set, asset_definition);
    add_zk_asset_rw(set, asset_definition);
}

fn record_anonymous_asset_escrow_lifecycle_access(set: &mut AccessSets, escrow_id: &EscrowId) {
    add_anonymous_asset_escrow_rw(set, escrow_id);
}

fn record_anonymous_asset_escrow_close_access(set: &mut AccessSets, escrow_id: &EscrowId) {
    add_anonymous_asset_escrow_rw(set, escrow_id);
    add_dynamic_zk_asset_rw(set);
}

fn instr_queues_isi(instr: &ir::Instr) -> bool {
    matches!(
        instr,
        ir::Instr::RegisterAsset { .. }
            | ir::Instr::CreateNewAsset { .. }
            | ir::Instr::TransferAsset { .. }
            | ir::Instr::EscrowOpenOffer { .. }
            | ir::Instr::EscrowAccept { .. }
            | ir::Instr::EscrowMarkPaymentSent { .. }
            | ir::Instr::EscrowRelease { .. }
            | ir::Instr::EscrowCancel { .. }
            | ir::Instr::EscrowOpenDispute { .. }
            | ir::Instr::EscrowResolveDispute { .. }
            | ir::Instr::AnonymousEscrowOpenOffer { .. }
            | ir::Instr::AnonymousEscrowAccept { .. }
            | ir::Instr::AnonymousEscrowMarkPaymentSent { .. }
            | ir::Instr::AnonymousEscrowRelease { .. }
            | ir::Instr::AnonymousEscrowCancel { .. }
            | ir::Instr::AnonymousEscrowOpenDispute { .. }
            | ir::Instr::AnonymousEscrowResolveDispute { .. }
            | ir::Instr::TransferBatchBegin
            | ir::Instr::TransferBatchEnd
            | ir::Instr::TransferBatchApply { .. }
            | ir::Instr::MintAsset { .. }
            | ir::Instr::BurnAsset { .. }
            | ir::Instr::SetAccountDetail { .. }
            | ir::Instr::CreateNft { .. }
            | ir::Instr::SetNftData { .. }
            | ir::Instr::BurnNft { .. }
            | ir::Instr::TransferNft { .. }
            | ir::Instr::RegisterDomain { .. }
            | ir::Instr::RegisterAccount { .. }
            | ir::Instr::AddSignatory { .. }
            | ir::Instr::RemoveSignatory { .. }
            | ir::Instr::SetAccountQuorum { .. }
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
            | ir::Instr::QueryExecuteNorito { .. }
            | ir::Instr::QueryGet { .. }
            | ir::Instr::GetAccountBalance { .. }
            | ir::Instr::UseNullifier { .. }
            | ir::Instr::SmartContractLifecycle { .. }
            | ir::Instr::ZkRootsGet { .. }
            | ir::Instr::ZkVoteGetTally { .. }
            | ir::Instr::VrfEpochSeed { .. }
            | ir::Instr::CallContract { .. }
            | ir::Instr::InvokeEntrypointAs { .. }
            | ir::Instr::InvokeEntrypointAsMulti { .. }
            | ir::Instr::ExpectRejectAs { .. }
            | ir::Instr::ActorAccount { .. }
            | ir::Instr::ActorPublicKey { .. }
            | ir::Instr::ActorSign { .. }
            | ir::Instr::SubscriptionBill
            | ir::Instr::SubscriptionRecordUsage
            | ir::Instr::BuildSubmitBallotInline { .. }
            | ir::Instr::BuildUnshieldInline { .. }
            | ir::Instr::AxtBegin { .. }
            | ir::Instr::AxtTouch { .. }
            | ir::Instr::VerifyDsProof { .. }
            | ir::Instr::UseAssetHandle { .. }
            | ir::Instr::AxtCommit
            | ir::Instr::SoracloudHostCall { .. }
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

    let mut trigger_anchor_names = typed
        .items
        .iter()
        .filter_map(|item| match item {
            TypedItem::Function(func)
                if entrypoint_kind_from_modifiers(&func.modifiers).is_some() =>
            {
                Some(func.name.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if trigger_anchor_names.is_empty()
        && let Some(func) = typed.items.iter().find_map(|item| match item {
            TypedItem::Function(func)
                if func.modifiers.kind == FunctionKind::Free && func.name == "main" =>
            {
                Some(func)
            }
            _ => None,
        })
    {
        trigger_anchor_names.push(func.name.clone());
    }
    let namespaced_trigger_anchor = trigger_anchor_names.first().cloned();

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
        let trigger_anchor = if trigger.call.namespace.is_some() {
            namespaced_trigger_anchor.clone().ok_or_else(|| {
                format!(
                    "trigger `{}` has a namespaced callback but the contract has no entrypoint descriptor to carry it",
                    trigger.id
                )
            })?
        } else {
            trigger.call.entrypoint.clone()
        };
        triggers_by_name
            .entry(trigger_anchor)
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
        reads.retain(|key| retain_taira_supported_access_key(key));
        writes.retain(|key| retain_taira_supported_access_key(key));
        if include_hints && (reads.is_empty() || writes.is_empty()) {
            let (fallback_reads, fallback_writes) = crate::semantic::function_state_accesses(func);
            if reads.is_empty() && !fallback_reads.is_empty() {
                reads = fallback_reads.iter().cloned().collect();
            }
            if writes.is_empty() && !fallback_writes.is_empty() {
                writes = fallback_writes.iter().cloned().collect();
            }
            reads.retain(|key| retain_taira_supported_access_key(key));
            writes.retain(|key| retain_taira_supported_access_key(key));
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
                .map(|param| EntrypointParamDescriptor {
                    name: param.name.clone(),
                    type_name: semantic::render_type_name(&param.ty),
                })
                .collect(),
            return_type: func.ret_ty.as_ref().map(semantic::render_type_name),
            permission: func.modifiers.permission.clone(),
            read_keys: reads,
            write_keys: writes,
            access_hints_complete: report.and_then(|r| r.emitted.then_some(r.complete)),
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
