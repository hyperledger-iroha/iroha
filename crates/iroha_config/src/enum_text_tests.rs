//! Exact configuration spellings, diagnostic text and JSON representations.

use std::{fmt::Debug, fmt::Display, mem::discriminant, str::FromStr};

use iroha_config_base::enum_text::ParseError;
use norito::json::{JsonDeserialize, JsonSerialize};

use crate::{
    kura::{FsyncMode, InitMode},
    logger::Format,
    parameters::user::{
        FastpqExecutionMode, FastpqPoseidonMode, NodeRole, NtsEnforcementMode, QueryCursorMode,
        TelemetryProfile,
    },
    snapshot::Mode,
};

fn assert_spellings<T>(cases: &[(T, &str)], as_str: fn(&T) -> &'static str)
where
    T: Debug + Display + FromStr<Err = ParseError> + JsonSerialize + JsonDeserialize,
    for<'a> T: TryFrom<&'a str, Error = ParseError>,
{
    for (variant, spelling) in cases {
        assert_eq!(as_str(variant), *spelling);
        assert_eq!(variant.to_string(), *spelling);
        assert_eq!(format!("{variant:*>16.4}"), format!("{spelling:*>16.4}"));
        assert_eq!(
            discriminant(&spelling.parse::<T>().unwrap()),
            discriminant(variant)
        );
        assert_eq!(
            discriminant(&T::try_from(*spelling).unwrap()),
            discriminant(variant)
        );
        let json = norito::json::to_json(variant).unwrap();
        assert_eq!(json, norito::json::to_json(spelling).unwrap());
        assert_eq!(
            discriminant(&norito::json::from_json::<T>(&json).unwrap()),
            discriminant(variant)
        );
        for invalid in [
            String::new(),
            "unknown".to_owned(),
            spelling.to_uppercase(),
            format!("{variant:?}"),
            format!(" {spelling}"),
            format!("{spelling} "),
            format!("{spelling}\0"),
        ] {
            let error = invalid.parse::<T>().unwrap_err();
            assert_eq!(error, ParseError::VariantNotFound, "{invalid:?}");
            assert_eq!(error.to_string(), "Matching variant not found");
            assert_eq!(format!("{error:?}"), "VariantNotFound");
            assert_eq!(T::try_from(invalid.as_str()).unwrap_err(), error);
            let json = norito::json::to_json(&invalid).unwrap();
            assert!(
                norito::json::from_json::<T>(&json)
                    .unwrap_err()
                    .to_string()
                    .contains("Matching variant not found")
            );
        }
    }
}

#[test]
fn configuration_enums_preserve_every_exact_spelling() {
    assert_spellings(
        &[(InitMode::Strict, "strict"), (InitMode::Fast, "fast")],
        InitMode::as_str,
    );
    assert_spellings(
        &[
            (FsyncMode::Always, "always"),
            (FsyncMode::Batched, "batched"),
        ],
        FsyncMode::as_str,
    );
    assert_spellings(
        &[
            (Format::Full, "full"),
            (Format::Compact, "compact"),
            (Format::Pretty, "pretty"),
            (Format::Json, "json"),
        ],
        Format::as_str,
    );
    assert_spellings(
        &[
            (Mode::ReadWrite, "read_write"),
            (Mode::Readonly, "readonly"),
            (Mode::Disabled, "disabled"),
        ],
        Mode::as_str,
    );
    for alias in ["readwrite", "read-write", "read_only", "read-only"] {
        assert!(alias.parse::<Mode>().is_err());
    }
    assert_spellings(
        &[
            (TelemetryProfile::Disabled, "disabled"),
            (TelemetryProfile::Operator, "operator"),
            (TelemetryProfile::Extended, "extended"),
            (TelemetryProfile::Developer, "developer"),
            (TelemetryProfile::Full, "full"),
        ],
        TelemetryProfile::as_str,
    );
    assert_spellings(
        &[
            (NtsEnforcementMode::Warn, "warn"),
            (NtsEnforcementMode::Reject, "reject"),
        ],
        NtsEnforcementMode::as_str,
    );
    assert_spellings(
        &[
            (QueryCursorMode::Ephemeral, "ephemeral"),
            (QueryCursorMode::Stored, "stored"),
        ],
        QueryCursorMode::as_str,
    );
    assert_spellings(
        &[
            (FastpqExecutionMode::Cpu, "cpu"),
            (FastpqExecutionMode::Gpu, "gpu"),
        ],
        FastpqExecutionMode::as_str,
    );
    assert_spellings(
        &[
            (FastpqPoseidonMode::Cpu, "cpu"),
            (FastpqPoseidonMode::Gpu, "gpu"),
        ],
        FastpqPoseidonMode::as_str,
    );
    assert_spellings(
        &[
            (NodeRole::Validator, "validator"),
            (NodeRole::Observer, "observer"),
        ],
        NodeRole::as_str,
    );
}

#[test]
fn configuration_enum_defaults_are_unchanged() {
    assert_eq!(InitMode::default().as_str(), "strict");
    assert_eq!(FsyncMode::default().as_str(), "batched");
    assert_eq!(Format::default().as_str(), "full");
    assert_eq!(Mode::default().as_str(), "read_write");
    assert_eq!(TelemetryProfile::default().as_str(), "operator");
}
