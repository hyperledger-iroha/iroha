//! Durable bounded native verification for the exact original payment and its carrier.

use super::super::finality_custody::{
    BusinessLocator, FinalityLookup, LoadedPendingFinality, PendingBusinessPin, PendingCarrierPin,
    PendingFinalityPrefix,
};
use super::*;
use iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint;

type Error = NativePaymentObservationError;

struct Walk {
    saved: LoadedPendingFinality,
    cursor: SumeragiFinalityVerifier,
    business: Option<VerifiedPaymentReceipt>,
    carrier: Option<(CommittedOutcome, Option<VerifiedPaymentReceipt>)>,
}

// Both original owners survive cancellation until the native thread, its captured values,
// result and thread-local destructors have completed under the canonical worker lifecycle.
struct WalkCustody {
    _observation: Arc<ObservationCustody>,
    _session: tokio::sync::OwnedMutexGuard<()>,
}

fn import(
    checkpoint: &SumeragiFinalityCheckpoint,
    selected: &SumeragiFinalityVerifier,
) -> Result<(SumeragiFinalityVerifier, VerifiedSumeragiBlock), Error> {
    let (verifier, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
        checkpoint,
        &selected.initial_epoch().network_id,
        selected.chain_id(),
    )?;
    if verifier.initial_epoch() != selected.initial_epoch()
        || verifier.instance() != selected.instance()
        || verifier.root_scope() != selected.root_scope()
    {
        return Err(Error::CandidateMismatch);
    }
    Ok((verifier, tip))
}

fn transaction_identity(transaction: &SignedTransaction) -> TransactionIdentityV1 {
    TransactionIdentityV1 {
        signed_transaction_hash: *Hash::from(transaction.hash()).as_ref(),
        entrypoint_hash: *Hash::from(transaction.hash_as_entrypoint()).as_ref(),
    }
}

fn carrier(
    request: &NativePaymentRequest,
    identity: &TransactionIdentityV1,
    block: &VerifiedSumeragiBlock,
) -> Result<
    (
        CommittedOutcome,
        Option<VerifiedPaymentReceipt>,
        Option<BusinessLocator>,
    ),
    Error,
> {
    let index = block
        .block()
        .network_entrypoints()
        .enumerate()
        .find_map(|(index, entry)| {
            let TransactionEntrypoint::External(transaction) = entry else {
                return None;
            };
            (transaction_identity(transaction) == *identity).then_some(index)
        })
        .ok_or(Error::CandidateMismatch)?;
    let evidence = evidence(
        block.block(),
        u32::try_from(index).map_err(|_| Error::CandidateMismatch)?,
    )?;
    block.verify_transaction_execution(&request.intent.network_id, &evidence)?;
    let TransactionEntrypoint::External(transaction) = evidence.entrypoint() else {
        return Err(Error::CandidateMismatch);
    };
    request
        .validate_payload(transaction.payload())
        .map_err(|_| Error::CandidateMismatch)?;
    if evidence.result().is_ok() {
        let hash =
            HashOf::from_untyped_unchecked(Hash::prehashed(identity.signed_transaction_hash));
        let receipt = VerifiedPaymentReceipt::verify(
            block,
            request.intent.network_id,
            hash,
            &request.intent,
            evidence,
        )?;
        NativePaymentLedger::check_fees(request, &receipt)?;
        Ok((
            CommittedOutcome::Applied {
                block_height: Some(block.height()),
            },
            Some(receipt),
            None,
        ))
    } else {
        let replay = match &evidence.result().0 {
            Err(TransactionRejectionReason::Validation(ValidationFail::SettlementReplay(
                replay,
            ))) if replay.settlement_id == request.intent.settlement_id => Some(BusinessLocator {
                height: replay.committed_block_height,
                block_hash: *Hash::from(replay.committed_block_hash).as_ref(),
            }),
            _ => None,
        };
        Ok((
            CommittedOutcome::Rejected {
                detail: "independently authenticated execution rejected this exact signed carrier"
                    .into(),
            },
            None,
            replay,
        ))
    }
}

