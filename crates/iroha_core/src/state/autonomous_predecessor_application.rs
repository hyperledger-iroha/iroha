impl State {
    /// Select only certificates eligible for ordinary canonical-body receipt
    /// reconstruction. READY identifies autonomous economic execution, whose
    /// receipt must instead come from its exact globally finalized merge carrier.
    fn ordinary_application_receipt_repair_session(
        artifact: crate::kura::CertifiedLaneBlockArtifact,
    ) -> Option<crate::lane_consensus::CommittedLaneBlockSession> {
        let session = crate::lane_consensus::CommittedLaneBlockSession {
            proposal: artifact.proposal,
            prepare_qc: artifact.prepare_qc,
            commit_qc: artifact.commit_qc,
        };
        session
            .prepare_qc
            .payload_availability_qc
            .is_none()
            .then_some(session)
    }
}
