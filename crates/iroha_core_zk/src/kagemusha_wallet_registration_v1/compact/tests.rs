//! Terminal DATA joins only: these tests deliberately do not create or fake a history grant.
use super::*;

#[test]
fn canonical_compact_data_roundtrip_and_bounds_are_not_authority() {
    let (original, _, _) = super::super::native_tests::compact_data_fixture();
    let bytes = original.encode_canonical().unwrap();
    let parsed = CompactRegistrationOriginalV1::decode_canonical(&bytes).unwrap();
    assert_eq!(parsed.encode_canonical().unwrap(), bytes);
    assert!(HistoryOriginalV1::decode_canonical(&parsed.history).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(CompactRegistrationOriginalV1::decode_canonical(&trailing).is_err());
    for change in 0..8 {
        let mut changed = original.clone();
        match change {
            0 => changed.version = 2,
            1 => changed.asset_digest = [0; 32],
            2 => changed.history.clear(),
            3 => changed.result.clear(),
            4 => changed.block.clear(),
            5 => changed.committed.clear(),
            6 => changed.history = vec![1; HISTORY_ORIGINAL_MAX_BYTES_V1 + 1],
            _ => changed.result = vec![1; MAX_RESULT_BYTES as usize + 1],
        }
        assert!(changed.encode_canonical().is_err(), "change {change}");
    }
}

#[test]
fn terminal_data_binds_exact_result_tape_height_and_successful_register() {
    use ff::Field;
    let (original, state, scheme) = super::super::native_tests::compact_data_fixture();
    let (data, _, height) = terminal_data(&original, &state, &scheme, None).unwrap();
    assert_eq!(data.asset().asset_digest(), original.asset_digest);
    assert_eq!(height + 1, state.next_height);
    for change in 0..10 {
        let mut changed = original.clone();
        let mut opened = state;
        let mut expected = scheme;
        match change {
            0 => opened.next_height += 1, // A later prefix cannot open this earlier Register.
            1 => opened.result[0] ^= 1,
            2 => opened.tape_root += iroha_pasta::Fp::ONE,
            3 => opened.frame_len += 1,
            4 => changed.result.push(0),
            5 => changed.block.push(0),
            6 => changed.committed.push(0),
            7 => changed.instruction_index = 0,
            8 => changed.asset_digest[0] ^= 1,
            _ => expected.network_id[0] ^= 1,
        }
        assert!(
            terminal_data(&changed, &opened, &expected, None).is_err(),
            "change {change}"
        );
    }
    let mut changed = original.clone();
    let mut committed: CommittedTransaction = decode(&changed.committed).unwrap();
    committed.block_hash =
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"foreign"));
    changed.committed = norito::encode_canonical(&committed).unwrap();
    assert!(terminal_data(&changed, &state, &scheme, None).is_err());
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        terminal_data(&original, &state, &scheme, Some(&token)),
        Err(RegistrationErrorV1::Cancelled)
    ));
}
