//! The golden cases on both sides: identifiers parsed from the vendored
//! table, the vendored keys and prover, the native export, keys and witness.
//!
//! [`setup`] builds each `(curve, family, k)` once per test process and
//! shares it ([`Arc`]) between `export_parity`, `proof_parity`,
//! `verdict_parity` and `timing`, so each golden is exported (and its
//! witness capture leaked, see `iroha_plonk_oracle::export`) once.

use std::{
    any::Any,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, OnceLock},
};

use halo2_axiom::{
    dev::MockProver,
    plonk::{
        Circuit, ConstraintSystem as VCs, ProvingKey as VProvingKey, keygen_pk as v_keygen_pk,
        keygen_vk as v_keygen_vk, verify_proof as v_verify_proof,
    },
    poly::{
        VerificationStrategy,
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::VerifierIPA,
            strategy::SingleStrategy,
        },
    },
    transcript::{Blake2bRead, Challenge255, TranscriptReadBuffer},
};
use iroha_plonk::{
    cs::{ProofSuffixV1, TranscriptV1},
    keys::ProvingKey,
    pcs::ipa::PinnedParams,
    prover::Witness,
};
use iroha_plonk_oracle::{
    convert::{CurveBridge, NativeScalar, native_scalars},
    export::{
        ExportedCircuit, configure_vendored, export_circuit, vendored_keygen_config,
        vendored_transcript_repr,
    },
};

use crate::golden_proof_bytes::{GOLDEN_CASES, GoldenVisitor, vendored_prove, visit_golden};

/// A golden circuit family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Two Pow5-style lanes, a bus and a byte lookup (degree 7).
    Sigma,
    /// Twelve advice columns, degree-4 products and two lookups.
    Wide,
}

impl Family {
    /// The label used in case names.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sigma => "sigma",
            Self::Wide => "wide",
        }
    }
}

/// One vendored golden case, `family/curve/k{k}/seed{seed}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseId {
    /// The vendored case name.
    pub name: &'static str,
    /// The family.
    pub family: Family,
    /// The curve label (`"eq"` or `"ep"`).
    pub curve: &'static str,
    /// `log2` of the domain size.
    pub k: u32,
    /// The seed byte (the prover seed is 32 copies of it).
    pub seed: u8,
    /// The recorded SHA-256 of the vendored proof bytes.
    pub sha256: &'static str,
}

impl CaseId {
    /// The 32-byte prover seed.
    pub const fn seed_bytes(&self) -> [u8; 32] {
        [self.seed; 32]
    }
}

/// Parses one golden case name.
fn parse(name: &'static str, sha256: &'static str) -> Option<CaseId> {
    let mut parts = name.split('/');
    let family = match parts.next()? {
        "sigma" => Family::Sigma,
        "wide" => Family::Wide,
        _ => return None,
    };
    let curve = parts.next()?;
    let k = parts.next()?.strip_prefix('k')?.parse().ok()?;
    let seed = parts.next()?.strip_prefix("seed")?.parse().ok()?;
    parts.next().is_none().then_some(CaseId {
        name,
        family,
        curve,
        k,
        seed,
        sha256,
    })
}

/// Every vendored golden case, in table order.
pub fn golden_cases() -> Vec<CaseId> {
    GOLDEN_CASES
        .iter()
        .map(|(name, sha256)| parse(name, sha256).expect("well-formed golden case name"))
        .collect()
}

/// The golden cases (both seeds) of one family, curve and `k`.
pub fn cases_for(family: Family, curve: &str, k: u32) -> Vec<CaseId> {
    let cases: Vec<_> = golden_cases()
        .into_iter()
        .filter(|case| case.family == family && case.curve == curve && case.k == k)
        .collect();
    assert_eq!(cases.len(), 2, "{}/{curve}/k{k}: two seeds", family.label());
    cases
}

/// A vendored prover bound to its circuit and public input.
type VendoredProver<B> = Box<
    dyn Fn(
            &ParamsIPA<<B as CurveBridge>::Vendored>,
            &VProvingKey<<B as CurveBridge>::Vendored>,
            [u8; 32],
        ) -> Vec<u8>
        + Send
        + Sync,
>;

