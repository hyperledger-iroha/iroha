//! Compact original Commit statement exported after complete certificate verification.
//!
//! Assigning these cells proves their encoding only. Authority requires the
//! source-qualified certificate proof and the genesis-rooted roster schedule.

use super::*;
use iroha_plonk::frontend::Value;

/// Domain for the certified roster geometry and original fixed-length Commit.
pub const STATEMENT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwqcst1");

/// Public certificate statement, independent of its private signer bitmap,
/// aggregate key and signature. The complete certificate source authenticates
/// those witnesses before exporting this exact statement.
#[derive(Clone, Copy, Debug)]
pub struct CertificateStatement {
    /// Ordered normal-validator key root, to authenticate from signed genesis.
    pub roster_root: Fp,
    /// Exact normal committee size.
    pub members: u8,
    /// Exact committee fault bound.
    pub faults: u8,
    /// Original ordinary Commit bytes, including instance, context, height and R.
    pub message: [u8; CommitVoteCells::BYTES],
}
impl CertificateStatement {
    /// Canonical statement commitment; it does not verify a certificate.
    pub fn digest(&self) -> Fp {
        let mut words = vec![
            self.roster_root,
            Fp::from(u64::from(self.members)),
            Fp::from(u64::from(self.faults)),
        ];
        words.extend(self.message.chunks(16).map(|chunk| {
            chunk
                .iter()
                .rev()
                .fold(Fp::ZERO, |n, b| n * Fp::from(256) + Fp::from(u64::from(*b)))
        }));
        hash_with_domain(STATEMENT_DOMAIN, &words)
    }
}

/// Constrained byte-level statement, still untrusted without its source proof.
#[derive(Clone, Debug)]
pub struct CertificateStatementCells {
    digest: Word<Fp>,
    vote: CommitVoteCells,
    roster_root: Word<Fp>,
    members: Word<Fp>,
    faults: Word<Fp>,
}
impl CertificateStatementCells {
    /// Assign an untrusted statement and independently constrain every exported field.
    /// # Errors
    /// Layout errors; wrong Commit domain, height, byte range or quorum geometry fail.
    pub fn assign(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Value<CertificateStatement>,
    ) -> Result<Self, Error> {
        let root = chip
            .uint()
            .glue()
            .witness(region, value.map(|v| v.roster_root))?;
        let members = chip
            .uint()
            .glue()
            .witness(region, value.map(|v| Fp::from(u64::from(v.members))))?;
        let faults = chip
            .uint()
            .glue()
            .witness(region, value.map(|v| Fp::from(u64::from(v.faults))))?;
        let message = (0..CommitVoteCells::BYTES)
            .map(|i| {
                chip.uint()
                    .glue()
                    .witness(region, value.map(|v| Fp::from(u64::from(v.message[i]))))
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        Self::from_cells(chip, region, &root, &members, &faults, &message)
    }

    pub(super) fn from_cells(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        roster_root: &Word<Fp>,
        members: &Word<Fp>,
        faults: &Word<Fp>,
        message: &[Word<Fp>; CommitVoteCells::BYTES],
    ) -> Result<Self, Error> {
        let vote = CommitVoteCells::from_bytes(&mut chip.uint(), region, message)?;
        let n = chip.uint().range_check::<5>(region, members)?;
        let f = chip.uint().range_check::<4>(region, faults)?;
        let one = chip.uint().constant::<4>(region, 1)?;
        let ten = chip.uint().constant::<4>(region, 10)?;
        chip.uint().assert_le(region, &one, &f)?;
        chip.uint().assert_le(region, &f, &ten)?;
        let expected = chip
            .uint()
            .glue()
            .linear(region, &[(Fp::from(3), f.word())], Fp::ONE)?;
        GlueChip::assert_equal(region, n.word(), &expected)?;
        let mut words = vec![roster_root.clone(), members.clone(), faults.clone()];
        for chunk in message.chunks(16) {
            let mut packed = chip.uint().glue().constant(region, Fp::ZERO)?;
            for byte in chunk.iter().rev() {
                packed = chip.uint().glue().linear(
                    region,
                    &[(Fp::from(256), &packed), (Fp::ONE, byte)],
                    Fp::ZERO,
                )?;
            }
            words.push(packed);
        }
        let digest = chip.hash_words(region, STATEMENT_DOMAIN, &words)?;
        Ok(Self {
            digest,
            vote,
            roster_root: roster_root.clone(),
            members: members.clone(),
            faults: faults.clone(),
        })
    }
    /// Commitment which must equal the complete certificate source's output.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Exact original signed fields, with native Commit domain and geometry.
    pub const fn vote(&self) -> &CommitVoteCells {
        &self.vote
    }
    /// Exact roster to bind to the authenticated native context.
    pub const fn roster_root(&self) -> &Word<Fp> {
        &self.roster_root
    }
    /// Exact normal committee size.
    pub const fn members(&self) -> &Word<Fp> {
        &self.members
    }
    /// Exact normal committee fault bound.
    pub const fn faults(&self) -> &Word<Fp> {
        &self.faults
    }
}
