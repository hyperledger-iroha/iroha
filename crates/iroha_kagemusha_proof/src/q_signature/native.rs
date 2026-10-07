//! Native original-key intake and proving of the fixed P-256 signature Q leaf.
//!
//! The installed owner authenticates the complete original descriptor/VK/PK and
//! selects the signature slots, hard/soft modes and fixed roots independently of
//! operation input. This component checks that exact compiled source and key
//! continuity, derives verdicts from the raw signatures, proves those verdicts,
//! and fully self-verifies the Q proof. It generates no runtime key.
//!
//! A still binds every exported digest, key and signature to its original object
//! tapes and owns the global incoming/burn branch. A Q leaf alone is neither a
//! monetary verifier nor a completed lineage. TODO(G3/G4): authenticate this
//! component in the whole producer catalog and connect original G1 preparation,
//! complete operation schedules and the final uniform Omega owner before open.

use core::fmt;

use ff::Field;
use iroha_pasta::{Ep, Fp, Fq};
use iroha_plonk::{
    DescriptorBinding, KeyError, ProverConfig, ProverError, ProverRandomness, ProvingKey,
    VerifyError, VerifyingKey, Witness, create_proof_owned,
    cs::{CurveV1, InstanceModeV1, ProofSuffixV1, TranscriptV2},
    frontend::{Circuit, Error as LayoutError},
    keys::pk::artifact::{Error as ArtifactError, ReadConfig},
    pcs::ipa::PinnedParams,
    verifier::verify_full,
};
use iroha_plonk_gadgets::{
    p256::{
        VerifyMode,
        native::{Affine, verify_prehashed},
    },
    sha256::native::sha256_of_digest,
};

use super::{QSignatureCircuit, QSignaturePlan, SignatureKey, SignatureWitness};

const DESCRIPTOR_MAX_BYTES: usize = 1 << 20;
const VERIFYING_KEY_MAX_BYTES: usize = 1 << 18;

/// Fixed-source signature-Q intake or proof failure; no failure advances custody.
#[derive(Debug)]
pub enum QSignatureError {
    /// Invalid witness dimensions or signature leaf layout.
    Layout(LayoutError),
    /// The pinned outer parameters must cover exactly k16.
    Parameters,
    /// The installed descriptor is not the fixed V2 signature-Q profile.
    Profile,
    /// Original descriptor decoding failed.
    Descriptor(iroha_plonk::cs::DescriptorError),
    /// Bounded original-key source/commitment import failed.
    Artifact(ArtifactError),
    /// The original PK does not retain the independently installed VK.
    UnauthorizedKey,
    /// A supplied raw key differs from its fixed signature slot.
    KeyBinding,
    /// A hard slot's native low-S P-256 signature was rejected.
    Signature,
    /// Circuit assignment or actual proof generation failed.
    Prover(ProverError),
    /// Complete self-verification failed.
    Verify(VerifyError),
}
impl fmt::Display for QSignatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "signature Q: {self:?}")
    }
}
impl std::error::Error for QSignatureError {}

/// One immutable installed signature-Q source and its original imported key.
/// Private fields prevent a witness from changing slot policy or fixed roots.
pub struct QSignatureProver {
    plan: QSignaturePlan,
    params: PinnedParams<Ep>,
    key: ProvingKey<Ep>,
}
impl QSignaturePlan {
    /// Reconstruct the exact unknown signature-Q source for offline key tooling.
    /// Slot order, hard/soft modes and fixed keys remain circuit constants;
    /// digest/key/signature witnesses are unknown. This creates no accepted
    /// signature verdict, prepared operation or catalog authority.
    /// # Errors
    /// Invalid fixed slot dimensions or source layout.
    pub fn source_circuit(&self) -> Result<QSignatureCircuit, QSignatureError> {
        let blank = SignatureWitness {
            digest: Fp::ZERO,
            key: [[0; 4]; 2],
            signature: [[0; 4]; 2],
        };
        QSignatureCircuit::new(self.clone(), vec![blank; self.slots().len()])
            .map(|source| source.without_witnesses())
            .map_err(QSignatureError::Layout)
    }
}

