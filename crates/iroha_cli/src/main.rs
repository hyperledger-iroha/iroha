//! Iroha client CLI

#![expect(clippy::doc_markdown)]

use std::{
    fmt::Display,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    time::Duration,
};

use erased_serde::Serialize;
use error_stack::{fmt::ColorMode, IntoReportCompat, ResultExt};
use eyre::{eyre, Result, WrapErr};
use futures::TryStreamExt;
use iroha::{client::Client, config::Config, data_model::prelude::*};
use thiserror::Error;
use tokio::runtime::Runtime;

/// Iroha Client CLI provides a simple way to interact with the Iroha Web API.
#[derive(clap::Parser, Debug)]
#[command(name = "iroha", version = concat!("version=", env!("CARGO_PKG_VERSION"), " git_commit_sha=", env!("VERGEN_GIT_SHA")), author)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, value_name("PATH"), default_value = "client.toml")]
    config: PathBuf,
    /// Print configuration details to stderr
    #[arg(short, long)]
    verbose: bool,
    /// Path to a JSON5 file for attaching transaction metadata (optional)
    #[arg(short, long, value_name("PATH"))]
    metadata: Option<PathBuf>,
    /// Reads instructions from stdin and appends new ones.
    ///
    /// Example usage:
    ///
    /// `echo "[]" | iroha -io domain register --id "domain" | iroha -i asset definition register --id "asset#domain" -t Numeric`
    #[arg(short, long)]
    input: bool,
    /// Outputs instructions to stdout without submitting them.
    ///
    /// Example usage:
    ///
    /// `iroha -o domain register --id "domain" | iroha -io asset definition register --id "asset#domain" -t Numeric | iroha transaction stdin`
    #[arg(short, long)]
    output: bool,
    /// Commands
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Read and write domains
    #[command(subcommand)]
    Domain(domain::Command),
    /// Read and write accounts
    #[command(subcommand)]
    Account(account::Command),
    /// Read and write assets
    #[command(subcommand)]
    Asset(asset::Command),
    /// Read and write NFTs
    #[command(subcommand)]
    Nft(nft::Command),
    /// Read and write peers
    #[command(subcommand)]
    Peer(peer::Command),
    /// Subscribe to events: state changes, transaction/block/trigger progress
    Events(events::Args),
    /// Subscribe to blocks
    Blocks(blocks::Args),
    /// Read and write multi-signature accounts and transactions.
    ///
    /// See the [usage guide](./docs/multisig.md) for details
    #[command(subcommand)]
    Multisig(multisig::Command),
    /// Read various data
    #[command(subcommand)]
    Query(query::Command),
    /// Read transactions and write various data
    #[command(subcommand)]
    Transaction(transaction::Command),
    /// Read and write roles
    #[command(subcommand)]
    Role(role::Command),
    /// Read and write system parameters
    #[command(subcommand)]
    Parameter(parameter::Command),
    /// Read and write triggers
    #[command(subcommand)]
    Trigger(trigger::Command),
    /// Read and write the executor
    #[command(subcommand)]
    Executor(executor::Command),
    /// Output CLI documentation in Markdown format
    MarkdownHelp(MarkdownHelp),
}

/// Context inside which commands run
trait RunContext {
    fn config(&self) -> &Config;

    fn transaction_metadata(&self) -> Option<&Metadata>;

    fn input_instructions(&self) -> bool;

    fn output_instructions(&self) -> bool;

    fn print_data(&mut self, data: &dyn Serialize) -> Result<()>;

    fn println(&mut self, data: impl Display) -> Result<()>;

    fn client_from_config(&self) -> Client {
        Client::new(self.config().clone())
    }

    /// Submit instructions or dump them to stdout depending on the flag
    fn finish(&mut self, instructions: impl Into<Executable>) -> Result<()> {
        let mut instructions = match instructions.into() {
            Executable::Wasm(wasm) => {
                if self.input_instructions() || self.output_instructions() {
                    eyre::bail!(
                        "Incompatible `--input` `--output` flags with `iroha transaction wasm`"
                    )
                }
                return self.submit(wasm);
            }
            Executable::Instructions(instructions) => instructions.into_vec(),
        };
        if self.input_instructions() {
            let mut acc: Vec<InstructionBox> = parse_json5_stdin_unchecked()?;
            acc.append(&mut instructions);
            instructions = acc;
        }
        if self.output_instructions() {
            dump_json5_stdout(&instructions)
        } else {
            self.submit(instructions)
        }
    }

    /// Combine instructions into a single transaction and submit it
    ///
    /// # Errors
    ///
    /// Fails if submitting over network fails
    fn submit(&mut self, instructions: impl Into<Executable>) -> Result<()> {
        let client = self.client_from_config();
        let transaction = client.build_transaction(
            instructions,
            self.transaction_metadata().cloned().unwrap_or_default(),
        );

        #[cfg(not(debug_assertions))]
        let err_msg = "Failed to submit transaction";
        #[cfg(debug_assertions)]
        let err_msg = format!("Failed to submit transaction {transaction:?}");

        let hash = client
            .submit_transaction_blocking(&transaction)
            .wrap_err(err_msg)?;

        self.println("Transaction Submitted. Details:")?;
        self.print_data(&transaction)?;
        self.println("Hash:")?;
        self.print_data(&hash)?;

        Ok(())
    }
}

struct PrintJsonContext<W> {
    write: W,
    config: Config,
    transaction_metadata: Option<Metadata>,
    input_instructions: bool,
    output_instructions: bool,
}

impl<W: std::io::Write> RunContext for PrintJsonContext<W> {
    fn config(&self) -> &Config {
        &self.config
    }

    fn transaction_metadata(&self) -> Option<&Metadata> {
        self.transaction_metadata.as_ref()
    }

    fn input_instructions(&self) -> bool {
        self.input_instructions
    }

    fn output_instructions(&self) -> bool {
        self.output_instructions
    }

    /// Serialize and print data
    ///
    /// # Errors
    ///
    /// - if serialization fails
    /// - if printing fails
    fn print_data(&mut self, data: &dyn Serialize) -> Result<()> {
        writeln!(&mut self.write, "{}", serde_json::to_string_pretty(data)?)?;
        Ok(())
    }

    fn println(&mut self, data: impl Display) -> Result<()> {
        writeln!(&mut self.write, "{data}")?;
        Ok(())
    }
}

