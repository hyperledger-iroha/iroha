//! Height-bound production beacon readiness, independent of transaction admission.
//!
//! The lifecycle authenticates public state and provider custody at activation.
//! HTTP only checks the published ownership and exact committed public value;
//! this observation never authorizes signing or changes mandatory pulse rules.

use std::sync::Arc;

use iroha_data_model::{block::consensus_v2 as wire, governance::types::BeaconSessionId};
use iroha_model_base::peer::PeerId;
use mv::storage::StorageReadOnly as _;
use parking_lot::Mutex;
use thiserror::Error;

use super::{
    FinalizedGlobalThresholdBeaconKeySessionRecordV1, GlobalThresholdBeaconPartialSignerV1,
    GlobalThresholdBeaconSessionBindingV1, ValidatedGlobalThresholdBeaconSessionV1,
    active_global_threshold_beacon_session_id_v1,
    authenticated_global_threshold_beacon_roster_hash_v1,
    validate_global_threshold_beacon_session_v1,
};
use crate::state::{State, WorldReadOnly as _};

/// Non-secret reason the current node cannot claim production beacon readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum GlobalBeaconReadinessErrorV1 {
    /// No authenticated height has published its readiness observation.
    #[error("Global beacon readiness is not initialized for the active height")]
    Uninitialized,
    /// State or lifecycle ownership changed after the observation.
    #[error("Global beacon readiness differs from the current committed height or session")]
    StateChanged,
    /// The required beacon has no committed public key installation.
    #[error("Global beacon key installation is required before the mandatory pulse")]
    MissingSession,
    /// Public key state, its transcript, or its lifecycle is invalid.
    #[error("Global beacon public key session is invalid")]
    InvalidSession,
    /// The public session names a different network or ordered committee.
    #[error("Global beacon public key session differs from the active consensus roster")]
    ForeignSession,
    /// Activation or retirement excludes the required pulse height.
    #[error("Global beacon key session does not cover the mandatory pulse height")]
    SessionNotLive,
    /// This validator has no resolved production signer.
    #[error("Global beacon production provider is absent for the local validator")]
    MissingProvider,
    /// The provider could not prove non-signing custody.
    #[error("Global beacon production provider capability is unavailable")]
    ProviderUnavailable,
    /// The provider attested a different session or local seat.
    #[error("Global beacon production provider does not own the exact session and local seat")]
    ProviderMismatch,
}

#[derive(Clone)]
struct HeightBinding {
    context_id: wire::HeightContextId,
    network_id: iroha_data_model::NetworkId,
    height: u64,
    earliest_required_height: u64,
    latest_required_height: u64,
    required: bool,
    roster: Vec<PeerId>,
    local_validator: Option<wire::ValidatorIndex>,
}

#[derive(Clone)]
struct PublishedReadiness {
    binding: HeightBinding,
    state_generation: u64,
    pointer: Result<Option<[u8; 32]>, ()>,
    record: Option<Arc<FinalizedGlobalThresholdBeaconKeySessionRecordV1>>,
    outcome: Result<(), GlobalBeaconReadinessErrorV1>,
}

#[derive(Default)]
struct ReadinessState {
    context_id: Option<wire::HeightContextId>,
    published: Option<Arc<PublishedReadiness>>,
}

/// Readiness observation owned by serialized height activation.
/// Missing setup never closes ingress needed by the existing installation ISI.
#[derive(Default)]
pub(crate) struct GlobalBeaconReadinessV1 {
    state: Mutex<ReadinessState>,
    // Only authenticated immutable transcript material survives a height change.
    // Lifecycle, roster, current state and provider checks are always fresh.
    authenticated_session: Mutex<
        Option<(
            GlobalThresholdBeaconSessionBindingV1,
            Arc<ValidatedGlobalThresholdBeaconSessionV1>,
        )>,
    >,
    #[cfg(test)]
    transcript_validations: std::sync::atomic::AtomicUsize,
}

