use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize};
use thiserror::Error;

pub use self::model::*;

#[model]
mod model {
    use super::*;

    /// Log level for reading from environment and (de)serializing
    #[derive(
        Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
    )]
    #[allow(clippy::upper_case_acronyms)]
    #[repr(u8)]
    pub enum Level {
        /// Trace
        TRACE,
        /// Debug
        DEBUG,
        /// Info (Default)
        #[default]
        INFO,
        /// Warn
        WARN,
        /// Error
        ERROR,
    }
}

impl ::core::fmt::Display for Level {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(match self {
            Self::TRACE => "TRACE",
            Self::DEBUG => "DEBUG",
            Self::INFO => "INFO",
            Self::WARN => "WARN",
            Self::ERROR => "ERROR",
        })
    }
}

#[derive(Debug, Error, Clone, Copy)]
#[error("invalid log level")]
/// Error returned when parsing a log level from text fails.
pub struct ParseLevelError;

impl ::core::str::FromStr for Level {
    type Err = ParseLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Accept case-insensitive inputs for ergonomics (e.g., LOG_LEVEL=info)
        match s.to_ascii_uppercase().as_str() {
            "TRACE" => Ok(Self::TRACE),
            "DEBUG" => Ok(Self::DEBUG),
            "INFO" => Ok(Self::INFO),
            "WARN" => Ok(Self::WARN),
            "ERROR" => Ok(Self::ERROR),
            _ => Err(ParseLevelError),
        }
    }
}

impl ::core::convert::TryFrom<u8> for Level {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::TRACE),
            1 => Ok(Self::DEBUG),
            2 => Ok(Self::INFO),
            3 => Ok(Self::WARN),
            4 => Ok(Self::ERROR),
            _ => Err(()),
        }
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for Level {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Level {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value
            .parse::<Level>()
            .map_err(|_| json::Error::InvalidField {
                field: "level".into(),
                message: format!("unexpected level `{value}`"),
            })
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use norito::json::{self, FastJsonWrite};

    use super::*;

    #[test]
    fn parse_level_from_str() {
        assert_eq!("INFO".parse::<Level>().unwrap(), Level::INFO);
    }

    #[test]
    fn parse_level_from_str_case_insensitive() {
        assert_eq!("info".parse::<Level>().unwrap(), Level::INFO);
        assert_eq!("Warn".parse::<Level>().unwrap(), Level::WARN);
        assert_eq!("DeBuG".parse::<Level>().unwrap(), Level::DEBUG);
    }

    #[test]
    fn parse_invalid_level_from_str() {
        assert!("invalid".parse::<Level>().is_err());
    }

    #[test]
    fn level_try_from_u8() {
        assert_eq!(Level::try_from(2).unwrap(), Level::INFO);
        assert!(Level::try_from(5).is_err());
    }

    #[test]
    fn level_json_roundtrip() {
        let mut json_repr = String::new();
        Level::WARN.write_json(&mut json_repr);
        assert_eq!(json_repr, "\"WARN\"");

        let decoded: Level = json::from_json(&json_repr).expect("deserialize");
        assert_eq!(decoded, Level::WARN);
    }
}
