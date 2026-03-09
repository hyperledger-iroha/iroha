//! Genesis-related logic and constructs. Contains the [`GenesisBlock`],
//! [`RawGenesisTransaction`] and the [`GenesisBuilder`] structures.
#![allow(unexpected_cfgs)]
#![allow(
    clippy::let_and_return,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::double_must_use,
    clippy::field_reassign_with_default,
    clippy::manual_contains,
    clippy::items_after_statements,
    clippy::clone_on_copy
)]
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    fmt::Debug,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
    time::Duration,
};

use derive_more::Constructor;
use eyre::{Result, WrapErr, eyre};
use iroha_config::parameters::{
    actual::Crypto as ActualCrypto, defaults::confidential::RULES_VERSION,
    user::SmIntrinsicsPolicyConfig,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey};
#[cfg(test)]
use iroha_data_model::isi::register::RegisterBox;
use iroha_data_model::{
    account::curve::CurveId,
    block::{
        SignedBlock,
        consensus::{ConsensusGenesisParams, NposGenesisParams},
    },
    confidential::{ConfidentialFeatureDigest, ConfidentialStatus},
    da::commitment::DaProofPolicyBundle,
    isi::{
        InstructionRegistry, Register, SetParameter, register::RegisterPeerWithPop,
        set_instruction_registry, verifying_keys,
    },
    parameter::{
        Parameter,
        custom::CustomParameter,
        system::{
            SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter,
            confidential_metadata, consensus_metadata, crypto_metadata,
        },
    },
    prelude::*,
    proof::{VerifyingKeyId, VerifyingKeyRecord},
};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

/// Domain of the genesis account, technically required for the pre-genesis state
pub static GENESIS_DOMAIN_ID: LazyLock<DomainId> = LazyLock::new(|| "genesis".parse().unwrap());

/// Construct an [`InstructionRegistry`] with all built-in Iroha instructions and
/// set it as the global registry.
///
/// The genesis tooling relies on dynamic instruction (de)serialization. Without
/// initializing the registry attempts to decode [`InstructionBox`] values will
/// fail at runtime.
pub fn init_instruction_registry() {
    set_instruction_registry(default_instruction_registry());
}

/// Create an [`InstructionRegistry`] populated with all instructions supported
/// by Iroha out of the box.
pub fn default_instruction_registry() -> InstructionRegistry {
    iroha_data_model::instruction_registry::default()
}

/// Genesis block, represented as a thin wrapper around the signed block emitted
/// by the builder.
///
/// If an executor upgrade is specified (see [`RawGenesisTransaction::executor`]),
/// the first transaction must contain a single [`Upgrade`] instruction to set
/// the executor. Otherwise, the executor upgrade is omitted and the first
/// transaction may be parameters or other instructions. Subsequent
/// transactions can contain parameter settings, instructions, topology change,
/// and IVM triggers. Callers can access the wrapped [`SignedBlock`] via tuple
/// struct syntax (`GenesisBlock.0`).
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct GenesisBlock(pub SignedBlock);

/// Format of `genesis.json` user file that tooling consumes before producing
/// the canonical [`GenesisBlock`].
///
/// It should be signed, converted to a [`GenesisBlock`],
/// and serialized in Norito format before supplying to an Iroha peer.
/// See `kagami genesis sign`. Only the canonical Norito form is supported. The structure
/// mirrors the user-facing manifest consumed by `kagami genesis`.
#[derive(Debug, Clone, JsonSerialize, IntoSchema, Encode, Decode)]
pub struct RawGenesisTransaction {
    /// Unique chain identifier of the blockchain instance.
    chain: ChainId,
    /// Optional path to the IVM executor bytecode file (`.to`). If omitted,
    /// no executor upgrade is included in genesis.
    #[norito(default)]
    executor: Option<IvmPath>,
    /// Path to the directory that contains prebuilt IVM bytecode referenced by
    /// triggers or other components.
    #[norito(default)]
    ivm_dir: IvmPath,
    /// List of raw genesis transactions that set parameters, execute
    /// instructions, update topology, or configure triggers.
    #[norito(default)]
    transactions: Vec<RawGenesisTx>,
    /// Consensus mode advertised in genesis for operator visibility.
    /// Required in JSON manifests; `None` is reserved for programmatic builders
    /// that set the value later (e.g., via `with_consensus_mode`).
    /// Not consumed by the node runtime (handshake gates the mode independently).
    #[norito(default)]
    consensus_mode: Option<iroha_data_model::parameter::system::SumeragiConsensusMode>,
    /// Optional BLS domain separation string for consensus votes/QCs.
    #[norito(default)]
    bls_domain: Option<String>,
    /// Optional consensus wire protocol versions supported by this genesis.
    #[norito(default)]
    wire_proto_versions: Vec<u32>,
    /// Optional deterministic fingerprint of consensus params (hex string, e.g., 0x..32bytes..).
    #[norito(default)]
    consensus_fingerprint: Option<String>,
    /// Cryptography configuration snapshot advertised alongside the manifest.
    #[norito(default)]
    crypto: ManifestCrypto,
}

/// Cryptography defaults advertised in the genesis manifest.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, IntoSchema, Encode, Decode, PartialEq, Eq,
)]
pub struct ManifestCrypto {
    /// Whether the OpenSSL-backed SM preview helpers are enabled.
    #[norito(default = "iroha_config::parameters::defaults::crypto::enable_sm_openssl_preview")]
    #[cfg_attr(
        feature = "schema",
        schemars(
            default = "iroha_config::parameters::defaults::crypto::enable_sm_openssl_preview"
        )
    )]
    pub sm_openssl_preview: bool,
    /// SM intrinsic dispatch policy (`auto`, `force-enable`, `force-disable`).
    #[norito(default = "iroha_config::parameters::defaults::crypto::sm_intrinsics_policy")]
    pub sm_intrinsics: String,
    /// Default hash algorithm identifier (e.g., `blake2b-256`, `sm3-256`).
    pub default_hash: String,
    /// Signing algorithms allowed for transaction admission.
    pub allowed_signing: Vec<Algorithm>,
    /// Default distinguishing identifier applied when SM2 signatures omit it.
    pub sm2_distid_default: String,
    /// Curve identifiers (per the registry) allowed for account controllers.
    ///
    /// When omitted, the list is derived from `allowed_signing`.
    #[norito(default)]
    pub allowed_curve_ids: Vec<u8>,
}

impl Default for ManifestCrypto {
    fn default() -> Self {
        use iroha_config::parameters::defaults::crypto as defaults;

        Self {
            sm_openssl_preview: defaults::enable_sm_openssl_preview(),
            sm_intrinsics: defaults::sm_intrinsics_policy(),
            default_hash: defaults::default_hash(),
            allowed_signing: defaults::allowed_signing(),
            sm2_distid_default: defaults::sm2_distid_default(),
            allowed_curve_ids: defaults::allowed_curve_ids(),
        }
    }
}

impl ManifestCrypto {
    /// Validate the manifest crypto configuration is internally consistent.
    ///
    /// # Errors
    ///
    /// Returns an error if the signing algorithms omit `ed25519`, if SM2 support
    /// is requested without enabling the `sm` feature toggles, or if SM2-related
    /// defaults (hash algorithm and distinguishing identifier) are inconsistent.
    pub fn validate(&self) -> eyre::Result<()> {
        if !self
            .allowed_signing
            .iter()
            .any(|algo| matches!(algo, Algorithm::Ed25519))
        {
            return Err(eyre!(
                "`allowed_signing` must include `ed25519` for control-plane operations"
            ));
        }

        let has_sm2 = self
            .allowed_signing
            .iter()
            .any(|algo| algo.as_static_str().eq_ignore_ascii_case("sm2"));

        if has_sm2 && !cfg!(feature = "sm") {
            return Err(eyre!(
                "`allowed_signing` includes `sm2`, but this build lacks SM support"
            ));
        }

        if has_sm2 {
            if !self.default_hash.trim().eq_ignore_ascii_case("sm3-256") {
                return Err(eyre!(
                    "`default_hash` must be `sm3-256` when `allowed_signing` contains `sm2`"
                ));
            }

            if self.sm2_distid_default.trim().is_empty() {
                return Err(eyre!(
                    "`sm2_distid_default` must be non-empty when `allowed_signing` contains `sm2`"
                ));
            }
        } else if self.default_hash.trim().eq_ignore_ascii_case("sm3-256") {
            return Err(eyre!(
                "`default_hash` is `sm3-256`, but `allowed_signing` does not include `sm2`; add `sm2` to enable SM cryptography"
            ));
        }

        if self.sm_openssl_preview && !cfg!(feature = "sm-ffi-openssl") {
            return Err(eyre!(
                "`sm_openssl_preview` requires building with the `sm-ffi-openssl` feature"
            ));
        }

        // Validate SM intrinsic policy string.
        SmIntrinsicsPolicyConfig::from_str(self.sm_intrinsics.as_str())?;

        let allowed_curves = self.resolved_allowed_curve_ids();
        if allowed_curves.is_empty() {
            return Err(eyre!(
                "`allowed_curve_ids` resolved to an empty set; enable at least one curve (ed25519)"
            ));
        }

        for id in &allowed_curves {
            let curve = CurveId::try_from(*id).map_err(|err| {
                eyre!("`allowed_curve_ids` contains unknown identifier {id:#04X}: {err}")
            })?;
            let algo = curve.algorithm();
            if !self.allowed_signing.contains(&algo) {
                return Err(eyre!(
                    "`allowed_curve_ids` includes curve id {id:#04X} ({}) \
                     but `allowed_signing` does not list the matching algorithm",
                    algo.as_static_str()
                ));
            }
        }

        Ok(())
    }

    /// Determine whether SM helper syscalls should be enabled based on the manifest.
    #[must_use]
    pub fn sm_helpers_enabled(&self) -> bool {
        #[cfg(feature = "sm")]
        {
            self.allowed_signing
                .iter()
                .any(|algo| matches!(algo, Algorithm::Sm2))
        }
        #[cfg(not(feature = "sm"))]
        {
            let _ = self;
            false
        }
    }

    fn resolved_allowed_curve_ids(&self) -> Vec<u8> {
        let mut ids = if self.allowed_curve_ids.is_empty() {
            iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
                &self.allowed_signing,
            )
        } else {
            self.allowed_curve_ids.clone()
        };
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

impl From<ManifestCrypto> for ActualCrypto {
    fn from(value: ManifestCrypto) -> Self {
        let allowed_curve_ids = value.resolved_allowed_curve_ids();
        let ManifestCrypto {
            sm_openssl_preview,
            sm_intrinsics,
            default_hash,
            allowed_signing,
            sm2_distid_default,
            ..
        } = value;
        Self {
            enable_sm_openssl_preview: sm_openssl_preview,
            sm_intrinsics: SmIntrinsicsPolicyConfig::from(sm_intrinsics.as_str()).into(),
            default_hash,
            allowed_signing,
            sm2_distid_default,
            allowed_curve_ids,
        }
    }
}

#[derive(Default)]
struct GenesisVkRegistry {
    entries: BTreeMap<VerifyingKeyId, VerifyingKeyRecord>,
    by_circuit: BTreeMap<(String, u32), VerifyingKeyId>,
}

/// Compute the verifying-key set hash derived from the provided genesis instructions.
///
/// # Errors
///
/// Returns an [`eyre::Report`] if any instruction fails while building the verifying-key registry.
pub fn compute_genesis_vk_set_hash<'a, I>(instructions: I) -> eyre::Result<Option<[u8; 32]>>
where
    I: IntoIterator<Item = &'a InstructionBox>,
{
    GenesisVkRegistry::build(instructions).map(|registry| registry.vk_set_hash())
}

impl GenesisVkRegistry {
    fn build<'a, I>(instructions: I) -> eyre::Result<Self>
    where
        I: IntoIterator<Item = &'a InstructionBox>,
    {
        let mut registry = Self::default();
        for instr in instructions {
            registry.apply_instruction(instr)?;
        }
        Ok(registry)
    }

    fn apply_instruction(&mut self, instr: &InstructionBox) -> eyre::Result<()> {
        if let Some(register) = instr
            .as_any()
            .downcast_ref::<verifying_keys::RegisterVerifyingKey>()
        {
            self.apply_register(register.id(), register.record())?;
        } else if let Some(update) = instr
            .as_any()
            .downcast_ref::<verifying_keys::UpdateVerifyingKey>()
        {
            self.apply_update(update.id(), update.record())?;
        }
        Ok(())
    }

    fn apply_register(
        &mut self,
        id: &VerifyingKeyId,
        record: &VerifyingKeyRecord,
    ) -> eyre::Result<()> {
        if self.entries.contains_key(id) {
            return Err(eyre!(
                "duplicate verifying key `{}` in genesis",
                Self::id_display(id)
            ));
        }
        if record.circuit_id.trim().is_empty() {
            return Err(eyre!(
                "verifying key `{}` missing circuit_id in genesis",
                Self::id_display(id)
            ));
        }
        if record.public_inputs_schema_hash == [0u8; 32] {
            return Err(eyre!(
                "verifying key `{}` missing public_inputs_schema_hash in genesis",
                Self::id_display(id)
            ));
        }
        if record.gas_schedule_id.is_none() {
            return Err(eyre!(
                "verifying key `{}` missing gas_schedule_id in genesis",
                Self::id_display(id)
            ));
        }
        let key = (record.circuit_id.clone(), record.version);
        if let Some(existing) = self.by_circuit.get(&key)
            && existing != id
        {
            return Err(eyre!(
                "circuit `{}` version {} already bound to `{}` in genesis",
                record.circuit_id,
                record.version,
                Self::id_display(existing)
            ));
        }
        self.entries.insert(id.clone(), record.clone());
        self.by_circuit.insert(key, id.clone());
        Ok(())
    }

    fn apply_update(
        &mut self,
        id: &VerifyingKeyId,
        record: &VerifyingKeyRecord,
    ) -> eyre::Result<()> {
        let Some(old) = self.entries.get(id) else {
            return Err(eyre!(
                "verifying key `{}` updated before registration in genesis",
                Self::id_display(id)
            ));
        };
        if record.version <= old.version {
            return Err(eyre!(
                "verifying key `{}` update does not bump version ({} -> {}) in genesis",
                Self::id_display(id),
                old.version,
                record.version
            ));
        }
        if record.circuit_id.trim().is_empty() {
            return Err(eyre!(
                "verifying key `{}` update missing circuit_id in genesis",
                Self::id_display(id)
            ));
        }
        if record.public_inputs_schema_hash == [0u8; 32] {
            return Err(eyre!(
                "verifying key `{}` update missing public_inputs_schema_hash in genesis",
                Self::id_display(id)
            ));
        }
        if record.gas_schedule_id.is_none() {
            return Err(eyre!(
                "verifying key `{}` update missing gas_schedule_id in genesis",
                Self::id_display(id)
            ));
        }
        let old_key = (old.circuit_id.clone(), old.version);
        self.by_circuit.remove(&old_key);
        let new_key = (record.circuit_id.clone(), record.version);
        if let Some(existing) = self.by_circuit.get(&new_key)
            && existing != id
        {
            return Err(eyre!(
                "circuit `{}` version {} already bound to `{}` in genesis update",
                record.circuit_id,
                record.version,
                Self::id_display(existing)
            ));
        }
        self.entries.insert(id.clone(), record.clone());
        self.by_circuit.insert(new_key, id.clone());
        Ok(())
    }

    fn vk_set_hash(&self) -> Option<[u8; 32]> {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, rec)| matches!(rec.status, ConfidentialStatus::Active))
            .collect();
        if entries.is_empty() {
            return None;
        }
        entries.sort_by(|(id_a, rec_a), (id_b, rec_b)| {
            rec_a
                .circuit_id
                .cmp(&rec_b.circuit_id)
                .then(rec_a.version.cmp(&rec_b.version))
                .then(id_a.backend.as_str().cmp(id_b.backend.as_str()))
                .then(id_a.name.cmp(&id_b.name))
        });
        let mut buf = Vec::with_capacity(entries.len() * 160);
        for (id, rec) in entries {
            buf.extend_from_slice(id.backend.as_bytes());
            buf.push(0);
            buf.extend_from_slice(id.name.as_bytes());
            buf.push(0);
            buf.extend_from_slice(rec.circuit_id.as_bytes());
            buf.push(0);
            buf.extend_from_slice(&rec.version.to_le_bytes());
            buf.extend_from_slice(&rec.commitment);
            buf.extend_from_slice(&rec.public_inputs_schema_hash);
            if let Some(ref gas) = rec.gas_schedule_id {
                buf.extend_from_slice(gas.as_bytes());
            }
            buf.push(0xFF);
        }
        Some(Hash::new(&buf).into())
    }

    fn id_display(id: &VerifyingKeyId) -> String {
        format!("{}::{}", id.backend.as_str(), id.name)
    }
}

/// Norito-compatible JSON helpers for serializing and deserializing genesis instruction lists.
pub mod genesis_instructions_json {
    use std::{collections::BTreeMap, str::FromStr};

    use iroha_data_model::{
        account::NewAccount,
        asset::definition::NewAssetDefinition,
        domain::NewDomain,
        isi::{
            ActivatePublicLaneValidator, Grant, GrantBox, InstructionBox, Mint, MintBox, Register,
            RegisterPublicLaneValidator, SetParameter, Transfer, TransferBox,
            register::RegisterBox,
        },
        metadata::Metadata,
        nexus::LaneId,
        parameter::Parameter,
        permission::Permission,
        prelude::{AccountId, AssetDefinitionId, AssetId, DomainId},
    };
    use iroha_primitives::numeric::Numeric;
    use norito::json::{self, Number, Parser, SeqVisitor, Value};

    use super::*;

