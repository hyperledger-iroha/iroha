//! VRF-backed parliament selection utilities shared between Torii, CLI, and core tests.
//!
//! The helpers in this module implement the deterministic seed generation and
//! committee selection rules described in `gov.md` (Sortition-based Sora
//! parliament). They centralise the VRF domain separation tags and ordering
//! semantics so that all callers derive identical results.

use std::collections::BTreeSet;

use iroha_crypto::{
    blake2::{Blake2b512, Digest as _},
    vrf::{self, VrfProof},
};
use iroha_data_model::{ChainId, account::AccountId};
use norito::codec::Encode;

/// Domain separator for parliament seed derivation (`Blake2b-512`).
pub const SEED_DOMAIN: &[u8] = b"gov:parliament:seed:v1";
/// Domain separator for VRF inputs (`itoha` VRF v1 parliament lane).
pub const INPUT_DOMAIN: &[u8] = b"iroha:vrf:v1:parliament|";

/// Candidate VRF variant: mirrors the BLS signature flavour used for proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateVariant {
    /// BLS Normal: public key in G1 (48 bytes), proof/signature in G2 (96 bytes).
    Normal,
    /// BLS Small: public key in G2 (96 bytes), proof/signature in G1 (48 bytes).
    Small,
}

impl CandidateVariant {
    /// Expected proof length (bytes) for the variant.
    #[must_use]
    pub const fn proof_len(self) -> usize {
        match self {
            CandidateVariant::Normal => 96,
            CandidateVariant::Small => 48,
        }
    }
}

impl core::str::FromStr for CandidateVariant {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.eq_ignore_ascii_case("normal") => Ok(Self::Normal),
            s if s.eq_ignore_ascii_case("small") => Ok(Self::Small),
            _ => Err("unknown VRF variant (expected Normal or Small)"),
        }
    }
}

/// Borrowed candidate descriptor supplied to [`derive_committee`].
#[derive(Clone, Copy)]
pub struct CandidateRef<'a> {
    /// Account identifier of the candidate.
    pub account_id: &'a AccountId,
    /// VRF variant used to generate the proof.
    pub variant: CandidateVariant,
    /// Canonical public-key bytes (compressed encoding) matching the variant.
    pub public_key: &'a [u8],
    /// Canonical proof/signature bytes (compressed encoding) matching the variant.
    pub proof: &'a [u8],
}

/// Committee member produced by [`derive_committee`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Account identifier admitted to the committee.
    pub account_id: AccountId,
    /// VRF output bytes used for ordering (big-endian when compared).
    pub output: [u8; 32],
}

/// Result of VRF verification and committee derivation.
#[derive(Debug, Clone, Default)]
pub struct DerivationResult {
    /// Ordered committee members (descending VRF output, tie-broken by account id).
    pub members: Vec<Member>,
    /// Number of candidates whose proofs verified successfully.
    pub verified: usize,
}

