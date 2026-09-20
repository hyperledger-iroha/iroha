//! Fixed one/four-lane localnet layouts for the canonical scaling workload.

use super::*;
use toml::{Table, Value};

pub(super) mod anchors;
pub(super) mod runtime_paths;
pub(super) mod seed;

/// Execution-lane counts admitted by the paired scaling experiment.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum ScalingLanes {
    /// One execution lane.
    #[value(name = "1")]
    One,
    /// Four execution lanes sharing the same validator committee.
    #[value(name = "4")]
    Four,
}

/// Validated generator geometry; this is not launch or evidence authority.
#[derive(Clone, Copy)]
pub(super) struct ScalingLayout {
    lanes: ScalingLanes,
    accounts: u16,
}

// At the generated fee policy, the bounded 1,024-request workload costs 7.68
// XOR per account (0.0075 per metadata update). Keep a fixed finite reserve;
// the workload does not receive minting or sponsor permissions.
const ACCOUNT_FEE_BALANCE: u64 = 100;

impl ScalingLayout {
    pub(super) fn from_args(
        lanes: Option<ScalingLanes>,
        accounts: Option<u16>,
    ) -> Result<Option<Self>> {
        let Some(lanes) = lanes else {
            ensure!(
                accounts.is_none(),
                "--scaling-accounts requires --scaling-lanes"
            );
            return Ok(None);
        };
        let accounts = accounts.unwrap_or(4);
        ensure!(
            (4..=64).contains(&accounts) && accounts.is_multiple_of(4),
            "--scaling-accounts must be 4..=64 in complete groups of four"
        );
        Ok(Some(Self { lanes, accounts }))
    }

    pub(super) fn lane_count(self) -> u16 {
        match self.lanes {
            ScalingLanes::One => 1,
            ScalingLanes::Four => 4,
        }
    }

    pub(super) fn validate(self, opts: &LocalnetOptions) -> Result<()> {
        ensure!(
            opts.peers.get() == 4,
            "fixed scaling layouts require exactly four validators"
        );
        ensure!(
            opts.consensus_mode == SumeragiConsensusMode::Npos,
            "fixed scaling layouts require NPoS"
        );
        ensure!(
            opts.sora_profile.is_none() && opts.perf_profile.is_none(),
            "fixed scaling layouts cannot use Sora or performance profiles"
        );
        ensure!(
            opts.extra_accounts == 0 && opts.assets.is_empty(),
            "fixed scaling layouts own the complete workload account and asset setup"
        );
        ensure!(
            opts.seed.as_deref().is_some_and(|seed| !seed.is_empty()),
            "fixed scaling layouts require a nonempty private development seed shared by the paired variants"
        );
        Ok(())
    }

    pub(super) fn identities(self, seed: Option<&[u8]>) -> Result<Vec<LocalnetClientIdentity>> {
        let seed = seed.filter(|seed| !seed.is_empty()).ok_or_else(|| {
            eyre!("fixed scaling account keys require the paired private development seed")
        })?;
        (0..self.accounts)
            .map(|index| {
                // The label never depends on lane count, routing, or public workload seed.
                localnet_ephemeral_identity(
                    Some(seed),
                    format!("scaling-account-{index:02}").as_bytes(),
                )
            })
            .collect()
    }

    fn validate_accounts(self, accounts: &[LocalnetClientIdentity]) -> Result<()> {
        ensure!(
            accounts.len() == usize::from(self.accounts),
            "fixed scaling account count changed"
        );
        let unique = accounts
            .iter()
            .map(|account| &account.account_id)
            .collect::<BTreeSet<_>>();
        ensure!(
            unique.len() == accounts.len(),
            "fixed scaling accounts must be unique"
        );
        Ok(())
    }

