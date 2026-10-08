//! Verifier-only original custody and real sealed-source identity regressions.

use super::*;
use crate::finality::catalog::builder::Assembler;
use iroha_plonk::{keys::pk::artifact::ReadConfig, pcs::ipa::PinnedParams};
use std::{fs, io::Cursor, path::PathBuf, sync::OnceLock};

#[derive(Clone, Default)]
struct Blobs {
    bytes: BTreeMap<[u8; 32], Vec<u8>>,
    opened: Vec<[u8; 32]>,
}
impl VerifierBlobSource for Blobs {
    fn open(&mut self, hash: &[u8; 32]) -> Result<Box<dyn Read + '_>, Error> {
        self.opened.push(*hash);
        let bytes = self.bytes.get(hash).ok_or(Error::Artifact)?;
        Ok(Box::new(Cursor::new(bytes)))
    }
}
fn params() -> Parameters {
    static PARAMS: OnceLock<Parameters> = OnceLock::new();
    PARAMS
        .get_or_init(|| Parameters {
            pallas: PinnedParams::derive(16).unwrap(),
            vesta: PinnedParams::derive(16).unwrap(),
        })
        .clone()
}
fn limits() -> VerifierLimits {
    VerifierLimits {
        maximum_artifacts: 16,
        maximum_verifier_bytes: 16 << 20,
        msm_budget: MemoryBudget::DEFAULT,
    }
}
fn anchor() -> HistoryAnchor {
    HistoryAnchor {
        network: [1; 32],
        instance: [2; 32],
        initial_context: [3; 32],
        initial_epoch: 0,
        parameters: [1000, 2000, 3000, 4000, 1 << 20, 100],
    }
}
fn sample_record() -> ArtifactRecord {
    ArtifactRecord {
        name: store::name(&ArtifactId::Source(NodeId::Genesis)).unwrap(),
        lengths: [1, 1, 1],
        sha256: [[1; 32], [2; 32], [3; 32]],
    }
}

