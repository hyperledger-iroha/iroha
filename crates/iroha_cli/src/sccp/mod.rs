//! `iroha sccp`: SCCP v1 reads, outbound transfers and proofs (`specs/sccp.md` §6, §7.1, §8).
//!
//! Reads come from one Taira peer's public API and are untrusted: proof bundles are verified by
//! the destination (and by the wallet flows) before use. `send` records an outbound transfer
//! from the configured account after checking the route (§7.1 steps 1–2).
//!
//! `finalize`, `roster-sync` and `deploy` act on every destination: the Solidity deployments
//! (Ethereum and BSC over JSON-RPC, TRON over the java-tron HTTP API, signed with an owner-only
//! secp256k1 key file) and the TON minter (over ADNL liteservers, sent from the user's `v5r1`
//! wallet with an owner-only Ed25519 key file). They read the deployment's roster state, verify
//! Taira's bundle or rotation chain locally and submit the destination call.
//!
//! `claim`, `lc-advance` and `lc-bootstrap` build Ethereum, BSC, TRON and TON light-client
//! evidence from the source chain's public RPC (for Ethereum also a beacon light-client API, for
//! TRON the java-tron HTTP API, for TON ADNL liteservers).
//!
//! TODO(ws42): control apply, deployment verify, governance show and bridge-key status/rotate.

mod evm;
mod governance;
mod ton;
mod tron;

use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    blocking::Client as BlockingClient,
    client::sccp::SccpAttestation,
    data_model::{
        bridge::SccpNetworkV1,
        isi::{InstructionBox, sccp::RecordSccpMessage},
        sccp::registry::SccpRouteActivationV1,
    },
};
use iroha_primitives::numeric::Numeric;

use crate::{Run, RunContext};

/// `iroha sccp` subcommands.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Show the Taira identity, SCCP parameters and attestation health.
    Info,
    /// List the routes with their revisions, escrows and liabilities.
    Routes,
    /// Record an outbound transfer from the configured account.
    Send(SendArgs),
    /// Show one outbound message record.
    Status(MessageArgs),
    /// Fetch the attestation proof bundle of one outbound message.
    Proof(ProofArgs),
    /// Show a roster generation (the current one by default).
    Roster(RosterArgs),
    /// Fetch the catch-up rotation chain from a generation.
    Rotations(RotationsArgs),
    /// Mint an attested outbound message on its EVM, TRON or TON destination.
    Finalize(FinalizeArgs),
    /// Rotate an EVM, TRON or TON destination's roster forward to Taira's current generation.
    RosterSync(RosterSyncArgs),
    /// SCCP proposals to the SORA Parliament.
    #[command(subcommand)]
    Governance(governance::Command),
    /// Deploy an EVM, TRON or TON destination pinned to a Taira generation and print its
    /// `RegisterRoute`.
    Deploy(DeployArgs),
    /// Prove an Ethereum, BSC or TRON (`transferToTaira`) or TON (`sccp_burn_to_taira`) burn on
    /// Taira and settle it.
    Claim(ClaimArgs),
    /// Advance Taira's Ethereum, BSC, TRON or TON light client to the latest finality.
    LcAdvance(LcAdvanceArgs),
    /// Build the Parliament `InitializeLightClient` action of the latest finalized source block.
    LcBootstrap(LcBootstrapArgs),
}

/// Arguments of `iroha sccp claim`.
#[derive(clap::Args, Debug)]
pub struct ClaimArgs {
    /// Source network (`ethereum-mainnet`, `bsc-mainnet`, `tron-mainnet`, `ton-mainnet`).
    #[arg(long)]
    pub network: String,
    /// Burn transaction: EVM `transferToTaira` tx hash, TRON tx id, or TON `<lt>:<hash>` of the
    /// minter transaction that emitted `sccp_transfer_to_taira`.
    #[arg(long)]
    pub tx_hash: String,
    /// JSON-RPC endpoint (EVM), java-tron HTTP API endpoint (TRON) or liteserver list (TON) of
    /// the source chain.
    #[arg(long)]
    pub rpc_url: String,
    /// Beacon API endpoint serving the light-client routes (Ethereum only).
    #[arg(long)]
    pub beacon_url: Option<String>,
}

/// Arguments of `iroha sccp lc-advance`.
#[derive(clap::Args, Debug)]
pub struct LcAdvanceArgs {
    /// Source network (`ethereum-mainnet`, `bsc-mainnet`, `tron-mainnet`, `ton-mainnet`).
    #[arg(long)]
    pub network: String,
    /// JSON-RPC endpoint (EVM), java-tron HTTP API endpoint (TRON) or liteserver list (TON) of
    /// the source chain.
    #[arg(long)]
    pub rpc_url: String,
    /// Beacon API endpoint serving the light-client routes (Ethereum only).
    #[arg(long)]
    pub beacon_url: Option<String>,
}

/// Arguments of `iroha sccp lc-bootstrap`.
#[derive(clap::Args, Debug)]
pub struct LcBootstrapArgs {
    /// Source network (`ethereum-mainnet`, `bsc-mainnet`, `tron-mainnet`, `ton-mainnet`).
    #[arg(long)]
    pub network: String,
    /// JSON-RPC endpoint (EVM), java-tron HTTP API endpoint (TRON) or liteserver list (TON) of
    /// the source chain.
    #[arg(long)]
    pub rpc_url: String,
    /// Beacon API endpoint serving the light-client routes (Ethereum only).
    #[arg(long)]
    pub beacon_url: Option<String>,
    /// Re-initialize a frozen or aged light client instead of installing a first one.
    #[arg(long)]
    pub reinitialize: bool,
}

