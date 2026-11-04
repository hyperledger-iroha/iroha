//! Fresh beacon fixtures bind short epochs, public catalogs, and original stake ownership.

use color_eyre::eyre::{Result, ensure, eyre};
use iroha_config::profile::Profile;
use iroha_data_model::isi::sccp::InitializeSccpV1;
use iroha_genesis::RawGenesisTransaction;
use norito::json::{self, Value};
use std::collections::BTreeSet;

/// Preserve every original genesis field except one isolated SCCP initialization batch.
/// The beacon fixtures retain their short epochs; SCCP's protocol floor stays unchanged.
pub(super) fn manifest_value(raw: &RawGenesisTransaction) -> Result<Value> {
    let mut selected = raw.transactions().iter().enumerate().filter(|(_, tx)| {
        tx.instructions()
            .iter()
            .any(|instruction| instruction.as_any().is::<InitializeSccpV1>())
    });
    let (index, transaction) = selected
        .next()
        .ok_or_else(|| eyre!("generated short-epoch source omitted SCCP initialization"))?;
    ensure!(
        selected.next().is_none() && transaction.instructions().len() == 1,
        "short-epoch source must have exactly one isolated SCCP initializer"
    );
    let mut value = json::value::to_value(raw)?;
    let transactions = value
        .get_mut("transactions")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| eyre!("generated short-epoch manifest has no transactions"))?;
    let selected = &transactions[index];
    ensure!(
        selected.get("parameters").is_none()
            && ["topology", "ivm_triggers"].iter().all(|field| {
                selected
                    .get(*field)
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
            }),
        "SCCP initializer must not share authored parameters, topology or triggers"
    );
    transactions.remove(index);
    Ok(value)
}

#[test]
fn short_epoch_scope_preserves_all_other_genesis_fields_and_refuses_ambiguous_sources() {
    use iroha_data_model::{
        Level, block::consensus::SumeragiGenesisContextParameters,
        crypto_admission_policy::CryptoAdmissionPolicyV1, isi::Log, sccp::params::SccpParametersV1,
    };
    use iroha_model_base::chain::ChainId;
    iroha_genesis::init_instruction_registry();
    let custody = iroha_config::parameters::defaults::governance::bond_escrow_account_id();
    let staking = iroha_genesis::default_staking_policy(
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .unwrap(),
        custody.clone(),
        custody,
    );
    let before = iroha_genesis::GenesisBuilder::new_without_executor(
        ChainId::from("short-epoch-beacon"),
        ".",
        CryptoAdmissionPolicyV1::new(vec![iroha_crypto::Algorithm::Ed25519]).unwrap(),
    )
    .with_staking_policy(staking)
    .unwrap()
    .with_sumeragi_context_parameters(SumeragiGenesisContextParameters::recommended())
    .append_instruction(Log::new(Level::INFO, "retained fixture instruction".into()))
    .build_raw()
    .unwrap();
    assert!(manifest_value(&before).is_err());
    let init = InitializeSccpV1 {
        parameters: SccpParametersV1::taira_default(),
        reset_nonce: [7; 32],
    };
    let source = before
        .clone()
        .append_instruction_transaction(init.clone())
        .unwrap();
    let selected = manifest_value(&source).unwrap();
    assert_eq!(selected, json::value::to_value(&before).unwrap());
    let authored = RawGenesisTransaction::from_json_slice_at_path(
        &json::to_vec(&selected).unwrap(),
        std::path::Path::new("short-epoch.json"),
    )
    .unwrap();
    assert_eq!(
        authored.effective_parameters().unwrap(),
        before.effective_parameters().unwrap()
    );
    assert!(
        authored
            .instructions()
            .all(|instruction| !instruction.as_any().is::<InitializeSccpV1>())
    );
    let duplicate = source.clone().append_instruction_transaction(init).unwrap();
    assert!(manifest_value(&duplicate).is_err());
    let mut mixed = source.clone();
    let last = mixed.transactions().len() - 1;
    let mut instructions = mixed.transactions()[last].instructions().to_vec();
    instructions.push(Log::new(Level::INFO, "must remain".into()).into());
    mixed
        .replace_instruction_only_transaction(last, vec![instructions])
        .unwrap();
    assert!(manifest_value(&mixed).is_err());
}

