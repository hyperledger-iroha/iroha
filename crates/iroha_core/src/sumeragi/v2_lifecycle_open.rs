//! Sealed durable-open and authenticated restart reconciliation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use iroha_config::parameters::actual::SumeragiV2Config;
use thiserror::Error;

use super::{
    AdmissionDecision, AdmissionRequest, CandidateAdmission, CoordinatorFault,
    DurablePayloadReference, LifecycleContext, LifecycleCoordinator, LifecycleDigest, LifecycleKey,
    LifecycleStage, LifecycleStageKind, LifecycleState, LifecycleWorkClass, TerminalOutcome,
    authority::{self, AuthenticatedEpisodeAuthority},
    ledger::{
        LifecycleLedgerError, LifecycleLedgerRecordV1, LifecycleLedgerStoreV1, LifecycleLedgerV1,
    },
    replay_authority::{
        CertifiedServeTerminalReplayAuthorityPairV1, LifecycleReplayAuthorityV1,
        PreparedDurableCertifiedFetchStartupV1,
    },
    schema::{CausalRoot, DurableContinuation, DurableContinuationEdge},
    wal_recovery::{
        AuthenticatedRecoveredWalControlProjection,
        AuthenticatedRecoveredWalDecisionFetchProjection,
    },
    work_registry::{
        AuthenticatedRecoveredWalSignProjection, CertifiedServeRegistryBatchPublicationError,
        ConcreteLifecycleWorkRegistry, PreparedCertifiedServeRegistryBatchV1,
    },
};

/// Exclusive WAL-owned startup projection admitted by storage recovery.
#[derive(Clone, Copy)]
enum RecoveredWalStartupProjectionV1<'authority> {
    None,
    PhaseVote(&'authority AuthenticatedRecoveredWalSignProjection),
    ControlSign(&'authority AuthenticatedRecoveredWalControlProjection),
    DecisionFetch(&'authority AuthenticatedRecoveredWalDecisionFetchProjection),
    DecisionApply(&'authority crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1),
}
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_body_store::{
        DurableBodyValidationOutcome, RecoveredTerminalValidateOutcomeCatalogError, V2BodyStore,
    },
    v2_certified_serve_payload_store::{
        AuthenticatedCertifiedServePayloadRecoveryCut,
        AuthenticatedRecoveredCertifiedServePayloadState, CertifiedServePayloadId,
        CertifiedServePayloadStoreError, CertifiedServePayloadStoreV1,
    },
};

#[cfg(test)]
use super::RolloverSnapshot;
#[cfg(test)]
use crate::sumeragi::v2_certified_serve_payload_store::{
    CertifiedServePayloadNegativeOutcome, DurableCertifiedServeAdmissionReceipt,
};

/// Storage-authenticated identity of one terminal Validate with no successor.
///
/// The body outcome is consumed while this seal is minted and cannot be
/// replayed or rebound afterward. The historical no-child reducer branch is
/// represented by the checksummed typed ledger tombstone itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuthenticatedValidateNoSuccessorRecovery {
    key: LifecycleKey,
    causal_root: CausalRoot,
    reconstruction_source: LifecycleDigest,
    stage: LifecycleStage,
    payload: DurablePayloadReference,
}

/// Ledger-authenticated claim for one terminal Validate body outcome.
///
/// The claim is not storage authority: its private fields are decoded from one
/// exact checksummed ledger row and become authoritative only when the body
/// store's move-only recovery catalog consumes a matching semantic outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct TerminalValidateNoSuccessorClaim {
    context: LifecycleContext,
    ordinal: u128,
    key: LifecycleKey,
    causal_root: CausalRoot,
    reconstruction_source: LifecycleDigest,
    stage: LifecycleStage,
    payload: DurablePayloadReference,
}

impl TerminalValidateNoSuccessorClaim {
    /// Compare one sealed body-store outcome with the complete ledger identity.
    pub(in crate::sumeragi) fn matches_outcome(
        &self,
        outcome: &DurableBodyValidationOutcome,
    ) -> bool {
        super::projection::recovered_validate_no_successor_ledger_identity_is_authenticated(
            self.context,
            self.key,
            self.causal_root,
            self.reconstruction_source,
            self.stage,
            self.payload,
            outcome,
        )
    }

    fn into_authenticated(self) -> AuthenticatedValidateNoSuccessorRecovery {
        AuthenticatedValidateNoSuccessorRecovery {
            key: self.key,
            causal_root: self.causal_root,
            reconstruction_source: self.reconstruction_source,
            stage: self.stage,
            payload: self.payload,
        }
    }
}

/// Move-only, post-authentication join between durable logical rows and their
/// exact storage-reconstructed work.
///
/// Constructors stay inside the lifecycle authority. Production storage code
/// receives this value only after the exhaustive effect classifier, body/WAL
/// reconciliation, and Certified-Serve payload resolver have authenticated all
/// of its parts. Terminal no-successor Validate rows additionally require an
/// exact body-store outcome bound to their immutable parent identity. The
/// move-only payload cut may retain authenticated store-only crash tails;
/// durable open removes those orphans only after every ledger Serve resolves
/// exactly and before the reconciled ledger is published.
#[derive(Debug)]
#[must_use]
pub(crate) struct AuthenticatedLifecycleRecoveryCut {
    context: LifecycleContext,
    /// Exact already-opened frame classified by the storage-only assembler.
    /// Durable open rejects a different reread for every production and focused
    /// test cut; there is no unauthenticated frame bypass.
    authenticated_ledger: LifecycleLedgerV1,
    candidates: BTreeMap<LifecycleKey, CandidateAdmission>,
    validate_no_successor: BTreeMap<LifecycleKey, AuthenticatedValidateNoSuccessorRecovery>,
    serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
}

impl AuthenticatedLifecycleRecoveryCut {
    /// Consume the exact post-prune Serve payload census into its owner.
    pub(super) fn into_serve_payloads(self) -> AuthenticatedCertifiedServePayloadRecoveryCut {
        self.serve_payloads
    }

    /// Assemble an exact test fixture from already authenticated projections.
    ///
    /// Production recovery must use the sealed storage-only factory matching
    /// its durable inputs: [`Self::assemble_storage_only`],
    /// [`Self::assemble_storage_only_with_body_validation_outcomes`],
    /// [`Self::assemble_storage_only_with_recovered_wal_sign`], or
    /// [`Self::assemble_storage_only_with_recovered_wal_sign_and_body_validation_outcomes`].
    /// This raw candidate surface deliberately does not exist outside test
    /// builds.
    #[cfg(test)]
    pub(super) fn from_authenticated_parts(
        authenticated_ledger: LifecycleLedgerV1,
        candidates: impl IntoIterator<Item = CandidateAdmission>,
        validate_no_successor: impl IntoIterator<
            Item = (CandidateAdmission, DurableBodyValidationOutcome),
        >,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Option<Self> {
        let context = authenticated_ledger.context();
        if digest_bytes(serve_payloads.context_id().0.as_ref()) != context.id()
            || serve_payloads.height() != context.height()
        {
            return None;
        }
        let mut candidate_map = BTreeMap::new();
        for candidate in candidates {
            if matches!(
                candidate.work_class,
                LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn
            ) || candidate_map.insert(candidate.key, candidate).is_some()
            {
                return None;
            }
        }
        let mut validate_no_successor_map = BTreeMap::new();
        for (candidate, outcome) in validate_no_successor {
            if candidate_map.contains_key(&candidate.key)
                || !super::projection::recovered_validate_no_successor_is_authenticated(
                    context, &candidate, &outcome,
                )
            {
                return None;
            }
            let authenticated = AuthenticatedValidateNoSuccessorRecovery {
                key: candidate.key,
                causal_root: candidate.causal_root,
                reconstruction_source: candidate.reconstruction_source,
                stage: candidate.stage,
                payload: candidate.payload,
            };
            if validate_no_successor_map
                .insert(candidate.key, authenticated)
                .is_some()
            {
                return None;
            }
        }
        Some(Self {
            context,
            authenticated_ledger,
            candidates: candidate_map,
            validate_no_successor: validate_no_successor_map,
            serve_payloads,
        })
    }

    // STORAGE_ONLY_LIFECYCLE_RECOVERY_ASSEMBLER_BEGIN
    /// Assemble the bounded storage-only recovery cut from one exact opened frame.
    ///
    /// This factory has no caller-supplied candidate surface. Certified-Serve
    /// and its adjacent ProducerTurn are reconstructed exclusively through the
    /// authenticated payload cut. Every other live row fails closed until a
    /// typed durable replay authority exists. Classification borrows both
    /// inputs; success moves them into the seal, while failure retains them in
    /// [`LifecycleRecoveryAssemblyError`].
    ///
    /// # Errors
    ///
    /// Returns an owned typed failure when a live ordinary row lacks durable
    /// replay authority, a terminal Validate/no-successor row lacks its body
    /// outcome, or Certified-Serve storage does not cover the frame exactly.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<Self, LifecycleRecoveryAssemblyError> {
        if let Err(kind) = validate_storage_only_recovery(&ledger, &serve_payloads) {
            return Err(LifecycleRecoveryAssemblyError {
                kind,
                _authenticated_ledger: ledger,
                _serve_payloads: serve_payloads,
            });
        }
        Ok(Self {
            context: ledger.context(),
            authenticated_ledger: ledger,
            candidates: BTreeMap::new(),
            validate_no_successor: BTreeMap::new(),
            serve_payloads,
        })
    }

    /// Assemble storage-only recovery while consuming exact terminal Validate outcomes.
    ///
    /// All ledger and Certified-Serve checks finish before the body-store
    /// catalog is detached. The aggregate cut then selects every terminal
    /// Validate/no-successor claim exactly once; any missing or ambiguous row
    /// restores the complete catalog before this owned error is returned.
    /// Unrelated validation outcomes are restored when the selected set is
    /// committed, so recovered-WAL success authority is not consumed here.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_body_validation_outcomes(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
    ) -> Result<Self, LifecycleRecoveryAssemblyError> {
        Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::None,
            None,
        )
    }

    /// Assemble the sole Ready-Fetch production startup recovery cut.
    ///
    /// The opaque Fetch phase moves every logical candidate directly into this
    /// recovery value while retaining every concrete completion for the
    /// subsequent empty-registry install. Terminal Validate outcomes are
    /// consumed from the same owned body-store instance. Any other live
    /// ordinary class remains unsupported and fails closed.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_durable_fetch_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        mut fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedFetchStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::None,
            Some(&mut fetches),
        )?;
        Ok((recovery, fetches))
    }

    /// Assemble one exact post-fsync recovered-WAL Sign crash cut.
    ///
    /// This remains a storage-only factory: its only additional input is the
    /// opaque projection minted by the installed recovered-WAL registry row.
    /// Exactly that projection's live Sign child may cross the ordinary-row
    /// fail-closed classifier. A live Validate parent, a foreign Sign, or any
    /// other live ordinary row is still rejected. The projection is borrowed,
    /// so the installed registry authority remains with the caller; the exact
    /// ledger frame and move-only Serve cut move into either the seal or the
    /// owned failure.
    ///
    /// # Errors
    ///
    /// Returns an owned typed failure if the repaired frame does not contain
    /// the exact installed live Sign child or otherwise fails the bounded
    /// storage-only census.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    // TODO: Invoke this only inside the consuming installed-registry startup
    // transaction; never expose its logical recovery cut as standalone
    // production authority.
    pub(super) fn assemble_storage_only_with_recovered_wal_sign(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> Result<Self, LifecycleRecoveryAssemblyError> {
        let candidates = match assemble_storage_only_recovered_wal_candidates(
            &ledger,
            &serve_payloads,
            projection,
        ) {
            Ok(candidates) => candidates,
            Err(kind) => {
                return Err(LifecycleRecoveryAssemblyError {
                    kind,
                    _authenticated_ledger: ledger,
                    _serve_payloads: serve_payloads,
                });
            }
        };
        Ok(Self {
            context: ledger.context(),
            authenticated_ledger: ledger,
            candidates,
            validate_no_successor: BTreeMap::new(),
            serve_payloads,
        })
    }

    /// Assemble a repaired-WAL Sign cut together with terminal Validate outcomes.
    ///
    /// The opaque installed Sign projection remains borrowed while the exact
    /// repaired parent/child pair, every other durable row, Certified-Serve
    /// coverage, and the aggregate body-outcome catalog are authenticated.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_wal_sign_and_body_validation_outcomes(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> Result<Self, LifecycleRecoveryAssemblyError> {
        Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::PhaseVote(projection),
            None,
        )
    }

    /// Assemble the final repaired-WAL Sign, every durable Ready-Fetch, and
    /// all terminal Validate outcomes from one exact post-repair frame.
    ///
    /// The installed Sign projection remains borrowed, while the Fetch phase
    /// is consumed only after its complete frame-bound census is spliced. This
    /// is the sole storage assembler used by the unified recovered-vote
    /// production startup.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_wal_sign_and_durable_fetch_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalSignProjection,
        mut fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedFetchStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::PhaseVote(projection),
            Some(&mut fetches),
        )?;
        Ok((recovery, fetches))
    }

    /// Assemble the exact standalone control Sign with every durable Fetch.
    ///
    /// The exclusive startup-projection enum makes a phase-vote/control pair
    /// unrepresentable. Only the control projection's exact live row may cross
    /// the ordinary-row fail-closed classifier.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_wal_control_sign_and_durable_fetch_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalControlProjection,
        mut fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedFetchStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::ControlSign(projection),
            Some(&mut fetches),
        )?;
        Ok((recovery, fetches))
    }

    /// Assemble the standalone Decision Fetch with every durable body-backed Fetch.
    ///
    /// The exclusive startup enum prevents coexistence with a phase vote or
    /// control Sign. Only this exact WAL-owned, payload-free Fetch row may
    /// cross the ordinary-row fail-closed classifier.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_wal_decision_fetch_and_durable_fetch_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
        mut fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedFetchStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::DecisionFetch(projection),
            Some(&mut fetches),
        )?;
        Ok((recovery, fetches))
    }

    /// Assemble one exact recovered Decision body chain with every unrelated
    /// durable Ready-Fetch row.
    ///
    /// The projection must name an already-terminal Fetch/Store/Validate
    /// prefix and the sole live Apply successor. It is borrowed while the
    /// candidate is spliced, so the dedicated registry carrier remains owned
    /// by the caller for the later atomic install.
    #[allow(clippy::result_large_err)]
    pub(super) fn assemble_storage_only_with_recovered_decision_apply_and_durable_fetch_startup(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
        mut fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(Self, PreparedDurableCertifiedFetchStartupV1), LifecycleRecoveryAssemblyError>
    {
        let recovery = Self::assemble_storage_only_with_terminal_validate_outcomes(
            ledger,
            serve_payloads,
            body_store,
            RecoveredWalStartupProjectionV1::DecisionApply(projection),
            Some(&mut fetches),
        )?;
        Ok((recovery, fetches))
    }

    #[allow(clippy::result_large_err)]
    fn assemble_storage_only_with_terminal_validate_outcomes(
        ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: &mut V2BodyStore,
        recovered_wal: RecoveredWalStartupProjectionV1<'_>,
        durable_fetches: Option<&mut PreparedDurableCertifiedFetchStartupV1>,
    ) -> Result<Self, LifecycleRecoveryAssemblyError> {
        let (candidates, claims) =
            match assemble_storage_only_candidates_and_terminal_validate_claims(
                &ledger,
                &serve_payloads,
                recovered_wal,
                durable_fetches,
            ) {
                Ok(assembled) => assembled,
                Err(kind) => {
                    return Err(LifecycleRecoveryAssemblyError {
                        kind,
                        _authenticated_ledger: ledger,
                        _serve_payloads: serve_payloads,
                    });
                }
            };
        if claims.is_empty() {
            return Ok(Self {
                context: ledger.context(),
                authenticated_ledger: ledger,
                candidates,
                validate_no_successor: BTreeMap::new(),
                serve_payloads,
            });
        }

        let mut catalog = match body_store.detach_terminal_validate_outcome_catalog() {
            Ok(catalog) => catalog,
            Err(error) => {
                let detail = match error {
                    RecoveredTerminalValidateOutcomeCatalogError::UnrevalidatedMarkers => {
                        "durable markers have not completed semantic replay"
                    }
                    RecoveredTerminalValidateOutcomeCatalogError::AmbiguousOutcome => {
                        "one proposal is present in both closed outcome maps"
                    }
                };
                return Err(LifecycleRecoveryAssemblyError {
                    kind: LifecycleRecoveryAssemblyErrorKind::TerminalValidateOutcomeCatalog(
                        detail,
                    ),
                    _authenticated_ledger: ledger,
                    _serve_payloads: serve_payloads,
                });
            }
        };
        for claim in claims.values() {
            if !catalog.select_exact_terminal_validate(claim) {
                return Err(LifecycleRecoveryAssemblyError {
                    kind: LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome {
                        ordinal: claim.ordinal,
                        stage: claim.stage,
                    },
                    _authenticated_ledger: ledger,
                    _serve_payloads: serve_payloads,
                });
            }
        }
        let validate_no_successor = claims
            .into_values()
            .map(|claim| (claim.key, claim.into_authenticated()))
            .collect();
        let recovery = Self {
            context: ledger.context(),
            authenticated_ledger: ledger,
            candidates,
            validate_no_successor,
            serve_payloads,
        };
        catalog.commit_selected();
        Ok(recovery)
    }
    // STORAGE_ONLY_LIFECYCLE_RECOVERY_ASSEMBLER_END

    fn authenticates_opened_ledger(&self, opened: &LifecycleLedgerV1) -> bool {
        &self.authenticated_ledger == opened
    }

    /// Assemble the empty logical side of a focused recovered-WAL fixture from
    /// a real authenticated payload-store cut.
    #[cfg(test)]
    pub(crate) fn empty_for_recovered_wal_test(
        verified: &VerifiedHeightContext,
        authenticated_ledger: LifecycleLedgerV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Option<Self> {
        if authenticated_ledger.context()
            != super::projection::lifecycle_context(verified.context())
        {
            return None;
        }
        Self::from_authenticated_parts(authenticated_ledger, [], [], serve_payloads)
    }

    // RECOVERED_WAL_SIGN_RECOVERY_SPLICE_BEGIN
    /// Replace one exact recovered Validate parent by its authenticated WAL
    /// Sign successor, or accept an already-repaired exact child.
    ///
    /// The comparison is complete before mutation. A recovery cut with neither
    /// exact side, both sides, a foreign context, or a terminal no-successor
    /// claim therefore stays byte-for-byte unchanged. The caller retains the
    /// closed concrete registry row which authenticated both candidates; this
    /// method never exposes either candidate outside the sealed startup path.
    pub(super) fn splice_recovered_wal_sign(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        if !projection.belongs_to_context(self.context)
            || self
                .validate_no_successor
                .contains_key(&projection.parent_key())
            || self
                .validate_no_successor
                .contains_key(&projection.child_key())
        {
            return false;
        }
        projection.splice_candidates(&mut self.candidates)
    }

    /// Revalidate the post-splice recovery ownership without exposing either
    /// retained candidate.
    pub(super) fn owns_recovered_wal_sign(
        &self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && !self
                .validate_no_successor
                .contains_key(&projection.parent_key())
            && !self
                .validate_no_successor
                .contains_key(&projection.child_key())
            && projection.owns_spliced_candidates(&self.candidates)
    }

    /// Revalidate the exact standalone control Sign retained by recovery.
    pub(super) fn owns_recovered_wal_control_sign(
        &self,
        projection: &AuthenticatedRecoveredWalControlProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && projection.owns_spliced_candidate(&self.candidates)
    }

    /// Revalidate the exact standalone Decision Fetch retained by recovery.
    pub(super) fn owns_recovered_wal_decision_fetch(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && projection.owns_spliced_candidate(&self.candidates)
    }

    /// Seed the opaque installed projection's exact Validate parent.
    #[cfg(test)]
    pub(super) fn seed_recovered_wal_parent_for_test(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && self.candidates.is_empty()
            && self.validate_no_successor.is_empty()
            && projection.seed_parent_candidate_for_test(&mut self.candidates)
    }

    /// Seed the opaque installed projection's exact Sign child.
    #[cfg(test)]
    pub(super) fn seed_recovered_wal_child_for_test(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && self.candidates.is_empty()
            && self.validate_no_successor.is_empty()
            && projection.seed_child_candidate_for_test(&mut self.candidates)
    }

    /// Seed both opaque projection sides for an ambiguity-preservation test.
    #[cfg(test)]
    pub(super) fn seed_both_recovered_wal_candidates_for_test(
        &mut self,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        projection.belongs_to_context(self.context)
            && self.candidates.is_empty()
            && self.validate_no_successor.is_empty()
            && projection.seed_both_candidates_for_test(&mut self.candidates)
    }
    // RECOVERED_WAL_SIGN_RECOVERY_SPLICE_END
}

/// Owned failure from the storage-only recovery-cut assembler.
///
/// The exact opened LedgerV1 frame and the move-only authenticated Serve cut
/// remain sealed here on every failure. A caller may therefore fail-stop
/// without discarding either durable authority or accidentally retrying from a
/// different frame.
#[derive(Debug, Error)]
#[error("{kind}")]
#[must_use = "failed lifecycle recovery assembly still owns durable authority"]
pub(crate) struct LifecycleRecoveryAssemblyError {
    #[source]
    kind: LifecycleRecoveryAssemblyErrorKind,
    _authenticated_ledger: LifecycleLedgerV1,
    _serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
}

impl LifecycleRecoveryAssemblyError {
    /// Borrow the typed, non-authorizing diagnostic.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn kind(&self) -> &LifecycleRecoveryAssemblyErrorKind {
        &self.kind
    }
}

