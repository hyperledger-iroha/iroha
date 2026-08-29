//! Bundle-level phase ordering for atomic private cross-dataspace settlement.
//!
//! Participant committees certify individual fixed-shape deltas, while this
//! coordinator enforces the global barrier: no Commit certificate is admitted
//! until every participant has a valid Prepare certificate, and no carrier is
//! produced until every participant has a valid Commit certificate.

use super::protocol::{
    PrivateSettlementProtocolErrorV1, private_settlement_phase_body_v1,
    private_settlement_prepared_bundle_digest_v1,
    private_settlement_reserved_prepared_bundle_digest_v1, validate_authority_cryptography_v1,
    verify_private_settlement_phase_certificate_v1, verify_private_settlement_receipt_v1,
};
use iroha_crypto::Hash;
use iroha_data_model::nexus::{
    ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1,
    PrivateSettlementAbortReasonV1, PrivateSettlementAbortReceiptV1,
    PrivateSettlementCommitBundleV1, PrivateSettlementCommitteeAuthorityV1,
    PrivateSettlementDeltaV1, PrivateSettlementLegReceiptV1, PrivateSettlementPhaseCertificateV1,
    PrivateSettlementPhaseV1, PrivateSettlementReceiptV1,
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Bundle-level private-settlement lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum PrivateSettlementBundleLifecycleV1 {
    /// Per-leg auditor evidence is still being collected.
    Collecting,
    /// Every leg has durable auditor evidence; Prepare QCs are being collected.
    Audited,
    /// Every leg has a valid Prepare QC; Commit QCs are being collected.
    Prepared,
    /// Every leg has a valid Commit QC and an atomic carrier may be built.
    CommitCertified,
    /// One verified receipt was applied atomically to global state.
    Finalized,
    /// A public reason-class marker terminated the bundle before application.
    Aborted,
    /// The globally committed expiry height passed before application.
    Expired,
}

/// Result of inserting phase evidence into the coordinator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrivateSettlementCoordinatorOutcomeV1 {
    /// New evidence was stored without completing the current global barrier.
    Stored,
    /// The supplied evidence was byte-for-byte identical to existing evidence.
    Idempotent,
    /// This insertion completed the current global barrier.
    BarrierCompleted,
}

/// Deterministic in-memory projection of the durable private-settlement journal.
#[derive(Clone, Debug)]
pub(crate) struct PrivateSettlementBundleCoordinatorV1 {
    manifest: AtomicPrivateSettlementV1,
    authority_catalog: Vec<PrivateSettlementCommitteeAuthorityV1>,
    audited_evidence: Vec<Option<Hash>>,
    deltas: Vec<Option<PrivateSettlementDeltaV1>>,
    prepare_certificates: Vec<Option<PrivateSettlementPhaseCertificateV1>>,
    prepared_bundle_digest: Option<Hash>,
    commit_certificates: Vec<Option<PrivateSettlementPhaseCertificateV1>>,
    lifecycle: PrivateSettlementBundleLifecycleV1,
    terminal_receipt: Option<PrivateSettlementReceiptV1>,
    abort_receipt: Option<PrivateSettlementAbortReceiptV1>,
}

impl PrivateSettlementBundleCoordinatorV1 {
    /// Construct one coordinator for the exact canonical manifest and authority catalog.
    pub(crate) fn new(
        manifest: AtomicPrivateSettlementV1,
        authority_catalog: Vec<PrivateSettlementCommitteeAuthorityV1>,
    ) -> Result<Self, PrivateSettlementCoordinatorErrorV1> {
        manifest
            .validate()
            .map_err(|_| PrivateSettlementCoordinatorErrorV1::Manifest)?;
        if authority_catalog.len() != manifest.legs.len() {
            return Err(PrivateSettlementCoordinatorErrorV1::Authority);
        }
        for (leg, authority) in manifest.legs.iter().zip(&authority_catalog) {
            validate_authority_cryptography_v1(authority)
                .map_err(|_| PrivateSettlementCoordinatorErrorV1::Authority)?;
            if authority.route != leg.route {
                return Err(PrivateSettlementCoordinatorErrorV1::Authority);
            }
        }
        let leg_count = manifest.legs.len();
        Ok(Self {
            manifest,
            authority_catalog,
            audited_evidence: vec![None; leg_count],
            deltas: vec![None; leg_count],
            prepare_certificates: vec![None; leg_count],
            prepared_bundle_digest: None,
            commit_certificates: vec![None; leg_count],
            lifecycle: PrivateSettlementBundleLifecycleV1::Collecting,
            terminal_receipt: None,
            abort_receipt: None,
        })
    }