impl QSignatureProver {
    /// Import one original under the exact independently installed slot plan.
    /// No key generation, alternate profile or import fallback is performed.
    ///
    /// The complete originals, scheme/catalog scope and resource policy must
    /// already be authenticated by the native installation owner. This method
    /// checks fixed source/key continuity; it grants no catalog admission,
    /// `NativeProofs` implementation, wallet-open capability or enrollment identity.
    /// Original/domain bounds do not qualify total synthesis/prover memory.
    ///
    /// # Errors
    /// Wrong k16/Pallas/V2 signature profile or original bounds, noncanonical
    /// descriptor/VK/PK, substituted installed VK, or fixed-slot source mismatch.
    pub fn from_original_artifact(
        plan: QSignaturePlan,
        params: PinnedParams<Ep>,
        descriptor: &[u8],
        installed_vk: &[u8],
        original: &[u8],
        config: ReadConfig,
    ) -> Result<Self, QSignatureError> {
        if params.k() != 16 {
            return Err(QSignatureError::Parameters);
        }
        if descriptor.is_empty()
            || descriptor.len() > DESCRIPTOR_MAX_BYTES
            || installed_vk.is_empty()
            || installed_vk.len() > VERIFYING_KEY_MAX_BYTES
            || original.len() > config.maximum_bytes
        {
            return Err(QSignatureError::Artifact(ArtifactError::Length));
        }
        let binding =
            DescriptorBinding::decode_v2(descriptor).map_err(QSignatureError::Descriptor)?;
        let d = binding.descriptor();
        let instance_length =
            u32::try_from(plan.instance_length()).map_err(|_| QSignatureError::Profile)?;
        if d.k != 16
            || d.curve != CurveV1::Pallas
            || d.transcript != TranscriptV2::KagemushaPoseidonRp57Base
            || d.instance_mode != InstanceModeV1::Direct
            || d.proof_suffix != ProofSuffixV1::FoldedGenerator
            || d.instance_lengths.as_slice() != [instance_length]
            || d.instance_types.as_deref() != Some(&QSignaturePlan::instance_types())
        {
            return Err(QSignatureError::Profile);
        }
        // Bound the original/domain before source construction, allocation or import.
        if binding.n() > config.maximum_rows {
            return Err(QSignatureError::Artifact(ArtifactError::Length));
        }
        VerifyingKey::<Ep>::read(installed_vk, &binding).map_err(|error| {
            QSignatureError::Artifact(ArtifactError::Key(KeyError::VerifyingKey(error)))
        })?;
        let circuit = plan.source_circuit()?;
        let key = ProvingKey::from_artifact_v2(original, &binding, &params, &circuit, config)
            .map_err(QSignatureError::Artifact)?;
        if key.vk().to_bytes() != installed_vk {
            return Err(QSignatureError::UnauthorizedKey);
        }
        Ok(Self { plan, params, key })
    }

    /// Exact installed slot order, hard/soft policy and fixed keys.
    #[must_use]
    pub const fn plan(&self) -> &QSignaturePlan {
        &self.plan
    }

    /// Imported original material for native package production; export adds no authority.
    #[must_use]
    pub const fn proving_key(&self) -> &ProvingKey<Ep> {
        &self.key
    }

    /// Fixed original V2 descriptor used by A's signature-Q verifier.
    #[must_use]
    pub fn binding(&self) -> &DescriptorBinding {
        self.key.binding()
    }

    /// Fixed Q verifying key, already retained by the imported original PK.
    #[must_use]
    pub fn verifying_key(&self) -> &VerifyingKey<Ep> {
        self.key.vk()
    }

    /// Pinned k16 Pallas parameters of this immutable component.
    #[must_use]
    pub const fn params(&self) -> &PinnedParams<Ep> {
        &self.params
    }

