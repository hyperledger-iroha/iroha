impl ConcreteLifecycleWorkRegistry {
    /// Borrow the still-pending adapter effect advertised by one lease slot.
    /// Closed carriers fail rather than re-executing their retained effects.
    pub(super) fn borrow_for_lease(
        &self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<&AdapterEffect, RegistryError> {
        let address = self.validated_lease_address(lease, slot)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated lease address remains present");
        if !work.is_pending_adapter() {
            return Err(RegistryError::WrongWorkKind);
        }
        Ok(work.effect())
    }
    /// Consume the complete still-pending adapter work advertised by one lease slot once.
    ///
    /// Returning the sealed pending authority together with the effect is
    /// essential: execution may report `Blocked` or `Replenished`, in which
    /// case a later atomic settlement must be able to restore the incumbent
    /// without reminting its causal binding. Closed-carrier consumption
    /// remains unavailable until its typed executor lands.
    pub(super) fn take_for_lease(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<ConcreteLifecycleWork, RegistryError> {
        let address = self.validated_lease_address(lease, slot)?;
        if !self
            .entries
            .get(&address)
            .expect("validated lease address remains present")
            .is_pending_adapter()
        {
            return Err(RegistryError::WrongWorkKind);
        }
        Ok(self
            .entries
            .remove(&address)
            .expect("validated lease address remains present"))
    }
    /// Remove only the exact digest installed by a failed outer transaction.
    pub(super) fn rollback_exact(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
    ) -> Result<ConcreteLifecycleWork, RegistryError> {
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if !work.validates_at(address) {
            return Err(RegistryError::CorruptWork);
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err(RegistryError::CausalOwnerMismatch);
        }
        if work.digest != expected_digest {
            return Err(RegistryError::DigestMismatch);
        }
        Ok(self
            .entries
            .remove(&address)
            .expect("validated rollback address remains present"))
    }
    fn validated_lease_address(
        &self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<ConcreteWorkAddress, RegistryError> {
        let address = ConcreteWorkAddress::new(lease.owner, lease.ordinal, slot)
            .ok_or(RegistryError::InvalidAddress)?;
        let expected_digest = lease
            .physical_slots
            .get(&slot)
            .ok_or(RegistryError::DigestMismatch)?;
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if !work.validates_at(address) {
            return Err(RegistryError::CorruptWork);
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err(RegistryError::CausalOwnerMismatch);
        }
        if work.digest != *expected_digest {
            return Err(RegistryError::DigestMismatch);
        }
        Ok(address)
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    #[cfg(test)]
    pub(super) fn certified_serve_and_producer_carrier_counts(&self) -> (usize, usize) {
        self.entries
            .values()
            .fold((0, 0), |counts, work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableCertifiedServe(_) => (counts.0 + 1, counts.1),
                ConcreteLifecycleWorkKind::DurableProducerTurn(_) => (counts.0, counts.1 + 1),
                _ => counts,
            })
    }
    #[cfg(test)]
    pub(super) fn one_certified_serve_pair_shares_replay_family(&self) -> bool {
        let serves = self
            .entries
            .values()
            .filter_map(|work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => Some(serve),
                _ => None,
            })
            .collect::<Vec<_>>();
        let producers = self
            .entries
            .values()
            .filter_map(|work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => Some(producer),
                _ => None,
            })
            .collect::<Vec<_>>();
        let ([serve], [producer]) = (serves.as_slice(), producers.as_slice()) else {
            return false;
        };
        Arc::ptr_eq(&serve.replay_evidence, &producer.replay_evidence)
    }
    #[cfg(test)]
    /// Remove one exact Serve carrier to exercise owner-private census faults.
    pub(super) fn remove_one_certified_serve_carrier_for_test(&mut self) -> bool {
        let address = self.entries.iter().find_map(|(address, work)| {
            matches!(
                &work.kind,
                ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            )
            .then_some(*address)
        });
        address.is_some_and(|address| self.entries.remove(&address).is_some())
    }
    #[cfg(test)]
    pub(super) fn exactly_contains(
        &self,
        address: ConcreteWorkAddress,
        effect: &AdapterEffect,
    ) -> bool {
        self.entries
            .get(&address)
            .is_some_and(|work| work.validates_at(address) && work.effect() == effect)
    }
}
