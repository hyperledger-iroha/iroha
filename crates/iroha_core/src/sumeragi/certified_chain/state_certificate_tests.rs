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

#[test]
fn state_certificate_original_qc_inner_limit_is_not_adopted_by_wider_query_scope() {
    let (chain, _) = chain();
    let current = chain.committed(3);
    let bytes = current.block().commit_certificate().unwrap().commit_qc();
    let original: iroha_sumeragi::message::Qc = norito::decode_canonical(bytes).unwrap();
    norito::core::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 128),
        || {
            let error = norito::core::with_decode_limits_scope(
                norito::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, 128),
                || norito::decode_canonical::<iroha_sumeragi::message::Qc>(bytes).unwrap_err(),
            );
            assert!(
                matches!(&error, norito::Error::FieldLengthExceeded { length, limit: 1 } if *length > 1)
            );
            let expected = error.to_string();
            assert!(matches!(verification_codec_error(3, error),
                VerificationReadError::Source(ChainReadError::Malformed { height: 3, reason }) if reason == expected));
            let mut malformed = bytes.to_vec();
            malformed.push(0);
            let error =
                norito::decode_canonical::<iroha_sumeragi::message::Qc>(&malformed).unwrap_err();
            assert!(error.decode_resource_error().is_none());
            assert!(matches!(
                verification_codec_error(3, error),
                VerificationReadError::Source(ChainReadError::Malformed { height: 3, .. })
            ));
        },
    );
    assert_eq!(
        norito::decode_canonical::<iroha_sumeragi::message::Qc>(bytes).unwrap(),
        original
    );
}

/// One valid quorum chosen by the proposal is common input even when the
/// independently executed source retains a different valid local certificate.
#[test]
fn parent_service_common_proposal_is_independent_of_local_certificate_subset() {
    use crate::sumeragi::certified_chain::ParentServiceError;
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new_for_parent_service(&view).unwrap();
    let proposal = chain.proposal(Some(6_000), Vec::new());
    let offered = proposal
        .npos_consensus_effects()
        .unwrap()
        .parent_service_commit_qc
        .as_deref()
        .unwrap();
    let offered_qc: Qc = norito::decode_canonical(offered).unwrap();
    let source = frame(&chain, 5);
    let (_, source_qc) = decode_certificate(source.commit_certificate().unwrap()).unwrap();
    assert_eq!(offered_qc, source_qc);
    let other_qc = chain.commit_qc(
        5,
        source_qc.block_hash,
        source_qc.result,
        source_qc.attest,
        Signers::LastThree,
    );
    assert_ne!(source_qc.signers, other_qc.signers);
    let other_source = with_parts(&source, |_, qc, _| *qc = other_qc.clone());
    let predecessor = chain.committed(4);
    let left = reader
        .verify_parent_service_at(
            &proposal,
            &predecessor,
            read_frame(source, 5).unwrap(),
            offered,
        )
        .unwrap()
        .unwrap();
    let right = reader
        .verify_parent_service_at(
            &proposal,
            &predecessor,
            read_frame(other_source, 5).unwrap(),
            offered,
        )
        .unwrap()
        .unwrap();
    assert_eq!(left.height(), right.height());
    assert_eq!(left.timestamp_ms(), right.timestamp_ms());
    assert_eq!(left.signers(), right.signers());
    let full = reader
        .authenticate_parent_service(&proposal, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(full.signers(), left.signers());
    full.require_proposal(&proposal).unwrap();

    let mut other_proposal = proposal.clone();
    let mut effects = proposal.npos_consensus_effects().unwrap().clone();
    effects.parent_service_commit_qc = Some(norito::to_bytes(&other_qc).unwrap());
    other_proposal.set_npos_consensus_effects(Some(effects));
    assert_ne!(proposal.hash(), other_proposal.hash());
    let other = reader
        .authenticate_parent_service(&other_proposal, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    assert_ne!(full.signers(), other.signers());
    assert!(matches!(
        full.require_proposal(&other_proposal),
        Err(ParentServiceError::Invalid(_))
    ));
}

#[test]
fn parent_service_proof_requires_exact_native_quorum_and_complete_subject() {
    use crate::sumeragi::certified_chain::ParentServiceError;
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new_for_parent_service(&view).unwrap();
    let proposal = chain.proposal(Some(6_000), Vec::new());
    let offered = proposal
        .npos_consensus_effects()
        .unwrap()
        .parent_service_commit_qc
        .as_ref()
        .unwrap();
    let qc: Qc = norito::decode_canonical(offered).unwrap();
    let mut cases = Vec::new();
    for mutate in 0..7 {
        let mut changed = qc.clone();
        match mutate {
            0 => changed.instance = Hash32([0x17; 32]),
            1 => changed.height -= 1,
            2 => changed.block_hash = Hash32([0x27; 32]),
            3 => changed.result = Hash32([0x37; 32]),
            4 => changed.attest = !changed.attest,
            5 => changed.epoch.epoch += 1,
            6 => changed.agg_sig.0[5] ^= 1,
            _ => unreachable!(),
        }
        cases.push(norito::to_bytes(&changed).unwrap());
    }
    for signers in [Signers::BelowQuorum, Signers::All] {
        cases.push(
            norito::to_bytes(&chain.commit_qc(5, qc.block_hash, qc.result, qc.attest, signers))
                .unwrap(),
        );
    }
    cases.push(Vec::new());
    cases.push(vec![
        0;
        iroha_data_model::consensus::PARENT_SERVICE_COMMIT_QC_MAX_BYTES
            + 1
    ]);
    for bytes in cases {
        let mut changed = proposal.clone();
        let mut effects = proposal.npos_consensus_effects().unwrap().clone();
        effects.parent_service_commit_qc = Some(bytes);
        changed.set_npos_consensus_effects(Some(effects));
        assert!(matches!(
            reader.authenticate_parent_service(&changed, |_, _| Ok(())),
            Err(ParentServiceError::Invalid(_))
        ));
    }
    let mut missing = proposal.clone();
    let mut effects = proposal.npos_consensus_effects().unwrap().clone();
    effects.parent_service_commit_qc = None;
    missing.set_npos_consensus_effects((!effects.is_empty()).then_some(effects));
    assert!(matches!(
        reader.authenticate_parent_service(&missing, |_, _| Ok(())),
        Err(ParentServiceError::Invalid(_))
    ));
}

#[test]
fn parent_service_original_decode_refusal_retries_same_proposal_without_invalidity() {
    use crate::sumeragi::certified_chain::ParentServiceError;
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new_for_parent_service(&view).unwrap();
    let proposal = chain.proposal(Some(6_000), Vec::new());
    let offered = proposal
        .npos_consensus_effects()
        .unwrap()
        .parent_service_commit_qc
        .as_ref()
        .unwrap();
    let current = chain.committed(5);
    let predecessor = chain.committed(4);
    let refused = norito::core::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 128),
        || reader.verify_parent_service_at(&proposal, &predecessor, current.clone(), offered),
    );
    assert!(matches!(refused, Err(ParentServiceError::Deferred(_))));
    let retry = reader
        .verify_parent_service_at(&proposal, &predecessor, current, offered)
        .unwrap()
        .unwrap();
    retry.require_proposal(&proposal).unwrap();
    assert_eq!(retry.height(), 5);
    assert!(!retry.signers().is_empty());
}