/// Runs command
trait Run {
    /// Runs command
    ///
    /// # Errors
    /// if inner command errors
    fn run<C: RunContext>(self, context: &mut C) -> Result<()>;
}

macro_rules! match_all {
    (($self:ident, $context:ident), { $($variants:path),* $(,)?}) => {
        match $self {
            $($variants(variant) => Run::run(variant, $context),)*
        }
    };
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use Command::*;
        match_all!((self, context), { Domain, Account, Asset, Nft, Peer, Events, Blocks, Multisig, Query, Transaction, Role, Parameter, Trigger, Executor, MarkdownHelp })
    }
}

#[derive(Error, Debug)]
enum MainError {
    #[error("Failed to load Iroha client configuration")]
    Config,
    #[error("Failed to serialize config")]
    SerializeConfig,
    #[error("Failed to get transaction metadata from file")]
    TransactionMetadata,
    #[error("Failed to run the command")]
    Command,
}

#[derive(clap::Args, Debug)]
struct MarkdownHelp;

impl Run for MarkdownHelp {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        Ok(())
    }
}

fn main() -> error_stack::Result<(), MainError> {
    let args: Args = clap::Parser::parse();

    if let Command::MarkdownHelp(_md) = args.command {
        clap_markdown::print_help_markdown::<Args>();
        return Ok(());
    }

    error_stack::Report::set_color_mode(color_mode());

    let config = Config::load(args.config)
        // FIXME: would be nice to NOT change the context, it's unnecessary
        .change_context(MainError::Config)
        .attach_printable("config path was set by `--config` argument")?;

    if args.verbose {
        eprintln!(
            "Configuration: {}",
            &serde_json::to_string_pretty(&config)
                .change_context(MainError::SerializeConfig)
                .attach_printable("caused by `--verbose` argument")?
        );
    }

    let mut context = PrintJsonContext {
        write: io::stdout(),
        config,
        transaction_metadata: None,
        input_instructions: args.input,
        output_instructions: args.output,
    };
    if let Some(path) = args.metadata {
        let str = fs::read_to_string(&path)
            .change_context(MainError::TransactionMetadata)
            .attach_printable("failed to read to string")?;
        let metadata: Metadata = json5::from_str(&str)
            .change_context(MainError::TransactionMetadata)
            .attach_printable("failed to deserialize to metadata")?;
        context.transaction_metadata = Some(metadata);
    }

    args.command
        .run(&mut context)
        .into_report()
        .map_err(|report| report.change_context(MainError::Command))?;

    Ok(())
}

fn color_mode() -> ColorMode {
    if supports_color::on(supports_color::Stream::Stdout).is_some()
        && supports_color::on(supports_color::Stream::Stderr).is_some()
    {
        ColorMode::Color
    } else {
        ColorMode::None
    }
}

mod filter {
    use iroha::data_model::query::dsl::CompoundPredicate;

    use super::*;

    #[derive(clap::Args, Debug)]
    pub struct DomainFilter {
        /// Filtering condition specified as a JSON5 string
        #[arg(value_parser = parse_json5::<CompoundPredicate<Domain>>)]
        pub predicate: CompoundPredicate<Domain>,
    }

    #[derive(clap::Args, Debug)]
    pub struct AccountFilter {
        /// Filtering condition specified as a JSON5 string
        #[arg(value_parser = parse_json5::<CompoundPredicate<Account>>)]
        pub predicate: CompoundPredicate<Account>,
    }

    #[derive(clap::Args, Debug)]
    pub struct AssetFilter {
        /// Filtering condition specified as a JSON5 string
        #[arg(value_parser = parse_json5::<CompoundPredicate<Asset>>)]
        pub predicate: CompoundPredicate<Asset>,
    }

    #[derive(clap::Args, Debug)]
    pub struct AssetDefinitionFilter {
        /// Filtering condition specified as a JSON5 string
        #[arg(value_parser = parse_json5::<CompoundPredicate<AssetDefinition>>)]
        pub predicate: CompoundPredicate<AssetDefinition>,
    }

    #[derive(clap::Args, Debug)]
    pub struct NftFilter {
        /// Filtering condition specified as a JSON5 string
        #[arg(value_parser = parse_json5::<CompoundPredicate<Nft>>)]
        pub predicate: CompoundPredicate<Nft>,
    }
}

mod events {

    use iroha::data_model::events::pipeline::{BlockEventFilter, TransactionEventFilter};

    use super::*;

    #[derive(clap::Args, Debug)]
    pub struct Args {
        /// Duration to listen for events.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        #[arg(short, long, global = true)]
        timeout: Option<humantime::Duration>,
        #[command(subcommand)]
        command: Command,
    }

    #[derive(clap::Subcommand, Debug)]
    enum Command {
        /// Notify when the world state undergoes certain changes
        State,
        /// Notify when a transaction reaches specific stages
        Transaction,
        /// Notify when a block reaches specific stages
        Block,
        /// Notify when a trigger execution is ordered
        TriggerExecute,
        /// Notify when a trigger execution is completed
        TriggerComplete,
    }

    impl Run for Args {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            let timeout: Option<Duration> = self.timeout.map(Into::into);

            match self.command {
                State => listen(DataEventFilter::Any, context, timeout),
                Transaction => listen(TransactionEventFilter::default(), context, timeout),
                Block => listen(BlockEventFilter::default(), context, timeout),
                TriggerExecute => listen(ExecuteTriggerEventFilter::new(), context, timeout),
                TriggerComplete => listen(TriggerCompletedEventFilter::new(), context, timeout),
            }
        }
    }

    fn listen(
        filter: impl Into<EventFilterBox>,
        context: &mut impl RunContext,
        timeout: Option<Duration>,
    ) -> Result<()> {
        let filter = filter.into();
        let client = context.client_from_config();

        if let Some(timeout) = timeout {
            eprintln!("Listening to events with filter: {filter:?} and timeout: {timeout:?}");
            let rt = Runtime::new().wrap_err("Failed to create runtime")?;
            rt.block_on(async {
                let mut stream = client
                    .listen_for_events_async([filter])
                    .await
                    .expect("Failed to listen for events");
                while let Ok(event) = tokio::time::timeout(timeout, stream.try_next()).await {
                    context.print_data(&event?)?;
                }
                eprintln!("Timeout period has expired.");
                Result::<()>::Ok(())
            })?;
        } else {
            eprintln!("Listening to events with filter: {filter:?}");
            client
                .listen_for_events([filter])
                .wrap_err("Failed to listen for events")?
                .try_for_each(|event| context.print_data(&event?))?;
        }
        Ok(())
    }
}