    /// Render a slice of instructions into a JSON array suitable for the genesis manifest.
    pub fn serialize(instructions: &[InstructionBox], out: &mut String) {
        out.push('[');
        for (idx, instruction) in instructions.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            let value = instruction_value(instruction);
            let rendered = norito::json::to_json(&value).expect("render genesis instruction JSON");
            out.push_str(&rendered);
        }
        out.push(']');
    }

    /// Convert a slice of instructions into a structured JSON value array.
    #[must_use]
    pub fn instructions_to_value(instructions: &[InstructionBox]) -> Value {
        Value::Array(
            instructions
                .iter()
                .map(instruction_value)
                .collect::<Vec<_>>(),
        )
    }

    /// Convert an instruction into a structured JSON value, falling back to base64 if JSON conversion fails.
    pub fn instruction_value(instruction: &InstructionBox) -> Value {
        instruction_value_inner(instruction, None)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn instruction_value_with_override(
        instruction: &InstructionBox,
        override_value: Option<Result<Value, json::Error>>,
    ) -> Value {
        instruction_value_inner(instruction, override_value)
    }

    fn instruction_value_inner(
        instruction: &InstructionBox,
        override_value: Option<Result<Value, json::Error>>,
    ) -> Value {
        if let Some(value) = instruction_to_value(instruction) {
            return value;
        }

        let value_result = override_value
            .unwrap_or_else(|| norito::json::value::to_value(instruction))
            .expect("serialize genesis instruction to JSON");
        value_result
    }

    /// Deserialize a sequence of genesis instructions from a JSON parser.
    ///
    /// # Errors
    /// Returns an error when the JSON stream cannot be parsed into genesis instructions
    /// or when any instruction fails to decode.
    pub fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<InstructionBox>, json::Error> {
        let mut seq = SeqVisitor::new(parser)?;
        let mut instructions = Vec::new();
        while let Some(value) = seq.next_element::<Value>()? {
            match value_to_instruction(value) {
                Ok(instr) => instructions.push(instr),
                Err(err) => {
                    return Err(json::Error::Message(format!(
                        "failed to decode genesis instruction: {err}"
                    )));
                }
            }
        }
        seq.finish()?;
        Ok(instructions)
    }

    fn value_to_instruction(value: Value) -> Result<InstructionBox, json::Error> {
        match value {
            Value::Array(_) => Err(json::Error::Message(
                "genesis instructions must be structured objects; byte arrays are unsupported"
                    .to_string(),
            )),
            Value::String(_) => Err(json::Error::Message(
                "genesis instructions must be structured objects; base64 strings are unsupported"
                    .to_string(),
            )),
            Value::Object(map) => {
                if map.len() == 1 {
                    if let Some((kind, inner)) = map.iter().next() {
                        let decoded = match kind.as_str() {
                            "Register" => try_decode_register(inner.clone())?,
                            "Mint" => try_decode_mint(inner.clone())?,
                            "Transfer" => try_decode_transfer(inner.clone())?,
                            "SetParameter" => try_decode_set_parameter(inner.clone())?,
                            "Grant" => try_decode_grant(inner.clone())?,
                            "RegisterPublicLaneValidator" => {
                                try_decode_register_public_lane_validator(inner.clone())?
                            }
                            "ActivatePublicLaneValidator" => {
                                try_decode_activate_public_lane_validator(inner.clone())?
                            }
                            _ => None,
                        };
                        if let Some(instr) = decoded {
                            return Ok(instr);
                        }
                    }
                }
                norito::json::value::from_value::<InstructionBox>(Value::Object(map)).map_err(
                    |err| {
                        json::Error::Message(format!(
                            "unsupported genesis instruction object: {err}"
                        ))
                    },
                )
            }
            other => Err(json::Error::Message(format!(
                "genesis instructions must be objects; found {other:?}"
            ))),
        }
    }

    fn try_decode_register(inner: Value) -> Result<Option<InstructionBox>, json::Error> {
        let map = match inner {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        if map.len() != 1 {
            return Ok(None);
        }
        let (variant, payload) = map.into_iter().next().unwrap();
        let instruction = match variant.as_str() {
            "Domain" => {
                let new_domain: NewDomain = norito::json::value::from_value(payload)?;
                InstructionBox::from(Register::domain(new_domain))
            }
            "Account" => {
                let new_account: NewAccount = norito::json::value::from_value(payload)?;
                InstructionBox::from(Register::account(new_account))
            }
            "AssetDefinition" => {
                let new_asset_definition: NewAssetDefinition =
                    norito::json::value::from_value(payload)?;
                InstructionBox::from(Register::asset_definition(new_asset_definition))
            }
            _ => return Ok(None),
        };
        Ok(Some(instruction))
    }

    fn try_decode_mint(inner: Value) -> Result<Option<InstructionBox>, json::Error> {
        let variants = match inner {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        if variants.len() != 1 {
            return Ok(None);
        }
        let (variant, payload) = variants.into_iter().next().unwrap();
        if variant != "Asset" {
            return Ok(None);
        }
        let mut fields = match payload {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for Mint::Asset fields, found {other:?}"
                )));
            }
        };
        let destination_str = take_string(&mut fields, "destination")?;
        let asset_id: AssetId = parse_id(&destination_str, "asset destination")?;
        let object_value = fields
            .remove("object")
            .ok_or_else(|| json::Error::missing_field("object"))?;
        ensure_no_extra_fields(&fields)?;
        let quantity = parse_numeric(object_value)?;
        let instruction = InstructionBox::from(Mint::asset_numeric(quantity, asset_id));
        Ok(Some(instruction))
    }

    fn try_decode_transfer(inner: Value) -> Result<Option<InstructionBox>, json::Error> {
        let variants = match inner {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        if variants.len() != 1 {
            return Ok(None);
        }
        let (variant, payload) = variants.into_iter().next().unwrap();
        let mut fields = match payload {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for Transfer::{variant} fields, found {other:?}"
                )));
            }
        };
        let instruction = match variant.as_str() {
            "AssetDefinition" => {
                let source_str = take_string(&mut fields, "source")?;
                let source: AccountId = parse_account_id(&source_str, "transfer source account")?;
                let object_str = take_string(&mut fields, "object")?;
                let object: AssetDefinitionId = parse_id(&object_str, "asset definition")?;
                let destination_str = take_string(&mut fields, "destination")?;
                let destination: AccountId =
                    parse_account_id(&destination_str, "transfer destination account")?;
                ensure_no_extra_fields(&fields)?;
                InstructionBox::from(Transfer::asset_definition(source, object, destination))
            }
            "Domain" => {
                let source_str = take_string(&mut fields, "source")?;
                let source: AccountId = parse_account_id(&source_str, "transfer source account")?;
                let domain_str = take_string(&mut fields, "object")?;
                let domain: DomainId = parse_id(&domain_str, "domain")?;
                let destination_str = take_string(&mut fields, "destination")?;
                let destination: AccountId =
                    parse_account_id(&destination_str, "transfer destination account")?;
                ensure_no_extra_fields(&fields)?;
                InstructionBox::from(Transfer::domain(source, domain, destination))
            }
            _ => return Ok(None),
        };
        Ok(Some(instruction))
    }

    fn try_decode_set_parameter(inner: Value) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = match inner {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        let parameter_value = fields
            .remove("parameter")
            .ok_or_else(|| json::Error::missing_field("parameter"))?;
        ensure_no_extra_fields(&fields)?;
        let parameter: Parameter = norito::json::value::from_value(parameter_value)?;
        Ok(Some(InstructionBox::from(SetParameter::new(parameter))))
    }

    fn try_decode_grant(inner: Value) -> Result<Option<InstructionBox>, json::Error> {
        let variants = match inner {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        if variants.len() != 1 {
            return Ok(None);
        }
        let (variant, payload) = variants.into_iter().next().unwrap();
        if variant != "Permission" {
            return Ok(None);
        }
        let mut fields = match payload {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for Grant::Permission fields, found {other:?}"
                )));
            }
        };
        let destination: AccountId = parse_account_id(
            &take_string(&mut fields, "destination")?,
            "grant destination account",
        )?;
        let object_value = fields
            .remove("object")
            .ok_or_else(|| json::Error::missing_field("object"))?;
        ensure_no_extra_fields(&fields)?;
        let mut permission_fields = match object_value {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for permission fields, found {other:?}"
                )));
            }
        };
        match permission_fields.get("name") {
            Some(Value::String(_)) => {}
            Some(other) => {
                return Err(json::Error::Message(format!(
                    "expected string for permission name, found {other:?}"
                )));
            }
            None => return Err(json::Error::missing_field("name")),
        }
        permission_fields
            .entry("payload".to_owned())
            .or_insert(Value::Null);
        ensure_only_keys(&permission_fields, &["name", "payload"])?;
        let permission: Permission =
            norito::json::value::from_value(Value::Object(permission_fields))?;
        let instruction = InstructionBox::from(Grant::account_permission(permission, destination));
        Ok(Some(instruction))
    }

    fn try_decode_register_public_lane_validator(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = match inner {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for RegisterPublicLaneValidator fields, found {other:?}"
                )));
            }
        };
        let lane_value = fields
            .remove("lane_id")
            .ok_or_else(|| json::Error::missing_field("lane_id"))?;
        let lane_id = LaneId::from(parse_u32(lane_value, "lane_id")?);
        let validator_str = take_string(&mut fields, "validator")?;
        let validator: AccountId = parse_account_id(&validator_str, "validator")?;
        let stake_account_str = take_string(&mut fields, "stake_account")?;
        let stake_account: AccountId = parse_account_id(&stake_account_str, "stake_account")?;
        let stake_value = fields
            .remove("initial_stake")
            .ok_or_else(|| json::Error::missing_field("initial_stake"))?;
        let initial_stake = parse_numeric(stake_value)?;
        let metadata_value = fields.remove("metadata");
        let metadata = match metadata_value {
            Some(Value::Null) | None => Metadata::default(),
            Some(value) => norito::json::value::from_value(value)?,
        };
        ensure_no_extra_fields(&fields)?;
        let register = RegisterPublicLaneValidator::new(
            lane_id,
            validator,
            stake_account,
            initial_stake,
            metadata,
        );
        Ok(Some(InstructionBox::from(register)))
    }

    fn try_decode_activate_public_lane_validator(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = match inner {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for ActivatePublicLaneValidator fields, found {other:?}"
                )));
            }
        };
        let lane_value = fields
            .remove("lane_id")
            .ok_or_else(|| json::Error::missing_field("lane_id"))?;
        let lane_id = LaneId::from(parse_u32(lane_value, "lane_id")?);
        let validator_str = take_string(&mut fields, "validator")?;
        let validator: AccountId = parse_account_id(&validator_str, "validator")?;
        ensure_no_extra_fields(&fields)?;
        let activate = ActivatePublicLaneValidator::new(lane_id, validator);
        Ok(Some(InstructionBox::from(activate)))
    }

    fn take_string(
        fields: &mut BTreeMap<String, Value>,
        field: &'static str,
    ) -> Result<String, json::Error> {
        match fields.remove(field) {
            Some(Value::String(s)) => Ok(s),
            Some(other) => Err(json::Error::Message(format!(
                "expected string for `{field}`, found {other:?}"
            ))),
            None => Err(json::Error::missing_field(field)),
        }
    }

    fn ensure_no_extra_fields(fields: &BTreeMap<String, Value>) -> Result<(), json::Error> {
        if let Some(field) = fields.keys().next().cloned() {
            return Err(json::Error::UnknownField { field });
        }
        Ok(())
    }

    fn ensure_only_keys(
        fields: &BTreeMap<String, Value>,
        allowed: &[&str],
    ) -> Result<(), json::Error> {
        for key in fields.keys() {
            if !allowed.iter().any(|allowed_key| key == allowed_key) {
                return Err(json::Error::UnknownField { field: key.clone() });
            }
        }
        Ok(())
    }

    fn parse_id<T>(value: &str, label: &'static str) -> Result<T, json::Error>
    where
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        value
            .parse::<T>()
            .map_err(|err| json::Error::Message(format!("invalid {label}: {err}")))
    }

    fn parse_account_id(value: &str, label: &'static str) -> Result<AccountId, json::Error> {
        AccountId::parse_encoded(value)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .map_err(|err| json::Error::Message(format!("invalid {label}: {err}")))
    }

    fn parse_u32(value: Value, label: &'static str) -> Result<u32, json::Error> {
        match value {
            Value::String(s) => s
                .parse::<u32>()
                .map_err(|err| json::Error::Message(format!("invalid {label}: {err}"))),
            Value::Number(Number::U64(v)) => {
                u32::try_from(v).map_err(|_| json::Error::Message(format!("invalid {label}: {v}")))
            }
            Value::Number(Number::I64(v)) => {
                u32::try_from(v).map_err(|_| json::Error::Message(format!("invalid {label}: {v}")))
            }
            other => Err(json::Error::Message(format!(
                "expected numeric {label} value, found {other:?}"
            ))),
        }
    }

    fn parse_numeric(value: Value) -> Result<Numeric, json::Error> {
        match value {
            Value::String(s) => s
                .parse::<Numeric>()
                .map_err(|err| json::Error::Message(err.to_string())),
            Value::Number(number) => {
                let repr = match number {
                    Number::I64(v) => v.to_string(),
                    Number::U64(v) => v.to_string(),
                    Number::F64(v) => v.to_string(),
                };
                repr.parse::<Numeric>()
                    .map_err(|err| json::Error::Message(err.to_string()))
            }
            other => Err(json::Error::Message(format!(
                "expected numeric value as string or number, found {other:?}"
            ))),
        }
    }

    fn account_literal(account: &AccountId) -> Option<String> {
        account.canonical_ih58().ok()
    }

    fn asset_literal(asset: &AssetId) -> String {
        asset.canonical_encoded()
    }

    #[allow(clippy::too_many_lines)]
    fn instruction_to_value(instruction: &InstructionBox) -> Option<Value> {
        use norito::json::Map;

        fn wrap(kind: &str, variant: &str, value: Value) -> Value {
            let mut variant_map = Map::new();
            variant_map.insert(variant.to_string(), value);
            let mut outer = Map::new();
            outer.insert(kind.to_string(), Value::Object(variant_map));
            Value::Object(outer)
        }

        if let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() {
            return match register {
                RegisterBox::Domain(domain) => norito::json::value::to_value(domain.object())
                    .ok()
                    .map(|value| wrap("Register", "Domain", value)),
                RegisterBox::Account(account) => norito::json::value::to_value(account.object())
                    .ok()
                    .map(|value| wrap("Register", "Account", value)),
                RegisterBox::AssetDefinition(asset_definition) => {
                    norito::json::value::to_value(asset_definition.object())
                        .ok()
                        .map(|value| wrap("Register", "AssetDefinition", value))
                }
                _ => None,
            };
        }

        if let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() {
            return match mint {
                MintBox::Asset(mint_asset) => {
                    let mut fields = Map::new();
                    fields.insert(
                        "object".to_string(),
                        Value::String(mint_asset.object().to_string()),
                    );
                    let destination = asset_literal(mint_asset.destination());
                    fields.insert("destination".to_string(), Value::String(destination));
                    Some(wrap("Mint", "Asset", Value::Object(fields)))
                }
                _ => None,
            };
        }

        if let Some(transfer) = instruction.as_any().downcast_ref::<TransferBox>() {
            return match transfer {
                TransferBox::AssetDefinition(tr) => {
                    let mut fields = Map::new();
                    let source = account_literal(tr.source())?;
                    fields.insert("source".to_string(), Value::String(source));
                    fields.insert("object".to_string(), Value::String(tr.object().to_string()));
                    let destination = account_literal(tr.destination())?;
                    fields.insert("destination".to_string(), Value::String(destination));
                    Some(wrap("Transfer", "AssetDefinition", Value::Object(fields)))
                }
                TransferBox::Domain(tr) => {
                    let mut fields = Map::new();
                    let source = account_literal(tr.source())?;
                    fields.insert("source".to_string(), Value::String(source));
                    fields.insert("object".to_string(), Value::String(tr.object().to_string()));
                    let destination = account_literal(tr.destination())?;
                    fields.insert("destination".to_string(), Value::String(destination));
                    Some(wrap("Transfer", "Domain", Value::Object(fields)))
                }
                _ => None,
            };
        }

        if let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() {
            return norito::json::value::to_value(set_parameter.inner())
                .ok()
                .map(|parameter| {
                    let mut inner = Map::new();
                    inner.insert("parameter".to_string(), parameter);
                    let mut outer = Map::new();
                    outer.insert("SetParameter".to_string(), Value::Object(inner));
                    Value::Object(outer)
                });
        }

        if let Some(grant) = instruction.as_any().downcast_ref::<GrantBox>() {
            return match grant {
                GrantBox::Permission(grant_perm) => {
                    let mut permission_name = grant_perm.object().to_string();
                    if let Some(idx) = permission_name.find('(') {
                        permission_name.truncate(idx);
                    }
                    let mut permission = Map::new();
                    permission.insert("name".to_string(), Value::String(permission_name));
                    let mut fields = Map::new();
                    fields.insert("object".to_string(), Value::Object(permission));
                    let destination = account_literal(grant_perm.destination())?;
                    fields.insert("destination".to_string(), Value::String(destination));
                    Some(wrap("Grant", "Permission", Value::Object(fields)))
                }
                _ => None,
            };
        }

        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
        {
            let mut fields = Map::new();
            fields.insert(
                "lane_id".to_string(),
                Value::Number(Number::U64(u64::from(register.lane_id().as_u32()))),
            );
            let validator = account_literal(register.validator())?;
            fields.insert("validator".to_string(), Value::String(validator));
            let stake_account = account_literal(register.stake_account())?;
            fields.insert("stake_account".to_string(), Value::String(stake_account));
            fields.insert(
                "initial_stake".to_string(),
                Value::String(register.initial_stake().to_string()),
            );
            let metadata = norito::json::value::to_value(register.metadata()).ok()?;
            fields.insert("metadata".to_string(), metadata);
            let mut outer = Map::new();
            outer.insert(
                "RegisterPublicLaneValidator".to_string(),
                Value::Object(fields),
            );
            return Some(Value::Object(outer));
        }

        if let Some(activate) = instruction
            .as_any()
            .downcast_ref::<ActivatePublicLaneValidator>()
        {
            let mut fields = Map::new();
            fields.insert(
                "lane_id".to_string(),
                Value::Number(Number::U64(u64::from(activate.lane_id().as_u32()))),
            );
            let validator = account_literal(activate.validator())?;
            fields.insert("validator".to_string(), Value::String(validator));
            let mut outer = Map::new();
            outer.insert(
                "ActivatePublicLaneValidator".to_string(),
                Value::Object(fields),
            );
            return Some(Value::Object(outer));
        }

        None
    }

    /// Parse genesis instructions from a JSON value.
    ///
    /// # Errors
    /// Returns an error when the provided value cannot be rendered to JSON or when
    /// the resulting stream fails to deserialize into genesis instructions.
    pub fn from_value(value: &Value) -> Result<Vec<InstructionBox>, json::Error> {
        let json = json::to_json(value)?;
        let mut parser = Parser::new(&json);
        let instructions = deserialize(&mut parser)?;
        parser.skip_ws();
        if !parser.eof() {
            let (byte, line, col) = pos_from_offset(parser.input(), parser.position());
            return Err(json::Error::TrailingCharacters { byte, line, col });
        }
        Ok(instructions)
    }

    fn pos_from_offset(s: &str, pos: usize) -> (usize, usize, usize) {
        let bytes = s.as_bytes();
        let mut line = 1usize;
        let mut col = 1usize;
        let mut i = 0usize;
        while i < pos && i < bytes.len() {
            if bytes[i] == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
        }
        (pos, line, col)
    }

    #[cfg(test)]
    mod tests {
        use std::{num::NonZeroU64, path::PathBuf};

        #[allow(unused_imports)]
        use iroha_data_model::{
            domain::Domain,
            isi::{
                GrantBox, Log, MintBox, RegisterBox, SetParameter, TransferBox,
                staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
            },
            level::Level,
            metadata::Metadata,
            nexus::LaneId,
            parameter::{Parameter, TransactionParameter},
            prelude::{
                AccountId, AssetDefinitionId, AssetId, Grant, InstructionBox, Mint, Register,
                Transfer,
            },
        };
        use iroha_executor_data_model::permission::parameter::CanSetParameters;
        use iroha_primitives::json::Json;
        use iroha_test_samples::ALICE_ID;

        use super::*;

        #[test]
        fn instructions_to_value_keeps_structure() {
            let domain = Register::domain(Domain::new("demo".parse().unwrap()));
            let value = instructions_to_value(&[InstructionBox::from(domain)]);
            let arr = value.as_array().expect("array");
            assert_eq!(arr.len(), 1);
            let outer = arr[0].as_object().expect("outer object");
            assert!(outer.contains_key("Register"));
        }

        #[test]
        fn serialize_register_uses_structured_json() {
            let domain = Register::domain(Domain::new("structured".parse().unwrap()));
            let instruction: InstructionBox = domain.into();
            let mut out = String::new();
            serialize(&[instruction], &mut out);
            let parsed = norito::json::from_str::<Value>(&out).expect("parse serialized JSON");
            let array = parsed.as_array().expect("instructions array");
            assert!(array.first().unwrap().is_object());
        }

        #[test]
        fn value_to_instruction_rejects_bytes() {
            let value = Value::Array(vec![Value::Number(Number::U64(1))]);
            let err = value_to_instruction(value).expect_err("byte arrays should be rejected");
            assert!(err.to_string().contains("byte arrays"));
        }

        #[test]
        fn value_to_instruction_rejects_base64_string() {
            let value = Value::String("deadbeef".to_string());
            let err = value_to_instruction(value).expect_err("strings should be rejected");
            assert!(err.to_string().contains("base64 strings"));
        }

        #[test]
        fn deserialize_structured_instructions_roundtrip() {
            let account_id = ALICE_ID.clone();
            let domain = Domain::new(account_id.domain().clone());
            let asset_def_id: AssetDefinitionId =
                format!("coin#{}", account_id.domain()).parse().unwrap();
            let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());

            let parameter = Parameter::Transaction(TransactionParameter::MaxInstructions(
                NonZeroU64::new(64).unwrap(),
            ));

            let instructions: Vec<InstructionBox> = vec![
                Register::domain(domain.clone()).into(),
                Mint::asset_numeric(42u32, asset_id.clone()).into(),
                Transfer::asset_definition(
                    account_id.clone(),
                    asset_def_id.clone(),
                    account_id.clone(),
                )
                .into(),
                Grant::account_permission(CanSetParameters, account_id.clone()).into(),
                SetParameter::new(parameter.clone()).into(),
            ];

            let mut json_text = String::new();
            serialize(&instructions, &mut json_text);
            let parsed =
                norito::json::from_str::<Value>(&json_text).expect("parse serialized JSON");
            let instructions = from_value(&parsed).expect("deserialize instructions");
            assert_eq!(instructions.len(), 5);

            match instructions[0].as_any().downcast_ref::<RegisterBox>() {
                Some(RegisterBox::Domain(reg)) => assert_eq!(reg.object(), &domain),
                other => panic!("unexpected register instruction: {other:?}"),
            }
            match instructions[1].as_any().downcast_ref::<MintBox>() {
                Some(MintBox::Asset(mint)) => {
                    assert_eq!(mint.destination(), &asset_id);
                    assert_eq!(mint.object().to_string(), "42");
                }
                other => panic!("unexpected mint instruction: {other:?}"),
            }
            match instructions[2].as_any().downcast_ref::<TransferBox>() {
                Some(TransferBox::AssetDefinition(tr)) => {
                    assert_eq!(tr.object(), &asset_def_id);
                }
                other => panic!("unexpected transfer instruction: {other:?}"),
            }
            match instructions[3].as_any().downcast_ref::<GrantBox>() {
                Some(GrantBox::Permission(grant)) => {
                    assert_eq!(grant.destination(), &account_id);
                    assert_eq!(grant.object().name(), "CanSetParameters");
                    assert_eq!(grant.object().payload(), &Json::default());
                }
                other => panic!("unexpected grant instruction: {other:?}"),
            }
            match instructions[4].as_any().downcast_ref::<SetParameter>() {
                Some(set_param) => assert_eq!(set_param.inner(), &parameter),
                other => panic!("unexpected set-parameter instruction: {other:?}"),
            }
        }

        #[test]
        fn deserialize_grant_without_payload_defaults_to_null() {
            let account_id = ALICE_ID.clone();
            let account_literal = account_literal(&account_id).expect("account literal");
            let grant_json = format!(
                r#"{{"Grant":{{"Permission":{{"destination":"{account_literal}","object":{{"name":"CanSetParameters"}}}}}}}}"#
            );
            let grant_value =
                norito::json::from_str(&grant_json).expect("parse grant instruction literal");

            let instruction =
                super::value_to_instruction(grant_value).expect("structured grant decodes");
            let GrantBox::Permission(grant) = instruction
                .as_any()
                .downcast_ref::<GrantBox>()
                .expect("GrantBox variant")
            else {
                panic!("expected permission grant");
            };
            assert_eq!(grant.destination(), &account_id);
            assert_eq!(grant.object().name(), "CanSetParameters");
            assert_eq!(grant.object().payload(), &Json::default());
        }

        #[test]
        fn deserialize_structured_instructions_supports_npos_bootstrap() {
            let validator_id = ALICE_ID.clone();
            let register = RegisterPublicLaneValidator::new(
                LaneId::SINGLE,
                validator_id.clone(),
                validator_id.clone(),
                Numeric::from(10_u64),
                Metadata::default(),
            );
            let activate = ActivatePublicLaneValidator::new(LaneId::SINGLE, validator_id.clone());
            let instructions: Vec<InstructionBox> = vec![
                InstructionBox::from(register),
                InstructionBox::from(activate),
            ];

            let mut json_text = String::new();
            serialize(&instructions, &mut json_text);
            let parsed =
                norito::json::from_str::<Value>(&json_text).expect("parse serialized JSON");
            let instructions = from_value(&parsed).expect("deserialize instructions");
            assert_eq!(instructions.len(), 2);

            match instructions[0]
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
            {
                Some(register) => {
                    assert_eq!(*register.lane_id(), LaneId::SINGLE);
                    assert_eq!(register.validator(), &validator_id);
                    assert_eq!(register.stake_account(), &validator_id);
                    assert_eq!(register.initial_stake(), &Numeric::from(10_u64));
                    assert!(register.metadata().is_empty());
                }
                other => panic!("unexpected register validator instruction: {other:?}"),
            }
            match instructions[1]
                .as_any()
                .downcast_ref::<ActivatePublicLaneValidator>()
            {
                Some(activate) => {
                    assert_eq!(*activate.lane_id(), LaneId::SINGLE);
                    assert_eq!(activate.validator(), &validator_id);
                }
                other => panic!("unexpected activate validator instruction: {other:?}"),
            }
        }

        #[test]
        fn defaults_genesis_manifest_parses_structured_instructions() {
            super::super::init_instruction_registry();
            let path =
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");
            let raw = std::fs::read_to_string(&path).expect("read defaults/genesis.json");
            let value: Value = norito::json::from_str(&raw).expect("parse defaults genesis JSON");
            let transactions = value
                .as_object()
                .and_then(|obj| obj.get("transactions"))
                .and_then(Value::as_array)
                .expect("transactions array");
            for (index, tx) in transactions.iter().enumerate() {
                norito::json::value::from_value::<RawGenesisTx>(tx.clone())
                    .unwrap_or_else(|err| panic!("decode transaction {index}: {err}"));
                if let Some(parameters_value) = tx.as_object().and_then(|obj| obj.get("parameters"))
                {
                    norito::json::value::from_value::<Parameters>(parameters_value.clone())
                        .expect("decode structured parameters");
                }
                if let Some(instructions) = tx
                    .as_object()
                    .and_then(|obj| obj.get("instructions"))
                    .and_then(Value::as_array)
                {
                    for instruction in instructions {
                        super::value_to_instruction(instruction.clone())
                            .expect("decode structured instruction");
                    }
                }
            }
            super::RawGenesisTransaction::from_path(&path)
                .expect("defaults genesis manifest should deserialize");
        }

        #[test]
        fn parse_allows_null_executor_in_canonical_manifest() {
            let mut manifest_fields = norito::json::Map::new();
            manifest_fields.insert("chain".to_string(), Value::String("test-chain".to_string()));
            manifest_fields.insert("executor".to_string(), Value::Null);
            manifest_fields.insert("ivm_dir".to_string(), Value::String(".".to_string()));
            manifest_fields.insert(
                "consensus_mode".to_string(),
                Value::String("Permissioned".to_string()),
            );
            manifest_fields.insert(
                "transactions".to_string(),
                Value::Array(vec![Value::Object(norito::json::Map::new())]),
            );
            let manifest = Value::Object(manifest_fields);
            let parsed: RawGenesisTransaction =
                norito::json::value::from_value(manifest).expect("canonical manifest parses");
            assert!(parsed.executor.is_none());
            assert_eq!(parsed.transactions.len(), 1);
        }
    }
}

