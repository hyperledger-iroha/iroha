//! Built-in parameter definitions and validation logic.

use core::{
    convert::TryFrom,
    num::{NonZeroU16, NonZeroU64},
    time::Duration,
};
#[cfg(feature = "json")]
use std::collections::BTreeMap;
use std::sync::LazyLock;

use iroha_crypto::Algorithm;
use iroha_data_model_derive::model;
use iroha_primitives::json::Json;
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};

pub use self::model::*;
#[cfg(feature = "json")]
use super::custom::json_helpers;
use super::custom::{CustomParameter, CustomParameterId, CustomParameters};

#[cfg(feature = "json")]
mod json_support {
    use std::string::String;

    use super::*;

    pub(super) type Map = BTreeMap<String, json::Value>;

    pub(super) fn write_field<T: JsonSerialize>(
        out: &mut String,
        first: &mut bool,
        key: &str,
        value: &T,
    ) {
        if *first {
            *first = false;
        } else {
            out.push(',');
        }
        json::write_json_string(key, out);
        out.push(':');
        value.json_serialize(out);
    }

    pub(super) fn expect_object(value: json::Value, context: &str) -> Result<Map, json::Error> {
        match value {
            json::Value::Object(map) => Ok(map),
            _ => Err(json::Error::InvalidField {
                field: context.into(),
                message: String::from("expected object"),
            }),
        }
    }

    pub(super) fn ensure_no_extra(map: Map) -> Result<(), json::Error> {
        if let Some((field, _)) = map.into_iter().next() {
            return Err(json::Error::UnknownField { field });
        }
        Ok(())
    }

    pub(super) fn expect_u64(value: &json::Value, field: &str) -> Result<u64, json::Error> {
        match value {
            json::Value::Number(num) => num.as_u64().ok_or_else(|| json::Error::InvalidField {
                field: field.to_owned(),
                message: String::from("expected unsigned integer"),
            }),
            _ => Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: String::from("expected unsigned integer"),
            }),
        }
    }

    pub(super) fn expect_nonzero_u64(
        value: &json::Value,
        field: &str,
    ) -> Result<NonZeroU64, json::Error> {
        let raw = expect_u64(value, field)?;
        NonZeroU64::new(raw).ok_or_else(|| json::Error::InvalidField {
            field: field.to_owned(),
            message: String::from("expected non-zero integer"),
        })
    }

    pub(super) fn expect_bool(value: &json::Value, field: &str) -> Result<bool, json::Error> {
        match value {
            json::Value::Bool(b) => Ok(*b),
            _ => Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: String::from("expected boolean"),
            }),
        }
    }

    pub(super) fn expect_u8(value: &json::Value, field: &str) -> Result<u8, json::Error> {
        let raw = expect_u64(value, field)?;
        u8::try_from(raw).map_err(|_| json::Error::InvalidField {
            field: field.to_owned(),
            message: String::from("value out of range for u8"),
        })
    }

    pub(super) fn expect_u16(value: &json::Value, field: &str) -> Result<u16, json::Error> {
        let raw = expect_u64(value, field)?;
        u16::try_from(raw).map_err(|_| json::Error::InvalidField {
            field: field.to_owned(),
            message: String::from("value out of range for u16"),
        })
    }

    pub(super) fn expect_u32(value: &json::Value, field: &str) -> Result<u32, json::Error> {
        let raw = expect_u64(value, field)?;
        u32::try_from(raw).map_err(|_| json::Error::InvalidField {
            field: field.to_owned(),
            message: String::from("value out of range for u32"),
        })
    }

    pub(super) fn parse_value_as<T>(value: &json::Value) -> Result<T, json::Error>
    where
        T: JsonDeserialize,
    {
        let json =
            norito::json::to_json(value).map_err(|err| json::Error::Message(err.to_string()))?;
        let mut parser = norito::json::Parser::new(&json);
        T::json_deserialize(&mut parser)
    }
}

