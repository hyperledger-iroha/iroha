//! A certified test chain (test support): a real signed genesis applied by
//! [`startup::apply_genesis`], blocks built by the leader's payload builder, executed, prepared,
//! stored and applied through the node's executor and block store, and every block certified by a
//! `CommitQC` of the fixed four-validator genesis committee with real BLS-normal signatures. The
//! [`certified_chain`](super::certified_chain) reader accepts its blocks as it accepts a running
//! node's.
//!
//! The only shortcut is who produces the certificate: the fixture signs the Commit votes itself
//! (with the committee's keys) instead of running the core's rounds. Block times follow the
//! chain's rule (parent time plus the cadence, after every transaction's creation time): the
//! fixture's cadence is one millisecond, and [`CertifiedTestChain::commit_at`] reaches an exact
//! block time with a `Log` transaction of its own clock account when the block's transactions do
//! not.

use std::{num::NonZeroU64, sync::Arc, time::Duration};

use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
use iroha_data_model::{
    IntoKeyValue, NetworkId, Registrable,
    account::{Account, AccountId},
    block::consensus_v2::ValidatorPower,
    block::{SignedBlock, consensus_v2::SumeragiV2GenesisContextParameters},
    domain::Domain,
    isi::{InstructionBox, Log},
    parameter::system::{ConsensusMode, SumeragiConsensusMode},
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
use iroha_logger::Level;
use iroha_model_base::{chain::ChainId, peer::PeerId};
use iroha_primitives::time::TimeSource;
use iroha_sumeragi::{
    api::ExecOutcome,
    crypto::{Signer as _, form_qc},
    message::{Block, BlockHeader, Qc, Vote, VoteKind},
    preimage::payload_hash,
    types::{Committee, Hash32},
};

use super::{
    block_store::{KuraBlockStore, Staging},
    certified_chain::{CommittedBlock, committed_block},
    crypto::{BlsCrypto, KeyPairSigner},
    driver::{
        SharedCrypto,
        traits::{BlockStore as _, Executor as _},
    },
    executor::{ExecutorContext, StateExecutor, attestation_required},
    node::global_instance,
    payload::{self, Assembly},
    startup::{self, GENESIS_HEIGHT},
};
use crate::{
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, StateReadOnly, World, WorldReadOnly},
    tx::AcceptedTransaction,
};

/// Seeds of the fixed validator committee (BLS-normal keys).
const VALIDATOR_SEEDS: [u8; 4] = [0xC1, 0xC2, 0xC3, 0xC4];
/// Seed of the fixture's clock account (Ed25519).
const CLOCK_SEED: u8 = 0xCC;

/// Which validators sign a block's `CommitQC`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signers {
    /// A quorum: the first three validators in committee order.
    Quorum,
    /// All four validators.
    All,
    /// Three validators other than the first ones (another valid certificate of the same block).
    LastThree,
    /// Two validators: below the quorum, a certificate that does not verify.
    BelowQuorum,
}

impl Signers {
    fn indices(self) -> &'static [u32] {
        match self {
            Self::Quorum => &[0, 1, 2],
            Self::All => &[0, 1, 2, 3],
            Self::LastThree => &[1, 2, 3],
            Self::BelowQuorum => &[0, 1],
        }
    }
}

/// How the test chain starts.
pub struct TestChainConfig {
    /// The chain id.
    pub chain_id: ChainId,
    /// The initial World (accounts, permissions, owners) genesis is applied to. The fixture adds
    /// the genesis domain, the genesis account and its clock account.
    pub world: World,
    /// The genesis key: its account authorizes every genesis transaction.
    pub genesis_key: KeyPair,
    /// Instructions of the genesis's ordinary transaction.
    pub genesis_instructions: Vec<InstructionBox>,
    /// Consensus mode carried by the signed genesis and used for execution.
    pub consensus_mode: SumeragiConsensusMode,
    /// Creation time of the first genesis transaction in milliseconds.
    pub genesis_time_ms: u64,
}

