//! The predecessor-recorded authorization of a governed adoption.

use iroha_crypto::Hash;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

use super::*;

const START: u64 = 100;
const END: u64 = 200;

fn selection() -> SelectionId {
    crate::runtime::release_catalog()
        .expect("release catalog")
        .baseline_id
}

fn manifest_naming(selection: SelectionId) -> PredecessorUpgradeManifest {
    PredecessorUpgradeManifest {
        name: ADOPTION_MANIFEST_NAME.into(),
        description: "adopt the live-upgrade foundation".into(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: Vec::new(),
        added_pointer_types: Vec::new(),
        start_height: START,
        end_height: END,
        sbom_digests: vec![PredecessorSbomDigest {
            algorithm: ADOPTION_SELECTION_DIGEST.into(),
            digest: selection.0.to_vec(),
        }],
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    }
}

fn entry(
    seed: u8,
    manifest: PredecessorUpgradeManifest,
    status: PredecessorUpgradeStatus,
) -> PredecessorRecord {
    PredecessorRecord {
        id: PredecessorUpgradeId([seed; 32]),
        record: PredecessorUpgradeRecord {
            manifest,
            status,
            proposer: ALICE_ID.clone(),
            created_height: START - 10,
        },
    }
}

fn activated(seed: u8) -> PredecessorRecord {
    entry(
        seed,
        manifest_naming(selection()),
        PredecessorUpgradeStatus::ActivatedAt(START),
    )
}

#[test]
fn an_activated_authorization_admits_a_checkpoint_inside_its_window() {
    let records = [activated(1)];
    for height in [START + 1, END - 1] {
        assert_eq!(
            authorization(&records, height, selection()).map(|found| found.id),
            Ok(PredecessorUpgradeId([1; 32])),
            "checkpoint {height}"
        );
    }
    for height in [START - 1, START, END, END + 1] {
        assert_eq!(
            authorization(&records, height, selection()),
            Err(AdoptionError::Unauthorized {
                selection: selection(),
                height
            }),
            "checkpoint {height} lies outside the window after activation"
        );
    }
}

#[test]
fn only_the_named_selection_of_an_activated_adoption_manifest_authorizes() {
    let height = START + 5;
    let proposed = entry(
        1,
        manifest_naming(selection()),
        PredecessorUpgradeStatus::Proposed,
    );
    let late = entry(
        1,
        manifest_naming(selection()),
        PredecessorUpgradeStatus::ActivatedAt(START + 1),
    );
    let canceled = entry(
        1,
        manifest_naming(selection()),
        PredecessorUpgradeStatus::Canceled,
    );
    let foreign = entry(
        1,
        manifest_naming(SelectionId([0xAA; 32])),
        PredecessorUpgradeStatus::ActivatedAt(START),
    );
    let mut twice_named = activated(1);
    twice_named
        .record
        .manifest
        .sbom_digests
        .push(twice_named.record.manifest.sbom_digests[0].clone());
    for record in [proposed, late, canceled, foreign, twice_named] {
        assert_eq!(
            authorization(std::slice::from_ref(&record), height, selection()).err(),
            Some(AdoptionError::Unauthorized {
                selection: selection(),
                height,
            }),
            "{record:?}"
        );
    }
}

#[test]
fn every_record_of_the_retired_table_is_an_adoption_manifest_and_one_authorizes() {
    let height = START + 5;
    let mut other = activated(2);
    other.record.manifest.name = "abi-v1 refresh".into();
    assert_eq!(
        authorization(&[activated(1), other], height, selection()),
        Err(AdoptionError::NotAnAuthorization(hex::encode([2; 32])))
    );
    assert_eq!(
        authorization(&[activated(1), activated(2)], height, selection()),
        Err(AdoptionError::Ambiguous(2))
    );
    let stale = entry(
        3,
        manifest_naming(selection()),
        PredecessorUpgradeStatus::Canceled,
    );
    assert_eq!(
        authorization(&[stale, activated(1)], height, selection()).map(|found| found.id),
        Ok(PredecessorUpgradeId([1; 32])),
        "a cancelled earlier authorization leaves with its table"
    );
}

/// The predecessor's snapshot JSON of `record` under `id`.
fn record_json(id: PredecessorUpgradeId, record: &PredecessorUpgradeRecord) -> String {
    use base64::Engine as _;
    let manifest = &record.manifest;
    let digests: Vec<String> = manifest
        .sbom_digests
        .iter()
        .map(|digest| {
            format!(
                r#"{{"algorithm":{},"digest":"{}"}}"#,
                json::to_json(&digest.algorithm).unwrap(),
                base64::engine::general_purpose::STANDARD.encode(&digest.digest)
            )
        })
        .collect();
    let status = match record.status {
        PredecessorUpgradeStatus::Proposed => r#"{"Proposed":null}"#.to_owned(),
        PredecessorUpgradeStatus::ActivatedAt(height) => format!(r#"{{"ActivatedAt":{height}}}"#),
        PredecessorUpgradeStatus::Canceled => r#"{"Canceled":null}"#.to_owned(),
    };
    format!(
        concat!(
            r#"{{"{id}":{{"manifest":{{"name":{name},"description":{description},"abi_version":{abi_version},"#,
            r#""abi_hash":{abi_hash},"added_syscalls":[],"added_pointer_types":[],"start_height":{start},"#,
            r#""end_height":{end},"sbom_digests":[{digests}],"slsa_attestation":"","provenance":[]}},"#,
            r#""status":{status},"proposer":{proposer},"created_height":{created}}}}}"#
        ),
        id = hex::encode_upper(id.0),
        name = json::to_json(&manifest.name).unwrap(),
        description = json::to_json(&manifest.description).unwrap(),
        abi_version = manifest.abi_version,
        abi_hash = json::to_json(&manifest.abi_hash.to_vec()).unwrap(),
        start = manifest.start_height,
        end = manifest.end_height,
        digests = digests.join(","),
        status = status,
        proposer = json::to_json(&record.proposer).unwrap(),
        created = record.created_height,
    )
}

#[test]
fn the_predecessor_table_parses_exactly() {
    let expected = activated(7);
    let blocks = record_json(expected.id, &expected.record);
    let parsed = parse_runtime_upgrades(&format!(r#"{{"revert":{{}},"blocks":{blocks}}}"#))
        .expect("the predecessor's table");
    assert_eq!(parsed, vec![expected.clone()]);
    assert_eq!(
        parse_runtime_upgrades(r#"{"revert":{},"blocks":{}}"#),
        Ok(Vec::new())
    );

    let changed_at_checkpoint = format!(
        r#"{{"revert":{{"{}":{{"value":null}}}},"blocks":{blocks}}}"#,
        hex::encode_upper(expected.id.0)
    );
    assert!(
        parse_runtime_upgrades(&changed_at_checkpoint).is_err(),
        "a table the checkpoint block changed carries an undo log nothing can apply"
    );
    let extra = blocks.replacen(r#""created_height""#, r#""extra":1,"created_height""#, 1);
    assert!(
        parse_runtime_upgrades(&format!(r#"{{"revert":{{}},"blocks":{extra}}}"#)).is_err(),
        "an unknown field is not the predecessor's layout"
    );
}

#[test]
fn the_printed_manifest_is_one_the_predecessor_admits() {
    let printed =
        authorization_manifest_json(selection(), START, END, "adopt").expect("manifest JSON");
    let parsed = manifest(&json::from_str::<Value>(&printed).unwrap()).expect("predecessor layout");
    let mut expected = manifest_naming(selection());
    expected.description = "adopt".into();
    assert_eq!(parsed, expected);
    assert!(authorization_manifest_json(selection(), START, START + 1, "adopt").is_err());
    let activated = PredecessorRecord {
        id: PredecessorUpgradeId([1; 32]),
        record: PredecessorUpgradeRecord {
            manifest: parsed,
            status: PredecessorUpgradeStatus::ActivatedAt(START),
            proposer: ALICE_ID.clone(),
            created_height: 1,
        },
    };
    assert!(authorization(std::slice::from_ref(&activated), START + 1, selection()).is_ok());
}

#[test]
fn records_roundtrip_through_their_predecessor_encoding() {
    let record = activated(5);
    let bytes = norito::encode_canonical(&record.record).expect("encode");
    let decoded: PredecessorUpgradeRecord = norito::decode_canonical(&bytes).expect("decode");
    assert_eq!(decoded, record.record);
    let (key, value) = record.element_hashes().expect("hashes");
    assert_ne!(key, value);
    assert_eq!(
        key,
        crate::state::world_projection::hash_value(&[5_u8; 32]).unwrap(),
        "the identity hashes as the predecessor's 32-byte key"
    );
}

#[test]
fn the_anchor_binds_the_checkpoint_roots_committee_selection_and_authorization() {
    let peers = vec![iroha_model_base::peer::PeerId::new(
        ALICE_KEYPAIR.public_key().clone(),
    )];
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x41; 32])));
    let block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0x42; 32]));
    let inputs = AnchorInputs {
        network,
        height: START + 3,
        block_hash,
        predecessor_world_root: Hash::prehashed([0x43; 32]),
        imported_state_root: Hash::prehashed([0x44; 32]),
        committee: &peers,
    };
    let record = anchor(&[activated(9)], inputs).expect("authorized adoption");
    assert_eq!(record.network, network);
    assert_eq!(record.height, START + 3);
    assert_eq!(record.block_hash, [0x42; 32]);
    assert_eq!(record.predecessor_world_root, [0x43; 32]);
    assert_eq!(record.imported_state_root, [0x44; 32]);
    assert_eq!(record.selection, selection());
    assert_eq!(record.authorization, [9; 32]);
    assert_eq!(
        record.committee,
        committee_digest(&[ALICE_KEYPAIR.public_key().clone()]).unwrap()
    );
    assert!(
        anchor(&[], inputs).is_err(),
        "no adoption without its governed authorization"
    );
}
