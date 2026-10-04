//! Key generation, proving and verification of the **prototype** step
//! relations ([`SigmaProver`], [`SigmaVerifier`]).
//!
//! A verifier needs only the descriptor bytes, the verifying-key bytes, the
//! pinned parameters of `k` and the public outputs (spec section 8): it never
//! configures the circuit. [`SigmaProver::prove`] refuses a witness that
//! breaks the relation ([`SigmaError::RelationViolated`]) before the engine
//! runs; the circuit itself has no satisfying assignment for one either.

use core::fmt;

use iroha_pasta::{PastaCurve, msm::MemoryBudget, poseidon::PoseidonField};
use iroha_plonk::{
    DescriptorBinding, KeyError, ProverConfig, ProverError, ProverRandomness, ProvingKey,
    VerifyError, VerifyingKey,
    cs::{CsError, DescriptorError},
    frontend::Error,
    keys::{KeygenConfig, VkError, keygen_pk},
    pcs::ipa::{ParamsTrustError, PinnedParams},
    prove_circuit, verify_full,
};
use iroha_plonk_gadgets::statement::StepRelation;

use crate::{
    circuit::{ParamsError, SigmaCircuit},
    shape::{ProofFormat, SigmaShape},
    witness::{StepPublic, StepWitness, Violation},
};

/// Why a step-relation operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SigmaError {
    /// The circuit parameters are invalid.
    Params(ParamsError),
    /// The relation does not fit `k`.
    DoesNotFit {
        /// The `k` tried.
        k: u32,
    },
    /// No shape in the policy's range fits and meets the byte budget.
    NoShape,
    /// Synthesis failed.
    Synthesis(Error),
    /// The constraint system could not be finalized.
    ConstraintSystem(CsError),
    /// The descriptor is invalid.
    Descriptor(DescriptorError),
    /// The protocol tables could not be derived.
    Protocol(iroha_plonk::ProtocolError),
    /// The curve is not a descriptor curve (never for the Pasta curves).
    UnknownCurve,
    /// The parameters could not be derived or are not pinned.
    ParamsTrust(ParamsTrustError),
    /// The parameters have another `k` than the shape.
    ParamsK {
        /// The shape's `k`.
        expected: u32,
        /// The parameters' `k`.
        found: u32,
    },
    /// Key generation failed.
    Key(KeyError),
    /// The verifying key failed strict decoding.
    VerifyingKey(VkError),
    /// The witness belongs to another step relation.
    WrongRelation {
        /// The circuit's relation.
        expected: StepRelation,
        /// The witness's relation.
        found: StepRelation,
    },
    /// The witness breaks the relation.
    RelationViolated(Vec<Violation>),
    /// The public outputs do not match the relation (a Request digest for
    /// `sigma_recv`, or none for `sigma_send`).
    PublicShape,
    /// The engine failed to prove.
    Prover(ProverError),
    /// The proof was rejected.
    Verify(VerifyError),
}

impl fmt::Display for SigmaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Params(error) => write!(f, "parameters: {error}"),
            Self::DoesNotFit { k } => write!(f, "the relation does not fit k = {k}"),
            Self::NoShape => f.write_str("no shape fits the policy"),
            Self::Synthesis(error) => write!(f, "synthesis: {error}"),
            Self::ConstraintSystem(error) => write!(f, "constraint system: {error}"),
            Self::Descriptor(error) => write!(f, "descriptor: {error}"),
            Self::Protocol(error) => write!(f, "protocol: {error}"),
            Self::UnknownCurve => f.write_str("the curve is not a descriptor curve"),
            Self::ParamsTrust(error) => write!(f, "parameters: {error}"),
            Self::ParamsK { expected, found } => {
                write!(
                    f,
                    "parameters for k = {found}, the shape needs k = {expected}"
                )
            }
            Self::Key(error) => write!(f, "key generation: {error}"),
            Self::VerifyingKey(error) => write!(f, "verifying key: {error}"),
            Self::WrongRelation { expected, found } => {
                write!(f, "a {found:?} witness for a {expected:?} circuit")
            }
            Self::RelationViolated(violations) => {
                write!(f, "the witness breaks the relation: {violations:?}")
            }
            Self::PublicShape => f.write_str("the public outputs do not match the relation"),
            Self::Prover(error) => write!(f, "prover: {error}"),
            Self::Verify(error) => write!(f, "verifier: {error:?}"),
        }
    }
}

impl std::error::Error for SigmaError {}

/// Memory and speed choices of a proving key. They change neither the
/// verifying key nor any proof byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyOptions {
    /// The budget of each fixed-base commitment table (of `g` and
    /// `g_lagrange`), or `None` for no tables. At `k = 11` a table takes
    /// about 3 MiB and the pair saves about 14% of the prover's time.
    pub commitment_tables: Option<MemoryBudget>,
}

impl KeyOptions {
    /// Fixed-base tables of at most 8 MiB each.
    pub const WITH_TABLES: Self = Self {
        commitment_tables: Some(MemoryBudget::new(8 << 20)),
    };
}

