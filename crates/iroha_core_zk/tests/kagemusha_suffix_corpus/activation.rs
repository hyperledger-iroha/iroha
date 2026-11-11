//! Recover the existing engineering merchant's Activate through Native, without reproving.
//! The original run used a deterministic simulated payment key, not a physical phone.
use super::*;
use iroha_core_zk::{
    kagemusha_wallet_advance_v1::{
        KagemushaWalletAnchorPolicyV1, KagemushaWalletKeyGenerationRequestV1,
        KagemushaWalletKeyGenerationV1, KagemushaWalletPlatformSignatureV1,
        KagemushaWalletPlatformV1, KagemushaWalletProbeV1, KagemushaWalletProviderOptionsV1,
        KagemushaWalletRemoveOutcomeV1, KagemushaWalletSignMessageV1, KagemushaWalletSlotIdV1,
        KagemushaWalletStdFsV1, KagemushaWalletUnavailableV1,
    },
    kagemusha_wallet_artifacts_v1::producer_inventory::{
        OriginalSourceV1, PROVING_KEY_MAX_BYTES_V1,
    },
    kagemusha_wallet_proofs_v1::Error as ProofError,
    kagemusha_wallet_state_v1::{NativeProviderV1, NativeWalletRuntimeV1},
};
use iroha_crypto::{Algorithm, KeyPair, Signature as AccountSignature};
use iroha_data_model::{
    account::AccountId,
    block::decode_framed_signed_block,
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier, authenticated_genesis},
};
use iroha_plonk::keys::{CosetCachePolicy, pk::artifact::ReadConfig};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Rehydrate the *same* public engineering fixture key for its existing slot. No key
/// generation/enrollment API is called, and no unrelated slot or signing domain is served.
#[derive(Clone)]
struct ExistingMerchantKey {
    slot: KagemushaWalletSlotIdV1,
    key: SigningKey,
    public: KagemushaDevicePublicKeyV1,
    signs: Arc<AtomicUsize>,
}
impl KagemushaWalletPlatformV1 for ExistingMerchantKey {
    fn key_enumerate(&self) -> Result<Vec<KagemushaWalletSlotIdV1>, KagemushaWalletUnavailableV1> {
        Ok(vec![self.slot])
    }
    fn key_probe(
        &self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> KagemushaWalletProbeV1<KagemushaDevicePublicKeyV1> {
        if slot == &self.slot {
            KagemushaWalletProbeV1::Present(self.public)
        } else {
            KagemushaWalletProbeV1::Unavailable(KagemushaWalletUnavailableV1::KeyUnusable)
        }
    }
    fn key_generate(
        &self,
        _: &KagemushaWalletSlotIdV1,
        _: &KagemushaWalletKeyGenerationRequestV1,
    ) -> KagemushaWalletKeyGenerationV1 {
        panic!("retained merchant activation must never generate a payment key")
    }
    fn key_sign(
        &self,
        slot: &KagemushaWalletSlotIdV1,
        message: KagemushaWalletSignMessageV1<'_>,
    ) -> Result<KagemushaWalletPlatformSignatureV1, KagemushaWalletUnavailableV1> {
        assert_eq!(slot, &self.slot);
        assert_eq!(
            message.domain(),
            KagemushaWalletSigningDomainV1::LedgerControl,
            "no new receipt, enrollment or monetary signature"
        );
        self.signs.fetch_add(1, Ordering::SeqCst);
        let signature: Signature = self.key.sign(message.as_bytes());
        Ok(KagemushaWalletPlatformSignatureV1::Der(
            signature.to_der().as_bytes().to_vec(),
        ))
    }
    fn key_delete(&self, _: &KagemushaWalletSlotIdV1) -> KagemushaWalletRemoveOutcomeV1 {
        panic!("retained merchant activation must never delete a payment key")
    }
    fn anchor_policy(&self) -> KagemushaWalletAnchorPolicyV1 {
        KagemushaWalletAnchorPolicyV1::NotRequired
    }
    fn storage_state(&self) -> Result<(), KagemushaWalletUnavailableV1> {
        Ok(())
    }
    fn boot_id(&self) -> Result<[u8; 32], KagemushaWalletUnavailableV1> {
        Ok([0xb2; 32])
    }
    fn monotonic_ms(&self) -> Result<u64, KagemushaWalletUnavailableV1> {
        Ok(1_000)
    }
}

struct Originals(PathBuf);
impl OriginalSourceV1 for Originals {
    fn open(&mut self, digest: [u8; 32]) -> Result<Box<dyn std::io::Read + '_>, ProofError> {
        let path = self.0.join(hex::encode(digest));
        let metadata = fs::symlink_metadata(&path).map_err(|_| ProofError::Unavailable)?;
        if !metadata.file_type().is_file() || metadata.len() > PROVING_KEY_MAX_BYTES_V1 as u64 {
            return Err(ProofError::Inventory);
        }
        Ok(Box::new(
            fs::File::open(path)
                .map_err(|_| ProofError::Unavailable)?
                .take(PROVING_KEY_MAX_BYTES_V1 as u64 + 1),
        ))
    }
}
fn read_config() -> ReadConfig {
    ReadConfig {
        maximum_bytes: PROVING_KEY_MAX_BYTES_V1,
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::new(128 << 20),
    }
}
fn private_dir(path: &Path) {
    fs::create_dir(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}
fn hashes(root: &Path) -> BTreeMap<PathBuf, (u64, [u8; 32])> {
    fn visit(
        root: &Path,
        path: &Path,
        total: &mut u64,
        output: &mut BTreeMap<PathBuf, (u64, [u8; 32])>,
    ) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "no symlink custody input"
            );
            if metadata.is_dir() {
                visit(root, &path, total, output);
                continue;
            }
            assert!(metadata.is_file());
            *total = total.checked_add(metadata.len()).unwrap();
            assert!(*total <= 2 << 30, "bounded retained merchant data");
            // The retained hundred-payer custody contains 222,254 files (121,752,257
            // payload bytes). Keep a finite manifest bound above that measured inventory.
            assert!(output.len() < 262_144);
            let mut digest = Sha256::new();
            let mut file = fs::File::open(&path).unwrap();
            let mut buffer = vec![0_u8; 65_536];
            loop {
                let length = file.read(&mut buffer).unwrap();
                if length == 0 {
                    break;
                }
                digest.update(&buffer[..length]);
            }
            output.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                (metadata.len(), digest.finalize().into()),
            );
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut 0, &mut result);
    result
}
fn copy_custody(source: &Path, target: &Path) {
    private_dir(target);
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source).unwrap();
        if metadata.is_dir() {
            copy_custody(&source, &target);
        } else {
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            fs::copy(source, target).unwrap();
        }
    }
}

