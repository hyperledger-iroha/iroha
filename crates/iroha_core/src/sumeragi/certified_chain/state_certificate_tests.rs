//! Actual native-tip certificate reads and independent quorum/availability refusals.

use super::*;
use iroha_data_model::query::error::QueryExecutionFail;

#[test]
fn state_certificate_reads_only_target_and_parent_under_exact_source_limits() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    for height in [2_u64, 4, 5] {
        let source_count = chain.height() - height + 2;
        let source_bytes = (height - 1..=chain.height())
            .map(|h| frame(&chain, h).encode_wire().unwrap().len() as u64)
            .sum::<u64>();
        for (work, bytes, success) in [
            (source_count, source_bytes, true),
            (source_count - 1, source_bytes, false),
            (source_count, source_bytes - 1, false),
        ] {
            let mut remaining_work = work;
            let mut remaining_bytes = bytes;
            let (result, observed) = relation_counts::measure(|| {
                reader.certified_from_execution(
                    NonZeroUsize::new(height as usize).unwrap(),
                    |work, bytes| {
                        remaining_work = remaining_work
                            .checked_sub(work)
                            .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                        remaining_bytes = remaining_bytes
                            .checked_sub(bytes)
                            .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                        Ok(())
                    },
                )
            });
            if success {
                assert_eq!(result.unwrap().id(), chain.committed(height).id());
                assert_eq!(remaining_work, 0);
                assert_eq!(remaining_bytes, 0);
                assert_eq!(
                    observed.frames,
                    (height - 1..=chain.height()).rev().collect::<Vec<_>>()
                );
                assert_eq!(observed.qcs, [height]);
            } else {
                assert!(matches!(result, Err(QueryExecutionFail::GasBudgetExceeded)));
                assert!(observed.qcs.is_empty());
            }
        }
    }
}

#[test]
fn state_certificate_refuses_unanchored_history_and_foreign_source_modes() {
    let (chain, _) = chain();
    let frames = (1..=5)
        .map(|height| frame(&chain, height))
        .collect::<Vec<_>>();
    let hashes = frames.iter().map(|block| block.hash()).collect::<Vec<_>>();
    let unanchored = state_with_history(&frames);
    let view = unanchored.view();
    let reader = CertifiedChain::new(&view).unwrap();
    assert!(
        reader
            .certified_from_execution(NonZeroUsize::new(5).unwrap(), |_, _| Ok(()))
            .is_err()
    );
    let network = chain.network_id();
    let reader = CertifiedChain::from_frames(view.chain_id(), &network, &hashes, &frames).unwrap();
    assert!(
        reader
            .certified_from_execution(NonZeroUsize::new(5).unwrap(), |_, _| Ok(()))
            .is_err()
    );
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    assert!(
        reader
            .certified_from_execution(NonZeroUsize::new(1).unwrap(), |_, _| Ok(()))
            .is_err()
    );
    chain
        .kura()
        .corrupt_canonical_body_for_testing(NonZeroUsize::new(4).unwrap())
        .unwrap();
    assert!(
        reader
            .certified_from_execution(NonZeroUsize::new(5).unwrap(), |_, _| Ok(()))
            .is_err()
    );
}

#[test]
fn state_certificate_checks_quorum_and_signed_availability_after_execution_authentication() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    let original = frame(&chain, 4);
    let parent = chain.committed(3);
    reader
        .verify_executed_successor(&parent, read_frame(original.clone(), 4).unwrap())
        .unwrap();
    let (_, qc) = decode_certificate(original.commit_certificate().unwrap()).unwrap();
    for signers in [Signers::BelowQuorum, Signers::All] {
        let changed = with_parts(&original, |_, target, _| {
            *target = chain.commit_qc(4, qc.block_hash, qc.result, qc.attest, signers);
        });
        assert!(
            reader
                .verify_executed_successor(&parent, read_frame(changed, 4).unwrap())
                .is_err()
        );
    }
    let forged = with_parts(&original, |_, qc, _| qc.agg_sig.0[5] ^= 1);
    assert!(
        reader
            .verify_executed_successor(&parent, read_frame(forged, 4).unwrap())
            .is_err()
    );
    let certificate = original.commit_certificate().unwrap();
    let mut corrupted = certificate.availability().to_vec();
    *corrupted.last_mut().unwrap() ^= 1;
    for availability in [
        Vec::new(),
        corrupted,
        frame(&chain, 3)
            .commit_certificate()
            .unwrap()
            .availability()
            .to_vec(),
    ] {
        let changed = Arc::new(original.as_ref().clone().with_commit_certificate(Some(
            CommitCertificate::from_untrusted_parts(
                certificate.consensus_header().to_vec(),
                certificate.commit_qc().to_vec(),
                certificate.result_preimage().to_vec(),
                availability,
            ),
        )));
        assert!(
            read_frame(changed, 4)
                .map_err(VerificationReadError::from)
                .and_then(|current| reader.verify_executed_successor(&parent, current))
                .is_err()
        );
    }
}

