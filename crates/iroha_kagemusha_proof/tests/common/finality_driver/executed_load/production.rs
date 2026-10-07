//! Exclusive completed-source reconstruction and ordered real history proof campaign.
use super::*;

/// Independently selected completed canonical source graph, never a live producer.
pub struct SourceSelection<'a> {
    /// Existing completed source-only compiler directory, held exclusively while proving.
    pub root: &'a Path,
    /// Exact source-complete.json digest retained independently of this driver.
    pub completion_sha256: [u8; 32],
}

fn retain(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => need(read_bounded(path, bytes.len())? == bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => publish(path, bytes),
        Err(_) => Err(Error::Artifact),
    }
}

/// Produce the original target's compact finality evidence using the complete selected graph.
///
/// The source catalog must already be complete and exclusively available. Only its marked
/// regenerable PK cache may change; exact source inventory, descriptor/VK identities and
/// anchor are checked again before any proof. The output owner retains exact input pins,
/// executable and source manifest across checkpoint resumes. No installed wallet grant is
/// created here, and no incomplete terminal result is published as complete.
pub fn run(
    output: &Path,
    selection: &Selection<'_>,
    source: &SourceSelection<'_>,
    source_sha256: [u8; 32],
) {
    assert_ne!(source_sha256, [0; 32]);
    let selected = validate(selection).expect("selected native executed Load originals");
    assert!(
        directory_exists(source.root).unwrap(),
        "completed source directory exists"
    );
    let _source_lock = RunLock::acquire(source.root).expect("completed source exclusive custody");
    let completion_frame = pinned(
        &source.root.join("source-complete.json"),
        8192,
        source.completion_sha256,
    )
    .expect("independently retained source completion");
    let completion: Value = norito::json::from_slice(&completion_frame).unwrap();
    assert_eq!(
        value(&completion, "schema").unwrap().as_str(),
        Some("iroha.kagemusha.source-only-finality-completion.v1")
    );
    assert_eq!(
        fixed::<32>(&completion, "setup_sha256").unwrap(),
        selection.setup_sha256
    );
    assert_eq!(
        read_bounded(&source.root.join("setup.json"), 32 << 10).unwrap(),
        selected.setup.manifest
    );
    for (name, bytes) in &selected.setup.originals {
        assert_eq!(
            read_bounded(&source.root.join(name), ORIGINAL_MAX).unwrap(),
            *bytes
        );
    }
    assert_eq!(
        read_bounded(&source.root.join("fixture.json"), 1 << 20).unwrap(),
        selected.setup.originals["capture.json"]
    );
    let inventory = pinned(
        &source.root.join("completed-inventory.norito"),
        4096 * 2048 + 4096,
        fixed(&completion, "inventory_sha256").unwrap(),
    )
    .unwrap();
    assert_eq!(
        read_bounded(
            &source.root.join("originals/inventory.norito"),
            inventory.len()
        )
        .unwrap(),
        inventory
    );
    let provenance_frame = read_bounded(&source.root.join("provenance.norito"), 8192).unwrap();
    let provenance: SourceProvenance = canonical(&provenance_frame).unwrap();
    assert_eq!(
        provenance.source_manifest_sha256,
        fixed::<32>(&completion, "source_sha256").unwrap()
    );
    if !directory_exists(output).unwrap() {
        let mut directory = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            directory.mode(0o700);
        }
        directory.create(output).unwrap();
    }
    let _output_lock = RunLock::acquire(output).expect("exclusive fixed-input proof output");
    let identity = norito::json!({
        "schema": "iroha.kagemusha.executed-load-producer-selection.v1",
        "setup_sha256": (hex_out(&selection.setup_sha256)),
        "capture_sha256": (hex_out(&selection.capture_sha256)),
        "target_sha256": (hex_out(&selection.target_sha256)),
        "receipt_sha256": (hex_out(&selection.receipt_sha256)),
        "source_completion_sha256": (hex_out(&source.completion_sha256)),
        "producer_source_sha256": (hex_out(&source_sha256)),
        "producer_binary_sha256": (hex_out(&Sha256::digest(read_bounded(&std::env::current_exe().unwrap(), 256 << 20).unwrap()))),
    });
    retain(
        &output.join("selection.json"),
        norito::json::to_json(&identity).unwrap().as_bytes(),
    )
    .unwrap();
    retain(&output.join("capture.json"), &selected.capture).unwrap();
    retain(&output.join("target.json"), &selected.target).unwrap();
    retain(&output.join("source-complete.json"), &completion_frame).unwrap();
    for (folder, records) in [
        ("executed-originals", &selected.originals),
        ("target-originals", &selected.target_originals),
    ] {
        ensure_directory(&output.join(folder)).unwrap();
        for (name, bytes) in records {
            retain(&output.join(folder).join(name), bytes).unwrap();
        }
    }
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
        inner: StreamingCatalog::reopen(source.root.join("originals"), limits, WORKING_PK_BYTES)
            .unwrap(),
        writes: 0,
    };
    eprintln!("EXECUTED_LOAD_PHASE reconstruct_exact_source_graph retained_source=true blocks=4");
    let compilation = catalog::compile(
        selected.setup.anchor,
        &mut artifacts,
        Parameters {
            pallas: PinnedParams::derive(16).unwrap(),
            vesta: PinnedParams::derive(16).unwrap(),
        },
        limits,
        provenance,
    )
    .expect("unchanged selected complete graph and strict originals");
    assert_eq!(artifacts.inner.inventory().unwrap(), inventory);
    assert_eq!(
        compilation.terminal.descriptor,
        fixed::<32>(&completion, "terminal_descriptor").unwrap()
    );
    assert_eq!(
        compilation.terminal.key,
        fixed::<32>(&completion, "terminal_key").unwrap()
    );
    let digest = selected.receipt.receipt_digest().unwrap();
    let digest_field = Option::<Fp>::from(Fp::from_repr(digest)).unwrap();
    let evidence = prove_history(
        &compilation.graph,
        &mut artifacts,
        output,
        &selected.blocks,
        &selected.load,
        digest_field,
    );
    // `prove_history` verifies the exact terminal receipt and decides both curves.
    let finality = KagemushaWalletLoadFinalityV1 {
        version: 1,
        anchor_digest: selected.setup.anchor.digest().to_repr(),
        receipt_digest: digest,
        proof: evidence.proof,
        pallas_claim: evidence.pallas.to_bytes(),
        vesta_claim: evidence.vesta.to_bytes(),
    };
    let finality = finality.to_canonical_bytes().unwrap();
    let receipt = selected.receipt.to_canonical_bytes().unwrap();
    retain(&output.join("receipt.norito"), &receipt).unwrap();
    retain(&output.join("finality.norito"), &finality).unwrap();
    let result = norito::json!({"schema": "iroha.kagemusha.executed-load-finality-completion.v1",
        "selection_sha256": (hex_out(&Sha256::digest(norito::json::to_json(&identity).unwrap().as_bytes()))),
        "receipt_sha256": (hex_out(&Sha256::digest(&receipt))), "receipt_bytes": (receipt.len()),
        "finality_sha256": (hex_out(&Sha256::digest(&finality))), "finality_bytes": (finality.len()),
        "height": 5, "blocks_proved_after_genesis": 4,
        "scope": "Exact executed target receipt and complete finality proof; no wallet installation, payment, phone or network qualification."});
    retain(
        &output.join("complete.json"),
        norito::json::to_json(&result).unwrap().as_bytes(),
    )
    .unwrap();
    eprintln!(
        "EXECUTED_LOAD_COMPLETE exact_target=true contiguous_history=true both_claims_decided=true finality_bytes={}",
        finality.len()
    );
}
