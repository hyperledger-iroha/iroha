//! Immutable asset-definition home projections from authoritative ledger state.

use super::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId};
use crate::{
    nexus::AxtAssetIncarnationV1,
    parameter::{CustomParameter, CustomParameterId},
};
use iroha_model_base::{domain::DomainId, error::ParseError, topology::DataSpaceId};
use iroha_primitives::json::{Json, MAX_JSON_BYTES};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::BTreeMap;

/// Maximum live bindings and retained tombstones in the protected asset home registry.
pub const MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1: usize = 4096;
/// Maximum canonical JSON bytes of the protected asset home registry.
pub const MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1: usize = MAX_JSON_BYTES;
const REGISTRY_DECODE_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1,
    MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1,
    32 * MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1,
    16 * MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1,
    32,
);

/// Exact immutable home assigned to one authenticated asset-definition incarnation.
///
/// Unregistration retains this record with `active = false`. Only a subsequent native
/// registration with a different incarnation may replace it; ordinary parameter writes,
/// ownership transfers, aliases, and balance movements never establish a home.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::asset::dataspace_definition::AssetDefinitionDataspaceBindingV1"
)]
pub struct AssetDefinitionDataspaceBindingV1 {
    /// Exact definition identity, repeated in the registry key to reject mismatched bindings.
    pub asset_definition_id: AssetDefinitionId,
    /// Incarnation produced by the genuine native asset-registration event.
    pub incarnation: AxtAssetIncarnationV1,
    /// Exact non-universal physical dataspace namespace home.
    pub dataspace_id: DataSpaceId,
    /// Whether this incarnation still exists; false retains an unregistration tombstone.
    pub active: bool,
}

impl AssetDefinitionDataspaceBindingV1 {
    /// Validate the canonical incarnation and physical dataspace shape.
    ///
    /// # Errors
    /// Rejects a reserved universal dataspace or an invalid native incarnation token.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.dataspace_id == DataSpaceId::UNIVERSAL {
            return Err(ParseError::new(
                "asset home binding requires a non-universal dataspace",
            ));
        }
        self.incarnation
            .validate()
            .map_err(|_| ParseError::new("asset home binding has an invalid incarnation"))
    }
}

/// Protected asset homes persisted through the existing authoritative custom-parameter state.
///
/// Absence of this parameter means no direct-dataspace registration has occurred. Do not
/// synthesize or write an empty registry while reading an existing chain. Core admits writes
/// only at native registration and unregistration boundaries; encoding this value grants no
/// authority. The existing World, block, transaction, genesis and definition layouts are intact.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::asset::dataspace_definition::AssetDefinitionDataspaceRegistryV1"
)]
pub struct AssetDefinitionDataspaceRegistryV1 {
    /// Layout version, exactly [`Self::VERSION`].
    pub version: u8,
    /// Canonically ordered live bindings and retained tombstones, keyed by exact definition ID.
    pub bindings: BTreeMap<AssetDefinitionId, AssetDefinitionDataspaceBindingV1>,
}

impl AssetDefinitionDataspaceRegistryV1 {
    /// Sole supported registry layout.
    pub const VERSION: u8 = 1;
    /// Reserved authoritative state key, forbidden to ordinary `SetParameter` instructions.
    pub const PARAMETER_ID_STR: &'static str = "asset_definition_dataspace_homes_v1";

