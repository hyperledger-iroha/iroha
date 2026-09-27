//! Prove a local full redemption without choosing a circuit or creating dummy inputs.
//!
//! Run `cargo run -p iroha_core --example confidential_redemption`. The example
//! creates a disposable private wallet and local tree; it never submits a transaction.

use iroha_core::zk::{
    confidential::{ConfidentialProver, ConfidentialTree},
    confidential_v2::{
        ConfidentialUnshieldInputV2, compute_confidential_merkle_path_v2,
        default_confidential_diversifier_v2, derive_confidential_note_v2,
        derive_confidential_owner_tag_v2_with_diversifier,
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, asset::AssetDefinitionId, block::BlockHeader};
use rand_core_06::{OsRng, RngCore};
use zeroize::Zeroizing;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Applications resolve these identifiers from authenticated ledger state.
    let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"local-confidential-redemption-example"),
    ));
    let asset = AssetDefinitionId::from_uuid_bytes([
        1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
    ])?;
    let mut spend_key = Zeroizing::new([0; 32]);
    OsRng.try_fill_bytes(spend_key.as_mut())?;
    let mut input = ConfidentialUnshieldInputV2 {
        amount: 42,
        rho: [0; 32],
        diversifier: default_confidential_diversifier_v2(),
        leaf_index: 0,
    };
    OsRng.try_fill_bytes(&mut input.rho)?;
    let owner =
        derive_confidential_owner_tag_v2_with_diversifier(spend_key.as_ref(), input.diversifier)?;
    let commitment =
        derive_confidential_note_v2(&asset.to_string(), input.amount, input.rho, owner)?;
    let paths = [compute_confidential_merkle_path_v2(&[commitment], 0)?];

    let prover = ConfidentialProver::new(network, &asset, spend_key)?;
    let result = prover.prove_unshield(
        ConfidentialTree::Paths {
            root: paths[0].root,
            paths: &paths,
        },
        vec![input],
        42,
        None,
    )?;
    println!(
        "Generated and locally verified {:?}: {} bytes, {} input note.",
        result.relation,
        result.proof.bytes.len(),
        result.nullifiers.len()
    );
    // This local proof does not assemble a transaction. A protocol-specific
    // ledger admission path must authorize any actual movement of value.
    Ok(())
}
