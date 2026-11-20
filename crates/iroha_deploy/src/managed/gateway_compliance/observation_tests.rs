//! Post-restart catalog checks reuse genuine original signatures and perform only one status GET.
use super::*;

#[test]
fn observation_reopens_exact_promoted_original_without_mutating_any_retained_bytes() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("compliance-observe");
    let provider = provider(&prepared);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    let path = operation(&publisher);
    let mut script = complete_script();
    script.push(Step::Status(Observation::Promoted));
    let mut http = RuntimeHttp::start(&prepared, provider, &path, script);
    let expected = publisher
        .advance(&mut TestLive::default(), deadline())
        .unwrap();
    let original = bytes(&path, "original.nrt");
    let ack = bytes(&path, "acknowledgement.nrt");
    let before = http.requests.lock().unwrap().len();
    drop(publisher);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    publisher
        .observe_promoted(&mut TestLive::default(), expected, deadline())
        .unwrap();
    assert_eq!(bytes(&path, "original.nrt"), original);
    assert_eq!(bytes(&path, "acknowledgement.nrt"), ack);
    http.finish();
    let requests = http.requests.lock().unwrap();
    assert_eq!(requests.len(), before + 1);
    assert_eq!(requests.last().unwrap().method, "GET");
}

#[test]
fn observation_requires_exact_original_ack_and_live_guard_before_http() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("compliance-observe-refusal");
    let provider = provider(&prepared);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    let path = operation(&publisher);
    let mut http = RuntimeHttp::start(&prepared, provider, &path, complete_script());
    let expected = publisher
        .advance(&mut TestLive::default(), deadline())
        .unwrap();
    let before = http.requests.lock().unwrap().len();
    let mut earlier_issue = expected;
    earlier_issue.generated_at_unix = earlier_issue.generated_at_unix.saturating_sub(1);
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), earlier_issue, deadline())
            .is_err()
    );
    let mut foreign = expected;
    foreign.digest[0] ^= 1;
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), foreign, deadline())
            .is_err()
    );
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), expected, Instant::now())
            .is_err()
    );
    assert!(
        publisher
            .observe_promoted(
                &mut TestLive {
                    checks: 0,
                    fail_at: Some(1)
                },
                expected,
                deadline()
            )
            .is_err()
    );
    let child = PrivateDirectory::open_exact(path.join("catalogs").join(catalog_name(1))).unwrap();
    child
        .write_atomic("acknowledgement.nrt", b"changed ACK", PublishMode::Replace)
        .unwrap();
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), expected, deadline())
            .is_err()
    );
    assert_eq!(http.requests.lock().unwrap().len(), before);
    http.finish();
}

#[test]
fn observation_rejects_live_guard_loss_or_original_change_during_status_without_post() {
    let _resources = crate::managed::native_test_guard();
    for (label, step, fail_at) in [
        ("guard", Step::Status(Observation::Promoted), Some(3)),
        (
            "deleted",
            Step::ChangeAfterStatus(Observation::Promoted, RetainedChange::DeleteOriginal),
            None,
        ),
        (
            "replaced",
            Step::ChangeAfterStatus(Observation::Promoted, RetainedChange::ReplaceOriginal),
            None,
        ),
        (
            "ack",
            Step::ChangeAfterStatus(Observation::Promoted, RetainedChange::CorruptAck),
            None,
        ),
        ("foreign", Step::Status(Observation::ForeignHead), None),
        ("stale", Step::Status(Observation::Stale), None),
        (
            "pending",
            Step::Status(Observation::Candidate { ack: true }),
            None,
        ),
    ] {
        let (_temporary, prepared) = fixture(&format!("compliance-observe-{label}"));
        let provider = provider(&prepared);
        let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
        let path = operation(&publisher);
        let mut script = complete_script();
        script.push(step);
        let mut http = RuntimeHttp::start(&prepared, provider, &path, script);
        let expected = publisher
            .advance(&mut TestLive::default(), deadline())
            .unwrap();
        let before = http.requests.lock().unwrap().len();
        assert!(
            publisher
                .observe_promoted(&mut TestLive { checks: 0, fail_at }, expected, deadline())
                .is_err(),
            "{label}"
        );
        http.finish();
        let requests = http.requests.lock().unwrap();
        assert_eq!(requests.len(), before + 1, "{label}");
        assert_eq!(requests.last().unwrap().method, "GET");
    }
}

#[test]
fn absent_catalog_observation_does_not_create_catalog_custody() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("compliance-observe-absent");
    let provider = provider(&prepared);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    let path = operation(&publisher);
    let mut http = RuntimeHttp::start(&prepared, provider, &path, vec![]);
    let expectation = PromotedGeneratedCatalog {
        digest: [1; 32],
        sequence: 1,
        generated_at_unix: now_ms().unwrap() / 1_000,
        valid_until_unix: now_ms().unwrap() / 1_000 + 60,
    };
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), expectation, deadline())
            .is_err()
    );
    assert!(!path.join("catalogs").exists());
    assert!(http.requests.lock().unwrap().is_empty());
    http.finish();
}