    pub(super) fn append_accounts(
        self,
        genesis: RawGenesisTransaction,
        accounts: &[LocalnetClientIdentity],
    ) -> Result<RawGenesisTransaction> {
        self.validate_accounts(accounts)?;
        let existing = BootstrapRegistrations::from_manifest(&genesis);
        let fee_asset = localnet_fee_asset_definition_id();
        ensure!(
            existing.asset_defs.contains(&fee_asset),
            "fixed scaling fee asset must already be registered"
        );
        let registered = genesis
            .instructions()
            .filter_map(|instruction| {
                let RegisterBox::Account(register) =
                    instruction.as_any().downcast_ref::<RegisterBox>()?
                else {
                    return None;
                };
                Some(register.object.id.clone())
            })
            .collect::<BTreeSet<_>>();
        ensure!(
            accounts
                .iter()
                .all(|account| !registered.contains(&account.account_id)),
            "fixed scaling workload accounts must start unregistered"
        );
        // Continue the existing bootstrap transaction so account count does not
        // increase the number of genesis transactions or create another layout.
        let mut builder = genesis.into_builder();
        for account in accounts {
            builder = builder
                .append_instruction(Register::account(Account::new(account.account_id.clone())))
                .append_instruction(Mint::asset_quantity(
                    ACCOUNT_FEE_BALANCE,
                    AssetId::new(fee_asset.clone(), account.account_id.clone()),
                ));
        }
        builder.build_raw()
    }

    pub(super) fn render_config(
        self,
        rendered: Zeroizing<String>,
        accounts: &[LocalnetClientIdentity],
    ) -> Result<Zeroizing<String>> {
        self.validate_accounts(accounts)?;
        let mut root = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
            &rendered,
            "generated scaling peer config",
        )?);
        // Workload accounts are registered and funded in the signed genesis. The fixed
        // benchmark has no onboarding or faucet service, so parsing its retained peer
        // configs must not open either service's private-key sidecar.
        if let Some(torii) = root.get_mut("torii").and_then(Value::as_table_mut) {
            crate::secret_toml::remove(torii, "account_onboarding");
            crate::secret_toml::remove(torii, "faucet");
        }
        // The fixed workload uses the compiled codec defaults. Explicit codec
        // overrides invoke the file-backed rANS parser even when all scalars equal
        // those defaults; omit them so retained config admission opens no side file.
        if let Some(streaming) = root.get_mut("streaming").and_then(Value::as_table_mut) {
            crate::secret_toml::remove(streaming, "codec");
        }
        let nexus = root
            .get_mut("nexus")
            .and_then(Value::as_table_mut)
            .ok_or_else(|| eyre!("generated peer config lacks Nexus settings"))?;
        let lanes = (0..self.lane_count())
            .map(|index| {
                let mut lane = Table::new();
                lane.insert("index".into(), Value::Integer(i64::from(index)));
                lane.insert("alias".into(), Value::String(format!("scaling-{index}")));
                lane.insert(
                    "description".into(),
                    Value::String("Fixed scaling execution lane".to_owned()),
                );
                lane.insert("dataspace".into(), Value::String("universal".to_owned()));
                lane.insert("visibility".into(), Value::String("public".to_owned()));
                lane.insert("metadata".into(), Value::Table(Table::new()));
                Value::Table(lane)
            })
            .collect();
        crate::secret_toml::insert(
            nexus,
            "lane_count".into(),
            Value::Integer(i64::from(self.lane_count())),
        );
        crate::secret_toml::insert(nexus, "lane_catalog".into(), Value::Array(lanes));
        crate::secret_toml::insert(
            nexus,
            "dataspace_catalog".into(),
            Value::Array(localnet_dataspace_catalog(None, 1, false)),
        );
        let mut autoscale = Table::new();
        autoscale.insert("enabled".into(), Value::Boolean(false));
        crate::secret_toml::insert(nexus, "autoscale".into(), Value::Table(autoscale));
        let rules = accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                let mut matcher = Table::new();
                matcher.insert(
                    "account".into(),
                    Value::String(account.account_id.to_string()),
                );
                let mut rule = Table::new();
                rule.insert(
                    "lane".into(),
                    Value::Integer((index % usize::from(self.lane_count())) as i64),
                );
                rule.insert("dataspace".into(), Value::String("universal".to_owned()));
                rule.insert("matcher".into(), Value::Table(matcher));
                Value::Table(rule)
            })
            .collect();
        let mut routing = Table::new();
        routing.insert("default_lane".into(), Value::Integer(0));
        routing.insert(
            "default_dataspace".into(),
            Value::String("universal".to_owned()),
        );
        routing.insert("rules".into(), Value::Array(rules));
        crate::secret_toml::insert(nexus, "routing_policy".into(), Value::Table(routing));
        toml::to_string(&*root)
            .map(Zeroizing::new)
            .wrap_err("render fixed scaling peer config")
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_accounts(
        self,
        out_dir: &Path,
        api_port: u16,
        host: &CanonicalHost,
        chain_id: &str,
        chain_discriminant: u16,
        accounts: &[LocalnetClientIdentity],
    ) -> Result<Vec<[u8; 32]>> {
        self.validate_accounts(accounts)?;
        let mut digests = Vec::with_capacity(accounts.len());
        for (index, account) in accounts.iter().enumerate() {
            digests.push(write_client_config_at(
                &out_dir.join(account_config_name(index)),
                api_port,
                host,
                chain_id,
                Some(chain_discriminant),
                account,
            )?);
        }
        // TODO: the retained four-process launcher must independently authenticate
        // these files and final signed genesis, prove readiness, and own shutdown
        // and original Kura capture. Generation alone cannot qualify G-SCALE.
        Ok(digests)
    }
}

