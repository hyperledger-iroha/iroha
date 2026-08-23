//! Root-custodied reservation input and immutable validator-seal output for
//! the explicit no-bind Kagemusha qualification command.

use std::path::{Path, PathBuf};

use iroha_config::parameters::actual::Root as Config;
use iroha_core::smartcontracts::isi::offline::KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1;
use iroha_data_model::offline::{
    KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES,
    KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES, KagemushaV4PromotionReservationV1,
    KagemushaV4ValidatorQualificationSealV1,
};

use super::root_owned_artifact_publication::RootOwnedNoReplaceArtifactPublicationTarget;

const KAGEMUSHA_VALIDATOR_QUALIFICATION_SEAL_MAX_BYTES_V1: usize = 1024 * 1024;
const KAGEMUSHA_CATALOG_REVALIDATION_RECEIPT_ROOT_V1: &str =
    "/Library/SORA/Kagemusha/catalog-revalidation";

/// Controller-authenticated root-custodied promotion reservation.
pub struct TrustedKagemushaPromotionReservationV1 {
    exact_reservation_bytes: Vec<u8>,
    catalog_revalidation_receipt_json: Vec<u8>,
}

impl TrustedKagemushaPromotionReservationV1 {
    /// Borrow the same-read canonical reservation bytes used by the decoder.
    pub(super) fn exact_reservation_bytes(&self) -> &[u8] {
        &self.exact_reservation_bytes
    }

    /// Borrow the exact promotion-scoped catalog-revalidation JSON bytes.
    pub(super) fn catalog_revalidation_receipt_json(&self) -> &[u8] {
        &self.catalog_revalidation_receipt_json
    }
}

/// Read and authenticate the configured root-owned promotion reservation.
#[cfg(target_os = "macos")]
pub fn read_configured_kagemusha_promotion_reservation(
    config: &Config,
) -> Result<TrustedKagemushaPromotionReservationV1, String> {
    read_configured_kagemusha_promotion_reservation_with(config, |path, maximum, label| {
        RootOwnedNoReplaceArtifactPublicationTarget::read_root_owned_bounded(path, maximum, label)
    })
}

/// Reject the fixed macOS custody path on platforms without a reviewed root.
#[cfg(not(target_os = "macos"))]
pub fn read_configured_kagemusha_promotion_reservation(
    _config: &Config,
) -> Result<TrustedKagemushaPromotionReservationV1, String> {
    Err(
        "Kagemusha validator qualification is unsupported outside macOS until a platform-specific root-custody path is reviewed"
            .to_owned(),
    )
}

