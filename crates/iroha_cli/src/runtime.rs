//! Runtime helpers for the CLI: ABI queries, status and governed runtime change sets
//! (`specs/live_runtime_upgrades.md` §6).
use crate::{CliOutputFormat, Run, RunContext};
use eyre::{Result, WrapErr as _, eyre};
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Runtime ABI helpers
    #[command(subcommand)]
    Abi(AbiCommand),
    /// Governed runtime change sets
    #[command(subcommand)]
    Upgrade(UpgradeCommand),
    /// Emergency containment by the runtime emergency panel
    #[command(subcommand)]
    Contain(ContainCommand),
    /// Show this node's release and whether it executes the committed selection and the
    /// successor of each scheduled change set
    Release,
    /// Show runtime metrics/status summary
    Status,
    /// Fetch node capability advert (ABI + crypto manifest)
    Capabilities,
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Abi(cmd) => cmd.run(context),
            Command::Upgrade(cmd) => cmd.run(context),
            Command::Contain(cmd) => cmd.run(context),
            Command::Release => {
                let client = context.client_from_config()?;
                let value = client.get_runtime_release_json()?;
                context.print_data(&value)
            }
            Command::Status => {
                let client = context.client_from_config()?;
                let value = client.get_runtime_metrics_json()?;
                // Compose a short one-line summary
                let abi_version = value
                    .get("abi_version")
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or(0);
                let up = value
                    .get("upgrade_events_total")
                    .and_then(|v| v.as_object());
                let count = |status: &str| {
                    up.and_then(|m| m.get(status))
                        .and_then(norito::json::Value::as_u64)
                        .unwrap_or(0)
                };
                let summary = format!(
                    "runtime: abi_version={abi_version} scheduled={} activated={} lapsed={} canceled={}",
                    count("scheduled"),
                    count("activated"),
                    count("lapsed"),
                    count("canceled"),
                );
                match context.output_format() {
                    CliOutputFormat::Text => context.println(summary)?,
                    CliOutputFormat::Json => context.print_data(&value)?,
                }
                Ok(())
            }
            Command::Capabilities => {
                let client = context.client_from_config()?;
                let value = client.get_node_capabilities_json()?;
                let sm_enabled = value
                    .get("crypto")
                    .and_then(|v| v.get("sm"))
                    .and_then(|v| v.get("enabled"))
                    .and_then(norito::json::Value::as_bool)
                    .unwrap_or(false);
                let policy = value
                    .get("crypto")
                    .and_then(|v| v.get("sm"))
                    .and_then(|v| v.get("acceleration"))
                    .and_then(|v| v.get("policy"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let openssl_preview = value
                    .get("crypto")
                    .and_then(|v| v.get("sm"))
                    .and_then(|v| v.get("openssl_preview"))
                    .and_then(norito::json::Value::as_bool)
                    .unwrap_or(false);
                let summary = format!(
                    "node capabilities: sm_enabled={sm_enabled} policy={policy} openssl_preview={openssl_preview}"
                );
                match context.output_format() {
                    CliOutputFormat::Text => context.println(summary)?,
                    CliOutputFormat::Json => context.print_data(&value)?,
                }
                Ok(())
            }
        }
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum AbiCommand {
    /// Fetch the active ABI version from the node
    Active,
    /// Fetch the active ABI version via signed Norito query (core /query)
    ActiveQuery,
    /// Fetch the node's canonical ABI hash for the active policy
    Hash,
}
impl Run for AbiCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            AbiCommand::Active => {
                let client = context.client_from_config()?;
                let value = client.get_runtime_abi_active_json()?;
                context.print_data(&value)
            }
            AbiCommand::ActiveQuery => {
                let client = context.client_from_config()?;
                let out: iroha::data_model::query::runtime::AbiVersion = client
                    .query_single(iroha::data_model::query::runtime::prelude::FindAbiVersion)?;
                context.print_data(&out)
            }
            AbiCommand::Hash => {
                let client = context.client_from_config()?;
                let value = client.get_runtime_abi_hash_json()?;
                context.print_data(&value)
            }
        }
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum UpgradeCommand {
    /// Show the committed selection, the attesting committee and every recorded change set
    List,
    /// Submit the Parliament proposal that schedules a change set, with the trusted release
    /// signers' endorsements the committed provenance policy requires
    Propose(ProposeArgs),
    /// Submit the Parliament proposal that withdraws a scheduled change set
    Cancel(CancelArgs),
    /// Endorse a change set's provenance as a trusted release signer
    Endorse(EndorseChangeSetArgs),
    /// Have the node sign its support for a scheduled change set with its validator key, then
    /// submit the attestation
    Attest(AttestArgs),
}
impl Run for UpgradeCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            UpgradeCommand::List => {
                let client = context.client_from_config()?;
                let value = client.get_runtime_upgrades_json()?;
                context.print_data(&value)
            }
            UpgradeCommand::Propose(args) => args.run(context),
            UpgradeCommand::Cancel(args) => args.run(context),
            UpgradeCommand::Endorse(args) => args.run(context),
            UpgradeCommand::Attest(args) => args.run(context),
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct ProposeArgs {
    /// Path to a JSON file with the `ChangeSetV1` to schedule
    #[arg(long, value_name = "PATH")]
    file: std::path::PathBuf,
    /// Path to a JSON file with one `ChangeSetEndorsementV1`; repeat for each trusted release
    /// signer. A root without a committed provenance policy takes none
    #[arg(long = "endorsement", value_name = "PATH")]
    endorsements: Vec<std::path::PathBuf>,
}
impl Run for ProposeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use iroha::data_model::runtime::lifecycle::{
            ChangeSetEndorsementV1, ChangeSetV1, RuntimeUpgradeActionV1,
        };
        let change_set: ChangeSetV1 = read_json(&self.file, "change set")?;
        let provenance = self
            .endorsements
            .iter()
            .map(|path| read_json::<ChangeSetEndorsementV1>(path, "endorsement"))
            .collect::<Result<Vec<_>>>()?;
        context.finish(runtime_proposal(
            RuntimeUpgradeActionV1::Schedule(change_set),
            provenance,
        ))
    }
}
#[derive(clap::Args, Debug)]
pub struct CancelArgs {
    /// Change-set id (hex)
    #[arg(long, value_name = "HEX")]
    id: String,
}
impl Run for CancelArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use iroha::data_model::runtime::lifecycle::{ChangeSetId, RuntimeUpgradeActionV1};
        let id = ChangeSetId(decode_hex_32("change-set id", &self.id)?);
        context.finish(runtime_proposal(
            RuntimeUpgradeActionV1::Cancel(id),
            Vec::new(),
        ))
    }
}
/// The SORA Parliament proposal that carries `action` and its endorsements
/// (`specs/live_runtime_upgrades.md` §6); the node admits it when the transaction executes.
fn runtime_proposal(
    action: iroha::data_model::runtime::lifecycle::RuntimeUpgradeActionV1,
    provenance: Vec<iroha::data_model::runtime::lifecycle::ChangeSetEndorsementV1>,
) -> Vec<iroha::data_model::isi::InstructionBox> {
    use iroha::data_model::isi::{InstructionBox, governance::ProposeRuntimeUpgradeProposal};
    vec![InstructionBox::from(ProposeRuntimeUpgradeProposal {
        action,
        provenance,
    })]
}
#[derive(clap::Args, Debug)]
pub struct EndorseChangeSetArgs {
    /// Path to a JSON file with the `ChangeSetV1` to endorse
    #[arg(long, value_name = "PATH")]
    file: std::path::PathBuf,
    /// File holding the release signer's private key (multihash hex); it is read at run time
    /// and never printed
    #[arg(long, value_name = "PATH")]
    signer_key_file: std::path::PathBuf,
}
impl Run for EndorseChangeSetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use iroha::data_model::runtime::lifecycle::{ChangeSetEndorsementV1, ChangeSetV1};
        let change_set: ChangeSetV1 = read_json(&self.file, "change set")?;
        let id = change_set
            .id()
            .map_err(|error| eyre!("change-set identity: {error}"))?;
        let key = read_private_key(&self.signer_key_file)?;
        let signer = iroha::crypto::KeyPair::from(key.clone())
            .public_key()
            .clone();
        let endorsement = ChangeSetEndorsementV1::sign(id, signer, &key)
            .map_err(|error| eyre!("failed to sign the endorsement: {error}"))?;
        context.print_data(&endorsement)
    }
}
#[derive(clap::Args, Debug)]
pub struct AttestArgs {
    /// Change-set id (hex)
    #[arg(long, value_name = "HEX")]
    id: String,
}
impl Run for AttestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let id = decode_hex_32("change-set id", &self.id)?;
        let client = context.client_from_config()?;
        // The node decides support and signs with its own validator key; the operator only
        // relays the attestation (`specs/live_runtime_upgrades.md` §6, "Attest").
        let draft = client.post_runtime_attest_upgrade_json(&hex::encode(id))?;
        context.finish(drafted_attestation(&draft)?)
    }
}
/// The one `AttestRuntimeSupportV1` instruction a node drafted.
fn drafted_attestation(
    draft: &norito::json::Value,
) -> Result<Vec<iroha::data_model::isi::InstructionBox>> {
    use iroha::data_model::isi::runtime_upgrade::AttestRuntimeSupportV1;
    let [instruction] = draft
        .get("tx_instructions")
        .and_then(norito::json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    else {
        return Err(eyre!("the node did not draft exactly one instruction"));
    };
    let field = |name: &str| {
        instruction
            .get(name)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("the drafted instruction has no {name}"))
    };
    let wire_id = field("wire_id")?;
    if wire_id != AttestRuntimeSupportV1::WIRE_ID {
        return Err(eyre!(
            "the node drafted {wire_id}, not a runtime support attestation"
        ));
    }
    let payload = hex::decode(field("payload_hex")?)
        .map_err(|_| eyre!("the drafted instruction payload is not hex"))?;
    let instruction = iroha::data_model::isi::decode_instruction_from_pair(wire_id, &payload)
        .map_err(|error| eyre!("malformed drafted attestation: {error}"))?;
    Ok(vec![instruction])
}
#[derive(clap::Subcommand, Debug)]
pub enum ContainCommand {
    /// List every recorded containment
    List,
    /// Endorse a containment as a member of the runtime emergency panel
    Endorse(EndorseArgs),
    /// Submit a containment with its panel endorsements
    Submit(SubmitContainmentArgs),
}
impl Run for ContainCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ContainCommand::List => {
                let client = context.client_from_config()?;
                let value = client.get_runtime_containments_json()?;
                context.print_data(&value)
            }
            ContainCommand::Endorse(args) => args.run(context),
            ContainCommand::Submit(args) => args.run(context),
        }
    }
}
#[derive(clap::Args, Debug)]
pub struct EndorseArgs {
    /// Path to a JSON file with the `ContainmentV1` to endorse
    #[arg(long, value_name = "PATH")]
    file: std::path::PathBuf,
    /// File holding the panel member's private key (multihash hex); it is read at run time and
    /// never printed
    #[arg(long, value_name = "PATH")]
    member_key_file: std::path::PathBuf,
    /// Endorsement time in Unix milliseconds; it must not be after the authorizing block's
    /// committed time, and the endorsement counts for 30 minutes [default: now]
    #[arg(long, value_name = "MS")]
    at_ms: Option<u64>,
}
impl Run for EndorseArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use iroha::data_model::runtime::lifecycle::{ContainmentV1, EndorsementV1};
        let containment: ContainmentV1 = read_json(&self.file, "containment")?;
        let id = containment
            .id()
            .map_err(|error| eyre!("containment identity: {error}"))?;
        let key = read_private_key(&self.member_key_file)?;
        let member = iroha::crypto::KeyPair::from(key.clone())
            .public_key()
            .clone();
        let at_ms = match self.at_ms {
            Some(at_ms) => at_ms,
            None => u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .wrap_err("the system clock is before the Unix epoch")?
                    .as_millis(),
            )
            .wrap_err("the system time exceeds u64 milliseconds")?,
        };
        let endorsement = EndorsementV1::sign(id, at_ms, member, &key)
            .map_err(|error| eyre!("failed to sign the endorsement: {error}"))?;
        context.print_data(&endorsement)
    }
}
#[derive(clap::Args, Debug)]
pub struct SubmitContainmentArgs {
    /// Path to a JSON file with the `ContainmentV1`
    #[arg(long, value_name = "PATH")]
    file: std::path::PathBuf,
    /// Path to a JSON file with one `EndorsementV1`; repeat for each panel member
    #[arg(long = "endorsement", value_name = "PATH", required = true)]
    endorsements: Vec<std::path::PathBuf>,
}
impl Run for SubmitContainmentArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use iroha::data_model::{
            isi::{InstructionBox, runtime_upgrade::ContainRuntimeV1},
            runtime::lifecycle::{ContainmentV1, EndorsementV1},
        };
        let containment: ContainmentV1 = read_json(&self.file, "containment")?;
        let endorsements = self
            .endorsements
            .iter()
            .map(|path| read_json::<EndorsementV1>(path, "endorsement"))
            .collect::<Result<Vec<_>>>()?;
        context.finish(vec![InstructionBox::from(ContainRuntimeV1 {
            containment,
            endorsements,
        })])
    }
}
fn read_json<T: norito::json::JsonDeserialize>(path: &std::path::Path, what: &str) -> Result<T> {
    let text = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read the {what} file {}", path.display()))?;
    norito::json::from_str(&text).wrap_err_with(|| format!("malformed {what} JSON"))
}
fn read_private_key(path: &std::path::Path) -> Result<iroha::crypto::PrivateKey> {
    std::fs::read_to_string(path)
        .wrap_err("failed to read the key file")?
        .trim()
        .parse::<iroha::crypto::ExposedPrivateKey>()
        .map(|exposed| exposed.0)
        .map_err(|_| eyre!("the key file does not hold a private key"))
}
fn decode_hex_32(what: &str, value: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(value.trim().trim_start_matches("0x"))
        .map_err(|_| eyre!("{what} is not hex"))?;
    bytes
        .try_into()
        .map_err(|_| eyre!("{what} is not 32 bytes"))
}
#[cfg(test)]
mod tests {
    use iroha::data_model::{
        isi::{InstructionBox, governance::ProposeRuntimeUpgradeProposal},
        prelude::Executable,
        runtime::{
            definition::SelectionId,
            lifecycle::{
                ChangeSetEndorsementV1, ChangeSetId, ChangeSetV1, RootScopeV1,
                RuntimeUpgradeActionV1,
            },
        },
    };