mod blocks {
    use std::num::NonZeroU64;

    use super::*;

    #[derive(clap::Args, Debug)]
    pub struct Args {
        /// Block height from which to start streaming blocks
        height: NonZeroU64,
        /// Duration to listen for events.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        #[arg(short, long)]
        timeout: Option<humantime::Duration>,
    }

    impl Run for Args {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let Args { height, timeout } = self;
            let timeout: Option<Duration> = timeout.map(Into::into);
            listen(height, context, timeout)
        }
    }

    fn listen(
        height: NonZeroU64,
        context: &mut impl RunContext,
        timeout: Option<Duration>,
    ) -> Result<()> {
        let client = context.client_from_config();
        if let Some(timeout) = timeout {
            eprintln!("Listening to blocks from height: {height} and timeout: {timeout:?}");
            let rt = Runtime::new().wrap_err("Failed to create runtime")?;
            rt.block_on(async {
                let mut stream = client
                    .listen_for_blocks_async(height)
                    .await
                    .expect("Failed to listen for blocks");
                while let Ok(event) = tokio::time::timeout(timeout, stream.try_next()).await {
                    context.print_data(&event?)?;
                }
                eprintln!("Timeout period has expired.");
                Result::<()>::Ok(())
            })?;
        } else {
            eprintln!("Listening to blocks from height: {height}");
            client
                .listen_for_blocks(height)
                .wrap_err("Failed to listen for blocks")?
                .try_for_each(|event| context.print_data(&event?))?;
        }
        Ok(())
    }
}

macro_rules! impl_list {
    ($filter:ty, $query:expr) => {
        #[derive(clap::Subcommand, Debug)]
        pub enum List {
            /// List all IDs, or full entries when `--verbose` is specified
            All {
                /// Display detailed entry information instead of just IDs
                #[arg(short, long)]
                verbose: bool,
            },
            /// Filter by a given predicate
            Filter($filter),
        }

        impl Run for List {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                let client = context.client_from_config();
                let query = client.query($query);
                match self {
                    List::All { verbose } => {
                        if verbose {
                            let entries = query.execute_all()?;
                            context.print_data(&entries)?;
                        } else {
                            let ids = query.select_with(|entry| entry.id).execute_all()?;
                            context.print_data(&ids)?;
                        }
                    }
                    List::Filter(filter) => {
                        let view = query.filter(filter.predicate).execute_all()?;
                        context.print_data(&view)?;
                    }
                }
                Ok(())
            }
        }
    };
}

mod domain {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List domains
        #[command(subcommand)]
        List(List),
        /// Retrieve details of a specific domain
        Get(Id),
        /// Register a domain
        Register(Id),
        /// Unregister a domain
        Unregister(Id),
        /// Transfer ownership of a domain
        Transfer(Transfer),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::domain::Command),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Get(args) => {
                    let client = context.client_from_config();
                    let entry = client
                        .query(FindDomains)
                        .filter_with(|entry| entry.id.eq(args.id))
                        .execute_single()
                        .wrap_err("Failed to get domain")?;
                    context.print_data(&entry)
                }
                Register(args) => {
                    let instruction =
                        iroha::data_model::isi::Register::domain(Domain::new(args.id));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register domain")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::domain(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister domain")
                }
                Transfer(args) => {
                    let instruction =
                        iroha::data_model::isi::Transfer::domain(args.from, args.id, args.to);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to transfer domain")
                }
                Meta(cmd) => cmd.run(context),
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// Domain name
        #[arg(short, long)]
        pub id: DomainId,
        /// Source account, in the format "multihash@domain"
        #[arg(short, long)]
        pub from: AccountId,
        /// Destination account, in the format "multihash@domain"
        #[arg(short, long)]
        pub to: AccountId,
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Domain name
        #[arg(short, long)]
        pub id: DomainId,
    }

    impl_list!(filter::DomainFilter, FindDomains);
}