#[cfg(feature = "json")]
fn parse_custom_parameters_value(value: json::Value) -> Result<CustomParameters, json::Error> {
    let map = json_support::expect_object(value, "Parameters.custom")?;
    let value = json::Value::Object(map);
    json_helpers::from_value(value)
}
#[model]
mod model {
    use derive_more::Display;
    use getset::{CopyGetters, Getters};
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Consensus runtime mode
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
    )]
    pub enum SumeragiConsensusMode {
        /// Classic permissioned Sumeragi
        Permissioned,
        /// Nominated Proof‑of‑Stake mode
        Npos,
    }

    // Bridge Norito codec to core slice decoding for strict-safe consumers.
    impl<'a> norito::core::DecodeFromSlice<'a> for SumeragiConsensusMode {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            let mut s: &'a [u8] = bytes;
            let value = <Self as norito::codec::DecodeAll>::decode_all(&mut s)
                .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
            let used = bytes.len() - s.len();
            Ok((value, used))
        }
    }

    /// Limits that govern consensus operation
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[display(
        "{block_time_ms},{commit_time_ms},{min_finality_ms},{pacing_factor_bps},{max_clock_drift_ms}_SL"
    )]
    pub struct SumeragiParameters {
        /// Maximal amount of time (in milliseconds) a peer will wait before forcing creation of a new block.
        ///
        /// A block is created if this limit or [`BlockParameters::max_transactions`] limit is reached,
        /// whichever comes first. Regardless of the limits, an empty block is never created.
        /// This value is authoritative for both permissioned and `NPoS` pacemaker timing.
        #[norito(default = "defaults::sumeragi::block_time_ms")]
        pub block_time_ms: u64,
        /// Time (in milliseconds) a peer will wait for a block to be committed.
        ///
        /// If this period expires the block will request a view change
        #[norito(default = "defaults::sumeragi::commit_time_ms")]
        pub commit_time_ms: u64,
        /// Minimum finality floor (in milliseconds) for consensus timing.
        ///
        /// All derived timeouts are clamped to be at least this value.
        #[norito(default = "defaults::sumeragi::min_finality_ms")]
        pub min_finality_ms: u64,
        /// Pacing factor applied to block/commit timing (basis points, `10_000` = 1.0x).
        #[norito(default = "defaults::sumeragi::pacing_factor_bps")]
        pub pacing_factor_bps: u32,
        /// Maximal allowed random deviation from the nominal rate
        ///
        /// # Warning
        ///
        /// This value should be kept as low as possible to not affect soundness of the consensus
        #[norito(default = "defaults::sumeragi::max_clock_drift_ms")]
        pub max_clock_drift_ms: u64,
        /// Number of collectors designated per height (contiguous from proxy tail, no wraparound).
        #[norito(default = "defaults::sumeragi::collectors_k")]
        pub collectors_k: u16,
        /// Redundant send fanout (how many distinct collectors a validator may target on timeouts).
        #[norito(default = "defaults::sumeragi::redundant_send_r")]
        pub collectors_redundant_send_r: u8,
        /// Enable data availability for Sumeragi.
        ///
        /// When enabled, block payload distribution uses Reliable Broadcast (RBC) and consensus
        /// gates finalization on availability evidence. When disabled, RBC payload dissemination
        /// is off and nodes rely on direct payloads.
        #[norito(default = "defaults::sumeragi::da_enabled")]
        pub da_enabled: bool,
        /// If set, indicates the next consensus mode to switch to at `mode_activation_height`.
        #[norito(default)]
        pub next_mode: Option<SumeragiConsensusMode>,
        /// If set, height at which to activate `next_mode`.
        #[norito(default)]
        pub mode_activation_height: Option<u64>,
        /// Minimum lead time (blocks) between publishing a new consensus key and activation.
        #[norito(default = "defaults::sumeragi::key_activation_lead_blocks")]
        pub key_activation_lead_blocks: u64,
        /// Overlap/grace window (blocks) permitting dual-signing during rotation.
        #[norito(default = "defaults::sumeragi::key_overlap_grace_blocks")]
        pub key_overlap_grace_blocks: u64,
        /// Expiry grace window (blocks) after declared expiry.
        #[norito(default = "defaults::sumeragi::key_expiry_grace_blocks")]
        pub key_expiry_grace_blocks: u64,
        /// Require HSM binding for consensus/committee keys.
        #[norito(default = "defaults::sumeragi::key_require_hsm")]
        pub key_require_hsm: bool,
        /// Allowed algorithms for consensus/committee keys.
        #[norito(default = "defaults::sumeragi::key_allowed_algorithms")]
        pub key_allowed_algorithms: Vec<iroha_crypto::Algorithm>,
        /// Allowed HSM providers for consensus/committee keys.
        #[norito(default = "defaults::sumeragi::key_allowed_hsm_providers")]
        pub key_allowed_hsm_providers: Vec<String>,
    }

    /// NPoS-specific consensus parameters persisted as a custom parameter payload.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        norito::derive::JsonSerialize,
        norito::derive::JsonDeserialize,
        Encode,
        Decode,
        IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct SumeragiNposParameters {
        /// Deterministic epoch seed used for PRF-based leader and collector selection.
        ///
        /// When absent in older payloads this defaults to all-zero bytes; nodes SHOULD ensure
        /// production manifests set a chain-specific, random seed to avoid correlated schedules.
        #[norito(default)]
        pub epoch_seed: [u8; 32],
        /// Number of aggregators (K) per round.
        pub k_aggregators: u16,
        /// Redundant send fanout (distinct aggregators contacted over time).
        pub redundant_send_r: u8,
        /// VRF commit window length in blocks.
        pub vrf_commit_window_blocks: u64,
        /// VRF reveal window length in blocks.
        pub vrf_reveal_window_blocks: u64,
        /// Maximum validators to elect for the next epoch (0 = unlimited).
        pub max_validators: u32,
        /// Minimum self-bond required for validator eligibility.
        pub min_self_bond: u64,
        /// Minimum nomination bond required for delegators.
        pub min_nomination_bond: u64,
        /// Maximum nominator concentration percentage.
        pub max_nominator_concentration_pct: u8,
        /// Seat allocation variance band percentage.
        pub seat_band_pct: u8,
        /// Maximum correlation percentage across validator entities.
        pub max_entity_correlation_pct: u8,
        /// Finality margin in blocks before activating a newly elected set.
        pub finality_margin_blocks: u64,
        /// Evidence retention horizon in blocks.
        pub evidence_horizon_blocks: u64,
        /// Activation lag in blocks for newly scheduled validator sets.
        pub activation_lag_blocks: u64,
        /// Slashing delay in blocks before evidence penalties apply.
        pub slashing_delay_blocks: u64,
        /// Epoch length in blocks.
        pub epoch_length_blocks: u64,
    }

    impl SumeragiNposParameters {
        /// Identifier used for the custom parameter storing `NPoS` tunables.
        pub const PARAMETER_ID_STR: &'static str = "sumeragi_npos_parameters";

        /// Construct the [`CustomParameterId`] associated with this payload.
        #[must_use]
        pub fn parameter_id() -> CustomParameterId {
            Self::PARAMETER_ID_STR
                .parse()
                .expect("valid sumeragi_npos custom parameter identifier")
        }

        /// Convert the payload into a [`CustomParameter`].
        #[must_use]
        pub fn into_custom_parameter(self) -> CustomParameter {
            CustomParameter::new(Self::parameter_id(), Json::new(self))
        }

        /// Attempt to decode this payload from a [`CustomParameter`].
        #[must_use]
        pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Self> {
            if custom.id != Self::parameter_id() {
                return None;
            }
            Self::parse_payload_compat(custom.payload().get())
        }

        fn parse_payload_compat(raw: &str) -> Option<Self> {
            norito::json::from_str::<Self>(raw)
                .ok()
                .or_else(|| {
                    let value = norito::json::parse_value(raw).ok()?;
                    Self::parse_value_compat(value)
                })
                .or_else(|| Self::parse_payload_text_compat(raw))
        }

        fn parse_value_compat(value: norito::json::Value) -> Option<Self> {
            if let Ok(parsed) = norito::json::from_value::<Self>(value.clone()) {
                return Some(parsed);
            }

            if let Some(inner) = value.as_str() {
                return Self::parse_payload_compat(inner);
            }

            let mut normalized = value;
            Self::normalize_numeric_string_payload(&mut normalized);
            if let Ok(parsed) = norito::json::from_value::<Self>(normalized.clone()) {
                return Some(parsed);
            }

            Self::normalize_epoch_seed_hex_payload(&mut normalized);
            norito::json::from_value(normalized).ok()
        }

        fn normalize_numeric_string_payload(value: &mut norito::json::Value) {
            let Some(map) = value.as_object_mut() else {
                return;
            };

            for field in Self::numeric_string_compat_fields() {
                let Some(raw_value) = map.get_mut(*field) else {
                    continue;
                };
                let Some(raw_string) = raw_value.as_str() else {
                    continue;
                };
                if let Ok(parsed) = raw_string.parse::<u64>() {
                    *raw_value = norito::json::Value::from(parsed);
                }
            }
        }

        fn normalize_epoch_seed_hex_payload(value: &mut norito::json::Value) {
            let Some(map) = value.as_object_mut() else {
                return;
            };
            let Some(seed_value) = map.get_mut("epoch_seed") else {
                return;
            };
            let Some(seed_raw) = seed_value.as_str() else {
                return;
            };
            let Some(seed_bytes) = Self::decode_epoch_seed_hex(seed_raw) else {
                return;
            };
            *seed_value = norito::json::Value::String(Self::encode_epoch_seed_hex(seed_bytes));
        }

        fn parse_payload_text_compat(raw: &str) -> Option<Self> {
            if let Some(parsed) = Self::parse_payload_text_object(raw) {
                return Some(parsed);
            }
            let unescaped = Self::unescape_wrapped_json(raw.trim())?;
            Self::parse_payload_text_object(&unescaped)
        }

        fn parse_payload_text_object(raw: &str) -> Option<Self> {
            let raw = raw.trim();
            if !raw.starts_with('{') || !raw.ends_with('}') {
                return None;
            }

            let epoch_seed =
                Self::decode_epoch_seed_hex(&Self::extract_string_field(raw, "epoch_seed")?)?;

            let k_aggregators =
                u16::try_from(Self::extract_u64_field(raw, "k_aggregators")?).ok()?;
            let redundant_send_r =
                u8::try_from(Self::extract_u64_field(raw, "redundant_send_r")?).ok()?;
            let vrf_commit_window_blocks =
                Self::extract_u64_field(raw, "vrf_commit_window_blocks")?;
            let vrf_reveal_window_blocks =
                Self::extract_u64_field(raw, "vrf_reveal_window_blocks")?;
            let max_validators =
                u32::try_from(Self::extract_u64_field(raw, "max_validators")?).ok()?;
            let min_self_bond = Self::extract_u64_field(raw, "min_self_bond")?;
            let min_nomination_bond = Self::extract_u64_field(raw, "min_nomination_bond")?;
            let max_nominator_concentration_pct = u8::try_from(Self::extract_u64_field(
                raw,
                "max_nominator_concentration_pct",
            )?)
            .ok()?;
            let seat_band_pct =
                u8::try_from(Self::extract_u64_field(raw, "seat_band_pct")?).ok()?;
            let max_entity_correlation_pct =
                u8::try_from(Self::extract_u64_field(raw, "max_entity_correlation_pct")?).ok()?;
            let finality_margin_blocks = Self::extract_u64_field(raw, "finality_margin_blocks")?;
            let evidence_horizon_blocks = Self::extract_u64_field(raw, "evidence_horizon_blocks")?;
            let activation_lag_blocks = Self::extract_u64_field(raw, "activation_lag_blocks")?;
            let slashing_delay_blocks = Self::extract_u64_field(raw, "slashing_delay_blocks")?;
            let epoch_length_blocks = Self::extract_u64_field(raw, "epoch_length_blocks")?;

            Some(Self {
                epoch_seed,
                k_aggregators,
                redundant_send_r,
                vrf_commit_window_blocks,
                vrf_reveal_window_blocks,
                max_validators,
                min_self_bond,
                min_nomination_bond,
                max_nominator_concentration_pct,
                seat_band_pct,
                max_entity_correlation_pct,
                finality_margin_blocks,
                evidence_horizon_blocks,
                activation_lag_blocks,
                slashing_delay_blocks,
                epoch_length_blocks,
            })
        }

        fn extract_u64_field(raw: &str, field: &str) -> Option<u64> {
            let value = Self::field_value_slice(raw, field)?;
            let value = value.trim_start();
            let digits = if let Some(rest) = value.strip_prefix('"') {
                rest.split('"').next()?
            } else {
                let end = value
                    .find(|ch: char| !ch.is_ascii_digit())
                    .unwrap_or(value.len());
                &value[..end]
            };
            if digits.is_empty() {
                return None;
            }
            digits.parse::<u64>().ok()
        }

        fn extract_string_field(raw: &str, field: &str) -> Option<String> {
            let value = Self::field_value_slice(raw, field)?;
            let value = value.trim_start();
            let rest = value.strip_prefix('"')?;
            let mut out = String::new();
            let mut chars = rest.chars();
            while let Some(ch) = chars.next() {
                match ch {
                    '"' => return Some(out),
                    '\\' => {
                        let escaped = chars.next()?;
                        match escaped {
                            '"' | '\\' | '/' => out.push(escaped),
                            'b' => out.push('\u{0008}'),
                            'f' => out.push('\u{000c}'),
                            'n' => out.push('\n'),
                            'r' => out.push('\r'),
                            't' => out.push('\t'),
                            'u' => {
                                let mut scalar = 0_u32;
                                for _ in 0..4 {
                                    let hex = chars.next()?;
                                    scalar = (scalar << 4) | hex.to_digit(16)?;
                                }
                                out.push(char::from_u32(scalar)?);
                            }
                            _ => return None,
                        }
                    }
                    _ => out.push(ch),
                }
            }
            None
        }

        fn field_value_slice<'a>(raw: &'a str, field: &str) -> Option<&'a str> {
            let needle = format!("\"{field}\"");
            let key_start = raw.find(&needle)?;
            let after_key = &raw[key_start + needle.len()..];
            let colon = after_key.find(':')?;
            Some(&after_key[colon + 1..])
        }

        fn unescape_wrapped_json(raw: &str) -> Option<String> {
            let raw = raw.trim();
            let inner = raw.strip_prefix('"')?.strip_suffix('"')?;
            let mut out = String::with_capacity(inner.len());
            let mut chars = inner.chars();
            while let Some(ch) = chars.next() {
                if ch != '\\' {
                    out.push(ch);
                    continue;
                }
                let escaped = chars.next()?;
                match escaped {
                    '"' | '\\' | '/' => out.push(escaped),
                    'b' => out.push('\u{0008}'),
                    'f' => out.push('\u{000c}'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'u' => {
                        let mut scalar = 0_u32;
                        for _ in 0..4 {
                            let hex = chars.next()?;
                            scalar = (scalar << 4) | hex.to_digit(16)?;
                        }
                        out.push(char::from_u32(scalar)?);
                    }
                    _ => return None,
                }
            }
            Some(out)
        }

        fn decode_epoch_seed_hex(raw: &str) -> Option<[u8; 32]> {
            let raw = raw.trim();
            let raw = raw
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(raw);
            let hex = raw
                .strip_prefix("0x")
                .or_else(|| raw.strip_prefix("0X"))
                .unwrap_or(raw);
            if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return None;
            }

            let mut out = [0_u8; 32];
            for (idx, slot) in out.iter_mut().enumerate() {
                let start = idx * 2;
                let end = start + 2;
                *slot = u8::from_str_radix(&hex[start..end], 16).ok()?;
            }
            Some(out)
        }

        fn encode_epoch_seed_hex(seed: [u8; 32]) -> String {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            let mut out = String::with_capacity(64);
            for byte in seed {
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0F) as usize] as char);
            }
            out
        }

        fn numeric_string_compat_fields() -> &'static [&'static str] {
            &[
                "k_aggregators",
                "redundant_send_r",
                "vrf_commit_window_blocks",
                "vrf_reveal_window_blocks",
                "max_validators",
                "min_self_bond",
                "min_nomination_bond",
                "max_nominator_concentration_pct",
                "seat_band_pct",
                "max_entity_correlation_pct",
                "finality_margin_blocks",
                "evidence_horizon_blocks",
                "activation_lag_blocks",
                "slashing_delay_blocks",
                "epoch_length_blocks",
            ]
        }

        /// Number of aggregators expected per round.
        #[must_use]
        pub fn k_aggregators(&self) -> u16 {
            self.k_aggregators
        }

        /// Redundant send factor for Sumeragi messages.
        #[must_use]
        pub fn redundant_send_r(&self) -> u8 {
            self.redundant_send_r
        }

        /// VRF commit window measured in blocks.
        #[must_use]
        pub fn vrf_commit_window_blocks(&self) -> u64 {
            self.vrf_commit_window_blocks
        }

        /// VRF reveal window measured in blocks.
        #[must_use]
        pub fn vrf_reveal_window_blocks(&self) -> u64 {
            self.vrf_reveal_window_blocks
        }

        /// Maximum validators to elect for the next epoch (0 = unlimited).
        #[must_use]
        pub fn max_validators(&self) -> u32 {
            self.max_validators
        }

        /// Minimum self-bonded stake required for validators.
        #[must_use]
        pub fn min_self_bond(&self) -> u64 {
            self.min_self_bond
        }

        /// Minimum nomination bond required for delegators.
        #[must_use]
        pub fn min_nomination_bond(&self) -> u64 {
            self.min_nomination_bond
        }

        /// Maximum percentage of stake concentrated under a single nominator.
        #[must_use]
        pub fn max_nominator_concentration_pct(&self) -> u8 {
            self.max_nominator_concentration_pct
        }

        /// Seat band percentage used when selecting validators.
        #[must_use]
        pub fn seat_band_pct(&self) -> u8 {
            self.seat_band_pct
        }

        /// Maximum correlation percentage allowed across validator entities.
        #[must_use]
        pub fn max_entity_correlation_pct(&self) -> u8 {
            self.max_entity_correlation_pct
        }

        /// Finality margin in blocks before activating a newly elected set.
        #[must_use]
        pub fn finality_margin_blocks(&self) -> u64 {
            self.finality_margin_blocks
        }

        /// Number of blocks for which slashing evidence remains valid.
        #[must_use]
        pub fn evidence_horizon_blocks(&self) -> u64 {
            self.evidence_horizon_blocks
        }

        /// Number of blocks to wait before activating newly elected validators.
        #[must_use]
        pub fn activation_lag_blocks(&self) -> u64 {
            self.activation_lag_blocks
        }

        /// Slashing delay (blocks) before evidence penalties apply.
        #[must_use]
        pub fn slashing_delay_blocks(&self) -> u64 {
            self.slashing_delay_blocks
        }

        /// Epoch length measured in blocks.
        #[must_use]
        pub fn epoch_length_blocks(&self) -> u64 {
            self.epoch_length_blocks
        }

        /// Epoch seed used for randomness-dependent protocols.
        #[must_use]
        pub fn epoch_seed(&self) -> [u8; 32] {
            self.epoch_seed
        }

        /// Override the epoch seed and return the updated payload.
        #[must_use]
        pub fn with_epoch_seed(mut self, value: [u8; 32]) -> Self {
            self.epoch_seed = value;
            self
        }
    }

    impl Default for SumeragiNposParameters {
        fn default() -> Self {
            use defaults::sumeragi::npos::*;
            Self {
                epoch_seed: [0u8; 32],
                k_aggregators: k_aggregators(),
                redundant_send_r: redundant_send_r(),
                vrf_commit_window_blocks: vrf_commit_window_blocks(),
                vrf_reveal_window_blocks: vrf_reveal_window_blocks(),
                max_validators: max_validators(),
                min_self_bond: min_self_bond(),
                min_nomination_bond: min_nomination_bond(),
                max_nominator_concentration_pct: max_nominator_concentration_pct(),
                seat_band_pct: seat_band_pct(),
                max_entity_correlation_pct: max_entity_correlation_pct(),
                finality_margin_blocks: finality_margin_blocks(),
                evidence_horizon_blocks: evidence_horizon_blocks(),
                activation_lag_blocks: activation_lag_blocks(),
                slashing_delay_blocks: slashing_delay_blocks(),
                epoch_length_blocks: epoch_length_blocks(),
            }
        }
    }

    impl From<SumeragiNposParameters> for CustomParameter {
        fn from(value: SumeragiNposParameters) -> Self {
            value.into_custom_parameter()
        }
    }

    /// Single Sumeragi parameter
    ///
    /// Check [`SumeragiParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum SumeragiParameter {
        BlockTimeMs(u64),
        CommitTimeMs(u64),
        MinFinalityMs(u64),
        /// Pacing factor (basis points, `10_000` = 1.0x).
        PacingFactorBps(u32),
        MaxClockDriftMs(u64),
        /// Number of collectors per height (K). Must be >= 1.
        CollectorsK(u16),
        /// Redundant send fanout (r). Must be >= 1.
        RedundantSendR(u8),
        /// Enable/disable data availability (RBC payload dissemination and availability gating).
        DaEnabled(bool),
        /// Set the next consensus mode for future activation.
        NextMode(SumeragiConsensusMode),
        /// Set the height to activate `next_mode`.
        ModeActivationHeight(u64),
    }

    /// Limits that a block must obey to be accepted.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{max_transactions}_BL")]
    #[getset(get_copy = "pub")]
    pub struct BlockParameters {
        /// Maximal number of transactions in a block.
        ///
        /// A block is created if this limit is reached or [`SumeragiParameters::block_time_ms`] has expired,
        /// whichever comes first. Regardless of the limits, an empty block is never created.
        pub max_transactions: NonZeroU64,
    }

    /// Single block parameter
    ///
    /// Check [`BlockParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum BlockParameter {
        MaxTransactions(NonZeroU64),
    }

    /// Limits that a transaction must obey to be accepted.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display(
        "{max_signatures},{max_instructions},{ivm_bytecode_size},{max_tx_bytes},{max_decompressed_bytes},{max_metadata_depth}_TL"
    )]
    #[getset(get_copy = "pub")]
    pub struct TransactionParameters {
        /// Maximum number of signatures allowed per transaction (non-zero).
        pub max_signatures: NonZeroU64,
        /// Maximum number of instructions per transaction
        pub max_instructions: NonZeroU64,
        /// Maximum size of IVM bytecode, in bytes
        pub ivm_bytecode_size: NonZeroU64,
        /// Maximum encoded transaction size accepted at admission (bytes).
        pub max_tx_bytes: NonZeroU64,
        /// Maximum post-decompression size accepted at admission (bytes).
        pub max_decompressed_bytes: NonZeroU64,
        /// Maximum allowed JSON nesting depth across transaction metadata values.
        pub max_metadata_depth: NonZeroU16,
        /// Enforce height-based TTL metadata (`expires_at_height`) at admission.
        #[norito(default)]
        pub require_height_ttl: bool,
        /// Enforce strictly monotonic per-sender sequence metadata (`tx_sequence`) at admission.
        #[norito(default)]
        pub require_sequence: bool,
    }

    /// Single transaction parameter
    ///
    /// Check [`TransactionParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum TransactionParameter {
        MaxSignatures(NonZeroU64),
        MaxInstructions(NonZeroU64),
        IvmBytecodeSize(NonZeroU64),
        MaxTxBytes(NonZeroU64),
        MaxDecompressedBytes(NonZeroU64),
        MaxMetadataDepth(NonZeroU16),
        RequireHeightTtl(bool),
        RequireSequence(bool),
    }

    /// Limits that a smart contract must obey at runtime to considered valid.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{fuel},{memory}_SCL")]
    #[getset(get_copy = "pub")]
    pub struct SmartContractParameters {
        /// Maximum amount of fuel that a smart contract can consume
        pub fuel: NonZeroU64,
        /// Maximum amount of memory that a smart contract can use
        pub memory: NonZeroU64,
        /// Maximum length of chained data trigger executions
        pub execution_depth: u8,
    }

    /// Single smart contract parameter
    ///
    /// Check [`SmartContractParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum SmartContractParameter {
        Fuel(NonZeroU64),
        Memory(NonZeroU64),
        ExecutionDepth(u8),
    }

    /// Set of all current blockchain parameter values
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Default,
        Getters,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    pub struct Parameters {
        /// Sumeragi parameters
        #[getset(get = "pub")]
        #[norito(default)]
        pub sumeragi: SumeragiParameters,
        /// Block parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub block: BlockParameters,
        /// Transaction parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub transaction: TransactionParameters,
        /// Executor parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub executor: SmartContractParameters,
        /// Smart contract parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub smart_contract: SmartContractParameters,
        /// Collection of blockchain specific parameters
        #[getset(get = "pub")]
        #[norito(default)]
        #[norito(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        pub custom: CustomParameters,
    }

    /// Single blockchain parameter.
    ///
    /// Check [`Parameters`] for more details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum Parameter {
        Sumeragi(SumeragiParameter),
        Block(BlockParameter),
        Transaction(TransactionParameter),
        SmartContract(SmartContractParameter),
        Executor(SmartContractParameter),
        Custom(CustomParameter),
    }
}

