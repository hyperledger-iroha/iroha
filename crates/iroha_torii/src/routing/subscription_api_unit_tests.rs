#[test]
fn derive_trigger_id_is_deterministic() {
    let subscription_id: NftId = "sub1$wonderland.universal".parse().unwrap();
    let bill = derive_trigger_id("sub_bill_", &subscription_id).unwrap();
    let bill2 = derive_trigger_id("sub_bill_", &subscription_id).unwrap();
    let usage = derive_trigger_id("sub_usage_", &subscription_id).unwrap();
    assert_eq!(bill, bill2);
    assert_ne!(bill, usage);
}
#[test]
fn default_charge_ms_fixed_period_respects_bill_for() {
    let billing = SubscriptionBilling {
        cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
            period_ms: 1_000,
        }),
        bill_for: SubscriptionBillFor::PreviousPeriod,
        retry_backoff_ms: 0,
        max_failures: 0,
        grace_ms: 0,
    };
    assert_eq!(default_charge_ms(10_000, billing).unwrap(), 11_000);
    let billing_next = SubscriptionBilling {
        bill_for: SubscriptionBillFor::NextPeriod,
        ..billing
    };
    assert_eq!(default_charge_ms(10_000, billing_next).unwrap(), 10_000);
}
#[test]
fn initial_period_fixed_period_matches_charge_window() {
    let billing = SubscriptionBilling {
        cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
            period_ms: 2_000,
        }),
        bill_for: SubscriptionBillFor::PreviousPeriod,
        retry_backoff_ms: 0,
        max_failures: 0,
        grace_ms: 0,
    };
    let (start, end) = initial_period_for_charge(billing, 10_000).unwrap();
    assert_eq!(start, 8_000);
    assert_eq!(end, 10_000);
    let billing_next = SubscriptionBilling {
        bill_for: SubscriptionBillFor::NextPeriod,
        ..billing
    };
    let (start, end) = initial_period_for_charge(billing_next, 10_000).unwrap();
    assert_eq!(start, 10_000);
    assert_eq!(end, 12_000);
}
#[test]
fn parse_subscription_status_filter_accepts_known_values() {
    assert_eq!(
        parse_subscription_status_filter("active").unwrap(),
        SubscriptionStatus::Active
    );
    assert_eq!(
        parse_subscription_status_filter("past_due").unwrap(),
        SubscriptionStatus::PastDue
    );
    assert!(parse_subscription_status_filter("unknown").is_err());
}
#[test]
fn subscription_plan_from_metadata_roundtrips() {
    let plan = SubscriptionPlan {
        provider: ALICE_ID.clone(),
        billing: SubscriptionBilling {
            cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                period_ms: 1_000,
            }),
            bill_for: SubscriptionBillFor::NextPeriod,
            retry_backoff_ms: 0,
            max_failures: 0,
            grace_ms: 0,
        },
        pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
            amount: Quantity::from(10_u32),
            asset_definition: test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400f1"),
        }),
    };
    let mut metadata = Metadata::default();
    metadata.insert(
        (*SUBSCRIPTION_PLAN_KEY).clone(),
        IrohaJson::new(plan.clone()),
    );
    let parsed = subscription_plan_from_metadata(&metadata)
        .unwrap()
        .expect("plan metadata present");
    assert_eq!(parsed, plan);
}
#[test]
fn subscription_state_and_invoice_from_metadata_roundtrip() {
    let billing_trigger_id: TriggerId = "billing_trigger".parse().unwrap();
    let subscription = SubscriptionState {
        plan_id: test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400f3"),
        provider: ALICE_ID.clone(),
        subscriber: ALICE_ID.clone(),
        status: SubscriptionStatus::Active,
        current_period_start_ms: 1_000,
        current_period_end_ms: 2_000,
        next_charge_ms: 2_000,
        cancel_at_period_end: false,
        cancel_at_ms: None,
        failure_count: 0,
        usage_accumulated: std::collections::BTreeMap::new(),
        billing_trigger_id,
    };
    let invoice = SubscriptionInvoice {
        subscription_nft_id: "sub1$wonderland.universal".parse().unwrap(),
        period_start_ms: 1_000,
        period_end_ms: 2_000,
        attempted_at_ms: 2_000,
        amount: Quantity::from(5_u32),
        asset_definition: test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400f1"),
        status: SubscriptionInvoiceStatus::Paid,
        tx_hash: None,
    };
    let mut metadata = Metadata::default();
    metadata.insert(
        (*SUBSCRIPTION_KEY).clone(),
        IrohaJson::new(subscription.clone()),
    );
    metadata.insert(
        (*SUBSCRIPTION_INVOICE_KEY).clone(),
        IrohaJson::new(invoice.clone()),
    );
    let parsed_state = subscription_state_from_metadata(&metadata)
        .unwrap()
        .expect("state present");
    let parsed_invoice = subscription_invoice_from_metadata(&metadata)
        .unwrap()
        .expect("invoice present");
    assert_eq!(parsed_state, subscription);
    assert_eq!(parsed_invoice, invoice);
}
#[test]
fn resolve_charge_ms_prefers_explicit() {
    let billing = SubscriptionBilling {
        cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
            period_ms: 1_000,
        }),
        bill_for: SubscriptionBillFor::NextPeriod,
        retry_backoff_ms: 0,
        max_failures: 0,
        grace_ms: 0,
    };
    assert_eq!(resolve_charge_ms(billing, Some(42_000)).unwrap(), 42_000);
}
#[test]
fn resolve_trigger_id_prefers_explicit() {
    let subscription_id: NftId = "sub2$wonderland.universal".parse().unwrap();
    let explicit: TriggerId = "explicit_trigger".parse().unwrap();
    let resolved =
        resolve_trigger_id("sub_bill_", &subscription_id, Some(explicit.clone())).unwrap();
    assert_eq!(resolved, explicit);
}
#[test]
fn network_time_ms_is_nonzero() {
    let now = network_time_ms().unwrap();
    assert!(now > 0);
}
#[test]
fn ivm_syscall_program_emits_bytecode() {
    let configured_limit = NonZeroU64::new(17).expect("non-zero test cycle limit");
    let program = ivm_syscall_program(ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL, configured_limit);
    assert!(!program.as_ref().is_empty());
    assert_eq!(
        ivm::ProgramMetadata::parse(program.as_ref())
            .expect("generated subscription program metadata")
            .metadata
            .max_cycles,
        configured_limit.get(),
        "Torii must embed the live admission ceiling, not a compiled default"
    );
    let admitted = iroha_core::smartcontracts::ivm::cache::IvmCache::new()
        .summarize_executable(program.as_ref())
        .expect("subscription syscall helper must be a valid program");
    assert!(matches!(
        admitted,
        iroha_core::smartcontracts::ivm::cache::ExecutableProgramSummary::Generic(_)
    ));
}
#[test]
fn build_billing_trigger_attaches_metadata_and_schedule() {
    use iroha_data_model::events::{EventFilterBox, time::ExecutionTime};
    let trigger_id: TriggerId = "bill_trigger".parse().unwrap();
    let subscription_id: NftId = "sub3$wonderland.universal".parse().unwrap();
    let authority = ALICE_ID.clone();
    let trigger = build_billing_trigger(
        trigger_id.clone(),
        authority.clone(),
        subscription_id.clone(),
        55,
        defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
    );
    assert_eq!(trigger.id(), &trigger_id);
    let meta = trigger.metadata();
    let ref_value = meta
        .get(&*SUBSCRIPTION_TRIGGER_REF_KEY)
        .expect("subscription_ref metadata");
    let parsed: SubscriptionTriggerRef = ref_value.try_into_any_norito().unwrap();
    assert_eq!(parsed.subscription_nft_id, subscription_id);
    match trigger.action().filter() {
        EventFilterBox::Time(filter) => match filter.0 {
            ExecutionTime::Schedule(schedule) => {
                assert_eq!(schedule.start_ms, 55);
                assert_eq!(schedule.period_ms, None);
            }
            _ => panic!("expected schedule execution time"),
        },
        _ => panic!("expected time filter"),
    }
    assert_eq!(trigger.action().authority(), &authority);
}
#[test]
fn build_usage_trigger_uses_execute_filter() {
    use iroha_data_model::events::{EventFilterBox, execute_trigger::ExecuteTriggerEventFilter};
    let trigger_id: TriggerId = "usage_trigger".parse().unwrap();
    let authority = ALICE_ID.clone();
    let trigger = build_usage_trigger(
        trigger_id.clone(),
        authority.clone(),
        defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
    );
    match trigger.action().filter() {
        EventFilterBox::ExecuteTrigger(filter) => {
            let expected = ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(authority.clone());
            assert_eq!(filter, &expected);
        }
        _ => panic!("expected execute trigger filter"),
    }
}
