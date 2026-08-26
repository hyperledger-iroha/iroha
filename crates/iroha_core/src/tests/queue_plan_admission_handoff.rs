macro_rules! qp_case { ($($tokens:tt)*) => {{ $($tokens)* }}; }

#[test]
fn queue_plan_admission_handoff_uses_bounded_reliable_consensus_decode() {
    qp_case! { let max_body = iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES; let message = NetworkMessage::QueuePlanAdmissionCertificate(Arc::new(vec![0xA5; max_body])); let encoded = norito::to_bytes(&message).expect("encode maximum admission handoff"); assert_eq!(raw_network_tag(&message), 17); assert_eq!(message.topic(), NetworkTopic::Consensus); assert_eq!(raw_network_topic(&message), NetworkTopic::Consensus); assert_eq!(message.subscriber_route(), SubscriberRoute::General); assert_eq!(message.progress_reconstruction(), iroha_p2p::network::message::ProgressReconstruction::Retransmit); assert!(iroha_p2p::network::reliable_progress_class(message.topic(), message.subscriber_route()).is_some()); assert!(encoded.len() <= crate::MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES); let view = ncore::from_bytes_view(&encoded).expect("inspect maximum admission handoff"); let limits = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(view.as_bytes(), encoded.len(), view.flags()).expect("select admission handoff decode policy").expect("tag 17 installs decode limits"); let decoded = ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits).expect("maximum admission bytes fit bounded decode"); assert!(matches!(decoded, NetworkMessage::QueuePlanAdmissionCertificate(bytes) if bytes.len() == max_body)); }
}

#[test]
fn oversized_queue_plan_admission_handoff_hits_frame_cap() {
    qp_case! { let message = NetworkMessage::QueuePlanAdmissionCertificate(Arc::new(vec![0xA5; crate::MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES])); let encoded = norito::to_bytes(&message).expect("encode oversized admission handoff"); let view = ncore::from_bytes_view(&encoded).expect("inspect oversized admission handoff"); let error = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(view.as_bytes(), encoded.len(), view.flags()).expect_err("outer cap rejects oversized handoff"); assert!(matches!(error, ncore::Error::ArchiveLengthExceeded { limit, .. } if limit == crate::MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES as u64)); }
}

#[test]
fn queue_plan_admission_handoff_rejects_body_over_certificate_cap() {
    qp_case! { let max_body = iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES; let message = NetworkMessage::QueuePlanAdmissionCertificate(Arc::new(vec![0xA5; max_body + 1])); let encoded = norito::to_bytes(&message).expect("encode overlong admission body"); assert!(encoded.len() <= crate::MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES); let view = ncore::from_bytes_view(&encoded).expect("inspect overlong admission body"); let limits = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(view.as_bytes(), encoded.len(), view.flags()).expect("outer frame fits").expect("tag 17 installs decode limits"); assert!(ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits).is_err()); }
}
