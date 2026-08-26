//! Space Directory manifest representations and evaluation helpers.
use super::DataSpaceId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{asset::AssetDefinitionId, error::ParseError, name::Name};
use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonSerialize, Map, Value};
use std::{convert::TryFrom, fmt, str::FromStr};
/// Universal account identifier shared across all dataspaces.
///
/// UAIDs provide a stable capability anchor for multi-lane Nexus deployments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[repr(transparent)]
#[norito(decode_from_slice)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct UniversalAccountId(Hash);
impl UniversalAccountId {
    /// Construct a UAID from a pre-hashed value (blake2b-32, LSB set to 1).
    #[must_use]
    pub fn from_hash(hash: Hash) -> Self {
        Self(hash)
    }
    /// Borrow the underlying hash.
    #[must_use]
    pub fn as_hash(&self) -> &Hash {
        &self.0
    }
}
impl fmt::Display for UniversalAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "uaid:{}", self.0)
    }
}
impl From<Hash> for UniversalAccountId {
    fn from(value: Hash) -> Self {
        Self::from_hash(value)
    }
}
impl From<UniversalAccountId> for Hash {
    fn from(value: UniversalAccountId) -> Self {
        value.0
    }
}
impl FromStr for UniversalAccountId {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex_literal = s.strip_prefix("uaid:").ok_or_else(|| {
            ParseError::new("UAID must use the canonical `uaid:<lowercase-hex>` form")
        })?;
        if hex_literal.len() != Hash::LENGTH * 2
            || !hex_literal
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ParseError::new(
                "UAID must use the canonical `uaid:<lowercase-hex>` form",
            ));
        }
        let uaid = Hash::from_str(hex_literal)
            .map(Self::from_hash)
            .map_err(|_| ParseError::new("UAID hash is invalid"))?;
        if uaid.to_string() != s {
            return Err(ParseError::new(
                "UAID must use the canonical `uaid:<lowercase-hex>` form",
            ));
        }
        Ok(uaid)
    }
}
/// Canonical smart-contract identifier scoped to a dataspace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[repr(transparent)]
#[norito(decode_from_slice)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct SmartContractId(Name);
impl SmartContractId {
    /// Construct an identifier from a [`Name`].
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self(name)
    }
    /// Borrow the underlying [`Name`].
    #[must_use]
    pub fn as_name(&self) -> &Name {
        &self.0
    }
}
impl fmt::Display for SmartContractId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl From<Name> for SmartContractId {
    fn from(value: Name) -> Self {
        Self::new(value)
    }
}
impl From<SmartContractId> for Name {
    fn from(value: SmartContractId) -> Self {
        value.0
    }
}
impl FromStr for SmartContractId {
    type Err = crate::ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Name::from_str(s).map(Self::new)
    }
}
/// Manifest version supported by the Space Directory.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "version", content = "state"))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub enum ManifestVersion {
    /// First capability manifest iteration.
    #[default]
    V1,
}
impl From<ManifestVersion> for u16 {
    fn from(value: ManifestVersion) -> Self {
        match value {
            ManifestVersion::V1 => 1,
        }
    }
}
/// Capability manifest describing deterministic allowances for a UAID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct AssetPermissionManifest {
    /// Schema version used to interpret the manifest.
    pub version: ManifestVersion,
    /// Universal account identifier the manifest applies to.
    pub uaid: UniversalAccountId,
    /// Dataspace hosting the manifest.
    pub dataspace: DataSpaceId,
    /// Timestamp when the manifest was issued (milliseconds since UNIX epoch).
    pub issued_ms: u64,
    /// Epoch (inclusive) when the manifest becomes active.
    pub activation_epoch: u64,
    /// Epoch (inclusive) when the manifest expires, if scheduled.
    pub expiry_epoch: Option<u64>,
    /// Ordered manifest entries evaluated against incoming requests.
    pub entries: Vec<ManifestEntry>,
}
impl AssetPermissionManifest {
    fn ensure_epoch_active(&self, epoch: u64) -> Result<(), DenyReason> {
        if epoch < self.activation_epoch {
            return Err(DenyReason::ManifestInactive {
                epoch,
                activation_epoch: self.activation_epoch,
                expiry_epoch: self.expiry_epoch,
            });
        }
        if let Some(expiry) = self.expiry_epoch
            && epoch > expiry
        {
            return Err(DenyReason::ManifestInactive {
                epoch,
                activation_epoch: self.activation_epoch,
                expiry_epoch: self.expiry_epoch,
            });
        }
        Ok(())
    }
    /// Evaluate the manifest against a capability request, applying deny-wins semantics.
    #[must_use]
    pub fn evaluate(&self, request: &CapabilityRequest<'_>) -> ManifestVerdict {
        if request.dataspace != self.dataspace {
            return ManifestVerdict::Denied(DenyReason::NoMatchingRule);
        }
        if let Err(reason) = self.ensure_epoch_active(request.epoch) {
            return ManifestVerdict::Denied(reason);
        }
        let mut allow_candidate: Option<(usize, Allowance)> = None;
        for (idx, entry) in self.entries.iter().enumerate() {
            if !entry.scope.matches(request) {
                continue;
            }
            match &entry.effect {
                ManifestEffect::Deny(directive) => {
                    let note = directive.reason.clone().or_else(|| entry.notes.clone());
                    let entry_index = Self::clamp_entry_index(idx);
                    return ManifestVerdict::Denied(DenyReason::ExplicitRule { entry_index, note });
                }
                ManifestEffect::Allow(allowance) => {
                    allow_candidate = Some((idx, allowance.clone()));
                }
            }
        }
        if let Some((idx, allowance)) = allow_candidate {
            if let (Some(requested), Some(limit)) =
                (request.amount.as_ref(), allowance.max_amount.as_ref())
                && requested > limit
            {
                return ManifestVerdict::Denied(DenyReason::AmountExceeded {
                    requested: requested.clone(),
                    permitted: limit.clone(),
                });
            }
            let entry_index = Self::clamp_entry_index(idx);
            return ManifestVerdict::Allowed(ManifestGrant {
                entry_index,
                allowance,
            });
        }
        ManifestVerdict::Denied(DenyReason::NoMatchingRule)
    }
    fn clamp_entry_index(idx: usize) -> u32 {
        u32::try_from(idx).unwrap_or(u32::MAX)
    }
}
#[cfg(feature = "json")]
impl json::JsonSerialize for AssetPermissionManifest {
    fn json_serialize(&self, out: &mut String) {
        let value = manifest_to_json_value(self);
        value.json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        out.begin_container()?;
        out.push_str("{\"activation_epoch\":")?;
        self.activation_epoch.json_serialize_to(out)?;
        out.push_str(",\"dataspace\":")?;
        self.dataspace.as_u64().json_serialize_to(out)?;
        out.push_str(",\"entries\":")?;
        out.begin_container()?;
        out.push('[')?;
        for (index, entry) in self.entries.iter().enumerate() {
            if index != 0 {
                out.push(',')?;
            }
            entry_json_serialize_to(entry, out)?;
        }
        out.push(']')?;
        out.end_container();
        if let Some(expiry_epoch) = self.expiry_epoch {
            out.push_str(",\"expiry_epoch\":")?;
            expiry_epoch.json_serialize_to(out)?;
        }
        out.push_str(",\"issued_ms\":")?;
        self.issued_ms.json_serialize_to(out)?;
        out.push_str(",\"uaid\":")?;
        json::write_json_display_to(&self.uaid, out)?;
        out.push_str(",\"version\":")?;
        u64::from(u16::from(self.version)).json_serialize_to(out)?;
        out.push('}')?;
        out.end_container();
        Ok(())
    }
}
#[cfg(feature = "json")]
fn entry_json_serialize_to(
    entry: &ManifestEntry,
    out: &mut dyn json::JsonWriteSink,
) -> Result<(), json::BoundedJsonError> {
    // `manifest_to_json_value` uses BTreeMap objects. Keep the same sorted key
    // order without constructing an owned response-sized Value graph.
    out.begin_container()?;
    out.push_str("{\"effect\":")?;
    effect_json_serialize_to(&entry.effect, out)?;
    if let Some(notes) = &entry.notes {
        out.push_str(",\"notes\":")?;
        json::write_json_string_to(notes, out)?;
    }
    out.push_str(",\"scope\":")?;
    scope_json_serialize_to(&entry.scope, out)?;
    out.push('}')?;
    out.end_container();
    Ok(())
}
#[cfg(feature = "json")]
fn scope_json_serialize_to(
    scope: &CapabilityScope,
    out: &mut dyn json::JsonWriteSink,
) -> Result<(), json::BoundedJsonError> {
    out.begin_container()?;
    out.push('{')?;
    let mut has_field = if let Some(asset) = &scope.asset {
        out.push_str("\"asset\":")?;
        json::write_json_display_to(asset, out)?;
        true
    } else {
        false
    };
    if let Some(dataspace) = scope.dataspace {
        if has_field {
            out.push(',')?;
        }
        out.push_str("\"dataspace\":")?;
        dataspace.as_u64().json_serialize_to(out)?;
        has_field = true;
    }
    if let Some(method) = &scope.method {
        if has_field {
            out.push(',')?;
        }
        out.push_str("\"method\":")?;
        json::write_json_display_to(method, out)?;
        has_field = true;
    }
    if let Some(program) = &scope.program {
        if has_field {
            out.push(',')?;
        }
        out.push_str("\"program\":")?;
        json::write_json_display_to(program, out)?;
        has_field = true;
    }
    if let Some(role) = scope.role {
        if has_field {
            out.push(',')?;
        }
        out.push_str("\"role\":")?;
        json::write_json_string_to(role_label(role), out)?;
    }
    out.push('}')?;
    out.end_container();
    Ok(())
}
#[cfg(feature = "json")]
fn effect_json_serialize_to(
    effect: &ManifestEffect,
    out: &mut dyn json::JsonWriteSink,
) -> Result<(), json::BoundedJsonError> {
    out.begin_container()?;
    match effect {
        ManifestEffect::Allow(allowance) => {
            out.push_str("{\"Allow\":{")?;
            out.begin_container()?;
            if let Some(max_amount) = &allowance.max_amount {
                out.push_str("\"max_amount\":")?;
                json::write_json_display_to(max_amount, out)?;
                out.push(',')?;
            }
            out.push_str("\"window\":")?;
            json::write_json_string_to(window_label(allowance.window), out)?;
            out.push_str("}}")?;
            out.end_container();
        }
        ManifestEffect::Deny(directive) => {
            out.push_str("{\"Deny\":{")?;
            out.begin_container()?;
            if let Some(reason) = &directive.reason {
                out.push_str("\"reason\":")?;
                json::write_json_string_to(reason, out)?;
            }
            out.push_str("}}")?;
            out.end_container();
        }
    }
    out.end_container();
    Ok(())
}
#[cfg(feature = "json")]
impl json::JsonDeserialize for AssetPermissionManifest {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        manifest_from_json_value(&value)
    }
    fn json_from_value(value: &Value) -> Result<Self, json::Error> {
        manifest_from_json_value(value)
    }
}
#[cfg(feature = "json")]
fn manifest_to_json_value(manifest: &AssetPermissionManifest) -> Value {
    let mut root = Map::new();
    root.insert(
        "version".into(),
        Value::from(u64::from(u16::from(manifest.version))),
    );
    root.insert("uaid".into(), Value::from(manifest.uaid.to_string()));
    root.insert("dataspace".into(), Value::from(manifest.dataspace.as_u64()));
    root.insert("issued_ms".into(), Value::from(manifest.issued_ms));
    root.insert(
        "activation_epoch".into(),
        Value::from(manifest.activation_epoch),
    );
    if let Some(expiry_epoch) = manifest.expiry_epoch {
        root.insert("expiry_epoch".into(), Value::from(expiry_epoch));
    }
    let entries = manifest.entries.iter().map(entry_to_json_value).collect();
    root.insert("entries".into(), Value::Array(entries));
    Value::Object(root)
}
#[cfg(feature = "json")]
fn entry_to_json_value(entry: &ManifestEntry) -> Value {
    let mut entry_obj = Map::new();
    entry_obj.insert("scope".into(), scope_to_json_value(&entry.scope));
    entry_obj.insert("effect".into(), effect_to_json_value(&entry.effect));
    if let Some(notes) = &entry.notes {
        entry_obj.insert("notes".into(), Value::from(notes.as_str()));
    }
    Value::Object(entry_obj)
}
#[cfg(feature = "json")]
fn scope_to_json_value(scope: &CapabilityScope) -> Value {
    let mut scope_obj = Map::new();
    if let Some(dataspace) = scope.dataspace {
        scope_obj.insert("dataspace".into(), Value::from(dataspace.as_u64()));
    }
    if let Some(program) = &scope.program {
        scope_obj.insert("program".into(), Value::from(program.to_string()));
    }
    if let Some(method) = &scope.method {
        scope_obj.insert("method".into(), Value::from(method.to_string()));
    }
    if let Some(asset) = &scope.asset {
        scope_obj.insert("asset".into(), Value::from(asset.to_string()));
    }
    if let Some(role) = scope.role {
        scope_obj.insert("role".into(), Value::from(role_label(role)));
    }
    Value::Object(scope_obj)
}
#[cfg(feature = "json")]
fn role_label(role: AmxRole) -> &'static str {
    match role {
        AmxRole::Initiator => "Initiator",
        AmxRole::Participant => "Participant",
    }
}
#[cfg(feature = "json")]
fn effect_to_json_value(effect: &ManifestEffect) -> Value {
    let mut effect_obj = Map::new();
    match effect {
        ManifestEffect::Allow(allowance) => {
            let mut details = Map::new();
            if let Some(max_amount) = &allowance.max_amount {
                details.insert("max_amount".into(), Value::from(max_amount.to_string()));
            }
            details.insert("window".into(), Value::from(window_label(allowance.window)));
            effect_obj.insert("Allow".into(), Value::Object(details));
        }
        ManifestEffect::Deny(directive) => {
            let mut details = Map::new();
            if let Some(reason) = &directive.reason {
                details.insert("reason".into(), Value::from(reason.as_str()));
            }
            effect_obj.insert("Deny".into(), Value::Object(details));
        }
    }
    Value::Object(effect_obj)
}
#[cfg(feature = "json")]
fn window_label(window: AllowanceWindow) -> &'static str {
    match window {
        AllowanceWindow::PerSlot => "PerSlot",
        AllowanceWindow::PerMinute => "PerMinute",
        AllowanceWindow::PerDay => "PerDay",
    }
}
#[cfg(feature = "json")]
fn manifest_from_json_value(value: &Value) -> Result<AssetPermissionManifest, json::Error> {
    let manifest_obj = value.as_object().ok_or_else(|| json::Error::InvalidField {
        field: "manifest".into(),
        message: "manifest must be a JSON object".into(),
    })?;
    ensure_known_manifest_fields(
        manifest_obj,
        &[
            "activation_epoch",
            "dataspace",
            "entries",
            "expiry_epoch",
            "issued_ms",
            "uaid",
            "version",
        ],
        "manifest",
    )?;
    let version_value = manifest_obj
        .get("version")
        .ok_or_else(|| json::Error::missing_field("version"))?;
    let version = parse_manifest_version(version_value)?;
    let uaid_value = manifest_obj
        .get("uaid")
        .ok_or_else(|| json::Error::missing_field("uaid"))?;
    let uaid = parse_uaid_value(uaid_value)?;
    let dataspace_value = manifest_obj
        .get("dataspace")
        .ok_or_else(|| json::Error::missing_field("dataspace"))?;
    let dataspace = DataSpaceId::from(parse_u64_field(dataspace_value, "dataspace")?);
    let issued_ms = parse_u64_field(
        manifest_obj
            .get("issued_ms")
            .ok_or_else(|| json::Error::missing_field("issued_ms"))?,
        "issued_ms",
    )?;
    let activation_epoch = parse_u64_field(
        manifest_obj
            .get("activation_epoch")
            .ok_or_else(|| json::Error::missing_field("activation_epoch"))?,
        "activation_epoch",
    )?;
    let expiry_epoch = match manifest_obj.get("expiry_epoch") {
        None => None,
        Some(Value::Null) => {
            return Err(noncanonical_optional_manifest_field("expiry_epoch"));
        }
        Some(value) => Some(parse_u64_field(value, "expiry_epoch")?),
    };
    let entries_value = manifest_obj
        .get("entries")
        .ok_or_else(|| json::Error::missing_field("entries"))?;
    let entries_array = entries_value
        .as_array()
        .ok_or_else(|| json::Error::InvalidField {
            field: "entries".into(),
            message: "entries must be a JSON array".into(),
        })?;
    let entries_bytes = entries_array
        .len()
        .checked_mul(core::mem::size_of::<ManifestEntry>())
        .ok_or(json::Error::DecodeResourceLimit)?;
    reserve_manifest_decode_allocation(entries_bytes)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(entries_array.len())
        .map_err(|_| json::Error::AllocationFailed)?;
    for (idx, entry_value) in entries_array.iter().enumerate() {
        entries.push(parse_entry(entry_value, idx)?);
    }
    Ok(AssetPermissionManifest {
        version,
        uaid,
        dataspace,
        issued_ms,
        activation_epoch,
        expiry_epoch,
        entries,
    })
}
#[cfg(feature = "json")]
fn ensure_known_manifest_fields(
    object: &Map,
    allowed: &[&str],
    context: &'static str,
) -> Result<(), json::Error> {
    if object
        .keys()
        .any(|field| !allowed.contains(&field.as_str()))
    {
        return Err(json::Error::InvalidField {
            field: context.into(),
            message: "object contains an unknown field".into(),
        });
    }
    Ok(())
}
#[cfg(feature = "json")]
fn noncanonical_optional_manifest_field(field: &'static str) -> json::Error {
    json::Error::InvalidField {
        field: field.into(),
        message: "optional manifest fields must be omitted instead of null".into(),
    }
}
#[cfg(feature = "json")]
fn reserve_manifest_decode_allocation(bytes: usize) -> Result<(), json::Error> {
    norito::core::reserve_decode_allocation(bytes).map_err(json::Error::from_decode_resource)
}
#[cfg(feature = "json")]
fn parse_manifest_version(value: &Value) -> Result<ManifestVersion, json::Error> {
    let Some(raw) = value.as_u64() else {
        return Err(json::Error::InvalidField {
            field: "version".into(),
            message: "version must be an unsigned integer".into(),
        });
    };
    match raw {
        1 => Ok(ManifestVersion::V1),
        _ => Err(json::Error::InvalidField {
            field: "version".into(),
            message: "unsupported manifest version".into(),
        }),
    }
}
#[cfg(feature = "json")]
fn parse_uaid_value(value: &Value) -> Result<UniversalAccountId, json::Error> {
    let Some(text) = value.as_str() else {
        return Err(json::Error::InvalidField {
            field: "uaid".into(),
            message: "uaid must be a string".into(),
        });
    };
    let Some(hex_literal) = text.strip_prefix("uaid:") else {
        return Err(json::Error::InvalidField {
            field: "uaid".into(),
            message: "uaid must use the canonical `uaid:<lowercase-hex>` form".into(),
        });
    };
    if hex_literal.len() != Hash::LENGTH * 2
        || !hex_literal
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(json::Error::InvalidField {
            field: "uaid".into(),
            message: "uaid must use the canonical `uaid:<lowercase-hex>` form".into(),
        });
    }
    let hash = Hash::from_str(hex_literal).map_err(|_| json::Error::InvalidField {
        field: "uaid".into(),
        message: "uaid digest is invalid".into(),
    })?;
    Ok(UniversalAccountId::from_hash(hash))
}
#[cfg(feature = "json")]
fn parse_u64_field(value: &Value, field: &str) -> Result<u64, json::Error> {
    value.as_u64().ok_or_else(|| json::Error::InvalidField {
        field: field.to_string(),
        message: "value must be an unsigned integer".into(),
    })
}
#[cfg(feature = "json")]
fn parse_entry(value: &Value, idx: usize) -> Result<ManifestEntry, json::Error> {
    let entry_obj = value.as_object().ok_or_else(|| json::Error::InvalidField {
        field: format!("entries[{idx}]"),
        message: "entry must be a JSON object".into(),
    })?;
    ensure_known_manifest_fields(entry_obj, &["effect", "notes", "scope"], "manifest entry")?;
    let scope_value = entry_obj
        .get("scope")
        .ok_or_else(|| json::Error::InvalidField {
            field: format!("entries[{idx}].scope"),
            message: "missing scope object".into(),
        })?;
    let effect_value = entry_obj
        .get("effect")
        .ok_or_else(|| json::Error::InvalidField {
            field: format!("entries[{idx}].effect"),
            message: "missing effect object".into(),
        })?;
    let scope = parse_scope(scope_value, idx)?;
    let effect = parse_effect(effect_value, idx)?;
    let notes = match entry_obj.get("notes") {
        None => None,
        Some(Value::Null) => {
            return Err(noncanonical_optional_manifest_field("entries[].notes"));
        }
        Some(value @ Value::String(_)) => {
            Some(<String as json::JsonDeserialize>::json_from_value(value)?)
        }
        Some(_) => {
            return Err(json::Error::InvalidField {
                field: format!("entries[{idx}].notes"),
                message: "notes must be a string".into(),
            });
        }
    };
    Ok(ManifestEntry {
        scope,
        effect,
        notes,
    })
}
#[cfg(feature = "json")]
fn parse_scope(value: &Value, idx: usize) -> Result<CapabilityScope, json::Error> {
    let scope_obj = value.as_object().ok_or_else(|| json::Error::InvalidField {
        field: format!("entries[{idx}].scope"),
        message: "scope must be a JSON object".into(),
    })?;
    ensure_known_manifest_fields(
        scope_obj,
        &["asset", "dataspace", "method", "program", "role"],
        "manifest scope",
    )?;
    let dataspace = match scope_obj.get("dataspace") {
        None => None,
        Some(Value::Null) => {
            return Err(noncanonical_optional_manifest_field(
                "entries[].scope.dataspace",
            ));
        }
        Some(value) => Some(DataSpaceId::from(parse_u64_field(
            value,
            "entries[].scope.dataspace",
        )?)),
    };
    let program = match parse_optional_manifest_value(scope_obj, "program")? {
        Some(value) => Some(SmartContractId::new(parse_canonical_manifest_name(
            value,
            "entries[].scope.program",
        )?)),
        None => None,
    };
    let method = match parse_optional_manifest_value(scope_obj, "method")? {
        Some(value) => Some(parse_canonical_manifest_name(
            value,
            "entries[].scope.method",
        )?),
        None => None,
    };
    let asset = match parse_optional_manifest_value(scope_obj, "asset")? {
        Some(value) => {
            let Some(text) = value.as_str() else {
                return Err(json::Error::InvalidField {
                    field: "entries[].scope.asset".into(),
                    message: "asset must be a string".into(),
                });
            };
            if text.trim() != text {
                return Err(json::Error::InvalidField {
                    field: "entries[].scope.asset".into(),
                    message: "asset must use its canonical spelling".into(),
                });
            }
            Some(
                <AssetDefinitionId as json::JsonDeserialize>::json_from_value(value).map_err(
                    |error| {
                        if error.is_decode_resource_limit() {
                            error
                        } else {
                            json::Error::InvalidField {
                                field: "entries[].scope.asset".into(),
                                message: "asset must be a canonical asset definition identifier"
                                    .into(),
                            }
                        }
                    },
                )?,
            )
        }
        None => None,
    };
    let role = match scope_obj.get("role") {
        None => None,
        Some(Value::Null) => {
            return Err(noncanonical_optional_manifest_field("entries[].scope.role"));
        }
        Some(Value::String(text)) => Some(parse_role(text, idx)?),
        Some(_) => {
            return Err(json::Error::InvalidField {
                field: format!("entries[{idx}].scope.role"),
                message: "role must be a string".into(),
            });
        }
    };
    Ok(CapabilityScope {
        dataspace,
        program,
        method,
        asset,
        role,
    })
}
#[cfg(feature = "json")]
fn parse_optional_manifest_value<'a>(
    object: &'a Map,
    field: &str,
) -> Result<Option<&'a Value>, json::Error> {
    match object.get(field) {
        None => Ok(None),
        Some(Value::Null) => Err(noncanonical_optional_manifest_field(
            "entries[].scope optional field",
        )),
        Some(value) => Ok(Some(value)),
    }
}
#[cfg(feature = "json")]
fn parse_canonical_manifest_name(value: &Value, field: &'static str) -> Result<Name, json::Error> {
    let text = value.as_str().ok_or_else(|| json::Error::InvalidField {
        field: field.into(),
        message: "name must be a string".into(),
    })?;
    let name = <Name as json::JsonDeserialize>::json_from_value(value).map_err(|error| {
        if error.is_decode_resource_limit() {
            error
        } else {
            json::Error::InvalidField {
                field: field.into(),
                message: "name is invalid".into(),
            }
        }
    })?;
    if name.as_ref() != text {
        return Err(json::Error::InvalidField {
            field: field.into(),
            message: "name must use its canonical NFC spelling".into(),
        });
    }
    Ok(name)
}
#[cfg(feature = "json")]
fn parse_role(value: &str, idx: usize) -> Result<AmxRole, json::Error> {
    match value {
        "Initiator" => Ok(AmxRole::Initiator),
        "Participant" => Ok(AmxRole::Participant),
        _ => Err(json::Error::InvalidField {
            field: format!("entries[{idx}].scope.role"),
            message: "unsupported AMX role".into(),
        }),
    }
}
#[cfg(feature = "json")]
fn parse_effect(value: &Value, idx: usize) -> Result<ManifestEffect, json::Error> {
    let effect_obj = value.as_object().ok_or_else(|| json::Error::InvalidField {
        field: format!("entries[{idx}].effect"),
        message: "effect must be a JSON object".into(),
    })?;
    ensure_known_manifest_fields(effect_obj, &["Allow", "Deny"], "manifest effect decision")?;
    if effect_obj.len() != 1 {
        return Err(json::Error::InvalidField {
            field: format!("entries[{idx}].effect"),
            message: "effect must contain exactly one decision".into(),
        });
    }
    if let Some(details) = effect_obj.get("Allow") {
        return parse_allowance(details, idx).map(ManifestEffect::Allow);
    }
    if let Some(details) = effect_obj.get("Deny") {
        return parse_deny(details, idx).map(ManifestEffect::Deny);
    }
    Err(json::Error::InvalidField {
        field: format!("entries[{idx}].effect"),
        message: "effect must contain Allow or Deny".into(),
    })
}
#[cfg(feature = "json")]
fn parse_allowance(value: &Value, idx: usize) -> Result<Allowance, json::Error> {
    let details = value.as_object().ok_or_else(|| json::Error::InvalidField {
        field: format!("entries[{idx}].effect.Allow"),
        message: "Allow effect must be a JSON object".into(),
    })?;
    ensure_known_manifest_fields(details, &["max_amount", "window"], "manifest Allow effect")?;
    let window_value = details
        .get("window")
        .ok_or_else(|| json::Error::InvalidField {
            field: format!("entries[{idx}].effect.Allow.window"),
            message: "Allow effect missing window".into(),
        })?;
    let window = parse_window(window_value, idx)?;
    let max_amount = match details.get("max_amount") {
        None => None,
        Some(Value::Null) => {
            return Err(noncanonical_optional_manifest_field(
                "entries[].effect.Allow.max_amount",
            ));
        }
        Some(value) => Some(parse_quantity(value, idx)?),
    };
    Ok(Allowance { max_amount, window })
}
#[cfg(feature = "json")]
fn parse_deny(value: &Value, idx: usize) -> Result<DenyDirective, json::Error> {
    let details = value.as_object().ok_or_else(|| json::Error::InvalidField {
        field: format!("entries[{idx}].effect.Deny"),
        message: "Deny effect must be a JSON object".into(),
    })?;
    ensure_known_manifest_fields(details, &["reason"], "manifest Deny effect")?;
    let reason = match details.get("reason") {
        None => None,
        Some(Value::Null) => {
            return Err(noncanonical_optional_manifest_field(
                "entries[].effect.Deny.reason",
            ));
        }
        Some(value @ Value::String(_)) => {
            Some(<String as json::JsonDeserialize>::json_from_value(value)?)
        }
        Some(_) => {
            return Err(json::Error::InvalidField {
                field: format!("entries[{idx}].effect.Deny.reason"),
                message: "reason must be a string".into(),
            });
        }
    };
    Ok(DenyDirective { reason })
}
#[cfg(feature = "json")]
fn parse_window(value: &Value, idx: usize) -> Result<AllowanceWindow, json::Error> {
    let Some(label) = value.as_str() else {
        return Err(json::Error::InvalidField {
            field: format!("entries[{idx}].effect.Allow.window"),
            message: "window must be a string".into(),
        });
    };
    match label {
        "PerSlot" => Ok(AllowanceWindow::PerSlot),
        "PerMinute" => Ok(AllowanceWindow::PerMinute),
        "PerDay" => Ok(AllowanceWindow::PerDay),
        _ => Err(json::Error::InvalidField {
            field: format!("entries[{idx}].effect.Allow.window"),
            message: "unsupported allowance window".into(),
        }),
    }
}
#[cfg(feature = "json")]
fn parse_quantity(value: &Value, idx: usize) -> Result<Quantity, json::Error> {
    if !matches!(value, Value::String(_)) {
        return Err(json::Error::InvalidField {
            field: format!("entries[{idx}].effect.Allow.max_amount"),
            message: "max_amount must be a canonical quantity string".into(),
        });
    }
    <Quantity as json::JsonDeserialize>::json_from_value(value).map_err(|error| {
        if error.is_decode_resource_limit() {
            error
        } else {
            json::Error::InvalidField {
                field: format!("entries[{idx}].effect.Allow.max_amount"),
                message: "max_amount must be a canonical non-negative quantity string".into(),
            }
        }
    })
}
/// Manifest entry describing a scoped allow/deny rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct ManifestEntry {
    /// Capability scope matcher.
    pub scope: CapabilityScope,
    /// Allow/deny decision applied when the scope matches the request.
    pub effect: ManifestEffect,
    /// Optional operator-facing notes for logging/auditing.
    pub notes: Option<String>,
}
/// AMX role enforced by a manifest entry.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(decode_from_slice)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "role", content = "details"))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub enum AmxRole {
    /// Transaction initiator (root of the AMX graph).
    Initiator,
    /// Participant leg in an AMX group.
    Participant,
}
/// Scope definition that determines whether a manifest entry matches a capability request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct CapabilityScope {
    /// Optional dataspace selector (defaults to manifest dataspace when omitted).
    pub dataspace: Option<DataSpaceId>,
    /// Optional smart-contract identifier constraint.
    pub program: Option<SmartContractId>,
    /// Optional method/entry-point constraint.
    pub method: Option<Name>,
    /// Optional asset definition constraint.
    pub asset: Option<AssetDefinitionId>,
    /// Optional AMX role requirement.
    pub role: Option<AmxRole>,
}
impl CapabilityScope {
    fn matches(&self, request: &CapabilityRequest<'_>) -> bool {
        if let Some(dataspace) = self.dataspace
            && request.dataspace != dataspace
        {
            return false;
        }
        if let Some(program) = &self.program {
            match request.program {
                Some(candidate) if candidate == program => {}
                _ => return false,
            }
        }
        if let Some(method) = &self.method {
            match request.method {
                Some(candidate) if candidate == method => {}
                _ => return false,
            }
        }
        if let Some(asset) = &self.asset {
            match request.asset {
                Some(candidate) if candidate == asset => {}
                _ => return false,
            }
        }
        if let Some(role) = self.role
            && request.role != Some(role)
        {
            return false;
        }
        true
    }
}
/// Decision encoded by a manifest entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(feature = "json", norito(tag = "decision", content = "details"))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub enum ManifestEffect {
    /// Allow the scoped capability subject to the provided allowance.
    Allow(Allowance),
    /// Deny the scoped capability with an optional reason.
    Deny(DenyDirective),
}
/// Allowance constraints attached to an `Allow` entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct Allowance {
    /// Optional deterministic amount cap enforced by the host.
    pub max_amount: Option<Quantity>,
    /// Accounting window applied to the allowance.
    pub window: AllowanceWindow,
}
/// Allowance accounting window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "window", content = "details"))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub enum AllowanceWindow {
    /// Per-slot accounting window.
    PerSlot,
    /// Rolling per-minute allowance.
    PerMinute,
    /// Rolling per-day allowance.
    PerDay,
}
impl AllowanceWindow {
    /// Millisecond duration of the accounting window.
    #[must_use]
    pub const fn duration_ms(self) -> u64 {
        match self {
            Self::PerSlot => 1_000,
            Self::PerMinute => 60_000,
            Self::PerDay => 86_400_000,
        }
    }
}
/// Deny directive metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct DenyDirective {
    /// Optional reason recorded for the deny rule.
    pub reason: Option<String>,
}
/// Capability request evaluated against a manifest.
#[derive(Debug, Clone)]
pub struct CapabilityRequest<'a> {
    /// Dataspace the request targets.
    pub dataspace: DataSpaceId,
    /// Optional smart-contract identifier.
    pub program: Option<&'a SmartContractId>,
    /// Optional entry-point name.
    pub method: Option<&'a Name>,
    /// Optional asset definition identifier.
    pub asset: Option<&'a AssetDefinitionId>,
    /// Optional AMX role associated with the request.
    pub role: Option<AmxRole>,
    /// Amount requested for the capability, when applicable.
    pub amount: Option<Quantity>,
    /// Epoch associated with the request.
    pub epoch: u64,
}
impl<'a> CapabilityRequest<'a> {
    /// Construct a new capability request helper.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dataspace: DataSpaceId,
        program: Option<&'a SmartContractId>,
        method: Option<&'a Name>,
        asset: Option<&'a AssetDefinitionId>,
        role: Option<AmxRole>,
        amount: Option<Quantity>,
        epoch: u64,
    ) -> Self {
        Self {
            dataspace,
            program,
            method,
            asset,
            role,
            amount,
            epoch,
        }
    }
}
/// Result of evaluating a manifest against a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestVerdict {
    /// Request satisfied by the manifest.
    Allowed(ManifestGrant),
    /// Request denied with the provided reason.
    Denied(DenyReason),
}
/// Allowance grant returned on successful evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestGrant {
    /// Manifest entry index that matched.
    pub entry_index: u32,
    /// Allowance metadata applied to the request.
    pub allowance: Allowance,
}
/// Deny reason emitted during manifest evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    /// Manifest is not yet active or has expired.
    ManifestInactive {
        /// Request epoch.
        epoch: u64,
        /// Activation epoch.
        activation_epoch: u64,
        /// Optional expiry epoch.
        expiry_epoch: Option<u64>,
    },
    /// Explicit deny rule matched the request.
    ExplicitRule {
        /// Entry index that triggered the deny.
        entry_index: u32,
        /// Optional human-readable note attached to the deny entry.
        note: Option<String>,
    },
    /// Requested amount exceeds the deterministic allowance.
    AmountExceeded {
        /// Amount requested by the capability.
        requested: Quantity,
        /// Allowance threshold.
        permitted: Quantity,
    },
    /// Manifest did not contain a matching allow rule.
    NoMatchingRule,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainId;
    use iroha_primitives::numeric::Numeric;
    #[cfg(feature = "json")]
    use norito::json::JsonDeserialize;
    use std::{fs, path::Path};
    #[derive(Encode)]
    struct ForgedAllowance {
        max_amount: Option<Numeric>,
        window: AllowanceWindow,
    }
    #[test]
    fn allowance_rejects_forged_negative_quantity() {
        let forged = ForgedAllowance {
            max_amount: Some(Numeric::new(-1_i32, 0)),
            window: AllowanceWindow::PerDay,
        };
        let encoded = forged.encode();
        let mut input = encoded.as_slice();
        assert!(
            <Allowance as Decode>::decode(&mut input).is_err(),
            "manifest allowance decoding must enforce the Quantity sign invariant"
        );
    }
    #[test]
    fn first_release_manifest_binary_roundtrip_keeps_all_fields() {
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace: DataSpaceId::new(7),
            issued_ms: 11,
            activation_epoch: 13,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let encoded = manifest.encode();
        let mut input = encoded.as_slice();
        let decoded = AssetPermissionManifest::decode(&mut input)
            .expect("complete first-release manifest must decode");
        assert_eq!(decoded, manifest);
        assert!(
            input.is_empty(),
            "manifest decoder must consume the exact payload"
        );
    }
    fn sample_uaid() -> UniversalAccountId {
        UniversalAccountId::from_hash(Hash::new(b"uaid::sample"))
    }
    fn sample_name(value: &str) -> Name {
        value.parse().expect("valid name")
    }
    #[test]
    fn uaid_from_str_accepts_only_canonical_literal() {
        let uaid = sample_uaid();
        let hex = uaid.as_hash().to_string();
        let parsed_literal =
            UniversalAccountId::from_str(&format!("uaid:{hex}")).expect("uaid literal must parse");
        let parsed_display = uaid
            .to_string()
            .parse::<UniversalAccountId>()
            .expect("display uaid must parse");
        assert_eq!(parsed_literal, uaid);
        assert_eq!(parsed_display, uaid);
    }
    #[test]
    fn uaid_from_str_rejects_noncanonical_spellings() {
        let uaid = sample_uaid();
        let hex = uaid.as_hash().to_string();
        for literal in [
            hex.clone(),
            format!("UAID:{hex}"),
            format!("uaid:{}", hex.to_uppercase()),
            format!(" uaid:{hex}"),
            format!("uaid:{hex} "),
        ] {
            assert!(
                UniversalAccountId::from_str(&literal).is_err(),
                "noncanonical UAID must reject: {literal:?}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn manifest_json_requires_canonical_uaid_literal() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/space_directory/capability/cbdc_wholesale.manifest.json");
        let fixture = fs::read_to_string(&fixture_path).expect("read fixture JSON");
        let expected = cbdc_manifest_fixture();
        let uaid_hex = expected.uaid.as_hash().to_string();
        let mut value: norito::json::Value =
            norito::json::from_str(&fixture).expect("parse manifest JSON");
        let norito::json::Value::Object(map) = &mut value else {
            panic!("fixture manifest JSON must be an object");
        };
        map.insert("uaid".into(), Value::String(expected.uaid.to_string()));
        let parsed = AssetPermissionManifest::json_from_value(&value)
            .expect("canonical UAID literal must parse");
        assert_eq!(parsed.uaid, expected.uaid);

        for literal in [
            uaid_hex.clone(),
            format!("UAID:{}", uaid_hex.to_uppercase()),
            format!("uaid:{}", uaid_hex.to_uppercase()),
            format!(" uaid:{uaid_hex}"),
        ] {
            let mut value: norito::json::Value =
                norito::json::from_str(&fixture).expect("parse manifest JSON");
            let norito::json::Value::Object(map) = &mut value else {
                panic!("fixture manifest JSON must be an object");
            };
            map.insert("uaid".into(), norito::json::Value::String(literal));
            assert!(
                AssetPermissionManifest::json_from_value(&value).is_err(),
                "noncanonical UAID spellings must be rejected"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn manifest_json_rejects_negative_allowance_quantity() {
        let mut value = manifest_to_json_value(&cbdc_manifest_fixture());
        let Value::Object(root) = &mut value else {
            panic!("manifest fixture must serialize as an object");
        };
        let Value::Array(entries) = root.get_mut("entries").expect("manifest entries") else {
            panic!("manifest entries must serialize as an array");
        };
        let Value::Object(entry) = entries.first_mut().expect("allow entry") else {
            panic!("manifest entry must serialize as an object");
        };
        let Value::Object(effect) = entry.get_mut("effect").expect("entry effect") else {
            panic!("manifest effect must serialize as an object");
        };
        let Value::Object(allow) = effect.get_mut("Allow").expect("allow effect") else {
            panic!("allow effect must serialize as an object");
        };
        allow.insert("max_amount".into(), Value::String("-1".to_owned()));
        assert!(
            AssetPermissionManifest::json_from_value(&value).is_err(),
            "manifest JSON must reject a negative amount cap"
        );
    }
    #[cfg(feature = "json")]
    fn manifest_json_entry_mut(value: &mut Value, index: usize) -> &mut Map {
        let Value::Object(root) = value else {
            panic!("manifest fixture must serialize as an object");
        };
        let Value::Array(entries) = root.get_mut("entries").expect("manifest entries") else {
            panic!("manifest entries must serialize as an array");
        };
        let Value::Object(entry) = entries.get_mut(index).expect("manifest entry") else {
            panic!("manifest entry must serialize as an object");
        };
        entry
    }
    #[cfg(feature = "json")]
    fn manifest_json_scope_mut(value: &mut Value, index: usize) -> &mut Map {
        let entry = manifest_json_entry_mut(value, index);
        let Value::Object(scope) = entry.get_mut("scope").expect("manifest scope") else {
            panic!("manifest scope must serialize as an object");
        };
        scope
    }
    #[cfg(feature = "json")]
    fn manifest_json_effect_mut(value: &mut Value, index: usize) -> &mut Map {
        let entry = manifest_json_entry_mut(value, index);
        let Value::Object(effect) = entry.get_mut("effect").expect("manifest effect") else {
            panic!("manifest effect must serialize as an object");
        };
        effect
    }
    #[cfg(feature = "json")]
    fn manifest_json_effect_details_mut<'a>(
        value: &'a mut Value,
        index: usize,
        decision: &str,
    ) -> &'a mut Map {
        let effect = manifest_json_effect_mut(value, index);
        let Value::Object(details) = effect.get_mut(decision).expect("manifest effect details")
        else {
            panic!("manifest effect details must serialize as an object");
        };
        details
    }
    #[cfg(feature = "json")]
    #[test]
    fn manifest_json_rejects_unknown_fields_at_every_object_level() {
        let expected = cbdc_manifest_fixture();
        for level in ["manifest", "entry", "scope", "effect", "allow", "deny"] {
            let mut value = manifest_to_json_value(&expected);
            let target = match level {
                "manifest" => {
                    let Value::Object(root) = &mut value else {
                        panic!("manifest fixture must serialize as an object");
                    };
                    root
                }
                "entry" => manifest_json_entry_mut(&mut value, 0),
                "scope" => manifest_json_scope_mut(&mut value, 0),
                "effect" => manifest_json_effect_mut(&mut value, 0),
                "allow" => manifest_json_effect_details_mut(&mut value, 0, "Allow"),
                "deny" => manifest_json_effect_details_mut(&mut value, 1, "Deny"),
                _ => unreachable!(),
            };
            target.insert("unknown".into(), Value::String("rejected".into()));
            assert!(
                AssetPermissionManifest::json_from_value(&value).is_err(),
                "unknown {level} fields must be rejected"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn manifest_json_requires_canonical_owned_literals() {
        let expected = cbdc_manifest_fixture();

        let mut value = manifest_to_json_value(&expected);
        manifest_json_scope_mut(&mut value, 0)
            .insert("method".into(), Value::String("e\u{0301}".to_owned()));
        assert!(
            AssetPermissionManifest::json_from_value(&value).is_err(),
            "decomposed Name spelling must be rejected"
        );

        let mut value = manifest_to_json_value(&expected);
        manifest_json_effect_details_mut(&mut value, 0, "Allow").insert(
            "max_amount".into(),
            Value::Number(norito::json::native::Number::U64(500_000_000)),
        );
        assert!(
            AssetPermissionManifest::json_from_value(&value).is_err(),
            "quantity numbers must use their canonical string form"
        );

        let mut value = manifest_to_json_value(&expected);
        manifest_json_effect_details_mut(&mut value, 0, "Allow")
            .insert("max_amount".into(), Value::String("0500000000".to_owned()));
        assert!(
            AssetPermissionManifest::json_from_value(&value).is_err(),
            "noncanonical quantity strings must be rejected"
        );

        let mut value = manifest_to_json_value(&expected);
        let scope = manifest_json_scope_mut(&mut value, 0);
        let asset = scope
            .get("asset")
            .and_then(Value::as_str)
            .expect("fixture asset")
            .to_owned();
        scope.insert("asset".into(), Value::String(format!(" {asset}")));
        assert!(
            AssetPermissionManifest::json_from_value(&value).is_err(),
            "noncanonical asset whitespace must be rejected"
        );

        let mut value = manifest_to_json_value(&expected);
        let Value::Object(root) = &mut value else {
            panic!("manifest fixture must serialize as an object");
        };
        root.insert("expiry_epoch".into(), Value::Null);
        assert!(
            AssetPermissionManifest::json_from_value(&value).is_err(),
            "optional fields must be omitted instead of set to null"
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn manifest_json_value_decode_obeys_exact_allocation_budget() {
        fn limits(bytes: usize) -> norito::DecodeLimits {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        }

        let expected = cbdc_manifest_fixture();
        let value = manifest_to_json_value(&expected);
        let (decoded, usage) =
            norito::core::with_decode_limits_measured(limits(usize::MAX), || {
                AssetPermissionManifest::json_from_value(&value)
            });
        assert_eq!(decoded.expect("measured manifest decode"), expected);
        let exact = usage.total_allocated_bytes();
        assert!(
            exact >= expected.entries.len() * core::mem::size_of::<ManifestEntry>(),
            "entry storage must be included in the allocation charge"
        );

        let (decoded, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
            AssetPermissionManifest::json_from_value(&value)
        });
        assert_eq!(decoded.expect("exact manifest decode budget"), expected);
        assert_eq!(usage.total_allocated_bytes(), exact);

        let (decoded, usage) = norito::core::with_decode_limits_measured(limits(exact - 1), || {
            AssetPermissionManifest::json_from_value(&value)
        });
        assert!(
            decoded
                .expect_err("one-byte-short manifest budget must fail")
                .is_decode_resource_limit()
        );
        assert!(usage.total_allocated_bytes() < exact);
    }
    #[cfg(feature = "json")]
    #[test]
    fn manifest_json_value_decode_precharges_retained_strings() {
        fn limits(bytes: usize) -> norito::DecodeLimits {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        }

        let reason = "r".repeat(1_024);
        let notes = "n".repeat(2_048);
        let expected = manifest_with_entries(
            DataSpaceId::new(1),
            vec![ManifestEntry {
                scope: CapabilityScope {
                    dataspace: None,
                    program: None,
                    method: None,
                    asset: None,
                    role: None,
                },
                effect: ManifestEffect::Deny(DenyDirective {
                    reason: Some(reason.clone()),
                }),
                notes: Some(notes.clone()),
            }],
        );
        let value = manifest_to_json_value(&expected);
        let exact = core::mem::size_of::<ManifestEntry>() + reason.len() + notes.len();

        let (decoded, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
            AssetPermissionManifest::json_from_value(&value)
        });
        assert_eq!(decoded.expect("exact retained-string budget"), expected);
        assert_eq!(usage.total_allocated_bytes(), exact);

        let (decoded, usage) = norito::core::with_decode_limits_measured(limits(exact - 1), || {
            AssetPermissionManifest::json_from_value(&value)
        });
        assert!(
            decoded
                .expect_err("one-byte-short retained-string budget must fail")
                .is_decode_resource_limit()
        );
        assert_eq!(
            usage.total_allocated_bytes(),
            core::mem::size_of::<ManifestEntry>() + reason.len(),
            "notes storage must be charged before it is allocated"
        );
    }
    fn manifest_with_entries(
        dataspace: DataSpaceId,
        entries: Vec<ManifestEntry>,
    ) -> AssetPermissionManifest {
        AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace,
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries,
        }
    }
    fn manifest_request(
        dataspace: DataSpaceId,
        method: &Name,
        amount: Quantity,
    ) -> CapabilityRequest<'_> {
        CapabilityRequest::new(
            dataspace,
            None,
            Some(method),
            None,
            Some(AmxRole::Initiator),
            Some(amount),
            5,
        )
    }
    fn cbdc_manifest_fixture() -> AssetPermissionManifest {
        let uaid_hex = "0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";
        let uaid = UniversalAccountId::from_hash(
            Hash::from_str(uaid_hex).expect("fixture uaid hex must parse"),
        );
        let dataspace = DataSpaceId::new(11);
        let allowance = Allowance {
            max_amount: Some(Quantity::from(500_000_000_u64)),
            window: AllowanceWindow::PerDay,
        };
        let allow_entry = ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(dataspace),
                program: Some("cbdc.transfer".parse().expect("program id")),
                method: Some(sample_name("transfer")),
                asset: Some(
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("centralbank", "universal").unwrap(),
                        "CBDC".parse().unwrap(),
                    ),
                ),
                role: Some(AmxRole::Initiator),
            },
            effect: ManifestEffect::Allow(allowance),
            notes: Some("Wholesale transfer allowance (per UAID, per day).".to_owned()),
        };
        let deny_entry = ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(dataspace),
                program: Some("cbdc.kit".parse().expect("program id")),
                method: Some(sample_name("withdraw")),
                asset: None,
                role: None,
            },
            effect: ManifestEffect::Deny(DenyDirective {
                reason: Some("Withdrawals disabled for this UAID.".to_owned()),
            }),
            notes: Some("Deny wins over any preceding allowance.".to_owned()),
        };
        AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid,
            dataspace,
            issued_ms: 1_762_723_200_000,
            activation_epoch: 4097,
            expiry_epoch: Some(4600),
            entries: vec![allow_entry, deny_entry],
        }
    }
    #[test]
    fn cbdc_manifest_fixture_matches_serialized_json() {
        let manifest = cbdc_manifest_fixture();
        let expected = norito::json::to_value(&manifest).expect("serialize manifest to JSON");
        if std::env::var_os("IROHA_DUMP_MANIFEST_JSON").is_some() {
            let rendered = norito::json::to_string_pretty(&expected).expect("render manifest JSON");
            eprintln!("{rendered}");
        }
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/space_directory/capability/cbdc_wholesale.manifest.json");
        let fixture = fs::read_to_string(&fixture_path).expect("read fixture JSON");
        let fixture_value: norito::json::Value =
            norito::json::from_str(&fixture).expect("parse fixture JSON");
        assert_eq!(fixture_value, expected);
    }
    #[cfg(feature = "json")]
    #[test]
    fn manifest_bounded_json_writer_matches_and_balances_depth() {
        #[derive(Default)]
        struct StructuralSink {
            depth: usize,
            max_depth: usize,
        }
        impl json::JsonWriteSink for StructuralSink {
            fn push(&mut self, _: char) -> Result<(), json::BoundedJsonError> {
                Ok(())
            }
            fn push_str(&mut self, _: &str) -> Result<(), json::BoundedJsonError> {
                Ok(())
            }
            fn begin_container(&mut self) -> Result<(), json::BoundedJsonError> {
                self.depth += 1;
                self.max_depth = self.max_depth.max(self.depth);
                Ok(())
            }
            fn end_container(&mut self) {
                assert!(self.depth > 0, "container depth must not underflow");
                self.depth -= 1;
            }
        }

        let manifest = cbdc_manifest_fixture();
        let ordinary = json::to_json(&manifest).expect("ordinary manifest JSON");
        let bounded = json::to_json_bounded(&manifest, ordinary.len())
            .expect("exact-size bounded manifest JSON");
        assert_eq!(bounded, ordinary);
        assert_eq!(
            json::to_json_bounded(&manifest, ordinary.len() - 1),
            Err(json::BoundedJsonError::BodyTooLarge)
        );

        let mut sink = StructuralSink::default();
        <AssetPermissionManifest as json::JsonSerialize>::json_serialize_to(&manifest, &mut sink)
            .expect("structurally tracked manifest JSON");
        assert_eq!(sink.depth, 0, "all containers must be closed");
        assert_eq!(
            sink.max_depth, 5,
            "root object, entries array, entry, effect, and effect details"
        );
    }
    #[test]
    fn cbdc_manifest_fixture_roundtrips_json() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/space_directory/capability/cbdc_wholesale.manifest.json");
        let fixture = fs::read_to_string(&fixture_path).expect("read fixture JSON");
        let parsed: AssetPermissionManifest =
            norito::json::from_str(&fixture).expect("parse manifest JSON");
        assert_eq!(parsed, cbdc_manifest_fixture());
    }
    fn matching_scope(method: &Name) -> CapabilityScope {
        CapabilityScope {
            dataspace: None,
            program: None,
            method: Some(method.clone()),
            asset: None,
            role: Some(AmxRole::Initiator),
        }
    }
    fn foreign_scope(foreign_dataspace: DataSpaceId) -> CapabilityScope {
        CapabilityScope {
            dataspace: Some(foreign_dataspace),
            program: None,
            method: Some(sample_name("other")),
            asset: None,
            role: Some(AmxRole::Participant),
        }
    }
    #[derive(Clone, Copy, Debug)]
    enum EntryKind {
        MatchingAllow,
        MatchingDeny,
        NonMatchingAllow,
        NonMatchingDeny,
    }
    fn matching_deny_cases() -> Vec<Vec<EntryKind>> {
        vec![
            vec![EntryKind::MatchingDeny],
            vec![EntryKind::MatchingAllow, EntryKind::MatchingDeny],
            vec![
                EntryKind::NonMatchingAllow,
                EntryKind::MatchingAllow,
                EntryKind::MatchingDeny,
                EntryKind::NonMatchingDeny,
            ],
            vec![
                EntryKind::NonMatchingDeny,
                EntryKind::MatchingDeny,
                EntryKind::MatchingAllow,
                EntryKind::MatchingDeny,
            ],
        ]
    }
    fn matching_allow_only_cases() -> Vec<Vec<EntryKind>> {
        vec![
            vec![EntryKind::MatchingAllow],
            vec![EntryKind::NonMatchingAllow, EntryKind::MatchingAllow],
            vec![
                EntryKind::MatchingAllow,
                EntryKind::NonMatchingDeny,
                EntryKind::MatchingAllow,
            ],
            vec![
                EntryKind::NonMatchingDeny,
                EntryKind::NonMatchingAllow,
                EntryKind::MatchingAllow,
            ],
        ]
    }
    fn build_entries(
        kinds: &[EntryKind],
        method: &Name,
        foreign_dataspace: DataSpaceId,
    ) -> Vec<ManifestEntry> {
        let matching_scope = matching_scope(method);
        let foreign_scope = foreign_scope(foreign_dataspace);
        kinds
            .iter()
            .map(|kind| match kind {
                EntryKind::MatchingAllow => ManifestEntry {
                    scope: matching_scope.clone(),
                    effect: ManifestEffect::Allow(Allowance {
                        max_amount: Some(Quantity::from(50_u32)),
                        window: AllowanceWindow::PerSlot,
                    }),
                    notes: None,
                },
                EntryKind::MatchingDeny => ManifestEntry {
                    scope: matching_scope.clone(),
                    effect: ManifestEffect::Deny(DenyDirective { reason: None }),
                    notes: None,
                },
                EntryKind::NonMatchingAllow => ManifestEntry {
                    scope: foreign_scope.clone(),
                    effect: ManifestEffect::Allow(Allowance {
                        max_amount: None,
                        window: AllowanceWindow::PerMinute,
                    }),
                    notes: None,
                },
                EntryKind::NonMatchingDeny => ManifestEntry {
                    scope: foreign_scope.clone(),
                    effect: ManifestEffect::Deny(DenyDirective { reason: None }),
                    notes: None,
                },
            })
            .collect()
    }
    fn first_matching_deny_index(
        entries: &[ManifestEntry],
        request: &CapabilityRequest<'_>,
    ) -> Option<u32> {
        entries.iter().enumerate().find_map(|(idx, entry)| {
            if entry.scope.matches(request) && matches!(entry.effect, ManifestEffect::Deny(_)) {
                return Some(AssetPermissionManifest::clamp_entry_index(idx));
            }
            None
        })
    }
    fn last_matching_allow_index(
        entries: &[ManifestEntry],
        request: &CapabilityRequest<'_>,
    ) -> Option<u32> {
        entries.iter().enumerate().rev().find_map(|(idx, entry)| {
            if entry.scope.matches(request) && matches!(entry.effect, ManifestEffect::Allow(_)) {
                return Some(AssetPermissionManifest::clamp_entry_index(idx));
            }
            None
        })
    }
    #[test]
    fn deny_rule_wins_over_allow() {
        let allowance = Allowance {
            max_amount: Some(Quantity::from(100_u32)),
            window: AllowanceWindow::PerSlot,
        };
        let method = sample_name("transfer");
        let entries = vec![
            ManifestEntry {
                scope: CapabilityScope {
                    dataspace: None,
                    program: None,
                    method: Some(method.clone()),
                    asset: None,
                    role: Some(AmxRole::Initiator),
                },
                effect: ManifestEffect::Allow(allowance),
                notes: None,
            },
            ManifestEntry {
                scope: CapabilityScope {
                    dataspace: None,
                    program: None,
                    method: Some(method.clone()),
                    asset: None,
                    role: Some(AmxRole::Initiator),
                },
                effect: ManifestEffect::Deny(DenyDirective {
                    reason: Some("regulator deny".to_owned()),
                }),
                notes: None,
            },
        ];
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace: DataSpaceId::new(7),
            issued_ms: 0,
            activation_epoch: 10,
            expiry_epoch: None,
            entries,
        };
        let request = CapabilityRequest::new(
            manifest.dataspace,
            None,
            Some(&method),
            None,
            Some(AmxRole::Initiator),
            Some(Quantity::from(1_u32)),
            12,
        );
        match manifest.evaluate(&request) {
            ManifestVerdict::Denied(DenyReason::ExplicitRule { entry_index, .. }) => {
                assert_eq!(entry_index, 1);
            }
            other => panic!("expected explicit deny, got {other:?}"),
        }
    }
    #[test]
    fn manifest_inactive_outside_epoch_window() {
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace: DataSpaceId::new(1),
            issued_ms: 0,
            activation_epoch: 5,
            expiry_epoch: Some(10),
            entries: Vec::new(),
        };
        let request = CapabilityRequest::new(manifest.dataspace, None, None, None, None, None, 4);
        assert!(matches!(
            manifest.evaluate(&request),
            ManifestVerdict::Denied(DenyReason::ManifestInactive { .. })
        ));
        let late_request =
            CapabilityRequest::new(manifest.dataspace, None, None, None, None, None, 11);
        assert!(matches!(
            manifest.evaluate(&late_request),
            ManifestVerdict::Denied(DenyReason::ManifestInactive { .. })
        ));
    }
    #[test]
    fn allowance_enforced_for_amounts() {
        let method = sample_name("mint");
        let entries = vec![ManifestEntry {
            scope: CapabilityScope {
                dataspace: None,
                program: None,
                method: Some(method.clone()),
                asset: None,
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Quantity::from(5_u32)),
                window: AllowanceWindow::PerMinute,
            }),
            notes: None,
        }];
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: sample_uaid(),
            dataspace: DataSpaceId::new(9),
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries,
        };
        let ok_request = CapabilityRequest::new(
            manifest.dataspace,
            None,
            Some(&method),
            None,
            None,
            Some(Quantity::from(3_u32)),
            1,
        );
        assert!(matches!(
            manifest.evaluate(&ok_request),
            ManifestVerdict::Allowed(ManifestGrant { entry_index: 0, .. })
        ));
        let over_request = CapabilityRequest::new(
            manifest.dataspace,
            None,
            Some(&method),
            None,
            None,
            Some(Quantity::from(10_u32)),
            1,
        );
        match manifest.evaluate(&over_request) {
            ManifestVerdict::Denied(DenyReason::AmountExceeded { .. }) => {}
            other => panic!("expected amount exceeded, got {other:?}"),
        }
    }
    #[test]
    fn matching_deny_always_wins_for_deterministic_cases() {
        for kinds in matching_deny_cases() {
            let method = sample_name("transfer_deny_prop");
            let dataspace = DataSpaceId::new(42);
            let entries = build_entries(&kinds, &method, DataSpaceId::new(99));
            let manifest = manifest_with_entries(dataspace, entries.clone());
            let request = manifest_request(dataspace, &method, Quantity::from(1_u32));
            let expected_idx = first_matching_deny_index(&entries, &request)
                .expect("generated at least one matching deny");
            match manifest.evaluate(&request) {
                ManifestVerdict::Denied(DenyReason::ExplicitRule { entry_index, .. }) => {
                    assert_eq!(
                        entry_index, expected_idx,
                        "deny verdict should point at the first matching deny entry"
                    );
                }
                other => {
                    panic!("expected explicit deny when a matching deny rule exists, got {other:?}")
                }
            }
        }
    }
    #[test]
    fn last_matching_allow_applied_when_no_denies_for_deterministic_cases() {
        for kinds in matching_allow_only_cases() {
            let method = sample_name("allow_prop");
            let dataspace = DataSpaceId::new(7);
            let entries = build_entries(&kinds, &method, DataSpaceId::new(11));
            let manifest = manifest_with_entries(dataspace, entries.clone());
            let request = manifest_request(dataspace, &method, Quantity::from(1_u32));
            let expected_idx = last_matching_allow_index(&entries, &request)
                .expect("generated at least one matching allow");
            match manifest.evaluate(&request) {
                ManifestVerdict::Allowed(ManifestGrant { entry_index, .. }) => {
                    assert_eq!(
                        entry_index, expected_idx,
                        "allow verdict should use the last matching allow entry"
                    );
                }
                other => panic!("expected allow when no matching deny exists, got {other:?}"),
            }
        }
    }
}
