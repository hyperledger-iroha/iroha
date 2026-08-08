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
use core::num::NonZeroU64;
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    fmt::Debug,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
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
        consensus::{ConsensusGenesisModeParams, ConsensusGenesisParams, NposGenesisParams},
        consensus_v2::SumeragiV2GenesisContextParameters,
    },
    confidential::{
        ConfidentialFeatureDigest, ConfidentialStatus, DEFAULT_GENESIS_CONFIDENTIAL_POLICY_HASH,
    },
    da::commitment::DaProofPolicyBundle,
    isi::{
        InstructionRegistry, Register, SetParameter, register::RegisterPeerWithPop,
        set_instruction_registry, verifying_keys,
    },
    parameter::{
        Parameter,
        custom::CustomParameter,
        system::{
            ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode,
            SumeragiNposParameters, SumeragiParameters, confidential_metadata, consensus_metadata,
            crypto_metadata,
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

const CONSENSUS_PROTOCOL_VERSION: u32 =
    iroha_data_model::block::consensus_v2::PROTOCOL_VERSION as u32;

#[cfg(test)]
fn checked_genesis_fixture_keypair() -> KeyPair {
    KeyPair::try_random().expect("genesis fixture key generation should succeed")
}

#[cfg(test)]
fn checked_genesis_fixture_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm)
        .expect("genesis fixture key generation should succeed")
}

