// Shared genuine builder items. Registered tests remain in the integration harness.
use ff::Field;
use iroha_kagemusha_proof::{
    a_relation::native::{
        artifact::KeyArtifact,
        load::{A_STAGE_COUNT, Inputs, Plan, Prover},
    },
    admin_sigma::StateWitness,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, msm::MemoryBudget};
use iroha_plonk::{DescriptorBinding, ProverConfig, VerifyingKey, keys::pk::artifact::ReadConfig};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig, FoldInput};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Exact original serialized source artifact; native import checks its complete layout.
pub struct Original {
    /// Canonical source descriptor.
    pub descriptor: Vec<u8>,
    /// Original verifying key.
    pub verifying_key: Vec<u8>,
    path: PathBuf,
    length: usize,
    sha256: [u8; 32],
}
impl Original {
    /// Persist a driver-owned regenerable original and retain only its exact identity.
    /// Existing bytes must match; a live imported key is never kept by this handle.
    pub fn persist(
        path: &Path,
        descriptor: Vec<u8>,
        verifying_key: Vec<u8>,
        bytes: Vec<u8>,
    ) -> Self {
        assert!(!bytes.is_empty() && bytes.len() <= 256 << 20);
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                assert!(metadata.file_type().is_file());
                assert_eq!(metadata.len(), u64::try_from(bytes.len()).unwrap());
                assert_eq!(
                    Sha256::digest(fs::read(path).unwrap()),
                    Sha256::digest(&bytes)
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let pending = path.with_extension("pending");
                match fs::symlink_metadata(&pending) {
                    Ok(metadata) => {
                        assert!(metadata.file_type().is_file());
                        fs::remove_file(&pending).unwrap();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => panic!("unavailable original staging: {error}"),
                }
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&pending)
                    .unwrap();
                file.write_all(&bytes).unwrap();
                file.sync_all().unwrap();
                fs::hard_link(&pending, path).unwrap();
                fs::remove_file(pending).unwrap();
                File::open(path.parent().unwrap())
                    .unwrap()
                    .sync_all()
                    .unwrap();
            }
            Err(error) => panic!("unavailable original artifact: {error}"),
        }
        Self {
            path: path.into(),
            length: bytes.len(),
            sha256: Sha256::digest(bytes).into(),
            descriptor,
            verifying_key,
        }
    }
    fn read(&self, maximum: usize) -> Vec<u8> {
        assert!(self.length <= maximum);
        assert!(
            fs::symlink_metadata(&self.path)
                .unwrap()
                .file_type()
                .is_file()
        );
        let file = File::open(&self.path).unwrap();
        assert_eq!(
            file.metadata().unwrap().len(),
            u64::try_from(self.length).unwrap()
        );
        let mut bytes = Vec::with_capacity(self.length);
        file.take(u64::try_from(self.length).unwrap() + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes.len(), self.length);
        assert_eq!(<[u8; 32]>::from(Sha256::digest(&bytes)), self.sha256);
        bytes
    }
    fn metadata<C: iroha_pasta::PastaCurve>(&self) -> KeyArtifact<C> {
        let binding = DescriptorBinding::decode_v2(&self.descriptor).unwrap();
        let key = VerifyingKey::read(&self.verifying_key, &binding).unwrap();
        KeyArtifact::new(binding, key).unwrap()
    }
}
/// Required independently installed source plan and finalized originals.
/// The native producer verifies every original proof and both carried claims.
pub struct InstalledLoad {
    /// Installed source plan with fixed genesis anchor and finality source catalog.
    pub plan: Plan,
    /// All five actual A source artifacts.
    pub a: [Original; A_STAGE_COUNT],
    /// All four mandatory W source artifacts.
    pub w: [Original; A_STAGE_COUNT - 1],
    /// Exact receipt, genuine finality source and predecessor/Q originals.
    pub inputs: Inputs,
    /// Finite original-artifact reader bounds.
    pub read: ReadConfig,
}
/// A real fixture owner must rebuild originals for each exact selected predecessor.
/// The callback supplies evidence only; it cannot override native verification.
pub type LoadFixture = Arc<dyn Fn(&bootstrap_outer::RootedBootstrapOmega) -> InstalledLoad>;

/// Actual terminal artifacts for the outer and subsequent-operation assertions.
#[derive(Clone)]
pub struct AuthenticatedLoad {
    /// Actual A5 key and descriptor.
    pub key: VerifyingKey<Eq>,
    /// Actual A5 descriptor.
    pub binding: DescriptorBinding,
    /// Original A5 proof.
    pub proof: Vec<u8>,
    /// Exact terminal frame.
    pub instances: Vec<Fp>,
    /// Derived original A5 opening.
    pub opening: FoldInput<Eq>,
    /// Fully verified carried Pallas obligation.
    pub pallas: AccumulatorT<Ep>,
    /// Fully verified terminal Vesta part.
    pub vesta_part: AccumulatorT<Eq>,
    /// Fully verified predecessor Vesta obligation.
    pub predecessor_vesta: AccumulatorT<Eq>,
    /// Exact funded successor state.
    pub state: StateWitness,
}

