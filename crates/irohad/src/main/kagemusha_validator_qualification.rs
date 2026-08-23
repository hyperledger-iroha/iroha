//! Fail-closed daemon-side availability and exact-source projection for one
//! Kagemusha V4 validator qualification seal.

use iroha_core::smartcontracts::isi::offline::{
    KagemushaValidatorQualificationCatalogCaptureV1,
    VerifiedKagemushaV4RuntimeEffectiveConfigV1,
};
use iroha_crypto::{KeyPair, PublicKey};
use iroha_data_model::{
    offline::KagemushaV4ValidatorQualificationSealV1,
    peer::PeerId,
};
use iroha_genesis::GenesisBlock;

/// Same-read startup sources which may be consumed only inside the daemon.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct KagemushaStartupQualificationSourcesV1 {
    snapshot_bootstrap: bool,
    flattened_toml_config_source: Option<Vec<u8>>,
    signed_genesis_source: Option<Vec<u8>>,
}

impl KagemushaStartupQualificationSourcesV1 {
    /// Retain exact buffers returned by the startup readers.
    pub(super) fn new(
        snapshot_bootstrap: bool,
        flattened_toml_config_source: Option<Vec<u8>>,
        signed_genesis_source: Option<Vec<u8>>,
    ) -> Self {
        Self {
            snapshot_bootstrap,
            flattened_toml_config_source,
            signed_genesis_source,
        }
    }

    /// Borrow the exact flattened TOML bytes retained by startup.
    pub(super) fn flattened_toml_config_source(&self) -> Option<&[u8]> {
        self.flattened_toml_config_source.as_deref()
    }

    /// Borrow the exact signed-genesis bytes used by the decoder.
    pub(super) fn signed_genesis_source(&self) -> Option<&[u8]> {
        self.signed_genesis_source.as_deref()
    }
}

/// Same-read controller reservation plus independently pinned receipt authority.
#[derive(Clone, Copy)]
pub(super) struct KagemushaTrustedPromotionInputsV1<'a> {
    pinned_controller: &'a PublicKey,
    exact_reservation_bytes: &'a [u8],
    catalog_revalidation_receipt_json: &'a [u8],
    catalog_revalidation_authority_key_id: &'a str,
    catalog_revalidation_authority_public_key: &'a PublicKey,
}

impl<'a> KagemushaTrustedPromotionInputsV1<'a> {
    /// Bundle exact buffers without reducing them to caller-supplied digests.
    pub(super) const fn new(
        pinned_controller: &'a PublicKey,
        exact_reservation_bytes: &'a [u8],
        catalog_revalidation_receipt_json: &'a [u8],
        catalog_revalidation_authority_key_id: &'a str,
        catalog_revalidation_authority_public_key: &'a PublicKey,
    ) -> Self {
        Self {
            pinned_controller,
            exact_reservation_bytes,
            catalog_revalidation_receipt_json,
            catalog_revalidation_authority_key_id,
            catalog_revalidation_authority_public_key,
        }
    }
}

/// Explicit reason why this process did not create a validator seal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KagemushaValidatorQualificationUnavailableV1 {
    /// Snapshot bootstrap has no ordinary signed-genesis source in this corridor.
    SnapshotBootstrap,
    /// The protected promotion controller supplied no trusted reservation.
    MissingTrustedPromotionReservation,
    /// Startup did not read one integrity-bound flattened TOML source.
    MissingFlattenedConfigSource,
    /// Startup did not retain one exact ordinary signed-genesis body.
    MissingSignedGenesis,
    /// Full genesis validation did not produce a runtime-effective projection.
    MissingRuntimeEffectiveConfig,
    /// No validator signer was authorized for this attempt.
    MissingValidatorSigner,
    /// No same-load catalog qualification capture was available.
    MissingCatalogQualification,
}

/// Result of the local one-seal seam; it never publishes or collects seals.
#[derive(Debug)]
#[allow(variant_size_differences)] // Boxing the one-byte unavailable reason would add a needless allocation.
pub(super) enum KagemushaValidatorQualificationOutcomeV1 {
    /// Qualification was deliberately unavailable and no signature was made.
    Unavailable(KagemushaValidatorQualificationUnavailableV1),
    /// One locally signed validator seal, not published or submitted.
    Signed(Box<KagemushaV4ValidatorQualificationSealV1>),
}

