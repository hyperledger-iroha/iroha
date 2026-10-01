//! Actual four installed-node current S/W read with native account witness custody.
use super::*;
use iroha_torii_shared::ordinary_wallet_current::{
    ORDINARY_WALLET_CURRENT_MAX_BYTES_V1 as MAX, ORDINARY_WALLET_CURRENT_ROUTE_V1 as ROUTE,
    OrdinaryWalletCurrentOriginalV1 as Original, OrdinaryWalletCurrentRequestV1 as Request,
};

impl KagemushaNativeCurrentWalletReadV1 {
    /// Fetch the genuine current original from each installed reporting node, then authenticate
    /// their complete certified cut, exact S/W values and signatures under this retained owner.
    /// Native owns the private account key, clock, read nonce and all four targets. No caller
    /// snapshot, bearer authority, current timestamp or decoded public key enters this producer.
    /// # Errors
    /// Refuses changed account/runtime/root/prefix, mismatched node originals, canonical HTTP
    /// authentication failure, transport bounds or the finite original suspend-inclusive budget.
    pub fn fetch_and_authenticate(self) -> Result<VerifiedEnrollmentWalletSignatoryV1> {
        self.inventory.recheck()?;
        self.challenge.remaining_native_budget()?;
        let transport = self.inventory.clock_transport(&self.account, &self.clock)?;
        let request = Request {
            version: 1,
            network_id: *self.account.network_id(),
            height: self.height,
            request_nonce: self.nonce(),
            signatory: self.signatory.clone(),
            wallet: self.account.authority().clone(),
        };
        let body = request.canonical_wire()?;
        let mut first: Option<Original> = None;
        let mut statements = Vec::with_capacity(4);
        for client in &transport.nodes {
            self.inventory.recheck()?;
            let interval = self
                .clock
                .lock()
                .map_err(|_| eyre!("Native clock unavailable"))?
                .current_native_time_interval()
                .map_err(|_| eyre!("Native current interval rejected"))?;
            let url = join_torii_url(&client.torii_url, ROUTE.trim_start_matches('/'));
            let witness = self.current_wallet_witness(&url, &body, interval.lower_ms())?;
            self.clock
                .lock()
                .map_err(|_| eyre!("Native clock unavailable"))?
                .current_native_time_interval()
                .map_err(|_| eyre!("Native signing interval expired"))?;
            let budget = self.challenge.remaining_native_budget()?;
            let response = client.send_builder(
                client
                    .request_without_canonical_account_auth(HttpMethod::POST, url)
                    .header(
                        HEADER_WITNESS,
                        canonical_request_witness_header_value(&witness)?,
                    )
                    .header("Content-Type", "application/x-norito")
                    .header("Accept", "application/x-norito")
                    .header(
                        "X-Iroha-Finality-Challenge",
                        hex::encode(request.request_nonce),
                    )
                    .body(body.clone())
                    .timeout(budget)
                    .max_response_bytes(MAX),
            )?;
            self.challenge.remaining_native_budget()?;
            self.clock
                .lock()
                .map_err(|_| eyre!("Native clock unavailable"))?
                .current_native_time_interval()
                .map_err(|_| eyre!("Native response interval expired"))?;
            ensure!(
                response.status() == StatusCode::OK,
                "Native current wallet HTTP original unavailable"
            );
            ensure!(
                response
                    .headers()
                    .get("Content-Type")
                    .and_then(|v| v.to_str().ok())
                    == Some("application/x-norito"),
                "Native current wallet response codec rejected"
            );
            let bytes = response.body();
            ensure!(
                !bytes.is_empty() && bytes.len() <= MAX,
                "Native current wallet response bound rejected"
            );
            let original: Original =
                norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(MAX))?;
            // The sole typed first-release decoder checks the canonical frame/field layout. The
            // full originals are compared structurally; no cloned snapshot is retained per node.
            original.validate_request_correlation(&request)?;
            if let Some(retained) = &first {
                require_same_cut(retained, &original)?;
                statements.push(original.attestation);
            } else {
                statements.push(original.attestation.clone());
                first = Some(original);
            }
            self.challenge.remaining_native_budget()?;
            self.inventory.recheck()?;
        }
        let first = first.ok_or_else(|| eyre!("Native current wallet originals absent"))?;
        let proof = norito::encode_canonical(&first.attestation.body.finality_proof)?;
        let statements = statements
            .try_into()
            .map_err(|_| eyre!("Native current wallet node count rejected"))?;
        self.authenticate_typed(KagemushaOrdinaryNativeCurrentWalletOriginalV1 {
            proof,
            world_snapshot: first.world_snapshot,
            signatory_value: first.signatory_value,
            wallet_value: first.wallet_value,
            statements,
        })
    }
    fn current_wallet_witness(
        &self,
        url: &Url,
        body: &[u8],
        timestamp_ms: u64,
    ) -> Result<CanonicalRequestWitnessV1> {
        self.challenge.remaining_native_budget()?;
        self.inventory.require_account_transport(&self.account)?;
        let mut witness = CanonicalRequestWitnessV1 {
            schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
            subject_account: self.account.authority().clone(),
            timestamp_ms,
            nonce: Client::signed_request_nonce()?,
            canonical_request_hash: canonical_network_request_hash(
                self.account.network_id(),
                &HttpMethod::POST,
                url,
                body,
            )?,
            signatures: Vec::new(),
        };
        let message = canonical_request_witness_message(&witness)?;
        witness.signatures.push(
            iroha_data_model::soracloud::CanonicalRequestSignatureWitnessV1 {
                signer: self.account.context.key_pair.public_key().clone(),
                signature: Signature::try_new(
                    self.account.context.key_pair.private_key(),
                    &message,
                )?,
            },
        );
        self.challenge.remaining_native_budget()?;
        Ok(witness)
    }
}
fn require_same_cut(a: &Original, b: &Original) -> Result<()> {
    ensure!(
        a.request == b.request
            && a.world_snapshot == b.world_snapshot
            && a.signatory_value == b.signatory_value
            && a.wallet_value == b.wallet_value
            && a.attestation.body.finality_proof == b.attestation.body.finality_proof,
        "Native installed nodes returned different exact current wallet cuts"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::participant_enrollment_request::NativeCustodyFixture;
    #[test]
    fn current_wallet_four_nodes_cannot_select_different_cut_accounts_or_request() {
        let fixture = NativeCustodyFixture::new();
        let original = fixture.wallet_original([7; 32]);
        original
            .validate_request_correlation(&original.request)
            .unwrap();
        require_same_cut(&original, &original).unwrap();
        let mut changed = fixture.wallet_original([8; 32]);
        assert!(require_same_cut(&original, &changed).is_err());
        changed = fixture.wallet_original([7; 32]);
        changed.world_snapshot.schema_hash = Hash::new(b"changed schema");
        assert!(require_same_cut(&original, &changed).is_err());
        changed = fixture.wallet_original([7; 32]);
        changed.request.wallet = changed.request.signatory.clone();
        assert!(require_same_cut(&original, &changed).is_err());
        changed = fixture.wallet_original([7; 32]);
        changed.world_snapshot.entries[0].value_hash = Hash::new(b"changed row");
        assert!(require_same_cut(&original, &changed).is_err());
    }
}