    /// Current deterministic bundle lifecycle.
    pub(crate) const fn lifecycle(&self) -> PrivateSettlementBundleLifecycleV1 {
        self.lifecycle
    }

    /// Exact immutable public manifest.
    pub(crate) const fn manifest(&self) -> &AtomicPrivateSettlementV1 {
        &self.manifest
    }

    fn recompute_prepared_bundle_digest(
        &self,
    ) -> Result<Hash, PrivateSettlementCoordinatorErrorV1> {
        let deltas = self
            .deltas
            .iter()
            .map(|delta| {
                delta
                    .clone()
                    .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prepares = self
            .prepare_certificates
            .iter()
            .map(|certificate| {
                certificate
                    .clone()
                    .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)
            })
            .collect::<Result<Vec<_>, _>>()?;
        private_settlement_prepared_bundle_digest_v1(
            &self.manifest,
            &self.authority_catalog,
            &deltas,
            &prepares,
        )
        .map_err(PrivateSettlementCoordinatorErrorV1::from_protocol)
    }

    /// Exact complete-bundle digest that every Commit vote must bind.
    pub(crate) fn prepared_bundle_digest(
        &self,
    ) -> Result<Hash, PrivateSettlementCoordinatorErrorV1> {
        if !matches!(
            self.lifecycle,
            PrivateSettlementBundleLifecycleV1::Prepared
                | PrivateSettlementBundleLifecycleV1::CommitCertified
        ) {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        self.prepared_bundle_digest
            .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)
    }

    fn check_live_height(
        &mut self,
        authoritative_height: u64,
    ) -> Result<(), PrivateSettlementCoordinatorErrorV1> {
        if authoritative_height < self.manifest.authority_context_height {
            return Err(PrivateSettlementCoordinatorErrorV1::Height);
        }
        if authoritative_height > self.manifest.expiry_height {
            if !matches!(
                self.lifecycle,
                PrivateSettlementBundleLifecycleV1::Finalized
                    | PrivateSettlementBundleLifecycleV1::Aborted
            ) {
                self.lifecycle = PrivateSettlementBundleLifecycleV1::Expired;
            }
            return Err(PrivateSettlementCoordinatorErrorV1::Expired);
        }
        if matches!(
            self.lifecycle,
            PrivateSettlementBundleLifecycleV1::Finalized
                | PrivateSettlementBundleLifecycleV1::Aborted
                | PrivateSettlementBundleLifecycleV1::Expired
        ) {
            return Err(PrivateSettlementCoordinatorErrorV1::Terminal);
        }
        Ok(())
    }

    fn slot<T>(
        slots: &mut [Option<T>],
        ordinal: u8,
    ) -> Result<&mut Option<T>, PrivateSettlementCoordinatorErrorV1> {
        slots
            .get_mut(usize::from(ordinal))
            .ok_or(PrivateSettlementCoordinatorErrorV1::Binding)
    }

    /// Record the canonical digest of durable per-leg auditor approvals.
    pub(crate) fn record_audited(
        &mut self,
        leg_ordinal: u8,
        evidence_digest: Hash,
        authoritative_height: u64,
    ) -> Result<PrivateSettlementCoordinatorOutcomeV1, PrivateSettlementCoordinatorErrorV1> {
        self.check_live_height(authoritative_height)?;
        if evidence_digest == Hash::prehashed([0; Hash::LENGTH]) {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        let slot = Self::slot(&mut self.audited_evidence, leg_ordinal)?;
        if let Some(existing) = slot {
            return if *existing == evidence_digest {
                Ok(PrivateSettlementCoordinatorOutcomeV1::Idempotent)
            } else {
                Err(PrivateSettlementCoordinatorErrorV1::Substitution)
            };
        }
        if !matches!(
            self.lifecycle,
            PrivateSettlementBundleLifecycleV1::Collecting
                | PrivateSettlementBundleLifecycleV1::Audited
        ) {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        *slot = Some(evidence_digest);
        if self.audited_evidence.iter().all(Option::is_some) {
            self.lifecycle = PrivateSettlementBundleLifecycleV1::Audited;
            Ok(PrivateSettlementCoordinatorOutcomeV1::BarrierCompleted)
        } else {
            Ok(PrivateSettlementCoordinatorOutcomeV1::Stored)
        }
    }

    /// Record one valid Prepare QC and its exact fixed-shape delta.
    pub(crate) fn record_prepare(
        &mut self,
        delta: PrivateSettlementDeltaV1,
        certificate: PrivateSettlementPhaseCertificateV1,
        authoritative_height: u64,
    ) -> Result<PrivateSettlementCoordinatorOutcomeV1, PrivateSettlementCoordinatorErrorV1> {
        self.check_live_height(authoritative_height)?;
        if !matches!(
            self.lifecycle,
            PrivateSettlementBundleLifecycleV1::Audited
                | PrivateSettlementBundleLifecycleV1::Prepared
        ) {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        let ordinal = delta.leg_ordinal;
        let authority = self
            .authority_catalog
            .get(usize::from(ordinal))
            .ok_or(PrivateSettlementCoordinatorErrorV1::Binding)?;
        let expected = private_settlement_phase_body_v1(
            &self.manifest,
            &delta,
            authority,
            PrivateSettlementPhaseV1::Prepare,
            private_settlement_reserved_prepared_bundle_digest_v1(),
        )
        .map_err(PrivateSettlementCoordinatorErrorV1::from_protocol)?;
        if certificate.body != expected {
            return Err(PrivateSettlementCoordinatorErrorV1::Binding);
        }
        verify_private_settlement_phase_certificate_v1(&certificate, ordinal, authority)
            .map_err(PrivateSettlementCoordinatorErrorV1::from_protocol)?;
        if let Some(existing) = self.prepare_certificates[usize::from(ordinal)].as_ref() {
            return if existing == &certificate
                && self.deltas[usize::from(ordinal)].as_ref() == Some(&delta)
            {
                Ok(PrivateSettlementCoordinatorOutcomeV1::Idempotent)
            } else {
                Err(PrivateSettlementCoordinatorErrorV1::Substitution)
            };
        }
        if self.lifecycle != PrivateSettlementBundleLifecycleV1::Audited {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        self.deltas[usize::from(ordinal)] = Some(delta);
        self.prepare_certificates[usize::from(ordinal)] = Some(certificate);
        if self.prepare_certificates.iter().all(Option::is_some) {
            let prepared_bundle_digest = match self.recompute_prepared_bundle_digest() {
                Ok(digest) => digest,
                Err(error) => {
                    self.deltas[usize::from(ordinal)] = None;
                    self.prepare_certificates[usize::from(ordinal)] = None;
                    return Err(error);
                }
            };
            self.prepared_bundle_digest = Some(prepared_bundle_digest);
            self.lifecycle = PrivateSettlementBundleLifecycleV1::Prepared;
            Ok(PrivateSettlementCoordinatorOutcomeV1::BarrierCompleted)
        } else {
            Ok(PrivateSettlementCoordinatorOutcomeV1::Stored)
        }
    }

    /// Record one valid Commit QC after the all-Prepare global barrier.
    pub(crate) fn record_commit(
        &mut self,
        certificate: PrivateSettlementPhaseCertificateV1,
        authoritative_height: u64,
    ) -> Result<PrivateSettlementCoordinatorOutcomeV1, PrivateSettlementCoordinatorErrorV1> {
        self.check_live_height(authoritative_height)?;
        if !matches!(
            self.lifecycle,
            PrivateSettlementBundleLifecycleV1::Prepared
                | PrivateSettlementBundleLifecycleV1::CommitCertified
        ) {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        let ordinal = certificate.body.leg_ordinal;
        let index = usize::from(ordinal);
        let authority = self
            .authority_catalog
            .get(index)
            .ok_or(PrivateSettlementCoordinatorErrorV1::Binding)?;
        let delta = self
            .deltas
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)?;
        let prepared_bundle_digest = self
            .prepared_bundle_digest
            .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)?;
        let expected = private_settlement_phase_body_v1(
            &self.manifest,
            delta,
            authority,
            PrivateSettlementPhaseV1::Commit,
            prepared_bundle_digest,
        )
        .map_err(PrivateSettlementCoordinatorErrorV1::from_protocol)?;
        if certificate.body != expected {
            return Err(PrivateSettlementCoordinatorErrorV1::Binding);
        }
        verify_private_settlement_phase_certificate_v1(&certificate, ordinal, authority)
            .map_err(PrivateSettlementCoordinatorErrorV1::from_protocol)?;
        if let Some(existing) = self.commit_certificates[index].as_ref() {
            return if existing == &certificate {
                Ok(PrivateSettlementCoordinatorOutcomeV1::Idempotent)
            } else {
                Err(PrivateSettlementCoordinatorErrorV1::Substitution)
            };
        }
        if self.lifecycle != PrivateSettlementBundleLifecycleV1::Prepared {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        self.commit_certificates[index] = Some(certificate);
        if self.commit_certificates.iter().all(Option::is_some) {
            self.lifecycle = PrivateSettlementBundleLifecycleV1::CommitCertified;
            Ok(PrivateSettlementCoordinatorOutcomeV1::BarrierCompleted)
        } else {
            Ok(PrivateSettlementCoordinatorOutcomeV1::Stored)
        }
    }

    /// Build the sole pre-finality bundle admissible in a global carrier.
    ///
    /// `authoritative_height` proves that coordination is still live, but is
    /// deliberately not encoded: the eventual inclusion height is assigned by
    /// consensus when the carrier executes.
    pub(crate) fn carrier_bundle(
        &mut self,
        authoritative_height: u64,
        max_carrier_bytes: usize,
    ) -> Result<PrivateSettlementCommitBundleV1, PrivateSettlementCoordinatorErrorV1> {
        self.check_live_height(authoritative_height)?;
        if self.lifecycle != PrivateSettlementBundleLifecycleV1::CommitCertified {
            return Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder);
        }
        if self.prepared_bundle_digest != Some(self.recompute_prepared_bundle_digest()?) {
            return Err(PrivateSettlementCoordinatorErrorV1::Binding);
        }
        let legs = (0..self.manifest.legs.len())
            .map(|index| {
                Ok(PrivateSettlementLegReceiptV1 {
                    delta: self.deltas[index]
                        .clone()
                        .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)?,
                    prepare: self.prepare_certificates[index]
                        .clone()
                        .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)?,
                    commit: self.commit_certificates[index]
                        .clone()
                        .ok_or(PrivateSettlementCoordinatorErrorV1::PhaseOrder)?,
                })
            })
            .collect::<Result<Vec<_>, PrivateSettlementCoordinatorErrorV1>>()?;
        let bundle = PrivateSettlementCommitBundleV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            manifest: self.manifest.clone(),
            authority_catalog: self.authority_catalog.clone(),
            legs,
        };
        let validation_receipt = bundle.clone().into_receipt(authoritative_height);
        verify_private_settlement_receipt_v1(&validation_receipt)
            .map_err(PrivateSettlementCoordinatorErrorV1::from_protocol)?;
        let encoded = norito::encode_canonical(&bundle)
            .map_err(|_| PrivateSettlementCoordinatorErrorV1::Encoding)?;
        if max_carrier_bytes == 0 || encoded.len() > max_carrier_bytes {
            return Err(PrivateSettlementCoordinatorErrorV1::CarrierTooLarge);
        }
        Ok(bundle)
    }

    /// Mark a receipt finalized only after the global state transaction commits.
    pub(crate) fn record_finalized(
        &mut self,
        receipt: PrivateSettlementReceiptV1,
        max_carrier_bytes: usize,
    ) -> Result<PrivateSettlementCoordinatorOutcomeV1, PrivateSettlementCoordinatorErrorV1> {
        if self.lifecycle == PrivateSettlementBundleLifecycleV1::Finalized {
            return if self.terminal_receipt.as_ref() == Some(&receipt) {
                Ok(PrivateSettlementCoordinatorOutcomeV1::Idempotent)
            } else {
                Err(PrivateSettlementCoordinatorErrorV1::Substitution)
            };
        }
        let expected = self
            .carrier_bundle(receipt.finalized_height, max_carrier_bytes)?
            .into_receipt(receipt.finalized_height);
        if expected != receipt {
            return Err(PrivateSettlementCoordinatorErrorV1::Substitution);
        }
        self.terminal_receipt = Some(receipt);
        self.lifecycle = PrivateSettlementBundleLifecycleV1::Finalized;
        Ok(PrivateSettlementCoordinatorOutcomeV1::BarrierCompleted)
    }

    /// Abort or expire a non-finalized bundle and return its opaque public marker.
    pub(crate) fn terminate(
        &mut self,
        finalized_height: u64,
        reason: PrivateSettlementAbortReasonV1,
    ) -> Result<PrivateSettlementAbortReceiptV1, PrivateSettlementCoordinatorErrorV1> {
        if self.lifecycle == PrivateSettlementBundleLifecycleV1::Finalized {
            return Err(PrivateSettlementCoordinatorErrorV1::Terminal);
        }
        if let Some(existing) = self.abort_receipt.as_ref() {
            return if existing.finalized_height == finalized_height && existing.reason == reason {
                Ok(*existing)
            } else {
                Err(PrivateSettlementCoordinatorErrorV1::Substitution)
            };
        }
        if finalized_height < self.manifest.authority_context_height {
            return Err(PrivateSettlementCoordinatorErrorV1::Height);
        }
        let expired = finalized_height > self.manifest.expiry_height;
        if expired != (reason == PrivateSettlementAbortReasonV1::Expired) {
            return Err(PrivateSettlementCoordinatorErrorV1::Binding);
        }
        let receipt = PrivateSettlementAbortReceiptV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: self.manifest.network_id,
            bundle_id: self.manifest.bundle_id,
            manifest_digest: self
                .manifest
                .manifest_digest()
                .map_err(|_| PrivateSettlementCoordinatorErrorV1::Encoding)?,
            finalized_height,
            reason,
        };
        receipt
            .validate()
            .map_err(|_| PrivateSettlementCoordinatorErrorV1::Binding)?;
        self.lifecycle = if expired {
            PrivateSettlementBundleLifecycleV1::Expired
        } else {
            PrivateSettlementBundleLifecycleV1::Aborted
        };
        self.abort_receipt = Some(receipt);
        Ok(receipt)
    }
}