    use super::*;

    /// Captures what an upgrade command prints or submits; the commands need no client.
    #[derive(Default)]
    struct Captured {
        printed: Option<String>,
        submitted: Option<Executable>,
    }
    impl RunContext for Captured {
        fn config(&self) -> &iroha::config::Config {
            panic!("the upgrade commands build their instructions locally")
        }
        fn transaction_metadata(&self) -> Option<&iroha_model_base::metadata::Metadata> {
            None
        }
        fn input_instructions(&self) -> bool {
            false
        }
        fn output_instructions(&self) -> bool {
            false
        }
        fn i18n(&self) -> &iroha_i18n::Localizer {
            panic!("the upgrade commands print no localized text")
        }
        fn print_data<T: norito::json::JsonSerialize + ?Sized>(&mut self, data: &T) -> Result<()> {
            assert!(self.printed.replace(norito::json::to_json(data)?).is_none());
            Ok(())
        }
        fn println(&mut self, _: impl std::fmt::Display) -> Result<()> {
            panic!("the upgrade commands print data only")
        }
        fn finish(&mut self, instructions: impl Into<Executable>) -> Result<()> {
            assert!(self.submitted.replace(instructions.into()).is_none());
            Ok(())
        }
    }

    fn change_set() -> ChangeSetV1 {
        ChangeSetV1 {
            network: iroha::data_model::NetworkId::from_genesis_hash(
                iroha::crypto::HashOf::from_untyped_unchecked(iroha::crypto::Hash::prehashed(
                    [0x11; 32],
                )),
            ),
            root: RootScopeV1::Universal,
            predecessor: SelectionId([0x01; 32]),
            successor: SelectionId([0x02; 32]),
            effective_height: 9,
            publish: Vec::new(),
            transitions: Vec::new(),
            qualification: [0x03; 32],
        }
    }