/// Attempt one local qualification without inventing any missing source.
///
/// Returns an explicit unavailable outcome for every absent trust input and an
/// error only when all inputs exist but fail closed validation or signing.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_build_kagemusha_validator_qualification_v1(
    sources: &KagemushaStartupQualificationSourcesV1,
    promotion: Option<KagemushaTrustedPromotionInputsV1<'_>>,
    catalog_capture: Option<&KagemushaValidatorQualificationCatalogCaptureV1>,
    genesis: Option<&GenesisBlock>,
    runtime_effective_config: Option<&VerifiedKagemushaV4RuntimeEffectiveConfigV1>,
    validator_id: &PeerId,
    validator_signer: Option<&KeyPair>,
) -> Result<KagemushaValidatorQualificationOutcomeV1, String> {
    if sources.snapshot_bootstrap {
        return Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
            KagemushaValidatorQualificationUnavailableV1::SnapshotBootstrap,
        ));
    }
    let Some(promotion) = promotion else {
        return Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
            KagemushaValidatorQualificationUnavailableV1::MissingTrustedPromotionReservation,
        ));
    };
    let Some(flattened_toml_config_source) = sources.flattened_toml_config_source() else {
        return Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
            KagemushaValidatorQualificationUnavailableV1::MissingFlattenedConfigSource,
        ));
    };
    let (Some(genesis), Some(signed_genesis_source)) = (genesis, sources.signed_genesis_source())
    else {
        return Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
            KagemushaValidatorQualificationUnavailableV1::MissingSignedGenesis,
        ));
    };
    let Some(validator_signer) = validator_signer else {
        return Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
            KagemushaValidatorQualificationUnavailableV1::MissingValidatorSigner,
        ));
    };
    let Some(catalog_capture) = catalog_capture else {
        return Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
            KagemushaValidatorQualificationUnavailableV1::MissingCatalogQualification,
        ));
    };
    let Some(runtime_effective_config) = runtime_effective_config else {
        return Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
            KagemushaValidatorQualificationUnavailableV1::MissingRuntimeEffectiveConfig,
        ));
    };
    catalog_capture
        .build_and_sign_validator_qualification_from_reservation_v1(
            promotion.exact_reservation_bytes,
            promotion.pinned_controller,
            promotion.catalog_revalidation_receipt_json,
            promotion.catalog_revalidation_authority_key_id,
            promotion.catalog_revalidation_authority_public_key,
            genesis,
            signed_genesis_source,
            flattened_toml_config_source,
            runtime_effective_config,
            validator_id,
            validator_signer,
        )
        .map(Box::new)
        .map(KagemushaValidatorQualificationOutcomeV1::Signed)
}

