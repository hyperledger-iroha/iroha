//! Non-admin global candidacy escrows canonical XOR before any authenticated committee change.
//!
//! Four real NPoS validators and a separate signed observer qualify admission, foreign-peer
//! proof rejection and delayed eligibility; this scenario does not claim later seat activation.

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::{sandbox, sync::rebind_blocking_client};
use iroha::{
    blocking::Client,
    crypto::SignatureOf,
    data_model::{
        bridge::{BridgeFinalityProof, verify_bridge_finality_proof},
        isi::staking::{
            PublicLaneCandidateAuthorization, RegisterPublicLaneCandidate,
            RegisterPublicLaneValidator,
        },
        nexus::{
            PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1,
            PublicLaneMonetaryRegistrationV1, PublicLaneMonetaryScopeV1,
        },
        parameter::system::SumeragiNposParameters,
        prelude::*,
        transaction::{FeePaymentIntent, error::TransactionRejectionReason},
        validation_fee::ValidationFeePolicyRegistryV1,
    },
};
use iroha_executor_data_model::permission::peer::CanManagePeers;
use iroha_model_base::{metadata::Metadata, topology::LaneId};
use iroha_test_network::{NetworkBuilder, ObserverP2pBootstrap, init_instruction_registry};
use iroha_test_samples::ALICE_ID;
use norito::json::Value;
use std::{
    collections::BTreeSet,
    num::NonZeroU64,
    thread,
    time::{Duration, Instant},
};

const EPOCH_LENGTH: u64 = 3_600;
const ACTIVATION_HEIGHT: u64 = EPOCH_LENGTH * 2 + 1;
const WAIT: Duration = Duration::from_secs(180);
const TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";

fn items(value: &Value) -> Result<&[Value]> {
    value
        .get("items")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| eyre!("staking response has no items array"))
}

fn validator_entry(client: &Client, validator: &str) -> Result<Option<Value>> {
    let response = client.client().get_public_lane_validators(LaneId::SINGLE)?;
    Ok(items(&response)?
        .iter()
        .find(|entry| entry.get("validator").and_then(Value::as_str) == Some(validator))
        .cloned())
}

/// Restrict candidate consent to one future election, allowing only its exact key lead window.
fn candidate_registration_window(
    current_height: u64,
    epoch_length: u64,
    key_lead: u64,
) -> Result<(u64, u64)> {
    ensure!(
        current_height > 0 && epoch_length > 0,
        "candidate requires a real nonzero height and epoch"
    );
    let first_execution = current_height
        .checked_add(1)
        .ok_or_else(|| eyre!("candidate execution height overflowed"))?;
    let first_key_height = first_execution
        .checked_add(key_lead)
        .ok_or_else(|| eyre!("candidate key height overflowed"))?;
    let epoch_end = ((first_key_height - 1) / epoch_length)
        .checked_add(1)
        .and_then(|epoch| epoch.checked_mul(epoch_length))
        .ok_or_else(|| eyre!("candidate epoch end overflowed"))?;
    ensure!(
        first_key_height < epoch_end,
        "candidate key lead reached an already frozen boundary"
    );
    let valid_until = epoch_end
        .checked_sub(1)
        .and_then(|height| height.checked_sub(key_lead))
        .ok_or_else(|| eyre!("candidate validity height underflowed"))?;
    let activation = epoch_end
        .checked_add(1)
        .ok_or_else(|| eyre!("candidate activation overflowed"))?;
    ensure!(
        valid_until >= first_execution,
        "candidate validity does not include an executable height"
    );
    Ok((valid_until, activation))
}