#[test]
fn metadata_limits_and_completeness_fail_before_opening_storage() {
    let record = sample_record();
    let mut blobs = Blobs::default();
    let records = [record.clone()];
    let qualifier = Qualification::new(&records, &mut blobs, params(), limits()).unwrap();
    assert!(qualifier.complete().is_err());
    drop(qualifier);
    for change in 0..6 {
        let mut changed = records.to_vec();
        match change {
            0 => changed.clear(),
            1 => changed.push(record.clone()),
            2 => changed[0].name.push(0),
            3 => changed[0].lengths[0] = u64::MAX,
            4 => changed[0].sha256[1] = [0; 32],
            _ => changed[0].lengths[2] = (1 << 30) + 1,
        }
        assert!(Qualification::new(&changed, &mut blobs, params(), limits()).is_err());
    }
    for cap in [0, 1] {
        assert!(
            Qualification::new(
                &records,
                &mut blobs,
                params(),
                VerifierLimits {
                    maximum_verifier_bytes: cap,
                    ..limits()
                }
            )
            .is_err()
        );
    }
    assert!(
        Qualification::new(
            &records,
            &mut blobs,
            params(),
            VerifierLimits {
                maximum_artifacts: 65_537,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(blobs.opened.is_empty());
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "actual k16 source/PK imports plus independent VK-only derivation; optimized explicitly"]
fn actual_genesis_and_history_verifiers_match_strict_import_without_pk_reads() {
    let import_limits = ImportLimits {
        key: ReadConfig {
            maximum_bytes: 128 << 20,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_artifacts: 16,
        maximum_original_bytes: 1 << 30,
    };
    let directory =
        Temporary(std::env::temp_dir().join(format!("kg-verifier-only-{}", std::process::id())));
    let mut store = DirectoryCatalog::create(&directory.0, import_limits).unwrap();
    let mut compiler = Compiler {
        sink: &mut store,
        params: params(),
        limits: import_limits,
        cache: BTreeMap::new(),
        emitted: BTreeMap::new(),
        total: 0,
    };
    let layout = GenesisSourceCircuit::for_source(anchor());
    let imported = compiler.source(NodeId::Genesis, &layout).unwrap();
    drop(compiler);
    let inventory = store.inventory().unwrap();
    let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
        &inventory,
        norito::canonical_decode_limits(inventory.len()),
    )
    .unwrap();
    let mut blobs = Blobs::default();
    for id in [
        ArtifactId::Source(NodeId::Genesis),
        ArtifactId::Wrapper(NodeId::Genesis),
    ] {
        let original = store.load(&id).unwrap();
        for bytes in [original.descriptor, original.verifying_key] {
            blobs.bytes.insert(Sha256::digest(&bytes).into(), bytes);
        }
        // No PK bytes are installed in the verifier-only store.
    }
    let mut qualifier = Qualification::new(&records, &mut blobs, params(), limits()).unwrap();
    let verified = qualifier.source(NodeId::Genesis, &layout).unwrap();
    qualifier.complete().unwrap();
    assert_eq!(identity(&imported).unwrap(), identity(&verified).unwrap());
    assert_eq!(
        imported.verifying_key().to_bytes(),
        verified.verifying_key().to_bytes()
    );
    drop(qualifier);
    assert_eq!(blobs.opened.len(), 4);
    assert!(
        records
            .iter()
            .all(|record| !blobs.opened.contains(&record.sha256[2]))
    );

    // Even correctly framed receipt endpoints are not evidence. This negative
    // case intentionally uses a qualified Genesis source, never a receipt grant.
    let receipt_owner = ReceiptVerifier {
        anchor: anchor(),
        source: verified.clone(),
        history: verified.clone(),
        vesta: params().vesta,
    };
    // Neither decoded state nor a blank source proof constructs the opaque history.
    let state = crate::finality::history::HistoryState::genesis(&anchor());
    let bad = verified.blank_evidence().unwrap();
    assert!(
        receipt_owner
            .restore_history(&state, bad.clone(), MemoryBudget::DEFAULT, None)
            .is_err()
    );
    let mut malformed = state;
    malformed.next_height = 1;
    assert!(matches!(
        receipt_owner.restore_history(&malformed, bad.clone(), MemoryBudget::DEFAULT, None),
        Err(Error::Input)
    ));
    let cancel = iroha_pasta::CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        receipt_owner.restore_history(&state, bad, MemoryBudget::DEFAULT, Some(&cancel)),
        Err(Error::Cancelled)
    ));
    let digest = Fp::from(91);
    let context = iroha_pasta::poseidon::hash_with_domain(
        crate::finality::receipt_finality::CONTEXT_DOMAIN,
        &[anchor().digest(), digest],
    );
    let mut evidence = verified.blank_evidence().unwrap();
    evidence.endpoints = [
        Fp::from(crate::finality::receipt_finality::PROGRAM_ID),
        context,
        Fp::ZERO,
        Fp::ONE,
        Fp::ZERO,
        context,
    ];
    assert!(matches!(
        receipt_owner.verify_receipt_evidence(digest, &evidence, MemoryBudget::DEFAULT),
        Err(Error::Proof)
    ));
    for index in 0..6 {
        let mut changed = evidence.clone();
        changed.endpoints[index] += Fp::ONE;
        assert!(matches!(
            receipt_owner.verify_receipt_evidence(digest, &changed, MemoryBudget::DEFAULT),
            Err(Error::Input)
        ));
    }
    assert!(matches!(
        receipt_owner.verify_receipt_evidence(digest + Fp::ONE, &evidence, MemoryBudget::DEFAULT),
        Err(Error::Input)
    ));

    // Exercise exact reads using the actual derived identities without repeated key derivation.
    let source = DerivedSource::derive(&layout, &params().vesta, MemoryBudget::DEFAULT).unwrap();
    for index in 0..2 {
        let record = records
            .iter()
            .find(|r| r.name == store::name(&ArtifactId::Source(NodeId::Genesis)).unwrap())
            .unwrap();
        for mutation in 0..4 {
            let mut changed = blobs.clone();
            let bytes = changed.bytes.get_mut(&record.sha256[index]).unwrap();
            match mutation {
                0 => {
                    bytes[0] ^= 1;
                }
                1 => {
                    bytes.pop();
                }
                2 => bytes.push(0),
                _ => {
                    changed.bytes.remove(&record.sha256[index]);
                }
            }
            let mut qualifier =
                Qualification::new(&records, &mut changed, params(), limits()).unwrap();
            assert!(
                qualifier
                    .check(
                        &ArtifactId::Source(NodeId::Genesis),
                        source.binding(),
                        source.key()
                    )
                    .is_err()
            );
        }
    }
    let mut changed = records.clone();
    changed[0].sha256[0][0] ^= 1;
    let mut fresh = blobs.clone();
    fresh.opened.clear();
    let mut qualifier = Qualification::new(&changed, &mut fresh, params(), limits()).unwrap();
    assert!(qualifier.source(NodeId::Genesis, &layout).is_err());
    drop(qualifier);
    assert!(fresh.opened.is_empty());

    let mut foreign = anchor();
    foreign.initial_epoch += 1;
    let mut qualifier = Qualification::new(&records, &mut blobs, params(), limits()).unwrap();
    assert!(
        qualifier
            .source(NodeId::Genesis, &GenesisSourceCircuit::for_source(foreign))
            .is_err()
    );
    drop(qualifier);
    // The finite two-source history wrapper must match the source-qualified PK
    // importer too; no receipt or complete graph is claimed by this body fixture.
    let mut compiler = Compiler {
        sink: &mut store,
        params: params(),
        limits: import_limits,
        cache: BTreeMap::new(),
        emitted: BTreeMap::new(),
        total: 0,
    };
    let imported_history = Assembler::history(&mut compiler, anchor(), imported.clone()).unwrap();
    drop(compiler);
    let inventory = store.inventory().unwrap();
    let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
        &inventory,
        norito::canonical_decode_limits(inventory.len()),
    )
    .unwrap();
    for id in [
        ArtifactId::Source(NodeId::Genesis),
        ArtifactId::Source(NodeId::Append),
        ArtifactId::HistoryWrapper,
    ] {
        let original = store.load(&id).unwrap();
        for bytes in [original.descriptor, original.verifying_key] {
            blobs.bytes.insert(Sha256::digest(&bytes).into(), bytes);
        }
    }
    let mut qualifier = Qualification::new(&records, &mut blobs, params(), limits()).unwrap();
    qualifier.recipe_limits = Some(import_limits);
    let body = qualifier.source(NodeId::Genesis, &layout).unwrap();
    let history = qualifier.history(anchor(), body).unwrap();
    qualifier.complete().unwrap();
    assert_eq!(
        identity(&history).unwrap(),
        identity(&imported_history).unwrap()
    );
    assert_eq!(
        history.layout_metadata().1.to_bytes(),
        imported_history.layout_metadata().1.to_bytes()
    );
    // Recipes retain compiled source shapes and exact qualified children only.
    // Rebuilding them must recover byte-identical PKs on both curves, including
    // the shared two-source history wrapper; these partial fixtures grant no graph.
    for id in [
        ArtifactId::Source(NodeId::Genesis),
        ArtifactId::Source(NodeId::Append),
        ArtifactId::HistoryWrapper,
    ] {
        let (_, recipe) = qualifier.recipes.get(&store::name(&id).unwrap()).unwrap();
        let restored = recipe.regenerate_cancellable(None).unwrap();
        let original = store.load(&id).unwrap();
        assert_eq!(restored.descriptor, original.descriptor);
        assert_eq!(restored.verifying_key, original.verifying_key);
        assert_eq!(restored.proving_key, original.proving_key);
    }
    drop(qualifier);
    assert!(
        records
            .iter()
            .all(|record| !blobs.opened.contains(&record.sha256[2]))
    );
    println!(
        "VERIFIER_ONLY_GENESIS_HISTORY exact_sources_and_wrappers=true PK_reads=0 mutations=true complete_receipt_graph=false"
    );
}

#[test]
fn cancelled_qualification_never_opens_originals_or_publishes_recipes() {
    let token = iroha_pasta::CancellationToken::new();
    token.cancel();
    let mut blobs = Blobs::default();
    let records = [sample_record()];
    let error = qualify_receipt_cancellable(
        anchor(),
        &records,
        &mut blobs,
        params(),
        limits(),
        Some(&token),
    )
    .err()
    .unwrap();
    assert!(error.is_cancelled());
    assert!(blobs.opened.is_empty());
    let mut qualifier = Qualification::new(&records, &mut blobs, params(), limits()).unwrap();
    qualifier.recipe_limits = Some(ImportLimits {
        key: ReadConfig {
            maximum_bytes: 128 << 20,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
        maximum_artifacts: 16,
        maximum_original_bytes: 1 << 30,
    });
    qualifier.consumed.insert(records[0].name.clone());
    assert!(qualifier.complete().is_err());
    assert!(qualifier.recipes.is_empty());
}