#[test]
fn failed_group_keeps_original_anchors_for_two_durable_owner_promotions() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("compliance-group-retry");
    let providers = prepared
        .stream_token_authorities()
        .unwrap()
        .unwrap()
        .providers;
    let owners = providers
        .map(|source| ManagedGatewayCompliance::open(&prepared, source.provider_id).unwrap());
    let serve = |owner: &ManagedGatewayCompliance, steps| {
        RuntimeHttp::start_selected(
            &prepared,
            owner.authority.provider_id().unwrap(),
            &operation(owner),
            steps,
            true,
        )
    };
    let original = owners.each_ref().map(|owner| {
        let mut server = serve(owner, complete_script());
        let value = owner.advance(&mut TestLive::default(), deadline()).unwrap();
        server.finish();
        value
    });
    let advance = |owner: &ManagedGatewayCompliance| {
        // Resume the original signed successor after publication to custody but before dispatch.
        let prior = retained(&operation(owner), "original.nrt");
        let successor = owner
            .authority
            .sign_gateway_compliance_catalog(Some(&prior), now_ms().unwrap() / 1_000)
            .unwrap();
        retain_catalog(owner, 2, &successor);
        let mut server = serve(
            owner,
            vec![
                Step::Status(Observation::Promoted),
                Step::SuccessorMutation(Mutation::Stage),
                Step::Status(Observation::SuccessorCandidate),
                Step::SuccessorMutation(Mutation::Ack),
                Step::SuccessorMutation(Mutation::Promote),
                Step::Status(Observation::PromotedSuccessor),
            ],
        );
        let promoted = owner.advance(&mut TestLive::default(), deadline()).unwrap();
        assert_eq!(promoted, report(&successor).unwrap());
        server.finish();
        promoted
    };
    let observe = |slot: usize, status| {
        let owner = &owners[slot];
        let mut server = serve(owner, vec![Step::Status(status)]);
        let result = owner.recover_promoted(&mut TestLive::default(), original[slot], deadline());
        server.finish();
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        result
    };
    let first = advance(&owners[0]);
    assert_eq!(observe(0, Observation::PromotedSuccessor).unwrap(), first);
    assert!(observe(1, Observation::ForeignHead).is_err());
    // A's effect is durable. A failed set grants no new lease and its old catalog anchors stay put.
    let mut changed_anchor = original[0];
    changed_anchor.digest[0] ^= 1;
    assert!(
        owners[0]
            .recover_promoted(&mut TestLive::default(), changed_anchor, deadline())
            .is_err()
    );
    assert!(
        owners[0]
            .observe_promoted(&mut TestLive::default(), original[0], deadline())
            .is_err()
    );
    let second = advance(&owners[1]);
    let joined = |statuses: [Observation; 3], refuse_at: Option<usize>| {
        let mut servers = std::array::from_fn::<_, 3, _>(|index| {
            serve(&owners[index], vec![Step::Status(statuses[index])])
        });
        let mut phases = 0;
        let result = ManagedGatewayCompliance::recover_promoted_set(
            &owners,
            original,
            deadline(),
            |plans| {
                phases += 1;
                for (owner, plan) in owners.iter().zip(plans) {
                    assert_eq!(owner.plan.gateway_id(), plan.gateway_id());
                    assert_eq!(owner.plan.original_commitment(), plan.original_commitment());
                }
                if Some(phases) == refuse_at {
                    return Err(invalid("original child stopped during status"));
                }
                Ok(())
            },
        );
        for server in &mut servers {
            server.finish();
            let calls = server.requests.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "GET");
        }
        (result, phases)
    };
    let statuses = [
        Observation::PromotedSuccessor,
        Observation::PromotedSuccessor,
        Observation::Promoted,
    ];
    let (failed, phases) = joined(statuses, Some(3));
    assert!(failed.is_err());
    assert_eq!(phases, 3);
    let mut foreign = statuses;
    foreign[1] = Observation::ForeignHead;
    assert!(joined(foreign, None).0.is_err());
    let (resolved, phases) = joined(statuses, None);
    let resolved = resolved.unwrap();
    assert_eq!(phases, 4);
    assert_eq!(resolved, [first, second, original[2]]);
    resolved[0].require_writer_receipt(None).unwrap();
    resolved[1].require_writer_receipt(Some(second)).unwrap();
    assert!(
        resolved[1]
            .require_writer_receipt(Some(original[1]))
            .is_err()
    );
    assert!(resolved[1].require_writer_receipt(Some(first)).is_err());
    // A fresh serving response for the genuine successor cannot cross its signed exclusive expiry.
    let catalog: GatewayComplianceCatalogV1 =
        retained_at(&operation(&owners[0]), 2, "original.nrt");
    let status = status_fixture(
        Observation::PromotedSuccessor,
        &owners[0].plan,
        &operation(&owners[0]),
        first.valid_until_unix - 1,
    );
    assert!(
        owners[0]
            .require_fresh_promoted(&status, &catalog, first.valid_until_unix)
            .is_err()
    );
    assert!(
        owners[0]
            .recover_promoted(&mut TestLive::default(), original[0], Instant::now())
            .is_err()
    );
}