/// Everything one golden `(family, curve, k)` needs on both sides.
pub struct Setup<B: CurveBridge> {
    /// The family.
    pub family: Family,
    /// `log2` of the domain size.
    pub k: u32,
    /// The vendored shape `(degree, permutation columns)`.
    pub shape: (usize, usize),
    /// The configure-time vendored constraint system.
    pub vendored_cs: VCs<B::VScalar>,
    /// The vendored parameters.
    pub vendored_params: ParamsIPA<B::Vendored>,
    /// The vendored proving key (`keygen_vk`, `keygen_pk` as the goldens run
    /// them).
    pub vendored_pk: VProvingKey<B::Vendored>,
    /// The vendored `MockProver` of the witness circuit.
    pub mock: MockProver<B::VScalar>,
    /// The public input (one instance column of one value).
    pub public: Vec<B::VScalar>,
    /// The native export.
    pub exported: ExportedCircuit<B>,
    /// The native parameters (derived).
    pub params: PinnedParams<B::Native>,
    /// The native proving key from the exported tables.
    pub pk: ProvingKey<B::Native>,
    /// The native witness from the exported tables.
    #[cfg_attr(not(iroha_plonk_oracle), allow(dead_code))]
    pub witness: Witness<NativeScalar<B>>,
    /// The vendored `transcript_repr`, injected in oracle mode.
    pub transcript_repr: NativeScalar<B>,
    #[cfg_attr(not(iroha_plonk_oracle), allow(dead_code))]
    prove_vendored: VendoredProver<B>,
    /// The vendored KAGEMUSHA path (Poseidon transcript, folded generator).
    prove_vendored_kagemusha: VendoredProver<B>,
    #[cfg_attr(not(iroha_plonk_oracle), allow(dead_code))]
    reexport: Box<dyn Fn() -> ExportedCircuit<B> + Send + Sync>,
}

// Proving and verifying run only in oracle builds (`--cfg iroha_plonk_oracle`).
#[cfg_attr(not(iroha_plonk_oracle), allow(dead_code))]
impl<B: CurveBridge> Setup<B> {
    /// The native instance columns.
    pub fn native_instances(&self) -> Vec<Vec<NativeScalar<B>>> {
        vec![native_scalars::<B>(&self.public)]
    }

    /// The vendored instance columns.
    pub fn vendored_instances(&self) -> Vec<Vec<B::VScalar>> {
        vec![self.public.clone()]
    }

    /// One vendored golden proof with `seed`.
    pub fn prove_vendored(&self, seed: [u8; 32]) -> Vec<u8> {
        (self.prove_vendored)(&self.vendored_params, &self.vendored_pk, seed)
    }

    /// One vendored KAGEMUSHA-path proof with `seed`
    /// ([`crate::kagemusha_vendored::prove_augmented`]).
    pub fn prove_vendored_kagemusha(&self, seed: [u8; 32]) -> Vec<u8> {
        (self.prove_vendored_kagemusha)(&self.vendored_params, &self.vendored_pk, seed)
    }

    /// The vendored augmented KAGEMUSHA verdict on `proof`
    /// ([`crate::kagemusha_vendored::verify_augmented`]).
    pub fn verify_vendored_kagemusha(
        &self,
        instances: &[Vec<B::VScalar>],
        proof: &[u8],
    ) -> Result<(), String> {
        crate::kagemusha_vendored::verify_augmented::<B>(
            &self.vendored_params,
            self.vendored_pk.get_vk(),
            instances,
            proof,
        )
    }

    /// Exports the circuit again (both capture passes), as the timing suite
    /// measures it.
    pub fn reexport(&self) -> ExportedCircuit<B> {
        (self.reexport)()
    }

    /// The vendored verdict on `proof` for `instances`: `Ok`, or the vendored
    /// error (a panic is reported as an error, never propagated).
    pub fn verify_vendored(
        &self,
        instances: &[Vec<B::VScalar>],
        proof: &[u8],
    ) -> Result<(), String> {
        let columns: Vec<&[B::VScalar]> = instances.iter().map(Vec::as_slice).collect();
        let per_proof: [&[&[B::VScalar]]; 1] = [&columns];
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut transcript =
                Blake2bRead::<_, B::Vendored, Challenge255<B::Vendored>>::init(proof);
            let strategy = SingleStrategy::<B::Vendored>::new(&self.vendored_params);
            v_verify_proof::<IPACommitmentScheme<B::Vendored>, VerifierIPA<'_, B::Vendored>, _, _, _>(
                &self.vendored_params,
                self.vendored_pk.get_vk(),
                strategy,
                &per_proof,
                &mut transcript,
            )
        }));
        match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(format!("{error:?}")),
            Err(_) => Err("vendored verifier panicked".to_owned()),
        }
    }
}

