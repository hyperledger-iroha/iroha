// Included in work_registry: all carriers remain behind its exclusive owner.

/// A complete, bounded cold Proposal continuation installed without ledger edits.
pub(super) struct InstalledRecoveredControlContinuationRegistryCutV1<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    ledger: super::ledger::LifecycleLedgerV1,
    addresses: [Option<ConcreteWorkAddress>; 3],
}

impl ConcreteLifecycleWorkRegistry {
    /// Recognize only a Proposal followed by its exact advanced Prepare and an
    /// optional live/advanced Commit. Ordinary carriers are counted separately.
    fn exact_control_continuation_addresses(&self) -> Option<[Option<ConcreteWorkAddress>; 3]> {
        let mut children = self
            .entries
            .iter()
            .filter(|(_, work)| {
                matches!(
                    work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_)
                        | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
                )
            })
            .map(|(&address, work)| (address, work))
            .collect::<Vec<_>>();
        if !(2..=3).contains(&children.len()) {
            return None;
        }
        children.sort_by_key(|(address, _)| address.ordinal);
        let (first, work) = children[0];
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(proposal) =
            &work.kind
        else {
            return None;
        };
        if !matches!(
            proposal.parent,
            DurableRecoveredLifecycleSignParentV1::Control(_)
        ) || proposal.broadcast.candidate().stage.kind()
            != super::LifecycleStageKind::BroadcastProposal
            || !proposal.is_unpaired()
            || !work.validates_at(first)
        {
            return None;
        }
        let mut addresses = [Some(first), None, None];
        let mut owners = std::collections::BTreeSet::from([first.owner]);
        let mut previous = proposal;
        for (index, &(address, work)) in children.iter().enumerate().skip(1) {
            if !owners.insert(address.owner) || !work.validates_at(address) {
                return None;
            }
            let expected_parent = previous.address.ordinal.checked_add(1)?;
            match &work.kind {
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) => {
                    let DurableRecoveredLifecycleSignParentV1::NextWalVote(parent) =
                        &broadcast.parent
                    else {
                        return None;
                    };
                    let expected_stage = if index == 1 {
                        super::LifecycleStageKind::BroadcastPrepareVote
                    } else {
                        super::LifecycleStageKind::BroadcastCommitVote
                    };
                    if parent.address.ordinal != expected_parent
                        || parent.address.owner != address.owner
                        || broadcast.broadcast.candidate().stage.kind() != expected_stage
                        || !previous.is_unpaired()
                    {
                        return None;
                    }
                    previous = broadcast;
                    if index + 1 == children.len() && !broadcast.is_unpaired() {
                        return None;
                    }
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                    if index != 2
                        || index + 1 != children.len()
                        || address.ordinal != expected_parent
                        || !sign.validates_at(address, work.digest)
                        || !previous.pairs_exact_next_sign(address, work.digest)
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            addresses[index] = Some(address);
        }
        Some(addresses)
    }