/// Individual genesis transaction as represented in JSON. A transaction may
/// set parameters, execute instructions, schedule IVM triggers, or set the
/// initial topology.
#[derive(Debug, Clone, JsonDeserialize, IntoSchema, Encode, Decode, Default)]
pub struct RawGenesisTx {
    /// Parameter updates applied at genesis.
    #[norito(skip_serializing_if = "Option::is_none")]
    parameters: Option<Parameters>,
    /// Iroha instructions executed during genesis.
    ///
    /// Genesis JSON stores each instruction as a structured Norito object.
    #[norito(default)]
    #[norito(with = "crate::genesis_instructions_json")]
    instructions: Vec<InstructionBox>,
    /// Triggers whose executable is IVM bytecode, not instructions.
    /// Retained as a dedicated collection until the trigger subsystem unifies
    /// instruction-backed and IVM-backed variants.
    #[norito(default)]
    ivm_triggers: Vec<GenesisIvmTrigger>,
    /// Initial topology (list of peers) to bootstrap the network.
    ///
    /// Entries are provided as `{ "peer": <PeerId>, "pop_hex": "<hex>" }` to keep
    /// peers and their PoPs together. `pop_hex` may be omitted while composing
    /// manifests but must be present before signing.
    #[norito(default)]
    topology: Vec<GenesisTopologyEntry>,
}

impl norito::json::JsonSerialize for RawGenesisTx {
    fn json_serialize(&self, out: &mut String) {
        fn write_field<F>(out: &mut String, first: &mut bool, key: &str, write_value: F)
        where
            F: FnOnce(&mut String),
        {
            if *first {
                *first = false;
            } else {
                out.push(',');
            }
            norito::json::write_json_string(key, out);
            out.push(':');
            write_value(out);
        }

        out.push('{');
        let mut first = true;

        // Preserve deterministic ordering (lexicographic by key) to match prior map output.
        write_field(out, &mut first, "instructions", |out| {
            genesis_instructions_json::instructions_to_value(&self.instructions)
                .json_serialize(out);
        });
        write_field(out, &mut first, "ivm_triggers", |out| {
            self.ivm_triggers.json_serialize(out);
        });
        if let Some(parameters) = &self.parameters {
            write_field(out, &mut first, "parameters", |out| {
                parameters.json_serialize(out);
            });
        }

        write_field(out, &mut first, "topology", |out| {
            self.topology.json_serialize(out);
        });

        out.push('}');
    }
}

impl RawGenesisTx {
    /// Topology entries carried by this transaction.
    #[must_use]
    pub fn topology(&self) -> &[GenesisTopologyEntry] {
        &self.topology
    }
}

/// Peer PoP entry used to merge PoPs into topology entries.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize, IntoSchema, Encode, Decode,
)]
pub struct GenesisPeerPop {
    /// Peer public key.
    pub public_key: PublicKey,
    /// Proof-of-possession bytes.
    pub pop: Vec<u8>,
}

/// Peer + proof-of-possession pair in genesis manifest.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, IntoSchema, Encode, Decode)]
pub struct GenesisTopologyEntry {
    /// Peer identifier.
    pub peer: PeerId,
    /// `PoP` hex string (lowercase, without `0x`).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub pop_hex: Option<String>,
}

impl From<PeerId> for GenesisTopologyEntry {
    fn from(peer: PeerId) -> Self {
        Self {
            peer,
            pop_hex: None,
        }
    }
}

impl GenesisTopologyEntry {
    /// Build a topology entry from raw PoP bytes.
    #[must_use]
    pub fn new(peer: PeerId, pop: Vec<u8>) -> Self {
        Self {
            peer,
            pop_hex: Some(hex::encode(pop)),
        }
    }

    /// Decode the PoP hex string into bytes, if present.
    pub fn pop_bytes(&self) -> Result<Option<Vec<u8>>> {
        let Some(pop_hex) = self.pop_hex.as_deref() else {
            return Ok(None);
        };
        let trimmed = pop_hex
            .strip_prefix("0x")
            .or_else(|| pop_hex.strip_prefix("0X"))
            .unwrap_or(pop_hex);
        let bytes = hex::decode(trimmed).map_err(|err| {
            eyre!(
                "invalid `pop_hex` for topology peer {}: {err}",
                self.peer.public_key()
            )
        })?;
        if bytes.is_empty() {
            return Err(eyre!(
                "`pop_hex` for topology peer {} is empty",
                self.peer.public_key()
            ));
        }
        Ok(Some(bytes))
    }
}

impl norito::json::JsonDeserialize for GenesisTopologyEntry {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        let mut map = match value {
            norito::json::Value::Object(map) => map,
            _ => {
                return Err(norito::json::Error::Message(
                    "topology entries must be objects with `peer` and optional `pop_hex`"
                        .to_string(),
                ));
            }
        };
        let peer_value = map
            .remove("peer")
            .ok_or_else(|| norito::json::Error::missing_field("peer"))?;
        let peer: PeerId = norito::json::value::from_value(peer_value).map_err(|err| {
            norito::json::Error::Message(format!("failed to decode `peer`: {err}"))
        })?;
        let pop_hex = match map.remove("pop_hex") {
            None | Some(norito::json::Value::Null) => None,
            Some(norito::json::Value::String(raw)) => Some(normalize_pop_hex(&raw)?),
            Some(other) => {
                let raw = norito::json::value::from_value::<String>(other).map_err(|err| {
                    norito::json::Error::Message(format!("failed to decode `pop_hex`: {err}"))
                })?;
                Some(normalize_pop_hex(&raw)?)
            }
        };
        if let Some((field, _)) = map.into_iter().next() {
            return Err(norito::json::Error::UnknownField { field });
        }
        Ok(Self { peer, pop_hex })
    }
}

fn normalize_pop_hex(raw: &str) -> Result<String, norito::json::Error> {
    let trimmed = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);
    let bytes = hex::decode(trimmed)
        .map_err(|err| norito::json::Error::Message(format!("invalid `pop_hex`: {err}")))?;
    if bytes.is_empty() {
        return Err(norito::json::Error::Message(
            "`pop_hex` must not be empty".to_string(),
        ));
    }
    Ok(hex::encode(bytes))
}

/// Fully expanded view of a genesis manifest after all automatic injections.
#[derive(Debug, Clone)]
pub struct NormalizedGenesis {
    /// Unique chain identifier.
    pub chain: ChainId,
    /// Optional path to the executor bytecode.
    pub executor: Option<IvmPath>,
    /// Directory containing IVM bytecode libraries.
    pub ivm_dir: PathBuf,
    /// Consensus mode advertised in genesis.
    pub consensus_mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    /// BLS domain separation tag for votes/QCs.
    pub bls_domain: String,
    /// Supported consensus protocol versions.
    pub wire_proto_versions: Vec<u32>,
    /// Deterministic fingerprint of consensus parameters.
    pub consensus_fingerprint: String,
    /// Cryptography snapshot advertised alongside genesis.
    pub crypto: ManifestCrypto,
    /// Final transaction batches that will be signed into the genesis block.
    pub transactions: Vec<Vec<InstructionBox>>,
}

impl NormalizedGenesis {
    /// Render the normalized manifest as a JSON value with structured instructions.
    #[must_use]
    pub fn to_json_value(&self) -> norito::json::Value {
        use norito::json::{Number, Value};

        let mut map = norito::json::Map::new();
        map.insert(
            "chain".to_string(),
            norito::json::value::to_value(&self.chain).expect("serialize chain id"),
        );
        if let Some(path) = &self.executor {
            map.insert(
                "executor".to_string(),
                norito::json::Value::String(path.0.display().to_string()),
            );
        } else {
            map.insert("executor".to_string(), Value::Null);
        }
        map.insert(
            "ivm_dir".to_string(),
            norito::json::Value::String(self.ivm_dir.display().to_string()),
        );
        map.insert(
            "consensus_mode".to_string(),
            norito::json::value::to_value(&self.consensus_mode).expect("serialize consensus_mode"),
        );
        map.insert(
            "bls_domain".to_string(),
            norito::json::Value::String(self.bls_domain.clone()),
        );
        map.insert(
            "wire_proto_versions".to_string(),
            norito::json::value::to_value(&self.wire_proto_versions)
                .expect("serialize wire_proto_versions"),
        );
        map.insert(
            "consensus_fingerprint".to_string(),
            norito::json::Value::String(self.consensus_fingerprint.clone()),
        );
        map.insert(
            "crypto".to_string(),
            norito::json::value::to_value(&self.crypto).expect("serialize crypto"),
        );

        let transactions = self
            .transactions
            .iter()
            .enumerate()
            .map(|(idx, instructions)| {
                let mut tx_map = norito::json::Map::new();
                tx_map.insert("index".to_string(), Value::Number(Number::U64(idx as u64)));
                tx_map.insert(
                    "instructions".to_string(),
                    genesis_instructions_json::instructions_to_value(instructions),
                );
                Value::Object(tx_map)
            })
            .collect();
        map.insert("transactions".to_string(), Value::Array(transactions));

        Value::Object(map)
    }

    /// Render normalized genesis as pretty JSON.
    pub fn to_pretty_json(&self) -> Result<String, norito::json::Error> {
        norito::json::to_json_pretty(&self.to_json_value())
    }
}

/// Path to IVM bytecode file or its directory
#[derive(Debug, Clone, IntoSchema)]
#[schema(transparent = "String")]
pub struct IvmPath(PathBuf);

impl Default for IvmPath {
    fn default() -> Self {
        Self(PathBuf::from("."))
    }
}

impl IvmPath {
    /// Access the underlying path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

fn parameter_targets_same_slot(lhs: &Parameter, rhs: &Parameter) -> bool {
    use core::mem::discriminant;

    match (lhs, rhs) {
        (Parameter::Sumeragi(a), Parameter::Sumeragi(b)) => discriminant(a) == discriminant(b),
        (Parameter::Block(a), Parameter::Block(b)) => discriminant(a) == discriminant(b),
        (Parameter::Transaction(a), Parameter::Transaction(b)) => {
            discriminant(a) == discriminant(b)
        }
        (Parameter::Executor(a), Parameter::Executor(b)) => discriminant(a) == discriminant(b),
        (Parameter::SmartContract(a), Parameter::SmartContract(b)) => {
            discriminant(a) == discriminant(b)
        }
        (Parameter::Custom(a), Parameter::Custom(b)) => a.id() == b.id(),
        _ => false,
    }
}

fn parameters_with_staging(parameters: &Parameters) -> Vec<Parameter> {
    let mut collected: Vec<Parameter> = parameters.parameters().collect();
    if let Some(next_mode) = parameters.sumeragi().next_mode() {
        collected.push(Parameter::Sumeragi(SumeragiParameter::NextMode(next_mode)));
    }
    if let Some(height) = parameters.sumeragi().mode_activation_height() {
        collected.push(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(height),
        ));
    }
    collected
}

fn has_set_parameter(instructions: &[InstructionBox], parameter: &Parameter) -> bool {
    instructions.iter().any(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<SetParameter>()
            .is_some_and(|existing| parameter_targets_same_slot(existing.inner(), parameter))
    })
}

fn collect_parameter_instructions(
    parameters: &Parameters,
    existing: &[InstructionBox],
    manual: &[Parameter],
) -> Vec<InstructionBox> {
    let mut generated = Vec::new();
    for parameter in parameters_with_staging(parameters) {
        match parameter {
            Parameter::Executor(_) | Parameter::Transaction(_) | Parameter::SmartContract(_) => {}
            other => {
                if manual
                    .iter()
                    .any(|manual| parameter_targets_same_slot(manual, &other))
                {
                    continue;
                }
                if has_set_parameter(existing, &other) || has_set_parameter(&generated, &other) {
                    continue;
                }
                generated.push(InstructionBox::from(SetParameter::new(other)));
            }
        }
    }
    generated
}

fn collect_manual_set_parameters(transactions: &[RawGenesisTx]) -> Vec<Parameter> {
    let mut manual = Vec::new();
    for tx in transactions {
        for instruction in &tx.instructions {
            if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() {
                let parameter = set_param.inner().clone();
                if manual.iter().any(|existing| existing == &parameter) {
                    continue;
                }
                manual.push(parameter);
            }
        }
    }
    manual
}

#[derive(Debug, Clone)]
struct ConsensusHandshakeMetadata {
    mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    bls_domain: String,
    wire_proto_versions: Vec<u32>,
    consensus_fingerprint: String,
}

fn parse_consensus_handshake_metadata_from_payload(
    payload: &norito::json::Value,
) -> Option<ConsensusHandshakeMetadata> {
    let mode = payload
        .get("mode")
        .and_then(norito::json::Value::as_str)
        .and_then(|mode| match mode {
            "Permissioned" => {
                Some(iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned)
            }
            "Npos" => Some(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos),
            _ => None,
        })?;
    let bls_domain = payload
        .get("bls_domain")
        .and_then(norito::json::Value::as_str)?
        .to_string();
    let wire_proto_versions = payload
        .get("wire_proto_versions")
        .and_then(norito::json::Value::as_array)
        .and_then(|versions| {
            versions
                .iter()
                .map(|version| version.as_u64().and_then(|value| u32::try_from(value).ok()))
                .collect::<Option<Vec<_>>>()
        })?;
    let consensus_fingerprint = payload
        .get("consensus_fingerprint")
        .and_then(norito::json::Value::as_str)?
        .to_string();

    if wire_proto_versions.is_empty() {
        return None;
    }

    let fp_hex = consensus_fingerprint.trim_start_matches("0x");
    let mut fp = [0u8; 32];
    if hex::decode_to_slice(fp_hex, &mut fp).is_err() {
        return None;
    }

    Some(ConsensusHandshakeMetadata {
        mode,
        bls_domain,
        wire_proto_versions,
        consensus_fingerprint,
    })
}

fn parse_consensus_handshake_metadata(
    transactions: &[RawGenesisTx],
) -> Option<ConsensusHandshakeMetadata> {
    for tx in transactions {
        for instruction in &tx.instructions {
            let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            let Parameter::Custom(custom) = set_param.inner() else {
                continue;
            };
            if custom.id() != &consensus_metadata::handshake_meta_id() {
                continue;
            }
            let payload = match custom
                .payload()
                .try_into_any_norito::<norito::json::Value>()
            {
                Ok(payload) => payload,
                Err(_) => continue,
            };
            if let Some(metadata) = parse_consensus_handshake_metadata_from_payload(&payload) {
                return Some(metadata);
            }
        }

        if let Some(parameters) = &tx.parameters {
            let Some(custom) = parameters
                .custom()
                .get(&consensus_metadata::handshake_meta_id())
            else {
                continue;
            };

            let payload: norito::json::Value = match custom
                .payload()
                .try_into_any_norito::<norito::json::Value>()
            {
                Ok(payload) => payload,
                Err(_) => continue,
            };
            if let Some(metadata) = parse_consensus_handshake_metadata_from_payload(&payload) {
                return Some(metadata);
            }
        }
    }

    None
}

