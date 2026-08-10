impl sorafs_node::GovernanceDagRuntimeSigner for GovernanceDagPublisherBindingSigner {
    fn handle(&self) -> &'static str {
        GOVERNANCE_DAG_PUBLISHER_HANDLE
    }

    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        Ok(
            sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                1,
                GOVERNANCE_DAG_PUBLISHER_POLICY_DIGEST,
            ),
        )
    }

    fn publisher_peer_id(&self) -> &[u8] {
        GOVERNANCE_DAG_PUBLISHER_PEER_ID.as_bytes()
    }

    fn public_key(&self) -> [u8; 32] {
        self.public_key_bytes()
    }

    fn sign(
        &self,
        _purpose: sorafs_node::GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        iroha_crypto::Signature::try_new(self.key_pair.private_key(), payload)
            .map_err(|_| "deterministic Governance DAG signer refused request".to_owned())?
            .payload()
            .try_into()
            .map_err(|_| "deterministic Governance DAG signature width changed".to_owned())
    }
}
