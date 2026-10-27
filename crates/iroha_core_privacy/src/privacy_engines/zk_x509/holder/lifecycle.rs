//! Consuming local holder attempts over authenticated State and exact signed material.
//!
//! The caller obtains current authenticated State, trusted chain time and the
//! network genesis at every phase. Local Unix-millisecond inputs are explicit
//! estimates, not clocks or a source of chain authority. No phase updates the
//! original statement, substitutes a CRL, or extends its signed deadline.

use super::{ZkX509EngineErrorV1, ZkX509WitnessV1};
use crate::{
    privacy_engines::zk_x509::{
        certificate::{parse_certificate_v1, parse_crl_v1},
        credential_stark::ZkX509CredentialPublicBindingV1,
        engine::prepare_zk_x509_prover_input_v1,
    },
    privacy_state::{
        PrivacyZkX509AuthoritativeStateV1, validate_privacy_zk_x509_statement_state_v1,
    },
};
use iroha_data_model::privacy::{
    IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1, PrivacyZkX509CertificateValidityV1,
    PrivacyZkX509CrlUpdateIntervalV1, PrivacyZkX509HolderBudgetV1,
    PrivacyZkX509HolderScheduleErrorV1, PrivacyZkX509HolderScheduleOutcomeV1,
    PrivacyZkX509HolderScheduleV1,
};
use rand::TryCryptoRng;
use std::fmt;
use thiserror::Error;
use zeroize::Zeroizing;

/// A local attempt failed before it could release a credential proof.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum ZkX509HolderAttemptErrorV1 {
    /// Exact native network, State, witness or credential-proof validation failed.
    #[error(transparent)]
    Engine(#[from] ZkX509EngineErrorV1),
    /// The signed intervals or checked local estimates are invalid.
    #[error(transparent)]
    Schedule(#[from] PrivacyZkX509HolderScheduleErrorV1),
    /// A supplied local phase time precedes the previous phase's time.
    #[error("X.509 holder local time moved backwards")]
    LocalTimeMovedBackwards,
    /// The original deadline cannot accommodate the remaining explicit budget.
    #[error("X.509 holder deadline cannot accommodate the remaining budget")]
    DeadlineExceeded,
}

struct SignedTimes {
    certificates: [PrivacyZkX509CertificateValidityV1; 3],
    chain_len: usize,
    crl: PrivacyZkX509CrlUpdateIntervalV1,
}

impl zeroize::Zeroize for SignedTimes {
    fn zeroize(&mut self) {
        self.certificates.zeroize();
        self.crl.this_update_unix_seconds.zeroize();
        self.crl.next_update_unix_seconds.zeroize();
        self.chain_len.zeroize();
        #[cfg(test)]
        tests::record_timing_clear(self);
    }
}

impl SignedTimes {
    // This is called only after the real engine preflight has authenticated the
    // exact codec bytes, complete path/CRL signatures and governance bindings.
    fn from_validated_witness(
        witness: &ZkX509WitnessV1,
    ) -> Result<Zeroizing<Self>, ZkX509HolderAttemptErrorV1> {
        // Establish the clearing owner before parsing or retaining any private
        // timing metadata. Partial certificate/CRL errors clear the same owner.
        let mut owned = Zeroizing::new(Self {
            certificates: [PrivacyZkX509CertificateValidityV1::new(0, 0); 3],
            chain_len: 0,
            crl: PrivacyZkX509CrlUpdateIntervalV1::new(0, 0),
        });
        if !(2..=3).contains(&witness.certificate_chain_der.len()) {
            return Err(ZkX509EngineErrorV1::ReferenceRelation(
                super::ZkX509RelationErrorV1::WitnessMismatch,
            )
            .into());
        }
        owned.chain_len = witness.certificate_chain_der.len();
        for (destination, der) in owned
            .certificates
            .iter_mut()
            .zip(&witness.certificate_chain_der)
        {
            let certificate = parse_certificate_v1(der).map_err(ZkX509EngineErrorV1::from)?;
            *destination = PrivacyZkX509CertificateValidityV1::new(
                certificate.not_before,
                certificate.not_after,
            );
        }
        let crl = parse_crl_v1(&witness.crl_der).map_err(ZkX509EngineErrorV1::from)?;
        owned.crl = PrivacyZkX509CrlUpdateIntervalV1::new(crl.this_update, crl.next_update);
        Ok(owned)
    }

    fn require_budget(
        &self,
        original: &IrohaZkX509StarkP256StatementV1,
        budget: PrivacyZkX509HolderBudgetV1,
        local_start_unix_ms: u64,
    ) -> Result<(), ZkX509HolderAttemptErrorV1> {
        let schedule = PrivacyZkX509HolderScheduleV1::new(
            original,
            &self.certificates[..self.chain_len],
            self.crl,
            budget,
        )?;
        // State validation, not this equality comparison, authenticates the
        // current selection. The owned original is never rebound to a successor.
        match schedule.assess(original, local_start_unix_ms)? {
            PrivacyZkX509HolderScheduleOutcomeV1::WithinBudget => Ok(()),
            _ => Err(ZkX509HolderAttemptErrorV1::DeadlineExceeded),
        }
    }
}

fn validate_current_phase(
    original: &IrohaZkX509StarkP256StatementV1,
    current_state: &PrivacyZkX509AuthoritativeStateV1,
    trusted_block_timestamp_ms: u64,
    consensus_limits: &PrivacyConsensusLimitsV1,
    genesis_hash: [u8; 32],
) -> Result<(), ZkX509HolderAttemptErrorV1> {
    ZkX509CredentialPublicBindingV1::from_consensus_context_v1(original, genesis_hash)
        .map_err(ZkX509EngineErrorV1::from)?;
    validate_privacy_zk_x509_statement_state_v1(
        original,
        current_state,
        trusted_block_timestamp_ms,
        consensus_limits,
    )
    .map_err(ZkX509EngineErrorV1::InvalidAuthoritativeState)?;
    Ok(())
}

fn require_forward_local_time(
    previous: u64,
    current: u64,
) -> Result<(), ZkX509HolderAttemptErrorV1> {
    if current < previous {
        return Err(ZkX509HolderAttemptErrorV1::LocalTimeMovedBackwards);
    }
    Ok(())
}

/// One immutable statement and owned witness after real native preflight.
///
/// Preparation checks all six budget terms. Proving consumes this attempt and
/// checks only the remaining proving, publication, submission and clock terms.
/// The caller must supply newly authenticated current State at each phase;
/// this local owner cannot determine whether a caller has reused an old snapshot.
/// A failed or revoked State lookup must stop the caller before proving/release.
/// Idle-chain wall-clock freshness, State acquisition and transport are external.
pub struct ZkX509HolderAttemptV1 {
    original: IrohaZkX509StarkP256StatementV1,
    witness: ZkX509WitnessV1,
    times: Zeroizing<SignedTimes>,
    budget: PrivacyZkX509HolderBudgetV1,
    local_start_unix_ms: u64,
}

impl fmt::Debug for ZkX509HolderAttemptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ZkX509HolderAttemptV1 { [REDACTED] }")
    }
}

