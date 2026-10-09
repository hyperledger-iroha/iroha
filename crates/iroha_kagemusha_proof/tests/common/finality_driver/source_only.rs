//! Source compilation from a pinned canonical executed setup, without a Load witness.
//!
//! The external setup receipt selects the execution provenance. This intake independently
//! validates the signed manifest, original registrations and contiguous native H1/H2 proofs;
//! it does not execute ledger state or turn compilation into deployment authority.

use super::*;
use iroha_data_model::{
    account::AccountId,
    asset::AssetBalancePolicy,
    block::{consensus::SumeragiRootScope, decode_framed_signed_block},
    isi::RegisterBox,
    kagemusha::KagemushaWalletAssetScopeV1,
    sumeragi_finality::{
        ConsensusSchedule, FinalityValidator, ScheduledSlot, SumeragiFinalityProof,
        SumeragiFinalityVerifier, genesis_epoch,
    },
};
use iroha_genesis::{RawGenesisTransaction, validate_prepared_genesis_bundle};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA: &str = "iroha.kagemusha.executed-ledger-setup.v1";
const ORIGINAL_MAX: usize = 16 << 20;
const SETUP_MAX: usize = 32 << 10;
const ORIGINALS: [&str; 9] = [
    "signed-genesis.wire",
    "genesis-manifest.json",
    "account.norito",
    "account-b.norito",
    "account-c.norito",
    "reserve.norito",
    "asset.norito",
    "registration-proof.norito",
    "capture.json",
];

pub(super) struct Setup {
    pub(super) manifest: Vec<u8>,
    pub(super) originals: BTreeMap<String, Vec<u8>>,
    pub(super) anchor: HistoryAnchor,
}

fn need(valid: bool) -> Result<(), Error> {
    if valid { Ok(()) } else { Err(Error::Input) }
}

fn original_limit(name: &str) -> usize {
    match name {
        "account.norito" | "account-b.norito" | "account-c.norito" | "reserve.norito"
        | "asset.norito" => 4096,
        "capture.json" => 1 << 20,
        _ => ORIGINAL_MAX,
    }
}

fn read_originals(root: &Path, setup: &Value) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let records = value(setup, "originals")?.as_array().ok_or(Error::Input)?;
    need(records.len() == ORIGINALS.len())?;
    let mut originals = BTreeMap::new();
    for record in records {
        let name = value(record, "name")?.as_str().ok_or(Error::Input)?;
        need(ORIGINALS.contains(&name) && !originals.contains_key(name))?;
        let length = value(record, "bytes")?.as_u64().ok_or(Error::Input)?;
        let maximum = original_limit(name);
        need(length > 0 && length <= maximum as u64)?;
        let original = read_bounded(&root.join(name), maximum)?;
        need(original.len() as u64 == length)?;
        need(<[u8; 32]>::from(Sha256::digest(&original)) == fixed(record, "sha256")?)?;
        originals.insert(name.to_owned(), original);
    }
    Ok(originals)
}