mod account {
    use std::fmt::Debug;

    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write account roles
        #[command(subcommand)]
        Role(RoleCommand),
        /// Read and write account permissions
        #[command(subcommand)]
        Permission(PermissionCommand),
        /// List accounts
        #[command(subcommand)]
        List(List),
        /// Retrieve details of a specific account
        Get(Id),
        /// Register an account
        Register(Id),
        /// Unregister an account
        Unregister(Id),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::account::Command),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Role(cmd) => cmd.run(context),
                Permission(cmd) => cmd.run(context),
                List(cmd) => cmd.run(context),
                Get(args) => {
                    let client = context.client_from_config();
                    let entry = client
                        .query(FindAccounts)
                        .filter_with(|entry| entry.id.eq(args.id))
                        .execute_single()
                        .wrap_err("Failed to get account")?;
                    context.print_data(&entry)
                }
                Register(args) => {
                    let instruction =
                        iroha::data_model::isi::Register::account(Account::new(args.id));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register account")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::account(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister account")
                }
                Meta(cmd) => cmd.run(context),
            }
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum RoleCommand {
        /// List account role IDs
        List(Id),
        /// Grant a role to an account
        Grant(IdRole),
        /// Revoke a role from an account
        Revoke(IdRole),
    }

    impl Run for RoleCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::RoleCommand::*;
            match self {
                List(args) => {
                    let client = context.client_from_config();
                    let roles = client
                        .query(FindRolesByAccountId::new(args.id))
                        .execute_all()?;
                    context.print_data(&roles)
                }
                Grant(args) => {
                    let instruction =
                        iroha::data_model::isi::Grant::account_role(args.role, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to grant the role to the account")
                }
                Revoke(args) => {
                    let instruction =
                        iroha::data_model::isi::Revoke::account_role(args.role, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to revoke the role from the account")
                }
            }
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum PermissionCommand {
        /// List account permissions
        List(Id),
        /// Grant an account permission using JSON5 input from stdin
        Grant(Id),
        /// Revoke an account permission using JSON5 input from stdin
        Revoke(Id),
    }

    impl Run for PermissionCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::PermissionCommand::*;
            match self {
                List(args) => {
                    let client = context.client_from_config();
                    let permissions = client
                        .query(FindPermissionsByAccountId::new(args.id))
                        .execute_all()?;
                    context.print_data(&permissions)
                }
                Grant(args) => {
                    let permission: Permission = parse_json5_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Grant::account_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to grant the permission to the account")
                }
                Revoke(args) => {
                    let permission: Permission = parse_json5_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Revoke::account_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to revoke the permission from the account")
                }
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Account in the format "multihash@domain"
        #[arg(short, long)]
        id: AccountId,
    }

    #[derive(clap::Args, Debug)]
    pub struct IdRole {
        /// Account in the format "multihash@domain"
        #[arg(short, long)]
        pub id: AccountId,
        /// Role name
        #[arg(short, long)]
        pub role: RoleId,
    }

    impl_list!(filter::AccountFilter, FindAccounts);
}

mod asset {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write asset definitions
        #[command(subcommand)]
        Definition(definition::Command),
        /// Retrieve details of a specific asset
        Get(Id),
        /// List assets
        #[command(subcommand)]
        List(List),
        /// Increase the quantity of an asset
        Mint(IdQuantity),
        /// Decrease the quantity of an asset
        Burn(IdQuantity),
        /// Transfer an asset between accounts
        Transfer(Transfer),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Definition(cmd) => cmd.run(context),
                Get(args) => {
                    let client = context.client_from_config();
                    let entry = client
                        .query(FindAssets)
                        .filter_with(|entry| entry.id.eq(args.id))
                        .execute_single()
                        .wrap_err("Failed to get asset")?;
                    context.print_data(&entry)
                }
                List(cmd) => cmd.run(context),
                Mint(args) => {
                    let instruction =
                        iroha::data_model::isi::Mint::asset_numeric(args.quantity, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to mint numeric asset")
                }
                Burn(args) => {
                    let instruction =
                        iroha::data_model::isi::Burn::asset_numeric(args.quantity, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to burn numeric asset")
                }
                Transfer(args) => {
                    let instruction = iroha::data_model::isi::Transfer::asset_numeric(
                        args.id,
                        args.quantity,
                        args.to,
                    );
                    context
                        .finish([instruction])
                        .wrap_err("Failed to transfer numeric asset")
                }
            }
        }
    }

    mod definition {
        use iroha::data_model::asset::{AssetDefinition, AssetDefinitionId};

        use super::*;

        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// List asset definitions
            #[command(subcommand)]
            List(List),
            /// Retrieve details of a specific asset definition
            Get(Id),
            /// Register an asset definition
            Register(Register),
            /// Unregister an asset definition
            Unregister(Id),
            /// Transfer ownership of an asset definition
            Transfer(Transfer),
            /// Read and write metadata
            #[command(subcommand)]
            Meta(metadata::asset_definition::Command),
        }

        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    List(cmd) => cmd.run(context),
                    Get(args) => {
                        let client = context.client_from_config();
                        let entry = client
                            .query(FindAssetsDefinitions)
                            .filter_with(|entry| entry.id.eq(args.id))
                            .execute_single()
                            .wrap_err("Failed to get asset definition")?;
                        context.print_data(&entry)
                    }
                    Register(args) => {
                        let mut entry = AssetDefinition::new(args.id, args.scale.into());
                        if args.mint_once {
                            entry = entry.mintable_once();
                        }
                        let instruction = iroha::data_model::isi::Register::asset_definition(entry);
                        context
                            .finish([instruction])
                            .wrap_err("Failed to register asset")
                    }
                    Unregister(args) => {
                        let instruction =
                            iroha::data_model::isi::Unregister::asset_definition(args.id);
                        context
                            .finish([instruction])
                            .wrap_err("Failed to unregister asset")
                    }
                    Transfer(args) => {
                        let instruction = iroha::data_model::isi::Transfer::asset_definition(
                            args.from, args.id, args.to,
                        );
                        context
                            .finish([instruction])
                            .wrap_err("Failed to transfer asset definition")
                    }
                    Meta(cmd) => cmd.run(context),
                }
            }
        }

        #[derive(clap::Args, Debug)]
        pub struct Register {
            /// Asset definition in the format "asset#domain"
            #[arg(short, long)]
            pub id: AssetDefinitionId,
            /// Disables minting after the first instance
            #[arg(short, long)]
            pub mint_once: bool,
            /// Numeric scale of the asset. No value means unconstrained.
            #[arg(short, long)]
            pub scale: Option<u32>,
        }

        #[derive(clap::Args, Debug)]
        pub struct Transfer {
            /// Asset definition in the format "asset#domain"
            #[arg(short, long)]
            pub id: AssetDefinitionId,
            /// Source account, in the format "multihash@domain"
            #[arg(short, long)]
            pub from: AccountId,
            /// Destination account, in the format "multihash@domain"
            #[arg(short, long)]
            pub to: AccountId,
        }

        #[derive(clap::Args, Debug)]
        pub struct Id {
            /// Asset definition in the format "asset#domain"
            #[arg(short, long)]
            pub id: AssetDefinitionId,
        }

        impl_list!(filter::AssetDefinitionFilter, FindAssetsDefinitions);
    }

    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"
        #[arg(short, long)]
        pub id: AssetId,
        /// Destination account, in the format "multihash@domain"
        #[arg(short, long)]
        pub to: AccountId,
        /// Transfer amount (integer or decimal)
        #[arg(short, long)]
        pub quantity: Numeric,
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"
        #[arg(short, long)]
        pub id: AssetId,
    }

    #[derive(clap::Args, Debug)]
    pub struct IdQuantity {
        /// Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"
        #[arg(short, long)]
        pub id: AssetId,
        /// Amount of change (integer or decimal)
        #[arg(short, long)]
        pub quantity: Numeric,
    }

    impl_list!(filter::AssetFilter, FindAssets);
}