/// Exercise the fail-closed seam for the stock launcher, which has no trusted
/// promotion reservation or same-load catalog capture and therefore cannot sign.
pub(super) fn evaluate_stock_launcher_unavailable_v1(
    sources: &KagemushaStartupQualificationSourcesV1,
    genesis: Option<&GenesisBlock>,
    validator_id: &PeerId,
    validator_signer: &KeyPair,
) -> Result<(), String> {
    match try_build_kagemusha_validator_qualification_v1(
        sources,
        None,
        None,
        genesis,
        None,
        validator_id,
        Some(validator_signer),
    )? {
        KagemushaValidatorQualificationOutcomeV1::Unavailable(reason) => {
            let _ = reason;
        }
        KagemushaValidatorQualificationOutcomeV1::Signed(seal) => {
            let _ = seal;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(super) fn reservation_fixture(
        controller: &KeyPair,
    ) -> iroha_data_model::offline::KagemushaV4PromotionReservationV1 {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::{
            NetworkId,
            offline::{
                KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                KAGEMUSHA_V4_PROMOTION_RESERVATION_BODY_SCHEMA, KagemushaExactBytesDigestV1,
                KagemushaV4GitHubPromotionRunV1, KagemushaV4PromotionReservationBodyV1,
                KagemushaV4PromotionReservationV1,
            },
        };

        let github_run = KagemushaV4GitHubPromotionRunV1 {
            repository: "hyperledger/iroha".to_owned(),
            workflow_ref: ".github/workflows/kagemusha.yml@refs/heads/main".to_owned(),
            workflow_sha: [0x41; 20],
            run_id: 41,
            run_attempt: 1,
        };
        let exact = |bytes: &[u8]| {
            KagemushaExactBytesDigestV1::from_bytes(bytes).expect("nonempty exact fixture")
        };
        let policy = iroha_core::smartcontracts::isi::offline::isi::production_offline_device_attestation_policy_v1(
            "TEAMID1234".to_owned(),
            "io.soramitsu.pk".to_owned(),
            vec![4],
            vec!["41".to_owned()],
            "com.pk.retailwallet".to_owned(),
            vec![[0x55; 32]],
            1_800_000_000_000,
        )
        .expect("production device policy fixture");
        let body = KagemushaV4PromotionReservationBodyV1 {
            schema: KAGEMUSHA_V4_PROMOTION_RESERVATION_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            promotion_controller: controller.public_key().clone(),
            promotion_id: github_run.promotion_id(),
            github_run,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"reservation fixture network",
            ))),
            reviewed_source_closure_descriptor: exact(b"source descriptor"),
            manifest_sha256: [0x42; 32],
            release_record_sha256: [0x43; 32],
            promotion_record_norito: exact(b"promotion record"),
            release_policy_source: exact(b"release policy"),
            signed_genesis: exact(b"signed genesis"),
            catalog_revalidation_receipt_json: exact(b"catalog receipt"),
            catalog_revalidation_catalog_sha256: [0x45; 32],
            catalog_consensus_policy_digest: [0x44; 32],
            execution_policy_hash: Hash::new(b"execution policy"),
            device_attestation_policy: policy,
            policy_evaluation_time_ms: 1_800_000_000_000,
            validator_qualification_expires_at_unix_ms: 1_800_000_300_000,
        };
        KagemushaV4PromotionReservationV1::try_sign(body, controller)
            .expect("signed promotion reservation fixture")
    }

    #[test]
    fn unavailable_reasons_are_explicit_and_snapshot_fails_first() {
        let signer = KeyPair::from_seed(vec![0x11; 32], iroha_crypto::Algorithm::BlsNormal);
        let validator_id = PeerId::new(signer.public_key().clone());
        let snapshot = KagemushaStartupQualificationSourcesV1::new(true, None, None);
        assert!(matches!(
            try_build_kagemusha_validator_qualification_v1(
                &snapshot,
                None,
                None,
                None,
                None,
                &validator_id,
                None,
            ),
            Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
                KagemushaValidatorQualificationUnavailableV1::SnapshotBootstrap
            ))
        ));
        assert!(
            evaluate_stock_launcher_unavailable_v1(&snapshot, None, &validator_id, &signer,)
                .is_ok(),
            "the stock launcher must accept an explicit unavailable outcome"
        );
        let ordinary = KagemushaStartupQualificationSourcesV1::default();
        assert!(matches!(
            try_build_kagemusha_validator_qualification_v1(
                &ordinary,
                None,
                None,
                None,
                None,
                &validator_id,
                Some(&signer),
            ),
            Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
                KagemushaValidatorQualificationUnavailableV1::MissingTrustedPromotionReservation
            ))
        ));
    }

    #[test]
    fn exact_startup_buffers_are_retained_without_digest_substitution() {
        let sources = KagemushaStartupQualificationSourcesV1::new(
            false,
            Some(b"exact config".to_vec()),
            Some(b"exact genesis".to_vec()),
        );
        assert_eq!(
            sources.flattened_toml_config_source(),
            Some(b"exact config".as_slice())
        );
        assert_eq!(
            sources.signed_genesis_source(),
            Some(b"exact genesis".as_slice())
        );
    }

    #[test]
    fn unavailable_sources_signer_and_catalog_are_distinct() {
        let signer = KeyPair::from_seed(vec![0x21; 32], iroha_crypto::Algorithm::BlsNormal);
        let validator_id = PeerId::new(signer.public_key().clone());
        let controller = KeyPair::from_seed(vec![0x22; 32], iroha_crypto::Algorithm::Ed25519);
        let reservation = reservation_fixture(&controller);
        let reservation_bytes =
            norito::encode_canonical(&reservation).expect("canonical reservation fixture");
        let catalog_authority =
            KeyPair::from_seed(vec![0x23; 32], iroha_crypto::Algorithm::Ed25519);
        let promotion = KagemushaTrustedPromotionInputsV1::new(
            controller.public_key(),
            &reservation_bytes,
            b"catalog receipt",
            "fixture.catalog-authority-v1",
            catalog_authority.public_key(),
        );
        let missing_config = KagemushaStartupQualificationSourcesV1::default();
        assert!(matches!(
            try_build_kagemusha_validator_qualification_v1(
                &missing_config,
                Some(promotion),
                None,
                None,
                None,
                &validator_id,
                Some(&signer),
            ),
            Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
                KagemushaValidatorQualificationUnavailableV1::MissingFlattenedConfigSource
            ))
        ));
        let missing_genesis = KagemushaStartupQualificationSourcesV1::new(
            false,
            Some(b"exact config".to_vec()),
            None,
        );
        assert!(matches!(
            try_build_kagemusha_validator_qualification_v1(
                &missing_genesis,
                Some(promotion),
                None,
                None,
                None,
                &validator_id,
                Some(&signer),
            ),
            Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
                KagemushaValidatorQualificationUnavailableV1::MissingSignedGenesis
            ))
        ));
        let genesis_signer = KeyPair::from_seed(vec![0x22; 32], iroha_crypto::Algorithm::Ed25519);
        let genesis = iroha_genesis::GenesisBuilder::new_without_executor(
            iroha_data_model::ChainId::from("qualification-unavailable-fixture"),
            ".",
        )
        .build_raw()
        .with_consensus_meta()
        .build_and_sign(&genesis_signer)
        .expect("signed genesis fixture");
        let genesis_bytes = genesis.0.encode_wire().expect("canonical genesis fixture");
        let complete_sources = KagemushaStartupQualificationSourcesV1::new(
            false,
            Some(b"exact config".to_vec()),
            Some(genesis_bytes),
        );
        assert!(matches!(
            try_build_kagemusha_validator_qualification_v1(
                &complete_sources,
                Some(promotion),
                None,
                Some(&genesis),
                None,
                &validator_id,
                None,
            ),
            Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
                KagemushaValidatorQualificationUnavailableV1::MissingValidatorSigner
            ))
        ));
        assert!(matches!(
            try_build_kagemusha_validator_qualification_v1(
                &complete_sources,
                Some(promotion),
                None,
                Some(&genesis),
                None,
                &validator_id,
                Some(&signer),
            ),
            Ok(KagemushaValidatorQualificationOutcomeV1::Unavailable(
                KagemushaValidatorQualificationUnavailableV1::MissingCatalogQualification
            ))
        ));
    }
}
