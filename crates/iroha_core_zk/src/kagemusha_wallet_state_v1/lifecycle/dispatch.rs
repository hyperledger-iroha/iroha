//! Source-bound native preparation and exact retry through the existing custody owner.

use crate::kagemusha_wallet_advance_v1::kagemusha_wallet_provider_digest_v1 as digest;

use super::*;

const FROZEN_BOUND: usize =
    KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1 + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1 + 1024;
const PLAN_BOUND: usize = PREPARATION_MAX_BYTES + 1024;

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::PreparationPlanV1")]
struct Plan {
    version: u16,
    scheme: [u8; 32],
    wallet: [u8; 32],
    request_id: [u8; 32],
    kind: KagemushaWalletOperationKindV1,
    request: [u8; 32],
    source: [u8; 32],
    native: Vec<u8>,
}

impl Plan {
    fn require(
        &self,
        scheme: &[u8; 32],
        wallet: &[u8; 32],
        request: &OperationRequestV1,
        original: &[u8],
    ) -> Result<(), Error> {
        if self.version != 1
            || self.scheme != *scheme
            || self.wallet != *wallet
            || self.request_id != request.request_id
            || self.kind != request.kind()
            || self.request != digest("wallet-preparation-request", original)
            || self.source == [0; 32]
            || self.native.is_empty()
            || self.native.len() > PREPARATION_MAX_BYTES
        {
            return Err(Error::WitnessLost("native preparation binding"));
        }
        Ok(())
    }
}

fn original(
    frozen: &FrozenTransition,
    role: KagemushaWalletRetainedInputRoleV1,
    bytes: &[u8],
) -> bool {
    let mut inputs = frozen
        .capsule
        .retained_inputs
        .iter()
        .filter(|input| input.role == role);
    inputs.next().is_some_and(|input| input.bytes == bytes) && inputs.next().is_none()
}

fn policy_original(
    frozen: &FrozenTransition,
    kind: KagemushaWalletPolicyUpdateKindV1,
    complete: &[u8],
) -> bool {
    let mut inputs = frozen
        .capsule
        .retained_inputs
        .iter()
        .filter(|input| input.role == KagemushaWalletRetainedInputRoleV1::PolicyUpdate);
    let Some(input) = inputs.next() else {
        return false;
    };
    inputs.next().is_none()
        && super::super::verify_policy_update_original(
            kind,
            &frozen.capsule.scheme_id,
            &input.bytes,
            complete,
        )
        .is_ok()
}