/// Arguments of `iroha sccp deploy`.
#[derive(clap::Args, Debug)]
pub struct DeployArgs {
    /// Destination network (`ethereum-mainnet`, `bsc-mainnet`, `tron-mainnet`, `ton-mainnet`).
    #[arg(long)]
    pub network: String,
    /// Route revision the deployment serves (the route's next revision).
    #[arg(long)]
    pub revision: u32,
    /// Supply cap in XOR.
    #[arg(long)]
    pub max_supply: String,
    /// Taira roster generation to pin; the current one when absent.
    #[arg(long)]
    pub generation: Option<u64>,
    /// Creation bytecode from `scripts/contract_artifact_corridor.py build` (hex text); it must
    /// match the locked artifact of the network's compiler (EVM or TRON).
    #[arg(long)]
    pub creation_bytecode: Option<std::path::PathBuf>,
    /// TON: the Acton build directory holding `SccpTairaXorMinter.json`,
    /// `SccpTairaXorWallet.json` and `SccpConsumedBucket.json`.
    #[arg(long)]
    pub ton_build_dir: Option<std::path::PathBuf>,
    /// JSON-RPC endpoint (EVM), java-tron HTTP API endpoint (TRON), or comma-separated
    /// `<ip>:<port>:<base64 key>` liteservers (TON; empty for the compiled list).
    #[arg(long)]
    pub rpc_url: String,
    /// Owner-only key file of the deploying account (secp256k1; TON: the Ed25519 seed of the
    /// wallet).
    #[arg(long)]
    pub key_file: std::path::PathBuf,
    /// TON: the raw `0:<hex>` address of the deployed `v5r1` wallet the key file controls.
    #[arg(long)]
    pub ton_wallet: Option<String>,
}

/// Locked `SccpTairaXor` artifacts (`scripts/contract_tooling/artifact-lock.json`).
const ARTIFACT_LOCK: &str = include_str!("../../../../scripts/contract_tooling/artifact-lock.json");

/// The locked keccak-256 of the `SccpTairaXor` creation bytecode of `target` (`evm` or
/// `tron`).
fn locked_creation_keccak(target: &str) -> Result<[u8; 32]> {
    let lock: norito::json::Value =
        norito::json::from_str(ARTIFACT_LOCK).map_err(|error| eyre!("artifact lock: {error}"))?;
    let text = lock
        .get("targets")
        .and_then(|targets| targets.get(target))
        .and_then(|evm| evm.get("contracts"))
        .and_then(|contracts| contracts.get("contracts/evm/sccp/SccpTairaXor.sol:SccpTairaXor"))
        .and_then(|contract| contract.get("creation_bytecode_keccak256_hex"))
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("the artifact lock has no {target} creation hash"))?;
    parse_word(text)
}

/// Read creation bytecode hex text and check it against the lock of `target`.
fn locked_creation_bytecode(path: &std::path::Path, target: &str) -> Result<Vec<u8>> {
    let text =
        std::fs::read_to_string(path).wrap_err_with(|| format!("read {}", path.display()))?;
    let text = text.trim();
    let bytecode = hex::decode(text.strip_prefix("0x").unwrap_or(text))
        .wrap_err("creation bytecode is not hex")?;
    if iroha_sccp::v1::hashes::keccak256(&[&bytecode]) != locked_creation_keccak(target)? {
        return Err(eyre!(
            "the creation bytecode differs from the locked {target} SccpTairaXor artifact"
        ));
    }
    Ok(bytecode)
}

/// Arguments of `iroha sccp finalize`.
#[derive(clap::Args, Debug)]
pub struct FinalizeArgs {
    /// Message id (32-byte hex).
    #[arg(long)]
    pub message_id: String,
    /// JSON-RPC endpoint (EVM), java-tron HTTP API endpoint (TRON), or comma-separated
    /// liteservers (TON; empty for the compiled list).
    #[arg(long)]
    pub rpc_url: String,
    /// Owner-only key file of the account that pays for the call (secp256k1; TON: the Ed25519
    /// seed of the wallet).
    #[arg(long)]
    pub key_file: std::path::PathBuf,
    /// TON: the raw `0:<hex>` address of the deployed `v5r1` wallet the key file controls.
    #[arg(long)]
    pub ton_wallet: Option<String>,
    /// Attesting subject: `own`, `latest` or a Taira height.
    #[arg(long, default_value = "own")]
    pub attestation: String,
}

/// Arguments of `iroha sccp roster-sync`.
#[derive(clap::Args, Debug)]
pub struct RosterSyncArgs {
    /// External network of the deployment.
    #[arg(long)]
    pub network: String,
    /// Route revision of the deployment; defaults to the route's live revision.
    #[arg(long)]
    pub revision: Option<u32>,
    /// JSON-RPC endpoint (EVM), java-tron HTTP API endpoint (TRON), or comma-separated
    /// liteservers (TON; empty for the compiled list).
    #[arg(long)]
    pub rpc_url: String,
    /// Owner-only key file of the account that pays for the calls (secp256k1; TON: the Ed25519
    /// seed of the wallet).
    #[arg(long)]
    pub key_file: std::path::PathBuf,
    /// TON: the raw `0:<hex>` address of the deployed `v5r1` wallet the key file controls.
    #[arg(long)]
    pub ton_wallet: Option<String>,
}

/// Arguments of `iroha sccp send`.
#[derive(clap::Args, Debug)]
pub struct SendArgs {
    /// External target network (`ethereum-mainnet`, `bsc-mainnet`, `tron-mainnet`,
    /// `ton-mainnet`).
    #[arg(long)]
    pub network: String,
    /// XOR amount, for example `1.5`.
    #[arg(long)]
    pub amount: String,
    /// Recipient on the target: `0x`-hex (EVM 20 bytes, TRON 21 bytes with the `41` prefix) or
    /// `0:<64 hex>` (TON raw address).
    #[arg(long)]
    pub recipient: String,
    /// Route revision to record on; defaults to the route's `Bidirectional` revision.
    #[arg(long)]
    pub revision: Option<u32>,
}

/// Arguments naming one outbound message.
#[derive(clap::Args, Debug)]
pub struct MessageArgs {
    /// Message id (32-byte hex).
    #[arg(long)]
    pub message_id: String,
}

/// Arguments of `iroha sccp proof`.
#[derive(clap::Args, Debug)]
pub struct ProofArgs {
    /// Message id (32-byte hex).
    #[arg(long)]
    pub message_id: String,
    /// Attesting subject: `own`, `latest` or a Taira height.
    #[arg(long, default_value = "own")]
    pub attestation: String,
}

