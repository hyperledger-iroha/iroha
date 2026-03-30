//! Transaction composition helpers for the MOCHI UI.
//!
//! The composer utilities provide lightweight building blocks for
//! transaction previews so front ends can offer form-based instruction
//! builders without pulling additional crates.

use std::{
    collections::BTreeSet,
    fmt,
    num::{NonZeroU32, NonZeroU64},
    str::FromStr,
    sync::LazyLock,
    time::Duration,
};

use iroha_crypto::KeyPair;
use iroha_data_model::{
    ChainId,
    account::{
        ACCOUNT_ADMISSION_POLICY_METADATA_KEY, Account, AccountAdmissionMode,
        AccountAdmissionPolicy, AccountId, ScopedAccountId,
        admission::ImplicitAccountFeeDestination,
    },
    asset::{
        definition::{AssetDefinition, Mintable, NewAssetDefinition},
        id::{AssetDefinitionId, AssetId},
    },
    domain::{Domain, DomainId},
    isi::{
        Burn, Grant, InstructionBox, Mint, Register, Revoke, SetKeyValue, Transfer,
        sorafs::RegisterPinManifest, space_directory::PublishSpaceDirectoryManifest,
    },
    name::Name,
    nexus::AssetPermissionManifest,
    role::RoleId,
    transaction::{Executable, SignedTransaction, TransactionBuilder},
};
use iroha_executor_data_model::isi::multisig::MultisigPropose;
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericError},
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use iroha_version::codec::EncodeVersioned;
use norito::json::{self, Map, Value};

/// Permission categories used to gate high-level instruction templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstructionPermission {
    /// Mint numeric assets.
    MintAsset,
    /// Burn numeric assets.
    BurnAsset,
    /// Transfer numeric assets between accounts.
    TransferAsset,
    /// Register new domains.
    RegisterDomain,
    /// Register new accounts.
    RegisterAccount,
    /// Register new asset definitions.
    RegisterAssetDefinition,
    /// Publish Space Directory manifests (AXT/AMX policies).
    PublishSpaceDirectoryManifest,
    /// Register SoraFS pin manifests (DA pin intents).
    RegisterPinManifest,
    /// Set account admission policies on domains.
    AccountAdmissionPolicy,
    /// Grant roles to accounts.
    GrantRole,
    /// Revoke roles from accounts.
    RevokeRole,
    /// Propose multisig transactions.
    MultisigPropose,
}

impl InstructionPermission {
    /// Return a static list containing every permission variant.
    #[must_use]
    pub const fn all() -> [Self; 12] {
        [
            Self::MintAsset,
            Self::BurnAsset,
            Self::TransferAsset,
            Self::RegisterDomain,
            Self::RegisterAccount,
            Self::RegisterAssetDefinition,
            Self::PublishSpaceDirectoryManifest,
            Self::RegisterPinManifest,
            Self::AccountAdmissionPolicy,
            Self::GrantRole,
            Self::RevokeRole,
            Self::MultisigPropose,
        ]
    }

    /// Human-readable label used in error messages and tooltips.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MintAsset => "mint assets",
            Self::BurnAsset => "burn assets",
            Self::TransferAsset => "transfer assets",
            Self::RegisterDomain => "register domains",
            Self::RegisterAccount => "register accounts",
            Self::RegisterAssetDefinition => "register asset definitions",
            Self::PublishSpaceDirectoryManifest => "publish space directory manifests",
            Self::RegisterPinManifest => "register pin manifests",
            Self::AccountAdmissionPolicy => "set account admission policies",
            Self::GrantRole => "grant roles",
            Self::RevokeRole => "revoke roles",
            Self::MultisigPropose => "propose multisig transactions",
        }
    }

    /// Stable key used for persistence.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::MintAsset => "mint_asset",
            Self::BurnAsset => "burn_asset",
            Self::TransferAsset => "transfer_asset",
            Self::RegisterDomain => "register_domain",
            Self::RegisterAccount => "register_account",
            Self::RegisterAssetDefinition => "register_asset_definition",
            Self::PublishSpaceDirectoryManifest => "publish_space_directory_manifest",
            Self::RegisterPinManifest => "register_pin_manifest",
            Self::AccountAdmissionPolicy => "account_admission_policy",
            Self::GrantRole => "grant_role",
            Self::RevokeRole => "revoke_role",
            Self::MultisigPropose => "multisig_propose",
        }
    }

    /// Convert a persisted key back into an [`InstructionPermission`].
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "mint_asset" => Some(Self::MintAsset),
            "burn_asset" => Some(Self::BurnAsset),
            "transfer_asset" => Some(Self::TransferAsset),
            "register_domain" => Some(Self::RegisterDomain),
            "register_account" => Some(Self::RegisterAccount),
            "register_asset_definition" => Some(Self::RegisterAssetDefinition),
            "publish_space_directory_manifest" => Some(Self::PublishSpaceDirectoryManifest),
            "register_pin_manifest" => Some(Self::RegisterPinManifest),
            "account_admission_policy" => Some(Self::AccountAdmissionPolicy),
            "grant_role" => Some(Self::GrantRole),
            "revoke_role" => Some(Self::RevokeRole),
            "multisig_propose" => Some(Self::MultisigPropose),
            _ => None,
        }
    }
}

impl fmt::Display for InstructionPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Errors that can occur while preparing transaction previews.
#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    /// The provided asset identifier could not be parsed.
    #[error("failed to parse asset id `{asset}`: {reason}")]
    InvalidAssetId {
        /// String representation of the asset identifier that failed to parse.
        asset: String,
        /// Human readable failure reason.
        reason: String,
    },
    /// The provided account identifier could not be parsed.
    #[error("failed to parse account id `{account}`: {reason}")]
    InvalidAccountId {
        /// String representation of the account identifier that failed to parse.
        account: String,
        /// Human readable failure reason.
        reason: String,
    },
    /// The provided numeric quantity could not be parsed or was invalid.
    #[error("failed to parse quantity `{quantity}`: {reason}")]
    InvalidQuantity {
        /// Text entered by the user.
        quantity: String,
        /// Human readable failure reason.
        reason: String,
    },
    /// The provided domain identifier could not be parsed.
    #[error("failed to parse domain id `{domain}`: {reason}")]
    InvalidDomainId {
        /// String representation of the domain identifier that failed to parse.
        domain: String,
        /// Human readable failure reason.
        reason: String,
    },
    /// The provided asset definition identifier could not be parsed.
    #[error("failed to parse asset definition id `{definition}`: {reason}")]
    InvalidAssetDefinitionId {
        /// String representation of the asset definition identifier that failed to parse.
        definition: String,
        /// Human readable failure reason.
        reason: String,
    },
    /// The provided role identifier could not be parsed.
    #[error("failed to parse role id `{role}`: {reason}")]
    InvalidRoleId {
        /// String representation of the role identifier that failed to parse.
        role: String,
        /// Human readable failure reason.
        reason: String,
    },
    /// Raw draft payload contained invalid structure or values.
    #[error("invalid raw instruction draft: {reason}")]
    InvalidRawDraft {
        /// Human readable failure reason.
        reason: String,
    },
    /// No instructions supplied when attempting to compose a transaction.
    #[error("cannot build preview with an empty instruction list")]
    EmptyInstructions,
    /// Selected signing authority lacks permission for the requested instruction.
    #[error("{signer} cannot {action}")]
    UnauthorizedInstruction {
        /// Label of the signing authority.
        signer: String,
        /// Action that was denied.
        action: InstructionPermission,
    },
}

/// Named signing authority used to author transactions in local workflows.
#[derive(Debug, Clone)]
pub struct SigningAuthority {
    label: String,
    account: AccountId,
    key_pair: KeyPair,
    allowed: BTreeSet<InstructionPermission>,
    roles: BTreeSet<RoleId>,
}

impl SigningAuthority {
    /// Create a new signing authority entry.
    #[must_use]
    pub fn new(label: impl Into<String>, account: AccountId, key_pair: KeyPair) -> Self {
        Self::with_permissions(label, account, key_pair, InstructionPermission::all())
    }

    /// Create a signing authority with an explicit permission set.
    #[must_use]
    pub fn with_permissions(
        label: impl Into<String>,
        account: AccountId,
        key_pair: KeyPair,
        permissions: impl IntoIterator<Item = InstructionPermission>,
    ) -> Self {
        Self {
            label: label.into(),
            account,
            key_pair,
            allowed: permissions.into_iter().collect(),
            roles: BTreeSet::new(),
        }
    }

    /// Create a signing authority with explicit permissions and roles.
    #[must_use]
    pub fn with_permissions_and_roles(
        label: impl Into<String>,
        account: AccountId,
        key_pair: KeyPair,
        permissions: impl IntoIterator<Item = InstructionPermission>,
        roles: impl IntoIterator<Item = RoleId>,
    ) -> Self {
        Self {
            label: label.into(),
            account,
            key_pair,
            allowed: permissions.into_iter().collect(),
            roles: roles.into_iter().collect(),
        }
    }

