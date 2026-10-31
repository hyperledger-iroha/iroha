//! Bounded authenticated progress for the original pending payment only.

use super::*;
use iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint;

pub(super) struct AuthenticatedEntry {
    pub(super) block: VerifiedSumeragiBlock,
    pub(super) evidence: CommittedTransaction,
    terminal: Option<VerifiedPrivateSettlementOutcome>,
}
pub(super) enum LocatedOutcome {
    Absent,
    Pending,
    Found(AuthenticatedEntry),
}
impl LocatedOutcome {
    pub(super) fn found(self) -> Option<(VerifiedSumeragiBlock, CommittedTransaction)> {
        match self {
            Self::Found(entry) => Some((entry.block, entry.evidence)),
            Self::Absent | Self::Pending => None,
        }
    }
    pub(super) fn terminal(self) -> Option<VerifiedPrivateSettlementOutcome> {
        match self {
            Self::Found(entry) => entry.terminal,
            Self::Absent | Self::Pending => None,
        }
    }
}

struct Walk {
    saved: LoadedPendingFinality,
    cursor: SumeragiFinalityVerifier,
    register: Option<VerifiedSumeragiBlock>,
    terminal: Option<VerifiedSumeragiBlock>,
}

fn import(
    checkpoint: &SumeragiFinalityCheckpoint,
    selected: &SumeragiFinalityVerifier,
) -> Result<(SumeragiFinalityVerifier, VerifiedSumeragiBlock), PrivateSettlementError> {
    let (mut verifier, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
        checkpoint,
        &selected.initial_epoch().network_id,
        selected.chain_id(),
    )?;
    if verifier.initial_epoch() != selected.initial_epoch()
        || verifier.instance() != selected.instance()
        || verifier.root_scope() != selected.root_scope()
    {
        return Err(PrivateSettlementError::Binding);
    }
    verifier.retain_recent_decisions(3);
    Ok((verifier, tip))
}

fn checked_entry(
    request: &PrivateSettlementRequest,
    context: &PublicContext,
    phase: FinalityPhase,
    identity: &TransactionIdentityV1,
    block: &VerifiedSumeragiBlock,
) -> Result<AuthenticatedEntry, PrivateSettlementError> {
    let index = block
        .block()
        .network_entrypoints()
        .enumerate()
        .find_map(|(index, entry)| {
            let TransactionEntrypoint::External(transaction) = entry else {
                return None;
            };
            (super::super::identity(transaction) == *identity).then_some(index)
        })
        .ok_or(PrivateSettlementError::Binding)?;
    let evidence = evidence(block.block(), network_index(index)?)?;
    block.verify_transaction_execution(&request.manifest().network_id, &evidence)?;
    let TransactionEntrypoint::External(transaction) = evidence.entrypoint() else {
        return Err(PrivateSettlementError::Binding);
    };
    let terminal = if phase == FinalityPhase::Abort {
        Some(VerifiedPrivateSettlementOutcome::Aborted(
            VerifiedPrivateSettlementAbort::verify_recovered(
                block,
                context.manifest(),
                evidence.clone(),
            )?,
        ))
    } else if phase == FinalityPhase::Finalize && evidence.result().is_ok() {
        // Successful recovery accepts every independently valid matching Prepare quorum.
        // The canonical receipt owner checks phase, request, certificates and exact fees.
        Some(VerifiedPrivateSettlementOutcome::Settled(
            VerifiedPrivateSettlementReceipt::verify_recovered(
                block,
                context.barrier()?,
                evidence.clone(),
            )?,
        ))
    } else {
        let expected = match phase {
            FinalityPhase::Register => REGISTER,
            FinalityPhase::Finalize => FINALIZE,
            FinalityPhase::Abort => unreachable!("Abort has its own successful-execution verifier"),
        };
        if check_payload(request, Some(context), transaction.payload(), false)? != expected {
            return Err(PrivateSettlementError::Binding);
        }
        if evidence.result().is_ok()
            && let Some(fee) = evidence.result().nexus_fee_receipt()
        {
            fee.validate_for_network_input(evidence.entrypoint(), block.height())
                .map_err(|_| PrivateSettlementError::Binding)?;
        }
        None
    };
    Ok(AuthenticatedEntry {
        block: block.clone(),
        evidence,
        terminal,
    })
}

