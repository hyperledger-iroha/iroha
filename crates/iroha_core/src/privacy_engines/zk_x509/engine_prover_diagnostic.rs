//! Opt-in complete structural-maximum credential proof and public-only receipt.
//!
//! This test uses the real first-release producer and independent verifier. It
//! retains public proof bytes before checking the unchanged time target. Run
//! the optimized exact test under `/usr/bin/time -l` on macOS to measure RSS;
//! the private allocation ledger is not an observation of process memory.

use super::*;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest as _, Sha256};

fn retain_public_proof_v1(directory: &Path, proof: &[u8]) -> io::Result<PathBuf> {
    let digest = hex::encode(Sha256::digest(proof));
    let path = directory.join(format!("{digest}.x5s1"));
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(proof)?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if fs::read(&path)? != proof {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "existing proof artifact does not match its content digest",
                ));
            }
        }
        Err(error) => return Err(error),
    }
    Ok(path)
}

fn append_receipt_v1(path: &Path, public_record: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{public_record}")?;
    file.sync_all()
}

fn run_directory_v1(parent: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(parent)?;
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let directory = parent.join(format!(
        "iroha-x509-full-credential-{}-{tick}",
        std::process::id(),
    ));
    fs::create_dir(&directory)?;
    Ok(directory)
}

// Only the ignored complete-proof process uses this global hook. The hook
// covers Rayon worker assertions too; its failure outcome never forwards an
// internal payload to libtest or the retained public receipt.
fn catch_private_prover_panic_v1<T>(operation: impl FnOnce() -> T) -> Result<T, ()> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {
        let _ = writeln!(io::stderr().lock(), "X509 prover panicked; payload omitted");
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation));
    std::panic::set_hook(previous);
    result.map_err(|payload| {
        // Formatted assertions normally own a String. Other panic payloads are
        // not formatted, inspected, or propagated beyond this diagnostic.
        if let Ok(mut text) = payload.downcast::<String>() {
            zeroize::Zeroize::zeroize(&mut *text);
        }
    })
}

#[test]
fn private_prover_panic_is_redacted_in_its_isolated_process() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "privacy_engines::zk_x509::engine::prover_diagnostic::private_prover_panic_hook_subprocess",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(combined.contains("1 passed"));
    assert!(!combined.contains("SYNTHETIC_PRIVATE_PANIC_4e9314"));
    assert!(combined.contains("X509 prover panicked; payload omitted"));
    assert!(combined.contains("PUBLIC_RESTORED_PANIC_HOOK"));
}

#[test]
#[ignore = "isolated process helper for private-prover panic-hook coverage"]
fn private_prover_panic_hook_subprocess() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    assert!(
        catch_private_prover_panic_v1(|| {
            pool.install(|| panic!("{}", "SYNTHETIC_PRIVATE_PANIC_4e9314"));
        })
        .is_err()
    );
    assert!(std::panic::catch_unwind(|| panic!("PUBLIC_RESTORED_PANIC_HOOK")).is_err());
}

