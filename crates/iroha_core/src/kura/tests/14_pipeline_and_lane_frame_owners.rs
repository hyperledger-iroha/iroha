// Exact declared frame owners for the included Kura pipeline/lane artifact definitions.
// Included by kura::tests to reuse the established valid artifact fixtures.
fn assert_pipeline_artifact_frame_bytes<T>(value: &T) -> T
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("encode pipeline artifact owner");
    assert_eq!(frame[6..22], norito::schema::identity::frame_hash::<T>());
    let decoded = norito::decode_canonical::<T>(&frame).expect("decode pipeline artifact owner");
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode pipeline artifact"),
        frame
    );
    let mut wrong_owner = frame.clone();
    wrong_owner[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<T>(&wrong_owner),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame;
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    decoded
}

#[test]
fn pipeline_and_fastpq_owner_frames_preserve_recovery_metadata() {
    use crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1 as check;
    let block = NativeBlocks::new().next();
    let tx_hash = HashOf::from_untyped_unchecked(Hash::new(b"pipeline-frame-entrypoint"));
    let tx = PipelineTxSnapshot::compact(tx_hash, 3, 7);
    let decoded = assert_pipeline_artifact_frame_bytes(&tx);
    assert_eq!(
        (
            decoded.hash,
            decoded.reads,
            decoded.writes,
            decoded.read_count,
            decoded.write_count
        ),
        (
            tx.hash,
            tx.reads.clone(),
            tx.writes.clone(),
            tx.read_count,
            tx.write_count
        )
    );
    let proof = sample_fastpq_snapshot(1, block.hash(), 8);
    check(&proof, "iroha_core::kura::FastpqProofSnapshot");
    let mut sidecar = PipelineRecoverySidecar::new(
        1,
        block.hash(),
        PipelineDagSnapshot {
            fingerprint: [0x42; 32],
            key_count: 10,
        },
        vec![tx],
    );
    sidecar.fastpq_proofs.push(proof);
    let decoded = assert_pipeline_artifact_frame_bytes(&sidecar);
    assert_eq!(decoded.to_json_value(), sidecar.to_json_value());
    let frame = norito::encode_canonical(&sidecar).expect("encode pipeline sidecar");
    assert!(matches!(
        norito::decode_canonical::<PipelineTxSnapshot>(&frame),
        Err(norito::Error::SchemaMismatch)
    ));
}

#[test]
fn pipeline_artifact_frame_contracts_use_declared_owners() {
    fn check<T>(name: &str)
    where
        T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    {
        assert_eq!(T::nominal_name(), name);
        assert_eq!(T::frame_name(), name);
    }
    check::<FastpqProofSnapshot>("iroha_core::kura::FastpqProofSnapshot");
    check::<PipelineRecoverySidecar>("iroha_core::kura::PipelineRecoverySidecar");
    check::<PipelineTxSnapshot>("iroha_core::kura::PipelineTxSnapshot");
    check::<BoundProgressAppendIntentV1>("iroha_core::kura::BoundProgressAppendIntentV1");
}

fn frame_kura_test_payload<Owner, Payload>(current: &Owner, unsupported: &Payload) -> Vec<u8>
where
    Owner: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    Payload: norito::SerializePayload,
{
    // Adversarial layouts remain payload-only; the real current owner supplies
    // their envelope. A positive control checks this exact framing procedure.
    let (current_payload, current_flags, payload, flags) = {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (current_payload, current_flags) = norito::codec::encode_with_header_flags(current);
        let (payload, flags) = norito::codec::encode_with_header_flags(unsupported);
        (current_payload, current_flags, payload, flags)
    };
    let control =
        norito::core::frame_bare_with_header_flags::<Owner>(&current_payload, current_flags)
            .expect("frame current Kura owner control");
    assert_eq!(
        control,
        norito::encode_canonical(current).expect("encode current owner")
    );
    let decoded = norito::decode_canonical::<Owner>(&control)
        .expect("current payload roundtrips through the same framing procedure");
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode control"),
        control
    );
    let frame = norito::core::frame_bare_with_header_flags::<Owner>(&payload, flags)
        .expect("frame adversarial Kura payload under current owner");
    let view = norito::core::from_bytes_view(&frame)
        .expect("adversarial frame has a valid length, header and checksum");
    assert_eq!(
        view.schema(),
        norito::schema::identity::frame_hash::<Owner>()
    );
    assert_eq!(view.as_bytes(), payload.as_slice());
    frame
}

fn assert_kura_test_payload_rejected<Owner>(frame: &[u8])
where
    Owner: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let error = match norito::decode_canonical::<Owner>(frame) {
        Ok(_) => panic!("unsupported Kura payload decoded as the current layout"),
        Err(error) => error,
    };
    assert!(
        !matches!(error, norito::Error::SchemaMismatch),
        "the negative control must reach payload decoding under its actual owner"
    );
}

#[test]
fn progress_sidecar_test_frame_has_its_own_current_identity() {
    crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1(
        &DummySidecar { height: 7 },
        "iroha_core::kura::tests::DummySidecar",
    );
}
