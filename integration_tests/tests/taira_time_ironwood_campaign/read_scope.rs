//! Resume four stopped release-R test peers and qualify the scoped-read activation and grants.

use std::{
    io::Read as _,
    process::{Child, Command, Stdio},
};

use super::*;
use iroha_config_base::{
    read::ConfigReader,
    toml::{TomlSource, WriteExt as _},
};
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    runtime::{
        definition::RootSelectionV1,
        lifecycle::RuntimeNoticeV1,
        taira_read_scope::{
            KEYED_PREDECESSOR, TAIRA_READ_SCOPE_VERSION_V1, TairaReadScopeCertificateV1,
            TairaReadScopePlanV1,
        },
    },
    sumeragi_finality::{KeyedStateRowsV1, keyed_state::StateKeyV1},
};
use iroha_executor_data_model::permission::query::{CanReadStateScope, StateReadScope};
use iroha_model_base::{chain::ChainId, state_path::StatePath};
use sha2::{Digest as _, Sha256};

struct Seat {
    config: PathBuf,
    chain: ChainId,
    network: NetworkId,
    validator: KeyPair,
    client: Client,
    reader: Client,
}

/// The test owns the lifetime of only the four children it creates here.
struct PeerProcesses(Vec<Child>);
impl Drop for PeerProcesses {
    fn drop(&mut self) {
        for peer in &mut self.0 {
            let _ = peer.kill();
            let _ = peer.wait();
        }
    }
}

fn fixture_path(name: &str) -> Result<PathBuf> {
    Ok(fs::canonicalize(
        std::env::var_os(name).ok_or_else(|| eyre!("set {name}"))?,
    )?)
}

fn seat_config(directory: &Path) -> Result<PathBuf> {
    (1..=16)
        .rev()
        .map(|i| directory.join(format!("run-{i}-config.toml")))
        .find(|p| p.is_file())
        .ok_or_else(|| eyre!("no retained campaign configuration"))
}

impl Seat {
    fn from_directory(dir: &Path) -> Result<Self> {
        let config = seat_config(dir)?;
        let actual = ConfigReader::new()
            .without_env()
            .read_toml_with_extends(&config)
            .map_err(|_| eyre!("cannot read the fixture configuration"))?
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .map_err(|_| eyre!("incomplete fixture configuration"))?
            .parse()
            .map_err(|_| eyre!("invalid fixture configuration"))?;
        let chain = actual.common.chain.clone();
        let network = NetworkId::from_genesis_hash(actual.genesis.expected_hash);
        let listener = actual.torii.address.value();
        ensure!(
            chain.as_str() == "00000000-0000-0000-0000-000000000000"
                && listener
                    .ip()
                    .is_some_and(|ip| std::net::IpAddr::from(ip).is_loopback()),
            "expected R's localhost network"
        );
        let validator = actual.common.key_pair.clone();
        let new_client = |key: &KeyPair| -> Result<Client> {
            let table = toml::Table::new()
                .write("chain", chain.to_string())
                .write("network_id", network.to_string())
                .write("torii_url", format!("http://127.0.0.1:{}", listener.port()))
                .write(
                    ["account", "chain_discriminant"],
                    i64::from(*actual.common.chain_discriminant.value()),
                )
                .write(["account", "public_key"], key.public_key().to_string())
                .write(
                    ["account", "private_key"],
                    ExposedPrivateKey(key.private_key().clone()).to_string(),
                )
                .write(["transaction", "status_timeout_ms"], 120_000i64)
                .write(["transaction", "time_to_live_ms"], 240_000i64)
                .write("torii_request_timeout_ms", 30_000i64);
            let conf = ConfigReader::new()
                .without_env()
                .with_toml_source(TomlSource::inline(table))
                .read_and_complete::<iroha::config::UserConfig>()
                .map_err(|_| eyre!("invalid test-client config"))?
                .parse()
                .map_err(|_| eyre!("invalid test-client identity"))?;
            let mut builder = iroha::client::Client::builder(conf);
            builder.operator_key_pair = Some(validator.clone());
            Ok(Client::from_client(builder.build()?)?)
        };
        let client = new_client(&iroha_test_samples::ALICE_KEYPAIR)?;
        let reader = new_client(&key(1))?;
        Ok(Self {
            config,
            chain,
            network,
            validator,
            client,
            reader,
        })
    }

    fn state(&self) -> Result<Value> {
        self.client
            .client()
            .get_runtime_upgrades_json()
            .wrap_err("signed upgrades read")
    }