impl Walk {
    fn load(
        custody: &BoundPrivateSettlementFinality,
        selected: &SumeragiFinalityVerifier,
        request: &PrivateSettlementRequest,
        context: &PublicContext,
    ) -> Result<Box<Self>, PrivateSettlementError> {
        let saved = custody.load()?;
        let cursor = if let Some(prefix) = &saved.record.prefix {
            if prefix.checkpoint.height() > prefix.lookup.height {
                return Err(PrivateSettlementError::Binding);
            }
            import(&prefix.checkpoint, selected)?.0
        } else {
            let mut cursor = selected.clone();
            cursor.retain_recent_decisions(3);
            cursor
        };
        let register = if let Some(pin) = &saved.record.register {
            let (_, tip) = import(&pin.checkpoint, selected)?;
            checked_entry(
                request,
                context,
                FinalityPhase::Register,
                &pin.identity,
                &tip,
            )?;
            Some(tip)
        } else {
            None
        };
        // Every saved slot is authenticated before any answer, including a terminal of the
        // other kind. A rejected carrier can never enter this immutable terminal slot.
        let terminal = if let Some(terminal) = &saved.record.terminal {
            let (_, tip) = import(&terminal.pin.checkpoint, selected)?;
            if !checked_entry(
                request,
                context,
                terminal.kind.phase(),
                &terminal.pin.identity,
                &tip,
            )?
            .evidence
            .result()
            .is_ok()
            {
                return Err(PrivateSettlementError::Binding);
            }
            Some(tip)
        } else {
            None
        };
        Ok(Box::new(Self {
            saved,
            cursor,
            register,
            terminal,
        }))
    }

    fn saved_entry(
        &self,
        request: &PrivateSettlementRequest,
        context: &PublicContext,
        phase: FinalityPhase,
        identity: Option<&TransactionIdentityV1>,
    ) -> Result<Option<AuthenticatedEntry>, PrivateSettlementError> {
        let selected = if phase == FinalityPhase::Register {
            self.saved
                .record
                .register
                .as_ref()
                .zip(self.register.as_ref())
        } else {
            self.saved
                .record
                .terminal
                .as_ref()
                .filter(|terminal| terminal.kind.phase() == phase)
                .map(|terminal| &terminal.pin)
                .zip(self.terminal.as_ref())
        };
        if let Some((pin, tip)) = selected {
            let entry = checked_entry(request, context, phase, &pin.identity, tip)?;
            if identity == Some(&pin.identity)
                || (identity.is_none()
                    && phase != FinalityPhase::Register
                    && entry.evidence.result().is_ok())
            {
                return Ok(Some(entry));
            }
        }
        // The current pending prefix also retains its exact reached carrier. This preserves
        // a later included rejection across restart without overwriting a terminal receipt or
        // adding a fourth/history slot. An unfinished or corrected locator grants no answer.
        if let Some(prefix) = &self.saved.record.prefix
            && prefix.lookup.phase == phase
            && prefix.lookup.identity.as_ref() == identity
            && identity.is_some()
            && prefix.checkpoint.height() == prefix.lookup.height
        {
            let (_, tip) = import(&prefix.checkpoint, &self.cursor)?;
            let identity = identity.expect("selected exact prefix identity");
            if !tip.block().network_entrypoints().any(|entry| {
                matches!(entry, TransactionEntrypoint::External(transaction) if super::super::identity(transaction) == *identity)
            }) {
                // A previous untrusted locator was wrong. Keep its authenticated prefix,
                // but let this pending lookup select the corrected earlier height.
                return Ok(None);
            }
            let entry = checked_entry(request, context, phase, identity, &tip)?;
            return Ok(Some(entry));
        }
        Ok(None)
    }