/// Exhaustive reason why durable storage could not form one recovery cut.
#[derive(Debug, Error)]
pub(crate) enum LifecycleRecoveryAssemblyErrorKind {
    /// One live ordinary row has no exact durable carrier reconstruction.
    #[error(
        "live lifecycle ordinal {ordinal} ({work_class:?}, {stage:?}) has no durable recovery authority"
    )]
    MissingDurableRecoveryAuthority {
        /// Immutable ledger ordinal of the unsupported live row.
        ordinal: u128,
        /// Exhaustive logical work class retained by LedgerV1.
        work_class: LifecycleWorkClass,
        /// Exact immutable execution stage retained by LedgerV1.
        stage: LifecycleStage,
    },
    /// A terminal Validate/no-child tombstone lost its consumed body outcome.
    #[error("terminal Validate ordinal {ordinal} ({stage:?}) has no authenticated body outcome")]
    MissingTerminalValidateOutcome {
        /// Immutable ledger ordinal of the no-successor tombstone.
        ordinal: u128,
        /// Exact immutable Validate stage retained by LedgerV1.
        stage: LifecycleStage,
    },
    /// The body-store recovery catalog is not ready for exact terminal coverage.
    #[error("terminal Validate body-outcome catalog is unavailable: {0}")]
    TerminalValidateOutcomeCatalog(&'static str),
    /// A checksummed record could not be decoded into the closed schema.
    #[error("lifecycle ordinal {ordinal} has invalid durable {field}")]
    InvalidDurableRecord {
        /// Immutable ledger ordinal of the malformed row.
        ordinal: u128,
        /// Closed field name which failed typed decoding.
        field: &'static str,
    },
    /// Work class and execution stage no longer form one closed schema pair.
    #[error(
        "lifecycle ordinal {ordinal} has inconsistent durable class/stage ({work_class:?}, {stage:?})"
    )]
    InvalidDurableRecordShape {
        /// Immutable ledger ordinal of the malformed row.
        ordinal: u128,
        /// Decoded logical work class.
        work_class: LifecycleWorkClass,
        /// Decoded immutable execution stage.
        stage: LifecycleStage,
    },
    /// The opaque installed recovered-WAL projection and repaired frame differ.
    #[error("recovered-WAL Sign storage recovery is incomplete: {0}")]
    RecoveredWalSign(&'static str),
    /// The complete recovered Ready-Fetch census differs from the ledger.
    #[error("durable Ready-Fetch startup census is inconsistent: {0}")]
    DurableCertifiedFetch(&'static str),
    /// The authenticated Serve cut did not resolve the ledger exactly.
    #[error("Certified-Serve storage recovery is incomplete: {0}")]
    CertifiedServe(#[source] LifecycleOpenError),
}

/// Failure to open the sole durable lifecycle authority for one height.
#[derive(Debug, Error)]
#[error("{0}")]
pub(crate) struct LifecycleOpenError(LifecycleOpenErrorKind);

#[derive(Debug, Error)]
enum LifecycleOpenErrorKind {
    #[error("verified height context cannot derive bounded lifecycle authority")]
    InvalidAuthority,
    #[error("authenticated lifecycle recovery cut is inconsistent: {0}")]
    InvalidRecovery(&'static str),
    #[error(transparent)]
    Ledger(#[from] LifecycleLedgerError),
    #[error(transparent)]
    PayloadStore(#[from] CertifiedServePayloadStoreError),
}

impl From<LifecycleOpenErrorKind> for LifecycleOpenError {
    fn from(error: LifecycleOpenErrorKind) -> Self {
        Self(error)
    }
}

impl From<LifecycleLedgerError> for LifecycleOpenError {
    fn from(error: LifecycleLedgerError) -> Self {
        Self(LifecycleOpenErrorKind::Ledger(error))
    }
}

impl From<CertifiedServePayloadStoreError> for LifecycleOpenError {
    fn from(error: CertifiedServePayloadStoreError) -> Self {
        Self(LifecycleOpenErrorKind::PayloadStore(error))
    }
}

/// In-memory durable-open result before either local store is published.
///
/// The coordinator has completed exhaustive recovery and rebinding, but its
/// LedgerV1 projection and payload-orphan pruning remain uncommitted. This
/// closed stage lets the recovered-WAL registry transaction compare its exact
/// installed Sign row before any durable open publication occurs.
#[must_use = "prepared lifecycle open has not published its durable stores"]
pub(super) struct PreparedLifecycleCoordinatorOpen {
    coordinator: LifecycleCoordinator,
    store: LifecycleLedgerStoreV1,
    persisted_predecessor: LifecycleLedgerV1,
    authenticated_successor: LifecycleLedgerV1,
    retained_serve_payloads: BTreeSet<CertifiedServePayloadId>,
    certified_serve_registry: Option<PreparedCertifiedServeRegistryBatchV1>,
}

/// Fail-stop durable-open commit error retaining the complete prepared state.
#[must_use = "failed lifecycle open still owns its prepared coordinator authority"]
pub(super) struct LifecycleOpenCommitError {
    error: LifecycleOpenError,
    _prepared: PreparedLifecycleCoordinatorOpen,
}

impl LifecycleOpenCommitError {
    pub(super) fn into_error(self) -> LifecycleOpenError {
        self.error
    }
}

impl PreparedLifecycleCoordinatorOpen {
    /// Borrow the completely rebound coordinator before store publication.
    pub(super) const fn coordinator(&self) -> &LifecycleCoordinator {
        &self.coordinator
    }

    /// Borrow the exact opened LedgerV1 store before publication.
    pub(super) const fn store(&self) -> &LifecycleLedgerStoreV1 {
        &self.store
    }

    /// Prune authenticated payload orphans, then publish the exact coordinator
    /// projection, retaining this whole stage on either failure.
    #[allow(clippy::result_large_err)]
    #[cfg(test)]
    pub(super) fn commit(
        mut self,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<LifecycleCoordinator, LifecycleOpenCommitError> {
        if let Err(error) = self.publish_durable_open(payload_store, recovery) {
            return Err(LifecycleOpenCommitError {
                error,
                _prepared: self,
            });
        }
        self.coordinator.ledger_store = Some(self.store);
        Ok(self.coordinator)
    }

    /// Atomically install the complete Serve/Producer concrete batch around
    /// LedgerV1 publication. Registry preflight failure changes neither owner;
    /// store publication failure removes the staged carriers before returning
    /// the still-complete prepared open.
    #[allow(clippy::result_large_err)]
    pub(super) fn commit_with_registry(
        mut self,
        registry: &mut ConcreteLifecycleWorkRegistry,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<LifecycleCoordinator, LifecycleOpenCommitError> {
        let Some(batch) = self.certified_serve_registry.take() else {
            return Err(LifecycleOpenCommitError {
                error: LifecycleOpenErrorKind::InvalidRecovery(
                    "Certified-Serve concrete batch was already consumed",
                )
                .into(),
                _prepared: self,
            });
        };
        let publication = registry.install_certified_serve_startup_batch_before_publication(
            batch,
            &self.coordinator,
            || self.publish_durable_open(payload_store, recovery),
        );
        match publication {
            Ok(()) => {}
            Err(CertifiedServeRegistryBatchPublicationError::Preflight(batch)) => {
                self.certified_serve_registry = Some(batch);
                return Err(LifecycleOpenCommitError {
                    error: LifecycleOpenErrorKind::InvalidRecovery(
                        "Certified-Serve concrete registry preflight failed",
                    )
                    .into(),
                    _prepared: self,
                });
            }
            Err(CertifiedServeRegistryBatchPublicationError::Publication(error, batch)) => {
                self.certified_serve_registry = Some(batch);
                return Err(LifecycleOpenCommitError {
                    error,
                    _prepared: self,
                });
            }
        }
        self.coordinator.ledger_store = Some(self.store);
        Ok(self.coordinator)
    }

    fn publish_durable_open(
        &self,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<(), LifecycleOpenError> {
        // Exact recovery stutters validate the attached frame without replacing it;
        // payload-orphan pruning still runs because it authenticates a separate store.
        let projection = match LifecycleLedgerV1::from_coordinator(&self.coordinator) {
            Ok(projection) => projection,
            Err(error) => return Err(error.into()),
        };
        if projection != self.authenticated_successor {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "prepared coordinator changed its authenticated LedgerV1 successor",
            )
            .into());
        }
        // Orphans are authenticated as absent from both the retained recovery
        // frame and this exact successor. Remove them before advancing the
        // ledger so every fallible filesystem operation precedes the sole
        // logical publication fsync. A partial prune can only remove unowned
        // Pending files and is safely repeated after restart.
        payload_store.prune_authenticated_orphans(
            &mut recovery.serve_payloads,
            &self.retained_serve_payloads,
        )?;
        if let Err(error) = self
            .store
            .persist_exact_successor(&self.persisted_predecessor, &projection)
        {
            return Err(error.into());
        }
        Ok(())
    }
}

impl LifecycleCoordinator {
    fn reconcile_store_ahead_terminal_serve(
        &mut self,
        serve_ordinal: u128,
        mut candidate: CandidateAdmission,
        update: TerminalUpdate,
    ) -> Result<(), LifecycleOpenError> {
        if update.ordinal != serve_ordinal
            || !update
                .replay
                .exactly_matches_recovered_candidate(self.active_context, &candidate)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead replay family changed its recovered candidate",
            )
            .into());
        }
        let producer_ordinal = self.producer_debts.get(&serve_ordinal).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead Serve has no adjacent producer debt",
            ),
        )?;
        if !update.replay.exactly_advances_pending_records(
            self.active_context,
            self.records
                .get(&serve_ordinal)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead Serve record disappeared",
                ))?,
            self.durable_records.get(&serve_ordinal).ok_or(
                LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead Serve metadata disappeared",
                ),
            )?,
            self.records
                .get(&producer_ordinal)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead producer record disappeared",
                ))?,
            self.durable_records.get(&producer_ordinal).ok_or(
                LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead producer metadata disappeared",
                ),
            )?,
        ) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead frame is not the exact Pending pair successor",
            )
            .into());
        }
        let producer =
            candidate
                .producer_turn
                .as_ref()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store-ahead Serve lost its producer companion",
                ))?;
        self.durable_records
            .get_mut(&serve_ordinal)
            .expect("preflight retained Serve metadata")
            .replay_authority = candidate.replay_authority.clone();
        self.durable_records
            .get_mut(&producer_ordinal)
            .expect("preflight retained ProducerTurn metadata")
            .replay_authority = producer.replay_authority.clone();
        if !matches!(
            self.records[&serve_ordinal].state,
            LifecycleState::Waiting(wait)
                if matches!(wait.source, super::WaitSource::Recovery(_))
        ) || self
            .rebind_recovered_candidate(serve_ordinal, &mut candidate)
            .is_err()
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead candidate did not rebind exactly",
            )
            .into());
        }
        if !update.replay.exactly_matches_rebound_records(
            self.active_context,
            &self.records[&serve_ordinal],
            &self.durable_records[&serve_ordinal],
            &self.records[&producer_ordinal],
            &self.durable_records[&producer_ordinal],
        ) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "rebound payload-store-ahead pair changed before settlement",
            )
            .into());
        }
        let expected_payload = update.payload;
        let expected_outcome = update.outcome;
        let (payload, outcome, serve_replay, producer_replay) =
            update.replay.consume_terminal_rebind();
        if payload != expected_payload || outcome != expected_outcome {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead terminal projection changed during settlement",
            )
            .into());
        }
        let serve_metadata = self
            .durable_records
            .get_mut(&serve_ordinal)
            .expect("rebound Serve metadata remains present");
        serve_metadata.payload = payload;
        serve_metadata.replay_authority = serve_replay;
        self.durable_records
            .get_mut(&producer_ordinal)
            .expect("rebound ProducerTurn metadata remains present")
            .replay_authority = producer_replay;
        self.finish_terminal(serve_ordinal, outcome).map_err(|_| {
            LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store terminal cut could not settle its Serve",
            )
        })?;
        if self.durable_records[&serve_ordinal].payload != expected_payload {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal payload projection changed during settlement",
            )
            .into());
        }
        Ok(())
    }

    /// Open the sole durable coordinator from a verified height context.
    ///
    /// The persisted ledger owns the ordinal high-water mark. Every live row
    /// must join exactly one authenticated recovery candidate (ProducerTurn
    /// rows join through their adjacent Serve), and every Serve row must join
    /// its payload-store reference. Rebinding and payload-store-ahead terminal
    /// cuts are persisted before this method returns. Authenticated payloads
    /// with no ledger owner are then durably pruned through `payload_store`.
    #[cfg(test)]
    pub(crate) fn open_from_verified_height_context(
        verified: &VerifiedHeightContext,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        let authority =
            authority::production_authority(verified, config, reply_route_source_capacity)
                .ok_or(LifecycleOpenErrorKind::InvalidAuthority)?;
        Self::open_with_authority(authority, ledger_root, payload_store, recovery)
    }

    /// Open with an already authenticated bounded episode authority.
    #[cfg(test)]
    pub(super) fn open_with_authority(
        authority: AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        Self::open_with_authority_borrowed(authority, ledger_root, payload_store, &mut recovery)
    }

    // RECOVERED_WAL_SIGN_BORROWED_OPEN_BEGIN
    /// Open while retaining the move-only authenticated recovery cut outside
    /// the coordinator transaction.
    ///
    /// Recovered-WAL startup needs this form so every failure can keep the
    /// exact payload authentication and installed registry borrow sealed for a
    /// fail-stop restart. Candidate values are cloned only into the new
    /// coordinator; no storage or concrete-work authority is duplicated.
    #[cfg(test)]
    pub(super) fn open_with_authority_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        let prepared =
            Self::prepare_with_authority_borrowed(authority, ledger_root, payload_store, recovery)?;
        prepared
            .commit(payload_store, recovery)
            .map_err(LifecycleOpenCommitError::into_error)
    }

    /// Complete recovery and rebinding without publishing either local store.
    pub(super) fn prepare_with_authority_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        let context = authority.context();
        let (store, ledger) = LifecycleLedgerStoreV1::open(ledger_root, context)?;
        Self::prepare_with_exact_store_borrowed(authority, store, ledger, payload_store, recovery)
    }

    /// Prepare from the exact ledger-store instance retained by the consuming
    /// Ready-Fetch storage cut. No caller-selected path can be substituted.
    pub(super) fn prepare_with_authenticated_store_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        let ledger = store.load()?;
        Self::prepare_with_exact_store_borrowed(authority, store, ledger, payload_store, recovery)
    }

    /// Prepare a fully authenticated prospective successor while the exact
    /// retained store still contains its predecessor frame.
    ///
    /// This is the sole pre-fsync open used by recovered Decision Apply. All
    /// logical reconstruction, Serve payload validation, and registry-batch
    /// preparation target `successor`; publication later compares and replaces
    /// `predecessor` through the same store instance.
    pub(super) fn prepare_with_authenticated_successor_store_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        predecessor: LifecycleLedgerV1,
        successor: LifecycleLedgerV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        if !store.load().is_ok_and(|opened| opened == predecessor) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "lifecycle ledger predecessor changed before prospective open",
            )
            .into());
        }
        Self::prepare_with_exact_store_successor_borrowed(
            authority,
            store,
            predecessor,
            successor,
            payload_store,
            recovery,
        )
    }

    fn prepare_with_exact_store_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        ledger: LifecycleLedgerV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        Self::prepare_with_exact_store_successor_borrowed(
            authority,
            store,
            ledger.clone(),
            ledger,
            payload_store,
            recovery,
        )
    }

    fn prepare_with_exact_store_successor_borrowed(
        authority: AuthenticatedEpisodeAuthority,
        store: LifecycleLedgerStoreV1,
        persisted_predecessor: LifecycleLedgerV1,
        ledger: LifecycleLedgerV1,
        payload_store: &CertifiedServePayloadStoreV1,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> Result<PreparedLifecycleCoordinatorOpen, LifecycleOpenError> {
        let context = authority.context();
        if recovery.context != context {
            return Err(LifecycleOpenErrorKind::InvalidRecovery("foreign recovery context").into());
        }
        payload_store.validate_authenticated_cut(&recovery.serve_payloads)?;
        if !recovery.authenticates_opened_ledger(&ledger) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "lifecycle ledger changed after recovery-cut authentication",
            )
            .into());
        }
        validate_terminal_validate_no_successor_recovery(&ledger, &recovery.validate_no_successor)?;
        let records_by_key = decoded_records_by_key(&ledger)?;
        let (serve_candidates, terminal_updates, retained_serve_payloads, serve_replay_pairs) =
            resolve_serve_payloads(context, &ledger, &records_by_key, &recovery.serve_payloads)?;
        let mut recovered_candidates = recovery.candidates.clone();
        for candidate in serve_candidates {
            if recovered_candidates
                .insert(candidate.key, candidate)
                .is_some()
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "Serve projection collided with non-Serve recovery work",
                )
                .into());
            }
        }

        let mut physical_universes = ledger
            .records()
            .iter()
            .map(|record| (record.ordinal(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut candidates_by_ordinal = BTreeMap::new();
        let mut producer_coverage = BTreeSet::new();
        let terminal_updates_by_ordinal = terminal_updates
            .iter()
            .map(|update| (update.ordinal, update))
            .collect::<BTreeMap<_, _>>();
        for (_, mut candidate) in recovered_candidates {
            candidate.canonicalize_geometry().map_err(|_| {
                LifecycleOpenErrorKind::InvalidRecovery("invalid physical geometry")
            })?;
            let record = records_by_key.get(&candidate.key).copied().ok_or(
                LifecycleOpenErrorKind::InvalidRecovery(
                    "recovered candidate has no durable semantic row",
                ),
            )?;
            let ordinal = record.ordinal();
            let terminal_update = terminal_updates_by_ordinal.get(&ordinal).copied();
            validate_candidate_record(ledger.context(), record, &candidate, terminal_update)?;
            if candidates_by_ordinal
                .insert(ordinal, candidate.clone())
                .is_some()
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "multiple candidates cover one durable row",
                )
                .into());
            }
            if record.terminal().flatten().is_none() {
                let (_, universe, _) = candidate.physical_geometry.normalized().map_err(|_| {
                    LifecycleOpenErrorKind::InvalidRecovery("invalid primary geometry")
                })?;
                if !authority.admits_slots(candidate.work_class.capacity_class(), &universe) {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "primary geometry exceeds authenticated capacity",
                    )
                    .into());
                }
                physical_universes.insert(ordinal, universe);
            }
            match (candidate.work_class, candidate.producer_turn.as_ref()) {
                (LifecycleWorkClass::CertifiedServe, Some(producer)) => {
                    let producer_ordinal =
                        ordinal
                            .checked_add(1)
                            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                                "producer ordinal overflowed",
                            ))?;
                    let producer_record = ledger_record_at(&ledger, producer_ordinal).ok_or(
                        LifecycleOpenErrorKind::InvalidRecovery(
                            "Serve has no adjacent durable producer",
                        ),
                    )?;
                    let replay_matches = terminal_update.map_or_else(
                        || producer_record.replay_matches_producer(producer),
                        |update| {
                            update
                                .replay
                                .exactly_matches_recovered_candidate(ledger.context(), &candidate)
                                && record.replay_is_exact_pending_predecessor(
                                    ledger.context(),
                                    producer_record,
                                    &update.replay,
                                )
                        },
                    );
                    if producer_record.key() != Some(producer.key)
                        || producer_record.owner() != record.owner()
                        || producer_record.work_class() != Some(LifecycleWorkClass::ProducerTurn)
                        || producer_record.stage() != Some(producer.stage)
                        || producer_record.reconstruction_source() != producer.reconstruction_source
                        || !replay_matches
                    {
                        return Err(LifecycleOpenErrorKind::InvalidRecovery(
                            "producer companion changed durable semantics",
                        )
                        .into());
                    }
                    if producer_record.terminal().flatten().is_none() {
                        let (_, universe, _) =
                            producer.physical_geometry.normalized().map_err(|_| {
                                LifecycleOpenErrorKind::InvalidRecovery("invalid producer geometry")
                            })?;
                        if !authority.admits_slots(
                            LifecycleWorkClass::ProducerTurn.capacity_class(),
                            &universe,
                        ) || !producer_coverage.insert(producer_ordinal)
                        {
                            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                                "producer geometry or coverage is invalid",
                            )
                            .into());
                        }
                        physical_universes.insert(producer_ordinal, universe);
                    }
                }
                (LifecycleWorkClass::CertifiedServe, None) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "recovered Serve lacks its producer companion",
                    )
                    .into());
                }
                (_, Some(_)) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "non-Serve candidate carries a producer companion",
                    )
                    .into());
                }
                (_, None) => {}
            }
        }
        drop(terminal_updates_by_ordinal);

        let mut required_candidates = BTreeSet::new();
        let mut required_producers = BTreeSet::new();
        for record in ledger.records() {
            let terminal = record
                .terminal()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "durable terminal cannot be decoded",
                ))?;
            match (record.work_class(), terminal) {
                (Some(LifecycleWorkClass::ProducerTurn), None) => {
                    required_producers.insert(record.ordinal());
                }
                (Some(_), None) => {
                    required_candidates.insert(record.ordinal());
                }
                (Some(LifecycleWorkClass::CertifiedServe), Some(_)) => {
                    if record
                        .ordinal()
                        .checked_add(1)
                        .and_then(|ordinal| ledger_record_at(&ledger, ordinal))
                        .is_some_and(|producer| producer.terminal().flatten().is_none())
                    {
                        required_candidates.insert(record.ordinal());
                    }
                }
                (Some(_), Some(_)) => {}
                (None, _) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "durable work class cannot be decoded",
                    )
                    .into());
                }
            }
        }
        if required_candidates != candidates_by_ordinal.keys().copied().collect()
            || required_producers != producer_coverage
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "live durable record coverage is not exact",
            )
            .into());
        }

        let snapshot = ledger.recovery_snapshot(physical_universes)?;
        let mut coordinator =
            LifecycleCoordinator::new_with_authority(authority, ledger.high_water());
        coordinator.reconcile_restart(snapshot);
        if coordinator.fault == Some(CoordinatorFault::RecoveryRejected) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "coordinator rejected the reconstructed durable state",
            )
            .into());
        }
        let mut terminal_updates = terminal_updates
            .into_iter()
            .map(|update| (update.ordinal, update))
            .collect::<BTreeMap<_, _>>();
        for (ordinal, candidate) in candidates_by_ordinal {
            if let Some(update) = terminal_updates.remove(&ordinal) {
                coordinator.reconcile_store_ahead_terminal_serve(ordinal, candidate, update)?;
                continue;
            }
            if matches!(
                coordinator.records[&ordinal].state,
                LifecycleState::Terminal(_)
            ) {
                coordinator.rebind_terminal_serve_producer(ordinal, candidate)?;
                continue;
            }
            match coordinator.reduce_admit(AdmissionRequest::Candidate(candidate)) {
                AdmissionDecision::Retry {
                    ordinal: rebound, ..
                } if rebound == ordinal => {}
                _ => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "recovered candidate did not rebind exactly",
                    )
                    .into());
                }
            }
        }
        if !terminal_updates.is_empty() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead Serve transition has no exact candidate owner",
            )
            .into());
        }
        if coordinator.records.values().any(|record| {
            matches!(
                record.state,
                LifecycleState::Waiting(wait)
                    if matches!(wait.source, super::WaitSource::Recovery(_))
            )
        }) {
            return Err(
                LifecycleOpenErrorKind::InvalidRecovery("recovery work remains unbound").into(),
            );
        }
        let certified_serve_registry = PreparedCertifiedServeRegistryBatchV1::from_recovered_pairs(
            &coordinator,
            serve_replay_pairs,
        )
        .map_err(|_| {
            LifecycleOpenErrorKind::InvalidRecovery(
                "Certified-Serve concrete recovery coverage is not exact",
            )
        })?;
        let authenticated_successor = LifecycleLedgerV1::from_coordinator(&coordinator)?;
        if authenticated_successor != ledger {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "recovered coordinator does not reproduce its authenticated LedgerV1 frame",
            )
            .into());
        }
        Ok(PreparedLifecycleCoordinatorOpen {
            coordinator,
            store,
            persisted_predecessor,
            authenticated_successor,
            retained_serve_payloads,
            certified_serve_registry: Some(certified_serve_registry),
        })
    }
    // RECOVERED_WAL_SIGN_BORROWED_OPEN_END

    /// Exercise the test-only rollover state transition in focused reducer tests.
    #[cfg(test)]
    pub(crate) fn rollover(&mut self, snapshot: RolloverSnapshot) {
        self.rollover_inner(snapshot, None);
    }

    /// Exercise test-only rollover with a retained Serve payload store.
    #[cfg(test)]
    pub(crate) fn rollover_with_payload_store(
        &mut self,
        snapshot: RolloverSnapshot,
        payload_store: &mut CertifiedServePayloadStoreV1,
    ) {
        self.rollover_inner(snapshot, Some(payload_store));
    }

    #[cfg(test)]
    fn rollover_inner(
        &mut self,
        snapshot: RolloverSnapshot,
        payload_store: Option<&mut CertifiedServePayloadStoreV1>,
    ) {
        if self.fault.is_some() {
            return;
        }
        if !self.rollover_snapshot_is_exact(&snapshot) {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        if self.ledger_store.is_none() {
            let mut next = self.stage_durable_transaction();
            if snapshot.successor_ledger_root.is_some()
                || !snapshot.serve_cancellations.is_empty()
                || next.retire_for_rollover(&snapshot).is_err()
            {
                self.fault = Some(CoordinatorFault::InvalidRollover);
                return;
            }
            next.activate_successor(snapshot);
            *self = next;
            return;
        }
        let Some(successor_root) = snapshot.successor_ledger_root.as_deref() else {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        };
        if !self.serve_cancellation_receipts_are_exact(&snapshot) {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        let Some(serve_wait_rollbacks) = self.serve_wait_rollback_receipts() else {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        };

        let mut retired = self.stage_durable_transaction();
        if retired.retire_for_rollover(&snapshot).is_err() {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        if !serve_wait_rollbacks.is_empty()
            && payload_store
                .ok_or(())
                .and_then(|store| {
                    store
                        .rollback_pending_batch(&serve_wait_rollbacks)
                        .map_err(|_| ())
                })
                .is_err()
        {
            self.fault = Some(CoordinatorFault::DurabilityFailure);
            return;
        }
        let retired_projection = match LifecycleLedgerV1::from_coordinator(&retired) {
            Ok(ledger) => ledger,
            Err(_) => {
                self.fault = Some(CoordinatorFault::DurabilityFailure);
                return;
            }
        };
        if retired
            .ledger_store
            .as_ref()
            .expect("durable rollover retains its predecessor store")
            .persist(&retired_projection)
            .is_err()
        {
            self.fault = Some(CoordinatorFault::DurabilityFailure);
            return;
        }

        let successor_store =
            match LifecycleLedgerStoreV1::open(successor_root, snapshot.successor_context) {
                Ok((store, existing))
                    if existing.records().is_empty()
                        && existing.producer_debts().is_empty()
                        && (existing.high_water() == 0
                            || existing.high_water() == snapshot.retained_high_water) =>
                {
                    store
                }
                Ok(_) | Err(_) => {
                    retired.fault = Some(CoordinatorFault::DurabilityFailure);
                    *self = retired;
                    return;
                }
            };
        let mut successor = LifecycleCoordinator::new_with_authority(
            snapshot.successor_authority.clone(),
            snapshot.retained_high_water,
        );
        let successor_projection = match LifecycleLedgerV1::from_coordinator(&successor) {
            Ok(ledger) => ledger,
            Err(_) => {
                retired.fault = Some(CoordinatorFault::DurabilityFailure);
                *self = retired;
                return;
            }
        };
        if successor_store.persist(&successor_projection).is_err() {
            retired.fault = Some(CoordinatorFault::DurabilityFailure);
            *self = retired;
            return;
        }
        successor.ledger_store = Some(successor_store);
        *self = successor;
    }

    #[cfg(test)]
    fn rollover_snapshot_is_exact(&self, snapshot: &RolloverSnapshot) -> bool {
        let live_ordinals = self
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                (!matches!(record.state, LifecycleState::Terminal(_))).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        let pending_keys = self
            .admission_waits
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        self.active_lease.is_none()
            && self.active_context == snapshot.retired_context
            && snapshot.successor_context.id != snapshot.retired_context.id
            && snapshot.successor_predecessor == snapshot.retired_context.id
            && snapshot.successor_authority.context() == snapshot.successor_context
            && snapshot.retired_context.height.checked_add(1)
                == Some(snapshot.successor_context.height)
            && snapshot.retained_high_water == self.high_water
            && snapshot.retire_ordinals == live_ordinals
            && snapshot.retire_admission_keys == pending_keys
    }

    #[cfg(test)]
    fn serve_cancellation_receipts_are_exact(&self, snapshot: &RolloverSnapshot) -> bool {
        let mut cancellations = BTreeMap::new();
        for receipt in &snapshot.serve_cancellations {
            if receipt.outcome() != CertifiedServePayloadNegativeOutcome::Cancelled {
                return false;
            }
            let request = digest_bytes(receipt.id().request_hash().as_ref());
            let certificate = digest_bytes(receipt.certificate_hash().as_ref());
            if cancellations.insert(request, certificate).is_some() {
                return false;
            }
        }
        let mut expected = BTreeMap::new();
        for record in self.records.values().filter(|record| {
            record.work_class == LifecycleWorkClass::CertifiedServe
                && !matches!(record.state, LifecycleState::Terminal(_))
        }) {
            let DurablePayloadReference::CertifiedServePending {
                request,
                certificate,
            } = self.durable_records[&record.ordinal].payload
            else {
                return false;
            };
            if expected.insert(request, certificate).is_some() {
                return false;
            }
        }
        expected == cancellations
    }

    #[cfg(test)]
    fn serve_wait_rollback_receipts(&self) -> Option<Vec<DurableCertifiedServeAdmissionReceipt>> {
        let mut receipts = Vec::new();
        for waiting in self.admission_waits.values() {
            match (waiting.candidate.work_class, waiting.serve_payload_receipt) {
                (LifecycleWorkClass::CertifiedServe, Some(receipt)) => receipts.push(receipt),
                (LifecycleWorkClass::CertifiedServe, None) | (_, Some(_)) => return None,
                (_, None) => {}
            }
        }
        Some(receipts)
    }

    #[cfg(test)]
    fn retire_for_rollover(&mut self, snapshot: &RolloverSnapshot) -> Result<(), CoordinatorFault> {
        let cancellations = snapshot
            .serve_cancellations
            .iter()
            .map(|receipt| (digest_bytes(receipt.id().request_hash().as_ref()), *receipt))
            .collect::<BTreeMap<_, _>>();
        for ordinal in &snapshot.retire_ordinals {
            let Some(record) = self.records.get(ordinal) else {
                return Err(CoordinatorFault::InvalidRollover);
            };
            if !matches!(record.state, LifecycleState::Terminal(_))
                && record.work_class == LifecycleWorkClass::CertifiedServe
                && self.ledger_store.is_some()
            {
                let DurablePayloadReference::CertifiedServePending { request, .. } = self
                    .durable_records
                    .get(ordinal)
                    .ok_or(CoordinatorFault::InvalidRollover)?
                    .payload
                else {
                    return Err(CoordinatorFault::InvalidRollover);
                };
                let receipt = cancellations
                    .get(&request)
                    .copied()
                    .ok_or(CoordinatorFault::InvalidRollover)?;
                let producer_ordinal = self
                    .producer_debts
                    .get(ordinal)
                    .copied()
                    .ok_or(CoordinatorFault::InvalidRollover)?;
                let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(
                    self.active_context,
                    &self.records[ordinal],
                    &self.durable_records[ordinal],
                    self.records
                        .get(&producer_ordinal)
                        .ok_or(CoordinatorFault::InvalidRollover)?,
                    self.durable_records
                        .get(&producer_ordinal)
                        .ok_or(CoordinatorFault::InvalidRollover)?,
                    receipt,
                )
                .ok_or(CoordinatorFault::InvalidRollover)?;
                let (payload, outcome, serve_replay, producer_replay) =
                    terminal.consume_terminal_rebind();
                if outcome != TerminalOutcome::Cancelled {
                    return Err(CoordinatorFault::InvalidRollover);
                }
                let metadata = self
                    .durable_records
                    .get_mut(ordinal)
                    .expect("rollover preflight retained Serve metadata");
                metadata.payload = payload;
                metadata.replay_authority = serve_replay;
                self.durable_records
                    .get_mut(&producer_ordinal)
                    .expect("rollover preflight retained ProducerTurn metadata")
                    .replay_authority = producer_replay;
            }
            if !self
                .records
                .get(ordinal)
                .is_some_and(|record| matches!(record.state, LifecycleState::Terminal(_)))
            {
                self.finish_terminal(*ordinal, TerminalOutcome::Cancelled)?;
            }
        }
        for key in &snapshot.retire_admission_keys {
            self.admission_waits.remove(key);
        }
        if !self.producer_debts.is_empty()
            || self.capacity_used.values().any(|used| *used != 0)
            || self
                .records
                .values()
                .any(|record| !matches!(record.state, LifecycleState::Terminal(_)))
        {
            return Err(CoordinatorFault::InvalidRollover);
        }
        Ok(())
    }

    #[cfg(test)]
    fn activate_successor(&mut self, snapshot: RolloverSnapshot) {
        self.records.clear();
        self.key_index.clear();
        self.ready_index.clear();
        self.owner_index.clear();
        self.durable_records.clear();
        self.producer_debts.clear();
        self.observed_generation.clear();
        self.capacity_generation
            .values_mut()
            .for_each(|generation| *generation = 0);
        self.next_lease = Some(1);
        self.capacity_geometry = snapshot.successor_authority.capacity_geometry().clone();
        self.episode_authority = snapshot.successor_authority;
        self.active_context = snapshot.successor_context;
    }

    fn rebind_terminal_serve_producer(
        &mut self,
        serve_ordinal: u128,
        mut candidate: CandidateAdmission,
    ) -> Result<(), LifecycleOpenError> {
        candidate.canonicalize_geometry().map_err(|_| {
            LifecycleOpenErrorKind::InvalidRecovery("invalid terminal Serve geometry")
        })?;
        let serve =
            self.records
                .get(&serve_ordinal)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve record disappeared",
                ))?;
        if serve.work_class != LifecycleWorkClass::CertifiedServe
            || !matches!(serve.state, LifecycleState::Terminal(_))
            || serve.key != candidate.key
            || serve.owner.causal_root() != candidate.causal_root
            || serve.stage != candidate.stage
            || !self.durable_records[&serve_ordinal].matches_admission(&candidate)
            || !self.retry_companion_matches(serve, &candidate)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Serve recovery companion changed semantics",
            )
            .into());
        }
        let producer_ordinal = self.producer_debts.get(&serve_ordinal).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("terminal Serve has no live producer debt"),
        )?;
        let producer =
            candidate
                .producer_turn
                .as_ref()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve lacks producer geometry",
                ))?;
        let (physical, universe, consumed) = producer
            .physical_geometry
            .normalized()
            .map_err(|_| LifecycleOpenErrorKind::InvalidRecovery("invalid producer geometry"))?;
        let record = self.records.get_mut(&producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("terminal Serve producer disappeared"),
        )?;
        if record.episode.slot_universe != universe
            || !record.physical_slots.is_empty()
            || !record.episode.consumed_slots.is_empty()
            || !matches!(record.state, LifecycleState::Ready)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Serve producer cannot be rebound",
            )
            .into());
        }
        record.physical_slots = physical;
        record.episode.consumed_slots = consumed;
        Ok(())
    }
}

