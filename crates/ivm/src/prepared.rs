//! Immutable, validated contract programs prepared for repeated IVM execution.

use std::{collections::BTreeMap, fmt, sync::Arc};

use iroha_crypto::Hash;
use iroha_data_model::smart_contract::manifest::ContractManifest;

use crate::{
    ProgramMetadata, VMError,
    instruction::wide,
    ivm::PreparedProgram,
    ivm_cache::DecodedOp,
    metadata::{EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor},
};

#[derive(Clone, Copy, Debug)]
struct PreparedEntrypointIndex {
    descriptor_index: usize,
    absolute_pc: u64,
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
    boundaries: Arc<[u64]>,
    nodes: Arc<[PreparedControlFlowNode]>,
}

impl PreparedControlFlow {
    pub(crate) fn from_decoded(decoded: &[DecodedOp]) -> Result<Self, VMError> {
        let boundaries = decoded.iter().map(|op| op.pc).collect::<Vec<_>>();
        let is_boundary = |pc: u64| boundaries.binary_search(&pc).is_ok();
        let mut nodes = Vec::with_capacity(decoded.len());

        for op in decoded {
            let opcode = wide::opcode(op.inst);
            let fallthrough = op.pc.checked_add(u64::from(op.len));
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
            boundaries: Arc::from(boundaries.into_boxed_slice()),
            nodes: Arc::from(nodes.into_boxed_slice()),
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

pub(crate) struct PreparedContractParts {
    pub(crate) artifact: Arc<[u8]>,
    pub(crate) metadata: ProgramMetadata,
    pub(crate) manifest: ContractManifest,
    pub(crate) header_len: usize,
    pub(crate) code_offset: usize,
    pub(crate) code_hash: Hash,
    pub(crate) contract_interface: Arc<EmbeddedContractInterfaceV1>,
    pub(crate) literal_pointers: Arc<[u64]>,
    pub(crate) decoded: Arc<[DecodedOp]>,
    pub(crate) prepared_program: PreparedProgram,
    pub(crate) control_flow: PreparedControlFlow,
}

struct PreparedContractInner {
    artifact: Arc<[u8]>,
    metadata: ProgramMetadata,
    manifest: ContractManifest,
    header_len: usize,
    code_offset: usize,
    instruction_entry_pc: u64,
    code_hash: Hash,
    contract_interface: Arc<EmbeddedContractInterfaceV1>,
    entrypoints: BTreeMap<String, PreparedEntrypointIndex>,
    literal_pointers: Arc<[u64]>,
    decoded: Arc<[DecodedOp]>,
    prepared_program: PreparedProgram,
    control_flow: PreparedControlFlow,
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
            if entrypoints
                .insert(
                    descriptor.name.clone(),
                    PreparedEntrypointIndex {
                        descriptor_index,
                        absolute_pc,
                    },
                )
                .is_some()
            {
                return Err(VMError::DecodeError);
            }
        }
        Ok(Self {
            inner: Arc::new(PreparedContractInner {
                artifact: parts.artifact,
                metadata: parts.metadata,
                manifest: parts.manifest,
                header_len: parts.header_len,
                code_offset: parts.code_offset,
                instruction_entry_pc,
                code_hash: parts.code_hash,
                contract_interface: parts.contract_interface,
                entrypoints,
                literal_pointers: parts.literal_pointers,
                decoded: parts.decoded,
                prepared_program: parts.prepared_program,
                control_flow: parts.control_flow,
            }),
        })
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
    /// [`Arc`] does not copy the artifact bytes.
    #[must_use]
    pub fn shared_artifact(&self) -> Arc<[u8]> {
        Arc::clone(&self.inner.artifact)
    }

    /// Return the parsed execution metadata.
    #[must_use]
    pub fn metadata(&self) -> &ProgramMetadata {
        &self.inner.metadata
    }

    /// Return the manifest derived from the validated artifact.
    ///
    /// The manifest is retained with the prepared contract so admission and
    /// request paths can compare security claims without reparsing or
    /// re-decoding the deployable image.
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

    pub(crate) fn literal_pointers(&self) -> &Arc<[u64]> {
        &self.inner.literal_pointers
    }

    pub(crate) fn decoded(&self) -> &Arc<[DecodedOp]> {
        &self.inner.decoded
    }

    pub(crate) fn prepared_program(&self) -> &PreparedProgram {
        &self.inner.prepared_program
    }

    #[cfg(test)]
    pub(crate) fn shares_prepared_program_with(&self, other: &Self) -> bool {
        self.inner
            .prepared_program
            .shares_ops(&other.inner.prepared_program)
    }
}

impl fmt::Debug for PreparedContract {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedContract")
            .field("code_hash", &self.inner.code_hash)
            .field("header_len", &self.inner.header_len)
            .field("code_offset", &self.inner.code_offset)
            .field(
                "contract_name",
                &self.inner.contract_interface.contract_name,
            )
            .field("entrypoints", &self.inner.entrypoints.keys())
            .field("instructions", &self.inner.decoded.len())
            .finish()
    }
}