    fn select_cursor(&mut self, height: u64) -> Result<(), PrivateSettlementError> {
        if self
            .cursor
            .authenticated_height()
            .is_some_and(|tip| tip > height)
        {
            // A prior untrusted locator may name a height above the real carrier.
            // Reset only this pending walk under the exact original selected genesis.
            self.cursor = self.cursor.clone().into_genesis_verifier();
        }
        for (pin, tip) in [
            (self.saved.record.register.as_ref(), self.register.as_ref()),
            (
                self.saved
                    .record
                    .terminal
                    .as_ref()
                    .map(|terminal| &terminal.pin),
                self.terminal.as_ref(),
            ),
        ] {
            if let (Some(pin), Some(tip)) = (pin, tip)
                && tip.height() <= height
                && tip.height() > self.cursor.authenticated_height().unwrap_or(0)
            {
                self.cursor = import(&pin.checkpoint, &self.cursor)?.0;
            }
        }
        Ok(())
    }

    fn pin_target(
        &mut self,
        lookup: &FinalityLookup,
        request: &PrivateSettlementRequest,
        context: &PublicContext,
        checkpoint: &SumeragiFinalityCheckpoint,
        block: &VerifiedSumeragiBlock,
    ) -> Result<AuthenticatedEntry, PrivateSettlementError> {
        let identity = if let Some(identity) = lookup.identity {
            identity
        } else {
            if lookup.phase == FinalityPhase::Register {
                return Err(PrivateSettlementError::Binding);
            }
            block
                .block()
                .network_entrypoints()
                .enumerate()
                .find_map(|(index, entry)| {
                    let TransactionEntrypoint::External(transaction) = entry else {
                        return None;
                    };
                    let Executable::Instructions(instructions) = transaction.instructions() else {
                        return None;
                    };
                    let [instruction] = instructions.as_ref() else {
                        return None;
                    };
                    let bundle_id = match lookup.phase {
                        FinalityPhase::Finalize => {
                            instruction
                                .as_any()
                                .downcast_ref::<FinalizeAtomicPrivateSettlementV1>()?
                                .commit_bundle
                                .manifest
                                .bundle_id
                        }
                        FinalityPhase::Abort => {
                            instruction
                                .as_any()
                                .downcast_ref::<AbortAtomicPrivateSettlementV1>()?
                                .manifest
                                .bundle_id
                        }
                        FinalityPhase::Register => return None,
                    };
                    let index = u32::try_from(index).ok()?;
                    (bundle_id == request.manifest().bundle_id
                        && block
                            .block()
                            .network_output_at(index)
                            .is_some_and(|(_, output)| output.result.is_ok()))
                    .then(|| super::super::identity(transaction))
                })
                .ok_or(PrivateSettlementError::Binding)?
        };
        let entry = checked_entry(request, context, lookup.phase, &identity, block)?;
        if lookup.phase == FinalityPhase::Register {
            if let (Some(previous), Some(tip)) = (&self.saved.record.register, &self.register)
                && previous.identity != identity
                && checked_entry(request, context, lookup.phase, &previous.identity, tip)?
                    .evidence
                    .result()
                    .is_ok()
            {
                return Ok(entry);
            }
            self.saved.record.register = Some(PendingFinalityPin {
                identity,
                checkpoint: checkpoint.clone(),
            });
            self.register = Some(block.clone());
        } else if entry.evidence.result().is_ok() {
            let kind = match lookup.phase {
                FinalityPhase::Finalize => FinalityTerminalKind::Settled,
                FinalityPhase::Abort => FinalityTerminalKind::Aborted,
                FinalityPhase::Register => unreachable!("registration handled above"),
            };
            if let Some(previous) = &self.saved.record.terminal {
                if previous.kind != kind {
                    return Err(PrivateSettlementError::Binding);
                }
                // Terminal authority is immutable. The exact newly selected carrier remains
                // observable in the prefix, including any later rejected Finalize.
                return Ok(entry);
            }
            self.saved.record.terminal = Some(PendingFinalityTerminal {
                kind,
                pin: PendingFinalityPin {
                    identity,
                    checkpoint: checkpoint.clone(),
                },
            });
            self.terminal = Some(block.clone());
        }
        Ok(entry)
    }
}

