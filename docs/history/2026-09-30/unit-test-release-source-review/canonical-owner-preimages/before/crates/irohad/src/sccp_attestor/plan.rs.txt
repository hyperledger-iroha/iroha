//! Pure decisions of the SCCP attestor (`specs/sccp.md` §4.2.3, §4.8, §4.9).
//!
//! Everything here is a function of values read from committed state and the key directory,
//! so it is tested without a node: key classification, when to generate and register a key,
//! the exact `SetSccpBridgeKeyV1` the node submits, the signing order and the batches.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    isi::sccp::SetSccpBridgeKeyV1,
    sccp::{
        attestation::SccpAttestationSignatureV1, keys::SccpBridgeKeyBindingV1,
        keys::SccpBridgeKeyStateV1,
    },
};
use iroha_model_base::peer::PeerId;
use iroha_sccp::v1::{
    eip712::{BridgeKeyPopFieldsV1, peer_key_hash},
    key_file::SccpBridgeKeyFileV1,
};

/// Local keys sorted by their on-chain ownership (§4.9 step 2).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ClassifiedKeys {
    /// Addresses this peer owns (active, pending or retired): signing keys.
    pub(crate) owned: BTreeSet<[u8; 20]>,
    /// Unowned addresses, newest (`created_at_ms`) first: registration candidates.
    pub(crate) candidates: Vec<[u8; 20]>,
    /// Addresses another peer owns: ignored and reported.
    pub(crate) foreign: Vec<[u8; 20]>,
}

/// Classify local `keys` with `owner_of`, the on-chain owner of an address.
pub(crate) fn classify(
    keys: &[SccpBridgeKeyFileV1],
    me: &PeerId,
    owner_of: impl Fn(&[u8; 20]) -> Option<PeerId>,
) -> ClassifiedKeys {
    let mut classified = ClassifiedKeys::default();
    let mut candidates: Vec<(u64, [u8; 20])> = Vec::new();
    for key in keys {
        let Ok(address) = key.address() else {
            continue;
        };
        match owner_of(&address) {
            Some(owner) if &owner == me => {
                classified.owned.insert(address);
            }
            Some(_) => classified.foreign.push(address),
            None => candidates.push((key.created_at_ms(), address)),
        }
    }
    candidates.sort_by(|a, b| b.cmp(a));
    classified.candidates = candidates.into_iter().map(|(_, address)| address).collect();
    classified
}

/// Return the peer's active and pending key addresses.
pub(crate) fn live_addresses(state: &SccpBridgeKeyStateV1) -> BTreeSet<[u8; 20]> {
    state
        .active
        .iter()
        .chain(state.pending.iter())
        .map(|key| key.address)
        .collect()
}

/// Whether the node must generate a key: it is not barred and holds neither the peer's
/// active or pending key nor an unregistered candidate (§4.9 step 2).
pub(crate) fn needs_new_key(classified: &ClassifiedKeys, state: &SccpBridgeKeyStateV1) -> bool {
    if state.is_barred() || !classified.candidates.is_empty() {
        return false;
    }
    let live = live_addresses(state);
    !classified
        .owned
        .iter()
        .any(|address| live.contains(address))
}

/// The candidate to register, if the newest local key is not already the peer's active or
/// pending key (§4.9 step 3).
pub(crate) fn registration_candidate(
    classified: &ClassifiedKeys,
    state: &SccpBridgeKeyStateV1,
) -> Option<[u8; 20]> {
    if state.is_barred() {
        return None;
    }
    classified.candidates.first().copied()
}

