//! Genuine actual-StateExecutor Register proof generation; no Load event or wallet grant.
//! TODO: Run this artifact-dependent gate and its mutation consumer before compact intake ships.
use super::*;
use crate::{
    kagemusha_wallet_artifacts_v1::producer_inventory::{
        CATALOG_MAX_BYTES_V1, open_pinned_engineering_finality_sources,
    },
    kagemusha_wallet_artifacts_v1::{
        InstallationV1, InstalledVerifierPackV1, VERIFIER_PACK_MAX_BYTES_V1,
    },
    kagemusha_wallet_finality_v1::server::{
        ServerFinalityCancellationV1, ServerFinalityLimitsV1, ServerFinalityStorageV1,
        ServerFinalityV1,
    },
};
use iroha_kagemusha_proof::finality::native::HistoryPrefix;
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write as _,
    path::PathBuf,
};

const CAPTURE_SCHEMA: &str = "iroha.kagemusha.executed-terminal-registration.v1";
const CAPTURE_MAX: usize = 16 * 1024;
const REGISTER_HEIGHT: u64 = 5;
const CACHE_BYTES: usize = 512 << 20;

fn pin(name: &str) -> [u8; 32] {
    let encoded = std::env::var(name).expect("independently captured exact SHA-256 pin");
    let value: [u8; 32] = hex::decode(&encoded).unwrap().try_into().unwrap();
    assert_eq!(hex::encode(value), encoded);
    assert_ne!(value, [0; 32]);
    value
}

fn named(directory: &PrivateDirectory, name: &str, maximum: usize) -> Vec<u8> {
    directory.revalidate().unwrap();
    let bytes = iroha_fs::read_private(directory.path().join(name), maximum)
        .unwrap()
        .to_vec();
    directory.revalidate().unwrap();
    bytes
}

fn named_pin(
    directory: &PrivateDirectory,
    name: &str,
    maximum: usize,
    expected: BlobV1,
) -> Vec<u8> {
    length(expected, maximum).unwrap();
    let bytes = named(directory, name, maximum);
    assert_eq!(
        BlobV1::of(&bytes),
        expected,
        "exact selected original {name}"
    );
    bytes
}

fn capture_rows(value: &norito::json::Value) -> BTreeMap<String, BlobV1> {
    assert_eq!(value["schema"].as_str(), Some(CAPTURE_SCHEMA));
    assert_eq!(value["registration_height"].as_u64(), Some(REGISTER_HEIGHT));
    assert_eq!(
        value["following_height"].as_u64(),
        Some(REGISTER_HEIGHT + 1)
    );
    assert_eq!(value["instruction_index"].as_u64(), Some(0));
    let failed = value["failed_registration_heights"].as_array().unwrap();
    assert_eq!(
        failed
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect::<Vec<_>>(),
        [3, 4]
    );
    let expected = [
        "scheme.norito",
        "asset.norito",
        "signed-genesis.wire",
        "committed.norito",
        "terminal-block.wire",
        "native-registration.norito",
        "successor-proof.norito",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain((1..=REGISTER_HEIGHT).map(|height| format!("proof-{height}.norito")))
    .collect::<BTreeSet<_>>();
    let rows = value["originals"].as_array().unwrap();
    assert_eq!(rows.len(), expected.len());
    let mut selected = BTreeMap::new();
    for row in rows {
        let name = row["name"].as_str().unwrap().to_owned();
        assert!(expected.contains(&name));
        let digest = row["sha256"].as_str().unwrap();
        let sha256: [u8; 32] = hex::decode(digest).unwrap().try_into().unwrap();
        assert_eq!(hex::encode(sha256), digest);
        let blob = BlobV1 {
            sha256,
            bytes: row["bytes"].as_u64().unwrap(),
        };
        length(blob, REGISTRATION_PROOF_MAX_BYTES_V1).unwrap();
        assert!(selected.insert(name, blob).is_none());
    }
    assert_eq!(selected.keys().cloned().collect::<BTreeSet<_>>(), expected);
    selected
}

fn put(directory: &PrivateDirectory, name: &str, bytes: &[u8], maximum: usize) {
    assert!(!bytes.is_empty() && bytes.len() <= maximum);
    let mut file = directory.create_retained_private(name, maximum).unwrap();
    file.write_all(bytes).unwrap();
    file.seal_read_only().unwrap();
}
fn immutable(directory: &PrivateDirectory, bytes: &[u8], maximum: usize) -> BlobV1 {
    let blob = BlobV1::of(bytes);
    length(blob, maximum).unwrap();
    put(directory, &hex::encode(blob.sha256), bytes, maximum);
    blob
}

fn signed_inputs() -> (InstallationV1, Vec<u8>, Vec<u8>) {
    let catalog = PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG").unwrap());
    let pack = iroha_fs::read_regular(
        catalog.join("engineering-verifier-pack.norito"),
        VERIFIER_PACK_MAX_BYTES_V1,
    )
    .unwrap()
    .to_vec();
    let inventory = iroha_fs::read_regular(
        catalog.join("producer-inventory.norito"),
        CATALOG_MAX_BYTES_V1,
    )
    .unwrap()
    .to_vec();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&pack)),
        pin("KAGEMUSHA_WALLET_CATALOG_PACK_SHA256")
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&inventory)),
        pin("KAGEMUSHA_WALLET_CATALOG_INVENTORY_SHA256")
    );
    let installation = InstallationV1 {
        scheme_id: pin("KAGEMUSHA_WALLET_SCHEME_ID"),
        manifest_digest: pin("KAGEMUSHA_WALLET_MANIFEST_DIGEST"),
    };
    (installation, pack, inventory)
}