fn registered_inputs(
    manifest: &RawGenesisTransaction,
    originals: &BTreeMap<String, Vec<u8>>,
    setup: &Value,
) -> Result<(), Error> {
    let accounts = [
        "account.norito",
        "account-b.norito",
        "account-c.norito",
        "reserve.norito",
    ]
    .map(|name| {
        norito::decode_canonical_with_limits::<AccountId>(
            &originals[name],
            norito::canonical_decode_limits(originals[name].len()),
        )
        .map_err(|_| Error::Input)
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    need(accounts.iter().collect::<BTreeSet<_>>().len() == accounts.len())?;
    let asset: KagemushaWalletAssetScopeV1 = norito::decode_canonical_with_limits(
        &originals["asset.norito"],
        norito::canonical_decode_limits(originals["asset.norito"].len()),
    )
    .map_err(|_| Error::Input)?;
    asset.validate().map_err(|_| Error::Input)?;
    need(asset.asset_digest() == fixed(setup, "asset_digest_hex")?)?;
    for account in accounts {
        need(
            manifest
                .instructions()
                .filter(|instruction| {
                    matches!(instruction.as_any().downcast_ref::<RegisterBox>(),
                Some(RegisterBox::Account(register)) if register.object().id == account)
                })
                .count()
                == 1,
        )?;
    }
    need(
        manifest
            .instructions()
            .filter(|instruction| {
                matches!(instruction.as_any().downcast_ref::<RegisterBox>(),
            Some(RegisterBox::AssetDefinition(register))
                if register.object().id == asset.asset
                && register.object().spec.scale() == Some(asset.scale)
                && register.object().balance_scope_policy == AssetBalancePolicy::Global)
            })
            .count()
            == 1,
    )
}

pub(super) fn validate(root: &Path, pin: [u8; 32]) -> Result<Setup, Error> {
    need(pin != [0; 32] && directory_exists(root)?)?;
    let manifest = read_bounded(&root.join("setup.json"), SETUP_MAX)?;
    need(<[u8; 32]>::from(Sha256::digest(&manifest)) == pin)?;
    let setup: Value = norito::json::from_slice(&manifest).map_err(|_| Error::Input)?;
    need(
        value(&setup, "schema")?.as_str() == Some(SCHEMA)
            && value(&setup, "registration_height")?.as_u64() == Some(1)
            && value(&setup, "certified_height")?.as_u64() == Some(2),
    )?;
    let originals = read_originals(root, &setup)?;
    let anchor = validate_contents(&setup, &originals)?;
    // Re-read the selected manifest after all bounded originals; no partial intake is retained.
    need(read_bounded(&root.join("setup.json"), SETUP_MAX)? == manifest)?;
    Ok(Setup {
        manifest,
        originals,
        anchor,
    })
}

fn validate_contents(
    setup: &Value,
    originals: &BTreeMap<String, Vec<u8>>,
) -> Result<HistoryAnchor, Error> {
    iroha_genesis::init_instruction_registry();
    let raw: RawGenesisTransaction =
        norito::json::from_slice(&originals["genesis-manifest.json"]).map_err(|_| Error::Input)?;
    let genesis =
        decode_framed_signed_block(&originals["signed-genesis.wire"]).map_err(|_| Error::Input)?;
    let first = genesis.external_transactions().next().ok_or(Error::Input)?;
    let signer = first.authority().try_signatory().ok_or(Error::Input)?;
    let validated = validate_prepared_genesis_bundle(
        &originals["signed-genesis.wire"],
        &raw,
        signer,
        genesis.hash(),
    )
    .map_err(|_| Error::Input)?;
    let chain = value(setup, "chain_id")?.as_str().ok_or(Error::Input)?;
    need(raw.chain_id().to_string() == chain)?;
    registered_inputs(&raw, originals, setup)?;
    let epoch = genesis_epoch(validated.block()).map_err(|_| Error::Input)?;
    let roster = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let mut native = SumeragiFinalityVerifier::new(validated.block(), chain, roster)
        .map_err(|_| Error::Input)?;
    need(native.root_scope().map_err(|_| Error::Input)? == SumeragiRootScope::Global)?;
    let parameters = native
        .initial_chain_parameters()
        .map_err(|_| Error::Input)?;
    let schedule =
        ConsensusSchedule::from_genesis(epoch.clone(), parameters).map_err(|_| Error::Input)?;
    for height in [2, 3] {
        need(
            matches!(schedule.get(height), Some(ScheduledSlot::Ready(slot))
            if slot.height == height && slot.epoch == epoch && slot.params == parameters),
        )?;
    }
    let anchor = HistoryAnchor {
        network: *epoch.network_id.as_bytes(),
        instance: native.instance().0,
        initial_context: epoch.context_id().map_err(|_| Error::Input)?,
        initial_epoch: epoch.authorization.epoch,
        parameters: [
            parameters.block_time_ms,
            parameters.payload_retry_interval_ms,
            parameters.exec_budget_ms,
            parameters.apply_budget_ms,
            u64::from(parameters.max_block_bytes),
            parameters.epoch_length_blocks,
        ],
    };
    need(
        anchor.network == fixed(setup, "network_hex")?
            && anchor.instance == fixed(setup, "instance_hex")?,
    )?;
    let capture: Value =
        norito::json::from_slice(&originals["capture.json"]).map_err(|_| Error::Input)?;
    need(
        value(&capture, "version")?.as_u64() == Some(1)
            && value(&capture, "chain_id")?.as_str() == Some(chain)
            && bytes(&capture, "signed_genesis_wire_hex")? == originals["signed-genesis.wire"],
    )?;
    let recorded = value(&capture, "history_anchor")?;
    let recorded_parameters = value(recorded, "parameters")?
        .as_array()
        .ok_or(Error::Input)?
        .iter()
        .map(|item| item.as_u64().ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    need(
        anchor.network == fixed(recorded, "network_hex")?
            && anchor.instance == fixed(recorded, "instance_hex")?
            && anchor.initial_context == fixed(recorded, "initial_context_hex")?
            && Some(anchor.initial_epoch) == value(recorded, "initial_epoch")?.as_u64()
            && anchor.parameters.as_slice() == recorded_parameters,
    )?;
    let proofs: [SumeragiFinalityProof; 2] = norito::decode_canonical_with_limits(
        &originals["registration-proof.norito"],
        norito::canonical_decode_limits(originals["registration-proof.norito"].len()),
    )
    .map_err(|_| Error::Input)?;
    for (index, proof) in proofs.iter().enumerate() {
        let verified = native.verify(proof).map_err(|_| Error::Input)?;
        need(verified.height() == (index + 1) as u64)?;
        if index == 0 {
            need(verified.block().hash() == validated.expected_hash())?;
        }
    }
    Ok(anchor)
}

/// Independently validate a pinned executed setup without key generation or proof creation.
pub fn check(root: &Path, setup_sha256: [u8; 32]) {
    let setup = validate(root, setup_sha256)
        .expect("canonical pinned executed setup and exact H1/H2 proofs");
    tests::check_verified_mutations(&setup);
    eprintln!(
        "FINALITY_SETUP canonical_genesis=true contiguous_native_h1_h2=true source_only=true"
    );
}

/// Compile the complete fixed source graph under a new exclusive directory.
/// This emits no receipt proof, authenticated inventory or wallet installation grant.
pub fn run(root: &Path, setup_root: &Path, setup_sha256: [u8; 32], source_sha256: [u8; 32]) {
    assert_ne!(source_sha256, [0; 32]);
    let setup = validate(setup_root, setup_sha256).expect("canonical pinned executed setup");
    fs::create_dir(root).expect("fresh exclusive source-only output");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let _lock = RunLock::acquire(root).expect("exclusive source-only directory");
    publish(&root.join("setup.json"), &setup.manifest).unwrap();
    for (name, original) in &setup.originals {
        publish(&root.join(name), original).unwrap();
    }
    // Existing metadata snapshot tooling reads this exact source capture name.
    publish(&root.join("fixture.json"), &setup.originals["capture.json"]).unwrap();
    let binary = read_bounded(&std::env::current_exe().unwrap(), 256 << 20).unwrap();
    publish(&root.join("binary.sha256"), &Sha256::digest(binary)).unwrap();
    let provenance = SourceProvenance {
        revision: "canonical executed setup; source-only engineering compilation".into(),
        source_manifest_sha256: source_sha256,
    };
    publish(
        &root.join("provenance.norito"),
        &norito::to_bytes(&provenance).unwrap(),
    )
    .unwrap();
    let limits = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 256 << 20,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_artifacts: 4096,
        maximum_original_bytes: 512_usize << 30,
    };
    let mut artifacts = Progress {
        inner: StreamingCatalog::create(root.join("originals"), limits, WORKING_PK_BYTES).unwrap(),
        writes: 0,
    };
    eprintln!("FINALITY_SOURCE_ONLY compile=true load_witness=false new_proofs=0");
    let compiled = catalog::compile(
        setup.anchor,
        &mut artifacts,
        Parameters {
            pallas: PinnedParams::derive(16).unwrap(),
            vesta: PinnedParams::derive(16).unwrap(),
        },
        limits,
        provenance,
    )
    .expect("complete exact source graph and strict imports");
    let inventory = artifacts.inner.inventory().unwrap();
    publish(&root.join("completed-inventory.norito"), &inventory).unwrap();
    let completion = norito::json!({
        "schema": "iroha.kagemusha.source-only-finality-completion.v1",
        "setup_sha256": (hex_out(&setup_sha256)),
        "source_sha256": (hex_out(&source_sha256)),
        "inventory_sha256": (hex_out(&Sha256::digest(&inventory))),
        "terminal_descriptor": (hex_out(&compiled.terminal.descriptor)),
        "terminal_key": (hex_out(&compiled.terminal.key)),
        "logical_original_bytes": (artifacts.inner.original_bytes()),
        "scope": "Complete source graph and strict key imports only; no Load, proof, wallet grant, phone or deployment qualification.",
    });
    publish(
        &root.join("source-complete.json"),
        norito::json::to_json(&completion).unwrap().as_bytes(),
    )
    .unwrap();
    eprintln!(
        "FINALITY_SOURCE_ONLY complete=true exact_graph=true new_proofs=0 wallet_admission=false"
    );
}

#[cfg(test)]
#[path = "source_only/tests.rs"]
mod tests;
