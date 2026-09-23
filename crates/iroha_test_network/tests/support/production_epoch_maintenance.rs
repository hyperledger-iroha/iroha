//! Authenticated automatic epoch retention under a genuine four-peer application workload.
//! The shipping observer is read-only; signed canaries provide useful nonempty work.
use super::*;
use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId,
    bridge::BridgeFinalityVerifier,
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
        KagemushaMintFinalityAuthorityGenerationV1 as Authority,
        KagemushaMintFinalityEpochAuthorizationV1 as Authorization,
        KagemushaMintFinalityEpochDecisionV1 as Decision,
    },
};
use iroha_model_base::peer::PeerId;
use std::{collections::BTreeMap, num::NonZeroU64};

#[cfg(target_os = "linux")]
#[path = "production_epoch_supervisor.rs"]
mod supervisor;

pub(super) enum Driver {
    Finite,
    #[cfg(target_os = "linux")]
    Supervised,
}

impl Driver {
    // Pure admission runs before fixture paths, credentials, genesis or children
    // exist. Do not substitute the development label into a supervisor policy.
    pub(super) fn admit_build_identity(
        &self,
        identity: iroha_core::release_identity::BuildIdentity,
    ) -> Result<iroha_core::release_identity::BuildIdentity> {
        match self {
            Self::Finite => Ok(identity),
            #[cfg(target_os = "linux")]
            Self::Supervised => {
                identity.release_source_commit().wrap_err(
                    "supervised beacon fixture requires an exact compiled Git source commit before setup; use maintained Taira checks, not --stable-local-metadata",
                )?;
                Ok(identity)
            }
        }
    }
}

const EPOCH_LENGTH: u64 = 11;
const OBSERVED_EPOCHS: u64 = 8;
// Existing application operations, one signed pulse canary and retained replay/
// snapshot phases keep their original deadlines. This only bounds the observer.
const MONITOR_PHASES: u32 = 29;

fn signed_genesis_authority(
    directory: &Path,
    network: NetworkId,
    public_key: &PublicKey,
) -> Result<Authority> {
    let (hash, metadata) = iroha_core::release_identity::genesis_identity(
        &fs::read(directory.join("genesis.signed.nrt"))?,
        public_key,
    )?;
    ensure!(
        hash == iroha_crypto::Hash::from(network.into_genesis_hash()),
        "epoch fixture genesis has another network identity"
    );
    Ok(metadata
        .kagemusha_mint_finality
        .authority_generation
        .bind_network_id(network)?)
}

fn verify_retained_authorization(
    previous: &Authorization,
    successor: &Authorization,
    authority: &Authority,
    genesis: &Authority,
    installed: BeaconEpochBindingV1,
) -> Result<()> {
    previous.validate_against_authority(genesis)?;
    successor.validate_against_authority(authority)?;
    successor.validate_successor(previous)?;
    let first = successor
        .epoch
        .checked_mul(EPOCH_LENGTH)
        .and_then(|height| height.checked_add(1))
        .ok_or_else(|| eyre!("retained epoch first height overflow"))?;
    let last = first
        .checked_add(EPOCH_LENGTH - 1)
        .ok_or_else(|| eyre!("retained epoch last height overflow"))?;
    ensure!(
        authority == genesis
            && genesis.generation == 0
            && genesis.validators.len() == 4
            && successor.decision == Decision::Retain
            && successor.transition_id == [0; 32]
            && successor.first_height == first
            && successor.last_height == last
            && matches!(installed, BeaconEpochBindingV1::Installed(_))
            && successor.beacon == installed,
        "retention changed the exact signed authority, interval, decision or installed beacon"
    );
    Ok(())
}