/// Arguments of `iroha sccp roster`.
#[derive(clap::Args, Debug)]
pub struct RosterArgs {
    /// Generation; the current one when absent.
    #[arg(long)]
    pub generation: Option<u64>,
}

/// Arguments of `iroha sccp rotations`.
#[derive(clap::Args, Debug)]
pub struct RotationsArgs {
    /// Generation the destination currently trusts.
    #[arg(long)]
    pub after_generation: u64,
    /// Steps per page (at most 16).
    #[arg(long, default_value_t = 16)]
    pub limit: usize,
}

/// Parse an external network profile key.
pub(crate) fn parse_network(text: &str) -> Result<SccpNetworkV1> {
    SccpNetworkV1::from_profile_key(text)
        .filter(|network| network.is_external())
        .ok_or_else(|| eyre!("`{text}` is not an external SCCP network profile"))
}

/// Parse a 32-byte hex identifier (optional `0x`).
pub(crate) fn parse_word(text: &str) -> Result<[u8; 32]> {
    let bytes =
        hex::decode(text.strip_prefix("0x").unwrap_or(text)).wrap_err("identifier is not hex")?;
    <[u8; 32]>::try_from(bytes).map_err(|_| eyre!("identifier must be 32 bytes"))
}

/// Parse `attestation` choices.
pub(crate) fn parse_attestation(text: &str) -> Result<SccpAttestation> {
    match text {
        "own" => Ok(SccpAttestation::Own),
        "latest" => Ok(SccpAttestation::Latest),
        height => height
            .parse()
            .map(SccpAttestation::At)
            .map_err(|_| eyre!("attestation must be `own`, `latest` or a height")),
    }
}

/// Encode `text` as the target network's recipient bytes (§3.1).
pub(crate) fn parse_recipient(network: SccpNetworkV1, text: &str) -> Result<Vec<u8>> {
    let bytes = if network == SccpNetworkV1::TonMainnet {
        let account = text
            .strip_prefix("0:")
            .ok_or_else(|| eyre!("a TON recipient is a raw `0:<64 hex>` address"))?;
        let id = hex::decode(account).wrap_err("TON account id is not hex")?;
        let mut bytes = vec![0_u8; 4];
        bytes.extend(id);
        bytes
    } else {
        hex::decode(text.strip_prefix("0x").unwrap_or(text)).wrap_err("recipient is not hex")?
    };
    let codec = iroha_sccp::v1::network::account_codec(network);
    if !iroha_sccp::v1::payload::is_valid_account(codec, &bytes) {
        return Err(eyre!(
            "recipient is not a valid {} account",
            network.profile_key()
        ));
    }
    Ok(bytes)
}

/// This machine's Unix time in milliseconds.
fn wall_clock_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

fn blocking<C: RunContext>(context: &C) -> Result<BlockingClient> {
    Ok(BlockingClient::from_client(context.client_from_config()?)?)
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Info => {
                let capabilities = blocking(context)?.sccp().capabilities()?;
                context.print_data(&capabilities)
            }
            Self::Routes => {
                let routes = blocking(context)?.sccp().registry()?;
                context.print_data(&routes)
            }
            Self::Send(args) => send(context, args),
            Self::Status(args) => {
                let record = blocking(context)?
                    .sccp()
                    .message(&parse_word(&args.message_id)?)?;
                context.print_data(&record)
            }
            Self::Proof(args) => {
                let bundle = blocking(context)?.sccp().message_proof(
                    &parse_word(&args.message_id)?,
                    parse_attestation(&args.attestation)?,
                )?;
                context.print_data(&bundle)
            }
            Self::Roster(args) => {
                let client = blocking(context)?;
                let roster = match args.generation {
                    Some(generation) => client.sccp().roster(generation)?,
                    None => client.sccp().current_roster()?,
                };
                context.print_data(&roster)
            }
            Self::Rotations(args) => {
                let chain = blocking(context)?
                    .sccp()
                    .rotations(args.after_generation, args.limit)?;
                context.print_data(&chain)
            }
            Self::Finalize(args) => finalize(context, args),
            Self::RosterSync(args) => roster_sync(context, args),
            Self::Governance(command) => command.run(context),
            Self::Deploy(args) => deploy(context, args),
            Self::Claim(args) => claim(context, args),
            Self::LcAdvance(args) => lc_advance(context, args),
            Self::LcBootstrap(args) => lc_bootstrap(context, args),
        }
    }
}

/// A Solidity deployment (EVM or TRON) reached through its chain's endpoint.
enum SolidityDestination {
    /// Ethereum or BSC.
    Evm(evm::EvmDestination),
    /// TRON.
    Tron(tron::TronDestination),
}

impl SolidityDestination {
    fn roster_generation(&self) -> Result<u64> {
        Ok(match self {
            Self::Evm(destination) => destination.roster_state()?.generation,
            Self::Tron(destination) => destination.roster_state()?.generation,
        })
    }

    fn finalize_call(
        &self,
        bundle: &iroha_sccp::api::SccpMessageProofBundleV1,
        taira_network_id: [u8; 32],
    ) -> Result<Vec<u8>> {
        match self {
            Self::Evm(destination) => destination.finalize_call(bundle, taira_network_id),
            Self::Tron(destination) => destination.finalize_call(bundle, taira_network_id),
        }
    }

    fn rotation_calls(
        &self,
        chain: &iroha_sccp::api::SccpRotationChainV1,
        target_generation: u64,
        taira_network_id: [u8; 32],
    ) -> Result<Vec<Vec<u8>>> {
        match self {
            Self::Evm(destination) => {
                destination.rotation_calls(chain, target_generation, taira_network_id)
            }
            Self::Tron(destination) => {
                destination.rotation_calls(chain, target_generation, taira_network_id)
            }
        }
    }

    fn send(
        &self,
        key: &iroha_sccp_wallet::pure::evm::EvmSigningKey,
        data: Vec<u8>,
    ) -> Result<[u8; 32]> {
        match self {
            Self::Evm(destination) => destination.send(key, data),
            Self::Tron(destination) => destination.send(key, data),
        }
    }
}