    /// Return the exact protected custom-parameter identity.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid asset home parameter ID")
    }

    /// Validate bounded, non-empty state without claiming transition authorization.
    ///
    /// # Errors
    /// Rejects unsupported versions, empty or oversized registries, mismatched definition IDs,
    /// invalid bindings, or a payload exceeding the existing custom-parameter JSON bound.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.version != Self::VERSION {
            return Err(ParseError::new("unsupported asset home registry version"));
        }
        if self.bindings.is_empty()
            || self.bindings.len() > MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1
        {
            return Err(ParseError::new(
                "asset home registry must contain a bounded non-empty binding set",
            ));
        }
        for (id, binding) in &self.bindings {
            if id != &binding.asset_definition_id {
                return Err(ParseError::new(
                    "asset home binding does not match its definition key",
                ));
            }
            binding.validate()?;
        }
        registry_preflight(&registry_bounded_json(self)?)
    }

    /// Encode validated registry state for a native executor transition.
    ///
    /// # Errors
    /// Returns structural or bounded canonical JSON failures. This does not authorize a write.
    pub fn into_custom_parameter(self) -> Result<CustomParameter, ParseError> {
        self.validate()?;
        let encoded = registry_bounded_json(&self)?;
        let payload = norito::with_decode_limits_scope(REGISTRY_DECODE_LIMITS, || {
            let value: norito::json::Value = norito::json::from_str(&encoded)?;
            Json::from_norito_value_ref(&value)
                .map_err(|error| norito::json::Error::Message(error.to_string()))
        })
        .map_err(|_| ParseError::new("asset home registry exceeds canonical JSON bounds"))?;
        Ok(CustomParameter::new(Self::parameter_id(), payload))
    }

    /// Decode this exact protected parameter, returning `None` for an unrelated identity.
    ///
    /// # Errors
    /// A matching parameter with malformed, unsupported, empty, or oversized data fails closed.
    pub fn from_custom_parameter(custom: &CustomParameter) -> Result<Option<Self>, ParseError> {
        if custom.id() != &Self::parameter_id() {
            return Ok(None);
        }
        registry_preflight(custom.payload().get())?;
        let registry: Self = norito::with_decode_limits_scope(REGISTRY_DECODE_LIMITS, || {
            norito::json::from_str(custom.payload().get())
        })
        .map_err(|_| ParseError::new("invalid asset home registry JSON"))?;
        registry.validate()?;
        Ok(Some(registry))
    }
}

fn registry_bounded_json(value: &AssetDefinitionDataspaceRegistryV1) -> Result<String, ParseError> {
    norito::json::to_json_bounded(value, MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1)
        .map_err(|_| ParseError::new("asset home registry exceeds canonical JSON bounds"))
}

fn registry_preflight(raw: &str) -> Result<(), ParseError> {
    let limits = norito::json::JsonPreflightLimits::new(
        MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1,
        128 * 1024,
        MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1,
        MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1,
        MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1,
        MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1,
        MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1,
        MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1,
        32 * MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1,
        32,
    );
    norito::json::preflight_slice(raw.as_bytes(), limits)
        .map(|_| ())
        .map_err(|_| ParseError::new("asset home registry fails bounded JSON preflight"))
}

/// Borrowed, validated read plan for an existing protected registry.
///
/// Planning and filling use only borrowed JSON fragments and fixed-size stack values. The
/// caller admits the exact binding buffer through its original allocation owner before filling
/// it. This does not create a decode-counter owner, an owned JSON tree, or a `BTreeMap`.
#[derive(Debug)]
pub struct AssetDefinitionDataspaceRegistryReadPlan<'a> {
    bindings: &'a str,
    binding_count: usize,
}

impl<'a> AssetDefinitionDataspaceRegistryReadPlan<'a> {
    /// Validate and borrow this exact parameter, returning `None` for an unrelated key.
    ///
    /// # Errors
    /// Rejects malformed or unsupported registry state without allocating diagnostics. Existing
    /// JSON resource errors retain their original variants for the caller's admission policy.
    pub fn from_custom_parameter(
        custom: &'a CustomParameter,
    ) -> Result<Option<Self>, norito::json::Error> {
        if custom.id().name().as_ref() != AssetDefinitionDataspaceRegistryV1::PARAMETER_ID_STR {
            return Ok(None);
        }
        Self::from_canonical_json(custom.payload().get()).map(Some)
    }

    fn from_canonical_json(raw: &'a str) -> Result<Self, norito::json::Error> {
        registry_preflight(raw).map_err(|error| registry_read_error(error.reason()))?;
        // Json stores canonical text. All fields in this format use ASCII names, Base58 IDs,
        // fixed hash literals, numbers, or booleans. No valid spelling needs a JSON escape.
        // Reject before MapVisitor can take parse_key's allocating escaped-string path.
        if !raw.is_ascii() || raw.as_bytes().contains(&b'\\') {
            return Err(registry_read_error(
                "asset home registry requires unescaped ASCII",
            ));
        }
        let mut parser = norito::json::Parser::new(raw);
        let mut object = norito::json::MapVisitor::new(&mut parser)?;
        let mut version = None;
        let mut bindings = None;
        while let Some(key) = object.next_key()? {
            match registry_read_key(key)? {
                "version" if version.is_none() => {
                    version = Some(object.parse_value_with_parser(|parser| parser.parse_u64())?);
                }
                "bindings" if bindings.is_none() => {
                    bindings =
                        Some(object.parse_value_with_parser(|parser| parser.raw_value_slice())?);
                }
                _ => {
                    return Err(registry_read_error(
                        "unknown or duplicate asset home registry field",
                    ));
                }
            }
        }
        object.finish()?;
        registry_read_end(&mut parser)?;
        if version != Some(u64::from(AssetDefinitionDataspaceRegistryV1::VERSION)) {
            return Err(registry_read_error(
                "unsupported or missing asset home registry version",
            ));
        }
        let bindings =
            bindings.ok_or_else(|| registry_read_error("missing asset home bindings"))?;
        let binding_count = read_registry_bindings(bindings, |_| Ok(()))?;
        Ok(Self {
            bindings,
            binding_count,
        })
    }

