// Complete trigger rows retain executable identity under the ordinary weighted query policy.
use super::*;
use iroha_core::{
    query::snapshot::{
        CursorMode, SnapshotQueryError, run_on_snapshot_with_server_owned_memory_arc,
    },
    smartcontracts::isi::query::{OrdinaryQueryMemoryLease, QueryCountMode, QueryLimits},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{
    events::execute_trigger::ExecuteTriggerEventFilter,
    isi::{InstructionBox, Register, SetKeyValue},
    query::{
        ErasedIterQuery, QueryBox, QueryRequest, QueryResponse, QueryWithParams,
        dsl::{CompoundPredicate, SelectorTuple},
        error::QueryExecutionFail,
        parameters::{FetchSize, QueryParams},
        trigger::prelude::FindTriggers,
    },
    transaction::{Executable, IvmBytecode},
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use norito::codec::Encode as _;

fn trigger_row(id: &str, executable: Executable, authority: &AccountId) -> Trigger {
    let id = id.parse::<TriggerId>().unwrap();
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert("contract_entrypoint".parse().unwrap(), "run");
    Trigger::new(
        id.clone(),
        Action::new(
            executable,
            Repeats::Indefinitely,
            authority.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(id)
                .under_authority(authority.clone()),
        )
        .unwrap()
        .with_metadata(metadata),
    )
}

fn trigger_chain(rows: &[Trigger], authority: &AccountId) -> CertifiedTestChain {
    let world = tests_runtime_handlers::world_with_account(authority);
    let mut config = TestChainConfig::new(world, 1_000);
    config.genesis_instructions.extend(
        rows.iter()
            .cloned()
            .map(Register::trigger)
            .map(InstructionBox::from),
    );
    let chain = CertifiedTestChain::start(config)
        .expect("signed genesis registers the original trigger rows");
    chain.setup_world_at(2_000, |transaction| {
        transaction
            .world_mut_for_testing()
            .add_account_permission(authority, CanReadAllLedgerData.into());
    });
    chain
}

fn trigger_start() -> QueryRequest {
    let query: QueryBox<iroha_data_model::query::QueryOutputBatchBox> =
        Box::new(ErasedIterQuery::<Trigger>::new(
            CompoundPredicate::PASS,
            SelectorTuple::default(),
            FindTriggers.encode(),
        ));
    QueryRequest::Start(
        QueryWithParams::new(
            &query,
            QueryParams {
                fetch_size: FetchSize::new(NonZeroU64::new(1)),
                ..QueryParams::default()
            },
        )
        .unwrap(),
    )
}

fn trigger_query(
    chain: &CertifiedTestChain,
    authority: &AccountId,
    request: QueryRequest,
    policy: OrdinaryQueryServerPolicy,
    pool: &ByteWeightedMemoryPool,
) -> Result<iroha_core::query::snapshot::ServerOwnedQueryResponse, SnapshotQueryError> {
    let permit = if matches!(request, QueryRequest::Start(_)) {
        pool.try_acquire_parts(policy.start_reservation_parts())
    } else {
        pool.try_acquire_parts(policy.execution_reservation_parts())
    }
    .expect("admit original policy weight");
    let memory = OrdinaryQueryMemoryLease::new(ToriiOrdinaryQueryMemoryReservation {
        permit,
        bytes_per_permit: pool.bytes_per_permit,
        pool_generation: pool.generation(),
    });
    run_on_snapshot_with_server_owned_memory_arc(
        chain.state(),
        &chain.state().view().query_handle,
        authority,
        request,
        CursorMode::Stored,
        QueryLimits::new(policy.max_fetch_size).with_count_mode(QueryCountMode::Bounded),
        Some(u64::MAX),
        policy.limits,
        memory,
    )
}

#[test]
fn ordinary_trigger_rows_preserve_nonempty_executables_and_cursor_charges() {
    let (geometry, pool) = default_geometry();
    let policy = default_policy(geometry, &pool);
    let key = tests_runtime_handlers::checked_torii_test_ed25519_keypair(
        0xB1,
        "nonempty trigger query authority",
    );
    let authority = AccountId::new(key.public_key().clone());
    let instruction =
        SetKeyValue::account(authority.clone(), "iterable_flag".parse().unwrap(), true);
    let rows = [
        trigger_row(
            "a_instruction",
            vec![InstructionBox::from(instruction)].into(),
            &authority,
        ),
        trigger_row(
            "b_ivm",
            IvmBytecode::from_compiled(
                include_bytes!("../../../../integration_tests/fixtures/ivm/mint_rose_trigger.to")
                    .to_vec(),
            )
            .into(),
            &authority,
        ),
    ];
    let chain = trigger_chain(&rows, &authority);
    let stored = rows
        .iter()
        .map(|original| {
            let actual = iroha_data_model::query::trigger::prelude::FindTriggerById::new(
                original.id().clone(),
            )
            .execute(&chain.state().view())
            .unwrap();
            let mut expected = original.clone();
            expected
                .action
                .metadata
                .insert("__registered_block_height".parse().unwrap(), 1_u64);
            expected
                .action
                .metadata
                .insert("__registered_at_ms".parse().unwrap(), 1_000_u64);
            assert_eq!(
                actual, expected,
                "registration only adds its authenticated height and time"
            );
            actual
        })
        .collect::<Vec<_>>();
    let capacity = pool.available_bytes();
    let mut request = trigger_start();
    for (index, expected) in stored.iter().enumerate() {
        let response = trigger_query(&chain, &authority, request, policy, &pool)
            .expect("complete trigger row is admitted");
        let (response, lease) = response.into_parts();
        let QueryResponse::Iterable(output) = response else {
            panic!("iterable trigger response")
        };
        let (batch, _, cursor) = output.into_parts();
        let iroha_data_model::query::QueryOutputBatchBox::Trigger(actual) =
            batch.into_iter().next().unwrap()
        else {
            panic!("trigger row")
        };
        assert_eq!(actual, [expected.clone()]);
        assert_eq!(
            norito::encode_canonical(&actual[0]).unwrap(),
            norito::encode_canonical(expected).unwrap()
        );
        drop(actual);
        drop(lease);
        if index == 0 {
            assert_eq!(
                pool.available_bytes(),
                capacity - policy.limits.max_cursor_retained_bytes()
            );
            let mut cursor = cursor.expect("second original row is retained");
            cursor.gas_budget = Some(u64::MAX);
            request = QueryRequest::Continue(cursor);
        } else {
            assert!(cursor.is_none());
            assert_eq!(
                pool.available_bytes(),
                capacity,
                "last page releases the same cursor pool"
            );
            break;
        }
    }
}

#[test]
fn ordinary_trigger_rows_refuse_oversize_source_and_refund_admission() {
    let (geometry, pool) = default_geometry();
    let policy = default_policy(geometry, &pool);
    let key = tests_runtime_handlers::checked_torii_test_ed25519_keypair(
        0xB2,
        "oversize trigger query authority",
    );
    let authority = AccountId::new(key.public_key().clone());
    let log = iroha_data_model::isi::Log::new(
        iroha_logger::Level::INFO,
        "x".repeat(usize::try_from(policy.limits.max_source_item_bytes()).unwrap()),
    );
    let row = trigger_row(
        "oversize_instruction",
        vec![InstructionBox::from(log)].into(),
        &authority,
    );
    assert!(
        norito::encode_canonical(&row).unwrap().len()
            > usize::try_from(policy.limits.max_source_item_bytes()).unwrap()
    );
    let chain = trigger_chain(&[row], &authority);
    let capacity = pool.available_bytes();
    assert!(matches!(
        trigger_query(&chain, &authority, trigger_start(), policy, &pool),
        Err(SnapshotQueryError::Execution(
            QueryExecutionFail::CapacityLimit
        ))
    ));
    assert_eq!(
        pool.available_bytes(),
        capacity,
        "refusal must not retain a cursor or response charge"
    );
}

#[test]
fn ordinary_source_rows_divide_the_admitted_phase_and_refuse_too_small_geometry() {
    let (geometry, pool) = default_geometry();
    let policy = default_policy(geometry, &pool);
    let envelope =
        QueryFanoutMemoryEnvelope::for_body_admission(geometry.fanout_working_set_bytes).unwrap();
    assert_eq!(
        policy.limits.max_source_item_bytes(),
        u64::try_from(envelope.candidate_allocation_bytes).unwrap() / (policy.max_fetch_size + 1)
    );
    assert!(
        policy.limits.max_source_item_bytes()
            > iroha_core::smartcontracts::isi::query::ORDINARY_NAME_ID_SOURCE_BYTES
    );
    assert!(
        policy.limits.execution_headroom_bytes()
            <= u64::try_from(geometry.fanout_working_set_bytes).unwrap()
    );
    let mut too_small = envelope;
    too_small.candidate_allocation_bytes = usize::try_from(
        (policy.max_fetch_size + 1)
            * iroha_core::smartcontracts::isi::query::ORDINARY_NAME_ID_SOURCE_BYTES
            - 1,
    )
    .unwrap();
    assert!(
        OrdinaryQueryServerPolicy::new(
            routing::AppQueryLimits::default(),
            geometry.ingress,
            too_small
        )
        .is_none()
    );
    let overflow = routing::AppQueryLimits {
        max_fetch_size: u64::MAX,
        ..routing::AppQueryLimits::default()
    };
    assert!(OrdinaryQueryServerPolicy::new(overflow, geometry.ingress, envelope).is_none());
}