#[test]
#[ignore = "requires independently pinned signed catalog; exports exact authenticated scheme for the Core execution fixture"]
fn export_authenticated_registration_fixture_selection() {
    let (installation, pack, inventory) = signed_inputs();
    let installed = InstalledVerifierPackV1::load(&pack, installation).unwrap();
    let authenticated = installed
        .authenticate_producer_inventory(&inventory)
        .unwrap();
    assert_eq!(
        authenticated.installation(),
        (installation.scheme_id, installation.manifest_digest)
    );
    let original = &installed.originals().scheme;
    KagemushaWalletSchemeV1::decode_canonical(original, &installation.scheme_id).unwrap();
    let path = PathBuf::from(
        std::env::var_os("KAGEMUSHA_REGISTER_SELECTION_OUTPUT")
            .expect("fresh selected-original output"),
    );
    let parent = PrivateDirectory::open_exact(path.parent().unwrap()).unwrap();
    let output = parent.create_child(path.file_name().unwrap()).unwrap();
    put(&output, "scheme.norito", original, 4096);
    let receipt = norito::json!({
        "schema": "iroha.kagemusha.registration-fixture-selection.v1",
        "scope": "Exact authenticated signed-pack scheme DATA for actual Core execution; no source graph qualification, Register, proof, wallet or deployment authority.",
        "scheme_id": (hex::encode(installation.scheme_id)),
        "scheme_sha256": (hex::encode(Sha256::digest(original))), "scheme_bytes": (original.len()),
        "signed_pack_sha256": (hex::encode(Sha256::digest(&pack))),
        "signed_inventory_sha256": (hex::encode(Sha256::digest(&inventory))),
    });
    put(
        &output,
        "selection.json",
        &norito::json::to_vec(&receipt).unwrap(),
        CAPTURE_MAX,
    );
    output.sync().unwrap();
    parent.sync().unwrap();
}

// Native verification always precedes server append, including on reopened exact journals.
// Only six real blocks are involved in this fixture; no fake history state is constructed.
fn produce(
    producer: &mut ServerFinalityV1,
    genesis: &SumeragiFinalityVerifier,
    capture: &PrivateDirectory,
    rows: &BTreeMap<String, BlobV1>,
    committed: &CommittedTransaction,
    terminal_bytes: &[u8],
    scheme: &KagemushaWalletSchemeV1,
    native: &FinalizedKagemushaWalletRegistrationV1,
) -> (HistoryPrefix, HistoryPrefix) {
    let mut prefix = producer.genesis().unwrap();
    assert_eq!(prefix.state().next_height, 2);
    let mut verifier = genesis.clone();
    for height in 1..=REGISTER_HEIGHT {
        let name = format!("proof-{height}.norito");
        let proof: SumeragiFinalityProof = decode(&named_pin(
            capture,
            &name,
            REGISTRATION_PROOF_MAX_BYTES_V1,
            rows[&name],
        ))
        .unwrap();
        assert_eq!(proof.height(), height);
        let verified = verifier.verify(&proof).unwrap();
        if height == 1 {
            assert_eq!(
                proof.block_wire,
                named_pin(
                    capture,
                    "signed-genesis.wire",
                    MAX_FINALITY_BLOCK_BYTES,
                    rows["signed-genesis.wire"]
                )
            );
        } else {
            prefix = producer.append(&prefix, &verified).unwrap();
        }
        assert_eq!(prefix.state().next_height, height + 1);
        if height == REGISTER_HEIGHT {
            assert_eq!(verified.block().encode_wire().unwrap(), terminal_bytes);
            let selected = verify_finalized_kagemusha_wallet_registration_v1(
                &verified,
                committed,
                genesis.initial_epoch().network_id,
                genesis.chain_id(),
                scheme,
                native.asset().asset_digest(),
                0,
            )
            .unwrap();
            super::genuine::same_registration(native, &selected);
        }
    }
    let following: SumeragiFinalityProof = decode(&named_pin(
        capture,
        "successor-proof.norito",
        REGISTRATION_PROOF_MAX_BYTES_V1,
        rows["successor-proof.norito"],
    ))
    .unwrap();
    assert_eq!(following.height(), REGISTER_HEIGHT + 1);
    let verified = verifier.verify(&following).unwrap();
    let later = producer.append(&prefix, &verified).unwrap();
    assert_eq!(later.state().next_height, REGISTER_HEIGHT + 2);
    (prefix, later)
}