    /// Number of fixed-size bindings requiring original-pool allocation admission.
    #[must_use]
    pub const fn binding_count(&self) -> usize {
        self.binding_count
    }

    /// Fill an empty prepaid buffer and order its entries by exact definition identity.
    ///
    /// This performs no capacity acquisition. On a parsing refusal, the initialized prefix is
    /// cleared while the caller retains the same funded backing for retry or abandonment.
    ///
    /// # Errors
    /// Rejects a nonempty or undersized destination, invalid bindings, or duplicate identities.
    /// Original parser resource errors are propagated unchanged.
    pub fn decode_into(
        &self,
        destination: &mut iroha_allocation::ChargedBuffer<AssetDefinitionDataspaceBindingV1>,
    ) -> Result<(), norito::json::Error> {
        if !destination.as_slice().is_empty() || destination.capacity() < self.binding_count {
            return Err(registry_read_error(
                "asset home destination must be empty and prepaid",
            ));
        }
        let result = (|| {
            let count = read_registry_bindings(self.bindings, |binding| {
                destination.try_push(binding).map_err(|_| {
                    registry_read_error("asset home destination capacity differs from its plan")
                })
            })?;
            if count != self.binding_count {
                return Err(registry_read_error(
                    "asset home binding count differs from its plan",
                ));
            }
            destination.as_mut_slice().sort_unstable_by(|left, right| {
                left.asset_definition_id.cmp(&right.asset_definition_id)
            });
            if destination
                .as_slice()
                .windows(2)
                .any(|pair| pair[0].asset_definition_id == pair[1].asset_definition_id)
            {
                return Err(registry_read_error(
                    "duplicate asset home definition identity",
                ));
            }
            Ok(())
        })();
        if result.is_err() {
            destination.truncate(0);
        }
        result
    }
}

fn registry_read_error(msg: &'static str) -> norito::json::Error {
    norito::json::Error::WithPos {
        msg,
        byte: 0,
        line: 1,
        col: 1,
    }
}

fn registry_read_key(key: norito::json::KeyRef<'_>) -> Result<&str, norito::json::Error> {
    match key {
        norito::json::KeyRef::Borrowed(key) => Ok(key),
        norito::json::KeyRef::Owned(_) => Err(registry_read_error("escaped asset home field")),
    }
}

fn registry_read_end(parser: &mut norito::json::Parser<'_>) -> Result<(), norito::json::Error> {
    parser.skip_ws();
    if parser.eof() {
        Ok(())
    } else {
        Err(registry_read_error("trailing asset home JSON"))
    }
}

fn registry_read_string<'a>(
    parser: &mut norito::json::Parser<'a>,
) -> Result<&'a str, norito::json::Error> {
    let raw = parser.raw_value_slice()?;
    raw.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| registry_read_error("asset home field must be a string"))
}

fn registry_read_incarnation(
    parser: &mut norito::json::Parser<'_>,
) -> Result<AxtAssetIncarnationV1, norito::json::Error> {
    // The existing derived newtype JSON is a one-element tuple array, not a bare hash.
    parser.skip_ws();
    parser.expect(b'[')?;
    let literal = registry_read_string(parser)?;
    parser.skip_ws();
    parser.expect(b']')?;
    let body = norito::literal::parse_without_diagnostics("hash", literal)
        .ok_or_else(|| registry_read_error("invalid asset home incarnation literal"))?;
    if body.len() != iroha_crypto::Hash::LENGTH * 2 {
        return Err(registry_read_error("invalid asset home incarnation width"));
    }
    let mut bytes = [0_u8; iroha_crypto::Hash::LENGTH];
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(registry_read_error(
            "asset home incarnation requires uppercase hex",
        )),
    };
    for (output, pair) in bytes.iter_mut().zip(body.as_bytes().chunks_exact(2)) {
        *output = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    AxtAssetIncarnationV1::try_from_bytes(bytes)
        .map_err(|_| registry_read_error("invalid asset home incarnation"))
}

