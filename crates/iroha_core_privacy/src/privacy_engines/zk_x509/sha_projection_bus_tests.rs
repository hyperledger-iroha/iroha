// Included by SHA tests; compares original Projection and actual SHA producers.

fn projection_message_product_for_test_v1(
    invocation: usize,
    message: &[u8],
    digest: &[u8; 32],
    lanes: ZkX509ShaCallBusChallengesV1,
) -> [F; 4] {
    use super::super::projection_air::hash_bus;
    assert!(message.len() <= 2048);
    let mut words = [0_u8; 2048];
    words[..message.len()].copy_from_slice(message);
    let mut result = [F::ONE; 4];
    for lane in 0..4 {
        let mut absorb = |kind, index, value| {
            result[lane] = result[lane].mul(hash_bus::factor_v1(
                F(invocation as u64),
                F(kind),
                F(index),
                F(value),
                lanes.lanes[lane],
            ));
        };
        for (index, bytes) in words.as_chunks::<4>().0.iter().enumerate() {
            absorb(3, index as u64, u64::from(u32::from_be_bytes(*bytes)));
        }
        absorb(4, 0, message.len() as u64);
        for (index, bytes) in digest.as_chunks::<4>().0.iter().enumerate() {
            absorb(1, index as u64, u64::from(u32::from_be_bytes(*bytes)));
        }
    }
    result
}

#[test]
fn projection_sha_typed_products_bind_length_call_order_and_zero_collisions() {
    use super::super::projection_air::hash_bus;
    let lanes = challenges();
    for len in [0, 1, 2, 3, 4, 31, 32, 55, 56, 63, 64, 65, 2047, 2048] {
        let message: Vec<_> = (0..len).map(|i| (i * 17 + 3) as u8).collect();
        let digest: [u8; 32] = Sha256::digest(&message).into();
        let honest = projection_message_product_for_test_v1(0, &message, &digest, lanes);
        assert_ne!(
            honest,
            projection_message_product_for_test_v1(1, &message, &digest, lanes)
        );
        let mut wrong_digest = digest;
        wrong_digest[0] ^= 1;
        assert_ne!(
            honest,
            projection_message_product_for_test_v1(0, &message, &wrong_digest, lanes)
        );
        if len < 2048 {
            let mut longer = message.clone();
            longer.push(0);
            // Identical padded words still differ through the exact length event.
            assert_ne!(
                honest,
                projection_message_product_for_test_v1(0, &longer, &digest, lanes)
            );
        }
        if len > 0 {
            let mut changed = message.clone();
            changed[len - 1] ^= 1;
            let changed_digest = Sha256::digest(&changed).into();
            assert_ne!(
                honest,
                projection_message_product_for_test_v1(0, &changed, &changed_digest, lanes)
            );
        }
    }
    let factor = |lane| hash_bus::factor_v1(F::ZERO, F(3), F::ZERO, F(0x01020304), lane);
    let mut collision = lanes;
    collision.lanes[0].terms[0] = collision.lanes[0].terms[0].sub(factor(collision.lanes[0]));
    collision.validate().unwrap();
    assert_eq!(factor(collision.lanes[0]), F::ZERO);
    let message = [1, 2, 3, 4];
    let digest = Sha256::digest(message).into();
    let products = projection_message_product_for_test_v1(0, &message, &digest, collision);
    assert_eq!(products[0], F::ZERO);
    let mut changed_digest = digest;
    changed_digest[5] ^= 1;
    let changed = projection_message_product_for_test_v1(0, &message, &changed_digest, collision);
    // A shared zero factor is explicitly a bad compression challenge. No
    // inversion or private cancellation rejects the honest zero product.
    assert_eq!(changed[0], products[0]);
    assert!(changed[1..].iter().zip(&products[1..]).any(|(a, b)| a != b));
}

