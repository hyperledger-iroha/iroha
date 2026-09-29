//! Original role-11 operation relation consumed by standalone and joined certified walks.

use super::*;

/// Bounded original rows and exact source view, with no independent authority constructor.
pub(in crate::query::stream_token_authority) struct PreparedStreamTokenHistoryV1<'view, 'state> {
    view: &'view StateView<'state>,
    history: OperationHistoryV1,
    floor: StreamTokenFinalityFloorV1,
    start: u64,
    terminal_height: Option<u64>,
    next_height: Option<u64>,
    finality_bytes: usize,
    reserved_seen: bool,
    terminal_seen: bool,
    target_error: Option<Error>,
    failed: bool,
}
impl<'view, 'state> PreparedStreamTokenHistoryV1<'view, 'state> {
    pub(in crate::query::stream_token_authority) fn new(
        view: &'view StateView<'state>,
        provider: ProviderId,
        operation_id: [u8; 32],
        floor: StreamTokenFinalityFloorV1,
    ) -> Result<Self, Error> {
        if floor.height == 0
            || floor.block_hash == [0; 32]
            || *floor.context_id.0.as_ref() == [0; 32]
        {
            return Err(Error::Finality);
        }
        let history = read_history(&view.world, provider, operation_id)?.ok_or(Error::Conflict)?;
        let start = history.reserved.operation.reserved_execution.height;
        let terminal_height = if history.current == history.reserved {
            None
        } else {
            Some(
                history
                    .current
                    .operation
                    .terminal_execution
                    .as_ref()
                    .ok_or(Error::CorruptHistory)?
                    .height,
            )
        };
        if start == 0
            || terminal_height.is_some_and(|height| height <= start || height > floor.height)
        {
            return Err(Error::CorruptHistory);
        }
        bound_history_span(start, floor.height)?;
        if floor.height > u64::try_from(view.block_hashes().len()).map_err(|_| Error::Finality)? {
            return Err(Error::Finality);
        }
        Ok(Self {
            view,
            history,
            floor,
            start,
            terminal_height,
            next_height: Some(start),
            finality_bytes: 0,
            reserved_seen: false,
            terminal_seen: false,
            target_error: None,
            failed: false,
        })
    }
    pub(in crate::query::stream_token_authority) const fn start_height(&self) -> u64 {
        self.start
    }
    pub(in crate::query::stream_token_authority) const fn terminal_height(&self) -> Option<u64> {
        self.terminal_height
    }

    /// Each certified block is charged once in the original history window, including targets.
    pub(in crate::query::stream_token_authority) fn consume(
        &mut self,
        block: &SignerCertifiedBlockV1<'_, '_>,
    ) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Execution);
        }
        self.failed = true;
        let block = block.in_view(self.view).map_err(|_| Error::Finality)?;
        let height = block.height();
        let offset = height
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(Error::Finality)?;
        if self.next_height != Some(height)
            || self.view.block_hashes().get(offset).copied() != Some(block.block_hash())
        {
            return Err(Error::Finality);
        }
        charge_finality_len(&mut self.finality_bytes, block.certificate_len())?;
        if height == self.floor.height
            && (*block.block_hash().as_ref() != self.floor.block_hash
                || block.id() != self.floor.context_id)
        {
            return Err(Error::Finality);
        }
        if height == self.start {
            self.target_error = authenticate_target(
                self.view,
                &self.history.reserved,
                TargetKind::Reserved,
                block,
            )
            .err();
            self.reserved_seen = true;
        }
        if self.terminal_height == Some(height) {
            let error = authenticate_target(
                self.view,
                &self.history.current,
                TargetKind::Terminal,
                block,
            )
            .err();
            self.target_error = self.target_error.or(error);
            self.terminal_seen = true;
        }
        self.next_height = if height == self.floor.height {
            None
        } else {
            height.checked_add(1)
        };
        self.failed = false;
        Ok(())
    }
    pub(in crate::query::stream_token_authority) fn finish(
        self,
    ) -> Result<VerifiedStreamTokenHistoryV1<'view, 'state>, Error> {
        if self.failed
            || self.next_height.is_some()
            || !self.reserved_seen
            || (self.terminal_height.is_some() && !self.terminal_seen)
        {
            return Err(Error::Finality);
        }
        if let Some(error) = self.target_error {
            return Err(error);
        }
        Ok(VerifiedStreamTokenHistoryV1 {
            view: self.view,
            history: self.history,
            floor: self.floor,
        })
    }
}