/// The deployment and destination identity of `(network, revision)` from Taira's registry (the
/// live revision when `revision` is `None`).
fn route_deployment(
    client: &BlockingClient,
    network: SccpNetworkV1,
    revision: Option<u32>,
) -> Result<(
    iroha::data_model::sccp::deployment::SccpDeploymentV1,
    iroha_sccp::v1::proof::DestinationV1,
)> {
    let route = client
        .sccp()
        .registry()?
        .into_iter()
        .find(|route| route.network == network)
        .ok_or_else(|| eyre!("Taira has no route to {}", network.profile_key()))?;
    let record = match revision {
        Some(revision) => route.revisions.get(&revision),
        None => route
            .revisions
            .values()
            .find(|record| record.activation.is_live()),
    }
    .ok_or_else(|| eyre!("the {} route has no such revision", network.profile_key()))?;
    Ok((
        record.deployment.clone(),
        iroha_sccp::v1::proof::DestinationV1 {
            network,
            route_revision: record.revision,
            destination_word: record.destination_word,
        },
    ))
}

/// Connect to the Solidity deployment of `(network, revision)` through `rpc_url`.
fn solidity_destination(
    client: &BlockingClient,
    network: SccpNetworkV1,
    revision: Option<u32>,
    rpc_url: &str,
) -> Result<SolidityDestination> {
    use iroha::data_model::sccp::deployment::SccpDeploymentV1;
    match route_deployment(client, network, revision)? {
        (SccpDeploymentV1::Evm(deployment), destination) => Ok(SolidityDestination::Evm(
            evm::EvmDestination::connect(rpc_url, network, deployment.address, destination)?,
        )),
        (SccpDeploymentV1::Tron(deployment), destination) => Ok(SolidityDestination::Tron(
            tron::TronDestination::connect(rpc_url, deployment.address, destination)?,
        )),
        (SccpDeploymentV1::Ton(_), _) => {
            Err(eyre!("{} is a TON destination", network.profile_key()))
        }
    }
}

/// Connect to the TON minter of `(network, revision)` through `liteservers` and load the wallet.
fn ton_destination(
    client: &BlockingClient,
    network: SccpNetworkV1,
    revision: Option<u32>,
    liteservers: &str,
    wallet: Option<&str>,
    key_file: &std::path::Path,
) -> Result<(ton::TonDestination, ton::TonWallet)> {
    use iroha::data_model::sccp::deployment::SccpDeploymentV1;
    let (SccpDeploymentV1::Ton(deployment), destination) =
        route_deployment(client, network, revision)?
    else {
        return Err(eyre!("{} is not a TON destination", network.profile_key()));
    };
    let destination =
        ton::TonDestination::connect(liteservers, deployment.master_account, destination)?;
    let wallet = ton::TonWallet::load(
        destination.lite(),
        wallet.ok_or_else(|| eyre!("TON destinations need `--ton-wallet 0:<hex>`"))?,
        key_file,
    )?;
    Ok((destination, wallet))
}

/// `iroha sccp finalize`: verify the bundle against the destination and mint.
fn finalize<C: RunContext>(context: &mut C, args: FinalizeArgs) -> Result<()> {
    let client = blocking(context)?;
    let message_id = parse_word(&args.message_id)?;
    let record = client.sccp().message(&message_id)?;
    let taira_network_id = client.sccp().capabilities()?.network_id;
    let bundle = client
        .sccp()
        .message_proof(&message_id, parse_attestation(&args.attestation)?)?;
    if record.network == SccpNetworkV1::TonMainnet {
        let (destination, wallet) = ton_destination(
            &client,
            record.network,
            Some(record.revision),
            &args.rpc_url,
            args.ton_wallet.as_deref(),
            &args.key_file,
        )?;
        let hash = destination.finalize(&bundle, taira_network_id, &wallet)?;
        return context.println(hex::encode(hash));
    }
    let destination = solidity_destination(
        &client,
        record.network,
        Some(record.revision),
        &args.rpc_url,
    )?;
    let calldata = destination.finalize_call(&bundle, taira_network_id)?;
    let key = evm::load_key(&args.key_file)?;
    let hash = destination.send(&key, calldata)?;
    context.println(format!("0x{}", hex::encode(hash)))
}

/// Poll `code_hash` every five seconds for up to five minutes until the deployed code appears.
fn wait_for_code(mut code_hash: impl FnMut() -> Result<[u8; 32]>) -> Result<[u8; 32]> {
    for _ in 0..60 {
        if let Ok(hash) = code_hash() {
            return Ok(hash);
        }
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
    Err(eyre!("the deployment was not mined within five minutes"))
}

/// Value sent with a TON minter deployment (its `sccp_init`), in nanotons: 0.5 TON.
const TON_DEPLOY_VALUE: u128 = 500_000_000;

/// The code cell of Acton artifact `<build_dir>/<name>.json` (`code_boc64`).
fn ton_code(build_dir: &std::path::Path, name: &str) -> Result<iroha_sccp::v1::ton_cell::Cell> {
    use base64::Engine as _;
    let path = build_dir.join(format!("{name}.json"));
    let text =
        std::fs::read_to_string(&path).wrap_err_with(|| format!("read {}", path.display()))?;
    let artifact: norito::json::Value =
        norito::json::from_str(&text).map_err(|error| eyre!("{}: {error}", path.display()))?;
    let encoded = artifact
        .get("code_boc64")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("{} has no `code_boc64`", path.display()))?;
    let boc = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| eyre!("{}: code_boc64: {error}", path.display()))?;
    iroha_sccp_wallet::pure::ton::code_cell(&boc)
        .map_err(|error| eyre!("{}: {error}", path.display()))
}

