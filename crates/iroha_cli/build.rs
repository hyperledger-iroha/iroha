//! Build script to extract git hash of Iroha build

use std::{fs, path::PathBuf};

use color_eyre::{
    eyre::{eyre, WrapErr},
    Result,
};
use iroha_data_model::{account::NewAccount, domain::NewDomain, prelude::*};
use parity_scale_codec::Encode;
use serde::de::DeserializeOwned;

fn main() -> Result<()> {
    vergen::EmitBuilder::builder()
        .git_sha(true)
        .emit()
        .map_err(|err| eyre!(Box::new(err)))
        .wrap_err("Failed to extract git hash")?;

    // Codec
    sample_into_binary_file::<NewAccount>("account").expect("Failed to encode into account.bin.");
    sample_into_binary_file::<NewDomain>("domain").expect("Failed to encode into domain.bin.");
    sample_into_binary_file::<Trigger>("trigger").expect("Failed to encode into trigger.bin.");

    Ok(())
}

fn sample_into_binary_file<T>(filename: &str) -> Result<()>
where
    T: Encode + DeserializeOwned,
{
    let mut path_to = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path_to.push("../../");
    path_to.push("samples/codec/");
    path_to.push(filename);

    let path_to_json = path_to.with_extension("json");
    let path_to_binary = path_to.with_extension("bin");

    println!("cargo:rerun-if-changed={}", path_to_json.to_str().unwrap());
    let buf = fs::read_to_string(path_to_json)?;

    let sample = serde_json::from_str::<T>(buf.as_str())?;

    let buf = sample.encode();

    fs::write(path_to_binary, buf)?;

    Ok(())
}
