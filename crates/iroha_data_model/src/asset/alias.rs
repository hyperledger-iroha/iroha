//! Asset definition alias literal.

use std::{format, str::FromStr, string::String};

use iroha_data_model_derive::model;
use iroha_primitives::conststr::ConstString;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use crate::{error::ParseError, name::Name};

#[model]
mod model {
    use derive_more::Display;
    use iroha_schema::IntoSchema;

    use super::*;

    /// Asset alias in either `<name>#<domain>.<dataspace>` or `<name>#<dataspace>` format.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct AssetDefinitionAlias(pub(super) ConstString);
}

impl AssetDefinitionAlias {
    /// Build an alias from validated components.
    ///
    /// # Errors
    /// Returns [`ParseError`] when any component is invalid.
    pub fn from_components(
        name: &str,
        domain_alias: Option<&str>,
        dataspace_alias: &str,
    ) -> Result<Self, ParseError> {
        validate_segment(name, "asset alias name")?;
        if let Some(domain_alias) = domain_alias {
            validate_segment(domain_alias, "asset alias domain")?;
        }
        validate_segment(dataspace_alias, "asset alias dataspace")?;
        let literal = domain_alias.map_or_else(
            || format!("{name}#{dataspace_alias}"),
            |domain_alias| format!("{name}#{domain_alias}.{dataspace_alias}"),
        );
        literal.parse()
    }

    /// Asset name segment (`<name>`).
    #[must_use]
    pub fn name_segment(&self) -> &str {
        let segments = split_alias_segments(self.as_ref())
            .expect("asset alias must remain valid after construction");
        segments.name
    }

    /// Optional alias-domain segment (`<domain>`).
    #[must_use]
    pub fn domain_segment(&self) -> Option<&str> {
        let segments = split_alias_segments(self.as_ref())
            .expect("asset alias must remain valid after construction");
        segments.domain
    }

    /// Dataspace segment (`<dataspace>`).
    #[must_use]
    pub fn dataspace_segment(&self) -> &str {
        let segments = split_alias_segments(self.as_ref())
            .expect("asset alias must remain valid after construction");
        segments.dataspace
    }

    fn validate(value: &str) -> Result<(), ParseError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new("asset alias must not be empty"));
        }
        if trimmed != value {
            return Err(ParseError::new(
                "asset alias must not contain leading or trailing whitespace",
            ));
        }
        if trimmed.chars().any(char::is_control) {
            return Err(ParseError::new(
                "asset alias must not contain control characters",
            ));
        }

        let segments = split_alias_segments(trimmed)?;
        validate_segment(segments.name, "asset alias name")?;
        if let Some(domain) = segments.domain {
            validate_segment(domain, "asset alias domain")?;
        }
        validate_segment(segments.dataspace, "asset alias dataspace")?;
        Ok(())
    }
}

struct AliasSegments<'a> {
    name: &'a str,
    domain: Option<&'a str>,
    dataspace: &'a str,
}

fn split_alias_segments(input: &str) -> Result<AliasSegments<'_>, ParseError> {
    let (name, right) = input.split_once('#').ok_or_else(|| {
        ParseError::new(
            "asset alias must use `<name>#<domain>.<dataspace>` or `<name>#<dataspace>` format",
        )
    })?;
    if right.contains('#') {
        return Err(ParseError::new(
            "asset alias must contain exactly one `#` separator",
        ));
    }
    if right.contains('@') {
        return Err(ParseError::new(
            "asset alias must use `.` instead of `@` between domain and dataspace",
        ));
    }
    let dot_count = right.bytes().filter(|byte| *byte == b'.').count();
    if dot_count == 1 {
        let (domain, dataspace) = right.split_once('.').expect("counted dot");
        return Ok(AliasSegments {
            name,
            domain: Some(domain),
            dataspace,
        });
    }
    if dot_count > 1 {
        return Err(ParseError::new(
            "asset alias must contain at most one `.` after `#`",
        ));
    }
    Ok(AliasSegments {
        name,
        domain: None,
        dataspace: right,
    })
}

fn validate_segment(value: &str, segment: &'static str) -> Result<(), ParseError> {
    if value.is_empty() {
        return Err(ParseError::new("asset alias segments must not be empty"));
    }
    if matches!(segment, "asset alias domain" | "asset alias dataspace") && value.contains('.') {
        return Err(ParseError::new(match segment {
            "asset alias domain" => "asset alias domain segment must not contain `.`",
            "asset alias dataspace" => "asset alias dataspace segment must not contain `.`",
            _ => "asset alias segment must not contain `.`",
        }));
    }
    Name::from_str(value).map_err(|_| {
        ParseError::new(match segment {
            "asset alias name" => "asset alias name segment is invalid",
            "asset alias domain" => "asset alias domain segment is invalid",
            "asset alias dataspace" => "asset alias dataspace segment is invalid",
            _ => "asset alias segment is invalid",
        })
    })?;
    Ok(())
}

impl FromStr for AssetDefinitionAlias {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::validate(value)?;
        Ok(Self(ConstString::from(value)))
    }
}

impl AsRef<str> for AssetDefinitionAlias {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for AssetDefinitionAlias {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AssetDefinitionAlias {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|err: ParseError| norito::json::Error::Message(err.reason.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_alias_parses_valid_literal() {
        let parsed: AssetDefinitionAlias = "usd#issuer.main".parse().expect("valid alias");
        assert_eq!(parsed.name_segment(), "usd");
        assert_eq!(parsed.domain_segment(), Some("issuer"));
        assert_eq!(parsed.dataspace_segment(), "main");
    }

    #[test]
    fn asset_alias_parses_valid_short_literal() {
        let parsed: AssetDefinitionAlias = "usd#main".parse().expect("valid short alias");
        assert_eq!(parsed.name_segment(), "usd");
        assert_eq!(parsed.domain_segment(), None);
        assert_eq!(parsed.dataspace_segment(), "main");
    }

    #[test]
    fn asset_alias_rejects_invalid_literals() {
        for raw in [
            "",
            " ",
            "usd",
            "usd@main",
            "usd##issuer.main",
            "usd#issuer@@main",
            "#issuer.main",
            "usd#.main",
            "usd#issuer.",
            "usd#main.extra.tail",
            "usd coin#issuer.main",
            "usd#issuer@main",
        ] {
            assert!(
                raw.parse::<AssetDefinitionAlias>().is_err(),
                "must fail: {raw}"
            );
        }
    }

    #[test]
    fn asset_alias_from_components_builds_long_literal() {
        let alias = AssetDefinitionAlias::from_components("usd", Some("issuer"), "main")
            .expect("components should build");
        assert_eq!(alias.as_ref(), "usd#issuer.main");
    }

    #[test]
    fn asset_alias_from_components_builds_short_literal() {
        let alias = AssetDefinitionAlias::from_components("usd", None, "main")
            .expect("components should build");
        assert_eq!(alias.as_ref(), "usd#main");
    }

    #[test]
    fn asset_alias_rejects_dotted_domain_and_dataspace_segments() {
        assert!(AssetDefinitionAlias::from_components("usd", Some("issuer.sub"), "main").is_err());
        assert!(AssetDefinitionAlias::from_components("usd", Some("issuer"), "main.ops").is_err());
    }
}