/// Import the required originals, prove all five A/four W stages and replay every
/// checkpoint from canonical original bytes. The fixture cannot bypass finality.
pub fn authenticated_load(
    rooted: &bootstrap_outer::RootedBootstrapOmega,
    fixture: &LoadFixture,
) -> AuthenticatedLoad {
    let source = fixture(rooted);
    assert_eq!(source.inputs.predecessor.proof, rooted.proof);
    assert_eq!(
        source.inputs.predecessor.pallas,
        rooted.source.pallas.to_bytes()
    );
    assert_eq!(source.inputs.predecessor.vesta, rooted.vesta.to_bytes());
    assert_eq!(
        source.inputs.state.predecessor.core,
        rooted.source.state.core
    );
    assert_eq!(
        source.inputs.state.predecessor.rest,
        rooted.source.state.rest
    );
    assert_eq!(
        source.inputs.state.predecessor.lineage,
        rooted.source.state.lineage
    );
    // Genuine originals are mandatory for these substitutions: no host verdict
    // or freshly invented source claim stands in for a finalized positive case.
    for substitution in 0..6 {
        let mut bad = source.inputs.clone();
        match substitution {
            0 => bad.finality.endpoints[1] += Fp::ONE,
            1 => bad.finality.proof.push(0),
            2 => {
                let mut challenges = *bad.finality.pallas.challenges();
                challenges[0] += Fq::ONE;
                bad.finality.pallas =
                    AccumulatorT::new(*bad.finality.pallas.g(), challenges).unwrap();
            }
            3 => {
                let mut challenges = *bad.finality.vesta.challenges();
                challenges[0] += Fp::ONE;
                bad.finality.vesta =
                    AccumulatorT::new(*bad.finality.vesta.g(), challenges).unwrap();
            }
            4 => bad.receipt[0] ^= 1,
            _ => bad.q[1].proof.push(0),
        }
        assert!(
            source.plan.prepare(bad, MemoryBudget::DEFAULT).is_err(),
            "source substitution {substitution}"
        );
    }
    let state = source.inputs.state.successor;
    let prover = Prover::from_artifacts(
        source.plan,
        core::array::from_fn(|i| source.a[i].metadata()),
        core::array::from_fn(|i| source.w[i].metadata()),
    )
    .expect("all original Load sources must match the installed plan");
    let binding = prover.descriptors()[8].clone();
    let key = VerifyingKey::read(&source.a[4].verifying_key, &binding).unwrap();
    let session = prover
        .prepare(source.inputs, MemoryBudget::DEFAULT)
        .expect("genuine finality, predecessor and Q originals are mandatory");
    let fold = FoldConfig::default();
    let config = ProverConfig::default();
    let active_seal = prover
        .import_a(0, &source.a[0].read(source.read.maximum_bytes), source.read)
        .unwrap();
    let active = prover.bind_a(0, &active_seal, None).unwrap();
    let mut a = session
        .first(&active, Fp::from(201), &fold, common::recovery(201), config)
        .unwrap();
    drop(active_seal);
    a = session
        .restore_first(a.proof().to_vec(), &a.pallas_bytes(), MemoryBudget::DEFAULT)
        .unwrap();
    for stage in 0..A_STAGE_COUNT - 1 {
        let seed = u8::try_from(202 + stage * 2).unwrap();
        let active_seal = prover
            .import_w(
                stage,
                &source.w[stage].read(source.read.maximum_bytes),
                source.read,
            )
            .unwrap();
        let active = prover.bind_w(stage, &active_seal, None).unwrap();
        let w = session
            .wrapper(
                &a,
                &active,
                Fq::from(u64::from(seed)),
                &fold,
                common::recovery(seed),
                config,
            )
            .unwrap();
        drop(active_seal);
        let w = session
            .restore_wrapper(
                &a,
                w.proof().to_vec(),
                &w.vesta_bytes(),
                MemoryBudget::DEFAULT,
            )
            .unwrap();
        let active_seal = prover
            .import_a(
                stage + 1,
                &source.a[stage + 1].read(source.read.maximum_bytes),
                source.read,
            )
            .unwrap();
        let active = prover.bind_a(stage + 1, &active_seal, None).unwrap();
        a = session
            .advance(
                &w,
                &active,
                Fp::from(u64::from(seed) + 1),
                &fold,
                common::recovery(seed + 1),
                config,
            )
            .unwrap();
        drop(active_seal);
        let mut changed = a.proof().to_vec();
        changed[0] ^= 1;
        assert!(
            session
                .restore_a(&w, changed, &a.pallas_bytes(), MemoryBudget::DEFAULT)
                .is_err()
        );
        a = session
            .restore_a(
                &w,
                a.proof().to_vec(),
                &a.pallas_bytes(),
                MemoryBudget::DEFAULT,
            )
            .unwrap();
    }
    let terminal = session.terminal(&a, MemoryBudget::DEFAULT).unwrap();
    AuthenticatedLoad {
        key,
        binding,
        proof: terminal.proof,
        instances: terminal.instances,
        opening: terminal.opening,
        pallas: terminal.pallas,
        vesta_part: terminal.vesta_part,
        predecessor_vesta: terminal.predecessor_vesta,
        state,
    }
}
