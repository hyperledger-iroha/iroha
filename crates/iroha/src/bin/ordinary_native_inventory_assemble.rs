//! Release-assembly producer of the actual checked unsigned shared public inventory.
//! No signing/private key or physical device is read by this command.
use std::{io::Read as _, path::Path};
fn main() -> eyre::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    eyre::ensure!(
        args.len() == 4,
        "expected original root, canonical inventory body, new unsigned output"
    );
    let mut original = std::fs::File::open(&args[2])?;
    let mut body = Vec::new();
    original
        .by_ref()
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut body)?;
    eyre::ensure!(
        !body.is_empty() && body.len() <= 4 * 1024 * 1024,
        "inventory body bound rejected"
    );
    let decoded: iroha::client::KagemushaOrdinaryNativeInventoryV1 =
        norito::decode_canonical_with_limits(&body, norito::canonical_decode_limits(body.len()))?;
    eyre::ensure!(
        norito::encode_canonical(&decoded)? == body,
        "inventory body must be exact canonical Norito"
    );
    let packet = iroha::client::assemble_kagemusha_ordinary_native_inventory_v1(
        Path::new(&args[1]),
        &decoded,
    )?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[3])?;
    use std::io::Write as _;
    output.write_all(&packet)?;
    output.sync_all()?;
    std::fs::File::open(
        Path::new(&args[3])
            .parent()
            .ok_or_else(|| eyre::eyre!("output parent absent"))?,
    )?
    .sync_all()?;
    Ok(())
}
