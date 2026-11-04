//! Bounded ordinary-transaction driver for the native SCCP anchor prefix.

use std::{
    collections::{BTreeSet, VecDeque},
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha::{
    blocking::Client,
    client::{
        AccountTransactionDraft, FeeQuoteRequest, TransactionDispatchOutcomeUnknownError,
        TransactionWaitOptions,
    },
    data_model::prelude::*,
};
use iroha_model_base::{metadata::Metadata, name::Name};
use iroha_test_samples::ALICE_ID;

/// Maximum number of accepted ordinary transactions awaiting exact finality.
pub(super) const WINDOW: usize = 8;
/// Original metadata key used by the native anchor fixture.
pub(super) const METADATA_KEY: &str = "sccp_anchor_sequence";

/// One exact signed transaction and its authoritative native application height.
#[derive(Clone, Debug)]
pub(super) struct Receipt {
    /// Sequence authored in the original metadata instruction.
    pub sequence: u64,
    /// Locally computed signed transaction identity confirmed by the SDK.
    pub transaction_hash: HashOf<SignedTransaction>,
    /// Height returned by authoritative global Applied status.
    pub observed_height: u64,
}

/// Bounded progress passed to the original all-peer observation callback.
#[derive(Clone, Copy, Debug)]
pub(super) struct Progress {
    /// Highest confirmed application height, allowing out-of-order execution.
    pub height: u64,
    /// Number of uniquely signed transactions admitted so far.
    pub submitted: u64,
    /// Accepted transactions still awaiting exact finality.
    pub pending: usize,
}

struct Pending {
    sequence: u64,
    hash: HashOf<SignedTransaction>,
    deadline: Instant,
    ambiguity: Option<eyre::Report>,
}

struct Prefix {
    start: u64,
    target: u64,
    height: u64,
    window: usize,
    submitted: u64,
    pending: VecDeque<Pending>,
    hashes: BTreeSet<HashOf<SignedTransaction>>,
    heights: BTreeSet<u64>,
    receipts: Vec<Receipt>,
}

impl Prefix {
    fn new(start: u64, target: u64, window: usize) -> Result<Self> {
        ensure!(
            window > 0 && window <= WINDOW,
            "invalid anchor pending bound"
        );
        Ok(Self {
            start,
            target,
            height: start,
            window,
            submitted: 0,
            pending: VecDeque::new(),
            hashes: BTreeSet::new(),
            heights: BTreeSet::new(),
            receipts: Vec::new(),
        })
    }

    fn next_sequence(&self) -> Option<u64> {
        let uncovered = self.target.saturating_sub(self.height);
        (self.pending.len() < self.window
            && u64::try_from(self.pending.len()).expect("bounded pending count") < uncovered)
            .then(|| self.submitted.checked_add(1))
            .flatten()
    }

    fn admit(&mut self, pending: Pending) -> Result<()> {
        ensure!(
            self.next_sequence() == Some(pending.sequence),
            "anchor admission exceeded its window or sequence"
        );
        ensure!(
            self.hashes.insert(pending.hash),
            "duplicate anchor transaction"
        );
        self.submitted = pending.sequence;
        self.pending.push_back(pending);
        Ok(())
    }

    fn applied(&mut self, hash: HashOf<SignedTransaction>, height: Option<u64>) -> Result<()> {
        let pending = self
            .pending
            .front()
            .ok_or_else(|| eyre!("no pending anchor transaction"))?;
        ensure!(
            hash == pending.hash,
            "anchor confirmation changed transaction identity"
        );
        let height =
            height.ok_or_else(|| eyre!("Applied anchor transaction lacks native height"))?;
        ensure!(
            height > self.start,
            "anchor transaction predates its prefix"
        );
        ensure!(
            !self.heights.contains(&height),
            "two anchor transactions share one native block"
        );
        let pending = self
            .pending
            .pop_front()
            .expect("checked pending transaction");
        self.heights.insert(height);
        self.height = self.height.max(height);
        self.receipts.push(Receipt {
            sequence: pending.sequence,
            transaction_hash: hash,
            observed_height: height,
        });
        Ok(())
    }

    fn complete(&self) -> bool {
        self.height >= self.target && self.pending.is_empty()
    }

    fn progress(&self) -> Progress {
        Progress {
            height: self.height,
            submitted: self.submitted,
            pending: self.pending.len(),
        }
    }
}

fn remaining_budget(deadline: Instant, now: Instant) -> Result<Duration> {
    let remaining = deadline.saturating_duration_since(now);
    ensure!(
        !remaining.is_zero(),
        "anchor confirmation exhausted its original deadline"
    );
    Ok(remaining)
}

fn validate_ambiguous_identity(
    transaction: &SignedTransaction,
    signed_hash: &HashOf<SignedTransaction>,
    entrypoint_hash: &HashOf<TransactionEntrypoint>,
) -> Result<()> {
    ensure!(
        *signed_hash == transaction.hash() && *entrypoint_hash == transaction.hash_as_entrypoint(),
        "ambiguous anchor dispatch differs from the exact signed transaction"
    );
    Ok(())
}

fn admission(
    transaction: &SignedTransaction,
    result: Result<HashOf<SignedTransaction>>,
) -> Result<Option<eyre::Report>> {
    match result {
        Ok(hash) => {
            ensure!(
                hash == transaction.hash(),
                "anchor admission changed transaction hash"
            );
            Ok(None)
        }
        Err(error) => {
            let Some(unknown) = error
                .chain()
                .find_map(|cause| cause.downcast_ref::<TransactionDispatchOutcomeUnknownError>())
            else {
                return Err(error);
            };
            validate_ambiguous_identity(
                transaction,
                unknown.signed_transaction_hash(),
                unknown.entrypoint_hash(),
            )?;
            Ok(Some(error))
        }
    }
}

/// Drive real signed metadata blocks while retaining every exact finality observation.
///
/// The caller must author `MaxTransactions(1)` in signed genesis. Accepted work is
/// fully drained before returning successfully, including when a real heartbeat
/// crosses the target. A failure aborts the fixture without publishing evidence.
pub(super) fn drive(
    client: &Client,
    start: u64,
    target: u64,
    window: usize,
    mut observe: impl FnMut(Progress) -> Result<()>,
) -> Result<Vec<Receipt>> {
    let mut prefix = Prefix::new(start, target, window)?;
    let confirmation_budget = client.client().transaction_status_timeout();
    ensure!(
        !confirmation_budget.is_zero(),
        "anchor fixture needs a finite confirmation budget"
    );
    let key: Name = METADATA_KEY.parse()?;
    while !prefix.complete() {
        while let Some(sequence) = prefix.next_sequence() {
            let account = client.account_client();
            let mut payload = account.prepare_transaction(AccountTransactionDraft::new(
                [SetKeyValue::account(
                    ALICE_ID.clone(),
                    key.clone(),
                    Json::new(sequence),
                )],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))?;
            let quote =
                client.quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })?;
            ensure!(
                payload
                    .fee_payment
                    .has_same_payer_and_gas_bound(&quote.intent),
                "anchor fee quote changed payer or gas bound"
            );
            payload.fee_payment = quote.intent;
            let transaction = account.sign_transaction(payload)?;
            let ambiguity = admission(&transaction, client.submit_transaction(&transaction))?;
            let deadline = Instant::now()
                .checked_add(confirmation_budget)
                .ok_or_else(|| eyre!("anchor confirmation deadline overflow"))?;
            prefix.admit(Pending {
                sequence,
                hash: transaction.hash(),
                deadline,
                ambiguity,
            })?;
        }
        let pending = prefix
            .pending
            .front()
            .ok_or_else(|| eyre!("anchor prefix made no progress"))?;
        let remaining = remaining_budget(pending.deadline, Instant::now())?;
        let outcome = client
            .wait_for_transaction_applied(
                pending.hash,
                TransactionWaitOptions {
                    timeout: remaining,
                    // Retain the existing 600-second fixture wait's automatic poll cadence.
                    poll_interval: Duration::from_secs(2).min(remaining),
                },
            )
            .wrap_err_with(|| match &pending.ambiguity {
                Some(error) => format!(
                    "anchor transaction {} did not settle; original ambiguous dispatch: {error:#}",
                    pending.hash
                ),
                None => format!("anchor transaction {} did not settle", pending.hash),
            })?;
        // The existing SDK waiter owns canonical hash/scope/state/Applied validation.
        prefix.applied(pending.hash, outcome.block_height)?;
        observe(prefix.progress())?;
    }
    Ok(prefix.receipts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;

    fn hash(sequence: u64) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::new(sequence.to_le_bytes()))
    }

    fn pending(sequence: u64, deadline: Instant) -> Pending {
        Pending {
            sequence,
            hash: hash(sequence),
            deadline,
            ambiguity: None,
        }
    }

    #[test]
    fn bounded_window_fills_ahead_and_covers_the_complete_anchor_prefix() {
        let mut prefix = Prefix::new(4, 4097, WINDOW).unwrap();
        let deadline = Instant::now() + Duration::from_secs(600);
        let mut peak = 0;
        while !prefix.complete() {
            while let Some(sequence) = prefix.next_sequence() {
                prefix.admit(pending(sequence, deadline)).unwrap();
                peak = peak.max(prefix.pending.len());
            }
            let first = prefix.pending.front().unwrap();
            prefix
                .applied(first.hash, Some(first.sequence + 4))
                .unwrap();
        }
        assert_eq!(peak, WINDOW);
        assert_eq!(prefix.submitted, 4093);
        assert_eq!(prefix.receipts.len(), 4093);
        assert_eq!(prefix.heights, (5..=4097).collect());
    }

    #[test]
    fn partial_window_and_crossing_heartbeat_drain_every_admission() {
        assert!(Prefix::new(4097, 4097, WINDOW).unwrap().complete());
        let mut prefix = Prefix::new(4094, 4097, WINDOW).unwrap();
        let deadline = Instant::now() + Duration::from_secs(600);
        for sequence in 1..=3 {
            prefix.admit(pending(sequence, deadline)).unwrap();
        }
        assert!(prefix.next_sequence().is_none());
        // A real intervening block or out-of-order execution may cross the target.
        prefix.applied(hash(1), Some(4098)).unwrap();
        assert!(!prefix.complete());
        assert!(prefix.next_sequence().is_none());
        prefix.applied(hash(2), Some(4095)).unwrap();
        prefix.applied(hash(3), Some(4097)).unwrap();
        assert!(prefix.complete());
        assert_eq!(
            prefix
                .receipts
                .iter()
                .map(|r| r.observed_height)
                .collect::<Vec<_>>(),
            [4098, 4095, 4097]
        );
    }

    #[test]
    fn invalid_height_identity_or_admission_does_not_retire_pending_work() {
        assert!(Prefix::new(1, 9, 0).is_err());
        assert!(Prefix::new(1, 9, WINDOW + 1).is_err());
        let mut prefix = Prefix::new(1, 9, 1).unwrap();
        let deadline = Instant::now() + Duration::from_secs(600);
        prefix.admit(pending(1, deadline)).unwrap();
        assert!(prefix.admit(pending(2, deadline)).is_err());
        assert!(prefix.applied(hash(2), Some(2)).is_err());
        assert!(prefix.applied(hash(1), None).is_err());
        assert!(prefix.applied(hash(1), Some(1)).is_err());
        assert_eq!(prefix.pending.len(), 1);
        prefix.applied(hash(1), Some(2)).unwrap();
        let mut duplicate = pending(2, deadline);
        duplicate.hash = hash(1);
        assert!(prefix.admit(duplicate).is_err());
        prefix.admit(pending(2, deadline)).unwrap();
        assert!(prefix.applied(hash(2), Some(2)).is_err());
        assert_eq!(prefix.pending.len(), 1);
    }

    #[test]
    fn queued_confirmation_keeps_its_original_deadline() {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(600);
        assert_eq!(
            remaining_budget(deadline, started + Duration::from_secs(23)).unwrap(),
            Duration::from_secs(577)
        );
        assert!(remaining_budget(deadline, deadline).is_err());
        assert!(remaining_budget(deadline, deadline + Duration::from_secs(1)).is_err());
    }

    #[test]
    fn ambiguity_requires_both_exact_identities_and_a_typed_dispatch_error() {
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"anchor admission fixture"),
        ));
        let transaction = TransactionBuilder::new(
            network,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([SetKeyValue::account(
            ALICE_ID.clone(),
            METADATA_KEY.parse().unwrap(),
            Json::new(1_u64),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        validate_ambiguous_identity(
            &transaction,
            &transaction.hash(),
            &transaction.hash_as_entrypoint(),
        )
        .unwrap();
        assert!(
            validate_ambiguous_identity(&transaction, &hash(99), &transaction.hash_as_entrypoint())
                .is_err()
        );
        let other_entrypoint = HashOf::from_untyped_unchecked(Hash::new(b"another entrypoint"));
        assert!(
            validate_ambiguous_identity(&transaction, &transaction.hash(), &other_entrypoint)
                .is_err()
        );
        assert!(
            admission(&transaction, Ok(transaction.hash()))
                .unwrap()
                .is_none()
        );
        assert!(admission(&transaction, Ok(hash(99))).is_err());
        assert!(admission(&transaction, Err(eyre!(
            "Transaction dispatch remains ambiguous; signed_transaction_hash={}; entrypoint_hash={}",
            transaction.hash(), transaction.hash_as_entrypoint(),
        ))).is_err(), "display text cannot authorize ambiguous-dispatch reconciliation");
    }
}
