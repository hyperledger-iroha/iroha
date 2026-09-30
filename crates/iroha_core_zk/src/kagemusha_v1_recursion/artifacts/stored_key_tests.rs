//! Independent small-key tests for the authenticated indexed owner and its original source.
//!
//! These use the actual structured codec and encrypted spool on Unix. The storage-only
//! for_stream_tests constructor deliberately does not authenticate a threshold release. Tiny
//! k=4/5 fixtures do not qualify fixed production circuits, standalone VK/profile/protocol
//! admission, stored proving, memory limits, devices or cryptographic assurance. Callback errors
//! and unwinds are exercised here; injected internal capture fatal/read-unwind cases remain in
//! the separate capture suite because its private snapshot/fault state is inaccessible here.

use std::{
    fs,
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use ff::{Field, PrimeField};
use halo2_proofs::{
    SerdeFormat,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Fixed, ProvingKey, Selector, keygen_pk2},
    poly::{Rotation, commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};

use super::super::{KagemushaDirectoryArtifactResolverV1, role_index};
use super::*;

#[derive(Clone)]
struct OriginalCircuit<F: Field> {
    extra_fixed: bool,
    marker: PhantomData<F>,
}

#[derive(Clone)]
struct OriginalConfig {
    advice: Column<Advice>,
    fixed: Column<Fixed>,
    selector: Selector,
}

impl<F: PrimeField> Circuit<F> for OriginalCircuit<F> {
    type Config = OriginalConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = bool;

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn params(&self) -> Self::Params {
        self.extra_fixed
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        Self::configure_with_params(meta, false)
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, extra: bool) -> Self::Config {
        let advice = meta.advice_column();
        let fixed = meta.fixed_column();
        let selector = meta.selector();
        meta.enable_equality(advice);
        meta.enable_equality(fixed);
        if extra {
            // Two extra columns make the wrong original parameter set unambiguously different
            // from the selector-compression allowance of the normal one-fixed-column fixture.
            meta.fixed_column();
            meta.fixed_column();
        }
        meta.create_gate("original indexed owner fixture", |meta| {
            let selector = meta.query_selector(selector);
            let advice = meta.query_advice(advice, Rotation::cur());
            let fixed = meta.query_fixed(fixed, Rotation::cur());
            vec![selector * (advice - fixed)]
        });
        OriginalConfig {
            advice,
            fixed,
            selector,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "original public fixed values and nontrivial copy cycle",
            |mut region| {
                let mut cells = Vec::new();
                for row in 0..8 {
                    config.selector.enable(&mut region, row)?;
                    let value = F::from(7);
                    let advice = region.assign_advice(config.advice, row, Value::known(value));
                    let fixed = region.assign_fixed(config.fixed, row, value);
                    region.constrain_equal(advice.cell(), fixed);
                    cells.push(advice.cell());
                }
                region.constrain_equal(cells[0], cells[7]);
                Ok(())
            },
        )
    }
}

fn key<C: KagemushaIndexedKeyCurveV1>(
    k: u32,
    compressed: bool,
    extra_fixed: bool,
) -> (ProvingKey<C>, Vec<u8>)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let parameters = ParamsIPA::<C>::new(k);
    let key = keygen_pk2(
        &parameters,
        &OriginalCircuit::<C::Scalar> {
            extra_fixed,
            marker: PhantomData,
        },
        compressed,
    )
    .expect("ordinary small key oracle");
    let mut bytes = Vec::new();
    key.write_structured_v1(&mut bytes)
        .expect("original structured oracle");
    (key, bytes)
}

fn role<C: KagemushaIndexedKeyCurveV1>() -> KagemushaArtifactRoleV1 {
    match C::PARITY {
        KagemushaPastaParityV1::Eq => KagemushaArtifactRoleV1::MintHashClaimPkEq,
        KagemushaPastaParityV1::Ep => KagemushaArtifactRoleV1::MintHashClaimPkEp,
    }
}

