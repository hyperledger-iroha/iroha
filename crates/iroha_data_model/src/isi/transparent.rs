use core::num::NonZeroU16;
use std::fmt::Display;

use iroha_crypto::PublicKey;
use iroha_primitives::json::Json;
#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize};

use super::*;
use crate::asset::id::AssetId;

iroha_data_model_derive::model_single! {
    /// Generic instruction for setting a chain-wide config parameter.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Constructor)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[display("SET `{_0}`")]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[repr(transparent)]
    pub struct SetParameter(pub Parameter);
}

isi! {
    /// Generic instruction to set key value at the object.
    pub struct SetKeyValue<O: Identifiable> {
        /// Where to set key value.
        pub object: O::Id,
        /// Key.
        pub key: Name,
        /// Value.
        pub value: Json,
    }
}

impl SetKeyValue<Domain> {
    /// Constructs a new [`SetKeyValue`] for a [`Domain`] with the given `key` and `value`.
    pub fn domain(domain_id: DomainId, key: Name, value: impl Into<Json>) -> Self {
        Self {
            object: domain_id,
            key,
            value: value.into(),
        }
    }
}

impl SetKeyValue<Account> {
    /// Constructs a new [`SetKeyValue`] for an [`Account`] with the given `key` and `value`.
    pub fn account(account_id: AccountId, key: Name, value: impl Into<Json>) -> Self {
        Self {
            object: account_id,
            key,
            value: value.into(),
        }
    }
}

impl SetKeyValue<AssetDefinition> {
    /// Constructs a new [`SetKeyValue`] for an [`AssetDefinition`] with the given `key` and `value`.
    pub fn asset_definition(
        asset_definition_id: AssetDefinitionId,
        key: Name,
        value: impl Into<Json>,
    ) -> Self {
        Self {
            object: asset_definition_id,
            key,
            value: value.into(),
        }
    }
}

impl SetKeyValue<Nft> {
    /// Constructs a new [`SetKeyValue`] for an [`Nft`] with the given `key` and `value`.
    pub fn nft(nft_id: NftId, key: Name, value: impl Into<Json>) -> Self {
        Self {
            object: nft_id,
            key,
            value: value.into(),
        }
    }
}

impl SetKeyValue<Trigger> {
    /// Constructs a new [`SetKeyValue`] for a [`Trigger`] with the given `key` and `value`.
    pub fn trigger(trigger_id: TriggerId, key: Name, value: impl Into<Json>) -> Self {
        Self {
            object: trigger_id,
            key,
            value: value.into(),
        }
    }
}

isi! {
    /// Set metadata on a concrete asset balance (`AssetId`).
    pub struct SetAssetKeyValue {
        /// Asset to edit.
        pub asset: AssetId,
        /// Metadata key.
        pub key: Name,
        /// Metadata value stored as JSON.
        pub value: Json,
    }
}

impl SetAssetKeyValue {
    /// Convenience constructor for asset metadata edits.
    pub fn new(asset: AssetId, key: Name, value: impl Into<Json>) -> Self {
        Self {
            asset,
            key,
            value: value.into(),
        }
    }
}

impl_display! {
    SetAssetKeyValue => "SET `{}` = `{}` ON ASSET `{}`", key, value, asset
}

impl_display! {
    SetKeyValue<O>
    where
        O: Identifiable,
        O::Id: Display,
    =>
    "SET `{}` = `{}` IN `{}`",
    key, value, object,
}

impl_into_box! {
    SetKeyValue<Domain> |
    SetKeyValue<Account> |
    SetKeyValue<AssetDefinition> |
    SetKeyValue<Nft> |
    SetKeyValue<Trigger>
=> SetKeyValueBox
}

isi! {
    /// Generic instruction to remove key value at the object.
    pub struct RemoveKeyValue<O: Identifiable> {
        /// From where to remove key value.
        pub object: O::Id,
        /// Key of the pair to remove.
        pub key: Name,
    }
}

