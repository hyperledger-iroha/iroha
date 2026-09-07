//! Shared native proof framing and public proving material for final Kaigi V1.

use halo2_proofs::{
    SerdeFormat,
    halo2curves::{ff::PrimeField as _, pasta::EqAffine},
    plonk::{Circuit, ProvingKey, create_proof, keygen_pk, keygen_vk},
    poly::{
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::ProverIPA,
        },
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
};
use iroha_data_model::{
    proof::{ProofBox, VerifyingKeyBox},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use kaigi_zk::{Scalar, authorization_v1::KaigiAuthorizationWitnessV1};
use napi::{
    Env,
    bindgen_prelude::{JsObjectValue as _, Uint8ArraySlice},
};
use rand_core_06::OsRng;

pub(super) const VK_BACKEND: &str = "halo2/ipa";
const ZK1_PREFIX: &[u8] = b"ZK1\0";

pub(super) fn invalid(message: impl ToString) -> napi::Error {
    napi::Error::new(napi::Status::InvalidArg, message.to_string())
}
pub(super) fn failure(message: impl ToString) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, message.to_string())
}

pub(super) fn take_witness(bytes: &mut [u8]) -> napi::Result<KaigiAuthorizationWitnessV1> {
    let mut owned = [0; 32];
    if bytes.len() == owned.len() {
        owned.copy_from_slice(bytes);
    }
    iroha_crypto::zeroize_value_for_confidential_discard(bytes);
    if bytes.len() != owned.len() {
        return Err(invalid(
            "blinding must contain exactly 32 canonical Pasta Fp bytes",
        ));
    }
    KaigiAuthorizationWitnessV1::take_blinding(&mut owned).map_err(invalid)
}

pub(super) fn consume_blinding(
    env: Env,
    blinding: &mut Uint8ArraySlice<'_>,
) -> napi::Result<KaigiAuthorizationWitnessV1> {
    let length =
        u32::try_from(blinding.len()).map_err(|_| invalid("blinding exceeds JS index width"))?;
    let mut owned = [0; 32];
    if length == 32 {
        owned.copy_from_slice(blinding.as_ref());
    }
    let witness = take_witness(&mut owned);
    let zero = env.create_uint32(0)?;
    // Scoped VM writes avoid an aliased mutable Rust slice into JS memory.
    // No JavaScript callback runs; the owned witness is dropped on every error.
    for index in 0..length {
        blinding.set_element(index, zero)?;
    }
    if length != 32 {
        return Err(invalid(
            "blinding must contain exactly 32 canonical Pasta Fp bytes",
        ));
    }
    witness
}

fn append_tlv(bytes: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) -> Result<(), String> {
    let length = u32::try_from(payload.len()).map_err(|_| "ZK1 payload exceeds u32".to_owned())?;
    bytes.extend_from_slice(&tag);
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(())
}

/// Public, witness-free setup for one fixed final circuit; safe to share.
pub(super) struct KaigiProvingMaterialV1 {
    params: ParamsIPA<EqAffine>,
    pk: ProvingKey<EqAffine>,
    pub(super) key: VerifyingKeyBox,
}
impl KaigiProvingMaterialV1 {
    pub(super) fn new<C: Circuit<Scalar>>(
        k: u32,
        circuit: &C,
        circuit_id: &str,
    ) -> Result<Self, String> {
        let params: ParamsIPA<EqAffine> = ParamsIPA::new(k);
        let vk = keygen_vk(&params, circuit).map_err(|error| error.to_string())?;
        let pk = keygen_pk(&params, vk.clone(), circuit).map_err(|error| error.to_string())?;
        let mut carrier = ZK1_PREFIX.to_vec();
        append_tlv(&mut carrier, *b"IPAK", &k.to_le_bytes())?;
        append_tlv(&mut carrier, *b"CID1", circuit_id.as_bytes())?;
        append_tlv(&mut carrier, *b"H2VK", &vk.to_bytes(SerdeFormat::Processed))?;
        Ok(Self {
            params,
            pk,
            key: VerifyingKeyBox::new(VK_BACKEND.to_owned(), carrier),
        })
    }
}

pub(super) fn encode_verified_envelope(
    envelope: &OpenVerifyEnvelope,
    key: &VerifyingKeyBox,
) -> napi::Result<Vec<u8>> {
    let encoded = norito::encode_canonical(envelope).map_err(failure)?;
    let proof = ProofBox::new(VK_BACKEND.to_owned(), encoded.clone());
    if !iroha_core::zk::verify_backend(VK_BACKEND, &proof, Some(key)) {
        return Err(failure(
            "generated Kaigi proof failed canonical native verification",
        ));
    }
    Ok(encoded)
}

pub(super) fn prove<C: Circuit<Scalar>>(
    material: &KaigiProvingMaterialV1,
    circuit: C,
    instance: &[Scalar],
    circuit_id: &str,
    schema: &[u8],
) -> napi::Result<Vec<u8>> {
    let mut transcript = Blake2bWrite::<_, EqAffine, Challenge255<EqAffine>>::init(Vec::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        Challenge255<EqAffine>,
        _,
        _,
        _,
    >(
        &material.params,
        &material.pk,
        &[circuit],
        &[&[instance]],
        OsRng,
        &mut transcript,
    )
    .map_err(failure)?;
    let mut proof = ZK1_PREFIX.to_vec();
    append_tlv(&mut proof, *b"PROF", &transcript.finalize()).map_err(failure)?;
    let mut rows = Vec::with_capacity(8 + instance.len() * 32);
    rows.extend_from_slice(&1_u32.to_le_bytes());
    rows.extend_from_slice(
        &u32::try_from(instance.len())
            .map_err(failure)?
            .to_le_bytes(),
    );
    for scalar in instance {
        rows.extend_from_slice(scalar.to_repr().as_ref());
    }
    append_tlv(&mut proof, *b"I10P", &rows).map_err(failure)?;
    encode_verified_envelope(
        &OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: circuit_id.to_owned(),
            vk_hash: iroha_core::zk::hash_vk(&material.key),
            public_inputs: schema.to_vec(),
            proof_bytes: proof,
            aux: Vec::new(),
        },
        &material.key,
    )
}
