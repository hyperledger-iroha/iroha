//! Production facade over the neutral ordinary Commit verification/original-custody kernel.
//! Cold Native replay uses that kernel directly without production proving features.
use super::*;
pub(crate) use crate::kagemusha_v1_recursion::ordinary_cash_commit_originals::GeneratedOrdinaryCashCommitOriginalsV1;
use crate::kagemusha_v1_recursion::ordinary_cash_commit_originals::assemble_ordinary_cash_commit_v1;
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1;
impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Assemble retained proof originals and independently verify them; no key generation.
    pub(crate) fn assemble_ordinary_cash_commit(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
        terminal_guard: &KagemushaAuthenticatedOrdinaryTerminalGuardV1,
        whole: &KagemushaAuthenticatedOrdinaryCashTerminalV1,
        reservation: &KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1<'_>,
    ) -> Result<GeneratedOrdinaryCashCommitOriginalsV1, KagemushaArtifactGenerationErrorV1> {
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        assemble_ordinary_cash_commit_v1(
            &self.verifier,
            selection,
            terminal_guard,
            whole,
            reservation,
        )
    }
}