fn is_consensus_handshake_metadata_instruction(instruction: &InstructionBox) -> bool {
    instruction
        .as_any()
        .downcast_ref::<SetParameter>()
        .is_some_and(|set_param| {
            matches!(
                set_param.inner(),
                Parameter::Custom(custom) if custom.id() == &consensus_metadata::handshake_meta_id()
            )
        })
}

fn extract_sumeragi_staging(
    transactions: &[RawGenesisTx],
) -> (
    Option<iroha_data_model::parameter::system::SumeragiConsensusMode>,
    Option<u64>,
) {
    let mut next_mode = None;
    let mut activation_height = None;
    for tx in transactions {
        if let Some(params) = &tx.parameters {
            if next_mode.is_none() {
                next_mode = params.sumeragi().next_mode();
            }
            if activation_height.is_none() {
                activation_height = params.sumeragi().mode_activation_height();
            }
        }
        if next_mode.is_some() && activation_height.is_some() {
            break;
        }
    }
    (next_mode, activation_height)
}

fn compute_consensus_fingerprint_v1(
    chain_id: &ChainId,
    params: &iroha_data_model::block::consensus::ConsensusGenesisParams,
    mode_tag: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    use norito::codec::Encode as _;
    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, mode_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        &iroha_data_model::block::consensus::PROTO_VERSION.to_be_bytes(),
    );
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        chain_id.clone().into_inner().as_bytes(),
    );
    let bytes = params.encode();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, &bytes);
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

impl RawGenesisTransaction {
    fn expect_object(
        value: norito::json::Value,
        context: &'static str,
    ) -> Result<norito::json::Map, norito::json::Error> {
        match value {
            norito::json::Value::Object(map) => Ok(map),
            _ => Err(norito::json::Error::InvalidField {
                field: context.into(),
                message: String::from("expected object"),
            }),
        }
    }

    fn take_required_field<T>(
        map: &mut norito::json::Map,
        field: &'static str,
    ) -> Result<T, norito::json::Error>
    where
        T: norito::json::JsonDeserialize,
    {
        let value = map
            .remove(field)
            .ok_or_else(|| norito::json::Error::missing_field(field))?;
        Self::decode_value(value, field)
    }

    fn take_optional_field<T>(
        map: &mut norito::json::Map,
        field: &'static str,
    ) -> Result<Option<T>, norito::json::Error>
    where
        T: norito::json::JsonDeserialize,
    {
        match map.remove(field) {
            Some(norito::json::Value::Null) | None => Ok(None),
            Some(value) => Self::decode_value(value, field).map(Some),
        }
    }

    fn decode_value<T>(
        value: norito::json::Value,
        field: &'static str,
    ) -> Result<T, norito::json::Error>
    where
        T: norito::json::JsonDeserialize,
    {
        norito::json::value::from_value(value).map_err(|err| {
            norito::json::Error::Message(format!("failed to decode `{field}`: {err}"))
        })
    }

    fn reject_set_parameter_instructions(
        transactions: &[RawGenesisTx],
    ) -> Result<(), norito::json::Error> {
        for (tx_index, tx) in transactions.iter().enumerate() {
            for (instr_index, instruction) in tx.instructions.iter().enumerate() {
                if instruction
                    .as_any()
                    .downcast_ref::<SetParameter>()
                    .is_some()
                {
                    return Err(norito::json::Error::Message(format!(
                        "genesis transactions must not contain SetParameter instructions (tx {tx_index}, instruction {instr_index}); move parameters into the `parameters` block"
                    )));
                }
            }
        }
        Ok(())
    }

    fn from_json_value(value: norito::json::Value) -> Result<Self, norito::json::Error> {
        let mut map = Self::expect_object(value, "RawGenesisTransaction")?;
        let chain = Self::take_required_field::<ChainId>(&mut map, "chain")?;
        let executor = Self::take_optional_field::<IvmPath>(&mut map, "executor")?;
        let ivm_dir = map
            .remove("ivm_dir")
            .map(|value| match value {
                norito::json::Value::String(raw) => Ok(IvmPath(PathBuf::from(raw))),
                norito::json::Value::Null => Ok(IvmPath::default()),
                other => Self::decode_value::<IvmPath>(other, "ivm_dir"),
            })
            .transpose()?
            .unwrap_or_else(IvmPath::default);
        let transactions_value = map
            .remove("transactions")
            .unwrap_or_else(|| norito::json::Value::Array(Vec::new()));
        let transactions =
            Self::decode_value::<Vec<RawGenesisTx>>(transactions_value, "transactions")?;
        Self::reject_set_parameter_instructions(&transactions)?;
        let consensus_mode = Some(Self::take_required_field::<
            iroha_data_model::parameter::system::SumeragiConsensusMode,
        >(&mut map, "consensus_mode")?);
        let bls_domain = Self::take_optional_field::<String>(&mut map, "bls_domain")?;
        let wire_proto_versions = map
            .remove("wire_proto_versions")
            .map(|value| Self::decode_value::<Vec<u32>>(value, "wire_proto_versions"))
            .transpose()?
            .unwrap_or_default();
        let consensus_fingerprint =
            Self::take_optional_field::<String>(&mut map, "consensus_fingerprint")?;
        let crypto = map
            .remove("crypto")
            .map(|value| Self::decode_value::<ManifestCrypto>(value, "crypto"))
            .transpose()?
            .unwrap_or_else(ManifestCrypto::default);
        if let Some((field, _)) = map.into_iter().next() {
            return Err(norito::json::Error::UnknownField { field });
        }
        Ok(Self {
            chain,
            executor,
            ivm_dir,
            transactions,
            consensus_mode,
            bls_domain,
            wire_proto_versions,
            consensus_fingerprint,
            crypto,
        })
    }

    /// Compute the effective parameter set after applying all structured sections and explicit `SetParameter` instructions.
    pub fn effective_parameters(&self) -> Parameters {
        // Mirror `parse()` parameter injection rules: structured `parameters` sections are first
        // turned into `SetParameter` instructions with `collect_parameter_instructions`, which
        // suppresses slots already set manually (any explicit `SetParameter` anywhere in the
        // manifest). This keeps the derived consensus fingerprint consistent with the final
        // parsed instruction batches.
        let manual_parameters = collect_manual_set_parameters(&self.transactions);
        let mut aggregated = Parameters::default();
        for tx in &self.transactions {
            if let Some(params) = &tx.parameters {
                for instruction in
                    collect_parameter_instructions(params, &tx.instructions, &manual_parameters)
                {
                    if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() {
                        aggregated.set_parameter(set_param.inner().clone());
                    }
                }
            }
            for instruction in &tx.instructions {
                if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() {
                    aggregated.set_parameter(set_param.inner().clone());
                }
            }
        }
        aggregated
    }

    /// Populate consensus metadata fields with defaults and a computed fingerprint (v1).
    ///
    /// This helper is best-effort and does not alter existing transactions. It derives
    /// parameters from data-model defaults to produce a stable fingerprint for basic networks.
    #[must_use]
    pub fn with_consensus_meta(mut self) -> Self {
        use iroha_data_model::parameter::system::{
            BlockParameters, SumeragiConsensusMode, SumeragiParameters,
        };
        let params = self.effective_parameters();
        let sumeragi: SumeragiParameters = params.sumeragi().clone();
        let block: BlockParameters = params.block();
        let custom = params.custom();
        let block_time_ms = sumeragi.block_time_ms();
        let commit_time_ms = sumeragi.commit_time_ms();
        let min_finality_ms = sumeragi.min_finality_ms();
        let da_enabled = sumeragi.da_enabled();
        let collectors_k = sumeragi.collectors_k();
        let collectors_redundant_send_r = sumeragi.collectors_redundant_send_r();
        let block_max_transactions = block.max_transactions().get();

        // `effective_parameters()` already applies both structured parameter sections and
        // explicit SetParameter instructions in manifest transaction order.

        let npos_param_id = SumeragiNposParameters::parameter_id();
        let npos_payload = custom
            .get(&npos_param_id)
            .and_then(SumeragiNposParameters::from_custom_parameter);

        let mode_hint = sumeragi.next_mode();
        let mode = self.consensus_mode.or(mode_hint).unwrap_or_else(|| {
            if npos_payload.is_some() {
                SumeragiConsensusMode::Npos
            } else {
                SumeragiConsensusMode::Permissioned
            }
        });

        let (mode_tag, default_bls_domain) = match mode {
            SumeragiConsensusMode::Permissioned => (
                "iroha2-consensus::permissioned-sumeragi@v1",
                "bls-iroha2:permissioned-sumeragi:v1",
            ),
            SumeragiConsensusMode::Npos => (
                "iroha2-consensus::npos-sumeragi@v1",
                "bls-iroha2:npos-sumeragi:v1",
            ),
        };

        let resolved_npos = match (mode, npos_payload) {
            (SumeragiConsensusMode::Npos, Some(npos)) => Some(npos),
            (SumeragiConsensusMode::Npos, None) => Some(SumeragiNposParameters::default()),
            _ => None,
        };

        let epoch_length_blocks = if mode == SumeragiConsensusMode::Npos {
            resolved_npos
                .as_ref()
                .map_or(0, SumeragiNposParameters::epoch_length_blocks)
        } else {
            0
        };

        let bls_domain = self
            .bls_domain
            .clone()
            .unwrap_or_else(|| default_bls_domain.to_string());

        let dm_params = ConsensusGenesisParams {
            block_time_ms,
            commit_time_ms,
            min_finality_ms,
            max_clock_drift_ms: sumeragi.max_clock_drift_ms(),
            collectors_k,
            redundant_send_r: collectors_redundant_send_r,
            block_max_transactions,
            da_enabled,
            epoch_length_blocks,
            bls_domain: bls_domain.clone(),
            npos: resolved_npos.map(|npos| {
                let block_time_for_timeouts_ms = block_time_ms.max(min_finality_ms.max(1));
                let npos_timeouts =
                    iroha_config::parameters::actual::SumeragiNposTimeouts::from_block_time(
                        Duration::from_millis(block_time_for_timeouts_ms),
                    );
                let duration_ms = |value: Duration| -> u64 {
                    let ms = value.as_millis();
                    u64::try_from(ms).expect("NPoS timeout exceeds millisecond range")
                };
                NposGenesisParams {
                    block_time_ms,
                    timeout_propose_ms: duration_ms(npos_timeouts.propose),
                    timeout_prevote_ms: duration_ms(npos_timeouts.prevote),
                    timeout_precommit_ms: duration_ms(npos_timeouts.precommit),
                    timeout_commit_ms: duration_ms(npos_timeouts.commit),
                    timeout_da_ms: duration_ms(npos_timeouts.da),
                    timeout_aggregator_ms: duration_ms(npos_timeouts.aggregator),
                    k_aggregators: npos.k_aggregators(),
                    redundant_send_r: npos.redundant_send_r(),
                    epoch_seed: npos.epoch_seed(),
                    vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                    vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                    max_validators: npos.max_validators(),
                    min_self_bond: npos.min_self_bond(),
                    min_nomination_bond: npos.min_nomination_bond(),
                    max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                    seat_band_pct: npos.seat_band_pct(),
                    max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                    finality_margin_blocks: npos.finality_margin_blocks(),
                    evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                    activation_lag_blocks: npos.activation_lag_blocks(),
                    slashing_delay_blocks: npos.slashing_delay_blocks(),
                }
            }),
        };
        let fp = compute_consensus_fingerprint_v1(&self.chain, &dm_params, mode_tag);
        self.consensus_mode = Some(mode);
        if self.bls_domain.is_none() {
            self.bls_domain = Some(bls_domain);
        }
        if self.wire_proto_versions.is_empty() {
            self.wire_proto_versions = vec![iroha_data_model::block::consensus::PROTO_VERSION];
        }
        self.consensus_fingerprint = Some(format!("0x{}", hex::encode(fp)));
        self
    }

    /// Expand the manifest into a normalized, fully-injected representation.
    ///
    /// The returned structure includes consensus/crypto metadata and the exact
    /// transaction batches that will be signed into the genesis block.
    ///
    /// # Errors
    ///
    /// - if consensus metadata cannot be populated
    /// - if instruction injection fails (e.g., invalid topology PoPs)
    pub fn normalize(self) -> Result<NormalizedGenesis> {
        // Always refresh consensus metadata so fingerprints stay aligned with
        // effective parameters after manifest edits.
        let manifest = self.with_consensus_meta();

        let consensus_mode = manifest.consensus_mode.ok_or_else(|| {
            eyre!("consensus_mode missing after normalization; call with_consensus_meta first")
        })?;
        let bls_domain = manifest.bls_domain.clone().ok_or_else(|| {
            eyre!("bls_domain missing after normalization; call with_consensus_meta first")
        })?;
        if manifest.wire_proto_versions.is_empty() {
            return Err(eyre!(
                "wire_proto_versions missing after normalization; call with_consensus_meta first"
            ));
        }
        let consensus_fingerprint = manifest.consensus_fingerprint.clone().ok_or_else(|| {
            eyre!(
                "consensus_fingerprint missing after normalization; call with_consensus_meta first"
            )
        })?;

        let chain = manifest.chain.clone();
        let executor = manifest.executor.clone();
        let ivm_dir = manifest.ivm_dir.as_path().to_path_buf();
        let wire_proto_versions = manifest.wire_proto_versions.clone();
        let crypto = manifest.crypto.clone();
        let transactions = manifest.parse()?;

        Ok(NormalizedGenesis {
            chain,
            executor,
            ivm_dir,
            consensus_mode,
            bls_domain,
            wire_proto_versions,
            consensus_fingerprint,
            crypto,
            transactions,
        })
    }

    /// Chain identifier advertised in the manifest.
    #[must_use]
    pub fn chain_id(&self) -> &ChainId {
        &self.chain
    }

    /// Raw genesis transactions preserved in the manifest.
    #[must_use]
    pub fn transactions(&self) -> &[RawGenesisTx] {
        &self.transactions
    }

    /// Remove topology entries from all transactions.
    #[must_use]
    pub fn clear_topology(mut self) -> Self {
        for tx in &mut self.transactions {
            tx.topology.clear();
        }
        self
    }

    /// Consensus mode advertised in the manifest.
    ///
    /// `None` is reserved for programmatic builders that have not called
    /// `with_consensus_mode` yet.
    #[must_use]
    pub fn consensus_mode(
        &self,
    ) -> Option<iroha_data_model::parameter::system::SumeragiConsensusMode> {
        self.consensus_mode
    }

    /// Return a copy of the manifest with `consensus_mode` populated for handshake metadata.
    #[must_use]
    pub fn with_consensus_mode(
        mut self,
        mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    ) -> Self {
        self.consensus_mode = Some(mode);
        self
    }

    /// Optional consensus fingerprint string advertised in the manifest.
    #[must_use]
    pub fn consensus_fingerprint(&self) -> Option<&str> {
        self.consensus_fingerprint.as_deref()
    }

    /// Optional BLS domain separation tag advertised in the manifest.
    #[must_use]
    pub fn bls_domain(&self) -> Option<&str> {
        self.bls_domain.as_deref()
    }

    /// Supported consensus wire protocol versions advertised in the manifest.
    #[must_use]
    pub fn wire_proto_versions(&self) -> &[u32] {
        &self.wire_proto_versions
    }

    /// Cryptography configuration snapshot advertised in the manifest.
    #[must_use]
    pub fn crypto(&self) -> &ManifestCrypto {
        &self.crypto
    }
}

#[cfg(test)]
mod tests2 {
    use std::{convert::TryInto, num::NonZeroU64, path::PathBuf};

    use iroha_crypto::Algorithm;
    use iroha_data_model::{
        block::consensus::ConsensusGenesisParams, isi::SetParameter,
        parameter::system::BlockParameter,
    };
    use iroha_version::codec::DecodeVersioned;

    use super::*;

    #[test]
    fn with_consensus_meta_adds_fields_and_stable_fingerprint() {
        let chain = ChainId::from("iroha:test:genesismeta");
        let tx = RawGenesisTransaction {
            chain: chain.clone(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };
        let tx2 = tx.clone().with_consensus_meta();
        assert!(tx2.consensus_mode.is_some());
        assert!(tx2.bls_domain.is_some());
        assert!(!tx2.wire_proto_versions.is_empty());
        let fp1 = tx2.consensus_fingerprint.clone().unwrap();
        let fp2 = tx
            .clone()
            .with_consensus_meta()
            .consensus_fingerprint
            .unwrap();
        assert_eq!(fp1, fp2);

        // Validate that the injected handshake payload parses as JSON.
        let normalized = tx.normalize().expect("normalize empty manifest");
        let mut saw_handshake = false;
        for instr in normalized
            .transactions
            .iter()
            .flat_map(|batch| batch.iter())
        {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
            {
                let payload = custom.payload();
                let parsed: norito::json::Value = norito::json::parse_value(payload.get())
                    .expect("handshake payload JSON must parse");
                assert!(
                    parsed.get("consensus_fingerprint").is_some(),
                    "handshake payload missing fingerprint"
                );
                saw_handshake = true;
            }
        }
        assert!(saw_handshake, "expected handshake parameter");
    }

    #[test]
    fn with_consensus_meta_handles_npos_mode() {
        let chain = ChainId::from("iroha:test:nposmeta");
        let npos = SumeragiNposParameters::default();
        let mut params = Parameters::default();
        params.set_parameter(Parameter::Custom(npos.clone().into()));

        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx {
                parameters: Some(params),
                ..RawGenesisTx::default()
            }],
            consensus_mode: Some(SumeragiConsensusMode::Npos),
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        }
        .with_consensus_meta();

        assert_eq!(manifest.consensus_mode, Some(SumeragiConsensusMode::Npos));
        assert!(
            manifest
                .bls_domain
                .as_deref()
                .is_some_and(|d| d.contains("npos-sumeragi")),
            "unexpected bls_domain: {:?}",
            manifest.bls_domain
        );
        assert!(
            manifest
                .wire_proto_versions
                .iter()
                .any(|v| *v == iroha_data_model::block::consensus::PROTO_VERSION),
            "expected PROTO_VERSION"
        );
        let fp = manifest
            .consensus_fingerprint
            .as_deref()
            .expect("fingerprint must be present");
        assert!(
            fp.starts_with("0x"),
            "fingerprint must be hex-prefixed, got {fp}"
        );