fn verify_receipt_continuity(previous: &Value, next: &Value) -> Result<()> {
    let epoch = field(previous, "completed_epoch")?
        .as_u64()
        .ok_or_else(|| eyre!("retained receipt epoch is not an integer"))?;
    let prior: Vec<Authorization> =
        json::from_value(field(previous, "authorization_chain")?.clone())?;
    let chain: Vec<Authorization> = json::from_value(field(next, "authorization_chain")?.clone())?;
    ensure!(
        field(next, "completed_epoch")?.as_u64() == epoch.checked_add(1)
            && field(next, "previous_cursor_id")? == field(previous, "cursor_id")?
            && field(next, "network_id")? == field(previous, "network_id")?
            && field(next, "authority")? == field(previous, "authority")?
            && field(next, "authority_generation")? == field(previous, "authority_generation")?
            && field(next, "authority_id")? == field(previous, "authority_id")?
            && field(next, "beacon_binding")? == field(previous, "beacon_binding")?
            && chain.len() == prior.len() + 1
            && chain[..prior.len()] == prior,
        "retention receipt replay, gap, authority substitution or prefix rewrite"
    );
    let preceding = prior
        .last()
        .ok_or_else(|| eyre!("retained receipt omitted authorization chain"))?;
    chain.last().unwrap().validate_successor(preceding)?;
    Ok(())
}

