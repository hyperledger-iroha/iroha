//! Static analysis helpers for compiled IVM programs.
//!
//! This module exposes lightweight inspection utilities that decode `.to`
//! artifacts, walk the canonical instruction stream, and derive register,
//! memory, and syscall usage summaries. The output feeds the AMX/Nexus
//! pipelines (see roadmap item NX-17) by providing a deterministic read/write
//! fingerprint that can be compared against the declared UAID manifests.
//!
//! Typical usage:
//! ```no_run
//! # use ivm::analysis;
//! # fn inspect(bytes: &[u8]) {
//! let report = analysis::analyze_program(bytes).expect("valid program");
//! println!("{} instructions", report.instruction_count);
//! println!("load64 count {}", report.memory.load64);
//! for syscall in &report.syscalls {
//!     println!(
//!         "syscall 0x{:02x} used {} times",
//!         syscall.number, syscall.count
//!     );
//! }
//! # }
//! ```

use core::convert::TryFrom as _;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
    num::NonZeroUsize,
};

use iroha_data_model::{name::Name, state_path::StatePath};

use crate::{
    VMError, encoding,
    instruction::wide,
    ivm_cache::{DecodedOp, IvmCache},
    metadata::ProgramMetadata,
    prepared::PreparedContract,
};

/// Bytecode-proven durable-state accesses for one deployable contract scope.
///
/// `complete` is true only when every reachable durable-state syscall receives
/// one canonical, authenticated `StatePath` payload on every control-flow
/// path. A caller must retain its conservative state wildcard when this flag
/// is false.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticStateAccessAnalysis {
    /// Canonical scheduler keys read by the selected entrypoint scope.
    pub read_keys: BTreeSet<String>,
    /// Canonical scheduler keys written by the selected entrypoint scope.
    pub write_keys: BTreeSet<String>,
    /// Whether the selected bytecode scope reaches a state-read syscall.
    pub has_state_reads: bool,
    /// Whether the selected bytecode scope reaches a state-write syscall.
    pub has_state_writes: bool,
    /// Whether every reachable state target was proven exact and direct.
    pub complete: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StaticStatePath {
    Literal(u16),
    FromName(u16),
    MapChild { base: u16, key: StaticNoritoKey },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StaticNoritoKey {
    PointerEnvelope(u16),
    LiteralPayload(u16),
}

#[derive(Clone, PartialEq, Eq)]
struct StaticStateFacts {
    names: [Option<u16>; 256],
    paths: [Option<StaticStatePath>; 256],
    pointer_literals: [Option<u16>; 256],
    norito_keys: [Option<StaticNoritoKey>; 256],
    content_may_be_mutable: [bool; 256],
    stack_offsets: [Option<i64>; 256],
    direct: bool,
}

impl StaticStateFacts {
    fn entrypoint() -> Self {
        Self {
            names: [None; 256],
            paths: [None; 256],
            pointer_literals: [None; 256],
            norito_keys: [None; 256],
            content_may_be_mutable: [false; 256],
            stack_offsets: std::array::from_fn(|register| (register == 31).then_some(0)),
            direct: true,
        }
    }

    fn clear(&mut self) {
        self.names = [None; 256];
        self.paths = [None; 256];
        self.pointer_literals = [None; 256];
        self.norito_keys = [None; 256];
        self.content_may_be_mutable = [false; 256];
        self.stack_offsets = [None; 256];
    }

    fn clear_content(&mut self, register: usize) {
        self.names[register] = None;
        self.paths[register] = None;
        self.pointer_literals[register] = None;
        self.norito_keys[register] = None;
        self.content_may_be_mutable[register] = false;
    }

    fn clear_register(&mut self, register: usize) {
        self.clear_content(register);
        self.stack_offsets[register] = None;
    }

    fn copy_register(&mut self, destination: usize, source: usize) {
        self.names[destination] = self.names[source];
        self.paths[destination] = self.paths[source];
        self.pointer_literals[destination] = self.pointer_literals[source];
        self.norito_keys[destination] = self.norito_keys[source];
        self.content_may_be_mutable[destination] = self.content_may_be_mutable[source];
        self.stack_offsets[destination] = self.stack_offsets[source];
    }

    fn mark_content_mutable(&mut self, register: usize) {
        if self.names[register].is_some()
            || self.paths[register].is_some()
            || self.pointer_literals[register].is_some()
            || self.norito_keys[register].is_some()
        {
            self.content_may_be_mutable[register] = true;
        }
    }

    fn invalidate_mutable_content(&mut self) {
        for register in 0..self.content_may_be_mutable.len() {
            if self.content_may_be_mutable[register] {
                self.clear_content(register);
            }
        }
    }

    fn store_is_within_minimum_stack(
        &self,
        base_register: usize,
        immediate: i64,
        byte_len: i64,
    ) -> bool {
        let Some(start) =
            self.stack_offsets[base_register].and_then(|offset| offset.checked_add(immediate))
        else {
            return false;
        };
        let Some(end) = start.checked_add(byte_len) else {
            return false;
        };
        start >= -(crate::memory::Memory::STACK_ALIGNMENT as i64) && end <= 0
    }

    fn merge_from(&mut self, incoming: &Self) -> bool {
        fn retain_common<T: Eq>(
            current_facts: &mut [Option<T>],
            incoming_facts: &[Option<T>],
        ) -> bool {
            let mut changed = false;
            for (current, next) in current_facts.iter_mut().zip(incoming_facts) {
                if current.as_ref() != next.as_ref() && current.take().is_some() {
                    changed = true;
                }
            }
            changed
        }
        let mut changed = retain_common(&mut self.names, &incoming.names);
        changed |= retain_common(&mut self.paths, &incoming.paths);
        changed |= retain_common(&mut self.pointer_literals, &incoming.pointer_literals);
        changed |= retain_common(&mut self.norito_keys, &incoming.norito_keys);
        changed |= retain_common(&mut self.stack_offsets, &incoming.stack_offsets);
        for register in 0..self.content_may_be_mutable.len() {
            let has_content = self.names[register].is_some()
                || self.paths[register].is_some()
                || self.pointer_literals[register].is_some()
                || self.norito_keys[register].is_some();
            let may_be_mutable = has_content
                && (self.content_may_be_mutable[register]
                    || incoming.content_may_be_mutable[register]);
            changed |= self.content_may_be_mutable[register] != may_be_mutable;
            self.content_may_be_mutable[register] = may_be_mutable;
        }
        let direct = self.direct && incoming.direct;
        changed |= self.direct != direct;
        self.direct = direct;
        changed
    }
}

/// Aggregate register usage counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUsage {
    /// Number of times each register was read.
    pub reads: [u64; 256],
    /// Number of times each register was written.
    pub writes: [u64; 256],
}

