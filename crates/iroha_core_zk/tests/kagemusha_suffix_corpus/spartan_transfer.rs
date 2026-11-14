//! Exercise the genuine installed-owner transfer, without touching retained custody.
//!
//! The real experimental programs are authorized by the engineering installation's
//! root for this installation-only test. Re-scoping that signed manifest does not
//! claim that its monetary circuits match the legacy corpus or migrate funded heads.
use super::*;
use ff::PrimeField;
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
use iroha_data_model::{
    block::decode_framed_signed_block,
    sumeragi_finality::{
        FinalityValidator, SumeragiCommitVerifierV1, SumeragiFinalityVerifier,
        authenticated_genesis,
    },
};
use iroha_pasta::{Fp, poseidon};
use iroha_plonk::keys::{CosetCachePolicy, pk::artifact::ReadConfig};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use std::{
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use zk_spartan::native::{Error as SpartanError, ParameterManifest};

/// Deliberately non-Clone: ownership transfer must preserve this exact object.
struct EmptyPlatform(Arc<AtomicUsize>);
impl Drop for EmptyPlatform {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl KagemushaWalletPlatformV1 for EmptyPlatform {
    fn key_enumerate(&self) -> Result<Vec<KagemushaWalletSlotIdV1>, KagemushaWalletUnavailableV1> {
        Ok(Vec::new())
    }
    fn key_probe(
        &self,
        _: &KagemushaWalletSlotIdV1,
    ) -> KagemushaWalletProbeV1<KagemushaDevicePublicKeyV1> {
        panic!("installation transfer cannot probe a payment key")
    }
    fn key_generate(
        &self,
        _: &KagemushaWalletSlotIdV1,
        _: &KagemushaWalletKeyGenerationRequestV1,
    ) -> KagemushaWalletKeyGenerationV1 {
        panic!("installation transfer cannot generate a payment key")
    }
    fn key_sign(
        &self,
        _: &KagemushaWalletSlotIdV1,
        _: KagemushaWalletSignMessageV1<'_>,
    ) -> Result<KagemushaWalletPlatformSignatureV1, KagemushaWalletUnavailableV1> {
        panic!("installation transfer cannot sign")
    }
    fn key_delete(&self, _: &KagemushaWalletSlotIdV1) -> KagemushaWalletRemoveOutcomeV1 {
        panic!("installation transfer cannot delete a payment key")
    }
    fn anchor_policy(&self) -> KagemushaWalletAnchorPolicyV1 {
        KagemushaWalletAnchorPolicyV1::NotRequired
    }
    fn storage_state(&self) -> Result<(), KagemushaWalletUnavailableV1> {
        Ok(())
    }
    fn boot_id(&self) -> Result<[u8; 32], KagemushaWalletUnavailableV1> {
        Ok([0x67; 32])
    }
    fn monotonic_ms(&self) -> Result<u64, KagemushaWalletUnavailableV1> {
        Ok(1_000)
    }
}

/// Wrap the same real content-addressed source used by catalog qualification.
struct Originals {
    inner: continuation::FixtureOriginals,
    drops: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
}
impl OriginalSourceV1 for Originals {
    fn open(&mut self, digest: [u8; 32]) -> Result<Box<dyn std::io::Read + '_>, ProofError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.open(digest)
    }
}
impl Drop for Originals {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
type Runtime = NativeWalletRuntimeV1<KagemushaWalletStdFsV1, EmptyPlatform, Originals>;

fn manifest_bytes(value: &ParameterManifest) -> Vec<u8> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    norito::to_bytes(value).unwrap()
}

fn manifest_from_bytes(bytes: &[u8]) -> ParameterManifest {
    assert_eq!(
        norito::core::from_bytes_view(bytes).unwrap().flags(),
        norito::core::V1_LAYOUT_FLAGS
    );
    let value =
        norito::decode_from_bytes_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .unwrap();
    assert_eq!(
        manifest_bytes(&value),
        bytes,
        "exact experimental manifest layout"
    );
    value
}

fn manifest_message(bytes: &[u8]) -> [u8; 32] {
    // Independently encode the specified length-delimited 31-byte Poseidon words.
    let values: Vec<_> = std::iter::once(Fp::from(bytes.len() as u64))
        .chain(bytes.chunks(31).map(|chunk| {
            let mut word = [0; 32];
            word[..chunk.len()].copy_from_slice(chunk);
            Option::<Fp>::from(Fp::from_repr(word)).unwrap()
        }))
        .collect();
    poseidon::hash_with_domain(u64::from_le_bytes(*b"kgwparm1"), &values).to_repr()
}

fn sign(bytes: &[u8], key: &SigningKey) -> [u8; 64] {
    // The canonical P-256 adapter hashes these 32 little-endian field bytes.
    let signature: Signature = key.sign(&manifest_message(bytes));
    signature.normalize_s().to_bytes().into()
}

#[test]
#[ignore = "requires the saved experimental parameter manifest and signature"]
fn experimental_manifest_codec_and_signer_preflight() {
    use p256::ecdsa::signature::Verifier as _;
    let directory = std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../experiments/kagemusha-spartan/output/full-backend-chain/native-device")
        });
    let bytes = bounded(&directory.join("parameter-manifest.norito"), 16 << 10);
    let parsed = manifest_from_bytes(&bytes);
    assert_eq!(manifest_bytes(&parsed), bytes);
    let key = SigningKey::from_bytes((&[0x11; 32]).into()).unwrap();
    let original =
        Signature::from_slice(&bounded(&directory.join("parameter-signature.bin"), 64)).unwrap();
    key.verifying_key()
        .verify(&manifest_message(&bytes), &original)
        .expect("independent signer/message agrees with the actual saved root signature");
    let fresh = Signature::from_slice(&sign(&bytes, &key)).unwrap();
    key.verifying_key()
        .verify(&manifest_message(&bytes), &fresh)
        .unwrap();
    let wrong = SigningKey::from_bytes((&[0x12; 32]).into()).unwrap();
    let wrong = Signature::from_slice(&sign(&bytes, &wrong)).unwrap();
    assert!(
        key.verifying_key()
            .verify(&manifest_message(&bytes), &wrong)
            .is_err()
    );
}

