//! Deployment completion under an independently selected genesis and four validator identities.
use super::*;
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus_v2::{ConsensusMode, ValidatorPower},
    },
    bridge::{BridgeFinalityAttestationV1, BridgeFinalityProof, BridgeFinalityVerifier},
    sns::NameStatus,
};
use iroha_model_base::peer::PeerId;
use norito::codec::Encode as _;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};

const MAX_NEW_PROOFS: usize = 128;

/// Public target profile selected independently of the server's proof responses.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TrustV1 {
    pub(crate) genesis_public_key: PublicKey,
    pub(crate) genesis_signed_wire_hex: String,
    pub(crate) peers: Vec<PeerV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct PeerV1 {
    pub(crate) torii_origin: String,
    pub(crate) peer_id: PeerId,
    pub(crate) node_fingerprint: Hash,
    pub(crate) build_fingerprint: Hash,
    pub(crate) config_fingerprint: Hash,
}

struct Authority {
    network: NetworkId,
    genesis: HashOf<BlockHeader>,
    roster: Vec<ValidatorPower>,
    pops: Vec<Vec<u8>>,
}

impl TrustV1 {
    pub(super) fn validate(&self, network: NetworkId) -> Result<()> {
        self.authority(network).map(|_| ())
    }

    fn authority(&self, network: NetworkId) -> Result<Authority> {
        require(
            self.genesis_signed_wire_hex.len() <= MAX_BYTES,
            "public genesis exceeds the deployment profile bound",
        )?;
        let wire = hex::decode(&self.genesis_signed_wire_hex)?;
        require(
            hex::encode(&wire) == self.genesis_signed_wire_hex,
            "public genesis wire must use canonical lowercase hexadecimal",
        )?;
        let (hash, metadata) =
            iroha_core::release_identity::genesis_identity(&wire, &self.genesis_public_key)?;
        let genesis = HashOf::<BlockHeader>::from_untyped_unchecked(hash);
        require(
            NetworkId::from_genesis_hash(genesis) == network
                && metadata.mode
                    == iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
            "public signed genesis differs from the independently selected Taira network",
        )?;
        let block = iroha_genesis::decode_signed_genesis(&wire)?;
        let validators = iroha_genesis::signed_genesis_validator_pops(&block)?;
        require(
            validators.len() == 4 && self.peers.len() == 4,
            "Taira deployment verification requires exactly four genesis validators",
        )?;
        let validators: BTreeMap<_, _> = validators
            .into_iter()
            .map(|(key, pop)| (PeerId::new(key), pop))
            .collect();
        let mut peers = BTreeSet::new();
        let mut origins = BTreeSet::new();
        for peer in &self.peers {
            let origin: url::Url = peer.torii_origin.parse()?;
            require(
                matches!(origin.scheme(), "http" | "https")
                    && origin.host_str().is_some()
                    && origin.username().is_empty()
                    && origin.password().is_none()
                    && origin.query().is_none()
                    && origin.fragment().is_none()
                    && origin.as_str() == peer.torii_origin,
                "validator endpoint must be a canonical credential-free Torii URL",
            )?;
            require(
                origins.insert(origin.as_str().to_owned())
                    && peers.insert(peer.peer_id.clone())
                    && validators.contains_key(&peer.peer_id)
                    && Hash::new(peer.peer_id.encode()) == peer.node_fingerprint,
                "validator profile must bind four distinct genesis peers and endpoints",
            )?;
        }
        let (roster, pops) = validators
            .into_iter()
            .map(|(validator, pop)| {
                (
                    ValidatorPower {
                        validator,
                        power: 1,
                    },
                    pop,
                )
            })
            .unzip();
        Ok(Authority {
            network,
            genesis,
            roster,
            pops,
        })
    }
}

impl Authority {
    fn roster(&self, proof: &BridgeFinalityProof) -> Result<()> {
        let artifact = &proof.finality_artifact;
        require(
            artifact.height_context.network_id == self.network
                && artifact.height_context.mode == ConsensusMode::Npos
                && artifact.height_context.roster == self.roster
                && artifact.validator_set_pops == self.pops
                && artifact.height_context.snapshot_bootstrap.is_none()
                && artifact.commit_qc.signers.len() == 3,
            "proof differs from the independently authenticated four-validator genesis roster",
        )
    }