#[test]
fn candidate_consent_window_is_finite_and_cannot_cross_its_key_election() {
    assert_eq!(candidate_registration_window(2, 20, 3).unwrap(), (16, 21));
    assert_eq!(candidate_registration_window(22, 20, 3).unwrap(), (36, 41));
    for (height, epoch, lead) in [
        (0, 20, 3),
        (1, 0, 0),
        (16, 20, 3),
        (u64::MAX, 20, 3),
        (1, 20, u64::MAX),
    ] {
        assert!(candidate_registration_window(height, epoch, lead).is_err());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn fresh_global_candidate_bonds_before_authenticated_committee_activation() -> Result<()> {
    init_instruction_registry();
    let mut npos = SumeragiNposParameters::default();
    npos.epoch_length_blocks = NonZeroU64::new(EPOCH_LENGTH).expect("nonzero epoch");
    npos.max_validators = 4;
    npos.min_self_bond = 1_000_u64.into();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_npos_consensus()
        .with_observer_p2p_bootstrap(ObserverP2pBootstrap::new(1)?)?
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )));
    let network = sandbox::start_network_async_or_skip(
        builder, stringify!(fresh_global_candidate_bonds_before_authenticated_committee_activation),
    ).await?.ok_or_else(|| eyre!("candidate admission requires a real four-validator network; sandbox skip is not qualification"))?;
    let result = async {
        ensure!(network.validators().len() == 4 && network.observers().len() == 1,
            "candidate scenario requires four validators and one separate observer");
        let observer = &network.observers()[0];
        let validator = observer.account_id();
        let peer_id = observer.id();
        let foreign_peer_key = network.validators()[0].bls_key_pair().expect("validator BLS key").clone();
        let peer_key = observer.bls_key_pair().expect("observer BLS key").clone();
        let proof = observer.bls_pop().expect("observer proof").to_vec();
        let network_id = network.network_id();
        let configured_staking = |field: &str| -> Result<String> {
            network.config_layers().filter_map(|layer| {
                layer.get("nexus")?.get("staking")?.get(field)?.as_str().map(str::to_owned)
            }).last().ok_or_else(|| eyre!("actual candidate bootstrap omitted staking {field}"))
        };
        let stake_definition: AssetDefinitionId = configured_staking("stake_asset_id")?.parse()?;
        let stake_escrow = AccountId::parse_encoded(&configured_staking("stake_escrow_account_id")?)?;

        let admin = rebind_blocking_client(&network.client(), |client| {
            client.transaction_status_timeout = WAIT;
        });
        let operator = rebind_blocking_client(&network.peers()[0].client_for(
            &validator, observer.streaming_key_pair().private_key().clone(),
        ), |client| { client.transaction_status_timeout = WAIT; });
        let readers = network.all_peers().map(|peer| peer.client()).collect::<Vec<_>>();
        let applied_height = tokio::task::spawn_blocking(move || -> Result<u64> {
            // This fixture isolates custody and election admission. Fee-enabled
            // candidate quoting and charging require their separate qualification.
            // Preserve that scope explicitly instead of treating fee rejection as success.
            let parameters = admin.client().query_single(FindParameters)?;
            ensure!(!parameters.custom().contains_key(&ValidationFeePolicyRegistryV1::parameter_id()),
                "candidate admission fixture requires no enacted validation-fee policy");
            let literal = validator.canonical_i105()?;
            ensure!(validator_entry(&admin, &literal)?.is_none(), "operator is not a genesis validator");
            ensure!(!admin.client().query(FindPeers).execute_all()?.contains(&peer_id),
                "observer must begin outside the registered validator peers");
            admin.submit_all::<InstructionBox>([
                Register::account(Account::new(validator.clone())).into(),
                Transfer::asset_quantity(AssetId::new(stake_definition.clone(), ALICE_ID.clone()),
                    13_000_u64, validator.clone()).into(),
            ], FeePaymentIntent::authority(Vec::new(), None))?;
            let permissions = admin.client().query(FindPermissionsByAccountId::new(validator.clone())).execute_all()?;
            let manage_peers: Permission = CanManagePeers.into();
            ensure!(!permissions.contains(&manage_peers), "operator must not receive peer-management authority");
            let current_height = admin.status().get()?.blocks;
            let parameters = admin.client().query_single(FindParameters)?;
            let schedule = parameters.custom().get(&SumeragiNposParameters::parameter_id())
                .and_then(SumeragiNposParameters::from_custom_parameter)
                .ok_or_else(|| eyre!("candidate consent requires the committed NPoS schedule"))?;
            let (valid_until_height, activation_height) = candidate_registration_window(
                current_height, schedule.epoch_length_blocks.get(),
                parameters.sumeragi().key_activation_lead_blocks,
            )?;
            let registration = RegisterPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: validator.clone(),
                peer_id: peer_id.clone(),
                stake_account: validator.clone(),
                initial_stake: 2_000_u64.into(),
                metadata: Metadata::default(),
                monetary_plan: PublicLaneMonetaryPlanV1 {
                    network_scope: PublicLaneMonetaryScopeV1::Network(network_id.clone()),
                    valid_until_height,
                    source_asset: AssetId::new(stake_definition.clone(), validator.clone()),
                    destination_asset: AssetId::new(stake_definition.clone(), stake_escrow.clone()),
                    amount: 2_000_u64.into(),
                    precondition: PublicLaneMonetaryPreconditionV1::Registration(
                        PublicLaneMonetaryRegistrationV1 { activation_height },
                    ),
                },
            };
            let authorization = PublicLaneCandidateAuthorization::new(network_id, registration.clone(), activation_height);
            let candidate = RegisterPublicLaneCandidate {
                registration,
                activation_height,
                proof_of_possession: proof,
                peer_signature: SignatureOf::try_new(peer_key.private_key(), &authorization)?,
            };
            let mut foreign = candidate.clone();
            foreign.peer_signature = SignatureOf::try_new(foreign_peer_key.private_key(), &authorization)?;
            let error = operator.submit(foreign, FeePaymentIntent::authority(Vec::new(), None))
                .expect_err("another consensus peer must not authorize this candidate's keys");
            let rejection = error.downcast_ref::<TransactionRejectionReason>()
                .ok_or_else(|| eyre!("expected an on-chain rejection, got {error:?}"))?;
            ensure!(format!("{rejection:?}").contains("candidate peer signature"),
                "foreign peer consent failed for an unexpected reason: {rejection:?}");
            ensure!(validator_entry(&admin, &literal)?.is_none(), "failed consent must not create a validator");
            ensure!(!admin.client().query(FindPeers).execute_all()?.contains(&peer_id),
                "failed consent must not register the candidate peer");
            ensure!(items(&admin.client().get_public_lane_stake(LaneId::SINGLE, Some(&literal))?)?.is_empty(),
                "failed consent must not create stake custody");
            let source = AssetId::new(stake_definition.clone(), validator.clone());
            ensure!(admin.client().query_single(FindAssetById::new(source.clone()))?.value() == &Quantity::from(13_000_u64),
                "failed consent must leave liquid XOR unchanged");
            ensure!(admin.client().query_single(FindAssetById::new(escrow_asset.clone()))?.value() == &escrow_before,
                "failed consent must leave pooled XOR custody unchanged");
            operator.submit(candidate, FeePaymentIntent::authority(Vec::new(), None))
                .wrap_err("ordinary observer must admit a future candidacy with its own peer consent and XOR")?;
            let applied_height = admin.status().get()?.blocks;
            for reader in readers {
                let deadline = Instant::now() + WAIT;
                while reader.status().get()?.blocks < applied_height {
                    ensure!(Instant::now() < deadline, "staking reader did not apply the candidate admission height");
                    thread::sleep(Duration::from_millis(100));
                }
                let entry = validator_entry(&reader, &literal)?.ok_or_else(|| eyre!("admitted candidate missing on replica"))?;
                ensure!(entry.get("activation_height").and_then(Value::as_u64) == Some(ACTIVATION_HEIGHT),
                    "candidate must target the exact E+2 activation boundary");
                ensure!(entry.get("self_stake").and_then(Value::as_str) == Some("2000")
                    && entry.get("total_stake").and_then(Value::as_str) == Some("2000"),
                    "candidate record must retain the exact self bond");
                ensure!(entry.get("status").and_then(|status| status.get("type")).and_then(Value::as_str) == Some("PendingActivation"),
                    "candidate cannot become active before the certified target boundary");
                ensure!(reader.client().query(FindPeers).execute_all()?.contains(&peer_id),
                    "successful candidacy must register the exact consented consensus peer");
                let shares = reader.client().get_public_lane_stake(LaneId::SINGLE, Some(&literal))?;
                let shares = items(&shares)?;
                ensure!(shares.len() == 1 && shares[0].get("bonded").and_then(Value::as_str) == Some("2000")
                    && shares[0].get("staker").and_then(Value::as_str) == Some(literal.as_str()),
                    "admission must create one exact self-owned stake share");
                let liquid = reader.client().query_single(FindAssetById::new(source.clone()))?;
                ensure!(liquid.value() == &Quantity::from(11_000_u64), "admission must escrow exactly 2000 XOR on every replica");
                let escrow_after = reader.client().query_single(FindAssetById::new(escrow_asset.clone()))?;
                ensure!(escrow_after.value() == &escrow_before.checked_add(&Quantity::from(2_000_u64))?,
                    "pooled escrow must increase by the exact candidate bond");
                let status = reader.client().get_sumeragi_status()?;
                status.validate().map_err(|error| eyre!("invalid v2 status: {error}"))?;
                ensure!(status.height < ACTIVATION_HEIGHT && status.height_context.validator_count == 4,
                    "candidate and observer must not pad the current four-validator committee");
            }
            Ok(applied_height)
        }).await.wrap_err("candidate scenario worker panicked")??;
        let response = integration_tests::http::client()
            .get(network.client().client().endpoint().join(&format!("v1/bridge/finality/{applied_height}"))?)
            .header(reqwest::header::ACCEPT, "application/json")
            .send().await?.error_for_status()?.bytes().await?;
        let proof: BridgeFinalityProof = norito::json::from_slice(&response)?;
        verify_bridge_finality_proof(&proof, &network.network_id())
            .wrap_err("candidate admission finality failed cryptographic verification")?;
        let context = &proof.finality_artifact.height_context;
        let expected = network.validators().iter().map(|peer| peer.id()).collect::<BTreeSet<_>>();
        let actual = context.roster.iter().map(|entry| entry.validator.clone()).collect::<BTreeSet<_>>();
        ensure!(actual == expected && context.roster.len() == 4 && context.roster.iter().all(|entry| entry.power == 1)
            && context.quorum.min_signers == 3 && context.quorum.total_power == 4
            && proof.finality_artifact.commit_qc.signers.len() == 3,
            "candidate admission must preserve exactly four equal voters and three authenticated commit signatures");
        ensure!(proof.finality_artifact.height == applied_height && proof.block_header.height().get() == applied_height,
            "candidate admission finality proof must authenticate the applied height");
        Ok(())
    }.await;
    network.shutdown_and_release().await;
    result
}

#[test]
fn staking_items_rejects_malformed_responses() {
    assert!(
        items(&norito::json!({"items":[]}))
            .expect("items")
            .is_empty()
    );
    assert!(items(&norito::json!({"items":null})).is_err());
    assert!(items(&norito::json!({})).is_err());
}