#[test]
fn public_proof_receipt_preserves_exact_bytes_and_rejects_existing_mismatch() {
    let directory = run_directory_v1(&std::env::temp_dir()).unwrap();
    let proof = b"public fixture for artifact-owner validation";
    let path = retain_public_proof_v1(&directory, proof).unwrap();
    assert_eq!(fs::read(&path).unwrap(), proof);
    assert_eq!(retain_public_proof_v1(&directory, proof).unwrap(), path);
    fs::write(&path, b"changed").unwrap();
    assert_eq!(
        retain_public_proof_v1(&directory, proof)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData,
    );
    let receipt = directory.join("receipt.txt");
    append_receipt_v1(&receipt, "producer=error").unwrap();
    append_receipt_v1(&receipt, "private_witness_recorded=false").unwrap();
    assert_eq!(
        fs::read_to_string(&receipt).unwrap(),
        "producer=error\nprivate_witness_recorded=false\n",
    );
    fs::remove_file(path).unwrap();
    fs::remove_file(receipt).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
#[ignore = "complete maximum-structural MAIN+CA credential proof; optimized build and external RSS measurement required"]
fn maximum_structural_credential_proof_with_retained_public_receipt() {
    use super::super::{
        profile::{
            ZK_X509_MAX_PROOF_BYTES_V1, ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1,
            ZK_X509_PROVER_ADDRESS_SPACE_CEILING_BYTES_V1, ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1,
            ZK_X509_PROVER_TARGET_SECONDS_V1,
        },
        relation::release_fixture::{
            build_zk_x509_release_fixture_v1, reference_statement_context_v1,
        },
    };

    assert!(
        !cfg!(debug_assertions),
        "run this full proof with --release"
    );
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Core crate is under the repository crates directory");
    // Keep actual proof evidence in the ignored repository output directory:
    // temporary-directory cleanup must not discard an expensive proof receipt.
    let directory = run_directory_v1(&repository.join("dist/zk-x509-prover-evidence"))
        .expect("persistent public diagnostic output directory");
    let receipt = directory.join("receipt.txt");
    let record = |text: String| {
        eprintln!("{text}");
        append_receipt_v1(&receipt, &text).expect("durable public diagnostic receipt");
    };
    record(format!(
        "output_directory={}\nprofile=complete49-MAIN-plus-compactCA\nproof_cap_bytes={ZK_X509_MAX_PROOF_BYTES_V1}\nencoded_geometry_bound_bytes={ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1}\nprover_target_seconds={ZK_X509_PROVER_TARGET_SECONDS_V1}\npeak_rss_limit_bytes={ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1}\naddress_space_limit_bytes={ZK_X509_PROVER_ADDRESS_SPACE_CEILING_BYTES_V1}\nrss_evidence=external-time-l-required\nactivation=unavailable\nprivate_witness_recorded=false",
        directory.display(),
    ));
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true)
        .expect("maximum structural release fixture");
    fixture.resource_shape.validate_v1().unwrap();
    assert_eq!(fixture.resource_shape.certificate_chain_depth, 3);
    assert_eq!(fixture.statement.disclosed_attributes.len(), 4);
    assert_eq!(fixture.crl_entry_count, 64);
    assert_eq!(fixture.resource_shape.maximum_serial_bytes, 20);
    record(format!("structural_shape={:?}", fixture.resource_shape));
    let witness = zeroize::Zeroizing::new(fixture.witness.encode_v1().unwrap());
    let genesis = *fixture.statement.context.network_id.as_bytes();
    let start = Instant::now();
    let produced = catch_private_prover_panic_v1(|| {
        prove_zk_x509_credential_proof_v1_with_rng(
            &fixture.statement,
            &fixture.authoritative_state,
            fixture.statement.presentation_not_before_unix_seconds * 1_000,
            &PrivacyConsensusLimitsV1::taira_default(),
            genesis,
            &witness,
            &mut rand::rngs::OsRng,
        )
    });
    let prove_elapsed = start.elapsed();
    let proof = match produced {
        Ok(Ok(proof)) => proof,
        Ok(Err(error)) => {
            record(format!(
                "producer=error\nproving_seconds={:.6}\nerror={error:?}",
                prove_elapsed.as_secs_f64(),
            ));
            panic!(
                "full credential producer failed; public receipt at {}",
                receipt.display()
            );
        }
        Err(()) => {
            // Preserve only the public outcome, including on stderr.
            record(format!(
                "producer=unwind\nproving_seconds={:.6}",
                prove_elapsed.as_secs_f64(),
            ));
            panic!(
                "full credential producer panicked; public receipt at {}",
                receipt.display()
            );
        }
    };
    let proof_path = retain_public_proof_v1(&directory, &proof).expect("public proof artifact");
    record(format!(
        "producer=success\nproof_path={}\nproof_bytes={}\nproof_sha256={}\nproving_seconds={:.6}\nproducer_self_check=passed",
        proof_path.display(),
        proof.len(),
        hex::encode(Sha256::digest(&proof)),
        prove_elapsed.as_secs_f64(),
    ));
    let replay_start = Instant::now();
    let replay = verify_zk_x509_credential_proof_v1(
        &fixture.statement,
        &fixture.authoritative_state,
        genesis,
        &proof,
    );
    record(format!(
        "independent_replay={replay:?}\nindependent_replay_seconds={:.6}",
        replay_start.elapsed().as_secs_f64(),
    ));
    replay.expect("independent complete credential verification");
    let mut wrong_genesis = genesis;
    wrong_genesis[0] ^= 1;
    assert!(
        verify_zk_x509_credential_proof_v1(
            &fixture.statement,
            &fixture.authoritative_state,
            wrong_genesis,
            &proof,
        )
        .is_err()
    );
    let mut tampered = proof.clone();
    *tampered.last_mut().expect("nonempty credential proof") ^= 1;
    assert!(
        verify_zk_x509_credential_proof_v1(
            &fixture.statement,
            &fixture.authoritative_state,
            genesis,
            &tampered,
        )
        .is_err()
    );
    record(format!(
        "wrong_genesis_rejected=true\ntampered_proof_rejected=true\ntime_target_met={}\nfull_release_qualification=false",
        prove_elapsed.as_secs_f64() <= ZK_X509_PROVER_TARGET_SECONDS_V1 as f64,
    ));
    assert!(proof.len() <= ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize);
    assert!(proof.len() <= ZK_X509_MAX_PROOF_BYTES_V1 as usize);
    assert!(
        prove_elapsed.as_secs_f64() <= ZK_X509_PROVER_TARGET_SECONDS_V1 as f64,
        "complete proof exceeds unchanged proving target; verified public proof and receipt retained at {}",
        directory.display(),
    );
}

#[test]
fn canonical_witness_round_trip_clears_its_owned_bytes_on_match_and_mismatch() {
    use super::super::private_table::inspection::observe_v1;
    let fixture = super::super::relation::tests::fixture();
    let encoded = zeroize::Zeroizing::new(fixture.witness.encode_v1().unwrap());
    for mismatch in [false, true] {
        let mut supplied = encoded.clone();
        if mismatch {
            supplied[0] ^= 1;
        }
        let (result, observations) =
            observe_v1(|| validate_witness_round_trip_v1(&fixture.witness, &supplied));
        assert_eq!(result.is_err(), mismatch);
        assert!(observations.iter().any(|row| {
            row.cells == encoded.len() && row.nonzero_before > 0 && row.nonzero_after == 0
        }));
        assert!(observations.iter().all(|row| row.nonzero_after == 0));
        assert_eq!(supplied[0], encoded[0] ^ u8::from(mismatch));
    }
}