fn business(
    request: &NativePaymentRequest,
    block: &VerifiedSumeragiBlock,
) -> Result<VerifiedPaymentReceipt, Error> {
    let index = block
        .block()
        .network_entrypoints()
        .enumerate()
        .find_map(|(index, entry)| {
            let TransactionEntrypoint::External(transaction) = entry else {
                return None;
            };
            let iroha_data_model::transaction::Executable::Instructions(instructions) =
                transaction.instructions()
            else {
                return None;
            };
            let [instruction] = instructions.as_ref() else {
                return None;
            };
            let any = instruction.as_any();
            let intent = any.downcast_ref::<SettleAtomic>().or_else(|| {
                match any.downcast_ref::<SettlementInstructionBox>() {
                    Some(SettlementInstructionBox::Atomic(intent)) => Some(intent),
                    _ => None,
                }
            })?;
            let index = u32::try_from(index).ok()?;
            (intent.settlement_id == request.intent.settlement_id
                && block
                    .block()
                    .network_output_at(index)
                    .is_some_and(|(_, output)| output.result.is_ok()))
            .then_some(index)
        })
        .ok_or(Error::CandidateMismatch)?;
    let receipt = VerifiedPaymentReceipt::verify_recovered(
        block,
        request.intent.network_id,
        &request.intent,
        evidence(block.block(), index)?,
    )?;
    NativePaymentLedger::check_fees(request, &receipt)?;
    Ok(receipt)
}

impl Walk {
    fn load(
        custody: &BoundNativePaymentFinality,
        selected: &SumeragiFinalityVerifier,
        request: &NativePaymentRequest,
    ) -> Result<Box<Self>, Error> {
        let saved = custody.load()?;
        let cursor = match &saved.record.prefix {
            Some(prefix) => import(&prefix.checkpoint, selected)?.0,
            None => selected.clone(),
        };
        // Authenticate every saved slot before using any answer. Even a valid successful
        // payment cannot hide a corrupt or substituted current-carrier checkpoint.
        let checked_carrier = saved
            .record
            .carrier
            .as_ref()
            .map(|pin| {
                let (_, block) = import(&pin.checkpoint, selected)?;
                let (outcome, receipt, replay) = carrier(request, &pin.identity, &block)?;
                if replay != pin.replay {
                    return Err(Error::CandidateMismatch);
                }
                Ok::<_, Error>((outcome, receipt))
            })
            .transpose()?;
        let checked_business = saved
            .record
            .business
            .as_ref()
            .map(|pin| {
                let (_, block) = import(&pin.checkpoint, selected)?;
                let receipt = business(request, &block)?;
                let TransactionEntrypoint::External(transaction) = receipt.evidence().entrypoint()
                else {
                    return Err(Error::CandidateMismatch);
                };
                if transaction_identity(transaction) != pin.identity {
                    return Err(Error::CandidateMismatch);
                }
                Ok::<_, Error>(receipt)
            })
            .transpose()?;
        Ok(Box::new(Self {
            saved,
            cursor,
            business: checked_business,
            carrier: checked_carrier,
        }))
    }

    fn select(&mut self, height: u64, warm: &SumeragiFinalityVerifier) -> Result<(), Error> {
        if height < 2 {
            return Err(Error::CandidateMismatch);
        }
        if self
            .cursor
            .authenticated_height()
            .is_some_and(|tip| tip > height)
        {
            self.cursor = self.cursor.clone().into_genesis_verifier();
        }
        // A corrected untrusted locator only resets this pending walk. Shared warm progress
        // is never rewound; a later authenticated warm tip is usable only below the target.
        if warm.authenticated_height().is_some_and(|tip| {
            tip <= height && tip > self.cursor.authenticated_height().unwrap_or(0)
        }) {
            self.cursor = warm.clone();
        }
        for checkpoint in self
            .saved
            .record
            .carrier
            .iter()
            .map(|pin| &pin.checkpoint)
            .chain(self.saved.record.business.iter().map(|pin| &pin.checkpoint))
        {
            if checkpoint.height() <= height
                && checkpoint.height() > self.cursor.authenticated_height().unwrap_or(0)
            {
                self.cursor = import(checkpoint, warm)?.0;
            }
        }
        Ok(())
    }