/// Domain of the genesis account, technically required for the pre-genesis state
pub static GENESIS_DOMAIN_ID: LazyLock<DomainId> =
    LazyLock::new(|| DomainId::parse_fully_qualified("genesis.universal").unwrap());

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
    /// Chain discriminant / i105 network prefix used to encode account literals in this manifest.
    chain_discriminant: u16,
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
    /// Consensus mode selected and signed by genesis.
    /// Fresh Sumeragi v2 startup consumes the corresponding signed handshake
    /// metadata and freezes this mode into the height-one context.
    consensus_mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    /// First-release consensus wire protocol version.
    wire_protocol_version: u32,
    /// Optional typed deterministic fingerprint of consensus parameters.
    #[norito(default)]
    consensus_fingerprint: Option<ConsensusFingerprint>,
    /// Genesis-selected Sumeragi v2 context parameters.
    ///
    /// JSON manifests must provide this explicitly. Programmatic builders put
    /// their selected profile here before signing; live nodes never infer it
    /// from local configuration.
    sumeragi_v2: SumeragiV2GenesisContextParameters,
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
        account::{NewAccount, OpaqueAccountId},
        asset::definition::NewAssetDefinition,
        domain::NewDomain,
        isi::{
            ActivatePublicLaneValidator, CustomInstruction, Grant, GrantBox, InstructionBox, Mint,
            MintBox, Register, RegisterPublicLaneValidator, SetAssetDefinitionAlias, SetParameter,
            Transfer, TransferBox,
            governance::RegisterCitizen,
            nexus::{
                ActivateFeeSponsorProgramRevision, CreateFeeSponsorProgram,
                EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram, StageFeeSponsorProgramRevision,
            },
            register::RegisterBox,
        },
        metadata::Metadata,
        nexus::{
            FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramRevision, LaneId,
            UniversalAccountId,
        },
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
            Value::String(encoded) => decode_base64_instruction(&encoded),
            Value::Object(map) => {
                if map.len() == 1 {
                    if let Some((kind, inner)) = map.iter().next() {
                        let decoded = match kind.as_str() {
                            "Register" => try_decode_register(inner.clone())?,
                            "Mint" => try_decode_mint(inner.clone())?,
                            "Transfer" => try_decode_transfer(inner.clone())?,
                            "SetParameter" => try_decode_set_parameter(inner.clone())?,
                            "Grant" => try_decode_grant(inner.clone())?,
                            "SetAssetDefinitionAlias" => {
                                try_decode_set_asset_definition_alias(inner.clone())?
                            }
                            "Custom" => try_decode_custom(inner.clone())?,
                            "RegisterCitizen" => try_decode_register_citizen(inner.clone())?,
                            "RegisterPublicLaneValidator" => {
                                try_decode_register_public_lane_validator(inner.clone())?
                            }
                            "ActivatePublicLaneValidator" => {
                                try_decode_activate_public_lane_validator(inner.clone())?
                            }
                            "CreateFeeSponsorProgram" => {
                                try_decode_create_fee_sponsor_program(inner.clone())?
                            }
                            "StageFeeSponsorProgramRevision" => {
                                try_decode_stage_fee_sponsor_program_revision(inner.clone())?
                            }
                            "EnrollFeeSponsorBeneficiary" => {
                                try_decode_enroll_fee_sponsor_beneficiary(inner.clone())?
                            }
                            "FundFeeSponsorProgram" => {
                                try_decode_fund_fee_sponsor_program(inner.clone())?
                            }
                            "ActivateFeeSponsorProgramRevision" => {
                                try_decode_activate_fee_sponsor_program_revision(inner.clone())?
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

    fn decode_base64_instruction(encoded: &str) -> Result<InstructionBox, json::Error> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|err| {
                json::Error::Message(format!("invalid base64 genesis instruction: {err}"))
            })?;
        norito::decode_canonical::<InstructionBox>(&bytes).map_err(|err| {
            json::Error::Message(format!(
                "failed to decode canonical base64 genesis instruction: {err}"
            ))
        })
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
                let mut fields = match payload {
                    Value::Object(map) => map,
                    other => {
                        return Err(json::Error::Message(format!(
                            "expected object for Register::Account fields, found {other:?}"
                        )));
                    }
                };
                let id = parse_account_id(&take_string(&mut fields, "id")?, "register account")?;
                let metadata = match fields.remove("metadata") {
                    None | Some(Value::Null) => Metadata::default(),
                    Some(value) => norito::json::value::from_value(value)?,
                };
                let label = match fields.remove("label") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(parse_account_alias(value, "Register.Account.label")?),
                };
                let uaid: Option<UniversalAccountId> = match fields.remove("uaid") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(norito::json::value::from_value(value)?),
                };
                let opaque_ids: Vec<OpaqueAccountId> = match fields.remove("opaque_ids") {
                    None | Some(Value::Null) => Vec::new(),
                    Some(value) => norito::json::value::from_value(value)?,
                };
                ensure_no_extra_fields(&fields)?;
                let new_account = NewAccount::new(id)
                    .with_metadata(metadata)
                    .with_label(label)
                    .with_uaid(uaid)
                    .with_opaque_ids(opaque_ids);
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
        let quantity =
            Quantity::try_from_numeric(parse_numeric(object_value)?).map_err(|error| {
                json::Error::Message(format!("invalid asset mint quantity: {error}"))
            })?;
        let instruction = InstructionBox::from(Mint::asset_quantity(quantity, asset_id));
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
                let domain = parse_domain_id(&domain_str, "domain")?;
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

    fn try_decode_set_asset_definition_alias(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = match inner {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        let asset_definition_id = match fields.remove("asset_definition_id") {
            Some(Value::String(value)) => AssetDefinitionId::from_str(&value).map_err(|err| {
                json::Error::Message(format!(
                    "invalid SetAssetDefinitionAlias.asset_definition_id `{value}`: {err}"
                ))
            })?,
            Some(other) => {
                return Err(json::Error::Message(format!(
                    "expected string for SetAssetDefinitionAlias.asset_definition_id, found {other:?}"
                )));
            }
            None => {
                return Err(json::Error::Message(
                    "missing SetAssetDefinitionAlias.asset_definition_id".to_string(),
                ));
            }
        };

        let alias = match fields.remove("alias") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) => Some(value.parse().map_err(|err| {
                json::Error::Message(format!(
                    "invalid SetAssetDefinitionAlias.alias `{value}`: {err}"
                ))
            })?),
            Some(other) => {
                return Err(json::Error::Message(format!(
                    "expected string or null for SetAssetDefinitionAlias.alias, found {other:?}"
                )));
            }
        };

        let lease_expiry_ms = match fields.remove("lease_expiry_ms") {
            None | Some(Value::Null) => None,
            Some(Value::Number(Number::U64(value))) => Some(value),
            Some(Value::Number(Number::I64(value))) if value >= 0 => Some(value.cast_unsigned()),
            Some(other) => {
                return Err(json::Error::Message(format!(
                    "expected unsigned integer or null for SetAssetDefinitionAlias.lease_expiry_ms, found {other:?}"
                )));
            }
        };

        if !fields.is_empty() {
            return Err(json::Error::Message(format!(
                "unexpected SetAssetDefinitionAlias fields: {}",
                fields.keys().cloned().collect::<Vec<_>>().join(",")
            )));
        }

        let instruction = match alias {
            Some(alias) => {
                SetAssetDefinitionAlias::bind(asset_definition_id, alias, lease_expiry_ms)
            }
            None => SetAssetDefinitionAlias::clear(asset_definition_id),
        };
        Ok(Some(InstructionBox::from(instruction)))
    }

    fn try_decode_custom(inner: Value) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = match inner {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        let payload = fields
            .remove("payload")
            .ok_or_else(|| json::Error::missing_field("payload"))?;
        ensure_no_extra_fields(&fields)?;
        Ok(Some(InstructionBox::from(CustomInstruction::new(
            iroha_primitives::json::Json::new(payload),
        ))))
    }

    fn try_decode_register_citizen(inner: Value) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = object_fields(inner, "RegisterCitizen")?;
        let owner = parse_account_id(&take_string(&mut fields, "owner")?, "RegisterCitizen owner")?;
        let amount = Quantity::try_from_numeric(parse_numeric(
            fields
                .remove("amount")
                .ok_or_else(|| json::Error::missing_field("amount"))?,
        )?)
        .map_err(|error| {
            json::Error::Message(format!("invalid RegisterCitizen amount: {error}"))
        })?;
        ensure_no_extra_fields(&fields)?;
        Ok(Some(InstructionBox::from(RegisterCitizen {
            owner,
            amount,
        })))
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
        let peer_id_str = take_string(&mut fields, "peer_id")?;
        let peer_id: PeerId = peer_id_str.parse().map_err(|_| {
            json::Error::Message(format!(
                "invalid peer id for RegisterPublicLaneValidator: {peer_id_str}"
            ))
        })?;
        let stake_account_str = take_string(&mut fields, "stake_account")?;
        let stake_account: AccountId = parse_account_id(&stake_account_str, "stake_account")?;
        let stake_value = fields
            .remove("initial_stake")
            .ok_or_else(|| json::Error::missing_field("initial_stake"))?;
        let initial_stake =
            Quantity::try_from_numeric(parse_numeric(stake_value)?).map_err(|error| {
                json::Error::Message(format!("invalid initial stake quantity: {error}"))
            })?;
        let metadata_value = fields.remove("metadata");
        let metadata = match metadata_value {
            Some(Value::Null) | None => Metadata::default(),
            Some(value) => norito::json::value::from_value(value)?,
        };
        ensure_no_extra_fields(&fields)?;
        let register = RegisterPublicLaneValidator::new(
            lane_id,
            validator,
            peer_id,
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

    fn object_fields(
        inner: Value,
        instruction: &str,
    ) -> Result<BTreeMap<String, Value>, json::Error> {
        match inner {
            Value::Object(fields) => Ok(fields),
            other => Err(json::Error::Message(format!(
                "expected object for {instruction} fields, found {other:?}"
            ))),
        }
    }

    fn take_typed<T>(
        fields: &mut BTreeMap<String, Value>,
        field: &'static str,
    ) -> Result<T, json::Error>
    where
        T: norito::json::JsonDeserialize,
    {
        norito::json::value::from_value(
            fields
                .remove(field)
                .ok_or_else(|| json::Error::missing_field(field))?,
        )
    }

    fn try_decode_create_fee_sponsor_program(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = object_fields(inner, "CreateFeeSponsorProgram")?;
        let program = take_typed::<FeeSponsorProgram>(&mut fields, "program")?;
        ensure_no_extra_fields(&fields)?;
        Ok(Some(InstructionBox::from(CreateFeeSponsorProgram {
            program,
        })))
    }

    fn try_decode_stage_fee_sponsor_program_revision(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = object_fields(inner, "StageFeeSponsorProgramRevision")?;
        let revision = take_typed::<FeeSponsorProgramRevision>(&mut fields, "revision")?;
        ensure_no_extra_fields(&fields)?;
        Ok(Some(InstructionBox::from(StageFeeSponsorProgramRevision {
            revision,
        })))
    }

    fn try_decode_enroll_fee_sponsor_beneficiary(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = object_fields(inner, "EnrollFeeSponsorBeneficiary")?;
        let program_id = take_typed::<FeeSponsorProgramId>(&mut fields, "program_id")?;
        let beneficiary = parse_account_id(
            &take_string(&mut fields, "beneficiary")?,
            "fee sponsor beneficiary",
        )?;
        ensure_no_extra_fields(&fields)?;
        Ok(Some(InstructionBox::from(EnrollFeeSponsorBeneficiary {
            program_id,
            beneficiary,
        })))
    }

    fn try_decode_fund_fee_sponsor_program(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = object_fields(inner, "FundFeeSponsorProgram")?;
        let program_id = take_typed::<FeeSponsorProgramId>(&mut fields, "program_id")?;
        let asset_definition_id =
            AssetDefinitionId::from_str(&take_string(&mut fields, "asset_definition_id")?)
                .map_err(|error| json::Error::Message(format!("invalid sponsor asset: {error}")))?;
        let amount = Quantity::try_from_numeric(parse_numeric(
            fields
                .remove("amount")
                .ok_or_else(|| json::Error::missing_field("amount"))?,
        )?)
        .map_err(|error| json::Error::Message(format!("invalid sponsor amount: {error}")))?;
        ensure_no_extra_fields(&fields)?;
        Ok(Some(InstructionBox::from(FundFeeSponsorProgram {
            program_id,
            asset_definition_id,
            amount,
        })))
    }

    fn try_decode_activate_fee_sponsor_program_revision(
        inner: Value,
    ) -> Result<Option<InstructionBox>, json::Error> {
        let mut fields = object_fields(inner, "ActivateFeeSponsorProgramRevision")?;
        let program_id = take_typed::<FeeSponsorProgramId>(&mut fields, "program_id")?;
        let revision = parse_u64(
            fields
                .remove("revision")
                .ok_or_else(|| json::Error::missing_field("revision"))?,
            "fee sponsor revision",
        )?;
        let activate_at_height = parse_u64(
            fields
                .remove("activate_at_height")
                .ok_or_else(|| json::Error::missing_field("activate_at_height"))?,
            "fee sponsor activation height",
        )?;
        ensure_no_extra_fields(&fields)?;
        Ok(Some(InstructionBox::from(
            ActivateFeeSponsorProgramRevision {
                program_id,
                revision,
                activate_at_height,
            },
        )))
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
        match AccountId::parse_encoded(value) {
            Ok(parsed) => Ok(parsed.into_account_id()),
            Err(err) => value
                .parse::<iroha_crypto::PublicKey>()
                .map(AccountId::new)
                .map_err(|_| json::Error::Message(format!("invalid {label}: {err}"))),
        }
    }

    fn parse_domain_id(value: &str, label: &'static str) -> Result<DomainId, json::Error> {
        DomainId::parse_fully_qualified(value)
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

    fn parse_u64(value: Value, label: &'static str) -> Result<u64, json::Error> {
        match value {
            Value::String(s) => s
                .parse::<u64>()
                .map_err(|err| json::Error::Message(format!("invalid {label}: {err}"))),
            Value::Number(Number::U64(value)) => Ok(value),
            Value::Number(Number::I64(value)) => u64::try_from(value)
                .map_err(|_| json::Error::Message(format!("invalid {label}: {value}"))),
            other => Err(json::Error::Message(format!(
                "expected numeric {label} value, found {other:?}"
            ))),
        }
    }

    fn parse_account_alias(
        value: Value,
        label: &'static str,
    ) -> Result<iroha_data_model::account::rekey::AccountAlias, json::Error> {
        let mut fields = match value {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for {label}, found {other:?}"
                )));
            }
        };

        let alias_label = match fields.remove("label") {
            Some(Value::String(value)) => value
                .parse()
                .map_err(|_| json::Error::Message(format!("invalid {label}.label `{value}`")))?,
            Some(other) => {
                return Err(json::Error::Message(format!(
                    "expected string for {label}.label, found {other:?}"
                )));
            }
            None => return Err(json::Error::Message(format!("missing {label}.label"))),
        };

        let domain = match fields.remove("domain") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) => Some(value.parse().map_err(|err| {
                json::Error::Message(format!("invalid {label}.domain `{value}`: {err}"))
            })?),
            Some(other) => {
                return Err(json::Error::Message(format!(
                    "expected string or null for {label}.domain, found {other:?}"
                )));
            }
        };

        let dataspace = match fields.remove("dataspace") {
            Some(value) => iroha_data_model::nexus::DataSpaceId::new(u64::from(parse_u32(
                value,
                "account alias dataspace",
            )?)),
            None => return Err(json::Error::Message(format!("missing {label}.dataspace"))),
        };

        ensure_no_extra_fields(&fields)?;

        Ok(iroha_data_model::account::rekey::AccountAlias::new(
            alias_label,
            domain,
            dataspace,
        ))
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
        account.canonical_i105().ok()
    }

    fn asset_literal(asset: &AssetId) -> String {
        asset.canonical_literal()
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

        if let Some(set_asset_definition_alias) = instruction
            .as_any()
            .downcast_ref::<SetAssetDefinitionAlias>()
        {
            let mut fields = Map::new();
            fields.insert(
                "asset_definition_id".to_string(),
                Value::String(set_asset_definition_alias.asset_definition_id().to_string()),
            );
            fields.insert(
                "alias".to_string(),
                set_asset_definition_alias
                    .alias()
                    .as_ref()
                    .map_or(Value::Null, |alias| Value::String(alias.to_string())),
            );
            fields.insert(
                "lease_expiry_ms".to_string(),
                set_asset_definition_alias
                    .lease_expiry_ms()
                    .as_ref()
                    .map_or(Value::Null, |value| Value::Number(Number::U64(*value))),
            );
            let mut outer = Map::new();
            outer.insert("SetAssetDefinitionAlias".to_string(), Value::Object(fields));
            return Some(Value::Object(outer));
        }

        if let Some(custom) = instruction.as_any().downcast_ref::<CustomInstruction>() {
            let payload = norito::json::parse_value(custom.payload().get()).ok()?;
            let mut inner = Map::new();
            inner.insert("payload".to_string(), payload);
            let mut outer = Map::new();
            outer.insert("Custom".to_string(), Value::Object(inner));
            return Some(Value::Object(outer));
        }

        if let Some(citizen) = instruction.as_any().downcast_ref::<RegisterCitizen>() {
            let mut fields = Map::new();
            fields.insert(
                "owner".to_string(),
                Value::String(account_literal(&citizen.owner)?),
            );
            fields.insert(
                "amount".to_string(),
                Value::String(citizen.amount.to_string()),
            );
            let mut outer = Map::new();
            outer.insert("RegisterCitizen".to_string(), Value::Object(fields));
            return Some(Value::Object(outer));
        }

        if let Some(grant) = instruction.as_any().downcast_ref::<GrantBox>() {
            return match grant {
                GrantBox::Permission(grant_perm) => {
                    let permission = norito::json::value::to_value(grant_perm.object()).ok()?;
                    let mut fields = Map::new();
                    fields.insert("object".to_string(), permission);
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
            fields.insert(
                "peer_id".to_string(),
                Value::String(register.peer_id().to_string()),
            );
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

        if let Some(create) = instruction
            .as_any()
            .downcast_ref::<CreateFeeSponsorProgram>()
        {
            let mut fields = Map::new();
            fields.insert(
                "program".to_owned(),
                norito::json::value::to_value(create.program()).ok()?,
            );
            let mut outer = Map::new();
            outer.insert("CreateFeeSponsorProgram".to_owned(), Value::Object(fields));
            return Some(Value::Object(outer));
        }

        if let Some(stage) = instruction
            .as_any()
            .downcast_ref::<StageFeeSponsorProgramRevision>()
        {
            let mut fields = Map::new();
            fields.insert(
                "revision".to_owned(),
                norito::json::value::to_value(stage.revision()).ok()?,
            );
            let mut outer = Map::new();
            outer.insert(
                "StageFeeSponsorProgramRevision".to_owned(),
                Value::Object(fields),
            );
            return Some(Value::Object(outer));
        }

        if let Some(enroll) = instruction
            .as_any()
            .downcast_ref::<EnrollFeeSponsorBeneficiary>()
        {
            let mut fields = Map::new();
            fields.insert(
                "program_id".to_owned(),
                norito::json::value::to_value(enroll.program_id()).ok()?,
            );
            fields.insert(
                "beneficiary".to_owned(),
                Value::String(account_literal(enroll.beneficiary())?),
            );
            let mut outer = Map::new();
            outer.insert(
                "EnrollFeeSponsorBeneficiary".to_owned(),
                Value::Object(fields),
            );
            return Some(Value::Object(outer));
        }

        if let Some(fund) = instruction.as_any().downcast_ref::<FundFeeSponsorProgram>() {
            let mut fields = Map::new();
            fields.insert(
                "program_id".to_owned(),
                norito::json::value::to_value(fund.program_id()).ok()?,
            );
            fields.insert(
                "asset_definition_id".to_owned(),
                Value::String(fund.asset_definition_id().canonical_address()),
            );
            fields.insert(
                "amount".to_owned(),
                Value::String(fund.amount().to_string()),
            );
            let mut outer = Map::new();
            outer.insert("FundFeeSponsorProgram".to_owned(), Value::Object(fields));
            return Some(Value::Object(outer));
        }

        if let Some(activate) = instruction
            .as_any()
            .downcast_ref::<ActivateFeeSponsorProgramRevision>()
        {
            let mut fields = Map::new();
            fields.insert(
                "program_id".to_owned(),
                norito::json::value::to_value(activate.program_id()).ok()?,
            );
            fields.insert(
                "revision".to_owned(),
                Value::Number(Number::U64(*activate.revision())),
            );
            fields.insert(
                "activate_at_height".to_owned(),
                Value::Number(Number::U64(*activate.activate_at_height())),
            );
            let mut outer = Map::new();
            outer.insert(
                "ActivateFeeSponsorProgramRevision".to_owned(),
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
        use std::{collections::BTreeSet, num::NonZeroU64, path::PathBuf};

        #[allow(unused_imports)]
        use iroha_data_model::{
            asset::AssetDefinitionAlias,
            domain::Domain,
            isi::{
                GrantBox, Log, MintBox, RegisterBox, SetParameter, TransferBox,
                governance::RegisterCitizen,
                nexus::{
                    ActivateFeeSponsorProgramRevision, CreateFeeSponsorProgram,
                    EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram,
                    StageFeeSponsorProgramRevision,
                },
                staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
            },
            level::Level,
            metadata::Metadata,
            nexus::{
                DataSpaceId, FeeSponsorAssetBudget, FeeSponsorEligibility,
                FeeSponsorNativeInstructionSelector, FeeSponsorProgram, FeeSponsorProgramId,
                FeeSponsorProgramRevision, FeeSponsorRule, FeeSponsorRuleEffect,
                FeeSponsorRuleSelector, LaneId,
            },
            parameter::{Parameter, TransactionParameter},
            permission::Permission,
            prelude::{
                AccountId, AssetDefinitionId, AssetId, Grant, InstructionBox, Mint, Register,
                Transfer,
            },
        };
        use iroha_executor_data_model::permission::{
            account::{AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias},
            parameter::CanSetParameters,
        };
        use iroha_primitives::json::Json;
        use iroha_test_samples::ALICE_ID;

        use super::*;

        #[test]
        fn instructions_to_value_keeps_structure() {
            let domain =
                Register::domain(Domain::new(DomainId::try_new("demo", "universal").unwrap()));
            let value = instructions_to_value(&[InstructionBox::from(domain)]);
            let arr = value.as_array().expect("array");
            assert_eq!(arr.len(), 1);
            let outer = arr[0].as_object().expect("outer object");
            assert!(outer.contains_key("Register"));
        }

        #[test]
        fn serialize_register_uses_structured_json() {
            let domain = Register::domain(Domain::new(
                DomainId::try_new("structured", "universal").unwrap(),
            ));
            let instruction: InstructionBox = domain.into();
            let mut out = String::new();
            serialize(&[instruction], &mut out);
            let parsed = norito::json::from_str::<Value>(&out).expect("parse serialized JSON");
            let array = parsed.as_array().expect("instructions array");
            assert!(array.first().unwrap().is_object());
        }

        #[test]
        fn fee_sponsor_lifecycle_uses_structured_genesis_json() {
            let program_id = FeeSponsorProgramId::new(
                ALICE_ID.clone(),
                "default".parse().expect("program name"),
            );
            let fee_asset_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("universal", "universal").expect("domain"),
                "xor".parse().expect("asset name"),
            );
            let revision = FeeSponsorProgramRevision {
                program_id: program_id.clone(),
                revision: 1,
                eligibility: FeeSponsorEligibility::EnrolledOnly,
                rules: vec![FeeSponsorRule {
                    id: "onboarding".parse().expect("rule name"),
                    effect: FeeSponsorRuleEffect::Allow,
                    selectors: vec![FeeSponsorRuleSelector::NativeInstruction(
                        FeeSponsorNativeInstructionSelector {
                            wire_id: RegisterBox::WIRE_ID.to_owned(),
                            asset_definition_id: None,
                        },
                    )],
                }],
                asset_budgets: vec![FeeSponsorAssetBudget {
                    asset_definition_id: fee_asset_id.clone(),
                    per_transaction: Quantity::from(10_u64),
                    per_block: Quantity::from(100_u64),
                    per_program_epoch: Quantity::from(1_000_u64),
                    per_beneficiary_epoch: Quantity::from(100_u64),
                    reserve_floor: Quantity::from(10_u64),
                    epoch_length_blocks: NonZeroU64::new(100).expect("non-zero"),
                }],
            };
            let instructions = vec![
                InstructionBox::from(CreateFeeSponsorProgram {
                    program: FeeSponsorProgram::new(program_id.clone()),
                }),
                InstructionBox::from(StageFeeSponsorProgramRevision { revision }),
                InstructionBox::from(EnrollFeeSponsorBeneficiary {
                    program_id: program_id.clone(),
                    beneficiary: ALICE_ID.clone(),
                }),
                InstructionBox::from(FundFeeSponsorProgram {
                    program_id: program_id.clone(),
                    asset_definition_id: fee_asset_id,
                    amount: Quantity::from(1_000_u64),
                }),
                InstructionBox::from(ActivateFeeSponsorProgramRevision {
                    program_id,
                    revision: 1,
                    activate_at_height: 1,
                }),
            ];

            let value = instructions_to_value(&instructions);
            let array = value.as_array().expect("instruction array");
            for (value, expected_key) in array.iter().zip([
                "CreateFeeSponsorProgram",
                "StageFeeSponsorProgramRevision",
                "EnrollFeeSponsorBeneficiary",
                "FundFeeSponsorProgram",
                "ActivateFeeSponsorProgramRevision",
            ]) {
                assert!(
                    value
                        .as_object()
                        .is_some_and(|object| object.contains_key(expected_key)),
                    "missing structured {expected_key}: {value:?}"
                );
            }

            let decoded = from_value(&value).expect("decode structured fee sponsor lifecycle");
            assert_eq!(decoded.len(), instructions.len());
            assert!(
                decoded[0]
                    .as_any()
                    .downcast_ref::<CreateFeeSponsorProgram>()
                    .is_some()
            );
            assert!(
                decoded[4]
                    .as_any()
                    .downcast_ref::<ActivateFeeSponsorProgramRevision>()
                    .is_some()
            );
        }

        #[test]
        fn register_citizen_uses_structured_genesis_json() {
            let instruction = InstructionBox::from(RegisterCitizen {
                owner: ALICE_ID.clone(),
                amount: Quantity::from(10_000_u64),
            });
            let value = instructions_to_value(std::slice::from_ref(&instruction));
            let array = value.as_array().expect("instruction array");
            let fields = array[0]
                .as_object()
                .and_then(|outer| outer.get("RegisterCitizen"))
                .and_then(Value::as_object)
                .expect("structured RegisterCitizen");
            assert_eq!(
                fields.get("owner").and_then(Value::as_str),
                ALICE_ID.canonical_i105().ok().as_deref()
            );
            assert_eq!(fields.get("amount").and_then(Value::as_str), Some("10000"));

            let decoded = from_value(&value).expect("decode structured RegisterCitizen");
            assert_eq!(decoded.len(), 1);
            assert_eq!(
                decoded[0].as_any().downcast_ref::<RegisterCitizen>(),
                instruction.as_any().downcast_ref::<RegisterCitizen>()
            );
        }

        #[test]
        fn value_to_instruction_rejects_bytes() {
            let value = Value::Array(vec![Value::Number(Number::U64(1))]);
            let err = value_to_instruction(value).expect_err("byte arrays should be rejected");
            assert!(err.to_string().contains("byte arrays"));
        }

        #[test]
        fn value_to_instruction_rejects_invalid_base64_string() {
            let value = Value::String("***".to_string());
            let err = value_to_instruction(value).expect_err("invalid base64 should fail");
            assert!(err.to_string().contains("invalid base64"));
        }

        #[test]
        fn value_to_instruction_accepts_base64_string_for_custom_instruction() {
            super::super::init_instruction_registry();
            let asset_definition_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("zk", "universal").expect("domain"),
                "xor".parse().expect("asset name"),
            );
            let instruction = InstructionBox::from(
                iroha_data_model::isi::zk::RegisterZkAsset::new(asset_definition_id, None, None),
            );

            let value = instruction_value(&instruction);
            assert!(
                value.is_string(),
                "custom instruction should fall back to base64"
            );

            let decoded =
                value_to_instruction(value).expect("base64-encoded instruction should decode");
            assert_eq!(
                norito::codec::encode_adaptive(&decoded),
                norito::codec::encode_adaptive(&instruction)
            );
        }

        #[test]
        fn base64_instruction_rejects_valid_noncanonical_norito_layout() {
            super::super::init_instruction_registry();
            let instruction = InstructionBox::from(Log::new(
                Level::INFO,
                "canonical genesis boundary".to_owned(),
            ));
            let canonical = norito::encode_canonical(&instruction)
                .expect("encode canonical genesis instruction");
            let canonical_value = Value::String(
                base64::engine::general_purpose::STANDARD.encode(canonical.as_slice()),
            );
            value_to_instruction(canonical_value)
                .expect("canonical base64 genesis instruction must decode");

            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let alternate = {
                let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
                norito::core::to_bytes(&instruction)
                    .expect("encode valid alternate-layout instruction")
            };
            assert_ne!(alternate, canonical);
            let alternate_value = Value::String(
                base64::engine::general_purpose::STANDARD.encode(alternate.as_slice()),
            );

            let error = value_to_instruction(alternate_value)
                .expect_err("noncanonical base64 genesis instruction must be rejected");
            assert!(error.to_string().contains("canonical"));
        }

        #[test]
        fn structured_genesis_rejects_negative_asset_mint_quantity() {
            let asset_id = AssetId::new(
                AssetDefinitionId::derive_from_components(
                    DomainId::try_new("wonderland", "universal").expect("domain"),
                    "coin".parse().expect("asset name"),
                ),
                ALICE_ID.clone(),
            );
            let source = format!(
                r#"{{"Mint":{{"Asset":{{"object":"-0.01","destination":"{asset_id}"}}}}}}"#
            );
            let value = norito::json::from_str(&source).expect("parse structured mint");

            let error = value_to_instruction(value)
                .expect_err("negative asset quantity must not enter genesis instructions");
            assert!(error.to_string().contains("invalid asset mint quantity"));
        }

        #[test]
        fn deserialize_structured_instructions_roundtrip() {
            let account_id = ALICE_ID.clone();
            let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
            let domain = Domain::new(domain_id.clone());
            let asset_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
                domain_id.clone(),
                "coin".parse().unwrap(),
            );
            let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
            let asset_alias: AssetDefinitionAlias = "coin#wonderland.universal".parse().unwrap();

            let parameter = Parameter::Transaction(TransactionParameter::MaxInstructions(
                NonZeroU64::new(64).unwrap(),
            ));

            let instructions: Vec<InstructionBox> = vec![
                Register::domain(domain.clone()).into(),
                Mint::asset_quantity(42u32, asset_id.clone()).into(),
                Transfer::asset_definition(
                    account_id.clone(),
                    asset_def_id.clone(),
                    account_id.clone(),
                )
                .into(),
                Grant::account_permission(CanSetParameters, account_id.clone()).into(),
                SetParameter::new(parameter.clone()).into(),
                SetAssetDefinitionAlias::bind(asset_def_id.clone(), asset_alias.clone(), None)
                    .into(),
            ];

            let mut json_text = String::new();
            serialize(&instructions, &mut json_text);
            let parsed =
                norito::json::from_str::<Value>(&json_text).expect("parse serialized JSON");
            let instructions = from_value(&parsed).expect("deserialize instructions");
            assert_eq!(instructions.len(), 6);

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
            match instructions[5]
                .as_any()
                .downcast_ref::<SetAssetDefinitionAlias>()
            {
                Some(set_alias) => {
                    assert_eq!(set_alias.asset_definition_id(), &asset_def_id);
                    assert_eq!(set_alias.alias().as_ref(), Some(&asset_alias));
                    assert_eq!(set_alias.lease_expiry_ms(), &None);
                }
                other => panic!("unexpected set-asset-definition-alias instruction: {other:?}"),
            }
        }

        #[test]
        fn scoped_alias_permission_grants_preserve_payloads_through_genesis_json() {
            let account_id = ALICE_ID.clone();
            let universal = AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL);
            let private = AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10));
            let domain = AccountAliasPermissionScope::Domain(
                DomainId::parse_fully_qualified("hbl.sbp").expect("domain scope must parse"),
            );
            let cases = [universal, private, domain]
                .into_iter()
                .flat_map(|scope| {
                    [
                        (
                            Permission::from(CanManageAccountAlias {
                                scope: scope.clone(),
                            }),
                            scope.clone(),
                        ),
                        (
                            Permission::from(CanResolveAccountAlias {
                                scope: scope.clone(),
                            }),
                            scope,
                        ),
                    ]
                })
                .collect::<Vec<_>>();
            let instructions = cases
                .iter()
                .map(|(permission, _)| {
                    Grant::account_permission(permission.clone(), account_id.clone()).into()
                })
                .collect::<Vec<InstructionBox>>();

            let encoded = instructions_to_value(&instructions);
            for instruction in encoded.as_array().expect("instruction array") {
                let payload = instruction
                    .get("Grant")
                    .and_then(|value| value.get("Permission"))
                    .and_then(|value| value.get("object"))
                    .and_then(|value| value.get("payload"))
                    .expect("structured permission grant must include its payload");
                assert_ne!(
                    payload,
                    &Value::Null,
                    "scoped permission payload must not collapse to null"
                );
            }

            let decoded = from_value(&encoded).expect("decode structured permission grants");
            assert_eq!(decoded.len(), cases.len());
            let mut unique = BTreeSet::new();
            for (instruction, (expected_permission, expected_scope)) in decoded.iter().zip(&cases) {
                let GrantBox::Permission(grant) = instruction
                    .as_any()
                    .downcast_ref::<GrantBox>()
                    .expect("decoded instruction must be a permission grant")
                else {
                    panic!("decoded grant must target an account");
                };
                assert_eq!(grant.destination(), &account_id);
                assert_eq!(grant.object(), expected_permission);
                assert!(
                    unique.insert((grant.destination().clone(), grant.object().clone())),
                    "scoped permission grants must remain distinct"
                );
                match expected_permission.name() {
                    "CanManageAccountAlias" => assert_eq!(
                        CanManageAccountAlias::try_from(grant.object())
                            .expect("decode manage permission")
                            .scope,
                        expected_scope.clone()
                    ),
                    "CanResolveAccountAlias" => assert_eq!(
                        CanResolveAccountAlias::try_from(grant.object())
                            .expect("decode resolve permission")
                            .scope,
                        expected_scope.clone()
                    ),
                    name => panic!("unexpected alias permission `{name}`"),
                }
            }
        }

        #[test]
        fn deserialize_structured_register_account_with_label() {
            let account_id = ALICE_ID.clone();
            let account_literal = account_literal(&account_id).expect("account literal");
            let expected_label = iroha_data_model::account::rekey::AccountAlias::new(
                "admin1".parse().expect("alias label"),
                Some("hbl".parse().expect("alias domain")),
                iroha_data_model::nexus::DataSpaceId::new(10),
            );
            let register_json = format!(
                r#"{{
                    "Register": {{
                        "Account": {{
                            "id": "{account_literal}",
                            "label": {{
                                "label": "admin1",
                                "domain": "hbl",
                                "dataspace": 10
                            }},
                            "metadata": {{}},
                            "opaque_ids": [],
                            "uaid": null
                        }}
                    }}
                }}"#
            );
            let register_value =
                norito::json::from_str(&register_json).expect("parse register instruction");

            let instruction =
                super::value_to_instruction(register_value).expect("structured account decodes");
            let RegisterBox::Account(account) = instruction
                .as_any()
                .downcast_ref::<RegisterBox>()
                .expect("RegisterBox variant")
            else {
                panic!("expected account registration");
            };

            assert_eq!(account.object().id(), &account_id);
            assert_eq!(account.object().label(), Some(&expected_label));
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
            let validator_peer_id = PeerId::from(validator_id.expect_single_signatory().clone());
            let register = RegisterPublicLaneValidator::new(
                LaneId::SINGLE,
                validator_id.clone(),
                validator_peer_id.clone(),
                validator_id.clone(),
                Quantity::from(10_u64),
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
                    assert_eq!(register.peer_id(), &validator_peer_id);
                    assert_eq!(register.stake_account(), &validator_id);
                    assert_eq!(register.initial_stake(), &Quantity::from(10_u64));
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
        fn deserialize_npos_bootstrap_rejects_negative_initial_stake() {
            let validator_id = ALICE_ID.clone();
            let register = RegisterPublicLaneValidator::new(
                LaneId::SINGLE,
                validator_id.clone(),
                PeerId::from(validator_id.expect_single_signatory().clone()),
                validator_id,
                Quantity::from(10_u64),
                Metadata::default(),
            );
            let mut json_text = String::new();
            serialize(&[InstructionBox::from(register)], &mut json_text);
            let negative = json_text.replace(r#""initial_stake":"10""#, r#""initial_stake":"-1""#);
            assert_ne!(negative, json_text, "fixture must replace the stake field");

            let parsed = norito::json::from_str::<Value>(&negative)
                .expect("negative quantity remains syntactically valid JSON");
            let error = from_value(&parsed).expect_err("negative initial stake must be rejected");
            assert!(
                error.to_string().contains("invalid initial stake quantity"),
                "unexpected error: {error}"
            );
        }

        fn assert_genesis_manifest_parses_structured_instructions(relative_path: &str) {
            super::super::init_instruction_registry();
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let value: Value = norito::json::from_str(&raw).expect("parse genesis JSON");
            let chain_discriminant = value
                .as_object()
                .and_then(|obj| obj.get("chain_discriminant"))
                .cloned()
                .and_then(|value| norito::json::value::from_value::<u16>(value).ok())
                .expect("chain_discriminant");
            let _chain_discriminant =
                iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                    chain_discriminant,
                );
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
                .unwrap_or_else(|error| panic!("{} should deserialize: {error}", path.display()));
        }

        #[test]
        fn defaults_genesis_manifest_parses_structured_instructions() {
            assert_genesis_manifest_parses_structured_instructions("../../defaults/genesis.json");
        }

        #[test]
        fn taira_genesis_manifest_parses_structured_instructions() {
            assert_genesis_manifest_parses_structured_instructions(
                "../../configs/soranexus/taira/genesis.json",
            );
        }

        #[test]
        fn parse_allows_null_executor_in_canonical_manifest() {
            let mut manifest_fields = norito::json::Map::new();
            manifest_fields.insert("chain".to_string(), Value::String("test-chain".to_string()));
            manifest_fields.insert(
                "chain_discriminant".to_string(),
                norito::json::value::to_value(
                    &iroha_data_model::account::address::chain_discriminant(),
                )
                .expect("serialize chain discriminant"),
            );
            manifest_fields.insert("executor".to_string(), Value::Null);
            manifest_fields.insert("ivm_dir".to_string(), Value::String(".".to_string()));
            manifest_fields.insert(
                "consensus_mode".to_string(),
                Value::String("Permissioned".to_string()),
            );
            manifest_fields.insert(
                "sumeragi_v2".to_string(),
                norito::json::value::to_value(&SumeragiV2GenesisContextParameters::recommended())
                    .expect("serialize v2 genesis context"),
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
    /// Instructions carried by this raw genesis transaction.
    #[must_use]
    pub fn instructions(&self) -> &[InstructionBox] {
        &self.instructions
    }

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
    /// Chain discriminant / i105 network prefix used to encode account literals in this manifest.
    pub chain_discriminant: u16,
    /// Optional path to the executor bytecode.
    pub executor: Option<IvmPath>,
    /// Directory containing IVM bytecode libraries.
    pub ivm_dir: PathBuf,
    /// Consensus mode advertised in genesis.
    pub consensus_mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    /// First-release consensus protocol version.
    pub wire_protocol_version: u32,
    /// Deterministic fingerprint of consensus parameters.
    pub consensus_fingerprint: ConsensusFingerprint,
    /// Signed Sumeragi v2 height-context transport parameters.
    pub sumeragi_v2: SumeragiV2GenesisContextParameters,
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
        map.insert(
            "chain_discriminant".to_string(),
            norito::json::value::to_value(&self.chain_discriminant)
                .expect("serialize chain_discriminant"),
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
            "wire_protocol_version".to_string(),
            norito::json::value::to_value(&self.wire_protocol_version)
                .expect("serialize wire_protocol_version"),
        );
        map.insert(
            "consensus_fingerprint".to_string(),
            norito::json::value::to_value(&self.consensus_fingerprint)
                .expect("serialize consensus fingerprint"),
        );
        map.insert(
            "sumeragi_v2".to_string(),
            norito::json::value::to_value(&self.sumeragi_v2)
                .expect("serialize Sumeragi v2 context parameters"),
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
    parameters.parameters().collect()
}

fn has_set_parameter(instructions: &[InstructionBox], parameter: &Parameter) -> bool {
    instructions.iter().any(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<SetParameter>()
            .is_some_and(|existing| parameter_targets_same_slot(existing.inner(), parameter))
    })
}

fn parameter_generation_priority(parameter: &Parameter, current: &Parameters) -> u8 {
    let _ = (parameter, current);
    25
}

fn collect_parameter_instructions(
    parameters: &Parameters,
    existing: &[InstructionBox],
    manual: &[Parameter],
    current: &Parameters,
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
                if has_set_parameter(existing, &other)
                    || generated
                        .iter()
                        .any(|existing| parameter_targets_same_slot(existing, &other))
                {
                    continue;
                }
                generated.push(other);
            }
        }
    }
    generated.sort_by_key(|parameter| parameter_generation_priority(parameter, current));
    generated
        .into_iter()
        .map(|parameter| InstructionBox::from(SetParameter::new(parameter)))
        .collect()
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