        // Confirm the handshake payload parses and advertises Npos mode.
        let normalized = manifest
            .clone()
            .normalize()
            .expect("normalize staged NPoS manifest");
        let mut saw_handshake = false;
        for instr in normalized
            .transactions
            .iter()
            .flat_map(|batch| batch.iter())
        {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
            {
                let payload = custom.payload();
                let parsed: norito::json::Value = norito::json::parse_value(payload.get())
                    .expect("handshake payload JSON must parse");
                assert_eq!(
                    parsed
                        .get("mode")
                        .and_then(norito::json::Value::as_str)
                        .unwrap_or_default(),
                    "Npos"
                );
                saw_handshake = true;
            }
        }
        assert!(saw_handshake, "expected handshake parameter");
    }

    #[test]
    fn with_consensus_meta_rejects_unpaired_mode_activation() {
        let chain = ChainId::from("iroha:test:unpaired-mode");
        let mut params = Parameters::default();
        params.set_parameter(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(10),
        ));

        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx {
                parameters: Some(params),
                ..RawGenesisTx::default()
            }],
            consensus_mode: Some(SumeragiConsensusMode::Permissioned),
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let err = manifest
            .normalize()
            .expect_err("unpaired activation height must fail");
        assert!(
            err.to_string().contains("consensus mode staging requires"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn with_consensus_meta_prefers_explicit_consensus_mode_for_handshake() {
        let chain = ChainId::from("iroha:test:staged-handshake");
        let mut params = Parameters::default();
        params.set_parameter(Parameter::Sumeragi(SumeragiParameter::NextMode(
            SumeragiConsensusMode::Npos,
        )));
        params.set_parameter(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(7),
        ));

        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx {
                parameters: Some(params),
                ..RawGenesisTx::default()
            }],
            consensus_mode: Some(SumeragiConsensusMode::Permissioned),
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let normalized = manifest
            .normalize()
            .expect("staged manifest should normalize");

        let mut handshake_mode = None;
        for batch in &normalized.transactions {
            for instr in batch {
                if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                    && let Parameter::Custom(custom) = set_param.inner()
                    && custom.id() == &consensus_metadata::handshake_meta_id()
                {
                    let parsed: norito::json::Value =
                        norito::json::parse_value(custom.payload().get())
                            .expect("handshake payload should decode");
                    handshake_mode = parsed
                        .get("mode")
                        .and_then(norito::json::Value::as_str)
                        .map(str::to_string);
                }
            }
        }

        assert_eq!(
            handshake_mode.as_deref(),
            Some("Permissioned"),
            "handshake metadata should reflect configured mode before activation",
        );
    }

    #[test]
    fn with_consensus_meta_respects_block_max_transactions_override() {
        let chain = ChainId::from("iroha:test:blockmax");
        let max_txs = NonZeroU64::new(13).expect("non-zero max transactions");
        let tx = RawGenesisTx {
            instructions: vec![InstructionBox::from(SetParameter::new(Parameter::Block(
                BlockParameter::MaxTransactions(max_txs),
            )))],
            ..RawGenesisTx::default()
        };

        let manifest = RawGenesisTransaction {
            chain: chain.clone(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![tx],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        }
        .with_consensus_meta();

        let params = manifest.effective_parameters();
        assert_eq!(
            params.block().max_transactions().get(),
            max_txs.get(),
            "effective parameters must reflect block max override"
        );

        let expected = compute_consensus_fingerprint_v1(
            &chain,
            &ConsensusGenesisParams {
                block_time_ms: params.sumeragi().block_time_ms(),
                commit_time_ms: params.sumeragi().commit_time_ms(),
                min_finality_ms: params.sumeragi().min_finality_ms(),
                max_clock_drift_ms: params.sumeragi().max_clock_drift_ms(),
                collectors_k: params.sumeragi().collectors_k(),
                redundant_send_r: params.sumeragi().collectors_redundant_send_r(),
                block_max_transactions: params.block().max_transactions().get(),
                da_enabled: params.sumeragi().da_enabled(),
                epoch_length_blocks: 0,
                bls_domain: manifest
                    .bls_domain
                    .as_deref()
                    .expect("bls_domain set")
                    .to_string(),
                npos: None,
            },
            iroha_data_model::block::consensus::PERMISSIONED_TAG,
        );
        let observed = manifest
            .consensus_fingerprint
            .as_deref()
            .expect("consensus fingerprint injected")
            .trim_start_matches("0x")
            .to_ascii_lowercase();
        assert_eq!(observed, hex::encode(expected));
    }

    #[test]
    fn build_and_sign_is_deterministic() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:deterministic");
        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let keypair = KeyPair::random();

        let genesis_a = manifest
            .clone()
            .build_and_sign(&keypair)
            .expect("sign genesis");
        let genesis_b = manifest.build_and_sign(&keypair).expect("sign genesis");

        let bytes_a = genesis_a.0.encode_wire().expect("encode canonical genesis");
        let bytes_b = genesis_b.0.encode_wire().expect("encode canonical genesis");
        assert_eq!(bytes_a, bytes_b, "Genesis encoding must be deterministic");

        let tx_times: Vec<u64> = genesis_a
            .0
            .external_transactions()
            .map(|tx| {
                tx.creation_time()
                    .as_millis()
                    .try_into()
                    .expect("creation_time fits into u64")
            })
            .collect();
        assert!(
            tx_times.windows(2).all(|window| window[0] <= window[1]),
            "transaction creation times must be non-decreasing"
        );
        if let Some(last_tx) = tx_times.last() {
            let block_time = genesis_a.0.header().creation_time().as_millis();
            let block_time = u64::try_from(block_time).expect("block creation time fits into u64");
            assert_eq!(
                block_time,
                last_tx + 1,
                "block creation time must follow the last transaction deterministically"
            );
        }
    }

    #[test]
    fn collect_parameter_instructions_respects_manual_values() {
        use iroha_data_model::parameter::{Parameters, system::SumeragiParameter};

        let parameters = Parameters::default();
        let manual = vec![Parameter::Sumeragi(SumeragiParameter::DaEnabled(true))];
        let generated = collect_parameter_instructions(&parameters, &[], &manual);
        let has_conflict = generated.iter().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .is_some_and(|set| {
                    matches!(
                        set.inner(),
                        Parameter::Sumeragi(SumeragiParameter::DaEnabled(_))
                    )
                })
        });
        assert!(
            !has_conflict,
            "manual Sumeragi overrides must suppress default value reinsertion"
        );
    }

    #[test]
    fn has_set_parameter_detects_conflicting_sumeragi_slots() {
        use iroha_data_model::parameter::system::SumeragiParameter;

        let instruction = InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )));
        assert!(has_set_parameter(
            &[instruction],
            &Parameter::Sumeragi(SumeragiParameter::DaEnabled(false))
        ));
    }

    #[test]
    fn build_and_sign_sets_confidential_digest() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:confdigest");
        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let keypair = KeyPair::random();
        let genesis = manifest.build_and_sign(&keypair).expect("sign genesis");

        assert_eq!(
            genesis.0.header().confidential_features(),
            Some(ConfidentialFeatureDigest::new(
                None,
                None,
                None,
                Some(RULES_VERSION)
            ))
        );
    }

    #[test]
    fn genesis_canonical_wire_roundtrip_preserves_digest() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:wire-digest");
        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let keypair = KeyPair::random();
        let genesis = manifest.build_and_sign(&keypair).expect("sign genesis");

        let wire = genesis.0.canonical_wire().expect("canonical wire encoding");
        let framed = wire.as_framed().to_vec();
        let versioned = wire.as_versioned().to_vec();
        let decoded =
            SignedBlock::decode_all_versioned(&versioned).expect("decode versioned signed block");
        assert_eq!(
            decoded.header().confidential_features(),
            genesis.0.header().confidential_features()
        );

        // Ensure framed payload also decodes through the deframed helper for completeness.
        let deframed =
            iroha_data_model::block::deframe_versioned_signed_block_bytes(framed.as_slice())
                .expect("deframe canonical block");
        let decoded_framed = SignedBlock::decode_all_versioned(deframed.bare_versioned.as_ref())
            .expect("decode deframed signed block");
        assert_eq!(
            decoded_framed.header().confidential_features(),
            genesis.0.header().confidential_features()
        );
    }

    #[test]
    fn effective_parameters_prefers_set_parameter_instructions() {
        use iroha_data_model::{isi::InstructionBox, parameter::system::SumeragiParameter};

        let chain = ChainId::from("iroha:test:paramagg");
        let mut base = Parameters::default();
        base.set_parameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(1_000)));
        let override_instruction = InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(1_500),
        )));
        let tx = RawGenesisTx {
            parameters: Some(base),
            instructions: vec![override_instruction],
            ..RawGenesisTx::default()
        };
        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![tx],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let effective = manifest.effective_parameters();
        assert_eq!(effective.sumeragi().block_time_ms(), 1_500);
    }

    #[test]
    fn effective_parameters_respects_manual_overrides_across_transactions() {
        init_instruction_registry();
        use iroha_data_model::{
            isi::InstructionBox,
            parameter::{
                Parameters,
                custom::CustomParameter,
                system::{SumeragiParameter, confidential_metadata},
            },
        };

        let chain = ChainId::from("iroha:test:paramagg-manual");
        let tx_manual = RawGenesisTx {
            instructions: vec![
                InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::BlockTimeMs(333),
                ))),
                InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::CommitTimeMs(667),
                ))),
            ],
            ..RawGenesisTx::default()
        };

        // Parameters created from a single custom entry still include system defaults.
        // `effective_parameters()` must follow the same suppression rules as `parse()` so
        // that later structured sections don't overwrite globally-manual overrides.
        let conf_param = Parameter::Custom(CustomParameter::new(
            confidential_metadata::registry_root_id(),
            Json::new(norito::json!({ "vk_set_hash": null })),
        ));
        let tx_defaults = RawGenesisTx {
            parameters: Some(Parameters::from_iter([conf_param])),
            ..RawGenesisTx::default()
        };

        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![tx_manual, tx_defaults],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let effective = manifest.effective_parameters();
        assert_eq!(effective.sumeragi().block_time_ms(), 333);
        assert_eq!(effective.sumeragi().commit_time_ms(), 667);
    }

    #[test]
    fn set_parameter_inside_instructions_is_rejected() {
        init_instruction_registry();
        use iroha_data_model::parameter::system::SumeragiParameter;

        let set_param = InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(1_000),
        )));
        let instructions = genesis_instructions_json::instructions_to_value(&[set_param]);

        let mut tx_map = norito::json::Map::new();
        tx_map.insert("instructions".to_string(), instructions);

        let mut manifest_fields = norito::json::Map::new();
        manifest_fields.insert(
            "chain".to_string(),
            norito::json::Value::String("test-chain".into()),
        );
        manifest_fields.insert("executor".to_string(), norito::json::Value::Null);
        manifest_fields.insert(
            "ivm_dir".to_string(),
            norito::json::Value::String(".".into()),
        );
        manifest_fields.insert(
            "consensus_mode".to_string(),
            norito::json::Value::String("Permissioned".into()),
        );
        manifest_fields.insert(
            "transactions".to_string(),
            norito::json::Value::Array(vec![norito::json::Value::Object(tx_map)]),
        );

        let manifest = norito::json::Value::Object(manifest_fields);
        let err = RawGenesisTransaction::from_json_value(manifest)
            .expect_err("SetParameter inside instructions should be rejected");
        assert!(
            err.to_string().contains("SetParameter"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn raw_genesis_requires_consensus_mode() {
        init_instruction_registry();

        let mut manifest_fields = norito::json::Map::new();
        manifest_fields.insert(
            "chain".to_string(),
            norito::json::Value::String("test-chain".into()),
        );
        manifest_fields.insert("executor".to_string(), norito::json::Value::Null);
        manifest_fields.insert(
            "ivm_dir".to_string(),
            norito::json::Value::String(".".into()),
        );
        manifest_fields.insert(
            "transactions".to_string(),
            norito::json::Value::Array(vec![norito::json::Value::Object(norito::json::Map::new())]),
        );

        let manifest = norito::json::Value::Object(manifest_fields);
        let err = RawGenesisTransaction::from_json_value(manifest)
            .expect_err("missing consensus_mode should be rejected");
        assert!(
            err.to_string().contains("consensus_mode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn topology_entries_parse_with_pop_hex() {
        init_instruction_registry();
        let peer = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let peer_value = norito::json::value::to_value(&peer).expect("serialize peer");
        let topo_entry = {
            let mut map = norito::json::Map::new();
            map.insert("peer".to_string(), peer_value);
            map.insert(
                "pop_hex".to_string(),
                norito::json::Value::String("0x00".to_string()),
            );
            norito::json::Value::Object(map)
        };

        let mut tx_map = norito::json::Map::new();
        tx_map.insert(
            "topology".to_string(),
            norito::json::Value::Array(vec![topo_entry]),
        );

        let mut manifest_fields = norito::json::Map::new();
        manifest_fields.insert(
            "chain".to_string(),
            norito::json::Value::String("test-chain".into()),
        );
        manifest_fields.insert("executor".to_string(), norito::json::Value::Null);
        manifest_fields.insert(
            "ivm_dir".to_string(),
            norito::json::Value::String(".".into()),
        );
        manifest_fields.insert(
            "consensus_mode".to_string(),
            norito::json::Value::String("Permissioned".into()),
        );
        manifest_fields.insert(
            "transactions".to_string(),
            norito::json::Value::Array(vec![norito::json::Value::Object(tx_map)]),
        );

        let manifest = norito::json::Value::Object(manifest_fields);
        let parsed =
            RawGenesisTransaction::from_json_value(manifest).expect("topology entry should parse");
        assert_eq!(parsed.transactions.len(), 1);
        let tx = &parsed.transactions[0];
        assert_eq!(tx.topology.len(), 1);
        assert_eq!(tx.topology[0].peer, peer);
        assert_eq!(tx.topology[0].pop_hex.as_deref(), Some("00"));
    }

    #[test]
    fn serialize_topology_embeds_pop_hex() {
        let (peer_pk, _) = KeyPair::random().into_parts();
        let peer = PeerId::from(peer_pk.clone());
        let tx = RawGenesisTx {
            parameters: None,
            instructions: Vec::new(),
            ivm_triggers: Vec::new(),
            topology: vec![GenesisTopologyEntry::new(peer, vec![0xAA, 0xBB])],
        };

        let json = norito::json::to_json(&tx).expect("serialize tx");
        assert!(
            json.contains("\"pop_hex\":\"aabb\""),
            "pop_hex should be embedded alongside topology peer: {json}"
        );
    }

    #[test]
    fn topology_entries_allow_missing_pop_hex() {
        init_instruction_registry();
        let peer = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let peer_value = norito::json::value::to_value(&peer).expect("serialize peer");
        let topo_entry = {
            let mut map = norito::json::Map::new();
            map.insert("peer".to_string(), peer_value);
            norito::json::Value::Object(map)
        };

        let mut tx_map = norito::json::Map::new();
        tx_map.insert(
            "topology".to_string(),
            norito::json::Value::Array(vec![topo_entry]),
        );

        let mut manifest_fields = norito::json::Map::new();
        manifest_fields.insert(
            "chain".to_string(),
            norito::json::Value::String("test-chain".into()),
        );
        manifest_fields.insert("executor".to_string(), norito::json::Value::Null);
        manifest_fields.insert(
            "ivm_dir".to_string(),
            norito::json::Value::String(".".into()),
        );
        manifest_fields.insert(
            "consensus_mode".to_string(),
            norito::json::Value::String("Permissioned".into()),
        );
        manifest_fields.insert(
            "transactions".to_string(),
            norito::json::Value::Array(vec![norito::json::Value::Object(tx_map)]),
        );

        let manifest = norito::json::Value::Object(manifest_fields);
        let parsed = RawGenesisTransaction::from_json_value(manifest)
            .expect("topology entry without pop_hex should parse");
        assert_eq!(parsed.transactions.len(), 1);
        let tx = &parsed.transactions[0];
        assert_eq!(tx.topology.len(), 1);
        assert_eq!(tx.topology[0].peer, peer);
        assert!(tx.topology[0].pop_hex.is_none());
    }

    #[test]
    fn topology_entries_reject_peer_value() {
        init_instruction_registry();
        let peer = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let peer_value = norito::json::value::to_value(&peer).expect("serialize peer");

        let mut tx_map = norito::json::Map::new();
        tx_map.insert(
            "topology".to_string(),
            norito::json::Value::Array(vec![peer_value]),
        );

        let mut manifest_fields = norito::json::Map::new();
        manifest_fields.insert(
            "chain".to_string(),
            norito::json::Value::String("test-chain".into()),
        );
        manifest_fields.insert("executor".to_string(), norito::json::Value::Null);
        manifest_fields.insert(
            "ivm_dir".to_string(),
            norito::json::Value::String(".".into()),
        );
        manifest_fields.insert(
            "consensus_mode".to_string(),
            norito::json::Value::String("Permissioned".into()),
        );
        manifest_fields.insert(
            "transactions".to_string(),
            norito::json::Value::Array(vec![norito::json::Value::Object(tx_map)]),
        );

        let manifest = norito::json::Value::Object(manifest_fields);
        let err = RawGenesisTransaction::from_json_value(manifest)
            .expect_err("peer-only topology entries should be rejected");
        assert!(
            err.to_string().contains("topology entries must be objects"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn clear_topology_removes_all_entries() {
        let chain = ChainId::from("iroha:test:clear-topology");
        let peer_a = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .set_topology(vec![peer_a])
            .next_transaction()
            .set_topology(vec![peer_b])
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned);

        let cleared = manifest.clear_topology();
        assert!(
            cleared
                .transactions()
                .iter()
                .all(|tx| tx.topology().is_empty()),
            "expected all topology entries to be removed"
        );
    }

    #[test]
    fn builder_preserves_consensus_metadata() {
        let manifest = RawGenesisTransaction {
            chain: ChainId::from("iroha:test:builder-meta"),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: Some(SumeragiConsensusMode::Permissioned),
            bls_domain: Some("bls:test-domain".to_string()),
            wire_proto_versions: vec![1],
            consensus_fingerprint: Some("0xabc123".to_string()),
            crypto: ManifestCrypto::default(),
        };

        let rebuilt = manifest
            .clone()
            .into_builder()
            .domain("example".parse().expect("domain name"))
            .finish_domain()
            .build_raw();

        assert_eq!(rebuilt.consensus_mode, manifest.consensus_mode);
        assert_eq!(rebuilt.bls_domain, manifest.bls_domain);
        assert_eq!(rebuilt.wire_proto_versions, manifest.wire_proto_versions);
        assert_eq!(
            rebuilt.consensus_fingerprint,
            manifest.consensus_fingerprint
        );
    }

    #[test]
    fn topology_entry_pop_bytes_none() {
        let peer = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let entry = GenesisTopologyEntry::from(peer);
        let pop = entry.pop_bytes().expect("pop_bytes");
        assert!(pop.is_none());
    }

    #[test]
    fn normalize_exposes_instruction_batches() {
        init_instruction_registry();

        let manifest = RawGenesisTransaction {
            chain: ChainId::from("iroha:test:normalize"),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        };

        let normalized = manifest.normalize().expect("normalize");
        assert!(
            !normalized.transactions.is_empty(),
            "normalize should emit at least one transaction batch"
        );
        assert!(
            !normalized.consensus_fingerprint.is_empty(),
            "normalize should expose fingerprint"
        );
    }

    #[allow(clippy::too_many_lines)]
    #[test]
    fn with_consensus_meta_uses_npos_custom_parameter() {
        use iroha_data_model::parameter::{
            Parameter as DataModelParameter,
            system::{SumeragiConsensusMode, SumeragiParameter},
        };

        fn fingerprint_for(tx: &RawGenesisTransaction) -> [u8; 32] {
            use iroha_data_model::parameter::system::{
                BlockParameters, SumeragiConsensusMode, SumeragiParameters,
            };

            let params = tx.effective_parameters();
            let sumeragi: SumeragiParameters = params.sumeragi().clone();
            let block: BlockParameters = params.block();
            let custom = params.custom().clone();

            let npos_param_id = SumeragiNposParameters::parameter_id();
            let npos_payload = custom
                .get(&npos_param_id)
                .and_then(SumeragiNposParameters::from_custom_parameter);
            let mode_hint = sumeragi.next_mode();
            let mode = tx.consensus_mode.or(mode_hint).unwrap_or_else(|| {
                if npos_payload.is_some() {
                    SumeragiConsensusMode::Npos
                } else {
                    SumeragiConsensusMode::Permissioned
                }
            });

            let resolved_npos = match (mode, npos_payload) {
                (SumeragiConsensusMode::Npos, Some(npos)) => Some(npos),
                (SumeragiConsensusMode::Npos, None) => Some(SumeragiNposParameters::default()),
                (_, payload) => payload,
            };

            let epoch_length_blocks = resolved_npos
                .as_ref()
                .map_or(0, SumeragiNposParameters::epoch_length_blocks);

            let (mode_tag, default_bls_domain) = match mode {
                SumeragiConsensusMode::Permissioned => (
                    "iroha2-consensus::permissioned-sumeragi@v1",
                    "bls-iroha2:permissioned-sumeragi:v1",
                ),
                SumeragiConsensusMode::Npos => (
                    "iroha2-consensus::npos-sumeragi@v1",
                    "bls-iroha2:npos-sumeragi:v1",
                ),
            };

            let bls_domain = tx
                .bls_domain
                .clone()
                .unwrap_or_else(|| default_bls_domain.to_string());

            let dm_params = iroha_data_model::block::consensus::ConsensusGenesisParams {
                block_time_ms: sumeragi.block_time_ms(),
                commit_time_ms: sumeragi.commit_time_ms(),
                min_finality_ms: sumeragi.min_finality_ms(),
                max_clock_drift_ms: sumeragi.max_clock_drift_ms(),
                collectors_k: sumeragi.collectors_k(),
                redundant_send_r: sumeragi.collectors_redundant_send_r(),
                block_max_transactions: block.max_transactions().get(),
                da_enabled: sumeragi.da_enabled(),
                epoch_length_blocks,
                bls_domain,
                npos: resolved_npos.map(|npos| {
                    let block_time_for_timeouts_ms = sumeragi
                        .block_time_ms()
                        .max(sumeragi.min_finality_ms().max(1));
                    let npos_timeouts =
                        iroha_config::parameters::actual::SumeragiNposTimeouts::from_block_time(
                            Duration::from_millis(block_time_for_timeouts_ms),
                        );
                    let duration_ms = |value: Duration| -> u64 {
                        let ms = value.as_millis();
                        u64::try_from(ms).expect("NPoS timeout exceeds millisecond range")
                    };
                    use iroha_data_model::block::consensus::NposGenesisParams;
                    NposGenesisParams {
                        block_time_ms: sumeragi.block_time_ms(),
                        timeout_propose_ms: duration_ms(npos_timeouts.propose),
                        timeout_prevote_ms: duration_ms(npos_timeouts.prevote),
                        timeout_precommit_ms: duration_ms(npos_timeouts.precommit),
                        timeout_commit_ms: duration_ms(npos_timeouts.commit),
                        timeout_da_ms: duration_ms(npos_timeouts.da),
                        timeout_aggregator_ms: duration_ms(npos_timeouts.aggregator),
                        k_aggregators: npos.k_aggregators(),
                        redundant_send_r: npos.redundant_send_r(),
                        epoch_seed: npos.epoch_seed(),
                        vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                        vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                        max_validators: npos.max_validators(),
                        min_self_bond: npos.min_self_bond(),
                        min_nomination_bond: npos.min_nomination_bond(),
                        max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                        seat_band_pct: npos.seat_band_pct(),
                        max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                        finality_margin_blocks: npos.finality_margin_blocks(),
                        evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                        activation_lag_blocks: npos.activation_lag_blocks(),
                        slashing_delay_blocks: npos.slashing_delay_blocks(),
                    }
                }),
            };

            compute_consensus_fingerprint_v1(&tx.chain, &dm_params, mode_tag)
        }

        fn build_manifest(chain: ChainId, seed_byte: u8) -> RawGenesisTransaction {
            let mut parameters = Parameters::default();
            // Align the base timers with a deterministic set so the fingerprint depends on NPoS payload.
            parameters.set_parameter(DataModelParameter::Sumeragi(
                SumeragiParameter::BlockTimeMs(1_000),
            ));
            parameters.set_parameter(DataModelParameter::Sumeragi(
                SumeragiParameter::CommitTimeMs(2_000),
            ));
            parameters.set_parameter(DataModelParameter::Sumeragi(
                SumeragiParameter::MaxClockDriftMs(250),
            ));
            parameters.set_parameter(DataModelParameter::Sumeragi(
                SumeragiParameter::CollectorsK(3),
            ));
            parameters.set_parameter(DataModelParameter::Sumeragi(
                SumeragiParameter::RedundantSendR(2),
            ));
            let npos = SumeragiNposParameters::default().with_epoch_seed([seed_byte; 32]);
            parameters.set_parameter(DataModelParameter::Custom(npos.into()));

            RawGenesisTransaction {
                chain,
                executor: None,
                ivm_dir: IvmPath::default(),
                transactions: vec![RawGenesisTx {
                    parameters: Some(parameters),
                    ..RawGenesisTx::default()
                }],
                consensus_mode: Some(SumeragiConsensusMode::Npos),
                bls_domain: None,
                wire_proto_versions: vec![],
                consensus_fingerprint: None,
                crypto: ManifestCrypto::default(),
            }
        }

        let chain = ChainId::from("iroha:test:nposmeta");

        let manifest_base_a = build_manifest(chain.clone(), 0xA0);
        let manifest_base_b = build_manifest(chain, 0xA1);
        let expected_a = fingerprint_for(&manifest_base_a);
        let expected_b = fingerprint_for(&manifest_base_b);

        let manifest_a = manifest_base_a.with_consensus_meta();
        let manifest_b = manifest_base_b.with_consensus_meta();
        let expected_low_hex = format!("0x{}", hex::encode(expected_a));
        let expected_high_hex = format!("0x{}", hex::encode(expected_b));

        assert_eq!(
            manifest_a.consensus_fingerprint.as_deref(),
            Some(expected_low_hex.as_str())
        );
        assert_eq!(
            manifest_b.consensus_fingerprint.as_deref(),
            Some(expected_high_hex.as_str())
        );
        assert_eq!(manifest_a.consensus_mode, Some(SumeragiConsensusMode::Npos));
        assert!(
            manifest_a
                .wire_proto_versions
                .contains(&iroha_data_model::block::consensus::PROTO_VERSION),
            "wire proto versions must advertise the consensus protocol"
        );
    }

    #[test]
    fn with_consensus_meta_respects_permissioned_next_mode() {
        use iroha_data_model::parameter::{
            Parameter as DataModelParameter,
            system::{SumeragiConsensusMode, SumeragiParameter},
        };

        let chain = ChainId::from("iroha:test:permmeta");
        let mut parameters = Parameters::default();
        let npos_defaults = SumeragiNposParameters::default();
        parameters.set_parameter(DataModelParameter::Custom(npos_defaults.into()));
        parameters.set_parameter(DataModelParameter::Sumeragi(SumeragiParameter::NextMode(
            SumeragiConsensusMode::Permissioned,
        )));

        let manifest = RawGenesisTransaction {
            chain,
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx {
                parameters: Some(parameters),
                ..RawGenesisTx::default()
            }],
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: vec![],
            consensus_fingerprint: None,
            crypto: ManifestCrypto::default(),
        }
        .with_consensus_meta();

        assert_eq!(
            manifest.consensus_mode,
            Some(SumeragiConsensusMode::Permissioned),
            "Declared permissioned next mode should override NPoS payload defaulting"
        );
        assert_eq!(
            manifest.bls_domain.as_deref(),
            Some("bls-iroha2:permissioned-sumeragi:v1"),
            "Permissioned mode should emit permissioned BLS domain"
        );
    }

    #[test]
    fn crypto_manifest_requires_ed25519() {
        init_instruction_registry();

        let crypto = ManifestCrypto {
            allowed_signing: vec![Algorithm::Secp256k1],
            ..ManifestCrypto::default()
        };

        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:crypto-ed25519"),
            PathBuf::from("."),
        )
        .with_crypto(crypto)
        .build_raw();

        let err = manifest
            .build_and_sign(&KeyPair::random())
            .expect_err("manifest without ed25519 should be rejected");
        assert!(
            err.to_string().contains("allowed_signing"),
            "unexpected error: {err:?}"
        );
    }

    #[cfg(feature = "sm")]
    #[test]
    fn crypto_manifest_requires_sm_defaults_when_sm2_allowed() {
        init_instruction_registry();

        let crypto = ManifestCrypto {
            allowed_signing: vec![Algorithm::Ed25519, Algorithm::Sm2],
            ..ManifestCrypto::default()
        };

        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:crypto-sm"),
            PathBuf::from("."),
        )
        .with_crypto(crypto)
        .build_raw();

        let err = manifest
            .build_and_sign(&KeyPair::random())
            .expect_err("manifest missing SM defaults should be rejected");
        assert!(
            err.to_string().contains("default_hash"),
            "unexpected error: {err:?}"
        );
    }

    #[cfg(feature = "sm")]
    #[test]
    fn crypto_manifest_accepts_valid_sm_configuration() {
        init_instruction_registry();

        let crypto = ManifestCrypto {
            default_hash: "sm3-256".to_owned(),
            allowed_signing: vec![Algorithm::Ed25519, Algorithm::Sm2],
            ..ManifestCrypto::default()
        };

        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:crypto-sm-valid"),
            PathBuf::from("."),
        )
        .with_crypto(crypto)
        .build_raw();

        manifest
            .build_and_sign(&KeyPair::random())
            .expect("manifest with valid SM configuration should build");
    }

    #[test]
    fn crypto_manifest_rejects_sm3_hash_without_sm2() {
        init_instruction_registry();

        let crypto = ManifestCrypto {
            default_hash: "sm3-256".to_owned(),
            ..ManifestCrypto::default()
        };

        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:crypto-sm3-without-sm2"),
            PathBuf::from("."),
        )
        .with_crypto(crypto)
        .build_raw();

        let err = manifest
            .build_and_sign(&KeyPair::random())
            .expect_err("manifest using sm3 default hash without sm2 should be rejected");
        assert!(
            err.to_string().contains("default_hash"),
            "unexpected error: {err:?}"
        );
    }
}

