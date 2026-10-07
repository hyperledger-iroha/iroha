//! Link a complete ordinary quorum certificate to the entire original result.
//!
//! The recursive owner must hard verify both source-qualified children and
//! retain both curves' obligations. The roster is still untrusted until the
//! schedule/history owner joins this statement to the selected signed genesis.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{GlueChip, Uint, Word};
use iroha_plonk_recursion::verifier::VerifierChip;

use super::{
    certificate::{self, CertificateStatement, CertificateStatementCells},
    continuity::SourceEndpoints,
    result_scan::{self, ResultScanContext},
};

/// Exact source program for a certificate and its complete result preimage.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwqcrp1");
/// Bind the original native certificate and the exact result tape/length.
pub const CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwqcrc1");

/// Untrusted original certificate and result-tape proposal.
#[derive(Clone, Copy, Debug)]
pub struct CertifiedResultContext {
    /// Native certificate whose signed R must equal the complete scan output.
    pub certificate: CertificateStatement,
    /// Internal commitment to the exact complete canonical result bytes.
    pub root: Fp,
    /// Original result length, excluding its native hash domain.
    pub frame_len: u32,
}
impl CertifiedResultContext {
    /// Expected scan context, with R taken from the original Commit message.
    pub fn scan(&self) -> ResultScanContext {
        ResultScanContext {
            root: self.root,
            frame_len: self.frame_len,
            expected: core::array::from_fn(|i| self.certificate.message[133 + i]),
        }
    }
    /// Native context prediction; computing this does not authenticate evidence.
    pub fn digest(&self) -> Fp {
        hash_with_domain(
            CONTEXT_DOMAIN,
            &[
                self.certificate.digest(),
                self.root,
                Fp::from(u64::from(self.frame_len)),
            ],
        )
    }
    /// Full source intervals required by the circuit, in certificate/scan order.
    pub fn source_endpoints(&self) -> [[Fp; 6]; 2] {
        let certificate = self.certificate.digest();
        let scan = self.scan();
        [
            [
                Fp::from(certificate::PROGRAM_ID),
                certificate,
                Fp::ZERO,
                Fp::ONE,
                Fp::ZERO,
                certificate,
            ],
            [
                Fp::from(result_scan::RESULT_SCAN_PROGRAM),
                scan.digest(),
                Fp::ZERO,
                Fp::from(u64::from(result_scan::RESULT_SCAN_LEAVES)),
                scan.boundary_digest(false),
                scan.boundary_digest(true),
            ],
        ]
    }
}

/// Commit the exact original certificate statement and complete result tape.
/// This only computes their linked identity; a qualified source proof must
/// authenticate the complete certificate and result-scan intervals separately.
/// # Errors
/// Circuit layout errors.
pub fn context_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    certificate: &Word<Fp>,
    root: &Word<Fp>,
    frame_len: &Uint<Fp, 32>,
) -> Result<Word<Fp>, Error> {
    chip.hash_words(
        region,
        CONTEXT_DOMAIN,
        &[certificate.clone(), root.clone(), frame_len.word().clone()],
    )
}

/// Exact statement linkage; no proof or genesis authority is implied by this type.
#[derive(Clone, Debug)]
pub struct CertifiedResultLinkCells {
    digest: Word<Fp>,
    certificate: CertificateStatementCells,
    root: Word<Fp>,
    frame_len: Uint<Fp, 32>,
}
impl CertifiedResultLinkCells {
    /// Bind both complete source intervals to the exact original signed R bytes.
    /// The enclosing owner must hard verify wrappers using these endpoint cells.
    /// # Errors
    /// Layout errors; partial scans, context substitutions and changed boundaries fail.
    pub fn constrain(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        certificate: &SourceEndpoints,
        scan: &SourceEndpoints,
        input: &CertificateStatementCells,
        root: &Word<Fp>,
        frame_len: &Uint<Fp, 32>,
    ) -> Result<Self, Error> {
        let linked = input;
        let words = certificate.words();
        for (index, expected) in [
            (0, Fp::from(certificate::PROGRAM_ID)),
            (2, Fp::ZERO),
            (3, Fp::ONE),
            (4, Fp::ZERO),
        ] {
            GlueChip::assert_constant(region, &words[index], expected)?;
        }
        for index in [1, 5] {
            GlueChip::assert_equal(region, &words[index], linked.digest())?;
        }
        let expected = linked
            .vote()
            .result()
            .to_vec()
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let context = result_scan::context_digest_cells(chip, region, root, frame_len, &expected)?;
        let zero = chip.uint().constant::<32>(region, 0)?;
        let end = result_scan::end_cursor_cells(chip, region, frame_len)?;
        let before = result_scan::boundary_digest_cells(chip, region, &context, &zero, false)?;
        let after = result_scan::boundary_digest_cells(chip, region, &context, &end, true)?;
        let words = scan.words();
        GlueChip::assert_constant(
            region,
            &words[0],
            Fp::from(result_scan::RESULT_SCAN_PROGRAM),
        )?;
        GlueChip::assert_constant(region, &words[2], Fp::ZERO)?;
        for (index, expected) in [(1, &context), (3, end.word()), (4, &before), (5, &after)] {
            GlueChip::assert_equal(region, &words[index], expected)?;
        }
        let digest = context_digest_cells(chip, region, linked.digest(), root, frame_len)?;
        Ok(Self {
            digest,
            certificate: linked.clone(),
            root: root.clone(),
            frame_len: frame_len.clone(),
        })
    }
    /// Exact context committed by the composed source proof.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Original certificate fields to join to the genesis-rooted schedule.
    pub const fn certificate(&self) -> &CertificateStatementCells {
        &self.certificate
    }
    /// Same complete byte-tape root consumed by every result parser.
    pub const fn root(&self) -> &Word<Fp> {
        &self.root
    }
    /// Same exact original frame length consumed by every result parser.
    pub const fn frame_len(&self) -> &Uint<Fp, 32> {
        &self.frame_len
    }
}

mod circuit;
pub use circuit::CertifiedResultCircuit;

#[cfg(test)]
mod tests;
