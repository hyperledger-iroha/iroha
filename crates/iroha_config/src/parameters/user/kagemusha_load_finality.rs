//! No implicit trust, artifact provisioning or proof-provider fallback.

use super::*;

/// Optional independently selected server proof graph and bounded existing custody.
#[derive(Clone, ReadConfig, norito::JsonDeserialize)]
pub struct KagemushaLoadFinality {
    /// Exact nonzero lowercase Scheme digest.
    pub scheme_id_hex: String,
    /// Exact nonzero lowercase independently selected signed manifest digest.
    pub manifest_digest_hex: String,
    /// Existing canonical signed verifier pack file.
    pub verifier_pack: PathBuf,
    /// Existing exact signed producer inventory preimage.
    pub producer_inventory: PathBuf,
    /// Existing private server proving-original directory; never wallet transport.
    pub server_originals: PathBuf,
    /// Existing private immutable proof journal directory.
    pub journal_dir: PathBuf,
    /// Maximum one original PK, at most 1 GiB.
    pub maximum_key_bytes: usize,
    /// Aggregate original graph extent ceiling, not RSS.
    pub maximum_original_bytes: usize,
    /// Maximum graph members, at most 65,536.
    pub maximum_artifacts: usize,
    /// Individual proof/MSM scratch ceiling, 1 MiB through 1 GiB.
    pub msm_bytes: usize,
    /// Maximum journal files, including incomplete original publication.
    pub maximum_journal_entries: usize,
    /// Maximum actual total journal extent in bytes.
    pub maximum_journal_bytes: u64,
    /// Combined queued, active and unread result limit, 1 through 64.
    pub max_pending_requests: usize,
    /// Native cursor allocation admission, 64 MiB through 4 GiB.
    pub native_working_set_bytes: usize,
    /// Native observation deadline per height, 1 through 60,000 milliseconds.
    pub native_step_timeout_ms: u64,
    /// Explicit maximum requested receipt height, 2 through 2^32-1.
    pub maximum_receipt_height: u64,
}
impl std::fmt::Debug for KagemushaLoadFinality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaLoadFinality")
            .finish_non_exhaustive()
    }
}
fn digest(value: &str) -> std::result::Result<[u8; 32], &'static str> {
    let mut out = [0; 32];
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || hex::decode_to_slice(value, &mut out).is_err()
        || out == [0; 32]
    {
        return Err("installation digests must be exact nonzero lowercase 32-byte hex");
    }
    Ok(out)
}
impl KagemushaLoadFinality {
    fn checked(self) -> std::result::Result<actual::KagemushaLoadFinality, &'static str> {
        for path in [
            &self.verifier_pack,
            &self.producer_inventory,
            &self.server_originals,
            &self.journal_dir,
        ] {
            let normalized: PathBuf = path.components().collect();
            if !path.is_absolute()
                || path.as_os_str().len() > 4096
                || normalized.as_os_str() != path.as_os_str()
                || path.components().any(|p| {
                    matches!(
                        p,
                        std::path::Component::CurDir | std::path::Component::ParentDir
                    )
                })
            {
                return Err(
                    "artifact and journal paths must have exact canonical absolute spelling",
                );
            }
        }
        if !(1..=1 << 30).contains(&self.maximum_key_bytes)
            || self.maximum_original_bytes == 0
            || self.maximum_original_bytes == usize::MAX
            || !(1..=65_536).contains(&self.maximum_artifacts)
            || !(1 << 20..=1 << 30).contains(&self.msm_bytes)
            || !(3..=1_000_000).contains(&self.maximum_journal_entries)
            || self.maximum_journal_bytes == 0
            || self.maximum_journal_bytes == u64::MAX
            || !(1..=64).contains(&self.max_pending_requests)
            || !(64_u64 << 20..=4_u64 << 30).contains(&(self.native_working_set_bytes as u64))
            || !(1..=60_000).contains(&self.native_step_timeout_ms)
            || !(2..=u64::from(u32::MAX)).contains(&self.maximum_receipt_height)
        {
            return Err("all finality producer bounds must be explicit and finite");
        }
        Ok(actual::KagemushaLoadFinality {
            scheme_id: digest(&self.scheme_id_hex)?,
            manifest_digest: digest(&self.manifest_digest_hex)?,
            verifier_pack: self.verifier_pack,
            producer_inventory: self.producer_inventory,
            server_originals: self.server_originals,
            journal_dir: self.journal_dir,
            maximum_key_bytes: self.maximum_key_bytes,
            maximum_original_bytes: self.maximum_original_bytes,
            maximum_artifacts: self.maximum_artifacts,
            msm_bytes: self.msm_bytes,
            maximum_journal_entries: self.maximum_journal_entries,
            maximum_journal_bytes: self.maximum_journal_bytes,
            max_pending_requests: self.max_pending_requests,
            native_working_set_bytes: self.native_working_set_bytes,
            native_step_timeout: Duration::from_millis(self.native_step_timeout_ms),
            maximum_receipt_height: self.maximum_receipt_height,
        })
    }
    pub(super) fn parse(
        self,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::KagemushaLoadFinality> {
        match self.checked() {
            Ok(config) => Some(config),
            Err(error) => {
                emit_torii_config_error(emitter, format!("torii.kagemusha_load_finality: {error}"));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected() -> KagemushaLoadFinality {
        KagemushaLoadFinality {
            scheme_id_hex: "11".repeat(32),
            manifest_digest_hex: "22".repeat(32),
            verifier_pack: "/var/lib/iroha/proof/verifier-pack".into(),
            producer_inventory: "/var/lib/iroha/proof/producer-inventory".into(),
            server_originals: "/var/lib/iroha/proof/originals".into(),
            journal_dir: "/var/lib/iroha/proof/journal".into(),
            maximum_key_bytes: 1 << 28,
            maximum_original_bytes: 1 << 30,
            maximum_artifacts: 4096,
            msm_bytes: 64 << 20,
            maximum_journal_entries: 100_000,
            maximum_journal_bytes: 1 << 30,
            max_pending_requests: 8,
            native_working_set_bytes: 256 << 20,
            native_step_timeout_ms: 10_000,
            maximum_receipt_height: 1_000_000,
        }
    }

    #[test]
    fn selection_preserves_exact_identity_paths_and_finite_limits_without_opening_them() {
        let selected = selected();
        let parsed = selected.clone().checked().unwrap();
        assert_eq!(parsed.scheme_id, [0x11; 32]);
        assert_eq!(parsed.manifest_digest, [0x22; 32]);
        assert_eq!(parsed.verifier_pack, selected.verifier_pack);
        assert_eq!(parsed.producer_inventory, selected.producer_inventory);
        assert_eq!(parsed.server_originals, selected.server_originals);
        assert_eq!(parsed.journal_dir, selected.journal_dir);
        assert_eq!(parsed.maximum_key_bytes, selected.maximum_key_bytes);
        assert_eq!(
            parsed.maximum_original_bytes,
            selected.maximum_original_bytes
        );
        assert_eq!(parsed.maximum_artifacts, selected.maximum_artifacts);
        assert_eq!(parsed.msm_bytes, selected.msm_bytes);
        assert_eq!(
            parsed.maximum_journal_entries,
            selected.maximum_journal_entries
        );
        assert_eq!(parsed.maximum_journal_bytes, selected.maximum_journal_bytes);
        assert_eq!(parsed.max_pending_requests, selected.max_pending_requests);
        assert_eq!(
            parsed.native_working_set_bytes,
            selected.native_working_set_bytes
        );
        assert_eq!(parsed.native_step_timeout, Duration::from_millis(10_000));
        assert_eq!(
            parsed.maximum_receipt_height,
            selected.maximum_receipt_height
        );
        let debug = format!("{parsed:?}");
        assert!(!debug.contains("/var/lib/iroha"));
        assert!(!format!("{selected:?}").contains("/var/lib/iroha"));
    }

    #[test]
    fn aliases_zero_identity_and_noncanonical_path_spellings_are_rejected() {
        for value in [
            "".into(),
            "00".repeat(32),
            "AB".repeat(32),
            "11".repeat(31),
            format!(" {}", "11".repeat(32)),
        ] {
            let mut config = selected();
            config.scheme_id_hex = value.clone();
            assert!(config.checked().is_err());
            let mut config = selected();
            config.manifest_digest_hex = value;
            assert!(config.checked().is_err());
        }
        for path in [
            "relative",
            "/var//proof",
            "/var/./proof",
            "/var/../proof",
            "/var/proof/",
        ] {
            for field in 0..4 {
                let mut config = selected();
                match field {
                    0 => config.verifier_pack = path.into(),
                    1 => config.producer_inventory = path.into(),
                    2 => config.server_originals = path.into(),
                    _ => config.journal_dir = path.into(),
                }
                assert!(config.checked().is_err(), "accepted {path}");
            }
        }
    }

    #[test]
    fn every_worker_import_and_journal_limit_is_finite_and_nonzero() {
        let invalid = [
            KagemushaLoadFinality {
                maximum_key_bytes: 0,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_key_bytes: (1 << 30) + 1,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_original_bytes: 0,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_original_bytes: usize::MAX,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_artifacts: 65_537,
                ..selected()
            },
            KagemushaLoadFinality {
                msm_bytes: (1 << 20) - 1,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_journal_entries: 2,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_journal_bytes: u64::MAX,
                ..selected()
            },
            KagemushaLoadFinality {
                max_pending_requests: 65,
                ..selected()
            },
            KagemushaLoadFinality {
                native_working_set_bytes: (64 << 20) - 1,
                ..selected()
            },
            KagemushaLoadFinality {
                native_step_timeout_ms: 60_001,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_receipt_height: 1,
                ..selected()
            },
            KagemushaLoadFinality {
                maximum_receipt_height: u64::from(u32::MAX) + 1,
                ..selected()
            },
        ];
        for config in invalid {
            assert!(config.checked().is_err());
        }
        let mut emitter = Emitter::new();
        assert!(
            KagemushaLoadFinality {
                native_step_timeout_ms: 0,
                ..selected()
            }
            .parse(&mut emitter)
            .is_none()
        );
        assert!(emitter.into_result().is_err());
        let mut emitter = Emitter::new();
        assert!(selected().parse(&mut emitter).is_some());
        assert!(emitter.into_result().is_ok());
    }

    #[test]
    fn optional_installation_never_invents_trust_or_resource_defaults() {
        use iroha_config_base::{read::ConfigReader, toml::TomlSource};
        assert!(
            ConfigReader::new()
                .with_toml_source(TomlSource::inline(toml::Table::new()))
                .read_and_complete::<KagemushaLoadFinality>()
                .is_err()
        );
    }
}
