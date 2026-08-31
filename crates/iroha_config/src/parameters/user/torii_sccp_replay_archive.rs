/// User configuration for the independently rebuildable SCCP replay archive.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ToriiSccpReplayArchive {
    /// Enable authenticated three-replica replay reads.
    #[config(default = "defaults::torii::sccp_replay_archive::ENABLED")]
    pub enabled: bool,
    /// Direct owner-only state directory. Required and absolute when enabled.
    pub state_dir: Option<PathBuf>,
    /// Exactly three independently hosted, independently keyed replicas.
    #[config(default)]
    pub replicas: Vec<ToriiSccpReplayArchiveReplica>,
    /// Complete response limit applied before buffering and decoding.
    #[config(default = "defaults::torii::sccp_replay_archive::MAX_RESPONSE_BYTES")]
    pub max_response_bytes: Bytes,
    /// Per-snapshot encoded byte limit applied before in-memory decoding.
    #[config(default = "defaults::torii::sccp_replay_archive::MAX_SNAPSHOT_BYTES")]
    pub max_snapshot_bytes: Bytes,
    /// Per-snapshot in-memory leaf cardinality limit.
    #[config(default = "defaults::torii::sccp_replay_archive::MAX_SNAPSHOT_LEAVES")]
    pub max_snapshot_leaves: usize,
    /// Maximum accumulator count in one agreed checkpoint set.
    #[config(default = "defaults::torii::sccp_replay_archive::MAX_ACCUMULATORS")]
    pub max_accumulators: usize,
    /// Complete timeout for one pinned replica request.
    #[config(default = "DurationMs(defaults::torii::sccp_replay_archive::REQUEST_TIMEOUT)")]
    pub request_timeout_ms: DurationMs,
}

impl Default for ToriiSccpReplayArchive {
    fn default() -> Self {
        use defaults::torii::sccp_replay_archive as archive;
        Self {
            enabled: archive::ENABLED,
            state_dir: None,
            replicas: Vec::new(),
            max_response_bytes: archive::MAX_RESPONSE_BYTES,
            max_snapshot_bytes: archive::MAX_SNAPSHOT_BYTES,
            max_snapshot_leaves: archive::MAX_SNAPSHOT_LEAVES,
            max_accumulators: archive::MAX_ACCUMULATORS,
            request_timeout_ms: DurationMs(archive::REQUEST_TIMEOUT),
        }
    }
}

const SCCP_REPLAY_NORITO_ARCHIVE_LIMIT_ERROR: &str = "torii.sccp_replay_archive.max_snapshot_bytes must not exceed norito.max_archive_len when the replay archive is enabled";

fn validate_sccp_replay_archive_norito_limit(
    replay_archive: &ToriiSccpReplayArchive,
    norito: &Norito,
) -> core::result::Result<(), &'static str> {
    if replay_archive.enabled && replay_archive.max_snapshot_bytes.get() > norito.max_archive_len {
        return Err(SCCP_REPLAY_NORITO_ARCHIVE_LIMIT_ERROR);
    }
    Ok(())
}

/// One configured SCCP replay archive replica.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ToriiSccpReplayArchiveReplica {
    /// Exact lowercase hexadecimal 32-byte replica identity.
    pub replica_id_hex: String,
    /// Exact canonical HTTPS origin.
    pub origin: Url,
    /// Exact Ed25519 verification key.
    pub public_key: PublicKey,
}

impl ToriiSccpReplayArchive {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::ToriiSccpReplayArchive> {
        match self.into_actual() {
            Ok(value) => value,
            Err(message) => {
                emit_torii_config_error(emitter, message);
                None
            }
        }
    }

    fn into_actual(
        self,
    ) -> core::result::Result<Option<actual::ToriiSccpReplayArchive>, &'static str> {
        use defaults::torii::sccp_replay_archive as limits;

        if !self.enabled {
            if self.state_dir.is_some() || !self.replicas.is_empty() {
                return Err(
                    "torii.sccp_replay_archive must not configure a state directory or replicas while disabled",
                );
            }
            return Ok(None);
        }

