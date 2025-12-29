//! Configuration related to Snapshot specifically

use std::str::FromStr;

use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self as ncore, Archived},
    json::{self, JsonDeserialize, JsonSerialize},
};

/// Functioning mode of the Snapshot Iroha module
#[derive(Copy, Clone, Debug, Default, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum Mode {
    /// Read the snapshot on startup, update periodically
    #[default]
    ReadWrite,
    /// Read the snapshot on startup, do not update
    Readonly,
    /// Do not read or write the snapshot
    Disabled,
}

impl JsonSerialize for Mode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for Mode {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        Mode::from_str(&value).map_err(|err| json::Error::InvalidField {
            field: "snapshot_mode".into(),
            message: err.to_string(),
        })
    }
}

impl NoritoSerialize for Mode {
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

impl<'de> NoritoDeserialize<'de> for Mode {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("stored snapshot mode must parse")
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, ncore::Error> {
        let text = <String as NoritoDeserialize>::deserialize(archived.cast());
        Mode::from_str(&text).map_err(|err| ncore::Error::Message(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::snapshot::Mode;

    #[test]
    fn mode_display_form() {
        assert_eq!(
            format!("{} {} {}", Mode::ReadWrite, Mode::Readonly, Mode::Disabled),
            "read_write readonly disabled"
        );
    }
}
