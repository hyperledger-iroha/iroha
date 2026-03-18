use iroha_data_model::events::EventBox;
use iroha_data_model::prelude::DataEvent;

pub fn normalize_proof_filters(
    proof_backend: Option<Vec<String>>,
    proof_call_hash: Option<Vec<[u8; 32]>>,
    proof_envelope_hash: Option<Vec<[u8; 32]>>,
) -> (
    Option<Vec<String>>,
    Option<Vec<[u8; 32]>>,
    Option<Vec<[u8; 32]>>,
) {
    (
        normalize_vec(proof_backend),
        normalize_vec(proof_call_hash),
        normalize_vec(proof_envelope_hash),
    )
}

pub fn has_any_proof_filters(
    proof_backend: Option<&Vec<String>>,
    proof_call_hash: Option<&Vec<[u8; 32]>>,
    proof_envelope_hash: Option<&Vec<[u8; 32]>>,
) -> bool {
    has_values(proof_backend) || has_values(proof_call_hash) || has_values(proof_envelope_hash)
}

pub fn event_matches_proof_filters(
    event: &EventBox,
    proof_backend: Option<&Vec<String>>,
    proof_call_hash: Option<&Vec<[u8; 32]>>,
    proof_envelope_hash: Option<&Vec<[u8; 32]>>,
    proof_only: bool,
) -> bool {
    let proof_backend = proof_backend.filter(|values| !values.is_empty());
    let proof_call_hash = proof_call_hash.filter(|values| !values.is_empty());
    let proof_envelope_hash = proof_envelope_hash.filter(|values| !values.is_empty());
    let has_filters =
        proof_backend.is_some() || proof_call_hash.is_some() || proof_envelope_hash.is_some();

    let proof_event = match event {
        EventBox::Data(ev) => match ev.as_ref() {
            DataEvent::Proof(pe) => pe,
            _ => return !proof_only,
        },
        _ => return !proof_only,
    };

    if !has_filters {
        return true;
    }

    match proof_event {
        iroha_data_model::events::data::proof::ProofEvent::Verified(v) => {
            if let Some(backends) = proof_backend {
                if !backends.contains(&v.id.backend) {
                    return false;
                }
            }
            if let Some(call_hashes) = proof_call_hash {
                if !v
                    .call_hash
                    .as_ref()
                    .is_some_and(|hash| call_hashes.contains(hash))
                {
                    return false;
                }
            }
            if let Some(envelope_hashes) = proof_envelope_hash {
                if !v
                    .envelope_hash
                    .as_ref()
                    .is_some_and(|hash| envelope_hashes.contains(hash))
                {
                    return false;
                }
            }
            true
        }
        iroha_data_model::events::data::proof::ProofEvent::Rejected(r) => {
            if let Some(backends) = proof_backend {
                if !backends.contains(&r.id.backend) {
                    return false;
                }
            }
            if let Some(call_hashes) = proof_call_hash {
                if !r
                    .call_hash
                    .as_ref()
                    .is_some_and(|hash| call_hashes.contains(hash))
                {
                    return false;
                }
            }
            if let Some(envelope_hashes) = proof_envelope_hash {
                if !r
                    .envelope_hash
                    .as_ref()
                    .is_some_and(|hash| envelope_hashes.contains(hash))
                {
                    return false;
                }
            }
            true
        }
        iroha_data_model::events::data::proof::ProofEvent::Pruned(p) => {
            if proof_call_hash.is_some() || proof_envelope_hash.is_some() {
                return false;
            }
            if let Some(backends) = proof_backend {
                if !backends.contains(&p.backend) {
                    return false;
                }
            }
            true
        }
    }
}

fn normalize_vec<T>(value: Option<Vec<T>>) -> Option<Vec<T>> {
    match value {
        Some(values) if values.is_empty() => None,
        other => other,
    }
}

