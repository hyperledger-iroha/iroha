//! Current, parent-bound threshold beacon production. No consensus view or retired context
//! contributes entropy. A bounded worker handles cryptography away from the P2P ingress and
//! starts local signing only for real payload demand or an authenticated valid peer share.
use super::{
    crypto::{core_key, iroha_key},
    driver::{
        DriverHandle,
        traits::{Frame, Net},
    },
    net::FrameSink,
};
use crate::{
    beacon::{
        GlobalThresholdBeaconPartialSignerV1, GlobalThresholdBeaconPulseAggregatorV1,
        GlobalThresholdBeaconSessionBindingV1, ValidatedGlobalThresholdBeaconSessionV1,
        authenticated_global_threshold_beacon_roster_hash_v1,
        validate_global_threshold_beacon_session_v1,
        verify_finalized_global_threshold_beacon_pulse_v1,
    },
    state::{GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, State, StateReadOnly, WorldReadOnly},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconChainAnchorV1,
        GlobalThresholdBeaconPartialSignatureV1,
    },
    governance::types::BeaconSessionId,
    parameter::system::ConsensusMode,
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    message::TrafficClass,
    types::{Hash32, PublicKey},
};
use mv::storage::StorageReadOnly;
use norito::{NoritoDeserialize, NoritoSerialize};
use parking_lot::Mutex;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const PREFIX: &[u8] = b"IROHA-BEACON\x01";
/// Strict bound for a fixed-size authenticated partial frame, including canonical framing.
pub const MAX_FRAME_BYTES: usize = 4096;
const QUEUE_CAPACITY: usize = 64;
const RETRANSMIT: Duration = Duration::from_secs(1);

/// A missing pulse is availability, never an empty block or permission to omit randomness.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BeaconError {
    /// Enough verified shares have not arrived yet.
    #[error("required threshold beacon pulse is pending")]
    Pending,
    /// The committed state cannot authorize this pulse.
    #[error("current beacon state: {0}")]
    State(&'static str),
    /// A public cryptographic statement or runtime signer failed verification.
    #[error("current beacon cryptographic validation failed")]
    Crypto,
    /// The worker stopped or could not retain bounded demand.
    #[error("current beacon worker unavailable")]
    Unavailable,
}

/// Complete authority selected from one immutable committed-state view.
pub struct BeaconRequirement {
    /// Exact requested slot.
    pub height: u64,
    /// Signed-genesis network.
    pub network_id: NetworkId,
    /// Committed parent signed by every share.
    pub parent: GlobalThresholdBeaconChainAnchorV1,
    /// Actual scheduled canonical validator order.
    pub roster: Vec<PeerId>,
    /// Active public DKG transcript independently validated against this roster.
    pub session: ValidatedGlobalThresholdBeaconSessionV1,
}
impl BeaconRequirement {
    /// Verify the entire finalized pulse against the current authority and parent.
    pub fn verify(&self, pulse: &FinalizedGlobalThresholdBeaconPulseV1) -> Result<(), BeaconError> {
        if pulse.height != self.height || pulse.network_id != self.network_id {
            return Err(BeaconError::State("pulse differs from requested slot"));
        }
        verify_finalized_global_threshold_beacon_pulse_v1(&self.session, pulse, self.parent)
            .map(|_| ())
            .map_err(|_| BeaconError::Crypto)
    }
}

fn requested(
    view: &impl StateReadOnly,
    height: u64,
    mode: ConsensusMode,
) -> Result<bool, BeaconError> {
    if u64::try_from(view.height())
        .ok()
        .and_then(|h| h.checked_add(1))
        != Some(height)
    {
        return Err(BeaconError::State(
            "pulse slot is not the next committed height",
        ));
    }
    let world = view.world();
    let scheduled = world
        .consensus_schedule()
        .get(height)
        .ok_or(BeaconError::State("exact height schedule is absent"))?;
    let boundary = if mode == ConsensusMode::Npos {
        let policy = world
            .sumeragi_npos_parameters()
            .ok_or(BeaconError::State("signed NPoS policy is absent"))?;
        let epoch = scheduled.params.epoch_length_blocks;
        if epoch == 0 || epoch != policy.epoch_length_blocks().get() {
            return Err(BeaconError::State(
                "scheduled and signed NPoS epoch lengths differ",
            ));
        }
        height.checked_add(1).is_some_and(|next| next % epoch == 0)
    } else {
        false
    };
    let logical = BeaconSessionId::for_network_v1(view.network_id());
    let parliament = world
        .parliament_required_beacon_pulse_slots()
        .get(&(logical, height))
        .is_some_and(|attempts| !attempts.is_empty());
    Ok(boundary || parliament)
}

