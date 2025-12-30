use std::io::Write as _;

use clap::Args as ClapArgs;
use color_eyre::eyre::WrapErr as _;
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PrivateKey};

use crate::{Outcome, RunArgs, tui};

/// Produce a BLS-normal Proof-of-Possession (PoP) for a validator key.
#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Algorithm to use; must be `bls_normal` for consensus PoP
    #[clap(long, default_value = "bls_normal")]
    algorithm: String,
    /// Private key hex (multihash payload, not prefixed)
    #[clap(long, conflicts_with = "seed")]
    private_key: Option<String>,
    /// Seed string to derive the key pair (for testing)
    #[clap(long, conflicts_with = "private_key")]
    seed: Option<String>,
    /// Output JSON instead of plain text
    #[clap(long)]
    json: bool,
    /// Print the private key in plain-text output (disabled by default)
    #[clap(long)]
    expose_private_key: bool,
}

impl<T: std::io::Write> RunArgs<T> for Args {
    fn run(self, writer: &mut std::io::BufWriter<T>) -> Outcome {
        tui::status("Producing validator Proof-of-Possession");
        let alg: Algorithm = self
            .algorithm
            .parse()
            .wrap_err("invalid algorithm; expected bls_normal")?;
        if alg != Algorithm::BlsNormal {
            color_eyre::eyre::bail!("PoP requires --algorithm bls_normal");
        }

        let (kp, generated) = match (self.private_key, self.seed) {
            (Some(sk_hex), None) => {
                let sk = PrivateKey::from_hex(alg, sk_hex).wrap_err("decode private key")?;
                (KeyPair::from(sk), false)
            }
            (None, Some(seed)) => (KeyPair::from_seed(seed.into_bytes(), alg), false),
            (None, None) => (KeyPair::random_with_algorithm(alg), true),
            _ => unreachable!("clap conflicts"),
        };

        let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key()).wrap_err("prove pop")?;
        let pop_hex = encode_hex(&pop);
        let expose_private_key = self.expose_private_key || generated;

        if self.json {
            #[derive(crate::json_macros::JsonSerialize)]
            struct Payload {
                public_key: String,
                pop_hex: String,
            }
            let payload = Payload {
                public_key: kp.public_key().to_string(),
                pop_hex,
            };
            let out = norito::json::to_json_pretty(&payload).wrap_err("serialize json")?;
            writeln!(writer, "{out}")?;
        } else {
            writeln!(writer, "public_key: {}", kp.public_key())?;
            writeln!(writer, "pop_hex: {}", pop_hex)?;
            if expose_private_key {
                writeln!(
                    writer,
                    "private_key (multihash): {}",
                    ExposedPrivateKey(kp.private_key().clone())
                )?;
            }
        }
        tui::success("Proof-of-Possession ready");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufWriter, Write},
        str::FromStr,
    };

    use super::*;

    #[test]
    fn pop_json_parses_and_verifies() {
        // Build args to emit JSON PoP using a seed
        let args = Args {
            algorithm: "bls_normal".to_string(),
            private_key: None,
            seed: Some("unit-seed".to_string()),
            json: true,
            expose_private_key: false,
        };
        let mut buf = Vec::new();
        {
            let mut bw = BufWriter::new(&mut buf);
            args.run(&mut bw).expect("run pop json");
            bw.flush().unwrap();
        }
        let s = String::from_utf8(buf).expect("utf8");
        let v: norito::json::Value = norito::json::from_str(&s).expect("json");
        let pk_s = v["public_key"].as_str().expect("public_key");
        let pop_hex = v["pop_hex"].as_str().expect("pop_hex");
        // Parse and verify PoP using crypto helpers
        let pk = iroha_crypto::PublicKey::from_str(pk_s).expect("pk parse");
        let pop = decode_hex(pop_hex.trim_start_matches("0x")).expect("pop hex");
        iroha_crypto::bls_normal_pop_verify(&pk, &pop).expect("pop verify");
    }

    #[test]
    fn pop_plaintext_omits_private_key_without_flag() {
        let args = Args {
            algorithm: "bls_normal".to_string(),
            private_key: None,
            seed: Some("omit-seed".to_string()),
            json: false,
            expose_private_key: false,
        };
        let mut buf = Vec::new();
        {
            let mut bw = BufWriter::new(&mut buf);
            args.run(&mut bw).expect("run pop text omit");
            bw.flush().unwrap();
        }
        let out = String::from_utf8(buf).expect("utf8");
        assert!(out.contains("public_key:"));
        assert!(out.contains("pop_hex:"));
        assert!(!out.contains("private_key (multihash):"));
    }

    #[test]
    fn pop_plaintext_includes_private_key_with_flag() {
        let args = Args {
            algorithm: "bls_normal".to_string(),
            private_key: None,
            seed: Some("expose-seed".to_string()),
            json: false,
            expose_private_key: true,
        };
        let mut buf = Vec::new();
        {
            let mut bw = BufWriter::new(&mut buf);
            args.run(&mut bw).expect("run pop text expose");
            bw.flush().unwrap();
        }
        let out = String::from_utf8(buf).expect("utf8");
        assert!(out.contains("private_key (multihash):"));
    }

    #[test]
    fn pop_plaintext_includes_private_key_when_generated() {
        let args = Args {
            algorithm: "bls_normal".to_string(),
            private_key: None,
            seed: None,
            json: false,
            expose_private_key: false,
        };
        let mut buf = Vec::new();
        {
            let mut bw = BufWriter::new(&mut buf);
            args.run(&mut bw).expect("run pop text generated");
            bw.flush().unwrap();
        }
        let out = String::from_utf8(buf).expect("utf8");
        assert!(out.contains("private_key (multihash):"));
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

#[cfg(test)]
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    let s = s.strip_prefix("0x").unwrap_or(s);
    if !s.len().is_multiple_of(2) {
        return Err("odd hex length".into());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let h = from_hex_nibble(bytes[i])?;
        let l = from_hex_nibble(bytes[i + 1])?;
        out.push((h << 4) | l);
    }
    Ok(out)
}

#[cfg(test)]
fn from_hex_nibble(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err("invalid hex digit".into()),
    }
}