    fn pin(
        &mut self,
        request: &NativePaymentRequest,
        lookup: &FinalityLookup,
        checkpoint: &SumeragiFinalityCheckpoint,
        block: &VerifiedSumeragiBlock,
    ) -> Result<Answer, Error> {
        let receipt = match lookup {
            FinalityLookup::Business(locator) => {
                if Hash::from(block.header().hash()).as_ref() != &locator.block_hash {
                    return Err(Error::CandidateMismatch);
                }
                business(request, block)?
            }
            FinalityLookup::Carrier { identity, .. } => {
                let (outcome, receipt, replay) = carrier(request, identity, block)?;
                self.saved.record.carrier = Some(PendingCarrierPin {
                    identity: *identity,
                    checkpoint: checkpoint.clone(),
                    replay,
                });
                if let Some(receipt) = &receipt {
                    self.pin_business(receipt, checkpoint)?;
                }
                return Ok(Answer::Carrier(outcome, receipt));
            }
        };
        self.pin_business(&receipt, checkpoint)?;
        Ok(Answer::Business(receipt))
    }

    fn pin_business(
        &mut self,
        receipt: &VerifiedPaymentReceipt,
        checkpoint: &SumeragiFinalityCheckpoint,
    ) -> Result<(), Error> {
        if self.saved.record.business.is_none() {
            let TransactionEntrypoint::External(transaction) = receipt.evidence().entrypoint()
            else {
                return Err(Error::CandidateMismatch);
            };
            self.saved.record.business = Some(PendingBusinessPin {
                identity: transaction_identity(transaction),
                checkpoint: checkpoint.clone(),
            });
        }
        Ok(())
    }
}

enum Selection {
    Business(Option<BusinessLocator>),
    Carrier(TransactionIdentityV1),
}
enum Answer {
    Business(VerifiedPaymentReceipt),
    Carrier(CommittedOutcome, Option<VerifiedPaymentReceipt>),
}

impl NativePaymentLedger {
    pub(super) async fn finality_business(
        &self,
        marker: Option<&NativePaymentCompletionV1>,
    ) -> Result<Option<VerifiedPaymentReceipt>, Error> {
        let selected = Selection::Business(marker.map(|marker| BusinessLocator {
            height: marker.block_height,
            block_hash: marker.block_hash,
        }));
        match self.resolve(selected).await? {
            Some(Answer::Business(receipt)) => Ok(Some(receipt)),
            None => Ok(None),
            _ => Err(Error::CandidateMismatch),
        }
    }

    pub(super) async fn finality_carrier(
        &self,
        identity: TransactionIdentityV1,
    ) -> Result<(CommittedOutcome, Option<VerifiedPaymentReceipt>), Error> {
        match self.resolve(Selection::Carrier(identity)).await? {
            Some(Answer::Carrier(outcome, receipt)) => Ok((outcome, receipt)),
            _ => Err(Error::CandidateMismatch),
        }
    }