fn validate_terminal_validate_no_successor_recovery(
    ledger: &LifecycleLedgerV1,
    recovered: &BTreeMap<LifecycleKey, AuthenticatedValidateNoSuccessorRecovery>,
) -> Result<(), LifecycleOpenError> {
    let mut expected = BTreeMap::new();
    for record in ledger.records() {
        if record.work_class() != Some(LifecycleWorkClass::Validate)
            || record.terminal() != Some(Some(TerminalOutcome::Advanced))
            || record.continuation() != Some(DurableContinuation::AdvancedNoSuccessor)
        {
            continue;
        }
        let key = record.key().ok_or(LifecycleOpenErrorKind::InvalidRecovery(
            "terminal Validate key cannot be decoded",
        ))?;
        let proof = AuthenticatedValidateNoSuccessorRecovery {
            key,
            causal_root: record.owner().causal_root(),
            reconstruction_source: record.reconstruction_source(),
            stage: record
                .stage()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Validate stage cannot be decoded",
                ))?,
            payload: record
                .durable_payload()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Validate body-frame payload cannot be decoded",
                ))?,
        };
        if expected.insert(key, proof).is_some() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Validate recovery identity is duplicated",
            )
            .into());
        }
    }
    if &expected != recovered {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "terminal Validate no-successor recovery coverage is not exact",
        )
        .into());
    }
    Ok(())
}