fn binding(role: KagemushaArtifactRoleV1, bytes: &[u8]) -> KagemushaArtifactBindingV1 {
    KagemushaArtifactBindingV1 {
        role,
        sha256: Sha256::digest(bytes).into(),
        byte_len: bytes.len() as u64,
    }
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    ErrorAt(usize),
    PanicAt(usize),
    InterruptedAt(usize),
    Overreport,
}

#[derive(Default)]
struct Observed {
    opens: AtomicUsize,
    drops: AtomicUsize,
    bytes: AtomicUsize,
    maximum_request: AtomicUsize,
}

struct Resolver {
    bytes: Arc<[u8]>,
    fault: Fault,
    observed: Arc<Observed>,
}

struct Reader {
    bytes: Arc<[u8]>,
    position: usize,
    fault: Fault,
    observed: Arc<Observed>,
}

impl KagemushaArtifactByteResolverV1 for Resolver {
    fn resolve_bytes(
        &self,
        _: KagemushaArtifactBindingV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        panic!("indexed ownership must not materialize through the whole-file resolver")
    }

    fn open_reader(
        &self,
        _: KagemushaArtifactBindingV1,
    ) -> Result<Box<dyn io::Read + Send>, KagemushaArtifactErrorV1> {
        self.observed.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Reader {
            bytes: Arc::clone(&self.bytes),
            position: 0,
            fault: self.fault,
            observed: Arc::clone(&self.observed),
        }))
    }
}

impl io::Read for Reader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.observed
            .maximum_request
            .fetch_max(output.len(), Ordering::SeqCst);
        match self.fault {
            Fault::ErrorAt(at) if at == self.position => {
                return Err(io::Error::other("original source failure"));
            }
            Fault::PanicAt(at) if at == self.position => panic!("original source unwind"),
            Fault::InterruptedAt(at) if at == self.position => {
                self.fault = Fault::None;
                return Err(io::ErrorKind::Interrupted.into());
            }
            Fault::Overreport => return Ok(output.len() + 1),
            _ => {}
        }
        let mut count = output.len().min(997).min(self.bytes.len() - self.position);
        match self.fault {
            Fault::ErrorAt(at) | Fault::PanicAt(at) | Fault::InterruptedAt(at)
                if at > self.position =>
            {
                count = count.min(at - self.position);
            }
            _ => {}
        }
        output[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
        self.position += count;
        self.observed.bytes.fetch_add(count, Ordering::SeqCst);
        Ok(count)
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.observed.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn fixture(
    bytes: Vec<u8>,
    binding: KagemushaArtifactBindingV1,
    fault: Fault,
) -> (KagemushaAuthenticatedArtifactSetV1<Resolver>, Arc<Observed>) {
    let observed = Arc::new(Observed::default());
    let resolver = Resolver {
        bytes: bytes.into(),
        fault,
        observed: Arc::clone(&observed),
    };
    (
        KagemushaAuthenticatedArtifactSetV1::for_stream_tests(resolver, binding),
        observed,
    )
}

fn capture<C: KagemushaIndexedKeyCurveV1, R: KagemushaArtifactByteResolverV1>(
    set: &KagemushaAuthenticatedArtifactSetV1<R>,
    k: u32,
    extra: bool,
    directory: &Path,
) -> Result<AuthenticatedIndexedProvingKeyV1<C>, KagemushaArtifactErrorV1>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    set.capture_indexed_proving_key::<C, OriginalCircuit<C::Scalar>>(
        role::<C>(),
        C::PARITY,
        k,
        extra,
        directory,
    )
}

fn empty(directory: &Path) {
    // Pathname cleanup only; this does not inspect detached descriptors or cryptographic keys.
    assert_eq!(fs::read_dir(directory).unwrap().count(), 0);
}