#[test]
#[ignore = "requires actual StateExecutor Register capture, signed finality inventory, authenticated code recipes and bounded proving cache"]
fn generate_genuine_terminal_register_history_for_compact_differential() {
    let capture = PrivateDirectory::open_exact(
        std::env::var_os("KAGEMUSHA_REGISTER_CAPTURE")
            .expect("exact private executed Register capture"),
    )
    .unwrap();
    let manifest_original = named(&capture, "capture.json", CAPTURE_MAX);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&manifest_original)),
        pin("KAGEMUSHA_REGISTER_CAPTURE_SHA256")
    );
    let manifest: norito::json::Value = norito::json::from_slice(&manifest_original).unwrap();
    let rows = capture_rows(&manifest);
    let output_path = PathBuf::from(
        std::env::var_os("KAGEMUSHA_COMPACT_REGISTER_OUTPUT").expect("fresh private proof output"),
    );
    let parent = PrivateDirectory::open_exact(output_path.parent().unwrap()).unwrap();
    let output = parent
        .create_child(output_path.file_name().unwrap())
        .unwrap();

    // Actual signed descriptor/VK admission. No wallet PKs or complete wallet grant.
    let (installed, graph, genesis, verifier_originals) =
        open_pinned_engineering_finality_sources(&output.path().join("verifier-admission"));
    let scheme = KagemushaWalletSchemeV1::decode_canonical(
        &installed.originals().scheme,
        &graph.installation().0,
    )
    .unwrap();
    assert_eq!(
        named_pin(&capture, "scheme.norito", 4096, rows["scheme.norito"]),
        installed.originals().scheme
    );
    assert_eq!(manifest["chain_id"].as_str(), Some(genesis.chain_id()));
    assert_eq!(
        manifest["network_hex"].as_str(),
        Some(hex::encode(scheme.network_id).as_str())
    );
    assert_eq!(
        manifest["instance_hex"].as_str(),
        Some(hex::encode(genesis.instance().0).as_str())
    );
    assert_eq!(
        manifest["scheme_id_hex"].as_str(),
        Some(hex::encode(scheme.scheme_id()).as_str())
    );
    let native_bytes = named_pin(
        &capture,
        "native-registration.norito",
        REGISTRATION_SOURCE_MAX_BYTES_V1,
        rows["native-registration.norito"],
    );
    let native_source = RegistrationSourceV1::decode_canonical(&native_bytes).unwrap();
    let native =
        verify_registration_source_v1(&native_source, &genesis, &scheme, || false).unwrap();
    assert_eq!(native.height(), REGISTER_HEIGHT);
    assert_eq!(native.instruction_index(), 0);
    assert_eq!(
        manifest["asset_digest_hex"].as_str(),
        Some(hex::encode(native.asset().asset_digest()).as_str())
    );
    assert_eq!(
        manifest["transaction_hash_hex"].as_str(),
        Some(hex::encode(native.transaction_hash()).as_str())
    );
    assert_eq!(
        manifest["block_hash_hex"].as_str(),
        Some(hex::encode(native.block_hash()).as_str())
    );
    assert_eq!(
        named_pin(&capture, "asset.norito", 4096, rows["asset.norito"]),
        native.asset_original()
    );
    let committed_bytes = named_pin(
        &capture,
        "committed.norito",
        REGISTRATION_PROOF_MAX_BYTES_V1,
        rows["committed.norito"],
    );
    assert_eq!(
        native_source.inventory.committed,
        BlobV1::of(&committed_bytes)
    );
    let committed: CommittedTransaction = decode(&committed_bytes).unwrap();
    let terminal_bytes = named_pin(
        &capture,
        "terminal-block.wire",
        MAX_FINALITY_BLOCK_BYTES,
        rows["terminal-block.wire"],
    );
    let terminal = decode_framed_signed_block(&terminal_bytes).unwrap();
    assert_eq!(terminal.encode_wire().unwrap(), terminal_bytes);
    assert_eq!(*terminal.hash().as_ref(), native.block_hash());

    // Only the server owner regenerates/imports exact signed PK identities. The live
    // compiler store is neither opened nor modified. Logical and resident bounds differ.
    let (installation, pack, inventory) = signed_inputs();
    assert_eq!(
        (installation.scheme_id, installation.manifest_digest),
        graph.installation()
    );
    let authenticated = installed
        .authenticate_producer_inventory(&inventory)
        .unwrap();
    let decoded = authenticated.inventory();
    let aggregate = decoded
        .finality
        .originals
        .iter()
        .flat_map(|record| record.lengths)
        .try_fold(0u64, |sum, size| sum.checked_add(size))
        .unwrap();
    assert!(
        aggregate > 0 && aggregate <= 1u64 << 40,
        "explicit engineering logical-original ceiling"
    );
    let largest_key = usize::try_from(
        decoded
            .finality
            .originals
            .iter()
            .map(|record| record.lengths[2])
            .max()
            .unwrap(),
    )
    .unwrap();
    assert!(
        largest_key > 0 && largest_key <= CACHE_BYTES,
        "signed key must fit explicit resident cache limit"
    );
    let limits = ServerFinalityLimitsV1 {
        maximum_key_bytes: largest_key,
        maximum_original_bytes: usize::try_from(aggregate).unwrap(),
        maximum_artifacts: decoded.finality.originals.len(),
        maximum_resident_proving_key_bytes: CACHE_BYTES,
        msm_bytes: 64 << 20,
        maximum_journal_entries: 100_000,
        maximum_journal_bytes: 512 << 20,
    };
    let cache = output.create_child("proving-cache").unwrap();
    let journal = output.create_child("proof-journal").unwrap();
    let storage = || ServerFinalityStorageV1 {
        verifier_originals: verifier_originals.root().unwrap(),
        proving_cache: cache.path(),
        journal: journal.path(),
    };
    let mut producer = ServerFinalityV1::initialize(
        &genesis,
        installation,
        &pack,
        &inventory,
        storage(),
        limits,
        ServerFinalityCancellationV1::default(),
    )
    .unwrap();
    let (prefix, later) = produce(
        &mut producer,
        &genesis,
        &capture,
        &rows,
        &committed,
        &terminal_bytes,
        &scheme,
        &native,
    );
    let original = CompactRegistrationOriginalV1::from_terminal(
        &prefix,
        &terminal,
        &committed,
        &scheme,
        native.asset().asset_digest(),
        0,
        None,
    )
    .unwrap();
    let compact = verify_compact_registration_v1(
        &original,
        &graph,
        &genesis,
        &scheme,
        MemoryBudget::DEFAULT,
        None,
    )
    .unwrap();
    super::genuine::same_registration(&native, &compact);
    let later_history = HistoryOriginalV1::from_prefix(&later)
        .encode_canonical()
        .unwrap();
    HistoryOriginalV1::decode_canonical(&later_history)
        .unwrap()
        .restore_qualified(&graph, MemoryBudget::DEFAULT, None)
        .unwrap();
    let mut substituted = original.clone();
    substituted.history = later_history.clone();
    assert!(
        verify_compact_registration_v1(
            &substituted,
            &graph,
            &genesis,
            &scheme,
            MemoryBudget::DEFAULT,
            None
        )
        .is_err(),
        "valid later prefix cannot authenticate earlier Register"
    );

    // An explicit restart uses existing-only open. Lost selection is never initialized anew.
    drop(producer);
    let mut reopened = ServerFinalityV1::open(
        &genesis,
        installation,
        &pack,
        &inventory,
        storage(),
        limits,
        ServerFinalityCancellationV1::default(),
    )
    .unwrap();
    let (same, same_later) = produce(
        &mut reopened,
        &genesis,
        &capture,
        &rows,
        &committed,
        &terminal_bytes,
        &scheme,
        &native,
    );
    assert_eq!(
        HistoryOriginalV1::from_prefix(&same)
            .encode_canonical()
            .unwrap(),
        original.history
    );
    assert_eq!(
        HistoryOriginalV1::from_prefix(&same_later)
            .encode_canonical()
            .unwrap(),
        later_history
    );
    let bytes = original.encode_canonical().unwrap();
    let originals = output.create_child("differential-originals").unwrap();
    let native_pin = immutable(&originals, &native_bytes, REGISTRATION_SOURCE_MAX_BYTES_V1);
    let compact_pin = immutable(&originals, &bytes, COMPACT_REGISTRATION_MAX_BYTES_V1);
    let later_pin = immutable(&originals, &later_history, HISTORY_ORIGINAL_MAX_BYTES_V1);
    let receipt = norito::json!({
        "schema": "iroha.kagemusha.generated-compact-registration.v1",
        "scope": "Genuine history proof for exact actual-StateExecutor terminal Register; native/compact equality, valid-later-prefix refusal and exact server restart checked. Full mutation consumer remains separate; no wallet, deployment, network or phone qualification.",
        "capture_sha256": (hex::encode(Sha256::digest(&manifest_original))),
        "native_sha256": (hex::encode(native_pin.sha256)), "native_bytes": (native_pin.bytes),
        "compact_sha256": (hex::encode(compact_pin.sha256)), "compact_bytes": (compact_pin.bytes),
        "later_history_sha256": (hex::encode(later_pin.sha256)), "later_history_bytes": (later_pin.bytes),
        "terminal_height": REGISTER_HEIGHT, "scheme_id": (hex::encode(scheme.scheme_id())),
        "asset_digest": (hex::encode(native.asset().asset_digest())),
        "differential_originals": (originals.path().to_str().unwrap()),
        "signed_pack_sha256": (hex::encode(Sha256::digest(&pack))),
        "signed_inventory_sha256": (hex::encode(Sha256::digest(&inventory))),
        "genuine_history": true, "load_event_used": false, "consumer_mutations_run": false,
    });
    put(
        &output,
        "generated.json",
        &norito::json::to_vec(&receipt).unwrap(),
        CAPTURE_MAX,
    );
    originals.sync().unwrap();
    output.sync().unwrap();
    parent.sync().unwrap();
    capture.revalidate().unwrap();
}

