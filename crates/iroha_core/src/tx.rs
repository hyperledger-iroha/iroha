//! `Transaction`-related functionality of Iroha.
//!
//! Types represent various stages of a `Transaction`'s lifecycle. For
//! example, `Transaction` is the start, when a transaction had been
//! received by Torii.
//!
//! This is also where the actual execution of instructions, as well
//! as various forms of validation are performed.

use std::time::{Duration, SystemTime};

use eyre::Result;
pub use iroha_data_model::prelude::*;
use iroha_data_model::{
    isi::error::Mismatch,
    query::error::FindError,
    transaction::{error::TransactionLimitError, TransactionPayload},
};
use iroha_logger::{debug, error};
use iroha_macro::FromVariant;
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::{wasm, wasm::cache::WasmCache},
    state::{StateBlock, StateTransaction},
};

/// `AcceptedTransaction` — a transaction accepted by Iroha peer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct AcceptedTransaction(SignedTransaction);

/// Verification failed of some signature due to following reason
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureVerificationFail {
    /// Signature which verification has failed
    pub signature: SignatureOf<TransactionPayload>,
    /// Error which happened during verification
    pub reason: String,
}

impl core::fmt::Display for SignatureVerificationFail {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Failed to verify signatures: {}", self.reason,)
    }
}

impl std::error::Error for SignatureVerificationFail {}

/// Error type for transaction from [`SignedTransaction`] to [`AcceptedTransaction`]
#[derive(Debug, displaydoc::Display, PartialEq, Eq, FromVariant, thiserror::Error)]
pub enum AcceptTransactionFail {
    /// Failure during limits check
    TransactionLimit(#[source] TransactionLimitError),
    /// Failure during signature verification
    SignatureVerification(#[source] SignatureVerificationFail),
    /// The genesis account can serve as the transaction authority only for genesis transactions
    UnexpectedGenesisAuthority,
    /// Chain id doesn't correspond to the id of current blockchain: {0}
    ChainIdMismatch(Mismatch<ChainId>),
    /// Transaction creation time is in the future
    TransactionInTheFuture,
}

impl AcceptedTransaction {
    fn validate_common(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        let actual_chain_id = tx.chain();

        if expected_chain_id != actual_chain_id {
            return Err(AcceptTransactionFail::ChainIdMismatch(Mismatch {
                expected: expected_chain_id.clone(),
                actual: actual_chain_id.clone(),
            }));
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        if tx.creation_time().saturating_sub(now) > max_clock_drift {
            return Err(AcceptTransactionFail::TransactionInTheFuture);
        }

        Ok(())
    }

    /// Like [`Self::accept_genesis`], but without wrapping.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn validate_genesis(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
    ) -> Result<(), AcceptTransactionFail> {
        Self::validate_common(tx, expected_chain_id, max_clock_drift)?;

        if *iroha_genesis::GENESIS_ACCOUNT_ID != *tx.authority() {
            return Err(AcceptTransactionFail::UnexpectedGenesisAuthority);
        }

        Ok(())
    }

    /// Like [`Self::accept`], but without wrapping.
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn validate(
        tx: &SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
    ) -> Result<(), AcceptTransactionFail> {
        Self::validate_common(tx, expected_chain_id, max_clock_drift)?;

        if *iroha_genesis::GENESIS_DOMAIN_ID == *tx.authority().domain() {
            return Err(AcceptTransactionFail::UnexpectedGenesisAuthority);
        }

        if let Err(err) = tx.verify_signature() {
            return Err(AcceptTransactionFail::SignatureVerification(
                SignatureVerificationFail {
                    signature: tx.signature().0.clone(),
                    reason: err.to_string(),
                },
            ));
        }

        match &tx.instructions() {
            Executable::Instructions(instructions) => {
                if instructions.is_empty() {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: "Transaction must contain at least one instruction".into(),
                        },
                    ));
                }

                let instruction_limit = limits
                    .max_instructions
                    .get()
                    .try_into()
                    .expect("INTERNAL BUG: max instructions exceeds usize::MAX");

                if instructions.len() > instruction_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "Too many instructions in payload, max number is {}, but got {}",
                                limits.max_instructions,
                                instructions.len()
                            ),
                        },
                    ));
                }
            }
            // TODO: Can we check the number of instructions in wasm? Because we do this check
            // when executing wasm where we deny wasm if number of instructions exceeds the limit.
            //
            // Should we allow infinite instructions in wasm? And deny only based on fuel and size
            Executable::Wasm(smart_contract) => {
                let smart_contract_size_limit = limits
                    .smart_contract_size
                    .get()
                    .try_into()
                    .expect("INTERNAL BUG: smart contract size exceeds usize::MAX");

                if smart_contract.size_bytes() > smart_contract_size_limit {
                    return Err(AcceptTransactionFail::TransactionLimit(
                        TransactionLimitError {
                            reason: format!(
                                "WASM binary size is too large: max {}, got {} \
                                (configured by \"Parameter::SmartContractLimits\")",
                                limits.smart_contract_size,
                                smart_contract.size_bytes()
                            ),
                        },
                    ));
                }
            }
        }

        Ok(())
    }

    /// Accept genesis transaction. Transition from [`SignedTransaction`] to [`AcceptedTransaction`].
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn accept_genesis(
        tx: SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate_genesis(&tx, expected_chain_id, max_clock_drift).map(|()| Self(tx))
    }

    /// Accept transaction. Transition from [`SignedTransaction`] to [`AcceptedTransaction`].
    ///
    /// # Errors
    ///
    /// See [`AcceptTransactionFail`]
    pub fn accept(
        tx: SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate(&tx, expected_chain_id, max_clock_drift, limits).map(|()| Self(tx))
    }

    /// Assume the transaction is acceptable.
    pub fn new_unchecked(tx: SignedTransaction) -> Self {
        Self(tx)
    }
}

