#[derive(Debug)]
struct StaticReputationCommittedReaderV1 {
    projection: ReputationCommittedReadProjectionV1,
    retained_snapshots: Vec<ReputationSnapshotV1>,
}

impl ReputationCommittedReadApiV1 for StaticReputationCommittedReaderV1 {
    fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError> {
        Ok(self.projection.clone())
    }

    fn committed_snapshot_by_id(
        &self,
        snapshot_id: [u8; 16],
    ) -> Result<Option<ReputationSnapshotV1>, ReputationRuntimeError> {
        Ok(self
            .retained_snapshots
            .iter()
            .find(|snapshot| snapshot.snapshot_id == snapshot_id)
            .cloned())
    }

    fn committed_events_after(
        &self,
        sequence: u64,
    ) -> Result<Vec<ReputationSnapshotEventV1>, ReputationRuntimeError> {
        Ok(self
            .projection
            .events
            .iter()
            .filter(|event| event.sequence > sequence)
            .cloned()
            .collect())
    }
}

#[derive(Debug)]
struct FailingReputationCommittedReaderV1;

impl ReputationCommittedReadApiV1 for FailingReputationCommittedReaderV1 {
    fn committed_read_projection(
        &self,
    ) -> Result<ReputationCommittedReadProjectionV1, ReputationRuntimeError> {
        Err(ReputationRuntimeError::InvalidRuntimePolicy)
    }

    fn committed_snapshot_by_id(
        &self,
        _snapshot_id: [u8; 16],
    ) -> Result<Option<ReputationSnapshotV1>, ReputationRuntimeError> {
        Err(ReputationRuntimeError::InvalidRuntimePolicy)
    }

    fn committed_events_after(
        &self,
        _sequence: u64,
    ) -> Result<Vec<ReputationSnapshotEventV1>, ReputationRuntimeError> {
        Err(ReputationRuntimeError::InvalidRuntimePolicy)
    }
}

fn attach_reputation_committed_projection(
    app: &mut SharedAppState,
    projection: ReputationCommittedReadProjectionV1,
) {
    let retained_snapshots = projection
        .latest
        .iter()
        .map(|committed| committed.signed_result.snapshot.clone())
        .collect();
    attach_reputation_committed_history(app, projection, retained_snapshots);
}

fn attach_reputation_committed_history(
    app: &mut SharedAppState,
    projection: ReputationCommittedReadProjectionV1,
    retained_snapshots: Vec<ReputationSnapshotV1>,
) {
    Arc::get_mut(app)
        .expect("unique app state")
        .sorafs_reputation_committed_reader = Some(Arc::new(StaticReputationCommittedReaderV1 {
        projection,
        retained_snapshots,
    }));
}
