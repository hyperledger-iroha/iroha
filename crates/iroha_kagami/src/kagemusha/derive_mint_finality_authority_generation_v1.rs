//! Provision one public mint authority generation and network-bound candidate possession proofs.
//!
//! Independent seed blocks arrive through one transferred pipe, are wiped on every
//! exit path, and are never serialized. Generation zero provisions genesis; later generations are
//! candidate material only. This command does not authorize an epoch, mutate state, or schedule
//! authority rotation. Retained authorities do not require a fresh generation each epoch.

use std::io::{BufWriter, Read, Write};

use clap::Args as ClapArgs;
use color_eyre::eyre::{bail, eyre};
use iroha_core::zk::kagemusha_v1_recursion::{
    derive_kagemusha_mint_finality_validator_keys_v1,
    prove_kagemusha_mint_finality_candidate_possession_v1,
    validate_kagemusha_mint_finality_authority_v1,
    verify_kagemusha_mint_finality_candidate_possession_v1,
};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::{MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size},
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationV1,
        KagemushaMintFinalityPairedPossessionProofV1, KagemushaMintFinalityValidatorKeysV1,
    },
    nexus::{ValidatorCandidateKeyAuthorizationV1, ValidatorCandidateKeysV1},
};
use iroha_model_base::peer::PeerId;
use zeroize::{Zeroize, Zeroizing};

use crate::{Outcome, secure_fs::take_seed_pipe};

#[cfg(test)]
const VALIDATORS: usize = 4;
const SEED_BYTES: usize = 32;
#[cfg(test)]
const INPUT_BYTES: usize = VALIDATORS * SEED_BYTES;

/// Public derivation context and an inherited descriptor number; contains no seed material.
#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    /// Exact nonzero genesis-derived NetworkId in its canonical checked hash spelling.
    #[arg(long, value_name = "NETWORK_ID")]
    network_id: String,
    /// Public key generation; zero provisions genesis, later values require a prepared transition.
    #[arg(long, value_name = "GENERATION")]
    generation: u64,
    /// Repeat an exact 3f+1 committee of canonical BLS-normal peers in strict order.
    #[arg(
        long = "validator",
        value_name = "PEER_ID",
        required = true,
        num_args = 1
    )]
    validators: Vec<String>,
    /// Transferred pipe read FD (>=3): 32 raw bytes per validator, then EOF; no file paths.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(i32).range(3..))]
    seed_fd: i32,
}

/// Independent candidate provisioning without assuming committee size or election selection.
#[derive(Debug, ClapArgs)]
pub(super) struct CandidateArgs {
    /// Exact canonical genesis-derived target network identity.
    #[arg(long, value_name = "NETWORK_ID")]
    network_id: String,
    /// Public key generation for the candidate's independent key material.
    #[arg(long)]
    generation: u64,
    /// One canonical BLS-normal candidate identity.
    #[arg(long, value_name = "PEER_ID")]
    validator: String,
    /// Transferred pipe read FD (>=3): exactly one nonzero 32-byte independent seed, then EOF.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(i32).range(3..))]
    seed_fd: i32,
    /// Separate transferred pipe FD containing the canonical private key for --validator
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(i32).range(3..))]
    peer_private_key_fd: i32,
}