    /// Prove exact raw signature inputs under the installed policy and self-verify.
    /// Verdicts are derived from the actual low-S native P-256 check; the caller
    /// supplies no verdict or operation-wide incoming/burn decision. Soft failure
    /// remains a proved false bit for A's mandatory total incoming rule.
    ///
    /// # Errors
    /// Wrong slot count or fixed key, invalid hard signature, circuit/proof failure
    /// or failed complete self-verification. No failure changes a wallet head.
    pub fn prove(
        &self,
        witnesses: &[SignatureWitness],
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<QSignatureProof, QSignatureError> {
        let verdicts = signature_verdicts(&self.plan, witnesses)?;
        let circuit = QSignatureCircuit::new(self.plan.clone(), witnesses.to_vec())
            .map_err(QSignatureError::Layout)?;
        let instances = circuit
            .instances(&verdicts)
            .map_err(QSignatureError::Layout)?;
        let witness = Witness::from_circuit(&self.key, &circuit, &instances)
            .map_err(QSignatureError::Prover)?;
        let bytes = create_proof_owned(&self.params, &self.key, witness, randomness, config)
            .map_err(QSignatureError::Prover)?;
        verify_full(
            &self.params,
            self.binding(),
            self.verifying_key(),
            &instances,
            &bytes,
            config.msm_budget,
        )
        .map_err(QSignatureError::Verify)?;
        Ok(QSignatureProof { bytes, instances })
    }
}

fn signature_verdicts(
    plan: &QSignaturePlan,
    witnesses: &[SignatureWitness],
) -> Result<Vec<bool>, QSignatureError> {
    if witnesses.len() != plan.slots().len() {
        return Err(QSignatureError::Layout(LayoutError::Synthesis));
    }
    let mut verdicts = Vec::with_capacity(witnesses.len());
    for (slot, witness) in plan.slots().iter().zip(witnesses) {
        if let SignatureKey::Fixed(key) = slot.key
            && witness.key != [key.x, key.y]
        {
            return Err(QSignatureError::KeyBinding);
        }
        let digest = sha256_of_digest(&witness.digest);
        let message = core::array::from_fn(|index| {
            let offset = (3 - index) * 8;
            u64::from_be_bytes(core::array::from_fn(|byte| digest[offset + byte]))
        });
        let verdict = verify_prehashed(
            &message,
            &witness.signature[0],
            &witness.signature[1],
            &Affine {
                x: witness.key[0],
                y: witness.key[1],
            },
        );
        if slot.mode == VerifyMode::Hard && !verdict {
            return Err(QSignatureError::Signature);
        }
        verdicts.push(verdict);
    }
    Ok(verdicts)
}

/// Actual self-verified Q bytes and their exact bound public signature columns.
/// A must still bind these values to original objects and retain the opening.
#[derive(Clone, Debug)]
pub struct QSignatureProof {
    /// Exact descriptor-sized unframed PIPA-R Q proof.
    pub bytes: Vec<u8>,
    /// The actual digests, raw keys/signatures and proved native verdicts.
    pub instances: [Vec<Fq>; 1],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::q_signature::SignatureSlot;

    #[test]
    fn derived_verdicts_preserve_hard_soft_and_fixed_key_boundaries() {
        let hard = QSignaturePlan::new(vec![SignatureSlot {
            mode: VerifyMode::Hard,
            key: SignatureKey::Variable,
        }])
        .unwrap();
        let soft = QSignaturePlan::new(vec![SignatureSlot {
            mode: VerifyMode::Soft,
            key: SignatureKey::Variable,
        }])
        .unwrap();
        let invalid = SignatureWitness {
            digest: Fp::ZERO,
            key: [[0; 4]; 2],
            signature: [[0; 4]; 2],
        };
        assert!(matches!(
            signature_verdicts(&hard, &[invalid]),
            Err(QSignatureError::Signature)
        ));
        assert_eq!(signature_verdicts(&soft, &[invalid]).unwrap(), [false]);
        assert!(matches!(
            signature_verdicts(&soft, &[]),
            Err(QSignatureError::Layout(_))
        ));
        let fixed = QSignaturePlan::new(vec![SignatureSlot {
            mode: VerifyMode::Soft,
            key: SignatureKey::Fixed(Affine::GENERATOR),
        }])
        .unwrap();
        assert!(matches!(
            signature_verdicts(&fixed, &[invalid]),
            Err(QSignatureError::KeyBinding)
        ));
    }
}
