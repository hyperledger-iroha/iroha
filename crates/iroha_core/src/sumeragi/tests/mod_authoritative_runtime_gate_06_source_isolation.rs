#[test]
fn fair_v2_ingress_wire_index_keeps_non_validator_relay_origins_distinct() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle_with_source_geometry(5, Some(1));
    let authenticated_via = validator_peers(1)
        .pop()
        .expect("authenticated relay fixture");
    let origin_a = PeerId::from(KeyPair::random().public_key().clone());
    let origin_b = PeerId::from(KeyPair::random().public_key().clone());
    let message = v2_message();
    for origin in [origin_a, origin_b] {
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_transport(
                message.clone(),
                origin,
                authenticated_via.clone(),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
    }
    assert_eq!(
        ingress.len(),
        2,
        "one authenticated relay lane preserves distinct semantic request origins"
    );
}
#[test]
fn fair_v2_ingress_wire_index_keeps_authenticated_origins_distinct() {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle_with_source_geometry(7, Some(2));
    let outsiders = validator_peers(2);
    let message = v2_message();
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            message.clone(),
            outsiders[0].clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            message,
            outsiders[1].clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(
        ingress.len(),
        2,
        "equal wire bytes from independent authenticated sources retain distinct semantic request origins"
    );
}
#[test]
fn fair_v2_ingress_byte_quota_isolates_validator_sources() {
    let validators = validator_peers(2);
    let first = v2_message_with_bytes(0, 64);
    let second = v2_message_with_bytes(1, 64);
    let encoded_len = encoded_v2_len(&first);
    assert_eq!(encoded_v2_len(&second), encoded_len);
    let source_capacity = encoded_len.checked_add(1).expect("ordinary byte partition");
    let ingress =
        super::FairV2Ingress::new(12, source_capacity * 2, source_capacity, 0, encoded_len);
    ingress
        .configure_roster(validators.clone())
        .expect("two validator byte partitions fit exactly");
    ingress.open().expect("open configured roster");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            first,
            validators[0].clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            second,
            validators[0].clone(),
        )),
        Err(super::FairV2IngressPushError::Full(_))
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            v2_message_with_bytes(2, 64),
            validators[1].clone(),
        )),
        Ok(super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert_eq!(
        ingress.len(),
        2,
        "one validator's byte pressure cannot consume another validator's quota"
    );
}