impl DerivationResult {
    /// Whether the result contains no members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// Compute the deterministic parliament seed for the given epoch and beacon.
///
/// Seed formula: `Blake2b512("gov:parliament:seed:v1" || chain_id || epoch_be || beacon)`.
#[must_use]
pub fn compute_seed(chain_id: &ChainId, epoch: u64, beacon: &[u8; 32]) -> [u8; 64] {
    let mut hasher = Blake2b512::new();
    hasher.update(SEED_DOMAIN);
    hasher.update(chain_id.as_str().as_bytes());
    hasher.update(epoch.to_be_bytes());
    hasher.update(beacon);
    let digest = hasher.finalize();
    let mut out = [0u8; 64];
    out.copy_from_slice(&digest);
    out
}

/// Construct the VRF input message `tag || seed || encode(account_id)`.
#[must_use]
pub fn build_input(seed: &[u8; 64], account_id: &AccountId) -> Vec<u8> {
    let account_bytes = Encode::encode(account_id);
    let mut buf = Vec::with_capacity(INPUT_DOMAIN.len() + seed.len() + account_bytes.len());
    buf.extend_from_slice(INPUT_DOMAIN);
    buf.extend_from_slice(seed);
    buf.extend_from_slice(&account_bytes);
    buf
}

/// Verify VRF proofs for the supplied candidates and return up to `committee_size`
/// members sorted by descending VRF output (tie-breaking by account id).
pub fn derive_committee<'a, I>(
    chain_id: &ChainId,
    seed: &[u8; 64],
    candidates: I,
    committee_size: usize,
) -> DerivationResult
where
    I: IntoIterator<Item = CandidateRef<'a>>,
{
    let chain_bytes = chain_id.as_str().as_bytes();
    let mut scored: Vec<([u8; 32], AccountId)> = Vec::new();
    let mut verified = 0usize;

    for candidate in candidates {
        if candidate.proof.len() != candidate.variant.proof_len() {
            continue;
        }
        let input = build_input(seed, candidate.account_id);
        let maybe_output = match candidate.variant {
            CandidateVariant::Normal => {
                let Ok(pk) = iroha_crypto::BlsNormal::parse_public_key(candidate.public_key) else {
                    continue;
                };
                let mut arr = [0u8; 96];
                arr.copy_from_slice(candidate.proof);
                let proof = VrfProof::SigInG2(arr);
                vrf::verify_normal_with_chain(&pk, chain_bytes, &input, &proof)
            }
            CandidateVariant::Small => {
                let Ok(pk) = iroha_crypto::BlsSmall::parse_public_key(candidate.public_key) else {
                    continue;
                };
                let mut arr = [0u8; 48];
                arr.copy_from_slice(candidate.proof);
                let proof = VrfProof::SigInG1(arr);
                vrf::verify_small_with_chain(&pk, chain_bytes, &input, &proof)
            }
        };
        if let Some(vrf::VrfOutput(output)) = maybe_output {
            verified += 1;
            scored.push((output, candidate.account_id.clone()));
        }
    }

    scored.sort_by(|a, b| {
        use core::cmp::Ordering;
        match b.0.cmp(&a.0) {
            Ordering::Equal => a.1.cmp(&b.1),
            other => other,
        }
    });

    let mut unique_accounts: BTreeSet<AccountId> = BTreeSet::new();
    let mut members = Vec::with_capacity(committee_size.min(scored.len()));
    for (output, account_id) in scored {
        if unique_accounts.insert(account_id.clone()) {
            members.push(Member { account_id, output });
            if members.len() >= committee_size {
                break;
            }
        }
    }

    DerivationResult { members, verified }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        Algorithm, BlsNormal, KeyGenOption, KeyPair,
        vrf::{VrfProof, prove_normal_with_chain},
    };
    use iroha_data_model::{account::AccountId, domain::DomainId};

    use super::*;

    fn mk_account(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        let domain: DomainId = "wonderland".parse().expect("valid domain id");
        AccountId::new(domain, public_key)
    }

    fn make_candidate(
        account: &AccountId,
        chain_id: &ChainId,
        seed: &[u8; 64],
        key_seed: u8,
    ) -> (Vec<u8>, Vec<u8>, [u8; 32]) {
        let input = build_input(seed, account);
        let (pk_raw, sk) = BlsNormal::keypair(KeyGenOption::UseSeed(vec![key_seed; 4]));
        let (vrf_output, proof) =
            prove_normal_with_chain(&sk, chain_id.as_str().as_bytes(), &input);
        let key_pair = KeyPair::from((pk_raw, sk));
        let (public_key, _) = key_pair.into_parts();
        let (algo, pk_payload) = public_key.to_bytes();
        assert_eq!(algo, Algorithm::BlsNormal);
        let proof_vec = match proof {
            VrfProof::SigInG2(arr) => arr.to_vec(),
            _ => unreachable!("normal variant produces SigInG2"),
        };
        (pk_payload.to_vec(), proof_vec, vrf_output.0)
    }

    #[test]
    fn seed_matches_manual_derivation() {
        let chain_id: ChainId = "chain-demo".into();
        let epoch = 42u64;
        let beacon = [0xAB; 32];
        let seed = compute_seed(&chain_id, epoch, &beacon);

        let mut hasher = Blake2b512::new();
        hasher.update(SEED_DOMAIN);
        hasher.update(chain_id.as_str().as_bytes());
        hasher.update(epoch.to_be_bytes());
        hasher.update(beacon);
        let manual = hasher.finalize();

        assert_eq!(&seed[..], &manual[..]);
    }

    #[test]
    #[ignore = "Requires full VRF fixture generation; re-enable with dedicated harness"]
    fn committee_orders_desc_and_dedups() {
        let chain_id: ChainId = "parliament-demo".into();
        let beacon = [7u8; 32];
        let epoch = 0u64;
        let seed = compute_seed(&chain_id, epoch, &beacon);

        let alice = mk_account(1);
        let bob = mk_account(2);

        let (alice_pk, alice_proof, _) = make_candidate(&alice, &chain_id, &seed, 11);
        let (bob_pk, bob_proof, _) = make_candidate(&bob, &chain_id, &seed, 22);

        let candidates = vec![
            CandidateRef {
                account_id: &alice,
                variant: CandidateVariant::Normal,
                public_key: &alice_pk,
                proof: &alice_proof,
            },
            // Duplicate Alice with the same proof to exercise deduplication.
            CandidateRef {
                account_id: &alice,
                variant: CandidateVariant::Normal,
                public_key: &alice_pk,
                proof: &alice_proof,
            },
            CandidateRef {
                account_id: &bob,
                variant: CandidateVariant::Normal,
                public_key: &bob_pk,
                proof: &bob_proof,
            },
        ];

        let derivation = derive_committee(&chain_id, &seed, candidates, 3);
        assert_eq!(
            derivation.members.len(),
            2,
            "duplicate candidate should be dropped"
        );
        assert_eq!(derivation.members[0].account_id, alice);
        assert_eq!(derivation.members[1].account_id, bob);
    }
}