impl SettlementLedger {
    pub(super) async fn finality_located(
        &self,
        phase: FinalityPhase,
        identity: Option<TransactionIdentityV1>,
        mut height: Option<u64>,
    ) -> Result<LocatedOutcome, PrivateSettlementError> {
        let mut lease = self
            .storage
            .acquire_runner(&self.finality_lease)
            .map_err(PrivateSettlementError::Custody)?;
        let selected = self.verifier.clone();
        let custody = self.finality.clone();
        let state = self.state.clone();
        let request = self.request.clone();
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| PrivateSettlementError::Capacity)?;
        let (loaded, returned_lease) = run_native_worker(permit, lease, move || {
            let selected = lock(&selected)?;
            let context = lock(&state)?;
            let context = context.as_ref().ok_or(PrivateSettlementError::Binding)?;
            let walk = Walk::load(&custody, &selected, &request, context)?;
            let found = walk.saved_entry(&request, context, phase, identity.as_ref())?;
            Ok::<_, PrivateSettlementError>((walk, found))
        })
        .await?;
        lease = returned_lease;
        let (mut walk, found) = loaded?;
        if let Some(entry) = found {
            return Ok(LocatedOutcome::Found(entry));
        }
        if height.is_none() {
            let Some(identity) = identity else {
                return Ok(LocatedOutcome::Pending);
            };
            let hash =
                HashOf::from_untyped_unchecked(Hash::prehashed(identity.signed_transaction_hash));
            let Some(response) = self
                .account
                .client()
                .fetch_transaction_status_response_global(hash)
                .await
                .map_err(PrivateSettlementError::Protocol)?
            else {
                return Ok(LocatedOutcome::Absent);
            };
            super::super::super::super::validate_global_pipeline_status_response(&response, hash)
                .map_err(PrivateSettlementError::Protocol)?;
            height = response.status.block_height;
        }
        let Some(height) = height else {
            return Ok(LocatedOutcome::Pending);
        };
        if height < 2 {
            return Err(PrivateSettlementError::Binding);
        }
        let lookup = FinalityLookup {
            phase,
            identity,
            height,
        };
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| PrivateSettlementError::Capacity)?;
        let (selected, returned_lease) = run_native_worker(permit, lease, move || {
            walk.select_cursor(height)?;
            Ok::<_, PrivateSettlementError>(walk)
        })
        .await?;
        lease = returned_lease;
        walk = selected?;
        loop {
            let next = walk
                .cursor
                .authenticated_height()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(PrivateSettlementError::Binding)?;
            let response = self.proof_response(next.min(height)).await?;
            let custody = self.finality.clone();
            let selected = self.verifier.clone();
            let request = self.request.clone();
            let state = self.state.clone();
            let lookup = lookup.clone();
            let permit = self
                .worker
                .clone()
                .try_acquire_owned()
                .map_err(|_| PrivateSettlementError::Capacity)?;
            let (advanced, returned_lease) = run_native_worker(permit, lease, move || {
                let proof = response
                    .decode_checked()
                    .map_err(PrivateSettlementError::Protocol)?;
                let mut candidate = walk.cursor.clone();
                let block = if candidate.authenticated_height() == Some(proof.height()) {
                    candidate.verify_retained_decision(&proof)?
                } else {
                    candidate.verify(&proof)?
                };
                let checkpoint = candidate.export_checkpoint(&proof)?;
                candidate.retain_recent_decisions(3);
                let result = if block.height() == lookup.height {
                    let context = lock(&state)?;
                    Some(walk.pin_target(
                        &lookup,
                        &request,
                        context.as_ref().ok_or(PrivateSettlementError::Binding)?,
                        &checkpoint,
                        &block,
                    ))
                } else {
                    None
                };
                walk.saved.record.prefix = Some(PendingFinalityPrefix { lookup, checkpoint });
                let digest =
                    custody.compare_and_publish(walk.saved.digest.as_ref(), &walk.saved.record)?;
                // No in-memory advance escapes a refused or uncertain publication.
                *lock(&selected)? = candidate.clone();
                walk.cursor = candidate;
                walk.saved.digest = Some(digest);
                Ok::<_, PrivateSettlementError>((walk, result))
            })
            .await?;
            lease = returned_lease;
            let (advanced, result) = advanced?;
            walk = advanced;
            if let Some(result) = result {
                let entry = result?;
                return Ok(LocatedOutcome::Found(entry));
            }
        }
    }
}

#[cfg(test)]
mod tests;
