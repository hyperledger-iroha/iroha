//! Configuration tools related to Kura specifically.
use std::str::FromStr;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self as ncore, Archived},
    json::{self, JsonDeserialize, JsonSerialize},
};
/// Kura initialization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum InitMode {
    /// Strict validation of all blocks.
    #[default]
    Strict,
    /// Fast initialization with basic checks.
    Fast,
}
impl JsonSerialize for InitMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}
impl JsonDeserialize for InitMode {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        InitMode::from_str(&value).map_err(|err| json::Error::InvalidField {
            field: "init_mode".into(),
            message: err.to_string(),
        })
    }
}
impl NoritoSerialize for InitMode {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
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
impl<'de> NoritoDeserialize<'de> for InitMode {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("stored init mode must parse")
    }
    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, ncore::Error> {
        let text = <String as NoritoDeserialize>::deserialize(archived.cast());
        InitMode::from_str(&text).map_err(|err| ncore::Error::Message(err.to_string()))
    }
}
/// Fsync policy for Kura block storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FsyncMode {
    /// Synchronously fsync after each batch write.
    Always,
    /// Batch fsyncs using a time-based interval.
    #[default]
    Batched,
}
impl JsonSerialize for FsyncMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}
impl JsonDeserialize for FsyncMode {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        FsyncMode::from_str(&value).map_err(|err| json::Error::InvalidField {
            field: "fsync_mode".into(),
            message: err.to_string(),
        })
    }
}
impl NoritoSerialize for FsyncMode {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
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
impl<'de> NoritoDeserialize<'de> for FsyncMode {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("stored fsync mode must parse")
    }
    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, ncore::Error> {
        let text = <String as NoritoDeserialize>::deserialize(archived.cast());
        FsyncMode::from_str(&text).map_err(|err| ncore::Error::Message(err.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use crate::kura::{FsyncMode, InitMode};
    #[test]
    fn init_mode_display_reprs() {
        assert_eq!(format!("{}", InitMode::Strict), "strict");
        assert_eq!(format!("{}", InitMode::Fast), "fast");
        assert_eq!("strict".parse::<InitMode>().unwrap(), InitMode::Strict);
        assert_eq!("fast".parse::<InitMode>().unwrap(), InitMode::Fast);
    }
    #[test]
    fn fsync_mode_display_reprs() {
        assert_eq!(format!("{}", FsyncMode::Always), "always");
        assert_eq!(format!("{}", FsyncMode::Batched), "batched");
        assert_eq!("always".parse::<FsyncMode>().unwrap(), FsyncMode::Always);
        assert_eq!("batched".parse::<FsyncMode>().unwrap(), FsyncMode::Batched);
        assert!("off".parse::<FsyncMode>().is_err());
        assert!("on".parse::<FsyncMode>().is_err());
        assert!(norito::json::from_json::<FsyncMode>(r#""off""#).is_err());
        assert!(norito::json::from_json::<FsyncMode>(r#""on""#).is_err());
    }
}