/// Publish public candidate keys and possession without asserting membership or readiness.
pub(super) fn run_candidate<T: Write>(args: CandidateArgs, writer: &mut BufWriter<T>) -> Outcome {
    let input = take_seed_pipe(args.seed_fd)?;
    if args.seed_fd == args.peer_private_key_fd {
        bail!("candidate seed and peer key require distinct pipe descriptors");
    }
    let peer_input = take_seed_pipe(args.peer_private_key_fd)?;
    if args.generation == 0 {
        bail!("candidate publication requires a nonzero successor generation");
    }
    let context = Args {
        network_id: args.network_id,
        generation: args.generation,
        validators: vec![args.validator],
        seed_fd: args.seed_fd,
    };
    let (network_id, validators) = validate_identity_context(&context)?;
    let peer_key = read_peer_private_key(peer_input, &validators[0])?;
    let mut scratch = [0_u8; SEED_BYTES + 1];
    let provisioned = with_seed_input(input, &mut scratch, |seeds| {
        let seed = seeds
            .try_into()
            .map_err(|_| eyre!("invalid candidate seed bound"))?;
        let keys = derive_kagemusha_mint_finality_validator_keys_v1(
            seed,
            args.generation,
            validators[0].clone(),
        )
        .map_err(|_| eyre!("candidate public key derivation failed"))?;
        let possession = prove_kagemusha_mint_finality_candidate_possession_v1(
            seed,
            network_id,
            args.generation,
            &keys,
        )
        .map_err(|_| eyre!("candidate possession failed"))?;
        verify_kagemusha_mint_finality_candidate_possession_v1(
            network_id,
            args.generation,
            &keys,
            &possession,
        )
        .map_err(|_| eyre!("candidate possession verification failed"))?;
        let authorization = ValidatorCandidateKeyAuthorizationV1::new(
            network_id,
            args.generation,
            keys.clone(),
            possession,
        );
        let peer_signature =
            iroha_crypto::SignatureOf::try_new(peer_key.private_key(), &authorization)
                .map_err(|_| eyre!("candidate peer consent signing failed"))?;
        Ok(ValidatorCandidateKeysV1 {
            network_id,
            generation: args.generation,
            keys,
            possession,
            peer_signature,
        })
    })?;
    drop(peer_key);
    let json = norito::json::to_string(&provisioned)
        .map_err(|_| eyre!("cannot encode public candidate"))?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn read_peer_private_key(
    mut input: impl Read,
    peer: &PeerId,
) -> color_eyre::Result<iroha_crypto::KeyPair> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .by_ref()
        .take(1025)
        .read_to_end(&mut bytes)
        .map_err(|_| eyre!("candidate peer key pipe read failed"))?;
    if bytes.len() > 1024 {
        bail!("candidate peer key exceeds the canonical input bound");
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| eyre!("candidate peer key is not canonical text"))?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let key = text
        .parse::<iroha_crypto::ExposedPrivateKey>()
        .map_err(|_| eyre!("candidate peer key is invalid"))?;
    let canonical = Zeroizing::new(key.to_string());
    if canonical.as_str() != text {
        bail!("candidate peer key is not canonical text");
    }
    let pair = iroha_crypto::KeyPair::from_private_key(key.0)
        .map_err(|_| eyre!("candidate peer key is invalid"))?;
    if pair.public_key() != peer.public_key() {
        bail!("candidate peer key differs from --validator");
    }
    Ok(pair)
}

/// Complete public provisioning output; possession grants no epoch or committee authorization.
#[derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ProvisionedAuthorityV1 {
    schema_version: u8,
    authority: KagemushaMintFinalityAuthorityGenerationV1,
    candidate_possessions: Vec<KagemushaMintFinalityPairedPossessionProofV1>,
}

