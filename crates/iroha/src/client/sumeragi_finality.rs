// Embedded-certificate finality transport; no alternate decoding or conversion.

const SUMERAGI_FINALITY_RESPONSE_MAX_BYTES: usize =
    2 * iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES + 4 * 1024 * 1024;

// The public gateway may serve several independently selected committee members
// at one root. Selection affects transport only; the signed response must still
// authenticate the exact expected node, challenge, network and certified state.
fn sumeragi_attestation_path(
    height: NonZeroU64,
    expected_node: &iroha_model_base::peer::PeerId,
) -> Result<String> {
    if expected_node.public_key().try_algorithm()? != iroha_crypto::Algorithm::BlsNormal {
        return Err(eyre!("finality attestation requires a selected BLS node"));
    }
    let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_ATTESTATION
        .path()
        .replace("{height}", &height.get().to_string());
    // Canonical BLS PeerId text contains only ASCII letters and digits; this
    // query is constructed from typed identity, never caller-supplied URL text.
    Ok(format!("{path}?peer_id={expected_node}"))
}

impl Client {
    /// Read current provider discovery and authenticate it against an independently selected block.
    ///
    /// The caller supplies its qualified native World schema and a fresh finality
    /// decision. Neither the endpoint nor this response can select those trust inputs.
    /// This returns no download token and grants no account spending authority.
    /// # Errors
    /// Invalid selectors, bounded transport/codec failure, stale or substituted
    /// native state, revoked admission, or a mismatched/invalid provider advert.
    pub fn get_provider_discovery(
        &self,
        provider: iroha_data_model::sorafs::capacity::ProviderId,
        native_schema: iroha_crypto::Hash,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
        now_unix_seconds: u64,
    ) -> Result<iroha_data_model::sorafs::provider_admission::discovery::VerifiedProviderDiscoveryV1>
    {
        let proof = self.read_provider_discovery_frame(provider, block)?;
        let verified = proof.verify(
            self.network_id,
            provider,
            native_schema,
            block,
            now_unix_seconds,
        )?;
        self.ensure_activation_evidence_deadline()?;
        Ok(verified)
    }

    /// Read the explicitly admitted account-download policy and current native token signer.
    /// The caller supplies its independently qualified schema and fresh certified decision.
    /// # Errors
    /// Refuses missing policy, expired/revoked custody, wrong exact scope, or bounded transport failure.
    pub fn get_account_read_provider_discovery(
        &self, provider: iroha_data_model::sorafs::capacity::ProviderId,
        native_schema: iroha_crypto::Hash,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
        now_unix_ms: u64,
    ) -> Result<iroha_data_model::sorafs::provider_admission::discovery::account_read::VerifiedAccountReadProviderV1>{
        let proof = self.read_provider_discovery_frame(provider, block)?;
        let verified = proof.verify_account_read(
            self.chain.as_ref(),
            self.network_id,
            provider,
            native_schema,
            block,
            now_unix_ms,
        )?;
        self.ensure_activation_evidence_deadline()?;
        Ok(verified)
    }