    fn record(&self, id: &str) -> Result<Option<ChangeSetRecordV1>> {
        let route = self.state()?;
        route["items"]
            .as_array()
            .ok_or_else(|| eyre!("no change-set inventory"))?
            .iter()
            .find(|row| row["id_hex"].as_str() == Some(id))
            .map(|row| Ok(json::from_json(&json::to_json(&row["record"])?)?))
            .transpose()
    }
}

fn position(path: &str) -> Result<StateKeyV1> {
    Ok(StateKeyV1 {
        table: "world.smart_contract_state".to_owned(),
        key: norito::codec::encode_adaptive(&path.parse::<StatePath>()?),
    })
}

fn state_row(
    rt: &tokio::runtime::Runtime,
    client: &Client,
    height: u64,
    key: &StateKeyV1,
) -> std::result::Result<KeyedStateRowsV1, iroha::Error> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut wait = Duration::from_secs(2);
    loop {
        let result = rt.block_on(
            client
                .account_client()
                .state_rows(height, std::slice::from_ref(key)),
        );
        if matches!(result, Err(iroha::Error::Http { status: 429, .. }))
            && Instant::now() < deadline
        {
            eprintln!(
                "read-scope: Torii rate-limited the row request at {height}; retrying in {wait:?}"
            );
            std::thread::sleep(wait);
            wait = (wait * 2).min(Duration::from_secs(16));
        } else {
            return result;
        }
    }
}

fn require_forbidden(rt: &tokio::runtime::Runtime, seat: &Seat, key: &StateKeyV1) -> Result<()> {
    let h = tip(&seat.client)?;
    let denied = state_row(rt, &seat.reader, h, key);
    ensure!(
        matches!(denied, Err(iroha::Error::Http { status: 403, .. })),
        "expected scoped authorization refusal at height {h}, got {denied:?}"
    );
    Ok(())
}

/// The already authenticated prefix shared by the activation and the granted-reader cuts.
struct EvidencePrefix {
    verified: Option<CertifiedPrefix>,
    height: u64,
    budget: iroha_core::state::AllocationBudget,
}

impl EvidencePrefix {
    /// Authenticate the original against its certified execution root and release-owned keyed schema.
    fn verify_row(
        &mut self,
        seat: &Seat,
        selected: &RootSelectionV1,
        expected: &StateKeyV1,
        rows: &KeyedStateRowsV1,
        directory: &Path,
        label: &str,
    ) -> Result<()> {
        ensure!(
            self.height < rows.height,
            "the evidence cuts must be in ascending order"
        );
        let scope = SumeragiRootScope::Global;
        for h in self.height + 1..=rows.height {
            let proof = seat
                .client
                .client()
                .get_sumeragi_finality_proof(NonZeroU64::new(h).unwrap(), scope)?;
            proof.decode_checked(scope.into())?;
            let signed = iroha_data_model::block::decode_framed_signed_block(&proof.block_wire)?;
            let block = iroha_data_model::block::SharedSignedBlock::try_new(signed, &self.budget)
                .map_err(|(_, e)| e)?;
            if h == 1 {
                self.verified = Some(CertifiedPrefix::new(&seat.chain, seat.network, block)?);
                self.height = h;
                fs::write(directory.join("finality-genesis.nrt"), &proof.block_wire)?;
                continue;
            }
            let step = self.verified.as_mut().unwrap().push(block)?.into_parts().0;
            self.height = h;
            if h == rows.height {
                let schema = iroha_core::runtime::keyed_state::schema_digest(selected)
                    .map_err(|e| eyre!(e))?;
                let committed = step.committed();
                let [row] = rows.rows.as_slice() else {
                    return Err(eyre!("expected one proven row"));
                };
                ensure!(
                    rows.block_hash == committed.block_hash()
                        && rows.schema_hash == schema
                        && row.position == *expected
                        && row.value.is_some()
                        && row.witness.verify(
                            &schema,
                            committed.commitment().execution.world_state_root.payload(),
                            expected,
                            row.value.as_deref()
                        ),
                    "row did not verify under the certified root"
                );
                fs::write(
                    directory.join(format!("{label}-rows.nrt")),
                    norito::encode_canonical(rows)?,
                )?;
                fs::write(
                    directory.join(format!("{label}-finality.nrt")),
                    norito::encode_canonical(&proof)?,
                )?;
                fs::write(
                    directory.join(format!("{label}-binding.json")),
                    json::to_json_pretty(&norito::json!({
                        "height": h,
                        "network_id": (seat.network.to_string()),
                        "chain_id": (seat.chain.to_string()),
                        "selection_id": (selected.id()?.to_string()),
                        "block_hash": (rows.block_hash.to_string()),
                        "state_schema_hex": (hex::encode(schema)),
                        "world_state_root_hex": (hex::encode(committed.commitment().execution.world_state_root.payload())),
                        "table": (expected.table.clone()),
                        "key_hex": (hex::encode(&expected.key)),
                        "result_hash_hex": (hex::encode(committed.result().0)),
                    }))?,
                )?;
            }
        }
        Ok(())
    }
}

