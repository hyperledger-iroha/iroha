//! Public-only artifact retention and independent replay for the seeded DEEP diagnostic.

use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest as _, Sha256};

use super::*;

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/fastpq-proof-diagnostics")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_bounded(path: &Path, cap: usize) -> io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    let bound = u64::try_from(cap).map_err(|_| invalid("artifact cap overflow"))?;
    if file.metadata()?.len() > bound {
        return Err(invalid("public artifact exceeds read cap"));
    }
    let mut bytes = Vec::new();
    file.take(bound.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(invalid("public artifact grew beyond read cap"));
    }
    Ok(bytes)
}

fn write_exact(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(bytes)?;
            file.sync_all()
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if read_bounded(path, bytes.len())? != bytes {
                return Err(invalid("existing public artifact differs"));
            }
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn retain_in(
    output: &Path,
    proof: &[u8],
    statement: &[u8],
    context: &[u8],
    elapsed: f64,
    charges: (usize, usize, usize),
) -> io::Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(output)?;
    let sha = hex::encode(Sha256::digest(proof));
    let artifact = output.join(format!("seeded-fixed-smt-{sha}.bin"));
    write_exact(&artifact, proof)?;
    // Receipt identity is per run, while the proof identity is content addressed.
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let receipt = output.join(format!(
        "seeded-fixed-smt-{sha}-{}-{tick}.receipt.txt",
        std::process::id()
    ));
    let public = format!(
        "format=fastpq-seeded-fixed-smt-public-receipt-v1\nproof_file={}\nproof_bytes={}\nproof_sha256={sha}\nproof_iroha_hash={}\npublic_context_hex={}\npublic_statement_canonical_payload_hex={}\npublic_statement_payload_flags={}\nbuild_and_self_check_seconds={elapsed:.9}\nplanned_payload_bytes={}\nplanned_work_units={}\nplanned_hash_calls={}\nprivate_witness_recorded=false\nverification_status=not_yet_checked_by_diagnostic_controls\n",
        artifact
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| invalid("artifact name"))?,
        proof.len(),
        iroha_crypto::Hash::new(proof),
        hex::encode(context),
        hex::encode(statement),
        norito::core::default_encode_flags(),
        charges.0,
        charges.1,
        charges.2,
    );
    write_exact(&receipt, public.as_bytes())?;
    // Unix supports syncing directory entries as well as the file contents.
    #[cfg(unix)]
    fs::File::open(output)?.sync_all()?;
    Ok((artifact, receipt))
}

pub(super) fn retain(
    proof: &[u8],
    statement: &[u8],
    context: &[u8],
    elapsed: f64,
    charges: (usize, usize, usize),
) -> io::Result<PathBuf> {
    let (artifact, receipt) = retain_in(&directory(), proof, statement, context, elapsed, charges)?;
    eprintln!(
        "retained_public_child={}; public_receipt={}",
        artifact.display(),
        receipt.display()
    );
    Ok(receipt)
}

pub(super) fn mark_controls_passed(receipt: &Path) -> io::Result<()> {
    let mut file = OpenOptions::new().append(true).open(receipt)?;
    writeln!(file, "diagnostic_controls=passed")?;
    file.sync_all()
}

fn read_addressed(path: &Path, cap: usize) -> io::Result<Vec<u8>> {
    let sha = path
        .file_name()
        .and_then(|v| v.to_str())
        .and_then(|v| v.strip_prefix("seeded-fixed-smt-"))
        .and_then(|v| v.strip_suffix(".bin"))
        .filter(|v| {
            v.len() == 64
                && v.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or_else(|| invalid("artifact must have its canonical SHA-256 name"))?;
    let bytes = read_bounded(path, cap)?;
    if hex::encode(Sha256::digest(&bytes)) != sha {
        return Err(invalid("artifact content differs from SHA-256 name"));
    }
    Ok(bytes)
}

#[test]
fn public_child_retention_is_exact_bounded_and_refuses_existing_mismatch() {
    let output = std::env::temp_dir().join(format!(
        "fastpq-public-child-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let proof = b"public artifact ownership fixture";
    let (artifact, receipt) =
        retain_in(&output, proof, b"statement", b"context", 1.25, (1, 2, 3)).unwrap();
    assert_eq!(read_addressed(&artifact, proof.len()).unwrap(), proof);
    assert!(read_addressed(&artifact, proof.len() - 1).is_err());
    write_exact(&artifact, proof).unwrap();
    mark_controls_passed(&receipt).unwrap();
    let text = fs::read_to_string(&receipt).unwrap();
    assert!(text.ends_with("diagnostic_controls=passed\n"));
    assert!(text.contains("private_witness_recorded=false\n"));
    assert!(text.contains("public_context_hex=636f6e74657874\n"));
    assert!(text.contains("planned_payload_bytes=1\nplanned_work_units=2\nplanned_hash_calls=3\n"));
    assert!(read_addressed(&receipt, usize::MAX).is_err());
    fs::write(&artifact, b"altered").unwrap();
    assert!(read_addressed(&artifact, proof.len()).is_err());
    assert!(write_exact(&artifact, proof).is_err());
    fs::remove_file(artifact).unwrap();
    fs::remove_file(receipt).unwrap();
    fs::remove_dir(output).unwrap();
}

#[test]
#[ignore = "requires a retained seeded public child; performs verification only, no witness or prover"]
fn captured_seeded_child_verifies_without_reproving() {
    // Expected facts are fixed independently before inspecting the file or receipt.
    let statement = complete_statement();
    let air = CompactTransferAir::new(&statement, Some(COMPLETE_CONTEXT)).unwrap();
    let path = PathBuf::from(
        std::env::var_os("FASTPQ_TEST_FIXED_SMT_ARTIFACT").expect("FASTPQ_TEST_FIXED_SMT_ARTIFACT"),
    );
    let proof = read_addressed(&path, deep_proof::PROOF_BYTE_TARGET).unwrap();
    assert_complete_proof_controls(&statement, &air, &proof);
    for changed in 0..5 {
        let mut other = statement;
        match changed {
            0 => other.updates[0].old_leaf = digest(91),
            1 => other.updates[1].new_leaf = digest(92),
            2 => other.updates[0].path ^= 1,
            3 => other.old_root = digest(93),
            _ => other.new_root = digest(94),
        }
        let air = CompactTransferAir::new(&other, Some(COMPLETE_CONTEXT)).unwrap();
        assert!(deep_engine::verify(&air, &proof, deep_proof::PROOF_BYTE_TARGET).is_err());
    }
}