/// Redacted bundle-coordination failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum PrivateSettlementCoordinatorErrorV1 {
    /// The manifest is not the canonical V1 bundle.
    #[error("private-settlement manifest is invalid")]
    Manifest,
    /// The authority catalog does not match the canonical participant list.
    #[error("private-settlement authority catalog is invalid")]
    Authority,
    /// The authoritative height precedes the manifest context.
    #[error("private-settlement height context is invalid")]
    Height,
    /// The bundle has passed its globally committed expiry.
    #[error("private-settlement bundle expired")]
    Expired,
    /// The bundle is already in a terminal state.
    #[error("private-settlement bundle is terminal")]
    Terminal,
    /// Evidence was supplied before the required global barrier.
    #[error("private-settlement phase order is invalid")]
    PhaseOrder,
    /// A route, ordinal, digest, phase, or receipt binding differs.
    #[error("private-settlement evidence binding is invalid")]
    Binding,
    /// Existing evidence was replaced rather than replayed identically.
    #[error("private-settlement evidence substitution was rejected")]
    Substitution,
    /// A participant certificate or authority proof is invalid.
    #[error("private-settlement certificate is invalid")]
    Certificate,
    /// Canonical receipt encoding failed.
    #[error("private-settlement carrier encoding failed")]
    Encoding,
    /// The encoded carrier exceeds the configured admission bound.
    #[error("private-settlement carrier exceeds the configured byte limit")]
    CarrierTooLarge,
}