/// Kagami's Digital Shekel asset and domain, which live in the Taira-only `is` data space.
const TAIRA_ONLY_MARKERS: [&str; 2] = ["7ZepsJTHCVLKsrFFNZGSRGZgvBhv", "\"boi.is\""];

/// Lane indices the profile's baseline catalog defines.
pub(super) fn profile_lanes(profile: &Profile) -> Result<BTreeSet<i64>> {
    profile
        .static_config()
        .get("nexus")
        .and_then(|nexus| nexus.get("lane_catalog"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| eyre!("profile has no lane catalog"))?
        .iter()
        .map(|lane| {
            lane.get("index")
                .and_then(toml::Value::as_integer)
                .ok_or_else(|| eyre!("lane without index"))
        })
        .collect()
}

/// Whether a Kagami Taira genesis instruction belongs to the Taira catalog the profile omits.
pub(super) fn taira_catalog_only(instruction: &Value, lanes: &BTreeSet<i64>) -> Result<bool> {
    let text = json::to_string(instruction)?;
    if TAIRA_ONLY_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
    {
        return Ok(true);
    }
    for kind in ["RegisterPublicLaneValidator", "ActivatePublicLaneValidator"] {
        if let Some(lane) = instruction
            .get(kind)
            .and_then(|body| body.get("lane_id"))
            .and_then(Value::as_i64)
        {
            return Ok(!lanes.contains(&lane));
        }
    }
    Ok(false)
}

/// Author the compiled public catalog while retaining original committees and stake custody.
pub(super) fn retarget_public_catalog(
    value: &mut Value,
    profile: &Profile,
    routed_authority: Option<&str>,
) -> Result<()> {
    use iroha_data_model::{
        parameter::system::Parameters,
        sumeragi_lanes::{SumeragiLanePolicy, SumeragiLaneRoute},
    };
    use iroha_model_base::topology::{DataSpaceId, LaneId};
    let lanes = profile_lanes(profile)?;
    ensure!(
        lanes == [0, 1, 2, 3].into_iter().collect(),
        "fixture needs the four-lane public profile"
    );
    let derived = profile.derived_config(&profile.derive(4)?);
    let data_spaces = derived["nexus"]["dataspace_catalog"]
        .as_array()
        .ok_or_else(|| eyre!("profile omitted its derived data spaces"))?;
    let catalog = profile.static_config()["nexus"]["lane_catalog"]
        .as_array()
        .ok_or_else(|| eyre!("profile omitted its lanes"))?;
    let transactions = value
        .get_mut("transactions")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| eyre!("generated public manifest has no transactions"))?;
    let mut moved = [0_usize; 2];
    let mut policies = 0;
    for transaction in transactions.iter_mut() {
        if let Some(instructions) = transaction
            .get_mut("instructions")
            .and_then(Value::as_array_mut)
        {
            let mut kept = Vec::with_capacity(instructions.len());
            for mut instruction in instructions.drain(..) {
                // The generator's public non-global stake owner is lane 5. Move that
                // original owner to the profile's public lane 3 before catalog filtering.
                for (index, kind) in ["RegisterPublicLaneValidator", "ActivatePublicLaneValidator"]
                    .iter()
                    .enumerate()
                {
                    if let Some(body) = instruction.get_mut(*kind)
                        && body.get("lane_id").and_then(Value::as_i64) == Some(5)
                    {
                        *body.get_mut("lane_id").expect("matched lane field") = Value::from(3_u32);
                        moved[index] += 1;
                    }
                }
                if !taira_catalog_only(&instruction, &lanes)? {
                    kept.push(instruction);
                }
            }
            *instructions = kept;
        }
        let Some(parameters) = transaction
            .get_mut("parameters")
            .filter(|value| !value.is_null())
        else {
            continue;
        };
        let mut typed: Parameters = json::value::from_value(parameters.clone())?;
        let mut policy = typed
            .custom
            .get(&SumeragiLanePolicy::parameter_id())
            .and_then(SumeragiLanePolicy::from_custom_parameter)
            .ok_or_else(|| eyre!("generated public manifest omitted its native lane policy"))?
            .map_err(|error| eyre!(error))?;
        policies += 1;
        policy
            .fixed
            .retain(|fixed| lanes.contains(&i64::from(fixed.lane.as_u32())));
        ensure!(
            policy.fixed.len() == 3,
            "public profile requires the original three fixed committees"
        );
        for fixed in &mut policy.fixed {
            let lane = catalog
                .iter()
                .find(|lane| lane["index"].as_integer() == Some(i64::from(fixed.lane.as_u32())))
                .ok_or_else(|| eyre!("fixed lane is absent from the compiled catalog"))?;
            let alias = lane["dataspace"]
                .as_str()
                .ok_or_else(|| eyre!("profile lane has no data space"))?;
            let id = data_spaces
                .iter()
                .find(|data_space| data_space["alias"].as_str() == Some(alias))
                .and_then(|data_space| data_space["id"].as_integer())
                .ok_or_else(|| eyre!("profile lane's data space is missing"))?;
            fixed.dataspace = DataSpaceId::new(u64::try_from(id)?);
            ensure!(
                fixed.committee.len() == 4,
                "public fixture requires its original four-seat committees"
            );
        }
        // The old alias routes name the removed Taira catalog. The qualification
        // profile has no account routes; its beacon fixture adds one exact account.
        policy.routes.clear();
        if let Some(authority) = routed_authority {
            policy.routes.push(SumeragiLaneRoute {
                lane: LaneId::new(3),
                account: Some(authority.to_owned()),
                instruction: None,
            });
        }
        policy.validate()?;
        typed.custom.insert(policy.into_custom_parameter());
        *parameters = json::value::to_value(&typed)?;
    }
    ensure!(
        moved == [4, 4] && policies == 1,
        "public fixture must retain exactly four original non-global validators and one lane policy"
    );
    transactions.retain(|transaction| {
        ["instructions", "topology", "ivm_triggers"]
            .iter()
            .any(|field| {
                transaction
                    .get(*field)
                    .and_then(Value::as_array)
                    .is_some_and(|entries| !entries.is_empty())
            })
            || transaction
                .get("parameters")
                .is_some_and(|parameters| !parameters.is_null())
    });
    Ok(())
}

