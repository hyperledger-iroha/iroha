//! Genesis-related logic and constructs. Contains the [`GenesisBlock`],
//! [`GenesisSpec`] and the [`GenesisBuilder`] structures.

use std::{
    fmt::Debug,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::SystemTime,
};

use derive_more::Constructor;
use eyre::{eyre, Result, WrapErr};
use humantime::Timestamp;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{block::SignedBlock, parameter::Parameter, prelude::*};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

/// Genesis domain hardcoded for the pre-genesis state.
pub static GENESIS_DOMAIN_ID: LazyLock<DomainId> = LazyLock::new(|| "genesis".parse().unwrap());

/// Genesis account hardcoded for the pre-genesis state, used as the placeholder authority for processing genesis transactions.
pub static GENESIS_ACCOUNT_ID: LazyLock<AccountId> = LazyLock::new(|| {
    // TODO #5022: replace this seeded key with a secure, non-personal key.
    let public_key = KeyPair::from_seed(b"genesis".to_vec(), Algorithm::Ed25519)
        .into_parts()
        .0;
    AccountId::new(GENESIS_DOMAIN_ID.clone(), public_key)
});

/// Genesis block generated from [`GenesisSpec`], consisting of the following transactions:
///
/// 1. Single `Upgrade` instruction to set the executor.
/// 2. Optional `SetParameter` instructions.
/// 3. Optional instructions to preset domains, assets, roles, etc.
/// 4. Optional `Register::trigger` instructions for Wasm executables.
/// 5. Optional `Register::peer` instructions to set the initial topology.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct GenesisBlock(pub SignedBlock);

/// Format of the genesis configuration file.
/// Can be converted into a [`GenesisBlock`].
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisSpec {
    /// Timestamp for genesis block creation.
    #[serde_as(as = "DisplayFromStr")]
    creation_time: humantime::Timestamp,
    /// Unique chain identifier.
    chain: ChainId,
    /// Path to the executor file.
    executor: WasmPath,
    /// Optional parameters override.
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Parameters>,
    /// Various instructions to preset domains, assets, roles, etc.
    instructions: Vec<InstructionBox>,
    /// Directory containing additional Wasm libraries.
    wasm_dir: WasmPath,
    /// Wasm-based triggers to register.
    wasm_triggers: Vec<GenesisWasmTrigger>,
    /// Initial peer list defining the network topology.
    topology: Vec<PeerId>,
}

/// Path for either a Wasm file or a directory containing Wasm files.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmPath(PathBuf);

impl GenesisSpec {
    const WARN_ON_GENESIS_GTE: u64 = 1024 * 1024 * 1024; // 1Gb

    /// Loads a [`GenesisSpec`] from a JSON file at `path`,
    /// resolving any relative paths against the `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file is not found.
    /// - Metadata access fails.
    /// - Deserialization fails.
    pub fn from_json(path: impl AsRef<Path>) -> Result<Self> {
        let here = path
            .as_ref()
            .parent()
            .expect("json file should be in some directory");
        let file = File::open(&path)
            .wrap_err_with(|| eyre!("failed to open genesis at {}", path.as_ref().display()))?;
        let size = file
            .metadata()
            .wrap_err("failed to access genesis file metadata")?
            .len();
        if size >= Self::WARN_ON_GENESIS_GTE {
            eprintln!("Genesis is quite large, it will take some time to process it (size = {}, threshold = {})", size, Self::WARN_ON_GENESIS_GTE);
        }
        let reader = BufReader::new(file);

        let mut spec: Self = serde_json::from_reader(reader).wrap_err_with(|| {
            eyre!(
                "failed to deserialize genesis spec from {}",
                path.as_ref().display()
            )
        })?;

        spec.executor.resolve(here);
        spec.wasm_dir.resolve(here);
        spec.wasm_triggers
            .iter_mut()
            .for_each(|trigger| trigger.action.executable.resolve(&spec.wasm_dir.0));

        Ok(spec)
    }

