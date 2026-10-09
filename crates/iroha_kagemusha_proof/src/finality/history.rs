//! Genesis-rooted ordinary-validator history with full native u64 heights.
//!
//! The installation owner authenticates the selected global signed genesis and
//! pins this complete anchor and the original source catalog. Constructing an
//! anchor from caller-provided fields supplies no authority. History endpoints
//! use a fixed 0..1 envelope; the constrained state contains the native height.
//! This keeps the source instruction cursor distinct from consensus height.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Uint, Word};
use iroha_plonk_recursion::verifier::VerifierChip;

/// One exact block step, authenticated separately from a complete history prefix.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwhist1");
/// Closed genesis-rooted prefix under the finite shared history wrapper catalog.
pub const PREFIX_PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwhprp1");
/// Bind the independent genesis anchor and complete actual history-wrapper key.
pub const PREFIX_CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwhprc1");
/// Native context prediction; the receipt owner independently pins the wrapper.
pub fn prefix_context(anchor: Fp, history_key: Fp) -> Fp {
    hash_with_domain(PREFIX_CONTEXT_DOMAIN, &[anchor, history_key])
}
/// Bind prefix context to the actual hard-verified wrapper key.
/// # Errors
/// Circuit layout errors.
pub fn prefix_context_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    anchor: &Word<Fp>,
    history_key: &Word<Fp>,
) -> Result<Word<Fp>, Error> {
    chip.hash_words(
        region,
        PREFIX_CONTEXT_DOMAIN,
        &[anchor.clone(), history_key.clone()],
    )
}
/// Domain binding every independently selected genesis policy field.
pub const ANCHOR_DOMAIN: u64 = u64::from_le_bytes(*b"kgwhian1");
/// Domain binding the complete history state and its selected genesis anchor.
pub const STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwhiss1");

mod state;
pub use state::{HistorySlot, HistorySlotCells, HistoryState, HistoryStateCells};
mod genesis;
pub use genesis::GenesisSourceCircuit;
mod append;
pub use append::{HistoryAppendCircuit, HistoryAppendPlan};
mod native;
pub use native::{HistoryArtifacts, HistoryProver};
mod step;
pub use step::{HistoryStepCells, HistoryStepCircuit, HistoryStepInput};

/// Complete fixed policy extracted from the independently authenticated global
/// signed genesis. The exact context commits its original ordered roster/PoPs;
/// the initial lag-two parameters are separately covered by genesis signatures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryAnchor {
    /// Genesis-derived native network identity.
    pub network: [u8; 32],
    /// Global consensus instance derived by the native genesis verifier.
    pub instance: [u8; 32],
    /// Exact canonical initial native epoch-context identity.
    pub initial_context: [u8; 32],
    /// Original initial scheduling epoch number.
    pub initial_epoch: u64,
    /// Initial block/retry/execution/apply times, payload bound and epoch length.
    pub parameters: [u64; 6],
}
impl HistoryAnchor {
    /// Complete policy commitment; computing it does not authenticate genesis.
    pub fn digest(&self) -> Fp {
        let mut words = Vec::with_capacity(13);
        for identity in [self.network, self.instance, self.initial_context] {
            words.extend(identity.chunks_exact(16).map(|bytes| {
                bytes.iter().rev().fold(Fp::ZERO, |value, byte| {
                    value * Fp::from(256) + Fp::from(u64::from(*byte))
                })
            }));
        }
        words.push(Fp::from(self.initial_epoch));
        words.extend(self.parameters.map(Fp::from));
        hash_with_domain(ANCHOR_DOMAIN, &words)
    }
}

/// Circuit-fixed genesis policy. Every value, including its complete digest,
/// is a fixed constant and therefore part of original source-key qualification.
#[derive(Clone, Debug)]
pub struct HistoryAnchorCells {
    digest: Word<Fp>,
    network: [Word<Fp>; 32],
    instance: [Word<Fp>; 32],
    initial_context: [Word<Fp>; 32],
    initial_epoch: Uint<Fp, 64>,
    parameters: [Uint<Fp, 64>; 6],
}
impl HistoryAnchorCells {
    /// Assign the installed fixed policy, never a private witness proposal.
    /// # Errors
    /// Circuit layout errors.
    pub fn assign(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        anchor: &HistoryAnchor,
    ) -> Result<Self, Error> {
        fn identity(
            chip: &mut VerifierChip<Ep>,
            region: &mut Region<'_, Fp>,
            bytes: [u8; 32],
        ) -> Result<[Word<Fp>; 32], Error> {
            bytes
                .into_iter()
                .map(|byte| {
                    chip.uint()
                        .glue()
                        .constant(region, Fp::from(u64::from(byte)))
                })
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| Error::Synthesis)
        }
        let digest = chip.uint().glue().constant(region, anchor.digest())?;
        let network = identity(chip, region, anchor.network)?;
        let instance = identity(chip, region, anchor.instance)?;
        let initial_context = identity(chip, region, anchor.initial_context)?;
        let initial_epoch = chip
            .uint()
            .constant(region, u128::from(anchor.initial_epoch))?;
        let parameters = anchor
            .parameters
            .into_iter()
            .map(|value| chip.uint().constant(region, u128::from(value)))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        Ok(Self {
            digest,
            network,
            instance,
            initial_context,
            initial_epoch,
            parameters,
        })
    }
    /// Complete installed policy digest.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Installed genesis-derived network.
    pub const fn network(&self) -> &[Word<Fp>; 32] {
        &self.network
    }
    /// Installed global consensus instance.
    pub const fn instance(&self) -> &[Word<Fp>; 32] {
        &self.instance
    }
    /// Installed exact initial context.
    pub const fn initial_context(&self) -> &[Word<Fp>; 32] {
        &self.initial_context
    }
    /// Installed initial scheduling epoch.
    pub const fn initial_epoch(&self) -> &Uint<Fp, 64> {
        &self.initial_epoch
    }
    /// Original signed initial lag-two parameters.
    pub const fn parameters(&self) -> &[Uint<Fp, 64>; 6] {
        &self.parameters
    }
}

#[cfg(test)]
mod tests;