#[test]
fn captured_manifest_requires_exact_roles_extents_and_terminal_heights() {
    let rows = ["scheme.norito", "asset.norito", "signed-genesis.wire", "committed.norito", "terminal-block.wire", "native-registration.norito", "successor-proof.norito"]
        .into_iter().map(str::to_owned).chain((1..=REGISTER_HEIGHT).map(|height| format!("proof-{height}.norito")))
        .map(|name| norito::json!({"name": name, "sha256": (hex::encode(Sha256::digest(b"bounded DATA fixture"))), "bytes": 1}))
        .collect::<Vec<_>>();
    let manifest = |schema: &str,
                    height: u64,
                    following: u64,
                    originals: Vec<norito::json::Value>| {
        norito::json!({"schema": schema, "registration_height": height, "following_height": following,
            "instruction_index": 0, "failed_registration_heights": (vec![3, 4]), "originals": originals})
    };
    assert_eq!(
        capture_rows(&manifest(CAPTURE_SCHEMA, 5, 6, rows.clone())).len(),
        12
    );
    for change in 0..7 {
        let mut changed = rows.clone();
        let mut schema = CAPTURE_SCHEMA;
        let mut height = 5;
        let mut following = 6;
        match change {
            0 => {
                changed.pop();
            }
            1 => changed[0] = changed[1].clone(),
            2 => {
                changed[0] = norito::json!({"name": "../scheme.norito", "sha256": ("11".repeat(32)), "bytes": 1})
            }
            3 => {
                changed[0] = norito::json!({"name": "scheme.norito", "sha256": ("11".repeat(32)), "bytes": 0})
            }
            4 => height = 6,
            5 => following = 5,
            _ => schema = "unsigned-or-old-capture",
        }
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                capture_rows(&manifest(schema, height, following, changed))
            }))
            .is_err(),
            "manifest mutation {change}"
        );
    }
}