impl core::fmt::Debug for TestChainConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TestChainConfig")
            .field("chain_id", &self.chain_id)
            .field("genesis_account", &self.genesis_key.public_key())
            .field("genesis_instructions", &self.genesis_instructions.len())
            .field("genesis_time_ms", &self.genesis_time_ms)
            .finish_non_exhaustive()
    }
}

impl TestChainConfig {
    /// A chain over `world` with a fixed chain id and genesis key, genesis at `genesis_time_ms`.
    #[must_use]
    pub fn new(world: World, genesis_time_ms: u64) -> Self {
        Self {
            chain_id: ChainId::from("sumeragi-certified-test-chain"),
            world,
            genesis_key: KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519),
            genesis_instructions: Vec::new(),
            consensus_mode: SumeragiConsensusMode::Permissioned,
            genesis_time_ms,
        }
    }
}

/// A chain of certified blocks over one State (see the module documentation).
pub struct CertifiedTestChain {
    state: Arc<State>,
    kura: Arc<Kura>,
    genesis: SignedBlock,
    validated_genesis: iroha_genesis::ValidatedGenesisBundle,
    genesis_account: AccountId,
    executor: StateExecutor,
    blocks: KuraBlockStore,
    signers: Vec<KeyPairSigner>,
    committee: Committee,
    validators: Vec<(PeerId, Vec<u8>)>,
    crypto: Arc<BlsCrypto>,
    instance: Hash32,
    /// Height, core block hash and `R` of the tip.
    tip: (u64, Hash32, Hash32),
    router: Queue,
    clock: KeyPair,
}

impl core::fmt::Debug for CertifiedTestChain {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CertifiedTestChain")
            .field("instance", &self.instance)
            .field("tip", &self.tip)
            .finish_non_exhaustive()
    }
}

