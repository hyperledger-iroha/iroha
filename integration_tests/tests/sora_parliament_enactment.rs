//! Shared real four-validator network builders and signed fee funding for the Parliament corridors.
//! All transitions are submitted as signed transactions; there is no direct certificate/state setup.
use super::*;
use iroha::data_model::prelude::AssetId;

/// Positive corridor genesis: proof-valid beacon custody and the short signed `NPoS` epoch
/// whose native pre-boundary pulses are the only sortition slots.
pub fn builder(builder: NetworkBuilder) -> NetworkBuilder {
    with_signed_npos_epoch(
        builder_with_signer_modes(builder, POSITIVE_BEACON_SIGNER_MODES),
        CORRIDOR_NPOS_EPOCH_LENGTH_BLOCKS,
    )
}

/// The same citizen and governance genesis with caller-selected beacon custody and no
/// explicit `NPoS` snapshot, so the network signs the default epoch length.
pub fn builder_with_signer_modes(
    builder: NetworkBuilder,
    signer_modes: [ParliamentBeaconSignerMode; VALIDATOR_COUNT],
) -> NetworkBuilder {
    let citizens = citizen_accounts(&citizen_keys());
    let mut builder = builder
        .with_npos_consensus()
        .with_parliament_beacon_signer_modes(signer_modes)
        .with_block_cadence(EXACT_HEIGHT_SUBMISSION_CADENCE)
        .with_config_layer(|layer| {
            layer
                .write(
                    ["concurrency", "rayon_global_threads"],
                    PARLIAMENT_NETWORK_RAYON_THREADS_PER_PEER,
                )
                // Keep mandatory SoraNet admission enabled while bounding the
                // localhost-only puzzle cost so this corridor measures
                // Parliament/consensus liveness rather than Argon2 contention.
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1_i64,
                )
                .write(["nexus", "lane_count"], 1_i64)
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                )
                .write(["gov", "citizenship_bond_amount"], "0")
                .write(
                    ["gov", "min_enactment_delay"],
                    i64::try_from(MIN_ENACTMENT_DELAY)
                        .expect("fixture phase length fits TOML integer"),
                )
                .write(["gov", "parliament_alternate_size"], 0_i64)
                .write(
                    ["gov", "parliament_invitation_phase_blocks"],
                    i64::try_from(INVITATION_PHASE_BLOCKS)
                        .expect("fixture phase length fits TOML integer"),
                )
                .write(["gov", "parliament_public_finding_phase_blocks"], 20_i64)
                .write(["gov", "rules_committee_size"], i64::from(BODY_SEATS))
                .write(["gov", "agenda_council_size"], i64::from(BODY_SEATS))
                .write(["gov", "interest_panel_size"], i64::from(BODY_SEATS))
                .write(["gov", "review_panel_size"], i64::from(BODY_SEATS))
                .write(["gov", "oversight_committee_size"], i64::from(BODY_SEATS))
                .write(["gov", "policy_jury_size"], i64::from(BODY_SEATS))
                .write(["gov", "confirmation_jury_size"], i64::from(BODY_SEATS));
        });
    for citizen in &citizens {
        builder = builder
            .with_genesis_instruction(Register::account(Account::new(citizen.clone())))
            .with_genesis_instruction(RegisterCitizen {
                owner: citizen.clone(),
                amount: 0_u64.into(),
            });
    }

    builder
}

// Caller-registered citizens are absent from the network's fee bootstrap. Fund them with
// ordinary signed transfers using the exact XOR definition in the signed NPoS parameters.
pub async fn fund_citizen_fees(
    network: &iroha_test_network::Network,
    client: &Client,
    citizens: &[AccountId],
) -> Result<()> {
    let mut snapshots = network
        .genesis_isi()
        .iter()
        .flatten()
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .filter_map(|set| match set.inner() {
            Parameter::Custom(custom) if custom.id() == &SumeragiNposParameters::parameter_id() => {
                Some(custom)
            }
            _ => None,
        });
    let parameters = snapshots
        .next()
        .map(SumeragiNposParameters::from_custom_parameter)
        .transpose()?
        .flatten()
        .ok_or_else(|| eyre!("citizen fee funding requires signed canonical NPoS parameters"))?;
    if snapshots.next().is_some() {
        return Err(eyre!(
            "citizen fee funding refuses duplicate NPoS snapshots"
        ));
    }
    let source = AssetId::of(
        parameters.xor_asset_definition_id,
        client.client().account().clone(),
    );
    // One ordinary transfer per signed entry leaves room for its fee fragments
    // under the committed intrinsic FASTPQ ceiling. A single 24-transfer entry
    // cannot fit the bootstrap limit of 16, even before fee fragments.
    for instructions in citizen_fee_entries(&source, citizens) {
        submit_parliament_instructions(client, instructions).await?;
    }
    Ok(())
}

fn citizen_fee_entries<'a>(
    source: &'a AssetId,
    citizens: &'a [AccountId],
) -> impl Iterator<Item = [InstructionBox; 1]> + 'a {
    citizens.iter().map(|citizen| {
        [iroha_data_model::isi::Transfer::asset_quantity(
            source.clone(),
            10_000_u32,
            citizen.clone(),
        )
        .into()]
    })
}

#[test]
fn citizen_fee_entries_preserve_every_recipient_amount_and_source_in_bounded_entries() {
    use iroha::data_model::prelude::{Asset, Quantity, Transfer};
    let citizens = citizen_accounts(&citizen_keys());
    let source = AssetId::of(
        SumeragiNposParameters::default().xor_asset_definition_id,
        citizens[0].clone(),
    );
    let entries = citizen_fee_entries(&source, &citizens).collect::<Vec<_>>();
    assert_eq!(entries.len(), CITIZEN_COUNT);
    assert!(
        CITIZEN_COUNT
            > usize::try_from(
                iroha_data_model::parameter::FastpqSourcePolicyV1::bootstrap()
                    .intrinsic
                    .max_transcripts,
            )
            .unwrap()
    );
    for (entry, citizen) in entries.iter().zip(&citizens) {
        let transfer = entry[0]
            .as_any()
            .downcast_ref::<Transfer<Asset, Quantity, Account>>()
            .expect("one ordinary signed funding transfer per entry");
        assert_eq!(&transfer.source, &source);
        assert_eq!(&transfer.destination, citizen);
        assert_eq!(transfer.object, Quantity::from(10_000_u32));
    }
    assert_eq!(citizen_fee_entries(&source, &[]).count(), 0);
}