    /// Converts this spec back into a [`GenesisBuilder`] for further modifications.
    pub fn into_builder(self) -> GenesisBuilder {
        let parameters = self
            .parameters
            .map_or(Vec::new(), |parameters| parameters.parameters().collect());

        GenesisBuilder {
            creation_time: self.creation_time,
            chain: self.chain,
            executor: self.executor,
            parameters,
            instructions: self.instructions,
            wasm_dir: self.wasm_dir.0,
            wasm_triggers: self.wasm_triggers,
            topology: self.topology,
        }
    }

    /// Converts this spec into a genesis block.
    ///
    /// - Genesis block has no block signatures.
    /// - Transaction signatures are dummy and not verified.
    /// - Transaction timestamps are set to the genesis block's.
    ///
    /// # Errors
    ///
    /// Fails if `instructions_list` fails.
    pub fn into_block(self) -> Result<GenesisBlock> {
        let chain = self.chain.clone();
        let creation_time_ms = self
            .creation_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .expect("shouldn't overflow within 584942417 years");
        let mut transactions = vec![];
        for instructions in self.into_instruction_batches()? {
            let transaction = TransactionBuilder::new_with_time(
                chain.clone(),
                GENESIS_ACCOUNT_ID.clone(),
                creation_time_ms,
            )
            .with_instructions(instructions)
            .genesis_sign();
            transactions.push(transaction);
        }

        Ok(GenesisBlock(SignedBlock::genesis(
            transactions,
            creation_time_ms,
        )))
    }

    /// Converts the [`GenesisSpec`] into batches of instructions, one batch per genesis transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if the `self.executor` path cannot be loaded as an executor,
    /// or if any of the `self.wasm_triggers` paths cannot be loaded as a Wasm executable.
    fn into_instruction_batches(self) -> Result<Vec<Vec<InstructionBox>>> {
        let mut instruction_batches = vec![];

        let upgrade_executor = Upgrade::new(Executor::new(self.executor.try_into()?)).into();
        instruction_batches.push(vec![upgrade_executor]);

        if let Some(parameters) = self.parameters {
            let instructions = parameters
                .parameters()
                .map(SetParameter::new)
                .map(InstructionBox::from)
                .collect();
            instruction_batches.push(instructions);
        }

        if !self.wasm_triggers.is_empty() {
            let instructions = self
                .wasm_triggers
                .into_iter()
                .map(Trigger::try_from)
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .map(Register::trigger)
                .map(InstructionBox::from)
                .collect();
            instruction_batches.push(instructions);
        }

        if !self.instructions.is_empty() {
            instruction_batches.push(self.instructions);
        }

        if !self.topology.is_empty() {
            let instructions = self
                .topology
                .into_iter()
                .map(Register::peer)
                .map(InstructionBox::from)
                .collect();
            instruction_batches.push(instructions)
        }

        Ok(instruction_batches)
    }
}

/// Builder for [`GenesisSpec`] and [`GenesisBlock`].
///
/// Note: The generated transactions and block are not validated.
#[must_use]
pub struct GenesisBuilder {
    creation_time: humantime::Timestamp,
    chain: ChainId,
    executor: WasmPath,
    parameters: Vec<Parameter>,
    instructions: Vec<InstructionBox>,
    wasm_dir: PathBuf,
    wasm_triggers: Vec<GenesisWasmTrigger>,
    topology: Vec<PeerId>,
}

/// Domain-specific registration mode of the [`GenesisBuilder`].
#[must_use]
pub struct GenesisDomainBuilder {
    creation_time: humantime::Timestamp,
    chain: ChainId,
    executor: WasmPath,
    parameters: Vec<Parameter>,
    instructions: Vec<InstructionBox>,
    wasm_dir: PathBuf,
    wasm_triggers: Vec<GenesisWasmTrigger>,
    topology: Vec<PeerId>,
    domain_id: DomainId,
}

impl GenesisBuilder {
    /// Construct [`GenesisBuilder`].
    pub fn new(
        chain: ChainId,
        executor: impl Into<PathBuf>,
        wasm_dir: impl Into<PathBuf>,
        creation_time: Timestamp,
    ) -> Self {
        Self {
            creation_time,
            chain,
            executor: executor.into().into(),
            parameters: Vec::new(),
            instructions: Vec::new(),
            wasm_dir: wasm_dir.into(),
            wasm_triggers: Vec::new(),
            topology: Vec::new(),
        }
    }