    #[test]
    fn a_release_signer_endorses_and_the_proposal_carries_the_endorsement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let change_set = change_set();
        let change_set_file = directory.path().join("change_set.json");
        std::fs::write(
            &change_set_file,
            norito::json::to_json(&change_set).expect("change-set JSON"),
        )
        .expect("write the change set");
        let signer =
            iroha::crypto::KeyPair::from_seed(vec![0x21; 32], iroha::crypto::Algorithm::Ed25519);
        let exposed = iroha::crypto::ExposedPrivateKey(signer.private_key().clone())
            .try_to_multihash_string()
            .expect("key multihash");
        let key_file = directory.path().join("signer.key");
        std::fs::write(&key_file, &exposed).expect("write the signer key");

        let mut context = Captured::default();
        EndorseChangeSetArgs {
            file: change_set_file.clone(),
            signer_key_file: key_file,
        }
        .run(&mut context)
        .expect("endorse");
        let printed = context.printed.expect("the endorsement is printed");
        assert!(
            !printed.contains(&exposed),
            "the key never reaches the output"
        );
        let endorsement: ChangeSetEndorsementV1 =
            norito::json::from_str(&printed).expect("endorsement JSON");
        assert_eq!(endorsement.change_set, change_set.id().expect("identity"));
        assert_eq!(&endorsement.signer, signer.public_key());
        endorsement.verify().expect("a valid endorsement");

