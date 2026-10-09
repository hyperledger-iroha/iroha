//! Bind complete aggregation and BLS source intervals to one native Commit.
//!
//! These cells only link the source statements. An enclosing circuit must hard
//! verify both source-qualified wrappers and retain their two-curve obligations.
//! The roster must additionally come from the genesis-rooted schedule at this
//! exact height/context; this component alone grants no finality or Load authority.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{GlueChip, Word};
use iroha_plonk_recursion::verifier::VerifierChip;

use super::{
    aggregate::{self, AggregateContext},
    bls::{self, BlsLeafPlan},
    consensus::CommitVoteCells,
    continuity::SourceEndpoints,
};

mod statement;
pub use statement::{CertificateStatement, CertificateStatementCells, STATEMENT_DOMAIN};

/// Untrusted native witness for the exact same roster, quorum and signed message.
#[derive(Clone, Copy, Debug)]
pub struct CertificateContext {
    /// Roster commitment, exact quorum bitmap and compressed aggregate key.
    pub aggregation: AggregateContext,
    /// Exact ordinary Commit signing bytes, including instance, context and R.
    pub message: [u8; CommitVoteCells::BYTES],
    /// Original compressed aggregate signature on those exact bytes.
    pub signature: [u8; 96],
}
impl CertificateContext {
    /// Exact required aggregation/BLS source intervals. This predicts public
    /// inputs only; the enclosing circuit independently binds all twelve cells.
    pub fn source_endpoints(&self) -> [[Fp; 6]; 2] {
        let aggregation = self.aggregation.digest();
        let signature = bls::context_digest_native(
            &self.message,
            &self.aggregation.aggregate_key,
            &self.signature,
        );
        [
            [
                Fp::from(aggregate::PROGRAM_ID),
                aggregation,
                Fp::ZERO,
                Fp::from(u64::from(aggregate::PROGRAM_LENGTH)),
                aggregate::boundary_digest_native(aggregation, false),
                aggregate::boundary_digest_native(aggregation, true),
            ],
            [
                Fp::from(BlsLeafPlan::PROGRAM_ID),
                signature,
                Fp::ZERO,
                Fp::from(u64::from(BlsLeafPlan::LENGTH)),
                bls::boundary_digest_native(signature, false),
                bls::boundary_digest_native(signature, true),
            ],
        ]
    }

    /// Exact public statement exported after verifying both complete programs.
    pub fn statement(&self) -> CertificateStatement {
        CertificateStatement {
            roster_root: self.aggregation.roster_root,
            members: self.aggregation.members,
            faults: self.aggregation.faults,
            message: self.message,
        }
    }
    /// Public statement commitment; the private signature witnesses remain bound
    /// in both source contexts and are verified before this statement is exported.
    pub fn digest(&self) -> Fp {
        self.statement().digest()
    }
}

/// Original assigned byte cells shared by both complete source statements.
#[derive(Clone, Copy)]
pub struct CertificateInputs<'a> {
    /// Internal roster root; the schedule owner must authenticate it.
    pub roster_root: &'a Word<Fp>,
    /// Exact ordered committee size and fault bound.
    pub members: &'a Word<Fp>,
    /// Fault bound of the same committee.
    pub faults: &'a Word<Fp>,
    /// LSB-first bitmap padded to four bytes, constrained by aggregation.
    pub bitmap: &'a [Word<Fp>; 4],
    /// The very same aggregate key in aggregation and BLS source contexts.
    pub aggregate_key: &'a [Word<Fp>; 48],
    /// Exact ordinary Commit vote preimage.
    pub message: &'a [Word<Fp>; CommitVoteCells::BYTES],
    /// Original native aggregate signature.
    pub signature: &'a [Word<Fp>; 96],
}

