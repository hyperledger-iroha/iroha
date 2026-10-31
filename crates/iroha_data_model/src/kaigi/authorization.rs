//! Complete canonical identity bindings for first-release Kaigi authorization.
//!
//! These values are public circuit inputs, not authorization evidence. Core
//! reconstructs them from the trusted network, stored call and host, and the
//! authenticated subject. Each account binding retains all 32 native digest bytes
//! over the complete canonical Norito frame, including its controller policy.

use std::io::{self, Write};

use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use crate::{NetworkId, account::AccountId, kaigi::KaigiId};
use iroha_crypto::native_commitment::{NativeCommitmentDomainV1, NativeCommitmentFrameV1};

/// Maximum canonical Norito frame size for a Kaigi account or call identity.
pub const KAIGI_AUTHORIZATION_IDENTITY_MAX_BYTES_V1: usize = 1024 * 1024;

/// Complete native BLAKE3-256 identity, encoded as exactly 32 raw bytes.
///
/// This is a public context value, not authorization evidence. Core must
/// recompute it from the complete canonical account or call before verification.
/// Every bit is retained; words are injected into Pasta without reduction.
#[repr(transparent)]
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::kaigi::authorization::KaigiIdentityDigestV1")]
pub struct KaigiIdentityDigestV1([u8; 32]);

impl KaigiIdentityDigestV1 {
    /// Exact fixed payload width, independent of ambient Norito layout flags.
    pub const BYTES: usize = 32;

    /// Borrow every digest byte in the native hash output order.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::BYTES] {
        &self.0
    }

    /// Preserve the complete byte representation for native consumers.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; Self::BYTES] {
        self.0
    }

    /// Inject all four little-endian words; no field reduction or truncation.
    #[must_use]
    pub fn words(self) -> [u64; 4] {
        core::array::from_fn(|index| {
            let start = index * 8;
            u64::from_le_bytes(self.0[start..start + 8].try_into().expect("fixed word"))
        })
    }
}

impl norito::core::SerializePayload for KaigiIdentityDigestV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        writer.write_all(&self.0)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(Self::BYTES)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(Self::BYTES)
    }
}
impl<'de> norito::core::DeserializePayload<'de> for KaigiIdentityDigestV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical Kaigi identity decode")
    }
    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let mut offset = 0;
        let bytes = norito::core::decode_context_byte_array::<{ Self::BYTES }>(ptr, &mut offset)?;
        norito::core::finish_context_fields(ptr, offset)?;
        Ok(Self(bytes))
    }
}
impl<'de> norito::core::DecodeFromSlice<'de> for KaigiIdentityDigestV1 {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), norito::core::Error> {
        let raw = bytes
            .get(..Self::BYTES)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let mut digest = [0; Self::BYTES];
        digest.copy_from_slice(raw);
        norito::core::note_payload_access(bytes, Self::BYTES);
        Ok((Self(digest), Self::BYTES))
    }
}
impl norito::json::FastJsonWrite for KaigiIdentityDigestV1 {
    fn write_json(&self, out: &mut String) {
        crate::json_helpers::fixed_bytes::serialize(&self.0, out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        crate::json_helpers::fixed_bytes::serialize_bounded(&self.0, out)
    }
}
impl norito::json::JsonDeserialize for KaigiIdentityDigestV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        crate::json_helpers::fixed_bytes::deserialize(parser).map(Self)
    }
}

/// Public identity inputs for the final Kaigi authorization relation.
///
/// Host and subject use the same account domain, so equality means the exact
/// same canonical controller identity. The separate network value must also be
/// absorbed by the circuit; these account and call digests are network-neutral.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(reuse_archived)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kaigi::authorization::KaigiAuthorizationIdentitiesV1")]
pub struct KaigiAuthorizationIdentitiesV1 {
    /// Exact network identity supplied by the trusted ledger context.
    pub network_id: NetworkId,
    /// Complete canonical call identifier under the call-identity domain.
    pub call_id: KaigiIdentityDigestV1,
    /// Complete canonical original host under the account-identity domain.
    pub host_id: KaigiIdentityDigestV1,
    /// Complete canonical proof subject under the same account-identity domain.
    pub subject_id: KaigiIdentityDigestV1,
}