fn require_frozen(
    request: &OperationRequestV1,
    source: &[u8; 32],
    frozen: &FrozenTransition,
) -> Result<(), Error> {
    use KagemushaWalletRetainedInputRoleV1 as R;
    frozen.validate()?;
    let capsule = &frozen.capsule;
    if capsule.kind != request.kind() || capsule.predecessor_capsule_digest != *source {
        return Err(Error::Invalid("prepared capsule identity"));
    }
    let matches = match &request.action {
        OperationActionV1::Load { receipt, finality } => {
            original(frozen, R::LoadReceipt, receipt) && original(frozen, R::LoadFinality, finality)
        }
        OperationActionV1::Send { request } => original(frozen, R::Request, request),
        OperationActionV1::Receive {
            payment,
            payer_credential,
            certificates,
        } => {
            original(frozen, R::Payment, payment)
                && original(frozen, R::Credential, payer_credential)
                && original(frozen, R::CertificateSet, certificates)
        }
        OperationActionV1::Refresh {
            kind,
            update,
            certificates,
        } => {
            policy_original(frozen, *kind, update)
                && original(frozen, R::CertificateSet, certificates)
                && matches!(capsule.statement.effect, KagemushaWalletEffectV1::RefreshPolicy { update_kind: actual, .. } if actual == *kind)
        }
        OperationActionV1::Unload { amount, charge } => {
            let quote = charge
                .as_ref()
                .map(|charge| {
                    KagemushaWalletChargeQuoteV1::decode_canonical(
                        &charge.quote,
                        &capsule.scheme_id,
                    )
                    .map(|quote| quote.charge_quote_digest())
                })
                .transpose()
                .map_err(|_| Error::Invalid("prepared charge quote"))?;
            matches!(capsule.statement.effect, KagemushaWalletEffectV1::Unload { amount: actual, charge_quote, .. }
                if actual == *amount && charge_quote == quote.unwrap_or([0; 32]))
        }
        OperationActionV1::Retire => capsule.statement.effect == KagemushaWalletEffectV1::Retiring,
    };
    if !matches {
        return Err(Error::Invalid("prepared request originals"));
    }
    Ok(())
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::PreparationEntryV1")]
struct Entry {
    request: [u8; 32],
    plan: Option<[u8; 32]>,
    capsule: Option<[u8; 32]>,
    operation: Option<[u8; 32]>,
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    fn preparation_entry(
        &mut self,
        manifest: &manifest::Manifest,
        request_id: &[u8; 32],
    ) -> Result<Option<Entry>, Error> {
        manifest
            .preparations
            .get(&mut self.archive, request_id)?
            .map(|bytes| archive::decode(&bytes))
            .transpose()
    }

    fn publish_preparation(
        &mut self,
        selected: &mut [u8; 32],
        manifest: &mut manifest::Manifest,
        request_id: [u8; 32],
        entry: &Entry,
    ) -> Result<(), Error> {
        manifest.preparations =
            manifest
                .preparations
                .set(&mut self.archive, request_id, &archive::encode(entry)?)?;
        *selected = self.publish_manifest(*selected, manifest)?;
        Ok(())
    }

    /// Resolve a local request identity through its source-selected actual operation mapping.
    /// An uncommitted preparation has no completion, even when its native plan is durable.
    ///
    /// # Errors
    /// Missing source-bound objects, corrupt mapping or unavailable custody never mean absence.
    pub fn retry_request(&mut self, request_id: &[u8; 32]) -> Result<RequestStatusV1, Error> {
        if *request_id == [0; 32] {
            return Err(Error::Invalid("lifecycle request identity"));
        }
        // The selected manifest remains authoritative while Advance is Pending.
        // Tail synchronization requires Released and would hide the exact operation lookup.
        let (_, manifest) = self.manifest()?;
        let Some(entry) = self.preparation_entry(&manifest, request_id)? else {
            return Ok(RequestStatusV1::Unknown);
        };
        let original = self
            .archive
            .read_object(&entry.request, REQUEST_MAX_BYTES)?;
        let (scheme, _) = self.proofs.ledger_scope()?;
        let request = OperationRequestV1::decode(&original, &scheme)?;
        if request.request_id != *request_id {
            return Err(Error::WitnessLost("lifecycle request key"));
        }
        if entry.capsule.is_some() != entry.operation.is_some() {
            return Err(Error::WitnessLost("prepared operation mapping"));
        }
        match entry.operation {
            Some(operation) => Ok(self
                .retry(&operation)?
                .map_or(RequestStatusV1::Preparing, RequestStatusV1::Outcome)),
            None => Ok(RequestStatusV1::Preparing),
        }
    }
}

impl<C: Custody, A: ArchiveStore, N: NativePreparation> Coordinator<C, A, N> {
    /// Execute typed intent through the same installed native preparation/proof owner.
    ///
    /// The caller holds exclusive wallet custody for this whole call. The existing
    /// source-selected manifest binds each request, plan and prepared capsule before the next
    /// phase starts. Restart reuses original choices and resolves the actual statement-derived
    /// operation identity before proving. No preparation record grants completion authority.
    /// Permanent request mappings and pending plans are excluded from proof collection.
    ///
    /// # Errors
    /// Conflicting request, missing native custody, unavailable storage/artifacts, invalid proof
    /// or uncertain Advance. A stale prepared source returns NotPerformed without rebasing.
    pub fn execute(&mut self, request: OperationRequestV1) -> Result<Completion, Error> {
        let _payment = self.scheduler.payment();
        let (scheme, _) = self.proofs.ledger_scope()?;
        request.validate(&scheme)?;
        let bytes = archive::encode(&request)?;
        if bytes.len() > REQUEST_MAX_BYTES {
            return Err(Error::Invalid("lifecycle request size"));
        }
        // Resolve the source-selected request before requiring a Released head: Advance may
        // already have irreversibly selected this operation and still owe its receipt.
        let (_, selected_manifest) = self.manifest()?;
        if let Some(entry) = self.preparation_entry(&selected_manifest, &request.request_id)? {
            if self
                .archive
                .read_object(&entry.request, REQUEST_MAX_BYTES)?
                != bytes
            {
                return Err(Error::OperationConflict);
            }
            if entry.capsule.is_some() != entry.operation.is_some() {
                return Err(Error::WitnessLost("prepared operation mapping"));
            }
            if let Some(operation) = entry.operation {
                if let Some(completion) = self.retry(&operation)? {
                    return Ok(completion);
                }
            }
        }
        let (mut selected, mut manifest) = self.sync_manifest()?;
        // This dispatcher starts from an enrolled/released wallet. Enrollment is a separate
        // pre-wallet owner; no preparation may manufacture a Bootstrap source.
        let sequence = manifest.indexed.ok_or(Error::NoHead)?;
        let mut entry =
            if let Some(entry) = self.preparation_entry(&manifest, &request.request_id)? {
                let original = self
                    .archive
                    .read_object(&entry.request, REQUEST_MAX_BYTES)?;
                if original != bytes {
                    return Err(Error::OperationConflict);
                }
                entry
            } else {
                let entry = Entry {
                    request: self.archive.write_object(&bytes, REQUEST_MAX_BYTES)?,
                    plan: None,
                    capsule: None,
                    operation: None,
                };
                self.publish_preparation(&mut selected, &mut manifest, request.request_id, &entry)?;
                entry
            };
        // Completion lookup precedes source selection/proof work. A mapping's absence from
        // authenticated storage is a custody failure, never permission for a second debit.
        if entry.capsule.is_some() != entry.operation.is_some() {
            return Err(Error::WitnessLost("prepared operation mapping"));
        }
        if let Some(operation) = entry.operation {
            if let Some(completion) = self.retry(&operation)? {
                return Ok(completion);
            }
        }
        let released = self.indexed_step(&manifest, sequence)?;
        let source_digest = valid(released.frozen.capsule.capsule_digest())?;
        let held_plan = entry
            .plan
            .map(|plan| archive::decode::<Plan>(&self.archive.read_object(&plan, PLAN_BOUND)?))
            .transpose()?;
        if let Some(plan) = &held_plan {
            plan.require(&self.scheme_id, &self.wallet_id, &request, &bytes)?;
            if plan.source != source_digest {
                return Ok(Completion::NotPerformed(NotPerformed::StaleHead));
            }
        }
        let folded = self.read_fold(&released)?.map(|fold| fold.record);
        if request.kind().consumes_lineage() && folded.is_none() {
            return Err(Error::FoldRequired);
        }
        let source = PreparationSourceV1 {
            released: &released,
            folded: folded.as_ref(),
        };
        let plan = if let Some(plan) = held_plan {
            plan
        } else {
            let native = self
                .proofs
                .plan_preparation(&request, &source, &mut self.archive)?;
            let plan = Plan {
                version: 1,
                scheme: self.scheme_id,
                wallet: self.wallet_id,
                request_id: request.request_id,
                kind: request.kind(),
                request: digest("wallet-preparation-request", &bytes),
                source: source_digest,
                native,
            };
            plan.require(&self.scheme_id, &self.wallet_id, &request, &bytes)?;
            self.proofs
                .validate_preparation(&request, &source, &plan.native, &mut self.archive)?;
            let original = archive::encode(&plan)?;
            entry.plan = Some(self.archive.write_object(&original, PLAN_BOUND)?);
            self.publish_preparation(&mut selected, &mut manifest, request.request_id, &entry)?;
            plan
        };
        plan.require(&self.scheme_id, &self.wallet_id, &request, &bytes)?;
        if plan.source != source_digest {
            return Ok(Completion::NotPerformed(NotPerformed::StaleHead));
        }
        self.proofs
            .validate_preparation(&request, &source, &plan.native, &mut self.archive)?;
        let frozen = if let Some(capsule) = entry.capsule {
            self.frozen(capsule)?
        } else {
            let frozen = self.proofs.prove_preparation(
                &request,
                &source,
                &plan.native,
                &mut self.archive,
            )?;
            require_frozen(&request, &source_digest, &frozen)?;
            let original = archive::encode(&frozen)?;
            if original.len() > FROZEN_BOUND {
                return Err(Error::Invalid("prepared capsule size"));
            }
            let capsule = valid(frozen.capsule.capsule_digest())?;
            self.archive.put(ArchiveKey::Capsule(capsule), &original)?;
            entry.capsule = Some(capsule);
            entry.operation = Some(frozen.capsule.operation_id);
            self.publish_preparation(&mut selected, &mut manifest, request.request_id, &entry)?;
            frozen
        };
        require_frozen(&request, &source_digest, &frozen)?;
        if entry.operation != Some(frozen.capsule.operation_id) {
            return Err(Error::WitnessLost("prepared operation identity"));
        }
        // Recheck source selection after any preparation/publication work. This is under the
        // same exclusive owner; commit independently repeats full native and Advance checks.
        let (_, current) = self.sync_manifest()?;
        if current.capsule != source_digest {
            return Ok(Completion::NotPerformed(NotPerformed::StaleHead));
        }
        self.commit(frozen)
    }
}