fn file_digest(path: &Path) -> Result<[u8; 32]> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 128 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hash.finalize().into())
}

fn signed_schedule(
    archive: &Path,
    seats: &[Seat],
    release_digest: [u8; 32],
    source_closure_digest: [u8; 32],
    change_set: iroha_data_model::runtime::lifecycle::ChangeSetV1,
) -> Result<InstructionBox> {
    use iroha_data_model::isi::taira_read_scope::ScheduleTairaReadScopeV1;
    let roster = canonical_committee(
        seats
            .iter()
            .map(|s| PeerId::new(s.validator.public_key().clone())),
    )?;
    let plan = TairaReadScopePlanV1 {
        version: TAIRA_READ_SCOPE_VERSION_V1,
        chain_id: seats[0].chain.clone(),
        network_id: seats[0].network,
        operation_id: change_set.id()?.0,
        change_set,
        notice: RuntimeNoticeV1 {
            blocks: 1,
            ms: 0,
            approval_cushion_ms: 0,
        },
        committee: roster
            .into_iter()
            .map(|p| p.public_key().clone())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| eyre!("expected four validators"))?,
        expires_at_unix_ms: unix_ms()? + 3_600_000,
        authorization_digest: Sha256::digest(
            b"Isolated four-seat Iroha release-R scoped-read activation fixture only",
        )
        .into(),
        release_digest,
        source_closure_digest,
    };
    let preimage = plan.signing_bytes()?;
    let signatures = plan.committee.each_ref().map(|key| {
        let seat = seats
            .iter()
            .find(|seat| seat.validator.public_key() == key)
            .expect("selected from seats");
        Signature::new(seat.validator.private_key(), &preimage)
    });
    let certificate = TairaReadScopeCertificateV1 { plan, signatures };
    let bytes = certificate.to_bytes()?;
    fs::write(archive, &bytes)?;
    eprintln!(
        "read-scope: signed certificate {} SHA-256 {}",
        certificate.plan.change_set.id()?,
        hex::encode(Sha256::digest(&bytes))
    );
    Ok(ScheduleTairaReadScopeV1 { certificate }.into())
}

