//! O(1) source-authenticated scheduling status. Never scans a suffix or verifies a proof.
use super::prepare::address;
use super::*;
use crate::kagemusha_wallet_state_v1::suffix_custody;
use std::ops::ControlFlow;

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    pub(super) fn suffix_request(
        &mut self,
        selected: &SelectedCheckpoint,
        id: &[u8; 32],
    ) -> Result<Option<SuffixRequestRecordV1>, Error> {
        if *id == [0; 32] {
            return Err(Error::Invalid("suffix request identity"));
        }
        let Some(namespace) = &selected.suffix else {
            return Ok(None);
        };
        namespace
            .requests
            .get(&mut self.archive, id)?
            .map(|value| {
                let record: SuffixRequestRecordV1 =
                    read_record(&mut self.archive, address(&value)?)?;
                record.require((self.scheme_id, self.wallet_id))?;
                if record.request_id != *id {
                    return Err(Error::WitnessLost("suffix request index"));
                }
                Ok(record)
            })
            .transpose()
    }
    /// Only an actual authenticated index miss can construct named-resume absence DATA.
    pub(super) fn suffix_resume_observation(
        &mut self,
        selected: &SelectedCheckpoint,
        request_id: &[u8; 32],
    ) -> Result<ControlFlow<SuffixUnloadStatusV1, SuffixRequestRecordV1>, Error> {
        if let Some(record) = self.suffix_request(selected, request_id)? {
            return Ok(ControlFlow::Continue(record));
        }
        let absent = SuffixUnloadStatusV1 {
            format_version: 1,
            discriminator: 15,
            state: 7,
            reason: 0,
            current_sequence: Some(
                selected
                    .manifest
                    .indexed
                    .ok_or(Error::WitnessLost("suffix absent head"))?,
            ),
            folded_sequence: selected.manifest.folded,
            request_id: Some(*request_id),
            completed_steps: 0,
            total_steps: 0,
        };
        read_model_original(
            || absent.validate(),
            Error::WitnessLost("suffix absent geometry"),
        )?;
        Ok(ControlFlow::Break(absent))
    }

    /// Read the selected fixed metadata and at most one request/progress/selection record.
    /// A Ready status is display DATA; resume must reconstruct a genuine live review.
    /// This global observation never emits the named-request-only `RequestAbsent` state.
    /// # Errors
    /// Unavailable custody, malformed selected records or inconsistent scheduling metadata.
    pub fn suffix_unload_status(&mut self) -> Result<SuffixUnloadStatusV1, Error> {
        let custody = self.status()?;
        let mut result = SuffixUnloadStatusV1 {
            format_version: 1,
            discriminator: 15,
            state: 0,
            reason: 0,
            current_sequence: None,
            folded_sequence: None,
            request_id: None,
            completed_steps: 0,
            total_steps: 0,
        };
        if !matches!(custody, SlotStatus::Released(_) | SlotStatus::Pending(_)) {
            return Ok(result);
        }
        let selected = self
            .selected_suffix_checkpoint()?
            .ok_or(Error::WitnessLost("suffix status checkpoint"))?;
        result.current_sequence = selected.manifest.indexed;
        result.folded_sequence = selected.manifest.folded;
        let request = selected
            .suffix
            .as_ref()
            .and_then(|suffix| suffix.active_request);
        if let Some(request_id) = request {
            let suffix = selected
                .suffix
                .as_ref()
                .ok_or(Error::WitnessLost("suffix status namespace"))?;
            let address = suffix
                .requests
                .get(&mut self.archive, &request_id)?
                .ok_or(Error::WitnessLost("suffix status active request"))?;
            let address: [u8; 32] = address
                .try_into()
                .map_err(|_| Error::WitnessLost("suffix status request address"))?;
            let request: SuffixRequestRecordV1 =
                suffix_custody::read_record(&mut self.archive, address)?;
            request.require((self.scheme_id, self.wallet_id))?;
            if request.request_id != request_id {
                return Err(Error::WitnessLost("suffix status request binding"));
            }
            result.request_id = Some(request_id);
            result.total_steps = request
                .source_sequence
                .checked_sub(request.anchor_sequence)
                .ok_or(Error::WitnessLost("suffix status source range"))?;
            result.state = 3;
            if let Some(address) = request.progress {
                let progress: SuffixPreparationProgressV1 =
                    suffix_custody::read_record(&mut self.archive, address)?;
                progress.require((self.scheme_id, self.wallet_id))?;
                if progress.request_id != request_id
                    || progress.source_checkpoint != request.source_checkpoint
                    || progress.pinned_steps > result.total_steps
                {
                    return Err(Error::WitnessLost("suffix status progress binding"));
                }
                result.completed_steps = progress.processed_steps;
            }
            if let Some(address) = request.selection {
                let selection: SuffixPublicationSelectionV1 =
                    suffix_custody::read_record(&mut self.archive, address)?;
                selection.require((self.scheme_id, self.wallet_id))?;
                if selection.request_id != request_id
                    || selection.source_sequence != request.source_sequence
                    || selection.source_capsule != request.source_capsule
                    || selection.step_count != result.total_steps
                {
                    return Err(Error::WitnessLost("suffix status selection binding"));
                }
                result.completed_steps = result.total_steps;
                result.state = if request.released.is_some() { 6 } else { 4 };
                if let Some(released_address) = request.released {
                    let released: SuffixReleasedOriginalV1 =
                        suffix_custody::read_record(&mut self.archive, released_address)?;
                    released.require((self.scheme_id, self.wallet_id))?;
                    if released.request_id != request_id
                        || released.selection != address
                        || released.authority != selection.authority
                        || released.operation_id != selection.operation_id
                        || released.instruction_count != selection.chunk_count + 2
                    {
                        return Err(Error::WitnessLost("suffix status released binding"));
                    }
                }
                if request.released.is_none() {
                    if let SlotStatus::Pending(marker) | SlotStatus::Released(marker) = custody {
                        if marker.head().is_some_and(|(sequence, operation, capsule)| {
                            sequence == selection.source_sequence.saturating_add(1)
                                && operation == selection.operation_id
                                && capsule != [0; 32]
                        }) {
                            // Hardware selection is final even while the post-Advance verifier
                            // has not yet published its exact released carriage originals.
                            result.state = 5;
                        }
                    }
                }
            }
        } else if selected.manifest.folded.is_none() {
            result.state = 1;
            result.reason = 1;
        } else if selected.manifest.folded == selected.manifest.indexed {
            result.state = 1;
            result.reason = 3;
        } else {
            result.state = 2;
        }
        read_model_original(
            || result.validate(),
            Error::WitnessLost("suffix status invariant"),
        )?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