impl RawGenesisTransaction {
    const WARN_ON_GENESIS_GTE: u64 = 1024 * 1024 * 1024; // 1Gb

    /// Iterate over all instructions contained in this manifest.
    #[must_use]
    pub fn instructions(&self) -> impl Iterator<Item = &InstructionBox> {
        self.transactions
            .iter()
            .flat_map(|tx| tx.instructions.iter())
    }

    /// Construct [`RawGenesisTransaction`] from a json file at `json_path`,
    /// resolving relative paths to `json_path`.
    ///
    /// # Errors
    ///
    /// - file not found
    /// - metadata access to the file failed
    /// - deserialization failed
    pub fn from_path(json_path: impl AsRef<Path>) -> Result<Self> {
        use std::io::Read as _;
        init_instruction_registry();
        let here = json_path
            .as_ref()
            .parent()
            .expect("json file should be in some directory");
        let file = File::open(&json_path).wrap_err_with(|| {
            eyre!("failed to open genesis at {}", json_path.as_ref().display())
        })?;
        let size = file
            .metadata()
            .wrap_err("failed to access genesis file metadata")?
            .len();
        if size >= Self::WARN_ON_GENESIS_GTE {
            eprintln!(
                "Genesis is quite large, it will take some time to process it (size = {size}, threshold = {})",
                Self::WARN_ON_GENESIS_GTE
            );
        }
        let mut reader = BufReader::new(file);
        let mut contents = String::new();
        reader
            .read_to_string(&mut contents)
            .wrap_err("failed to read genesis file")?;

        let raw_value: norito::json::Value = norito::json::from_str(&contents).map_err(|err| {
            eyre!(
                "failed to deserialize raw genesis transaction from {}: {err}",
                json_path.as_ref().display()
            )
        })?;

        let mut value = RawGenesisTransaction::from_json_value(raw_value).map_err(|err| {
            eyre!(
                "failed to deserialize raw genesis transaction from {}: {err}",
                json_path.as_ref().display()
            )
        })?;

        if value.transactions.is_empty() {
            return Err(eyre!(
                "genesis manifest at {} must include at least one transaction entry",
                json_path.as_ref().display()
            ));
        }

        if let Some(executor) = &mut value.executor {
            executor.resolve(here);
        }
        value.ivm_dir.resolve(here);
        for tx in &mut value.transactions {
            tx.ivm_triggers
                .iter_mut()
                .for_each(|trigger| trigger.action.executable.resolve(&value.ivm_dir.0));
        }

        Ok(value)
    }

    /// Revert to builder to add modifications.
    pub fn into_builder(self) -> GenesisBuilder {
        let transactions = self
            .transactions
            .into_iter()
            .map(|tx| GenesisTxBuilder {
                parameters: tx
                    .parameters
                    .map_or(Vec::new(), |p| parameters_with_staging(&p)),
                instructions: tx.instructions,
                ivm_triggers: tx.ivm_triggers,
                topology: tx.topology,
            })
            .collect();

        GenesisBuilder {
            chain: self.chain,
            executor: self.executor,
            ivm_dir: self.ivm_dir.0,
            transactions,
            crypto: self.crypto,
            da_proof_policies: None,
            consensus_mode: self.consensus_mode,
            bls_domain: self.bls_domain,
            wire_proto_versions: self.wire_proto_versions,
            consensus_fingerprint: self.consensus_fingerprint,
        }
    }

    /// Build and sign genesis block.
    ///
    /// # Errors
    ///
    /// Fails if `RawGenesisTransaction::parse` fails.
    pub fn build_and_sign(self, genesis_key_pair: &KeyPair) -> Result<GenesisBlock> {
        self.build_and_sign_with_da_proof_policies(genesis_key_pair, None)
    }

    /// Build and sign genesis block, overriding the embedded DA proof policies.
    ///
    /// # Errors
    ///
    /// Fails if `RawGenesisTransaction::parse` fails.
    pub fn build_and_sign_with_da_proof_policies(
        self,
        genesis_key_pair: &KeyPair,
        da_proof_policies: Option<DaProofPolicyBundle>,
    ) -> Result<GenesisBlock> {
        let chain = self.chain.clone();
        let genesis_account = AccountId::new(
            GENESIS_DOMAIN_ID.clone(),
            genesis_key_pair.public_key().clone(),
        );
        let instruction_batches = self.parse()?;
        let vk_set_hash =
            compute_genesis_vk_set_hash(instruction_batches.iter().flat_map(|batch| batch.iter()))?;
        let confidential_digest =
            ConfidentialFeatureDigest::new(vk_set_hash, None, None, Some(RULES_VERSION));

        let mut transactions = Vec::new();
        for (tx_index, instructions) in instruction_batches.into_iter().enumerate() {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                let encoded = norito::codec::encode_adaptive(&instructions);
                eprintln!(
                    "GenesisBuilder::build_and_sign: instructions batch len={} encoded_bytes={}",
                    instructions.len(),
                    encoded.len()
                );
            }
            let mut builder = TransactionBuilder::new(chain.clone(), genesis_account.clone())
                .with_instructions(instructions);
            builder.set_creation_time(Duration::from_millis(
                u64::try_from(tx_index)
                    .expect("too many genesis transactions")
                    .saturating_add(1),
            ));
            let transaction = builder.sign(genesis_key_pair.private_key());
            transactions.push(transaction);
        }
        let block = SignedBlock::genesis_with_da_proof_policies(
            transactions,
            genesis_key_pair.private_key(),
            Some(confidential_digest),
            None,
            da_proof_policies,
        );

        Ok(GenesisBlock(block))
    }

    /// Parse [`RawGenesisTransaction`] to the list of source instructions of the genesis transactions
    ///
    /// # Errors
    ///
    /// Fails if `self.executor` path fails to load [`Executor`].
    #[allow(clippy::too_many_lines)]
    fn parse(self) -> Result<Vec<Vec<InstructionBox>>> {
        let explicit_handshake_meta = parse_consensus_handshake_metadata(&self.transactions);
        let mut manifest = self.with_consensus_meta();
        if let Some(handshake_meta) = explicit_handshake_meta {
            manifest.consensus_mode = Some(handshake_meta.mode);
            manifest.bls_domain = Some(handshake_meta.bls_domain);
            manifest.wire_proto_versions = handshake_meta.wire_proto_versions;
            manifest.consensus_fingerprint = Some(handshake_meta.consensus_fingerprint);
        }

        // Recompute generated fields only when no valid injected handshake metadata
        // is present.
        // (Injected values are intentionally preserved so test-network overrides are
        // not accidentally discarded.)

        manifest
            .crypto
            .validate()
            .map_err(|err| eyre!("invalid crypto configuration in genesis manifest: {err}"))?;

        let RawGenesisTransaction {
            chain: _,
            executor,
            ivm_dir: _,
            mut transactions,
            consensus_mode,
            bls_domain,
            wire_proto_versions,
            consensus_fingerprint,
            crypto: _,
        } = manifest;

        for tx in &mut transactions {
            tx.instructions
                .retain(|instruction| !is_consensus_handshake_metadata_instruction(instruction));
            if let Some(parameters) = &mut tx.parameters {
                let filtered_parameters = parameters
                    .parameters()
                    .filter(|parameter| {
                        !matches!(
                            parameter,
                            Parameter::Custom(custom)
                                if custom.id() == &consensus_metadata::handshake_meta_id()
                        )
                    })
                    .collect::<Vec<_>>();
                *parameters = Parameters::from_iter(filtered_parameters);
            }
        }

        let manual_parameters = collect_manual_set_parameters(&transactions);
        let (staged_next_mode, staged_activation_height) = extract_sumeragi_staging(&transactions);
        let meta_vec = Self::build_consensus_meta_instructions(
            consensus_mode,
            bls_domain,
            wire_proto_versions,
            consensus_fingerprint,
            staged_next_mode,
            staged_activation_height,
            &manual_parameters,
        )?;
        let mut pending_meta = if meta_vec.is_empty() {
            None
        } else {
            Some(meta_vec)
        };

        let mut instructions_list = Vec::new();

        if let Some(executor_path) = executor {
            let upgrade_executor = Upgrade::new(Executor::new(executor_path.try_into()?)).into();
            instructions_list.push(vec![upgrade_executor]);
        }

        for tx in transactions {
            let mut instructions = Vec::new();

            if let Some(parameters) = tx.parameters {
                instructions.extend(collect_parameter_instructions(
                    &parameters,
                    &tx.instructions,
                    &manual_parameters,
                ));
            }

            if !tx.instructions.is_empty() {
                instructions.extend(tx.instructions);
            }

            if !tx.ivm_triggers.is_empty() {
                instructions.extend(
                    tx.ivm_triggers
                        .into_iter()
                        .map(Trigger::try_from)
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .map(Register::trigger)
                        .map(InstructionBox::from),
                );
            }

            if !tx.topology.is_empty() {
                let mut seen = BTreeSet::new();
                for entry in tx.topology {
                    let pk = entry.peer.public_key().clone();
                    if !seen.insert(pk.clone()) {
                        return Err(eyre!("duplicate `topology` entry for peer {pk}"));
                    }
                    let pop = entry.pop_bytes()?.ok_or_else(|| {
                        eyre!(
                            "missing `pop_hex` entry for topology peer {}",
                            entry.peer.public_key()
                        )
                    })?;
                    let register = RegisterPeerWithPop::new(entry.peer, pop);
                    instructions.push(InstructionBox::from(register));
                }
            }

            if let Some(meta) = pending_meta.take() {
                if instructions.is_empty() {
                    instructions = meta;
                } else {
                    instructions_list.push(instructions);
                    instructions_list.push(meta);
                    continue;
                }
            }

            if !instructions.is_empty() {
                instructions_list.push(instructions);
            }
        }

        if let Some(meta) = pending_meta
            && !meta.is_empty()
        {
            instructions_list.push(meta);
        }

        Self::inject_crypto_manifest_param(
            &mut instructions_list,
            &manual_parameters,
            &manifest.crypto,
        )?;

        let registry = GenesisVkRegistry::build(instructions_list.iter().flatten())?;
        Self::inject_confidential_registry_param(
            &mut instructions_list,
            &manual_parameters,
            registry.vk_set_hash(),
        );

        Ok(instructions_list)
    }

    fn inject_confidential_registry_param(
        instructions_list: &mut Vec<Vec<InstructionBox>>,
        manual_parameters: &[Parameter],
        vk_set_hash: Option<[u8; 32]>,
    ) {
        if manual_parameters.iter().any(|param| {
            matches!(
                param,
                Parameter::Custom(custom)
                    if custom.id() == &confidential_metadata::registry_root_id()
            )
        }) {
            return;
        }
        let already_present = instructions_list.iter().flatten().any(|instr| {
            instr
                .as_any()
                .downcast_ref::<SetParameter>()
                .and_then(|set| {
                    if let Parameter::Custom(custom) = set.inner() {
                        (custom.id() == &confidential_metadata::registry_root_id()).then_some(())
                    } else {
                        None
                    }
                })
                .is_some()
        });
        if already_present {
            return;
        }

        let mut meta_fields = norito::json::Map::new();
        let hash_field = vk_set_hash.map_or(norito::json::Value::Null, |hash| {
            let encoded = format!("0x{}", hex::encode(hash));
            norito::json::Value::String(encoded)
        });
        meta_fields.insert("vk_set_hash".to_string(), hash_field);
        let meta_value = norito::json::Value::Object(meta_fields);
        let param = Parameter::Custom(CustomParameter::new(
            confidential_metadata::registry_root_id(),
            Json::new(meta_value),
        ));
        instructions_list.push(vec![InstructionBox::from(SetParameter::new(param))]);
    }

    fn inject_crypto_manifest_param(
        instructions_list: &mut Vec<Vec<InstructionBox>>,
        manual_parameters: &[Parameter],
        crypto: &ManifestCrypto,
    ) -> eyre::Result<()> {
        let meta_id = crypto_metadata::manifest_meta_id();
        let ensure_matches = |existing: &CustomParameter| -> eyre::Result<()> {
            let observed: ManifestCrypto = existing
                .payload()
                .try_into_any()
                .map_err(|err| eyre!("failed to decode crypto manifest payload: {err}"))?;
            if &observed != crypto {
                return Err(eyre!(
                    "crypto manifest payload in genesis differs from advertised `crypto` block"
                ));
            }
            Ok(())
        };

        for param in manual_parameters {
            if let Parameter::Custom(custom) = param
                && custom.id() == &meta_id
            {
                return ensure_matches(custom);
            }
        }

        for existing in instructions_list
            .iter()
            .flatten()
            .filter_map(|instr| instr.as_any().downcast_ref::<SetParameter>())
        {
            if let Parameter::Custom(custom) = existing.inner()
                && custom.id() == &meta_id
            {
                return ensure_matches(custom);
            }
        }

        let mut payload_map = norito::json::Map::new();
        payload_map.insert(
            "sm_openssl_preview".to_string(),
            norito::json::Value::Bool(crypto.sm_openssl_preview),
        );
        payload_map.insert(
            "default_hash".to_string(),
            norito::json::Value::String(crypto.default_hash.clone()),
        );
        payload_map.insert(
            "sm2_distid_default".to_string(),
            norito::json::Value::String(crypto.sm2_distid_default.clone()),
        );
        payload_map.insert(
            "allowed_curve_ids".to_string(),
            norito::json::Value::Array(
                crypto
                    .allowed_curve_ids
                    .iter()
                    .copied()
                    .map(|n| norito::json::Value::Number(u64::from(n).into()))
                    .collect(),
            ),
        );
        payload_map.insert(
            "allowed_signing".to_string(),
            norito::json::Value::Array(
                crypto
                    .allowed_signing
                    .iter()
                    .map(|algo| norito::json::Value::String(algo.as_static_str().to_string()))
                    .collect(),
            ),
        );
        let payload = norito::json::Value::Object(payload_map);
        let param = Parameter::Custom(CustomParameter::new(meta_id, Json::new(payload)));
        instructions_list.push(vec![InstructionBox::from(SetParameter::new(param))]);
        Ok(())
    }

    fn build_consensus_meta_instructions(
        consensus_mode: Option<SumeragiConsensusMode>,
        bls_domain: Option<String>,
        wire_proto_versions: Vec<u32>,
        consensus_fingerprint: Option<String>,
        staged_next_mode: Option<SumeragiConsensusMode>,
        activation_height: Option<u64>,
        manual_parameters: &[Parameter],
    ) -> Result<Vec<InstructionBox>> {
        let mut instructions = Vec::new();

        let mut staged_next_mode = staged_next_mode.or_else(|| {
            manual_parameters.iter().find_map(|param| {
                if let Parameter::Sumeragi(SumeragiParameter::NextMode(mode)) = param {
                    Some(*mode)
                } else {
                    None
                }
            })
        });
        let activation_height = activation_height.or_else(|| {
            manual_parameters.iter().find_map(|param| {
                if let Parameter::Sumeragi(SumeragiParameter::ModeActivationHeight(height)) = param
                {
                    Some(*height)
                } else {
                    None
                }
            })
        });
        if activation_height.is_none() {
            if let Some(next) = staged_next_mode {
                if Some(next) == consensus_mode {
                    staged_next_mode = None;
                }
            }
        }
        if staged_next_mode.is_some() ^ activation_height.is_some() {
            return Err(eyre!(
                "consensus mode staging requires both `NextMode` and `ModeActivationHeight` to be set in the same block"
            ));
        }

        let resolved_mode = consensus_mode
            .or(staged_next_mode)
            .unwrap_or(SumeragiConsensusMode::Permissioned);

        let bls_domain = bls_domain.ok_or_else(|| {
            eyre!(
                "genesis manifest missing `bls_domain`; call `with_consensus_meta` before signing"
            )
        })?;
        let versions = if wire_proto_versions.is_empty() {
            return Err(eyre!(
                "genesis manifest missing `wire_proto_versions`; call `with_consensus_meta` before signing"
            ));
        } else {
            wire_proto_versions
        };
        let fingerprint = consensus_fingerprint.ok_or_else(|| {
            eyre!(
                "genesis manifest missing `consensus_fingerprint`; call `with_consensus_meta` before signing"
            )
        })?;
        let mode_str = match resolved_mode {
            SumeragiConsensusMode::Permissioned => "Permissioned",
            SumeragiConsensusMode::Npos => "Npos",
        };

        let mut meta_fields = norito::json::Map::new();
        meta_fields.insert(
            "mode".to_string(),
            norito::json::Value::String(mode_str.to_string()),
        );
        meta_fields.insert(
            "bls_domain".to_string(),
            norito::json::Value::String(bls_domain),
        );
        meta_fields.insert(
            "wire_proto_versions".to_string(),
            norito::json::value::to_value(&versions)
                .expect("serialize wire_proto_versions to JSON"),
        );
        meta_fields.insert(
            "consensus_fingerprint".to_string(),
            norito::json::Value::String(fingerprint),
        );
        let meta_value = norito::json::Value::Object(meta_fields);
        let handshake_payload = Json::from_norito_value_ref(&meta_value)
            .expect("handshake metadata JSON must serialize");
        let handshake_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            handshake_payload,
        ));
        if !manual_parameters
            .iter()
            .any(|existing| existing == &handshake_param)
        {
            instructions.push(InstructionBox::from(SetParameter::new(handshake_param)));
        }

        Ok(instructions)
    }
}

