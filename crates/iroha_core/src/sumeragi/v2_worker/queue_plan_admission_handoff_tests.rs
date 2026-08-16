macro_rules! qp_rollover_case { ($($tokens:tt)*) => {{ $($tokens)* }}; }

fn queue_plan_rollover_fixture(
    service: &ProductionV2Services,
    certificate: Arc<Vec<u8>>,
) -> (PeerId, ExactOutputRolloverClaim, Vec<NetworkMessage>) {
    let view = 0;
    let target = service.context.roster[usize::try_from(service.context.leader(view)).unwrap()]
        .validator
        .clone();
    let claim = ExactOutputRolloverClaim::QueuePlanAdmission {
        scope: service.exact_output_scope(),
        target: target.clone(),
        view,
        certificate_hash: Hash::new(certificate.as_slice()),
    };
    (
        target,
        claim,
        vec![NetworkMessage::QueuePlanAdmissionCertificate(certificate)],
    )
}

fn queue_plan_reconstruct(
    service: &ProductionV2Services,
    artifact: &wire::finality::V2FinalityArtifact,
    messages: &[NetworkMessage],
    target: &PeerId,
    claim: &ExactOutputRolloverClaim,
) -> Result<(), String> {
    applied_height_reconstruction_covers(
        messages,
        std::slice::from_ref(target),
        claim,
        artifact,
        None,
        Some(service.kura.as_ref()),
    )
}

#[test]
fn queue_plan_rollover_binds_target_hash_and_frozen_view() {
    qp_rollover_case! { let (service, keys) = fixture(); let (_, artifact) = durable_finality_fixture(&service, &keys); let certificate = Arc::new(vec![0x51, 0x50, 0x41]); service.kura.persist_pending_queue_plan_admission_certificate(&certificate).expect("persist exact source"); let (target, claim, messages) = queue_plan_rollover_fixture(&service, certificate); assert_eq!(queue_plan_reconstruct(&service, &artifact, &messages, &target, &claim), Ok(())); let other = service.context.roster.iter().map(|entry| &entry.validator).find(|peer| *peer != &target).expect("another peer"); assert!(queue_plan_reconstruct(&service, &artifact, &messages, other, &claim).unwrap_err().contains("changed semantic identity")); let wrong_view = (1..=u64::try_from(service.context.roster.len()).unwrap()).find(|view| service.context.leader(*view) != service.context.leader(0)).unwrap(); let ExactOutputRolloverClaim::QueuePlanAdmission { scope, certificate_hash, .. } = claim else { unreachable!() }; let wrong = ExactOutputRolloverClaim::QueuePlanAdmission { scope, target: target.clone(), view: wrong_view, certificate_hash }; assert!(queue_plan_reconstruct(&service, &artifact, &messages, &target, &wrong).unwrap_err().contains("frozen view leader")); }
}

#[test]
fn queue_plan_rollover_requires_exact_kura_bytes() {
    qp_rollover_case! { let (service, keys) = fixture(); let (_, artifact) = durable_finality_fixture(&service, &keys); let certificate = Arc::new(vec![0x44, 0x55, 0x52, 0x41]); let (target, claim, messages) = queue_plan_rollover_fixture(&service, Arc::clone(&certificate)); assert!(applied_height_reconstruction_covers(&messages, std::slice::from_ref(&target), &claim, &artifact, None, None).unwrap_err().contains("Kura source")); service.kura.persist_pending_queue_plan_admission_certificate(b"different").expect("persist other source"); assert!(queue_plan_reconstruct(&service, &artifact, &messages, &target, &claim).unwrap_err().contains("exact durable Kura source")); service.kura.persist_pending_queue_plan_admission_certificate(&certificate).expect("persist exact source"); assert_eq!(queue_plan_reconstruct(&service, &artifact, &messages, &target, &claim), Ok(())); }
}
