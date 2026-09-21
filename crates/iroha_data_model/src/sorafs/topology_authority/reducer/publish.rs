//! Immutable transition publication plan; the native State owner supplies atomic persistence.
use super::*;

impl<L: TopologyIndexedReadV1 + ?Sized> TopologyStateViewV1<'_, L> {
    pub(super) fn prepare_publication(
        &self,
        transition: &TopologyTransitionV1,
        context: &TopologyContextClaimV1,
        control: Option<control::PreparedControl>,
        operation: Option<TopologyOperationRecordV1>,
    ) -> Result<TopologyPreparedTransitionV1, TopologyPreparationErrorV1<L::Error>> {
        let control_head = control
            .as_ref()
            .map(|next| {
                Ok::<_, Error>(TopologyHeadV1 {
                    revision: next.record.revision,
                    digest: digest(b"iroha.sorafs.topology.control.v1\0", &next.record)?,
                })
            })
            .transpose()?
            .unwrap_or(self.root.control_head);
        let operation_head = operation
            .as_ref()
            .map(|row| {
                encode(row, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
                Ok::<_, Error>(TopologyHeadV1 {
                    revision: row.revision,
                    digest: digest(b"iroha.sorafs.topology.operation.v1\0", row)?,
                })
            })
            .transpose()?
            .unwrap_or(self.root.operation_head);
        let revision = self
            .root
            .history_head
            .revision
            .checked_add(1)
            .ok_or(Error::Capacity)?;
        if revision > TOPOLOGY_HISTORY_LIMIT_V1 {
            return Err(Error::Capacity.into());
        }
        let entry = TopologyHistoryEntryV1 {
            revision,
            predecessor_digest: self.root.history_head.digest,
            transition: transition.clone(),
            context: context.clone(),
            control: control_head,
            operations: operation_head,
        };
        encode(&entry, TOPOLOGY_HISTORY_MAX_BYTES_V1)?;
        let history_head = TopologyHeadV1 {
            revision,
            digest: digest(b"iroha.sorafs.topology.history.v1\0", &entry)?,
        };
        let signer_key = control.as_ref().and_then(|next| next.signer_key);
        let attester_key = control.as_ref().and_then(|next| next.attester_key);
        let mut next = (*self.root).clone();
        if signer_key.is_some() {
            next.signer_key_count = next
                .signer_key_count
                .checked_add(1)
                .ok_or(Error::Capacity)?;
        }
        if attester_key.is_some() {
            next.attester_key_count = next
                .attester_key_count
                .checked_add(1)
                .ok_or(Error::Capacity)?;
        }
        if let Some(row) = &operation {
            match row.outcome {
                TopologyOutcomeV1::Reserved => {
                    next.operation_count =
                        next.operation_count.checked_add(1).ok_or(Error::Capacity)?;
                    next.fence = row.reservation.fence;
                    next.active = Some(row.reviewed.request.operation_id);
                }
                TopologyOutcomeV1::Completed { commitment, .. } => {
                    next.audit = commitment.audit;
                    next.active = None;
                }
                TopologyOutcomeV1::Expired | TopologyOutcomeV1::Invalidated => next.active = None,
            }
        }
        next.control_head = control_head;
        next.operation_head = operation_head;
        next.history_head = history_head;
        next.last_execution = Some(context.execution.clone());
        encode(&next, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
        let (control, control_state) =
            control.map_or((None, None), |next| (Some(next.record), Some(next.state)));
        Ok(TopologyPreparedTransitionV1 {
            delta: TopologyTransitionDeltaV1 {
                next: Some(next),
                history: Some(entry),
                control,
                operation,
                signer_key,
                attester_key,
            },
            control_state,
        })
    }
}