/// Resolve a pulse requirement before execution begins; no mutable status is a trust root.
pub fn current_requirement(
    state: &State,
    height: u64,
    mode: ConsensusMode,
) -> Result<Option<BeaconRequirement>, BeaconError> {
    let view = state.view();
    if !requested(&view, height, mode)? {
        return Ok(None);
    }
    let world = view.world();
    let roster = world
        .consensus_schedule()
        .get(height)
        .ok_or(BeaconError::State("exact height schedule is absent"))?
        .committee
        .clone();
    let session_id = world
        .global_beacon_active_session()
        .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
        .copied()
        .ok_or(BeaconError::State("active public session is absent"))?;
    let record = world
        .global_beacon_key_sessions()
        .get(&session_id)
        .ok_or(BeaconError::State("active public transcript is absent"))?;
    if !record.is_active_at(height) {
        return Err(BeaconError::State(
            "session is inactive at requested height",
        ));
    }
    let network_id = *view.network_id();
    let roster_hash =
        authenticated_global_threshold_beacon_roster_hash_v1(&record.session, &roster)
            .map_err(|_| BeaconError::Crypto)?;
    let session = validate_global_threshold_beacon_session_v1(
        record.session.clone(),
        &GlobalThresholdBeaconSessionBindingV1 {
            network_id,
            session_id,
            roster_hash,
            transcript_hash: record.session.transcript_hash,
        },
    )
    .map_err(|_| BeaconError::Crypto)?;
    let parent = GlobalThresholdBeaconChainAnchorV1 {
        height: height
            .checked_sub(1)
            .ok_or(BeaconError::State("pulse parent is absent"))?,
        block_hash: view
            .latest_block_hash()
            .ok_or(BeaconError::State("committed parent hash is absent"))?,
    };
    let logical = BeaconSessionId::for_network_v1(&network_id);
    if world.global_beacon_pulse_slots().len() != world.global_beacon_pulses().len()
        || world
            .global_beacon_pulse_slots()
            .get(&(logical, height))
            .is_some()
        || world
            .parliament_unavailable_beacon_pulse_slots()
            .get(&(logical, height))
            .is_some_and(|attempts| !attempts.is_empty())
    {
        return Err(BeaconError::State(
            "pulse history is inconsistent or the slot is closed",
        ));
    }
    if let Some(previous) = world
        .global_beacon_latest_pulse()
        .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
    {
        previous.validate().map_err(|_| BeaconError::Crypto)?;
        if previous.height >= height {
            return Err(BeaconError::State(
                "pulse history does not precede requested height",
            ));
        }
    } else if world.global_beacon_pulses().iter().next().is_some() {
        return Err(BeaconError::State(
            "pulse history has no authenticated cursor",
        ));
    }
    Ok(Some(BeaconRequirement {
        height,
        network_id,
        parent,
        roster,
        session,
    }))
}