#[test]
#[ignore = "actual credential Projection and seven native SHA producers; root controlled native queue"]
fn projection_sha_products_match_actual_credentials_and_rehashed_substitution_fails() {
    use super::super::{
        main_assembly::build_zk_x509_main_trace_assembly_v1,
        projection_air::{
            build_zk_x509_projection_aux_trace_v1, compile_zk_x509_projection_stark_fixed_rows_v1,
            evaluate_zk_x509_projection_stark_residues_v1, hash_bus,
        },
        relation::{
            ZkX509GovernanceV1,
            release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
        },
    };
    for maximum in [false, true] {
        let fixture =
            build_zk_x509_release_fixture_v1(reference_statement_context_v1(), maximum).unwrap();
        let trust = fixture.authoritative_state.trust_anchor();
        let crl = fixture.authoritative_state.crl_record();
        let assembly = build_zk_x509_main_trace_assembly_v1(
            &fixture.statement,
            ZkX509GovernanceV1 {
                trust_anchor: &trust,
                certificate_policy: fixture.authoritative_state.certificate_policy(),
                crl: &crl,
            },
            &fixture.witness,
        )
        .unwrap();
        let binding = credential_binding(0x71);
        let projection_challenges = binding.main_post_base().projection();
        assert_eq!(projection_challenges.hash_bus, binding.sha());
        let trace = &assembly.projection_trace;
        let aux =
            build_zk_x509_projection_aux_trace_v1(&trace.base, &trace.fixed, projection_challenges)
                .unwrap();
        let fixed = compile_zk_x509_projection_stark_fixed_rows_v1(&fixture.statement).unwrap();
        for index in 0..trace.base.rows.len() {
            let next = (index + 1) % trace.base.rows.len();
            let residues = evaluate_zk_x509_projection_stark_residues_v1(
                &trace.base.rows[index],
                &trace.base.rows[next],
                &aux.rows[index],
                &aux.rows[next],
                &fixed[index],
                projection_challenges,
            )
            .unwrap();
            assert!(
                residues.iter().all(|value| *value == F::ZERO),
                "projection row {index}"
            );
        }
        let disclosures = fixture.statement.disclosed_attributes.len();
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        })
        .unwrap();
        let mut sha_total = [F::ONE; 4];
        for invocation in 0..7 {
            let manifest = schedule.calls[5 + invocation];
            let witness = &assembly.sha_witnesses[5 + invocation];
            let call = build_zk_x509_sha_batch_call_trace_v1(
                manifest,
                witness,
                binding.sha_word(),
                binding.sha(),
                binding.rfc5280(),
                disclosures,
            )
            .unwrap();
            let expected = if projection_bus::is_active_v1(manifest) {
                projection_message_product_for_test_v1(
                    invocation,
                    &witness.message,
                    &witness.digest,
                    binding.sha(),
                )
            } else {
                [F::ONE; 4]
            };
            assert_eq!(
                call.segment_product_state.projection_products, expected,
                "slot {invocation}"
            );
            for lane in 0..4 {
                sha_total[lane] = sha_total[lane].mul(expected[lane]);
            }
            let mut counts = [0_usize; 3];
            for index in 0..call.logical_rows() {
                let current = call.row(index).unwrap();
                for (kind, column) in [
                    projection_bus::FIX_WORD_V1,
                    projection_bus::FIX_LENGTH_V1,
                    projection_bus::FIX_DIGEST_V1,
                ]
                .into_iter()
                .enumerate()
                {
                    assert!(current.fixed[column] == F::ZERO || current.fixed[column] == F::ONE);
                    counts[kind] += usize::from(current.fixed[column] == F::ONE);
                }
                assert!(
                    current.fixed[projection_bus::FIX_WORD_V1..]
                        .iter()
                        .filter(|v| **v == F::ONE)
                        .count()
                        <= 1
                );
                let next = if index + 1 < call.logical_rows() {
                    call.row(index + 1).unwrap()
                } else {
                    let mut terminal = current;
                    terminal.aux[projection_bus::AUX_START_V1..].copy_from_slice(&expected);
                    terminal
                };
                let mut residues = Vec::new();
                projection_bus::push_residues_v1(&current, &next, binding.sha(), &mut residues);
                assert!(
                    residues.iter().all(|v| *v == F::ZERO),
                    "slot {invocation} row {index}"
                );
                if current.fixed[projection_bus::FIX_WORD_V1] == F::ONE {
                    let word = current.fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0 as usize;
                    if word == 0 || word == witness.message.len() / 4 || word == 511 {
                        let full = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
                            &current,
                            &next,
                            binding.sha_word(),
                            binding.sha(),
                            binding.rfc5280(),
                            (manifest.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1) as u8,
                        )
                        .unwrap();
                        assert!(full.iter().all(|v| *v == F::ZERO));
                        let mut wrong = next;
                        wrong.aux[projection_bus::AUX_START_V1] =
                            wrong.aux[projection_bus::AUX_START_V1].add(F::ONE);
                        let mut failures = Vec::new();
                        projection_bus::push_residues_v1(
                            &current,
                            &wrong,
                            binding.sha(),
                            &mut failures,
                        );
                        assert!(failures.iter().any(|v| *v != F::ZERO));
                    }
                }
            }
            assert_eq!(
                counts,
                if projection_bus::is_active_v1(manifest) {
                    [512, 1, 8]
                } else {
                    [0; 3]
                }
            );
            if invocation == 0 {
                let mut changed = witness.clone();
                *changed.message.last_mut().unwrap() ^= 1;
                changed.digest = Sha256::digest(&changed.message).into();
                let recomputed = build_zk_x509_sha_batch_call_trace_v1(
                    manifest,
                    &changed,
                    binding.sha_word(),
                    binding.sha(),
                    binding.rfc5280(),
                    disclosures,
                )
                .unwrap();
                assert_ne!(
                    recomputed.segment_product_state.projection_products,
                    expected
                );
            }
        }
        assert_eq!(aux.rows[0][hash_bus::AUX_START_V1..], sha_total);
        let mut segmented = [F::ONE; 4];
        for segment in 0..4 {
            let terminal = projection_bus::segment_terminal_v1(
                &schedule,
                &assembly.sha_witnesses,
                segment,
                binding.sha(),
            )
            .unwrap();
            for lane in 0..4 {
                segmented[lane] = segmented[lane].mul(terminal[lane]);
            }
        }
        assert_eq!(segmented, sha_total);
        for (current, next) in [(0, 1), (trace.base.rows.len() - 1, 0)] {
            let mut bad = aux.rows[next];
            bad[hash_bus::AUX_START_V1] = bad[hash_bus::AUX_START_V1].add(F::ONE);
            let residues = evaluate_zk_x509_projection_stark_residues_v1(
                &trace.base.rows[current],
                &trace.base.rows[next],
                &aux.rows[current],
                &bad,
                &fixed[current],
                projection_challenges,
            )
            .unwrap();
            assert!(residues.iter().any(|v| *v != F::ZERO));
        }
    }
}