#[test]
#[ignore = "requires retained hundred-payer engineering custody; emits real Native activation only"]
fn retained_merchant_activation_preserves_every_original_and_reopens_exactly() {
    let corpus = Corpus::load();
    let output = PathBuf::from(
        std::env::var_os("KAGEMUSHA_SUFFIX_ACTIVATION_OUTPUT")
            .expect("new private output directory"),
    );
    let parent = output.parent().unwrap().canonicalize().unwrap();
    assert!(!parent.starts_with(&corpus.root));
    assert!(!output.exists(), "activation output must be new");
    private_dir(&output);
    let original = corpus.root.join("run-hundred/wallet-000/custody");
    let original_hashes = hashes(&original);
    let copied = output.join("custody");
    copy_custody(&original, &copied);
    assert_eq!(
        hashes(&copied),
        original_hashes,
        "complete exact data copy before any Native open"
    );
    let before =
        original_hashes
            .iter()
            .fold(String::new(), |mut listing, (name, (bytes, hash))| {
                use std::fmt::Write as _;
                writeln!(listing, "{} {bytes} {}", hex::encode(hash), name.display())
                    .expect("writing to a String cannot fail");
                listing
            });
    fs::write(output.join("source.sha256"), before).unwrap();

    let credential = corpus.credential;
    let certificates = corpus.certificates();
    let account_bytes = bounded(&corpus.root.join("setup/account-b.norito"), 4_096);
    let account: AccountId = decode(&account_bytes).unwrap();
    assert_eq!(
        kagemusha_wallet_account_digest_v1(&account).unwrap(),
        credential.body.account_digest
    );
    let account_key = KeyPair::from_seed(vec![42; 32], Algorithm::Ed25519);
    assert_eq!(AccountId::new(account_key.public_key().clone()), account);
    let asset_bytes = bounded(&corpus.root.join("setup/asset.norito"), 4_096);
    let asset: KagemushaWalletAssetScopeV1 = decode(&asset_bytes).unwrap();
    assert_eq!(asset.asset_digest(), credential.body.asset_digest);
    let bootstrap_completion = completion(&corpus.slot, &corpus.frozen[&0]);
    let bootstrap: KagemushaWalletPackageV1 = decode(&bootstrap_completion.output).unwrap();
    let key = SigningKey::from_slice(&[250; 32]).unwrap();
    let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_sec1_point(false).as_bytes(),
    )
    .unwrap();
    assert_eq!(
        public, credential.body.payment_key,
        "same retained engineering key, no replacement"
    );
    let slot = KagemushaWalletSlotIdV1(hex::decode(SLOT).unwrap().try_into().unwrap());
    let platform = ExistingMerchantKey {
        slot,
        key,
        public,
        signs: Arc::new(AtomicUsize::new(0)),
    };
    let genesis = decode_framed_signed_block(&bounded(
        &corpus.root.join("setup/signed-genesis.wire"),
        16 << 20,
    ))
    .unwrap();
    let epoch = authenticated_genesis(&genesis).unwrap().into_parts().0;
    let validators = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let finality = Arc::new(
        SumeragiFinalityVerifier::new(
            &genesis,
            "kagemusha-native-monetary-acceptance-v1",
            validators,
        )
        .unwrap(),
    );
    let (sources, _) = continuation::qualify(&corpus);
    let sources = Arc::new(sources);
    let catalog = corpus.root.join("catalog/originals");
    let installed = Arc::new(corpus.installed);
    let frames = [
        credential.to_canonical_bytes().unwrap(),
        norito::encode_canonical(&certificates).unwrap(),
        account_bytes,
        asset_bytes,
    ];
    let open = || {
        let provider = NativeProviderV1::open(
            KagemushaWalletStdFsV1::open(&copied).unwrap(),
            platform.clone(),
            credential.body.scheme_id,
            KagemushaWalletProviderOptionsV1 {
                ballast_bytes: 2_048,
            },
        )
        .unwrap();
        let runtime = NativeWalletRuntimeV1::new(
            provider,
            Arc::clone(&installed),
            Arc::clone(&sources),
            Arc::clone(&finality),
            Originals(catalog.clone()),
            read_config(),
            MemoryBudget::new(128 << 20),
        );
        let pending = runtime
            .begin(&frames[0], &frames[1], &frames[2], &frames[3])
            .unwrap();
        let signature =
            AccountSignature::try_new(account_key.private_key(), pending.challenge()).unwrap();
        pending.finish(signature.payload()).unwrap()
    };
    let mut wallet = open();
    let snapshot = wallet.snapshot().unwrap();
    assert_eq!(
        snapshot.sequence, 101,
        "this retained owner already made its onward Send; not an unfinished head100"
    );
    let bytes = wallet
        .activation()
        .expect("actual Native authenticates Bootstrap and signs retained Activate");
    let activation =
        KagemushaWalletActivationV1::decode_canonical(&bytes, &credential.body.scheme_id).unwrap();
    assert_eq!(activation.credential, credential);
    assert_eq!(activation.asset, asset);
    assert_eq!(
        activation.bootstrap, bootstrap,
        "exact original Bootstrap proof/receipt/output"
    );
    activation.verify(installed.verifier().scheme()).unwrap();
    assert_eq!(platform.signs.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.activation().unwrap(), bytes);
    assert_eq!(wallet.snapshot().unwrap(), snapshot);
    drop(wallet);
    let mut wallet = open();
    assert_eq!(
        wallet.activation().unwrap(),
        bytes,
        "reopen reads exact retained activation; never re-signs"
    );
    assert_eq!(wallet.snapshot().unwrap(), snapshot);
    assert_eq!(platform.signs.load(Ordering::SeqCst), 1);
    drop(wallet);
    assert_eq!(
        hashes(&original),
        original_hashes,
        "all original custody/proof files remain byte-identical"
    );
    fs::write(output.join("merchant-activation.norito"), &bytes).unwrap();
    eprintln!(
        "SUFFIX_ACTIVATION bytes={} sha256={} source_sequence={} payment_signatures=1 generated_keys=0 deleted_keys=0 new_receipts=0 proof_generation=0 original_files={} engineering_simulated_key=true",
        bytes.len(),
        hex::encode(Sha256::digest(&bytes)),
        snapshot.sequence,
        original_hashes.len()
    );
}