fn attest(attesting: &[Seat], first: &Seat, id: &str) -> Result<()> {
    for (i, seat) in attesting.iter().enumerate() {
        wait_until(
            "scheduled on attesting seat",
            Duration::from_secs(90),
            || Ok(seat.record(id).is_ok_and(|r| r.is_some())),
        )?;
        let draft = seat.client.client().post_runtime_attest_upgrade_json(id)?;
        let [instruction] = draft["tx_instructions"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
        else {
            return Err(eyre!("expected one attestation"));
        };
        let wire = instruction["wire_id"]
            .as_str()
            .ok_or_else(|| eyre!("missing wire id"))?;
        let bytes = hex::decode(
            instruction["payload_hex"]
                .as_str()
                .ok_or_else(|| eyre!("missing attestation"))?,
        )?;
        applied(&first.client, decode_instruction_from_pair(wire, &bytes)?)?;
        let record = first
            .record(id)?
            .ok_or_else(|| eyre!("record disappeared"))?;
        ensure!(
            record.attestations.len() == i + 1 && record.status == ChangeSetStatusV1::Scheduled,
            "attestation count or schedule changed"
        );
        eprintln!(
            "read-scope: ordinary support attestation {} by {} at height {}",
            i + 1,
            seat.validator.public_key(),
            tip(&first.client)?
        );
    }
    Ok(())
}

fn advance(seat: &Seat, height: u64) -> Result<()> {
    while tip(&seat.client)? < height {
        applied(
            &seat.client,
            Log::new(Level::INFO, format!("scope-height-{}", unix_ms()?)).into(),
        )?;
    }
    Ok(())
}

fn resume(store: &Path, daemon: &Path) -> Result<(Vec<Seat>, PeerProcesses)> {
    let mut seats = fs::read_dir(store)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().join("config.base.toml").is_file())
        .map(|entry| Seat::from_directory(&entry.path()))
        .collect::<Result<Vec<_>>>()?;
    seats.sort_by(|a, b| a.config.cmp(&b.config));
    ensure!(
        seats.len() == 4 && seats.iter().all(|seat| seat.network == seats[0].network),
        "expected exactly four retained R seats on one fixture genesis"
    );
    let mut peers = PeerProcesses(Vec::new());
    for seat in &seats {
        let dir = seat.config.parent().unwrap();
        let log = fs::File::create(dir.join("read-scope-campaign.log"))?;
        peers.0.push(
            Command::new(daemon)
                .current_dir(dir)
                .arg("--config")
                .arg(&seat.config)
                .stdout(Stdio::from(log.try_clone()?))
                .stderr(Stdio::from(log))
                .spawn()?,
        );
    }
    let first = &seats[0];
    for (seat, process) in seats.iter().zip(&mut peers.0) {
        let mut observed = String::new();
        let mut next_observation = Instant::now() + Duration::from_secs(45);
        wait_until(
            "R's certified keyed selection",
            Duration::from_secs(600),
            || {
                if let Some(status) = process.try_wait()? {
                    return Err(eyre!("fixture daemon exited with {status}"));
                }
                let ready = match seat.state() {
                    Ok(route) => {
                        observed = route["selection_id_hex"]
                            .as_str()
                            .unwrap_or("missing")
                            .to_owned();
                        observed == KEYED_PREDECESSOR.to_string()
                    }
                    Err(error) => {
                        observed = format!("{error:#}");
                        false
                    }
                };
                if !ready && Instant::now() >= next_observation {
                    eprintln!(
                        "read-scope: seat {} awaiting R: {observed}",
                        seat.validator.public_key()
                    );
                    next_observation = Instant::now() + Duration::from_secs(45);
                }
                Ok(ready)
            },
        )
        .wrap_err_with(|| {
            format!(
                "seat {} last runtime response: {observed}",
                seat.validator.public_key()
            )
        })?;
        eprintln!(
            "read-scope: seat {} serves R at height {}",
            seat.validator.public_key(),
            tip(&seat.client)?
        );
    }
    eprintln!(
        "read-scope: all four restored R at height {} on network {}",
        tip(&first.client)?,
        first.network
    );
    Ok((seats, peers))
}