/// Emit only public authority and possession JSON after closing and wiping seed input.
pub(super) fn run<T: Write>(args: Args, writer: &mut BufWriter<T>) -> Outcome {
    // Claim first so an invalid public context also closes the transferred descriptor.
    let input = take_seed_pipe(args.seed_fd)?;
    let (network_id, validators) = validate_public_context(&args)?;
    let mut scratch = vec![0_u8; validators.len() * SEED_BYTES + 1];
    let provisioned = with_seed_input(input, &mut scratch, |seeds| {
        derive_authority(network_id, args.generation, &validators, seeds)
    })?;
    // Both the input descriptor and guarded seed buffer are gone before any public output.
    let json = norito::json::to_string(&provisioned)
        .map_err(|_| eyre!("cannot encode the public authority generation"))?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn validate_public_context(args: &Args) -> color_eyre::Result<(NetworkId, Vec<PeerId>)> {
    if !is_valid_committee_size(args.validators.len())
        || args.validators.len() > MAX_VALIDATORS_PER_HEIGHT
    {
        bail!("authority generation requires an exact bounded 3f+1 validator committee");
    }
    validate_identity_context(args)
}

fn validate_identity_context(args: &Args) -> color_eyre::Result<(NetworkId, Vec<PeerId>)> {
    let network_id = args
        .network_id
        .parse::<NetworkId>()
        .map_err(|_| eyre!("network-id must be one canonical genesis-derived NetworkId"))?;
    if network_id.as_bytes() == &[0; 32] || network_id.to_string() != args.network_id {
        bail!("network-id must be canonical and nonzero");
    }
    let mut validators = Vec::with_capacity(args.validators.len());
    for text in &args.validators {
        // Bound the current production BLS-normal voter spelling; generic PeerId keys may be larger.
        if text.len() > 512 {
            bail!("validator identity exceeds the canonical input bound");
        }
        let peer = text
            .parse::<PeerId>()
            .map_err(|_| eyre!("validator must be one canonical PeerId"))?;
        if peer.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal {
            bail!("validator must be a canonical BLS-normal consensus voter");
        }
        if peer.to_string() != *text {
            bail!("validator must use its canonical PeerId spelling");
        }
        if validators.last().is_some_and(|prior| prior >= &peer) {
            bail!("validators must be distinct and strictly ordered by PeerId");
        }
        validators.push(peer);
    }
    Ok((network_id, validators))
}

/// Wipe the original storage on success, rejection, I/O failure and unwinding, without copying it.
struct SeedWipe<'a>(&'a mut [u8]);

impl Drop for SeedWipe<'_> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

fn with_seed_input<R: Read, T>(
    mut reader: R,
    scratch: &mut [u8],
    derive: impl FnOnce(&[u8]) -> color_eyre::Result<T>,
) -> color_eyre::Result<T> {
    let guard = SeedWipe(scratch);
    let input_bytes = guard
        .0
        .len()
        .checked_sub(1)
        .filter(|length| {
            *length > 0
                && *length <= MAX_VALIDATORS_PER_HEIGHT * SEED_BYTES
                && *length % SEED_BYTES == 0
        })
        .ok_or_else(|| eyre!("invalid seed payload bound"))?;
    let mut length = 0;
    while length < guard.0.len() {
        match reader.read(&mut guard.0[length..]) {
            Ok(0) => break,
            Ok(count) => length += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => bail!("cannot read the inherited seed payload"),
        }
    }
    // No descriptor is held during cryptographic derivation or output serialization.
    drop(reader);
    if length != input_bytes {
        bail!("seed payload must contain exactly {input_bytes} raw bytes followed by EOF");
    }
    let seeds = &guard.0[..input_bytes];
    for (index, seed) in seeds.chunks_exact(SEED_BYTES).enumerate() {
        if seed.iter().all(|byte| *byte == 0)
            || seeds[..index * SEED_BYTES]
                .chunks_exact(SEED_BYTES)
                .any(|prior| prior == seed)
        {
            bail!("seed blocks must be nonzero and independently provisioned per validator");
        }
    }
    derive(seeds)
}

