//! Governance body selector helpers (parliament seats + alternates) using VRF draws.

use iroha_config::parameters::actual::Governance;
use iroha_data_model::ChainId;

use crate::governance::{
    draw::{self, Draw},
    parliament::CandidateRef,
};

/// Select parliament members/alternates using VRF draw and governance config.
pub fn select_parliament<'a, I>(
    gov_cfg: &Governance,
    chain_id: &ChainId,
    epoch: u64,
    beacon: &[u8; 32],
    candidates: I,
) -> Draw
where
    I: IntoIterator<Item = CandidateRef<'a>>,
{
    let committee = gov_cfg.parliament_committee_size;
    let alternates = gov_cfg
        .parliament_alternate_size
        .unwrap_or(committee)
        .max(committee);
    draw::run_draw(chain_id, epoch, beacon, candidates, committee, alternates)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        Algorithm, BlsNormal, KeyGenOption, KeyPair,
        vrf::{VrfProof, prove_normal_with_chain},
    };
    use iroha_data_model::account::AccountId;

    use super::*;

    fn mk_account(seed: u8) -> AccountId {
        use iroha_crypto::{Algorithm, KeyPair};
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        AccountId::new(public_key)
    }

    fn make_candidate(
        account: &AccountId,
        chain_id: &ChainId,
        seed: &[u8; 64],
        key_seed: u8,
    ) -> (Vec<u8>, Vec<u8>) {
        let input = crate::governance::parliament::build_input(seed, account);
        let (pk_raw, sk) = BlsNormal::keypair(KeyGenOption::UseSeed(vec![key_seed; 4]))
            .expect("deterministic BLS keypair");
        let (_, proof) = prove_normal_with_chain(&sk, chain_id.as_str().as_bytes(), &input)
            .expect("deterministic normal VRF proof");
        let key_pair = KeyPair::from((pk_raw, sk));
        let (public_key, _) = key_pair.into_parts();
        let (algo, pk_payload) = public_key
            .try_to_bytes()
            .expect("fixture public key must be valid");
        assert_eq!(algo, Algorithm::BlsNormal);
        let proof_vec = match proof {
            VrfProof::SigInG2(arr) => arr.to_vec(),
            _ => unreachable!("normal variant produces SigInG2"),
        };
        (pk_payload.to_vec(), proof_vec)
    }

    #[test]
    fn select_parliament_draws_members_and_alternates() {
        let mut cfg = Governance::default();
        cfg.parliament_committee_size = 2;
        cfg.parliament_alternate_size = Some(1);
        let chain_id: ChainId = "selector-demo".into();
        let beacon = [3u8; 32];
        let accounts = [mk_account(1), mk_account(2), mk_account(3)];
        let seed = crate::governance::parliament::compute_seed(&chain_id, 5, &beacon);
        let proofs: Vec<_> = accounts
            .iter()
            .enumerate()
            .map(|(idx, account_id)| {
                make_candidate(
                    account_id,
                    &chain_id,
                    &seed,
                    u8::try_from(idx).expect("idx fits").saturating_add(11),
                )
            })
            .collect();
        let candidates =
            accounts
                .iter()
                .zip(proofs.iter())
                .map(|(account_id, (public_key, proof))| CandidateRef {
                    account_id,
                    variant: crate::governance::parliament::CandidateVariant::Normal,
                    public_key,
                    proof,
                });

        let draw = select_parliament(&cfg, &chain_id, 5, &beacon, candidates);

        assert_eq!(draw.members.len(), 2);
        assert_eq!(draw.alternates.len(), 1);
    }
}
