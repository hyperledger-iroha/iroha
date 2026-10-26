//! When each signer's approval of a native multisig proposal landed, read from committed blocks.
//!
//! The ledger keeps who approved a proposal, not when. Every committed proposing or approving
//! transaction carries its signer and its block's time and height, so this node folds them into an
//! index as blocks commit: the proposer's approval lands with its `MultisigPropose`, every other
//! signer's with its first `MultisigApprove`. Only successful transactions' top-level instructions
//! count, so an approval made inside another proposal's execution or by a contract is not seen and
//! stays untimed. The index changes nothing the chain executes: it ships with an in-place node
//! update and covers the existing history.

use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::{
        Arc, LazyLock, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId, account::AccountId, block::BlockHeader, isi::InstructionBox,
    transaction::signed::TransactionEntrypoint,
};
use iroha_executor_data_model::isi::multisig::{
    MultisigInstructionBox, MultisigProposalId, MultisigProposalValue,
};

use crate::routing::MultisigProposalApprovalDto;

/// Most blocks one read folds in before answering. A longer gap, as after a restart, is left
/// to a single background pass, and approvals in blocks it has not reached read as untimed.
const REQUEST_TAIL_BLOCKS: usize = 16;
/// Most landings kept. Past it the index stops growing, so its memory stays bounded.
const MAX_LANDINGS: usize = 200_000;

/// The block an approval landed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Landing {
    /// Block creation time, Unix milliseconds.
    pub(crate) at_ms: u64,
    /// Block height.
    pub(crate) height: u64,
}

/// First landing of each (proposal, signer) pair, folded from committed blocks up to `height`.
#[derive(Clone, Debug, Default)]
pub(crate) struct MultisigApprovalLandings {
    height: usize,
    tip: Option<HashOf<BlockHeader>>,
    first: BTreeMap<(MultisigProposalId, AccountId), Landing>,
}

/// The proposal a top-level instruction proposes or approves, if it is either.
fn approval_target(
    network: &NetworkId,
    instruction: &InstructionBox,
) -> Option<MultisigProposalId> {
    match MultisigInstructionBox::try_from(instruction) {
        Ok(MultisigInstructionBox::Propose(propose)) => Some(MultisigProposalId::derive(
            network,
            &propose.account,
            &propose.proposal_nonce,
        )),
        Ok(MultisigInstructionBox::Approve(approve)) => Some(approve.proposal_id),
        _ => None,
    }
}

impl MultisigApprovalLandings {
    /// Fold one committed block: `approvals` lists the proposals that its successful external
    /// transactions proposed or approved, each with the transaction's signer.
    fn fold(
        &mut self,
        height: usize,
        hash: HashOf<BlockHeader>,
        at_ms: u64,
        approvals: impl IntoIterator<Item = (MultisigProposalId, AccountId)>,
    ) {
        let landing = Landing {
            at_ms,
            height: u64::try_from(height).unwrap_or(u64::MAX),
        };
        for key in approvals {
            if self.first.len() >= MAX_LANDINGS {
                break;
            }
            self.first.entry(key).or_insert(landing);
        }
        self.height = height;
        self.tip = Some(hash);
    }

    /// The landing that put `approver`'s approval on `proposal`, inside its signing window.
    fn approval(&self, proposal: &MultisigProposalValue, approver: &AccountId) -> Option<Landing> {
        self.first
            .get(&(proposal.proposal_id, approver.clone()))
            .copied()
            .filter(|landing| {
                proposal.proposed_at_ms <= landing.at_ms && landing.at_ms < proposal.expires_at_ms
            })
    }

    /// One entry per approver in `proposal.approvals`, in the order the approvals landed. An
    /// approval this node has not seen (or not folded yet) has no time and sorts last.
    pub(crate) fn approvals(
        &self,
        proposal: &MultisigProposalValue,
    ) -> Vec<MultisigProposalApprovalDto> {
        let mut approvals: Vec<_> = proposal
            .approvals
            .iter()
            .map(|approver| {
                let landing = self.approval(proposal, approver);
                MultisigProposalApprovalDto {
                    approver: approver.clone(),
                    approved_at_ms: landing.map(|landing| landing.at_ms),
                    block_height: landing.map(|landing| landing.height),
                }
            })
            .collect();
        approvals.sort_by(|left, right| {
            (
                left.block_height.is_none(),
                left.block_height,
                &left.approver,
            )
                .cmp(&(
                    right.block_height.is_none(),
                    right.block_height,
                    &right.approver,
                ))
        });
        approvals
    }
}