/// `iroha sccp deploy --network ton-mainnet`: deploy the minter pinned to a Taira generation
/// from the user's wallet (with `sccp_init`) and print its `RegisterRoute` action.
fn deploy_ton<C: RunContext>(context: &mut C, args: DeployArgs) -> Result<()> {
    use iroha::data_model::sccp::{
        deployment::{SccpDeploymentV1, SccpTonCodeRefV1, SccpTonDeploymentV1},
        governance::{SccpGovernanceActionV1, SccpRegisterRouteActionV1},
    };
    use iroha_sccp::v1::ton_cell::{TonMinterInitV1, minter_deployment_data, state_init};
    use iroha_sccp_wallet::pure::ton::{
        SEND_MODE_PAY_FEES_SEPARATELY_IGNORE_ERRORS, init_body, internal_message,
    };
    let build_dir = args
        .ton_build_dir
        .as_deref()
        .ok_or_else(|| eyre!("TON deployments need `--ton-build-dir`"))?;
    let minter_code = ton_code(build_dir, "SccpTairaXorMinter")?;
    let wallet_code = ton_code(build_dir, "SccpTairaXorWallet")?;
    let bucket_code = ton_code(build_dir, "SccpConsumedBucket")?;
    let code_ref = |cell: &iroha_sccp::v1::ton_cell::Cell| SccpTonCodeRefV1 {
        hash: *cell.hash(),
        depth: cell.depth(),
    };
    let max_supply: Numeric = args.max_supply.parse().map_err(|_| {
        eyre!(
            "max supply `{}` is not a decimal XOR amount",
            args.max_supply
        )
    })?;
    let max_wrapped_supply = iroha_sccp::v1::amount::taira_units(&max_supply)
        .map_err(|error| eyre!("max supply: {error}"))?;
    let client = blocking(context)?;
    let taira_network_id = client.sccp().capabilities()?.network_id;
    let view = match args.generation {
        Some(generation) => client.sccp().roster(generation)?,
        None => client.sccp().current_roster()?,
    };
    let roster = view
        .to_roster()
        .map_err(|error| eyre!("Taira served an invalid roster: {error:?}"))?;
    if roster.digest(&taira_network_id).ok() != Some(view.digest) {
        return Err(eyre!("Taira served a roster whose digest does not match"));
    }
    let init = TonMinterInitV1 {
        taira_network_id,
        route_revision: args.revision,
        max_supply: max_wrapped_supply,
        roster: roster.clone(),
        wallet_code: code_ref(&wallet_code),
        bucket_code: code_ref(&bucket_code),
    };
    let data = minter_deployment_data(&init, wallet_code.clone(), bucket_code.clone())
        .map_err(|error| eyre!("minter data: {error}"))?;
    let state = state_init(minter_code.clone(), data.root)
        .map_err(|error| eyre!("minter StateInit: {error}"))?;
    let minter = *state.hash();
    let lite = ton::connect(&args.rpc_url)?;
    let wallet = ton::TonWallet::load(
        &lite,
        args.ton_wallet
            .as_deref()
            .ok_or_else(|| eyre!("TON deployments need `--ton-wallet 0:<hex>`"))?,
        &args.key_file,
    )?;
    let message = internal_message(
        &minter,
        TON_DEPLOY_VALUE,
        false,
        init_body(0).map_err(|error| eyre!("sccp_init: {error}"))?,
        Some(state),
    )
    .map_err(|error| eyre!("deployment message: {error}"))?;
    let hash = wallet.send(
        &lite,
        &[(message, SEND_MODE_PAY_FEES_SEPARATELY_IGNORE_ERRORS)],
    )?;
    context.println(format!(
        "deployment message {}; minter 0:{}",
        hex::encode(hash),
        hex::encode(minter)
    ))?;
    let action = SccpGovernanceActionV1::RegisterRoute(SccpRegisterRouteActionV1 {
        network: SccpNetworkV1::TonMainnet,
        revision: args.revision,
        deployment: SccpDeploymentV1::Ton(SccpTonDeploymentV1 {
            master_account: minter,
            minter_code: code_ref(&minter_code),
            wallet_code: code_ref(&wallet_code),
            bucket_code: code_ref(&bucket_code),
        }),
        max_wrapped_supply,
        initial_roster_generation: roster.generation,
    });
    context.print_data(&vec![action])
}

/// `iroha sccp deploy`: deploy a pinned EVM, TRON or TON destination and print its
/// `RegisterRoute` action.
fn deploy<C: RunContext>(context: &mut C, args: DeployArgs) -> Result<()> {
    use iroha::data_model::sccp::{
        deployment::{SccpDeploymentV1, SccpEvmDeploymentV1, SccpTronDeploymentV1},
        governance::{SccpGovernanceActionV1, SccpRegisterRouteActionV1},
    };
    let network = parse_network(&args.network)?;
    let target = match network {
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet => "evm",
        SccpNetworkV1::TronMainnet => "tron",
        SccpNetworkV1::TonMainnet => return deploy_ton(context, args),
        SccpNetworkV1::SoraTaira => {
            return Err(eyre!("{} is not a destination", network.profile_key()));
        }
    };
    let bytecode = locked_creation_bytecode(
        args.creation_bytecode
            .as_deref()
            .ok_or_else(|| eyre!("Solidity deployments need `--creation-bytecode`"))?,
        target,
    )?;
    let max_supply: Numeric = args.max_supply.parse().map_err(|_| {
        eyre!(
            "max supply `{}` is not a decimal XOR amount",
            args.max_supply
        )
    })?;
    let max_wrapped_supply = iroha_sccp::v1::amount::taira_units(&max_supply)
        .map_err(|error| eyre!("max supply: {error}"))?;
    let client = blocking(context)?;
    let taira_network_id = client.sccp().capabilities()?.network_id;
    let view = match args.generation {
        Some(generation) => client.sccp().roster(generation)?,
        None => client.sccp().current_roster()?,
    };
    let roster = view
        .to_roster()
        .map_err(|error| eyre!("Taira served an invalid roster: {error:?}"))?;
    if roster.digest(&taira_network_id).ok() != Some(view.digest) {
        return Err(eyre!("Taira served a roster whose digest does not match"));
    }
    let mut data = bytecode;
    data.extend(evm::constructor_args(
        &taira_network_id,
        network,
        args.revision,
        max_wrapped_supply,
        &roster,
    ));
    let key = evm::load_key(&args.key_file)?;
    // Deploy, then wait for the code to appear and print the Parliament action.
    let deployment = if network == SccpNetworkV1::TronMainnet {
        let api = tron::connect_api(&args.rpc_url)?;
        let (id, address) = tron::deploy(&api, &key, data)?;
        context.println(format!(
            "deployment transaction {}; contract {}",
            hex::encode(id),
            hex::encode(address)
        ))?;
        let runtime_code_hash = wait_for_code(|| tron::runtime_code_hash(&api, &address))?;
        SccpDeploymentV1::Tron(SccpTronDeploymentV1 {
            address,
            runtime_code_hash,
        })
    } else {
        let chain = evm::connect_chain(&args.rpc_url, network)?;
        let (hash, nonce) = evm::send_transaction(&chain, network, &key, None, data)?;
        let address = evm::created_address(&key.address(), nonce);
        context.println(format!(
            "deployment transaction 0x{}; contract 0x{}",
            hex::encode(hash),
            hex::encode(address)
        ))?;
        let runtime_code_hash = wait_for_code(|| evm::runtime_code_hash(&chain, &address))?;
        SccpDeploymentV1::Evm(SccpEvmDeploymentV1 {
            address,
            runtime_code_hash,
        })
    };
    let action = SccpGovernanceActionV1::RegisterRoute(SccpRegisterRouteActionV1 {
        network,
        revision: args.revision,
        deployment,
        max_wrapped_supply,
        initial_roster_generation: roster.generation,
    });
    context.print_data(&vec![action])
}