        let endorsement_file = directory.path().join("endorsement.json");
        std::fs::write(&endorsement_file, &printed).expect("write the endorsement");
        let mut context = Captured::default();
        ProposeArgs {
            file: change_set_file,
            endorsements: vec![endorsement_file],
        }
        .run(&mut context)
        .expect("propose");
        let expected = InstructionBox::from(ProposeRuntimeUpgradeProposal {
            action: RuntimeUpgradeActionV1::Schedule(change_set),
            provenance: vec![endorsement],
        });
        assert_eq!(
            context.submitted,
            Some(Executable::Instructions(vec![expected].into()))
        );
    }

    #[test]
    fn a_cancellation_carries_no_endorsements() {
        let mut context = Captured::default();
        CancelArgs {
            id: hex::encode([0xA5; 32]),
        }
        .run(&mut context)
        .expect("cancel");
        let expected = InstructionBox::from(ProposeRuntimeUpgradeProposal {
            action: RuntimeUpgradeActionV1::Cancel(ChangeSetId([0xA5; 32])),
            provenance: Vec::new(),
        });
        assert_eq!(
            context.submitted,
            Some(Executable::Instructions(vec![expected].into()))
        );
        assert!(
            CancelArgs { id: "zz".into() }
                .run(&mut Captured::default())
                .is_err(),
            "a malformed identity submits nothing"
        );
    }
}
