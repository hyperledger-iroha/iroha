//! Prime-power packed routing, full-ring relative trace and local threshold opening.
//!
//! Genuine encrypted arithmetic under small diagnostic profiles; these fixtures
//! do not qualify parameters, imported keys, circuit privacy or a refresh.
use iroha_fhe::bgv::{
    BgvParameters, BgvProfile, Ciphertext, Plaintext, RelativeTraceBudget,
    RelativeTraceDecompositionPlan, RelativeTraceRecompositionPlan, SlotEncoding, SlotLinearBudget,
    SlotLinearMap, SlotLinearPlan,
    fixtures::{self, FixtureEntropy},
    threshold::{DealerKeyShares, ThresholdContext, ThresholdSelection},
};

fn profile(exponent: u32) -> BgvProfile {
    BgvProfile::new(BgvParameters::derive(3, 5, exponent, (50, 4), (60, 2), 2).unwrap()).unwrap()
}

fn routing_budget() -> SlotLinearBudget {
    SlotLinearBudget {
        coefficient_bytes: 24,
        layout_metadata_bytes: 4096,
        preparation_scratch_bytes: 12,
        output_residue_bytes: 4096,
        scratch_residue_bytes: 32768,
    }
}

fn trace_budget() -> RelativeTraceBudget {
    RelativeTraceBudget {
        output_residue_bytes: 16384,
        scratch_residue_bytes: 8192,
        plaintext_bytes: 64,
    }
}

#[test]
fn all_effective_precisions_route_and_trace_under_one_master_key_set() {
    for exponent in [2, 3] {
        let profile = profile(exponent);
        let master = profile.master_plaintext();
        let encoding = SlotEncoding::new(&profile, master).unwrap();
        let decomposition = RelativeTraceDecompositionPlan::new(
            &profile,
            &profile.plan_fresh(master).unwrap(),
            trace_budget(),
        )
        .unwrap();
        let mut elements = encoding.broadcast_elements();
        elements.extend_from_slice(decomposition.galois_elements());
        elements.sort_unstable();
        elements.dedup();
        let mut entropy = FixtureEntropy::new([81 + exponent as u8; 64]);
        let keys = fixtures::key_set(&profile, &elements, &mut entropy).unwrap();
        for k in 1..=exponent {
            let precision = profile.plaintext_precision(k).unwrap();
            let t = precision.modulus();
            let encoding = SlotEncoding::new(&profile, precision).unwrap();
            assert_eq!(encoding.slots(), 2);
            let slots = [t - 1, 3];
            let packed = encoding.encode(&profile, &slots).unwrap();
            assert_eq!(packed.plaintext_precision(), precision);
            let input = fixtures::encrypt(&profile, &keys.public, &packed, &mut entropy).unwrap();
            let plan =
                SlotLinearPlan::<1, 1>::new(&profile, &[input.shape()], routing_budget()).unwrap();
            let matrix = [2, t - 1, 4, 3, 1, t - 2];
            let owner = SlotLinearMap::new(&plan, &matrix.map(|x| x as u32)).unwrap();
            let output = owner
                .evaluate(&profile, &plan, &[input], &keys.evaluation)
                .unwrap();
            let opened = keys.secret.decrypt(&profile, &output[0]).unwrap();
            assert_eq!(opened.plaintext_precision(), precision);
            let expected = [
                (2 * slots[0] + (t - 1) * slots[1] + 1) % t,
                (4 * slots[0] + 3 * slots[1] + t - 2) % t,
            ];
            assert_eq!(encoding.decode(&opened).unwrap().as_slice(), expected);
            let mask = encoding.public_prefix_mask(&profile, 1).unwrap();
            assert_eq!(mask.plaintext_precision(), precision);
            let masked = profile.mul_plain(&output[0], &mask).unwrap();
            assert_eq!(
                encoding
                    .decode(&keys.secret.decrypt(&profile, &masked).unwrap())
                    .unwrap()
                    .as_slice(),
                [expected[0], 0]
            );

            // Dense full-ring input, including coordinates off the slot stride.
            let coefficients: Vec<_> = (0..profile.degree())
                .map(|i| match i % 4 {
                    0 => t - 1,
                    1 => 0,
                    2 => t / 2,
                    _ => (i as u64 * 7) % t,
                })
                .collect();
            let plaintext = Plaintext::new(&profile, precision, coefficients.clone()).unwrap();
            let input =
                fixtures::encrypt(&profile, &keys.public, &plaintext, &mut entropy).unwrap();
            let decomposition =
                RelativeTraceDecompositionPlan::new(&profile, &input.shape(), trace_budget())
                    .unwrap();
            assert_eq!(decomposition.banks(), 4);
            let banks = decomposition
                .evaluate(&profile, &input, &keys.evaluation)
                .unwrap();
            for (r, bank) in banks.iter().enumerate() {
                let opened = keys.secret.decrypt(&profile, bank).unwrap();
                assert_eq!(opened.plaintext_precision(), precision);
                for (i, &value) in opened.coefficients().iter().enumerate() {
                    let expected = if i % 4 == 0 { coefficients[i + r] } else { 0 };
                    assert_eq!(
                        value, expected,
                        "E={exponent}, k={k}, bank={r}, coefficient={i}"
                    );
                }
                encoding.check_scalar_subalgebra(&opened).unwrap();
            }
            let recomposition = RelativeTraceRecompositionPlan::new(
                &profile,
                &banks.iter().map(Ciphertext::shape).collect::<Vec<_>>(),
                trace_budget(),
            )
            .unwrap();
            let restored = recomposition.evaluate(&profile, &banks).unwrap();
            assert_eq!(
                keys.secret
                    .decrypt(&profile, &restored)
                    .unwrap()
                    .coefficients(),
                coefficients
            );
            assert_eq!(restored.plaintext_precision(), precision);
        }
    }
}