    async fn resolve(&self, selection: Selection) -> Result<Option<Answer>, Error> {
        let observation = lock(&self.observation)?
            .upgrade()
            .ok_or(Error::Synchronization)?;
        let mut custody = WalkCustody {
            _observation: observation,
            _session: self.session.owner.catch_up.clone().lock_owned().await,
        };
        let finality = self.finality.clone();
        let session = self.session.clone();
        let request = self.request.clone();
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Capacity)?;
        let (loaded, returned) =
            super::super::super::run_native_worker(permit, custody, move || {
                Walk::load(&finality, &*session.verifier()?, &request)
            })
            .await?;
        custody = returned;
        let mut walk = loaded?;
        let lookup = match selection {
            Selection::Business(marker) => {
                if let Some(receipt) = walk.business.take() {
                    return Ok(Some(Answer::Business(receipt)));
                }
                let replay = walk
                    .saved
                    .record
                    .carrier
                    .as_ref()
                    .and_then(|pin| pin.replay);
                let locator = if let Some(locator) = marker.or(replay) {
                    Some(locator)
                } else {
                    self.source
                        .settlement(&self.request.intent.settlement_id)
                        .await?
                        .map(|locator| BusinessLocator {
                            height: locator.height,
                            block_hash: *Hash::from(locator.block_hash).as_ref(),
                        })
                };
                let Some(locator) = locator else {
                    return Ok(None);
                };
                FinalityLookup::Business(locator)
            }
            Selection::Carrier(identity) => {
                if walk
                    .saved
                    .record
                    .carrier
                    .as_ref()
                    .is_some_and(|pin| pin.identity == identity)
                    && let Some((outcome, receipt)) = walk.carrier.take()
                {
                    return Ok(Some(Answer::Carrier(outcome, receipt)));
                }
                let hash = HashOf::from_untyped_unchecked(Hash::prehashed(
                    identity.signed_transaction_hash,
                ));
                match self.source.carrier(hash).await? {
                    CarrierLocation::Absent => {
                        return Ok(Some(Answer::Carrier(CommittedOutcome::ExactAbsence, None)));
                    }
                    CarrierLocation::Observed(None) => {
                        return Ok(Some(Answer::Carrier(
                            CommittedOutcome::ObservedNotFinal,
                            None,
                        )));
                    }
                    CarrierLocation::Observed(Some(height)) => {
                        FinalityLookup::Carrier { identity, height }
                    }
                }
            }
        };
        let target = lookup.height();
        let session = self.session.clone();
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Capacity)?;
        let (selected, returned) =
            super::super::super::run_native_worker(permit, custody, move || {
                walk.select(target, &*session.verifier()?)?;
                Ok::<_, Error>(walk)
            })
            .await?;
        custody = returned;
        walk = selected?;
        loop {
            let next = walk
                .cursor
                .authenticated_height()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(Error::CandidateMismatch)?
                .min(target);
            let response = self.read_proof(next).await?;
            let request = self.request.clone();
            let session = self.session.clone();
            let finality = self.finality.clone();
            let lookup = lookup.clone();
            let permit = self
                .worker
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::Capacity)?;
            let (advanced, returned) =
                super::super::super::run_native_worker(permit, custody, move || {
                    let proof = response.decode_checked()?;
                    if proof.height() != next {
                        return Err(Error::CandidateMismatch);
                    }
                    let mut candidate = walk.cursor.clone();
                    let block = if candidate.authenticated_height() == Some(next) {
                        candidate.verify_retained_decision(&proof)?
                    } else {
                        candidate.verify(&proof)?
                    };
                    let checkpoint = candidate.export_checkpoint(&proof)?;
                    candidate.retain_recent_decisions(session.owner.retained_decisions);
                    let answer =
                        (next == target).then(|| walk.pin(&request, &lookup, &checkpoint, &block));
                    walk.saved.record.prefix = Some(PendingFinalityPrefix { lookup, checkpoint });
                    let digest = finality
                        .compare_and_publish(walk.saved.digest.as_ref(), &walk.saved.record)?;
                    // Refused or uncertain publication cannot advance the warm cache or cursor.
                    let mut warm = session.verifier()?;
                    if candidate.authenticated_height() > warm.authenticated_height() {
                        *warm = candidate.clone();
                    }
                    walk.cursor = candidate;
                    walk.saved.digest = Some(digest);
                    Ok::<_, Error>((walk, answer))
                })
                .await?;
            custody = returned;
            let (advanced, answer) = advanced?;
            walk = advanced;
            if let Some(answer) = answer {
                return answer.map(Some);
            }
        }
    }
}