impl norito::json::JsonDeserialize for RawGenesisTransaction {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        Self::from_json_value(value)
    }
}

/// Builder to build [`RawGenesisTransaction`] and [`GenesisBlock`].
/// No guarantee of validity of the built genesis transactions and block.
#[must_use]
pub struct GenesisBuilder {
    chain: ChainId,
    executor: Option<IvmPath>,
    ivm_dir: PathBuf,
    transactions: Vec<GenesisTxBuilder>,
    crypto: ManifestCrypto,
    da_proof_policies: Option<DaProofPolicyBundle>,
    consensus_mode: Option<iroha_data_model::parameter::system::SumeragiConsensusMode>,
    bls_domain: Option<String>,
    wire_proto_versions: Vec<u32>,
    consensus_fingerprint: Option<String>,
}

/// Domain editing mode of the [`GenesisBuilder`] to register accounts and assets under the domain.
#[must_use]
pub struct GenesisDomainBuilder {
    chain: ChainId,
    executor: Option<IvmPath>,
    ivm_dir: PathBuf,
    transactions: Vec<GenesisTxBuilder>,
    domain_id: DomainId,
    crypto: ManifestCrypto,
    da_proof_policies: Option<DaProofPolicyBundle>,
    consensus_mode: Option<iroha_data_model::parameter::system::SumeragiConsensusMode>,
    bls_domain: Option<String>,
    wire_proto_versions: Vec<u32>,
    consensus_fingerprint: Option<String>,
}

#[derive(Default)]
struct GenesisTxBuilder {
    parameters: Vec<Parameter>,
    instructions: Vec<InstructionBox>,
    ivm_triggers: Vec<GenesisIvmTrigger>,
    topology: Vec<GenesisTopologyEntry>,
}

impl GenesisBuilder {
    /// Construct [`GenesisBuilder`] with an executor upgrade.
    pub fn new(chain: ChainId, executor: impl Into<PathBuf>, ivm_dir: impl Into<PathBuf>) -> Self {
        Self {
            chain,
            executor: Some(executor.into().into()),
            ivm_dir: ivm_dir.into(),
            transactions: vec![GenesisTxBuilder::default()],
            crypto: ManifestCrypto::default(),
            da_proof_policies: None,
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: Vec::new(),
            consensus_fingerprint: None,
        }
    }

    /// Construct [`GenesisBuilder`] without an executor upgrade.
    pub fn new_without_executor(chain: ChainId, ivm_dir: impl Into<PathBuf>) -> Self {
        Self {
            chain,
            executor: None,
            ivm_dir: ivm_dir.into(),
            transactions: vec![GenesisTxBuilder::default()],
            crypto: ManifestCrypto::default(),
            da_proof_policies: None,
            consensus_mode: None,
            bls_domain: None,
            wire_proto_versions: Vec::new(),
            consensus_fingerprint: None,
        }
    }

    /// Override the cryptography snapshot advertised alongside the manifest.
    pub fn with_crypto(mut self, crypto: ManifestCrypto) -> Self {
        self.crypto = crypto;
        self
    }

    /// Override the DA proof policy bundle embedded into genesis.
    pub fn with_da_proof_policies(mut self, policies: DaProofPolicyBundle) -> Self {
        self.da_proof_policies = Some(policies);
        self
    }

    fn current_tx_mut(&mut self) -> &mut GenesisTxBuilder {
        self.transactions
            .last_mut()
            .expect("at least one transaction exists")
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

        self.current_tx_mut()
            .instructions
            .push(Register::domain(new_domain).into());

        GenesisDomainBuilder {
            chain: self.chain,
            executor: self.executor,
            ivm_dir: self.ivm_dir,
            transactions: self.transactions,
            domain_id,
            crypto: self.crypto,
            da_proof_policies: self.da_proof_policies,
            consensus_mode: self.consensus_mode,
            bls_domain: self.bls_domain,
            wire_proto_versions: self.wire_proto_versions,
            consensus_fingerprint: self.consensus_fingerprint,
        }
    }

    /// Entry a parameter setting to the end of entries.
    pub fn append_parameter(mut self, parameter: Parameter) -> Self {
        self.current_tx_mut().parameters.push(parameter);
        self
    }

    /// Entry a instruction to the end of entries.
    pub fn append_instruction(mut self, instruction: impl Into<InstructionBox>) -> Self {
        self.current_tx_mut().instructions.push(instruction.into());
        self
    }

    /// Entry an IVM trigger to the end of entries.
    pub fn append_ivm_trigger(mut self, ivm_trigger: GenesisIvmTrigger) -> Self {
        self.current_tx_mut().ivm_triggers.push(ivm_trigger);
        self
    }

    /// Overwrite the initial topology of the current transaction.
    pub fn set_topology<T: Into<GenesisTopologyEntry>>(mut self, topology: Vec<T>) -> Self {
        self.current_tx_mut().topology = topology.into_iter().map(Into::into).collect();
        self
    }

    /// Merge PoPs into the topology entries of the current transaction.
    ///
    /// # Panics
    ///
    /// Panics if the input contains duplicate peers or peers not present in the topology.
    pub fn set_topology_pop(mut self, topology_pop: Vec<GenesisPeerPop>) -> Self {
        if topology_pop.is_empty() {
            return self;
        }
        let mut pop_map = BTreeMap::new();
        for GenesisPeerPop { public_key, pop } in topology_pop {
            assert!(
                !pop_map.contains_key(&public_key),
                "duplicate topology pop entry for peer {public_key}"
            );
            pop_map.insert(public_key, pop);
        }
        let tx = self.current_tx_mut();
        for entry in &mut tx.topology {
            if let Some(pop) = pop_map.remove(entry.peer.public_key()) {
                entry.pop_hex = Some(hex::encode(pop));
            }
        }
        if let Some(pk) = pop_map.keys().next() {
            panic!("topology pop entry provided for peer {pk} missing from topology");
        }
        self
    }

    /// Start a new empty transaction.
    pub fn next_transaction(mut self) -> Self {
        self.transactions.push(GenesisTxBuilder::default());
        self
    }

    /// Finish building, sign, and produce a [`GenesisBlock`].
    ///
    /// # Errors
    ///
    /// Fails if internal [`RawGenesisTransaction::build_and_sign`] fails.
    pub fn build_and_sign(self, genesis_key_pair: &KeyPair) -> Result<GenesisBlock> {
        let da_proof_policies = self.da_proof_policies.clone();
        self.build_raw()
            .build_and_sign_with_da_proof_policies(genesis_key_pair, da_proof_policies)
    }

    /// Finish building and produce a [`RawGenesisTransaction`].
    pub fn build_raw(self) -> RawGenesisTransaction {
        let transactions = self
            .transactions
            .into_iter()
            .map(|tx| {
                let parameters =
                    (!tx.parameters.is_empty()).then(|| tx.parameters.into_iter().collect());
                RawGenesisTx {
                    parameters,
                    instructions: tx.instructions,
                    ivm_triggers: tx.ivm_triggers,
                    topology: tx.topology,
                }
            })
            .collect();

        RawGenesisTransaction {
            chain: self.chain,
            executor: self.executor,
            ivm_dir: self.ivm_dir.into(),
            transactions,
            consensus_mode: self.consensus_mode,
            bls_domain: self.bls_domain,
            wire_proto_versions: self.wire_proto_versions,
            consensus_fingerprint: self.consensus_fingerprint,
            crypto: self.crypto,
        }
    }
}

impl GenesisDomainBuilder {
    /// Finish this domain and return to genesis block building.
    pub fn finish_domain(self) -> GenesisBuilder {
        GenesisBuilder {
            chain: self.chain,
            executor: self.executor,
            ivm_dir: self.ivm_dir,
            transactions: self.transactions,
            crypto: self.crypto,
            da_proof_policies: self.da_proof_policies,
            consensus_mode: self.consensus_mode,
            bls_domain: self.bls_domain,
            wire_proto_versions: self.wire_proto_versions,
            consensus_fingerprint: self.consensus_fingerprint,
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
        self.current_tx_mut().instructions.push(register.into());
        self
    }

    /// Add [`AssetDefinition`] to this domain.
    pub fn asset(mut self, asset_name: Name, asset_spec: NumericSpec) -> Self {
        let asset_definition_id = AssetDefinitionId::new(self.domain_id.clone(), asset_name);
        let asset_definition = AssetDefinition::new(asset_definition_id, asset_spec);
        self.current_tx_mut()
            .instructions
            .push(Register::asset_definition(asset_definition).into());
        self
    }

    fn current_tx_mut(&mut self) -> &mut GenesisTxBuilder {
        self.transactions
            .last_mut()
            .expect("at least one transaction exists")
    }
}

// Encode/Decode are provided generically by `norito` for any type that implements
// `NoritoSerialize`/`NoritoDeserialize`, so no explicit impls are needed here.

// Provide Norito core serialization so `IvmPath` can participate in
// derive(Encode, Decode) on containing types.
impl norito::core::NoritoSerialize for IvmPath {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let s = self.0.to_str().expect("path contains not valid UTF-8");
        norito::core::NoritoSerialize::serialize(&s, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for IvmPath {
    fn deserialize(archived: &'a norito::core::Archived<IvmPath>) -> Self {
        let s: String = norito::core::NoritoDeserialize::deserialize(archived.cast());
        IvmPath(PathBuf::from(s))
    }
}

impl From<PathBuf> for IvmPath {
    fn from(value: PathBuf) -> Self {
        Self(value)
    }
}

impl TryFrom<IvmPath> for IvmBytecode {
    type Error = eyre::Report;

    fn try_from(value: IvmPath) -> Result<Self, Self::Error> {
        let blob = fs::read(&value.0)
            .wrap_err_with(|| eyre!("failed to read bytecode from {}", value.0.display()))?;

        Ok(IvmBytecode::from_compiled(blob))
    }
}

impl IvmPath {
    /// Resolve `self` to `here/self`,
    /// assuming `self` is an unresolved relative path to `here`.
    /// In case `self` is absolute, it replaces `here` i.e. this method mutates nothing.
    fn resolve(&mut self, here: impl AsRef<Path>) {
        self.0 = here.as_ref().join(&self.0)
    }
}

impl norito::json::FastJsonWrite for IvmPath {
    fn write_json(&self, out: &mut String) {
        let value = self.0.to_str().expect("path contains not valid UTF-8");
        norito::json::JsonSerialize::json_serialize(value, out);
    }
}

impl norito::json::JsonDeserialize for IvmPath {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let raw = parser.parse_string()?;
        Ok(Self(PathBuf::from(raw)))
    }
}

/// Human-readable alternative to [`Trigger`] whose action executes IVM
/// bytecode instead of a native instruction sequence.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, IntoSchema, Encode, Decode, Constructor)]
pub struct GenesisIvmTrigger {
    /// Unique trigger identifier.
    id: TriggerId,
    /// Action describing executable, repeats, authority and filter.
    action: GenesisIvmAction,
}

/// Human-readable alternative to [`Action`] which contains IVM bytecode as the
/// executable payload.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, IntoSchema, Encode, Decode)]
pub struct GenesisIvmAction {
    /// Path to the compiled IVM bytecode (`.to`) file.
    executable: IvmPath,
    /// Trigger repetition policy.
    repeats: Repeats,
    /// Account authorized to trigger execution.
    authority: AccountId,
    /// Event filter selecting which events cause the trigger to fire.
    filter: EventFilterBox,
}

impl GenesisIvmAction {
    /// Construct [`GenesisIvmAction`]
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

impl TryFrom<GenesisIvmTrigger> for Trigger {
    type Error = eyre::Report;

    fn try_from(value: GenesisIvmTrigger) -> Result<Self, Self::Error> {
        Ok(Trigger::new(value.id, value.action.try_into()?))
    }
}

// Enable packed-sequence decoding of genesis triggers under Norito by
// delegating slice-based decoding to the regular codec decoder. This avoids
// duplicating decode logic and keeps behavior consistent.
impl<'a> norito::core::DecodeFromSlice<'a> for GenesisIvmTrigger {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        Ok((v, bytes.len()))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for GenesisIvmAction {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        Ok((v, bytes.len()))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for IvmPath {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        Ok((v, bytes.len()))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RawGenesisTx {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        Ok((v, bytes.len()))
    }
}

impl TryFrom<GenesisIvmAction> for Action {
    type Error = eyre::Report;

    fn try_from(value: GenesisIvmAction) -> Result<Self, Self::Error> {
        Ok(Action::new(
            IvmBytecode::try_from(value.executable)?,
            value.repeats,
            value.authority,
            value.filter,
        ))
    }
}

#[cfg(test)]
mod tests {
    use eyre::Result;
    use iroha_data_model::{
        block::SignedBlock,
        isi::SetParameter,
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, confidential_metadata, consensus_metadata},
        },
        transaction::Executable,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_KEYPAIR, BOB_KEYPAIR};
    use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
    use tempfile::TempDir;

    use super::*;

    fn test_builder() -> (TempDir, GenesisBuilder) {
        let tmp_dir = TempDir::new().unwrap();
        let dummy_bytecode = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let executor_path = tmp_dir.path().join("executor.to");
        std::fs::write(&executor_path, dummy_bytecode).unwrap();
        let chain = ChainId::from("00000000-0000-0000-0000-000000000000");
        let ivm_dir = tmp_dir.path().join("ivm/");
        let builder = GenesisBuilder::new(chain, executor_path, ivm_dir);

        (tmp_dir, builder)
    }

    #[test]
    fn parse_without_optional_fields() -> Result<()> {
        let tmp_dir = TempDir::new().unwrap();
        let dummy_bytecode = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let executor_path = tmp_dir.path().join("executor.to");
        std::fs::write(&executor_path, dummy_bytecode).unwrap();
        let genesis = format!(
            r#"{{"chain":"00000000-0000-0000-0000-000000000000","executor":"{}","consensus_mode":"Permissioned","transactions":[{{}}]}}"#,
            executor_path.file_name().unwrap().to_str().unwrap()
        );
        let genesis_path = tmp_dir.path().join("genesis.json");
        std::fs::write(&genesis_path, genesis).unwrap();
        let kp = KeyPair::random();
        RawGenesisTransaction::from_path(&genesis_path)?.build_and_sign(&kp)?;
        Ok(())
    }

    #[test]
    fn parse_genesis_accepts_structured_accounts_without_selector_bootstrap() -> Result<()> {
        init_instruction_registry();

        let (tmp_dir, builder) = test_builder();
        let (public_key, _) = KeyPair::random().into_parts();
        let domain_name: Name = "wonderland".parse()?;
        let domain_id = DomainId::new(domain_name.clone());
        let account_id = AccountId::new(domain_id, public_key.clone());

        let genesis = builder
            .domain(domain_name)
            .account(public_key)
            .finish_domain()
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned);
        let json = norito::json::to_json_pretty(&genesis)?;
        assert!(
            json.contains(&account_id.to_string()),
            "expected IH58 account id in genesis JSON"
        );
        let genesis_path = tmp_dir.path().join("genesis.json");
        std::fs::write(&genesis_path, json)?;
        RawGenesisTransaction::from_path(&genesis_path)?;
        Ok(())
    }

    #[test]
    fn build_and_sign_refreshes_stale_consensus_fingerprint() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("iroha:test:refresh-consensus-fp");
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        let expected = manifest
            .clone()
            .with_consensus_meta()
            .consensus_fingerprint
            .clone()
            .expect("expected consensus fingerprint");
        manifest.consensus_fingerprint = Some("0xdeadbeef".to_string());

