//! Native prover helpers for ZK-ACE post-quantum authorization v0.

use core::fmt;

use iroha_core::zk_stark::{STARK_HASH_SHA256_V1, StarkFriParamsV1, StarkFriVerifyingKeyV1};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::id::AssetDefinitionId,
    confidential::ConfidentialStatus,
    proof::{ProofAttachment, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::{
        BackendTag, ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID, ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        ZkAcePublicInputsV1, ZkAceWitnessV1, derive_zk_ace_identity_commitment,
        derive_zk_ace_replay_nullifier, derive_zk_ace_transfer_digest,
        zk_ace_public_inputs_schema_hash_v1,
    },
};

/// Error returned by ZK-ACE prover helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZkAceProverError(String);

impl ZkAceProverError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ZkAceProverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ZkAceProverError {}

/// Result type for this crate.
pub type Result<T> = core::result::Result<T, ZkAceProverError>;

/// Complete ZK-ACE transparent-transfer authorization artifact.
#[derive(Debug, Clone)]
pub struct ZkAceTransferAuthorizationV1 {
    /// Canonical public inputs that validators bind to the transfer instruction.
    pub public_inputs: ZkAcePublicInputsV1,
    /// STARK/FRI proof attachment for `public_inputs`.
    pub proof: ProofAttachment,
    /// Norito-encoded canonical public-input bytes.
    pub public_inputs_bytes: Vec<u8>,
}

/// Deterministic STARK/FRI parameters for the v0 authorization circuit.
#[must_use]
pub fn zk_ace_stark_fri_params_v1() -> StarkFriParamsV1 {
    StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 1,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:zk-ace:stark-fri:v0".to_owned(),
    }
}

/// Build the canonical STARK/FRI verifying-key payload for ZK-ACE v0.
pub fn zk_ace_verifying_key_box_v1() -> Result<VerifyingKeyBox> {
    let params = zk_ace_stark_fri_params_v1();
    let payload = StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.to_owned(),
        n_log2: params.n_log2,
        blowup_log2: params.blowup_log2,
        fold_arity: params.fold_arity,
        queries: params.queries,
        merkle_arity: params.merkle_arity,
        hash_fn: params.hash_fn,
    };
    let bytes = norito::to_bytes(&payload)
        .map_err(|err| ZkAceProverError::new(format!("encode ZK-ACE verifying key: {err}")))?;
    Ok(VerifyingKeyBox::new(
        ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND.into(),
        bytes,
    ))
}

/// Build an active verifier-key record suitable for registering the ZK-ACE v0 verifier.
pub fn zk_ace_verifying_key_record_v1(version: u32) -> Result<VerifyingKeyRecord> {
    let key = zk_ace_verifying_key_box_v1()?;
    let commitment = iroha_core::zk::hash_vk(&key);
    let mut record = VerifyingKeyRecord::new(
        version,
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        BackendTag::Stark,
        "goldilocks",
        zk_ace_public_inputs_schema_hash_v1(),
        commitment,
    );
    record.namespace = "zk-ace".to_owned();
    record.vk_len = u32::try_from(key.bytes.len()).unwrap_or(u32::MAX);
    record.max_proof_bytes = 256 * 1024;
    record.gas_schedule_id = Some("zk_ace_stark_default".to_owned());
    record.key = Some(key);
    record.status = ConfidentialStatus::Active;
    Ok(record)
}

/// Commitment of the canonical bundled ZK-ACE v0 verifier key.
pub fn zk_ace_verifying_key_commitment_v1() -> Result<[u8; 32]> {
    let key = zk_ace_verifying_key_box_v1()?;
    Ok(iroha_core::zk::hash_vk(&key))
}