impl GlobalBeaconReadinessV1 {
    pub(crate) fn begin_height(&self, context_id: wire::HeightContextId) {
        *self.state.lock() = ReadinessState {
            context_id: Some(context_id),
            published: None,
        };
    }

    pub(crate) fn publish_for_height(
        &self,
        context: &wire::HeightContext,
        state: &State,
        local_validator: Option<wire::ValidatorIndex>,
        signer: Option<&dyn GlobalThresholdBeaconPartialSignerV1>,
    ) {
        let parliament_requested = {
            let world = state.world_view();
            world
                .parliament_required_beacon_pulse_slots
                .get(&(
                    BeaconSessionId::for_network_v1(&context.network_id),
                    context.height,
                ))
                .is_some_and(|attempts| !attempts.is_empty())
        };
        let npos = context.mode == wire::ConsensusMode::Npos;
        let npos_pulse_height = if npos && context.height < context.epoch_end_height {
            context.epoch_end_height.saturating_sub(1)
        } else {
            context.height
        };
        self.publish_binding(
            HeightBinding {
                context_id: context.id(),
                network_id: context.network_id,
                height: context.height,
                earliest_required_height: if parliament_requested {
                    context.height
                } else {
                    npos_pulse_height
                },
                latest_required_height: npos_pulse_height,
                required: npos || parliament_requested,
                roster: context
                    .roster
                    .iter()
                    .map(|seat| seat.validator.clone())
                    .collect(),
                local_validator,
            },
            state,
            signer,
        );
    }

    fn publish_binding(
        &self,
        binding: HeightBinding,
        state: &State,
        signer: Option<&dyn GlobalThresholdBeaconPartialSignerV1>,
    ) {
        let mut published = PublishedReadiness {
            binding,
            state_generation: state.state_view_generation(),
            pointer: Ok(None),
            record: None,
            outcome: Ok(()),
        };
        published.outcome = assess_public_and_provider(self, &mut published, state, signer);
        if !matches_committed_state(&published, state) {
            published.outcome = Err(GlobalBeaconReadinessErrorV1::StateChanged);
        }
        let mut current = self.state.lock();
        if current.context_id == Some(published.binding.context_id) {
            current.published = Some(Arc::new(published));
        }
    }

    fn authenticate_session(
        &self,
        record: &iroha_data_model::consensus::GlobalThresholdBeaconKeySessionV1,
        binding: &GlobalThresholdBeaconSessionBindingV1,
    ) -> Result<Arc<ValidatedGlobalThresholdBeaconSessionV1>, GlobalBeaconReadinessErrorV1> {
        if let Some((authenticated_binding, session)) = self.authenticated_session.lock().as_ref()
            && authenticated_binding == binding
            && session.record() == record
        {
            return Ok(Arc::clone(session));
        }
        #[cfg(test)]
        self.transcript_validations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let session = Arc::new(
            validate_global_threshold_beacon_session_v1(record.clone(), binding)
                .map_err(|_| GlobalBeaconReadinessErrorV1::InvalidSession)?,
        );
        *self.authenticated_session.lock() = Some((*binding, Arc::clone(&session)));
        Ok(session)
    }

    pub(crate) fn check(&self, state: &State) -> Result<(), GlobalBeaconReadinessErrorV1> {
        let published = self
            .state
            .lock()
            .published
            .clone()
            .ok_or(GlobalBeaconReadinessErrorV1::Uninitialized)?;
        if !matches_committed_state(&published, state) {
            return Err(GlobalBeaconReadinessErrorV1::StateChanged);
        }
        let current = self.state.lock();
        if current.context_id != Some(published.binding.context_id)
            || current
                .published
                .as_ref()
                .is_none_or(|now| !Arc::ptr_eq(now, &published))
        {
            return Err(GlobalBeaconReadinessErrorV1::StateChanged);
        }
        published.outcome
    }
}