pub(super) fn account_config_name(index: usize) -> String {
    format!("workload-account-{index:02}.toml")
}

/// Fixed-profile public context, retained and independently pinned by the launcher.
pub(super) const GENESIS_CONTEXT_FILE: &str = "genesis-context.nrt";
// The existing owner-only atomic writer/readback path admits at most one MiB.
// This stricter fixed four-validator bound fits the collector's eight-MiB input cap.
const GENESIS_CONTEXT_MAX_BYTES: usize = 1024 * 1024;

/// Encode the actual final signed-genesis authority's context without rebuilding it.
///
/// This is generator output, not a retained launch or runtime completion owner.
/// The later launcher must retain and independently pin all original artifacts.
pub(super) fn genesis_context_bytes(
    manifest_path: &Path,
    signed_path: &Path,
    config: &actual::Root,
) -> Result<Vec<u8>> {
    let manifest = RawGenesisTransaction::from_path(manifest_path)
        .wrap_err("read bounded final scaling genesis manifest")?;
    let signed = iroha_genesis::read_signed_genesis_bytes(signed_path)
        .wrap_err("read bounded final scaling signed genesis")?;
    let authority =
        crate::genesis::staged_signed_genesis_merge_authority(&manifest, &signed, config)
            .wrap_err("authenticate final scaling genesis context")?;
    ensure!(
        authority.context().network_id
            == NetworkId::from_genesis_hash(config.genesis.expected_hash),
        "fixed scaling genesis context has a foreign network"
    );
    encode_genesis_context(&authority)
}

fn encode_genesis_context(
    authority: &iroha_core::sumeragi::GenesisMergeAuthority,
) -> Result<Vec<u8>> {
    let context = authority.context();
    ensure!(
        context.height == 1 && context.roster.len() == 4,
        "fixed scaling genesis context must anchor height one and four validators"
    );
    let count = norito::canonical_frame_len(context)?;
    ensure!(
        count > 0 && count <= GENESIS_CONTEXT_MAX_BYTES,
        "fixed scaling genesis context exceeds its canonical frame bound"
    );
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::to_bytes_bounded(context, GENESIS_CONTEXT_MAX_BYTES)?;
    ensure!(
        bytes.len() == count,
        "fixed scaling genesis context canonical length changed"
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests;