fn refused(
    runtime: Runtime,
    bundle: &[u8],
    manifest: &[u8],
    signature: [u8; 64],
    expected: impl FnOnce(&SpartanError) -> bool,
) -> Runtime {
    match runtime.into_spartan(bundle, manifest, signature) {
        Ok(_) => panic!("invalid installed transfer was accepted"),
        Err((same, error)) => {
            assert!(expected(&error), "wrong refusal: {error:?}");
            same
        }
    }
}

#[test]
#[ignore = "requires genuine installed suffix catalog and exact full experimental public programs"]
fn installed_spartan_transfer_preserves_owner_on_refusal_and_moves_it_once() {
    let corpus = Corpus::load();
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
    let reader = SumeragiCommitVerifierV1::new(&finality).unwrap();
    let scheme = corpus.installed.verifier().scheme();
    let root = SigningKey::from_bytes((&[0x11; 32]).into()).unwrap();
    assert_eq!(
        scheme.scheme_root_key.as_ref(),
        root.verifying_key().to_sec1_point(false).as_bytes(),
        "the public engineering signer must equal the independently installed root"
    );

    let artifacts = std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../experiments/kagemusha-spartan/output/full-backend-chain/native-device")
        });
    let bundle = bounded(&artifacts.join("program-bundle.norito"), 256 << 20);
    let original_manifest = bounded(&artifacts.join("parameter-manifest.norito"), 16 << 10);
    let bundle_hash: [u8; 32] = Sha256::digest(&bundle).into();
    let mut manifest = manifest_from_bytes(&original_manifest);
    // Root-authorized installation scope only. All eight exact program pins remain unchanged.
    manifest.scheme = scheme.scheme_id();
    manifest.finality_network = reader.network().to_string();
    manifest.finality_chain = reader.chain_id().to_owned();
    let bytes = manifest_bytes(&manifest);
    let signature = sign(&bytes, &root);
    // Catch fixture framing/scope mistakes before the deliberately genuine,
    // comparatively expensive legacy source qualification.
    {
        let parameters = zk_spartan::native::AuthenticatedParameters::install(
            &bundle, &bytes, signature, scheme, reader,
        )
        .unwrap();
        assert_eq!(parameters.id(), &manifest.relation);
    }
    let (qualified, originals) = continuation::qualify(&corpus);
    let installed = Arc::new(corpus.installed);
    let platform_drops = Arc::new(AtomicUsize::new(0));
    let original_drops = Arc::new(AtomicUsize::new(0));
    let original_reads = Arc::new(AtomicUsize::new(0));
    let temporary = tempfile::tempdir().unwrap();
    let custody = temporary.path().join("fresh-custody");
    fs::create_dir(&custody).unwrap();
    fs::set_permissions(&custody, fs::Permissions::from_mode(0o700)).unwrap();
    let provider = NativeProviderV1::open(
        KagemushaWalletStdFsV1::open(&custody).unwrap(),
        EmptyPlatform(Arc::clone(&platform_drops)),
        manifest.scheme,
        KagemushaWalletProviderOptionsV1 {
            ballast_bytes: 2_048,
        },
    )
    .unwrap();
    let sentinel = fs::read(custody.join("root.norito")).unwrap();
    let inode = fs::metadata(custody.join("lock")).unwrap().ino();
    let assert_retained = || {
        assert_eq!(platform_drops.load(Ordering::SeqCst), 0);
        assert_eq!(original_drops.load(Ordering::SeqCst), 0);
        assert_eq!(
            original_reads.load(Ordering::SeqCst),
            0,
            "transfer must not replace or reopen the qualified original source"
        );
        assert_eq!(fs::read(custody.join("root.norito")).unwrap(), sentinel);
        assert_eq!(fs::metadata(custody.join("lock")).unwrap().ino(), inode);
        let competing = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(custody.join("lock"))
            .unwrap();
        assert_eq!(
            rustix::fs::flock(
                &competing,
                rustix::fs::FlockOperation::NonBlockingLockExclusive
            ),
            Err(rustix::io::Errno::WOULDBLOCK),
            "same exclusive kernel custody remains held"
        );
    };
    let runtime = NativeWalletRuntimeV1::new(
        provider,
        Arc::clone(&installed),
        Arc::new(qualified),
        finality,
        Originals {
            inner: originals,
            drops: Arc::clone(&original_drops),
            reads: Arc::clone(&original_reads),
        },
        ReadConfig {
            maximum_bytes: PROVING_KEY_MAX_BYTES_V1,
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::new(128 << 20),
        },
        MemoryBudget::new(128 << 20),
    );
    assert_retained();
    let wrong_root = SigningKey::from_bytes((&[0x12; 32]).into()).unwrap();
    let mut runtime = refused(
        runtime,
        &bundle,
        &bytes,
        sign(&bytes, &wrong_root),
        |e| matches!(e, SpartanError::Rejected(message) if message == "P-256 signature"),
    );
    assert_retained();
    for case in 0..3 {
        let mut changed = manifest_from_bytes(&bytes);
        let expected = match case {
            0 => {
                changed.scheme[0] ^= 1;
                "installed native scheme"
            }
            1 => {
                changed.finality_chain.push_str("-foreign");
                "parameter ledger root"
            }
            _ => {
                changed.primary[0][0] ^= 1;
                "exact compiled monetary program"
            }
        };
        let changed = manifest_bytes(&changed);
        runtime = refused(
            runtime,
            &bundle,
            &changed,
            sign(&changed, &root),
            |e| matches!(e, SpartanError::AuthorityMismatch(message) if *message == expected),
        );
        assert_retained();
    }
    let mut altered_bundle = bundle.clone();
    altered_bundle.push(0);
    runtime = refused(runtime, &altered_bundle, &bytes, signature, |e| {
        matches!(e, SpartanError::Malformed(_))
    });
    drop(altered_bundle);
    assert_retained();
    let future = match runtime.into_spartan(&bundle, &bytes, signature) {
        Ok(future) => future,
        Err((_, error)) => panic!("genuine installed transfer refused: {error:?}"),
    };
    assert_eq!(future.parameter_id(), &manifest.relation);
    assert_retained();
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&bundle)), bundle_hash);
    assert_eq!(
        bounded(&artifacts.join("parameter-manifest.norito"), 16 << 10),
        original_manifest,
        "re-scoping exists only in memory, never rewrites retained originals"
    );
    drop(future);
    assert_eq!(platform_drops.load(Ordering::SeqCst), 1);
    assert_eq!(original_drops.load(Ordering::SeqCst), 1);
    let released = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(custody.join("lock"))
        .unwrap();
    rustix::fs::flock(
        &released,
        rustix::fs::FlockOperation::NonBlockingLockExclusive,
    )
    .expect("only dropping the transferred owner releases exclusive custody");
    assert_eq!(fs::metadata(custody.join("lock")).unwrap().ino(), inode);
    assert_eq!(fs::read(custody.join("root.norito")).unwrap(), sentinel);
    eprintln!(
        "installed Spartan transfer: real installation/catalog/genesis; five exact refusals; same owner and lock; successful move and single drop"
    );
}
