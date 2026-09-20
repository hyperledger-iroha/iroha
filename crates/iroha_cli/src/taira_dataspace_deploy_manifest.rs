//! Generate the native Taira committee manifest from independently authenticated genesis.

use super::{MAX_BYTES, finality, require};
use eyre::{Result, WrapErr};
use iroha_data_model::{
    account::AccountId,
    isi::{
        ActivatePublicLaneValidator, ExitPublicLaneValidator, InstructionBox,
        RebindPublicLaneValidatorPeer, RegisterPublicLaneValidator, SlashPublicLaneValidator,
    },
    nexus::{
        MAX_NEXUS_RUNTIME_MANIFEST_BYTES, NativeLaneManifestV1, NativeLaneValidatorBindingV1,
        RuntimeLaneManifestV1,
    },
    transaction::Executable,
};
use iroha_model_base::{peer::PeerId, topology::LaneId};
use iroha_primitives::json::Json;
use norito::json;
use std::collections::{BTreeMap, BTreeSet};

const VALIDATORS: usize = 4;
const QUORUM: u32 = 3;

/// Produce one bounded typed manifest after the caller has validated `trust`.
///
/// Genesis is the account-to-peer authority; selected trust records supply only
/// each corresponding Torii endpoint. Live HTTP responses cannot choose members.
pub(super) fn generate(alias: &str, trust: &finality::TrustV1) -> Result<Json> {
    require(
        !alias.contains('.') && iroha_model_base::name::canonicalize_domain_label(alias)? == alias,
        "native lane manifest requires the canonical dataspace alias",
    )?;
    require(
        trust.genesis_signed_wire_hex.len() <= MAX_BYTES,
        "public genesis exceeds the deployment profile bound",
    )?;
    let wire = hex::decode(&trust.genesis_signed_wire_hex)
        .wrap_err("decode independently selected genesis wire")?;
    require(
        hex::encode(&wire) == trust.genesis_signed_wire_hex,
        "public genesis wire must use canonical lowercase hexadecimal",
    )?;
    let genesis = iroha_genesis::decode_signed_genesis(&wire)?;
    let instructions = genesis
        .external_transactions()
        .filter_map(|transaction| match transaction.instructions() {
            Executable::Instructions(instructions) => Some(instructions.iter()),
            _ => None,
        });
    let validators = validator_bindings(instructions.flatten(), &trust.peers)?;
    let descriptor = NativeLaneManifestV1 {
        lane: Some(alias.to_owned()),
        version: Some(NativeLaneManifestV1::VERSION),
        validators: Some(validators),
        quorum: Some(QUORUM),
        ..NativeLaneManifestV1::default()
    };
    let raw = json::to_json_bounded(&descriptor, MAX_NEXUS_RUNTIME_MANIFEST_BYTES)?;
    // Typed writers emit declaration order; the ledger Json owner requires the
    // canonical lexical order supplied by Norito's semantic Value writer. Keep
    // both encodings under the native source bound before strict Json admission.
    let value = json::parse_value(&raw)?;
    let canonical = json::to_json_bounded(&value, MAX_NEXUS_RUNTIME_MANIFEST_BYTES)?;
    let manifest = Json::from_raw_json(canonical)?;
    // The lane ID is immaterial to this structural check; the caller binds the
    // resulting source to its real lane/dataspace and the Core semantic validator.
    RuntimeLaneManifestV1 {
        lane_id: LaneId::SINGLE,
        manifest: manifest.clone(),
    }
    .validate_structure()?;
    Ok(manifest)
}