mod nft {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Retrieve details of a specific NFT
        Get(Id),
        /// List NFTs
        #[clap(subcommand)]
        List(List),
        /// Register NFT with content provided from stdin in JSON5 format
        Register(Id),
        /// Unregister NFT
        Unregister(Id),
        /// Transfer ownership of NFT
        Transfer(Transfer),
        /// Get a value from NFT
        #[command(name = "getkv")]
        GetKeyValue(IdKey),
        /// Create or update a key-value entry of NFT using JSON5 input from stdin
        #[command(name = "setkv")]
        SetKeyValue(IdKey),
        /// Remove a key-value entry from NFT
        #[command(name = "removekv")]
        RemoveKeyValue(IdKey),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Get(args) => {
                    let client = context.client_from_config();
                    let entry = client
                        .query(FindNfts)
                        .filter_with(|entry| entry.id.eq(args.id))
                        .execute_single()
                        .wrap_err("Failed to get NFT")?;
                    context.print_data(&entry)
                }
                List(cmd) => cmd.run(context),
                Register(args) => {
                    let metadata: Metadata = parse_json5_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Register::nft(Nft::new(args.id, metadata));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register NFT")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::nft(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister NFT")
                }
                Transfer(args) => {
                    let instruction =
                        iroha::data_model::isi::Transfer::nft(args.from, args.id, args.to);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to transfer NFT")
                }
                GetKeyValue(args) => {
                    let client = context.client_from_config();
                    let value = client
                        .query(FindNfts)
                        .filter_with(|nft| nft.id.eq(args.id))
                        .select_with(|nft| nft.content.key(args.key))
                        .execute_single()
                        .wrap_err("Failed to get value")?;
                    context.print_data(&value)
                }
                SetKeyValue(args) => {
                    let value: Json = parse_json5_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::SetKeyValue::nft(args.id, args.key, value);
                    context.finish([instruction])
                }
                RemoveKeyValue(args) => {
                    let instruction =
                        iroha::data_model::isi::RemoveKeyValue::nft(args.id, args.key);
                    context.finish([instruction])
                }
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
        /// Source account, in the format "multihash@domain"
        #[arg(short, long)]
        pub from: AccountId,
        /// Destination account, in the format "multihash@domain"
        #[arg(short, long)]
        pub to: AccountId,
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
    }

    #[derive(clap::Args, Debug)]
    pub struct IdKey {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
        #[arg(short, long)]
        pub key: Name,
    }

    impl_list!(filter::NftFilter, FindNfts);
}

mod peer {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List registered peers expected to connect with each other
        #[command(subcommand)]
        List(List),
        /// Register a peer
        Register(Id),
        /// Unregister a peer
        Unregister(Id),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Register(args) => {
                    let instruction = iroha::data_model::isi::Register::peer(args.key.into());
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register peer")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::peer(args.key.into());
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister peer")
                }
            }
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all registered peers
        All,
    }

    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let entries = client.query(FindPeers).execute_all()?;
            context.print_data(&entries)
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Peer's public key in multihash format
        #[arg(short, long)]
        pub key: PublicKey,
    }
}

mod multisig {
    use std::{
        collections::BTreeMap,
        num::{NonZeroU16, NonZeroU64},
        time::{Duration, SystemTime},
    };

    use derive_more::{Constructor, Display};
    use iroha::executor_data_model::isi::multisig::*;
    use serde::Serialize;
    use serde_with::{serde_as, DisplayFromStr, SerializeDisplay};

    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List pending multisig transactions relevant to you
        #[command(subcommand)]
        List(List),
        /// Register a multisig account
        Register(Register),
        /// Propose a multisig transaction using JSON5 input from stdin
        Propose(Propose),
        /// Approve a multisig transaction
        Approve(Approve),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match_all!((self, context), { List, Register, Propose, Approve })
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Register {
        /// ID of the multisig account to be registered
        #[arg(short, long)]
        pub account: AccountId,
        /// List of signatories for the multisig account
        #[arg(short, long, num_args(2..))]
        pub signatories: Vec<AccountId>,
        /// Relative weights of signatories' responsibilities
        #[arg(short, long, num_args(2..))]
        pub weights: Vec<u8>,
        /// Threshold of total weight required for authentication
        #[arg(short, long)]
        pub quorum: u16,
        /// Time-to-live for multisig transactions.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        #[arg(short, long, default_value_t = default_transaction_ttl())]
        pub transaction_ttl: humantime::Duration,
    }

    fn default_transaction_ttl() -> humantime::Duration {
        std::time::Duration::from_millis(DEFAULT_MULTISIG_TTL_MS).into()
    }

    impl Run for Register {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            if self.signatories.len() != self.weights.len() {
                return Err(eyre!("signatories and weights must be equal in length"));
            }
            let instruction = MultisigRegister::new(
                self.account,
                MultisigSpec::new(
                    self.signatories.into_iter().zip(self.weights).collect(),
                    NonZeroU16::new(self.quorum).expect("quorum should not be 0"),
                    self.transaction_ttl
                        .as_millis()
                        .try_into()
                        .ok()
                        .and_then(NonZeroU64::new)
                        .expect("ttl should be between 1 ms and 584942417 years"),
                ),
            );

            context
                .finish([instruction])
                .wrap_err("Failed to register multisig account")
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Propose {
        /// Multisig authority managing the proposed transaction
        #[arg(short, long)]
        pub account: AccountId,
        /// Overrides the default time-to-live for this transaction.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        #[arg(short, long)]
        pub transaction_ttl: Option<humantime::Duration>,
    }

    impl Run for Propose {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instructions: Vec<InstructionBox> = parse_json5_stdin(context)?;
            let transaction_ttl_ms = self.transaction_ttl.map(|duration| {
                duration
                    .as_millis()
                    .try_into()
                    .ok()
                    .and_then(NonZeroU64::new)
                    .expect("ttl should be between 1 ms and 584942417 years")
            });

            let instructions_hash = HashOf::new(&instructions);
            println!("{instructions_hash}");

            let propose_multisig_transaction =
                MultisigPropose::new(self.account, instructions, transaction_ttl_ms);

            context
                .finish([propose_multisig_transaction])
                .wrap_err("Failed to propose transaction")
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Approve {
        /// Multisig authority of the transaction
        #[arg(short, long)]
        pub account: AccountId,
        /// Hash of the instructions to approve
        #[arg(short, long)]
        pub instructions_hash: ProposalKey,
    }

    impl Run for Approve {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let approve_multisig_transaction =
                MultisigApprove::new(self.account, self.instructions_hash);