#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
struct PartialFrame {
    instance: [u8; 32],
    height: u64,
    parent_hash: HashOf<BlockHeader>,
    partial: GlobalThresholdBeaconPartialSignatureV1,
}
/// Identify only this current application frame domain and finite transport bound.
pub fn is_frame(bytes: &[u8]) -> bool {
    bytes.len() > PREFIX.len() && bytes.len() <= MAX_FRAME_BYTES && bytes.starts_with(PREFIX)
}
fn decode(bytes: &[u8]) -> Result<PartialFrame, BeaconError> {
    if !is_frame(bytes) {
        return Err(BeaconError::Crypto);
    }
    norito::decode_canonical_with_limits(
        &bytes[PREFIX.len()..],
        norito::canonical_decode_limits(MAX_FRAME_BYTES),
    )
    .map_err(|_| BeaconError::Crypto)
}
fn encode(value: &PartialFrame) -> Result<Arc<[u8]>, BeaconError> {
    let mut bytes = PREFIX.to_vec();
    bytes.extend(norito::encode_canonical(value).map_err(|_| BeaconError::Crypto)?);
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(BeaconError::Crypto);
    }
    Ok(bytes.into())
}

#[derive(Default)]
struct Cache {
    result: Option<(
        u64,
        Result<Option<FinalizedGlobalThresholdBeaconPulseV1>, BeaconError>,
    )>,
    wakeup: Option<DriverHandle>,
}
enum Input {
    Demand(u64),
    Partial(PublicKey, Vec<u8>),
}
/// Nonblocking payload/ingress handle. Cryptography and provider calls run on its own worker.
pub struct BeaconService {
    state: Arc<State>,
    mode: ConsensusMode,
    cache: Arc<Mutex<Cache>>,
    sender: mpsc::SyncSender<Input>,
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
}
impl BeaconService {
    /// Spawn one bounded worker for one current consensus instance.
    pub fn spawn(
        state: Arc<State>,
        instance: Hash32,
        local: PeerId,
        signer: Option<Arc<dyn GlobalThresholdBeaconPartialSignerV1>>,
        net: Arc<dyn Net>,
        mode: ConsensusMode,
    ) -> std::io::Result<Arc<Self>> {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let cache = Arc::new(Mutex::new(Cache::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            state: Arc::clone(&state),
            instance,
            local,
            signer,
            net,
            mode,
            cache: Arc::clone(&cache),
            stop: Arc::clone(&stop),
            round: None,
        };
        let worker = thread::Builder::new()
            .name("sumeragi-beacon".into())
            .spawn(move || worker.run(receiver))?;
        Ok(Arc::new(Self {
            state,
            mode,
            cache,
            sender,
            stop,
            worker: Mutex::new(Some(worker)),
        }))
    }
    /// Wake outstanding transaction work as soon as the exact pulse becomes available.
    pub fn set_wakeup(&self, driver: DriverHandle) {
        let mut cache = self.cache.lock();
        if !self.stop.load(Ordering::Acquire) {
            cache.wakeup = Some(driver);
        }
    }
    /// Called only after real transactions have been selected. Never manufactures work.
    pub fn pulse_for_height(
        &self,
        height: u64,
    ) -> Result<Option<FinalizedGlobalThresholdBeaconPulseV1>, BeaconError> {
        if self.stop.load(Ordering::Acquire) {
            return Err(BeaconError::Unavailable);
        }
        if !requested(&self.state.view(), height, self.mode)? {
            return Ok(None);
        }
        let mut cache = self.cache.lock();
        if let Some((cached, result)) = &cache.result {
            if *cached == height {
                return result.clone();
            }
        }
        self.sender
            .try_send(Input::Demand(height))
            .map_err(|_| BeaconError::Unavailable)?;
        cache.result = Some((height, Err(BeaconError::Pending)));
        Err(BeaconError::Pending)
    }
    /// Stop independently of queue capacity, release wakeup ownership, and join the signer worker.
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        self.cache.lock().wakeup = None;
        if let Some(worker) = self.worker.lock().take() {
            let _ = worker.join();
        }
    }
    fn deliver(&self, from: &PublicKey, frame: &[u8]) -> bool {
        !self.stop.load(Ordering::Acquire)
            && is_frame(frame)
            && self
                .sender
                .try_send(Input::Partial(from.clone(), frame.to_vec()))
                .is_ok()
    }
}