static LANDINGS: LazyLock<RwLock<Arc<MultisigApprovalLandings>>> = LazyLock::new(RwLock::default);
static FOLDING: Mutex<()> = Mutex::new(());
static BACKFILLING: AtomicBool = AtomicBool::new(false);

/// Clears `BACKFILLING` when the background pass ends, also by panic.
struct Backfilling;

impl Drop for Backfilling {
    fn drop(&mut self) {
        BACKFILLING.store(false, Ordering::Release);
    }
}

fn snapshot() -> Arc<MultisigApprovalLandings> {
    match LANDINGS.read() {
        Ok(guard) => Arc::clone(&guard),
        Err(poisoned) => Arc::clone(&poisoned.into_inner()),
    }
}

fn publish(landings: MultisigApprovalLandings) -> Arc<MultisigApprovalLandings> {
    let landings = Arc::new(landings);
    match LANDINGS.write() {
        Ok(mut guard) => *guard = Arc::clone(&landings),
        Err(poisoned) => *poisoned.into_inner() = Arc::clone(&landings),
    }
    landings
}

/// The index for a read that asked for approval times, folded up to the committed tip when only
/// a short tail is missing. A longer gap starts one background pass and answers with what is
/// folded so far. Emergency Fast mode reads no history, so its approvals stay untimed.
pub(crate) async fn current(app: &crate::SharedAppState) -> Arc<MultisigApprovalLandings> {
    let folded = snapshot();
    let tip = app.state.committed_height();
    if folded.height >= tip || app.state.emergency_fast_startup_enabled() {
        return folded;
    }
    if tip - folded.height <= REQUEST_TAIL_BLOCKS && extends_committed_chain(app, &folded) {
        let app = app.clone();
        return tokio::task::spawn_blocking(move || fold_to_tip(&app, false))
            .await
            .unwrap_or(folded);
    }
    if !BACKFILLING.swap(true, Ordering::AcqRel) {
        let app = app.clone();
        tokio::task::spawn_blocking(move || {
            let _backfilling = Backfilling;
            fold_to_tip(&app, true);
        });
    }
    folded
}

/// Whether the committed chain still holds the block `landings` was folded up to.
fn extends_committed_chain(
    app: &crate::SharedAppState,
    landings: &MultisigApprovalLandings,
) -> bool {
    landings.height == 0
        || app
            .state
            .committed_block_hash_at_height(u64::try_from(landings.height).unwrap_or(u64::MAX))
            == landings.tip
}

