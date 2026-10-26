//! One hundred distinct software-fixture owners, executed account registration and real
//! Bootstrap/Activate execution. No World rows, execution results or finality are injected.
//! The platform/issuer fixture is engineering evidence, never production admission.
use super::funding::{canonical, pin, read, selected};
use super::*;
use crate::kagemusha_wallet_v1::{Phase, WalletRecord};
use iroha_core_zk::kagemusha_wallet_artifacts_v1::{
    InstallationV1, InstalledVerifierPackV1, VERIFIER_PACK_MAX_BYTES_V1,
};
use iroha_data_model::transaction::Executable;
use iroha_data_model::{
    asset::AssetBalanceScope,
    isi::kagemusha_wallet::{KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1},
    kagemusha::*,
    sumeragi_finality::{
        CertifiedTransactionInclusionV1, SumeragiCommitCertificateV1, VerifiedSumeragiBlock,
    },
};
use iroha_version::codec::DecodeVersioned as _;
use std::{collections::BTreeSet, path::PathBuf, time::Instant};

const COUNT: usize = 100;
const FIRST_SEED: u8 = 100;
const SETUP: &str = "iroha.kagemusha.enrollment-cohort-setup.v1";
const TARGET: &str = "iroha.kagemusha.enrollment-cohort-target.v1";
const FINALITY: &str = "iroha.kagemusha.enrollment-cohort-finality.v1";
const APP_STAGE_LIMIT: Duration = Duration::from_secs(90);

struct Cohort {
    setup: ExecutedKagemushaSetup,
    accounts: Vec<AccountId>,
    registration_elapsed_ms: u128,
}

fn cohort() -> Cohort {
    let mut setup = start();
    let began = Instant::now();
    let accounts: Vec<_> = (0..COUNT)
        .map(|index| AccountId::new(key(FIRST_SEED + index as u8).public_key().clone()))
        .collect();
    assert_eq!(accounts.iter().collect::<BTreeSet<_>>().len(), COUNT);
    {
        let view = setup.chain.state().view();
        for account in &accounts {
            assert!(view.world().account(account).is_err());
            assert_ne!(account, &setup.reserve);
            assert_ne!(account, &setup.account);
        }
    }
    let instructions: Vec<InstructionBox> = accounts
        .iter()
        .map(|account| Register::account(Account::new(account.clone())).into())
        .collect();
    let transaction = setup.chain.sign(&key(0xCE), instructions, 2_999);
    assert_eq!(setup.chain.commit_at(3_000, vec![transaction]), [true]);
    assert_eq!(setup.chain.height(), 3);
    {
        let view = setup.chain.state().view();
        for account in &accounts {
            assert!(view.world().account(account).is_ok());
            assert!(
                view.world()
                    .assets()
                    .get(&AssetId::of(setup.asset.asset.clone(), account.clone()))
                    .is_none()
            );
        }
    }
    let mut verifier = native(&setup);
    for height in 1..=3 {
        let proof =
            crate::sumeragi::finality::build_proof(&setup.chain.state().view(), height).unwrap();
        assert_eq!(
            verifier.verify(&proof).unwrap().result(),
            setup.chain.committed(height).result()
        );
    }
    assert!(
        began.elapsed() < APP_STAGE_LIMIT,
        "100 account registrations exceeded the iOS stage deadline"
    );
    Cohort {
        setup,
        accounts,
        registration_elapsed_ms: began.elapsed().as_millis(),
    }
}

