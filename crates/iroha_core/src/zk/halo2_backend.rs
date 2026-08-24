//! Internal adapter for the vendored Halo2 Pasta IPA backend.
//!
//! This module intentionally keeps `halo2-axiom` proof plumbing behind a small
//! Iroha-owned surface so runtime code does not spread direct dependency usage
//! across verifier dispatch and proof builders.
use halo2_proofs::{
    SerdeFormat,
    circuit::{AssignedCell, Region, Value},
    halo2curves::{
        ff::Field,
        pasta::{EqAffine, Fp},
    },
    plonk::{
        Advice, Assigned, Circuit, Column, Error as PlonkError, ProvingKey as Halo2ProvingKey,
        VerifyingKey as Halo2VerifyingKey, create_proof as halo2_create_proof,
        keygen_pk as halo2_keygen_pk, keygen_vk as halo2_keygen_vk,
        verify_proof as halo2_verify_proof,
    },
    poly::{
        VerificationStrategy,
        commitment::{Params as _, ParamsProver as _},
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::{ProverIPA, VerifierIPA},
            strategy::SingleStrategy,
        },
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer as _,
        TranscriptWriterBuffer as _,
    },
};
use rand_core_06::OsRng;
use std::{io, io::Write};
/// Pasta curve used by the transparent Halo2 IPA backend.
pub(crate) type Curve = EqAffine;
/// Pasta scalar field used by the transparent Halo2 IPA backend.
pub(crate) type Scalar = Fp;
/// IPA parameters for the Pasta backend.
pub(crate) type PastaParams = ParamsIPA<Curve>;
/// Verifying key for the Pasta backend.
pub(crate) type VerifyingKey = Halo2VerifyingKey<Curve>;
/// Proving key for the Pasta backend.
pub(crate) type ProvingKey = Halo2ProvingKey<Curve>;
/// Plonk error emitted by the Pasta backend.
pub(crate) type Error = PlonkError;
/// Construct deterministic Pasta IPA parameters for a domain size exponent.
pub(crate) fn params_new(k: u32) -> PastaParams {
    ParamsIPA::<Curve>::new(k)
}
/// Generate a Pasta Halo2 verifying key.
pub(crate) fn keygen_vk<C>(params: &PastaParams, circuit: &C) -> Result<VerifyingKey, PlonkError>
where
    C: Circuit<Scalar>,
{
    halo2_keygen_vk(params, circuit)
}
/// Generate a Pasta Halo2 proving key from an existing verifying key.
pub(crate) fn keygen_pk<C>(
    params: &PastaParams,
    vk: VerifyingKey,
    circuit: &C,
) -> Result<ProvingKey, PlonkError>
where
    C: Circuit<Scalar>,
{
    halo2_keygen_pk(params, vk, circuit)
}
/// Return the standard processed verifying-key serialization.
pub(crate) fn verifying_key_to_processed_bytes(vk: &VerifyingKey) -> Vec<u8> {
    vk.to_bytes(SerdeFormat::Processed)
}
/// Return the standard processed proving-key serialization.
pub(crate) fn proving_key_to_processed_bytes(pk: &ProvingKey) -> Vec<u8> {
    pk.to_bytes(SerdeFormat::Processed)
}
/// Return the processed verifying-key serialization embedded in a proving key.
pub(crate) fn proving_key_vk_to_processed_bytes(pk: &ProvingKey) -> Vec<u8> {
    verifying_key_to_processed_bytes(pk.get_vk())
}
/// Return the proving-key domain exponent.
pub(crate) fn proving_key_domain_k(pk: &ProvingKey) -> u32 {
    pk.get_vk().get_domain().k()
}
/// Canonical constraint-system failure used by cache adapters.
pub(crate) fn constraint_system_failure() -> Error {
    PlonkError::ConstraintSystemFailure
}
/// Read a processed Pasta verifying key, respecting the optional circuit-params API.
pub(crate) fn read_verifying_key<C, R>(reader: &mut R) -> io::Result<VerifyingKey>
where
    R: io::Read,
    C: Circuit<Scalar>,
    C::Params: Default,
{
    #[cfg(feature = "circuit-params")]
    {
        VerifyingKey::read::<_, C>(reader, SerdeFormat::Processed, C::Params::default())
    }
    #[cfg(not(feature = "circuit-params"))]
    {
        VerifyingKey::read::<_, C>(reader, SerdeFormat::Processed)
    }
}
/// Read a processed Pasta proving key, respecting the optional circuit-params API.
pub(crate) fn read_proving_key<C, R>(reader: &mut R) -> io::Result<ProvingKey>
where
    R: io::Read,
    C: Circuit<Scalar>,
    C::Params: Default,
{
    #[cfg(feature = "circuit-params")]
    {
        ProvingKey::read::<_, C>(reader, SerdeFormat::Processed, C::Params::default())
    }
    #[cfg(not(feature = "circuit-params"))]
    {
        ProvingKey::read::<_, C>(reader, SerdeFormat::Processed)
    }
}
/// Assign advice through the canonical vendored Halo2 API shape.
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub(crate) fn assign_advice_vendored<'r, F, A, AR, V, T>(
    region: &mut Region<'r, F>,
    annotation: A,
    column: Column<Advice>,
    offset: usize,
    mut to: V,
) -> Result<AssignedCell<&'r Assigned<F>, F>, PlonkError>
where
    F: Field,
    A: Fn() -> AR,
    AR: Into<String>,
    V: FnMut() -> Value<T>,
    T: Into<Assigned<F>>,
{
    let _ = annotation;
    let value = to().map(Into::into);
    Ok(Region::assign_advice(region, column, offset, value))
}
/// Hash serialized IPA parameters for VK cache keys.
pub(crate) fn params_fingerprint(params: &PastaParams) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    struct HashWriter<'a, H: Digest>(&'a mut H);
    impl<H: Digest> Write for HashWriter<'_, H> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.update(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut hasher = Sha256::new();
    let mut writer = HashWriter(&mut hasher);
    params
        .write(&mut writer)
        .expect("failed to hash Halo2 params");
    hasher.finalize().into()
}
/// Create a Pasta IPA proof and return the raw transcript bytes.
pub(crate) fn create_ipa_proof<C>(
    params: &PastaParams,
    pk: &ProvingKey,
    circuits: &[C],
    instances: &[&[&[Scalar]]],
) -> Result<Vec<u8>, PlonkError>
where
    C: Circuit<Scalar>,
{
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(Vec::new());
    halo2_create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(params, pk, circuits, instances, OsRng, &mut transcript)?;
    Ok(transcript.finalize())
}
/// Verify a Pasta IPA proof from raw transcript bytes.
pub(crate) fn verify_ipa_proof(
    params: &PastaParams,
    vk: &VerifyingKey,
    proof_payload: &[u8],
    instances: &[&[&[Scalar]]],
) -> Result<(), PlonkError> {
    let mut cursor = io::Cursor::new(proof_payload);
    let mut transcript = Blake2bRead::<_, Curve, Challenge255<Curve>>::init(&mut cursor);
    let strategy = SingleStrategy::new(params);
    halo2_verify_proof::<
        IPACommitmentScheme<Curve>,
        VerifierIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
    >(params, vk, strategy, instances, &mut transcript)?;
    drop(transcript);
    if cursor.position() != u64::try_from(proof_payload.len()).expect("proof length must fit u64") {
        return Err(PlonkError::Transcript(io::Error::new(
            io::ErrorKind::InvalidData,
            "Halo2 IPA proof contains trailing transcript bytes",
        )));
    }
    Ok(())
}
/// Verify a Pasta IPA proof that has no public instances.
#[allow(dead_code)]
pub(crate) fn verify_ipa_proof_no_instances(
    params: &PastaParams,
    vk: &VerifyingKey,
    proof_payload: &[u8],
) -> Result<(), PlonkError> {
    let instances: [&[Scalar]; 0] = [];
    verify_ipa_proof(params, vk, proof_payload, &[&instances])
}
/// Verify a Pasta IPA proof whose public inputs are provided as instance columns.
#[allow(dead_code)]
pub(crate) fn verify_ipa_proof_with_columns(
    params: &PastaParams,
    vk: &VerifyingKey,
    proof_payload: &[u8],
    columns: &[&[Scalar]],
) -> Result<(), PlonkError> {
    let proofs_instances = [columns];
    verify_ipa_proof(params, vk, proof_payload, &proofs_instances)
}
#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Instance},
        poly::{
            VerificationStrategy,
            ipa::{multiopen::VerifierIPA, strategy::AccumulatorStrategy},
        },
    };
    /// A deliberately tiny circuit used to exercise Halo2's native IPA accumulator strategy. This
    /// is a host-side batch-verification proof of concept, not a recursive verifier circuit:
    /// `AccumulatorStrategy` keeps an unevaluated MSM in private Rust state and `finalize` decides
    /// it on the host.
    #[derive(Clone, Default)]
    struct PublicValue {
        value: Scalar,
    }
    impl Circuit<Scalar> for PublicValue {
        type Config = (Column<Advice>, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self::default()
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            (advice, instance)
        }
        fn synthesize(
            &self,
            (advice, instance): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let cell = layouter.assign_region(
                || "public value",
                |mut region| {
                    let cell = assign_advice_vendored(
                        &mut region,
                        || "value",
                        advice,
                        0,
                        || Value::known(self.value),
                    )?;
                    Ok(cell.cell())
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }
    #[test]
    fn verify_ipa_proof_rejects_trailing_transcript_bytes() {
        let params = params_new(5);
        let value = Scalar::from(7);
        let circuit = PublicValue { value };
        let vk = keygen_vk(&params, &circuit).expect("tiny verifier key");
        let pk = keygen_pk(&params, vk.clone(), &circuit).expect("tiny proving key");
        let column = [value];
        let columns: [&[Scalar]; 1] = [&column];
        let instances: [&[&[Scalar]]; 1] = [&columns];
        let mut proof = create_ipa_proof(&params, &pk, &[circuit], &instances)
            .expect("create canonical tiny IPA proof");
        verify_ipa_proof(&params, &vk, &proof, &instances).expect("canonical proof must verify");

        proof.push(0);
        let error = verify_ipa_proof(&params, &vk, &proof, &instances)
            .expect_err("a trailing byte must make the transcript non-canonical");
        assert!(
            matches!(
                &error,
                PlonkError::Transcript(inner) if inner.kind() == io::ErrorKind::InvalidData
            ),
            "unexpected trailing-byte error: {error}"
        );
    }
    #[test]
    fn native_ipa_accumulator_batches_full_plonk_proofs_but_is_not_recursive() {
        let params = params_new(5);
        let circuit = PublicValue {
            value: Scalar::from(7),
        };
        let vk = keygen_vk(&params, &circuit).expect("tiny verifier key");
        let pk = keygen_pk(&params, vk.clone(), &circuit).expect("tiny proving key");
        let values = [Scalar::from(7), Scalar::from(11)];
        let proofs = values
            .iter()
            .map(|value| {
                let circuit = PublicValue { value: *value };
                let column = [*value];
                let columns: [&[Scalar]; 1] = [&column];
                create_ipa_proof(&params, &pk, &[circuit], &[&columns]).expect("tiny IPA proof")
            })
            .collect::<Vec<_>>();
        let mut strategy = AccumulatorStrategy::new(&params);
        for (proof, value) in proofs.iter().zip(values) {
            let column = [value];
            let columns: [&[Scalar]; 1] = [&column];
            let instances: [&[&[Scalar]]; 1] = [&columns];
            let mut transcript =
                Blake2bRead::<_, Curve, Challenge255<Curve>>::init(io::Cursor::new(proof));
            strategy = halo2_verify_proof::<
                IPACommitmentScheme<Curve>,
                VerifierIPA<'_, Curve>,
                Challenge255<Curve>,
                _,
                _,
            >(&params, &vk, strategy, &instances, &mut transcript)
            .expect("well-formed proof contributes to accumulator");
        }
        assert!(strategy.finalize(), "both accumulated proofs must verify");
        let wrong_column = [Scalar::from(8)];
        let wrong_columns: [&[Scalar]; 1] = [&wrong_column];
        let wrong_instances: [&[&[Scalar]]; 1] = [&wrong_columns];
        let mut transcript =
            Blake2bRead::<_, Curve, Challenge255<Curve>>::init(io::Cursor::new(&proofs[0]));
        let wrong_strategy = halo2_verify_proof::<
            IPACommitmentScheme<Curve>,
            VerifierIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
        >(
            &params,
            &vk,
            AccumulatorStrategy::new(&params),
            &wrong_instances,
            &mut transcript,
        )
        .expect("instance substitution is detected when the MSM is decided");
        assert!(
            !wrong_strategy.finalize(),
            "the native accumulator must reject a substituted public instance"
        );
    }
}