            context
                .finish([approve_multisig_transaction])
                .wrap_err("Failed to approve transaction")
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all pending multisig transactions relevant to you
        All,
    }

    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let me = client.account.clone();
            let Ok(my_multisig_roles) = client
                .query(FindRolesByAccountId::new(me.clone()))
                .filter_with(|role_id| role_id.name.starts_with(MULTISIG_SIGNATORY))
                .execute_all()
            else {
                return Ok(());
            };
            let mut stack = my_multisig_roles
                .iter()
                .filter_map(multisig_account_from)
                .map(|account_id| Context::new(me.clone(), account_id, None))
                .collect();
            let mut proposals = BTreeMap::new();

            fold_proposals(&mut proposals, &mut stack, &client)?;
            context.print_data(&proposals)
        }
    }

    const DELIMITER: char = '/';
    const MULTISIG: &str = "multisig";
    const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";

    fn spec_key() -> Name {
        format!("{MULTISIG}{DELIMITER}spec").parse().unwrap()
    }

    fn proposal_key_prefix() -> String {
        format!("{MULTISIG}{DELIMITER}proposals{DELIMITER}")
    }

    fn multisig_account_from(role: &RoleId) -> Option<AccountId> {
        role.name()
            .as_ref()
            .strip_prefix(MULTISIG_SIGNATORY)?
            .rsplit_once(DELIMITER)
            .and_then(|(init, last)| {
                format!("{last}@{}", init.trim_matches(DELIMITER))
                    .parse()
                    .ok()
            })
    }

    type PendingProposals = BTreeMap<ProposalKey, ProposalStatus>;

    type ProposalKey = HashOf<Vec<InstructionBox>>;

    #[serde_as]
    #[derive(Debug, Serialize, Constructor)]
    struct ProposalStatus {
        instructions: Vec<InstructionBox>,
        #[serde_as(as = "DisplayFromStr")]
        proposed_at: humantime::Timestamp,
        #[serde_as(as = "DisplayFromStr")]
        expires_in: humantime::Duration,
        approval_path: Vec<ApprovalEdge>,
    }

    impl Default for ProposalStatus {
        fn default() -> Self {
            Self::new(
                Vec::new(),
                SystemTime::UNIX_EPOCH.into(),
                Duration::ZERO.into(),
                Vec::new(),
            )
        }
    }

    #[derive(Debug, SerializeDisplay, Display, Constructor)]
    #[display(fmt = "{weight} {} [{got}/{quorum}] {target}", "self.relation()")]
    struct ApprovalEdge {
        weight: u8,
        has_approved: bool,
        got: u16,
        quorum: u16,
        target: AccountId,
    }

    impl ApprovalEdge {
        fn relation(&self) -> &str {
            if self.has_approved {
                "joined"
            } else {
                "->"
            }
        }
    }

    #[derive(Debug, Constructor)]
    struct Context {
        child: AccountId,
        this: AccountId,
        key_span: Option<(ProposalKey, ProposalKey)>,
    }

    fn fold_proposals(
        proposals: &mut PendingProposals,
        stack: &mut Vec<Context>,
        client: &Client,
    ) -> Result<()> {
        let Some(context) = stack.pop() else {
            return Ok(());
        };
        let account = client
            .query(FindAccounts)
            .filter_with(|account| account.id.eq(context.this.clone()))
            .execute_single()?;
        let spec: MultisigSpec = account
            .metadata()
            .get(&spec_key())
            .unwrap()
            .try_into_any()?;
        for (proposal_key, proposal_value) in account
            .metadata()
            .iter()
            .filter_map(|(k, v)| {
                k.as_ref().strip_prefix(&proposal_key_prefix()).map(|k| {
                    (
                        k.parse::<ProposalKey>().unwrap(),
                        v.try_into_any::<MultisigProposalValue>().unwrap(),
                    )
                })
            })
            .filter(|(k, _v)| context.key_span.is_none_or(|(_, top)| *k == top))
        {
            let mut is_root_proposal = true;
            for instruction in &proposal_value.instructions {
                let InstructionBox::Custom(instruction) = instruction else {
                    continue;
                };
                let Ok(MultisigInstructionBox::Approve(approve)) = instruction.payload().try_into()
                else {
                    continue;
                };
                is_root_proposal = false;
                let leaf = context.key_span.map_or(proposal_key, |(leaf, _)| leaf);
                let top = approve.instructions_hash;
                stack.push(Context::new(
                    context.this.clone(),
                    approve.account,
                    Some((leaf, top)),
                ));
            }
            let proposal_status = match context.key_span {
                None => proposals.entry(proposal_key).or_default(),
                Some((leaf, _)) => proposals.get_mut(&leaf).unwrap(),
            };
            let edge = ApprovalEdge::new(
                *spec.signatories.get(&context.child).unwrap(),
                proposal_value.approvals.contains(&context.child),
                spec.signatories
                    .iter()
                    .filter(|(id, _)| proposal_value.approvals.contains(id))
                    .map(|(_, weight)| u16::from(*weight))
                    .sum(),
                spec.quorum.into(),
                context.this.clone(),
            );
            proposal_status.approval_path.push(edge);
            if is_root_proposal {
                proposal_status.instructions = proposal_value.instructions;
                proposal_status.proposed_at = {
                    let proposed_at = Duration::from_secs(
                        Duration::from_millis(proposal_value.proposed_at_ms).as_secs(),
                    );
                    SystemTime::UNIX_EPOCH
                        .checked_add(proposed_at)
                        .unwrap()
                        .into()
                };
                proposal_status.expires_in = {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap();
                    let expires_at = Duration::from_millis(proposal_value.expires_at_ms);
                    Duration::from_secs(expires_at.saturating_sub(now).as_secs()).into()
                };
            }
        }
        fold_proposals(proposals, stack, client)
    }
}

mod query {
    use iroha::data_model::query::AnyQueryBox;

    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Query using JSON5 input from stdin
        Stdin(Stdin),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match_all!((self, context), { Stdin })
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Stdin;

    impl Run for Stdin {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = Client::new(context.config().clone());
            let query: AnyQueryBox = parse_json5_stdin(context)?;

