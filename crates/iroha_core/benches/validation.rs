//! Benchmarks for transaction signing, acceptance, validation, and block signing.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::all)]
#![allow(clippy::disallowed_types)] // benches use HashSet internally for metrics
use std::sync::{Arc, LazyLock};

use criterion::{BatchSize, Criterion};
use iroha_core::{
    block::*,
    prelude::*,
    query::store::LiveQueryStore,
    smartcontracts::{Execute, isi::Registrable as _, ivm::cache::IvmCache},
    state::{State, World},
};
use iroha_data_model::{
    account::AccountId,
    isi::{InstructionBox, Log},
    prelude::*,
    transaction::{IvmBytecode, TransactionBuilder},
};
use iroha_test_samples::gen_account_in;

static STARTER_DOMAIN: LazyLock<DomainId> = LazyLock::new(|| "start".parse().unwrap());
static STARTER_KEYPAIR: LazyLock<KeyPair> = LazyLock::new(KeyPair::random);
static STARTER_ID: LazyLock<AccountId> =
    LazyLock::new(|| AccountId::new(STARTER_DOMAIN.clone(), STARTER_KEYPAIR.public_key().clone()));

// Shared Tokio runtime for benches that need background tasks (e.g., LiveQueryStore)
static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime")
});

fn build_test_transaction(chain_id: ChainId) -> TransactionBuilder {
    let domain_id: DomainId = "domain".parse().unwrap();
    let create_domain = Register::domain(Domain::new(domain_id.clone()));
    let create_account = Register::account(Account::new(gen_account_in(&domain_id).0));
    let asset_definition_id = "xor#domain".parse().unwrap();
    let create_asset = Register::asset_definition(AssetDefinition::numeric(asset_definition_id));

    TransactionBuilder::new(chain_id, STARTER_ID.clone()).with_instructions::<InstructionBox>([
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ])
}

fn build_test_and_transient_state() -> State {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    // Ensure Tokio reactor is available for LiveQueryStore background task
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();
    let (account_id, key_pair) = gen_account_in(&*STARTER_DOMAIN);

    let state = State::new(
        {
            let domain = Domain::new(STARTER_DOMAIN.clone()).build(&account_id);
            let account = Account::new(account_id.clone()).build(&account_id);
            World::with([domain], [account], [])
        },
        Arc::clone(&kura),
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    );

    {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let transaction = TransactionBuilder::new(chain_id.clone(), account_id.clone())
            .with_instructions([Log::new(Level::INFO, "init".to_string())])
            .sign(key_pair.private_key());
        let (max_clock_drift, tx_limits) = {
            let state_view = state.view();
            let params = state_view.world.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let crypto_cfg = state.crypto();
        let unverified_block = BlockBuilder::new(vec![
            AcceptedTransaction::accept(
                transaction,
                &chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            )
            .unwrap(),
        ])
        .chain(0, state.view().latest_block().as_deref())
        .sign(key_pair.private_key())
        .unpack(|_| {});
        let signed_block = Arc::new(SignedBlock::from(unverified_block.clone()));
        let mut state_block = state.block(unverified_block.header());
        let mut state_transaction = state_block.transaction();
        let path_to_executor =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");
        let bytecode = std::fs::read(&path_to_executor)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", path_to_executor.display()));
        let executor = Executor::new(IvmBytecode::from_compiled(bytecode));
        let (authority, _authority_keypair) = gen_account_in("genesis");
        // Ignore upgrade failure and keep the default executor when bytecode is invalid
        let _ = Upgrade::new(executor).execute(&authority, &mut state_transaction);
        state_transaction.apply();
        // Mark the transaction as recorded in this block before commit
        {
            use std::collections::HashSet;
            let hash = unverified_block
                .transactions()
                .first()
                .expect("Block must contain transaction")
                .as_ref()
                .hash();
            let height = std::num::NonZeroUsize::new(
                unverified_block
                    .header()
                    .height()
                    .get()
                    .try_into()
                    .expect("Block height fits into usize"),
            )
            .expect("Block height is non-zero");
            state_block
                .transactions
                .insert_block(HashSet::from([hash]), height);
        }
        state_block.block_hashes.push_for_tests(signed_block.hash());
        state_block.commit().unwrap();
        kura.store_block(signed_block)
            .expect("store block in bench setup");
    }

    state
}

fn accept_transaction(criterion: &mut Criterion) {
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let state = build_test_and_transient_state();
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };

    let transaction = build_test_transaction(chain_id.clone()).sign(STARTER_KEYPAIR.private_key());
    let crypto_cfg = state.crypto();
    let mut success_count = 0;
    let mut failures_count = 0;
    let _ = criterion.bench_function("accept", |b| {
        b.iter(|| {
            match AcceptedTransaction::accept(
                transaction.clone(),
                &chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            ) {
                Ok(_) => success_count += 1,
                Err(_) => failures_count += 1,
            }
        });
    });
    println!("Success count: {success_count}, Failures count: {failures_count}");
}

