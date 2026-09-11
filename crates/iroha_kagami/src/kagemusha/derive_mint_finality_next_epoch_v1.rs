//! Derive a public next-epoch mint-finality parameter from inherited, ephemeral seed input.
//!
//! The caller supplies four canonical, strictly ordered BLS-normal voter identities and transfers ownership
//! of a pipe read descriptor greater than stderr. Its writer sends exactly four independent,
//! nonzero 32-byte seeds in that identity order, then closes. No seed path, environment variable,
//! textual seed or consensus private key is accepted. The caller must provision independent seeds
//! per network: Core's provisioning derivation binds the epoch and peer, while the resulting
//! roster and its identifier additionally bind the explicit genesis-derived network identity.

use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    os::fd::FromRawFd,
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{bail, eyre};
use iroha_core::zk::kagemusha_v1_recursion::{
    derive_kagemusha_mint_finality_validator_keys_v1,
    validate_kagemusha_mint_finality_roster_keys_v1,
};
use iroha_data_model::{
    NetworkId,
    isi::kagemusha_v1::{KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1},
    parameter::{Parameter, system::KagemushaMintFinalityNextEpochParameterV1},
};
use iroha_model_base::peer::PeerId;
use zeroize::Zeroize;

use crate::Outcome;

const VALIDATORS: usize = 4;
const SEED_BYTES: usize = 32;
const INPUT_BYTES: usize = VALIDATORS * SEED_BYTES;

/// Public derivation context and an inherited descriptor number; contains no seed material.
#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    /// Exact nonzero genesis-derived NetworkId in its canonical checked hash spelling.
    #[arg(long, value_name = "NETWORK_ID")]
    network_id: String,
    /// Next election epoch; epoch zero is reserved for genesis provisioning.
    #[arg(long, value_name = "EPOCH")]
    epoch: u64,
    /// Repeat four canonical BLS-normal voters in strict PeerId order; seeds use that order.
    #[arg(
        long = "validator",
        value_name = "PEER_ID",
        required = true,
        num_args = 1
    )]
    validators: Vec<String>,
    /// Transferred pipe read FD (>=3): exactly 128 raw seed bytes, then EOF; no file paths.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(i32).range(3..))]
    seed_fd: i32,
}

/// Emit only the canonical typed Parameter JSON and one trailing newline.
pub(super) fn run<T: Write>(args: Args, writer: &mut BufWriter<T>) -> Outcome {
    // Claim first so an invalid public context also closes the transferred descriptor.
    let input = take_seed_pipe(args.seed_fd)?;
    let (network_id, validators) = validate_public_context(&args)?;
    let mut scratch = [0_u8; INPUT_BYTES + 1];
    let parameter = with_seed_input(input, &mut scratch, |seeds| {
        derive_parameter(network_id, args.epoch, &validators, seeds)
    })?;
    // Both the input descriptor and guarded seed buffer are gone before any public output.
    let json = norito::json::to_string(&parameter)
        .map_err(|_| eyre!("cannot encode the public next-epoch parameter"))?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn validate_public_context(args: &Args) -> color_eyre::Result<(NetworkId, Vec<PeerId>)> {
    if args.epoch == 0 {
        bail!("next-epoch derivation requires epoch greater than zero");
    }
    let network_id = args
        .network_id
        .parse::<NetworkId>()
        .map_err(|_| eyre!("network-id must be one canonical genesis-derived NetworkId"))?;
    if network_id.as_bytes() == &[0; 32] || network_id.to_string() != args.network_id {
        bail!("network-id must be canonical and nonzero");
    }
    if args.validators.len() != VALIDATORS {
        bail!("next-epoch derivation requires exactly four validators");
    }
    let mut validators = Vec::with_capacity(VALIDATORS);
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

#[allow(
    unsafe_code,
    reason = "the CLI explicitly transfers one validated inherited descriptor"
)]
fn take_seed_pipe(fd: i32) -> color_eyre::Result<File> {
    if fd < 3 {
        bail!("seed-fd must be an inherited descriptor greater than stderr");
    }
    // SAFETY: F_GETFD accepts an arbitrary integer and does not dereference memory. Probe before
    // constructing an owned descriptor; this CLI owns the transferred FD and has no competing
    // closer. No path is opened and no secret bytes pass through this boundary.
    if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        bail!("seed-fd is not an open inherited descriptor");
    }
    // SAFETY: the preceding probe established validity; the caller transfers unique ownership.
    // File closes it on all subsequent success/error paths. Never duplicate or retain this FD.
    let input = unsafe { File::from_raw_fd(fd) };
    let flags = rustix::io::fcntl_getfd(&input)
        .map_err(|_| eyre!("cannot inspect seed-fd ownership flags"))?;
    rustix::io::fcntl_setfd(&input, flags | rustix::io::FdFlags::CLOEXEC)
        .map_err(|_| eyre!("cannot protect seed-fd from inheritance"))?;
    let stat = rustix::fs::fstat(&input).map_err(|_| eyre!("cannot inspect seed-fd type"))?;
    let mode =
        rustix::fs::fcntl_getfl(&input).map_err(|_| eyre!("cannot inspect seed-fd access mode"))?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::Fifo
        || mode & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
    {
        bail!("seed-fd must be the read end of an inherited pipe");
    }
    Ok(input)
}

/// Wipe the original storage on success, rejection, I/O failure and unwinding, without copying it.
struct SeedWipe<'a>(&'a mut [u8; INPUT_BYTES + 1]);

impl Drop for SeedWipe<'_> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

