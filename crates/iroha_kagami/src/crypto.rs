use super::*;
use crate::tui;
use clap::{ArgGroup, ValueEnum, builder::PossibleValue};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PrivateKey};
use std::path::PathBuf;
use zeroize::Zeroizing;
/// Use `Kagami` to generate cryptographic key-pairs.
#[derive(ClapArgs, Debug, Clone)]
#[command(group = ArgGroup::new("generate_from").required(false))]
#[command(group = ArgGroup::new("format").required(false))]
pub struct Args {
    /// An algorithm to use for the key-pair generation
    #[clap(default_value_t, long, short)]
    algorithm: AlgorithmArg,
    /// A private key to generate the key-pair from
    ///
    /// `--private-key` specifies the payload of the private key, while `--algorithm`
    /// specifies its algorithm.
    #[clap(long, short, group = "generate_from")]
    private_key: Option<String>,
    /// A 32-byte secret key-generation seed encoded as 64 hexadecimal characters.
    ///
    /// This is for reproducible fixtures. Omit it for OS-random production keys.
    #[clap(long = "seed-hex", group = "generate_from", value_name = "HEX")]
    seed: Option<String>,
    /// Output the key-pair in JSON format
    #[clap(long, short, group = "format")]
    json: bool,
    /// Use algorithm-prefixed multihash strings in JSON (e.g., "ml-dsa:...")
    #[clap(long)]
    json_mh_prefixed: bool,
    /// Output the key-pair without additional text
    #[clap(long, short, group = "format")]
    compact: bool,
    /// Write the key pair into a new owner-only custody directory.
    ///
    /// The directory must not contain any existing entries. Files are written
    /// as `public.key` and `private.key`; `--pop` also writes `pop.hex`. The
    /// private key never passes through standard output.
    #[clap(long, value_name = "DIR", group = "format")]
    out_dir: Option<PathBuf>,
    /// Also output a BLS Proof-of-Possession (PoP) for this key (BLS-normal only).
    /// Printed as hex in JSON or plain hex in compact mode.
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
            #[cfg(feature = "bls")]
            AlgorithmArg(Algorithm::BlsNormal),
            #[cfg(feature = "bls")]
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
        let algorithm_name = self.algorithm.to_string();
        tui::status(format!("Generating {algorithm_name} key pair"));
        let json = self.json;
        let json_mh_prefixed = self.json_mh_prefixed;
        let compact = self.compact;
        let out_dir = self.out_dir.clone();
        let key_pair = self.clone().key_pair()?;
        let exposed_private_key = ExposedPrivateKey(key_pair.private_key().clone());
        let pop_hex = if self.pop {
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
            Some(encode_hex(&pop))
        } else {
            None
        };
        if let Some(out_dir) = out_dir {
            write_key_custody(
                writer,
                &out_dir,
                key_pair.public_key(),
                &exposed_private_key,
                pop_hex.as_deref(),
            )?;
        } else if json {
            if json_mh_prefixed {
                #[derive(crate::json_macros::JsonSerialize)]
                struct KeyPairStrings {
                    public_key: String,
                    private_key: String,
                    #[norito(skip_serializing_if = "Option::is_none")]
                    pop_hex: Option<String>,
                }
                let pk_str = key_pair
                    .public_key()
                    .try_to_prefixed_string()
                    .wrap_err("generated public key is malformed")?;
                let sk_str = exposed_private_key
                    .try_to_prefixed_string()
                    .wrap_err("generated private key is malformed")?;
                let payload = KeyPairStrings {
                    public_key: pk_str,
                    private_key: sk_str,
                    pop_hex: pop_hex.clone(),
                };
                let output = norito::json::to_json_pretty(&payload)
                    .wrap_err("Failed to serialise to JSON.")?;
                writeln!(writer, "{output}")?;
            } else {
                #[derive(crate::json_macros::JsonSerialize)]
                pub struct ExposedKeyPair {
                    public_key: String,
                    private_key: ExposedPrivateKey,
                    #[norito(skip_serializing_if = "Option::is_none")]
                    pop_hex: Option<String>,
                }
                let exposed_key_pair = ExposedKeyPair {
                    public_key: key_pair.public_key().to_string(),
                    private_key: exposed_private_key,
                    pop_hex: pop_hex.clone(),
                };
                let output = norito::json::to_json_pretty(&exposed_key_pair)
                    .wrap_err("Failed to serialise to JSON.")?;
                writeln!(writer, "{output}")?;
            }
        } else if compact {
            writeln!(writer, "{}", &key_pair.public_key())?;
            writeln!(writer, "{}", &exposed_private_key)?;
            if let Some(pop_hex) = pop_hex.as_deref() {
                writeln!(writer, "{pop_hex}")?;
            }
        } else {
            writeln!(
                writer,
                "Public key (multihash): \"{}\"",
                &key_pair.public_key()
            )?;
            writeln!(
                writer,
                "Private key (multihash): \"{}\"",
                &exposed_private_key
            )?;
            if let Some(pop_hex) = pop_hex.as_deref() {
                writeln!(writer, "PoP (hex): \"{}\"", pop_hex)?;
            }
        }
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
    crate::secure_fs::prepare_empty_private_directory(out_dir)
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
fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
impl Args {
    fn key_pair(self) -> color_eyre::Result<KeyPair> {
        let algorithm = self.algorithm.0;
        let key_pair = match (self.seed, self.private_key) {
            (None, None) => KeyPair::try_random_with_algorithm(algorithm)
                .wrap_err("Failed to generate random key pair")?,
            (None, Some(private_key_hex)) => {
                let private_key = PrivateKey::from_hex(algorithm, private_key_hex)
                    .wrap_err("Failed to decode private key")?;
                KeyPair::from_private_key(private_key)
                    .wrap_err("Failed to derive key pair from private key")?
            }
            (Some(seed), None) => {
                let seed = parse_keygen_seed_hex(&seed)?;
                KeyPair::try_from_seed(seed, algorithm)
                    .wrap_err("Failed to derive seeded key pair")?
            }
            _ => unreachable!("Clap group invariant"),
        };
        Ok(key_pair)
    }
}
pub fn parse_keygen_seed_hex(seed: &str) -> color_eyre::Result<Vec<u8>> {
    let seed = seed.strip_prefix("0x").unwrap_or(seed);
    if seed.len() != 64 {
        color_eyre::eyre::bail!(
            "key-generation seed must be exactly 32 bytes encoded as 64 hexadecimal characters"
        );
    }
    let mut decoded = vec![0u8; 32];
    hex::decode_to_slice(seed, &mut decoded)
        .wrap_err("key-generation seed must contain exactly 64 hexadecimal characters")?;
    Ok(decoded)
}
#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, io::BufWriter};
    // Bring `ValueEnum` into scope so `AlgorithmArg::value_variants()` is callable in this module.
    use super::{
        Algorithm, AlgorithmArg, Args, ExposedPrivateKey, KeyPair, RunArgs, parse_keygen_seed_hex,
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
    #[test]
    fn json_prefixed_output_uses_checked_formatters() {
        let args = Args {
            algorithm: AlgorithmArg(Algorithm::Ed25519),
            private_key: None,
            seed: Some("11".repeat(32)),
            json: true,
            json_mh_prefixed: true,
            compact: false,
            out_dir: None,
            pop: false,
        };
        let mut writer = BufWriter::new(Vec::new());
        args.clone()
            .run(&mut writer)
            .expect("generate keypair JSON");
        let output = String::from_utf8(writer.into_inner().expect("writer flush")).expect("utf8");
        let value: norito::json::Value = norito::json::from_str(&output).expect("json output");
        let keypair = args.key_pair().expect("expected keypair");
        let expected_public = keypair
            .public_key()
            .try_to_prefixed_string()
            .expect("checked public formatter");
        let expected_private = ExposedPrivateKey(keypair.private_key().clone())
            .try_to_prefixed_string()
            .expect("checked private formatter");
        assert_eq!(
            value
                .get("public_key")
                .and_then(norito::json::Value::as_str),
            Some(expected_public.as_str())
        );
        assert_eq!(
            value
                .get("private_key")
                .and_then(norito::json::Value::as_str),
            Some(expected_private.as_str())
        );
    }
    #[cfg(unix)]
    #[test]
    fn out_dir_writes_consistent_owner_only_custody_and_refuses_reuse() {
        use std::{fs, os::unix::fs::PermissionsExt as _, str::FromStr as _};
        let sandbox = tempfile::tempdir().expect("create key custody sandbox");
        let out_dir = sandbox.path().join("custody");
        let args = Args {
            algorithm: AlgorithmArg(Algorithm::Ed25519),
            private_key: None,
            seed: Some("42".repeat(32)),
            json: false,
            json_mh_prefixed: false,
            compact: false,
            out_dir: Some(out_dir.clone()),
            pop: false,
        };
        let mut writer = BufWriter::new(Vec::new());
        args.clone()
            .run(&mut writer)
            .expect("write key custody directory");
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
        let error = args
            .run(&mut BufWriter::new(Vec::new()))
            .expect_err("existing custody directory must never be reused");
        assert!(error.to_string().contains("prepare key custody directory"));
    }
    #[test]
    fn key_pair_random_path_uses_checked_generation() {
        let args = Args {
            algorithm: AlgorithmArg(Algorithm::Ed25519),
            private_key: None,
            seed: None,
            json: false,
            json_mh_prefixed: false,
            compact: false,
            out_dir: None,
            pop: false,
        };
        let key_pair = args.key_pair().expect("checked random keypair");
        assert_eq!(key_pair.algorithm(), Algorithm::Ed25519);
    }
    #[test]
    fn seeded_key_generation_requires_exact_secret_hex() {
        let err = parse_keygen_seed_hex("human password")
            .expect_err("human-readable seed must be rejected");
        assert!(err.to_string().contains("exactly 32 bytes"));
        assert_eq!(
            parse_keygen_seed_hex(&"a5".repeat(32))
                .expect("32-byte hex seed")
                .len(),
            32
        );
    }
}