/// Verify that the private witness matches the declared public inputs.
pub fn validate_zk_ace_witness_v1(
    public_inputs: &ZkAcePublicInputsV1,
    witness: &ZkAceWitnessV1,
) -> Result<()> {
    if witness.identity_root == [0u8; 32] {
        return Err(ZkAceProverError::new(
            "identity root witness must be nonzero",
        ));
    }
    if witness.identity_blinding == [0u8; 32] {
        return Err(ZkAceProverError::new(
            "identity blinding witness must be nonzero",
        ));
    }
    if witness.replay_secret == [0u8; 32] {
        return Err(ZkAceProverError::new(
            "replay secret witness must be nonzero",
        ));
    }
    if public_inputs.identity_commitment == [0u8; 32] {
        return Err(ZkAceProverError::new("identity commitment must be nonzero"));
    }
    if public_inputs.replay_nullifier == [0u8; 32] {
        return Err(ZkAceProverError::new("replay nullifier must be nonzero"));
    }
    if public_inputs.policy_hash == [0u8; 32] {
        return Err(ZkAceProverError::new("policy hash must be nonzero"));
    }
    if public_inputs.domain_tag.trim().is_empty() || public_inputs.action_class.trim().is_empty() {
        return Err(ZkAceProverError::new(
            "domain_tag and action_class are required",
        ));
    }
    let identity_commitment = derive_zk_ace_identity_commitment(
        &witness.identity_root,
        &witness.identity_blinding,
        &public_inputs.domain_tag,
    );
    if identity_commitment != public_inputs.identity_commitment {
        return Err(ZkAceProverError::new(
            "identity commitment witness mismatch",
        ));
    }
    let replay_nullifier = derive_zk_ace_replay_nullifier(
        &witness.replay_secret,
        &public_inputs.tx_digest,
        &public_inputs.chain_id,
        &public_inputs.action_class,
        &public_inputs.domain_tag,
    );
    if replay_nullifier != public_inputs.replay_nullifier {
        return Err(ZkAceProverError::new("replay nullifier witness mismatch"));
    }
    let tx_digest = derive_zk_ace_transfer_digest(
        &public_inputs.from,
        &public_inputs.to,
        &public_inputs.asset,
        public_inputs.amount,
        &public_inputs.chain_id,
        &public_inputs.action_class,
        &public_inputs.policy_hash,
    );
    if tx_digest != public_inputs.tx_digest {
        return Err(ZkAceProverError::new(
            "tx_digest does not match public transfer fields",
        ));
    }
    Ok(())
}

/// Build a STARK/FRI-backed ZK-ACE authorization proof attachment.
pub fn build_zk_ace_authorization_proof_v1(
    public_inputs: &ZkAcePublicInputsV1,
    witness: &ZkAceWitnessV1,
    vk_commitment: [u8; 32],
) -> Result<ProofAttachment> {
    validate_zk_ace_witness_v1(public_inputs, witness)?;
    if public_inputs.verifier_key_id.backend.as_str() != ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND {
        return Err(ZkAceProverError::new(
            "verifier key id must use stark/fri/sha256-goldilocks",
        ));
    }
    let vk_box = zk_ace_verifying_key_box_v1()?;
    let expected_vk_commitment = iroha_core::zk::hash_vk(&vk_box);
    if vk_commitment != expected_vk_commitment {
        return Err(ZkAceProverError::new(
            "verifier key commitment does not match bundled ZK-ACE verifier",
        ));
    }
    let proof_box = iroha_core::zk::prove_stark_fri_zk_ace_open_verify_envelope(
        ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        &vk_box,
        public_inputs,
        witness,
    )
    .map_err(|err| ZkAceProverError::new(format!("prove STARK/FRI: {err}")))?;
    let mut attachment = ProofAttachment::new_ref(
        ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND.into(),
        proof_box,
        public_inputs.verifier_key_id.clone(),
    );
    attachment.vk_commitment = Some(vk_commitment);
    Ok(attachment)
}