impl RemoveKeyValue<Domain> {
    /// Constructs a new [`RemoveKeyValue`] for a [`Domain`] with the given `key`.
    pub fn domain(domain_id: DomainId, key: Name) -> Self {
        Self {
            object: domain_id,
            key,
        }
    }
}

impl RemoveKeyValue<Account> {
    /// Constructs a new [`RemoveKeyValue`] for an [`Account`] with the given `key`.
    pub fn account(account_id: AccountId, key: Name) -> Self {
        Self {
            object: account_id,
            key,
        }
    }
}

impl RemoveKeyValue<AssetDefinition> {
    /// Constructs a new [`RemoveKeyValue`] for an [`AssetDefinition`] with the given `key`.
    pub fn asset_definition(asset_definition_id: AssetDefinitionId, key: Name) -> Self {
        Self {
            object: asset_definition_id,
            key,
        }
    }
}

impl RemoveKeyValue<Nft> {
    /// Constructs a new [`RemoveKeyValue`] for an [`Nft`] with the given `key`.
    pub fn nft(nft_id: NftId, key: Name) -> Self {
        Self {
            object: nft_id,
            key,
        }
    }
}

impl RemoveKeyValue<Trigger> {
    /// Constructs a new [`RemoveKeyValue`] for a [`Trigger`] with the given `key`.
    pub fn trigger(trigger_id: TriggerId, key: Name) -> Self {
        Self {
            object: trigger_id,
            key,
        }
    }
}

isi! {
    /// Remove a metadata key from a concrete asset balance (`AssetId`).
    pub struct RemoveAssetKeyValue {
        /// Asset to edit.
        pub asset: AssetId,
        /// Metadata key to remove.
        pub key: Name,
    }
}

impl RemoveAssetKeyValue {
    /// Convenience constructor for removing asset metadata.
    pub fn new(asset: AssetId, key: Name) -> Self {
        Self { asset, key }
    }
}

impl_display! {
    RemoveAssetKeyValue => "REMOVE `{}` FROM ASSET `{}`", key, asset
}

impl_display! {
    RemoveKeyValue<O>
    where
        O: Identifiable,
        O::Id: Display,
    =>
    "REMOVE `{}` from `{}`",
    key, object,
}

impl_into_box! {
    RemoveKeyValue<Domain> |
    RemoveKeyValue<Account> |
    RemoveKeyValue<AssetDefinition> |
    RemoveKeyValue<Nft> |
    RemoveKeyValue<Trigger>
=> RemoveKeyValueBox
}

isi! {
    /// Add a signatory to an account's multisig specification.
    pub struct AddSignatory {
        /// Account whose multisig spec is updated.
        pub account: AccountId,
        /// Public key to add as a signatory (weight defaults to 1).
        pub signatory: PublicKey,
    }
}

impl AddSignatory {
    /// Construct a signatory-add instruction.
    pub fn new(account: AccountId, signatory: PublicKey) -> Self {
        Self { account, signatory }
    }
}

isi! {
    /// Remove a signatory from an account's multisig specification.
    pub struct RemoveSignatory {
        /// Account whose multisig spec is updated.
        pub account: AccountId,
        /// Public key to remove.
        pub signatory: PublicKey,
    }
}

impl RemoveSignatory {
    /// Construct a signatory-remove instruction.
    pub fn new(account: AccountId, signatory: PublicKey) -> Self {
        Self { account, signatory }
    }
}

isi! {
    /// Set the quorum threshold for an account's multisig specification.
    pub struct SetAccountQuorum {
        /// Account whose multisig spec is updated.
        pub account: AccountId,
        /// Required approval weight (must be non-zero).
        pub quorum: NonZeroU16,
    }
}

impl SetAccountQuorum {
    /// Construct an account quorum update.
    pub fn new(account: AccountId, quorum: NonZeroU16) -> Self {
        Self { account, quorum }
    }
}

impl_display! {
    AddSignatory => "ADD SIGNATORY `{}` TO `{}`", signatory, account
}

impl_display! {
    RemoveSignatory => "REMOVE SIGNATORY `{}` FROM `{}`", signatory, account
}