    fn anchor(&self, proof: &BridgeFinalityProof) -> Result<BridgeFinalityVerifier> {
        require(
            proof.block_header.height().get() == 1
                && proof.block_header.hash() == self.genesis
                && proof.finality_artifact.block_hash == self.genesis,
            "finality anchor differs from the selected signed genesis",
        )?;
        self.roster(proof)?;
        let mut verifier = BridgeFinalityVerifier::with_context(
            self.network,
            proof.finality_artifact.context_id(),
        );
        verifier.verify(proof)?;
        Ok(verifier)
    }
}

fn validate_attestation(
    authority: &Authority,
    peer: &PeerV1,
    challenge: [u8; 32],
    attestation: &BridgeFinalityAttestationV1,
) -> Result<()> {
    attestation.verify()?;
    let body = &attestation.body;
    require(
        body.challenge == challenge
            && body.node_id == peer.peer_id
            && body.node_fingerprint == peer.node_fingerprint
            && body.status.build_fingerprint == peer.build_fingerprint
            && body.status.config_fingerprint == peer.config_fingerprint
            && body.network_id == authority.network
            && body.genesis_block_hash == authority.genesis,
        "attested node, build, configuration, challenge or genesis differs from the target profile",
    )?;
    authority.anchor(&body.genesis_finality_proof)?;
    authority.roster(&body.finality_proof)
}

