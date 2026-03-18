use std::{
    error::Error,
    fmt::Write as FmtWrite,
    fs::{self, File},
    path::{Path, PathBuf},
};

use fastpq_isi::poseidon::{FIELD_MODULUS, MDS, RATE, ROUND_CONSTANTS, STATE_WIDTH};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    metadata::Metadata,
    offline::{
        AppleAppAttestProof, OfflineAllowanceCommitment, OfflinePlatformProof, OfflineReceiptLeaf,
        OfflineSpendReceipt, OfflineWalletCertificate, OfflineWalletPolicy, PoseidonDigest,
        compute_receipts_root,
    },
};
use iroha_primitives::numeric::Numeric;
use norito::json::{self, Value};

use crate::workspace_root;

const DEFAULT_DOMAIN_TAG: &str = "iroha.offline.receipt.merkle.v1";
const SNAPSHOT_VERSION: u64 = 1;
const POSEIDON_PROFILE: &str = "fastpq-goldilocks-width3";
const SAMPLE_CASE_DESCRIPTION: &str =
    "Two receipts hashed in counter order; matches OfflineReceiptLeaf fixtures in tests.";

pub struct OfflinePoseidonFixtureOptions {
    constants_path: PathBuf,
    vectors_path: PathBuf,
    domain_tag: String,
    mirror_sdk: bool,
}

impl OfflinePoseidonFixtureOptions {
    pub fn new(
        constants_path: Option<PathBuf>,
        vectors_path: Option<PathBuf>,
        domain_tag: Option<String>,
        mirror_sdk: bool,
    ) -> Self {
        let root = workspace_root();
        let default_constants = root.join("artifacts/offline_poseidon/constants.ron");
        let default_vectors = root.join("artifacts/offline_poseidon/vectors.json");
        Self {
            constants_path: constants_path.unwrap_or(default_constants),
            vectors_path: vectors_path.unwrap_or(default_vectors),
            domain_tag: domain_tag.unwrap_or_else(|| DEFAULT_DOMAIN_TAG.to_string()),
            mirror_sdk,
        }
    }
}

pub fn generate(options: OfflinePoseidonFixtureOptions) -> Result<(), Box<dyn Error>> {
    write_constants(&options.constants_path)?;
    write_vectors(&options.vectors_path, &options.domain_tag)?;
    println!(
        "wrote Poseidon constants to {}",
        options.constants_path.display()
    );
    println!(
        "wrote Poseidon vectors to {}",
        options.vectors_path.display()
    );
    if options.mirror_sdk {
        mirror_sdk_fixtures(&options.constants_path, &options.vectors_path)?;
    }
    Ok(())
}

fn write_constants(path: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut buffer = String::new();
    writeln!(buffer, "PoseidonSnapshot(")?;
    writeln!(buffer, "    version: {SNAPSHOT_VERSION},")?;
    writeln!(buffer, "    profile: \"{POSEIDON_PROFILE}\",")?;
    writeln!(
        buffer,
        "    field_modulus: \"{}\",",
        format_hex(FIELD_MODULUS)
    )?;
    writeln!(buffer, "    width: {STATE_WIDTH},")?;
    writeln!(buffer, "    rate: {RATE},")?;
    writeln!(buffer, "    full_rounds: 8,")?;
    let partial_rounds = ROUND_CONSTANTS.len() - 8;
    writeln!(buffer, "    partial_rounds: {partial_rounds},")?;
    writeln!(buffer, "    round_constants: [")?;
    for row in ROUND_CONSTANTS.iter() {
        writeln!(
            buffer,
            "        [{}, {}, {}],",
            format_hex(row[0]),
            format_hex(row[1]),
            format_hex(row[2])
        )?;
    }
    writeln!(buffer, "    ],")?;
    writeln!(buffer, "    mds: [")?;
    for row in MDS.iter() {
        writeln!(
            buffer,
            "        [{}, {}, {}],",
            format_hex(row[0]),
            format_hex(row[1]),
            format_hex(row[2])
        )?;
    }
    writeln!(buffer, "    ],")?;
    writeln!(buffer, ")")?;
    fs::write(path, buffer)?;
    Ok(())
}

fn write_vectors(path: &Path, domain_tag: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let cases = vec![empty_case(), sample_receipt_case()];
    let mut manifest = json::Map::new();
    manifest.insert("version".into(), Value::from(SNAPSHOT_VERSION));
    manifest.insert("domain_tag".into(), Value::from(domain_tag));
    manifest.insert("cases".into(), Value::Array(cases));
    let manifest = Value::Object(manifest);
    let mut file = File::create(path)?;
    json::to_writer_pretty(&mut file, &manifest)?;
    Ok(())
}

fn mirror_sdk_fixtures(constants_path: &Path, vectors_path: &Path) -> Result<(), Box<dyn Error>> {
    const SWIFT_DIR: &str = "IrohaSwift/Fixtures/offline_poseidon";
    const ANDROID_DIR: &str = "java/iroha_android/src/test/resources/offline_poseidon";
    let root = workspace_root();
    for dir in [SWIFT_DIR, ANDROID_DIR] {
        let target = root.join(dir);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&target)?;
        let swift_constants = target.join("constants.ron");
        let swift_vectors = target.join("vectors.json");
        fs::copy(constants_path, &swift_constants)?;
        fs::copy(vectors_path, &swift_vectors)?;
        println!("mirrored Poseidon fixtures to {}", target.display());
    }
    Ok(())
}