/// Build the canonical public inputs and proof for a transparent transfer.
#[allow(clippy::too_many_arguments)]
pub fn build_zk_ace_transfer_authorization_v1(
    from: AccountId,
    to: AccountId,
    asset: AssetDefinitionId,
    amount: u128,
    chain_id: ChainId,
    witness: ZkAceWitnessV1,
    policy_hash: [u8; 32],
    verifier_key_id: VerifyingKeyId,
    vk_commitment: [u8; 32],
) -> Result<ZkAceTransferAuthorizationV1> {
    if amount == 0 {
        return Err(ZkAceProverError::new("amount must be greater than zero"));
    }
    if policy_hash == [0u8; 32] {
        return Err(ZkAceProverError::new("policy hash must be nonzero"));
    }
    let identity_commitment = derive_zk_ace_identity_commitment(
        &witness.identity_root,
        &witness.identity_blinding,
        ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
    );
    let tx_digest = derive_zk_ace_transfer_digest(
        &from,
        &to,
        &asset,
        amount,
        &chain_id,
        ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
        &policy_hash,
    );
    let replay_nullifier = derive_zk_ace_replay_nullifier(
        &witness.replay_secret,
        &tx_digest,
        &chain_id,
        ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
        ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
    );
    let public_inputs = ZkAcePublicInputsV1::transparent_transfer(
        identity_commitment,
        tx_digest,
        chain_id,
        replay_nullifier,
        policy_hash,
        from,
        to,
        asset,
        amount,
        verifier_key_id,
    );
    let public_inputs_bytes = norito::to_bytes(&public_inputs)
        .map_err(|err| ZkAceProverError::new(format!("encode public inputs: {err}")))?;
    let proof = build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)?;
    Ok(ZkAceTransferAuthorizationV1 {
        public_inputs,
        proof,
        public_inputs_bytes,
    })
}

