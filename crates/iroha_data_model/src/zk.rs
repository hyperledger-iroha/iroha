//! Zero-knowledge envelope types (Norito TLV payloads).

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{AssetDefinitionId, ChainId, account::AccountId, proof::VerifyingKeyId};

/// Canonical ZK-ACE circuit identifier for post-quantum authorization v0.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID: &str = "zk_ace_pq_authorization_v0";

/// Production backend label used by ZK-ACE authorization v0.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND: &str = "stark/fri/sha256-goldilocks";

/// Domain tag used when deriving ZK-ACE identity commitments and replay nullifiers.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG: &str = "iroha:zk-ace:pq-authorization:v0";

/// First executable ZK-ACE action class.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER: &str = "transparent_asset_transfer";

/// Maximum source accounts that one ZK-ACE identity commitment may authorize.
pub const ZK_ACE_MAX_ALLOWED_ACCOUNTS: usize = 16;

/// Number of bytes packed into each Goldilocks field limb for ZK-ACE hashes.
pub const ZK_ACE_PACKED_LIMB_BYTES: usize = 7;

/// Backend tag for zero-knowledge verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
pub enum BackendTag {
    /// Halo2 IPA over Pasta curves
    Halo2IpaPasta,
    /// Halo2 over BN254 (optional)
    Halo2Bn254,
    /// Groth16 (stub)
    Groth16,
    /// STARK/FRI (transparent, no trusted setup)
    Stark,
    /// Unknown/unsupported backend
    Unsupported,
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for BackendTag {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            BackendTag::Halo2IpaPasta => "halo2-ipa-pasta",
            BackendTag::Halo2Bn254 => "halo2-bn254",
            BackendTag::Groth16 => "groth16",
            BackendTag::Stark => "stark",
            BackendTag::Unsupported => "unsupported",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for BackendTag {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        let tag = match value.as_str() {
            "halo2-ipa-pasta" => BackendTag::Halo2IpaPasta,
            "halo2-bn254" => BackendTag::Halo2Bn254,
            "groth16" => BackendTag::Groth16,
            "stark" => BackendTag::Stark,
            _ => BackendTag::Unsupported,
        };
        Ok(tag)
    }
}

/// Envelope for open-verify operations (canonical `SignedQuery` layout).
///
/// This structure is serialized with Norito and used as the TLV payload for
/// `&NoritoBytes` pointer-ABI types passed to IVM verify syscalls or host vendor bridges.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OpenVerifyEnvelope {
    /// Backend tag string (e.g., `halo2-ipa-pasta`).
    pub backend: BackendTag,
    /// Circuit identifier string (backend-specific; opaque to host).
    pub circuit_id: String,
    /// Domain-separated verifying-key hash.
    ///
    /// Generic codecs may still represent an unavailable key binding as all
    /// zeros, but chain admission for registered proof attachments requires an
    /// exact match with the active verifier-key commitment.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub vk_hash: [u8; 32],
    /// Public-input metadata bytes (opaque; backend-specific canonical encoding).
    ///
    /// For backends that separate schema from values (e.g., `stark/fri` wrappers),
    /// this field carries the stable schema descriptor while concrete values are
    /// stored inside backend-specific payloads.
    pub public_inputs: Vec<u8>,
    /// Proof bytes (opaque, backend-specific canonical encoding).
    pub proof_bytes: Vec<u8>,
    /// Opaque aux map encoded as JSON bytes (for small structured extras).
    ///
    /// Production chain proof-admission paths require this to be empty unless a
    /// future instruction explicitly defines and validates auxiliary semantics.
    pub aux: Vec<u8>,
}

impl OpenVerifyEnvelope {
    /// Create a new envelope with required fields; `aux` defaults to empty.
    pub fn new(
        backend: BackendTag,
        circuit_id: impl Into<String>,
        vk_hash: [u8; 32],
        public_inputs: Vec<u8>,
        proof_bytes: Vec<u8>,
    ) -> Self {
        Self {
            backend,
            circuit_id: circuit_id.into(),
            vk_hash,
            public_inputs,
            proof_bytes,
            aux: Vec::new(),
        }
    }
}

// Note: Norito serialization is derived via `Encode`/`Decode` (packed structs compatible)