impl From<AcceptedTransaction> for SignedTransaction {
    fn from(source: AcceptedTransaction) -> Self {
        source.0
    }
}

impl From<AcceptedTransaction> for (AccountId, Executable) {
    fn from(source: AcceptedTransaction) -> Self {
        source.0.into()
    }
}

impl AsRef<SignedTransaction> for AcceptedTransaction {
    fn as_ref(&self) -> &SignedTransaction {
        &self.0
    }
}

impl StateBlock<'_> {
    /// Validate and apply the transaction to the state if validation succeeds; leave the state unchanged on failure.
    ///
    /// Returns the hash and the result of the transaction -- the trigger sequence on success, or the rejection reason on failure.
    pub fn validate_transaction(
        &mut self,
        tx: AcceptedTransaction,
        wasm_cache: &mut WasmCache<'_, '_, '_>,
    ) -> (HashOf<TransactionEntrypoint>, TransactionResultInner) {
        let mut state_transaction = self.transaction();
        let hash = tx.as_ref().hash_as_entrypoint();
        let result = Self::validate_transaction_internal(tx, &mut state_transaction, wasm_cache);
        if result.is_ok() {
            state_transaction.apply();
        }

        (hash, result)
    }

    /// Validate the transaction, staging its state changes.
    ///
    /// Returns the trigger sequence on success, or the rejection reason on failure.
    fn validate_transaction_internal(
        tx: AcceptedTransaction,
        state_transaction: &mut StateTransaction<'_, '_>,
        wasm_cache: &mut WasmCache<'_, '_, '_>,
    ) -> TransactionResultInner {
        let authority = tx.as_ref().authority().clone();

        if state_transaction.world.accounts.get(&authority).is_none() {
            return Err(TransactionRejectionReason::AccountDoesNotExist(
                FindError::Account(authority.clone()),
            ));
        }

        debug!(tx=%tx.as_ref().hash(), "Validating transaction");
        Self::validate_transaction_with_runtime_executor(
            tx.clone(),
            state_transaction,
            wasm_cache,
        )?;

        if let (authority, Executable::Wasm(bytes)) = tx.into() {
            Self::validate_wasm(authority, state_transaction, bytes)?
        }

        debug!("Transaction validated successfully; processing data triggers");
        let trigger_sequence = state_transaction.execute_data_triggers_dfs(&authority)?;
        debug!("Data triggers executed successfully");

        Ok(trigger_sequence)
    }

    fn validate_wasm(
        authority: AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
        wasm: WasmSmartContract,
    ) -> Result<(), TransactionRejectionReason> {
        debug!("Validating wasm");

        wasm::RuntimeBuilder::<wasm::state::SmartContract>::new()
            .build()
            .and_then(|mut wasm_runtime| {
                wasm_runtime.validate(
                    state_transaction,
                    authority,
                    wasm,
                    state_transaction
                        .world
                        .parameters
                        .transaction
                        .max_instructions,
                )
            })
            .map_err(|error| WasmExecutionFail {
                reason: format!("{:?}", eyre::Report::from(error)),
            })
            .map_err(TransactionRejectionReason::WasmExecution)
    }

    /// Validate transaction with runtime executors.
    ///
    /// Note: transaction instructions will be executed on the given `state_transaction`.
    fn validate_transaction_with_runtime_executor(
        tx: AcceptedTransaction,
        state_transaction: &mut StateTransaction<'_, '_>,
        wasm_cache: &mut WasmCache<'_, '_, '_>,
    ) -> Result<(), TransactionRejectionReason> {
        let tx: SignedTransaction = tx.into();
        let authority = tx.authority().clone();

        state_transaction
            .world
            .executor
            .clone() // Cloning executor is a cheap operation
            .execute_transaction(state_transaction, &authority, tx, wasm_cache)
            .map_err(|error| {
                if let ValidationFail::InternalError(msg) = &error {
                    error!(
                        error = msg,
                        "Internal error occurred during transaction validation, \
                         is Runtime Executor correct?"
                    )
                }
                error.into()
            })
    }
}

