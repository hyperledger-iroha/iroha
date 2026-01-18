use clap::{ArgGroup, ValueEnum, builder::PossibleValue};
use color_eyre::eyre::WrapErr as _;
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PrivateKey};

use super::*;
use crate::tui;

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
    /// The Unicode `seed` string to generate the key-pair from
    #[clap(long, short, group = "generate_from")]
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
        let key_pair = self.clone().key_pair()?;
        let exposed_private_key = ExposedPrivateKey(key_pair.private_key().clone());
        let pop_hex = if self.pop {
            if key_pair.public_key().algorithm() != Algorithm::BlsNormal {
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

        if json {
            if json_mh_prefixed {
                #[derive(crate::json_macros::JsonSerialize)]
                struct KeyPairStrings {
                    public_key: String,
                    private_key: String,
                    #[norito(skip_serializing_if = "Option::is_none")]
                    pop_hex: Option<String>,
                }
                let pk_str = key_pair.public_key().to_prefixed_string();
                let sk_str = exposed_private_key.to_prefixed_string();
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
            (None, None) => KeyPair::random_with_algorithm(algorithm),
            (None, Some(private_key_hex)) => {
                let private_key = PrivateKey::from_hex(algorithm, private_key_hex)
                    .wrap_err("Failed to decode private key")?;
                KeyPair::from(private_key)
            }
            (Some(seed), None) => {
                let seed: Vec<u8> = seed.as_bytes().into();
                KeyPair::from_seed(seed, algorithm)
            }
            _ => unreachable!("Clap group invariant"),
        };

        Ok(key_pair)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    // Bring `ValueEnum` into scope so `AlgorithmArg::value_variants()` is callable in this module.
    use clap::ValueEnum;

    use super::{Algorithm, AlgorithmArg};

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
}
