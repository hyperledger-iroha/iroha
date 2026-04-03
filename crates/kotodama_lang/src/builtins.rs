//! Shared builtin classification for the stable Kotodama helper surface.
//!
//! The parser still sees raw identifiers, but semantic analysis, lowering, and
//! effect checks should agree on the canonical builtin set through this enum
//! instead of open-coded string matching.

/// Canonical Kotodama helper/builtin calls that are part of the current source
/// surface and are worth classifying centrally.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    Contains,
    GetOrDefault,
    GetOr,
    Ensure,
    KeysTake2,
    ValuesTake2,
    KeysValuesTake2,
    StateGet,
    StateSet,
    StateDel,
    Path,
    GetInt,
    GetNumeric,
    GetJson,
    GetName,
    GetAccountId,
    GetAssetDefinitionId,
    GetNftId,
    GetBlobHex,
}

impl Builtin {
    /// Resolve a canonical builtin from the user-facing helper name.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "contains" => Self::Contains,
            "get_or_default" => Self::GetOrDefault,
            "get_or" => Self::GetOr,
            "ensure" => Self::Ensure,
            "keys_take2" => Self::KeysTake2,
            "values_take2" => Self::ValuesTake2,
            "keys_values_take2" => Self::KeysValuesTake2,
            "state_get" => Self::StateGet,
            "state_set" => Self::StateSet,
            "state_del" => Self::StateDel,
            "path" => Self::Path,
            "get_int" => Self::GetInt,
            "get_numeric" => Self::GetNumeric,
            "get_json" => Self::GetJson,
            "get_name" => Self::GetName,
            "get_account_id" => Self::GetAccountId,
            "get_asset_definition_id" => Self::GetAssetDefinitionId,
            "get_nft_id" => Self::GetNftId,
            "get_blob_hex" => Self::GetBlobHex,
            _ => return None,
        })
    }

    /// The canonical source spelling for the builtin.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::GetOrDefault => "get_or_default",
            Self::GetOr => "get_or",
            Self::Ensure => "ensure",
            Self::KeysTake2 => "keys_take2",
            Self::ValuesTake2 => "values_take2",
            Self::KeysValuesTake2 => "keys_values_take2",
            Self::StateGet => "state_get",
            Self::StateSet => "state_set",
            Self::StateDel => "state_del",
            Self::Path => "path",
            Self::GetInt => "get_int",
            Self::GetNumeric => "get_numeric",
            Self::GetJson => "get_json",
            Self::GetName => "get_name",
            Self::GetAccountId => "get_account_id",
            Self::GetAssetDefinitionId => "get_asset_definition_id",
            Self::GetNftId => "get_nft_id",
            Self::GetBlobHex => "get_blob_hex",
        }
    }

    /// Whether the builtin is a JSON payload helper that public/view entrypoints
    /// must reject in favor of typed parameters.
    pub const fn is_payload_helper(self) -> bool {
        matches!(
            self,
            Self::GetInt
                | Self::GetNumeric
                | Self::GetJson
                | Self::GetName
                | Self::GetAccountId
                | Self::GetAssetDefinitionId
                | Self::GetNftId
                | Self::GetBlobHex
        )
    }
}