#[test]
fn state_certificate_verifies_actual_attested_npos_boundary() {
    let mut chain = CertifiedTestChain::npos_boundary_fixture();
    chain.commit(Vec::new());
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    let result = reader
        .certified_from_execution(NonZeroUsize::new(10).unwrap(), |_, _| Ok(()))
        .unwrap();
    assert!(result.header().unwrap().attest);
    assert!(result.commitment().schedule.boundary.is_some());
    assert_eq!(result.id(), reader.certified(10).unwrap().id());
    let original = frame(&chain, 10);
    let forged = with_parts(&original, |_, qc, _| {
        let mut seal = qc.attestations[0].as_slice().to_vec();
        seal[8] ^= 1;
        qc.attestations[0] =
            iroha_sumeragi::message::AttestationSignature::try_from_slice(&seal).unwrap();
    });
    assert!(
        reader
            .verify_executed_successor(&chain.committed(9), read_frame(forged, 10).unwrap())
            .is_err()
    );
}

#[test]
fn state_certificate_original_tip_does_not_authorize_a_corrupt_local_quorum() {
    for signers in [Signers::BelowQuorum, Signers::All] {
        let (chain, _) = chain();
        chain.corrupt_local_quorum_for_test(4, signers);
        let view = chain.state().view();
        let reader = CertifiedChain::new(&view).unwrap();
        assert!(
            reader.committed(4).is_ok(),
            "original execution remains intact"
        );
        assert!(
            reader
                .certified_from_execution(NonZeroUsize::new(4).unwrap(), |_, _| Ok(()))
                .is_err()
        );
    }
}

#[test]
fn state_certificate_genesis_admission_precedes_every_body_read() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let expected = frame(&chain, 1);
    let length = expected.encode_wire().unwrap().len() as u64;
    for (work, bytes, allowed) in [
        (0, length, false),
        (1, length - 1, false),
        (1, length, true),
    ] {
        chain.kura().reset_canonical_query_reads_for_test();
        let mut charges = Vec::new();
        let result = CertifiedChain::new_with_source_admission(&view, |count, length| {
            charges.push((count, length));
            if count > work || length > bytes {
                return Err(QueryExecutionFail::GasBudgetExceeded);
            }
            Ok(())
        });
        assert_eq!(charges, [(1, length)]);
        if allowed {
            assert_eq!(result.unwrap().genesis().as_ref(), expected.as_ref());
            assert_eq!(chain.kura().canonical_query_reads_for_test(), (1, length));
        } else {
            assert!(matches!(result, Err(QueryExecutionFail::GasBudgetExceeded)));
            assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
        }
    }
    // An occupied corrupt frame must still be refused before I/O when unpaid,
    // and an admitted read must not substitute the old cached valid genesis.
    chain
        .kura()
        .corrupt_native_frame_for_test(NonZeroUsize::new(1).unwrap());
    chain.kura().reset_canonical_query_reads_for_test();
    assert!(matches!(
        CertifiedChain::new_with_source_admission(&view, |_, _| Err(
            QueryExecutionFail::GasBudgetExceeded
        )),
        Err(QueryExecutionFail::GasBudgetExceeded)
    ));
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
    assert!(CertifiedChain::new_with_source_admission(&view, |_, _| Ok(())).is_err());
    assert_eq!(chain.kura().canonical_query_reads_for_test(), (1, length));
}

#[test]
fn state_certificate_genesis_decode_refusal_is_typed_and_retryable() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let refusal =
        norito::with_decode_limits_scope(norito::DecodeLimits::new(1, 1, 1, 1, 1), || {
            CertifiedChain::new_with_source_admission(&view, |_, _| Ok(()))
        });
    assert!(
        matches!(refusal, Err(QueryExecutionFail::GasBudgetExceeded)),
        "{refusal:?}"
    );
    let reader = CertifiedChain::new_with_source_admission(&view, |_, _| Ok(())).unwrap();
    assert_eq!(reader.genesis().as_ref(), frame(&chain, 1).as_ref());
    assert_eq!(
        reader
            .certified_from_execution(NonZeroUsize::new(5).unwrap(), |_, _| Ok(()))
            .unwrap()
            .id(),
        chain.committed(5).id()
    );
}

