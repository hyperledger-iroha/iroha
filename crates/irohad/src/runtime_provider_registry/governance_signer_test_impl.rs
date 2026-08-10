impl sorafs_node::GovernanceDagRuntimeSigner for GovernanceSigner {
    fn handle(&self) -> &str {
        let call = self.handle_calls.fetch_add(1, Ordering::Relaxed);
        if call == 0 {
            self.handle
        } else {
            self.later_handle.unwrap_or(self.handle)
        }
    }

    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        let call = self.qualification_calls.fetch_add(1, Ordering::Relaxed);
        Ok(if call == 0 {
            self.first_qualification
        } else {
            self.later_qualification.unwrap_or(self.first_qualification)
        })
    }

    fn publisher_peer_id(&self) -> &[u8] {
        let call = self.publisher_peer_id_calls.fetch_add(1, Ordering::Relaxed);
        if call == 0 {
            &self.publisher_peer_id
        } else {
            self.later_publisher_peer_id
                .as_deref()
                .unwrap_or(&self.publisher_peer_id)
        }
    }

    fn public_key(&self) -> [u8; 32] {
        let first = self
            .key_pair
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 public key has 32 bytes");
        let call = self.public_key_calls.fetch_add(1, Ordering::Relaxed);
        if call == 0 {
            first
        } else {
            self.later_public_key.unwrap_or(first)
        }
    }

    fn sign(
        &self,
        _purpose: sorafs_node::GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        if self.sign_error {
            return Err("redacted Governance DAG signing failure".to_owned());
        }
        let key_pair = self.signing_key_pair.as_ref().unwrap_or(&self.key_pair);
        iroha_crypto::Signature::try_new(key_pair.private_key(), payload)
            .map_err(|_| "redacted Governance DAG signing failure".to_owned())?
            .payload()
            .try_into()
            .map_err(|_| "redacted Governance DAG signature width failure".to_owned())
    }
}