impl KaigiAuthorizationIdentitiesV1 {
    /// Derive public identity inputs from canonical model values.
    ///
    /// This does not establish a signature, lineage, membership, or proof claim.
    ///
    /// # Errors
    ///
    /// Returns the canonical serializer error or an I/O error if any encoded
    /// identity exceeds the fixed byte bound or cannot be allocated.
    pub fn new(
        network_id: NetworkId,
        call_id: &KaigiId,
        host_id: &AccountId,
        subject_id: &AccountId,
    ) -> Result<Self, norito::core::Error> {
        Ok(Self {
            network_id,
            call_id: hash_canonical_identity_v1(b"call-id", call_id)?,
            host_id: kaigi_account_identity_v1(host_id)?,
            subject_id: kaigi_account_identity_v1(subject_id)?,
        })
    }
}

/// Bind the complete canonical account controller for stable Kaigi lookup.
///
/// No display prefix, alias, routing domain, seed, or reduced scalar is used.
/// The host and subject bindings both call this exact function.
///
/// # Errors
///
/// Returns the canonical serializer error or an I/O error if the encoded
/// identity exceeds the fixed byte bound or cannot be allocated.
pub fn kaigi_account_identity_v1(
    account: &AccountId,
) -> Result<KaigiIdentityDigestV1, norito::core::Error> {
    hash_canonical_identity_v1(b"account-id", account)
}

fn hash_canonical_identity_v1<T: norito::core::NoritoSerialize>(
    role: &'static [u8],
    value: &T,
) -> Result<KaigiIdentityDigestV1, norito::core::Error> {
    let mut frame = BoundedIdentityFrameV1::default();
    norito::core::write_canonical_to_writer(value, &mut frame)?;
    let fields = [&frame.bytes[..]];
    let commitment = NativeCommitmentFrameV1::new(
        NativeCommitmentDomainV1 {
            catalog: b"iroha-kaigi-first-release-v1",
            protocol: b"kaigi-authorization-v1",
            role,
            phase: b"canonical-norito",
            level: 0,
            index: 0,
            counter: 0,
        },
        &fields,
    )
    .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(KaigiIdentityDigestV1(*commitment.digest().as_bytes()))
}

#[derive(Default)]
struct BoundedIdentityFrameV1 {
    bytes: Vec<u8>,
}

