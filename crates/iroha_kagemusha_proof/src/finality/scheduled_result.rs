//! Bind a complete certified result to its exact current native schedule.
//!
//! The two source proofs authenticate the same original result tape. Its parsed
//! current context supplies precisely the roster which signed its Commit. This
//! does not independently authenticate that roster: the history owner must join
//! the exported native context to the selected signed genesis and its promised
//! successor slots before this result can authorize a Load.

use ff::Field;
use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{GlueChip, Uint, Word};
use iroha_plonk_recursion::verifier::VerifierChip;

use super::{
    certificate::CertificateStatementCells,
    certified_result::{self, CertifiedResultContext},
    continuity::SourceEndpoints,
    schedule::{
        complete,
        source::{ScheduleSourceBinding, ScheduleSourceInput},
    },
};

mod statement;
pub use statement::{ScheduledResultStatement, ScheduledResultStatementCells};
mod circuit;
pub use circuit::ScheduledResultCircuit;

/// Complete certified-result/current-schedule source program identity.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwsrsp1");
/// Domain committing the compact scheduled-result statement.
pub const CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwsrsc1");

/// Untrusted proposals for the two original complete source proofs.
#[derive(Clone, Copy, Debug)]
pub struct ScheduledResultInput {
    /// Exact original Commit statement and complete result tape.
    pub certified: CertifiedResultContext,
    /// Exact current-context parser/hash input from that same result.
    pub schedule: ScheduleSourceInput,
}
impl ScheduledResultInput {
    /// Compact native prediction; this does not verify either source or history.
    pub fn statement(&self) -> ScheduledResultStatement {
        let message = &self.certified.certificate.message;
        ScheduledResultStatement {
            network: self.schedule.projection.network,
            instance: core::array::from_fn(|i| message[13 + i]),
            epoch: u64::from_be_bytes(core::array::from_fn(|i| message[45 + i])),
            height: u64::from_be_bytes(core::array::from_fn(|i| message[85 + i])),
            context: core::array::from_fn(|i| message[53 + i]),
            result: core::array::from_fn(|i| message[133 + i]),
            tape_root: self.certified.root,
            frame_len: self.certified.frame_len,
        }
    }
    /// Exact complete child endpoints in certified-result/current-schedule order.
    /// Computing these openings grants no certificate or schedule authority.
    pub fn source_endpoints(&self) -> [[Fp; 6]; 2] {
        [
            singleton_native(certified_result::PROGRAM_ID, self.certified.digest()),
            singleton_native(complete::PROGRAM_ID, self.schedule.digest()),
        ]
    }
}
fn singleton_native(program: u64, digest: Fp) -> [Fp; 6] {
    [
        Fp::from(program),
        digest,
        Fp::ZERO,
        Fp::ONE,
        Fp::ZERO,
        digest,
    ]
}
fn require_singleton(
    region: &mut Region<'_, Fp>,
    endpoint: &SourceEndpoints,
    program: u64,
    digest: &Word<Fp>,
) -> Result<(), Error> {
    let words = endpoint.words();
    for (i, value) in [
        (0, Fp::from(program)),
        (2, Fp::ZERO),
        (3, Fp::ONE),
        (4, Fp::ZERO),
    ] {
        GlueChip::assert_constant(region, &words[i], value)?;
    }
    for i in [1, 5] {
        GlueChip::assert_equal(region, &words[i], digest)?;
    }
    Ok(())
}

/// Exact linkage between a certified result and its original current context.
/// The enclosing owner must hard verify both wrappers using these same cells.
#[derive(Clone, Debug)]
pub struct ScheduledResultLinkCells {
    statement: ScheduledResultStatementCells,
}
impl ScheduledResultLinkCells {
    /// Require both complete programs, the same tape and exact current committee.
    /// No host-decoded fact replaces an authenticated source commitment.
    /// # Errors
    /// Layout errors; wrong role, tape, committee, epoch, height or bounds fail.
    pub fn constrain(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        sources: [&SourceEndpoints; 2],
        certificate: &CertificateStatementCells,
        root: &Word<Fp>,
        frame_len: &Uint<Fp, 32>,
        schedule: &ScheduleSourceBinding,
    ) -> Result<Self, Error> {
        let certified = certified_result::context_digest_cells(
            chip,
            region,
            certificate.digest(),
            root,
            frame_len,
        )?;
        require_singleton(region, sources[0], certified_result::PROGRAM_ID, &certified)?;
        require_singleton(region, sources[1], complete::PROGRAM_ID, schedule.digest())?;
        GlueChip::assert_constant(region, schedule.authorized().word(), Fp::ZERO)?;
        GlueChip::assert_equal(region, root, schedule.tape().root())?;
        GlueChip::assert_equal(region, frame_len.word(), schedule.tape().frame_len().word())?;
        for (actual, expected) in [
            (certificate.roster_root(), schedule.roster_root()),
            (certificate.members(), schedule.members()),
            (certificate.faults(), schedule.faults()),
            (certificate.vote().epoch().word(), schedule.epoch()),
            (certificate.vote().height().word(), schedule.height().word()),
        ] {
            GlueChip::assert_equal(region, actual, expected)?;
        }
        for (actual, expected) in certificate
            .vote()
            .context()
            .iter()
            .zip(schedule.context_id())
        {
            GlueChip::assert_equal(region, actual, expected)?;
        }
        let first = chip
            .uint()
            .range_check::<64>(region, schedule.first_height())?;
        let last = chip
            .uint()
            .range_check::<64>(region, schedule.last_height())?;
        chip.uint()
            .assert_le(region, &first, certificate.vote().height())?;
        chip.uint()
            .assert_le(region, certificate.vote().height(), &last)?;
        let instance = certificate
            .vote()
            .instance()
            .to_vec()
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let context = certificate
            .vote()
            .context()
            .to_vec()
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let result = certificate
            .vote()
            .result()
            .to_vec()
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let statement = ScheduledResultStatementCells::from_cells(
            chip,
            region,
            statement::StatementInputs {
                network: schedule.network(),
                instance: &instance,
                epoch: certificate.vote().epoch(),
                height: certificate.vote().height(),
                context: &context,
                result: &result,
                tape_root: root,
                frame_len,
            },
        )?;
        Ok(Self { statement })
    }
    /// Compact output digest, computed from the exact linked original cells.
    pub const fn digest(&self) -> &Word<Fp> {
        self.statement.digest()
    }
    /// Original native identities to bind to the genesis-rooted history owner.
    pub const fn statement(&self) -> &ScheduledResultStatementCells {
        &self.statement
    }
}

#[cfg(test)]
mod tests;