    /// Construct [`GenesisBuilder`] with the creation time set to `UNIX_EPOCH`.
    pub fn new_unix_epoch(
        chain: ChainId,
        executor: impl Into<PathBuf>,
        wasm_dir: impl Into<PathBuf>,
    ) -> Self {
        Self::new(chain, executor, wasm_dir, SystemTime::UNIX_EPOCH.into())
    }

    /// Entry a domain registration and transition to [`GenesisDomainBuilder`].
    pub fn domain(self, domain_name: Name) -> GenesisDomainBuilder {
        self.domain_with_metadata(domain_name, Metadata::default())
    }

    /// Same as [`GenesisBuilder::domain`], but attach a metadata to the domain.
    pub fn domain_with_metadata(
        mut self,
        domain_name: Name,
        metadata: Metadata,
    ) -> GenesisDomainBuilder {
        let domain_id = DomainId::new(domain_name);
        let new_domain = Domain::new(domain_id.clone()).with_metadata(metadata);

        self.instructions.push(Register::domain(new_domain).into());

        GenesisDomainBuilder {
            creation_time: self.creation_time,
            chain: self.chain,
            executor: self.executor,
            parameters: self.parameters,
            instructions: self.instructions,
            wasm_dir: self.wasm_dir,
            wasm_triggers: self.wasm_triggers,
            topology: self.topology,
            domain_id,
        }
    }

    /// Entry a parameter setting to the end of entries.
    pub fn append_parameter(mut self, parameter: Parameter) -> Self {
        self.parameters.push(parameter);
        self
    }

    /// Entry a instruction to the end of entries.
    pub fn append_instruction(mut self, instruction: impl Into<InstructionBox>) -> Self {
        self.instructions.push(instruction.into());
        self
    }

    /// Entry a wasm trigger to the end of entries.
    pub fn append_wasm_trigger(mut self, wasm_trigger: GenesisWasmTrigger) -> Self {
        self.wasm_triggers.push(wasm_trigger);
        self
    }

    /// Overwrite the initial topology.
    pub fn set_topology(mut self, topology: Vec<PeerId>) -> Self {
        self.topology = topology;
        self
    }

    /// Finish building and produce a [`GenesisBlock`].
    ///
    /// # Errors
    ///
    /// Fails if internal [`GenesisSpec::build_block`] fails.
    pub fn build_block(self) -> Result<GenesisBlock> {
        self.build_spec().into_block()
    }

    /// Finish building and produce a [`GenesisSpec`].
    pub fn build_spec(self) -> GenesisSpec {
        let parameters =
            (!self.parameters.is_empty()).then(|| self.parameters.into_iter().collect());

        GenesisSpec {
            creation_time: self.creation_time,
            chain: self.chain,
            executor: self.executor,
            parameters,
            instructions: self.instructions,
            wasm_dir: self.wasm_dir.into(),
            wasm_triggers: self.wasm_triggers,
            topology: self.topology,
        }
    }
}

impl GenesisDomainBuilder {
    /// Finish this domain and return to genesis block building.
    pub fn finish_domain(self) -> GenesisBuilder {
        GenesisBuilder {
            creation_time: self.creation_time,
            chain: self.chain,
            executor: self.executor,
            parameters: self.parameters,
            instructions: self.instructions,
            wasm_dir: self.wasm_dir,
            wasm_triggers: self.wasm_triggers,
            topology: self.topology,
        }
    }

    /// Add an account to this domain.
    pub fn account(self, signatory: PublicKey) -> Self {
        self.account_with_metadata(signatory, Metadata::default())
    }

    /// Add an account (having provided `metadata`) to this domain.
    pub fn account_with_metadata(mut self, signatory: PublicKey, metadata: Metadata) -> Self {
        let account_id = AccountId::new(self.domain_id.clone(), signatory);
        let register = Register::account(Account::new(account_id).with_metadata(metadata));
        self.instructions.push(register.into());
        self
    }

