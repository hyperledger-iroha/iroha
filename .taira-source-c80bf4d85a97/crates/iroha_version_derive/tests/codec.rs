//! Versioned Norito codec tests for `iroha_version_derive`.
#[cfg(test)]
mod tests {
    use iroha_version::{
        RawVersioned,
        codec::*,
        error::{Error, Result},
    };
    use iroha_version_derive::{declare_versioned, version};
    use norito::{
        Decode as NoritoDecode, Encode as NoritoEncode,
        json::{JsonDeserialize, JsonSerialize},
    };

    mod model_1 {
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
        let bytes = versioned_message.encode_versioned();
        let decoded_message =
            VersionedMessage::decode_all_versioned(&bytes).map_err(|e| e.to_string())?;
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

        let bytes = {
            use model_2::*;

            let versioned_message: VersionedMessage = Message3("test string".to_string()).into();
            versioned_message.encode_versioned()
        };

        let decoded_message = VersionedMessage::decode_all_versioned(&bytes);
        match decoded_message {
            Err(Error::UnsupportedVersion(unsupported_version)) => {
                assert_eq!(unsupported_version.version, 3);
                if let RawVersioned::NoritoBytes(bytes) = unsupported_version.raw {
                    assert_eq!(bytes[0], 3);
                    assert!(!bytes[1..].is_empty());
                    Ok(())
                } else {
                    Err("Should be Norito bytes.".to_owned())
                }
            }
            _ => Err("Should be an unsupported version".to_owned()),
        }
    }
}
