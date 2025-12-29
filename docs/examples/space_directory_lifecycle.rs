//! Minimal Space Directory lifecycle helper demonstrating publish/expire flows.
//!
//! Build with:
//!     cargo run --example space_directory_lifecycle --features="iroha/client"

use eyre::Result;
use iroha::client::Client;
use iroha::config::Config;
use iroha::data_model::{
    isi::{
        InstructionBox,
        space_directory::{ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest},
    },
    nexus::{AssetPermissionManifest, DataSpaceId, ManifestVersion, UniversalAccountId},
};

fn publish_and_expire(client: &Client) -> Result<()> {
    // Replace with the UAID/dataspace you are managing.
    let uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11"
        .parse::<UniversalAccountId>()?;
    let dataspace = DataSpaceId::new(11);

    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 1_762_723_200_000,
        activation_epoch: 4_097,
        expiry_epoch: Some(4_700),
        entries: vec![], // Populate with your capability entries.
    };

    client.submit_all([InstructionBox::from(PublishSpaceDirectoryManifest {
        manifest: manifest.clone(),
    })])?;

    client.submit_all([InstructionBox::from(ExpireSpaceDirectoryManifest {
        uaid: manifest.uaid,
        dataspace: manifest.dataspace,
        expired_epoch: manifest.expiry_epoch.expect("expiry_epoch set"),
    })])?;
    Ok(())
}

fn main() -> Result<()> {
    let cfg = Config::from_path("client.toml")?;
    let client = Client::new(cfg)?;
    publish_and_expire(&client)
}