fn read_registry_binding(
    parser: &mut norito::json::Parser<'_>,
) -> Result<AssetDefinitionDataspaceBindingV1, norito::json::Error> {
    let mut object = norito::json::MapVisitor::new(parser)?;
    let (mut id, mut incarnation, mut dataspace, mut active) = (None, None, None, None);
    while let Some(key) = object.next_key()? {
        match registry_read_key(key)? {
            "asset_definition_id" if id.is_none() => {
                id = Some(object.parse_value_with_parser(|parser| {
                    AssetDefinitionId::parse_address_literal(registry_read_string(parser)?)
                        .map_err(|_| registry_read_error("invalid asset home definition identity"))
                })?);
            }
            "incarnation" if incarnation.is_none() => {
                incarnation = Some(object.parse_value_with_parser(registry_read_incarnation)?);
            }
            "dataspace_id" if dataspace.is_none() => {
                dataspace = Some(object.parse_value_with_parser(|parser| parser.parse_u64())?);
            }
            "active" if active.is_none() => {
                active = Some(object.parse_value_with_parser(|parser| parser.parse_bool())?);
            }
            _ => {
                return Err(registry_read_error(
                    "unknown or duplicate asset home binding field",
                ));
            }
        }
    }
    object.finish()?;
    let binding = AssetDefinitionDataspaceBindingV1 {
        asset_definition_id: id
            .ok_or_else(|| registry_read_error("missing asset home definition identity"))?,
        incarnation: incarnation
            .ok_or_else(|| registry_read_error("missing asset home incarnation"))?,
        dataspace_id: DataSpaceId::new(
            dataspace.ok_or_else(|| registry_read_error("missing asset home dataspace"))?,
        ),
        active: active.ok_or_else(|| registry_read_error("missing asset home active flag"))?,
    };
    binding
        .validate()
        .map_err(|error| registry_read_error(error.reason()))?;
    Ok(binding)
}

fn read_registry_bindings(
    raw: &str,
    mut consume: impl FnMut(AssetDefinitionDataspaceBindingV1) -> Result<(), norito::json::Error>,
) -> Result<usize, norito::json::Error> {
    let mut parser = norito::json::Parser::new(raw);
    let mut bindings = norito::json::MapVisitor::new(&mut parser)?;
    let count = bindings.total_entries();
    if count == 0 || count > MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1 {
        return Err(registry_read_error(
            "asset home registry requires a bounded nonempty binding set",
        ));
    }
    let mut previous = None;
    let mut seen = 0;
    while let Some(key) = bindings.next_key()? {
        let key = registry_read_key(key)?;
        // The authoritative Json wrapper stores lexically sorted object keys. Check that
        // original order without a key set; decoded IDs are sorted separately for callers.
        if previous.is_some_and(|previous| previous >= key) {
            return Err(registry_read_error(
                "asset home binding keys are duplicate or noncanonical",
            ));
        }
        previous = Some(key);
        let id = AssetDefinitionId::parse_address_literal(key)
            .map_err(|_| registry_read_error("invalid asset home binding key"))?;
        let binding = bindings.parse_value_with_parser(read_registry_binding)?;
        if binding.asset_definition_id != id {
            return Err(registry_read_error(
                "asset home binding does not match its definition key",
            ));
        }
        consume(binding)?;
        seen += 1;
    }
    bindings.finish()?;
    registry_read_end(&mut parser)?;
    if seen != count {
        return Err(registry_read_error(
            "asset home binding count changed while parsing",
        ));
    }
    Ok(count)
}

/// Immutable namespace home of an asset definition.
///
/// This is a projection of the definition and its separate authoritative direct-dataspace
/// registration. It is not appended to the signed [`AssetDefinition`] payload. Aliases and
/// concrete balance buckets do not establish or change a definition's home.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "kind", content = "content")]
#[norito_schema(name = "iroha_data_model::asset::dataspace_definition::AssetDefinitionHome")]
#[expect(
    variant_size_differences,
    reason = "keep the bounded domain identifier inline in this read projection without another heap allocation"
)]
pub enum AssetDefinitionHome {
    /// An existing global definition with no domain or direct-dataspace home.
    Global,
    /// A definition owned by this exact domain.
    Domain(DomainId),
    /// A definition registered directly under this exact dataspace.
    Dataspace(DataSpaceId),
}

