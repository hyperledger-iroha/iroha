//! CLI helper for generating the canonical `SoraFS` gateway fixture bundle.

use std::{env, path::PathBuf};

use eyre::WrapErr;
use integration_tests::sorafs_gateway_conformance::{default_fixture_dir, write_fixture_bundle};

fn main() -> eyre::Result<()> {
    let output_dir = parse_output_dir()?;
    let metadata =
        write_fixture_bundle(&output_dir).wrap_err("failed to generate SoraFS gateway fixtures")?;

    println!(
        "wrote SoraFS gateway fixtures {} to {}",
        metadata.version,
        output_dir.display()
    );
    println!(
        "  fixtures_digest={}\n  manifest_blake3={}\n  payload_blake3={}\n  car_blake3={}",
        metadata.fixtures_digest_blake3_hex,
        metadata.manifest_blake3_hex,
        metadata.payload_blake3_hex,
        metadata.car_blake3_hex
    );
    println!("  released_at_unix={}", metadata.released_at_unix);

    Ok(())
}

fn parse_output_dir() -> eyre::Result<PathBuf> {
    let mut args = env::args().skip(1);
    let mut output: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                let value = args
                    .next()
                    .ok_or_else(|| eyre::eyre!("--out requires a path argument"))?;
                let path = PathBuf::from(value);
                output = Some(if path.is_absolute() {
                    path
                } else {
                    env::current_dir()
                        .wrap_err("failed to resolve current working directory")?
                        .join(path)
                });
            }
            other => {
                return Err(eyre::eyre!(
                    "unrecognized argument `{}`; only `--out <path>` is supported",
                    other
                ));
            }
        }
    }

    Ok(output.unwrap_or_else(default_fixture_dir))
}