    /// Human-readable label suitable for UI selectors.
    #[must_use]
    pub fn label(&self) -> &str {
        self.label.as_str()
    }

    /// Account identifier that will act as the transaction authority.
    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.account
    }

    /// Key pair used for signing.
    #[must_use]
    pub fn key_pair(&self) -> &KeyPair {
        &self.key_pair
    }

    /// Iterator over permission categories allowed for this authority.
    pub fn permissions(&self) -> impl Iterator<Item = InstructionPermission> + '_ {
        self.allowed.iter().copied()
    }

    /// Iterator over configured role identifiers for this authority.
    pub fn roles(&self) -> impl Iterator<Item = &RoleId> + '_ {
        self.roles.iter()
    }

    /// Check whether the authority may execute the provided permission.
    #[must_use]
    pub fn allows_permission(&self, permission: InstructionPermission) -> bool {
        self.allowed.contains(&permission)
    }

    /// Ensure that every draft in the provided slice is authorised for this signer.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError::UnauthorizedInstruction`] when a draft requires a
    /// permission the signer does not possess.
    pub fn validate_drafts(&self, drafts: &[InstructionDraft]) -> Result<(), ComposeError> {
        if let Some(denied) = drafts
            .iter()
            .find(|draft| !self.allows_permission(draft.permission()))
        {
            return Err(ComposeError::UnauthorizedInstruction {
                signer: self.label.clone(),
                action: denied.permission(),
            });
        }
        Ok(())
    }
}

static DEVELOPMENT_AUTHORITIES: LazyLock<Vec<SigningAuthority>> = LazyLock::new(|| {
    let all_permissions = InstructionPermission::all();
    vec![
        SigningAuthority::with_permissions(
            "Alice (dev)",
            ALICE_ID.clone(),
            ALICE_KEYPAIR.clone(),
            all_permissions.into_iter(),
        ),
        SigningAuthority::with_permissions(
            "Bob (dev)",
            BOB_ID.clone(),
            BOB_KEYPAIR.clone(),
            [InstructionPermission::TransferAsset].into_iter(),
        ),
    ]
});

static ACCOUNT_ADMISSION_POLICY_KEY: LazyLock<Name> = LazyLock::new(|| {
    ACCOUNT_ADMISSION_POLICY_METADATA_KEY
        .parse()
        .expect("account admission policy metadata key must be valid")
});

fn default_authority() -> &'static SigningAuthority {
    DEVELOPMENT_AUTHORITIES
        .first()
        .expect("development authorities must not be empty")
}

/// Access the bundled development signing authorities.
#[must_use]
pub fn development_signing_authorities() -> &'static [SigningAuthority] {
    DEVELOPMENT_AUTHORITIES.as_slice()
}

/// Optional overrides applied when composing transactions.
#[derive(Debug, Clone, Copy)]
pub struct TransactionComposeOptions {
    ttl: Option<Duration>,
    creation_time: Option<Duration>,
    nonce: Option<NonZeroU32>,
}

impl TransactionComposeOptions {
    /// Create a new options struct with no overrides.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ttl: None,
            creation_time: None,
            nonce: None,
        }
    }

    /// Override the time-to-live field using the supplied duration.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Freeze the creation time to a specific timestamp.
    #[must_use]
    pub fn with_creation_time(mut self, creation_time: Duration) -> Self {
        self.creation_time = Some(creation_time);
        self
    }

    /// Override the nonce recorded in the transaction payload.
    #[must_use]
    pub fn with_nonce(mut self, nonce: NonZeroU32) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Access the configured TTL override, if any.
    #[must_use]
    pub const fn ttl(&self) -> Option<Duration> {
        self.ttl
    }

    /// Access the configured creation timestamp override, if any.
    #[must_use]
    pub const fn creation_time(&self) -> Option<Duration> {
        self.creation_time
    }

    /// Access the configured nonce override, if any.
    #[must_use]
    pub const fn nonce(&self) -> Option<NonZeroU32> {
        self.nonce
    }
}

impl Default for TransactionComposeOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight summary of a composed transaction ready for preview or submission.
#[derive(Debug, Clone)]
pub struct TransactionPreview {
    signed: SignedTransaction,
    encoded_hex: String,
    instructions: Vec<String>,
    hash: String,
    authority: String,
}

impl TransactionPreview {
    /// Canonical transaction hash derived from the signed payload.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Hex-encoded Norito payload suitable for Torii submission.
    #[must_use]
    pub fn encoded_hex(&self) -> &str {
        &self.encoded_hex
    }

    /// Versioned Norito payload bytes ready to POST to `/transaction`.
    #[must_use]
    pub fn encoded_bytes(&self) -> Vec<u8> {
        self.signed.encode_versioned()
    }

    /// Human-readable list of instruction descriptions contained in the transaction.
    #[must_use]
    pub fn instructions(&self) -> &[String] {
        &self.instructions
    }

    /// Authority account recorded in the transaction payload.
    #[must_use]
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Access the underlying signed transaction for advanced consumers.
    #[must_use]
    pub fn signed_transaction(&self) -> &SignedTransaction {
        &self.signed
    }

    /// Time-to-live override expressed as a duration, if present.
    #[must_use]
    pub fn time_to_live(&self) -> Option<Duration> {
        self.signed.time_to_live()
    }

    /// Creation timestamp attached to the transaction payload.
    #[must_use]
    pub fn creation_time(&self) -> Duration {
        self.signed.creation_time()
    }

    /// Nonce recorded in the payload, if supplied.
    #[must_use]
    pub fn nonce(&self) -> Option<NonZeroU32> {
        self.signed.nonce()
    }
}

impl TransactionPreview {
    fn new(signed: SignedTransaction) -> Self {
        let encoded_hex = encode_hex(&signed.encode_versioned());
        let hash = signed.hash().to_string();
        let authority = account_literal(signed.authority());
        let instructions = match signed.instructions() {
            Executable::Instructions(list) => list.iter().map(|instr| format!("{instr}")).collect(),
            Executable::IvmProved(proved) => proved
                .overlay
                .iter()
                .map(|instr| format!("{instr}"))
                .collect(),
            Executable::Ivm(_) => vec!["IVM bytecode executable".to_owned()],
        };
        Self {
            signed,
            encoded_hex,
            instructions,
            hash,
            authority,
        }
    }
}

/// Compose a numeric asset mint transaction assigned to the default MOCHI signer.
///
/// The helper signs the transaction with the sample Alice key pair so local
/// deployments have a predictable authority without requiring user input.
///
/// # Errors
///
/// Returns [`ComposeError::InvalidAssetId`] when the supplied identifier cannot
/// be parsed according to Iroha's asset id rules.
pub fn mint_numeric_preview(
    chain_id: &str,
    asset_id: &str,
    quantity: impl Into<Numeric>,
) -> Result<TransactionPreview, ComposeError> {
    let draft = InstructionDraft::MintAsset {
        asset: parse_asset_id(asset_id)?,
        quantity: quantity.into(),
    };
    compose_preview_with_authority(chain_id, &[draft], default_authority())
}

/// Compose a transaction preview from a list of [`InstructionDraft`] entries.
///
/// # Errors
///
/// Returns [`ComposeError::EmptyInstructions`] when the provided slice is empty.
pub fn compose_preview(
    chain_id: &str,
    drafts: &[InstructionDraft],
) -> Result<TransactionPreview, ComposeError> {
    compose_preview_with_authority(chain_id, drafts, default_authority())
}

/// Compose a transaction preview signed by the provided authority.
///
/// # Errors
///
/// Returns [`ComposeError::EmptyInstructions`] when the provided slice is empty.
pub fn compose_preview_with_authority(
    chain_id: &str,
    drafts: &[InstructionDraft],
    authority: &SigningAuthority,
) -> Result<TransactionPreview, ComposeError> {
    compose_preview_with_options(
        chain_id,
        drafts,
        authority,
        &TransactionComposeOptions::default(),
    )
}