pub(super) struct Maintenance {
    binary: PathBuf,
    directory: PathBuf,
    trust: PathBuf,
    journal: PathBuf,
    network: NetworkId,
    child: Child,
    stopped: bool,
    #[cfg(target_os = "linux")]
    supervisor: Option<supervisor::Supervisor>,
}
impl Maintenance {
    pub(super) fn start(
        binary: &Path,
        prepared: &prepare::Prepared,
        trust: PathBuf,
    ) -> Result<Self> {
        let directory = prepared.directory.clone();
        let journal = directory.join("epoch-maintenance");
        fs::create_dir(&journal)?;
        fs::set_permissions(&journal, fs::Permissions::from_mode(0o700))?;
        let mut child = Self::base_command(binary, &directory);
        child
            .args(["maintain", "--trust"])
            .arg(&trust)
            .arg("--journal-dir")
            .arg(&journal)
            .arg("--stop-after-epoch")
            .arg(OBSERVED_EPOCHS.to_string())
            .arg("--timeout-ms")
            .arg((PHASE_BUDGET * MONITOR_PHASES).as_millis().to_string())
            .stdout(private_file(&journal.join("stdout.log"), &[])?)
            .stderr(private_file(&journal.join("stderr.log"), &[])?);
        Ok(Self {
            binary: binary.into(),
            directory,
            trust,
            journal,
            network: prepared.network_id,
            child: child.spawn()?,
            stopped: false,
            #[cfg(target_os = "linux")]
            supervisor: None,
        })
    }
    #[cfg(target_os = "linux")]
    pub(super) fn start_supervisor(
        binary: &Path,
        prepared: &prepare::Prepared,
        trust: PathBuf,
        build_identity: iroha_core::release_identity::BuildIdentity,
    ) -> Result<Self> {
        supervisor::start(binary, prepared, trust, build_identity)
    }
    fn base_command(binary: &Path, directory: &Path) -> Command {
        let mut child = command(binary, directory);
        child
            .arg("--machine")
            .arg("--config")
            .arg(directory.join("client.toml"))
            .arg("--operator-private-key-file")
            .arg(directory.join("runtime/operator-signer.key"))
            .args(["taira", "epoch-maintenance"]);
        child
    }
    fn retention(&self) -> PathBuf {
        self.journal.join(format!("retention-{}", self.network))
    }
    fn completion(&self, epoch: u64) -> PathBuf {
        self.retention().join(format!("epoch-{epoch}.json"))
    }
    fn retained_epoch_ready(&self, epoch: u64) -> Result<bool> {
        if !self.completion(epoch).try_exists()? {
            return Ok(false);
        }
        let cursor = self.retention().join("cursor.json");
        if !cursor.try_exists()? {
            return Ok(false);
        }
        let cursor: Value = json::from_slice(&fs::read(cursor)?)?;
        ensure!(
            field(&cursor, "schema_version")?.as_u64() == Some(1)
                && json::from_value::<NetworkId>(field(&cursor, "network_id")?.clone())?
                    == self.network,
            "native retention cursor changed its schema or network"
        );
        Ok(field(&cursor, "completed_epoch")?
            .as_u64()
            .is_some_and(|completed| completed >= epoch))
    }
    async fn status(&self, epoch: u64, deadline: Instant) -> Result<Value> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "retention audit exceeded its original deadline"
        );
        let retained = fs::read(self.completion(epoch))?;
        let mut child = Self::base_command(&self.binary, &self.directory);
        child
            .args(["status", "--trust"])
            .arg(&self.trust)
            .arg("--journal-dir")
            .arg(&self.journal)
            .arg("--stop-after-epoch")
            .arg(epoch.to_string())
            .arg("--timeout-ms")
            .arg(remaining.as_millis().to_string());
        let receipt: Value = json::from_slice(&run(child, deadline).await?)?;
        ensure!(
            field(&receipt, "schema_version")?.as_u64() == Some(1)
                && field(&receipt, "completed_epoch")?.as_u64() == Some(epoch)
                && field(&receipt, "observed_height")?.as_u64() == Some(epoch * EPOCH_LENGTH + 1)
                && json::from_value::<NetworkId>(field(&receipt, "network_id")?.clone())?
                    == self.network,
            "native retention receipt changed its authenticated network, epoch or first height"
        );
        ensure!(
            fs::read(self.completion(epoch))? == retained
                && receipt == json::from_slice::<Value>(&retained)?,
            "read-only verification changed the retained epoch receipt"
        );
        Ok(receipt)
    }
    /// Supply one real signed canary and retain its exact nonempty pulse carrier.
    pub(super) async fn first_progress(
        &mut self,
        clients: &[iroha::client::Client],
        deadline: Instant,
    ) -> Result<HashOf<TransactionEntrypoint>> {
        ensure!(clients.len() == 4, "retention canary needs four validators");
        let before = status_height(clients, deadline).await?;
        ensure!(
            before < EPOCH_LENGTH - 1,
            "the signed-genesis pulse height has already passed before its canary"
        );
        let client = clients[0].with_request_deadline(deadline.into_std());
        let account = client.account_client()?;
        let mut payload = account.prepare_transaction(
            AccountTransactionDraft::new(
                vec![
                    InstructionBox::from(iroha_data_model::isi::Log::new(
                        iroha_data_model::Level::INFO,
                        "verify automatic retained-authority epoch progression".to_owned(),
                    )),
                ],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            )
            .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced),
        )?;
        let quote = account
            .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
            .await?;
        ensure!(
            payload
                .fee_payment
                .has_same_payer_and_gas_bound(&quote.intent),
            "retention canary quote changed payer or gas bound"
        );
        payload.fee_payment = quote.intent;
        let transaction = account.sign_transaction(payload)?;
        transaction.verify_signature()?;
        ensure!(
            transaction.network_id() == Some(&self.network),
            "canary belongs to another network"
        );
        let wire = transaction.encode_wire_v1()?;
        let wire_path = self.journal.join("pulse-canary.signed.nrt");
        private_file(&wire_path, &wire)?;
        File::open(&self.journal)?.sync_all()?;
        let expected = transaction.hash();
        ensure!(
            timeout_at(deadline, account.submit_transaction_and_wait(&transaction)).await??
                == expected,
            "canary submission changed its retained transaction hash"
        );
        let expected_hex = hex(expected.as_ref());
        let height = timeout_at(deadline, async {
            loop {
                let observations = try_join_all(clients.iter().map(|observer| async {
                    let observer = observer.with_request_deadline(deadline.into_std());
                    let global = observer
                        .fetch_transaction_status_response_global(expected)
                        .await?;
                    let blocks = validator_status_until(&observer, deadline).await?.blocks;
                    let local = iroha_test_network::read_on_dedicated_thread(move || {
                        observer.get_transaction_status_response_local(expected)
                    })
                    .await?;
                    Ok::<_, eyre::Report>((blocks, global, local))
                }))
                .await?;
                let applied = observations.iter().all(|(blocks, global, local)| {
                    [("global", global), ("local", local)]
                        .iter()
                        .all(|(scope, response)| {
                            response.as_ref().is_some_and(|response| {
                                response.hash == expected_hex
                                    && response.scope == *scope
                                    && response.resolved_from == "state"
                                    && response.status.kind == "Applied"
                                    && response
                                        .status
                                        .block_height
                                        .is_some_and(|height| height > before && *blocks >= height)
                            })
                        })
                });
                if applied {
                    let height = observations[0]
                        .1
                        .as_ref()
                        .unwrap()
                        .status
                        .block_height
                        .unwrap();
                    ensure!(
                        observations.iter().all(|(_, global, local)| global
                            .as_ref()
                            .unwrap()
                            .status
                            .block_height
                            == Some(height)
                            && local.as_ref().unwrap().status.block_height == Some(height)),
                        "four validators disagree on retained canary execution"
                    );
                    return Ok::<_, eyre::Report>(height);
                }
                ensure!(
                    self.child.try_wait()?.is_none(),
                    "retention observer exited during canary execution"
                );
                sleep(Duration::from_millis(200)).await;
            }
        })
        .await
        .wrap_err("canary exceeded its original execution deadline")??;
        ensure!(
            fs::read(&wire_path)? == wire,
            "retained canary changed after its sole dispatch"
        );
        private_file(
            &self.journal.join("pulse-canary-applied.json"),
            &json::to_vec(&norito::json!({
                "schema_version":1,"network_id":(self.network),"transaction_hash":expected_hex,
                "applied_height":height,"signed_wire_sha256":(hex(&iroha_crypto::sha256(&wire))),
                "four_peer_local_and_global_applied":true
            }))?,
        )?;
        ensure!(
            height == EPOCH_LENGTH - 1,
            "the exact canary did not carry the signed-genesis mandatory pulse"
        );
        Ok(transaction.hash_as_entrypoint())
    }
    /// Observe the first activated Retain after the application crosses its boundary.
    pub(super) async fn first_retention(&mut self, deadline: Instant) -> Result<()> {
        timeout_at(deadline, async {
            while !self.retained_epoch_ready(1)? {
                ensure!(
                    self.child.try_wait()?.is_none(),
                    "observer exited before retained epoch one"
                );
                sleep(Duration::from_millis(200)).await;
            }
            self.status(1, deadline).await?;
            Ok::<_, eyre::Report>(())
        })
        .await
        .wrap_err("first actual Retain exceeded the original phase deadline")??;
        #[cfg(target_os = "linux")]
        if self.supervisor.is_some() {
            supervisor::restart(self, deadline).await?;
        }
        Ok(())
    }
    /// Catch up to the fixed application tip before terminating the owned observer.
    pub(super) async fn await_current_retention(
        &mut self,
        clients: &[iroha::client::Client],
        deadline: Instant,
    ) -> Result<()> {
        let height = status_height(clients, deadline).await?;
        let epoch = height.saturating_sub(1) / EPOCH_LENGTH;
        ensure!(
            (2..OBSERVED_EPOCHS).contains(&epoch),
            "application must reach two retained epochs within the observer bound"
        );
        timeout_at(deadline, async {
            while !self.retained_epoch_ready(epoch)? {
                ensure!(
                    self.child.try_wait()?.is_none(),
                    "observer exited before the application tip"
                );
                sleep(Duration::from_millis(200)).await;
            }
            self.status(epoch, deadline).await?;
            ensure!(
                status_height(clients, deadline).await? == height,
                "read-only retention catch-up generated unexpected ledger work"
            );
            Ok::<_, eyre::Report>(())
        })
        .await
        .wrap_err("retention observer did not reach the fixed application tip")?
    }

    pub(super) async fn stop(&mut self, deadline: Instant) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        if let Some(status) = self.child.try_wait()? {
            ensure!(
                status.success(),
                "native retention observer failed; inspect retained stderr"
            );
        } else {
            self.child.start_kill()?;
            timeout_at(deadline, self.child.wait())
                .await
                .wrap_err("owned observer failed to stop")??;
        }
        self.stopped = true;
        Ok(())
    }
    pub(super) async fn verify(
        &self,
        prepared: &prepare::Prepared,
        clients: &[iroha::client::Client],
        deadline: Instant,
    ) -> Result<()> {
        ensure!(self.stopped, "stop observer before final read-only audit");
        let height = status_height(clients, deadline).await?;
        let completed = height.saturating_sub(1) / EPOCH_LENGTH;
        ensure!(
            (2..OBSERVED_EPOCHS).contains(&completed),
            "qualification must cross two real epochs within its explicit observation bound"
        );
        let mut receipts = Vec::new();
        for epoch in 1..=completed {
            receipts.push(self.status(epoch, deadline).await?);
        }
        for pair in receipts.windows(2) {
            verify_receipt_continuity(&pair[0], &pair[1])?;
        }
        private_file(
            &self.journal.join("fixture-verified-completions.json"),
            &json::to_vec(&receipts)?,
        )?;
        #[cfg(target_os = "linux")]
        if let Some(supervisor) = &self.supervisor {
            supervisor::verify(self, supervisor, height)?;
        }
        verify_boundary_chain(prepared, clients, height, deadline).await
    }
}

