//! Canonical history state and constrained cells; encoding alone grants no authority.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{Bit, GlueChip, Uint, Word};
use iroha_plonk_recursion::verifier::VerifierChip;

use super::{HistoryAnchor, STATE_DOMAIN};

/// One authenticated height slot. Pending slots carry no signing authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistorySlot {
    /// Whether authorization awaits the exact incumbent boundary.
    pub pending: bool,
    /// Ready scheduling epoch; zero for Pending.
    pub epoch: u64,
    /// Ready canonical context identifier; zero for Pending.
    pub context: [u8; 32],
    /// Pending boundary height; zero for Ready.
    pub boundary_height: u64,
    /// Pending incumbent context; zero for Ready.
    pub predecessor: [u8; 32],
    /// Block/retry/execution/apply times, payload bound and epoch length.
    pub parameters: [u64; 6],
}
impl HistorySlot {
    /// Whether fields unused by this exact variant are canonically zero.
    pub fn is_canonical(&self) -> bool {
        if self.pending {
            self.epoch == 0 && self.context == [0; 32]
        } else {
            self.boundary_height == 0 && self.predecessor == [0; 32]
        }
    }
    fn words(&self) -> Vec<Fp> {
        let mut words = vec![Fp::from(u64::from(self.pending)), Fp::from(self.epoch)];
        words.extend(self.context.chunks_exact(16).map(pack));
        words.push(Fp::from(self.boundary_height));
        words.extend(self.predecessor.chunks_exact(16).map(pack));
        words.extend(self.parameters.map(Fp::from));
        words
    }
}

