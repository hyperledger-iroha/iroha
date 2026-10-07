//! Terminal ordinary Load receipt binding to complete genesis-rooted history.
//!
//! Both qualified source wrappers are hard verified by the circuit. The selected
//! genesis policy is fixed in its original key. The only exported statement is
//! that anchor together with the exact canonical receipt digest; a later Load
//! owner must pin this source and retain all of its two-curve proof obligations.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{GlueChip, Uint, Word, bytes::p_bytes_native};
use iroha_plonk_recursion::verifier::VerifierChip;

use super::{
    LoadReceiptCells,
    continuity::SourceEndpoints,
    history::{self, HistoryAnchor, HistoryState, HistoryStateCells},
    load_source::{self, LoadSourceContext},
    schedule::tape::ResultTape,
};

mod circuit;
pub use circuit::{ReceiptFinalityCircuit, ReceiptFinalityConfig};

/// Complete ordinary receipt finality source program identity.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwrfnp1");
/// Bind the exact selected genesis policy and original canonical receipt.
pub const CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrfnc1");

// Shared by installed server graphs and verifier-only wallet installations.
// Neither source identity nor these predicted endpoints authenticate a receipt
// until its original proof and both carried obligations verify in full.
pub(in crate::finality) fn verify_receipt_evidence(
    anchor: &HistoryAnchor,
    source: &super::continuity::SourceVerifier,
    vesta: &iroha_plonk::pcs::ipa::PinnedParams<iroha_pasta::Eq>,
    receipt_digest: Fp,
    evidence: &super::continuity::SourceNodeEvidence,
    budget: iroha_pasta::msm::MemoryBudget,
) -> Result<(), super::continuity::producer::Error> {
    use super::continuity::producer::Error as ProofError;
    let context = hash_with_domain(CONTEXT_DOMAIN, &[anchor.digest(), receipt_digest]);
    let expected = [
        Fp::from(PROGRAM_ID),
        context,
        Fp::ZERO,
        Fp::ONE,
        Fp::ZERO,
        context,
    ];
    if evidence.endpoints != expected {
        return Err(ProofError::Input);
    }
    let _opening = source
        .verify_native(evidence, vesta, budget)
        .map_err(|_| ProofError::Proof)?;
    Ok(())
}

/// Untrusted terminal history, inclusion context and original receipt transcript.
#[derive(Clone, Copy, Debug)]
pub struct ReceiptFinalityInput {
    /// Complete state after the receipt's original successful block.
    pub terminal: HistoryState,
    /// Full original receipt/event/count/path context proved by the Load source.
    pub load: LoadSourceContext,
    /// Exact native 282-byte transcript, including the canonical payer digest.
    pub receipt: [u8; LoadReceiptCells::BYTES],
}
impl ReceiptFinalityInput {
    /// Native prediction of the original receipt digest, without validation.
    pub fn receipt_digest(&self) -> Fp {
        p_bytes_native(LoadReceiptCells::DOMAIN, &self.receipt)
    }
    /// Native terminal statement prediction; computing it grants no authority.
    pub fn digest(&self, anchor: &HistoryAnchor) -> Fp {
        hash_with_domain(CONTEXT_DOMAIN, &[anchor.digest(), self.receipt_digest()])
    }
    /// Complete mandatory child endpoints in history/Load-inclusion order.
    pub fn source_endpoints(&self, anchor: &HistoryAnchor, history_key: Fp) -> [[Fp; 6]; 2] {
        let anchor = anchor.digest();
        [
            [
                Fp::from(history::PREFIX_PROGRAM_ID),
                history::prefix_context(anchor, history_key),
                Fp::ZERO,
                Fp::ONE,
                Fp::ZERO,
                self.terminal.digest(anchor),
            ],
            [
                Fp::from(load_source::PROGRAM_ID),
                self.load.digest(),
                Fp::ZERO,
                Fp::from(u64::from(load_source::PROGRAM_LENGTH)),
                self.load.boundary_digest(false),
                self.load.boundary_digest(true),
            ],
        ]
    }
}