    /// Add [`AssetDefinition`] to this domain.
    pub fn asset(mut self, asset_name: Name, asset_spec: NumericSpec) -> Self {
        let asset_definition_id = AssetDefinitionId::new(self.domain_id.clone(), asset_name);
        let asset_definition = AssetDefinition::new(asset_definition_id, asset_spec);
        self.instructions
            .push(Register::asset_definition(asset_definition).into());
        self
    }
}

impl From<PathBuf> for WasmPath {
    fn from(value: PathBuf) -> Self {
        Self(value)
    }
}

impl TryFrom<WasmPath> for WasmSmartContract {
    type Error = eyre::Report;

    fn try_from(value: WasmPath) -> Result<Self, Self::Error> {
        let blob = fs::read(&value.0)
            .wrap_err_with(|| eyre!("failed to read wasm blob from {}", value.0.display()))?;

        Ok(WasmSmartContract::from_compiled(blob))
    }
}

impl WasmPath {
    /// Resolve `self` to `here/self`,
    /// assuming `self` is an unresolved relative path to `here`.
    /// In case `self` is absolute, it replaces `here` i.e. this method mutates nothing.
    fn resolve(&mut self, here: impl AsRef<Path>) {
        self.0 = here.as_ref().join(&self.0)
    }
}

/// Human-readable alternative to [`Trigger`] whose action has wasm executable
#[derive(Debug, Clone, Serialize, Deserialize, Constructor)]
pub struct GenesisWasmTrigger {
    id: TriggerId,
    action: GenesisWasmAction,
}

/// Human-readable alternative to [`Action`] which has wasm executable
#[derive(Debug, Clone, Serialize, Deserialize)]
// TODO: include metadata field
pub struct GenesisWasmAction {
    executable: WasmPath,
    repeats: Repeats,
    authority: AccountId,
    filter: EventFilterBox,
}

impl GenesisWasmAction {
    /// Construct [`GenesisWasmAction`]
    pub fn new(
        executable: impl Into<PathBuf>,
        repeats: impl Into<Repeats>,
        authority: AccountId,
        filter: impl Into<EventFilterBox>,
    ) -> Self {
        Self {
            executable: executable.into().into(),
            repeats: repeats.into(),
            authority,
            filter: filter.into(),
        }
    }
}

impl TryFrom<GenesisWasmTrigger> for Trigger {
    type Error = eyre::Report;

    fn try_from(value: GenesisWasmTrigger) -> Result<Self, Self::Error> {
        Ok(Trigger::new(value.id, value.action.try_into()?))
    }
}

impl TryFrom<GenesisWasmAction> for Action {
    type Error = eyre::Report;

    fn try_from(value: GenesisWasmAction) -> Result<Self, Self::Error> {
        Ok(Action::new(
            WasmSmartContract::try_from(value.executable)?,
            value.repeats,
            value.authority,
            value.filter,
        ))
    }
}

#[cfg(test)]
mod tests {
    use iroha_test_samples::{ALICE_KEYPAIR, BOB_KEYPAIR};
    use tempfile::TempDir;

    use super::*;

    fn test_builder() -> (TempDir, GenesisBuilder) {
        let tmp_dir = TempDir::new().unwrap();
        let dummy_wasm = WasmSmartContract::from_compiled(vec![1, 2, 3]);
        let executor_path = tmp_dir.path().join("executor.wasm");
        std::fs::write(&executor_path, dummy_wasm).unwrap();
        let chain = ChainId::from("00000000-0000-0000-0000-000000000000");
        let wasm_dir = tmp_dir.path().join("wasm/");
        let builder = GenesisBuilder::new_unix_epoch(chain, executor_path, wasm_dir);

        (tmp_dir, builder)
    }