fn roundtrip<C: KagemushaIndexedKeyCurveV1>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let directory = tempfile::tempdir().unwrap();
    for k in [4, 5] {
        for compressed in [false, true] {
            for extra in [false, true] {
                let (ordinary, bytes) = key::<C>(k, compressed, extra);
                let original_vk = ordinary.get_vk().to_bytes(SerdeFormat::Processed);
                let bound = binding(role::<C>(), &bytes);
                let (set, observed) = fixture(bytes.clone(), bound, Fault::InterruptedAt(997));
                let owner =
                    capture::<C, _>(&set, k, extra, directory.path()).expect("same captured index");
                assert!(owner.matches_artifact_set(&set));
                assert_eq!(owner.binding(), bound);
                assert_eq!(
                    owner.checked_vk().to_bytes(SerdeFormat::Processed),
                    original_vk
                );
                assert_eq!(
                    owner.checked_vk().transcript_repr(),
                    ordinary.get_vk().transcript_repr()
                );
                assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
                assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
                assert_eq!(observed.bytes.load(Ordering::SeqCst), bytes.len());
                assert!(observed.maximum_request.load(Ordering::SeqCst) <= 8192);
                drop(set);
                drop(ordinary);
                let mut owner = owner;
                for iteration in 0..3 {
                    let (returned, digest) = owner
                        .with_original_reader::<_, KagemushaArtifactErrorV1>(|index, reader| {
                            assert_eq!(index.rows(), 1 << k);
                            assert_eq!(index.frame_bytes(), bound.byte_len);
                            assert_eq!(
                                index.get_vk().to_bytes(SerdeFormat::Processed),
                                original_vk
                            );
                            assert_eq!(reader.stream_position().unwrap(), 0);
                            let mut reproduced = Vec::new();
                            reader.read_to_end(&mut reproduced).unwrap();
                            assert_eq!(reproduced, bytes);
                            assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
                            reader
                                .seek(SeekFrom::Start((iteration * 11) as u64))
                                .unwrap();
                            let position = reader.stream_position().unwrap();
                            assert!(reader.seek(SeekFrom::End(1)).is_err());
                            assert_eq!(reader.stream_position().unwrap(), position);
                            let mut sample = [0; 17];
                            reader.read_exact(&mut sample).unwrap();
                            assert_eq!(sample, bytes[iteration * 11..iteration * 11 + 17]);
                            Ok(<[u8; 32]>::from(Sha256::digest(&reproduced)))
                        })
                        .expect("return same owner after original reader gate");
                    assert_eq!(digest, bound.sha256);
                    assert_eq!(returned.binding(), bound);
                    owner = returned;
                }
                drop(owner);
                assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
                empty(directory.path());
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn indexed_owner_both_pasta_retains_original_key_vk_canonical_bytes_and_same_owner_across_reads() {
    roundtrip::<EqAffine>();
    roundtrip::<EpAffine>();
}

fn all_roles<C: KagemushaIndexedKeyCurveV1>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let directory = tempfile::tempdir().unwrap();
    let (_, bytes) = key::<C>(4, true, false);
    let mut admitted = 0;
    let mut rejected = 0;
    for candidate_role in KagemushaArtifactRoleV1::ALL {
        let (set, observed) = fixture(bytes.clone(), binding(candidate_role, &bytes), Fault::None);
        let descriptor = KagemushaArtifactDescriptorV1::for_role(candidate_role);
        let actual = set.capture_indexed_proving_key::<C, OriginalCircuit<C::Scalar>>(
            candidate_role,
            C::PARITY,
            4,
            false,
            directory.path(),
        );
        if descriptor.kind == KagemushaArtifactKindV1::ProvingKey && descriptor.parity == C::PARITY
        {
            let owner = actual.expect("fixed proving-key role with correct curve parity");
            assert_eq!(owner.binding().role, candidate_role);
            assert!(owner.matches_artifact_set(&set));
            assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
            admitted += 1;
        } else {
            assert!(
                matches!(actual, Err(KagemushaArtifactErrorV1::InvalidBinding(role)) if role == candidate_role)
            );
            assert_eq!(observed.opens.load(Ordering::SeqCst), 0);
            rejected += 1;
        }
        empty(directory.path());
    }
    assert_eq!((admitted, rejected), (12, 38));
    let (set, observed) = fixture(bytes.clone(), binding(role::<C>(), &bytes), Fault::None);
    let other = match C::PARITY {
        KagemushaPastaParityV1::Eq => KagemushaPastaParityV1::Ep,
        KagemushaPastaParityV1::Ep => KagemushaPastaParityV1::Eq,
    };
    assert!(
        set.capture_indexed_proving_key::<C, OriginalCircuit<C::Scalar>>(
            role::<C>(),
            other,
            4,
            false,
            directory.path()
        )
        .is_err()
    );
    let other_role = match C::PARITY {
        KagemushaPastaParityV1::Eq => KagemushaArtifactRoleV1::MintHashClaimPkEp,
        KagemushaPastaParityV1::Ep => KagemushaArtifactRoleV1::MintHashClaimPkEq,
    };
    assert!(
        set.capture_indexed_proving_key::<C, OriginalCircuit<C::Scalar>>(
            other_role,
            other,
            4,
            false,
            directory.path()
        )
        .is_err()
    );
    for k in [32, u32::MAX] {
        assert!(capture::<C, _>(&set, k, false, directory.path()).is_err());
    }
    assert_eq!(observed.opens.load(Ordering::SeqCst), 0);
    for case in 0..4 {
        let mut bound = binding(role::<C>(), &bytes);
        match case {
            0 => bound.byte_len = 0,
            1 => {
                bound.byte_len = KagemushaArtifactDescriptorV1::for_role(role::<C>()).byte_limit + 1
            }
            2 => bound.sha256 = [0; 32],
            3 => bound.role = other_role,
            _ => unreachable!(),
        }
        let (mut set, observed) = fixture(bytes.clone(), binding(role::<C>(), &bytes), Fault::None);
        set.bindings[role_index(role::<C>())] = bound;
        assert!(capture::<C, _>(&set, 4, false, directory.path()).is_err());
        assert_eq!(observed.opens.load(Ordering::SeqCst), 0);
    }
}

#[cfg(unix)]
#[test]
fn indexed_owner_all_roles_bind_actual_curve_and_refuse_invalid_descriptor_shape_before_opening() {
    all_roles::<EqAffine>();
    all_roles::<EpAffine>();
}

fn identity<C: KagemushaIndexedKeyCurveV1>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let directory = tempfile::tempdir().unwrap();
    let (_, bytes) = key::<C>(4, true, false);
    let bound = binding(role::<C>(), &bytes);
    let (original, _) = fixture(bytes.clone(), bound, Fault::None);
    let owner = capture::<C, _>(&original, 4, false, directory.path()).unwrap();
    for case in 0..16 {
        let (mut other, observed) = fixture(bytes.clone(), bound, Fault::None);
        match case {
            0 => other.recursion.release_id[0] ^= 1,
            1 => other.recursion.profile_digest[0] ^= 1,
            2 => other.recursion.artifact_manifest_digest[0] ^= 1,
            3 => other.native_profile_digest[0] ^= 1,
            4 => other.provider_policy_root[0] ^= 1,
            5 => other.suite_id[0] ^= 1,
            6 => other.vk_set_digest[0] ^= 1,
            7 => other.bindings[role_index(role::<C>())].sha256[0] ^= 1,
            8 => other.bindings[role_index(role::<C>())].byte_len -= 1,
            9 => other.bindings[role_index(role::<C>())].role = KagemushaArtifactRoleV1::StateVkEq,
            10 => other.recursion.eq_protocol_digest[0] ^= 1,
            11 => other.recursion.ep_protocol_digest[0] ^= 1,
            12 => other.recursion.mint_hash_claim_eq_protocol_digest[0] ^= 1,
            13 => other.recursion.mint_hash_shard_ep_protocol_digest[0] ^= 1,
            14 => other.recursion.canonical_empty_effect_digest[0] ^= 1,
            15 => other.recursion.mint_finality.proving_key_ep.sha256[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(
            !owner.matches_artifact_set(&other),
            "identity substitution {case}"
        );
        assert_eq!(observed.opens.load(Ordering::SeqCst), 0);
        assert!(owner.matches_artifact_set(&original));
    }
    let (_, other_bytes) = key::<C>(5, false, true);
    let (other, _) = fixture(
        other_bytes.clone(),
        binding(role::<C>(), &other_bytes),
        Fault::None,
    );
    let other_owner = capture::<C, _>(&other, 5, true, directory.path()).unwrap();
    assert!(!owner.matches_artifact_set(&other));
    assert!(!other_owner.matches_artifact_set(&original));
    for (owner, expected, k) in [(owner, bytes, 4), (other_owner, other_bytes, 5)] {
        let _ = owner
            .with_original_reader::<_, KagemushaArtifactErrorV1>(|index, reader| {
                assert_eq!(index.get_vk().get_domain().k(), k);
                let mut actual = Vec::new();
                reader.read_to_end(&mut actual).unwrap();
                assert_eq!(actual, expected);
                Ok(())
            })
            .unwrap();
    }
    empty(directory.path());
}

#[cfg(unix)]
#[test]
fn indexed_owner_rejects_release_profile_protocol_provider_and_original_binding_substitutions() {
    identity::<EqAffine>();
    identity::<EpAffine>();
}

fn malformed<C: KagemushaIndexedKeyCurveV1>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let directory = tempfile::tempdir().unwrap();
    let (ordinary, bytes) = key::<C>(4, true, false);
    let vk_bytes = ordinary.get_vk().to_bytes(SerdeFormat::Processed).len();
    let first_mask = 56 + vk_bytes + 4;
    let first_fixed_mode = 56 + vk_bytes + 3 * (4 + 16 * 32) + 4;
    assert_eq!(
        bytes[first_fixed_mode], 2,
        "fixture first fixed column uses canonical raw values"
    );
    let other_curve = match C::PARITY {
        KagemushaPastaParityV1::Eq => key::<EpAffine>(4, true, false).1,
        KagemushaPastaParityV1::Ep => key::<EqAffine>(4, true, false).1,
    };
    for case in 0..16 {
        let mut changed = bytes.clone();
        let mut expected_k = 4;
        let mut extra = false;
        match case {
            0 => changed[0] ^= 1,
            1 => changed[16] ^= 1,
            2 => changed[48] ^= 1,
            3 => changed[56] ^= 1,
            4 => changed[57..61].copy_from_slice(&5_u32.to_le_bytes()),
            5 => changed[61] = 2,
            6 => changed[62..66].copy_from_slice(&u32::MAX.to_le_bytes()),
            7 => changed[first_mask - 4..first_mask].copy_from_slice(&15_u32.to_be_bytes()),
            8 => changed[first_mask..first_mask + 32].fill(255),
            9 => changed[first_fixed_mode] = 255,
            10 => changed[first_fixed_mode + 1..first_fixed_mode + 1 + 16 * 32].fill(0),
            11 => {
                changed.pop();
            }
            12 => changed.push(0),
            13 => expected_k = 5,
            14 => extra = true,
            15 => changed = other_curve.clone(),
            _ => unreachable!(),
        }
        // Rebind every malformed byte sequence: source capture must authenticate it completely
        // before the original checked parser, trusted params or canonical checks reject it.
        let bound = binding(role::<C>(), &changed);
        let (set, observed) = fixture(changed.clone(), bound, Fault::None);
        let result = capture::<C, _>(&set, expected_k, extra, directory.path());
        assert!(
            matches!(result, Err(KagemushaArtifactErrorV1::Read { role: rejected, .. }) if rejected == role::<C>()),
            "canonical/shape case {case}"
        );
        assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        assert_eq!(observed.bytes.load(Ordering::SeqCst), changed.len());
        empty(directory.path());
    }
    // The shared writer remains the same bounded writer used by all original dense loaders.
    let bound = binding(role::<C>(), &bytes);
    let mut writer = CanonicalArtifactDigestWriterV1::new(bound.byte_len);
    for chunk in bytes.chunks(13) {
        writer.write_all(chunk).unwrap();
    }
    assert!(writer.matches(bound));
    let mut writer = CanonicalArtifactDigestWriterV1::new(bound.byte_len);
    writer.write_all(&bytes).unwrap();
    let mut other = bound;
    other.sha256[0] ^= 1;
    assert!(!writer.matches(other));
    let mut short = CanonicalArtifactDigestWriterV1::new(bound.byte_len);
    short.write_all(&bytes[..bytes.len() - 1]).unwrap();
    assert!(!short.matches(bound));
}

#[cfg(unix)]
#[test]
fn indexed_owner_authenticates_then_rejects_malformed_nonminimal_wrong_curve_k_and_circuit_params()
{
    malformed::<EqAffine>();
    malformed::<EpAffine>();
}

fn source_failures<C: KagemushaIndexedKeyCurveV1>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let directory = tempfile::tempdir().unwrap();
    let (_, bytes) = key::<C>(5, true, false);
    let original = binding(role::<C>(), &bytes);
    for case in 0..11 {
        let mut supplied = bytes.clone();
        let mut bound = original;
        let fault = match case {
            0 => {
                bound.sha256[0] ^= 1;
                Fault::None
            }
            1 => {
                supplied.pop();
                Fault::None
            }
            2 => {
                supplied.push(91);
                Fault::None
            }
            3 => Fault::ErrorAt(0),
            4 => Fault::ErrorAt(1003),
            5 => Fault::ErrorAt(bytes.len()),
            6 => Fault::Overreport,
            7 => {
                supplied[1013] ^= 1;
                Fault::None
            }
            8 => Fault::PanicAt(0),
            9 => Fault::PanicAt(1003),
            10 => Fault::PanicAt(bytes.len()),
            _ => unreachable!(),
        };
        let (set, observed) = fixture(supplied, bound, fault);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            capture::<C, _>(&set, 5, false, directory.path())
        }));
        if case >= 8 {
            assert!(outcome.is_err(), "original source unwind {case}");
        } else {
            let outcome = outcome.expect("ordinary original source refusal");
            match case {
                0 | 7 => assert!(
                    matches!(outcome, Err(KagemushaArtifactErrorV1::DigestMismatch(rejected)) if rejected == role::<C>())
                ),
                2 => assert!(
                    matches!(outcome, Err(KagemushaArtifactErrorV1::TrailingBytes(rejected)) if rejected == role::<C>())
                ),
                _ => assert!(outcome.is_err(), "original source failure {case}"),
            }
        }
        assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        empty(directory.path());
    }
}

