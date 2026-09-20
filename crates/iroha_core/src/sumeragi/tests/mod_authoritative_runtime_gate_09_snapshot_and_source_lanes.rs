#[test]
fn fair_v2_ingress_snapshot_tracks_live_depth_and_oldest_age() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(12);
    let validators = validator_peers(2);
    ingress.close();
    ingress
        .configure_roster(validators.clone())
        .expect("two validators and their protected slots fit");
    ingress.open().expect("open configured roster");
    let captured_at = Instant::now();
    ingress
        .try_push_at(
            InboundBlockMessage::from_authenticated_peer(v2_message(), validators[0].clone()),
            captured_at - Duration::from_secs(5),
        )
        .expect("enqueue oldest validator message");
    ingress
        .try_push_at(
            InboundBlockMessage::from_authenticated_peer(v2_message(), validators[1].clone()),
            captured_at - Duration::from_secs(2),
        )
        .expect("enqueue newer validator message");
    ingress
        .try_push_at(
            InboundBlockMessage::from_authenticated_peer(
                v2_message_with_index(1),
                validators[0].clone(),
            ),
            captured_at - Duration::from_secs(7),
        )
        .expect("enqueue timestamp-inverted message behind the source head");
    assert_eq!(
        ingress.snapshot_at(captured_at),
        super::FairV2IngressSnapshot {
            depth: 3,
            capacity: 12,
            oldest_age: Some(Duration::from_secs(7)),
            service_idle_age: Some(Duration::from_secs(5)),
        }
    );
    ingress.close();
    assert!(ingress.ensure_closed_drained_cut().is_err());
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            v2_message_with_index(2),
            validators[1].clone(),
        )),
        Err(super::FairV2IngressPushError::Closed(_))
    ));
    let mut first = ingress
        .try_recv_if_at_checked(captured_at, |_| true)
        .expect("checked closed-prefix dequeue")
        .expect("drain first fair source head");
    assert_eq!(first.sender(), &validators[0]);
    let first_owner = first
        .take_ingress_ownership()
        .expect("first occurrence owner");
    assert!(first_owner.validate_exact());
    assert_eq!(first_owner.physical_admission_ordinal(), Some(1));
    assert_eq!(first_owner.runtime_physical_cut(), Some(4));
    assert!(ingress.ensure_closed_drained_cut().is_err());
    assert_eq!(
        ingress.snapshot_at(captured_at),
        super::FairV2IngressSnapshot {
            depth: 2,
            capacity: 12,
            oldest_age: Some(Duration::from_secs(7)),
            service_idle_age: Some(Duration::ZERO),
        }
    );
    let mut second = ingress
        .try_recv_if_at_checked(captured_at, |_| true)
        .expect("checked closed-prefix dequeue")
        .expect("drain second fair source head");
    assert_eq!(second.sender(), &validators[1]);
    let second_owner = second
        .take_ingress_ownership()
        .expect("second occurrence owner");
    assert!(second_owner.validate_exact());
    assert_eq!(second_owner.physical_admission_ordinal(), Some(2));
    assert_eq!(second_owner.runtime_physical_cut(), Some(4));
    assert!(ingress.ensure_closed_drained_cut().is_err());
    assert_eq!(
        ingress.snapshot_at(captured_at),
        super::FairV2IngressSnapshot {
            depth: 1,
            capacity: 12,
            oldest_age: Some(Duration::from_secs(7)),
            service_idle_age: Some(Duration::ZERO),
        }
    );
    let mut third = ingress
        .try_recv_if_at_checked(captured_at, |_| true)
        .expect("checked closed-prefix dequeue")
        .expect("drain remaining fair source");
    assert_eq!(third.sender(), &validators[0]);
    let third_owner = third
        .take_ingress_ownership()
        .expect("third occurrence owner");
    assert!(third_owner.validate_exact());
    assert_eq!(third_owner.physical_admission_ordinal(), Some(3));
    assert_eq!(third_owner.runtime_physical_cut(), Some(4));
    assert_eq!(
        ingress.snapshot_at(captured_at),
        super::FairV2IngressSnapshot {
            depth: 0,
            capacity: 12,
            oldest_age: None,
            service_idle_age: None,
        }
    );
    assert!(
        ingress
            .try_recv_if_at_checked(captured_at, |_| true)
            .unwrap()
            .is_none()
    );
    ingress
        .ensure_closed_drained_cut()
        .expect("all three exact closed-prefix occurrences retired");
}
#[test]
fn fair_v2_ingress_service_idle_age_tracks_scans_not_oldest_item_age() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
    let validator = validator_peers(1).pop().expect("validator fixture");
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("validator lane fits");
    ingress.open().expect("open configured roster");
    let captured_at = Instant::now();
    ingress
        .try_push_at(
            InboundBlockMessage::from_authenticated_peer(
                v2_auxiliary_prepare(0),
                validator.clone(),
            ),
            captured_at - Duration::from_secs(5),
        )
        .expect("enqueue old blocked entry");
    ingress
        .try_push_at(
            InboundBlockMessage::from_authenticated_peer(v2_auxiliary_prepare(1), validator),
            captured_at - Duration::from_secs(1),
        )
        .expect("enqueue later admissible entry");
    assert!(
        ingress.try_recv_if_at(captured_at, |_| false).is_none(),
        "a rejected scan must retain both entries"
    );
    assert_eq!(
        ingress.snapshot_at(captured_at + Duration::from_secs(2)),
        super::FairV2IngressSnapshot {
            depth: 2,
            capacity: 8,
            oldest_age: Some(Duration::from_secs(7)),
            service_idle_age: Some(Duration::from_secs(2)),
        }
    );
    let selected = ingress
        .try_recv_if_at(captured_at + Duration::from_secs(2), |inbound| {
            vote_height(inbound) == Some(2)
        })
        .expect("later admissible entry bypasses the old blocked entry");
    assert_eq!(vote_height(&selected), Some(2));
    assert_eq!(
        ingress.snapshot_at(captured_at + Duration::from_secs(3)),
        super::FairV2IngressSnapshot {
            depth: 1,
            capacity: 8,
            oldest_age: Some(Duration::from_secs(8)),
            service_idle_age: Some(Duration::from_secs(1)),
        },
        "successful fair service refreshes the scan clock without hiding old ownership"
    );
}
#[test]
fn fair_v2_ingress_empty_to_nonempty_resets_service_idle_baseline() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(2);
    let sender = authenticated_peer_for_test();
    ingress.close();
    ingress
        .configure_roster(std::iter::empty())
        .expect("empty authenticated roster fits");
    ingress.open().expect("open configured roster");
    let captured_at = Instant::now();
    assert!(ingress.try_recv_if_at(captured_at, |_| true).is_none());
    ingress
        .try_push_at(
            InboundBlockMessage::from_authenticated_peer(v2_auxiliary_prepare(0), sender.clone()),
            captured_at + Duration::from_secs(5),
        )
        .expect("enqueue after an empty-queue scan");
    assert_eq!(
        ingress
            .snapshot_at(captured_at + Duration::from_secs(6))
            .service_idle_age,
        Some(Duration::from_secs(1)),
        "an empty-queue scan must not make later ownership look serviced"
    );
    let _ = ingress
        .try_recv_if_at(captured_at + Duration::from_secs(7), |_| true)
        .expect("drain the first ownership interval");
    assert_eq!(
        ingress
            .snapshot_at(captured_at + Duration::from_secs(8))
            .service_idle_age,
        None
    );
    ingress
        .try_push_at(
            InboundBlockMessage::from_authenticated_peer(v2_auxiliary_prepare(1), sender),
            captured_at + Duration::from_secs(10),
        )
        .expect("enqueue a fresh ownership interval");
    assert_eq!(
        ingress
            .snapshot_at(captured_at + Duration::from_secs(11))
            .service_idle_age,
        Some(Duration::from_secs(1)),
        "the prior ownership interval's service time must not mask a fresh stall"
    );
}
#[test]
fn authenticated_non_validator_sources_use_distinct_bounded_lanes() {
    const SOURCE_BYTES: usize = 1024 * 1024;
    let ingress = super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
        29,
        7 * SOURCE_BYTES,
        SOURCE_BYTES,
        0,
        0,
        0,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        Some(3),
    );
    let validators = validator_peers(4);
    let outsiders = validator_peers(9).split_off(4);
    ingress
        .configure_roster(validators.clone())
        .expect("four validators and three authenticated non-validator sources fit");
    ingress.open().expect("open configured roster");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            v2_auxiliary_prepare(0),
            outsiders[0].clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    let relayed = |index, via: PeerId| {
        InboundBlockMessage::from_transport(v2_auxiliary_prepare(index), validators[0].clone(), via)
    };
    assert!(matches!(
        ingress.try_push(relayed(1, outsiders[1].clone())),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(relayed(2, outsiders[2].clone())),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(relayed(3, outsiders[3].clone())),
        Err(super::FairV2IngressPushError::Full(_))
    ));
    {
        let state = ingress.state.lock();
        assert!(
            state
                .lanes
                .contains_key(&super::FairV2IngressSource::Authenticated(
                    outsiders[0].clone()
                ))
        );
        assert!(
            state
                .lanes
                .contains_key(&super::FairV2IngressSource::Authenticated(
                    outsiders[2].clone()
                ))
        );
    }
    let first_authenticated = ingress
        .try_recv_if(|inbound| inbound.via() == &outsiders[0])
        .expect("the first authenticated owner remains independently serviceable");
    assert_eq!(first_authenticated.sender(), &outsiders[0]);
    assert!(matches!(
        ingress.try_push(relayed(3, outsiders[3].clone())),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
}