fn decoded_records_by_key(
    ledger: &super::ledger::LifecycleLedgerV1,
) -> Result<BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>, LifecycleOpenError> {
    let mut records = BTreeMap::new();
    for record in ledger.records() {
        let key = record.key().ok_or(LifecycleOpenErrorKind::InvalidRecovery(
            "durable key cannot be decoded",
        ))?;
        if records.insert(key, record).is_some() {
            return Err(
                LifecycleOpenErrorKind::InvalidRecovery("duplicate durable semantic key").into(),
            );
        }
    }
    Ok(records)
}

fn ledger_record_at(
    ledger: &super::ledger::LifecycleLedgerV1,
    ordinal: u128,
) -> Option<&LifecycleLedgerRecordV1> {
    ledger
        .records()
        .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
        .ok()
        .and_then(|index| ledger.records().get(index))
}

fn digest_bytes(bytes: &[u8]) -> LifecycleDigest {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(bytes);
    LifecycleDigest::new(digest)
}

fn validate_candidate_record(
    context: LifecycleContext,
    record: &LifecycleLedgerRecordV1,
    candidate: &CandidateAdmission,
    terminal_update: Option<&TerminalUpdate>,
) -> Result<(), LifecycleOpenError> {
    let replay_matches = terminal_update.map_or_else(
        || record.replay_matches_candidate(candidate),
        |update| {
            update.ordinal == record.ordinal()
                && update
                    .payload
                    .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(update.outcome))
                && update
                    .replay
                    .exactly_matches_recovered_candidate(context, candidate)
        },
    );
    if !candidate.replay_authority_is_exact(context)
        || record.owner().causal_root() != candidate.causal_root
        || record.work_class() != Some(candidate.work_class)
        || record.stage() != Some(candidate.stage)
        || record.reconstruction_source() != candidate.reconstruction_source
        || record
            .durable_payload()
            .is_none_or(|payload| !payload.same_admission_material(candidate.payload))
        || !replay_matches
        || candidate.initial_state != super::InitialLifecycleState::Ready
    {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "recovered candidate changed durable semantics",
        )
        .into());
    }
    Ok(())
}

struct TerminalUpdate {
    ordinal: u128,
    outcome: TerminalOutcome,
    payload: DurablePayloadReference,
    replay: CertifiedServeTerminalReplayAuthorityPairV1,
}

/// One payload-store-ahead terminal transition bound to its exact ledger pair.
///
/// The only consuming surface rechecks the immutable source rows before
/// releasing the values which the ledger module must install. There is no raw
/// constructor or parts accessor.
#[must_use = "the authenticated Serve terminal update must be applied or dropped"]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct CompleteTipServeTerminalUpdateV1 {
    context: LifecycleContext,
    source_serve: LifecycleLedgerRecordV1,
    source_producer: LifecycleLedgerRecordV1,
    terminal: TerminalUpdate,
}

impl CompleteTipServeTerminalUpdateV1 {
    fn exactly_matches_pair(
        &self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> bool {
        self.source_serve == *serve
            && self.source_producer == *producer
            && self.terminal.ordinal == serve.ordinal()
            && self.terminal.payload.matches_terminal(
                LifecycleWorkClass::CertifiedServe,
                Some(self.terminal.outcome),
            )
            && serve.replay_is_exact_pending_predecessor(
                self.context,
                producer,
                &self.terminal.replay,
            )
    }

    /// Consume this update only for the exact Pending Serve/Producer source pair.
    ///
    /// The returned tuple is the fixed ledger mutation payload: terminal Serve
    /// payload, terminal outcome, Serve replay authority, and Producer replay
    /// authority, in that order. It is unavailable for substituted rows.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn consume_for_exact_ledger_pair(
        self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> Option<(
        DurablePayloadReference,
        TerminalOutcome,
        LifecycleReplayAuthorityV1,
        LifecycleReplayAuthorityV1,
    )> {
        if !self.exactly_matches_pair(serve, producer) {
            return None;
        }
        let expected_payload = self.terminal.payload;
        let expected_outcome = self.terminal.outcome;
        let parts = self.terminal.replay.consume_terminal_rebind();
        (parts.0 == expected_payload && parts.1 == expected_outcome).then_some(parts)
    }
}

/// Move-only CompleteTip reconciliation of one final payload cut and ledger frame.
///
/// The authenticated payload cut remains owned by this seal. Every final-cut
/// ID has exactly one ledger Serve owner, every live Serve has one terminal
/// update, and a terminal Serve whose adjacent Producer remains live has one
/// explicit no-update coverage entry. Callers can neither reconstruct updates
/// nor detach the underlying payload authentication.
#[must_use = "CompleteTip Serve reconciliation must be consumed by ledger retirement"]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct CompleteTipServeRetirementReconciliationV1
{
    source_context: LifecycleContext,
    source_frame_identity: LifecycleDigest,
    terminal_updates: BTreeMap<u128, CompleteTipServeTerminalUpdateV1>,
    terminal_serve_live_producers:
        BTreeMap<u128, (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1)>,
    _authenticated_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
}

impl CompleteTipServeRetirementReconciliationV1 {
    /// Check that a ledger is byte-identical to the frame authenticated here.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn authenticates_source(
        &self,
        ledger: &LifecycleLedgerV1,
    ) -> bool {
        ledger.context() == self.source_context
            && ledger.frame_identity() == self.source_frame_identity
    }

    /// Remove the sole terminal transition for an exact live Serve pair.
    ///
    /// A mismatched or already-consumed pair leaves the reconciliation intact.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn take_terminal_update_for_exact_pair(
        &mut self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> Option<CompleteTipServeTerminalUpdateV1> {
        let ordinal = serve.ordinal();
        self.terminal_updates
            .get(&ordinal)
            .is_some_and(|update| update.exactly_matches_pair(serve, producer))
            .then(|| {
                self.terminal_updates
                    .remove(&ordinal)
                    .expect("the exact update remained present")
            })
    }

