//! Genuine exported suffix stages through the production verifier and WSV payout owner.
//! The local fixture creates an activated wallet row; it does not qualify enrollment,
//! public testnet finality, SDK custody or physical-device performance.
use super::*;
use iroha_core_zk::kagemusha_wallet_artifacts_v1::{VERIFIER_PACK_MAX_BYTES_V1, VerifierPackV1};
use iroha_data_model::isi::kagemusha_wallet::{
    KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1,
};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1, KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1, KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1,
    KagemushaWalletSuffixAnchorV1 as Anchor, KagemushaWalletSuffixHeaderV1 as Header,
    kagemusha_wallet_suffix_authority_digest_v1,
};

fn read(directory: &std::path::Path, name: &str, maximum: usize) -> Vec<u8> {
    let path = directory.join(name);
    assert!(
        std::fs::metadata(&path).unwrap().len() <= maximum as u64,
        "{name} bound"
    );
    std::fs::read(path).unwrap()
}

#[test]
#[ignore = "requires genuine canonical stages exported by the suffix continuation gate"]
fn genuine_suffix_stages_verify_pay_once_and_reject_mutated_originals() {
    let directory = std::path::PathBuf::from(
        std::env::var("KAGEMUSHA_GENUINE_SUFFIX_STAGES").expect("KAGEMUSHA_GENUINE_SUFFIX_STAGES"),
    );
    let pack_bytes = read(
        &directory,
        "verifier-pack.norito",
        VERIFIER_PACK_MAX_BYTES_V1,
    );
    let pack = VerifierPackV1::decode_canonical(&pack_bytes).unwrap();
    let header_bytes = read(
        &directory,
        "header.norito",
        KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1,
    );
    let header: Header =
        storage::decode(&header_bytes, KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1).unwrap();
    header.validate().unwrap();
    let authority = kagemusha_wallet_suffix_authority_digest_v1(&header_bytes).unwrap();
    let anchor_bytes = read(
        &directory,
        "anchor.norito",
        KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1,
    );
    let anchor = Anchor::decode_canonical(&anchor_bytes, &header.scheme_id).unwrap();
    let unload_bytes = read(
        &directory,
        "unload.norito",
        KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1,
    );
    let terms = header.payout_terms().unwrap();
    let mut memory = Memory::new();
    memory.registration.scheme =
        KagemushaWalletSchemeV1::decode_canonical(&pack.scheme, &header.scheme_id).unwrap();
    memory.registration.asset =
        storage::decode(&read(&directory, "asset.norito", 4_096), 4_096).unwrap();
    assert_eq!(
        memory.registration.asset.asset_digest(),
        anchor.credential.body.asset_digest
    );
    memory.authority = header.account.clone();
    memory.balances = BTreeMap::from([
        (memory.registration.reserve.clone(), terms.amount + 1_000),
        (header.account.clone(), 0),
    ]);
    if let Some(beneficiary) = header.charge.beneficiary() {
        memory.balances.entry(beneficiary.clone()).or_default();
    }
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    transact(&mut state, 2, |tx| {
        wsv::WsvLedger::new(tx, &memory.registration.reserve)?.install_verifier_pack(
            header.scheme_id,
            memory.registration.asset.asset_digest(),
            header.artifact_manifest,
            pack_bytes.clone(),
        )?;
        tx.world.kagemusha_wallet_ledger.insert(
            storage::key(storage::WALLET, header.scheme_id, header.wallet_id),
            storage::encode(&WalletRecord {
                asset: memory.registration.asset.asset_digest(),
                phase: Phase::Active,
                activation: [1; 32],
                next_load: 0,
            })?,
        );
        Ok(())
    })
    .unwrap();
    let begin = Action::SuffixUnloadBegin {
        wallet: header.wallet_id,
        authority,
        account: header.account.clone(),
        ordinal: 0,
        header: header_bytes,
        anchor: anchor_bytes,
    };
    let finalize = Action::SuffixUnloadFinalize {
        wallet: header.wallet_id,
        authority,
        account: header.account.clone(),
        ordinal: header.chunk_count + 1,
        unload: unload_bytes.clone(),
    };
    let dispatch = |state: &mut State, height, action| {
        transact(state, height, |tx| {
            KagemushaWalletLedgerV1::new(header.scheme_id, action)
                .execute(&header.account, tx)
                .map_err(Error::from)
        })
    };
    dispatch(&mut state, 3, begin.clone()).unwrap();
    assert!(dispatch(&mut state, 4, finalize.clone()).is_err());
    let mut height = 4;
    // Reverse order proves Appends need only their Begin, never a prior Append.
    for index in (0..header.chunk_count).rev() {
        let chunk = read(
            &directory,
            &format!("chunk-{index:03}.norito"),
            KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1,
        );
        let make_append = |chunk| Action::SuffixUnloadAppend {
            wallet: header.wallet_id,
            authority,
            account: header.account.clone(),
            ordinal: index + 1,
            chunk,
        };
        let mut changed = chunk.clone();
        let last = changed.last_mut().unwrap();
        *last ^= 1;
        assert!(dispatch(&mut state, height, make_append(changed)).is_err());
        dispatch(&mut state, height, make_append(chunk.clone())).unwrap();
        height += 1;
        dispatch(&mut state, height, make_append(chunk)).unwrap();
        height += 1;
    }
    let mut changed_unload = unload_bytes;
    *changed_unload.last_mut().unwrap() ^= 1;
    assert!(
        dispatch(
            &mut state,
            height,
            Action::SuffixUnloadFinalize {
                wallet: header.wallet_id,
                authority,
                account: header.account.clone(),
                ordinal: header.chunk_count + 1,
                unload: changed_unload,
            }
        )
        .is_err()
    );
    let reserve_before = balance(&state, &memory, &memory.registration.reserve);
    dispatch(&mut state, height, finalize.clone()).unwrap();
    let quantity = |value| {
        Quantity::from_canonical_numeric(Numeric::new(value, memory.registration.asset.scale))
            .unwrap()
    };
    assert_eq!(
        balance(&state, &memory, &header.account),
        quantity(terms.account_payout)
    );
    assert_eq!(
        balance(&state, &memory, &memory.registration.reserve),
        reserve_before.checked_sub(&quantity(terms.amount)).unwrap()
    );
    let first_payout = state
        .view()
        .world
        .kagemusha_wallet_ledger()
        .get(&KagemushaWalletPayoutKeyV1::Unload(terms.nullifier).ledger_key(header.scheme_id))
        .unwrap()
        .clone();
    dispatch(&mut state, height + 1, finalize).unwrap();
    dispatch(&mut state, height + 2, begin).unwrap();
    assert_eq!(
        balance(&state, &memory, &header.account),
        quantity(terms.account_payout)
    );
    assert_eq!(
        state
            .view()
            .world
            .kagemusha_wallet_ledger()
            .get(&KagemushaWalletPayoutKeyV1::Unload(terms.nullifier).ledger_key(header.scheme_id)),
        Some(&first_payout)
    );
    let rows: BTreeMap<_, _> = state
        .view()
        .world
        .kagemusha_wallet_ledger()
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    storage::validate_snapshot(rows.iter(), |key| rows.get(key).map(Vec::as_slice)).unwrap();
}
