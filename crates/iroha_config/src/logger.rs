//! Configuration utils related to Logger specifically.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

pub use iroha_data_model::Level;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self as ncore, Archived},
    json::{self, JsonDeserialize, JsonSerialize},
};
use tracing_subscriber::filter::Directive;

/// Reflects formatters in [`mod@tracing_subscriber::fmt::format`]
#[derive(Debug, Copy, Clone, Eq, PartialEq, strum::Display, strum::EnumString, Default)]
#[strum(serialize_all = "snake_case")]
pub enum Format {
    /// See [`tracing_subscriber::fmt::format::Full`]
    #[default]
    Full,
    /// See [`tracing_subscriber::fmt::format::Compact`]
    Compact,
    /// See [`tracing_subscriber::fmt::format::Pretty`]
    Pretty,
    /// See [`tracing_subscriber::fmt::format::Json`]
    Json,
}

impl JsonSerialize for Format {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for Format {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Format::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "format".into(),
            message: err.to_string(),
        })
    }
}

impl NoritoSerialize for Format {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
        let text = self.to_string();
        <String as NoritoSerialize>::serialize(&text, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(self.to_string().len())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> NoritoDeserialize<'de> for Format {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("stored logger format strings must parse successfully")
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, ncore::Error> {
        let text = <String as NoritoDeserialize>::deserialize(archived.cast());
        Format::from_str(&text).map_err(|err| ncore::Error::Message(err.to_string()))
    }
}

/// List of filtering directives
#[derive(Clone, PartialEq, Eq)]
pub struct Directives(Vec<Directive>);

impl Directives {
    /// Join two sets of directives
    pub fn extend(&mut self, Directives(vec): Directives) {
        self.0.extend(vec);
    }
}

impl FromStr for Directives {
    type Err = tracing_subscriber::filter::ParseError;

    fn from_str(dirs: &str) -> std::result::Result<Self, Self::Err> {
        if dirs.is_empty() {
            return Ok(Self(Vec::new()));
        }
        let directives = dirs
            .split(',')
            .filter(|s| !s.is_empty())
            .map(FromStr::from_str)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self(directives))
    }
}

impl Display for Directives {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut directives_iter = self.0.iter();
        if let Some(directive) = directives_iter.next() {
            write!(f, "{directive}")?;
        }
        for directive in directives_iter {
            write!(f, ",{directive}")?;
        }
        Ok(())
    }
}

impl Debug for Directives {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

impl From<Level> for Directives {
    fn from(level: Level) -> Self {
        Directives(Vec::from([into_tracing_level(level).into()]))
    }
}

impl Default for Directives {
    fn default() -> Self {
        Self::from(Level::INFO)
    }
}

impl JsonSerialize for Directives {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for Directives {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        Directives::from_str(&value).map_err(|err| json::Error::InvalidField {
            field: "directives".into(),
            message: err.to_string(),
        })
    }
}

impl NoritoSerialize for Directives {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
        let text = self.to_string();
        <String as NoritoSerialize>::serialize(&text, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(self.to_string().len())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> NoritoDeserialize<'de> for Directives {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("stored logger directives must parse successfully")
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, ncore::Error> {
        let text = <String as NoritoDeserialize>::deserialize(archived.cast());
        Directives::from_str(&text).map_err(|err| ncore::Error::Message(err.to_string()))
    }
}

/// Convert [`Level`] into [`tracing::Level`]
fn into_tracing_level(level: Level) -> tracing::Level {
    match level {
        Level::TRACE => tracing::Level::TRACE,
        Level::DEBUG => tracing::Level::DEBUG,
        Level::INFO => tracing::Level::INFO,
        Level::WARN => tracing::Level::WARN,
        Level::ERROR => tracing::Level::ERROR,
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::Level;

    use crate::logger::{Directives, Format};

    #[test]
    fn serialize_pretty_format_in_lowercase() {
        let value = Format::Pretty;
        let actual = norito::json::to_json(&value).unwrap();
        assert_eq!("\"pretty\"", actual);
    }

    #[test]
    fn parse_display_directives() {
        for sample in [
            "iroha_core=info",
            "info",
            "trace",
            "iroha_p2p=trace,axum=warn",
        ] {
            let value: Directives = sample.parse().unwrap();
            assert_eq!(format!("{value}"), sample)
        }
    }

    #[test]
    fn directives_from_level() {
        for level in [
            Level::DEBUG,
            Level::DEBUG,
            Level::TRACE,
            Level::INFO,
            Level::WARN,
            Level::ERROR,
        ] {
            let value: Directives = level.into();
            assert_eq!(format!("{value}"), format!("{level}").to_lowercase())
        }
    }
}