impl AssetDefinitionHome {
    /// Project the immutable home from an asset definition and its committed direct binding.
    ///
    /// `direct_dataspace` must come from the authoritative binding keyed by `definition.id`,
    /// never from an alias, account home, transaction route, or concrete balance bucket.
    /// Both global and dataspace-restricted balance policies may have an explicit home.
    ///
    /// # Errors
    /// Returns an error if a definition claims both domain and direct-dataspace homes, if a
    /// direct binding names the reserved universal dataspace, or if a dataspace-restricted
    /// definition has neither home.
    pub fn from_definition(
        definition: &AssetDefinition,
        direct_dataspace: Option<DataSpaceId>,
    ) -> Result<Self, ParseError> {
        match (definition.owning_domain.as_ref(), direct_dataspace) {
            (Some(_), Some(_)) => Err(ParseError::new(
                "asset definition cannot have both domain and direct-dataspace homes",
            )),
            (Some(domain), None) => Ok(Self::Domain(domain.clone())),
            (None, Some(DataSpaceId::UNIVERSAL)) => Err(ParseError::new(
                "direct-dataspace asset home must be a non-universal dataspace",
            )),
            (None, Some(dataspace)) => Ok(Self::Dataspace(dataspace)),
            (None, None) if definition.balance_scope_policy == AssetBalancePolicy::Global => {
                Ok(Self::Global)
            }
            (None, None) => Err(ParseError::new(
                "dataspace-restricted asset definition requires an immutable home",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Registrable, account::AccountId, asset::AssetDefinitionId};
    use iroha_crypto::{Algorithm, Hash, KeyPair};

    fn registry(active: bool) -> AssetDefinitionDataspaceRegistryV1 {
        let definition = definition(AssetBalancePolicy::DataspaceRestricted, None);
        let binding = AssetDefinitionDataspaceBindingV1 {
            asset_definition_id: definition.id.clone(),
            incarnation: AxtAssetIncarnationV1::try_from_bytes(
                Hash::new(b"public registry fixture").into(),
            )
            .expect("canonical nonzero fixture incarnation"),
            dataspace_id: DataSpaceId::new(u64::MAX),
            active,
        };
        AssetDefinitionDataspaceRegistryV1 {
            version: AssetDefinitionDataspaceRegistryV1::VERSION,
            bindings: BTreeMap::from([(definition.id, binding)]),
        }
    }

    #[test]
    fn borrowed_registry_plan_matches_owned_decode_in_exact_prepaid_buffer() {
        use iroha_allocation::{AllocationBudget, ChargedBuffer};
        for active in [true, false] {
            let mut expected = registry(active);
            let seed = expected.bindings.values().next().unwrap().clone();
            for name in ["second", "third", "fourth"] {
                let id = AssetDefinitionId::derive_from_components(
                    DomainId::parse_fully_qualified("fixture.universal").unwrap(),
                    name.parse().unwrap(),
                );
                expected.bindings.insert(
                    id.clone(),
                    AssetDefinitionDataspaceBindingV1 {
                        asset_definition_id: id,
                        ..seed.clone()
                    },
                );
            }
            let parameter = expected.clone().into_custom_parameter().unwrap();
            let plan = AssetDefinitionDataspaceRegistryReadPlan::from_custom_parameter(&parameter)
                .unwrap()
                .unwrap();
            assert_eq!(plan.binding_count(), expected.bindings.len());
            let source = parameter.payload().get().as_bytes();
            let offset = plan.bindings.as_ptr() as usize - source.as_ptr() as usize;
            assert_eq!(
                &source[offset..offset + plan.bindings.len()],
                plan.bindings.as_bytes()
            );
            let bytes = std::alloc::Layout::array::<AssetDefinitionDataspaceBindingV1>(
                plan.binding_count(),
            )
            .unwrap()
            .size();
            let budget = AllocationBudget::new(bytes);
            let mut rows = ChargedBuffer::new(plan.binding_count(), &budget).unwrap();
            assert_eq!(budget.reserved_bytes(), bytes);
            plan.decode_into(&mut rows).unwrap();
            assert_eq!(budget.reserved_bytes(), bytes);
            assert!(expected.bindings.values().eq(rows.as_slice()));
            let restored = AssetDefinitionDataspaceRegistryV1::from_custom_parameter(&parameter)
                .unwrap()
                .unwrap();
            assert_eq!(expected, restored);
            drop(rows);
            assert_eq!(budget.reserved_bytes(), 0);
        }
    }

    #[test]
    fn borrowed_registry_plan_ignores_other_parameters_and_rejects_invalid_shape() {
        let parameter = registry(true).into_custom_parameter().unwrap();
        let unrelated =
            CustomParameter::new("unrelated".parse().unwrap(), parameter.payload().clone());
        assert!(
            AssetDefinitionDataspaceRegistryReadPlan::from_custom_parameter(&unrelated)
                .unwrap()
                .is_none()
        );
        let raw = parameter.payload().get();
        let invalid = [
            raw.replacen("\"version\":1", "\"version\":2", 1),
            raw.replacen("\"version\":1", "\"version\":1,\"other\":0", 1),
            raw.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
            raw.replacen("\"active\":true", "\"active\":true,\"active\":false", 1),
            raw.replacen("\"active\":true", "\"active\":null", 1),
            raw.replacen(
                "\"dataspace_id\":18446744073709551615",
                "\"dataspace_id\":0",
                1,
            ),
            raw.replacen(
                "\"dataspace_id\":18446744073709551615",
                "\"dataspace_id\":18446744073709551616",
                1,
            ),
            raw.replacen("\"version\"", "\"versi\\u006fn\"", 1),
            raw.replacen("\"version\"", "\"versión\"", 1),
            "{\"version\":1,\"bindings\":{}}".to_owned(),
        ];
        for invalid in invalid {
            assert_ne!(
                invalid.as_str(), raw.as_str(),
                "invalid fixture must change the retained JSON"
            );
            assert!(
                AssetDefinitionDataspaceRegistryReadPlan::from_canonical_json(&invalid).is_err()
            );
        }
    }

    #[test]
    fn borrowed_registry_plan_checks_hash_shape_key_identity_and_duplicate_keys() {
        let expected = registry(true);
        let binding = expected.bindings.values().next().unwrap();
        let encoded = norito::json::to_json_bounded(binding, MAX_JSON_BYTES).unwrap();
        let id = &binding.asset_definition_id;
        let valid = format!("{{\"version\":1,\"bindings\":{{\"{id}\":{encoded}}}}}");
        AssetDefinitionDataspaceRegistryReadPlan::from_canonical_json(&valid).unwrap();
        let other = AssetDefinitionId::derive_from_components(
            DomainId::parse_fully_qualified("fixture.universal").unwrap(),
            "other".parse().unwrap(),
        );
        let duplicate =
            format!("{{\"version\":1,\"bindings\":{{\"{id}\":{encoded},\"{id}\":{encoded}}}}}");
        let mismatched = format!("{{\"version\":1,\"bindings\":{{\"{other}\":{encoded}}}}}");
        let incarnation =
            norito::json::to_json_bounded(&binding.incarnation, MAX_JSON_BYTES).unwrap();
        assert!(incarnation.starts_with("[\"hash:") && incarnation.ends_with("\"]"));
        let bare = valid.replace(&incarnation, &incarnation[1..incarnation.len() - 1]);
        let broken_checksum = valid.replace(&incarnation, "[\"hash:01#0000\"]");
        for invalid in [duplicate, mismatched, bare, broken_checksum] {
            assert!(
                AssetDefinitionDataspaceRegistryReadPlan::from_canonical_json(&invalid).is_err()
            );
        }
        for bytes in [[0_u8; 32], [0xA0_u8; 32]] {
            let body = bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>();
            let literal = norito::literal::format("hash", &body);
            let invalid = valid.replace(&incarnation, &format!("[\"{literal}\"]"));
            assert!(
                AssetDefinitionDataspaceRegistryReadPlan::from_canonical_json(&invalid).is_err()
            );
        }
    }

    #[test]
    fn borrowed_registry_fill_refuses_short_or_nonempty_prepaid_destinations() {
        use iroha_allocation::{AllocationBudget, ChargedBuffer};
        let parameter = registry(true).into_custom_parameter().unwrap();
        let plan = AssetDefinitionDataspaceRegistryReadPlan::from_custom_parameter(&parameter)
            .unwrap()
            .unwrap();
        let budget =
            AllocationBudget::new(std::mem::size_of::<AssetDefinitionDataspaceBindingV1>());
        let mut short = ChargedBuffer::new(0, &budget).unwrap();
        assert!(plan.decode_into(&mut short).is_err());
        assert!(short.as_slice().is_empty());
        let mut full = ChargedBuffer::new(1, &budget).unwrap();
        plan.decode_into(&mut full).unwrap();
        let original = full.as_slice()[0].clone();
        assert!(plan.decode_into(&mut full).is_err());
        assert_eq!(full.as_slice(), &[original]);
    }

    #[test]
    fn borrowed_registry_reader_preserves_outer_parser_resource_refusal() {
        let parameter = registry(true).into_custom_parameter().unwrap();
        let limits = norito::DecodeLimits::new(0, MAX_JSON_BYTES, 0, 0, 32);
        let result = norito::with_decode_limits_scope(limits, || {
            AssetDefinitionDataspaceRegistryReadPlan::from_custom_parameter(&parameter)
        });
        assert!(matches!(
            result,
            Err(norito::json::Error::DecodeResourceLimit
                | norito::json::Error::ScopedDecodeResource(_))
        ));
        let plan = AssetDefinitionDataspaceRegistryReadPlan::from_custom_parameter(&parameter)
            .unwrap()
            .unwrap();
        let budget = iroha_allocation::AllocationBudget::new(std::mem::size_of::<
            AssetDefinitionDataspaceBindingV1,
        >());
        let mut rows = iroha_allocation::ChargedBuffer::new(1, &budget).unwrap();
        let result = norito::with_decode_limits_scope(limits, || plan.decode_into(&mut rows));
        assert!(matches!(
            result,
            Err(norito::json::Error::DecodeResourceLimit
                | norito::json::Error::ScopedDecodeResource(_))
        ));
        assert!(rows.as_slice().is_empty());
        plan.decode_into(&mut rows).unwrap();
    }

    #[test]
    fn protected_registry_roundtrips_live_bindings_and_tombstones() {
        for active in [true, false] {
            let original = registry(active);
            original.validate().expect("valid exact registry");
            let parameter = original
                .clone()
                .into_custom_parameter()
                .expect("native parameter");
            assert_eq!(
                parameter.id().to_string(),
                AssetDefinitionDataspaceRegistryV1::PARAMETER_ID_STR
            );
            let restored = AssetDefinitionDataspaceRegistryV1::from_custom_parameter(&parameter)
                .expect("bounded decode")
                .expect("exact reserved parameter");
            assert_eq!(restored, original);
            assert_eq!(restored.bindings.values().next().unwrap().active, active);
            assert_eq!(
                restored
                    .bindings
                    .values()
                    .next()
                    .unwrap()
                    .dataspace_id
                    .as_u64(),
                u64::MAX
            );
            let frame = norito::encode_canonical(&original).expect("encode typed registry");
            assert_eq!(
                norito::decode_canonical::<AssetDefinitionDataspaceRegistryV1>(&frame)
                    .expect("decode typed registry"),
                original
            );
            let unrelated = CustomParameter::new(
                "unrelated_parameter".parse().expect("parameter id"),
                parameter.payload().clone(),
            );
            assert!(
                AssetDefinitionDataspaceRegistryV1::from_custom_parameter(&unrelated)
                    .expect("unrelated parameter is not this registry")
                    .is_none()
            );
        }
    }

    #[test]
    fn protected_registry_rejects_empty_wrong_version_and_inconsistent_binding() {
        let mut empty = registry(true);
        empty.bindings.clear();
        assert!(empty.clone().into_custom_parameter().is_err());
        let mut wrong_version = registry(true);
        wrong_version.version = 2;
        assert!(wrong_version.validate().is_err());
        let mut reserved_dataspace = registry(true);
        reserved_dataspace
            .bindings
            .values_mut()
            .next()
            .unwrap()
            .dataspace_id = DataSpaceId::UNIVERSAL;
        assert!(reserved_dataspace.validate().is_err());
        let mut mismatched = registry(true);
        let entry = mismatched.bindings.values_mut().next().unwrap();
        let mut bytes = entry.asset_definition_id.aid_bytes();
        bytes[0] ^= 1;
        entry.asset_definition_id =
            AssetDefinitionId::from_uuid_bytes(bytes).expect("other UUIDv4");
        assert!(mismatched.validate().is_err());
        for invalid in [empty, wrong_version, reserved_dataspace, mismatched] {
            let unchecked = CustomParameter::new(
                AssetDefinitionDataspaceRegistryV1::parameter_id(),
                Json::try_new(invalid).expect("bounded but semantically invalid registry"),
            );
            assert!(AssetDefinitionDataspaceRegistryV1::from_custom_parameter(&unchecked).is_err());
        }
    }

    #[test]
    fn protected_registry_enforces_entry_and_json_decode_bounds() {
        let mut oversized = registry(true);
        let template = oversized.bindings.values().next().unwrap().clone();
        oversized.bindings.clear();
        for counter in 0..=MAX_ASSET_DEFINITION_DATASPACE_BINDINGS_V1 {
            let mut bytes = template.asset_definition_id.aid_bytes();
            bytes[..4].copy_from_slice(&u32::try_from(counter).unwrap().to_le_bytes());
            let id = AssetDefinitionId::from_uuid_bytes(bytes).expect("distinct fixture UUIDv4");
            let mut binding = template.clone();
            binding.asset_definition_id = id.clone();
            oversized.bindings.insert(id, binding);
        }
        assert!(oversized.validate().is_err());
        assert!(
            registry_preflight(&" ".repeat(MAX_ASSET_DEFINITION_DATASPACE_REGISTRY_BYTES_V1 + 1))
                .is_err()
        );
        for raw in [
            "{}",
            "{\"version\":1,\"bindings\":{},\"unexpected\":0}",
            "{\"version\":1,\"bindings\":{}}",
        ] {
            let value: norito::json::Value = norito::json::from_str(raw).expect("JSON fixture");
            let parameter = CustomParameter::new(
                AssetDefinitionDataspaceRegistryV1::parameter_id(),
                Json::from_norito_value_ref(&value).expect("canonical fixture JSON"),
            );
            assert!(AssetDefinitionDataspaceRegistryV1::from_custom_parameter(&parameter).is_err());
        }
    }

    fn definition(policy: AssetBalancePolicy, domain: Option<DomainId>) -> AssetDefinition {
        let id = AssetDefinitionId::from_uuid_bytes([
            0x91, 0x21, 0x63, 0x05, 0x0a, 0xb8, 0x46, 0x22, 0xab, 0x0d, 0x09, 0x0c, 0x31, 0x41,
            0x51, 0x61,
        ])
        .expect("public synthetic UUIDv4");
        let keys = KeyPair::try_from_seed(vec![0x6c; 32], Algorithm::Ed25519)
            .expect("public synthetic owner");
        AssetDefinition::numeric(id, "Native unit", policy, domain)
            .build(&AccountId::new(keys.public_key().clone()))
    }

    #[test]
    fn exact_committed_home_is_independent_of_balance_policy() {
        let domain = DomainId::try_new("issuer", "public").expect("domain");
        let dataspace = DataSpaceId::new(u64::MAX);
        for policy in [
            AssetBalancePolicy::Global,
            AssetBalancePolicy::DataspaceRestricted,
        ] {
            assert_eq!(
                AssetDefinitionHome::from_definition(&definition(policy, None), Some(dataspace))
                    .expect("direct home"),
                AssetDefinitionHome::Dataspace(dataspace)
            );
            assert_eq!(
                AssetDefinitionHome::from_definition(
                    &definition(policy, Some(domain.clone())),
                    None
                )
                .expect("domain home"),
                AssetDefinitionHome::Domain(domain.clone())
            );
            assert!(
                AssetDefinitionHome::from_definition(
                    &definition(policy, Some(domain.clone())),
                    Some(dataspace)
                )
                .is_err()
            );
            assert!(
                AssetDefinitionHome::from_definition(
                    &definition(policy, None),
                    Some(DataSpaceId::UNIVERSAL)
                )
                .is_err()
            );
        }
        assert_eq!(
            AssetDefinitionHome::from_definition(
                &definition(AssetBalancePolicy::Global, None),
                None
            )
            .expect("existing global home"),
            AssetDefinitionHome::Global
        );
        assert!(
            AssetDefinitionHome::from_definition(
                &definition(AssetBalancePolicy::DataspaceRestricted, None),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn alias_changes_cannot_rehome_a_definition() {
        let dataspace = DataSpaceId::new(u64::MAX - 17);
        let mut definition = definition(AssetBalancePolicy::DataspaceRestricted, None);
        for alias in [None, Some("unit#other".parse().expect("alias")), None] {
            definition.alias = alias;
            assert_eq!(
                AssetDefinitionHome::from_definition(&definition, Some(dataspace))
                    .expect("direct home"),
                AssetDefinitionHome::Dataspace(dataspace)
            );
        }
    }

    #[test]
    fn typed_homes_roundtrip_without_losing_dataspace_bits() {
        for home in [
            AssetDefinitionHome::Global,
            AssetDefinitionHome::Domain(DomainId::try_new("issuer", "public").expect("domain")),
            AssetDefinitionHome::Dataspace(DataSpaceId::new(u64::MAX)),
        ] {
            let bytes = norito::encode_canonical(&home).expect("encode home");
            assert_eq!(
                norito::decode_canonical::<AssetDefinitionHome>(&bytes).expect("decode home"),
                home
            );
            let json = norito::json::to_json(&home).expect("serialize home");
            assert_eq!(
                norito::json::from_json::<AssetDefinitionHome>(&json).expect("deserialize home"),
                home
            );
        }
    }
}