#[cfg(unix)]
#[test]
fn indexed_owner_never_escapes_original_source_digest_length_io_or_unwind_failures() {
    source_failures::<EqAffine>();
    source_failures::<EpAffine>();
}

#[derive(Debug)]
enum ConsumerError {
    Artifact(KagemushaArtifactErrorV1),
    Deliberate,
}

impl From<KagemushaArtifactErrorV1> for ConsumerError {
    fn from(error: KagemushaArtifactErrorV1) -> Self {
        Self::Artifact(error)
    }
}

fn callbacks<C: KagemushaIndexedKeyCurveV1>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let directory = tempfile::tempdir().unwrap();
    let (_, bytes) = key::<C>(4, true, false);
    let (set, observed) = fixture(bytes.clone(), binding(role::<C>(), &bytes), Fault::None);
    let sentinel = capture::<C, _>(&set, 4, false, directory.path()).unwrap();
    for case in 0..4 {
        let mut owner = Some(capture::<C, _>(&set, 4, false, directory.path()).unwrap());
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner
                .take()
                .unwrap()
                .with_original_reader::<(), ConsumerError>(|index, reader| {
                    assert_eq!(index.rows(), 16);
                    if case % 2 == 1 {
                        let mut prefix = [0; 13];
                        reader.read_exact(&mut prefix).unwrap();
                        assert_eq!(prefix, bytes[..13]);
                    }
                    if case < 2 {
                        return Err(ConsumerError::Deliberate);
                    }
                    panic!("original reader consumer unwind")
                })
        }));
        assert!(owner.is_none());
        if case < 2 {
            assert!(matches!(outcome, Ok(Err(ConsumerError::Deliberate))));
        } else {
            assert!(outcome.is_err());
        }
        assert!(sentinel.matches_artifact_set(&set));
        empty(directory.path());
    }
    let sentinel = sentinel.with_original_reader::<_, ConsumerError>(|index, reader| {
        let mut original = Vec::new();
        reader.read_to_end(&mut original).unwrap();
        assert_eq!(original, bytes);
        assert_eq!(index.frame_bytes(), original.len() as u64);
        Ok(())
    });
    match sentinel {
        Ok((same, ())) => assert!(same.matches_artifact_set(&set)),
        Err(ConsumerError::Artifact(error)) => panic!("unrelated capture changed: {error}"),
        Err(ConsumerError::Deliberate) => panic!("unexpected consumer refusal"),
    }
    assert_eq!(observed.opens.load(Ordering::SeqCst), 5);
    assert_eq!(observed.drops.load(Ordering::SeqCst), 5);
    empty(directory.path());
}