impl Drop for BeaconService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Round {
    requirement: BeaconRequirement,
    aggregator: GlobalThresholdBeaconPulseAggregatorV1,
    local_frame: Option<Frame>,
    authorized: bool,
}
struct Worker {
    state: Arc<State>,
    instance: Hash32,
    local: PeerId,
    signer: Option<Arc<dyn GlobalThresholdBeaconPartialSignerV1>>,
    net: Arc<dyn Net>,
    mode: ConsensusMode,
    cache: Arc<Mutex<Cache>>,
    stop: Arc<AtomicBool>,
    round: Option<Round>,
}
impl Worker {
    fn run(mut self, receiver: mpsc::Receiver<Input>) {
        let mut next_retry = Instant::now() + RETRANSMIT;
        while !self.stop.load(Ordering::Acquire) {
            let input = receiver.recv_timeout(next_retry.saturating_duration_since(Instant::now()));
            if self.stop.load(Ordering::Acquire) {
                break;
            }
            match input {
                Ok(Input::Demand(height)) => self.demand(height),
                Ok(Input::Partial(from, bytes)) => {
                    let _ = self.receive(&from, &bytes);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            // Invalid traffic cannot postpone provider retries or retransmission forever.
            if Instant::now() >= next_retry {
                self.retry();
                next_retry = Instant::now() + RETRANSMIT;
            }
        }
    }
    fn demand(&mut self, height: u64) {
        if let Err(error) = self.open(height).and_then(|_| {
            if let Some(round) = self.round.as_mut() {
                round.authorized = true;
            }
            self.sign_local()
        }) {
            self.publish(height, Err(error));
        }
        self.finalize();
        self.broadcast();
    }
    fn retry(&mut self) {
        let next_height = u64::try_from(self.state.view().height())
            .ok()
            .and_then(|h| h.checked_add(1));
        if let Some(height) = self.round.as_ref().map(|r| r.requirement.height) {
            if next_height != Some(height) {
                self.round = None;
            } else {
                let _ = self.sign_local();
                self.finalize();
                self.broadcast();
                return;
            }
        }
        let height = {
            let mut cache = self.cache.lock();
            match cache.result.as_ref().map(|(height, _)| *height) {
                Some(height) if next_height == Some(height) => Some(height),
                _ => {
                    cache.result = None;
                    None
                }
            }
        };
        if let Some(height) = height {
            self.demand(height);
        }
    }
    fn open(&mut self, height: u64) -> Result<(), BeaconError> {
        if self
            .round
            .as_ref()
            .is_some_and(|round| round.requirement.height == height)
        {
            if u64::try_from(self.state.view().height())
                .ok()
                .and_then(|h| h.checked_add(1))
                == Some(height)
            {
                return Ok(());
            }
            return Err(BeaconError::State("partial belongs to a finished height"));
        }
        let requirement = current_requirement(&self.state, height, self.mode)?
            .ok_or(BeaconError::State("no pulse is requested"))?;
        let aggregator = GlobalThresholdBeaconPulseAggregatorV1::new(
            requirement.session.clone(),
            height,
            requirement.parent,
        )
        .map_err(|_| BeaconError::Crypto)?;
        self.round = Some(Round {
            requirement,
            aggregator,
            local_frame: None,
            authorized: false,
        });
        Ok(())
    }
    fn sign_local(&mut self) -> Result<(), BeaconError> {
        let Some(round) = self.round.as_mut() else {
            return Ok(());
        };
        if !round.authorized || round.local_frame.is_some() {
            return Ok(());
        }
        let Some(signer) = &self.signer else {
            return Ok(());
        };
        let Some(index) = round
            .requirement
            .roster
            .iter()
            .position(|peer| peer == &self.local)
        else {
            return Ok(());
        };
        let expected = u16::try_from(index + 1).map_err(|_| BeaconError::Crypto)?;
        let partial = signer
            .sign_partial(&round.requirement.session, round.aggregator.payload())
            .map_err(|_| BeaconError::Unavailable)?;
        if partial.signer_index != expected {
            return Err(BeaconError::Crypto);
        }
        #[cfg(feature = "test-network-parliament-signers")]
        let deliberately_invalid = signer.test_network_emit_invalid_outbound_partial_v1();
        #[cfg(not(feature = "test-network-parliament-signers"))]
        let deliberately_invalid = false;
        let partial = if deliberately_invalid {
            // Feature-isolated fault injection must not retain a hidden valid local share.
            let mut corrupted = partial;
            corrupted.signature_share[0] ^= 1;
            corrupted
        } else {
            round
                .aggregator
                .accept_partial(partial)
                .map_err(|_| BeaconError::Crypto)?;
            partial
        };
        round.local_frame = Some(Frame {
            instance: self.instance,
            class: TrafficClass::Control,
            bytes: encode(&PartialFrame {
                instance: self.instance.0,
                height: round.requirement.height,
                parent_hash: round.requirement.parent.block_hash,
                partial,
            })?,
        });
        self.finalize();
        Ok(())
    }
    fn receive(&mut self, from: &PublicKey, bytes: &[u8]) -> Result<(), BeaconError> {
        let message = decode(bytes)?;
        if message.instance != self.instance.0 {
            return Err(BeaconError::Crypto);
        }
        self.open(message.height)?;
        let round = self.round.as_mut().ok_or(BeaconError::Unavailable)?;
        let index = usize::from(
            message
                .partial
                .signer_index
                .checked_sub(1)
                .ok_or(BeaconError::Crypto)?,
        );
        let peer = PeerId::new(iroha_key(from).map_err(|_| BeaconError::Crypto)?);
        if round.requirement.parent.block_hash != message.parent_hash
            || round.requirement.roster.get(index) != Some(&peer)
        {
            return Err(BeaconError::Crypto);
        }
        round
            .aggregator
            .accept_partial(message.partial)
            .map_err(|_| BeaconError::Crypto)?;
        round.authorized = true;
        // A valid contribution authorizes participation in this exact requested slot; a
        // malformed transport claim alone never causes local signing.
        let _ = self.sign_local();
        self.finalize();
        self.broadcast();
        Ok(())
    }
    fn finalize(&self) {
        let Some(round) = &self.round else {
            return;
        };
        if round.aggregator.verified_partial_count()
            >= usize::from(round.requirement.session.record().threshold)
        {
            if let Ok(pulse) = round
                .aggregator
                .finalize()
                .map_err(|_| BeaconError::Crypto)
                .and_then(|pulse| {
                    round.requirement.verify(&pulse)?;
                    Ok(pulse)
                })
            {
                self.publish(round.requirement.height, Ok(Some(pulse)));
            }
        }
    }
    fn publish(
        &self,
        height: u64,
        result: Result<Option<FinalizedGlobalThresholdBeaconPulseV1>, BeaconError>,
    ) {
        let wakeup = {
            let mut cache = self.cache.lock();
            if cache
                .result
                .as_ref()
                .is_some_and(|(cached, old)| *cached == height && old == &result)
            {
                return;
            }
            cache.result = Some((height, result.clone()));
            if result.is_ok() {
                cache.wakeup.clone()
            } else {
                None
            }
        };
        if let Some(driver) = wakeup {
            driver.transactions_available();
        }
    }
    fn broadcast(&self) {
        if let Some(round) = &self.round {
            if let Some(frame) = &round.local_frame {
                for peer in &round.requirement.roster {
                    if peer != &self.local {
                        if let Ok(key) = core_key(peer.public_key()) {
                            self.net.send(&key, frame);
                        }
                    }
                }
            }
        }
    }
}

/// One authenticated ingress destination for the instance's consensus and application frames.
pub struct BeaconFrameSink {
    driver: DriverHandle,
    beacon: Arc<BeaconService>,
}
impl BeaconFrameSink {
    /// Preserve the driver's ordinary route while exposing only the bounded current beacon domain.
    pub fn new(driver: DriverHandle, beacon: Arc<BeaconService>) -> Self {
        Self { driver, beacon }
    }
}
impl FrameSink for BeaconFrameSink {
    fn deliver(&self, from: &PublicKey, frame: &[u8]) -> bool {
        if frame.starts_with(PREFIX) {
            self.beacon.deliver(from, frame)
        } else {
            self.driver.deliver(from, frame)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        beacon::{
            FinalizedGlobalThresholdBeaconKeySessionRecordV1,
            global_threshold_beacon_roster_hash_v1,
            prepared_session_and_signers_fixture_for_keys_v1,
        },
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        consensus::{GLOBAL_THRESHOLD_BEACON_VERSION_V1, GlobalThresholdBeaconDkgSessionV1},
        governance::types::GovernanceAttemptId,
    };
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct CaptureNet(Mutex<Vec<Frame>>);
    impl Net for CaptureNet {
        fn send(&self, _: &PublicKey, frame: &Frame) {
            self.0.lock().push(frame.clone());
        }
    }
    struct Fixture {
        chain: CertifiedTestChain,
        keys: Vec<KeyPair>,
        signers: Vec<Arc<dyn GlobalThresholdBeaconPartialSignerV1>>,
    }
    fn fixture() -> Fixture {
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000))
            .expect("signed current genesis");
        for time in [20_000, 30_000, 40_000] {
            chain.commit_at(time, Vec::new()); // A real signed clock transaction, never an empty block.
        }
        let mut keys = [0xC1, 0xC2, 0xC3, 0xC4]
            .into_iter()
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let roster = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            roster,
            chain
                .validators()
                .iter()
                .map(|(peer, _)| peer.clone())
                .collect::<Vec<_>>()
        );
        let (session, signers) = prepared_session_and_signers_fixture_for_keys_v1(
            GlobalThresholdBeaconDkgSessionV1 {
                version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
                network_id: chain.network_id(),
                session_id: [0xB1; 32],
                attempt_id: [0xB1; 32],
                authority_generation: 0,
                roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
                committee_size: 4,
                threshold: 2,
                start_height: 1,
                commitments_end_height: 2,
                deliveries_end_height: 3,
                acceptances_end_height: 4,
            },
            &keys,
        );
        let mut key =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(session.record().clone())
                .expect("complete authenticated DKG");
        key.activate(4).expect("active prepared session");
        {
            let mut world = chain.state().world.block();
            world
                .global_beacon_key_sessions
                .insert(key.session.session_id, key.clone());
            world.global_beacon_active_session.insert(
                GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
                key.session.session_id,
            );
            world.parliament_required_beacon_pulse_slots.insert(
                (BeaconSessionId::for_network_v1(&chain.network_id()), 5),
                BTreeSet::from([GovernanceAttemptId::new([0xC5; 32])]),
            );
            world.commit();
        }
        Fixture {
            chain,
            keys,
            signers: signers
                .into_iter()
                .map(|signer| Arc::new(signer) as Arc<dyn GlobalThresholdBeaconPartialSignerV1>)
                .collect(),
        }
    }
    fn worker(fixture: &Fixture, index: usize) -> Worker {
        Worker {
            state: fixture.chain.state().clone(),
            instance: fixture.chain.instance(),
            local: PeerId::new(fixture.keys[index].public_key().clone()),
            signer: Some(fixture.signers[index].clone()),
            net: Arc::new(CaptureNet::default()),
            mode: ConsensusMode::Permissioned,
            cache: Arc::new(Mutex::new(Cache::default())),
            stop: Arc::new(AtomicBool::new(false)),
            round: None,
        }
    }
    fn result(worker: &Worker) -> FinalizedGlobalThresholdBeaconPulseV1 {
        let cache = worker.cache.lock();
        match &cache.result {
            Some((5, Ok(Some(pulse)))) => *pulse,
            other => panic!("missing verified required pulse: {other:?}"),
        }
    }

