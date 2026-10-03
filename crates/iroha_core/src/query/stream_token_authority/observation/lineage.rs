//! One immutable applied view, one native continuation, and a fixed join of proof targets.

use super::super::historical_execution::PreparedStreamTokenHistoryV1;
use super::*;
use crate::query::signer_check::{
    BorrowedCheckExecutionCutV1, PreparedCheckExecutionV1, SignerCertifiedWalkV1,
};

mod targets;
use targets::{CHECK, FLOOR, RESERVE, TERMINAL, TIP, Targets};

pub(super) fn authenticate<'view, 'state>(
    view: &'view StateView<'state>,
    prepared: &PreparedStreamTokenCheckV1,
    bound: BoundNativeCheckV1,
) -> Result<BorrowedCheckExecutionCutV1<'view, 'state>, Error> {
    crate::query::signer_check::with_native_check_read_limits(|| {
        let mut check = PreparedCheckExecutionV1::new(
            view,
            NativeCustodyCheckPurposeV1::StreamToken,
            bound,
            &prepared.round,
        )?;
        let floor = prepared.expected.floor.height;
        // Check proof failures retain precedence over historical failures. Preparing the bounded
        // history before the walk does not allow an unverified row to hide a failed Check proof.
        let mut history_error = None;
        let mut history = if matches!(prepared.expected.phase, Phase::Current(_)) {
            None
        } else {
            match PreparedStreamTokenHistoryV1::new(
                view,
                prepared.instruction.request.provider_id,
                prepared.expected.reviewed.request.operation_id,
                prepared.expected.floor,
            ) {
                Ok(proof) => Some(proof),
                Err(error) => {
                    history_error = Some(error);
                    None
                }
            }
        };
        let mut targets = Targets::new([
            history
                .as_ref()
                .map(|proof| (proof.start_height(), RESERVE)),
            history
                .as_ref()
                .and_then(|proof| proof.terminal_height())
                .map(|height| (height, TERMINAL)),
            Some((floor, FLOOR)),
            Some((check.check_height(), CHECK)),
            Some((check.applied_height(), TIP)),
        ])?;
        let chain = SignerCertifiedWalkV1::new(view)?;
        for block in chain.walk(targets.start(), targets.end()) {
            prepared.round.ensure_live()?;
            let block = block.map_err(|_| Error::Finality)?;
            targets.consume(block.height())?;
            if block.height() >= floor {
                check.consume(&block)?;
            }
            if block.height() <= floor
                && let Some(proof) = history.as_mut()
                && let Err(error) = proof.consume(&block)
            {
                history_error = Some(error);
                history = None;
            }
        }
        targets.finish()?;
        let check = check.finish()?;
        if history_error.is_some() {
            return Err(Error::Execution);
        }
        if let Some(history) = history {
            let history = history.finish().map_err(|_| Error::Execution)?;
            let claimed = match &prepared.expected.phase {
                Phase::BeforeProvider(row)
                | Phase::AfterProvider(row)
                | Phase::BeforeCommit(row)
                | Phase::AfterCommit(row)
                | Phase::BeforeRelease(row) => row,
                Phase::Current(_) => return Err(Error::Invalid),
            };
            if &history.current().operation != claimed
                || history.reserved().operation.operation.reviewed != prepared.expected.reviewed
            {
                return Err(Error::Execution);
            }
        }
        Ok(check)
    })
}