#[test]
fn qualification_catalog_keeps_public_members_and_unrelated_instructions() {
    let profile = Profile::compiled(iroha_config::profile::ProfileId::SoraNexusV1Qual).unwrap();
    let lanes = profile_lanes(&profile).unwrap();
    assert_eq!(lanes, [0, 1, 2, 3].into_iter().collect());
    for kind in ["RegisterPublicLaneValidator", "ActivatePublicLaneValidator"] {
        for (lane, omitted) in [(3, false), (4, true), (6, true)] {
            let instruction: Value =
                json::from_str(&format!("{{\"{kind}\":{{\"lane_id\":{lane}}}}}")).unwrap();
            assert_eq!(taira_catalog_only(&instruction, &lanes).unwrap(), omitted);
        }
    }
    assert!(!taira_catalog_only(&norito::json!({"Log": {"msg": "retained"}}), &lanes).unwrap());
}

#[test]
fn public_catalog_refuses_missing_native_policy_and_original_stake_owners() {
    let profile = Profile::compiled(iroha_config::profile::ProfileId::SoraNexusV1Qual).unwrap();
    let mut missing = norito::json!({"transactions": []});
    assert!(retarget_public_catalog(&mut missing, &profile, None).is_err());
    let parameters = iroha_data_model::parameter::system::Parameters::default();
    let mut missing_policy = norito::json!({"transactions": [{"parameters": parameters}]});
    assert!(retarget_public_catalog(&mut missing_policy, &profile, None).is_err());
}
