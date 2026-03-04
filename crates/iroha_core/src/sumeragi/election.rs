//! Validator election helpers for `NPoS` epoch transitions.
//!
//! Deterministically derives an ordered validator set from a candidate roster and
//! a shared seed. The selected set is recorded for audit purposes via
//! [`ValidatorElectionOutcome`].

use std::{collections::BTreeMap, str::FromStr};

use iroha_crypto::{
    HashOf,
    blake2::{Blake2b512, Digest},
};
use iroha_data_model::{
    consensus::{ValidatorElectionOutcome, ValidatorElectionParameters, ValidatorTieBreak},
    name::Name,
    nexus::staking::{PublicLaneStakeShare, PublicLaneValidatorRecord},
    peer::PeerId,
};
use iroha_primitives::numeric::Numeric;
use norito::json::Value as JsonValue;
use rust_decimal::Decimal;

/// Stake/context snapshot for a validator candidate.
#[derive(Debug, Clone)]
pub struct CandidateProfile {
    /// Candidate peer identifier.
    pub peer_id: PeerId,
    /// Optional staking record associated with the peer.
    pub record: Option<PublicLaneValidatorRecord>,
    /// Bonded stake shares attributed to the candidate (per staker).
    pub stake_shares: Vec<PublicLaneStakeShare>,
}

/// Filter candidates using the supplied election parameters.
#[must_use]
pub fn filter_candidates_with_constraints(
    profiles: Vec<CandidateProfile>,
    params: &ValidatorElectionParameters,
) -> Vec<CandidateProfile> {
    profiles
        .into_iter()
        .filter(|profile| candidate_satisfies_constraints(profile, params))
        .collect()
}

fn candidate_satisfies_constraints(
    profile: &CandidateProfile,
    params: &ValidatorElectionParameters,
) -> bool {
    let Some(record) = &profile.record else {
        // Council snapshots (no staking record) are only allowed when constraints are disabled.
        return params.min_self_bond == 0
            && params.min_nomination_bond == 0
            && params.max_nominator_concentration_pct == 0;
    };

    if params.min_self_bond > 0 && !numeric_at_least(&record.self_stake, params.min_self_bond) {
        return false;
    }

    if params.min_nomination_bond > 0
        && has_undersized_nomination(&record.stake_account, &profile.stake_shares, params)
    {
        return false;
    }

    if params.max_nominator_concentration_pct > 0
        && exceeds_nominator_concentration(record, &profile.stake_shares, params)
    {
        return false;
    }

    true
}

fn numeric_at_least(value: &Numeric, threshold: u64) -> bool {
    let Some(dec) = numeric_to_decimal(value) else {
        return false;
    };
    dec >= Decimal::from(threshold)
}

fn has_undersized_nomination(
    stake_account: &iroha_data_model::account::AccountId,
    shares: &[PublicLaneStakeShare],
    params: &ValidatorElectionParameters,
) -> bool {
    let min_bond = Decimal::from(params.min_nomination_bond);
    shares.iter().any(|share| {
        if &share.staker == stake_account {
            return false;
        }
        numeric_to_decimal(&share.bonded).is_some_and(|bonded| bonded < min_bond)
    })
}

fn exceeds_nominator_concentration(
    record: &PublicLaneValidatorRecord,
    shares: &[PublicLaneStakeShare],
    params: &ValidatorElectionParameters,
) -> bool {
    let Some(total) = numeric_to_decimal(&record.total_stake) else {
        return true;
    };
    if total.is_zero() {
        return true;
    }

    let mut max_nom_share = Decimal::ZERO;
    for share in shares {
        if share.staker == record.stake_account {
            continue;
        }
        if let Some(bonded) = numeric_to_decimal(&share.bonded) {
            if bonded > max_nom_share {
                max_nom_share = bonded;
            }
        } else {
            return true;
        }
    }

    if max_nom_share.is_zero() {
        return false;
    }

    let pct = (max_nom_share * Decimal::from(100u8)) / total;
    pct > Decimal::from(params.max_nominator_concentration_pct)
}

fn numeric_to_decimal(n: &Numeric) -> Option<Decimal> {
    let mantissa = n.try_mantissa_i128()?;
    let mantissa_i64 = i64::try_from(mantissa).ok()?;
    Some(Decimal::new(mantissa_i64, n.scale()))
}