/// Fold committed blocks into the shared index from its folded height. A read folds at most
/// `REQUEST_TAIL_BLOCKS` blocks and gives up at once while another pass runs, the history pool is
/// busy or the chain no longer holds the folded tip. The background pass folds to the tip (from
/// genesis again on a replaced chain), publishes its progress as it goes and retries a busy
/// history pool. A block too large for the fetch budget is skipped, so its approvals stay untimed;
/// any other refusal stops the pass until a later read.
fn fold_to_tip(app: &crate::SharedAppState, background: bool) -> Arc<MultisigApprovalLandings> {
    let _folding = if background {
        match FOLDING.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    } else {
        match FOLDING.try_lock() {
            Ok(guard) => guard,
            Err(_) => return snapshot(),
        }
    };
    let folded = snapshot();
    let tip = app.state.committed_height();
    let mut landings = match (extends_committed_chain(app, &folded), background) {
        // A replaced chain (another block at the folded height) is folded again from genesis.
        (false, true) => MultisigApprovalLandings::default(),
        (false, false) => return folded,
        (true, _) if folded.height >= tip => return folded,
        (true, _) => (*folded).clone(),
    };
    let stop = if background {
        tip
    } else {
        tip.min(landings.height + REQUEST_TAIL_BLOCKS)
    };
    let network = *app.state.network_id_ref();
    let mut busy_retries = 0_u32;
    while landings.height < stop {
        let Some(height) = NonZeroUsize::new(landings.height + 1) else {
            break;
        };
        let Some(hash) = app
            .state
            .committed_block_hash_at_height(u64::try_from(height.get()).unwrap_or(u64::MAX))
        else {
            break;
        };
        match read_block_approvals(app, &network, height, hash) {
            Ok((at_ms, approvals)) => {
                busy_retries = 0;
                landings.fold(height.get(), hash, at_ms, approvals);
                if background && landings.height % 64 == 0 {
                    publish(landings.clone());
                }
            }
            Err(error) => match crate::background_history_refusal(&error) {
                // A busy history pool: a later read goes on from here.
                crate::BackgroundHistoryRefusal::Transient if !background => break,
                crate::BackgroundHistoryRefusal::Transient if busy_retries < 50 => {
                    busy_retries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                crate::BackgroundHistoryRefusal::Budget => {
                    iroha_logger::warn!(
                        ?error,
                        height = height.get(),
                        "multisig approval times skip a block larger than the history fetch budget"
                    );
                    landings.fold(height.get(), hash, 0, []);
                }
                _ => {
                    iroha_logger::warn!(
                        ?error,
                        height = height.get(),
                        "multisig approval times stop at a block this node could not read"
                    );
                    break;
                }
            },
        }
    }
    publish(landings)
}

/// Read one committed block under a fresh background history owner: its creation time and the
/// proposals its successful external transactions proposed or approved, with their signers.
fn read_block_approvals(
    app: &crate::SharedAppState,
    network: &NetworkId,
    height: NonZeroUsize,
    hash: HashOf<BlockHeader>,
) -> Result<(u64, Vec<(MultisigProposalId, AccountId)>), crate::Error> {
    let original = crate::read_background_execution_carrier(app, height)?;
    let mut approvals = Vec::new();
    let header = original
        .owner
        .scope(|| {
            original
                .carrier
                .visit_network_transactions(hash, |source, result| {
                    if !result.is_ok() {
                        return;
                    }
                    let transaction = match source {
                        TransactionEntrypoint::External(transaction) => transaction,
                        TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
                        TransactionEntrypoint::SealedCommitment(_)
                        | TransactionEntrypoint::AnonymousBallot(_) => return,
                    };
                    for instruction in transaction.instructions().explicit_instructions() {
                        if let Some(proposal) = approval_target(network, instruction) {
                            approvals.push((proposal, transaction.authority().clone()));
                        }
                    }
                })
        })
        .map_err(|error| crate::canonical_history::query_attempt_error(error.into()))?;
    let at_ms = u64::try_from(header.creation_time().as_millis()).unwrap_or(u64::MAX);
    Ok((at_ms, approvals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        account::Account,
        isi::{Log, Register},
    };
    use iroha_executor_data_model::isi::multisig::{
        MultisigApprove, MultisigProposalNonce, MultisigPropose,
    };
    use std::collections::BTreeSet;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }
    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x47; 32])))
    }
    fn block_hash(height: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([height; 32]))
    }
    fn nonce(seed: u8) -> MultisigProposalNonce {
        MultisigProposalNonce::new([seed; 32]).unwrap()
    }
    fn proposal(
        multisig: &AccountId,
        nonce: MultisigProposalNonce,
        proposed_at_ms: u64,
        approvals: impl IntoIterator<Item = AccountId>,
    ) -> MultisigProposalValue {
        MultisigProposalValue {
            network_id: network(),
            proposal_nonce: nonce,
            proposal_id: MultisigProposalId::derive(&network(), multisig, &nonce),
            transaction_ttl_ms: None,
            instructions: Vec::new(),
            proposed_at_ms,
            expires_at_ms: proposed_at_ms + 60_000,
            approvals: approvals.into_iter().collect::<BTreeSet<_>>(),
        }
    }
    /// The fixture app, reading `chain`'s state and blocks.
    fn app_reading(chain: &CertifiedTestChain) -> crate::SharedAppState {
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let unique = Arc::get_mut(&mut app).expect("unique fixture app");
        unique.state = chain.state().clone();
        unique.kura = chain.kura().clone();
        app
    }
    /// Run `fold` on an empty shared index. Tests that fold through it take turns.
    fn with_empty_index<T>(fold: impl FnOnce() -> T) -> T {
        static TURN: Mutex<()> = Mutex::new(());
        let _turn = TURN
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        publish(MultisigApprovalLandings::default());
        fold()
    }

    #[test]
    fn proposals_and_approvals_name_their_target_occurrence() {
        let multisig = account(1);
        let id = MultisigProposalId::derive(&network(), &multisig, &nonce(0x21));
        let propose: InstructionBox =
            MultisigPropose::new(multisig.clone(), nonce(0x21), Vec::new(), None).into();
        let approve: InstructionBox =
            MultisigApprove::new(multisig, id, HashOf::new(&Vec::<InstructionBox>::new())).into();
        let log: InstructionBox = Log::new(iroha_data_model::Level::INFO, "other".into()).into();
        assert_eq!(approval_target(&network(), &propose), Some(id));
        assert_eq!(approval_target(&network(), &approve), Some(id));
        assert_eq!(approval_target(&network(), &log), None);
    }

    #[test]
    fn each_approval_takes_its_signers_first_landing_in_block_order() {
        let multisig = account(1);
        let (proposer, second, third) = (account(2), account(3), account(4));
        let id = MultisigProposalId::derive(&network(), &multisig, &nonce(0x21));
        let mut landings = MultisigApprovalLandings::default();
        landings.fold(7, block_hash(7), 1_000, [(id, proposer.clone())]);
        landings.fold(9, block_hash(9), 3_000, [(id, third.clone())]);
        // A repeated approval keeps the signer's first landing.
        landings.fold(
            12,
            block_hash(12),
            5_000,
            [(id, third.clone()), (id, second.clone())],
        );
        let approvals = landings.approvals(&proposal(
            &multisig,
            nonce(0x21),
            1_000,
            [proposer.clone(), second.clone(), third.clone()],
        ));
        let times: Vec<_> = approvals
            .into_iter()
            .map(|approval| {
                (
                    approval.approver,
                    approval.approved_at_ms,
                    approval.block_height,
                )
            })
            .collect();
        assert_eq!(
            times,
            [
                (proposer, Some(1_000), Some(7)),
                (third, Some(3_000), Some(9)),
                (second, Some(5_000), Some(12)),
            ]
        );
        assert_eq!(landings.height, 12);
        assert_eq!(landings.tip, Some(block_hash(12)));
    }

    #[test]
    fn approvals_this_node_did_not_see_stay_untimed_and_sort_last() {
        let multisig = account(1);
        let (proposer, nested, late) = (account(2), account(5), account(6));
        let id = MultisigProposalId::derive(&network(), &multisig, &nonce(0x22));
        let other = MultisigProposalId::derive(&network(), &multisig, &nonce(0x23));
        let mut landings = MultisigApprovalLandings::default();
        // The proposer's landing is for another occurrence; `late` approved after the deadline;
        // `nested` approved inside another proposal's execution, which no block lists.
        landings.fold(3, block_hash(3), 500, [(other, proposer.clone())]);
        landings.fold(40, block_hash(40), 70_000, [(id, late.clone())]);
        let approvals = landings.approvals(&proposal(
            &multisig,
            nonce(0x22),
            1_000,
            [proposer.clone(), nested.clone(), late.clone()],
        ));
        assert!(approvals.iter().all(|approval| {
            approval.approved_at_ms.is_none() && approval.block_height.is_none()
        }));
        let mut expected = vec![proposer, nested, late];
        expected.sort();
        assert_eq!(
            approvals
                .into_iter()
                .map(|approval| approval.approver)
                .collect::<Vec<_>>(),
            expected
        );
    }

    /// Real certified blocks: a 2-of-3 proposal made at 4 s and executed by a second signer at
    /// 6 s, with an unrelated block between them.
    #[test]
    fn committed_blocks_give_each_signer_its_block_time_and_height() {
        use iroha_core::state::WorldReadOnly;
        use iroha_data_model::domain::Domain;
        use iroha_executor_data_model::isi::multisig::{MultisigRegister, MultisigSpec};
        use iroha_model_base::domain::DomainId;
        use std::num::{NonZeroU16, NonZeroU64};

        let keys = [0x81_u8, 0x82, 0x83]
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
        let ids = keys
            .each_ref()
            .map(|key| AccountId::new(key.public_key().clone()));
        let domain = DomainId::try_new("approval_landings", "universal").unwrap();
        let mut config = TestChainConfig::new(iroha_core::state::World::new(), 1_000);
        let genesis_key = config.genesis_key.clone();
        config.genesis_instructions = vec![Register::domain(Domain::new(domain.clone())).into()];
        config.genesis_instructions.extend(
            ids.iter()
                .map(|id| Register::account(Account::new(id.clone())).into()),
        );
        let mut chain = CertifiedTestChain::start(config).unwrap();
        let spec = MultisigSpec::new(
            ids.iter().map(|id| (id.clone(), 1)).collect(),
            NonZeroU16::new(2).unwrap(),
            NonZeroU64::new(86_400_000).unwrap(),
        );
        let register = chain.sign(
            &genesis_key,
            [MultisigRegister::with_account(account(0x84), domain, spec).into()],
            2_000,
        );
        assert_eq!(chain.commit_at(2_000, vec![register]), [true]);
        let multisig = chain
            .state()
            .view()
            .world()
            .accounts_iter()
            .find(|account| account.id().multisig_policy().is_some())
            .unwrap()
            .id()
            .clone();
        let instructions = vec![InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "reviewed".into(),
        ))];
        let id = MultisigProposalId::derive(&chain.network_id(), &multisig, &nonce(0x41));
        let propose = chain.sign(
            &keys[0],
            [
                MultisigPropose::new(multisig.clone(), nonce(0x41), instructions.clone(), None)
                    .into(),
            ],
            4_000,
        );
        assert_eq!(chain.commit_at(4_000, vec![propose]), [true]);
        let proposed_height = chain.height();
        let unrelated = chain.sign(
            &keys[2],
            [Log::new(iroha_data_model::Level::INFO, "unrelated".into()).into()],
            5_000,
        );
        assert_eq!(chain.commit_at(5_000, vec![unrelated]), [true]);
        let approve = chain.sign(
            &keys[1],
            [MultisigApprove::new(multisig.clone(), id, HashOf::new(&instructions)).into()],
            6_000,
        );
        assert_eq!(chain.commit_at(6_000, vec![approve]), [true]);
        let approved_height = chain.height();

        let landings = with_empty_index(|| fold_to_tip(&app_reading(&chain), true));
        assert_eq!(landings.height, chain.height() as usize);
        let at = |height: u64| {
            u64::try_from(
                chain
                    .committed(height)
                    .block()
                    .header()
                    .creation_time()
                    .as_millis(),
            )
            .unwrap()
        };
        assert_eq!(
            landings.first.get(&(id, ids[0].clone())),
            Some(&Landing {
                at_ms: at(proposed_height),
                height: proposed_height,
            })
        );
        assert_eq!(
            landings.first.get(&(id, ids[1].clone())),
            Some(&Landing {
                at_ms: at(approved_height),
                height: approved_height,
            })
        );
        assert!(
            !landings.first.contains_key(&(id, ids[2].clone())),
            "an unrelated transaction is not an approval"
        );
    }

    /// A read folds at most a short tail itself; the next read goes on from there.
    #[test]
    fn a_read_folds_at_most_a_short_tail() {
        let key = KeyPair::from_seed(vec![0x91; 32], Algorithm::Ed25519);
        let signer = AccountId::new(key.public_key().clone());
        let mut config = TestChainConfig::new(iroha_core::state::World::new(), 1_000);
        config.genesis_instructions = vec![Register::account(Account::new(signer)).into()];
        let mut chain = CertifiedTestChain::start(config).unwrap();
        for block in 0..REQUEST_TAIL_BLOCKS + 4 {
            let at = 2_000 + 1_000 * u64::try_from(block).unwrap();
            let tick = chain.sign(
                &key,
                [Log::new(iroha_data_model::Level::INFO, "tick".into()).into()],
                at,
            );
            assert_eq!(chain.commit_at(at, vec![tick]), [true]);
        }
        let app = app_reading(&chain);
        with_empty_index(|| {
            assert_eq!(fold_to_tip(&app, false).height, REQUEST_TAIL_BLOCKS);
            assert_eq!(
                fold_to_tip(&app, false).height,
                chain.height() as usize,
                "the next read folds the rest"
            );
        });
    }

    #[test]
    fn landings_stop_growing_at_their_memory_bound() {
        let multisig = account(1);
        let signer = account(7);
        let occurrence = |index: usize| {
            let mut bytes = [0xff_u8; 32];
            bytes[..8].copy_from_slice(&index.to_be_bytes());
            MultisigProposalId::derive(
                &network(),
                &multisig,
                &MultisigProposalNonce::new(bytes).unwrap(),
            )
        };
        let mut landings = MultisigApprovalLandings::default();
        landings.fold(
            1,
            block_hash(1),
            1,
            (0..MAX_LANDINGS).map(|index| (occurrence(index), signer.clone())),
        );
        assert_eq!(landings.first.len(), MAX_LANDINGS);
        landings.fold(2, block_hash(2), 2, [(occurrence(MAX_LANDINGS), signer)]);
        assert_eq!(landings.first.len(), MAX_LANDINGS);
        assert_eq!(landings.height, 2, "the folded height still advances");
    }
}