/// Compose a preview while applying the supplied [`TransactionComposeOptions`].
///
/// # Errors
///
/// Returns [`ComposeError::EmptyInstructions`] when the provided slice is empty.
pub fn compose_preview_with_options(
    chain_id: &str,
    drafts: &[InstructionDraft],
    authority: &SigningAuthority,
    options: &TransactionComposeOptions,
) -> Result<TransactionPreview, ComposeError> {
    if drafts.is_empty() {
        return Err(ComposeError::EmptyInstructions);
    }
    authority.validate_drafts(drafts)?;
    let chain = ChainId::from(chain_id.to_owned());
    let instructions = drafts
        .iter()
        .map(InstructionDraft::instruction)
        .collect::<Vec<_>>();
    let mut builder = TransactionBuilder::new(chain, authority.account_id().clone())
        .with_instructions(instructions);
    if let Some(creation_time) = options.creation_time() {
        builder.set_creation_time(creation_time);
    }
    if let Some(ttl) = options.ttl() {
        builder.set_ttl(ttl);
    }
    if let Some(nonce) = options.nonce() {
        builder.set_nonce(nonce);
    }
    let signed = builder.sign(authority.key_pair().private_key());
    Ok(TransactionPreview::new(signed))
}

/// Declarative specification for a high-level instruction created in the UI.
#[derive(Debug, Clone)]
pub enum InstructionDraft {
    /// Mint a numeric quantity of the specified asset.
    MintAsset {
        /// Identifier of the asset being minted.
        asset: AssetId,
        /// Positive numeric quantity to mint.
        quantity: Numeric,
    },
    /// Burn a numeric quantity of the specified asset.
    BurnAsset {
        /// Identifier of the asset being burned.
        asset: AssetId,
        /// Quantity to burn.
        quantity: Numeric,
    },
    /// Transfer a numeric asset between accounts.
    TransferAsset {
        /// Identifier of the asset (including source account) to transfer.
        asset: AssetId,
        /// Quantity to transfer.
        quantity: Numeric,
        /// Destination account receiving the asset.
        destination: AccountId,
    },
    /// Register a new domain.
    RegisterDomain {
        /// Identifier of the domain to register.
        domain: DomainId,
    },
    /// Register a new account.
    RegisterAccount {
        /// Identifier of the account to register.
        account: ScopedAccountId,
    },
    /// Register a numeric asset definition.
    RegisterAssetDefinition {
        /// Identifier of the asset definition.
        definition: AssetDefinitionId,
        /// Mintability policy for the asset.
        mintable: Mintable,
    },
    /// Publish a Space Directory manifest for AXT/AMX policies.
    PublishSpaceDirectoryManifest {
        /// Manifest payload to publish.
        manifest: AssetPermissionManifest,
    },
    /// Register a SoraFS pin manifest (DA pin intent).
    RegisterPinManifest {
        /// Pin manifest registration payload.
        request: RegisterPinManifest,
    },
    /// Set the account admission policy for a domain.
    SetAccountAdmissionPolicy {
        /// Target domain for the policy.
        domain: DomainId,
        /// Policy configuration payload.
        policy: AccountAdmissionPolicy,
    },
    /// Grant a role to an account.
    GrantRole {
        /// Identifier of the role to grant.
        role: RoleId,
        /// Target account receiving the role.
        account: AccountId,
    },
    /// Revoke a role from an account.
    RevokeRole {
        /// Identifier of the role to revoke.
        role: RoleId,
        /// Target account losing the role.
        account: AccountId,
    },
    /// Propose a multisig transaction for a multisig account.
    MultisigPropose {
        /// Multisig account issuing the proposal.
        account: AccountId,
        /// Instruction drafts to include in the proposal.
        instructions: Vec<InstructionDraft>,
        /// Optional TTL override in milliseconds for the proposal.
        transaction_ttl_ms: Option<NonZeroU64>,
    },
}

impl InstructionDraft {
    /// Create a mint draft by parsing textual inputs collected from the UI.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if any field fails to parse.
    pub fn mint_from_input(asset: &str, quantity: &str) -> Result<Self, ComposeError> {
        let asset = parse_asset_id(asset)?;
        let quantity = parse_quantity(quantity)?;
        Ok(Self::MintAsset { asset, quantity })
    }

    /// Create a burn draft by parsing textual inputs collected from the UI.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if any field fails to parse.
    pub fn burn_from_input(asset: &str, quantity: &str) -> Result<Self, ComposeError> {
        let asset = parse_asset_id(asset)?;
        let quantity = parse_quantity(quantity)?;
        Ok(Self::BurnAsset { asset, quantity })
    }

    /// Create a transfer draft by parsing textual inputs collected from the UI.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if any field fails to parse.
    pub fn transfer_from_input(
        asset: &str,
        quantity: &str,
        destination: &str,
    ) -> Result<Self, ComposeError> {
        let asset = parse_asset_id(asset)?;
        let quantity = parse_quantity(quantity)?;
        let destination = parse_account_id(destination)?;
        Ok(Self::TransferAsset {
            asset,
            quantity,
            destination,
        })
    }

    /// Create a domain registration draft by parsing textual inputs.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if the domain identifier fails to parse.
    pub fn register_domain_from_input(domain: &str) -> Result<Self, ComposeError> {
        let domain = parse_domain_id(domain)?;
        Ok(Self::RegisterDomain { domain })
    }

    /// Create an account registration draft by parsing textual inputs.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if the account identifier fails to parse.
    pub fn register_account_from_input(account: &str) -> Result<Self, ComposeError> {
        let account = parse_scoped_account_id(account)?;
        Ok(Self::RegisterAccount { account })
    }

    /// Create an asset definition registration draft from textual inputs.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if any field fails to parse.
    pub fn register_asset_definition_from_input(
        definition: &str,
        mintable: Mintable,
    ) -> Result<Self, ComposeError> {
        let definition = parse_asset_definition_id(definition)?;
        Ok(Self::RegisterAssetDefinition {
            definition,
            mintable,
        })
    }

    /// Create a Space Directory manifest publication draft from JSON input.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if the manifest payload fails to decode.
    pub fn publish_space_directory_manifest_from_json(
        manifest_json: &str,
    ) -> Result<Self, ComposeError> {
        let manifest: AssetPermissionManifest =
            json::from_str(manifest_json).map_err(|err| ComposeError::InvalidRawDraft {
                reason: format!("invalid space directory manifest: {err}"),
            })?;
        Ok(Self::PublishSpaceDirectoryManifest { manifest })
    }

    /// Create a pin manifest registration draft from JSON input.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if the payload fails to decode.
    pub fn register_pin_manifest_from_json(request_json: &str) -> Result<Self, ComposeError> {
        let request: RegisterPinManifest =
            json::from_str(request_json).map_err(|err| ComposeError::InvalidRawDraft {
                reason: format!("invalid pin manifest request: {err}"),
            })?;
        Ok(Self::RegisterPinManifest { request })
    }

    /// Create an account admission policy draft by parsing textual inputs.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if the domain identifier fails to parse.
    pub fn account_admission_policy_from_input(
        domain: &str,
        policy: AccountAdmissionPolicy,
    ) -> Result<Self, ComposeError> {
        let domain = parse_domain_id(domain)?;
        Ok(Self::SetAccountAdmissionPolicy { domain, policy })
    }

    /// Create a grant role draft by parsing textual inputs.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if either identifier fails to parse.
    pub fn grant_role_from_input(role: &str, account: &str) -> Result<Self, ComposeError> {
        let role = parse_role_id(role)?;
        let account = parse_account_id(account)?;
        Ok(Self::GrantRole { role, account })
    }

    /// Create a revoke role draft by parsing textual inputs.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if either identifier fails to parse.
    pub fn revoke_role_from_input(role: &str, account: &str) -> Result<Self, ComposeError> {
        let role = parse_role_id(role)?;
        let account = parse_account_id(account)?;
        Ok(Self::RevokeRole { role, account })
    }

    /// Create a multisig proposal draft from JSON-encoded instruction drafts.
    ///
    /// # Errors
    ///
    /// Returns a [`ComposeError`] if any field fails to parse.
    pub fn multisig_propose_from_json(
        account: &str,
        instructions_json: &str,
        transaction_ttl_ms: Option<u64>,
    ) -> Result<Self, ComposeError> {
        let account = parse_account_id(account)?;
        let instructions = drafts_from_json_str(instructions_json)?;
        if instructions.is_empty() {
            return Err(ComposeError::InvalidRawDraft {
                reason: "multisig proposal instructions must not be empty".to_owned(),
            });
        }
        let transaction_ttl_ms = match transaction_ttl_ms {
            Some(value) => {
                Some(
                    NonZeroU64::new(value).ok_or_else(|| ComposeError::InvalidRawDraft {
                        reason: "multisig transaction TTL must be greater than zero".to_owned(),
                    })?,
                )
            }
            None => None,
        };
        Ok(Self::MultisigPropose {
            account,
            instructions,
            transaction_ttl_ms,
        })
    }