impl ZkX509HolderAttemptV1 {
    /// Consume one exact signed attempt and assess the full initial estimate.
    ///
    /// # Errors
    ///
    /// Rejects invalid network/State, codec or signed relation inputs before
    /// deriving private timing bounds; then rejects invalid or infeasible budgets.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        original: IrohaZkX509StarkP256StatementV1,
        witness: ZkX509WitnessV1,
        current_state: &PrivacyZkX509AuthoritativeStateV1,
        trusted_block_timestamp_ms: u64,
        consensus_limits: &PrivacyConsensusLimitsV1,
        genesis_hash: [u8; 32],
        budget: PrivacyZkX509HolderBudgetV1,
        local_start_unix_ms: u64,
    ) -> Result<Self, ZkX509HolderAttemptErrorV1> {
        validate_current_phase(
            &original,
            current_state,
            trusted_block_timestamp_ms,
            consensus_limits,
            genesis_hash,
        )?;
        let encoded = Zeroizing::new(witness.encode_v1().map_err(ZkX509EngineErrorV1::from)?);
        let prepared = prepare_zk_x509_prover_input_v1(
            &original,
            current_state,
            trusted_block_timestamp_ms,
            consensus_limits,
            &encoded,
        )?;
        let times = SignedTimes::from_validated_witness(prepared.witness())?;
        times.require_budget(&original, budget, local_start_unix_ms)?;
        Ok(Self {
            original,
            witness,
            times,
            budget,
            local_start_unix_ms,
        })
    }

    /// Borrow the exact original public statement without allowing mutation.
    #[must_use]
    pub const fn original(&self) -> &IrohaZkX509StarkP256StatementV1 {
        &self.original
    }

    /// Consume the attempt, revalidate it and invoke the complete native prover.
    ///
    /// The observed local time must not move backwards. Completed observation
    /// and preparation are not charged again; the remaining four terms must fit
    /// the original signed/public deadline before proof entropy is consumed.
    /// Successful proof bytes remain opaque until a separate release check.
    ///
    /// # Errors
    ///
    /// Rejects stale, revoked or expired selection, a wrong network, backwards
    /// local time, an exhausted remaining budget or any native prover failure.
    #[allow(clippy::too_many_arguments)]
    pub fn prove_with_rng<R: TryCryptoRng>(
        self,
        current_state: &PrivacyZkX509AuthoritativeStateV1,
        trusted_block_timestamp_ms: u64,
        consensus_limits: &PrivacyConsensusLimitsV1,
        genesis_hash: [u8; 32],
        local_proving_start_unix_ms: u64,
        rng: &mut R,
    ) -> Result<ZkX509PendingCredentialProofV1, ZkX509HolderAttemptErrorV1> {
        require_forward_local_time(self.local_start_unix_ms, local_proving_start_unix_ms)?;
        validate_current_phase(
            &self.original,
            current_state,
            trusted_block_timestamp_ms,
            consensus_limits,
            genesis_hash,
        )?;
        let mut remaining = self.budget;
        remaining.observation_ms = 0;
        remaining.preparation_ms = 0;
        self.times
            .require_budget(&self.original, remaining, local_proving_start_unix_ms)?;
        let proof = super::prove_credential_v1_with_rng(
            &self.original,
            current_state,
            trusted_block_timestamp_ms,
            consensus_limits,
            genesis_hash,
            self.witness,
            rng,
        )?;
        Ok(ZkX509PendingCredentialProofV1 {
            original: self.original,
            proof,
            times: self.times,
            budget: remaining,
            local_proving_start_unix_ms,
        })
    }
}