impl_display! {
    SetAccountQuorum => "SET QUORUM `{}` FOR `{}`", quorum, account
}

isi! {
    /// Generic instruction for granting permission to an entity.
    pub struct Grant<O, D: Identifiable> {
        /// Object to grant.
        pub object: O,
        /// Entity to which to grant this token.
        pub destination: D::Id,
    }
}

impl Grant<Permission, Account> {
    /// Constructs a new [`Grant`] for a [`Permission`].
    pub fn account_permission(permission: impl Into<Permission>, to: AccountId) -> Self {
        Self {
            object: permission.into(),
            destination: to,
        }
    }
}

impl Grant<RoleId, Account> {
    /// Constructs a new [`Grant`] for a [`Role`].
    pub fn account_role(role_id: RoleId, to: AccountId) -> Self {
        Self {
            object: role_id,
            destination: to,
        }
    }
}

impl Grant<Permission, Role> {
    /// Constructs a new [`Grant`] for giving a [`Permission`] to [`Role`].
    pub fn role_permission(permission: impl Into<Permission>, to: RoleId) -> Self {
        Self {
            object: permission.into(),
            destination: to,
        }
    }
}

impl_display! {
    Grant<O, D>
    where
        O: Display,
        D: Identifiable,
        D::Id: Display,
    =>
    "GRANT `{}` TO `{}`",
    object,
    destination,
}

impl_into_box! {
    Grant<Permission, Account> |
    Grant<RoleId, Account> |
    Grant<Permission, Role>
=> GrantBox
}

isi! {
    /// Generic instruction for revoking permission from an entity.
    pub struct Revoke<O, D: Identifiable> {
        /// Object to revoke.
        pub object: O,
        /// Entity which is being revoked this token from.
        pub destination: D::Id,
    }
}

impl Revoke<Permission, Account> {
    /// Constructs a new [`Revoke`] for a [`Permission`].
    pub fn account_permission(permission: impl Into<Permission>, from: AccountId) -> Self {
        Self {
            object: permission.into(),
            destination: from,
        }
    }
}

impl Revoke<RoleId, Account> {
    /// Constructs a new [`Revoke`] for a [`Role`].
    pub fn account_role(role_id: RoleId, from: AccountId) -> Self {
        Self {
            object: role_id,
            destination: from,
        }
    }
}

impl Revoke<Permission, Role> {
    /// Constructs a new [`Revoke`] for removing a [`Permission`] from [`Role`].
    pub fn role_permission(permission: impl Into<Permission>, from: RoleId) -> Self {
        Self {
            object: permission.into(),
            destination: from,
        }
    }
}

impl_display! {
    Revoke<O, D>
    where
        O: Display,
        D: Identifiable,
        D::Id: Display,
    =>
    "REVOKE `{}` FROM `{}`",
    object,
    destination,
}

impl_into_box! {
    Revoke<Permission, Account> |
    Revoke<RoleId, Account> |
    Revoke<Permission, Role>
=> RevokeBox
}

// NOTE: `BuiltInInstruction` is blanket-implemented for all `T: Instruction + Encode`
// in `isi::mod`. The following specializations duplicated that behaviour and caused
// conflicting impl errors (E0119). They are intentionally removed to rely on the
// blanket implementation.

isi! {
    /// Instruction to execute specified trigger
    #[derive(Display)]
    #[display("EXECUTE `{trigger}`")]
    pub struct ExecuteTrigger {
        /// Id of a trigger to execute
        pub trigger: TriggerId,
        /// Arguments to trigger execution
        pub args: Json,
    }
}

impl ExecuteTrigger {
    /// Constructor for [`Self`]
    pub fn new(trigger: TriggerId) -> Self {
        Self {
            trigger,
            args: Json::default(),
        }
    }

    /// Add trigger execution args
    #[must_use]
    pub fn with_args<T: norito::json::JsonSerialize + 'static>(mut self, args: T) -> Self {
        self.args = Json::new(args);
        self
    }
}