    /// Permission category associated with this draft.
    #[must_use]
    pub const fn permission(&self) -> InstructionPermission {
        match self {
            InstructionDraft::MintAsset { .. } => InstructionPermission::MintAsset,
            InstructionDraft::BurnAsset { .. } => InstructionPermission::BurnAsset,
            InstructionDraft::TransferAsset { .. } => InstructionPermission::TransferAsset,
            InstructionDraft::RegisterDomain { .. } => InstructionPermission::RegisterDomain,
            InstructionDraft::RegisterAccount { .. } => InstructionPermission::RegisterAccount,
            InstructionDraft::RegisterAssetDefinition { .. } => {
                InstructionPermission::RegisterAssetDefinition
            }
            InstructionDraft::PublishSpaceDirectoryManifest { .. } => {
                InstructionPermission::PublishSpaceDirectoryManifest
            }
            InstructionDraft::RegisterPinManifest { .. } => {
                InstructionPermission::RegisterPinManifest
            }
            InstructionDraft::SetAccountAdmissionPolicy { .. } => {
                InstructionPermission::AccountAdmissionPolicy
            }
            InstructionDraft::GrantRole { .. } => InstructionPermission::GrantRole,
            InstructionDraft::RevokeRole { .. } => InstructionPermission::RevokeRole,
            InstructionDraft::MultisigPropose { .. } => InstructionPermission::MultisigPropose,
        }
    }