async fn verify_boundary_chain(
    prepared: &prepare::Prepared,
    clients: &[iroha::client::Client],
    height: u64,
    deadline: Instant,
) -> Result<()> {
    let genesis = iroha_genesis::decode_signed_genesis(&fs::read(
        prepared.genesis_directory.join("genesis.signed.nrt"),
    )?)?;
    let authority = signed_genesis_authority(
        &prepared.genesis_directory,
        prepared.network_id,
        &prepared.genesis_public_key,
    )?;
    let pops = iroha_genesis::signed_genesis_validator_pops(&genesis)?;
    let network = prepared.network_id;
    let genesis_hash = genesis.hash();
    let expected_roster = pops
        .into_iter()
        .map(|(key, pop)| (PeerId::new(key), pop))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        expected_roster.len() == 4,
        "genesis proof authority is not four peers"
    );
    let (roster, pops): (Vec<_>, Vec<_>) = expected_roster
        .into_iter()
        .map(|(validator, pop)| {
            (
                iroha_data_model::block::consensus_v2::ValidatorPower {
                    validator,
                    power: 1,
                },
                pop,
            )
        })
        .unzip();
    let bundle: Value = json::from_slice(&fs::read(
        prepared
            .directory
            .join("beacon-ceremony/public-bundle.json"),
    )?)?;
    let record: beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1 =
        json::from_value(field(&bundle, "record")?.clone())?;
    record.validate()?;
    ensure!(
        record.session.network_id == network,
        "installed ceremony belongs to another network"
    );
    let installed = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
        session_id: record.session.session_id,
        transcript_hash: record.session.transcript_hash,
    });
    let tips = try_join_all(clients.iter().enumerate().map(|(index, client)| {
        let client = client.clone(); let roster = roster.clone(); let pops = pops.clone();
        let authority = authority.clone();
        let path = prepared.directory.join(format!("epoch-boundary-proof-peer{index}.json"));
        iroha_test_network::read_on_dedicated_thread(move || {
            let bounded = || -> Result<iroha::client::Client> {
                let remaining = deadline.saturating_duration_since(Instant::now());
                ensure!(!remaining.is_zero(), "epoch proof chain exceeded original audit deadline");
                let mut builder = client.to_builder();
                builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
                Ok(builder.build()?)
            };
            let (first, hash) = bounded()?.get_bridge_finality_anchor(NonZeroU64::new(1).unwrap(), network)?;
            let first_context = &first.finality_artifact.height_context;
            let mut previous = first_context.kagemusha_mint_finality_authorization;
            ensure!(hash == genesis_hash && first.block_header.hash() == genesis_hash
                && first_context.roster == roster && first.finality_artifact.validator_set_pops == pops
                && first_context.kagemusha_mint_finality_authority == authority
                && previous.decision == Decision::Genesis && previous.epoch == 0
                && previous.first_height == 1 && previous.last_height == EPOCH_LENGTH,
                "proof chain is not anchored to the exact signed genesis authority");
            previous.validate_against_authority(&authority)?;
            let mut verifier = BridgeFinalityVerifier::with_context(network, first.finality_artifact.context_id());
            verifier.verify(&first)?;
            let mut proofs = vec![first];
            for next in 2..=height {
                let proof = bounded()?.get_next_bridge_finality_proof(NonZeroU64::new(next).unwrap(), &mut verifier)?;
                let context = &proof.finality_artifact.height_context;
                let current = context.kagemusha_mint_finality_authorization;
                ensure!(context.kagemusha_mint_finality_authority == authority
                    && context.roster == roster && proof.finality_artifact.validator_set_pops == pops
                    && current.epoch == (next - 1) / EPOCH_LENGTH,
                    "retained current context changed paired keys, voters, proofs or scheduling epoch");
                if current.epoch == previous.epoch {
                    ensure!(current == previous, "authorization changed within its certified epoch");
                } else {
                    verify_retained_authorization(&previous, &current,
                        &context.kagemusha_mint_finality_authority, &authority, installed)?;
                }
                if next % EPOCH_LENGTH == 0 {
                    let transition = context.next_epoch_snapshot.as_ref()
                        .ok_or_else(|| eyre!("authenticated epoch boundary omitted next authority"))?;
                    ensure!(transition.epoch == next / EPOCH_LENGTH
                        && transition.roster == roster && transition.validator_set_pops == pops,
                        "retained boundary changed its exact consensus voters or proofs");
                    verify_retained_authorization(&current, &transition.kagemusha_mint_finality_authorization,
                        &transition.kagemusha_mint_finality_authority, &authority, installed)?;
                } else {
                    ensure!(context.next_epoch_snapshot.is_none(), "nonboundary context has a successor snapshot");
                }
                previous = current;
                proofs.push(proof);
            }
            let tip = proofs.last().unwrap().block_header.hash();
            private_file(&path, &json::to_vec(&proofs)?)?;
            Ok(tip)
        })
    })).await?;
    ensure!(
        tips.len() == 4 && tips.iter().all(|tip| *tip == tips[0]),
        "epoch proof tips differ across validators"
    );
    Ok(())
}
#[test]
fn production_epoch_driver_admits_required_build_identity_before_setup() -> Result<()> {
    use iroha_core::release_identity::{BuildIdentity, BuildIdentityError};

    let development = BuildIdentity::from_compiled_parts(
        "fixture-test",
        Some("local-fast-build"),
        None,
        None,
        None,
        None,
    )?;
    assert_eq!(
        Driver::Finite.admit_build_identity(development)?,
        development
    );

    // This known public source revision tests admission syntax only. It is never
    // selected as executable metadata or claimed as authenticated provenance.
    let source = BuildIdentity::from_compiled_parts(
        "fixture-test",
        Some("592c6e0e5adcd2ff5e0492d971bfbb179f591b53"),
        None,
        None,
        None,
        None,
    )?;
    assert_eq!(Driver::Finite.admit_build_identity(source)?, source);
    #[cfg(target_os = "linux")]
    {
        let error = Driver::Supervised
            .admit_build_identity(development)
            .expect_err("supervised custody must reject a development identity before setup");
        assert_eq!(
            error.downcast_ref::<BuildIdentityError>(),
            Some(&BuildIdentityError::DevelopmentSource)
        );
        assert!(error.to_string().contains("before setup"));
        assert_eq!(Driver::Supervised.admit_build_identity(source)?, source);
    }
    #[cfg(not(target_os = "linux"))]
    assert_eq!(
        development.release_source_commit(),
        Err(BuildIdentityError::DevelopmentSource)
    );
    Ok(())
}