/// Constrained linkage of complete source intervals. Proof verification and
/// genesis-rooted roster authentication remain duties of the enclosing owner.
#[derive(Clone, Debug)]
pub struct CertificateLinkCells {
    statement: CertificateStatementCells,
    contexts: [Word<Fp>; 2],
}
impl CertificateLinkCells {
    /// Bind every complete-program endpoint to the same native certificate bytes.
    /// Neither a partial BLS run nor a different aggregate key can satisfy this.
    /// The caller must use these exact endpoint cells in both wrapper verifiers.
    /// # Errors
    /// Circuit layout errors; changed endpoints, contexts or bytes are unsatisfiable.
    pub fn constrain(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        aggregation: &SourceEndpoints,
        signature: &SourceEndpoints,
        input: CertificateInputs<'_>,
    ) -> Result<Self, Error> {
        let linked = Self::context(chip, region, input)?;
        let [aggregate_context, signature_context] = &linked.contexts;
        let aggregate_start =
            aggregate::boundary_digest_cells(chip, region, aggregate_context, false)?;
        let aggregate_end =
            aggregate::boundary_digest_cells(chip, region, aggregate_context, true)?;
        let signature_start = bls::boundary_digest_cells(chip, region, signature_context, false)?;
        let signature_end = bls::boundary_digest_cells(chip, region, signature_context, true)?;
        complete(
            region,
            aggregation,
            aggregate::PROGRAM_ID,
            aggregate::PROGRAM_LENGTH,
            aggregate_context,
            &aggregate_start,
            &aggregate_end,
        )?;
        complete(
            region,
            signature,
            BlsLeafPlan::PROGRAM_ID,
            BlsLeafPlan::LENGTH,
            signature_context,
            &signature_start,
            &signature_end,
        )?;
        Ok(linked)
    }

    // Recompute the very same native certificate context from original cells.
    // This does not verify source programs; callers must hard bind the complete
    // certificate source or both complete aggregate/BLS programs independently.
    pub(crate) fn context(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: CertificateInputs<'_>,
    ) -> Result<Self, Error> {
        let statement = CertificateStatementCells::from_cells(
            chip,
            region,
            input.roster_root,
            input.members,
            input.faults,
            input.message,
        )?;
        let aggregate_context = aggregate::context_digest_cells(
            chip,
            region,
            input.roster_root,
            input.members,
            input.faults,
            input.bitmap,
            input.aggregate_key,
        )?;
        let signature_context = bls::context_digest_cells(
            chip,
            region,
            input.message,
            input.aggregate_key,
            input.signature,
        )?;
        let contexts = [aggregate_context, signature_context];
        Ok(Self {
            statement,
            contexts,
        })
    }

    /// Exact shared context to commit in the enclosing source proof.
    pub const fn digest(&self) -> &Word<Fp> {
        self.statement.digest()
    }
    /// Original vote fields to bind to schedule authority and the complete R scan.
    pub const fn vote(&self) -> &CommitVoteCells {
        self.statement.vote()
    }
    /// Ordered key root which the schedule proof must authenticate.
    pub const fn roster_root(&self) -> &Word<Fp> {
        self.statement.roster_root()
    }
    /// Same ordered committee size proved by the aggregation program.
    pub const fn members(&self) -> &Word<Fp> {
        self.statement.members()
    }
    /// Same fault bound proved by the aggregation program.
    pub const fn faults(&self) -> &Word<Fp> {
        self.statement.faults()
    }
}

fn complete(
    region: &mut Region<'_, Fp>,
    source: &SourceEndpoints,
    program: u64,
    length: u32,
    context: &Word<Fp>,
    start: &Word<Fp>,
    end: &Word<Fp>,
) -> Result<(), Error> {
    let words = source.words();
    for (index, expected) in [
        (0, Fp::from(program)),
        (2, Fp::ZERO),
        (3, Fp::from(u64::from(length))),
    ] {
        GlueChip::assert_constant(region, &words[index], expected)?;
    }
    for (index, expected) in [(1, context), (4, start), (5, end)] {
        GlueChip::assert_equal(region, &words[index], expected)?;
    }
    Ok(())
}

mod circuit;
pub use circuit::{CertificateCircuit, PROGRAM_ID};

#[cfg(test)]
mod tests;