/// Elect an ordered validator set for the supplied epoch.
///
/// Candidates are scored using `Blake2b(seed || peer_public_key)`; the lowest
/// score wins. `max_validators == 0` preserves all candidates.
#[must_use]
pub fn elect_validator_set(
    epoch: u64,
    snapshot_height: u64,
    seed: [u8; 32],
    candidates: Vec<CandidateProfile>,
    params: ValidatorElectionParameters,
) -> ValidatorElectionOutcome {
    let mut scored: Vec<(CandidateProfile, ValidatorTieBreak)> = candidates
        .into_iter()
        .map(|profile| {
            let tie = ValidatorTieBreak {
                peer_id: profile.peer_id.clone(),
                score: score_peer(seed, &profile.peer_id),
            };
            (profile, tie)
        })
        .collect();

    scored.sort_by(|a, b| {
        a.1.score
            .cmp(&b.1.score)
            .then_with(|| a.1.peer_id.cmp(&b.1.peer_id))
    });

    let base_take = if params.max_validators == 0 {
        scored.len()
    } else {
        usize::try_from(params.max_validators)
            .unwrap_or(usize::MAX)
            .min(scored.len())
    };
    let band_extra = if params.seat_band_pct == 0 {
        0
    } else {
        base_take
            .saturating_mul(params.seat_band_pct as usize)
            .div_ceil(100)
    };
    let desired = base_take.saturating_add(band_extra).min(scored.len());
    let required_validators = if params.max_validators == 0 {
        desired
    } else {
        base_take
    };

    let per_entity_cap = if params.max_entity_correlation_pct == 0 {
        usize::MAX
    } else {
        let cap_basis = if params.seat_band_pct == 0 {
            base_take
        } else {
            desired
        };
        let cap = cap_basis.saturating_mul(params.max_entity_correlation_pct as usize) / 100;
        cap.max(1)
    };

    let mut entity_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut validator_set: Vec<PeerId> = Vec::new();
    let mut deferred_due_to_correlation: Vec<PeerId> = Vec::new();
    for (profile, tie) in &scored {
        if validator_set.len() >= desired {
            break;
        }
        let entity_key = entity_key(profile.record.as_ref(), &tie.peer_id);
        let count = entity_counts.entry(entity_key).or_insert(0);
        if *count >= per_entity_cap {
            if validator_set.len() < required_validators {
                deferred_due_to_correlation.push(tie.peer_id.clone());
            }
            continue;
        }
        *count += 1;
        validator_set.push(tie.peer_id.clone());
    }
    if validator_set.len() < required_validators {
        for peer_id in deferred_due_to_correlation {
            if validator_set.len() >= required_validators {
                break;
            }
            validator_set.push(peer_id);
        }
    }

    let tie_break: Vec<ValidatorTieBreak> = scored.into_iter().map(|(_, tie)| tie).collect();

    let rejection_reason = if tie_break.is_empty() {
        Some("no candidates in stake snapshot".to_owned())
    } else if validator_set.is_empty() {
        Some("candidates rejected by entity correlation limit".to_owned())
    } else if params.max_validators != 0 && validator_set.len() < required_validators {
        Some("insufficient distinct entities to satisfy validator target".to_owned())
    } else {
        None
    };

    ValidatorElectionOutcome {
        epoch,
        snapshot_height,
        seed,
        candidates_total: tie_break.len().try_into().unwrap_or(u32::MAX),
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        params,
        rejection_reason,
        tie_break,
    }
}

fn score_peer(seed: [u8; 32], peer: &PeerId) -> [u8; 32] {
    let (_, pk_bytes) = peer.public_key().to_bytes();
    let mut hasher = Blake2b512::new();
    hasher.update(seed);
    hasher.update(pk_bytes);
    let digest = hasher.finalize();
    let mut score = [0u8; 32];
    score.copy_from_slice(&digest[..32]);
    score
}