/// Prove the selected public verification routes and peer identities before any deployment write.
pub(super) fn preflight<C: RunContext>(context: &C, manifest: &ManifestV1) -> Result<()> {
    let authority = manifest.finality.authority(manifest.network_id)?;
    let challenge: [u8; 32] = rand::random();
    require(challenge != [0; 32], "random finality challenge is zero")?;
    for peer in &manifest.finality.peers {
        let mut config = context.config().clone();
        config.torii_api_url = peer.torii_origin.parse()?;
        let mut builder = Client::builder(config);
        builder.operator_key_pair = context.operator_key_pair().cloned();
        let client = builder.build()?;
        let height = NonZeroU64::new(client.get_sumeragi_status()?.last_committed_height)
            .ok_or_else(|| eyre!("validator has no durable tip"))?;
        let attestation = client.get_bridge_finality_attestation(height, challenge, &peer.peer_id)?;
        validate_attestation(&authority, peer, challenge, &attestation)?;
        attestation.body.finality_proof.finality_artifact.verify()?;
        client.get_lane_lifecycle_status()?.validate()?;
    }
    Ok(())
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PeerReceipt {
    peer_id: PeerId,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    attestation: BridgeFinalityAttestationV1,
    transactions: Vec<PhaseObservationV1>,
    carriers: Vec<CarrierReceipt>,
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CarrierReceipt {
    height: u64,
    file: String,
    wire_sha256: String,
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CompletionV1 {
    schema_version: u8,
    operation_id: String,
    intent_sha256: String,
    network_id: NetworkId,
    challenge: [u8; 32],
    peers: Vec<PeerReceipt>,
}

fn namespace_matches(plan: &PlanV1, client: &Client) -> Result<()> {
    let now = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    for ensure in &plan.manifest.alias_request.intents {
        let (namespace, name) = match &ensure.intent {
            AliasIntentV1::Dataspace(value) => (
                SnsNamespacePath::Dataspace,
                value.dataspace.canonical_text(),
            ),
            AliasIntentV1::AccountAlias(value) => {
                (SnsNamespacePath::AccountAlias, value.alias.canonical_text())
            }
            _ => eyre::bail!("unexpected namespace intent"),
        };
        let record = client
            .sns()
            .get_name_optional(namespace, &name)?
            .ok_or_else(|| eyre!("paid namespace record is absent"))?;
        require(
            record.name_hash == record.selector.name_hash()
                && record.owner == plan.manifest.owner
                && record.status == NameStatus::Active
                && record.expires_at_ms > now
                && record.ownership_generation == 1,
            "paid namespace is missing, expired, transferred or owned by another account",
        )?;
        if let AliasIntentV1::Dataspace(_) = &ensure.intent {
            let mapped = if let Some(value) = record
                .metadata
                .get(iroha_core::sns::SNS_DATASPACE_ID_METADATA_KEY)
            {
                iroha_model_base::topology::DataSpaceId::new(json::from_str::<u64>(value.get())?)
            } else {
                iroha_model_base::topology::DataSpaceId::from_hash(&record.name_hash)
            };
            require(
                mapped == plan.manifest.dataspace.descriptor.id,
                "paid namespace maps to another physical dataspace",
            )?;
        }
        if let AliasIntentV1::AccountAlias(intent) = &ensure.intent {
            let resolved = client
                .resolve_account_alias_authenticated(&intent.alias.canonical_name)?
                .ok_or_else(|| eyre!("committed account alias does not resolve"))?;
            require(
                resolved.account_id() == &intent.target_account
                    && resolved.alias() == &intent.alias.canonical_name,
                "account alias resolves to another canonical account",
            )?;
            let selector = iroha::client::AccountAliasesByAccountRequestV1::try_new(
                &intent.target_account,
                Some(intent.alias.canonical_name.dataspace.as_ref()),
                None,
            )?;
            let aliases = client
                .list_account_aliases_authenticated(&selector)?
                .ok_or_else(|| eyre!("committed account alias index is absent"))?;
            let expected_primary =
                intent.role == iroha_data_model::alias_setup::AccountAliasRoleV1::Primary;
            require(
                aliases.items().iter().any(|row| {
                    row.alias() == &intent.alias.canonical_name
                        && row.is_primary() == expected_primary
                }),
                "account alias primary role differs from intent",
            )?;
        }
    }
    Ok(())
}

/// Read-only finality synchronization and final observations. No transaction is created here.
pub(super) fn complete<C: RunContext>(
    context: &C,
    plan: &PlanV1,
    journal: &Journal,
    report: &mut ReportV1,
) -> Result<()> {
    require(
        report.verification.transactions.len() == PHASES.len()
            && report
                .verification
                .transactions
                .iter()
                .all(|phase| phase.state == "applied_verification_pending"),
        "completion requires all three retained phases to be applied",
    )?;
    let trust = &plan.manifest.finality;
    let authority = trust.authority(plan.manifest.network_id)?;
    let challenge: [u8; 32] = rand::random();
    require(challenge != [0; 32], "random finality challenge is zero")?;
    let mut proofs = BTreeMap::<u64, BridgeFinalityProof>::new();
    let mut verifier = None::<BridgeFinalityVerifier>;
    let mut new_proofs = 0_usize;
    let mut receipts = Vec::new();
    for peer in &trust.peers {
        let mut config = context.config().clone();
        config.torii_api_url = peer.torii_origin.parse()?;
        let mut builder = Client::builder(config);
        builder.operator_key_pair = context.operator_key_pair().cloned();
        let client = builder.build()?;
        let height = client.get_sumeragi_status()?.last_committed_height;
        let height =
            NonZeroU64::new(height).ok_or_else(|| eyre!("validator has no durable tip"))?;
        let before = client.get_bridge_finality_attestation(height, challenge, &peer.peer_id)?;
        validate_attestation(&authority, peer, challenge, &before)?;
        let mut next = proofs.last_key_value().map_or(1, |(height, _)| height + 1);
        while next <= height.get() {
            let name = format!("proof-{next:020}.json");
            let cached: Option<BridgeFinalityProof> = journal.optional_json(&name)?;
            let fresh = cached.is_none();
            let proof = if let Some(proof) = cached {
                proof
            } else {
                if new_proofs == MAX_NEW_PROOFS {
                    report.state = "verification_sync_pending".into();
                    return Ok(());
                }
                new_proofs += 1;
                if next == 1 {
                    before.body.genesis_finality_proof.clone()
                } else {
                    let mut trial = verifier
                        .clone()
                        .ok_or_else(|| eyre!("missing genesis verifier"))?;
                    client.get_next_bridge_finality_proof(
                        NonZeroU64::new(next).unwrap(),
                        &mut trial,
                    )?
                }
            };
            require(
                proof.block_header.height().get() == next,
                "retained proof cache has a missing or reordered height",
            )?;
            authority.roster(&proof)?;
            if next == 1 {
                verifier = Some(authority.anchor(&proof)?);
            } else {
                verifier
                    .as_mut()
                    .ok_or_else(|| eyre!("missing genesis verifier"))?
                    .verify(&proof)?;
            }
            if fresh {
                journal.install_json(&name, &proof)?;
            }
            proofs.insert(next, proof);
            next = next
                .checked_add(1)
                .ok_or_else(|| eyre!("finality height overflow"))?;
        }
        require(
            proofs.get(&height.get()) == Some(&before.body.finality_proof),
            "peer durable tip differs from the independently verified successor chain",
        )?;
        let mut transactions = Vec::new();
        let mut carriers = Vec::new();
        for phase in PHASES {
            let prepared: PreparedV1 = journal.read_json(&format!("{phase}.prepared.json"))?;
            let transaction = prepared.verify(plan, phase)?;
            let observed = observe(&client, &prepared, &transaction)?;
            require(
                observed.state == "applied_verification_pending",
                "one validator has not applied the exact retained deployment transaction",
            )?;
            let carrier_height = matching_applied_height(
                &prepared.transaction_hash,
                &observed.global_status,
                &observed.peer_status,
            )?
            .ok_or_else(|| eyre!("missing exact carrier height"))?;
            let proof = proofs
                .get(&carrier_height)
                .ok_or_else(|| eyre!("transaction carrier is ahead of the verified peer tip"))?;
            let committed = &observed
                .committed
                .as_ref()
                .ok_or_else(|| eyre!("missing committed transaction"))?
                .transaction;
            require(
                committed.block_hash() == &proof.block_header.hash(),
                "transaction carrier differs from authenticated finality",
            )?;
            let wire = client.get_canonical_executed_block_wire(
                NonZeroU64::new(carrier_height).unwrap(),
                committed,
                &proof.finality_artifact.commit_qc.execution_commitment,
            )?;
            let maximum =
                iroha_data_model::block::proofs::AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1;
            let file = format!("carrier-{carrier_height:020}.nrt");
            if let Some(existing) = journal.read_optional_bounded(&file, maximum)? {
                require(
                    existing == wire,
                    "verified carrier bytes differ from retained evidence",
                )?;
            } else {
                journal.install_bounded(&file, &wire, maximum)?;
            }
            carriers.push(CarrierReceipt {
                height: carrier_height,
                file,
                wire_sha256: digest(&wire),
            });
            transactions.push(observed);
        }
        physical_matches(plan, &client)?;
        require(
            bootstrap_present(plan, &client)?,
            "one validator omits the exact bootstrap grant",
        )?;
        namespace_matches(plan, &client)?;
        let after = client.get_bridge_finality_attestation(height, challenge, &peer.peer_id)?;
        validate_attestation(&authority, peer, challenge, &after)?;
        require(
            after.body.finality_proof == before.body.finality_proof,
            "validator tip changed while reading deployment state; rerun status",
        )?;
        receipts.push(PeerReceipt {
            peer_id: peer.peer_id.clone(),
            height: height.get(),
            block_hash: after.body.finality_proof.block_header.hash(),
            attestation: after,
            transactions,
            carriers,
        });
    }
    require(receipts.len() == 4, "four validator receipts are required")?;
    let completion = CompletionV1 {
        schema_version: 1,
        operation_id: plan.operation_id.clone(),
        intent_sha256: plan.intent_sha256.clone(),
        network_id: plan.manifest.network_id,
        challenge,
        peers: receipts,
    };
    let receipt_name = format!("completion-{}.json", hex::encode(challenge));
    journal.install_json(&receipt_name, &completion)?;
    report.completion_receipt = Some(receipt_name);
    report.deployment_complete = true;
    report.state = "completed".into();
    Ok(())
}

#[cfg(test)]
pub(super) fn test_trust() -> TrustV1 {
    let (block, key) = crate::taira_public_reset::deployment_genesis_fixture();
    let validators = iroha_genesis::signed_genesis_validator_pops(&block).unwrap();
    TrustV1 {
        genesis_public_key: key.public_key().clone(),
        genesis_signed_wire_hex: hex::encode(block.encode_wire().unwrap()),
        peers: validators
            .into_keys()
            .enumerate()
            .map(|(index, key)| {
                let peer_id = PeerId::new(key);
                PeerV1 {
                    torii_origin: format!("http://127.0.0.1:{}/", 8080 + index),
                    node_fingerprint: Hash::new(peer_id.encode()),
                    peer_id,
                    build_fingerprint: Hash::new([1]),
                    config_fingerprint: Hash::new([2]),
                }
            })
            .collect(),
    }
}

#[cfg(test)]
pub(super) fn test_network_id() -> NetworkId {
    NetworkId::from_genesis_hash(
        crate::taira_public_reset::deployment_genesis_fixture()
            .0
            .hash(),
    )
}

#[cfg(test)]
#[path = "taira_dataspace_deploy_finality_tests.rs"]
mod tests;
