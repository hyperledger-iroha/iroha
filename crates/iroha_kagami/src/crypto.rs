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
    /// The directory must not contain any existing entries. Files are written
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
    let out_dir = crate::secure_fs::prepare_empty_private_directory(out_dir)
        .wrap_err("prepare key custody directory")?;
    let public_path = out_dir.join(PUBLIC_KEY_FILE);
    let mut public_record = public_key.to_string();
    public_record.push('\n');
    crate::secure_fs::write_private_file_atomic(&public_path, public_record.as_bytes())
        .wrap_err("write public-key custody file")?;
    let pop_path = pop_hex
        .map(|pop_hex| -> color_eyre::Result<_> {
            let path = out_dir.join(POP_FILE);
            let mut pop_record = pop_hex.to_owned();
            pop_record.push('\n');
            crate::secure_fs::write_private_file_atomic(&path, pop_record.as_bytes())
                .wrap_err("write proof-of-possession custody file")?;
            Ok(path)
        })
        .transpose()?;
    let private_path = out_dir.join(PRIVATE_KEY_FILE);
    let canonical_private = Zeroizing::new(
        private_key
            .try_to_multihash_string()
            .wrap_err("encode private key")?,
    );
    let mut private_record = Zeroizing::new(Vec::with_capacity(canonical_private.len() + 1));
    private_record.extend_from_slice(canonical_private.as_bytes());
    private_record.push(b'\n');
    crate::secure_fs::write_private_file_atomic(&private_path, private_record.as_slice())
        .wrap_err("write private-key custody file")?;
    writeln!(writer, "public_key_file: {}", public_path.display())?;
    writeln!(writer, "private_key_file: {}", private_path.display())?;
    if let Some(pop_path) = pop_path {
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