#[test]
fn state_certificate_source_allowance_includes_genesis_and_recent_ancestry() {
    let (chain, _) = chain();
    let view = chain.state().view();
    for height in [2_u64, 5] {
        let count = 1 + chain.height() - height + 2;
        let bytes = frame(&chain, 1).encode_wire().unwrap().len() as u64
            + (height - 1..=chain.height())
                .map(|h| frame(&chain, h).encode_wire().unwrap().len() as u64)
                .sum::<u64>();
        for (work_limit, byte_limit, succeeds) in [
            (count, bytes, true),
            (count - 1, bytes, false),
            (count, bytes - 1, false),
        ] {
            let mut work_left = work_limit;
            let mut bytes_left = byte_limit;
            let mut admit = |work, bytes| {
                let work_next = work_left
                    .checked_sub(work)
                    .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                let bytes_next = bytes_left
                    .checked_sub(bytes)
                    .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                work_left = work_next;
                bytes_left = bytes_next;
                Ok(())
            };
            let reader = CertifiedChain::new_with_source_admission(&view, &mut admit).unwrap();
            let result = reader
                .certified_from_execution(NonZeroUsize::new(height as usize).unwrap(), &mut admit);
            if succeeds {
                assert_eq!(result.unwrap().id(), chain.committed(height).id());
                assert_eq!((work_left, bytes_left), (0, 0));
            } else {
                assert!(matches!(result, Err(QueryExecutionFail::GasBudgetExceeded)));
            }
        }
    }
}

#[test]
fn state_certificate_selected_ancestor_reuses_original_walk_and_adjacent_parent() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    let latest_id = chain.committed(5).id();
    for selected in [None, Some(4_usize), Some(3), Some(2)] {
        let last = selected.unwrap_or(5) - 1;
        let source_heights = (last as u64..=5).rev().collect::<Vec<_>>();
        let source_bytes = source_heights
            .iter()
            .map(|h| frame(&chain, *h).encode_wire().unwrap().len() as u64)
            .sum::<u64>();
        for refused in [true, false] {
            let mut work_left = source_heights.len() as u64 - u64::from(refused);
            let mut bytes_left = source_bytes;
            let (result, observed) = relation_counts::measure(|| {
                reader.certified_with_ancestor_from_execution(
                    NonZeroUsize::new(5).unwrap(),
                    |work, bytes| {
                        work_left = work_left
                            .checked_sub(work)
                            .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                        bytes_left = bytes_left
                            .checked_sub(bytes)
                            .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                        Ok(())
                    },
                    |latest| {
                        assert_eq!(latest.id(), latest_id);
                        Ok(selected.and_then(NonZeroUsize::new))
                    },
                )
            });
            if refused {
                assert!(matches!(result, Err(QueryExecutionFail::GasBudgetExceeded)));
            } else {
                let (latest, ancestor) = result.unwrap();
                assert_eq!(latest.id(), latest_id);
                assert_eq!(
                    ancestor.as_ref().map(|block| block.height()),
                    selected.map(|h| h as u64)
                );
                assert_eq!(observed.frames, source_heights);
                assert_eq!(
                    observed.qcs,
                    [Some(5), selected.map(|h| h as u64)]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                );
                assert_eq!((work_left, bytes_left), (0, 0));
            }
        }
    }
    for invalid in [1, 5, 6] {
        assert!(
            reader
                .certified_with_ancestor_from_execution(
                    NonZeroUsize::new(5).unwrap(),
                    |_, _| Ok(()),
                    |_| Ok(NonZeroUsize::new(invalid)),
                )
                .is_err()
        );
    }
}

#[test]
fn state_certificate_selected_ancestor_checks_both_native_quorums() {
    for bad_height in [3, 5] {
        for signers in [Signers::BelowQuorum, Signers::All] {
            let (chain, _) = chain();
            chain.corrupt_local_quorum_for_test(bad_height, signers);
            let view = chain.state().view();
            let reader = CertifiedChain::new(&view).unwrap();
            assert!(
                reader.committed(bad_height).is_ok(),
                "original execution remains authenticated"
            );
            let mut selected = false;
            let result = reader.certified_with_ancestor_from_execution(
                NonZeroUsize::new(5).unwrap(),
                |_, _| Ok(()),
                |_| {
                    selected = true;
                    Ok(NonZeroUsize::new(3))
                },
            );
            assert!(
                result.is_err(),
                "both requested certificates require the exact native quorum"
            );
            assert_eq!(
                selected,
                bad_height != 5,
                "selector must not see an unverified target"
            );
        }
    }
}

