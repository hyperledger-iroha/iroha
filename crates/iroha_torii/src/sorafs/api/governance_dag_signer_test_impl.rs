impl GovernanceDagRuntimeSigner for ApiTestGovernanceDagSigner {
    fn handle(&self) -> &str {
        Self::HANDLE
    }

    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        Ok(Self::expected_qualification())
    }

    fn publisher_peer_id(&self) -> &[u8] {
        Self::PEER_ID
    }

    fn public_key(&self) -> [u8; 32] {
        self.public_key_bytes()
    }

    fn sign(
        &self,
        _purpose: sorafs_node::GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        IrohaSignature::try_new(self.key_pair.private_key(), payload)
            .map_err(|_| "Torii API test Governance DAG signing failed".to_owned())?
            .payload()
            .try_into()
            .map_err(|_| "Torii API test Governance DAG signature width changed".to_owned())
    }
}