/// Complete state after a contiguous genesis-rooted prefix.
/// The next height and both slots have native u64 geometry, independent of the
/// source proof's fixed singleton cursor envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryState {
    /// Next block height to authenticate.
    pub next_height: u64,
    /// Authority and parameters for `next_height`; must be Ready.
    pub current: HistorySlot,
    /// Original lag-two promise for `next_height + 1`.
    pub following: HistorySlot,
    /// Last authenticated result digest; zero at the genesis start.
    pub result: [u8; 32],
    /// Last authenticated complete original result tape; zero at genesis.
    pub tape_root: Fp,
    /// Last authenticated result length; zero at genesis.
    pub frame_len: u32,
}
impl HistoryState {
    /// Exact initial height-two state from independently authenticated fixed policy.
    pub fn genesis(anchor: &HistoryAnchor) -> Self {
        let ready = HistorySlot {
            epoch: anchor.initial_epoch,
            context: anchor.initial_context,
            parameters: anchor.parameters,
            ..HistorySlot::default()
        };
        Self {
            next_height: 2,
            current: ready,
            following: ready,
            result: [0; 32],
            tape_root: Fp::ZERO,
            frame_len: 0,
        }
    }
    /// Canonical geometry only; this does not authenticate any state or result.
    pub fn is_canonical(&self) -> bool {
        self.next_height >= 2
            && !self.current.pending
            && self.current.is_canonical()
            && self.following.is_canonical()
            && self.frame_len <= crate::finality::result::MAX_RESULT_BYTES
    }
    /// Complete canonical state commitment under the selected genesis policy.
    pub fn digest(&self, anchor_digest: Fp) -> Fp {
        let mut words = vec![anchor_digest, Fp::from(self.next_height)];
        words.extend(self.current.words());
        words.extend(self.following.words());
        words.extend(self.result.chunks_exact(16).map(pack));
        words.extend([self.tape_root, Fp::from(u64::from(self.frame_len))]);
        hash_with_domain(STATE_DOMAIN, &words)
    }
}
fn pack(bytes: &[u8]) -> Fp {
    bytes
        .iter()
        .rev()
        .fold(Fp::ZERO, |v, b| v * Fp::from(256) + Fp::from(u64::from(*b)))
}
fn pack_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    bytes: &[Word<Fp>],
) -> Result<Word<Fp>, Error> {
    let mut packed = chip.uint().glue().constant(region, Fp::ZERO)?;
    for byte in bytes.iter().rev() {
        packed = chip.uint().glue().linear(
            region,
            &[(Fp::from(256), &packed), (Fp::ONE, byte)],
            Fp::ZERO,
        )?;
    }
    Ok(packed)
}
fn bytes(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    value: Value<[u8; 32]>,
) -> Result<[Word<Fp>; 32], Error> {
    (0..32)
        .map(|i| {
            chip.uint()
                .assign::<8>(region, value.map(|v| u128::from(v[i])))
                .map(|v| v.word().clone())
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Synthesis)
}

/// Constrained canonical slot fields, with an explicit pending-authority barrier.
#[derive(Clone, Debug)]
pub struct HistorySlotCells {
    pending: Bit<Fp>,
    epoch: Uint<Fp, 64>,
    context: [Word<Fp>; 32],
    boundary_height: Uint<Fp, 64>,
    predecessor: [Word<Fp>; 32],
    parameters: [Uint<Fp, 64>; 6],
    words: Vec<Word<Fp>>,
}
impl HistorySlotCells {
    /// Assign a canonical untrusted slot, enforcing every unused field is zero.
    /// # Errors
    /// Layout errors; noncanonical variants fail their constraints.
    pub fn assign(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Value<HistorySlot>,
    ) -> Result<Self, Error> {
        let pending = chip
            .uint()
            .glue()
            .boolean(region, value.map(|v| v.pending))?;
        let ready = chip.uint().glue().not(region, &pending)?;
        let epoch = chip
            .uint()
            .assign::<64>(region, value.map(|v| u128::from(v.epoch)))?;
        let context = bytes(chip, region, value.map(|v| v.context))?;
        let boundary_height = chip
            .uint()
            .assign::<64>(region, value.map(|v| u128::from(v.boundary_height)))?;
        let predecessor = bytes(chip, region, value.map(|v| v.predecessor))?;
        let parameters: [Uint<Fp, 64>; 6] = (0..6)
            .map(|i| {
                chip.uint()
                    .assign(region, value.map(|v| u128::from(v.parameters[i])))
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let mut words = vec![pending.word().clone(), epoch.word().clone()];
        for half in context.chunks_exact(16) {
            words.push(pack_cells(chip, region, half)?);
        }
        words.push(boundary_height.word().clone());
        for half in predecessor.chunks_exact(16) {
            words.push(pack_cells(chip, region, half)?);
        }
        words.extend(parameters.iter().map(|p| p.word().clone()));
        for (flag, indices) in [(&pending, [1, 2, 3]), (&ready, [4, 5, 6])] {
            for i in indices {
                let zero = chip.uint().glue().mul(region, flag.word(), &words[i])?;
                GlueChip::assert_constant(region, &zero, Fp::ZERO)?;
            }
        }
        Ok(Self {
            pending,
            epoch,
            context,
            boundary_height,
            predecessor,
            parameters,
            words,
        })
    }
    /// Explicit pending-authority barrier.
    pub const fn pending(&self) -> &Bit<Fp> {
        &self.pending
    }
    /// Ready epoch, zero while Pending.
    pub const fn epoch(&self) -> &Uint<Fp, 64> {
        &self.epoch
    }
    /// Ready context, zero while Pending.
    pub const fn context(&self) -> &[Word<Fp>; 32] {
        &self.context
    }
    /// Exact pending boundary, zero while Ready.
    pub const fn boundary_height(&self) -> &Uint<Fp, 64> {
        &self.boundary_height
    }
    /// Exact pending incumbent, zero while Ready.
    pub const fn predecessor(&self) -> &[Word<Fp>; 32] {
        &self.predecessor
    }
    /// Exact six native lag-two parameters.
    pub const fn parameters(&self) -> &[Uint<Fp, 64>; 6] {
        &self.parameters
    }
    pub(super) fn words(&self) -> &[Word<Fp>] {
        &self.words
    }
}

/// Complete constrained state commitment. Its source proof must supply authority.
#[derive(Clone, Debug)]
pub struct HistoryStateCells {
    digest: Word<Fp>,
    next_height: Uint<Fp, 64>,
    current: HistorySlotCells,
    following: HistorySlotCells,
    result: [Word<Fp>; 32],
    tape_root: Word<Fp>,
    frame_len: Uint<Fp, 32>,
}
impl HistoryStateCells {
    /// Assign all state fields and hash them under the same constrained fixed anchor.
    /// # Errors
    /// Layout errors; invalid height, variant or frame bounds fail constraints.
    pub fn assign(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: &Value<HistoryState>,
        anchor_digest: &Word<Fp>,
    ) -> Result<Self, Error> {
        let next_height = chip
            .uint()
            .assign::<64>(region, value.map(|v| u128::from(v.next_height)))?;
        let two = chip.uint().constant::<64>(region, 2)?;
        chip.uint().assert_le(region, &two, &next_height)?;
        let current = HistorySlotCells::assign(chip, region, value.map(|v| v.current))?;
        let following = HistorySlotCells::assign(chip, region, value.map(|v| v.following))?;
        GlueChip::assert_constant(region, current.pending().word(), Fp::ZERO)?;
        let result = bytes(chip, region, value.map(|v| v.result))?;
        let tape_root = chip
            .uint()
            .glue()
            .witness(region, value.map(|v| v.tape_root))?;
        let frame_len = chip
            .uint()
            .assign::<32>(region, value.map(|v| u128::from(v.frame_len)))?;
        let maximum = chip.uint().constant::<32>(
            region,
            u128::from(crate::finality::result::MAX_RESULT_BYTES),
        )?;
        chip.uint().assert_le(region, &frame_len, &maximum)?;
        let mut words = vec![anchor_digest.clone(), next_height.word().clone()];
        words.extend_from_slice(current.words());
        words.extend_from_slice(following.words());
        for half in result.chunks_exact(16) {
            words.push(pack_cells(chip, region, half)?);
        }
        words.extend([tape_root.clone(), frame_len.word().clone()]);
        let digest = chip.hash_words(region, STATE_DOMAIN, &words)?;
        Ok(Self {
            digest,
            next_height,
            current,
            following,
            result,
            tape_root,
            frame_len,
        })
    }
    /// Commitment to every state field and the fixed anchor.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Next native consensus height.
    pub const fn next_height(&self) -> &Uint<Fp, 64> {
        &self.next_height
    }
    /// Exact Ready authority for the next block.
    pub const fn current(&self) -> &HistorySlotCells {
        &self.current
    }
    /// Exact original lag-two promise.
    pub const fn following(&self) -> &HistorySlotCells {
        &self.following
    }
    /// Last authenticated result, zero only at the fixed genesis start.
    pub const fn result(&self) -> &[Word<Fp>; 32] {
        &self.result
    }
    /// Last complete original result tape.
    pub const fn tape_root(&self) -> &Word<Fp> {
        &self.tape_root
    }
    /// Last complete original result length.
    pub const fn frame_len(&self) -> &Uint<Fp, 32> {
        &self.frame_len
    }
}

#[cfg(test)]
mod tests;