fn empty_case() -> Value {
    let mut case = json::Map::new();
    case.insert("name".into(), Value::from("empty"));
    case.insert("leaves".into(), Value::Array(Vec::new()));
    case.insert(
        "root_hex".into(),
        Value::from(PoseidonDigest::zero().to_hex_upper()),
    );
    Value::Object(case)
}

fn sample_receipt_case() -> Value {
    let receipts = vec![sample_receipt(5, "a"), sample_receipt(6, "b")];
    let root = compute_receipts_root(&receipts).expect("compute Poseidon root");
    let mut leaves = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        let leaf = OfflineReceiptLeaf::from_receipt(&receipt).expect("leaf encoding");
        let amount_mantissa = leaf
            .amount
            .try_mantissa_u128()
            .expect("amount mantissa must fit u128");
        let amount_scale = leaf.amount.scale();
        let mut entry = json::Map::new();
        entry.insert("tx_id_hex".into(), Value::from(hash_hex(&leaf.tx_id)));
        entry.insert("amount_mantissa".into(), mantissa_value(amount_mantissa));
        entry.insert("amount_scale".into(), Value::from(amount_scale));
        entry.insert("counter".into(), Value::from(leaf.counter));
        entry.insert(
            "receiver_hash_hex".into(),
            Value::from(hash_hex(&leaf.receiver_hash)),
        );
        entry.insert(
            "invoice_hash_hex".into(),
            Value::from(hash_hex(&leaf.invoice_hash)),
        );
        entry.insert(
            "platform_proof_hash_hex".into(),
            Value::from(hash_hex(&leaf.platform_proof_hash)),
        );
        leaves.push(Value::Object(entry));
    }
    let mut case = json::Map::new();
    case.insert("name".into(), Value::from("sample-app-attest"));
    case.insert("description".into(), Value::from(SAMPLE_CASE_DESCRIPTION));
    case.insert("leaves".into(), Value::Array(leaves));
    case.insert("root_hex".into(), Value::from(root.to_hex_upper()));
    Value::Object(case)
}

fn sample_receipt(counter: u64, seed: &str) -> OfflineSpendReceipt {
    let account = sample_account();
    let asset = sample_asset(&account);
    let certificate = sample_certificate(&account, &asset);
    let key_pair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
    OfflineSpendReceipt {
        tx_id: Hash::new(format!("tx-{seed}").as_bytes()),
        from: account.clone(),
        to: account.clone(),
        asset,
        amount: Numeric::new(250, 0),
        issued_at_ms: 1,
        invoice_id: format!("invoice-{seed}"),
        platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
            key_id: seed.to_string(),
            counter,
            assertion: vec![1, 2, 3],
            challenge_hash: Hash::new(b"challenge"),
        }),
        platform_snapshot: None,
        sender_certificate_id: certificate.certificate_id(),
        sender_signature: Signature::new(key_pair.private_key(), b"receipt"),
        build_claim: None,
    }
}

fn sample_account() -> AccountId {
    let key_pair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
    AccountId::new(key_pair.public_key().clone())
}

fn sample_asset(owner: &AccountId) -> AssetId {
    let definition: AssetDefinitionId = "usd#wonderland".parse().expect("definition");
    AssetId::new(definition, owner.clone())
}

fn sample_certificate(owner: &AccountId, asset: &AssetId) -> OfflineWalletCertificate {
    let spend_key = KeyPair::from_seed(vec![0x02; 32], Algorithm::Ed25519);
    let operator = KeyPair::from_seed(vec![0x03; 32], Algorithm::Ed25519);
    let _ = owner;
    let operator_account = AccountId::new(operator.public_key().clone());
    OfflineWalletCertificate {
        controller: owner.clone(),
        operator: operator_account,
        allowance: OfflineAllowanceCommitment {
            asset: asset.clone(),
            amount: Numeric::new(1_000, 0),
            commitment: vec![0u8; 32],
        },
        spend_public_key: spend_key.public_key().clone(),
        attestation_report: vec![],
        issued_at_ms: 0,
        expires_at_ms: 1,
        policy: OfflineWalletPolicy {
            max_balance: Numeric::new(10_000, 0),
            max_tx_value: Numeric::new(1_000, 0),
            expires_at_ms: 1,
        },
        operator_signature: Signature::new(operator.private_key(), b"certificate"),
        metadata: Metadata::default(),
        verdict_id: None,
        attestation_nonce: None,
        refresh_at_ms: None,
    }
}

fn hash_hex(hash: &Hash) -> String {
    hex::encode_upper(hash.as_ref())
}

fn mantissa_value(mantissa: u128) -> Value {
    if mantissa <= u128::from(u64::MAX) {
        Value::Number(json::Number::from(mantissa as u64))
    } else {
        Value::String(mantissa.to_string())
    }
}

fn format_hex(value: u64) -> String {
    let digits = format!("{value:016x}");
    let mut formatted = String::with_capacity(2 + digits.len() + digits.len() / 4);
    formatted.push_str("0x");
    for (idx, ch) in digits.chars().enumerate() {
        if idx != 0 && idx % 4 == 0 {
            formatted.push('_');
        }
        formatted.push(ch);
    }
    formatted
}