/// Builds a [`Setup`] from the circuit the vendored module hands out.
struct Builder<B> {
    family: Family,
    k: u32,
    marker: core::marker::PhantomData<B>,
}

impl<B: CurveBridge> GoldenVisitor<B::VScalar> for Builder<B> {
    type Output = Setup<B>;

    fn visit<C: Circuit<B::VScalar> + Clone + Send + Sync + 'static>(
        self,
        circuit: C,
        public: Vec<B::VScalar>,
        shape: (usize, usize),
    ) -> Setup<B> {
        let k = self.k;
        let label = format!("{}/{}/k{k}", self.family.label(), B::NAME);
        let vendored_params = ParamsIPA::<B::Vendored>::new(k);
        let vk = v_keygen_vk(&vendored_params, &circuit.without_witnesses())
            .unwrap_or_else(|error| panic!("{label}: vendored verifying key: {error:?}"));
        let vendored_pk = v_keygen_pk(&vendored_params, vk, &circuit.without_witnesses())
            .unwrap_or_else(|error| panic!("{label}: vendored proving key: {error:?}"));
        let (vendored_cs, _) = configure_vendored::<B::VScalar, C>(&circuit);
        let mock = MockProver::run(k, &circuit, vec![public.clone()])
            .unwrap_or_else(|error| panic!("{label}: MockProver: {error:?}"));
        let exported = export_circuit::<B, C>(k, &circuit, std::slice::from_ref(&public))
            .unwrap_or_else(|error| panic!("{label}: export: {error}"));
        let params = PinnedParams::<B::Native>::derive(k)
            .unwrap_or_else(|error| panic!("{label}: native params: {error:?}"));
        let pk = exported
            .keygen(
                &params,
                &vendored_keygen_config::<B>(
                    vendored_pk.get_vk(),
                    TranscriptV1::Blake2bChallenge255,
                    ProofSuffixV1::None,
                ),
            )
            .unwrap_or_else(|error| panic!("{label}: native proving key: {error}"));
        let witness = exported
            .witness(&pk)
            .unwrap_or_else(|error| panic!("{label}: native witness: {error}"));
        let transcript_repr = vendored_transcript_repr::<B>(vendored_pk.get_vk());
        let export_circuit_again = circuit.clone();
        let export_public = public.clone();
        let reexport = Box::new(move || {
            export_circuit::<B, C>(
                k,
                &export_circuit_again,
                std::slice::from_ref(&export_public),
            )
            .expect("export")
        });
        let prove_public = public.clone();
        let kagemusha_circuit = circuit.clone();
        let kagemusha_instances = vec![public.clone()];
        let prove_vendored_kagemusha: VendoredProver<B> = Box::new(move |params, pk, seed| {
            crate::kagemusha_vendored::prove_augmented::<B, C>(
                params,
                pk,
                &kagemusha_circuit,
                &kagemusha_instances,
                seed,
            )
        });
        let prove_vendored: VendoredProver<B> = Box::new(move |params, pk, seed| {
            vendored_prove(params, pk, &circuit, &prove_public, seed)
        });
        Setup {
            family: self.family,
            k,
            shape,
            vendored_cs,
            vendored_params,
            vendored_pk,
            mock,
            public,
            exported,
            params,
            pk,
            witness,
            transcript_repr,
            prove_vendored,
            prove_vendored_kagemusha,
            reexport,
        }
    }
}

/// A shared, lazily built setup.
type CachedSetup = Arc<OnceLock<Arc<dyn Any + Send + Sync>>>;

/// A setup cache key: `(curve, family, k)`.
type SetupKey = (&'static str, &'static str, u32);

/// The setups built so far.
static SETUPS: OnceLock<Mutex<BTreeMap<SetupKey, CachedSetup>>> = OnceLock::new();

