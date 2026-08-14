//! Re-sign a Norito-framed genesis block with the configured genesis key.
use eyre::{Result, eyre};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    block::SignedBlock,
    confidential::{CONFIDENTIAL_RULES_VERSION, ConfidentialFeatureDigest},
};
use std::{
    env,
    fs::{self, File},
    io::Read as _,
    path::{Path, PathBuf},
};
const MAX_GENESIS_PRIVATE_KEY_FILE_BYTES: usize = 4 * 1024;
fn main() -> Result<()> {
    iroha_genesis::init_instruction_registry();
    let args = Args::parse()?;
    let key_pair = args.load_key_pair()?;
    let block = iroha_genesis::read_signed_genesis(&args.input)?;
    let resigned = resign_block(&block, &key_pair, args.zk_policy_hash)?;
    let framed = resigned.encode_wire()?;
    if framed.len() > iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1 {
        return Err(eyre!(
            "re-signed genesis body is {} bytes, exceeding the {}-byte first-release limit",
            framed.len(),
            iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1
        ));
    }
    fs::write(&args.output, framed)?;
    println!(
        "event=genesis_resign input={} output={} signer={} old_signatures={} new_signatures=1",
        args.input.display(),
        args.output.display(),
        key_pair.public_key(),
        block.signatures().count()
    );
    Ok(())
}
struct Args {
    input: PathBuf,
    output: PathBuf,
    private_key: Option<String>,
    private_key_file: Option<PathBuf>,
    seed: Option<String>,
    algorithm: Algorithm,
    zk_policy_hash: Option<[u8; 32]>,
}
impl Args {
    fn parse() -> Result<Self> {
        let mut input = None;
        let mut output = None;
        let mut private_key = None;
        let mut private_key_file = None;
        let mut seed = None;
        let mut algorithm = Algorithm::default();
        let mut zk_policy_hash = None;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--input" => input = Some(next_value(&mut args, "--input")?.into()),
                "--output" => output = Some(next_value(&mut args, "--output")?.into()),
                "--private-key" => private_key = Some(next_value(&mut args, "--private-key")?),
                "--private-key-file" => {
                    private_key_file = Some(next_value(&mut args, "--private-key-file")?.into());
                }
                "--seed" => seed = Some(next_value(&mut args, "--seed")?),
                "--algorithm" => {
                    algorithm = next_value(&mut args, "--algorithm")?
                        .parse()
                        .map_err(|err| eyre!("invalid --algorithm: {err}"))?;
                }
                "--zk-policy-hash-hex" => {
                    zk_policy_hash = Some(parse_hex_32(&next_value(
                        &mut args,
                        "--zk-policy-hash-hex",
                    )?)?);
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(eyre!("unknown argument: {other}")),
            }
        }
        let key_sources = [
            private_key.is_some(),
            private_key_file.is_some(),
            seed.is_some(),
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
        if key_sources != 1 {
            return Err(eyre!(
                "pass exactly one of --private-key, --private-key-file, or --seed"
            ));
        }
        Ok(Self {
            input: input.ok_or_else(|| eyre!("missing --input"))?,
            output: output.ok_or_else(|| eyre!("missing --output"))?,
            private_key,
            private_key_file,
            seed,
            algorithm,
            zk_policy_hash,
        })
    }
    fn load_key_pair(&self) -> Result<KeyPair> {
        match (&self.private_key, &self.private_key_file, &self.seed) {
            (Some(hex), None, None) => self.load_private_key_hex(hex),
            (None, Some(path), None) => {
                let hex = read_genesis_private_key_file(path)?;
                self.load_private_key_hex(hex.trim())
            }
            (None, None, Some(seed)) => {
                KeyPair::try_from_seed(seed.as_bytes().to_vec(), self.algorithm)
                    .map_err(|err| eyre!("derive seeded genesis key pair: {err}"))
            }
            _ => unreachable!("validated by Args::parse"),
        }
    }
    fn load_private_key_hex(&self, hex: &str) -> Result<KeyPair> {
        let private_key = PrivateKey::from_hex(self.algorithm, hex)
            .map_err(|err| eyre!("decode genesis private key: {err}"))?;
        KeyPair::from_private_key(private_key)
            .map_err(|err| eyre!("derive genesis key pair from private key: {err}"))
    }
}
fn read_genesis_private_key_file(path: &Path) -> Result<String> {
    let max_bytes_u64 =
        u64::try_from(MAX_GENESIS_PRIVATE_KEY_FILE_BYTES).expect("private-key file limit fits u64");
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| eyre!("inspect genesis private key file: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes_u64 {
        return Err(eyre!(
            "genesis private key must be a direct regular file of at most {MAX_GENESIS_PRIVATE_KEY_FILE_BYTES} bytes"
        ));
    }
    let mut file =
        File::open(path).map_err(|error| eyre!("open genesis private key file: {error}"))?;
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| eyre!("genesis private key file length does not fit usize"))?;
    let mut hex = String::with_capacity(capacity.saturating_add(1));
    file.by_ref()
        .take(metadata.len().saturating_add(1))
        .read_to_string(&mut hex)
        .map_err(|error| eyre!("read genesis private key file: {error}"))?;
    if hex.len() > MAX_GENESIS_PRIVATE_KEY_FILE_BYTES {
        return Err(eyre!(
            "genesis private key file exceeds the {MAX_GENESIS_PRIVATE_KEY_FILE_BYTES}-byte limit"
        ));
    }
    Ok(hex)
}
fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| eyre!("{flag} requires a value"))
        .and_then(|value| {
            if value.is_empty() {
                Err(eyre!("{flag} requires a non-empty value"))
            } else {
                Ok(value)
            }
        })
}
fn print_help() {
    println!(
        "Usage: genesis_resign --input genesis.signed.nrt --output fixed.nrt \\
         (--seed SEED | --private-key HEX | --private-key-file PATH) [--algorithm ed25519] \\
         [--zk-policy-hash-hex HEX]"
    );
}
fn parse_hex_32(hex: &str) -> Result<[u8; 32]> {
    let compact = hex.trim();
    if compact.len() != 64 {
        return Err(eyre!(
            "--zk-policy-hash-hex must be exactly 64 hexadecimal characters"
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&compact[start..start + 2], 16)
            .map_err(|err| eyre!("invalid --zk-policy-hash-hex: {err}"))?;
    }
    Ok(bytes)
}
fn confidential_features_with_policy_hash(
    existing: Option<ConfidentialFeatureDigest>,
    hash: [u8; 32],
) -> ConfidentialFeatureDigest {
    let mut digest = existing.unwrap_or_else(|| {
        ConfidentialFeatureDigest::new(None, None, None, Some(CONFIDENTIAL_RULES_VERSION), None)
    });
    digest.zk_policy_hash = Some(hash);
    digest
}
fn resign_block(
    block: &SignedBlock,
    key_pair: &KeyPair,
    zk_policy_hash: Option<[u8; 32]>,
) -> Result<SignedBlock> {
    let transactions = block.external_transactions().cloned().collect::<Vec<_>>();
    let confidential_features = zk_policy_hash.map_or_else(
        || block.header().confidential_features,
        |hash| {
            Some(confidential_features_with_policy_hash(
                block.header().confidential_features,
                hash,
            ))
        },
    );
    SignedBlock::try_genesis_with_da_proof_policies(
        transactions,
        key_pair.private_key(),
        confidential_features,
        block.da_commitments().cloned(),
        block.da_proof_policies().cloned(),
    )
    .map_err(|err| eyre!("rebuild canonical genesis block: {err}"))
}
#[cfg(test)]
mod tests {
    use super::{
        MAX_GENESIS_PRIVATE_KEY_FILE_BYTES, confidential_features_with_policy_hash, parse_hex_32,
        read_genesis_private_key_file,
    };
    use iroha_data_model::confidential::{CONFIDENTIAL_RULES_VERSION, ConfidentialFeatureDigest};
    #[test]
    fn parse_hex_32_accepts_exact_hash() {
        let hash = parse_hex_32("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
            .expect("valid hash");
        assert_eq!(hash[0], 0x00);
        assert_eq!(hash[1], 0x01);
        assert_eq!(hash[31], 0x1f);
    }
    #[test]
    fn parse_hex_32_rejects_bad_length_and_characters() {
        assert!(parse_hex_32("00").is_err());
        assert!(
            parse_hex_32("zz0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .is_err()
        );
    }
    #[test]
    fn private_key_reader_rejects_first_byte_over_limit() {
        let directory = tempfile::tempdir().expect("create private-key reader directory");
        let path = directory.path().join("genesis.private_key");
        std::fs::write(&path, vec![b'a'; MAX_GENESIS_PRIVATE_KEY_FILE_BYTES + 1])
            .expect("write oversized private-key file");
        let error = read_genesis_private_key_file(&path)
            .expect_err("oversized private-key file must fail closed");
        assert!(error.to_string().contains("at most"));
    }
    #[test]
    fn confidential_features_with_policy_hash_preserves_existing_commitments() {
        let existing = ConfidentialFeatureDigest::new(
            Some([0x11; 32]),
            Some(7),
            Some(9),
            Some(3),
            Some([0x22; 32]),
        );
        let updated = confidential_features_with_policy_hash(Some(existing), [0x33; 32]);
        assert_eq!(updated.vk_set_hash, Some([0x11; 32]));
        assert_eq!(updated.poseidon_params_id, Some(7));
        assert_eq!(updated.pedersen_params_id, Some(9));
        assert_eq!(updated.conf_rules_version, Some(3));
        assert_eq!(updated.zk_policy_hash, Some([0x33; 32]));
    }
    #[test]
    fn confidential_features_with_policy_hash_creates_v1_digest_when_missing() {
        let updated = confidential_features_with_policy_hash(None, [0x44; 32]);
        assert_eq!(updated.vk_set_hash, None);
        assert_eq!(updated.poseidon_params_id, None);
        assert_eq!(updated.pedersen_params_id, None);
        assert_eq!(updated.conf_rules_version, Some(CONFIDENTIAL_RULES_VERSION));
        assert_eq!(updated.zk_policy_hash, Some([0x44; 32]));
    }
}