            match query {
                AnyQueryBox::Singular(query) => {
                    let result = client
                        .query_single(query)
                        .wrap_err("Failed to query response")?;

                    context.print_data(&result)
                }
                AnyQueryBox::Iterable(query) => {
                    // we can't really do type-erased iterable queries in a nice way right now...
                    use iroha::data_model::query::builder::QueryExecutor;

                    let (mut accumulated_batch, _remaining_items, mut continue_cursor) =
                        client.start_query(query)?;

                    while let Some(cursor) = continue_cursor {
                        let (next_batch, _remaining_items, next_continue_cursor) =
                            <Client as QueryExecutor>::continue_query(cursor)?;

                        accumulated_batch.extend(next_batch);
                        continue_cursor = next_continue_cursor;
                    }

                    // for efficiency reasons iroha encodes query results in a columnar format,
                    // so we need to transpose the batch to get the format that is more natural for humans
                    let mut batches = vec![Vec::new(); accumulated_batch.len()];
                    for batch in accumulated_batch {
                        // downcast to json and extract the actual array
                        // dynamic typing is just easier to use here than introducing a bunch of new types only for iroha_cli
                        let batch = serde_json::to_value(batch)?;
                        let serde_json::Value::Object(batch) = batch else {
                            panic!("Expected the batch serialization to be a JSON object");
                        };
                        let (_ty, batch) = batch
                            .into_iter()
                            .next()
                            .expect("Expected the batch to have exactly one key");
                        let serde_json::Value::Array(batch_vec) = batch else {
                            panic!("Expected the batch payload to be a JSON array");
                        };
                        for (target, value) in batches.iter_mut().zip(batch_vec) {
                            target.push(value);
                        }
                    }

                    context.print_data(&batches)
                }
            }
        }
    }
}

mod transaction {
    use iroha::data_model::{isi::Log, Level as LogLevel};

    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Retrieve details of a specific transaction
        Get(Get),
        /// Send an empty transaction that logs a message
        Ping(Ping),
        /// Send a transaction using Wasm input
        Wasm(Wasm),
        /// Send a transaction using JSON5 input from stdin
        Stdin(Stdin),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match_all!((self, context), { Get, Ping, Wasm, Stdin })
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Get {
        /// Hash of the transaction to retrieve
        #[arg(short('H'), long)]
        pub hash: HashOf<SignedTransaction>,
    }

    impl Run for Get {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let transaction = client
                .query(FindTransactions)
                .filter_with(|txn| txn.value.hash.eq(self.hash))
                .execute_single()?;
            context.print_data(&transaction)
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Ping {
        /// Log levels: TRACE, DEBUG, INFO, WARN, ERROR (in increasing order of visibility)
        #[arg(short, long, default_value = "INFO")]
        pub log_level: LogLevel,
        /// Log message
        #[arg(short, long)]
        pub msg: String,
    }

    impl Run for Ping {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instruction = Log::new(self.log_level, self.msg);
            context.finish([instruction])
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Wasm {
        /// Path to the Wasm file. If omitted, reads from stdin
        #[arg(short, long)]
        path: Option<PathBuf>,
    }

    impl Run for Wasm {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let blob = if let Some(path) = self.path {
                fs::read(path).wrap_err("Failed to read a Wasm from the file into the buffer")?
            } else {
                bytes_from_stdin().wrap_err("Failed to read a Wasm from stdin into the buffer")?
            };

            context
                .finish(WasmSmartContract::from_compiled(blob))
                .wrap_err("Failed to submit a Wasm transaction")
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Stdin;

    impl Run for Stdin {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instructions: Vec<InstructionBox> = parse_json5_stdin(context)?;
            context
                .finish(instructions)
                .wrap_err("Failed to submit parsed instructions")
        }
    }
}

mod role {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write role permissions
        #[command(subcommand)]
        Permission(PermissionCommand),
        /// List role IDs
        #[command(subcommand)]
        List(List),
        /// Register a role and grant it to the registrant
        Register(Id),
        /// Unregister a role
        Unregister(Id),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Permission(cmd) => cmd.run(context),
                List(cmd) => cmd.run(context),
                Register(args) => {
                    let instruction = iroha::data_model::isi::Register::role(Role::new(
                        args.id,
                        context.config().account.clone(),
                    ));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register role")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::role(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister role")
                }
            }
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum PermissionCommand {
        /// List role permissions
        List(Id),
        /// Grant role permission using JSON5 input from stdin
        Grant(Id),
        /// Revoke role permission using JSON5 input from stdin
        Revoke(Id),
    }

    impl Run for PermissionCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::PermissionCommand::*;
            match self {
                List(args) => {
                    let client = context.client_from_config();
                    let role = client
                        .query(FindRoles)
                        .filter_with(|entry| entry.id.eq(args.id))
                        .execute_single()?;
                    for permission in role.permissions() {
                        context.print_data(&permission)?;
                    }
                    Ok(())
                }
                Grant(args) => {
                    let permission: Permission = parse_json5_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Grant::role_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to grant the permission to the role")
                }
                Revoke(args) => {
                    let permission: Permission = parse_json5_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Revoke::role_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to revoke the permission from the role")
                }
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Role name
        #[arg(short, long)]
        id: RoleId,
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all role IDs
        All,
    }

    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let ids = client.query(FindRoleIds).execute_all()?;
            context.print_data(&ids)
        }
    }
}

mod parameter {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List system parameters
        #[command(subcommand)]
        List(List),
        /// Set a system parameter using JSON5 input from stdin
        Set(Set),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match_all!((self, context), { List, Set })
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all system parameters
        All,
    }

    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let params = client.query_single(FindParameters)?;
            context.print_data(&params)
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Set;

    impl Run for Set {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let entry: Parameter = parse_json5_stdin(context)?;
            let instruction = SetParameter::new(entry);
            context.finish([instruction])
        }
    }
}