    /// Consume no-update coverage for a terminal Serve with a live Producer.
    ///
    /// Retirement still has to terminalize the Producer and discharge its
    /// debt, but must not rewrite the already-terminal Serve payload.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn take_terminal_serve_live_producer_coverage(
        &mut self,
        serve: &LifecycleLedgerRecordV1,
        producer: &LifecycleLedgerRecordV1,
    ) -> bool {
        let ordinal = serve.ordinal();
        let exact = self
            .terminal_serve_live_producers
            .get(&ordinal)
            .is_some_and(|(expected_serve, expected_producer)| {
                expected_serve == serve && expected_producer == producer
            });
        if exact {
            self.terminal_serve_live_producers.remove(&ordinal);
        }
        exact
    }

    /// Return true after every required Serve action or coverage proof was consumed.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn is_drained(&self) -> bool {
        self.terminal_updates.is_empty() && self.terminal_serve_live_producers.is_empty()
    }
}

#[allow(clippy::too_many_lines)]
fn resolve_serve_payloads(
    context: LifecycleContext,
    ledger: &LifecycleLedgerV1,
    records: &BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>,
    recovered: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<
    (
        Vec<CandidateAdmission>,
        Vec<TerminalUpdate>,
        BTreeSet<CertifiedServePayloadId>,
        BTreeMap<LifecycleKey, super::replay_authority::CertifiedServeReplayEvidencePairV1>,
    ),
    LifecycleOpenError,
> {
    if digest_bytes(recovered.context_id().0.as_ref()) != context.id()
        || recovered.height() != context.height()
    {
        return Err(
            LifecycleOpenErrorKind::InvalidRecovery("foreign Certified-Serve payload cut").into(),
        );
    }
    let mut recovered_by_request = BTreeMap::new();
    for payload in recovered.iter() {
        let request = digest_bytes(payload.id().request_hash().as_ref());
        if recovered_by_request.insert(request, payload).is_some() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "duplicate authenticated Serve request identity",
            )
            .into());
        }
    }

    let mut candidates = Vec::new();
    let mut updates = Vec::new();
    let mut retained = BTreeSet::new();
    let mut replay_pairs = BTreeMap::new();
    for (key, record) in records {
        if record.work_class() != Some(LifecycleWorkClass::CertifiedServe) {
            continue;
        }
        let durable = record
            .durable_payload()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve payload cannot be decoded",
            ))?;
        let request = durable
            .request()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve ledger row lost its signed-request identity",
            ))?;
        let payload = recovered_by_request.get(&request).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("Serve payload is missing from storage"),
        )?;
        retained.insert(payload.id());
        let (candidate, resolved, projected_terminal, projected_replay, replay_pair) =
            super::projection::recovered_certified_serve_projection(context, payload)
                .map_err(|_| {
                    LifecycleOpenErrorKind::InvalidRecovery(
                        "authenticated Serve payload could not be projected",
                    )
                })?
                .into_registry_parts();
        if candidate.key != *key {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve six-field key changed its body/request identity",
            )
            .into());
        }
        if !durable.same_admission_material(resolved) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve payload changed request or certificate identity",
            )
            .into());
        }
        let ledger_terminal = record
            .terminal()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve terminal cannot be decoded",
            ))?;
        if durable == resolved {
            if ledger_terminal != projected_terminal {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "Serve payload state disagrees with its ledger terminal",
                )
                .into());
            }
            let producer =
                candidate
                    .producer_turn
                    .as_ref()
                    .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                        "steady Serve recovery lost its producer replay authority",
                    ))?;
            let producer_record = record
                .ordinal()
                .checked_add(1)
                .and_then(|ordinal| ledger_record_at(ledger, ordinal))
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "steady Serve recovery lost its adjacent producer row",
                ))?;
            if !record.replay_matches_candidate(&candidate)
                || !producer_record.replay_matches_producer(producer)
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "steady Serve recovery frame changed its exact persisted family",
                )
                .into());
            }
            if projected_terminal.is_some() != projected_replay.is_some() {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve recovery lost its exact replay family",
                )
                .into());
            }
        } else {
            let outcome = match (durable, resolved, projected_terminal) {
                (
                    DurablePayloadReference::CertifiedServePending { .. },
                    DurablePayloadReference::CertifiedServeCompleted { response, .. },
                    Some(TerminalOutcome::Completed(Some(projected_response))),
                ) if response == projected_response
                    && matches!(
                        payload.state(),
                        AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed)
                            if completed.permits_payload_store_ahead_terminal_rebind()
                    ) =>
                {
                    TerminalOutcome::Completed(Some(response))
                }
                (
                    DurablePayloadReference::CertifiedServePending { .. },
                    DurablePayloadReference::CertifiedServeNegative { outcome, .. },
                    Some(projected),
                ) if outcome.terminal() == projected => projected,
                _ => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "Serve payload storage regressed or conflicts with the ledger",
                    )
                    .into());
                }
            };
            if ledger_terminal.is_some() {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve payload disagrees with its ledger tombstone",
                )
                .into());
            }
            let replay = projected_replay.ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "payload-store-ahead Serve lost its terminal replay family",
            ))?;
            updates.push(TerminalUpdate {
                ordinal: record.ordinal(),
                outcome,
                payload: resolved,
                replay,
            });
        }

        let producer_is_live = record
            .ordinal()
            .checked_add(1)
            .and_then(|ordinal| ledger_record_at(ledger, ordinal))
            .is_some_and(|producer| {
                producer.work_class() == Some(LifecycleWorkClass::ProducerTurn)
                    && producer.terminal() == Some(None)
            });
        if ledger_terminal.is_none() || producer_is_live {
            if replay_pairs.insert(candidate.key, replay_pair).is_some() {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "duplicate Certified-Serve concrete replay family",
                )
                .into());
            }
            candidates.push(candidate);
        }
    }
    if recovered.iter().any(|payload| {
        !retained.contains(&payload.id())
            && !matches!(
                payload.state(),
                AuthenticatedRecoveredCertifiedServePayloadState::Pending
            )
    }) {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "terminal Serve payload has no durable ledger owner",
        )
        .into());
    }
    Ok((candidates, updates, retained, replay_pairs))
}

/// Authenticate the complete predecessor Serve/payload census for CompleteTip retirement.
///
/// This comparison performs no payload or ledger mutation. It accepts
/// payload-store-ahead terminal frames so the consuming retirement transaction
/// can reconcile them, while rejecting a terminal orphan or any missing,
/// duplicate, foreign, or semantically drifted Serve owner.
pub(super) fn authenticate_complete_tip_serve_census(
    ledger: &LifecycleLedgerV1,
    recovered: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<BTreeSet<CertifiedServePayloadId>, LifecycleOpenError> {
    let mut records = BTreeMap::new();
    for record in ledger.records() {
        let key = record.key().ok_or(LifecycleOpenErrorKind::InvalidRecovery(
            "CompleteTip predecessor has an undecodable lifecycle key",
        ))?;
        if records.insert(key, record).is_some() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip predecessor has duplicate lifecycle keys",
            )
            .into());
        }
    }
    let (_, _, retained, _) =
        resolve_serve_payloads(ledger.context(), ledger, &records, recovered)?;
    Ok(retained)
}

/// Seal the final post-mutation Serve cut for CompleteTip ledger retirement.
///
/// Unlike the pre-mutation census, this boundary permits no Pending orphan:
/// every payload ID in the final authenticated cut must be retained by one
/// exact ledger Serve. Every live Serve must resolve through a payload-store-
/// ahead terminal update, while an already-terminal Serve may contribute only
/// explicit coverage for its still-live adjacent Producer.
///
/// # Errors
///
/// Returns an error when the final cut is foreign, incomplete, still contains
/// any unowned payload, or cannot cover the exact Serve/Producer inventory.
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn reconcile_complete_tip_serve_retirement(
    ledger: &LifecycleLedgerV1,
    recovered: AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<CompleteTipServeRetirementReconciliationV1, LifecycleOpenError> {
    let records = decoded_records_by_key(ledger)?;
    let (serve_candidates, terminal_updates, retained, _replay_pairs) =
        resolve_serve_payloads(ledger.context(), ledger, &records, &recovered)?;
    validate_storage_only_serve_coverage(ledger, &records, &serve_candidates, &terminal_updates)?;

    let final_cut_ids = recovered
        .iter()
        .map(|payload| payload.id())
        .collect::<BTreeSet<_>>();
    if retained != final_cut_ids {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "CompleteTip final Serve cut contains an unowned payload",
        )
        .into());
    }

    let mut expected_terminal_updates = BTreeSet::new();
    let mut expected_terminal_serve_live_producers = BTreeSet::new();
    for serve in ledger
        .records()
        .iter()
        .filter(|record| record.work_class() == Some(LifecycleWorkClass::CertifiedServe))
    {
        let producer_ordinal =
            serve
                .ordinal()
                .checked_add(1)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "CompleteTip Serve producer ordinal overflowed",
                ))?;
        let producer = ledger_record_at(ledger, producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("CompleteTip Serve lost its adjacent Producer"),
        )?;
        let serve_terminal = serve
            .terminal()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip Serve terminal cannot be decoded",
            ))?;
        let producer_terminal =
            producer
                .terminal()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "CompleteTip Producer terminal cannot be decoded",
                ))?;
        if serve_terminal.is_none() {
            expected_terminal_updates.insert(serve.ordinal());
        } else if producer_terminal.is_none() {
            expected_terminal_serve_live_producers.insert(serve.ordinal());
        }
    }

    let mut updates = BTreeMap::new();
    for terminal in terminal_updates {
        let serve = ledger_record_at(ledger, terminal.ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip terminal update lost its Serve row",
            ),
        )?;
        let producer_ordinal =
            terminal
                .ordinal
                .checked_add(1)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "CompleteTip terminal update producer ordinal overflowed",
                ))?;
        let producer = ledger_record_at(ledger, producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip terminal update lost its Producer row",
            ),
        )?;
        let update = CompleteTipServeTerminalUpdateV1 {
            context: ledger.context(),
            source_serve: serve.clone(),
            source_producer: producer.clone(),
            terminal,
        };
        if !update.exactly_matches_pair(serve, producer)
            || updates.insert(serve.ordinal(), update).is_some()
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "CompleteTip terminal Serve updates are not an exact ledger pair census",
            )
            .into());
        }
    }
    if updates.keys().copied().collect::<BTreeSet<_>>() != expected_terminal_updates {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "CompleteTip terminal updates do not cover every live Serve exactly",
        )
        .into());
    }

    let mut terminal_serve_live_producers = BTreeMap::new();
    for serve_ordinal in expected_terminal_serve_live_producers {
        let serve =
            ledger_record_at(ledger, serve_ordinal).expect("Serve ordinal came from ledger");
        let producer = ledger_record_at(
            ledger,
            serve_ordinal
                .checked_add(1)
                .expect("validated Serve producer ordinal"),
        )
        .expect("validated Serve retained its adjacent Producer");
        terminal_serve_live_producers.insert(serve_ordinal, (serve.clone(), producer.clone()));
    }

    Ok(CompleteTipServeRetirementReconciliationV1 {
        source_context: ledger.context(),
        source_frame_identity: ledger.frame_identity(),
        terminal_updates: updates,
        terminal_serve_live_producers,
        _authenticated_payloads: recovered,
    })
}

fn validate_storage_only_recovery(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<(), LifecycleRecoveryAssemblyErrorKind> {
    for record in ledger.records() {
        classify_storage_only_record(record)?;
    }
    validate_storage_only_serve_recovery(ledger, serve_payloads)
}

fn terminal_validate_no_successor_claim(
    context: LifecycleContext,
    record: &LifecycleLedgerRecordV1,
) -> Result<Option<TerminalValidateNoSuccessorClaim>, LifecycleRecoveryAssemblyErrorKind> {
    let ordinal = record.ordinal();
    let work_class =
        record
            .work_class()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "work class",
            })?;
    let stage = record
        .stage()
        .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "stage",
        })?;
    let terminal =
        record
            .terminal()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "terminal",
            })?;
    let continuation =
        record
            .continuation()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "continuation",
            })?;
    if work_class != LifecycleWorkClass::Validate
        || terminal != Some(TerminalOutcome::Advanced)
        || continuation != DurableContinuation::AdvancedNoSuccessor
    {
        return Ok(None);
    }
    let key = record
        .key()
        .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "key",
        })?;
    let payload = record.durable_payload().ok_or(
        LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "payload",
        },
    )?;
    Ok(Some(TerminalValidateNoSuccessorClaim {
        context,
        ordinal,
        key,
        causal_root: record.owner().causal_root(),
        reconstruction_source: record.reconstruction_source(),
        stage,
        payload,
    }))
}

fn assemble_storage_only_candidates_and_terminal_validate_claims(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
    recovered_wal: RecoveredWalStartupProjectionV1<'_>,
    mut durable_fetches: Option<&mut PreparedDurableCertifiedFetchStartupV1>,
) -> Result<
    (
        BTreeMap<LifecycleKey, CandidateAdmission>,
        BTreeMap<LifecycleKey, TerminalValidateNoSuccessorClaim>,
    ),
    LifecycleRecoveryAssemblyErrorKind,
> {
    let belongs_to_context = match recovered_wal {
        RecoveredWalStartupProjectionV1::None => true,
        RecoveredWalStartupProjectionV1::PhaseVote(projection) => {
            projection.belongs_to_context(ledger.context())
        }
        RecoveredWalStartupProjectionV1::ControlSign(projection) => {
            projection.belongs_to_context(ledger.context())
        }
        RecoveredWalStartupProjectionV1::DecisionFetch(projection) => {
            projection.belongs_to_context(ledger.context())
        }
        RecoveredWalStartupProjectionV1::DecisionApply(projection) => {
            projection.fetch().belongs_to_context(ledger.context())
        }
    };
    if !belongs_to_context {
        return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
            "installed projection belongs to another lifecycle context",
        ));
    }

    let mut candidates = BTreeMap::new();
    let mut claims = BTreeMap::new();
    for record in ledger.records() {
        match classify_storage_only_record(record) {
            Ok(()) => {}
            Err(LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome { .. }) => {
                let Some(claim) = terminal_validate_no_successor_claim(ledger.context(), record)?
                else {
                    return Err(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                        ordinal: record.ordinal(),
                        field: "terminal Validate recovery claim",
                    });
                };
                if claims.insert(claim.key, claim).is_some() {
                    return Err(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                        ordinal: record.ordinal(),
                        field: "duplicate terminal Validate recovery key",
                    });
                }
            }
            Err(
                kind @ LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                    work_class,
                    ..
                },
            ) => {
                let admitted_recovered_wal = match recovered_wal {
                    RecoveredWalStartupProjectionV1::None => false,
                    RecoveredWalStartupProjectionV1::PhaseVote(projection)
                        if record.key() == Some(projection.child_key()) =>
                    {
                        projection.insert_repaired_child_from_record(
                            ledger.context(),
                            record,
                            &mut candidates,
                        )
                    }
                    RecoveredWalStartupProjectionV1::ControlSign(projection)
                        if projection.names_record(record) =>
                    {
                        projection.splice_candidate_from_record(record, &mut candidates)
                    }
                    RecoveredWalStartupProjectionV1::DecisionFetch(projection)
                        if projection.names_record(record) =>
                    {
                        projection.splice_candidate_from_record(record, &mut candidates)
                    }
                    RecoveredWalStartupProjectionV1::DecisionApply(projection)
                        if work_class == LifecycleWorkClass::Apply =>
                    {
                        splice_recovered_decision_apply_candidate(
                            ledger,
                            projection,
                            record,
                            &mut candidates,
                        )
                    }
                    RecoveredWalStartupProjectionV1::PhaseVote(_)
                    | RecoveredWalStartupProjectionV1::ControlSign(_)
                    | RecoveredWalStartupProjectionV1::DecisionFetch(_)
                    | RecoveredWalStartupProjectionV1::DecisionApply(_) => false,
                };
                if admitted_recovered_wal {
                    continue;
                }
                if work_class == LifecycleWorkClass::Fetch
                    && durable_fetches.is_some()
                    && matches!(
                        record.durable_payload(),
                        Some(DurablePayloadReference::BodyFrame(_))
                    )
                {
                    continue;
                }
                return Err(kind);
            }
            Err(kind) => return Err(kind),
        }
    }

    if let Some(fetches) = durable_fetches.as_mut()
        && !fetches.splice_candidates(ledger, &mut candidates)
    {
        return Err(LifecycleRecoveryAssemblyErrorKind::DurableCertifiedFetch(
            "the frame-bound all-row census did not splice exactly once",
        ));
    }

    match recovered_wal {
        RecoveredWalStartupProjectionV1::PhaseVote(projection) => {
            if !projection.owns_spliced_candidates(&candidates) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live installed phase-vote Sign",
                ));
            }
            if !projection.repaired_pair_is_exact(ledger.context(), ledger.records()) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame lost the exact terminal Validate parent or typed Sign edge",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::ControlSign(projection) => {
            if !projection.owns_spliced_candidate(&candidates) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live installed control Sign",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::DecisionFetch(projection) => {
            if !projection.owns_spliced_candidate(&candidates) {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live installed Decision Fetch",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::DecisionApply(projection) => {
            if !projection
                .lineage()
                .owns_spliced_apply_candidate(&candidates)
                || !recovered_decision_apply_chain_is_exact(ledger, projection)
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "repaired frame has no exact live recovered Decision Apply",
                ));
            }
        }
        RecoveredWalStartupProjectionV1::None => {
            if candidates
                .values()
                .any(|candidate| candidate.work_class != LifecycleWorkClass::Fetch)
            {
                return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                    "storage-only assembly created a Sign without installed authority",
                ));
            }
        }
    }
    validate_storage_only_serve_recovery(ledger, serve_payloads)?;
    Ok((candidates, claims))
}

