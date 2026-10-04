//! Original native ownership joined to root initialization and complete history.
//!
//! Native capture is witness production, not an execution proof. The root bank
//! constrains initial state from public artifact/gas semantics; private history
//! constrains consistency of the same original packets at their actual clocks.
//! The narrower instruction join consumes thirty-one public straight-line
//! arithmetic and bit operations, admitted scalar literals, public-root LOAD64/STORE64
//! and root-return control through the dispatcher.
//! Atomic destinations, initialized stack reads, STORE effects and all compact
//! gaps use original clocks; private/wide memory and faults remain open.
//! The Unit return joins all original operands, both staged validation debits,
//! its typed memory read and every initialization-scan cell. The successful
//! terminal bank constrains optional padding and the complete unused suffix;
//! compact activity is an exact prefix before the single fixed root return.
//! TODO: extend memory/typed/instruction coverage and faults, and bind final
//! output/publication and these columns in the masked invocation transcript.
//! No State/intent authority, verifier registration or proof admission is added.

use super::{F, PHASES, packet, private_history};
use crate::execution_proofs::ivm_step_air::residues::{Scratch, Sink, Stream};
use iroha_allocation::AllocationBudget;
use ivm::execution_packets::{
    INSTRUCTION_WINDOWS, NativeInvocation, NativePacket, PACKET_SLOTS, ROOT_SLOTS,
};

mod instructions;
mod returning;
mod root;
mod terminal;

/// One small projection scratch; its private fields never escape by cloning.
struct Fields([F; packet::WIDTH]);
impl Fields {
    fn native(packet: &NativePacket) -> Self {
        let mut fields = [F::ZERO; packet::WIDTH];
        let space = packet.space().map_or(0, |space| space as u64);
        fields[packet::SPACE] = F(space);
        fields[packet::GENERATION] = F(u64::from(packet.generation()));
        fields[packet::INDEX] = F(u64::from(packet.index()));
        fields[packet::KEY] =
            F(u64::from(packet.index()) + (u64::from(packet.generation()) << 32) + (space << 56));
        // Preserve the original clock, including the zero clock of padding.
        fields[packet::CLOCK] = F(u64::from(packet.clock()));
        fields[packet::ENABLED] = F(u64::from(packet.enabled()));
        fields[packet::WRITE] = F(u64::from(packet.is_write()));
        for (offset, bytes) in [
            (packet::BEFORE, packet.before()),
            (packet::AFTER, packet.after()),
        ] {
            for (limb, pair) in bytes.chunks_exact(2).enumerate() {
                fields[offset + limb] = F(u64::from(u16::from_le_bytes([pair[0], pair[1]])));
            }
        }
        fields[packet::BEFORE_TAG] = F(u64::from(packet.before_private()));
        fields[packet::AFTER_TAG] = F(u64::from(packet.after_private()));
        Self(fields)
    }
    fn clear(&mut self) {
        for value in &mut self.0 {
            value.zeroize_v1();
        }
    }
}
impl Drop for Fields {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Sole owner of the original interpreter output. There is no raw packet,
/// initializer-array or caller-selected clock constructor.
pub(super) struct Source {
    native: NativeInvocation,
    root: root::Plan,
    instructions: instructions::Instructions,
    returning: returning::Returning,
    terminal: terminal::Terminal,
}
#[derive(Debug)]
pub(super) enum SourceError {
    Initializer(root::Error),
    Instructions(instructions::Error),
    Return(returning::Error),
    Terminal(iroha_allocation::ChargedBufferError),
}
impl Source {
    pub(super) fn new(
        native: NativeInvocation,
        budget: &AllocationBudget,
    ) -> Result<Self, SourceError> {
        let root = root::Plan::derive(
            native.artifact(),
            native.entrypoint_index(),
            native.initial_gas(),
        )
        .map_err(SourceError::Initializer)?;
        let instructions = instructions::Instructions::new(&native, &root, budget)
            .map_err(SourceError::Instructions)?;
        let returning = returning::Returning::new(&native, budget).map_err(SourceError::Return)?;
        let terminal =
            terminal::Terminal::new(&native, &root, budget).map_err(SourceError::Terminal)?;
        Ok(Self {
            native,
            root,
            instructions,
            returning,
            terminal,
        })
    }
    fn packet(&self, clock: usize) -> Fields {
        Fields::native(&self.native.packets()[clock])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    Root(usize),
    Instruction(usize),
    Return(returning::Row),
    Terminal(terminal::Row),
    History(usize),
}
#[derive(Debug)]
pub(super) enum EvaluationError<E> {
    History(private_history::ShapeError),
    Consumer(E),
}

/// Stream every initializer, admitted instruction and original history phase.
/// The history view retains the sole challenge family; callers cannot replace
/// it per producer, row or window. No extra backing or packet banks are copied.
pub(super) fn evaluate<E>(
    source: &Source,
    history: &private_history::View<'_>,
    mut consume: impl FnMut(Row, &[F]) -> Result<(), E>,
) -> Result<(), EvaluationError<E>> {
    history
        .require_complete_slots(PACKET_SLOTS)
        .map_err(EvaluationError::History)?;
    let mut scratch = Scratch::new();
    for clock in 0..ROOT_SLOTS {
        let fields = source.packet(clock);
        let mut emit = |values: &[F]| consume(Row::Root(clock), values);
        let mut out = Stream::new(&mut scratch, &mut emit);
        source.root.append_residues(&mut out, clock, &fields.0);
        debug_assert_eq!(out.len(), packet::WIDTH);
        out.finish().map_err(EvaluationError::Consumer)?;
    }
    for window in 0..INSTRUCTION_WINDOWS {
        let mut emit = |values: &[F]| consume(Row::Instruction(window), values);
        let mut out = Stream::new(&mut scratch, &mut emit);
        source
            .instructions
            .append_residues(&mut out, &source.native, window);
        out.finish().map_err(EvaluationError::Consumer)?;
    }
    source
        .returning
        .evaluate(&source.native, &mut scratch, |row, residues| {
            consume(Row::Return(row), residues)
        })
        .map_err(EvaluationError::Consumer)?;
    source
        .terminal
        .evaluate(&source.native, &mut scratch, |row, residues| {
            consume(Row::Terminal(row), residues)
        })
        .map_err(EvaluationError::Consumer)?;
    for clock in 0..PACKET_SLOTS {
        let fields = source.packet(clock);
        for phase in 0..PHASES {
            let index = clock * PHASES + phase;
            let mut emit = |values: &[F]| consume(Row::History(index), values);
            let mut out = Stream::new(&mut scratch, &mut emit);
            history.append_row(&mut out, index, &fields.0);
            debug_assert_eq!(out.len(), super::CONSTRAINTS);
            out.finish().map_err(EvaluationError::Consumer)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
