//! Sign and strictly qualify a complete generated engineering catalog.
//!
//! No source is substituted and no deployment authority is claimed. The final
//! capability is created only by the production complete-source constructor.

use super::*;
use crate::kagemusha_wallet_artifacts_v1::InstalledVerifierPackV1;
use p256::ecdsa::{Signature, signature::Signer};

struct Combined<'a> {
    wallet: &'a mut Originals,
    metadata: &'a mut VerifierFiles,
    reads: usize,
}
impl OriginalSourceV1 for Combined<'_> {
    fn open(&mut self, digest: [u8; 32]) -> Result<Box<dyn Read + '_>, Error> {
        self.reads += 1;
        if self.metadata.files.contains_key(&digest) {
            VerifierBlobSource::open(self.metadata, &digest).map_err(|_| Error::Inventory)
        } else {
            self.wallet.open(digest)
        }
    }
}

fn signature(key: &SigningKey, message: &[u8]) -> KagemushaWalletSignerOutputV1<'static> {
    let signature: Signature = key.sign(message);
    KagemushaWalletSignerOutputV1::Raw(signature.to_bytes().into())
}

pub(super) fn accept(
    output: &Path,
    draft: WalletArtifactDraftV1,
    wallet: &mut Originals,
    metadata: &mut VerifierFiles,
    native: &SumeragiFinalityVerifier,
    config: ReadConfig,
) -> (InstalledVerifierPackV1, QualifiedWalletSourcesV1) {
    // Public deterministic engineering authority only. The production draft owns
    // every generated body and verifies both original signatures in finish().
    let root = SigningKey::from_bytes((&[0x11; 32]).into()).unwrap();
    let artifact = SigningKey::from_bytes((&[0x18; 32]).into()).unwrap();
    let certificate_body = KagemushaWalletSignerCertificateBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: draft.scheme().scheme_id(),
        role: KagemushaWalletSignerRoleV1::Artifact,
        key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            artifact.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap(),
        serial: 1,
    };
    let certificate = KagemushaWalletSignerCertificateV1::sign(
        certificate_body,
        draft.scheme(),
        signature(&root, &certificate_body.signing_message()),
    )
    .unwrap();
    let certificate_bytes = certificate.to_canonical_bytes().unwrap();
    let body = draft.manifest_body(&certificate_bytes).unwrap();
    let manifest = KagemushaWalletArtifactManifestV1::sign(
        body,
        &certificate,
        signature(&artifact, &body.signing_message()),
    )
    .unwrap();
    let installation = InstallationV1 {
        scheme_id: draft.scheme().scheme_id(),
        manifest_digest: manifest.manifest_digest(),
    };
    let complete = draft
        .finish(&certificate_bytes, &manifest.to_canonical_bytes().unwrap())
        .unwrap();
    // Retain signed engineering material before acceptance, without upgrading it.
    publish(
        &output.join("engineering-verifier-pack.norito"),
        complete.verifier_pack(),
    )
    .unwrap();
    publish(
        &output.join("engineering-installation.norito"),
        &norito::to_bytes(&(installation.scheme_id, installation.manifest_digest)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bounded_file(
            &output.join("producer-inventory.norito"),
            CATALOG_MAX_BYTES_V1
        )
        .unwrap(),
        complete.producer_inventory(),
    );
    qualify(
        output,
        complete.verifier_pack(),
        complete.producer_inventory(),
        installation,
        wallet,
        metadata,
        native,
        config,
    )
}

fn qualify(
    output: &Path,
    pack_bytes: &[u8],
    producer: &[u8],
    installation: InstallationV1,
    wallet: &mut Originals,
    metadata: &mut VerifierFiles,
    native: &SumeragiFinalityVerifier,
    config: ReadConfig,
) -> (InstalledVerifierPackV1, QualifiedWalletSourcesV1) {
    let inventory: ProducerInventoryV1 = norito::decode_canonical_with_limits(
        producer,
        norito::canonical_decode_limits(producer.len()),
    )
    .unwrap();
    inventory.validate().unwrap();
    assert_eq!(inventory.routes.len(), compiled_routes().len());
    let installed = InstalledVerifierPackV1::load(pack_bytes, installation).unwrap();
    let authenticated = installed.authenticate_producer_inventory(producer).unwrap();
    let mut originals = Combined {
        wallet,
        metadata,
        reads: 0,
    };
    let qualified = authenticated
        .qualify_wallet(
            &installed,
            native,
            &mut originals,
            config,
            Parameters {
                pallas: PinnedParams::derive(16).unwrap(),
                vesta: PinnedParams::derive(16).unwrap(),
            },
            VerifierLimits {
                maximum_artifacts: RECORDS,
                maximum_verifier_bytes: VERIFIER_BYTES,
                msm_budget: MemoryBudget::DEFAULT,
            },
        )
        .unwrap();
    assert_eq!(
        qualified.installation(),
        (installation.scheme_id, installation.manifest_digest)
    );
    assert_eq!(qualified.scope(), fixture_scope());
    assert_eq!(
        qualified.finality().anchor(),
        &crate::kagemusha_wallet_finality_v1::derive_history_anchor(native).unwrap()
    );
    for route in compiled_routes() {
        qualified.route(route).unwrap();
        qualified.q(route).unwrap();
    }
    publish(
        &output.join("engineering-source-acceptance.norito"),
        &norito::to_bytes(&(
            b"complete generated engineering source qualification; no deployment authority"
                .to_vec(),
            installation.scheme_id,
            installation.manifest_digest,
            u64::try_from(inventory.routes.len()).unwrap(),
            u64::try_from(originals.reads).unwrap(),
        ))
        .unwrap(),
    )
    .unwrap();
    eprintln!(
        "WALLET_SOURCE_ACCEPTED engineering=true deployment_authority=false complete_routes={} strict_original_reads={} scheme={} manifest={} genuine_receipt=false native_wallet_open=false",
        inventory.routes.len(),
        originals.reads,
        hex::encode(installation.scheme_id),
        hex::encode(installation.manifest_digest),
    );
    (installed, qualified)
}