/// The Ethereum evidence builder over `rpc_url` and `beacon_url`.
fn ethereum_builder(
    rpc_url: &str,
    beacon_url: Option<&str>,
) -> Result<iroha_sccp_rpc::builders::ethereum::EthereumBuilder> {
    use iroha_sccp_rpc::{BeaconClient, EndpointSet, FailoverPolicy, HttpConfig, HttpTransport};
    let beacon_url = beacon_url.ok_or_else(|| eyre!("Ethereum evidence needs `--beacon-url`"))?;
    let execution = evm::connect_chain(rpc_url, SccpNetworkV1::EthereumMainnet)?;
    let endpoints = EndpointSet::parse(&[beacon_url], &[])
        .map_err(|error| eyre!("beacon endpoint: {error}"))?;
    let transport = HttpTransport::new(endpoints, HttpConfig::default(), FailoverPolicy::default())
        .map_err(|error| eyre!("beacon transport: {error}"))?;
    Ok(iroha_sccp_rpc::builders::ethereum::EthereumBuilder::new(
        BeaconClient::new(transport),
        execution,
    ))
}

/// `iroha sccp claim` for TON: `--tx-hash <lt>:<hash hex>` names the minter transaction whose
/// external message 0 is the `sccp_transfer_to_taira` event; the proof hangs from a masterchain
/// block signed by the epoch of Taira's newest key block.
fn ton_claim(
    client: &BlockingClient,
    args: &ClaimArgs,
) -> Result<(
    u32,
    Vec<u8>,
    iroha::data_model::sccp::inbound::SccpSourceProofBytesV1,
)> {
    use iroha::data_model::sccp::deployment::SccpDeploymentV1;
    use iroha_sccp::v1::payload::SccpTransferPayloadV1;
    use iroha_sccp_rpc::builders::ton::TonBuilder;
    let (lt, hash) = args
        .tx_hash
        .split_once(':')
        .ok_or_else(|| eyre!("a TON transaction is `<lt>:<hash hex>`"))?;
    let lt: u64 = lt
        .parse()
        .map_err(|_| eyre!("the transaction lt is not a number"))?;
    let hash = parse_word(hash)?;
    let (SccpDeploymentV1::Ton(deployment), _) =
        route_deployment(client, SccpNetworkV1::TonMainnet, None)?
    else {
        return Err(eyre!("the TON route has no TON deployment"));
    };
    let minter = deployment.master_account;
    let light_client = client
        .sccp()
        .light_clients()?
        .into_iter()
        .find(|light_client| light_client.params.network == SccpNetworkV1::TonMainnet)
        .ok_or_else(|| eyre!("Taira has no TON light client"))?;
    let key_block = u32::try_from(light_client.head.latest_set_id)
        .map_err(|_| eyre!("the TON key block seqno overflows"))?;
    let builder = TonBuilder::new(ton::connect(&args.rpc_url)?);
    let payload = builder
        .transfer_payload(minter, lt, hash, 0)
        .map_err(|error| eyre!("reading the burn: {error}"))?;
    let revision = SccpTransferPayloadV1::decode(&payload)
        .map_err(|error| eyre!("the burned payload does not decode: {error}"))?
        .route_revision;
    let proof = builder
        .source_proof(minter, lt, hash, 0, key_block)
        .map_err(|error| eyre!("building the proof: {error}"))?;
    Ok((revision, payload, proof))
}

/// `iroha sccp claim` for TRON: the payload comes from the burn's `SccpTransferToTaira` log
/// (read from the solidified transaction info), the proof from the call itself.
fn tron_claim(
    args: &ClaimArgs,
) -> Result<(
    u32,
    Vec<u8>,
    iroha::data_model::sccp::inbound::SccpSourceProofBytesV1,
)> {
    use iroha_sccp::v1::{evm_abi::TransferToTairaLogV1, payload::SccpTransferPayloadV1};
    use iroha_sccp_rpc::builders::tron::TronBuilder;
    let tx_id = parse_word(&args.tx_hash)?;
    let api = tron::connect_api(&args.rpc_url)?;
    let info = api
        .solidity_transaction_info(&tx_id)
        .map_err(|error| eyre!("gettransactioninfobyid: {error}"))?
        .ok_or_else(|| eyre!("the transaction is not solidified yet"))?;
    let log = info
        .logs
        .iter()
        .find_map(|log| TransferToTairaLogV1::decode(&log.topics, &log.data).ok())
        .ok_or_else(|| eyre!("the transaction emitted no SccpTransferToTaira log"))?;
    let payload = SccpTransferPayloadV1::decode(&log.payload)
        .map_err(|error| eyre!("the burned payload does not decode: {error}"))?;
    let proof = TronBuilder::new(api)
        .source_proof(&tx_id)
        .map_err(|error| eyre!("building the proof: {error}"))?;
    Ok((payload.route_revision, log.payload, proof))
}