/// Convenience constructor for the v0 verifier id.
#[must_use]
pub fn zk_ace_verifier_key_id(name: impl Into<String>) -> VerifyingKeyId {
    VerifyingKeyId::new(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        domain::prelude::DomainId,
        name::Name,
        zk::{
            OpenVerifyEnvelope, StarkFriOpenProofV1, ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG, derive_zk_ace_air_public_digest,
            derive_zk_ace_public_inputs_digest,
        },
    };
    use std::str::FromStr;

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset() -> AssetDefinitionId {
        asset_named("xor")
    }

    fn asset_named(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            Name::from_str(name).expect("asset name"),
        )
    }

    fn witness(seed: u8) -> ZkAceWitnessV1 {
        ZkAceWitnessV1 {
            identity_root: [seed; 32],
            identity_blinding: [seed.wrapping_add(1); 32],
            replay_secret: [seed.wrapping_add(2); 32],
        }
    }

    fn structured_bytes(seed: u8, step: u8) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = seed.wrapping_add((index as u8).wrapping_mul(step));
        }
        out
    }

    fn structured_witness(seed: u8) -> ZkAceWitnessV1 {
        ZkAceWitnessV1 {
            identity_root: structured_bytes(seed, 3),
            identity_blinding: structured_bytes(seed ^ 0x5a, 5),
            replay_secret: structured_bytes(seed.wrapping_add(0x33), 7),
        }
    }

    fn public_inputs_for(witness: &ZkAceWitnessV1) -> ZkAcePublicInputsV1 {
        let from = account(1);
        let to = account(2);
        let asset = asset();
        let chain_id = ChainId::from_str("taira").expect("chain id");
        let policy_hash = [0x55; 32];
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = derive_zk_ace_transfer_digest(
            &from,
            &to,
            &asset,
            25,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            &policy_hash,
        );
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &tx_digest,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        ZkAcePublicInputsV1::transparent_transfer(
            identity_commitment,
            tx_digest,
            chain_id,
            replay_nullifier,
            policy_hash,
            from,
            to,
            asset,
            25,
            zk_ace_verifier_key_id(ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID),
        )
    }

    fn verify_attachment(proof: &ProofAttachment) -> bool {
        let vk = zk_ace_verifying_key_box_v1().expect("vk");
        iroha_core::zk::verify_backend(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND, &proof.proof, Some(&vk))
    }

    fn lower_level_proof_verifies(
        public_inputs: &ZkAcePublicInputsV1,
        witness: &ZkAceWitnessV1,
    ) -> bool {
        let vk = zk_ace_verifying_key_box_v1().expect("vk");
        let proof = iroha_core::zk::prove_stark_fri_zk_ace_open_verify_envelope(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
            &vk,
            public_inputs,
            witness,
        )
        .expect("lower-level prover emits an envelope");
        iroha_core::zk::verify_backend(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND, &proof, Some(&vk))
    }

    fn tamper_inner_stark(
        mut proof: ProofAttachment,
        mutate: impl FnOnce(&mut iroha_core::zk_stark::StarkVerifyEnvelopeV1),
    ) -> ProofAttachment {
        let mut envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).expect("open verify envelope");
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&envelope.proof_bytes).expect("stark wrapper");
        let mut inner: iroha_core::zk_stark::StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&open.envelope_bytes).expect("inner STARK envelope");
        mutate(&mut inner);
        open.envelope_bytes = norito::to_bytes(&inner).expect("encode inner STARK envelope");
        envelope.proof_bytes = norito::to_bytes(&open).expect("encode STARK wrapper");
        proof.proof.bytes = norito::to_bytes(&envelope).expect("encode open verify envelope");
        proof
    }

    fn tamper_outer_public_inputs(
        mut proof: ProofAttachment,
        mutate: impl FnOnce(&mut ZkAcePublicInputsV1),
    ) -> ProofAttachment {
        let mut envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).expect("open verify envelope");
        let mut public_inputs: ZkAcePublicInputsV1 =
            norito::decode_from_bytes(&envelope.public_inputs).expect("public inputs");
        mutate(&mut public_inputs);
        envelope.public_inputs = norito::to_bytes(&public_inputs).expect("encode public inputs");
        proof.proof.bytes = norito::to_bytes(&envelope).expect("encode open verify envelope");
        proof
    }

    fn assert_public_input_mutation_rejected(
        label: &str,
        proof: &ProofAttachment,
        mutate: impl FnOnce(&mut ZkAcePublicInputsV1),
    ) {
        let tampered = tamper_outer_public_inputs(proof.clone(), mutate);
        assert!(
            !verify_attachment(&tampered),
            "{label} mutation must be rejected by the STARK verifier"
        );
    }

    fn field_sub(lhs: u64, rhs: u64) -> u64 {
        const MOD_P: u128 = (1u128 << 64) - (1u128 << 32) + 1;
        if lhs >= rhs {
            lhs - rhs
        } else {
            ((u128::from(lhs) + MOD_P) - u128::from(rhs)) as u64
        }
    }

    fn bytes_from_zk_ace_limbs(limbs: &[u64]) -> [u8; 32] {
        let limbs: &[u64; 5] = limbs.try_into().expect("five limbs");
        let mut out = [0u8; 32];
        let mut offset = 0usize;
        for (index, limb) in limbs.iter().enumerate() {
            let bytes = limb.to_le_bytes();
            let take = if index == 4 { 4 } else { 7 };
            out[offset..offset + take].copy_from_slice(&bytes[..take]);
            offset += take;
        }
        out
    }

    fn recover_witness_from_opened_zk_ace_row(row: &[u64]) -> ZkAceWitnessV1 {
        assert_eq!(row.len(), 31, "unexpected ZK-ACE AIR width");
        let limbs = (0..15)
            .map(|limb_index| field_sub(row[1 + limb_index], row[16 + limb_index]))
            .collect::<Vec<_>>();
        ZkAceWitnessV1 {
            identity_root: bytes_from_zk_ace_limbs(&limbs[..5]),
            identity_blinding: bytes_from_zk_ace_limbs(&limbs[5..10]),
            replay_secret: bytes_from_zk_ace_limbs(&limbs[10..15]),
        }
    }

    fn inner_stark_envelope(
        proof: &ProofAttachment,
    ) -> iroha_core::zk_stark::StarkVerifyEnvelopeV1 {
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).expect("open verify envelope");
        let open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&envelope.proof_bytes).expect("stark wrapper");
        norito::decode_from_bytes(&open.envelope_bytes).expect("inner STARK envelope")
    }

    #[test]
    fn transfer_authorization_derives_canonical_fields_and_verifies() {
        let witness = witness(0x11);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let proof = build_zk_ace_transfer_authorization_v1(
            account(1),
            account(2),
            asset(),
            25,
            ChainId::from_str("taira").expect("chain id"),
            witness.clone(),
            [0x55; 32],
            zk_ace_verifier_key_id(ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID),
            vk_commitment,
        )
        .expect("build transfer authorization");

        let expected = public_inputs_for(&witness);
        assert_eq!(
            proof.public_inputs.identity_commitment,
            expected.identity_commitment
        );
        assert_eq!(proof.public_inputs.tx_digest, expected.tx_digest);
        assert_eq!(
            proof.public_inputs.replay_nullifier,
            expected.replay_nullifier
        );
        assert_eq!(
            proof.public_inputs.domain_tag,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG
        );
        assert_eq!(
            proof.public_inputs.action_class,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER
        );
        assert_eq!(
            proof.public_inputs_bytes,
            norito::to_bytes(&proof.public_inputs).expect("public inputs bytes")
        );
        assert_eq!(proof.proof.vk_commitment, Some(vk_commitment));
        let vk = zk_ace_verifying_key_box_v1().expect("vk");
        assert!(iroha_core::zk::verify_backend(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            &proof.proof.proof,
            Some(&vk),
        ));
    }

    #[test]
    fn proof_envelope_binds_public_inputs_circuit_and_vk_commitment() {
        let witness = witness(0x21);
        let public_inputs = public_inputs_for(&witness);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let proof = build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
            .expect("build proof");

        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).expect("open verify envelope");
        assert_eq!(envelope.backend, BackendTag::Stark);
        assert_eq!(envelope.circuit_id, ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID);
        assert_eq!(envelope.vk_hash, vk_commitment);
        assert_eq!(
            envelope.public_inputs,
            norito::to_bytes(&public_inputs).expect("public input bytes")
        );
        let open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&envelope.proof_bytes).expect("stark wrapper");
        assert_eq!(open.version, 1);
        assert_eq!(
            open.public_inputs,
            vec![vec![
                derive_zk_ace_public_inputs_digest(&public_inputs).expect("public input digest")
            ]]
        );
        let inner: iroha_core::zk_stark::StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&open.envelope_bytes).expect("inner STARK envelope");
        let air = inner.proof.air.as_ref().expect("ZK-ACE AIR section");
        assert_eq!(
            air.public_digest,
            derive_zk_ace_air_public_digest(&public_inputs).expect("AIR public digest")
        );
        assert!(
            iroha_core::zk_stark::verify_stark_fri_zk_ace_envelope_with_limits(
                &open.envelope_bytes,
                &iroha_core::zk_stark::StarkVerifierLimits::default(),
                &public_inputs,
            )
        );
    }

    #[test]
    fn proof_bytes_do_not_contain_raw_witness_values() {
        let witness = witness(0x24);
        let public_inputs = public_inputs_for(&witness);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let proof = build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
            .expect("build proof");

        for raw in [
            witness.identity_root.as_slice(),
            witness.identity_blinding.as_slice(),
            witness.replay_secret.as_slice(),
        ] {
            assert!(
                !proof
                    .proof
                    .bytes
                    .windows(raw.len())
                    .any(|window| window == raw),
                "proof bytes leaked a raw witness field"
            );
        }
        for raw in [
            witness.identity_root,
            witness.identity_blinding,
            witness.replay_secret,
        ] {
            let packed = iroha_data_model::zk::zk_ace_pack_bytes_to_field_limbs(&raw);
            for limb in packed.limbs {
                let limb_bytes = limb.to_le_bytes();
                assert!(
                    !proof
                        .proof
                        .bytes
                        .windows(limb_bytes.len())
                        .any(|window| window == limb_bytes),
                    "proof bytes leaked an unmasked witness limb"
                );
            }
        }
    }

    #[test]
    fn zk_ace_air_openings_do_not_recover_private_witness() {
        for witness in [
            witness(0x24),
            structured_witness(0x35),
            structured_witness(0x7d),
        ] {
            let public_inputs = public_inputs_for(&witness);
            let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
            let proof =
                build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
                    .expect("build proof");
            let inner = inner_stark_envelope(&proof);
            let air = inner.proof.air.as_ref().expect("ZK-ACE AIR section");
            assert!(
                !air.openings.is_empty(),
                "ZK-ACE proof must carry AIR openings"
            );
            let domain = 1usize << inner.params.n_log2;

            for opening in &air.openings {
                let opened_index = opening.index as usize;
                assert_ne!(
                    opened_index, 0,
                    "ZK-ACE AIR must not open the private witness row"
                );
                assert_ne!(
                    (opened_index + 1) % domain,
                    0,
                    "ZK-ACE AIR must not open the private witness row as next_row"
                );

                assert_ne!(
                    recover_witness_from_opened_zk_ace_row(&opening.row),
                    witness,
                    "ZK-ACE AIR row exposes enough information to recover the witness"
                );
                assert_ne!(
                    recover_witness_from_opened_zk_ace_row(&opening.next_row),
                    witness,
                    "ZK-ACE next AIR row exposes enough information to recover the witness"
                );
            }
        }
    }

    #[test]
    fn verifier_rejects_tampered_zk_ace_air_openings() {
        let witness = witness(0x25);
        let public_inputs = public_inputs_for(&witness);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let proof = build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
            .expect("build proof");
        assert!(verify_attachment(&proof));

        let row_tampered = tamper_inner_stark(proof.clone(), |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.openings[0].row[1] ^= 1;
        });
        assert!(!verify_attachment(&row_tampered));

        let next_row_tampered = tamper_inner_stark(proof.clone(), |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.openings[0].next_row[2] ^= 1;
        });
        assert!(!verify_attachment(&next_row_tampered));

        let composition_tampered = tamper_inner_stark(proof, |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.openings[0].composition_value ^= 1;
        });
        assert!(!verify_attachment(&composition_tampered));
    }

    #[test]
    fn verifier_rejects_tampered_zk_ace_air_bindings() {
        let witness = witness(0x27);
        let public_inputs = public_inputs_for(&witness);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let proof = build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
            .expect("build proof");
        assert!(verify_attachment(&proof));

        let circuit_tampered = tamper_inner_stark(proof.clone(), |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.circuit_id.push_str(":wrong");
        });
        assert!(!verify_attachment(&circuit_tampered));

        let public_digest_tampered = tamper_inner_stark(proof.clone(), |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.public_digest[0] ^= 1;
        });
        assert!(!verify_attachment(&public_digest_tampered));

        let trace_root_tampered = tamper_inner_stark(proof.clone(), |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.trace_root[0] ^= 1;
        });
        assert!(!verify_attachment(&trace_root_tampered));

        let composition_root_tampered = tamper_inner_stark(proof.clone(), |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.composition_root[0] ^= 1;
        });
        assert!(!verify_attachment(&composition_root_tampered));

        let trace_width_tampered = tamper_inner_stark(proof.clone(), |inner| {
            let air = inner.proof.air.as_mut().expect("AIR section");
            air.trace_width += 1;
        });
        assert!(!verify_attachment(&trace_width_tampered));

        let domain_tampered = tamper_inner_stark(proof.clone(), |inner| {
            inner.params.domain_tag.push_str(":wrong");
        });
        assert!(!verify_attachment(&domain_tampered));

        let query_root_tampered = tamper_inner_stark(proof, |inner| {
            inner.proof.commits.roots[0][0] ^= 1;
        });
        assert!(!verify_attachment(&query_root_tampered));
    }

    #[test]
    fn verifier_rejects_tampered_zk_ace_public_bindings() {
        let witness = witness(0x26);
        let public_inputs = public_inputs_for(&witness);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let proof = build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
            .expect("build proof");
        assert!(verify_attachment(&proof));

        let amount_tampered = tamper_outer_public_inputs(proof.clone(), |public_inputs| {
            public_inputs.amount += 1;
        });
        assert!(!verify_attachment(&amount_tampered));

        let policy_tampered = tamper_outer_public_inputs(proof.clone(), |public_inputs| {
            public_inputs.policy_hash[0] ^= 1;
        });
        assert!(!verify_attachment(&policy_tampered));

        let domain_tampered = tamper_outer_public_inputs(proof, |public_inputs| {
            public_inputs.domain_tag.push_str(":evil");
        });
        assert!(!verify_attachment(&domain_tampered));
    }

    #[test]
    fn verifier_rejects_reproved_mismatched_zk_ace_witness() {
        let valid_witness = witness(0x2a);
        let wrong_witness = witness(0x2b);
        let public_inputs = public_inputs_for(&valid_witness);
        let vk = zk_ace_verifying_key_box_v1().expect("vk");
        let proof = iroha_core::zk::prove_stark_fri_zk_ace_open_verify_envelope(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
            &vk,
            &public_inputs,
            &wrong_witness,
        )
        .expect("lower-level prover still emits a malformed-witness envelope");

        assert!(
            !iroha_core::zk::verify_backend(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND, &proof, Some(&vk)),
            "native STARK verifier must reject a re-proved envelope whose witness does not match public inputs"
        );
    }

    #[test]
    fn verifier_rejects_zero_private_witness_fields_without_host_preflight() {
        let valid_witness = witness(0x2c);
        let valid_public_inputs = public_inputs_for(&valid_witness);
        assert!(
            lower_level_proof_verifies(&valid_public_inputs, &valid_witness),
            "baseline lower-level proof must verify before adversarial zero-witness cases"
        );

        for (label, zero_witness) in [
            (
                "identity root",
                ZkAceWitnessV1 {
                    identity_root: [0; 32],
                    identity_blinding: [0x2d; 32],
                    replay_secret: [0x2e; 32],
                },
            ),
            (
                "identity blinding",
                ZkAceWitnessV1 {
                    identity_root: [0x2f; 32],
                    identity_blinding: [0; 32],
                    replay_secret: [0x30; 32],
                },
            ),
            (
                "replay secret",
                ZkAceWitnessV1 {
                    identity_root: [0x31; 32],
                    identity_blinding: [0x32; 32],
                    replay_secret: [0; 32],
                },
            ),
        ] {
            let public_inputs = public_inputs_for(&zero_witness);
            assert!(
                !lower_level_proof_verifies(&public_inputs, &zero_witness),
                "native STARK verifier must enforce nonzero {label} without relying on host preflight"
            );
        }
    }

    #[test]
    fn verifier_rejects_every_zk_ace_public_input_field_mutation() {
        let witness = witness(0x28);
        let public_inputs = public_inputs_for(&witness);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let proof = build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
            .expect("build proof");
        assert!(verify_attachment(&proof));

        assert_public_input_mutation_rejected("identity commitment", &proof, |public_inputs| {
            public_inputs.identity_commitment[0] ^= 1;
        });
        assert_public_input_mutation_rejected("tx digest", &proof, |public_inputs| {
            public_inputs.tx_digest[0] ^= 1;
        });
        assert_public_input_mutation_rejected("chain id", &proof, |public_inputs| {
            public_inputs.chain_id = ChainId::from_str("minamoto").expect("chain id");
        });
        assert_public_input_mutation_rejected("replay nullifier", &proof, |public_inputs| {
            public_inputs.replay_nullifier[0] ^= 1;
        });
        assert_public_input_mutation_rejected("policy hash", &proof, |public_inputs| {
            public_inputs.policy_hash[0] ^= 1;
        });
        assert_public_input_mutation_rejected("domain tag", &proof, |public_inputs| {
            public_inputs.domain_tag.push_str(":wrong");
        });
        assert_public_input_mutation_rejected("action class", &proof, |public_inputs| {
            public_inputs.action_class.push_str(":wrong");
        });
        assert_public_input_mutation_rejected("source account", &proof, |public_inputs| {
            public_inputs.from = account(3);
        });
        assert_public_input_mutation_rejected("destination account", &proof, |public_inputs| {
            public_inputs.to = account(4);
        });
        assert_public_input_mutation_rejected("asset", &proof, |public_inputs| {
            public_inputs.asset = asset_named("rose");
        });
        assert_public_input_mutation_rejected("amount", &proof, |public_inputs| {
            public_inputs.amount += 1;
        });
        assert_public_input_mutation_rejected("verifier key id", &proof, |public_inputs| {
            public_inputs.verifier_key_id =
                VerifyingKeyId::new(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND, "wrong_circuit");
        });
    }

    #[test]
    fn verifier_rejects_corrupted_zk_ace_proof_bytes() {
        let witness = witness(0x29);
        let public_inputs = public_inputs_for(&witness);
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let mut proof =
            build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
                .expect("build proof");
        assert!(verify_attachment(&proof));

        let mid = proof.proof.bytes.len() / 2;
        proof.proof.bytes[mid] ^= 1;
        assert!(!verify_attachment(&proof));
    }

    #[test]
    fn witness_validation_rejects_mismatched_private_identity() {
        let valid_witness = witness(0x31);
        let mut public_inputs = public_inputs_for(&valid_witness);
        let wrong = witness(0x41);
        public_inputs.identity_commitment = derive_zk_ace_identity_commitment(
            &wrong.identity_root,
            &wrong.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let err =
            validate_zk_ace_witness_v1(&public_inputs, &valid_witness).expect_err("must reject");
        assert!(err.to_string().contains("identity commitment"));
    }

    #[test]
    fn witness_validation_rejects_replay_secret_substitution() {
        let witness = witness(0x51);
        let mut public_inputs = public_inputs_for(&witness);
        public_inputs.replay_nullifier = derive_zk_ace_replay_nullifier(
            &[0x99; 32],
            &public_inputs.tx_digest,
            &public_inputs.chain_id,
            &public_inputs.action_class,
            &public_inputs.domain_tag,
        );
        let err = validate_zk_ace_witness_v1(&public_inputs, &witness).expect_err("must reject");
        assert!(err.to_string().contains("replay nullifier"));
    }

    #[test]
    fn witness_validation_rejects_action_field_substitution() {
        let witness = witness(0x61);
        let mut public_inputs = public_inputs_for(&witness);
        public_inputs.amount += 1;
        let err = validate_zk_ace_witness_v1(&public_inputs, &witness).expect_err("must reject");
        assert!(err.to_string().contains("tx_digest"));
    }

    #[test]
    fn proof_builder_rejects_wrong_backend_verifier_key() {
        let witness = witness(0x71);
        let mut public_inputs = public_inputs_for(&witness);
        public_inputs.verifier_key_id = VerifyingKeyId::new("halo2/ipa", "zk_ace");
        let err = build_zk_ace_authorization_proof_v1(
            &public_inputs,
            &witness,
            zk_ace_verifying_key_commitment_v1().expect("vk commitment"),
        )
        .expect_err("must reject");
        assert!(err.to_string().contains("stark/fri/sha256-goldilocks"));
    }

    #[test]
    fn transfer_builder_rejects_zero_policy_amount_and_witness_fields() {
        let vk_commitment = zk_ace_verifying_key_commitment_v1().expect("vk commitment");
        let verifier = zk_ace_verifier_key_id(ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID);
        let base = || {
            (
                account(1),
                account(2),
                asset(),
                ChainId::from_str("taira").expect("chain id"),
            )
        };

        let (from, to, asset, chain_id) = base();
        let err = build_zk_ace_transfer_authorization_v1(
            from,
            to,
            asset,
            0,
            chain_id,
            witness(0x81),
            [0x55; 32],
            verifier.clone(),
            vk_commitment,
        )
        .expect_err("zero amount rejected");
        assert!(err.to_string().contains("amount"));

        let (from, to, asset, chain_id) = base();
        let err = build_zk_ace_transfer_authorization_v1(
            from,
            to,
            asset,
            1,
            chain_id,
            witness(0x82),
            [0u8; 32],
            verifier.clone(),
            vk_commitment,
        )
        .expect_err("zero policy rejected");
        assert!(err.to_string().contains("policy hash"));

        let (from, to, asset, chain_id) = base();
        let err = build_zk_ace_transfer_authorization_v1(
            from,
            to,
            asset,
            1,
            chain_id,
            ZkAceWitnessV1 {
                identity_root: [0; 32],
                identity_blinding: [1; 32],
                replay_secret: [2; 32],
            },
            [0x55; 32],
            verifier,
            vk_commitment,
        )
        .expect_err("zero witness rejected");
        assert!(err.to_string().contains("identity root"));
    }

    #[test]
    fn verifier_key_record_is_active_and_bound_to_zk_ace() {
        let record = zk_ace_verifying_key_record_v1(3).expect("record");
        assert_eq!(record.version, 3);
        assert_eq!(record.namespace, "zk-ace");
        assert_eq!(record.circuit_id, ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID);
        assert_eq!(record.backend, BackendTag::Stark);
        assert_eq!(record.status, ConfidentialStatus::Active);
        assert_eq!(record.max_proof_bytes, 256 * 1024);
        assert_eq!(
            record.commitment,
            zk_ace_verifying_key_commitment_v1().expect("vk commitment")
        );
        assert!(record.key.is_some());
    }
}