impl Default for RegisterUsage {
    fn default() -> Self {
        Self {
            reads: [0; 256],
            writes: [0; 256],
        }
    }
}

/// Memory instruction statistics captured during analysis.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MemoryAccesses {
    pub load64: u64,
    pub store64: u64,
    pub load128: u64,
    pub store128: u64,
}

/// Syscall usage summary sorted by syscall number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyscallUsage {
    pub number: u32,
    pub count: u64,
}

/// Result of analysing a compiled program.
#[derive(Debug, Clone)]
pub struct ProgramAnalysis {
    pub metadata: ProgramMetadata,
    pub instruction_count: usize,
    pub registers: RegisterUsage,
    pub memory: MemoryAccesses,
    pub syscalls: Vec<SyscallUsage>,
}

/// Errors emitted by [`analyze_program`].
#[derive(Debug)]
pub enum ProgramAnalysisError {
    Metadata(VMError),
    Decode(VMError),
}

impl fmt::Display for ProgramAnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramAnalysisError::Metadata(err) => {
                write!(f, "failed to parse program metadata: {err}")
            }
            ProgramAnalysisError::Decode(err) => {
                write!(f, "failed to decode instruction stream: {err}")
            }
        }
    }
}

impl Error for ProgramAnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProgramAnalysisError::Metadata(err) | ProgramAnalysisError::Decode(err) => Some(err),
        }
    }
}

/// Decode the program contained in `bytes` and return aggregate read/write and
/// syscall usage information.
pub fn analyze_program(bytes: &[u8]) -> Result<ProgramAnalysis, ProgramAnalysisError> {
    let parsed = ProgramMetadata::parse(bytes).map_err(ProgramAnalysisError::Metadata)?;
    let code = &bytes[parsed.code_offset..];
    let decoded = IvmCache::decode_stream(code).map_err(ProgramAnalysisError::Decode)?;
    Ok(analyze_decoded(parsed.metadata, decoded.as_ref()))
}

/// Return aggregate usage information from an already prepared contract.
///
/// This path reuses the validated metadata and decoded instruction stream, so
/// admission caches do not parse or predecode the artifact a second time.
#[must_use]
pub fn analyze_prepared(contract: &PreparedContract) -> ProgramAnalysis {
    analyze_decoded(contract.metadata().clone(), contract.decoded().as_ref())
}

/// Prove exact durable-state targets from a prepared contract's authenticated
/// literal table and reachable bytecode.
///
/// Passing an entrypoint restricts the proof to that public selector. Passing
/// `None` analyzes the union of every embedded entrypoint. The proof is
/// deliberately narrow: state targets hidden behind a call, computed at
/// runtime, or merged from ambiguous paths make the result incomplete. This
/// lets scheduler metadata improve precision without becoming a security
/// authority.
#[must_use]
pub fn analyze_prepared_static_state_accesses(
    contract: &PreparedContract,
    entrypoint: Option<&str>,
) -> Option<StaticStateAccessAnalysis> {
    let roots = match entrypoint {
        Some(name) => vec![contract.entrypoint_descriptor(name)?.entry_pc],
        None => contract
            .contract_interface()
            .entrypoints
            .iter()
            .map(|descriptor| descriptor.entry_pc)
            .collect::<Vec<_>>(),
    };
    if roots.is_empty() {
        return None;
    }

    let decoded = contract.decoded();
    let literal_names = authenticated_literal_names(contract);
    let literal_state_paths = authenticated_literal_state_paths(contract);
    let literal_pointer_envelopes = authenticated_literal_pointer_envelopes(contract);
    let literal_norito_payloads = authenticated_literal_norito_payloads(contract);
    let mut incoming = BTreeMap::<u64, StaticStateFacts>::new();
    let mut pending = VecDeque::new();
    for &root in &roots {
        if !contract.is_instruction_boundary(root) {
            return None;
        }
        match incoming.entry(root) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(StaticStateFacts::entrypoint());
                pending.push_back(root);
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
    }

    let mut result = StaticStateAccessAnalysis {
        complete: true,
        ..StaticStateAccessAnalysis::default()
    };
    while let Some(pc) = pending.pop_front() {
        let index = decoded.binary_search_by_key(&pc, |op| op.pc).ok()?;
        let op = decoded.get(index)?;
        let mut outgoing = incoming.get(&pc)?.clone();
        transfer_static_state_facts(
            op,
            &literal_names,
            &literal_state_paths,
            &literal_pointer_envelopes,
            &literal_norito_payloads,
            &mut outgoing,
            &mut result,
        );
        if contract.has_indirect_control_flow(pc) {
            // An indirect edge has no authenticated target in the prepared
            // control-flow graph. Treat the proof as incomplete even when the
            // visible instruction itself is not a durable-state syscall:
            // otherwise a JR/JALR target could hide an unaccounted state
            // access while the scheduler accepts an exact access set.
            result.complete = false;
        }

        let successors = contract.control_flow_successors(pc)?;
        let call_edges = direct_call_edges(op);
        // Production Kotodama entrypoints are authenticated two-instruction
        // thunks (`call body; halt`). Crossing that compiler-owned boundary is
        // still direct entrypoint code; calls made by the body remain
        // conservative helper edges.
        let entrypoint_wrapper_call = call_edges.is_some_and(|(_, return_pc)| {
            roots.contains(&pc)
                && decoded
                    .binary_search_by_key(&return_pc, |candidate| candidate.pc)
                    .ok()
                    .and_then(|index| decoded.get(index))
                    .is_some_and(|candidate| wide::opcode(candidate.inst) == wide::control::HALT)
        });
        for successor in successors.iter().copied() {
            let mut successor_facts = outgoing.clone();
            if let Some((call_target, return_pc)) = call_edges {
                if successor == call_target && !entrypoint_wrapper_call {
                    // Even when a helper's path is literal, keep helper-hidden
                    // state access conservative.
                    successor_facts.direct = false;
                }
                if successor == return_pc {
                    // A callee may overwrite any caller-visible register, so a
                    // literal loaded before the call is not proof of a later state
                    // target. The caller can recover exactness by loading a fresh
                    // authenticated literal after the call.
                    successor_facts.clear();
                }
            }
            match incoming.entry(successor) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(successor_facts);
                    pending.push_back(successor);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if entry.get_mut().merge_from(&successor_facts) {
                        pending.push_back(successor);
                    }
                }
            }
        }
    }
    Some(result)
}