        let state_dir = self
            .state_dir
            .ok_or("torii.sccp_replay_archive.state_dir is required when the archive is enabled")?;
        if !state_dir.is_absolute() {
            return Err("torii.sccp_replay_archive.state_dir must be an absolute direct path");
        }
        if self.replicas.len() != 3 {
            return Err("torii.sccp_replay_archive.replicas must contain exactly three entries");
        }

        let max_response_bytes = bounded_usize(
            self.max_response_bytes.get(),
            limits::MAX_RESPONSE_BYTES_HARD,
            "torii.sccp_replay_archive.max_response_bytes is outside the first-release bounds",
        )?;
        let max_snapshot_bytes = bounded_usize(
            self.max_snapshot_bytes.get(),
            limits::MAX_SNAPSHOT_BYTES_HARD,
            "torii.sccp_replay_archive.max_snapshot_bytes is outside the first-release bounds",
        )?;
        if max_snapshot_bytes > max_response_bytes {
            return Err(
                "torii.sccp_replay_archive.max_snapshot_bytes must not exceed max_response_bytes",
            );
        }
        let max_snapshot_leaves = bounded_usize(
            u64::try_from(self.max_snapshot_leaves).unwrap_or(u64::MAX),
            limits::MAX_SNAPSHOT_LEAVES_HARD,
            "torii.sccp_replay_archive.max_snapshot_leaves is outside the first-release bounds",
        )?;
        let max_accumulators = bounded_usize(
            u64::try_from(self.max_accumulators).unwrap_or(u64::MAX),
            limits::MAX_ACCUMULATORS_HARD,
            "torii.sccp_replay_archive.max_accumulators is outside the first-release bounds",
        )?;
        let request_timeout = self.request_timeout_ms.get();
        if request_timeout.is_zero() || request_timeout > limits::REQUEST_TIMEOUT_HARD {
            return Err(
                "torii.sccp_replay_archive.request_timeout_ms is outside the first-release bounds",
            );
        }

        let mut replicas = Vec::with_capacity(3);
        let mut origins = BTreeSet::new();
        let mut public_keys = BTreeSet::new();
        for replica in self.replicas {
            let replica_id = decode_nonzero_lower_hex_32(&replica.replica_id_hex).ok_or(
                "torii.sccp_replay_archive replica_id_hex must be exactly 64 lowercase hexadecimal characters and nonzero",
            )?;
            if !canonical_https_origin(&replica.origin)
                || !origins.insert(replica.origin.as_str().to_owned())
            {
                return Err(
                    "torii.sccp_replay_archive replica origins must be unique canonical HTTPS origins",
                );
            }
            let (algorithm, raw_key) = replica.public_key.try_to_bytes().map_err(
                |_| "torii.sccp_replay_archive replica public keys must be canonical Ed25519 keys",
            )?;
            let ed25519_public_key: [u8; 32] = raw_key.try_into().map_err(
                |_| "torii.sccp_replay_archive replica public keys must be canonical Ed25519 keys",
            )?;
            if algorithm != Algorithm::Ed25519
                || ed25519_public_key == [0; 32]
                || !public_keys.insert(ed25519_public_key)
            {
                return Err(
                    "torii.sccp_replay_archive replica public keys must be distinct nonzero Ed25519 keys",
                );
            }
            replicas.push(actual::ToriiSccpReplayArchiveReplica {
                replica_id,
                origin: replica.origin,
                ed25519_public_key,
            });
        }
        replicas.sort_unstable_by_key(|replica| replica.replica_id);
        if replicas
            .windows(2)
            .any(|pair| pair[0].replica_id == pair[1].replica_id)
        {
            return Err("torii.sccp_replay_archive replica identities must be unique");
        }
        let replicas: [actual::ToriiSccpReplayArchiveReplica; 3] = replicas
            .try_into()
            .map_err(|_| "torii.sccp_replay_archive requires exactly three replicas")?;

        Ok(Some(actual::ToriiSccpReplayArchive {
            state_dir,
            replicas,
            max_response_bytes,
            max_snapshot_bytes,
            max_snapshot_leaves,
            max_accumulators,
            request_timeout,
        }))
    }
}

fn bounded_usize(
    value: u64,
    hard_max: u64,
    message: &'static str,
) -> core::result::Result<usize, &'static str> {
    if value == 0 || value > hard_max {
        return Err(message);
    }
    usize::try_from(value).map_err(|_| message)
}