/// Both sides of the golden `family` at `k` over curve `B`, built once per
/// process and shared. Concurrent callers of one key wait for one build;
/// different keys build in parallel.
pub fn setup<B: CurveBridge>(family: Family, k: u32) -> Arc<Setup<B>> {
    let key = (B::NAME, family.label(), k);
    let slot = {
        let mut setups = SETUPS
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(setups.entry(key).or_default())
    };
    let built = slot.get_or_init(|| {
        let setup: Arc<dyn Any + Send + Sync> = Arc::new(build_setup::<B>(family, k));
        setup
    });
    Arc::clone(built)
        .downcast::<Setup<B>>()
        .unwrap_or_else(|_| panic!("{key:?}: one setup type per key"))
}

/// Builds both sides of the golden `family` at `k` over curve `B`.
fn build_setup<B: CurveBridge>(family: Family, k: u32) -> Setup<B> {
    visit_golden::<B::VScalar, _>(
        family.label(),
        k,
        Builder::<B> {
            family,
            k,
            marker: core::marker::PhantomData,
        },
    )
    .expect("known family")
}

/// The index of the first differing 32-byte message, if any (a length
/// difference counts at the shorter length).
pub fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(a, b)| a != b)
        .or_else(|| (left.len() != right.len()).then(|| left.len().min(right.len())))
        .map(|byte| byte / 32)
}

#[test]
fn every_golden_case_name_parses() {
    let cases = golden_cases();
    assert_eq!(cases.len(), GOLDEN_CASES.len());
    for family in [Family::Sigma, Family::Wide] {
        for curve in ["eq", "ep"] {
            let ks: Vec<u32> = match family {
                Family::Sigma => vec![6, 9, 11],
                Family::Wide => vec![8, 10],
            };
            for k in ks {
                let seeds: Vec<u8> = cases_for(family, curve, k)
                    .iter()
                    .map(|case| case.seed)
                    .collect();
                assert_eq!(seeds, [42, 43]);
            }
        }
    }
    assert_eq!(parse("sigma/eq/k6/seed42/extra", ""), None);
    assert_eq!(parse("other/eq/k6/seed42", ""), None);
    assert_eq!(cases[0].seed_bytes(), [42; 32]);
}

/// The vendored KAGEMUSHA path is self-consistent on a golden: the
/// augmented proof verifies, and a wrong public input, a wrong suffix and
/// trailing bytes are rejected (as `iroha_core_zk::prover_golden_tests`
/// checks for its goldens).
#[test]
fn vendored_kagemusha_path_verifies_its_goldens() {
    use iroha_plonk_oracle::convert::Pallas;
    let setup = setup::<Pallas>(Family::Sigma, 6);
    let instances = setup.vendored_instances();
    let proof = setup.prove_vendored_kagemusha([42; 32]);
    assert_eq!(setup.verify_vendored_kagemusha(&instances, &proof), Ok(()));
    assert_eq!(
        proof,
        setup.prove_vendored_kagemusha([42; 32]),
        "deterministic"
    );
    let mut wrong = instances.clone();
    wrong[0][0] += <Pallas as CurveBridge>::VScalar::from(1);
    assert!(setup.verify_vendored_kagemusha(&wrong, &proof).is_err());
    let mut suffix = proof.clone();
    let last = suffix.len() - 1;
    suffix[last - 31..].copy_from_slice(&proof[..32]);
    assert!(
        setup
            .verify_vendored_kagemusha(&instances, &suffix)
            .is_err()
    );
    let mut trailing = proof.clone();
    trailing.push(0);
    assert!(
        setup
            .verify_vendored_kagemusha(&instances, &trailing)
            .is_err()
    );
    assert!(
        setup
            .verify_vendored_kagemusha(&instances, &proof[..16])
            .is_err()
    );
}

#[test]
fn setups_are_built_once_and_shared() {
    use iroha_plonk_oracle::convert::{Pallas, Vesta};
    let first = setup::<Vesta>(Family::Sigma, 6);
    let again = setup::<Vesta>(Family::Sigma, 6);
    assert!(Arc::ptr_eq(&first, &again));
    let other_curve = setup::<Pallas>(Family::Sigma, 6);
    assert_eq!(other_curve.k, first.k);
    assert_eq!(first.family, Family::Sigma);
}

#[test]
fn first_difference_names_the_message() {
    assert_eq!(first_difference(&[0; 64], &[0; 64]), None);
    let mut changed = [0_u8; 64];
    changed[40] = 1;
    assert_eq!(first_difference(&[0; 64], &changed), Some(1));
    assert_eq!(first_difference(&[0; 64], &[0; 32]), Some(1));
}