    /// Read current native `StreamToken` control for an independently selected signer binding.
    ///
    /// The caller supplies the complete expected binding, qualified World schema and fresh
    /// certified decision. This reuses the bounded provider-discovery transport and returns only
    /// authenticated control evidence. Unenrolled, expired and revoked custody remain readable;
    /// policy and revocation must be checked separately before enrollment or signing. No signing,
    /// enrollment, token, provider or account spending authority is granted by this read.
    ///
    /// # Errors
    /// Rejects invalid independent selectors before transport, exceeded request deadlines or
    /// response bounds, noncanonical evidence, wrong exact binding, stale/substituted control,
    /// and failed provider admission or advertisement verification.
    pub fn get_stream_token_custody_control(
        &self,
        provider: iroha_data_model::sorafs::capacity::ProviderId,
        expected_binding: &sorafs_manifest::signer::custody::SignerCustodyBindingV1,
        native_schema: iroha_crypto::Hash,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
        now_unix_ms: u64,
    ) -> Result<iroha_data_model::sorafs::provider_admission::discovery::stream_token_control::VerifiedStreamTokenCustodyControlV1>{
        use sorafs_manifest::signer::protocol::{SignerPurposeBindingV1, SignerRoleV1};
        expected_binding.validate()?;
        if expected_binding.chain_id != self.chain.to_string()
            || expected_binding.network_id != *self.network_id.as_bytes()
            || expected_binding.role != SignerRoleV1::StreamToken
            || expected_binding.purpose
                != (SignerPurposeBindingV1::StreamToken {
                    provider_id: *provider.as_bytes(),
                })
        {
            return Err(eyre!(
                "stream-token control requires an independent binding on this chain, network and provider"
            ));
        }
        let proof = self.read_provider_discovery_frame(provider, block)?;
        let verified = proof.verify_stream_token_custody_control(
            self.network_id,
            provider,
            native_schema,
            expected_binding,
            block,
            now_unix_ms,
        )?;
        self.ensure_activation_evidence_deadline()?;
        Ok(verified)
    }

    /// Read native custody presence or absence without requiring admission or an advertisement.
    ///
    /// The caller independently selects the exact owner, complete signer binding, qualified
    /// native World schema and fresh certified decision. An absent record is authenticated
    /// against the complete World; HTTP absence or failure never means unconfigured custody.
    /// This grants no enrollment, current-use, token or account spending authority.
    /// # Errors
    /// Invalid independent scope before dispatch, expired deadline, bounded transport/codec
    /// failure, concealed state, changed owner, substituted binding or native preimages.
    pub fn get_stream_token_custody_state(
        &self,
        provider: iroha_data_model::sorafs::capacity::ProviderId,
        expected_owner: &iroha_data_model::account::AccountId,
        expected_binding: &sorafs_manifest::signer::custody::SignerCustodyBindingV1,
        native_schema: iroha_crypto::Hash,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
    ) -> Result<
        iroha_data_model::sorafs::stream_token_custody::proof::VerifiedStreamTokenCustodyStateV1,
    > {
        use iroha_data_model::sorafs::stream_token_custody::proof::{
            MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1, StreamTokenCustodyProofV1,
        };
        use sorafs_manifest::signer::protocol::{SignerPurposeBindingV1, SignerRoleV1};
        expected_binding.validate()?;
        block.verify_global_scope(self.network_id, self.chain.as_ref())?;
        if provider.as_bytes() == &[0; 32]
            || block.height() < 2
            || block.commitment().schedule.current.network_id != self.network_id
            || expected_binding.chain_id != self.chain.to_string()
            || expected_binding.network_id != *self.network_id.as_bytes()
            || expected_binding.role != SignerRoleV1::StreamToken
            || expected_binding.purpose
                != (SignerPurposeBindingV1::StreamToken {
                    provider_id: *provider.as_bytes(),
                })
        {
            return Err(eyre!(
                "custody state requires independently selected scope on this chain and network"
            ));
        }
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sorafs::STREAM_TOKEN_CUSTODY
            .path()
            .replace("{provider_id}", &hex::encode(provider.as_bytes()))
            .replace("{height}", &block.height().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1,
            "Failed to get native custody state",
        )?;
        let proof = StreamTokenCustodyProofV1::decode_frame(body)?;
        let verified = proof.verify(
            self.network_id,
            provider,
            expected_owner,
            expected_binding,
            native_schema,
            block,
        )?;
        self.ensure_activation_evidence_deadline()?;
        Ok(verified)
    }