#[test]
#[ignore = "requires four stopped release-R loopback campaign stores and their deployment-identity-pinned fixture daemon"]
fn four_seats_attest_and_activate_before_the_scoped_grant() -> Result<()> {
    iroha_test_network::init_instruction_registry();
    let store = fixture_path("IROHA_READ_SCOPE_R_FIXTURE")?;
    let daemon = fixture_path("IROHA_READ_SCOPE_FIXTURE_DAEMON")?;
    let release_digest = file_digest(&daemon)?;
    let source_closure_digest =
        file_digest(&fixture_path("IROHA_READ_SCOPE_FIXTURE_SOURCE_MANIFEST")?)?;
    eprintln!(
        "read-scope: fixture daemon SHA-256 {}, fixture source-manifest SHA-256 {}",
        hex::encode(release_digest),
        hex::encode(source_closure_digest)
    );
    let (seats, _peers) = resume(&store, &daemon)?;
    let first = &seats[0];
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let allowed = position("runtime/taira_keyed_state/profile_cutover/certificate")?;
    let adjacent = position("runtime/taira_keyed_state/kernel_adoption/certificate")?;
    require_forbidden(&rt, first, &allowed)?;
    let token = CanReadStateScope {
        scope: StateReadScope::ContractStatePrefix(
            "runtime/taira_keyed_state/profile_cutover/".parse()?,
        ),
    };
    refused(
        &first.client,
        Grant::account_permission(token.clone(), account(1)).into(),
        "CanReadStateScope",
    )
    .wrap_err("the genesis read authority cannot issue this new permission before activation")?;
    let predecessor: RootSelectionV1 =
        json::from_json(&json::to_json(&first.state()?["selection"])?)?;
    let lapse_height = tip(&first.client)? + 7;
    let incomplete = iroha_core::runtime::taira_read_scope::draft(
        &predecessor,
        first.network,
        lapse_height,
        [0x63; 32],
    )
    .map_err(|e| eyre!(e))?;
    let lapsed_id = incomplete.id()?.to_string();
    applied(
        &first.client,
        signed_schedule(
            &store.join("lapsed-certificate.nrt"),
            &seats,
            release_digest,
            source_closure_digest,
            incomplete,
        )?,
    )?;
    attest(&seats[..3], first, &lapsed_id)?;
    advance(first, lapse_height)?;
    for seat in &seats {
        wait_until(
            "incomplete support lapses on every seat",
            Duration::from_secs(120),
            || {
                Ok(seat.record(&lapsed_id).is_ok_and(|r| {
                    r.is_some_and(|r| {
                        r.status == ChangeSetStatusV1::Lapsed(lapse_height)
                            && r.attestations.len() == 3
                    })
                }))
            },
        )?;
        ensure!(
            seat.state()?["selection_id_hex"].as_str() == Some(&KEYED_PREDECESSOR.to_string()),
            "three attestations selected the successor"
        );
    }
    require_forbidden(&rt, first, &allowed)?;
    eprintln!(
        "read-scope: all four seats observed {lapsed_id} lapse at {lapse_height} with three attestations; predecessor retained"
    );
    let height = tip(&first.client)? + 12;
    let change = iroha_core::runtime::taira_read_scope::draft(
        &predecessor,
        first.network,
        height,
        [0x64; 32],
    )
    .map_err(|e| eyre!(e))?;
    let id = change.id()?.to_string();
    let schedule = signed_schedule(
        &store.join("activated-certificate.nrt"),
        &seats,
        release_digest,
        source_closure_digest,
        change.clone(),
    )?;
    applied(&first.client, schedule.clone())?;
    ensure!(
        first
            .record(&id)?
            .is_some_and(|r| r.status == ChangeSetStatusV1::Scheduled),
        "scheduling did not record the change"
    );
    eprintln!(
        "read-scope: scheduled {id} at height {}, effective height {height}",
        tip(&first.client)?
    );
    require_forbidden(&rt, first, &allowed)?;
    // A retained schedule cannot be submitted again, even with all four original signatures.
    ensure!(
        first
            .client
            .submit(schedule, fee())
            .and_then(|hash| first
                .client
                .wait_for_transaction_applied(hash, TransactionWaitOptions::default()))
            .is_err(),
        "the spent schedule was accepted"
    );
    attest(&seats, first, &id)?;
    ensure!(
        first.state()?["selection_id_hex"].as_str() == Some(&KEYED_PREDECESSOR.to_string()),
        "four attestations selected too early"
    );
    refused(
        &first.client,
        Grant::account_permission(token.clone(), account(1)).into(),
        "CanReadStateScope",
    )
    .wrap_err("four attestations alone must not enable the new grant")?;
    require_forbidden(&rt, first, &allowed)?;
    eprintln!(
        "read-scope: {} scheduled, four separate node attestations, reads denied before effective height {height}",
        id
    );
    advance(first, height)?;
    for seat in &seats {
        wait_until("activation at every seat", Duration::from_secs(120), || {
            Ok(seat
                .record(&id)
                .is_ok_and(|r| r.is_some_and(|r| r.status == ChangeSetStatusV1::Activated(height))))
        })?;
        ensure!(
            seat.state()?["selection_id_hex"].as_str() == Some(&change.successor.to_string()),
            "seat selected a different successor"
        );
    }
    require_forbidden(&rt, first, &allowed)?;
    let public = store.join("public-read-scope");
    fs::create_dir(&public)?;
    let genesis_wire = fs::read(first.config.parent().unwrap().join("run-1-genesis.nrt"))?;
    let signed_genesis = iroha_data_model::block::decode_framed_signed_block(&genesis_wire)?;
    ensure!(
        NetworkId::from_genesis_hash(signed_genesis.hash()) == first.network
            && signed_genesis.encode_wire()? == genesis_wire,
        "fixture signed-genesis original differs from its configured network"
    );
    fs::write(public.join("signed-genesis.nrt"), genesis_wire)?;
    let selected: RootSelectionV1 = json::from_json(&json::to_json(&first.state()?["selection"])?)?;
    ensure!(
        selected.id()? == change.successor,
        "signed route selected a different runtime"
    );
    let selection_key = StateKeyV1 {
        table: "world.runtime_selection".to_owned(),
        key: Vec::new(),
    };
    let mut prefix = EvidencePrefix {
        verified: None,
        height: 0,
        budget: iroha_core::state::AllocationBudget::new(64 * 1024 * 1024),
    };
    let selection_rows = state_row(&rt, &first.client, height, &selection_key)?;
    prefix.verify_row(
        first,
        &selected,
        &selection_key,
        &selection_rows,
        &public,
        "activation",
    )?;
    ensure!(
        selection_rows.rows[0].value.as_deref()
            == Some(norito::codec::encode_adaptive(&selected).as_slice()),
        "the runtime route's effective selection differs from its certified State original"
    );
    fs::write(
        public.join("effective-selection.json"),
        json::to_json_pretty(&selected)?,
    )?;
    let grant = Grant::account_permission(token.clone(), account(1));
    applied(&first.client, grant.into())?;
    let rows = state_row(&rt, &first.reader, tip(&first.client)?, &allowed)?;
    prefix.verify_row(first, &selected, &allowed, &rows, &public, "granted-reader")?;
    refused(
        &first.reader,
        Grant::account_permission(token.clone(), account(2)).into(),
        "CanReadStateScope",
    )
    .wrap_err("holding a scoped permission must not confer grant authority")?;
    require_forbidden(&rt, first, &adjacent)?;
    applied(
        &first.client,
        Revoke::account_permission(token, account(1)).into(),
    )?;
    require_forbidden(&rt, first, &allowed)?;
    eprintln!(
        "read-scope: all four activated {height}/{}; allowed signed membership proof at {}, neighboring prefix and post-revocation read returned HTTP 403",
        change.successor, rows.height
    );
    Ok(())
}