fn has_values<T>(value: Option<&Vec<T>>) -> bool {
    value.is_some_and(|values| !values.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        events::EventBox,
        events::data::proof::{
            ProofEvent, ProofPruneOrigin, ProofPruned, ProofRejected, ProofVerified,
        },
        events::time::{TimeEvent, TimeInterval},
        prelude::DataEvent,
        proof::ProofId,
    };

    fn proof_id(backend: &str, proof_hash: [u8; 32]) -> ProofId {
        ProofId {
            backend: backend.to_string(),
            proof_hash,
        }
    }

    fn verified_event(
        backend: &str,
        call_hash: Option<[u8; 32]>,
        envelope_hash: Option<[u8; 32]>,
    ) -> EventBox {
        EventBox::Data(
            DataEvent::Proof(ProofEvent::Verified(ProofVerified {
                id: proof_id(backend, [0x11; 32]),
                vk_ref: None,
                vk_commitment: None,
                call_hash,
                envelope_hash,
            }))
            .into(),
        )
    }

    fn rejected_event(backend: &str, call_hash: Option<[u8; 32]>) -> EventBox {
        EventBox::Data(
            DataEvent::Proof(ProofEvent::Rejected(ProofRejected {
                id: proof_id(backend, [0x22; 32]),
                vk_ref: None,
                vk_commitment: None,
                call_hash,
                envelope_hash: None,
            }))
            .into(),
        )
    }

    fn pruned_event(backend: &str) -> EventBox {
        let account = AccountId::of(KeyPair::random().public_key().clone());
        EventBox::Data(
            DataEvent::Proof(ProofEvent::Pruned(ProofPruned {
                backend: backend.to_string(),
                removed: vec![proof_id(backend, [0x33; 32])],
                remaining: 0,
                cap: 0,
                grace_blocks: 0,
                prune_batch: 0,
                pruned_at_height: 1,
                pruned_by: account,
                origin: ProofPruneOrigin::Manual,
            }))
            .into(),
        )
    }

    fn time_event() -> EventBox {
        EventBox::Time(TimeEvent {
            interval: TimeInterval {
                since_ms: 0,
                length_ms: 1,
            },
        })
    }

    #[test]
    fn proof_filters_match_verified_with_all_fields() {
        let event = verified_event("halo2/ipa", Some([0xAA; 32]), Some([0xBB; 32]));
        let proof_backend = Some(vec!["halo2/ipa".to_string()]);
        let proof_call_hash = Some(vec![[0xAA; 32]]);
        let proof_envelope_hash = Some(vec![[0xBB; 32]]);

        assert!(event_matches_proof_filters(
            &event,
            proof_backend.as_ref(),
            proof_call_hash.as_ref(),
            proof_envelope_hash.as_ref(),
            false,
        ));
    }

    #[test]
    fn proof_filters_drop_verified_without_call_hash() {
        let event = verified_event("halo2/ipa", None, Some([0xBB; 32]));
        let proof_call_hash = Some(vec![[0xAA; 32]]);

        assert!(!event_matches_proof_filters(
            &event,
            None,
            proof_call_hash.as_ref(),
            None,
            false,
        ));
    }

    #[test]
    fn proof_filters_drop_pruned_when_hash_filters_present() {
        let event = pruned_event("halo2/ipa");
        let proof_call_hash = Some(vec![[0xAA; 32]]);

        assert!(!event_matches_proof_filters(
            &event,
            None,
            proof_call_hash.as_ref(),
            None,
            false,
        ));
    }

    #[test]
    fn proof_filters_allow_pruned_with_backend_only() {
        let event = pruned_event("halo2/ipa");
        let proof_backend = Some(vec!["halo2/ipa".to_string()]);

        assert!(event_matches_proof_filters(
            &event,
            proof_backend.as_ref(),
            None,
            None,
            false,
        ));
    }

    #[test]
    fn proof_filters_gate_non_proof_events_when_proof_only() {
        let event = time_event();
        let proof_backend = Some(vec!["halo2/ipa".to_string()]);

        assert!(!event_matches_proof_filters(
            &event,
            proof_backend.as_ref(),
            None,
            None,
            true,
        ));
        assert!(event_matches_proof_filters(
            &event,
            proof_backend.as_ref(),
            None,
            None,
            false,
        ));
    }

    #[test]
    fn proof_filters_match_rejected_backend_only() {
        let event = rejected_event("halo2/ipa", Some([0x44; 32]));
        let proof_backend = Some(vec!["halo2/ipa".to_string()]);

        assert!(event_matches_proof_filters(
            &event,
            proof_backend.as_ref(),
            None,
            None,
            false,
        ));
    }
}