fn decode_nonzero_lower_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    let decoded = hex::decode(value).ok()?;
    let decoded: [u8; 32] = decoded.try_into().ok()?;
    (decoded != [0; 32]).then_some(decoded)
}

fn canonical_https_origin(origin: &Url) -> bool {
    origin.scheme() == "https"
        && origin.host_str().is_some_and(|host| !host.ends_with('.'))
        && origin.username().is_empty()
        && origin.password().is_none()
        && origin.path() == "/"
        && origin.query().is_none()
        && origin.fragment().is_none()
        && origin.as_str() == format!("{}/", origin.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replica(index: u8) -> ToriiSccpReplayArchiveReplica {
        let key = KeyPair::from_seed(vec![index; 32], Algorithm::Ed25519);
        ToriiSccpReplayArchiveReplica {
            replica_id_hex: hex::encode([index; 32]),
            origin: Url::parse(&format!("https://replay-{index}.example/")).expect("valid URL"),
            public_key: key.public_key().clone(),
        }
    }

    fn enabled() -> ToriiSccpReplayArchive {
        ToriiSccpReplayArchive {
            enabled: true,
            state_dir: Some(PathBuf::from("/var/lib/iroha/sccp-replay")),
            replicas: vec![replica(3), replica(1), replica(2)],
            ..ToriiSccpReplayArchive::default()
        }
    }

    #[test]
    fn replay_archive_is_disabled_without_placeholder_bindings() {
        let parsed = ToriiSccpReplayArchive::default()
            .into_actual()
            .expect("disabled default is valid");
        assert!(parsed.is_none());

        let mut dormant = ToriiSccpReplayArchive::default();
        dormant.state_dir = Some(PathBuf::from("/var/lib/iroha/dormant-replay"));
        assert!(dormant.into_actual().is_err());
    }

    #[test]
    fn enabled_replay_archive_orders_exact_three_pinned_replicas() {
        let parsed = enabled()
            .into_actual()
            .expect("valid archive config")
            .expect("archive enabled");
        assert_eq!(parsed.replicas[0].replica_id, [1; 32]);
        assert_eq!(parsed.replicas[1].replica_id, [2; 32]);
        assert_eq!(parsed.replicas[2].replica_id, [3; 32]);
    }

    #[test]
    fn replay_archive_rejects_ambiguous_or_insecure_bindings() {
        let mut cases = Vec::new();
        let mut missing = enabled();
        missing.replicas.pop();
        cases.push(missing);
        let mut relative = enabled();
        relative.state_dir = Some(PathBuf::from("replay"));
        cases.push(relative);
        let mut http = enabled();
        http.replicas[0].origin = Url::parse("http://replay-1.example/").expect("valid URL");
        cases.push(http);
        let mut path = enabled();
        path.replicas[0].origin =
            Url::parse("https://replay-1.example/checkpoint").expect("valid URL");
        cases.push(path);
        let mut dns_alias = enabled();
        dns_alias.replicas[0].origin = Url::parse("https://replay-1.example./").expect("valid URL");
        cases.push(dns_alias);
        let mut duplicate_id = enabled();
        duplicate_id.replicas[1].replica_id_hex = duplicate_id.replicas[0].replica_id_hex.clone();
        cases.push(duplicate_id);
        let mut duplicate_origin = enabled();
        duplicate_origin.replicas[1].origin = duplicate_origin.replicas[0].origin.clone();
        cases.push(duplicate_origin);
        let mut duplicate_key = enabled();
        duplicate_key.replicas[1].public_key = duplicate_key.replicas[0].public_key.clone();
        cases.push(duplicate_key);
        let mut zero_id = enabled();
        zero_id.replicas[0].replica_id_hex = "00".repeat(32);
        cases.push(zero_id);
        let mut wrong_algorithm = enabled();
        wrong_algorithm.replicas[0].public_key =
            KeyPair::from_seed(vec![0x55; 32], Algorithm::Secp256k1)
                .public_key()
                .clone();
        cases.push(wrong_algorithm);
        let mut zero_limit = enabled();
        zero_limit.max_accumulators = 0;
        cases.push(zero_limit);

        for case in cases {
            assert!(case.into_actual().is_err());
        }
    }
}