#[test]
#[ignore = "requires fresh stopped release-R loopback stores, the unpatched successor and the positive campaign's unexpired certificate"]
fn unpatched_successor_refuses_the_same_four_signature_fixture_certificate() -> Result<()> {
    use iroha_data_model::isi::taira_read_scope::ScheduleTairaReadScopeV1;
    iroha_test_network::init_instruction_registry();
    let store = fixture_path("IROHA_READ_SCOPE_R_FIXTURE")?;
    let daemon = fixture_path("IROHA_READ_SCOPE_PRODUCTION_DAEMON")?;
    let bytes = fs::read(fixture_path("IROHA_READ_SCOPE_CERTIFICATE")?)?;
    let certificate = TairaReadScopeCertificateV1::from_bytes(&bytes)?;
    let plan = &certificate.plan;
    let production_digest = file_digest(&daemon)?;
    ensure!(
        plan.release_digest != production_digest,
        "select the separate production binary"
    );
    let (seats, _peers) = resume(&store, &daemon)?;
    let first = &seats[0];
    let current: RootSelectionV1 = json::from_json(&json::to_json(&first.state()?["selection"])?)?;
    let block = tip(&first.client)? + 1;
    let roster = canonical_committee(
        seats
            .iter()
            .map(|s| PeerId::new(s.validator.public_key().clone())),
    )?;
    ensure!(
        plan.chain_id == first.chain
            && plan.network_id == first.network
            && plan
                .committee
                .iter()
                .eq(roster.iter().map(PeerId::public_key))
            && unix_ms()? < plan.expires_at_unix_ms
            && plan
                .change_set
                .effective_height
                .checked_sub(block)
                .is_some_and(|n| n >= plan.notice.blocks)
            && iroha_core::runtime::taira_read_scope::draft(
                &current,
                first.network,
                plan.change_set.effective_height,
                plan.change_set.qualification
            )
            .map_err(|e| eyre!(e))?
                == plan.change_set,
        "certificate must remain valid for these exact original fixture seats and their keyed R selection"
    );
    let id = plan.change_set.id()?.to_string();
    refused(
        &first.client,
        ScheduleTairaReadScopeV1 { certificate }.into(),
        "definition_inactive",
    )?;
    for seat in &seats {
        ensure!(
            seat.record(&id)?.is_none()
                && seat.state()?["selection_id_hex"].as_str()
                    == Some(&KEYED_PREDECESSOR.to_string()),
            "the foreign certificate changed a production seat"
        );
    }
    eprintln!(
        "read-scope: unpatched daemon SHA-256 {} rejected the same four-signature fixture certificate SHA-256 {} at Torii admission; all four seats retained R",
        hex::encode(production_digest),
        hex::encode(Sha256::digest(&bytes))
    );
    Ok(())
}
