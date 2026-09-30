#[test]
fn descriptor_and_terminal_known_answer_are_pinned() {
    let descriptor: [u8; 32] = Sha256::digest(ZK_X509_DER_STARK_AIR_DESCRIPTOR_V1).into();
    assert!(
        ZK_X509_DER_STARK_AIR_DESCRIPTOR_V1.ends_with(b":standalone-activation=not-applicable")
    );
    assert!(
        !ZK_X509_DER_STARK_AIR_DESCRIPTOR_V1
            .windows(b"pending".len())
            .any(|window| window == b"pending")
    );
    let nested = [
        0x30, 0x0a, 0x31, 0x04, 0x05, 0x00, 0x05, 0x00, 0x02, 0x02, 0x00, 0x80,
    ];
    let challenges = challenges();
    let base = build_zk_x509_der_stark_base_v1(&[&nested]).expect("base");
    let trace = build_zk_x509_der_stark_trace_v1(base, challenges).expect("trace");
    let terminals = zk_x509_der_stark_terminals_v1(&trace).expect("terminals");
    assert_eq!(descriptor, ZK_X509_DER_STARK_AIR_DESCRIPTOR_SHA256_V1);
    assert_eq!(
        &terminals.stack_push[..3],
        &[F(15_077_000), F(18_234_700), F(21_692_400)]
    );
    assert_eq!(terminals.stack_push, terminals.stack_pop);
    assert_eq!(&terminals.document[..3], &[F(14_024), F(15_424), F(16_824)]);
    assert_eq!(
        &terminals.node[..3],
        &[
            F(9_465_867_933_392_773_807),
            F(12_380_679_788_795_919_922),
            F(543_777_462_973_859_109),
        ]
    );
    assert_eq!(
        &terminals.pair_producer[..3],
        &[F(34_182), F(37_582), F(40_982)]
    );
    assert_eq!(terminals.pair_producer, terminals.pair_consumer);
    assert_eq!(
        &terminals.byte_table_sum[..3],
        &[
            F(16_661_283_889_178_025_548),
            F(7_361_935_896_858_379_781),
            F(6_554_425_153_307_073_952),
        ]
    );
    assert_eq!(terminals.byte_table_sum, terminals.byte_query_sum);
    assert_eq!(
        &terminals.input_byte[..3],
        &[
            F(17_868_045_749_908_660_777),
            F(13_559_048_740_293_346_376),
            F(15_933_393_656_887_798_260),
        ]
    );
    assert!(
        [
            terminals.stack_push[3],
            terminals.document[3],
            terminals.node[3],
            terminals.pair_producer[3],
            terminals.byte_table_sum[3],
            terminals.input_byte[3],
        ]
        .iter()
        .all(|value| *value != F::ZERO)
    );
}
