//! Shared pre-auxiliary Fiat--Shamir schedule for the X5S1 credential proof.
//!
//! The MAIN and compact-CA traces compare grand-product terminals. Those
//! products are meaningful only when both subproofs use the same tuple
//! challenges. This module samples that shared challenge family after all six
//! MAIN base roots and the compact-CA base root have been committed, then
//! supplies a typed binding that each local subproof absorbs before its
//! auxiliary commitments.

use thiserror::Error;

use super::{
    profile::{ZK_X509_PROOF_VERSION_V1, ZK_X509_SUITE_V1, ZK_X509_TRACE_GROUPS_V1},
    rfc5280_stark::{ZkX509Rfc5280StarkChallengesV1, derive_zk_x509_rfc5280_stark_challenges_v1},
    sha_call_bus_stark::{ZkX509ShaCallBusChallengesV1, derive_zk_x509_sha_call_bus_challenges_v1},
};
use crate::privacy_engines::transparent_stark::{
    TransparentStarkErrorV1, TransparentTranscriptV1, append_u16_v1, sha256_frame_v1,
};

const CREDENTIAL_PRE_AUX_MAGIC_V1: [u8; 4] = *b"X5B1";
const CREDENTIAL_PRE_AUX_PROFILE_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:credential-pre-aux:profile:v1";
const CREDENTIAL_PRE_AUX_PUBLIC_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:credential-pre-aux:public:v1";
const CREDENTIAL_PRE_AUX_ROOTS_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:credential-pre-aux:base-roots:v1";
const CREDENTIAL_PRE_AUX_LOCAL_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:credential-pre-aux:local-binding:v1";
const MAIN_ROOT_KIND_V1: u8 = 1;
const CA_ROOT_KIND_V1: u8 = 2;

/// Consensus-critical framing and sampling order for the joint challenge set.
pub(crate) const ZK_X509_CREDENTIAL_PRE_AUX_DESCRIPTOR_V1: &[u8] =
    b"zk-x509-credential-pre-aux-v1:X5B1:version1:profile=main-profile-digest+ca-profile-digest:public=consensus-context-digest+ca-public-digest:base-roots=exact-main6-ordered-log5,8,15,16,18,19-then-ca1-log7:derive-sha-four-lane-seven-term-first:derive-rfc-four-lane-twelve-term-second:bind-post-challenge-state-and-canonical-challenges-into-each-local-transcript-before-aux-roots:no-caller-selected-binding";

/// Exact number of verifier-owned MAIN trace groups in the first release.
pub(crate) const ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1: usize = ZK_X509_TRACE_GROUPS_V1;

const _: () = assert!(ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1 == 6);

/// MAIN-owned input to the joint pre-auxiliary challenge schedule.
///
/// The outer verifier constructs this only after decoding the canonical MAIN
/// layout. The fixed-size root array makes omission, excess, and a caller-
/// selected group count unrepresentable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CredentialMainPreAuxV1 {
    /// Digest of the complete verifier-owned statement and genesis context.
    pub(crate) consensus_context_digest: [u8; 32],
    /// Digest of the canonical first-release MAIN profile.
    pub(crate) main_profile_digest: [u8; 32],
    /// Exact log5, log8, log15, log16, log18, and log19 MAIN base roots.
    pub(crate) main_base_roots: [[u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
}

/// Challenges jointly derived from all X5S1 base commitments.
///
/// Fields are private so another adapter cannot assemble a caller-selected
/// state/challenge combination. Verifiers always recompute this value from
/// decoded roots and verifier-owned profile/public digests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CredentialPreAuxBindingV1 {
    sha: ZkX509ShaCallBusChallengesV1,
    rfc5280: ZkX509Rfc5280StarkChallengesV1,
    transcript_state: [u8; 32],
}

impl ZkX509CredentialPreAuxBindingV1 {
    /// Shared SHA address/value grand-product challenges.
    pub(crate) const fn sha(self) -> ZkX509ShaCallBusChallengesV1 {
        self.sha
    }

    /// Shared RFC output grand-product challenges.
    pub(crate) const fn rfc5280(self) -> ZkX509Rfc5280StarkChallengesV1 {
        self.rfc5280
    }

