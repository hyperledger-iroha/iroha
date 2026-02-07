//! Versioned JSON codec tests for `iroha_version_derive`.
#[cfg(test)]
mod tests {
    use iroha_version::{
        RawVersioned,
        error::{Error, Result},
        json::*,
    };
    use iroha_version_derive::{declare_versioned, version};
    use norito::{
        Decode as NoritoDecode, Encode as NoritoEncode,
        json::{JsonDeserialize, JsonSerialize, Value},
    };

    mod model_1 {
        #![allow(unused_results)]
        use super::*;

        declare_versioned!(VersionedMessage 1..3, Debug, Clone, iroha_macro::FromVariant);

        #[version(version = 1, versioned_alias = "VersionedMessage")]
        #[derive(Debug, Clone, NoritoDecode, NoritoEncode)]
        pub struct Message;

        impl JsonSerialize for Message {
            fn json_serialize(&self, out: &mut String) {
                out.push_str("null");
            }
        }

        impl JsonDeserialize for Message {
            fn json_deserialize(
                p: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                p.parse_null()?;
                Ok(Self)
            }
        }

        #[version(version = 2, versioned_alias = "VersionedMessage")]
        #[derive(Debug, Clone, NoritoDecode, NoritoEncode)]
        pub struct Message2;

        impl JsonSerialize for Message2 {
            fn json_serialize(&self, out: &mut String) {
                out.push_str("null");
            }
        }

        impl JsonDeserialize for Message2 {
            fn json_deserialize(
                p: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                p.parse_null()?;
                Ok(Self)
            }
        }
    }

    mod model_2 {
        #![allow(unused_results)]

        use super::*;

        declare_versioned!(VersionedMessage 1..4, Debug, Clone, iroha_macro::FromVariant);

        #[version(version = 1, versioned_alias = "VersionedMessage")]
        #[derive(Debug, Clone, NoritoDecode, NoritoEncode)]
        pub struct Message;

        impl JsonSerialize for Message {
            fn json_serialize(&self, out: &mut String) {
                out.push_str("null");
            }
        }

        impl JsonDeserialize for Message {
            fn json_deserialize(
                p: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                p.parse_null()?;
                Ok(Self)
            }
        }

        #[version(version = 2, versioned_alias = "VersionedMessage")]
        #[derive(Debug, Clone, NoritoDecode, NoritoEncode)]
        pub struct Message2;

        impl JsonSerialize for Message2 {
            fn json_serialize(&self, out: &mut String) {
                out.push_str("null");
            }
        }

        impl JsonDeserialize for Message2 {
            fn json_deserialize(
                p: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                p.parse_null()?;
                Ok(Self)
            }
        }

        #[version(version = 3, versioned_alias = "VersionedMessage")]
        #[derive(Debug, Clone, NoritoDecode, NoritoEncode)]
        pub struct Message3(pub String);

        impl JsonSerialize for Message3 {
            fn json_serialize(&self, out: &mut String) {
                self.0.json_serialize(out);
            }
        }

        impl JsonDeserialize for Message3 {
            fn json_deserialize(
                p: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                let inner = String::json_deserialize(p)?;
                Ok(Self(inner))
            }
        }
    }

    #[test]
    fn supported_version() -> Result<(), String> {
        use model_1::*;

        let versioned_message: VersionedMessage = Message.into();
        let json = versioned_message
            .to_versioned_json_str()
            .map_err(|e| e.to_string())?;
        let decoded_message =
            VersionedMessage::from_versioned_json_str(&json).map_err(|e| e.to_string())?;
        match decoded_message {
            VersionedMessage::V1(message) => {
                let _: Message = message;
                Ok(())
            }
            VersionedMessage::V2(message) => {
                let _: Message2 = message;
                Err("Should have been message v1.".to_owned())
            }
        }
    }

    #[test]
    fn unsupported_version() -> Result<(), String> {
        use model_1::*;

        let json = {
            use model_2::*;

            let versioned_message: VersionedMessage = Message3("test string".to_string()).into();
            versioned_message
                .to_versioned_json_str()
                .map_err(|e| e.to_string())?
        };

        let raw_string = "{\"version\":\"3\",\"content\":\"test string\"}";
        let decoded_message = VersionedMessage::from_versioned_json_str(&json);
        match decoded_message {
            Err(Error::UnsupportedVersion(unsupported_version)) => {
                assert_eq!(unsupported_version.version, 3);
                if let RawVersioned::Json(json) = unsupported_version.raw {
                    let json: Value = norito::json::from_str(&json).map_err(|e| e.to_string())?;
                    let raw_string: Value =
                        norito::json::from_str(raw_string).map_err(|e| e.to_string())?;
                    assert_eq!(json, raw_string);
                    Ok(())
                } else {
                    Err("Should be json.".to_owned())
                }
            }
            _ => Err("Should be an unsupported version".to_owned()),
        }
    }
}