fn recovered_decision_apply_chain_records<'ledger>(
    ledger: &'ledger LifecycleLedgerV1,
    projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
) -> Option<[&'ledger LifecycleLedgerRecordV1; 4]> {
    let mut fetches = ledger
        .records()
        .iter()
        .filter(|record| projection.fetch().names_record(record));
    let fetch = fetches.next()?;
    if fetches.next().is_some() {
        return None;
    }
    let (DurableContinuationEdge::FetchToStore, store_ordinal) = fetch
        .continuation()
        .and_then(DurableContinuation::successor_parts)?
    else {
        return None;
    };
    let validate_ordinal = store_ordinal.checked_add(1)?;
    let apply_ordinal = validate_ordinal.checked_add(1)?;
    let record_at = |ordinal| {
        ledger
            .records()
            .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| ledger.records().get(index))
    };
    let store = record_at(store_ordinal)?;
    let validate = record_at(validate_ordinal)?;
    let apply = record_at(apply_ordinal)?;
    let owner = fetch.owner();
    (ledger
        .records()
        .iter()
        .filter(|record| record.owner() == owner)
        .count()
        == 4
        && projection
            .fetch()
            .exactly_matches_advanced_apply_parent(fetch, store_ordinal)
        && projection
            .lineage()
            .exactly_matches_successor_records(owner, store, validate, apply))
    .then_some([fetch, store, validate, apply])
}

fn recovered_decision_apply_chain_is_exact(
    ledger: &LifecycleLedgerV1,
    projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
) -> bool {
    recovered_decision_apply_chain_records(ledger, projection).is_some()
}

fn splice_recovered_decision_apply_candidate(
    ledger: &LifecycleLedgerV1,
    projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    current: &LifecycleLedgerRecordV1,
    candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
) -> bool {
    let Some([_fetch, store, validate, apply]) =
        recovered_decision_apply_chain_records(ledger, projection)
    else {
        return false;
    };
    apply.ordinal() == current.ordinal()
        && projection.lineage().splice_apply_candidate_from_records(
            apply.owner(),
            store,
            validate,
            apply,
            candidates,
        )
}

fn assemble_storage_only_recovered_wal_candidates(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
    projection: &AuthenticatedRecoveredWalSignProjection,
) -> Result<BTreeMap<LifecycleKey, CandidateAdmission>, LifecycleRecoveryAssemblyErrorKind> {
    if !projection.belongs_to_context(ledger.context()) {
        return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
            "installed projection belongs to another lifecycle context",
        ));
    }
    let mut candidates = BTreeMap::new();
    for record in ledger.records() {
        match classify_storage_only_record(record) {
            Ok(()) => {}
            Err(LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority { .. })
                if record.key() == Some(projection.child_key()) =>
            {
                if !projection.insert_repaired_child_from_record(
                    ledger.context(),
                    record,
                    &mut candidates,
                ) {
                    return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
                        "live Sign row changed installed owner, ordinal, or admission semantics",
                    ));
                }
            }
            Err(kind) => return Err(kind),
        }
    }
    if !projection.owns_spliced_candidates(&candidates) {
        return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
            "repaired frame has no exact live installed Sign child",
        ));
    }
    if !projection.repaired_pair_is_exact(ledger.context(), ledger.records()) {
        return Err(LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(
            "repaired frame lost the exact terminal Validate parent or typed Sign edge",
        ));
    }
    validate_storage_only_serve_recovery(ledger, serve_payloads)?;
    Ok(candidates)
}

fn validate_storage_only_serve_recovery(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<(), LifecycleRecoveryAssemblyErrorKind> {
    let records = decoded_records_by_key(ledger)
        .map_err(LifecycleRecoveryAssemblyErrorKind::CertifiedServe)?;
    let (serve_candidates, terminal_updates, _retained, _replay_pairs) =
        resolve_serve_payloads(ledger.context(), ledger, &records, serve_payloads)
            .map_err(LifecycleRecoveryAssemblyErrorKind::CertifiedServe)?;
    validate_storage_only_serve_coverage(ledger, &records, &serve_candidates, &terminal_updates)
        .map_err(LifecycleRecoveryAssemblyErrorKind::CertifiedServe)
}

/// Recheck the retained post-prune Serve cut against one exact owner ledger.
pub(super) fn authenticated_serve_payloads_match_ledger(
    ledger: &LifecycleLedgerV1,
    serve_payloads: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> bool {
    validate_storage_only_serve_recovery(ledger, serve_payloads).is_ok()
}

// STORAGE_ONLY_LIFECYCLE_RECOVERY_CLASSIFIER_BEGIN
fn classify_storage_only_record(
    record: &LifecycleLedgerRecordV1,
) -> Result<(), LifecycleRecoveryAssemblyErrorKind> {
    let ordinal = record.ordinal();
    let work_class =
        record
            .work_class()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "work class",
            })?;
    let stage = record
        .stage()
        .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
            ordinal,
            field: "stage",
        })?;
    let terminal =
        record
            .terminal()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "terminal",
            })?;
    let continuation =
        record
            .continuation()
            .ok_or(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                ordinal,
                field: "continuation",
            })?;

    let stage_work_class = match stage.kind() {
        LifecycleStageKind::SignProposal => LifecycleWorkClass::SignProposal,
        LifecycleStageKind::SignPrepareVote | LifecycleStageKind::SignCommitVote => {
            LifecycleWorkClass::SignVote
        }
        LifecycleStageKind::SignTimeoutVote => LifecycleWorkClass::SignTimeout,
        LifecycleStageKind::FetchBody => LifecycleWorkClass::Fetch,
        LifecycleStageKind::StoreBody => LifecycleWorkClass::Store,
        LifecycleStageKind::ValidateBody => LifecycleWorkClass::Validate,
        LifecycleStageKind::ApplyDecision => LifecycleWorkClass::Apply,
        LifecycleStageKind::BroadcastProposal
        | LifecycleStageKind::BroadcastPrepareVote
        | LifecycleStageKind::BroadcastCommitVote
        | LifecycleStageKind::BroadcastPrepareQc
        | LifecycleStageKind::BroadcastCommitQc
        | LifecycleStageKind::BroadcastTimeoutVote
        | LifecycleStageKind::BroadcastTc => LifecycleWorkClass::Broadcast,
        LifecycleStageKind::EnterView => LifecycleWorkClass::EnterView,
        LifecycleStageKind::ReportProposalEquivocation
        | LifecycleStageKind::ReportVoteEquivocation
        | LifecycleStageKind::ReportTimeoutEquivocation => LifecycleWorkClass::EquivocationReport,
        LifecycleStageKind::ReportInvalidBody => LifecycleWorkClass::InvalidBodyReport,
        LifecycleStageKind::CertifiedServe => LifecycleWorkClass::CertifiedServe,
        LifecycleStageKind::ProducerTurn => LifecycleWorkClass::ProducerTurn,
    };
    if stage_work_class != work_class {
        return Err(
            LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecordShape {
                ordinal,
                work_class,
                stage,
            },
        );
    }

    if work_class == LifecycleWorkClass::Validate
        && terminal == Some(TerminalOutcome::Advanced)
        && continuation == DurableContinuation::AdvancedNoSuccessor
    {
        return Err(
            LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome { ordinal, stage },
        );
    }
    match work_class {
        LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn => Ok(()),
        LifecycleWorkClass::SignProposal
        | LifecycleWorkClass::SignVote
        | LifecycleWorkClass::SignTimeout
        | LifecycleWorkClass::Fetch
        | LifecycleWorkClass::Store
        | LifecycleWorkClass::Validate
        | LifecycleWorkClass::Apply
        | LifecycleWorkClass::Broadcast
        | LifecycleWorkClass::EnterView
        | LifecycleWorkClass::EquivocationReport
        | LifecycleWorkClass::InvalidBodyReport => terminal.map_or_else(
            || {
                Err(
                    LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                        ordinal,
                        work_class,
                        stage,
                    },
                )
            },
            |_| Ok(()),
        ),
    }
}
// STORAGE_ONLY_LIFECYCLE_RECOVERY_CLASSIFIER_END