    /// Post-challenge state bound into both local subproof transcripts.
    pub(crate) const fn transcript_state(self) -> [u8; 32] {
        self.transcript_state
    }
}

/// Joint transcript construction or challenge validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509CredentialPreAuxErrorV1 {
    /// A framed transcript operation failed.
    #[error("zk-X509 credential pre-auxiliary transcript is invalid")]
    Transcript,
    /// A derived bus challenge family failed its canonical validation.
    #[error("zk-X509 credential pre-auxiliary challenges are invalid")]
    Challenge,
    /// Checked framing allocation failed.
    #[error("zk-X509 credential pre-auxiliary framing exceeded its resource envelope")]
    Resource,
}

impl From<TransparentStarkErrorV1> for ZkX509CredentialPreAuxErrorV1 {
    fn from(_: TransparentStarkErrorV1) -> Self {
        Self::Transcript
    }
}

fn pre_aux_profile_digest_v1(
    main_profile_digest: [u8; 32],
    ca_profile_digest: [u8; 32],
) -> Result<[u8; 32], ZkX509CredentialPreAuxErrorV1> {
    sha256_frame_v1(
        CREDENTIAL_PRE_AUX_PROFILE_DOMAIN_V1,
        &[
            &CREDENTIAL_PRE_AUX_MAGIC_V1,
            &ZK_X509_PROOF_VERSION_V1.to_be_bytes(),
            ZK_X509_CREDENTIAL_PRE_AUX_DESCRIPTOR_V1,
            &main_profile_digest,
            &ca_profile_digest,
        ],
    )
    .map_err(Into::into)
}

fn pre_aux_public_digest_v1(
    consensus_context_digest: [u8; 32],
    ca_public_digest: [u8; 32],
) -> Result<[u8; 32], ZkX509CredentialPreAuxErrorV1> {
    sha256_frame_v1(
        CREDENTIAL_PRE_AUX_PUBLIC_DOMAIN_V1,
        &[
            &CREDENTIAL_PRE_AUX_MAGIC_V1,
            &ZK_X509_PROOF_VERSION_V1.to_be_bytes(),
            &consensus_context_digest,
            &ca_public_digest,
        ],
    )
    .map_err(Into::into)
}

