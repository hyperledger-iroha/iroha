#[test]
fn rfc5280_io_is_exact_and_uses_attribute_contents_at_256_byte_boundary() {
    let common_name = vec![b'A'; ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1];
    let encoded_common_name = tlv(&[0x0c], &common_name);
    assert_eq!(encoded_common_name.len(), 260);
    assert_eq!(
        fixed_padded_v1(&encoded_common_name, ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1),
        Err(ZkX509DerAirErrorV1::Resource)
    );
    let (chain, crl, statement) = rfc5280_fixture_with_leaf_common_name(2, &[10, 11], &common_name);
    let trace = build_zk_x509_rfc5280_trace_v1(&chain, &crl, statement).expect("boundary trace");
    assert_eq!(
        trace.certificates[0].subject.attributes[3].as_deref(),
        Some(common_name.as_slice())
    );
    let witnesses = rfc5280_io_witnesses_v1(&trace, 0).expect("DER I/O witnesses");
    // Three fixed SPKI slots, serial length/value, country length/value,
    // then CN length/value.
    assert_eq!(witnesses[7].producer_value, 256_u64.to_be_bytes());
    assert_eq!(witnesses[8].producer_value, common_name);
    assert_eq!(witnesses[2].producer_value, vec![0; 91]);
    assert_eq!(witnesses[15].producer_value, vec![0]);
    assert_eq!(witnesses[22].producer_value, vec![0; 72]);
    assert_eq!(witnesses[23].producer_value, 0_u64.to_be_bytes());
    assert_eq!(witnesses[24].producer_value, vec![0; 65]);
    assert_eq!(
        witnesses[18].producer_value,
        trace.certificates[1].public_key
    );
    assert_eq!(
        witnesses[21].producer_value,
        trace.certificates[1].public_key
    );
    let crl_key_channel = &witnesses[witnesses.len() - 4];
    assert_eq!(
        crl_key_channel.producer_value,
        trace.certificates[1].public_key
    );
    let wallet_key_channel = &witnesses[witnesses.len() - 3];
    assert_eq!(
        wallet_key_channel.producer_value,
        trace.certificates[0].public_key
    );
    let issuer_spki = &trace.certificates[1].spki_der;
    let root_spki = &trace.certificates.last().expect("root").spki_der;
    let issuer_channel = &witnesses[witnesses.len() - 2];
    assert_eq!(issuer_channel.producer_value, *issuer_spki);
    assert_eq!(
        issuer_channel.declaration.consumers,
        vec![ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::Sha256,
            instance: 0,
        }]
    );
    let root_channel = witnesses.last().expect("root SPKI channel");
    assert_eq!(root_channel.producer_value, *root_spki);
    assert_eq!(
        root_channel.declaration.consumers,
        vec![ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::CaAccumulator,
            instance: 0,
        }]
    );
    let io = build_zk_x509_io_trace_v1(&witnesses, io_challenges()).expect("global I/O");
    validate_rfc5280_io_v1(&trace, &io, 0).expect("DER I/O binding");
    let canonical_witnesses = witnesses.clone();
    let (depth_three_chain, depth_three_crl, depth_three_statement) = rfc5280_fixture(3, &[10, 11]);
    let depth_three_trace =
        build_zk_x509_rfc5280_trace_v1(&depth_three_chain, &depth_three_crl, depth_three_statement)
            .expect("depth-three trace");
    let depth_three_witnesses =
        rfc5280_io_witnesses_v1(&depth_three_trace, 0).expect("depth-three I/O");
    assert_eq!(depth_three_witnesses[15].producer_value, vec![1]);
    assert_ne!(depth_three_witnesses[22].producer_value, vec![0; 72]);
    assert_ne!(depth_three_witnesses[24].producer_value, vec![0; 65]);
    assert_eq!(
        depth_three_witnesses[18].producer_value,
        depth_three_trace.certificates[1].public_key
    );
    assert_eq!(
        depth_three_witnesses[21].producer_value,
        depth_three_trace.certificates[2].public_key
    );
    assert_eq!(
        depth_three_witnesses[24].producer_value,
        depth_three_trace.certificates[2].public_key
    );
    let mut mismatched = witnesses.clone();
    mismatched[8].producer_value[0] ^= 1;
    mismatched[8].consumer_values[0][0] ^= 1;
    let mismatched_io =
        build_zk_x509_io_trace_v1(&mismatched, io_challenges()).expect("self-consistent I/O");
    assert_eq!(
        validate_rfc5280_io_v1(&trace, &mismatched_io, 0),
        Err(ZkX509DerAirErrorV1::ByteBinding)
    );
    let mut unequal_endpoints = witnesses;
    unequal_endpoints[7].consumer_values[0][0] ^= 1;
    assert!(build_zk_x509_io_trace_v1(&unequal_endpoints, io_challenges()).is_err());
    let reject_topology = |label: &str, changed: Vec<ZkX509IoChannelWitnessV1>| {
        if let Ok(changed_io) = build_zk_x509_io_trace_v1(&changed, io_challenges()) {
            assert!(
                validate_rfc5280_io_v1(&trace, &changed_io, 0).is_err(),
                "self-consistent but noncanonical I/O topology {label}"
            );
        }
    };
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.channel += 1;
    reject_topology("channel", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.producer.role = ZkX509IoSegmentRoleV1::Sha256;
    reject_topology("producer role", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.producer.instance += 1;
    reject_topology("producer instance", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.consumers[0].role = ZkX509IoSegmentRoleV1::Sha256;
    reject_topology("consumer role", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.consumers[0].instance += 1;
    reject_topology("consumer instance", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.byte_len += 1;
    changed[0].producer_value.push(0);
    changed[0].consumer_values[0].push(0);
    reject_topology("byte length", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.consumers[0].role = ZkX509IoSegmentRoleV1::PublicInput;
    changed[0].declaration.public_value = Some(changed[0].producer_value.clone());
    reject_topology("public consumer and value", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.public_value = Some(changed[0].producer_value.clone());
    reject_topology("public value without public endpoint", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].declaration.consumers.clear();
    changed[0].consumer_values.clear();
    reject_topology("missing consumer", changed);
    let mut changed = canonical_witnesses.clone();
    let duplicate_consumer = changed[0].declaration.consumers[0];
    let duplicate_value = changed[0].consumer_values[0].clone();
    changed[0].declaration.consumers.push(duplicate_consumer);
    changed[0].consumer_values.push(duplicate_value);
    reject_topology("duplicate consumer", changed);
    let mut changed = canonical_witnesses.clone();
    let sha_consumer = ZkX509IoEndpointV1 {
        role: ZkX509IoSegmentRoleV1::Sha256,
        instance: 0,
    };
    let extra_consumer_value = changed[0].producer_value.clone();
    changed[0].declaration.consumers.insert(0, sha_consumer);
    changed[0].consumer_values.insert(0, extra_consumer_value);
    reject_topology("extra canonical consumer", changed);
    let mut changed = canonical_witnesses.clone();
    changed.swap(0, 1);
    reject_topology("channel reorder", changed);
    let mut changed = canonical_witnesses.clone();
    changed.pop();
    reject_topology("channel omission", changed);
    let mut changed = canonical_witnesses.clone();
    changed.remove(0);
    reject_topology("leading channel omission", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].producer_value.pop();
    reject_topology("short producer value", changed);
    let mut changed = canonical_witnesses.clone();
    changed[0].consumer_values[0].pop();
    reject_topology("short consumer value", changed);
    let declaration_mutations: [fn(&mut ZkX509IoTraceV1); 8] = [
        |value| value.declarations[0].channel += 1,
        |value| value.declarations[0].producer.role = ZkX509IoSegmentRoleV1::Sha256,
        |value| value.declarations[0].producer.instance += 1,
        |value| value.declarations[0].consumers[0].role = ZkX509IoSegmentRoleV1::Sha256,
        |value| value.declarations[0].consumers[0].instance += 1,
        |value| value.declarations[0].byte_len += 1,
        |value| value.declarations[0].public_value = Some(vec![0]),
        |value| {
            value.declarations.pop();
        },
    ];
    for (index, mutate) in declaration_mutations.into_iter().enumerate() {
        let mut changed_io = io.clone();
        mutate(&mut changed_io);
        assert!(
            validate_rfc5280_io_v1(&trace, &changed_io, 0).is_err(),
            "I/O declaration mutation family {index}"
        );
    }
    let mut reordered_io = io;
    reordered_io.declarations.swap(0, 1);
    assert!(validate_rfc5280_io_v1(&trace, &reordered_io, 0).is_err());
}
