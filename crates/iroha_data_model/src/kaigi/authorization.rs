//! Complete canonical identity bindings for first-release Kaigi authorization.
//!
//! These values are public circuit inputs, not authorization evidence. Core
//! reconstructs them from the trusted network, stored call and host, and the
//! authenticated subject. Each account binding retains all six digest lanes
//! over the complete canonical Norito frame, including its controller policy.

use std::io::{self, Write};

use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use crate::{NetworkId, account::AccountId, kaigi::KaigiId, privacy::GoldilocksDigest384V1};

/// Maximum canonical Norito frame size for a Kaigi account or call identity.
pub const KAIGI_AUTHORIZATION_IDENTITY_MAX_BYTES_V1: usize = 1024 * 1024;

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
    pub call_id: GoldilocksDigest384V1,
    /// Complete canonical original host under the account-identity domain.
    pub host_id: GoldilocksDigest384V1,
    /// Complete canonical proof subject under the same account-identity domain.
    pub subject_id: GoldilocksDigest384V1,
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
) -> Result<GoldilocksDigest384V1, norito::core::Error> {
    hash_canonical_identity_v1(b"account-id", account)
}

fn hash_canonical_identity_v1<T: norito::core::NoritoSerialize>(
    role: &'static [u8],
    value: &T,
) -> Result<GoldilocksDigest384V1, norito::core::Error> {
    let mut frame = BoundedIdentityFrameV1::default();
    norito::core::write_canonical_to_writer(value, &mut frame)?;
    fastpq_isi::hash_bytes_384_v1(
        fastpq_isi::GoldilocksDigestDomainV1 {
            catalog: b"iroha-kaigi-first-release-v1",
            protocol: b"kaigi-authorization-v1",
            profile: b"poseidon-x7-goldilocks-6x64-v1",
            role,
            phase: b"canonical-norito",
            level: 0,
            index: 0,
            counter: 0,
        },
        &[&frame.bytes],
    )
    .map(Into::into)
    .ok_or_else(|| io::Error::other("Kaigi identity digest frame exceeds canonical bounds").into())
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
            assert_eq!(digest.to_le_bytes().len(), 48);
            assert_eq!(digest.words().len(), 6);
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
    fn identities_match_independent_six_lane_canonical_frame_vectors() {
        // The model owner emitted the full canonical Norito frames. The
        // independent Python SHAKE256/integer implementation in
        // scripts/check_goldilocks_digest384_reference.py then evaluated all
        // six lanes with the exact domains above, without using this adapter.
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("fixed canonical account public key");
        let account = AccountId::new(public_key);
        let inputs = KaigiAuthorizationIdentitiesV1::new(
            network(0x31),
            &call("wonderland", "kaigi-authorization"),
            &account,
            &account,
        )
        .unwrap();
        assert_eq!(
            inputs.host_id.words(),
            [
                0xe696_37e2_f1fd_81d2,
                0x7d8b_68f1_3700_deac,
                0x17ca_2400_6e05_1ed6,
                0x0bec_8048_fc1f_df62,
                0x3d58_9bf3_292c_474c,
                0x4f1a_f501_656d_52f5,
            ]
        );
        assert_eq!(inputs.subject_id, inputs.host_id);
        assert_eq!(
            inputs.call_id.words(),
            [
                0x6c3f_869e_c1ca_f8f3,
                0xa17b_87f2_9393_cb3c,
                0xaf29_09b4_eee6_2165,
                0xcdd9_089f_bdba_11fc,
                0x5757_08e3_3f7a_b761,
                0x3ee0_794d_e3aa_38bc,
            ]
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
