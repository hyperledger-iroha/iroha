//! Exact historical selection, bounded canonical decoding and no-write read regressions.
use super::*;
#[test]
fn reader_rejects_wrong_scope_missing_height_and_does_not_mutate_on_repeated_admission() {
    let mut f = fixture();
    configure(&mut f);
    let before = f
        .state
        .world_view()
        .smart_contract_state()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    for _ in 0..1_000 {
        let result = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 1)
            .expect("indexed admission")
            .expect("configured");
        assert!(result.state.active_head.is_none());
        assert_eq!(result.anchor.height, 1);
    }
    assert_eq!(
        f.state
            .world_view()
            .smart_contract_state()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(
        read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 0),
        Err(Error::HeightUnavailable)
    );
    assert_eq!(
        read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 2),
        Err(Error::HeightUnavailable)
    );
    let mut wrong = f.policy.binding.clone();
    wrong.network_id = [88; 32];
    assert_eq!(
        read_stream_token_custody_control_at_v1(&f.state.view(), &wrong, 1),
        Err(Error::BindingMismatch)
    );
    let mut wrong = f.policy.binding.clone();
    wrong.chain_id = "wrong-chain".into();
    assert_eq!(
        read_stream_token_custody_control_at_v1(&f.state.view(), &wrong, 1),
        Err(Error::BindingMismatch)
    );
    let mut wrong = f.policy.binding.clone();
    wrong.policy_digest = [88; 32];
    assert_eq!(
        read_stream_token_custody_control_at_v1(&f.state.view(), &wrong, 1),
        Err(Error::BindingMismatch)
    );
}
#[test]
fn reader_rejects_missing_height_key_replaced_active_pointer_and_missing_key_tombstone() {
    let mut f = fixture();
    configure(&mut f);
    transact(&mut f.state, 1_500, |tx| {
        let original = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head")
            .index;
        instruction(
            tx,
            f.provider,
            Action::Revoke {
                signer: true,
                attester: false,
            },
        )
        .execute(&f.authority, tx)
        .expect("revoke");
        let current = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head")
            .index;
        tx.world.smart_contract_state.insert(
            head_key(f.provider),
            encode(&original).expect("old pointer"),
        );
        assert!(read_active(tx.world(), f.provider).is_err());
        tx.world.smart_contract_state.insert(
            head_key(f.provider),
            encode(&current).expect("current pointer"),
        );
        let index_path = height_key(f.provider, current.height, current.ordinal);
        let bytes = tx
            .world
            .smart_contract_state
            .get(&index_path)
            .expect("height index")
            .clone();
        tx.world.smart_contract_state.remove(index_path.clone());
        assert!(read_active(tx.world(), f.provider).is_err());
        tx.world.smart_contract_state.insert(index_path, bytes);
        let key =
            key_path(f.provider, true, &f.policy.binding.public_key).expect("key tombstone path");
        let bytes = tx
            .world
            .smart_contract_state
            .get(&key)
            .expect("first-use index")
            .clone();
        tx.world.smart_contract_state.remove(key.clone());
        assert!(read_active(tx.world(), f.provider).is_err());
        tx.world.smart_contract_state.insert(key, bytes);
        assert_eq!(
            read_active(tx.world(), f.provider)
                .expect("restored")
                .expect("head")
                .index,
            current
        );
    });
}
#[test]
fn native_record_digest_binds_authority_request_and_execution_but_not_later_blocks() {
    let mut f = fixture();
    configure(&mut f);
    let first = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 1)
        .expect("snapshot")
        .expect("head");
    let record = read_active(&f.state.world_view(), f.provider)
        .expect("record")
        .expect("head")
        .record;
    let frame = encode(&record).expect("native canonical frame");
    assert_eq!(
        decode::<StreamTokenCustodyControlRecordV1>(&frame).expect("bounded native replay"),
        record
    );
    let digest = record_digest(&record).expect("digest");
    for changed in [
        StreamTokenCustodyControlRecordV1 {
            authority: f.other.clone(),
            ..record.clone()
        },
        StreamTokenCustodyControlRecordV1 {
            request_digest: [33; 32],
            ..record.clone()
        },
        StreamTokenCustodyControlRecordV1 {
            execution_height: 2,
            ..record.clone()
        },
        StreamTokenCustodyControlRecordV1 {
            ordinal: 1,
            ..record.clone()
        },
    ] {
        assert_ne!(record_digest(&changed).expect("mutated digest"), digest);
    }
    let mut suffix = frame.clone();
    suffix.push(0);
    assert!(decode::<StreamTokenCustodyControlRecordV1>(&suffix).is_err());
    assert!(decode::<StreamTokenCustodyControlRecordV1>(&frame[..frame.len() - 1]).is_err());
    assert!(decode::<StreamTokenCustodyControlRecordV1>(&vec![0; 16 * 1024 + 1]).is_err());
    transact(&mut f.state, 1_500, |_| {});
    let later = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 2)
        .expect("later state")
        .expect("head");
    assert_eq!(later.anchor.state_digest, first.anchor.state_digest);
    assert_ne!(later.anchor.block_hash, first.anchor.block_hash);
}
#[test]
fn unknown_provider_and_bad_time_or_oversized_action_cannot_create_state() {
    let mut f = fixture();
    transact(&mut f.state, 0, |tx| {
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&f.policy).expect("policy"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert!(
            read_active(tx.world(), f.provider)
                .expect("absent")
                .is_none()
        );
    });
    transact(&mut f.state, 1_000, |tx| {
        assert!(
            instruction(tx, f.provider, Action::Configure(vec![0; 32 * 1024 + 1]))
                .execute(&f.authority, tx)
                .is_err()
        );
        tx.world.provider_owners.remove(f.provider);
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&f.policy).expect("policy"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert!(
            read_active(tx.world(), f.provider)
                .expect("absent")
                .is_none()
        );
    });
}