/// STARK/FRI proof payload embedded inside [`OpenVerifyEnvelope::proof_bytes`] when
/// [`OpenVerifyEnvelope::backend`] is [`BackendTag::Stark`].
///
/// This wrapper carries:
/// - `public_inputs`: public inputs expressed as 32-byte words, column-major (matching
///   the instance-column layout used by Halo2 envelopes), and
/// - `envelope_bytes`: backend-native proof bytes (typically a Norito-encoded STARK/FRI
///   envelope such as `StarkVerifyEnvelopeV1`).
///
/// Higher-level flows (governance voting, `Executable::IvmProved`, etc.) interpret the public
/// inputs according to the circuit/policy definitions and must validate their semantics.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct StarkFriOpenProofV1 {
    /// Version tag for format evolution.
    pub version: u16,
    /// Public inputs encoded as 32-byte words, column-major.
    pub public_inputs: Vec<Vec<[u8; 32]>>,
    /// Backend-native proof envelope bytes.
    pub envelope_bytes: Vec<u8>,
}

/// Canonical public input record proven by `zk_ace_pq_authorization_v0`.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ZkAcePublicInputsV1 {
    /// Public-input schema version.
    pub version: u16,
    /// On-chain identity commitment being authorized.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub identity_commitment: [u8; 32],
    /// Digest of the visible action fields.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub tx_digest: [u8; 32],
    /// Chain id bound into the replay-nullifier domain.
    pub chain_id: ChainId,
    /// Domain separation tag.
    pub domain_tag: String,
    /// Action class authorized by this proof.
    pub action_class: String,
    /// Replay-prevention nullifier derived inside the circuit.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub replay_nullifier: [u8; 32],
    /// Policy hash bound to the registered identity record.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_hash: [u8; 32],
    /// Source account whose transfer authority is proven.
    pub from: AccountId,
    /// Destination account.
    pub to: AccountId,
    /// Transparent asset definition being transferred.
    pub asset: AssetDefinitionId,
    /// Transparent amount being transferred.
    pub amount: u128,
    /// Verifier key that must validate the proof.
    pub verifier_key_id: VerifyingKeyId,
}

/// Private witness used by the ZK-ACE prover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ZkAceWitnessV1 {
    /// External DIDP identity root.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub identity_root: [u8; 32],
    /// Identity blinding factor.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub identity_blinding: [u8; 32],
    /// Replay secret used to derive per-action nullifiers.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub replay_secret: [u8; 32],
}

/// Canonical byte packing used by ZK-ACE Poseidon2-domain hashing.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ZkAcePackedBytesV1 {
    /// Original byte length before padding.
    pub length: u64,
    /// Little-endian 7-byte Goldilocks limbs.
    pub limbs: Vec<u64>,
}

impl ZkAcePublicInputsV1 {
    /// Construct v1 public inputs for the transparent-transfer action.
    #[allow(clippy::too_many_arguments)]
    pub fn transparent_transfer(
        identity_commitment: [u8; 32],
        tx_digest: [u8; 32],
        chain_id: ChainId,
        replay_nullifier: [u8; 32],
        policy_hash: [u8; 32],
        from: AccountId,
        to: AccountId,
        asset: AssetDefinitionId,
        amount: u128,
        verifier_key_id: VerifyingKeyId,
    ) -> Self {
        Self {
            version: 1,
            identity_commitment,
            tx_digest,
            chain_id,
            domain_tag: ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            action_class: ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            replay_nullifier,
            policy_hash,
            from,
            to,
            asset,
            amount,
            verifier_key_id,
        }
    }
}

/// Pack arbitrary bytes into canonical 7-byte Goldilocks limbs.
#[must_use]
pub fn zk_ace_pack_bytes_to_field_limbs(bytes: &[u8]) -> ZkAcePackedBytesV1 {
    let mut limbs = Vec::with_capacity(bytes.len().div_ceil(ZK_ACE_PACKED_LIMB_BYTES));
    let mut offset = 0usize;
    while offset < bytes.len() {
        let take = core::cmp::min(ZK_ACE_PACKED_LIMB_BYTES, bytes.len() - offset);
        let mut chunk = [0u8; 8];
        chunk[..take].copy_from_slice(&bytes[offset..offset + take]);
        limbs.push(u64::from_le_bytes(chunk));
        offset += take;
    }
    ZkAcePackedBytesV1 {
        length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        limbs,
    }
}

/// Domain-separated Poseidon2 hash over already canonical byte parts.
#[must_use]
pub fn zk_ace_poseidon2_domain_hash(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let words = zk_ace_poseidon2_domain_words(domain, parts);
    let mut sponge = fastpq_isi::poseidon::PoseidonSponge::new();
    sponge.absorb_slice(&words);

    let mut out = [0u8; 32];
    for chunk in out.chunks_exact_mut(core::mem::size_of::<u64>()) {
        chunk.copy_from_slice(&sponge.squeeze_element().to_le_bytes());
    }
    out
}

