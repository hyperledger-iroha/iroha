//! Shared constant-rate profile catalogue used by the relay runtime and tooling.

use std::{fmt, str::FromStr};

use norito::json::{self, Error as JsonError, JsonDeserialize, JsonSerialize, Parser};

/// Payload bytes carried by each constant-rate cell.
pub const CONSTANT_RATE_CELL_BYTES: u32 = 1_024;

/// Specification describing a single profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstantRateProfileSpec {
    /// Canonical short name (e.g., `core`, `home`).
    pub name: &'static str,
    /// Operator-facing description of the preset.
    pub description: &'static str,
    /// Tick duration in milliseconds.
    pub tick_millis: f64,
    /// Maximum simultaneous lanes.
    pub lane_cap: u16,
    /// Minimum dummy lanes kept alive even when idle.
    pub dummy_lane_floor: u16,
    /// Fraction of the uplink that may be consumed by constant-rate payloads.
    pub ceiling_fraction: f64,
    /// Recommended uplink budget in megabits per second.
    pub recommended_uplink_mbps: f64,
    /// Maximum constant-rate neighbors permitted.
    pub neighbor_cap: u16,
    /// Saturation percentage that should trigger auto-disable of excess lanes.
    pub auto_disable_threshold_percent: f64,
    /// Saturation percentage that should allow auto re-enabling of full capacity.
    pub auto_reenable_threshold_percent: f64,
}

const CORE_PROFILE: ConstantRateProfileSpec = ConstantRateProfileSpec {
    name: "core",
    description: "Data-centre grade nodes or operators with ≥30 Mbps uplink; keep PQ guard coverage fully constant-rate.",
    tick_millis: 5.0,
    lane_cap: 12,
    dummy_lane_floor: 4,
    ceiling_fraction: 0.65,
    recommended_uplink_mbps: 30.0,
    neighbor_cap: 8,
    auto_disable_threshold_percent: 85.0,
    auto_reenable_threshold_percent: 75.0,
};

const HOME_PROFILE: ConstantRateProfileSpec = ConstantRateProfileSpec {
    name: "home",
    description: "Residential or low-uplink operators that still need cover links for privacy-critical circuits.",
    tick_millis: 10.0,
    lane_cap: 4,
    dummy_lane_floor: 2,
    ceiling_fraction: 0.40,
    recommended_uplink_mbps: 10.0,
    neighbor_cap: 2,
    auto_disable_threshold_percent: 70.0,
    auto_reenable_threshold_percent: 60.0,
};

const NULL_PROFILE: ConstantRateProfileSpec = ConstantRateProfileSpec {
    name: "null",
    description: "Dogfood preset that exercises the constant-rate envelope with relaxed duty-cycle limits.",
    tick_millis: 20.0,
    lane_cap: 2,
    dummy_lane_floor: 1,
    ceiling_fraction: 0.15,
    recommended_uplink_mbps: 5.0,
    neighbor_cap: 1,
    auto_disable_threshold_percent: 55.0,
    auto_reenable_threshold_percent: 45.0,
};

const ALL_PROFILES: [ConstantRateProfileSpec; 3] = [CORE_PROFILE, HOME_PROFILE, NULL_PROFILE];

/// Named preset stored in configuration files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConstantRateProfileName {
    #[default]
    Core,
    Home,
    Null,
}

impl ConstantRateProfileName {
    /// Returns the canonical short label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Home => "home",
            Self::Null => "null",
        }
    }

    /// Retrieves the immutable specification for this profile.
    pub fn spec(self) -> ConstantRateProfileSpec {
        match self {
            Self::Core => CORE_PROFILE,
            Self::Home => HOME_PROFILE,
            Self::Null => NULL_PROFILE,
        }
    }

    fn parse_str(value: &str) -> Result<Self, JsonError> {
        match value {
            "core" => Ok(Self::Core),
            "home" => Ok(Self::Home),
            "null" => Ok(Self::Null),
            other => Err(JsonError::Message(format!(
                "unknown constant-rate profile `{other}`"
            ))),
        }
    }
}

impl fmt::Display for ConstantRateProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ConstantRateProfileName {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_str(value).map_err(|err| err.to_string())
    }
}

/// Returns the full list of supported presets.
pub fn all_profiles() -> &'static [ConstantRateProfileSpec] {
    &ALL_PROFILES
}

/// Finds a preset by name (case-insensitive).
pub fn profile_by_name(name: &str) -> Option<ConstantRateProfileSpec> {
    ALL_PROFILES
        .iter()
        .copied()
        .find(|spec| name.eq_ignore_ascii_case(spec.name))
}

impl JsonSerialize for ConstantRateProfileName {
    fn json_serialize(&self, out: &mut String) {
        out.push('"');
        out.push_str(self.as_str());
        out.push('"');
    }
}

impl JsonDeserialize for ConstantRateProfileName {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, JsonError> {
        let value = p.parse_string()?;
        Self::parse_str(&value)
    }

    fn json_from_value(value: &json::Value) -> Result<Self, JsonError> {
        value
            .as_str()
            .ok_or_else(|| JsonError::Message("expected string".into()))
            .and_then(Self::parse_str)
    }

    fn json_from_map_key(key: &str) -> Result<Self, JsonError> {
        Self::parse_str(key)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn null_profile_is_registered_and_parses() {
        let names: Vec<_> = all_profiles().iter().map(|spec| spec.name).collect();
        assert!(
            names.contains(&"null"),
            "null profile must be listed in the catalog"
        );

        let spec = profile_by_name("null").expect("null profile registered");
        assert_eq!(spec.tick_millis, 20.0);
        assert_eq!(spec.neighbor_cap, 1);

        let parsed = ConstantRateProfileName::from_str("null").expect("parse null");
        assert_eq!(parsed, ConstantRateProfileName::Null);
        assert_eq!(parsed.as_str(), "null");
    }
}
