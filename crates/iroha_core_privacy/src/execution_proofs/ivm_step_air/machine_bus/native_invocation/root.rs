//! Public initializer semantics derived from the retained admitted artifact.
//!
//! No expected value is read from native observations. These are fixed public
//! coefficients of the initializer relation, not a native replay or a supplied
//! equality between commitments. Root authority is selected through the public
//! entrypoint table before its callable metadata can authorize any frame.

use super::{F, ROOT_SLOTS, packet};
use crate::execution_proofs::ivm_step_air::residues::Sink;
use ivm::{IvmStackPolicy, Memory, PreparedContract, execution_packets::MAX_STEPS};
use ivm_abi::{call::CallTypeNodeV1, entrypoint::EntrypointValueKindV1};

/// Fixed public coefficient selected only from the retained admitted artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PublicLeafKind {
    Unit,
    Bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Error {
    Profile,
    Entrypoint,
    Geometry,
    Gas,
}

pub(super) struct Plan {
    leaf_kind: PublicLeafKind,
    gas: [u64; 3],
    absolute_entry: u64,
    return_pc: u64,
    cycle_limit: u64,
    stack_top: u64,
    descriptor: [u64; 8],
}
impl Plan {
    pub(super) fn derive(
        artifact: &PreparedContract,
        public_entrypoint: usize,
        initial_gas: u64,
    ) -> Result<Self, Error> {
        let metadata = artifact.metadata();
        if metadata.abi_version != 1
            || metadata.mode != ivm::ivm_mode::ZK
            || !(1..=MAX_STEPS as u64).contains(&metadata.max_cycles)
            || artifact
                .artifact()
                .len()
                .checked_sub(artifact.code_offset())
                .is_none_or(|bytes| bytes > MAX_STEPS * 4)
        {
            return Err(Error::Profile);
        }
        let interface = artifact.contract_interface();
        let public = interface
            .entrypoints
            .get(public_entrypoint)
            .ok_or(Error::Entrypoint)?;
        let callable = interface
            .callables
            .iter()
            .find(|call| call.entry_pc == public.entry_pc)
            .ok_or(Error::Entrypoint)?;
        if public.argument_schema.is_some() || !callable.arguments.nodes.is_empty() {
            return Err(Error::Profile);
        }
        let leaf_kind = match callable.results.nodes.as_slice() {
            [CallTypeNodeV1::Unit] => PublicLeafKind::Unit,
            [CallTypeNodeV1::Leaf(EntrypointValueKindV1::Bool)] => PublicLeafKind::Bool,
            _ => return Err(Error::Profile),
        };
        let prefix = artifact
            .code_offset()
            .checked_sub(artifact.header_len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::Geometry)?;
        let absolute_entry = prefix.checked_add(public.entry_pc).ok_or(Error::Geometry)?;
        let return_pc = artifact
            .artifact()
            .len()
            .checked_sub(artifact.header_len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::Geometry)?;
        if absolute_entry >= return_pc {
            return Err(Error::Geometry);
        }
        let stack_top = Memory::STACK_START
            .checked_add(IvmStackPolicy::V1.stack_limit_for_gas(initial_gas))
            .ok_or(Error::Geometry)?;
        let stack_start = stack_top
            .checked_sub(u64::from(callable.frame_bytes))
            .filter(|start| *start >= Memory::STACK_START)
            .ok_or(Error::Geometry)?;
        let result_end = Memory::HEAP_START.checked_add(8).ok_or(Error::Geometry)?;
        let allocated_gas = initial_gas.checked_sub(8).ok_or(Error::Gas)?;
        // V1 fixes one bitmap byte per eight frame bytes and one per result
        // word. This is public metadata arithmetic, never a captured debit.
        let frame_cost = u64::from(callable.frame_bytes).div_ceil(8) + 1;
        let entered_gas = allocated_gas.checked_sub(frame_cost).ok_or(Error::Gas)?;
        Ok(Self {
            leaf_kind,
            gas: [initial_gas, allocated_gas, entered_gas],
            absolute_entry,
            return_pc,
            cycle_limit: metadata.max_cycles,
            stack_top,
            descriptor: [
                stack_start,
                stack_top,
                0,
                0,
                Memory::HEAP_START,
                result_end,
                stack_top,
                callable.entry_pc,
            ],
        })
    }