fn encode_pre_aux_roots_v1(
    main_base_roots: [[u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
    ca_base_root: [u8; 32],
) -> Result<Vec<u8>, ZkX509CredentialPreAuxErrorV1> {
    const ROOT_RECORD_BYTES_V1: usize = 1 + 2 + 32;
    let exact = 4 + 2 + 2 + (ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1 + 1) * ROOT_RECORD_BYTES_V1;
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(exact)
        .map_err(|_| ZkX509CredentialPreAuxErrorV1::Resource)?;
    encoded.extend_from_slice(&CREDENTIAL_PRE_AUX_MAGIC_V1);
    append_u16_v1(&mut encoded, ZK_X509_PROOF_VERSION_V1);
    append_u16_v1(
        &mut encoded,
        u16::try_from(ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1)
            .map_err(|_| ZkX509CredentialPreAuxErrorV1::Resource)?,
    );
    for (index, root) in main_base_roots.into_iter().enumerate() {
        encoded.push(MAIN_ROOT_KIND_V1);
        append_u16_v1(
            &mut encoded,
            u16::try_from(index).map_err(|_| ZkX509CredentialPreAuxErrorV1::Resource)?,
        );
        encoded.extend_from_slice(&root);
    }
    encoded.push(CA_ROOT_KIND_V1);
    append_u16_v1(&mut encoded, 0);
    encoded.extend_from_slice(&ca_base_root);
    if encoded.len() != exact {
        return Err(ZkX509CredentialPreAuxErrorV1::Resource);
    }
    Ok(encoded)
}

fn encode_pre_aux_challenges_v1(
    binding: ZkX509CredentialPreAuxBindingV1,
) -> Result<Vec<u8>, ZkX509CredentialPreAuxErrorV1> {
    binding
        .sha
        .validate()
        .map_err(|_| ZkX509CredentialPreAuxErrorV1::Challenge)?;
    binding
        .rfc5280
        .validate()
        .map_err(|_| ZkX509CredentialPreAuxErrorV1::Challenge)?;
    let field_count = binding
        .sha
        .lanes
        .iter()
        .map(|lane| lane.terms.len())
        .sum::<usize>()
        .checked_add(
            binding
                .rfc5280
                .tuple
                .iter()
                .map(|lane| lane.len())
                .sum::<usize>(),
        )
        .ok_or(ZkX509CredentialPreAuxErrorV1::Resource)?;
    let exact = field_count
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(ZkX509CredentialPreAuxErrorV1::Resource)?;
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(exact)
        .map_err(|_| ZkX509CredentialPreAuxErrorV1::Resource)?;
    for value in binding
        .sha
        .lanes
        .iter()
        .flat_map(|lane| lane.terms)
        .chain(binding.rfc5280.tuple.into_iter().flatten())
    {
        encoded.extend_from_slice(&value.0.to_be_bytes());
    }
    if encoded.len() != exact {
        return Err(ZkX509CredentialPreAuxErrorV1::Resource);
    }
    Ok(encoded)
}

/// Derive the sole shared X5S1 bus challenge family.
///
/// The compact-CA root is supplied separately because it is decoded from (or
/// produced by) the X5C1 subproof. The caller never supplies a challenge or
/// post-challenge state. MAIN and compact-CA verifiers must each compare their
/// own decoded base roots with the roots used here before accepting the proof.
pub(crate) fn derive_zk_x509_credential_pre_aux_binding_v1(
    main: ZkX509CredentialMainPreAuxV1,
    ca_profile_digest: [u8; 32],
    ca_public_digest: [u8; 32],
    ca_base_root: [u8; 32],
) -> Result<ZkX509CredentialPreAuxBindingV1, ZkX509CredentialPreAuxErrorV1> {
    let profile_digest = pre_aux_profile_digest_v1(main.main_profile_digest, ca_profile_digest)?;
    let public_digest = pre_aux_public_digest_v1(main.consensus_context_digest, ca_public_digest)?;
    let roots = encode_pre_aux_roots_v1(main.main_base_roots, ca_base_root)?;
    let mut transcript =
        TransparentTranscriptV1::new(ZK_X509_SUITE_V1, &profile_digest, &public_digest)?;
    transcript.absorb(CREDENTIAL_PRE_AUX_ROOTS_DOMAIN_V1, &[&roots])?;
    let sha = derive_zk_x509_sha_call_bus_challenges_v1(&mut transcript)?;
    let rfc5280 = derive_zk_x509_rfc5280_stark_challenges_v1(&mut transcript)?;
    sha.validate()
        .map_err(|_| ZkX509CredentialPreAuxErrorV1::Challenge)?;
    rfc5280
        .validate()
        .map_err(|_| ZkX509CredentialPreAuxErrorV1::Challenge)?;
    Ok(ZkX509CredentialPreAuxBindingV1 {
        sha,
        rfc5280,
        transcript_state: transcript.state(),
    })
}

/// Bind a recomputed joint pre-auxiliary schedule into one local transcript.
///
/// This call belongs immediately after that subproof's own base-root frame and
/// immediately before its auxiliary-root frame. Encoding the challenges as
/// well as the post-challenge state makes accidental state/challenge mixing
/// fail closed even inside crate-private integration code.
pub(crate) fn absorb_zk_x509_credential_pre_aux_binding_v1(
    transcript: &mut TransparentTranscriptV1,
    binding: ZkX509CredentialPreAuxBindingV1,
) -> Result<(), ZkX509CredentialPreAuxErrorV1> {
    let challenges = encode_pre_aux_challenges_v1(binding)?;
    transcript
        .absorb(
            CREDENTIAL_PRE_AUX_LOCAL_BINDING_DOMAIN_V1,
            &[
                &CREDENTIAL_PRE_AUX_MAGIC_V1,
                &ZK_X509_PROOF_VERSION_V1.to_be_bytes(),
                &binding.transcript_state,
                &challenges,
            ],
        )
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn main_pre_aux_v1() -> ZkX509CredentialMainPreAuxV1 {
        ZkX509CredentialMainPreAuxV1 {
            consensus_context_digest: [0x11; 32],
            main_profile_digest: [0x22; 32],
            main_base_roots: core::array::from_fn(|index| {
                [u8::try_from(0x30 + index).expect("fixture byte"); 32]
            }),
        }
    }

    fn derive_v1(main: ZkX509CredentialMainPreAuxV1) -> ZkX509CredentialPreAuxBindingV1 {
        derive_zk_x509_credential_pre_aux_binding_v1(main, [0x44; 32], [0x55; 32], [0x66; 32])
            .expect("joint pre-auxiliary binding")
    }

    #[test]
    fn exact_seven_root_schedule_is_deterministic_and_valid() {
        let binding = derive_v1(main_pre_aux_v1());
        assert_eq!(binding, derive_v1(main_pre_aux_v1()));
        binding.sha().validate().expect("SHA challenges");
        binding.rfc5280().validate().expect("RFC challenges");
        assert_ne!(binding.transcript_state(), [0; 32]);
    }

    #[test]
    fn exact_seven_root_schedule_has_a_stable_known_answer() {
        let binding = derive_v1(main_pre_aux_v1());
        assert_eq!(binding.sha().lanes[0].terms[0].0, 5_205_700_668_208_280_363);
        assert_eq!(binding.sha().lanes[3].terms[6].0, 7_171_101_348_464_007_617);
        assert_eq!(binding.rfc5280().tuple[0][0].0, 18_330_132_050_529_501_757);
        assert_eq!(binding.rfc5280().tuple[3][11].0, 13_590_619_537_651_967_184);
        assert_eq!(
            binding.transcript_state(),
            [
                0x1d, 0xe2, 0x0a, 0xa9, 0x60, 0x3c, 0x2c, 0x31, 0xd9, 0x3a, 0x6b, 0x59, 0xca, 0xbb,
                0x83, 0xf8, 0xc0, 0xd7, 0x66, 0x27, 0x14, 0xcf, 0x38, 0x16, 0xac, 0x1b, 0x0b, 0x38,
                0xfd, 0x1d, 0x73, 0x4b,
            ]
        );
    }

    #[test]
    fn every_public_profile_and_base_root_input_changes_the_binding() {
        let canonical_main = main_pre_aux_v1();
        let canonical = derive_v1(canonical_main);

        let mut changed = canonical_main;
        changed.consensus_context_digest[0] ^= 1;
        assert_ne!(derive_v1(changed), canonical);

        let mut changed = canonical_main;
        changed.main_profile_digest[0] ^= 1;
        assert_ne!(derive_v1(changed), canonical);

        for index in 0..ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1 {
            let mut changed = canonical_main;
            changed.main_base_roots[index][0] ^= 1;
            assert_ne!(derive_v1(changed), canonical, "MAIN root {index}");
        }

        let changed_ca_profile = derive_zk_x509_credential_pre_aux_binding_v1(
            canonical_main,
            [0x45; 32],
            [0x55; 32],
            [0x66; 32],
        )
        .expect("changed CA profile");
        assert_ne!(changed_ca_profile, canonical);

        let changed_ca_public = derive_zk_x509_credential_pre_aux_binding_v1(
            canonical_main,
            [0x44; 32],
            [0x54; 32],
            [0x66; 32],
        )
        .expect("changed CA public");
        assert_ne!(changed_ca_public, canonical);

        let changed_ca_root = derive_zk_x509_credential_pre_aux_binding_v1(
            canonical_main,
            [0x44; 32],
            [0x55; 32],
            [0x67; 32],
        )
        .expect("changed CA root");
        assert_ne!(changed_ca_root, canonical);
    }

    #[test]
    fn main_root_order_and_local_binding_state_are_domain_bound() {
        let canonical_main = main_pre_aux_v1();
        let canonical = derive_v1(canonical_main);
        let mut reordered = canonical_main;
        reordered.main_base_roots.swap(0, 1);
        assert_ne!(derive_v1(reordered), canonical);

        let mut left = TransparentTranscriptV1::new(b"local", &[0x71; 32], &[0x72; 32])
            .expect("left transcript");
        let mut right = left;
        absorb_zk_x509_credential_pre_aux_binding_v1(&mut left, canonical)
            .expect("canonical local binding");
        let mut hostile = canonical;
        hostile.transcript_state[0] ^= 1;
        absorb_zk_x509_credential_pre_aux_binding_v1(&mut right, hostile)
            .expect("hostile local binding");
        assert_ne!(left.state(), right.state());
    }
}