/// A self-verified native proof whose original attempt still needs a fresh State check.
///
/// No proof bytes, clone, serialization or public constructor are exposed. A
/// release failure consumes this object. The caller must check again if time or
/// State changes between release and actual submission; ordinary admission remains
/// authoritative. This API does not submit transactions or authorize replay.
pub struct ZkX509PendingCredentialProofV1 {
    original: IrohaZkX509StarkP256StatementV1,
    proof: Vec<u8>,
    times: Zeroizing<SignedTimes>,
    budget: PrivacyZkX509HolderBudgetV1,
    local_proving_start_unix_ms: u64,
}

impl fmt::Debug for ZkX509PendingCredentialProofV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ZkX509PendingCredentialProofV1 { [REDACTED] }")
    }
}

impl ZkX509PendingCredentialProofV1 {
    /// Borrow the exact original public statement; proof bytes remain private.
    #[must_use]
    pub const fn original(&self) -> &IrohaZkX509StarkP256StatementV1 {
        &self.original
    }

    /// Consume the pending proof after revalidating its original selection.
    ///
    /// Only publication jitter, submission/inclusion and clock margin remain.
    /// The exact original statement and proof are returned together; no window,
    /// governance binding or proof byte is rewritten. Equality at the final
    /// admissible millisecond is permitted by the canonical schedule.
    ///
    /// # Errors
    ///
    /// Rejects stale or expired State, changed network/genesis, backwards local
    /// time or a remaining estimate that exceeds the original signed deadline.
    #[allow(clippy::too_many_arguments)]
    pub fn release(
        self,
        current_state: &PrivacyZkX509AuthoritativeStateV1,
        trusted_block_timestamp_ms: u64,
        consensus_limits: &PrivacyConsensusLimitsV1,
        genesis_hash: [u8; 32],
        local_release_unix_ms: u64,
    ) -> Result<(IrohaZkX509StarkP256StatementV1, Vec<u8>), ZkX509HolderAttemptErrorV1> {
        require_forward_local_time(self.local_proving_start_unix_ms, local_release_unix_ms)?;
        validate_current_phase(
            &self.original,
            current_state,
            trusted_block_timestamp_ms,
            consensus_limits,
            genesis_hash,
        )?;
        let mut remaining = self.budget;
        remaining.proving_ms = 0;
        self.times
            .require_budget(&self.original, remaining, local_release_unix_ms)?;
        Ok((self.original, self.proof))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize as _;

    std::thread_local! {
        static TIMING_CLEAR_EVENTS: std::cell::RefCell<Vec<bool>> = const {
            std::cell::RefCell::new(Vec::new())
        };
    }

    fn timing_is_clear(times: &SignedTimes) -> bool {
        times.certificates.iter().all(|validity| {
            validity.not_before_unix_seconds() == 0 && validity.not_after_unix_seconds() == 0
        }) && times.crl.this_update_unix_seconds == 0
            && times.crl.next_update_unix_seconds == 0
            && times.chain_len == 0
    }

    pub(super) fn record_timing_clear(times: &SignedTimes) {
        // Retain only a post-clear Boolean, never a private date or freed pointer.
        TIMING_CLEAR_EVENTS.with(|events| events.borrow_mut().push(timing_is_clear(times)));
    }

    #[test]
    fn timing_owner_clears_all_fields_on_explicit_clear_partial_parse_and_unwind() {
        let fixture = build_zk_x509_reference_fixture_v1().unwrap();
        TIMING_CLEAR_EVENTS.with(|events| events.borrow_mut().clear());
        {
            let mut owned = SignedTimes::from_validated_witness(&fixture.witness).unwrap();
            assert!(!timing_is_clear(&owned));
            owned.zeroize();
            assert!(timing_is_clear(&owned));
        }
        assert_eq!(
            TIMING_CLEAR_EVENTS.with(|events| events.borrow().clone()),
            [true, true]
        );
        for corrupt_crl in [false, true] {
            TIMING_CLEAR_EVENTS.with(|events| events.borrow_mut().clear());
            let mut witness = fixture.witness.clone();
            if corrupt_crl {
                witness.crl_der[0] = 0;
            } else {
                // The first certificate has already populated private slots.
                witness.certificate_chain_der[1][0] = 0;
            }
            assert!(SignedTimes::from_validated_witness(&witness).is_err());
            assert_eq!(
                TIMING_CLEAR_EVENTS.with(|events| events.borrow().clone()),
                [true]
            );
        }
        TIMING_CLEAR_EVENTS.with(|events| events.borrow_mut().clear());
        assert!(
            std::panic::catch_unwind(|| {
                let _owned = SignedTimes::from_validated_witness(&fixture.witness).unwrap();
                panic!("timing owner unwind control");
            })
            .is_err()
        );
        assert_eq!(
            TIMING_CLEAR_EVENTS.with(|events| events.borrow().clone()),
            [true]
        );
    }

    use crate::privacy_engines::zk_x509::relation::release_fixture::{
        ZkX509ReleaseFixtureV1, build_zk_x509_network_release_successor_fixture_v1,
        build_zk_x509_reference_fixture_v1,
    };
    use crate::privacy_state::{
        PrivacyCommitmentKeyV1, PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1, PrivacyRootKeyV1,
        PrivacyRootProvenanceV1, PrivacyStateItemRecordV1, PrivacyZkX509AuthoritativeStateV1,
        load_privacy_zk_x509_authoritative_state_v1, privacy_zk_x509_ca_namespace_v1,
    };
    use iroha_data_model::privacy::{
        PrivacyConsensusLimitsV1, PrivacyPolicyDigestV1, PrivacyRootPublicationV1,
        PrivacyRootRoleV1, PrivacyRootV1, PrivacyX509TrustStoreDigestV1,
        PrivacyZkX509CertificatePolicyRecordV1, PrivacyZkX509CrlRecordV1,
        PrivacyZkX509RecordLifecycleV1, PrivacyZkX509TrustAnchorRecordV1,
        validate_zk_x509_certificate_policy_rotation_v1, validate_zk_x509_crl_revocation_v1,
        validate_zk_x509_trust_anchor_rotation_v1,
    };
    use mv::storage::Storage;

    // This reconstructs a real accepted State snapshot, including the complete
    // anchor/policy lineage and corresponding retained CA-root history. It never
    // constructs PrivacyZkX509AuthoritativeStateV1 through private fields.
    fn joined_state(
        anchors: &[PrivacyZkX509TrustAnchorRecordV1],
        policies: &[PrivacyZkX509CertificatePolicyRecordV1],
        crl: PrivacyZkX509CrlRecordV1,
    ) -> Result<PrivacyZkX509AuthoritativeStateV1, String> {
        let mut commitments = Storage::<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>::new();
        let mut roots = Storage::<PrivacyRootKeyV1, PrivacyRootProvenanceV1>::new();
        let mut root_heads = Storage::<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>::new();
        for anchor in anchors {
            commitments.insert(
                PrivacyCommitmentKeyV1::zk_x509_trust_anchor_revision(
                    anchor.trust_anchor_id,
                    anchor.record_epoch,
                )
                .expect("anchor key"),
                PrivacyStateItemRecordV1::zk_x509_trust_anchor_governance(
                    *anchor,
                    anchor.record_epoch,
                )
                .expect("anchor State record"),
            );
            // A revoked anchor keeps its old CA root and does not publish another.
            if anchor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
                continue;
            }
            let namespace =
                privacy_zk_x509_ca_namespace_v1(anchor.trust_anchor_id).expect("CA namespace");
            let role = PrivacyRootRoleV1::CertificateAuthorityMembership;
            let key = PrivacyRootKeyV1::new(
                namespace,
                role,
                anchor.ca_membership_root_epoch,
                anchor.ca_membership_root,
            )
            .expect("CA root key");
            let publication =
                PrivacyRootPublicationV1::new(namespace, role, key.epoch(), key.root())
                    .expect("CA publication");
            let provenance = PrivacyRootProvenanceV1::zk_x509_ca_governance(
                publication.digest().expect("CA publication digest"),
                namespace,
                key.epoch(),
                key.root(),
                *anchor,
                anchor.record_epoch,
            )
            .expect("CA governance provenance");
            roots.insert(key, provenance.clone());
            root_heads.insert(
                PrivacyRootHeadKeyV1::new(namespace, role).expect("CA head key"),
                PrivacyRootHeadRecordV1::new(key.epoch(), key.root(), provenance, None, 1)
                    .expect("CA head"),
            );
        }
        for policy in policies {
            commitments.insert(
                PrivacyCommitmentKeyV1::zk_x509_certificate_policy_revision(
                    policy.trust_anchor_id,
                    policy.policy_id,
                    policy.record_epoch,
                )
                .expect("policy key"),
                PrivacyStateItemRecordV1::zk_x509_certificate_policy_governance(
                    policy.clone(),
                    policy.record_epoch,
                )
                .expect("policy State record"),
            );
        }
        commitments.insert(
            PrivacyCommitmentKeyV1::zk_x509_crl_current(
                crl.trust_anchor_id,
                crl.certificate_policy_id,
            )
            .expect("current CRL key"),
            PrivacyStateItemRecordV1::zk_x509_crl_governance(crl, crl.record_epoch)
                .expect("current CRL State record"),
        );
        load_privacy_zk_x509_authoritative_state_v1(
            crl.trust_anchor_id,
            crl.certificate_policy_id,
            PrivacyConsensusLimitsV1::taira_default().retained_root_count,
            &commitments.view(),
            &roots.view(),
            &root_heads.view(),
        )
    }