#[cfg(unix)]
#[test]
fn indexed_owner_consumes_callback_errors_and_unwinds_without_affecting_another_original_owner() {
    callbacks::<EqAffine>();
    callbacks::<EpAffine>();
}

fn replacement<C: KagemushaIndexedKeyCurveV1>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let artifacts = tempfile::tempdir().unwrap();
    let spool = tempfile::tempdir().unwrap();
    let (ordinary, bytes) = key::<C>(4, true, false);
    let (other, changed) = key::<C>(5, false, true);
    let standalone_vk = ordinary.get_vk().to_bytes(SerdeFormat::Processed);
    let changed_vk = other.get_vk().to_bytes(SerdeFormat::Processed);
    assert_ne!(standalone_vk, changed_vk);
    let resolver = KagemushaDirectoryArtifactResolverV1::new(artifacts.path()).unwrap();
    let bound = binding(role::<C>(), &bytes);
    let address = resolver.path_for_digest(bound.sha256);
    fs::write(&address, &bytes).unwrap();
    let set = KagemushaAuthenticatedArtifactSetV1::for_stream_tests(resolver, bound);
    let owner = capture::<C, _>(&set, 4, false, spool.path()).unwrap();
    // Replace the name with a different real key after original authentication/indexing.
    fs::remove_file(&address).unwrap();
    fs::write(&address, &changed).unwrap();
    assert!(capture::<C, _>(&set, 4, false, spool.path()).is_err());
    assert!(owner.matches_artifact_set(&set));
    assert_eq!(
        owner.checked_vk().to_bytes(SerdeFormat::Processed),
        standalone_vk
    );
    assert_ne!(
        owner.checked_vk().to_bytes(SerdeFormat::Processed),
        changed_vk
    );
    drop(set);
    fs::remove_file(&address).unwrap();
    let (owner, ()) = owner
        .with_original_reader::<_, KagemushaArtifactErrorV1>(|index, reader| {
            assert_eq!(
                index.get_vk().to_bytes(SerdeFormat::Processed),
                standalone_vk
            );
            let mut original = Vec::new();
            reader.read_to_end(&mut original).unwrap();
            assert_eq!(original, bytes);
            assert_ne!(original, changed);
            Ok(())
        })
        .unwrap();
    assert_eq!(owner.binding(), bound);
    drop(owner);
    empty(spool.path());
}

#[cfg(unix)]
#[test]
fn indexed_owner_uses_captured_original_after_content_address_replacement_and_resolver_drop() {
    replacement::<EqAffine>();
    replacement::<EpAffine>();
}