impl Write for BoundedIdentityFrameV1 {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|&size| size <= KAIGI_AUTHORIZATION_IDENTITY_MAX_BYTES_V1)
            .ok_or_else(|| {
                io::Error::other("Kaigi canonical identity exceeds the V1 byte bound")
            })?;
        self.bytes
            .try_reserve(next - self.bytes.len())
            .map_err(|_| io::Error::other("Kaigi canonical identity allocation failed"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{MultisigMember, MultisigPolicy, address::ChainDiscriminantGuard};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_model_base::domain::DomainId;
    use iroha_model_base::name::Name;
    use std::str::FromStr;

    fn account(seed: u8) -> AccountId {
        let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(pair.public_key().clone())
    }

    fn network(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([seed; 32])))
    }

    fn call(domain: &str, name: &str) -> KaigiId {
        KaigiId::new(
            DomainId::try_new(domain, "universal").expect("fixture domain"),
            Name::from_str(name).expect("fixture call"),
        )
    }

    fn identities() -> KaigiAuthorizationIdentitiesV1 {
        KaigiAuthorizationIdentitiesV1::new(
            network(0x31),
            &call("wonderland", "kaigi-authorization"),
            &account(1),
            &account(2),
        )
        .expect("complete canonical identity inputs")
    }

    #[test]
    fn identity_inputs_roundtrip_canonical_norito_and_json() {
        let original = identities();
        let encoded = norito::encode_canonical(&original).expect("encode identities");
        let decoded: KaigiAuthorizationIdentitiesV1 =
            norito::decode_from_bytes(&encoded).expect("decode identities");
        assert_eq!(decoded, original);
        let json = norito::json::to_json(&original).expect("encode identity JSON");
        let decoded: KaigiAuthorizationIdentitiesV1 =
            norito::json::from_str(&json).expect("decode identity JSON");
        assert_eq!(decoded, original);
        for digest in [original.call_id, original.host_id, original.subject_id] {
            assert_eq!(digest.into_bytes().len(), 32);
            assert_eq!(digest.words().len(), 4);
        }
    }

    #[test]
    fn identity_inputs_ignore_all_eight_ambient_layouts() {
        let expected = identities();
        for flags in 0..8 {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(identities(), expected, "ambient flags={flags}");
        }
    }

    #[test]
    fn canonical_identity_frames_match_independently_assembled_native_framing() {
        // Assemble the specified bytes directly, without calling the production
        // frame emitter. The lower crypto owner separately pins independent
        // BLAKE3 known-answer vectors for this frame, including the Kaigi role.
        fn reference(role: &[u8], canonical: &[u8]) -> [u8; 32] {
            let mut frame = b"iroha:native-commitment:frame:v1\0".to_vec();
            for (tag, field) in (1_u8..=6).zip([
                &b"blake3-256"[..],
                &b"iroha-kaigi-first-release-v1"[..],
                &b"kaigi-authorization-v1"[..],
                &b"native-commitment-blake3-256-v1"[..],
                role,
                &b"canonical-norito"[..],
            ]) {
                frame.push(tag);
                frame.extend_from_slice(&u32::try_from(field.len()).unwrap().to_le_bytes());
                frame.extend_from_slice(field);
            }
            for tag in 7_u8..=9 {
                frame.push(tag);
                frame.extend_from_slice(&0_u64.to_le_bytes());
            }
            frame.push(10);
            frame.extend_from_slice(&1_u32.to_le_bytes());
            frame.push(11);
            frame.extend_from_slice(&u64::try_from(canonical.len()).unwrap().to_le_bytes());
            frame.extend_from_slice(canonical);
            *blake3::hash(&frame).as_bytes()
        }
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("fixed canonical account public key");
        let account = AccountId::new(public_key);
        let call = call("wonderland", "kaigi-authorization");
        let inputs =
            KaigiAuthorizationIdentitiesV1::new(network(0x31), &call, &account, &account).unwrap();
        let account_bytes = norito::encode_canonical(&account).unwrap();
        let call_bytes = norito::encode_canonical(&call).unwrap();
        assert_eq!(
            inputs.host_id.as_bytes(),
            &reference(b"account-id", &account_bytes)
        );
        assert_eq!(inputs.subject_id, inputs.host_id);
        assert_eq!(
            inputs.call_id.as_bytes(),
            &reference(b"call-id", &call_bytes)
        );
        assert_ne!(
            reference(b"account-id", &account_bytes),
            reference(b"call-id", &account_bytes)
        );
        for bytes in [&account_bytes, &call_bytes] {
            for end in 0..bytes.len() {
                assert_ne!(
                    reference(b"account-id", bytes),
                    reference(b"account-id", &bytes[..end])
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert_ne!(
                reference(b"account-id", bytes),
                reference(b"account-id", &trailing)
            );
        }
    }

    #[test]
    fn canonical_identity_frames_match_fixed_external_blake3_vectors() {
        // Canonical model bytes were captured from the actual encoder. Digest
        // answers were derived by an independent Python BLAKE3 implementation
        // over the complete specified native frame, including domain and role.
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("fixed canonical account public key");
        let account = AccountId::new(public_key);
        let call = call("wonderland", "kaigi-authorization");
        let inputs =
            KaigiAuthorizationIdentitiesV1::new(network(0x31), &call, &account, &account).unwrap();
        assert_eq!(
            norito::encode_canonical(&account).unwrap(),
            hex::decode(concat!(
                "4e525430000060e81473aed0a1276f1c5776d0f69c38004f0000000000000018cb63d94c8fc56a02",
                "000000004a2100000000000000010001ce017f01a4016c019d01ce017e01a401b1012501e201e301",
                "6b01db016301ea01330107013e0175019001ac01920181016a01e101e8016101b70104018b0103",
            ))
            .unwrap()
        );
        let account_digest: [u8; 32] =
            hex::decode("b23bd023a5beb6da1e0473a701b0fe28f9a2e5db13b09903acf306c12d06dd89")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(
            norito::encode_canonical(&call).unwrap(),
            hex::decode(concat!(
                "4e52543000003c59d4919ec6cf3e2a2a26363002a6c4002d000000000000007df53756bab6151902",
                "170b0a776f6e6465726c616e640a09756e6976657273616c14136b616967692d617574686f72697a",
                "6174696f6e",
            ))
            .unwrap()
        );
        let call_digest: [u8; 32] =
            hex::decode("85ed8175f4ce2177050c95f9695d6b58a6965a678998925a420152382efa50a0")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(inputs.host_id.into_bytes(), account_digest);
        assert_eq!(inputs.subject_id, inputs.host_id);
        assert_eq!(kaigi_account_identity_v1(&account).unwrap(), inputs.host_id);
        assert_eq!(inputs.call_id.into_bytes(), call_digest);
        assert_eq!(
            inputs.host_id.words(),
            [
                0xdab6_bea5_23d0_3bb2,
                0x28fe_b001_a773_041e,
                0x0399_b013_dbe5_a2f9,
                0x89dd_062d_c106_f3ac
            ]
        );
        assert_eq!(
            inputs.call_id.words(),
            [
                0x7721_cef4_7581_ed85,
                0x586b_5d69_f995_0c05,
                0x5a92_9889_675a_96a6,
                0xa050_fa2e_3852_0142
            ]
        );
    }

    #[test]
    fn native_identity_preserves_every_bit_and_exact_codec_width() {
        for bytes in [
            [0; 32],
            [0xff; 32],
            [0x80; 32],
            core::array::from_fn(|i| u8::try_from(i).unwrap()),
        ] {
            let digest = KaigiIdentityDigestV1(bytes);
            assert_eq!(digest.as_bytes(), &bytes);
            assert_eq!(digest.into_bytes(), bytes);
            assert_eq!(digest.words().map(u64::to_le_bytes).concat(), bytes);
            for flags in (u8::MIN..=u8::MAX)
                .filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
            {
                let _flags = norito::core::DecodeFlagsGuard::enter(flags);
                let encoded = norito::codec::encode_with_header_flags(&digest).0;
                assert_eq!(encoded, bytes);
                let (decoded, consumed) =
                    norito::core::decode_field_canonical::<KaigiIdentityDigestV1>(&encoded)
                        .unwrap();
                assert_eq!(decoded, digest);
                assert_eq!(consumed, 32);
            }
            let json = norito::json::to_json(&digest).unwrap();
            assert_eq!(
                norito::json::from_str::<KaigiIdentityDigestV1>(&json).unwrap(),
                digest
            );
        }
        for bit in 0..256 {
            let mut bytes = [0; 32];
            bytes[bit / 8] = 1 << (bit % 8);
            let digest = KaigiIdentityDigestV1(bytes);
            assert_eq!(digest.words().map(u64::to_le_bytes).concat(), bytes);
            assert_ne!(digest, KaigiIdentityDigestV1([0; 32]));
        }
    }

    #[test]
    fn native_identity_rejects_every_truncation_trailing_byte_and_old_width() {
        for length in (0..32).chain([33, 48, 64]) {
            assert!(
                norito::core::decode_field_canonical::<KaigiIdentityDigestV1>(&vec![0xff; length])
                    .is_err()
            );
            let json = norito::json::to_json(&vec![0xff_u8; length]).unwrap();
            assert!(norito::json::from_str::<KaigiIdentityDigestV1>(&json).is_err());
        }
        let bytes = [0xff; 33];
        let (decoded, consumed) =
            <KaigiIdentityDigestV1 as norito::core::DecodeFromSlice>::decode_from_slice(&bytes)
                .unwrap();
        assert_eq!(decoded.as_bytes(), &[0xff; 32]);
        assert_eq!(
            consumed, 32,
            "prefix ownership is explicit to the enclosing codec"
        );
    }

    #[test]
    fn account_identity_ignores_display_chain_prefix() {
        let account = account(3);
        let expected = kaigi_account_identity_v1(&account).expect("account identity");
        let mut displays = Vec::new();
        for prefix in [0, 7, 753, u16::MAX] {
            let _prefix = ChainDiscriminantGuard::enter(prefix);
            displays.push(account.to_string());
            assert_eq!(kaigi_account_identity_v1(&account).unwrap(), expected);
        }
        assert!(displays.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn identity_inputs_bind_network_call_domain_host_and_subject() {
        let original = identities();
        let host = account(1);
        let subject = account(2);
        let call_id = call("wonderland", "kaigi-authorization");
        let other_network =
            KaigiAuthorizationIdentitiesV1::new(network(0x32), &call_id, &host, &subject).unwrap();
        assert_ne!(original, other_network);
        assert_eq!(original.call_id, other_network.call_id);
        assert_eq!(original.host_id, other_network.host_id);
        assert_eq!(original.subject_id, other_network.subject_id);
        for other_call in [
            call("wonderland", "other-call"),
            call("otherland", "kaigi-authorization"),
        ] {
            let other = KaigiAuthorizationIdentitiesV1::new(
                original.network_id,
                &other_call,
                &host,
                &subject,
            )
            .unwrap();
            assert_ne!(original.call_id, other.call_id);
        }
        let swapped =
            KaigiAuthorizationIdentitiesV1::new(original.network_id, &call_id, &subject, &host)
                .unwrap();
        assert_eq!(original.host_id, swapped.subject_id);
        assert_eq!(original.subject_id, swapped.host_id);
        assert_ne!(original.host_id, original.subject_id);
        let same = KaigiAuthorizationIdentitiesV1::new(original.network_id, &call_id, &host, &host)
            .unwrap();
        assert_eq!(same.host_id, same.subject_id);
    }

    #[test]
    fn account_identity_binds_full_multisig_policy_and_controller() {
        let members: Vec<_> = (1..=24)
            .map(|seed| {
                MultisigMember::new(account(seed).expect_single_signatory().clone(), 1).unwrap()
            })
            .collect();
        let policy = MultisigPolicy::new(2, members).unwrap();
        let original = AccountId::new_multisig(policy.clone());
        let original_bytes = norito::encode_canonical(&original).unwrap();
        let original_digest = kaigi_account_identity_v1(&original).unwrap();
        let mut changed_members = policy.members().to_vec();
        let last = changed_members.last_mut().unwrap();
        *last = MultisigMember::new(last.public_key().clone(), 2).unwrap();
        let weighted = AccountId::new_multisig(MultisigPolicy::new(2, changed_members).unwrap());
        let weighted_bytes = norito::encode_canonical(&weighted).unwrap();
        // The header checksum changes too; the policy payload differs at the
        // final member weight, well beyond any scalar-sized account prefix.
        assert!(original_bytes.len() > 256);
        assert_ne!(
            &original_bytes[original_bytes.len() - 16..],
            &weighted_bytes[weighted_bytes.len() - 16..]
        );
        assert_ne!(
            original_digest,
            kaigi_account_identity_v1(&weighted).unwrap()
        );
        let threshold =
            AccountId::new_multisig(MultisigPolicy::new(3, policy.members().to_vec()).unwrap());
        assert_ne!(
            original_digest,
            kaigi_account_identity_v1(&threshold).unwrap()
        );
        let mut changed_members = policy.members().to_vec();
        *changed_members.last_mut().unwrap() =
            MultisigMember::new(account(99).expect_single_signatory().clone(), 1).unwrap();
        let member = AccountId::new_multisig(MultisigPolicy::new(2, changed_members).unwrap());
        assert_ne!(original_digest, kaigi_account_identity_v1(&member).unwrap());
        let single = account(1);
        let multisig = AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                vec![MultisigMember::new(single.expect_single_signatory().clone(), 1).unwrap()],
            )
            .unwrap(),
        );
        assert_ne!(
            kaigi_account_identity_v1(&single).unwrap(),
            kaigi_account_identity_v1(&multisig).unwrap()
        );
        let secp = AccountId::new(
            KeyPair::from_seed(vec![1; 32], Algorithm::Secp256k1)
                .public_key()
                .clone(),
        );
        assert_ne!(
            kaigi_account_identity_v1(&single).unwrap(),
            kaigi_account_identity_v1(&secp).unwrap()
        );
    }

    #[test]
    fn canonical_identity_frame_is_bounded_without_partial_write() {
        let mut output = BoundedIdentityFrameV1::default();
        output
            .write_all(&vec![0x51; KAIGI_AUTHORIZATION_IDENTITY_MAX_BYTES_V1])
            .unwrap();
        output.flush().unwrap();
        assert!(output.write_all(&[0x52]).is_err());
        assert_eq!(
            output.bytes.len(),
            KAIGI_AUTHORIZATION_IDENTITY_MAX_BYTES_V1
        );
        assert!(output.bytes.iter().all(|&byte| byte == 0x51));
        assert!(
            hash_canonical_identity_v1(
                b"account-id",
                &"x".repeat(KAIGI_AUTHORIZATION_IDENTITY_MAX_BYTES_V1)
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod additional_frame_owner_identity_tests {
    //! Typed frame contracts observed with the original codec.

    #[test]
    fn captured_additional_frame_owner_identities() {
        crate::frame_owner_identity_tests::assert_bidirectional::<
            crate::kaigi::authorization::KaigiAuthorizationIdentitiesV1,
        >("iroha_data_model::kaigi::authorization::KaigiAuthorizationIdentitiesV1");
    }
}