fn direct_call_edges(op: &DecodedOp) -> Option<(u64, u64)> {
    let offset_words = match wide::opcode(op.inst) {
        wide::control::JAL if wide::rd(op.inst) != 0 => i64::from(wide::imm16(op.inst)),
        wide::control::JALS => i64::from(wide::imm24(op.inst)),
        _ => return None,
    };
    let byte_offset = offset_words.checked_mul(4)?;
    let call_target = i128::from(op.pc)
        .checked_add(i128::from(byte_offset))
        .and_then(|target| u64::try_from(target).ok())?;
    let return_pc = op.pc.checked_add(u64::from(op.len))?;
    Some((call_target, return_pc))
}

fn authenticated_literal_names(contract: &PreparedContract) -> Vec<Option<String>> {
    contract
        .literal_table()
        .entries()
        .iter()
        .map(|literal| match literal {
            crate::ivm::DecodedLiteral::Pointer(pointer) => {
                authenticated_literal_name(contract, *pointer)
            }
            crate::ivm::DecodedLiteral::I64(_) => None,
        })
        .collect()
}

fn authenticated_literal_state_paths(contract: &PreparedContract) -> Vec<Option<String>> {
    contract
        .literal_table()
        .entries()
        .iter()
        .map(|literal| match literal {
            crate::ivm::DecodedLiteral::Pointer(pointer) => {
                authenticated_literal_state_path(contract, *pointer)
            }
            crate::ivm::DecodedLiteral::I64(_) => None,
        })
        .collect()
}

fn authenticated_literal_pointer_envelopes(contract: &PreparedContract) -> Vec<Option<String>> {
    contract
        .literal_table()
        .entries()
        .iter()
        .map(|literal| match literal {
            crate::ivm::DecodedLiteral::Pointer(pointer) => {
                authenticated_literal_tlv_bytes(contract, *pointer).map(hex::encode)
            }
            crate::ivm::DecodedLiteral::I64(_) => None,
        })
        .collect()
}

fn authenticated_literal_norito_payloads(contract: &PreparedContract) -> Vec<Option<String>> {
    contract
        .literal_table()
        .entries()
        .iter()
        .map(|literal| match literal {
            crate::ivm::DecodedLiteral::Pointer(pointer) => {
                let bytes = authenticated_literal_tlv_bytes(contract, *pointer)?;
                let tlv = crate::pointer_abi::validate_tlv_bytes(bytes).ok()?;
                (tlv.type_id == crate::pointer_abi::PointerType::NoritoBytes)
                    .then(|| hex::encode(tlv.payload))
            }
            crate::ivm::DecodedLiteral::I64(_) => None,
        })
        .collect()
}

fn authenticated_literal_name(contract: &PreparedContract, pointer: u64) -> Option<String> {
    let bytes = authenticated_literal_tlv_bytes(contract, pointer)?;
    let tlv = crate::pointer_abi::validate_tlv_bytes(bytes).ok()?;
    if tlv.type_id != crate::pointer_abi::PointerType::Name {
        return None;
    }
    let name: Name = norito::decode_canonical(tlv.payload).ok()?;
    if crate::host::state_path_name_payload_len(&name).ok()?
        > crate::syscalls::STATE_MAP_MAX_BASE_FRAME_BYTES
    {
        return None;
    }
    Some(name.to_string())
}

fn authenticated_literal_state_path(contract: &PreparedContract, pointer: u64) -> Option<String> {
    let bytes = authenticated_literal_tlv_bytes(contract, pointer)?;
    let tlv = crate::pointer_abi::validate_tlv_bytes(bytes).ok()?;
    if tlv.type_id != crate::pointer_abi::PointerType::NoritoBytes {
        return None;
    }
    if tlv.payload.len() > crate::syscalls::STATE_MAX_PATH_FRAME_BYTES {
        return None;
    }
    let path: StatePath = norito::decode_canonical(tlv.payload).ok()?;
    crate::host::validate_state_path(&path).ok()?;
    Some(path.to_string())
}

fn authenticated_literal_tlv_bytes(contract: &PreparedContract, pointer: u64) -> Option<&[u8]> {
    let start = contract
        .header_len()
        .checked_add(usize::try_from(pointer).ok()?)?;
    let artifact = contract.artifact();
    let fixed_end = start.checked_add(7)?;
    let fixed = artifact.get(start..fixed_end)?;
    let payload_len =
        usize::try_from(u32::from_be_bytes(fixed.get(3..7)?.try_into().ok()?)).ok()?;
    let end = fixed_end
        .checked_add(payload_len)?
        .checked_add(iroha_crypto::Hash::LENGTH)?;
    let bytes = artifact.get(start..end)?;
    crate::pointer_abi::validate_tlv_bytes(bytes).ok()?;
    Some(bytes)
}