fn with_seed_input<R: Read, T>(
    mut reader: R,
    scratch: &mut [u8; INPUT_BYTES + 1],
    derive: impl FnOnce(&[u8; INPUT_BYTES]) -> color_eyre::Result<T>,
) -> color_eyre::Result<T> {
    let guard = SeedWipe(scratch);
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
    if length != INPUT_BYTES {
        bail!("seed payload must contain exactly 128 raw bytes followed by EOF");
    }
    let seeds: &[u8; INPUT_BYTES] = guard.0[..INPUT_BYTES]
        .try_into()
        .map_err(|_| eyre!("invalid seed payload bound"))?;
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

fn derive_parameter(
    network_id: NetworkId,
    epoch: u64,
    validators: &[PeerId],
    seeds: &[u8; INPUT_BYTES],
) -> color_eyre::Result<Parameter> {
    if epoch == 0 || validators.len() != VALIDATORS {
        bail!("invalid next-epoch derivation context");
    }
    let validators = validators
        .iter()
        .zip(seeds.chunks_exact(SEED_BYTES))
        .map(|(validator, seed)| {
            let seed = seed
                .try_into()
                .map_err(|_| eyre!("invalid seed block bound"))?;
            derive_kagemusha_mint_finality_validator_keys_v1(seed, epoch, validator.clone())
                .map_err(|_| eyre!("mint-finality public key derivation failed"))
        })
        .collect::<color_eyre::Result<Vec<_>>>()?;
    let typed = KagemushaMintFinalityNextEpochParameterV1 {
        roster: KagemushaMintFinalityEpochRosterV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id,
            epoch,
            validators,
        },
    };
    typed
        .validate()
        .map_err(|_| eyre!("derived next-epoch roster is structurally invalid"))?;
    validate_kagemusha_mint_finality_roster_keys_v1(&typed.roster)
        .map_err(|_| eyre!("derived next-epoch roster contains invalid public points"))?;
    Ok(Parameter::Custom(typed.into_custom_parameter()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use std::{
        io::Cursor,
        os::fd::{AsRawFd, IntoRawFd},
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
            epoch: 7,
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

    fn unpack(parameter: Parameter) -> KagemushaMintFinalityNextEpochParameterV1 {
        let Parameter::Custom(custom) = parameter else {
            panic!("expected a typed custom parameter");
        };
        KagemushaMintFinalityNextEpochParameterV1::from_custom_parameter(&custom)
            .expect("typed roster parameter")
    }

    #[test]
    fn derived_parameter_matches_core_and_binds_network_epoch_and_order() {
        let args = context();
        let (network_id, validators) = validate_public_context(&args).unwrap();
        let seed_bytes = seeds();
        let parameter = derive_parameter(network_id, args.epoch, &validators, &seed_bytes).unwrap();
        let json = norito::json::to_string(&parameter).unwrap();
        let typed = unpack(norito::json::from_str::<Parameter>(&json).unwrap());
        assert_eq!(typed.roster.network_id, network_id);
        assert_eq!(typed.roster.epoch, 7);
        assert_eq!(typed.roster.validators.len(), 4);
        for (index, validator) in validators.iter().enumerate() {
            assert_eq!(
                typed.roster.validators[index],
                derive_kagemusha_mint_finality_validator_keys_v1(
                    seed_bytes[index * SEED_BYTES..(index + 1) * SEED_BYTES]
                        .try_into()
                        .unwrap(),
                    7,
                    validator.clone(),
                )
                .unwrap()
            );
        }
        validate_kagemusha_mint_finality_roster_keys_v1(&typed.roster).unwrap();
        let changed_epoch =
            unpack(derive_parameter(network_id, 8, &validators, &seed_bytes).unwrap());
        assert_ne!(typed.roster.validators, changed_epoch.roster.validators);
        let changed_network = unpack(
            derive_parameter(network(b"another-network"), 7, &validators, &seed_bytes).unwrap(),
        );
        // Core deliberately does not use network_id in provisioning key derivation; its roster
        // identifier binds the network. This command must not silently change that contract.
        assert_eq!(typed.roster.validators, changed_network.roster.validators);
        assert_ne!(
            typed.roster.finality_epoch_id().unwrap(),
            changed_network.roster.finality_epoch_id().unwrap()
        );
        assert!(derive_parameter(network_id, 0, &validators, &seed_bytes).is_err());
        assert!(derive_parameter(network_id, 1, &validators[..3], &seed_bytes).is_err());
        let mut malformed = typed.roster;
        malformed.validators[0].eq_proof_public_key = [0; 32];
        assert!(validate_kagemusha_mint_finality_roster_keys_v1(&malformed).is_err());
    }

    #[test]
    fn public_context_rejects_malformed_network_epoch_count_order_and_duplicates() {
        let mut args = context();
        args.epoch = 0;
        assert!(validate_public_context(&args).is_err());
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
            "derive-mint-finality-next-epoch-v1".to_owned(),
            "--network-id".to_owned(),
            args.network_id,
            "--epoch".to_owned(),
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
        const CHILD: &str = "IROHA_KAGAMI_EPOCH_DERIVE_FD_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "kagemusha::derive_mint_finality_next_epoch_v1::tests::inherited_descriptor_ownership_is_closed",
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
                args.epoch = 0;
            }
            let mut output = BufWriter::new(Vec::new());
            let result = run(args, &mut output);
            assert_closed(fd);
            let output = output.into_inner().unwrap();
            if case == 0 {
                result.unwrap();
                assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
                let parameter = norito::json::from_slice::<Parameter>(&output).unwrap();
                assert_eq!(unpack(parameter).roster.epoch, 7);
            } else {
                assert!(result.is_err());
                assert!(output.is_empty());
            }
        }
    }
}