fn read_configured_kagemusha_promotion_reservation_with(
    config: &Config,
    mut read: impl FnMut(&Path, usize, &'static str) -> Result<Vec<u8>, String>,
) -> Result<TrustedKagemushaPromotionReservationV1, String> {
    let offline = &config.settlement.offline;
    let path = offline
        .kagemusha_promotion_reservation_path
        .as_deref()
        .ok_or_else(|| {
            "validator qualification requires kagemusha_promotion_reservation_path".to_owned()
        })?;
    let controller = offline
        .kagemusha_promotion_controller_public_key
        .as_ref()
        .ok_or_else(|| {
            "validator qualification requires kagemusha_promotion_controller_public_key".to_owned()
        })?;
    let exact_bytes = read(
        path,
        KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
        "Kagemusha promotion reservation",
    )?;
    let reservation =
        KagemushaV4PromotionReservationV1::decode_and_verify_canonical(&exact_bytes, controller)
            .map_err(|error| {
                format!("invalid configured Kagemusha promotion reservation: {error}")
            })?;
    let receipt_path =
        kagemusha_catalog_revalidation_receipt_path_v1(reservation.body.promotion_id);
    let catalog_revalidation_receipt_json = read(
        &receipt_path,
        KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES,
        "Kagemusha promotion-scoped catalog-revalidation receipt",
    )?;
    if !reservation
        .body
        .catalog_revalidation_receipt_json
        .matches_bytes(&catalog_revalidation_receipt_json)
    {
        return Err(
            "configured Kagemusha promotion reservation does not bind the exact promotion-scoped catalog-revalidation receipt"
                .to_owned(),
        );
    }
    Ok(TrustedKagemushaPromotionReservationV1 {
        exact_reservation_bytes: exact_bytes,
        catalog_revalidation_receipt_json,
    })
}

fn kagemusha_catalog_revalidation_receipt_path_v1(promotion_id: [u8; 32]) -> PathBuf {
    Path::new(KAGEMUSHA_CATALOG_REVALIDATION_RECEIPT_ROOT_V1)
        .join(format!("{}.json", hex::encode(promotion_id)))
}

/// Read exact bytes of the already-published catalog seal for same-load comparison.
pub fn read_configured_kagemusha_catalog_qualification_seal(
    config: &Config,
) -> Result<Vec<u8>, String> {
    let path = config
        .settlement
        .offline
        .kagemusha_catalog_qualification_seal_path
        .as_deref()
        .ok_or_else(|| {
            "validator qualification requires kagemusha_catalog_qualification_seal_path".to_owned()
        })?;
    RootOwnedNoReplaceArtifactPublicationTarget::read_root_owned_bounded(
        path,
        KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1,
        "Kagemusha catalog qualification seal",
    )
}

/// Prepared root-owned, no-replace destination for one local validator seal.
pub struct KagemushaValidatorSealPublicationTarget {
    inner: RootOwnedNoReplaceArtifactPublicationTarget,
}

impl KagemushaValidatorSealPublicationTarget {
    /// Prepare the configured absent output path under root custody.
    pub(super) fn prepare(config: &Config, requested_path: &Path) -> Result<Self, String> {
        let configured_path = config
            .settlement
            .offline
            .kagemusha_validator_qualification_seal_path
            .as_deref()
            .ok_or_else(|| {
                "--write-kagemusha-validator-qualification-seal requires settlement.offline.kagemusha_validator_qualification_seal_path"
                    .to_owned()
            })?;
        if requested_path != configured_path {
            return Err(format!(
                "requested validator seal path `{}` does not exactly match configured path `{}`",
                requested_path.display(),
                configured_path.display()
            ));
        }
        validate_validator_seal_directory_separation(config, requested_path)?;
        RootOwnedNoReplaceArtifactPublicationTarget::prepare_root_owned(
            requested_path,
            "Kagemusha validator qualification seal",
        )
        .map(|inner| Self { inner })
        .map_err(|error| error.to_string())
    }

    /// Return the immutable output path.
    pub(super) fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Publish one already-verified seal and retain the final inode on uncertain commit.
    pub(super) fn publish_and_verify(
        self,
        seal: &KagemushaV4ValidatorQualificationSealV1,
    ) -> Result<(), String> {
        seal.verify()
            .map_err(|error| format!("invalid Kagemusha validator qualification seal: {error}"))?;
        let canonical = norito::encode_canonical(seal)
            .map_err(|error| format!("failed to encode Kagemusha validator seal: {error}"))?;
        if canonical.is_empty()
            || canonical.len() > KAGEMUSHA_VALIDATOR_QUALIFICATION_SEAL_MAX_BYTES_V1
        {
            return Err(format!(
                "Kagemusha validator seal exceeds the {KAGEMUSHA_VALIDATOR_QUALIFICATION_SEAL_MAX_BYTES_V1}-byte limit"
            ));
        }
        self.inner
            .publish_bytes_and_verify(&canonical, |_| {
                seal.verify().map_err(|error| {
                    format!("published Kagemusha validator seal failed verification: {error}")
                })
            })
            .map_err(|error| error.to_string())
    }
}

fn validate_validator_seal_directory_separation(
    config: &Config,
    output_path: &Path,
) -> Result<(), String> {
    let output_parent = output_path
        .parent()
        .ok_or_else(|| "validator seal path must have a parent directory".to_owned())?;
    let offline = &config.settlement.offline;
    if offline
        .kagemusha_artifact_dir
        .as_deref()
        .is_some_and(|artifact_dir| {
            output_parent.starts_with(artifact_dir) || artifact_dir.starts_with(output_parent)
        })
    {
        return Err(
            "validator seal output directory must be disjoint from the authenticated artifact tree"
                .to_owned(),
        );
    }
    for (label, source_parent) in [
        (
            "release policy",
            offline
                .kagemusha_release_policy_path
                .as_deref()
                .and_then(Path::parent),
        ),
        (
            "catalog qualification seal",
            offline
                .kagemusha_catalog_qualification_seal_path
                .as_deref()
                .and_then(Path::parent),
        ),
        (
            "promotion reservation",
            offline
                .kagemusha_promotion_reservation_path
                .as_deref()
                .and_then(Path::parent),
        ),
        (
            "catalog-revalidation receipt",
            Some(Path::new(KAGEMUSHA_CATALOG_REVALIDATION_RECEIPT_ROOT_V1)),
        ),
    ] {
        if source_parent.is_some_and(|source_parent| {
            output_parent.starts_with(source_parent) || source_parent.starts_with(output_parent)
        }) {
            return Err(format!(
                "validator seal output directory must be disjoint from the {label} directory"
            ));
        }
    }
    let executable_parent = std::env::current_exe()
        .map_err(|error| format!("failed to resolve current executable path: {error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "current executable has no parent directory".to_owned())?;
    if output_parent.starts_with(&executable_parent) || executable_parent.starts_with(output_parent)
    {
        return Err(
            "validator seal output directory must be disjoint from the executable directory"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::base::toml::TomlSource;
    use iroha_crypto::{Algorithm, KeyPair};
    use std::path::PathBuf;

    fn config_fixture() -> Config {
        Config::from_toml_source(TomlSource::inline(
            crate::config_tests::minimal_config_table(),
        ))
        .expect("minimal production config fixture")
    }

    #[test]
    fn configured_reservation_reader_requires_exact_path_key_and_bytes() {
        let config = config_fixture();
        let error = read_configured_kagemusha_promotion_reservation_with(&config, |_, _, _| {
            unreachable!("missing config must fail before reading")
        })
        .err()
        .expect("missing reservation configuration must fail");
        assert!(error.contains("reservation_path"));

        let mut configured = config_fixture();
        configured
            .settlement
            .offline
            .kagemusha_promotion_reservation_path =
            Some(PathBuf::from("/trusted/reservation.norito"));
        configured
            .settlement
            .offline
            .kagemusha_promotion_controller_public_key = Some(
            iroha_crypto::KeyPair::from_seed(vec![0x31; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let error = read_configured_kagemusha_promotion_reservation_with(
            &configured,
            |path, maximum, label| {
                assert_eq!(path, Path::new("/trusted/reservation.norito"));
                assert_eq!(maximum, KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES);
                assert_eq!(label, "Kagemusha promotion reservation");
                Ok(b"not canonical Norito".to_vec())
            },
        )
        .err()
        .expect("noncanonical reservation must fail");
        assert!(error.contains("invalid configured"));
    }

    #[test]
    fn validator_output_rejects_config_path_substitution() {
        let mut config = config_fixture();
        config
            .settlement
            .offline
            .kagemusha_validator_qualification_seal_path =
            Some(PathBuf::from("/trusted/expected.norito"));
        let error = KagemushaValidatorSealPublicationTarget::prepare(
            &config,
            Path::new("/trusted/substituted.norito"),
        )
        .err()
        .expect("substituted output path must fail before custody inspection");
        assert!(error.contains("does not exactly match"));
    }

    #[test]
    fn validator_output_directory_must_not_contain_trusted_source_directories() {
        let mut config = config_fixture();
        config.settlement.offline.kagemusha_artifact_dir =
            Some(PathBuf::from("/trusted/kagemusha/artifacts"));
        let output = Path::new("/trusted/kagemusha/validator.norito");
        let error = validate_validator_seal_directory_separation(&config, output)
            .expect_err("an output ancestor of the artifact tree must fail closed");
        assert!(error.contains("authenticated artifact tree"));

        config.settlement.offline.kagemusha_artifact_dir = None;
        config
            .settlement
            .offline
            .kagemusha_promotion_reservation_path = Some(PathBuf::from(
            "/trusted/kagemusha/promotion/reservation.norito",
        ));
        let error = validate_validator_seal_directory_separation(&config, output)
            .expect_err("an output ancestor of the reservation directory must fail closed");
        assert!(error.contains("promotion reservation"));

        let output = Path::new("/Library/SORA/Kagemusha/validator.norito");
        let error = validate_validator_seal_directory_separation(&config_fixture(), output)
            .expect_err("an output ancestor of the fixed revalidation root must fail closed");
        assert!(error.contains("catalog-revalidation receipt"));
    }

    #[test]
    fn validator_output_directory_must_not_nest_under_trusted_source_directories() {
        let mut config = config_fixture();
        config
            .settlement
            .offline
            .kagemusha_catalog_qualification_seal_path =
            Some(PathBuf::from("/trusted/catalog/seal.norito"));
        let output = Path::new("/trusted/catalog/validator/seal.norito");
        let error = validate_validator_seal_directory_separation(&config, output)
            .expect_err("an output nested below the catalog-seal directory must fail closed");
        assert!(error.contains("catalog qualification seal"));
    }

    #[test]
    fn configured_reservation_reader_pins_the_derived_catalog_receipt() {
        let controller = KeyPair::from_seed(vec![0x45; 32], Algorithm::Ed25519);
        let reservation =
            super::super::kagemusha_validator_qualification::tests::reservation_fixture(
                &controller,
            );
        let reservation_bytes =
            norito::encode_canonical(&reservation).expect("canonical reservation fixture");
        let mut config = config_fixture();
        config
            .settlement
            .offline
            .kagemusha_promotion_reservation_path =
            Some(PathBuf::from("/trusted/reservation.norito"));
        config
            .settlement
            .offline
            .kagemusha_promotion_controller_public_key = Some(controller.public_key().clone());
        let expected_receipt_path =
            kagemusha_catalog_revalidation_receipt_path_v1(reservation.body.promotion_id);
        let mut reads = 0_u8;
        let trusted = read_configured_kagemusha_promotion_reservation_with(
            &config,
            |path, maximum, label| {
                reads += 1;
                match reads {
                    1 => {
                        assert_eq!(path, Path::new("/trusted/reservation.norito"));
                        assert_eq!(maximum, KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES);
                        assert_eq!(label, "Kagemusha promotion reservation");
                        Ok(reservation_bytes.clone())
                    }
                    2 => {
                        assert_eq!(path, expected_receipt_path.as_path());
                        assert_eq!(maximum, KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES);
                        assert_eq!(
                            label,
                            "Kagemusha promotion-scoped catalog-revalidation receipt"
                        );
                        Ok(b"catalog receipt".to_vec())
                    }
                    _ => panic!("unexpected extra trusted-artifact read"),
                }
            },
        )
        .expect("exact reservation and promotion-scoped receipt are admitted");
        assert_eq!(reads, 2);
        assert_eq!(trusted.exact_reservation_bytes(), reservation_bytes);
        assert_eq!(
            trusted.catalog_revalidation_receipt_json(),
            b"catalog receipt"
        );

        let error = read_configured_kagemusha_promotion_reservation_with(&config, |path, _, _| {
            if path == Path::new("/trusted/reservation.norito") {
                Ok(reservation_bytes.clone())
            } else {
                Ok(b"substituted catalog receipt".to_vec())
            }
        })
        .err()
        .expect("a different promotion-scoped receipt must fail closed");
        assert!(error.contains("does not bind the exact"));
    }

    #[test]
    fn promotion_receipt_path_is_fixed_lowercase_hex_without_traversal() {
        let mut promotion_id = [0_u8; 32];
        for (index, byte) in promotion_id.iter_mut().enumerate() {
            *byte = u8::try_from(index * 7).expect("fixture byte fits u8");
        }
        let path = kagemusha_catalog_revalidation_receipt_path_v1(promotion_id);
        assert_eq!(
            path,
            Path::new(KAGEMUSHA_CATALOG_REVALIDATION_RECEIPT_ROOT_V1)
                .join("00070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d9.json")
        );
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .expect("fixed path has UTF-8 file name");
        assert_eq!(file_name.len(), 69);
        assert!(
            file_name[..64]
                .bytes()
                .all(|byte| { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) })
        );
        assert!(!file_name.contains(".."));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn live_reader_rejects_unreviewed_platform_root() {
        let error = read_configured_kagemusha_promotion_reservation(&config_fixture())
            .expect_err("unreviewed platform must fail before reading any path");
        assert!(error.contains("unsupported outside macOS"));
    }
}