#[test]
fn local_threshold_opening_preserves_effective_precision_at_every_level() {
    let profile = profile(3);
    let mut entropy = FixtureEntropy::new([91; 64]);
    let secret = fixtures::secret_key(&profile, &mut entropy).unwrap();
    let secret_words = fixtures::secret_coefficients(&secret);
    let public = fixtures::public_key(&profile, &secret, &mut entropy).unwrap();
    let context = ThresholdContext::new(&profile, [92; 32], 3, 2, vec![1, 2, 7]).unwrap();
    for k in 1..=3 {
        let precision = profile.plaintext_precision(k).unwrap();
        let coefficients: Vec<_> = (0..profile.degree())
            .map(|i| (i as u64 * 19 + 7) % precision.modulus())
            .collect();
        let plaintext = Plaintext::new(&profile, precision, coefficients.clone()).unwrap();
        let cipher = fixtures::encrypt(&profile, &public, &plaintext, &mut entropy).unwrap();
        for level in 0..=profile.top_level() {
            let cipher = profile.switch_to_level(&cipher, level).unwrap();
            let source = fixtures::secret_key_from_coefficients(&profile, &secret_words).unwrap();
            let mut dealer = fixtures::threshold_dealer(
                &profile,
                context.clone(),
                source,
                DealerKeyShares::required_secret_bytes(&profile, &context).unwrap(),
                &mut entropy,
            )
            .unwrap();
            let selection =
                ThresholdSelection::new(&profile, &context, *dealer.instance(), &[1, 7]).unwrap();
            let shares = [1, 7]
                .into_iter()
                .map(|party| {
                    dealer
                        .take_share(party)
                        .unwrap()
                        .try_into_subset_additive(&profile, &selection)
                        .unwrap()
                })
                .collect();
            let opened = selection
                .open_local(
                    &profile,
                    &cipher,
                    shares,
                    selection
                        .required_opening_secret_bytes(&profile, level)
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(opened.plaintext_precision(), precision);
            assert_eq!(opened.coefficients(), coefficients);
            assert_eq!(opened, secret.decrypt(&profile, &cipher).unwrap());
        }
    }
}