#[test]
fn retained_authorization_requires_exact_keys_beacon_interval_and_predecessor() -> Result<()> {
    use iroha_crypto::{Algorithm, Hash};
    use iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1;
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"retained-epoch-fixture",
    )));
    let mut roster = (1..=4u8)
        .map(|marker| {
            PeerId::new(
                KeyPair::from_seed(vec![marker; 32], Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            )
        })
        .collect::<Vec<_>>();
    roster.sort();
    let authority = Authority {
        version: KAGEMUSHA_CHAIN_VERSION_V1, network_id: network, generation: 0,
        validators: roster.iter().enumerate().map(|(index, peer)| {
            iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[index as u8 + 1; 32], 0, peer.clone()).map_err(|error| eyre!("fixture keys: {error:?}"))
        }).collect::<Result<Vec<_>>>()?,
    };
    let genesis = Authorization {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: network,
        epoch: 0,
        first_height: 1,
        last_height: EPOCH_LENGTH,
        authority_generation: 0,
        authority_id: authority.authority_id()?,
        beacon: BeaconEpochBindingV1::Bootstrap,
        previous_authorization_id: [0; 32],
        transition_id: [0; 32],
        decision: Decision::Genesis,
    };
    let installed = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
        session_id: [7; 32],
        transcript_hash: [8; 32],
    });
    let first = Authorization {
        epoch: 1,
        first_height: 12,
        last_height: 22,
        beacon: installed,
        previous_authorization_id: genesis.authorization_id()?,
        decision: Decision::Retain,
        ..genesis
    };
    verify_retained_authorization(&genesis, &first, &authority, &authority, installed)?;
    let second = Authorization {
        epoch: 2,
        first_height: 23,
        last_height: 33,
        previous_authorization_id: first.authorization_id()?,
        ..first
    };
    verify_retained_authorization(&first, &second, &authority, &authority, installed)?;
    // Public receipt linkage is independently checked after the native CLI has
    // authenticated both proof prefixes. No stored cursor grants authority.
    let receipt = |authorization: Authorization,
                   chain: Vec<Authorization>,
                   previous: Option<&str>,
                   cursor: &str| {
        norito::json!({
            "network_id":network,"completed_epoch":(authorization.epoch),
            "authority":authority,"authority_generation":0,"authority_id":(authority.authority_id().unwrap()),
            "beacon_binding":installed,"authorization_chain":chain,
            "previous_cursor_id":previous,"cursor_id":cursor
        })
    };
    let one = receipt(first, vec![genesis, first], None, "first");
    let two = receipt(
        second,
        vec![genesis, first, second],
        Some("first"),
        "second",
    );
    verify_receipt_continuity(&one, &two)?;
    assert!(
        verify_receipt_continuity(&one, &one).is_err(),
        "replayed receipt"
    );
    for (key, replacement) in [
        ("previous_cursor_id", Value::from("foreign")),
        ("completed_epoch", Value::from(3_u64)),
        ("authority_id", json::to_value(&[99_u8; 32])?),
        ("authority_generation", Value::from(1_u64)),
        (
            "authorization_chain",
            json::to_value(&vec![genesis, second])?,
        ),
        (
            "beacon_binding",
            json::to_value(&BeaconEpochBindingV1::Bootstrap)?,
        ),
    ] {
        let mut bad = two.clone();
        *bad.get_mut(key).unwrap() = replacement;
        assert!(
            verify_receipt_continuity(&one, &bad).is_err(),
            "receipt field {key}"
        );
    }
    for case in 0..9 {
        let mut bad = second;
        match case {
            0 => bad.epoch += 1,
            1 => bad.first_height += 1,
            2 => bad.last_height += 1,
            3 => bad.previous_authorization_id = genesis.authorization_id()?,
            4 => bad.authority_generation += 1,
            5 => bad.authority_id[0] ^= 1,
            6 => {
                bad.beacon = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                    session_id: [9; 32],
                    transcript_hash: [8; 32],
                })
            }
            7 => {
                bad.decision = Decision::RetainAndCancel;
                bad.transition_id = [1; 32];
            }
            _ => {
                bad.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"foreign-network"),
                ))
            }
        }
        assert!(
            verify_retained_authorization(&first, &bad, &authority, &authority, installed).is_err(),
            "case {case}"
        );
    }
    let mut replaced = authority.clone();
    replaced.validators[0] =
        iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
            &[99; 32],
            0,
            roster[0].clone(),
        )
        .map_err(|error| eyre!("substituted keys: {error:?}"))?;
    let resigned = Authorization {
        authority_id: replaced.authority_id()?,
        ..second
    };
    assert!(
        verify_retained_authorization(&first, &resigned, &replaced, &authority, installed).is_err()
    );
    let mut short = authority.clone();
    short.validators.pop();
    assert!(verify_retained_authorization(&first, &second, &short, &authority, installed).is_err());
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_epoch_supervisor_retains_and_resumes_after_owned_restart() -> Result<()> {
    super::run_fresh_custody_bootstrap(Driver::Supervised).await
}