fn validate_storage_only_serve_coverage(
    ledger: &LifecycleLedgerV1,
    records: &BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>,
    candidates: &[CandidateAdmission],
    terminal_updates: &[TerminalUpdate],
) -> Result<(), LifecycleOpenError> {
    let mut covered_serves = BTreeSet::new();
    let mut covered_producers = BTreeSet::new();
    let terminal_updates = terminal_updates
        .iter()
        .map(|update| (update.ordinal, update))
        .collect::<BTreeMap<_, _>>();
    for candidate in candidates {
        if candidate.work_class != LifecycleWorkClass::CertifiedServe {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only payload projection produced non-Serve work",
            )
            .into());
        }
        let record =
            records
                .get(&candidate.key)
                .copied()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only Serve projection has no durable owner",
                ))?;
        let terminal_update = terminal_updates.get(&record.ordinal()).copied();
        validate_candidate_record(ledger.context(), record, candidate, terminal_update)?;
        if !covered_serves.insert(record.ordinal()) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only Serve projection duplicated one ledger row",
            )
            .into());
        }
        let producer_ordinal =
            record
                .ordinal()
                .checked_add(1)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only Serve producer ordinal overflowed",
                ))?;
        let producer_record = ledger_record_at(ledger, producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only Serve projection lost its adjacent producer",
            ),
        )?;
        let producer =
            candidate
                .producer_turn
                .as_ref()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only Serve projection lacks its producer companion",
                ))?;
        let replay_matches = terminal_update.map_or_else(
            || producer_record.replay_matches_producer(producer),
            |update| {
                update
                    .replay
                    .exactly_matches_recovered_candidate(ledger.context(), candidate)
                    && record.replay_is_exact_pending_predecessor(
                        ledger.context(),
                        producer_record,
                        &update.replay,
                    )
            },
        );
        if producer_record.key() != Some(producer.key)
            || producer_record.owner() != record.owner()
            || producer_record.work_class() != Some(LifecycleWorkClass::ProducerTurn)
            || producer_record.stage() != Some(producer.stage)
            || producer_record.reconstruction_source() != producer.reconstruction_source
            || !replay_matches
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only producer projection changed durable semantics",
            )
            .into());
        }
        if producer_record.terminal().flatten().is_none()
            && !covered_producers.insert(producer_ordinal)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only producer projection duplicated one ledger row",
            )
            .into());
        }
    }

    let mut expected_serves = BTreeSet::new();
    let mut expected_producers = BTreeSet::new();
    for record in ledger.records() {
        let terminal = record
            .terminal()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "storage-only coverage cannot decode durable terminal",
            ))?;
        match record.work_class() {
            Some(LifecycleWorkClass::CertifiedServe) => {
                let producer_is_live = record
                    .ordinal()
                    .checked_add(1)
                    .and_then(|ordinal| ledger_record_at(ledger, ordinal))
                    .is_some_and(|producer| producer.terminal().flatten().is_none());
                if terminal.is_none() || producer_is_live {
                    expected_serves.insert(record.ordinal());
                }
            }
            Some(LifecycleWorkClass::ProducerTurn) if terminal.is_none() => {
                expected_producers.insert(record.ordinal());
            }
            Some(
                LifecycleWorkClass::SignProposal
                | LifecycleWorkClass::SignVote
                | LifecycleWorkClass::SignTimeout
                | LifecycleWorkClass::Fetch
                | LifecycleWorkClass::Store
                | LifecycleWorkClass::Validate
                | LifecycleWorkClass::Apply
                | LifecycleWorkClass::Broadcast
                | LifecycleWorkClass::EnterView
                | LifecycleWorkClass::EquivocationReport
                | LifecycleWorkClass::InvalidBodyReport
                | LifecycleWorkClass::ProducerTurn,
            ) => {}
            None => {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "storage-only coverage cannot decode durable work class",
                )
                .into());
            }
        }
    }
    if covered_serves != expected_serves || covered_producers != expected_producers {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "storage-only Serve/producer recovery coverage is not exact",
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod recovery_tests {
    #[cfg(feature = "bls")]
    use std::num::NonZeroU64;

    #[cfg(feature = "bls")]
    use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
    #[cfg(feature = "bls")]
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
        peer::PeerId,
    };
    #[cfg(feature = "bls")]
    use tempfile::TempDir;

    #[cfg(feature = "bls")]
    use super::super::schema::DurableContinuationEdge;
    use super::super::schema::{
        CausalRoot, DurableContinuation, LifecyclePhase, LifecycleRound, LifecycleStageKind,
        OwnerId, PredecessorScope,
    };
    use super::*;
    #[cfg(feature = "bls")]
    use crate::sumeragi::{
        v2_body_store::{V2BodyStore, ValidatedBodyReceipt},
        v2_chunks::encode_payload,
    };

    #[cfg(feature = "bls")]
    struct EmptyAuthenticatedPayloadFixture {
        context: LifecycleContext,
        verified: VerifiedHeightContext,
        root: TempDir,
        payload_store: CertifiedServePayloadStoreV1,
        payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        body_store: V2BodyStore,
        keys: Vec<KeyPair>,
    }

    #[cfg(feature = "bls")]
    fn empty_authenticated_payload_fixture() -> EmptyAuthenticatedPayloadFixture {
        let mut keys = (0xC1_u8..=0xC4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic assembler BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id(
                "storage-only-lifecycle-recovery-assembler-test",
            ),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"storage-only recovery AMX context"),
            execution_policy_hash: Hash::new(b"storage-only recovery execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_048_576,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1_048_576,
                max_chunk_count: 2,
            },
            leader_seed: [0xC5; 32],
        };
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture BLS proof of possession")
            })
            .collect();
        let verified = VerifiedHeightContext::genesis(context, proofs)
            .expect("verify assembler height context");
        let root = TempDir::new().expect("temporary assembler storage root");
        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.path().join("body"),
            verified.context().clone(),
        )
        .expect("open empty body store");
        let (payload_store, recovered) =
            CertifiedServePayloadStoreV1::open(&root.path().join("payload"), verified.context())
                .expect("open empty payload store");
        let payloads = recovered
            .authenticate(&verified, &keys[0], &body_store)
            .expect("authenticate empty payload cut");
        EmptyAuthenticatedPayloadFixture {
            context: super::super::projection::lifecycle_context(verified.context()),
            verified,
            root,
            payload_store,
            payloads,
            body_store,
            keys,
        }
    }

    #[cfg(feature = "bls")]
    fn empty_authenticated_payload_cut() -> (
        LifecycleContext,
        AuthenticatedCertifiedServePayloadRecoveryCut,
    ) {
        let EmptyAuthenticatedPayloadFixture {
            context, payloads, ..
        } = empty_authenticated_payload_fixture();
        (context, payloads)
    }

    #[cfg(feature = "bls")]
    fn terminal_validate_record_with_body_outcome(
        fixture: &mut EmptyAuthenticatedPayloadFixture,
        ordinal: u128,
        view: u64,
        rejected: bool,
    ) -> LifecycleLedgerRecordV1 {
        let wire_context = fixture.verified.context();
        let round = wire::ConsensusRound {
            context_id: wire_context.id(),
            height: wire_context.height,
            view,
        };
        let leader = wire_context.leader(view);
        let leader_index = usize::try_from(leader).expect("small fixture leader index");
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            1_000_u64.saturating_add(view),
            view,
        );
        let signature =
            SignatureOf::try_from_hash(fixture.keys[leader_index].private_key(), header.hash())
                .expect("sign terminal Validate body");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block.encode_wire().expect("encode terminal Validate body");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = encode_payload(wire_context, round, subject, &canonical_wire)
            .expect("encode terminal Validate payload")
            .manifest()
            .clone();
        let receipt = fixture
            .body_store
            .store(manifest.clone(), canonical_wire)
            .expect("persist terminal Validate body");
        let replay = super::super::replay_authority::exact_local_body_record_fixture(
            fixture.context,
            crate::sumeragi::v2_core::EventTag::new(
                round.height,
                round.view,
                crate::sumeragi::v2_core::Generation::new(0),
            ),
            manifest,
            &receipt,
            LifecycleStageKind::ValidateBody,
        )
        .expect("exact stored body mints one canonical Validate fixture");
        if rejected {
            let _rejected = fixture
                .body_store
                .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                    Err::<wire::ExecutionCommitment, _>(
                        "deterministic terminal Validate rejection".to_owned(),
                    )
                })
                .expect("persist terminal Validate rejection");
        } else {
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = fixture
                .body_store
                .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                    Ok::<_, String>(commitment)
                })
                .expect("persist terminal Validate success");
        }

        let causal_root = CausalRoot::new(LifecycleDigest::new(
            [u8::try_from(ordinal).expect("small fixture ordinal"); 32],
        ));
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            replay.work_class,
            replay.stage,
            Some(TerminalOutcome::Advanced),
            causal_root.digest(),
            replay.payload,
            replay.authority,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct terminal Validate body-outcome record")
    }

    #[cfg(feature = "bls")]
    fn sign_proposal_record(
        context: LifecycleContext,
        ordinal: u128,
        marker: u8,
        terminal: Option<TerminalOutcome>,
    ) -> LifecycleLedgerRecordV1 {
        let causal_root = CausalRoot::new(LifecycleDigest::new([marker; 32]));
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::SignProposal,
            u8::try_from(ordinal).expect("small SignProposal fixture ordinal"),
        );
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            replay.work_class,
            replay.stage,
            terminal,
            causal_root.digest(),
            replay.payload,
            replay.authority,
            DurableContinuation::None,
        )
        .expect("construct SignProposal record")
    }

    #[cfg(feature = "bls")]
    fn live_sign_proposal_ledger(context: LifecycleContext) -> LifecycleLedgerV1 {
        let record = sign_proposal_record(context, 1, 0xC6, None);
        LifecycleLedgerV1::new(context, 1, vec![record], BTreeMap::new())
            .expect("construct live SignProposal ledger")
    }

    #[cfg(feature = "bls")]
    fn live_synthetic_serve_ledger(context: LifecycleContext) -> LifecycleLedgerV1 {
        let serve = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::CertifiedServe,
            0xC7,
        );
        let producer = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::ProducerTurn,
            0xC7,
        );
        let causal_root = CausalRoot::new(LifecycleDigest::new([0xC8; 32]));
        let owner = OwnerId::new(causal_root, 1);
        let serve = LifecycleLedgerRecordV1::new(
            serve.key,
            owner,
            1,
            serve.work_class,
            serve.stage,
            None,
            causal_root.digest(),
            serve.payload,
            serve.authority,
            DurableContinuation::None,
        )
        .expect("construct synthetic live Serve row");
        let producer = LifecycleLedgerRecordV1::new(
            producer.key,
            owner,
            2,
            producer.work_class,
            producer.stage,
            None,
            causal_root.digest(),
            producer.payload,
            producer.authority,
            DurableContinuation::None,
        )
        .expect("construct synthetic live Producer row");
        LifecycleLedgerV1::new(context, 2, vec![serve, producer], BTreeMap::from([(1, 2)]))
            .expect("construct synthetic live Serve ledger")
    }

    #[cfg(feature = "bls")]
    #[test]
    fn complete_tip_serve_reconciliation_binds_the_exact_source_frame() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = LifecycleLedgerV1::empty(context);
        let reconciliation = reconcile_complete_tip_serve_retirement(&ledger, payloads)
            .expect("empty final cut reconciles with the empty frame");

        assert!(reconciliation.authenticates_source(&ledger));
        assert!(reconciliation.is_drained());

        let stale = LifecycleLedgerV1::new(
            context,
            1,
            vec![sign_proposal_record(
                context,
                1,
                0xC9,
                Some(TerminalOutcome::Cancelled),
            )],
            BTreeMap::new(),
        )
        .expect("construct same-context stale frame");
        assert!(!reconciliation.authenticates_source(&stale));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn complete_tip_serve_reconciliation_rejects_missing_final_cut_coverage() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = live_synthetic_serve_ledger(context);

        assert!(reconcile_complete_tip_serve_retirement(&ledger, payloads).is_err());
    }

    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_seals_an_empty_exact_frame() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = LifecycleLedgerV1::empty(context);
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only(ledger.clone(), payloads)
                .expect("empty storage census assembles exactly");

        assert_eq!(recovery.context, context);
        assert_eq!(recovery.authenticated_ledger, ledger);
        assert!(recovery.candidates.is_empty());
        assert!(recovery.validate_no_successor.is_empty());
        assert!(recovery.authenticates_opened_ledger(&ledger));
        let foreign = LifecycleLedgerV1::empty(LifecycleContext::new(
            LifecycleDigest::new([0xC7; 32]),
            context.height(),
        ));
        assert!(!recovery.authenticates_opened_ledger(&foreign));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_consumes_exact_success_and_rejection_outcomes() {
        let mut fixture = empty_authenticated_payload_fixture();
        let validated = terminal_validate_record_with_body_outcome(&mut fixture, 1, 0, false);
        let rejected = terminal_validate_record_with_body_outcome(&mut fixture, 2, 1, true);
        let ledger = LifecycleLedgerV1::new(
            fixture.context,
            2,
            vec![validated, rejected],
            BTreeMap::new(),
        )
        .expect("construct two-outcome terminal Validate ledger");
        let strict = AuthenticatedLifecycleRecoveryCut::assemble_storage_only(
            ledger.clone(),
            fixture.payloads,
        )
        .expect_err("the body-free factory must reject terminal Validate tombstones");
        assert!(matches!(
            strict.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome { .. }
        ));
        let LifecycleRecoveryAssemblyError {
            _serve_payloads: payloads,
            ..
        } = strict;

        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_body_validation_outcomes(
                ledger.clone(),
                payloads,
                &mut fixture.body_store,
            )
            .expect("consume both exact terminal Validate outcomes");

        assert_eq!(recovery.authenticated_ledger, ledger);
        assert_eq!(recovery.validate_no_successor.len(), 2);
        assert!(recovery.candidates.is_empty());
        assert!(fixture.body_store.validated_recovery_catalog().is_empty());
    }

    #[cfg(feature = "bls")]
    #[test]
    fn terminal_validate_catalog_failure_restores_prior_exact_selection() {
        let mut fixture = empty_authenticated_payload_fixture();
        let first = terminal_validate_record_with_body_outcome(&mut fixture, 1, 0, false);
        let second = terminal_validate_record_with_body_outcome(&mut fixture, 2, 1, true);
        assert!(
            first.key().expect("decode first key") < second.key().expect("decode second key"),
            "the exact first claim must be selected before the substituted second claim"
        );
        let exact = LifecycleLedgerV1::new(
            fixture.context,
            2,
            vec![first.clone(), second.clone()],
            BTreeMap::new(),
        )
        .expect("construct exact two-outcome ledger");
        let DurablePayloadReference::BodyFrame(mut substituted_frame) = second
            .durable_payload()
            .expect("decode second terminal body frame")
        else {
            panic!("terminal Validate must retain a BodyFrame")
        };
        substituted_frame.frame = LifecycleDigest::new([0xCE; 32]);
        let substituted = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            second.key().expect("decode second key"),
            second.owner(),
            second.ordinal(),
            second.work_class().expect("decode second class"),
            second.stage().expect("decode second stage"),
            second.terminal().expect("decode second terminal"),
            second.reconstruction_source(),
            DurablePayloadReference::BodyFrame(substituted_frame),
            second.continuation().expect("decode second continuation"),
        )
        .expect("construct checksum-valid substituted body frame");
        assert!(matches!(
            LifecycleLedgerV1::new(
                fixture.context,
                2,
                vec![first, substituted],
                BTreeMap::new(),
            ),
            Err(super::super::ledger::LifecycleLedgerError::InvalidLedger(_))
        ));
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_body_validation_outcomes(
                exact,
                fixture.payloads,
                &mut fixture.body_store,
            )
            .expect("structural replay rejection leaves every exact outcome available");
        assert_eq!(recovery.validate_no_successor.len(), 2);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn repaired_wal_sign_and_terminal_validate_outcome_assemble_together() {
        let mut fixture = empty_authenticated_payload_fixture();
        let terminal = terminal_validate_record_with_body_outcome(&mut fixture, 3, 3, true);
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                fixture.context,
                0xCF,
            )
            .expect("construct repaired WAL ledger fixture");
        let mut records = repaired.records().to_vec();
        records.push(terminal);
        let combined = LifecycleLedgerV1::new(fixture.context, 3, records, BTreeMap::new())
            .expect("construct repaired WAL plus terminal Validate ledger");

        let recovery = AuthenticatedLifecycleRecoveryCut::
            assemble_storage_only_with_recovered_wal_sign_and_body_validation_outcomes(
                combined.clone(),
                fixture.payloads,
                &mut fixture.body_store,
                &projection,
            )
            .expect("assemble repaired Sign and terminal outcome atomically");
        assert_eq!(recovery.authenticated_ledger, combined);
        assert_eq!(recovery.candidates.len(), 1);
        assert_eq!(recovery.validate_no_successor.len(), 1);
        assert!(recovery.owns_recovered_wal_sign(&projection));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_failure_retains_frame_and_payload_authority() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let ledger = live_sign_proposal_ledger(context);
        let expected = ledger.clone();
        let error = AuthenticatedLifecycleRecoveryCut::assemble_storage_only(ledger, payloads)
            .expect_err("live SignProposal lacks storage-only replay authority");

        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 1,
                work_class: LifecycleWorkClass::SignProposal,
                stage,
            } if stage.kind() == LifecycleStageKind::SignProposal
        ));
        assert_eq!(error._authenticated_ledger, expected);
        assert!(error._serve_payloads.is_empty());
        assert_eq!(
            digest_bytes(error._serve_payloads.context_id().0.as_ref()),
            context.id()
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn storage_only_assembler_still_rejects_repaired_wal_sign_child() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (_projection, ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD0,
            )
            .expect("construct repaired WAL ledger fixture");
        let error = AuthenticatedLifecycleRecoveryCut::assemble_storage_only(ledger, payloads)
            .expect_err("unqualified storage-only recovery must reject the live Sign child");

        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 2,
                work_class: LifecycleWorkClass::SignVote,
                stage,
            } if stage.kind() == LifecycleStageKind::SignPrepareVote
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_seals_exact_repaired_child_and_frame() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD1,
            )
            .expect("construct repaired WAL ledger fixture");
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger.clone(),
                payloads,
                &projection,
            )
            .expect("exact repaired Sign child must assemble");

        assert_eq!(recovery.authenticated_ledger, ledger);
        assert!(projection.owns_spliced_candidates(&recovery.candidates));
        assert!(recovery.authenticates_opened_ledger(&ledger));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_foreign_live_sign_child() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, _own_ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD2,
            )
            .expect("construct installed WAL projection fixture");
        let (_foreign_projection, foreign_ledger) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD3,
            )
            .expect("construct foreign repaired WAL ledger fixture");
        let expected = foreign_ledger.clone();
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                foreign_ledger,
                payloads,
                &projection,
            )
            .expect_err("foreign live Sign must not consume installed WAL authority");

        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 2,
                work_class: LifecycleWorkClass::SignVote,
                ..
            }
        ));
        assert_eq!(error._authenticated_ledger, expected);
        assert!(error._serve_payloads.is_empty());
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_exact_child_at_wrong_durable_ordinal() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD9,
            )
            .expect("construct repaired WAL ledger fixture");
        let parent = repaired.records().first().expect("repaired fixture parent");
        let child = repaired.records().get(1).expect("repaired fixture child");
        let displaced_parent = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            parent.key().expect("decode parent key"),
            parent.owner(),
            parent.ordinal(),
            parent.work_class().expect("decode parent class"),
            parent.stage().expect("decode parent stage"),
            parent.terminal().expect("decode parent terminal"),
            parent.reconstruction_source(),
            parent.durable_payload().expect("decode parent payload"),
            DurableContinuation::successor(DurableContinuationEdge::ValidateToSignPrepare, 3),
        )
        .expect("construct displaced repaired parent");
        let filler = sign_proposal_record(context, 2, 0xDA, Some(TerminalOutcome::Cancelled));
        let displaced_child = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            child.key().expect("decode child key"),
            child.owner(),
            3,
            child.work_class().expect("decode child class"),
            child.stage().expect("decode child stage"),
            child.terminal().expect("decode child terminal"),
            child.reconstruction_source(),
            child.durable_payload().expect("decode child payload"),
            child.continuation().expect("decode child continuation"),
        )
        .expect("construct displaced repaired child");
        let ledger = LifecycleLedgerV1::new(
            context,
            3,
            vec![displaced_parent, filler, displaced_child],
            BTreeMap::new(),
        )
        .expect("construct valid wrong-ordinal repaired frame");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("semantic child at a foreign durable address must fail closed");

        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(message)
                if message.contains("owner, ordinal")
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_an_extra_live_ordinary_row() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD4,
            )
            .expect("construct repaired WAL ledger fixture");
        let mut records = repaired.records().to_vec();
        records.push(sign_proposal_record(context, 3, 0xD5, None));
        let ledger = LifecycleLedgerV1::new(context, 3, records, BTreeMap::new())
            .expect("construct repaired ledger with extra live work");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("one opaque WAL projection cannot authorize extra live work");

        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 3,
                work_class: LifecycleWorkClass::SignProposal,
                ..
            }
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_exact_child_with_foreign_first_owner_row() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xDB,
            )
            .expect("construct repaired WAL ledger fixture");
        let child = repaired
            .records()
            .get(1)
            .expect("repaired fixture Sign child")
            .clone();
        let owner = child.owner();
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::SignProposal,
            1,
        );
        let foreign_parent = LifecycleLedgerRecordV1::new(
            replay.key,
            owner,
            1,
            replay.work_class,
            replay.stage,
            Some(TerminalOutcome::Cancelled),
            owner.causal_root().digest(),
            replay.payload,
            replay.authority,
            DurableContinuation::None,
        )
        .expect("construct same-owner foreign first row");
        let ledger = LifecycleLedgerV1::new(
            context,
            child.ordinal(),
            vec![foreign_parent, child],
            BTreeMap::new(),
        )
        .expect("construct structurally valid child-only repaired impostor");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("exact Sign child cannot replace its typed Validate parent edge");

        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::RecoveredWalSign(message)
                if message.contains("terminal Validate parent")
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembler_rejects_pre_repair_live_validate() {
        let (context, payloads) = empty_authenticated_payload_cut();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD6,
            )
            .expect("construct repaired WAL ledger fixture");
        let repaired_parent = repaired
            .records()
            .first()
            .expect("repaired fixture retains its Validate parent");
        let live_parent = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            repaired_parent.key().expect("decode parent key"),
            repaired_parent.owner(),
            repaired_parent.ordinal(),
            repaired_parent.work_class().expect("decode parent class"),
            repaired_parent.stage().expect("decode parent stage"),
            None,
            repaired_parent.reconstruction_source(),
            repaired_parent
                .durable_payload()
                .expect("decode parent payload"),
            DurableContinuation::None,
        )
        .expect("construct pre-repair live Validate parent");
        let ledger = LifecycleLedgerV1::new(context, 1, vec![live_parent], BTreeMap::new())
            .expect("construct pre-repair WAL ledger");
        let error =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                ledger,
                payloads,
                &projection,
            )
            .expect_err("post-repair factory must not authorize the old live Validate parent");

        assert!(matches!(
            error.kind(),
            LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: 1,
                work_class: LifecycleWorkClass::Validate,
                ..
            }
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_assembled_cut_rejects_same_context_stale_reread() {
        let EmptyAuthenticatedPayloadFixture {
            context,
            verified,
            root,
            payload_store,
            payloads,
            ..
        } = empty_authenticated_payload_fixture();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD7,
            )
            .expect("construct repaired WAL ledger fixture");
        let recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                repaired.clone(),
                payloads,
                &projection,
            )
            .expect("assemble exact production-shaped repaired cut");

        let mut stale_records = repaired.records().to_vec();
        stale_records.push(sign_proposal_record(
            context,
            3,
            0xD8,
            Some(TerminalOutcome::Cancelled),
        ));
        let stale = LifecycleLedgerV1::new(context, 3, stale_records, BTreeMap::new())
            .expect("construct valid same-context stale frame");
        assert!(!recovery.authenticates_opened_ledger(&stale));

        let ledger_root = root.path().join("ledger");
        let (ledger_store, opened) = LifecycleLedgerStoreV1::open(&ledger_root, context)
            .expect("open stale-reread ledger store");
        assert_eq!(opened, LifecycleLedgerV1::empty(context));
        ledger_store
            .persist(&stale)
            .expect("persist same-context stale frame");
        let authority = authority::recovered_wal_test_authority(&verified)
            .expect("construct focused recovered-WAL authority");
        let prepared = LifecycleCoordinator::prepare_with_authority_borrowed(
            authority,
            &ledger_root,
            &payload_store,
            &recovery,
        );
        let Err(LifecycleOpenError(LifecycleOpenErrorKind::InvalidRecovery(message))) = prepared
        else {
            panic!("durable open did not reject the changed authenticated ledger frame")
        };
        assert_eq!(
            message,
            "lifecycle ledger changed after recovery-cut authentication"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn prepared_open_rejects_same_store_drift_without_overwrite() {
        let EmptyAuthenticatedPayloadFixture {
            context,
            verified,
            root,
            mut payload_store,
            payloads,
            ..
        } = empty_authenticated_payload_fixture();
        let (projection, repaired) =
            AuthenticatedRecoveredWalSignProjection::repaired_ledger_fixture_for_test(
                context, 0xD9,
            )
            .expect("construct prepared-open repaired WAL fixture");
        let mut recovery =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign(
                repaired.clone(),
                payloads,
                &projection,
            )
            .expect("assemble exact prepared-open recovery cut");

        let ledger_root = root.path().join("ledger");
        let (ledger_store, opened) = LifecycleLedgerStoreV1::open(&ledger_root, context)
            .expect("open prepared-open ledger store");
        assert_eq!(opened, LifecycleLedgerV1::empty(context));
        ledger_store
            .persist(&repaired)
            .expect("persist prepared-open predecessor frame");
        let authority = authority::recovered_wal_test_authority(&verified)
            .expect("construct prepared-open recovered-WAL authority");
        let prepared = LifecycleCoordinator::prepare_with_authority_borrowed(
            authority,
            &ledger_root,
            &payload_store,
            &recovery,
        )
        .expect("prepare against the exact predecessor frame");

        let drift = LifecycleLedgerV1::empty(context);
        ledger_store
            .persist(&drift)
            .expect("replace the predecessor after preparation");
        let error = prepared
            .commit(&mut payload_store, &mut recovery)
            .expect_err("commit must not overwrite a changed predecessor frame")
            .into_error();
        let LifecycleOpenError(LifecycleOpenErrorKind::Ledger(
            LifecycleLedgerError::InvalidLedger(message),
        )) = error
        else {
            panic!("prepare-to-commit drift returned the wrong error")
        };
        assert_eq!(
            message,
            "attached lifecycle ledger changed before successor publication"
        );
        assert_eq!(
            ledger_store
                .load()
                .expect("reload the externally changed predecessor"),
            drift,
            "failed exact-successor publication must not overwrite the changed frame"
        );
    }

    fn classifier_record(
        ordinal: u128,
        work_class: LifecycleWorkClass,
        stage_kind: LifecycleStageKind,
        terminal: Option<TerminalOutcome>,
        continuation: DurableContinuation,
    ) -> LifecycleLedgerRecordV1 {
        let context = LifecycleContext::new(LifecycleDigest::new([0x91; 32]), 11);
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            stage_kind,
            u8::try_from(ordinal).expect("small classifier view"),
        );
        assert_eq!(replay.work_class, work_class);
        let causal_root = CausalRoot::new(LifecycleDigest::new(
            [u8::try_from(ordinal).expect("small classifier marker"); 32],
        ));
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            work_class,
            replay.stage,
            terminal,
            causal_root.digest(),
            replay.payload,
            replay.authority,
            continuation,
        )
        .expect("construct classifier-only durable record")
    }

    fn ordinary_stage_inventory() -> [(LifecycleWorkClass, LifecycleStageKind); 20] {
        [
            (
                LifecycleWorkClass::SignProposal,
                LifecycleStageKind::SignProposal,
            ),
            (
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignPrepareVote,
            ),
            (
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignCommitVote,
            ),
            (
                LifecycleWorkClass::SignTimeout,
                LifecycleStageKind::SignTimeoutVote,
            ),
            (LifecycleWorkClass::Fetch, LifecycleStageKind::FetchBody),
            (LifecycleWorkClass::Store, LifecycleStageKind::StoreBody),
            (
                LifecycleWorkClass::Validate,
                LifecycleStageKind::ValidateBody,
            ),
            (LifecycleWorkClass::Apply, LifecycleStageKind::ApplyDecision),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastProposal,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastPrepareVote,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastCommitVote,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastPrepareQc,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastCommitQc,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTimeoutVote,
            ),
            (
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTc,
            ),
            (LifecycleWorkClass::EnterView, LifecycleStageKind::EnterView),
            (
                LifecycleWorkClass::EquivocationReport,
                LifecycleStageKind::ReportProposalEquivocation,
            ),
            (
                LifecycleWorkClass::EquivocationReport,
                LifecycleStageKind::ReportVoteEquivocation,
            ),
            (
                LifecycleWorkClass::EquivocationReport,
                LifecycleStageKind::ReportTimeoutEquivocation,
            ),
            (
                LifecycleWorkClass::InvalidBodyReport,
                LifecycleStageKind::ReportInvalidBody,
            ),
        ]
    }

    #[test]
    fn storage_only_classifier_rejects_every_live_ordinary_stage_typed() {
        for (index, (work_class, stage_kind)) in ordinary_stage_inventory().into_iter().enumerate()
        {
            let ordinal = u128::try_from(index + 1).expect("small classifier ordinal");
            let record = classifier_record(
                ordinal,
                work_class,
                stage_kind,
                None,
                DurableContinuation::None,
            );
            let Err(LifecycleRecoveryAssemblyErrorKind::MissingDurableRecoveryAuthority {
                ordinal: observed_ordinal,
                work_class: observed_class,
                stage: observed_stage,
            }) = classify_storage_only_record(&record)
            else {
                panic!("live {work_class:?}/{stage_kind:?} did not fail with typed authority debt")
            };
            assert_eq!(observed_ordinal, ordinal);
            assert_eq!(observed_class, work_class);
            assert_eq!(observed_stage.kind(), stage_kind);
        }
    }

    #[test]
    fn storage_only_classifier_accepts_terminal_inventory_and_serve_pair_only() {
        for (ordinal, work_class, stage_kind) in [
            (
                1,
                LifecycleWorkClass::CertifiedServe,
                LifecycleStageKind::CertifiedServe,
            ),
            (
                2,
                LifecycleWorkClass::ProducerTurn,
                LifecycleStageKind::ProducerTurn,
            ),
        ] {
            let record = classifier_record(
                ordinal,
                work_class,
                stage_kind,
                None,
                DurableContinuation::None,
            );
            assert!(classify_storage_only_record(&record).is_ok());
        }
        for (index, (work_class, stage_kind)) in ordinary_stage_inventory()
            .into_iter()
            .chain([
                (
                    LifecycleWorkClass::CertifiedServe,
                    LifecycleStageKind::CertifiedServe,
                ),
                (
                    LifecycleWorkClass::ProducerTurn,
                    LifecycleStageKind::ProducerTurn,
                ),
            ])
            .enumerate()
        {
            let ordinal = u128::try_from(index + 1).expect("small classifier ordinal");
            let record = classifier_record(
                ordinal,
                work_class,
                stage_kind,
                Some(TerminalOutcome::Cancelled),
                DurableContinuation::None,
            );
            assert!(
                classify_storage_only_record(&record).is_ok(),
                "terminal {work_class:?}/{stage_kind:?} should need no physical carrier"
            );
        }
        assert_eq!(LifecycleWorkClass::ALL.len(), 13);
        assert_eq!(LifecycleStageKind::ALL.len(), 22);
    }

    #[test]
    fn storage_only_classifier_rejects_a_class_stage_mismatch() {
        let record = classifier_record(
            3,
            LifecycleWorkClass::SignProposal,
            LifecycleStageKind::FetchBody,
            Some(TerminalOutcome::Cancelled),
            DurableContinuation::None,
        );
        assert!(matches!(
            classify_storage_only_record(&record),
            Err(LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecordShape {
                ordinal: 3,
                work_class: LifecycleWorkClass::SignProposal,
                stage,
            }) if stage.kind() == LifecycleStageKind::FetchBody
        ));
    }

    #[test]
    fn storage_only_classifier_checks_validate_no_successor_before_terminality() {
        let record = classifier_record(
            7,
            LifecycleWorkClass::Validate,
            LifecycleStageKind::ValidateBody,
            Some(TerminalOutcome::Advanced),
            DurableContinuation::AdvancedNoSuccessor,
        );
        let Err(LifecycleRecoveryAssemblyErrorKind::MissingTerminalValidateOutcome {
            ordinal,
            stage,
        }) = classify_storage_only_record(&record)
        else {
            panic!("terminal Validate/no-successor lost its typed body-outcome debt")
        };
        assert_eq!(ordinal, 7);
        assert_eq!(stage.kind(), LifecycleStageKind::ValidateBody);
    }

    #[test]
    fn storage_only_assembler_source_is_sealed_and_exhaustive() {
        let source = include_str!("v2_lifecycle_open.rs");
        let assembler_start = source
            .find("// STORAGE_ONLY_LIFECYCLE_RECOVERY_ASSEMBLER_BEGIN")
            .expect("locate storage-only assembler");
        let assembler_end = source[assembler_start..]
            .find("// STORAGE_ONLY_LIFECYCLE_RECOVERY_ASSEMBLER_END")
            .map(|offset| assembler_start + offset)
            .expect("locate storage-only assembler end");
        let assembler = &source[assembler_start..assembler_end];
        for required in [
            "ledger: LifecycleLedgerV1",
            "serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut",
            "validate_storage_only_recovery(&ledger, &serve_payloads)",
            "_authenticated_ledger: ledger",
            "_serve_payloads: serve_payloads",
            "authenticated_ledger: ledger",
            "assemble_storage_only_with_recovered_wal_sign(",
            "projection: &AuthenticatedRecoveredWalSignProjection",
            "assemble_storage_only_recovered_wal_candidates(",
            "assemble_storage_only_with_body_validation_outcomes(",
            "assemble_storage_only_with_recovered_wal_sign_and_body_validation_outcomes(",
            "detach_terminal_validate_outcome_catalog()",
            "select_exact_terminal_validate(claim)",
            "catalog.commit_selected()",
        ] {
            assert!(assembler.contains(required), "assembler omitted {required}");
        }
        for forbidden in [
            "CandidateAdmission",
            "from_authenticated_parts",
            "into_parts",
        ] {
            assert!(
                !assembler.contains(forbidden),
                "assembler exposes forbidden raw surface {forbidden}"
            );
        }

        let classifier_start = source
            .find("// STORAGE_ONLY_LIFECYCLE_RECOVERY_CLASSIFIER_BEGIN")
            .expect("locate storage-only classifier");
        let classifier_end = source[classifier_start..]
            .find("// STORAGE_ONLY_LIFECYCLE_RECOVERY_CLASSIFIER_END")
            .map(|offset| classifier_start + offset)
            .expect("locate storage-only classifier end");
        let classifier = &source[classifier_start..classifier_end];
        for stage in [
            "SignProposal",
            "SignPrepareVote",
            "SignCommitVote",
            "SignTimeoutVote",
            "FetchBody",
            "StoreBody",
            "ValidateBody",
            "ApplyDecision",
            "BroadcastProposal",
            "BroadcastPrepareVote",
            "BroadcastCommitVote",
            "BroadcastPrepareQc",
            "BroadcastCommitQc",
            "BroadcastTimeoutVote",
            "BroadcastTc",
            "EnterView",
            "ReportProposalEquivocation",
            "ReportVoteEquivocation",
            "ReportTimeoutEquivocation",
            "ReportInvalidBody",
            "CertifiedServe",
            "ProducerTurn",
        ] {
            assert!(
                classifier.contains(&format!("LifecycleStageKind::{stage}")),
                "classifier omitted stage {stage}"
            );
        }
        assert!(classifier.contains("MissingDurableRecoveryAuthority"));
        assert!(classifier.contains("MissingTerminalValidateOutcome"));
        assert!(source.contains("#[cfg(test)]\n    pub(super) fn from_authenticated_parts("));
        assert!(source.contains("&self.authenticated_ledger == opened"));
        assert!(source.contains("if !recovery.authenticates_opened_ledger(&ledger)"));
        assert!(
            source
                .contains("projection.repaired_pair_is_exact(ledger.context(), ledger.records())")
        );

        let production = source
            .split("\n#[cfg(test)]\nmod recovery_tests {")
            .next()
            .expect("lifecycle open has one production prefix");
        assert_eq!(
            production
                .matches("assemble_storage_only_with_recovered_wal_sign(")
                .count(),
            1,
            "post-repair assembly must remain inside the future consuming installed-cut join"
        );
        assert_eq!(
            production
                .matches("assemble_storage_only_with_body_validation_outcomes(")
                .count(),
            1,
            "terminal Validate storage assembly has one sealed production entry point"
        );
    }

    fn terminal_validate_no_successor_ledger()
    -> (LifecycleLedgerV1, AuthenticatedValidateNoSuccessorRecovery) {
        let context = LifecycleContext::new(LifecycleDigest::new([0x81; 32]), 9);
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::ValidateBody,
            4,
        );
        let key = replay.key;
        let causal_root = CausalRoot::new(LifecycleDigest::new([0x83; 32]));
        let owner = OwnerId::new(causal_root, 1);
        let stage = replay.stage;
        let payload = replay.payload;
        let record = LifecycleLedgerRecordV1::new(
            key,
            owner,
            1,
            replay.work_class,
            stage,
            Some(TerminalOutcome::Advanced),
            causal_root.digest(),
            payload,
            replay.authority,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct terminal Validate ledger record");
        let ledger = LifecycleLedgerV1::new(context, 1, vec![record], BTreeMap::new())
            .expect("construct exact terminal Validate ledger");
        let proof = AuthenticatedValidateNoSuccessorRecovery {
            key,
            causal_root,
            reconstruction_source: causal_root.digest(),
            stage,
            payload,
        };
        (ledger, proof)
    }

    #[test]
    fn terminal_validate_no_successor_requires_exact_recovery_coverage() {
        let (ledger, proof) = terminal_validate_no_successor_ledger();
        assert!(
            validate_terminal_validate_no_successor_recovery(&ledger, &BTreeMap::new()).is_err()
        );

        let exact = BTreeMap::from([(proof.key, proof)]);
        assert!(validate_terminal_validate_no_successor_recovery(&ledger, &exact).is_ok());

        let mut foreign = proof;
        foreign.reconstruction_source = LifecycleDigest::new([0x86; 32]);
        assert!(
            validate_terminal_validate_no_successor_recovery(
                &ledger,
                &BTreeMap::from([(foreign.key, foreign)]),
            )
            .is_err()
        );

        let mut substituted = proof;
        let DurablePayloadReference::BodyFrame(mut frame) = substituted.payload else {
            panic!("terminal Validate proof must retain one body frame");
        };
        frame.frame = LifecycleDigest::new([0x87; 32]);
        substituted.payload = DurablePayloadReference::BodyFrame(frame);
        assert!(
            validate_terminal_validate_no_successor_recovery(
                &ledger,
                &BTreeMap::from([(substituted.key, substituted)]),
            )
            .is_err()
        );
    }
}
