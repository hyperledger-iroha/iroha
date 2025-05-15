//! `Transaction`-related functionality of Iroha.
//!
//!
//! Types represent various stages of a `Transaction`'s lifecycle. For
//! example, `Transaction` is the start, when a transaction had been
//! received by Torii.
//!
//! This is also where the actual execution of instructions, as well
//! as various forms of validation are performed.

use std::time::{Duration, SystemTime};

use eyre::Result;
use iroha_crypto::SignatureOf;
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
pub struct AcceptedTransaction(pub(super) SignedTransaction);

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
    /// The genesis account can only sign transactions in the genesis block
    UnexpectedGenesisAccountSignature,
    /// Chain id doesn't correspond to the id of current blockchain: {0}
    ChainIdMismatch(Mismatch<ChainId>),
    /// Transaction creation time is in the future
    TransactionInTheFuture,
}

impl AcceptedTransaction {
    fn validate(
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
    /// Accept genesis transaction. Transition from [`SignedTransaction`] to [`AcceptedTransaction`].
    ///
    /// # Errors
    ///
    /// - if transaction chain id doesn't match
    pub fn accept_genesis(
        tx: SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        genesis_account: &AccountId,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate(&tx, expected_chain_id, max_clock_drift)?;

        if genesis_account != tx.authority() {
            return Err(AcceptTransactionFail::UnexpectedGenesisAccountSignature);
        }

        Ok(Self(tx))
    }

    /// Accept transaction. Transition from [`SignedTransaction`] to [`AcceptedTransaction`].
    ///
    /// # Errors
    ///
    /// - if it does not adhere to limits
    pub fn accept(
        tx: SignedTransaction,
        expected_chain_id: &ChainId,
        max_clock_drift: Duration,
        limits: TransactionParameters,
    ) -> Result<Self, AcceptTransactionFail> {
        Self::validate(&tx, expected_chain_id, max_clock_drift)?;

        if *iroha_genesis::GENESIS_DOMAIN_ID == *tx.authority().domain() {
            return Err(AcceptTransactionFail::UnexpectedGenesisAccountSignature);
        }

        match &tx.instructions() {
            Executable::Instructions(instructions) => {
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

        Ok(Self(tx))
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
    /// Move transaction lifecycle forward by checking if the
    /// instructions can be applied to the [`StateBlock`].
    ///
    /// Validation is skipped for genesis.
    ///
    /// # Errors
    /// Fails if validation of instruction fails (e.g. permissions mismatch).
    pub fn validate_transaction(
        &mut self,
        tx: AcceptedTransaction,
        wasm_cache: &mut WasmCache<'_, '_, '_>,
    ) -> Result<SignedTransaction, (Box<SignedTransaction>, TransactionRejectionReason)> {
        let mut state_transaction = self.transaction();
        if let Err(rejection_reason) =
            Self::validate_transaction_internal(tx.clone(), &mut state_transaction, wasm_cache)
        {
            return Err((tx.0.into(), rejection_reason));
        }
        state_transaction.apply();

        Ok(tx.0)
    }

    fn validate_transaction_internal(
        tx: AcceptedTransaction,
        state_transaction: &mut StateTransaction<'_, '_>,
        wasm_cache: &mut WasmCache<'_, '_, '_>,
    ) -> Result<(), TransactionRejectionReason> {
        let authority = tx.as_ref().authority();

        if state_transaction.world.accounts.get(authority).is_none() {
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
        state_transaction.execute_data_triggers_dfs()?;
        debug!("Data triggers executed successfully");

        Ok(())
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
mod tests {
    use std::sync::LazyLock;

    use iroha_data_model::{block::SignedBlock, isi::Instruction, prelude::EventBox};
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_test_samples::{gen_account_in, PEER_KEYPAIR};

    use super::*;
    use crate::{
        block::{BlockBuilder, ValidBlock},
        smartcontracts::{
            isi::Registrable,
            triggers::{set::SetTransaction, specialized::SpecializedTrigger},
        },
        state::{State, StateBlock, StateReadOnly, World},
        sumeragi::network_topology::Topology,
    };

    /// The origin that initiates a chain of data triggers.
    enum TriggerOrigin {
        /// A user-submitted transaction.
        ExternalTransaction,
        /// A scheduled time-based trigger.
        TimeTrigger,
    }

    mod time_trigger {
        use super::*;

        /// # Scenario
        ///
        /// 1. Transaction transfers an asset from Alice to Bob.
        /// 2. Data trigger fires and transfers a LARGE asset from Bob to Carol.
        /// 3. Time trigger fires and __should succeed__ to transfer the LARGE asset from Carol to Dave.
        /// 4. Data trigger fires and transfers the LARGE asset from Dave to Eve.
        #[tokio::test]
        #[expect(clippy::cast_possible_wrap)]
        async fn fires_after_external_transactions() {
            let mut sandbox = Sandbox::new()
                .with_data_trigger_oneshot("bob", "carol", INIT_BALANCE["bob"])
                .with_time_trigger_oneshot("carol", "dave", INIT_BALANCE["bob"])
                .with_data_trigger_oneshot("dave", "eve", INIT_BALANCE["bob"]);
            sandbox.transfer_one("alice", "bob");
            let mut block = sandbox.block();
            let _events = block.apply();
            block.assert_balances([
                ("alice", -1),
                ("bob", 1 - INIT_BALANCE["bob"] as i32),
                ("carol", 0),
                ("dave", 0),
                ("eve", INIT_BALANCE["bob"] as i32),
            ]);
        }
    }

    mod data_trigger {
        use super::*;

        /// # Scenario
        ///
        /// 1. Transaction transfers an asset from Alice to Bob.
        /// 2. Trigger fires and transfers a LARGE asset from Bob to Carol.
        /// 3. Transaction __should succeed__ to transfer the LARGE asset from Carol to Dave.
        #[tokio::test]
        #[expect(clippy::cast_possible_wrap)]
        async fn fires_for_each_transaction() {
            let mut sandbox =
                Sandbox::new().with_data_trigger_oneshot("bob", "carol", INIT_BALANCE["bob"]);
            sandbox.transfer_one("alice", "bob");
            sandbox.transfers_batched(1, "carol", "dave", INIT_BALANCE["bob"]);
            let mut block = sandbox.block();
            let _events = block.apply();
            block.assert_balances([
                ("alice", -1),
                ("bob", 1 - INIT_BALANCE["bob"] as i32),
                ("carol", 0),
                ("dave", INIT_BALANCE["bob"] as i32),
            ]);
        }

        /// # Scenario
        ///
        /// 1. Transaction transfers an asset from Alice to Bob twice.
        /// 2. Trigger __should fire once__ and transfer one from Bob to Carol.
        #[tokio::test]
        async fn fires_at_most_once_per_transaction() {
            let mut sandbox = Sandbox::new().with_data_trigger("bob", "carol");
            sandbox.transfers_batched(2, "alice", "bob", 1);
            let mut block = sandbox.block();
            let _events = block.apply();
            block.assert_balances([("alice", -2), ("bob", 1), ("carol", 1)]);
        }

        /// # Scenario
        ///
        /// 1. Transaction transfers an asset from Alice to Bob.
        /// 2. Depth-first trigger chain: Bob -> Carol -> Dave -> Bob.
        /// 3. Sibling trigger (Bob -> Eve) executes only after the depth-first chain completes.
        #[tokio::test]
        #[expect(clippy::cast_possible_wrap)]
        async fn chains_in_depth_first_order() {
            let mut sandbox = Sandbox::new()
                .with_data_trigger_oneshot("bob", "carol", INIT_BALANCE["bob"])
                // Sibling trigger waits for depth-first resolution.
                .with_data_trigger_oneshot("bob", "eve", INIT_BALANCE["bob"])
                .with_data_trigger_oneshot("carol", "dave", INIT_BALANCE["bob"])
                .with_data_trigger_oneshot("dave", "bob", INIT_BALANCE["bob"]);
            sandbox.transfer_one("alice", "bob");
            let mut block = sandbox.block();
            let _events = block.apply();
            block.assert_balances([
                ("alice", -1),
                ("bob", 1 - INIT_BALANCE["bob"] as i32),
                ("carol", 0),
                ("dave", 0),
                ("eve", INIT_BALANCE["bob"] as i32),
            ]);
        }

        /// All or none of the initial transaction and subsequent data triggers should take effect.
        #[tokio::test]
        async fn atomically_chains_from_transaction() {
            aborts_on_execution_error(&TriggerOrigin::ExternalTransaction);
            aborts_on_exceeding_depth(&TriggerOrigin::ExternalTransaction);
            commits_on_depleting_lives(&TriggerOrigin::ExternalTransaction);
            commits_on_regular_success(&TriggerOrigin::ExternalTransaction);
        }

        /// All or none of the initial time trigger and subsequent data triggers should take effect.
        #[tokio::test]
        async fn atomically_chains_from_time_trigger() {
            aborts_on_execution_error(&TriggerOrigin::TimeTrigger);
            aborts_on_exceeding_depth(&TriggerOrigin::TimeTrigger);
            commits_on_depleting_lives(&TriggerOrigin::TimeTrigger);
            commits_on_regular_success(&TriggerOrigin::TimeTrigger);
        }

        fn aborts_on_execution_error(origin: &TriggerOrigin) {
            let mut sandbox = Sandbox::new()
                .with_data_trigger("bob", "carol")
                // This trigger execution fails.
                .with_data_trigger_fail("carol", "dave");
            if let TriggerOrigin::TimeTrigger = origin {
                sandbox = sandbox.with_time_trigger("alice", "bob");
            }
            if let TriggerOrigin::ExternalTransaction = origin {
                sandbox.transfer_one("alice", "bob");
            }
            let mut block = sandbox.block();
            let _events = block.apply();
            // Everything should be rolled back.
            block.assert_balances([("alice", 0), ("bob", 0), ("carol", 0), ("dave", 0)]);
        }

        fn aborts_on_exceeding_depth(origin: &TriggerOrigin) {
            let mut sandbox = Sandbox::new()
                .with_max_execution_depth(2)
                .with_data_trigger("bob", "carol")
                .with_data_trigger("carol", "dave")
                // The execution sequence exceeds the depth limit.
                .with_data_trigger("dave", "eve");

            if let TriggerOrigin::TimeTrigger = origin {
                sandbox = sandbox.with_time_trigger("alice", "bob");
            }
            if let TriggerOrigin::ExternalTransaction = origin {
                sandbox.transfer_one("alice", "bob");
            }
            let mut block = sandbox.block();
            let _events = block.apply();
            // Everything should be rolled back.
            block.assert_balances([
                ("alice", 0),
                ("bob", 0),
                ("carol", 0),
                ("dave", 0),
                ("eve", 0),
            ]);
        }

        fn commits_on_depleting_lives(origin: &TriggerOrigin) {
            let mut sandbox = Sandbox::new()
                .with_data_trigger("bob", "carol")
                // This trigger depletes after an execution.
                .with_data_trigger_oneshot("carol", "bob", 1);
            if let TriggerOrigin::TimeTrigger = origin {
                sandbox = sandbox.with_time_trigger("alice", "bob");
            }
            if let TriggerOrigin::ExternalTransaction = origin {
                sandbox.transfer_one("alice", "bob");
            }
            let mut block = sandbox.block();
            let _events = block.apply();
            // Everything should be rolled back.
            block.assert_balances([("alice", -1), ("bob", 0), ("carol", 1)]);
        }

        fn commits_on_regular_success(origin: &TriggerOrigin) {
            let mut sandbox = Sandbox::new()
                .with_max_execution_depth(3)
                .with_data_trigger("bob", "carol")
                .with_data_trigger("carol", "dave")
                .with_data_trigger("dave", "eve");
            if let TriggerOrigin::TimeTrigger = origin {
                sandbox = sandbox.with_time_trigger("alice", "bob");
            }
            if let TriggerOrigin::ExternalTransaction = origin {
                sandbox.transfer_one("alice", "bob");
            }
            let mut block = sandbox.block();
            let _events = block.apply();
            // The execution sequence should take effect.
            block.assert_balances([
                ("alice", -1),
                ("bob", 0),
                ("carol", 0),
                ("dave", 0),
                ("eve", 1),
            ]);
        }
    }

    struct Sandbox {
        state: State,
        // Buffered transactions
        transactions: Vec<SignedTransaction>,
    }

    struct SandboxBlock<'state> {
        state: StateBlock<'state>,
        // Candidate to be validated and committed
        block: Option<SignedBlock>,
    }

    const DOMAIN_STR: &str = "wonderland";
    const ASSET_STR: &str = "rose";
    const ACCOUNTS_STR: [&str; 5] = ["alice", "bob", "carol", "dave", "eve"];
    const INIT_EXECUTION_DEPTH: u8 = 5;

    static DOMAIN: LazyLock<DomainId> = LazyLock::new(|| DOMAIN_STR.parse().unwrap());
    static ASSET: LazyLock<AssetDefinitionId> =
        LazyLock::new(|| format!("{ASSET_STR}#{DOMAIN_STR}").parse().unwrap());
    static ACCOUNT: LazyLock<AccountMap> = LazyLock::new(|| {
        ACCOUNTS_STR
            .iter()
            .map(|name| {
                let key_pair = iroha_crypto::KeyPair::random().into_parts();
                let credential = Credential {
                    id: format!("{}@{DOMAIN_STR}", key_pair.0).parse().unwrap(),
                    key: key_pair.1,
                };
                (*name, credential)
            })
            .collect()
    });
    static INIT_BALANCE: LazyLock<AccountBalance> =
        LazyLock::new(|| ACCOUNTS_STR.into_iter().zip([10, 10, 1, 1, 1]).collect());

    type AccountMap = std::collections::BTreeMap<&'static str, Credential>;
    type AccountBalance = std::collections::BTreeMap<&'static str, u32>;
    type AccountBalanceDiff = std::collections::BTreeMap<&'static str, i32>;

    #[derive(Debug, Clone)]
    struct Credential {
        id: AccountId,
        key: iroha_crypto::PrivateKey,
    }

    static TOPOLOGY: LazyLock<Topology> = LazyLock::new(|| {
        let leader: PeerId = PEER_KEYPAIR.public_key().clone().into();
        Topology::new([leader])
    });
    static GENESIS_ACCOUNT: LazyLock<Credential> = LazyLock::new(|| {
        let (id, key_pair) = gen_account_in(GENESIS_DOMAIN_ID.clone());
        Credential {
            id,
            key: key_pair.into_parts().1,
        }
    });
    static CHAIN_ID: LazyLock<ChainId> =
        LazyLock::new(|| ChainId::from("00000000-0000-0000-0000-000000000000"));

    fn asset(account_name: &str) -> AssetId {
        AssetId::new(ASSET.clone(), ACCOUNT[account_name].id.clone())
    }

    fn transfer_one<'a>(
        src: &'a str,
        dest: &'a str,
    ) -> impl IntoIterator<Item = impl Instruction> + 'a {
        transfers_batched(1, src, dest, 1u32)
    }

    fn transfers_batched<'a>(
        n_instructions: usize,
        src: &'a str,
        dest: &'a str,
        q_per_instruction: u32,
    ) -> impl IntoIterator<Item = impl Instruction> + 'a {
        (0..n_instructions).map(move |_| {
            Transfer::asset_numeric(asset(src), q_per_instruction, ACCOUNT[dest].id.clone())
        })
    }

    impl Sandbox {
        fn new() -> Self {
            let world = {
                let domain = Domain::new(DOMAIN.clone()).build(&GENESIS_ACCOUNT.id);
                let asset_def = AssetDefinition::new(ASSET.clone(), NumericSpec::default())
                    .build(&GENESIS_ACCOUNT.id);
                let accounts = ACCOUNT
                    .clone()
                    .into_iter()
                    .chain([("genesis", GENESIS_ACCOUNT.clone())])
                    .map(|(_name, cred)| Account::new(cred.id.clone()).build(&GENESIS_ACCOUNT.id));
                let assets = INIT_BALANCE
                    .iter()
                    .map(|(name, num)| Asset::new(asset(name), *num));

                World::with_assets([domain], accounts, [asset_def], assets)
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

        fn with_time_trigger(self, src: &str, dest: &str) -> Self {
            let engine = self.state.engine.clone();
            self.with_trigger(
                "time",
                src,
                dest,
                transfer_one(src, dest),
                None,
                TimeEventFilter::new(ExecutionTime::PreCommit),
                |txn, trg| txn.add_time_trigger(&engine, trg),
            )
        }

        fn with_time_trigger_oneshot(self, src: &str, dest: &str, q_per_instruction: u32) -> Self {
            let engine = self.state.engine.clone();
            self.with_trigger(
                "time_oneshot",
                src,
                dest,
                transfers_batched(1, src, dest, q_per_instruction),
                Some(1),
                TimeEventFilter::new(ExecutionTime::PreCommit),
                |txn, trg| txn.add_time_trigger(&engine, trg),
            )
        }

        fn with_data_trigger(self, src: &str, dest: &str) -> Self {
            let engine = self.state.engine.clone();
            self.with_trigger(
                "data",
                src,
                dest,
                transfer_one(src, dest),
                None,
                AssetEventFilter::new()
                    .for_events(AssetEventSet::Added)
                    .for_asset(asset(src))
                    .into(),
                |txn, trg| txn.add_data_trigger(&engine, trg),
            )
        }

        fn with_data_trigger_oneshot(self, src: &str, dest: &str, q_per_instruction: u32) -> Self {
            let engine = self.state.engine.clone();
            self.with_trigger(
                "data_oneshot",
                src,
                dest,
                transfers_batched(1, src, dest, q_per_instruction),
                Some(1),
                AssetEventFilter::new()
                    .for_events(AssetEventSet::Added)
                    .for_asset(asset(src))
                    .into(),
                |txn, trg| txn.add_data_trigger(&engine, trg),
            )
        }

        fn with_data_trigger_fail(self, src: &str, dest: &str) -> Self {
            let engine = self.state.engine.clone();
            self.with_trigger(
                "data_fail",
                src,
                dest,
                [Unregister::domain("nowhere".parse().unwrap())],
                None,
                AssetEventFilter::new()
                    .for_events(AssetEventSet::Added)
                    .for_asset(asset(src))
                    .into(),
                |txn, trg| txn.add_data_trigger(&engine, trg),
            )
        }

        #[expect(clippy::too_many_arguments)]
        fn with_trigger<F, G>(
            self,
            id_prefix: &str,
            src: &str,
            dest: &str,
            instructions: impl IntoIterator<Item = impl Instruction>,
            lives: Option<u32>,
            filter: F,
            add_trigger: G,
        ) -> Self
        where
            F: Into<EventFilterBox>,
            SpecializedTrigger<F>: TryFrom<Trigger>,
            <SpecializedTrigger<F> as TryFrom<Trigger>>::Error: std::fmt::Debug,
            G: FnOnce(
                &mut SetTransaction,
                SpecializedTrigger<F>,
            ) -> Result<bool, crate::smartcontracts::triggers::set::Error>,
        {
            let mut block = self.state.world.triggers.block();
            let mut transaction: SetTransaction<'_, '_> = block.transaction();
            let trigger = Trigger::new(
                format!("{id_prefix}-{src}-{dest}").parse().unwrap(),
                Action::new(
                    instructions,
                    lives.map_or(Repeats::Indefinitely, Repeats::Exactly),
                    GENESIS_ACCOUNT.id.clone(),
                    filter,
                ),
            )
            .try_into()
            .unwrap();

            add_trigger(&mut transaction, trigger).unwrap();
            transaction.apply();
            block.commit();
            self
        }

        fn with_max_execution_depth(self, depth: u8) -> Self {
            let mut world = self.state.world.block();
            world.parameters.smart_contract.execution_depth = depth;
            world.commit();
            self
        }

        fn transfer_one(&mut self, src: &str, dest: &str) {
            self.transfers_batched(1, src, dest, 1u32);
        }

        fn transfers_batched(
            &mut self,
            n_instructions: usize,
            src: &str,
            dest: &str,
            q_per_instruction: u32,
        ) {
            let transaction = {
                let instructions = transfers_batched(n_instructions, src, dest, q_per_instruction);
                TransactionBuilder::new(CHAIN_ID.clone(), GENESIS_ACCOUNT.id.clone())
                    .with_instructions(instructions)
                    .sign(&GENESIS_ACCOUNT.key)
            };
            self.transactions.push(transaction);
        }

        fn block(&mut self) -> SandboxBlock<'_> {
            let block: SignedBlock = {
                let transactions = {
                    let signed = core::mem::take(&mut self.transactions);
                    // Skip static analysis (AcceptedTransaction::accept)
                    signed.into_iter().map(AcceptedTransaction).collect()
                };
                BlockBuilder::new(transactions)
                    .chain(0, self.state.view().latest_block().as_deref())
                    .sign(&GENESIS_ACCOUNT.key)
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
        fn apply(&mut self) -> Vec<EventBox> {
            let valid = ValidBlock::validate(
                core::mem::take(&mut self.block).unwrap(),
                &TOPOLOGY,
                &CHAIN_ID,
                &GENESIS_ACCOUNT.id,
                &mut self.state,
            )
            .unpack(|_| {})
            .unwrap();

            let committed = valid.commit(&TOPOLOGY).unpack(|_| {}).unwrap();
            self.state
                .apply_without_execution(&committed, TOPOLOGY.iter().cloned().collect())
        }

        fn assert_balances(&self, expected: impl Into<AccountBalanceDiff>) {
            let actual: AccountBalance = ACCOUNTS_STR
                .iter()
                .map(|name| {
                    let balance = self
                        .state
                        .world
                        .assets
                        .get(&asset(name))
                        .map_or(Numeric::ZERO, |asset| asset.value)
                        .try_into()
                        .unwrap();
                    (*name, balance)
                })
                .collect();

            #[expect(clippy::cast_possible_wrap)]
            expected.into().iter().for_each(|(name, diff)| {
                assert_eq!(actual[name] as i32, INIT_BALANCE[name] as i32 + *diff,);
            });
        }
    }
}