mod trigger {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List trigger IDs
        #[command(subcommand)]
        List(List),
        /// Retrieve details of a specific trigger
        // TODO: For better readability and reusability, triggers should reference a Wasm executable instead of storing the blob itself.
        Get(Id),
        /// TODO: Register a trigger
        Register(Register),
        /// Unregister a trigger
        Unregister(Id),
        /// Increase the number of trigger executions
        Mint(IdInt),
        /// Decrease the number of trigger executions
        Burn(IdInt),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::trigger::Command),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Get(args) => {
                    let client = context.client_from_config();
                    let entry = client
                        .query(FindTriggers)
                        .filter_with(|entry| entry.id.eq(args.id))
                        .execute_single()
                        .wrap_err("Failed to get trigger")?;
                    context.print_data(&entry)
                }
                Register(args) => args.run(context),
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::trigger(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister trigger")
                }
                Mint(args) => {
                    let instruction = iroha::data_model::isi::Mint::trigger_repetitions(
                        args.repetitions,
                        args.id,
                    );
                    context
                        .finish([instruction])
                        .wrap_err("Failed to mint trigger repetitions")
                }
                Burn(args) => {
                    let instruction = iroha::data_model::isi::Burn::trigger_repetitions(
                        args.repetitions,
                        args.id,
                    );
                    context
                        .finish([instruction])
                        .wrap_err("Failed to burn trigger repetitions")
                }
                Meta(cmd) => cmd.run(context),
            }
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all trigger IDs
        All,
    }

    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let ids = client.query(FindActiveTriggerIds).execute_all()?;
            context.print_data(&ids)
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Trigger name
        #[arg(short, long)]
        pub id: TriggerId,
    }

    #[derive(clap::Args, Debug)]
    pub struct IdInt {
        /// Trigger name
        #[arg(short, long)]
        pub id: TriggerId,
        /// Amount of change (integer)
        #[arg(short, long)]
        pub repetitions: u32,
    }

    #[derive(clap::Args, Debug)]
    pub struct Register;

    impl Run for Register {
        fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
            todo!()
        }
    }
}

mod executor {
    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Retrieve the executor data model
        DataModel,
        /// Upgrade the executor
        Upgrade(Upgrade),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                DataModel => {
                    let client = context.client_from_config();
                    let model = client.query_single(FindExecutorDataModel)?;
                    context.print_data(&model)
                }
                Upgrade(args) => {
                    let instruction = fs::read(args.path)
                        .map(WasmSmartContract::from_compiled)
                        .map(Executor::new)
                        .map(iroha::data_model::isi::Upgrade::new)
                        .wrap_err("Failed to read a Wasm from the file")?;
                    context.finish([instruction])
                }
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Upgrade {
        /// Path to the compiled Wasm file
        #[arg(short, long)]
        path: PathBuf,
    }
}

mod metadata {
    use super::*;

    macro_rules! impl_metadata_command {
        ($entity:ty, $query:expr, $constructor:ident) => {
            pub mod $constructor {
                use super::*;

                #[derive(clap::Subcommand, Debug)]
                pub enum Command {
                    /// Retrieve a value from the key-value store
                    Get(IdKey),
                    /// Create or update an entry in the key-value store using JSON5 input from stdin
                    Set(IdKey),
                    /// Delete an entry from the key-value store
                    Remove(IdKey),
                }

                #[derive(clap::Args, Debug)]
                pub struct IdKey {
                    #[arg(short, long)]
                    pub id: <$entity as Identifiable>::Id,
                    #[arg(short, long)]
                    pub key: Name,
                }

                impl Run for Command {
                    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                        use self::Command::*;
                        match self {
                            Get(args) => {
                                let client = context.client_from_config();
                                let value = client
                                    .query($query)
                                    .filter_with(|entry| entry.id.eq(args.id))
                                    .select_with(|entry| entry.metadata.key(args.key))
                                    .execute_single()
                                    .wrap_err("Failed to get value")?;
                                context.print_data(&value)
                            }
                            Set(args) => {
                                let value: Json = parse_json5_stdin(context)?;
                                let instruction = iroha::data_model::isi::SetKeyValue::$constructor(
                                    args.id, args.key, value,
                                );
                                context.finish([instruction])
                            }
                            Remove(args) => {
                                let instruction =
                                    iroha::data_model::isi::RemoveKeyValue::$constructor(
                                        args.id, args.key,
                                    );
                                context.finish([instruction])
                            }
                        }
                    }
                }
            }
        };
    }

    impl_metadata_command!(Domain, FindDomains, domain);
    impl_metadata_command!(Account, FindAccounts, account);
    impl_metadata_command!(AssetDefinition, FindAssetsDefinitions, asset_definition);

    // TODO apply macro after trigger.action.metadata is relocated to trigger.metadata
    pub mod trigger {
        use super::*;

        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// Retrieve a value from the key-value store
            Get(IdKey),
            /// Create or update an entry in the key-value store using JSON5 input from stdin
            Set(IdKey),
            /// Delete an entry from the key-value store
            Remove(IdKey),
        }

        #[derive(clap::Args, Debug)]
        pub struct IdKey {
            #[arg(short, long)]
            pub id: <Trigger as Identifiable>::Id,
            #[arg(short, long)]
            pub key: Name,
        }

        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let client = context.client_from_config();
                        let value = client
                            .query(FindTriggers)
                            .filter_with(|entry| entry.id.eq(args.id))
                            .select_with(|entry| entry.action.metadata.key(args.key))
                            .execute_single()
                            .wrap_err("Failed to get value")?;
                        context.print_data(&value)
                    }
                    Set(args) => {
                        let value: Json = parse_json5_stdin(context)?;
                        let instruction =
                            iroha::data_model::isi::SetKeyValue::trigger(args.id, args.key, value);
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction =
                            iroha::data_model::isi::RemoveKeyValue::trigger(args.id, args.key);
                        context.finish([instruction])
                    }
                }
            }
        }
    }
}

fn dump_json5_stdout<T>(value: &T) -> Result<()>
where
    T: serde::Serialize,
{
    let s = json5::to_string(value)?;
    io::stdout().write_all(s.as_bytes())?;
    Ok(())
}

fn parse_json5_stdin<T>(context: &impl RunContext) -> Result<T>
where
    T: for<'a> serde::Deserialize<'a>,
{
    if context.input_instructions() {
        eyre::bail!("Incompatible `--input` flag with the command")
    }
    parse_json5_stdin_unchecked()
}

fn parse_json5_stdin_unchecked<T>() -> Result<T>
where
    T: for<'a> serde::Deserialize<'a>,
{
    parse_json5(&string_from_stdin()?)
}

fn parse_json5<T>(s: &str) -> Result<T>
where
    T: for<'a> serde::Deserialize<'a>,
{
    Ok(json5::from_str(s)?)
}

fn string_from_stdin() -> Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn bytes_from_stdin() -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf)?;
    Ok(buf)
}