    #[test]
    fn current_partials_require_exact_parent_instance_and_authenticated_seat() {
        let fixture = fixture();
        let mut receiver = worker(&fixture, 0);
        receiver.open(5).unwrap();
        let round = receiver.round.as_ref().unwrap();
        let partial = fixture.signers[1]
            .sign_partial(&round.requirement.session, round.aggregator.payload())
            .unwrap();
        let message = PartialFrame {
            instance: fixture.chain.instance().0,
            height: 5,
            parent_hash: round.requirement.parent.block_hash,
            partial,
        };
        let from = core_key(fixture.keys[1].public_key()).unwrap();
        let mut wrong = message.clone();
        wrong.instance[0] ^= 1;
        assert_eq!(
            receiver.receive(&from, &encode(&wrong).unwrap()),
            Err(BeaconError::Crypto)
        );
        wrong = message.clone();
        wrong.parent_hash = HashOf::from_untyped_unchecked(Hash::new(b"another committed parent"));
        assert_eq!(
            receiver.receive(&from, &encode(&wrong).unwrap()),
            Err(BeaconError::Crypto)
        );
        assert_eq!(
            receiver.receive(
                &core_key(fixture.keys[2].public_key()).unwrap(),
                &encode(&message).unwrap()
            ),
            Err(BeaconError::Crypto)
        );
        wrong = message.clone();
        wrong.partial.signature_share[0] ^= 1;
        assert_eq!(
            receiver.receive(&from, &encode(&wrong).unwrap()),
            Err(BeaconError::Crypto)
        );
        receiver.retry();
        assert!(!receiver.round.as_ref().unwrap().authorized);
        assert!(receiver.round.as_ref().unwrap().local_frame.is_none());
        assert!(receiver.cache.lock().result.is_none());
        receiver.receive(&from, &encode(&message).unwrap()).unwrap();
        let pulse = result(&receiver);
        current_requirement(fixture.chain.state(), 5, ConsensusMode::Permissioned)
            .unwrap()
            .unwrap()
            .verify(&pulse)
            .unwrap();
        receiver.receive(&from, &encode(&message).unwrap()).unwrap();
        assert_eq!(result(&receiver), pulse);
        let mut trailing = encode(&message).unwrap().to_vec();
        trailing.push(0);
        assert!(decode(&trailing).is_err());
        let mut oversized = PREFIX.to_vec();
        oversized.resize(MAX_FRAME_BYTES + 1, 0);
        assert!(!is_frame(&oversized));
        assert!(decode(&oversized).is_err());
    }