    fn successor_policy(
        current: &PrivacyZkX509CertificatePolicyRecordV1,
    ) -> PrivacyZkX509CertificatePolicyRecordV1 {
        let mut digest = *current.policy_digest.as_bytes();
        digest[0] ^= 1;
        let next = PrivacyZkX509CertificatePolicyRecordV1::new(
            current.trust_anchor_id,
            current.policy_id,
            current.record_epoch + 1,
            PrivacyPolicyDigestV1::new(digest),
            current.required_key_usage,
            current.required_extended_key_usages.clone(),
            current.required_disclosed_attribute_indices.clone(),
            Some(current.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("successor policy record");
        validate_zk_x509_certificate_policy_rotation_v1(current, &next)
            .expect("valid policy transition");
        next
    }

    fn successor_anchor(
        current: PrivacyZkX509TrustAnchorRecordV1,
    ) -> PrivacyZkX509TrustAnchorRecordV1 {
        // Changed governance commitments are deliberate negative-test input. A proof
        // for the old signed fixture must fail before attempting membership/proving.
        let mut store = *current.trust_store_digest.as_bytes();
        let mut root = *current.ca_membership_root.as_bytes();
        store[0] ^= 1;
        root[0] ^= 1;
        let next = PrivacyZkX509TrustAnchorRecordV1::new(
            current.trust_anchor_id,
            current.record_epoch + 1,
            PrivacyX509TrustStoreDigestV1::new(store),
            PrivacyRootV1::new(root),
            current.ca_membership_root_epoch + 1,
            Some(current.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("successor anchor record");
        validate_zk_x509_trust_anchor_rotation_v1(&current, &next)
            .expect("valid anchor transition");
        next
    }

    fn revoked_crl(current: PrivacyZkX509CrlRecordV1) -> PrivacyZkX509CrlRecordV1 {
        let revoked = PrivacyZkX509CrlRecordV1::new(
            current.trust_anchor_id,
            current.certificate_policy_id,
            current.record_epoch + 1,
            current.crl_number,
            current.crl_der_digest,
            current.issuer_spki_digest,
            current.this_update_unix_seconds,
            current.next_update_unix_seconds,
            Some(current.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        )
        .expect("revoked CRL record");
        validate_zk_x509_crl_revocation_v1(&current, &revoked)
            .expect("valid CRL revocation transition");
        revoked
    }

    #[derive(Default)]
    struct NoEntropy {
        requests: usize,
    }
    impl rand::TryRngCore for NoEntropy {
        type Error = &'static str;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            self.requests += 1;
            Err("invalid attempt must not request entropy")
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            self.requests += 1;
            Err("invalid attempt must not request entropy")
        }
        fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), Self::Error> {
            self.requests += 1;
            Err("invalid attempt must not request entropy")
        }
    }
    impl rand::TryCryptoRng for NoEntropy {}

    fn budget() -> PrivacyZkX509HolderBudgetV1 {
        PrivacyZkX509HolderBudgetV1 {
            observation_ms: 100,
            preparation_ms: 100,
            proving_ms: 100,
            publication_jitter_ms: 100,
            submission_ms: 100,
            clock_margin_ms: 100,
        }
    }
    fn start(fixture: &ZkX509ReleaseFixtureV1) -> u64 {
        fixture.statement.presentation_not_before_unix_seconds * 1_000
    }
    fn deadline(fixture: &ZkX509ReleaseFixtureV1) -> u64 {
        (fixture.statement.presentation_not_after_unix_seconds + 1) * 1_000 - 1
    }
    fn prepare(
        fixture: &ZkX509ReleaseFixtureV1,
        estimate: PrivacyZkX509HolderBudgetV1,
    ) -> Result<ZkX509HolderAttemptV1, ZkX509HolderAttemptErrorV1> {
        ZkX509HolderAttemptV1::prepare(
            fixture.statement.clone(),
            fixture.witness.clone(),
            &fixture.authoritative_state,
            start(fixture),
            &PrivacyConsensusLimitsV1::taira_default(),
            *fixture.statement.context.network_id.as_bytes(),
            estimate,
            start(fixture),
        )
    }
    // A private sentinel is used ONLY for release rejection controls. It cannot
    // be created by the public API and is never released successfully. These
    // tests do not claim a successful credential proof or prover execution.
    fn pending_for_rejection(attempt: ZkX509HolderAttemptV1) -> ZkX509PendingCredentialProofV1 {
        let mut remaining = attempt.budget;
        remaining.observation_ms = 0;
        remaining.preparation_ms = 0;
        ZkX509PendingCredentialProofV1 {
            original: attempt.original,
            proof: Vec::new(),
            times: attempt.times,
            budget: remaining,
            local_proving_start_unix_ms: attempt.local_start_unix_ms,
        }
    }

    #[test]
    fn lifecycle_preparation_authenticates_signed_material_before_budgeting() {
        let fixture = build_zk_x509_reference_fixture_v1().unwrap();
        let attempt = prepare(&fixture, budget()).expect("real native signed preflight");
        assert_eq!(attempt.original(), &fixture.statement);
        assert_eq!(
            format!("{attempt:?}"),
            "ZkX509HolderAttemptV1 { [REDACTED] }"
        );
        let mut corrupt = fixture.witness.clone();
        corrupt.wallet_ownership_signature_rs[0] ^= 1;
        let overflowing = PrivacyZkX509HolderBudgetV1 {
            observation_ms: u64::MAX,
            ..budget()
        };
        let error = ZkX509HolderAttemptV1::prepare(
            fixture.statement.clone(),
            corrupt,
            &fixture.authoritative_state,
            start(&fixture),
            &PrivacyConsensusLimitsV1::taira_default(),
            *fixture.statement.context.network_id.as_bytes(),
            overflowing,
            start(&fixture),
        )
        .expect_err("signature authentication must precede timing projection");
        assert!(
            matches!(
                error,
                ZkX509HolderAttemptErrorV1::Engine(ZkX509EngineErrorV1::ReferenceRelation(_))
            ),
            "{error:?}"
        );
        assert!(matches!(
            prepare(&fixture, overflowing),
            Err(ZkX509HolderAttemptErrorV1::Schedule(
                PrivacyZkX509HolderScheduleErrorV1::BudgetOverflow
            ))
        ));
    }

    #[test]
    fn lifecycle_full_and_remaining_budgets_keep_the_original_signed_deadline() {
        let fixture = build_zk_x509_reference_fixture_v1().unwrap();
        let total = deadline(&fixture) - start(&fixture);
        let estimate = PrivacyZkX509HolderBudgetV1 {
            proving_ms: total - 500,
            ..budget()
        };
        let attempt =
            prepare(&fixture, estimate).expect("full estimate fits at inclusive endpoint");
        assert_eq!(attempt.original(), &fixture.statement);
        let too_large = PrivacyZkX509HolderBudgetV1 {
            clock_margin_ms: 101,
            ..estimate
        };
        assert!(matches!(
            prepare(&fixture, too_large),
            Err(ZkX509HolderAttemptErrorV1::DeadlineExceeded)
        ));
        let mut proving = estimate;
        proving.observation_ms = 0;
        proving.preparation_ms = 0;
        attempt
            .times
            .require_budget(attempt.original(), proving, start(&fixture) + 200)
            .expect("completed observation/preparation are not charged twice");
        assert!(matches!(
            attempt
                .times
                .require_budget(attempt.original(), estimate, start(&fixture) + 200),
            Err(ZkX509HolderAttemptErrorV1::DeadlineExceeded)
        ));
        let mut release = proving;
        release.proving_ms = 0;
        let release_start = deadline(&fixture) - 300;
        attempt
            .times
            .require_budget(attempt.original(), release, release_start)
            .expect("only publication/submission/clock reserve remains");
        assert!(matches!(
            attempt
                .times
                .require_budget(attempt.original(), release, release_start + 1),
            Err(ZkX509HolderAttemptErrorV1::DeadlineExceeded)
        ));
        assert!(matches!(
            attempt
                .times
                .require_budget(attempt.original(), release, u64::MAX),
            Err(ZkX509HolderAttemptErrorV1::Schedule(
                PrivacyZkX509HolderScheduleErrorV1::CompletionOverflow
            ))
        ));
        // This exercises signed timing only, never a fabricated successful proof.
        assert_eq!(attempt.original(), &fixture.statement);
    }

    #[test]
    fn lifecycle_prove_and_release_reject_time_network_and_expiry_before_output() {
        let fixture = build_zk_x509_reference_fixture_v1().unwrap();
        let genesis = *fixture.statement.context.network_id.as_bytes();
        let mut other_genesis = genesis;
        other_genesis[0] ^= 1;
        for (local, chain, network, expected) in [
            (start(&fixture) - 1, start(&fixture), genesis, "backwards"),
            (start(&fixture), start(&fixture), other_genesis, "network"),
            (start(&fixture), deadline(&fixture) + 1, genesis, "expiry"),
            (deadline(&fixture), start(&fixture), genesis, "budget"),
        ] {
            let mut rng = NoEntropy::default();
            let error = prepare(&fixture, budget())
                .unwrap()
                .prove_with_rng(
                    &fixture.authoritative_state,
                    chain,
                    &PrivacyConsensusLimitsV1::taira_default(),
                    network,
                    local,
                    &mut rng,
                )
                .expect_err("invalid attempt must not reach proof construction");
            assert_eq!(rng.requests, 0);
            match expected {
                "backwards" => {
                    assert_eq!(error, ZkX509HolderAttemptErrorV1::LocalTimeMovedBackwards)
                }
                "budget" => assert_eq!(error, ZkX509HolderAttemptErrorV1::DeadlineExceeded),
                "network" => assert!(matches!(
                    error,
                    ZkX509HolderAttemptErrorV1::Engine(ZkX509EngineErrorV1::CredentialProof(_))
                )),
                _ => assert!(matches!(
                    error,
                    ZkX509HolderAttemptErrorV1::Engine(
                        ZkX509EngineErrorV1::InvalidAuthoritativeState(_)
                    )
                )),
            }
            let pending = pending_for_rejection(prepare(&fixture, budget()).unwrap());
            assert_eq!(pending.original(), &fixture.statement);
            assert_eq!(
                format!("{pending:?}"),
                "ZkX509PendingCredentialProofV1 { [REDACTED] }"
            );
            let error = pending
                .release(
                    &fixture.authoritative_state,
                    chain,
                    &PrivacyConsensusLimitsV1::taira_default(),
                    network,
                    local,
                )
                .expect_err("revalidation cannot release even the private rejection sentinel");
            match expected {
                "backwards" => {
                    assert_eq!(error, ZkX509HolderAttemptErrorV1::LocalTimeMovedBackwards)
                }
                "budget" => assert_eq!(error, ZkX509HolderAttemptErrorV1::DeadlineExceeded),
                _ => assert!(matches!(error, ZkX509HolderAttemptErrorV1::Engine(_))),
            }
        }
        let expired = ZkX509HolderAttemptV1::prepare(
            fixture.statement.clone(),
            fixture.witness.clone(),
            &fixture.authoritative_state,
            deadline(&fixture) + 1,
            &PrivacyConsensusLimitsV1::taira_default(),
            genesis,
            budget(),
            start(&fixture),
        )
        .expect_err("an old active State snapshot cannot authorize an expired statement");
        assert!(matches!(
            expired,
            ZkX509HolderAttemptErrorV1::Engine(ZkX509EngineErrorV1::InvalidAuthoritativeState(_))
        ));
    }

    #[test]
    fn lifecycle_merged_governance_supersedes_original_and_revocation_blocks_acquisition() {
        let fixture = build_zk_x509_reference_fixture_v1().unwrap();
        let anchor = fixture.authoritative_state.trust_anchor();
        let policy = fixture.authoritative_state.certificate_policy().clone();
        let crl = fixture.authoritative_state.crl_record();
        assert_eq!(
            joined_state(&[anchor], &[policy.clone()], crl).unwrap(),
            fixture.authoritative_state
        );
        let successor = build_zk_x509_network_release_successor_fixture_v1(
            fixture.statement.context.clone(),
            start(&fixture) + 1_000,
            fixture.statement.wallet_account.clone(),
            crl,
        )
        .expect("genuinely signed successor CRL");
        prepare(&successor, budget()).expect("successor has its own authentic native preflight");
        let changed_states = [
            joined_state(&[anchor], &[policy.clone(), successor_policy(&policy)], crl).unwrap(),
            joined_state(&[anchor, successor_anchor(anchor)], &[policy.clone()], crl).unwrap(),
            successor.authoritative_state,
        ];
        for current in &changed_states {
            let mut rng = NoEntropy::default();
            let error = prepare(&fixture, budget())
                .unwrap()
                .prove_with_rng(
                    current,
                    start(&fixture) + 1_000,
                    &PrivacyConsensusLimitsV1::taira_default(),
                    *fixture.statement.context.network_id.as_bytes(),
                    start(&fixture) + 1_000,
                    &mut rng,
                )
                .expect_err("each merged event supersedes the exact original before proving");
            assert!(matches!(error, ZkX509HolderAttemptErrorV1::Engine(
                ZkX509EngineErrorV1::InvalidAuthoritativeState(ref detail)) if detail.contains("stale or substituted")));
            assert_eq!(rng.requests, 0);
            let error = pending_for_rejection(prepare(&fixture, budget()).unwrap())
                .release(
                    current,
                    start(&fixture) + 1_000,
                    &PrivacyConsensusLimitsV1::taira_default(),
                    *fixture.statement.context.network_id.as_bytes(),
                    start(&fixture) + 1_000,
                )
                .expect_err("superseded original is never rebound at release");
            assert!(matches!(
                error,
                ZkX509HolderAttemptErrorV1::Engine(ZkX509EngineErrorV1::InvalidAuthoritativeState(
                    _
                ))
            ));
        }
        let error = joined_state(&[anchor], &[policy], revoked_crl(crl))
            .expect_err("revoked acquisition supplies no current active snapshot to the holder");
        assert!(
            error.contains("revoked") || error.contains("not active"),
            "{error}"
        );
        // The caller must propagate this acquisition failure, never reuse the
        // earlier fixture snapshot as evidence of current authority.
    }
}