fn validator_bindings<'a, I>(
    instructions: I,
    selected: &[finality::PeerV1],
) -> Result<Vec<NativeLaneValidatorBindingV1>>
where
    I: IntoIterator<Item = &'a InstructionBox>,
{
    let selected_by_peer = selected
        .iter()
        .map(|peer| (&peer.peer_id, &peer.torii_origin))
        .collect::<BTreeMap<_, _>>();
    require(
        selected.len() == VALIDATORS && selected_by_peer.len() == VALIDATORS,
        "native Taira manifest requires exactly four distinct trusted peers",
    )?;
    let mut registrations = BTreeMap::<AccountId, PeerId>::new();
    let mut registered_peers = BTreeSet::new();
    let mut active = BTreeSet::new();
    for instruction in instructions {
        let any = instruction.as_any();
        if let Some(register) = any.downcast_ref::<RegisterPublicLaneValidator>() {
            if register.lane_id != LaneId::SINGLE {
                continue;
            }
            require(
                selected_by_peer.contains_key(&register.peer_id),
                "signed genesis lane registration names a peer outside the trusted roster",
            )?;
            require(
                registrations.len() < VALIDATORS
                    && !registrations.contains_key(&register.validator)
                    && registered_peers.insert(register.peer_id.clone()),
                "signed genesis lane registrations must bind four unique accounts and peers",
            )?;
            registrations.insert(register.validator.clone(), register.peer_id.clone());
        } else if let Some(activate) = any.downcast_ref::<ActivatePublicLaneValidator>() {
            if activate.lane_id != LaneId::SINGLE {
                continue;
            }
            require(
                registrations.contains_key(&activate.validator)
                    && active.insert(activate.validator.clone()),
                "signed genesis must activate each registered validator exactly once after registration",
            )?;
        } else {
            // This producer deliberately requires explicit initial active bindings;
            // it must not silently use a registration superseded by another action.
            let changed = any
                .downcast_ref::<RebindPublicLaneValidatorPeer>()
                .is_some_and(|change| change.lane_id == LaneId::SINGLE)
                || any
                    .downcast_ref::<ExitPublicLaneValidator>()
                    .is_some_and(|change| change.lane_id == LaneId::SINGLE)
                || any
                    .downcast_ref::<SlashPublicLaneValidator>()
                    .is_some_and(|change| change.lane_id == LaneId::SINGLE);
            require(
                !changed,
                "signed genesis changes an initial lane binding; explicit unchanged active bindings are required",
            )?;
        }
    }
    require(
        registrations.len() == VALIDATORS
            && active.len() == VALIDATORS
            && registrations.keys().all(|account| active.contains(account)),
        "signed genesis must explicitly register and activate all four trusted lane-zero validators",
    )?;
    Ok(registrations
        .into_iter()
        .map(|(account, peer)| NativeLaneValidatorBindingV1 {
            validator: Some(account.to_string()),
            peer_id: Some(peer.to_string()),
            // Presence follows from admission above; no peer-key-derived account
            // or endpoint fallback is permitted.
            torii_url: Some(selected_by_peer[&peer].as_str().to_owned()),
        })
        .collect())
}

