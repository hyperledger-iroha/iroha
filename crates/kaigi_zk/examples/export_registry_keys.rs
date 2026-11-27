//! Export the fixed V1 public registration bytes from the original circuit definitions.
//! Run `cargo run -p kaigi_zk --example export_registry_keys -- <output-directory>`.
use std::{env, error::Error, fs, path::PathBuf};

use kaigi_zk::native::NativeRelationV1;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let directory = PathBuf::from(args.next().ok_or("expected an output directory")?);
    if args.next().is_some() {
        return Err("expected one output directory".into());
    }
    fs::create_dir_all(&directory)?;
    for (name, relation) in [
        ("authorization", NativeRelationV1::Authorization),
        ("usage", NativeRelationV1::Usage),
    ] {
        let verifier = relation.verifier()?;
        fs::write(
            directory.join(format!("{name}.descriptor")),
            verifier.descriptor_bytes(),
        )?;
        fs::write(directory.join(format!("{name}.vk")), verifier.key_bytes())?;
    }
    Ok(())
}
