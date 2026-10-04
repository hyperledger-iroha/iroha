//! Explicit local test custody resolved only against authenticated scheduled committees.

use super::*;
use iroha_data_model::sumeragi::epoch::ValidatorEpochContextV1;

impl CertifiedTestChain {
    /// Provision one new peer's local signing material without changing any chain authority.
    /// Candidate publication, election, DKG, readiness and activation still require signed work.
    ///
    /// # Errors
    /// Rejects a non-BLS peer, duplicate local peer or invalid unseated custody binding.
    pub fn provision_candidate_custody(
        &mut self,
        key: &KeyPair,
        seed: zeroize::Zeroizing<[u8; 32]>,
    ) -> Result<(), String> {
        let peer = PeerId::new(key.public_key().clone());
        let signer = KeyPairSigner::new(key).map_err(|error| error.to_string())?;
        if self.signer_for_member(&peer).is_some() {
            return Err("candidate peer already has original local custody".to_owned());
        }
        let genesis = super::super::epoch::genesis_epoch(&self.genesis)
            .map_err(|error| error.to_string())?
            .authority;
        crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1::new_unseated(
            &genesis,
            peer.clone(),
            zeroize::Zeroizing::new(*seed),
        )
        .map_err(|error| error.to_string())?;
        self.signers.push(signer);
        self.candidate_pasta_seeds.push((peer, seed));
        Ok(())
    }

    /// Borrow a provisioned BLS signer by identity, never by a stale genesis seat number.
    pub(super) fn signer_for_member(&self, peer: &PeerId) -> Option<&KeyPairSigner> {
        let key = super::super::crypto::core_key(peer.public_key()).ok()?;
        self.signers
            .iter()
            .find(|signer| signer.public_key() == &key)
    }

    /// Recover an explicit local seed owner by identity without granting it scheduling authority.
    /// The resulting holder must still derive a signer from an authenticated generation.
    ///
    /// # Errors
    /// The peer is not provisioned or its original custody no longer matches signed genesis.
    pub fn pasta_custody_for_peer(
        &self,
        peer: &PeerId,
    ) -> Result<crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1, String>
    {
        let genesis = super::super::epoch::genesis_epoch(&self.genesis)
            .map_err(|error| error.to_string())?
            .authority;
        if let Some(index) = genesis
            .validators
            .iter()
            .position(|seat| &seat.validator == peer)
        {
            return crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1::new(
                Arc::new(genesis),
                zeroize::Zeroizing::new(*self.pasta_seeds[index]),
                u32::try_from(index).expect("bounded original committee"),
            )
            .map_err(|error| error.to_string());
        }
        let (_, seed) = self
            .candidate_pasta_seeds
            .iter()
            .find(|(candidate, _)| candidate == peer)
            .ok_or_else(|| "candidate peer has no provisioned original custody".to_owned())?;
        crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1::new_unseated(
            &genesis,
            peer.clone(),
            zeroize::Zeroizing::new(**seed),
        )
        .map_err(|error| error.to_string())
    }