#[cfg(test)]
#[allow(missing_docs)]
pub mod tests {
    use core::panic;
    use std::sync::LazyLock;

    use iroha_data_model::{block::SignedBlock, isi::Instruction, prelude::EventBox};
    use iroha_genesis::GENESIS_ACCOUNT_ID;
    use iroha_test_samples::PEER_KEYPAIR;

    use super::*;
    use crate::{
        block::{BlockBuilder, CommittedBlock, ValidBlock},
        smartcontracts::isi::Registrable,
        state::{State, StateBlock, StateReadOnly, World},
    };

    mod time_trigger {
        use super::*;

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends a large donation to Bob.
        /// 2. Data trigger: Bob forwards the donation to Carol.
        /// 3. Time trigger: Carol attempts to send the donation to Dave; this should fail if step 2 did not occur.
        /// 4. Data trigger: Dave forwards the donation to Eve.
        #[tokio::test]
        async fn fires_after_external_transactions() {
            let mut sandbox = Sandbox::default()
                .with_data_trigger_transfer("bob", 50, "carol")
                .with_time_trigger_transfer("carol", 50, "dave")
                .with_data_trigger_transfer("dave", 50, "eve");
            sandbox.request_transfer("alice", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "time_trigger/fires_after_external_transactions");
            block.assert_balances([
                ("alice", 10),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 60),
            ]);
        }
    }

    mod data_trigger {
        use super::*;

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends a large donation to Bob.
        /// 2. Data trigger: Bob forwards the donation to Carol.
        /// 3. Transaction: Carol attempts to send the donation to Dave; this should fail if step 2 did not occur.
        #[tokio::test]
        async fn fires_for_each_transaction() {
            let mut sandbox = Sandbox::default().with_data_trigger_transfer("bob", 50, "carol");
            sandbox.request_transfer("alice", 50, "bob");
            sandbox.request_transfer("carol", 50, "dave");
            let mut block = sandbox.block();
            block.assert_balances([("alice", 60), ("bob", 10), ("carol", 10), ("dave", 10)]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/fires_for_each_transaction");
            block.assert_balances([("alice", 10), ("bob", 10), ("carol", 10), ("dave", 60)]);
        }

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends the asset to Bob in two separate packages, emitting two events.
        /// 2. Data trigger: Bob forwards exactly one package to Carol; this trigger fires only once.
        #[tokio::test]
        async fn fires_at_most_once_per_step() {
            let mut sandbox = Sandbox::default().with_data_trigger_transfer("bob", 10, "carol");
            sandbox.request_transfers_batched::<2>("alice", 10, "bob");
            let mut block = sandbox.block();
            block.assert_balances([("alice", 60), ("bob", 10), ("carol", 10)]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/fires_at_most_once_per_step");
            block.assert_balances([("alice", 40), ("bob", 20), ("carol", 20)]);
        }

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends a large donation to Bob.
        /// 2. Data triggers: Bob forwards the donation to Carol, Carol forwards it to Dave, and Dave forwards it back to Bob.
        /// 3. Data trigger: Bob forwards the donation to Eve; this should fail if step 2 has not completed.
        #[tokio::test]
        async fn chains_in_depth_first_order() {
            let mut sandbox = Sandbox::default()
                // Carol receives it before Eve because triggers matching the same event are processed in lexicographical order of their IDs.
                .with_data_trigger_transfer_once("bob", 50, "carol")
                // Sibling trigger waits for depth-first resolution.
                .with_data_trigger_transfer_once("bob", 50, "eve")
                .with_data_trigger_transfer("carol", 50, "dave")
                .with_data_trigger_transfer("dave", 50, "bob");
            sandbox.request_transfer("alice", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/chains_in_depth_first_order");
            block.assert_balances([
                ("alice", 10),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 60),
            ]);
        }

        /// # Scenario
        ///
        /// 1. Transaction: Alice sends 50 units to Bob.
        /// 2. Data triggers: each branch (Bob -> Carol -> Dave -> Eve) runs independently to a max depth of 3, forwarding 1 unit per step.
        #[tokio::test]
        async fn each_branch_is_assigned_depth() {
            let mut sandbox = Sandbox::default()
                .with_max_execution_depth(3)
                // Branches: Bob -> Carol
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 0)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 1)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 2)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 3)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 4)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 5)
                .with_data_trigger_transfer_labeled("bob", 1, "carol", 6)
                // Common path: Carol -> Dave -> Eve
                .with_data_trigger_transfer("carol", 1, "dave")
                .with_data_trigger_transfer("dave", 1, "eve");
            sandbox.request_transfer("alice", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(&events, "data_trigger/each_branch_is_assigned_depth");
            block.assert_balances([
                ("alice", 10),
                ("bob", 53),
                ("carol", 10),
                ("dave", 10),
                ("eve", 17),
            ]);
        }

        /// All or none of the initial transaction and subsequent data triggers should take effect.
        #[tokio::test]
        async fn atomically_chains_from_transaction() {
            let sandbox = || {
                let mut res = Sandbox::default();
                res.request_transfer("alice", 50, "bob");
                res
            };

            aborts_on_execution_error(sandbox(), "txn");
            aborts_on_exceeding_depth(sandbox(), "txn");
            commits_on_depleting_lives(sandbox(), "txn");
            commits_on_regular_success(sandbox(), "txn");
        }

        /// All or none of the initial time trigger and subsequent data triggers should take effect.
        #[tokio::test]
        async fn atomically_chains_from_time_trigger() {
            let sandbox = || Sandbox::default().with_time_trigger_transfer("alice", 50, "bob");

            aborts_on_execution_error(sandbox(), "time");
            aborts_on_exceeding_depth(sandbox(), "time");
            commits_on_depleting_lives(sandbox(), "time");
            commits_on_regular_success(sandbox(), "time");
        }

        fn aborts_on_execution_error(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_data_trigger_transfer("bob", 10, "carol")
                .with_data_trigger_transfer("bob", 10, "dave")
                // This trigger execution fails.
                .with_data_trigger_transfer("dave", 500, "eve");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(
                &events,
                format!("data_trigger/aborts_on_execution_error-{snapshot_suffix}"),
            );
            // Everything should be rolled back.
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
        }

        fn aborts_on_exceeding_depth(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_max_execution_depth(2)
                .with_data_trigger_transfer("bob", 50, "carol")
                .with_data_trigger_transfer("carol", 50, "dave")
                // The execution sequence exceeds the depth limit.
                .with_data_trigger_transfer("dave", 50, "eve");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(
                &events,
                format!("data_trigger/aborts_on_exceeding_depth-{snapshot_suffix}"),
            );
            // Everything should be rolled back.
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
        }

        fn commits_on_depleting_lives(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_data_trigger_transfer("bob", 50, "carol")
                // This trigger depletes after an execution.
                .with_data_trigger_transfer_once("carol", 50, "bob");
            let mut block = sandbox.block();
            block.assert_balances([("alice", 60), ("bob", 10), ("carol", 10)]);
            let (events, _committed_block) = block.apply();
            assert_events(
                &events,
                format!("data_trigger/commits_on_depleting_lives-{snapshot_suffix}"),
            );
            // The execution sequence should take effect.
            block.assert_balances([("alice", 10), ("bob", 10), ("carol", 60)]);
        }

        fn commits_on_regular_success(sandbox: Sandbox, snapshot_suffix: &str) {
            let mut sandbox = sandbox
                .with_max_execution_depth(3)
                .with_data_trigger_transfer("bob", 50, "carol")
                .with_data_trigger_transfer("carol", 50, "dave")
                .with_data_trigger_transfer("dave", 50, "eve");
            let mut block = sandbox.block();
            block.assert_balances([
                ("alice", 60),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 10),
            ]);
            let (events, _committed_block) = block.apply();
            assert_events(
                &events,
                format!("data_trigger/commits_on_regular_success-{snapshot_suffix}"),
            );
            // The execution sequence should take effect.
            block.assert_balances([
                ("alice", 10),
                ("bob", 10),
                ("carol", 10),
                ("dave", 10),
                ("eve", 60),
            ]);
        }
    }

    pub struct Sandbox {
        pub state: State,
        // Buffered transactions
        pub transactions: Vec<SignedTransaction>,
    }

    pub struct SandboxBlock<'state> {
        pub state: StateBlock<'state>,
        // Candidate to be validated and committed
        pub block: Option<SignedBlock>,
    }

    pub const ACCOUNTS_STR: [&str; 5] = ["alice", "bob", "carol", "dave", "eve"];
    pub static INIT_BALANCE: LazyLock<AccountBalance> =
        LazyLock::new(|| ACCOUNTS_STR.into_iter().zip([60, 10, 10, 10, 10]).collect());
    pub const INIT_EXECUTION_DEPTH: u8 = u8::MAX;

    pub type AccountBalance = std::collections::BTreeMap<&'static str, u32>;
    pub type AccountMap = std::collections::BTreeMap<&'static str, Credential>;

    pub const DOMAIN_STR: &str = "wonderland";
    pub const ASSET_STR: &str = "rose";
    pub static DOMAIN: LazyLock<DomainId> = LazyLock::new(|| DOMAIN_STR.parse().unwrap());
    pub static ASSET: LazyLock<AssetDefinitionId> =
        LazyLock::new(|| format!("{ASSET_STR}#{DOMAIN_STR}").parse().unwrap());
    pub static ACCOUNT: LazyLock<AccountMap> = LazyLock::new(|| {
        ACCOUNTS_STR
            .iter()
            .map(|name| {
                let key_pair = iroha_crypto::KeyPair::from_seed(
                    name.as_bytes().into(),
                    iroha_crypto::Algorithm::Ed25519,
                )
                .into_parts();
                let credential = Credential {
                    id: format!("{}@{DOMAIN_STR}", key_pair.0).parse().unwrap(),
                    key: key_pair.1,
                };
                (*name, credential)
            })
            .collect()
    });

    #[derive(Debug, Clone)]
    pub struct Credential {
        pub id: AccountId,
        pub key: iroha_crypto::PrivateKey,
    }

    pub static CHAIN_ID: LazyLock<ChainId> =
        LazyLock::new(|| ChainId::from("00000000-0000-0000-0000-000000000000"));

    pub fn asset(account_name: &str) -> AssetId {
        AssetId::new(ASSET.clone(), ACCOUNT[account_name].id.clone())
    }

    pub fn transfer<'a>(
        src: &'a str,
        quantity: u32,
        dest: &'a str,
    ) -> impl IntoIterator<Item = impl Instruction> + 'a {
        transfers_batched::<1>(src, quantity, dest)
    }

    pub fn transfers_batched<'a, const N_INSTRUCTIONS: usize>(
        src: &'a str,
        quantity_per_instruction: u32,
        dest: &'a str,
    ) -> impl IntoIterator<Item = impl Instruction> + 'a {
        (0..N_INSTRUCTIONS).map(move |_| {
            Transfer::asset_numeric(
                asset(src),
                quantity_per_instruction,
                ACCOUNT[dest].id.clone(),
            )
        })
    }

    pub fn assert_events(actual: &[EventBox], snapshot_path: impl AsRef<std::path::Path>) {
        let expected = {
            let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(snapshot_path.as_ref());
            path.set_extension("json");
            expect_test::expect_file![path]
        };
        let actual = actual
            .iter()
            .filter(|e| !matches!(e, EventBox::Time(_) | EventBox::Pipeline(_)))
            .collect::<Vec<_>>();
        expected.assert_eq(&serde_json::to_string_pretty(&actual).unwrap());
    }

    impl Default for Sandbox {
        fn default() -> Self {
            let world = {
                let domain = Domain::new(DOMAIN.clone()).build(&GENESIS_ACCOUNT_ID);
                let asset_def = AssetDefinition::new(ASSET.clone(), NumericSpec::default())
                    .build(&GENESIS_ACCOUNT_ID);
                let accounts = ACCOUNT
                    .values()
                    .map(|cred| cred.id.clone())
                    .chain([GENESIS_ACCOUNT_ID.clone()])
                    .map(|id| Account::new(id).build(&GENESIS_ACCOUNT_ID));
                let assets = INIT_BALANCE
                    .iter()
                    .map(|(name, num)| Asset::new(asset(name), *num));

                World::with_assets([domain], accounts, [asset_def], assets, [])
            };
            let kura = crate::kura::Kura::blank_kura_for_testing();
            let query_handle = crate::query::store::LiveQueryStore::start_test();
            let state = State::new(world, kura, query_handle);

            Self {
                state,
                transactions: vec![],
            }
            .with_max_execution_depth(INIT_EXECUTION_DEPTH)
        }
    }

    impl Sandbox {
        #[must_use]
        pub fn with_time_trigger_transfer(self, src: &str, quantity: u32, dest: &str) -> Self {
            self.with_time_trigger_transfer_internal(src, quantity, dest, Repeats::Indefinitely, 0)
        }

        #[must_use]
        pub fn with_time_trigger_transfer_labeled(
            self,
            src: &str,
            quantity: u32,
            dest: &str,
            label: u32,
        ) -> Self {
            self.with_time_trigger_transfer_internal(
                src,
                quantity,
                dest,
                Repeats::Indefinitely,
                label,
            )
        }

        fn with_time_trigger_transfer_internal(
            self,
            src: &str,
            quantity: u32,
            dest: &str,
            repeats: Repeats,
            label: u32,
        ) -> Self {
            let mut block = self.state.world.triggers.block();
            let mut transaction = block.transaction();
            let trigger = Trigger::new(
                format!("time-{src}-{dest}-{label}").parse().unwrap(),
                Action::new(
                    transfer(src, quantity, dest),
                    repeats,
                    GENESIS_ACCOUNT_ID.clone(),
                    TimeEventFilter::new(ExecutionTime::PreCommit),
                ),
            )
            .try_into()
            .unwrap();

            transaction
                .add_time_trigger(&self.state.engine, trigger)
                .unwrap();
            transaction.apply();
            block.commit();
            self
        }

        #[must_use]
        pub fn with_data_trigger_transfer(self, src: &str, quantity: u32, dest: &str) -> Self {
            self.with_data_trigger_transfer_internal(src, quantity, dest, Repeats::Indefinitely, 0)
        }

        #[must_use]
        pub fn with_data_trigger_transfer_once(self, src: &str, quantity: u32, dest: &str) -> Self {
            self.with_data_trigger_transfer_internal(src, quantity, dest, Repeats::Exactly(1), 0)
        }

        #[must_use]
        pub fn with_data_trigger_transfer_labeled(
            self,
            src: &str,
            quantity: u32,
            dest: &str,
            label: u32,
        ) -> Self {
            self.with_data_trigger_transfer_internal(
                src,
                quantity,
                dest,
                Repeats::Indefinitely,
                label,
            )
        }

        fn with_data_trigger_transfer_internal(
            self,
            src: &str,
            quantity: u32,
            dest: &str,
            repeats: Repeats,
            label: u32,
        ) -> Self {
            let mut block = self.state.world.triggers.block();
            let mut transaction = block.transaction();
            let trigger = Trigger::new(
                format!("data-{src}-{dest}-{label}").parse().unwrap(),
                Action::new(
                    transfer(src, quantity, dest),
                    repeats,
                    GENESIS_ACCOUNT_ID.clone(),
                    AssetEventFilter::new()
                        .for_events(AssetEventSet::Added)
                        .for_asset(asset(src)),
                ),
            )
            .try_into()
            .unwrap();

            transaction
                .add_data_trigger(&self.state.engine, trigger)
                .unwrap();
            transaction.apply();
            block.commit();
            self
        }

        #[must_use]
        pub fn with_max_execution_depth(self, depth: u8) -> Self {
            let mut world = self.state.world.block();
            world.parameters.smart_contract.execution_depth = depth;
            world.commit();
            self
        }

        pub fn request_transfer(&mut self, src: &str, quantity: u32, dest: &str) {
            self.request_transfers_batched::<1>(src, quantity, dest);
        }

        pub fn request_transfers_batched<const N_INSTRUCTIONS: usize>(
            &mut self,
            src: &str,
            quantity_per_instruction: u32,
            dest: &str,
        ) {
            let transaction = {
                let instructions =
                    transfers_batched::<N_INSTRUCTIONS>(src, quantity_per_instruction, dest);
                TransactionBuilder::new(CHAIN_ID.clone(), GENESIS_ACCOUNT_ID.clone())
                    .with_instructions(instructions)
                    .genesis_sign()
            };
            self.transactions.push(transaction);
        }

        pub fn block(&mut self) -> SandboxBlock<'_> {
            let block: SignedBlock = {
                let transactions = {
                    let signed = core::mem::take(&mut self.transactions);
                    // Skip static analysis (AcceptedTransaction::accept)
                    signed.into_iter().map(AcceptedTransaction).collect()
                };
                BlockBuilder::new(transactions)
                    .chain(0, self.state.view().latest_block().as_deref())
                    .sign(PEER_KEYPAIR.private_key())
                    .unpack(|_| {})
                    .into()
            };

            SandboxBlock {
                state: self.state.block(block.header()),
                block: Some(block),
            }
        }
    }

    impl SandboxBlock<'_> {
        pub fn apply(&mut self) -> (Vec<EventBox>, CommittedBlock) {
            let valid = ValidBlock::validate_unchecked(
                core::mem::take(&mut self.block).unwrap(),
                &mut self.state,
            )
            .unpack(|_| {});
            let committed = valid.commit_unchecked().unpack(|_| {});
            let events = self.state.apply_without_execution(
                &committed,
                // topology in state is only used by sumeragi
                vec![],
            );

            (events, committed)
        }

        pub fn assert_balances(&self, expected: impl Into<AccountBalance>) {
            let expected = expected.into();
            let actual: AccountBalance = ACCOUNTS_STR
                .iter()
                .filter(|name| expected.contains_key(*name))
                .map(|name| {
                    let balance = self
                        .state
                        .world
                        .assets
                        .get(&asset(name))
                        .map_or_else(|| panic!("{name}'s asset not found"), |asset| asset.value)
                        .try_into()
                        .unwrap();
                    (*name, balance)
                })
                .collect();

            assert_eq!(actual, expected);
        }
    }
}