fn transfer_static_state_facts(
    op: &DecodedOp,
    literal_names: &[Option<String>],
    literal_state_paths: &[Option<String>],
    literal_pointer_envelopes: &[Option<String>],
    literal_norito_payloads: &[Option<String>],
    facts: &mut StaticStateFacts,
    result: &mut StaticStateAccessAnalysis,
) {
    let opcode = wide::opcode(op.inst);
    match opcode {
        wide::memory::LDLIT => {
            let destination = wide::rd(op.inst);
            let index = wide::literal_index(op.inst);
            facts.clear_register(destination);
            facts.names[destination] = literal_names
                .get(index)
                .and_then(Option::as_ref)
                .and_then(|_| u16::try_from(index).ok());
            facts.paths[destination] = literal_state_paths
                .get(index)
                .and_then(Option::as_ref)
                .and_then(|_| u16::try_from(index).ok())
                .map(StaticStatePath::Literal);
            facts.pointer_literals[destination] = literal_pointer_envelopes
                .get(index)
                .and_then(Option::as_ref)
                .and_then(|_| u16::try_from(index).ok());
            facts.norito_keys[destination] = literal_norito_payloads
                .get(index)
                .and_then(Option::as_ref)
                .and_then(|_| u16::try_from(index).ok())
                .map(StaticNoritoKey::LiteralPayload);
        }
        wide::memory::LDI64 => {
            facts.clear_register(wide::rd(op.inst));
        }
        wide::memory::LOAD64 => {
            let (_, destination, _, _) = encoding::wide::decode_mem(op.inst);
            facts.clear_register(usize::from(destination));
        }
        wide::memory::STORE64 => {
            let (_, base, _, immediate) = encoding::wide::decode_mem(op.inst);
            if !facts.store_is_within_minimum_stack(usize::from(base), i64::from(immediate), 8) {
                facts.invalidate_mutable_content();
            }
        }
        wide::memory::STORE128 => {
            let (_, base, _, _) = encoding::wide::decode_store128(op.inst);
            if !facts.store_is_within_minimum_stack(usize::from(base), 0, 16) {
                facts.invalidate_mutable_content();
            }
        }
        wide::memory::LOAD128 => {
            let (_, destination_low, _, destination_high) = encoding::wide::decode_load128(op.inst);
            facts.clear_register(usize::from(destination_low));
            facts.clear_register(usize::from(destination_high));
        }
        wide::arithmetic::ADDI => {
            let (_, destination, source, immediate) = encoding::wide::decode_ri(op.inst);
            let stack_offset = facts.stack_offsets[usize::from(source)]
                .and_then(|offset| offset.checked_add(i64::from(immediate)));
            if immediate == 0 {
                facts.copy_register(usize::from(destination), usize::from(source));
            } else {
                facts.clear_register(usize::from(destination));
            }
            facts.stack_offsets[usize::from(destination)] = stack_offset;
        }
        wide::control::BEQ
        | wide::control::BNE
        | wide::control::BLT
        | wide::control::BGE
        | wide::control::BLTU
        | wide::control::BGEU
        | wide::control::JMP
        | wide::control::JR
        | wide::control::HALT => {}
        wide::control::JAL => {
            facts.clear_register(wide::rd(op.inst));
        }
        wide::control::JALS => {
            facts.clear_register(1);
        }
        wide::control::JALR => {
            facts.clear_register(wide::rd(op.inst));
        }
        wide::system::SCALL => {
            let number = u32::from(wide::imm8(op.inst) as u8);
            transfer_static_state_syscall(
                number,
                literal_names,
                literal_state_paths,
                literal_pointer_envelopes,
                literal_norito_payloads,
                facts,
                result,
            );
        }
        wide::system::SYSTEM => {
            let number = encoding::wide::decode_syscallx(op.inst);
            transfer_static_state_syscall(
                number,
                literal_names,
                literal_state_paths,
                literal_pointer_envelopes,
                literal_norito_payloads,
                facts,
                result,
            );
        }
        _ => {
            // A narrow whitelist is intentional. Any unmodelled register or
            // memory transformation invalidates literal provenance.
            facts.clear();
        }
    }
}

fn transfer_static_state_syscall(
    number: u32,
    literal_names: &[Option<String>],
    literal_state_paths: &[Option<String>],
    literal_pointer_envelopes: &[Option<String>],
    literal_norito_payloads: &[Option<String>],
    facts: &mut StaticStateFacts,
    result: &mut StaticStateAccessAnalysis,
) {
    if number == crate::syscalls::SYSCALL_INPUT_PUBLISH_TLV {
        facts.mark_content_mutable(10);
        return;
    }
    if number == crate::syscalls::SYSCALL_POINTER_TO_NORITO {
        let pointer_literal = facts.pointer_literals[10];
        facts.clear_register(10);
        facts.norito_keys[10] = pointer_literal.map(StaticNoritoKey::PointerEnvelope);
        facts.mark_content_mutable(10);
        return;
    }
    if matches!(
        number,
        crate::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
            | crate::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT
    ) {
        let path = match (facts.names[10], facts.norito_keys[11]) {
            (Some(base), Some(key)) => Some(StaticStatePath::MapChild { base, key }),
            _ => None,
        };
        facts.clear_register(10);
        facts.paths[10] = path;
        facts.mark_content_mutable(10);
        return;
    }
    if number == crate::syscalls::SYSCALL_STATE_PATH_FROM_NAME {
        let path = facts.names[10].map(StaticStatePath::FromName);
        facts.clear_register(10);
        facts.paths[10] = path;
        facts.mark_content_mutable(10);
        return;
    }
    if matches!(
        number,
        crate::syscalls::SYSCALL_STATE_VALUE_ENCODE
            | crate::syscalls::SYSCALL_STATE_VALUE_DECODE
            | crate::syscalls::SYSCALL_STATE_MAP_KEY_AT
    ) {
        facts.clear_register(10);
        return;
    }
    let access = crate::syscalls::syscall_access(number);
    if matches!(
        access,
        crate::syscalls::SyscallAccess::StateRead | crate::syscalls::SyscallAccess::StateWrite
    ) {
        match access {
            crate::syscalls::SyscallAccess::StateRead => result.has_state_reads = true,
            crate::syscalls::SyscallAccess::StateWrite => result.has_state_writes = true,
            _ => unreachable!("state access was matched above"),
        }
        let key = facts.paths[10]
            .and_then(|path| match path {
                StaticStatePath::Literal(index) => literal_state_paths
                    .get(usize::from(index))
                    .and_then(Option::as_deref)
                    .map(ToOwned::to_owned),
                StaticStatePath::FromName(index) => literal_names
                    .get(usize::from(index))
                    .and_then(Option::as_deref)
                    .map(ToOwned::to_owned),
                StaticStatePath::MapChild { base, key } => {
                    let base = literal_names
                        .get(usize::from(base))
                        .and_then(Option::as_deref)?;
                    let key = match key {
                        StaticNoritoKey::PointerEnvelope(index) => literal_pointer_envelopes
                            .get(usize::from(index))
                            .and_then(Option::as_deref)?,
                        StaticNoritoKey::LiteralPayload(index) => literal_norito_payloads
                            .get(usize::from(index))
                            .and_then(Option::as_deref)?,
                    };
                    Some(format!("{base}/{key}"))
                }
            })
            .and_then(|name| {
                if !matches!(
                    number,
                    crate::syscalls::SYSCALL_STATE_KEYS | crate::syscalls::SYSCALL_STATE_COUNT
                ) {
                    return Some(format!("state:{name}"));
                }
                // Scheduler wildcard interning is keyed by the map's first
                // path segment. A scan rooted at a concrete map entry (or any
                // deeper path) cannot be represented exactly as
                // `state:{name}[*]`: that nested wildcard would not conflict
                // with the concrete entry key. Keep only a declared-style
                // bare map base exact and fail closed for nested scan roots.
                (!name.contains('/')).then(|| format!("state:{name}[*]"))
            });
        if facts.direct {
            if let Some(key) = key {
                match access {
                    crate::syscalls::SyscallAccess::StateRead => {
                        result.read_keys.insert(key);
                    }
                    crate::syscalls::SyscallAccess::StateWrite => {
                        result.write_keys.insert(key);
                    }
                    _ => unreachable!("state access was matched above"),
                }
            } else {
                result.complete = false;
            }
        } else {
            result.complete = false;
        }
    }
    // Host calls may publish output pointers or otherwise change the calling
    // convention. A later exact target must establish fresh literal evidence.
    facts.clear();
}