/// `iroha sccp claim`: prove a source-chain burn and submit `SubmitSccpInboundMessageV1`.
fn claim<C: RunContext>(context: &mut C, args: ClaimArgs) -> Result<()> {
    use iroha::data_model::isi::sccp::SubmitSccpInboundMessageV1;
    use iroha_sccp::v1::{evm_abi::TransferToTairaLogV1, payload::SccpTransferPayloadV1};
    use iroha_sccp_rpc::builders::{bsc::BscBuilder, ethereum::EthereumEventV1};
    let network = parse_network(&args.network)?;
    if matches!(
        network,
        SccpNetworkV1::TronMainnet | SccpNetworkV1::TonMainnet
    ) {
        let (revision, payload, proof) = if network == SccpNetworkV1::TronMainnet {
            tron_claim(&args)?
        } else {
            ton_claim(&blocking(context)?, &args)?
        };
        return context.finish(vec![InstructionBox::from(SubmitSccpInboundMessageV1 {
            network,
            revision,
            payload,
            proof,
        })]);
    }
    let tx_hash = parse_word(&args.tx_hash)?;
    let execution = evm::connect_chain(&args.rpc_url, network)?;
    let receipt = execution
        .transaction_receipt(&tx_hash)
        .map_err(|error| eyre!("eth_getTransactionReceipt: {error}"))?
        .ok_or_else(|| eyre!("the transaction is not mined"))?;
    let (log_index, log) = receipt
        .logs
        .iter()
        .enumerate()
        .find_map(|(index, log)| {
            TransferToTairaLogV1::decode(&log.topics, &log.data)
                .ok()
                .map(|decoded| (index, decoded))
        })
        .ok_or_else(|| eyre!("the transaction emitted no SccpTransferToTaira log"))?;
    let payload = SccpTransferPayloadV1::decode(&log.payload)
        .map_err(|error| eyre!("the burned payload does not decode: {error}"))?;
    let event = EthereumEventV1::TransferToTaira {
        log_index: u32::try_from(log_index).map_err(|_| eyre!("log index overflows"))?,
    };
    let proof = if network == SccpNetworkV1::BscMainnet {
        let sets = blocking(context)?.sccp().light_client_sets(network)?;
        BscBuilder::new(execution).source_proof(&tx_hash, event, &sets)
    } else {
        ethereum_builder(&args.rpc_url, args.beacon_url.as_deref())?.source_proof(&tx_hash, event)
    }
    .map_err(|error| eyre!("building the proof: {error}"))?;
    context.finish(vec![InstructionBox::from(SubmitSccpInboundMessageV1 {
        network,
        revision: payload.route_revision,
        payload: log.payload,
        proof,
    })])
}

/// `iroha sccp lc-advance`: advance Taira's light client of a source chain to its latest
/// finality.
fn lc_advance<C: RunContext>(context: &mut C, args: LcAdvanceArgs) -> Result<()> {
    use iroha::data_model::isi::sccp::AdvanceSccpLightClientV1;
    use iroha_sccp_rpc::builders::{bsc::BscBuilder, tron::TronBuilder};
    let network = parse_network(&args.network)?;
    let light_client = blocking(context)?
        .sccp()
        .light_clients()?
        .into_iter()
        .find(|light_client| light_client.params.network == network)
        .ok_or_else(|| eyre!("Taira has no {} light client", network.profile_key()))?;
    let latest = light_client.head.latest_set_id;
    let max_updates =
        usize::try_from(light_client.params.max_updates_per_advance).unwrap_or(usize::MAX);
    let advance = match network {
        SccpNetworkV1::BscMainnet => BscBuilder::new(evm::connect_chain(&args.rpc_url, network)?)
            .advance(latest, max_updates),
        SccpNetworkV1::TronMainnet => {
            TronBuilder::new(tron::connect_api(&args.rpc_url)?).advance(latest, max_updates)
        }
        SccpNetworkV1::TonMainnet => {
            iroha_sccp_rpc::builders::ton::TonBuilder::new(ton::connect(&args.rpc_url)?)
                .advance(latest, max_updates)
        }
        _ => ethereum_builder(&args.rpc_url, args.beacon_url.as_deref())?
            .advance(latest, max_updates),
    }
    .map_err(|error| eyre!("building the advance: {error}"))?;
    context.finish(vec![InstructionBox::from(AdvanceSccpLightClientV1 {
        network,
        expected_state_hash: Some(light_client.state_hash),
        advance,
    })])
}

/// `iroha sccp lc-bootstrap`: print the `InitializeLightClient` action of the latest finalized
/// source block with the default params, for `iroha sccp governance propose --actions`.
fn lc_bootstrap<C: RunContext>(context: &mut C, args: LcBootstrapArgs) -> Result<()> {
    use iroha::data_model::sccp::{
        governance::{SccpGovernanceActionV1, SccpInitializeLightClientActionV1},
        light_client::{SccpLcInitExpectationV1, SccpLightClientParamsV1},
    };
    use iroha_sccp_rpc::builders::{bsc::BscBuilder, tron::TronBuilder};
    let network = parse_network(&args.network)?;
    let bootstrap = match network {
        SccpNetworkV1::BscMainnet => {
            BscBuilder::new(evm::connect_chain(&args.rpc_url, network)?).bootstrap()
        }
        SccpNetworkV1::TronMainnet => {
            TronBuilder::new(tron::connect_api(&args.rpc_url)?).bootstrap()
        }
        SccpNetworkV1::TonMainnet => {
            iroha_sccp_rpc::builders::ton::TonBuilder::new(ton::connect(&args.rpc_url)?).bootstrap()
        }
        _ => ethereum_builder(&args.rpc_url, args.beacon_url.as_deref())?.finalized_bootstrap(),
    }
    .map_err(|error| eyre!("building the bootstrap: {error}"))?;
    let params = SccpLightClientParamsV1::defaults_for(network)
        .ok_or_else(|| eyre!("{} has no light-client defaults", network.profile_key()))?;
    iroha_sccp::light_client::verify_bootstrap(network, &params, &bootstrap, wall_clock_ms())
        .map_err(|error| eyre!("the built bootstrap does not verify: {error}"))?;
    let action = SccpGovernanceActionV1::InitializeLightClient(SccpInitializeLightClientActionV1 {
        network,
        expected: if args.reinitialize {
            SccpLcInitExpectationV1::Unusable
        } else {
            SccpLcInitExpectationV1::Absent
        },
        params,
        bootstrap,
    });
    context.print_data(&vec![action])
}