/// Derive the terminal identity from the independently selected anchor and exact
/// original receipt digest. The Load owner must hard verify its qualified source.
/// # Errors
/// Circuit layout errors.
pub fn context_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    anchor: &Word<Fp>,
    receipt: &Word<Fp>,
) -> Result<Word<Fp>, Error> {
    chip.hash_words(region, CONTEXT_DOMAIN, &[anchor.clone(), receipt.clone()])
}

/// Original receipt and event location cells from one authenticated result tape.
#[derive(Clone, Copy, Debug)]
pub struct ReceiptInclusionCells<'a> {
    /// Complete original result tape commitment.
    pub tape: &'a ResultTape,
    /// Parsed canonical ordinary Load receipt.
    pub receipt: &'a LoadReceiptCells,
    /// Counted event commitment from the same result.
    pub event_root: &'a [Word<Fp>; 32],
    /// Original counted event tree size.
    pub count: &'a Uint<Fp, 64>,
    /// Exact original event position.
    pub index: &'a Uint<Fp, 32>,
}

/// Exact terminal linkage, which the enclosing circuit authenticates with both
/// hard source verifiers. This type alone is not a native finality capability.
#[derive(Clone, Debug)]
pub struct ReceiptFinalityLinkCells {
    digest: Word<Fp>,
    receipt_digest: Word<Fp>,
}
impl ReceiptFinalityLinkCells {
    /// Require complete genesis history and all 35 Load-source stages for the
    /// same original result, receipt height, receipt terms and event geometry.
    /// # Errors
    /// Layout errors; partial history/source, changed receipt or result, and
    /// genesis-only history fail their constraints.
    pub fn constrain(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        anchor: &Word<Fp>,
        history_key: &Word<Fp>,
        sources: [&SourceEndpoints; 2],
        terminal: &HistoryStateCells,
        inclusion: ReceiptInclusionCells<'_>,
    ) -> Result<Self, Error> {
        let ReceiptInclusionCells {
            tape,
            receipt,
            event_root,
            count,
            index,
        } = inclusion;
        let history = sources[0].words();
        for (i, value) in [
            (0, Fp::from(history::PREFIX_PROGRAM_ID)),
            (2, Fp::ZERO),
            (3, Fp::ONE),
            (4, Fp::ZERO),
        ] {
            GlueChip::assert_constant(region, &history[i], value)?;
        }
        let prefix_context = history::prefix_context_cells(chip, region, anchor, history_key)?;
        GlueChip::assert_equal(region, &history[1], &prefix_context)?;
        GlueChip::assert_equal(region, &history[5], terminal.digest())?;
        let next = chip
            .uint()
            .checked_add_constant(region, receipt.height(), 1)?;
        GlueChip::assert_equal(region, next.word(), terminal.next_height().word())?;
        chip.uint().assert_nonzero(region, terminal.frame_len())?;
        GlueChip::assert_equal(region, terminal.tape_root(), tape.root())?;
        GlueChip::assert_equal(region, terminal.frame_len().word(), tape.frame_len().word())?;

        let context = load_source::context_digest_cells(
            chip, region, tape, receipt, event_root, count, index,
        )?;
        let before = load_source::boundary_digest_cells(chip, region, &context, false)?;
        let after = load_source::boundary_digest_cells(chip, region, &context, true)?;
        let inclusion = sources[1].words();
        for (i, value) in [
            (0, Fp::from(load_source::PROGRAM_ID)),
            (2, Fp::ZERO),
            (3, Fp::from(u64::from(load_source::PROGRAM_LENGTH))),
        ] {
            GlueChip::assert_constant(region, &inclusion[i], value)?;
        }
        for (i, expected) in [(1, &context), (4, &before), (5, &after)] {
            GlueChip::assert_equal(region, &inclusion[i], expected)?;
        }
        Ok(Self {
            digest: context_digest_cells(chip, region, anchor, receipt.digest())?,
            receipt_digest: receipt.digest().clone(),
        })
    }
    /// Exact terminal context which the later Load relation must authenticate.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Digest derived from the same original parsed and source-bound receipt.
    pub const fn receipt_digest(&self) -> &Word<Fp> {
        &self.receipt_digest
    }
}

#[cfg(test)]
mod tests;
