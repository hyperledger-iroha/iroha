#![allow(clippy::restriction)]

use eyre::{Result, WrapErr};
use iroha_crypto::KeyPair;
use iroha_data_model::prelude::*;
use iroha_logger::info;
use test_network::*;

#[test]
fn validator_upgrade_should_work() -> Result<()> {
    let (_rt, _peer, client) = <PeerBuilder>::new().with_port(10_795).start_with_runtime();
    wait_for_genesis_committed(&vec![client.clone()], 0);

    // Register `admin` domain and account
    let admin_domain = Domain::new("admin".parse()?);
    let register_admin_domain = RegisterBox::new(admin_domain);
    client.submit_blocking(register_admin_domain)?;

    let admin_id: <Account as Identifiable>::Id = "admin@admin".parse()?;
    let admin_keypair = KeyPair::generate()?;
    let admin_account = Account::new(admin_id.clone(), [admin_keypair.public_key().clone()]);
    let register_admin_account = RegisterBox::new(admin_account);
    client.submit_blocking(register_admin_account)?;

    // Check that admin isn't allowed to transfer alice's rose by default
    let alice_rose: <Asset as Identifiable>::Id = "rose##alice@wonderland".parse()?;
    let admin_rose: <Account as Identifiable>::Id = "admin@admin".parse()?;
    let transfer_alice_rose = TransferBox::new(alice_rose, NumericValue::U32(1), admin_rose);
    let transfer_rose_tx = TransactionBuilder::new(
        admin_id.clone(),
        vec![transfer_alice_rose.clone().into()],
        100_000,
    )
    .sign(admin_keypair.clone())?;
    let _ = client
        .submit_transaction_blocking(transfer_rose_tx)
        .expect_err("Should fail");

    // Upgrade Validator
    info!("Building validator");
    let temp_out_dir =
        tempfile::tempdir().wrap_err("Failed to create temporary output directory")?;

    let wasm =
        iroha_wasm_builder::Builder::new("tests/integration/smartcontracts/validator_with_admin")
            .out_dir(temp_out_dir.path())
            .build()?
            .optimize()?
            .into_bytes();

    temp_out_dir
        .close()
        .wrap_err("Failed to remove temporary output directory")?;

    info!("WASM size is {} bytes", wasm.len());

    let upgrade_validator = UpgradeBox::new(Validator::new(WasmSmartContract::from_compiled(wasm)));
    client.submit_blocking(upgrade_validator)?;

    // Check that admin can transfer alice's rose now
    // Creating new transaction instead of cloning, because we need to update it's creation time
    let transfer_rose_tx =
        TransactionBuilder::new(admin_id, vec![transfer_alice_rose.into()], 100_000)
            .sign(admin_keypair)?;
    client
        .submit_transaction_blocking(transfer_rose_tx)
        .expect("Should succeed");

    Ok(())
}