#[test]
fn parent_service_source_loss_is_recovery_without_participation_receipt() {
    use crate::sumeragi::certified_chain::ParentServiceError;
    let (chain, _) = chain();
    let proposal = chain.proposal(Some(6_000), Vec::new());
    let view = chain.state().view();
    let reader = CertifiedChain::new_for_parent_service(&view).unwrap();
    chain
        .kura()
        .corrupt_canonical_body_for_testing(NonZeroUsize::new(4).unwrap())
        .unwrap();
    assert!(matches!(
        reader.authenticate_parent_service(&proposal, |_, _| Ok(())),
        Err(ParentServiceError::Source(_))
    ));
}

#[test]
fn parent_service_source_allowance_counts_every_original_before_authentication() {
    use crate::sumeragi::certified_chain::ParentServiceError;
    let (chain, _) = chain();
    let proposal = chain.proposal(Some(6_000), Vec::new());
    let view = chain.state().view();
    let reader = CertifiedChain::new_for_parent_service(&view).unwrap();
    let bytes = (4..=5)
        .map(|height| frame(&chain, height).encode_wire().unwrap().len() as u64)
        .sum::<u64>();
    for (work, allowance, success) in [
        (2_u64, bytes, true),
        (1, bytes, false),
        (2, bytes - 1, false),
    ] {
        let mut remaining_work = work;
        let mut remaining_bytes = allowance;
        let (result, count) = relation_counts::measure(|| {
            reader.authenticate_parent_service(&proposal, |work, bytes| {
                remaining_work = remaining_work
                    .checked_sub(work)
                    .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                remaining_bytes = remaining_bytes
                    .checked_sub(bytes)
                    .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                Ok(())
            })
        });
        if success {
            assert_eq!(result.unwrap().unwrap().height(), 5);
            assert_eq!(remaining_work, 0);
            assert_eq!(remaining_bytes, 0);
            assert_eq!(count.qcs, [5]);
        } else {
            assert!(matches!(
                result,
                Err(ParentServiceError::Deferred(reason))
                    if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity
            ));
            assert!(count.qcs.is_empty());
        }
    }
}