/// Why the test chain could not start.
#[derive(Debug, thiserror::Error)]
pub enum TestChainError {
    /// The genesis could not be built.
    #[error("genesis: {0}")]
    Genesis(String),
    /// Genesis did not apply (for example, one of its instructions failed).
    #[error(transparent)]
    Startup(#[from] startup::StartupError),
}

/// A chain that did not start, with the State genesis left behind (for inspection).
pub struct StartFailure {
    /// Why the chain did not start.
    pub error: TestChainError,
    /// The State with whatever genesis left unapplied.
    pub state: Arc<State>,
}

impl core::fmt::Debug for StartFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StartFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl CertifiedTestChain {
    /// Start a chain: build and sign genesis with the fixed committee, apply it to a fresh
    /// State over a fresh Kura, and spawn the executor.
    ///
    /// # Errors
    /// Genesis cannot be built or does not apply; the State (with whatever genesis left
    /// unapplied) is returned for inspection.
    pub fn start(config: TestChainConfig) -> Result<Self, StartFailure> {
        iroha_genesis::init_instruction_registry();
        let TestChainConfig {
            chain_id,
            mut world,
            genesis_key,
            genesis_instructions,
            consensus_mode,
            genesis_time_ms,
        } = config;
        let genesis_account = AccountId::new(genesis_key.public_key().clone());
        let clock = KeyPair::from_seed(vec![CLOCK_SEED; 32], Algorithm::Ed25519);
        let clock_account = AccountId::new(clock.public_key().clone());
        let domain_id = iroha_genesis::GENESIS_DOMAIN_ID.clone();
        if world.domains.view().get(&domain_id).is_none() {
            let domain = Domain::new(domain_id.clone()).build(&genesis_account);
            world.insert_domain_for_testing(domain_id, domain);
        }
        for account in [&genesis_account, &clock_account] {
            if world.accounts.view().get(account).is_none() {
                let (id, value) = Account::new(account.clone())
                    .build(account)
                    .into_key_value();
                world.accounts.insert(id, value);
            }
        }
        let mut keys = VALIDATOR_SEEDS
            .iter()
            .map(|seed| KeyPair::from_seed(vec![*seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let validators = keys
            .iter()
            .map(|key| {
                (
                    PeerId::new(key.public_key().clone()),
                    bls_normal_pop_prove(key.private_key()).expect("PoP of a fixture key"),
                )
            })
            .collect::<Vec<_>>();
        let (genesis, manifest) = match build_genesis(
            &chain_id,
            &genesis_key,
            &validators,
            genesis_instructions,
            consensus_mode,
            genesis_time_ms,
        ) {
            Ok(genesis) => genesis,
            Err(error) => {
                let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
                    world,
                    Kura::blank_kura_for_testing(),
                    LiveQueryStore::start_test(),
                    chain_id,
                    NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                        iroha_crypto::Hash::new(b"unbuilt genesis"),
                    )),
                ));
                return Err(StartFailure {
                    error: TestChainError::Genesis(error),
                    state,
                });
            }
        };
        let kura = Kura::blank_kura_for_testing();
        let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
            world,
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            chain_id.clone(),
            NetworkId::from_genesis_hash(genesis.hash()),
        ));
        let nexus = state.nexus_snapshot();
        state.install_lane_manifests(&Arc::new(
            LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
        ));
        let tip = match startup::apply_genesis(
            &state,
            genesis.clone(),
            &genesis_account,
            consensus_mode.into(),
            None,
        ) {
            Ok(tip) => tip,
            Err(error) => {
                return Err(StartFailure {
                    error: error.into(),
                    state,
                });
            }
        };
        let validated_genesis = iroha_genesis::validate_prepared_genesis_bundle(
            &genesis.encode_wire().expect("fixture genesis framing"),
            &manifest, genesis_key.public_key(), genesis.hash(),
        ).expect("fixture signed genesis and manifest agree");
        let crypto = Arc::new(BlsCrypto::new());
        crypto
            .admit_committee(
                validators
                    .iter()
                    .map(|(peer, pop)| (peer.public_key(), pop.as_slice())),
            )
            .expect("fixture committee admits");
        let shared: SharedCrypto = crypto.clone();
        let staging = Staging::new();
        let blocks =
            KuraBlockStore::new(Arc::clone(&kura), shared, GENESIS_HEIGHT, staging.clone());
        let executor = StateExecutor::spawn(ExecutorContext {
            state: Arc::clone(&state),
            queue: None,
            staging,
            events: tokio::sync::broadcast::channel(1024).0,
            genesis_account: genesis_account.clone(),
            consensus_mode: consensus_mode.into(),
            applied: (GENESIS_HEIGHT, tip.block_hash),
            crypto: Some(Arc::clone(&crypto)),
        })
        .expect("executor thread");
        let signers = keys
            .iter()
            .map(|key| KeyPairSigner::new(key).expect("BLS-normal fixture key"))
            .collect::<Vec<_>>();
        let committee = Committee::new(
            signers
                .iter()
                .map(|signer| signer.public_key().clone())
                .collect(),
        )
        .expect("fixture committee");
        let instance = global_instance(&genesis, &chain_id.to_string());
        let router = Queue::from_config(
            iroha_config::parameters::actual::Queue::default(),
            tokio::sync::broadcast::channel(16).0,
        );
        Ok(Self {
            state,
            kura,
            genesis,
            validated_genesis,
            genesis_account,
            executor,
            blocks,
            signers,
            committee,
            validators,
            crypto,
            instance,
            tip: (GENESIS_HEIGHT, tip.block_hash, tip.result),
            router,
            clock,
        })
    }

    /// The State the chain applies to.
    #[must_use]
    pub fn state(&self) -> &Arc<State> {
        &self.state
    }

    /// The chain's Kura.
    #[must_use]
    pub fn kura(&self) -> &Arc<Kura> {
        &self.kura
    }

    /// The signed genesis block.
    #[must_use]
    pub fn genesis(&self) -> &SignedBlock {
        &self.genesis
    }

    /// The independently validated signed genesis and its exact source manifest binding.
    #[must_use]
    pub fn validated_genesis(&self) -> &iroha_genesis::ValidatedGenesisBundle {
        &self.validated_genesis
    }

    /// The genesis account.
    #[must_use]
    pub fn genesis_account(&self) -> &AccountId {
        &self.genesis_account
    }

    /// The network id (the genesis hash).
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        NetworkId::from_genesis_hash(self.genesis.hash())
    }

    /// The chain instance `I`.
    #[must_use]
    pub fn instance(&self) -> Hash32 {
        self.instance
    }

    /// The committed height.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.tip.0
    }

    /// The validators with their proofs of possession, in committee order.
    #[must_use]
    pub fn validators(&self) -> &[(PeerId, Vec<u8>)] {
        &self.validators
    }

    /// The consensus-visible receipt of `height`.
    ///
    /// # Panics
    /// The height is not committed.
    #[must_use]
    pub fn committed(&self, height: u64) -> CommittedBlock {
        committed_block(&self.state.view(), height).expect("a committed fixture height")
    }

    /// Commit `transactions` in one block at the chain's next canonical time, with a quorum
    /// certificate. Returns whether each transaction executed successfully.
    ///
    /// # Panics
    /// A transaction is not accepted or the block does not execute `Valid` (fixture misuse).
    pub fn commit(&mut self, transactions: Vec<SignedTransaction>) -> Vec<bool> {
        self.commit_with(None, transactions, Signers::Quorum)
    }

    /// [`Self::commit`] with block time `time_ms`: the fixture adds a `Log` transaction of its
    /// clock account, created one millisecond earlier, when the block's own transactions would
    /// leave it earlier. A block cannot be earlier than its parent plus the cadence or than a
    /// transaction it carries: the time is then the earliest canonical one.
    ///
    /// # Panics
    /// See [`Self::commit`].
    pub fn commit_at(&mut self, time_ms: u64, transactions: Vec<SignedTransaction>) -> Vec<bool> {
        self.commit_with(Some(time_ms), transactions, Signers::Quorum)
    }

    /// Commit one block (see [`Self::commit_at`]) certified by `signers`.
    ///
    /// # Panics
    /// See [`Self::commit_at`].
    pub fn commit_with(
        &mut self,
        time_ms: Option<u64>,
        mut transactions: Vec<SignedTransaction>,
        signers: Signers,
    ) -> Vec<bool> {
        let submitted = transactions.len();
        let height = self.tip.0 + 1;
        let view = self.state.view();
        let parent = view.latest_block().expect("the applied parent");
        let scheduled = view
            .world()
            .consensus_schedule()
            .get(height)
            .cloned()
            .expect("the schedule covers the next height");
        let transaction_parameters = view.world().parameters().transaction();
        drop(view);
        let cadence = Duration::from_millis(scheduled.params.block_time_ms);
        let parent_time = parent.header().creation_time();
        let inputs_time = |transactions: &[SignedTransaction]| {
            transactions
                .iter()
                .map(|tx| tx.creation_time() + Duration::from_millis(1))
                .fold(parent_time + cadence, Duration::max)
        };
        if let Some(time_ms) = time_ms
            && inputs_time(&transactions) < Duration::from_millis(time_ms)
        {
            transactions.push(self.tick(time_ms - 1));
        }
        if transactions.is_empty() {
            transactions.push(self.tick(u64::try_from((parent_time + cadence).as_millis()).expect("fixture time fits") - 1));
        }
        let block_time = inputs_time(&transactions);
        let (_, time_source) = TimeSource::new_mock(block_time);
        let accepted = transactions
            .into_iter()
            .map(|tx| {
                let accepted = AcceptedTransaction::accept_with_time_source(
                    tx,
                    &self.network_id(),
                    Duration::from_secs(1),
                    transaction_parameters,
                    &iroha_config::parameters::actual::Crypto::default(),
                    &time_source,
                )
                .expect("the fixture transaction is accepted");
                let plan = self
                    .router
                    .route_plan_with_state(&accepted, &self.state)
                    .expect("the fixture transaction routes");
                (accepted, plan)
            })
            .collect::<Vec<_>>();
        let assembly = Assembly {
            parent: &parent,
            view: 0,
            cadence,
        };
        let proposal = payload::assemble(&self.state, assembly, &accepted).expect("assembly");
        assert_eq!(
            proposal.header().creation_time(),
            block_time,
            "the fixture predicts the canonical block time"
        );
        let payload_bytes = payload::encode(&proposal).expect("non-empty payload");
        let header = BlockHeader {
            instance: self.instance,
            height,
            origin_view: 0,
            parent_hash: self.tip.1,
            parent_result: self.tip.2,
            payload_hash: payload_hash(&*self.crypto, &payload_bytes),
            payload_len: u32::try_from(payload_bytes.len()).expect("payload fits"),
            proposer: 0,
            skipped_leaders: Vec::new(),
            attest: attestation_required(&proposal),
        };
        let block = Block {
            header,
            payload: payload_bytes,
        };
        let block_hash = block.hash(&*self.crypto);
        let result = match self.executor.execute(&block, &block_hash) {
            Some(ExecOutcome::Valid(result)) => result,
            other => panic!("fixture block {height} does not execute: {other:?}"),
        };
        let commit_qc = self.commit_qc(height, block_hash, result, block.header.attest, signers);
        assert_eq!(
            self.executor.prepare(&block, &commit_qc).expect("prepare"),
            Some(result)
        );
        self.blocks.append(&block, &commit_qc).expect("append");
        self.executor.commit(&block, &commit_qc).expect("commit");
        self.tip = (height, block_hash, result);
        let stored = self.committed(height);
        (0..submitted)
            .map(|index| {
                stored
                    .block()
                    .network_output_at(u32::try_from(index).expect("index fits"))
                    .is_some_and(|(_, output)| output.result.is_ok())
            })
            .collect()
    }

    /// A `CommitQC` of `(height, block_hash, result)` signed by `signers` (view 0).
    #[must_use]
    pub fn commit_qc(
        &self,
        height: u64,
        block_hash: Hash32,
        result: Hash32,
        attest: bool,
        signers: Signers,
    ) -> Qc {
        let votes = signers
            .indices()
            .iter()
            .map(|&signer| {
                let mut vote = Vote {
                    kind: VoteKind::Commit,
                    instance: self.instance,
                    height,
                    view: 0,
                    block_hash,
                    result,
                    attest,
                    signer,
                    sig: iroha_sumeragi::types::Signature(
                        [0; iroha_sumeragi::types::SIGNATURE_LEN],
                    ),
                    attestation: None,
                };
                vote.sig = self.signers[signer as usize].sign(&vote.preimage());
                vote
            })
            .collect::<Vec<_>>();
        let refs = votes.iter().collect::<Vec<_>>();
        match form_qc(&*self.crypto, self.committee.n(), &refs) {
            Ok(qc) => qc,
            // Below the quorum `form_qc` refuses: aggregate by hand (a certificate that does not
            // verify, for negative tests).
            Err(_) => {
                let mut signers_bitmap = iroha_sumeragi::types::Bitmap::new(self.committee.n());
                for vote in &votes {
                    signers_bitmap.set(vote.signer);
                }
                let sigs = votes.iter().map(|vote| vote.sig).collect::<Vec<_>>();
                Qc {
                    kind: VoteKind::Commit,
                    instance: self.instance,
                    height,
                    view: 0,
                    block_hash,
                    result,
                    attest,
                    signers: signers_bitmap,
                    agg_sig: iroha_sumeragi::crypto::Crypto::aggregate(&*self.crypto, &sigs),
                    attestations: Vec::new(),
                }
            }
        }
    }

    /// Test setup outside consensus: run `edit` on a transaction of an overlay for the next
    /// height at block time `time_ms` and publish only its World changes (no block, no height).
    /// The next [`Self::commit_at`] at `time_ms` commits the block the edited World is read at;
    /// rows the edit records carry that height and time.
    ///
    /// # Panics
    /// The overlay cannot be published.
    pub fn setup_world_at(
        &self,
        time_ms: u64,
        edit: impl FnOnce(&mut crate::state::StateTransaction<'_, '_>),
    ) {
        let view = self.state.view();
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(self.tip.0 + 1).expect("non-zero"),
            view.latest_block_hash(),
            None,
            time_ms,
            0,
        );
        drop(view);
        let mut block = self.state.block(header);
        let mut transaction = block.transaction();
        edit(&mut transaction);
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("fixture World setup publishes");
    }

    /// A signed transaction of `key`'s account on this chain, created at `created_ms`.
    #[must_use]
    pub fn sign(
        &self,
        key: &KeyPair,
        instructions: impl IntoIterator<Item = InstructionBox>,
        created_ms: u64,
    ) -> SignedTransaction {
        let mut builder = TransactionBuilder::new(
            self.network_id(),
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(created_ms));
        builder
            .with_instructions(instructions)
            .sign(key.private_key())
    }

    fn tick(&self, created_ms: u64) -> SignedTransaction {
        self.sign(
            &self.clock,
            [InstructionBox::from(Log::new(
                Level::DEBUG,
                format!("certified test chain clock {created_ms}"),
            ))],
            created_ms,
        )
    }
}