fn derive_authority(
    network_id: NetworkId,
    generation: u64,
    validators: &[PeerId],
    seeds: &[u8],
) -> color_eyre::Result<ProvisionedAuthorityV1> {
    if !is_valid_committee_size(validators.len())
        || validators.len() > MAX_VALIDATORS_PER_HEIGHT
        || seeds.len() != validators.len() * SEED_BYTES
    {
        bail!("invalid mint-finality derivation context");
    }
    let mut keys = Vec::with_capacity(validators.len());
    let mut candidate_possessions = Vec::with_capacity(validators.len());
    for (validator, seed) in validators.iter().zip(seeds.chunks_exact(SEED_BYTES)) {
        let seed = seed
            .try_into()
            .map_err(|_| eyre!("invalid seed block bound"))?;
        let public_keys =
            derive_kagemusha_mint_finality_validator_keys_v1(seed, generation, validator.clone())
                .map_err(|_| eyre!("mint-finality public key derivation failed"))?;
        let possession = prove_kagemusha_mint_finality_candidate_possession_v1(
            seed,
            network_id,
            generation,
            &public_keys,
        )
        .map_err(|_| eyre!("mint-finality candidate possession failed"))?;
        verify_kagemusha_mint_finality_candidate_possession_v1(
            network_id,
            generation,
            &public_keys,
            &possession,
        )
        .map_err(|_| eyre!("derived possession proof failed verification"))?;
        keys.push(public_keys);
        candidate_possessions.push(possession);
    }
    let authority = KagemushaMintFinalityAuthorityGenerationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        generation,
        validators: keys,
    };
    validate_kagemusha_mint_finality_authority_v1(&authority)
        .map_err(|_| eyre!("derived mint authority contains invalid public points or ordering"))?;
    Ok(ProvisionedAuthorityV1 {
        schema_version: 1,
        authority,
        candidate_possessions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use std::{
        fs::File,
        io::Cursor,
        os::fd::{AsRawFd, FromRawFd, IntoRawFd},
    };

    fn context() -> Args {
        let mut validators = (1_u8..=4)
            .map(|index| {
                PeerId::new(
                    KeyPair::try_from_seed(vec![index; 32], Algorithm::BlsNormal)
                        .expect("synthetic test identity")
                        .public_key()
                        .clone(),
                )
            })
            .collect::<Vec<_>>();
        validators.sort();
        Args {
            network_id: network(b"epoch-command-test").to_string(),
            generation: 7,
            validators: validators.iter().map(ToString::to_string).collect(),
            seed_fd: 3,
        }
    }

    fn network(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(label)))
    }

    fn seeds() -> [u8; INPUT_BYTES] {
        let mut result = [0; INPUT_BYTES];
        for (index, seed) in result.chunks_exact_mut(SEED_BYTES).enumerate() {
            seed.fill(u8::try_from(index + 11).expect("four synthetic seeds"));
        }
        result
    }

    #[test]
    fn authority_provisioning_binds_network_generation_identity_and_possession() {
        let args = context();
        let (network_id, validators) = validate_public_context(&args).unwrap();
        for generation in [0, args.generation] {
            let provisioned =
                derive_authority(network_id, generation, &validators, &seeds()).unwrap();
            let json = norito::json::to_vec(&provisioned).unwrap();
            let decoded: ProvisionedAuthorityV1 = norito::json::from_slice(&json).unwrap();
            assert_eq!(decoded.schema_version, 1);
            assert_eq!(decoded.authority.generation, generation);
            assert_eq!(decoded.authority.network_id, network_id);
            assert_eq!(decoded.candidate_possessions.len(), 4);
            for (index, (keys, proof)) in decoded
                .authority
                .validators
                .iter()
                .zip(&decoded.candidate_possessions)
                .enumerate()
            {
                assert_eq!(keys.validator, validators[index]);
                assert_eq!(
                    *keys,
                    derive_kagemusha_mint_finality_validator_keys_v1(
                        seeds()[index * 32..(index + 1) * 32].try_into().unwrap(),
                        generation,
                        validators[index].clone()
                    )
                    .unwrap()
                );
                verify_kagemusha_mint_finality_candidate_possession_v1(
                    network_id, generation, keys, proof,
                )
                .unwrap();
                assert!(
                    verify_kagemusha_mint_finality_candidate_possession_v1(
                        network(b"other-network"),
                        generation,
                        keys,
                        proof
                    )
                    .is_err()
                );
                assert!(
                    verify_kagemusha_mint_finality_candidate_possession_v1(
                        network_id,
                        generation + 1,
                        keys,
                        proof
                    )
                    .is_err()
                );
            }
            let mut unknown = norito::json::from_slice::<norito::json::Value>(&json).unwrap();
            unknown
                .as_object_mut()
                .unwrap()
                .insert("private_seed".into(), norito::json::Value::Null);
            assert!(norito::json::from_value::<ProvisionedAuthorityV1>(unknown).is_err());
        }
        let original = derive_authority(network_id, 0, &validators, &seeds()).unwrap();
        let next = derive_authority(network_id, 1, &validators, &seeds()).unwrap();
        assert_ne!(original.authority.validators, next.authority.validators);
        assert!(derive_authority(network_id, 1, &validators[..3], &seeds()).is_err());
    }

    #[test]
    fn independent_candidate_consumes_one_seed_and_emits_verified_public_possession() {
        let args = context();
        let (read, mut write) = pipe();
        write.write_all(&[0xA3; 32]).unwrap();
        drop(write);
        let pair = (1..=4)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .find(|pair| pair.public_key().to_string() == args.validators[0])
            .unwrap();
        let (peer_read, mut peer_write) = pipe();
        peer_write
            .write_all(
                iroha_crypto::ExposedPrivateKey(pair.private_key().clone())
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        drop(peer_write);
        let mut writer = BufWriter::new(Vec::new());
        run_candidate(
            CandidateArgs {
                network_id: args.network_id.clone(),
                generation: 9,
                validator: args.validators[0].clone(),
                seed_fd: read.into_raw_fd(),
                peer_private_key_fd: peer_read.into_raw_fd(),
            },
            &mut writer,
        )
        .unwrap();
        let output = writer.into_inner().unwrap();
        let candidate: ValidatorCandidateKeysV1 = norito::json::from_slice(&output).unwrap();
        candidate.validate().unwrap();
        candidate
            .peer_signature
            .verify(pair.public_key(), &candidate.authorization())
            .unwrap();
        assert_eq!(candidate.generation, 9);
        assert_eq!(candidate.keys.validator.to_string(), args.validators[0]);
        verify_kagemusha_mint_finality_candidate_possession_v1(
            candidate.network_id,
            9,
            &candidate.keys,
            &candidate.possession,
        )
        .unwrap();
        assert!(
            verify_kagemusha_mint_finality_candidate_possession_v1(
                candidate.network_id,
                10,
                &candidate.keys,
                &candidate.possession
            )
            .is_err()
        );
        for input in [vec![0; 32], vec![1; 31], vec![1; 33]] {
            let mut scratch = [0_u8; 33];
            assert!(with_seed_input(Cursor::new(input), &mut scratch, |_| Ok(())).is_err());
            assert_eq!(scratch, [0; 33]);
        }
    }

    #[test]
    fn candidate_peer_custody_rejects_wrong_identity_noncanonical_and_oversized_secrets() {
        let pair = KeyPair::from_seed(vec![0x61; 32], Algorithm::BlsNormal);
        let peer = PeerId::new(pair.public_key().clone());
        let text = iroha_crypto::ExposedPrivateKey(pair.private_key().clone()).to_string();
        for input in [text.clone(), format!("{text}\n")] {
            assert_eq!(
                read_peer_private_key(Cursor::new(input), &peer)
                    .unwrap()
                    .public_key(),
                pair.public_key()
            );
        }
        let other = PeerId::new(
            KeyPair::from_seed(vec![0x62; 32], Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        assert!(
            read_peer_private_key(Cursor::new(&text), &other)
                .unwrap_err()
                .to_string()
                .contains("differs")
        );
        for input in [
            format!(" {text}"),
            format!("{text}\n\n"),
            format!("{text}\r\n"),
            "invalid-secret-marker".to_owned(),
            "x".repeat(1025),
        ] {
            let error = read_peer_private_key(Cursor::new(&input), &peer)
                .unwrap_err()
                .to_string();
            assert!(!error.contains(&text));
            assert!(!error.contains("invalid-secret-marker"));
        }
    }

    #[test]
    fn candidate_parser_requires_separate_numeric_peer_key_descriptor() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: super::super::Args,
        }
        let context = context();
        let mut argv = vec![
            "kagemusha".to_owned(),
            "derive-mint-finality-candidate-v1".to_owned(),
            "--network-id".to_owned(),
            context.network_id,
            "--generation".to_owned(),
            "1".to_owned(),
            "--validator".to_owned(),
            context.validators[0].clone(),
            "--seed-fd".to_owned(),
            "3".to_owned(),
        ];
        assert!(Cli::try_parse_from(&argv).is_err());
        argv.extend(["--peer-private-key-fd".to_owned(), "4".to_owned()]);
        assert!(Cli::try_parse_from(&argv).is_ok());
        for value in ["0", "2", "/run/peer.key"] {
            let mut bad = argv.clone();
            *bad.last_mut().unwrap() = value.to_owned();
            assert!(Cli::try_parse_from(bad).is_err());
        }
    }

    #[test]
    fn authority_provisioning_accepts_seven_and_maximum_committees_without_four_seat_limit() {
        for count in [7, MAX_VALIDATORS_PER_HEIGHT] {
            let mut args = context();
            let mut peers = (1..=count)
                .map(|index| {
                    PeerId::new(
                        KeyPair::try_from_seed(
                            vec![u8::try_from(index).unwrap(); 32],
                            Algorithm::BlsNormal,
                        )
                        .unwrap()
                        .public_key()
                        .clone(),
                    )
                })
                .collect::<Vec<_>>();
            peers.sort();
            args.validators = peers.iter().map(ToString::to_string).collect();
            let (network, peers) = validate_public_context(&args).unwrap();
            let seeds = (1..=count)
                .flat_map(|index| [u8::try_from(index + 32).unwrap(); 32])
                .collect::<Vec<_>>();
            let mut scratch = vec![0_u8; seeds.len() + 1];
            let authority = with_seed_input(Cursor::new(seeds), &mut scratch, |seeds| {
                derive_authority(network, 8, &peers, seeds)
            })
            .unwrap();
            assert_eq!(authority.authority.validators.len(), count);
            assert_eq!(authority.candidate_possessions.len(), count);
            assert!(scratch.iter().all(|byte| *byte == 0));
        }
    }

    #[test]
    fn public_context_rejects_malformed_network_count_order_and_duplicates() {
        for value in ["", "taira", "00", "hash:00#0000"] {
            let mut args = context();
            args.network_id = value.to_owned();
            assert!(validate_public_context(&args).is_err());
        }
        let mut args = context();
        args.network_id = args.network_id.to_lowercase();
        assert!(validate_public_context(&args).is_err());
        let mut args = context();
        args.network_id = norito::literal::format("hash", &"0".repeat(64));
        assert!(validate_public_context(&args).is_err());
        for count in [0, 3, 5] {
            let mut args = context();
            args.validators.resize(count, args.validators[0].clone());
            assert!(validate_public_context(&args).is_err());
        }
        let mut args = context();
        args.validators.swap(0, 1);
        assert!(validate_public_context(&args).is_err());
        let mut args = context();
        args.validators[1] = args.validators[0].clone();
        assert!(validate_public_context(&args).is_err());
        let non_voter = PeerId::new(
            KeyPair::try_from_seed(vec![0x55; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        );
        let mut args = context();
        args.validators[0] = non_voter.to_string();
        assert!(
            validate_public_context(&args)
                .unwrap_err()
                .to_string()
                .contains("BLS-normal")
        );
        for value in ["not-a-peer".to_owned(), "x".repeat(513)] {
            let mut args = context();
            args.validators[0] = value;
            assert!(validate_public_context(&args).is_err());
        }
    }

    #[test]
    fn parser_exposes_only_public_arguments_and_numeric_pipe_descriptor() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: super::super::Args,
        }
        let args = context();
        let mut argv = vec![
            "kagemusha".to_owned(),
            "derive-mint-finality-authority-generation-v1".to_owned(),
            "--network-id".to_owned(),
            args.network_id,
            "--generation".to_owned(),
            "7".to_owned(),
        ];
        for peer in args.validators {
            argv.extend(["--validator".to_owned(), peer]);
        }
        argv.extend(["--seed-fd".to_owned(), "3".to_owned()]);
        assert!(Cli::try_parse_from(&argv).is_ok());
        for value in ["0", "2", "-1", "2147483648", "/dev/fd/3"] {
            let mut bad = argv.clone();
            *bad.last_mut().unwrap() = value.to_owned();
            assert!(Cli::try_parse_from(bad).is_err());
        }
        for option in ["--seed", "--seed-file", "--seed-env", "--output"] {
            let mut bad = argv.clone();
            bad.extend([option.to_owned(), "unsupported".to_owned()]);
            assert!(Cli::try_parse_from(bad).is_err());
        }
    }

    #[test]
    fn seed_reader_enforces_exact_bound_and_wipes_success_rejections_and_unwind() {
        let mut scratch = [0; INPUT_BYTES + 1];
        let mut reader = Cursor::new(seeds());
        assert_eq!(
            with_seed_input(&mut reader, &mut scratch, |input| Ok(input.len())).unwrap(),
            INPUT_BYTES
        );
        assert_eq!(scratch, [0; INPUT_BYTES + 1]);
        for length in [0, 1, 127, 129, 4096] {
            let mut reader = Cursor::new(vec![0xAA; length]);
            let error = with_seed_input(&mut reader, &mut scratch, |_| Ok(())).unwrap_err();
            assert!(error.to_string().contains("exactly 128"));
            assert!(reader.position() <= 129);
            assert_eq!(scratch, [0; INPUT_BYTES + 1]);
        }
        for input in [[0; INPUT_BYTES], [0xAA; INPUT_BYTES]] {
            assert!(with_seed_input(Cursor::new(input), &mut scratch, |_| Ok(())).is_err());
            assert_eq!(scratch, [0; INPUT_BYTES + 1]);
        }
        assert!(
            with_seed_input(Cursor::new(seeds()), &mut scratch, |_| Err::<(), _>(eyre!(
                "synthetic derivation error"
            )))
            .is_err()
        );
        assert_eq!(scratch, [0; INPUT_BYTES + 1]);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: color_eyre::Result<()> =
                with_seed_input(Cursor::new(seeds()), &mut scratch, |_| {
                    panic!("test unwind")
                });
        }));
        assert!(panic.is_err());
        assert_eq!(scratch, [0; INPUT_BYTES + 1]);
    }

    #[test]
    fn read_errors_are_redacted_and_partial_seeds_are_wiped() {
        struct FailingReader(u8);
        impl Read for FailingReader {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                self.0 += 1;
                match self.0 {
                    1 => Err(std::io::ErrorKind::Interrupted.into()),
                    2 => {
                        bytes[..12].fill(0xAB);
                        Ok(12)
                    }
                    _ => Err(std::io::Error::other("do not expose this reader context")),
                }
            }
        }
        let mut scratch = [0; INPUT_BYTES + 1];
        let error = with_seed_input(FailingReader(0), &mut scratch, |_| Ok(())).unwrap_err();
        assert_eq!(error.to_string(), "cannot read the inherited seed payload");
        assert_eq!(scratch, [0; INPUT_BYTES + 1]);
    }

    #[allow(
        unsafe_code,
        reason = "test-only anonymous pipe construction transfers both owned ends"
    )]
    fn pipe() -> (File, File) {
        let mut descriptors = [-1; 2];
        // SAFETY: the array has space for both descriptors; success transfers both valid FDs.
        assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
        // SAFETY: each freshly created descriptor is uniquely owned exactly once.
        unsafe {
            (
                File::from_raw_fd(descriptors[0]),
                File::from_raw_fd(descriptors[1]),
            )
        }
    }

    #[allow(
        unsafe_code,
        reason = "F_GETFD tests descriptor closure without acquiring ownership"
    )]
    fn assert_closed(fd: i32) {
        // SAFETY: F_GETFD validates arbitrary descriptor numbers without memory access.
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
    }

    #[test]
    fn buffered_output_failures_are_returned() {
        struct FailingOutput {
            reject_write: bool,
        }
        impl Write for FailingOutput {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.reject_write {
                    Err(std::io::Error::other("synthetic output write failure"))
                } else {
                    Ok(bytes.len())
                }
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("synthetic output flush failure"))
            }
        }
        for reject_write in [false, true] {
            let (read, mut write) = pipe();
            write.write_all(&seeds()).unwrap();
            drop(write);
            let mut args = context();
            args.seed_fd = read.into_raw_fd();
            let mut output = BufWriter::new(FailingOutput { reject_write });
            let error = run(args, &mut output).unwrap_err();
            assert!(error.to_string().contains(if reject_write {
                "synthetic output write failure"
            } else {
                "synthetic output flush failure"
            }));
        }
    }

    #[test]
    fn inherited_descriptor_ownership_is_closed() {
        // A separate single-test process prevents unrelated parallel tests from reusing an FD
        // between its close and the EBADF probe. This environment marker contains no seed data.
        const CHILD: &str = "IROHA_KAGAMI_AUTHORITY_DERIVE_FD_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "kagemusha::derive_mint_finality_authority_generation_v1::tests::inherited_descriptor_ownership_is_closed",
                    "--test-threads=1",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
            return;
        }
        for fd in [-1, 0, 1, 2, i32::MAX] {
            assert!(take_seed_pipe(fd).is_err());
        }
        let regular = tempfile::tempfile().unwrap(); // Empty; no seed file is written.
        let fd = regular.into_raw_fd();
        assert!(take_seed_pipe(fd).is_err());
        assert_closed(fd);
        let (read, write) = pipe();
        let fd = write.into_raw_fd();
        assert!(take_seed_pipe(fd).is_err());
        assert_closed(fd);
        drop(read);
        let (read, write) = pipe();
        let fd = read.into_raw_fd();
        let owned = take_seed_pipe(fd).unwrap();
        assert_eq!(owned.as_raw_fd(), fd);
        assert!(
            rustix::io::fcntl_getfd(&owned)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        drop(owned);
        assert_closed(fd);
        drop(write);
        let (read, mut write) = pipe();
        write.write_all(&seeds()).unwrap();
        drop(write);
        let fd = read.into_raw_fd();
        let mut scratch = [0; INPUT_BYTES + 1];
        with_seed_input(take_seed_pipe(fd).unwrap(), &mut scratch, |_| {
            assert_closed(fd); // Closed before the derivation callback can run.
            Ok(())
        })
        .unwrap();
        assert_eq!(scratch, [0; INPUT_BYTES + 1]);
        for case in 0..3 {
            let (read, mut write) = pipe();
            let seed_bytes = seeds();
            write
                .write_all(if case == 2 {
                    &seed_bytes[..127]
                } else {
                    &seed_bytes
                })
                .unwrap();
            drop(write);
            let mut args = context();
            let fd = read.into_raw_fd();
            args.seed_fd = fd;
            if case == 1 {
                args.network_id.clear();
            }
            let mut output = BufWriter::new(Vec::new());
            let result = run(args, &mut output);
            assert_closed(fd);
            let output = output.into_inner().unwrap();
            if case == 0 {
                result.unwrap();
                assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
                let provisioned =
                    norito::json::from_slice::<ProvisionedAuthorityV1>(&output).unwrap();
                assert_eq!(provisioned.authority.generation, 7);
                assert_eq!(provisioned.candidate_possessions.len(), 4);
            } else {
                assert!(result.is_err());
                assert!(output.is_empty());
            }
        }
        // Candidate admission owns two independent secret pipes, including refusals
        // before either payload is consumed. Probe in this isolated process only.
        for (generation, duplicate) in [(0, false), (1, true), (1, false)] {
            let (read, mut write) = pipe();
            write.write_all(&[0xA3; 32]).unwrap();
            drop(write);
            let seed_fd = read.into_raw_fd();
            let peer_fd = if duplicate {
                seed_fd
            } else {
                let (read, mut write) = pipe();
                write.write_all(b"invalid private key").unwrap();
                drop(write);
                read.into_raw_fd()
            };
            let context = context();
            let mut output = BufWriter::new(Vec::new());
            assert!(
                run_candidate(
                    CandidateArgs {
                        network_id: context.network_id,
                        generation,
                        validator: context.validators[0].clone(),
                        seed_fd,
                        peer_private_key_fd: peer_fd,
                    },
                    &mut output
                )
                .is_err()
            );
            assert_closed(seed_fd);
            assert_closed(peer_fd);
            assert!(output.into_inner().unwrap().is_empty());
        }
    }
}