    /// Produce a human-readable summary suitable for list views.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            InstructionDraft::MintAsset { asset, quantity } => {
                format!("Mint {quantity} to {asset}")
            }
            InstructionDraft::BurnAsset { asset, quantity } => {
                format!("Burn {quantity} from {asset}")
            }
            InstructionDraft::TransferAsset {
                asset,
                quantity,
                destination,
            } => format!("Transfer {quantity} from {asset} to {destination}"),
            InstructionDraft::RegisterDomain { domain } => {
                format!("Register domain {domain}")
            }
            InstructionDraft::RegisterAccount { account } => {
                format!("Register account {account}")
            }
            InstructionDraft::RegisterAssetDefinition {
                definition,
                mintable,
            } => format!("Register asset definition {definition} ({mintable})"),
            InstructionDraft::PublishSpaceDirectoryManifest { manifest } => format!(
                "Publish Space Directory manifest for {} (dataspace {}, {} entr{})",
                manifest.uaid,
                manifest.dataspace,
                manifest.entries.len(),
                if manifest.entries.len() == 1 {
                    "y"
                } else {
                    "ies"
                }
            ),
            InstructionDraft::RegisterPinManifest { request } => {
                let digest = encode_hex(request.digest.as_bytes());
                let short = &digest[..8.min(digest.len())];
                format!(
                    "Register pin manifest {} (epoch {})",
                    short, request.submitted_epoch
                )
            }
            InstructionDraft::SetAccountAdmissionPolicy { domain, policy } => format!(
                "Set account admission policy for {domain} ({})",
                format_admission_policy_summary(policy)
            ),
            InstructionDraft::GrantRole { role, account } => {
                format!("Grant role {role} to {account}")
            }
            InstructionDraft::RevokeRole { role, account } => {
                format!("Revoke role {role} from {account}")
            }
            InstructionDraft::MultisigPropose {
                account,
                instructions,
                ..
            } => format!(
                "Propose multisig transaction for {account} ({} instruction{})",
                instructions.len(),
                if instructions.len() == 1 { "" } else { "s" }
            ),
        }
    }

    fn instruction(&self) -> InstructionBox {
        match self {
            InstructionDraft::MintAsset { asset, quantity } => {
                Mint::asset_numeric(quantity.clone(), asset.clone()).into()
            }
            InstructionDraft::BurnAsset { asset, quantity } => {
                Burn::asset_numeric(quantity.clone(), asset.clone()).into()
            }
            InstructionDraft::TransferAsset {
                asset,
                quantity,
                destination,
            } => {
                Transfer::asset_numeric(asset.clone(), quantity.clone(), destination.clone()).into()
            }
            InstructionDraft::RegisterDomain { domain } => {
                Register::domain(Domain::new(domain.clone())).into()
            }
            InstructionDraft::RegisterAccount { account } => Register::account(
                Account::new(account.account().clone())
                    ,
            )
            .into(),
            InstructionDraft::RegisterAssetDefinition {
                definition,
                mintable,
            } => {
                let builder =
                    apply_mintable(AssetDefinition::numeric(definition.clone()), *mintable);
                Register::asset_definition(builder).into()
            }
            InstructionDraft::PublishSpaceDirectoryManifest { manifest } => {
                InstructionBox::from(PublishSpaceDirectoryManifest {
                    manifest: manifest.clone(),
                })
            }
            InstructionDraft::RegisterPinManifest { request } => {
                InstructionBox::from(request.clone())
            }
            InstructionDraft::SetAccountAdmissionPolicy { domain, policy } => SetKeyValue::domain(
                domain.clone(),
                ACCOUNT_ADMISSION_POLICY_KEY.clone(),
                Json::new(policy.clone()),
            )
            .into(),
            InstructionDraft::GrantRole { role, account } => {
                Grant::account_role(role.clone(), account.clone()).into()
            }
            InstructionDraft::RevokeRole { role, account } => {
                Revoke::account_role(role.clone(), account.clone()).into()
            }
            InstructionDraft::MultisigPropose {
                account,
                instructions,
                transaction_ttl_ms,
            } => {
                let payload = instructions
                    .iter()
                    .map(InstructionDraft::instruction)
                    .collect::<Vec<_>>();
                InstructionBox::from(MultisigPropose::new(
                    account.clone(),
                    payload,
                    *transaction_ttl_ms,
                ))
            }
        }
    }

    /// Convert the draft into a JSON object suitable for the raw editor.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let mut object = Map::new();
        match self {
            InstructionDraft::MintAsset { asset, quantity } => {
                object.insert("kind".to_owned(), Value::String("mint_asset".to_owned()));
                object.insert("asset".to_owned(), Value::String(asset_literal(asset)));
                object.insert("quantity".to_owned(), Value::String(quantity.to_string()));
            }
            InstructionDraft::BurnAsset { asset, quantity } => {
                object.insert("kind".to_owned(), Value::String("burn_asset".to_owned()));
                object.insert("asset".to_owned(), Value::String(asset_literal(asset)));
                object.insert("quantity".to_owned(), Value::String(quantity.to_string()));
            }
            InstructionDraft::TransferAsset {
                asset,
                quantity,
                destination,
            } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("transfer_asset".to_owned()),
                );
                object.insert("asset".to_owned(), Value::String(asset_literal(asset)));
                object.insert("quantity".to_owned(), Value::String(quantity.to_string()));
                object.insert(
                    "destination".to_owned(),
                    Value::String(account_literal(destination)),
                );
            }
            InstructionDraft::RegisterDomain { domain } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("register_domain".to_owned()),
                );
                object.insert("domain".to_owned(), Value::String(domain.to_string()));
            }
            InstructionDraft::RegisterAccount { account } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("register_account".to_owned()),
                );
                object.insert("account".to_owned(), Value::String(account.to_string()));
            }
            InstructionDraft::RegisterAssetDefinition {
                definition,
                mintable,
            } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("register_asset_definition".to_owned()),
                );
                object.insert(
                    "definition".to_owned(),
                    Value::String(definition.to_string()),
                );
                object.insert(
                    "mintable".to_owned(),
                    Value::String(format_mintable(*mintable)),
                );
            }
            InstructionDraft::PublishSpaceDirectoryManifest { manifest } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("publish_space_directory_manifest".to_owned()),
                );
                let value =
                    json::to_value(manifest).expect("space directory manifest should serialize");
                object.insert("manifest".to_owned(), value);
            }
            InstructionDraft::RegisterPinManifest { request } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("register_pin_manifest".to_owned()),
                );
                let value = json::to_value(request).expect("pin manifest request should serialize");
                object.insert("request".to_owned(), value);
            }
            InstructionDraft::SetAccountAdmissionPolicy { domain, policy } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("account_admission_policy".to_owned()),
                );
                object.insert("domain".to_owned(), Value::String(domain.to_string()));
                let value =
                    json::to_value(policy).expect("account admission policy should serialize");
                object.insert("policy".to_owned(), value);
            }
            InstructionDraft::GrantRole { role, account } => {
                object.insert("kind".to_owned(), Value::String("grant_role".to_owned()));
                object.insert("role".to_owned(), Value::String(role.to_string()));
                object.insert(
                    "account".to_owned(),
                    Value::String(account_literal(account)),
                );
            }
            InstructionDraft::RevokeRole { role, account } => {
                object.insert("kind".to_owned(), Value::String("revoke_role".to_owned()));
                object.insert("role".to_owned(), Value::String(role.to_string()));
                object.insert(
                    "account".to_owned(),
                    Value::String(account_literal(account)),
                );
            }
            InstructionDraft::MultisigPropose {
                account,
                instructions,
                transaction_ttl_ms,
            } => {
                object.insert(
                    "kind".to_owned(),
                    Value::String("multisig_propose".to_owned()),
                );
                object.insert(
                    "account".to_owned(),
                    Value::String(account_literal(account)),
                );
                object.insert(
                    "instructions".to_owned(),
                    drafts_to_json_value(instructions),
                );
                if let Some(ttl) = transaction_ttl_ms {
                    object.insert("transaction_ttl_ms".to_owned(), Value::from(ttl.get()));
                }
            }
        }
        Value::Object(object)
    }

    /// Construct a draft by parsing a JSON value emitted by [`Self::to_json_value`].
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError::InvalidRawDraft`] if the payload is malformed.
    pub fn from_json_value(value: &Value) -> Result<Self, ComposeError> {
        let Value::Object(map) = value else {
            return Err(ComposeError::InvalidRawDraft {
                reason: "expected object".to_owned(),
            });
        };

        let kind = extract_string(map, "kind")?;
        match kind.as_str() {
            "mint_asset" => {
                let asset = extract_string(map, "asset")?;
                let quantity = extract_string(map, "quantity")?;
                InstructionDraft::mint_from_input(&asset, &quantity)
            }
            "burn_asset" => {
                let asset = extract_string(map, "asset")?;
                let quantity = extract_string(map, "quantity")?;
                InstructionDraft::burn_from_input(&asset, &quantity)
            }
            "transfer_asset" => {
                let asset = extract_string(map, "asset")?;
                let quantity = extract_string(map, "quantity")?;
                let destination = extract_string(map, "destination")?;
                InstructionDraft::transfer_from_input(&asset, &quantity, &destination)
            }
            "register_domain" => {
                let domain = extract_string(map, "domain")?;
                InstructionDraft::register_domain_from_input(&domain)
            }
            "register_account" => {
                let account = extract_string(map, "account")?;
                InstructionDraft::register_account_from_input(&account)
            }
            "register_asset_definition" => {
                let definition = extract_string(map, "definition")?;
                let mintable = match map.get("mintable") {
                    Some(value) => {
                        let label = match value {
                            Value::String(label) => label.as_str(),
                            _ => {
                                return Err(ComposeError::InvalidRawDraft {
                                    reason: "mintable must be a string".to_owned(),
                                });
                            }
                        };
                        parse_mintable_label(label)?
                    }
                    None => Mintable::Infinitely,
                };
                InstructionDraft::register_asset_definition_from_input(&definition, mintable)
            }
            "publish_space_directory_manifest" => {
                let Some(manifest_value) = map.get("manifest") else {
                    return Err(ComposeError::InvalidRawDraft {
                        reason: "field `manifest` is required".to_owned(),
                    });
                };
                let manifest: AssetPermissionManifest = json::from_value(manifest_value.clone())
                    .map_err(|err| ComposeError::InvalidRawDraft {
                        reason: format!("invalid space directory manifest: {err}"),
                    })?;
                Ok(InstructionDraft::PublishSpaceDirectoryManifest { manifest })
            }
            "register_pin_manifest" => {
                let Some(request_value) = map.get("request") else {
                    return Err(ComposeError::InvalidRawDraft {
                        reason: "field `request` is required".to_owned(),
                    });
                };
                let request: RegisterPinManifest = json::from_value(request_value.clone())
                    .map_err(|err| ComposeError::InvalidRawDraft {
                        reason: format!("invalid pin manifest request: {err}"),
                    })?;
                Ok(InstructionDraft::RegisterPinManifest { request })
            }
            "account_admission_policy" => {
                let domain = extract_string(map, "domain")?;
                let Some(policy_value) = map.get("policy") else {
                    return Err(ComposeError::InvalidRawDraft {
                        reason: "field `policy` is required".to_owned(),
                    });
                };
                let policy: AccountAdmissionPolicy = json::from_value(policy_value.clone())
                    .map_err(|err| ComposeError::InvalidRawDraft {
                        reason: format!("invalid account admission policy: {err}"),
                    })?;
                InstructionDraft::account_admission_policy_from_input(&domain, policy)
            }
            "grant_role" => {
                let role = extract_string(map, "role")?;
                let account = extract_string(map, "account")?;
                InstructionDraft::grant_role_from_input(&role, &account)
            }
            "revoke_role" => {
                let role = extract_string(map, "role")?;
                let account = extract_string(map, "account")?;
                InstructionDraft::revoke_role_from_input(&role, &account)
            }
            "multisig_propose" => {
                let account = extract_string(map, "account")?;
                let Some(instructions_value) = map.get("instructions") else {
                    return Err(ComposeError::InvalidRawDraft {
                        reason: "field `instructions` is required".to_owned(),
                    });
                };
                let instructions = drafts_from_json_value(instructions_value)?;
                if instructions.is_empty() {
                    return Err(ComposeError::InvalidRawDraft {
                        reason: "multisig proposal instructions must not be empty".to_owned(),
                    });
                }
                let transaction_ttl_ms = extract_optional_nonzero_u64(map, "transaction_ttl_ms")?;
                Ok(InstructionDraft::MultisigPropose {
                    account: parse_account_id(&account)?,
                    instructions,
                    transaction_ttl_ms,
                })
            }
            other => Err(ComposeError::InvalidRawDraft {
                reason: format!("unknown instruction kind `{other}`"),
            }),
        }
    }
}

fn format_admission_policy_summary(policy: &AccountAdmissionPolicy) -> String {
    let mode = match policy.mode {
        AccountAdmissionMode::ImplicitReceive => "implicit receive",
        AccountAdmissionMode::ExplicitOnly => "explicit only",
    };
    let mut details = Vec::new();
    if let Some(max) = policy.max_implicit_creations_per_tx {
        details.push(format!("tx cap {max}"));
    }
    if let Some(max) = policy.max_implicit_creations_per_block {
        details.push(format!("block cap {max}"));
    }
    if let Some(fee) = policy.implicit_creation_fee.as_ref() {
        let destination = match &fee.destination {
            ImplicitAccountFeeDestination::Burn => "burn".to_owned(),
            ImplicitAccountFeeDestination::Account(account) => format!("acct {account}"),
        };
        details.push(format!(
            "fee {} {} -> {destination}",
            fee.amount, fee.asset_definition_id
        ));
    }
    if !policy.min_initial_amounts.is_empty() {
        details.push(format!("min amounts {}", policy.min_initial_amounts.len()));
    }
    if let Some(role) = policy.default_role_on_create.as_ref() {
        details.push(format!("default role {role}"));
    }
    if details.is_empty() {
        return mode.to_owned();
    }
    format!("{mode}, {}", details.join(", "))
}

fn encode_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0F) as usize] as char);
    }
    out
}

fn account_literal(account_id: &AccountId) -> String {
    account_id.to_string()
}

fn asset_literal(asset_id: &AssetId) -> String {
    asset_id.to_string()
}

fn parse_asset_id(value: &str) -> Result<AssetId, ComposeError> {
    AssetId::from_str(value).map_err(|err| ComposeError::InvalidAssetId {
        asset: value.to_owned(),
        reason: err.reason().into(),
    })
}

fn parse_account_id(value: &str) -> Result<AccountId, ComposeError> {
    AccountId::parse_encoded(value)
        .map(|parsed| parsed.into_account_id())
        .map_err(|err| ComposeError::InvalidAccountId {
            account: value.to_owned(),
            reason: err.to_string(),
        })
}

fn parse_scoped_account_id(value: &str) -> Result<ScopedAccountId, ComposeError> {
    ScopedAccountId::from_str(value).map_err(|err| ComposeError::InvalidAccountId {
        account: value.to_owned(),
        reason: err.to_string(),
    })
}

fn parse_quantity(value: &str) -> Result<Numeric, ComposeError> {
    Numeric::from_str(value).map_err(|err: NumericError| ComposeError::InvalidQuantity {
        quantity: value.to_owned(),
        reason: err.to_string(),
    })
}

fn parse_domain_id(value: &str) -> Result<DomainId, ComposeError> {
    DomainId::from_str(value).map_err(|err| ComposeError::InvalidDomainId {
        domain: value.to_owned(),
        reason: err.to_string(),
    })
}

fn parse_asset_definition_id(value: &str) -> Result<AssetDefinitionId, ComposeError> {
    AssetDefinitionId::from_str(value).map_err(|err| ComposeError::InvalidAssetDefinitionId {
        definition: value.to_owned(),
        reason: err.to_string(),
    })
}

fn parse_role_id(value: &str) -> Result<RoleId, ComposeError> {
    RoleId::from_str(value).map_err(|err| ComposeError::InvalidRoleId {
        role: value.to_owned(),
        reason: err.to_string(),
    })
}

fn extract_string(map: &Map, key: &str) -> Result<String, ComposeError> {
    match map.get(key) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(ComposeError::InvalidRawDraft {
            reason: format!("field `{key}` must be a string"),
        }),
        None => Err(ComposeError::InvalidRawDraft {
            reason: format!("field `{key}` is required"),
        }),
    }
}

fn extract_optional_nonzero_u64(map: &Map, key: &str) -> Result<Option<NonZeroU64>, ComposeError> {
    let Some(value) = map.get(key) else {
        return Ok(None);
    };
    match value {
        Value::Null => Ok(None),
        Value::Number(number) => {
            let Some(parsed) = number.as_u64() else {
                return Err(ComposeError::InvalidRawDraft {
                    reason: format!("field `{key}` must be an unsigned integer"),
                });
            };
            NonZeroU64::new(parsed)
                .ok_or_else(|| ComposeError::InvalidRawDraft {
                    reason: format!("field `{key}` must be greater than zero"),
                })
                .map(Some)
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let parsed = trimmed
                .parse::<u64>()
                .map_err(|_| ComposeError::InvalidRawDraft {
                    reason: format!("field `{key}` must be an unsigned integer"),
                })?;
            NonZeroU64::new(parsed)
                .ok_or_else(|| ComposeError::InvalidRawDraft {
                    reason: format!("field `{key}` must be greater than zero"),
                })
                .map(Some)
        }
        _ => Err(ComposeError::InvalidRawDraft {
            reason: format!("field `{key}` must be an unsigned integer"),
        }),
    }
}

fn format_mintable(mintable: Mintable) -> String {
    match mintable {
        Mintable::Infinitely => "Infinitely",
        Mintable::Once => "Once",
        Mintable::Not => "Not",
        Mintable::Limited(tokens) => return format!("Limited({})", tokens.value()),
    }
    .to_owned()
}

fn parse_mintable_label(label: &str) -> Result<Mintable, ComposeError> {
    let trimmed = label.trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "infinitely" | "infinite" => Ok(Mintable::Infinitely),
        "once" => Ok(Mintable::Once),
        "not" => Ok(Mintable::Not),
        other if other.starts_with("limited") => {
            let suffix = trimmed["limited".len()..].trim();
            let value_str = if suffix.starts_with('(') && suffix.ends_with(')') {
                &suffix[1..suffix.len() - 1]
            } else if suffix.is_empty() {
                return Err(ComposeError::InvalidRawDraft {
                    reason: "Limited mintable requires a positive token count".to_owned(),
                });
            } else {
                suffix
            };
            let tokens =
                value_str
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| ComposeError::InvalidRawDraft {
                        reason: format!("invalid Limited token count `{value_str}`"),
                    })?;
            Mintable::limited_from_u32(tokens).map_err(|err| ComposeError::InvalidRawDraft {
                reason: format!("{err}"),
            })
        }
        other => Err(ComposeError::InvalidRawDraft {
            reason: format!("unknown mintable variant `{other}`"),
        }),
    }
}

fn apply_mintable(builder: NewAssetDefinition, mintable: Mintable) -> NewAssetDefinition {
    match mintable {
        Mintable::Infinitely => builder,
        Mintable::Once => builder.mintable_once(),
        Mintable::Limited(tokens) => builder.mintable_limited(tokens),
        other => builder.with_mintable(other),
    }
}

/// Serialize a list of drafts to a JSON array representation.
#[must_use]
pub fn drafts_to_json_value(drafts: &[InstructionDraft]) -> Value {
    Value::Array(drafts.iter().map(|draft| draft.to_json_value()).collect())
}

/// Serialize a list of drafts into a formatted JSON string.
///
/// # Errors
///
/// Returns [`ComposeError::InvalidRawDraft`] when serialization fails.
pub fn drafts_to_pretty_json(drafts: &[InstructionDraft]) -> Result<String, ComposeError> {
    json::to_string_pretty(&drafts_to_json_value(drafts)).map_err(|err| {
        ComposeError::InvalidRawDraft {
            reason: err.to_string(),
        }
    })
}

/// Decode a list of drafts from a JSON array representation.
///
/// # Errors
///
/// Returns [`ComposeError::InvalidRawDraft`] when the payload is malformed.
pub fn drafts_from_json_value(value: &Value) -> Result<Vec<InstructionDraft>, ComposeError> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(InstructionDraft::from_json_value)
            .collect(),
        _ => Err(ComposeError::InvalidRawDraft {
            reason: "expected array".to_owned(),
        }),
    }
}

/// Parse drafts from a JSON string.
///
/// # Errors
///
/// Returns [`ComposeError::InvalidRawDraft`] when the payload fails to decode.
pub fn drafts_from_json_str(input: &str) -> Result<Vec<InstructionDraft>, ComposeError> {
    let value: Value = json::from_str(input).map_err(|err| ComposeError::InvalidRawDraft {
        reason: err.to_string(),
    })?;
    drafts_from_json_value(&value)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, num::NonZeroU32, time::Duration};

    use iroha_data_model::{
        account::{AccountAdmissionMode, admission::ImplicitAccountCreationFee},
        asset::prelude::AssetDefinitionId,
    };
    use iroha_version::Version;

    use super::*;

    const FIXTURE_ADMISSION_POLICY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/composer/draft_account_admission_policy.json"
    ));
    const FIXTURE_MULTISIG_PROPOSE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/composer/draft_multisig_propose.json"
    ));
    const FIXTURE_SPACE_MANIFEST_TOUCH: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/composer/draft_space_manifest_touch.json"
    ));
    const FIXTURE_SPACE_MANIFEST_HANDLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/composer/draft_space_manifest_handle.json"
    ));
    const FIXTURE_PIN_MANIFEST: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/composer/draft_pin_manifest.json"
    ));

    fn draft_from_fixture(fixture: &str) -> InstructionDraft {
        let value: Value = json::from_str(fixture).expect("fixture json");
        InstructionDraft::from_json_value(&value).expect("fixture draft")
    }

    #[test]
    fn mint_preview_produces_summary() {
        let account = account_literal(&ALICE_ID);
        let asset_def: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("definition id");
        let asset_id = format!("{asset_def}#{account}");

        let preview = mint_numeric_preview("mochi-local", &asset_id, 5_u32).expect("preview");

        assert_eq!(preview.authority(), account);
        assert!(
            !preview.instructions().is_empty(),
            "expected human readable instruction summary"
        );
        assert!(
            !preview.encoded_hex().is_empty(),
            "hex encoding must not be empty"
        );
        assert!(!preview.hash().is_empty(), "hash must not be empty");

        // Ensure Norito payload roundtrips back to a transaction.
        assert!(
            SignedTransaction::supported_versions().contains(&preview.encoded_bytes()[0]),
            "unexpected signed transaction version"
        );
    }

    #[test]
    fn invalid_asset_id_reports_error() {
        let err = mint_numeric_preview("chain", "invalid-format", 1_u32)
            .expect_err("invalid identifiers should produce compose error");
        matches!(err, ComposeError::InvalidAssetId { .. });
    }

    #[test]
    fn transfer_draft_parses_inputs() {
        let asset_id = AssetId::new(
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM".parse().unwrap(),
            ALICE_ID.clone(),
        );
        let asset_str = asset_literal(&asset_id);
        let destination = account_literal(&BOB_ID);
        let draft =
            InstructionDraft::transfer_from_input(&asset_str, "10.5", &destination).expect("draft");
        matches!(draft, InstructionDraft::TransferAsset { .. });
        let summary = draft.summary();
        assert!(
            summary.contains("Transfer"),
            "summary should mention transfer action"
        );
    }

    #[test]
    fn burn_draft_parses_inputs() {
        let asset_id = AssetId::new(
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM".parse().unwrap(),
            ALICE_ID.clone(),
        );
        let asset_str = asset_literal(&asset_id);
        let draft =
            InstructionDraft::burn_from_input(&asset_str, "3").expect("burn draft should parse");
        matches!(draft, InstructionDraft::BurnAsset { .. });
        assert!(
            draft.summary().contains("Burn"),
            "summary should mention burn action"
        );
    }

    #[test]
    fn compose_preview_requires_instructions() {
        let err = compose_preview("chain", &[]).expect_err("empty instructions should fail");
        matches!(err, ComposeError::EmptyInstructions);
    }

    #[test]
    fn compose_preview_accepts_multiple_instructions() {
        let asset_def: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("definition id");
        let asset_id = AssetId::new(asset_def, ALICE_ID.clone());
        let mint = InstructionDraft::MintAsset {
            asset: asset_id.clone(),
            quantity: Numeric::from(5_u32),
        };
        let transfer = InstructionDraft::TransferAsset {
            asset: asset_id,
            quantity: Numeric::from(3_u32),
            destination: BOB_ID.clone(),
        };
        let preview =
            compose_preview("chain", &[mint, transfer]).expect("compose multi instruction");
        assert_eq!(
            preview.instructions().len(),
            2,
            "preview should include both instructions"
        );
    }

    #[test]
    fn development_signing_authorities_present() {
        let authorities = development_signing_authorities();
        assert!(
            !authorities.is_empty(),
            "expected at least one development signing authority"
        );
        assert!(
            authorities
                .iter()
                .any(|authority| authority.account_id() == &ALICE_ID.clone()),
            "expected Alice development signer to be available"
        );
    }

    #[test]
    fn register_domain_from_input_validates_identifier() {
        let err = InstructionDraft::register_domain_from_input("invalid domain")
            .expect_err("invalid domain id should error");
        match err {
            ComposeError::InvalidDomainId { domain, .. } => {
                assert_eq!(domain, "invalid domain");
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn drafts_json_roundtrip_covers_all_variants() {
        let asset_def: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("definition id");
        let asset_id = AssetId::new(asset_def.clone(), ALICE_ID.clone());
        let asset_id_str = asset_literal(&asset_id);
        let account = account_literal(&ALICE_ID);
        let scoped_account = format!("{account}@wonderland");

        let mint = InstructionDraft::mint_from_input(&asset_id_str, "10").expect("mint draft");
        let burn = InstructionDraft::burn_from_input(&asset_id_str, "1").expect("burn draft");
        let transfer = InstructionDraft::transfer_from_input(&asset_id_str, "5", &account)
            .expect("transfer draft");
        let register_domain =
            InstructionDraft::register_domain_from_input("wonderland").expect("domain draft");
        let register_account =
            InstructionDraft::register_account_from_input(&scoped_account).expect("account draft");
        let register_definition = InstructionDraft::register_asset_definition_from_input(
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            Mintable::Once,
        )
        .expect("definition draft");
        let space_manifest = draft_from_fixture(FIXTURE_SPACE_MANIFEST_HANDLE);
        let pin_manifest = draft_from_fixture(FIXTURE_PIN_MANIFEST);
        let policy = AccountAdmissionPolicy {
            mode: AccountAdmissionMode::ImplicitReceive,
            max_implicit_creations_per_tx: Some(3),
            max_implicit_creations_per_block: None,
            implicit_creation_fee: None,
            min_initial_amounts: BTreeMap::new(),
            default_role_on_create: None,
        };
        let admission = InstructionDraft::account_admission_policy_from_input("wonderland", policy)
            .expect("policy draft");

        let grant_role =
            InstructionDraft::grant_role_from_input("council", &account).expect("grant draft");
        let revoke_role =
            InstructionDraft::revoke_role_from_input("council", &account).expect("revoke draft");
        let nested_json =
            drafts_to_pretty_json(&[mint.clone(), transfer.clone()]).expect("nested json");
        let multisig =
            InstructionDraft::multisig_propose_from_json(&account, &nested_json, Some(60_000))
                .expect("multisig draft");

        let drafts = vec![
            mint,
            burn,
            transfer,
            register_domain,
            register_account,
            register_definition,
            space_manifest,
            pin_manifest,
            admission,
            grant_role,
            revoke_role,
            multisig,
        ];

        let json = drafts_to_pretty_json(&drafts).expect("serialize drafts");
        let parsed = drafts_from_json_str(&json).expect("parse drafts");

        assert_eq!(parsed.len(), drafts.len());
        for (expected, actual) in drafts.iter().zip(parsed.iter()) {
            assert_eq!(expected.summary(), actual.summary());
        }
    }

    #[test]
    fn register_account_instruction_preserves_scoped_domain() {
        let account = account_literal(&ALICE_ID);
        let scoped_account = format!("{account}@wonderland");
        let expected: ScopedAccountId = scoped_account.parse().expect("scoped account");
        let draft =
            InstructionDraft::register_account_from_input(&scoped_account).expect("account draft");

        let instruction = draft.instruction();
        let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*instruction;
        let Some(register_box) = instruction_ref
            .as_any()
            .downcast_ref::<iroha_data_model::isi::RegisterBox>()
        else {
            panic!("expected RegisterBox instruction");
        };
        let iroha_data_model::isi::RegisterBox::Account(register) = register_box else {
            panic!("expected Register<Account> instruction");
        };

        assert_eq!(register.object.id, ALICE_ID.clone());
        assert_eq!(register.object.linked_domains().len(), 1);
        assert!(register.object.linked_domains().contains(expected.domain()));
    }

    #[test]
    fn drafts_from_json_str_rejects_non_array() {
        let err = drafts_from_json_str("{\"kind\":\"mint_asset\"}")
            .expect_err("non-array payload should be rejected");
        assert!(matches!(err, ComposeError::InvalidRawDraft { .. }));
    }

    #[test]
    fn grant_role_draft_parses_inputs() {
        let account = account_literal(&ALICE_ID);
        let draft =
            InstructionDraft::grant_role_from_input("council", &account).expect("grant draft");
        let expected_account = ALICE_ID.to_string();
        assert_eq!(
            draft.summary(),
            format!("Grant role council to {expected_account}")
        );
    }

    #[test]
    fn revoke_role_draft_parses_inputs() {
        let account = account_literal(&ALICE_ID);
        let draft =
            InstructionDraft::revoke_role_from_input("auditor", &account).expect("revoke draft");
        let expected_account = ALICE_ID.to_string();
        assert_eq!(
            draft.summary(),
            format!("Revoke role auditor from {expected_account}")
        );
    }

    #[test]
    fn development_signer_permissions_match_expectations() {
        use std::collections::BTreeSet;

        let authorities = development_signing_authorities();
        let alice = authorities
            .iter()
            .find(|auth| auth.label() == "Alice (dev)")
            .expect("Alice signer present");
        let bob = authorities
            .iter()
            .find(|auth| auth.label() == "Bob (dev)")
            .expect("Bob signer present");

        let alice_permissions: BTreeSet<_> = alice.permissions().collect();
        assert_eq!(
            alice_permissions,
            InstructionPermission::all().into_iter().collect(),
            "Alice should support every instruction template"
        );

        let bob_permissions: Vec<_> = bob.permissions().collect();
        assert_eq!(
            bob_permissions,
            vec![InstructionPermission::TransferAsset],
            "Bob should default to transfer-only permissions"
        );
    }

    #[test]
    fn account_admission_policy_draft_parses_domain() {
        let policy = AccountAdmissionPolicy::default();
        let draft = InstructionDraft::account_admission_policy_from_input("wonderland", policy)
            .expect("policy draft");
        assert!(
            draft.summary().contains("wonderland"),
            "summary should mention domain"
        );
    }

    #[test]
    fn multisig_propose_from_json_requires_instructions() {
        let account = account_literal(&ALICE_ID);
        let err = InstructionDraft::multisig_propose_from_json(&account, "[]", None)
            .expect_err("empty proposal should be rejected");
        assert!(matches!(err, ComposeError::InvalidRawDraft { .. }));
    }

    #[test]
    fn space_directory_manifest_from_json_rejects_invalid_payload() {
        let err = InstructionDraft::publish_space_directory_manifest_from_json("{\"bad\":true}")
            .expect_err("invalid manifest should be rejected");
        assert!(matches!(err, ComposeError::InvalidRawDraft { .. }));
    }

    #[test]
    fn pin_manifest_from_json_rejects_invalid_payload() {
        let err = InstructionDraft::register_pin_manifest_from_json("{\"digest\":\"bad\"}")
            .expect_err("invalid pin manifest should be rejected");
        assert!(matches!(err, ComposeError::InvalidRawDraft { .. }));
    }

    #[test]
    fn signing_authority_roles_roundtrip() {
        let role: RoleId = "basic_user".parse().expect("role id");
        let authority = SigningAuthority::with_permissions_and_roles(
            "Carol",
            ALICE_ID.clone(),
            ALICE_KEYPAIR.clone(),
            [InstructionPermission::TransferAsset],
            [role.clone()],
        );
        let roles: Vec<_> = authority.roles().collect();
        assert_eq!(roles, vec![&role]);
    }

    #[test]
    fn instruction_permission_keys_roundtrip() {
        for permission in InstructionPermission::all() {
            let key = permission.key();
            let restored = InstructionPermission::from_key(key)
                .unwrap_or_else(|| panic!("expected key {key} to map back to a permission"));
            assert_eq!(
                restored, permission,
                "roundtrip between key `{key}` and permission should match"
            );
        }
        assert!(
            InstructionPermission::from_key("unknown").is_none(),
            "unknown permission keys should be rejected"
        );
    }

    #[test]
    fn draft_fixtures_parse_to_expected_variants() {
        let cases = [
            (FIXTURE_ADMISSION_POLICY, "account admission policy"),
            (FIXTURE_MULTISIG_PROPOSE, "multisig proposal"),
            (FIXTURE_SPACE_MANIFEST_TOUCH, "space directory touch"),
            (FIXTURE_SPACE_MANIFEST_HANDLE, "space directory handle"),
            (FIXTURE_PIN_MANIFEST, "pin manifest"),
        ];

        for (fixture, label) in cases {
            let draft = draft_from_fixture(fixture);
            match (label, draft) {
                (
                    "account admission policy",
                    InstructionDraft::SetAccountAdmissionPolicy { .. },
                ) => {}
                ("multisig proposal", InstructionDraft::MultisigPropose { .. }) => {}
                (
                    "space directory touch",
                    InstructionDraft::PublishSpaceDirectoryManifest { .. },
                ) => {}
                (
                    "space directory handle",
                    InstructionDraft::PublishSpaceDirectoryManifest { .. },
                ) => {}
                ("pin manifest", InstructionDraft::RegisterPinManifest { .. }) => {}
                _ => panic!("fixture {label} did not decode to expected draft"),
            }
        }
    }

    #[test]
    fn admission_policy_summary_surfaces_caps_fee_and_role() {
        let policy = AccountAdmissionPolicy {
            mode: AccountAdmissionMode::ImplicitReceive,
            max_implicit_creations_per_tx: Some(3),
            max_implicit_creations_per_block: Some(10),
            implicit_creation_fee: Some(ImplicitAccountCreationFee {
                asset_definition_id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
                    .parse()
                    .expect("asset definition"),
                amount: Numeric::from(1_u32),
                destination: ImplicitAccountFeeDestination::Burn,
            }),
            min_initial_amounts: BTreeMap::new(),
            default_role_on_create: Some("basic_user".parse().expect("role id")),
        };
        let draft = InstructionDraft::SetAccountAdmissionPolicy {
            domain: "wonderland".parse().expect("domain"),
            policy,
        };
        let summary = draft.summary();
        assert!(summary.contains("tx cap 3"));
        assert!(summary.contains("block cap 10"));
        assert!(summary.contains("fee 1 62Fk4FPcMuLvW5QjDGNF2a4jAmjM"));
        assert!(summary.contains("default role basic_user"));
    }

    #[test]
    fn compose_preview_rejects_unauthorised_instruction() {
        let authorities = development_signing_authorities();
        let bob = authorities
            .iter()
            .find(|auth| auth.label() == "Bob (dev)")
            .expect("Bob signer present");

        let draft =
            InstructionDraft::register_domain_from_input("side_garden").expect("domain draft");
        let err = compose_preview_with_authority("chain", &[draft], bob)
            .expect_err("Bob should not be allowed to register domains");

        match err {
            ComposeError::UnauthorizedInstruction { action, .. } => {
                assert_eq!(action, InstructionPermission::RegisterDomain);
            }
            other => panic!("unexpected compose error: {other:?}"),
        }
    }

    #[test]
    fn compose_preview_rejects_unauthorised_multisig_proposal() {
        let authorities = development_signing_authorities();
        let bob = authorities
            .iter()
            .find(|auth| auth.label() == "Bob (dev)")
            .expect("Bob signer present");

        let asset_def: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("definition id");
        let asset_id = AssetId::new(asset_def, ALICE_ID.clone());
        let asset_id_str = asset_literal(&asset_id);
        let mint = InstructionDraft::mint_from_input(&asset_id_str, "1").expect("mint draft");
        let nested_json = drafts_to_pretty_json(&[mint]).expect("nested json");
        let draft = InstructionDraft::multisig_propose_from_json(
            &account_literal(&ALICE_ID),
            &nested_json,
            None,
        )
        .expect("multisig draft");

        let err = compose_preview_with_authority("chain", &[draft], bob)
            .expect_err("Bob should not be allowed to propose multisig transactions");

        match err {
            ComposeError::UnauthorizedInstruction { action, .. } => {
                assert_eq!(action, InstructionPermission::MultisigPropose);
            }
            other => panic!("unexpected compose error: {other:?}"),
        }
    }

    #[test]
    fn compose_preview_rejects_unauthorised_space_manifest() {
        let authorities = development_signing_authorities();
        let bob = authorities
            .iter()
            .find(|auth| auth.label() == "Bob (dev)")
            .expect("Bob signer present");
        let draft = draft_from_fixture(FIXTURE_SPACE_MANIFEST_TOUCH);

        let err = compose_preview_with_authority("chain", &[draft], bob)
            .expect_err("Bob should not be allowed to publish space directory manifests");

        match err {
            ComposeError::UnauthorizedInstruction { action, .. } => {
                assert_eq!(action, InstructionPermission::PublishSpaceDirectoryManifest);
            }
            other => panic!("unexpected compose error: {other:?}"),
        }
    }

    #[test]
    fn compose_preview_rejects_unauthorised_pin_manifest_json() {
        let authorities = development_signing_authorities();
        let bob = authorities
            .iter()
            .find(|auth| auth.label() == "Bob (dev)")
            .expect("Bob signer present");
        let draft = draft_from_fixture(FIXTURE_PIN_MANIFEST);

        let err = compose_preview_with_authority("chain", &[draft], bob)
            .expect_err("Bob should not be allowed to register pin manifests");

        match err {
            ComposeError::UnauthorizedInstruction { action, .. } => {
                assert_eq!(action, InstructionPermission::RegisterPinManifest);
            }
            other => panic!("unexpected compose error: {other:?}"),
        }
    }

    #[test]
    fn compose_preview_rejects_unauthorised_space_directory_manifest() {
        let authorities = development_signing_authorities();
        let bob = authorities
            .iter()
            .find(|auth| auth.label() == "Bob (dev)")
            .expect("Bob signer present");
        let manifest_json = r#"{
  "version": 1,
  "uaid": "uaid:62af083a61d42ca6c269585b9a33c563f4e0a5c0726614f7a98b6aab6b98f70b",
  "dataspace": 12,
  "issued_ms": 1710000000000,
  "activation_epoch": 500,
  "expiry_epoch": 550,
  "entries": []
}"#;
        let draft = InstructionDraft::publish_space_directory_manifest_from_json(manifest_json)
            .expect("space directory draft");

        let err = compose_preview_with_authority("chain", &[draft], bob)
            .expect_err("Bob should not be allowed to publish space directory manifests");

        match err {
            ComposeError::UnauthorizedInstruction { action, .. } => {
                assert_eq!(action, InstructionPermission::PublishSpaceDirectoryManifest);
            }
            other => panic!("unexpected compose error: {other:?}"),
        }
    }

    #[test]
    fn compose_preview_rejects_unauthorised_pin_manifest() {
        let authorities = development_signing_authorities();
        let bob = authorities
            .iter()
            .find(|auth| auth.label() == "Bob (dev)")
            .expect("Bob signer present");
        let pin_manifest_json = r#"{
  "digest": [[9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9]],
  "chunker": {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "multihash_code": 31
  },
  "chunk_digest_sha3_256": [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7],
  "policy": {
    "min_replicas": 2,
    "storage_class": {
      "type": "Hot",
      "value": null
    },
    "retention_epoch": 900
  },
  "submitted_epoch": 42,
  "alias": null,
  "successor_of": null
}"#;
        let draft = InstructionDraft::register_pin_manifest_from_json(pin_manifest_json)
            .expect("pin manifest draft");

        let err = compose_preview_with_authority("chain", &[draft], bob)
            .expect_err("Bob should not be allowed to register pin manifests");

        match err {
            ComposeError::UnauthorizedInstruction { action, .. } => {
                assert_eq!(action, InstructionPermission::RegisterPinManifest);
            }
            other => panic!("unexpected compose error: {other:?}"),
        }
    }

    #[test]
    fn compose_preview_with_options_applies_overrides() {
        let authorities = development_signing_authorities();
        let signer = authorities
            .iter()
            .find(|auth| auth.allows_permission(InstructionPermission::MintAsset))
            .expect("signer with mint permission present");
        let account = account_literal(signer.account_id());
        let asset_def: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("definition id");
        let asset_id = format!("{asset_def}#{account}");
        let draft = InstructionDraft::mint_from_input(&asset_id, "5").expect("mint draft");

        let options = TransactionComposeOptions::new()
            .with_ttl(Duration::from_secs(5))
            .with_creation_time(Duration::from_millis(1_234))
            .with_nonce(NonZeroU32::new(42).expect("non-zero nonce"));

        let preview = compose_preview_with_options("chain", &[draft], signer, &options)
            .expect("compose with overrides");

        assert_eq!(
            preview.time_to_live(),
            Some(Duration::from_secs(5)),
            "expected TTL override to propagate into preview"
        );
        assert_eq!(
            preview.creation_time(),
            Duration::from_millis(1_234),
            "expected creation time override to propagate"
        );
        assert_eq!(
            preview.nonce(),
            NonZeroU32::new(42),
            "nonce override should propagate"
        );
    }
}
