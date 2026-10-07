//! Real Unload execution and permanent replay after the exact funded native exchange.
use super::*;

fn ledger_rows(setup: &ExecutedKagemushaSetup) -> Vec<(KagemushaWalletLedgerKeyV1, Vec<u8>)> {
    setup
        .chain
        .state()
        .view()
        .world()
        .kagemusha_wallet_ledger()
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect()
}

fn require_funded_receipt(setup: &ExecutedKagemushaSetup, input: &Inputs) {
    let original = read(
        &selected("KAGEMUSHA_NATIVE_LOAD_RECEIPT"),
        4_096,
        pin("KAGEMUSHA_NATIVE_LOAD_RECEIPT_SHA256"),
    );
    let view = setup.chain.state().view();
    let source =
        CommittedLoadReceipts::new(&view, 2_000_000, norito::canonical_decode_limits(2_000_000))
            .unwrap();
    let Action::IssueLoad {
        wallet, request_id, ..
    } = &input.load.action
    else {
        panic!("selected actual Load")
    };
    let receipt = source
        .receipt_for(
            &setup.account,
            &input.scheme.scheme_id(),
            wallet,
            request_id,
        )
        .unwrap();
    assert_eq!(
        norito::encode_canonical(&receipt).unwrap(),
        original,
        "recreated chain must execute the exact already-proved funded receipt"
    );
}

fn settlement_proofs(setup: &ExecutedKagemushaSetup, output: &Path) {
    assert!(!output.exists());
    let mut directory = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        directory.mode(0o700);
    }
    directory.create(output).unwrap();
    let mut verifier = native(setup);
    let mut records = Vec::new();
    for height in 1..=setup.chain.height() {
        let proof =
            crate::sumeragi::finality::build_proof(&setup.chain.state().view(), height).unwrap();
        let verified = verifier.verify(&proof).unwrap();
        assert_eq!(verified.result(), setup.chain.committed(height).result());
        records.push(publish(
            output,
            &format!("native-proof-{height}.norito"),
            &norito::encode_canonical(&proof).unwrap(),
        ));
    }
    let receipt = norito::json!({
        "schema": "iroha.kagemusha.executed-unload-settlement.v1",
        "network_hex": (hex::encode(setup.chain.network_id().as_bytes())),
        "target_sha256": (hex::encode(pin("KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256"))),
        "receipt_sha256": (hex::encode(pin("KAGEMUSHA_NATIVE_LOAD_RECEIPT_SHA256"))),
        "claim_sha256": (hex::encode(pin("KAGEMUSHA_NATIVE_UNLOAD_CLAIM_SHA256"))),
        "initial_supply_atomic_units": "1000",
        "payer_balance_atomic_units": "900",
        "recipient_b_balance_atomic_units": "0",
        "recipient_c_balance_atomic_units": "100",
        "reserve_balance_atomic_units": "0",
        "exact_once_payout": true,
        "scope": "Actual native proof verification and ledger execution with exact receipt replay; fixture signs exactly three votes of a four-seat committee. Real P2P rounds and physical hardware qualification remain separate.",
        "originals": records,
    });
    publish(
        output,
        "settlement.json",
        norito::json::to_json(&receipt).unwrap().as_bytes(),
    );
    File::open(output).unwrap().sync_all().unwrap();
    File::open(output.parent().unwrap())
        .unwrap()
        .sync_all()
        .unwrap();
}

#[test]
#[ignore = "requires complete installed native A→B→C proof originals and exact funded receipt; executes real ledger Unload and replay"]
fn execute_actual_c_unload_with_conservation_and_permanent_replay() {
    let mut setup = start();
    let input = inputs(&setup);
    fund(&mut setup, &input);
    require_funded_receipt(&setup, &input);
    let original = read(
        &selected("KAGEMUSHA_NATIVE_UNLOAD_CLAIM"),
        KAGEMUSHA_WALLET_UNLOAD_CLAIM_MAX_BYTES_V1,
        pin("KAGEMUSHA_NATIVE_UNLOAD_CLAIM_SHA256"),
    );
    let claim =
        KagemushaWalletUnloadClaimV1::decode_canonical(&original, &input.scheme.scheme_id())
            .unwrap();
    assert_eq!(claim.account, setup.recipients[1]);
    assert_ne!(claim.account, setup.account);
    assert_ne!(claim.account, setup.recipients[0]);
    let paid = claim.verify(&input.scheme).unwrap();
    assert_eq!(
        (paid.amount, paid.account_payout, paid.online_charge),
        (100, 100, 0)
    );

    let instruction =
        KagemushaWalletLedgerV1::new(input.scheme.scheme_id(), Action::Unload(original));
    let transaction = setup
        .chain
        .sign(&key(43), [instruction.clone().into()], 5_999);
    assert_eq!(setup.chain.commit_at(6_000, vec![transaction]), [true]);
    assert_eq!(
        balance(&setup, &setup.account),
        Quantity::from_canonical_numeric(Numeric::new(900, SCALE)).unwrap()
    );
    assert_eq!(balance(&setup, &setup.recipients[0]), Quantity::zero());
    assert_eq!(
        balance(&setup, &setup.recipients[1]),
        Quantity::from_canonical_numeric(Numeric::new(100, SCALE)).unwrap()
    );
    assert_eq!(balance(&setup, &setup.reserve), Quantity::zero());
    let rows = ledger_rows(&setup);

    // An exact claim retry returns the retained outcome and cannot pay a second time.
    let transaction = setup.chain.sign(&key(43), [instruction.into()], 6_999);
    assert_eq!(setup.chain.commit_at(7_000, vec![transaction]), [true]);
    assert_eq!(ledger_rows(&setup), rows);
    assert_eq!(
        balance(&setup, &setup.recipients[1]),
        Quantity::from_canonical_numeric(Numeric::new(100, SCALE)).unwrap()
    );
    assert_eq!(balance(&setup, &setup.reserve), Quantity::zero());

    // Even after payout, a modified claim must pass complete verification before replay lookup.
    for (index, changed) in [
        {
            let mut changed = claim.clone();
            changed.account = setup.account.clone();
            changed
        },
        {
            let mut changed = claim.clone();
            changed.package.step_proof.bytes[0] ^= 1;
            changed
        },
    ]
    .into_iter()
    .enumerate()
    {
        let instruction = KagemushaWalletLedgerV1::new(
            input.scheme.scheme_id(),
            Action::Unload(norito::encode_canonical(&changed).unwrap()),
        );
        let time = (8 + index as u64) * 1_000;
        let transaction = setup.chain.sign(&key(43), [instruction.into()], time - 1);
        assert_eq!(setup.chain.commit_at(time, vec![transaction]), [false]);
        assert_eq!(ledger_rows(&setup), rows);
    }
    assert_eq!(
        balance(&setup, &setup.account),
        Quantity::from_canonical_numeric(Numeric::new(900, SCALE)).unwrap()
    );
    assert_eq!(balance(&setup, &setup.recipients[0]), Quantity::zero());
    assert_eq!(
        balance(&setup, &setup.recipients[1]),
        Quantity::from_canonical_numeric(Numeric::new(100, SCALE)).unwrap()
    );
    assert_eq!(balance(&setup, &setup.reserve), Quantity::zero());
    settlement_proofs(&setup, &selected("KAGEMUSHA_EXECUTED_UNLOAD_OUTPUT"));
}