fn analyze_decoded(metadata: ProgramMetadata, decoded: &[DecodedOp]) -> ProgramAnalysis {
    let mut builder = ProgramAnalysisBuilder::new(metadata);
    for op in decoded {
        builder.visit(op);
    }
    builder.finish()
}

/// Default execution budgets for atomic multi-dataspace execution (NX-17).
#[derive(Debug, Clone)]
pub struct AmxLimits {
    /// Per-dataspace budget in milliseconds (defaults to 30 ms).
    pub per_dataspace_budget_ms: u64,
    /// Group budget across all dataspaces in milliseconds (defaults to 140 ms).
    pub group_budget_ms: u64,
    /// Per-instruction cost in nanoseconds used for coarse estimation.
    pub per_instruction_ns: u64,
    /// Cost in nanoseconds for each memory access.
    pub per_memory_access_ns: u64,
    /// Cost in nanoseconds per syscall invocation.
    pub per_syscall_ns: u64,
}

impl Default for AmxLimits {
    fn default() -> Self {
        Self {
            per_dataspace_budget_ms: 30,
            group_budget_ms: 140,
            per_instruction_ns: 50,
            per_memory_access_ns: 80,
            per_syscall_ns: 120,
        }
    }
}

/// Estimated cost summary for AMX budgeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmxCost {
    /// Estimated per-dataspace execution time (nanoseconds).
    pub estimated_ns_per_dataspace: u64,
    /// Estimated group execution time across all dataspaces (nanoseconds).
    pub estimated_group_ns: u64,
}

/// Budget violation raised when AMX estimates exceed configured limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmxBudgetError {
    /// The per-dataspace budget was exceeded.
    PerDataspaceBudgetExceeded {
        /// Estimated cost in nanoseconds.
        estimated_ns: u64,
        /// Allowed budget in nanoseconds.
        limit_ns: u64,
    },
    /// The group budget across dataspaces was exceeded.
    GroupBudgetExceeded {
        /// Estimated group cost in nanoseconds.
        estimated_ns: u64,
        /// Allowed group budget in nanoseconds.
        limit_ns: u64,
    },
}

/// Estimate the AMX execution cost and enforce per-dataspace/group budgets.
pub fn enforce_amx_budget(
    analysis: &ProgramAnalysis,
    dataspace_count: NonZeroUsize,
    limits: &AmxLimits,
) -> Result<AmxCost, AmxBudgetError> {
    let per_ds_limit = limits.per_dataspace_budget_ms.saturating_mul(1_000_000);
    let group_limit = limits.group_budget_ms.saturating_mul(1_000_000);

    let memory_accesses = analysis.memory.load64
        + analysis.memory.store64
        + analysis.memory.load128
        + analysis.memory.store128;
    let syscall_calls: u64 = analysis.syscalls.iter().map(|entry| entry.count).sum();

    let estimated_ns = analysis
        .instruction_count
        .saturating_mul(limits.per_instruction_ns as usize) as u64
        + memory_accesses.saturating_mul(limits.per_memory_access_ns)
        + syscall_calls.saturating_mul(limits.per_syscall_ns);

    if estimated_ns > per_ds_limit {
        return Err(AmxBudgetError::PerDataspaceBudgetExceeded {
            estimated_ns,
            limit_ns: per_ds_limit,
        });
    }

    let group_estimated_ns = estimated_ns.saturating_mul(dataspace_count.get() as u64);
    if group_estimated_ns > group_limit {
        return Err(AmxBudgetError::GroupBudgetExceeded {
            estimated_ns: group_estimated_ns,
            limit_ns: group_limit,
        });
    }

    Ok(AmxCost {
        estimated_ns_per_dataspace: estimated_ns,
        estimated_group_ns: group_estimated_ns,
    })
}

struct ProgramAnalysisBuilder {
    metadata: ProgramMetadata,
    registers: RegisterUsage,
    memory: MemoryAccesses,
    instruction_count: usize,
    syscall_table: BTreeMap<u32, u64>,
}

impl ProgramAnalysisBuilder {
    fn new(metadata: ProgramMetadata) -> Self {
        Self {
            metadata,
            registers: RegisterUsage::default(),
            memory: MemoryAccesses::default(),
            instruction_count: 0,
            syscall_table: BTreeMap::new(),
        }
    }

    fn finish(self) -> ProgramAnalysis {
        let syscalls = self
            .syscall_table
            .into_iter()
            .map(|(number, count)| SyscallUsage { number, count })
            .collect();
        ProgramAnalysis {
            metadata: self.metadata,
            instruction_count: self.instruction_count,
            registers: self.registers,
            memory: self.memory,
            syscalls,
        }
    }