/// A step proof and the public outputs it proves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigmaProof<F> {
    /// The public outputs.
    pub public: StepPublic<F>,
    /// The proof bytes.
    pub bytes: Vec<u8>,
}

/// The public outputs as the single instance column, checked against the
/// relation.
fn instance<F: Copy>(
    relation: StepRelation,
    public: &StepPublic<F>,
) -> Result<Vec<Vec<F>>, SigmaError> {
    let request = public.request.is_some();
    if request != (relation == StepRelation::Send) {
        return Err(SigmaError::PublicShape);
    }
    Ok(vec![public.instance()])
}

/// The proving side of one step shape: parameters and proving key.
#[derive(Clone, Debug)]
pub struct SigmaProver<C: PastaCurve> {
    shape: SigmaShape,
    format: ProofFormat,
    params: PinnedParams<C>,
    pk: ProvingKey<C>,
}

impl<C: PastaCurve> SigmaProver<C>
where
    C::ScalarExt: PoseidonField,
{
    /// Derives the parameters of `shape.k` and generates the keys.
    ///
    /// # Errors
    ///
    /// [`SigmaError::ParamsTrust`] from the derivation, and the errors of
    /// [`Self::keygen_with_params`].
    pub fn keygen(shape: SigmaShape, format: ProofFormat) -> Result<Self, SigmaError> {
        let params = PinnedParams::derive(shape.k).map_err(SigmaError::ParamsTrust)?;
        Self::keygen_with_params(shape, format, params)
    }

    /// Generates the keys with already derived or pinned `params` and the
    /// default [`KeyOptions`].
    ///
    /// # Errors
    ///
    /// As [`Self::keygen_with_options`].
    pub fn keygen_with_params(
        shape: SigmaShape,
        format: ProofFormat,
        params: PinnedParams<C>,
    ) -> Result<Self, SigmaError> {
        Self::keygen_with_options(shape, format, params, KeyOptions::default())
    }

    /// Generates the keys with already derived or pinned `params` and
    /// `options` (which change memory and speed, never a key or proof byte).
    ///
    /// # Errors
    ///
    /// [`SigmaError::ParamsK`] when `params` are for another `k`, and
    /// [`SigmaError::Key`] from key generation (a shape that does not fit
    /// included).
    pub fn keygen_with_options(
        shape: SigmaShape,
        format: ProofFormat,
        params: PinnedParams<C>,
        options: KeyOptions,
    ) -> Result<Self, SigmaError> {
        if params.k() != shape.k {
            return Err(SigmaError::ParamsK {
                expected: shape.k,
                found: params.k(),
            });
        }
        let mut config = KeygenConfig::new(format.transcript);
        config.instance_mode = format.instance_mode;
        config.proof_suffix = format.proof_suffix;
        config.table_budget = options.commitment_tables;
        let circuit = SigmaCircuit::<C::ScalarExt>::keygen(shape.params);
        let pk = keygen_pk(&params, &circuit, &config).map_err(SigmaError::Key)?;
        Ok(Self {
            shape,
            format,
            params,
            pk,
        })
    }

    /// The shape.
    #[must_use]
    pub const fn shape(&self) -> &SigmaShape {
        &self.shape
    }

    /// The proof format.
    #[must_use]
    pub const fn format(&self) -> ProofFormat {
        self.format
    }

    /// The parameters.
    #[must_use]
    pub const fn params(&self) -> &PinnedParams<C> {
        &self.params
    }

    /// The proving key.
    #[must_use]
    pub const fn proving_key(&self) -> &ProvingKey<C> {
        &self.pk
    }

    /// The circuit of `witness` under this shape.
    ///
    /// # Errors
    ///
    /// [`SigmaError::WrongRelation`] for a witness of the other step.
    pub fn circuit(
        &self,
        witness: &StepWitness<C::ScalarExt>,
    ) -> Result<SigmaCircuit<C::ScalarExt>, SigmaError> {
        let expected = self.shape.params.relation().step;
        let found = witness.relation();
        if expected != found {
            return Err(SigmaError::WrongRelation { expected, found });
        }
        Ok(SigmaCircuit::new(self.shape.params, witness.clone()))
    }

    /// Proves `witness` with `randomness` (on the caller's Rayon pool).
    ///
    /// # Errors
    ///
    /// [`SigmaError::WrongRelation`], [`SigmaError::RelationViolated`] for a
    /// witness that breaks the relation (checked natively first), and
    /// [`SigmaError::Prover`] from the engine.
    pub fn prove(
        &self,
        witness: &StepWitness<C::ScalarExt>,
        randomness: ProverRandomness<'_>,
    ) -> Result<SigmaProof<C::ScalarExt>, SigmaError> {
        let circuit = self.circuit(witness)?;
        let native = circuit
            .native()
            .ok_or(SigmaError::Synthesis(Error::Synthesis))?;
        if !native.is_honest() {
            return Err(SigmaError::RelationViolated(native.violations.clone()));
        }
        let public = native.public();
        let instances = instance(witness.relation(), &public)?;
        let bytes = prove_circuit(
            &self.params,
            &self.pk,
            &circuit,
            &instances,
            randomness,
            ProverConfig::default(),
        )
        .map_err(SigmaError::Prover)?;
        Ok(SigmaProof { public, bytes })
    }

    /// The matching verifier.
    #[must_use]
    pub fn verifier(&self) -> SigmaVerifier<C> {
        SigmaVerifier {
            relation: self.shape.params.relation().step,
            params: self.params.clone(),
            binding: self.pk.binding().clone(),
            vk: self.pk.vk().clone(),
            budget: MemoryBudget::DEFAULT,
        }
    }
}