impl PrivateSettlementCoordinatorErrorV1 {
    fn from_protocol(error: PrivateSettlementProtocolErrorV1) -> Self {
        match error {
            PrivateSettlementProtocolErrorV1::CanonicalEncoding => Self::Encoding,
            PrivateSettlementProtocolErrorV1::Authority => Self::Authority,
            PrivateSettlementProtocolErrorV1::Binding => Self::Binding,
            PrivateSettlementProtocolErrorV1::Vote
            | PrivateSettlementProtocolErrorV1::Quorum
            | PrivateSettlementProtocolErrorV1::Certificate
            | PrivateSettlementProtocolErrorV1::Receipt => Self::Certificate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_settlement::{
        protocol::{
            aggregate_private_settlement_phase_votes_v1, sign_private_settlement_phase_vote_v1,
        },
        sidecar_store::tests::{SidecarFixtureV1, sidecar_fixture},
    };
    use iroha_crypto::{HashOf, KeyPair};
    use iroha_data_model::peer::PeerId;

    fn fixture_parts() -> (
        SidecarFixtureV1,
        Vec<PrivateSettlementDeltaV1>,
        Vec<PrivateSettlementCommitteeAuthorityV1>,
    ) {
        let fixture = sidecar_fixture();
        let first = fixture.sidecar.payload.delta.clone();
        let mut second = first.clone();
        second.leg_ordinal = 1;
        second.route = fixture.sidecar.manifest.legs[1].route;
        second.pool_id = fixture.sidecar.manifest.legs[1].pool_id;
        second.asset_binding_commitment = fixture.sidecar.manifest.legs[1].asset_binding_commitment;
        second.audit_policy_digest = fixture.sidecar.manifest.legs[1].audit_policy_digest;
        assert_eq!(
            second.digest().expect("second delta digest"),
            fixture.sidecar.manifest.legs[1].delta_digest
        );
        let first_authority = fixture.sidecar.authority.clone();
        let validators = fixture
            .validator_keys
            .iter()
            .map(|key| PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let second_authority = PrivateSettlementCommitteeAuthorityV1 {
            route: second.route,
            validator_set_hash: HashOf::new(&validators),
            validators,
            validator_pops: fixture
                .validator_keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP")
                })
                .collect(),
        };
        (
            fixture,
            vec![first, second],
            vec![first_authority, second_authority],
        )
    }

    fn certificate(
        manifest: &AtomicPrivateSettlementV1,
        delta: &PrivateSettlementDeltaV1,
        authority: &PrivateSettlementCommitteeAuthorityV1,
        keys: &[KeyPair],
        phase: PrivateSettlementPhaseV1,
        prepared_bundle_digest: Hash,
    ) -> PrivateSettlementPhaseCertificateV1 {
        let body = private_settlement_phase_body_v1(
            manifest,
            delta,
            authority,
            phase,
            prepared_bundle_digest,
        )
        .expect("phase body");
        let votes = keys[..3]
            .iter()
            .map(|key| sign_private_settlement_phase_vote_v1(body, key).expect("phase vote"))
            .collect::<Vec<_>>();
        aggregate_private_settlement_phase_votes_v1(body, delta.leg_ordinal, authority, &votes)
            .expect("phase certificate")
    }

    #[test]
    fn all_prepare_and_commit_barriers_precede_finalization() {
        let (fixture, deltas, authorities) = fixture_parts();
        let manifest = fixture.sidecar.manifest.clone();
        let mut coordinator =
            PrivateSettlementBundleCoordinatorV1::new(manifest.clone(), authorities.clone())
                .expect("coordinator");
        assert_eq!(coordinator.manifest(), &manifest);
        assert_eq!(
            coordinator.record_audited(0, Hash::new(b"audit-0"), 12),
            Ok(PrivateSettlementCoordinatorOutcomeV1::Stored)
        );
        assert_eq!(
            coordinator.record_audited(1, Hash::new(b"audit-1"), 12),
            Ok(PrivateSettlementCoordinatorOutcomeV1::BarrierCompleted)
        );
        let early_commit = certificate(
            &manifest,
            &deltas[0],
            &authorities[0],
            &fixture.validator_keys,
            PrivateSettlementPhaseV1::Commit,
            Hash::new(b"early incomplete Prepare barrier"),
        );
        assert_eq!(
            coordinator.record_commit(early_commit, 12),
            Err(PrivateSettlementCoordinatorErrorV1::PhaseOrder)
        );
        for index in 0..2 {
            let prepare = certificate(
                &manifest,
                &deltas[index],
                &authorities[index],
                &fixture.validator_keys,
                PrivateSettlementPhaseV1::Prepare,
                private_settlement_reserved_prepared_bundle_digest_v1(),
            );
            let outcome = coordinator
                .record_prepare(deltas[index].clone(), prepare, 12)
                .expect("prepare");
            assert_eq!(
                outcome,
                if index == 1 {
                    PrivateSettlementCoordinatorOutcomeV1::BarrierCompleted
                } else {
                    PrivateSettlementCoordinatorOutcomeV1::Stored
                }
            );
        }
        assert_eq!(
            coordinator.lifecycle(),
            PrivateSettlementBundleLifecycleV1::Prepared
        );
        let prepared_bundle_digest = coordinator
            .prepared_bundle_digest()
            .expect("complete Prepare barrier digest");
        let wrong_bundle_commit = certificate(
            &manifest,
            &deltas[0],
            &authorities[0],
            &fixture.validator_keys,
            PrivateSettlementPhaseV1::Commit,
            Hash::new(b"substituted complete Prepare barrier"),
        );
        assert_eq!(
            coordinator.record_commit(wrong_bundle_commit, 13),
            Err(PrivateSettlementCoordinatorErrorV1::Binding)
        );
        for index in 0..2 {
            let commit = certificate(
                &manifest,
                &deltas[index],
                &authorities[index],
                &fixture.validator_keys,
                PrivateSettlementPhaseV1::Commit,
                prepared_bundle_digest,
            );
            coordinator.record_commit(commit, 13).expect("commit");
        }
        let receipt = coordinator
            .carrier_bundle(14, 4 * 1024 * 1024)
            .expect("carrier bundle")
            .into_receipt(14);
        assert_eq!(
            coordinator.record_finalized(receipt.clone(), 4 * 1024 * 1024),
            Ok(PrivateSettlementCoordinatorOutcomeV1::BarrierCompleted)
        );
        assert_eq!(
            coordinator.record_finalized(receipt, 4 * 1024 * 1024),
            Ok(PrivateSettlementCoordinatorOutcomeV1::Idempotent)
        );
        assert_eq!(
            coordinator.lifecycle(),
            PrivateSettlementBundleLifecycleV1::Finalized
        );
    }

    #[test]
    fn substitution_expiry_and_carrier_bounds_fail_closed() {
        let (fixture, deltas, authorities) = fixture_parts();
        let manifest = fixture.sidecar.manifest.clone();
        let mut coordinator =
            PrivateSettlementBundleCoordinatorV1::new(manifest.clone(), authorities.clone())
                .expect("coordinator");
        coordinator
            .record_audited(0, Hash::new(b"audit-0"), 12)
            .expect("audit");
        assert_eq!(
            coordinator.record_audited(0, Hash::new(b"substitution"), 12),
            Err(PrivateSettlementCoordinatorErrorV1::Substitution)
        );
        coordinator
            .record_audited(1, Hash::new(b"audit-1"), 12)
            .expect("audit");
        for index in 0..2 {
            let prepare = certificate(
                &manifest,
                &deltas[index],
                &authorities[index],
                &fixture.validator_keys,
                PrivateSettlementPhaseV1::Prepare,
                private_settlement_reserved_prepared_bundle_digest_v1(),
            );
            coordinator
                .record_prepare(deltas[index].clone(), prepare, 12)
                .expect("prepare");
        }
        let prepared_bundle_digest = coordinator
            .prepared_bundle_digest()
            .expect("complete Prepare barrier digest");
        for index in 0..2 {
            let commit = certificate(
                &manifest,
                &deltas[index],
                &authorities[index],
                &fixture.validator_keys,
                PrivateSettlementPhaseV1::Commit,
                prepared_bundle_digest,
            );
            coordinator.record_commit(commit, 13).expect("commit");
        }
        assert_eq!(
            coordinator.carrier_bundle(14, 1),
            Err(PrivateSettlementCoordinatorErrorV1::CarrierTooLarge)
        );

        let mut expiring = PrivateSettlementBundleCoordinatorV1::new(manifest.clone(), authorities)
            .expect("coordinator");
        let abort = expiring
            .terminate(
                manifest.expiry_height + 1,
                PrivateSettlementAbortReasonV1::Expired,
            )
            .expect("expiry receipt");
        assert_eq!(abort.reason, PrivateSettlementAbortReasonV1::Expired);
        assert_eq!(
            expiring.lifecycle(),
            PrivateSettlementBundleLifecycleV1::Expired
        );
    }
}