    #[test]
    fn current_partials_from_four_validators_converge_without_idle_signing() {
        let fixture = fixture();
        let mut workers = (0..4)
            .map(|index| worker(&fixture, index))
            .collect::<Vec<_>>();
        for worker in &mut workers {
            worker.retry();
            assert!(
                worker.round.is_none(),
                "idle polling must not initiate a beacon round"
            );
            assert!(worker.cache.lock().result.is_none());
            worker.demand(5);
            assert_eq!(
                worker
                    .round
                    .as_ref()
                    .unwrap()
                    .aggregator
                    .verified_partial_count(),
                1
            );
            assert!(
                worker.cache.lock().result.is_none(),
                "one signer is below threshold"
            );
        }
        let frames = workers
            .iter()
            .map(|worker| worker.round.as_ref().unwrap().local_frame.clone().unwrap())
            .collect::<Vec<_>>();
        for (index, worker) in workers.iter_mut().enumerate() {
            let sender = (index + 1) % frames.len();
            worker
                .receive(
                    &core_key(fixture.keys[sender].public_key()).unwrap(),
                    &frames[sender].bytes,
                )
                .unwrap();
        }
        let pulse = result(&workers[0]);
        for worker in &workers {
            assert_eq!(
                result(worker),
                pulse,
                "qualifying subsets produce the same unique pulse"
            );
        }
        let service = BeaconService::spawn(
            fixture.chain.state().clone(),
            fixture.chain.instance(),
            PeerId::new(fixture.keys[0].public_key().clone()),
            None,
            Arc::new(CaptureNet::default()),
            ConsensusMode::Permissioned,
        )
        .unwrap();
        // A stop never sits behind bounded queue backpressure or pending threshold demand.
        for _ in 0..=QUEUE_CAPACITY {
            let _ = service.sender.try_send(Input::Demand(5));
        }
        service.shutdown();
        service.shutdown();
        assert!(service.worker.lock().is_none());
        assert!(service.cache.lock().wakeup.is_none());
        assert_eq!(service.pulse_for_height(5), Err(BeaconError::Unavailable));
    }
}