/// Build the node's `SetSccpBridgeKeyV1` registering `key` for `peer_key_pair`'s peer
/// (§4.2.2): the binding is signed by the consensus key and the proof of possession by the
/// bridge key.
///
/// # Errors
///
/// Fails when the bridge key cannot produce its public key, address or signature.
pub(crate) fn registration(
    network_id: NetworkId,
    peer_key_pair: &KeyPair,
    key: &SccpBridgeKeyFileV1,
    activation_epoch: u64,
    binding_nonce: u64,
) -> eyre::Result<SetSccpBridgeKeyV1> {
    let peer = PeerId::new(peer_key_pair.public_key().clone());
    let public_key = key
        .public_key()
        .map_err(|error| eyre::eyre!("bridge key public key: {error}"))?;
    let address = key
        .address()
        .map_err(|error| eyre::eyre!("bridge key address: {error}"))?;
    let binding = SccpBridgeKeyBindingV1::new(
        network_id,
        peer.clone(),
        Some(public_key),
        activation_epoch,
        binding_nonce,
    );
    let peer_signature = SignatureOf::try_new(peer_key_pair.private_key(), &binding)
        .map_err(|error| eyre::eyre!("consensus-key binding signature: {error}"))?;
    let (_, peer_key_bytes) = peer.public_key().to_bytes();
    let pop_digest = BridgeKeyPopFieldsV1 {
        peer_key_hash: peer_key_hash(&peer_key_bytes),
        bridge_address: address,
        activation_epoch,
    }
    .digest(network_id.as_bytes());
    let key_pop = key
        .sign_digest(&pop_digest)
        .map_err(|error| eyre::eyre!("bridge key proof of possession: {error}"))?;
    Ok(SetSccpBridgeKeyV1 {
        peer,
        public_key: Some(public_key),
        activation_epoch,
        binding_nonce,
        peer_signature,
        key_pop: Some(key_pop),
    })
}

/// One subject a local key must sign (§4.9 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Duty {
    /// Subject height.
    pub(crate) height: u64,
    /// The key's member index in the subject's generation.
    pub(crate) signer_index: u8,
    /// The signing key's address.
    pub(crate) address: [u8; 20],
    /// Whether the subject is a rotation subject (handoff).
    pub(crate) rotation: bool,
    /// Subject block time.
    pub(crate) timestamp_ms: u64,
}

/// Order duties: rotation subjects first, then oldest first (§4.9 step 4).
pub(crate) fn order(mut duties: Vec<Duty>) -> Vec<Duty> {
    duties.sort_by_key(|duty| (!duty.rotation, duty.height, duty.signer_index));
    duties
}

/// Whether a rotation subject is dated further into the future than `max_drift_ms` from the
/// local wall clock `now_ms`; such subjects are refused because they would extend roster
/// validity (§4.9).
pub(crate) const fn future_dated(duty: &Duty, now_ms: u64, max_drift_ms: u64) -> bool {
    duty.rotation && duty.timestamp_ms > now_ms.saturating_add(max_drift_ms)
}