fn entity_key(record: Option<&PublicLaneValidatorRecord>, peer_id: &PeerId) -> String {
    if let Some(rec) = record {
        if let Ok(name) = Name::from_str("entity") {
            if let Some(json) = rec.metadata.get(&name) {
                if let Ok(JsonValue::String(val)) = json.try_into_any() {
                    return val;
                }
            }
        }
        return rec.validator.to_string();
    }
    peer_id.to_string()
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, PublicKey};
    use iroha_data_model::{
        account::AccountId,
        nexus::LaneId,
        prelude::{DomainId, Json, Metadata},
    };

    use super::*;

    fn parse_public_key(hex: &str) -> PublicKey {
        match PublicKey::from_hex(Algorithm::Ed25519, hex) {
            Ok(pk) => pk,
            Err(err) => {
                // Some historical fixtures embed an algorithm prefix; strip it if present.
                if hex.len() > 6 {
                    let trimmed = &hex[6..];
                    PublicKey::from_hex(Algorithm::Ed25519, trimmed)
                        .unwrap_or_else(|_| panic!("public key parses: {err}"))
                } else {
                    panic!("public key parses: {err}")
                }
            }
        }
    }

    fn peer_from_hex(hex: &str) -> PeerId {
        PeerId::from(parse_public_key(hex))
    }

    fn account_for_peer(domain: &DomainId, peer: &PeerId) -> AccountId {
        AccountId::of(domain.clone(), peer.public_key().clone())
    }

    #[test]
    fn election_is_deterministic_and_capped() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peers = [
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"),
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"),
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B"),
        ];
        let profiles: Vec<CandidateProfile> = peers
            .iter()
            .map(|peer| CandidateProfile {
                peer_id: peer.clone(),
                record: Some(sample_record(
                    peer,
                    2_000,
                    2_000,
                    &account_for_peer(&domain, peer),
                )),
                stake_shares: Vec::new(),
            })
            .collect();
        let params = ValidatorElectionParameters {
            max_validators: 2,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 25,
            seat_band_pct: 0,
            max_entity_correlation_pct: 25,
            finality_margin_blocks: 8,
        };
        let seed = [0xAA; 32];
        let out_a = elect_validator_set(5, 12, seed, profiles.clone(), params);
        let out_b = elect_validator_set(5, 12, seed, profiles, params);

        assert_eq!(out_a.validator_set.len(), 2);
        assert_eq!(out_b.validator_set, out_a.validator_set);
        assert_eq!(out_a.candidates_total, 3);
        assert!(out_a.rejection_reason.is_none());
        assert_eq!(out_a.validator_set_hash, HashOf::new(&out_a.validator_set));
        assert_eq!(out_a.tie_break.len(), 3);
    }

    #[test]
    fn election_is_deterministic_under_input_reordering() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peers = [
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"),
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"),
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B"),
            peer_from_hex("ed012009AF095595D88C116C3CC3AE2435999040A48DABFEA6985F3B6F7E335A8925B2"),
        ];

        let profiles: Vec<CandidateProfile> = peers
            .iter()
            .map(|peer| CandidateProfile {
                peer_id: peer.clone(),
                record: Some(sample_record(
                    peer,
                    5_000,
                    5_000,
                    &account_for_peer(&domain, peer),
                )),
                stake_shares: Vec::new(),
            })
            .collect();
        let mut reversed = profiles.clone();
        reversed.reverse();

        let params = ValidatorElectionParameters {
            max_validators: 3,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 0,
            seat_band_pct: 25,
            max_entity_correlation_pct: 100,
            finality_margin_blocks: 8,
        };
        let seed = [0x42; 32];
        let forward = elect_validator_set(7, 15, seed, profiles, params);
        let backward = elect_validator_set(7, 15, seed, reversed, params);

        assert_eq!(
            forward.validator_set, backward.validator_set,
            "validator selection must be stable across input orderings"
        );
        assert_eq!(forward.tie_break, backward.tie_break);
        assert_eq!(forward.validator_set_hash, backward.validator_set_hash);
        assert_eq!(forward.rejection_reason, backward.rejection_reason);
    }

    #[test]
    fn election_handles_empty_roster() {
        let params = ValidatorElectionParameters {
            max_validators: 0,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 25,
            seat_band_pct: 5,
            max_entity_correlation_pct: 25,
            finality_margin_blocks: 8,
        };
        let out = elect_validator_set(1, 1, [0u8; 32], Vec::new(), params);
        assert!(out.validator_set.is_empty());
        assert!(out.rejection_reason.is_some());
        assert_eq!(out.candidates_total, 0);
    }

    fn sample_record(
        peer: &PeerId,
        total: u64,
        self_stake: u64,
        stake_account: &AccountId,
    ) -> PublicLaneValidatorRecord {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("entity").expect("entity key"),
            Json::new(peer.to_string()),
        );
        PublicLaneValidatorRecord {
            lane_id: LaneId::SINGLE,
            validator: stake_account.clone(),
            stake_account: stake_account.clone(),
            total_stake: Numeric::new(total, 0),
            self_stake: Numeric::new(self_stake, 0),
            metadata,
            status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        }
    }

    #[test]
    fn candidates_below_self_bond_filtered() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let pk = parse_public_key(
            "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
        );
        let account = AccountId::of(domain, pk.clone());
        let peer = PeerId::from(pk);

        let profiles = vec![CandidateProfile {
            peer_id: peer.clone(),
            record: Some(sample_record(&peer, 500, 400, &account)),
            stake_shares: Vec::new(),
        }];
        let params = ValidatorElectionParameters {
            max_validators: 1,
            min_self_bond: 1_000,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 25,
            seat_band_pct: 5,
            max_entity_correlation_pct: 25,
            finality_margin_blocks: 8,
        };

        let filtered = filter_candidates_with_constraints(profiles, &params);
        assert!(filtered.is_empty(), "candidate should be rejected");
    }

    #[test]
    fn candidates_violating_concentration_filtered() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let pk = parse_public_key(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
        );
        let account = AccountId::of(domain.clone(), pk.clone());
        let peer = PeerId::from(pk);
        let nominator = AccountId::of(
            domain,
            parse_public_key(
                "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B",
            ),
        );

        let record = sample_record(&peer, 1_000, 200, &account);
        let shares = vec![PublicLaneStakeShare {
            lane_id: LaneId::SINGLE,
            validator: account.clone(),
            staker: nominator,
            bonded: Numeric::new(900, 0),
            pending_unbonds: BTreeMap::default(),
            metadata: Metadata::default(),
        }];
        let profiles = vec![CandidateProfile {
            peer_id: peer.clone(),
            record: Some(record),
            stake_shares: shares,
        }];
        let params = ValidatorElectionParameters {
            max_validators: 1,
            min_self_bond: 100,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 50,
            seat_band_pct: 5,
            max_entity_correlation_pct: 25,
            finality_margin_blocks: 8,
        };

        let filtered = filter_candidates_with_constraints(profiles, &params);
        assert!(
            filtered.is_empty(),
            "single nominator exceeding 50% should be rejected"
        );
    }

    #[test]
    fn candidates_respect_min_nomination_bond() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let pk = parse_public_key(
            "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B",
        );
        let account = AccountId::of(domain.clone(), pk.clone());
        let peer = PeerId::from(pk);
        let nominator = AccountId::of(
            domain,
            parse_public_key(
                "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
            ),
        );

        let record = sample_record(&peer, 1_000, 800, &account);
        let shares = vec![PublicLaneStakeShare {
            lane_id: LaneId::SINGLE,
            validator: account.clone(),
            staker: nominator,
            bonded: Numeric::new(50, 0),
            pending_unbonds: BTreeMap::default(),
            metadata: Metadata::default(),
        }];
        let profiles = vec![CandidateProfile {
            peer_id: peer.clone(),
            record: Some(record),
            stake_shares: shares,
        }];
        let params = ValidatorElectionParameters {
            max_validators: 1,
            min_self_bond: 100,
            min_nomination_bond: 100,
            max_nominator_concentration_pct: 90,
            seat_band_pct: 5,
            max_entity_correlation_pct: 25,
            finality_margin_blocks: 8,
        };

        let filtered = filter_candidates_with_constraints(profiles, &params);
        assert!(
            filtered.is_empty(),
            "nomination below minimum bond should be rejected"
        );
    }

    #[test]
    fn seat_band_allows_extra_validators_and_correlation_limits_entities() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peer_a =
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
        let peer_b =
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B");
        let peer_c =
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4");

        let account_a = AccountId::of(domain.clone(), peer_a.public_key().clone());
        let _account_b = AccountId::of(domain.clone(), peer_b.public_key().clone());
        let account_c = AccountId::of(domain, peer_c.public_key().clone());

        let record = |acct: &AccountId| PublicLaneValidatorRecord {
            lane_id: LaneId::SINGLE,
            validator: acct.clone(),
            stake_account: acct.clone(),
            total_stake: Numeric::new(10_000, 0),
            self_stake: Numeric::new(10_000, 0),
            metadata: Metadata::default(),
            status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };

        let profiles = vec![
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record(&account_a)),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record(&account_a)), // same entity as peer_a
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_c.clone(),
                record: Some(record(&account_c)),
                stake_shares: Vec::new(),
            },
        ];

        let params = ValidatorElectionParameters {
            max_validators: 2,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 100,
            seat_band_pct: 50,              // allow one extra seat
            max_entity_correlation_pct: 50, // cap per-entity to 50% of target set
            finality_margin_blocks: 8,
        };

        let outcome = elect_validator_set(1, 10, [0x11; 32], profiles.clone(), params);
        assert_eq!(
            outcome.validator_set.len(),
            2,
            "band permits up to 3, entity cap trims to 2"
        );
        assert!(
            outcome.validator_set.iter().any(|p| p == &peer_a)
                || outcome.validator_set.iter().any(|p| p == &peer_b),
            "one of the correlated peers should be selected"
        );
        assert!(
            outcome.validator_set.iter().any(|p| p == &peer_c),
            "non-correlated peer should remain"
        );
        let mut entity_counts = BTreeMap::new();
        for peer in &outcome.validator_set {
            let entity = if peer == &peer_a || peer == &peer_b {
                "acme"
            } else {
                "zeon"
            };
            *entity_counts.entry(entity).or_insert(0) += 1;
        }
        assert_eq!(
            entity_counts.get("acme"),
            Some(&1),
            "correlation cap should trim the second acme seat despite the band"
        );
        assert_eq!(entity_counts.get("zeon"), Some(&1));
        assert!(outcome.rejection_reason.is_none());
    }

    #[test]
    fn seat_band_trimmed_is_deterministic() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peer_a =
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
        let peer_b =
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B");
        let peer_c =
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4");

        let account_a = AccountId::of(domain.clone(), peer_a.public_key().clone());
        let account_c = AccountId::of(domain, peer_c.public_key().clone());

        let record = |acct: &AccountId| PublicLaneValidatorRecord {
            lane_id: LaneId::SINGLE,
            validator: acct.clone(),
            stake_account: acct.clone(),
            total_stake: Numeric::new(10_000, 0),
            self_stake: Numeric::new(10_000, 0),
            metadata: Metadata::default(),
            status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };

        let profiles_a = vec![
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record(&account_a)),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record(&account_a)), // same entity as peer_a
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_c.clone(),
                record: Some(record(&account_c)),
                stake_shares: Vec::new(),
            },
        ];
        let profiles_b = vec![
            CandidateProfile {
                peer_id: peer_c.clone(),
                record: Some(record(&account_c)),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record(&account_a)), // same entity as peer_a
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record(&account_a)),
                stake_shares: Vec::new(),
            },
        ];

        let params = ValidatorElectionParameters {
            max_validators: 2,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 100,
            seat_band_pct: 50,              // allow one extra seat
            max_entity_correlation_pct: 50, // cap per-entity to 50% of target set
            finality_margin_blocks: 8,
        };

        let seed = [0x77; 32];
        let outcome_a = elect_validator_set(2, 20, seed, profiles_a, params);
        let outcome_b = elect_validator_set(2, 20, seed, profiles_b, params);

        assert_eq!(outcome_a.validator_set, outcome_b.validator_set);
        assert_eq!(outcome_a.tie_break, outcome_b.tie_break);
        assert_eq!(
            outcome_a.validator_set.len(),
            2,
            "entity correlation cap should trim the banded seat"
        );
        assert!(
            outcome_a
                .validator_set
                .iter()
                .any(|peer| peer == &peer_a || peer == &peer_b),
            "one of the correlated peers should be selected"
        );
        assert!(
            outcome_a.validator_set.iter().any(|peer| peer == &peer_c),
            "non-correlated peer should remain"
        );
        assert!(outcome_a.rejection_reason.is_none());
    }

    #[test]
    fn seat_band_fractional_entity_cap_fills_base_target() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peer_a =
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
        let peer_b =
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B");
        let peer_c =
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4");

        let record_with_entity = |peer: &PeerId, entity: &str| {
            let account = account_for_peer(&domain, peer);
            let mut metadata = Metadata::default();
            metadata.insert(Name::from_str("entity").unwrap(), Json::new(entity));
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: account.clone(),
                stake_account: account,
                total_stake: Numeric::new(10_000, 0),
                self_stake: Numeric::new(10_000, 0),
                metadata,
                status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            }
        };

        let profiles = vec![
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record_with_entity(&peer_a, "acme")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record_with_entity(&peer_b, "acme")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_c.clone(),
                record: Some(record_with_entity(&peer_c, "zeon")),
                stake_shares: Vec::new(),
            },
        ];

        let params = ValidatorElectionParameters {
            max_validators: 3,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 100,
            seat_band_pct: 50,
            max_entity_correlation_pct: 30, // floor(5 * 0.30) = 1 per entity
            finality_margin_blocks: 8,
        };

        let outcome = elect_validator_set(2, 20, [0x99; 32], profiles, params);
        assert_eq!(outcome.validator_set.len(), 3);
        assert!(outcome.rejection_reason.is_none());
        let entity_counts = outcome
            .validator_set
            .iter()
            .fold(BTreeMap::new(), |mut acc, peer| {
                let label = if peer == &peer_a || peer == &peer_b {
                    "acme"
                } else {
                    "zeon"
                };
                *acc.entry(label).or_insert(0) += 1;
                acc
            });
        assert_eq!(entity_counts.get("acme"), Some(&2));
        assert_eq!(entity_counts.get("zeon"), Some(&1));
    }

    #[test]
    fn election_deterministic_with_seat_band_and_permuted_candidates() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peer_a =
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4");
        let peer_b =
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
        let peer_c =
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B");
        let peer_d =
            peer_from_hex("ed0120ED8FBA0C4978E7E51A7AE3229F32238A0674052A509F1F8C8236DA0D7B6614A7");

        let account_a = AccountId::of(domain.clone(), peer_a.public_key().clone());
        let account_b = AccountId::of(domain.clone(), peer_b.public_key().clone());
        let account_c = AccountId::of(domain.clone(), peer_c.public_key().clone());
        let account_d = AccountId::of(domain.clone(), peer_d.public_key().clone());

        let record_with_entity = |account: &AccountId, entity: &str| {
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("entity").expect("entity key"),
                Json::new(entity.to_owned()),
            );
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: account.clone(),
                stake_account: account.clone(),
                total_stake: Numeric::new(5_000, 0),
                self_stake: Numeric::new(5_000, 0),
                metadata,
                status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            }
        };

        let profiles = vec![
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record_with_entity(&account_a, "acme")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record_with_entity(&account_b, "acme")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_c.clone(),
                record: Some(record_with_entity(&account_c, "solo")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_d.clone(),
                record: Some(record_with_entity(&account_d, "solo-2")),
                stake_shares: Vec::new(),
            },
        ];
        let mut reversed = profiles.clone();
        reversed.reverse();

        let params = ValidatorElectionParameters {
            max_validators: 2,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 100,
            seat_band_pct: 50,              // allow one extra seat
            max_entity_correlation_pct: 50, // cap correlated entities to half the target set
            finality_margin_blocks: 8,
        };
        let seed = [0x24; 32];

        let outcome_a = elect_validator_set(3, 10, seed, profiles, params);
        let outcome_b = elect_validator_set(3, 10, seed, reversed, params);

        assert_eq!(
            outcome_a.validator_set.len(),
            3,
            "seat band should add an extra seat on top of the base target"
        );
        assert_eq!(
            outcome_a.validator_set, outcome_b.validator_set,
            "candidate ordering must not affect deterministic selection"
        );
        assert_eq!(
            outcome_a.tie_break.len(),
            4,
            "tie-break list should include every candidate"
        );
        assert!(outcome_a.rejection_reason.is_none());
    }

    #[test]
    fn correlation_shortfall_fills_base_target() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peer_a =
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
        let peer_b =
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B");

        let record = |peer: &PeerId| {
            let account = account_for_peer(&domain, peer);
            let mut metadata = Metadata::default();
            metadata.insert(Name::from_str("entity").unwrap(), Json::new("acme"));
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: account.clone(),
                stake_account: account,
                total_stake: Numeric::new(10_000, 0),
                self_stake: Numeric::new(10_000, 0),
                metadata,
                status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            }
        };

        let profiles = vec![
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record(&peer_a)),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record(&peer_b)),
                stake_shares: Vec::new(),
            },
        ];

        let params = ValidatorElectionParameters {
            max_validators: 2,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 0,
            seat_band_pct: 0,
            max_entity_correlation_pct: 50,
            finality_margin_blocks: 8,
        };

        let outcome = elect_validator_set(1, 1, [0x33; 32], profiles, params);
        assert_eq!(outcome.validator_set.len(), 2);
        assert!(outcome.rejection_reason.is_none());
    }

    #[test]
    fn seat_band_scales_entity_cap() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peer_a =
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
        let peer_b =
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B");
        let peer_c =
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4");
        let peer_d =
            peer_from_hex("ed012009AF095595D88C116C3CC3AE2435999040A48DABFEA6985F3B6F7E335A8925B2");

        let record = |peer: &PeerId, entity: &str| {
            let account = account_for_peer(&domain, peer);
            let mut metadata = Metadata::default();
            metadata.insert(Name::from_str("entity").unwrap(), Json::new(entity));
            PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: account.clone(),
                stake_account: account,
                total_stake: Numeric::new(5_000, 0),
                self_stake: Numeric::new(5_000, 0),
                metadata,
                status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            }
        };

        let profiles = vec![
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record(&peer_a, "acme")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record(&peer_b, "acme")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_c.clone(),
                record: Some(record(&peer_c, "zeon")),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_d.clone(),
                record: Some(record(&peer_d, "zeon")),
                stake_shares: Vec::new(),
            },
        ];

        let params = ValidatorElectionParameters {
            max_validators: 2,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 0,
            seat_band_pct: 100, // target 4 validators
            max_entity_correlation_pct: 50,
            finality_margin_blocks: 8,
        };

        let outcome = elect_validator_set(1, 10, [0x44; 32], profiles, params);
        assert_eq!(outcome.validator_set.len(), 4);
        assert!(outcome.rejection_reason.is_none());
        let entity_counts = outcome.validator_set.iter().fold(
            BTreeMap::new(),
            |mut acc: BTreeMap<&str, usize>, peer| {
                let label = if peer == &peer_a || peer == &peer_b {
                    "acme"
                } else {
                    "zeon"
                };
                *acc.entry(label).or_insert(0) += 1;
                acc
            },
        );
        assert_eq!(entity_counts.get("acme"), Some(&2));
        assert_eq!(entity_counts.get("zeon"), Some(&2));
    }

    #[test]
    fn correlation_uses_entity_metadata_when_present() {
        let domain = DomainId::from_str("wonderland").unwrap();
        let peer_a =
            peer_from_hex("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
        let peer_b =
            peer_from_hex("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4");
        let peer_c =
            peer_from_hex("ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B");
        let account_a = account_for_peer(&domain, &peer_a);
        let account_b = account_for_peer(&domain, &peer_b);
        let account_c = account_for_peer(&domain, &peer_c);

        let mut meta = Metadata::default();
        meta.insert(Name::from_str("entity").unwrap(), Json::new("acme"));

        let record_with_entity = |acct: &AccountId| PublicLaneValidatorRecord {
            lane_id: LaneId::SINGLE,
            validator: acct.clone(),
            stake_account: acct.clone(),
            total_stake: Numeric::new(10_000, 0),
            self_stake: Numeric::new(10_000, 0),
            metadata: meta.clone(),
            status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };

        let profiles = vec![
            CandidateProfile {
                peer_id: peer_a.clone(),
                record: Some(record_with_entity(&account_a)),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_b.clone(),
                record: Some(record_with_entity(&account_b)),
                stake_shares: Vec::new(),
            },
            CandidateProfile {
                peer_id: peer_c.clone(),
                record: Some(sample_record(&peer_c, 10_000, 10_000, &account_c)),
                stake_shares: Vec::new(),
            },
        ];

        let params = ValidatorElectionParameters {
            max_validators: 2,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 100,
            seat_band_pct: 0,
            max_entity_correlation_pct: 50,
            finality_margin_blocks: 8,
        };

        let outcome = elect_validator_set(1, 1, [0x22; 32], profiles, params);
        assert_eq!(
            outcome.validator_set.len(),
            2,
            "base validator target should still be filled"
        );
        assert!(
            outcome.validator_set.iter().any(|peer| peer == &peer_c),
            "entity tag should still cap acme to one seat in the capped pass"
        );
    }
}