impl From<ExecuteTrigger> for super::InstructionBox {
    fn from(instruction: ExecuteTrigger) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

isi! {
    /// Generic instruction for upgrading runtime objects.
    #[derive(Constructor, Display)]
    #[display("UPGRADE")]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[repr(transparent)]
    pub struct Upgrade {
        /// Object to upgrade.
        pub executor: Executor,
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SetParameter {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SetParameter {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let parameter = <Parameter as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self(parameter))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Upgrade {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.executor, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Upgrade {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let executor = <Executor as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self { executor })
    }
}

isi! {
    /// Instruction to print logs
    #[derive(Constructor, Display)]
    #[display("LOG({level}): {msg}")]
    pub struct Log {
        /// Message log level
        pub level: Level,
        /// Msg to be logged
        pub msg: String,
    }
}

impl From<Log> for super::InstructionBox {
    fn from(instruction: Log) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

isi! {
    /// Custom executor-defined instruction envelope.
    ///
    /// `CustomInstruction` is a standardized wrapper to carry executor-specific
    /// instructions without forking the data model. It serializes via Norito and
    /// is transported on-chain like other ISI. The default executor in `iroha_core`
    /// does not execute it and will panic if encountered; production use requires
    /// a custom executor that recognizes and handles the payload deterministically
    /// on every validating peer.
    ///
    /// Intended usage
    /// - Private/consortium deployments or prototyping: implement a custom
    ///   executor that downcasts `InstructionBox` to `CustomInstruction` and
    ///   interprets its `payload` deterministically. Advertise supported custom
    ///   instruction identifiers in `ExecutorDataModel::instructions`.
    /// - Public networks: avoid; prefer Kotodama contracts for application logic
    ///   or upstream new built-in ISI for platform features. Divergent executors
    ///   will fork consensus.
    ///
    /// With Norito and IVM
    /// - Norito: `CustomInstruction` derives `Encode`/`Decode` (via the module
    ///   macro); the payload is stored as `Json` (deterministic Norito encoding).
    ///   Round-trips are stable provided the registry includes `CustomInstruction`.
    /// - IVM: Kotodama targets IVM bytecode (`.to`) and is the recommended way
    ///   to express application logic. `CustomInstruction` is for executor-level
    ///   extensions; keep execution deterministic and avoid hardware-dependent
    ///   behavior. Ensure all validators run identical executor binaries.
    ///
    /// Note: When enabling custom instructions, remember to populate
    /// [`ExecutorDataModel::instructions`] during executor migration so peers can
    /// advertise support and clients can validate payload identifiers.
    ///
    /// # Examples
    /// - See `data_model/samples/executor_custom_data_model/{simple_isi.rs,complex_isi.rs}`
    ///   for wrapping domain-specific ISI into `CustomInstruction` and dispatching
    ///   in a custom executor.
    #[derive(Display)]
    #[display("CUSTOM({payload})")]
    pub struct CustomInstruction {
        pub payload: Json,
    }
}

impl CustomInstruction {
    /// Constructor
    pub fn new(payload: impl Into<Json>) -> Self {
        Self {
            payload: payload.into(),
        }
    }
}

isi! {
    /// Placeholder instruction used when decoding an ISI payload fails.
    ///
    /// Nodes may choose to decode malformed instruction payloads into this
    /// sentinel value instead of panicking. The runtime executor must reject
    /// it deterministically.
    ///
    /// Dev note: This instruction is not intended to be submitted by clients.
    #[derive(Display)]
    #[display("INVALID_INSTRUCTION({wire_id}, {payload_hash:?}): {message}")]
    pub struct InvalidInstruction {
        /// Wire identifier of the instruction that failed to decode.
        pub wire_id: String,
        /// Hash of the raw instruction payload bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub payload_hash: [u8; 32],
        /// Human-readable decode failure message (best-effort).
        pub message: String,
    }
}

impl InvalidInstruction {
    /// Stable wire identifier for invalid instruction placeholders.
    pub const WIRE_ID: &'static str = "iroha.invalid_instruction";

    /// Construct a new invalid instruction placeholder.
    #[must_use]
    pub fn new(
        wire_id: impl Into<String>,
        payload_hash: [u8; 32],
        message: impl Into<String>,
    ) -> Self {
        Self {
            wire_id: wire_id.into(),
            payload_hash,
            message: message.into(),
        }
    }
}

// Seal implementations
impl crate::seal::Instruction for SetParameter {}
impl crate::seal::Instruction for SetKeyValueBox {}
impl crate::seal::Instruction for RemoveKeyValueBox {}
impl crate::seal::Instruction for GrantBox {}
impl crate::seal::Instruction for RevokeBox {}
impl crate::seal::Instruction for CustomInstruction {}
impl crate::seal::Instruction for InvalidInstruction {}
impl crate::seal::Instruction for SetKeyValue<Domain> {}
impl crate::seal::Instruction for SetKeyValue<AssetDefinition> {}
impl crate::seal::Instruction for SetKeyValue<Account> {}
impl crate::seal::Instruction for SetKeyValue<Nft> {}
impl crate::seal::Instruction for SetKeyValue<Trigger> {}
impl crate::seal::Instruction for SetAssetKeyValue {}
impl crate::seal::Instruction for RemoveKeyValue<Domain> {}
impl crate::seal::Instruction for RemoveKeyValue<AssetDefinition> {}
impl crate::seal::Instruction for RemoveKeyValue<Account> {}
impl crate::seal::Instruction for RemoveKeyValue<Nft> {}
impl crate::seal::Instruction for RemoveKeyValue<Trigger> {}
impl crate::seal::Instruction for RemoveAssetKeyValue {}
impl crate::seal::Instruction for AddSignatory {}
impl crate::seal::Instruction for RemoveSignatory {}
impl crate::seal::Instruction for SetAccountQuorum {}
impl crate::seal::Instruction for Grant<Permission, Account> {}
impl crate::seal::Instruction for Grant<RoleId, Account> {}
impl crate::seal::Instruction for Grant<Permission, Role> {}
impl crate::seal::Instruction for Revoke<Permission, Account> {}
impl crate::seal::Instruction for Revoke<RoleId, Account> {}
impl crate::seal::Instruction for Revoke<Permission, Role> {}
impl crate::seal::Instruction for ExecuteTrigger {}
impl crate::seal::Instruction for Upgrade {}
impl crate::seal::Instruction for Log {}

// Allow direct conversion into `InstructionBox` for common built-in instructions
impl From<SetParameter> for super::InstructionBox {
    fn from(instruction: SetParameter) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Stable wire IDs for instruction encoding
impl SetParameter {
    /// Norito wire identifier for parameter updates.
    pub const WIRE_ID: &'static str = "iroha.set_parameter";
}
impl ExecuteTrigger {
    /// Norito wire identifier for trigger execution.
    pub const WIRE_ID: &'static str = "iroha.execute_trigger";
}
impl Upgrade {
    /// Norito wire identifier for runtime upgrades.
    pub const WIRE_ID: &'static str = "iroha.upgrade";
}
impl Log {
    /// Norito wire identifier for log instructions.
    pub const WIRE_ID: &'static str = "iroha.log";
}
impl CustomInstruction {
    /// Norito wire identifier for custom Kotodama instructions.
    pub const WIRE_ID: &'static str = "iroha.custom";
}

#[cfg(feature = "json")]
impl<O> FastJsonWrite for SetKeyValue<O>
where
    O: Identifiable,
    O::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push_str(",\"key\":");
        JsonSerialize::json_serialize(&self.key, out);
        out.push_str(",\"value\":");
        JsonSerialize::json_serialize(&self.value, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for SetAssetKeyValue {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"asset\":");
        JsonSerialize::json_serialize(&self.asset, out);
        out.push_str(",\"key\":");
        JsonSerialize::json_serialize(&self.key, out);
        out.push_str(",\"value\":");
        JsonSerialize::json_serialize(&self.value, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<O> FastJsonWrite for RemoveKeyValue<O>
where
    O: Identifiable,
    O::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push_str(",\"key\":");
        JsonSerialize::json_serialize(&self.key, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for RemoveAssetKeyValue {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"asset\":");
        JsonSerialize::json_serialize(&self.asset, out);
        out.push_str(",\"key\":");
        JsonSerialize::json_serialize(&self.key, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<O, D> FastJsonWrite for Grant<O, D>
where
    O: JsonSerialize,
    D: Identifiable,
    D::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push_str(",\"destination\":");
        JsonSerialize::json_serialize(&self.destination, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<O, D> FastJsonWrite for Revoke<O, D>
where
    O: JsonSerialize,
    D: Identifiable,
    D::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push_str(",\"destination\":");
        JsonSerialize::json_serialize(&self.destination, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for ExecuteTrigger {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"trigger\":");
        JsonSerialize::json_serialize(&self.trigger, out);
        out.push_str(",\"args\":");
        JsonSerialize::json_serialize(&self.args, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for Log {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"level\":");
        JsonSerialize::json_serialize(&self.level, out);
        out.push_str(",\"msg\":");
        JsonSerialize::json_serialize(&self.msg, out);
        out.push('}');
    }
}

impl From<Upgrade> for super::InstructionBox {
    fn from(instruction: Upgrade) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

impl From<CustomInstruction> for super::InstructionBox {
    fn from(instruction: CustomInstruction) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

impl From<InvalidInstruction> for super::InstructionBox {
    fn from(instruction: InvalidInstruction) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Allow direct conversion for smart contract manifest registration instruction
impl From<super::smart_contract_code::RegisterSmartContractCode> for super::InstructionBox {
    fn from(instruction: super::smart_contract_code::RegisterSmartContractCode) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Allow direct conversion for smart contract instance deactivation instruction
impl From<super::smart_contract_code::DeactivateContractInstance> for super::InstructionBox {
    fn from(instruction: super::smart_contract_code::DeactivateContractInstance) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Allow direct conversion for smart contract bytecode registration instruction
impl From<super::smart_contract_code::RegisterSmartContractBytes> for super::InstructionBox {
    fn from(instruction: super::smart_contract_code::RegisterSmartContractBytes) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Allow direct conversion for smart contract bytecode removal instruction
impl From<super::smart_contract_code::RemoveSmartContractBytes> for super::InstructionBox {
    fn from(instruction: super::smart_contract_code::RemoveSmartContractBytes) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Allow direct conversion for contract instance activation instruction
impl From<super::smart_contract_code::ActivateContractInstance> for super::InstructionBox {
    fn from(instruction: super::smart_contract_code::ActivateContractInstance) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Allow direct conversion for council persistence instruction
#[cfg(feature = "governance")]
impl From<super::governance::PersistCouncilForEpoch> for super::InstructionBox {
    fn from(instruction: super::governance::PersistCouncilForEpoch) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Allow direct conversion for governance citizenship and service instructions
#[cfg(feature = "governance")]
impl From<super::governance::RecordCitizenServiceOutcome> for super::InstructionBox {
    fn from(instruction: super::governance::RecordCitizenServiceOutcome) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

#[cfg(feature = "governance")]
impl From<super::governance::RegisterCitizen> for super::InstructionBox {
    fn from(instruction: super::governance::RegisterCitizen) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

#[cfg(feature = "governance")]
impl From<super::governance::UnregisterCitizen> for super::InstructionBox {
    fn from(instruction: super::governance::UnregisterCitizen) -> Self {
        super::Instruction::into_instruction_box(Box::new(instruction))
    }
}

// Convenience accessor to avoid re-decoding `Parameter` in executors
impl SetParameter {
    /// Borrow the underlying `Parameter` value
    pub fn inner(&self) -> &Parameter {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_getters_expose_level_and_message() {
        let log = Log::new(Level::INFO, "hello ffi".to_owned());

        assert_eq!(*log.level(), Level::INFO);
        assert_eq!(log.msg(), "hello ffi");
    }
}
