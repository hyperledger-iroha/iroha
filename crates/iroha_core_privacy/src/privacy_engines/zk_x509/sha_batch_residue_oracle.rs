// Preserved batch relation body for independent test comparison.

fn original_evaluate_sha_batch_relation_v1<A: PolynomialAirFieldV1>(
    current: &ZkX509ShaBatchRowV1<A>,
    next: &ZkX509ShaBatchRowV1<A>,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    segment: u8,
    ca_call_boundaries: Option<&[ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1]>,
) -> Result<Vec<A>, ZkX509ShaCallBusStarkErrorV1> {
    call_challenges.validate()?;
    rfc_challenges
        .validate()
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    if current
        .base
        .iter()
        .chain(&next.base)
        .chain(&current.aux)
        .chain(&next.aux)
        .chain(&current.fixed)
        .chain(&next.fixed)
        .any(|value| !value.is_canonical())
    {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    if usize::from(segment) >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Terminal);
    }
    if let Some(boundaries) = ca_call_boundaries {
        for (index, boundary) in boundaries.iter().copied().enumerate() {
            boundary.validate_identity_v1(index)?;
        }
    }
    let current_word_base: &[A; SHA_WORD_CAPACITY_BASE_WIDTH_V1] = current.base
        [..SHA_WORD_CAPACITY_BASE_WIDTH_V1]
        .try_into()
        .expect("word base prefix");
    let next_word_base: &[A; SHA_WORD_CAPACITY_BASE_WIDTH_V1] = next.base
        [..SHA_WORD_CAPACITY_BASE_WIDTH_V1]
        .try_into()
        .expect("word base prefix");
    let current_word_aux: &[A; SHA_WORD_CAPACITY_AUX_WIDTH_V1] = current.aux
        [..SHA_WORD_CAPACITY_AUX_WIDTH_V1]
        .try_into()
        .expect("word aux prefix");
    let next_word_aux: &[A; SHA_WORD_CAPACITY_AUX_WIDTH_V1] = next.aux
        [..SHA_WORD_CAPACITY_AUX_WIDTH_V1]
        .try_into()
        .expect("word aux prefix");
    let word_fixed: &[A; SHA_WORD_CAPACITY_FIXED_WIDTH_V1] = current.fixed
        [..SHA_WORD_CAPACITY_FIXED_WIDTH_V1]
        .try_into()
        .expect("word fixed prefix");
    let mut residues = super::super::sha_word_stark::residue_oracles::original_evaluate_zk_x509_sha_word_capacity_residues_over_field_v1(
        current_word_base,
        next_word_base,
        current_word_aux,
        next_word_aux,
        word_fixed,
        word_challenges,
    )?;
    for bit in 0..ZK_X509_SHA_RFC_LENGTH_BITS_V1 {
        let value = current.base[ZK_X509_SHA_LENGTH_BITS_START_V1 + bit];
        residues.push(value.mul(value.sub(A::ONE)));
    }
    residues.push(sha_rfc_length_recomposition_residue_v1(
        &current.base,
        &current.aux,
        &current.fixed,
    ));
    let input_event = current.fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1]
        .mul(current.base[SHA_WORD_CAPACITY_ROW_ACTIVE_V1]);
    let digest_event = current.fixed[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1];
    let segment_last = current.fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1];
    let segment_first = current.fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1];
    let padding = current.fixed[ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1];
    // The native domain is cyclic: its final zero padding row precedes the
    // first live row, whose products start at one. The public schedule makes
    // terminal and padding selectors disjoint, so this linear gate preserves
    // live recurrences without joining either boundary to zero padding.
    let continue_products = A::ONE.sub(segment_last).sub(padding);
    let manifest_call = current.fixed[ZK_X509_SHA_FIXED_CALL_V1];
    let manifest_role = current.fixed[ZK_X509_SHA_FIXED_ROLE_V1];
    let manifest_slot = current.fixed[ZK_X509_SHA_FIXED_SLOT_V1];
    let input_word = current.fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1];
    let digest_word = current.fixed[SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1];
    for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
        let challenge = call_challenges.lanes[lane];
        let input_factor = compress_sha_call_fields_v1(
            manifest_call,
            manifest_role,
            manifest_slot,
            A::from_base(F(u64::from(ZkX509ShaCallWordKindV1::Input.code()))),
            input_word,
            current.base[0],
            challenge,
        );
        let digest_factor = compress_sha_call_fields_v1(
            manifest_call,
            manifest_role,
            manifest_slot,
            A::from_base(F(u64::from(ZkX509ShaCallWordKindV1::Digest.code()))),
            digest_word,
            current.base[0],
            challenge,
        );
        let input_before = current.aux[ZK_X509_SHA_INPUT_PRODUCTS_V1 + lane];
        let input_after = input_before.mul(A::ONE.add(input_event.mul(input_factor.sub(A::ONE))));
        let digest_before = current.aux[ZK_X509_SHA_DIGEST_PRODUCTS_V1 + lane];
        let digest_after =
            digest_before.mul(A::ONE.add(digest_event.mul(digest_factor.sub(A::ONE))));
        residues.push(
            continue_products.mul(next.aux[ZK_X509_SHA_INPUT_PRODUCTS_V1 + lane].sub(input_after)),
        );
        residues.push(
            continue_products
                .mul(next.aux[ZK_X509_SHA_DIGEST_PRODUCTS_V1 + lane].sub(digest_after)),
        );
        residues.push(segment_first.mul(input_before.sub(A::ONE)));
        residues.push(segment_first.mul(digest_before.sub(A::ONE)));
        // Whole-segment source/digest totals have no external consumer. Their
        // initial-one and live recurrence constraints remain, and compact-CA
        // boundaries below bind the actual call-local products without division.
        if let Some(boundaries) = ca_call_boundaries {
            for (boundary_index, boundary) in boundaries.iter().copied().enumerate() {
                let selector =
                    current.fixed[ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1 + boundary_index];
                let call_first = current.fixed[SHA_WORD_CAPACITY_CALL_FIRST_V1].mul(selector);
                let call_last = current.fixed[SHA_WORD_CAPACITY_CALL_LAST_V1].mul(selector);
                residues.push(
                    call_first
                        .mul(input_before.sub(A::from_base(boundary.source_start_products[lane]))),
                );
                residues
                    .push(call_first.mul(
                        digest_before.sub(A::from_base(boundary.digest_start_products[lane])),
                    ));
                residues.push(call_last.mul(input_after.sub(A::from_base(
                    boundary.source_start_products[lane].mul(boundary.source_products[lane]),
                ))));
                residues.push(call_last.mul(digest_after.sub(A::from_base(
                    boundary.digest_start_products[lane].mul(boundary.digest_products[lane]),
                ))));
            }
        }
    }
    for stream in 0..ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
        for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
            let before = current.aux
                [ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1 + stream * ZK_X509_SHA_BUS_LANES_V1 + lane];
            let after = before.mul(A::ONE.add(sha_rfc_opened_event_delta_v1(
                &current.base,
                &current.fixed,
                stream,
                lane,
                rfc_challenges,
            )?));
            residues.push(
                continue_products.mul(
                    next.aux[ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1
                        + stream * ZK_X509_SHA_BUS_LANES_V1
                        + lane]
                        .sub(after),
                ),
            );
            residues.push(segment_first.mul(before.sub(A::ONE)));
        }
    }
    residues.extend(current.base.map(|value| padding.mul(value)));
    residues.extend(current.aux.map(|value| padding.mul(value)));
    let expected_count = if ca_call_boundaries.is_some() {
        ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1
    } else {
        ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1
    };
    if residues.len() != expected_count {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    Ok(residues)
}