fn compute_consensus_fingerprint_v2(
    chain_id: &ChainId,
    params: &iroha_data_model::block::consensus::ConsensusGenesisParams,
) -> Result<[u8; 32]> {
    iroha_data_model::block::consensus_v2::fingerprint::compute(chain_id, params)
        .map_err(|error| eyre!("invalid signed consensus parameters: {error}"))
}

impl RawGenesisTransaction {
    fn validate_mode_specific_consensus_parameters(&self) -> Result<()> {
        self.validate_structured_parameter_blocks()?;
        let has_npos = self
            .effective_parameters()?
            .custom()
            .contains_key(&SumeragiNposParameters::parameter_id());
        match (self.consensus_mode, has_npos) {
            (SumeragiConsensusMode::Permissioned, false) | (SumeragiConsensusMode::Npos, true) => {
                Ok(())
            }
            (SumeragiConsensusMode::Permissioned, true) => Err(eyre!(
                "permissioned genesis must omit `sumeragi_npos_parameters`"
            )),
            (SumeragiConsensusMode::Npos, false) => Err(eyre!(
                "NPoS genesis requires `sumeragi_npos_parameters`; node-local election defaults are not signed inputs"
            )),
        }
    }

    fn validate_structured_parameter_blocks(&self) -> Result<()> {
        let positions = self
            .transactions
            .iter()
            .enumerate()
            .filter_map(|(index, tx)| tx.parameters.as_ref().map(|_| index))
            .collect::<Vec<_>>();
        if positions.len() > 1 {
            return Err(eyre!(
                "genesis manifest contains multiple structured `parameters` blocks at transaction indices {positions:?}; use exactly one authoritative block because `Parameters` is a complete snapshot, not a patch"
            ));
        }
        Ok(())
    }

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
        let chain_discriminant = Self::take_required_field::<u16>(&mut map, "chain_discriminant")?;
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
        let _chain_discriminant =
            iroha_data_model::account::address::ChainDiscriminantGuard::enter(chain_discriminant);
        let transactions =
            Self::decode_value::<Vec<RawGenesisTx>>(transactions_value, "transactions")?;
        Self::reject_set_parameter_instructions(&transactions)?;
        let parameter_blocks = transactions
            .iter()
            .enumerate()
            .filter_map(|(index, tx)| tx.parameters.as_ref().map(|_| index))
            .collect::<Vec<_>>();
        if parameter_blocks.len() > 1 {
            return Err(norito::json::Error::Message(format!(
                "genesis manifest contains multiple structured `parameters` blocks at transaction indices {parameter_blocks:?}; use exactly one authoritative block because `Parameters` is a complete snapshot, not a patch"
            )));
        }
        let consensus_mode = Self::take_required_field::<
            iroha_data_model::parameter::system::SumeragiConsensusMode,
        >(&mut map, "consensus_mode")?;
        let wire_protocol_version =
            Self::take_required_field::<u32>(&mut map, "wire_protocol_version")?;
        let consensus_fingerprint =
            Self::take_optional_field::<ConsensusFingerprint>(&mut map, "consensus_fingerprint")?;
        let sumeragi_v2 = Self::take_required_field::<SumeragiV2GenesisContextParameters>(
            &mut map,
            "sumeragi_v2",
        )?;
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
            chain_discriminant,
            executor,
            ivm_dir,
            transactions,
            consensus_mode,
            wire_protocol_version,
            consensus_fingerprint,
            sumeragi_v2,
            crypto,
        })
    }

    /// Compute the effective parameter set after applying all structured sections and explicit `SetParameter` instructions.
    pub fn effective_parameters(&self) -> Result<Parameters> {
        self.validate_structured_parameter_blocks()?;
        // Mirror `parse()` parameter injection rules: structured `parameters` sections are first
        // turned into `SetParameter` instructions with `collect_parameter_instructions`, which
        // suppresses slots already set manually (any explicit `SetParameter` anywhere in the
        // manifest). This keeps the derived consensus fingerprint consistent with the final
        // parsed instruction batches.
        let manual_parameters = collect_manual_set_parameters(&self.transactions);
        let mut aggregated = Parameters::default();
        for tx in &self.transactions {
            if let Some(params) = &tx.parameters {
                aggregated.sumeragi.block_cadence_ms = params.sumeragi.block_cadence_ms;
                for instruction in collect_parameter_instructions(
                    params,
                    &tx.instructions,
                    &manual_parameters,
                    &aggregated,
                ) {
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
        Ok(aggregated)
    }

    /// Populate consensus metadata fields with defaults and a computed v2 fingerprint.
    ///
    /// This helper is best-effort and does not alter existing transactions. It derives
    /// parameters from data-model defaults to produce a stable fingerprint for basic networks.
    #[must_use]
    pub fn with_consensus_meta(mut self) -> Self {
        use iroha_data_model::parameter::system::{
            BlockParameters, SumeragiConsensusMode, SumeragiParameters,
        };
        let Ok(params) = self.effective_parameters() else {
            self.consensus_fingerprint = None;
            return self;
        };
        let sumeragi: SumeragiParameters = params.sumeragi().clone();
        let block: BlockParameters = params.block();
        let custom = params.custom();
        let block_cadence_ms = sumeragi.block_cadence_ms();
        let block_max_transactions = block.max_transactions();

        // `effective_parameters()` already applies both structured parameter sections and
        // explicit SetParameter instructions in manifest transaction order.

        let npos_param_id = SumeragiNposParameters::parameter_id();
        let npos_payload = custom
            .get(&npos_param_id)
            .and_then(SumeragiNposParameters::from_custom_parameter);

        // Consensus mode is a first-release signed-genesis choice. Runtime
        // mode staging is unrepresentable, and the mere presence of NPoS
        // tuning data must never infer or flip the live protocol mode.
        let mode = self.consensus_mode;

        let mode = match (mode, npos_payload) {
            (SumeragiConsensusMode::Permissioned, None) => ConsensusGenesisModeParams::Permissioned,
            (SumeragiConsensusMode::Permissioned, Some(_))
            | (SumeragiConsensusMode::Npos, None) => {
                self.consensus_fingerprint = None;
                return self;
            }
            (SumeragiConsensusMode::Npos, Some(npos)) => {
                ConsensusGenesisModeParams::Npos(NposGenesisParams {
                    epoch_length_blocks: npos.epoch_length_blocks(),
                    epoch_seed: npos.epoch_seed(),
                    vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                    vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                    max_validators: npos.max_validators(),
                    min_self_bond: npos.min_self_bond().clone(),
                    min_nomination_bond: npos.min_nomination_bond().clone(),
                    max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                    seat_band_pct: npos.seat_band_pct(),
                    max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                    finality_margin_blocks: npos.finality_margin_blocks(),
                    evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                    activation_lag_blocks: npos.activation_lag_blocks(),
                    slashing_delay_blocks: npos.slashing_delay_blocks(),
                })
            }
        };

        let dm_params = ConsensusGenesisParams {
            block_cadence_ms,
            block_max_transactions,
            mode,
            protocol_version: iroha_config::parameters::defaults::sumeragi::PROTOCOL_VERSION,
            v2_context: self.sumeragi_v2,
        };
        let Ok(fp) = compute_consensus_fingerprint_v2(&self.chain, &dm_params) else {
            self.consensus_fingerprint = None;
            return self;
        };
        self.wire_protocol_version = CONSENSUS_PROTOCOL_VERSION;
        self.consensus_fingerprint = Some(ConsensusFingerprint::new(fp));
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
        self.validate_mode_specific_consensus_parameters()?;
        // Always refresh consensus metadata so fingerprints stay aligned with
        // effective parameters after manifest edits.
        let manifest = self.with_consensus_meta();

        let consensus_mode = manifest.consensus_mode;
        if manifest.wire_protocol_version != CONSENSUS_PROTOCOL_VERSION {
            return Err(eyre!(
                "unsupported wire_protocol_version after normalization"
            ));
        }
        let consensus_fingerprint = manifest.consensus_fingerprint.clone().ok_or_else(|| {
            eyre!(
                "consensus_fingerprint missing after normalization; call with_consensus_meta first"
            )
        })?;
        let sumeragi_v2 = manifest.sumeragi_v2;
        sumeragi_v2
            .validate()
            .map_err(|error| eyre!("invalid signed Sumeragi v2 context parameters: {error}"))?;

        let chain = manifest.chain.clone();
        let chain_discriminant = manifest.chain_discriminant;
        let executor = manifest.executor.clone();
        let ivm_dir = manifest.ivm_dir.as_path().to_path_buf();
        let wire_protocol_version = manifest.wire_protocol_version;
        let crypto = manifest.crypto.clone();
        let transactions = manifest.parse()?;

        Ok(NormalizedGenesis {
            chain,
            chain_discriminant,
            executor,
            ivm_dir,
            consensus_mode,
            wire_protocol_version,
            consensus_fingerprint,
            sumeragi_v2,
            crypto,
            transactions,
        })
    }

    /// Chain identifier advertised in the manifest.
    #[must_use]
    pub fn chain_id(&self) -> &ChainId {
        &self.chain
    }

    /// Chain discriminant / i105 network prefix advertised in the manifest.
    #[must_use]
    pub const fn chain_discriminant(&self) -> u16 {
        self.chain_discriminant
    }

    /// Override the chain discriminant used when rendering this manifest.
    #[must_use]
    pub fn with_chain_discriminant(mut self, chain_discriminant: u16) -> Self {
        self.chain_discriminant = chain_discriminant;
        self
    }

    /// Raw genesis transactions preserved in the manifest.
    #[must_use]
    pub fn transactions(&self) -> &[RawGenesisTx] {
        &self.transactions
    }

    /// Replace one instruction-only raw transaction with one or more
    /// instruction-only transactions.
    ///
    /// This deliberately refuses to rewrite a transaction that also carries
    /// parameters, IVM triggers, or topology. Callers can therefore perform a
    /// narrow transaction-boundary migration without silently moving any
    /// other genesis semantics.
    pub fn replace_instruction_only_transaction(
        &mut self,
        index: usize,
        replacement_batches: Vec<Vec<InstructionBox>>,
    ) -> Result<()> {
        if replacement_batches.is_empty() {
            return Err(eyre!(
                "replacement for raw genesis transaction {index} must contain at least one batch"
            ));
        }
        if let Some((batch_index, _)) = replacement_batches
            .iter()
            .enumerate()
            .find(|(_, batch)| batch.is_empty())
        {
            return Err(eyre!(
                "replacement batch {batch_index} for raw genesis transaction {index} must not be empty"
            ));
        }

        let original = self.transactions.get(index).ok_or_else(|| {
            eyre!(
                "raw genesis transaction index {index} is out of bounds for {} transactions",
                self.transactions.len()
            )
        })?;
        if original.parameters.is_some()
            || !original.ivm_triggers.is_empty()
            || !original.topology.is_empty()
        {
            return Err(eyre!(
                "raw genesis transaction {index} is not instruction-only; refusing to move parameters, IVM triggers, or topology"
            ));
        }

        let replacements = replacement_batches
            .into_iter()
            .map(|instructions| RawGenesisTx {
                parameters: None,
                instructions,
                ivm_triggers: Vec::new(),
                topology: Vec::new(),
            });
        self.transactions.splice(index..=index, replacements);
        Ok(())
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
    #[must_use]
    pub fn consensus_mode(&self) -> iroha_data_model::parameter::system::SumeragiConsensusMode {
        self.consensus_mode
    }

    /// Return a copy of the manifest with `consensus_mode` populated for handshake metadata.
    #[must_use]
    pub fn with_consensus_mode(
        mut self,
        mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    ) -> Self {
        self.consensus_mode = mode;
        self
    }

    /// Optional typed consensus fingerprint advertised in the manifest.
    #[must_use]
    pub const fn consensus_fingerprint(&self) -> Option<ConsensusFingerprint> {
        self.consensus_fingerprint
    }

    /// First-release consensus wire protocol version advertised in the manifest.
    #[must_use]
    pub const fn wire_protocol_version(&self) -> u32 {
        self.wire_protocol_version
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

    fn manifest_chain_discriminant_value() -> norito::json::Value {
        norito::json::value::to_value(&iroha_data_model::account::address::chain_discriminant())
            .expect("serialize chain discriminant")
    }

    fn manifest_v2_context_value() -> norito::json::Value {
        norito::json::value::to_value(&SumeragiV2GenesisContextParameters::recommended())
            .expect("serialize v2 genesis context")
    }

    #[test]
    fn genesis_fixture_key_generation_preserves_algorithms() {
        assert_eq!(
            checked_genesis_fixture_keypair().public_key().algorithm(),
            Algorithm::default()
        );
        for algorithm in [Algorithm::Ed25519, Algorithm::BlsNormal] {
            assert_eq!(
                checked_genesis_fixture_keypair_with_algorithm(algorithm)
                    .public_key()
                    .algorithm(),
                algorithm
            );
        }
    }

    #[test]
    fn with_consensus_meta_adds_fields_and_stable_fingerprint() {
        let chain = ChainId::from("iroha:test:genesismeta");
        let tx = RawGenesisTransaction {
            chain: chain.clone(),
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };
        let tx2 = tx.clone().with_consensus_meta();
        assert_eq!(tx2.consensus_mode, SumeragiConsensusMode::Permissioned);
        assert_eq!(tx2.wire_protocol_version, CONSENSUS_PROTOCOL_VERSION);
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
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx {
                parameters: Some(params),
                ..RawGenesisTx::default()
            }],
            consensus_mode: SumeragiConsensusMode::Npos,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        }
        .with_consensus_meta();

        assert_eq!(manifest.consensus_mode, SumeragiConsensusMode::Npos);
        assert_eq!(manifest.wire_protocol_version, CONSENSUS_PROTOCOL_VERSION);
        let fp = manifest
            .consensus_fingerprint
            .expect("fingerprint must be present");
        assert!(
            fp.to_string().starts_with("0x"),
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
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![tx],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        }
        .with_consensus_meta();

        let params = manifest
            .effective_parameters()
            .expect("single structured parameter block");
        assert_eq!(
            params.block().max_transactions().get(),
            max_txs.get(),
            "effective parameters must reflect block max override"
        );

        let expected = compute_consensus_fingerprint_v2(
            &chain,
            &ConsensusGenesisParams {
                block_cadence_ms: params.sumeragi().block_cadence_ms(),
                block_max_transactions: params.block().max_transactions(),
                mode: ConsensusGenesisModeParams::Permissioned,
                protocol_version: iroha_config::parameters::defaults::sumeragi::PROTOCOL_VERSION,
                v2_context: SumeragiV2GenesisContextParameters::recommended(),
            },
        )
        .expect("canonical permissioned parameters must fingerprint");
        let observed = manifest
            .consensus_fingerprint
            .expect("consensus fingerprint injected")
            .into_bytes();
        assert_eq!(observed, expected);
    }

    #[test]
    fn build_and_sign_uses_stable_internal_creation_times() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:deterministic");
        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let keypair = checked_genesis_fixture_keypair();

        let genesis = manifest.build_and_sign(&keypair).expect("sign genesis");

        let bytes_a = genesis.0.encode_wire().expect("encode canonical genesis");
        let bytes_b = genesis.0.encode_wire().expect("encode canonical genesis");
        assert_eq!(bytes_a, bytes_b, "Genesis encoding must be deterministic");

        let tx_times: Vec<u64> = genesis
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
            let block_time = genesis.0.header().creation_time().as_millis();
            let block_time = u64::try_from(block_time).expect("block creation time fits into u64");
            assert_eq!(
                block_time,
                last_tx + 1,
                "block creation time must follow the last transaction deterministically"
            );
        }
    }

    #[test]
    fn explicit_creation_time_makes_signed_genesis_reproducible() {
        init_instruction_registry();

        let manifest = RawGenesisTransaction {
            chain: ChainId::from("iroha:test:fixed-genesis-time"),
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default(), RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };
        let keypair = checked_genesis_fixture_keypair();
        let batch_count = u64::try_from(
            manifest
                .clone()
                .parse()
                .expect("parse fixed-time genesis manifest")
                .len(),
        )
        .expect("genesis transaction batch count fits into u64");
        assert!(
            batch_count > 0,
            "a parsed genesis manifest must contain at least one transaction batch"
        );
        let sign = |manifest: RawGenesisTransaction| {
            manifest
                .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                    &keypair,
                    None,
                    None,
                    1_700_000_000_000,
                )
                .expect("sign genesis at fixed time")
                .0
                .encode_wire()
                .expect("encode fixed-time genesis")
        };

        assert_eq!(sign(manifest.clone()), sign(manifest.clone()));
        let last_representable_base = u64::MAX - batch_count;
        let boundary = manifest
            .clone()
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                &keypair,
                None,
                None,
                last_representable_base,
            )
            .expect("the last representable explicit creation-time base must succeed");
        assert_eq!(
            boundary.0.header().creation_time().as_millis(),
            u128::from(u64::MAX),
            "the block timestamp must use the final representable millisecond"
        );
        let error = manifest
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                &keypair,
                None,
                None,
                last_representable_base + 1,
            )
            .expect_err("overflowing explicit creation-time base must be rejected");
        assert!(
            error.to_string().contains("cannot represent"),
            "unexpected overflow error: {error:#}"
        );
    }

    #[test]
    fn build_and_sign_checked_genesis_transaction_signatures_verify() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:checked-genesis-sign");
        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default(), RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };
        let keypair = checked_genesis_fixture_keypair();

        let genesis = manifest.build_and_sign(&keypair).expect("sign genesis");
        let transactions: Vec<_> = genesis.0.external_transactions().collect();
        assert!(
            !transactions.is_empty(),
            "genesis builder should emit signed external transactions"
        );

        for transaction in transactions {
            transaction
                .verify_signature()
                .expect("checked genesis transaction signature should verify");
        }
    }

    #[test]
    fn collect_parameter_instructions_respects_manual_values() {
        use iroha_data_model::parameter::{Parameters, system::SumeragiParameter};

        let parameters = Parameters::default();
        let manual = vec![Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(250))];
        let generated =
            collect_parameter_instructions(&parameters, &[], &manual, &Parameters::default());
        let has_conflict = generated.iter().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .is_some_and(|set| {
                    matches!(
                        set.inner(),
                        Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(_))
                    )
                })
        });
        assert!(
            !has_conflict,
            "manual Sumeragi overrides must suppress default value reinsertion"
        );
    }

    #[test]
    fn collect_parameter_instructions_emits_max_clock_drift_update() {
        use iroha_data_model::parameter::{Parameters, system::SumeragiParameter};

        let current = Parameters::default();
        let mut target = current.clone();
        target.set_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333)));

        let generated = collect_parameter_instructions(&target, &[], &[], &current);
        assert!(
            generated.iter().any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<SetParameter>()
                    .is_some_and(|set| {
                        matches!(
                            set.inner(),
                            Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333))
                        )
                    })
            }),
            "generated instructions must contain the mutable Sumeragi update"
        );
    }

    #[test]
    fn has_set_parameter_detects_conflicting_sumeragi_slots() {
        use iroha_data_model::parameter::system::SumeragiParameter;

        let instruction = InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::MaxClockDriftMs(100),
        )));
        assert!(has_set_parameter(
            &[instruction],
            &Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(200))
        ));
    }

    #[test]
    fn build_and_sign_sets_confidential_digest() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:confdigest");
        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let keypair = checked_genesis_fixture_keypair();
        let genesis = manifest.build_and_sign(&keypair).expect("sign genesis");

        assert_eq!(
            genesis.0.header().confidential_features(),
            Some(ConfidentialFeatureDigest::new(
                None,
                None,
                None,
                Some(RULES_VERSION),
                Some(DEFAULT_GENESIS_CONFIDENTIAL_POLICY_HASH),
            ))
        );
    }

    #[test]
    fn build_and_sign_sets_explicit_confidential_policy_hash() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:confpolicy");
        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let keypair = checked_genesis_fixture_keypair();
        let policy_hash = [0x42; 32];
        let genesis = manifest
            .build_and_sign_with_confidential_policy_hash(&keypair, Some(policy_hash))
            .expect("sign genesis with policy hash");

        assert_eq!(
            genesis.0.header().confidential_features(),
            Some(ConfidentialFeatureDigest::new(
                None,
                None,
                None,
                Some(RULES_VERSION),
                Some(policy_hash),
            ))
        );
    }

    #[test]
    fn genesis_canonical_wire_roundtrip_preserves_digest() {
        init_instruction_registry();

        let chain = ChainId::from("iroha:test:wire-digest");
        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let keypair = checked_genesis_fixture_keypair();
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
        base.set_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(
            1_000,
        )));
        let override_instruction = InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::MaxClockDriftMs(1_500),
        )));
        let tx = RawGenesisTx {
            parameters: Some(base),
            instructions: vec![override_instruction],
            ..RawGenesisTx::default()
        };
        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![tx],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let effective = manifest
            .effective_parameters()
            .expect("single structured parameter block");
        assert_eq!(effective.sumeragi().max_clock_drift_ms(), 1_500);
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
            instructions: vec![InstructionBox::from(SetParameter::new(
                Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333)),
            ))],
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
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![tx_manual, tx_defaults],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let effective = manifest
            .effective_parameters()
            .expect("single structured parameter block");
        assert_eq!(effective.sumeragi().max_clock_drift_ms(), 333);
    }

    #[test]
    fn multiple_structured_parameter_blocks_are_rejected_as_ambiguous_snapshots() {
        init_instruction_registry();
        use iroha_data_model::parameter::{Parameters, system::SumeragiParameter};

        let chain = ChainId::from("iroha:test:paramparse-order");

        let mut base = Parameters::default();
        base.set_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)));

        let mut updated = base.clone();
        updated.set_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333)));

        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![
                RawGenesisTx {
                    parameters: Some(base),
                    ..RawGenesisTx::default()
                },
                RawGenesisTx::default(),
                RawGenesisTx {
                    parameters: Some(updated),
                    ..RawGenesisTx::default()
                },
            ],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let error = manifest
            .effective_parameters()
            .expect_err("multiple complete parameter snapshots must be rejected");
        assert!(
            error
                .to_string()
                .contains("multiple structured `parameters` blocks")
        );

        let error = manifest
            .clone()
            .parse()
            .expect_err("signing parse must reject ambiguous parameter snapshots");
        assert!(
            error
                .to_string()
                .contains("multiple structured `parameters` blocks")
        );

        let value = norito::json::to_value(&manifest).expect("serialize adversarial manifest");
        let error = RawGenesisTransaction::from_json_value(value)
            .expect_err("JSON admission must reject ambiguous parameter snapshots");
        assert!(
            error
                .to_string()
                .contains("multiple structured `parameters` blocks")
        );
    }

    #[test]
    #[ignore = "debug helper for inspecting parsed genesis instruction order"]
    fn debug_dump_set_parameter_order_for_manifest_path() -> Result<()> {
        use std::env;

        init_instruction_registry();

        let path = env::var("IROHA_DEBUG_GENESIS_PATH")
            .wrap_err("IROHA_DEBUG_GENESIS_PATH must point to a genesis manifest JSON")?;
        let manifest = RawGenesisTransaction::from_path(&path)?;
        let batches = manifest.parse()?;

        eprintln!("manifest={path}");
        for (batch_idx, batch) in batches.iter().enumerate() {
            eprintln!("BATCH {batch_idx}");
            for (instr_idx, instruction) in batch.iter().enumerate() {
                let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>()
                else {
                    continue;
                };
                eprintln!("  {instr_idx}: {:?}", set_parameter.inner());
            }
        }

        Ok(())
    }

    #[test]
    #[ignore = "debug helper for inspecting signed genesis instruction order"]
    fn debug_dump_set_parameter_order_for_signed_genesis_path() -> Result<()> {
        use std::{env, fs};

        use iroha_data_model::{block::decode_framed_signed_block, transaction::Executable};

        init_instruction_registry();

        let path = env::var("IROHA_DEBUG_SIGNED_GENESIS_PATH")
            .wrap_err("IROHA_DEBUG_SIGNED_GENESIS_PATH must point to a signed genesis .nrt")?;
        let bytes = fs::read(&path).wrap_err_with(|| format!("read signed genesis {path}"))?;
        let block = decode_framed_signed_block(&bytes)
            .wrap_err_with(|| format!("decode signed genesis {path}"))?;

        eprintln!("signed_genesis={path}");
        for (batch_idx, tx) in block.external_transactions().enumerate() {
            let Executable::Instructions(batch) = tx.instructions() else {
                eprintln!("BATCH {batch_idx} <non-instruction-executable>");
                continue;
            };
            eprintln!("BATCH {batch_idx}");
            for (instr_idx, instruction) in batch.iter().enumerate() {
                if let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() {
                    eprintln!("  {instr_idx}: {:?}", set_parameter.inner());
                }
            }
        }

        Ok(())
    }

    #[test]
    #[ignore = "debug helper for inspecting build_and_sign instruction order before encoding"]
    fn debug_dump_set_parameter_order_for_built_manifest_path() -> Result<()> {
        use std::env;

        use iroha_data_model::transaction::Executable;

        init_instruction_registry();

        let path = env::var("IROHA_DEBUG_GENESIS_PATH")
            .wrap_err("IROHA_DEBUG_GENESIS_PATH must point to a genesis manifest JSON")?;
        let manifest = RawGenesisTransaction::from_path(&path)?;
        let block = manifest.build_and_sign(&checked_genesis_fixture_keypair())?;

        eprintln!("built_manifest={path}");
        for (batch_idx, tx) in block.0.external_transactions().enumerate() {
            let Executable::Instructions(batch) = tx.instructions() else {
                eprintln!("BATCH {batch_idx} <non-instruction-executable>");
                continue;
            };
            eprintln!("BATCH {batch_idx}");
            for (instr_idx, instruction) in batch.iter().enumerate() {
                if let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() {
                    eprintln!("  {instr_idx}: {:?}", set_parameter.inner());
                }
            }
        }

        Ok(())
    }

    #[test]
    #[ignore = "debug helper for inspecting build_and_sign instruction order after encode_wire roundtrip"]
    fn debug_dump_set_parameter_order_for_encoded_manifest_path() -> Result<()> {
        use std::env;

        use iroha_data_model::{block::decode_framed_signed_block, transaction::Executable};

        init_instruction_registry();

        let path = env::var("IROHA_DEBUG_GENESIS_PATH")
            .wrap_err("IROHA_DEBUG_GENESIS_PATH must point to a genesis manifest JSON")?;
        let manifest = RawGenesisTransaction::from_path(&path)?;
        let block = manifest.build_and_sign(&checked_genesis_fixture_keypair())?;
        let encoded = block.0.encode_wire()?;
        let decoded = decode_framed_signed_block(&encoded)?;

        eprintln!("encoded_manifest={path}");
        for (batch_idx, tx) in decoded.external_transactions().enumerate() {
            let Executable::Instructions(batch) = tx.instructions() else {
                eprintln!("BATCH {batch_idx} <non-instruction-executable>");
                continue;
            };
            eprintln!("BATCH {batch_idx}");
            for (instr_idx, instruction) in batch.iter().enumerate() {
                if let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() {
                    eprintln!("  {instr_idx}: {:?}", set_parameter.inner());
                }
            }
        }

        Ok(())
    }

    #[test]
    fn set_parameter_inside_instructions_is_rejected() {
        init_instruction_registry();
        use iroha_data_model::parameter::system::SumeragiParameter;

        let set_param = InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::MaxClockDriftMs(1_000),
        )));
        let instructions = genesis_instructions_json::instructions_to_value(&[set_param]);

        let mut tx_map = norito::json::Map::new();
        tx_map.insert("instructions".to_string(), instructions);

        let mut manifest_fields = norito::json::Map::new();
        manifest_fields.insert(
            "chain".to_string(),
            norito::json::Value::String("test-chain".into()),
        );
        manifest_fields.insert(
            "chain_discriminant".to_string(),
            manifest_chain_discriminant_value(),
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
        manifest_fields.insert("sumeragi_v2".to_string(), manifest_v2_context_value());
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
        manifest_fields.insert(
            "chain_discriminant".to_string(),
            manifest_chain_discriminant_value(),
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
    fn raw_genesis_requires_chain_discriminant() {
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
            "consensus_mode".to_string(),
            norito::json::Value::String("Permissioned".into()),
        );
        manifest_fields.insert("sumeragi_v2".to_string(), manifest_v2_context_value());
        manifest_fields.insert(
            "transactions".to_string(),
            norito::json::Value::Array(vec![norito::json::Value::Object(norito::json::Map::new())]),
        );

        let manifest = norito::json::Value::Object(manifest_fields);
        let err = RawGenesisTransaction::from_json_value(manifest)
            .expect_err("missing chain_discriminant should be rejected");
        assert!(
            err.to_string().contains("chain_discriminant"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn raw_genesis_roundtrip_uses_manifest_chain_discriminant_for_account_literals() -> Result<()> {
        init_instruction_registry();
        let _chain = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:testnet-prefix"),
            PathBuf::from("."),
        )
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned)
        .with_chain_discriminant(369);

        let json = norito::json::to_json(&manifest)?;
        let decoded: RawGenesisTransaction = norito::json::from_str(&json)?;
        assert_eq!(decoded.chain_discriminant(), 369);
        Ok(())
    }

    #[test]
    fn topology_entries_parse_with_pop_hex() {
        init_instruction_registry();
        let peer = PeerId::new(checked_genesis_fixture_keypair().public_key().clone());
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
        manifest_fields.insert(
            "chain_discriminant".to_string(),
            manifest_chain_discriminant_value(),
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
        manifest_fields.insert("sumeragi_v2".to_string(), manifest_v2_context_value());
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
        let (peer_pk, _) = checked_genesis_fixture_keypair().into_parts();
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
        let peer = PeerId::new(checked_genesis_fixture_keypair().public_key().clone());
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
        manifest_fields.insert(
            "chain_discriminant".to_string(),
            manifest_chain_discriminant_value(),
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
        manifest_fields.insert("sumeragi_v2".to_string(), manifest_v2_context_value());
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
        let peer = PeerId::new(checked_genesis_fixture_keypair().public_key().clone());
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
        manifest_fields.insert(
            "chain_discriminant".to_string(),
            manifest_chain_discriminant_value(),
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
        manifest_fields.insert("sumeragi_v2".to_string(), manifest_v2_context_value());
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
        let peer_a = PeerId::new(checked_genesis_fixture_keypair().public_key().clone());
        let peer_b = PeerId::new(checked_genesis_fixture_keypair().public_key().clone());
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
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: 1,
            consensus_fingerprint: Some(ConsensusFingerprint::new([0xAB; 32])),
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let rebuilt = manifest
            .clone()
            .into_builder()
            .domain(DomainId::try_new("example", "universal").expect("domain id"))
            .finish_domain()
            .build_raw();

        assert_eq!(rebuilt.consensus_mode, manifest.consensus_mode);
        assert_eq!(
            rebuilt.wire_protocol_version,
            manifest.wire_protocol_version
        );
        assert_eq!(
            rebuilt.consensus_fingerprint,
            manifest.consensus_fingerprint
        );
        assert_eq!(rebuilt.sumeragi_v2, manifest.sumeragi_v2);
    }

    #[test]
    fn raw_v2_genesis_requires_signed_context_parameters() {
        let manifest = RawGenesisTransaction {
            chain: ChainId::from("iroha:test:missing-v2-context"),
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };
        let mut value = norito::json::value::to_value(&manifest).expect("serialize manifest");
        value
            .as_object_mut()
            .expect("manifest object")
            .remove("sumeragi_v2");
        let error = RawGenesisTransaction::from_json_value(value)
            .expect_err("v2 context parameters are required");
        assert!(
            error.to_string().contains("sumeragi_v2"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn raw_genesis_rejects_retired_and_malformed_consensus_manifest_shapes() {
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:strict-consensus-manifest"),
            ".",
        )
        .build_raw()
        .with_consensus_meta();
        let base = norito::json::to_value(&manifest).expect("serialize strict manifest");
        let protocol_version_array =
            norito::json::value::to_value(&vec![CONSENSUS_PROTOCOL_VERSION])
                .expect("serialize invalid protocol-version array");

        let mut old_plural = base.clone();
        let map = old_plural.as_object_mut().expect("manifest object");
        map.remove("wire_protocol_version");
        map.insert(
            "wire_proto_versions".to_owned(),
            protocol_version_array.clone(),
        );
        assert!(RawGenesisTransaction::from_json_value(old_plural).is_err());

        let mut array_version = base.clone();
        array_version
            .as_object_mut()
            .expect("manifest object")
            .insert("wire_protocol_version".to_owned(), protocol_version_array);
        assert!(RawGenesisTransaction::from_json_value(array_version).is_err());

        for malformed in [
            "0xAA00000000000000000000000000000000000000000000000000000000000000",
            "0x00",
            "aa00000000000000000000000000000000000000000000000000000000000000",
        ] {
            let mut value = base.clone();
            value.as_object_mut().expect("manifest object").insert(
                "consensus_fingerprint".to_owned(),
                norito::json::Value::String(malformed.to_owned()),
            );
            assert!(
                RawGenesisTransaction::from_json_value(value).is_err(),
                "malformed fingerprint `{malformed}` must fail closed"
            );
        }

        let mut unknown = base;
        unknown
            .as_object_mut()
            .expect("manifest object")
            .insert("unknown".to_owned(), norito::json::Value::Bool(true));
        assert!(RawGenesisTransaction::from_json_value(unknown).is_err());
    }

    #[test]
    fn topology_entry_pop_bytes_none() {
        let peer = PeerId::new(checked_genesis_fixture_keypair().public_key().clone());
        let entry = GenesisTopologyEntry::from(peer);
        let pop = entry.pop_bytes().expect("pop_bytes");
        assert!(pop.is_none());
    }

    #[test]
    fn normalize_exposes_instruction_batches() {
        init_instruction_registry();

        let manifest = RawGenesisTransaction {
            chain: ChainId::from("iroha:test:normalize"),
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx::default()],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let normalized = manifest.normalize().expect("normalize");
        assert!(
            !normalized.transactions.is_empty(),
            "normalize should emit at least one transaction batch"
        );
        assert_ne!(
            normalized.consensus_fingerprint,
            ConsensusFingerprint::new([0; 32]),
            "normalize should expose the computed fingerprint"
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
            let params = tx
                .effective_parameters()
                .expect("single structured parameter block");
            let npos_param_id = SumeragiNposParameters::parameter_id();
            let npos = params
                .custom()
                .get(&npos_param_id)
                .and_then(SumeragiNposParameters::from_custom_parameter)
                .expect("NPoS fixture must carry signed election parameters");
            assert_eq!(tx.consensus_mode, SumeragiConsensusMode::Npos);

            let dm_params = ConsensusGenesisParams {
                block_cadence_ms: params.sumeragi().block_cadence_ms(),
                block_max_transactions: params.block().max_transactions(),
                mode: ConsensusGenesisModeParams::Npos(NposGenesisParams {
                    epoch_length_blocks: npos.epoch_length_blocks(),
                    epoch_seed: npos.epoch_seed(),
                    vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                    vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                    max_validators: npos.max_validators(),
                    min_self_bond: npos.min_self_bond().clone(),
                    min_nomination_bond: npos.min_nomination_bond().clone(),
                    max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                    seat_band_pct: npos.seat_band_pct(),
                    max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                    finality_margin_blocks: npos.finality_margin_blocks(),
                    evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                    activation_lag_blocks: npos.activation_lag_blocks(),
                    slashing_delay_blocks: npos.slashing_delay_blocks(),
                }),
                protocol_version: iroha_config::parameters::defaults::sumeragi::PROTOCOL_VERSION,
                v2_context: tx.sumeragi_v2,
            };

            compute_consensus_fingerprint_v2(&tx.chain, &dm_params)
                .expect("canonical NPoS fixture must fingerprint")
        }

        fn build_manifest(chain: ChainId, seed_byte: u8) -> RawGenesisTransaction {
            let mut parameters = Parameters::default();
            parameters.set_parameter(DataModelParameter::Sumeragi(
                SumeragiParameter::MaxClockDriftMs(250),
            ));
            let npos = SumeragiNposParameters::default().with_epoch_seed([seed_byte; 32]);
            parameters.set_parameter(DataModelParameter::Custom(npos.into()));

            RawGenesisTransaction {
                chain,
                chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
                executor: None,
                ivm_dir: IvmPath::default(),
                transactions: vec![RawGenesisTx {
                    parameters: Some(parameters),
                    ..RawGenesisTx::default()
                }],
                consensus_mode: SumeragiConsensusMode::Npos,
                wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
                consensus_fingerprint: None,
                sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
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
        assert_eq!(
            manifest_a.consensus_fingerprint,
            Some(ConsensusFingerprint::new(expected_a))
        );
        assert_eq!(
            manifest_b.consensus_fingerprint,
            Some(ConsensusFingerprint::new(expected_b))
        );
        assert_eq!(manifest_a.consensus_mode, SumeragiConsensusMode::Npos);
        assert_eq!(manifest_a.wire_protocol_version, CONSENSUS_PROTOCOL_VERSION);
    }

    #[test]
    fn permissioned_genesis_rejects_npos_parameters() {
        use iroha_data_model::parameter::{
            Parameter as DataModelParameter, system::SumeragiConsensusMode,
        };

        let chain = ChainId::from("iroha:test:permmeta");
        let mut parameters = Parameters::default();
        let npos_defaults = SumeragiNposParameters::default();
        parameters.set_parameter(DataModelParameter::Custom(npos_defaults.into()));

        let manifest = RawGenesisTransaction {
            chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: None,
            ivm_dir: IvmPath::default(),
            transactions: vec![RawGenesisTx {
                parameters: Some(parameters),
                ..RawGenesisTx::default()
            }],
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
            crypto: ManifestCrypto::default(),
        };

        let error = manifest
            .normalize()
            .expect_err("permissioned genesis must reject NPoS election parameters");
        assert!(
            error
                .to_string()
                .contains("permissioned genesis must omit `sumeragi_npos_parameters`"),
            "unexpected error: {error:?}"
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
            .build_and_sign(&checked_genesis_fixture_keypair())
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
            .build_and_sign(&checked_genesis_fixture_keypair())
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
            .build_and_sign(&checked_genesis_fixture_keypair())
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
            .build_and_sign(&checked_genesis_fixture_keypair())
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

    /// Return the exact Sumeragi v2 context parameters selected by this
    /// manifest.
    #[must_use]
    pub const fn sumeragi_v2_context_parameters(&self) -> SumeragiV2GenesisContextParameters {
        self.sumeragi_v2
    }

    /// Replace the Sumeragi v2 context parameters that will be fingerprinted
    /// and signed with this manifest.
    #[must_use]
    pub fn with_sumeragi_v2_context_parameters(
        mut self,
        parameters: SumeragiV2GenesisContextParameters,
    ) -> Self {
        self.sumeragi_v2 = parameters;
        self
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
        let block_cadence_ms = self
            .transactions
            .iter()
            .find_map(|tx| {
                tx.parameters
                    .as_ref()
                    .map(|parameters| parameters.sumeragi.block_cadence_ms)
            })
            .unwrap_or_else(|| Parameters::default().sumeragi.block_cadence_ms);
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
            block_cadence_ms,
            consensus_mode: self.consensus_mode,
            wire_protocol_version: self.wire_protocol_version,
            consensus_fingerprint: self.consensus_fingerprint,
            sumeragi_v2: self.sumeragi_v2,
        }
    }

    /// Build and sign genesis block.
    ///
    /// # Errors
    ///
    /// Fails if `RawGenesisTransaction::parse` fails or the transaction and
    /// block timestamps cannot be represented in `u64` milliseconds.
    pub fn build_and_sign(self, genesis_key_pair: &KeyPair) -> Result<GenesisBlock> {
        self.build_and_sign_with_da_proof_policies(genesis_key_pair, None)
    }

    /// Build and sign genesis block with an explicit confidential policy hash.
    ///
    /// This does not derive the hash from the manifest. Callers that know the
    /// runtime confidential policy must compute it before signing, so the signed genesis
    /// header commits to the same policy that validators will enforce.
    ///
    /// # Errors
    ///
    /// Fails if `RawGenesisTransaction::parse` fails.
    pub fn build_and_sign_with_confidential_policy_hash(
        self,
        genesis_key_pair: &KeyPair,
        confidential_policy_hash: Option<[u8; 32]>,
    ) -> Result<GenesisBlock> {
        self.build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
            genesis_key_pair,
            None,
            confidential_policy_hash,
        )
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
        self.build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
            genesis_key_pair,
            da_proof_policies,
            None,
        )
    }

    /// Build and sign genesis block, overriding DA proof policies and the confidential policy hash.
    ///
    /// # Errors
    ///
    /// Fails if `RawGenesisTransaction::parse` fails.
    pub fn build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
        self,
        genesis_key_pair: &KeyPair,
        da_proof_policies: Option<DaProofPolicyBundle>,
        confidential_policy_hash: Option<[u8; 32]>,
    ) -> Result<GenesisBlock> {
        let genesis_creation_base_ms: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("system clock is before UNIX_EPOCH")?
            .as_millis()
            .try_into()
            .wrap_err("current UNIX timestamp does not fit into u64 milliseconds")?;
        self.build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
            genesis_key_pair,
            da_proof_policies,
            confidential_policy_hash,
            genesis_creation_base_ms,
        )
    }

    /// Build and sign genesis with explicit DA/confidential policy commitments
    /// and a deterministic transaction creation-time base.
    ///
    /// Transaction `i` receives `creation_time_base_ms + i`; the genesis block
    /// timestamp remains one millisecond after the final transaction.
    ///
    /// # Errors
    ///
    /// Fails if `RawGenesisTransaction::parse` fails or the transaction and
    /// block timestamps cannot be represented in `u64` milliseconds.
    pub fn build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
        self,
        genesis_key_pair: &KeyPair,
        da_proof_policies: Option<DaProofPolicyBundle>,
        confidential_policy_hash: Option<[u8; 32]>,
        creation_time_base_ms: u64,
    ) -> Result<GenesisBlock> {
        let chain = self.chain.clone();
        let genesis_account = AccountId::new(genesis_key_pair.public_key().clone());
        let instruction_batches = self.parse()?;
        let timestamp_span = u64::try_from(instruction_batches.len())
            .wrap_err("genesis transaction count does not fit into u64")?;
        creation_time_base_ms
            .checked_add(timestamp_span)
            .ok_or_else(|| {
                eyre!(
                    "genesis creation-time base {creation_time_base_ms} cannot represent \
                     {} transactions and the block timestamp",
                    instruction_batches.len()
                )
            })?;

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
            let mut builder = TransactionBuilder::new(
                chain.clone(),
                genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(instructions);
            let tx_index =
                u64::try_from(tx_index).expect("genesis transaction count validated above");
            builder.set_creation_time(Duration::from_millis(
                creation_time_base_ms
                    .checked_add(tx_index)
                    .expect("genesis timestamp span validated above"),
            ));
            let transaction = builder
                .try_sign(genesis_key_pair.private_key())
                .wrap_err_with(|| format!("failed to sign genesis transaction batch {tx_index}"))?;
            transactions.push(transaction);
        }
        let confidential_digest = ConfidentialFeatureDigest::new(
            None,
            None,
            None,
            Some(RULES_VERSION),
            Some(confidential_policy_hash.unwrap_or(DEFAULT_GENESIS_CONFIDENTIAL_POLICY_HASH)),
        );
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
    pub fn parse(self) -> Result<Vec<Vec<InstructionBox>>> {
        self.validate_mode_specific_consensus_parameters()?;
        // Always recompute generated fields for the live Sumeragi v2 protocol,
        // so stale or externally injected handshake metadata cannot survive
        // into the signed genesis block.
        let manifest = self.with_consensus_meta();

        manifest
            .crypto
            .validate()
            .map_err(|err| eyre!("invalid crypto configuration in genesis manifest: {err}"))?;

        let block_cadence_ms = manifest.effective_parameters()?.sumeragi.block_cadence_ms;

        let RawGenesisTransaction {
            chain: _,
            chain_discriminant: _,
            executor,
            ivm_dir: _,
            mut transactions,
            consensus_mode,
            wire_protocol_version,
            consensus_fingerprint,
            sumeragi_v2,
            crypto: _,
        } = manifest;

        for tx in &mut transactions {
            tx.instructions
                .retain(|instruction| !is_consensus_handshake_metadata_instruction(instruction));
            if let Some(parameters) = &mut tx.parameters {
                let filtered_parameters = parameters_with_staging(parameters)
                    .into_iter()
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
        let meta_vec = Self::build_consensus_meta_instructions(
            consensus_mode,
            block_cadence_ms,
            wire_protocol_version,
            consensus_fingerprint,
            sumeragi_v2,
            &manual_parameters,
        )?;
        let mut pending_meta = if meta_vec.is_empty() {
            None
        } else {
            Some(meta_vec)
        };

        let mut instructions_list = Vec::new();
        let mut aggregated_parameters = Parameters::default();

        if let Some(executor_path) = executor {
            let upgrade_executor = Upgrade::new(Executor::new(executor_path.try_into()?)).into();
            instructions_list.push(vec![upgrade_executor]);
        }

        for tx in transactions {
            let mut instructions = Vec::new();

            if let Some(parameters) = tx.parameters {
                let generated = collect_parameter_instructions(
                    &parameters,
                    &tx.instructions,
                    &manual_parameters,
                    &aggregated_parameters,
                );
                for instruction in &generated {
                    if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() {
                        aggregated_parameters.set_parameter(set_param.inner().clone());
                    }
                }
                instructions.extend(generated);
            }

            if !tx.instructions.is_empty() {
                for instruction in &tx.instructions {
                    if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() {
                        aggregated_parameters.set_parameter(set_param.inner().clone());
                    }
                }
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
        consensus_mode: SumeragiConsensusMode,
        block_cadence_ms: NonZeroU64,
        wire_protocol_version: u32,
        consensus_fingerprint: Option<ConsensusFingerprint>,
        sumeragi_v2: SumeragiV2GenesisContextParameters,
        manual_parameters: &[Parameter],
    ) -> Result<Vec<InstructionBox>> {
        let mut instructions = Vec::new();
        let fingerprint = consensus_fingerprint.ok_or_else(|| {
            eyre!(
                "genesis manifest missing `consensus_fingerprint`; call `with_consensus_meta` before signing"
            )
        })?;
        let metadata = ConsensusHandshakeMetadata {
            mode: consensus_mode,
            block_cadence_ms,
            wire_protocol_version,
            consensus_fingerprint: fingerprint,
            sumeragi_v2,
        };
        metadata
            .validate()
            .map_err(|error| eyre!("invalid signed consensus handshake metadata: {error}"))?;
        let meta_value = norito::json::value::to_value(&metadata)
            .expect("serialize consensus handshake metadata to JSON");
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
    block_cadence_ms: NonZeroU64,
    consensus_mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    wire_protocol_version: u32,
    consensus_fingerprint: Option<ConsensusFingerprint>,
    sumeragi_v2: SumeragiV2GenesisContextParameters,
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
    block_cadence_ms: NonZeroU64,
    consensus_mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
    wire_protocol_version: u32,
    consensus_fingerprint: Option<ConsensusFingerprint>,
    sumeragi_v2: SumeragiV2GenesisContextParameters,
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
            block_cadence_ms: SumeragiParameters::default().block_cadence_ms,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
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
            block_cadence_ms: SumeragiParameters::default().block_cadence_ms,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            wire_protocol_version: CONSENSUS_PROTOCOL_VERSION,
            consensus_fingerprint: None,
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
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

    /// Select the exact Sumeragi v2 context parameters which will be embedded
    /// in and signed by genesis.
    #[must_use]
    pub fn with_sumeragi_v2_context_parameters(
        mut self,
        parameters: SumeragiV2GenesisContextParameters,
    ) -> Self {
        self.sumeragi_v2 = parameters;
        self
    }

    /// Select the signed immutable block cadence stored by genesis.
    #[must_use]
    pub fn with_block_cadence_ms(mut self, block_cadence_ms: NonZeroU64) -> Self {
        self.block_cadence_ms = block_cadence_ms;
        self
    }

    fn current_tx_mut(&mut self) -> &mut GenesisTxBuilder {
        self.transactions
            .last_mut()
            .expect("at least one transaction exists")
    }

    /// Entry a domain registration and transition to [`GenesisDomainBuilder`].
    pub fn domain(self, domain_id: DomainId) -> GenesisDomainBuilder {
        self.domain_with_metadata(domain_id, Metadata::default())
    }

    /// Same as [`GenesisBuilder::domain`], but attach a metadata to the domain.
    pub fn domain_with_metadata(
        mut self,
        domain_id: DomainId,
        metadata: Metadata,
    ) -> GenesisDomainBuilder {
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
            block_cadence_ms: self.block_cadence_ms,
            consensus_mode: self.consensus_mode,
            wire_protocol_version: self.wire_protocol_version,
            consensus_fingerprint: self.consensus_fingerprint,
            sumeragi_v2: self.sumeragi_v2,
        }
    }

    /// Append a parameter to the authoritative snapshot in the first transaction.
    ///
    /// [`Parameters`] is a complete snapshot rather than a transaction-local patch, so a
    /// genesis manifest must contain exactly one structured `parameters` block. Calling this
    /// method after [`Self::next_transaction`] still updates that first authoritative snapshot.
    pub fn append_parameter(mut self, parameter: Parameter) -> Self {
        self.transactions
            .first_mut()
            .expect("genesis builder always contains at least one transaction")
            .parameters
            .push(parameter);
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

    /// Finish building, sign, and produce a [`GenesisBlock`] with a confidential policy hash.
    ///
    /// # Errors
    ///
    /// Fails if internal [`RawGenesisTransaction::build_and_sign_with_confidential_policy_hash`] fails.
    pub fn build_and_sign_with_confidential_policy_hash(
        self,
        genesis_key_pair: &KeyPair,
        confidential_policy_hash: Option<[u8; 32]>,
    ) -> Result<GenesisBlock> {
        let da_proof_policies = self.da_proof_policies.clone();
        self.build_raw()
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
                genesis_key_pair,
                da_proof_policies,
                confidential_policy_hash,
            )
    }

    /// Finish building and produce a [`RawGenesisTransaction`].
    pub fn build_raw(self) -> RawGenesisTransaction {
        let mut parameter_snapshot = Parameters::default();
        let mut source_transactions = self.transactions;
        for tx in &mut source_transactions {
            for parameter in std::mem::take(&mut tx.parameters) {
                parameter_snapshot.set_parameter(parameter);
            }
        }
        parameter_snapshot.sumeragi.block_cadence_ms = self.block_cadence_ms;

        let mut transactions: Vec<_> = source_transactions
            .into_iter()
            .map(|tx| RawGenesisTx {
                parameters: None,
                instructions: tx.instructions,
                ivm_triggers: tx.ivm_triggers,
                topology: tx.topology,
            })
            .collect();
        let first = transactions
            .first_mut()
            .expect("genesis builder always contains at least one transaction");
        first.parameters = Some(parameter_snapshot);

        RawGenesisTransaction {
            chain: self.chain,
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            executor: self.executor,
            ivm_dir: self.ivm_dir.into(),
            transactions,
            consensus_mode: self.consensus_mode,
            wire_protocol_version: self.wire_protocol_version,
            consensus_fingerprint: self.consensus_fingerprint,
            sumeragi_v2: self.sumeragi_v2,
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
            block_cadence_ms: self.block_cadence_ms,
            consensus_mode: self.consensus_mode,
            wire_protocol_version: self.wire_protocol_version,
            consensus_fingerprint: self.consensus_fingerprint,
            sumeragi_v2: self.sumeragi_v2,
        }
    }

    /// Add an account to this domain.
    pub fn account(self, signatory: PublicKey) -> Self {
        self.account_with_metadata(signatory, Metadata::default())
    }

    /// Add an account (having provided `metadata`) to this domain.
    pub fn account_with_metadata(mut self, signatory: PublicKey, metadata: Metadata) -> Self {
        let account_id = AccountId::new(signatory);
        let register = Register::account(Account::new(account_id.clone()).with_metadata(metadata));
        self.current_tx_mut().instructions.push(register.into());
        self
    }

    /// Add [`AssetDefinition`] to this domain.
    pub fn asset(mut self, asset_name: Name, asset_spec: NumericSpec) -> Self {
        let asset_display_name = asset_name.to_string();
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(self.domain_id.clone(), asset_name);
        let asset_definition = AssetDefinition::new(
            asset_definition_id,
            asset_display_name,
            asset_spec,
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        );
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
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
        Action::new(
            IvmBytecode::try_from(value.executable)?,
            value.repeats,
            value.authority,
            value.filter,
        )
        .map_err(Into::into)
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
        let sumeragi_v2 =
            norito::json::to_json(&SumeragiV2GenesisContextParameters::recommended())?;
        let genesis = format!(
            r#"{{"chain":"00000000-0000-0000-0000-000000000000","chain_discriminant":{},"executor":"{}","consensus_mode":"Permissioned","sumeragi_v2":{},"transactions":[{{}}]}}"#,
            iroha_data_model::account::address::chain_discriminant(),
            executor_path.file_name().unwrap().to_str().unwrap(),
            sumeragi_v2,
        );
        let genesis_path = tmp_dir.path().join("genesis.json");
        std::fs::write(&genesis_path, genesis).unwrap();
        let kp = checked_genesis_fixture_keypair();
        RawGenesisTransaction::from_path(&genesis_path)?.build_and_sign(&kp)?;
        Ok(())
    }

    #[test]
    fn parse_genesis_accepts_structured_accounts_without_selector_bootstrap() -> Result<()> {
        init_instruction_registry();

        let (tmp_dir, builder) = test_builder();
        let (public_key, _) = checked_genesis_fixture_keypair().into_parts();
        let domain_name: Name = "wonderland".parse()?;
        let account_id = AccountId::new(public_key.clone());
        let domain_id = DomainId::try_new(&domain_name, "universal")?;

        let genesis = builder
            .domain(domain_id)
            .account(public_key)
            .finish_domain()
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned);
        let json = norito::json::to_json_pretty(&genesis)?;
        assert!(
            json.contains(&account_id.to_string()),
            "expected i105 account id in genesis JSON"
        );
        let genesis_path = tmp_dir.path().join("genesis.json");
        std::fs::write(&genesis_path, json)?;
        RawGenesisTransaction::from_path(&genesis_path)?;
        Ok(())
    }

    #[test]
    fn parse_genesis_accepts_legacy_public_key_account_literals() -> Result<()> {
        init_instruction_registry();

        let public_key_literal = ALICE_KEYPAIR.public_key().to_string();
        let account_id = AccountId::new(ALICE_KEYPAIR.public_key().clone());
        let sumeragi_v2 =
            norito::json::to_json(&SumeragiV2GenesisContextParameters::recommended())?;
        let genesis = format!(
            r#"{{
                "chain":"00000000-0000-0000-0000-000000000000",
                "chain_discriminant":{},
                "executor":null,
                "ivm_dir":".",
                "consensus_mode":"Permissioned",
                "sumeragi_v2":{},
                "transactions":[{{
                    "instructions":[{{"Register":{{"Account":{{"id":"{public_key_literal}","metadata":{{}},"label":null,"uaid":null}}}}}}]
                }}]
            }}"#,
            iroha_data_model::account::address::chain_discriminant(),
            sumeragi_v2,
        );

        let decoded: RawGenesisTransaction = norito::json::from_str(&genesis)?;
        let account = decoded.transactions[0].instructions[0]
            .as_any()
            .downcast_ref::<RegisterBox>()
            .and_then(|register| match register {
                RegisterBox::Account(account) => Some(account.object().id().clone()),
                _ => None,
            })
            .expect("account registration");
        assert_eq!(account, account_id);

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
        manifest.consensus_fingerprint = Some(ConsensusFingerprint::new([0xDE; 32]));

        let genesis = manifest.build_and_sign(&checked_genesis_fixture_keypair())?;
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
        assert_eq!(got, expected.to_string());
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
    fn shipped_genesis_assets_have_non_blank_human_names() -> Result<()> {
        use iroha_data_model::asset::definition::validate_asset_name;

        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifests = [
            repo_root.join("defaults/genesis.json"),
            repo_root.join("defaults/nexus/genesis.json"),
            repo_root.join("defaults/kagami/iroha3-dev/genesis.json"),
            repo_root.join("defaults/kagami/iroha3-nexus/genesis.json"),
            repo_root.join("defaults/kagami/iroha3-taira/genesis.json"),
            repo_root.join("configs/soranexus/nexus/genesis.json"),
            repo_root.join("configs/soranexus/taira/genesis.json"),
        ];

        for manifest_path in manifests {
            let raw = std::fs::read_to_string(&manifest_path)?;
            let value = norito::json::parse_value(&raw)?;
            let transactions = value
                .get("transactions")
                .and_then(norito::json::Value::as_array)
                .ok_or_else(|| eyre!("{} missing transactions array", manifest_path.display()))?;
            for instruction in transactions
                .iter()
                .filter_map(|tx| tx.get("instructions"))
                .filter_map(norito::json::Value::as_array)
                .flatten()
            {
                let Some(asset_definition) = instruction
                    .get("Register")
                    .and_then(|register| register.get("AssetDefinition"))
                else {
                    continue;
                };
                let name = asset_definition
                    .get("name")
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or_default();
                let id = asset_definition
                    .get("id")
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or("<missing-id>");
                validate_asset_name(name).map_err(|err| {
                    eyre!(
                        "{} contains invalid asset definition `{}`: {}",
                        manifest_path.display(),
                        id,
                        err
                    )
                })?;
            }
        }

        Ok(())
    }

    #[test]
    fn shipped_public_genesis_manifests_do_not_fake_public_xor() -> Result<()> {
        use std::collections::BTreeSet;

        const PUBLIC_TAIRA_XOR_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
        const PUBLIC_XOR_ALIAS: &str = "xor#universal";

        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifests = [
            (
                repo_root.join("defaults/kagami/iroha3-taira/genesis.json"),
                true,
            ),
            (repo_root.join("configs/soranexus/taira/genesis.json"), true),
            (repo_root.join("defaults/nexus/genesis.json"), false),
            (
                repo_root.join("configs/soranexus/nexus/genesis.json"),
                false,
            ),
        ];

        for (manifest_path, requires_taira_xor_id) in manifests {
            let raw = std::fs::read_to_string(&manifest_path)?;
            let value = norito::json::parse_value(&raw)?;
            let transactions = value
                .get("transactions")
                .and_then(norito::json::Value::as_array)
                .ok_or_else(|| eyre!("{} missing transactions array", manifest_path.display()))?;
            let mut registered_asset_ids = BTreeSet::new();
            let mut public_xor_binding = None;
            for instruction in transactions
                .iter()
                .filter_map(|tx| tx.get("instructions"))
                .filter_map(norito::json::Value::as_array)
                .flatten()
            {
                if let Some(id) = instruction
                    .get("Register")
                    .and_then(|register| register.get("AssetDefinition"))
                    .and_then(|asset| asset.get("id"))
                    .and_then(norito::json::Value::as_str)
                {
                    if id.starts_with("xor#") {
                        return Err(eyre!(
                            "{} registers alias-shaped public XOR asset definition id `{id}`; register a canonical Base58 id and bind `{PUBLIC_XOR_ALIAS}` instead",
                            manifest_path.display()
                        ));
                    }
                    registered_asset_ids.insert(id.to_owned());
                }

                let Some(binding) = instruction.get("SetAssetDefinitionAlias") else {
                    continue;
                };
                if binding.get("alias").and_then(norito::json::Value::as_str)
                    == Some(PUBLIC_XOR_ALIAS)
                {
                    let target = binding
                        .get("asset_definition_id")
                        .and_then(norito::json::Value::as_str)
                        .ok_or_else(|| {
                            eyre!(
                                "{} binds `{PUBLIC_XOR_ALIAS}` without asset_definition_id",
                                manifest_path.display()
                            )
                        })?;
                    public_xor_binding = Some(target.to_owned());
                }
            }

            if let Some(target) = public_xor_binding {
                if target.starts_with("xor#") {
                    return Err(eyre!(
                        "{} binds `{PUBLIC_XOR_ALIAS}` to alias-shaped asset definition id `{target}`",
                        manifest_path.display()
                    ));
                }
                if !registered_asset_ids.contains(&target) {
                    return Err(eyre!(
                        "{} binds `{PUBLIC_XOR_ALIAS}` to `{target}` without registering that canonical asset",
                        manifest_path.display()
                    ));
                }
                if requires_taira_xor_id && target != PUBLIC_TAIRA_XOR_ID {
                    return Err(eyre!(
                        "{} must bind `{PUBLIC_XOR_ALIAS}` to live Taira XOR `{PUBLIC_TAIRA_XOR_ID}`, found `{target}`",
                        manifest_path.display()
                    ));
                }
            } else if requires_taira_xor_id {
                return Err(eyre!(
                    "{} must bind `{PUBLIC_XOR_ALIAS}` to live Taira XOR `{PUBLIC_TAIRA_XOR_ID}`",
                    manifest_path.display()
                ));
            }
        }

        Ok(())
    }

    #[test]
    fn shipped_taira_genesis_binds_sorafs_appeal_xor_at_scale_nine() -> Result<()> {
        const SORA_XOR_ID: &str = "61CtjvNd9T3THAR65GsMVHr82Bjc";
        const SORA_XOR_ALIAS: &str = "xor#sora";
        const SORA_XOR_SCALE: u64 = 9;

        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for manifest_path in [
            repo_root.join("defaults/kagami/iroha3-taira/genesis.json"),
            repo_root.join("configs/soranexus/taira/genesis.json"),
        ] {
            let raw = std::fs::read_to_string(&manifest_path)?;
            let value = norito::json::parse_value(&raw)?;
            let transactions = value
                .get("transactions")
                .and_then(norito::json::Value::as_array)
                .ok_or_else(|| eyre!("{} missing transactions array", manifest_path.display()))?;
            let mut sora_xor_registered = false;
            let mut sora_xor_scale = None;
            let mut sora_xor_binding = None;

            for instruction in transactions
                .iter()
                .filter_map(|tx| tx.get("instructions"))
                .filter_map(norito::json::Value::as_array)
                .flatten()
            {
                if let Some(asset_definition) = instruction
                    .get("Register")
                    .and_then(|register| register.get("AssetDefinition"))
                    && asset_definition
                        .get("id")
                        .and_then(norito::json::Value::as_str)
                        == Some(SORA_XOR_ID)
                {
                    if sora_xor_registered {
                        return Err(eyre!(
                            "{} registers governed Sora XOR `{SORA_XOR_ID}` more than once",
                            manifest_path.display()
                        ));
                    }
                    sora_xor_registered = true;
                    sora_xor_scale = asset_definition
                        .get("spec")
                        .and_then(|spec| spec.get("scale"))
                        .and_then(norito::json::Value::as_u64);
                }

                let Some(binding) = instruction.get("SetAssetDefinitionAlias") else {
                    continue;
                };
                if binding.get("alias").and_then(norito::json::Value::as_str)
                    != Some(SORA_XOR_ALIAS)
                {
                    continue;
                }
                if sora_xor_binding.is_some() {
                    return Err(eyre!(
                        "{} binds governed Sora XOR alias `{SORA_XOR_ALIAS}` more than once",
                        manifest_path.display()
                    ));
                }
                sora_xor_binding = binding
                    .get("asset_definition_id")
                    .and_then(norito::json::Value::as_str);
            }

            assert_eq!(
                sora_xor_binding,
                Some(SORA_XOR_ID),
                "{} must bind governed appeal asset `{SORA_XOR_ALIAS}` to `{SORA_XOR_ID}`",
                manifest_path.display()
            );
            assert_eq!(
                sora_xor_scale,
                Some(SORA_XOR_SCALE),
                "{} must register governed appeal asset `{SORA_XOR_ID}` at fixed scale {SORA_XOR_SCALE}; reseed pre-release state instead of mutating a live chain",
                manifest_path.display()
            );
        }

        Ok(())
    }

    #[test]
    fn shipped_genesis_manifests_advertise_current_npos_crypto_caps() -> Result<()> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifests = [
            repo_root.join("defaults/genesis.json"),
            repo_root.join("defaults/nexus/genesis.json"),
            repo_root.join("defaults/kagami/iroha3-dev/genesis.json"),
            repo_root.join("defaults/kagami/iroha3-nexus/genesis.json"),
            repo_root.join("defaults/kagami/iroha3-taira/genesis.json"),
            repo_root.join("configs/soranexus/nexus/genesis.json"),
            repo_root.join("configs/soranexus/taira/genesis.json"),
        ];

        let bls_curve = iroha_data_model::account::curve::CurveId::try_from_algorithm(
            iroha_crypto::Algorithm::BlsNormal,
        )
        .expect("bls curve id");

        for manifest_path in manifests {
            let raw = std::fs::read_to_string(&manifest_path)?;
            let value = norito::json::parse_value(&raw)?;
            let wire_protocol_version = value
                .get("wire_protocol_version")
                .and_then(norito::json::Value::as_u64)
                .ok_or_else(|| {
                    eyre!("{} missing wire_protocol_version", manifest_path.display())
                })?;
            assert_eq!(
                wire_protocol_version,
                u64::from(CONSENSUS_PROTOCOL_VERSION),
                "{} must advertise the current consensus wire protocol",
                manifest_path.display()
            );
            let crypto = value
                .get("crypto")
                .ok_or_else(|| eyre!("{} missing crypto section", manifest_path.display()))?;
            let allowed_signing = crypto
                .get("allowed_signing")
                .and_then(norito::json::Value::as_array)
                .ok_or_else(|| {
                    eyre!("{} missing crypto.allowed_signing", manifest_path.display())
                })?;
            assert!(
                allowed_signing
                    .iter()
                    .filter_map(norito::json::Value::as_str)
                    .any(|algo| algo.eq_ignore_ascii_case("bls_normal")),
                "{} must advertise bls_normal for NPoS bootstrap",
                manifest_path.display()
            );
            let allowed_curve_ids = crypto
                .get("allowed_curve_ids")
                .and_then(norito::json::Value::as_array)
                .ok_or_else(|| {
                    eyre!(
                        "{} missing crypto.allowed_curve_ids",
                        manifest_path.display()
                    )
                })?;
            assert!(
                allowed_curve_ids.iter().any(|value| {
                    value
                        .as_u64()
                        .is_some_and(|curve| curve == u64::from(bls_curve.as_u8()))
                }),
                "{} must advertise the BLS curve id for NPoS bootstrap",
                manifest_path.display()
            );
        }

        Ok(())
    }

    #[test]
    fn set_topology_pop_merges_entries() {
        let bls = checked_genesis_fixture_keypair_with_algorithm(Algorithm::BlsNormal);
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
        let (peer_pk, _) = checked_genesis_fixture_keypair().into_parts();
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
        let (peer_pk, _) = checked_genesis_fixture_keypair().into_parts();
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
            .expect("consensus fingerprint expected")
            .to_string();

        let stale_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "wire_protocol_version".to_string(),
                    norito::json::to_value(&1u32).expect("serialize protocol version"),
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
            .expect("consensus fingerprint expected")
            .to_string();

        let stale_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "wire_protocol_version".to_string(),
                    norito::json::to_value(&1u32).expect("serialize protocol version"),
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
    fn parse_recomputes_explicit_consensus_handshake_metadata() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-preserve-valid");
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        manifest.consensus_mode = SumeragiConsensusMode::Permissioned;
        manifest.wire_protocol_version = 7;
        let expected_fingerprint = manifest
            .clone()
            .with_consensus_meta()
            .consensus_fingerprint
            .expect("consensus fingerprint expected")
            .to_string();
        let explicit_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "wire_protocol_version".to_string(),
                    norito::json::to_value(&7u32).expect("serialize protocol version"),
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
                .get("wire_protocol_version")
                .and_then(norito::json::Value::as_u64),
            Some(u64::from(CONSENSUS_PROTOCOL_VERSION))
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
    fn parse_replaces_explicit_consensus_handshake_metadata_with_external_fingerprint() -> Result<()>
    {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-preserve-external-fingerprint");
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        let expected_fingerprint = manifest
            .consensus_fingerprint
            .clone()
            .expect("consensus fingerprint expected")
            .to_string();
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
                    "wire_protocol_version".to_string(),
                    norito::json::to_value(&1u32).expect("serialize protocol version"),
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
            Some(expected_fingerprint.as_str())
        );
        assert_ne!(
            payload
                .get("consensus_fingerprint")
                .and_then(norito::json::Value::as_str),
            Some(external_fingerprint)
        );
        Ok(())
    }

    #[test]
    fn parse_recomputes_explicit_consensus_handshake_metadata_in_parameters() -> Result<()> {
        init_instruction_registry();
        let chain = ChainId::from("test-consensus-meta-preserve-valid-params");
        let mut manifest = GenesisBuilder::new_without_executor(chain, ".")
            .build_raw()
            .with_consensus_meta();
        manifest.consensus_mode = SumeragiConsensusMode::Permissioned;
        manifest.wire_protocol_version = 7;
        let expected_fingerprint = manifest
            .clone()
            .with_consensus_meta()
            .consensus_fingerprint
            .expect("consensus fingerprint expected")
            .to_string();
        let explicit_param = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            Json::from_norito_value_ref(&norito::json::Value::Object({
                let mut payload = norito::json::Map::new();
                payload.insert(
                    "mode".to_string(),
                    norito::json::Value::String("Permissioned".to_string()),
                );
                payload.insert(
                    "wire_protocol_version".to_string(),
                    norito::json::to_value(&7u32).expect("serialize protocol version"),
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
                .get("wire_protocol_version")
                .and_then(norito::json::Value::as_u64),
            Some(u64::from(CONSENSUS_PROTOCOL_VERSION))
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
        let genesis_key_pair = checked_genesis_fixture_keypair();
        let (alice_public_key, _) = checked_genesis_fixture_keypair().into_parts();
        let (_tmp_dir, builder) = test_builder();

        let _genesis_block = builder
            .domain(DomainId::try_new("wonderland", "universal")?)
            .account(alice_public_key)
            .finish_domain()
            .build_and_sign(&genesis_key_pair)?;

        Ok(())
    }

    #[test]
    fn signed_block_versioned_roundtrip() -> Result<()> {
        init_instruction_registry();
        let genesis_key_pair = checked_genesis_fixture_keypair();
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
            (
                "cheshire_cat",
                checked_genesis_fixture_keypair().into_parts().0,
            ),
            (
                "mad_hatter",
                checked_genesis_fixture_keypair().into_parts().0,
            ),
        ]
        .into_iter()
        .collect();
        let (_tmp_dir, mut genesis_builder) = test_builder();
        let _executor_path = genesis_builder.executor.clone();

        genesis_builder = genesis_builder
            .domain(DomainId::try_new("wonderland", "universal").unwrap())
            .account(public_key["alice"].clone())
            .account(public_key["bob"].clone())
            .finish_domain()
            .domain(DomainId::try_new("tulgey_wood", "universal").unwrap())
            .account(public_key["cheshire_cat"].clone())
            .finish_domain()
            .domain(DomainId::try_new("meadow", "universal").unwrap())
            .account(public_key["mad_hatter"].clone())
            .asset("hats".parse().unwrap(), NumericSpec::default())
            .finish_domain();

        // In real cases executor should be constructed from an IVM bytecode blob
        let finished_genesis = genesis_builder.build_and_sign(&checked_genesis_fixture_keypair())?;

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
            let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
            assert_eq!(
                instructions[0],
                Register::domain(Domain::new(domain_id.clone())).into()
            );
            assert_eq!(
                instructions[1],
                Register::account(Account::new(
                    AccountId::new(public_key["alice"].clone()).clone()
                ))
                .into()
            );
            assert_eq!(
                instructions[2],
                Register::account(Account::new(
                    AccountId::new(public_key["bob"].clone()).clone()
                ))
                .into()
            );
        }
        {
            let domain_id: DomainId = DomainId::try_new("tulgey_wood", "universal").unwrap();
            assert_eq!(
                instructions[3],
                Register::domain(Domain::new(domain_id.clone())).into()
            );
            assert_eq!(
                instructions[4],
                Register::account(Account::new(
                    AccountId::new(public_key["cheshire_cat"].clone()).clone()
                ))
                .into()
            );
        }
        {
            let domain_id: DomainId = DomainId::try_new("meadow", "universal").unwrap();
            assert_eq!(
                instructions[5],
                Register::domain(Domain::new(domain_id.clone())).into()
            );
            assert_eq!(
                instructions[6],
                Register::account(Account::new(
                    AccountId::new(public_key["mad_hatter"].clone()).clone()
                ))
                .into()
            );
            assert_eq!(
                instructions[7],
                Register::asset_definition(AssetDefinition::numeric(
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("meadow", "universal").unwrap(),
                        "hats".parse().unwrap(),
                    ),
                    "hats".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
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
    fn build_raw_coalesces_parameters_into_one_authoritative_snapshot() -> Result<()> {
        use iroha_data_model::parameter::system::SumeragiParameter;

        init_instruction_registry();
        let raw = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:build-raw-authoritative"),
            ".",
        )
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(667)))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333)))
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned);

        let transactions = &raw.transactions;
        assert_eq!(transactions.len(), 3);
        let parameter_positions = transactions
            .iter()
            .enumerate()
            .filter_map(|(index, tx)| tx.parameters.as_ref().map(|_| index))
            .collect::<Vec<_>>();
        assert_eq!(parameter_positions, vec![0]);

        let authoritative = transactions[0]
            .parameters
            .as_ref()
            .expect("first transaction must carry the authoritative parameter snapshot");
        assert_eq!(authoritative.sumeragi().max_clock_drift_ms(), 333);
        assert!(transactions[1..].iter().all(|tx| tx.parameters.is_none()));
        assert_eq!(
            raw.effective_parameters()?.sumeragi().max_clock_drift_ms(),
            333
        );
        raw.clone().parse()?;

        let json = norito::json::to_json(&raw)?;
        let decoded: RawGenesisTransaction = norito::json::from_str(&json)?;
        let decoded_positions = decoded
            .transactions
            .iter()
            .enumerate()
            .filter_map(|(index, tx)| tx.parameters.as_ref().map(|_| index))
            .collect::<Vec<_>>();
        assert_eq!(decoded_positions, vec![0]);
        assert_eq!(
            decoded.transactions[0]
                .parameters
                .as_ref()
                .expect("decoded first transaction should carry authoritative params")
                .sumeragi()
                .max_clock_drift_ms(),
            333
        );
        assert_eq!(
            decoded
                .effective_parameters()?
                .sumeragi()
                .max_clock_drift_ms(),
            333
        );
        decoded.parse()?;

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

        let kp = checked_genesis_fixture_keypair();
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
    fn instruction_registry_decodes_register_domain_box() {
        let registry = default_instruction_registry();
        let instruction = RegisterBox::Domain(Register::domain(Domain::new(
            DomainId::try_new("test", "universal").unwrap(),
        )));
        let (payload, flags) = norito::codec::encode_with_header_flags(&instruction);
        let bytes = norito::core::frame_bare_with_header_flags::<RegisterBox>(&payload, flags)
            .expect("frame register-domain instruction");
        registry
            .decode(RegisterBox::WIRE_ID, &bytes)
            .expect("entry")
            .expect("decode register-domain instruction");
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