#[test]
fn canonical_control_frames_and_native_digest_ignore_all_ten_ambient_layouts() {
    use norito::core::header_flags::{
        COMPACT_LEN as C, FIELD_BITSET as F, PACKED_SEQ as Q, PACKED_STRUCT as S,
    };
    let mut f = fixture();
    configure(&mut f);
    let record = read_active(&f.state.world_view(), f.provider)
        .expect("native control")
        .expect("configured")
        .record;
    let frame = encode(&record).expect("canonical native record");
    let digest = record_digest(&record).expect("native digest");
    let mut preimage =
        iroha_data_model::sorafs::stream_token_custody::STREAM_TOKEN_CUSTODY_RECORD_DOMAIN_V1
            .to_vec();
    preimage.extend_from_slice(&frame);
    assert_eq!(digest, *Hash::new(preimage).as_ref());
    for flags in [
        0,
        C,
        Q,
        Q | C,
        S,
        S | C,
        Q | S,
        Q | S | C,
        S | C | F,
        Q | S | C | F,
    ] {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(encode(&record).expect("ambient canonical encoding"), frame);
        assert_eq!(
            decode::<StreamTokenCustodyControlRecordV1>(&frame)
                .expect("actual bounded reader under ambient flags"),
            record
        );
        assert_eq!(record_digest(&record).expect("ambient digest"), digest);
    }
}

#[test]
fn provider_removal_preserves_historical_reads_but_rejects_mutation_and_exact_retry() {
    let mut f = fixture();
    configure(&mut f);
    let original = MutateSorafsStreamTokenCustody {
        provider_id: f.provider,
        expected_revision: 0,
        expected_digest: [0; 32],
        action: Action::Configure(encode(&f.policy).expect("original policy")),
    };
    let approval = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 1)
        .expect("historical read")
        .expect("configured");
    transact(&mut f.state, 1_500, |tx| {
        tx.world.provider_owners.remove(f.provider);
        let before = tx
            .world
            .smart_contract_state
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        assert!(
            original.execute(&f.authority, tx).is_err(),
            "removed provider cannot acknowledge retries"
        );
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Revoke {
                    signer: true,
                    attester: true
                }
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert_eq!(
            tx.world
                .smart_contract_state
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
            before
        );
    });
    assert_eq!(
        read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 1)
            .expect("retained historical record"),
        Some(approval.clone())
    );
    let later = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 2)
        .expect("retained control at later height")
        .expect("retained head");
    assert_eq!(later.state, approval.state);
    assert_eq!(later.anchor.state_digest, approval.anchor.state_digest);
}