/// The verifying side: the descriptor, the verifying key and the pinned
/// parameters (no circuit code).
#[derive(Clone, Debug)]
pub struct SigmaVerifier<C: PastaCurve> {
    relation: StepRelation,
    params: PinnedParams<C>,
    binding: DescriptorBinding,
    vk: VerifyingKey<C>,
    budget: MemoryBudget,
}

impl<C: PastaCurve> SigmaVerifier<C>
where
    C::ScalarExt: PoseidonField,
{
    /// A verifier from the canonical descriptor and verifying-key bytes.
    ///
    /// # Errors
    ///
    /// [`SigmaError::Descriptor`] or [`SigmaError::VerifyingKey`] from strict
    /// decoding, and [`SigmaError::ParamsK`] when `params` are for another
    /// `k`.
    pub fn from_bytes(
        relation: StepRelation,
        params: PinnedParams<C>,
        descriptor: &[u8],
        vk: &[u8],
    ) -> Result<Self, SigmaError> {
        let binding = DescriptorBinding::decode(descriptor).map_err(SigmaError::Descriptor)?;
        let k = u32::from(binding.descriptor().k);
        if params.k() != k {
            return Err(SigmaError::ParamsK {
                expected: k,
                found: params.k(),
            });
        }
        let vk = VerifyingKey::read(vk, &binding).map_err(SigmaError::VerifyingKey)?;
        Ok(Self {
            relation,
            params,
            binding,
            vk,
            budget: MemoryBudget::DEFAULT,
        })
    }

    /// The canonical descriptor bytes.
    #[must_use]
    pub fn descriptor_bytes(&self) -> &[u8] {
        self.binding.encoded()
    }

    /// The verifying-key bytes.
    #[must_use]
    pub fn vk_bytes(&self) -> &[u8] {
        self.vk.to_bytes()
    }

    /// The descriptor binding.
    #[must_use]
    pub const fn binding(&self) -> &DescriptorBinding {
        &self.binding
    }

    /// The verifying key.
    #[must_use]
    pub const fn vk(&self) -> &VerifyingKey<C> {
        &self.vk
    }

    /// The parameters.
    #[must_use]
    pub const fn params(&self) -> &PinnedParams<C> {
        &self.params
    }

    /// Verifies `proof` for `public` in full.
    ///
    /// # Errors
    ///
    /// [`SigmaError::PublicShape`] for outputs of the wrong shape and
    /// [`SigmaError::Verify`] with the engine's typed rejection.
    pub fn verify(
        &self,
        public: &StepPublic<C::ScalarExt>,
        proof: &[u8],
    ) -> Result<(), SigmaError> {
        let instances = instance(self.relation, public)?;
        verify_full(
            &self.params,
            &self.binding,
            &self.vk,
            &instances,
            proof,
            self.budget,
        )
        .map_err(SigmaError::Verify)
    }
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use iroha_pasta::Fp;

    use super::*;

    #[test]
    fn instances_follow_the_relation() {
        let send = StepPublic {
            statement: Fp::ONE,
            request: Some(Fp::ZERO),
        };
        let receive = StepPublic {
            statement: Fp::ONE,
            request: None,
        };
        assert_eq!(
            instance(StepRelation::Send, &send),
            Ok(vec![vec![Fp::ONE, Fp::ZERO]])
        );
        assert_eq!(
            instance(StepRelation::Receive, &receive),
            Ok(vec![vec![Fp::ONE]])
        );
        assert_eq!(
            instance(StepRelation::Send, &receive),
            Err(SigmaError::PublicShape)
        );
        assert_eq!(
            instance(StepRelation::Receive, &send),
            Err(SigmaError::PublicShape)
        );
    }

    #[test]
    fn errors_display() {
        for error in [
            SigmaError::NoShape,
            SigmaError::DoesNotFit { k: 9 },
            SigmaError::PublicShape,
            SigmaError::UnknownCurve,
            SigmaError::ParamsK {
                expected: 10,
                found: 11,
            },
            SigmaError::WrongRelation {
                expected: StepRelation::Send,
                found: StepRelation::Receive,
            },
            SigmaError::RelationViolated(vec![Violation::Overdraft]),
            SigmaError::Synthesis(Error::Synthesis),
            SigmaError::Params(ParamsError::Lanes(0)),
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
        }
    }
}