fn sign_transaction(criterion: &mut Criterion) {
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let transaction = build_test_transaction(chain_id);
    let (_, private_key) = KeyPair::random().into_parts();
    let mut count = 0;
    let _ = criterion.bench_function("sign", |b| {
        b.iter_batched(
            || transaction.clone(),
            |transaction| {
                let _: SignedTransaction = transaction.sign(&private_key);
                count += 1;
            },
            BatchSize::SmallInput,
        );
    });
    println!("Count: {count}");
}

fn validate_transaction(criterion: &mut Criterion) {
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let state = build_test_and_transient_state();

    let (account_id, key_pair) = gen_account_in(&*STARTER_DOMAIN);
    let transaction = TransactionBuilder::new(chain_id.clone(), account_id.clone())
        .with_instructions([Log::new(Level::INFO, "init".to_string())])
        .sign(key_pair.private_key());
    let (max_clock_drift, tx_limits) = {
        let state_view = state.view();
        let params = state_view.world.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let crypto_cfg = state.crypto();
    let unverified_block = BlockBuilder::new(vec![
        AcceptedTransaction::accept(
            transaction,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .unwrap(),
    ])
    .chain(0, state.view().latest_block().as_deref())
    .sign(key_pair.private_key())
    .unpack(|_| {});
    let signed_block = Arc::new(SignedBlock::from(unverified_block.clone()));
    let transaction = AcceptedTransaction::accept(
        build_test_transaction(chain_id.clone()).sign(STARTER_KEYPAIR.private_key()),
        &chain_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("Failed to accept transaction.");
    let mut success_count = 0;
    let mut failure_count = 0;
    let mut ivm_cache = IvmCache::new();
    let mut state_block = state.block(unverified_block.header());
    let _ = criterion.bench_function("validate", |b| {
        b.iter(|| {
            match state_block
                .validate_transaction(transaction.clone(), &mut ivm_cache)
                .1
            {
                Ok(_) => success_count += 1,
                Err(_) => failure_count += 1,
            }
        });
    });
    {
        use std::collections::HashSet;
        let hash = unverified_block
            .transactions()
            .first()
            .expect("Block must contain transaction")
            .as_ref()
            .hash();
        let height = std::num::NonZeroUsize::new(
            unverified_block
                .header()
                .height()
                .get()
                .try_into()
                .expect("Block height fits into usize"),
        )
        .expect("Block height is non-zero");
        state_block
            .transactions
            .insert_block(HashSet::from([hash]), height);
    }
    state_block.block_hashes.push_for_tests(signed_block.hash());
    state_block.commit().unwrap();
    {
        let view = state.view();
        view.kura()
            .store_block(signed_block)
            .expect("store block in bench setup");
    }
    println!("Success count: {success_count}, Failure count: {failure_count}");
}

fn sign_blocks(criterion: &mut Criterion) {
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    // Ensure Tokio reactor is available for LiveQueryStore background task
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(
        World::new(),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    );
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };

    let crypto_cfg = state.crypto();
    let transaction = AcceptedTransaction::accept(
        build_test_transaction(chain_id.clone()).sign(STARTER_KEYPAIR.private_key()),
        &chain_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("Failed to accept transaction.");
    let (_, peer_private_key) = KeyPair::random().into_parts();

    let mut count = 0;

    let block =
        BlockBuilder::new(vec![transaction]).chain(0, state.view().latest_block().as_deref());

    let _ = criterion.bench_function("sign_block", |b| {
        b.iter_batched(
            || block.clone(),
            |block| {
                let _: NewBlock = block.sign(&peer_private_key).unpack(|_| {});
                count += 1;
            },
            BatchSize::SmallInput,
        );
    });
    println!("Count: {count}");
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence IVM banner if executor/VM paths initialize it.
    #[allow(unused_imports)]
    {
        use ivm::set_banner_enabled;
        set_banner_enabled(false);
    }
    let mut c = Criterion::default().configure_from_args();
    accept_transaction(&mut c);
    sign_transaction(&mut c);
    validate_transaction(&mut c);
    sign_blocks(&mut c);
    c.final_summary();
}
