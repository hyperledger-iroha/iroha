//! Reuse the certificate owners on a bounded worker without persisting restricted artifacts.

use super::*;

pub(super) trait SettlementPhases: Send + Sync {
    fn run(
        &self,
        phase: usize,
        journal: WorkflowJournalV1,
        read_only: bool,
    ) -> WorkflowFuture<'_, std::result::Result<Option<Vec<u8>>, PrivateSettlementError>>;
}

pub(super) struct OnlinePhases {
    client: Client,
    request: PrivateSettlementRequest,
    worker: Arc<tokio::sync::Semaphore>,
    storage: Arc<dyn WorkflowStorage>,
    lease_id: WorkflowId,
}
impl OnlinePhases {
    pub fn new(
        client: Client,
        request: PrivateSettlementRequest,
        worker: Arc<tokio::sync::Semaphore>,
        storage: Arc<dyn WorkflowStorage>,
    ) -> Self {
        // Separate from the workflow engine's own runner lease. This exact Global
        // request keeps the same phase custody across reconstructed operations and
        // endpoint updates, without retaining any completed-payment history.
        let lease_id = WorkflowId::from_bytes(domain_digest(
            b"iroha:private-settlement:global-phase-worker:v1\0",
            &[request.manifest().network_id.as_bytes(), &request.0.digest],
        ));
        Self {
            client,
            request,
            worker,
            storage,
            lease_id,
        }
    }
}
impl SettlementPhases for OnlinePhases {
    fn run(
        &self,
        phase: usize,
        journal: WorkflowJournalV1,
        read_only: bool,
    ) -> WorkflowFuture<'_, std::result::Result<Option<Vec<u8>>, PrivateSettlementError>> {
        Box::pin(async move {
            let permit = self
                .worker
                .clone()
                .try_acquire_owned()
                .map_err(|_| PrivateSettlementError::Capacity)?;
            let lease = self
                .storage
                .acquire_runner(&self.lease_id)
                .map_err(PrivateSettlementError::Custody)?;
            let client = self.client.clone();
            let request = self.request.clone();
            // The original join owner keeps this durable lease through OS TLS
            // teardown, panic and cancelled result cleanup, including when the
            // operation is dropped and another instance reopens the same store.
            let (result, original_lease) = run_native_worker(permit, lease, move || {
                run(&client, &request, phase, &journal, read_only)
            })
            .await?;
            drop(original_lease);
            result
        })
    }
}

fn encode<T: norito::NoritoSerialize>(
    value: &T,
) -> std::result::Result<Option<Vec<u8>>, PrivateSettlementError> {
    let bytes = norito::encode_canonical(value)?;
    if bytes.len() > CERTIFICATE_TRANSCRIPT_BYTES as usize {
        return Err(PrivateSettlementError::Binding);
    }
    Ok(Some(bytes))
}
pub(super) fn run(
    client: &Client,
    request: &PrivateSettlementRequest,
    phase: usize,
    journal: &WorkflowJournalV1,
    read_only: bool,
) -> std::result::Result<Option<Vec<u8>>, PrivateSettlementError> {
    let legs = request.0.materials.len();
    if phase == AVAILABILITY {
        // A missing local certificate checkpoint has never authorized promotion. Provisional
        // uploads may exist, so report Waiting rather than inventing external absence.
        if read_only {
            return Ok(None);
        }
        // Each leg's committee certifies its own material. The legs overlap within the
        // bounded leg fan-out and all join before any certificate is used or recorded.
        let certificates = collect_private_settlement_legs_v1(legs, |leg| {
            client.certify_private_settlement_leg_availability_v1(
                &request.0.endpoints[leg],
                &request.0.materials[leg],
            )
        })
        .map_err(PrivateSettlementError::Protocol)?;
        request.manifest_with(&certificates)?;
        return encode(&certificates);
    }
    let availability =
        transcript::<Vec<PrivateSettlementSidecarAvailabilityV1>>(journal, AVAILABILITY)?;
    let manifest = request.manifest_with(&availability)?;
    // The request already computed every digest once; none can fail here.
    let payload_digests = request
        .0
        .materials
        .iter()
        .map(PrivateSettlementProvisionalLegMaterialV1::payload_digest)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if phase == PREPARE {
        if !read_only {
            // This exact all-leg certificate snapshot is already durable in the preceding
            // round. Promotion must never recollect availability or change signer subsets.
            client
                .promote_private_settlement_legs_v1(
                    &request.0.materials,
                    &availability,
                    &request.0.endpoints,
                )
                .map_err(PrivateSettlementError::Protocol)?;
        }
        // Every promotion above has joined. Each committee now stages and votes on its own
        // leg; the legs overlap within the bounded leg fan-out and all join before the
        // first leg in order that failed or has no certificate yet decides the phase.
        let recovered = collect_private_settlement_leg_results_v1(legs, |leg| {
            let (material, endpoints) = (&request.0.materials[leg], &request.0.endpoints[leg]);
            if read_only {
                client.recover_private_settlement_prepare_certificate_v1(
                    endpoints,
                    &manifest,
                    payload_digests[leg],
                    &material.committee_authority,
                )
            } else {
                client
                    .recover_or_certify_private_settlement_prepare_v1(
                        endpoints,
                        &manifest,
                        payload_digests[leg],
                        &material.committee_authority,
                    )
                    .map(Some)
            }
        })
        .map_err(PrivateSettlementError::Protocol)?;
        let Some(certificates) = recovered else {
            return Ok(None);
        };
        // Reuse the complete barrier validator before recording the public certificates.
        Client::build_private_settlement_prepare_barrier_v1(
            manifest,
            &request
                .0
                .materials
                .iter()
                .map(|m| m.committee_authority.clone())
                .collect::<Vec<_>>(),
            request
                .0
                .materials
                .iter()
                .map(|m| m.delta.clone())
                .collect(),
            certificates.clone(),
        )
        .map_err(PrivateSettlementError::Protocol)?;
        return encode(&certificates);
    }
    if phase == COMMIT {
        let barrier = request.barrier(journal)?;
        // The registered barrier is final. Each committee votes Commit on its own leg; the
        // legs overlap within the bounded leg fan-out and all join before any is used.
        let recovered = collect_private_settlement_leg_results_v1(legs, |leg| {
            let (material, endpoints) = (&request.0.materials[leg], &request.0.endpoints[leg]);
            if read_only {
                client.recover_private_settlement_commit_certificate_v1(
                    endpoints,
                    payload_digests[leg],
                    &barrier,
                    &material.committee_authority,
                )
            } else {
                client
                    .recover_or_certify_private_settlement_commit_v1(
                        endpoints,
                        payload_digests[leg],
                        &barrier,
                        &material.committee_authority,
                    )
                    .map(Some)
            }
        })
        .map_err(PrivateSettlementError::Protocol)?;
        let Some(certificates) = recovered else {
            return Ok(None);
        };
        build_private_settlement_commit_bundle_v1(&barrier, &certificates)
            .map_err(PrivateSettlementError::Protocol)?;
        return encode(&certificates);
    }
    Err(PrivateSettlementError::Binding)
}