    /// Exact one-node kind; no native value, summary or witness selects it.
    pub(super) fn leaf_kind(&self) -> PublicLeafKind {
        self.leaf_kind
    }

    /// The admitted artifact fixes successful native ZK padding geometry.
    pub(super) fn cycle_limit(&self) -> u64 {
        self.cycle_limit
    }

    /// Only public initializer semantics supply the root stack and result regions.
    /// LOAD reads the stack; STORE may also write the result. Compact instructions
    /// cannot mutate descriptors or enter child frames.
    pub(super) fn memory_bounds(&self) -> [[u64; 2]; 2] {
        [
            [self.descriptor[0], self.descriptor[1]],
            [self.descriptor[4], self.descriptor[5]],
        ]
    }

    /// Derive all 64 original slots, including mandatory zero gaps. The first
    /// access to every state cell is zero; no observed prestate supplies values.
    fn expected(&self, clock: usize) -> [F; packet::WIDTH] {
        assert!(clock < ROOT_SLOTS);
        let (space, generation, index, before, after) = match clock {
            0 => (packet::Space::Owner, 0, 33, 0, self.gas[0]),
            1 => (packet::Space::Owner, 0, 32, 0, self.absolute_entry),
            2 => (packet::Space::Owner, 0, 35, 0, 1),
            3 => (packet::Space::Owner, 0, 20, 0, self.stack_top),
            4 => (packet::Space::Owner, 0, 21, 0, Memory::HEAP_START),
            8 => (packet::Space::Owner, 0, 33, self.gas[0], self.gas[1]),
            9 => (packet::Space::Owner, 0, 33, self.gas[1], self.gas[2]),
            16 => (packet::Space::Register, 0, 10, 0, 0),
            17 => (packet::Space::Register, 0, 11, 0, 0),
            18 => (packet::Space::Register, 0, 12, 0, Memory::HEAP_START),
            19 => (packet::Space::Register, 0, 13, 0, 1),
            20 => (packet::Space::Register, 0, 31, 0, self.stack_top),
            21 => (packet::Space::Register, 0, 1, 0, self.return_pc),
            22 => (
                packet::Space::Owner,
                0,
                21,
                Memory::HEAP_START,
                self.descriptor[5],
            ),
            24 => (packet::Space::Owner, 0, 1, 0, 1),
            25 => (packet::Space::Owner, 0, 0, 0, 1),
            26 => (packet::Space::Owner, 1, 2, 0, 0),
            32..=39 => (
                packet::Space::Owner,
                1,
                4 + (clock - 32) as u32,
                0,
                self.descriptor[clock - 32],
            ),
            40 => (packet::Space::Owner, 1, 12, 0, self.return_pc),
            _ => return [F::ZERO; packet::WIDTH],
        };
        let mut fields = [F::ZERO; packet::WIDTH];
        fields[packet::SPACE] = F(space as u64);
        fields[packet::GENERATION] = F(generation);
        fields[packet::INDEX] = F(u64::from(index));
        fields[packet::KEY] = F(u64::from(index) + (generation << 32) + ((space as u64) << 56));
        fields[packet::CLOCK] = F(clock as u64);
        fields[packet::ENABLED] = F::ONE;
        fields[packet::WRITE] = F::ONE;
        for (offset, value) in [(packet::BEFORE, before), (packet::AFTER, after)] {
            for limb in 0..4 {
                fields[offset + limb] = F((value >> (16 * limb)) & 0xffff);
            }
        }
        fields
    }

    /// Linear equations in the original producer fields. Fixed coefficients
    /// depend only on admitted public metadata and the invocation's public gas.
    pub(super) fn append_residues(
        &self,
        out: &mut impl Sink,
        clock: usize,
        producer: &[F; packet::WIDTH],
    ) {
        for (actual, expected) in producer.iter().zip(self.expected(clock)) {
            out.push(actual.sub(expected));
        }
    }
}