/// Canonical Goldilocks field preimage used by ZK-ACE Poseidon2-domain hashing.
#[must_use]
pub fn zk_ace_poseidon2_domain_words(domain: &[u8], parts: &[&[u8]]) -> Vec<u64> {
    let mut words = Vec::new();
    let domain = zk_ace_pack_bytes_to_field_limbs(domain);
    words.push(domain.length);
    words.extend_from_slice(&domain.limbs);
    words.push(u64::try_from(parts.len()).unwrap_or(u64::MAX));
    for part in parts {
        let packed = zk_ace_pack_bytes_to_field_limbs(part);
        words.push(packed.length);
        words.extend_from_slice(&packed.limbs);
    }
    words
}

fn zk_ace_poseidon_bytes(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    zk_ace_poseidon2_domain_hash(domain, parts)
}

/// Derive a private prover-side AIR statement digest from public inputs and witness.
///
/// # Errors
///
/// Returns [`norito::Error`] if the public inputs cannot be encoded canonically.
pub fn derive_zk_ace_air_statement_digest(
    public_inputs: &ZkAcePublicInputsV1,
    witness: &ZkAceWitnessV1,
) -> Result<[u8; 32], norito::Error> {
    let public_bytes = norito::to_bytes(public_inputs)?;
    Ok(zk_ace_poseidon2_domain_hash(
        b"zk-ace.air-statement.v1",
        &[
            &public_bytes,
            &witness.identity_root,
            &witness.identity_blinding,
            &witness.replay_secret,
        ],
    ))
}

/// Derive the verifier-side public AIR word for a ZK-ACE proof.
///
/// # Errors
///
/// Returns [`norito::Error`] if the public inputs cannot be encoded canonically.
pub fn derive_zk_ace_air_public_digest(
    public_inputs: &ZkAcePublicInputsV1,
) -> Result<[u8; 32], norito::Error> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"zk-ace.air-public.v1");
    buf.extend_from_slice(&norito::to_bytes(public_inputs)?);
    Ok(zk_ace_poseidon2_domain_hash(
        b"zk-ace.air-public-digest.v1",
        &[&buf],
    ))
}

/// Derive the ZK-ACE identity commitment from its private witness components.
pub fn derive_zk_ace_identity_commitment(
    identity_root: &[u8; 32],
    identity_blinding: &[u8; 32],
    domain_tag: &str,
) -> [u8; 32] {
    zk_ace_poseidon_bytes(
        b"zk-ace.identity-commitment.v1",
        &[identity_root, identity_blinding, domain_tag.as_bytes()],
    )
}

/// Derive the ZK-ACE replay nullifier for a specific action.
pub fn derive_zk_ace_replay_nullifier(
    replay_secret: &[u8; 32],
    tx_digest: &[u8; 32],
    chain_id: &ChainId,
    action_class: &str,
    domain_tag: &str,
) -> [u8; 32] {
    zk_ace_poseidon_bytes(
        b"zk-ace.replay-nullifier.v1",
        &[
            replay_secret,
            tx_digest,
            chain_id.as_str().as_bytes(),
            action_class.as_bytes(),
            domain_tag.as_bytes(),
        ],
    )
}

/// Derive the action digest for a ZK-ACE-authorized transparent asset transfer.
pub fn derive_zk_ace_transfer_digest(
    from: &AccountId,
    to: &AccountId,
    asset: &AssetDefinitionId,
    amount: u128,
    chain_id: &ChainId,
    action_class: &str,
    policy_hash: &[u8; 32],
) -> [u8; 32] {
    zk_ace_poseidon_bytes(
        b"zk-ace.transparent-transfer.v1",
        &[
            from.to_string().as_bytes(),
            to.to_string().as_bytes(),
            asset.to_string().as_bytes(),
            &amount.to_be_bytes(),
            chain_id.as_str().as_bytes(),
            action_class.as_bytes(),
            policy_hash,
        ],
    )
}

/// Hash canonical public inputs into a STARK public-input word.
///
/// # Errors
///
/// Returns [`norito::Error`] if the public inputs cannot be encoded canonically.
pub fn derive_zk_ace_public_inputs_digest(
    public_inputs: &ZkAcePublicInputsV1,
) -> Result<[u8; 32], norito::Error> {
    let bytes = norito::to_bytes(public_inputs)?;
    Ok(zk_ace_poseidon2_domain_hash(
        b"zk-ace.public-inputs.v1",
        &[&bytes],
    ))
}