#[test]
fn state_certificate_signed_availability_scratch_uses_original_query_allowance() {
    use iroha_allocation::AllocationBudget;
    use iroha_sumeragi::availability::PayloadBytes;

    let (chain, _) = chain();
    let source = frame(&chain, 3);
    let current = read_frame(Arc::clone(&source), 3).unwrap();
    let parent = chain.committed(2);
    let schedule::ScheduledSlot::Ready(scheduled) = &parent.commitment.schedule.next else {
        panic!("original executed parent authorizes height 3");
    };
    let mut validation = EpochValidationScope::new();
    let config = scheduled
        .height_config_with_validation(&mut validation)
        .unwrap();
    let authority = VerifiedAuthority::new(scheduled.epoch.clone(), 3, &mut validation).unwrap();
    let budget = AllocationBudget::new(1 << 26);
    let artifacts = artifacts::PrefixArtifactsRead::new(Arc::clone(&source), budget.clone())
        .complete(&budget)
        .unwrap_or_else(|(_, error)| panic!("original artifact owners: {error}"));
    let (_, table, payload) = artifacts
        .into_parts(&source, current.header.as_ref().unwrap())
        .unwrap();
    let retained = budget.reserved_bytes();
    assert!(table.admitted_to(&budget) && payload.admitted_to(&budget));
    let verified = iroha_sumeragi::availability::verify_availability(
        current.header.as_ref().unwrap().instance,
        &config,
        current.header.as_ref().unwrap(),
        table.as_slice(),
        &authority.crypto,
    )
    .unwrap();
    let shape = verified.shape();
    let scratch = shape.encoded_bytes() + shape.workspace_words() * size_of::<u16>();
    let check = |limit, bytes: &PayloadBytes| {
        norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, limit, 128),
            || {
                verify_availability(
                    &current,
                    &config,
                    &authority.crypto,
                    Some((table.clone(), bytes.clone())),
                    &mut crate::sumeragi::certified_chain::state_certificate::query_scratch_admission,
                )
            },
        )
    };
    assert!(matches!(
        check(scratch - 1, &payload),
        Err(VerificationReadError::Resource(
            norito::core::DecodeResourceError::TotalAllocationExceeded { attempted, limit }
        )) if attempted == scratch as u64 && limit == (scratch - 1) as u64
    ));
    assert_eq!(budget.reserved_bytes(), retained);
    check(scratch, &payload).expect("same source and original owners retry at the exact allowance");
    assert_eq!(budget.reserved_bytes(), retained);
    assert!(Arc::ptr_eq(&current.block, &source));

    let mut corrupt = payload.as_slice().to_vec();
    corrupt[0] ^= 1;
    let mut corrupt = PayloadBytes::from_untrusted(corrupt).unwrap();
    corrupt.admit(&budget).unwrap();
    assert!(matches!(
        check(scratch, &corrupt),
        Err(VerificationReadError::Source(
            ChainReadError::Malformed { .. }
        ))
    ));
    drop((corrupt, payload, table));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn state_certificate_native_qc_decode_refusal_is_capacity_and_retries_original_source() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    let current = chain.committed(3);
    let parent = chain.committed(2);
    let schedule::ScheduledSlot::Ready(scheduled) = &parent.commitment.schedule.next else {
        panic!("original parent authorizes the source");
    };
    let mut validation = EpochValidationScope::new();
    let authority = VerifiedAuthority::new(scheduled.epoch.clone(), 3, &mut validation).unwrap();
    let config = scheduled
        .height_config_with_validation(&mut validation)
        .unwrap();
    let check = |field_limit| {
        norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, field_limit, usize::MAX, usize::MAX, 128),
            || {
                reader.verification_context().verify_certificate_with_scratch_admission(
                    current.clone(),
                    &authority,
                    Some(&config),
                    None,
                    &mut crate::sumeragi::certified_chain::state_certificate::query_scratch_admission,
                )
            },
        )
    };
    assert!(matches!(
        check(1),
        Err(VerificationReadError::Resource(
            norito::core::DecodeResourceError::FieldLengthExceeded { limit: 1, .. }
        ))
    ));
    let certified =
        check(usize::MAX).expect("exact original source retries without a new authority");
    assert_eq!(certified.id(), current.id());
    assert!(Arc::ptr_eq(certified.block(), current.block()));
}

#[test]
fn state_certificate_pairing_constructor_refusal_preserves_original_source_for_retry() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    let parent = chain.committed(2);
    let current = chain.committed(3);
    let backing = iroha_crypto::BlsNormalAggregateScratch::<()>::backing_bytes();
    let check = |limit| {
        norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, limit, 128),
            || reader.verify_executed_successor(&parent, current.clone()),
        )
    };
    for _ in 0..2 {
        assert!(matches!(
            check(backing - 1),
            Err(VerificationReadError::Resource(
                norito::core::DecodeResourceError::TotalAllocationExceeded { attempted, limit }
            )) if attempted == backing as u64 && limit == (backing - 1) as u64
        ));
    }
    let certified =
        check(1 << 26).expect("same authenticated body and parent retry after local refusal");
    assert_eq!(certified.id(), current.id());
    assert!(Arc::ptr_eq(certified.block(), current.block()));
    assert_eq!(parent.id(), chain.committed(2).id());
}