    /// Prove the exact seat challenge already admitted into this chain's frozen preparation.
    /// This reads the authenticated transition and does not manufacture or install credentials.
    ///
    /// # Errors
    /// Rejects absent/rebound preparation, a foreign seat, or unprovisioned local seed custody.
    pub fn prove_prepared_seat_readiness(
        &self,
        peer: &PeerId,
        authority: &iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
        context: &iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalitySeatReadinessContextV1,
    ) -> Result<
        iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityPairedPossessionProofV1,
        String,
    > {
        let view = self.state.view();
        let transition = view
            .world()
            .validator_committee_transitions()
            .get(&context.target_epoch)
            .ok_or_else(|| "prepared seat has no authenticated original transition".to_owned())?;
        let credentials = transition
            .credentials
            .as_ref()
            .ok_or_else(|| "original transition has no prepared credentials".to_owned())?;
        if &credentials.authority != authority
            || transition.readiness_context(context.validator_index)? != *context
            || authority
                .validators
                .get(context.validator_index as usize)
                .is_none_or(|seat| &seat.validator != peer)
        {
            return Err(
                "prepared seat challenge differs from its authenticated original".to_owned(),
            );
        }
        self.pasta_custody_for_peer(peer)?
            .signer_for_authority(authority)
            .map_err(|error| error.to_string())?;
        let genesis = super::super::epoch::genesis_epoch(&self.genesis)
            .map_err(|error| error.to_string())?
            .authority;
        let seed = if let Some(index) = genesis
            .validators
            .iter()
            .position(|seat| &seat.validator == peer)
        {
            &self.pasta_seeds[index]
        } else {
            &self
                .candidate_pasta_seeds
                .iter()
                .find(|(candidate, _)| candidate == peer)
                .ok_or_else(|| "prepared peer has no original seed custody".to_owned())?
                .1
        };
        crate::zk::kagemusha_v1_recursion::prove_kagemusha_mint_finality_seat_readiness_v1(
            seed, authority, context,
        )
        .map_err(|error| error.to_string())
    }

    /// Resolve historical and prospective certificates from independently authenticated State.
    pub(super) fn certificate_context(&self, height: u64) -> ValidatorEpochContextV1 {
        if height <= self.tip.0 {
            self.committed(height).commitment().schedule.current.clone()
        } else {
            assert_eq!(
                height,
                self.tip.0 + 1,
                "only the actual successor may be certified"
            );
            self.state
                .view()
                .world()
                .consensus_schedule()
                .ready(height)
                .expect("the exact successor is authorized")
                .epoch
                .clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_custody_cannot_change_or_sign_the_original_authenticated_schedule() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        let original = chain.certificate_context(2);
        let candidate = KeyPair::from_seed(vec![0xED; 32], Algorithm::BlsNormal);
        let peer = PeerId::new(candidate.public_key().clone());
        assert!(chain.pasta_custody_for_peer(&peer).is_err());
        chain
            .provision_candidate_custody(&candidate, zeroize::Zeroizing::new([0xEF; 32]))
            .unwrap();
        assert_eq!(chain.height(), 1);
        assert!(
            !chain.crypto.is_admitted(
                &super::super::super::crypto::core_key(candidate.public_key()).unwrap()
            ),
            "provisioning alone cannot admit an unseated BLS key"
        );
        assert_eq!(chain.certificate_context(2), original);
        assert!(!original.committee.iter().any(|seat| seat.validator == peer));
        let custody = chain.pasta_custody_for_peer(&peer).unwrap();
        assert!(custody.signer_for_authority(&original.authority).is_err());
        let (keys, proof) = custody.candidate_possession(1).unwrap();
        assert_eq!(keys.validator, peer);
        crate::zk::kagemusha_v1_recursion::verify_kagemusha_mint_finality_candidate_possession_v1(
            chain.network_id(),
            1,
            &keys,
            &proof,
        )
        .unwrap();
        assert!(
            chain
                .provision_candidate_custody(&candidate, zeroize::Zeroizing::new([0xF0; 32]))
                .is_err()
        );
        assert_eq!(
            chain
                .pasta_custody_for_peer(&peer)
                .unwrap()
                .candidate_possession(1)
                .unwrap()
                .0,
            keys
        );
        let wrong_algorithm = KeyPair::from_seed(vec![0xED; 32], Algorithm::Ed25519);
        assert!(
            chain
                .provision_candidate_custody(&wrong_algorithm, zeroize::Zeroizing::new([0xF1; 32]))
                .is_err()
        );
        chain.commit(Vec::new());
        assert_eq!(chain.committed(2).commitment().schedule.current, original);
        assert_eq!(
            chain
                .committed_body(2)
                .unwrap()
                .unwrap()
                .1
                .signers
                .count_ones(),
            3
        );
    }
}