/// `iroha sccp roster-sync`: rotate the destination to Taira's current generation.
fn roster_sync<C: RunContext>(context: &mut C, args: RosterSyncArgs) -> Result<()> {
    let client = blocking(context)?;
    let network = parse_network(&args.network)?;
    if network == SccpNetworkV1::TonMainnet {
        let (destination, wallet) = ton_destination(
            &client,
            network,
            args.revision,
            &args.rpc_url,
            args.ton_wallet.as_deref(),
            &args.key_file,
        )?;
        let capabilities = client.sccp().capabilities()?;
        let from = destination.roster_state()?.generation;
        let chain = client
            .sccp()
            .rotations(from, iroha_sccp::v1::constants::MAX_ROTATIONS_PER_CALL)?;
        let sent = destination.rotate(
            &chain,
            capabilities.current_generation,
            capabilities.network_id,
            &wallet,
        )?;
        if sent.is_empty() {
            return context.println("the destination roster is current");
        }
        for hash in sent {
            context.println(hex::encode(hash))?;
        }
        return Ok(());
    }
    let destination = solidity_destination(&client, network, args.revision, &args.rpc_url)?;
    let capabilities = client.sccp().capabilities()?;
    let from = destination.roster_generation()?;
    let chain = client
        .sccp()
        .rotations(from, iroha_sccp::v1::constants::MAX_ROTATIONS_PER_CALL)?;
    let calls = destination.rotation_calls(
        &chain,
        capabilities.current_generation,
        capabilities.network_id,
    )?;
    if calls.is_empty() {
        return context.println("the destination roster is current");
    }
    let key = evm::load_key(&args.key_file)?;
    for call in calls {
        let hash = destination.send(&key, call)?;
        context.println(format!("0x{}", hex::encode(hash)))?;
    }
    Ok(())
}

/// `iroha sccp send`: check the route (§7.1 steps 1–2) and record the transfer.
fn send<C: RunContext>(context: &mut C, args: SendArgs) -> Result<()> {
    let network = parse_network(&args.network)?;
    let recipient = parse_recipient(network, &args.recipient)?;
    let amount: Numeric = args
        .amount
        .parse()
        .map_err(|_| eyre!("amount `{}` is not a decimal XOR amount", args.amount))?;
    iroha_sccp::v1::amount::taira_units(&amount)
        .map_err(|error| eyre!("amount `{}`: {error}", args.amount))?;
    let client = blocking(context)?;
    let route = client
        .sccp()
        .registry()?
        .into_iter()
        .find(|route| route.network == network)
        .ok_or_else(|| eyre!("Taira has no route to {}", network.profile_key()))?;
    let live = route
        .revisions
        .values()
        .find(|revision| revision.activation == SccpRouteActivationV1::Bidirectional)
        .ok_or_else(|| {
            eyre!(
                "the {} route has no Bidirectional revision",
                network.profile_key()
            )
        })?;
    if args
        .revision
        .is_some_and(|revision| revision != live.revision)
    {
        return Err(eyre!(
            "revision {} is not the Bidirectional revision {}",
            args.revision.unwrap_or_default(),
            live.revision
        ));
    }
    if live.destination_paused {
        return Err(eyre!("the destination is paused by the Parliament"));
    }
    let instruction = RecordSccpMessage {
        network,
        expected_revision: live.revision,
        amount,
        recipient,
    };
    context.finish(vec![InstructionBox::from(instruction)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipients_are_encoded_per_target_codec() {
        let evm = parse_recipient(
            SccpNetworkV1::EthereumMainnet,
            &format!("0x{}", "11".repeat(20)),
        )
        .expect("evm");
        assert_eq!(evm, vec![0x11; 20]);
        assert!(parse_recipient(SccpNetworkV1::EthereumMainnet, &"00".repeat(20)).is_err());
        let tron = parse_recipient(
            SccpNetworkV1::TronMainnet,
            &format!("41{}", "22".repeat(20)),
        )
        .expect("tron");
        assert_eq!(tron.len(), 21);
        let ton = parse_recipient(SccpNetworkV1::TonMainnet, &format!("0:{}", "33".repeat(32)))
            .expect("ton");
        assert_eq!(&ton[..4], &[0; 4]);
        assert_eq!(ton.len(), 36);
        assert!(parse_recipient(SccpNetworkV1::TonMainnet, &"33".repeat(32)).is_err());
    }

    #[test]
    fn networks_words_and_attestations_parse() {
        assert_eq!(
            parse_network("bsc-mainnet").expect("bsc"),
            SccpNetworkV1::BscMainnet
        );
        assert!(parse_network("sora-taira").is_err());
        assert_eq!(parse_word(&"ab".repeat(32)).expect("word"), [0xab; 32]);
        assert!(parse_word("ab").is_err());
        assert_eq!(parse_attestation("own").expect("own"), SccpAttestation::Own);
        assert_eq!(
            parse_attestation("9").expect("height"),
            SccpAttestation::At(9)
        );
        assert!(parse_attestation("later").is_err());
    }

    #[test]
    fn sccp_commands_parse() {
        use clap::Parser as _;
        #[derive(clap::Parser, Debug)]
        struct Cli {
            #[command(subcommand)]
            command: Command,
        }
        let parsed = Cli::try_parse_from([
            "sccp",
            "send",
            "--network",
            "ethereum-mainnet",
            "--amount",
            "1.5",
            "--recipient",
            "0x1111111111111111111111111111111111111111",
        ])
        .expect("send");
        assert!(matches!(parsed.command, Command::Send(_)));
        Cli::try_parse_from(["sccp", "rotations", "--after-generation", "3"]).expect("rotations");
        Cli::try_parse_from(["sccp", "proof", "--message-id", "00"]).expect("proof");
    }
}