/// Split signed entries into one sorted batch per key of at most `max_entries` entries; only
/// the first batch of each key is submitted per block (§4.9 step 5).
pub(crate) fn batches(
    signed: Vec<([u8; 20], SccpAttestationSignatureV1)>,
    max_entries: usize,
) -> BTreeMap<[u8; 20], Vec<SccpAttestationSignatureV1>> {
    let mut by_key: BTreeMap<[u8; 20], Vec<SccpAttestationSignatureV1>> = BTreeMap::new();
    for (address, entry) in signed {
        by_key.entry(address).or_default().push(entry);
    }
    for entries in by_key.values_mut() {
        entries.sort_by_key(|entry| (entry.height, entry.signer_index));
        entries.dedup_by_key(|entry| (entry.height, entry.signer_index));
        entries.truncate(max_entries.max(1));
    }
    by_key
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Algorithm;
    use iroha_data_model::sccp::keys::{SccpBridgeKeyV1, SccpFaultRefV1};
    use iroha_sccp::v1::signature::verify_signature;

    fn key(seed: u8, created_at_ms: u64) -> SccpBridgeKeyFileV1 {
        SccpBridgeKeyFileV1::new([seed; 32], created_at_ms).expect("key")
    }

    fn peer(seed: u8) -> PeerId {
        PeerId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn bridge_key(address: [u8; 20]) -> SccpBridgeKeyV1 {
        SccpBridgeKeyV1 {
            public_key: [2; 33],
            address,
            activation_epoch: 1,
            registered_at_height: 1,
            faulted: false,
        }
    }

    #[test]
    fn keys_classify_by_owner_with_the_newest_candidate_first() {
        let keys = [key(1, 10), key(2, 30), key(3, 20), key(4, 5)];
        let address = |index: usize| keys[index].address().expect("address");
        let me = peer(1);
        let (mine, theirs) = (address(0), address(3));
        let classified = classify(&keys, &me, |candidate| {
            (*candidate == mine)
                .then(|| me.clone())
                .or_else(|| (*candidate == theirs).then(|| peer(2)))
        });
        assert_eq!(classified.owned, BTreeSet::from([mine]));
        assert_eq!(classified.candidates, vec![address(1), address(2)]);
        assert_eq!(classified.foreign, vec![theirs]);
    }

    #[test]
    fn keys_are_generated_only_without_a_live_key_or_candidate() {
        let mut state = SccpBridgeKeyStateV1::default();
        let empty = ClassifiedKeys::default();
        assert!(needs_new_key(&empty, &state));
        let candidate = ClassifiedKeys {
            candidates: vec![[1; 20]],
            ..ClassifiedKeys::default()
        };
        assert!(!needs_new_key(&candidate, &state));
        assert_eq!(registration_candidate(&candidate, &state), Some([1; 20]));

        state.active = Some(bridge_key([5; 20]));
        let owned = ClassifiedKeys {
            owned: BTreeSet::from([[5; 20]]),
            ..ClassifiedKeys::default()
        };
        assert!(!needs_new_key(&owned, &state));
        // A wiped store (the active key is gone locally) generates a replacement.
        assert!(needs_new_key(&empty, &state));
        // A retired owned key does not count as live.
        let retired = ClassifiedKeys {
            owned: BTreeSet::from([[9; 20]]),
            ..ClassifiedKeys::default()
        };
        assert!(needs_new_key(&retired, &state));

        state.barred = Some(SccpFaultRefV1 {
            address: [5; 20],
            height: 3,
        });
        assert!(!needs_new_key(&empty, &state));
        assert_eq!(registration_candidate(&candidate, &state), None);
    }

    #[test]
    fn registrations_verify_under_both_keys() {
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([4; 32])),
        );
        let consensus = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
        let bridge = key(8, 1);
        let instruction =
            registration(network_id, &consensus, &bridge, 3, 2).expect("registration");
        assert_eq!(
            instruction.peer,
            PeerId::new(consensus.public_key().clone())
        );
        assert_eq!(
            instruction.public_key,
            Some(bridge.public_key().expect("public key"))
        );
        assert_eq!(
            (instruction.activation_epoch, instruction.binding_nonce),
            (3, 2)
        );
        instruction
            .peer_signature
            .verify(consensus.public_key(), &instruction.binding(network_id))
            .expect("binding signature");
        let (_, peer_key_bytes) = instruction.peer.public_key().to_bytes();
        let digest = BridgeKeyPopFieldsV1 {
            peer_key_hash: peer_key_hash(&peer_key_bytes),
            bridge_address: bridge.address().expect("address"),
            activation_epoch: 3,
        }
        .digest(network_id.as_bytes());
        verify_signature(
            &digest,
            &instruction.key_pop.expect("pop"),
            &bridge.address().expect("address"),
        )
        .expect("proof of possession");
    }

    #[test]
    fn rotations_come_first_and_future_dated_rotations_are_refused() {
        let duty = |height: u64, rotation: bool| Duty {
            height,
            signer_index: 0,
            address: [1; 20],
            rotation,
            timestamp_ms: height * 1_000,
        };
        let ordered = order(vec![
            duty(5, false),
            duty(9, true),
            duty(2, false),
            duty(7, true),
        ]);
        let heights: Vec<_> = ordered.iter().map(|duty| duty.height).collect();
        assert_eq!(heights, vec![7, 9, 2, 5]);
        assert!(future_dated(&duty(9, true), 1_000, 5_000));
        assert!(!future_dated(&duty(9, true), 4_000, 5_000));
        assert!(!future_dated(&duty(9, false), 0, 0));
    }

    #[test]
    fn batches_are_per_key_sorted_and_bounded() {
        let entry = |height: u64, signer_index: u8| SccpAttestationSignatureV1 {
            height,
            signer_index,
            signature: [0; 65],
        };
        let batches = batches(
            vec![
                ([1; 20], entry(9, 0)),
                ([2; 20], entry(3, 1)),
                ([1; 20], entry(4, 0)),
                ([1; 20], entry(6, 0)),
                ([1; 20], entry(4, 0)),
            ],
            2,
        );
        assert_eq!(batches.len(), 2);
        let first: Vec<_> = batches[&[1; 20]].iter().map(|entry| entry.height).collect();
        assert_eq!(first, vec![4, 6]);
        assert_eq!(batches[&[2; 20]].len(), 1);
    }
}