fn make_output(path: &Path) {
    assert!(
        path.is_absolute() && !path.exists(),
        "fresh absolute output directory required"
    );
    fs::create_dir(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}
fn finish_output(path: &Path) {
    File::open(path).unwrap().sync_all().unwrap();
    File::open(path.parent().unwrap())
        .unwrap()
        .sync_all()
        .unwrap();
}
fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn word(value: &norito::json::Value) -> [u8; 32] {
    hex::decode(value.as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap()
}
fn original(root: &Path, row: &norito::json::Value, expected_name: &str, cap: usize) -> Vec<u8> {
    assert_eq!(row["name"].as_str(), Some(expected_name));
    let bytes = read(&root.join(expected_name), cap, word(&row["sha256"]));
    assert_eq!(row["bytes"].as_u64(), Some(bytes.len() as u64));
    bytes
}
fn owner_original(
    root: &Path,
    owner: &norito::json::Value,
    index: usize,
    name: &str,
    cap: usize,
) -> Vec<u8> {
    let matches: Vec<_> = owner["originals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["name"].as_str() == Some(name))
        .collect();
    assert_eq!(matches.len(), 1, "exactly one selected owner original");
    original(
        &root.join(format!("owner-{index:03}")),
        matches[0],
        name,
        cap,
    )
}
fn verify_base_setup(setup: &ExecutedKagemushaSetup) -> String {
    let path = selected("KAGEMUSHA_NATIVE_LEDGER_SETUP");
    let bytes = read(&path, 64 << 10, pin("KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256"));
    let manifest: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(manifest["schema"].as_str(), Some(SETUP_SCHEMA));
    assert_eq!(
        word(&manifest["network_hex"]),
        *setup.chain.network_id().as_bytes()
    );
    assert_eq!(word(&manifest["instance_hex"]), setup.chain.instance().0);
    let rows = manifest["originals"].as_array().unwrap();
    let genesis = rows
        .iter()
        .find(|row| row["name"].as_str() == Some("signed-genesis.wire"))
        .unwrap();
    assert_eq!(
        original(
            path.parent().unwrap(),
            genesis,
            "signed-genesis.wire",
            16 << 20
        ),
        setup.chain.genesis().encode_wire().unwrap()
    );
    digest(&bytes)
}
fn history(setup: &ExecutedKagemushaSetup, output: &Path) -> Vec<norito::json::Value> {
    let mut verifier = native(setup);
    (1..=setup.chain.height())
        .map(|height| {
            let proof = crate::sumeragi::finality::build_proof(&setup.chain.state().view(), height)
                .unwrap();
            assert_eq!(
                verifier.verify(&proof).unwrap().result(),
                setup.chain.committed(height).result()
            );
            let row = publish(
                output,
                &format!("proof-{height:04}.norito"),
                &norito::encode_canonical(&proof).unwrap(),
            );
            norito::json!({"height":height,"original":row})
        })
        .collect()
}
fn export_setup(cohort: &Cohort, output: &Path) {
    let base_setup_sha256 = verify_base_setup(&cohort.setup);
    make_output(output);
    let owners: Vec<_> = cohort
        .accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let row = publish(
                output,
                &format!("account-{index:03}.norito"),
                &norito::encode_canonical(account).unwrap(),
            );
            norito::json!({"index":index,"seed":(FIRST_SEED + index as u8),"account":row})
        })
        .collect();
    let history = history(&cohort.setup, output);
    let manifest = norito::json!({
        "schema":SETUP,"base_setup_sha256":base_setup_sha256,
        "network_hex":(hex::encode(cohort.setup.chain.network_id().as_bytes())),
        "instance_hex":(hex::encode(cohort.setup.chain.instance().0)),
        "registration_height":3,"activation_time_ms":4999,"owner_count":COUNT,"owners":owners,"history":history,
        "registration_elapsed_ms":(cohort.registration_elapsed_ms),
        "scope":"100 distinct accounts registered by genuine StateExecutor execution after unchanged genesis; exact-quorum fixture history, no physical hardware or production authority."
    });
    publish(
        output,
        "cohort.json",
        norito::json::to_json(&manifest).unwrap().as_bytes(),
    );
    finish_output(output);
}
fn selected_setup(cohort: &Cohort) -> [u8; 32] {
    let path = selected("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_SETUP");
    let expected = pin("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_SETUP_SHA256");
    let bytes = read(&path, 256 << 10, expected);
    let manifest: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(manifest["schema"].as_str(), Some(SETUP));
    assert_eq!(
        manifest["base_setup_sha256"].as_str(),
        Some(verify_base_setup(&cohort.setup).as_str())
    );
    assert_eq!(
        word(&manifest["network_hex"]),
        *cohort.setup.chain.network_id().as_bytes()
    );
    assert_eq!(
        word(&manifest["instance_hex"]),
        cohort.setup.chain.instance().0
    );
    assert_eq!(manifest["registration_height"].as_u64(), Some(3));
    assert_eq!(manifest["activation_time_ms"].as_u64(), Some(4_999));
    assert_eq!(manifest["owner_count"].as_u64(), Some(COUNT as u64));
    let rows = manifest["owners"].as_array().unwrap();
    assert_eq!(rows.len(), COUNT);
    for (index, (row, account)) in rows.iter().zip(&cohort.accounts).enumerate() {
        assert_eq!(row["index"].as_u64(), Some(index as u64));
        assert_eq!(
            row["seed"].as_u64(),
            Some(u64::from(FIRST_SEED) + index as u64)
        );
        assert_eq!(
            original(
                path.parent().unwrap(),
                &row["account"],
                &format!("account-{index:03}.norito"),
                4096
            ),
            norito::encode_canonical(account).unwrap()
        );
    }
    let proofs = manifest["history"].as_array().unwrap();
    assert_eq!(proofs.len(), 3);
    for (index, row) in proofs.iter().enumerate() {
        let height = index as u64 + 1;
        assert_eq!(row["height"].as_u64(), Some(height));
        let expected =
            crate::sumeragi::finality::build_proof(&cohort.setup.chain.state().view(), height)
                .unwrap();
        assert_eq!(
            original(
                path.parent().unwrap(),
                &row["original"],
                &format!("proof-{height:04}.norito"),
                16 << 20
            ),
            norito::encode_canonical(&expected).unwrap()
        );
    }
    expected
}

fn activation_finality(block: &VerifiedSumeragiBlock, signed: &SignedTransaction) -> Vec<u8> {
    let entrypoint_hash = signed.hash_as_entrypoint();
    let certified = block.block();
    let index = certified
        .network_entrypoints()
        .position(|entry| entry.hash() == entrypoint_hash)
        .unwrap() as u32;
    let (output_index, _) = certified.network_output_at(index).unwrap();
    KagemushaWalletLedgerFinalityV1 {
        version: 1,
        entrypoint_hash,
        certificate: SumeragiCommitCertificateV1::from_verified(block).unwrap(),
        inclusion: CertifiedTransactionInclusionV1 {
            entry_proof: certified.network_input_proof(index).unwrap(),
            output: certified.execution_outputs()[output_index as usize].clone(),
            output_proof: certified.output_proof(output_index).unwrap(),
        },
    }
    .to_canonical_bytes()
    .unwrap()
}

fn execute_and_export(mut cohort: Cohort, output: &Path) {
    let setup_sha256 = selected_setup(&cohort);
    let target_path = selected("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_TARGET");
    let target_sha256 = pin("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_TARGET_SHA256");
    let target_bytes = read(&target_path, 2 << 20, target_sha256);
    let target: norito::json::Value = norito::json::from_slice(&target_bytes).unwrap();
    assert_eq!(target["schema"].as_str(), Some(TARGET));
    assert_eq!(target["chain_id"].as_str(), Some(CHAIN));
    assert_eq!(target["owner_count"].as_u64(), Some(COUNT as u64));
    assert_eq!(word(&target["cohort_setup_sha256"]), setup_sha256);
    assert_eq!(
        word(&target["network_hex"]),
        *cohort.setup.chain.network_id().as_bytes()
    );
    assert_eq!(
        word(&target["instance_hex"]),
        cohort.setup.chain.instance().0
    );
    let scheme_id = word(&target["scheme_id"]);
    let manifest_digest = word(&target["manifest_digest"]);
    let pack = read(
        &selected("KAGEMUSHA_NATIVE_VERIFIER_PACK"),
        VERIFIER_PACK_MAX_BYTES_V1,
        pin("KAGEMUSHA_NATIVE_VERIFIER_PACK_SHA256"),
    );
    let installed = InstalledVerifierPackV1::load(
        &pack,
        InstallationV1 {
            scheme_id,
            manifest_digest,
        },
    )
    .unwrap();
    let scheme = *installed.verifier().scheme();
    assert_eq!(
        scheme.network_id,
        *cohort.setup.chain.network_id().as_bytes()
    );
    let owners = target["owners"].as_array().unwrap();
    assert_eq!(owners.len(), COUNT);
    let mut wallet_ids = BTreeSet::new();
    let mut activations = Vec::new();
    let mut signed_activations = Vec::new();
    let mut entrypoints = BTreeSet::new();
    for (index, (row, account)) in owners.iter().zip(&cohort.accounts).enumerate() {
        assert_eq!(row["index"].as_u64(), Some(index as u64));
        assert_eq!(
            row["seed"].as_u64(),
            Some(u64::from(FIRST_SEED) + index as u64)
        );
        assert_eq!(row["originals"].as_array().unwrap().len(), 7);
        let root = target_path.parent().unwrap();
        assert_eq!(
            owner_original(root, row, index, "account.norito", 4096),
            norito::encode_canonical(account).unwrap()
        );
        assert_eq!(
            owner_original(root, row, index, "asset.norito", 4096),
            norito::encode_canonical(&cohort.setup.asset).unwrap()
        );
        let raw = owner_original(
            root,
            row,
            index,
            "activation.norito",
            KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1,
        );
        let typed = KagemushaWalletActivationV1::decode_canonical(&raw, &scheme_id).unwrap();
        let account_digest = kagemusha_wallet_account_digest_v1(account).unwrap();
        assert_eq!(typed.credential.body.account_digest, account_digest);
        assert_eq!(word(&row["account_digest_hex"]), account_digest);
        assert_eq!(word(&row["wallet_id_hex"]), typed.credential.body.wallet_id);
        assert!(
            wallet_ids.insert(typed.credential.body.wallet_id),
            "100 independent wallet IDs"
        );
        assert_eq!(typed.asset, cohort.setup.asset);
        assert_eq!(
            typed.credential.to_canonical_bytes().unwrap(),
            owner_original(
                root,
                row,
                index,
                "credential.norito",
                KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
            )
        );
        assert_eq!(
            norito::encode_canonical(&typed.certificates).unwrap(),
            owner_original(root, row, index, "certificates.norito", 32_768)
        );
        assert_eq!(
            norito::encode_canonical(&typed.bootstrap).unwrap(),
            owner_original(
                root,
                row,
                index,
                "bootstrap.norito",
                KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
            )
        );
        let supplied = owner_original(root, row, index, "activate-signed.wire", 2 << 20);
        let signed = SignedTransaction::decode_all_versioned(&supplied).unwrap();
        assert_eq!(
            signed.encode_wire_v1().unwrap(),
            supplied,
            "retain and execute the exact original wire"
        );
        assert_eq!(signed.authority(), account);
        assert_eq!(signed.network_id(), Some(&cohort.setup.chain.network_id()));
        assert_eq!(signed.creation_time(), Duration::from_millis(4_999));
        assert_eq!(signed.time_to_live(), Some(Duration::from_secs(100)));
        let Executable::Instructions(instructions) = signed.instructions() else {
            panic!("one native Activate instruction required")
        };
        assert_eq!(instructions.len(), 1);
        let instruction = instructions[0]
            .as_any()
            .downcast_ref::<KagemushaWalletLedgerV1>()
            .unwrap();
        assert_eq!(
            instruction,
            &KagemushaWalletLedgerV1::new(scheme_id, Action::Activate(raw.clone()))
        );
        assert!(
            entrypoints.insert(signed.hash_as_entrypoint()),
            "100 independently signed transaction identities"
        );
        signed_activations.push(signed);
        activations.push((raw, typed.credential.body.wallet_id, account_digest));
    }
    make_output(output);
    let began = Instant::now();
    let setup = &mut cohort.setup;
    let register = KagemushaWalletLedgerV1::new(
        scheme_id,
        Action::Register {
            scheme: scheme.to_canonical_bytes().unwrap(),
            asset: norito::encode_canonical(&setup.asset).unwrap(),
            reserve: setup.reserve.clone(),
            balance_scope: AssetBalanceScope::Global,
        },
    );
    let transaction = setup.chain.sign(&key(95), [register.into()], 3_999);
    assert_eq!(setup.chain.commit_at(4_000, vec![transaction]), [true]);
    let install = KagemushaWalletLedgerV1::new(
        scheme_id,
        Action::InstallVerifierPack {
            asset: setup.asset.asset_digest(),
            manifest_digest,
            pack,
        },
    );
    let transaction = setup.chain.sign(&key(95), [install.into()], 4_999);
    assert_eq!(setup.chain.commit_at(5_000, vec![transaction]), [true]);
    let block_budget = usize::try_from(
        native(setup)
            .initial_chain_parameters()
            .unwrap()
            .max_block_bytes,
    )
    .unwrap()
        / 2;
    let mut completed = Vec::new();
    let mut next = 0;
    while next < COUNT {
        let height = setup.chain.height() + 1;
        let now = height * 1_000;
        let mut batch = Vec::new();
        let mut bytes = 0;
        let first = next;
        while next < COUNT && batch.len() < 20 {
            let signed = signed_activations[next].clone();
            assert!(
                Duration::from_millis(now)
                    < signed.creation_time() + signed.time_to_live().unwrap(),
                "queued original must remain inside its signed transaction lifetime"
            );
            let wire = signed.encode_wire_v1().unwrap();
            assert!(
                wire.len() <= block_budget,
                "one valid activation must fit the existing block policy"
            );
            if bytes + wire.len() > block_budget {
                break;
            }
            bytes += wire.len();
            batch.push(signed);
            next += 1;
        }
        assert!(!batch.is_empty());
        let results = setup.chain.commit_at(now, batch.clone());
        assert_eq!(
            results,
            vec![true; batch.len()],
            "each Bootstrap/Activate must actually execute"
        );
        let mut verifier = native(setup);
        let mut at = None;
        for h in 1..=height {
            let proof =
                crate::sumeragi::finality::build_proof(&setup.chain.state().view(), h).unwrap();
            at = Some(verifier.verify(&proof).unwrap());
        }
        let at = at.unwrap();
        for (offset, signed) in batch.iter().enumerate() {
            let index = first + offset;
            let signed_row = publish(
                output,
                &format!("activation-{index:03}.signed.norito"),
                &signed.encode_wire_v1().unwrap(),
            );
            let finality = activation_finality(&at, signed);
            let finality_row = publish(
                output,
                &format!("activation-{index:03}.finality.norito"),
                &finality,
            );
            completed.push(norito::json!({"index":index,"seed":(FIRST_SEED + index as u8),
                "account_digest_hex":(hex::encode(activations[index].2)),"wallet_id_hex":(hex::encode(activations[index].1)),
                "activation_height":height,"signed_transaction":signed_row,"finality":finality_row}));
        }
    }
    assert_eq!(completed.len(), COUNT);
    {
        let view = setup.chain.state().view();
        for (index, (_, wallet, _)) in activations.iter().enumerate() {
            // The public canonical ledger key's wallet tag is 2, with no entry word.
            let ledger_key = KagemushaWalletLedgerKeyV1::from_parts(2, scheme_id, *wallet, [0; 32]);
            let raw = view
                .world()
                .kagemusha_wallet_ledger()
                .get(&ledger_key)
                .unwrap();
            crate::kagemusha_wallet_v1::validate_row(&ledger_key, raw).unwrap();
            let row: WalletRecord = canonical(raw);
            assert_eq!(row.phase, Phase::Active);
            assert_eq!(row.asset, setup.asset.asset_digest());
            assert_eq!(row.next_load, 0);
            assert_ne!(row.activation, [0; 32]);
            assert!(
                view.world()
                    .assets()
                    .get(&AssetId::of(
                        setup.asset.asset.clone(),
                        cohort.accounts[index].clone()
                    ))
                    .is_none()
            );
        }
    }
    let elapsed = began.elapsed();
    assert!(
        elapsed < APP_STAGE_LIMIT,
        "100 native activations exceeded the iOS stage deadline"
    );
    let history = history(setup, output);
    let manifest = norito::json!({"schema":FINALITY,"cohort_setup_sha256":(hex::encode(setup_sha256)),
        "target_sha256":(hex::encode(target_sha256)),"network_hex":(hex::encode(setup.chain.network_id().as_bytes())),
        "instance_hex":(hex::encode(setup.chain.instance().0)),"owner_count":COUNT,"scheme_registration_height":4,
        "installation_height":5,"final_height":(setup.chain.height()),"activation_elapsed_ms":(elapsed.as_millis()),
        "app_stage_deadline_ms":90_000,"owners":completed,"history":history,
        "assertions":{"all_accounts_registered":true,"all_bootstrap_activate_executed":true,"all_wallet_rows_active":true,"all_next_load_zero":true,"all_owners_zero_online_balance":true},
        "scope":"Isolated StateExecutor, original native Bootstrap proofs, 100 distinct signed Activate transactions and exact-quorum certified inclusion. Engineering issuer/platform fixtures; not physical hardware or live Taira evidence."
    });
    publish(
        output,
        "finality.json",
        norito::json::to_json(&manifest).unwrap().as_bytes(),
    );
    finish_output(output);
}

#[test]
fn hundred_distinct_cohort_accounts_are_executed_after_unchanged_genesis() {
    let cohort = cohort();
    assert_eq!(cohort.accounts.len(), COUNT);
    assert_eq!(cohort.setup.chain.height(), 3);
    assert_eq!(
        cohort.setup.chain.genesis().encode_wire().unwrap(),
        start().chain.genesis().encode_wire().unwrap()
    );
}

#[test]
#[ignore = "explicit export of 100 actually executed fixture accounts against the pinned existing base setup"]
fn export_executed_enrollment_cohort_setup() {
    let output = selected("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_SETUP_OUTPUT");
    export_setup(&cohort(), &output);
}

#[test]
#[ignore = "requires the current pinned native 100-owner Bootstrap/Activate outputs and verifier pack"]
fn execute_hundred_native_enrollment_activations_and_export_finality() {
    let output: PathBuf = selected("KAGEMUSHA_NATIVE_ENROLLMENT_COHORT_FINALITY_OUTPUT");
    execute_and_export(cohort(), &output);
}
