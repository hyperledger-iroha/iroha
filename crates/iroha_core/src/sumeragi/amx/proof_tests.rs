//! Persisted AMX proofs from actual native execution, finality, pruning and certified replay.

use std::{fs, path::PathBuf};

use iroha_data_model::{
    isi::sumeragi_amx::RegisterAmxDataspaceV1,
    sumeragi_amx::{AmxForeignInstanceV1, AmxLegV1, AmxRecordKind, AmxRecordV1, AmxTransactionV1},
    sumeragi_finality::{genesis_epoch, test_fixtures::NativeFinalityFixture},
};
use iroha_model_base::topology::DataSpaceId;

use super::{BeginAmxV1, amx_record_proof};
use crate::{
    state::{NativeExecutionProjectionV1, World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};

fn config() -> TestChainConfig {
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_instructions = [21, 22]
        .into_iter()
        .map(|id| {
            let chain = NativeFinalityFixture::start(&format!("archived-amx-participant-{id}"));
            RegisterAmxDataspaceV1 {
                dataspace: DataSpaceId::new(id),
                instance: chain.verifier().instance().0,
                anchor: norito::encode_canonical(&genesis_epoch(chain.genesis()).unwrap()).unwrap(),
            }
            .into()
        })
        .collect();
    config
}

fn chain() -> (CertifiedTestChain, [u8; 32]) {
    let config = config();
    let authority = config.genesis_key.clone();
    let mut chain = CertifiedTestChain::start(config).unwrap();
    let transaction = AmxTransactionV1 {
        legs: [21, 22]
            .into_iter()
            .map(|id| AmxLegV1 {
                dataspace: DataSpaceId::new(id),
                payload: vec![7],
            })
            .collect(),
        deadline: 3,
        nonce: [7; 32],
    };
    let tx = transaction.id().unwrap();
    let signed = chain.sign(&authority, [BeginAmxV1 { transaction }.into()], 1_999);
    assert_eq!(chain.commit_at(2_000, vec![signed]), vec![true]);
    (chain, tx)
}

fn path(chain: &CertifiedTestChain) -> PathBuf {
    let paths = fs::read_dir(chain.kura().store_root().join("native-contexts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("00000000000000000002-")
        })
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    paths.into_iter().next().unwrap()
}

#[test]
fn persisted_amx_records_survive_deadline_pruning_and_certified_replay() {
    let (mut chain, tx) = chain();
    let tracker =
        AmxForeignInstanceV1::new(chain.instance().0, genesis_epoch(chain.genesis()).unwrap())
            .unwrap();
    let begin = amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
        .unwrap()
        .unwrap();
    assert!(matches!(begin.record, AmxRecordV1::Begin(_)));
    assert_eq!(tracker.verify_record(&begin).unwrap().height, 2);
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Decision, tx)
            .unwrap()
            .is_none()
    );
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, [0; 32])
            .unwrap()
            .is_none()
    );
    assert!(amx_record_proof(&chain.state().view(), 1, AmxRecordKind::Begin, tx).is_err());
    assert!(amx_record_proof(&chain.state().view(), 3, AmxRecordKind::Begin, tx).is_err());
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    assert!(
        chain
            .state()
            .view()
            .world()
            .sumeragi_amx()
            .transaction(&tx)
            .is_none()
    );
    let decision = amx_record_proof(&chain.state().view(), 4, AmxRecordKind::Decision, tx)
        .unwrap()
        .unwrap();
    assert!(matches!(decision.record, AmxRecordV1::Decision(_)));
    tracker.verify_record(&decision).unwrap();
    assert_eq!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx).unwrap(),
        Some(begin.clone())
    );
    let mut restored = CertifiedTestChain::start(config()).unwrap();
    restored.replay_from(&chain).unwrap();
    for (height, kind, expected) in [
        (2, AmxRecordKind::Begin, begin),
        (4, AmxRecordKind::Decision, decision),
    ] {
        let proof = amx_record_proof(&restored.state().view(), height, kind, tx)
            .unwrap()
            .unwrap();
        assert_eq!(proof, expected);
        tracker.verify_record(&proof).unwrap();
    }
}

#[test]
fn persisted_amx_proof_rejects_missing_corrupt_and_substituted_archives() {
    let (chain, tx) = chain();
    let path = path(&chain);
    let original = fs::read(&path).unwrap();
    let projection: NativeExecutionProjectionV1 = norito::decode_canonical(&original).unwrap();
    for change in 0..6 {
        let mut changed = projection.clone();
        match change {
            0 => changed.carrier_height += 1,
            1 => {
                changed.carrier_hash = iroha_crypto::HashOf::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"foreign AMX carrier"),
                )
            }
            2 => changed.lanes.incarnations += 1,
            3 => changed
                .ordinary_writes
                .retain(|write| write.key.first() != Some(&0xD9)),
            4 => changed
                .ordinary_writes
                .retain(|write| write.key.first() == Some(&0xD9)),
            _ => changed
                .ordinary_writes
                .push(iroha_data_model::block::consensus::ExecKv {
                    key: b"unexecuted".to_vec(),
                    value: vec![1],
                }),
        }
        fs::write(&path, norito::encode_canonical(&changed).unwrap()).unwrap();
        assert!(
            amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx).is_err(),
            "canonical archive mutation {change} must fail authentication"
        );
    }
    fs::write(&path, b"corrupt original archive").unwrap();
    assert!(amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx).is_err());
    fs::remove_file(&path).unwrap();
    assert!(amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx).is_err());
    fs::write(&path, original).unwrap();
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .unwrap()
            .is_some()
    );
}

#[test]
fn persisted_amx_proof_refuses_original_pool_exhaustion_and_unverified_certificates() {
    let (chain, tx) = chain();
    let budget = chain.state().ivm_execution_budget();
    let limit = budget.limit_bytes();
    let reserved = budget.reserved_bytes();
    budget.set_limit_bytes(reserved);
    assert!(amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx).is_err());
    assert_eq!(budget.reserved_bytes(), reserved);
    budget.set_limit_bytes(limit);
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .unwrap()
            .is_some()
    );
    assert_eq!(budget.reserved_bytes(), reserved);
    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    assert!(amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx).is_err());
}
