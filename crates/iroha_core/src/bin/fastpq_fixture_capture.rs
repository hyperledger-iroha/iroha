//! One-shot local helper for refreshing execution-captured FASTPQ measurement fixtures.
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use fastpq_prover::{
    OperationKind, PublicInputs, StateTransition, TransitionBatch, transition_batch_to_model,
};
use iroha_core::{
    fastpq,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::prelude::*;
use nonzero_ext::nonzero;
use std::env;
fn account(label: &str) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(label).into();
    let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
        .expect("deterministic fixture account key");
    AccountId::new(keypair.public_key().clone())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let dsid: u64 = args.next().ok_or("missing source dataspace id")?.parse()?;
    let entry_hash = args.next().ok_or("missing entry hash")?;
    let capture_kind = args
        .next()
        .ok_or("missing capture kind (`transfer` or `opaque-effect`)")?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let entry_hash: [u8; Hash::LENGTH] = hex::decode(entry_hash)?
        .try_into()
        .map_err(|_| "entry hash must be exactly 32 bytes")?;
    let entry_hash = Hash::prehashed(entry_hash);
    let mut dsid_bytes = [0_u8; 16];
    dsid_bytes[..8].copy_from_slice(&dsid.to_le_bytes());
    let public_inputs = PublicInputs {
        dsid: dsid_bytes,
        slot: 1,
        old_root: [0_u8; 32],
        new_root: [0_u8; 32],
        perm_root: [0x33_u8; 32],
        tx_set_hash: [0x44_u8; 32],
    };
    let batch = match capture_kind.as_str() {
        "transfer" => capture_transfer_batch(public_inputs, entry_hash)?,
        "opaque-effect" => capture_opaque_effect_batch(public_inputs, entry_hash)?,
        _ => return Err("capture kind must be `transfer` or `opaque-effect`".into()),
    };
    let encoded = norito::to_bytes(&transition_batch_to_model(&batch))?;
    println!("{}", BASE64_STANDARD.encode(encoded));
    Ok(())
}

fn capture_transfer_batch(
    public_inputs: PublicInputs,
    entry_hash: Hash,
) -> Result<TransitionBatch, Box<dyn std::error::Error>> {
    let alice_id = account("pkdeploy-fastpq-fixture-alice");
    let bob_id = account("pkdeploy-fastpq-fixture-bob");
    let domain_id = DomainId::try_new("wonderland", "universal")?;
    let domain = Domain::new(domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let bob_account = Account::new(bob_id.clone()).build(&bob_id);
    let asset_definition_id = AssetDefinitionId::derive_from_components(domain_id, "rose".parse()?);
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&alice_id);
    let alice_asset_id = AssetId::new(asset_definition_id.clone(), alice_id.clone());
    let bob_asset_id = AssetId::new(asset_definition_id, bob_id.clone());
    let alice_asset = Asset::new(alice_asset_id.clone(), Quantity::from(1_000_u32));
    let bob_asset = Asset::new(bob_asset_id, Quantity::from(75_u32));
    let world = World::with_assets(
        [domain],
        [alice_account, bob_account],
        [asset_definition],
        [alice_asset, bob_asset],
        [],
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    transaction.tx_call_hash = Some(entry_hash);
    Transfer::asset_quantity(alice_asset_id, 75_u32, bob_id)
        .execute(&alice_id, &mut transaction)?;
    transaction.apply();
    let mut captured = block.drain_transfer_transcripts();
    let transcripts = captured
        .remove(&entry_hash)
        .ok_or("executed transfer did not produce a FASTPQ transcript")?;
    if !captured.is_empty() || transcripts.len() != 1 {
        return Err("execution capture produced an unexpected transcript set".into());
    }
    let batch = fastpq::batch_from_transcript_bundle(
        "fastpq-lane-balanced",
        public_inputs,
        entry_hash,
        &transcripts,
    )?;
    Ok(batch)
}

fn capture_opaque_effect_batch(
    public_inputs: PublicInputs,
    entry_hash: Hash,
) -> Result<TransitionBatch, Box<dyn std::error::Error>> {
    // Authorization and compliance use the explicitly selected AXT opaque-effect
    // profile. Its execution carrier must therefore contain only MetaSet rows;
    // reusing a captured transfer row would select an invalid statement shape.
    // Execute both sides of the metadata transition so the maintained synthetic
    // measurement carrier is execution-derived. It is deliberately not an
    // execution transcript or evidence of the surrounding business effect.
    let fixture_account_id = account("pkdeploy-fastpq-fixture-opaque-effect");
    let domain_id = DomainId::try_new("wonderland", "universal")?;
    let domain = Domain::new(domain_id).build(&fixture_account_id);
    let fixture_account = Account::new(fixture_account_id.clone()).build(&fixture_account_id);
    let state = State::new_for_testing(
        World::with([domain], [fixture_account], []),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    transaction.tx_call_hash = Some(entry_hash);
    let metadata_key: Name = "pkdeploy_fastpq_opaque_effect".parse()?;
    SetKeyValue::account(
        fixture_account_id.clone(),
        metadata_key.clone(),
        Json::new("pending"),
    )
    .execute(&fixture_account_id, &mut transaction)?;
    let old_value = transaction
        .world
        .account(&fixture_account_id)?
        .metadata()
        .get(&metadata_key)
        .ok_or("executed metadata write did not persist its pending value")?
        .as_ref()
        .as_bytes()
        .to_vec();
    SetKeyValue::account(
        fixture_account_id.clone(),
        metadata_key.clone(),
        Json::new("authenticated"),
    )
    .execute(&fixture_account_id, &mut transaction)?;
    let new_value = transaction
        .world
        .account(&fixture_account_id)?
        .metadata()
        .get(&metadata_key)
        .ok_or("executed metadata write did not persist its authenticated value")?
        .as_ref()
        .as_bytes()
        .to_vec();
    transaction.apply();
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", public_inputs);
    batch.push(StateTransition::new(
        b"axt/pkdeploy/proof-budget/opaque-effect".to_vec(),
        old_value,
        new_value,
        OperationKind::MetaSet,
    ));
    batch.sort();
    batch
        .metadata
        .insert("entry_hash".to_owned(), entry_hash.as_ref().to_vec());
    Ok(batch)
}
