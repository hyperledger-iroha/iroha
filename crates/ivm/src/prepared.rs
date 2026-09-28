//! Immutable, validated contract programs prepared for repeated IVM execution.
use crate::{
    ProgramMetadata, VMError,
    cache_memory::{MemoryReservation, SharedValue},
    instruction::wide,
    ivm::{DecodedLiteralTable, PreparedProgram},
    ivm_cache::DecodedOp,
    metadata::{EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor},
};
use iroha_crypto::Hash;
use iroha_data_model::smart_contract::manifest::ContractManifest;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::Arc,
};
#[derive(Clone, Copy, Debug)]
struct PreparedEntrypointIndex {
    descriptor_index: usize,
    absolute_pc: u64,
    requires_private_inputs: bool,
}
#[derive(Clone, Copy, Debug)]
struct PreparedControlFlowNode {
    pc: u64,
    successors: [u64; 2],
    successor_count: u8,
    has_indirect_successor: bool,
}
impl PreparedControlFlowNode {
    fn new(pc: u64) -> Self {
        Self {
            pc,
            successors: [0; 2],
            successor_count: 0,
            has_indirect_successor: false,
        }
    }
    fn push_successor(&mut self, target: u64) -> Result<(), VMError> {
        let index = usize::from(self.successor_count);
        let Some(slot) = self.successors.get_mut(index) else {
            return Err(VMError::DecodeError);
        };
        *slot = target;
        self.successor_count = self.successor_count.saturating_add(1);
        Ok(())
    }
    fn successors(&self) -> &[u64] {
        &self.successors[..usize::from(self.successor_count)]
    }
}
#[derive(Clone, Debug)]
pub(crate) struct PreparedControlFlow {
    boundaries: crate::cache_memory::SharedAllocation<u64>,
    nodes: crate::cache_memory::SharedAllocation<PreparedControlFlowNode>,
}
impl PreparedControlFlow {
    pub(crate) fn from_decoded(decoded: &[DecodedOp]) -> Result<Self, VMError> {
        let boundaries = decoded.iter().map(|op| op.pc).collect::<Vec<_>>();
        let is_boundary = |pc: u64| boundaries.binary_search(&pc).is_ok();
        let mut nodes = Vec::with_capacity(decoded.len());
        for op in decoded {
            let opcode = wide::opcode(op.inst);
            let fallthrough = op.pc.checked_add(4);
            let mut node = PreparedControlFlowNode::new(op.pc);
            match opcode {
                wide::control::HALT => {}
                wide::control::BEQ
                | wide::control::BNE
                | wide::control::BLT
                | wide::control::BGE
                | wide::control::BLTU
                | wide::control::BGEU => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                    let next = fallthrough.ok_or(VMError::DecodeError)?;
                    if !is_boundary(next) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(next)?;
                }
                wide::control::JAL => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                    if wide::rd(op.inst) != 0 {
                        let next = fallthrough.ok_or(VMError::DecodeError)?;
                        if !is_boundary(next) {
                            return Err(VMError::DecodeError);
                        }
                        node.push_successor(next)?;
                    }
                }
                wide::control::JMP => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                }
                wide::control::JALS => {
                    let target = direct_target(op).ok_or(VMError::DecodeError)?;
                    if !is_boundary(target) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(target)?;
                    let next = fallthrough.ok_or(VMError::DecodeError)?;
                    if !is_boundary(next) {
                        return Err(VMError::DecodeError);
                    }
                    node.push_successor(next)?;
                }
                wide::control::JALR | wide::control::JR => {
                    node.has_indirect_successor = true;
                }
                _ => {
                    if let Some(next) = fallthrough.filter(|next| is_boundary(*next)) {
                        node.push_successor(next)?;
                    }
                }
            }
            nodes.push(node);
        }
        Ok(Self {
            boundaries: boundaries.into(),
            nodes: nodes.into(),
        })
    }
    fn node(&self, pc: u64) -> Option<&PreparedControlFlowNode> {
        let index = self.boundaries.binary_search(&pc).ok()?;
        let node = self.nodes.get(index)?;
        debug_assert_eq!(node.pc, pc);
        Some(node)
    }
}
fn direct_target(op: &DecodedOp) -> Option<u64> {
    let offset_words = match wide::opcode(op.inst) {
        wide::control::BEQ
        | wide::control::BNE
        | wide::control::BLT
        | wide::control::BGE
        | wide::control::BLTU
        | wide::control::BGEU => i64::from(wide::imm8(op.inst)),
        wide::control::JAL => i64::from(wide::imm16(op.inst)),
        wide::control::JMP | wide::control::JALS => i64::from(wide::imm24(op.inst)),
        _ => return None,
    };
    let byte_offset = offset_words.checked_mul(4)?;
    i128::from(op.pc)
        .checked_add(i128::from(byte_offset))
        .and_then(|target| u64::try_from(target).ok())
}
fn decoded_syscall_number(op: &DecodedOp) -> Option<u32> {
    match wide::opcode(op.inst) {
        wide::system::SCALL => Some(u32::from(wide::imm8(op.inst) as u8)),
        wide::system::SYSTEM => Some(crate::encoding::wide::decode_syscallx(op.inst)),
        _ => None,
    }
}
fn entrypoint_reaches_private_input(
    decoded: &[DecodedOp],
    control_flow: &PreparedControlFlow,
    entry_pc: u64,
) -> Result<bool, VMError> {
    let mut pending = VecDeque::from([entry_pc]);
    let mut visited = BTreeSet::new();
    while let Some(pc) = pending.pop_front() {
        if !visited.insert(pc) {
            continue;
        }
        let index = decoded
            .binary_search_by_key(&pc, |op| op.pc)
            .map_err(|_| VMError::DecodeError)?;
        let op = decoded.get(index).ok_or(VMError::DecodeError)?;
        if decoded_syscall_number(op) == Some(crate::syscalls::SYSCALL_GET_PRIVATE_INPUT) {
            return Ok(true);
        }
        let node = control_flow.node(pc).ok_or(VMError::DecodeError)?;
        pending.extend(node.successors().iter().copied());
    }
    Ok(false)
}
pub(crate) struct PreparedContractParts {
    pub(crate) artifact: crate::cache_memory::SharedAllocation<u8>,
    pub(crate) metadata: ProgramMetadata,
    pub(crate) manifest: ContractManifest,
    pub(crate) header_len: usize,
    pub(crate) code_offset: usize,
    pub(crate) code_hash: Hash,
    pub(crate) contract_interface: SharedValue<EmbeddedContractInterfaceV1>,
    pub(crate) literal_table: DecodedLiteralTable,
    pub(crate) decoded: crate::ivm_cache::DecodedStream,
    pub(crate) prepared_program: PreparedProgram,
    pub(crate) control_flow: PreparedControlFlow,
}
struct PreparedContractInner {
    artifact: crate::cache_memory::SharedAllocation<u8>,
    metadata: ProgramMetadata,
    manifest: SharedValue<ContractManifest>,
    header_len: usize,
    code_offset: usize,
    instruction_entry_pc: u64,
    code_hash: Hash,
    contract_interface: SharedValue<EmbeddedContractInterfaceV1>,
    entrypoints: BTreeMap<String, PreparedEntrypointIndex>,
    literal_table: DecodedLiteralTable,
    decoded: crate::ivm_cache::DecodedStream,
    prepared_program: PreparedProgram,
    control_flow: PreparedControlFlow,
    reservation: MemoryReservation,
}
/// Immutable validated contract artifact ready for repeated IVM loading.
///
/// Cloning this value only clones one [`Arc`]. The complete deployable image,
/// decoded interface, literal index, prepared instruction stream, and validated
/// control-flow boundaries are shared by every runtime instance.
#[derive(Clone)]
pub struct PreparedContract {
    inner: Arc<PreparedContractInner>,
}
/// Normalize native metadata into a measured, immutable allocation owner.
///
/// Norito reports cumulative allocator requests, including temporary decode
/// storage. Keeping that conservative charge until the last owner drops avoids
/// estimating native metadata from encoded byte lengths. A local normalization
/// failure makes the original admitted value uncacheable; it cannot invalidate it.
pub(crate) fn shared_metadata<T>(value: T, exclusively_owned: bool) -> SharedValue<T>
where
    T: norito::NoritoSerialize + PartialEq,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    // TODO: Give trigger Json/other independently shared backing allocations
    // their own lifetime reservations. Cloning a child must not escape a charge
    // attached only to this aggregate owner; until then these values stay cold.
    if !exclusively_owned {
        return SharedValue::new(value, None);
    }
    let limits =
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX);
    let measured = norito::to_bytes(&value).ok().and_then(|bytes| {
        let (decoded, usage) = norito::core::with_decode_limits_measured(limits, || {
            norito::decode_from_bytes_with_limits::<T>(&bytes, limits)
        });
        decoded
            .ok()
            .map(|decoded| (decoded, usage.total_allocated_bytes()))
    });
    match measured {
        Some((decoded, heap_bytes)) if decoded == value => {
            SharedValue::new(decoded, Some(heap_bytes))
        }
        // The reservation marks unknown active owners explicitly; they can
        // never consume an unmeasured share of the retention budget.
        _ => SharedValue::new(value, None),
    }
}
impl PreparedContract {
    pub(crate) fn from_parts(parts: PreparedContractParts) -> Result<Self, VMError> {
        let instruction_entry_pc = parts
            .code_offset
            .checked_sub(parts.header_len)
            .and_then(|offset| u64::try_from(offset).ok())
            .ok_or(VMError::DecodeError)?;
        let mut entrypoints = BTreeMap::new();
        for (descriptor_index, descriptor) in
            parts.contract_interface.entrypoints.iter().enumerate()
        {
            let absolute_pc = instruction_entry_pc
                .checked_add(descriptor.entry_pc)
                .ok_or(VMError::DecodeError)?;
            let requires_private_inputs = entrypoint_reaches_private_input(
                parts.decoded.as_ref(),
                &parts.control_flow,
                descriptor.entry_pc,
            )?;
            if entrypoints
                .insert(
                    descriptor.name.clone(),
                    PreparedEntrypointIndex {
                        descriptor_index,
                        absolute_pc,
                        requires_private_inputs,
                    },
                )
                .is_some()
            {
                return Err(VMError::DecodeError);
            }
        }
        let index_bytes = norito::core::owned_btree_allocation_bytes::<
            String,
            PreparedEntrypointIndex,
        >(entrypoints.len())
        .expect("validated entrypoint index fits host allocation limits")
            + entrypoints.keys().map(String::capacity).sum::<usize>();
        let reservation = MemoryReservation::active(
            std::mem::size_of::<PreparedContractInner>()
                + 2 * std::mem::size_of::<usize>()
                + index_bytes,
        );
        Ok(Self {
            inner: Arc::new(PreparedContractInner {
                artifact: parts.artifact,
                metadata: parts.metadata,
                manifest: {
                    let exclusively_owned =
                        parts.manifest.entrypoints.as_ref().is_none_or(|entries| {
                            entries.iter().all(|entry| entry.triggers.is_empty())
                        });
                    shared_metadata(parts.manifest, exclusively_owned)
                },
                header_len: parts.header_len,
                code_offset: parts.code_offset,
                instruction_entry_pc,
                code_hash: parts.code_hash,
                contract_interface: parts.contract_interface,
                entrypoints,
                literal_table: parts.literal_table,
                decoded: parts.decoded,
                prepared_program: parts.prepared_program,
                control_flow: parts.control_flow,
                reservation,
            }),
        })
    }
    /// Whether two prepared handles share the same immutable allocation owner.
    #[must_use]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Arc::ptr_eq(&this.inner, &other.inner)
    }
    /// Attempt nonblocking retention of the prepared program's shared allocations.
    ///
    /// A false result only disables caching; the program remains valid and executable.
    pub fn try_retain_allocations(&self) -> bool {
        self.inner.contract_interface.try_retain()
            && self.inner.manifest.try_retain()
            && self.inner.reservation.try_retain()
            && self.inner.artifact.try_retain()
            && self.inner.decoded.try_retain()
            && self.inner.prepared_program.try_retain()
            && self.inner.literal_table.try_retain()
            && self.inner.control_flow.boundaries.try_retain()
            && self.inner.control_flow.nodes.try_retain()
    }
    /// Return the canonical full-artifact hash used as the preparation key.
    #[must_use]
    pub fn code_hash(&self) -> Hash {
        self.inner.code_hash
    }
    /// Return the complete canonical deployable `.to` image.
    #[must_use]
    pub fn artifact(&self) -> &[u8] {
        &self.inner.artifact
    }
    /// Clone the shared complete canonical deployable `.to` image.
    ///
    /// This is intended for asynchronous consumers that must retain the
    /// artifact after the prepared-contract borrow ends. Cloning the returned
    /// handle does not copy the artifact bytes and keeps their memory charge alive.
    #[must_use]
    pub fn shared_artifact(&self) -> crate::cache_memory::SharedAllocation<u8> {
        self.inner.artifact.clone()
    }
    /// Return the parsed execution metadata.
    #[must_use]
    pub fn metadata(&self) -> &ProgramMetadata {
        &self.inner.metadata
    }
    /// Return the manifest derived from the validated artifact.
    ///
    /// The manifest is retained with the prepared contract so admission and request paths can
    /// compare security claims without reparsing or re-decoding the deployable image.
    #[must_use]
    pub fn manifest(&self) -> &ContractManifest {
        &self.inner.manifest
    }
    /// Return the fixed metadata-header length.
    #[must_use]
    pub fn header_len(&self) -> usize {
        self.inner.header_len
    }
    /// Return the artifact offset of the executable instruction stream.
    #[must_use]
    pub fn code_offset(&self) -> usize {
        self.inner.code_offset
    }
    /// Return the decoded, validated contract interface.
    #[must_use]
    pub fn contract_interface(&self) -> &EmbeddedContractInterfaceV1 {
        &self.inner.contract_interface
    }
    pub(crate) fn shared_contract_interface(&self) -> SharedValue<EmbeddedContractInterfaceV1> {
        self.inner.contract_interface.clone()
    }
    /// Resolve an entrypoint to its absolute PC in IVM code memory.
    #[must_use]
    pub fn entrypoint_pc(&self, name: &str) -> Option<u64> {
        self.inner
            .entrypoints
            .get(name)
            .map(|entrypoint| entrypoint.absolute_pc)
    }
    /// Resolve an entrypoint descriptor without reparsing the embedded interface.
    #[must_use]
    pub fn entrypoint_descriptor(&self, name: &str) -> Option<&EmbeddedEntrypointDescriptor> {
        let index = self.inner.entrypoints.get(name)?.descriptor_index;
        self.inner.contract_interface.entrypoints.get(index)
    }
    /// Return whether validated bytecode reachable from `name` reads a private witness.
    ///
    /// This fact is derived from the complete decoded call graph while the
    /// immutable contract is prepared; it never trusts a CNTR effect claim.
    #[must_use]
    pub fn entrypoint_requires_private_inputs(&self, name: &str) -> Option<bool> {
        self.inner
            .entrypoints
            .get(name)
            .map(|entrypoint| entrypoint.requires_private_inputs)
    }
    /// Return the validated relative instruction boundaries.
    #[must_use]
    pub fn instruction_boundaries(&self) -> &[u64] {
        &self.inner.control_flow.boundaries
    }
    /// Return whether `relative_pc` is a validated instruction boundary.
    #[must_use]
    pub fn is_instruction_boundary(&self, relative_pc: u64) -> bool {
        self.inner
            .control_flow
            .boundaries
            .binary_search(&relative_pc)
            .is_ok()
    }
    /// Return statically known successor PCs for a relative instruction PC.
    ///
    /// An empty slice represents a halt, return, or unverifiable indirect edge.
    #[must_use]
    pub fn control_flow_successors(&self, relative_pc: u64) -> Option<&[u64]> {
        self.inner
            .control_flow
            .node(relative_pc)
            .map(PreparedControlFlowNode::successors)
    }
    /// Return whether a relative instruction has an indirect control-flow edge.
    #[must_use]
    pub fn has_indirect_control_flow(&self, relative_pc: u64) -> bool {
        self.inner
            .control_flow
            .node(relative_pc)
            .is_some_and(|node| node.has_indirect_successor)
    }
    pub(crate) fn code_region(&self) -> &[u8] {
        &self.inner.artifact[self.inner.header_len..]
    }
    pub(crate) fn instruction_entry_pc(&self) -> u64 {
        self.inner.instruction_entry_pc
    }
    pub(crate) fn literal_table(&self) -> &DecodedLiteralTable {
        &self.inner.literal_table
    }
    pub(crate) fn decoded(&self) -> &crate::ivm_cache::DecodedStream {
        &self.inner.decoded
    }
    pub(crate) fn prepared_program(&self) -> &PreparedProgram {
        &self.inner.prepared_program
    }
}
impl AsRef<PreparedContract> for PreparedContract {
    fn as_ref(&self) -> &PreparedContract {
        self
    }
}
impl fmt::Debug for PreparedContract {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedContract")
            .field("code_hash", &self.inner.code_hash)
            .field("header_len", &self.inner.header_len)
            .field("code_offset", &self.inner.code_offset)
            .field("seiyaku_name", &self.inner.contract_interface.seiyaku_name)
            .field("entrypoints", &self.inner.entrypoints.keys())
            .field("instructions", &self.inner.decoded.len())
            .finish()
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::shared_metadata;