    fn visit(&mut self, op: &DecodedOp) {
        self.instruction_count += 1;
        let opcode = wide::opcode(op.inst);
        match opcode {
            // ALU operations with two explicit sources.
            wide::arithmetic::ADD
            | wide::arithmetic::SUB
            | wide::arithmetic::AND
            | wide::arithmetic::OR
            | wide::arithmetic::XOR
            | wide::arithmetic::SLL
            | wide::arithmetic::SRL
            | wide::arithmetic::SRA
            | wide::arithmetic::SLT
            | wide::arithmetic::SLTU
            | wide::arithmetic::CMOV
            | wide::arithmetic::SEQ
            | wide::arithmetic::SNE
            | wide::arithmetic::MUL
            | wide::arithmetic::MULH
            | wide::arithmetic::MULHU
            | wide::arithmetic::MULHSU
            | wide::arithmetic::DIV
            | wide::arithmetic::DIVU
            | wide::arithmetic::REM
            | wide::arithmetic::REMU
            | wide::arithmetic::ROTL
            | wide::arithmetic::ROTR
            | wide::arithmetic::MIN
            | wide::arithmetic::MAX
            | wide::arithmetic::DIV_CEIL
            | wide::arithmetic::GCD
            | wide::arithmetic::MEAN => {
                self.two_src_one_dst(op.inst);
            }
            // Unary ALU operations.
            wide::arithmetic::NOT
            | wide::arithmetic::NEG
            | wide::arithmetic::POPCNT
            | wide::arithmetic::CLZ
            | wide::arithmetic::CTZ
            | wide::arithmetic::ABS
            | wide::arithmetic::ISQRT => {
                self.one_src_one_dst(op.inst);
            }
            // Immediate ALU operations.
            wide::arithmetic::ADDI
            | wide::arithmetic::ANDI
            | wide::arithmetic::ORI
            | wide::arithmetic::XORI
            | wide::arithmetic::CMOVI
            | wide::arithmetic::ROTL_IMM
            | wide::arithmetic::ROTR_IMM => {
                self.one_src_one_dst(op.inst);
            }
            // Memory access instructions.
            wide::memory::LOAD64 => {
                self.memory.load64 += 1;
                let (_, dest, base, _) = encoding::wide::decode_mem(op.inst);
                self.write(dest);
                self.read(base);
            }
            wide::memory::STORE64 => {
                self.memory.store64 += 1;
                let (_, base, value, _) = encoding::wide::decode_mem(op.inst);
                self.read(base);
                self.read(value);
            }
            wide::memory::LOAD128 => {
                self.memory.load128 += 1;
                let (_, rd_lo, base, rd_hi) = encoding::wide::decode_load128(op.inst);
                self.write(rd_lo);
                self.write(rd_hi);
                self.read(base);
            }
            wide::memory::STORE128 => {
                self.memory.store128 += 1;
                let (_, base, rs_lo, rs_hi) = encoding::wide::decode_store128(op.inst);
                self.read(base);
                self.read(rs_lo);
                self.read(rs_hi);
            }
            wide::memory::LDLIT | wide::memory::LDI64 => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
            }
            // Control flow.
            wide::control::BEQ
            | wide::control::BNE
            | wide::control::BLT
            | wide::control::BGE
            | wide::control::BLTU
            | wide::control::BGEU => {
                let rs1 = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                let rs2 = u8::try_from(wide::rs1(op.inst)).expect("register index fits in u8");
                self.read(rs1);
                self.read(rs2);
            }
            wide::control::JR => {
                let rs = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.read(rs);
            }
            wide::control::JALR => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                let rs = u8::try_from(wide::rs1(op.inst)).expect("register index fits in u8");
                self.write(rd);
                self.read(rs);
            }
            wide::control::JAL => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
            }
            wide::control::JALS => self.write(1u8),
            wide::control::JMP | wide::control::HALT => {}
            // System helpers.
            wide::system::GETGAS => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
            }
            wide::system::SCALL => {
                let number = u32::from(wide::imm8(op.inst).to_ne_bytes()[0]);
                *self.syscall_table.entry(number).or_default() += 1;
            }
            wide::system::SYSTEM => {
                let number = crate::encoding::wide::decode_syscallx(op.inst);
                *self.syscall_table.entry(number).or_default() += 1;
            }
            // Vector configuration.
            wide::crypto::SETVL => {
                // SETVL carries its lane count in the rs2/immediate field; it
                // does not consume a vector or scalar register operand.
            }
            wide::crypto::PARBEGIN | wide::crypto::PAREND => {}
            wide::crypto::POSEIDON2 => self.two_src_one_dst(op.inst),
            wide::crypto::POSEIDON6 => {
                let rd = Self::reg(wide::rd(op.inst));
                self.write(rd);
                if let Some((_, rs_base)) = crate::encoding::wide::decode_poseidon6(op.inst) {
                    for offset in 0..wide::crypto::POSEIDON6_INPUTS {
                        self.read(usize::from(rs_base) + offset);
                    }
                }
            }
            // All remaining opcodes (crypto, ISO20022, ZK, vector ALU, etc.)
            // follow the canonical rd/rs1/rs2 layout.
            _ => {
                self.two_src_one_dst(op.inst);
            }
        }
    }

    fn two_src_one_dst(&mut self, inst: u32) {
        let rd = Self::reg(wide::rd(inst));
        let rs1 = Self::reg(wide::rs1(inst));
        let rs2 = Self::reg(wide::rs2(inst));
        self.write(rd);
        self.read(rs1);
        self.read(rs2);
    }

    fn one_src_one_dst(&mut self, inst: u32) {
        let rd = Self::reg(wide::rd(inst));
        let rs = Self::reg(wide::rs1(inst));
        self.write(rd);
        self.read(rs);
    }

    fn read<R>(&mut self, reg: R)
    where
        R: Into<usize>,
    {
        let idx = reg.into();
        debug_assert!(idx < self.registers.reads.len());
        self.registers.reads[idx] = self.registers.reads[idx].saturating_add(1);
    }

    fn write<R>(&mut self, reg: R)
    where
        R: Into<usize>,
    {
        let idx = reg.into();
        debug_assert!(idx < self.registers.writes.len());
        self.registers.writes[idx] = self.registers.writes[idx].saturating_add(1);
    }

    fn reg(index: usize) -> u8 {
        u8::try_from(index).expect("register index fits in u8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encoding::wide as wide_enc, instruction::wide};

    fn base_analysis(instruction_count: usize) -> ProgramAnalysis {
        ProgramAnalysis {
            metadata: ProgramMetadata::default(),
            instruction_count,
            registers: RegisterUsage::default(),
            memory: MemoryAccesses::default(),
            syscalls: Vec::new(),
        }
    }

    fn build_program(words: &[u32]) -> Vec<u8> {
        let mut bytes = ProgramMetadata::default_for(1, 0, 1).encode();
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn static_state_analysis_proves_compiler_literal_map_child() {
        let source = r#"
seiyaku StaticMapAnalysis {
  state StateMap<int, int> Counters;
  kotoage fn write_one() authorize("CanWrite") { Counters[1] = 10; }
}
"#;
        let (program, manifest) = crate::KotodamaCompiler::new()
            .compile_source_with_manifest(source)
            .expect("compile literal StateMap access");
        let prepared = crate::prepare_contract(std::sync::Arc::<[u8]>::from(program))
            .expect("prepare literal StateMap access");
        let analysis = analyze_prepared_static_state_accesses(&prepared, Some("write_one"))
            .expect("analyze literal StateMap access");
        let descriptor = manifest
            .entrypoints
            .as_deref()
            .and_then(|entrypoints| {
                entrypoints
                    .iter()
                    .find(|entrypoint| entrypoint.name == "write_one")
            })
            .expect("compiled entrypoint descriptor");

        assert!(analysis.complete);
        assert!(analysis.has_state_writes);
        assert_eq!(
            analysis.write_keys,
            descriptor.write_keys.iter().cloned().collect()
        );
    }

    #[test]
    fn static_state_analysis_keeps_user_helper_access_incomplete() {
        let source = r#"
seiyaku HelperMapAnalysis {
  state StateMap<int, int> Counters;
  fn hidden_write() { Counters[1] = 10; }
  kotoage fn helper_write() authorize("CanWrite") { hidden_write(); }
}
"#;
        let program = crate::KotodamaCompiler::new()
            .compile_source(source)
            .expect("compile helper-hidden StateMap access");
        let prepared = crate::prepare_contract(std::sync::Arc::<[u8]>::from(program))
            .expect("prepare helper-hidden StateMap access");
        let analysis = analyze_prepared_static_state_accesses(&prepared, Some("helper_write"))
            .expect("analyze helper-hidden StateMap access");

        assert!(analysis.has_state_writes);
        assert!(!analysis.complete);
        assert!(analysis.write_keys.is_empty());
    }

    #[test]
    fn static_state_analysis_marks_reachable_indirect_edge_incomplete() {
        let source = r#"
seiyaku IndirectStateAnalysis {
  kotoage fn run() authorize("CanRun") {}
}
"#;
        let program = crate::KotodamaCompiler::new()
            .compile_source(source)
            .expect("compile direct control-flow fixture");
        let prepared = crate::prepare_contract(std::sync::Arc::<[u8]>::from(program))
            .expect("prepare direct control-flow fixture");
        let entry_pc = prepared
            .entrypoint_descriptor("run")
            .expect("run entrypoint")
            .entry_pc;
        let mut decoded = prepared.decoded().as_ref().to_vec();
        let entry = decoded
            .iter_mut()
            .find(|op| op.pc == entry_pc)
            .expect("entrypoint instruction");
        entry.inst = wide_enc::encode_rr(wide::control::JALR, 0, 2, 0);
        let control_flow = crate::prepared::PreparedControlFlow::from_decoded(&decoded)
            .expect("build adversarial indirect control flow");
        let adversarial =
            crate::prepared::PreparedContract::from_parts(crate::prepared::PreparedContractParts {
                artifact: prepared.shared_artifact(),
                metadata: prepared.metadata().clone(),
                manifest: prepared.manifest().clone(),
                header_len: prepared.header_len(),
                code_offset: prepared.code_offset(),
                code_hash: prepared.code_hash(),
                contract_interface: prepared.shared_contract_interface(),
                literal_table: prepared.literal_table().clone(),
                decoded: std::sync::Arc::from(decoded),
                prepared_program: prepared.prepared_program().clone(),
                control_flow,
            })
            .expect("construct analyzer-only adversarial contract");

        let analysis = analyze_prepared_static_state_accesses(&adversarial, Some("run"))
            .expect("analyze indirect control flow");
        assert!(!analysis.complete);
        assert!(!analysis.has_state_reads);
        assert!(!analysis.has_state_writes);
    }

    #[test]
    fn static_state_analysis_invalidates_host_paths_across_unbounded_stores() {
        let literal_names = vec![Some("Counters".to_owned())];
        let literal_state_paths = vec![None];
        let literal_pointer_envelopes = vec![None];
        let literal_norito_payloads = vec![Some("00".to_owned())];
        let stores = [
            wide_enc::encode_store(wide::memory::STORE64, 2, 3, 0),
            wide_enc::encode_store128(wide::memory::STORE128, 2, 3, 4),
        ];

        for store in stores {
            let mut facts = StaticStateFacts::entrypoint();
            facts.names[10] = Some(0);
            facts.norito_keys[11] = Some(StaticNoritoKey::LiteralPayload(0));
            let mut analysis = StaticStateAccessAnalysis {
                complete: true,
                ..StaticStateAccessAnalysis::default()
            };
            transfer_static_state_syscall(
                crate::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO,
                &literal_names,
                &literal_state_paths,
                &literal_pointer_envelopes,
                &literal_norito_payloads,
                &mut facts,
                &mut analysis,
            );
            assert!(facts.content_may_be_mutable[10]);

            transfer_static_state_facts(
                &DecodedOp {
                    pc: 0,
                    inst: store,
                    len: 4,
                },
                &literal_names,
                &literal_state_paths,
                &literal_pointer_envelopes,
                &literal_norito_payloads,
                &mut facts,
                &mut analysis,
            );
            transfer_static_state_syscall(
                crate::syscalls::SYSCALL_STATE_SET,
                &literal_names,
                &literal_state_paths,
                &literal_pointer_envelopes,
                &literal_norito_payloads,
                &mut facts,
                &mut analysis,
            );

            assert!(analysis.has_state_writes);
            assert!(!analysis.complete);
            assert!(analysis.write_keys.is_empty());
        }
    }

    #[test]
    fn static_state_analysis_rejects_legacy_name_carrier() {
        let literal_names = vec![Some("legacy".to_owned())];
        let literal_state_paths = vec![None];
        let literal_pointer_envelopes = vec![None];
        let literal_norito_payloads = vec![None];
        let mut facts = StaticStateFacts::entrypoint();
        facts.names[10] = Some(0);
        let mut analysis = StaticStateAccessAnalysis {
            complete: true,
            ..StaticStateAccessAnalysis::default()
        };

        transfer_static_state_syscall(
            crate::syscalls::SYSCALL_STATE_SET,
            &literal_names,
            &literal_state_paths,
            &literal_pointer_envelopes,
            &literal_norito_payloads,
            &mut facts,
            &mut analysis,
        );

        assert!(analysis.has_state_writes);
        assert!(!analysis.complete);
        assert!(analysis.write_keys.is_empty());
    }

    #[test]
    fn static_state_analysis_rejects_nested_scan_wildcard_claim() {
        let literal_names = vec![None];
        let literal_state_paths = vec![Some("Counters/00".to_owned())];
        let literal_pointer_envelopes = vec![None];
        let literal_norito_payloads = vec![None];

        for syscall in [
            crate::syscalls::SYSCALL_STATE_KEYS,
            crate::syscalls::SYSCALL_STATE_COUNT,
        ] {
            let mut facts = StaticStateFacts::entrypoint();
            facts.paths[10] = Some(StaticStatePath::Literal(0));
            let mut analysis = StaticStateAccessAnalysis {
                complete: true,
                ..StaticStateAccessAnalysis::default()
            };

            transfer_static_state_syscall(
                syscall,
                &literal_names,
                &literal_state_paths,
                &literal_pointer_envelopes,
                &literal_norito_payloads,
                &mut facts,
                &mut analysis,
            );

            assert!(analysis.has_state_reads);
            assert!(!analysis.complete);
            assert!(analysis.read_keys.is_empty());
        }
    }

    #[test]
    fn analysis_reports_registers_memory_and_syscalls() {
        let words = [
            wide_enc::encode_ri(wide::arithmetic::ADDI, 1, 0, 5),
            wide_enc::encode_load(wide::memory::LOAD64, 2, 1, 0),
            wide_enc::encode_store(wide::memory::STORE64, 3, 2, 8),
            wide_enc::encode_branch(wide::control::BEQ, 2, 1, 1),
            wide_enc::encode_sys(wide::system::SCALL, 0x22),
            wide_enc::encode_halt(),
        ];
        let program = build_program(&words);
        let report = analyze_program(&program).expect("analysis succeeds");
        assert_eq!(report.instruction_count, words.len());
        assert_eq!(report.memory.load64, 1);
        assert_eq!(report.memory.store64, 1);
        assert_eq!(report.syscalls.len(), 1);
        assert_eq!(
            report.syscalls[0],
            SyscallUsage {
                number: 0x22,
                count: 1
            }
        );
        assert_eq!(report.registers.reads[0], 1);
        assert!(report.registers.writes[1] >= 1);
        assert!(report.registers.reads[1] >= 2);
        assert!(report.registers.reads[2] >= 1);
    }

    #[test]
    fn analysis_reports_extended_syscall_numbers() {
        let syscall = crate::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS;
        let words = [wide_enc::encode_syscallx(syscall), wide_enc::encode_halt()];
        let program = build_program(&words);
        let report = analyze_program(&program).expect("analysis succeeds");
        assert_eq!(
            report.syscalls,
            vec![SyscallUsage {
                number: syscall,
                count: 1
            }]
        );
    }

    #[test]
    fn analysis_rejects_truncated_program() {
        let bytes = vec![0u8; 4];
        let err = analyze_program(&bytes).expect_err("metadata parse should fail");
        assert!(matches!(err, ProgramAnalysisError::Metadata(_)));
    }

    #[test]
    fn analysis_treats_setvl_operand_as_immediate() {
        let words = [
            wide_enc::encode_rr(wide::crypto::SETVL, 0, 0, 8),
            wide_enc::encode_halt(),
        ];
        let program = build_program(&words);
        let report = analyze_program(&program).expect("analysis succeeds");
        assert_eq!(report.instruction_count, words.len());
        assert_eq!(
            report.registers.reads[8], 0,
            "SETVL immediate must not be reported as a register read"
        );
    }

    #[test]
    fn analysis_tracks_poseidon_register_inputs() {
        let words = [
            wide_enc::encode_poseidon2(9, 10, 11),
            wide_enc::encode_poseidon6(8, 20),
            wide_enc::encode_halt(),
        ];
        let report = analyze_program(&build_program(&words)).expect("analysis succeeds");

        assert_eq!(report.registers.writes[9], 1);
        assert_eq!(report.registers.writes[8], 1);
        assert_eq!(report.registers.reads[10], 1);
        assert_eq!(report.registers.reads[11], 1);
        for register in 20..26 {
            assert_eq!(report.registers.reads[register], 1);
        }
        assert_eq!(report.memory, MemoryAccesses::default());
    }

    #[test]
    fn analysis_tracks_indexed_literal_and_implicit_far_link() {
        let words = [
            wide_enc::encode_literal(wide::memory::LDLIT, 42, 0x1234),
            wide_enc::encode_literal(wide::memory::LDI64, 43, 0x5678),
            wide_enc::encode_offset24(wide::control::JALS, 1),
            wide_enc::encode_halt(),
        ];
        let program = build_program(&words);
        let report = analyze_program(&program).expect("analysis succeeds");

        assert_eq!(report.registers.writes[42], 1);
        assert_eq!(report.registers.writes[43], 1);
        assert_eq!(report.registers.writes[1], 1);
        assert_eq!(report.registers.reads.iter().sum::<u64>(), 0);
    }

    #[test]
    fn amx_budget_accepts_small_program() {
        let analysis = base_analysis(10_000);
        let limits = AmxLimits::default();
        let result = enforce_amx_budget(&analysis, NonZeroUsize::new(1).unwrap(), &limits).unwrap();
        assert!(result.estimated_ns_per_dataspace < limits.per_dataspace_budget_ms * 1_000_000);
    }

    #[test]
    fn amx_budget_rejects_large_program() {
        // 700k instructions * 50 ns = 35 ms > 30 ms per-dataspace budget.
        let analysis = base_analysis(700_000);
        let limits = AmxLimits::default();
        let err =
            enforce_amx_budget(&analysis, NonZeroUsize::new(1).unwrap(), &limits).unwrap_err();
        assert!(matches!(
            err,
            AmxBudgetError::PerDataspaceBudgetExceeded { .. }
        ));
    }

    #[test]
    fn amx_budget_rejects_group_overflow() {
        // Fits per-dataspace budget (~27.5 ms) but violates group budget when scaled to 6 lanes.
        let analysis = base_analysis(550_000);
        let limits = AmxLimits::default();
        let err =
            enforce_amx_budget(&analysis, NonZeroUsize::new(6).unwrap(), &limits).unwrap_err();
        assert!(matches!(err, AmxBudgetError::GroupBudgetExceeded { .. }));
    }
}