impl core::fmt::Display for Parameter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Sumeragi(v) => core::fmt::Display::fmt(&v, f),
            Self::Block(v) => core::fmt::Display::fmt(&v, f),
            Self::Transaction(v) => core::fmt::Display::fmt(&v, f),
            Self::SmartContract(v) | Self::Executor(v) => core::fmt::Display::fmt(&v, f),
            Self::Custom(v) => write!(f, "{}({})", v.id, v.payload),
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for Parameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            Parameter::Sumeragi(value) => {
                json::write_json_string("Sumeragi", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Block(value) => {
                json::write_json_string("Block", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Transaction(value) => {
                json::write_json_string("Transaction", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::SmartContract(value) => {
                json::write_json_string("SmartContract", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Executor(value) => {
                json::write_json_string("Executor", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Custom(value) => {
                json::write_json_string("Custom", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Parameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "Parameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("Parameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "Sumeragi" => Ok(Self::Sumeragi(json_support::parse_value_as::<
                SumeragiParameter,
            >(&payload)?)),
            "Block" => Ok(Self::Block(json_support::parse_value_as::<BlockParameter>(
                &payload,
            )?)),
            "Transaction" => Ok(Self::Transaction(json_support::parse_value_as::<
                TransactionParameter,
            >(&payload)?)),
            "SmartContract" => Ok(Self::SmartContract(json_support::parse_value_as::<
                SmartContractParameter,
            >(&payload)?)),
            "Executor" => Ok(Self::Executor(json_support::parse_value_as::<
                SmartContractParameter,
            >(&payload)?)),
            "Custom" => Ok(Self::Custom(
                json_support::parse_value_as::<CustomParameter>(&payload)?,
            )),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SumeragiConsensusMode {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            SumeragiConsensusMode::Permissioned => "Permissioned",
            SumeragiConsensusMode::Npos => "Npos",
        };
        json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SumeragiConsensusMode {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Permissioned" => Ok(SumeragiConsensusMode::Permissioned),
            "Npos" => Ok(SumeragiConsensusMode::Npos),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

// (Codecs are provided by derives and candidate decoders. Tests use wrappers where needed.)

impl SumeragiParameters {
    /// Raw block time in milliseconds.
    #[must_use]
    pub fn block_time_ms(&self) -> u64 {
        self.block_time_ms
    }

    /// Raw commit time in milliseconds.
    #[must_use]
    pub fn commit_time_ms(&self) -> u64 {
        self.commit_time_ms
    }

    /// Raw minimum finality floor in milliseconds.
    #[must_use]
    pub fn min_finality_ms(&self) -> u64 {
        self.min_finality_ms
    }

    /// Raw pacing factor in basis points (`10_000` = 1.0x).
    #[must_use]
    pub fn pacing_factor_bps(&self) -> u32 {
        self.pacing_factor_bps
    }

    /// Raw clock drift bound in milliseconds.
    #[must_use]
    pub fn max_clock_drift_ms(&self) -> u64 {
        self.max_clock_drift_ms
    }

    /// Number of collectors designated per height.
    #[must_use]
    pub fn collectors_k(&self) -> u16 {
        self.collectors_k
    }

    /// Redundant send fanout per validator.
    #[must_use]
    pub fn collectors_redundant_send_r(&self) -> u8 {
        self.collectors_redundant_send_r
    }

    /// Whether data availability (RBC payload dissemination and availability gating) is enabled.
    #[must_use]
    pub fn da_enabled(&self) -> bool {
        self.da_enabled
    }

    /// Next consensus mode requested via parameter updates, if any.
    #[must_use]
    pub fn next_mode(&self) -> Option<SumeragiConsensusMode> {
        self.next_mode
    }

    /// Activation height for the requested consensus mode, if present.
    #[must_use]
    pub fn mode_activation_height(&self) -> Option<u64> {
        self.mode_activation_height
    }

    /// Effective minimum finality floor in milliseconds (clamped to at least 1ms).
    #[must_use]
    pub fn effective_min_finality_ms(&self) -> u64 {
        self.min_finality_ms.max(1)
    }

    /// Effective pacing factor (basis points, clamped to >= `10_000`).
    #[must_use]
    pub fn effective_pacing_factor_bps(&self) -> u32 {
        self.pacing_factor_bps.max(10_000)
    }

    /// Maximal allowed random deviation from the nominal rate
    ///
    /// # Warning
    ///
    /// This value should be kept as low as possible to not affect soundness of the consensus
    pub fn max_clock_drift(&self) -> Duration {
        Duration::from_millis(self.max_clock_drift_ms)
    }

    /// Effective minimum finality floor duration.
    #[must_use]
    pub fn min_finality(&self) -> Duration {
        Duration::from_millis(self.effective_min_finality_ms())
    }

    /// Maximal amount of time (in milliseconds) a peer will wait before forcing creation of a new block.
    ///
    /// A block is created if this limit or [`BlockParameters::max_transactions`] limit is reached,
    /// whichever comes first. Regardless of the limits, an empty block is never created.
    /// This value is authoritative for both permissioned and `NPoS` pacemaker timing.
    pub fn block_time(&self) -> Duration {
        Duration::from_millis(self.block_time_ms)
    }

    /// Effective block time duration clamped to `min_finality_ms`.
    #[must_use]
    pub fn effective_block_time(&self) -> Duration {
        Duration::from_millis(self.effective_block_time_ms())
    }

    /// Time (in milliseconds) a peer will wait for a block to be committed.
    ///
    /// If this period expires the block will request a view change
    pub fn commit_time(&self) -> Duration {
        Duration::from_millis(self.commit_time_ms)
    }

    /// Effective commit time duration clamped to `block_time_ms`.
    #[must_use]
    pub fn effective_commit_time(&self) -> Duration {
        Duration::from_millis(self.effective_commit_time_ms())
    }

    /// Effective block time in milliseconds (clamped to `min_finality_ms`).
    #[must_use]
    pub fn effective_block_time_ms(&self) -> u64 {
        let floor = self.effective_min_finality_ms();
        let base_ms = self.block_time_ms.max(floor);
        let scaled = Self::apply_pacing_factor_ms(base_ms, self.effective_pacing_factor_bps());
        scaled.max(floor)
    }

    /// Effective commit time in milliseconds (clamped to `block_time_ms`).
    #[must_use]
    pub fn effective_commit_time_ms(&self) -> u64 {
        let base_ms = self.commit_time_ms.max(self.block_time_ms);
        let scaled = Self::apply_pacing_factor_ms(base_ms, self.effective_pacing_factor_bps());
        scaled.max(self.effective_block_time_ms())
    }

    /// Validate timing constraints for this parameter set.
    ///
    /// # Errors
    /// Returns an error string when `min_finality_ms`, `pacing_factor_bps`,
    /// `block_time_ms`, or `commit_time_ms` violate the required ordering
    /// and lower-bound invariants.
    pub fn validate_timing(&self) -> Result<(), &'static str> {
        if self.min_finality_ms == 0 {
            return Err("min_finality_ms must be greater than zero");
        }
        if self.pacing_factor_bps < 10_000 {
            return Err("pacing_factor_bps must be greater than or equal to 10_000");
        }
        if self.block_time_ms < self.min_finality_ms {
            return Err("block_time_ms must be greater than or equal to min_finality_ms");
        }
        if self.commit_time_ms < self.block_time_ms {
            return Err("commit_time_ms must be greater than or equal to block_time_ms");
        }
        Ok(())
    }

    fn apply_pacing_factor_ms(base_ms: u64, factor_bps: u32) -> u64 {
        let scaled = u128::from(base_ms)
            .saturating_mul(u128::from(factor_bps))
            .saturating_div(10_000);
        u64::try_from(scaled).unwrap_or(u64::MAX)
    }

    /// Maximal amount of time it takes to commit a block
    #[cfg(feature = "transparent_api")]
    pub fn pipeline_time(&self, view_change_index: u64, shift: usize) -> Duration {
        let shifted_view_change_index = view_change_index.saturating_sub(shift as u64);
        self.block_time().saturating_add(
            self.commit_time().saturating_mul(
                shifted_view_change_index
                    .saturating_add(1)
                    .try_into()
                    .unwrap_or(u32::MAX),
            ),
        )
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SumeragiParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            SumeragiParameter::BlockTimeMs(v) => {
                json::write_json_string("BlockTimeMs", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::CommitTimeMs(v) => {
                json::write_json_string("CommitTimeMs", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::MinFinalityMs(v) => {
                json::write_json_string("MinFinalityMs", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::PacingFactorBps(v) => {
                json::write_json_string("PacingFactorBps", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::MaxClockDriftMs(v) => {
                json::write_json_string("MaxClockDriftMs", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::CollectorsK(v) => {
                json::write_json_string("CollectorsK", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::RedundantSendR(v) => {
                json::write_json_string("RedundantSendR", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::DaEnabled(v) => {
                json::write_json_string("DaEnabled", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::NextMode(v) => {
                json::write_json_string("NextMode", out);
                out.push(':');
                v.json_serialize(out);
            }
            SumeragiParameter::ModeActivationHeight(v) => {
                json::write_json_string("ModeActivationHeight", out);
                out.push(':');
                v.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SumeragiParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "SumeragiParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("SumeragiParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "BlockTimeMs" => Ok(Self::BlockTimeMs(json_support::expect_u64(
                &payload,
                "BlockTimeMs",
            )?)),
            "CommitTimeMs" => Ok(Self::CommitTimeMs(json_support::expect_u64(
                &payload,
                "CommitTimeMs",
            )?)),
            "MinFinalityMs" => Ok(Self::MinFinalityMs(json_support::expect_u64(
                &payload,
                "MinFinalityMs",
            )?)),
            "PacingFactorBps" => Ok(Self::PacingFactorBps(json_support::expect_u32(
                &payload,
                "PacingFactorBps",
            )?)),
            "MaxClockDriftMs" => Ok(Self::MaxClockDriftMs(json_support::expect_u64(
                &payload,
                "MaxClockDriftMs",
            )?)),
            "CollectorsK" => Ok(Self::CollectorsK(json_support::expect_u16(
                &payload,
                "CollectorsK",
            )?)),
            "RedundantSendR" => Ok(Self::RedundantSendR(json_support::expect_u8(
                &payload,
                "RedundantSendR",
            )?)),
            "DaEnabled" => Ok(Self::DaEnabled(json_support::expect_bool(
                &payload,
                "DaEnabled",
            )?)),
            "NextMode" => Ok(Self::NextMode(json_support::parse_value_as::<
                SumeragiConsensusMode,
            >(&payload)?)),
            "ModeActivationHeight" => Ok(Self::ModeActivationHeight(json_support::expect_u64(
                &payload,
                "ModeActivationHeight",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SumeragiParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "block_time_ms", &self.block_time_ms);
        json_support::write_field(out, &mut first, "commit_time_ms", &self.commit_time_ms);
        json_support::write_field(out, &mut first, "min_finality_ms", &self.min_finality_ms);
        json_support::write_field(
            out,
            &mut first,
            "pacing_factor_bps",
            &self.pacing_factor_bps,
        );
        json_support::write_field(
            out,
            &mut first,
            "max_clock_drift_ms",
            &self.max_clock_drift_ms,
        );
        json_support::write_field(out, &mut first, "collectors_k", &self.collectors_k);
        json_support::write_field(
            out,
            &mut first,
            "collectors_redundant_send_r",
            &self.collectors_redundant_send_r,
        );
        json_support::write_field(out, &mut first, "da_enabled", &self.da_enabled);
        if let Some(mode) = self.next_mode {
            json_support::write_field(out, &mut first, "next_mode", &mode);
        }
        if let Some(height) = self.mode_activation_height {
            json_support::write_field(out, &mut first, "mode_activation_height", &height);
        }
        json_support::write_field(
            out,
            &mut first,
            "key_activation_lead_blocks",
            &self.key_activation_lead_blocks,
        );
        json_support::write_field(
            out,
            &mut first,
            "key_overlap_grace_blocks",
            &self.key_overlap_grace_blocks,
        );
        json_support::write_field(
            out,
            &mut first,
            "key_expiry_grace_blocks",
            &self.key_expiry_grace_blocks,
        );
        json_support::write_field(out, &mut first, "key_require_hsm", &self.key_require_hsm);
        json_support::write_field(
            out,
            &mut first,
            "key_allowed_algorithms",
            &self.key_allowed_algorithms,
        );
        json_support::write_field(
            out,
            &mut first,
            "key_allowed_hsm_providers",
            &self.key_allowed_hsm_providers,
        );
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SumeragiParameters {
    #[allow(clippy::too_many_lines)]
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "SumeragiParameters")?;

        let block_time_ms = map
            .remove("block_time_ms")
            .map(|value| json_support::expect_u64(&value, "block_time_ms"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::block_time_ms);
        let commit_time_ms = map
            .remove("commit_time_ms")
            .map(|value| json_support::expect_u64(&value, "commit_time_ms"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::commit_time_ms);
        let min_finality_ms = map
            .remove("min_finality_ms")
            .map(|value| json_support::expect_u64(&value, "min_finality_ms"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::min_finality_ms);
        let pacing_factor_bps = map
            .remove("pacing_factor_bps")
            .map(|value| json_support::expect_u32(&value, "pacing_factor_bps"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::pacing_factor_bps);
        let max_clock_drift_ms = map
            .remove("max_clock_drift_ms")
            .map(|value| json_support::expect_u64(&value, "max_clock_drift_ms"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::max_clock_drift_ms);
        let collectors_k = map
            .remove("collectors_k")
            .map(|value| json_support::expect_u16(&value, "collectors_k"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::collectors_k);
        let collectors_redundant_send_r = map
            .remove("collectors_redundant_send_r")
            .map(|value| json_support::expect_u8(&value, "collectors_redundant_send_r"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::redundant_send_r);
        let da_enabled = map
            .remove("da_enabled")
            .map(|value| json_support::expect_bool(&value, "da_enabled"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::da_enabled);
        let next_mode = map
            .remove("next_mode")
            .map(|value| json_support::parse_value_as::<SumeragiConsensusMode>(&value))
            .transpose()?;
        let mode_activation_height = map
            .remove("mode_activation_height")
            .map(|value| json_support::expect_u64(&value, "mode_activation_height"))
            .transpose()?;
        let key_activation_lead_blocks = map
            .remove("key_activation_lead_blocks")
            .map(|value| json_support::expect_u64(&value, "key_activation_lead_blocks"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_activation_lead_blocks);
        let key_overlap_grace_blocks = map
            .remove("key_overlap_grace_blocks")
            .map(|value| json_support::expect_u64(&value, "key_overlap_grace_blocks"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_overlap_grace_blocks);
        let key_expiry_grace_blocks = map
            .remove("key_expiry_grace_blocks")
            .map(|value| json_support::expect_u64(&value, "key_expiry_grace_blocks"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_expiry_grace_blocks);
        let key_require_hsm = map
            .remove("key_require_hsm")
            .map(|value| json_support::expect_bool(&value, "key_require_hsm"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_require_hsm);
        let key_allowed_algorithms = map
            .remove("key_allowed_algorithms")
            .map(|value| json_support::parse_value_as::<Vec<Algorithm>>(&value))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_allowed_algorithms);
        let key_allowed_hsm_providers = map
            .remove("key_allowed_hsm_providers")
            .map(|value| json_support::parse_value_as::<Vec<String>>(&value))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_allowed_hsm_providers);

        json_support::ensure_no_extra(map)?;

        let params = Self {
            block_time_ms,
            commit_time_ms,
            min_finality_ms,
            pacing_factor_bps,
            max_clock_drift_ms,
            collectors_k,
            collectors_redundant_send_r,
            da_enabled,
            next_mode,
            mode_activation_height,
            key_activation_lead_blocks,
            key_overlap_grace_blocks,
            key_expiry_grace_blocks,
            key_require_hsm,
            key_allowed_algorithms,
            key_allowed_hsm_providers,
        };

        params
            .validate_timing()
            .map_err(|message| json::Error::InvalidField {
                field: "SumeragiParameters".to_owned(),
                message: message.to_owned(),
            })?;

        Ok(params)
    }
}

mod defaults {
    pub mod sumeragi {
        use iroha_crypto::Algorithm;

        pub const fn min_finality_ms() -> u64 {
            100
        }
        pub const fn pacing_factor_bps() -> u32 {
            10_000
        }
        pub const fn block_time_ms() -> u64 {
            100
        }
        pub const fn commit_time_ms() -> u64 {
            100
        }
        pub const fn max_clock_drift_ms() -> u64 {
            1_000
        }
        pub const fn collectors_k() -> u16 {
            1
        }
        pub const fn redundant_send_r() -> u8 {
            3
        }
        pub const fn da_enabled() -> bool {
            true
        }
        pub const fn key_activation_lead_blocks() -> u64 {
            1
        }
        pub const fn key_overlap_grace_blocks() -> u64 {
            8
        }
        pub const fn key_expiry_grace_blocks() -> u64 {
            0
        }
        pub const fn key_require_hsm() -> bool {
            false
        }
        pub fn key_allowed_algorithms() -> Vec<Algorithm> {
            vec![Algorithm::BlsNormal]
        }
        pub fn key_allowed_hsm_providers() -> Vec<String> {
            vec!["pkcs11".into(), "yubihsm".into(), "softkey".into()]
        }

        pub mod npos {
            pub const fn k_aggregators() -> u16 {
                3
            }
            pub const fn redundant_send_r() -> u8 {
                3
            }
            pub const fn vrf_commit_window_blocks() -> u64 {
                100
            }
            pub const fn vrf_reveal_window_blocks() -> u64 {
                40
            }
            pub const fn max_validators() -> u32 {
                128
            }
            pub const fn min_self_bond() -> u64 {
                1_000
            }
            pub const fn min_nomination_bond() -> u64 {
                1
            }
            pub const fn max_nominator_concentration_pct() -> u8 {
                25
            }
            pub const fn seat_band_pct() -> u8 {
                5
            }
            pub const fn max_entity_correlation_pct() -> u8 {
                25
            }
            pub const fn finality_margin_blocks() -> u64 {
                8
            }
            pub const fn evidence_horizon_blocks() -> u64 {
                7_200
            }
            pub const fn activation_lag_blocks() -> u64 {
                1
            }
            pub const fn slashing_delay_blocks() -> u64 {
                259_200
            }
            pub const fn epoch_length_blocks() -> u64 {
                3_600
            }
        }
    }

    pub mod block {
        use core::num::NonZeroU64;

        use nonzero_ext::nonzero;

        /// Default value for [`Parameters::MaxTransactionsInBlock`]
        pub const fn max_transactions() -> NonZeroU64 {
            nonzero!(2_u64.pow(9))
        }
    }

    pub mod transaction {
        use core::num::{NonZeroU16, NonZeroU64};

        use nonzero_ext::nonzero;

        pub const fn max_signatures() -> NonZeroU64 {
            nonzero!(16_u64)
        }
        pub const fn max_instructions() -> NonZeroU64 {
            // Keep in sync with iroha_config defaults::transaction::max_instructions.
            nonzero!(50_000_u64)
        }
        pub const fn ivm_bytecode_size() -> NonZeroU64 {
            nonzero!(4 * 2_u64.pow(20))
        }
        pub const fn max_tx_bytes() -> NonZeroU64 {
            // Default total transaction wire-size cap set to 10 MiB.
            nonzero!(10_485_760_u64)
        }
        pub const fn max_decompressed_bytes() -> NonZeroU64 {
            // Default post-decompression budget aligns with max_tx_bytes until
            // additional compression formats are supported.
            max_tx_bytes()
        }
        pub const fn max_metadata_depth() -> NonZeroU16 {
            // Default metadata nesting limit: 1 (metadata map) + up to 7 nested structures.
            nonzero!(8_u16)
        }
    }

    pub mod smart_contract {
        use core::num::NonZeroU64;

        use nonzero_ext::nonzero;

        pub const fn fuel() -> NonZeroU64 {
            nonzero!(55_000_000_u64)
        }
        pub const fn memory() -> NonZeroU64 {
            nonzero!(55_000_000_u64)
        }
        pub const fn execution_depth() -> u8 {
            3
        }
    }
}

impl Default for SumeragiParameters {
    fn default() -> Self {
        use defaults::sumeragi::*;
        Self {
            block_time_ms: block_time_ms(),
            commit_time_ms: commit_time_ms(),
            min_finality_ms: min_finality_ms(),
            pacing_factor_bps: pacing_factor_bps(),
            max_clock_drift_ms: max_clock_drift_ms(),
            collectors_k: collectors_k(),
            collectors_redundant_send_r: redundant_send_r(),
            da_enabled: da_enabled(),
            next_mode: None,
            mode_activation_height: None,
            key_activation_lead_blocks: key_activation_lead_blocks(),
            key_overlap_grace_blocks: key_overlap_grace_blocks(),
            key_expiry_grace_blocks: key_expiry_grace_blocks(),
            key_require_hsm: key_require_hsm(),
            key_allowed_algorithms: key_allowed_algorithms(),
            key_allowed_hsm_providers: key_allowed_hsm_providers(),
        }
    }
}
impl Default for BlockParameters {
    fn default() -> Self {
        Self::new(defaults::block::max_transactions())
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for BlockParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "max_transactions", &self.max_transactions);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for BlockParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "BlockParameters")?;
        let max_transactions = map
            .remove("max_transactions")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_transactions"))
            .transpose()?
            .unwrap_or_else(defaults::block::max_transactions);
        json_support::ensure_no_extra(map)?;
        Ok(Self::new(max_transactions))
    }
}

impl Default for TransactionParameters {
    fn default() -> Self {
        use defaults::transaction::*;
        Self::with_max_signatures(
            max_signatures(),
            max_instructions(),
            ivm_bytecode_size(),
            max_tx_bytes(),
            max_decompressed_bytes(),
            max_metadata_depth(),
        )
    }
}

impl Default for SmartContractParameters {
    fn default() -> Self {
        use defaults::smart_contract::*;
        Self {
            fuel: fuel(),
            memory: memory(),
            execution_depth: execution_depth(),
        }
    }
}

impl FromIterator<Parameter> for Parameters {
    fn from_iter<T: IntoIterator<Item = Parameter>>(iter: T) -> Self {
        iter.into_iter().fold(Parameters::default(), |mut acc, x| {
            acc.set_parameter(x);
            acc
        })
    }
}

impl Parameters {
    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        self.sumeragi
            .parameters()
            .map(Parameter::Sumeragi)
            .chain(self.block.parameters().map(Parameter::Block))
            .chain(self.transaction.parameters().map(Parameter::Transaction))
            .chain(self.executor.parameters().map(Parameter::Executor))
            .chain(
                self.smart_contract
                    .parameters()
                    .map(Parameter::SmartContract),
            )
            .chain(self.custom.values().cloned().map(Parameter::Custom))
    }

    /// Set `parameter` value to corresponding parameter in `self`
    pub fn set_parameter(&mut self, parameter: Parameter) {
        macro_rules! apply_parameter {
            ($($container:ident($param:ident.$field:ident) => $single:ident::$variant:ident),* $(,)?) => {
                match parameter {
                    // Special scheduling fields wrap values into Option
                    Parameter::Sumeragi(SumeragiParameter::NextMode(m)) => {
                        self.sumeragi.next_mode = Some(m);
                    }
                    Parameter::Sumeragi(SumeragiParameter::ModeActivationHeight(h)) => {
                        self.sumeragi.mode_activation_height = Some(h);
                    }
                    $(
                    Parameter::$container($single::$variant(next)) => {
                        self.$param.$field = next;
                    }
                    )*
                    Parameter::Custom(next) => {
                        self.custom.insert(next.id.clone(), next);
                    }
                }
            };
        }

        apply_parameter!(
            Sumeragi(sumeragi.max_clock_drift_ms) => SumeragiParameter::MaxClockDriftMs,
            Sumeragi(sumeragi.block_time_ms) => SumeragiParameter::BlockTimeMs,
            Sumeragi(sumeragi.commit_time_ms) => SumeragiParameter::CommitTimeMs,
            Sumeragi(sumeragi.min_finality_ms) => SumeragiParameter::MinFinalityMs,
            Sumeragi(sumeragi.pacing_factor_bps) => SumeragiParameter::PacingFactorBps,
            Sumeragi(sumeragi.collectors_k) => SumeragiParameter::CollectorsK,
            Sumeragi(sumeragi.collectors_redundant_send_r) => SumeragiParameter::RedundantSendR,
            Sumeragi(sumeragi.da_enabled) => SumeragiParameter::DaEnabled,

            Block(block.max_transactions) => BlockParameter::MaxTransactions,

            Transaction(transaction.max_signatures) => TransactionParameter::MaxSignatures,
            Transaction(transaction.max_instructions) => TransactionParameter::MaxInstructions,
            Transaction(transaction.ivm_bytecode_size) => TransactionParameter::IvmBytecodeSize,
            Transaction(transaction.max_tx_bytes) => TransactionParameter::MaxTxBytes,
            Transaction(transaction.max_decompressed_bytes) => TransactionParameter::MaxDecompressedBytes,
            Transaction(transaction.max_metadata_depth) => TransactionParameter::MaxMetadataDepth,
            Transaction(transaction.require_height_ttl) => TransactionParameter::RequireHeightTtl,
            Transaction(transaction.require_sequence) => TransactionParameter::RequireSequence,

            SmartContract(smart_contract.fuel) => SmartContractParameter::Fuel,
            SmartContract(smart_contract.memory) => SmartContractParameter::Memory,
            SmartContract(smart_contract.execution_depth) => SmartContractParameter::ExecutionDepth,

            Executor(executor.fuel) => SmartContractParameter::Fuel,
            Executor(executor.memory) => SmartContractParameter::Memory,
            Executor(executor.execution_depth) => SmartContractParameter::ExecutionDepth,
        );
    }
}

/// Consensus handshake metadata helpers used during genesis provisioning.
pub mod consensus_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static HANDSHAKE_META_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("consensus_handshake_meta")
                .expect("consensus_handshake_meta is a valid Name"),
        )
    });

    /// Identifier of the custom parameter embedding consensus handshake metadata.
    pub fn handshake_meta_id() -> CustomParameterId {
        HANDSHAKE_META_ID.clone()
    }
}

/// Cryptography snapshot metadata helpers used during genesis provisioning.
pub mod crypto_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static MANIFEST_META_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("crypto_manifest_meta").expect("crypto_manifest_meta is a valid Name"),
        )
    });

    /// Identifier of the custom parameter anchoring the genesis crypto snapshot.
    pub fn manifest_meta_id() -> CustomParameterId {
        MANIFEST_META_ID.clone()
    }
}

/// Confidential registry metadata helpers used during genesis provisioning.
pub mod confidential_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static REGISTRY_ROOT_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("confidential_registry_root")
                .expect("confidential_registry_root is a valid Name"),
        )
    });

    /// Identifier of the custom parameter anchoring the confidential registry fingerprint.
    pub fn registry_root_id() -> CustomParameterId {
        REGISTRY_ROOT_ID.clone()
    }
}

/// IVM metadata helpers stored in the custom parameter registry.
pub mod ivm_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static PUBLIC_INPUTS_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("ivm_public_inputs").expect("ivm_public_inputs is a valid Name"),
        )
    });

    /// Identifier of the custom parameter embedding the IVM public input registry.
    pub fn public_inputs_id() -> CustomParameterId {
        PUBLIC_INPUTS_ID.clone()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for Parameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "sumeragi", &self.sumeragi);
        json_support::write_field(out, &mut first, "block", &self.block);
        json_support::write_field(out, &mut first, "transaction", &self.transaction);
        json_support::write_field(out, &mut first, "executor", &self.executor);
        json_support::write_field(out, &mut first, "smart_contract", &self.smart_contract);
        if !self.custom.is_empty() {
            json_support::write_field(out, &mut first, "custom", &self.custom);
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Parameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "Parameters")?;
        let sumeragi = map
            .remove("sumeragi")
            .map(|value| json_support::parse_value_as::<SumeragiParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let block = map
            .remove("block")
            .map(|value| json_support::parse_value_as::<BlockParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let transaction = map
            .remove("transaction")
            .map(|value| json_support::parse_value_as::<TransactionParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let executor = map
            .remove("executor")
            .map(|value| json_support::parse_value_as::<SmartContractParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let smart_contract = map
            .remove("smart_contract")
            .map(|value| json_support::parse_value_as::<SmartContractParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let custom = map
            .remove("custom")
            .map(parse_custom_parameters_value)
            .transpose()?
            .unwrap_or_default();
        json_support::ensure_no_extra(map)?;
        Ok(Self {
            sumeragi,
            block,
            transaction,
            executor,
            smart_contract,
            custom,
        })
    }
}

impl SumeragiParameters {
    /// Construct [`Self`]
    pub fn new(block_time: Duration, commit_time: Duration, max_clock_drift: Duration) -> Self {
        let block_time_ms = block_time
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Time should fit into u64");
        Self {
            block_time_ms,
            commit_time_ms: commit_time
                .as_millis()
                .try_into()
                .expect("INTERNAL BUG: Time should fit into u64"),
            min_finality_ms: block_time_ms,
            pacing_factor_bps: defaults::sumeragi::pacing_factor_bps(),
            max_clock_drift_ms: max_clock_drift
                .as_millis()
                .try_into()
                .expect("INTERNAL BUG: Time should fit into u64"),
            collectors_k: defaults::sumeragi::collectors_k(),
            collectors_redundant_send_r: defaults::sumeragi::redundant_send_r(),
            da_enabled: defaults::sumeragi::da_enabled(),
            next_mode: None,
            mode_activation_height: None,
            key_activation_lead_blocks: defaults::sumeragi::key_activation_lead_blocks(),
            key_overlap_grace_blocks: defaults::sumeragi::key_overlap_grace_blocks(),
            key_expiry_grace_blocks: defaults::sumeragi::key_expiry_grace_blocks(),
            key_require_hsm: defaults::sumeragi::key_require_hsm(),
            key_allowed_algorithms: defaults::sumeragi::key_allowed_algorithms(),
            key_allowed_hsm_providers: defaults::sumeragi::key_allowed_hsm_providers(),
        }
    }

    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = SumeragiParameter> {
        [
            SumeragiParameter::BlockTimeMs(self.block_time_ms),
            SumeragiParameter::CommitTimeMs(self.commit_time_ms),
            SumeragiParameter::MinFinalityMs(self.min_finality_ms),
            SumeragiParameter::PacingFactorBps(self.pacing_factor_bps),
            SumeragiParameter::MaxClockDriftMs(self.max_clock_drift_ms),
            SumeragiParameter::CollectorsK(self.collectors_k),
            SumeragiParameter::RedundantSendR(self.collectors_redundant_send_r),
            SumeragiParameter::DaEnabled(self.da_enabled),
            // Scheduling parameters are not emitted here because they are optional and do not
            // reflect steady-state runtime behavior; they are set via explicit SetParameter.
        ]
        .into_iter()
    }
}

impl BlockParameters {
    /// Construct [`Self`]
    pub const fn new(max_transactions: NonZeroU64) -> Self {
        Self { max_transactions }
    }

    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = BlockParameter> {
        [BlockParameter::MaxTransactions(self.max_transactions)].into_iter()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for BlockParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            BlockParameter::MaxTransactions(value) => {
                json::write_json_string("MaxTransactions", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for BlockParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "BlockParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("BlockParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "MaxTransactions" => Ok(Self::MaxTransactions(json_support::expect_nonzero_u64(
                &payload,
                "MaxTransactions",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

impl TransactionParameters {
    /// Construct [`Self`] with an explicit maximum signature count.
    pub const fn with_max_signatures(
        max_signatures: NonZeroU64,
        max_instructions: NonZeroU64,
        ivm_bytecode_size: NonZeroU64,
        max_tx_bytes: NonZeroU64,
        max_decompressed_bytes: NonZeroU64,
        max_metadata_depth: NonZeroU16,
    ) -> Self {
        Self {
            max_signatures,
            max_instructions,
            ivm_bytecode_size,
            max_tx_bytes,
            max_decompressed_bytes,
            max_metadata_depth,
            require_height_ttl: false,
            require_sequence: false,
        }
    }

    /// Configure ingress metadata enforcement (height-based TTL and per-sender sequence checks).
    #[must_use]
    pub const fn with_ingress_enforcement(
        mut self,
        require_height_ttl: bool,
        require_sequence: bool,
    ) -> Self {
        self.require_height_ttl = require_height_ttl;
        self.require_sequence = require_sequence;
        self
    }

    /// Construct [`Self`] using the default signature limit.
    pub const fn new(
        max_instructions: NonZeroU64,
        ivm_bytecode_size: NonZeroU64,
        max_tx_bytes: NonZeroU64,
        max_decompressed_bytes: NonZeroU64,
        max_metadata_depth: NonZeroU16,
    ) -> Self {
        Self::with_max_signatures(
            defaults::transaction::max_signatures(),
            max_instructions,
            ivm_bytecode_size,
            max_tx_bytes,
            max_decompressed_bytes,
            max_metadata_depth,
        )
    }

    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = TransactionParameter> {
        [
            TransactionParameter::MaxSignatures(self.max_signatures),
            TransactionParameter::MaxInstructions(self.max_instructions),
            TransactionParameter::IvmBytecodeSize(self.ivm_bytecode_size),
            TransactionParameter::MaxTxBytes(self.max_tx_bytes),
            TransactionParameter::MaxDecompressedBytes(self.max_decompressed_bytes),
            TransactionParameter::MaxMetadataDepth(self.max_metadata_depth),
            TransactionParameter::RequireHeightTtl(self.require_height_ttl),
            TransactionParameter::RequireSequence(self.require_sequence),
        ]
        .into_iter()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for TransactionParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "max_signatures", &self.max_signatures);
        json_support::write_field(out, &mut first, "max_instructions", &self.max_instructions);
        json_support::write_field(
            out,
            &mut first,
            "ivm_bytecode_size",
            &self.ivm_bytecode_size,
        );
        json_support::write_field(out, &mut first, "max_tx_bytes", &self.max_tx_bytes);
        json_support::write_field(
            out,
            &mut first,
            "max_decompressed_bytes",
            &self.max_decompressed_bytes,
        );
        json_support::write_field(
            out,
            &mut first,
            "max_metadata_depth",
            &self.max_metadata_depth,
        );
        json_support::write_field(
            out,
            &mut first,
            "require_height_ttl",
            &self.require_height_ttl,
        );
        json_support::write_field(out, &mut first, "require_sequence", &self.require_sequence);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for TransactionParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "TransactionParameters")?;
        let max_signatures = map
            .remove("max_signatures")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_signatures"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_signatures);
        let max_instructions = map
            .remove("max_instructions")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_instructions"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_instructions);
        let ivm_bytecode_size = map
            .remove("ivm_bytecode_size")
            .map(|value| json_support::expect_nonzero_u64(&value, "ivm_bytecode_size"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::ivm_bytecode_size);
        let max_tx_bytes = map
            .remove("max_tx_bytes")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_tx_bytes"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_tx_bytes);
        let max_decompressed_bytes = map
            .remove("max_decompressed_bytes")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_decompressed_bytes"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_decompressed_bytes);
        let require_height_ttl = map
            .remove("require_height_ttl")
            .map(|value| json_support::expect_bool(&value, "require_height_ttl"))
            .transpose()?
            .unwrap_or(false);
        let require_sequence = map
            .remove("require_sequence")
            .map(|value| json_support::expect_bool(&value, "require_sequence"))
            .transpose()?
            .unwrap_or(false);
        let max_metadata_depth = map
            .remove("max_metadata_depth")
            .map(|value| {
                let depth = json_support::expect_nonzero_u64(&value, "max_metadata_depth")?;
                let depth_u16 =
                    u16::try_from(depth.get()).map_err(|_| json::Error::InvalidField {
                        field: String::from("max_metadata_depth"),
                        message: String::from("value exceeds u16::MAX"),
                    })?;
                NonZeroU16::new(depth_u16).ok_or_else(|| json::Error::InvalidField {
                    field: String::from("max_metadata_depth"),
                    message: String::from("value must be non-zero"),
                })
            })
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_metadata_depth);
        json_support::ensure_no_extra(map)?;
        Ok(Self::with_max_signatures(
            max_signatures,
            max_instructions,
            ivm_bytecode_size,
            max_tx_bytes,
            max_decompressed_bytes,
            max_metadata_depth,
        )
        .with_ingress_enforcement(require_height_ttl, require_sequence))
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for TransactionParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            TransactionParameter::MaxSignatures(value) => {
                json::write_json_string("MaxSignatures", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxInstructions(value) => {
                json::write_json_string("MaxInstructions", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::IvmBytecodeSize(value) => {
                json::write_json_string("IvmBytecodeSize", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxTxBytes(value) => {
                json::write_json_string("MaxTxBytes", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxDecompressedBytes(value) => {
                json::write_json_string("MaxDecompressedBytes", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxMetadataDepth(value) => {
                json::write_json_string("MaxMetadataDepth", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::RequireHeightTtl(value) => {
                json::write_json_string("RequireHeightTtl", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::RequireSequence(value) => {
                json::write_json_string("RequireSequence", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for TransactionParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "TransactionParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("TransactionParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "MaxSignatures" => Ok(Self::MaxSignatures(json_support::expect_nonzero_u64(
                &payload,
                "MaxSignatures",
            )?)),
            "MaxInstructions" => Ok(Self::MaxInstructions(json_support::expect_nonzero_u64(
                &payload,
                "MaxInstructions",
            )?)),
            "IvmBytecodeSize" => Ok(Self::IvmBytecodeSize(json_support::expect_nonzero_u64(
                &payload,
                "IvmBytecodeSize",
            )?)),
            "MaxTxBytes" => Ok(Self::MaxTxBytes(json_support::expect_nonzero_u64(
                &payload,
                "MaxTxBytes",
            )?)),
            "MaxDecompressedBytes" => Ok(Self::MaxDecompressedBytes(
                json_support::expect_nonzero_u64(&payload, "MaxDecompressedBytes")?,
            )),
            "MaxMetadataDepth" => Ok(Self::MaxMetadataDepth({
                let depth = json_support::expect_nonzero_u64(&payload, "MaxMetadataDepth")?;
                let value = u16::try_from(depth.get()).map_err(|_| json::Error::InvalidField {
                    field: String::from("MaxMetadataDepth"),
                    message: String::from("value exceeds u16::MAX"),
                })?;
                NonZeroU16::new(value).ok_or_else(|| json::Error::InvalidField {
                    field: String::from("MaxMetadataDepth"),
                    message: String::from("value must be non-zero"),
                })?
            })),
            "RequireHeightTtl" => Ok(Self::RequireHeightTtl(json_support::expect_bool(
                &payload,
                "RequireHeightTtl",
            )?)),
            "RequireSequence" => Ok(Self::RequireSequence(json_support::expect_bool(
                &payload,
                "RequireSequence",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

impl SmartContractParameters {
    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = SmartContractParameter> {
        [
            SmartContractParameter::Fuel(self.fuel),
            SmartContractParameter::Memory(self.memory),
            SmartContractParameter::ExecutionDepth(self.execution_depth),
        ]
        .into_iter()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SmartContractParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "fuel", &self.fuel);
        json_support::write_field(out, &mut first, "memory", &self.memory);
        json_support::write_field(out, &mut first, "execution_depth", &self.execution_depth);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SmartContractParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "SmartContractParameters")?;
        let fuel = map
            .remove("fuel")
            .map(|value| json_support::expect_nonzero_u64(&value, "fuel"))
            .transpose()?
            .unwrap_or_else(defaults::smart_contract::fuel);
        let memory = map
            .remove("memory")
            .map(|value| json_support::expect_nonzero_u64(&value, "memory"))
            .transpose()?
            .unwrap_or_else(defaults::smart_contract::memory);
        let execution_depth = map
            .remove("execution_depth")
            .map(|value| json_support::expect_u8(&value, "execution_depth"))
            .transpose()?
            .unwrap_or_else(defaults::smart_contract::execution_depth);
        json_support::ensure_no_extra(map)?;
        Ok(Self {
            fuel,
            memory,
            execution_depth,
        })
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SmartContractParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            SmartContractParameter::Fuel(value) => {
                json::write_json_string("Fuel", out);
                out.push(':');
                value.json_serialize(out);
            }
            SmartContractParameter::Memory(value) => {
                json::write_json_string("Memory", out);
                out.push(':');
                value.json_serialize(out);
            }
            SmartContractParameter::ExecutionDepth(value) => {
                json::write_json_string("ExecutionDepth", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SmartContractParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "SmartContractParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("SmartContractParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "Fuel" => Ok(Self::Fuel(json_support::expect_nonzero_u64(
                &payload, "Fuel",
            )?)),
            "Memory" => Ok(Self::Memory(json_support::expect_nonzero_u64(
                &payload, "Memory",
            )?)),
            "ExecutionDepth" => Ok(Self::ExecutionDepth(json_support::expect_u8(
                &payload,
                "ExecutionDepth",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

// Norito decoding now relies on derived implementations for parameter types.

#[cfg(test)]
mod tests {
    use core::str::FromStr as _;

    use iroha_primitives::json::Json;
    // Norito core helpers for header-framed encode/decode in tests
    use norito::codec::{DecodeAll as _, Encode as _};
    use norito::core as norito_core;
    use norito::json::{Number, Value};

    use super::*;
    use crate::{
        name::Name,
        parameter::custom::{CustomParameter, CustomParameterId},
    };

    #[test]
    fn set_custom_parameter() {
        let mut params = Parameters::default();
        let id = CustomParameterId::new(Name::from_str("test").expect("valid"));
        let payload = Json::new(1u32);
        let custom = CustomParameter::new(id.clone(), payload);
        params.set_parameter(Parameter::Custom(custom.clone()));
        assert_eq!(params.custom().get(&id), Some(&custom));
    }

    #[test]
    fn set_block_parameter() {
        let mut params = Parameters::default();
        let max_transactions = NonZeroU64::new(10).unwrap();
        params.set_parameter(Parameter::Block(BlockParameter::MaxTransactions(
            max_transactions,
        )));
        assert_eq!(params.block().max_transactions(), max_transactions);
    }

    #[test]
    fn expect_u32_accepts_valid_range() {
        let value = Value::Number(Number::from(u64::from(u32::MAX)));
        let parsed = super::json_support::expect_u32(&value, "value")
            .expect("u32 should parse from JSON number");
        assert_eq!(parsed, u32::MAX);
    }

    #[test]
    fn expect_u32_rejects_out_of_range() {
        let value = Value::Number(Number::from(u64::from(u32::MAX) + 1));
        let err = super::json_support::expect_u32(&value, "value")
            .expect_err("out-of-range u32 should fail");
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn sumeragi_parameter_norito_roundtrip() {
        // Cover each variant for stability
        let vals = [
            SumeragiParameter::BlockTimeMs(2000),
            SumeragiParameter::CommitTimeMs(4000),
            SumeragiParameter::MinFinalityMs(100),
            SumeragiParameter::PacingFactorBps(12_000),
            SumeragiParameter::MaxClockDriftMs(1000),
            SumeragiParameter::CollectorsK(2),
            SumeragiParameter::RedundantSendR(2),
            SumeragiParameter::DaEnabled(true),
        ];
        for v in vals {
            // Encode as a stable (tag, value) tuple and reconstruct
            let tag_val: (u8, u64) = match v {
                SumeragiParameter::BlockTimeMs(x) => (0, x),
                SumeragiParameter::CommitTimeMs(x) => (1, x),
                SumeragiParameter::MaxClockDriftMs(x) => (2, x),
                SumeragiParameter::CollectorsK(x) => (3, u64::from(x)),
                SumeragiParameter::RedundantSendR(x) => (4, u64::from(x)),
                SumeragiParameter::DaEnabled(x) => (5, u64::from(x)),
                SumeragiParameter::MinFinalityMs(x) => (6, x),
                SumeragiParameter::PacingFactorBps(x) => (7, u64::from(x)),
                // Scheduling parameters are intentionally excluded from this stability test
                // since they are optional and not part of steady-state runtime behavior.
                // These arms satisfy exhaustiveness; they won’t be reached because
                // `vals` above does not include these variants.
                SumeragiParameter::NextMode(_) => {
                    unreachable!("NextMode is not exercised in this stability test")
                }
                SumeragiParameter::ModeActivationHeight(_) => {
                    unreachable!("ModeActivationHeight is not exercised in this stability test")
                }
            };
            let bytes = norito_core::to_bytes(&tag_val).expect("encode");
            let archived = norito_core::from_bytes::<(u8, u64)>(&bytes).expect("archived");
            let (tag, val) = <(u8, u64) as norito_core::NoritoDeserialize>::deserialize(archived);
            let dec = match tag {
                0 => SumeragiParameter::BlockTimeMs(val),
                1 => SumeragiParameter::CommitTimeMs(val),
                2 => SumeragiParameter::MaxClockDriftMs(val),
                3 => SumeragiParameter::CollectorsK(u16::try_from(val).expect("fits in u16")),
                4 => SumeragiParameter::RedundantSendR(u8::try_from(val).expect("fits in u8")),
                5 => SumeragiParameter::DaEnabled(val != 0),
                6 => SumeragiParameter::MinFinalityMs(val),
                7 => SumeragiParameter::PacingFactorBps(u32::try_from(val).expect("fits in u32")),
                _ => unreachable!(),
            };
            assert_eq!(v, dec);
        }
    }

    #[test]

    fn sumeragi_parameters_norito_roundtrip() {
        let params = SumeragiParameters::default();
        // Encode as a stable tuple and reconstruct (append-only order)
        let bytes = params.encode();
        let mut cursor = bytes.as_slice();
        let dec = SumeragiParameters::decode_all(&mut cursor).expect("decode sumeragi parameters");
        assert_eq!(params, dec);

        // Skip bare codec path; headerful Norito is the canonical test path

        // JSON path: ensure defaults parse and roundtrip
        #[cfg(feature = "json")]
        {
            let j = norito::json::to_json(&params).expect("json ser");
            let parsed: SumeragiParameters = norito::json::from_str(&j).expect("json de");
            assert_eq!(params, parsed);
        }
    }

    #[test]
    fn sumeragi_parameters_effective_timing_clamps_to_min_finality() {
        let params = SumeragiParameters {
            block_time_ms: 50,
            commit_time_ms: 40,
            min_finality_ms: 100,
            ..SumeragiParameters::default()
        };
        assert_eq!(params.effective_min_finality_ms(), 100);
        assert_eq!(params.effective_block_time_ms(), 100);
        assert_eq!(params.effective_commit_time_ms(), 100);
    }

    #[test]
    fn sumeragi_parameters_effective_timing_applies_pacing_factor() {
        let params = SumeragiParameters {
            block_time_ms: 200,
            commit_time_ms: 400,
            min_finality_ms: 100,
            pacing_factor_bps: 12_500,
            ..SumeragiParameters::default()
        };
        assert_eq!(params.effective_pacing_factor_bps(), 12_500);
        assert_eq!(params.effective_block_time_ms(), 250);
        assert_eq!(params.effective_commit_time_ms(), 500);
    }

    #[cfg(feature = "json")]
    #[test]
    fn sumeragi_parameters_json_rejects_invalid_timing() {
        let json = r#"{"block_time_ms":200,"commit_time_ms":100,"min_finality_ms":50}"#;
        let err = norito::json::from_str::<SumeragiParameters>(json)
            .expect_err("invalid timing should fail");
        match err {
            norito::json::Error::InvalidField { field, .. } => {
                assert_eq!(field, "SumeragiParameters");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn sumeragi_parameters_json_rejects_low_pacing_factor() {
        let json = r#"{"block_time_ms":200,"commit_time_ms":200,"min_finality_ms":100,"pacing_factor_bps":9000}"#;
        let err = norito::json::from_str::<SumeragiParameters>(json)
            .expect_err("low pacing factor should fail");
        match err {
            norito::json::Error::InvalidField { field, .. } => {
                assert_eq!(field, "SumeragiParameters");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_support_expect_u32_handles_range() {
        let ok = json::Value::from(12u32);
        assert_eq!(
            json_support::expect_u32(&ok, "pacing_factor_bps").expect("valid u32"),
            12
        );

        let overflow = json::Value::from(u64::from(u32::MAX) + 1);
        let err = json_support::expect_u32(&overflow, "pacing_factor_bps")
            .expect_err("out of range should fail");
        match err {
            norito::json::Error::InvalidField { field, .. } => {
                assert_eq!(field, "pacing_factor_bps");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_string_wrapped_payload() {
        let expected = SumeragiNposParameters::default();
        let wrapped = Json::new(norito::json::to_json(&expected).expect("serialize npos payload"));
        let custom = CustomParameter::new(SumeragiNposParameters::parameter_id(), wrapped);

        let decoded = SumeragiNposParameters::from_custom_parameter(&custom)
            .expect("decode string-wrapped npos payload");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_numeric_string_payload() {
        let expected = SumeragiNposParameters::default();
        let mut payload =
            norito::json::to_value(&expected).expect("serialize npos payload to json value");
        let map = payload
            .as_object_mut()
            .expect("npos payload should serialize as object");
        for field in [
            "k_aggregators",
            "redundant_send_r",
            "vrf_commit_window_blocks",
            "vrf_reveal_window_blocks",
            "max_validators",
            "min_self_bond",
            "min_nomination_bond",
            "max_nominator_concentration_pct",
            "seat_band_pct",
            "max_entity_correlation_pct",
            "finality_margin_blocks",
            "evidence_horizon_blocks",
            "activation_lag_blocks",
            "slashing_delay_blocks",
            "epoch_length_blocks",
        ] {
            let value = map
                .get_mut(field)
                .unwrap_or_else(|| panic!("missing `{field}` in npos payload"));
            let number = value
                .as_u64()
                .unwrap_or_else(|| panic!("`{field}` should serialize as number"));
            *value = norito::json::Value::String(number.to_string());
        }
        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_norito_value_ref(&payload).expect("serialize compatibility payload"),
        );

        let decoded = SumeragiNposParameters::from_custom_parameter(&custom)
            .expect("decode numeric-string npos payload");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_epoch_seed_hex_string_payload() {
        let expected = SumeragiNposParameters::default();
        let mut payload =
            norito::json::to_value(&expected).expect("serialize npos payload to json value");
        let map = payload
            .as_object_mut()
            .expect("npos payload should serialize as object");
        let mut hex_seed = String::with_capacity(64);
        for byte in expected.epoch_seed() {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            hex_seed.push(HEX[(byte >> 4) as usize] as char);
            hex_seed.push(HEX[(byte & 0x0F) as usize] as char);
        }
        map.insert(
            "epoch_seed".to_owned(),
            norito::json::Value::String(hex_seed),
        );

        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_norito_value_ref(&payload).expect("serialize compatibility payload"),
        );
        let decoded = SumeragiNposParameters::from_custom_parameter(&custom)
            .expect("decode hex epoch seed payload");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_epoch_seed_with_nested_quotes() {
        let expected = SumeragiNposParameters::default();
        let mut hex_seed = String::with_capacity(64);
        for byte in expected.epoch_seed() {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            hex_seed.push(HEX[(byte >> 4) as usize] as char);
            hex_seed.push(HEX[(byte & 0x0F) as usize] as char);
        }
        let mut payload =
            norito::json::to_value(&expected).expect("serialize npos payload to json value");
        let map = payload
            .as_object_mut()
            .expect("npos payload should serialize as object");
        map.insert(
            "epoch_seed".to_owned(),
            norito::json::Value::String(format!("\"{hex_seed}\"")),
        );

        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_norito_value_ref(&payload).expect("serialize compatibility payload"),
        );
        let decoded = SumeragiNposParameters::from_custom_parameter(&custom)
            .expect("decode nested-quoted epoch seed payload");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_real_chain_payload() {
        let payload = r#"{"epoch_seed":"0000000000000000000000000000000000000000000000000000000000000000","k_aggregators":3,"redundant_send_r":3,"vrf_commit_window_blocks":100,"vrf_reveal_window_blocks":40,"max_validators":128,"min_self_bond":1000,"min_nomination_bond":1,"max_nominator_concentration_pct":25,"seat_band_pct":5,"max_entity_correlation_pct":25,"finality_margin_blocks":8,"evidence_horizon_blocks":7200,"activation_lag_blocks":1,"slashing_delay_blocks":259200,"epoch_length_blocks":3600}"#;
        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            payload
                .parse::<Json>()
                .expect("fixture payload should be valid json"),
        );
        assert!(
            SumeragiNposParameters::from_custom_parameter(&custom).is_some(),
            "real chain payload should decode"
        );
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_trailing_comma_payload() {
        let payload = r#"{"epoch_seed":"0000000000000000000000000000000000000000000000000000000000000000","k_aggregators":3,"redundant_send_r":3,"vrf_commit_window_blocks":100,"vrf_reveal_window_blocks":40,"max_validators":128,"min_self_bond":1,"min_nomination_bond":1,"max_nominator_concentration_pct":25,"seat_band_pct":100,"max_entity_correlation_pct":25,"finality_margin_blocks":8,"evidence_horizon_blocks":7200,"activation_lag_blocks":1,"slashing_delay_blocks":259200,"epoch_length_blocks":3600,}"#;
        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_string_unchecked(payload.to_owned()),
        );
        let parsed = SumeragiNposParameters::from_custom_parameter(&custom)
            .expect("decode payload with trailing comma");
        assert_eq!(parsed.k_aggregators(), 3);
        assert_eq!(parsed.epoch_length_blocks(), 3600);
    }
}