#[cfg(test)]
pub(super) fn test_trust() -> finality::TrustV1 {
    use iroha_crypto::Hash;
    use norito::codec::Encode as _;

    let (block, key) = crate::taira_public_reset::deployment_lane_genesis_fixture();
    let validators = iroha_genesis::signed_genesis_validator_pops(&block).unwrap();
    finality::TrustV1 {
        genesis_public_key: key.public_key().clone(),
        genesis_signed_wire_hex: hex::encode(block.encode_wire().unwrap()),
        peers: validators
            .into_keys()
            .enumerate()
            .map(|(index, key)| {
                let peer_id = PeerId::new(key);
                finality::PeerV1 {
                    torii_origin: format!("http://127.0.0.1:{}/", 8080 + index),
                    node_fingerprint: Hash::new(peer_id.encode()),
                    peer_id,
                    build_fingerprint: Hash::new([1]),
                    config_fingerprint: Hash::new([2]),
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{NetworkId, account::address::ChainDiscriminantGuard};
    use iroha_model_base::metadata::Metadata;
    use iroha_primitives::numeric::Quantity;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    }

    fn instructions(peers: &[finality::PeerV1]) -> Vec<InstructionBox> {
        peers
            .iter()
            .zip(130..134)
            .flat_map(|(peer, seed)| {
                let account = account(seed);
                [
                    RegisterPublicLaneValidator::new(
                        LaneId::SINGLE,
                        account.clone(),
                        peer.peer_id.clone(),
                        account.clone(),
                        Quantity::from(1_u64),
                        Metadata::default(),
                    )
                    .into(),
                    ActivatePublicLaneValidator::new(LaneId::SINGLE, account).into(),
                ]
            })
            .collect()
    }

    #[test]
    fn generates_typed_manifest_from_executed_signed_genesis() {
        let _profile = ChainDiscriminantGuard::enter(369);
        let trust = test_trust();
        let genesis = iroha_genesis::decode_signed_genesis(
            &hex::decode(&trust.genesis_signed_wire_hex).unwrap(),
        )
        .unwrap();
        trust
            .validate(NetworkId::from_genesis_hash(genesis.hash()))
            .unwrap();
        let encoded = generate("dpn", &trust).unwrap();
        let manifest: NativeLaneManifestV1 = json::from_str(encoded.get()).unwrap();
        assert!(encoded.get().len() <= MAX_NEXUS_RUNTIME_MANIFEST_BYTES);
        assert_eq!(encoded.get().parse::<Json>().unwrap(), encoded);
        assert_eq!(Json::try_new(&manifest).unwrap(), encoded);
        assert_eq!(manifest.lane.as_deref(), Some("dpn"));
        assert_eq!(manifest.version, Some(1));
        assert_eq!(manifest.quorum, Some(3));
        assert!(manifest.governance.is_none());
        assert!(manifest.protected_namespaces.is_none());
        assert!(manifest.hooks.is_none());
        assert!(manifest.privacy_commitments.is_none());
        let bindings = manifest.validators.unwrap();
        assert_eq!(bindings.len(), 4);
        let expected = genesis
            .external_transactions()
            .filter_map(|transaction| match transaction.instructions() {
                Executable::Instructions(instructions) => Some(instructions.iter()),
                _ => None,
            })
            .flatten()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterPublicLaneValidator>()
            })
            .map(|registration| (registration.validator.clone(), registration.peer_id.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(expected.len(), 4);
        for (binding, (account, peer)) in bindings.iter().zip(expected) {
            assert_eq!(
                binding.validator.as_deref(),
                Some(account.to_string().as_str())
            );
            assert_eq!(binding.peer_id.as_deref(), Some(peer.to_string().as_str()));
            assert_ne!(account, AccountId::new(peer.public_key().clone()));
            let selected = trust
                .peers
                .iter()
                .find(|entry| entry.peer_id == peer)
                .unwrap();
            assert_eq!(binding.torii_url.as_ref(), Some(&selected.torii_origin));
        }
        let mut reordered = trust.clone();
        reordered.peers.reverse();
        assert_eq!(generate("dpn", &reordered).unwrap(), encoded);
        assert_ne!(generate("different", &trust).unwrap(), encoded);
        for alias in ["", "DPN", "dpn.invalid", " dpn"] {
            assert!(generate(alias, &trust).is_err(), "{alias:?}");
        }
    }

    #[test]
    fn rejects_genesis_without_explicit_bindings_or_with_malformed_wire() {
        let _profile = ChainDiscriminantGuard::enter(369);
        let trust = finality::test_trust();
        trust.validate(finality::test_network_id()).unwrap();
        assert!(generate("dpn", &trust).is_err());
        for wire in [
            String::new(),
            "zz".into(),
            format!("{}00", trust.genesis_signed_wire_hex),
        ] {
            let mut changed = trust.clone();
            changed.genesis_signed_wire_hex = wire;
            assert!(generate("dpn", &changed).is_err());
        }
    }

    #[test]
    fn requires_unique_complete_activated_genesis_bindings() {
        let _profile = ChainDiscriminantGuard::enter(369);
        let trust = finality::test_trust();
        let valid = instructions(&trust.peers);
        assert_eq!(validator_bindings(&valid, &trust.peers).unwrap().len(), 4);
        for missing in 0..valid.len() {
            let mut changed = valid.clone();
            changed.remove(missing);
            assert!(
                validator_bindings(&changed, &trust.peers).is_err(),
                "missing {missing}"
            );
        }
        for duplicate in 0..valid.len() {
            let mut changed = valid.clone();
            changed.insert(duplicate, valid[duplicate].clone());
            assert!(
                validator_bindings(&changed, &trust.peers).is_err(),
                "duplicate {duplicate}"
            );
        }
        let mut changed = valid.clone();
        changed.swap(0, 1);
        assert!(validator_bindings(&changed, &trust.peers).is_err());
        let mut changed = valid.clone();
        let register = valid[2]
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
            .unwrap();
        let mut duplicate = register.clone();
        duplicate.peer_id = trust.peers[0].peer_id.clone();
        changed[2] = duplicate.into();
        assert!(validator_bindings(&changed, &trust.peers).is_err());
        let mut changed = valid.clone();
        let mut foreign = register.clone();
        foreign.peer_id = PeerId::new(
            KeyPair::try_from_seed(vec![99; 32], Algorithm::BlsNormal)
                .unwrap()
                .public_key()
                .clone(),
        );
        changed[2] = foreign.into();
        assert!(validator_bindings(&changed, &trust.peers).is_err());
        let mut changed = valid.clone();
        let mut wrong_lane = register.clone();
        wrong_lane.lane_id = LaneId::new(9);
        changed[2] = wrong_lane.into();
        assert!(validator_bindings(&changed, &trust.peers).is_err());
    }

    #[test]
    fn rejects_changed_binding_and_non_four_trusted_roster() {
        let _profile = ChainDiscriminantGuard::enter(369);
        let trust = finality::test_trust();
        let valid = instructions(&trust.peers);
        for peers in [
            vec![],
            trust.peers[..3].to_vec(),
            vec![trust.peers[0].clone(); 4],
        ] {
            assert!(validator_bindings(&valid, &peers).is_err());
        }
        let register = valid[0]
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
            .unwrap();
        let changes: Vec<InstructionBox> = vec![
            RebindPublicLaneValidatorPeer::new(
                LaneId::SINGLE,
                register.validator.clone(),
                trust.peers[1].peer_id.clone(),
            )
            .into(),
            ExitPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: register.validator.clone(),
                release_at_ms: 0,
            }
            .into(),
            SlashPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: register.validator.clone(),
                offence_height: 1,
                slash_id: iroha_crypto::Hash::new(b"slash"),
                amount: Quantity::from(1_u64),
                reason_code: "double_sign".into(),
                metadata: Metadata::default(),
            }
            .into(),
        ];
        for change in changes {
            let mut changed = valid.clone();
            changed.push(change);
            assert!(validator_bindings(&changed, &trust.peers).is_err());
        }
    }
}