#[test]
#[ignore = "strict complete52 engineering acceptance from independently pinned persisted originals; no key regeneration or deployment authority"]
fn qualify_signed_wallet_catalog_from_pinned_originals() {
    let snapshot = PathBuf::from(std::env::var_os("KAGEMUSHA_FINALITY_METADATA_SNAPSHOT").unwrap());
    let pins = Pins {
        producer: pin("KAGEMUSHA_FINALITY_PRODUCER_SHA256"),
        sources: pin("KAGEMUSHA_FINALITY_SOURCE_SHA256"),
        fixture: pin("KAGEMUSHA_FINALITY_FIXTURE_SHA256"),
        inventory: pin("KAGEMUSHA_FINALITY_INVENTORY_SHA256"),
    };
    let finality_records = records(&snapshot, pins).unwrap();
    let catalog = PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG").unwrap());
    regular_directory(&catalog).unwrap();
    let producer_pin = pin("KAGEMUSHA_WALLET_CATALOG_PRODUCER_SHA256");
    let source_pin = pin("KAGEMUSHA_WALLET_CATALOG_SOURCE_SHA256");
    let inventory_pin = pin("KAGEMUSHA_WALLET_CATALOG_INVENTORY_SHA256");
    let pack_pin = pin("KAGEMUSHA_WALLET_CATALOG_PACK_SHA256");
    let installation = InstallationV1 {
        scheme_id: pin("KAGEMUSHA_WALLET_SCHEME_ID"),
        manifest_digest: pin("KAGEMUSHA_WALLET_MANIFEST_DIGEST"),
    };
    let pack_bytes = pinned_file(
        &catalog.join("engineering-verifier-pack.norito"),
        CATALOG_MAX_BYTES_V1 + (64 << 10),
        pack_pin,
    )
    .unwrap();
    assert_eq!(
        bounded_file(&catalog.join("binary.sha256"), 32).unwrap(),
        producer_pin
    );
    assert_eq!(
        bounded_file(&catalog.join("source.sha256"), 32).unwrap(),
        source_pin
    );
    let bytes = pinned_file(
        &catalog.join("producer-inventory.norito"),
        CATALOG_MAX_BYTES_V1,
        inventory_pin,
    )
    .unwrap();
    let inventory: ProducerInventoryV1 =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .unwrap();
    inventory.validate().unwrap();
    assert_eq!(inventory.finality.originals, finality_records);
    let scope = fixture_scope();
    assert_eq!(
        bounded_file(&catalog.join("source-policy.norito"), 4096).unwrap(),
        norito::to_bytes(&(scope.provider(), scope.root().x, scope.root().y)).unwrap()
    );
    let output = PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_ACCEPTANCE_OUTPUT").unwrap());
    fs::create_dir(&output)
        .expect("fresh acceptance output, original compiler directory remains unchanged");
    publish(
        &output.join("signed-input-pins.norito"),
        &norito::to_bytes(&(
            producer_pin,
            source_pin,
            inventory_pin,
            pack_pin,
            installation.scheme_id,
            installation.manifest_digest,
            pins.producer,
            pins.sources,
            pins.fixture,
            pins.inventory,
        ))
        .unwrap(),
    )
    .unwrap();
    let mut wallet = Originals {
        root: catalog.join("originals"),
        bytes: 0,
        count: 0,
    };
    regular_directory(&wallet.root).unwrap();
    let mut metadata = VerifierFiles::new(&snapshot.join("originals"), &finality_records).unwrap();
    let config = ReadConfig {
        maximum_bytes: PROVING_KEY_MAX_BYTES_V1,
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::DEFAULT,
    };
    let (_installed, _qualified) = qualify(
        &output,
        &pack_bytes,
        &bytes,
        installation,
        &mut wallet,
        &mut metadata,
        &native_finality(),
        config,
    );
}