        let genesis = manifest.build_and_sign(&KeyPair::random())?;
        let mut found = None;
        for tx in genesis.0.external_transactions() {
            if let Executable::Instructions(batch) = tx.instructions() {
                for instr in batch {
                    if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                        && let Parameter::Custom(custom) = set_param.inner()
                        && custom.id() == &consensus_metadata::handshake_meta_id()
                    {
                        let payload: norito::json::Value = custom
                            .payload()
                            .try_into_any_norito()
                            .expect("decode handshake metadata payload");
                        if let Some(norito::json::Value::String(fp)) =
                            payload.get("consensus_fingerprint")
                        {
                            found = Some(fp.clone());
                            break;
                        }
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        let got = found.expect("consensus_handshake_meta not found");
        assert_eq!(got, expected);
        Ok(())
    }

    #[test]
    fn raw_genesis_tx_parameters_json_serializes() {
        let tx = RawGenesisTx {
            parameters: Some(Parameters::default()),
            ..RawGenesisTx::default()
        };
        let json = norito::json::to_json(&tx).expect("serialize raw genesis tx");
        let value = norito::json::parse_value(&json).expect("parse raw genesis tx json");
        let obj = value
            .as_object()
            .expect("raw genesis tx must serialize to an object");
        assert!(
            obj.get("parameters").is_some(),
            "parameters must be present when provided"
        );
    }

    #[test]
    fn default_genesis_omits_set_parameter_instructions() -> Result<()> {
        let genesis_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");
        init_instruction_registry();
        let genesis = RawGenesisTransaction::from_path(&genesis_path)?;
        assert!(!genesis.transactions.is_empty());
        assert!(
            genesis
                .transactions
                .iter()
                .any(|tx| tx.parameters.is_some()),
            "default genesis should seed parameters in the structured block"
        );
        assert!(
            genesis
                .transactions
                .iter()
                .flat_map(|tx| &tx.instructions)
                .all(|instr| instr.as_any().downcast_ref::<SetParameter>().is_none()),
            "manifest instructions must not include SetParameter"
        );
        Ok(())
    }

    #[test]
    fn set_topology_pop_merges_entries() {
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let pop =
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation");
        let peer = PeerId::new(bls.public_key().clone());
        let manifest =
            GenesisBuilder::new_without_executor(ChainId::from("test-topology-pop"), ".")
                .set_topology(vec![peer.clone()])
                .set_topology_pop(vec![GenesisPeerPop {
                    public_key: peer.public_key().clone(),
                    pop: pop.clone(),
                }])
                .build_raw();
        let tx = &manifest.transactions()[0];
        assert_eq!(tx.topology().len(), 1);
        assert_eq!(tx.topology()[0].peer, peer);
        let expected = hex::encode(pop);
        assert_eq!(tx.topology()[0].pop_hex.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn parse_injects_register_peer_with_pop() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-chain");
        let (peer_pk, _) = KeyPair::random().into_parts();
        let peer_id = PeerId::from(peer_pk.clone());
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .set_topology(vec![GenesisTopologyEntry::new(
                peer_id.clone(),
                vec![1, 2, 3, 4],
            )])
            .build_raw()
            .with_consensus_meta();
        let batches = manifest.parse()?;
        let registers: Vec<_> = batches
            .into_iter()
            .flatten()
            .filter_map(|instr| {
                instr
                    .as_any()
                    .downcast_ref::<RegisterBox>()
                    .and_then(|register| match register {
                        RegisterBox::Peer(inner) => Some(inner.clone()),
                        _ => None,
                    })
            })
            .collect();
        assert_eq!(registers.len(), 1);
        assert_eq!(registers[0].peer, peer_id);
        assert_eq!(registers[0].pop, vec![1, 2, 3, 4]);
        Ok(())
    }

    #[test]
    fn parse_errors_when_pop_missing() {
        init_instruction_registry();
        let chain = ChainId::from("test-pop-missing");
        let (peer_pk, _) = KeyPair::random().into_parts();
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .set_topology(vec![GenesisTopologyEntry::from(PeerId::from(peer_pk))])
            .build_raw()
            .with_consensus_meta();
        let err = manifest.parse().expect_err("missing pop must error");
        assert!(
            err.to_string()
                .contains("missing `pop_hex` entry for topology peer"),
            "{err}"
        );
    }

    #[test]
    fn parse_injects_consensus_handshake_metadata() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta");
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        let batches = manifest.parse()?;
        let mut found = false;
        for instr in batches.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
            {
                found = true;
                break;
            }
        }
        assert!(found, "consensus handshake metadata parameter not found");
        Ok(())
    }

    #[test]
    fn parse_replaces_stale_consensus_handshake_metadata() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-replace");
        let expected_fingerprint = GenesisBuilder::new_without_executor(chain.clone(), ".")
            .build_raw()
            .with_consensus_meta()
            .consensus_fingerprint
            .expect("consensus fingerprint expected");

        let stale_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "bls_domain".to_string(),
                    norito::json::Value::String("bls:stale-domain".to_string()),
                );
                payload.insert(
                    "wire_proto_versions".to_string(),
                    norito::json::to_value(&vec![1u32]).expect("serialize proto versions"),
                );
                payload.insert(
                    "consensus_fingerprint".to_string(),
                    norito::json::Value::String("0x0000bad".to_string()),
                );
                payload
            }))
            .expect("construct stale handshake payload"),
        ));
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        manifest
            .transactions
            .first_mut()
            .expect("missing manifest transaction")
            .instructions
            .push(InstructionBox::from(SetParameter::new(stale_param)));

        let mut found = Vec::new();
        for instr in manifest.parse()?.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
                && let Ok(payload) = custom
                    .payload()
                    .try_into_any_norito::<norito::json::Value>()
            {
                if let Some(fingerprint) =
                    payload
                        .get("consensus_fingerprint")
                        .and_then(|value: &norito::json::Value| {
                            value.as_str().map(std::string::ToString::to_string)
                        })
                {
                    found.push(fingerprint);
                }
            }
        }
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], expected_fingerprint);
        Ok(())
    }

    #[test]
    fn parse_replaces_stale_consensus_handshake_metadata_in_parameters() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-replace-params");
        let expected_fingerprint = GenesisBuilder::new_without_executor(chain.clone(), ".")
            .build_raw()
            .with_consensus_meta()
            .consensus_fingerprint
            .expect("consensus fingerprint expected");

        let stale_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "bls_domain".to_string(),
                    norito::json::Value::String("bls:stale-domain".to_string()),
                );
                payload.insert(
                    "wire_proto_versions".to_string(),
                    norito::json::to_value(&vec![1u32]).expect("serialize proto versions"),
                );
                payload.insert(
                    "consensus_fingerprint".to_string(),
                    norito::json::Value::String("0x0000bad".to_string()),
                );
                payload
            }))
            .expect("construct stale handshake payload"),
        ));
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        let mut parameters = Parameters::default();
        parameters.set_parameter(stale_param);
        manifest
            .transactions
            .first_mut()
            .expect("missing manifest transaction")
            .parameters = Some(parameters);

        let mut found = Vec::new();
        for instr in manifest.parse()?.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
                && let Ok(payload) = custom
                    .payload()
                    .try_into_any_norito::<norito::json::Value>()
            {
                if let Some(fingerprint) =
                    payload
                        .get("consensus_fingerprint")
                        .and_then(|value: &norito::json::Value| {
                            value.as_str().map(std::string::ToString::to_string)
                        })
                {
                    found.push(fingerprint);
                }
            }
        }
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], expected_fingerprint);
        Ok(())
    }

    #[test]
    fn parse_preserves_valid_consensus_handshake_metadata() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-preserve-valid");
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        manifest.consensus_mode = Some(SumeragiConsensusMode::Permissioned);
        manifest.bls_domain = Some("bls:override:mode".to_string());
        manifest.wire_proto_versions = vec![7];
        let expected_fingerprint = manifest
            .clone()
            .with_consensus_meta()
            .consensus_fingerprint
            .expect("consensus fingerprint expected");
        let explicit_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "bls_domain".to_string(),
                    norito::json::Value::String("bls:override:mode".to_string()),
                );
                payload.insert(
                    "wire_proto_versions".to_string(),
                    norito::json::to_value(&vec![7u32]).expect("serialize proto versions"),
                );
                payload.insert(
                    "consensus_fingerprint".to_string(),
                    norito::json::Value::String(expected_fingerprint.clone()),
                );
                payload
            }))
            .expect("construct handshake payload"),
        ));
        manifest
            .transactions
            .first_mut()
            .expect("missing manifest transaction")
            .instructions
            .push(InstructionBox::from(SetParameter::new(explicit_param)));

        let mut found = Vec::new();
        for instr in manifest.parse()?.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
                && let Ok(payload) = custom
                    .payload()
                    .try_into_any_norito::<norito::json::Value>()
            {
                found.push(payload);
            }
        }
        assert_eq!(found.len(), 1);
        let payload = found.remove(0);
        assert_eq!(
            payload.get("mode").and_then(norito::json::Value::as_str),
            Some("Permissioned")
        );
        assert_eq!(
            payload
                .get("bls_domain")
                .and_then(norito::json::Value::as_str),
            Some("bls:override:mode")
        );
        assert_eq!(
            payload
                .get("wire_proto_versions")
                .and_then(norito::json::Value::as_array)
                .expect("wire_proto_versions should be encoded"),
            &vec![norito::json::Value::Number(7u64.into())]
        );
        assert_eq!(
            payload
                .get("consensus_fingerprint")
                .and_then(norito::json::Value::as_str),
            Some(expected_fingerprint.as_str())
        );
        Ok(())
    }

    #[test]
    fn parse_preserves_explicit_consensus_handshake_metadata_with_external_fingerprint()
    -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-preserve-external-fingerprint");
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        let external_fingerprint =
            "0x1111111111111111111111111111111111111111111111111111111111111111";
        let explicit_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Npos".to_string()),
                );
                payload.insert(
                    "bls_domain".to_string(),
                    norito::json::Value::String("bls-iroha2:npos-sumeragi:v1".to_string()),
                );
                payload.insert(
                    "wire_proto_versions".to_string(),
                    norito::json::to_value(&vec![1u32]).expect("serialize proto versions"),
                );
                payload.insert(
                    "consensus_fingerprint".to_string(),
                    norito::json::Value::String(external_fingerprint.to_string()),
                );
                payload
            }))
            .expect("construct handshake payload"),
        ));
        manifest
            .transactions
            .first_mut()
            .expect("missing manifest transaction")
            .instructions
            .push(InstructionBox::from(SetParameter::new(explicit_param)));

        let mut found = Vec::new();
        for instr in manifest.parse()?.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
                && let Ok(payload) = custom
                    .payload()
                    .try_into_any_norito::<norito::json::Value>()
            {
                found.push(payload);
            }
        }
        assert_eq!(found.len(), 1);
        let payload = found.remove(0);
        assert_eq!(
            payload
                .get("consensus_fingerprint")
                .and_then(norito::json::Value::as_str),
            Some(external_fingerprint)
        );
        Ok(())
    }

    #[test]
    fn parse_preserves_valid_consensus_handshake_metadata_in_parameters() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-preserve-valid-params");
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        manifest.consensus_mode = Some(SumeragiConsensusMode::Permissioned);
        manifest.bls_domain = Some("bls:override:mode".to_string());
        manifest.wire_proto_versions = vec![7];
        let expected_fingerprint = manifest
            .clone()
            .with_consensus_meta()
            .consensus_fingerprint
            .expect("consensus fingerprint expected");
        let explicit_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "bls_domain".to_string(),
                    norito::json::Value::String("bls:override:mode".to_string()),
                );
                payload.insert(
                    "wire_proto_versions".to_string(),
                    norito::json::to_value(&vec![7u32]).expect("serialize proto versions"),
                );
                payload.insert(
                    "consensus_fingerprint".to_string(),
                    norito::json::Value::String(expected_fingerprint.clone()),
                );
                payload
            }))
            .expect("construct handshake payload"),
        ));
        let mut parameters = Parameters::default();
        parameters.set_parameter(explicit_param);
        manifest
            .transactions
            .first_mut()
            .expect("missing manifest transaction")
            .parameters = Some(parameters);

        let mut found = Vec::new();
        for instr in manifest.parse()?.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &consensus_metadata::handshake_meta_id()
                && let Ok(payload) = custom
                    .payload()
                    .try_into_any_norito::<norito::json::Value>()
            {
                found.push(payload);
            }
        }
        assert_eq!(found.len(), 1);
        let payload = found.remove(0);
        assert_eq!(
            payload.get("mode").and_then(norito::json::Value::as_str),
            Some("Permissioned")
        );
        assert_eq!(
            payload
                .get("bls_domain")
                .and_then(norito::json::Value::as_str),
            Some("bls:override:mode")
        );
        assert_eq!(
            payload
                .get("wire_proto_versions")
                .and_then(norito::json::Value::as_array)
                .expect("wire_proto_versions should be encoded"),
            &vec![norito::json::Value::Number(7u64.into())]
        );
        assert_eq!(
            payload
                .get("consensus_fingerprint")
                .and_then(norito::json::Value::as_str),
            Some(expected_fingerprint.as_str())
        );
        Ok(())
    }

    #[test]
    fn parse_injects_confidential_registry_root() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-confidential-meta");
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        let batches = manifest.parse()?;
        let mut found = false;
        for instr in batches.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &confidential_metadata::registry_root_id()
            {
                let value: norito::json::Value = custom
                    .payload()
                    .try_into_any_norito()
                    .expect("decode confidential registry payload");
                let vk_field = value.get("vk_set_hash");
                assert!(
                    matches!(vk_field, Some(norito::json::Value::Null)),
                    "expected null vk_set_hash for empty registry, got {vk_field:?}"
                );
                found = true;
                break;
            }
        }
        assert!(found, "confidential registry root parameter not found");
        Ok(())
    }

    #[test]
    fn parse_injects_crypto_manifest_metadata() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-crypto-meta");
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        let expected_crypto = manifest.crypto().clone();
        let batches = manifest.parse()?;
        let mut found = None;
        for instr in batches.into_iter().flatten() {
            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                && let Parameter::Custom(custom) = set_param.inner()
                && custom.id() == &crypto_metadata::manifest_meta_id()
            {
                let value: ManifestCrypto = custom
                    .payload()
                    .try_into_any()
                    .expect("decode manifest crypto payload");
                found = Some(value);
                break;
            }
        }
        let found = found.expect("crypto manifest metadata parameter not found");
        assert_eq!(found, expected_crypto);
        Ok(())
    }

    #[test]
    fn parse_rejects_mismatched_crypto_manifest_metadata() {
        init_instruction_registry();
        let chain = ChainId::from("test-crypto-meta-mismatch");
        let mut wrong_crypto = ManifestCrypto::default();
        wrong_crypto.default_hash = "blake2b-512".to_owned();
        let payload =
            norito::json::value::to_value(&wrong_crypto).expect("serialize mismatched crypto");
        let manual_param = Parameter::Custom(CustomParameter::new(
            crypto_metadata::manifest_meta_id(),
            Json::new(payload),
        ));
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .append_parameter(manual_param)
            .build_raw()
            .with_consensus_meta();
        let err = manifest
            .parse()
            .expect_err("mismatched crypto metadata should be rejected");
        assert!(
            err.to_string()
                .contains("crypto manifest payload in genesis differs"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn parse_respects_manual_confidential_registry_root() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-confidential-manual");
        let manual = Parameter::Custom(CustomParameter::new(
            confidential_metadata::registry_root_id(),
            Json::new({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "vk_set_hash".to_string(),
                    norito::json::Value::String(
                        "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_string(),
                    ),
                );
                norito::json::Value::Object(payload)
            }),
        ));
        let manifest = GenesisBuilder::new_without_executor(chain, ".")
            .append_parameter(manual)
            .build_raw()
            .with_consensus_meta();
        let batches = manifest.parse()?;
        let count = batches
            .into_iter()
            .flatten()
            .filter(|instr| {
                instr
                    .as_any()
                    .downcast_ref::<SetParameter>()
                    .is_some_and(|set_param| {
                        matches!(
                            set_param.inner(),
                            Parameter::Custom(custom)
                                if custom.id() == &confidential_metadata::registry_root_id()
                        )
                    })
            })
            .count();
        assert_eq!(count, 1, "expected exactly one registry root parameter");
        Ok(())
    }

    #[test]
    fn load_new_genesis_block() -> Result<()> {
        let genesis_key_pair = KeyPair::random();
        let (alice_public_key, _) = KeyPair::random().into_parts();
        let (_tmp_dir, builder) = test_builder();

        let _genesis_block = builder
            .domain("wonderland".parse()?)
            .account(alice_public_key)
            .finish_domain()
            .build_and_sign(&genesis_key_pair)?;

        Ok(())
    }

    #[test]
    fn signed_block_versioned_roundtrip() -> Result<()> {
        init_instruction_registry();
        let genesis_key_pair = KeyPair::random();
        let (tmp_dir, builder) = test_builder();
        let _ = tmp_dir;
        let block = builder.build_and_sign(&genesis_key_pair)?;
        let encoded = block.0.encode_versioned();
        let decoded = SignedBlock::decode_all_versioned(&encoded)?;

        assert_eq!(
            decoded.external_transactions().count(),
            block.0.external_transactions().count()
        );

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
        let _executor_path = genesis_builder.executor.clone();

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

        // In real cases executor should be constructed from an IVM bytecode blob
        let finished_genesis = genesis_builder.build_and_sign(&KeyPair::random())?;

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
    fn roundtrip_raw_genesis_serialization() -> Result<()> {
        let (_tmp_dir, builder) = test_builder();
        let raw = builder
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned);
        let json = norito::json::to_json(&raw)?;
        let de: RawGenesisTransaction = norito::json::from_str(&json)?;
        let json2 = norito::json::to_json(&de)?;
        assert_eq!(json, json2);

        Ok(())
    }

    #[test]
    fn default_genesis_deserializes() {
        init_instruction_registry();
        let genesis_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");
        let result = RawGenesisTransaction::from_path(&genesis_path);
        assert!(result.is_ok());
    }

    #[test]
    fn default_genesis_block_roundtrips() -> Result<()> {
        use iroha_data_model::parameter::system::SumeragiNposParameters;

        init_instruction_registry();
        if norito::debug_trace_enabled() {
            // Debug tracing interferes with ConstVec decode guards; skip engineering checks in this mode.
            return Ok(());
        }
        let genesis_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");
        let genesis = RawGenesisTransaction::from_path(&genesis_path)?;

        let kp = KeyPair::random();
        let block = genesis.build_and_sign(&kp)?;

        let mut saw_handshake_mode = false;
        let mut saw_npos_custom = false;
        for tx in block.0.external_transactions() {
            if let iroha_data_model::transaction::Executable::Instructions(instrs) =
                tx.instructions()
            {
                for instr in instrs {
                    if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>() {
                        match set_param.inner() {
                            Parameter::Transaction(_) | Parameter::SmartContract(_) => {
                                panic!("unexpected high-level parameter instruction generated")
                            }
                            Parameter::Executor(_) => {
                                panic!("unexpected executor parameter instruction generated")
                            }
                            Parameter::Custom(custom)
                                if custom.id() == &consensus_metadata::handshake_meta_id() =>
                            {
                                let payload: norito::json::Value = custom
                                    .payload()
                                    .try_into_any_norito()
                                    .expect("decode handshake metadata payload");
                                let mode = payload
                                    .get("mode")
                                    .and_then(norito::json::Value::as_str)
                                    .expect("handshake metadata must carry mode");
                                assert_eq!(
                                    mode, "Npos",
                                    "Default genesis should advertise NPoS consensus mode"
                                );
                                saw_handshake_mode = true;
                            }
                            Parameter::Custom(custom)
                                if *custom.id() == SumeragiNposParameters::parameter_id() =>
                            {
                                saw_npos_custom = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        assert!(
            saw_handshake_mode,
            "Default genesis must emit SetParameter for consensus handshake metadata"
        );
        assert!(
            saw_npos_custom,
            "Default genesis must emit SetParameter for `sumeragi_npos_parameters`"
        );

        let encoded = block.0.encode_versioned();
        norito::core::reset_decode_state();
        let decoded = SignedBlock::decode_all_versioned(&encoded)
            .wrap_err("default genesis block should decode via canonical layout")?;
        assert_eq!(
            decoded, block.0,
            "Encoded + decoded default genesis block must preserve all fields"
        );

        Ok(())
    }

    #[test]
    fn instruction_registry_decodes_register_domain() {
        let registry = default_instruction_registry();
        let name = core::any::type_name::<Register<Domain>>();
        let instruction = Register::domain(Domain::new("test".parse().unwrap()));
        let bytes = norito::to_bytes(&instruction).expect("encode register-domain instruction");
        let decoded = registry.decode(name, &bytes).expect("entry");
        if let Err(err) = decoded {
            panic!("failed to decode register-domain instruction: {err}");
        }
    }

    #[test]
    fn uses_shared_instruction_registry() {
        let shared = iroha_data_model::instruction_registry::default();
        let local = default_instruction_registry();

        assert_eq!(local.len(), shared.len());
        for name in shared.names() {
            assert!(local.contains(name), "missing {name}");
        }
    }
}