fn assess_public_and_provider(
    readiness: &GlobalBeaconReadinessV1,
    published: &mut PublishedReadiness,
    state: &State,
    signer: Option<&dyn GlobalThresholdBeaconPartialSignerV1>,
) -> Result<(), GlobalBeaconReadinessErrorV1> {
    use GlobalBeaconReadinessErrorV1 as E;
    let binding = &published.binding;
    if !binding.required {
        return Ok(());
    }
    let record = {
        let world = state.world_view();
        published.pointer = active_global_threshold_beacon_session_id_v1(&world).map_err(|_| ());
        let session_id = published
            .pointer
            .map_err(|_| E::InvalidSession)?
            .ok_or(E::MissingSession)?;
        let record = world
            .global_beacon_key_sessions()
            .get(&session_id)
            .cloned()
            .ok_or(E::MissingSession)?;
        record
    };
    // Preserve the exact owned public value so a setup change also invalidates
    // a previously failed observation. Never retain a World view across crypto.
    published.record = Some(Arc::new(record.clone()));
    if published.pointer != Ok(Some(record.session.session_id)) {
        return Err(E::InvalidSession);
    }
    if record.session.network_id != binding.network_id {
        return Err(E::ForeignSession);
    }
    let roster_hash =
        authenticated_global_threshold_beacon_roster_hash_v1(&record.session, &binding.roster)
            .map_err(|_| E::ForeignSession)?;
    let session = readiness.authenticate_session(
        &record.session,
        &GlobalThresholdBeaconSessionBindingV1 {
            network_id: binding.network_id,
            session_id: record.session.session_id,
            roster_hash,
            transcript_hash: record.session.transcript_hash,
        },
    )?;
    let Some(activation) = record.activated_at_height else {
        return Err(E::SessionNotLive);
    };
    if activation < record.session.adaptive_dkg.finalized_at_height
        || record
            .retired_at_height
            .is_some_and(|retired| retired <= activation)
    {
        return Err(E::InvalidSession);
    }
    if !record.is_active_at(binding.earliest_required_height)
        || !record.is_active_at(binding.latest_required_height)
    {
        return Err(E::SessionNotLive);
    }
    let Some(local_validator) = binding.local_validator else {
        return Ok(());
    };
    let signer_index = local_validator
        .checked_add(1)
        .and_then(|index| u16::try_from(index).ok())
        .ok_or(E::ProviderMismatch)?;
    if usize::try_from(local_validator)
        .ok()
        .is_none_or(|index| index >= binding.roster.len())
    {
        return Err(E::ProviderMismatch);
    }
    let capability = signer
        .ok_or(E::MissingProvider)?
        .attest_partial_signing_capability(&session, signer_index)
        .map_err(|error| match error {
            super::GlobalThresholdBeaconCapabilityErrorV1::Unavailable => E::ProviderUnavailable,
            super::GlobalThresholdBeaconCapabilityErrorV1::NotOwned
            | super::GlobalThresholdBeaconCapabilityErrorV1::InvalidRequest => E::ProviderMismatch,
        })?;
    if !capability.matches(&session, signer_index) {
        return Err(E::ProviderMismatch);
    }
    Ok(())
}

fn matches_committed_state(published: &PublishedReadiness, state: &State) -> bool {
    let before = state.state_view_generation();
    if before % 2 != 0
        || before != published.state_generation
        || state.network_id_ref() != &published.binding.network_id
        || u64::try_from(state.committed_height())
            .ok()
            .and_then(|height| height.checked_add(1))
            != Some(published.binding.height)
    {
        return false;
    }
    let session_matches = if published.binding.required {
        let world = state.world_view();
        let pointer = active_global_threshold_beacon_session_id_v1(&world).map_err(|_| ());
        pointer == published.pointer
            && match (pointer, &published.record) {
                (Ok(None), None) | (Err(()), None) => true,
                (Ok(Some(id)), record) => {
                    world.global_beacon_key_sessions().get(&id) == record.as_deref()
                }
                _ => false,
            }
    } else {
        true
    };
    session_matches && state.state_view_generation() == before
}