    fn read_provider_discovery_frame(
        &self,
        provider: iroha_data_model::sorafs::capacity::ProviderId,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
    ) -> Result<iroha_data_model::sorafs::provider_admission::discovery::ProviderDiscoveryProofV1>
    {
        use iroha_data_model::sorafs::provider_admission::discovery::{
            MAX_PROVIDER_DISCOVERY_BYTES_V1, ProviderDiscoveryProofV1,
        };
        if provider.as_bytes() == &[0; 32]
            || block.height() < 2
            || block.commitment().schedule.current.network_id != self.network_id
        {
            return Err(eyre!(
                "provider discovery requires a nonzero provider and an independently verified block on this network"
            ));
        }
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sorafs::PROVIDER_DISCOVERY
            .path()
            .replace("{provider_id}", &hex::encode(provider.as_bytes()))
            .replace("{height}", &block.height().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            MAX_PROVIDER_DISCOVERY_BYTES_V1,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_PROVIDER_DISCOVERY_BYTES_V1,
            "Failed to get provider discovery",
        )?;
        ProviderDiscoveryProofV1::decode_frame(body).map_err(Into::into)
    }

    /// Probe the original genesis tip under a caller-authenticated current genesis verifier.
    /// A successful node signature authenticates that node's observed genesis execution;
    /// callers requiring quorum readiness must collect independently selected nodes.
    /// The transport selects that BLS node with the canonical `peer_id` query,
    /// permitting distinct committee attestations through one public gateway.
    ///
    /// # Errors
    /// Invalid independent inputs, elapsed deadline, transport, signature, root or instance mismatch.
    pub fn poll_sumeragi_genesis_readiness(
        &self,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
        trusted: &iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier,
        deadline: std::time::Instant,
    ) -> Result<GenesisFinalityReadiness> {
        if challenge == [0; 32]
            || expected_node.public_key().try_algorithm()? != iroha_crypto::Algorithm::BlsNormal
        {
            return Err(eyre!(
                "genesis readiness requires a nonzero challenge and selected BLS node"
            ));
        }
        let path = sumeragi_attestation_path(NonZeroU64::new(1).expect("genesis height"), expected_node)?;
        let deadline = self
            .http_transport
            .deadline()
            .map_or(deadline, |prior| prior.min(deadline));
        Self::ensure_genesis_readiness_deadline(deadline)?;
        let client = self.with_request_deadline(deadline);
        client.ensure_data_model_compatibility()?;
        let response = client.send_activation_evidence_read(
            &path,
            SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
            Some(challenge),
            ActivationEvidenceReadAuth::Public,
        )?;
        Self::ensure_genesis_readiness_deadline(deadline)?;
        if response.status() != StatusCode::OK {
            let failure = Self::decode_finality_attestation_failure(
                &response,
                1,
                challenge,
                expected_node,
                self.network_id,
            )?;
            Self::ensure_genesis_readiness_deadline(deadline)?;
            return Ok(GenesisFinalityReadiness::NotReady(failure.reason));
        }
        let attestation: iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation =
            Self::decode_canonical_norito_response(
                &response,
                SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                "sumeragi.genesis_readiness.read",
            )?;
        attestation.verify()?;
        let body = &attestation.body;
        if body.challenge != challenge
            || body.node_id != *expected_node
            || body.network_id != self.network_id
            || body.status.instance != trusted.instance().0
            || body.finality_proof.height() != 1
        {
            return Err(eyre!(
                "genesis readiness differs from selected request, network or instance"
            ));
        }
        let mut candidate = trusted.clone();
        candidate.verify(&body.genesis_finality_proof)?;
        candidate.verify_same_decision(&body.genesis_finality_proof, &body.finality_proof)?;
        Self::ensure_genesis_readiness_deadline(deadline)?;
        Ok(GenesisFinalityReadiness::Ready(Box::new(attestation)))
    }
    /// Read and authenticate the selected node's fresh current-consensus tip statement.
    /// The caller's independently anchored verifier must authenticate the embedded chain.
    /// An internally constructed `peer_id` query selects the expected BLS node
    /// when several committee members share this client's public Torii root.
    ///
    /// # Errors
    /// Bounded transport/codec failure, malformed proof, invalid node signature or wrong request binding.
    pub fn get_sumeragi_finality_attestation(
        &self,
        height: NonZeroU64,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation> {
        if challenge == [0; 32] {
            return Err(eyre!("finality challenge must be nonzero"));
        }
        let path = sumeragi_attestation_path(height, expected_node)?;
        self.ensure_data_model_compatibility()?;
        let response = self.send_activation_evidence_read(
            &path,
            SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
            Some(challenge),
            ActivationEvidenceReadAuth::Public,
        )?;
        if response.status() != StatusCode::OK {
            let failure = Self::decode_finality_attestation_failure(
                &response,
                height.get(),
                challenge,
                expected_node,
                self.network_id,
            )?;
            if let Some(progress) = failure.tip_mismatch {
                return Err(BridgeFinalityAttestationTipMismatch::from_response(
                    progress,
                    height,
                    challenge,
                    expected_node,
                    self.network_id,
                )?
                .into());
            }
            return Err(eyre!(
                "current finality attestation unavailable: {}",
                failure.reason.as_str()
            ));
        }
        let attestation: iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation =
            Self::decode_canonical_norito_response(
                &response,
                SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                "sumeragi.finality_attestation.read",
            )?;
        attestation.verify()?;
        if attestation.body.challenge != challenge
            || attestation.body.node_id != *expected_node
            || attestation.body.network_id != self.network_id
            || attestation.body.status.committed_height != height.get()
        {
            return Err(eyre!(
                "current finality attestation differs from exact request bindings"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(attestation)
    }

    /// Fetch one canonical current proof without choosing a trust root from the response.
    /// A structural certificate check does not authenticate its candidate committee.
    ///
    /// # Errors
    /// Bounded transport/codec error, wrong height or malformed current certificate.
    pub fn get_sumeragi_finality_proof(
        &self,
        height: NonZeroU64,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY
            .path()
            .replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let proof: iroha_data_model::sumeragi_finality::SumeragiFinalityProof =
            Self::decode_canonical_norito_response(
                &response,
                SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                "sumeragi.finality_proof.read",
            )?;
        if proof.height() != height.get() {
            return Err(eyre!(
                "current finality proof differs from requested height"
            ));
        }
        proof.decode_checked()?;
        self.ensure_activation_evidence_deadline()?;
        Ok(proof)
    }

    /// Fetch this child's compact registration using the retained owner listener token.
    ///
    /// The bounded response must match this client's child network and chain label and the
    /// caller's independently selected parent/dataspace scope. This does not establish trust:
    /// compare the entire registration with the retained signed-genesis registration before
    /// authorizing it on the parent. No private genesis or transaction body is transported.
    ///
    /// # Errors
    /// Invalid scope, HTTP/media/deadline/size failure, malformed registration, or scope,
    /// network, or child-chain substitution.
    pub fn get_private_root_registration(
        &self,
        expected_scope: iroha_data_model::block::consensus::SumeragiRootScope,
    ) -> Result<iroha_data_model::private_dataspace::PrivateDataspaceRegistration> {
        use iroha_data_model::{
            block::consensus::SumeragiRootScope,
            private_dataspace::{
                MAX_PRIVATE_DATASPACE_REGISTRATION_BYTES, PrivateDataspaceRegistration,
            },
        };
        if !matches!(expected_scope, SumeragiRootScope::Dataspace { .. })
            || expected_scope.validate().is_err()
        {
            return Err(eyre!(
                "private-root registration requires an exact private scope"
            ));
        }
        self.ensure_data_model_compatibility()?;
        let response = self.send_activation_evidence_read(
            iroha_torii_shared::route_catalog::sumeragi::PRIVATE_ROOT_REGISTRATION.path(),
            MAX_PRIVATE_DATASPACE_REGISTRATION_BYTES,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_PRIVATE_DATASPACE_REGISTRATION_BYTES,
            "Failed to get private root registration",
        )?;
        let registration = PrivateDataspaceRegistration::decode(body)?;
        if registration.child_network_id != self.network_id
            || registration.child_chain_id != self.chain
            || registration.scope != expected_scope
        {
            return Err(eyre!(
                "private-root registration substitutes the selected child or scope"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(registration)
    }

    /// Fetch one bounded compact anchor using this child's retained owner listener token.
    ///
    /// This checks the exact selected parent/dataspace, child network, and claimed height.
    /// Transport grants no finality authority: apply the result to the independently authorized
    /// `PrivateDataspaceAnchorState` to authenticate its quorum and contiguous ancestry.
    ///
    /// # Errors
    /// Invalid scope/genesis height, HTTP/media/deadline/size failure, malformed public
    /// certificate, private-body witness, or requested binding substitution.
    pub fn get_private_root_anchor(
        &self,
        expected_scope: iroha_data_model::block::consensus::SumeragiRootScope,
        height: NonZeroU64,
    ) -> Result<iroha_data_model::private_dataspace::PrivateDataspaceAnchor> {
        use iroha_data_model::{
            block::consensus::SumeragiRootScope,
            private_dataspace::{MAX_PRIVATE_DATASPACE_ANCHOR_BYTES, PrivateDataspaceAnchor},
        };
        let SumeragiRootScope::Dataspace {
            parent_network_id,
            dataspace_id,
        } = expected_scope
        else {
            return Err(eyre!("private-root anchor requires an exact private scope"));
        };
        if expected_scope.validate().is_err() || height.get() < 2 {
            return Err(eyre!(
                "private-root anchor requires a nonzero dataspace and non-genesis height"
            ));
        }
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::PRIVATE_ROOT_ANCHOR
            .path()
            .replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            MAX_PRIVATE_DATASPACE_ANCHOR_BYTES,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_PRIVATE_DATASPACE_ANCHOR_BYTES,
            "Failed to get private root anchor",
        )?;
        let anchor = PrivateDataspaceAnchor::decode(body)?;
        if anchor.child_network_id != self.network_id
            || anchor.parent_network_id != parent_network_id
            || anchor.dataspace_id != dataspace_id
            || anchor.height()? != height.get()
        {
            return Err(eyre!(
                "private-root anchor substitutes the selected child, scope, or height"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(anchor)
    }

    /// Fetch one bounded private-root record proof from its global parent.
    ///
    /// The response is structurally validated and bound to this client's parent network,
    /// the exact dataspace, and the requested carrier height. Transport does not authenticate
    /// finality: the recipient must independently authenticate the parent block, then call
    /// `proof.verify(dataspace_id, &verified_parent_block)` before trusting the record.
    ///
    /// # Errors
    /// Invalid selector, transport or media-type failure, oversized/noncanonical proof,
    /// malformed record, or substituted parent network, dataspace, or carrier height.
    pub fn get_private_dataspace_record_proof(
        &self,
        dataspace_id: iroha_model_base::topology::DataSpaceId,
        height: NonZeroU64,
    ) -> Result<iroha_data_model::private_dataspace::PrivateDataspaceRecordProof> {
        use iroha_data_model::private_dataspace::{
            MAX_PRIVATE_DATASPACE_RECORD_PROOF_BYTES, PrivateDataspaceRecordProof,
        };
        if dataspace_id == iroha_model_base::topology::DataSpaceId::UNIVERSAL || height.get() < 2 {
            return Err(eyre!(
                "private-root record proof requires a nonzero dataspace and non-genesis carrier"
            ));
        }
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::PRIVATE_DATASPACE_RECORD_PROOF
            .path()
            .replace("{dataspace_id}", &dataspace_id.as_u64().to_string())
            .replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            MAX_PRIVATE_DATASPACE_RECORD_PROOF_BYTES,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_PRIVATE_DATASPACE_RECORD_PROOF_BYTES,
            "Failed to get private dataspace record proof",
        )?;
        let proof = PrivateDataspaceRecordProof::decode(body)?;
        if proof.parent_network_id != self.network_id
            || proof.parent_height != height.get()
            || proof.record.dataspace_id != dataspace_id
        {
            return Err(eyre!(
                "private-root record proof substitutes the requested parent, dataspace, or height"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(proof)
    }

    /// Fetch and admit the immediate successor into the caller's independently anchored prefix.
    ///
    /// # Errors
    /// Fetch failure, invalid certificate, wrong committee or discontinuous certified chain.
    pub fn get_next_sumeragi_finality_proof(
        &self,
        height: NonZeroU64,
        verifier: &mut iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        let proof = self.get_sumeragi_finality_proof(height)?;
        let mut candidate = verifier.clone();
        candidate.verify(&proof)?;
        self.ensure_activation_evidence_deadline()?;
        *verifier = candidate;
        Ok(proof)
    }
}
impl Client {
    /// Read one complete current World original at an exact selected node and request nonce.
    /// This existing data-only endpoint requires no retail signer. Decoding grants no FI,
    /// issuer, account or monetary authority; the caller must authenticate the whole cut.
    /// # Errors
    /// Invalid independent coordinates, bounded transport/codec failure, wrong node,
    /// network, challenge, selected asset or current certified height.
    pub fn get_challenged_kagemusha_world_original(
        &self,
        asset: &iroha_data_model::asset::AssetDefinitionId,
        height: NonZeroU64,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
    ) -> Result<iroha_torii_shared::kagemusha_state::KagemushaAuthorityStateV1> {
        use iroha_torii_shared::kagemusha_state::{
            KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1 as MAX,
            KAGEMUSHA_AUTHORITY_STATE_ROUTE_PREFIX_V1 as PREFIX,
        };
        if challenge == [0; 32]
            || height.get() < 2
            || expected_node.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal
        {
            return Err(eyre!(
                "FI World read requires current exact selected coordinates"
            ));
        }
        self.ensure_data_model_compatibility()?;
        // Preserve the actual Torii mount and encode the selected asset as one path
        // segment; native IDs may contain URL-significant bytes such as '#'.
        let asset_literal = asset.to_string();
        let selected_url = join_torii_url_with_path_segments(
            &self.torii_url,
            PREFIX.trim_end_matches('/'),
            &[&asset_literal],
        );
        let path = selected_url
            .path()
            .strip_prefix(self.torii_url.path())
            .ok_or_else(|| eyre!("FI World read changed the actual Torii mount"))?;
        let response = self.send_activation_evidence_read(
            path,
            MAX,
            Some(challenge),
            ActivationEvidenceReadAuth::Public,
        )?;
        let original: iroha_torii_shared::kagemusha_state::KagemushaAuthorityStateV1 =
            Self::decode_canonical_norito_response(
                &response,
                MAX,
                "kagemusha.authority_state.read",
            )?;
        original.attestation.verify()?;
        let body = &original.attestation.body;
        if body.challenge != challenge
            || body.node_id != *expected_node
            || body.network_id != self.network_id
            || body.finality_proof.height() != height.get()
            || body.status.committed_height != height.get()
            || body.status.applied_height != height.get()
            || original.asset_definition.id() != asset
        {
            return Err(eyre!(
                "FI World original changed the actual selected request"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(original)
    }

    /// Produce request evidence using the FI's actual four Clients and independently held
    /// current verifier/proof/schema/node/asset selection. No retail private key is used.
    /// The existing account queries keep their actual FI authentication/permission checks;
    /// no grant or public owner lookup is added. The resulting value is primitive evidence,
    /// not authenticated bearer/customer status, FI release custody or nonce admission.
    /// # Errors
    /// Refuses a foreign network, bad retail signature, substituted or incomplete World,
    /// account rows from a different cut, changed selected nodes/prefix, or expired read.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate_current_ordinary_enrollment_request(
        clients: [&Self; 4],
        request: &crate::participant_enrollment_request::ParticipantEnrollmentRequestV1<'_>,
        signature: &iroha_crypto::Signature,
        nodes: &[crate::participant_enrollment_request::SelectedEnrollmentReadNodeV1; 4],
        verifier: &iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier,
        proof: &iroha_data_model::sumeragi_finality::SumeragiFinalityProof,
        compiled_schema: iroha_crypto::Hash,
        asset: &iroha_data_model::asset::AssetDefinitionId,
    ) -> Result<crate::participant_enrollment_request::VerifiedParticipantEnrollmentRequestV1> {
        use crate::participant_enrollment_request::{
            EnrollmentWalletReadChallengeV1, authenticate_fi_current_request_cut,
        };
        use iroha_data_model::{IntoKeyValue as _, query::account::prelude::FindAccountById};
        let message = request.signing_message()?;
        signature.verify(
            request
                .signatory
                .try_signatory()
                .ok_or_else(|| eyre!("FI request S is not Ed"))?,
            &message,
        )?;
        if clients
            .iter()
            .any(|client| client.network_id != *request.network_id)
        {
            return Err(eyre!(
                "FI read Clients differ from the original request network"
            ));
        }
        let block = verifier.verify_retained_decision(proof)?;
        let height = NonZeroU64::new(block.height())
            .filter(|height| height.get() >= 2)
            .ok_or_else(|| eyre!("FI read has no current non-genesis prefix"))?;
        let challenge = EnrollmentWalletReadChallengeV1::for_request(request)?;
        let mut snapshot = None;
        let mut statements = Vec::with_capacity(4);
        for (client, selected) in clients.iter().zip(nodes) {
            let deadline = std::time::Instant::now()
                .checked_add(challenge.remaining_native_budget()?)
                .ok_or_else(|| eyre!("FI read deadline overflow"))?;
            let original = client
                .with_request_deadline(deadline)
                .get_challenged_kagemusha_world_original(
                    asset,
                    height,
                    challenge.bytes(),
                    &selected.peer_id,
                )?;
            challenge.remaining_native_budget()?;
            // A valid alternate certificate for the same retained decision is not a
            // different current cut; use the sole existing native finality verifier.
            verifier.verify_same_decision(proof, &original.attestation.body.finality_proof)?;
            if let Some(first) = &snapshot {
                if first != &original.world_snapshot {
                    return Err(eyre!("FI nodes returned different complete World cuts"));
                }
            } else {
                snapshot = Some(original.world_snapshot);
            }
            statements.push(original.attestation);
        }
        // These original values may be read later than the snapshot, but cannot be accepted
        // unless their exact model AccountValue bytes belong to that same certified World.
        let deadline = std::time::Instant::now()
            .checked_add(challenge.remaining_native_budget()?)
            .ok_or_else(|| eyre!("FI read deadline overflow"))?;
        let reader = clients[0].with_request_deadline(deadline);
        let signatory = reader.query_single(FindAccountById::new(request.signatory.clone()))?;
        challenge.remaining_native_budget()?;
        let wallet = reader.query_single(FindAccountById::new(request.wallet.clone()))?;
        challenge.remaining_native_budget()?;
        let statements = statements
            .try_into()
            .map_err(|_| eyre!("FI read node count changed"))?;
        authenticate_fi_current_request_cut(
            challenge,
            request,
            signature,
            verifier,
            proof,
            compiled_schema,
            &snapshot.ok_or_else(|| eyre!("FI complete World absent"))?,
            signatory.into_key_value(),
            wallet.into_key_value(),
            nodes,
            &statements,
        )
    }
}
