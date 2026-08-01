    // for `collect`
    use std::{
        collections::HashSet,
        net::SocketAddr,
        num::{NonZeroU32, NonZeroU64, NonZeroUsize},
        path::PathBuf,
        str::FromStr,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use super::*;
    use axum::{
        extract::State,
        http::{HeaderMap, HeaderValue, Method, Request, StatusCode},
    };
    #[cfg(feature = "app_api")]
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as JWT_BASE64};
    use futures::executor;
    use http_body_util::BodyExt as _;
    use iroha_config::parameters::actual;
    use iroha_core::{query::store::LiveQueryStore, state::State as IrohaState};
    use iroha_crypto::{
        Algorithm, BfvEvaluationKeyBundle, BfvIdentifierPublicParameters, BfvParameters, Hash,
        KeyPair, RamLfeBackend, RamLfeVerificationMode, Signature as IrohaSignature, SignatureOf,
        bfv_affine_policy_commitment, bfv_programmed_policy_commitment_with_program,
        default_bfv_programmed_hidden_program, derive_identifier_key_material_from_seed,
        encrypt_identifier_from_seed, ram_lfe_bfv_parameters_v1, ram_lfe_output_hash,
        try_bfv_programmed_public_parameters_with_program,
    };
    use iroha_data_model::{
        ChainId, Identifiable, Registrable, ValidationFail,
        account::rekey::AccountAlias,
        account::{Account, AccountId, OpaqueAccountId},
        block::{BlockHeader, BlockSignature, SignedBlock},
        domain::{Domain, DomainId},
        identifier::{IdentifierNormalization, IdentifierPolicy, IdentifierPolicyId},
        isi::identifier::{ActivateIdentifierPolicy, ClaimIdentifier, RegisterIdentifierPolicy},
        isi::ram_lfe::{ActivateRamLfeProgramPolicy, RegisterRamLfeProgramPolicy},
        name::Name,
        nexus::{
            AxtPolicySnapshot, AxtRejectContext, AxtRejectReason, DataSpaceId, LaneId,
            UniversalAccountId,
        },
        permission::Permission,
        prelude::{Parameter, Quantity},
        proof::{ProofId, ProofRecord, ProofStatus, VerifyingKeyId, VerifyingKeyRecord},
        ram_lfe::{
            RamLfeOutputOpening, RamLfeOutputOpeningPayload, RamLfeProgramId, RamLfeProgramPolicy,
        },
        role::{Role, RoleId},
        transaction::{
            IvmBytecode, IvmProved,
            signed::{TransactionBuilder, TransactionResultInner},
        },
    };
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias,
    };
    use iroha_test_samples::ALICE_ID;
    #[cfg(feature = "app_api")]
    use jsonwebtoken::EncodingKey;
    use nonzero_ext::nonzero;

    const PREBUILT_QUARANTINE_PROVIDER_HANDLE: &str = "kms://moderation/quarantine/primary";
    const PREBUILT_QUARANTINE_PROVIDER_QUALIFICATION:
        sorafs_node::ModerationQuarantineKeyProviderQualificationV1 =
        sorafs_node::ModerationQuarantineKeyProviderQualificationV1::new(1, [0x51; 32]);

    async fn assert_default_body_limit_boundary(limit: usize) {
        use tower::ServiceExt as _;

        let router = axum::Router::new().route(
            "/probe",
            axum::routing::post(move |body: Bytes| async move {
                assert_eq!(body.len(), limit);
                StatusCode::NO_CONTENT
            })
            .layer(DefaultBodyLimit::max(limit)),
        );
        let boundary = Request::builder()
            .method(Method::POST)
            .uri("/probe")
            .body(Body::from(vec![0_u8; limit]))
            .expect("boundary request");
        assert_eq!(
            router
                .clone()
                .oneshot(boundary)
                .await
                .expect("boundary response")
                .status(),
            StatusCode::NO_CONTENT
        );

        let one_over = Request::builder()
            .method(Method::POST)
            .uri("/probe")
            .body(Body::from(vec![0_u8; limit.saturating_add(1)]))
            .expect("one-over request");
        assert_eq!(
            router
                .oneshot(one_over)
                .await
                .expect("one-over response")
                .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn sorafs_protocol_body_limits_admit_boundary_and_reject_one_over() {
        for limit in [
            0,
            sorafs_manifest::provider_advert::PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1,
            crate::routing::POR_PROOF_SUBMISSION_MAX_HTTP_BODY_BYTES_V1,
            crate::routing::POR_VERDICT_SUBMISSION_MAX_HTTP_BODY_BYTES_V1,
            sorafs_manifest::por::PROVIDER_VRF_SUBMISSION_MAX_CANONICAL_BYTES_V1,
            sorafs_node::orderbook_transaction_forwarder::
                ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1,
        ] {
            assert_default_body_limit_boundary(limit).await;
        }
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn bodyless_por_and_orderbook_get_mounts_have_zero_body_limits() {
        let compact_source: String = include_str!("../../lib.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for (route, handler) in [
            ("SORAFS_POR_STATUS_GET", "handler_get_sorafs_por_status"),
            ("SORAFS_POR_EXPORT_GET", "handler_get_sorafs_por_export"),
            (
                "SORAFS_POR_INGESTION_BY_MANIFEST_DIGEST_HEX_GET",
                "sorafs::api::handle_get_sorafs_por_ingestion",
            ),
            (
                "SORAFS_POR_REPORT_BY_ISO_WEEK_GET",
                "handler_get_sorafs_por_report",
            ),
            (
                "SORAFS_ORDERBOOK_RECEIPTS_GET",
                "sorafs::api::handle_get_sorafs_orderbook_receipts",
            ),
            (
                "SORAFS_ORDERBOOK_BOOK_GET",
                "sorafs::api::handle_get_sorafs_orderbook_book",
            ),
            (
                "SORAFS_ORDERBOOK_TRADES_GET",
                "sorafs::api::handle_get_sorafs_orderbook_trades",
            ),
            (
                "SORAFS_ORDERBOOK_CHANNELS_GET",
                "sorafs::api::handle_get_sorafs_orderbook_channels",
            ),
            (
                "SORAFS_ORDERBOOK_EVENTS_GET",
                "sorafs::api::handle_get_sorafs_orderbook_events",
            ),
            (
                "SORAFS_ORDERBOOK_EVENTS_STREAM_GET",
                "sorafs::api::handle_get_sorafs_orderbook_events_stream",
            ),
            (
                "SORAFS_ORDERBOOK_EVENTS_WS_GET",
                "sorafs::api::handle_get_sorafs_orderbook_events_ws",
            ),
        ] {
            let expected = format!(
                "&route_catalog::contracts_and_verification_keys::{route},catalog_get({handler}).layer(DefaultBodyLimit::max(0))"
            );
            assert!(
                compact_source.contains(&expected),
                "{route} must reject every non-empty request body"
            );
        }
    }

    #[derive(Debug)]
    struct PrebuiltQuarantineKeyWrapper;

    impl sorafs_node::ModerationQuarantineKeyWrapper for PrebuiltQuarantineKeyWrapper {
        fn provider_handle(&self) -> &str {
            PREBUILT_QUARANTINE_PROVIDER_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<
            sorafs_node::ModerationQuarantineKeyProviderQualificationV1,
            sorafs_node::ModerationQuarantineKeyProviderReadinessErrorV1,
        > {
            Ok(PREBUILT_QUARANTINE_PROVIDER_QUALIFICATION)
        }

        fn active_key_id(&self) -> &str {
            "kms:test/torii-prebuilt-quarantine"
        }

        fn wrap_dek(
            &self,
            _context_digest: [u8; 32],
            _dek: &[u8; 32],
        ) -> Result<Vec<u8>, sorafs_node::ModerationQuarantineKeyOperationErrorV1> {
            Err(sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected)
        }

        fn unwrap_dek(
            &self,
            _key_id: &str,
            _context_digest: [u8; 32],
            _wrapped_dek: &[u8],
        ) -> Result<[u8; 32], sorafs_node::ModerationQuarantineKeyOperationErrorV1> {
            Err(sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected)
        }
    }

    fn prebuilt_quarantine_provider_config(
        qualification: sorafs_node::ModerationQuarantineKeyProviderQualificationV1,
    ) -> actual::SorafsModerationQuarantineKeyProviderBinding {
        actual::SorafsModerationQuarantineKeyProviderBinding {
            handle: PREBUILT_QUARANTINE_PROVIDER_HANDLE.to_owned(),
            revision: qualification.revision(),
            policy_digest: qualification.policy_digest(),
        }
    }

    #[test]
    #[should_panic(
        expected = "injected SoraFS node quarantine-key provider binding does not match torii.sorafs.storage"
    )]
    fn prebuilt_sorafs_node_rejects_mismatched_quarantine_key_provider_binding() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt SoraFS node temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt SoraFS node temp dir");
        let retained_config = sorafs_node::config::StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .moderation_quarantine_key_provider(Some(prebuilt_quarantine_provider_config(
                PREBUILT_QUARANTINE_PROVIDER_QUALIFICATION,
            )))
            .build();
        let key_wrapper: Arc<dyn sorafs_node::ModerationQuarantineKeyWrapper> =
            Arc::new(PrebuiltQuarantineKeyWrapper);
        let node = sorafs_node::NodeHandle::try_new_with_quarantine_key_wrapper(
            retained_config,
            Arc::clone(&key_wrapper),
        )
        .expect("start prebuilt SoraFS node with exact provider binding");
        assert!(node.uses_moderation_quarantine_key_wrapper(&key_wrapper));

        let substituted_config = sorafs_node::config::StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .moderation_quarantine_key_provider(Some(prebuilt_quarantine_provider_config(
                sorafs_node::ModerationQuarantineKeyProviderQualificationV1::new(2, [0x52; 32]),
            )))
            .build();
        assert_prebuilt_sorafs_quarantine_key_provider_binding(&node, &substituted_config);
    }

    const PREBUILT_PRIVACY_PRF_HANDLE: &str = "threshold-prf:transparency:primary";
    const PREBUILT_PRIVACY_ANCHOR_HANDLE: &str = "governance-dag:transparency:primary";
    const PREBUILT_TRANSPARENCY_LEADER_LEASE_HANDLE: &str =
        "sealed-cas:transparency:leader-primary";
    const PREBUILT_FENCED_PRIVACY_HANDLE: &str = "governance-cas:transparency:privacy-primary";
    const PREBUILT_FENCED_PRIVACY_POLICY_DIGEST: [u8; 32] = [0xF7; 32];
    const PREBUILT_GOVERNANCE_SIGNER_HANDLE: &str = "pkcs11:governance:primary";
    const PREBUILT_GOVERNANCE_SIGNER_PEER_ID: &[u8] = b"governance-torii-primary";
    const PREBUILT_GOVERNANCE_SIGNER_POLICY_DIGEST: [u8; 32] = [0x97; 32];
    const PREBUILT_GOVERNANCE_CHECKPOINT_STORE_HANDLE: &str =
        "sealed:governance:producer-checkpoint-primary";
    const PREBUILT_GOVERNANCE_CHECKPOINT_STORE_POLICY_DIGEST: [u8; 32] = [0x96; 32];

    #[derive(Debug)]
    struct PrebuiltGovernanceDagSigner {
        key_pair: KeyPair,
    }

    impl PrebuiltGovernanceDagSigner {
        fn new() -> Self {
            Self::from_seed(0x97)
        }

        fn from_seed(seed: u8) -> Self {
            Self {
                key_pair: KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("derive prebuilt Governance DAG signer key"),
            }
        }

        fn public_key_bytes(&self) -> [u8; 32] {
            let (algorithm, bytes) = self
                .key_pair
                .public_key()
                .try_to_bytes()
                .expect("serialize prebuilt Governance DAG public key");
            assert_eq!(algorithm, Algorithm::Ed25519);
            bytes.try_into().expect("Ed25519 public key width")
        }
    }

    impl sorafs_node::GovernanceDagRuntimeSigner for PrebuiltGovernanceDagSigner {
        fn handle(&self) -> &str {
            PREBUILT_GOVERNANCE_SIGNER_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    PREBUILT_GOVERNANCE_SIGNER_POLICY_DIGEST,
                ),
            )
        }

        fn publisher_peer_id(&self) -> &[u8] {
            PREBUILT_GOVERNANCE_SIGNER_PEER_ID
        }

        fn public_key(&self) -> [u8; 32] {
            self.public_key_bytes()
        }

        fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String> {
            IrohaSignature::try_new(self.key_pair.private_key(), payload)
                .map_err(|_| "prebuilt Governance DAG signer refused request".to_owned())?
                .payload()
                .try_into()
                .map_err(|_| "prebuilt Governance DAG signature width changed".to_owned())
        }
    }

    #[derive(Debug)]
    struct PrebuiltGovernanceDagCheckpointStoreState {
        records: [Option<sorafs_node::GovernanceDagSealedStateRecord>; 6],
        generation_floors: [u64; 6],
    }

    impl Default for PrebuiltGovernanceDagCheckpointStoreState {
        fn default() -> Self {
            Self {
                records: std::array::from_fn(|_| None),
                generation_floors: [0; 6],
            }
        }
    }

    #[derive(Debug)]
    struct PrebuiltGovernanceDagCheckpointStore {
        handle: &'static str,
        qualification: sorafs_node::GovernanceDagRuntimeProviderQualificationV1,
        state: Mutex<PrebuiltGovernanceDagCheckpointStoreState>,
        qualification_refuse: AtomicBool,
    }

    impl PrebuiltGovernanceDagCheckpointStore {
        fn exact() -> Self {
            Self::with_binding(
                PREBUILT_GOVERNANCE_CHECKPOINT_STORE_HANDLE,
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    PREBUILT_GOVERNANCE_CHECKPOINT_STORE_POLICY_DIGEST,
                ),
            )
        }

        fn with_binding(
            handle: &'static str,
            qualification: sorafs_node::GovernanceDagRuntimeProviderQualificationV1,
        ) -> Self {
            Self {
                handle,
                qualification,
                state: Mutex::new(PrebuiltGovernanceDagCheckpointStoreState::default()),
                qualification_refuse: AtomicBool::new(false),
            }
        }

        const fn slot_index(slot: sorafs_node::GovernanceDagSealedStateSlot) -> usize {
            match slot {
                sorafs_node::GovernanceDagSealedStateSlot::Checkpoint => 0,
                sorafs_node::GovernanceDagSealedStateSlot::PublishIntent => 1,
                sorafs_node::GovernanceDagSealedStateSlot::ProducerCheckpoint => 2,
                sorafs_node::GovernanceDagSealedStateSlot::ProducerPublishIntent => 3,
                sorafs_node::GovernanceDagSealedStateSlot::IpfsRequestReplay => 4,
                sorafs_node::GovernanceDagSealedStateSlot::SignedHeadRequestReplay => 5,
            }
        }

        fn refuse_qualification(&self) {
            self.qualification_refuse
                .store(true, AtomicOrdering::SeqCst);
        }
    }

    impl sorafs_node::GovernanceDagSealedCheckpointStore for PrebuiltGovernanceDagCheckpointStore {
        fn handle(&self) -> &str {
            self.handle
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            if self.qualification_refuse.load(AtomicOrdering::SeqCst) {
                return Err("checkpoint credential must remain redacted".to_owned());
            }
            Ok(self.qualification)
        }

        fn load(
            &self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
        ) -> Result<Option<sorafs_node::GovernanceDagSealedStateRecord>, String> {
            let state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            Ok(state.records[Self::slot_index(slot)].clone())
        }

        fn compare_and_swap(
            &self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
            expected_revision: Option<[u8; 32]>,
            next: sorafs_node::GovernanceDagSealedStateRecord,
        ) -> Result<(), String> {
            let index = Self::slot_index(slot);
            let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            if state.records[index].as_ref().map(|record| record.revision) != expected_revision {
                return Err("compare-and-swap conflict".to_owned());
            }
            if next.generation <= state.generation_floors[index]
                || next.payload.is_empty()
                || !next.has_valid_revision(slot)
            {
                return Err("invalid or non-monotonic record".to_owned());
            }
            state.generation_floors[index] = next.generation;
            state.records[index] = Some(next);
            Ok(())
        }

        fn delete(
            &self,
            slot: sorafs_node::GovernanceDagSealedStateSlot,
            expected_revision: [u8; 32],
        ) -> Result<(), String> {
            let index = Self::slot_index(slot);
            let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            if state.records[index].as_ref().map(|record| record.revision)
                != Some(expected_revision)
            {
                return Err("delete conflict".to_owned());
            }
            state.records[index] = None;
            Ok(())
        }
    }

    struct PrebuiltPrivacyPrfProvider;

    impl sorafs_node::PrivacyCyclePrfProviderV1 for PrebuiltPrivacyPrfProvider {
        fn derive_cycle_output(
            &self,
            _request: &sorafs_node::PrivacyCyclePrfRequestV1,
        ) -> Result<sorafs_node::PrivacyCyclePrfOutputV1, sorafs_node::PrivacyCyclePrfProviderErrorV1>
        {
            sorafs_node::PrivacyCyclePrfOutputV1::new([0xA5; 32])
                .map_err(|_| sorafs_node::PrivacyCyclePrfProviderErrorV1::Internal)
        }
    }

    impl sorafs_node::ProductionTransparencyRuntimeProviderV1 for PrebuiltPrivacyPrfProvider {
        fn handle(&self) -> &str {
            PREBUILT_PRIVACY_PRF_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, String> {
            Ok(sorafs_node::TransparencyRuntimeProviderQualificationV1::new(1, [0xC7; 32]))
        }
    }

    struct PrebuiltPrivacyReleaseAnchor;

    impl sorafs_node::PrivacyReleaseAnchorV1 for PrebuiltPrivacyReleaseAnchor {
        fn finalized_head(
            &self,
            query_id: [u8; 32],
        ) -> Result<sorafs_node::PrivacyReleaseAnchorHeadV1, sorafs_node::PrivacyReleaseAnchorErrorV1>
        {
            Ok(sorafs_node::PrivacyReleaseAnchorHeadV1::genesis(query_id))
        }

        fn compare_and_set_finalized_head(
            &self,
            _expected: sorafs_node::PrivacyReleaseAnchorHeadV1,
            _next: sorafs_node::PrivacyReleaseAnchorHeadV1,
            _lease: &sorafs_node::TransparencyLeaderLeaseGrantV1,
        ) -> Result<(), sorafs_node::PrivacyReleaseAnchorErrorV1> {
            Ok(())
        }
    }

    impl sorafs_node::ProductionTransparencyRuntimeProviderV1 for PrebuiltPrivacyReleaseAnchor {
        fn handle(&self) -> &str {
            PREBUILT_PRIVACY_ANCHOR_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, String> {
            Ok(sorafs_node::TransparencyRuntimeProviderQualificationV1::new(1, [0xD7; 32]))
        }
    }

    struct PrebuiltTransparencyLeaderLeaseProvider;

    impl sorafs_node::TransparencyLeaderLeaseProviderV1 for PrebuiltTransparencyLeaderLeaseProvider {
        fn acquire(
            &self,
            _request: &sorafs_node::TransparencyLeaderLeaseAcquireRequestV1,
        ) -> Result<
            sorafs_node::TransparencyLeaderLeaseGrantV1,
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
        > {
            Err(sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Internal)
        }

        fn renew(
            &self,
            _request: &sorafs_node::TransparencyLeaderLeaseRenewRequestV1,
        ) -> Result<
            sorafs_node::TransparencyLeaderLeaseGrantV1,
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
        > {
            Err(sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Internal)
        }

        fn release(
            &self,
            _request: &sorafs_node::TransparencyLeaderLeaseReleaseRequestV1,
        ) -> Result<
            sorafs_node::TransparencyLeaderLeaseReleaseReceiptV1,
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
        > {
            Err(sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Internal)
        }
    }

    impl sorafs_node::ProductionTransparencyRuntimeProviderV1
        for PrebuiltTransparencyLeaderLeaseProvider
    {
        fn handle(&self) -> &str {
            PREBUILT_TRANSPARENCY_LEADER_LEASE_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, String> {
            Ok(sorafs_node::TransparencyRuntimeProviderQualificationV1::new(1, [0xE7; 32]))
        }
    }

    #[derive(Debug)]
    struct PrebuiltFencedTransparencyProvider;

    impl sorafs_node::FencedTransparencyPublisherV1 for PrebuiltFencedTransparencyProvider {
        fn handle(&self) -> &str {
            PREBUILT_FENCED_PRIVACY_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    PREBUILT_FENCED_PRIVACY_POLICY_DIGEST,
                ),
            )
        }

        fn compare_and_append_privacy(
            &self,
            _request: &sorafs_node::FencedPrivacyPublicationRequestV1,
        ) -> Result<
            sorafs_node::FencedPrivacyPublicationReceiptV1,
            sorafs_node::FencedTransparencyPublishErrorV1,
        > {
            Err(sorafs_node::FencedTransparencyPublishErrorV1::Rejected)
        }
    }

    impl sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1
        for PrebuiltFencedTransparencyProvider
    {
        fn handle(&self) -> &str {
            PREBUILT_FENCED_PRIVACY_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    PREBUILT_FENCED_PRIVACY_POLICY_DIGEST,
                ),
            )
        }

        fn read_authoritative_head_with_ancestry(
            &self,
            required_ancestors: &[sorafs_node::FencedTransparencyTargetHeadV1],
            required_publications: &[sorafs_node::FencedTransparencyPublicationInclusionV1],
        ) -> Result<sorafs_node::FencedTransparencyHeadAncestryProofV1, String> {
            if !required_ancestors.is_empty() || !required_publications.is_empty() {
                return Err(
                    "fresh fused privacy target cannot prove retained ancestry or publication inclusion"
                        .to_owned(),
                );
            }
            sorafs_node::FencedTransparencyHeadAncestryProofV1::try_new(
                None,
                Vec::new(),
                Vec::new(),
                [0xF8; 32],
            )
            .map_err(|_| "fresh fused privacy target returned a malformed genesis proof".to_owned())
        }
    }

    #[derive(Debug)]
    struct SubstitutedFencedTransparencyHeadReader;

    impl sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1
        for SubstitutedFencedTransparencyHeadReader
    {
        fn handle(&self) -> &str {
            PREBUILT_FENCED_PRIVACY_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    2,
                    PREBUILT_FENCED_PRIVACY_POLICY_DIGEST,
                ),
            )
        }

        fn read_authoritative_head_with_ancestry(
            &self,
            required_ancestors: &[sorafs_node::FencedTransparencyTargetHeadV1],
            required_publications: &[sorafs_node::FencedTransparencyPublicationInclusionV1],
        ) -> Result<sorafs_node::FencedTransparencyHeadAncestryProofV1, String> {
            if !required_ancestors.is_empty() || !required_publications.is_empty() {
                return Err(
                    "fresh substituted privacy reader cannot prove retained ancestry or publication inclusion"
                        .to_owned(),
                );
            }
            sorafs_node::FencedTransparencyHeadAncestryProofV1::try_new(
                None,
                Vec::new(),
                Vec::new(),
                [0xF9; 32],
            )
            .map_err(|_| {
                "fresh substituted privacy reader returned a malformed genesis proof".to_owned()
            })
        }
    }

    fn prebuilt_privacy_runtime_deps_without_fenced_target() -> sorafs_node::NodeRuntimeDeps {
        sorafs_node::NodeRuntimeDeps::default()
            .with_privacy_cycle_prf_provider(Arc::new(PrebuiltPrivacyPrfProvider))
            .with_privacy_release_anchor(Arc::new(PrebuiltPrivacyReleaseAnchor))
            .with_transparency_leader_lease_provider(Arc::new(
                PrebuiltTransparencyLeaderLeaseProvider,
            ))
            .with_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
            .with_governance_dag_checkpoint_store(prebuilt_governance_dag_checkpoint_store())
    }

    fn prebuilt_governance_dag_runtime_signer() -> Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>
    {
        Arc::new(PrebuiltGovernanceDagSigner::new())
    }

    fn prebuilt_governance_dag_checkpoint_store()
    -> Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore> {
        Arc::new(PrebuiltGovernanceDagCheckpointStore::exact())
    }

    fn prebuilt_fenced_transparency_runtime() -> (
        Arc<dyn sorafs_node::FencedTransparencyPublisherV1>,
        Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1>,
    ) {
        let provider = Arc::new(PrebuiltFencedTransparencyProvider);
        let publisher: Arc<dyn sorafs_node::FencedTransparencyPublisherV1> = provider.clone();
        let head_reader: Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1> =
            provider;
        (publisher, head_reader)
    }

    fn prebuilt_privacy_runtime_deps() -> sorafs_node::NodeRuntimeDeps {
        let (publisher, head_reader) = prebuilt_fenced_transparency_runtime();
        prebuilt_privacy_runtime_deps_without_fenced_target()
            .with_fenced_transparency_publisher(publisher)
            .with_fenced_transparency_head_reader(head_reader)
    }

    fn prebuilt_privacy_storage_config(
        data_dir: PathBuf,
        prf_revision: u64,
        fenced_publisher_revision: u64,
    ) -> sorafs_node::config::StorageConfig {
        let governance_dir = data_dir.join("governance");
        prebuilt_privacy_storage_config_with_governance_dir(
            data_dir,
            governance_dir,
            prf_revision,
            fenced_publisher_revision,
        )
    }

    fn prebuilt_privacy_storage_config_with_governance_dir(
        data_dir: PathBuf,
        governance_dir: PathBuf,
        prf_revision: u64,
        fenced_publisher_revision: u64,
    ) -> sorafs_node::config::StorageConfig {
        let mut storage = actual::SorafsStorage::default();
        storage.enabled = true;
        storage.provider_id = Some(iroha_data_model::sorafs::capacity::ProviderId::new(
            [0x91; 32],
        ));
        storage.data_dir = data_dir.clone();
        let governance_signer = PrebuiltGovernanceDagSigner::new();
        storage.governance_dag_dir = Some(governance_dir);
        storage.governance_dag_publisher_peer_id = Some(
            String::from_utf8(PREBUILT_GOVERNANCE_SIGNER_PEER_ID.to_vec())
                .expect("Governance DAG peer id is UTF-8"),
        );
        storage.governance_dag_signer_handle = Some(PREBUILT_GOVERNANCE_SIGNER_HANDLE.to_owned());
        storage.governance_dag_signer_revision = Some(1);
        storage.governance_dag_signer_policy_digest =
            Some(PREBUILT_GOVERNANCE_SIGNER_POLICY_DIGEST);
        storage.governance_dag_publisher_public_key_hex =
            Some(hex::encode(governance_signer.public_key_bytes()));
        storage.governance_dag_service.checkpoint_store_handle =
            Some(PREBUILT_GOVERNANCE_CHECKPOINT_STORE_HANDLE.to_owned());
        storage.governance_dag_service.checkpoint_store_revision = Some(1);
        storage
            .governance_dag_service
            .checkpoint_store_policy_digest =
            Some(PREBUILT_GOVERNANCE_CHECKPOINT_STORE_POLICY_DIGEST);
        storage.privacy_aggregates = actual::SorafsPrivacyAggregateSchedule {
            enabled: true,
            cycle_seconds: 100,
            first_cycle_start_unix: 100,
            publish_delay_seconds: 10,
            query_id: Some([0xB0; 32]),
            population_inventory: vec![actual::SorafsPrivacyAggregatePopulation {
                label: "jurisdiction-a".to_owned(),
                digest: [0xA0; 32],
            }],
            metric_schema: vec![actual::SorafsPrivacyAggregateMetric {
                key: "moderation_actions".to_owned(),
                unit: "count".to_owned(),
            }],
            policy_digest: Some([0xC0; 32]),
            cycle_prf_provider: Some(actual::SorafsTransparencyRuntimeProviderBinding {
                handle: PREBUILT_PRIVACY_PRF_HANDLE.to_owned(),
                revision: prf_revision,
                policy_digest: [0xC7; 32],
            }),
            release_anchor_provider: Some(actual::SorafsTransparencyRuntimeProviderBinding {
                handle: PREBUILT_PRIVACY_ANCHOR_HANDLE.to_owned(),
                revision: 1,
                policy_digest: [0xD7; 32],
            }),
            leader_lease_provider: Some(actual::SorafsTransparencyRuntimeProviderBinding {
                handle: PREBUILT_TRANSPARENCY_LEADER_LEASE_HANDLE.to_owned(),
                revision: 1,
                policy_digest: [0xE7; 32],
            }),
            fenced_privacy_publisher: Some(actual::SorafsTransparencyRuntimeProviderBinding {
                handle: PREBUILT_FENCED_PRIVACY_HANDLE.to_owned(),
                revision: fenced_publisher_revision,
                policy_digest: PREBUILT_FENCED_PRIVACY_POLICY_DIGEST,
            }),
            ..actual::SorafsPrivacyAggregateSchedule::default()
        };
        sorafs_node::config::StorageConfig::from(&storage)
    }

    fn prebuilt_governance_storage_config(data_dir: PathBuf) -> sorafs_node::config::StorageConfig {
        let governance_signer = PrebuiltGovernanceDagSigner::new();
        sorafs_node::config::StorageConfig::builder()
            .enabled(true)
            .data_dir(data_dir.clone())
            .governance_dir(Some(data_dir.join("governance")))
            .governance_dag_publisher_peer_id(Some(
                String::from_utf8(PREBUILT_GOVERNANCE_SIGNER_PEER_ID.to_vec())
                    .expect("Governance DAG peer id is UTF-8"),
            ))
            .governance_dag_signer_handle(Some(PREBUILT_GOVERNANCE_SIGNER_HANDLE.to_owned()))
            .governance_dag_signer_qualification(Some(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    PREBUILT_GOVERNANCE_SIGNER_POLICY_DIGEST,
                ),
            ))
            .governance_dag_checkpoint_store_handle(Some(
                PREBUILT_GOVERNANCE_CHECKPOINT_STORE_HANDLE.to_owned(),
            ))
            .governance_dag_checkpoint_store_qualification(Some(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    PREBUILT_GOVERNANCE_CHECKPOINT_STORE_POLICY_DIGEST,
                ),
            ))
            .governance_dag_publisher_public_key_hex(Some(hex::encode(
                governance_signer.public_key_bytes(),
            )))
            .build()
    }

    #[test]
    fn prebuilt_sorafs_node_accepts_exact_privacy_provider_bindings() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt privacy temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config.clone(),
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        preflight_sorafs_fenced_privacy_runtime(
            &config,
            &ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_node(node.clone()),
        )
        .expect("live-revalidate the prebuilt SoraFS fused privacy runtime");
        assert_prebuilt_sorafs_privacy_provider_bindings(
            &node, &config, false, false, false, false, false,
        );
    }

    #[test]
    fn fused_privacy_preflight_rejects_substituted_signed_governance_root() {
        let temp_dir = tempfile::tempdir().expect("create signed-root preflight temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical signed-root preflight temp dir");
        let data_dir = root.join("storage");
        let retained_config = prebuilt_privacy_storage_config(data_dir.clone(), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            retained_config,
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact signed root");
        let substituted_config = prebuilt_privacy_storage_config_with_governance_dir(
            data_dir,
            root.join("substituted-governance"),
            1,
            1,
        );

        let error = preflight_sorafs_fenced_privacy_runtime(
            &substituted_config,
            &ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled()).with_sorafs_node(node),
        )
        .expect_err("prebuilt signed Governance root substitution must fail preflight");

        assert!(
            error.contains("signed Governance root and signer binding does not match"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn fused_privacy_preflight_live_qualifies_exact_raw_pair() {
        let temp_dir = tempfile::tempdir().expect("create raw privacy preflight temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical raw privacy preflight temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let (publisher, head_reader) = prebuilt_fenced_transparency_runtime();
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_fenced_transparency_publisher(publisher)
                .with_sorafs_fenced_transparency_head_reader(head_reader)
                .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );
        preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect("live-qualify the exact raw fused privacy pair");
    }

    #[test]
    fn fused_privacy_preflight_requires_raw_governance_signer() {
        let temp_dir = tempfile::tempdir().expect("create signer preflight temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical signer preflight temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let (publisher, head_reader) = prebuilt_fenced_transparency_runtime();
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_fenced_transparency_publisher(publisher)
                .with_sorafs_fenced_transparency_head_reader(head_reader)
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );

        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("standalone signed Governance publication must require its raw signer");

        assert!(
            error.contains("requires a raw runtime HSM signer"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn fused_privacy_preflight_rejects_substituted_raw_governance_signer() {
        let temp_dir = tempfile::tempdir().expect("create signer substitution temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical signer substitution temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let (publisher, head_reader) = prebuilt_fenced_transparency_runtime();
        let substituted_signer: Arc<dyn sorafs_node::GovernanceDagRuntimeSigner> =
            Arc::new(PrebuiltGovernanceDagSigner::from_seed(0x98));
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_fenced_transparency_publisher(publisher)
                .with_sorafs_fenced_transparency_head_reader(head_reader)
                .with_sorafs_governance_dag_signer(substituted_signer)
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );

        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("substituted raw Governance signer must fail preflight");

        assert!(
            error.contains("does not match the exact configured binding"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn governance_checkpoint_preflight_rejects_missing_raw_store() {
        let temp_dir = tempfile::tempdir().expect("create checkpoint preflight temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical checkpoint preflight temp dir");
        let config = prebuilt_governance_storage_config(root.join("storage"));
        let runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
            .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer());

        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("configured producer must require its raw sealed checkpoint store");

        assert!(
            error.contains("requires a raw sealed checkpoint store"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn governance_checkpoint_preflight_rejects_substituted_raw_store() {
        let temp_dir = tempfile::tempdir().expect("create checkpoint substitution temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical checkpoint substitution temp dir");
        let config = prebuilt_governance_storage_config(root.join("storage"));
        let substituted_store: Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore> =
            Arc::new(PrebuiltGovernanceDagCheckpointStore::with_binding(
                PREBUILT_GOVERNANCE_CHECKPOINT_STORE_HANDLE,
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    2,
                    PREBUILT_GOVERNANCE_CHECKPOINT_STORE_POLICY_DIGEST,
                ),
            ));
        let runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
            .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
            .with_sorafs_governance_dag_checkpoint_store(substituted_store);

        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("substituted raw checkpoint store must fail preflight");

        assert!(
            error.contains(
                "checkpoint-store qualification does not match the exact configured binding"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn governance_checkpoint_preflight_rejects_ambiguous_prebuilt_and_raw_store() {
        let temp_dir = tempfile::tempdir().expect("create checkpoint ambiguity temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical checkpoint ambiguity temp dir");
        let config = prebuilt_governance_storage_config(root.join("storage"));
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config.clone(),
            sorafs_node::NodeRuntimeDeps::default()
                .with_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
                .with_governance_dag_checkpoint_store(prebuilt_governance_dag_checkpoint_store()),
        )
        .expect("start prebuilt SoraFS node with exact checkpoint binding");
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_node(node)
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );

        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("prebuilt node and raw checkpoint store must be mutually exclusive");

        assert!(
            error.contains("must not also receive a raw Governance DAG checkpoint store"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn standalone_node_retains_and_live_revalidates_raw_governance_checkpoint_store() {
        let temp_dir = tempfile::tempdir().expect("create checkpoint retention temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical checkpoint retention temp dir");
        let config = prebuilt_governance_storage_config(root.join("storage"));
        let checkpoint_store = Arc::new(PrebuiltGovernanceDagCheckpointStore::exact());
        let runtime_checkpoint_store: Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore> =
            checkpoint_store.clone();
        let runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
            .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
            .with_sorafs_governance_dag_checkpoint_store(runtime_checkpoint_store);
        preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect("exact raw checkpoint store passes early preflight");
        let node_runtime_deps = sorafs_node::NodeRuntimeDeps::default()
            .with_governance_dag_signer(Arc::clone(
                runtime_deps
                    .sorafs_governance_dag_signer
                    .as_ref()
                    .expect("raw Governance signer retained"),
            ))
            .with_governance_dag_checkpoint_store(Arc::clone(
                runtime_deps
                    .sorafs_governance_dag_checkpoint_store
                    .as_ref()
                    .expect("raw checkpoint store retained"),
            ));

        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(config, node_runtime_deps)
            .expect("standalone node retains exact checkpoint provider");

        assert_eq!(
            node.governance_dag_checkpoint_store_binding(),
            Some((
                PREBUILT_GOVERNANCE_CHECKPOINT_STORE_HANDLE,
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    1,
                    PREBUILT_GOVERNANCE_CHECKPOINT_STORE_POLICY_DIGEST,
                ),
            ))
        );
        node.revalidate_fenced_privacy_runtime()
            .expect("retained checkpoint store live-revalidates");
        checkpoint_store.refuse_qualification();
        let error = node
            .revalidate_fenced_privacy_runtime()
            .expect_err("built node must keep consulting the retained checkpoint provider");
        assert!(
            error.to_string().contains("checkpoint store"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn fused_privacy_preflight_rejects_missing_raw_pair() {
        let temp_dir = tempfile::tempdir().expect("create raw privacy preflight temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical raw privacy preflight temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );
        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("configured fused privacy target must require both raw roles");
        assert!(
            error.contains("requires both a raw writer and authenticated-head reader"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn fused_privacy_preflight_rejects_substituted_raw_writer() {
        let temp_dir = tempfile::tempdir().expect("create raw privacy preflight temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical raw privacy preflight temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 2);
        let (publisher, head_reader) = prebuilt_fenced_transparency_runtime();
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_fenced_transparency_publisher(publisher)
                .with_sorafs_fenced_transparency_head_reader(head_reader)
                .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );
        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("substituted raw writer must fail preflight");
        assert!(
            error.contains("raw fused privacy writer failed live qualification"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn fused_privacy_preflight_rejects_substituted_raw_head_reader() {
        let temp_dir = tempfile::tempdir().expect("create raw privacy preflight temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical raw privacy preflight temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let (publisher, _) = prebuilt_fenced_transparency_runtime();
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_fenced_transparency_publisher(publisher)
                .with_sorafs_fenced_transparency_head_reader(Arc::new(
                    SubstitutedFencedTransparencyHeadReader,
                ))
                .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );
        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("substituted raw head reader must fail preflight");
        assert!(
            error.contains("raw fused privacy authenticated-head reader failed live qualification"),
            "unexpected error: {error}"
        );
    }

    #[test]
    #[should_panic(
        expected = "injected SoraFS node threshold-PRF provider binding does not match torii.sorafs.storage"
    )]
    fn prebuilt_sorafs_node_rejects_mismatched_privacy_provider_binding() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt privacy temp dir");
        let retained_config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            retained_config,
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        let substituted_config = prebuilt_privacy_storage_config(root.join("storage"), 2, 1);
        assert_prebuilt_sorafs_privacy_provider_bindings(
            &node,
            &substituted_config,
            false,
            false,
            false,
            false,
            false,
        );
    }

    #[test]
    #[should_panic(
        expected = "injected SoraFS node fused privacy publisher binding does not match torii.sorafs.storage"
    )]
    fn prebuilt_sorafs_node_rejects_substituted_fenced_privacy_binding() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt privacy temp dir");
        let retained_config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            retained_config,
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        let substituted_config = prebuilt_privacy_storage_config(root.join("storage"), 1, 2);
        assert_prebuilt_sorafs_privacy_provider_bindings(
            &node,
            &substituted_config,
            false,
            false,
            false,
            false,
            false,
        );
    }

    #[test]
    #[should_panic(
        expected = "a prebuilt SoraFS node must not also receive a raw threshold-PRF provider through Torii"
    )]
    fn prebuilt_sorafs_node_rejects_ambiguous_raw_privacy_provider() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt privacy temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config.clone(),
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        assert_prebuilt_sorafs_privacy_provider_bindings(
            &node, &config, true, false, false, false, false,
        );
    }

    #[test]
    #[should_panic(
        expected = "a prebuilt SoraFS node must not also receive a raw fused privacy publisher through Torii"
    )]
    fn prebuilt_sorafs_node_rejects_ambiguous_raw_fenced_privacy_publisher() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt privacy temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config.clone(),
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        assert_prebuilt_sorafs_privacy_provider_bindings(
            &node, &config, false, false, false, true, false,
        );
    }

    #[test]
    #[should_panic(
        expected = "a prebuilt SoraFS node must not also receive a raw authenticated privacy-head reader through Torii"
    )]
    fn prebuilt_sorafs_node_rejects_ambiguous_raw_fenced_privacy_head_reader() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt privacy temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config.clone(),
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        assert_prebuilt_sorafs_privacy_provider_bindings(
            &node, &config, false, false, false, false, true,
        );
    }

    #[test]
    fn fused_privacy_preflight_rejects_prebuilt_and_raw_ambiguity() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt privacy temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config.clone(),
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        let (publisher, _) = prebuilt_fenced_transparency_runtime();
        let runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
            .with_sorafs_node(node)
            .with_sorafs_fenced_transparency_publisher(publisher);
        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("prebuilt and raw fused runtimes must be mutually exclusive");
        assert!(
            error.contains("prebuilt SoraFS node is mutually exclusive"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn fused_privacy_preflight_rejects_prebuilt_and_raw_governance_signer() {
        let temp_dir = tempfile::tempdir().expect("create prebuilt signer temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical prebuilt signer temp dir");
        let config = prebuilt_privacy_storage_config(root.join("storage"), 1, 1);
        let node = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config.clone(),
            prebuilt_privacy_runtime_deps(),
        )
        .expect("start prebuilt SoraFS node with exact privacy bindings");
        let runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
            .with_sorafs_node(node)
            .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer());

        let error = preflight_sorafs_fenced_privacy_runtime(&config, &runtime_deps)
            .expect_err("prebuilt node and raw Governance signer must be mutually exclusive");

        assert!(
            error.contains("prebuilt SoraFS node is mutually exclusive"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn standalone_sorafs_node_rejects_incomplete_fenced_privacy_pairs() {
        for (label, inject_publisher, inject_reader, expected) in [
            (
                "missing-writer",
                false,
                true,
                "requires an injected fused target writer",
            ),
            (
                "missing-reader",
                true,
                false,
                "requires an injected authenticated authoritative-head reader",
            ),
        ] {
            let temp_dir = tempfile::tempdir().expect("create standalone privacy temp dir");
            let root = temp_dir
                .path()
                .canonicalize()
                .expect("canonical standalone privacy temp dir");
            let config =
                prebuilt_privacy_storage_config(root.join(format!("storage-{label}")), 1, 1);
            let (publisher, reader) = prebuilt_fenced_transparency_runtime();
            let mut torii_runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );
            if inject_publisher {
                torii_runtime_deps = torii_runtime_deps
                    .with_sorafs_fenced_transparency_publisher(Arc::clone(&publisher));
            }
            if inject_reader {
                torii_runtime_deps = torii_runtime_deps
                    .with_sorafs_fenced_transparency_head_reader(Arc::clone(&reader));
            }
            let preflight_error =
                preflight_sorafs_fenced_privacy_runtime(&config, &torii_runtime_deps)
                    .expect_err(label);
            assert!(
                preflight_error.contains("one complete pair"),
                "{label} produced unexpected preflight error: {preflight_error}"
            );
            let mut runtime_deps = prebuilt_privacy_runtime_deps_without_fenced_target();
            if inject_publisher {
                runtime_deps = runtime_deps.with_fenced_transparency_publisher(publisher);
            }
            if inject_reader {
                runtime_deps = runtime_deps.with_fenced_transparency_head_reader(reader);
            }
            let error = sorafs_node::NodeHandle::try_new_with_runtime_deps(config, runtime_deps)
                .expect_err(label);
            assert!(
                error.to_string().contains(expected),
                "{label} produced unexpected error: {error}"
            );
        }
    }

    #[test]
    fn standalone_sorafs_node_rejects_unexpected_fenced_privacy_pair() {
        let temp_dir = tempfile::tempdir().expect("create standalone privacy temp dir");
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("canonical standalone privacy temp dir");
        let config = sorafs_node::config::StorageConfig::builder()
            .data_dir(root.join("storage"))
            .build();
        let (publisher, reader) = prebuilt_fenced_transparency_runtime();
        let torii_runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
            .with_sorafs_fenced_transparency_publisher(Arc::clone(&publisher))
            .with_sorafs_fenced_transparency_head_reader(Arc::clone(&reader));
        let preflight_error = preflight_sorafs_fenced_privacy_runtime(&config, &torii_runtime_deps)
            .expect_err("disabled privacy publication must fail Torii preflight");
        assert!(
            preflight_error.contains("unexpected without a configured target binding"),
            "unexpected preflight error: {preflight_error}"
        );
        let error = sorafs_node::NodeHandle::try_new_with_runtime_deps(
            config,
            sorafs_node::NodeRuntimeDeps::default()
                .with_fenced_transparency_publisher(publisher)
                .with_fenced_transparency_head_reader(reader),
        )
        .expect_err("disabled privacy publication must reject the fused pair");
        assert!(
            error
                .to_string()
                .contains("fused privacy target writer is unexpected"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn torii_runtime_deps_retain_fenced_privacy_pair() {
        let (publisher, reader) = prebuilt_fenced_transparency_runtime();
        let runtime_deps =
            ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
                .with_sorafs_fenced_transparency_publisher(publisher)
                .with_sorafs_fenced_transparency_head_reader(reader)
                .with_sorafs_governance_dag_signer(prebuilt_governance_dag_runtime_signer())
                .with_sorafs_governance_dag_checkpoint_store(
                    prebuilt_governance_dag_checkpoint_store(),
                );
        assert!(runtime_deps.sorafs_fenced_transparency_publisher.is_some());
        assert!(
            runtime_deps
                .sorafs_fenced_transparency_head_reader
                .is_some()
        );
        assert!(runtime_deps.sorafs_governance_dag_signer.is_some());
        assert!(
            runtime_deps
                .sorafs_governance_dag_checkpoint_store
                .is_some()
        );
    }

    #[tokio::test]
    #[should_panic(
        expected = "invalid SoraFS node runtime preflight: standalone fused privacy runtime requires the raw writer and authenticated-head reader as one complete pair"
    )]
    async fn new_with_handle_preflights_fused_privacy_before_startup() {
        tokio::task::yield_now().await;
        let cfg = crate::test_utils::mk_minimal_root_cfg();
        let (kiso, _child) = KisoHandle::start(cfg.clone());
        let kura = Kura::blank_kura_for_testing();
        let state = Arc::new(IrohaState::new_for_testing(
            World::default(),
            kura.clone(),
            LiveQueryStore::start_test(),
        ));
        let queue_cfg = iroha_config::parameters::actual::Queue {
            capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
            capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
            transaction_time_to_live: Duration::from_secs(60),
            ..Default::default()
        };
        let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
        let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
        let (publisher, _) = prebuilt_fenced_transparency_runtime();
        let runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
            .with_sorafs_fenced_transparency_publisher(publisher);

        let _ = Torii::new_with_handle(
            ChainId::from("fused-privacy-preflight-test"),
            kiso,
            cfg.torii.clone(),
            queue,
            tokio::sync::broadcast::channel(1).0,
            LiveQueryStore::start_test(),
            kura,
            state,
            cfg.common.key_pair.clone(),
            OnlinePeersProvider::new(peers_rx),
            None,
            runtime_deps,
        );
    }

    fn proof_json_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers
    }

    fn query_conversion_message(error: &Error) -> Option<&str> {
        match error {
            Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => Some(message),
            _ => None,
        }
    }

    struct FailingZkJobIdRng;

    #[derive(Debug)]
    struct FailingZkJobIdRngError;

    impl std::fmt::Display for FailingZkJobIdRngError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("failing zk job-id RNG")
        }
    }

    impl rand::rand_core::TryRngCore for FailingZkJobIdRng {
        type Error = FailingZkJobIdRngError;

        fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
            Err(FailingZkJobIdRngError)
        }

        fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
            Err(FailingZkJobIdRngError)
        }

        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> std::result::Result<(), Self::Error> {
            Err(FailingZkJobIdRngError)
        }
    }

    impl rand::rand_core::TryCryptoRng for FailingZkJobIdRng {}

    #[test]
    fn zk_ivm_prove_job_id_reports_rng_failure() {
        let mut rng = FailingZkJobIdRng;

        let error =
            zk_ivm_prove_job_id_with_rng(&mut rng).expect_err("RNG failure must be reported");

        match error {
            Error::Query(ValidationFail::InternalError(message)) => {
                assert!(message.contains("zk IVM prove job-id OS RNG failed"));
                assert!(message.contains("failing zk job-id RNG"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn zk_ivm_job_routes_reject_noncanonical_ids_before_lookup_or_echo() {
        let app = mk_app_state_for_tests();
        let invalid = [
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "0123456789ABCDEF0123456789ABCDEF",
            "g123456789abcdef0123456789abcdef",
        ];
        for job_id in invalid {
            let get_error = match handler_zk_ivm_prove_get(
                State(app.clone()),
                HeaderMap::new(),
                crate::loopback_connect_info(),
                axum::extract::Path(job_id.to_owned()),
            )
            .await
            {
                Ok(_) => panic!("GET accepted invalid job id"),
                Err(error) => error,
            };
            let message = query_conversion_message(&get_error).expect("GET conversion error");
            assert!(message.contains("exactly 32 lowercase hexadecimal"));
            assert!(
                !message.contains(job_id),
                "invalid id must not be reflected"
            );

            let delete_error = match handler_zk_ivm_prove_delete(
                State(app.clone()),
                HeaderMap::new(),
                crate::loopback_connect_info(),
                axum::extract::Path(job_id.to_owned()),
            )
            .await
            {
                Ok(_) => panic!("DELETE accepted and echoed invalid job id"),
                Err(error) => error,
            };
            let message = query_conversion_message(&delete_error).expect("DELETE conversion error");
            assert!(message.contains("exactly 32 lowercase hexadecimal"));
            assert!(
                !message.contains(job_id),
                "invalid id must not be reflected"
            );
        }
        validate_zk_ivm_prove_job_id("0123456789abcdef0123456789abcdef").expect("canonical id");
    }

    #[cfg(feature = "push")]
    use crate::tests_runtime_handlers::mk_app_state_for_tests_with_world_and_push;
    #[cfg(feature = "telemetry")]
    use crate::tests_runtime_handlers::mk_norito_rpc_test_harness;
    #[cfg(feature = "app_api")]
    use crate::tests_runtime_handlers::{
        bind_account_alias_for_test, bind_contract_alias_for_test,
        bind_dynamic_account_alias_for_test, configure_multiple_dataspace_routes_for_test,
        configure_private_ingress_routes_for_test, world_with_account_bound_to_dataspace,
        world_with_target_and_caller_bound_to_dataspace,
    };
    use crate::{
        limits,
        tests_runtime_handlers::{
            app_auth_test_guard, checked_torii_test_account_id, checked_torii_test_ed25519_keypair,
            mk_app_state_for_tests, mk_app_state_for_tests_with_iso_bridge,
            mk_app_state_for_tests_with_options, mk_app_state_for_tests_with_world,
            record_latest_committed_header_for_test, signed_app_headers, world_with_account,
        },
    };
    use iroha_core::smartcontracts::Execute;

    #[test]
    fn stark_fri_backend_labels_require_non_empty_profile() {
        assert!(is_stark_fri_v1_backend("stark/fri"));
        assert!(is_stark_fri_v1_backend("stark/fri/sha256-goldilocks"));
        assert!(is_stark_fri_v1_backend("stark/fri/poseidon2-goldilocks"));
        assert!(is_stark_fri_v1_backend("stark/fri/sha256_goldilocks.v1"));
        assert!(!is_stark_fri_v1_backend("stark/fri/"));
        assert!(!is_stark_fri_v1_backend("stark/fri/latest"));
        assert!(!is_stark_fri_v1_backend("stark/fri/random-profile"));
        assert!(!is_stark_fri_v1_backend("stark/fri/sha512-goldilocks"));
        assert!(!is_stark_fri_v1_backend("stark/fri/kzg"));
        assert!(!is_stark_fri_v1_backend("stark/fri/bn254"));
        assert!(!is_stark_fri_v1_backend("stark/fri/debug"));
        assert!(!is_stark_fri_v1_backend("stark/fri/debug-proof"));
        assert!(!is_stark_fri_v1_backend("stark/fri/mock"));
        assert!(!is_stark_fri_v1_backend("stark/fri/mock-proof"));
        assert!(!is_stark_fri_v1_backend("stark/fri-v2"));
    }

    #[test]
    fn parse_pipeline_status_scope_defaults_to_global_and_trims_case() {
        assert_eq!(
            parse_pipeline_status_scope(None).expect("default scope"),
            PipelineStatusReadScope::Global
        );
        assert_eq!(
            parse_pipeline_status_scope(Some("")).expect("blank scope"),
            PipelineStatusReadScope::Global
        );
        assert_eq!(
            parse_pipeline_status_scope(Some(" GLOBAL ")).expect("case-insensitive global"),
            PipelineStatusReadScope::Global
        );
        assert_eq!(
            parse_pipeline_status_scope(Some(" local ")).expect("case-insensitive local"),
            PipelineStatusReadScope::Local
        );
        assert_eq!(
            parse_pipeline_status_scope(Some(" AUTO ")).expect("auto scope compatibility"),
            PipelineStatusReadScope::Global
        );
    }

    #[test]
    fn parse_pipeline_status_scope_rejects_injected_values() {
        for raw in [
            "auto&scope=local",
            "global&scope=local",
            "local,global",
            "../global",
            "global\nscope=local",
        ] {
            let err = parse_pipeline_status_scope(Some(raw)).expect_err("invalid scope");
            assert!(
                format!("{err:?}").contains("expected local|global|auto"),
                "unexpected error for {raw:?}: {err:?}"
            );
        }
    }

    fn bind_asset_alias_for_test(
        app: &SharedAppState,
        authority: &AccountId,
        definition_id: &AssetDefinitionId,
        alias: &AssetDefinitionAlias,
        lease_expiry_ms: Option<u64>,
        height: u64,
        creation_time_ms: u64,
    ) {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            creation_time_ms,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        iroha_data_model::isi::SetAssetDefinitionAlias::bind(
            definition_id.clone(),
            alias.clone(),
            lease_expiry_ms,
        )
        .execute(authority, &mut tx)
        .expect("bind asset alias for test");
        tx.apply();
        block.commit().expect("commit asset alias for test");
    }

    fn sample_iso_bridge_config(alias: &str, account_id: &AccountId) -> actual::IsoBridge {
        let signer_keypair =
            checked_torii_test_ed25519_keypair(0x80, "derive ISO bridge signer fixture key");
        actual::IsoBridge {
            enabled: true,
            max_body_bytes:
                iroha_config::parameters::defaults::torii::ISO_BRIDGE_MAX_BODY_BYTES,
            dedupe_ttl_secs: 30,
            default_profile: "generic-iso20022".to_owned(),
            profiles: Vec::new(),
            store_dir: None,
            store_retention_secs:
                iroha_config::parameters::defaults::torii::ISO_BRIDGE_STORE_RETENTION_SECS,
            store_max_records:
                iroha_config::parameters::defaults::torii::ISO_BRIDGE_STORE_MAX_RECORDS,
            audit_export_dir: None,
            embedded_signature_policy: None,
            signer: Some(actual::IsoBridgeSigner {
                account_id: account_id.to_string(),
                private_key: signer_keypair.private_key().clone(),
            }),
            account_aliases: vec![actual::IsoAccountAlias {
                iban: alias.to_string(),
                account_id: account_id.to_string(),
            }],
            currency_assets: Vec::new(),
            reference_data: actual::IsoReferenceData::default(),
        }
    }

    fn local_connect_info() -> axum::extract::ConnectInfo<std::net::SocketAddr> {
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
    }

    #[tokio::test]
    async fn iso_audit_messages_endpoint_exports_digest_bound_manifest() {
        let app = mk_app_state_for_tests_with_iso_bridge(Some(sample_iso_bridge_config(
            "DE89370400440532013000",
            &ALICE_ID,
        )));
        let runtime = app.iso_bridge.as_ref().expect("iso bridge enabled");
        runtime.mark_accepted("handler-audit", "handler-tx");

        let (status, JsonBody(body)) =
            handler_iso_audit_messages(State(app), HeaderMap::new(), local_connect_info())
                .await
                .expect("audit endpoint");
        assert_eq!(status, StatusCode::OK);
        let body = body.as_object().expect("audit manifest object");
        assert_eq!(
            body.get("record_count")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert!(
            body.get("index_sha256")
                .and_then(norito::json::Value::as_str)
                .is_some_and(|digest| digest.len() == 64)
        );
        let records = body
            .get("records")
            .and_then(norito::json::Value::as_array)
            .expect("audit records");
        assert_eq!(
            records[0]
                .as_object()
                .and_then(|entry| entry.get("message_id"))
                .and_then(norito::json::Value::as_str),
            Some("handler-audit")
        );
    }

    #[tokio::test]
    async fn iso_audit_messages_endpoint_rejects_disabled_bridge() {
        let err = handler_iso_audit_messages(
            State(mk_app_state_for_tests()),
            HeaderMap::new(),
            local_connect_info(),
        )
        .await
        .expect_err("disabled bridge should reject audit export");
        assert!(
            matches!(
                &err,
                Error::Query(iroha_data_model::ValidationFail::NotPermitted(message))
                    if message.contains("iso20022 bridge disabled")
            ),
            "unexpected error: {err:?}"
        );
    }

    pub(crate) fn test_inrou_manifest() -> iroha_data_model::soracloud::SoraInrouManifestV1 {
        iroha_data_model::soracloud::SoraInrouManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_INROU_MANIFEST_VERSION_V1,
            guest_os: iroha_data_model::soracloud::SoraInrouGuestOsV1::DebianSlim,
            guest_images: std::collections::BTreeMap::from([
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
                (
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::Aarch64,
                    iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                        initrd_image_path: None,
                        distribution: Default::default(),
                        published_artifact: None,
                    },
                ),
            ]),
            bootstrap_user_data_path: None,
            ssh_authorized_keys: vec!["ssh-ed25519 test-key torii-tests".to_owned()],
        }
    }

    fn sample_identifier_policy(
        owner: &AccountId,
        signer: &KeyPair,
        policy_id: &IdentifierPolicyId,
    ) -> (IdentifierPolicy, RamLfeProgramPolicy) {
        sample_identifier_policy_with_backend(
            owner,
            signer,
            policy_id,
            RamLfeBackend::BfvAffineSha3_256V1,
        )
    }

    fn sample_programmed_identifier_policy(
        owner: &AccountId,
        signer: &KeyPair,
        policy_id: &IdentifierPolicyId,
    ) -> (IdentifierPolicy, RamLfeProgramPolicy) {
        sample_identifier_policy_with_backend(
            owner,
            signer,
            policy_id,
            RamLfeBackend::BfvProgrammedSha3_256V1,
        )
    }

    fn sample_identifier_policy_with_backend(
        owner: &AccountId,
        signer: &KeyPair,
        policy_id: &IdentifierPolicyId,
        backend: RamLfeBackend,
    ) -> (IdentifierPolicy, RamLfeProgramPolicy) {
        let program_id = sample_program_id(policy_id);
        let (public_parameters, _, relinearization_key) = derive_identifier_key_material_from_seed(
            &sample_identifier_bfv_parameters(backend),
            63,
            b"resolver-secret",
            &identifier_resolution::program_id_bytes(&program_id),
        )
        .expect("identifier BFV parameters");
        let program_policy = match backend {
            RamLfeBackend::BfvAffineSha3_256V1 => RamLfeProgramPolicy::new(
                program_id.clone(),
                owner.clone(),
                backend,
                RamLfeVerificationMode::Signed,
                bfv_affine_policy_commitment(
                    b"resolver-secret",
                    norito::to_bytes(&public_parameters).expect("encode BFV parameters"),
                )
                .expect("policy commitment"),
                signer.public_key().clone(),
            ),
            RamLfeBackend::BfvProgrammedSha3_256V1 => {
                let hidden_program = default_bfv_programmed_hidden_program();
                let evaluation_keys = BfvEvaluationKeyBundle {
                    relinearization_key,
                    rotation_keys: Vec::new(),
                    galois_keys: Vec::new(),
                    bootstrap_key: None,
                };
                let programmed_public_parameters =
                    try_bfv_programmed_public_parameters_with_program(
                        public_parameters,
                        evaluation_keys,
                        &hidden_program,
                        RamLfeVerificationMode::Signed,
                        None,
                    )
                    .expect("build programmed BFV public parameters");
                let encoded_public_parameters = norito::to_bytes(&programmed_public_parameters)
                    .expect("encode programmed BFV parameters");
                RamLfeProgramPolicy::new(
                    program_id.clone(),
                    owner.clone(),
                    backend,
                    RamLfeVerificationMode::Signed,
                    bfv_programmed_policy_commitment_with_program(
                        b"resolver-secret",
                        &encoded_public_parameters,
                        &hidden_program,
                    )
                    .expect("policy commitment"),
                    signer.public_key().clone(),
                )
            }
            RamLfeBackend::HkdfSha3_512PrfV1 => unreachable!("sample BFV policy"),
        };
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::PhoneE164,
            program_id,
        );
        (policy, program_policy)
    }

    fn sample_program_id(policy_id: &IdentifierPolicyId) -> RamLfeProgramId {
        policy_id
            .to_string()
            .replace('#', "_")
            .parse()
            .expect("program id")
    }

    fn sample_identifier_bfv_parameters(_backend: RamLfeBackend) -> BfvParameters {
        ram_lfe_bfv_parameters_v1()
    }

    fn encrypted_identifier_ciphertext(
        program_policy: &RamLfeProgramPolicy,
        input: &[u8],
        seed: &[u8],
    ) -> iroha_crypto::BfvIdentifierCiphertext {
        let public_parameters = identifier_resolution::decode_bfv_public_parameters(program_policy)
            .expect("decode BFV public parameters");
        encrypt_identifier_from_seed(&public_parameters, input, seed).expect("encrypt identifier")
    }

    fn encrypted_identifier_hex(
        program_policy: &RamLfeProgramPolicy,
        input: &[u8],
        seed: &[u8],
    ) -> String {
        hex::encode(
            norito::to_bytes(&encrypted_identifier_ciphertext(
                program_policy,
                input,
                seed,
            ))
            .expect("encode encrypted identifier"),
        )
    }

    fn output_opening_for_ciphertext(
        resolver: &identifier_resolution::IdentifierResolutionService,
        program_policy: &RamLfeProgramPolicy,
        signer: &KeyPair,
        ciphertext: &iroha_crypto::BfvIdentifierCiphertext,
    ) -> RamLfeOutputOpening {
        let execution = resolver
            .execute_encrypted(program_policy, ciphertext)
            .expect("execute encrypted identifier input");
        let payload = RamLfeOutputOpeningPayload {
            program_id: program_policy.program_id.clone(),
            input_ciphertext_hash: execution.input_ciphertext_hash,
            output_ciphertext_hash: execution.output_ciphertext_hash,
            parameter_digest: execution.parameter_digest,
            evaluation_key_digest: execution.evaluation_key_digest,
            opened_output_hash: ram_lfe_output_hash(&execution.output),
            opened_at_ms: execution.executed_at_ms,
            expires_at_ms: execution.expires_at_ms,
        };
        RamLfeOutputOpening {
            signature: SignatureOf::try_new(signer.private_key(), &payload)
                .expect("sign RAM-LFE output opening fixture")
                .into(),
            payload,
        }
    }

    fn dummy_output_opening_for_access_test() -> RamLfeOutputOpening {
        let signer = checked_torii_test_ed25519_keypair(
            0x81,
            "derive RAM-LFE dummy output opening fixture key",
        );
        let payload = RamLfeOutputOpeningPayload {
            program_id: "access_test".parse().expect("program id"),
            input_ciphertext_hash: Hash::new(b"access-test-input"),
            output_ciphertext_hash: Hash::new(b"access-test-output"),
            parameter_digest: Hash::new(b"access-test-parameters"),
            evaluation_key_digest: Hash::new(b"access-test-evaluation-key"),
            opened_output_hash: Hash::new(b"access-test-opened-output"),
            opened_at_ms: 0,
            expires_at_ms: None,
        };
        RamLfeOutputOpening {
            signature: SignatureOf::try_new(signer.private_key(), &payload)
                .expect("sign dummy RAM-LFE output opening fixture")
                .into(),
            payload,
        }
    }

    fn shared_sdk_identifier_bfv_public_parameters(
        policy_id: &IdentifierPolicyId,
    ) -> BfvIdentifierPublicParameters {
        let parameters = sample_identifier_bfv_parameters(RamLfeBackend::BfvProgrammedSha3_256V1);
        let program_id = sample_program_id(policy_id);
        let (derived, _, _) = derive_identifier_key_material_from_seed(
            &parameters,
            63,
            b"resolver-secret",
            &identifier_resolution::program_id_bytes(&program_id),
        )
        .expect("derive shared SDK BFV public parameters");
        derived
    }

    fn sample_identifier_policy_with_public_parameters(
        owner: &AccountId,
        signer: &KeyPair,
        policy_id: &IdentifierPolicyId,
        normalization: IdentifierNormalization,
        public_parameters: &BfvIdentifierPublicParameters,
    ) -> (IdentifierPolicy, RamLfeProgramPolicy) {
        let program_id = sample_program_id(policy_id);
        let hidden_program = default_bfv_programmed_hidden_program();
        let (derived, _, relinearization_key) = derive_identifier_key_material_from_seed(
            &public_parameters.parameters,
            public_parameters.max_input_bytes,
            b"resolver-secret",
            &identifier_resolution::program_id_bytes(&program_id),
        )
        .expect("derive programmed evaluation keys");
        assert_eq!(&derived, public_parameters);
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        let programmed_public_parameters = try_bfv_programmed_public_parameters_with_program(
            public_parameters.clone(),
            evaluation_keys,
            &hidden_program,
            RamLfeVerificationMode::Signed,
            None,
        )
        .expect("build programmed BFV public parameters");
        let encoded_public_parameters =
            norito::to_bytes(&programmed_public_parameters).expect("encode BFV parameters");
        let program_policy = RamLfeProgramPolicy::new(
            program_id.clone(),
            owner.clone(),
            RamLfeBackend::BfvProgrammedSha3_256V1,
            RamLfeVerificationMode::Signed,
            bfv_programmed_policy_commitment_with_program(
                b"resolver-secret",
                &encoded_public_parameters,
                &hidden_program,
            )
            .expect("policy commitment"),
            signer.public_key().clone(),
        );
        let policy =
            IdentifierPolicy::new(policy_id.clone(), owner.clone(), normalization, program_id);
        (policy, program_policy)
    }

    fn register_and_activate_identifier_policy_bundle(
        authority: &AccountId,
        tx: &mut iroha_core::state::StateTransaction<'_, '_>,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
    ) {
        RegisterRamLfeProgramPolicy {
            policy: program_policy.clone(),
        }
        .execute(authority, tx)
        .expect("register program policy");
        ActivateRamLfeProgramPolicy {
            program_id: program_policy.program_id.clone(),
        }
        .execute(authority, tx)
        .expect("activate program policy");
        RegisterIdentifierPolicy {
            policy: policy.clone(),
        }
        .execute(authority, tx)
        .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy.id.clone(),
        }
        .execute(authority, tx)
        .expect("activate policy");
    }

    fn register_and_activate_program_policy(
        authority: &AccountId,
        tx: &mut iroha_core::state::StateTransaction<'_, '_>,
        program_policy: &RamLfeProgramPolicy,
    ) {
        RegisterRamLfeProgramPolicy {
            policy: program_policy.clone(),
        }
        .execute(authority, tx)
        .expect("register program policy");
        ActivateRamLfeProgramPolicy {
            program_id: program_policy.program_id.clone(),
        }
        .execute(authority, tx)
        .expect("activate program policy");
    }

    fn seed_proof_record_at_height(
        app: &SharedAppState,
        backend: &str,
        proof_hash: [u8; 32],
        verified_at_height: u64,
    ) -> String {
        let height = verified_at_height.max(1);
        // Ensure the core state height is aligned with the proof being seeded to avoid
        // commit-height mismatches when tests inject multiple proof blocks.
        set_latest_block_height(app, height.saturating_sub(1));
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        let id = ProofId {
            backend: backend.to_string(),
            proof_hash,
        };
        let rec = ProofRecord {
            id: id.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Verified,
            verified_at_height: Some(verified_at_height),
            bridge: None,
        };
        stx.world.proofs_mut_for_testing().insert(id.clone(), rec);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block
            .commit()
            .expect("seed proof block commit should succeed");
        id.to_string()
    }

    fn seed_proof_record(app: &SharedAppState, backend: &str, proof_hash: [u8; 32]) -> String {
        seed_proof_record_at_height(app, backend, proof_hash, 1)
    }

    fn current_block_height(app: &SharedAppState) -> u64 {
        app.state
            .transactions_latest_height_for_testing()
            .try_into()
            .expect("height should fit into u64")
    }

    fn next_block_height(app: &SharedAppState) -> u64 {
        current_block_height(app).saturating_add(1).max(1)
    }

    #[cfg(feature = "app_api")]
    fn sample_tx_history_jwt_claims(subject: &str) -> Value {
        let mut claims = Map::new();
        claims.insert("sub".to_string(), Value::from(subject));
        claims.insert("dataspace_id".to_string(), Value::from("banka"));
        claims.insert(
            "roles".to_string(),
            Value::Array(vec![Value::from("FI_OPERATOR")]),
        );
        claims.insert("iat".to_string(), Value::from(1_700_000_000_u64));
        claims.insert("nbf".to_string(), Value::from(1_700_000_000_u64));
        claims.insert("exp".to_string(), Value::from(4_102_444_800_u64));
        claims.insert("iss".to_string(), Value::from("pk-cbdc-dev"));
        claims.insert("aud".to_string(), Value::from("pk-cbdc"));
        Value::Object(claims)
    }

    #[cfg(feature = "app_api")]
    fn sign_tx_history_jwt_claims(secret: &str, claims: Value) -> String {
        let mut header = Map::new();
        header.insert("typ".to_string(), Value::from("JWT"));
        header.insert(
            "alg".to_string(),
            Value::from(tx_history_jwt_algorithm_name(JwtAlgorithm::HS256)),
        );
        let encoded_header = JWT_BASE64.encode(
            norito::json::to_vec(&Value::Object(header)).expect("sample JWT header should encode"),
        );
        let encoded_claims = JWT_BASE64
            .encode(norito::json::to_vec(&claims).expect("sample JWT claims should encode"));
        let message = format!("{encoded_header}.{encoded_claims}");
        let signature = jsonwebtoken::crypto::sign(
            message.as_bytes(),
            &EncodingKey::from_secret(secret.as_bytes()),
            JwtAlgorithm::HS256,
        )
        .expect("sample tx-history jwt should sign");
        format!("{message}.{signature}")
    }

    #[cfg(feature = "app_api")]
    fn sample_tx_history_jwt(secret: &str) -> String {
        sign_tx_history_jwt_claims(secret, sample_tx_history_jwt_claims("operator1@banka"))
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn tx_history_jwt_claims_accept_valid_hmac_token() {
        let secret = "shared-secret";
        let token = sample_tx_history_jwt(secret);
        let jwt = TxHistoryJwtConfig {
            algorithm: JwtAlgorithm::HS256,
            key: TxHistoryJwtKey::Hmac(secret.as_bytes().to_vec()),
            issuer: Some("pk-cbdc-dev".to_string()),
            audience: Some("pk-cbdc".to_string()),
        };

        let claims = decode_tx_history_jwt_claims(&format!("Bearer {token}"), &jwt)
            .expect("valid tx-history token should decode");

        assert_eq!(claims.sub.as_deref(), Some("operator1@banka"));
        assert_eq!(claims.dataspace_id.as_deref(), Some("banka"));
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn tx_history_jwt_claims_reject_invalid_hmac_signature() {
        let token = sample_tx_history_jwt("correct-secret");
        let jwt = TxHistoryJwtConfig {
            algorithm: JwtAlgorithm::HS256,
            key: TxHistoryJwtKey::Hmac(b"wrong-secret".to_vec()),
            issuer: Some("pk-cbdc-dev".to_string()),
            audience: Some("pk-cbdc".to_string()),
        };

        let err = decode_tx_history_jwt_claims(&format!("Bearer {token}"), &jwt)
            .expect_err("mismatched secret must fail");
        assert_eq!(err, "invalid JWT");
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn tx_history_alias_resolution_reject_maps_duplicate_bindings_to_conflict() {
        let response = tx_history_alias_resolution_reject(Error::AppConflict {
            code: "account_alias_conflict",
            message:
                "account alias `operator1@banka` is bound to multiple accounts: account-a and account-b"
                    .to_string(),
        });

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn tx_history_alias_resolution_reject_maps_invalid_alias_literals_to_bad_request() {
        let response = tx_history_alias_resolution_reject(Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "alias contains invalid scope".to_string(),
                ),
            ),
        ));

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn normalize_tx_history_alias_preserves_on_chain_literals() {
        assert_eq!(
            normalize_tx_history_alias("operator1@banka"),
            "operator1@banka"
        );
        assert_eq!(
            normalize_tx_history_alias("operator2@bankb"),
            "operator2@bankb"
        );
        assert_eq!(
            normalize_tx_history_alias("banking@universal"),
            "banking@universal"
        );
        assert_eq!(
            normalize_tx_history_alias("operator1@banka.dataspace"),
            "operator1@banka.dataspace"
        );
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn canonical_tx_history_subject_alias_accepts_canonical_literals() {
        let catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(10),
                alias: "banka".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        assert_eq!(
            canonical_tx_history_subject_alias(&catalog, "operator1@banka")
                .expect("canonical dataspace alias should parse"),
            Some("operator1@banka".to_string())
        );
        assert_eq!(
            canonical_tx_history_subject_alias(&catalog, "banking@universal")
                .expect("canonical dataspace alias should parse"),
            Some("banking@universal".to_string())
        );
        assert_eq!(
            canonical_tx_history_subject_alias(&catalog, "operator1@branch.banka")
                .expect("canonical domain alias should parse"),
            Some("operator1@branch.banka".to_string())
        );
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn canonical_tx_history_subject_alias_rejects_bare_subjects() {
        let catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(10),
                alias: "banka".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        assert_eq!(
            canonical_tx_history_subject_alias(&catalog, "operator1")
                .expect("bare subjects should not parse as aliases"),
            None
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn tx_history_viewer_from_headers_rejects_bare_subject_aliases() {
        let mut app = mk_app_state_for_tests();
        let secret = "shared-secret";
        let token = sign_tx_history_jwt_claims(secret, sample_tx_history_jwt_claims("operator1"));
        let app_state = Arc::get_mut(&mut app).expect("unique app state");
        app_state.tx_history_access_policy = Arc::new(TxHistoryAccessPolicy {
            jwt: Some(TxHistoryJwtConfig {
                algorithm: JwtAlgorithm::HS256,
                key: TxHistoryJwtKey::Hmac(secret.as_bytes().to_vec()),
                issuer: Some("pk-cbdc-dev".to_string()),
                audience: Some("pk-cbdc".to_string()),
            }),
            ..TxHistoryAccessPolicy::default()
        });

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}")
                .parse()
                .expect("authorization header"),
        );

        let response = tx_history_viewer_from_headers(&app, &headers)
            .expect_err("bare subject aliases must be rejected");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[cfg(feature = "zk-stark")]
    fn sample_stark_vk_box(
        backend: &str,
        circuit_id: &str,
        hash_fn: u8,
    ) -> iroha_data_model::proof::VerifyingKeyBox {
        let vk_payload = iroha_core::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.to_owned(),
            n_log2: iroha_core::zk_stark::STARK_FRI_CONSENSUS_MIN_N_LOG2,
            blowup_log2: iroha_core::zk_stark::STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: iroha_core::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
            merkle_arity: 2,
            hash_fn,
        };
        let bytes = norito::to_bytes(&vk_payload).expect("encode stark vk payload");
        iroha_data_model::proof::VerifyingKeyBox::new(backend.to_owned(), bytes)
    }

    fn sample_ivm_prove_authority() -> AccountId {
        checked_torii_test_account_id(0x83, "derive ZK IVM prove authority fixture key")
    }

    fn sample_ivm_fee_payment() -> iroha_data_model::transaction::FeePaymentIntent {
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            NonZeroU64::new(50_000_000),
        )
    }

    #[test]
    fn zk_ivm_fee_payment_requires_typed_gas_bound_and_rejects_legacy_metadata() {
        let metadata = iroha_data_model::metadata::Metadata::default();
        let missing_gas =
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None);
        assert!(validate_zk_ivm_fee_payment(&missing_gas, &metadata).is_err());

        let valid = sample_ivm_fee_payment();
        validate_zk_ivm_fee_payment(&valid, &metadata).expect("typed gas bound should validate");

        let mut legacy = metadata;
        legacy.insert(
            Name::from_str("gas_limit").expect("static legacy metadata key"),
            iroha_primitives::json::Json::new(50_000_000_u64),
        );
        assert!(validate_zk_ivm_fee_payment(&valid, &legacy).is_err());
    }

    fn make_ivm_prove_request(
        vk_ref: VerifyingKeyId,
        bytecode: IvmBytecode,
        proved: Option<IvmProved>,
    ) -> ZkIvmProveRequestDto {
        ZkIvmProveRequestDto {
            vk_ref,
            authority: sample_ivm_prove_authority(),
            fee_payment: sample_ivm_fee_payment(),
            metadata: iroha_data_model::metadata::Metadata::default(),
            bytecode,
            proved,
        }
    }

    fn set_latest_block_height(app: &SharedAppState, height: u64) {
        let mut current_height = current_block_height(app);
        while current_height < height {
            let next_height = current_height.saturating_add(1);
            let header = BlockHeader::new(
                NonZeroU64::new(next_height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(next_height as usize).expect("block count should be non-zero"),
            );
            block
                .commit()
                .expect("set latest block height commit should succeed");
            current_height = next_height;
        }
    }

    fn grant_alias_resolve_permissions(
        app: &SharedAppState,
        account_id: &AccountId,
        alias: &AccountAlias,
    ) {
        let height = next_block_height(app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        let scope = match alias
            .domain_id(&app.state.nexus_snapshot().dataspace_catalog)
            .expect("test alias dataspace must resolve")
        {
            Some(domain) => AccountAliasPermissionScope::Domain(domain),
            None => AccountAliasPermissionScope::Dataspace(alias.dataspace),
        };
        stx.world_mut_for_testing().add_account_permission(
            account_id,
            Permission::from(CanResolveAccountAlias { scope }),
        );
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block
            .commit()
            .expect("commit should persist alias resolve permission");
    }

    fn signed_alias_resolve_headers_for_test(
        app: &SharedAppState,
        account_id: &AccountId,
        keypair: &KeyPair,
        alias: &AccountAlias,
        body: &[u8],
    ) -> HeaderMap {
        grant_alias_resolve_permissions(app, account_id, alias);
        let method = Method::POST;
        let uri: Uri = "/v1/aliases/resolve"
            .parse()
            .expect("alias resolve test URI");
        signed_app_headers(account_id, keypair, &method, &uri, body)
    }

    fn grant_alias_resolve_dataspace_permission(
        app: &SharedAppState,
        account_id: &AccountId,
        dataspace: DataSpaceId,
    ) {
        let height = next_block_height(app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world_mut_for_testing().add_account_permission(
            account_id,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(dataspace),
            }),
        );
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block
            .commit()
            .expect("commit should persist alias dataspace resolve permission");
    }

    fn recipient_lookup_sbp_dataspace_for_test() -> DataSpaceId {
        DataSpaceId::new(20)
    }

    fn recipient_lookup_cbuae_dataspace_for_test() -> DataSpaceId {
        DataSpaceId::new(10)
    }

    fn recipient_lookup_aed_definition_for_test() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("fx", "universal").expect("FX domain"),
            "aed".parse().expect("AED name"),
        )
    }

    fn recipient_lookup_world_for_test(caller: &AccountId, target: &AccountId) -> World {
        let definition_id = recipient_lookup_aed_definition_for_test();
        World::with_assets(
            [Domain::new(DomainId::try_new("fx", "universal").expect("FX domain")).build(caller)],
            [
                Account::new(caller.clone()).build(caller),
                Account::new(target.clone()).build(caller),
            ],
            [
                iroha_data_model::asset::AssetDefinition::numeric(definition_id.clone())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(caller),
            ],
            [iroha_data_model::asset::Asset::new(
                AssetId::with_scope(
                    definition_id,
                    caller.clone(),
                    AssetBalanceScope::Dataspace(recipient_lookup_cbuae_dataspace_for_test()),
                ),
                Quantity::from(100_u32),
            )],
            [],
        )
    }

    fn configure_recipient_lookup_sbp_dataspace_for_test(
        app: &mut SharedAppState,
        visibility: iroha_data_model::nexus::LaneVisibility,
    ) {
        let sbp_dataspace = recipient_lookup_sbp_dataspace_for_test();
        let cbuae_dataspace = recipient_lookup_cbuae_dataspace_for_test();
        let cbuae_lane = LaneId::new(1);
        let sbp_lane = LaneId::new(2);
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            std::num::NonZeroU32::new(3).expect("nonzero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                iroha_data_model::nexus::LaneConfig {
                    id: cbuae_lane,
                    dataspace_id: cbuae_dataspace,
                    alias: "cbuae".to_owned(),
                    visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
                iroha_data_model::nexus::LaneConfig {
                    id: sbp_lane,
                    dataspace_id: sbp_dataspace,
                    alias: "sbp".to_owned(),
                    visibility,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: cbuae_dataspace,
                alias: "cbuae".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: sbp_dataspace,
                alias: "sbp".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let nexus = actual::Nexus {
            enabled: true,
            lane_catalog,
            dataspace_catalog,
            ..actual::Nexus::default()
        };

        let app_state = Arc::get_mut(app).expect("unique app state");
        let state = Arc::get_mut(&mut app_state.state).expect("unique state");
        state.set_nexus(nexus.clone()).expect("apply nexus config");
        let state_view = app_state.state.view();
        app_state.queue.reconfigure_nexus(&nexus, &state_view, None);
    }

    fn onboarding_alias_test_app(
        authority: &AccountId,
        domain_owner: &AccountId,
    ) -> SharedAppState {
        let mut accounts = vec![Account::new(authority.clone()).build(authority)];
        if domain_owner != authority {
            accounts.push(Account::new(domain_owner.clone()).build(domain_owner));
        }
        let domains = [
            Domain::new(DomainId::try_new("hbl", "sbp").expect("HBL domain")).build(domain_owner),
            Domain::new(DomainId::try_new("ubl", "sbp").expect("UBL domain")).build(domain_owner),
        ];
        let fee_asset_id: AssetDefinitionId =
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
                .parse()
                .expect("default fee asset id");
        let fee_definition =
            iroha_data_model::asset::AssetDefinition::numeric(fee_asset_id.clone())
                .with_name("xor".to_owned())
                .build(authority);
        let fee_asset = iroha_data_model::asset::Asset::new(
            iroha_data_model::asset::AssetId::of(fee_asset_id, authority.clone()),
            Quantity::from(100_u32),
        );
        let mut world = World::with_assets(domains, accounts, [fee_definition], [fee_asset], []);
        install_account_alias_policy_for_test(&mut world, authority);
        install_onboarding_parent_leases_for_test(&mut world, domain_owner);
        let mut app = mk_app_state_for_tests_with_world(world);
        configure_recipient_lookup_sbp_dataspace_for_test(
            &mut app,
            iroha_data_model::nexus::LaneVisibility::Restricted,
        );
        app
    }

    fn install_account_alias_policy_for_test(world: &mut World, authority: &AccountId) {
        let mut policy = iroha_data_model::sns::fixtures::default_policy();
        policy.suffix_id = iroha_data_model::sns::ACCOUNT_ALIAS_SUFFIX_ID;
        policy.suffix = "account-alias".to_owned();
        policy.steward = authority.clone();
        policy.fund_splitter_account = authority.clone();
        policy.payment_asset_id = iroha_config::parameters::defaults::nexus::fees::fee_asset_id();
        for tier in &mut policy.pricing {
            tier.base_price.asset_id = policy.payment_asset_id.clone();
        }
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::policy_storage_key(iroha_data_model::sns::ACCOUNT_ALIAS_SUFFIX_ID),
            norito::codec::Encode::encode(&policy),
        );
    }

    fn install_onboarding_parent_leases_for_test(world: &mut World, owner: &AccountId) {
        let controller = iroha_data_model::sns::NameControllerV1::account(
            &AccountAddress::from_account_id(owner).expect("parent lease owner address"),
        );
        let dataspace_selector =
            iroha_core::sns::selector_for_dataspace_alias("sbp").expect("SBP selector");
        let mut dataspace_metadata = iroha_data_model::metadata::Metadata::default();
        dataspace_metadata.insert(
            iroha_core::sns::SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace metadata key"),
            iroha_primitives::json::Json::new(recipient_lookup_sbp_dataspace_for_test().as_u64()),
        );
        let dataspace_record = iroha_data_model::sns::NameRecordV1::new(
            dataspace_selector.clone(),
            owner.clone(),
            vec![controller.clone()],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            dataspace_metadata,
        );
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::record_storage_key(&dataspace_selector),
            norito::codec::Encode::encode(&dataspace_record),
        );

        for name in ["hbl", "ubl"] {
            let domain = DomainId::try_new(name, "sbp").expect("onboarding parent domain");
            let selector = iroha_core::sns::selector_for_domain(&domain).expect("domain selector");
            let record = iroha_data_model::sns::NameRecordV1::new(
                selector.clone(),
                owner.clone(),
                vec![controller.clone()],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                iroha_data_model::metadata::Metadata::default(),
            );
            world.smart_contract_state_mut_for_testing().insert(
                iroha_core::sns::record_storage_key(&selector),
                norito::codec::Encode::encode(&record),
            );
        }
    }

    fn grant_account_permissions_for_test(
        app: &SharedAppState,
        authority: &AccountId,
        permissions: impl IntoIterator<Item = Permission>,
    ) {
        let height = next_block_height(app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        for permission in permissions {
            stx.world_mut_for_testing()
                .add_account_permission(authority, permission);
        }
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block
            .commit()
            .expect("commit should persist onboarding permissions");
    }

    fn onboarding_credential_domain_permissions(domain: &DomainId) -> [Permission; 1] {
        [Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(domain.clone()),
        })]
    }

    fn onboarding_fee_sponsor_program_for_test(account: &AccountId) -> FeeSponsorProgramId {
        FeeSponsorProgramId::new(
            account.clone(),
            "retail".parse().expect("retail fee sponsor program name"),
        )
    }

    fn onboarding_fee_sponsor_enrollment_permission(
        program_id: &FeeSponsorProgramId,
    ) -> Permission {
        Permission::from(CanEnrollFeeSponsorProgram {
            program_id: program_id.clone(),
        })
    }

    fn register_fee_sponsor_program_for_test(
        app: &SharedAppState,
        program_id: FeeSponsorProgramId,
    ) {
        let height = next_block_height(app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        iroha_data_model::isi::nexus::CreateFeeSponsorProgram {
            program: FeeSponsorProgram::new(program_id.clone()),
        }
        .execute(&program_id.sponsor, &mut stx)
        .expect("sponsor may register its program");
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit fee sponsor program fixture");
    }

    fn onboarding_alias_test_app_with_role_permissions(
        authority: &AccountId,
        domain_owner: &AccountId,
        permissions: impl IntoIterator<Item = Permission>,
    ) -> SharedAppState {
        let mut accounts = vec![Account::new(authority.clone()).build(authority)];
        if domain_owner != authority {
            accounts.push(Account::new(domain_owner.clone()).build(domain_owner));
        }
        let domains = [
            Domain::new(DomainId::try_new("hbl", "sbp").expect("HBL domain")).build(domain_owner),
            Domain::new(DomainId::try_new("ubl", "sbp").expect("UBL domain")).build(domain_owner),
        ];
        let role_id: RoleId = "onboarding_credential_role".parse().expect("role id");
        let role = permissions
            .into_iter()
            .fold(
                Role::new(role_id.clone(), authority.clone()),
                |role, permission| role.add_permission(permission),
            )
            .build(authority);
        let fee_asset_id: AssetDefinitionId =
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
                .parse()
                .expect("default fee asset id");
        let fee_definition =
            iroha_data_model::asset::AssetDefinition::numeric(fee_asset_id.clone())
                .with_name("xor".to_owned())
                .build(authority);
        let fee_asset = iroha_data_model::asset::Asset::new(
            iroha_data_model::asset::AssetId::of(fee_asset_id, authority.clone()),
            Quantity::from(100_u32),
        );
        let mut world = World::with_assets_and_roles(
            domains,
            accounts,
            [fee_definition],
            [fee_asset],
            std::iter::empty::<iroha_data_model::nft::Nft>(),
            [role],
        );
        install_account_alias_policy_for_test(&mut world, authority);
        install_onboarding_parent_leases_for_test(&mut world, domain_owner);
        world.grant_role_for_tests(authority.clone(), role_id);
        let mut app = mk_app_state_for_tests_with_world(world);
        configure_recipient_lookup_sbp_dataspace_for_test(
            &mut app,
            iroha_data_model::nexus::LaneVisibility::Restricted,
        );
        app
    }

    fn onboarding_alias_signer_for_test(key_pair: &KeyPair) -> AccountOnboardingSigner {
        AccountOnboardingSigner {
            authority: AccountId::new(key_pair.public_key().clone()),
            private_key: ExposedPrivateKey(key_pair.private_key().clone()),
            api_token_hashes_by_domain: BTreeMap::new(),
            api_token_hashes_by_dataspace: BTreeMap::new(),
            allowed_permissions: BTreeSet::new(),
            fee_sponsor_program_id: None,
            alias_lease_term_years: 1,
            owner_auto_renew: None,
        }
    }

    fn assert_onboarding_readiness_blocked(
        app: &SharedAppState,
        signer: &AccountOnboardingSigner,
        message: &str,
    ) {
        let report = validate_account_onboarding_readiness(app.state.as_ref(), signer);
        assert_ne!(
            report.status,
            iroha_data_model::alias_setup::AliasSetupStatusV1::Ready,
            "{message}: readiness unexpectedly succeeded: {report:?}"
        );
        assert!(!report.diagnostics.is_empty(), "{message}");
    }

    fn assert_onboarding_readiness_ready(app: &SharedAppState, signer: &AccountOnboardingSigner) {
        let report = validate_account_onboarding_readiness(app.state.as_ref(), signer);
        assert_eq!(
            report.status,
            iroha_data_model::alias_setup::AliasSetupStatusV1::Ready,
            "{report:?}"
        );
        assert!(report.diagnostics.is_empty());
    }