#[cfg(test)]
mod tests {
    use std::sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    };

    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::block::BlockHeader;
    use iroha_model_base::chain::ChainId;

    use super::*;
    use crate::{
        beacon::{
            GlobalThresholdBeaconCapabilityErrorV1, GlobalThresholdBeaconPartialSignatureV1,
            GlobalThresholdBeaconPartialSigningCapabilityV1,
            ValidatedGlobalThresholdBeaconSessionV1,
            fixtures::{adaptive_beacon_fixture_for_session, adaptive_dkg_session_fixture},
            global_threshold_beacon_roster_hash_v1,
        },
        kura::Kura,
        query::store::LiveQueryStore,
        state::{GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, World},
    };

    static FIXTURE: LazyLock<(Vec<KeyPair>, ValidatedGlobalThresholdBeaconSessionV1)> =
        LazyLock::new(|| {
            let mut keys = (1_u8..=4)
                .map(|marker| {
                    KeyPair::try_from_seed(vec![marker; 32], iroha_crypto::Algorithm::BlsNormal)
                        .expect("deterministic consensus validator")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let roster = keys
                .iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect::<Vec<_>>();
            let mut dkg = adaptive_dkg_session_fixture();
            dkg.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
            (keys, adaptive_beacon_fixture_for_session(dkg).session)
        });

    struct CapabilityProvider {
        calls: AtomicUsize,
        seat: Option<u16>,
        unavailable: bool,
        session: Option<ValidatedGlobalThresholdBeaconSessionV1>,
    }

    impl CapabilityProvider {
        fn exact() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                seat: None,
                unavailable: false,
                session: None,
            }
        }
    }

    impl GlobalThresholdBeaconPartialSignerV1 for CapabilityProvider {
        fn attest_partial_signing_capability(
            &self,
            session: &ValidatedGlobalThresholdBeaconSessionV1,
            expected_signer_index: u16,
        ) -> Result<
            GlobalThresholdBeaconPartialSigningCapabilityV1,
            GlobalThresholdBeaconCapabilityErrorV1,
        > {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.unavailable {
                return Err(GlobalThresholdBeaconCapabilityErrorV1::Unavailable);
            }
            GlobalThresholdBeaconPartialSigningCapabilityV1::for_validated_session(
                self.session.as_ref().unwrap_or(session),
                self.seat.unwrap_or(expected_signer_index),
            )
        }

        fn sign_partial(
            &self,
            _session: &ValidatedGlobalThresholdBeaconSessionV1,
            _payload: &[u8],
        ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
            panic!("readiness must never request a signature")
        }
    }

    fn fixture() -> (
        State,
        wire::HeightContext,
        FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    ) {
        fixture_with_epoch_end_height(42)
    }

    fn fixture_with_epoch_end_height(
        epoch_end_height: u64,
    ) -> (
        State,
        wire::HeightContext,
        FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    ) {
        let (keys, session) = &*FIXTURE;
        let parent_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"readiness parent"));
        let context = super::super::tests::live_producer_context(
            keys,
            session.record().network_id,
            parent_hash,
            epoch_end_height,
        );
        let mut record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(session.record().clone())
                .expect("authenticated fixture transcript");
        record.activate(30).expect("activate finalized key");
        let mut state = State::new_with_chain_and_network_id_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("global-beacon-readiness"),
            session.record().network_id,
        );
        for marker in 1_u8..40 {
            state.push_block_hash_for_testing(HashOf::from_untyped_unchecked(Hash::prehashed(
                [marker; 32],
            )));
        }
        state.push_block_hash_for_testing(parent_hash);
        install(&state, &record);
        (state, context, record)
    }

    fn install(state: &State, record: &FinalizedGlobalThresholdBeaconKeySessionRecordV1) {
        let mut world = state.world.block();
        world
            .global_beacon_key_sessions
            .insert(record.session.session_id, record.clone());
        world.global_beacon_active_session.insert(
            GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
            record.session.session_id,
        );
        world.commit();
    }

    fn publish(
        readiness: &GlobalBeaconReadinessV1,
        context: &wire::HeightContext,
        state: &State,
        signer: Option<&dyn GlobalThresholdBeaconPartialSignerV1>,
    ) {
        readiness.begin_height(context.id());
        readiness.publish_for_height(context, state, Some(0), signer);
    }

    #[test]
    fn readiness_authenticates_exact_session_once_without_signing_on_http_checks() {
        let (state, context, _) = fixture();
        let readiness = GlobalBeaconReadinessV1::default();
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::Uninitialized)
        );
        let provider = CapabilityProvider::exact();
        publish(&readiness, &context, &state, Some(&provider));
        for _ in 0..32 {
            assert_eq!(readiness.check(&state), Ok(()));
        }
        assert_eq!(provider.calls.load(Ordering::Relaxed), 1);
        assert_eq!(readiness.transcript_validations.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn readiness_reuses_only_exact_authenticated_transcripts_across_heights() {
        let (mut state, mut context, mut record) = fixture_with_epoch_end_height(80);
        let readiness = GlobalBeaconReadinessV1::default();
        let provider = CapabilityProvider::exact();
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(readiness.check(&state), Ok(()));
        let next_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"new readiness committed height"));
        state.push_block_hash_for_testing(next_hash);
        context.height += 1;
        let parent = context
            .parent_commit_qc
            .as_mut()
            .expect("fixture parent QC");
        parent.round.height += 1;
        parent.proposal_round.height += 1;
        parent.subject.parent_block_hash = Some(parent.subject.block_hash);
        parent.subject.block_hash = next_hash;
        context.validate().expect("valid successor context");
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(readiness.check(&state), Ok(()));
        assert_eq!(
            provider.calls.load(Ordering::Relaxed),
            2,
            "custody is fresh for each owner"
        );
        assert_eq!(readiness.transcript_validations.load(Ordering::Relaxed), 1);
        record.session.transcript_hash[0] ^= 1;
        install(&state, &record);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::InvalidSession)
        );
        assert_eq!(
            readiness.transcript_validations.load(Ordering::Relaxed),
            2,
            "changed transcript cannot inherit earlier authentication"
        );
        assert_eq!(provider.calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn readiness_requires_pending_session_to_cover_the_mandatory_pulse() {
        let (state, context, mut record) = fixture_with_epoch_end_height(80);
        context.validate().expect("valid later NPoS boundary");
        record.activated_at_height = Some(50);
        install(&state, &record);
        let readiness = GlobalBeaconReadinessV1::default();
        let provider = CapabilityProvider::exact();
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Ok(()),
            "committed pending activation covers pulse 79"
        );
        record.retired_at_height = Some(79);
        install(&state, &record);
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::StateChanged)
        );
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::SessionNotLive)
        );
        record.retired_at_height = None;
        record.activated_at_height = Some(80);
        install(&state, &record);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::SessionNotLive)
        );
    }

    #[test]
    fn readiness_requires_both_current_parliament_and_future_npos_pulses() {
        let (state, context, mut record) = fixture_with_epoch_end_height(80);
        context.validate().expect("valid later NPoS boundary");
        {
            let mut world = state.world.block();
            world.parliament_required_beacon_pulse_slots.insert(
                (
                    BeaconSessionId::for_network_v1(&context.network_id),
                    context.height,
                ),
                std::collections::BTreeSet::from([
                    iroha_data_model::governance::types::GovernanceAttemptId::new([0x75; 32]),
                ]),
            );
            world.commit();
        }
        let readiness = GlobalBeaconReadinessV1::default();
        let provider = CapabilityProvider::exact();
        record.retired_at_height = Some(79);
        install(&state, &record);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::SessionNotLive),
            "current Parliament request cannot conceal retirement before the NPoS pulse"
        );
        record.retired_at_height = Some(80);
        record.activated_at_height = Some(50);
        install(&state, &record);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::SessionNotLive),
            "future NPoS coverage cannot conceal a missing current Parliament key"
        );
        record.activated_at_height = Some(context.height);
        install(&state, &record);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Ok(()),
            "a key covering both required endpoints qualifies from the current committed state"
        );
    }

    #[test]
    fn readiness_rejects_missing_foreign_and_corrupt_public_sessions() {
        let (state, context, record) = fixture();
        let readiness = GlobalBeaconReadinessV1::default();
        let provider = CapabilityProvider::exact();
        {
            let mut world = state.world.block();
            world
                .global_beacon_active_session
                .remove(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY);
            world.commit();
        }
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::MissingSession)
        );
        install(&state, &record);
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::StateChanged)
        );
        let mut foreign = record.clone();
        foreign.session.roster_hash[0] ^= 1;
        install(&state, &foreign);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::ForeignSession)
        );
        let mut corrupt = record.clone();
        corrupt.session.transcript_hash[0] ^= 1;
        install(&state, &corrupt);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::InvalidSession)
        );
        let mut wrong_id = record;
        wrong_id.session.session_id[0] ^= 1;
        {
            let mut world = state.world.block();
            world
                .global_beacon_key_sessions
                .insert(FIXTURE.1.record().session_id, wrong_id);
            world.commit();
        }
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::InvalidSession)
        );
        assert_eq!(
            provider.calls.load(Ordering::Relaxed),
            0,
            "invalid public state cannot reach custody"
        );
    }

    #[test]
    fn readiness_rejects_absent_unavailable_and_wrong_seat_providers() {
        let (state, context, _) = fixture();
        let readiness = GlobalBeaconReadinessV1::default();
        publish(&readiness, &context, &state, None);
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::MissingProvider)
        );
        let mut provider = CapabilityProvider::exact();
        provider.unavailable = true;
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::ProviderUnavailable)
        );
        provider.unavailable = false;
        provider.seat = Some(2);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::ProviderMismatch)
        );
        provider.seat = None;
        let mut other_dkg = adaptive_dkg_session_fixture();
        other_dkg.session_id[0] ^= 1;
        provider.session = Some(adaptive_beacon_fixture_for_session(other_dkg).session);
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::ProviderMismatch)
        );
        provider.session = None;
        publish(&readiness, &context, &state, Some(&provider));
        assert_eq!(readiness.check(&state), Ok(()));
    }

    #[test]
    fn readiness_invalidates_old_height_roster_and_publication_owner() {
        let (mut state, context, _) = fixture();
        let readiness = GlobalBeaconReadinessV1::default();
        let provider = CapabilityProvider::exact();
        publish(&readiness, &context, &state, Some(&provider));
        let mut next_owner = context.clone();
        next_owner.roster.swap(0, 1);
        readiness.begin_height(next_owner.id());
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::Uninitialized)
        );
        readiness.publish_for_height(&context, &state, Some(0), Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::Uninitialized),
            "stale owner cannot publish"
        );
        readiness.publish_for_height(&next_owner, &state, Some(0), Some(&provider));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::ForeignSession)
        );
        publish(&readiness, &context, &state, Some(&provider));
        state.push_block_hash_for_testing(HashOf::from_untyped_unchecked(Hash::new(
            b"next committed height",
        )));
        assert_eq!(
            readiness.check(&state),
            Err(GlobalBeaconReadinessErrorV1::StateChanged)
        );
    }

    #[test]
    fn readiness_does_not_require_local_custody_for_observers_or_unused_permissioned_beacons() {
        let (state, mut context, _) = fixture();
        let readiness = GlobalBeaconReadinessV1::default();
        readiness.begin_height(context.id());
        readiness.publish_for_height(&context, &state, None, None);
        assert_eq!(
            readiness.check(&state),
            Ok(()),
            "observer has no signer seat"
        );
        context.mode = wire::ConsensusMode::Permissioned;
        {
            let mut world = state.world.block();
            world
                .global_beacon_active_session
                .remove(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY);
            world.commit();
        }
        publish(&readiness, &context, &state, None);
        assert_eq!(
            readiness.check(&state),
            Ok(()),
            "no mandatory beacon applies in this permissioned context"
        );
    }
}
