use super::*;
use crate::tui;
use clap::{ValueEnum, builder::PossibleValue};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
use std::path::PathBuf;
use zeroize::Zeroizing;
/// Use `Kagami` to generate cryptographic key-pairs.
#[derive(ClapArgs)]
pub struct Args {
    /// An algorithm to use for the key-pair generation
    #[clap(default_value_t, long, short)]
    algorithm: AlgorithmArg,
    /// A 32-byte secret key-generation seed encoded as 64 hexadecimal characters.
    ///
    /// This is for reproducible fixtures. Omit it for OS-random production keys.
    #[clap(long = "seed-hex", value_name = "HEX")]
    seed: Option<String>,
    /// Write the key pair into a new owner-only custody directory.
    ///
    /// The directory must be fresh. The complete key pair publishes atomically.
    /// Files are written
    /// as `public.key` and `private.key`; `--pop` also writes `pop.hex`. The
    /// private key never passes through standard output.
    #[clap(long, value_name = "DIR")]
    out_dir: PathBuf,
    /// Also output a BLS Proof-of-Possession (PoP) for this key (BLS-normal only).
    /// Written as `pop.hex` in the custody directory.
    #[clap(long)]
    pop: bool,
}
#[derive(Clone, Debug, Default, derive_more::Display)]
struct AlgorithmArg(Algorithm);
impl ValueEnum for AlgorithmArg {
    fn value_variants<'a>() -> &'a [Self] {
        // Keep in sync with `Algorithm`; coverage is enforced by a unit test.
        const VARIANTS: &[AlgorithmArg] = &[
            AlgorithmArg(Algorithm::Ed25519),
            AlgorithmArg(Algorithm::Secp256k1),
            AlgorithmArg(Algorithm::MlDsa),
            #[cfg(feature = "gost")]
            AlgorithmArg(Algorithm::Gost3410_2012_256ParamSetA),
            #[cfg(feature = "gost")]
            AlgorithmArg(Algorithm::Gost3410_2012_256ParamSetB),
            #[cfg(feature = "gost")]
            AlgorithmArg(Algorithm::Gost3410_2012_256ParamSetC),
            #[cfg(feature = "gost")]
            AlgorithmArg(Algorithm::Gost3410_2012_512ParamSetA),
            #[cfg(feature = "gost")]
            AlgorithmArg(Algorithm::Gost3410_2012_512ParamSetB),
            AlgorithmArg(Algorithm::BlsNormal),
            AlgorithmArg(Algorithm::BlsSmall),
        ];
        VARIANTS
    }
    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(self.0.as_static_str().into())
    }
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self {
            algorithm,
            seed,
            out_dir,
            pop,
        } = self;
        let algorithm_name = algorithm.to_string();
        tui::status(format!("Generating {algorithm_name} key pair"));
        let key_pair = key_pair_from_source(algorithm.0, seed)?;
        let exposed_private_key = ExposedPrivateKey(key_pair.private_key().clone());
        let pop_hex = if pop {
            let public_algorithm = key_pair
                .public_key()
                .try_algorithm()
                .wrap_err("generated public key is malformed")?;
            if public_algorithm != Algorithm::BlsNormal {
                color_eyre::eyre::bail!(
                    "--pop requires --algorithm bls_normal (validator consensus key)"
                );
            }
            let pop = iroha_crypto::bls_normal_pop_prove(key_pair.private_key())
                .wrap_err("failed to construct PoP for BLS-normal key")?;
            Some(hex::encode(pop))
        } else {
            None
        };
        write_key_custody(
            writer,
            &out_dir,
            key_pair.public_key(),
            &exposed_private_key,
            pop_hex.as_deref(),
        )?;
        tui::success(format!("{algorithm_name} key pair ready"));
        Ok(())
    }
}
const PUBLIC_KEY_FILE: &str = "public.key";
const PRIVATE_KEY_FILE: &str = "private.key";
const POP_FILE: &str = "pop.hex";
fn write_key_custody<T: Write>(
    writer: &mut BufWriter<T>,
    out_dir: &std::path::Path,
    public_key: &iroha_crypto::PublicKey,
    private_key: &ExposedPrivateKey,
    pop_hex: Option<&str>,
) -> Outcome {
    let mut public_record = public_key.to_string();
    public_record.push('\n');
    let pop_record = pop_hex.map(|pop| format!("{pop}\n"));
    let canonical_private = Zeroizing::new(
        private_key
            .try_to_multihash_string()
            .wrap_err("encode private key")?,
    );
    let mut private_record = Zeroizing::new(Vec::with_capacity(canonical_private.len() + 1));
    private_record.extend_from_slice(canonical_private.as_bytes());
    private_record.push(b'\n');
    let out_dir = crate::atomic_output::resolve_output_file(out_dir)
        .wrap_err("prepare key custody directory")?;
    let parent = iroha_fs::OwnerDirectory::open(
        out_dir
            .parent()
            .expect("resolved custody path has a parent"),
    )
    .wrap_err("prepare key custody directory")?;
    let mut records = vec![
        (PUBLIC_KEY_FILE, public_record.as_bytes()),
        (PRIVATE_KEY_FILE, private_record.as_slice()),
    ];
    if let Some(pop_record) = &pop_record {
        records.push((POP_FILE, pop_record.as_bytes()));
    }
    let custody = parent
        .publish_private_child(
            out_dir.file_name().expect("resolved custody path has a name"),
            &records,
        )
        .wrap_err("publish complete key custody directory; reconcile the exact destination before retrying")?;
    let public_path = custody.path().join(PUBLIC_KEY_FILE);
    let private_path = custody.path().join(PRIVATE_KEY_FILE);
    writeln!(writer, "public_key_file: {}", public_path.display())?;
    writeln!(writer, "private_key_file: {}", private_path.display())?;
    if pop_record.is_some() {
        let pop_path = custody.path().join(POP_FILE);
        writeln!(writer, "pop_file: {}", pop_path.display())?;
    }
    Ok(())
}
fn key_pair_from_source(algorithm: Algorithm, seed: Option<String>) -> color_eyre::Result<KeyPair> {
    let seed = seed.map(Zeroizing::new);
    let key_pair = match seed.as_ref() {
        None => KeyPair::try_random_with_algorithm(algorithm)
            .wrap_err("Failed to generate random key pair")?,
        Some(seed) => {
            let mut seed = parse_keygen_seed_hex(seed.as_str())?;
            KeyPair::try_from_seed(std::mem::take(&mut *seed), algorithm)
                .wrap_err("Failed to derive seeded key pair")?
        }
    };
    Ok(key_pair)
}
pub fn parse_keygen_seed_hex(seed: &str) -> color_eyre::Result<Zeroizing<Vec<u8>>> {
    let seed = seed.strip_prefix("0x").unwrap_or(seed);
    if seed.len() != 64 {
        color_eyre::eyre::bail!(
            "key-generation seed must be exactly 32 bytes encoded as 64 hexadecimal characters"
        );
    }
    let mut decoded = Zeroizing::new(vec![0u8; 32]);
    hex::decode_to_slice(seed, decoded.as_mut_slice())
        .wrap_err("key-generation seed must contain exactly 64 hexadecimal characters")?;
    Ok(decoded)
}
#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, io::BufWriter};
    // Bring `ValueEnum` into scope so `AlgorithmArg::value_variants()` is callable in this module.
    use super::{
        Algorithm, AlgorithmArg, Args, ExposedPrivateKey, KeyPair, RunArgs, key_pair_from_source,
        parse_keygen_seed_hex,
    };
    use clap::ValueEnum;
    #[test]
    fn algorithm_arg_displays_as_algorithm() {
        assert_eq!(
            format!("{}", AlgorithmArg(Algorithm::Ed25519)),
            format!("{}", Algorithm::Ed25519)
        )
    }
    #[test]
    fn value_variants_covers_all_algorithms() {
        // Names advertised by clap for AlgorithmArg
        let variants: BTreeSet<&'static str> = AlgorithmArg::value_variants()
            .iter()
            .map(|a| a.0.as_static_str())
            .collect();
        // Expected algorithms derived from Algorithm::from_str availability.
        // Avoid direct references to feature-gated variants to keep the test robust across features.
        let mut expected = BTreeSet::new();
        expected.insert("ed25519");
        expected.insert("secp256k1");
        if "bls_normal".parse::<Algorithm>().is_ok() {
            expected.insert("bls_normal");
        }
        if "bls_small".parse::<Algorithm>().is_ok() {
            expected.insert("bls_small");
        }
        if "ml-dsa".parse::<Algorithm>().is_ok() {
            expected.insert("ml-dsa");
        }
        for gost in &[
            "gost3410-2012-256-paramset-a",
            "gost3410-2012-256-paramset-b",
            "gost3410-2012-256-paramset-c",
            "gost3410-2012-512-paramset-a",
            "gost3410-2012-512-paramset-b",
        ] {
            if gost.parse::<Algorithm>().is_ok() {
                expected.insert(*gost);
            }
        }
        assert_eq!(
            variants, expected,
            "AlgorithmArg::value_variants is out of sync with Algorithm"
        );
    }
    #[cfg(unix)]
    #[test]
    fn out_dir_writes_consistent_owner_only_custody_and_refuses_reuse() {
        use std::{fs, os::unix::fs::PermissionsExt as _, str::FromStr as _};
        let sandbox = tempfile::tempdir().expect("create key custody sandbox");
        let sandbox_root = fs::canonicalize(sandbox.path()).expect("canonical custody sandbox");
        let out_dir = sandbox_root.join("custody");
        let args = Args {
            algorithm: AlgorithmArg(Algorithm::Ed25519),
            seed: Some("42".repeat(32)),
            out_dir: out_dir.clone(),
            pop: false,
        };
        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("write key custody directory");
        let output = String::from_utf8(writer.into_inner().expect("flush custody summary"))
            .expect("custody summary is UTF-8");
        let public_record =
            fs::read_to_string(out_dir.join(super::PUBLIC_KEY_FILE)).expect("read public key");
        let private_record =
            fs::read_to_string(out_dir.join(super::PRIVATE_KEY_FILE)).expect("read private key");
        let exposed_private =
            ExposedPrivateKey::from_str(private_record.trim_end()).expect("parse private key");
        let reconstructed = KeyPair::from_private_key(exposed_private.0.clone())
            .expect("derive matching public key");
        assert_eq!(public_record, format!("{}\n", reconstructed.public_key()));
        assert_eq!(private_record, format!("{exposed_private}\n"));
        assert!(!output.contains(private_record.trim_end()));
        assert!(output.contains("public_key_file:"));
        assert!(output.contains("private_key_file:"));
        assert_eq!(
            fs::metadata(&out_dir)
                .expect("custody directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        for name in [super::PUBLIC_KEY_FILE, super::PRIVATE_KEY_FILE] {
            assert_eq!(
                fs::metadata(out_dir.join(name))
                    .expect("custody file metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let error = Args {
            algorithm: AlgorithmArg(Algorithm::Ed25519),
            seed: Some("42".repeat(32)),
            out_dir,
            pop: false,
        }
        .run(&mut BufWriter::new(Vec::new()))
        .expect_err("existing custody directory must never be reused");
        assert!(error.to_string().contains("prepare key custody directory"));
    }
    #[test]
    fn key_custody_refuses_existing_empty_destination() {
        let root = tempfile::tempdir().unwrap();
        let custody = root.path().join("custody");
        std::fs::create_dir(&custody).unwrap();
        let mut writer = BufWriter::new(Vec::new());
        assert!(
            Args {
                algorithm: AlgorithmArg(Algorithm::Ed25519),
                seed: Some("42".repeat(32)),
                out_dir: custody.clone(),
                pop: false,
            }
            .run(&mut writer)
            .is_err()
        );
        assert_eq!(std::fs::read_dir(custody).unwrap().count(), 0);
        assert!(writer.into_inner().unwrap().is_empty());
    }
    #[test]
    fn concurrent_key_generation_publishes_one_complete_original_identity() {
        let root = tempfile::tempdir().unwrap();
        let custody = root.path().join("custody");
        let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
        let contenders = ["42", "43"].map(|seed| {
            let gate = gate.clone();
            let custody = custody.clone();
            std::thread::spawn(move || {
                gate.wait();
                let mut writer = BufWriter::new(Vec::new());
                let result = Args {
                    algorithm: AlgorithmArg(Algorithm::Ed25519),
                    seed: Some(seed.repeat(32)),
                    out_dir: custody,
                    pop: false,
                }
                .run(&mut writer);
                (result.is_ok(), writer.into_inner().unwrap())
            })
        });
        let results = contenders.map(|thread| thread.join().unwrap());
        assert_eq!(results.iter().filter(|(success, _)| *success).count(), 1);
        assert!(
            results
                .iter()
                .all(|(success, output)| *success || output.is_empty())
        );
        let public = iroha_fs::read_regular(custody.join(super::PUBLIC_KEY_FILE), 4096).unwrap();
        let private = iroha_fs::read_private(custody.join(super::PRIVATE_KEY_FILE), 4096).unwrap();
        let exposed: ExposedPrivateKey = std::str::from_utf8(&private)
            .unwrap()
            .strip_suffix('\n')
            .unwrap()
            .parse()
            .unwrap();
        let key_pair = KeyPair::from_private_key(exposed.0).unwrap();
        assert_eq!(
            public.as_slice(),
            format!("{}\n", key_pair.public_key()).as_bytes()
        );
        assert_eq!(std::fs::read_dir(custody).unwrap().count(), 2);
    }
    #[test]
    fn validator_key_and_pop_publish_together_and_verify() {
        let root = tempfile::tempdir().unwrap();
        let custody = root.path().join("custody");
        Args {
            algorithm: AlgorithmArg(Algorithm::BlsNormal),
            seed: Some("42".repeat(32)),
            out_dir: custody.clone(),
            pop: true,
        }
        .run(&mut BufWriter::new(Vec::new()))
        .unwrap();
        let public = iroha_fs::read_regular(custody.join(super::PUBLIC_KEY_FILE), 4096).unwrap();
        let pop = iroha_fs::read_regular(custody.join(super::POP_FILE), 4096).unwrap();
        let public = std::str::from_utf8(&public)
            .unwrap()
            .trim_end()
            .parse()
            .unwrap();
        let pop = hex::decode(std::str::from_utf8(&pop).unwrap().trim_end()).unwrap();
        iroha_crypto::bls_normal_pop_verify(&public, &pop).unwrap();
        assert!(iroha_fs::read_private(custody.join(super::PRIVATE_KEY_FILE), 4096).is_ok());
        assert_eq!(std::fs::read_dir(custody).unwrap().count(), 3);
    }
    #[test]
    fn key_pair_random_path_uses_checked_generation() {
        let key_pair =
            key_pair_from_source(Algorithm::Ed25519, None).expect("checked random keypair");
        assert_eq!(key_pair.algorithm(), Algorithm::Ed25519);
    }
    #[test]
    fn seeded_key_generation_requires_exact_secret_hex() {
        let err = parse_keygen_seed_hex("human password")
            .expect_err("human-readable seed must be rejected");
        assert!(err.to_string().contains("exactly 32 bytes"));
        let err = parse_keygen_seed_hex(&format!("{}zz", "a5".repeat(31)))
            .expect_err("invalid exact-length hex must be rejected after partial decoding");
        assert!(err.to_string().contains("hexadecimal characters"));
        assert_eq!(
            parse_keygen_seed_hex(&"a5".repeat(32))
                .expect("32-byte hex seed")
                .len(),
            32
        );
    }
}