/// Stable schema hash for ZK-ACE v0 transparent-transfer public inputs.
#[must_use]
pub fn zk_ace_public_inputs_schema_hash_v1() -> [u8; 32] {
    zk_ace_poseidon2_domain_hash(
        b"zk-ace.public-inputs-schema.v1",
        &[
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.as_bytes(),
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.as_bytes(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.as_bytes(),
            b"version:u32",
            b"identity_commitment:bytes32",
            b"tx_digest:bytes32",
            b"chain_id:string",
            b"domain_tag:string",
            b"action_class:string",
            b"replay_nullifier:bytes32",
            b"policy_hash:bytes32",
            b"from:account_id",
            b"to:account_id",
            b"asset:asset_definition_id",
            b"amount:u128",
            b"verifier_key_id:verifying_key_id",
        ],
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{domain::DomainId, name::Name};

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            Name::from_str("xor").expect("asset name"),
        )
    }

    #[cfg(feature = "json")]
    fn assert_json_roundtrip<T>(value: &T)
    where
        T: PartialEq
            + core::fmt::Debug
            + norito::json::JsonSerialize
            + norito::json::JsonDeserialize,
    {
        let json = norito::json::to_json(value).expect("serialize to json");
        let decoded: T = norito::json::from_json(&json).expect("deserialize from json");
        assert_eq!(&decoded, value);
    }

    #[test]
    fn zk_ace_packing_and_hash_vectors_are_stable() {
        let packed = zk_ace_pack_bytes_to_field_limbs(b"ABCDEFGH");
        assert_eq!(packed.length, 8);
        assert_eq!(packed.limbs, vec![0x0047_4645_4443_4241, 0x48]);

        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let policy_hash = [0x44; 32];
        let chain_id: ChainId = "boi-test-chain".parse().expect("chain id");
        let from = account(1);
        let to = account(2);
        let asset = asset_definition_id();
        let verifier_key_id = VerifyingKeyId::new(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        );
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = derive_zk_ace_transfer_digest(
            &from,
            &to,
            &asset,
            17,
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
            17,
            verifier_key_id,
        );
        let public_digest =
            derive_zk_ace_public_inputs_digest(&public_inputs).expect("public digest");
        let air_public_digest =
            derive_zk_ace_air_public_digest(&public_inputs).expect("air public digest");
        let air_statement_digest = derive_zk_ace_air_statement_digest(&public_inputs, &witness)
            .expect("air statement digest");
        let schema_hash = zk_ace_public_inputs_schema_hash_v1();

        assert_eq!(
            hex::encode(identity_commitment),
            "9cb1c494eaf171b6ce218d3c7c6de88cdc8228f9b4eda310a325b4b2c1cbd68f"
        );
        assert_eq!(
            hex::encode(tx_digest),
            "f5e3f7120d12b98f65f088b419db9607d40eedfd412f767062cc4f1e18527036"
        );
        assert_eq!(
            hex::encode(replay_nullifier),
            "1ddaf81b2865d10fdc5b597f0283c675a76928bfd171eadb6410aacb971cefc1"
        );
        assert_eq!(
            hex::encode(public_digest),
            "2873792251b35ebcb9b9357b46bb38d0022dd7e6fb8091f2d5d85677bab52389"
        );
        assert_eq!(
            hex::encode(air_public_digest),
            "248c2c007fcfd20ab285bdad0490ed7b7b046001614b4d2aa4b6021d6c952bc1"
        );
        assert_eq!(
            hex::encode(air_statement_digest),
            "7c1cfdf8ec0e2a4c1a10eeca670558293c8468dd8ade96bd86bf1f95e2dc34f4"
        );
        assert_eq!(
            hex::encode(schema_hash),
            "2f265a860aa24df7d6703513fb95cb9b6323eae70203cbb32b53bd6e4fd1325c"
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn zk_ace_json_roundtrips_public_proof_witness_and_packing() {
        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let policy_hash = [0x44; 32];
        let chain_id: ChainId = "boi-test-chain".parse().expect("chain id");
        let from = account(1);
        let to = account(2);
        let asset = asset_definition_id();
        let verifier_key_id = VerifyingKeyId::new(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        );
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = derive_zk_ace_transfer_digest(
            &from,
            &to,
            &asset,
            17,
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
            17,
            verifier_key_id,
        );
        let packed = zk_ace_pack_bytes_to_field_limbs(b"ABCDEFGH");
        let open_proof = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: vec![vec![[0xAA; 32], [0xBB; 32]], vec![[0xCC; 32]]],
            envelope_bytes: vec![0x01, 0x02, 0x03, 0x04],
        };

        assert_json_roundtrip(&public_inputs);
        assert_json_roundtrip(&witness);
        assert_json_roundtrip(&packed);
        assert_json_roundtrip(&open_proof);
    }
}
