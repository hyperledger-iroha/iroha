//! Exact-key coefficient staging and complete protocol-owner lifetime tests.
//!
//! Reuses the plaintext prefix backend only as an oracle. Phase conversion tests separately
//! inspect original blind guards and interleaved schedules. No complete stored proof is made.

use super::*;

fn owned_stage<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for compressed in [false, true] {
        let params = ParamsIPA::<C>::new(4);
        let pk = key(&params, compressed);
        // This actual key has copy constraints; coefficient staging must not masquerade as an
        // empty-argument special case or discard original inputs needed by those constraints.
        assert!(!pk.vk.cs.permutation.columns.is_empty());
        let vk = pk.get_vk().to_bytes(crate::SerdeFormat::Processed);
        let fixed_allocation = pk.fixed_values.as_ptr();
        let values = columns::<C::Scalar>();
        let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let shared = Shared::<C>::new();
        let prefix =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk,
                Producer::new(&shared, 3),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap();
        let (events, draws, expected) = {
            let log = shared.log.lock().unwrap();
            (
                log.events.clone(),
                log.rng_calls,
                log.sealed
                    .iter()
                    .map(|(layout, encoded)| {
                        let values = encoded
                            .iter()
                            .map(|bytes| {
                                Option::<C::Scalar>::from(C::Scalar::from_repr(*bytes)).unwrap()
                            })
                            .collect();
                        let polynomial = prefix.pk.vk.domain.lagrange_from_vec(values);
                        let polynomial = prefix.pk.vk.domain.lagrange_to_coeff(polynomial);
                        (
                            *layout,
                            polynomial
                                .iter()
                                .map(PrimeField::to_repr)
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let mut oracle_rng = shared.rng.lock().unwrap().clone();
        let mut staged = prefix.stage_advice_coefficients().unwrap();
        assert!(std::ptr::eq(staged.params, &params));
        assert!(std::ptr::eq(staged.instances.as_ptr(), instances.as_ptr()));
        assert_eq!(
            staged.pk.get_vk().to_bytes(crate::SerdeFormat::Processed),
            vk
        );
        assert_eq!(staged.pk.fixed_values.as_ptr(), fixed_allocation);
        assert_eq!(staged.instances, instances.as_slice());
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.producer_drops, 1);
        assert_eq!(log.created, 4);
        assert_eq!(log.writer_drops, 4);
        assert_eq!(
            log.snapshot_drops, 0,
            "both original Lagrange snapshots still serve real arguments"
        );
        assert_eq!(log.provider_drops, 0);
        assert_eq!(log.rng_drops, 0);
        assert_eq!(log.transcript_drops, 0);
        assert_eq!(log.sealed.len(), 4);
        for (index, (layout, encoded)) in log.sealed[2..].iter().enumerate() {
            assert_eq!(layout.basis(), StoredPolynomialBasisV1::Coefficient);
            assert_eq!(
                layout.advice_coordinates().unwrap().0,
                expected[index].0.advice_coordinates().unwrap().0
            );
            assert_eq!(
                layout.advice_coordinates().unwrap().1,
                expected[index].0.advice_coordinates().unwrap().1
            );
            assert!(layout.same_proof_context(expected[index].0));
            assert_eq!(layout.ordinal(), 2 + index as u64);
            assert_eq!(*encoded, expected[index].1);
        }
        drop(log);
        let mut actual_next = [0; 64];
        let mut expected_next = [0; 64];
        staged.rng.fill_bytes(&mut actual_next);
        oracle_rng.fill_bytes(&mut expected_next);
        assert_eq!(actual_next, expected_next);
        drop(staged);
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.snapshot_drops, 4);
        assert_eq!(log.provider_drops, 1);
        assert_eq!(log.rng_drops, 1);
        assert_eq!(log.transcript_drops, 1);
    }
}

#[test]
fn both_pasta_keyed_coefficient_stage_preserves_original_protocol_state_and_fft_values() {
    owned_stage::<EqAffine>();
    owned_stage::<EpAffine>();
}

fn failed_stage<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for fault in [
        Fault::Create,
        Fault::CreateColumn(1),
        Fault::Write(1),
        Fault::Seal,
        Fault::Read(1),
        Fault::PanicRead(1),
    ] {
        let shared = Shared::<C>::new();
        let prefix =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                Producer::new(&shared, 300),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap();
        let (events, draws) = {
            let mut log = shared.log.lock().unwrap();
            log.fault = Some(fault);
            (log.events.clone(), log.rng_calls)
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            prefix.stage_advice_coefficients().map(|_| ())
        }));
        if matches!(fault, Fault::PanicRead(_)) {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap(),
                Err(StoredPrefixErrorV1::Phase(StoredPhaseErrorV1::Store(
                    StoredPolynomialErrorV1::Storage
                )))
            );
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        let sealed = if fault == Fault::CreateColumn(1) {
            3
        } else {
            2
        };
        assert_eq!(log.sealed.len(), sealed);
        assert_eq!(
            log.snapshot_drops, sealed,
            "every sealed partial coefficient also drops"
        );
        assert_eq!(log.created, if fault == Fault::Create { 2 } else { 3 });
        assert_eq!(log.writer_drops, log.created);
        assert_eq!(log.provider_drops, 1);
        assert_eq!(log.rng_drops, 1);
        assert_eq!(log.transcript_drops, 1);
    }
}

#[test]
fn both_pasta_coefficient_error_and_unwind_destroy_provider_rng_transcript_and_all_receipts() {
    failed_stage::<EqAffine>();
    failed_stage::<EpAffine>();
}