    /// Publish all live carriers only after the sealed continuation and exact
    /// on-disk frame agree. No durable rows are allocated, removed, or rewritten.
    #[allow(clippy::too_many_lines)]
    pub(super) fn install_recovered_control_continuation<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        continuation: super::wal_recovery::RecoveredControlContinuationV1,
    ) -> Result<InstalledRecoveredControlContinuationRegistryCutV1<'registry>, &'static str> {
        if !self.entries.is_empty()
            || !continuation.exactly_matches_ledger(ledger)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err("cold Proposal continuation registry preflight changed");
        }
        let address_at = |ordinal, capacity| {
            ledger
                .records()
                .iter()
                .find(|row| row.ordinal() == ordinal)
                .and_then(|row| {
                    ConcreteWorkAddress::new(
                        row.owner(),
                        ordinal,
                        PhysicalSlotId::for_capacity(capacity, 0),
                    )
                })
                .ok_or("cold Proposal continuation lost its physical address")
        };
        let (control, parent_ordinal, broadcast, broadcast_ordinal, votes) = continuation
            .into_registry_parts(RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1::new());
        let parent_address = address_at(parent_ordinal, CapacityClass::Effect)?;
        let broadcast_address = address_at(broadcast_ordinal, CapacityClass::Consensus)?;
        let parent = control
            .into_durable_carrier(
                parent_address.owner,
                parent_address.ordinal,
                parent_address.slot,
            )
            .map_err(|_| "cold Proposal continuation lost its control carrier")?;
        let mut staged = Self::default();
        staged.entries.insert(
            broadcast_address,
            ConcreteLifecycleWork {
                digest: broadcast.digest(),
                kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                    Box::new(DurableRecoveredLifecycleSignedBroadcastWork {
                        parent: DurableRecoveredLifecycleSignParentV1::Control(
                            DurableRecoveredWalControlSignWork {
                                carrier: parent,
                                address: parent_address,
                                dispatch_key: None,
                            },
                        ),
                        broadcast,
                        verified: verified.clone(),
                        address: broadcast_address,
                        paired_next_sign: None,
                    }),
                ),
            },
        );
        let final_sign = votes
            .last()
            .filter(|vote| vote.broadcast.is_none())
            .map(|vote| {
                address_at(vote.ordinal, CapacityClass::Effect)
                    .map(|address| (address, vote.vote.digest()))
            })
            .transpose()?;
        for vote in votes {
            let sign_address = address_at(vote.ordinal, CapacityClass::Effect)?;
            let parent = DurableRecoveredLifecycleNextWalVoteSignWork {
                projection: vote.vote,
                verified: verified.clone(),
                address: sign_address,
                dispatch_key: None,
            };
            let (address, work) = match vote.broadcast {
                None => (
                    sign_address,
                    ConcreteLifecycleWork {
                        digest: parent.projection.digest(),
                        kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                            parent,
                        ),
                    },
                ),
                Some((ordinal, broadcast)) => {
                    let address = address_at(ordinal, CapacityClass::Consensus)?;
                    let paired_next_sign =
                        final_sign.filter(|(next, _)| ordinal.checked_add(1) == Some(next.ordinal));
                    (
                        address,
                        ConcreteLifecycleWork {
                            digest: broadcast.digest(),
                            kind:
                                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                                    Box::new(DurableRecoveredLifecycleSignedBroadcastWork {
                                        parent: DurableRecoveredLifecycleSignParentV1::NextWalVote(
                                            parent,
                                        ),
                                        broadcast,
                                        verified: verified.clone(),
                                        address,
                                        paired_next_sign,
                                    }),
                                ),
                        },
                    )
                }
            };
            if staged.entries.insert(address, work).is_some() {
                return Err("cold Proposal continuation aliases a physical carrier");
            }
        }
        let addresses = staged
            .exact_control_continuation_addresses()
            .ok_or("cold Proposal continuation registry is not a closed causal chain")?;
        if !staged.exact_control_continuation_frame(ledger, addresses)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err("cold Proposal continuation changed before registry publication");
        }
        self.entries = staged.entries;
        Ok(InstalledRecoveredControlContinuationRegistryCutV1 {
            registry: self,
            ledger: ledger.clone(),
            addresses,
        })
    }

    fn exact_control_continuation_frame(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
        addresses: [Option<ConcreteWorkAddress>; 3],
    ) -> bool {
        self.exact_control_continuation_addresses() == Some(addresses)
            && addresses.into_iter().flatten().all(|address| {
                self.entries.get(&address).is_some_and(|work| {
                    work.validates_at(address)
                        && match &work.kind {
                            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                                broadcast,
                            ) => broadcast.validates_in_ledger(ledger),
                            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                                sign,
                            ) => sign.validates_in_ledger(ledger),
                            _ => false,
                        }
                })
            })
    }
}

impl InstalledRecoveredControlContinuationRegistryCutV1<'_> {
    /// Install ordinary body work beside the complete exclusive WAL chain.
    pub(super) fn install_body_pipeline(
        &mut self,
        body_pipeline: PreparedDurableCertifiedBodyPipelineStartupV1,
    ) -> Result<(), &'static str> {
        body_pipeline
            .install_alongside_recovered_wal_authority(&mut *self.registry)
            .map_err(|_| "cold Proposal continuation and body-pipeline carriers conflict")
    }

    /// Open the exact storage census and publish its process-local coordinator.
    pub(super) fn open_with_exact_store_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        store: super::ledger::LifecycleLedgerStoreV1,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<(LifecycleCoordinator, AuthenticatedLifecycleRecoveryCut), &'static str> {
        if !store.load().is_ok_and(|ledger| ledger == self.ledger)
            || !recovery.owns_control_continuation_frame(&self.ledger)
            || !self
                .registry
                .exact_control_continuation_frame(&self.ledger, self.addresses)
        {
            return Err("installed cold Proposal continuation changed its exact frame");
        }
        let prepared = LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            store,
            payload_store,
            &recovery,
        )
        .map_err(|_| "cold Proposal continuation coordinator preparation failed")?;
        let extra = RecoveredWalRegistrySlotV1::ControlContinuation(self.addresses);
        if !self
            .registry
            .exactly_covers_recovered_ready_body_pipeline_with_extra(prepared.coordinator(), extra)
        {
            return Err("cold Proposal continuation prepared census changed");
        }
        let coordinator = prepared
            .commit_with_registry(&mut *self.registry, payload_store, &mut recovery)
            .map_err(|_| "cold Proposal continuation coordinator commit failed")?;
        if !self
            .registry
            .exactly_covers_recovered_ready_work_with_extra(&coordinator, extra)
            || !recovery.owns_control_continuation_frame(&self.ledger)
        {
            return Err("cold Proposal continuation opened census changed");
        }
        Ok((coordinator, recovery))
    }
}