/// The signed genesis of the fixed committee with `instructions` in its ordinary transaction.
fn build_genesis(
    chain_id: &ChainId,
    genesis_key: &KeyPair,
    validators: &[(PeerId, Vec<u8>)],
    instructions: Vec<InstructionBox>,
    consensus_mode: SumeragiConsensusMode,
    genesis_time_ms: u64,
) -> Result<(SignedBlock, iroha_genesis::RawGenesisTransaction), String> {
    let entries = validators
        .iter()
        .map(|(peer, pop)| GenesisTopologyEntry::new(peer.clone(), pop.clone()))
        .collect::<Vec<_>>();
    let roster = validators
        .iter()
        .map(|(peer, _)| ValidatorPower {
            validator: peer.clone(),
            power: 1,
        })
        .collect::<Vec<_>>();
    let builder = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
        .with_block_cadence_ms(NonZeroU64::new(1).expect("non-zero"))
        .set_topology(entries)
        .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
        .with_kagemusha_mint_finality_genesis_parameters(
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
        );
    let builder = instructions
        .into_iter()
        .fold(builder, GenesisBuilder::append_instruction);
    let raw = builder.build_raw().map_err(|error| format!("{error:#}"))?
        .with_consensus_mode(consensus_mode).with_consensus_meta();
    let genesis = raw.clone()
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
            genesis_key, None, None, genesis_time_ms,
        ).map_err(|error| format!("{error:#}"))?;
    Ok((genesis.0, raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_commit_certified_at_exact_times() {
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000))
            .expect("chain starts");
        assert_eq!(chain.height(), 1);
        let genesis = chain.committed(1);
        assert!(genesis.header().is_none());
        assert_eq!(chain.commit_at(20_000, Vec::new()), Vec::<bool>::new());
        assert_eq!(chain.height(), 2);
        let block = chain.committed(2);
        assert_eq!(block.block_time_ms(), 20_000);
        assert!(block.extends(&genesis));
        // Without a target time the block follows its parent by the one-millisecond cadence.
        chain.commit(Vec::new());
        assert_eq!(chain.committed(3).block_time_ms(), 20_001);
        assert!(chain.committed(3).extends(&block));
        // A transaction of an unknown account executes and fails; its outcome is reported.
        let stranger = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
        let failing = chain.sign(
            &stranger,
            [InstructionBox::from(Log::new(Level::INFO, "who".into()))],
            29_999,
        );
        assert_eq!(chain.commit_at(30_000, vec![failing]), [false]);
        assert_eq!(chain.committed(4).block_time_ms(), 30_000);
    }
}