    #[test]
    fn independently_shared_metadata_cannot_enter_aggregate_retention() {
        let source = vec!["shared child".to_owned()];
        let owner = shared_metadata(source.clone(), false);
        assert_eq!(&*owner, &source);
        assert_eq!(owner.allocation_bytes(), None);
        assert!(!owner.try_retain());
    }
    #[test]
    fn metadata_normalization_measures_native_storage_and_preserves_values() {
        let mut nested = String::with_capacity(16_384);
        nested.push_str("retained interface");
        let mut source = Vec::with_capacity(1024);
        source.push(nested);
        let normalized = shared_metadata(source, true);
        assert_eq!(normalized.as_ref(), &["retained interface".to_owned()]);
        let native_bytes = normalized.capacity() * std::mem::size_of::<String>()
            + normalized.iter().map(String::capacity).sum::<usize>();
        assert!(
            normalized
                .allocation_bytes()
                .expect("measured Norito allocations")
                >= native_bytes
        );
        // Normalizing removes producer spare capacity without deriving the
        // retained footprint from the wire encoding's byte length.
        assert!(normalized.capacity() < 1024);
        assert!(normalized[0].capacity() < 16_384);
        let borrower = normalized.clone();
        drop(normalized);
        assert_eq!(borrower[0], "retained interface");
    }
}