    #[test]
    fn load_new_genesis_block() -> Result<()> {
        let (alice_public_key, _) = KeyPair::random().into_parts();
        let (_tmp_dir, builder) = test_builder();

        let _genesis_block = builder
            .domain("wonderland".parse()?)
            .account(alice_public_key)
            .finish_domain()
            .build_block()?;

        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn genesis_block_builder_example() -> Result<()> {
        let public_key: std::collections::HashMap<&'static str, PublicKey> = [
            ("alice", ALICE_KEYPAIR.public_key().clone()),
            ("bob", BOB_KEYPAIR.public_key().clone()),
            ("cheshire_cat", KeyPair::random().into_parts().0),
            ("mad_hatter", KeyPair::random().into_parts().0),
        ]
        .into_iter()
        .collect();
        let (_tmp_dir, mut genesis_builder) = test_builder();
        let executor_path = genesis_builder.executor.clone();

        genesis_builder = genesis_builder
            .domain("wonderland".parse().unwrap())
            .account(public_key["alice"].clone())
            .account(public_key["bob"].clone())
            .finish_domain()
            .domain("tulgey_wood".parse().unwrap())
            .account(public_key["cheshire_cat"].clone())
            .finish_domain()
            .domain("meadow".parse().unwrap())
            .account(public_key["mad_hatter"].clone())
            .asset("hats".parse().unwrap(), NumericSpec::default())
            .finish_domain();

        // In real cases executor should be constructed from a wasm blob
        let finished_genesis = genesis_builder.build_block()?;

        let transactions = &finished_genesis
            .0
            .external_transactions()
            .collect::<Vec<_>>();

        // First transaction
        {
            let transaction = transactions[0];
            let instructions = transaction.instructions();
            let Executable::Instructions(instructions) = instructions else {
                panic!("Expected instructions");
            };

            assert_eq!(
                instructions[0],
                Upgrade::new(Executor::new(executor_path.try_into()?)).into()
            );
            assert_eq!(instructions.len(), 1);
        }

        // Second transaction
        let transaction = transactions[1];
        let instructions = transaction.instructions();
        let Executable::Instructions(instructions) = instructions else {
            panic!("Expected instructions");
        };

        {
            let domain_id: DomainId = "wonderland".parse().unwrap();
            assert_eq!(
                instructions[0],
                Register::domain(Domain::new(domain_id.clone())).into()
            );
            assert_eq!(
                instructions[1],
                Register::account(Account::new(AccountId::new(
                    domain_id.clone(),
                    public_key["alice"].clone()
                ),))
                .into()
            );
            assert_eq!(
                instructions[2],
                Register::account(Account::new(AccountId::new(
                    domain_id,
                    public_key["bob"].clone()
                ),))
                .into()
            );
        }
        {
            let domain_id: DomainId = "tulgey_wood".parse().unwrap();
            assert_eq!(
                instructions[3],
                Register::domain(Domain::new(domain_id.clone())).into()
            );
            assert_eq!(
                instructions[4],
                Register::account(Account::new(AccountId::new(
                    domain_id,
                    public_key["cheshire_cat"].clone()
                ),))
                .into()
            );
        }
        {
            let domain_id: DomainId = "meadow".parse().unwrap();
            assert_eq!(
                instructions[5],
                Register::domain(Domain::new(domain_id.clone())).into()
            );
            assert_eq!(
                instructions[6],
                Register::account(Account::new(AccountId::new(
                    domain_id,
                    public_key["mad_hatter"].clone()
                ),))
                .into()
            );
            assert_eq!(
                instructions[7],
                Register::asset_definition(AssetDefinition::numeric(
                    "hats#meadow".parse().unwrap(),
                ))
                .into()
            );
        }

        Ok(())
    }

    #[test]
    fn genesis_parameters_deserialization() {
        fn test(parameters: &str) {
            let genesis_json = format!(
                r#"{{
                "creation_time": "1970-01-01T00:00:00Z",
                "chain": "0",
                "executor": "executor.wasm",
                "parameters": {parameters},
                "instructions": [],
                "wasm_dir": "libs",
                "wasm_triggers": [],
                "topology": []
                }}"#
            );

            let _genesis: GenesisSpec =
                serde_json::from_str(&genesis_json).expect("Failed to deserialize");
        }

        // Empty parameters
        test("{}");
        test(
            r#"{"sumeragi": {}, "block": {}, "transaction": {}, "executor": {}, "smart_contract": {}}"#,
        );

        // Inner value missing
        test(r#"{"sumeragi": {"block_time_ms": 2000}}"#);
        test(r#"{"transaction": {"max_instructions": 4096}}"#);
        test(r#"{"executor": {"fuel": 55000000}}"#);
        test(r#"{"smart_contract": {"fuel": 55000000}}"#);
    }
}
