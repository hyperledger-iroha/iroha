//! Original-codec-refusal ranking for the separately selected anonymous family.

use super::*;
use norito::core::DecodeAttemptError;

fn score(
    seed: &[u8; 64],
    domain: &[u8],
    account: &impl norito::codec::Encode,
) -> Result<[u8; 32], DecodeAttemptError> {
    struct Writer(Blake2b512);
    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    norito::core::classify_decode_attempt(|| {
        let mut writer = Writer(Blake2b512::new());
        writer.0.update(domain);
        writer.0.update(seed);
        // The exact old preimage is domain || seed || bare encode(account), without a frame.
        norito::codec::encode_adaptive_into(account, &mut writer)?;
        let digest = writer.0.finalize();
        let mut output = [0; 32];
        output.copy_from_slice(&digest[..32]);
        Ok(output)
    })
}

/// Derive the same bounded plan while retaining original account-encoding refusals.
/// The frozen baseline entrypoint and the common allocation/ranking policy are unchanged.
pub(crate) fn derive_bounded_body_plan_checked_v1(
    sizing: &dyn ParliamentDrawSizingV1,
    network_id: &NetworkId,
    pulse_height: u64,
    future_beacon: &[u8; 32],
    candidates: &[AccountId],
    bodies: &[ParliamentBody],
    maximum_cap: u32,
) -> Result<Option<ParliamentDrawPlan>, DecodeAttemptError> {
    let candidates: Vec<_> = candidates
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut rankings = BTreeMap::new();
    for body in bodies {
        let seed = sortition::compute_seed(
            network_id,
            pulse_height,
            future_beacon,
            body_seed_domain(*body),
        );
        let mut scored = Vec::with_capacity(candidates.len());
        for account in &candidates {
            scored.push((
                score(&seed, body_input_domain(*body), account)?,
                account.clone(),
            ));
        }
        scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        scored.dedup_by(|left, right| left.1 == right.1);
        rankings.insert(
            *body,
            scored.into_iter().map(|(_, account)| account).collect(),
        );
    }
    Ok(bounded_plan_from_rankings(
        sizing,
        &candidates,
        bodies,
        &rankings,
        pulse_height,
        maximum_cap,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    struct Sizing;
    impl ParliamentDrawSizingV1 for Sizing {
        fn target_seats(&self, _: ParliamentBody) -> usize {
            2
        }
        fn alternates(&self, _: ParliamentBody) -> usize {
            1
        }
    }

    #[test]
    fn checked_draw_preserves_original_scores_and_bounded_assignments() {
        let candidates: Vec<_> = (1..=4)
            .map(|seed| {
                AccountId::new(
                    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                        .public_key()
                        .clone(),
                )
            })
            .collect();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([3; 32])));
        for body in iroha_data_model::governance::types::PARLIAMENT_BODIES_V1 {
            for candidate in &candidates {
                assert_eq!(
                    score(&[9; 64], body_input_domain(body), candidate).unwrap(),
                    scored_output(&[9; 64], body_input_domain(body), candidate)
                );
            }
        }
        let bodies = [ParliamentBody::RulesCommittee, ParliamentBody::PolicyJury];
        for cap in 0..=2 {
            assert_eq!(
                derive_bounded_body_plan_checked_v1(
                    &Sizing,
                    &network,
                    17,
                    &[5; 32],
                    &candidates,
                    &bodies,
                    cap
                )
                .unwrap(),
                derive_bounded_body_plan_v1(
                    &Sizing,
                    &network,
                    17,
                    &[5; 32],
                    &candidates,
                    &bodies,
                    cap
                ),
            );
        }
    }

    #[test]
    fn checked_sortition_score_retains_original_encoder_scratch_refusal() {
        let accounts = std::collections::HashMap::from([(1_u64, 2_u64)]);
        let error = norito::core::with_decode_limits_scope(
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || score(&[3; 64], b"original-input", &accounts),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            norito::core::DecodeAttemptErrorKind::EnclosingLimit
        );
        let original = error.local_refusal().unwrap();
        let reducer =
            crate::governance::parliament_anonymous::AnonymousParliamentErrorV1::from(error);
        assert_eq!(reducer.original_codec_refusal(), Some(&original));
    }

    #[test]
    fn checked_candidate_snapshot_bounds_preserve_the_installed_verdict() {
        use crate::governance::parliament::{
            candidate_snapshot_fits_resource_bounds_checked_v1 as checked,
            candidate_snapshot_fits_resource_bounds_v1 as installed,
        };
        let account = AccountId::new(
            KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        for candidates in [Vec::new(), vec![account.clone(); 2]] {
            assert_eq!(checked(&candidates).unwrap(), installed(&candidates));
        }
        let excessive =
            vec![
                account;
                iroha_data_model::governance::types::MAX_PARLIAMENT_CITIZENS_V1 as usize + 1
            ];
        assert!(!checked(&excessive).unwrap());
        assert!(!installed(&excessive));
    }
}
